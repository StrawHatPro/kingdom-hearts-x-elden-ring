//! (IMPL_ER11) THE GAME'S OWN PAUSE (build/sora_tmp/er/IMPL_ER11_STATE.md).
//!
//! WHAT THE GAME DOES (eldenring.exe 2.7.1.0, read from the file): the in-game step's update (RVA 0xaf9000,
//! `fn(step, FD4Time*)`) decides EVERY FRAME whether the world is updated: the byte `step + 0x4B8` (1 = the
//! world runs, 0 = THE WORLD STANDS, drawing goes on) is computed from, in this order:
//!   `step + 0x12A` or `GameMan + 0xBC4` != 0 -> 0;   `step + 0x128` (off in the retail game; the public
//!   "pause the game" mods flip the jump after its compare at RVA 0xaf972e) -> 0 unless the menu job says idle;
//!   `WorldChrMan + 0x1E524 == 2` -> 0 (+0x4B9 = 1);   **`step + 0x348` != 0 -> 0**;   `step + 0x349` (one
//!   shot) -> 1 (a single frame's advance);   the global byte at RVA 0x3D748B8 (one shot) -> 0.
//! and hands it to the two world updates (0xb025f0, 0xb038d0) with the frame's delta.
//!
//! WHAT WE DO: the step's update is detoured (found by pattern, used only at its 2.7.1.0 address) only to
//! set, on the game's own thread right before the game reads it, THE SAME ONE-SHOT BYTE A TUTORIAL POPUP SETS
//! (`POPUP_BYTE` below; `menu.pause = hold`: the step's sticky byte +0x348 instead): while a menu of ours renews the request (`hold`; the request LAPSES 400 ms after its last renewal:
//! the watchdog is the hook itself, which the game calls every frame whether the world runs or not), 0 again
//! when it lapses. Nothing else of the game is written; no code is patched.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

use windows_sys::Win32::System::Memory::{VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, PAGE_GUARD, PAGE_NOACCESS};
use windows_sys::Win32::System::SystemInformation::GetTickCount64;

use crate::control::{Control, OneShot};
use crate::erlog;
use crate::havok::{locate, Located};

pub(crate) const STEP_FN: Located = Located {
    name: "in-game step update (the world's pause decision)",
    pattern: "48 8b c4 55 56 57 41 54 41 55 41 56 41 57 48 8d 68 a1 48 81 ec d0 00 00 00 48 c7 45 a7 fe ff ff ff 48 89 58 18 0f 29 70 b8 0f 29 78 a8 44 0f 29 40 98 44 0f 29 48 88 44 0f 29 90 78 ff ff ff",
    capture: 0,
    rva_2_7_1_0: 0x00af_9000, rva_2_7_2_0: 0x00af_94c0 /* (PORT) */,
};

/// THE TUTORIAL POPUP'S OWN ROUTE (MEASURED in the game with a help popup and the tutorial "New Inventory Features"
/// open: +0x4B8 0, +0x4BA 0, no byte of the step object set): the one-shot global byte the step reads at
/// RVA 0xaf9ab7 (`cmp byte [rip+..], 0; je; mov byte [rip+..], 0; mov byte [step+4B8], 0; mov byte [step+4BA], 0`),
/// which whatever shows the popup sets again every frame. We set it the same way, on the game's thread, right
/// before the step reads it; the game clears it itself, so nothing is ever left behind.
pub(crate) const POPUP_CHECK_IN_FN: usize = 0xab7;
pub(crate) const POPUP_BYTE_RVA: usize = 0x03D7_48B8;
/// (PORT) The same byte in 2.7.2.0 (6 of 6 references carried over; the compare at step + 0xab7 is unchanged).
pub(crate) const POPUP_BYTE_RVA_2720: usize = 0x03D7_78D8;
static POPUP_BYTE: AtomicUsize = AtomicUsize::new(0);
/// `menu.pause = hold`: the step's own sticky byte (+0x348) instead (the world stands, +0x4BA stays 1).
static USE_HOLD_BYTE: AtomicBool = AtomicBool::new(false);

