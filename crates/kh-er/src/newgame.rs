//! (IMPL_ER11 / IMPL_ER12) NEW GAME AS THE USER SPECIFIED IT (build/sora_tmp/er/IMPL_ER12_STATE.md):
//! "can we skip character creation playing as sora" / "Automatically name the character Sora" / "the new game
//! selection choices should be on a black screen before the intro cutscene" / "the class selection should never show".
//!
//! WHAT THE GAME DOES after New Game on its title menu (MEASURED, 60 fps capture b1_newgame.png + title.rs signals):
//! the title fades to black (0.5 s), stays black ~3 s, then "Select Character Base" fades in: its dialog is MADE
//! (factory RVA 0x8A3370) in the frame the fade-in starts. Then: confirm (Vagabond), confirm (Type A), "Create
//! Character" (interface element 69) with Name focused and Finish one Up away, the question "Starting game with
//! this character. Is that okay?" (NO focused, YES one Left away). ONLY WHEN THAT IS ANSWERED does the title's step
//! machine leave MenuJobWait: `TitleStep::STEP_BeginNewGame` runs once and the game (loading screen, intro) starts.
//!
//! WHAT WE DO (`newgame.auto`):
//! 1. THE COVER: an opaque black layer of our overlay from the call that MAKES the base-select dialog (set inside
//!    that call: before the frame it would first be drawn in), so no frame of the creation screens is ever shown.
//! 2. THE CREATION is pressed for the player under the cover (keyboard when the game's window is in front, else the
//!    pad hook): confirm, confirm, [Create Character] the name "Sora" (typed), Finish, YES.
//! 3. THE CHOICES ON BLACK, BEFORE THE INTRO (`newgame.choice`, also without `newgame.auto`): `STEP_BeginNewGame`
//!    is HELD (title.rs: the state function is not run, the game asks again every frame; no menu of the game's is
//!    open any more, so the pad is free) while KH3's choice menu is asked in the HUD's title mode over the cover.
//!    The answered pair is kept in `<saves>\newgame-pending.txt` and applied when the character first loads
//!    (save.rs); then the step is let go and the cover goes when the game's own loading screen has the picture.
//! Nothing of the game's memory is written.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use windows_sys::Win32::System::SystemInformation::GetTickCount64;

use crate::control::Control;
use crate::erlog;
use crate::inputs;
use crate::title;

/// The interface element that is VISIBLE on the Create Character screen (MEASURED).
pub const EL_CREATE: usize = 69;
/// The overlay's layer of the cover (above every HUD layer but drawn UNDER them by its place: see `cover`).
const COVER_LAYER: usize = 15;
/// While the choice menu is asked the black is drawn UNDER the menu's two layers (12, 13): the cutscene pause menu's
/// layer, which cannot be in use then (the overlay draws its layers in index order).
const BACKDROP_LAYER: usize = 11;

const DPAD_UP: u16 = 0x0001;
const DPAD_LEFT: u16 = 0x0004;
const SCAN_UP: u16 = 0x148;
const SCAN_LEFT: u16 = 0x14B;
const SCAN_ENTER: u16 = 0x1C;
const SCAN_LSHIFT: u16 = 0x2A;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Step {
    Idle,
    /// "Select Character Base" is up under the cover: confirm is pressed until the creation screen is up.
    Base,
    /// The creation screen: the sequence's `k`-th press is due.
    Create(usize),
    /// The last confirm was pressed: waiting for the game to begin the new game.
    Leaving,
    /// `STEP_BeginNewGame` is held: the choices are asked on black.
    Ask,
    /// The step was let go: the cover stays until the game's loading screen has the picture.
    Tail,
    /// Gave up (the cover is off): nothing more until the title is back.
    Done,
}

struct State {
    step: Step,
    next_at: u64,
    started: u64,
    cover: bool,
    cover_size: (u32, u32),
    cover_layer: usize,
    last: Option<Vec<(usize, u8)>>,
    watch_lines: u32,
    tries: u8,
    /// This new game's creation was pressed by us.
    auto: bool,
    asked_at: u64,
    saves: Option<PathBuf>,
    /// (CV3, ERT4 fault 7) our automatic creation gave up and handed the screens to the player: when he has made the
    /// character himself the choices (the starter first) are still asked on the title's black, as for a manual creation.
    handed_over: bool,
}

