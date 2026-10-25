//! (savepoint) KH3'S SAVE POINT as a host places it (docs/sora-savepoint-spec.md): the actor
//! `g_ex_SavePoint` (`ATresSavePointActor`) is five particle systems, two capsules and a rule;
//! the data is `PACK/savepoint/savepoint.json` (tools/sora_savepoint_pack.js).
//!
//! - DRAWN: the base and body effects at the actor root while it is active, as free effects of the
//!   host's effect side; the heal effect while Sora recovers there (and for 3 s at his first visit).
//! - RECOVER (`UTresSavePointRecoverComponent` tick 0x140F93590): while he overlaps the recover
//!   capsule and needs it, +180 points per second of HP, MP and focus, in whole points, through
//!   the core's `Recover(n, n, n, kind 1)` (kind 1: while the MP gauge recharges, the recharge timer
//!   loses n seconds instead).
//! - THE SAVE COMMAND: offered while he is inside the reactor capsule ([`SavePointStep::in_reactor`]);
//!   the host shows the prompt and opens the save menu (`kh_host::hud::Hud::open_savepoint`). The
//!   POUCH is refilled when that menu closes: the game has no pouch call of the save point; its
//!   refill is the one of every unpause (`Sora::on_unpause`, magic spec J 7.5).
//!
//! - THE SOUND (IMPL_ER19; the actor itself names no sound asset): the heal burst's particle system carries one
//!   (`ParticleSeAssets` of vg_com_svp_in00 -> se00000_016, 4.5 s, 3D; audio spec 5.5: a sound a particle system
//!   carries starts with the effect, at it). The effects here are FREE effects, drawn outside the core's effect seam
//!   where such sounds are started, so the save point asks for it itself each time the heal burst STARTS: a seam sound
//!   named "effect:<the heal effect>" at the actor root (`kh_audio::Routes::resolve`), through Sora's own event list
//!   like the orbs' sounds. The base / body effects carry none. NOT READ: the pawn's own recover effect of cure group
//!   SAVEPOINT (spec 7.2) and whether it has a sound of its own.
//!
//! INFERRED (not read): that the capsules' `Size.Z` is their full height, that "overlap" is his
//! capsule against the shape (the component is a `TresPawnOverlapComponentBase`), and how high the
//! actor root sits above the floor in a level (the blueprint's root is a default capsule component).
//!
//! (IMPL_ER52) WHERE THE EFFECTS SIT: NOT AT THE ROOT. VERIFIED AT INSTRUCTION LEVEL (0x140F9AA40, the actor's
//! `BeginPlay` after `AActor::BeginPlay` 0x143EA6430): a line trace (0x143FD41F0, channel 2) from the root's world
//! position straight down 140 cm (the constant at 0x145D785B0); on a blocking hit that did not start inside something
//! (`bBlockingHit` and not `bStartPenetrating`) the effect offset `+0x4E4` = the hit's location - the root, else
//! (0, 0, -90) (0x145C9C5A4). Base, body and heal effects are all spawned attached to the root AT THAT OFFSET
//! (0x140F8DCF0 / 0x140F8E280 / 0x140F8E910 pass `+0x4E4` as the relative location). So in KH3 the glow lies on the
//! floor under the root and the column of light rises from it ([`ground_offset`], [`SavePoint::effect_offset`]).
//! The capsules stay where the root puts them. A host without a trace leaves the offset at zero (the constructor's
//! value: the effects at the root, as before IMPL_ER52).

use std::path::Path;

use glam::Vec3;
use kh_core::{Attach, DebugEmit, SoundId, SoundPlay, Sora, StatusRequest};

use crate::prize::FreeEffects;

/// (IMPL_ER52) KH3's ground trace from the root (cm, straight down): 0x140F9AA40, constant at 0x145D785B0.
pub const GROUND_TRACE: f32 = 140.0;
/// (IMPL_ER52) The effect offset when that trace hits nothing (cm in Z): 0x140F9AA40, constant at 0x145C9C5A4.
pub const GROUND_MISS: f32 = -90.0;

/// (IMPL_ER52) KH3'S EFFECT OFFSET (`ATresSavePointActor + 0x4E4`, set in BeginPlay 0x140F9AA40): `hit` = where the
/// line from the root straight down met the floor (None: nothing within the trace, or the trace started inside
/// something); `miss` = the offset without one (KH3: [`GROUND_MISS`]).
pub fn ground_offset(root: Vec3, hit: Option<Vec3>, miss: f32) -> Vec3 {
    match hit {
        Some(at) if at.is_finite() => at - root,
        _ => Vec3::new(0.0, 0.0, miss),
    }
}

/// The actor's data (the pack's `savepoint/savepoint.json`; the defaults are the blueprint's values).
#[derive(Clone, Debug, PartialEq)]
pub struct SavePointData {
    pub base: String,
    pub body: String,
    pub limit_base: String,
    pub limit_body: String,
    pub heal: String,
    /// (radius, height, offset from the root).
    pub reactor: (f32, f32, Vec3),
    pub recover: (f32, f32, Vec3),
    /// Points per second of each gauge.
    pub recover_rate: f32,
    pub first_access_time: f32,
}

impl Default for SavePointData {
    fn default() -> SavePointData {
        SavePointData {
            base: "vg_com_svp_nml00".into(),
            body: "vg_com_svp_nml01".into(),
            limit_base: "vg_com_svp_lmt00".into(),
            limit_body: "vg_com_svp_lmt01".into(),
            heal: "vg_com_svp_in00".into(),
            reactor: (110.0, 180.0, Vec3::new(0.0, 0.0, -40.0)),
            recover: (50.0, 140.0, Vec3::new(0.0, 0.0, -40.0)),
            recover_rate: 180.0,
            first_access_time: 3.0,
        }
    }
}

