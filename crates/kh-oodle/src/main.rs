//! kh-oodle: undoes Elden Ring's DCX (KRAK = Oodle Kraken) with the game's own `oo2core_6_win64.dll`, for the Linux setup
//! (tools/er_icons.js; on Windows it calls the same DLL through PowerShell). There is no Oodle for Linux, and the DLL is a
//! Windows DLL: this program loads it itself (a minimal PE loader: its sections mapped, its relocations applied, its 66
//! KERNEL32 imports answered by the small functions below, its CRT started through its entry point) and calls its
//! `OodleLZ_Decompress` export with the same arguments as tools/er_icons.js. Nothing of Oodle is ours; the DLL is read
//! from the player's own Elden Ring install (a copy made by tools/er_icons.js).
//!
//!   kh-oodle --dll <oo2core_6_win64.dll> <file.dcx>...      writes each file without its .dcx
//!
//! Output lines and exit codes are the PowerShell helper's: 2 = not KRAK, 3 = Oodle failed.
#![allow(clippy::missing_safety_doc)]
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::PathBuf;

// ---------------------------------------------------------------- memory for the image
#[cfg(unix)]
mod sys {
    use std::ffi::c_void;
    extern "C" {
        fn mmap(addr: *mut c_void, len: usize, prot: i32, flags: i32, fd: i32, off: i64) -> *mut c_void;
        fn syscall(n: i64, ...) -> i64;
    }
    pub fn alloc_rwx(size: usize) -> *mut u8 {
        // PROT_READ | PROT_WRITE | PROT_EXEC, MAP_PRIVATE | MAP_ANONYMOUS
        let p = unsafe { mmap(std::ptr::null_mut(), size, 7, 0x02 | 0x20, -1, 0) };
        if p as isize == -1 { std::ptr::null_mut() } else { p as *mut u8 }
    }
    /// The DLL's code reads the thread's TIB through `gs` (its stack-probe helper reads gs:[0x10], the stack limit): `gs`
    /// is pointed at a zeroed TIB of our own (stack limit 0: no probing needed on a thread stack that is all mapped).
    pub fn set_tib(tib: *mut u8) -> bool {
        // arch_prctl(ARCH_SET_GS, tib)
        unsafe { syscall(158, 0x1001i64, tib as i64) == 0 }
    }
}
#[cfg(windows)]
mod sys {
    use std::ffi::c_void;
    #[link(name = "kernel32")]
    extern "system" {
        fn VirtualAlloc(addr: *mut c_void, size: usize, kind: u32, protect: u32) -> *mut c_void;
    }
    pub fn alloc_rwx(size: usize) -> *mut u8 {
        unsafe { VirtualAlloc(std::ptr::null_mut(), size, 0x3000, 0x40) as *mut u8 }
    }
    /// (Windows: the thread's own TIB is there; this build exists to test the loader and the functions below)
    pub fn set_tib(_: *mut u8) -> bool {
        true
    }
}

// ---------------------------------------------------------------- the KERNEL32 functions the DLL imports
#[allow(non_snake_case)]
mod k32 {
    use std::alloc::{alloc_zeroed, dealloc, Layout};
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

    pub static IMAGE: AtomicUsize = AtomicUsize::new(0);
    static LAST_ERROR: AtomicU32 = AtomicU32::new(0);
    static TLS: [AtomicUsize; 64] = [const { AtomicUsize::new(0) }; 64];
    static TLS_NEXT: AtomicU32 = AtomicU32::new(0);
    const NAME: &[u8] = b"oo2core_6_win64.dll\0";
    static CMDLINE: &[u8] = b"oo2core\0";
    static ENV: [u16; 2] = [0, 0];

