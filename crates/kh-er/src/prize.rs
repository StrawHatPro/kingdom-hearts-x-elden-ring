//! (prize) KH3'S PRIZE ORBS IN ELDEN RING (`prize.kh`) AND MUNNY AS THE CURRENCY (`munny.mirror`).
//! State notes: build/sora_tmp/IMPL_PRIZE_STATE.md (the offline design), build/sora_tmp/er/IMPL_ER9_STATE.md
//! (what was seen in the game).
//!
//! An enemy Sora brought to 0 (the same `ErTargets::kills` the EXP rule reads) bursts into orbs by
//! THE ONE RULE of prizemap.rs; the orbs are KH3's own (crates/kh-prize through
//! `kh_host::prize::PrizeHost`): thrown, bouncing on Elden Ring's collision through this host's
//! `World`, flying to Sora, applied through the core's status path, drawn as free effects of the
//! effect side (fx.rs), sounded through the core's audio seam.
//!
//! WHAT A KILL'S MUNNY IS: what the game itself adds to its rune counter for that kill (read from
//! the counter: every rate of the game's is in it: area, journey, rune-gain effects); the enemy
//! row's rune value (NpcParam getSoul) only when the game adds nothing within `prize.wait` (5 s). The stat orbs
//! burst out at the kill; the munny orbs when the game's number is known (0.85 s after the blow for an ordinary enemy).
//! (IMPL_ER19) A KILL WHOSE ROW NAMES NO RUNE VALUE (a boss: the game awards its runes by an event of its own, 6 - 8 s
//! later; or an enemy the game pays nothing for) waits `prize.wait_unpriced` (20 s) for the game's award and is worth
//! exactly that: NOTHING when the game gives nothing. (Before, the guess max HP x 0.4 was paid after 5 s: READ in the
//! user's session of 2026-10-13: 1354 munny, and by the mirror runes, for nine enemies the game paid nothing for; and a
//! boss's munny came partly as orbs of the guess, partly without orbs.) The guess is paid only with `prize.runes = 0`.
//!
//! THE ORBS' OWN SHAPE AGAINST THE WORLD (IMPL_ER19): their casts are made with the orb's own sphere (13 - 34 cm), not
//! with the cap that keeps Sora's capsule slim (`collision.radius` 32: it turned the large munny orb's 34 cm sphere
//! into a 32 cm capsule).
//!
//! THE ORBS AND THE GAME'S PHYSICS ORIGIN: an orb's position is a KH3 position, and those do not
//! change when the game shifts its origin (`coords::Frame::rebase` moves the frame's origin, not
//! the KH3 coordinates): the orbs stay where they lie in the world. (Only the testing warps
//! `sora.warp` / `dbg.goto` / `dbg.hop`, which move the frame under Sora, take them along.)
//!
//! THE MUNNY MIRROR (`munny.mirror`, prizemap.rs `Wallet`): the game's rune counter is the picture
//! of Sora's munny. This file reads and writes `PlayerGameData::rune_count`, reads `::level` and
//! the eight attributes (a level-up that happens all the same is put back).
//! - A SESSION STARTS with what the game's save holds in the counter: that IS the munny (so the
//!   munny persists in the game's own save, with or without `save.kh`).
//! - DEATH: the counter is emptied the moment the player dies (before Sora's own death is handed
//!   to the game, and on the first frame of a death the game caused): the game has no runes to
//!   drop; after the respawn the counter is the munny again. MUNNY IS NOT LOST ON DEATH.
//! - RESTING AT A GRACE (the game's grace menu up): the counter shows 0: its Level Up can raise
//!   nothing. A level that rises anyway is put back with the attributes, uncharged, and logged.
//!
//! | key | default | meaning |
//! |---|---|---|
//! | `prize.kh` | 1 | killed enemies drop KH3 prizes |
//! | `prize.runes` | 1 | 0 = the enemy's rune value (NpcParam getSoul) is never read: every kill's fallback is the guess from its maximum HP |
//! | `prize.wait` | sheet prize.award_wait (5) | seconds a kill with a rune value waits for the game's award before that value is paid |
//! | `prize.wait_unpriced` | sheet prize.award_wait_unpriced (20) | seconds a kill WITHOUT a rune value waits for the game's award before it is closed without munny |
//! | `prize.height` | sheet prize.spawn_height (80) | cm above the dead enemy's centre where its orbs burst out (KH3: the pawn's location, 0, after an offset that was not read) |
//! | `prize.glow` | sheet prize.glow (3.27) | a factor on the brightness of the orbs' effects only (3.27 = their emissive where KH3 shows it: see the row) |
//! | `prize.hp.small`, `prize.hp.large` | sheet rows (4, 20) | hit points of an HP orb here (KH3: 1 / 5) |
//! | `prize.size` | sheet prize.size (1) | a factor on the size of the orbs' effects (the picture only; KH3's are 0.5 / 0.75 / 1 of the particle system by orb size) |
//! | `prize.munny.expire` | sheet prize.munny_expire_pays (0) | 1 = a munny orb that vanishes uncollected gives its munny all the same (0 = KH3: lost) |
//! | `prize.log` | sheet prize.log_flight (1) | 1 = a line per orb when it flies to him or vanishes: where it burst out, its first ground contact, where it lay |
//! | `prize.munny_rate` | sheet er.munny_per_rune (1) | munny per rune |
//! | `prize.munny_hp` | sheet er.munny_per_max_hp (0.4) | munny per hit point when no rune value is read |
//! | `prize.weights` | sheet er.slot1_weights (67,36,10) | HP, MP, focus shares of the first slot |
//! | `prize.test` | - | ONE-SHOT: `hp[,runes[,times]]` drops what an enemy of that maximum HP would, 3 m in front of Sora |
//! | `munny.mirror` | 1 | the rune counter shows Sora's munny; purchases cost munny; rune gains become munny |
//! | `munny.mirror.gains` | 1 | a rune gain that is not a kill's reward (a rune item, a sale) becomes munny; 0 = it is taken away |

