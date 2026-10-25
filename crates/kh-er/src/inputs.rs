//! INPUT OWNERSHIP (IMPL_ER4_STATE.md, item 4): who gets which pad button, the keyboard path, and
//! what Elden Ring's interface is doing (a menu open, the world paused).
//!
//! THE PAD AS ELDEN RING SEES IT. Elden Ring reads the pad through XInput like we do. An inline
//! hook of ours on `XInputGetState` (and the unnamed `XInputGetStateEx`, ordinal 100) of the XInput
//! library the process has loaded lets the game's own call through and then clears, in the state
//! it returns, what belongs to Sora: by default every button except Menu (Start) and View
//! (Back), both triggers and the left stick; the right stick (Elden Ring's camera) is left. Our
//! own read of the pad goes around the hook. The mask has to be renewed by the frame task every
//! frame (`set_mask`): 250 ms after the last renewal the hook passes everything through again,
//! so a loading screen, the title menu, a dead payload or a hot reload never leave Elden Ring
//! without its pad. The hook is undone at shutdown with the renderer's (`kh_er_render::shutdown`).
//!
//! THE KEYBOARD PATH (an extra, never the only one; also what unattended tests drive with
//! `SendInput`): while the game's window is in the foreground, keys that Elden Ring's default
//! layout leaves unused act as Sora's pad:
//! W A S D = left stick | J = A (attack) | K = B (jump) | L = X (guard / dodge) | I = Y
//! (situation command) | U = LB (shortcuts) | O = RB (lock-on / shotlock) | P = RT (next target) |
//! Y = LT | H = R3 | T = L3 | arrow keys = D-pad. `in.keys = 0` turns it off.

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

use glam::Vec2;
use kh_core::{PadButton, PadButtons};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows_sys::Win32::System::SystemInformation::GetTickCount64;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows_sys::Win32::UI::Input::XboxController::{XInputGetState, XINPUT_CAPABILITIES, XINPUT_STATE};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

use crate::erlog;

// ---------------------------------------------------------------- the XInput mask

pub const XI_DPAD: u16 = 0x000F;
pub const XI_START: u16 = 0x0010;
pub const XI_BACK: u16 = 0x0020;
pub const XI_L3: u16 = 0x0040;
pub const XI_R3: u16 = 0x0080;
pub const XI_Y: u16 = 0x8000;
/// Everything but Menu (Start) and View (Back).
pub const XI_GAMEPLAY: u16 = !(XI_START | XI_BACK);

/// What the hook clears: bits 0..15 = `wButtons` bits, bit 16 = the left stick, bit 17 = both triggers.
static MASK: AtomicU32 = AtomicU32::new(0);
/// `GetTickCount64` until which the mask holds.
static MASK_UNTIL: AtomicU64 = AtomicU64::new(0);
static ORIGINAL_GET_STATE: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_GET_STATE_EX: AtomicUsize = AtomicUsize::new(0);
/// Calls of the game that went through the hook (for the log).
pub static HOOK_CALLS: AtomicU32 = AtomicU32::new(0);

pub const MASK_LEFT_STICK: u32 = 1 << 16;
pub const MASK_TRIGGERS: u32 = 1 << 17;
/// The right stick (while KH3's camera has the view: Elden Ring's own camera gets no stick).
pub const MASK_RIGHT_STICK: u32 = 1 << 18;

/// Every call of the game's, connected or not (for the log).
pub static ALL_CALLS: AtomicU32 = AtomicU32::new(0);
/// THE SCRIPTED PAD as Elden Ring sees it (IMPL_ER8): bits 0..15 `wButtons`, 16..31 the left stick (two signed
/// bytes, x then y, -127..127), 32..47 the right stick, 48 / 49 the left / right trigger; alive until `VIRT_UNTIL`.
static VIRT: AtomicU64 = AtomicU64::new(0);
static VIRT_UNTIL: AtomicU64 = AtomicU64::new(0);
static VIRT_PACKET: AtomicU32 = AtomicU32::new(1);
/// A real pad answered our own read until this moment (no slot is invented then).
static REAL_PAD_UNTIL: AtomicU64 = AtomicU64::new(0);

