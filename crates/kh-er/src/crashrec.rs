//! (CONT-CRASH, 2026-11-10) THE CRASH RECORDER: when an exception reaches the top of a thread with nobody to take it (the
//! process is about to die), its place is written to the log and a minidump is written beside it, then the exception goes
//! on to whoever was there before (the game's own filter, me3's, Windows Error Reporting) EXACTLY as without us.
//!
//! - ONLY UNHANDLED EXCEPTIONS: it is the process's top-level filter (`SetUnhandledExceptionFilter`), which Windows asks
//!   only after every handler on the stack declined. First-chance exceptions the game handles itself never reach it.
//! - THE LOG (`kh-er.log`, our own append handle, opened at install): `!!! crash:` lines with the exception code, the
//!   faulting address as module+offset, the access (read / write / execute, and of what), the thread id, the registers, a
//!   stack walk (the unwinder's frames, module+offset) and the code addresses found in the top of the stack. Module names
//!   from the loaded images themselves (never a folder: no personal information). Written from fixed buffers: no
//!   allocation, no lock, no loader lock in the faulting thread.
//! - THE MINIDUMP (`kh-er-crash-<date>-<time>.dmp` in the log folder): written by a thread made at install (the faulting
//!   thread waits for it, at most [`DUMP_WAIT_MS`]) with dbghelp's MiniDumpWriteDump, small: threads and stacks, modules,
//!   thread info, unloaded modules and the memory the stacks point at. Then the faulting thread's name (its description)
//!   is logged. A dump holds memory of the game (and the module paths): send it only to whom you mean to. A crash inside
//!   one of ntdll's own locks (the 2026-11-10 crash was one: RtlDeleteCriticalSection) stops an in-process dump for good:
//!   then the exception goes on after [`DUMP_WAIT_MS`] without it (the lines are written before; Windows' own dump, where
//!   Windows Error Reporting keeps them, still comes).
//! - ONCE A PROCESS: the first fatal exception is recorded; any other (another thread, one inside a filter) only passes.
//! - LINUX / PROTON (Wine): the log lines are written the same way (plain file writes); NO minidump is attempted there.
//! - KEPT FIRST: every 10 s the frame looks whether someone replaced the filter since; if so ours goes back on top and
//!   theirs is called after it (a filter that calls ours back is answered "not mine": never a loop).
//! - A PAYLOAD THAT GOES (the loader's reload) puts the previous filter back and ends its thread ([`uninstall`]).

use std::cell::UnsafeCell;
use std::fmt::Write as _;
use std::io::Write as _;
use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL};
use windows_sys::Win32::System::Diagnostics::Debug::{
    MiniDumpWithIndirectlyReferencedMemory, MiniDumpWithThreadInfo, MiniDumpWithUnloadedModules, RtlLookupFunctionEntry, RtlPcToFileHeader, RtlVirtualUnwind, SetUnhandledExceptionFilter, CONTEXT, EXCEPTION_POINTERS, MINIDUMP_EXCEPTION_INFORMATION, MINIDUMP_TYPE, UNW_FLAG_NHANDLER,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32};
use windows_sys::Win32::System::SystemInformation::GetLocalTime;
use windows_sys::Win32::System::Threading::{CreateEventW, GetCurrentProcess, GetCurrentProcessId, GetCurrentThreadId, GetCurrentThreadStackLimits, OpenThread, SetEvent, WaitForSingleObject, THREAD_QUERY_LIMITED_INFORMATION};

/// How long the faulting thread waits for the dump (ms) before it lets the exception go on regardless. Short: a crash
/// inside a lock of ntdll's (the user's own crash: RtlDeleteCriticalSection holds its list lock) stops dbghelp for good
/// (measured: `the_old_press_write_is_the_users_crash`); the log lines are written before, and Windows' own dump still comes.
pub const DUMP_WAIT_MS: u32 = 5_000;
/// Frames of the stack walk.
const FRAMES: usize = 48;
/// Code addresses listed from the top of the stack, and how far it is looked through (bytes).
const SCAN_HITS: usize = 32;
const SCAN_BYTES: usize = 0x1800;
/// STATUS_STACK_OVERFLOW: the walk and the scan are skipped (the faulting stack has almost no room).
const STACK_OVERFLOW: u32 = 0xC000_00FD;
const ACCESS_VIOLATION: u32 = 0xC000_0005;
const IN_PAGE_ERROR: u32 = 0xC000_0006;
const EXCEPTION_CONTINUE_SEARCH: i32 = 0;
const EXCEPTION_CONTINUE_EXECUTION: i32 = -1;
/// The kinds of dump asked for: small, with what names the place (thread info, unloaded modules, memory the stacks point at).
pub const DUMP_TYPE: MINIDUMP_TYPE = MiniDumpWithThreadInfo | MiniDumpWithUnloadedModules | MiniDumpWithIndirectlyReferencedMemory;

/// A static only the recording thread touches (the [`OWNER`] claim makes it one thread).
struct Owned<T>(UnsafeCell<T>);
// SAFETY: only the thread that won [`OWNER`] touches the contents (the watcher has buffers of its own).
unsafe impl<T> Sync for Owned<T> {}

/// CONTEXT with the alignment the unwinder wants.
#[repr(C, align(16))]
struct Ctx(CONTEXT);

/// Text into a fixed buffer: a line too long is cut, never an error, never an allocation.
struct Line<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> Line<N> {
    const fn new() -> Self {
        Line { buf: [0; N], len: 0 }
    }

