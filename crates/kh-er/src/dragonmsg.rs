//! (ER-TWEAKS, THE USER 2026-11-06: "Remove Elden Ring's pop up talking about new draconic powers being available after
//! defeating dragons.") ELDEN RING'S "A new draconic power is available at the Cathedral of Dragon Communion" IS NEVER SHOWN.
//!
//! WHAT THE GAME DOES (read from its own files: build/sora_tmp/er/er33/game/event, the FMGs of build/sora_tmp/er/conv/game/fmg;
//! the code read in both executables):
//! - The text is EventTextForMap 30060 .. 30066 (seven entries with the same words, one per dragon; base game, dlc01 and
//!   dlc02 binders alike; no `_dlc01` / `_dlc02` entry of that kind exists).
//! - It is shown by ONE event instruction, EMEVD 2007[4] (the community's DisplayBlinkingMessage), whose only argument is the
//!   message id:
//!   - common_func event 90005861 (a field dragon's defeat: banner, its defeat flags X0 / X4 SET FIRST, then 5 s, its heart's
//!     item lot X16 awarded, 2 s, then `2007[4](X20)`), started by m60_43_36_00 (entity 1043360800, X20 30060), m60_34_45_00
//!     (1034450800, 30061), m60_35_53_00 (1035530800, 30062), m60_48_37_00 (1048370800, 30064), m60_50_56_00 (1050560800,
//!     30065), m60_54_56_00 (1054560800, 30066);
//!   - m60_50_40_00 event 1050402205 (entity 1050400800: waits for its death, 2 s, `2007[4](30063)`; it sets no flag).
//! - The instruction is the LAST step of each of those events: nothing after it depends on it.
//! - Bank 2007's dispatcher (2.7.1.0 RVA 0x571370 / 2.7.2.0 0x571820; one caller each, the bank switch 0x140568E62 /
//!   0x140569312) takes the event's context in r8: `[r8 + 0xD0]` the instruction (+4 its index in the bank, +0x10 the offset of
//!   its arguments), `[r8 + 0xD8]` the arguments when already resolved, else `[[r8 + 0xC8] + 8]` + `[that + 0x78]` + the offset
//!   (0x140CE2340 / 0x140CE3A30, the same 9 instructions in both). Case 4 reads `[args]` as the message id, looks it up
//!   (0x140D11860: category 34 = EventTextForMap, then 367 its DLC table) and shows it (0x1405F71F0 / 0x1405F76A0). Every case
//!   that handled its instruction returns 1; the caller then steps the event on (0x140584FD0).
//!
//! THE FIX: the dispatcher is detoured. For instruction 4 whose message id is one of [`DRACONIC_TEXTS`] it returns 1 at once
//! (handled, as the game's own case does after showing it): the event goes on exactly as before, nothing is shown. Every
//! other instruction of the bank and every other message of 2007[4] (common_func 20600 / 4200, the maps' own) goes to the
//! game unchanged. Nothing of the game's is written: the defeat flags, the banner rule (erkeyui) and the heart's award are the
//! event's own, before the message. Key `er.dragon_popup = 1` gives the popup back (read every frame).
//!
//! CONFIDENCE: read from the scripts and both executables; NOT SEEN IN THE GAME (agents never launch it).

use std::ops::RangeInclusive;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicUsize, Ordering};

use crate::control::Control;
use crate::erlog;
use crate::havok::{locate, Located};

/// The bank-2007 dispatcher's start: prologue, `mov rdi, r8; mov rax, [r8 + 0xD0]; mov ecx, [rax + 4]; dec ecx;
/// cmp ecx, 15; ja ..`, its jump table (rel32s, the cookie and the table's offset masked).
const PAT_BANK_2007: &str = "48 8b c4 55 41 54 41 55 41 56 41 57 48 8d 68 c8 48 81 ec 10 01 00 00 48 c7 44 24 70 fe ff ff ff 48 89 58 08 48 89 70 10 48 89 78 20 0f 29 70 c8 48 8b 05 ? ? ? ? 48 33 c4 48 89 45 f0 49 8b f8 49 8b 80 d0 00 00 00 8b 48 04 ff c9 83 f9 0f 0f 87 ? ? ? ? 8b c1 48 8d 15 ? ? ? ? 8b 8c 8a ? ? ? ? 48 03 ca ff e1";

