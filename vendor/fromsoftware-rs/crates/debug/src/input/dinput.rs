use ilhook::x64::*;
use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
use windows::core::{GUID, s};

use crate::hooks::is_blocked;
use crate::input::InputFlags;

const DIRECTINPUT_VERSION: u32 = 0x0800;

// {BF798031-483A-4DA2-AA99-5D64ED369700}
const IID_IDIRECTINPUT8W: GUID = GUID::from_values(
    0xbf798031,
    0x483a,
    0x4da2,
    [0xaa, 0x99, 0x5d, 0x64, 0xed, 0x36, 0x97, 0x00],
);
// {6F1D2B61-D5A0-11CF-BFC7-444553540000}
const GUID_SYS_KEYBOARD: GUID = GUID::from_values(
    0x6F1D2B61,
    0xD5A0,
    0x11CF,
    [0xBF, 0xC7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00],
);
// {6F1D2B60-D5A0-11CF-BFC7-444553540000}
const GUID_SYS_MOUSE: GUID = GUID::from_values(
    0x6F1D2B60,
    0xD5A0,
    0x11CF,
    [0xBF, 0xC7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00],
);

const VTBL_RELEASE: usize = 2;
const VTBL_CREATE_DEVICE: usize = 3;
const VTBL_GET_DEVICE_STATE: usize = 9;

type RawObj = *mut *const usize;

type DInput8CreateFn =
    unsafe extern "system" fn(usize, u32, *const GUID, *mut RawObj, usize) -> i32;
type CreateDeviceFn = unsafe extern "system" fn(RawObj, *const GUID, *mut RawObj, usize) -> i32;
type ReleaseFn = unsafe extern "system" fn(RawObj) -> u32;

unsafe fn vtable_fn<F: Copy>(obj: RawObj, slot: usize) -> F {
    unsafe { std::mem::transmute_copy(&*(*obj).add(slot)) }
}

unsafe fn with_probe_device(
    di8_create: DInput8CreateFn,
    hinstance: usize,
    guid: &GUID,
    f: impl FnOnce(usize),
) {
    let mut di8: RawObj = std::ptr::null_mut();
    let hr = unsafe {
        di8_create(
            hinstance,
            DIRECTINPUT_VERSION,
            &IID_IDIRECTINPUT8W,
            &mut di8,
            0,
        )
    };
    assert_eq!(hr, 0, "DirectInput8Create failed: {hr:#010x}");

    let create_device: CreateDeviceFn = unsafe { vtable_fn(di8, VTBL_CREATE_DEVICE) };
    let mut device: RawObj = std::ptr::null_mut();
    let hr = unsafe { create_device(di8, guid, &mut device, 0) };
    assert_eq!(
        hr, 0,
        "IDirectInput8::CreateDevice({guid:?}) failed: {hr:#010x}"
    );

    let get_state_addr = unsafe { *(*device).add(VTBL_GET_DEVICE_STATE) as usize };
    f(get_state_addr);

    let release_device: ReleaseFn = unsafe { vtable_fn(device, VTBL_RELEASE) };
    let release_di8: ReleaseFn = unsafe { vtable_fn(di8, VTBL_RELEASE) };
    unsafe { release_device(device) };
    unsafe { release_di8(di8) };
}

// IDirectInputDevice8W::GetDeviceState(cbData: DWORD, lpvData: LPVOID) -> HRESULT
// rcx = this, rdx = cbData, r8 = lpvData
fn make_get_state_closure(
    flags: InputFlags,
) -> impl Fn(*mut ilhook::x64::Registers, usize) -> usize {
    move |reg, original| {
        let size = unsafe { (*reg).rdx };
        let data = unsafe { (*reg).r8 as *mut u8 };

        let original: unsafe extern "system" fn(u64, u64, u64) -> usize =
            unsafe { std::mem::transmute(original) };
        let hr = unsafe { original((*reg).rcx, size, data as u64) };

        if hr == 0 && is_blocked(flags) {
            unsafe { std::ptr::write_bytes(data, 0, size as usize) };
        }
        hr
    }
}

pub(super) unsafe fn install() -> Result<(), ilhook::HookError> {
    let dinput8 = unsafe { GetModuleHandleA(s!("dinput8.dll")).expect("dinput8.dll not loaded") };
    let di8_create: DInput8CreateFn = unsafe {
        std::mem::transmute(
            GetProcAddress(dinput8, s!("DirectInput8Create"))
                .expect("DirectInput8Create not found"),
        )
    };
    let hinstance = unsafe { GetModuleHandleA(None).expect("GetModuleHandle failed").0 as usize };

    let mut keyboard_addr = 0usize;
    let mut mouse_addr = 0usize;

    unsafe {
        with_probe_device(di8_create, hinstance, &GUID_SYS_KEYBOARD, |a| {
            keyboard_addr = a
        })
    };
    unsafe { with_probe_device(di8_create, hinstance, &GUID_SYS_MOUSE, |a| mouse_addr = a) };

    // When both devices share the same GetDeviceState implementation, one hook
    // covers both. When they differ each hook handles its own device type only
    let kb_flags = if keyboard_addr == mouse_addr {
        InputFlags::Keyboard | InputFlags::Mouse
    } else {
        InputFlags::Keyboard
    };

    let kb_hook = unsafe {
        hook_closure_retn(
            keyboard_addr,
            make_get_state_closure(kb_flags),
            CallbackOption::None,
            HookFlags::empty(),
        )?
    };

    let mut hooks = vec![kb_hook];

    if keyboard_addr != mouse_addr {
        let ms_hook = unsafe {
            hook_closure_retn(
                mouse_addr,
                make_get_state_closure(InputFlags::Mouse),
                CallbackOption::None,
                HookFlags::empty(),
            )?
        };
        hooks.push(ms_hook);
    }

    std::mem::forget(hooks);
    Ok(())
}
