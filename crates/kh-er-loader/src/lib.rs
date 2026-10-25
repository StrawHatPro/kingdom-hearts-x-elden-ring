//! `kh_er_loader.dll`: what me3 injects. It owns the things that cannot be undone inside a running
//! game (the task registrations) and forwards every frame phase to the payload `kh_er.dll`, which
//! it copies to `live\` and loads from there, again whenever the file next to it changes. Old
//! payloads are told to shut down (so they can put the player back as he was) and are never
//! unloaded.
//!
//! Rules kept here: nothing happens on any executable other than Elden Ring 2.7.1.0 or 2.7.2.0 (PORT); no panic
//! unwinds into the game.
#![cfg(windows)]

use std::ffi::c_void;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use eldenring::cs::{CSTaskGroupIndex, CSTaskImp};
use eldenring::fd4::FD4TaskData;
use fromsoftware_shared::SharedTaskImpExt;
use pelite::pe64::{Pe, PeView};
use windows_sys::Win32::System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleW, GetProcAddress, LoadLibraryW};

/// (PORT) The executables this mod is built for (file versions of eldenring.exe): 2.7.1.0 (app 1.17.1) and 2.7.2.0
/// (the 2026-10-21 patch; kh-er's gamever.rs holds its address tables, the vendored SDK its RVA table).
const SUPPORTED_EXE_VERSIONS: [(u16, u16, u16, u16); 2] = [(2, 7, 1, 0), (2, 7, 2, 0)];

/// The frame phases forwarded to the payload; the payload is given the group index as a number.
const PHASES: &[CSTaskGroupIndex] = &[
    CSTaskGroupIndex::FrameBegin,
    CSTaskGroupIndex::PadStep,
    CSTaskGroupIndex::EndShiftWorldPosition,
    CSTaskGroupIndex::ChrIns_PreBehavior,
    CSTaskGroupIndex::ChrIns_PrePhysics_Begin,
    CSTaskGroupIndex::ChrIns_PrePhysics,
    CSTaskGroupIndex::ChrIns_PrePhysics_End,
    CSTaskGroupIndex::HavokWorldUpdate_Pre,
    CSTaskGroupIndex::HavokWorldUpdate_Post,
    CSTaskGroupIndex::CameraStep,
    CSTaskGroupIndex::ChrIns_PostPhysics,
    CSTaskGroupIndex::WorldChrMan_PostPhysics,
    CSTaskGroupIndex::MenuMan,
    CSTaskGroupIndex::Draw_Pre,
    CSTaskGroupIndex::BeginShiftWorldPosition,
];

type InitFn = unsafe extern "C" fn(dir: *const u16, len: usize, generation: u32) -> u32;
type FrameFn = unsafe extern "C" fn(phase: u32, dt: f32);
type ShutdownFn = unsafe extern "C" fn();

static MODULE: AtomicUsize = AtomicUsize::new(0);
static DEAD: AtomicBool = AtomicBool::new(false);
/// The payload's `kh_er_frame`, 0 while there is none.
static FRAME_FN: AtomicUsize = AtomicUsize::new(0);
static SHUTDOWN_FN: AtomicUsize = AtomicUsize::new(0);
static GENERATION: AtomicU32 = AtomicU32::new(0);

struct Loader {
    dir: PathBuf,
    stamp: Option<(SystemTime, u64)>,
    frames: u32,
}

static LOADER: Mutex<Option<Loader>> = Mutex::new(None);
static LOG: Mutex<Option<(std::fs::File, Instant)>> = Mutex::new(None);

fn log(text: &str) {
    if let Ok(mut guard) = LOG.lock() {
        if let Some((file, start)) = guard.as_mut() {
            let _ = writeln!(file, "[{:9.3}] {text}", start.elapsed().as_secs_f64());
            let _ = file.flush();
        }
    }
}

#[no_mangle]
/// # Safety
/// Called by the Windows loader only.
pub unsafe extern "system" fn DllMain(module: *mut c_void, reason: u32, _reserved: *mut c_void) -> i32 {
    const DLL_PROCESS_ATTACH: u32 = 1;
    if reason == DLL_PROCESS_ATTACH {
        MODULE.store(module as usize, Ordering::SeqCst);
        // Nothing but a thread start under the loader lock.
        std::thread::spawn(|| {
            if catch_unwind(main_thread).is_err() {
                DEAD.store(true, Ordering::SeqCst);
                log("PANIC in the start thread; the loader is inert");
                tell_player("the loader failed while starting (see logs/kh-er-loader.log)");
            }
        });
    }
    1
}

