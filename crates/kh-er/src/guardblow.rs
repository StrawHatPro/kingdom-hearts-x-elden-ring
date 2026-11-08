//! (THE USER 2026-11-02: "For Goofy and Sora, and any character added in the future, make sure an enemy hitting them while
//! they are guarding plays the 'guard' sound effect instead of hit and they don't get bloody from it.") A BLOW HIS GUARD
//! TOOK, FOR EVERY KINGDOM HEARTS CHARACTER, BY ONE ROUTE.
//!
//! WHERE THE HOST LEARNS IT (each place calls `guarded(who, at)`, nothing else is per character):
//! - Sora, the leader or AI Sora: his core's own verdict in the step's events (`sora_events`): `PlayerGuarded` (his guard
//!   took it: a guard, a just guard, a guard whose gate refused) or `PlayerGuard` with a reflect result (a guard's recoil);
//! - every friend, the leader (party/leader.rs `hit_now`) or AI (party/glue.rs, the bodies' hits): kh-party's
//!   `HitResult::guarded` (Goofy's shield block, every deflect / parry / technique; Donald's Magic Block; whatever guard a
//!   character added later answers with `guarded`).
//!
//! WHAT IT DOES:
//! - NO BLOOD from it: blood.rs `note_guarded` (that frame's HP change adds none, whatever the HP did);
//! - HIS GUARD SOUND, from his row `guard_sound.<name>` of sheets/port/sora-er-host.json (name: `PartyChara::name`, lower
//!   case): a name the audio seam plays (an SE asset by name, or `effect:<particle system>` = the sound that effect
//!   carries), `kit` = his kit already plays his guard sounds itself (Goofy: guard.block_se, the deflect's VN_slg_hit0
//!   with se00302_002, parry.right_se of party-kit-goofy; Donald: block.se of party-kit-donald), `-` = none. A character
//!   with no row takes `guard_sound.default`: KH3's COMMON guard sound (`m_CmnGuardReflectEffect` vs_com_haj00 carries
//!   se00001_037; KH3 spawns it when the guarding character has no guard effect of his own: combat spec 8.6), so a
//!   character added later is heard guarding with no new code. Sora's row is that same KH3 sound: in KH3 the attacker's
//!   hit spawned it on his guard, and an Elden Ring attacker runs none of KH3's hit code, so before this nothing of KH3
//!   played when Sora guarded an Elden Ring blow;
//! - ELDEN RING'S OWN SOUNDS OF THE HIDDEN BODY ARE HELD OFF while the leader's guard takes a blow (the leader's body is
//!   the only one the game's own hits reach: party/allybody_live.rs GF-INGAME, the friends' bodies are hit by our sensor):
//!   the game's `ChrInsFlags1ca::sounds_active` (the switch its distance rule drives; zpose.rs mutes a held enemy with it)
//!   is held at 0 on the Tarnished WHILE SORA'S GUARD IS UP (his guard parts on: a blow now meets his guard) and for
//!   `guard_sound.quiet_s` after any guarded blow on the leader, and given back after. Whether the game's impact sound of
//!   a hit goes through that switch is NOT MEASURED (the Tarnished's own hurt sounds do, as any character's): UNTESTED IN
//!   GAME. Control key `guard_sound.quiet = 0`: the body's sounds are left alone.
//!   GIVEN BACK AT ONCE, and only when WE set it to 0 (a 0 the game had set itself is never touched): when the hold ends,
//!   when `guard_sound.quiet = 0` is set mid-hold (`hold_body_sounds`), when the KH host stops (sora.rs `SoraHost::stop`:
//!   the payload's shutdown, a change of `mode`, a new Sora after a death the game was handed: frame.rs `release` and the
//!   respawn) and when the player dies (frame.rs, the first dead frame: `release_body_sounds`). When the player object is
//!   gone (a load, the quit to title, a respawn that replaces it) there is nothing to write: the note is dropped
//!   (`forget_body`), and a new body starts with the game's own value.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use glam::Vec3;
use kh_core::host::{Attach, Event, EventKind, ReactionResult, SoundId, SoundPlay};
use kh_host::party::PartyChara;

