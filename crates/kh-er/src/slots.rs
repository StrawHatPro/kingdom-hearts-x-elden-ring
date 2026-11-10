//! (IMPL_ER13) THE TEN SAVE SLOTS (build/sora_tmp/er/IMPL_ER13_STATE.md): Elden Ring's ten character slots are the ten
//! Kingdom Hearts save slots of Sora's playthrough (the user's plan). This is the one internal API the save / load
//! screens, the save point, the game-over menu and the testing aids use:
//!
//! - [`slot_summaries`]: what each slot holds, WITHOUT loading it: from the game's own profile summary (the list its
//!   Load Game shows: used or empty, the character's name, play time, the place's name) and from Sora's `.khsave` of
//!   that slot (his level, munny, his own play time). Works at the title and in play.
//! - [`save_to_slot`]: the running session is written into slot `n` by the game's own save routine (savegate.rs:
//!   the slot is set, both requests, the gate) and Sora's state of the same moment into that slot's `.khsave`, with
//!   the slot's save-points marker; [`save_status`] / [`take_save_result`] report busy, done or failed.
//! - [`load_slot`]: the game's own load of slot `n` (in play: `GameMan + 0xB78`); nothing of the running session
//!   survives it (savegate.rs `load_slot`).
//!
//! THE GAME'S PROFILE SUMMARY IN MEMORY (eldenring.exe 2.7.1.0, read from the file: fn 0x256780 returns
//! `[GameDataMan + 0x78]`; 0x262260 sets the byte, 0x262280 fills the record): "in use" byte of slot `s` at
//! `+ 8 + s`; its record at `+ s * 0x2A0`: name UTF-16 (16 units) at `+0x18`, level u32 `+0x3C`, play seconds u32
//! `+0x40`, map id u32 `+0x48`, place-name id u32 `+0x4C`. THE SAME IN THE SAVE FILE (BND4 entry 10, after its
//! 16-byte MD5): bytes at `0x1954`, records of `0x24C` from `0x195E`: name `+0`, level `+0x22`, seconds `+0x26`,
//! map `+0x2E`, place `+0x32`. The file is only ever read.
//!
//! "THE CHARACTER'S NAME" WHEN EVERY SLOT IS SORA: the game's character name stays what its creation gave it (new
//! games are named "Sora" by newgame.rs; older characters keep theirs) and is NOT what tells saves apart: the SLOT
//! is. Sora's file key is `slot<N>_<name>_<hash>`; a save into another slot carries the running character's name
//! there (the game's own routine writes it), so key and slot always agree. Lists show the place, the level and the
//! play time, as KH3's save slots do; the name is carried for the log only.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use kh_core::Sora;

use crate::erlog;

pub const SLOTS: usize = 10;

/// One slot as the game's profile summary describes it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RawSlot {
    pub used: bool,
    pub name: String,
    pub level: u32,
    pub seconds: u32,
    pub map: u32,
    pub place: u32,
}

/// What Sora's own file of the slot says.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SoraSummary {
    pub level: u32,
    pub munny: i32,
    /// Seconds played as Sora.
    pub play_seconds: u64,
    /// The game's play time (ms) the file is paired with.
    pub host_play_ms: u64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlotSummary {
    pub slot: usize,
    /// The game lists a character here.
    pub used: bool,
    pub name: String,
    /// The game's play time of the slot, seconds.
    pub play_seconds: u32,
    /// The place's name as the game's own list shows it ("Stranded Graveyard"); the map's id when it has no name.
    pub location: String,
    pub map: u32,
    pub place: u32,
    /// Sora's level, munny and play time from the slot's `.khsave` (`None`: no file of his for this slot).
    pub sora: Option<SoraSummary>,
    /// The slot saves at save points only (its marker is there).
    pub points_only: bool,
    /// The running session's slot.
    pub current: bool,
}

const MEM_RECORD: usize = 0x2A0;
const FILE_FLAGS: usize = 0x10 + 0x1954;
const FILE_RECORDS: usize = 0x10 + 0x195E;
const FILE_RECORD: usize = 0x24C;

fn utf16(units: &[u16]) -> String {
    let n = units.iter().position(|u| *u == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..n])
}

/// The ten records of a save file's profile-summary entry (`data` = BND4 entry 10 with its MD5). Pure.
pub fn parse_summary_entry(data: &[u8]) -> Option<[RawSlot; SLOTS]> {
    if data.len() < FILE_RECORDS + SLOTS * FILE_RECORD {
        return None;
    }
    let u32_at = |o: usize| u32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]);
    let mut out: [RawSlot; SLOTS] = Default::default();
    for (s, slot) in out.iter_mut().enumerate() {
        let r = FILE_RECORDS + s * FILE_RECORD;
        let units: Vec<u16> = (0..16).map(|k| u16::from_le_bytes([data[r + k * 2], data[r + k * 2 + 1]])).collect();
        *slot = RawSlot { used: data[FILE_FLAGS + s] != 0, name: utf16(&units), level: u32_at(r + 0x22), seconds: u32_at(r + 0x26), map: u32_at(r + 0x2E), place: u32_at(r + 0x32) };
    }
    Some(out)
}

/// Where entry `index` of a BND4 container lies: (offset, size), from the container's first bytes. Pure.
pub fn bnd4_entry(header: &[u8], index: usize) -> Option<(u64, u64)> {
    if header.len() < 0x40 || &header[..4] != b"BND4" {
        return None;
    }
    let u32_at = |o: usize| header.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let u64_at = |o: usize| header.get(o..o + 8).map(|b| u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]));
    let count = u32_at(0x0C)? as usize;
    let (table, entry) = (u64_at(0x10)? as usize, u64_at(0x20)? as usize);
    if index >= count || entry < 0x18 {
        return None;
    }
    let o = table + index * entry;
    Some((u64::from(u32_at(o + 0x10)?), u64_at(o + 8)?))
}

/// The profile summary read from the game's save file (read only, shared; a file the game is writing is tried later).
/// (SAVEPATH) The file is the one the game uses (savepath.rs detects it, any name); `None` while it is not detected: the
/// lists then come from the game's summary in memory only.
fn from_file() -> Option<[RawSlot; SLOTS]> {
    use std::io::{Read, Seek, SeekFrom};
    let path = crate::savegate::save_file_path()?;
    let mut file = std::fs::File::open(path).ok()?;
    let mut header = vec![0u8; 0x400];
    file.read_exact(&mut header).ok()?;
    let (offset, size) = bnd4_entry(&header, 10)?;
    let want = FILE_RECORDS + SLOTS * FILE_RECORD;
    if (size as usize) < want {
        return None;
    }
    let mut data = vec![0u8; want];
    file.seek(SeekFrom::Start(offset)).ok()?;
    file.read_exact(&mut data).ok()?;
    parse_summary_entry(&data)
}

