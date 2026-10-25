//! (savegate) SAVING ONLY AT SAVE POINTS (`save.points_only`) AND RELOADING THE LAST SAVE
//! (build/sora_tmp/er/IMPL_ER10_STATE.md).
//!
//! WHAT THE GAME DOES (eldenring.exe 2.7.1.0, read from the file, RVAs):
//! - Everything that wants a save sets `GameMan + 0xB72` (`save_requested`) and / or `+ 0xB73` (0x67b1f0,
//!   0x67b270, 0x67b370, 0x6808d3: item pick-ups, kills, area changes, timers, the rest at a grace ...).
//! - One step (0xafcba0) looks at the requests every frame and, when no save or load is running
//!   (`GameMan + 0xB80`, `save_state`, is 0), calls one of THE FOUR FUNCTIONS THAT START A SAVE: each
//!   serialises the game's data into a buffer and hands it to the writer task, sets `save_state = 1` and
//!   returns true; each returns false without having done anything when a save is already running:
//!   S1 0x67c3c0 (the character's slot), S2 0x67c5a0 (slot + profile), S3 0x67c790 (both), S4 0x67ca60 (the
//!   system part). The menu steps of "Quit Game" call S1 / S2 / S4 directly (0x82ca80, 0x82cac1, 0x82caf9).
//! - The same step handles `GameMan + 0xB78` (`requested_save_slot_load_index`, -1 = none): with a slot
//!   number there it starts THE GAME'S OWN LOAD of that slot (0x67c050: the save file is read again,
//!   `save_state = 2`), and when the read is done applies it (0x67c0e0). Nothing in the executable writes a
//!   slot number there any more: the request is ours to make.
//!
//! WHAT WE DO:
//! - `save.points_only = 1`: the four start functions are detoured; a call returns false at once (the path
//!   every caller already has for "a save is running") unless the save point's Save opened the gate
//!   (`request_save`). A save is therefore all or nothing: the game's whole routine runs, or none of it.
//!   The pending request bytes are cleared with a refused call, so nothing piles up for the moment the
//!   gate opens.
//! - `request_save`: gate open for 20 s + `save_requested = 1`: the game's own step saves; `poll_saved`
//!   says when the file was written (`save_state` back to 0).
//! - `request_load`: `requested_save_slot_load_index = save_slot`.
//!
//! The hooks are inline hooks of the renderer crate: found by pattern, used only at their 2.7.1.0
//! addresses, removed with every other one at the payload's shutdown.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::SystemTime;

use windows_sys::Win32::System::SystemInformation::GetTickCount64;

use crate::control::{Control, OneShot};
use crate::erlog;
use crate::havok::{locate, Located};

pub(crate) const STARTS: [Located; 4] = [
    Located { name: "save start S1 (slot)", pattern: "48 8b c4 57 48 81 ec a0 00 00 00 48 c7 44 24 20 fe ff ff ff 48 89 58 08 48 89 68 10 48 89 70 18 41 0f b6 f0 0f b6 da 0f b6 e9", capture: 0, rva_2_7_1_0: 0x0067_c3c0, rva_2_7_2_0: 0x0067_c870 /* (PORT) */ },
    Located { name: "save start S2 (slot + profile)", pattern: "48 89 5c 24 20 57 41 54 41 57 48 83 ec 30 45 0f b6 f8 44 0f b6 e2 8b f9 32 db e8 ? ? ? ? 84 c0 48 8b 05", capture: 0, rva_2_7_1_0: 0x0067_c5a0, rva_2_7_2_0: 0x0067_ca50 /* (PORT) */ },
    Located { name: "save start S3 (both)", pattern: "48 8b c4 44 88 40 18 57 41 54 41 55 41 56 41 57 48 81 ec b0 00 00 00 48 c7 44 24 30 fe ff ff ff", capture: 0, rva_2_7_1_0: 0x0067_c790, rva_2_7_2_0: 0x0067_cc40 /* (PORT) */ },
    Located { name: "save start S4 (system)", pattern: "48 8b c4 57 48 81 ec 90 00 00 00 48 c7 40 88 fe ff ff ff 48 89 58 08 48 89 70 10 40 32 ff 48 8b 05 ? ? ? ? 83 b8 80 0b 00 00 00", capture: 0, rva_2_7_1_0: 0x0067_ca60, rva_2_7_2_0: 0x0067_cf10 /* (PORT) */ },
];

/// Offsets in `GameMan` (the SDK's fields `save_requested`, `requested_save_slot_load_index`, `save_state`, `save_slot`).
const SAVE_REQUESTED: usize = 0xB72;
const SAVE_REQUESTED_2: usize = 0xB73;
const LOAD_SLOT: usize = 0xB78;
const SAVE_STATE: usize = 0xB80;
const SAVE_SLOT: usize = 0xAC0;
/// The quit's phase (0 none, 1 a save is wanted before leaving, 2 it is under way, 3 it is done: leave).
const QUIT_PHASE: usize = 0xBC4;

static ORIGINAL: [AtomicUsize; 4] = [AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0)];
/// `save.points_only`: the game's saves are refused unless the gate is open.
static BLOCK: AtomicBool = AtomicBool::new(false);
/// The gate is open until this tick (ms); closed by the first save that starts.
static OPEN_UNTIL: AtomicU64 = AtomicU64::new(0);
static CALLS: [AtomicU32; 4] = [AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)];
static REFUSED: [AtomicU32; 4] = [AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)];
static STARTED: [AtomicU32; 4] = [AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)];
/// A save we let through started and has not been reported as written yet.
static OURS_RUNNING: AtomicBool = AtomicBool::new(false);
/// Tick at which the latest save we let through started (the file watcher's wording: its write shows up a moment later).
static OURS_AT: AtomicU64 = AtomicU64::new(0);

type StartFn = unsafe extern "C" fn(u64, u64, u64, u64) -> u8;