/// in.vpad = 1 (testing aid): with no real pad connected, slot 0 is a connected, idle pad to Elden Ring (the input
/// script's buttons then reach it as a pad's); renewed by the frame task, lapses 1 s after its last renewal.
static VPAD_UNTIL: AtomicU64 = AtomicU64::new(0);
static ORIGINAL_GET_CAPS: AtomicUsize = AtomicUsize::new(0);

pub fn set_vpad(on: bool) {
    // SAFETY: plain getter.
    let now = unsafe { GetTickCount64() };
    VPAD_UNTIL.store(if on { now + 1000 } else { 0 }, Ordering::Relaxed);
}

/// The virtual pad can stand in right now (the hook is in, no real pad answers).
pub fn vpad_live() -> bool {
    // SAFETY: plain getter.
    let now = unsafe { GetTickCount64() };
    ORIGINAL_GET_STATE.load(Ordering::Relaxed) != 0 && now < VPAD_UNTIL.load(Ordering::Relaxed) && now >= REAL_PAD_UNTIL.load(Ordering::Relaxed)
}

unsafe extern "system" fn get_caps_hook(index: u32, flags: u32, caps: *mut XINPUT_CAPABILITIES) -> u32 {
    let original: unsafe extern "system" fn(u32, u32, *mut XINPUT_CAPABILITIES) -> u32 = std::mem::transmute(ORIGINAL_GET_CAPS.load(Ordering::Relaxed));
    let result = original(index, flags, caps);
    let now = GetTickCount64();
    if result != 0 && index == 0 && !caps.is_null() && now < VPAD_UNTIL.load(Ordering::Relaxed) && now >= REAL_PAD_UNTIL.load(Ordering::Relaxed) {
        // (an Xbox pad: type 1 = gamepad, subtype 1 = gamepad, every button and axis)
        let mut c = XINPUT_CAPABILITIES::default();
        c.Type = 1;
        c.SubType = 1;
        c.Gamepad.wButtons = 0xf3ff;
        c.Gamepad.bLeftTrigger = 255;
        c.Gamepad.bRightTrigger = 255;
        c.Gamepad.sThumbLX = -64;
        c.Gamepad.sThumbLY = -64;
        c.Gamepad.sThumbRX = -64;
        c.Gamepad.sThumbRY = -64;
        c.Vibration.wLeftMotorSpeed = 255;
        c.Vibration.wRightMotorSpeed = 255;
        *caps = c;
        return 0;
    }
    result
}

/// (PORT30) The input script's held pad buttons (XInput `wButtons` bits) while it runs, else 0.
pub fn virtual_buttons() -> u16 {
    // SAFETY: plain getter.
    let now = unsafe { GetTickCount64() };
    if now < VIRT_UNTIL.load(Ordering::Relaxed) {
        (VIRT.load(Ordering::Relaxed) & 0xffff) as u16
    } else {
        0
    }
}

/// The frame task saw a real pad (renewed every frame it does).
pub fn real_pad_seen() {
    // SAFETY: plain getter.
    REAL_PAD_UNTIL.store(unsafe { GetTickCount64() } + 1000, Ordering::Relaxed);
}

/// The input script's sticks and buttons of this frame, as a held pad (lapses 100 ms after the last renewal).
pub fn set_virtual(left: Vec2, right: Vec2, buttons: PadButtons) {
    let mut v = 0u64;
    for (button, bits) in [
        (PadButton::DPadUp, 0x0001u64),
        (PadButton::DPadDown, 0x0002),
        (PadButton::DPadLeft, 0x0004),
        (PadButton::DPadRight, 0x0008),
        (PadButton::SpecialRight, 0x0010),
        (PadButton::SpecialLeft, 0x0020),
        (PadButton::LeftThumb, 0x0040),
        (PadButton::RightThumb, 0x0080),
        (PadButton::LeftShoulder, 0x0100),
        (PadButton::RightShoulder, 0x0200),
        (PadButton::FaceBottom, 0x1000),
        (PadButton::FaceRight, 0x2000),
        (PadButton::FaceLeft, 0x4000),
        (PadButton::FaceTop, 0x8000),
        (PadButton::LeftTrigger, 1 << 48),
        (PadButton::RightTrigger, 1 << 49),
    ] {
        if buttons.has(button) {
            v |= bits;
        }
    }
    let byte = |x: f32| -> u64 { u64::from((x.clamp(-1.0, 1.0) * 127.0).round() as i8 as u8) };
    v |= byte(left.x) << 16 | byte(left.y) << 24 | byte(right.x) << 32 | byte(right.y) << 40;
    VIRT.store(v, Ordering::Relaxed);
    // SAFETY: plain getter.
    VIRT_UNTIL.store(unsafe { GetTickCount64() } + 100, Ordering::Relaxed);
}