    fn text(&self) -> &str {
        match std::str::from_utf8(&self.buf[..self.len]) {
            Ok(t) => t,
            // a cut inside a character: up to the last whole one
            Err(e) => std::str::from_utf8(&self.buf[..e.valid_up_to()]).unwrap_or(""),
        }
    }
}

impl<const N: usize> std::fmt::Write for Line<N> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let n = s.len().min(N - self.len);
        self.buf[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

/// The installed filter (ours) holds the top.
static INSTALLED: AtomicBool = AtomicBool::new(false);
/// The filter that was there before ours (a function pointer; 0 = none).
static PREV: AtomicUsize = AtomicUsize::new(0);
/// The thread recording (0 = nobody yet; once set it stays: one record a process).
static OWNER: AtomicU32 = AtomicU32::new(0);
/// The thread inside our filter's chain call (a filter calling ours back is answered "not mine").
static CHAINING: AtomicU32 = AtomicU32::new(0);
/// Under Wine / Proton: no minidump.
static WINE: AtomicBool = AtomicBool::new(false);
/// dbghelp's MiniDumpWriteDump (0 = not found: the log lines only).
static MINIDUMP: AtomicUsize = AtomicUsize::new(0);
/// The watcher's events: wake (a crash or the stop), done (the dump is written).
static WAKE: AtomicUsize = AtomicUsize::new(0);
static DONE: AtomicUsize = AtomicUsize::new(0);
static STOP: AtomicBool = AtomicBool::new(false);
static WATCHING: AtomicBool = AtomicBool::new(false);
/// The crash handed to the watcher: the exception pointers and the faulting thread.
static EXC: AtomicUsize = AtomicUsize::new(0);
static EXC_TID: AtomicU32 = AtomicU32::new(0);
/// Our own handle to the log (append) and the log folder (UTF-16 with its last `\`, no terminator).
static LOG: OnceLock<std::fs::File> = OnceLock::new();
static DIR_W: OnceLock<Vec<u16>> = OnceLock::new();
/// The executable's file name (eldenring.exe in the game), read at install.
static EXE_NAME: OnceLock<String> = OnceLock::new();
static WATCHER: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);

static LINE: Owned<Line<1024>> = Owned(UnsafeCell::new(Line::new()));
// SAFETY (zeroed): CONTEXT is plain data; all zero is a valid value.
static WALK: Owned<Ctx> = Owned(UnsafeCell::new(Ctx(unsafe { std::mem::zeroed() })));

type MiniDumpWriteDumpFn = unsafe extern "system" fn(HANDLE, u32, HANDLE, MINIDUMP_TYPE, *const MINIDUMP_EXCEPTION_INFORMATION, *const core::ffi::c_void, *const core::ffi::c_void) -> i32;
type Filter = unsafe extern "system" fn(*const EXCEPTION_POINTERS) -> i32;
type GetThreadDescriptionFn = unsafe extern "system" fn(HANDLE, *mut *mut u16) -> i32;

// ------------------------------------------------------------------------------------------ pure parts

/// The usual name of an exception code (pure).
pub fn code_name(code: u32) -> &'static str {
    match code {
        ACCESS_VIOLATION => "access violation",
        IN_PAGE_ERROR => "in-page error",
        STACK_OVERFLOW => "stack overflow",
        0xC000_0374 => "heap corruption",
        0xC000_0409 => "fail-fast (stack buffer overrun / __fastfail)",
        0xC000_001D => "illegal instruction",
        0xC000_0094 => "integer divide by zero",
        0xC000_0096 => "privileged instruction",
        0xC000_008C => "array bounds exceeded",
        0xC000_0420 => "assertion failure",
        0x8000_0003 => "breakpoint (int3)",
        0x8000_0002 => "datatype misalignment",
        0xC000_0008 => "invalid handle",
        0xE06D_7363 => "C++ exception",
        _ => "other",
    }
}

/// What an access violation did (pure): its first parameter.
pub fn access_kind(param0: usize) -> &'static str {
    match param0 {
        0 => "READ of",
        1 => "WRITE to",
        8 => "EXECUTE of (DEP)",
        _ => "access to",
    }
}

/// The dump's file name for a local time (pure, no allocation): `kh-er-crash-YYYYMMDD-HHMMSS.dmp`.
fn write_dump_name(out: &mut impl std::fmt::Write, y: u16, mo: u16, d: u16, h: u16, mi: u16, s: u16) {
    let _ = write!(out, "kh-er-crash-{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}.dmp");
}

// ------------------------------------------------------------------------------------------ install / uninstall

