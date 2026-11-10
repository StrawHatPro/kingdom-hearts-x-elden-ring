//! (SUMMON-POOL, the user 2026-11-10: "Disable summoning pools") ELDEN RING'S SUMMONING POOLS (the Martyr Effigies) CAN
//! NEVER BE ACTIVATED: no prompt, no press, no "This summoning pool is now functional", for every pool, whoever leads, pad
//! or keyboard. The statues stay where they are (scenery).
//!
//! WHAT THE GAME DOES (READ in eldenring.exe 2.7.2.0, the copy build/sora_tmp/quit_crash/eldenring_272.exe; notes and tools
//! build/sora_tmp/summon_pool/STATE.md):
//! - A pool is an ASSET with the module `CS::CSGeomSignPuddleOpenActionButtonModule` (its function names are the strings at
//!   0x142a8ca10 / 0x142a8ca70 / 0x142a8cac0). Its constructor 0x1406e8b40 reads the pool's event flag (state 0xb when it is
//!   already set, 1 when not); its SetEnableCheckRegion 0x1406e8c00 asks for the action button only while the flag is NOT set; its
//!   `_CheckActionButtonRegion` 0x1406e8db0 asks the action-button region system whether the player stands in the region of
//!   ACTIONBUTTON ROW 9280 around the asset (0x140a67620 with r9d = 0x2440, at 0x1406e8e8f) and whether that button was pressed
//!   (0x140a673d0 with edx = 0x2440, at 0x1406e8f4e); only then it sets the flag (0x1405d3410), plays the asset's state 0xd
//!   and shows its message.
//! - The pool asset's second module (constructor 0x1406e4470: +0x14 = 0x2440, its check 0x1406e4700 makes the same two
//!   calls with it and sets event flag 2230 on a press). Both modules are made only for the asset whose id % 1000000 is
//!   GameSystemCommonParam.sign_puddle_asset_id (+0x1d4; the regulation: 99015 = AEG099_015, the Martyr Effigy; the asset
//!   factory 0x1406bab80).
//! - Row 9280 is the pools' own: every other 0x2440 immediate of the exe's code is an allocation size or a deleting
//!   destructor's size (0x140505edb, 0x14071555a) or no immediate at all; no event script (940 extracted .emevd: every
//!   IfActionButtonInArea, 3[24]) and no ObjActParam row uses it. Its text is ActionButtonText 1000 "Examine" (shared with
//!   other rows: the row, not the text, is what is matched).
//! - The pools' flags are the SignPuddleParam row ids (670100 ...; common.emevd event 6909 sets some of them).
//!
//! WHAT WE DO (telescope.rs's way: the game's own data, no list of places):
//! - ROW 9280'S REGION IS SET TO NOTHING (radius, depth, width, height = 0, in the regulation in memory, not the file): nobody
//!   is ever inside it, so `_CheckActionButtonRegion` (and the second module's check 0x1406e4700) never shows the prompt nor takes a press, the flag is never set, no message.
//!   Applied once a body is in the world, checked twice a second (a reload of the params puts it back: written again).
//! - The second line (sora.rs `watch_ui`): a prompt of row 9280 the manager still shows is no prompt for us (no KH3 command, Y
//!   never reaches the game for it).
//! - A pool already activated in a save: its module asks for no button at all (SetEnableCheckRegion), so nothing changes for it; the
//!   game's own activated-pool state is not touched (no flag, no save written).
//! - The row, the switch and the evidence: sheets/port/kh-er-summon-pools.json. `pools.er = 1`: the game's pools as before.

use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use eldenring::cs::{ActionButtonParam, SoloParamRepository};
use fromsoftware_shared::FromStatic;

use crate::control::Control;
use crate::erlog;

const SHEET: &str = include_str!("../../../sheets/port/kh-er-summon-pools.json");

/// What the sheet says (pure data).
#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    /// ActionButtonParam row of the pools' activation.
    pub row: u32,
    /// The pools are switched off by default.
    pub off_by_default: bool,
    /// The control key that gives them back.
    pub key: String,
}

