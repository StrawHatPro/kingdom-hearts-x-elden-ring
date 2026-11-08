//! (CAMP-OPTIONS) ELDEN RING'S KEYBOARD AND MOUSE, MASKED AS ITS PAD IS: the game reads both through DirectInput 8 (it imports
//! DirectInput8Create; no raw input, no GetAsyncKeyState: checked in eldenring.exe 2.7.2.0), so window messages never reach
//! it. With KH3's own keyboard and mouse defaults (remap_er.rs) many KH keys are also the game's (E, F, R, Space, the mouse
//! buttons): without this mask the hidden Tarnished would act on them too, and on the title the game's own list would move
//! under our settings screen.
//!
//! HOW: the two methods every DirectInput device of the process shares, `IDirectInputDevice8W::GetDeviceState` (vtable slot
//! 9) and `GetDeviceData` (slot 10), are replaced in that vtable (found by creating a keyboard and a mouse device of our own,
//! put back at the payload's stop). After the game's own call, what the level asks for is taken out: 1 = the keys and mouse
//! buttons the KH table binds (Sora plays), 2 = every key and button (a screen of ours is up). The mouse's movement stays the
//! game's (its camera when ours is off). Our own reading (GetAsyncKeyState) is not DirectInput: untouched.
//!
//! (CAMP-OPTIONS follow-up, "camera invert must cover mouse look too") THE MOUSE'S MOVEMENT IS ALSO READ HERE for the KH
//! camera's mouse look (`take_mouse_motion`, cam.rs): the counts the game's own reads return, summed (the buffered path's
//! when the game reads that one, else the immediate one's, never both), left in the game's data as they were.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::erlog;

/// 0 nothing taken out, 1 the KH-bound inputs, 2 every key and button.
static LEVEL: AtomicU8 = AtomicU8::new(0);
/// The keyboard's DirectInput key codes the KH table binds (a 256-bit set), the mouse buttons (bits 0 .. 4) and the wheel (bit 7).
static KEYS: Mutex<[u64; 4]> = Mutex::new([0; 4]);
static MOUSE: AtomicU8 = AtomicU8::new(0);
/// (CAMP-OPTIONS) the mouse's movement since the last take (counts, x right, y down): through GetDeviceState, through
/// GetDeviceData, and whether the buffered path ever carried movement.
static STATE_X: AtomicI32 = AtomicI32::new(0);
static STATE_Y: AtomicI32 = AtomicI32::new(0);
static DATA_X: AtomicI32 = AtomicI32::new(0);
static DATA_Y: AtomicI32 = AtomicI32::new(0);
static DATA_MOVES: AtomicBool = AtomicBool::new(false);
/// The window's raw input (WM_INPUT, when the game registered the mouse for it): read beside the game, preferred when it moves.
static RAW_X: AtomicI32 = AtomicI32::new(0);
static RAW_Y: AtomicI32 = AtomicI32::new(0);
static RAW_MOVES: AtomicBool = AtomicBool::new(false);

/// (CAMP-OPTIONS) A raw-input mouse movement the game's window was sent (remap_er.rs's window procedure).
pub fn add_raw_motion(dx: i32, dy: i32) {
    RAW_X.fetch_add(dx, Ordering::Relaxed);
    RAW_Y.fetch_add(dy, Ordering::Relaxed);
    RAW_MOVES.store(true, Ordering::Relaxed);
}
static TOLD_MOTION: AtomicBool = AtomicBool::new(false);

/// (CAMP-OPTIONS) THE MOUSE'S MOVEMENT since the last call (counts: x right, y down; DirectInput's relative axes).
pub fn take_mouse_motion() -> (i32, i32) {
    let (sx, sy) = (STATE_X.swap(0, Ordering::Relaxed), STATE_Y.swap(0, Ordering::Relaxed));
    let (dx, dy) = (DATA_X.swap(0, Ordering::Relaxed), DATA_Y.swap(0, Ordering::Relaxed));
    let (rx, ry) = (RAW_X.swap(0, Ordering::Relaxed), RAW_Y.swap(0, Ordering::Relaxed));
    let data = DATA_MOVES.load(Ordering::Relaxed);
    let raw = RAW_MOVES.load(Ordering::Relaxed);
    let (x, y) = if raw { (rx, ry) } else if data { (dx, dy) } else { (sx, sy) };
    if (x, y) != (0, 0) && !TOLD_MOTION.swap(true, Ordering::Relaxed) {
        erlog!("remap: the mouse's movement is read through {} (the KH camera's mouse look)", if raw { "the game window's raw input (WM_INPUT)" } else if data { "the game's DirectInput GetDeviceData" } else { "the game's DirectInput GetDeviceState" });
    }
    crate::button_icons_er::mouse_moved((x, y)); // (BUTTON-ICONS) the device used last
    (x, y)
}