type GetStateFn = unsafe extern "system" fn(u32, *mut XINPUT_STATE) -> u32;

unsafe fn masked(original: usize, index: u32, state: *mut XINPUT_STATE) -> u32 {
    let original: GetStateFn = std::mem::transmute(original);
    let mut result = original(index, state);
    ALL_CALLS.fetch_add(1, Ordering::Relaxed);
    ER_CALL_AT.store(GetTickCount64(), Ordering::Relaxed);
    // (IMPL_ER8) THE SCRIPTED PAD (`in.script`, a testing aid) and our own presses (`open_er_system_menu`) as a pad
    // Elden Ring sees: with no pad connected, slot 0 answers "connected, nothing held" while either is running;
    // the script's sticks and buttons go in BEFORE the mask, exactly as a held pad's would.
    if !state.is_null() && index == 0 {
        let now = GetTickCount64();
        let virt = now < VIRT_UNTIL.load(Ordering::Relaxed);
        if result != 0 && (virt || now < VPAD_UNTIL.load(Ordering::Relaxed) || (FORCE.load(Ordering::Relaxed) != 0 && now < FORCE_UNTIL.load(Ordering::Relaxed))) && now >= REAL_PAD_UNTIL.load(Ordering::Relaxed) {
            *state = XINPUT_STATE::default();
            (*state).dwPacketNumber = VIRT_PACKET.fetch_add(1, Ordering::Relaxed);
            result = 0;
        }
        if result == 0 && virt {
            let v = VIRT.load(Ordering::Relaxed);
            let pad = &mut (*state).Gamepad;
            pad.wButtons |= v as u16;
            let axis = |byte: u64| -> i16 { ((byte as u8 as i8) as i32 * 258).clamp(-32767, 32767) as i16 };
            if (v >> 16) & 0xffff != 0 {
                pad.sThumbLX = axis(v >> 16);
                pad.sThumbLY = axis(v >> 24);
            }
            if (v >> 32) & 0xffff != 0 {
                pad.sThumbRX = axis(v >> 32);
                pad.sThumbRY = axis(v >> 40);
            }
            if v >> 48 & 1 != 0 {
                pad.bLeftTrigger = 255;
            }
            if v >> 49 & 1 != 0 {
                pad.bRightTrigger = 255;
            }
        }
    }
    if result == 0 && !state.is_null() {
        HOOK_CALLS.fetch_add(1, Ordering::Relaxed);
        let mask = MASK.load(Ordering::Relaxed);
        if mask != 0 && GetTickCount64() < MASK_UNTIL.load(Ordering::Relaxed) {
            let pad = &mut (*state).Gamepad;
            pad.wButtons &= !(mask as u16);
            if mask & MASK_LEFT_STICK != 0 {
                pad.sThumbLX = 0;
                pad.sThumbLY = 0;
            }
            if mask & MASK_RIGHT_STICK != 0 {
                pad.sThumbRX = 0;
                pad.sThumbRY = 0;
            }
            if mask & MASK_TRIGGERS != 0 {
                pad.bLeftTrigger = 0;
                pad.bRightTrigger = 0;
            }
        }
        // (presses of ours that only Elden Ring is to see: `open_er_system_menu`)
        let force = FORCE.load(Ordering::Relaxed);
        if force != 0 {
            if GetTickCount64() < FORCE_UNTIL.load(Ordering::Relaxed) {
                (*state).Gamepad.wButtons |= force as u16;
            } else {
                FORCE.store(0, Ordering::Relaxed);
            }
        }
    }
    result
}