/// (entry.rs, right after the log) The recorder is put in place: the log handle, dbghelp, the watcher, the filter.
pub fn install() {
    if INSTALLED.load(Ordering::SeqCst) {
        return;
    }
    let dir = crate::log::dir();
    if !dir.is_absolute() {
        return;
    }
    if let Ok(file) = std::fs::OpenOptions::new().append(true).open(dir.join("kh-er.log")) {
        let _ = LOG.set(file);
    }
    let mut dir_w: Vec<u16> = dir.as_os_str().encode_wide().collect();
    if dir_w.last().is_some_and(|&c| c != u16::from(b'\\')) {
        dir_w.push(u16::from(b'\\'));
    }
    let _ = DIR_W.set(dir_w);
    let _ = EXE_NAME.set(std::env::current_exe().ok().and_then(|e| e.file_name().map(|n| n.to_string_lossy().into_owned())).unwrap_or_else(|| "exe".to_string()));
    // SAFETY: plain module lookups by constant names.
    let wine = unsafe {
        let ntdll = GetModuleHandleW(wide("ntdll.dll").as_ptr());
        !ntdll.is_null() && GetProcAddress(ntdll, c"wine_get_version".as_ptr().cast()).is_some()
    };
    WINE.store(wine, Ordering::SeqCst);
    if !wine {
        // SAFETY: the system's dbghelp, from System32 only; the export is looked up by its documented name.
        unsafe {
            let dbghelp = LoadLibraryExW(wide("dbghelp.dll").as_ptr(), std::ptr::null_mut(), LOAD_LIBRARY_SEARCH_SYSTEM32);
            if !dbghelp.is_null() {
                if let Some(f) = GetProcAddress(dbghelp, c"MiniDumpWriteDump".as_ptr().cast()) {
                    MINIDUMP.store(f as usize, Ordering::SeqCst);
                }
            }
        }
    }
    // SAFETY: two unnamed auto-reset events.
    let (wake, done) = unsafe { (CreateEventW(std::ptr::null(), 0, 0, std::ptr::null()), CreateEventW(std::ptr::null(), 0, 0, std::ptr::null())) };
    if !wake.is_null() && !done.is_null() {
        WAKE.store(wake as usize, Ordering::SeqCst);
        DONE.store(done as usize, Ordering::SeqCst);
        STOP.store(false, Ordering::SeqCst);
        match std::thread::Builder::new().name("kh-crash-recorder".into()).stack_size(256 * 1024).spawn(watcher) {
            Ok(h) => {
                WATCHING.store(true, Ordering::SeqCst);
                if let Ok(mut w) = WATCHER.lock() {
                    *w = Some(h);
                }
            }
            Err(e) => crate::erlog!("crash recorder: no dump thread ({e}): the log lines only"),
        }
    }
    // SAFETY: our filter is a plain extern "system" function of this payload; uninstall puts the previous one back.
    let prev = unsafe { SetUnhandledExceptionFilter(Some(filter)) };
    PREV.store(prev.map_or(0, |f| f as usize), Ordering::SeqCst);
    INSTALLED.store(true, Ordering::SeqCst);
    let dumps = if wine {
        " (Wine / Proton: NO minidump, the lines only)"
    } else if MINIDUMP.load(Ordering::SeqCst) == 0 || !WATCHING.load(Ordering::SeqCst) {
        " (NO minidump: dbghelp or the dump thread missing; the lines only)"
    } else {
        ""
    };
    crate::erlog!("crash recorder: on: an exception nobody takes is written here (\"!!! crash:\" lines) and as a minidump kh-er-crash-<date>-<time>.dmp in this folder{dumps}; the filter before ours: {}", describe_owned(PREV.load(Ordering::SeqCst)));
}

/// (entry.rs `kh_er_shutdown`, the payload goes) The previous filter back (if ours is still on top), the watcher ended.
pub fn uninstall() {
    if !INSTALLED.swap(false, Ordering::SeqCst) {
        return;
    }
    // SAFETY: the value found is put back unless it was ours (then the previous one takes the top again).
    unsafe {
        let current = SetUnhandledExceptionFilter(as_filter(PREV.load(Ordering::SeqCst)));
        if current.map_or(0, |f| f as usize) != filter as Filter as usize {
            SetUnhandledExceptionFilter(current);
        }
    }
    STOP.store(true, Ordering::SeqCst);
    let wake = WAKE.load(Ordering::SeqCst);
    if wake != 0 {
        // SAFETY: our own event.
        unsafe { SetEvent(wake as HANDLE) };
    }
    if let Some(h) = WATCHER.lock().ok().and_then(|mut w| w.take()) {
        let _ = h.join();
    }
    WATCHING.store(false, Ordering::SeqCst);
    for ev in [WAKE.swap(0, Ordering::SeqCst), DONE.swap(0, Ordering::SeqCst)] {
        if ev != 0 {
            // SAFETY: our own events, no longer waited on.
            unsafe { CloseHandle(ev as HANDLE) };
        }
    }
}

/// (frame.rs, at each second's tick) Every 10 s: still the top filter? If another was set since, ours goes back on top and
/// that one is called after it.
pub fn frame(tick: bool) {
    static SECONDS: AtomicU32 = AtomicU32::new(0);
    if !tick || !INSTALLED.load(Ordering::Relaxed) || SECONDS.fetch_add(1, Ordering::Relaxed) % 10 != 9 {
        return;
    }
    // SAFETY: ours set again; the one found is kept as the next in the chain.
    let current = unsafe { SetUnhandledExceptionFilter(Some(filter)) }.map_or(0, |f| f as usize);
    if current != filter as Filter as usize {
        PREV.store(current, Ordering::SeqCst);
        crate::erlog!("crash recorder: the top-level filter had been replaced ({}): ours is first again, that one is called after it", describe_owned(current));
    }
}