    // the heap: blocks with their size in front (HeapSize)
    unsafe fn block(size: usize) -> *mut u8 {
        let p = alloc_zeroed(Layout::from_size_align_unchecked(size + 16, 16));
        if p.is_null() { return p; }
        (p as *mut usize).write(size);
        p.add(16)
    }
    unsafe fn size_of(p: *mut u8) -> usize { (p.sub(16) as *mut usize).read() }
    unsafe fn free(p: *mut u8) { let s = size_of(p); dealloc(p.sub(16), Layout::from_size_align_unchecked(s + 16, 16)); }
    pub extern "win64" fn HeapAlloc(_h: usize, _f: u32, size: usize) -> *mut u8 { unsafe { block(size) } }
    pub extern "win64" fn HeapFree(_h: usize, _f: u32, p: *mut u8) -> i32 { if !p.is_null() { unsafe { free(p) } } 1 }
    pub extern "win64" fn HeapReAlloc(_h: usize, _f: u32, p: *mut u8, size: usize) -> *mut u8 {
        unsafe {
            let n = block(size);
            if !n.is_null() && !p.is_null() { std::ptr::copy_nonoverlapping(p, n, size_of(p).min(size)); free(p); }
            n
        }
    }
    pub extern "win64" fn HeapSize(_h: usize, _f: u32, p: *mut u8) -> usize { if p.is_null() { usize::MAX } else { unsafe { size_of(p) } } }
    pub extern "win64" fn GetProcessHeap() -> usize { 0x1000 }

    pub extern "win64" fn EncodePointer(p: usize) -> usize { p }
    pub extern "win64" fn DecodePointer(p: usize) -> usize { p }
    pub extern "win64" fn GetLastError() -> u32 { LAST_ERROR.load(Ordering::Relaxed) }
    pub extern "win64" fn SetLastError(e: u32) { LAST_ERROR.store(e, Ordering::Relaxed) }
    pub extern "win64" fn GetCurrentThreadId() -> u32 { 1 }
    pub extern "win64" fn GetCurrentProcessId() -> u32 { 1 }
    pub extern "win64" fn GetCurrentProcess() -> isize { -1 }
    pub extern "win64" fn IsDebuggerPresent() -> i32 { 0 }
    /// SSE / SSE2 / SSE3 (6, 10, 13) are there on every x86-64; fast-fail (23) and the rest are answered no
    pub extern "win64" fn IsProcessorFeaturePresent(f: u32) -> i32 { matches!(f, 6 | 10 | 13) as i32 }
    pub extern "win64" fn ExitProcess(code: u32) -> ! { std::process::exit(code as i32) }
    pub extern "win64" fn TerminateProcess(_h: isize, code: u32) -> i32 { eprintln!("kh-oodle: the DLL ended the process ({code:#x})"); std::process::exit(3) }
    pub extern "win64" fn GetCommandLineA() -> *const u8 { CMDLINE.as_ptr() }
    pub extern "win64" fn GetEnvironmentStringsW() -> *const u16 { ENV.as_ptr() }
    pub extern "win64" fn FreeEnvironmentStringsW(_p: *const u16) -> i32 { 1 }
    pub extern "win64" fn GetStartupInfoW(p: *mut u8) { unsafe { std::ptr::write_bytes(p, 0, 104); (p as *mut u32).write(104); } }
    pub extern "win64" fn GetStdHandle(_n: u32) -> isize { -1 }
    pub extern "win64" fn GetFileType(_h: isize) -> u32 { 0 }
    pub extern "win64" fn WriteFile(_h: isize, buf: *const u8, n: u32, written: *mut u32, _o: usize) -> i32 {
        unsafe { eprint!("{}", String::from_utf8_lossy(std::slice::from_raw_parts(buf, n as usize))); if !written.is_null() { written.write(n); } }
        1
    }
    pub extern "win64" fn WriteConsoleW(_h: isize, buf: *const u16, n: u32, written: *mut u32, _r: usize) -> i32 {
        unsafe { eprint!("{}", String::from_utf16_lossy(std::slice::from_raw_parts(buf, n as usize))); if !written.is_null() { written.write(n); } }
        1
    }
    pub extern "win64" fn GetConsoleCP() -> u32 { 437 }
    pub extern "win64" fn GetConsoleMode(_h: isize, _m: *mut u32) -> i32 { 0 }
    pub extern "win64" fn FlushFileBuffers(_h: isize) -> i32 { 1 }
    pub extern "win64" fn SetStdHandle(_n: u32, _h: isize) -> i32 { 1 }
    pub extern "win64" fn SetFilePointerEx(_h: isize, _d: i64, _p: *mut i64, _m: u32) -> i32 { 0 }
    pub extern "win64" fn CloseHandle(_h: isize) -> i32 { 1 }
    pub extern "win64" fn CreateFileW(_n: *const u16, _a: u32, _s: u32, _sa: usize, _c: u32, _f: u32, _t: isize) -> isize { SetLastError(2); -1 }
    pub extern "win64" fn OutputDebugStringA(_s: *const u8) {}
    pub extern "win64" fn OutputDebugStringW(_s: *const u16) {}
    pub extern "win64" fn Sleep(ms: u32) { std::thread::sleep(std::time::Duration::from_millis(ms as u64)) }