/// (pure) The sheet's rule.
pub fn parse(text: &str) -> Result<Rule, String> {
    let j: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let rows = j.get("rows").and_then(|r| r.as_array()).ok_or("no rows")?;
    let value = |id: &str| rows.iter().find(|r| r.get(0).and_then(|v| v.as_str()) == Some(id)).and_then(|r| r.get(1)).cloned();
    let button = value("pools.action_button").ok_or("no pools.action_button row")?;
    let row = button.get("row").and_then(|v| v.as_u64()).and_then(|v| u32::try_from(v).ok()).ok_or("pools.action_button: no row")?;
    let switch = value("pools.switch").ok_or("no pools.switch row")?;
    let off_by_default = switch.get("off_by_default").and_then(|v| v.as_bool()).ok_or("pools.switch: no off_by_default")?;
    let key = switch.get("key").and_then(|v| v.as_str()).ok_or("pools.switch: no key")?.to_string();
    Ok(Rule { row, off_by_default, key })
}

/// The sheet's rule (read once; a bad sheet: row 9280, off, `pools.er`, logged).
pub fn rule() -> &'static Rule {
    static RULE: OnceLock<Rule> = OnceLock::new();
    RULE.get_or_init(|| {
        parse(SHEET).unwrap_or_else(|e| {
            erlog!("pools: sheets/port/kh-er-summon-pools.json unreadable ({e}): row 9280, switched off, key pools.er");
            Rule { row: 9280, off_by_default: true, key: "pools.er".to_string() }
        })
    })
}

/// (pure) The pools are switched off (`key` absent: the sheet's default; 1: the game's own; 0: off).
pub fn wanted_off(er_key: Option<bool>, off_by_default: bool) -> bool {
    match er_key {
        Some(true) => false,
        Some(false) => true,
        None => off_by_default,
    }
}

fn off(control: &Control) -> bool {
    let r = rule();
    wanted_off(control.flag(&r.key), r.off_by_default)
}

/// (sora.rs `watch_ui`, every frame) The prompt the game shows is a summoning pool's and the pools are switched off.
pub fn hides_prompt(control: &Control, row: i32) -> bool {
    row >= 0 && row as u32 == rule().row && off(control)
}