pub(crate) const BANK_2007_FN: Located = Located { name: "EMEVD bank 2007 dispatcher (the event messages; ER-TWEAKS)", pattern: PAT_BANK_2007, capture: 0, rva_2_7_1_0: 0x0057_1370, rva_2_7_2_0: 0x0057_1820 };

/// EMEVD 2007[4]: the message instruction the dragons' events use.
pub const MESSAGE_INSTR: i32 = 4;
/// EventTextForMap ids of "A new draconic power is available at the Cathedral of Dragon Communion".
pub const DRACONIC_TEXTS: RangeInclusive<i32> = 30060..=30066;

/// THE PURE RULE: an instruction of bank 2007 is answered "handled" without showing anything when the rule is on, it is
/// instruction 4 and its message is one of the draconic-power texts.
pub fn suppress(on: bool, instr: i32, message: i32) -> bool {
    on && instr == MESSAGE_INSTR && DRACONIC_TEXTS.contains(&message)
}

type Bank2007 = unsafe extern "C" fn(usize, usize, usize, usize) -> u8;

static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static ON: AtomicBool = AtomicBool::new(true);
static SKIPPED: AtomicU32 = AtomicU32::new(0);
static LAST: AtomicI32 = AtomicI32::new(0);
static TOLD: AtomicU32 = AtomicU32::new(0);
static TOLD_ON: AtomicU32 = AtomicU32::new(u32::MAX);

/// The instruction's index in its bank and its first argument (the message id of 2007[4]); None when unreadable.
/// Read only through the system's copy (probe.rs): a bad pointer gives None, never a fault.
fn instruction(ctx: usize) -> Option<(i32, Option<i32>)> {
    let ins = crate::probe::word(ctx + 0xD0)?;
    let index = crate::probe::u32_at(ins + 4)? as i32;
    if index != MESSAGE_INSTR {
        return Some((index, None));
    }
    let args = match crate::probe::word(ctx + 0xD8).filter(|a| *a != 0) {
        Some(a) => a,
        None => {
            // 0x140CE2340: [[ctx + 0xC8] + 8] + [that + 0x78] + the instruction's argument offset (none when -1)
            let base = crate::probe::word(crate::probe::word(ctx + 0xC8)? + 8).filter(|b| *b != 0)?;
            let rel = crate::probe::word(base + 0x78).filter(|r| *r != usize::MAX)?;
            base.checked_add(rel)?.checked_add(crate::probe::word(ins + 0x10)?)?
        }
    };
    Some((index, crate::probe::u32_at(args).map(|m| m as i32)))
}

unsafe extern "C" fn bank_2007_hook(handler: usize, rdx: usize, ctx: usize, r9: usize) -> u8 {
    // (ER-POPUPS) the event-script boxes (2007[1] messages, 2007[10] questions) as KH3's window: crate::erdialog
    match crate::erdialog::on_instruction(ctx) {
        crate::erdialog::Route::Handled => return 1,
        crate::erdialog::Route::Suppress => return crate::erdialog::suppressed(|| original_2007(handler, rdx, ctx, r9)),
        crate::erdialog::Route::Game => {}
    }
    if ON.load(Ordering::Relaxed) {
        if let Some((index, Some(message))) = instruction(ctx) {
            if suppress(true, index, message) {
                LAST.store(message, Ordering::Relaxed);
                SKIPPED.fetch_add(1, Ordering::Relaxed);
                return 1;
            }
        }
    }
    original_2007(handler, rdx, ctx, r9)
}

/// The game's own dispatcher (its trampoline).
unsafe fn original_2007(handler: usize, rdx: usize, ctx: usize, r9: usize) -> u8 {
    let original = ORIGINAL.load(Ordering::Relaxed);
    if original == 0 {
        return 0;
    }
    // SAFETY: the trampoline of the game's own function; the four argument registers passed through unchanged.
    let f: Bank2007 = std::mem::transmute(original);
    f(handler, rdx, ctx, r9)
}

