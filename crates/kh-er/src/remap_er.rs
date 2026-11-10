//! (CAMP-OPTIONS, THE USER 2026-11-05: "make button remapping possible on both controller and keyboard, using the settings
//! menu"; "each button can be remapped, not each action"; "keyboard and mouse defaults should be KH3's keyboard and mouse
//! defaults") THE BUTTON TABLES AT THE INPUT LAYER: kh-host remap.rs's two tables applied where the pad and the keyboard are
//! read (pad.rs `Reader::read`, inputs.rs `keyboard`), before any KH logic: gameplay and menus alike see the remapped
//! buttons.
//!
//! LOCK-OUT PROTECTION, ON EVERY KH SCREEN (load, save, new game, camp, settings, shops, Workshop, dialogs, the game over;
//! `set_kh_screen`, `set_options_up`): the keyboard table plus KH3's default key of every role whose default key the table
//! gives to no role (kh-host remap.rs `apply_kbm_protected`); the controller's table is a permutation, so every role always
//! has a button and nothing is added (no swap presses two roles at once). Whatever a table says, the menus can be navigated.
//!
//! THE MOUSE: its buttons through GetAsyncKeyState (VK_LBUTTON .. VK_XBUTTON2); the wheel through the game window's own
//! messages (WM_MOUSEWHEEL, the window procedure subclassed once, put back at the payload's stop): a notch is a press of
//! `WHEEL_MS`. The mouse's movement is KH3 PC's second right stick (dinput_mask.rs, eroptions.rs `mouse_stick`).
//!
//! THE CAPTURE (a row of the settings screen waits for the new button): the inputs held when it begins do not count; the
//! first input pressed after is the answer; Esc or `CAPTURE_MS` gives it up.

use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::sync::Mutex;

use glam::Vec2;
use kh_core::{PadButton, PadButtons};
use kh_host::remap::{Device, Table, WHEEL_DOWN, WHEEL_UP};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::SystemInformation::GetTickCount64;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;

use crate::erlog;

struct Tables {
    pad: Table,
    kbm: Table,
    /// The settings file's text they were made from (re-parsed only when it changes).
    text: (String, String),
}

static TABLES: Mutex<Option<Tables>> = Mutex::new(None);
/// Our settings screen is up: the unmapped inputs act too.
static OPTIONS_UP: AtomicBool = AtomicBool::new(false);
/// `in.keys_legacy = 1` (testing aid): the keys of the port's keyboard before KH3's defaults (J K L I U O P Y H T, the
/// arrows), for drivers written for them.
static LEGACY: AtomicBool = AtomicBool::new(false);

/// A wheel notch counts as a press this long.
pub const WHEEL_MS: u64 = 90;
/// A capture is given up after this long.
pub const CAPTURE_MS: u64 = 6000;

fn now() -> u64 {
    // SAFETY: plain getter.
    unsafe { GetTickCount64() }
}

/// Our settings screen is up / down (eroptions.rs, the HUD's frames on the title and in game).
pub fn set_options_up(on: bool) {
    OPTIONS_UP.store(on, Ordering::Relaxed);
}

/// (CAMP-OPTIONS) A KH screen has the input (the title's load / new-game / settings screens, in game a menu of ours: camp,
/// save point, shops, Workshop, dialogs, the game over): KH3's default keys the table left free act too.
static KH_SCREEN: AtomicBool = AtomicBool::new(false);

pub fn set_kh_screen(on: bool) {
    KH_SCREEN.store(on, Ordering::Relaxed);
}

/// Once a frame (eroptions.rs): the tables from the settings file's keys, the switches.
pub fn set(pad_text: &str, kbm_text: &str, legacy: bool) {
    LEGACY.store(legacy, Ordering::Relaxed);
    let Ok(mut t) = TABLES.lock() else { return };
    if t.as_ref().is_some_and(|t| t.text.0 == pad_text && t.text.1 == kbm_text) {
        return;
    }
    let tables = Tables { pad: Table::parse(Device::Pad, pad_text), kbm: Table::parse(Device::Kbm, kbm_text), text: (pad_text.to_string(), kbm_text.to_string()) };
    erlog!("remap: the button tables in force: controller [{}], keyboard and mouse [{}] (empty = KH3's defaults)", tables.pad.format(), tables.kbm.format());
    crate::dinput_mask::set_bound(&tables.kbm.assign);
    *t = Some(tables);
}

