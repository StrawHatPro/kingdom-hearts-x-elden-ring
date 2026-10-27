//! (IMPL_ER15) KH3'S LOAD SCREEN ON ELDEN RING'S TITLE (build/sora_tmp/er/IMPL_ER15_STATE.md; `saveload.kh` with
//! `saveload.title` not 0): choosing Load Game on the title shows OUR Load screen over black; a slot chosen there is
//! loaded by THE GAME'S OWN Load Game list; B returns to the title menu.
//!
//! WHAT THE GAME HAS (read in eldenring.exe 2.7.1.0): the title's load of a slot is a chain of menu jobs made by the
//! list's OK handler (the job "プロフィール%d", step function RVA 0x82D230, takes the slot from the closure RVA
//! 0x825850 builds it with: it reads the slot's data, calls the save-slot setter RVA 0x67B660 and fills the game
//! data); nothing outside that chain asks for "load slot N" (the in-session request `GameMan + 0xB78` needs a world).
//! Making that closure ourselves would mean building the menu system's functor objects by hand: NOT done.
//!
//! WHAT WE DO INSTEAD: the game's own list carries the load out, pressed under our black, and so that the press can
//! only ever pick the wanted slot THE LIST IS MADE WITH THAT ONE SLOT IN IT: the profile summary's "in use" bytes of
//! every other slot are cleared IN MEMORY for the duration of the call that makes the list (`ProfileLoadDialog`'s
//! factory RVA 0x81F950, detoured in title.rs) and put back right after (`saveload.title.unmask` ms later; no save
//! start is let through meanwhile: savegate.rs). The steps:
//! 1. the player chooses Load Game: inside the factory's call our black goes up (overlay layer 11); the game's list
//!    is there under it; the pad is hidden from the game; KH3's Load screen runs in the HUD's title mode;
//! 2. B: Back is pressed for the game's list, the black goes when the title menu is back;
//! 3. a slot: Back is pressed for the game's list; when the title menu is back (interface element 0 visible) confirm
//!    is pressed (its cursor is still on Load Game: MEASURED), the list is made again WITH ONE ROW, confirm picks it,
//!    confirm answers "Starting with selected profile" (OK is its first choice: MEASURED), the title step leaves
//!    MenuJobWait; from there plain black (the overlay's plate) until the loaded character has the picture: the game's
//!    own loading screen is not shown.
//! If anything does not come within its time the black goes and the game's own screen is the player's.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, Ordering};
use std::sync::Mutex;

use windows_sys::Win32::System::SystemInformation::GetTickCount64;

use crate::control::Control;
use crate::erlog;
use crate::inputs;
use crate::title;

/// The black under the screen's two layers (12, 13): the cutscene pause menu's layer (not in use at the title).
const BACKDROP_LAYER: usize = 11;
/// The interface element that is visible while the title's own menu is up (MEASURED in IMPL_ER11).
const EL_TITLE_MENU: usize = 0;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Step {
    Idle,
    /// Our Load screen has the picture and the pad.
    Screen,
    /// B was pressed on our screen: the game's list is being left.
    Leave,
    /// A slot was chosen: the game's list is being left first.
    Back(usize),
    /// The title menu is back: Load Game is confirmed again, the list will be made with one row.
    Open(usize),
    /// The one-row list is up: confirm until the title step begins the game.
    Confirm(usize),
    /// The game is loading: the black stays until its loading screen has the picture.
    Tail,
    /// Gave up / done: nothing more until the title menu is back.
    Done,
}

struct State {
    step: Step,
    started: u64,
    next_at: u64,
    tries: u8,
    cover: bool,
    cover_size: (u32, u32),
}

