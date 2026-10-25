//! DCX, the game's compressed container (MEASURED on a chrbnd, 2.7.2.0): "DCX\0", 0x11000 (big-endian words), "DCS\0" with
//! the sizes (uncompressed, compressed; big-endian) at 0x1C / 0x20, "DCP\0" + "KRAK" at 0x24 / 0x28, "DCA\0" at 0x44, the data
//! at 0x4C. KRAK is Oodle Kraken: decompressed by the game's own `oo2core_6_win64.dll` (in its folder; already loaded in the
//! game's process), nothing of ours.

use std::path::Path;
use std::sync::OnceLock;

fn be32(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4).map(|x| u32::from_be_bytes([x[0], x[1], x[2], x[3]]))
}

#[cfg(windows)]
mod oodle {
    use std::ffi::c_void;

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }

    /// `OodleLZ_Decompress` (oo2core 2.6+: 14 arguments).
    pub type Decompress = unsafe extern "system" fn(*const u8, isize, *mut u8, isize, i32, i32, i32, *mut c_void, isize, *mut c_void, *mut c_void, *mut c_void, isize, i32) -> isize;

    pub fn load(path: &std::path::Path) -> Option<Decompress> {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
        // SAFETY: a plain library load of the game's own Oodle DLL by its full path; the symbol is looked up by name.
        unsafe {
            let module = LoadLibraryW(wide.as_ptr());
            if module.is_null() {
                return None;
            }
            let f = GetProcAddress(module, b"OodleLZ_Decompress\0".as_ptr());
            (!f.is_null()).then(|| std::mem::transmute::<*mut c_void, Decompress>(f))
        }
    }
}

#[cfg(windows)]
static OODLE: OnceLock<Option<oodle::Decompress>> = OnceLock::new();

/// Is this a DCX?
pub fn is_dcx(b: &[u8]) -> bool {
    b.starts_with(b"DCX\0")
}

/// A DCX's contents (`game` = the game's folder, for its Oodle DLL).
pub fn decompress(b: &[u8], game: &Path) -> Result<Vec<u8>, String> {
    if !is_dcx(b) {
        return Err("not a DCX".to_string());
    }
    let raw = be32(b, 0x1C).ok_or("short DCX")? as usize;
    let packed = be32(b, 0x20).ok_or("short DCX")? as usize;
    let kind = b.get(0x28..0x2C).ok_or("short DCX")?;
    let data_at = be32(b, 0x14).unwrap_or(0x4C) as usize;
    let data = b.get(data_at..data_at + packed).ok_or("DCX data past its end")?;
    match kind {
        b"KRAK" => kraken(data, raw, game),
        other => Err(format!("DCX compression {:?} is not read", String::from_utf8_lossy(other))),
    }
}

#[cfg(windows)]
fn kraken(data: &[u8], raw: usize, game: &Path) -> Result<Vec<u8>, String> {
    let f = OODLE.get_or_init(|| oodle::load(&game.join("oo2core_6_win64.dll"))).ok_or("the game's oo2core_6_win64.dll is not loadable")?;
    let mut out = vec![0u8; raw];
    // SAFETY: Oodle reads `data` and writes at most `raw` bytes into `out` (fuzz-safe decoding).
    let n = unsafe { f(data.as_ptr(), data.len() as isize, out.as_mut_ptr(), raw as isize, 1, 0, 0, std::ptr::null_mut(), 0, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), 0, 3) };
    if n as usize != raw {
        return Err(format!("Oodle gave {n} of {raw} bytes"));
    }
    Ok(out)
}

#[cfg(not(windows))]
fn kraken(_: &[u8], _: usize, _: &Path) -> Result<Vec<u8>, String> {
    Err("Oodle is Windows-only here".to_string())
}