/// THE CONTROLLER'S BUTTONS AS KH SEES THEM (pad.rs): the table applied, on every screen. The table is a permutation (every
/// role has a button, every button one role), so no screen can be locked out; the buttons themselves are NOT added on top
/// (with a swap that pressed two roles at once: A = Confirm and Cancel).
pub fn pad(raw: PadButtons) -> PadButtons {
    TABLES.lock().ok().and_then(|t| t.as_ref().map(|t| t.pad.apply_pad(raw))).unwrap_or(raw)
}

/// (CAMP-OPTIONS) The controller's table gives the physical `button` another role than its own (with the default table:
/// false). sora.rs: the game's own prompt then still gets its own physical Y (its action button), whatever KH made of it.
pub fn pad_moves(button: PadButton) -> bool {
    let mut one = PadButtons::NONE;
    one.set(button, true);
    TABLES.lock().ok().and_then(|t| t.as_ref().map(|t| t.pad.apply_pad(one) != one)).unwrap_or(false)
}

/// An input's state: keys and mouse buttons by GetAsyncKeyState, the wheel by its last notch.
fn down(code: u16) -> bool {
    match code {
        WHEEL_UP => now().saturating_sub(WHEEL_UP_AT.load(Ordering::Relaxed)) < WHEEL_MS,
        WHEEL_DOWN => now().saturating_sub(WHEEL_DOWN_AT.load(Ordering::Relaxed)) < WHEEL_MS,
        0x01..=0xFE => {
            // SAFETY: plain getter.
            (unsafe { GetAsyncKeyState(i32::from(code)) } as u16) & 0x8000 != 0
        }
        _ => false,
    }
}

/// The port's keys before KH3's defaults (`in.keys_legacy = 1` only).
const LEGACY_KEYS: [(u16, PadButton); 14] = [
    (0x4A, PadButton::FaceBottom),
    (0x4B, PadButton::FaceRight),
    (0x4C, PadButton::FaceLeft),
    (0x49, PadButton::FaceTop),
    (0x55, PadButton::LeftShoulder),
    (0x4F, PadButton::RightShoulder),
    (0x50, PadButton::RightTrigger),
    (0x59, PadButton::LeftTrigger),
    (0x48, PadButton::RightThumb),
    (0x54, PadButton::LeftThumb),
    (0x26, PadButton::DPadUp),
    (0x28, PadButton::DPadDown),
    (0x25, PadButton::DPadLeft),
    (0x27, PadButton::DPadRight),
];

/// THE KEYBOARD AND MOUSE AS KH'S PAD (inputs.rs `keyboard`, the game's window in front): the table applied; with our settings
/// screen up, KH3's default keys too.
pub fn keyboard() -> (Vec2, PadButtons) {
    ensure_wheel();
    // (on a KH screen, our settings screen among them: the table plus KH3's default keys it left free, kh-host remap.rs
    // `apply_kbm_protected`; in play the table alone)
    let protected = OPTIONS_UP.load(Ordering::Relaxed) || KH_SCREEN.load(Ordering::Relaxed) || crate::hud::menu_open() || crate::hud::gameover_holds();
    let table = TABLES.lock().ok().and_then(|t| t.as_ref().map(|t| t.kbm.clone())).unwrap_or_else(|| Table::defaults(Device::Kbm));
    let (stick, mut buttons) = if protected { table.apply_kbm_protected(&down) } else { table.apply_kbm(&down) };
    if stick != Vec2::ZERO || buttons != PadButtons::NONE {
        crate::button_icons_er::kbm_used(); // (BUTTON-ICONS) the device used last
    }
    if LEGACY.load(Ordering::Relaxed) {
        for (vk, b) in LEGACY_KEYS {
            if down(vk) {
                buttons.set(b, true);
            }
        }
    }
    (stick, buttons)
}