use std::collections::HashMap;
use std::sync::Mutex;

use eldenring::cs::{NpcParam, PlayerIns, SoloParamRepository};
use fromsoftware_shared::FromStatic;
use glam::Vec3;
use kh_core::status::PrizeKind;
use kh_core::{Sora, StatusRequest, World};
use kh_host::prize::PrizeHost;
use kh_prize::Rng;

use crate::control::{Control, OneShot};
use crate::erlog;
use crate::ermap::PrizeRules;
use crate::prizemap::{Rule, Wallet, WalletEvent, WalletIn};

/// (IMPL_ER19) THE WORLD AS AN ORB MEETS IT: Elden Ring's collision through this host's casts, WITHOUT the radius cap
/// of Sora's own capsule (`ErWorld::radius_cap`, IMPL_ER18: `collision.radius`): an orb is cast with its own sphere.
struct OrbWorld<'a>(&'a crate::world::ErWorld);

impl World for OrbWorld<'_> {
    fn sweep_capsule(&self, start: Vec3, end: Vec3, radius: f32, half_height: f32) -> Option<kh_core::Hit> {
        let cap = self.0.radius_cap.replace(f32::MAX);
        let hit = self.0.sweep_capsule(start, end, radius, half_height);
        self.0.radius_cap.set(cap);
        hit
    }

    fn line_trace(&self, start: Vec3, end: Vec3) -> Option<kh_core::Hit> {
        self.0.line_trace(start, end)
    }

    fn water_surface(&self, position: Vec3) -> Option<f32> {
        self.0.water_surface(position)
    }
}

/// What an enemy type's row says about its runes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuneRow {
    /// `NpcParam.getSoul` > 0.
    Value(i32),
    /// The row was read and names no runes (a boss, whose runes the game awards by its own event; an enemy it pays
    /// nothing for).
    Zero,
    /// The row could not be read (no id, no repository, no such row).
    Unread,
}

impl RuneRow {
    fn value(self) -> Option<i32> {
        match self {
            RuneRow::Value(v) => Some(v),
            _ => None,
        }
    }
}

/// What outlives a Sora (a respawn makes a new `SoraHost`): his munny.
struct Session {
    munny: Option<i32>,
    /// The Sora before this one ended with the player's death (the next start is a respawn).
    died: bool,
}

static SESSION: Mutex<Session> = Mutex::new(Session { munny: None, died: false });
/// `<root>logskh-munny-held.txt`: Sora's munny while the counter is held at 0 through a death. A run that
/// ends in that window leaves the game's save with an empty counter: the next start reads the munny here.
static HELD_FILE: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);

/// Where the marker file goes (frame.rs, once).
pub fn set_root(root: &std::path::Path) {
    if let Ok(mut file) = HELD_FILE.lock() {
        *file = Some(crate::log::dir().join("kh-munny-held.txt"));
    }
}

fn held_file() -> Option<std::path::PathBuf> {
    HELD_FILE.lock().ok().and_then(|f| f.clone())
}

/// (prize) hook in `SoraHost::start`, after the loadout and the save: a respawn keeps the munny he
/// had (KH3 loses nothing at a death), with or without `save.kh`; without the mirror and without a
/// save the munny also survives a loading screen.
/// (savegate) The last save is being loaded again: nothing of this session's munny carries over.
pub fn forget_session() {
    if let Ok(mut session) = SESSION.lock() {
        session.munny = None;
        session.died = false;
    }
    // (IMPL_ER13) A LOAD IS ASKED FOR (or the title is up): the wallet of the Sora that is still running is FROZEN from
    // this moment (it neither reads nor writes the game's rune counter any more): the counter the load brings is the
    // loaded slot's, not "runes the game gave", and nothing of the old session is written over it (IMPL_ER12's caveat 2:
    // slot 2 was loaded with its 1401 and the old wallet put its own 1234 back).
    LOAD_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    crate::savegate::publish_munny(None);
}

/// Counts the loads asked for (and the visits of the title): a wallet made before the latest one is frozen.
static LOAD_EPOCH: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// What the held-munny file says: (munny, the character key it belongs to). A file without a key (written before
/// IMPL_ER13: one file for every slot) belongs to nobody any more.
fn parse_held(text: &str) -> Option<(i32, String)> {
    let mut parts = text.split_whitespace();
    let munny = parts.next()?.parse::<i32>().ok().filter(|m| *m > 0)?;
    Some((munny, parts.next()?.to_string()))
}

