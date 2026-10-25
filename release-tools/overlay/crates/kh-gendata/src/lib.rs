//! THE GENERATED TABLES MADE FROM KH3 DATA, READ AT RUN TIME (the released DLL carries no KH3 table).
//!
//! tools/packgen_genruntime.js turns each generated file made from the player's pack (Sora's and the party's animation
//! tables, the team cameras, ...) into a run-time version: its types and code stay, each table becomes a [`Gen`] that
//! decodes its values from `<pack>/gen/<file>.khg` on first use. The setup makes those data files on the player's PC from
//! their own KINGDOM HEARTS III install. A DLL and a pack made from different data have different schemas: [`init`] says
//! so instead of reading wrong values.
//!
//! The host calls [`init`] once with the folder before any table is used; a table used without it panics with what to run.
use std::collections::HashMap;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// The (file, schema) pairs this build expects (from `$KH_GEN_RUST_DIR/khg_schemas.rs` at build time; empty when the
/// tables are compiled in).
pub static EXPECTED: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/khg_schemas.rs"));

static DIR: OnceLock<PathBuf> = OnceLock::new();
static FILES: Mutex<Option<HashMap<String, &'static DataFile>>> = Mutex::new(None);

struct DataFile {
    blobs: Vec<&'static [u8]>,
}

const RUN_SETUP: &str = "run the setup (node setup/setup.js, or setup\\setup.bat) to make the pack and its generated data";

/// Sets the folder of the data files and checks every file this build expects: present, readable, the same schema.
/// Returns a line for the log, or what is wrong.
pub fn init(dir: &Path) -> Result<String, String> {
    let _ = DIR.set(dir.to_path_buf());
    if DIR.get().map(|d| d.as_path()) != Some(dir) {
        return Err(format!("kh-gendata: the data folder was set to {} already", DIR.get().unwrap().display()));
    }
    let mut bytes = 0usize;
    for (file, schema) in EXPECTED {
        let f = load(file, schema)?;
        bytes += f.blobs.iter().map(|b| b.len()).sum::<usize>();
    }
    Ok(format!("kh-gendata: {} generated table file(s) from {} ({} KB)", EXPECTED.len(), dir.display(), bytes / 1024))
}

fn load(file: &str, schema: &str) -> Result<&'static DataFile, String> {
    let mut guard = FILES.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    if let Some(f) = map.get(file) {
        return Ok(f);
    }
    let dir = DIR.get().ok_or_else(|| format!("kh-gendata: a generated table ({file}) was used before the data folder was set"))?;
    let path = dir.join(format!("{file}.khg"));
    let raw = std::fs::read(&path).map_err(|e| format!("kh-gendata: {} could not be read ({e}): {RUN_SETUP}", path.display()))?;
    let raw: &'static [u8] = Box::leak(raw.into_boxed_slice());
    let mut r = Reader { b: raw, p: 0 };
    if raw.len() < 6 || &raw[..6] != b"KHGD1\n" {
        return Err(format!("kh-gendata: {} is not a generated data file: {RUN_SETUP}", path.display()));
    }
    r.p = 6;
    let name = r.text_checked().ok_or("bad header")?;
    let have = r.text_checked().ok_or("bad header")?;
    if name != file || have != schema {
        return Err(format!(
            "kh-gendata: {} was made for another build of the mod (schema {have}, this DLL expects {schema}): {RUN_SETUP} again with this release",
            path.display()
        ));
    }
    let n = r.varint_checked().ok_or("bad header")?;
    let mut lens = Vec::with_capacity(n);
    for _ in 0..n {
        lens.push(r.varint_checked().ok_or("bad header")?);
    }
    let mut blobs = Vec::with_capacity(n);
    for len in lens {
        if r.p + len > raw.len() {
            return Err(format!("kh-gendata: {} is cut short: {RUN_SETUP}", path.display()));
        }
        blobs.push(&raw[r.p..r.p + len]);
        r.p += len;
    }
    let f: &'static DataFile = Box::leak(Box::new(DataFile { blobs }));
    map.insert(file.to_string(), f);
    Ok(f)
}

/// One table, decoded from its data file on first use.
pub struct Gen<T: 'static> {
    file: &'static str,
    schema: &'static str,
    index: usize,
    init: fn(&mut Reader) -> T,
    cell: OnceLock<T>,
}

impl<T: 'static> Gen<T> {
    pub const fn new(file: &'static str, schema: &'static str, index: usize, init: fn(&mut Reader) -> T) -> Self {
        Gen { file, schema, index, init, cell: OnceLock::new() }
    }

    /// The value (decoded on first use).
    pub fn khg_get(&self) -> &T {
        self.cell.get_or_init(|| {
            let f = load(self.file, self.schema).unwrap_or_else(|e| panic!("{e}"));
            let blob = *f.blobs.get(self.index).unwrap_or_else(|| panic!("kh-gendata: {}.khg has no table {}: {RUN_SETUP}", self.file, self.index));
            let mut r = Reader { b: blob, p: 0 };
            let v = (self.init)(&mut r);
            if r.p != blob.len() {
                panic!("kh-gendata: {}.khg table {}: {} byte(s) left over: {RUN_SETUP}", self.file, self.index, blob.len() - r.p);
            }
            v
        })
    }
}

impl<T: 'static> Deref for Gen<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.khg_get()
    }
}

/// Reads the values of one table (the generated decoders call it).
pub struct Reader {
    b: &'static [u8],
    p: usize,
}

