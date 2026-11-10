//! (CAMP-OPTIONS, THE USER 2026-11-05: "change the camp's options menu so that they actually reflect Elden Ring's
//! settings, every Elden Ring settings option should be accessible from it, and remove any options that don't do
//! anything. Use tabs like Elden Ring but keep it all looking like KH"; then "our new settings menu should also be the
//! one used when selecting settings from the main menu") THE GAME SIDE OF THE TABBED CONFIG SCREEN
//! (kh-host er_options.rs is the catalogue and the controller; kh-ui camp_options.rs the screen).
//!
//! EVERY ADDRESS BELOW WAS READ IN eldenring.exe 2.7.1.0 AND 2.7.2.0 (offline, read-only; build/sora_tmp/weapons/
//! CAMP-OPTIONS/re-a, re-b, re-c: inventory.json + NOTES.md + patterns.json) and is found by its masked pattern, checked
//! by the PORT offline test (gamever.rs) in both files:
//! - `load_menu_option_data(GameSettings*, MENU_OPTION_DATA*)` 0x25bee0 and `apply_menu_option_data(..)` 0x25c780 (both
//!   versions): what Elden Ring's options screen runs on every change (its OnValueChanged 0x966240 -> the composite's
//!   function 0x93dba0 -> 0x25c780); the MENU_OPTION_DATA constructor 0x86a2f0 / 0x86a7a0 (the game's defaults); the
//!   HDR apply request 0x67b150 / 0x67b600 (GameMan+0xA90 = 1).
//! - CSWindowImp (static 0x458f910 / 0x4592930): GetScreenMode 0xe913f0 / 0xe92ae0, SetScreenMode 0xe925d0 / 0xe93cc0,
//!   GetResolutionIndex 0xe91200 / 0xe928f0, SetResolutionIndex 0xe92530 / 0xe93c20, its resolution list 0xe93910 /
//!   0xe95000 (16-byte entries, width and height first; the list's allocator frees it, vtable slot 13 as 0x78b9cc).
//! - CSPcKeyConfig (static 0x3d61f08 / 0x3d64f28): the Keyboard/Mouse rows' live bytes (+0x880 .. +0x884, defaults
//!   +0x878 ..), the prompt device +0x4 (default +0x0); the ray-tracing support object (0x47f3858 / 0x47f6878: non-null
//!   = Ray Tracing Quality offers Low .. Maximum); the display's HDR manager (0x47f33e0 / 0x47f6400: HDR in force =
//!   byte [[+0x128]+0x13C]).
//! - THE ONE SETTINGS SCREEN: the title menu's SYSTEM job builder 0x9b0820 / 0x9b0cd0 and the in-game main menu's entry
//!   job builder 0x90f150 / 0x90f600 (System = text 101003) are detoured: they hand back the game's own "no job" (a null
//!   job reference, as its refused entries do, 0x90f55f) and our screen opens instead. Each decide calls the builder
//!   twice in a frame (0x790314 / 0x790461): the first opens, the second is ignored. Elden Ring's own screen is reached
//!   only when one of our rows asks for it (Key Bindings, Button Settings, Data Usage Agreement, Quit Game, Return to
//!   Desktop: [`NATIVE_PASS`] lets the next call through, and the pad presses walk to that row). (NO-ER-PAUSE 2026-11-09:
//!   only from the TITLE now; in game the in-game top menu never opens at all, nopause.rs, and Quit Game / Return to Desktop
//!   go the KH way, [`native_route`].)
//!
//! THE SETTINGS REACH THE DISK where the game keeps them: its system data (USER_DATA_10, written by the save start S1
//! 0x67c3c0 with GraphicsConfig.xml after it). Elden Ring's options screen asks no save itself (re-a NOTES 2.2): the
//! game's next save carries them; with `save.points_only` that would be the next save point, so leaving the screen with
//! changes asks THAT ONE SAVE (GameMan+0xB73, the byte the game's own request step answers with S1), let through by
//! savegate's settings pass (never the character's slot).
//!
//! THE KH SIDE FOLLOWS ELDEN RING'S ROWS: the KH camera's speed, axes and auto rotation and the KH sounds' level are
//! published from GameSettings every frame (so a change made anywhere, ours or the game's, reaches them).

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

use glam::Vec2;
use kh_host::er_options::{ErGame, Fmg, Native, OptionData, INVERT_X_KEY, INVERT_Y_KEY, MOUSE_INVERT_X_KEY, MOUSE_INVERT_Y_KEY, OLD_AXES_FLAGS, OPTION_DATA_SIZE};
use windows_sys::Win32::System::SystemInformation::GetTickCount64;

use crate::control::Control;
use crate::erlog;
use crate::havok::{locate, Located};

// ------------------------------------------------------------------------------------------------ the addresses

pub(crate) const LOCATED: [Located; 16] = [
    Located { name: "(CAMP-OPTIONS) load_menu_option_data (GameSettings -> MENU_OPTION_DATA)", pattern: "0f b6 01 88 02 0f b6 41 01 88 42 01 0f b6 41 02 88 42 02 0f b6 41 03 88 42 03 0f b6 41 04 88 42 05 0f b6 41 05 88 42 06", capture: 0, rva_2_7_1_0: 0x0025_bee0, rva_2_7_2_0: 0x0025_bee0 },
    Located { name: "(CAMP-OPTIONS) apply_menu_option_data (MENU_OPTION_DATA -> GameSettings + graphics)", pattern: "40 53 48 83 ec 20 0f b6 02 48 8b da 88 01 0f b6 42 01 88 41 01 0f b6 42 02 88 41 02 0f b6 42 03 88 41 03 0f b6 42 05 88 41 04", capture: 0, rva_2_7_1_0: 0x0025_c780, rva_2_7_2_0: 0x0025_c780 },
    Located { name: "(CAMP-OPTIONS) MENU_OPTION_DATA constructor (the game's defaults)", pattern: "40 53 48 81 ec f0 01 00 00 48 c7 44 24 20 fe ff ff ff 48 8b 05 ? ? ? ? 48 33 c4 48 89 84 24 e0 01 00 00 48 8b d9 48 89 4c 24 28 c7 01 05 0a 05 00 c7 41 04 0a 0a 0a 0a", capture: 0, rva_2_7_1_0: 0x0086_a2f0, rva_2_7_2_0: 0x0086_a7a0 },
    Located { name: "(CAMP-OPTIONS) HDR apply request (GameMan+0xA90 = 1)", pattern: "48 8b 05 $ { ' } c6 80 90 0a 00 00 01 c3", capture: 0, rva_2_7_1_0: 0x0067_b150, rva_2_7_2_0: 0x0067_b600 },
    Located { name: "(CAMP-OPTIONS) CSWindowImp static", pattern: "4c 8b 0d $ { ' } 4d 85 c9 74 55 33 c0 4d 8d 81 f8 00 00 00", capture: 1, rva_2_7_1_0: 0x0458_f910, rva_2_7_2_0: 0x0459_2930 },
    Located { name: "(CAMP-OPTIONS) CSWindowImp GetScreenMode", pattern: "40 53 48 83 ec 20 8b 99 84 00 00 00 48 8b 0d ? ? ? ?", capture: 0, rva_2_7_1_0: 0x00e9_13f0, rva_2_7_2_0: 0x00e9_2ae0 },
    Located { name: "(CAMP-OPTIONS) CSWindowImp SetScreenMode", pattern: "40 53 48 83 ec 20 80 79 48 00 8b da 74 0a 85 d2", capture: 0, rva_2_7_1_0: 0x00e9_25d0, rva_2_7_2_0: 0x00e9_3cc0 },
    Located { name: "(CAMP-OPTIONS) CSWindowImp GetResolutionIndex", pattern: "48 8b c4 57 48 83 ec 60 48 c7 40 b8 fe ff ff ff 48 89 58 08 8b da 48 8b f9", capture: 0, rva_2_7_1_0: 0x00e9_1200, rva_2_7_2_0: 0x00e9_28f0 },
    Located { name: "(CAMP-OPTIONS) CSWindowImp SetResolutionIndex", pattern: "48 89 5c 24 08 57 48 83 ec 20 48 8b d9 8b fa 48 8b 49 38 e8 ? ? ? ?", capture: 0, rva_2_7_1_0: 0x00e9_2530, rva_2_7_2_0: 0x00e9_3c20 },
    Located { name: "(CAMP-OPTIONS) CSWindowImp resolution list", pattern: "48 8b c4 48 89 50 10 55 56 57 41 54 41 55 41 56 41 57 48 83 ec 50", capture: 0, rva_2_7_1_0: 0x00e9_3910, rva_2_7_2_0: 0x00e9_5000 },
    Located { name: "(CAMP-OPTIONS) CSPcKeyConfig static", pattern: "4c 8b 35 $ { ' } 4c 8b e9 4d 85 f6 75 2e 48 8d 0d ? ? ? ?", capture: 1, rva_2_7_1_0: 0x03d6_1f08, rva_2_7_2_0: 0x03d6_4f28 },
    Located { name: "(CAMP-OPTIONS) ray-tracing support static", pattern: "48 8b 0d $ { ' } 48 85 c9 74 05 e8 ? ? ? ? 48 8d 35 ? ? ? ?", capture: 1, rva_2_7_1_0: 0x047f_3858, rva_2_7_2_0: 0x047f_6878 },
    Located { name: "(CAMP-OPTIONS) display HDR manager static", pattern: "48 8b 05 $ { ' } 48 8b 88 28 01 00 00 0f b6 81 3c 01 00 00 38 45 35", capture: 1, rva_2_7_1_0: 0x047f_33e0, rva_2_7_2_0: 0x047f_6400 },
    Located { name: "(CAMP-OPTIONS) title menu SYSTEM job builder", pattern: "40 55 56 57 41 54 41 55 41 56 41 57 48 8d 6c 24 d0 48 81 ec 30 01 00 00 48 c7 45 b8 fe ff ff ff 48 89 9c 24 88 01 00 00 48 8b 05 ? ? ? ? 48 33 c4 48 89 45 20 4d 8b f0 4c 8b e2 48 8b d9 48 89 55 c0 45 33 ed 44 89 6c 24 20", capture: 0, rva_2_7_1_0: 0x009b_0820, rva_2_7_2_0: 0x009b_0cd0 },
    // (CAMP-OPTIONS follow-up) the game's own menu-list cursor setter `select(list, index, bool)` (162 callers; it range-checks,
    // scrolls the list to the row and plays the focus change), on a CommandSelectDialog's list at +0xA38 (cursor +0xD4, rows +0xD0)
    Located { name: "(CAMP-OPTIONS) menu list cursor setter", pattern: "48 89 5c 24 08 48 89 6c 24 10 48 89 74 24 18 48 89 7c 24 20 41 56 48 83 ec 20 44 8b b1 d4 00 00 00", capture: 0, rva_2_7_1_0: 0x0073_9b90, rva_2_7_2_0: 0x0073_a040 },
    Located { name: "(CAMP-OPTIONS) in-game main menu entry job builder", pattern: "48 89 54 24 10 55 56 57 41 54 41 55 41 56 41 57 48 8d ac 24 30 ff ff ff 48 81 ec d0 01 00 00 48 c7 45 40 fe ff ff ff 48 89 9c 24 20 02 00 00 49 8b d8 48 8b fa 48 8b f1 45 33 ed 44 89 6c 24 30 48 8b 01 48 8b 88 c0 00 00 00 48 85 c9 75 06 e8 ? ? ? ? cc 48 8b 01 ff 50 10 84 c0 0f 84 ? ? ? ?", capture: 0, rva_2_7_1_0: 0x0090_f150, rva_2_7_2_0: 0x0090_f600 },
];