fn as_filter(p: usize) -> Option<Filter> {
    // SAFETY: a value SetUnhandledExceptionFilter returned (a filter function) or 0.
    (p != 0).then(|| unsafe { std::mem::transmute::<usize, Filter>(p) })
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// module+offset of an address (allocating: for lines outside the filter).
fn describe_owned(addr: usize) -> String {
    if addr == 0 {
        return "none".to_string();
    }
    let mut l = Line::<160>::new();
    where_(&mut l, addr);
    l.text().to_string()
}

/// The text of a line with the log's time prefix, written to our log handle (no allocation).
fn emit<const N: usize>(l: &mut Line<N>, args: std::fmt::Arguments) {
    l.len = 0;
    let _ = write!(l, "[{:9.3}] ", crate::log::elapsed_s());
    let _ = l.write_fmt(args);
    if l.len == N {
        l.len -= 1;
    }
    l.buf[l.len] = b'\n';
    l.len += 1;
    if let Some(mut f) = LOG.get() {
        let _ = f.write_all(&l.buf[..l.len]);
    }
}

/// One line from the recording thread's buffer.
///
/// # Safety
/// Only the thread that won [`OWNER`] may call it.
unsafe fn line(args: std::fmt::Arguments) {
    // SAFETY: the caller owns the buffer.
    emit(unsafe { &mut *LINE.0.get() }, args);
}

// ------------------------------------------------------------------------------------------ the filter

unsafe extern "system" fn filter(info: *const EXCEPTION_POINTERS) -> i32 {
    // SAFETY: Windows' own thread id call.
    let me = unsafe { GetCurrentThreadId() };
    if CHAINING.load(Ordering::SeqCst) == me {
        return EXCEPTION_CONTINUE_SEARCH; // a filter after ours called ours back: not ours to answer twice
    }
    let owner = OWNER.compare_exchange(0, me, Ordering::SeqCst, Ordering::SeqCst).is_ok();
    if owner && !info.is_null() {
        // SAFETY: the pointers Windows handed to the filter; this thread owns the static buffers now.
        unsafe { record(info, me) };
    }
    let Some(prev) = as_filter(PREV.load(Ordering::SeqCst)) else { return EXCEPTION_CONTINUE_SEARCH };
    let chaining = CHAINING.compare_exchange(0, me, Ordering::SeqCst, Ordering::SeqCst).is_ok();
    // SAFETY: the filter that was there before ours, with the same pointers.
    let answer = unsafe { prev(info) };
    if chaining {
        CHAINING.store(0, Ordering::SeqCst);
    }
    if answer == EXCEPTION_CONTINUE_EXECUTION && owner {
        // SAFETY: this thread owns the buffers.
        unsafe { line(format_args!("!!! crash: (the filter before ours took it back: the game goes on)")) };
    }
    answer
}

/// module+offset of an address into a line (lock-free: the loaded image's own header and export name).
fn where_<const N: usize>(l: &mut Line<N>, addr: usize) {
    let mut base: *mut core::ffi::c_void = std::ptr::null_mut();
    // SAFETY: Windows' lookup of the image holding an address (no loader lock, the address is not followed).
    unsafe { RtlPcToFileHeader(addr as *const core::ffi::c_void, &mut base) };
    if base.is_null() {
        let _ = write!(l, "0x{addr:x}");
        return;
    }
    let base = base as usize;
    // SAFETY: the process's own executable handle.
    let exe = unsafe { GetModuleHandleW(std::ptr::null()) } as usize;
    if base == exe {
        let _ = write!(l, "{}+0x{:x}", EXE_NAME.get().map_or("exe", String::as_str), addr - base);
        return;
    }
    // SAFETY: `base` is a mapped image (Windows just said so); its headers are read within its own size.
    match unsafe { export_name(base) } {
        Some(name) => {
            for &b in name {
                let _ = l.write_char(char::from(b));
            }
            let _ = write!(l, "+0x{:x}", addr - base);
        }
        None => {
            let _ = write!(l, "0x{base:x}+0x{:x}", addr - base);
        }
    }
}

/// The export directory's DLL name of a mapped image (its own name, e.g. "ntdll.dll"; at most 64 printable bytes).
///
/// # Safety
/// `base` must be the base of a mapped PE image.
unsafe fn export_name(base: usize) -> Option<&'static [u8]> {
    // SAFETY: the DOS / NT headers of a mapped image; every offset is checked against the image's size.
    unsafe {
        if (base as *const u16).read_unaligned() != 0x5A4D {
            return None;
        }
        let nt = base + (((base + 0x3C) as *const u32).read_unaligned() as usize);
        if (nt as *const u32).read_unaligned() != 0x4550 {
            return None;
        }
        let opt = nt + 24;
        if (opt as *const u16).read_unaligned() != 0x20B {
            return None; // PE32+ only
        }
        let size = ((opt + 56) as *const u32).read_unaligned() as usize; // SizeOfImage
        let export_rva = ((opt + 112) as *const u32).read_unaligned() as usize; // DataDirectory[0]
        if export_rva == 0 || export_rva + 40 > size {
            return None;
        }
        let name_rva = ((base + export_rva + 12) as *const u32).read_unaligned() as usize;
        if name_rva == 0 || name_rva >= size {
            return None;
        }
        let p = (base + name_rva) as *const u8;
        let mut n = 0;
        while n < 64 && name_rva + n < size {
            let c = p.add(n).read();
            if c == 0 {
                break;
            }
            if !c.is_ascii_graphic() {
                return None;
            }
            n += 1;
        }
        (n > 0).then(|| std::slice::from_raw_parts(p, n))
    }
}

