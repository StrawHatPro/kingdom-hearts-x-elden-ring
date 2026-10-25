//! (IMPL_ER12) THE TITLE'S FLOW (build/sora_tmp/er/IMPL_ER12_STATE.md): what the game is doing between its title
//! menu and the first frame of a character, told by the game's own code instead of by what is on screen.
//!
//! WHAT THE GAME HAS (eldenring.exe 2.7.1.0, read from the file):
//! - `CS::TitleStep`, a step machine with the states (table at RVA 0x3D755F0, `{fn, name}` pairs)
//!   0 InitMenu, 1 BeginXR117Dialog, 2 BeginLogo, 3 BeginTitle, 4 BeginNewGame, 5 PlayGame, 6 GameStepWait, 7 EndFlow,
//!   8 EndFlowWait, 9 NextLap, 10 MenuJobWait, 11 Finish; its state setter is RVA 0xB0F070 (`fn(step, state)`,
//!   current state at step + 0x48, asked state at + 0x4C).
//! - the dialogs of the title's menu job, each made by one factory: `TitleTopDialog` (0x81F460), `ProfileLoadDialog`
//!   (Load Game, 0x81F950), `ChrMakeBaseSelectDialog` ("Select Character Base", 0x8A3370).
//!
//! WHAT WE DO: those functions are detoured (each used only where 2.7.1.0 has it and only when its first bytes
//! are the ones read from the file) to KNOW the flow; `STEP_BeginNewGame` (0xB0DC50) can also be HELD: while
//! `hold_new_game` is asked the state function is simply not run, so the step stays in that state (the game calls
//! it again next frame) until we let it go. Nothing of the game's memory is written.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, AtomicUsize, Ordering};

use windows_sys::Win32::System::SystemInformation::GetTickCount64;

use crate::erlog;

/// A function of the executable at its 2.7.1.0 address with the bytes it must start with (`??` = any).
/// (PORT) `rva` is the 2.7.1.0 address, `rva_2_7_2_0` the 2.7.2.0 one (0 = none recorded: the one unique place the
/// bytes are at is taken, see `at`).
pub(crate) struct Known {
    pub name: &'static str,
    pub rva: usize,
    pub rva_2_7_2_0: usize,
    pub bytes: &'static str,
}

impl Known {
    /// (PORT) The bytes as a pelite pattern (`??` = any byte).
    pub fn pattern(&self) -> String {
        self.bytes.split_whitespace().map(|b| if b == "??" { "?" } else { b }).collect::<Vec<_>>().join(" ")
    }
}

/// (PORT) THE RULE for a `Known`, for the running executable and for the offline check of both files alike:
/// `Ok(rva)` when the version's recorded RVA starts with the bytes; with no recorded RVA, the single place in the
/// code the bytes are at (none or several: `Err`, the feature stays off).
pub(crate) fn known_in<'a, P: pelite::pe64::Pe<'a>>(pe: P, known: &Known, version: Option<crate::gamever::ExeVersion>) -> Result<usize, String> {
    let want: Vec<Option<u8>> = known.bytes.split_whitespace().map(|b| u8::from_str_radix(b, 16).ok()).collect();
    let vname = version.map(crate::gamever::ExeVersion::name).unwrap_or("an unknown version");
    if let Some(rva) = crate::gamever::pick_for(version, known.rva, known.rva_2_7_2_0) {
        let code = pe.derva_slice::<u8>(rva as u32, want.len()).map_err(|e| format!("{} at RVA {rva:#x} not readable: {e}", known.name))?;
        // (a jump of somebody else's hook at the start is not ours to follow here: the bytes must be the file's)
        return if code.iter().zip(&want).all(|(have, want)| want.is_none_or(|w| w == *have)) {
            Ok(rva)
        } else {
            Err(format!("{} does not start with the bytes {vname} has at RVA {rva:#x}: not used", known.name))
        };
    }
    let atoms = pelite::pattern::parse(&known.pattern()).map_err(|e| format!("{}: bytes do not parse: {e}", known.name))?;
    let mut save = [0u32; 1];
    let mut matches = pe.scanner().matches_code(&atoms);
    let mut found = Vec::new();
    while matches.next(&mut save) && found.len() < 3 {
        found.push(save[0]);
    }
    match found.as_slice() {
        [rva] => Ok(*rva as usize),
        [] => Err(format!("{}: no RVA recorded for {vname} and its bytes are nowhere", known.name)),
        many => Err(format!("{}: no RVA recorded for {vname} and its bytes are at several places {many:x?}", known.name)),
    }
}