// ------------------------------------------------------------------------------------------------ the wheel

static WHEEL_UP_AT: AtomicU64 = AtomicU64::new(0);
static WHEEL_DOWN_AT: AtomicU64 = AtomicU64::new(0);
static WINDOW: AtomicIsize = AtomicIsize::new(0);
static ORIGINAL_PROC: AtomicIsize = AtomicIsize::new(0);
static WHEEL_TRIED: AtomicBool = AtomicBool::new(false);

/// (CAMP-OPTIONS) What the game's window procedure is not shown: 0 nothing; 1 the keys / mouse buttons KH's table binds (Sora
/// plays: the game must not also act on them, as its pad is masked); 2 every key and mouse button (our settings screen, on the
/// title or in game: the game's own list under it must not move); 3 (CC-MUTE, ngquiet.rs: Elden Ring's creation pressed for the
/// player) level 2 plus the mouse's movement (WM_MOUSEMOVE, and its DirectInput counts). The game's menus (ui.menu) get everything.
static SWALLOW: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub fn set_swallow(level: u8) {
    SWALLOW.store(level.min(3), Ordering::Relaxed);
    crate::dinput_mask::set_level(level); // (the game reads its keyboard and mouse through DirectInput: dinput_mask.rs)
}

/// The key / mouse code a window message is about (None: not a key or button message).
fn message_code(message: u32, w: WPARAM) -> Option<u16> {
    match message {
        // WM_KEYDOWN / UP, WM_SYSKEYDOWN / UP, WM_CHAR, WM_SYSCHAR
        0x0100 | 0x0101 | 0x0104 | 0x0105 => Some((w & 0xFF) as u16),
        0x0102 | 0x0106 => Some(0),
        0x0201..=0x0203 => Some(kh_host::remap::MOUSE_LEFT),
        0x0204..=0x0206 => Some(kh_host::remap::MOUSE_RIGHT),
        0x0207..=0x0209 => Some(kh_host::remap::MOUSE_MIDDLE),
        0x020B..=0x020D => Some(if (w >> 16) & 0xFFFF == 1 { kh_host::remap::MOUSE_X1 } else { kh_host::remap::MOUSE_X2 }),
        0x020A => Some(if ((w >> 16) & 0xFFFF) as u16 as i16 > 0 { WHEEL_UP } else { WHEEL_DOWN }),
        _ => None,
    }
}

/// (CC-MUTE) A pointer message (pure).
pub fn pointer_message(message: u32) -> bool {
    matches!(message, 0x0200 | 0x00A0 | 0x02A1 | 0x02A3)
}

/// (CC-MUTE) A key message: WM_KEYDOWN / UP, WM_CHAR, WM_SYSKEYDOWN / UP, WM_SYSCHAR (pure).
pub fn key_message(message: u32) -> bool {
    matches!(message, 0x0100..=0x0102 | 0x0104..=0x0106)
}

/// (CC-MUTE) A key message's scan code from its lParam (bits 16..23, bit 24 extended -> 0x100), as SendInput's callers write it (pure).
pub fn message_scan(l: LPARAM) -> u16 {
    (((l >> 16) & 0xFF) as u16) | if (l >> 24) & 1 != 0 { 0x100 } else { 0 }
}

/// The keyboard / mouse table binds the code (the generic Shift / Ctrl / Alt by either side).
fn bound(code: u16) -> bool {
    let Ok(t) = TABLES.lock() else { return false };
    let table = t.as_ref().map_or_else(|| Table::defaults(Device::Kbm), |t| t.kbm.clone());
    let sides: &[u16] = match code {
        0x10 => &[0xA0, 0xA1],
        0x11 => &[0xA2, 0xA3],
        0x12 => &[0xA4, 0xA5],
        _ => &[],
    };
    table.holder(code).is_some() || sides.iter().any(|c| table.holder(*c).is_some())
}