/// The record (the owning thread, inside the filter).
///
/// # Safety
/// `info` are the pointers Windows handed to the filter; the caller won [`OWNER`].
unsafe fn record(info: *const EXCEPTION_POINTERS, tid: u32) {
    // SAFETY: Windows' own exception record and context of this thread.
    let (rec, ctx) = unsafe { ((*info).ExceptionRecord, (*info).ContextRecord) };
    if rec.is_null() || ctx.is_null() {
        // SAFETY: owned buffer.
        unsafe { line(format_args!("!!! crash: AN EXCEPTION NOBODY TOOK on thread {tid} (no record given)")) };
        return;
    }
    // SAFETY: as above.
    let (rec, ctx) = unsafe { (&*rec, &*ctx) };
    let code = rec.ExceptionCode as u32;
    let mut place = Line::<128>::new();
    where_(&mut place, rec.ExceptionAddress as usize);
    let mut access = Line::<96>::new();
    if (code == ACCESS_VIOLATION || code == IN_PAGE_ERROR) && rec.NumberParameters >= 2 {
        let _ = write!(access, "; {} 0x{:x}", access_kind(rec.ExceptionInformation[0]), rec.ExceptionInformation[1]);
    }
    // SAFETY: owned buffers (this thread won OWNER); `walk` / `scan` run on this, the faulting, thread.
    unsafe {
        line(format_args!("!!! crash: AN EXCEPTION NOBODY TOOK: code 0x{code:08X} ({}) at {} on thread {tid}{}; the game's filter / Windows get it next (kh-er's crash recorder)", code_name(code), place.text(), access.text()));
        line(format_args!(
            "!!! crash: registers rip {:x} rsp {:x} rbp {:x} rax {:x} rbx {:x} rcx {:x} rdx {:x} rsi {:x} rdi {:x} r8 {:x} r9 {:x} r10 {:x} r11 {:x} r12 {:x} r13 {:x} r14 {:x} r15 {:x}",
            ctx.Rip, ctx.Rsp, ctx.Rbp, ctx.Rax, ctx.Rbx, ctx.Rcx, ctx.Rdx, ctx.Rsi, ctx.Rdi, ctx.R8, ctx.R9, ctx.R10, ctx.R11, ctx.R12, ctx.R13, ctx.R14, ctx.R15
        ));
        if code == STACK_OVERFLOW {
            line(format_args!("!!! crash: (a stack overflow: no walk on the faulting stack; the dump has it)"));
        } else {
            walk(ctx);
            scan(ctx.Rsp as usize);
        }
    }
    hand_to_watcher(info, tid);
}

/// This thread's stack bounds.
fn stack_bounds() -> (usize, usize) {
    let (mut low, mut high) = (0usize, 0usize);
    // SAFETY: two out-parameters.
    unsafe { GetCurrentThreadStackLimits(&mut low, &mut high) };
    (low, high)
}

/// The stack walk by the unwinder from the faulting context, 8 frames a line.
///
/// # Safety
/// The caller won [`OWNER`]; `ctx` is the context of this, the faulting, thread.
unsafe fn walk(ctx: &CONTEXT) {
    let (low, high) = stack_bounds();
    // SAFETY: the owned CONTEXT copy.
    let c = unsafe { &mut (*WALK.0.get()).0 };
    *c = *ctx;
    let mut frames = [0usize; FRAMES];
    let mut n = 0;
    while n < FRAMES && c.Rip != 0 {
        frames[n] = c.Rip as usize;
        n += 1;
        let sp_before = c.Rsp as usize;
        let mut image: u64 = 0;
        // SAFETY: Windows' function-table lookup for a pc.
        let entry = unsafe { RtlLookupFunctionEntry(c.Rip, &mut image, std::ptr::null_mut()) };
        if entry.is_null() {
            // a leaf: its return address is at the top of the stack
            if sp_before < low || sp_before + 8 > high {
                break;
            }
            // SAFETY: inside this thread's committed stack (checked just above).
            c.Rip = unsafe { (sp_before as *const u64).read_unaligned() };
            c.Rsp += 8;
        } else {
            let mut handler_data: *mut core::ffi::c_void = std::ptr::null_mut();
            let mut establisher: u64 = 0;
            // SAFETY: the unwinder on our own copy of the context, with the entry it found.
            unsafe { RtlVirtualUnwind(UNW_FLAG_NHANDLER, image, c.Rip, entry, c, &mut handler_data, &mut establisher, std::ptr::null_mut()) };
        }
        let sp = c.Rsp as usize;
        if sp <= sp_before || sp < low || sp >= high {
            break;
        }
    }
    if n == 0 {
        // SAFETY: owned buffer.
        unsafe { line(format_args!("!!! crash: stack walk: no frame")) };
    }
    for (chunk_no, chunk) in frames[..n].chunks(8).enumerate() {
        let mut l = Line::<640>::new();
        for (k, &pc) in chunk.iter().enumerate() {
            let _ = write!(l, "{}#{} ", if k > 0 { " | " } else { "" }, chunk_no * 8 + k);
            where_(&mut l, pc);
        }
        // SAFETY: owned buffer.
        unsafe { line(format_args!("!!! crash: stack walk {}", l.text())) };
    }
}

