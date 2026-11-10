//! (NO-ER-PAUSE, THE USER 2026-11-09: "remove all access to the elden ring pause menu, esc should make KH menu appear on
//! keyboard. Elden Ring pause menu should be impossible to ever show up") ELDEN RING'S IN-GAME TOP MENU (Equipment, Inventory,
//! Status, Messages, Multiplayer, Item Crafting, System) IS REFUSED WHERE THE GAME OPENS IT.
//!
//! HOW THE GAME OPENS IT (READ in eldenring.exe 2.7.2.0, the copy build/sora_tmp/quit_crash/eldenring_272.exe, with Ghidra and
//! the byte tools of CAMP-OPTIONS re-c; the same function in 2.7.1.0 at 0x7ef6d0, the same bytes and the same branch):
//! - the field input handler (0x140b037b0) asks the Start key's gate (0x140e2d120: key 0x34 / 0x35, no menu of its own busy,
//!   ...) and, when it passes, calls 0x1407ef270, which writes `CSPopupMenu + 0x121 = 1` (THE MENU REQUEST, only while
//!   interface element 0x1c is not created). Its View gate (0x140e2ce90) calls 0x1407ef2c0: `CSPopupMenu + 0x122 = 1` (the map).
//! - THE POPUP MENU'S UPDATE (0x1407efb80, every frame) is the one reader that opens anything for those bytes: `+0x121` set and
//!   element 0x1c not up -> 0x1407f0d20 -> 0x140803840 -> 0x140910020, the builder of the scene `02_000_IngameTop` (its window
//!   factory 0x1409131c0 constructs `CS::MainTopDialog`, vtable 0x142b02710), then `+0x121 = 0`; else `+0x122` set -> 0x1407fc610,
//!   the job of the scene `02_120_WorldMap` (factory 0x1407fe850 -> 0x1409d1040 -> `CS::WorldMapDialog`), then `+0x122 = 0`.
//! - Every caller of that chain, checked by calls, RIP-relative references and data pointers: 0x140910020 has one caller
//!   (0x140803840), it one (0x1407f0d20), it one (the update at 0x1407efecb). NO OTHER CODE BUILDS THE TOP MENU.
//!
//! WHAT IS DONE: the update is detoured; before the game's own update runs, a set `+0x121` is put back at 0, so the update takes
//! its own "nothing asked" branch. Whoever wrote the byte (the Start / Esc handler, a press of ours for the game, the game itself,
//! any other code), the top menu is never built. The map's byte (+0x122) and everything else of the update are untouched.
//! Always, while the payload runs (no switch: the user asked for it never to show up).

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use crate::erlog;
use crate::havok::Located;

/// `CSPopupMenu` + this: the in-game top menu's request byte (0x1407ef270 writes 1; the update opens `02_000_IngameTop`).
pub const MENU_REQUEST: usize = 0x121;
/// `CSPopupMenu` + this: the map's request byte (0x1407ef2c0 writes 1; the update opens `02_120_WorldMap`).
pub const MAP_REQUEST: usize = 0x122;

pub(crate) const LOCATED: [Located; 1] = [Located {
    name: "(NO-ER-PAUSE) the popup menu's update (opens 02_000_IngameTop on +0x121, the map on +0x122)",
    pattern: "48 8b c4 55 56 57 41 54 41 55 41 56 41 57 48 8d a8 68 fe ff ff 48 81 ec 60 02 00 00 48 c7 45 68 fe ff ff ff 48 89 58 20 0f 29 70 b8 48 8b 05 ? ? ? ? 48 33 c4 48 89 85 40 01 00 00 49 8b f0 0f 28 f1 48 8b f9 48 8b 89 48 02 00 00 48 85 c9 74 05 e8",
    capture: 0,
    rva_2_7_1_0: 0x007e_f6d0,
    rva_2_7_2_0: 0x007e_fb80,
}];

static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
/// Requests refused so far / already told in the log.
static REFUSED: AtomicU32 = AtomicU32::new(0);
static TOLD: AtomicU32 = AtomicU32::new(0);

/// THE RULE (pure): the request byte as the game's update is to see it (always 0: the top menu is never asked for), and
/// whether a request was refused.
pub fn refuse(request: u8) -> (u8, bool) {
    (0, request != 0)
}