unsafe extern "system" fn get_state_hook(index: u32, state: *mut XINPUT_STATE) -> u32 {
    masked(ORIGINAL_GET_STATE.load(Ordering::Relaxed), index, state)
}

unsafe extern "system" fn get_state_ex_hook(index: u32, state: *mut XINPUT_STATE) -> u32 {
    masked(ORIGINAL_GET_STATE_EX.load(Ordering::Relaxed), index, state)
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Hooks the XInput state functions of every XInput library the process has loaded. Returns how
/// many functions were hooked (0: Elden Ring keeps seeing the whole pad; logged).
pub fn install() -> u32 {
    let mut hooked = 0;
    for name in ["XINPUT1_4.dll", "XINPUT1_3.dll", "XINPUT9_1_0.dll"] {
        // SAFETY: plain lookups; the hook functions have the functions' signature.
        unsafe {
            let module = GetModuleHandleW(wide(name).as_ptr());
            if module.is_null() {
                continue;
            }
            if ORIGINAL_GET_STATE.load(Ordering::SeqCst) == 0 {
                if let Some(f) = GetProcAddress(module, c"XInputGetState".as_ptr() as *const u8) {
                    match kh_er_render::inline_hook(f as usize, get_state_hook as *const () as usize, &ORIGINAL_GET_STATE, "XInputGetState") {
                        Ok(()) => {
                            hooked += 1;
                            erlog!("input: XInputGetState of {name} hooked (Elden Ring's view of the pad is masked while Sora plays)");
                        }
                        Err(e) => {
                            ORIGINAL_GET_STATE.store(0, Ordering::SeqCst);
                            erlog!("input: XInputGetState of {name} NOT hooked ({e})");
                        }
                    }
                }
            }
            if ORIGINAL_GET_CAPS.load(Ordering::SeqCst) == 0 {
                if let Some(f) = GetProcAddress(module, c"XInputGetCapabilities".as_ptr() as *const u8) {
                    match kh_er_render::inline_hook(f as usize, get_caps_hook as *const () as usize, &ORIGINAL_GET_CAPS, "XInputGetCapabilities") {
                        Ok(()) => erlog!("input: XInputGetCapabilities of {name} hooked (only answers for the testing aid in.vpad)"),
                        Err(e) => {
                            ORIGINAL_GET_CAPS.store(0, Ordering::SeqCst);
                            erlog!("input: XInputGetCapabilities of {name} NOT hooked ({e})");
                        }
                    }
                }
            }
            if ORIGINAL_GET_STATE_EX.load(Ordering::SeqCst) == 0 {
                // (ordinal 100: the state with the Guide button; some games read this one)
                if let Some(f) = GetProcAddress(module, 100usize as *const u8) {
                    match kh_er_render::inline_hook(f as usize, get_state_ex_hook as *const () as usize, &ORIGINAL_GET_STATE_EX, "XInputGetStateEx") {
                        Ok(()) => {
                            hooked += 1;
                            erlog!("input: XInputGetStateEx (ordinal 100) of {name} hooked");
                        }
                        Err(e) => {
                            ORIGINAL_GET_STATE_EX.store(0, Ordering::SeqCst);
                            erlog!("input: XInputGetStateEx of {name} NOT hooked ({e})");
                        }
                    }
                }
            }
        }
    }
    if hooked == 0 {
        erlog!("input: no XInput function hooked: Elden Ring keeps reacting to the whole pad");
    }
    hooked
}

/// The hooks are gone (the renderer's shutdown removed every inline hook): our own reads use the import again.
pub fn uninstalled() {
    MASK.store(0, Ordering::SeqCst);
    ORIGINAL_GET_STATE.store(0, Ordering::SeqCst);
    ORIGINAL_GET_STATE_EX.store(0, Ordering::SeqCst);
    ORIGINAL_GET_CAPS.store(0, Ordering::SeqCst);
}

/// Renews what Elden Ring does not see of the pad for the next 250 ms (0 = everything is seen).
pub fn set_mask(mask: u32) {
    MASK.store(mask, Ordering::Relaxed);
    // SAFETY: plain getter.
    MASK_UNTIL.store(unsafe { GetTickCount64() } + 250, Ordering::Relaxed);
}

/// (IMPL_ER38) What the hook hides from the game right now (0 once the mask has lapsed).
pub fn current_mask() -> u32 {
    // SAFETY: plain getter.
    if unsafe { GetTickCount64() } < MASK_UNTIL.load(Ordering::Relaxed) {
        MASK.load(Ordering::Relaxed)
    } else {
        0
    }
}

/// Our own read of the pad: around the hook.
pub fn xinput_state(index: u32) -> Option<XINPUT_STATE> {
    let mut state = XINPUT_STATE::default();
    let original = ORIGINAL_GET_STATE.load(Ordering::Relaxed);
    // SAFETY: `state` is a valid, writable XINPUT_STATE; the function only writes into it.
    let result = unsafe {
        if original != 0 {
            let f: GetStateFn = std::mem::transmute(original);
            f(index, &mut state)
        } else {
            XInputGetState(index, &mut state)
        }
    };
    (result == 0).then_some(state)
}

// ---------------------------------------------------------------- the keyboard path

const KEY_BUTTONS: [(i32, PadButton); 14] = [
    (0x4A, PadButton::FaceBottom),    // J
    (0x4B, PadButton::FaceRight),     // K
    (0x4C, PadButton::FaceLeft),      // L
    (0x49, PadButton::FaceTop),       // I
    (0x55, PadButton::LeftShoulder),  // U
    (0x4F, PadButton::RightShoulder), // O
    (0x50, PadButton::RightTrigger),  // P
    (0x59, PadButton::LeftTrigger),   // Y
    (0x48, PadButton::RightThumb),    // H
    (0x54, PadButton::LeftThumb),     // T
    (0x26, PadButton::DPadUp),
    (0x28, PadButton::DPadDown),
    (0x25, PadButton::DPadLeft),
    (0x27, PadButton::DPadRight),
];

/// The game's window is the foreground window.
pub fn game_in_foreground() -> bool {
    // SAFETY: plain getters.
    unsafe {
        let window = GetForegroundWindow();
        if window.is_null() {
            return false;
        }
        let mut process = 0u32;
        GetWindowThreadProcessId(window, &mut process);
        process == GetCurrentProcessId()
    }
}

/// (IMPL_ER38, the log) The foreground window in words: its title and process id (ours or another's).
pub fn foreground_text() -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW;
    // SAFETY: plain getters; the buffer is ours and its length is passed.
    unsafe {
        let window = GetForegroundWindow();
        if window.is_null() {
            return "none".to_string();
        }
        let mut process = 0u32;
        GetWindowThreadProcessId(window, &mut process);
        let mut buffer = [0u16; 128];
        let n = GetWindowTextW(window, buffer.as_mut_ptr(), buffer.len() as i32).max(0) as usize;
        format!("\"{}\" of process {process}{}", String::from_utf16_lossy(&buffer[..n.min(buffer.len())]), if process == GetCurrentProcessId() { " (the game's own)" } else { "" })
    }
}