const LOAD: usize = 0;
const APPLY: usize = 1;
const CTOR: usize = 2;
const HDR_REQUEST: usize = 3;
const WINDOW: usize = 4;
const GET_MODE: usize = 5;
const SET_MODE: usize = 6;
const GET_RES: usize = 7;
const SET_RES: usize = 8;
const RES_LIST: usize = 9;
const KEY_CONFIG: usize = 10;
const RT: usize = 11;
const HDR_MAN: usize = 12;
const TITLE_SYSTEM: usize = 13;
const INGAME_ENTRY: usize = 15;
const LIST_SELECT: usize = 14;

/// The in-game main menu's System entry (GR_MenuText 101003).
pub const SYSTEM_ENTRY_TEXT: u32 = 101_003;

static ADDRS: std::sync::OnceLock<[Option<usize>; 16]> = std::sync::OnceLock::new();

fn addr(k: usize) -> Option<usize> {
    ADDRS.get_or_init(|| std::array::from_fn(|i| locate(&LOCATED[i])))[k]
}

/// A static's object (the static's address holds the pointer).
fn object(k: usize) -> Option<usize> {
    let at = addr(k)?;
    // SAFETY: a global of the executable found by its pattern; read as a word.
    let p = unsafe { (at as *const usize).read() };
    crate::game::plausible(p).then_some(p)
}

fn game_settings() -> Option<*mut u8> {
    use eldenring::cs::GameDataMan;
    use fromsoftware_shared::FromStatic;
    // SAFETY: game thread; the singleton lives as long as the game; only the settings pointer is taken.
    let man = unsafe { GameDataMan::instance() }.ok()?;
    let p = crate::word!(man.game_settings);
    crate::game::plausible(p).then_some(p as *mut u8)
}

/// The copy the game's load / apply take (0x1A8 bytes used; aligned, with room).
#[repr(C, align(16))]
struct Copy([u8; 0x200]);

// ------------------------------------------------------------------------------------------------ the game

/// THE REAL GAME (game thread only: the HUD frame).
pub struct RealGame;

/// The mod's settings file: its keys as the last frame read them, the path of the file to write, our own writes since.
struct Ini {
    keys: std::collections::HashMap<String, String>,
    path: Option<std::path::PathBuf>,
}
static INI: Mutex<Option<Ini>> = Mutex::new(None);
static IN_GAME: AtomicBool = AtomicBool::new(false);
/// The resolution lists by screen mode, kept for 2 s (each list is the game's allocation).
/// (screen mode, the tick it was read, the list)
type ResolutionCache = Vec<(u8, u64, Vec<(i32, i32)>)>;
static RESOLUTIONS: Mutex<ResolutionCache> = Mutex::new(Vec::new());

impl ErGame for RealGame {
    fn text(&self, fmg: Fmg, id: u32) -> Option<String> {
        if id == 0 {
            return None;
        }
        use eldenring::cs::MsgRepository;
        use fromsoftware_shared::FromStatic;
        // SAFETY: game thread; the repository lives as long as the game.
        let msgs = unsafe { MsgRepository::instance() }.ok()?;
        fmg.categories().iter().find_map(|c| msgs.get_msg(*c, id).map(String::from_utf16_lossy)).map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
    }

    fn load(&mut self) -> Option<OptionData> {
        let (f, gs) = (addr(LOAD)?, game_settings()?);
        let mut copy = Copy([0; 0x200]);
        // SAFETY: the game's own `load_menu_option_data(GameSettings*, MENU_OPTION_DATA*)` (found by pattern), with the live
        // settings and a zeroed copy larger than the 0x1A8 bytes it writes (plain bytes: no constructor needed, re-b NOTES).
        unsafe { std::mem::transmute::<usize, unsafe extern "C" fn(*mut u8, *mut u8)>(f)(gs, copy.0.as_mut_ptr()) };
        let mut d = OptionData::default();
        d.0.copy_from_slice(&copy.0[..OPTION_DATA_SIZE]);
        Some(d)
    }

    fn apply(&mut self, data: &OptionData) -> bool {
        let (Some(f), Some(gs)) = (addr(APPLY), game_settings()) else { return false };
        let mut copy = Copy([0; 0x200]);
        copy.0[..OPTION_DATA_SIZE].copy_from_slice(&data.0);
        // SAFETY: the game's own `apply_menu_option_data(GameSettings*, const MENU_OPTION_DATA*)`, as its options screen
        // calls it on every change, with a copy its own load filled just before.
        unsafe { std::mem::transmute::<usize, unsafe extern "C" fn(*mut u8, *const u8)>(f)(gs, copy.0.as_ptr()) };
        true
    }

    fn defaults(&mut self) -> Option<OptionData> {
        let f = addr(CTOR)?;
        game_settings()?;
        let mut copy = Copy([0; 0x200]);
        // SAFETY: the game's MENU_OPTION_DATA constructor on a zeroed buffer of the size its callers give it (0x1B0+);
        // it builds the defaults (GameSettings ctor + reset, the graphics block's defaults) and an empty string at +0x28.
        unsafe { std::mem::transmute::<usize, unsafe extern "C" fn(*mut u8) -> *mut u8>(f)(copy.0.as_mut_ptr()) };
        let mut d = OptionData::default();
        d.0.copy_from_slice(&copy.0[..OPTION_DATA_SIZE]);
        Some(d)
    }

    fn hdr_changed(&mut self) {
        if let Some(f) = addr(HDR_REQUEST) {
            // SAFETY: the game's own HDR apply request (one byte of GameMan), no arguments.
            unsafe { std::mem::transmute::<usize, unsafe extern "C" fn()>(f)() };
        }
    }

    fn hdr_active(&self) -> bool {
        let Some(man) = object(HDR_MAN) else { return false };
        // SAFETY: the display manager's sub-object pointer at +0x128 and its HDR byte at +0x13C (re-b NOTES), each checked.
        unsafe {
            let sub = ((man + 0x128) as *const usize).read();
            crate::game::plausible(sub) && ((sub + 0x13C) as *const u8).read() != 0
        }
    }

    fn rt_supported(&self) -> bool {
        object(RT).is_some()
    }

    fn key_config(&self, at: u16) -> Option<u8> {
        let cfg = object(KEY_CONFIG)?;
        // SAFETY: a byte of CSPcKeyConfig (its rows' offsets, re-b NOTES; the object is 0x890+ bytes).
        Some(unsafe { ((cfg + at as usize) as *const u8).read() })
    }

    fn set_key_config(&mut self, at: u16, value: u8) -> bool {
        let Some(cfg) = object(KEY_CONFIG) else { return false };
        // SAFETY: as `key_config`: the game's own rows write these live bytes the same way.
        unsafe { ((cfg + at as usize) as *mut u8).write(value) };
        true
    }

    fn screen_mode(&self) -> Option<u8> {
        let (f, w) = (addr(GET_MODE)?, object(WINDOW)?);
        // SAFETY: CSWindowImp's own getter on the window object.
        Some(unsafe { std::mem::transmute::<usize, unsafe extern "C" fn(usize) -> u32>(f)(w) }.min(2) as u8)
    }