use crate::erlog;

/// What a character's guard sound row says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardSound {
    /// The audio seam plays this name (an SE asset, or `effect:<particle system>`).
    Play(&'static str),
    /// His kit plays its own guard sounds (kh-party's rows).
    Kit,
    /// Nothing.
    Silent,
}

impl GuardSound {
    fn parse(text: &str) -> Option<GuardSound> {
        match text.trim() {
            "" => None,
            "kit" => Some(GuardSound::Kit),
            "-" => Some(GuardSound::Silent),
            name => Some(GuardSound::Play(leak(name))),
        }
    }
}

/// A row's text made `'static` once (the audio seam's names are `&'static str`); the same text is leaked only once.
fn leak(name: &str) -> &'static str {
    static NAMES: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    let Ok(mut names) = NAMES.lock() else { return "se00001_037" };
    if let Some(n) = names.iter().find(|n| **n == name) {
        return n;
    }
    let n: &'static str = Box::leak(name.to_string().into_boxed_str());
    names.push(n);
    n
}

/// The rows `guard_sound.*` (sora-er-host).
#[derive(Clone, Debug, PartialEq)]
pub struct GuardRules {
    /// (row key = the character's name in lower case, the sound)
    pub rows: Vec<(String, GuardSound)>,
    /// A character with no row of his own.
    pub default: GuardSound,
    /// Seconds the leader's hidden body stays silent after a guarded blow.
    pub quiet_s: f32,
}

/// KH3's common guard sound (vs_com_haj00's se00001_037).
pub const COMMON_GUARD_SE: &str = "se00001_037";

impl Default for GuardRules {
    fn default() -> Self {
        GuardRules {
            rows: vec![("sora".to_string(), GuardSound::Play(COMMON_GUARD_SE)), ("donald".to_string(), GuardSound::Kit), ("goofy".to_string(), GuardSound::Kit)],
            default: GuardSound::Play(COMMON_GUARD_SE),
            quiet_s: 0.6,
        }
    }
}

impl GuardRules {
    /// The rows of the sheet text over the defaults (a row not understood keeps its default); the rows not understood.
    pub fn from_sheet(text: &str) -> (GuardRules, Vec<String>) {
        let mut r = GuardRules::default();
        let mut problems = Vec::new();
        let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else { return (r, vec!["the sheet is not JSON".to_string()]) };
        for row in json.get("rows").and_then(|v| v.as_array()).into_iter().flatten() {
            let Some(id) = row.get(0).and_then(|v| v.as_str()) else { continue };
            let Some(key) = id.strip_prefix("guard_sound.") else { continue };
            let value = row.get(1);
            let ok = match key {
                "quiet_s" => value.and_then(|v| v.as_f64()).map(|v| v as f32).filter(|v| v.is_finite() && (0.0..=10.0).contains(v)).map(|v| r.quiet_s = v).is_some(),
                "default" => value.and_then(|v| v.as_str()).and_then(GuardSound::parse).map(|s| r.default = s).is_some(),
                who => match value.and_then(|v| v.as_str()).and_then(GuardSound::parse) {
                    Some(s) => {
                        match r.rows.iter_mut().find(|(k, _)| k == who) {
                            Some(row) => row.1 = s,
                            None => r.rows.push((who.to_string(), s)),
                        }
                        true
                    }
                    None => false,
                },
            };
            if !ok {
                problems.push(id.to_string());
            }
        }
        (r, problems)
    }

    /// The guard sound of `who`: his row, else the default.
    pub fn sound_of(&self, who: &str) -> GuardSound {
        let key = who.to_ascii_lowercase();
        self.rows.iter().find(|(k, _)| *k == key).map_or(self.default, |(_, s)| *s)
    }
}