/// The keyboard as Sora's pad: (left stick, buttons held). Nothing while the game is in the background.
pub fn keyboard() -> (Vec2, PadButtons) {
    let mut buttons = PadButtons::NONE;
    if !game_in_foreground() {
        return (Vec2::ZERO, buttons);
    }
    // SAFETY: plain getter.
    let down = |vk: i32| unsafe { GetAsyncKeyState(vk) } as u16 & 0x8000 != 0;
    for (vk, button) in KEY_BUTTONS {
        if down(vk) {
            buttons.set(button, true);
        }
    }
    let axis = |plus: i32, minus: i32| f32::from(u8::from(down(plus))) - f32::from(u8::from(down(minus)));
    (Vec2::new(axis(0x44, 0x41), axis(0x57, 0x53)), buttons)
}

// ---------------------------------------------------------------- Elden Ring's interface

/// What Elden Ring's interface is doing, read once a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ErUi {
    /// `CSFeManImp::hud_state` (0 HideAll, 1 ShowAll = the main menu, 2 PopupMenu, 3 Default; 255 = not read).
    pub hud_state: u8,
    /// A menu of Elden Ring has the input: Sora gets none, Elden Ring sees the whole pad.
    pub menu: bool,
    /// (IMPL_ER10) A real-time cutscene plays (the game's cutscene manager): as a menu, and Sora's HUD is hidden.
    pub cutscene: bool,
}

