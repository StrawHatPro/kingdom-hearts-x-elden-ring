//! (IMPL_ER33, the user 2026-10-13 with a picture of Melina's first scene: "also this cutscene has grace instead of save
//! point") A REAL-TIME CUTSCENE'S OWN SITE OF GRACE, hidden; KH3's save point is drawn there instead (savepoint.rs keeps
//! its save points drawn in a scene, fx.rs keeps the effects running: `cut.savepoint`).
//!
//! WHY THE GRACE SHOWED IN THE SCENE although graceflame.rs hides it in play (the user's log, er/logs/kh-er.log
//! 474.7 - 493.3 s): the scene is made of its OWN models (`cutfig: A SCENE: its animated models: ... AEG099: 1 ...`: one
//! act animates an `AEG099` object, the asset family of the grace's `AEG099_060`), and graceflame.rs only knows the
//! effect slots of MAP objects (the blocks' object lists): the number of grace objects it knew did not change when the
//! scene began (40 before, during and after). The scene's grace and its light are the scene's, so nothing zeroed them.
//! IMPL_ER17 had seen the same ("the scene draws its own golden flame ... left alone").
//!
//! WHAT IS DONE (UNTESTED IN GAME; every part logs what it found, line prefix `cutgrace:`):
//! 1. THE MODEL: the scene's animation acts whose object name begins with a grace asset (`cut.grace.models`, default
//!    `AEG099_060`) get bit 0 of their model's display flags cleared every frame, as the scene's figure of the player
//!    is hidden (figure.rs). Hides the model (the pile on the ground, if it is part of that model).
//! 2. THE LIGHT: an effect of the scene is a record of the game's effect pool like any other, but nothing leads to it
//!    from the scene (NOT MEASURED), so the pool is searched: a worker thread reads the process's private memory for
//!    effect records (the search of `dbg.fxlist`, 4 times in a scene: 0.4, 2.5, 6 and 12 s after its start), and the
//!    records whose effect id is one of the grace's (the sheet's `grace.flame.sfx.*` = 1010 / 1011, plus
//!    `cut.grace.sfx = id,id`) get their seven size / colour floats set to 0 on the game thread, each checked again
//!    right before (the write graceflame.rs makes). The first search of a scene lists EVERY effect id that is live, so
//!    a scene whose grace uses other ids names them in the log (then: `cut.grace.sfx`).
//!
//! | key | default | meaning |
//! |---|---|---|
//! | `cut.grace` | 1 | 0 = a cutscene's own grace is left as the game draws it |
//! | `cut.grace.models` | `AEG099_060` | the object-name beginnings of the scene models that are hidden |
//! | `cut.grace.sfx` | - | more effect ids to zero in a scene (besides the sheet's two) |
//! | `cut.grace.scan` | 1 | 0 = the effect pool is not searched (the model is still hidden) |

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use windows_sys::Win32::System::Memory::{VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, MEM_PRIVATE, PAGE_READWRITE};

use crate::control::Control;
use crate::erlog;
use crate::probe::{fx_record, read, u32_at};

const RECORD_FLOATS: usize = 0x0C;
const FLOATS: usize = 7;
/// Seconds after a scene's start at which the effect pool is searched.
const SCANS: [f32; 4] = [0.4, 2.5, 6.0, 12.0];

/// What a search found: (record, effect id) of the wanted ids; how many records of each id are live; milliseconds.
type Found = (Vec<(usize, u32)>, BTreeMap<u32, u32>, u128);