static VTABLE: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_STATE: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_DATA: AtomicUsize = AtomicUsize::new(0);
/// (CAMP-OPTIONS) the mouse device's vtable when it is not the keyboard's, and its two methods.
static MOUSE_VTABLE: AtomicUsize = AtomicUsize::new(0);
static MOUSE_STATE: AtomicUsize = AtomicUsize::new(0);
static MOUSE_DATA: AtomicUsize = AtomicUsize::new(0);
static TRIED: AtomicU8 = AtomicU8::new(0);
/// Device pointer -> DI8DEVTYPE (the low byte of DIDEVCAPS.dwDevType): 0x13 keyboard, 0x12 mouse.
static KINDS: Mutex<Vec<(usize, u8)>> = Mutex::new(Vec::new());

/// 0 nothing, 1 the KH-bound inputs, 2 every key and button, 3 (CC-MUTE: Elden Ring's creation pressed for the player,
/// ngquiet.rs) level 2 plus the mouse's movement; a release is never taken there, so nothing the player held stays down.
pub fn set_level(level: u8) {
    LEVEL.store(level.min(3), Ordering::Relaxed);
    if level < 3 {
        FROZEN.store(NO_POINT, Ordering::Relaxed); // (CC-MUTE) the pointer is the system's again
    }
}

// ------------------------------------------------------------------------------------------------ (CC-MUTE) the pointer

// THE USER (2026-11-07): "block mouse pointer as well". Elden Ring's other ways to the mouse and keyboard besides DirectInput and
// its window's messages, READ from the import names of both executables (2.7.1.0 and 2.7.2.0, gamever.rs `pointer_imports`):
// user32's GetCursorPos (+ ScreenToClient, a pure conversion of the point it is handed), GetKeyState and GetKeyboardState (the
// mouse buttons are VK 1, 2, 4, 5, 6 there). Neither file imports GetRawInputData, GetRawInputBuffer, RegisterRawInputDevices,
// GetCursorInfo, GetPhysicalCursorPos, GetMessagePos or GetMouseMovePointsEx (WM_INPUT, which DirectInput itself may cause, is
// already swallowed by the window procedure, remap_er.rs). ClipCursor / SetCursorPos only place the pointer: nothing is read.
// At level 3 those three are answered by our detours (user32's own exports, inline-hooked as inputs.rs does XInput's, undone
// with every other inline hook): GetCursorPos the point it gave when the hold began (frozen), the key states with nothing held
// but our own allowed presses (the toggle bit kept). Below level 3 they are the original functions, untouched.

/// The frozen pointer, packed (x, y); `NO_POINT` = none frozen.
static FROZEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(NO_POINT);
const NO_POINT: u64 = u64::MAX;
static ORIG_CURSOR: AtomicUsize = AtomicUsize::new(0);
static ORIG_KEY_STATE: AtomicUsize = AtomicUsize::new(0);
static ORIG_KEYBOARD_STATE: AtomicUsize = AtomicUsize::new(0);

/// (pure) A point packed into the frozen word and back.
pub fn pack_point(x: i32, y: i32) -> u64 {
    (u64::from(x as u32) << 32) | u64::from(y as u32)
}

pub fn unpack_point(word: u64) -> (i32, i32) {
    ((word >> 32) as u32 as i32, word as u32 as i32)
}

/// (pure) GetCursorPos's answer: `level` now, the frozen word, the live point (None: the call failed). Returns (the point to
/// give, the frozen word after). Level 3 freezes the first live point and gives it until the level drops.
pub fn pointer_answer(level: u8, frozen: u64, live: Option<(i32, i32)>) -> (Option<(i32, i32)>, u64) {
    if level < 3 {
        return (live, NO_POINT);
    }
    if frozen != NO_POINT {
        return (Some(unpack_point(frozen)), frozen);
    }
    match live {
        Some((x, y)) => (live, pack_point(x, y)),
        None => (None, NO_POINT),
    }
}

/// (pure) A key state (GetKeyState's SHORT, or a GetKeyboardState byte widened) as the game is shown it: at level 3 nothing is
/// held but our own allowed presses; the toggle bit (Caps Lock ...) is kept.
pub fn key_state_answer(level: u8, allowed: bool, raw: i16) -> i16 {
    if level >= 3 && !allowed {
        raw & 1
    } else {
        raw
    }
}

/// The mouse buttons' virtual keys.
fn mouse_vk(vk: i32) -> bool {
    matches!(vk, 0x01 | 0x02 | 0x04 | 0x05 | 0x06)
}

/// A virtual key is one of our own allowed presses (a key, never a mouse button).
fn vk_allowed(vk: i32) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MapVirtualKeyW, MAPVK_VK_TO_VSC_EX};
    if mouse_vk(vk) || !(0..256).contains(&vk) {
        return false;
    }
    // SAFETY: plain getter.
    let sc = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC_EX) };
    sc != 0 && allowed(((sc & 0x7F) | if sc & 0xFF00 == 0xE000 { 0x80 } else { 0 }) as usize)
}