/// The profile summary object of the running game (checked pointers, one guarded read of its ten records).
fn from_memory() -> Option<[RawSlot; SLOTS]> {
    use eldenring::cs::GameDataMan;
    use fromsoftware_shared::FromStatic;
    // SAFETY: game thread; the singleton lives as long as the game; the pointer is checked before it is followed.
    let man = unsafe { GameDataMan::instance() }.ok()? as *const GameDataMan as usize;
    // SAFETY: a pointer-sized field of the live manager (the SDK's private `profile_summary`, + 0x78).
    let object = unsafe { std::ptr::read_unaligned((man + 0x78) as *const usize) };
    let len = SLOTS * MEM_RECORD + 0x18;
    if !crate::game::plausible(object) || !crate::pause::heap_readable(object, len) {
        return None;
    }
    // SAFETY: `len` readable bytes (checked just now); plain data.
    let bytes = unsafe { std::slice::from_raw_parts(object as *const u8, len) };
    let u32_at = |o: usize| u32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
    let mut out: [RawSlot; SLOTS] = Default::default();
    for (s, slot) in out.iter_mut().enumerate() {
        let r = s * MEM_RECORD;
        let units: Vec<u16> = (0..16).map(|k| u16::from_le_bytes([bytes[r + 0x18 + k * 2], bytes[r + 0x19 + k * 2]])).collect();
        *slot = RawSlot { used: bytes[8 + s] != 0, name: utf16(&units), level: u32_at(r + 0x3C), seconds: u32_at(r + 0x40), map: u32_at(r + 0x48), place: u32_at(r + 0x4C) };
    }
    // (a flag byte that is not 0 / 1, or a level beyond the game's, means this is not the object we think it is)
    if (0..SLOTS).any(|s| bytes[8 + s] > 1) || out.iter().any(|r| r.used && !(1..=713).contains(&r.level)) {
        return None;
    }
    Some(out)
}

/// The game's profile summary: its live object when that lists anything, else the save file's.
pub fn raw_slots() -> ([RawSlot; SLOTS], &'static str) {
    // (SV) a slot whose data was deleted is EMPTY in every list of ours, also before the game has written its profile
    // summary without it (the tombstone matches the record the game still lists)
    let (mut raw, source) = raw_slots_as_listed();
    apply_tombstones(&mut raw, &tombstones());
    (raw, source)
}

/// The game's profile summary as the game lists it (no tombstone applied).
fn raw_slots_as_listed() -> ([RawSlot; SLOTS], &'static str) {
    if let Some(memory) = from_memory().filter(|m| m.iter().any(|r| r.used)) {
        return (memory, "the game's profile summary in memory");
    }
    match from_file() {
        Some(file) => (file, "the save file's profile summary"),
        None => (Default::default(), "NOT READABLE (neither the game's summary object nor the save file)"),
    }
}

// ------------------------------------------------------------------------------------------ (SV) deleting a slot
//
// THE USER (2026-10-17): "have the ability to delete save data in the save menu (without saving)". A slot of ours is the
// game's character slot `n` + Sora's files of that slot. A DELETE:
// 1. Sora's files of the slot (`slot<n>_*`: .khsave, .bak, .points, .old*) leave the save folder: moved, byte for byte,
//    into `<save.dir>\deleted\<stamp>_slot<n>\` (kh_host::save::delete_slot_files);
// 2. THE GAME'S SIDE, BY THE GAME'S OWN ROUTE: the slot's "in use" byte of the game's profile summary (in memory,
//    `GameDataMan + 0x78`, + 8 + n: the byte the game's own Load Game list and its New Game read, and the byte
//    titleload.rs already masks) is cleared. The game writes that summary itself with its next profile write (a Save at
//    a save point: the "both" save; the title's system save): then the save file lists the slot as empty. NOTHING IS
//    SAVED BY THE DELETE and the save file is never written by us. The character's own data stays in the file, unused,
//    as after the game's own delete (it lists slots by that byte), until a new character or a save takes the slot;
// 3. until the file says so, a TOMBSTONE `<save.dir>\slot<n>.deleted` (the record the game listed: name, play time, map)
//    keeps the slot empty in every list of ours and in the game's memory: after a restart before the profile was written
//    the byte is cleared again as soon as the summary is in memory. The tombstone goes when the file lists the slot as
//    empty, or the slot holds another record (a new character, a save into it).
// The running session's own slot is NOT deleted (the screen refuses that row): its character is the one in memory, and
// the game would write it back with its next save.

/// (SV) What the game listed for a deleted slot (pure data; one tombstone file each).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tombstone {
    pub slot: usize,
    pub name: String,
    pub seconds: u32,
    pub map: u32,
}

impl Tombstone {
    pub fn of(slot: usize, r: &RawSlot) -> Tombstone {
        Tombstone { slot, name: r.name.clone(), seconds: r.seconds, map: r.map }
    }

    /// The record the game lists for this slot is the one that was deleted.
    pub fn matches(&self, r: &RawSlot) -> bool {
        r.used && r.name == self.name && r.seconds == self.seconds && r.map == self.map
    }

    pub fn to_text(&self) -> String {
        format!("kh-er deleted slot (SV): the game's profile summary still listed this record when its data was deleted; it is listed as empty until the game writes the summary without it.\nslot={}\nname={}\nseconds={}\nmap={}\n", self.slot, self.name, self.seconds, self.map)
    }

    pub fn from_text(text: &str) -> Option<Tombstone> {
        let field = |key: &str| text.lines().find_map(|l| l.strip_prefix(key).and_then(|v| v.strip_prefix('='))).map(str::to_string);
        Some(Tombstone { slot: field("slot")?.parse().ok()?, name: field("name")?, seconds: field("seconds")?.parse().ok()?, map: field("map")?.parse().ok()? })
    }
}

