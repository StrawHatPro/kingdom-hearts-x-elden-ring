//! (IMPL_ER13) ELDEN RING'S TUTORIALS ARE NOT SHOWN WHEN PLAYING AS SORA (`tutorials.er = 0`, the default; the user:
//! "all tutorial notifications need to be removed"): they explain Elden Ring's controls, which do not apply, and
//! THEY BROKE THE GAME FOR A PAD PLAYER: a modal tutorial ("The Cave of Knowledge", closed with the Menu button)
//! pauses Elden Ring's world, the Menu button is Sora's (the Kingdom Hearts menu) and never reached it, so the
//! window could not be closed with the pad: Sora ran around a world that stood still, in which every prompt of
//! the game is dead ("they all failed, the prompt was greyed out").
//!
//! WHAT THE GAME HAS (eldenring.exe 2.7.1.0, read from the file; RTTI `CS::CSTutorialData`, `CSTutorialRequestJob`,
//! `CSTutorialModalDialog`, `CSTutorialToastDialog`): tutorials that were asked for (by event scripts, by first
//! pick-ups ...) wait in a list of `CSTutorialData`; the interface asks ONE FUNCTION (RVA 0x794DF0,
//! `fn(out: *mut Job, kind: u8)`) for a job that shows the waiting tutorials of a kind (the modal windows, the
//! toasts); that function answers "no job" by itself when `kind` is 0 or nothing of the kind waits.
//!
//! WHAT WE DO: that function is detoured and, with `tutorials.er = 0`, called with kind 0: its own "nothing to
//! show" path runs: no dialog is created, nothing pauses, nothing is written. `tutorials.er = 1` passes the call
//! through unchanged (the tutorials that waited meanwhile are then shown).

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use crate::erlog;
use crate::title::{at, Known};

pub(crate) const MAKE_JOB: Known = Known { name: "tutorial request job maker", rva: 0x0079_4DF0, rva_2_7_2_0: 0x0079_52A0 /* (PORT) */, bytes: "48 89 4c 24 08 55 53 56 57 41 54 41 55 41 56 41 57 48 8d 6c 24 e1 48 81 ec 88 00 00 00 48 c7 45 ef fe ff ff ff 44 0f b6 e2 4c 8b f9 45 33 ed" };

static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
/// `tutorials.er = 1`: the game's tutorials are shown as it means them.
static ALLOW: AtomicBool = AtomicBool::new(false);
/// Calls answered "nothing to show" per kind (1, 2, other), for the log.
static REFUSED: [AtomicU32; 3] = [AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)];

type MakeFn = unsafe extern "C" fn(usize, usize, usize, usize) -> usize;

/// (nothing in here can panic: loads, one counter, one log line per kind)
unsafe extern "C" fn make_hook(out: usize, kind: usize, c: usize, d: usize) -> usize {
    let original: MakeFn = std::mem::transmute(ORIGINAL.load(Ordering::Relaxed));
    if ALLOW.load(Ordering::Relaxed) || kind & 0xFF == 0 {
        return original(out, kind, c, d);
    }
    let k = match kind & 0xFF {
        1 => 0,
        2 => 1,
        _ => 2,
    };
    if REFUSED[k].fetch_add(1, Ordering::Relaxed) == 0 {
        erlog!("tutorials: the game asks for its waiting tutorials of kind {} : answered \"none\" (tutorials.er = 0: Elden Ring's tutorial windows and toasts are not shown while playing as Sora)", kind & 0xFF);
    }
    // (kind 0 = the function's own "no job" answer; the high bytes of the register are not the argument's)
    original(out, kind & !0xFF, c, d)
}

pub fn install() {
    let Some(f) = at(&MAKE_JOB) else {
        erlog!("tutorials: the tutorial job maker is not where this executable has it (PORT): Elden Ring's tutorials are shown");
        return;
    };
    // SAFETY: the function's start at its recorded address with its recorded bytes; the hook passes its register arguments on.
    match unsafe { kh_er_render::inline_hook(f, make_hook as *const () as usize, &ORIGINAL, "tutorial request job") } {
        Ok(()) => erlog!("tutorials: the game's tutorial job maker is detoured (tutorials.er = 0, the default: no tutorial window or toast is made)"),
        Err(e) => {
            ORIGINAL.store(0, Ordering::SeqCst);
            erlog!("tutorials: the tutorial job maker was NOT detoured ({e}): Elden Ring's tutorials are shown");
        }
    }
}

pub fn uninstalled() {
    ORIGINAL.store(0, Ordering::SeqCst);
}

/// Once a frame (FRAME_BEGIN): the switch.
pub fn frame(control: &crate::control::Control) {
    let allow = control.on("tutorials.er");
    if ALLOW.swap(allow, Ordering::Relaxed) != allow {
        erlog!("tutorials: tutorials.er = {}: Elden Ring's tutorials are {}", u8::from(allow), if allow { "shown as the game means them" } else { "not shown" });
    }
    option(!allow);
}

/// THE GAME'S OWN OPTION "Show Tutorials" (`GameDataMan -> game_settings.show_tutorials`; Game Options in its system
/// menu): OFF while tutorials are not wanted: the windows that EVENT SCRIPTS raise in the world ("The Cave of
/// Knowledge": SEEN to come although the menus' job maker answered "none") obey it. The value the game had is
/// remembered and put back when `tutorials.er = 1` or the payload goes (the game stores the option in its system
/// save: while Sora's payload runs that is "off").
static OWN: std::sync::Mutex<Option<bool>> = std::sync::Mutex::new(None);

fn option(off: bool) {
    use eldenring::cs::GameDataMan;
    use fromsoftware_shared::FromStatic;
    // SAFETY: game thread; the singleton lives as long as the game; the pointer is checked before it is followed.
    let Ok(data) = (unsafe { GameDataMan::instance_mut() }) else { return };
    if !crate::game::plausible(crate::word!(data.game_settings)) {
        return;
    }
    // SAFETY: a one-byte field of the live settings object (read as a byte: an odd value must not become a bool).
    let field = std::ptr::addr_of_mut!(data.game_settings.show_tutorials) as *mut u8;
    let now = unsafe { *field };
    let Ok(mut own) = OWN.lock() else { return };
    if off {
        if now != 0 {
            if own.is_none() {
                *own = Some(true);
            }
            unsafe { *field = 0 };
            erlog!("tutorials: the game's own option \"Show Tutorials\" was ON: switched OFF (tutorials.er = 0)");
        } else if own.is_none() {
            *own = Some(false);
        }
    } else if let Some(before) = own.take() {
        if before && now == 0 {
            unsafe { *field = 1 };
            erlog!("tutorials: the game's own option \"Show Tutorials\" is put back ON");
        }
    }
}

/// The payload goes away: the option is the game's again.
pub fn shutdown() {
    option(false);
}

/// The once-a-second line's part.
pub fn summary() -> String {
    format!("tutorials: {} | requests answered \"none\": modal {} toast {} other {}", if ORIGINAL.load(Ordering::Relaxed) == 0 { "NOT detoured" } else if ALLOW.load(Ordering::Relaxed) { "shown (tutorials.er = 1)" } else { "refused" }, REFUSED[0].load(Ordering::Relaxed), REFUSED[1].load(Ordering::Relaxed), REFUSED[2].load(Ordering::Relaxed))
}
