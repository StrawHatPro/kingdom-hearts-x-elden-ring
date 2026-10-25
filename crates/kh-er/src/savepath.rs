//! (SAVEPATH) THE SAVE FILE THE GAME ACTUALLY USES, whatever its name (the user, 2026-10-24: "change it": players pick any
//! `.sl2` name in the Me3 Manager GUI or a hand-made me3 profile, and the mod must work with it).
//!
//! HOW me3 MOVES THE SAVE (MEASURED in its own log, er\logs_test\me3-cli-*.err.log): the profile's `savefile = "<name>.sl2"`
//! reaches the game process in me3's attach config; its mod host rewrites the path IN THE GAME'S FILE-DEVICE MANAGER
//! (`me3_mod_host::asset_hooks: device_manager: override=...\EldenRing\<steam id>\<name>.sl2`). Everything below the
//! device manager (CreateFileW -> ntdll's NtCreateFile) is handed the REAL file. The game's own memory still names
//! `ER0000.sl2` (the virtual path the device manager rewrites), so the save manager's path in memory is the wrong
//! answer, and me3 exposes the name to the game process only inside its private attach config (a temp file of its
//! cache), not through an environment variable or the command line: neither is a route that holds across me3 versions.
//!
//! WHAT WE DO, in this order of trust:
//! 1. `save.file = <name>` in the settings: an explicit override (looked for in `%APPDATA%\EldenRing\*\`);
//! 2. THE FILE THE GAME OPENS: ntdll's `NtCreateFile` is detoured (the lowest user-mode door every file open goes
//!    through: CreateFileW, CreateFile2, any launcher's redirect above it). A successful open whose name ends in `.sl2`
//!    has its real path taken from the handle (`GetFinalPathNameByHandleW`; the name as given when that fails). The
//!    detour does nothing else: four compares for every other open, no allocation, no lock it can wait on, no log;
//! 3. the path an earlier payload of this process detected (an environment variable of the process: a reloaded payload
//!    starts after the game's opens);
//! 4. the `.sl2` under `%APPDATA%\EldenRing\*\` written most recently SINCE THIS PROCESS STARTED (the title writes its
//!    system part: the game's own file is the one that changed);
//! 5. not detected: the watcher looks for `KHSora.sl2` (the bundled profile's name); the slots read the game's summary
//!    in memory only.
//!
//! THE FILE IS ONLY EVER READ. When it is `ER0000.sl2` (no `savefile` in the me3 profile) the log says so loudly and the
//! player is told once in game: the mod would be playing on the normal Elden Ring save. Once a file of another name has
//! been seen open, an `ER0000.sl2` open (another tool reading it) never replaces it.

use std::cell::UnsafeCell;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::control::Control;
use crate::erlog;

/// The name the bundled me3 profile gives the mod's save (the watcher's fallback when nothing is detected).
pub const DEFAULT_NAME: &str = "KHSora.sl2";
/// Elden Ring's own save: what the game uses when no `savefile` is set.
pub const VANILLA_NAME: &str = "ER0000.sl2";
/// Carries the detected path to a reloaded payload of the same process.
const ENV_CARRY: &str = "KH_ER_SAVE_PATH";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    /// `save.file` in the settings.
    Override,
    /// The file the game opened (NtCreateFile).
    Opened,
    /// Detected by an earlier payload of this process.
    Carried,
    /// The `.sl2` written most recently since the process started.
    Newest,
    /// Nothing detected: `save.file` / `KHSora.sl2` looked for by name (watcher only).
    Fallback,
}