/// THE MOD DID NOT START AND THE GAME RUNS WITHOUT IT: a log line is not visible to the player, so
/// he is told: three falling beeps, a file `logsMOD-NOT-STARTED.txt`, and a message box on top of
/// the game (from a thread of its own: nothing of the game waits for it).
fn tell_player(why: &str) {
    log(&format!("TELLING THE PLAYER: the mod did not start: {why}"));
    let dir = dll_dir();
    let root = dir.parent().map(|p| p.to_path_buf()).unwrap_or(dir);
    let _ = root;
    let _ = std::fs::write(logs_dir().join("MOD-NOT-STARTED.txt"), format!("Sora (kh-er) did not start in this run of Elden Ring: {why}
Close the game and start it again with play-er.bat.
"));
    let text: Vec<u16> = format!("Sora did NOT start in this run: {why}.

Elden Ring is running WITHOUT the mod. Close the game and start it again (play-er.bat).").encode_utf16().chain(std::iter::once(0)).collect();
    std::thread::spawn(move || {
        use windows_sys::Win32::System::Diagnostics::Debug::Beep;
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONWARNING, MB_OK, MB_SETFOREGROUND, MB_TOPMOST};
        let title: Vec<u16> = "Kingdom Hearts III Sora in Elden Ring".encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: plain Win32 calls with NUL-terminated strings that live through the call.
        unsafe {
            for hz in [880u32, 660, 440] {
                Beep(hz, 220);
            }
            MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONWARNING | MB_TOPMOST | MB_SETFOREGROUND);
        }
    });
}

/// (IMPL_ER18) THE USER'S OWN LOG FOLDER IS er\\logs AND ONLY THE PROFILE IN er\\mods WRITES IT: a loader that runs from another
/// folder (mods_test) logs to the folder of the same suffix (logs_test), as the payload does since IMPL_ER14.
fn logs_dir() -> PathBuf {
    let dir = dll_dir();
    let root = dir.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| dir.clone());
    let folder = dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    match folder.strip_prefix("mods") {
        Some(suffix) if !suffix.is_empty() => root.join(format!("logs{suffix}")),
        _ => root.join("logs"),
    }
}

fn dll_dir() -> PathBuf {
    let mut buffer = [0u16; 1024];
    // SAFETY: the buffer is valid for its length; the handle is the one DllMain was given.
    let n = unsafe { GetModuleFileNameW(MODULE.load(Ordering::SeqCst) as _, buffer.as_mut_ptr(), buffer.len() as u32) };
    let path = PathBuf::from(String::from_utf16_lossy(&buffer[..n as usize]));
    path.parent().map(|p| p.to_path_buf()).unwrap_or_default()
}

fn exe_version() -> Option<(u16, u16, u16, u16)> {
    // SAFETY: a null name gives the handle of the process's executable, which stays mapped.
    let base = unsafe { GetModuleHandleW(std::ptr::null()) } as *const u8;
    if base.is_null() {
        return None;
    }
    // SAFETY: `base` is the base of a mapped PE image.
    let view = unsafe { PeView::module(base) };
    let info = view.resources().ok()?.version_info().ok()?;
    let v = info.fixed()?.dwFileVersion;
    Some((v.Major, v.Minor, v.Patch, v.Build))
}