pub fn read_ui(hud_hidden_by_us: bool) -> ErUi {
    use eldenring::cs::CSFeManImp;
    use fromsoftware_shared::FromStatic;
    // SAFETY: game thread (a task phase); the singleton lives as long as the game.
    let Ok(fe) = (unsafe { CSFeManImp::instance() }) else { return ErUi { hud_state: 255, menu: false, cutscene: false } };
    // SAFETY: a one-byte field (read as a byte: an unknown value must not become an enum).
    let hud_state = unsafe { *(std::ptr::addr_of!(fe.hud_state) as *const u8) };
    // (with `hud.hide` we hold the state at HideAll ourselves: the game's own value is not readable then)
    // THE RULE (measured, IMPL_ER4_STATE.md): in gameplay the state is Default (3); the main menu
    // shows ShowAll (1), popups PopupMenu (2), the map / a grace / a cutscene HideAll (0).
    ErUi { hud_state, menu: !hud_hidden_by_us && hud_state != 3, cutscene: false }
}

/// (IMPL_ER23A) Elden Ring's popup-menu object (SDK `CSMenuManImp::popup_menu` -> `CSPopupMenu`): its top menu job (+ 0xb0)
/// and the talk id (+ 0x180), by the SDK's layout; checked reads. NOT MEASURED BY US: guards.rs `DialogWatch` is built so
/// that a field that does not behave as its name says takes nothing from Sora, and every change is logged.
pub fn popup_state() -> Option<crate::guards::PopupState> {
    use eldenring::cs::CSMenuManImp;
    use fromsoftware_shared::FromStatic;
    // SAFETY: game thread; the pointer is checked, the fields are read through the checked reader.
    let menu = unsafe { CSMenuManImp::instance() }.ok()?;
    let popup = crate::word!(menu.popup_menu);
    if !crate::game::plausible(popup) {
        return None;
    }
    Some(crate::guards::PopupState { top_job: crate::probe::word(popup + 0xb0)?, talk_id: crate::probe::u32_at(popup + 0x180)? as i32 })
}

/// The `created` / `visible` bits of the 0x46 interface elements of `CSMenuManImp` (discovery of
/// which element is the action-button prompt: `dbg.ui`).
pub fn ui_states() -> Option<[u8; 0x46]> {
    use eldenring::cs::CSMenuManImp;
    use fromsoftware_shared::FromStatic;
    // SAFETY: game thread; plain bytes.
    let menu = unsafe { CSMenuManImp::instance() }.ok()?;
    let mut out = [0u8; 0x46];
    for (k, state) in menu.ui_states.iter().enumerate() {
        out[k] = u8::from(state.created()) | u8::from(state.visible()) << 1;
    }
    Some(out)
}

// ---------------------------------------------------------------- Elden Ring's system menu, opened by us

/// Buttons our XInput hook ADDS to what Elden Ring reads, until `FORCE_UNTIL` (GetTickCount64).
static FORCE: AtomicU32 = AtomicU32::new(0);
static FORCE_UNTIL: AtomicU64 = AtomicU64::new(0);
/// Presses still to make: (milliseconds after the one before, `wButtons` bits), stored last-first.
static PAD_SCRIPT: std::sync::Mutex<Vec<(u64, u16)>> = std::sync::Mutex::new(Vec::new());
static PAD_NEXT_AT: AtomicU64 = AtomicU64::new(0);
const PRESS_MS: u64 = 120;