impl SavePointData {
    /// From the pack; the defaults when the file is missing or a field is not there.
    pub fn load(pack: &Path) -> SavePointData {
        let mut data = SavePointData::default();
        let Ok(text) = std::fs::read_to_string(pack.join("savepoint").join("savepoint.json")) else { return data };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else { return data };
        let name = |role: &str, into: &mut String| {
            if let Some(s) = json["effects"][role].as_str() {
                *into = s.to_string();
            }
        };
        name("base", &mut data.base);
        name("body", &mut data.body);
        name("limit_base", &mut data.limit_base);
        name("limit_body", &mut data.limit_body);
        name("heal", &mut data.heal);
        let capsule = |key: &str, into: &mut (f32, f32, Vec3)| {
            let c = &json[key];
            if let (Some(r), Some(h), Some(o)) = (c["radius"].as_f64(), c["height"].as_f64(), c["offset"].as_array()) {
                let at = |k: usize| o.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                *into = (r as f32, h as f32, Vec3::new(at(0), at(1), at(2)));
            }
        };
        capsule("reactor", &mut data.reactor);
        capsule("recover", &mut data.recover);
        if let Some(v) = json["recover_rate"]["value"].as_f64() {
            data.recover_rate = v as f32;
        }
        if let Some(v) = json["first_access_time"]["value"].as_f64() {
            data.first_access_time = v as f32;
        }
        data
    }
}

/// What a step found.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SavePointStep {
    /// He is inside the reactor capsule: the Save command is offered.
    pub in_reactor: bool,
    /// He is inside the recover capsule.
    pub in_recover: bool,
    /// Points given this step (each of HP, MP, focus).
    pub recovered: i32,
    /// The heal effect is on.
    pub healing: bool,
    /// The heal effect went on in this step: its sound was asked for (when the save point sounds).
    pub heal_started: bool,
}

/// (IMPL_ER19) When the save point's sound is asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SavePointSound {
    /// Never (a host's off switch).
    Off,
    /// KH3: with the heal burst, each time it starts (he steps on it needing recovery; the first visit).
    WithHeal,
    /// A HOST CHOICE, not KH3: also every time he steps into the recover capsule with nothing to recover.
    EveryEntry,
}

/// (SP2, the user 2026-10-20: "save points should always play a sound effect when you enter them", "The save point should
/// also look like that when you first step in it") THE ENTRY BURST, a host choice on KH3's own parts: KH3's save point is
/// five particle systems and no other asset (docs/sora-savepoint-spec.md 1: no mesh, no animation, no sound asset of the
/// actor): its "idle" is base + body (`vg_com_svp_nml00` / `nml01`), its "enter" is the heal burst `vg_com_svp_in00` (the
/// column of light, the rings, the sparkles) with the sound that particle system carries (`se00000_016`), and its "stay"
/// is the burst while he recovers, base + body otherwise. KH3 plays the enter part only at the first visit of a save point
/// not accessed yet or when he needs recovery (0x140F9CA00); with this switch EVERY ENTRY into the save point's area (the
/// reactor for a host that sets `first_in_reactor`, else the recover capsule) plays KH3's first-visit burst again: the
/// burst for `first_access_time` (3 s) and its sound, also after loads and warps (a save point made anew around him counts
/// his first step as an entry). Leaving ends it as KH3 ends its burst (Deactivate: what is in the air runs out).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EntryBurst {
    pub on: bool,
    /// Seconds of the burst left on this visit.
    left: f32,
    /// He was inside the area at the last step.
    inside: bool,
    /// Entries counted since the save point was made.
    pub entries: u32,
}

impl EntryBurst {
    /// One step: `inside` = his capsule overlaps the area; `time` = the burst's length. True = he entered in this step.
    pub fn step(&mut self, inside: bool, time: f32, dt: f32) -> bool {
        let entered = self.on && inside && !self.inside;
        self.inside = inside;
        if entered {
            self.left = time;
            self.entries += 1;
        } else if inside {
            self.left = (self.left - dt).max(0.0);
        } else {
            self.left = 0.0;
        }
        entered
    }

    /// (LDI) HE IS ALREADY HERE (a load or a warp put him here, the screen black): his overlap now is taken as it stands,
    /// with no entry: the next step inside plays no burst and no sound; leaving and coming back is an entry as always.
    pub fn settle(&mut self, inside: bool) {
        (self.inside, self.left) = (inside, 0.0);
    }

    /// The burst is on (he stands in the area within its time).
    pub fn burning(&self) -> bool {
        self.on && self.inside && self.left > 0.0
    }

    /// The save point stopped acting on him (hidden, refused): the next step inside is an entry again.
    pub fn reset(&mut self) {
        (self.inside, self.left) = (false, 0.0);
    }
}

/// The seam name of the sound an effect carries ("effect:<name>"); the seam wants a `'static` name: each
/// distinct effect name is kept once for the life of the process.
fn effect_sound_name(effect: &str) -> &'static str {
    use std::collections::HashMap;
    use std::sync::Mutex;
    static NAMES: Mutex<Option<HashMap<String, &'static str>>> = Mutex::new(None);
    if effect == "vg_com_svp_in00" {
        return "effect:vg_com_svp_in00";
    }
    let Ok(mut names) = NAMES.lock() else { return "effect:vg_com_svp_in00" };
    let names = names.get_or_insert_with(HashMap::new);
    if let Some(name) = names.get(effect) {
        return name;
    }
    let name: &'static str = Box::leak(format!("effect:{effect}").into_boxed_str());
    names.insert(effect.to_string(), name);
    name
}