    fn set_screen_mode(&mut self, mode: u8) -> bool {
        let (Some(f), Some(w)) = (addr(SET_MODE), object(WINDOW)) else { return false };
        // SAFETY: CSWindowImp's own setter (the pending mode; its Update applies it), as the Screen Mode row calls it.
        unsafe { std::mem::transmute::<usize, unsafe extern "C" fn(usize, u32)>(f)(w, u32::from(mode.min(2))) };
        true
    }

    fn resolutions(&mut self, mode: u8) -> Vec<(i32, i32)> {
        // SAFETY: plain getter.
        let now = unsafe { GetTickCount64() };
        if let Ok(cache) = RESOLUTIONS.lock() {
            if let Some((_, _, list)) = cache.iter().find(|(m, at, _)| *m == mode && now.saturating_sub(*at) < 2000) {
                return list.clone();
            }
        }
        let (Some(f), Some(w)) = (addr(RES_LIST), object(WINDOW)) else { return Vec::new() };
        let mut out = [0usize; 4];
        // SAFETY: CSWindowImp's own list builder `(this, out, mode, true)` (as 0x78bb90 calls it): it constructs `out` (its
        // allocator, begin, end, capacity); the entries are 16 bytes, width and height first (0xe99c20).
        unsafe { std::mem::transmute::<usize, unsafe extern "C" fn(usize, *mut usize, u32, u8) -> *mut usize>(f)(w, out.as_mut_ptr(), u32::from(mode), 1) };
        let (alloc, begin, end) = (out[0], out[1], out[2]);
        let mut list = Vec::new();
        if begin != 0 && end > begin && (end - begin) % 16 == 0 && (end - begin) / 16 < 512 {
            for k in 0..(end - begin) / 16 {
                // SAFETY: inside the list the game just built.
                let (wd, ht) = unsafe { (((begin + 16 * k) as *const i32).read(), ((begin + 16 * k + 4) as *const i32).read()) };
                list.push((wd, ht));
            }
        }
        if begin != 0 && crate::game::plausible(alloc) {
            // SAFETY: the list's own allocator frees its block, as the game's 0x78b9cc does (vtable slot 13).
            unsafe {
                let vt = (alloc as *const usize).read();
                if crate::game::plausible(vt) {
                    let free = ((vt + 0x68) as *const usize).read();
                    std::mem::transmute::<usize, unsafe extern "C" fn(usize, usize)>(free)(alloc, begin);
                }
            }
        }
        if let Ok(mut cache) = RESOLUTIONS.lock() {
            cache.retain(|(m, _, _)| *m != mode);
            cache.push((mode, now, list.clone()));
        }
        list
    }

    fn resolution(&self, mode: u8) -> Option<u8> {
        let (f, w) = (addr(GET_RES)?, object(WINDOW)?);
        // SAFETY: CSWindowImp's own getter `(this, mode)`.
        Some(unsafe { std::mem::transmute::<usize, unsafe extern "C" fn(usize, u32) -> u32>(f)(w, u32::from(mode)) }.min(255) as u8)
    }

    fn set_resolution(&mut self, index: u8) -> bool {
        let (Some(f), Some(w)) = (addr(SET_RES), object(WINDOW)) else { return false };
        // SAFETY: CSWindowImp's own setter for the pending mode, as the Resolution row calls it.
        unsafe { std::mem::transmute::<usize, unsafe extern "C" fn(usize, u32)>(f)(w, u32::from(index)) };
        true
    }

    fn open_native(&mut self, screen: Native) -> bool {
        let in_game = IN_GAME.load(Ordering::Relaxed);
        match native_route(screen, in_game) {
            // (NO-ER-PAUSE) in game: the KH way out, never Elden Ring's System menu (its top menu is refused, nopause.rs)
            NativeRoute::Title => crate::savegate::quit_to_title("the settings' Quit Game"),
            NativeRoute::TitleThenDesktop => {
                let asked = crate::savegate::quit_to_title("the settings' Return to Desktop");
                if asked {
                    DESKTOP_AT.store(now(), Ordering::Relaxed);
                    TITLE_SEEN_AT.store(0, Ordering::Relaxed);
                    erlog!("options: Return to Desktop: the game's title screen first, then its window is closed the way its own close button closes it");
                }
                asked
            }
            NativeRoute::Refused => {
                erlog!("options: Elden Ring's own {screen:?} screen is not opened in game (NO-ER-PAUSE: its System menu would show)");
                false
            }
            // the title: its own SYSTEM list is walked (the title is not the pause menu)
            NativeRoute::TitleSystem => {
                if !crate::inputs::er_reads_pad() {
                    return false;
                }
                if let Ok(mut n) = NATIVE.lock() {
                    *n = Some(NativeRun { screen, step: 0, next_at: 0 });
                }
                true
            }
        }
    }

    fn request_settings_save(&mut self) -> bool {
        crate::savegate::allow_settings_save("the options screen was left with changes", !IN_GAME.load(Ordering::Relaxed))
    }

    fn log(&mut self, line: &str) {
        erlog!("{line}");
    }

    fn ini(&self, key: &str) -> Option<String> {
        INI.lock().ok()?.as_ref()?.keys.get(key).cloned()
    }

    fn set_ini(&mut self, key: &str, value: &str) -> bool {
        let Ok(mut guard) = INI.lock() else { return false };
        let Some(ini) = guard.as_mut() else { return false };
        let Some(path) = ini.path.clone() else { return false };
        let text = crate::control::read_text(&path).unwrap_or_default();
        let new = set_line(&text, key, value);
        match std::fs::write(&path, new) {
            Ok(()) => {
                ini.keys.insert(key.to_string(), value.to_string());
                erlog!("options: {key} = {value} written into {} (read again by the running mod)", path.display());
                true
            }
            Err(e) => {
                erlog!("options: {key} could not be written into {} ({e})", path.display());
                false
            }
        }
    }

    fn in_game(&self) -> bool {
        IN_GAME.load(Ordering::Relaxed)
    }

    fn capture_begin(&mut self, device: kh_host::remap::Device) {
        crate::remap_er::capture_begin(device);
    }

    fn capture_poll(&mut self, device: kh_host::remap::Device) -> Option<Option<u16>> {
        crate::remap_er::capture_poll(device)
    }

    fn wrote(&mut self, key: u32, value: u8) {
        match key {
            // (Camera / Mouse X / Y-Axis: the rows write the KH camera's own per-axis values themselves, kh-host er_options.rs
            // `Store::KhAxis`)
            // Display > Show Tutorials: the mod holds the game's option Off unless `tutorials.er = 1` (tutorial.rs): the row
            // is the user's choice of that too
            304 => {
                self.set_ini("tutorials.er", if value != 0 { "1" } else { "0" });
            }
            // Display > HUD: the mod holds an Off at On while Sora's HUD is up (hud.rs, part-by-part mode) unless the user
            // chose Off here (`hud.er.user_off`)
            303 => {
                self.set_ini("hud.er.user_off", if value == 0 { "1" } else { "0" });
            }
            _ => {}
        }
    }
}

/// THE SETTINGS FILE'S LINE OF `key` set to `value` (pure): the first line of that key is replaced (its inline comment
/// kept), a later duplicate dropped, or the line appended; every other line as it was.
pub fn set_line(text: &str, key: &str, value: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut done = false;
    for line in text.lines() {
        let t = line.trim_start();
        let is_key = !t.starts_with('#') && !t.starts_with(';') && t.split_once('=').is_some_and(|(k, _)| k.trim() == key);
        if is_key {
            if done {
                continue;
            }
            done = true;
            let comment = t.split_once('=').map(|(_, v)| v).and_then(|v| v.find(" #").or_else(|| v.find(" ;")).map(|i| v[i..].to_string())).unwrap_or_default();
            out.push(format!("{key} = {value}{comment}"));
        } else {
            out.push(line.to_string());
        }
    }
    if !done {
        out.push(format!("{key} = {value}"));
    }
    let mut s = out.join("\n");
    s.push('\n');
    s
}

/// (CAMP-OPTIONS) THE SETTINGS FILE WITHOUT `key` (pure): every line of that key dropped (comments and other lines as they were).
pub fn remove_line(text: &str, key: &str) -> String {
    let kept: Vec<&str> = text.lines().filter(|line| {
        let t = line.trim_start();
        t.starts_with('#') || t.starts_with(';') || !t.split_once('=').is_some_and(|(k, _)| k.trim() == key)
    }).collect();
    let mut s = kept.join("\n");
    s.push('\n');
    s
}

/// (CAMP-OPTIONS, THE USER 2026-11-05: "make sure the default setting for the camera is NOT reversed") the older single axis
/// flags are dropped from the settings file that holds them (once a session; they are ignored either way).
fn drop_old_axes_flags(control: &Control) {
    static DONE: AtomicBool = AtomicBool::new(false);
    if DONE.swap(true, Ordering::Relaxed) {
        return;
    }
    let path = control.source().to_path_buf();
    let Some(text) = crate::control::read_text(&path) else { return };
    let mut new = text.clone();
    for key in OLD_AXES_FLAGS {
        new = remove_line(&new, key);
    }
    if new.trim_end() != text.trim_end() {
        match std::fs::write(&path, new) {
            Ok(()) => erlog!("options: the older camera axis flags ({}) dropped from {}: each axis is its own value now (absent = Normal)", OLD_AXES_FLAGS.join(", "), path.display()),
            Err(e) => erlog!("options: the older camera axis flags could not be dropped from {} ({e}); they are ignored", path.display()),
        }
    }
}

// ------------------------------------------------------------------------------------------------ the hooks