    // modules: only the DLL itself is known
    pub extern "win64" fn GetModuleHandleW(name: *const u16) -> usize { if name.is_null() { IMAGE.load(Ordering::Relaxed) } else { SetLastError(126); 0 } }
    pub extern "win64" fn GetModuleHandleExW(_f: u32, _n: *const u16, out: *mut usize) -> i32 { if !out.is_null() { unsafe { out.write(0) } } SetLastError(126); 0 }
    pub extern "win64" fn GetProcAddress(_m: usize, _n: *const u8) -> usize { SetLastError(127); 0 }
    pub extern "win64" fn LoadLibraryExW(_n: *const u16, _f: isize, _fl: u32) -> usize { SetLastError(126); 0 }
    fn module_name<T: Copy>(buf: *mut T, n: u32, conv: fn(u8) -> T) -> u32 {
        let len = (NAME.len() - 1).min(n.saturating_sub(1) as usize);
        unsafe { for (i, b) in NAME[..len].iter().enumerate() { buf.add(i).write(conv(*b)); } if n > 0 { buf.add(len).write(conv(0)); } }
        len as u32
    }
    pub extern "win64" fn GetModuleFileNameA(_m: usize, buf: *mut u8, n: u32) -> u32 { module_name(buf, n, |b| b) }
    pub extern "win64" fn GetModuleFileNameW(_m: usize, buf: *mut u16, n: u32) -> u32 { module_name(buf, n, |b| b as u16) }