impl Method {
    pub fn text(self) -> &'static str {
        match self {
            Method::Override => "the settings' save.file (explicit override)",
            Method::Opened => "the file the game opened (NtCreateFile)",
            Method::Carried => "detected by an earlier payload of this process",
            Method::Newest => "the .sl2 written most recently since the game started",
            Method::Fallback => "NOT DETECTED: looked for by name",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub path: Option<PathBuf>,
    pub method: Method,
}

impl Choice {
    /// The path is the game's own file (detected or set explicitly), not a guess by name.
    pub fn detected(&self) -> bool {
        self.path.is_some() && self.method != Method::Fallback
    }
}

// ------------------------------------------------------------------------------------------ pure rules

/// The file's name is Elden Ring's own save (`ER0000.sl2`, any case).
pub fn is_vanilla(path: &Path) -> bool {
    path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.eq_ignore_ascii_case(VANILLA_NAME))
}

/// A path's extension is exactly `.sl2` (any case; `.sl2.bak` is not).
pub fn is_sl2(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("sl2"))
}

/// UTF-16 units that end in `.sl2` (any case). What the detour checks on every open: four compares.
pub fn units_end_sl2(units: &[u16]) -> bool {
    let n = units.len();
    n >= 4 && units[n - 4] == b'.' as u16 && units[n - 3] | 0x20 == b's' as u16 && units[n - 2] | 0x20 == b'l' as u16 && units[n - 1] == b'2' as u16
}

/// A path as NT / the handle give it, in the form std opens: `\??\C:\..`, `\\?\C:\..` -> `C:\..`; `\\?\UNC\s\x` -> `\\s\x`.
pub fn dos_path(text: &str) -> String {
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\").or_else(|| text.strip_prefix(r"\??\UNC\")) {
        return format!(r"\\{rest}");
    }
    text.strip_prefix(r"\??\").or_else(|| text.strip_prefix(r"\\?\")).unwrap_or(text).to_string()
}

/// A newly opened `.sl2` replaces the one seen before, unless it is `ER0000.sl2` and the one before is not.
pub fn accept_open(before: Option<&Path>, new: &Path) -> bool {
    !(is_vanilla(new) && before.is_some_and(|b| !is_vanilla(b)))
}

/// The `.sl2` among `files` (path, modified) written most recently at or after `since`.
pub fn newest_since(files: &[(PathBuf, SystemTime)], since: SystemTime) -> Option<PathBuf> {
    files.iter().filter(|(p, t)| is_sl2(p) && *t >= since).max_by_key(|(_, t)| *t).map(|(p, _)| p.clone())
}

/// The explicit override from the setting's text (`""` / `auto`: none).
pub fn override_name(setting: Option<&str>) -> Option<&str> {
    setting.map(str::trim).filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("auto"))
}

/// WHICH FILE (see the module's head). `find(name)`: that name under `%APPDATA%\EldenRing\*\`; `newest()`: rule 4.
pub fn choose(setting: Option<&str>, opened: Option<&Path>, carried: Option<&Path>, find: &dyn Fn(&str) -> Option<PathBuf>, newest: &dyn Fn() -> Option<PathBuf>) -> Choice {
    if let Some(path) = override_name(setting).and_then(find) {
        return Choice { path: Some(path), method: Method::Override };
    }
    if let Some(p) = opened {
        return Choice { path: Some(p.to_path_buf()), method: Method::Opened };
    }
    if let Some(p) = carried {
        return Choice { path: Some(p.to_path_buf()), method: Method::Carried };
    }
    if let Some(p) = newest() {
        return Choice { path: Some(p), method: Method::Newest };
    }
    Choice { path: find(override_name(setting).unwrap_or(DEFAULT_NAME)), method: Method::Fallback }
}

/// The loud log line for a save that is Elden Ring's own (`None`: it is not).
pub fn vanilla_warning(choice: &Choice) -> Option<String> {
    let path = choice.path.as_ref().filter(|p| choice.detected() && is_vanilla(p))?;
    Some(format!(
        "savepath: !!!!! WARNING: THE GAME IS USING ELDEN RING'S NORMAL SAVE FILE {} !!!!! The me3 profile sets no `savefile` (or sets ER0000.sl2): the mod's saving rules now act on your normal Elden Ring characters. Close the game and give the profile its own save file (savefile = \"KHSora.sl2\" in the .me3 file, or Profile settings -> custom save file in the Me3 Manager). The mod itself only ever reads the file.",
        path.display()
    ))
}