/// The step's "hold the world" byte (see the header), its result and the other inputs (for the log).
const HOLD: usize = 0x348;
const RUNS: usize = 0x4B8;

static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
/// The step object of the latest call (0: none yet / not in the game).
static STEP: AtomicUsize = AtomicUsize::new(0);
static STEP_AT: AtomicU64 = AtomicU64::new(0);
/// The pause is wanted until this tick (ms).
static WANT_UNTIL: AtomicU64 = AtomicU64::new(0);
/// The hold byte holds OUR 1.
static HELD: AtomicBool = AtomicBool::new(false);
/// The latest step call was made with our request in (either route).
static OURS: AtomicBool = AtomicBool::new(false);
/// Frames of the game's in which the world stood by our request / calls of the step (for the log).
pub static HELD_FRAMES: AtomicU32 = AtomicU32::new(0);
pub static CALLS: AtomicU32 = AtomicU32::new(0);
/// The inputs and the result as the latest call left them (packed; `watch`).
static FACTS: AtomicU64 = AtomicU64::new(0);
static WATCH: AtomicBool = AtomicBool::new(false);
static WATCH_LINES: AtomicU32 = AtomicU32::new(0);

type StepFn = unsafe extern "C" fn(*mut u8, *mut u8);

/// (nothing in here can panic: loads, stores, single byte reads / writes inside the live step object)
unsafe extern "C" fn step_hook(step: *mut u8, time: *mut u8) {
    let original: StepFn = std::mem::transmute(ORIGINAL.load(Ordering::Relaxed));
    let ok = crate::game::plausible(step as usize);
    let now = GetTickCount64();
    if ok {
        STEP.store(step as usize, Ordering::Relaxed);
        STEP_AT.store(now, Ordering::Relaxed);
        let flag = step.add(HOLD);
        let popup = POPUP_BYTE.load(Ordering::Relaxed);
        let want = now < WANT_UNTIL.load(Ordering::Relaxed);
        if want && popup != 0 && !USE_HOLD_BYTE.load(Ordering::Relaxed) {
            *(popup as *mut u8) = 1;
            OURS.store(true, Ordering::Relaxed);
            if HELD.swap(false, Ordering::Relaxed) {
                *flag = 0;
            }
        } else if want {
            if *flag == 0 {
                *flag = 1;
                HELD.store(true, Ordering::Relaxed);
            }
            OURS.store(true, Ordering::Relaxed);
        } else {
            if HELD.swap(false, Ordering::Relaxed) {
                *flag = 0;
            }
            OURS.store(false, Ordering::Relaxed);
        }
    }
    original(step, time);
    CALLS.fetch_add(1, Ordering::Relaxed);
    if ok {
        if OURS.load(Ordering::Relaxed) && *step.add(RUNS) == 0 {
            HELD_FRAMES.fetch_add(1, Ordering::Relaxed);
        }
        let b = |o: usize| u64::from(*step.add(o));
        let facts = b(0x128) | b(0x129) << 4 | b(0x12A) << 8 | b(0x12B) << 12 | b(0x344) << 16 | b(HOLD) << 20 | b(0x349) << 24 | b(RUNS) << 28 | b(0x4B9) << 32 | b(0x4BA) << 36 | (u64::from(std::ptr::read_unaligned(step.add(0x100) as *const u32)) & 0xFF) << 40;
        if FACTS.swap(facts, Ordering::Relaxed) != facts && WATCH.load(Ordering::Relaxed) && WATCH_LINES.fetch_add(1, Ordering::Relaxed) < 400 {
            let n = |s: u32| facts >> s & 0xF;
            erlog!("pause: step {:#x}: +128 {} +129 {} +12A {} +12B {} +344 {} +348 {} +349 {} | world runs (+4B8) {} +4B9 {} +4BA {} | state (+100) {}", step as usize, n(0), n(4), n(8), n(12), n(16), n(20), n(24), n(28), n(32), n(36), facts >> 40 & 0xFF);
        }
    }
}