/// Elden Ring's own options screen is let through once (one of our rows asked for it), until this tick.
static NATIVE_PASS: AtomicU64 = AtomicU64::new(0);
/// Our screen was asked for by the title's SYSTEM / the in-game System entry.
static TITLE_WANTED: AtomicBool = AtomicBool::new(false);
static INGAME_WANTED: AtomicBool = AtomicBool::new(false);
/// The tick of the last open (the decide's second call within the same frame is ignored).
static OPENED_AT: AtomicU64 = AtomicU64::new(0);
static ORIGINAL_TITLE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static ORIGINAL_INGAME: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static HOOKS_ON: AtomicBool = AtomicBool::new(true);
static CALLS: AtomicU32 = AtomicU32::new(0);

fn now() -> u64 {
    // SAFETY: plain getter.
    unsafe { GetTickCount64() }
}

fn passing() -> bool {
    now() < NATIVE_PASS.load(Ordering::Relaxed)
}

type JobFn = unsafe extern "C" fn(usize, *mut usize, usize) -> *mut usize;

/// The title's SYSTEM: our screen (the game's own "no job"), unless one of our rows asked for the game's.
unsafe extern "C" fn title_system_hook(owner: usize, out: *mut usize, cs: usize) -> *mut usize {
    CALLS.fetch_add(1, Ordering::Relaxed);
    let original = ORIGINAL_TITLE.load(Ordering::Relaxed);
    if passing() || !HOOKS_ON.load(Ordering::Relaxed) {
        NATIVE_PASS.store(0, Ordering::Relaxed);
        // SAFETY: the original builder with its own arguments.
        return std::mem::transmute::<usize, JobFn>(original)(owner, out, cs);
    }
    if now().saturating_sub(OPENED_AT.load(Ordering::Relaxed)) > 400 {
        OPENED_AT.store(now(), Ordering::Relaxed);
        TITLE_WANTED.store(true, Ordering::Relaxed);
        remember_title_list(cs);
    }
    // the game's own null job reference: its menu stays as it is (re-c NOTES 2)
    *out = 0;
    out
}

/// (CAMP-OPTIONS follow-up, "after our screen closes on the title, ER moves its cursor back to CONTINUE") THE TITLE LIST AS IT
/// WAS WHEN SYSTEM WAS CHOSEN: the CommandSelectDialog (the decide's ComponentStack is its +0x50, re-c NOTES 1), its vtable,
/// its row count and its cursor (the SYSTEM row). Seen in game (2026-11-05): the game DESTROYS that list while our screen is
/// up (its first word reads 0 at our leave) and makes the title's list anew, cursor on CONTINUE. So the new list is caught
/// where every list of that class passes each frame: its update (vtable slot 2, 0x78f040 / 0x78f4f0: `add rcx, 0xA38`), that
/// one slot replaced in the class's vtable at SYSTEM's decide (put back at the payload's stop).
static TITLE_LIST: Mutex<Option<TitleList>> = Mutex::new(None);

#[derive(Clone, Copy, Debug)]
struct TitleList {
    dialog: usize,
    vtable: usize,
    rows: u32,
    cursor: u32,
}

/// The list object inside a CommandSelectDialog (its update 0x78f040 / 0x78f4f0: `add rcx, 0xA38`).
const DIALOG_LIST: usize = 0xA38;
/// The list's row count and cursor (0x73ca60: range check against +0xD0; the getter 0x73ac70 reads +0xD4).
const LIST_ROWS: usize = 0xD0;
const LIST_CURSOR: usize = 0xD4;
/// The dialog's per-frame update in its vtable (0x78f040 is slot 2 of 0x2aa5020).
const UPDATE_SLOT: usize = 2;

/// The patched vtable and the slot's own function (0: none).
static UPDATE_VTABLE: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_UPDATE: AtomicUsize = AtomicUsize::new(0);
/// The cursor is to be put back (set when our screen leaves, until this tick; 0: not).
static RESTORE_UNTIL: AtomicU64 = AtomicU64::new(0);

fn read_list(dialog: usize) -> Option<TitleList> {
    if !crate::game::plausible(dialog) {
        return None;
    }
    // SAFETY: a live dialog (the game's decide just ran on it, or its own update is running), read as words.
    unsafe {
        let vtable = (dialog as *const usize).read();
        let rows = ((dialog + DIALOG_LIST + LIST_ROWS) as *const u32).read();
        let cursor = ((dialog + DIALOG_LIST + LIST_CURSOR) as *const u32).read();
        (crate::game::plausible(vtable) && rows > 0 && rows < 64 && cursor < rows).then_some(TitleList { dialog, vtable, rows, cursor })
    }
}

fn remember_title_list(cs: usize) {
    let list = cs.checked_sub(0x50).and_then(read_list);
    if let Ok(mut l) = TITLE_LIST.lock() {
        *l = list;
    }
    match list {
        Some(l) => {
            erlog!("options: the title's list: {} rows, SYSTEM on row {} (put back there when our screen closes)", l.rows, l.cursor);
            patch_update(l.vtable);
        }
        None => erlog!("options: the title's list was not readable at SYSTEM's decide: its cursor is left to the game"),
    }
}

/// The list class's update slot replaced by ours (once; the same class every time).
fn patch_update(vtable: usize) {
    use windows_sys::Win32::System::Memory::{VirtualProtect, PAGE_READWRITE};
    if UPDATE_VTABLE.load(Ordering::SeqCst) != 0 {
        return;
    }
    let slot = (vtable + UPDATE_SLOT * 8) as *mut usize;
    // SAFETY: the class's vtable in the executable (read-only data): its one slot written with the page made writable for
    // that moment; the slot's own function kept and called by ours.
    unsafe {
        let mut old = 0u32;
        if VirtualProtect(slot.cast(), 8, PAGE_READWRITE, &mut old) == 0 {
            erlog!("options: the title list's update could not be watched: its cursor is left to the game");
            return;
        }
        ORIGINAL_UPDATE.store(slot.read(), Ordering::SeqCst);
        slot.write(list_update_hook as *const () as usize);
        VirtualProtect(slot.cast(), 8, old, &mut old);
    }
    UPDATE_VTABLE.store(vtable, Ordering::SeqCst);
}

/// The payload stops: the list class's own update back.
fn unpatch_update() {
    use windows_sys::Win32::System::Memory::{VirtualProtect, PAGE_READWRITE};
    let vtable = UPDATE_VTABLE.swap(0, Ordering::SeqCst);
    if vtable == 0 {
        return;
    }
    let slot = (vtable + UPDATE_SLOT * 8) as *mut usize;
    // SAFETY: the slot we wrote, put back with the page made writable for that moment.
    unsafe {
        let mut old = 0u32;
        if VirtualProtect(slot.cast(), 8, PAGE_READWRITE, &mut old) != 0 {
            slot.write(ORIGINAL_UPDATE.load(Ordering::SeqCst));
            VirtualProtect(slot.cast(), 8, old, &mut old);
        }
    }
}

/// EVERY LIST OF THAT CLASS, EACH FRAME: the game's own update, then, while the cursor is to be put back, the title's list
/// (the same class and row count as SYSTEM's list) with another cursor is put on SYSTEM by the game's own setter, once.
unsafe extern "system" fn list_update_hook(this: usize, dt: f32, arg: usize) -> usize {
    // (rax passed on as the game's own update left it)
    let original: unsafe extern "system" fn(usize, f32, usize) -> usize = std::mem::transmute(ORIGINAL_UPDATE.load(Ordering::Relaxed));
    let result = original(this, dt, arg);
    after_list_update(this);
    result
}

unsafe fn after_list_update(this: usize) {
    let until = RESTORE_UNTIL.load(Ordering::Relaxed);
    if until == 0 || TITLE_UP.load(Ordering::Relaxed) {
        return;
    }
    if now() >= until {
        RESTORE_UNTIL.store(0, Ordering::Relaxed);
        return;
    }
    let Some(was) = TITLE_LIST.lock().ok().and_then(|l| *l) else { return };
    let Some(list) = read_list(this).filter(|l| l.vtable == was.vtable && l.rows == was.rows) else { return };
    RESTORE_UNTIL.store(0, Ordering::Relaxed);
    if list.cursor == was.cursor {
        erlog!("options: the title's cursor is on SYSTEM (row {})", was.cursor);
        return;
    }
    let Some(select) = addr(LIST_SELECT) else { return };
    // SAFETY: the game's own list cursor setter on the list whose update just ran (alive), with a row inside it (checked
    // against its own row count); `false`: as most of its callers pass.
    let moved = std::mem::transmute::<usize, unsafe extern "C" fn(usize, u32, u8) -> u8>(select)(this + DIALOG_LIST, was.cursor, 0);
    erlog!("options: the title's list {} (its cursor on row {}): put back on SYSTEM (row {}) by the game's own setter ({})", if this == was.dialog { "is the same" } else { "was made anew" }, list.cursor, was.cursor, if moved != 0 { "done" } else { "refused" });
}