pub(crate) const SET_STATE: Known = Known { name: "TitleStep state setter", rva: 0x00B0_F070, rva_2_7_2_0: 0x00B0_F530 /* (PORT) */, bytes: "40 53 48 83 ec 70 48 c7 44 24 28 fe ff ff ff 48 8b 05 ?? ?? ?? ?? 48 33 c4 48 89 44 24 60 89 51 4c 8b 41 48 ff c0 83 f8 0e" };
pub(crate) const BEGIN_NEW_GAME: Known = Known { name: "TitleStep::STEP_BeginNewGame", rva: 0x00B0_DC50, rva_2_7_2_0: 0x00B0_E110 /* (PORT) */, bytes: "40 53 48 83 ec 20 48 8b 05 ?? ?? ?? ?? 48 8b d9 48 85 c0 75 2e 48 8d 0d ?? ?? ?? ?? e8 ?? ?? ?? ?? 4c 8b c8 4c 8d 05 ?? ?? ?? ?? ba b4 00 00 00" };
pub(crate) const MAKE_BASE_SELECT: Known = Known { name: "ChrMakeBaseSelectDialog factory", rva: 0x008A_3370, rva_2_7_2_0: 0x008A_3820 /* (PORT) */, bytes: "40 57 48 83 ec 50 48 c7 44 24 30 fe ff ff ff 48 89 5c 24 60 48 89 74 24 68 49 8b f9 49 8b f0 48 8b da 4c 8b 05 ?? ?? ?? ?? 4c 89 44 24 38 ba 08 00 00 00 b9 b8 26 00 00" };
pub(crate) const MAKE_TITLE_TOP: Known = Known { name: "TitleTopDialog factory", rva: 0x0081_F460, rva_2_7_2_0: 0x0081_F910 /* (PORT) */, bytes: "48 8b c4 56 57 41 56 48 81 ec 90 00 00 00 48 c7 40 80 fe ff ff ff 48 89 58 08 48 8b f2 4c 8b f1 33 db 89 5c 24 20 4c 8b 05 ?? ?? ?? ?? 4c 89 40 18" };
pub(crate) const MAKE_PROFILE_LOAD: Known = Known { name: "ProfileLoadDialog factory", rva: 0x0081_F950, rva_2_7_2_0: 0x0081_FE00 /* (PORT) */, bytes: "40 56 57 41 56 48 81 ec 90 00 00 00 48 c7 44 24 28 fe ff ff ff 48 89 9c 24 b0 00 00 00 48 8b f2 4c 8b f1 33 db 89 5c 24 20 e8 ?? ?? ?? ?? 4c 8b 05 ?? ?? ?? ??" };

/// The states of `CS::TitleStep` (the game's own names).
pub const STATE_NAMES: [&str; 12] = ["InitMenu", "BeginXR117Dialog", "BeginLogo", "BeginTitle", "BeginNewGame", "PlayGame", "GameStepWait", "EndFlow", "EndFlowWait", "NextLap", "MenuJobWait", "Finish"];
pub const ST_BEGIN_TITLE: i32 = 3;
pub const ST_BEGIN_NEW_GAME: i32 = 4;
pub const ST_PLAY_GAME: i32 = 5;
pub const ST_GAME_STEP_WAIT: i32 = 6;
pub const ST_MENU_JOB_WAIT: i32 = 10;

pub(crate) fn at(known: &Known) -> Option<usize> {
    // (PORT) the version's recorded address (or the bytes' one unique place), read through the mapped image
    let version = crate::gamever::current();
    if let Some(rva) = crate::gamever::pick_for(version, known.rva, known.rva_2_7_2_0) {
        let len = known.bytes.split_whitespace().count();
        if crate::pause::readable(crate::entry::exe_base() + rva, len) < len {
            erlog!("title: {} is not readable at RVA {rva:#x}: not used", known.name);
            return None;
        }
    }
    match known_in(crate::entry::exe_view(), known, version) {
        Ok(rva) => Some(crate::entry::exe_base() + rva),
        Err(why) => {
            erlog!("title: {why}");
            None
        }
    }
}

fn now() -> u64 {
    // SAFETY: plain getter.
    unsafe { GetTickCount64() }
}

static ORIG_SET_STATE: AtomicUsize = AtomicUsize::new(0);
static ORIG_BEGIN_NEW: AtomicUsize = AtomicUsize::new(0);
static ORIG_BASE: AtomicUsize = AtomicUsize::new(0);
static ORIG_TOP: AtomicUsize = AtomicUsize::new(0);
static ORIG_LOAD: AtomicUsize = AtomicUsize::new(0);