/// OPENS ELDEN RING'S SYSTEM MENU (settings, save and quit): the intended caller is the Kingdom
/// Hearts camp menu's Config entry (a later overlay); `er.sysmenu` of the control file fires it
/// for testing. HOW: presses that only Elden Ring sees, put into the pad state our XInput hook
/// hands it: Start (its main menu), D-pad Up (the cursor wraps from Equipment to System, the last
/// entry), A (OK): the System page, tab "Game Options" (the other tabs: LB / RB there). It needs a
/// CONNECTED PAD (the hook only edits a state the system returned) and the default menu layout.
/// No function of the SDK opens a menu; keys sent with SendInput from inside the game were not
/// seen by it (tried, run 5). Returns false when it cannot be done now.
pub fn open_er_system_menu(ui: ErUi) -> bool {
    // (IMPL_ER8: a connected pad, or the testing aid's virtual one: MEASURED, with no pad Elden Ring does not read XInput)
    if ui.menu || ORIGINAL_GET_STATE.load(Ordering::Relaxed) == 0 || ((0..4).all(|i| xinput_state(i).is_none()) && !vpad_live()) {
        return false;
    }
    let mut script = PAD_SCRIPT.lock().unwrap();
    if !script.is_empty() {
        return false;
    }
    *script = vec![(450, 0x1000), (800, 0x0001), (0, XI_START)];
    PAD_NEXT_AT.store(0, Ordering::Relaxed);
    true
}

/// (ZETTA 2026-10-23, zboss.rs) THE GAME'S OWN CUTSCENE SKIP as a player makes it: Menu (Start), then A after 0.7 s,
/// pressed for Elden Ring through the hook (a pad must be one it reads, as `open_er_system_menu`). False: not pressed.
pub fn skip_cutscene() -> bool {
    if ORIGINAL_GET_STATE.load(Ordering::Relaxed) == 0 || ((0..4).all(|i| xinput_state(i).is_none()) && !vpad_live()) {
        return false;
    }
    let Ok(mut script) = PAD_SCRIPT.lock() else { return false };
    if !script.is_empty() {
        return false;
    }
    // (popped from the end: Start first, A 0.7 s after it)
    *script = vec![(0, 0x1000), (700, XI_START)];
    PAD_NEXT_AT.store(0, Ordering::Relaxed);
    true
}

/// Makes the next press of the script when it is due (call once a frame).
pub fn run_key_script() {
    // SAFETY: plain getter.
    let now = unsafe { GetTickCount64() };
    if now < PAD_NEXT_AT.load(Ordering::Relaxed) {
        return;
    }
    let Ok(mut script) = PAD_SCRIPT.try_lock() else { return };
    let Some((_, buttons)) = script.pop() else { return };
    FORCE.store(u32::from(buttons), Ordering::Relaxed);
    FORCE_UNTIL.store(now + PRESS_MS, Ordering::Relaxed);
    PAD_NEXT_AT.store(now + PRESS_MS + script.last().map_or(0, |k| k.0), Ordering::Relaxed);
}

// ---------------------------------------------------------------- what Elden Ring would read (testing aid)

static PROBE_LAST: std::sync::Mutex<Option<(u32, u16, [i8; 4], [u8; 2])>> = std::sync::Mutex::new(None);

/// `dbg.padprobe = 1` (IMPL_ER8, testing aid): reads slot 0 THROUGH THE HOOKED FUNCTION, as the game's own call
/// does, and logs every change of what comes back: the pad as Elden Ring is shown it (after the script's
/// virtual pad, the mask and our own presses). MEASURED: with no pad connected Elden Ring does not call XInput
/// at all (it looks for devices through DirectInput first), so this is how the ownership rules are checked
/// without a pad. (The probe's own calls count in the log line's "call(s)".)
pub fn probe_log() {
    let mut state = XINPUT_STATE::default();
    // SAFETY: `state` is a valid, writable XINPUT_STATE; the import leads to the hooked function.
    let result = unsafe { XInputGetState(0, &mut state) };
    let pad = state.Gamepad;
    let now = (result, pad.wButtons, [(pad.sThumbLX >> 8) as i8, (pad.sThumbLY >> 8) as i8, (pad.sThumbRX >> 8) as i8, (pad.sThumbRY >> 8) as i8], [pad.bLeftTrigger, pad.bRightTrigger]);
    let Ok(mut last) = PROBE_LAST.lock() else { return };
    if *last != Some(now) {
        *last = Some(now);
        if result != 0 {
            erlog!("padprobe: Elden Ring would read slot 0 as NOT CONNECTED ({result})");
        } else {
            erlog!("padprobe: Elden Ring would read buttons {:#06x} (0x0010 Start, 0x0020 View, 0x0100 LB, 0x8000 Y, 0x1000 A, 0x2000 B), sticks L ({}, {}) R ({}, {}) /127, triggers {} {}", now.1, now.2[0], now.2[1], now.2[2], now.2[3], now.3[0], now.3[1]);
        }
    }
}