/// The in-game main menu's entries: System = our screen; the six others are the game's.
unsafe extern "C" fn ingame_entry_hook(cap: usize, out: *mut usize, dlg: usize) -> *mut usize {
    let original = ORIGINAL_INGAME.load(Ordering::Relaxed);
    // `*(u32*)*cap` = the entry's text id (re-c NOTES 3)
    let entry = if cap != 0 { (cap as *const usize).read() } else { 0 };
    let id = if crate::game::plausible(entry) { (entry as *const u32).read() } else { 0 };
    if id != SYSTEM_ENTRY_TEXT || passing() || !HOOKS_ON.load(Ordering::Relaxed) {
        if id == SYSTEM_ENTRY_TEXT {
            NATIVE_PASS.store(0, Ordering::Relaxed);
        }
        // SAFETY: the original builder with its own arguments.
        return std::mem::transmute::<usize, JobFn>(original)(cap, out, dlg);
    }
    CALLS.fetch_add(1, Ordering::Relaxed);
    if now().saturating_sub(OPENED_AT.load(Ordering::Relaxed)) > 400 {
        OPENED_AT.store(now(), Ordering::Relaxed);
        INGAME_WANTED.store(true, Ordering::Relaxed);
    }
    *out = 0;
    out
}

/// Installs the two detours (entry.rs, with the others; undone with the renderer's hooks).
pub fn install() {
    for (k, hook, original) in [(TITLE_SYSTEM, title_system_hook as *const () as usize, &ORIGINAL_TITLE), (INGAME_ENTRY, ingame_entry_hook as *const () as usize, &ORIGINAL_INGAME)] {
        match addr(k) {
            // SAFETY: a function's start found by pattern at the recorded address; the hook passes its three arguments on.
            Some(f) => match unsafe { kh_er_render::inline_hook(f, hook, original, LOCATED[k].name) } {
                Ok(()) => erlog!("options: {} detoured: our settings screen opens there", LOCATED[k].name),
                Err(e) => {
                    original.store(0, Ordering::SeqCst);
                    erlog!("options: {} NOT detoured ({e}): Elden Ring's own options screen opens there", LOCATED[k].name);
                }
            },
            None => erlog!("options: {} not found: Elden Ring's own options screen opens there", LOCATED[k].name),
        }
    }
}

pub fn uninstalled() {
    unpatch_update(); // (the title list's update slot)
    ORIGINAL_TITLE.store(0, Ordering::SeqCst);
    ORIGINAL_INGAME.store(0, Ordering::SeqCst);
}

// ------------------------------------------------------------------------------------------------ Elden Ring's own screens

/// (NO-ER-PAUSE, THE USER 2026-11-09: "Elden Ring pause menu should be impossible to ever show up") HOW A ROW THAT ASKS FOR ONE OF
/// ELDEN RING'S OWN SCREENS IS SERVED. In game its System menu is never walked any more (it is a page of the pause menu): Quit
/// Game is the game's own return-to-title request (savegate.rs `quit_to_title`, what the camp menu's Title Screen asks: with
/// `save.points_only` nothing is saved on the way, as its own Quit Game under the gate saved nothing), Return to Desktop the same,
/// then the game's window closed on the title (its window procedure's own WM_CLOSE: its close button / Alt+F4); the other screens'
/// rows are not offered in game. On the title its SYSTEM list is walked as before (the title is not the pause menu).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeRoute {
    Title,
    TitleThenDesktop,
    TitleSystem,
    Refused,
}

/// The route of a row (pure).
pub fn native_route(screen: Native, in_game: bool) -> NativeRoute {
    match (screen, in_game) {
        (Native::QuitGame, true) => NativeRoute::Title,
        (Native::Desktop, true) => NativeRoute::TitleThenDesktop,
        (_, true) => NativeRoute::Refused,
        (_, false) => NativeRoute::TitleSystem,
    }
}

/// Return to Desktop was confirmed at this tick (0: not); the title screen first seen at this tick (0: not yet).
static DESKTOP_AT: AtomicU64 = AtomicU64::new(0);
static TITLE_SEEN_AT: AtomicU64 = AtomicU64::new(0);
/// The title must come within this long; the window is closed this long after it came (its own fade-in settled).
const DESKTOP_WAIT_MS: u64 = 90_000;
const DESKTOP_AFTER_TITLE_MS: u64 = 1_500;

/// (NO-ER-PAUSE) The step of Return to Desktop (pure): (asked at, title seen at, the title is up, now) -> (close the window now,
/// the title-seen tick after, the request still stands).
pub fn desktop_step(asked: u64, seen: u64, title: bool, t: u64) -> (bool, u64, bool) {
    if asked == 0 {
        return (false, 0, false);
    }
    if seen == 0 {
        if title {
            return (false, t, true);
        }
        return (false, 0, t.saturating_sub(asked) <= DESKTOP_WAIT_MS);
    }
    if t.saturating_sub(seen) >= DESKTOP_AFTER_TITLE_MS {
        return (true, seen, false);
    }
    (false, seen, true)
}

/// Once a frame (FRAME_BEGIN): Return to Desktop, on the title.
fn desktop_frame() {
    let asked = DESKTOP_AT.load(Ordering::Relaxed);
    if asked == 0 {
        return;
    }
    let title = crate::game::player_age() == 0 && crate::inputs::ui_states().is_some_and(|s| s[0] & 2 != 0);
    let (close, seen, stands) = desktop_step(asked, TITLE_SEEN_AT.load(Ordering::Relaxed), title, now());
    TITLE_SEEN_AT.store(seen, Ordering::Relaxed);
    if close {
        let posted = crate::remap_er::close_game_window();
        erlog!("options: Return to Desktop: the title is up: {}", if posted { "the game's window is asked to close (WM_CLOSE, as its own close button)" } else { "its window is NOT known: the title's QUIT GAME is the way out" });
    } else if !stands {
        erlog!("options: Return to Desktop: the title did not come in {} s: the request is dropped", DESKTOP_WAIT_MS / 1000);
    }
    if !stands {
        DESKTOP_AT.store(0, Ordering::Relaxed);
        TITLE_SEEN_AT.store(0, Ordering::Relaxed);
    }
}

/// A walk to one of Elden Ring's own screens from the TITLE: the presses its System menu needs (never in game: NO-ER-PAUSE).
#[derive(Clone, Copy, Debug)]
struct NativeRun {
    screen: Native,
    step: usize,
    next_at: u64,
}
static NATIVE: Mutex<Option<NativeRun>> = Mutex::new(None);

const XI_DOWN: u16 = 0x0002;
const XI_RB: u16 = 0x0200;

/// THE PRESSES (pure): from the title's list as it is when our screen has closed (on SYSTEM, where it stayed) to the row: A (the
/// System page), RB to the tab, Down to the row, A. Each (buttons, keyboard scan code, wait before it in ms). (NO-ER-PAUSE: the
/// in-game walk, Start / Esc, Up, A into the pause menu's System page, is gone.)
pub fn native_presses(screen: Native) -> Vec<(u16, u16, u64)> {
    let (tabs, rows) = screen.place();
    let mut p: Vec<(u16, u16, u64)> = vec![(crate::inputs::XI_A, crate::inputs::SCAN_E, 600)];
    // the options screen opens (its own fade) before its tabs take a press
    let first = 1400;
    for k in 0..tabs {
        p.push((XI_RB, 0, if k == 0 { first } else { 350 }));
    }
    for k in 0..rows {
        p.push((XI_DOWN, 0, if k == 0 && tabs == 0 { first } else { 300 }));
    }
    p.push((crate::inputs::XI_A, crate::inputs::SCAN_E, 450));
    p
}

/// One step of a walk (FRAME_BEGIN). The game's own screen is let through for the next 12 s.
fn native_frame(our_menu_up: bool) {
    let Ok(mut guard) = NATIVE.lock() else { return };
    let Some(run) = guard.as_mut() else { return };
    if our_menu_up {
        return;
    }
    let t = now();
    if run.step == 0 && run.next_at == 0 {
        NATIVE_PASS.store(t + 12_000, Ordering::Relaxed);
        run.next_at = t + 500;
        erlog!("options: Elden Ring's own {:?} screen: the title's System menu is walked to it", run.screen);
    }
    if t < run.next_at {
        return;
    }
    let presses = native_presses(run.screen);
    match presses.get(run.step) {
        Some((buttons, scan, _)) => {
            let how = crate::inputs::press_for_er(*buttons, *scan).unwrap_or("NOT PRESSED");
            run.step += 1;
            run.next_at = t + presses.get(run.step).map_or(0, |p| p.2);
            if run.step == 1 {
                erlog!("options: press {:#06x} for Elden Ring ({how})", buttons);
            }
        }
        None => {
            erlog!("options: Elden Ring's own {:?} screen reached by {} presses", run.screen, presses.len());
            *guard = None;
        }
    }
}

// ------------------------------------------------------------------------------------------------ the frames

/// THE TABBED SCREEN IS ON (`config.tabs`, default on; 0 = KH3's Config rows driving Elden Ring's as IMPL_ER21 made them).
pub fn on(control: &Control) -> bool {
    control.flag("config.tabs") != Some(false)
}