/// The title step's state as its setter was last asked (-1: never), and when (ms tick).
static STATE: AtomicI32 = AtomicI32::new(-1);
static STATE_AT: AtomicU64 = AtomicU64::new(0);
/// When each dialog was last made (ms tick; 0 = never in this run).
static TOP_AT: AtomicU64 = AtomicU64::new(0);
static LOAD_AT: AtomicU64 = AtomicU64::new(0);
static BASE_AT: AtomicU64 = AtomicU64::new(0);
/// `STEP_BeginNewGame`: when it was first called for the current new game (0: not in it), calls held back.
static BEGIN_AT: AtomicU64 = AtomicU64::new(0);
static HELD_CALLS: AtomicU32 = AtomicU32::new(0);
/// The hold is asked for until this tick (renewed by `hold_new_game`; it lapses by itself).
static HOLD_UNTIL: AtomicU64 = AtomicU64::new(0);
/// This new game's hold was given up for good (the choices are made).
static RELEASED: AtomicBool = AtomicBool::new(false);
static LOG_LINES: AtomicU32 = AtomicU32::new(0);
static WATCH: AtomicBool = AtomicBool::new(false);

fn told() -> bool {
    WATCH.load(Ordering::Relaxed) && LOG_LINES.fetch_add(1, Ordering::Relaxed) < 400
}

type SetStateFn = unsafe extern "C" fn(*mut u8, i32);
type StepFn = unsafe extern "C" fn(*mut u8, *mut u8);
type FactoryFn = unsafe extern "C" fn(usize, usize, usize, usize, usize, usize) -> usize;

unsafe extern "C" fn set_state_hook(step: *mut u8, state: i32) {
    let original: SetStateFn = std::mem::transmute(ORIG_SET_STATE.load(Ordering::Relaxed));
    let before = STATE.swap(state, Ordering::Relaxed);
    STATE_AT.store(now(), Ordering::Relaxed);
    if state != ST_BEGIN_NEW_GAME {
        BEGIN_AT.store(0, Ordering::Relaxed);
        RELEASED.store(false, Ordering::Relaxed);
    }
    if told() {
        let name = |s: i32| STATE_NAMES.get(s as usize).copied().unwrap_or("?");
        erlog!("title: TitleStep {} ({before}) -> {} ({state})", name(before), name(state));
    }
    original(step, state);
}

unsafe extern "C" fn begin_new_game_hook(step: *mut u8, time: *mut u8) {
    let original: StepFn = std::mem::transmute(ORIG_BEGIN_NEW.load(Ordering::Relaxed));
    let t = now();
    if BEGIN_AT.compare_exchange(0, t, Ordering::Relaxed, Ordering::Relaxed).is_ok() {
        HELD_CALLS.store(0, Ordering::Relaxed);
        STATE.store(ST_BEGIN_NEW_GAME, Ordering::Relaxed);
        STATE_AT.store(t, Ordering::Relaxed);
        if told() {
            erlog!("title: STEP_BeginNewGame is called (a new game begins)");
        }
    }
    // THE HOLD: the state function is not run; the step stays in this state and the game asks again next frame.
    // (it lapses 500 ms after the last renewal, and never lasts longer than 10 minutes)
    if !RELEASED.load(Ordering::Relaxed) && t < HOLD_UNTIL.load(Ordering::Relaxed) && t.saturating_sub(BEGIN_AT.load(Ordering::Relaxed)) < 600_000 {
        HELD_CALLS.fetch_add(1, Ordering::Relaxed);
        return;
    }
    original(step, time);
}

