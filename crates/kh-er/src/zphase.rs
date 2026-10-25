//! (ZETTA 2026-10-24) A TWO-PHASE BOSS KILLED WHOLE BY ZETTAFLARE, THE GAME'S WAY. The user: "killing a boss with a second
//! phase during it's first phase with Zettaflare should just kill the boss entirely skipping the second phase. The cutscene
//! should not play, I don't mean skipped, but it shouldn't play. You still get all the rewards for killing the boss and there
//! can be no progression blockers".
//!
//! Generic (no per-boss table): THE BOSS'S OWN MAP EVENTS ARE READ (read-only, `kh_erfile::emevd`, the map of the boss's
//! character, its file from the game's archives on a reader thread) into a plan (`emevd::boss_plan`): the PHASE-CHANGE
//! events (an HP threshold above 0 on the first form, or its death leading into a cutscene / an enabled character), their
//! cutscenes, THE DEFEAT EVENT (2003[12] HandleBossDefeat) and the final form it waits on, and whether the defeat or the
//! change is STORY-CRITICAL (a post-defeat cutscene: an ending; a move of the player or the map: a transition).
//!
//! THE WHOLE KILL (targets.rs at the hand-back, when the plan is not story-critical):
//! 1. the phase-change events are BLOCKED: the game's event update (`CSEmkEventIns`'s update, 2.7.1.0 0x140583ba0, the
//!    instance's event id at +0x30: read from the update and InitializeEvent's key builder 0x140584250) is not run for them,
//!    so they never reach their cutscene, never change the boss, never enable its second form, never show its second bar;
//! 2. the first form dies (its no-death states off, a lethal hit, its HP written 0 if a floor holds: zdefer / targets);
//! 3. NEXT CHARACTER (Godfrey -> Hoarah Loux): the final form is ENABLED the event's own way (2004[5]'s handler,
//!    0x1405e2770) hidden (its render switch off, never drawn) and its HP written 0: its death is the game's;
//! 4. THE GAME'S OWN DEFEAT EVENT then runs (it waits on the final form's death): its banner (hidden by ERUI), its flags,
//!    the item lots and everything that follows them, the boss bar's end, the arena's state. Its first flag is watched: if
//!    the game has not set it [`DEFEAT_WAIT_S`] s after the final form's death, THE LAST RESORT sets the defeat event's flags
//!    directly (logged loudly; its item lots are not awarded by that route: the log names it).
//!
//! STORY-CRITICAL bosses (Radagon -> the Elden Beast: the ending; Rennala: the change moves the player) are never blocked:
//! the game's own change plays (logged), as the user's rule "Never block the ending sequence or a story transition" wants.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::{mpsc, Mutex};

use eldenring::cs::{CSEventFlagMan, ChrIns, ChrSet, WorldChrMan};
use fromsoftware_shared::FromStatic;
use kh_erfile::emevd::{self, BossPlan};

use crate::erlog;
use crate::game::{self, plausible};
use crate::title::Known;
use crate::word;

/// The event instance's update (`CSEmkEventIns`): blocked events are not run.
pub(crate) const EVENT_UPDATE: Known = Known { name: "CSEmkEventIns update", rva: 0x0058_3BA0, rva_2_7_2_0: 0x0058_4050, bytes: "40 53 41 57 48 83 ec 38 f2 0f 10 41 30 48 8b d9 8b 41 38 4c 8b fa 48 8b 0d ?? ?? ?? ??" };
/// EMEVD 2004[5] ChangeCharacterEnableState's handler (entity by pointer, disable).
pub(crate) const ENABLE: Known = Known { name: "EMEVD ChangeCharacterEnableState", rva: 0x005E_2770, rva_2_7_2_0: 0x005E_2C20, bytes: "48 89 5c 24 08 57 48 83 ec 30 0f b6 fa c6 44 24 50 00 48 8b d9 e8 ?? ?? ?? ?? 48 8b cb 84 c0" };
/// The instance's event id.
pub const EVENT_ID_AT: usize = 0x30;
/// How long the game may take to set the defeat event's first flag after the final form's death (s).
pub const DEFEAT_WAIT_S: f32 = 20.0;