fn main_thread() {
    let dir = dll_dir();
    let root = dir.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| dir.clone());
    let logs = logs_dir();
    let _ = std::fs::create_dir_all(&logs);
    if let Ok(file) = OpenOptions::new().create(true).write(true).truncate(true).open(logs.join("kh-er-loader.log")) {
        *LOG.lock().unwrap() = Some((file, Instant::now()));
    }
    std::panic::set_hook(Box::new(|info| log(&format!("panic: {info}"))));
    log(&format!("kh-er-loader {} in {}", env!("CARGO_PKG_VERSION"), dir.display()));
    let _ = std::fs::remove_file(logs.join("MOD-NOT-STARTED.txt"));

    match exe_version() {
        Some(v) if SUPPORTED_EXE_VERSIONS.contains(&v) => log(&format!("executable version {}.{}.{}.{}: supported (PORT)", v.0, v.1, v.2, v.3)),
        other => {
            log(&format!("executable version {other:?} is not 2.7.1.0 or 2.7.2.0: nothing is done in this process"));
            tell_player(&format!("this is Elden Ring executable version {other:?}; the mod is built for 2.7.1.0 and 2.7.2.0 only"));
            return;
        }
    }

    // Old copies of earlier runs.
    let live = dir.join("live");
    let _ = std::fs::remove_dir_all(&live);
    let _ = std::fs::create_dir_all(&live);

    // (MEASURED 2026-10-12, twice in ten launches, with the same loader file: the SDK answers
    // "InvalidRva" 15 - 20 ms after the DLL is loaded when the game has not registered its task
    // system yet, instead of waiting; the game then ran WITHOUT the mod. It is asked again for two
    // minutes before giving up.)
    // (WHY THE RETRY WORKS, read in the SDK's sources: InvalidRva here is the singleton lookup by
    // name answering "not found" while the game's reflection data is not registered yet; the lookup
    // table is only kept for good once some singleton exists, so asking again later looks afresh.
    // KH_ER_TEST_RETRY = n pretends the first n answers were that error: the loop itself was run so.)
    let asked = Instant::now();
    let mut told = false;
    let mut pretend = std::env::var("KH_ER_TEST_RETRY").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
    let mut tries = 0u32;
    let cs_task = loop {
        tries += 1;
        let answer = if pretend > 0 {
            pretend -= 1;
            Err(eldenring::util::system::SystemInitError::InvalidRva)
        } else {
            CSTaskImp::wait_for_instance(Duration::from_secs(300))
        };
        match answer {
            Ok(task) => {
                if told {
                    log(&format!("the task system appeared at try {tries}, {:.2} s after the first: THE RETRY WORKED", asked.elapsed().as_secs_f32()));
                }
                break task;
            }
            Err(e) if asked.elapsed() < Duration::from_secs(120) => {
                if !told {
                    log(&format!("the task system of the game is not there yet ({e:?}): asking again for up to two minutes"));
                    told = true;
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            Err(e) => {
                log(&format!("the task system of the game did not appear: {e:?}; nothing is done"));
                tell_player("the game's task system did not appear within two minutes");
                return;
            }
        }
    };
    log("task system found");
    *LOADER.lock().unwrap() = Some(Loader { dir, stamp: None, frames: 0 });

    for &group in PHASES {
        let phase = group as u32;
        let handle = cs_task.run_recurring(
            move |data: &FD4TaskData| {
                if DEAD.load(Ordering::Relaxed) {
                    return;
                }
                let dt = data.delta_time.time;
                if catch_unwind(AssertUnwindSafe(|| dispatch(phase, dt))).is_err() {
                    DEAD.store(true, Ordering::SeqCst);
                    log("PANIC in the loader's dispatch; the loader is inert");
                }
            },
            group,
        );
        // The handle of the SDK cancels on drop: keep the task for the life of the process.
        std::mem::forget(handle);
    }
    log(&format!("{} tasks registered", PHASES.len()));
}

fn dispatch(phase: u32, dt: f32) {
    if phase == CSTaskGroupIndex::FrameBegin as u32 {
        if let Ok(mut guard) = LOADER.lock() {
            if let Some(loader) = guard.as_mut() {
                loader.frames += 1;
                if loader.frames % 20 == 1 {
                    reload_if_changed(loader);
                }
            }
        }
    }
    let frame = FRAME_FN.load(Ordering::Acquire);
    if frame != 0 {
        // SAFETY: the address came from GetProcAddress on a payload that is never unloaded.
        let frame: FrameFn = unsafe { std::mem::transmute(frame) };
        unsafe { frame(phase, dt) };
    }
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}

fn reload_if_changed(loader: &mut Loader) {
    let source = loader.dir.join("kh_er.dll");
    let Ok(meta) = std::fs::metadata(&source) else { return };
    let stamp = (meta.modified().unwrap_or(SystemTime::UNIX_EPOCH), meta.len());
    if loader.stamp == Some(stamp) {
        return;
    }
    // A file still being written: wait until it is two seconds old.
    if stamp.0.elapsed().map(|e| e < Duration::from_secs(2)).unwrap_or(false) && loader.stamp.is_some() {
        return;
    }
    let generation = GENERATION.load(Ordering::SeqCst);
    let copy = loader.dir.join("live").join(format!("kh_er_{generation}.dll"));
    if let Err(e) = std::fs::copy(&source, &copy) {
        log(&format!("cannot copy {}: {e} (will try again)", source.display()));
        return;
    }
    loader.stamp = Some(stamp);

    // The old payload first: it puts back what it changed.
    let old_shutdown = SHUTDOWN_FN.swap(0, Ordering::AcqRel);
    FRAME_FN.store(0, Ordering::Release);
    if old_shutdown != 0 {
        // SAFETY: as in `dispatch`.
        let shutdown: ShutdownFn = unsafe { std::mem::transmute(old_shutdown) };
        unsafe { shutdown() };
    }

    let name = wide(&copy);
    // SAFETY: `name` is a NUL-terminated wide string.
    let module = unsafe { LoadLibraryW(name.as_ptr()) };
    if module.is_null() {
        log(&format!("LoadLibrary failed for {}", copy.display()));
        return;
    }
    // SAFETY: valid module handle, NUL-terminated names.
    let (init, frame, shutdown) = unsafe {
        (
            GetProcAddress(module, b"kh_er_init\0".as_ptr()),
            GetProcAddress(module, b"kh_er_frame\0".as_ptr()),
            GetProcAddress(module, b"kh_er_shutdown\0".as_ptr()),
        )
    };
    let (Some(init), Some(frame)) = (init, frame) else {
        log(&format!("{} lacks kh_er_init / kh_er_frame", copy.display()));
        return;
    };
    let dir = wide(&loader.dir);
    // SAFETY: the payload's documented entry: a wide string and its length without the NUL.
    let init: InitFn = unsafe { std::mem::transmute(init) };
    let answer = unsafe { init(dir.as_ptr(), dir.len() - 1, generation) };
    if answer != 1 {
        log(&format!("payload {generation} refused to start (answer {answer})"));
        if generation == 0 {
            tell_player("kh_er.dll refused to start (see logs/kh-er.log)");
        }
        return;
    }
    SHUTDOWN_FN.store(shutdown.map(|f| f as usize).unwrap_or(0), Ordering::Release);
    FRAME_FN.store(frame as usize, Ordering::Release);
    GENERATION.store(generation + 1, Ordering::SeqCst);
    log(&format!("payload {generation} loaded from {} ({} bytes)", copy.display(), stamp.1));
}