static STATE: Mutex<State> = Mutex::new(State { step: Step::Idle, started: 0, next_at: 0, tries: 0, cover: false, cover_size: (0, 0) });
/// The feature is on as of the latest frame (read by the factory's hook).
static ENABLED: AtomicBool = AtomicBool::new(false);
/// The player's own Load Game: the list was made (the hook), the frame takes it from here.
static MADE: AtomicBool = AtomicBool::new(false);
/// The next list is ours: it is to list this slot only (-1: none asked).
static ONLY: AtomicI32 = AtomicI32::new(-1);
/// The list we asked for was made (tick).
static OURS_MADE_AT: AtomicU64 = AtomicU64::new(0);
/// The summary's "in use" bytes are masked: their true values, the tick they go back at the latest.
static MASK: Mutex<Option<(usize, [u8; 10])>> = Mutex::new(None);
static MASKED: AtomicBool = AtomicBool::new(false);
static UNMASK_AT: AtomicU64 = AtomicU64::new(0);
static UNMASK_MS: AtomicU64 = AtomicU64::new(0);

fn now() -> u64 {
    // SAFETY: plain getter.
    unsafe { GetTickCount64() }
}

/// The profile summary's "in use" bytes are masked right now (savegate.rs lets no save start through).
pub fn masked() -> bool {
    MASKED.load(Ordering::Relaxed)
}

/// The profile summary object of the running game (`GameDataMan + 0x78`), checked: its ten "in use" bytes at + 8.
fn summary_flags() -> Option<usize> {
    use eldenring::cs::GameDataMan;
    use fromsoftware_shared::FromStatic;
    // SAFETY: game thread; the singleton lives as long as the game; the pointer is checked before it is followed.
    let man = unsafe { GameDataMan::instance() }.ok()? as *const GameDataMan as usize;
    // SAFETY: a pointer-sized field of the live manager.
    let object = unsafe { std::ptr::read_unaligned((man + 0x78) as *const usize) };
    if !crate::game::plausible(object) || !crate::pause::heap_readable(object, 0x18) {
        return None;
    }
    // SAFETY: readable (checked).
    let flags = unsafe { std::slice::from_raw_parts((object + 8) as *const u8, 10) };
    flags.iter().all(|f| *f <= 1).then_some(object + 8)
}

fn unmask(why: &str) {
    let Ok(mut mask) = MASK.lock() else { return };
    if let Some((at, original)) = mask.take() {
        // SAFETY: the ten flag bytes of the live summary object (the address was checked when they were masked; the
        // object lives as long as the game's data manager).
        unsafe { std::ptr::copy_nonoverlapping(original.as_ptr(), at as *mut u8, 10) };
        erlog!("titleload: the profile summary's \"in use\" bytes are put back ({}): {why}", original.iter().map(|f| f.to_string()).collect::<String>());
    }
    MASKED.store(false, Ordering::SeqCst);
}

/// (title.rs, inside the game's call that makes the Load Game list, BEFORE the game's own code) a list of ours is
/// made with one slot; the player's own list gets our black over it at once.
pub fn before_dialog() {
    let only = ONLY.swap(-1, Ordering::SeqCst);
    if (0..10).contains(&only) {
        if let (Some(at), Ok(mut mask)) = (summary_flags(), MASK.lock()) {
            // SAFETY: ten readable and writable bytes of the live summary object (checked in `summary_flags`).
            let flags = unsafe { std::slice::from_raw_parts_mut(at as *mut u8, 10) };
            if flags[only as usize] == 1 && mask.is_none() {
                let mut original = [0u8; 10];
                original.copy_from_slice(flags);
                MASKED.store(true, Ordering::SeqCst);
                for (k, f) in flags.iter_mut().enumerate() {
                    if k != only as usize {
                        *f = 0;
                    }
                }
                *mask = Some((at, original));
                UNMASK_AT.store(now() + 3000, Ordering::SeqCst);
                OURS_MADE_AT.store(now(), Ordering::SeqCst);
            }
        }
        return;
    }
    if ENABLED.load(Ordering::Relaxed) {
        MADE.store(true, Ordering::SeqCst);
        if let Ok(mut st) = STATE.try_lock() {
            if st.step == Step::Idle || st.step == Step::Done {
                cover(&mut st, true);
            }
        }
    }
}