// ------------------------------------------------------------------------------------------ the detour

#[repr(C)]
pub struct UnicodeString {
    pub length: u16,
    pub maximum: u16,
    pub buffer: *const u16,
}

#[repr(C)]
pub struct ObjectAttributes {
    pub length: u32,
    pub root: *mut c_void,
    pub name: *const UnicodeString,
    pub attributes: u32,
    pub security: *mut c_void,
    pub qos: *mut c_void,
}

type NtCreateFile = unsafe extern "system" fn(*mut *mut c_void, u32, *const ObjectAttributes, *mut c_void, *mut i64, u32, u32, u32, u32, *mut c_void, u32) -> i32;

#[link(name = "kernel32")]
extern "system" {
    fn GetFinalPathNameByHandleW(file: *mut c_void, path: *mut u16, units: u32, flags: u32) -> u32;
}

const CAP: usize = 1024;
struct Seen(UnsafeCell<[u16; CAP]>);
// SAFETY: written and read only by whoever holds BUSY.
unsafe impl Sync for Seen {}
static SEEN: Seen = Seen(UnsafeCell::new([0; CAP]));
static SEEN_LEN: AtomicUsize = AtomicUsize::new(0);
/// Counts the `.sl2` opens recorded (0: none yet).
static SEEN_SEQ: AtomicU32 = AtomicU32::new(0);
static BUSY: AtomicBool = AtomicBool::new(false);
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static HOOKED: AtomicBool = AtomicBool::new(false);

/// The units of an open's name (`None`: no name).
///
/// # Safety
/// `attrs` is null or the caller's OBJECT_ATTRIBUTES (valid for the call).
unsafe fn name_units<'a>(attrs: *const ObjectAttributes) -> Option<&'a [u16]> {
    if attrs.is_null() {
        return None;
    }
    let name = (*attrs).name;
    if name.is_null() || (*name).buffer.is_null() {
        return None;
    }
    Some(std::slice::from_raw_parts((*name).buffer, (*name).length as usize / 2))
}

/// RECORDS A `.sl2` THE GAME OPENED: its real path from the handle, else the name as given (when it is not relative to
/// a directory handle). Skipped when another thread is recording (never waits).
///
/// # Safety
/// `handle` is an open file handle; `attrs` as for `name_units`.
pub unsafe fn record(handle: *mut c_void, attrs: *const ObjectAttributes) {
    if BUSY.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        return;
    }
    let buf = &mut *SEEN.0.get();
    let mut n = GetFinalPathNameByHandleW(handle, buf.as_mut_ptr(), CAP as u32, 0) as usize;
    if n == 0 || n >= CAP {
        n = 0;
        if let Some(units) = name_units(attrs).filter(|_| (*attrs).root.is_null()) {
            n = units.len().min(CAP);
            buf[..n].copy_from_slice(&units[..n]);
        }
    }
    if n > 0 {
        SEEN_LEN.store(n, Ordering::Relaxed);
        SEEN_SEQ.fetch_add(1, Ordering::Release);
    }
    BUSY.store(false, Ordering::Release);
}

/// The latest `.sl2` the game opened, with its count (`None`: none yet, or being written this instant).
fn read_seen() -> Option<(u32, PathBuf)> {
    if BUSY.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        return None;
    }
    let seq = SEEN_SEQ.load(Ordering::Acquire);
    let n = SEEN_LEN.load(Ordering::Relaxed).min(CAP);
    // SAFETY: BUSY is held.
    let text = String::from_utf16_lossy(unsafe { &(&*SEEN.0.get())[..n] });
    BUSY.store(false, Ordering::Release);
    (seq != 0 && n > 0).then(|| (seq, PathBuf::from(dos_path(&text))))
}