pub fn on_start(sora: &mut Sora, control: &Control) {
    let Ok(mut session) = SESSION.lock() else { return };
    // (savegate; IMPL_ER13: for EVERY character, armed or not) THE MUNNY OF A LOADED `.khsave` STANDS: Sora's whole
    // state comes from the loaded slot's file, his munny with it; the game's counter is set to it (the two are
    // written together and say the same number: savegate.rs puts the munny into the counter while a save is
    // serialised). Only a character WITHOUT a file adopts the game's counter.
    if crate::save::file_munny() {
        session.munny = None;
        session.died = false;
        // SAFETY: game thread; the singleton lives as long as the game; the pointer is checked.
        let counter = unsafe { eldenring::cs::GameDataMan::instance() }.ok().filter(|d| crate::game::plausible(crate::word!(d.main_player_game_data))).map(|d| d.main_player_game_data.rune_count);
        erlog!("munny: Sora's save file was loaded: its {} munny stand (the game's loaded rune counter reads {counter:?}{}); the counter is set to them", sora.status().munny, if counter.is_some_and(|c| c as i64 != sora.status().munny as i64) { ": THEY DIFFER, the file wins" } else { "" });
        return;
    }
    let keep = session.died || (!control.on("munny.mirror") && !control.on("save.kh"));
    if let (Some(munny), true) = (session.munny, keep) {
        if sora.status().munny != munny {
            erlog!("munny: Sora starts again with the {munny} munny he had (the loadout / save said {})", sora.status().munny);
            sora.status_mut().munny = munny;
        }
    }
}

/// A kill whose munny is not known yet.
struct Waiting {
    at: Vec3,
    tier: usize,
    target: u32,
    soul: Option<i32>,
}

pub struct ErPrizes {
    host: PrizeHost,
    rule_rng: Rng,
    on: bool,
    wallet: Wallet,
    waiting: HashMap<u32, Waiting>,
    next_key: u32,
    /// This Sora started after a death (the first wallet frame syncs instead of adopting).
    respawn: bool,
    /// The Tarnished's level and eight attributes as last seen without a level-up.
    stats: Option<(u32, [u32; 8])>,
    test: OneShot,
    mirror_told: Option<bool>,
    said_runes: bool,
    /// Totals for the once-a-second line: runes the game awarded for kills, their rows' values, kills paid.
    awarded: (u64, u64, u32),
    /// The counter as the dead frames saw it last (its changes during a death are logged).
    dead_runes: Option<u32>,
    /// The counter is held at 0 (a death, a rest) and the marker file holds the munny.
    holding: bool,
    munny_shot: OneShot,
    /// (IMPL_ER13) `LOAD_EPOCH` when this wallet was made, and for how long it has been frozen by a later load request.
    epoch: u32,
    frozen_for: f32,
    /// (IMPL_ER19) The host's rows `prize.*` (sheet sora-er-host; the control keys override them).
    rules: PrizeRules,
    /// Kills closed without munny (the game awarded nothing for a row without a rune value).
    unpaid: u32,
    /// The magnet radius last said in the log (cm; -1 = no limit).
    radius_told: Option<i32>,
    /// (IMPL_ER20) NO ORBS NOW (a real-time cutscene plays, or he is dead): nothing is thrown; a kill's munny is
    /// given at once when the game's award is known.
    cutscene: bool,
    dead: bool,
}

/// (IMPL_ER20) THE RULE FOR MUNNY THAT IS STILL IN ORBS when he can no longer collect them. An orb's munny is the
/// game's own award for a kill (taken out of the counter when the orb was made): KH3 loses a prize only when its
/// living time runs out. So: at his DEATH, at the start of a REAL-TIME CUTSCENE and when Sora STOPS with the
/// player still there (the payload goes), the munny orbs' munny is his at once (no flight, no popup) and every orb
/// is removed (HP / MP / focus orbs unapplied). When a LOAD was asked for, nothing is given: the loaded save's
/// munny stands, and the kills belong to the time that is abandoned.
pub(crate) fn settle_line(why: &str, orbs: usize, munny: i64, given: bool, total: i32) -> String {
    format!("prize: {why}: {orbs} orb(s) removed{}", if munny <= 0 { String::new() } else if given { format!("; the {munny} munny they held are Sora's at once (the game had awarded them): munny {total}") } else { format!("; the {munny} munny they held are NOT given (a load: the loaded save's munny stands)") })
}

/// The rune value of an enemy type (`NpcParam.getSoul`), when the row can be read. NOT what the
/// game finally awards (it multiplies it): the wallet reads the award itself; this is the fallback
/// and the weight when several kills share one award.
fn rune_value(npc_param: i32) -> RuneRow {
    if npc_param <= 0 {
        return RuneRow::Unread;
    }
    // SAFETY: game thread; the singleton lives as long as the game. Its holder table is checked by the SDK.
    let Ok(repo) = (unsafe { SoloParamRepository::instance() }) else { return RuneRow::Unread };
    match repo.get::<NpcParam>(npc_param as u32).map(|row| row.get_soul() as i32) {
        Some(soul) if soul > 0 => RuneRow::Value(soul),
        Some(_) => RuneRow::Zero,
        None => RuneRow::Unread,
    }
}

