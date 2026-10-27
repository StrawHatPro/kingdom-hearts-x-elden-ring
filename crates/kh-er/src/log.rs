//! The log file of the DLL (`er\logs\kh-er.log`): one line per call, flushed at once (the game may
//! die under us and the last lines are the ones that matter). (LOGPRIV) No personal information: every line goes
//! through `redact::line`.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static LOG: OnceLock<Mutex<File>> = OnceLock::new();
static START: OnceLock<Instant> = OnceLock::new();
static DIR: OnceLock<std::path::PathBuf> = OnceLock::new();

/// (IMPL_ER14) The folder this payload logs to (`er\\logs`; `er\\logs_test` for the test profile).
pub fn dir() -> std::path::PathBuf {
    DIR.get().cloned().unwrap_or_default()
}

/// `fresh`: the first payload of this game run starts a new file (the previous run's is kept as
/// `kh-er.prev.log`); a reloaded payload appends.
/// The log folder of a payload that runs from `mods_dir`: `<root>\logs` for `<root>\mods` (THE USER'S), `<root>\logs<suffix>`
/// for `<root>\mods<suffix>` (the test profile's `mods_test` -> `logs_test`). ALWAYS under the folder's parent, never relative:
/// (IMPL_ER18) a relative path here once put the user's log into the game's own folder (the process's working directory).
pub fn folder_for(mods_dir: &Path) -> std::path::PathBuf {
    let root = mods_dir.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| mods_dir.to_path_buf());
    let folder = mods_dir.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    match folder.strip_prefix("mods") {
        Some(suffix) if !suffix.is_empty() => root.join(format!("logs{suffix}")),
        _ => root.join("logs"),
    }
}

pub fn init(dir: &Path, fresh: bool) {
    if !dir.is_absolute() {
        return; // never a file relative to the game's working directory
    }
    let _ = std::fs::create_dir_all(dir);
    let _ = DIR.set(dir.to_path_buf());
    // (LOGPRIV) the mod's folder (<mod>erlogs) and the game's are never written out
    if let Some(root) = dir.parent().and_then(|er| er.parent()) {
        crate::redact::folder(root, "<mod>");
    }
    if let Some(game) = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.to_path_buf())) {
        crate::redact::folder(&game, "<game>");
    }
    let path = dir.join("kh-er.log");
    if fresh {
        let _ = std::fs::rename(&path, dir.join("kh-er.prev.log"));
    }
    if let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = LOG.set(Mutex::new(file));
    }
    let _ = START.set(Instant::now());
}

pub fn write(text: &str) {
    let Some(log) = LOG.get() else { return };
    let t = START.get().map(|s| s.elapsed().as_secs_f64()).unwrap_or(0.0);
    if let Ok(mut file) = log.lock() {
        let _ = writeln!(file, "[{t:9.3}] {}", crate::redact::line(text));
        let _ = file.flush();
    }
}

#[macro_export]
macro_rules! erlog {
    ($($arg:tt)*) => { $crate::log::write(&format!($($arg)*)) };
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn the_log_folder_follows_the_mods_folder_and_is_never_relative() {
        assert_eq!(super::folder_for(Path::new(r"C:\x\er\mods")), Path::new(r"C:\x\er\logs"));
        assert_eq!(super::folder_for(Path::new(r"C:\x\er\mods_test")), Path::new(r"C:\x\er\logs_test"));
        assert_eq!(super::folder_for(Path::new(r"C:\x\er\other")), Path::new(r"C:\x\er\logs"));
        assert!(super::folder_for(Path::new(r"C:\x\er\mods")).is_absolute());
    }
}