/// (SV, pure) The slots whose record a tombstone names are empty.
pub fn apply_tombstones(raw: &mut [RawSlot; SLOTS], stones: &[Tombstone]) {
    for t in stones {
        if let Some(r) = raw.get_mut(t.slot) {
            if t.matches(r) {
                *r = RawSlot::default();
            }
        }
    }
}

fn tombstone_path(dir: &std::path::Path, slot: usize) -> std::path::PathBuf {
    dir.join(format!("slot{slot}.deleted"))
}

/// (SV) The tombstones in the save folder (read each time: tiny files, read when a list opens and twice a second).
pub fn tombstones() -> Vec<Tombstone> {
    let Some(dir) = crate::save::dir() else { return Vec::new() };
    (0..SLOTS).filter_map(|s| std::fs::read_to_string(tombstone_path(&dir, s)).ok().and_then(|t| Tombstone::from_text(&t)).filter(|t| t.slot == s)).collect()
}

/// (SV) The "in use" byte of slot `n` in the game's profile summary in memory: cleared (true when it was set and is now 0).
fn clear_in_use(n: usize) -> bool {
    use eldenring::cs::GameDataMan;
    use fromsoftware_shared::FromStatic;
    if crate::titleload::masked() || n >= SLOTS {
        return false;
    }
    // SAFETY: game thread; the singleton lives as long as the game; the pointer is checked before it is followed.
    let Some(man) = (unsafe { GameDataMan::instance() }).ok().map(|m| m as *const GameDataMan as usize) else { return false };
    // SAFETY: a pointer-sized field of the live manager (the SDK's private `profile_summary`, + 0x78).
    let object = unsafe { std::ptr::read_unaligned((man + 0x78) as *const usize) };
    if !crate::game::plausible(object) || !crate::pause::heap_readable(object, 0x18) {
        return false;
    }
    // SAFETY: ten readable bytes of the live summary object (checked); each is 0 or 1 (checked before the write).
    let flags = unsafe { std::slice::from_raw_parts_mut((object + 8) as *mut u8, SLOTS) };
    if flags.iter().any(|f| *f > 1) || flags[n] != 1 {
        return false;
    }
    flags[n] = 0;
    true
}

/// (SV) Why a slot cannot be deleted now (`None`: it can).
pub fn delete_refusal(n: usize) -> Option<&'static str> {
    if n >= SLOTS {
        return Some("no such slot");
    }
    if OPS.lock().is_ok_and(|o| o.saving.is_some()) || (crate::game::player_age() > 0 && crate::savegate::busy()) {
        return Some("the game is saving or loading");
    }
    if crate::game::player_age() > 0 && crate::savegate::current_slot() == Some(n as i32) {
        return Some("it is the running game's own slot");
    }
    if !raw_slots().0[n].used {
        return Some("the slot is empty");
    }
    if crate::save::dir().is_none() {
        return Some("Sora's save folder is not known");
    }
    None
}

/// (SV) DELETES SLOT `n`'s DATA (see above). Nothing is saved. `Ok`: the line for the log.
pub fn delete_slot(n: usize) -> Result<String, String> {
    if let Some(why) = delete_refusal(n) {
        return Err(why.to_string());
    }
    let dir = crate::save::dir().ok_or("Sora's save folder is not known")?;
    let (listed, _) = raw_slots_as_listed();
    let record = listed[n].clone();
    let stamp = {
        let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        format!("{s}")
    };
    let moved = kh_host::save::delete_slot_files(&dir, n, &stamp).map_err(|e| format!("Sora's files could not be moved aside: {e}"))?;
    let stone = Tombstone::of(n, &record);
    std::fs::write(tombstone_path(&dir, n), stone.to_text()).map_err(|e| format!("Sora's files were moved aside, but the tombstone could not be written: {e}"))?;
    let cleared = clear_in_use(n);
    forget_continue(n); // (SAVE-DELETE) the game's own delete does it too (0x9a4c8d)
    Ok(format!(
        "SLOT {n} DELETED (\"{}\", the game's play time {} s, map {}): {} file(s) of Sora's moved to {} [{}]; the game's profile summary in memory: {}; NOTHING WAS SAVED",
        record.name,
        record.seconds,
        map_name(record.map),
        moved.len(),
        kh_host::save::trash_dir(&dir, n, &stamp).display(),
        moved.iter().filter_map(|(f, _)| f.file_name().map(|n| n.to_string_lossy().to_string())).collect::<Vec<_>>().join(", "),
        if cleared { "its \"in use\" byte is cleared (the game writes the summary with its next profile write)" } else { "not cleared now (not readable / masked / not set): the tombstone keeps the slot empty in our lists and clears it as soon as it can" }
    ))
}

/// (SV, FRAME_BEGIN, twice a second) The tombstones are kept true: a record the game lists in memory again (a restart
/// before the summary was written) is cleared again; a tombstone whose slot the save file lists as empty, or as another
/// record, is done and goes.
fn tombstone_frame() {
    static NEXT: Mutex<Option<Instant>> = Mutex::new(None);
    let Ok(mut next) = NEXT.lock() else { return };
    if next.is_some_and(|t| t > Instant::now()) {
        return;
    }
    *next = Some(Instant::now() + std::time::Duration::from_millis(500));
    drop(next);
    let stones = tombstones();
    if stones.is_empty() {
        return;
    }
    let Some(dir) = crate::save::dir() else { return };
    let memory = from_memory();
    let file = from_file();
    for t in stones {
        if let Some(m) = memory.as_ref().map(|m| &m[t.slot]) {
            if t.matches(m) && clear_in_use(t.slot) {
                erlog!("slots: (SV) slot {}: the game listed the deleted record \"{}\" in memory again: its \"in use\" byte is cleared again", t.slot, t.name);
            }
        }
        if let Some(f) = file.as_ref().map(|f| &f[t.slot]) {
            if !t.matches(f) {
                let _ = std::fs::remove_file(tombstone_path(&dir, t.slot));
                erlog!("slots: (SV) slot {}: the save file {}: the delete is complete, its tombstone goes", t.slot, if f.used { "holds another record there now" } else { "lists it as empty" });
            }
        }
    }
}

#[cfg(test)]
mod sv_tests {
    use super::*;

    fn rec(name: &str, seconds: u32) -> RawSlot {
        RawSlot { used: true, name: name.into(), level: 5, seconds, map: 0x3C2A_2400, place: 1 }
    }