/// ONCE A FRAME (FRAME_BEGIN, title and game alike): the settings file's keys for the Style row, the hooks' switch, the
/// walks to Elden Ring's own screens, the KH side's holds that follow the user's rows.
pub fn frame(control: &Control, our_menu_up: bool) {
    let on = on(control);
    HOOKS_ON.store(on, Ordering::Relaxed);
    if let Ok(mut ini) = INI.lock() {
        let keys = control.pairs().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let path = Some(control.source().to_path_buf());
        *ini = Some(Ini { keys, path });
    }
    native_frame(our_menu_up);
    desktop_frame(); // (NO-ER-PAUSE) Return to Desktop, on the title
    drop_old_axes_flags(control); // (CAMP-OPTIONS)
    // (no in-game HUD frame for half a second and our title screen not up: the title, a load: the game's keys are its own)
    // (TITLE-AUDIO) ... nor KH3's Load screen / delete list on the title (titleload.rs `keys_ours` masks the game's keyboard
    // under it: its keys drove the game's list waiting below)
    // (CC-MUTE) ... nor while Elden Ring's creation is pressed for the player (ngquiet.rs holds every key and button from him)
    if !TITLE_UP.load(Ordering::Relaxed) && !crate::hud::title_mode_on() && !crate::ngquiet::input_held() && now().saturating_sub(LAST_INGAME.load(Ordering::Relaxed)) > 500 {
        crate::remap_er::set_swallow(0);
    }
    // (CAMP-OPTIONS) the button tables of the settings file, at the input layer (remap_er.rs)
    crate::remap_er::set(control.str("remap.pad").unwrap_or(""), control.str("remap.kbm").unwrap_or(""), control.on("in.keys_legacy"));
    // (BUTTON-ICONS) the UI's button icons: the Button Icons setting, the tables, the device used last (button_icons_er.rs)
    crate::button_icons_er::frame(control);
    // (Display > HUD) the user's Off is the game's own Off while Sora's HUD is up (hud.rs)
    crate::hud::set_er_hud_user_off(control.on("hud.er.user_off"));
    let calls = CALLS.load(Ordering::Relaxed);
    if calls != LOGGED_CALLS.swap(calls, Ordering::Relaxed) {
        erlog!("options: our settings screen asked for by the game's own menus {calls} time(s) so far (tabs {})", if on { "on" } else { "off" });
    }
}

static LOGGED_CALLS: AtomicU32 = AtomicU32::new(0);

/// The host of the tabbed screen into a HUD (idempotent; off = KH3's rows).
pub fn install_host(hud: &mut kh_host::hud::Hud, control: &Control, in_game: bool) {
    IN_GAME.store(in_game, Ordering::Relaxed);
    if on(control) {
        if hud.options_host.is_none() {
            hud.options_host = Some(Box::new(kh_host::er_options::Controller::new(RealGame)));
            erlog!("options: the camp menu's Config is ELDEN RING'S OPTIONS SCREEN in KH3's look (tabs, the game's own texts and code; config.tabs = 0: KH3's rows)");
        }
    } else {
        hud.options_host = None;
    }
}

/// (THE USER 2026-11-05: "our new settings menu should also be the one used when selecting settings from the main menu")
/// The title's SYSTEM was chosen since the last call (once).
pub fn take_title_wanted() -> bool {
    TITLE_WANTED.swap(false, Ordering::Relaxed)
}

/// The in-game main menu's System was chosen (once).
pub fn take_ingame_wanted() -> bool {
    INGAME_WANTED.swap(false, Ordering::Relaxed)
}

/// THE KH SIDE OF ELDEN RING'S ROWS, once a HUD frame in game: the KH camera (speed, axes, auto rotation) and the KH
/// sounds' level follow GameSettings, merged over the KH block's own rows (erconfig's readers take the merged block).
pub fn publish(kh: &kh_core::config::UserConfig) {
    use kh_core::config::ConfigOption as O;
    let mut merged = *kh;
    if let Some(gs) = game_settings() {
        // SAFETY: bytes of the live settings object (read as bytes).
        let b = |o: usize| unsafe { gs.add(o).read() };
        let speed = b(0x00).min(10);
        merged.set(O::VerticalCameraSensitivity, crate::erconfig::sensitivity_of(speed));
        merged.set(O::HorizontalCameraSensitivity, crate::erconfig::sensitivity_of(speed));
        // (THE USER 2026-11-05: "make sure the default setting for the camera is NOT reversed") the KH camera's flips are its own
        // per-axis values (`camera.invert_x` / `camera.invert_y`, absent = Normal), never the game's stored bytes
        let (flip_h, flip_v) = kh_flips(INVERT_X_KEY, INVERT_Y_KEY);
        merged.set(O::HorizontalCameraControl, u8::from(flip_h));
        merged.set(O::VerticalCameraControl, u8::from(flip_v));
        // Camera Auto Rotation On = KH3's Camera Work Auto (choice 0)
        merged.set(O::CameraWork, u8::from(b(0x2F) == 0));
        merged.set(O::MasterVolume, b(0x20).min(10));
        // (TITLE-AUDIO) the Sound rows are read live by fx.rs `apply_levels` (Sound Effects no longer scales every KH sound)
    }
    crate::erconfig::publish(&merged);
}

/// (CAMP-OPTIONS, THE USER 2026-11-05: "keep the elden ring camera invert options, make them work with KH's camera (non-inverted
/// is default)"; "make sure the default setting for the camera is NOT reversed") THE KH CAMERA'S FLIPS (horizontal, vertical):
/// the settings file's own per-axis values (1 Reversed; absent / 0 Normal), each axis on its own. The game's stored bytes are
/// never read for them, and the older single flags (`camera.axes_set`, `camera.mouse_axes_set`) are ignored.
pub fn kh_flips(x_key: &str, y_key: &str) -> (bool, bool) {
    let guard = INI.lock().ok();
    let keys = guard.as_ref().and_then(|g| g.as_ref()).map(|i| &i.keys);
    flips_of(&|k| keys.and_then(|m| m.get(k).cloned()), x_key, y_key)
}

/// The flips of two per-axis keys (pure), the settings file read by `get`.
pub fn flips_of(get: &dyn Fn(&str) -> Option<String>, x_key: &str, y_key: &str) -> (bool, bool) {
    (kh_host::er_options::inverted(get(x_key).as_deref()), kh_host::er_options::inverted(get(y_key).as_deref()))
}

/// (CAMP-OPTIONS follow-ups, "camera invert must cover mouse look too", "make the mouse drive every KH camera and aim mode
/// the right stick drives") THE MOUSE AS KH3 PC READS IT: AS THE RIGHT STICK. KH3 PC registers the mouse as stick 1 (re-d NOTES,
/// 0x141074520 -> 0x14442BC20): MouseX / MouseY massaged by their ini Sensitivity 0.07, nothing while both are below 0.01, Y
/// inverted, divided by the frame time relative to 1/60, times the scale 0.5 (0x1486DAAF0), clamped to +-3.0 (0x1486DAAF4), and
/// injected as Gamepad_RightX / RightY with the engine's dead zone PRE-COMPENSATED (0x144424D10: `(1 - dz) v + sign(v) dz`, dz
/// the key's own DeadZone 0.25, so the engine's massage gives v back): every camera and aim mode that reads the right stick
/// reads the mouse (the field and lock-on cameras, the shotlock aim, the formchanges' cameras and ShooterMode aims); the
/// camera's own dead zone (0.3, KH3's camera code) still applies, as in KH3 PC.
/// Elden Ring's Keyboard/Mouse rows set it: Mouse Sensitivity (CSPcKeyConfig +0x880, relative to its default +0x878) scales,
/// Mouse X / Y-Axis flip by the KH camera's own per-axis values (`camera.mouse_invert_x` / `_y`, absent = Normal; the rows
/// write them and the game's bytes +0x881 / +0x882). Raw stick units: x right, y up.
pub fn mouse_stick((dx, dy): (i32, i32), frame_dt: f32) -> Vec2 {
    if (dx, dy) == (0, 0) {
        return Vec2::ZERO;
    }
    let byte = |at: u16| RealGame.key_config(at);
    mouse_stick_of((dx, dy), frame_dt, byte(0x880).unwrap_or(5), byte(0x878).unwrap_or(5), kh_flips(MOUSE_INVERT_X_KEY, MOUSE_INVERT_Y_KEY))
}

/// KH3 PC's mouse axes: the ini `MouseX` / `MouseY` Sensitivity (DefaultInput.ini lines 35-36).
pub const MOUSE_AXIS_SENSITIVITY: f32 = 0.07;
/// KH3 PC's mouse-to-stick scale and clamp (0x1486DAAF0 / 0x1486DAAF4, written by 0x140EDF630).
pub const MOUSE_STICK_SCALE: f32 = 0.5;
pub const MOUSE_STICK_CLAMP: f32 = 3.0;
/// The right stick's engine dead zone the injection pre-compensates (DefaultInput.ini Gamepad_RightX / RightY DeadZone).
pub const STICK_ENGINE_DEAD_ZONE: f32 = 0.25;

/// KH3 PC's 0x144424D10: a value the engine's dead zone `dz` will give back unchanged after its massage.
pub fn undead(v: f32, dz: f32) -> f32 {
    if v == 0.0 || dz <= 0.0 {
        v
    } else {
        (1.0 - dz) * v + dz.copysign(v)
    }
}

/// The mouse as the right stick (pure; KH3 PC's 0x14442BC20): `sens` 0 ..= 10 relative to the game's default `default_sens`
/// scales KH3's 0.5; `flips` the per-axis inverts (horizontal, vertical). Normal: mouse right = stick right, mouse up = stick up.
pub fn mouse_stick_of((dx, dy): (i32, i32), frame_dt: f32, sens: u8, default_sens: u8, (flip_x, flip_y): (bool, bool)) -> Vec2 {
    let (mx, my) = (dx as f32 * MOUSE_AXIS_SENSITIVITY, dy as f32 * MOUSE_AXIS_SENSITIVITY);
    if mx.abs() < 0.01 && my.abs() < 0.01 {
        return Vec2::ZERO;
    }
    // (KH3: Y inverted, mouse movement down is positive counts; then per 1/60 s of the frame)
    let frames = if frame_dt > 0.0 { frame_dt * 60.0 } else { 1.0 };
    let scale = MOUSE_STICK_SCALE * (f32::from(sens.min(10)) + 1.0) / (f32::from(default_sens.min(10)) + 1.0);
    let x = mx / frames * scale * if flip_x { -1.0 } else { 1.0 };
    let y = -my / frames * scale * if flip_y { -1.0 } else { 1.0 };
    let (x, y) = (x.clamp(-MOUSE_STICK_CLAMP, MOUSE_STICK_CLAMP), y.clamp(-MOUSE_STICK_CLAMP, MOUSE_STICK_CLAMP));
    Vec2::new(undead(x, STICK_ENGINE_DEAD_ZONE), undead(y, STICK_ENGINE_DEAD_ZONE))
}