static STATE: Mutex<State> = Mutex::new(State { step: Step::Idle, next_at: 0, started: 0, cover: false, cover_size: (0, 0), cover_layer: COVER_LAYER, last: None, watch_lines: 0, tries: 0, auto: false, asked_at: 0, saves: None, handed_over: false });
/// `newgame.auto` as of the latest frame (read by the dialog factory's hook).
static AUTO: AtomicBool = AtomicBool::new(false);
/// The base-select dialog was made and the cover must be up before anything else happens.
static BASE_MADE: AtomicBool = AtomicBool::new(false);

/// What is pressed on the creation screen, in order: (pad buttons, scan code (0x100 = extended), shift held,
/// keyboard only, milliseconds to wait after it).
const SEQUENCE: &[(u16, u16, bool, bool, u64)] = &[
    (inputs::XI_A, inputs::SCAN_E, false, false, 1200), // Name: the entry opens
    (0, 0x1F, true, true, 200),                         // S
    (0, 0x18, false, true, 200),                        // o
    (0, 0x13, false, true, 200),                        // r
    (0, 0x1E, false, true, 400),                        // a
    (0, SCAN_ENTER, false, true, 1200),                 // Enter: the entry closes
    (DPAD_UP, SCAN_UP, false, false, 600),              // Finish
    (inputs::XI_A, inputs::SCAN_E, false, false, 1200), // the question
    (DPAD_LEFT, SCAN_LEFT, false, false, 500),          // YES
    (inputs::XI_A, inputs::SCAN_E, false, false, 800),  // go
];

fn key(scan: u16, up: bool) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE};
    let flags = KEYEVENTF_SCANCODE | if up { KEYEVENTF_KEYUP } else { 0 } | if scan & 0x100 != 0 { KEYEVENTF_EXTENDEDKEY } else { 0 };
    let input = INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: 0, wScan: scan & 0xFF, dwFlags: flags, time: 0, dwExtraInfo: 0 } } };
    // SAFETY: one well-formed INPUT.
    unsafe { SendInput(1, &input, std::mem::size_of::<INPUT>() as i32) };
}

/// One key by the keyboard (scan code, 0x100 = extended), optionally with Shift, released 80 ms later by a thread.
fn type_key(scan: u16, shift: bool) -> bool {
    if !inputs::game_in_foreground() {
        return false;
    }
    if shift {
        key(SCAN_LSHIFT, false);
    }
    key(scan, false);
    let _ = std::thread::Builder::new().name("kh-key-up".into()).spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(80));
        key(scan, true);
        if shift {
            std::thread::sleep(std::time::Duration::from_millis(30));
            key(SCAN_LSHIFT, true);
        }
    });
    true
}

fn press(buttons: u16, scan: u16, shift: bool, keyboard_only: bool) -> Option<&'static str> {
    // THE KEYBOARD IS THE FIRST ROUTE (the one seen working; the name's letters need the game's window in front
    // anyway); the pad hook only when the window is not in front and the game reads a pad.
    if type_key(scan, shift) {
        return Some("keyboard");
    }
    if !keyboard_only && inputs::er_reads_pad() {
        return inputs::press_for_er_try(buttons, 0, 0);
    }
    None
}

fn cover(st: &mut State, on: bool) {
    cover_at(st, on, COVER_LAYER);
}

/// The black on `layer` (the other of the two cover layers is taken away AFTER the new one is up).
fn cover_at(st: &mut State, on: bool, layer: usize) {
    use kh_er_render::hud;
    if on && st.cover && st.cover_layer != layer {
        let old = st.cover_layer;
        st.cover = false;
        cover_at(st, true, layer);
        if st.cover {
            hud::hide_layer(old);
        } else {
            st.cover = true;
        }
        return;
    }
    if on {
        let Some(size) = hud::output_size().filter(|(w, h)| *w >= 64 && *h >= 64) else { return };
        if !st.cover || st.cover_size != size {
            let mut rgba = vec![0u8; size.0 as usize * size.1 as usize * 4];
            for px in rgba.chunks_exact_mut(4) {
                px[3] = 255;
            }
            hud::set_options(hud::HudOptions { on: true, opacity: 1.0, ..hud::HudOptions::default() });
            hud::show_layer(layer, (0, 0, size.0 as usize, size.1 as usize), &rgba, None);
            st.cover = true;
            st.cover_layer = layer;
            st.cover_size = size;
        }
    } else if st.cover {
        hud::hide_layer(st.cover_layer);
        hud::set_options(hud::HudOptions::default());
        st.cover = false;
    }
}