unsafe extern "system" fn cursor_hook(point: *mut windows_sys::Win32::Foundation::POINT) -> i32 {
    let original: unsafe extern "system" fn(*mut windows_sys::Win32::Foundation::POINT) -> i32 = std::mem::transmute(ORIG_CURSOR.load(Ordering::Relaxed));
    let level = LEVEL.load(Ordering::Relaxed);
    let frozen = FROZEN.load(Ordering::Relaxed);
    if level < 3 || point.is_null() {
        return original(point);
    }
    let live = if frozen == NO_POINT { (original(point) != 0).then(|| ((*point).x, (*point).y)) } else { None };
    let (give, after) = pointer_answer(level, frozen, live);
    FROZEN.store(after, Ordering::Relaxed);
    match give {
        Some((x, y)) => {
            (*point).x = x;
            (*point).y = y;
            1
        }
        None => 0,
    }
}

unsafe extern "system" fn key_state_hook(vk: i32) -> i16 {
    let original: unsafe extern "system" fn(i32) -> i16 = std::mem::transmute(ORIG_KEY_STATE.load(Ordering::Relaxed));
    let raw = original(vk);
    let level = LEVEL.load(Ordering::Relaxed);
    if level < 3 {
        return raw;
    }
    key_state_answer(level, vk_allowed(vk), raw)
}

unsafe extern "system" fn keyboard_state_hook(state: *mut u8) -> i32 {
    let original: unsafe extern "system" fn(*mut u8) -> i32 = std::mem::transmute(ORIG_KEYBOARD_STATE.load(Ordering::Relaxed));
    let ok = original(state);
    let level = LEVEL.load(Ordering::Relaxed);
    if ok == 0 || level < 3 || state.is_null() {
        return ok;
    }
    for vk in 0..256i32 {
        let byte = state.add(vk as usize);
        if *byte & 0x80 != 0 {
            *byte = key_state_answer(level, vk_allowed(vk), i16::from(*byte)) as u8;
        }
    }
    ok
}

/// user32's three functions detoured (once per payload, from `install`; undone with the renderer's other inline hooks).
fn install_pointer() {
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    let name: Vec<u16> = "user32.dll\0".encode_utf16().collect();
    // SAFETY: plain lookups; each hook has its function's signature.
    unsafe {
        let module = GetModuleHandleW(name.as_ptr());
        if module.is_null() {
            erlog!("remap: (CC-MUTE) user32.dll not loaded: the pointer is not held");
            return;
        }
        let hooks: [(&std::ffi::CStr, usize, &AtomicUsize); 3] = [
            (c"GetCursorPos", cursor_hook as *const () as usize, &ORIG_CURSOR),
            (c"GetKeyState", key_state_hook as *const () as usize, &ORIG_KEY_STATE),
            (c"GetKeyboardState", keyboard_state_hook as *const () as usize, &ORIG_KEYBOARD_STATE),
        ];
        for (function, hook, original) in hooks {
            let what = function.to_str().unwrap_or("user32");
            let Some(f) = GetProcAddress(module, function.as_ptr().cast()) else {
                erlog!("remap: (CC-MUTE) user32!{what} not found: not held");
                continue;
            };
            match kh_er_render::inline_hook(f as usize, hook, original, what) {
                Ok(()) => erlog!("remap: (CC-MUTE) user32!{what} detoured (held only while Elden Ring's creation is pressed for the player)"),
                Err(e) => {
                    original.store(0, Ordering::SeqCst);
                    erlog!("remap: (CC-MUTE) user32!{what} NOT detoured ({e})");
                }
            }
        }
    }
}

/// (CC-MUTE) OUR OWN KEY PRESSES PASS: the keys (scan codes, 0x100 = extended) newgame.rs sends by SendInput while the game's
/// keyboard is held from the player, for `ALLOW_MS` (the press, its release 80 ms later and the Shift's after it).
static ALLOW: Mutex<[u16; 2]> = Mutex::new([0; 2]);
static ALLOW_UNTIL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
const ALLOW_MS: u64 = 500;

fn tick() -> u64 {
    // SAFETY: plain getter.
    unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() }
}

/// (CC-MUTE) A scan code (0x100 = extended, as SendInput's callers write it) as DirectInput's key code (pure).
pub fn dik_of_scan(scan: u16) -> usize {
    usize::from(scan & 0x7F) | if scan & 0x100 != 0 { 0x80 } else { 0 }
}

/// (CC-MUTE) `scans` (at most two) are ours for the next `ALLOW_MS`: they reach the game through any level.
pub fn allow_press(scans: &[u16]) {
    if let Ok(mut a) = ALLOW.lock() {
        *a = [scans.first().copied().unwrap_or(0), scans.get(1).copied().unwrap_or(0)];
    }
    ALLOW_UNTIL.store(tick() + ALLOW_MS, Ordering::Relaxed);
}

/// (CC-MUTE, remap_er.rs's window procedure) A key message's scan code is one of our own allowed presses.
pub fn scan_allowed(scan: u16) -> bool {
    scan & 0xFF != 0 && allowed(dik_of_scan(scan))
}