/// (title.rs, right after the game's own code made the list) the masked bytes go back (at once, or
/// `saveload.title.unmask` ms later when the list reads them after its construction).
pub fn after_dialog() {
    if !MASKED.load(Ordering::Relaxed) {
        return;
    }
    let wait = UNMASK_MS.load(Ordering::Relaxed);
    if wait == 0 {
        unmask("the list is made");
    } else {
        UNMASK_AT.store(now() + wait.min(3000), Ordering::SeqCst);
    }
}

fn cover(st: &mut State, on: bool) {
    use kh_er_render::hud;
    if on {
        let Some(size) = hud::output_size().filter(|(w, h)| *w >= 64 && *h >= 64) else { return };
        if !st.cover || st.cover_size != size {
            let mut rgba = vec![0u8; size.0 as usize * size.1 as usize * 4];
            for px in rgba.chunks_exact_mut(4) {
                px[3] = 255;
            }
            hud::set_options(hud::HudOptions { on: true, opacity: 1.0, ..hud::HudOptions::default() });
            hud::show_layer(BACKDROP_LAYER, (0, 0, size.0 as usize, size.1 as usize), &rgba, None);
            st.cover = true;
            st.cover_size = size;
        }
    } else if st.cover {
        hud::hide_layer(BACKDROP_LAYER);
        st.cover = false;
    }
}

fn press(buttons: u16, scan: u16, attempt: u8) -> &'static str {
    inputs::press_for_er_try(buttons, scan, attempt).unwrap_or("NO WAY: neither a pad the game reads nor its window in front")
}

fn give_up(st: &mut State, why: &str) {
    erlog!("titleload: {why}: the black goes, the game's own screen is the player's");
    ONLY.store(-1, Ordering::SeqCst);
    unmask("giving up");
    crate::hud::title_end();
    cover(st, false);
    st.step = Step::Done;
}

/// The "in use" bytes as last seen at the title (unmasked), and the tick before which a change is not acted on.
static SEEN: Mutex<(Option<[u8; 10]>, u64)> = Mutex::new((None, 0));