/// The route's state (engine-free: unit-tested with its own clock).
#[derive(Default)]
pub struct GuardBlows {
    pub rules: GuardRules,
    /// The guard sounds asked for since the last `take_sounds`.
    sounds: Vec<SoundPlay>,
    /// (who, until): the leader's hidden body stays silent until then (s of the route's clock).
    quiet: Vec<(PartyChara, f64)>,
    /// Guarded blows by character since the start (the log's counts).
    pub counts: [u32; 3],
}

impl GuardBlows {
    /// A blow on `who` (at `at`, KH3 cm) was guarded at `now` (s).
    pub fn guarded(&mut self, who: PartyChara, at: Vec3, now: f64) -> GuardSound {
        let sound = self.rules.sound_of(who.name());
        if let GuardSound::Play(name) = sound {
            self.sounds.push(SoundPlay { id: SoundId(kh_core::notify::notify_asset_id(name)), name, at: Attach::World(at), volume: 1.0, pitch: 1.0, looping: false, spatial: true });
        }
        let until = now + self.rules.quiet_s as f64;
        match self.quiet.iter_mut().find(|q| q.0 == who) {
            Some(q) => q.1 = q.1.max(until),
            None => self.quiet.push((who, until)),
        }
        if let Some(n) = self.counts.get_mut(who.slot()) {
            *n += 1;
        }
        sound
    }

    pub fn take_sounds(&mut self) -> Vec<SoundPlay> {
        std::mem::take(&mut self.sounds)
    }

    /// Whether the hidden body of the leader `who` is to be silent at `now`: his guard is up, or a blow his guard took is
    /// less than `quiet_s` old.
    pub fn quiet(&mut self, who: PartyChara, guard_up: bool, now: f64) -> bool {
        self.quiet.retain(|q| q.1 > now);
        guard_up || self.quiet.iter().any(|q| q.0 == who)
    }
}

/// Sora's guarded blows of a step (the leader's or AI Sora's: his core's verdict): at most one per step (a GUARD result
/// may come as `PlayerGuard` and `PlayerGuarded` both).
pub fn sora_guarded_in(events: &[Event]) -> bool {
    events.iter().any(|e| match e.kind {
        EventKind::PlayerGuarded { .. } => true,
        EventKind::PlayerGuard { victim, .. } => matches!(victim, ReactionResult::Reflect | ReactionResult::GuardReflect),
        _ => false,
    })
}

static STATE: Mutex<Option<GuardBlows>> = Mutex::new(None);
static START: OnceLock<std::time::Instant> = OnceLock::new();

fn now() -> f64 {
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
}

/// The rows of the host sheet (sora.rs at Sora's start, beside blood.rs's).
pub fn load(sheet: &std::path::Path) {
    let rules = match std::fs::read_to_string(sheet) {
        Ok(text) => {
            let (rules, problems) = GuardRules::from_sheet(&text);
            erlog!("guard: guard sounds of {} ({} not understood{}): {:?}, any other character {:?}; the leader's hidden body silent {:.1} s after a guarded blow", sheet.display(), problems.len(), if problems.is_empty() { String::new() } else { format!(": {}", problems.join(", ")) }, rules.rows, rules.default, rules.quiet_s);
            rules
        }
        Err(e) => {
            erlog!("guard: {} not read ({e}): the built-in guard sounds", sheet.display());
            GuardRules::default()
        }
    };
    if let Ok(mut g) = STATE.lock() {
        g.get_or_insert_with(GuardBlows::default).rules = rules;
    }
}