static ORIG_UPDATE: AtomicUsize = AtomicUsize::new(0);
static BLOCKED: Mutex<Vec<u32>> = Mutex::new(Vec::new());
static BLOCKED_RUNS: AtomicU32 = AtomicU32::new(0);
static TRIED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

type UpdateFn = unsafe extern "C" fn(*mut u8, usize);

unsafe extern "C" fn update_hook(ins: *mut u8, arg: usize) {
    let original: UpdateFn = std::mem::transmute(ORIG_UPDATE.load(Ordering::Relaxed));
    if plausible(ins as usize) {
        let id = std::ptr::read_unaligned(ins.add(EVENT_ID_AT) as *const u32);
        if BLOCKED.try_lock().is_ok_and(|b| b.contains(&id)) {
            BLOCKED_RUNS.fetch_add(1, Ordering::Relaxed);
            return;
        }
    }
    original(ins, arg)
}

/// The event block's hook, put in once (game thread). False: not available (the phase change is the game's).
fn hooked() -> bool {
    if ORIG_UPDATE.load(Ordering::Relaxed) != 0 {
        return true;
    }
    if TRIED.swap(true, Ordering::Relaxed) {
        return false;
    }
    let Some(at) = crate::title::at(&EVENT_UPDATE) else {
        erlog!("zphase: {} NOT found: a two-phase boss's change cannot be blocked (the game's own change plays)", EVENT_UPDATE.name);
        return false;
    };
    // SAFETY: the located function (bytes checked for the running version), hooked on the game thread like the others.
    match unsafe { kh_er_render::inline_hook(at, update_hook as *const () as usize, &ORIG_UPDATE, EVENT_UPDATE.name) } {
        Ok(()) => {
            erlog!("zphase: the game's event update is detoured (a phase change Zettaflare's whole kill blocks is not run)");
            true
        }
        Err(e) => {
            ORIG_UPDATE.store(0, Ordering::SeqCst);
            erlog!("zphase: the event update could NOT be detoured ({e}): the game's own change plays");
            false
        }
    }
}

/// THE PHASE-CHANGE EVENTS NOT RUN from now (until the mod's exit; the defeat flag ends them at the next load).
pub fn block(ids: &[u32]) -> bool {
    if !hooked() {
        return false;
    }
    if let Ok(mut b) = BLOCKED.lock() {
        for id in ids {
            if !b.contains(id) {
                b.push(*id);
            }
        }
    }
    true
}

/// How often a blocked event's update was refused (the log's evidence that it was the one).
pub fn blocked_runs() -> u32 {
    BLOCKED_RUNS.load(Ordering::Relaxed)
}

/// ENABLES a character the event's own way (2004[5] ChangeCharacterEnableState(entity, enabled)). False: not called.
pub fn enable(entity: u32) -> bool {
    let Some(f) = crate::title::at(&ENABLE) else { return false };
    let mut e = entity;
    // SAFETY: the game's own handler, called as its dispatcher calls it (the entity by pointer, "disable" false); game thread.
    unsafe {
        let call: unsafe extern "C" fn(*mut u32, u8) -> u64 = std::mem::transmute(f);
        call(&mut e, 0) & 0xff != 0
    }
}

// ------------------------------------------------------------------------------------------------- the boss's events

enum Slot {
    Loading,
    Ready(Vec<emevd::Event>),
    Failed(String),
}

struct Reader {
    maps: HashMap<String, Slot>,
    tx: Option<mpsc::Sender<String>>,
}

static READER: Mutex<Option<Reader>> = Mutex::new(None);

fn game_dir() -> std::path::PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())).unwrap_or_else(kh_erfile::archive::default_game_dir)
}

