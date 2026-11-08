//! (MUNNY-TEXT, THE USER 2026-11-06: "any mention of Rune in text should be replaced with Munny", then "Runes need to be
//! changed to Munny, and DO NOT need to replace depending on context. Like if someone is talking about Great runes, DO NOT
//! replace the word runes") ELDEN RING'S OWN TEXT, CHANGED IN ITS LOADED MESSAGE TABLES: the currency "runes" read as
//! munny everywhere the game (or our UI, which reads the same tables) shows them.
//!
//! THE ROUTE: the mod reads Elden Ring's text from the game's `MsgRepository` (eroptions, erprompt, gracelock, loot, poise,
//! slots, talkkh, telescope); it ships no message files (no msgbnd in the pack, natives only in the me3 profile). So the
//! text is changed where all of them read it: each loaded FMG keeps a table of 64-bit string offsets from its header
//! (eldenring::cs::FmgFileHeader::msg_offsets, the lookup FmgFile::msg_by_index uses, as the game's own lookup does). A
//! changed entry's offset is pointed at our own UTF-16 copy (kept alive here); nothing of the game's file is overwritten.
//! - THE ROWS: sheets/port/sora-er-munny-text.json, one per changed entry ([`Rule`]: message category, id, exact swaps
//!   `[from, to]`). EVERY `from` must be in the loaded text, or the entry stays as the game has it (another language, a
//!   changed game text: never half a sentence). The classification (currency vs. Great Runes, Rune Arc, item names, lore
//!   runes, Runebear ...) is build/sora_tmp/munny/inventory.csv, made by build/sora_tmp/munny/tools/munny_rules.js from
//!   the engus FMG files (base game and DLC); its ASK THE USER entries are left unchanged.
//! - Checked once a second (title and game): a reload of the tables is changed again. `munny.text = 0` (kh-er.ini) gives
//!   Elden Ring's own words back (the offsets are put back), as does the payload's shutdown ([`release`]).
//! - Our copies are never freed while the game could still hold a pointer from a lookup: a replaced or restored copy is
//!   kept (a few kilobytes).
//!
//! CONFIDENCE: the tables' layout is the SDK's (the same lookup every reader here uses); NOT SEEN IN THE GAME (agents never
//! launch it).

use std::num::NonZero;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use eldenring::cs::{FmgFileHeader, MsgRepository};
use fromsoftware_shared::FromStatic;

use crate::control::Control;
use crate::erlog;

/// The rows (MUNNY-TEXT).
pub const SHEET: &str = include_str!("../../../sheets/port/sora-er-munny-text.json");

/// One changed entry: the message category (the msgbnd's file id: 24 GoodsCaption, 204 GR_Dialogues, 321
/// GoodsCaption_dlc01 ...), the message id and the exact swaps, applied in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub fmg: u32,
    pub id: u32,
    pub swaps: Vec<(String, String)>,
}

/// The sheet: `munny.on` and the rules; problems name rows that could not be read.
#[derive(Clone, Debug, Default)]
pub struct Rules {
    pub on: bool,
    pub rules: Vec<Rule>,
    pub problems: Vec<String>,
}

/// The sheet's rows -> [`Rules`].
pub fn parse(text: &str) -> Rules {
    let mut out = Rules { on: true, ..Rules::default() };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(text) else {
        out.problems.push("the sheet is not JSON".to_string());
        return out;
    };
    for row in json["rows"].as_array().map_or(&[][..], Vec::as_slice) {
        let id = row[0].as_str().unwrap_or("");
        let v = &row[1];
        if id == "munny.on" {
            out.on = v.as_bool().unwrap_or(true);
            continue;
        }
        if !id.starts_with("munny.text.") {
            continue;
        }
        let fmg = v["fmg"].as_u64().and_then(|n| u32::try_from(n).ok());
        let msg = v["id"].as_u64().and_then(|n| u32::try_from(n).ok());
        let swaps: Option<Vec<(String, String)>> = v["swap"].as_array().map(|a| a.iter().map(|s| Some((s[0].as_str()?.to_string(), s[1].as_str()?.to_string()))).collect()).and_then(|s: Option<Vec<_>>| s);
        match (fmg, msg, swaps) {
            (Some(fmg), Some(id), Some(swaps)) if !swaps.is_empty() && swaps.iter().all(|(from, _)| !from.is_empty()) => out.rules.push(Rule { fmg, id, swaps }),
            _ => out.problems.push(format!("{id}: not {{fmg, id, swap: [[from, to], ..]}}")),
        }
    }
    out
}