#[derive(Default)]
struct State {
    env: usize,
    began: Option<Instant>,
    looked: Option<Instant>,
    acts: Vec<(usize, String)>,
    listed: bool,
    /// Flag words we cleared bit 0 of in this scene (for the count in the log).
    cleared: Vec<usize>,
    scans: usize,
    zeroed: u32,
    busy: Arc<AtomicBool>,
    found: Arc<Mutex<Option<Found>>>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// WHICH SEARCH IS DUE (pure): `done` searches were started, the scene runs for `age` seconds, none is running.
pub fn scan_due(done: usize, age: f32, busy: bool) -> bool {
    !busy && SCANS.get(done).is_some_and(|at| age >= *at)
}

/// The ids to zero: the sheet's two and the key's.
pub fn wanted_ids(sheet: [i32; 2], key: Option<&str>) -> Vec<u32> {
    let mut ids: Vec<u32> = sheet.iter().filter(|i| **i > 0).map(|i| *i as u32).collect();
    for id in key.unwrap_or("").split(',').filter_map(|t| t.trim().parse::<u32>().ok()).filter(|i| *i > 0) {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}

/// The effect records of the process (a worker thread: every read is `probe::read`, which copies or refuses).
fn search(system: usize, ids: &[u32]) -> Found {
    let started = Instant::now();
    let (mut hits, mut all) = (Vec::new(), BTreeMap::new());
    let mut buf = vec![0u8; 1 << 20];
    let mut at = 0x1_0000usize;
    let mut records = 0usize;
    while at < 0x7FFF_FFFF_0000 && records < 20_000 {
        // SAFETY: VirtualQuery on any address; plain structure.
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { VirtualQuery(at as *const _, &mut info, std::mem::size_of::<MEMORY_BASIC_INFORMATION>()) } == 0 {
            break;
        }
        let (base, next) = (info.BaseAddress as usize, (info.BaseAddress as usize).saturating_add(info.RegionSize));
        if next <= at {
            break;
        }
        if info.State == MEM_COMMIT && info.Type == MEM_PRIVATE && info.Protect == PAGE_READWRITE && base >= 0x7000_0000_0000 {
            let mut p = base;
            while p < next {
                let len = (next - p).min(buf.len());
                if read(p, &mut buf[..len]) {
                    for (k, c) in buf[..len].chunks_exact(8).enumerate() {
                        if usize::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]) == system && p + k * 8 >= 0x38 {
                            let rec = p + k * 8 - 0x38;
                            if let Some(id) = fx_record(rec, system) {
                                records += 1;
                                *all.entry(id).or_insert(0u32) += 1;
                                if ids.contains(&id) {
                                    hits.push((rec, id));
                                }
                            }
                        }
                    }
                }
                p += len;
            }
        }
        at = next;
    }
    (hits, all, started.elapsed().as_millis())
}