fn game_man() -> Option<*mut u8> {
    use eldenring::cs::GameMan;
    use fromsoftware_shared::FromStatic;
    // SAFETY: the singleton lives as long as the game; only its address is taken.
    unsafe { GameMan::instance().ok().map(|g| g as *const GameMan as *mut u8) }.filter(|p| crate::game::plausible(*p as usize))
}

/// (nothing in here can panic: loads, stores, byte writes into the game manager, one log line)
unsafe fn start_hook(k: usize, a: u64, b: u64, c: u64, d: u64) -> u8 {
    let original: StartFn = std::mem::transmute(ORIGINAL[k].load(Ordering::Relaxed));
    CALLS[k].fetch_add(1, Ordering::Relaxed);
    let open = GetTickCount64() < OPEN_UNTIL.load(Ordering::Relaxed);
    // (IMPL_ER15) while the title's Load Game list is made with all slots but one hidden IN MEMORY (titleload.rs),
    // NOTHING is saved: the profile summary must never reach the file in that state.
    if crate::titleload::masked() {
        erlog!("savegate: a save start ({}) while the profile summary is masked for the title's load: REFUSED", STARTS[k].name);
        return 0;
    }
    // (IMPL_ER11) THE SYSTEM PART (S4: settings, the profile list; the title screen writes it) IS NEVER REFUSED: MEASURED:
    // refused or answered without being done at the title, the game reports "Failed to save game. Save data is
    // corrupted." It holds nothing of the character's progress.
    // (IMPL_ER21) ... nor on the way to the title screen we asked for (Title Screen of our menus: no save, whatever
    // the character's state), nor while a new character's creation save is ours to make (CREATION 2: only the "both"
    // save we asked for goes through, by the open gate)
    if BLOCK.load(Ordering::Relaxed) && (ARMED.load(Ordering::Relaxed) || LEAVING.load(Ordering::Relaxed)) && !open && k != 3 {
        if let Some(g) = game_man() {
            *g.add(SAVE_REQUESTED) = 0;
            *g.add(SAVE_REQUESTED_2) = 0;
        }
        // (IMPL_ER11) ELDEN RING'S OWN "QUIT GAME" / "RETURN TO DESKTOP" (read in the executable, MEASURED in the game):
        // the quit (RVA 0x67b244) sets both save requests and `GameMan + 0xBC4 = 1` ("quitting: a save is wanted"); a save
        // start that really starts moves it to 2 (0x67c6f8 / 0x67c9f8), the finished save to 3, and only at 3 does the
        // quit go on (0x67a1ee / 0x67a260). With the save refused it stayed at 1 for ever: the menu did nothing.
        // THE QUIT MUST WORK WITHOUT SAVING: a refused start while the phase is 1 puts it at 3 ("saved") and answers
        // true: the game leaves, nothing was written. (The menu job's direct calls, RVA 0x82ca70.., are answered the same.)
        if QUIT_PASS.load(Ordering::Relaxed) {
            let phase = game_man().map(|g| std::ptr::read_unaligned(g.add(QUIT_PHASE) as *const u32));
            if phase == Some(1) {
                if let (Some(1), Some(g)) = (phase, game_man()) {
                    std::ptr::write_unaligned(g.add(QUIT_PHASE) as *mut u32, 3);
                }
                if QUIT_TOLD.fetch_add(1, Ordering::Relaxed) < 12 {
                    erlog!("savegate: Elden Ring's QUIT wanted to save ({}; quit phase {:?}): answered \"saved\" WITHOUT SAVING ANYTHING (save.points_only: the quit goes on, the last save point's save stays)", STARTS[k].name, phase);
                }
                QUITTING.store(GetTickCount64(), Ordering::Relaxed);
                return 1;
            }
        }
        if REFUSED[k].fetch_add(1, Ordering::Relaxed) < 40 {
            erlog!("savegate: the game wanted to SAVE ({}; arguments {:#x}, {:#x}, {:#x}): REFUSED (save.points_only: only the save point's Save writes)", STARTS[k].name, a as u32, b as u8, c as u8);
        }
        return 0;
    }
    // (IMPL_ER13) WHAT THE SAVE HOLDS IN ITS RUNE COUNTER IS SORA'S MUNNY: the mirror shows 0 in the counter while he rests
    // (the save point's Save is made seated) or is dead; the start function serialises the game's data inside this call,
    // so the counter is the munny for exactly that long. Sora's own file and the game's slot then say the same number.
    let fixed = if k != 3 { munny_into_counter() } else { None };
    let done = original(a, b, c, d);
    if let Some((counter, before)) = fixed {
        *counter = before;
    }
    if done != 0 {
        if fixed.is_some() {
            MUNNY_FIXED.fetch_add(1, Ordering::Relaxed);
        }
        STARTED[k].fetch_add(1, Ordering::Relaxed);
        // (IMPL_ER21) the creation save we asked for has started: slot data + profile summary are being written
        if open && k == 2 && CREATION.load(Ordering::Relaxed) == 2 {
            CREATION.store(3, Ordering::Relaxed);
        }
        if open {
            OURS_RUNNING.store(true, Ordering::Relaxed);
            OURS_AT.store(GetTickCount64(), Ordering::Relaxed);
            // (the system part alone is not the character's save: the gate stays open for the slot's)
            if k != 3 {
                OPEN_UNTIL.store(0, Ordering::Relaxed);
            }
        }
        erlog!("savegate: the game STARTS A SAVE ({}; arguments {:#x}, {:#x}, {:#x}){}", STARTS[k].name, a as u32, b as u8, c as u8, if open { ": asked for by the save point" } else { "" });
    }
    done
}

/// (IMPL_ER13) Sora's munny as the wallet last published it (prize.rs, every frame of the mirror), and when (ms tick).
static SAVE_MUNNY: AtomicU64 = AtomicU64::new(u64::MAX);
static SAVE_MUNNY_AT: AtomicU64 = AtomicU64::new(0);
static MUNNY_FIXED: AtomicU32 = AtomicU32::new(0);

