//! (GF-SENSOR) EVERY CHARACTER'S ATTACK WINDOWS, READ FROM THE GAME AT RUN TIME: its TAE says for each animation when each
//! hit window opens and closes and which judge id it uses. Nothing of it is shipped (the owner's rule: no per-enemy tables):
//! the first sight of a kind of character starts its reading on the reader's thread, and until that is done (or when no TAE
//! is found) `book` answers nothing and the learned book (`telegraph.rs`) and the live flags take over.
//! build/sora_tmp/party/GOOFY/SENSOR_NOTES.md.
//!
//! THE CHARACTER'S OWN TAE COMES FIRST: `memory_taes` copies the file the game loaded for this very character
//! (`CSChrTimeActModule` + 0x10 -> + 0xA8 and + 0x2E8 = `CS::TaeDat`, + 8 = the file as loaded; no address of the executable,
//! and every read cannot fault: RTTI proven on 2.7.2.0, the magic checked here). Only without it is the file route asked
//! (`/chr/cNNNN.anibnd.dcx`), which needs a rule for variants (skeleton names, the decade's round id).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use kh_erfile::tae;

use crate::erlog;

/// One timed event of an animation that matters to the sensor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Window {
    pub kind: u32,
    pub start: f32,
    pub end: f32,
    /// The judge id (`BehaviorParam` = 200000000 + variation x 1000 + judge), -1 for a parry window.
    pub judge: i32,
}

/// The windows of one kind of character, by TimeAct animation id.
#[derive(Debug, Default)]
pub struct Book {
    pub anims: HashMap<i64, Vec<Window>>,
    pub from: String,
}

enum Slot {
    Loading,
    Ready(Arc<Book>),
    Failed,
}

struct Shelf {
    books: HashMap<i32, Slot>,
    tx: Option<mpsc::Sender<Ask>>,
}

/// What the reader's thread is asked: a character's file route, or the bytes of its loaded TAEs to parse.
enum Ask {
    Files(i32),
    Memory(i32, Vec<(usize, usize)>),
}

static SHELF: Mutex<Option<Shelf>> = Mutex::new(None);
static NO_THREAD: AtomicBool = AtomicBool::new(false);
/// "TAE " and the 64-bit mark of the file's first eight bytes.
const TAE_MAGIC: usize = 0xFF00_0000_2045_4154;
/// The `CS::TaeDat` pointers of the object at `CSChrTimeActModule` + 0x10 (MEASURED 2.7.2.0).
const TAE_SLOTS: [usize; 2] = [0xA8, 0x2E8];

/// The judge id of a kept event (`THROW`: its first argument; `ATTACK` and `BULLET`: the third; a flag event: none).
fn judge_of(e: &tae::Event) -> i32 {
    match e.kind {
        tae::ATTACK | tae::BULLET => e.args[2],
        tae::THROW => e.args[0],
        _ => -1,
    }
}

/// A file's animations as windows, imported animations given the events of the one they name.
pub fn windows_of(file: &tae::Tae) -> HashMap<i64, Vec<Window>> {
    let own = |a: &tae::Anim| a.events.iter().map(|e| Window { kind: e.kind, start: e.start, end: e.end, judge: judge_of(e) }).collect::<Vec<_>>();
    let mut out = HashMap::new();
    for (id, a) in &file.anims {
        let list = match a.import.and_then(|i| file.anims.get(&i)) {
            Some(source) => own(source),
            None => own(a),
        };
        if !list.is_empty() {
            out.insert(*id, list);
        }
    }
    out
}

/// Which files to try for a character: its own, then the character its skeleton names (MEASURED c4311 > c4310).
fn read_book(lib: &mut kh_erfile::Library, npc_id: i32) -> Result<Book, String> {
    let own = format!("c{npc_id:04}");
    let mut tried = vec![own.clone()];
    let mut next = vec![own];
    while let Some(chr) = next.pop() {
        let files = lib.animation_files(&chr)?;
        let mut book = Book { anims: HashMap::new(), from: chr.clone() };
        for f in files.iter().filter(|f| f.name.to_ascii_lowercase().ends_with(".tae")) {
            book.anims.extend(windows_of(&tae::read(&f.data)?));
        }
        if !book.anims.is_empty() {
            return Ok(book);
        }
        for f in files.iter().filter(|f| f.name.to_ascii_lowercase().contains("skeleton")) {
            for name in tae::chr_names(&f.data) {
                if !tried.contains(&name) {
                    tried.push(name.clone());
                    next.push(name);
                }
            }
        }
    }
    // (MEASURED: the TAE ids loaded beside c4191, c4351, c4071, c4371) a variant without a name of its base: the decade's round id
    let base = format!("c{:04}", npc_id - npc_id % 10);
    if !tried.contains(&base) {
        let files = lib.animation_files(&base)?;
        let mut book = Book { anims: HashMap::new(), from: base.clone() };
        for f in files.iter().filter(|f| f.name.to_ascii_lowercase().ends_with(".tae")) {
            book.anims.extend(windows_of(&tae::read(&f.data)?));
        }
        if !book.anims.is_empty() {
            return Ok(book);
        }
    }
    Err(format!("no TAE with attack windows (tried {tried:?})"))
}