impl Reader {
    fn varint_checked(&mut self) -> Option<usize> {
        let mut x = 0usize;
        let mut shift = 0;
        loop {
            let byte = *self.b.get(self.p)?;
            self.p += 1;
            x |= ((byte & 0x7f) as usize) << shift;
            if byte & 0x80 == 0 {
                return Some(x);
            }
            shift += 7;
            if shift > 56 {
                return None;
            }
        }
    }
    fn text_checked(&mut self) -> Option<&'static str> {
        let n = self.varint_checked()?;
        let s = self.b.get(self.p..self.p + n)?;
        self.p += n;
        std::str::from_utf8(s).ok()
    }
    /// A shape or a unit enum's index.
    pub fn index(&mut self) -> usize {
        self.varint_checked().unwrap_or_else(|| bad_data())
    }
    /// A number, parsed as the literal the generator wrote (exactly as the compiler would have).
    pub fn num<T: std::str::FromStr>(&mut self) -> T {
        let s = self.text_checked().unwrap_or_else(|| bad_data());
        s.parse().unwrap_or_else(|_| panic!("kh-gendata: the number {s:?} does not fit its field: {RUN_SETUP}"))
    }
    /// A string (borrowed from the data file, which stays loaded).
    pub fn str(&mut self) -> &'static str {
        self.text_checked().unwrap_or_else(|| bad_data())
    }
    pub fn boolean(&mut self) -> bool {
        self.index() != 0
    }
    /// A slice: its length, then each element.
    pub fn slice<T>(&mut self, mut f: impl FnMut(&mut Reader) -> T) -> &'static [T] {
        let n = self.index();
        let mut v = Vec::with_capacity(n.min(1 << 20));
        for _ in 0..n {
            v.push(f(self));
        }
        Box::leak(v.into_boxed_slice())
    }
}

/// A shape or enum index the decoder does not know: data made for another build.
pub fn bad_index() -> ! {
    panic!("kh-gendata: a generated table has a value this DLL does not know: {RUN_SETUP} again with this release")
}

fn bad_data() -> ! {
    panic!("kh-gendata: a generated data file is cut short or damaged: {RUN_SETUP}")
}

// ---------------------------------------------------------------- the game's own text
// (release) The KH3 sentences the mod shows (menu help lines, shop refusals, the level-up lines, KH3 abilities' help) are
// not in the DLL: they are read from the player's pack (`<pack>/ui/strings/en/<table>.json`, made by the setup from their
// own KH3 install) by key, `<table>.<namespace>.<key>` (e.g. `kh3_menu.shop.dialog_0020`). The pack is the folder above
// the data folder given to [`init`]. A key that is not there reads as "" (and is logged once by the caller's own checks).
static TEXT: Mutex<Option<HashMap<String, &'static str>>> = Mutex::new(None);
static TABLES: Mutex<Option<HashMap<String, Option<serde_json::Value>>>> = Mutex::new(None);

fn lookup(key: &str) -> Option<String> {
    let table = key.split('.').next()?;
    let (ns, k) = key.rsplit_once('.')?;
    let mut guard = TABLES.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    let json = map.entry(table.to_string()).or_insert_with(|| {
        let pack = DIR.get()?.parent()?.to_path_buf();
        let text = std::fs::read_to_string(pack.join("ui").join("strings").join("en").join(format!("{table}.json"))).ok()?;
        serde_json::from_str(&text).ok()
    });
    json.as_ref()?.get("namespaces")?.get(ns)?.get(k)?.as_str().map(|s| s.to_string())
}

fn cached(key: &str, sep: &str) -> &'static str {
    let id = format!("{sep}\u{0}{key}");
    let mut guard = TEXT.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    if let Some(s) = map.get(&id) {
        return s;
    }
    let s: &'static str = Box::leak(lookup(key).unwrap_or_default().replace("<br>", sep).into_boxed_str());
    map.insert(id, s);
    s
}

/// The game's text for `key` (`<table>.<namespace>.<key>` of the pack's ui/strings/en), its line breaks as "\n".
pub fn text(key: &str) -> &'static str {
    cached(key, "\n")
}

/// The same on one line (its line breaks as spaces).
pub fn line(key: &str) -> &'static str {
    cached(key, " ")
}

/// `text` for a key of the game's (`kh3_...`); any other string is returned as it is (our own words).
pub fn text_or(s: &'static str) -> &'static str {
    if s.starts_with("kh3_") { text(s) } else { s }
}

// ---------------------------------------------------------------- the repository
// (release) The folder the mod runs from (the repository: er/mods/kh_er.dll is in it), set by the DLL at start; the files
// the player may edit (sheets/port/*.json) are read from it, else the compiled copies.
static PROJECT: OnceLock<PathBuf> = OnceLock::new();

/// Sets the repository folder (once).
pub fn set_project(dir: &Path) {
    let _ = PROJECT.set(dir.to_path_buf());
}

/// `rel` (e.g. `sheets/port/sora-er-items.json`) under the repository; relative to the working folder before [`set_project`].
pub fn project_file(rel: &str) -> &'static str {
    static CACHE: Mutex<Option<HashMap<String, &'static str>>> = Mutex::new(None);
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    if let Some(p) = map.get(rel) {
        return p;
    }
    let p: &'static str = Box::leak(PROJECT.get().map(|d| d.join(rel)).unwrap_or_else(|| PathBuf::from(rel)).to_string_lossy().into_owned().into_boxed_str());
    map.insert(rel.to_string(), p);
    p
}