    #[test]
    fn a_tombstone_empties_only_its_own_record() {
        let mut raw: [RawSlot; SLOTS] = Default::default();
        raw[1] = rec("Sora", 700);
        raw[2] = rec("Sora", 900);
        let stone = Tombstone::of(1, &raw[1]);
        assert_eq!(Tombstone::from_text(&stone.to_text()), Some(stone.clone()));
        let mut listed = raw.clone();
        apply_tombstones(&mut listed, std::slice::from_ref(&stone));
        assert!(!listed[1].used && listed[2] == raw[2]);
        // another record in that slot (a new character, a save into it): not deleted
        let mut again = raw.clone();
        again[1] = rec("Sora", 701);
        apply_tombstones(&mut again, std::slice::from_ref(&stone));
        assert!(again[1].used);
        // a tombstone of another slot number never touches this one
        let mut other = raw.clone();
        apply_tombstones(&mut other, &[Tombstone { slot: 2, ..stone }]);
        assert!(other[1].used && other[2].used);
    }
}

/// `m60_42_36` from a map id (area, block, region, index bytes, highest first).
pub fn map_name(map: u32) -> String {
    if map == u32::MAX || map == 0 {
        return String::new();
    }
    let b = map.to_be_bytes();
    format!("m{:02}_{:02}_{:02}", b[0], b[1], b[2])
}

/// The game's message category of the place names (PlaceName.fmg).
const FMG_PLACE_NAME: u32 = 19;
static PLACES: Mutex<Option<HashMap<u32, Option<String>>>> = Mutex::new(None);

/// The place's name in the game's own words (its PlaceName text of that id), cached.
fn place_name(place: u32) -> Option<String> {
    use eldenring::cs::MsgRepository;
    use fromsoftware_shared::FromStatic;
    if place == 0 || place == u32::MAX {
        return None;
    }
    let mut guard = PLACES.lock().ok()?;
    let cache = guard.get_or_insert_with(HashMap::new);
    if let Some(known) = cache.get(&place) {
        return known.clone();
    }
    // SAFETY: game thread; the singleton lives as long as the game; the SDK checks the tables it walks.
    let text = std::panic::catch_unwind(|| unsafe { MsgRepository::instance() }.ok().and_then(|m| m.get_msg(FMG_PLACE_NAME, place)).map(String::from_utf16_lossy)).ok().flatten().map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    // (a text that is not there yet at the title's start is asked for again later: only answers are kept)
    if text.is_some() {
        cache.insert(place, text.clone());
    }
    text
}

/// WHAT EACH OF THE TEN SLOTS HOLDS (see the module's head). Cheap: one guarded read of the game's summary object
/// (or ~12 KiB of the save file) and up to ten small files of Sora's; call it when a list opens, not every frame.
pub fn slot_summaries() -> Vec<SlotSummary> {
    let (raw, _) = raw_slots();
    let current = if crate::game::player_age() > 0 { crate::savegate::current_slot() } else { None };
    raw.iter()
        .enumerate()
        .map(|(slot, r)| {
            if !r.used {
                return SlotSummary { slot, ..SlotSummary::default() };
            }
            let sora = crate::save::peek(&r.name, slot as i32).map(|(level, munny, play_seconds, host_play_ms)| SoraSummary { level, munny, play_seconds, host_play_ms });
            SlotSummary { slot, used: true, name: r.name.clone(), play_seconds: r.seconds, location: place_name(r.place).unwrap_or_else(|| map_name(r.map)), map: r.map, place: r.place, sora, points_only: crate::save::armed_slot(&r.name, slot as i32), current: current == Some(slot as i32) }
        })
        .collect()
}

// ------------------------------------------------------------------------------------------ (CONT) the latest save
//
// THE USER (2026-11-03): "make 'continue' simply load the latest save game as if it was selected from load game".
// THE LATEST SAVE = of the slots the Load screen lists as used (the game's profile summary, deleted slots taken out),
// the one whose save was WRITTEN LAST: the modification time of its Sora save file (`slot<N>_<name>_<hash>.khsave`),
// which is written with every save of that slot (a save point's Save, the Save screen, and without `save.points_only`
// Elden Ring's own saves of the slot). Ties: the running session's slot, then the lower slot number. The game's play
// time per slot is NOT a save time (two characters' play times say nothing of which was saved last) and the game's one
// save file has one time for all ten slots, so a listed slot without a Sora file cannot be dated and is not "latest".

// (SAVE-DELETE, THE USER 2026-11-07: "continue should load the most recently loaded save game (or most recently saved,
// whichever is most recent) if the most recent is deleted, then continue goes to the next most recent. If there are no
// save games, continue doesn't appear") A SLOT'S TIME = the later of its last save (above) and its LAST LOAD: the
// record `slot<N>_<name>_<hash>.loaded` written each time that character starts (save.rs `on_start` -> [`record_loaded`]).
// It begins with `slot<N>_`, so a delete (ours, the game's own, a new game made over the slot) moves it with the slot's
// other files and a deleted character can never be "latest". A deleted slot is not listed (`used` false) in any case.

/// One listed slot, as the latest-save rule reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dated {
    pub slot: usize,
    pub used: bool,
    /// When its save was last written (`None`: it has no Sora save file to tell).
    pub saved_at: Option<std::time::SystemTime>,
    /// (SAVE-DELETE) When its character was last loaded (`None`: no record of a load).
    pub loaded_at: Option<std::time::SystemTime>,
    pub current: bool,
}

impl Dated {
    /// (SAVE-DELETE) The slot's time: the later of its last save and its last load (`None`: neither is known).
    pub fn time(&self) -> Option<std::time::SystemTime> {
        self.saved_at.max(self.loaded_at)
    }
}

/// THE LATEST SAVE among `rows` (the rule above). `None`: no listed slot has a dated save or load. Pure.
pub fn latest_of(rows: &[Dated]) -> Option<usize> {
    rows.iter().filter(|r| r.used && r.time().is_some()).max_by(|a, b| a.time().cmp(&b.time()).then(a.current.cmp(&b.current)).then(b.slot.cmp(&a.slot))).map(|r| r.slot)
}

/// (SAVE-DELETE) THE TITLE'S CONTINUE: the slot it is to load, or -1 = no Continue (the game then shows none). The latest
/// save / load of the listed slots; with none dated, a listed slot the game itself had as its Continue (`er_last`), else
/// the lowest listed slot; with no listed slot at all, -1. Pure.
pub fn title_continue_of(rows: &[Dated], er_last: i32) -> i32 {
    let listed = |s: i32| rows.iter().any(|r| r.used && r.slot as i32 == s);
    if let Some(latest) = latest_of(rows) {
        return latest as i32;
    }
    if listed(er_last) {
        return er_last;
    }
    rows.iter().filter(|r| r.used).map(|r| r.slot as i32).min().unwrap_or(-1)
}