/// (entry.rs) Detours the bank-2007 dispatcher (once per payload; undone by the renderer's shutdown with every other
/// inline hook).
pub fn install() {
    let Some(at) = locate(&BANK_2007_FN) else {
        erlog!("dragonmsg: (ER-TWEAKS) the event messages' dispatcher (EMEVD bank 2007) is not located: Elden Ring's draconic-power popup still shows");
        return;
    };
    // SAFETY: the function's start at its located address (pattern + recorded RVA); our hook takes its four register
    // arguments and passes them on unchanged (it has no stack arguments: the bank switch calls it with rcx, r8 and r9).
    match unsafe { kh_er_render::inline_hook(at, bank_2007_hook as *const () as usize, &ORIGINAL, "EMEVD bank 2007") } {
        Ok(()) => {
            crate::erdialog::install(); // (ER-POPUPS) the box open request and the question waiter
            erlog!("dragonmsg: (ER-TWEAKS) the event messages' dispatcher (EMEVD bank 2007) is detoured: \"A new draconic power is available\" (EventTextForMap {}..{}) is not shown (er.dragon_popup = 1: shown)", DRACONIC_TEXTS.start(), DRACONIC_TEXTS.end());
        }
        Err(e) => {
            ORIGINAL.store(0, Ordering::SeqCst);
            erlog!("dragonmsg: (ER-TWEAKS) the event messages' dispatcher was NOT detoured ({e}): Elden Ring's draconic-power popup still shows");
        }
    }
}

pub fn uninstalled() {
    ORIGINAL.store(0, Ordering::SeqCst);
    crate::erdialog::uninstalled(); // (ER-POPUPS)
}

/// The payload's shutdown: the game's messages all pass again.
pub fn release() {
    ON.store(false, Ordering::Relaxed);
    crate::erdialog::release(); // (ER-POPUPS)
}

/// (frame.rs, every frame) The key, the log.
pub fn frame(control: &Control) {
    crate::erdialog::frame(control); // (ER-POPUPS) Elden Ring's event-script boxes as KH3's window (`er.dialog_kh = 0`: the game's)
    let on = control.flag("er.dragon_popup") != Some(true);
    ON.store(on, Ordering::Relaxed);
    if TOLD_ON.swap(u32::from(on), Ordering::Relaxed) != u32::from(on) {
        erlog!("dragonmsg: (ER-TWEAKS) Elden Ring's \"A new draconic power is available\" popup: {}", if on { "HIDDEN" } else { "shown (er.dragon_popup = 1)" });
    }
    let skipped = SKIPPED.load(Ordering::Relaxed);
    if skipped != TOLD.swap(skipped, Ordering::Relaxed) {
        erlog!("dragonmsg: (ER-TWEAKS) EMEVD 2007[4] message {} (\"A new draconic power is available at the Cathedral of Dragon Communion\") NOT shown ({skipped} so far); the dragon's flags and heart are the event's own, as always", LAST.load(Ordering::Relaxed));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (ER-TWEAKS) Every dragon's text is suppressed; its neighbours in EventTextForMap and the other 2007[4] messages are not.
    #[test]
    fn only_the_draconic_power_texts_are_suppressed() {
        for id in 30060..=30066 {
            assert!(suppress(true, MESSAGE_INSTR, id), "{id}");
        }
        // EventTextForMap neighbours: 30050 / 30051 "Blocked shut for now", 30080 "Seal can only be broken by a sealbreaker",
        // 30110 the Knight of the Great Jar; the other 2007[4] users of common_func (20600, 4200)
        for id in [30059, 30067, 30050, 30051, 30080, 30110, 20600, 4200, 0, -1] {
            assert!(!suppress(true, MESSAGE_INSTR, id), "{id}");
        }
    }

    /// Only instruction 4: the bank's other instructions (dialogs, banners ...) pass even with a draconic id as argument.
    #[test]
    fn other_instructions_of_the_bank_pass() {
        for instr in [1, 2, 3, 5, 9, 10, 12, 13, 14, 15, 16] {
            assert!(!suppress(true, instr, 30060), "2007[{instr}]");
        }
    }

    /// The key gives Elden Ring's popup back.
    #[test]
    fn the_key_gives_the_popup_back() {
        assert!(!suppress(false, MESSAGE_INSTR, 30064));
    }
}