    // time
    pub extern "win64" fn QueryPerformanceCounter(p: *mut i64) -> i32 {
        static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        let t = START.get_or_init(std::time::Instant::now).elapsed().as_nanos() as i64;
        unsafe { p.write(t + 1) }
        1
    }
    pub extern "win64" fn GetSystemTimeAsFileTime(p: *mut u64) {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() / 100).unwrap_or(0) as u64 + 116_444_736_000_000_000;
        unsafe { p.write(t) }
    }

    // critical sections and thread-local slots (the DLL runs on one thread here)
    pub extern "win64" fn InitializeCriticalSectionAndSpinCount(_c: *mut c_void, _s: u32) -> i32 { 1 }
    pub extern "win64" fn EnterCriticalSection(_c: *mut c_void) {}
    pub extern "win64" fn LeaveCriticalSection(_c: *mut c_void) {}
    pub extern "win64" fn DeleteCriticalSection(_c: *mut c_void) {}
    pub extern "win64" fn TlsAlloc() -> u32 { let i = TLS_NEXT.fetch_add(1, Ordering::Relaxed); if i < 64 { i } else { u32::MAX } }
    pub extern "win64" fn TlsFree(_i: u32) -> i32 { 1 }
    pub extern "win64" fn TlsGetValue(i: u32) -> usize { SetLastError(0); TLS.get(i as usize).map_or(0, |v| v.load(Ordering::Relaxed)) }
    pub extern "win64" fn TlsSetValue(i: u32, v: usize) -> i32 { TLS.get(i as usize).map_or(0, |s| { s.store(v, Ordering::Relaxed); 1 }) }

    // code pages and character types: plain ASCII / Latin-1 (the DLL formats no text when it decompresses)
    pub extern "win64" fn IsValidCodePage(_cp: u32) -> i32 { 1 }
    pub extern "win64" fn GetACP() -> u32 { 1252 }
    pub extern "win64" fn GetOEMCP() -> u32 { 437 }
    pub extern "win64" fn GetCPInfo(_cp: u32, info: *mut u8) -> i32 {
        unsafe { std::ptr::write_bytes(info, 0, 20); (info as *mut u32).write(1); info.add(4).write(b'?'); }
        1
    }
    pub extern "win64" fn MultiByteToWideChar(_cp: u32, _f: u32, src: *const u8, n: i32, dst: *mut u16, cap: i32) -> i32 {
        unsafe {
            let len = if n < 0 { (0..).take_while(|&i| *src.add(i) != 0).count() + 1 } else { n as usize };
            if cap == 0 { return len as i32; }
            if (cap as usize) < len { SetLastError(122); return 0; }
            for i in 0..len { dst.add(i).write(*src.add(i) as u16); }
            len as i32
        }
    }
    pub extern "win64" fn WideCharToMultiByte(_cp: u32, _f: u32, src: *const u16, n: i32, dst: *mut u8, cap: i32, _d: *const u8, used: *mut i32) -> i32 {
        unsafe {
            let len = if n < 0 { (0..).take_while(|&i| *src.add(i) != 0).count() + 1 } else { n as usize };
            if !used.is_null() { used.write(0); }
            if cap == 0 { return len as i32; }
            if (cap as usize) < len { SetLastError(122); return 0; }
            for i in 0..len { let c = *src.add(i); dst.add(i).write(if c < 0x100 { c as u8 } else { b'?' }); }
            len as i32
        }
    }
    pub extern "win64" fn GetStringTypeW(_t: u32, src: *const u16, n: i32, out: *mut u16) -> i32 {
        unsafe {
            let len = if n < 0 { (0..).take_while(|&i| *src.add(i) != 0).count() } else { n as usize };
            for i in 0..len {
                let c = *src.add(i);
                let b = if c < 0x80 { c as u8 } else { 0 };
                let mut f = 0x200u16; // C1_DEFINED
                if b.is_ascii_uppercase() { f |= 0x1 | 0x100; }
                if b.is_ascii_lowercase() { f |= 0x2 | 0x100; }
                if b.is_ascii_digit() { f |= 0x4; }
                if matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c) { f |= 0x8; }
                if b.is_ascii_punctuation() { f |= 0x10; }
                if b < 0x20 || b == 0x7f { f |= 0x20; }
                if b == b' ' || b == b'\t' { f |= 0x40; }
                if b.is_ascii_hexdigit() { f |= 0x80; }
                out.add(i).write(if c < 0x80 { f } else { 0x200 | 0x100 });
            }
        }
        1
    }
    pub extern "win64" fn LCMapStringW(_l: u32, flags: u32, src: *const u16, n: i32, dst: *mut u16, cap: i32) -> i32 {
        unsafe {
            let len = if n < 0 { (0..).take_while(|&i| *src.add(i) != 0).count() + 1 } else { n as usize };
            if cap == 0 { return len as i32; }
            if (cap as usize) < len { SetLastError(122); return 0; }
            for i in 0..len {
                let c = *src.add(i);
                let m = if c < 0x80 { let b = c as u8; (if flags & 0x200 != 0 { b.to_ascii_uppercase() } else if flags & 0x100 != 0 { b.to_ascii_lowercase() } else { b }) as u16 } else { c };
                dst.add(i).write(m);
            }
            len as i32
        }
    }

    // exceptions: Oodle throws none when it decompresses; reaching these means the DLL failed
    pub extern "win64" fn RtlCaptureContext(_c: *mut c_void) {}
    pub extern "win64" fn RtlLookupFunctionEntry(_pc: u64, _base: *mut u64, _h: *mut c_void) -> usize { 0 }
    pub extern "win64" fn RtlVirtualUnwind(_t: u32, _b: u64, _pc: u64, _f: *mut c_void, _c: *mut c_void, _h: *mut c_void, _e: *mut u64, _cp: *mut c_void) -> usize { 0 }
    pub extern "win64" fn RtlPcToFileHeader(_pc: usize, base: *mut usize) -> usize { let b = IMAGE.load(Ordering::Relaxed); if !base.is_null() { unsafe { base.write(b) } } b }
    pub extern "win64" fn RtlUnwindEx(_a: usize, _b: usize, _c: usize, _d: usize, _e: usize, _f: usize) { eprintln!("kh-oodle: the DLL unwound an exception"); std::process::exit(3) }
    pub extern "win64" fn RaiseException(code: u32, _f: u32, _n: u32, _a: usize) { eprintln!("kh-oodle: the DLL raised exception {code:#x}"); std::process::exit(3) }
    pub extern "win64" fn UnhandledExceptionFilter(_e: usize) -> i32 { 0 }
    pub extern "win64" fn SetUnhandledExceptionFilter(_f: usize) -> usize { 0 }
}