fn attributes(data: &eldenring::cs::PlayerGameData) -> [u32; 8] {
    [data.vigor, data.mind, data.endurance, data.strength, data.dexterity, data.intelligence, data.faith, data.arcane]
}

impl ErPrizes {
    /// (loot) The orbs' host: the item boxes of a kill are thrown into the same simulation (loot.rs).
    pub fn loot_host(&mut self) -> &mut PrizeHost {
        &mut self.host
    }

    pub fn start(control: &Control, rules: PrizeRules) -> ErPrizes {
        let respawn = SESSION.lock().map(|mut s| std::mem::take(&mut s.died)).unwrap_or(false);
        let respawn = crate::save::take_file_munny() || respawn; // (savegate) the file's munny: the counter is synced to it, not adopted
        ErPrizes { host: PrizeHost::new(0x4552_5052), rule_rng: Rng::new(0x52_554C), on: false, wallet: Wallet::default(), waiting: HashMap::new(), next_key: 1, respawn, stats: None, test: OneShot::new("prize.test", control), mirror_told: None, said_runes: false, awarded: (0, 0, 0), dead_runes: None, holding: false, munny_shot: OneShot::new("dbg.munny", control), epoch: LOAD_EPOCH.load(std::sync::atomic::Ordering::Relaxed), frozen_for: 0.0, rules, unpaid: 0, radius_told: None, cutscene: false, dead: false }
    }

    /// `prize.height` over the sheet's row: cm above the dead enemy's centre where its orbs burst out.
    fn spawn_height(&self, control: &Control) -> f32 {
        control.f32("prize.height").filter(|v| (-200.0..=500.0).contains(v)).unwrap_or(self.rules.spawn_height)
    }

    /// The keys over the sheet's rows, into the orbs' host and the wallet (every step and frame: the keys are live).
    fn configure(&mut self, control: &Control) {
        // (VFXR) the orbs' materials are eye-adaptation materials: the renderer now gives every such material KH3's factor
        // (kh_er_render::vfx::eye_boost, 3.27), so prize.glow (3.27 = where KH3 shows them) is divided by it (1 = no extra)
        // (ORBS) the same formula as the offline harness's `--orbs` (kh_er_render::vfx::prize_gain); the orbs' colours and
        // brightness are in their effects (pack, ORBS orbs_look.js), their lit balls' shading in er_vfx.wgsl (KH_ORB)
        self.host.glow = kh_er_render::vfx::prize_gain(control.f32("prize.glow").filter(|v| (0.0..=100.0).contains(v)).unwrap_or(self.rules.glow));
        self.host.size = control.f32("prize.size").filter(|v| (0.05..=10.0).contains(v)).unwrap_or(self.rules.size);
        self.host.pay_expired_munny = control.flag("prize.munny.expire").unwrap_or(self.rules.munny_expire_pays);
        self.host.log_flight = control.flag("prize.log").unwrap_or(self.rules.log_flight);
        self.wallet.set_waits(control.f32("prize.wait").unwrap_or(self.rules.award_wait), control.f32("prize.wait_unpriced").unwrap_or(self.rules.award_wait_unpriced));
    }

    /// (IMPL_ER20) Every orb removed now; the munny orbs' munny given at once unless a load was asked for.
    fn settle(&mut self, sora: &mut Sora, why: &str) {
        let host = &mut self.host;
        let mut out = None;
        if !crate::fx::with_effects(|fx| out = Some(host.recall(Some(fx)))) {
            // (the effect side is busy or not there: `Fx` ends every effect when Sora goes)
            out = Some(host.recall(None));
        }
        let (orbs, munny) = out.unwrap_or((0, 0));
        if orbs == 0 {
            return;
        }
        let loading = self.epoch != LOAD_EPOCH.load(std::sync::atomic::Ordering::Relaxed);
        if munny > 0 && !loading {
            let status = sora.status_mut();
            status.munny = (status.munny as i64 + munny).min(i32::MAX as i64) as i32;
            if let Ok(mut session) = SESSION.lock() {
                session.munny = Some(status.munny);
            }
        }
        erlog!("{}", settle_line(why, orbs, munny, !loading, sora.status().munny));
    }

    /// (IMPL_ER20) hook in sora.rs `follow_phase`: a real-time cutscene starts / is over. While one plays no orb
    /// is out: those that were are removed (their munny given: `settle`), nothing new is thrown.
    pub fn cutscene(&mut self, on: bool, sora: &mut Sora) {
        if on && !self.cutscene {
            self.settle(sora, "a cutscene starts");
        }
        self.cutscene = on;
    }

    /// The host's HP values on a slot (`prize.hp.small` / `prize.hp.large` over the sheet's rows).
    fn hp_values(&self, control: &Control) -> (i32, i32) {
        let key = |name: &str, row: i32| control.f32(name).filter(|v| (1.0..=9999.0).contains(v)).map_or(row, |v| v as i32);
        (key("prize.hp.small", self.rules.hp_small), key("prize.hp.large", self.rules.hp_large))
    }