/// The cover is up (the HUD's own option writes must not take it down).
pub fn covering() -> bool {
    STATE.lock().is_ok_and(|s| s.cover)
}

/// (title.rs, inside the game's call that makes "Select Character Base") the cover goes up NOW.
pub fn on_base_made() {
    if !AUTO.load(Ordering::Relaxed) {
        return;
    }
    BASE_MADE.store(true, Ordering::Relaxed);
    if let Ok(mut st) = STATE.try_lock() {
        cover(&mut st, true);
    }
}

fn pending_path(st: &State) -> Option<PathBuf> {
    st.saves.as_ref().map(|d| d.join("newgame-pending.txt"))
}

/// The pair the player chose on the title's black for the character that is being made (taken by save.rs when a
/// character without a file first loads; the file is removed then). `None`: nothing waits.
pub fn take_pending(dir: &std::path::Path) -> Option<kh_core::status::OpeningChoice> {
    use kh_core::status::{Growth, OpeningChoice, Power};
    let path = dir.join("newgame-pending.txt");
    let text = std::fs::read_to_string(&path).ok()?;
    let _ = std::fs::remove_file(&path);
    let mut desire = None;
    let mut power = None;
    for line in text.lines() {
        match line.split_once('=').map(|(k, v)| (k.trim(), v.trim())) {
            Some(("desire", "vitality")) => desire = Some(Growth::Vitality),
            Some(("desire", "balance")) => desire = Some(Growth::Balance),
            Some(("desire", "wisdom")) => desire = Some(Growth::Wisdom),
            Some(("power", name)) => power = Power::from_name(name),
            _ => {}
        }
    }
    Some(OpeningChoice { desire: desire?, power: power? })
}

fn store_pending(st: &State, choice: kh_core::status::OpeningChoice) {
    use kh_core::status::Growth;
    let Some(path) = pending_path(st) else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let desire = match choice.desire {
        Growth::Vitality => "vitality",
        Growth::Balance => "balance",
        Growth::Wisdom => "wisdom",
    };
    match std::fs::write(&path, format!("desire={desire}\r\npower={}\r\n", choice.power.name())) {
        Ok(()) => erlog!("newgame: the pair {:?} + {:?} is kept in {} for the character that is being made", choice.desire, choice.power, path.display()),
        Err(e) => erlog!("newgame: the pair could NOT be kept ({e}): the menu comes again after the intro"),
    }
}