/// (prize) The munny the game's saves must hold in the rune counter (`None`: no mirror / no Sora: the counter is left).
pub fn publish_munny(munny: Option<i32>) {
    SAVE_MUNNY.store(munny.map_or(u64::MAX, |m| m.max(0) as u64), Ordering::Relaxed);
    // SAFETY: plain getter.
    SAVE_MUNNY_AT.store(unsafe { GetTickCount64() }, Ordering::Relaxed);
}

/// The live rune counter of the main player's game data (checked pointers; nothing that can panic).
unsafe fn rune_counter() -> Option<*mut u32> {
    use eldenring::cs::{GameDataMan, PlayerGameData};
    use fromsoftware_shared::FromStatic;
    let data = GameDataMan::instance().ok()?;
    let player = crate::word!(data.main_player_game_data);
    if !crate::game::plausible(player) {
        return None;
    }
    Some(std::ptr::addr_of_mut!((*(player as *mut PlayerGameData)).rune_count))
}

/// The counter is set to the published munny (when that is fresh: the wallet ran within the last 2 s, and the
/// counter differs); returns what to put back.
unsafe fn munny_into_counter() -> Option<(*mut u32, u32)> {
    let munny = SAVE_MUNNY.load(Ordering::Relaxed);
    if munny == u64::MAX || GetTickCount64().saturating_sub(SAVE_MUNNY_AT.load(Ordering::Relaxed)) > 2000 {
        return None;
    }
    let counter = rune_counter()?;
    let before = *counter;
    let want = munny.min(999_999_999) as u32;
    if before == want {
        return None;
    }
    *counter = want;
    Some((counter, before))
}

unsafe extern "C" fn hook0(a: u64, b: u64, c: u64, d: u64) -> u8 {
    start_hook(0, a, b, c, d)
}
unsafe extern "C" fn hook1(a: u64, b: u64, c: u64, d: u64) -> u8 {
    start_hook(1, a, b, c, d)
}
unsafe extern "C" fn hook2(a: u64, b: u64, c: u64, d: u64) -> u8 {
    start_hook(2, a, b, c, d)
}
unsafe extern "C" fn hook3(a: u64, b: u64, c: u64, d: u64) -> u8 {
    start_hook(3, a, b, c, d)
}

static HOOKED: AtomicBool = AtomicBool::new(false);
/// (IMPL_ER11) `save.quit` is not `refuse`: the quit menu's save calls are answered true without saving.
static QUIT_PASS: AtomicBool = AtomicBool::new(true);
static QUIT_TOLD: AtomicU32 = AtomicU32::new(0);
/// Tick of the latest quit-menu save call that was answered without saving (0: none).
pub static QUITTING: AtomicU64 = AtomicU64::new(0);
/// The quit menu's job steps that call the save starts (2.7.1.0; the starts themselves are RVA-gated).
const QUIT_STEPS: std::ops::Range<usize> = 0x0082_ca70..0x0082_cb20;

/// A save start was called from the menu's job steps (the return addresses on the stack). NOT USED for the rule any
/// more: the title screen's own system save comes through these steps too (MEASURED).
#[allow(dead_code)]
fn from_quit_menu() -> bool {
    // (PORT) the steps' range is recorded for 2.7.1.0 only (unused code; 2.7.2.0 start 0x82cf20 is known, its end is not)
    if crate::gamever::current() != Some(crate::gamever::ExeVersion::V2710) {
        return false;
    }
    use windows_sys::Win32::System::Diagnostics::Debug::RtlCaptureStackBackTrace;
    let mut frames = [std::ptr::null_mut::<core::ffi::c_void>(); 8];
    // SAFETY: the buffer holds 8 entries.
    let n = unsafe { RtlCaptureStackBackTrace(0, 8, frames.as_mut_ptr(), std::ptr::null_mut()) } as usize;
    let base = crate::entry::exe_base();
    frames[..n.min(8)].iter().any(|f| QUIT_STEPS.contains(&(*f as usize).wrapping_sub(base)))
}

/// Detours the four save-start functions (all four or none is used).
pub fn install() -> bool {
    crate::savepath::install(); // (SAVEPATH) the save file the game really opens, whatever its name
    let found: Vec<Option<usize>> = STARTS.iter().map(locate).collect();
    if found.iter().any(Option::is_none) {
        erlog!("savegate: not every save-start function was found for this executable (PORT: 2.7.1.0 / 2.7.2.0 table): the game's saves CANNOT be held back (save.points_only has no effect)");
        return false;
    }
    let hooks: [usize; 4] = [hook0 as *const () as usize, hook1 as *const () as usize, hook2 as *const () as usize, hook3 as *const () as usize];
    let mut all = true;
    for k in 0..4 {
        // SAFETY: a function's start found by pattern at the recorded address; the hook passes the four register arguments on unchanged.
        if let Err(e) = unsafe { kh_er_render::inline_hook(found[k].unwrap(), hooks[k], &ORIGINAL[k], STARTS[k].name) } {
            ORIGINAL[k].store(0, Ordering::SeqCst);
            erlog!("savegate: {} was NOT detoured ({e})", STARTS[k].name);
            all = false;
        }
    }
    HOOKED.store(all, Ordering::SeqCst);
    erlog!("savegate: the game's four save-start functions are {}", if all { "detoured (its saves can be held back: save.points_only)" } else { "NOT all detoured: save.points_only has no effect" });
    all
}

pub fn uninstalled() {
    crate::savepath::uninstalled(); // (SAVEPATH)
    HOOKED.store(false, Ordering::SeqCst);
    BLOCK.store(false, Ordering::SeqCst);
    for o in &ORIGINAL {
        o.store(0, Ordering::SeqCst);
    }
}