    fn rule(control: &Control) -> Rule {
        Rule::default().with_overrides(control.str("prize.weights"), control.f32("prize.munny_rate"), control.f32("prize.munny_hp"))
    }

    /// AN ENEMY SORA KILLED (sora.rs, where the EXP is given): the stat orbs burst out now; the
    /// munny orbs follow when the wallet knows what the game awarded.
    /// (IMPL_ER22) KH3's `AllPrizeCollection` (the boss finish, when the white-out is full): every orb on the field
    /// flies to Sora. Returns how many.
    pub fn collect_all(&mut self) -> usize {
        self.host.prizes.collect_all()
    }

    pub fn on_kill(&mut self, control: &Control, sora: &mut Sora, id: u32, npc_param: i32, max_hp: i32, at: Vec3) {
        self.on = control.on("prize.kh");
        if !self.on {
            return;
        }
        self.configure(control);
        let rule = Self::rule(control);
        // `prize.runes = 0`: the rows are not read: every kill is priced by the guess from its maximum HP (the old rule)
        let guess = control.flag("prize.runes") == Some(false);
        let row = if guess { RuneRow::Unread } else { rune_value(npc_param) };
        let soul = row.value();
        if !self.said_runes {
            self.said_runes = true;
            erlog!("prize: the first kill: NpcParam {npc_param} getSoul reads {row:?} (the munny is what the game itself adds to its counter; a row's value is the fallback; a row without one is worth what the game awards, nothing else)");
        }
        let mut drop = rule.drop_for(max_hp, soul, &mut self.rule_rng);
        // (IMPL_ER19) no rune value in the row: no fallback (the guess only with prize.runes = 0)
        let unpriced = soul.is_none() && !guess;
        if unpriced {
            drop.munny = 0;
        }
        let mods = kh_prize::host::mods_of(sora);
        // (ER party S6, P2) the party's abilities count for the drop: PRIZE_FEEVER on any member, LUCK_UP summed, converters
        #[cfg(feature = "party")]
        let mods = crate::partyui::prize_mods(control, mods);
        let centre = at;
        let at = at + Vec3::Z * self.spawn_height(control);
        let (hp_small, hp_large) = self.hp_values(control);
        let quiet = self.cutscene || self.dead; // (IMPL_ER20) no orbs in a cutscene or after his death
        if let Some(slot) = drop.slots.first_mut().filter(|_| !quiet) {
            crate::prizemap::hp_amounts(slot, hp_small, hp_large);
            self.host.prizes.drop_orbs(at, slot.kind, slot.count, &slot.amounts, &mods, false);
        }
        self.host.take(sora);
        let key = self.next_key;
        self.next_key = self.next_key.wrapping_add(1).max(1);
        if unpriced {
            self.wallet.kill_unpriced(key, max_hp.max(1) as f32);
        } else {
            self.wallet.kill(key, soul.unwrap_or(1).max(1) as f32, drop.munny);
        }
        self.waiting.insert(key, Waiting { at, tier: drop.tier, target: id, soul });
        let getter = kh_prize::Getter::sora(sora, sora.output().capsule_center);
        erlog!(
            "prize: target {id} (NpcParam {npc_param}, max HP {max_hp}, {}) KILLED by Sora -> tier {}: {} now, bursting out {:.0} cm above its centre = {:+.0} cm from Sora's centre in height, {:.0} cm from him (his magnet radius: {}); its munny when the game's award is known ({})",
            match row {
                RuneRow::Value(r) => format!("rune value {r}"),
                RuneRow::Zero => "its row names NO rune value".to_string(),
                RuneRow::Unread => if guess { "rows not read: prize.runes = 0".to_string() } else { "its row could not be read".to_string() },
            },
            drop.tier,
            drop.slots.first().map_or(String::new(), |s| format!("{:?} x{}", s.kind, s.count)),
            at.z - centre.z,
            at.z - getter.point.z,
            (at - getter.point).truncate().length(),
            getter.radius.map_or("no limit".to_string(), |r| format!("{r:.0} cm")),
            if unpriced { "no fallback: nothing unless the game awards something".to_string() } else { format!("fallback {}", drop.munny) },
        );
    }