/// (nothing in here can panic or allocate: loads, compares, the recorder above)
unsafe extern "system" fn nt_create_file_hook(handle: *mut *mut c_void, access: u32, attrs: *const ObjectAttributes, io: *mut c_void, size: *mut i64, file_attributes: u32, share: u32, disposition: u32, options: u32, ea: *mut c_void, ea_length: u32) -> i32 {
    let original: NtCreateFile = std::mem::transmute(ORIGINAL.load(Ordering::Relaxed));
    let status = original(handle, access, attrs, io, size, file_attributes, share, disposition, options, ea, ea_length);
    if status >= 0 && !handle.is_null() && name_units(attrs).is_some_and(units_end_sl2) {
        record(*handle, attrs);
    }
    status
}

/// Detours ntdll's NtCreateFile (undone with the renderer's hooks). Called once per payload, before the game's saves.
pub fn install() -> bool {
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    if HOOKED.load(Ordering::SeqCst) {
        return true;
    }
    let ntdll: Vec<u16> = "ntdll.dll\0".encode_utf16().collect();
    // SAFETY: ntdll is mapped in every process; the export is looked up by name.
    let target = unsafe {
        let module = GetModuleHandleW(ntdll.as_ptr());
        if module.is_null() {
            None
        } else {
            GetProcAddress(module, c"NtCreateFile".as_ptr() as *const u8).map(|f| f as usize)
        }
    };
    let Some(target) = target else {
        erlog!("savepath: ntdll's NtCreateFile was not found: the save file is detected by its write time only");
        return false;
    };
    // SAFETY: the function's start, by export; the hook has its exact signature and stays loaded (the payload is never unloaded).
    match unsafe { kh_er_render::inline_hook(target, nt_create_file_hook as *const () as usize, &ORIGINAL, "NtCreateFile (SAVEPATH)") } {
        Ok(()) => {
            HOOKED.store(true, Ordering::SeqCst);
            erlog!("savepath: ntdll's NtCreateFile is detoured: the .sl2 the game opens is recorded (read only; nothing else is touched)");
            true
        }
        Err(e) => {
            ORIGINAL.store(0, Ordering::SeqCst);
            erlog!("savepath: NtCreateFile was NOT detoured ({e}): the save file is detected by its write time only");
            false
        }
    }
}

pub fn uninstalled() {
    HOOKED.store(false, Ordering::SeqCst);
}

// ------------------------------------------------------------------------------------------ the game side

/// `name` under `%APPDATA%\EldenRing\<id>\` (the first folder that has it).
pub fn find_named(name: &str) -> Option<PathBuf> {
    let base = PathBuf::from(std::env::var_os("APPDATA")?).join("EldenRing");
    std::fs::read_dir(base).ok()?.flatten().map(|e| e.path().join(name)).find(|p| p.is_file())
}

/// Every `.sl2` under `%APPDATA%\EldenRing\<id>\` with its write time.
fn listed_saves() -> Vec<(PathBuf, SystemTime)> {
    let Some(appdata) = std::env::var_os("APPDATA") else { return Vec::new() };
    let Ok(ids) = std::fs::read_dir(PathBuf::from(appdata).join("EldenRing")) else { return Vec::new() };
    ids.flatten()
        .filter_map(|id| std::fs::read_dir(id.path()).ok())
        .flat_map(|files| files.flatten())
        .filter_map(|f| {
            let p = f.path();
            let t = f.metadata().ok()?.modified().ok()?;
            is_sl2(&p).then_some((p, t))
        })
        .collect()
}

/// When this process started.
fn process_start() -> Option<SystemTime> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};
    let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
    // SAFETY: four FILETIMEs of ours; the pseudo handle of this process.
    if unsafe { GetProcessTimes(GetCurrentProcess(), &mut created, &mut exited, &mut kernel, &mut user) } == 0 {
        return None;
    }
    let ticks = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
    let unix = (ticks / 10_000_000).checked_sub(11_644_473_600)?;
    Some(UNIX_EPOCH + Duration::from_secs(unix))
}