fn allowed(dik: usize) -> bool {
    tick() < ALLOW_UNTIL.load(Ordering::Relaxed) && ALLOW.lock().ok().is_some_and(|a| a.iter().any(|s| *s != 0 && dik_of_scan(*s) == dik))
}

/// (CC-MUTE) WHAT THE GAME DOES NOT SEE OF ONE BUFFERED ENTRY (pure): `kind` 0x13 keyboard / 0x12 mouse, `ofs` its offset,
/// `down` its press bit (dwData & 0x80), `mouse` the mouse inputs taken at this level, `key_taken` the key rule's answer. At
/// level 3 the mouse's movement goes too and no release is ever taken.
pub fn entry_taken(kind: u8, ofs: usize, down: bool, level: u8, mouse: u8, key_taken: bool) -> bool {
    if level >= 3 && !down && (kind == 0x13 || (kind == 0x12 && ofs >= 12)) {
        return false;
    }
    match kind {
        0x13 => key_taken,
        // DIMOFS_X 0, DIMOFS_Y 4, DIMOFS_Z 8, DIMOFS_BUTTON0.. 12..
        0x12 => (level >= 3 && ofs < 8) || (ofs == 8 && mouse & 0x80 != 0) || (12..17).contains(&ofs) && mouse & (1 << (ofs - 12)) != 0 || (level >= 2 && ofs >= 12),
        _ => false,
    }
}

/// The KH table's keyboard keys (virtual-key codes) and mouse inputs, as DirectInput codes.
pub fn set_bound(codes: &[u16]) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MapVirtualKeyW, MAPVK_VK_TO_VSC_EX};
    let mut keys = [0u64; 4];
    let mut mouse = 0u8;
    for &c in codes {
        match c {
            kh_host::remap::MOUSE_LEFT => mouse |= 1,
            kh_host::remap::MOUSE_RIGHT => mouse |= 2,
            kh_host::remap::MOUSE_MIDDLE => mouse |= 4,
            kh_host::remap::MOUSE_X1 => mouse |= 8,
            kh_host::remap::MOUSE_X2 => mouse |= 16,
            kh_host::remap::WHEEL_UP | kh_host::remap::WHEEL_DOWN => mouse |= 0x80,
            0x07..=0xFE => {
                // SAFETY: plain getter.
                let sc = unsafe { MapVirtualKeyW(u32::from(c), MAPVK_VK_TO_VSC_EX) };
                if sc == 0 {
                    continue;
                }
                // (a DirectInput code: the scan code, 0x80 for the extended (E0) ones)
                let dik = ((sc & 0x7F) | if sc & 0xFF00 == 0xE000 { 0x80 } else { 0 }) as usize;
                keys[dik / 64] |= 1 << (dik % 64);
            }
            _ => {}
        }
    }
    if let Ok(mut k) = KEYS.lock() {
        *k = keys;
    }
    MOUSE.store(mouse, Ordering::Relaxed);
}

/// (CAMP-OPTIONS, OUTFIT-UNLOCK's report: "E never reaches ER") ELDEN RING'S ACTION KEY (its keyboard "Event Action", E by
/// default: DirectInput DIK_E) while its own action prompt is up and no KH prompt or menu owns the moment (sora.rs: the prompt
/// is up, the save point does not offer its Y, no menu of ours): talk, open, pick up, activate are the game's, so the key is
/// the game's then, KH-bound or not.
static ER_ACTION_OPEN: AtomicBool = AtomicBool::new(false);
pub const DIK_ER_ACTION: usize = 0x12;

pub fn set_er_action_open(open: bool) {
    ER_ACTION_OPEN.store(open, Ordering::Relaxed);
}

fn key_taken(dik: usize, level: u8) -> bool {
    let bound = KEYS.lock().ok().is_some_and(|k| dik < 256 && k[dik / 64] & (1 << (dik % 64)) != 0);
    taken(dik, level, bound, ER_ACTION_OPEN.load(Ordering::Relaxed)) && !allowed(dik)
}

/// WHAT THE GAME DOES NOT SEE OF A KEY (pure): level 2 (a screen of ours) everything; level 1 (Sora plays) the keys the KH table
/// binds, except the game's action key while its own prompt is open to it; level 0 nothing.
pub fn taken(dik: usize, level: u8, bound: bool, er_action_open: bool) -> bool {
    match level {
        0 => false,
        1 => bound && !(er_action_open && dik == DIK_ER_ACTION),
        _ => true,
    }
}

fn kind_of(device: usize) -> u8 {
    if let Ok(k) = KINDS.lock() {
        if let Some((_, t)) = k.iter().find(|(d, _)| *d == device) {
            return *t;
        }
    }
    // DIDEVCAPS (44 bytes, dwSize first) through GetCapabilities (slot 3)
    let mut caps = [0u32; 11];
    caps[0] = 44;
    // SAFETY: `device` is a live IDirectInputDevice8W (the game is inside a call on it); its own GetCapabilities.
    let kind = unsafe {
        let vt = *(device as *const *const usize);
        let get_caps: unsafe extern "system" fn(usize, *mut u32) -> i32 = std::mem::transmute(*vt.add(3));
        if get_caps(device, caps.as_mut_ptr()) >= 0 { (caps[2] & 0xFF) as u8 } else { 0 }
    };
    if let Ok(mut k) = KINDS.lock() {
        k.push((device, kind));
    }
    kind
}