pub struct SavePoint {
    pub data: SavePointData,
    /// The actor root, core cm.
    pub root: Vec3,
    /// `m_bLimitedSavePoint`: the "limited" effects (a save point that only heals / saves in part).
    pub limited: bool,
    /// `m_bIsActive`.
    pub active: bool,
    accumulator: f32,
    first_timer: f32,
    was_inside: bool,
    healing: bool,
    effects: [Option<u64>; 3],
    shown_active: bool,
    /// Points given since it was made (each gauge).
    pub recovered_total: i64,
    /// (IMPL_ER19) When its sound is asked for (default: KH3's, with the heal burst).
    pub sound: SavePointSound,
    /// Times the sound was asked for since it was made.
    pub sounded: u32,
    /// (er-host, IMPL_ER26) The first-visit burst plays as soon as he is in the REACTOR (the Save command's capsule), not
    /// only in the recover capsule: a host that unlocks its save point when he runs into the reactor shows it there.
    pub first_in_reactor: bool,
    /// (IMPL_ER52) KH3's `+0x4E4`: where the three effects (and the heal burst's sound) are, relative to the root. Zero
    /// until the host sets it ([`ground_offset`]); a change is applied by the host taking the effects down and up again.
    pub effect_offset: Vec3,
    /// (SP2) Every entry plays KH3's burst and its sound ([`EntryBurst`]; off = KH3's own rule).
    pub entry: EntryBurst,
    /// (SP2) A host whose sounds must not wait for Sora's next step (the Elden Ring mod: Sora out of the party is not
    /// stepped, so a sound queued on him would never come): `Some` = the save point's sounds are put here for the host to
    /// play (`take_sounds`); `None` = through Sora's own events (the test app).
    pub host_sounds: Option<Vec<SoundPlay>>,
    /// (LDI) Seconds its idle effects (base, body) are warmed by at their next spawn (`FreeEffects::free_prewarm`): a save
    /// point made while the screen is black (a load), or made again where it stood (its floor found anew), is first drawn
    /// in its idle state instead of filling up from nothing. Taken by that spawn; 0 = KH3's own start.
    pub warm: f32,
}

/// Two upright capsules overlap: (centre, radius, half height incl. caps) each. Tested as cylinders
/// with the caps' height (a touch more generous at the rims than true capsules).
fn overlap(a: (Vec3, f32, f32), b: (Vec3, f32, f32)) -> bool {
    let d = a.0 - b.0;
    d.truncate().length() <= a.1 + b.1 && d.z.abs() <= a.2 + b.2
}

impl SavePoint {
    pub fn new(data: SavePointData, root: Vec3) -> SavePoint {
        let first_timer = data.first_access_time;
        SavePoint { data, root, limited: false, active: true, accumulator: 0.0, first_timer, was_inside: false, healing: false, effects: [None; 3], shown_active: false, recovered_total: 0, sound: SavePointSound::WithHeal, sounded: 0, first_in_reactor: false, effect_offset: Vec3::ZERO, entry: EntryBurst::default(), host_sounds: None /* (SP2) */, warm: 0.0 /* (LDI) */ }
    }

    /// (SP2) The sounds asked for since the last call (only with `host_sounds`).
    pub fn take_sounds(&mut self) -> Vec<SoundPlay> {
        self.host_sounds.as_mut().map(std::mem::take).unwrap_or_default()
    }

    /// (IMPL_ER19) THE SAVE POINT WAS ACCESSED BEFORE (the game keeps that per save point in its save data): no
    /// first-visit burst. A host whose save points come and go with distance (the Elden Ring mod makes one per site of
    /// grace in reach) calls this for those he has been at, or every return would be a "first visit".
    pub fn set_accessed(&mut self) {
        self.first_timer = 0.0;
    }

    /// The first-visit burst is still to come.
    pub fn first_visit_pending(&self) -> bool {
        self.first_timer > 0.0
    }

    /// What the pawn's "needs recovery" answers (0x141436260) for a Sora without a party.
    pub fn needs_recovery(sora: &Sora) -> bool {
        let s = sora.status();
        s.hp > 0 && (s.hp < s.max_hp() || s.mp < s.max_mp() || s.focus < s.focus_max)
    }

    /// ONE SIMULATION STEP, after Sora's. `capsule`: his capsule (centre, radius, half height).
    pub fn step(&mut self, dt: f32, sora: &mut Sora, capsule: (Vec3, f32, f32)) -> SavePointStep {
        let needs = Self::needs_recovery(sora);
        let step = self.step_checked(dt, sora, capsule, needs);
        if step.recovered > 0 {
            let n = step.recovered;
            sora.status_request(StatusRequest::Recover { hp: n, mp: n, focus: n, kind: 1 });
        }
        step
    }