/// A BLOW ON `who` WAS GUARDED (`at`: where, KH3 cm): no blood from it, his guard sound, his hidden body silent.
pub fn guarded(who: PartyChara, at: Vec3) {
    crate::blood::note_guarded(who);
    let Ok(mut g) = STATE.lock() else { return };
    let state = g.get_or_insert_with(GuardBlows::default);
    let sound = state.guarded(who, at, now());
    let n = state.counts.get(who.slot()).copied().unwrap_or(0);
    if n <= 20 || n.is_power_of_two() {
        erlog!(
            "guard: a blow on {} WAS GUARDED ({n} so far): no blood from it; {}",
            who.name(),
            match sound {
                GuardSound::Play(name) => format!("his guard sound {name} (row guard_sound.{})", who.name().to_ascii_lowercase()),
                GuardSound::Kit => "his kit's own guard sound".to_string(),
                GuardSound::Silent => "no guard sound (his row says none)".to_string(),
            }
        );
    }
}

/// Sora's step (sora.rs, before the step's sounds are handed over): his guarded blow of the step, at `at` (KH3 cm).
pub fn sora_step(events: &[Event], at: Vec3) {
    if sora_guarded_in(events) {
        guarded(PartyChara::Sora, at);
    }
}

/// The guard sounds asked for since the last call (sora.rs plays them through the effect side's audio queue).
pub fn take_sounds() -> Vec<SoundPlay> {
    STATE.lock().ok().and_then(|mut g| g.as_mut().map(GuardBlows::take_sounds)).unwrap_or_default()
}

/// The hidden body's sounds were switched off BY US (to be given back; a 0 the game set itself is not ours).
static HELD: AtomicBool = AtomicBool::new(false);

/// One phase of the hold, engine-free: `held` = we set the switch to 0; `active` = the switch now; `quiet` = the hold
/// wants silence. Returns what to write (None: leave it alone).
pub fn hold_step(held: &mut bool, active: bool, quiet: bool) -> Option<bool> {
    if !quiet {
        return give_back(held);
    }
    // (the game may switch it on again within the hold: written off again, still ours)
    active.then(|| {
        *held = true;
        false
    })
}

/// The hold ends at once: `Some(true)` when we were the ones who set it to 0, then never again.
pub fn give_back(held: &mut bool) -> Option<bool> {
    std::mem::take(held).then_some(true)
}

/// EVERY PHASE (sora.rs `assert_flags`): the leader's hidden body silent while his guard takes a blow (module text);
/// `on` false (`guard_sound.quiet = 0`) gives it back at once.
pub fn hold_body_sounds(chr: &mut eldenring::cs::ChrIns, leader: PartyChara, guard_up: bool, on: bool) {
    let quiet = on && STATE.lock().ok().is_some_and(|mut g| g.get_or_insert_with(GuardBlows::default).quiet(leader, guard_up, now()));
    let mut held = HELD.load(Ordering::Relaxed);
    let was = held;
    let write = hold_step(&mut held, chr.chr_flags1ca.sounds_active(), quiet);
    HELD.store(held, Ordering::Relaxed);
    if let Some(v) = write {
        chr.chr_flags1ca.set_sounds_active(v);
    }
    if held && !was {
        erlog!("guard: the hidden body's own sounds held off ({} guards)", leader.name());
    } else if was && !held && !on {
        erlog!("guard: guard_sound.quiet = 0: the hidden body's own sounds given back at once");
    }
}

/// THE KH HOST STOPS / THE PLAYER DIES (module text): the body's sounds given back at once if we hold them off, and the
/// leader's quiet windows dropped. `why`: for the log.
pub fn release_body_sounds(chr: &mut eldenring::cs::ChrIns, why: &str) {
    forget_quiet();
    let mut held = HELD.load(Ordering::Relaxed);
    let write = give_back(&mut held);
    HELD.store(held, Ordering::Relaxed);
    if let Some(v) = write {
        chr.chr_flags1ca.set_sounds_active(v);
        erlog!("guard: {why}: the hidden body's own sounds given back (we had held them off)");
    }
}

/// THE PLAYER OBJECT IS GONE (a load, the quit to title, a respawn that replaces it): nothing to write; our note dropped.
pub fn forget_body(why: &str) {
    forget_quiet();
    if HELD.swap(false, Ordering::Relaxed) {
        erlog!("guard: {why}: the hidden body we held silent is gone; the next one keeps the game's own sound switch");
    }
}