/// The address of our function for the import `name`
fn import(name: &str) -> Option<usize> {
    macro_rules! t { ($($n:ident),* $(,)?) => { match name { $(stringify!($n) => Some(k32::$n as *const () as usize),)* _ => None } } }
    t!(OutputDebugStringA, HeapAlloc, HeapFree, GetProcessHeap, EncodePointer, DecodePointer, RtlLookupFunctionEntry, RtlUnwindEx, GetCommandLineA,
        GetCurrentThreadId, IsDebuggerPresent, IsProcessorFeaturePresent, GetLastError, ExitProcess, GetModuleHandleExW, GetProcAddress, MultiByteToWideChar,
        WideCharToMultiByte, HeapSize, SetLastError, RtlPcToFileHeader, RaiseException, IsValidCodePage, GetACP, GetOEMCP, GetCPInfo, GetStdHandle,
        GetFileType, DeleteCriticalSection, GetStartupInfoW, GetModuleFileNameA, QueryPerformanceCounter, GetCurrentProcessId, GetSystemTimeAsFileTime,
        GetEnvironmentStringsW, FreeEnvironmentStringsW, RtlCaptureContext, RtlVirtualUnwind, UnhandledExceptionFilter, SetUnhandledExceptionFilter,
        InitializeCriticalSectionAndSpinCount, Sleep, GetCurrentProcess, TerminateProcess, TlsAlloc, TlsGetValue, TlsSetValue, TlsFree, GetModuleHandleW,
        EnterCriticalSection, LeaveCriticalSection, WriteFile, GetModuleFileNameW, LoadLibraryExW, HeapReAlloc, GetStringTypeW, LCMapStringW,
        OutputDebugStringW, FlushFileBuffers, GetConsoleCP, GetConsoleMode, SetStdHandle, SetFilePointerEx, WriteConsoleW, CloseHandle, CreateFileW)
}

// ---------------------------------------------------------------- the PE loader
struct Image {
    base: *mut u8,
    entry: usize,
    exports: HashMap<String, usize>,
}

fn u16_at(b: &[u8], o: usize) -> Result<u16, String> { b.get(o..o + 2).map(|x| u16::from_le_bytes([x[0], x[1]])).ok_or_else(|| format!("PE: short at {o:#x}")) }
fn u32_at(b: &[u8], o: usize) -> Result<u32, String> { b.get(o..o + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]])).ok_or_else(|| format!("PE: short at {o:#x}")) }
fn u64_at(b: &[u8], o: usize) -> Result<u64, String> { b.get(o..o + 8).map(|x| u64::from_le_bytes(x.try_into().unwrap())).ok_or_else(|| format!("PE: short at {o:#x}")) }