/// A PROFILE DELETED IN ELDEN RING'S OWN MENUS ("Delete" on its Load Game list; "Delete selected profile and begin a new
/// game?" when all ten are used): at the title a slot's "in use" byte going 1 -> 0 means the character is gone, and
/// Sora's files of that slot (`slot<N>_*.khsave`, its `.bak`, the save-points marker) are moved into the folder
/// `deleted` beside them (kept, not erased), so that a new character in that slot starts as a new Sora.
fn deleted_slots(root: &std::path::Path, control: &Control, t: u64) {
    if masked() || crate::game::player_mut().is_some() {
        if let Ok(mut seen) = SEEN.lock() {
            // (in play the bytes change with saves into new slots: nothing is concluded; after a mask: a second's grace)
            seen.0 = None;
            seen.1 = t + 1000;
        }
        return;
    }
    // (TITLEPERF) four looks a second are plenty for a change that must stand 1.5 s
    static NEXT_AT: AtomicU64 = AtomicU64::new(0);
    if t < NEXT_AT.load(Ordering::Relaxed) {
        return;
    }
    NEXT_AT.store(t + 250, Ordering::Relaxed);
    let Some(at) = summary_flags() else { return };
    let mut now_flags = [0u8; 10];
    // SAFETY: ten readable bytes of the live summary object (checked in `summary_flags`).
    now_flags.copy_from_slice(unsafe { std::slice::from_raw_parts(at as *const u8, 10) });
    let Ok(mut seen) = SEEN.lock() else { return };
    if t < seen.1 {
        return;
    }
    // (ONE slot at a time, and only once the new bytes have stood for 1.5 s: the summary being read afresh on the way
    // to the title must never look like ten deletions)
    static CANDIDATE: Mutex<Option<([u8; 10], u64)>> = Mutex::new(None);
    if let Some(before) = seen.0 {
        if before != now_flags {
            let gone = (0..10).filter(|k| before[*k] == 1 && now_flags[*k] == 0).count();
            let Ok(mut candidate) = CANDIDATE.lock() else { return };
            if gone != 1 {
                // (a new character, or more than one slot at once: taken as the new state without concluding anything)
                *candidate = None;
                seen.0 = Some(now_flags);
                return;
            }
            match *candidate {
                Some((flags, since)) if flags == now_flags => {
                    if t.saturating_sub(since) < 1500 {
                        return;
                    }
                    *candidate = None;
                }
                _ => {
                    *candidate = Some((now_flags, t));
                    return;
                }
            }
        }
        for slot in 0..10 {
            if before[slot] == 1 && now_flags[slot] == 0 {
                // (and the SAVE FILE must say so too: the game writes its profile list when a profile is deleted)
                if crate::slots::file_used(slot) != Some(false) {
                    erlog!("titleload: slot {slot}'s \"in use\" byte went 1 -> 0 in memory but the save file still lists it (or cannot be read): nothing is moved");
                    continue;
                }
                let dir = root.join(control.str("save.dir").unwrap_or("saves"));
                let prefix = format!("slot{slot}_");
                let mut moved = Vec::new();
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        if name.starts_with(&prefix) && entry.path().is_file() {
                            let to = dir.join("deleted");
                            let _ = std::fs::create_dir_all(&to);
                            if std::fs::rename(entry.path(), to.join(format!("{t}_{name}"))).is_ok() {
                                moved.push(name);
                            }
                        }
                    }
                }
                erlog!("titleload: ELDEN RING'S PROFILE IN SLOT {slot} WAS DELETED (its \"in use\" byte went 1 -> 0 at the title): Sora's files of that slot are moved to the folder \"deleted\" in {}: {}", dir.display(), if moved.is_empty() { "none were there".to_string() } else { moved.join(", ") });
            }
        }
    }
    seen.0 = Some(now_flags);
}

/// Once a frame (FRAME_BEGIN; runs at the title too).
/// (TITLEPERF) This frame step's time in parts since the last [`take_perf`] (ns): the deleted-slot watch, the wait for the
/// step's state, title_prepare (the HUD's lock and load step), the rest.
static PERF: [std::sync::atomic::AtomicU64; 4] = [const { std::sync::atomic::AtomicU64::new(0) }; 4];

pub fn take_perf() -> [u64; 4] {
    std::array::from_fn(|k| PERF[k].swap(0, Ordering::Relaxed))
}

pub fn frame(control: &Control, root: &std::path::Path) {
    let started = std::time::Instant::now();
    deleted_slots(root, control, now());
    PERF[0].fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
    let rest = std::time::Instant::now();
    frame_steps(control);
    PERF[3].fetch_add(rest.elapsed().as_nanos() as u64, Ordering::Relaxed);
}