unsafe extern "system" fn state_hook(device: usize, size: u32, data: *mut u8) -> i32 {
    state_read(ORIGINAL_STATE.load(Ordering::Relaxed), device, size, data)
}

/// (CAMP-OPTIONS) the mouse device's own vtable (seen in game: not the keyboard's).
unsafe extern "system" fn mouse_state_hook(device: usize, size: u32, data: *mut u8) -> i32 {
    state_read(MOUSE_STATE.load(Ordering::Relaxed), device, size, data)
}

unsafe extern "system" fn mouse_data_hook(device: usize, object_size: u32, objects: *mut u8, count: *mut u32, flags: u32) -> i32 {
    data_read(MOUSE_DATA.load(Ordering::Relaxed), device, object_size, objects, count, flags)
}

unsafe fn state_read(original: usize, device: usize, size: u32, data: *mut u8) -> i32 {
    let original: unsafe extern "system" fn(usize, u32, *mut u8) -> i32 = std::mem::transmute(original);
    let hr = original(device, size, data);
    if hr >= 0 {
        seen(kind_of(device), false, size);
    }
    // (CAMP-OPTIONS) the mouse's movement (DIMOUSESTATE: lX, lY first), before anything is taken out
    if hr >= 0 && !data.is_null() && (size == 16 || size == 20) && kind_of(device) == 0x12 {
        STATE_X.fetch_add((data as *const i32).read_unaligned(), Ordering::Relaxed);
        STATE_Y.fetch_add((data.add(4) as *const i32).read_unaligned(), Ordering::Relaxed);
    }
    let level = LEVEL.load(Ordering::Relaxed);
    if hr < 0 || level == 0 || data.is_null() {
        return hr;
    }
    match size {
        // the keyboard: 256 key bytes
        256 => {
            for dik in 0..256usize {
                if *data.add(dik) != 0 && key_taken(dik, level) {
                    *data.add(dik) = 0;
                }
            }
        }
        // the mouse: DIMOUSESTATE (16) / DIMOUSESTATE2 (20): x, y, z, then the buttons
        16 | 20 => {
            let mouse = if level >= 2 { 0xFF } else { MOUSE.load(Ordering::Relaxed) };
            if mouse & 0x80 != 0 {
                (data.add(8) as *mut i32).write_unaligned(0);
            }
            // (CC-MUTE) level 3: the movement too (already summed above for the KH camera's mouse look)
            if level >= 3 {
                (data as *mut i32).write_unaligned(0);
                (data.add(4) as *mut i32).write_unaligned(0);
            }
            for b in 0..(size as usize - 12) {
                if b < 5 && mouse & (1 << b) != 0 || level >= 2 {
                    *data.add(12 + b) = 0;
                }
            }
        }
        _ => {}
    }
    hr
}

/// DIDEVICEOBJECTDATA (DirectInput 8, 64-bit): dwOfs, dwData, dwTimeStamp, dwSequence, uAppData (pointer-sized) = 24 bytes.
unsafe extern "system" fn data_hook(device: usize, object_size: u32, objects: *mut u8, count: *mut u32, flags: u32) -> i32 {
    data_read(ORIGINAL_DATA.load(Ordering::Relaxed), device, object_size, objects, count, flags)
}

unsafe fn data_read(original: usize, device: usize, object_size: u32, objects: *mut u8, count: *mut u32, flags: u32) -> i32 {
    let original: unsafe extern "system" fn(usize, u32, *mut u8, *mut u32, u32) -> i32 = std::mem::transmute(original);
    let hr = original(device, object_size, objects, count, flags);
    if hr >= 0 {
        seen(kind_of(device), true, object_size);
    }
    // (CAMP-OPTIONS) the mouse's movement in the buffer (DIMOFS_X 0, DIMOFS_Y 4; dwData the relative counts); DIGDD_PEEK (1)
    // leaves the buffer as it was: not counted, the next read returns the same entries
    const DIGDD_PEEK: u32 = 1;
    if hr >= 0 && !objects.is_null() && !count.is_null() && object_size >= 8 && flags & DIGDD_PEEK == 0 && kind_of(device) == 0x12 {
        for i in 0..*count as usize {
            let entry = objects.add(i * object_size as usize);
            let (ofs, value) = ((entry as *const u32).read_unaligned(), (entry.add(4) as *const i32).read_unaligned());
            match ofs {
                0 => {
                    DATA_X.fetch_add(value, Ordering::Relaxed);
                    DATA_MOVES.store(true, Ordering::Relaxed);
                }
                4 => {
                    DATA_Y.fetch_add(value, Ordering::Relaxed);
                    DATA_MOVES.store(true, Ordering::Relaxed);
                }
                _ => {}
            }
        }
    }
    let level = LEVEL.load(Ordering::Relaxed);
    if hr < 0 || level == 0 || objects.is_null() || count.is_null() || object_size < 8 {
        return hr;
    }
    let kind = kind_of(device);
    let mouse = if level >= 2 { 0xFF } else { MOUSE.load(Ordering::Relaxed) };
    let n = *count as usize;
    let mut kept = 0usize;
    for i in 0..n {
        let entry = objects.add(i * object_size as usize);
        let ofs = (entry as *const u32).read_unaligned() as usize;
        let down = (entry.add(4) as *const u32).read_unaligned() & 0x80 != 0;
        let taken = entry_taken(kind, ofs, down, level, mouse, kind == 0x13 && key_taken(ofs, level));
        if !taken {
            if kept != i {
                std::ptr::copy(entry, objects.add(kept * object_size as usize), object_size as usize);
            }
            kept += 1;
        }
    }
    *count = kept as u32;
    hr
}