fn forget_quiet() {
    if let Ok(mut g) = STATE.lock() {
        if let Some(s) = g.as_mut() {
            s.quiet.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_core::states::HitOnPlayer;

    const SHEET: &str = include_str!("../../../sheets/port/sora-er-host.json");

    fn hit() -> HitOnPlayer {
        HitOnPlayer::new(kh_core::enums::ETresDamageKind::Small, 20.0, kh_core::TargetId(7), Vec3::X * 100.0)
    }

    #[test]
    fn the_sheet_rows_are_the_defaults() {
        let (rules, problems) = GuardRules::from_sheet(SHEET);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(rules, GuardRules::default());
    }

    /// (THE USER 2026-11-02) A guarded blow on Sora plays KH3's guard sound (his row), on a friend his kit's own (no second
    /// one), and on a character with no row of his own KH3's common guard sound (a character added later: no new code).
    #[test]
    fn a_guarded_blow_plays_the_characters_guard_sound_from_his_row() {
        let mut g = GuardBlows::default();
        assert_eq!(g.guarded(PartyChara::Sora, Vec3::new(1.0, 2.0, 3.0), 0.0), GuardSound::Play(COMMON_GUARD_SE));
        let sounds = g.take_sounds();
        assert_eq!(sounds.len(), 1);
        assert_eq!((sounds[0].name, sounds[0].at), (COMMON_GUARD_SE, Attach::World(Vec3::new(1.0, 2.0, 3.0))));
        assert!(g.take_sounds().is_empty(), "taken once");
        // Goofy and Donald: their kits' own clangs (guard.block_se, the deflect's, block.se): nothing doubled here
        assert_eq!(g.guarded(PartyChara::Goofy, Vec3::ZERO, 0.0), GuardSound::Kit);
        assert_eq!(g.guarded(PartyChara::Donald, Vec3::ZERO, 0.0), GuardSound::Kit);
        assert!(g.take_sounds().is_empty());
        // a character with no row: the default (KH3's common guard sound)
        assert_eq!(g.rules.sound_of("Riku"), GuardSound::Play(COMMON_GUARD_SE));
        // a row of his own wins, in any case of the name
        let text = r#"{"rows":[["guard_sound.riku","se09999_001"],["guard_sound.goofy","-"],["guard_sound.default","effect:vs_com_haj00"]]}"#;
        let (rules, problems) = GuardRules::from_sheet(text);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(rules.sound_of("Riku"), GuardSound::Play("se09999_001"));
        assert_eq!(rules.sound_of("Goofy"), GuardSound::Silent);
        assert_eq!(rules.sound_of("Kairi"), GuardSound::Play("effect:vs_com_haj00"));
        // a malformed row keeps its default and is named
        let (rules, problems) = GuardRules::from_sheet(r#"{"rows":[["guard_sound.sora",3],["guard_sound.quiet_s","x"]]}"#);
        assert_eq!(problems, vec!["guard_sound.sora".to_string(), "guard_sound.quiet_s".to_string()]);
        assert_eq!(rules, GuardRules::default());
    }

    /// Sora's core says a blow was guarded by `PlayerGuarded` or a reflect `PlayerGuard`; a landed hit (`PlayerHit`) or a
    /// dodge is no guard (an unguarded blow is unchanged: no guard sound).
    #[test]
    fn soras_guarded_blow_is_his_cores_verdict() {
        let ev = |kind| Event { step: 1, kind };
        assert!(sora_guarded_in(&[ev(EventKind::PlayerGuarded { hit: hit(), success: true, critical: false })]));
        assert!(sora_guarded_in(&[ev(EventKind::PlayerGuard { hit: hit(), victim: ReactionResult::GuardReflect, attacker: ReactionResult::NoReaction })]));
        assert!(!sora_guarded_in(&[ev(EventKind::PlayerHit { hit: hit(), hp_lost: 12, hp_after: 88 })]));
        assert!(!sora_guarded_in(&[ev(EventKind::PlayerDodged { hit: hit() })]));
        assert!(!sora_guarded_in(&[ev(EventKind::PlayerGuard { hit: hit(), victim: ReactionResult::Damage, attacker: ReactionResult::NoReaction })]));
        assert!(!sora_guarded_in(&[]));
    }

    const DT: f32 = 1.0 / 60.0;

    /// Sora with a real status (the level-99 loadout sheet with its abilities, Block among them: Sora::new's placeholder takes
    /// no damage).
    fn real_sora() -> kh_core::Sora {
        let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
        let mut loadout = kh_core::Loadout::new("sora-loadout-99");
        for row in json["rows"].as_array().unwrap() {
            use kh_core::status::LoadoutValue as V;
            let key = row[0].as_str().unwrap();
            let value = match &row[1] {
                serde_json::Value::Number(n) => V::Number(n.as_f64().unwrap()),
                serde_json::Value::String(t) => V::Text(t.clone()),
                serde_json::Value::Array(items) => V::List(items.iter().filter_map(|i| i.as_str().map(str::to_string)).collect()),
                serde_json::Value::Object(map) => V::Map(map.iter().filter_map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect()),
                _ => continue,
            };
            loadout.set(key, &value).unwrap_or_else(|e| panic!("{e}"));
        }
        loadout.resolve().unwrap();
        let origin = Vec3::new(0.0, 0.0, kh_core::Sora::origin_height_on_floor(&kh_core::SORA_PARAMS));
        kh_core::Sora::new(kh_core::SORA_PARAMS, kh_core::Pose { position: origin, yaw: 0.0 }).with_tuning(kh_core::Tuning::GAME_EXACT).with_status(kh_core::Status::from_loadout(&loadout))
    }

    /// An Elden Ring blow as sora.rs `damage_in` hands it: from the front (he faces +X), with its direction.
    fn front_blow(at: Vec3) -> HitOnPlayer {
        let from = at + Vec3::X * 200.0;
        HitOnPlayer { direction: Vec3::NEG_X, causer_yaw: 180.0, ..HitOnPlayer::new(kh_core::enums::ETresDamageKind::Small, 30.0, kh_core::TargetId(1), from) }
    }

    /// One blow on Sora (guarding or not) through his real core, then the host's route and his blood, as sora.rs runs them.
    fn blow_on_sora(guarding: bool) -> (Vec<SoundPlay>, Vec<Event>, i32, i32, crate::blood::Blood) {
        use kh_core::{Input, PadButton, PadButtons, PadState};
        let world = kh_core::test_world::TestWorld::floor();
        let mut sora = real_sora();
        let (hp, max) = (sora.status().hp, sora.status().max_hp());
        let rules = crate::blood::BloodRules::DEFAULT;
        let mut blood = crate::blood::Blood::default();
        blood.step(&rules, &crate::blood::BloodInputs { hp, max_hp: max, ..Default::default() }, 0.0);
        for _ in 0..5 {
            sora.step(DT, &Input::default(), &world);
        }
        if guarding {
            sora.step(DT, &Input::default().with_pad(PadState::from_held(PadButtons::NONE.with(PadButton::GUARD), PadButtons::NONE)), &world);
            for _ in 0..10 {
                sora.step(DT, &Input::default(), &world);
            }
            assert_eq!(sora.output().state, kh_core::StateId::Guard, "his guard is up");
        }
        let at = sora.output().position;
        sora.take_hit(front_blow(at));
        let out = sora.step(DT, &Input::default(), &world);
        let mut g = GuardBlows::default();
        let guarded = sora_guarded_in(&out.events);
        if guarded {
            g.guarded(PartyChara::Sora, out.capsule_center, 0.0);
        }
        blood.step(&rules, &crate::blood::BloodInputs { hp: sora.status().hp, max_hp: max, guarded, ..Default::default() }, DT);
        (g.take_sounds(), out.events.clone(), hp, sora.status().hp, blood)
    }

    /// (THE USER 2026-11-02: "...make sure an enemy hitting them while they are guarding plays the 'guard' sound effect instead
    /// of hit and they don't get bloody from it") SORA, THROUGH HIS REAL CORE: a blow on his guard is his guard sound only (no
    /// hit: no HP lost, no Damage state, whose clip carries his hurt voice) and no blood; the same blow unguarded is a hit as
    /// before (HP lost, the Damage reaction, blood) and no guard sound.
    #[test]
    fn a_blow_on_soras_guard_is_his_guard_sound_and_no_blood_unguarded_as_before() {
        let (sounds, events, before, after, blood) = blow_on_sora(true);
        assert_eq!(sounds.iter().map(|s| s.name).collect::<Vec<_>>(), vec![COMMON_GUARD_SE], "{events:?}");
        assert_eq!(after, before, "no HP lost");
        assert!(!events.iter().any(|e| matches!(e.kind, EventKind::PlayerHit { .. })), "no hit: {events:?}");
        assert_eq!((blood.body, blood.blade), (0.0, 0.0), "no blood");
        let (sounds, events, before, after, blood) = blow_on_sora(false);
        assert!(sounds.is_empty(), "no guard sound for a hit");
        assert!(after < before && events.iter().any(|e| matches!(e.kind, EventKind::PlayerHit { hp_lost, .. } if hp_lost > 0)), "{events:?}");
        assert!(blood.body > 0.0, "a landed blow bloodies him: {blood:?}");
    }

    /// GOOFY (A PARTY PAWN), THROUGH THE REAL KIT: a blow on his raised shield is `guarded` (his kit's own clang: the row says
    /// `kit`, the host adds no second sound), no HP lost, no hurt reaction, no blood; unguarded it lands as before.
    #[cfg(feature = "party")]
    #[test]
    fn a_blow_on_goofys_shield_is_guarded_and_bleeds_nothing_unguarded_as_before() {
        use kh_core::host::{AttackInfo, HitAttacker, HitInfo};
        use kh_party::NpcStateId;
        let blow = |guarding: bool| {
            let mut rt = kh_partyhost::PartyRuntime::new(kh_partyhost::RuntimeConfig { wanted: true, test: kh_partyhost::PartyTest::from_rows(&[]), weapons: Vec::new(), ai_pack: None, ai_pack_source: None });
            rt.spawn_one(PartyChara::Goofy, Vec3::ZERO, 0.0);
            let id = rt.id(PartyChara::Goofy).expect("Goofy");
            if guarding {
                rt.party.member_mut(id).expect("his pawn").enter(NpcStateId::Pad(kh_party::pad::goofy::GUARD));
            }
            let v = rt.view(PartyChara::Goofy).expect("his view");
            let from = v.position + Vec3::X * 150.0;
            let record = kh_core::HitRecord {
                position: v.position + Vec3::Z * 90.0,
                direction: Vec3::NEG_X,
                damage: 20.0,
                damage_kind: kh_core::enums::ETresDamageKind::Small,
                attack_name: "er.body",
                attacker: HitAttacker { position: from, yaw: 180.0, radius: 50.0, valid: true },
                info: HitInfo { victim_result: ReactionResult::Damage, ..HitInfo::DEFAULT },
                attack_info: AttackInfo { attacker: Some(kh_core::TargetId(1)), ..AttackInfo::NONE },
                ..kh_core::HitRecord::empty(kh_core::TargetId::NONE)
            };
            let rules = crate::blood::BloodRules::DEFAULT;
            let mut blood = crate::blood::Blood::default();
            blood.step(&rules, &crate::blood::BloodInputs { hp: v.hp, max_hp: v.hp_max, ..Default::default() }, 0.0);
            let r = rt.receive_hit(PartyChara::Goofy, &record).expect("in the party");
            let mut g = GuardBlows::default();
            let sound = r.guarded.then(|| g.guarded(PartyChara::Goofy, record.position, 0.0));
            let after = rt.party.view(id).expect("his view");
            blood.step(&rules, &crate::blood::BloodInputs { hp: after.hp, max_hp: after.hp_max, guarded: r.guarded, ..Default::default() }, DT);
            (r, sound, g.take_sounds(), v.hp, after, blood)
        };
        let (r, sound, sounds, before, after, blood) = blow(true);
        assert!(r.guarded && r.accepted, "{r:?}");
        assert_eq!((sound, sounds.len()), (Some(GuardSound::Kit), 0), "his kit's clang, no second sound");
        assert_eq!(after.hp, before, "no HP lost");
        assert_ne!(after.state, NpcStateId::Damage, "no hurt reaction (no hurt voice)");
        assert_eq!((blood.body, blood.blade), (0.0, 0.0), "no blood");
        let (r, sound, _, before, after, blood) = blow(false);
        assert!(!r.guarded && sound.is_none(), "{r:?}");
        assert!(after.hp < before && blood.body > 0.0, "the blow lands as before: {} -> {}, {blood:?}", before, after.hp);
    }

    /// THE RESTORE: the switch we set to 0 is given back at once when the hold ends, when the key turns it off mid-hold,
    /// when the host stops or the player dies (`give_back`), once only; a 0 the game set itself is never claimed or touched.
    #[test]
    fn the_hidden_bodys_sound_switch_is_given_back_at_once_and_only_when_we_set_it() {
        // the hold, the game switching it on again inside it, the hold's end
        let mut switch = true;
        let mut held = false;
        let phase = |held: &mut bool, switch: &mut bool, quiet: bool| {
            if let Some(v) = hold_step(held, *switch, quiet) {
                *switch = v;
            }
        };
        phase(&mut held, &mut switch, true);
        assert!(!switch && held, "held off by us");
        switch = true; // the game's distance rule
        phase(&mut held, &mut switch, true);
        assert!(!switch && held, "off again, still ours");
        phase(&mut held, &mut switch, false);
        assert!(switch && !held, "the hold's end (or guard_sound.quiet = 0 mid-hold): given back at once");
        phase(&mut held, &mut switch, false);
        assert!(switch && !held, "nothing more");
        // the host stops / the player dies mid-hold: given back once
        phase(&mut held, &mut switch, true);
        assert!(!switch && held);
        assert_eq!(give_back(&mut held), Some(true));
        assert_eq!(give_back(&mut held), None, "once only");
        // the game's own 0 when the hold starts: not ours, never touched
        let (mut switch, mut held) = (false, false);
        phase(&mut held, &mut switch, true);
        assert!(!switch && !held, "not claimed");
        assert_eq!(give_back(&mut held), None, "not given back by us");
        phase(&mut held, &mut switch, false);
        assert!(!switch, "left as the game set it");
    }

    /// The leader's hidden body is silent while his guard is up and for `quiet_s` after a guarded blow, for him only.
    #[test]
    fn the_hidden_body_is_silent_while_the_guard_takes_a_blow() {
        let mut g = GuardBlows::default();
        assert!(!g.quiet(PartyChara::Sora, false, 0.0));
        assert!(g.quiet(PartyChara::Sora, true, 0.0), "his guard is up");
        g.guarded(PartyChara::Goofy, Vec3::ZERO, 1.0);
        assert!(g.quiet(PartyChara::Goofy, false, 1.5));
        assert!(!g.quiet(PartyChara::Sora, false, 1.5), "another's guard");
        assert!(!g.quiet(PartyChara::Goofy, false, 1.0 + g.rules.quiet_s as f64 + 0.01), "given back");
    }
}