fn load(file: &[u8]) -> Result<Image, String> {
    if file.get(0..2) != Some(b"MZ") { return Err("not a PE file".into()); }
    let pe = u32_at(file, 0x3c)? as usize;
    if file.get(pe..pe + 4) != Some(b"PE\0\0") || u16_at(file, pe + 4)? != 0x8664 { return Err("not an x86-64 PE file".into()); }
    let nsec = u16_at(file, pe + 6)? as usize;
    let opt = pe + 24;
    if u16_at(file, opt)? != 0x20b { return Err("not a PE32+ file".into()); }
    let entry = u32_at(file, opt + 16)? as usize;
    let image_base = u64_at(file, opt + 24)?;
    let size = u32_at(file, opt + 56)? as usize;
    let headers = u32_at(file, opt + 60)? as usize;
    let dir = |i: usize| -> Result<(usize, usize), String> { Ok((u32_at(file, opt + 112 + i * 8)? as usize, u32_at(file, opt + 116 + i * 8)? as usize)) };
    let base = sys::alloc_rwx(size);
    if base.is_null() { return Err("no memory for the DLL".into()); }
    let img = unsafe { std::slice::from_raw_parts_mut(base, size) };
    img[..headers.min(file.len())].copy_from_slice(&file[..headers.min(file.len())]);
    let sec0 = opt + u16_at(file, pe + 20)? as usize;
    for k in 0..nsec {
        let s = sec0 + k * 40;
        let (va, raw_size, raw) = (u32_at(file, s + 12)? as usize, u32_at(file, s + 16)? as usize, u32_at(file, s + 20)? as usize);
        let n = raw_size.min(size.saturating_sub(va));
        if n > 0 { img[va..va + n].copy_from_slice(file.get(raw..raw + n).ok_or("PE: section past the file's end")?); }
    }
    // relocations (DIR64)
    let delta = (base as u64).wrapping_sub(image_base);
    let (mut r, rsize) = dir(5)?;
    let rend = r + rsize;
    while r + 8 <= rend {
        let page = u32_at(img, r)? as usize;
        let block = u32_at(img, r + 4)? as usize;
        if block < 8 { break; }
        for e in (r + 8..r + block).step_by(2) {
            let v = u16_at(img, e)?;
            match v >> 12 {
                0 => {}
                10 => { let at = page + (v & 0xfff) as usize; let x = u64_at(img, at)?.wrapping_add(delta); img[at..at + 8].copy_from_slice(&x.to_le_bytes()); }
                t => return Err(format!("PE: relocation type {t}")),
            }
        }
        r += block;
    }
    // imports
    let (mut d, _) = dir(1)?;
    let cstr = |img: &[u8], o: usize| -> String { let e = img[o..].iter().position(|&c| c == 0).map_or(img.len(), |p| o + p); String::from_utf8_lossy(&img[o..e]).into_owned() };
    loop {
        let (oft, name, ft) = (u32_at(img, d)? as usize, u32_at(img, d + 12)? as usize, u32_at(img, d + 16)? as usize);
        if name == 0 { break; }
        let dll = cstr(img, name);
        let mut k = 0;
        loop {
            let thunk = u64_at(img, if oft != 0 { oft } else { ft } + k * 8)?;
            if thunk == 0 { break; }
            if thunk >> 63 != 0 { return Err(format!("{dll}: import by ordinal {}", thunk & 0xffff)); }
            let f = cstr(img, thunk as usize + 2);
            let addr = if dll.eq_ignore_ascii_case("KERNEL32.dll") { import(&f) } else { None }.ok_or_else(|| format!("the DLL imports {dll}!{f}, which kh-oodle does not provide"))?;
            img[ft + k * 8..ft + k * 8 + 8].copy_from_slice(&(addr as u64).to_le_bytes());
            k += 1;
        }
        d += 20;
    }
    // exports
    let (e, _) = dir(0)?;
    let mut exports = HashMap::new();
    if e != 0 {
        let (n, funcs, names, ords) = (u32_at(img, e + 24)? as usize, u32_at(img, e + 28)? as usize, u32_at(img, e + 32)? as usize, u32_at(img, e + 36)? as usize);
        for i in 0..n {
            let nm = cstr(img, u32_at(img, names + i * 4)? as usize);
            let ord = u16_at(img, ords + i * 2)? as usize;
            exports.insert(nm, base as usize + u32_at(img, funcs + ord * 4)? as usize);
        }
    }
    k32::IMAGE.store(base as usize, std::sync::atomic::Ordering::Relaxed);
    Ok(Image { base, entry: base as usize + entry, exports })
}