unsafe extern "system" fn wheel_proc(window: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    use windows_sys::Win32::UI::WindowsAndMessaging::{CallWindowProcW, DefWindowProcW, WM_MOUSEWHEEL};
    // (CAMP-OPTIONS) the mouse's movement for the KH camera's mouse look: the raw input the window is sent (read beside the
    // game: GetRawInputData leaves the message to it), and which messages carry it (logged once each)
    if message == 0x00FF {
        raw_motion(l);
    } else if message == 0x0200 && !TOLD_MOVE.swap(true, Ordering::Relaxed) {
        erlog!("remap: the game window gets WM_MOUSEMOVE (raw input seen so far: {})", TOLD_RAW.load(Ordering::Relaxed));
    }
    let level = SWALLOW.load(Ordering::Relaxed);
    if level > 0 {
        // WM_INPUT (raw input): only with everything swallowed; its cleanup is the default procedure's
        if message == 0x00FF && level >= 2 {
            return DefWindowProcW(window, message, w, l);
        }
        // (CC-MUTE) level 3: the pointer is not the game's either (WM_MOUSEMOVE, WM_NCMOUSEMOVE, WM_MOUSEHOVER, WM_MOUSELEAVE)
        if pointer_message(message) && level >= 3 {
            return 0;
        }
        // (CC-MUTE) our own key presses (newgame.rs types the name "Sora": its WM_KEYDOWN / WM_CHAR) pass every level
        if key_message(message) && crate::dinput_mask::scan_allowed(message_scan(l)) {
            let original = ORIGINAL_PROC.load(Ordering::Relaxed);
            // SAFETY: the window's own procedure, as it was before us, with the message as it came.
            return CallWindowProcW(std::mem::transmute::<isize, windows_sys::Win32::UI::WindowsAndMessaging::WNDPROC>(original), window, message, w, l);
        }
        if let Some(code) = message_code(message, w) {
            if message == WM_MOUSEWHEEL {
                if ((w >> 16) & 0xFFFF) as u16 as i16 > 0 {
                    WHEEL_UP_AT.store(now(), Ordering::Relaxed);
                } else {
                    WHEEL_DOWN_AT.store(now(), Ordering::Relaxed);
                }
            }
            if level >= 2 || (code != 0 && bound(code)) {
                return 0;
            }
        }
    }
    if message == WM_MOUSEWHEEL {
        let delta = ((w >> 16) & 0xFFFF) as u16 as i16;
        if delta > 0 {
            WHEEL_UP_AT.store(now(), Ordering::Relaxed);
        } else if delta < 0 {
            WHEEL_DOWN_AT.store(now(), Ordering::Relaxed);
        }
    }
    let original = ORIGINAL_PROC.load(Ordering::Relaxed);
    // SAFETY: the window's own procedure, as it was before us, with the message as it came.
    CallWindowProcW(std::mem::transmute::<isize, windows_sys::Win32::UI::WindowsAndMessaging::WNDPROC>(original), window, message, w, l)
}

static TOLD_RAW: AtomicBool = AtomicBool::new(false);
static TOLD_MOVE: AtomicBool = AtomicBool::new(false);