/// (SAVE-DELETE) The last-load record of a character key in `dir`.
fn loaded_path(dir: &std::path::Path, key: &str) -> std::path::PathBuf {
    dir.join(format!("{key}.loaded"))
}

/// (SAVE-DELETE) The record's text: the Unix seconds of the load (pure).
pub fn loaded_text(at: std::time::SystemTime) -> String {
    format!("kh-er last load of this character (SAVE-DELETE: the title's Continue takes the most recent load or save)\nloaded={}\n", at.duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()))
}

/// (SAVE-DELETE) The time a record names (pure; `None`: not a record).
pub fn loaded_from_text(text: &str) -> Option<std::time::SystemTime> {
    let secs: u64 = text.lines().find_map(|l| l.strip_prefix("loaded="))?.trim().parse().ok()?;
    Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs))
}

/// (SAVE-DELETE; save.rs `on_start`) Character `key` (`slot<N>_...`) was just loaded: its last-load record says now.
pub fn record_loaded(dir: &std::path::Path, key: &str) {
    let path = loaded_path(dir, key);
    let _ = std::fs::create_dir_all(dir);
    match std::fs::write(&path, loaded_text(std::time::SystemTime::now())) {
        Ok(()) => erlog!("slots: the last load of {key} is recorded ({}): the title's Continue ranks it by it", path.display()),
        Err(e) => erlog!("slots: the last load of {key} could NOT be recorded ({e})"),
    }
}

/// When slot `s`'s character was last loaded (its record).
fn loaded_at(s: &SlotSummary) -> Option<std::time::SystemTime> {
    let dir = crate::save::dir()?;
    std::fs::read_to_string(loaded_path(&dir, &kh_host::save::character_key(&s.name, s.slot as i32))).ok().and_then(|t| loaded_from_text(&t))
}

/// CONTINUE'S SLOT: the latest save; without one, the running session's slot (what Continue loaded before 2026-11-03);
/// `None` (no save of any kind is known): the caller does what Continue did then without a slot (Elden Ring's own death
/// and respawn). Pure.
pub fn continue_slot(latest: Option<usize>, session: Option<i32>) -> Option<usize> {
    latest.filter(|n| *n < SLOTS).or_else(|| session.filter(|s| (0..SLOTS as i32).contains(s)).map(|s| s as usize))
}