type DllMain = unsafe extern "win64" fn(*mut u8, u32, *mut c_void) -> i32;
/// `OodleLZ_Decompress` (oo2core 2.6+: 14 arguments)
type Decompress = unsafe extern "win64" fn(*const u8, isize, *mut u8, isize, i32, i32, i32, *mut c_void, isize, *mut c_void, *mut c_void, *mut c_void, isize, i32) -> isize;

fn be32(b: &[u8], at: usize) -> usize { u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]) as usize }

fn run(dll: PathBuf, files: Vec<PathBuf>) -> i32 {
    let bytes = match std::fs::read(&dll) { Ok(b) => b, Err(e) => { eprintln!("kh-oodle: {}: {e}", dll.display()); return 3; } };
    // the thread's TIB (gs), before the DLL's first instruction
    let tib = Box::leak(vec![0u8; 0x1000].into_boxed_slice()).as_mut_ptr();
    unsafe { (tib.add(0x30) as *mut usize).write(tib as usize) }; // NT_TIB.Self
    if !sys::set_tib(tib) { eprintln!("kh-oodle: the thread's TIB could not be set"); return 3; }
    let img = match load(&bytes) { Ok(i) => i, Err(e) => { eprintln!("kh-oodle: {}: {e}", dll.display()); return 3; } };
    // SAFETY: the DLL's own entry point (its CRT start-up), DLL_PROCESS_ATTACH
    let main: DllMain = unsafe { std::mem::transmute::<usize, DllMain>(img.entry) };
    if unsafe { main(img.base, 1, std::ptr::null_mut()) } == 0 { eprintln!("kh-oodle: the DLL's start-up failed"); return 3; }
    let Some(&f) = img.exports.get("OodleLZ_Decompress") else { eprintln!("kh-oodle: no OodleLZ_Decompress export"); return 3; };
    let decompress: Decompress = unsafe { std::mem::transmute::<usize, Decompress>(f) };
    for file in files {
        let b = match std::fs::read(&file) { Ok(b) => b, Err(e) => { println!("{}: {e}", file.display()); return 3; } };
        if b.len() < 0x4c || &b[0x28..0x2c] != b"KRAK" { println!("{}: not KRAK", file.display()); return 2; }
        let (usize_, csize) = (be32(&b, 0x1c), be32(&b, 0x20));
        let Some(data) = b.get(0x4c..0x4c + csize) else { println!("{}: OODLE FAILED (short file)", file.display()); return 3; };
        let mut out = vec![0u8; usize_];
        // SAFETY: Oodle reads `data` and writes at most `usize_` bytes into `out` (fuzz-safe decoding), as tools/er_icons.js calls it
        let n = unsafe { decompress(data.as_ptr(), csize as isize, out.as_mut_ptr(), usize_ as isize, 1, 0, 0, std::ptr::null_mut(), 0, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), 0, 3) };
        if n as usize != usize_ { println!("{}: OODLE FAILED ({n} of {usize_})", file.display()); return 3; }
        let s = file.to_string_lossy();
        let target = PathBuf::from(&s[..s.len() - 4]);
        if let Err(e) = std::fs::write(&target, &out) { println!("{}: {e}", target.display()); return 3; }
        println!("{}: {csize} -> {usize_}", file.display());
    }
    0
}

fn main() {
    let mut args = std::env::args_os().skip(1);
    let (Some(flag), Some(dll)) = (args.next(), args.next()) else { eprintln!("usage: kh-oodle --dll <oo2core_6_win64.dll> <file.dcx>..."); std::process::exit(2) };
    if flag != "--dll" { eprintln!("usage: kh-oodle --dll <oo2core_6_win64.dll> <file.dcx>..."); std::process::exit(2); }
    let files: Vec<PathBuf> = args.map(PathBuf::from).collect();
    // (a thread with a large, fully mapped stack: the DLL's stack probes are not needed on it)
    let t = std::thread::Builder::new().stack_size(64 << 20).spawn(move || run(PathBuf::from(dll), files)).expect("thread");
    std::process::exit(t.join().unwrap_or(3));
}