/// A WM_INPUT's mouse movement (relative only) into the mouse look's sum (dinput_mask.rs `take_mouse_motion`).
fn raw_motion(l: LPARAM) {
    use windows_sys::Win32::UI::Input::{GetRawInputData, RAWINPUT, RAWINPUTHEADER, RID_INPUT, RIM_TYPEMOUSE};
    let mut data = std::mem::MaybeUninit::<RAWINPUT>::zeroed();
    let mut size = std::mem::size_of::<RAWINPUT>() as u32;
    // SAFETY: the message's own raw-input handle; the buffer is a RAWINPUT of the size given (the mouse's fits).
    let got = unsafe { GetRawInputData(l as _, RID_INPUT, data.as_mut_ptr().cast(), &mut size, std::mem::size_of::<RAWINPUTHEADER>() as u32) };
    if got == u32::MAX || got == 0 {
        return;
    }
    // SAFETY: GetRawInputData filled `got` bytes (at least the header) of the RAWINPUT.
    let raw = unsafe { data.assume_init() };
    if raw.header.dwType != RIM_TYPEMOUSE {
        return;
    }
    // SAFETY: a mouse's raw input: the `mouse` member is the one written.
    let mouse = unsafe { raw.data.mouse };
    // MOUSE_MOVE_ABSOLUTE (1): a tablet / remote desktop's position, not a movement
    if mouse.usFlags & 1 != 0 {
        return;
    }
    if !TOLD_RAW.swap(true, Ordering::Relaxed) {
        erlog!("remap: the game window gets the mouse's raw input (WM_INPUT): the KH camera's mouse look reads it");
    }
    crate::dinput_mask::add_raw_motion(mouse.lLastX, mouse.lLastY);
}

/// (CC-MUTE, ngquiet.rs) The game's DirectInput reads and window procedure are taken over now if they were not yet (a New Game
/// chosen before any screen of ours asked for the keyboard).
pub fn ensure_installed() {
    ensure_wheel();
}

/// The game's window procedure is subclassed once (the first frame the game's window is in front).
fn ensure_wheel() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId, SetWindowLongPtrW, GWLP_WNDPROC};
    if WHEEL_TRIED.load(Ordering::Relaxed) {
        return;
    }
    crate::dinput_mask::install();
    // SAFETY: plain getters; the window is the game's own (its process is ours), its procedure replaced by one that passes
    // every message on to it.
    unsafe {
        let window = GetForegroundWindow();
        if window.is_null() {
            return;
        }
        let mut process = 0u32;
        GetWindowThreadProcessId(window, &mut process);
        if process != windows_sys::Win32::System::Threading::GetCurrentProcessId() {
            return;
        }
        WHEEL_TRIED.store(true, Ordering::Relaxed);
        let old = SetWindowLongPtrW(window, GWLP_WNDPROC, wheel_proc as *const () as isize);
        if old == 0 {
            erlog!("remap: the game window's messages could not be read: the mouse wheel binds nothing");
            return;
        }
        ORIGINAL_PROC.store(old, Ordering::Relaxed);
        WINDOW.store(window as isize, Ordering::Relaxed);
        erlog!("remap: the mouse wheel is read from the game window's own messages");
    }
}

/// The payload stops: the window's own procedure back (when ours is still the one in place).
pub fn uninstalled() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, SetWindowLongPtrW, GWLP_WNDPROC};
    let (window, original) = (WINDOW.swap(0, Ordering::SeqCst), ORIGINAL_PROC.load(Ordering::SeqCst));
    if window == 0 || original == 0 {
        return;
    }
    // SAFETY: the window we subclassed; its procedure is put back only while ours is in place.
    unsafe {
        let hwnd = window as HWND;
        if GetWindowLongPtrW(hwnd, GWLP_WNDPROC) == wheel_proc as *const () as isize {
            SetWindowLongPtrW(hwnd, GWLP_WNDPROC, original);
        }
    }
}


// ------------------------------------------------------------------------------------------------ (NO-ER-PAUSE) Esc = the KH menu

/// An Esc that a button capture saw (it gives the capture up) is not the menu key until it has been let go.
static ESC_LATCH: AtomicBool = AtomicBool::new(false);

/// (NO-ER-PAUSE, eroptions.rs Return to Desktop) The game's window is asked to close the way its own close button asks it: a
/// WM_CLOSE posted to it (its window procedure handles the message itself: gosound.rs reads its close mute there). False: the
/// window is not known (never subclassed) or the post failed.
pub fn close_game_window() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};
    let window = WINDOW.load(Ordering::SeqCst);
    if window == 0 {
        return false;
    }
    // SAFETY: a plain post to the game's own top-level window (the handle the subclass was made on); nothing is read back.
    unsafe { PostMessageW(window as HWND, WM_CLOSE, 0, 0) != 0 }
}