/// THE MODE IS ARMED PER CHARACTER: a character's saving belongs to the save points FROM ITS FIRST SAVE AT ONE
/// (the marker file `<saves>\<character>.points` beside Sora's save, made by that Save). Until then the game
/// saves by itself as it always does: a new character's opening (creation, the tutorial's scripted death, its
/// cutscenes and warps) is saved by the game, or the character could never persist. True at a payload's
/// start (nothing is written before Sora's start has looked for the marker).
static ARMED: AtomicBool = AtomicBool::new(false);

pub fn set_armed(on: bool, why: &str) {
    if ARMED.swap(on, Ordering::Relaxed) != on || !ARMED_TOLD.swap(true, Ordering::Relaxed) {
        erlog!("savegate: this character's saving {} ({why})", if on { "BELONGS TO THE SAVE POINTS: the game's own saves are refused" } else { "is still the game's own (no Save at a save point yet): nothing is refused" });
    }
}
static ARMED_TOLD: AtomicBool = AtomicBool::new(false);

/// (IMPL_ER21) `save.points_only = 1` IS ABSOLUTE (the user, 2026-10-13: "ONLY possible to save at save points"):
/// every character's saving belongs to the save points from the moment Sora plays it; `armed` = the rule before
/// (a character is armed by its first Save at a save point).
static ABSOLUTE: AtomicBool = AtomicBool::new(false);
/// (IMPL_ER21) A NEW CHARACTER'S CREATION SAVE: 0 = nothing to do, 1 = this character has never been written under
/// our rule (no marker, no Sora file, under 5 minutes of play): the game saves by itself until WE have made one
/// whole save (slot data + profile summary, the kind the Save screen makes) the first time Sora stands in the world;
/// 2 = that save is asked for, 3 = it started, then the marker `<key>.created` is made and the character is armed.
static CREATION: AtomicU32 = AtomicU32::new(0);
static CREATION_TRIES: AtomicU32 = AtomicU32::new(0);
/// (IMPL_ER21) The title screen was asked for by our menus: every save of the way out is refused.
static LEAVING: AtomicBool = AtomicBool::new(false);

pub fn absolute() -> bool {
    mode_on() && ABSOLUTE.load(Ordering::Relaxed)
}

/// (IMPL_ER21, save.rs at Sora's start) This character still needs its creation save (see `CREATION`).
pub fn set_creation_pending(pending: bool) {
    let now = if pending { 1 } else { 0 };
    if CREATION.load(Ordering::Relaxed) != now && (pending || CREATION.load(Ordering::Relaxed) == 1) {
        CREATION.store(now, Ordering::Relaxed);
        CREATION_TRIES.store(0, Ordering::Relaxed);
    }
}

/// `save.points_only` is on and the hooks are in (whether or not this character is armed yet).
pub fn mode_on() -> bool {
    BLOCK.load(Ordering::Relaxed) && HOOKED.load(Ordering::Relaxed)
}

/// `save.points_only` is in force for this character: the game's own saves are refused.
pub fn points_only() -> bool {
    mode_on() && ARMED.load(Ordering::Relaxed)
}

/// The game's current save slot (`GameMan::save_slot`: where its save routine writes when it is given -1), 0..=9.
pub fn current_slot() -> Option<i32> {
    // SAFETY: an i32 of the live game manager.
    game_man().map(|g| unsafe { std::ptr::read_unaligned(g.add(SAVE_SLOT) as *const i32) }).filter(|s| (0..=9).contains(s))
}

/// A save or a load of the game's is under way, or one is asked for and not taken yet.
pub fn busy() -> bool {
    let Some(g) = game_man() else { return true };
    // SAFETY: fields of the live game manager.
    unsafe { std::ptr::read_unaligned(g.add(SAVE_STATE) as *const u32) != 0 || std::ptr::read_unaligned(g.add(LOAD_SLOT) as *const i32) != -1 || OURS_RUNNING.load(Ordering::Relaxed) }
}

/// (IMPL_ER13) The game's current save slot is written (a slot save that failed puts the old one back).
pub fn set_slot(slot: i32) -> bool {
    let (Some(g), true) = (game_man(), (0..=9).contains(&slot)) else { return false };
    // SAFETY: an i32 of the live game manager.
    unsafe { std::ptr::write_unaligned(g.add(SAVE_SLOT) as *mut i32, slot) };
    true
}