type UpdateFn = unsafe extern "system" fn(usize, f32, usize) -> usize;

/// The popup menu's update: (CSPopupMenu*, the frame's delta in xmm1, a third argument in r8); rax passed on as the game's
/// own update left it.
unsafe extern "system" fn update_hook(popup: usize, dt: f32, arg: usize) -> usize {
    if popup != 0 {
        let byte = (popup + MENU_REQUEST) as *mut u8;
        let (to, refused) = refuse(byte.read_volatile());
        if refused {
            byte.write_volatile(to);
            REFUSED.fetch_add(1, Ordering::Relaxed);
        }
    }
    // (MAP-NOW) the map request a press of View / G earned, written right before the game's own update (mapgate.rs)
    let map_before = if popup != 0 { crate::mapgate::before_update(popup) } else { 0 };
    let original = ORIGINAL.load(Ordering::Relaxed);
    // SAFETY: the game's own update (its trampoline) with its own three arguments.
    let result = std::mem::transmute::<usize, UpdateFn>(original)(popup, dt, arg);
    if popup != 0 {
        crate::mapgate::after_update(popup, map_before);
    }
    result
}

/// (MAP-NOW) The popup update is detoured (mapgate.rs makes its map request there).
pub fn detoured() -> bool {
    ORIGINAL.load(Ordering::Relaxed) != 0
}

/// Installs the detour (entry.rs, with the others; undone with the renderer's hooks).
pub fn install() {
    let Some(f) = crate::havok::locate(&LOCATED[0]) else {
        erlog!("nopause: {} NOT FOUND: Elden Ring's top menu is kept away by the masks alone (Start and Esc hidden from it)", LOCATED[0].name);
        return;
    };
    // SAFETY: the function's start found by pattern at the recorded address (5 whole instructions there: mov rax, rsp; push
    // rbp; push rsi); the hook takes its three register arguments and passes them on unchanged.
    match unsafe { kh_er_render::inline_hook(f, update_hook as *const () as usize, &ORIGINAL, "popup menu update") } {
        Ok(()) => erlog!("nopause: (NO-ER-PAUSE) the popup menu's update detoured: ELDEN RING'S TOP MENU CAN NEVER OPEN (its request byte +0x121 is refused before every update; the map's +0x122 is the game's)"),
        Err(e) => {
            ORIGINAL.store(0, Ordering::SeqCst);
            erlog!("nopause: the popup menu's update NOT detoured ({e}): Elden Ring's top menu is kept away by the masks alone");
        }
    }
}

pub fn uninstalled() {
    ORIGINAL.store(0, Ordering::SeqCst);
}

/// Once a frame (FRAME_BEGIN): the refusals since the last frame, in the log (the first 20, then every 50th).
pub fn frame() {
    let n = REFUSED.load(Ordering::Relaxed);
    let told = TOLD.swap(n, Ordering::Relaxed);
    if n > told && (n <= 20 || n / 50 != told / 50) {
        erlog!("nopause: Elden Ring asked for its top menu (Start / Esc or a request of its own): REFUSED ({n} so far); the Kingdom Hearts menu is the menu");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (NO-ER-PAUSE) every request is refused, a byte that is not set stays 0 and counts nothing
    #[test]
    fn np_the_top_menu_request_is_always_refused() {
        assert_eq!(refuse(0), (0, false));
        for v in [1u8, 2, 0x80, 0xFF] {
            assert_eq!(refuse(v), (0, true), "{v}");
        }
    }

    #[test]
    fn np_the_pattern_is_recorded_for_both_versions_and_is_not_the_map_byte() {
        assert!(LOCATED[0].name.starts_with("(NO-ER-PAUSE)") && LOCATED[0].rva_2_7_1_0 != 0 && LOCATED[0].rva_2_7_2_0 != 0);
        assert_ne!(MENU_REQUEST, MAP_REQUEST);
        // the delta of that region of the executable (CAMP-OPTIONS re-c: +0x4b0)
        assert_eq!(LOCATED[0].rva_2_7_2_0 - LOCATED[0].rva_2_7_1_0, 0x4b0);
    }
}