/// Detours the in-game step's update (once per payload; undone by the renderer's shutdown with every other
/// inline hook). False: the game's own pause cannot be asked for (`menu.pause` then falls back to the speed).
pub fn install() -> bool {
    let Some(f) = locate(&STEP_FN) else {
        erlog!("pause: the in-game step's update was not found for this executable (PORT: 2.7.1.0 / 2.7.2.0 table): the game's own pause is not available");
        return false;
    };
    // The popup's byte: read from the function's own compare (must be the 2.7.1.0 global).
    // SAFETY: bytes of the function just located, inside the mapped executable.
    unsafe {
        let at = f + POPUP_CHECK_IN_FN;
        let code = std::slice::from_raw_parts(at as *const u8, 7);
        if code[0] == 0x80 && code[1] == 0x3d && code[6] == 0 {
            let target = (at + 7).wrapping_add(i32::from_le_bytes([code[2], code[3], code[4], code[5]]) as isize as usize);
            // (PORT) the running version's recorded byte
            if crate::gamever::pick(POPUP_BYTE_RVA, POPUP_BYTE_RVA_2720).map(|r| crate::entry::exe_base() + r) == Some(target) {
                POPUP_BYTE.store(target, Ordering::SeqCst);
            }
        }
    }
    erlog!("pause: the tutorial popup's pause byte {}", if POPUP_BYTE.load(Ordering::Relaxed) != 0 { "is found (the popup's own route is used)" } else { "was NOT found where this executable has it (PORT): the step's hold byte is used instead" });
    // SAFETY: the function's start, found by pattern at the recorded address; our hook has its signature.
    match unsafe { kh_er_render::inline_hook(f, step_hook as *const () as usize, &ORIGINAL, "in-game step") } {
        Ok(()) => {
            erlog!("pause: the in-game step's update is detoured (the world can be held the game's own way while a Kingdom Hearts menu is open)");
            true
        }
        Err(e) => {
            ORIGINAL.store(0, Ordering::SeqCst);
            erlog!("pause: the in-game step's update was NOT detoured ({e}): the game's own pause is not available");
            false
        }
    }
}

/// Before the hooks are removed (the payload's shutdown): our hold byte is taken back now.
pub fn release() {
    WANT_UNTIL.store(0, Ordering::SeqCst);
    let step = STEP.load(Ordering::Relaxed);
    // SAFETY: the step object the game's own call gave us less than a second ago; one byte of it.
    if HELD.swap(false, Ordering::SeqCst) && crate::game::plausible(step) && unsafe { GetTickCount64() }.saturating_sub(STEP_AT.load(Ordering::Relaxed)) < 1000 {
        unsafe { *(step as *mut u8).add(HOLD) = 0 };
    }
}

/// The hooks are gone (the renderer's shutdown removed every inline hook).
pub fn uninstalled() {
    ORIGINAL.store(0, Ordering::SeqCst);
    STEP.store(0, Ordering::SeqCst);
    HELD.store(false, Ordering::SeqCst);
    OURS.store(false, Ordering::SeqCst);
    POPUP_BYTE.store(0, Ordering::SeqCst);
}

/// The game's own pause can be asked for right now: the step is detoured and the game called it lately.
pub fn available() -> bool {
    // SAFETY: plain getter.
    ORIGINAL.load(Ordering::Relaxed) != 0 && unsafe { GetTickCount64() }.saturating_sub(STEP_AT.load(Ordering::Relaxed)) < 500
}

/// The step's update is detoured.
pub fn hooked() -> bool {
    ORIGINAL.load(Ordering::Relaxed) != 0
}

/// Renews the pause for the next 400 ms (call every frame while a menu of ours is open).
pub fn hold() {
    // SAFETY: plain getter.
    WANT_UNTIL.store(unsafe { GetTickCount64() } + 400, Ordering::Relaxed);
}

/// The pause ends with the game's next frame.
pub fn let_go() {
    WANT_UNTIL.store(0, Ordering::Relaxed);
}

/// The world stands by our request (as of the game's latest frame).
pub fn holding() -> bool {
    OURS.load(Ordering::Relaxed)
}

