//! (ZDRAW) Elden Ring's own character models, read from the game's files (read-only) for Zettaflare's finale drawn by us
//! (kh-er zdraw.rs, kh-er-render zdraw.rs). `Library::character(id)` gives a character's FLVER from `/chr/cNNNN.chrbnd.dcx`,
//! `Library::part(name)` an equipment part's from `/parts/<name>.partsbnd.dcx` (a human of the `c0000` kind wears them).

pub mod aes;
pub mod archive;
pub mod bnd4;
pub mod dcx;
pub mod emevd; // (ZETTA 2026-10-24) the map events read for Zettaflare's whole kill of a two-phase boss
pub mod flver;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub use flver::Flver;

/// The archives equipment parts are in, in the order they are searched (MEASURED on 2.7.2.0: Data0 holds 1522 of them; the
/// exe maps `parts:` to `data0:/parts`).
pub const PART_ARCHIVES: [&str; 2] = ["Data0", "DLC"];

/// The game's archives, each opened (its index decrypted) the first time a file is looked for in it: ~0.3 s for Data3,
/// ~1.5 s for DLC or Data0.
pub struct Library {
    pub game: PathBuf,
    keys: Vec<archive::RsaKey>,
    archives: HashMap<String, archive::Archive>,
}

impl Library {
    pub fn open(game: &Path) -> Result<Library, String> {
        let keys = archive::exe_keys(&game.join("eldenring.exe"))?;
        if keys.is_empty() {
            return Err("no RSA key in eldenring.exe".to_string());
        }
        Ok(Library { game: game.to_path_buf(), keys, archives: HashMap::new() })
    }

    /// A file of the game by its virtual path (`/chr/c4311.chrbnd.dcx`), decrypted (still a DCX), searched in `names` order;
    /// the archive it came from.
    pub fn file_in(&mut self, path: &str, names: &[&str]) -> Result<Option<(Vec<u8>, String)>, String> {
        for name in names {
            if !self.archives.contains_key(*name) {
                let a = archive::Archive::open(&self.game, name, &self.keys)?;
                self.archives.insert(name.to_string(), a);
            }
            if let Some(a) = self.archives.get(*name) {
                if let Some(bytes) = a.read(path)? {
                    return Ok(Some((bytes, a.name.clone())));
                }
            }
        }
        Ok(None)
    }

    /// A character's file by its virtual path, searched in `CHR_ARCHIVES`.
    pub fn file(&mut self, path: &str) -> Result<Option<(Vec<u8>, String)>, String> {
        self.file_in(path, &archive::CHR_ARCHIVES)
    }

    fn binder(&mut self, path: &str, names: &[&str]) -> Result<Vec<bnd4::BndFile>, String> {
        let (bytes, _) = self.file_in(path, names)?.ok_or(format!("{path} is in none of {names:?}"))?;
        let raw = if dcx::is_dcx(&bytes) { dcx::decompress(&bytes, &self.game)? } else { bytes };
        bnd4::read(&raw)
    }

    /// Every file of a character's binder (`cNNNN`, e.g. `c4311`): (name, bytes).
    pub fn character_files(&mut self, chr: &str) -> Result<Vec<bnd4::BndFile>, String> {
        self.binder(&format!("/chr/{chr}.chrbnd.dcx"), &archive::CHR_ARCHIVES)
    }

    /// A character's model: its binder's FLVER named `cNNNN.flver` (or the first FLVER in it).
    pub fn character(&mut self, chr: &str) -> Result<Flver, String> {
        let files = self.character_files(chr)?;
        pick_flver(&files, chr)
    }

    /// An equipment part's model (`bd_m_1100`, `wp_a_0200`, ..): its binder's FLVER of that name (or the first FLVER).
    pub fn part(&mut self, name: &str) -> Result<Flver, String> {
        let name = name.to_ascii_lowercase();
        let files = self.binder(&format!("/parts/{name}.partsbnd.dcx"), &PART_ARCHIVES)?;
        pick_flver(&files, &name)
    }
}

fn pick_flver(files: &[bnd4::BndFile], stem: &str) -> Result<Flver, String> {
    let want = format!("{stem}.flver").to_ascii_lowercase();
    let pick = files.iter().find(|f| f.name.to_ascii_lowercase().replace('\\', "/").ends_with(&want)).or_else(|| files.iter().find(|f| f.data.starts_with(b"FLVER\0"))).ok_or(format!("{stem}: no FLVER in its binder ({} files)", files.len()))?;
    flver::read(&pick.data)
}

/// The file stem the game's model name of a part gives (`N:\..\BD_M_1100.flver` / `BD_M_1100` -> `bd_m_1100`).
pub fn part_stem(name: &str) -> String {
    let last = name.rsplit(['\\', '/']).next().unwrap_or(name);
    let stem = last.split('.').next().unwrap_or(last);
    stem.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    #[test]
    fn part_stems() {
        assert_eq!(super::part_stem("N:\\GR\\data\\INTERROOT_win64\\parts\\FullBody\\BD_M_1100\\BD_M_1100.flver"), "bd_m_1100");
        assert_eq!(super::part_stem("WP_A_0200"), "wp_a_0200");
    }
}