/// A button capture of the settings screen is waiting.
pub fn capturing() -> bool {
    CAPTURE.lock().is_ok_and(|c| c.is_some())
}

/// THE RULE (pure): Esc `down` now, a capture waiting, the latch -> (Esc is the menu key this frame, the latch after). Esc
/// held while a capture waits latches; the latch lets go only with the key.
pub fn esc_as_menu(down: bool, capturing: bool, latched: bool) -> (bool, bool) {
    let latched = down && (latched || capturing);
    (down && !latched, latched)
}

/// (inputs.rs `esc_menu`) Esc counts as the KH menu key now.
pub fn esc_counts(down: bool) -> bool {
    let (menu, latched) = esc_as_menu(down, capturing(), ESC_LATCH.load(Ordering::Relaxed));
    ESC_LATCH.store(latched, Ordering::Relaxed);
    menu
}
// ------------------------------------------------------------------------------------------------ the capture

struct Capture {
    device: Device,
    at: u64,
    /// The inputs held when it began (pad: its buttons; keyboard / mouse: the codes).
    held_pad: PadButtons,
    held_keys: Vec<u16>,
}
static CAPTURE: Mutex<Option<Capture>> = Mutex::new(None);

/// The controller's buttons as they are (no table): the first connected pad.
fn raw_pad() -> PadButtons {
    let mut b = PadButtons::NONE;
    for index in 0..4 {
        if let Some(s) = crate::inputs::xinput_state(index) {
            b = crate::pad::buttons_of(&s.Gamepad);
            break;
        }
    }
    b
}

fn keys_down() -> Vec<u16> {
    let mut out: Vec<u16> = (0x01u16..=0xFE).filter(|c| !(0x10..=0x12).contains(c) && down(*c)).collect();
    for w in [WHEEL_UP, WHEEL_DOWN] {
        if down(w) {
            out.push(w);
        }
    }
    out
}

/// A row's capture begins.
pub fn capture_begin(device: Device) {
    if let Ok(mut c) = CAPTURE.lock() {
        *c = Some(Capture { device, at: now(), held_pad: raw_pad(), held_keys: keys_down() });
    }
    erlog!("remap: waiting for the new {} (Esc or {} s gives up)", if device == Device::Pad { "controller button" } else { "key or mouse button" }, CAPTURE_MS / 1000);
}