/// `menu.pause = hold`: the step's sticky byte instead of the popup's one-shot byte.
pub fn use_hold_byte(on: bool) {
    USE_HOLD_BYTE.store(on, Ordering::Relaxed);
}

/// The world's update flag as the game's latest frame computed it (`None`: no step object lately).
pub fn world_runs() -> Option<bool> {
    available().then(|| FACTS.load(Ordering::Relaxed) >> 28 & 0xF != 0)
}

// ---------------------------------------------------------------- testing aids

struct Snap {
    label: String,
    regions: Vec<(String, usize, Vec<u8>)>,
}

struct Aids {
    snap: Option<OneShot>,
    diff: Option<OneShot>,
    hold: Option<OneShot>,
    snaps: Vec<Snap>,
    test_until: u64,
}

static AIDS: Mutex<Aids> = Mutex::new(Aids { snap: None, diff: None, hold: None, snaps: Vec::new(), test_until: 0 });

/// The bytes at `base` that can be read (committed, not guarded), at most `len`.
pub fn readable(base: usize, len: usize) -> usize {
    if !crate::game::plausible(base & !7) {
        return 0;
    }
    let mut at = base;
    let end = base + len;
    while at < end {
        // SAFETY: VirtualQuery on any address is fine; the structure is plain.
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { VirtualQuery(at as *const _, &mut info, std::mem::size_of::<MEMORY_BASIC_INFORMATION>()) } == 0 || info.State != MEM_COMMIT || info.Protect & (PAGE_GUARD | PAGE_NOACCESS) != 0 || info.Protect == 0 {
            break;
        }
        let next = (info.BaseAddress as usize).saturating_add(info.RegionSize);
        // (never loop on a region that does not move the cursor on)
        if next <= at {
            break;
        }
        at = next;
    }
    at.min(end).saturating_sub(base)
}

/// What a snapshot holds: the managers the pause could live in (name, address, bytes asked for).
fn regions() -> Vec<(String, usize, usize)> {
    use eldenring::cs::{CSFeManImp, CSFlipper, CSMenuManImp, CSRemo, GameMan};
    use fromsoftware_shared::FromStatic;
    let mut out = Vec::new();
    // SAFETY: game thread; the singletons live as long as the game; only their addresses are taken.
    unsafe {
        if let Ok(o) = CSFeManImp::instance() {
            out.push(("CSFeManImp".to_string(), o as *const _ as usize, 0x80));
        }
        if let Ok(o) = CSMenuManImp::instance() {
            let at = o as *const _ as usize;
            out.push(("CSMenuManImp".to_string(), at, 0x900));
            let data = std::ptr::read_unaligned((at + 8) as *const usize);
            out.push(("CSMenuManImp.menu_data".to_string(), data, 0xF0));
        }
        if let Ok(o) = CSFlipper::instance() {
            out.push(("CSFlipper".to_string(), o as *const _ as usize, 0x300));
        }
        if let Ok(o) = GameMan::instance() {
            out.push(("GameMan".to_string(), o as *const _ as usize, 0xE00));
        }
        if let Ok(o) = CSRemo::instance() {
            out.push(("CSRemo".to_string(), o as *const _ as usize, 0x80));
        }
    }
    // SAFETY: as above.
    unsafe {
        if let Ok(o) = eldenring::cs::GameDataMan::instance() {
            let at = o as *const _ as usize;
            out.push(("GameDataMan".to_string(), at, 0x120));
            out.push(("PlayerGameData".to_string(), std::ptr::read_unaligned((at + 8) as *const usize), 0x300));
        }
    }
    let step = STEP.load(Ordering::Relaxed);
    if step != 0 {
        out.push(("InGameStep".to_string(), step, 0x500));
    }
    out
}

fn snapshot(label: &str) -> Snap {
    let mut snap = Snap { label: label.to_string(), regions: Vec::new() };
    for (name, base, len) in regions() {
        let n = readable(base, len);
        // SAFETY: `n` bytes at `base` are committed and readable (checked just now, on the game's thread).
        let bytes = unsafe { std::slice::from_raw_parts(base as *const u8, n) }.to_vec();
        snap.regions.push((name, base, bytes));
    }
    snap
}