fn read_map(lib: &mut kh_erfile::Library, map: &str) -> Result<Vec<emevd::Event>, String> {
    let path = format!("/event/{map}.emevd.dcx");
    let (bytes, _) = lib.file_in(&path, &["Data0", "Data1", "Data2", "Data3", "DLC"])?.ok_or(format!("{path} is in no archive"))?;
    let raw = if kh_erfile::dcx::is_dcx(&bytes) { kh_erfile::dcx::decompress(&bytes, &game_dir())? } else { bytes };
    Ok(emevd::instances(&emevd::parse(&raw)?))
}

/// The map name of a block (`mAA_BB_CC_DD`).
pub fn map_name(area: u8, block: u8, region: u8, index: u8) -> String {
    format!("m{area:02}_{block:02}_{region:02}_{index:02}")
}

/// (ZETTA 2026-10-24) The cutscenes the defeat event of `entity` plays (None: its map's events not read yet).
pub fn defeat_cutscenes(map: &str, entity: u32) -> Option<Vec<i32>> {
    let g = READER.lock().ok()?;
    match g.as_ref()?.maps.get(map)? {
        Slot::Ready(ev) => Some(emevd::defeat_cutscenes(ev, entity)),
        _ => None,
    }
}

/// The map of a character (its block).
pub fn map_of(chr: &ChrIns) -> String {
    let b = chr.block_id;
    map_name(b.area(), b.block(), b.region(), b.index())
}

/// What is known of a boss now.
pub enum PlanNow {
    Loading,
    /// Its events have no second phase for it (a plain kill).
    NoPlan,
    Plan(BossPlan),
    Failed(String),
}

/// THE PLAN of the boss whose first form is `entity` in `map` (its events read on the reader thread the first time).
pub fn plan(map: &str, entity: u32) -> PlanNow {
    let Ok(mut g) = READER.lock() else { return PlanNow::Loading };
    let r = g.get_or_insert_with(|| Reader { maps: HashMap::new(), tx: None });
    match r.maps.get(map) {
        Some(Slot::Ready(ev)) => return emevd::boss_plan(ev, entity).map_or(PlanNow::NoPlan, PlanNow::Plan),
        Some(Slot::Failed(e)) => return PlanNow::Failed(e.clone()),
        Some(Slot::Loading) => return PlanNow::Loading,
        None => {}
    }
    if r.tx.is_none() {
        let (tx, rx) = mpsc::channel::<String>();
        if std::thread::Builder::new().name("kh-zphase-reader".into()).spawn(move || reader(rx)).is_err() {
            return PlanNow::Failed("the reader's thread cannot start".to_string());
        }
        r.tx = Some(tx);
    }
    r.maps.insert(map.to_string(), Slot::Loading);
    if let Some(tx) = &r.tx {
        let _ = tx.send(map.to_string());
    }
    PlanNow::Loading
}

fn reader(rx: mpsc::Receiver<String>) {
    let game = game_dir();
    let mut lib: Option<kh_erfile::Library> = None;
    for map in rx {
        let started = std::time::Instant::now();
        let result = (|| -> Result<Vec<emevd::Event>, String> {
            if lib.is_none() {
                lib = Some(kh_erfile::Library::open(&game)?);
            }
            read_map(lib.as_mut().ok_or("no library")?, &map)
        })();
        let slot = match result {
            Ok(ev) => {
                erlog!("zphase: the events of {map} read in {:.2} s: {} event instance(s)", started.elapsed().as_secs_f32(), ev.len());
                Slot::Ready(ev)
            }
            Err(e) => {
                erlog!("zphase: the events of {map} could NOT be read ({e}): a two-phase boss there gets the old route");
                Slot::Failed(e)
            }
        };
        if let Ok(mut g) = READER.lock() {
            if let Some(r) = g.as_mut() {
                r.maps.insert(map, slot);
            }
        }
    }
}

// ------------------------------------------------------------------------------------------------- the final form