/// (TITLE-AUDIO) THE KH SOUNDS' LEVELS are Elden Ring's own sliders per kind (fx.rs `apply_levels`): the output = Master,
/// KH3's config layer of each category = Music (BGM), Voice (VOICE, Vo_*) or Sound Effects (every SE category), each
/// relative to its default 7 so the shipped balance stays where it was. (Before: Master x Sound Effects on everything.)
/// These are Elden Ring's four Sound rows (0..10 each).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoundRows {
    pub master: u8,
    pub music: u8,
    pub sfx: u8,
    pub voice: u8,
}

/// (TITLE-AUDIO) Elden Ring's Sound rows NOW: Master Volume, Music, Sound Effects, Voice (GameSettings + 0x20 / 0x04 / 0x05 /
/// 0x06, the MENU_OPTION_DATA rows 0x04..0x07 through `MD_TO_GS`). Read live (the title has no HUD frame); while the game's
/// settings are not made: KH3's merged Master (erconfig) and the game's defaults 7.
pub fn sound_rows() -> SoundRows {
    match game_settings() {
        // SAFETY: bytes of the live settings object (read as bytes).
        Some(gs) => unsafe { SoundRows { master: gs.add(0x20).read().min(10), music: gs.add(0x04).read().min(10), sfx: gs.add(0x05).read().min(10), voice: gs.add(0x06).read().min(10) } },
        None => SoundRows { master: (crate::erconfig::master_gain() * 10.0).round() as u8, music: 7, sfx: 7, voice: 7 },
    }
}

/// Sound Effects' factor (pure): 7 (the game's default) = 1.
pub fn sfx_factor(sfx: u32) -> f32 {
    sfx.min(10) as f32 / 7.0
}

/// The KH block of kh-config.txt (the KH rows), read once / written when a KH row changed.
pub fn kh_block(hud: &mut kh_host::hud::Hud) {
    static READ: AtomicBool = AtomicBool::new(false);
    let file = crate::save::dir().map(|d| d.join("kh-config.txt"));
    if !READ.swap(true, Ordering::Relaxed) {
        if let Some(config) = file.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|t| kh_core::config::UserConfig::from_text(&t)) {
            hud.set_config(config);
            erlog!("options: KH3's own block read (the KINGDOM HEARTS page's rows)");
        }
    }
    if hud.take_config_save() {
        if let Some(path) = file {
            let _ = path.parent().map(std::fs::create_dir_all);
            if let Err(e) = std::fs::write(&path, hud.config().to_text()) {
                erlog!("options: {} could not be written ({e})", path.display());
            }
        }
    }
}

/// The in-game System entry was chosen at this tick (0: not): Elden Ring's main menu is closed first, then ours opens.
static INGAME_AT: AtomicU64 = AtomicU64::new(0);
/// The tick of the latest in-game HUD frame.
static LAST_INGAME: AtomicU64 = AtomicU64::new(0);
static INGAME_PRESSES: AtomicU32 = AtomicU32::new(0);

/// ONCE A HUD FRAME IN GAME (kh-er hud.rs): the host, the KH block, the System entry, the KH side's publication.
pub fn ingame(hud: &mut kh_host::hud::Hud, control: &Control, er_menu: bool) {
    install_host(hud, control, true);
    hud.settings_only = false; // (the title's "settings only" never reaches the game's HUD)
    crate::remap_er::set_options_up(hud.camp_screen() == Some(kh_ui::camp::CampScreen::Config)); // (the lock-out protection)
    crate::remap_er::set_kh_screen(false); // (in game a menu of ours is hud.rs `menu_open`; the title's screens set it)
    // the keys KH binds are not the game's while Sora plays; all of them while a menu of ours is up; none in a menu of the game's
    crate::remap_er::set_swallow(if er_menu { 0 } else if hud.camp_screen().is_some() { 2 } else { 1 });
    LAST_INGAME.store(now(), Ordering::Relaxed);
    hud.config_external = false;
    kh_block(hud);
    let t = now();
    if take_ingame_wanted() {
        INGAME_AT.store(t, Ordering::Relaxed);
        INGAME_PRESSES.store(1, Ordering::Relaxed);
        erlog!("options: Elden Ring's System entry chosen: its main menu is closed ({}), then our settings screen opens", crate::inputs::press_for_er(crate::inputs::XI_B, crate::inputs::SCAN_Q).unwrap_or("NOT PRESSED"));
    }
    let asked = INGAME_AT.load(Ordering::Relaxed);
    if asked != 0 {
        if !er_menu && hud.open_camp_settings() {
            INGAME_AT.store(0, Ordering::Relaxed);
            erlog!("options: OUR SETTINGS SCREEN opened from Elden Ring's System entry");
        } else if t.saturating_sub(asked) > 6000 {
            INGAME_AT.store(0, Ordering::Relaxed);
            erlog!("options: Elden Ring's main menu did not close in 6 s: our settings screen was not opened");
        } else if er_menu && t.saturating_sub(asked) > 900 * u64::from(INGAME_PRESSES.load(Ordering::Relaxed)) && INGAME_PRESSES.load(Ordering::Relaxed) < 4 {
            INGAME_PRESSES.fetch_add(1, Ordering::Relaxed);
            let _ = crate::inputs::press_for_er(crate::inputs::XI_B, crate::inputs::SCAN_Q);
        }
    }
    publish(hud.config());
}

/// Our screen is up on the title (the SYSTEM entry); after it has closed, its frames run on until this tick (the camp's
/// own closing), then the title's HUD goes.
static TITLE_UP: AtomicBool = AtomicBool::new(false);
static TITLE_LEAVE: AtomicU64 = AtomicU64::new(0);
/// The title's screen was asked of the HUD (once per SYSTEM).
static TITLE_OPENED: AtomicBool = AtomicBool::new(false);

/// ONCE A FRAME (FRAME_BEGIN) on the title: our screen in place of Elden Ring's (its title list stays on SYSTEM under it,
/// the pad hidden from it), and B on our screen's top brings the title back as it was.
pub fn title_frame(control: &Control) {
    if take_title_wanted() && !TITLE_UP.load(Ordering::Relaxed) {
        crate::hud::title_prepare(control);
        TITLE_UP.store(true, Ordering::Relaxed);
        TITLE_LEAVE.store(0, Ordering::Relaxed);
        erlog!("options: the title's SYSTEM chosen: our settings screen takes its place");
    }
    if !TITLE_UP.load(Ordering::Relaxed) {
        return;
    }
    crate::inputs::set_mask(u32::MAX);
    crate::remap_er::set_options_up(true); // (the lock-out protection)
    crate::remap_er::set_swallow(2); // (the title's list under it must not move)
    let leave = TITLE_LEAVE.load(Ordering::Relaxed);
    // (asked only once: after its B the frames run on through the camp's closing, never opening it again)
    let (up, opened) = crate::hud::title_options_frame(control, leave == 0 && !TITLE_OPENED.load(Ordering::Relaxed));
    if opened {
        TITLE_OPENED.store(true, Ordering::Relaxed);
    }
    if leave == 0 && !up {
        TITLE_LEAVE.store(now() + 200, Ordering::Relaxed);
    } else if leave != 0 && now() >= leave {
        TITLE_OPENED.store(false, Ordering::Relaxed);
        crate::hud::title_end();
        crate::remap_er::set_options_up(false);
        crate::remap_er::set_swallow(0);
        TITLE_UP.store(false, Ordering::Relaxed);
        TITLE_LEAVE.store(0, Ordering::Relaxed);
        erlog!("options: our settings screen left: Elden Ring's title menu is back (on SYSTEM)");
        RESTORE_UNTIL.store(now() + 1500, Ordering::Relaxed);
    }
}