/// When slot `s`'s save was last written: its Sora save file's modification time.
fn saved_at(s: &SlotSummary) -> Option<std::time::SystemTime> {
    let dir = crate::save::dir()?;
    let path = kh_host::save::save_path(&dir, &kh_host::save::character_key(&s.name, s.slot as i32));
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// THE LATEST SAVE of the running game's lists, and a line for the log (every listed slot's date).
pub fn latest_slot() -> (Option<usize>, String) {
    let rows = dated_rows();
    (latest_of(&rows), describe_dates(&rows))
}

/// (SAVE-DELETE) The listed slots with their save and load times.
fn dated_rows() -> Vec<Dated> {
    slot_summaries().iter().map(|s| Dated { slot: s.slot, used: s.used, saved_at: if s.used { saved_at(s) } else { None }, loaded_at: if s.used { loaded_at(s) } else { None }, current: s.current }).collect()
}

fn describe_dates(rows: &[Dated]) -> String {
    let now = std::time::SystemTime::now();
    let ago = |t: Option<std::time::SystemTime>| t.and_then(|t| now.duration_since(t).ok()).map_or("never".to_string(), |d| format!("{:.0} s ago", d.as_secs_f64()));
    let listed: Vec<String> = rows.iter().filter(|r| r.used).map(|r| format!("slot {}{} saved {}, loaded {}", r.slot, if r.current { " (the session's)" } else { "" }, ago(r.saved_at), ago(r.loaded_at))).collect();
    if listed.is_empty() {
        "no slot is listed".to_string()
    } else {
        listed.join(", ")
    }
}

// ------------------------------------------------------------------------------------------ (SAVE-DELETE) the title's Continue
//
// WHAT THE GAME HAS (eldenring.exe 2.7.1.0 and 2.7.2.0, read from the files; the offline test
// gamever.rs `continue_offline` proves every piece in both): CONTINUE'S SLOT IS ONE i32, `[GameDataMan + 0x60] + 0x1200`
// (fn 0x256330 = `mov rax,[GameDataMan]; mov rax,[rax+60h]; ret`, both versions):
// - the title menu's two Continue lambdas (0x9a9d70, 0x9aa1e0; 2.7.2.0 0x9aa21c.., 0x9aa690..) read it and ask
//   0x876740(slot) = `slot >= 0 && profile summary "in use" byte [summary + 8 + slot]` (0x261ce0): Continue is offered
//   only for a listed slot;
// - the Continue job (0x9ad9b0) loads THAT slot (0x261b90(summary, slot), then 0x827500(.., slot, ..) builds the load);
// - the game writes it when a game starts from the title (TitleStep 0xb0ecc0: `= GameMan::save_slot`) and on a load
//   (0x9aa3f0); ITS OWN DELETE (both delete dialogs, 0x9a4c30 / 0x9a5ff0) clears the slot's "in use" byte (0x262240) and
//   `if lastSlot == slot { lastSlot = -1 }` (no Continue).
// WHAT WE DO: our delete does the game's delete in memory exactly (the byte, already; now the -1 too: [`delete_slot`]),
// and AT THE TITLE the i32 is set to OUR pick (the user's rule, [`title_continue_of`]) whenever the title menu is made
// (title.rs `top_hook`, before the game's code) and while the title's menus run (titleload.rs frame, TitleStep
// MenuJobWait, no load under way). The game's own Continue then offers and loads that slot, or none at -1.

/// The title's Continue follows the user's rule (`title.continue`, default on; 0 = the game's own Continue untouched).
static TITLE_CONTINUE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// (titleload.rs frame) The switch of the latest frame.
pub fn set_title_continue(on: bool) {
    TITLE_CONTINUE.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// `GameDataMan + 0x60`: the menu system's saved data; `+ 0x1200`: Continue's slot (i32, -1 = none).
pub const MENU_DATA: usize = 0x60;
pub const CONTINUE_SLOT: usize = 0x1200;

/// The address of Continue's slot in the running game (checked readable).
fn continue_slot_addr() -> Option<usize> {
    use eldenring::cs::GameDataMan;
    use fromsoftware_shared::FromStatic;
    // SAFETY: game thread; the singleton lives as long as the game; the pointer is checked before it is followed.
    let man = unsafe { GameDataMan::instance() }.ok()? as *const GameDataMan as usize;
    // SAFETY: a pointer-sized field of the live manager.
    let data = unsafe { std::ptr::read_unaligned((man + MENU_DATA) as *const usize) };
    (crate::game::plausible(data) && crate::pause::heap_readable(data + CONTINUE_SLOT, 4)).then_some(data + CONTINUE_SLOT)
}

/// (SAVE-DELETE) The game's own delete in memory: Continue's slot was the deleted one -> -1 (0x9a4c8d..0x9a4c95).
fn forget_continue(n: usize) {
    let Some(at) = continue_slot_addr() else { return };
    // SAFETY: four readable and writable bytes of the live menu data (checked); the game writes the same field the same way.
    unsafe {
        if std::ptr::read_unaligned(at as *const i32) == n as i32 {
            std::ptr::write_unaligned(at as *mut i32, -1);
            erlog!("slots: slot {n} was the game's Continue: it is -1 now, as the game's own delete leaves it");
        }
    }
}

/// (SAVE-DELETE; at the title only) THE TITLE'S CONTINUE IS SET TO THE USER'S RULE: the most recent load or save of the
/// listed slots, -1 (no Continue) without any. `why` for the log (said only when the value changes).
pub fn apply_title_continue(why: &str) {
    if !TITLE_CONTINUE.load(std::sync::atomic::Ordering::Relaxed) || crate::game::player_age() > 0 || crate::titleload::masked() {
        return;
    }
    let Some(at) = continue_slot_addr() else { return };
    // SAFETY: four readable bytes of the live menu data (checked).
    let was = unsafe { std::ptr::read_unaligned(at as *const i32) };
    let rows = dated_rows();
    let pick = title_continue_of(&rows, was);
    if pick != was {
        // SAFETY: four writable bytes of the live menu data (checked); the game itself stores slot numbers and -1 there.
        unsafe { std::ptr::write_unaligned(at as *mut i32, pick) };
        erlog!("slots: THE TITLE'S CONTINUE ({why}): slot {was} -> {} [{}]", if pick < 0 { "NONE (-1: the game offers no Continue)".to_string() } else { format!("slot {pick}") }, describe_dates(&rows));
    }
}

/// The first slot the game lists nothing in (a new game's slot), `None` when all ten are used.
pub fn first_empty() -> Option<usize> {
    raw_slots().0.iter().position(|r| !r.used)
}

pub fn describe(s: &SlotSummary) -> String {
    if !s.used {
        return format!("slot {}: EMPTY", s.slot);
    }
    let (h, m, sec) = (s.play_seconds / 3600, s.play_seconds / 60 % 60, s.play_seconds % 60);
    format!(
        "slot {}: \"{}\" | {} (map {:#010x}, place {}) | the game's play time {h}:{m:02}:{sec:02} | {}{}{}",
        s.slot,
        s.name,
        if s.location.is_empty() { "no place" } else { &s.location },
        s.map,
        s.place,
        match s.sora {
            Some(k) => format!("Sora: level {}, munny {}, {} s played, paired with {:.1} min", k.level, k.munny, k.play_seconds, k.host_play_ms as f64 / 60_000.0),
            None => "no .khsave of Sora's".to_string(),
        },
        if s.points_only { " | saves at save points only" } else { "" },
        if s.current { " | THE RUNNING SESSION'S SLOT" } else { "" },
    )
}

// ------------------------------------------------------------------------------------------ saving into a slot

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveStatus {
    Idle,
    /// The game is writing slot `.0`.
    Busy(usize),
    Done(usize),
    Failed(usize, &'static str),
}

struct Saving {
    slot: usize,
    /// The game's slot before (put back when nothing comes of the save).
    from: i32,
    at: Instant,
}

struct Ops {
    saving: Option<Saving>,
    /// The latest save's outcome, until taken.
    result: Option<Result<usize, (usize, &'static str)>>,
    last: SaveStatus,
}

static OPS: Mutex<Ops> = Mutex::new(Ops { saving: None, result: None, last: SaveStatus::Idle });

/// THE RUNNING SESSION IS SAVED INTO SLOT `n` (0..=9; an empty one or one in use: the caller asks the player before
/// overwriting). Sora's state of this moment is captured for that slot's file; the game is asked for a "both"-type
/// save into the slot through the save gate. `Ok`: under way (`save_status` / `take_save_result` tell the rest).
pub fn save_to_slot(n: usize, sora: &Sora, why: &str) -> Result<(), &'static str> {
    if n >= SLOTS {
        return Err("no such slot");
    }
    let Ok(mut ops) = OPS.lock() else { return Err("locked") };
    if ops.saving.is_some() {
        return Err("a save is under way");
    }
    if crate::game::player_mut().is_none() {
        return Err("no character is loaded");
    }
    let Some(from) = crate::savegate::current_slot() else { return Err("the game has no current save slot") };
    if crate::savegate::busy() {
        return Err("the game is saving or loading");
    }
    // (an outcome nobody took, e.g. of the testing aid `dbg.save`, is not this save's)
    let _ = crate::savegate::take_saved();
    crate::save::slot_save_begin(sora, n as i32)?;
    // (CONT-LADDER) the body's place (the save point) into the game's saved place and our record of this save
    crate::saveplace::before_save(n as i32);
    if !crate::savegate::request_save_slot(n as i32, why) {
        crate::save::slot_save_finish(false);
        crate::saveplace::after_save(false);
        return Err("the game could not be asked to save");
    }
    ops.saving = Some(Saving { slot: n, from, at: Instant::now() });
    ops.result = None;
    ops.last = SaveStatus::Busy(n);
    Ok(())
}

/// Where the latest save into a slot stands (not consumed).
pub fn save_status() -> SaveStatus {
    OPS.lock().map(|o| o.last).unwrap_or(SaveStatus::Idle)
}

/// A save into a slot is under way (the world must run for it: no pause of ours meanwhile).
pub fn saving() -> bool {
    OPS.lock().is_ok_and(|o| o.saving.is_some())
}

/// The outcome of the latest save into a slot, once: `Ok(slot)` = the game's slot AND Sora's file are written.
pub fn take_save_result() -> Option<Result<usize, (usize, &'static str)>> {
    OPS.lock().ok().and_then(|mut o| o.result.take())
}

/// Once a frame (FRAME_BEGIN, after `savegate::frame`): the save under way is followed to its end.
pub fn frame() {
    tombstone_frame(); // (SV)
    crate::saveplace::frame(); // (CONT-LADDER) a KH load's arrival at its save's place
    let Ok(mut ops) = OPS.lock() else { return };
    let Some(saving) = ops.saving.as_ref() else { return };
    let (slot, from, seconds) = (saving.slot, saving.from, saving.at.elapsed().as_secs_f32());
    let outcome = match crate::savegate::take_saved() {
        Some(true) => {
            let sora_written = crate::save::slot_save_finish(true);
            crate::saveplace::after_save(true); // (CONT-LADDER) the place record of this save
            let listed = raw_slots().0[slot].used;
            erlog!("slots: SLOT {slot} SAVED in {seconds:.2} s: the game's slot is written{}; Sora's file {}", if listed { " and its profile summary lists it" } else { " BUT ITS PROFILE SUMMARY DOES NOT LIST IT" }, if sora_written { "is written with it" } else { "could NOT be written" });
            if sora_written { Ok(slot) } else { Err((slot, "the game saved, but Sora's file could not be written")) }
        }
        Some(false) => {
            crate::save::slot_save_finish(false);
            crate::saveplace::after_save(false);
            crate::savegate::set_slot(from);
            erlog!("slots: SLOT {slot} WAS NOT SAVED (the game did not save in 20 s): its current slot is {from} again; nothing of Sora's was written");
            Err((slot, "the game did not save"))
        }
        None => return,
    };
    ops.saving = None;
    ops.last = match outcome {
        Ok(slot) => SaveStatus::Done(slot),
        Err((slot, why)) => SaveStatus::Failed(slot, why),
    };
    ops.result = Some(outcome);
}

// ------------------------------------------------------------------------------------------ loading a slot

/// THE GAME LOADS SLOT `n` by its own routine. In play: `GameMan + 0xB78` (savegate.rs `load_slot`): nothing of the
/// running session survives; Sora, his munny and the slot's save-points marker come from slot `n`'s files. At the
/// title (no world) this route does not exist: the title's own load is driven by the load screen's flow (titleload).
pub fn load_slot(n: usize) -> Result<(), &'static str> {
    if n >= SLOTS {
        return Err("no such slot");
    }
    if OPS.lock().is_ok_and(|o| o.saving.is_some()) {
        return Err("a save is under way");
    }
    let (raw, _) = raw_slots();
    if !raw[n].used {
        return Err("the slot is empty");
    }
    if crate::game::player_mut().is_none() {
        return Err("no world is loaded (the title's own load is needed)");
    }
    let armed = crate::savegate::mode_on() && crate::save::armed_slot(&raw[n].name, n as i32);
    if crate::savegate::load_slot(n as i32, Some(armed), "load of a save slot") {
        Ok(())
    } else {
        Err("the game could not be asked to load (it is saving or loading)")
    }
}

static DBG_SAVE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

/// (testing aid) The slot `dbg.saveto` asked for, once (taken where a Sora is at hand: save.rs `tick`).
pub fn take_dbg_save() -> Option<usize> {
    let n = DBG_SAVE.swap(-1, std::sync::atomic::Ordering::Relaxed);
    (0..SLOTS as i32).contains(&n).then_some(n as usize)
}

/// (testing aids, FRAME_BEGIN) `dbg.slots` (one-shot): the ten slots in the log; `dbg.saveto = N` (one-shot): the
/// running session is saved into slot N through `save_to_slot`. (`dbg.loadslot = N`: savegate.rs, through `load_slot`.)
pub fn aids(control: &crate::control::Control) {
    use crate::control::OneShot;
    static SHOTS: Mutex<Option<(OneShot, OneShot)>> = Mutex::new(None);
    let Ok(mut shots) = SHOTS.lock() else { return };
    let (list, save) = shots.get_or_insert_with(|| (OneShot::new("dbg.slots", control), OneShot::new("dbg.saveto", control)));
    if list.fired(control).is_some() {
        dump();
    }
    if let Some(n) = save.fired(control).and_then(|v| v.trim().parse::<i32>().ok()).filter(|n| (0..SLOTS as i32).contains(n)) {
        DBG_SAVE.store(n, std::sync::atomic::Ordering::Relaxed);
    }
}

/// (testing aid, `dbg.slots`) The ten slots in the log.
pub fn dump() {
    let (_, source) = raw_slots();
    erlog!("slots: from {source}; first empty {:?}", first_empty());
    for s in slot_summaries() {
        erlog!("slots: {}", describe(&s));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry_with(slot: usize, name: &str, level: u32, seconds: u32, map: u32, place: u32) -> Vec<u8> {
        let mut d = vec![0u8; FILE_RECORDS + SLOTS * FILE_RECORD];
        d[FILE_FLAGS + slot] = 1;
        let r = FILE_RECORDS + slot * FILE_RECORD;
        for (k, u) in name.encode_utf16().enumerate() {
            d[r + k * 2..r + k * 2 + 2].copy_from_slice(&u.to_le_bytes());
        }
        d[r + 0x22..r + 0x26].copy_from_slice(&level.to_le_bytes());
        d[r + 0x26..r + 0x2A].copy_from_slice(&seconds.to_le_bytes());
        d[r + 0x2E..r + 0x32].copy_from_slice(&map.to_le_bytes());
        d[r + 0x32..r + 0x36].copy_from_slice(&place.to_le_bytes());
        d
    }

    /// The record layout measured on KHTest.sl2 (er13/sum.js): slot 2 "Sora", level 9, 733 s, m18_00, place 18000.
    #[test]
    fn the_save_files_profile_summary_is_read() {
        let d = entry_with(2, "Sora", 9, 733, 0x1200_0000, 18000);
        let slots = parse_summary_entry(&d).unwrap();
        assert_eq!(slots[2], RawSlot { used: true, name: "Sora".into(), level: 9, seconds: 733, map: 0x1200_0000, place: 18000 });
        assert!(!slots[0].used && !slots[9].used);
        assert_eq!(slots.iter().position(|r| !r.used), Some(0));
        assert!(parse_summary_entry(&d[..d.len() - 1]).is_none());
        assert_eq!(map_name(0x3C2A_2400), "m60_42_36");
        assert_eq!(map_name(u32::MAX), "");
    }

    #[test]
    fn continue_takes_the_latest_save_of_every_listed_slot() {
        use std::time::{Duration, SystemTime};
        let t = |s: u64| Some(SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000 + s));
        let row = |slot, used, saved_at, current| Dated { slot, used, saved_at, loaded_at: None, current };
        // the newest file wins, whichever slot the session is in and whatever their play times
        let rows = [row(0, true, t(50), true), row(3, true, t(90), false), row(5, false, t(500), false), row(7, true, None, false), row(9, true, t(10), false)];
        assert_eq!(latest_of(&rows), Some(3));
        // an empty (or deleted) slot never counts, even with a file left behind; an undated one neither
        assert_eq!(latest_of(&[row(5, false, t(500), false), row(7, true, None, false)]), None);
        // ties: the session's slot, then the lower number
        assert_eq!(latest_of(&[row(2, true, t(5), false), row(6, true, t(5), true), row(8, true, t(5), false)]), Some(6));
        assert_eq!(latest_of(&[row(8, true, t(5), false), row(2, true, t(5), false)]), Some(2));
        // CONTINUE -> the load request: the latest save; without one the session's slot (as before); else none
        assert_eq!(continue_slot(latest_of(&rows), Some(0)), Some(3));
        assert_eq!(continue_slot(None, Some(4)), Some(4));
        assert_eq!(continue_slot(None, Some(-1)), None);
        assert_eq!(continue_slot(None, None), None);
        assert_eq!(continue_slot(Some(12), Some(1)), Some(1));
    }

    /// (SAVE-DELETE, the user 2026-11-07) The title's Continue: the most recent load or save; the top one deleted -> the
    /// next; none left -> no Continue (-1).
    #[test]
    fn the_titles_continue_takes_the_most_recent_load_or_save_and_skips_deleted_slots() {
        use std::time::{Duration, SystemTime};
        let t = |s: u64| Some(SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000 + s));
        let row = |slot, saved_at, loaded_at| Dated { slot, used: true, saved_at, loaded_at, current: false };
        // slot 0 saved at 100 (loaded at 50), slot 1 saved at 80 but LOADED at 120, slot 4 loaded at 110 only
        let mut rows = vec![row(0, t(100), t(50)), row(1, t(80), t(120)), row(4, None, t(110))];
        assert_eq!(title_continue_of(&rows, 0), 1, "the load of slot 1 is the most recent");
        // then slot 0 is saved at 130: it is the most recent
        rows[0].saved_at = t(130);
        assert_eq!(title_continue_of(&rows, 1), 0);
        // slot 0 deleted (not listed any more, whatever its times): the next most recent, slot 1 (120), then slot 4 (110)
        rows[0].used = false;
        assert_eq!(title_continue_of(&rows, 0), 1);
        rows[1].used = false;
        assert_eq!(title_continue_of(&rows, 1), 4);
        // every slot deleted: no Continue
        rows[2].used = false;
        assert_eq!(title_continue_of(&rows, 4), -1);
        assert_eq!(title_continue_of(&[], 3), -1);
        // listed slots without any time (never run with the mod): the game's own Continue when it is listed, else the lowest
        let undated = [row(2, None, None), row(7, None, None)];
        assert_eq!(title_continue_of(&undated, 7), 7);
        assert_eq!(title_continue_of(&undated, 5), 2);
        assert_eq!(title_continue_of(&undated, -1), 2);
        // a dated slot beats the game's own pick
        assert_eq!(title_continue_of(&[row(2, None, None), row(7, None, t(1))], 2), 7);
    }

    /// (SAVE-DELETE) The last-load record: its text round-trips; it is named after the character's key, so a delete of the
    /// slot (Sora's files moved aside) takes it along and leaves every other slot's record where it was.
    #[test]
    fn the_last_load_record_moves_with_its_slot() {
        use std::time::{Duration, SystemTime};
        let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_800_000_000);
        assert_eq!(loaded_from_text(&loaded_text(at)), Some(at));
        assert_eq!(loaded_from_text("nothing here"), None);
        let dir = std::env::temp_dir().join(format!("kh-er-save-delete-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (k3, k1) = (kh_host::save::character_key("Sora", 3), kh_host::save::character_key("Sora", 1));
        record_loaded(&dir, &k3);
        record_loaded(&dir, &k1);
        assert!(loaded_path(&dir, &k3).is_file() && loaded_path(&dir, &k3).file_name().unwrap().to_string_lossy().starts_with("slot3_"));
        let moved = kh_host::save::delete_slot_files(&dir, 3, "test").unwrap();
        assert_eq!(moved.len(), 1);
        assert!(!loaded_path(&dir, &k3).exists() && loaded_path(&dir, &k1).is_file());
        assert!(kh_host::save::trash_dir(&dir, 3, "test").join(format!("{k3}.loaded")).is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bnd4_entry_is_found_from_the_header() {
        let mut h = vec![0u8; 0x400];
        h[..4].copy_from_slice(b"BND4");
        h[0x0C..0x10].copy_from_slice(&12u32.to_le_bytes());
        h[0x10..0x18].copy_from_slice(&0x40u64.to_le_bytes());
        h[0x20..0x28].copy_from_slice(&0x20u64.to_le_bytes());
        let o = 0x40 + 10 * 0x20;
        h[o + 8..o + 0x10].copy_from_slice(&0x60010u64.to_le_bytes());
        h[o + 0x10..o + 0x14].copy_from_slice(&0x019003B0u32.to_le_bytes());
        assert_eq!(bnd4_entry(&h, 10), Some((0x019003B0, 0x60010)));
        assert_eq!(bnd4_entry(&h, 12), None);
        h[0] = b'X';
        assert_eq!(bnd4_entry(&h, 10), None);
    }
}

/// (IMPL_ER15) Whether the SAVE FILE's profile summary lists slot `n` as in use (`None`: the file cannot be read now).
pub fn file_used(n: usize) -> Option<bool> {
    from_file().and_then(|slots| slots.get(n).map(|s| s.used))
}