/// Once a frame (FRAME_BEGIN; runs at the title too).
pub fn frame(control: &Control, root: &std::path::Path) {
    let Ok(mut st) = STATE.lock() else { return };
    #[cfg(feature = "party")]
    crate::unlock::set_root(root); // (ER party S8) the unlock sheet is found from the root
    if st.saves.is_none() {
        st.saves = Some(root.join(control.str("save.dir").unwrap_or("saves")));
    }
    let states = inputs::ui_states();
    // (testing aid) `dbg.ui.watch = 1`: every change of the visible / created interface elements is logged
    if control.on("dbg.ui.watch") {
        let now: Option<Vec<(usize, u8)>> = states.map(|s| s.iter().enumerate().filter(|(_, v)| **v != 0).map(|(k, v)| (k, *v)).collect());
        if now != st.last && st.watch_lines < 300 {
            st.watch_lines += 1;
            erlog!("newgame: interface elements (index:created|visible<<1): {}", now.as_ref().map_or("none".to_string(), |s| s.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(" ")));
            st.last = now;
        }
    }
    let create = control.i64("newgame.el.create").map_or(EL_CREATE, |v| v.max(0) as usize);
    let on_create = states.is_some_and(|s| s.get(create).is_some_and(|v| v & 2 != 0));
    // SAFETY: plain getter.
    let now = unsafe { GetTickCount64() };
    let hooked = title::hooked();
    let auto = control.on("newgame.auto") && hooked;
    let choice_on = hooked && control.on("save.kh") && control.on("hud.kh") && control.flag("newgame.choice") != Some(false) && control.flag("newgame.choice.title") != Some(false);
    AUTO.store(auto, Ordering::Relaxed);
    let has_player = crate::game::player_mut().is_some();
    if has_player {
        if st.step != Step::Idle {
            cover(&mut st, false);
            crate::hud::title_end();
            title::let_go();
            st.step = Step::Idle;
        }
        BASE_MADE.store(false, Ordering::Relaxed);
        return;
    }
    // AT THE TITLE (no player): the UI pack is read ahead, and a new game's first step is held until the choices are made.
    // (CV3) also after our creation gave up (`handed_over`): the new game the player starts himself is held for the choices too
    if choice_on && (!matches!(st.step, Step::Tail | Step::Done) || (st.step == Step::Done && st.handed_over)) {
        crate::hud::title_prepare(control);
        title::hold_new_game();
    }
    match st.step {
        Step::Idle => {
            if BASE_MADE.swap(false, Ordering::Relaxed) && auto {
                st.step = Step::Base;
                st.auto = true;
                st.started = now;
                st.next_at = now + control.i64("newgame.base.wait").map_or(1500, |v| v.max(0) as u64);
                st.tries = 0;
                cover(&mut st, true);
                erlog!("newgame: \"Select Character Base\" was made: THE CREATION IS DONE FOR THE PLAYER BEHIND A BLACK COVER (newgame.auto): default origin, name \"Sora\"");
            } else if title::begin_new_game_for().is_some() && choice_on {
                // (the player made the character himself: the choices are still asked on black before the intro)
                st.auto = false;
                st.started = now;
                st.step = Step::Ask;
                st.asked_at = now;
                cover_at(&mut st, true, BACKDROP_LAYER);
            }
        }
        Step::Base => {
            cover(&mut st, true);
            if on_create {
                st.step = Step::Create(0);
                st.next_at = now + 1200;
                st.tries = 0;
            } else if now >= st.next_at && st.tries < 2 {
                let how = press(inputs::XI_A, inputs::SCAN_E, false, false);
                erlog!("newgame: base screen: confirm {} pressed ({})", st.tries + 1, how.unwrap_or("NO WAY: neither a pad the game reads nor its window in front"));
                if how.is_some() {
                    st.tries += 1;
                }
                st.next_at = now + if how.is_some() { 1300 } else { 400 };
            } else if now >= st.next_at + 6000 {
                // (two confirms did not bring the creation screen: once more)
                st.tries = 1;
            }
        }
        Step::Create(k) => {
            cover(&mut st, true);
            if title::begin_new_game_for().is_some() {
                st.step = Step::Leaving;
            } else if now >= st.next_at {
                // (IMPL_ER17, TRIED AND TAKEN OUT: leaving the Name entry alone when the keyboard cannot type it. The game refuses
                // Finish with an empty name ("None entered" in red), so the name cannot be written into the character's data
                // afterwards; without the game's window in front the letters stay untyped and the time limit hands the screen over.)
                match SEQUENCE.get(k) {
                    Some(&(buttons, scan, shift, keyboard_only, wait)) => {
                        let how = press(buttons, scan, shift, keyboard_only);
                        erlog!("newgame: creation screen: step {k} pressed ({})", how.unwrap_or("NO WAY"));
                        if how.is_none() {
                            // (the window is not in front: try again in a moment; the time limit ends it)
                            st.next_at = now + 500;
                        } else {
                            st.step = Step::Create(k + 1);
                            st.next_at = now + wait;
                        }
                    }
                    None => {
                        st.step = Step::Leaving;
                        st.next_at = now + 4000;
                    }
                }
            }
        }
        Step::Leaving => {
            cover(&mut st, true);
            if title::begin_new_game_for().is_some() {
                erlog!("newgame: the character \"Sora\" is made ({:.1} s after its first screen): the game begins its new game", (now - st.started) as f32 / 1000.0);
                st.step = Step::Ask;
                st.asked_at = now;
            } else if now >= st.next_at && on_create {
                erlog!("newgame: the creation screen is still up: the cover goes, the player finishes it (the choices, the starter first, are asked when he has made it) (CV3)");
                cover(&mut st, false);
                st.step = Step::Done;
                st.handed_over = true;
            }
        }
        Step::Ask => {
            cover_at(&mut st, true, BACKDROP_LAYER);
            let answer = if choice_on { crate::hud::title_choice_frame(control) } else { crate::hud::TitleChoice::Unavailable };
            let go = match answer {
                crate::hud::TitleChoice::Asking => false,
                crate::hud::TitleChoice::Loading => now.saturating_sub(st.asked_at) > 15_000,
                crate::hud::TitleChoice::Unavailable => true,
                crate::hud::TitleChoice::Chosen(choice) => {
                    erlog!("newgame: THE OPENING'S CHOICES were made on the title's black: {:?} + {:?} ({} calls of the game's new-game step were held back meanwhile)", choice.desire, choice.power, title::held_calls());
                    store_pending(&st, choice);
                    #[cfg(feature = "party")]
                    crate::unlock::store_starter(st.saves.as_deref()); // (ER party S8) the character select's pick beside the pair
                    true
                }
            };
            if go {
                if !matches!(answer, crate::hud::TitleChoice::Chosen(_)) {
                    erlog!("newgame: the choice menu is not available on the title ({answer:?}): the game goes on; the menu comes after the intro");
                }
                crate::hud::title_end();
                title::release();
                st.step = Step::Tail;
                st.next_at = now + 20_000;
            }
        }
        Step::Tail => {
            // the game's own loading screen (black) takes over: the cover goes once the title step waits for the game step
            cover(&mut st, true);
            let waiting = title::state() == title::ST_GAME_STEP_WAIT && title::state_for() > control.i64("newgame.tail").map_or(700, |v| v.max(0) as u64);
            if waiting || now >= st.next_at {
                erlog!("newgame: the game is starting ({}): the cover goes", if waiting { "its title step waits for the game step" } else { "20 s passed" });
                cover(&mut st, false);
                st.step = Step::Done;
            }
        }
        Step::Done => {
            // (CV3, ERT4 fault 7) THE AUTOMATIC CREATION FAILED AND THE PLAYER MADE THE CHARACTER HIMSELF: recover as a manual
            // creation does: the choices, the starter first, on the title's black before the intro. Before, Done waited for
            // the next title menu, so the new game began unasked (the menu after the intro was the only chance left).
            if st.handed_over && choice_on && title::begin_new_game_for().is_some() {
                st.handed_over = false;
                st.auto = false;
                st.step = Step::Ask;
                st.asked_at = now;
                cover_at(&mut st, true, BACKDROP_LAYER);
                erlog!("newgame: the player made the character himself after our creation gave up: THE CHOICES ARE ASKED on the title's black (the starter first) (CV3)");
            } else if title::top_made_ago().is_some_and(|ms| ms < 1000) || title::state() == title::ST_MENU_JOB_WAIT && title::state_for() < 1000 {
                st.step = Step::Idle;
                st.handed_over = false;
            }
        }
    }
    // WHILE THE CREATION IS PRESSED FOR THE PLAYER the game sees nothing of the real pad (a press of his under the cover
    // would land in the creation screens; our own presses go in by the keyboard or by the pad hook's virtual pad).
    if matches!(st.step, Step::Base | Step::Create(_) | Step::Leaving) {
        inputs::set_mask(u32::MAX);
    }
    if matches!(st.step, Step::Base | Step::Create(_) | Step::Leaving) && now.saturating_sub(st.started) > 45_000 {
        erlog!("newgame: NOT FINISHED in 45 s (step {:?}): the cover goes, the creation screens are the player's (the choices, the starter first, are asked when he has made it) (CV3)", st.step);
        cover(&mut st, false);
        st.step = Step::Done;
        st.handed_over = true;
    }
}

/// The payload goes away.
pub fn shutdown() {
    if let Ok(mut st) = STATE.lock() {
        cover(&mut st, false);
        st.step = Step::Idle;
    }
    title::let_go();
}