    /// (party P2) THE STEP OF A HOST WITH A PARTY: the pawn in the capsules is `capsule`, `needs` is HIS "needs recovery"
    /// (player 0x141436260: he is not dying, and below max HP / MP / focus, OR A PARTY MEMBER below max HP or MP), and
    /// the host gives `Recover(n, n, n, kind 1)` itself when `recovered` = n > 0: to that pawn AND to each of his six
    /// party slots (tick 0x140F93590 calls vt+0x1878 on the pawn, then on every party member that is not pending
    /// kill). `sora` only carries the heal burst's sound. Nothing is given here.
    pub fn step_checked(&mut self, dt: f32, sora: &mut Sora, capsule: (Vec3, f32, f32), needs: bool) -> SavePointStep {
        if !self.active {
            (self.was_inside, self.healing, self.accumulator) = (false, false, 0.0);
            self.entry.reset(); // (SP2)
            return SavePointStep::default();
        }
        let was_healing = self.healing;
        let shape = |c: &(f32, f32, Vec3)| (self.root + c.2, c.0, c.1 * 0.5);
        let in_reactor = overlap(capsule, shape(&self.data.reactor));
        let in_recover = overlap(capsule, shape(&self.data.recover));
        let mut recovered = 0;
        let entered = in_recover && !self.was_inside;
        if in_recover != self.was_inside {
            // Overlap begin / end (0x140F8FCD0 / 0x140F93980): the accumulator starts over.
            self.accumulator = 0.0;
            self.was_inside = in_recover;
        }
        if in_recover && needs {
            self.accumulator += dt * self.data.recover_rate;
            if self.accumulator > 1.0 {
                let n = self.accumulator.floor() as i32;
                // (party P2) the Recover itself is the caller's (`step`: Sora's; a party host: everyone's)
                self.accumulator -= n as f32;
                recovered = n;
                self.recovered_total += n as i64;
            }
        }
        // The heal effect (actor tick 0x140F9CA00): while the checked pawn needs recovery, or for the
        // first-access time of his first visit.
        // (IMPL_ER26) `first_in_reactor`: the first visit counts from the reactor on
        let first = (in_recover || (self.first_in_reactor && in_reactor)) && self.first_timer > 0.0;
        if first {
            self.first_timer = (self.first_timer - dt).max(0.0);
        }
        // (SP2) every entry into the save point's area: KH3's first-visit burst again (`EntryBurst`)
        let area = if self.first_in_reactor { in_reactor } else { in_recover };
        let came_in = self.entry.step(area, self.data.first_access_time, dt);
        self.healing = (in_recover && needs) || first || self.entry.burning();
        // (IMPL_ER19) The heal burst starts: the sound its particle system carries, at the actor root (it is in his
        // next step's events, like every seam sound a host part asks for).
        let heal_started = self.healing && !was_healing;
        let sound = match self.sound {
            SavePointSound::Off => false,
            SavePointSound::WithHeal => heal_started,
            SavePointSound::EveryEntry => heal_started || (entered && !self.healing),
        } || (came_in && self.sound != SavePointSound::Off); // (SP2) every entry sounds, once
        if sound {
            self.sounded += 1;
            let play = SoundPlay { at: Attach::World(self.effects_at()), ..SoundPlay::at_sora(SoundId(u32::MAX), effect_sound_name(&self.data.heal)) };
            match self.host_sounds.as_mut() {
                Some(queue) => queue.push(play), // (SP2) the host plays it now, whoever leads
                None => sora.debug_emit(DebugEmit::Sound(play)),
            }
        }
        SavePointStep { in_reactor, in_recover, recovered, healing: self.healing, heal_started }
    }

    /// (LDI) HE ARRIVES HERE WITH THE LOAD (fast travel, Continue, a respawn: the save point made or shown around him while
    /// the screen was black): what his capsule overlaps now is taken as where he already stands. No entry burst, no entry
    /// sound, and a first visit inside it counts as made (he was put at it). KH3's heal while someone needs recovery is
    /// untouched (that is its own feedback, not an appearance). Returns: he stands in its area.
    pub fn arrive(&mut self, capsule: (Vec3, f32, f32)) -> bool {
        let shape = |c: &(f32, f32, Vec3)| (self.root + c.2, c.0, c.1 * 0.5);
        let in_reactor = overlap(capsule, shape(&self.data.reactor));
        let in_recover = overlap(capsule, shape(&self.data.recover));
        let area = if self.first_in_reactor { in_reactor } else { in_recover };
        // (a host may call it every step of its load-in window: an unchanged overlap keeps the recovery's accumulator)
        if self.was_inside != in_recover {
            (self.was_inside, self.accumulator) = (in_recover, 0.0);
        }
        self.entry.settle(area);
        if in_recover || (self.first_in_reactor && in_reactor) {
            self.first_timer = 0.0;
        }
        area || in_recover
    }

    /// (er-host, IMPL_ER21) A step in which the save point does not act on him (the host refuses its recovery now):
    /// nothing is given, the heal effect is off, the overlap starts over when it acts again.
    pub fn idle(&mut self) {
        (self.was_inside, self.healing, self.accumulator) = (false, false, 0.0);
        self.entry.reset(); // (SP2)
    }

    /// THE PICTURE: base and body while active, the heal effect while healing.
    pub fn show(&mut self, fx: &mut dyn FreeEffects) {
        if self.active != self.shown_active {
            self.shown_active = self.active;
            for slot in 0..2 {
                if let Some(id) = self.effects[slot].take() {
                    fx.free_stop(id, false);
                }
            }
            if self.active {
                let (base, body) = if self.limited { (&self.data.limit_base, &self.data.limit_body) } else { (&self.data.base, &self.data.body) };
                self.effects[0] = fx.free_spawn(base, self.effects_at(), 0.0, 1.0);
                self.effects[1] = fx.free_spawn(body, self.effects_at(), 0.0, 1.0);
                // (LDI) made while the screen was black / made again where it stood: drawn in its idle state at once
                let warm = std::mem::take(&mut self.warm);
                if warm > 0.0 {
                    for id in self.effects[..2].iter().flatten() {
                        fx.free_prewarm(*id, warm);
                    }
                }
            }
        }
        match (self.healing && self.active, self.effects[2]) {
            (true, None) => self.effects[2] = fx.free_spawn(&self.data.heal, self.effects_at(), 0.0, 1.0),
            (false, Some(id)) => {
                // `Deactivate`: it stops emitting, what is in the air runs out.
                fx.free_stop(id, false);
                self.effects[2] = None;
            }
            _ => {}
        }
    }