#[derive(Default)]
struct State {
    /// The row's own region (radius, depth, width, height) as the game had it; None: not read yet.
    region: Option<[f32; 4]>,
    tried_at: Option<Instant>,
    tries: u32,
    next_check: Option<Instant>,
    locked: Option<bool>,
    rewrites: u32,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// The row's region now (None: the param is not loaded or the row is missing).
fn read_region(row: u32) -> Option<([f32; 4], i32)> {
    // SAFETY: game thread; the singleton lives as long as the game; the SDK checks the tables it walks.
    let repo = unsafe { SoloParamRepository::instance() }.ok().filter(|r| crate::game::param_loaded::<ActionButtonParam>(r))?;
    let r = repo.get::<ActionButtonParam>(row)?;
    Some(([r.radius(), r.depth(), r.width(), r.height()], r.text_id()))
}

/// Writes the row's region: nothing (`lock`) or its own. True: it differed.
fn write_region(row: u32, own: [f32; 4], lock: bool) -> bool {
    // SAFETY: as telescope.rs `write_rows`: game thread; never into a holder without its res cap (the game's exit).
    let Some(repo) = (unsafe { SoloParamRepository::instance_mut() }).ok().filter(|r| crate::game::param_loaded::<ActionButtonParam>(r)) else { return false };
    let Some(r) = repo.get_mut::<ActionButtonParam>(row) else { return false };
    let want = if lock { [0.0; 4] } else { own };
    if [r.radius(), r.depth(), r.width(), r.height()] == want {
        return false;
    }
    r.set_radius(want[0]);
    r.set_depth(want[1]);
    r.set_width(want[2]);
    r.set_height(want[3]);
    true
}

/// Once a frame (FRAME_BEGIN, beside telescope.rs): the row is read once a body is in the world, then kept switched off.
pub fn frame(control: &Control) {
    let Ok(mut guard) = STATE.lock() else { return };
    let s = guard.get_or_insert_with(State::default);
    let off = off(control);
    let row = rule().row;
    if s.region.is_none() {
        // (the SDK panics on a param that is not loaded yet: the look is made once a body is in the world, and guarded)
        if !off || crate::game::player_age() == 0 || s.tried_at.is_some_and(|t| t.elapsed().as_secs_f32() < 2.0) {
            return;
        }
        s.tried_at = Some(Instant::now());
        s.tries += 1;
        match std::panic::catch_unwind(|| read_region(row)).ok().flatten() {
            Some((region, text_id)) => {
                erlog!("pools: (SUMMON-POOL) the summoning pools' action button is ActionButtonParam row {row} (text {text_id}, radius {:.2} depth {:.2} width {:.2} height {:.2}; the exe's CSGeomSignPuddleOpenActionButtonModule asks for this row only)", region[0], region[1], region[2], region[3]);
                s.region = Some(region);
            }
            None if s.tries == 15 => erlog!("!!! pools: ActionButtonParam row {row} NOT FOUND in 30 s: the summoning pools are NOT switched off (only the prompt filter is left)"),
            None => return,
        }
    }
    let Some(own) = s.region else { return };
    let due = s.next_check.is_none_or(|t| Instant::now() >= t);
    if s.locked != Some(off) || due {
        s.next_check = Some(Instant::now() + std::time::Duration::from_millis(500));
        let changed = std::panic::catch_unwind(|| write_region(row, own, off)).unwrap_or(false);
        if s.locked != Some(off) {
            erlog!("pools: the game's summoning pools {}", if off { "ARE SWITCHED OFF: their action button's region is nothing (no \"Examine\" at a Martyr Effigy, no activation, no \"This summoning pool is now functional\"; pools already active in the save stay as they are; pools.er = 1 gives them back)" } else { "are the game's again (pools.er = 1)" });
        } else if changed {
            s.rewrites += 1;
            erlog!("pools: the pools' action button had its region again (a reload of the params?): switched off again ({} so far)", s.rewrites);
        }
        s.locked = Some(off);
    }
}

/// The payload stops: the row gets its own region back.
pub fn release() {
    if let Ok(mut guard) = STATE.lock() {
        if let Some(s) = guard.as_mut() {
            if let Some(own) = s.region {
                let row = rule().row;
                let _ = std::panic::catch_unwind(|| write_region(row, own, false));
            }
            s.locked = Some(false);
        }
    }
}

/// (sora.rs `watch_ui`) The log line of a pool prompt taken away (once per sighting).
pub fn told_line(row: u32) -> String {
    format!("pools: (SUMMON-POOL) the game offers a summoning pool (ActionButtonParam row {row}): NOT OFFERED (no command, Y is not the game's; pools.er = 1 gives it back)")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sheet holds the row the exe's pool module asks for (0x2440 at 0x1406e8e8f / 0x1406e8f4e), switched off by default.
    #[test]
    fn pools_the_sheet_names_row_9280_switched_off() {
        let r = parse(SHEET).expect("the sheet parses");
        assert_eq!(r, Rule { row: 9280, off_by_default: true, key: "pools.er".to_string() });
        assert_eq!(r.row, 0x2440);
        assert!(parse("{}").is_err());
    }

    #[test]
    fn pools_switched_off_unless_the_key_gives_them_back() {
        assert!(wanted_off(None, true), "absent: the sheet's default (off)");
        assert!(wanted_off(Some(false), true));
        assert!(!wanted_off(Some(true), true), "pools.er = 1: the game's own");
        assert!(!wanted_off(None, false));
    }

    /// Row 9280 in the regulation (the extracted dump, when present): "Examine" (text 1000), its own region; no other row of
    /// the dump is the pools' (matched by row, never by the shared text).
    #[test]
    fn pools_row_9280_in_the_regulation_dump() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/summon_pool/ab_2720.txt");
        let Ok(text) = std::fs::read_to_string(path) else { return };
        let line = text.lines().find(|l| l.starts_with("9280 ")).expect("row 9280 in the dump");
        assert!(line.contains("text 1000 \"Examine\"") && line.contains("r 1.00") && line.contains("h 1.50"), "{line}");
        assert_eq!(text.lines().filter(|l| l.starts_with("9280 ")).count(), 1);
    }
}