    /// AFTER EACH SIMULATION STEP of Sora, before the effect side's step: the orbs move against
    /// the game's collision, their sounds and prizes go to the core, their effects are placed.
    pub fn after_step(&mut self, control: &Control, dt: f32, world: &crate::world::ErWorld, sora: &mut Sora, capsule_center: Vec3, forward: Vec3) {
        self.on = control.on("prize.kh");
        self.configure(control);
        let world = &OrbWorld(world);
        let spawn_height = self.spawn_height(control);
        if let Some(value) = self.test.fired(control).filter(|_| self.on) {
            let parts: Vec<i32> = value.split(',').filter_map(|p| p.trim().parse::<i32>().ok()).collect();
            if let Some(&hp) = parts.first() {
                let rule = Self::rule(control);
                let mods = kh_prize::host::mods_of(sora);
                let times = parts.get(2).copied().unwrap_or(1).clamp(1, 200);
                let mut orbs = 0;
                for _ in 0..times {
                    let mut drop = rule.drop_for(hp, parts.get(1).copied(), &mut self.rule_rng);
                    let (hp_small, hp_large) = self.hp_values(control);
                    for slot in &mut drop.slots {
                        crate::prizemap::hp_amounts(slot, hp_small, hp_large);
                        self.host.prizes.drop_orbs(capsule_center + forward * 300.0 + Vec3::Z * spawn_height, slot.kind, slot.count, &slot.amounts, &mods, false);
                        orbs += slot.count;
                    }
                }
                self.host.take(sora);
                erlog!("prize: prize.test: {times} drop(s) of an enemy of {hp} max HP: {orbs} orb(s) 3 m in front of him");
            }
        }
        if !self.on && self.host.prizes.is_empty() {
            return;
        }
        // (ER party S6, P2) KH3's getters are the player and his party (FindGetter 0x1414794C0): with friends in the party
        // every member is a getter slot (the nearest inside his own magnet radius), the leader is the player pawn,
        // Share Prizes passes HP / MP on; Sora alone as before
        #[cfg(feature = "party")]
        let party = match crate::partyui::prize_members(control, sora, capsule_center) {
            Some(members) => {
                let point = members.iter().find(|m| m.player).map_or(capsule_center, |m| m.getter.point);
                let mut to = crate::partyui::PartyPrizes { members: members.clone() };
                self.host.step_party(dt, world, sora, &members, point, &mut to);
                true
            }
            None => false,
        };
        #[cfg(not(feature = "party"))]
        let party = false;
        if !party {
            self.host.step(dt, world, sora, capsule_center);
        }
        for line in self.host.log.drain(..) {
            erlog!("prize: {line}");
        }
        // (IMPL_ER19) his magnet radius, said whenever it changes (300 / 600 / 900 cm by Treasure Magnet copies; the
        // Kingdom Key carries one)
        let radius = kh_prize::Getter::sora(sora, capsule_center).radius.map_or(-1, |r| r as i32);
        if self.radius_told != Some(radius) {
            self.radius_told = Some(radius);
            let status = sora.status();
            erlog!("prize: Sora collects orbs from {} (Treasure Magnet copies counted: {}, Master Treasure Magnet: {})", if radius < 0 { "any distance".to_string() } else { format!("{radius} cm") }, status.count(kh_core::enums::ETresAbilityKind::PrizeDraw), status.has(kh_core::enums::ETresAbilityKind::MasterDraw));
        }
        let host = &mut self.host;
        crate::fx::with_effects(|fx| host.show(fx));
    }

    /// ONCE PER GAME FRAME while the game's player lives (`ChrIns_PrePhysics`): the wallet.
    /// `resting`: he sits at a site of grace (the game's grace menu).
    pub fn frame(&mut self, control: &Control, player: &mut PlayerIns, sora: &mut Sora, dt: f32, resting: bool) {
        self.wallet_frame(control, player, sora, dt, false, resting);
    }

    /// THE PLAYER IS DEAD OR ABOUT TO BE (Sora's death is handed to the game this frame; or the
    /// game's own kill was seen): with the mirror the counter is emptied: nothing can be dropped.
    pub fn dead_frame(&mut self, control: &Control, player: &mut PlayerIns, sora: &mut Sora, dt: f32) {
        if let Ok(mut session) = SESSION.lock() {
            session.died = true;
        }
        // (IMPL_ER20) the orbs that are out go with his death; the munny in them is his (`settle`)
        if !self.dead {
            self.dead = true;
            self.settle(sora, "Sora's death");
        }
        self.wallet_frame(control, player, sora, dt, true, false);
    }