unsafe extern "C" fn base_hook(a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> usize {
    let original: FactoryFn = std::mem::transmute(ORIG_BASE.load(Ordering::Relaxed));
    BASE_AT.store(now(), Ordering::Relaxed);
    // (the cover goes up inside this call: before the frame the dialog would first be drawn in)
    let _ = std::panic::catch_unwind(crate::newgame::on_base_made);
    if told() {
        erlog!("title: ChrMakeBaseSelectDialog is being made (Select Character Base)");
    }
    original(a, b, c, d, e, f)
}

unsafe extern "C" fn top_hook(a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> usize {
    let original: FactoryFn = std::mem::transmute(ORIG_TOP.load(Ordering::Relaxed));
    TOP_AT.store(now(), Ordering::Relaxed);
    if told() {
        erlog!("title: TitleTopDialog is being made (the title menu)");
    }
    original(a, b, c, d, e, f)
}

unsafe extern "C" fn load_hook(a: usize, b: usize, c: usize, d: usize, e: usize, f: usize) -> usize {
    let original: FactoryFn = std::mem::transmute(ORIG_LOAD.load(Ordering::Relaxed));
    LOAD_AT.store(now(), Ordering::Relaxed);
    if told() {
        erlog!("title: ProfileLoadDialog is being made (Load Game)");
    }
    // (IMPL_ER15) KH3's Load screen on the title: the cover goes up inside this call; a list we asked for lists one slot only
    let _ = std::panic::catch_unwind(crate::titleload::before_dialog);
    let made = original(a, b, c, d, e, f);
    let _ = std::panic::catch_unwind(crate::titleload::after_dialog);
    made
}

/// Detours the title flow's functions (once per payload; undone by the renderer's shutdown with every other inline hook).
pub fn install() {
    let mut n = 0;
    let hooks: [(&Known, usize, &AtomicUsize, &'static str); 5] = [
        (&SET_STATE, set_state_hook as *const () as usize, &ORIG_SET_STATE, "title step state"),
        (&BEGIN_NEW_GAME, begin_new_game_hook as *const () as usize, &ORIG_BEGIN_NEW, "title begin new game"),
        (&MAKE_BASE_SELECT, base_hook as *const () as usize, &ORIG_BASE, "character base dialog"),
        (&MAKE_TITLE_TOP, top_hook as *const () as usize, &ORIG_TOP, "title top dialog"),
        (&MAKE_PROFILE_LOAD, load_hook as *const () as usize, &ORIG_LOAD, "profile load dialog"),
    ];
    for (known, hook, original, what) in hooks {
        let Some(f) = at(known) else { continue };
        // SAFETY: the function's start at its recorded address with its recorded bytes; our hook has its signature.
        match unsafe { kh_er_render::inline_hook(f, hook, original, what) } {
            Ok(()) => n += 1,
            Err(e) => {
                original.store(0, Ordering::SeqCst);
                erlog!("title: {} was NOT detoured ({e})", known.name);
            }
        }
    }
    erlog!("title: {n} of 5 functions of the title's flow are detoured (state setter, BeginNewGame, the three dialog factories)");
}

/// The hooks are gone (the renderer's shutdown removed every inline hook).
pub fn uninstalled() {
    for o in [&ORIG_SET_STATE, &ORIG_BEGIN_NEW, &ORIG_BASE, &ORIG_TOP, &ORIG_LOAD] {
        o.store(0, Ordering::SeqCst);
    }
    HOLD_UNTIL.store(0, Ordering::SeqCst);
}

/// The new-game signals can be had (the two functions that give them are detoured).
pub fn hooked() -> bool {
    ORIG_BEGIN_NEW.load(Ordering::Relaxed) != 0 && ORIG_BASE.load(Ordering::Relaxed) != 0 && ORIG_SET_STATE.load(Ordering::Relaxed) != 0
}

/// The title step's state as last asked of its setter (-1: none seen in this run).
pub fn state() -> i32 {
    STATE.load(Ordering::Relaxed)
}

/// Milliseconds since "Select Character Base" was last made (`None`: never).
pub fn base_made_ago() -> Option<u64> {
    let t = BASE_AT.load(Ordering::Relaxed);
    (t != 0).then(|| now().saturating_sub(t))
}

/// Milliseconds since the Load Game list was last made (`None`: never).
pub fn load_made_ago() -> Option<u64> {
    let t = LOAD_AT.load(Ordering::Relaxed);
    (t != 0).then(|| now().saturating_sub(t))
}

/// Milliseconds since the title menu was last made (`None`: never).
pub fn top_made_ago() -> Option<u64> {
    let t = TOP_AT.load(Ordering::Relaxed);
    (t != 0).then(|| now().saturating_sub(t))
}

/// `STEP_BeginNewGame` is being called (held or not): milliseconds since its first call.
pub fn begin_new_game_for() -> Option<u64> {
    let t = BEGIN_AT.load(Ordering::Relaxed);
    (t != 0).then(|| now().saturating_sub(t))
}

/// Renews the hold of `STEP_BeginNewGame` for the next 500 ms (call every frame while the flow must wait).
pub fn hold_new_game() {
    HOLD_UNTIL.store(now() + 500, Ordering::Relaxed);
}

/// The held new game goes on now and is not held again.
pub fn release() {
    RELEASED.store(true, Ordering::Relaxed);
}

/// Milliseconds since the title step's state was last set.
pub fn state_for() -> u64 {
    now().saturating_sub(STATE_AT.load(Ordering::Relaxed))
}

pub fn let_go() {
    HOLD_UNTIL.store(0, Ordering::Relaxed);
}

/// Calls of the state function that were held back since the new game began.
pub fn held_calls() -> u32 {
    HELD_CALLS.load(Ordering::Relaxed)
}

/// `dbg.title.watch = 1`: every signal is logged.
pub fn watch(on: bool) {
    WATCH.store(on, Ordering::Relaxed);
}

/// (IMPL_ER15) The Load Game list's factory and the state setter are detoured (titleload.rs needs both).
pub fn load_hooked() -> bool {
    ORIG_LOAD.load(Ordering::Relaxed) != 0 && ORIG_SET_STATE.load(Ordering::Relaxed) != 0
}