/// The entry's new text: every swap made (all occurrences of each `from`, in order). `None` when one `from` is not in the
/// text (another language, another game text): the entry is left as it is.
pub fn apply(text: &str, swaps: &[(String, String)]) -> Option<String> {
    let mut out = text.to_string();
    for (from, to) in swaps {
        if !out.contains(from.as_str()) {
            return None;
        }
        out = out.replace(from.as_str(), to);
    }
    (out != text).then_some(out)
}

// ------------------------------------------------------------------------------------------------ the game side

/// A changed entry: where its offset is and what it was.
struct Patch {
    fmg: u32,
    id: u32,
    header: usize,
    index: u32,
    original: i64,
    ours: Vec<u16>,
}

impl Patch {
    fn ours_offset(&self) -> i64 {
        self.ours.as_ptr() as i64 - self.header as i64
    }
}

#[derive(Default)]
struct State {
    rules: Option<Rules>,
    patches: Vec<Patch>,
    /// Copies no table points at any more (kept: a lookup may still hold one).
    retired: Vec<Vec<u16>>,
    next: Option<Instant>,
    told: Option<(bool, usize, usize, usize)>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// A loaded message file: its header's address, the offset table and the table's length.
struct Table {
    header: usize,
    offsets: *mut Option<NonZero<i64>>,
    count: u32,
}

/// The entry `id` of category `fmg`: the table and the entry's index.
fn locate(msgs: &MsgRepository, fmg: u32, id: u32) -> Option<(Table, u32)> {
    let file = msgs.get_file(fmg)?;
    let index = file.msg_index_by_id(id)?;
    let header: &FmgFileHeader = file.header();
    let table = Table { header: std::ptr::from_ref(header) as usize, offsets: header.msg_offsets, count: header.msg_count };
    (!table.offsets.is_null() && table.offsets.is_aligned() && index < table.count).then_some((table, index))
}

/// The offset in the table now (0 = no text).
fn offset_at(table: &Table, index: u32) -> i64 {
    // SAFETY: `index < count` (checked in `locate`); the table is the loaded file's own, aligned and not null.
    unsafe { table.offsets.add(index as usize).read() }.map_or(0, NonZero::get)
}

fn set_offset(table: &Table, index: u32, offset: i64) {
    // SAFETY: as `offset_at`; one aligned 8-byte store, on the game thread (the frame task).
    unsafe { table.offsets.add(index as usize).write(NonZero::new(offset)) }
}

/// The counts of one pass: (entries saying munny, of them newly changed, not found, text not as the rows expect).
fn apply_all(s: &mut State, msgs: &MsgRepository) -> (usize, usize, usize, usize) {
    let (mut held, mut changed, mut missing, mut differs) = (0, 0, 0, 0);
    let rules = s.rules.as_ref().map_or(&[][..], |r| r.rules.as_slice());
    for rule in rules {
        let Some((table, index)) = locate(msgs, rule.fmg, rule.id) else {
            missing += 1;
            continue;
        };
        let current = offset_at(&table, index);
        if let Some(k) = s.patches.iter().position(|p| p.fmg == rule.fmg && p.id == rule.id) {
            if s.patches[k].header == table.header && s.patches[k].index == index && current == s.patches[k].ours_offset() {
                held += 1;
                continue;
            }
            let old = s.patches.swap_remove(k); // (the tables were loaded again)
            s.retired.push(old.ours);
        }
        if current == 0 {
            missing += 1;
            continue;
        }
        let Some(file) = msgs.get_file(rule.fmg) else { continue };
        let Some(text) = file.msg_by_index(index).and_then(|units| String::from_utf16(units).ok()) else {
            differs += 1;
            continue;
        };
        let Some(new) = apply(&text, &rule.swaps) else {
            differs += 1;
            continue;
        };
        let ours: Vec<u16> = new.encode_utf16().chain(std::iter::once(0)).collect();
        let patch = Patch { fmg: rule.fmg, id: rule.id, header: table.header, index, original: current, ours };
        set_offset(&table, index, patch.ours_offset());
        s.patches.push(patch);
        held += 1;
        changed += 1;
    }
    (held, changed, missing, differs)
}

/// Every changed entry gets the game's own offset back (where the table is still the one we changed).
fn restore_all(s: &mut State, msgs: Option<&MsgRepository>) -> usize {
    let mut restored = 0;
    for p in s.patches.drain(..) {
        if let Some((table, index)) = msgs.and_then(|m| locate(m, p.fmg, p.id)) {
            if table.header == p.header && index == p.index && offset_at(&table, index) == p.ours_offset() {
                set_offset(&table, index, p.original);
                restored += 1;
            }
        }
        s.retired.push(p.ours);
    }
    restored
}

/// Once a frame (FRAME_BEGIN, title and game); the work once a second.
pub fn frame(control: &Control) {
    let Ok(mut guard) = STATE.lock() else { return };
    let s = guard.get_or_insert_with(State::default);
    if s.next.is_some_and(|t| Instant::now() < t) {
        return;
    }
    s.next = Some(Instant::now() + Duration::from_secs(1));
    let rules = s.rules.get_or_insert_with(|| {
        let r = parse(SHEET);
        erlog!("ertext: (MUNNY-TEXT) sheet sora-er-munny-text: {} entr(ies) of Elden Ring's text say munny for the currency runes (munny.text = 0: Elden Ring's words){}", r.rules.len(), if r.problems.is_empty() { String::new() } else { format!(" | {} PROBLEM ROW(S): {}", r.problems.len(), r.problems.join("; ")) });
        r
    });
    let on = rules.on && control.flag("munny.text") != Some(false);
    // SAFETY: game thread; the singleton lives as long as the game; the SDK checks the tables it walks.
    let Ok(msgs) = (unsafe { MsgRepository::instance() }) else { return };
    let counts = if on {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| apply_all(s, msgs))).unwrap_or_default()
    } else {
        let restored = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| restore_all(s, Some(msgs)))).unwrap_or_default();
        (0, restored, 0, 0)
    };
    let told = (on, counts.0, counts.2, counts.3);
    if s.told != Some(told) || counts.1 > 0 {
        if on {
            erlog!("ertext: munny: {} entr(ies) say munny ({} changed now); {} not loaded, {} not in the words the rows expect (another language or game text: left as Elden Ring has them)", counts.0, counts.1, counts.2, counts.3);
        } else {
            erlog!("ertext: munny.text = 0: Elden Ring's own words ({} entr(ies) given back)", counts.1);
        }
        s.told = Some(told);
    }
}

