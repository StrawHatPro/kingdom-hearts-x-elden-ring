//! (BUTTON-ICONS) THE BUTTON-ICON STATE OF THE KH UI, KEPT LIVE IN ELDEN RING: once a frame (eroptions.rs `frame`) the
//! "Button Icons" setting (`hud.button_icons`), the two remap tables (`remap.pad`, `remap.kbm`), the device used last and
//! the connected controller's family make kh-host button_icons.rs's state, published to kh-ui glyphs.rs (every icon of the
//! HUD and the menus draws through it; a change redraws them).
//!
//! THE DEVICE USED LAST: the controller when one of its buttons is pressed or a stick leaves its dead zone (pad.rs), the
//! keyboard / mouse when a key or mouse button plays a role or the mouse moves (remap_er.rs, dinput_mask.rs).
//! THE CONTROLLER'S FAMILY: a Sony controller among the system's HID game controllers (vendor 0x054C, usage page 1, usage
//! 4 / 5; looked for every 2 s) = PlayStation, any other = Xbox (KH3's auto setting asks the connected device too, its
//! default "xboxone", 0x141595160). The KH side reads every controller through XInput (a PlayStation pad through Steam Input
//! or DS4Windows); the family only chooses the art.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use kh_host::button_icons::{glyph_state, shown, Last, Setting, SETTING_KEY};
use kh_host::remap::{Device, Table};
use kh_ui::glyphs::Family;

use crate::control::Control;
use crate::erlog;

static PAD_USED: AtomicBool = AtomicBool::new(false);
static KBM_USED: AtomicBool = AtomicBool::new(false);
static SONY: AtomicBool = AtomicBool::new(false);
static SONY_AT: AtomicU64 = AtomicU64::new(0);
static LAST: Mutex<Option<Last>> = Mutex::new(None);
/// The stick's travel that counts as using the controller (past the pads' usual dead zones).
const STICK_USED: f32 = 0.5;
/// The mouse's movement in one read that counts as using it.
const MOUSE_USED: i32 = 3;
/// How often the HID controllers are looked through.
const SONY_EVERY_MS: u64 = 2000;

/// The controller was used this frame (pad.rs: a button, or a stick past [`STICK_USED`]).
pub fn pad_read(buttons_down: bool, left: glam::Vec2, right: glam::Vec2) {
    if buttons_down || left.length() > STICK_USED || right.length() > STICK_USED {
        PAD_USED.store(true, Ordering::Relaxed);
    }
}

/// The keyboard / mouse played a role this frame (remap_er.rs `keyboard`).
pub fn kbm_used() {
    KBM_USED.store(true, Ordering::Relaxed);
}

/// The mouse moved (dinput_mask.rs `take_mouse_motion`).
pub fn mouse_moved((dx, dy): (i32, i32)) {
    if dx.abs() + dy.abs() >= MOUSE_USED {
        KBM_USED.store(true, Ordering::Relaxed);
    }
}

fn now() -> u64 {
    // SAFETY: plain getter.
    unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() }
}

/// A Sony game controller among the system's HID devices.
fn sony_connected() -> bool {
    use windows_sys::Win32::UI::Input::{GetRawInputDeviceInfoW, GetRawInputDeviceList, RAWINPUTDEVICELIST, RIDI_DEVICEINFO, RID_DEVICE_INFO, RIM_TYPEHID};
    let size = std::mem::size_of::<RAWINPUTDEVICELIST>() as u32;
    let mut count = 0u32;
    // SAFETY: a null list asks the count only.
    if unsafe { GetRawInputDeviceList(std::ptr::null_mut(), &mut count, size) } == u32::MAX || count == 0 {
        return false;
    }
    let mut list = vec![RAWINPUTDEVICELIST::default(); count as usize + 4];
    let mut room = list.len() as u32;
    // SAFETY: `list` holds `room` entries of the size given.
    let got = unsafe { GetRawInputDeviceList(list.as_mut_ptr(), &mut room, size) };
    if got == u32::MAX {
        return false;
    }
    list.iter().take(got as usize).filter(|d| d.dwType == RIM_TYPEHID).any(|d| {
        let mut info = RID_DEVICE_INFO { cbSize: std::mem::size_of::<RID_DEVICE_INFO>() as u32, ..RID_DEVICE_INFO::default() };
        let mut bytes = info.cbSize;
        // SAFETY: `info` is a RID_DEVICE_INFO of `bytes` bytes with its cbSize set, as RIDI_DEVICEINFO requires.
        let read = unsafe { GetRawInputDeviceInfoW(d.hDevice, RIDI_DEVICEINFO, (&mut info as *mut RID_DEVICE_INFO).cast(), &mut bytes) };
        if read == u32::MAX || read == 0 || info.dwType != RIM_TYPEHID {
            return false;
        }
        // SAFETY: dwType is RIM_TYPEHID: the union holds the HID member.
        let hid = unsafe { info.Anonymous.hid };
        hid.dwVendorId == 0x054C && hid.usUsagePage == 1 && (hid.usUsage == 4 || hid.usUsage == 5)
    })
}

/// ONCE A FRAME (eroptions.rs `frame`): the state from the setting, the tables and the device used last, published.
pub fn frame(control: &Control) {
    let t = now();
    if t.saturating_sub(SONY_AT.load(Ordering::Relaxed)) >= SONY_EVERY_MS {
        SONY_AT.store(t, Ordering::Relaxed);
        let sony = sony_connected();
        if sony != SONY.swap(sony, Ordering::Relaxed) {
            erlog!("button icons: {} controller connected", if sony { "a PlayStation (Sony)" } else { "no PlayStation" });
        }
    }
    let family = if SONY.load(Ordering::Relaxed) { Family::PlayStation } else { Family::Xbox };
    let (pad_used, kbm_used) = (PAD_USED.swap(false, Ordering::Relaxed), KBM_USED.swap(false, Ordering::Relaxed));
    let Ok(mut guard) = LAST.lock() else { return };
    let last = guard.get_or_insert_with(|| Last { device: family, pad: family });
    last.input(pad_used, family, kbm_used);
    let setting = Setting::parse(control.str(SETTING_KEY));
    let pad = Table::parse(Device::Pad, control.str("remap.pad").unwrap_or(""));
    let kbm = Table::parse(Device::Kbm, control.str("remap.kbm").unwrap_or(""));
    let state = glyph_state(shown(setting, last), &pad, &kbm);
    let family_now = state.family;
    if kh_ui::glyphs::set_state(state) {
        erlog!("button icons: {:?} ({:?}; the controller art {:?}; device used last {:?})", family_now, setting, last.pad, last.device);
    }
}