fn each_chr(f: &mut dyn FnMut(usize)) {
    // SAFETY: game thread; the singleton lives as long as the game; every pointer is checked before it is followed.
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return };
    unsafe fn each<T>(set: &ChrSet<T>, f: &mut dyn FnMut(usize))
    where
        T: fromsoftware_shared::Subclass<ChrIns> + 'static,
    {
        let entries = word!(set.entries);
        let capacity = set.capacity as usize;
        if !plausible(entries) || capacity == 0 || capacity > 8192 {
            return;
        }
        for k in 0..capacity {
            let p = std::ptr::read_unaligned((entries + k * 16) as *const usize);
            if plausible(p) {
                f(p);
            }
        }
    }
    // SAFETY: as above.
    unsafe {
        for k in 0..wcm.chr_sets.len() {
            let w = word!(wcm.chr_sets[k]);
            if plausible(w) {
                each(&*(w as *const ChrSet<ChrIns>), f);
            }
        }
        each(&wcm.open_field_chr_set.base, f);
    }
}

/// The character of an event entity id.
pub fn chr_of_entity(entity: u32) -> Option<&'static mut ChrIns> {
    if entity == 0 {
        return None;
    }
    let mut found = 0usize;
    each_chr(&mut |p| {
        if found == 0 {
            // SAFETY: a character-set entry, checked plausible; read after completeness.
            let chr = unsafe { &*(p as *const ChrIns) };
            if game::chr_complete(chr) && chr.event_entity_id == entity {
                found = p;
            }
        }
    });
    // SAFETY: found complete in this walk; game thread.
    (found != 0).then(|| unsafe { &mut *(found as *mut ChrIns) })
}

fn flag(id: u32) -> Option<bool> {
    if id == 0 {
        return None;
    }
    // SAFETY: game thread; the game's own flag tables.
    let man = unsafe { CSEventFlagMan::instance() }.ok()?;
    if man.virtual_memory_flag.flag_blocks.is_null() {
        return None;
    }
    Some(man.virtual_memory_flag.get_flag(id))
}

fn set_flag(id: u32) -> bool {
    if id == 0 {
        return false;
    }
    // SAFETY: the SDK's own writer (nothing for a flag without a block); game thread.
    let Ok(man) = (unsafe { CSEventFlagMan::instance_mut() }) else { return false };
    if man.virtual_memory_flag.flag_blocks.is_null() {
        return false;
    }
    man.virtual_memory_flag.set_flag(id, true);
    true
}

/// A whole kill under way: the final form (NextCharacter: enabled hidden and killed) and the defeat watched.
#[derive(Clone, Debug)]
struct Whole {
    plan: BossPlan,
    since: f32,
    enabled: bool,
    final_dead_at: Option<f32>,
    registered: bool,
    resorted: bool,
}

static WHOLE: Mutex<Vec<Whole>> = Mutex::new(Vec::new());

/// Pure: the defeat's state, `since_dead` s after the final form's death (None: not dead yet), its flag as read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Defeat {
    Waiting,
    Registered,
    LastResort,
}

pub fn defeat_state(since_dead: Option<f32>, flag_on: bool) -> Defeat {
    if flag_on {
        Defeat::Registered
    } else if since_dead.is_some_and(|s| s >= DEFEAT_WAIT_S) {
        Defeat::LastResort
    } else {
        Defeat::Waiting
    }
}

/// THE WHOLE KILL BEGINS (targets.rs, the hand-back, before the first form's lethal hit): its change blocked, the final
/// form (another character) enabled hidden and killed by `frame`, the defeat watched. False: the block is not available
/// (the caller keeps the old route).
pub fn begin(plan: BossPlan) -> bool {
    if !block(&plan.block) {
        return false;
    }
    erlog!("zphase: WHOLE KILL of the boss {} ({:?}): its phase change {:?} is BLOCKED (never run: its cutscene(s) {:?} never start, its second form never comes); the final form {} {}; the game's defeat event {} runs on its death (flags {:?})", plan.entity, plan.shape, plan.block, plan.cutscenes, plan.final_entity, if plan.final_entity == plan.entity { "is the boss itself" } else { "is enabled hidden and killed" }, plan.defeat_event, plan.defeat_flags);
    if let Ok(mut w) = WHOLE.lock() {
        w.retain(|x| x.plan.entity != plan.entity);
        w.push(Whole { plan, since: 0.0, enabled: false, final_dead_at: None, registered: false, resorted: false });
    }
    true
}