/// The code addresses in the top of the stack (what a walk through the game's obfuscated code can miss), 8 a line.
///
/// # Safety
/// The caller won [`OWNER`]; this is the faulting thread.
unsafe fn scan(rsp: usize) {
    let (low, high) = stack_bounds();
    if rsp < low || rsp >= high {
        return;
    }
    let end = high.min(rsp + SCAN_BYTES);
    let mut hits = [(0usize, 0usize); SCAN_HITS];
    let mut n = 0;
    let mut a = rsp & !7;
    while a + 8 <= end && n < SCAN_HITS {
        // SAFETY: inside this thread's committed stack.
        let v = unsafe { (a as *const usize).read() };
        let mut base: *mut core::ffi::c_void = std::ptr::null_mut();
        if v > 0x10000 {
            // SAFETY: Windows' lookup of the image holding a value (the value is not followed).
            unsafe { RtlPcToFileHeader(v as *const core::ffi::c_void, &mut base) };
        }
        if !base.is_null() {
            hits[n] = (a - rsp, v);
            n += 1;
        }
        a += 8;
    }
    for chunk in hits[..n].chunks(8) {
        let mut l = Line::<640>::new();
        for (k, &(off, v)) in chunk.iter().enumerate() {
            let _ = write!(l, "{}[rsp+0x{off:x}] ", if k > 0 { " | " } else { "" });
            where_(&mut l, v);
        }
        // SAFETY: owned buffer.
        unsafe { line(format_args!("!!! crash: code addresses on the stack {}", l.text())) };
    }
}

/// The dump by the watcher; the faulting thread waits for it (bounded).
fn hand_to_watcher(info: *const EXCEPTION_POINTERS, tid: u32) {
    let (wake, done) = (WAKE.load(Ordering::SeqCst), DONE.load(Ordering::SeqCst));
    if wake == 0 || done == 0 || !WATCHING.load(Ordering::SeqCst) {
        // SAFETY: owned buffer (the caller won OWNER).
        unsafe { line(format_args!("!!! crash: (no dump thread: no minidump)")) };
        return;
    }
    EXC.store(info as usize, Ordering::SeqCst);
    EXC_TID.store(tid, Ordering::SeqCst);
    // SAFETY: our own events.
    let waited = unsafe {
        SetEvent(wake as HANDLE);
        WaitForSingleObject(done as HANDLE, DUMP_WAIT_MS)
    };
    if waited != 0 {
        // SAFETY: owned buffer (the watcher writes with a buffer of its own).
        unsafe { line(format_args!("!!! crash: the minidump did not finish in {} s (a lock the crash holds stops it): the exception goes on without it; Windows' own dump, where it keeps them, is in %LOCALAPPDATA%\\CrashDumps", DUMP_WAIT_MS / 1000)) };
    }
}

/// The watcher thread: waits for a crash (or the stop), writes the minidump, names the faulting thread.
fn watcher() {
    let wake = WAKE.load(Ordering::SeqCst);
    if wake == 0 {
        return;
    }
    // SAFETY: our own event (closed only after this thread is joined).
    unsafe { WaitForSingleObject(wake as HANDLE, u32::MAX) };
    let info = EXC.load(Ordering::SeqCst) as *mut EXCEPTION_POINTERS;
    if STOP.load(Ordering::SeqCst) || info.is_null() {
        return;
    }
    let tid = EXC_TID.load(Ordering::SeqCst);
    let mut msg = Line::<512>::new();
    dump(info, tid, &mut msg);
    thread_name(tid, &mut msg);
    let mut out = Line::<640>::new();
    emit(&mut out, format_args!("!!! crash: {}", msg.text()));
    let done = DONE.load(Ordering::SeqCst);
    if done != 0 {
        // SAFETY: our own event.
        unsafe { SetEvent(done as HANDLE) };
    }
}

/// The minidump into the log folder (the watcher thread).
fn dump(info: *mut EXCEPTION_POINTERS, tid: u32, msg: &mut Line<512>) {
    if WINE.load(Ordering::SeqCst) {
        let _ = write!(msg, "no minidump under Wine / Proton");
        return;
    }
    let f = MINIDUMP.load(Ordering::SeqCst);
    let Some(dir) = DIR_W.get().filter(|_| f != 0) else {
        let _ = write!(msg, "no minidump (dbghelp's writer not found)");
        return;
    };
    // SAFETY: a plain struct out-parameter.
    let t = unsafe {
        let mut t = std::mem::zeroed();
        GetLocalTime(&mut t);
        t
    };
    let mut name = Line::<64>::new();
    write_dump_name(&mut name, t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond);
    let mut path = [0u16; 1100];
    if dir.len() + name.len + 1 > path.len() {
        let _ = write!(msg, "no minidump (the folder's path is too long)");
        return;
    }
    path[..dir.len()].copy_from_slice(dir);
    for (slot, &b) in path[dir.len()..].iter_mut().zip(&name.buf[..name.len]) {
        *slot = u16::from(b);
    }
    // SAFETY: a new file at a terminated path (the buffer is zeroed past the name); dbghelp's writer with the faulting
    // thread's pointers (ClientPointers false: our own process); the handle is closed after.
    let written = unsafe {
        let file = CreateFileW(path.as_ptr(), GENERIC_WRITE, 0, std::ptr::null(), CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, std::ptr::null_mut());
        if file == INVALID_HANDLE_VALUE || file.is_null() {
            None
        } else {
            let mei = MINIDUMP_EXCEPTION_INFORMATION { ThreadId: tid, ExceptionPointers: info, ClientPointers: 0 };
            let write_dump = std::mem::transmute::<usize, MiniDumpWriteDumpFn>(f);
            let r = write_dump(GetCurrentProcess(), GetCurrentProcessId(), file, DUMP_TYPE, &mei, std::ptr::null(), std::ptr::null());
            CloseHandle(file);
            Some(r != 0)
        }
    };
    let _ = match written {
        Some(true) => write!(msg, "MINIDUMP WRITTEN: {} in the log folder (send it with this log)", name.text()),
        Some(false) => write!(msg, "the minidump {} could NOT be written (dbghelp refused)", name.text()),
        None => write!(msg, "the minidump file {} could NOT be made", name.text()),
    };
}