struct State {
    started: bool,
    seq: u32,
    opened: Option<PathBuf>,
    carried: Option<PathBuf>,
    setting: Option<String>,
    current: Option<Choice>,
    next_scan: Option<Instant>,
    since: Option<SystemTime>,
    switches: u32,
    ignored_vanilla: bool,
    told_mismatch: Option<PathBuf>,
    notice: bool,
}

static STATE: Mutex<State> = Mutex::new(State { started: false, seq: 0, opened: None, carried: None, setting: None, current: None, next_scan: None, since: None, switches: 0, ignored_vanilla: false, told_mismatch: None, notice: false });
/// The player was told in game that the save is Elden Ring's own (once per process: the variable outlives a payload).
const ENV_TOLD: &str = "KH_ER_SAVE_VANILLA_TOLD";

/// Once a frame (savegate.rs): `Some` when the save file to use changed (always on the first call).
pub fn refresh(control: &Control) -> Option<Choice> {
    let mut st = STATE.lock().ok()?;
    let setting = control.str("save.file").map(str::to_string);
    let mut again = !st.started || setting != st.setting;
    if !st.started {
        st.started = true;
        st.since = process_start().map(|t| t - Duration::from_secs(2));
        st.carried = std::env::var_os(ENV_CARRY).map(PathBuf::from).filter(|p| p.is_file());
    }
    st.setting = setting;
    // the detour's latest record
    if SEEN_SEQ.load(Ordering::Acquire) != st.seq {
        if let Some((seq, path)) = read_seen() {
            st.seq = seq;
            if st.opened.as_deref() != Some(path.as_path()) {
                if accept_open(st.opened.as_deref(), &path) {
                    st.opened = Some(path);
                    again = true;
                } else if !std::mem::replace(&mut st.ignored_vanilla, true) {
                    erlog!("savepath: the game (or a tool in it) also opened {} : IGNORED, the save in use stays {}", path.display(), st.opened.as_ref().map_or(String::new(), |p| p.display().to_string()));
                }
            }
        }
    }
    // not detected yet: the write times are looked at every 2 s
    let undetected = !st.current.as_ref().is_some_and(|c| c.detected());
    if undetected && st.next_scan.is_none_or(|t| Instant::now() >= t) {
        st.next_scan = Some(Instant::now() + Duration::from_secs(2));
        again = true;
    }
    if !again {
        return notice(&mut st);
    }
    let since = st.since;
    let newest = move || since.and_then(|s| newest_since(&listed_saves(), s));
    let choice = choose(st.setting.as_deref(), st.opened.as_deref(), st.carried.as_deref(), &find_named, &newest);
    // (an override that differs from what the game opens is said once)
    if choice.method == Method::Override && st.opened.is_some() && st.opened != choice.path && st.told_mismatch != st.opened {
        st.told_mismatch = st.opened.clone();
        erlog!("savepath: NOTE: save.file = {} is in force, but the game opens {}", st.setting.as_deref().unwrap_or(""), st.opened.as_ref().map_or(String::new(), |p| p.display().to_string()));
    }
    if st.current.as_ref() == Some(&choice) {
        return notice(&mut st);
    }
    if st.switches < 12 {
        erlog!("savepath: THE GAME'S SAVE FILE: {} (method: {})", choice.path.as_ref().map_or("NOT FOUND".to_string(), |p| p.display().to_string()), choice.method.text());
        if !choice.detected() {
            erlog!("savepath: the save file is NOT DETECTED yet: the save confirmation watches {} by name; the slot lists read the game's summary in memory", override_name(st.setting.as_deref()).unwrap_or(DEFAULT_NAME));
        }
    }
    st.switches += 1;
    if let Some(warning) = vanilla_warning(&choice) {
        erlog!("{warning}");
        st.notice = std::env::var_os(ENV_TOLD).is_none();
    }
    if choice.detected() {
        if let Some(p) = &choice.path {
            std::env::set_var(ENV_CARRY, p);
        }
    }
    st.current = Some(choice.clone());
    drop(notice(&mut st));
    Some(choice)
}