// ---------------------------------------------------------------- one press for Elden Ring (the save point's commands)

/// `GetTickCount64` of the game's latest call through the hooked pad function (0: never).
static ER_CALL_AT: AtomicU64 = AtomicU64::new(0);

pub const XI_A: u16 = 0x1000;
pub const XI_B: u16 = 0x2000;
/// Keyboard scan codes of Elden Ring's default layout: E (the action button), Q (cancel), G (the map).
pub const SCAN_E: u16 = 0x12;
pub const SCAN_Q: u16 = 0x10;
pub const SCAN_G: u16 = 0x22;
pub const SCAN_ESC: u16 = 0x01;

/// Elden Ring is reading a connected pad through our hook right now (so a press put into the pad
/// state reaches it).
pub fn er_reads_pad() -> bool {
    // SAFETY: plain getter.
    let now = unsafe { GetTickCount64() };
    ORIGINAL_GET_STATE.load(Ordering::Relaxed) != 0 && now.saturating_sub(ER_CALL_AT.load(Ordering::Relaxed)) < 500 && ((0..4).any(|i| xinput_state(i).is_some()) || vpad_live())
}

/// (savepoint) ONE PRESS THAT ONLY ELDEN RING SEES: the pad button `buttons` through the pad hook
/// when the game reads a pad (as `open_er_system_menu` does it), else the key of scan code `scan`
/// (SendInput with the scan-code flag, the way er\tools\keys.ps1 does from outside; the game's
/// window must be the foreground window). Returns how it was sent, or None when neither way is open.
pub fn press_for_er(buttons: u16, scan: u16) -> Option<&'static str> {
    press_for_er_try(buttons, scan, 0)
}


/// (IMPL_ER10) As `press_for_er`, for the `attempt`-th try of the same press: THE PAD HOOK IS THE FIRST ROUTE
/// whenever the game reads a pad; every second try goes by the keyboard when that route is open too (a
/// press that did nothing may have gone the route the game is not listening on).
pub fn press_for_er_try(buttons: u16, scan: u16, attempt: u8) -> Option<&'static str> {
    // SAFETY: plain getter.
    let now = unsafe { GetTickCount64() };
    let keyboard = scan != 0 && game_in_foreground();
    if er_reads_pad() && (attempt % 2 == 0 || !keyboard) {
        FORCE.store(u32::from(buttons), Ordering::Relaxed);
        FORCE_UNTIL.store(now + PRESS_MS, Ordering::Relaxed);
        return Some("pad hook");
    }
    if keyboard {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE};
        let key = |up: bool| INPUT { r#type: INPUT_KEYBOARD, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: 0, wScan: scan, dwFlags: KEYEVENTF_SCANCODE | if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, dwExtraInfo: 0 } } };
        // SAFETY: one well-formed INPUT each; the key is released by the thread below 80 ms later.
        unsafe { SendInput(1, &key(false), std::mem::size_of::<INPUT>() as i32) };
        let up = key(true);
        let _ = std::thread::Builder::new().name("kh-key-up".into()).spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(80));
            // SAFETY: as above.
            unsafe { SendInput(1, &up, std::mem::size_of::<INPUT>() as i32) };
        });
        return Some("keyboard");
    }
    None
}