fn diff(a: &Snap, b: &Snap) {
    let mut lines = 0;
    erlog!("pause: DIFF {} -> {}", a.label, b.label);
    for (name, base, before) in &a.regions {
        let Some((_, _, after)) = b.regions.iter().find(|(n, at, _)| n == name && at == base) else {
            erlog!("pause:   {name}: not in both (or moved)");
            continue;
        };
        let n = before.len().min(after.len());
        let mut k = 0;
        while k < n {
            if before[k] == after[k] {
                k += 1;
                continue;
            }
            let start = k & !3;
            let mut end = start + 4;
            while end < n && end - start < 16 && before[end..(end + 4).min(n)] != after[end..(end + 4).min(n)] {
                end += 4;
            }
            let end = end.min(n);
            let hex = |s: &[u8]| s.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ");
            if lines < 160 {
                erlog!("pause:   {name} +{start:#06x}: {} -> {}", hex(&before[start..end]), hex(&after[start..end]));
            }
            lines += 1;
            k = end;
        }
    }
    erlog!("pause: DIFF end ({lines} place(s){})", if lines > 160 { ", 160 shown" } else { "" });
}

/// Once a frame (FRAME_BEGIN): the testing aids. `dbg.pause.watch = 1`: every change of the step's pause
/// inputs / result is logged; `dbg.pause.snap = label` (one-shot): a snapshot of the managers; `dbg.pause.diff =
/// a,b` (one-shot): what differs between two snapshots; `dbg.pause.hold = seconds` (one-shot): the game's own
/// pause is asked for that long with no menu of ours (capped at 20 s).
pub fn frame(control: &Control, tick: bool) {
    WATCH.store(control.on("dbg.pause.watch"), Ordering::Relaxed);
    let Ok(mut a) = AIDS.lock() else { return };
    if a.snap.is_none() {
        a.snap = Some(OneShot::new("dbg.pause.snap", control));
        a.diff = Some(OneShot::new("dbg.pause.diff", control));
        a.hold = Some(OneShot::new("dbg.pause.hold", control));
    }
    if let Some(label) = a.snap.as_mut().and_then(|s| s.fired(control)) {
        let snap = snapshot(&label);
        erlog!("pause: snapshot '{label}': {}", snap.regions.iter().map(|(n, at, b)| format!("{n} {at:#x} {:#x}", b.len())).collect::<Vec<_>>().join(", "));
        a.snaps.retain(|s| s.label != label);
        if a.snaps.len() >= 8 {
            a.snaps.remove(0);
        }
        a.snaps.push(snap);
    }
    if let Some(pair) = a.diff.as_mut().and_then(|s| s.fired(control)) {
        let mut names = pair.split(',').map(str::trim);
        match (names.next().and_then(|n| a.snaps.iter().find(|s| s.label == n)), names.next().and_then(|n| a.snaps.iter().find(|s| s.label == n))) {
            (Some(x), Some(y)) => diff(x, y),
            _ => erlog!("pause: dbg.pause.diff = {pair}: no such snapshots"),
        }
    }
    if let Some(seconds) = a.hold.as_mut().and_then(|s| s.fired(control)).and_then(|v| v.parse::<f32>().ok()) {
        // SAFETY: plain getter.
        a.test_until = unsafe { GetTickCount64() } + (seconds.clamp(0.0, 20.0) * 1000.0) as u64;
        erlog!("pause: dbg.pause.hold: the game's own pause is asked for {seconds} s");
    }
    // SAFETY: plain getter.
    if unsafe { GetTickCount64() } < a.test_until {
        hold();
    }
    if tick && (holding() || control.on("dbg.pause.watch")) {
        erlog!("pause: step calls {} | world held by us in {} frame(s) | holding {} | world runs {:?}", CALLS.swap(0, Ordering::Relaxed), HELD_FRAMES.swap(0, Ordering::Relaxed), holding(), world_runs());
    }
}