    fn wallet_frame(&mut self, control: &Control, player: &mut PlayerIns, sora: &mut Sora, dt: f32, dead: bool, resting: bool) {
        let mirror = control.on("munny.mirror");
        let accept = control.flag("munny.mirror.gains") != Some(false);
        self.configure(control);
        // (testing aid) `dbg.munny = N` (one-shot): Sora's munny is set to N
        if let Some(n) = self.munny_shot.fired(control).and_then(|v| v.trim().parse::<i32>().ok()).filter(|n| *n >= 0) {
            erlog!("dbg: Sora's munny set to {n} (it was {}; dbg.munny)", sora.status().munny);
            sora.status_mut().munny = n;
        }
        if self.mirror_told != Some(mirror) {
            self.mirror_told = Some(mirror);
            if mirror {
                erlog!("munny: the mirror is ON: the game's rune counter shows Sora's munny; what the game takes is spent, what it gives {}", if accept { "becomes munny (a kill's as orbs)" } else { "is taken away unless a kill's orbs carry it" });
            } else {
                erlog!("munny: the mirror is off: the rune counter is the game's own (read only, for the value of a kill)");
            }
        }
        if !crate::game::plausible(crate::word!(player.player_game_data)) {
            return;
        }
        // (IMPL_ER13) FROZEN: a load was asked for after this wallet was made: the counter is the loaded slot's business
        // until the new Sora starts. If nothing comes of the load in 30 s (this Sora is still running), the wallet starts
        // over by setting the counter to his munny.
        let epoch = LOAD_EPOCH.load(std::sync::atomic::Ordering::Relaxed);
        if self.epoch != epoch {
            if self.frozen_for == 0.0 {
                erlog!("munny: a load was asked for: this Sora's wallet is FROZEN (the rune counter is neither read nor written until the loaded slot's Sora starts)");
            }
            self.frozen_for += dt.clamp(0.0, 0.5).max(0.004);
            if self.frozen_for < 30.0 || crate::savegate::busy() {
                return;
            }
            erlog!("munny: nothing came of the load in 30 s: the wallet starts over (the counter is set to Sora's munny {})", sora.status().munny);
            self.epoch = epoch;
            self.frozen_for = 0.0;
            self.wallet = Wallet::default();
            self.respawn = true;
        }
        // SAFETY: the player's game data, alive with the player (the pointer was checked).
        let data = unsafe { player.player_game_data.as_mut() };
        // (a start that is not a respawn: a load of the game's save, or a loading screen in play, across which
        // the counter still holds what the mirror last wrote: either way the counter IS the munny)
        let adopt = mirror && !self.respawn;
        let input = WalletIn { runes: data.rune_count, level: data.level, munny: sora.status().munny, dead, mirror, accept_gains: accept, resting, adopt, dt };
        if dead {
            if self.dead_runes != Some(data.rune_count) {
                erlog!("munny: death: the game's counter reads {} (it read {:?} on the dead frame before)", data.rune_count, self.dead_runes);
                self.dead_runes = Some(data.rune_count);
            }
        } else {
            self.dead_runes = None;
        }
        let mut out = self.wallet.frame(input);
        let mut level_up = false;
        // THE COUNTER IS HELD AT 0 (a death, a rest) while the munny is not 0: the game may save in that window, and a
        // run that ends in it (a crash, the process ended, a payload replaced) would start the next one with an empty
        // counter: the munny of the moment is kept in the marker file meanwhile.
        let hold = mirror && (dead || resting);
        if hold != self.holding {
            self.holding = hold;
            if let Some(path) = held_file() {
                if hold {
                    // (IMPL_ER13) with the character's key: the file is one for all slots and must only ever serve its own
                    let _ = std::fs::write(&path, format!("{} {}", sora.status().munny, crate::save::current_key().unwrap_or_default()));
                } else if !matches!(out.events.first(), Some(WalletEvent::Adopted { .. })) {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
        if let (Some(WalletEvent::Adopted { runes, munny_before }), Some(path)) = (out.events.first().copied(), held_file()) {
            let mine = crate::save::current_key();
            if let Some(held) = std::fs::read_to_string(&path).ok().and_then(|t| parse_held(&t)).filter(|(_, key)| runes == 0 && mine.as_deref() == Some(key.as_str())).map(|(m, _)| m) {
                erlog!("munny: an earlier run ended while the counter was held at 0 (a death or a rest): the {held} munny Sora had then are his again");
                out.munny_delta = held - munny_before.max(0);
                out.write_runes = Some(if resting { 0 } else { held as u32 });
                if resting {
                    self.holding = true;
                }
            }
            if !self.holding {
                let _ = std::fs::remove_file(&path);
            }
        }
        for event in &out.events {
            match *event {
                WalletEvent::Adopted { runes, munny_before } => {
                    erlog!("munny: Sora starts: the game holds {runes} in its counter: THAT IS SORA'S MUNNY now (his loadout / save said {munny_before})");
                }
                WalletEvent::Synced { munny, held } => erlog!("munny: the rune counter is set to Sora's munny {munny} (it held {held})"),
                WalletEvent::KillPaid { key, munny, awarded, fallback } => {
                    let Some(kill) = self.waiting.remove(&key) else { continue };
                    let rule = Self::rule(control);
                    // (SHOP, the user 2026-10-23: "reduce the amount of money most enemies give you") an ordinary enemy (its row names a
                    // rune value) pays the sheet sora-er-shop `munny.kill_scale` share of the award by its prize tier; a boss (no rune
                    // value: the game awards its runes by its own event) `munny.boss_scale` (the whole); `munny.kill_cut = 0`: no cut
                    let cut = if control.flag("munny.kill_cut") == Some(false) { 1.0 } else { kh_host::shop::rules().kill_scale(kill.tier, kill.soul.is_some()) };
                    let worth = (munny as f32 * rule.munny_per_rune * cut).round() as i32;
                    if awarded {
                        self.awarded = (self.awarded.0 + munny.max(0) as u64, self.awarded.1 + kill.soul.unwrap_or(0).max(0) as u64, self.awarded.2 + 1);
                    }
                    if !awarded && fallback == 0 {
                        self.unpaid += 1;
                    }
                    erlog!(
                        "prize: target {}: {} -> {worth} munny in orbs (x{cut:.2} by munny.kill_scale / boss_scale)",
                        kill.target,
                        if awarded {
                            format!("THE GAME AWARDED {munny} rune(s) for it (its row's getSoul: {}{})", kill.soul.map_or("none".to_string(), |s| s.to_string()), kill.soul.filter(|s| *s > 0).map_or(String::new(), |s| format!(", x{:.3}", munny as f32 / s as f32)))
                        } else if fallback == 0 {
                            "the game awarded NOTHING for it and its row names no rune value: no munny".to_string()
                        } else {
                            format!("the game added nothing to its counter in time: the fallback {fallback}")
                        }
                    );
                    if (self.cutscene || self.dead) && self.on && worth > 0 {
                        // (IMPL_ER20) no orbs now: the award is his at once (the popup with it while he lives)
                        erlog!("prize: target {}: no orbs {}: its {worth} munny are given at once", kill.target, if self.dead { "after his death" } else { "in a cutscene" });
                        if self.dead {
                            let status = sora.status_mut();
                            status.munny = status.munny.saturating_add(worth);
                            if let Ok(mut session) = SESSION.lock() {
                                session.munny = Some(status.munny);
                            }
                        } else {
                            sora.status_request(StatusRequest::PrizeCollected { kind: PrizeKind::Munny1, amount: worth });
                        }
                    } else if let (true, Some(slot)) = (self.on, rule.munny_slot(kill.tier, worth)) {
                        let mods = kh_prize::host::mods_of(sora);
                        self.host.prizes.drop_orbs(kill.at, slot.kind, slot.count, &slot.amounts, &mods, false);
                        self.host.take(sora);
                    }
                }
                WalletEvent::GainTaken(n) => erlog!("munny: the game gave {n} rune(s) outside a kill (a rune item, a sale, a pick-up): now munny"),
                WalletEvent::GainDropped(n) => erlog!("munny: the game gave {n} rune(s) outside a kill: taken away again (munny.mirror.gains = 0)"),
                WalletEvent::GainLeft(n) => erlog!("munny: the game gave {n} rune(s) outside a kill (no mirror: they stay runes)"),
                WalletEvent::Spent(n) => erlog!("munny: {n} spent (the game took them from its counter): munny {} -> {}", sora.status().munny, (sora.status().munny + out.munny_delta).max(0)),
                WalletEvent::LevelUp { from, to, runes } => {
                    level_up = true;
                    erlog!("munny: !!! THE TARNISHED'S LEVEL ROSE {from} -> {to} (the game took {runes}): levelling is not part of this game: the level and the attributes are put back, nothing is charged");
                }
                WalletEvent::DeathHeld(n) => erlog!("munny: DEATH: the game's counter held {n}: emptied before the game can drop it; Sora keeps his {} munny", sora.status().munny),
                WalletEvent::DeathAgain(n) => erlog!("munny: death: the game wrote {n} into its counter: emptied again"),
            }
        }
        // (the HUD's munny popup comes with the status path: gains go through it, payments are plain)
        if out.munny_delta > 0 && !out.events.iter().any(|e| matches!(e, WalletEvent::Adopted { .. })) {
            sora.status_request(StatusRequest::PrizeCollected { kind: PrizeKind::Munny1, amount: out.munny_delta });
            // (the request is carried out in his next step: the counter follows then; until then
            // the wallet must not see the munny as unchanged and write the old value: it does, and
            // the next frame's write corrects it: harmless)
        } else if out.munny_delta != 0 {
            let status = sora.status_mut();
            status.munny = (status.munny + out.munny_delta).max(0);
        }
        if let Some(runes) = out.write_runes {
            data.rune_count = runes;
        }
        if level_up {
            if let Some((level, a)) = self.stats {
                data.level = level;
                (data.vigor, data.mind, data.endurance, data.strength, data.dexterity, data.intelligence, data.faith, data.arcane) = (a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]);
            }
        } else if !dead {
            self.stats = Some((data.level, attributes(data)));
        }
        if !dead {
            if let Ok(mut session) = SESSION.lock() {
                session.munny = Some(sora.status().munny);
            }
        }
        // (IMPL_ER13) what a save of the game's must hold in the counter (savegate.rs)
        crate::savegate::publish_munny(mirror.then(|| sora.status().munny));
    }

    /// The once-a-second line (None while nothing of it is on).
    pub fn summary(&self, control: &Control, sora: &Sora, runes: u32) -> Option<String> {
        let mirror = control.on("munny.mirror");
        (self.on || mirror || !self.host.prizes.is_empty()).then(|| {
            format!(
                "prize: {} | {} | munny {} | the game's counter {runes}{} | kills waiting for their award {} | awarded for {} kill(s): {} rune(s) (their rows' getSoul: {}){}",
                if self.on { "on" } else { "off" },
                self.host.summary(),
                sora.status().munny,
                if mirror { " (mirror)" } else { "" },
                self.wallet.pending_kills(),
                self.awarded.2,
                self.awarded.0,
                self.awarded.1,
                if self.unpaid > 0 { format!(" | {} kill(s) without a rune value got nothing from the game: no munny", self.unpaid) } else { String::new() },
            )
        })
    }

    /// Sora stops (the payload goes, a load): the orbs and their effects go; the munny in them by `settle`'s rule.
    /// Called BEFORE his state is captured for the save file.
    pub fn stop(&mut self, sora: &mut Sora) {
        self.settle(sora, "Sora stops");
    }
}