/// The answer of a capture (see kh-host er_options.rs `ErGame::capture_poll`).
pub fn capture_poll(device: Device) -> Option<Option<u16>> {
    let Ok(mut guard) = CAPTURE.lock() else { return Some(None) };
    let Some(c) = guard.as_mut().filter(|c| c.device == device) else { return Some(None) };
    if now().saturating_sub(c.at) > CAPTURE_MS || down(0x1B) {
        if down(0x1B) {
            ESC_LATCH.store(true, Ordering::Relaxed); // (NO-ER-PAUSE) the Esc that gave the capture up does not also close the menu
        }
        *guard = None;
        return Some(None);
    }
    let found = match device {
        Device::Pad => {
            let now_held = raw_pad();
            // a button let go since the start may be pressed again
            c.held_pad = PadButtons(c.held_pad.0 & now_held.0);
            PadButton::ALL.iter().position(|b| now_held.has(*b) && !c.held_pad.has(*b)).map(|k| k as u16)
        }
        Device::Kbm => {
            let now_held = keys_down();
            c.held_keys.retain(|k| now_held.contains(k));
            now_held.iter().copied().find(|k| !c.held_keys.contains(k))
        }
    };
    let code = found?;
    *guard = None;
    erlog!("remap: captured {}", kh_host::remap::input_name(device, code));
    Some(Some(code))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (the tables are one global: the tests that set them run one at a time)
    static TABLES_LOCK: Mutex<()> = Mutex::new(());

    /// (CC-MUTE) The window procedure at level 3: the pointer's messages are swallowed; a key message's scan code is read from
    /// its lParam as SendInput's callers write it (our own typed name passes by it).
    #[test]
    fn cc_pointer_messages_and_key_scan_codes() {
        for m in [0x0200, 0x00A0, 0x02A1, 0x02A3] {
            assert!(pointer_message(m), "{m:#x}");
        }
        assert!(!pointer_message(0x0201), "a button is a key/button message, handled by the level rule");
        for m in [0x0100, 0x0101, 0x0102, 0x0104, 0x0105, 0x0106] {
            assert!(key_message(m), "{m:#x}");
        }
        assert!(!key_message(0x0103) && !key_message(0x0200));
        // 'S' down: repeat 1, scan 0x1F; Up (extended): scan 0x48 + bit 24
        assert_eq!(message_scan(0x001F_0001), 0x1F);
        assert_eq!(message_scan(0x0148_0001), 0x148);
        assert_eq!(crate::dinput_mask::dik_of_scan(message_scan(0x0148_0001)), 0xC8);
        assert_eq!(message_scan(0xC01F_0001u32 as i32 as LPARAM), 0x1F, "a key-up's transition bits are not the scan code");
    }

    /// (NO-ER-PAUSE) Esc is the KH menu key; the Esc that gives a button capture up is not, until it is let go
    #[test]
    fn np_esc_is_the_menu_key_but_not_the_capture_give_up() {
        assert_eq!(esc_as_menu(true, false, false), (true, false), "Esc in play / in the menu: Start");
        assert_eq!(esc_as_menu(false, false, false), (false, false));
        // a capture waits: Esc gives it up (remap_er's poll), the same press must not close the menu
        let (menu, latched) = esc_as_menu(true, true, false);
        assert!(!menu && latched);
        // the capture is over, the key still held: still not the menu key
        let (menu, latched) = esc_as_menu(true, false, latched);
        assert!(!menu && latched);
        // let go: the latch goes; the next press is Start again
        let (menu, latched) = esc_as_menu(false, false, latched);
        assert!(!menu && !latched);
        assert_eq!(esc_as_menu(true, false, latched), (true, false));
    }

    #[test]
    fn co_the_tables_apply_on_every_screen_without_double_presses() {
        let _one = TABLES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // the table swaps A and B: A plays B's role everywhere, never both at once (the pad table is a permutation: no
        // screen can be locked out)
        let a = {
            let mut b = PadButtons::NONE;
            b.set(PadButton::FaceBottom, true);
            b
        };
        set("A=0x1 B=0x0", "", false);
        for (options, kh) in [(false, false), (true, false), (false, true)] {
            set_options_up(options);
            set_kh_screen(kh);
            let mapped = pad(a);
            assert!(mapped.has(PadButton::FaceRight) && !mapped.has(PadButton::FaceBottom), "A plays B's role only ({options} {kh})");
        }
        set_options_up(false);
        set_kh_screen(false);
        set("", "", false);
        assert_eq!(pad(a), a, "the defaults: identity");
    }

    /// (CAMP-OPTIONS) the game's own action prompt (talk, open, pick up) on the pad: with the default table KH's Y route hands
    /// Y on (pad_moves false: nothing extra); with a table that moves the physical Y to another role, sora.rs gives the game its
    /// physical Y while its prompt is open (pad_moves true)
    #[test]
    fn co_the_games_action_prompt_keeps_its_own_pad_button() {
        let _one = TABLES_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set("", "", false);
        assert!(!pad_moves(PadButton::FaceTop), "the default table: Y is Y");
        // Confirm (A's role) moved to the physical Y, Y's role to the physical A (the swap the menu makes)
        set(&format!("A={:#x} Y={:#x}", PadButton::FaceTop as u16, PadButton::FaceBottom as u16), "", false);
        let moved = pad_moves(PadButton::FaceTop);
        set("", "", false);
        assert!(moved, "the physical Y plays Confirm now: the game's prompt gets the physical Y itself");
    }
}