    /// (IMPL_ER52) Where the effects are spawned: the root + KH3's effect offset.
    pub fn effects_at(&self) -> Vec3 {
        self.root + self.effect_offset
    }

    /// (base, body, heal) are drawn.
    pub fn drawn(&self) -> [bool; 3] {
        self.effects.map(|e| e.is_some())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use kh_core::status::LoadoutValue;
    use kh_core::test_world::TestWorld;
    use kh_core::{Input, Loadout, Pose, Status, SORA_PARAMS};

    const DT: f32 = 1.0 / 60.0;

    pub(crate) fn sora99() -> Sora {
        let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let mut loadout = Loadout::new("sora-loadout-99");
        for row in json["rows"].as_array().unwrap() {
            let value = match &row[1] {
                serde_json::Value::Number(n) => n.as_f64().map(LoadoutValue::Number),
                serde_json::Value::String(s) => Some(LoadoutValue::Text(s.clone())),
                serde_json::Value::Array(items) => items.iter().map(|i| i.as_str().map(str::to_string)).collect::<Option<Vec<_>>>().map(LoadoutValue::List),
                serde_json::Value::Object(map) => map.iter().map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect::<Option<Vec<_>>>().map(LoadoutValue::Map),
                _ => None,
            };
            loadout.set(row[0].as_str().unwrap(), &value.unwrap()).unwrap();
        }
        loadout.resolve().unwrap();
        Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 }).with_status(Status::from_loadout(&loadout))
    }

    #[derive(Default)]
    struct Fake {
        next: u64,
        live: Vec<(u64, String)>,
    }

    impl FreeEffects for Fake {
        fn free_spawn(&mut self, name: &str, _position: Vec3, _yaw: f32, _scale: f32) -> Option<u64> {
            self.next += 1;
            self.live.push((self.next, name.to_string()));
            Some(self.next)
        }

        fn free_move(&mut self, _id: u64, _position: Vec3, _yaw: f32, _scale: f32) {}

        fn free_stop(&mut self, id: u64, _immediate: bool) {
            self.live.retain(|e| e.0 != id);
        }
    }

    /// Spec 3: standing in the recover capsule he gets 180 points a second of HP, MP and focus until
    /// all three are full; outside it nothing; the heal effect shows while he recovers; the Save
    /// command is offered in the wider reactor capsule.
    #[test]
    fn standing_on_it_recovers_180_points_a_second_until_full() {
        let world = TestWorld::floor();
        let mut sora = sora99();
        let (max_hp, max_mp) = (sora.status().max_hp(), sora.status().max_mp());
        (sora.status_mut().hp, sora.status_mut().mp, sora.status_mut().focus) = (20, 5, 0);
        let centre = sora.output().capsule_center;
        let capsule = (centre, SORA_PARAMS.capsule_radius, SORA_PARAMS.capsule_half_height);
        // The actor root 92 cm above the floor (a player start's capsule centre), at his feet.
        let mut point = SavePoint::new(SavePointData::default(), Vec3::new(0.0, 0.0, 92.0));
        let mut fx = Fake::default();
        point.show(&mut fx);
        assert_eq!(fx.live.iter().map(|e| e.1.as_str()).collect::<Vec<_>>(), ["vg_com_svp_nml00", "vg_com_svp_nml01"]);
        // Far away: nothing.
        let away = (centre + Vec3::X * 400.0, capsule.1, capsule.2);
        assert_eq!(point.step(DT, &mut sora, away), SavePointStep::default());
        // In the reactor but not in the recover capsule: the command, no healing.
        let near = (centre + Vec3::X * (capsule.1 + 80.0), capsule.1, capsule.2);
        let s = point.step(DT, &mut sora, near);
        assert!(s.in_reactor && !s.in_recover && s.recovered == 0 && !s.healing);
        // On it: one second gives 180 points to each gauge (clamped at the maxima).
        let mut given = 0;
        for _ in 0..60 {
            let s = point.step(DT, &mut sora, capsule);
            assert!(s.in_recover && s.healing);
            given += s.recovered;
            sora.step(DT, &Input::default(), &world);
            point.show(&mut fx);
        }
        assert!((179..=180).contains(&given), "{given}");
        assert_eq!(fx.live.len(), 3);
        assert_eq!(fx.live[2].1, "vg_com_svp_in00");
        let st = sora.status();
        assert_eq!((st.hp, st.mp, st.focus), ((20 + given).min(max_hp), (5 + given).min(max_mp), given.min(st.focus_max)));
        // Until everything is full; then it stops giving and (the first-access time being over) the heal effect ends.
        for _ in 0..240 {
            point.step(DT, &mut sora, capsule);
            sora.step(DT, &Input::default(), &world);
            point.show(&mut fx);
        }
        let st = sora.status();
        assert_eq!((st.hp, st.mp, st.focus), (max_hp, max_mp, st.focus_max));
        let before = point.recovered_total;
        let s = point.step(DT, &mut sora, capsule);
        assert!(s.in_recover && s.recovered == 0 && !s.healing && point.recovered_total == before);
        point.show(&mut fx);
        assert_eq!(fx.live.len(), 2);
    }

    /// The seam sounds of a step of Sora's (the save point's sound is queued on him and comes out in his next step).
    fn sounds_of_step(sora: &mut Sora, world: &TestWorld) -> Vec<(String, Attach)> {
        let out = sora.step(DT, &Input::default(), world);
        out.events.iter().filter_map(|e| if let kh_core::EventKind::SoundPlay { play, .. } = &e.kind { Some((play.name.to_string(), play.at)) } else { None }).collect()
    }

    /// (IMPL_ER19) "no SFX when you enter the save point": each time the heal burst starts, the sound its particle
    /// system carries is asked for ONCE, at the actor root, through Sora's own events; not again while it stays on;
    /// again at the next start; never with the off switch; a save point he was at before has no first-visit burst
    /// (and so no sound when he needs nothing), unless the host asks for a sound at every entry.
    #[test]
    fn the_heal_burst_asks_for_its_sound_each_time_it_starts() {
        let world = TestWorld::floor();
        let mut sora = sora99();
        let centre = sora.output().capsule_center;
        let capsule = (centre, SORA_PARAMS.capsule_radius, SORA_PARAMS.capsule_half_height);
        let away = (centre + Vec3::X * 400.0, capsule.1, capsule.2);
        let root = Vec3::new(0.0, 0.0, 92.0);
        let mut point = SavePoint::new(SavePointData::default(), root);
        // The first visit with full gauges: the burst starts -> one sound, at the root.
        let s = point.step(DT, &mut sora, capsule);
        assert!(s.healing && s.heal_started);
        assert_eq!(sounds_of_step(&mut sora, &world), vec![("effect:vg_com_svp_in00".to_string(), Attach::World(root))]);
        for _ in 0..240 {
            assert!(!point.step(DT, &mut sora, capsule).heal_started);
            assert!(sounds_of_step(&mut sora, &world).is_empty());
        }
        assert_eq!(point.sounded, 1);
        // Away and back, full: the first visit is over: nothing.
        point.step(DT, &mut sora, away);
        assert!(!point.step(DT, &mut sora, capsule).healing);
        assert!(sounds_of_step(&mut sora, &world).is_empty());
        // Hurt while standing on it: the burst starts again -> the sound again.
        sora.status_mut().hp -= 30;
        let s = point.step(DT, &mut sora, capsule);
        assert!(s.healing && s.heal_started);
        assert_eq!(sounds_of_step(&mut sora, &world).len(), 1);
        assert_eq!(point.sounded, 2);
        // The off switch.
        let mut silent = SavePoint::new(SavePointData::default(), root);
        silent.sound = SavePointSound::Off;
        assert!(silent.step(DT, &mut sora, capsule).heal_started);
        assert_eq!(silent.sounded, 0);
        // A save point he was at before: no first-visit burst; full gauges: nothing; with the host's
        // "every entry" choice: a sound when he steps in.
        for _ in 0..120 {
            point.step(DT, &mut sora, capsule);
            sounds_of_step(&mut sora, &world);
        }
        assert!(!SavePoint::needs_recovery(&sora));
        let mut known = SavePoint::new(SavePointData::default(), root);
        assert!(known.first_visit_pending());
        known.set_accessed();
        assert!(!known.first_visit_pending());
        let s = known.step(DT, &mut sora, capsule);
        assert!(s.in_recover && !s.healing && !s.heal_started && known.sounded == 0);
        known.step(DT, &mut sora, away);
        known.sound = SavePointSound::EveryEntry;
        known.step(DT, &mut sora, capsule);
        known.step(DT, &mut sora, capsule);
        assert_eq!(known.sounded, 1);
    }

    /// (SP2) "save points should always play a sound effect when you enter them" / "The save point should also look like
    /// that when you first step in it": with the entry burst on, EVERY entry (accessed save point, full gauges) plays KH3's
    /// burst (in00 for 3 s) and its sound once; again at the next entry; a save point made anew around him (a load, a warp)
    /// counts his first step in it as an entry; the reactor is the area for a host that sets `first_in_reactor`; the off
    /// switch keeps it silent; a host queue gets the sound instead of Sora's events.
    #[test]
    fn every_entry_plays_the_burst_and_its_sound_once() {
        let world = TestWorld::floor();
        let mut sora = sora99();
        let centre = sora.output().capsule_center;
        let capsule = (centre, SORA_PARAMS.capsule_radius, SORA_PARAMS.capsule_half_height);
        let away = (centre + Vec3::X * 400.0, capsule.1, capsule.2);
        let root = Vec3::new(0.0, 0.0, 92.0);
        assert!(!SavePoint::needs_recovery(&sora));
        let mut point = SavePoint::new(SavePointData::default(), root);
        point.set_accessed();
        point.entry.on = true;
        let mut fx = Fake::default();
        for visit in 1..=3u32 {
            // out of it: nothing
            for _ in 0..30 {
                assert!(!point.step(DT, &mut sora, away).healing);
                sounds_of_step(&mut sora, &world);
            }
            // he steps in: the burst starts and its sound is asked for at the save point, once
            let s = point.step(DT, &mut sora, capsule);
            assert!(s.healing && s.heal_started, "visit {visit}");
            assert_eq!(sounds_of_step(&mut sora, &world), vec![("effect:vg_com_svp_in00".to_string(), Attach::World(root))]);
            point.show(&mut fx);
            assert!(fx.live.iter().any(|e| e.1 == "vg_com_svp_in00"), "the column of light");
            // it stays 3 s while he stands there (KH3's first-access time), silent; then the idle effects only
            let frames = 1 + (0..400).filter(|_| { let on = point.step(DT, &mut sora, capsule).healing; assert!(sounds_of_step(&mut sora, &world).is_empty()); on }).count();
            assert!((179..=181).contains(&frames), "{frames}");
            point.show(&mut fx);
            assert!(!fx.live.iter().any(|e| e.1 == "vg_com_svp_in00"));
            assert_eq!((point.sounded, point.entry.entries), (visit, visit));
        }
        // leaving within the burst ends it; coming back is a new entry with a new sound
        point.step(DT, &mut sora, away);
        point.step(DT, &mut sora, capsule);
        assert!(point.step(DT, &mut sora, capsule).healing);
        assert!(!point.step(DT, &mut sora, away).healing);
        assert!(point.step(DT, &mut sora, capsule).heal_started);
        assert_eq!(point.sounded, 5);
        // AFTER A LOAD / WARP: a save point made anew while he stands in it: his first step is an entry
        let mut fresh = SavePoint::new(SavePointData::default(), root);
        fresh.set_accessed();
        fresh.entry.on = true;
        assert!(fresh.step(DT, &mut sora, capsule).heal_started);
        assert_eq!(fresh.sounded, 1);
        // hidden / refused (the host's idle: combat) and back while he stands there: an entry again
        fresh.idle();
        assert!(fresh.step(DT, &mut sora, capsule).heal_started);
        assert_eq!(fresh.sounded, 2);
        // the reactor is the area when the host says so (120 cm beside the root: in the reactor, not the recover capsule)
        let mut reactor = SavePoint::new(SavePointData::default(), Vec3::new(120.0, 0.0, 92.0));
        (reactor.first_in_reactor, reactor.entry.on) = (true, true);
        reactor.set_accessed();
        let s = reactor.step(DT, &mut sora, capsule);
        assert!(s.in_reactor && !s.in_recover && s.heal_started && reactor.sounded == 1);
        // the off switch: the burst, no sound
        let mut silent = SavePoint::new(SavePointData::default(), root);
        (silent.sound, silent.entry.on) = (SavePointSound::Off, true);
        silent.set_accessed();
        assert!(silent.step(DT, &mut sora, capsule).healing);
        assert_eq!(silent.sounded, 0);
        // KH3's own rule without the switch: an accessed save point with full gauges is silent and dark
        let mut kh3 = SavePoint::new(SavePointData::default(), root);
        kh3.set_accessed();
        assert!(!kh3.step(DT, &mut sora, capsule).healing && kh3.sounded == 0);
        // THE HOST QUEUE (Sora out of the party is not stepped): the sound goes there, not into his events
        let mut hosted = SavePoint::new(SavePointData::default(), root);
        (hosted.host_sounds, hosted.entry.on) = (Some(Vec::new()), true);
        hosted.set_accessed();
        sounds_of_step(&mut sora, &world); // (what the save points above queued on him)
        hosted.step(DT, &mut sora, capsule);
        assert!(sounds_of_step(&mut sora, &world).is_empty());
        let queued = hosted.take_sounds();
        assert_eq!(queued.len(), 1);
        assert_eq!((queued[0].name, queued[0].at), ("effect:vg_com_svp_in00", Attach::World(root)));
        assert!(hosted.take_sounds().is_empty());
    }

    /// (LDI, the user 2026-10-22: save points should "load instantly when the game loads") A SAVE POINT HE ARRIVES AT WITH A
    /// LOAD (made around him while the screen is black): no entry burst, no entry sound, no first-visit burst; stepping out
    /// and in again is an entry as always; someone needing recovery still gets KH3's heal; the idle effects of a warmed
    /// spawn are warmed once (the next spawn starts from nothing again).
    #[test]
    fn a_save_point_he_arrives_at_with_a_load_shows_no_entry_burst() {
        #[derive(Default)]
        struct Warm {
            inner: Fake,
            warmed: Vec<(u64, f32)>,
        }
        impl FreeEffects for Warm {
            fn free_spawn(&mut self, name: &str, position: Vec3, yaw: f32, scale: f32) -> Option<u64> {
                self.inner.free_spawn(name, position, yaw, scale)
            }
            fn free_move(&mut self, _id: u64, _position: Vec3, _yaw: f32, _scale: f32) {}
            fn free_stop(&mut self, id: u64, immediate: bool) {
                self.inner.free_stop(id, immediate)
            }
            fn free_prewarm(&mut self, id: u64, seconds: f32) {
                self.warmed.push((id, seconds));
            }
        }
        let world = TestWorld::floor();
        let mut sora = sora99();
        let centre = sora.output().capsule_center;
        let capsule = (centre, SORA_PARAMS.capsule_radius, SORA_PARAMS.capsule_half_height);
        let away = (centre + Vec3::X * 400.0, capsule.1, capsule.2);
        let root = Vec3::new(0.0, 0.0, 92.0);
        assert!(!SavePoint::needs_recovery(&sora));
        for first_in_reactor in [false, true] {
            // fast travel to it: made anew around him, entry burst on (SP2), a first visit still pending even
            let mut point = SavePoint::new(SavePointData::default(), root);
            (point.entry.on, point.first_in_reactor, point.host_sounds) = (true, first_in_reactor, Some(Vec::new()));
            point.warm = 3.0;
            let mut fx = Warm::default();
            point.show(&mut fx);
            assert_eq!(fx.warmed.len(), 2, "base and body warmed");
            assert!(fx.warmed.iter().all(|w| w.1 == 3.0) && point.warm == 0.0);
            assert!(point.arrive(capsule));
            for _ in 0..240 {
                let s = point.step(DT, &mut sora, capsule);
                assert!(!s.healing && !s.heal_started, "no burst while he stands where the load put him");
                point.show(&mut fx);
            }
            assert!(point.take_sounds().is_empty() && point.sounded == 0 && !fx.inner.live.iter().any(|e| e.1 == "vg_com_svp_in00"));
            // out and in again: KH3's entry, with its sound
            point.step(DT, &mut sora, away);
            assert!(point.step(DT, &mut sora, capsule).heal_started);
            assert_eq!(point.take_sounds().len(), 1);
            // hurt when he arrives: the heal (KH3's recover feedback) is not an entry burst and stays
            let mut hurt = SavePoint::new(SavePointData::default(), root);
            (hurt.entry.on, hurt.first_in_reactor) = (true, first_in_reactor);
            hurt.arrive(capsule);
            sora.status_mut().hp -= 30;
            assert!(hurt.step(DT, &mut sora, capsule).heal_started);
            for _ in 0..120 {
                hurt.step(DT, &mut sora, capsule);
                sounds_of_step(&mut sora, &world);
            }
            assert!(!SavePoint::needs_recovery(&sora));
            // a later spawn (hidden in combat and back) starts from nothing: the warm was for the load's spawn only
            point.active = false;
            point.show(&mut fx);
            point.active = true;
            point.show(&mut fx);
            assert_eq!(fx.warmed.len(), 2);
        }
        // arriving beside it (outside its area): nothing taken; walking in later is an entry
        let mut beside = SavePoint::new(SavePointData::default(), root + Vec3::X * 600.0);
        beside.entry.on = true;
        beside.set_accessed();
        assert!(!beside.arrive(capsule));
        assert!(!beside.step(DT, &mut sora, capsule).healing);
        assert!(beside.step(DT, &mut sora, (root + Vec3::new(600.0, 0.0, 0.0), capsule.1, capsule.2)).heal_started);
    }

    /// Spec 3: the first visit shows the heal effect for 3 s even when he needs nothing.
    #[test]
    fn the_effects_lie_on_the_floor_under_the_root_as_kh3_traces_it() {
        // a player start's root 92 cm above a flat floor: the trace (140 cm) meets the floor 92 cm under it
        let root = Vec3::new(100.0, -50.0, 92.0);
        assert_eq!(ground_offset(root, Some(Vec3::new(100.0, -50.0, 0.0)), GROUND_MISS), Vec3::new(0.0, 0.0, -92.0));
        // nothing within the trace (or it started inside something): 90 cm under the root
        assert_eq!(ground_offset(root, None, GROUND_MISS), Vec3::new(0.0, 0.0, -90.0));
        assert_eq!(ground_offset(root, Some(Vec3::NAN), GROUND_MISS), Vec3::new(0.0, 0.0, -90.0));
        let mut point = SavePoint::new(SavePointData::default(), root);
        assert_eq!(point.effects_at(), root, "zero until the host sets it");
        point.effect_offset = ground_offset(root, Some(Vec3::new(100.0, -50.0, 0.0)), GROUND_MISS);
        assert_eq!(point.effects_at(), Vec3::new(100.0, -50.0, 0.0));
    }

    #[test]
    fn the_first_visit_shows_the_heal_effect_for_three_seconds() {
        let mut sora = sora99();
        let centre = sora.output().capsule_center;
        let capsule = (centre, SORA_PARAMS.capsule_radius, SORA_PARAMS.capsule_half_height);
        let mut point = SavePoint::new(SavePointData::default(), Vec3::new(0.0, 0.0, 92.0));
        assert!(!SavePoint::needs_recovery(&sora));
        let mut frames = 0;
        for _ in 0..400 {
            if point.step(DT, &mut sora, capsule).healing {
                frames += 1;
            }
        }
        assert!((179..=181).contains(&frames), "{frames}");
        // The pack's data file says the same as the defaults.
        let pack = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack");
        if pack.join("savepoint/savepoint.json").is_file() {
            assert_eq!(SavePointData::load(&pack), SavePointData::default());
        }
    }
    /// (IMPL_ER26) A host that unlocks its save point in the reactor: the first-visit burst plays from the reactor on
    /// (once: 3 s in all), and nothing changes for a host that does not ask for it.
    #[test]
    fn a_host_that_unlocks_in_the_reactor_gets_the_first_visit_burst_there() {
        let mut sora = sora99();
        let centre = sora.output().capsule_center;
        // 120 cm beside the root: inside the reactor (110 + his radius), outside the recover capsule (50 + his radius)
        let capsule = (centre, SORA_PARAMS.capsule_radius, SORA_PARAMS.capsule_half_height);
        let root = Vec3::new(120.0, 0.0, 92.0);
        let mut plain = SavePoint::new(SavePointData::default(), root);
        let step = plain.step(DT, &mut sora, capsule);
        assert!(step.in_reactor && !step.in_recover && !step.healing);
        let mut point = SavePoint::new(SavePointData::default(), root);
        point.first_in_reactor = true;
        let frames = (0..400).filter(|_| point.step(DT, &mut sora, capsule).healing).count();
        assert!((179..=181).contains(&frames), "{frames}");
        assert!(!point.first_visit_pending());
        assert_eq!(point.sounded, 1, "the burst's sound, once");
    }
}