#[repr(C)]
struct Guid(u32, u16, u16, [u8; 8]);
const IID_IDIRECTINPUT8W: Guid = Guid(0xBF79_8031, 0x483A, 0x4DA2, [0xAA, 0x99, 0x5D, 0x64, 0xED, 0x36, 0x97, 0x00]);
const GUID_SYS_KEYBOARD: Guid = Guid(0x6F1D_2B61, 0xD5A0, 0x11CF, [0xBF, 0xC7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00]);
const GUID_SYS_MOUSE: Guid = Guid(0x6F1D_2B60, 0xD5A0, 0x11CF, [0xBF, 0xC7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00]);

/// (CAMP-OPTIONS) which of the game's device reads reached the hooks (logged once each: 1 keyboard state, 2 keyboard data,
/// 4 mouse state, 8 mouse data, 16 another device)
static SEEN: AtomicU8 = AtomicU8::new(0);

fn seen(kind: u8, data: bool, size: u32) {
    let bit = match (kind, data) {
        (0x13, false) => 1,
        (0x13, true) => 2,
        (0x12, false) => 4,
        (0x12, true) => 8,
        _ => 16,
    };
    if SEEN.fetch_or(bit, Ordering::Relaxed) & bit == 0 {
        erlog!("remap: the game reads its DirectInput {} through {} (size {size})", match kind { 0x13 => "keyboard", 0x12 => "mouse", _ => "device" }, if data { "GetDeviceData" } else { "GetDeviceState" });
    }
}

/// Finds the devices' shared vtable and replaces its two read methods (once).
pub fn install() {
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW};
    use windows_sys::Win32::System::Memory::{VirtualProtect, PAGE_EXECUTE_READWRITE};
    if TRIED.swap(1, Ordering::SeqCst) != 0 {
        return;
    }
    install_pointer(); // (CC-MUTE) user32's pointer and key-state reads
    // SAFETY: DirectInput's own creation functions with valid out pointers; the vtable's two slots are written with the page
    // made writable for that moment; every object of ours is released.
    unsafe {
        let name: Vec<u16> = "dinput8.dll\0".encode_utf16().collect();
        let module = LoadLibraryW(name.as_ptr());
        if module.is_null() {
            erlog!("remap: dinput8.dll not loaded: the game's keyboard is not masked");
            return;
        }
        let Some(create) = GetProcAddress(module, c"DirectInput8Create".as_ptr().cast()) else { return };
        let create: unsafe extern "system" fn(*mut core::ffi::c_void, u32, *const Guid, *mut usize, usize) -> i32 = std::mem::transmute(create);
        let mut di = 0usize;
        if create(GetModuleHandleW(std::ptr::null()), 0x0800, &IID_IDIRECTINPUT8W, &mut di, 0) < 0 || di == 0 {
            erlog!("remap: DirectInput could not be created: the game's keyboard is not masked");
            return;
        }
        let di_vt = *(di as *const *const usize);
        let create_device: unsafe extern "system" fn(usize, *const Guid, *mut usize, usize) -> i32 = std::mem::transmute(*di_vt.add(3));
        let release_di: unsafe extern "system" fn(usize) -> u32 = std::mem::transmute(*di_vt.add(2));
        let mut device = 0usize;
        if create_device(di, &GUID_SYS_KEYBOARD, &mut device, 0) < 0 || device == 0 {
            release_di(di);
            erlog!("remap: no DirectInput keyboard: the game's keyboard is not masked");
            return;
        }
        let vt = *(device as *const *mut usize);
        ORIGINAL_STATE.store(*vt.add(9), Ordering::SeqCst);
        ORIGINAL_DATA.store(*vt.add(10), Ordering::SeqCst);
        let mut old = 0u32;
        if VirtualProtect(vt.add(9).cast(), 16, PAGE_EXECUTE_READWRITE, &mut old) != 0 {
            *vt.add(9) = state_hook as *const () as usize;
            *vt.add(10) = data_hook as *const () as usize;
            VirtualProtect(vt.add(9).cast(), 16, old, &mut old);
            VTABLE.store(vt as usize, Ordering::SeqCst);
            erlog!("remap: the game's DirectInput keyboard and mouse are masked as its pad is (the keys KH binds while Sora plays, all of them under our screens)");
        }
        let release_device: unsafe extern "system" fn(usize) -> u32 = std::mem::transmute(*(*(device as *const *const usize)).add(2));
        release_device(device);
        // (CAMP-OPTIONS) the mouse device: the same vtable (its reads are hooked too) or another (logged)
        let mut mouse = 0usize;
        if create_device(di, &GUID_SYS_MOUSE, &mut mouse, 0) >= 0 && mouse != 0 {
            let mvt = *(mouse as *const *mut usize);
            if mvt != vt {
                MOUSE_STATE.store(*mvt.add(9), Ordering::SeqCst);
                MOUSE_DATA.store(*mvt.add(10), Ordering::SeqCst);
                let mut old = 0u32;
                if VirtualProtect(mvt.add(9).cast(), 16, PAGE_EXECUTE_READWRITE, &mut old) != 0 {
                    *mvt.add(9) = mouse_state_hook as *const () as usize;
                    *mvt.add(10) = mouse_data_hook as *const () as usize;
                    VirtualProtect(mvt.add(9).cast(), 16, old, &mut old);
                    MOUSE_VTABLE.store(mvt as usize, Ordering::SeqCst);
                }
            }
            erlog!("remap: the DirectInput mouse device has {} (its reads hooked too)", if mvt == vt { "the keyboard's vtable" } else { "a vtable of its own" });
            let release_mouse: unsafe extern "system" fn(usize) -> u32 = std::mem::transmute(*mvt.add(2));
            release_mouse(mouse);
        }
        release_di(di);
    }
}