/// Where the TAE files the game holds for this character are: (address, size). Nothing when the path does not hold what it
/// should. Every read here cannot fault (`chranim::read_word`); the files are copied the same way on the reader's thread.
fn memory_taes(chr: &eldenring::cs::ChrIns) -> Vec<(usize, usize)> {
    use crate::chranim::read_word;
    let mut out = Vec::new();
    if !crate::game::plausible(crate::word!(chr.modules.time_act)) {
        return out;
    }
    let time_act: &eldenring::cs::CSChrTimeActModule = &chr.modules.time_act;
    let Some(anim) = read_word(time_act as *const eldenring::cs::CSChrTimeActModule as usize + 0x10) else { return out };
    for slot in TAE_SLOTS {
        let Some(dat) = read_word(anim + slot) else { continue };
        let Some(file) = read_word(dat + 8) else { continue };
        if read_word(file) != Some(TAE_MAGIC) {
            continue;
        }
        let Some(size) = read_word(file + 8).map(|w| (w >> 32) as u32 as usize).filter(|s| (0x80..1 << 27).contains(s)) else { continue };
        out.push((file, size));
    }
    out
}

fn reader(rx: mpsc::Receiver<Ask>) {
    let game = crate::zdraw::game_dir();
    let mut lib: Option<kh_erfile::Library> = None;
    for ask in rx {
        let started = std::time::Instant::now();
        let (npc_id, result) = match ask {
            Ask::Memory(npc_id, files) => {
                let mut book = Book { anims: HashMap::new(), from: format!("memory ({} file(s))", files.len()) };
                for (base, size) in &files {
                    let mut f = vec![0u8; *size];
                    if !crate::chranim::read_bytes(*base, &mut f) {
                        erlog!("taebook: c{npc_id:04}: the TAE at {base:#x} went away");
                        continue;
                    }
                    match tae::read_loaded(&f, *base) {
                        Ok(t) => {
                            let windows = windows_of(&t);
                            book.from = format!("memory, TAE {}", t.id);
                            book.anims.extend(windows);
                        }
                        Err(e) => erlog!("taebook: c{npc_id:04}: a TAE read from memory: {e}"),
                    }
                }
                if book.anims.is_empty() {
                    // (no window in what the game holds: the files may say more)
                    let result = (|| -> Result<Book, String> {
                        if lib.is_none() {
                            lib = Some(kh_erfile::Library::open(&game)?);
                        }
                        read_book(lib.as_mut().ok_or("no library")?, npc_id)
                    })();
                    (npc_id, result)
                } else {
                    (npc_id, Ok(book))
                }
            }
            Ask::Files(npc_id) => {
                let result = (|| -> Result<Book, String> {
                    if lib.is_none() {
                        lib = Some(kh_erfile::Library::open(&game)?);
                    }
                    read_book(lib.as_mut().ok_or("no library")?, npc_id)
                })();
                (npc_id, result)
            }
        };
        let slot = match result {
            Ok(book) => {
                erlog!("taebook: c{npc_id:04}: {} animations with windows, from {} ({:.2} s)", book.anims.len(), book.from, started.elapsed().as_secs_f32());
                Slot::Ready(Arc::new(book))
            }
            Err(why) => {
                erlog!("taebook: c{npc_id:04}: {why}: the learned book and the live flags answer for it");
                Slot::Failed
            }
        };
        if let Some(s) = SHELF.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            s.books.insert(npc_id, slot);
        }
    }
}

/// The windows of a kind of character, once read (the first ask starts the reading, from this character's own memory
/// when it can be read, else from the files).
pub fn book(npc_id: i32, chr: &eldenring::cs::ChrIns) -> Option<Arc<Book>> {
    if NO_THREAD.load(Ordering::Relaxed) {
        return None;
    }
    let mut guard = SHELF.try_lock().ok()?;
    let shelf = guard.get_or_insert_with(|| Shelf { books: HashMap::new(), tx: None });
    match shelf.books.get(&npc_id) {
        Some(Slot::Ready(b)) => return Some(b.clone()),
        Some(_) => return None,
        None => {}
    }
    if shelf.tx.is_none() {
        let (tx, rx) = mpsc::channel::<Ask>();
        if std::thread::Builder::new().name("kh-taebook-reader".into()).spawn(move || reader(rx)).is_err() {
            NO_THREAD.store(true, Ordering::Relaxed);
            erlog!("taebook: the reader's thread cannot start: no windows from the files");
            return None;
        }
        shelf.tx = Some(tx);
    }
    let files = memory_taes(chr);
    let ask = if files.is_empty() { Ask::Files(npc_id) } else { Ask::Memory(npc_id, files) };
    if shelf.tx.as_ref().is_some_and(|tx| tx.send(ask).is_ok()) {
        shelf.books.insert(npc_id, Slot::Loading);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn an_imported_animation_gets_the_windows_of_its_source() {
        let event = tae::Event { kind: tae::ATTACK, start: 0.5, end: 0.7, args: [0, 0, 100, 255] };
        let mut anims = BTreeMap::new();
        anims.insert(3000, tae::Anim { events: vec![event], import: None });
        anims.insert(1_003_000, tae::Anim { events: Vec::new(), import: Some(3000) });
        anims.insert(9, tae::Anim::default());
        let w = windows_of(&tae::Tae { id: 1, anims });
        assert_eq!(w.len(), 2, "an animation without windows is not kept");
        assert_eq!(w[&1_003_000], vec![Window { kind: tae::ATTACK, start: 0.5, end: 0.7, judge: 100 }]);
    }

    #[test]
    fn judge_ids_by_kind() {
        let e = |kind, args| tae::Event { kind, start: 0.0, end: 0.1, args };
        assert_eq!(judge_of(&e(tae::ATTACK, [0, 0, 100, 255])), 100);
        assert_eq!(judge_of(&e(tae::BULLET, [2, 0, 120, 0])), 120);
        assert_eq!(judge_of(&e(tae::FLAG, [5, 0, -1, 6])), -1);
    }
}