fn frame_steps(control: &Control) {
    // (the masked bytes never outlive their time, whatever else happens)
    if MASKED.load(Ordering::Relaxed) && now() >= UNMASK_AT.load(Ordering::Relaxed) {
        unmask("their time is up");
    }
    UNMASK_MS.store(control.i64("saveload.title.unmask").map_or(0, |v| v.clamp(0, 3000) as u64), Ordering::Relaxed);
    let waited = std::time::Instant::now();
    let Ok(mut st) = STATE.lock() else { return };
    PERF[1].fetch_add(waited.elapsed().as_nanos() as u64, Ordering::Relaxed);
    let on = control.on("saveload.kh") && control.on("hud.kh") && control.flag("saveload.title") != Some(false) && title::load_hooked();
    ENABLED.store(on, Ordering::Relaxed);
    let t = now();
    if st.step == Step::Tail {
        // THE GAME IS LOADING: plain black (the overlay's plate: the HUD's own "loading screen" rule cannot take it off)
        // until the in-game step has run for half a second with the loaded character there, 25 s at the most
        let playing = crate::game::player_mut().is_some() && crate::pause::hooked() && crate::pause::available();
        if !playing {
            st.tries = 0;
        } else if st.tries == 0 {
            st.tries = 1;
            st.started = t;
        }
        if (playing && t.saturating_sub(st.started) > 500) || t >= st.next_at {
            erlog!("titleload: {}: the black goes", if playing { "the loaded character has the picture" } else { "25 s passed" });
            kh_er_render::hud::set_plate(None);
            st.step = Step::Idle;
            MADE.store(false, Ordering::Relaxed);
        } else {
            kh_er_render::hud::set_plate(Some((0, [0.0, 0.0, 0.0, 1.0], 600)));
        }
        return;
    }
    if crate::game::player_mut().is_some() {
        if st.step != Step::Idle {
            if st.cover || matches!(st.step, Step::Tail) {
                erlog!("titleload: the loaded character is there ({:.1} s after the slot was chosen): the black goes", t.saturating_sub(st.started) as f32 / 1000.0);
            }
            crate::hud::title_end();
            cover(&mut st, false);
            st.step = Step::Idle;
        }
        MADE.store(false, Ordering::Relaxed);
        ONLY.store(-1, Ordering::Relaxed);
        return;
    }
    let title_menu = inputs::ui_states().is_some_and(|s| s[EL_TITLE_MENU] & 2 != 0);
    let no_cover = control.flag("saveload.title.cover") == Some(false); // (testing aid: the game's screens stay visible)
    match st.step {
        Step::Idle => {
            if on {
                let prepared = std::time::Instant::now();
                crate::hud::title_prepare(control);
                PERF[2].fetch_add(prepared.elapsed().as_nanos() as u64, Ordering::Relaxed);
            }
            if MADE.swap(false, Ordering::SeqCst) && on {
                st.step = Step::Screen;
                st.started = t;
                st.next_at = t + 15_000;
                if !no_cover {
                    cover(&mut st, true);
                }
                erlog!("titleload: the game's Load Game list was made: KH3'S LOAD SCREEN takes its place over black (the game's list waits under it; the pad is hidden from the game)");
            }
        }
        Step::Screen => {
            if !no_cover {
                cover(&mut st, true);
            }
            inputs::set_mask(u32::MAX);
            match crate::hud::title_load_frame(control, true) {
                crate::hud::TitleLoad::Open => st.next_at = t + 15_000,
                crate::hud::TitleLoad::Loading => {
                    if t >= st.next_at {
                        give_up(&mut st, "the UI pack was not read in 15 s");
                    }
                }
                crate::hud::TitleLoad::Unavailable => give_up(&mut st, "KH3's Load screen is not available"),
                crate::hud::TitleLoad::Cancel => {
                    erlog!("titleload: B on the Load screen: Back is pressed for the game's list ({})", press(inputs::XI_B, inputs::SCAN_Q, 0));
                    st.step = Step::Leave;
                    st.started = t;
                    st.next_at = t + 900;
                    st.tries = 1;
                }
                crate::hud::TitleLoad::Load(slot) => {
                    erlog!("titleload: SLOT {slot} CHOSEN on the title (no question, as in KH3): the game's own list will load it: Back is pressed first ({})", press(inputs::XI_B, inputs::SCAN_Q, 0));
                    st.step = Step::Back(slot);
                    st.started = t;
                    st.next_at = t + 900;
                    st.tries = 1;
                }
            }
        }
        Step::Leave => {
            inputs::set_mask(u32::MAX);
            if title_menu {
                erlog!("titleload: the title menu is back: the black goes");
                crate::hud::title_end();
                cover(&mut st, false);
                st.step = Step::Idle;
                MADE.store(false, Ordering::Relaxed);
            } else if t >= st.next_at && st.tries < 4 {
                erlog!("titleload: Back pressed again ({})", press(inputs::XI_B, inputs::SCAN_Q, st.tries));
                st.tries += 1;
                st.next_at = t + 900;
            } else if t.saturating_sub(st.started) > 6000 {
                give_up(&mut st, "the title menu did not come back in 6 s");
            }
        }
        Step::Back(slot) => {
            inputs::set_mask(u32::MAX);
            let _ = crate::hud::title_load_frame(control, false);
            if title_menu {
                ONLY.store(slot as i32, Ordering::SeqCst);
                OURS_MADE_AT.store(0, Ordering::SeqCst);
                erlog!("titleload: the title menu is back: Load Game is confirmed again ({}); its list will be made with slot {slot} alone", press(inputs::XI_A, inputs::SCAN_E, 0));
                st.step = Step::Open(slot);
                st.next_at = t + 1500;
                st.tries = 1;
            } else if t >= st.next_at && st.tries < 4 {
                erlog!("titleload: Back pressed again ({})", press(inputs::XI_B, inputs::SCAN_Q, st.tries));
                st.tries += 1;
                st.next_at = t + 900;
            } else if t.saturating_sub(st.started) > 6000 {
                give_up(&mut st, "the title menu did not come back in 6 s");
            }
        }
        Step::Open(slot) => {
            inputs::set_mask(u32::MAX);
            let _ = crate::hud::title_load_frame(control, false);
            if OURS_MADE_AT.load(Ordering::SeqCst) != 0 {
                erlog!("titleload: the game's list is made with slot {slot} alone");
                st.step = Step::Confirm(slot);
                st.next_at = t + control.i64("saveload.title.wait").map_or(900, |v| v.clamp(100, 5000) as u64);
                st.tries = 0;
            } else if ONLY.load(Ordering::SeqCst) < 0 {
                // (the hook took the request but could not mask: the summary was not what we think)
                give_up(&mut st, "the profile summary could not be masked");
            } else if t >= st.next_at && st.tries < 3 && title_menu {
                erlog!("titleload: Load Game confirmed again ({})", press(inputs::XI_A, inputs::SCAN_E, st.tries));
                st.tries += 1;
                st.next_at = t + 1500;
            } else if t.saturating_sub(st.started) > 12_000 {
                give_up(&mut st, "the game's list was not made again in time");
            }
        }
        Step::Confirm(slot) => {
            inputs::set_mask(u32::MAX);
            let _ = crate::hud::title_load_frame(control, false);
            let state = title::state();
            if state != title::ST_MENU_JOB_WAIT && title::state_for() < 5000 {
                erlog!("titleload: THE GAME BEGINS ITS LOAD OF SLOT {slot} (title step {}; {:.1} s after the choice, {} confirm(s)): the black stays until its loading screen has the picture", title::STATE_NAMES.get(state as usize).copied().unwrap_or("?"), t.saturating_sub(st.started) as f32 / 1000.0, st.tries);
                kh_er_render::hud::set_plate(Some((0, [0.0, 0.0, 0.0, 1.0], 600)));
                crate::hud::title_end();
                cover(&mut st, false);
                st.step = Step::Tail;
                st.tries = 0;
                st.next_at = t + 25_000;
            } else if t >= st.next_at && st.tries < 8 {
                erlog!("titleload: confirm {} for the one-row list ({})", st.tries + 1, press(inputs::XI_A, inputs::SCAN_E, st.tries));
                st.tries += 1;
                st.next_at = t + 900;
            } else if t.saturating_sub(st.started) > 20_000 {
                give_up(&mut st, "the game did not begin the load in 20 s");
            }
        }
        Step::Tail => {}
        Step::Done => {
            if title_menu {
                st.step = Step::Idle;
                MADE.store(false, Ordering::Relaxed);
            }
        }
    }
}

/// The payload goes away: the masked bytes go back, the black goes.
pub fn shutdown() {
    unmask("the payload goes away");
    ONLY.store(-1, Ordering::SeqCst);
    if let Ok(mut st) = STATE.lock() {
        cover(&mut st, false);
        st.step = Step::Idle;
    }
}