/// The payload stops: every changed entry points at the game's own text again.
pub fn release() {
    let Ok(mut guard) = STATE.lock() else { return };
    let Some(s) = guard.as_mut() else { return };
    // SAFETY: as in `frame`.
    let msgs = unsafe { MsgRepository::instance() }.ok();
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| restore_all(s, msgs)));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(rules: &Rules, fmg: u32, id: u32) -> &Rule {
        rules.rules.iter().find(|r| r.fmg == fmg && r.id == id).unwrap_or_else(|| panic!("no row {fmg}:{id}"))
    }

    #[test]
    fn the_sheet_reads_and_every_row_turns_its_game_text_into_the_text_it_names() {
        let rules = parse(SHEET);
        assert!(rules.problems.is_empty(), "{:?}", rules.problems);
        assert!(rules.on);
        assert!(rules.rules.len() > 150, "{} rows", rules.rules.len());
        let json: serde_json::Value = serde_json::from_str(SHEET).unwrap();
        for row in json["rows"].as_array().unwrap().iter().filter(|r| r[0].as_str().unwrap().starts_with("munny.text.")) {
            let r = rule(&rules, row[1]["fmg"].as_u64().unwrap() as u32, row[1]["id"].as_u64().unwrap() as u32);
            let evidence = row[4].as_str().unwrap();
            let old = evidence.split_once("): ").map(|(_, t)| t).unwrap();
            let new = apply(old, &r.swaps).unwrap_or_else(|| panic!("{}:{} does not apply to its own evidence", r.fmg, r.id));
            assert_eq!(new, row[7].as_str().unwrap(), "{}:{}", r.fmg, r.id);
            assert!(new.to_lowercase().contains("munny"), "{}:{}", r.fmg, r.id);
        }
    }

    #[test]
    fn only_the_currency_changes_great_runes_and_names_stay() {
        let rules = parse(SHEET);
        // no NAME file is touched: GoodsName 10 / 319, NpcName 18 / 328, PlaceName 19 / 329, WeaponName 11 / 310 ...
        for r in &rules.rules {
            assert!(![10, 11, 12, 13, 18, 19, 310, 313, 316, 319, 328, 329].contains(&r.fmg), "{}:{} is a name", r.fmg, r.id);
            for (from, to) in &r.swaps {
                assert!(!from.contains("Great Rune") && !from.contains("Rune Arc") && !from.contains("Rune of"), "{}:{} {from:?}", r.fmg, r.id);
                assert!(to.contains("munny") || to.contains("Munny"), "{}:{} {to:?}", r.fmg, r.id);
            }
        }
        // a Golden Rune's caption: its gain line is munny, its lore line is asked (left), and its name is not a row
        let golden = rule(&rules, 24, 2900);
        let text = "Use to gain 200 runes.\n\nRunes are nourishment for the development of any Tarnished";
        assert_eq!(apply(text, &golden.swaps).as_deref(), Some("Use to gain 200 munny.\n\nRunes are nourishment for the development of any Tarnished"));
        // the Great Rune menu's text and Rennala's Great Rune are not rows
        assert!(!rules.rules.iter().any(|r| (r.fmg, r.id) == (200, 10204) || (r.fmg, r.id) == (24, 10080)));
    }

    #[test]
    fn the_hud_menus_prices_and_death_say_munny_with_its_grammar() {
        let rules = parse(SHEET);
        let say = |fmg: u32, id: u32, old: &str| apply(old, &rule(&rules, fmg, id).swaps);
        assert_eq!(say(200, 10201, "Runes Held").as_deref(), Some("Munny Held"));
        assert_eq!(say(204, 231000, "Purchase <?itemName?>\nfor <?demandSoul?> runes").as_deref(), Some("Purchase <?itemName?>\nfor <?demandSoul?> munny"));
        assert_eq!(say(32, 50_000_200, "Retrieve lost runes").as_deref(), Some("Retrieve lost munny"));
        assert_eq!(say(200, 30202, "Number of runes required to raise a level.").as_deref(), Some("Amount of munny required to raise a level."));
        assert_eq!(
            say(206, 50, "You can reclaim those runes by reaching that same spot again.\nHowever, if you die again before reclaiming those dropped runes, they will be lost forever.\nUpon dying, you will drop any runes").as_deref(),
            Some("You can reclaim that munny by reaching that same spot again.\nHowever, if you die again before reclaiming that dropped munny, it will be lost forever.\nUpon dying, you will drop any munny")
        );
        assert_eq!(say(1, 333_006_010, "Care to donate a few runes?").as_deref(), Some("Care to donate a bit of munny?"));
    }

    #[test]
    fn a_text_without_the_words_is_left_whole() {
        let swaps = vec![("drop any runes".to_string(), "drop any munny".to_string()), ("lost runes".to_string(), "lost munny".to_string())];
        assert_eq!(apply("You will drop any runes.", &swaps), None, "the second swap is missing: nothing is changed");
        assert_eq!(apply("Vous perdrez vos runes.", &swaps), None);
        assert_eq!(apply("drop any runes, the lost runes", &swaps).as_deref(), Some("drop any munny, the lost munny"));
        assert!(parse("[").problems.len() == 1);
        assert_eq!(parse(r#"{"rows":[["munny.text.1.2",{"fmg":1,"id":2,"swap":[]},"text"]]}"#).problems.len(), 1);
    }
}