/// (SAVEPATH) The in-game notice that the save is Elden Ring's own: two item plates, once, when a character stands in the world.
fn notice(st: &mut State) -> Option<Choice> {
    if st.notice && crate::game::player_age() > 120 {
        st.notice = false;
        std::env::set_var(ENV_TOLD, "1");
        crate::hud::say("item WARNING:_this_is_your_normal_Elden_Ring_save_(ER0000.sl2)");
        crate::hud::say("item Set_a_custom_save_file_in_your_me3_profile");
        erlog!("savepath: the player was told in game that the save in use is ER0000.sl2");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: &str) -> Option<PathBuf> {
        None
    }
    fn no_newest() -> Option<PathBuf> {
        None
    }

    /// DETECTION FROM A SIMULATED OPEN: a real `.sl2` of any name opened by handle goes through the detour's recorder and
    /// is chosen over every other route but the override.
    #[test]
    fn a_file_the_game_opens_is_detected_whatever_its_name() {
        use std::os::windows::io::AsRawHandle;
        let dir = std::env::temp_dir().join(format!("kh-er-savepath-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("My Own Name.SL2");
        std::fs::write(&file, b"BND4").unwrap();
        let open = std::fs::File::open(&file).unwrap();
        let nt: Vec<u16> = format!(r"\??\{}", file.display()).encode_utf16().collect();
        assert!(units_end_sl2(&nt) && !units_end_sl2(&"x.sl2.bak".encode_utf16().collect::<Vec<_>>()));
        let name = UnicodeString { length: (nt.len() * 2) as u16, maximum: (nt.len() * 2) as u16, buffer: nt.as_ptr() };
        let attrs = ObjectAttributes { length: std::mem::size_of::<ObjectAttributes>() as u32, root: std::ptr::null_mut(), name: &name, attributes: 0, security: std::ptr::null_mut(), qos: std::ptr::null_mut() };
        let before = SEEN_SEQ.load(Ordering::SeqCst);
        // SAFETY: an open handle and attributes of ours.
        unsafe { record(open.as_raw_handle() as *mut c_void, &attrs) };
        let (seq, seen) = read_seen().unwrap();
        assert_eq!(seq, before + 1);
        assert_eq!(std::fs::canonicalize(&seen).unwrap(), std::fs::canonicalize(&file).unwrap());
        let choice = choose(None, Some(&seen), Some(Path::new(r"C:\old.sl2")), &none, &no_newest);
        assert_eq!(choice.method, Method::Opened);
        assert!(choice.detected() && vanilla_warning(&choice).is_none());
        drop(open);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The detour can be put on this Windows' NtCreateFile: its stub starts `mov r10, rcx; mov eax, imm32` (8 bytes the
    /// renderer's prologue decoder moves as they are), or with someone's jump already there (followed by the decoder).
    #[test]
    fn ntcreatefile_starts_with_a_prologue_the_detour_moves() {
        use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
        let ntdll: Vec<u16> = "ntdll.dll\0".encode_utf16().collect();
        // SAFETY: lookups; 8 bytes of a mapped export are read.
        let code = unsafe {
            let f = GetProcAddress(GetModuleHandleW(ntdll.as_ptr()), c"NtCreateFile".as_ptr() as *const u8).unwrap();
            std::slice::from_raw_parts(f as usize as *const u8, 8).to_vec()
        };
        let stub = code[..4] == [0x4C, 0x8B, 0xD1, 0xB8];
        let jumped = code[0] == 0xE9 || code[..2] == [0xFF, 0x25];
        assert!(stub || jumped, "NtCreateFile starts {code:02x?}");
    }

    #[test]
    fn nt_and_handle_paths_become_plain_paths() {
        assert_eq!(dos_path(r"\??\D:\Profiles\a\AppData\Roaming\EldenRing\7656\KHTest.sl2"), r"D:\Profiles\a\AppData\Roaming\EldenRing\7656\KHTest.sl2");
        assert_eq!(dos_path(r"\\?\D:\x.sl2"), r"D:\x.sl2");
        assert_eq!(dos_path(r"\\?\UNC\srv\share\x.sl2"), r"\\srv\share\x.sl2");
        assert_eq!(dos_path(r"C:\x.sl2"), r"C:\x.sl2");
    }

    #[test]
    fn the_override_wins_and_auto_is_no_override() {
        let find = |n: &str| Some(PathBuf::from(format!(r"C:\EldenRing\1\{n}")));
        let c = choose(Some("KHTest.sl2"), Some(Path::new(r"C:\EldenRing\1\Other.sl2")), None, &find, &no_newest);
        assert_eq!((c.path.unwrap(), c.method), (PathBuf::from(r"C:\EldenRing\1\KHTest.sl2"), Method::Override));
        // `auto` / empty: the open decides
        for s in [Some("auto"), Some(" "), None] {
            assert_eq!(choose(s, Some(Path::new(r"C:\a\B.sl2")), None, &find, &no_newest).method, Method::Opened);
        }
        // an override that is not there: detection goes on
        assert_eq!(choose(Some("Gone.sl2"), Some(Path::new(r"C:\a\B.sl2")), None, &none, &no_newest).method, Method::Opened);
    }

    #[test]
    fn the_fallbacks_in_order() {
        let carried = Path::new(r"C:\a\Carried.sl2");
        assert_eq!(choose(None, None, Some(carried), &none, &no_newest).method, Method::Carried);
        let newest = || Some(PathBuf::from(r"C:\a\New.sl2"));
        assert_eq!(choose(None, None, None, &none, &newest).method, Method::Newest);
        // nothing detected: the watcher's name (save.file, else KHSora.sl2), never counted as detected
        let find = |n: &str| (n == DEFAULT_NAME).then(|| PathBuf::from(r"C:\a\KHSora.sl2"));
        let c = choose(None, None, None, &find, &no_newest);
        assert_eq!((c.path.clone(), c.method, c.detected()), (Some(PathBuf::from(r"C:\a\KHSora.sl2")), Method::Fallback, false));
        assert!(vanilla_warning(&c).is_none());
        // the newest write since the start, .sl2 only
        let t = |s: u64| UNIX_EPOCH + Duration::from_secs(s);
        let files = vec![(PathBuf::from("a.sl2"), t(100)), (PathBuf::from("b.sl2"), t(300)), (PathBuf::from("b.sl2.bak"), t(400)), (PathBuf::from("old.sl2"), t(10))];
        assert_eq!(newest_since(&files, t(50)), Some(PathBuf::from("b.sl2")));
        assert_eq!(newest_since(&files, t(500)), None);
    }

    #[test]
    fn er0000_is_warned_about_and_never_replaces_the_mods_save() {
        let vanilla = Path::new(r"D:\Profiles\a\AppData\Roaming\EldenRing\7656\er0000.SL2");
        let c = choose(None, Some(vanilla), None, &none, &no_newest);
        let w = vanilla_warning(&c).unwrap();
        assert!(w.contains("WARNING") && w.contains("er0000.SL2") && w.contains("savefile"));
        // a name of its own is not warned about
        assert!(vanilla_warning(&choose(None, Some(Path::new(r"C:\x\KHSora.sl2")), None, &none, &no_newest)).is_none());
        // once the mod's own file was seen open, an ER0000.sl2 open does not replace it; the other way round it does
        assert!(!accept_open(Some(Path::new(r"C:\x\KHSora.sl2")), vanilla));
        assert!(accept_open(Some(vanilla), Path::new(r"C:\x\KHSora.sl2")));
        assert!(accept_open(None, vanilla));
    }
}