/// ONCE A FRAME (graceflame.rs calls it; game thread). `hide`: the graces' own light is hidden in play (the save points
/// are on and `savepoint.flame` is not 1); `system`: the game's effect system; `sheet_ids`: the grace's effect ids.
pub fn frame(control: &Control, hide: bool, system: usize, sheet_ids: [i32; 2], tick: bool) {
    let Ok(mut guard) = STATE.lock() else { return };
    let state = guard.get_or_insert_with(State::default);
    let on = hide && control.flag("cut.grace") != Some(false);
    let env = if on && crate::game::remo_state().is_some_and(|s| s >= 2) { crate::figure::env().unwrap_or(0) } else { 0 };
    if env == 0 {
        if state.env != 0 {
            erlog!("cutgrace: the scene is over: {} model flag word(s) of grace models were held hidden, {} effect record(s) zeroed in {} search(es)", state.cleared.len(), state.zeroed, state.scans);
            // (the scene's models and effects go with the scene: nothing is put back, no address is kept)
            let (busy, found) = (state.busy.clone(), state.found.clone());
            *state = State { busy, found, ..State::default() };
        }
        return;
    }
    if env != state.env {
        let (busy, found) = (state.busy.clone(), state.found.clone());
        *state = State { env, began: Some(Instant::now()), busy, found, ..State::default() };
        if let Ok(mut f) = state.found.lock() {
            *f = None; // (a search of the scene before)
        }
    }
    let age = state.began.map_or(0.0, |t| t.elapsed().as_secs_f32());
    // ---- 1. the scene's grace models (the act list is read twice a second)
    if state.looked.is_none_or(|t| t.elapsed().as_millis() > 500) {
        state.looked = Some(Instant::now());
        let prefixes: Vec<String> = control.str("cut.grace.models").unwrap_or("AEG099_060").split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect();
        state.acts = crate::figure::named_acts(env, &prefixes);
        if !state.listed {
            let mut names = crate::figure::act_names(env);
            if !names.is_empty() {
                state.listed = true;
                names.sort();
                names.dedup();
                erlog!("cutgrace: A SCENE BEGINS: {} object name(s) animated; the grace's ({prefixes:?}): {:?}{} | every name: {}", names.len(), state.acts.iter().map(|(_, n)| n.as_str()).collect::<Vec<_>>(), if state.acts.is_empty() { " = NONE: this scene has no model of that name (another AEG099 name in the list is the candidate for cut.grace.models)" } else { ": HELD HIDDEN (bit 0 of the model's display flags)" }, names.iter().take(80).cloned().collect::<Vec<_>>().join(" "));
            }
        }
    }
    for (act, _) in &state.acts {
        let Some(w) = crate::figure::act_flag_word(*act) else { continue };
        let Some(now) = u32_at(w) else { continue };
        if now & 1 != 0 {
            if !state.cleared.contains(&w) && state.cleared.len() < 64 {
                state.cleared.push(w);
            }
            // SAFETY: a flag word of a live model's display entity, reached from the scene's act and read through the
            // system an instant ago (the write figure.rs makes for the player's figure); game thread.
            unsafe { std::ptr::write_volatile(w as *mut u32, now & !1) };
        }
    }
    // ---- 2. the scene's grace light: the effect pool is searched on a worker thread, the writes are made here
    let ids = wanted_ids(sheet_ids, control.str("cut.grace.sfx"));
    if control.flag("cut.grace.scan") != Some(false) && scan_due(state.scans, age, state.busy.load(Relaxed)) && !ids.is_empty() {
        state.scans += 1;
        state.busy.store(true, Relaxed);
        let (busy, found, wanted) = (state.busy.clone(), state.found.clone(), ids.clone());
        let spawned = std::thread::Builder::new().name("kh-cutgrace".into()).spawn(move || {
            let result = search(system, &wanted);
            if let Ok(mut f) = found.lock() {
                *f = Some(result);
            }
            busy.store(false, Relaxed);
        });
        if spawned.is_err() {
            state.busy.store(false, Relaxed);
        }
    }
    let result = state.found.try_lock().ok().and_then(|mut f| f.take());
    if let Some((hits, all, ms)) = result {
        let mut zeroed = 0;
        for (record, id) in &hits {
            // (the record is checked again: the search ran while the game went on)
            if fx_record(*record, system) != Some(*id) || !ids.contains(id) {
                continue;
            }
            let Some(now) = crate::probe::floats::<FLOATS>(record + RECORD_FLOATS) else { continue };
            if now.iter().any(|f| *f != 0.0) {
                for f in 0..FLOATS {
                    // SAFETY: the record was read through the system an instant ago (its system pointer and id checked,
                    // its floats just read); game thread.
                    unsafe { std::ptr::write_volatile((record + RECORD_FLOATS + 4 * f) as *mut f32, 0.0) };
                }
                zeroed += 1;
            }
        }
        state.zeroed += zeroed;
        let first = state.scans == 1;
        if first || zeroed > 0 {
            erlog!("cutgrace: search {} of the effect pool ({ms} ms, {:.1} s into the scene): {} live record(s) of {} effect id(s); the grace's ids {ids:?}: {} record(s), {zeroed} NOT YET ZERO -> set to zero size and colour{}", state.scans, age, all.values().sum::<u32>(), all.len(), hits.len(), if first { format!(" | every live effect id (id x records): {}", all.iter().map(|(id, n)| format!("{id}x{n}")).collect::<Vec<_>>().join(" ")) } else { String::new() });
        }
    }
    if tick && control.on("cut.grace.log") {
        erlog!("cutgrace: {:.1} s into the scene | {} grace act(s), {} flag word(s) cleared | {} search(es), {} record(s) zeroed", age, state.acts.len(), state.cleared.len(), state.scans, state.zeroed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pool_is_searched_four_times_in_a_scene_and_never_twice_at_once() {
        assert!(!scan_due(0, 0.1, false));
        assert!(scan_due(0, 0.5, false));
        assert!(!scan_due(0, 0.5, true), "a search is running");
        assert!(!scan_due(1, 1.0, false));
        assert!(scan_due(1, 2.6, false));
        assert!(scan_due(3, 30.0, false));
        assert!(!scan_due(4, 300.0, false), "four a scene");
    }

    #[test]
    fn the_ids_are_the_sheets_two_and_the_keys() {
        assert_eq!(wanted_ids([1010, 1011], None), [1010, 1011]);
        assert_eq!(wanted_ids([1010, 1011], Some("1012, 1011,x, 6400")), [1010, 1011, 1012, 6400]);
        assert_eq!(wanted_ids([0, -1], Some("")), Vec::<u32>::new());
    }
}