/// The faulting thread's own name (its description), after the dump (it allocates: last).
fn thread_name(tid: u32, msg: &mut Line<512>) {
    // SAFETY: a query-only handle; the description is freed with LocalFree as documented; at most 128 units are read.
    unsafe {
        let h = OpenThread(THREAD_QUERY_LIMITED_INFORMATION, 0, tid);
        if h.is_null() {
            return;
        }
        // looked up, not imported: an older Wine / Proton without it still loads the DLL
        let kernel32 = GetModuleHandleW(wide("kernel32.dll").as_ptr());
        let Some(get) = (!kernel32.is_null()).then(|| GetProcAddress(kernel32, c"GetThreadDescription".as_ptr().cast())).flatten() else {
            CloseHandle(h);
            return;
        };
        let get = std::mem::transmute::<unsafe extern "system" fn() -> isize, GetThreadDescriptionFn>(get);
        let mut p: *mut u16 = std::ptr::null_mut();
        let hr = get(h, &mut p);
        CloseHandle(h);
        if hr < 0 || p.is_null() {
            return;
        }
        let mut n = 0;
        while n < 128 && *p.add(n) != 0 {
            n += 1;
        }
        let _ = write!(msg, "; thread {tid} is named \"");
        for c in char::decode_utf16(std::slice::from_raw_parts(p, n).iter().copied()) {
            let _ = msg.write_char(c.unwrap_or('?'));
        }
        let _ = write!(msg, "\"{}", if n == 0 { " (no name: e.g. the game's main thread)" } else { "" });
        LocalFree(p.cast());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_and_accesses_have_their_names() {
        assert_eq!(code_name(0xC000_0005), "access violation");
        assert_eq!(code_name(0xC000_0374), "heap corruption");
        assert_eq!(code_name(0x1234), "other");
        assert_eq!(access_kind(0), "READ of");
        assert_eq!(access_kind(1), "WRITE to");
        assert_eq!(access_kind(8), "EXECUTE of (DEP)");
    }

    #[test]
    fn the_dump_is_named_by_its_time() {
        let mut l = Line::<64>::new();
        write_dump_name(&mut l, 2026, 11, 10, 13, 6, 44);
        assert_eq!(l.text(), "kh-er-crash-20261110-130644.dmp");
    }

    /// A long line is cut (also inside a character), never an error or a panic.
    #[test]
    fn a_line_too_long_is_cut() {
        let mut l = Line::<20>::new();
        for _ in 0..10 {
            let _ = write!(l, "abcdefgh");
        }
        assert_eq!(l.len, 20);
        let mut u = Line::<5>::new();
        let _ = write!(u, "abcdé");
        assert_eq!(u.text(), "abcd");
    }

    /// Module names come from the images: ntdll's own export name; an address in no image stays a number.
    #[test]
    fn module_names_come_from_the_image() {
        let mut l = Line::<160>::new();
        // SAFETY: a module handle by name.
        let ntdll = unsafe { GetModuleHandleW(wide("ntdll.dll").as_ptr()) } as usize;
        where_(&mut l, ntdll + 0x1000);
        assert!(l.text().eq_ignore_ascii_case("ntdll.dll+0x1000"), "{}", l.text());
        let mut n = Line::<160>::new();
        where_(&mut n, 0x10);
        assert_eq!(n.text(), "0x10");
    }

    /// The child of [`a_real_crash_is_recorded_and_passed_on`]: does nothing unless that test started it.
    #[test]
    fn crash_child() {
        let Some(dir) = std::env::var_os("KH_CRASHREC_CHILD") else { return };
        unsafe extern "system" fn the_filter_before(_: *const EXCEPTION_POINTERS) -> i32 {
            // SAFETY: ends this (child) process with a code the parent recognises: the chain reached the older filter.
            unsafe { windows_sys::Win32::System::Threading::TerminateProcess(GetCurrentProcess(), 77) };
            EXCEPTION_CONTINUE_SEARCH
        }
        // SAFETY: a test filter of this child process, set before the recorder's (as the game's / me3's are).
        unsafe { SetUnhandledExceptionFilter(Some(the_filter_before)) };
        crate::log::init(std::path::Path::new(&dir), true);
        install();
        // SAFETY: NOT SAFE ON PURPOSE: a write to the null page, the fatal exception the recorder must name.
        unsafe { std::ptr::write_volatile(std::hint::black_box(8usize) as *mut u8, 1) };
    }

    /// END TO END in a child process: a fatal access violation is written to the log (code, place, access, walk), a
    /// minidump is written beside it, the thread is named, and the exception still reaches the filter that was there
    /// before (it ends the child with 77: behaviour as without the recorder).
    #[test]
    fn a_real_crash_is_recorded_and_passed_on() {
        let dir = std::env::temp_dir().join(format!("kh_crashrec_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp folder");
        let exe = std::env::current_exe().expect("test binary");
        let status = std::process::Command::new(exe).args(["--exact", "crashrec::tests::crash_child", "--nocapture", "--test-threads", "1"]).env("KH_CRASHREC_CHILD", &dir).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().expect("child");
        let log = std::fs::read_to_string(dir.join("kh-er.log")).unwrap_or_default();
        let dumps: Vec<u64> = std::fs::read_dir(&dir).map(|r| r.flatten().filter(|e| e.file_name().to_string_lossy().starts_with("kh-er-crash-")).filter_map(|e| e.metadata().ok().map(|m| m.len())).collect()).unwrap_or_default();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(status.code(), Some(77), "the older filter got the exception after ours; log:\n{log}");
        assert!(log.contains("crash recorder: on"), "{log}");
        assert!(log.contains("!!! crash: AN EXCEPTION NOBODY TOOK: code 0xC0000005 (access violation)"), "{log}");
        assert!(log.contains("WRITE to 0x8"), "{log}");
        assert!(log.contains("!!! crash: registers rip"), "{log}");
        assert!(log.contains("!!! crash: stack walk #0 "), "{log}");
        assert!(log.contains("MINIDUMP WRITTEN: kh-er-crash-"), "{log}");
        assert!(log.contains("is named \"crashrec::tests::crash_child\""), "{log}");
        assert_eq!(dumps.len(), 1, "{log}");
        assert!(dumps[0] > 4096, "a real dump: {} bytes", dumps[0]);
    }

    /// The child of [`the_old_press_write_is_the_users_crash`]: does nothing unless that test started it.
    #[test]
    fn critical_section_child() {
        use windows_sys::Win32::System::Threading::{DeleteCriticalSection, InitializeCriticalSectionEx, CRITICAL_SECTION, RTL_CRITICAL_SECTION_FLAG_NO_DEBUG_INFO};
        let Some(dir) = std::env::var_os("KH_CRASHREC_CS_CHILD") else { return };
        unsafe extern "system" fn the_filter_before(_: *const EXCEPTION_POINTERS) -> i32 {
            // SAFETY: ends this (child) process with a code the parent recognises.
            unsafe { windows_sys::Win32::System::Threading::TerminateProcess(GetCurrentProcess(), 78) };
            EXCEPTION_CONTINUE_SEARCH
        }
        // SAFETY: a test filter of this child process, set before the recorder's.
        unsafe { SetUnhandledExceptionFilter(Some(the_filter_before)) };
        crate::log::init(std::path::Path::new(&dir), true);
        install();
        // A DLPlainLightMutex's CRITICAL_SECTION as the game makes it (no debug info: DebugInfo = -1), then THE OLD
        // mapgate.rs WRITE: one byte 1 at its first byte (CSActionButtonRegionSystemImp + 0x80), then the game's delete.
        let cs = Box::into_raw(Box::new(std::mem::MaybeUninit::<CRITICAL_SECTION>::zeroed()));
        // SAFETY: NOT SAFE ON PURPOSE (the child dies here): the corruption the old code made, and the delete that faults.
        unsafe {
            InitializeCriticalSectionEx((*cs).as_mut_ptr(), 0, RTL_CRITICAL_SECTION_FLAG_NO_DEBUG_INFO);
            std::ptr::write_volatile(cs.cast::<u8>(), 1);
            DeleteCriticalSection((*cs).as_mut_ptr());
        }
    }

    /// (CONT-CRASH) THE USER'S CRASH, REPRODUCED: a CRITICAL_SECTION with no debug info whose first byte got the old
    /// "press done" 1 faults in ntdll's RtlDeleteCriticalSection reading 0xFFFFFFFFFFFFFF11 (the user's dump: c0000005,
    /// ntdll+0x62485, param1 0xffffffffffffff11, rdi 0xffffffffffffff01) - and the recorder names it.
    #[test]
    fn the_old_press_write_is_the_users_crash() {
        let dir = std::env::temp_dir().join(format!("kh_crashrec_cs_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp folder");
        let exe = std::env::current_exe().expect("test binary");
        let status = std::process::Command::new(exe).args(["--exact", "crashrec::tests::critical_section_child", "--nocapture", "--test-threads", "1"]).env("KH_CRASHREC_CS_CHILD", &dir).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().expect("child");
        let log = std::fs::read_to_string(dir.join("kh-er.log")).unwrap_or_default();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(status.code(), Some(78), "{log}");
        assert!(log.contains("code 0xC0000005 (access violation) at ntdll.dll+0x"), "{log}");
        assert!(log.contains("READ of 0xffffffffffffff11"), "{log}");
        assert!(log.contains("rdi ffffffffffffff01"), "{log}");
    }

    /// The walk from a captured context of this thread finds frames (the unwinder runs on our copy).
    #[test]
    fn the_unwinder_walks_a_live_stack() {
        use windows_sys::Win32::System::Diagnostics::Debug::RtlCaptureContext;
        // SAFETY (zeroed): CONTEXT is plain data.
        let mut c = Ctx(unsafe { std::mem::zeroed() });
        // SAFETY: this thread's own context into an aligned buffer.
        unsafe { RtlCaptureContext(&mut c.0) };
        let (low, high) = stack_bounds();
        assert!((low..high).contains(&(c.0.Rsp as usize)));
        let mut image: u64 = 0;
        // SAFETY: the function table of this test binary's own code.
        let entry = unsafe { RtlLookupFunctionEntry(c.0.Rip, &mut image, std::ptr::null_mut()) };
        assert!(!entry.is_null() && image != 0);
    }
}