/// The payload stops: the vtable's own methods back.
pub fn uninstalled() {
    // (CC-MUTE) the renderer's shutdown removed the user32 detours with every other inline hook
    for o in [&ORIG_CURSOR, &ORIG_KEY_STATE, &ORIG_KEYBOARD_STATE] {
        o.store(0, Ordering::SeqCst);
    }
    LEVEL.store(0, Ordering::SeqCst);
    FROZEN.store(NO_POINT, Ordering::SeqCst);
    use windows_sys::Win32::System::Memory::{VirtualProtect, PAGE_EXECUTE_READWRITE};
    let mvt = MOUSE_VTABLE.swap(0, Ordering::SeqCst) as *mut usize;
    if !mvt.is_null() {
        // SAFETY: the mouse vtable's two slots we wrote, put back with the page made writable for that moment.
        unsafe {
            let mut old = 0u32;
            if VirtualProtect(mvt.add(9).cast(), 16, PAGE_EXECUTE_READWRITE, &mut old) != 0 {
                *mvt.add(9) = MOUSE_STATE.load(Ordering::SeqCst);
                *mvt.add(10) = MOUSE_DATA.load(Ordering::SeqCst);
                VirtualProtect(mvt.add(9).cast(), 16, old, &mut old);
            }
        }
    }
    let vt = VTABLE.swap(0, Ordering::SeqCst) as *mut usize;
    if vt.is_null() {
        return;
    }
    // SAFETY: the two slots we wrote, put back with the page made writable for that moment.
    unsafe {
        let mut old = 0u32;
        if VirtualProtect(vt.add(9).cast(), 16, PAGE_EXECUTE_READWRITE, &mut old) != 0 {
            *vt.add(9) = ORIGINAL_STATE.load(Ordering::SeqCst);
            *vt.add(10) = ORIGINAL_DATA.load(Ordering::SeqCst);
            VirtualProtect(vt.add(9).cast(), 16, old, &mut old);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// (CAMP-OPTIONS) Elden Ring's talk / open / pick-up prompt takes its own action key (E) on the keyboard, KH-bound or not,
    /// while no KH prompt owns the moment; every other KH-bound key stays KH's; a remapped table that frees E leaves it alone
    #[test]
    fn co_the_games_action_prompt_gets_its_key() {
        // the default table: E is KH3's Change Target (bound)
        assert!(taken(DIK_ER_ACTION, 1, true, false), "in play E is KH's");
        assert!(!taken(DIK_ER_ACTION, 1, true, true), "the game's prompt is up: E is the game's (talk to Boc, a merchant)");
        assert!(taken(0x21, 1, true, true), "F stays KH's under the prompt (its Y goes the KH route)");
        // a remapped table that moved Change Target off E: E is never taken
        assert!(!taken(DIK_ER_ACTION, 1, false, false));
        // a screen of ours: nothing reaches the game; the game's own menus: everything
        assert!(taken(DIK_ER_ACTION, 2, true, true));
        assert!(!taken(DIK_ER_ACTION, 0, true, false));
    }

    /// (CC-MUTE) Elden Ring's creation pressed for the player (level 3): every key press, mouse button, the wheel and the
    /// movement are taken; no release is (nothing the player held stays down for the game); levels 0..2 are as before; our own
    /// presses pass by their scan codes.
    #[test]
    fn cc_level_three_takes_every_press_and_the_movement() {
        // the keyboard: presses taken, releases kept
        assert!(entry_taken(0x13, 0x12, true, 3, 0xFF, taken(0x12, 3, false, false)));
        assert!(!entry_taken(0x13, 0x12, false, 3, 0xFF, taken(0x12, 3, false, false)));
        // the mouse: the movement, the wheel, the buttons' presses taken; a button's release kept
        for ofs in [0, 4, 8, 12, 13, 16] {
            assert!(entry_taken(0x12, ofs, true, 3, 0xFF, false), "offset {ofs}");
        }
        assert!(entry_taken(0x12, 0, false, 3, 0xFF, false), "a movement count has no press bit: still taken");
        assert!(!entry_taken(0x12, 12, false, 3, 0xFF, false));
        // level 2 as before: buttons and wheel, never the movement; level 1 the KH-bound ones; level 0 nothing
        assert!(!entry_taken(0x12, 0, true, 2, 0xFF, false));
        assert!(entry_taken(0x12, 12, false, 2, 0xFF, false), "level 2 keeps its old rule (releases taken too)");
        assert!(entry_taken(0x12, 13, true, 1, 0x02, false));
        assert!(!entry_taken(0x12, 12, true, 1, 0x02, false));
        assert!(!entry_taken(0x12, 12, true, 0, 0, false));
        assert!(!entry_taken(0x13, 0x12, true, 0, 0, false));
        // our own presses: the scan codes newgame.rs sends, as DirectInput's codes
        assert_eq!(dik_of_scan(0x12), 0x12, "E");
        assert_eq!(dik_of_scan(0x1F), 0x1F, "S");
        assert_eq!(dik_of_scan(0x2A), 0x2A, "Left Shift");
        assert_eq!(dik_of_scan(0x1C), 0x1C, "Enter");
        assert_eq!(dik_of_scan(0x148), 0xC8, "Up (extended) = DIK_UP");
        assert_eq!(dik_of_scan(0x14B), 0xCB, "Left (extended) = DIK_LEFT");
        allow_press(&[0x148, 0x2A]);
        assert!(allowed(0xC8) && allowed(0x2A) && !allowed(0x48) && !allowed(0x12));
        assert!(!key_taken(0xC8, 3), "our own Up passes level 3");
        assert!(key_taken(0x12, 3), "the player's E does not");
        ALLOW_UNTIL.store(0, Ordering::Relaxed);
        assert!(key_taken(0xC8, 3), "the allowance lapses");
    }

    /// (CC-MUTE, THE USER: "block mouse pointer as well") GetCursorPos at level 3 gives the point it gave when the hold began,
    /// whatever the mouse does, until the level drops; the key states show nothing held but our own presses (toggles kept).
    #[test]
    fn cc_the_pointer_is_frozen_and_the_key_states_empty_at_level_three() {
        for (x, y) in [(0, 0), (1919, 1079), (-5, 7), (i32::MIN, i32::MAX)] {
            assert_eq!(unpack_point(pack_point(x, y)), (x, y));
        }
        // below level 3: the live point, nothing frozen
        for level in 0..3 {
            assert_eq!(pointer_answer(level, NO_POINT, Some((10, 20))), (Some((10, 20)), NO_POINT));
            assert_eq!(pointer_answer(level, pack_point(1, 2), Some((10, 20))), (Some((10, 20)), NO_POINT), "a stale freeze never shows");
        }
        // level 3: the first live point is frozen and given from then on
        let (give, frozen) = pointer_answer(3, NO_POINT, Some((640, 360)));
        assert_eq!(give, Some((640, 360)));
        assert_eq!(pointer_answer(3, frozen, None), (Some((640, 360)), frozen), "the mouse moved: still the frozen point");
        assert_eq!(pointer_answer(3, NO_POINT, None), (None, NO_POINT), "a failed first call freezes nothing");
        // the level drops (every exit path goes through set_level): the freeze is gone
        FROZEN.store(frozen, Ordering::Relaxed);
        set_level(3);
        assert_eq!(FROZEN.load(Ordering::Relaxed), frozen);
        set_level(0);
        assert_eq!(FROZEN.load(Ordering::Relaxed), NO_POINT);
        // the key states
        let down = i16::MIN; // 0x8000
        assert_eq!(key_state_answer(3, false, down), 0);
        assert_eq!(key_state_answer(3, false, down | 1), 1, "a toggle stays");
        assert_eq!(key_state_answer(3, true, down), down, "our own press passes");
        assert_eq!(key_state_answer(2, false, down), down, "below level 3 untouched");
        assert_eq!(key_state_answer(3, false, 0x80) as u8, 0, "a GetKeyboardState byte");
        assert!(mouse_vk(1) && mouse_vk(2) && mouse_vk(4) && mouse_vk(5) && mouse_vk(6) && !mouse_vk(0x10));
        assert!(!vk_allowed(1), "a mouse button is never ours");
        assert!(!scan_allowed(0), "no scan code, nothing allowed");
    }
}