/// (IMPL_ER13) THE CURRENT SESSION IS SAVED INTO SLOT `slot` BY THE GAME'S OWN ROUTINE (MEASURED in IMPL_ER12: its save
/// starts write to `GameMan + 0xAC0` when given -1): the slot is set, the gate opens, and BOTH requests are set
/// (`+0xB72` and `+0xB73` -> the "both" start S3), because only that kind also writes the PROFILE SUMMARY (the list
/// the title's Load Game shows): a slot that was empty is listed at once. `take_saved` tells what came of it.
pub fn request_save_slot(slot: i32, why: &str) -> bool {
    let Some(g) = game_man() else { return false };
    if !(0..=9).contains(&slot) || busy() {
        erlog!("savegate: the save into slot {slot} was NOT asked for ({why}): the game is saving / loading, or no such slot");
        return false;
    }
    // SAFETY: fields of the live game manager.
    unsafe {
        let before = std::ptr::read_unaligned(g.add(SAVE_SLOT) as *const i32);
        std::ptr::write_unaligned(g.add(SAVE_SLOT) as *mut i32, slot);
        OPEN_UNTIL.store(GetTickCount64() + 20_000, Ordering::Relaxed);
        *g.add(SAVE_REQUESTED) = 1;
        *g.add(SAVE_REQUESTED_2) = 1;
        erlog!("savegate: SAVE INTO SLOT {slot} ASKED OF THE GAME ({why}; its current slot was {before}): both requests set (slot data + profile summary), the gate is open");
    }
    if let Ok(mut w) = WATCH.lock() {
        w.asked_at = Some(std::time::Instant::now());
        w.file_before = w.path.as_ref().and_then(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    }
    true
}

/// The game's save file (only ever read by us). (SAVEPATH) Only a DETECTED one (savepath.rs: the file the game opened,
/// `save.file`, ...): a name merely guessed is not given, so the slot lists fall back to the game's summary in memory.
pub fn save_file_path() -> Option<PathBuf> {
    WATCH.lock().ok().filter(|w| w.detected).and_then(|w| w.path.clone())
}

/// The save point's Save: the gate opens and the game is asked to save now.
pub fn request_save(why: &str) -> bool {
    let Some(g) = game_man() else { return false };
    // SAFETY: plain getter; a byte of the live game manager (what every request of the game's own sets).
    unsafe {
        OPEN_UNTIL.store(GetTickCount64() + 20_000, Ordering::Relaxed);
        *g.add(SAVE_REQUESTED) = 1;
    }
    if let Ok(mut w) = WATCH.lock() {
        w.asked_at = Some(std::time::Instant::now());
        w.file_before = w.path.as_ref().and_then(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    }
    erlog!("savegate: SAVE ASKED OF THE GAME ({why}): the gate is open for its next save");
    true
}

/// What became of the save `request_save` asked for, once: true = the game has written it, false = nothing came of it.
pub fn take_saved() -> Option<bool> {
    match RESULT.swap(0, Ordering::Relaxed) {
        1 => Some(true),
        2 => Some(false),
        _ => None,
    }
}
static RESULT: AtomicU32 = AtomicU32::new(0);

/// What became of the save `request_save` asked for: `Some(true)` once when the game has written it,
/// `Some(false)` once when nothing came of it in 20 s.
fn poll_saved(w: &mut Watch) -> Option<bool> {
    let asked = w.asked_at?;
    let state = save_state().unwrap_or(0);
    if OURS_RUNNING.load(Ordering::Relaxed) && state == 0 {
        OURS_RUNNING.store(false, Ordering::Relaxed);
        w.asked_at = None;
        let now = w.path.as_ref().and_then(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
        erlog!("savegate: THE GAME'S SAVE IS WRITTEN ({:.2} s after it was asked for; the save file's time {})", asked.elapsed().as_secs_f32(), if now != w.file_before { "CHANGED" } else { "did NOT change" });
        return Some(true);
    }
    if asked.elapsed().as_secs_f32() > 20.0 {
        w.asked_at = None;
        OURS_RUNNING.store(false, Ordering::Relaxed);
        OPEN_UNTIL.store(0, Ordering::Relaxed);
        erlog!("savegate: THE GAME DID NOT SAVE within 20 s of being asked (save state {state})");
        return Some(false);
    }
    None
}

fn save_state() -> Option<u32> {
    // SAFETY: a u32 of the live game manager.
    game_man().map(|g| unsafe { std::ptr::read_unaligned(g.add(SAVE_STATE) as *const u32) })
}

/// THE GAME'S OWN LOAD of the current save slot is asked for (the save file is read again; the world and
/// the character become what the last save holds). False: no slot / a save or load is running.
pub fn request_load(why: &str) -> bool {
    // SAFETY: an i32 of the live game manager.
    let Some(slot) = game_man().map(|g| unsafe { std::ptr::read_unaligned(g.add(SAVE_SLOT) as *const i32) }) else { return false };
    request_load_of(slot, why)
}

/// (IMPL_ER13) THE GAME'S OWN LOAD OF ANY SLOT (`GameMan + 0xB78 = slot`; MEASURED in IMPL_ER12: slots other than the
/// current one load the same way, 4 - 5 s, and the game's current slot is the loaded one afterwards). Needs a world.
pub fn request_load_of(slot: i32, why: &str) -> bool {
    let Some(g) = game_man() else { return false };
    // SAFETY: i32 / u32 fields of the live game manager.
    unsafe {
        let state = std::ptr::read_unaligned(g.add(SAVE_STATE) as *const u32);
        if !(0..=9).contains(&slot) || state != 0 {
            erlog!("savegate: the load was NOT asked for ({why}): save slot {slot}, save state {state}");
            return false;
        }
        // (no pending save request may win over the load in the game's step)
        *g.add(SAVE_REQUESTED) = 0;
        *g.add(SAVE_REQUESTED_2) = 0;
        std::ptr::write_unaligned(g.add(LOAD_SLOT) as *mut i32, slot);
        erlog!("savegate: LOAD OF SAVE SLOT {slot} ASKED OF THE GAME ({why})");
    }
    true
}

// ---------------------------------------------------------------- the last save again

/// Tick (ms) at which the last save was asked to be loaded again (0: not asked).
static RELOAD_AT: AtomicU64 = AtomicU64::new(0);
/// The game killed its player by itself (`save.points_only`): the last save is loaded when its respawn is done.
static DEATH_RELOAD: AtomicBool = AtomicBool::new(false);
pub static RELOADS: AtomicU32 = AtomicU32::new(0);

/// CONTINUE: the last save point's save is loaded again by the game's own load (`request_load`); this
/// session's state of Sora and of the wallet is dropped so that both come from the files of that moment.
/// False without `save.points_only` or when the game cannot be asked now.
pub fn reload(why: &str) -> bool {
    if !points_only() {
        return false;
    }
    let Some(slot) = current_slot() else { return false };
    load_slot(slot, None, why)
}

/// (IMPL_ER13) EVERY IN-SESSION LOAD GOES THROUGH HERE (Continue, a death of the game's own, the load of another
/// slot, the testing aids): the game's load of `slot` is asked for, and NOTHING OF THIS SESSION SURVIVES IT:
/// Sora's state and the wallet are dropped at the request (the still-running Sora of the old session no longer
/// reads or writes the rune counter: IMPL_ER12's caveat 2), the next start of Sora reads the TARGET slot's
/// `.khsave`; `armed` (when known) = that slot's save-points marker, in force from now (the game saves by
/// itself at every load: that save is the target slot's to refuse or allow).
pub fn load_slot(slot: i32, armed: Option<bool>, why: &str) -> bool {
    if !request_load_of(slot, why) {
        return false;
    }
    crate::save::forget_session();
    crate::prize::forget_session();
    crate::hud::on_session_load(); // (IMPL_ER15) the old session's choice menu must not meet the loaded character
    if let Some(armed) = armed {
        set_armed(armed, if armed { "the slot being loaded has its marker file" } else { "the slot being loaded has no marker file" });
    }
    // SAFETY: plain getter.
    RELOAD_AT.store(unsafe { GetTickCount64() }, Ordering::Relaxed);
    RELOADS.fetch_add(1, Ordering::Relaxed);
    true
}

/// Milliseconds since the latest in-session load was asked for (`None`: never in this run).
pub fn load_asked_ago() -> Option<u64> {
    let at = RELOAD_AT.load(Ordering::Relaxed);
    // SAFETY: plain getter.
    (at != 0).then(|| unsafe { GetTickCount64() }.saturating_sub(at))
}

/// (frame.rs) The game killed its player itself (a pit, deep water, a blow of his whole HP): with
/// `save.points_only` the last save is loaded again once the game lets it (asked for each second until taken).
pub fn death_of_the_games() {
    if points_only() {
        DEATH_RELOAD.store(true, Ordering::Relaxed);
        erlog!("savegate: the game killed its player: its saves stay refused; THE LAST SAVE IS LOADED AGAIN as soon as the game takes the request");
    }
}

/// ELDEN RING'S TITLE SCREEN WITHOUT SAVING (the game-over menu's Title Screen; `dbg.title`): the game's own
/// "return to title" request byte in its menu data (`CSMenuMan -> menu_data + 0x5D`, what the community's
/// practice tools write for a quit-out); the saves of the way out are refused by the gate.
pub fn quit_to_title(why: &str) -> bool {
    use eldenring::cs::CSMenuManImp;
    use fromsoftware_shared::FromStatic;
    // SAFETY: game thread; the singleton lives as long as the game; the pointer is checked.
    let Ok(man) = (unsafe { CSMenuManImp::instance() }) else { return false };
    let data = unsafe { std::ptr::read_unaligned((man as *const CSMenuManImp as *const usize).add(1)) };
    if !crate::game::plausible(data) {
        return false;
    }
    // SAFETY: a byte of the live menu data object (0xF0 bytes).
    unsafe { *((data + 0x5D) as *mut u8) = 1 };
    // (IMPL_ER21) nothing is saved on the way out whenever the gate exists (armed or not)
    LEAVING.store(mode_on(), Ordering::Relaxed);
    erlog!("savegate: ELDEN RING'S TITLE SCREEN ASKED FOR ({why}); nothing is saved on the way{}", if mode_on() { "" } else { " ONLY IF save.points_only is on (it is off: the game saves as it leaves)" });
    true
}

// ---------------------------------------------------------------- the watch (facts for the log)

struct Watch {
    path: Option<PathBuf>,
    /// (SAVEPATH) `path` is the game's own file (detected), not the fallback name.
    detected: bool,
    stamp: Option<SystemTime>,
    last: [u32; 6],
    since_file: f32,
    asked_at: Option<std::time::Instant>,
    file_before: Option<SystemTime>,
    save_shot: Option<OneShot>,
    load_shot: Option<OneShot>,
    title_shot: Option<OneShot>,
    remo_shot: Option<OneShot>,
    kill_shot: Option<OneShot>,
    death_try: f32,
    told_block: Option<bool>,
}

static WATCH: Mutex<Watch> = Mutex::new(Watch { path: None, detected: false, stamp: None, last: [0; 6], since_file: 0.0, asked_at: None, file_before: None, save_shot: None, load_shot: None, title_shot: None, remo_shot: None, kill_shot: None, death_try: 0.0, told_block: None });

// (SAVEPATH) the save file's name is no longer assumed: savepath.rs detects the file the game uses (`refresh` in `frame`).

fn stamp_text(t: Option<SystemTime>) -> String {
    match t.and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok()) {
        Some(d) => {
            let s = d.as_secs();
            format!("{:02}:{:02}:{:02} UTC", s / 3600 % 24, s / 60 % 60, s % 60)
        }
        None => "unknown".to_string(),
    }
}

/// Once a frame (FRAME_BEGIN, also at the title and under loading screens): the switch, the testing aids
/// (`dbg.save`, `dbg.load`: one-shots), and the log of what the game's save machinery does.
pub fn frame(control: &Control, dt: f32) {
    // (IMPL_ER21) `save.points_only = 1` (always, the default) | `armed` (per character from its first Save) | `0`
    let mode = control.str("save.points_only").unwrap_or("0");
    let want = mode == "armed" || control.on("save.points_only");
    BLOCK.store(want, Ordering::Relaxed);
    ABSOLUTE.store(want && mode != "armed", Ordering::Relaxed);
    creation_step(control);
    QUIT_PASS.store(control.str("save.quit") != Some("refuse"), Ordering::Relaxed);
    let Ok(mut w) = WATCH.lock() else { return };
    if w.told_block != Some(want) {
        w.told_block = Some(want);
        erlog!("savegate: save.points_only is {}", if !want { "off: the game saves by itself as it always does" } else if !HOOKED.load(Ordering::Relaxed) { "asked for but the hooks are not in: THE GAME SAVES BY ITSELF" } else if mode == "armed" { "ON (armed): a character's own saves are refused from its first Save at a save point" } else { "ON (always): the game never saves by itself; only the save point's Save writes (a new character's creation save is made once)" });
    }
    if w.save_shot.is_none() {
        w.save_shot = Some(OneShot::new("dbg.save", control));
        w.load_shot = Some(OneShot::new("dbg.load", control));
        w.title_shot = Some(OneShot::new("dbg.title", control));
        w.remo_shot = Some(OneShot::new("dbg.remo", control));
    }
    // (SAVEPATH) the file the game really uses (detected; `save.file` = an explicit override; else KHSora.sl2 by name)
    if let Some(choice) = crate::savepath::refresh(control) {
        w.detected = choice.detected();
        w.path = choice.path;
        w.stamp = w.path.as_ref().and_then(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
        erlog!("savegate: the game's save file of the mod character: {} (written {}; {})", w.path.as_ref().map_or("NOT FOUND".to_string(), |p| p.display().to_string()), stamp_text(w.stamp), choice.method.text());
    }
    let fire_save = w.save_shot.as_mut().is_some_and(|s| s.fired(control).is_some());
    let fire_load = w.load_shot.as_mut().is_some_and(|s| s.fired(control).is_some());
    if w.remo_shot.as_mut().is_some_and(|s| s.fired(control).is_some()) {
        crate::game::dump_cutscene_facts();
        crate::remo::dump(crate::game::player_mut().map(|p| crate::game::v3(&p.chr_ins.modules.physics.position)));
    }
    let fire_title = w.title_shot.as_mut().is_some_and(|s| s.fired(control).is_some());
    // (testing aid, `dbg.kill`) THE GAME'S OWN DEATH of its player, as a fall or a pit gives it: his HP is set to 0
    // behind Sora's back (Sora did not die: nothing was "sent")
    if w.kill_shot.get_or_insert_with(|| OneShot::new("dbg.kill", control)).fired(control).is_some() {
        if let Some(player) = crate::game::player_mut() {
            player.chr_ins.modules.data.hp = 0;
            erlog!("savegate: dbg.kill: the Tarnished's HP is set to 0 (a death of the game's own)");
        }
    }
    // (IMPL_ER12, THE SAVE-SLOT EXPERIMENT; testing aids) `dbg.saveslot = N`: the game's CURRENT save slot
    // (`GameMan::save_slot`, what its save routine writes to when it is given -1) becomes N, and Sora's file key with it;
    // `dbg.loadslot = N`: the game's own load of slot N is asked for.
    {
        static SHOTS: Mutex<Option<(OneShot, OneShot)>> = Mutex::new(None);
        if let Ok(mut shots) = SHOTS.lock() {
            let (to, load) = shots.get_or_insert_with(|| (OneShot::new("dbg.saveslot", control), OneShot::new("dbg.loadslot", control)));
            if let (Some(slot), Some(g)) = (to.fired(control).and_then(|v| v.trim().parse::<i32>().ok()).filter(|s| (0..=9).contains(s)), game_man()) {
                // SAFETY: an i32 of the live game manager.
                let before = unsafe { std::ptr::read_unaligned(g.add(SAVE_SLOT) as *const i32) };
                unsafe { std::ptr::write_unaligned(g.add(SAVE_SLOT) as *mut i32, slot) };
                erlog!("savegate: dbg.saveslot: THE GAME'S CURRENT SAVE SLOT {before} -> {slot}");
                crate::save::retarget_slot();
            }
            if let (Some(slot), Some(g)) = (load.fired(control).and_then(|v| v.trim().parse::<i32>().ok()).filter(|s| (0..=9).contains(s)), game_man()) {
                let _ = g;
                let asked = crate::slots::load_slot(slot as usize);
                erlog!("savegate: dbg.loadslot: LOAD OF SAVE SLOT {slot}: {asked:?}");
            }
        }
    }
    // ---- a death of the game's own: the load is asked for once a second until the game's step took it
    let mut death_reload = false;
    if DEATH_RELOAD.load(Ordering::Relaxed) {
        w.death_try += dt.clamp(0.0, 0.5).max(0.004);
        if w.death_try >= 1.0 {
            w.death_try = 0.0;
            death_reload = true;
        }
    }
    // ---- the game's request bytes and states
    if let Some(g) = game_man() {
        // SAFETY: bytes / words of the live game manager.
        let now = unsafe { [*g.add(SAVE_REQUESTED) as u32, *g.add(SAVE_REQUESTED_2) as u32, *g.add(0xB75) as u32, std::ptr::read_unaligned(g.add(LOAD_SLOT) as *const u32), std::ptr::read_unaligned(g.add(SAVE_STATE) as *const u32), std::ptr::read_unaligned(g.add(0xBC4) as *const u32)] };
        if now != w.last {
            erlog!("savegate: game manager: save requested {} / {} (flag {}), load slot {}, save state {}, autosave phase {}  (was {} / {} ({}), {}, {}, {})", now[0], now[1], now[2], now[3] as i32, now[4], now[5], w.last[0], w.last[1], w.last[2], w.last[3] as i32, w.last[4], w.last[5]);
            w.last = now;
        }
    }
    // ---- (IMPL_ER11) a quit whose save request was refused before the phase was looked at: it is answered here
    if points_only() && QUIT_PASS.load(Ordering::Relaxed) {
        if let Some(g) = game_man() {
            // SAFETY: bytes / words of the live game manager.
            unsafe {
                if std::ptr::read_unaligned(g.add(QUIT_PHASE) as *const u32) == 1 && *g.add(SAVE_REQUESTED) == 0 && *g.add(SAVE_REQUESTED_2) == 0 && std::ptr::read_unaligned(g.add(SAVE_STATE) as *const u32) == 0 {
                    std::ptr::write_unaligned(g.add(QUIT_PHASE) as *mut u32, 3);
                    QUITTING.store(GetTickCount64(), Ordering::Relaxed);
                    erlog!("savegate: Elden Ring's QUIT waits for a save that was refused: its phase is put at 3 (\"saved\"): the quit goes on WITHOUT SAVING");
                }
            }
        }
    }
    // ---- (IMPL_ER11) at the title screen no character is loaded: nothing is refused there
    // (IMPL_ER13) ... and NOTHING OF A SESSION SURVIVES THE TITLE: whatever is loaded from it (the same slot after a quit
    // without saving, another slot) starts from that slot's files, never from the state the session before left behind
    {
        static AT_TITLE: AtomicBool = AtomicBool::new(false);
        let title = crate::game::player_age() == 0 && crate::inputs::ui_states().is_some_and(|s| s[0] & 2 != 0);
        if title {
            if !AT_TITLE.swap(true, Ordering::Relaxed) {
                // (IMPL_ER21) the way out is over; a creation save that was not made stays to be made at the next load
                LEAVING.store(false, Ordering::Relaxed);
                CREATION.store(0, Ordering::Relaxed);
                crate::save::left_for_title();
                crate::prize::forget_session();
                crate::erhud::unstick(); // (IMPL_ER13) no character: Elden Ring's HUD mask goes
            }
            if ARMED.load(Ordering::Relaxed) {
                set_armed(false, "the title menu is up: no character is loaded");
            }
        } else if crate::game::player_age() > 0 {
            AT_TITLE.store(false, Ordering::Relaxed);
        }
    }
    // ---- the file's time, four times a second
    w.since_file += dt.clamp(0.0, 0.5).max(0.004);
    if w.since_file >= 0.25 {
        w.since_file = 0.0;
        let stamp = w.path.as_ref().and_then(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
        if stamp != w.stamp {
            erlog!("savegate: THE GAME'S SAVE FILE WAS WRITTEN (its time {} -> {}){}", stamp_text(w.stamp), stamp_text(stamp), if OURS_RUNNING.load(Ordering::Relaxed) || unsafe { GetTickCount64() }.saturating_sub(OURS_AT.load(Ordering::Relaxed)) < 6000 { ": the save point's save" } else if want && ARMED.load(Ordering::Relaxed) { ": NOT ASKED FOR BY US (save.points_only is armed: this should not happen)" } else if want { ": the game's own save (this character is not armed yet)" } else { "" });
            w.stamp = stamp;
        }
    }
    if let Some(done) = poll_saved(&mut w) {
        // (IMPL_ER21) the creation save's own answer is not the Save screen's
        if matches!(CREATION.load(Ordering::Relaxed), 2 | 3) {
            if done && CREATION.load(Ordering::Relaxed) == 3 {
                CREATION.store(0, Ordering::Relaxed);
                crate::save::creation_saved();
            } else {
                CREATION.store(1, Ordering::Relaxed);
                erlog!("savegate: the new character's creation save was NOT written ({} of 3 tries): its saving stays the game's own until it is", CREATION_TRIES.load(Ordering::Relaxed));
            }
        } else {
            RESULT.store(if done { 1 } else { 2 }, Ordering::Relaxed);
        }
    }
    drop(w);
    if fire_save {
        crate::save::ask_write(); // (IMPL_ER12)
        request_save("dbg.save");
    }
    if fire_load {
        if !reload("dbg.load") {
            request_load("dbg.load (save.points_only is off: the plain request)");
        }
    }
    if fire_title {
        quit_to_title("dbg.title");
    }
    if death_reload {
        let when = control.str("save.death_reload").unwrap_or("alive");
        let alive = crate::game::player_mut().is_some_and(|p| crate::game::player_alive(p));
        if !points_only() {
            DEATH_RELOAD.store(false, Ordering::Relaxed);
        } else if (when == "now" || alive) && reload("after a death of the game's own") {
            DEATH_RELOAD.store(false, Ordering::Relaxed);
        }
    }
}

/// (IMPL_ER21) THE CREATION SAVE (see `CREATION`): asked for once Sora has stood in the world for three seconds with
/// nothing in the way (no cutscene, no menu, no save or load running), at most three times.
fn creation_step(control: &Control) {
    if CREATION.load(Ordering::Relaxed) != 1 || !absolute() {
        return;
    }
    static READY_SINCE: AtomicU64 = AtomicU64::new(0);
    // SAFETY: plain getter.
    let now = unsafe { GetTickCount64() };
    let ready = control.str("mode") == Some("sora") && crate::game::player_age() >= 180 && !crate::hud::in_cutscene() && !crate::hud::menu_open() && !busy() && crate::game::player_mut().is_some_and(|p| crate::game::player_alive(p));
    if !ready {
        READY_SINCE.store(0, Ordering::Relaxed);
        return;
    }
    let since = READY_SINCE.load(Ordering::Relaxed);
    if since == 0 {
        READY_SINCE.store(now, Ordering::Relaxed);
        return;
    }
    if now.saturating_sub(since) < 3000 {
        return;
    }
    READY_SINCE.store(0, Ordering::Relaxed);
    if CREATION_TRIES.fetch_add(1, Ordering::Relaxed) >= 3 {
        CREATION.store(0, Ordering::Relaxed);
        erlog!("savegate: THE NEW CHARACTER'S CREATION SAVE COULD NOT BE MADE in three tries: its saving STAYS THE GAME'S OWN in this session (nothing is refused)");
        return;
    }
    let Some(slot) = current_slot() else { return };
    CREATION.store(2, Ordering::Relaxed);
    if request_save_slot(slot, "a NEW character's creation save: the one write that makes the slot loadable; after it only save points save") {
        erlog!("savegate: A NEW CHARACTER: its creation save is asked of the game now (slot {slot}); from its completion the game never saves this character by itself");
    } else {
        CREATION.store(1, Ordering::Relaxed);
    }
}

/// The once-a-second line's part.
pub fn summary() -> String {
    let n = |a: &[AtomicU32; 4]| format!("{}/{}/{}/{}", a[0].load(Ordering::Relaxed), a[1].load(Ordering::Relaxed), a[2].load(Ordering::Relaxed), a[3].load(Ordering::Relaxed));
    format!("savegate: {} | save starts asked {} refused {} started {} (S1/S2/S3/S4) | saves holding Sora's munny in the counter {} | slot {:?}", if points_only() { "points only" } else { "the game's own saving" }, n(&CALLS), n(&REFUSED), n(&STARTED), MUNNY_FIXED.load(Ordering::Relaxed), current_slot())
}