/// Once a title frame (frame.rs, with `title_frame`): nothing left to do here (the list's own update puts the cursor back,
/// `list_update_hook`); kept as the frame's seam.
pub fn title_cursor_frame() {
    if RESTORE_UNTIL.load(Ordering::Relaxed) != 0 && now() >= RESTORE_UNTIL.load(Ordering::Relaxed) {
        RESTORE_UNTIL.store(0, Ordering::Relaxed);
        erlog!("options: the title's list was not seen updating after our screen: its cursor is the game's");
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// D-pad Up (the in-game walk used it to wrap the pause menu's cursor to System: NO-ER-PAUSE, never pressed now)
    const XI_UP: u16 = 0x0001;

    /// (CAMP-OPTIONS follow-ups) the mouse is the right stick as in KH3 PC (0.07 x 0.5 per 1/60 s, clamped to +-3), Elden Ring's
    /// Mouse Sensitivity scales it, Mouse X / Y-Axis flip it once chosen
    #[test]
    fn co_the_mouse_is_kh3_pcs_right_stick_by_the_games_mouse_rows() {
        // (the values before the engine dead zone's pre-compensation; `undead` gives them back through the engine's massage)
        let massage = |r: f32| r.signum() * (r.abs() - STICK_ENGINE_DEAD_ZONE).max(0.0) / (1.0 - STICK_ENGINE_DEAD_ZONE);
        let near = |v: Vec2, x: f32, y: f32| (massage(v.x) - x).abs() < 1e-4 && (massage(v.y) - y).abs() < 1e-4;
        let f = 1.0 / 60.0;
        assert!((massage(undead(0.35, STICK_ENGINE_DEAD_ZONE)) - 0.35).abs() < 1e-6 && undead(0.0, 0.25) == 0.0 && undead(-0.1, 0.25) < -0.25);
        // 10 counts right and up in a 1/60 s frame: 10 x 0.07 x 0.5 = 0.35 right, 0.35 up (mouse up = stick up)
        assert!(near(mouse_stick_of((10, -10), f, 5, 5, (false, false)), 0.35, 0.35));
        // per 1/60 s: the same movement in a 1/30 s frame is half the stick
        assert!(near(mouse_stick_of((10, -10), 2.0 * f, 5, 5, (false, false)), 0.175, 0.175));
        // Reversed X / Y flip their own axis only; neither: Normal
        assert!(near(mouse_stick_of((10, -10), f, 5, 5, (true, false)), -0.35, 0.35));
        assert!(near(mouse_stick_of((10, -10), f, 5, 5, (false, true)), 0.35, -0.35));
        assert!(near(mouse_stick_of((10, -10), f, 5, 5, (false, false)), 0.35, 0.35));
        // sensitivity: 10 is 11 / 6 of the default, 0 one sixth
        assert!(near(mouse_stick_of((6, 0), f, 10, 5, (false, false)), 6.0 * 0.07 * 0.5 * 11.0 / 6.0, 0.0));
        assert!(near(mouse_stick_of((6, 0), f, 0, 5, (false, false)), 6.0 * 0.07 * 0.5 / 6.0, 0.0));
        // KH3's clamp (+-3: the mouse can go past the stick's 1) and its 0.01 floor
        assert!(near(mouse_stick_of((1000, 0), f, 5, 5, (false, false)), 3.0, 0.0));
        assert_eq!(mouse_stick_of((0, 0), f, 5, 5, (false, false)), Vec2::ZERO);
        assert_eq!(mouse_stick((0, 0), f), Vec2::ZERO);
    }

    #[test]
    fn co_the_settings_file_line_is_replaced_or_added() {
        let text = "# KH settings\nhud.kh = 1\nr.hybrid_native = 1 # the native route\nr.hybrid_native = 1\n; comment\n";
        let out = set_line(text, "r.hybrid_native", "0");
        assert_eq!(out, "# KH settings\nhud.kh = 1\nr.hybrid_native = 0 # the native route\n; comment\n");
        assert_eq!(crate::control::parse(&out).iter().filter(|(k, _)| k == "r.hybrid_native").map(|(_, v)| v.as_str()).collect::<Vec<_>>(), ["0"]);
        let added = set_line("hud.kh = 1", "r.hybrid_native", "1");
        assert_eq!(added, "hud.kh = 1\nr.hybrid_native = 1\n");
        // a commented-out line is not the key
        assert_eq!(set_line("# r.hybrid_native = 1\n", "r.hybrid_native", "0"), "# r.hybrid_native = 1\nr.hybrid_native = 0\n");
        assert_eq!(set_line("", "a", "b"), "a = b\n");
        // (CAMP-OPTIONS) the older axis flag dropped, the rest kept
        assert_eq!(remove_line("hud.kh = 1\ncamera.axes_set = 1\n# camera.axes_set = 1\nr.x = 2\n", "camera.axes_set"), "hud.kh = 1\n# camera.axes_set = 1\nr.x = 2\n");
    }

    #[test]
    fn co_the_walks_to_elden_rings_own_screens() {
        // the title: its list stayed on SYSTEM: A, then the walk
        let t = native_presses(Native::DataAgreement);
        assert_eq!(t.iter().map(|x| x.0).collect::<Vec<_>>(), [crate::inputs::XI_A, XI_RB, XI_RB, XI_RB, XI_RB, XI_DOWN, XI_DOWN, XI_DOWN, crate::inputs::XI_A]);
        assert_eq!(native_presses(Native::KeyBindings).iter().filter(|x| x.0 == XI_RB).count(), 6);
        // (NO-ER-PAUSE) no walk ever presses Start (the pause menu's key), Esc or Up (its wrap to System)
        for screen in [Native::KeyBindings, Native::PadButtons, Native::DataAgreement, Native::QuitGame, Native::Desktop] {
            assert!(native_presses(screen).iter().all(|x| x.0 != crate::inputs::XI_START && x.0 != XI_UP && x.1 != crate::inputs::SCAN_ESC), "{screen:?}");
        }
    }

    /// (NO-ER-PAUSE) in game a row never walks Elden Ring's System menu: Quit Game = the title, Return to Desktop = the title then
    /// the window; the title's rows walk the title's own list
    #[test]
    fn np_the_quit_rows_go_the_kh_way_in_game() {
        assert_eq!(native_route(Native::QuitGame, true), NativeRoute::Title);
        assert_eq!(native_route(Native::Desktop, true), NativeRoute::TitleThenDesktop);
        for screen in [Native::KeyBindings, Native::PadButtons, Native::DataAgreement] {
            assert_eq!(native_route(screen, true), NativeRoute::Refused, "{screen:?}");
            assert_eq!(native_route(screen, false), NativeRoute::TitleSystem, "{screen:?}");
        }
    }

    /// (NO-ER-PAUSE) Return to Desktop: nothing before the title; the window 1.5 s after it came; dropped when it never comes
    #[test]
    fn np_return_to_desktop_closes_the_window_on_the_title() {
        assert_eq!(desktop_step(0, 0, true, 5_000), (false, 0, false), "not asked");
        assert_eq!(desktop_step(1_000, 0, false, 30_000), (false, 0, true), "the way out still loading");
        assert_eq!(desktop_step(1_000, 0, true, 31_000), (false, 31_000, true), "the title came: its fade first");
        assert_eq!(desktop_step(1_000, 31_000, true, 32_000), (false, 31_000, true));
        assert_eq!(desktop_step(1_000, 31_000, true, 32_500), (true, 31_000, false), "closed once, then over");
        assert_eq!(desktop_step(1_000, 0, false, 1_000 + DESKTOP_WAIT_MS + 1), (false, 0, false), "the title never came: dropped");
    }

    /// (the user: "keep the elden ring camera invert options, make them work with KH's camera (non-inverted is default)"; "make
    /// sure the default setting for the camera is NOT reversed") each axis is its own value, absent = Normal; the game's bytes
    /// and the older single flag never invert it
    #[test]
    fn co_each_axis_flips_only_its_own_kh_camera_input() {
        let ini = |pairs: &'static [(&'static str, &'static str)]| move |k: &str| pairs.iter().find(|(key, _)| *key == k).map(|(_, v)| v.to_string());
        // a fresh file: Normal (whatever the game's stored bytes are: they are not read)
        assert_eq!(flips_of(&ini(&[]), INVERT_X_KEY, INVERT_Y_KEY), (false, false));
        // the user's file after the 2026-11-05 deploy: the old single flag, no per-axis value: Normal
        assert_eq!(flips_of(&ini(&[("camera.axes_set", "1"), ("camera.mouse_axes_set", "1")]), INVERT_X_KEY, INVERT_Y_KEY), (false, false));
        assert_eq!(flips_of(&ini(&[("camera.axes_set", "1")]), MOUSE_INVERT_X_KEY, MOUSE_INVERT_Y_KEY), (false, false));
        // X Reversed flips the horizontal only, and the other way round
        assert_eq!(flips_of(&ini(&[("camera.invert_x", "1")]), INVERT_X_KEY, INVERT_Y_KEY), (true, false));
        assert_eq!(flips_of(&ini(&[("camera.invert_y", "1"), ("camera.invert_x", "0")]), INVERT_X_KEY, INVERT_Y_KEY), (false, true));
        assert_eq!(flips_of(&ini(&[("camera.mouse_invert_x", "1")]), MOUSE_INVERT_X_KEY, MOUSE_INVERT_Y_KEY), (true, false));
        assert!(OLD_AXES_FLAGS.contains(&"camera.axes_set"));
        // through erconfig's readers: each flip reaches its own stick axis
        use kh_core::config::{ConfigOption as O, UserConfig};
        let mut c = UserConfig::default();
        c.set(O::HorizontalCameraControl, 1);
        c.set(O::VerticalCameraControl, 0);
        crate::erconfig::publish(&c);
        let (x, y, _) = crate::erconfig::camera();
        assert!(x < 0.0 && y > 0.0, "x {x} y {y}");
        crate::erconfig::publish(&UserConfig::default());
    }

    #[test]
    fn co_kh_sounds_follow_the_games_sliders() {
        assert!((sfx_factor(7) - 1.0).abs() < 1e-6);
        assert_eq!(sfx_factor(0), 0.0);
        assert!(sfx_factor(10) > 1.4 && sfx_factor(99) == sfx_factor(10));
    }

    #[test]
    fn co_patterns_are_the_ones_the_port_check_runs() {
        // every name is ours and unique; the table's versions are both recorded
        let mut names = std::collections::HashSet::new();
        for l in &LOCATED {
            assert!(l.name.starts_with("(CAMP-OPTIONS)") && names.insert(l.name) && l.rva_2_7_1_0 != 0 && l.rva_2_7_2_0 != 0, "{}", l.name);
        }
    }
}