/// Once a frame: the final forms enabled hidden and killed; the defeats watched (the last resort when the game never sets
/// the defeat).
pub fn frame(dt: f32) {
    let Ok(mut list) = WHOLE.lock() else { return };
    list.retain_mut(|w| {
        w.since += dt;
        // (2026-10-24) every character the defeat waits on (the final form, a second gargoyle): enabled the event's own way,
        // hidden, its HP written 0; the boss's first form is targets.rs' own
        let finals: Vec<u32> = if w.plan.finals.is_empty() { vec![w.plan.final_entity] } else { w.plan.finals.clone() };
        if !w.enabled {
            w.enabled = true;
            for fin in finals.iter().filter(|f| **f != w.plan.entity) {
                let done = enable(*fin);
                erlog!("zphase: {fin}, which the defeat waits on, ENABLED the event's own way ({}), hidden", if done { "done" } else { "the handler said no / not found" });
            }
        }
        let mut all_dead = true;
        for fin in finals.iter() {
            let other = *fin != w.plan.entity;
            match chr_of_entity(*fin) {
                Some(chr) => {
                    if other {
                        // never drawn: its render switch off every frame; its HP written 0 (its death is the game's)
                        chr.chr_flags1c5.set_enable_render(false);
                        crate::zdefer::clear_no_death(chr);
                        if chr.modules.data.hp > 0 && w.since > 0.2 {
                            chr.modules.data.as_mut().hp = 0;
                            erlog!("zphase: {fin}: its HP WRITTEN 0, hidden (its death is the game's)");
                        }
                    }
                    all_dead &= chr.modules.data.hp <= 0 || chr.chr_flags1c5.death_flag();
                }
                None => {}
            }
        }
        if w.final_dead_at.is_none() && all_dead && w.since > 0.2 {
            w.final_dead_at = Some(w.since);
            erlog!("zphase: everything the defeat waits on ({finals:?}) is DEAD {:.1} s into the whole kill: the game's defeat event {} should now run", w.since, w.plan.defeat_event);
        }
        let first = w.plan.defeat_flags.first().copied().unwrap_or(0) as u32;
        let on = flag(first) == Some(true);
        match defeat_state(w.final_dead_at.map(|t| w.since - t), on) {
            Defeat::Registered => {
                if !w.registered {
                    w.registered = true;
                    erlog!("zphase: THE DEFEAT IS REGISTERED by the game's own event {} (flag {first} ON, {:.1} s in; {} update(s) of the blocked change refused so far)", w.plan.defeat_event, w.since, blocked_runs());
                }
                false
            }
            Defeat::LastResort if !w.resorted => {
                w.resorted = true;
                let set: Vec<String> = w.plan.defeat_flags.iter().map(|f| format!("{f}={}", set_flag(*f as u32))).collect();
                erlog!("!!! zphase: THE GAME DID NOT REGISTER THE DEFEAT of {} within {DEFEAT_WAIT_S} s of its final form's death: LAST RESORT, the defeat event's flags set directly [{}] (its item lots are NOT awarded by this route; the boss's own rewards that follow those flags do)", w.plan.entity, set.join(", "));
                false
            }
            _ => w.since < 120.0,
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (2026-10-24, "there can be no progression blockers") the defeat is the game's when its flag rises; only when it never
    /// does after the final form's death is it set directly
    #[test]
    fn zphase_the_defeat_is_the_games_the_last_resort_only_after_a_while() {
        assert_eq!(defeat_state(None, false), Defeat::Waiting, "the final form not dead yet");
        assert_eq!(defeat_state(Some(3.0), false), Defeat::Waiting, "its event takes its seconds (Malenia's waits 4 s)");
        assert_eq!(defeat_state(Some(3.0), true), Defeat::Registered);
        assert_eq!(defeat_state(Some(DEFEAT_WAIT_S), false), Defeat::LastResort);
        assert_eq!(map_name(60, 52, 38, 0), "m60_52_38_00");
        assert_eq!(map_name(15, 0, 0, 0), "m15_00_00_00");
    }
}
