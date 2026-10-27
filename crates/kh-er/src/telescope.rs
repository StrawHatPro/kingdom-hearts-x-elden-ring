//! (TELESCOPE, the user 2026-10-27: "disable using eagle eye telescopes entirely, give no prompt for them") THE GAME'S
//! BIRDSEYE TELESCOPES ARE SWITCHED OFF AT THEIR SOURCE, for whoever leads (Sora, Donald, Goofy, the Tarnished), pad or
//! keyboard.
//!
//! WHAT THE GAME DOES: a telescope (the cliff edges', the towers' ...) offers "Use Birdseye Telescope" as an ACTION BUTTON,
//! a row of ActionButtonParam: the prompt text is the row's `textId` in the game's ActionButtonText messages (msgbnd file
//! 32 / 365 / 465, as erprompt.rs reads them), its region is the row's `radius` / `depth` / `width` / `height` around the
//! telescope. The prompt and the press both come from that region (the same route gracelock.rs closes for the graces).
//!
//! WHAT WE DO (gracelock.rs's way, no list of places: every telescope of every map uses those rows):
//! - THE ROWS ARE FOUND BY THEIR TEXT, from the game's own data: every ActionButtonParam row whose `textId` is
//!   ActionButtonText [`TEXT_IDS`] (9270 "Use Birdseye Telescope" in build/sora_tmp/er/conv/game/txt/all/ActionButtonText.tsv,
//!   the same id in every language), or whose ActionButtonText line names a telescope ([`WORDS`], looked up in the
//!   ActionButtonText files only, so another file's "Birdseye Telescope" lines are never taken).
//!   `telescope.text_ids = id,id` / `telescope.words = a|b` replace them.
//! - THEIR REGION IS SET TO NOTHING (radius, depth, width, height = 0, in memory: the regulation in the game's memory, not
//!   the file): nobody is ever inside it, so the game neither shows the prompt nor takes the press. Applied once a body
//!   is in the world, checked twice a second (a reload of the params puts them back: they are written again).
//! - The second line (sora.rs `watch_ui`): a telescope prompt the manager still shows is no prompt for us (no KH3 command,
//!   Y never reaches the game for it), as the ladder's "Descend".
//! - `telescope.er = 1`: the game's telescopes as before (the rows get their region back). Default: switched off.
//! (ActionButtonText 9240 "Stargaze" is NOT a Birdseye Telescope and is left alone; `telescope.text_ids = 9270,9240`
//! would take it too.)

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{ActionButtonParam, MsgRepository, SoloParamRepository};
use fromsoftware_shared::FromStatic;

use crate::control::Control;
use crate::erlog;

/// ActionButtonText ids of the telescope prompt (9270 "Use Birdseye Telescope"; `telescope.text_ids` replaces them).
pub const TEXT_IDS: &str = "9270";
/// Words that make an ActionButtonText line a telescope prompt (case-insensitive, `|` between; `telescope.words`).
pub const WORDS: &str = "telescope";
/// ActionButtonText msgbnd files: base, dlc01, dlc02 (erprompt.rs `TEXT_FILES`).
const TEXT_FILES: [u32; 3] = [32, 365, 465];

/// (pure) The telescopes are switched off (`telescope.er` absent or 0: off; 1: the game's own).
pub fn wanted_off(er_key: Option<bool>) -> bool {
    er_key != Some(true)
}

/// (pure) Is an action button with this ActionButtonText id (and its line, when read) a telescope prompt?
pub fn is_telescope(text_id: i32, text: Option<&str>, ids: &[i32], words: &[&str]) -> bool {
    if text_id >= 0 && ids.contains(&text_id) {
        return true;
    }
    let Some(t) = text else { return false };
    let t = t.to_lowercase();
    words.iter().map(|w| w.trim().to_lowercase()).any(|w| !w.is_empty() && t.contains(&w))
}

/// (pure) The parsed id list (`telescope.text_ids`).
pub fn parse_ids(text: &str) -> Vec<i32> {
    text.split(',').filter_map(|t| t.trim().parse::<i32>().ok()).collect()
}

/// (pure) The parsed word list (`telescope.words`).
pub fn parse_words(text: &str) -> Vec<&str> {
    text.split('|').map(str::trim).filter(|t| !t.is_empty()).collect()
}

/// The rule from the keys (ids, words).
fn rule(control: &Control) -> (Vec<i32>, String) {
    (parse_ids(control.str("telescope.text_ids").unwrap_or(TEXT_IDS)), control.str("telescope.words").unwrap_or(WORDS).to_string())
}

/// (sora.rs `watch_ui`, every frame) The prompt the game shows is a telescope's and the telescopes are switched off.
pub fn hides_prompt(control: &Control, text_id: i32, text: Option<&str>) -> bool {
    if !wanted_off(control.flag("telescope.er")) {
        return false;
    }
    let (ids, words) = rule(control);
    is_telescope(text_id, text, &ids, &parse_words(&words))
}

/// One telescope row with its own region (put back with `telescope.er = 1` / on release).
#[derive(Clone, Debug, PartialEq)]
struct Row {
    id: u32,
    text_id: i32,
    /// radius, depth, width, height as the game had them.
    region: [f32; 4],
}

#[derive(Default)]
struct State {
    rows: Option<Vec<Row>>,
    tried_at: Option<Instant>,
    tries: u32,
    next_check: Option<Instant>,
    locked: Option<bool>,
    rewrites: u32,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// The game's own ActionButtonText line of a text id.
fn message(msgs: &MsgRepository, id: i32) -> Option<String> {
    if id < 0 {
        return None;
    }
    TEXT_FILES.iter().find_map(|file| {
        let units = msgs.get_msg(*file, id as u32)?;
        let end = units.iter().position(|c| *c == 0).unwrap_or(units.len()).min(200);
        let t = String::from_utf16_lossy(&units[..end]).trim().to_string();
        (!t.is_empty() && !t.contains("[ERROR]")).then_some(t)
    })
}

/// The telescope rows, and how many rows' texts could be read (0: the messages are not loaded yet).
fn find_rows(ids: &[i32], words: &[&str]) -> Option<(Vec<Row>, usize)> {
    // SAFETY: game thread; the singletons live as long as the game; the SDK checks the tables it walks.
    let repo = unsafe { SoloParamRepository::instance() }.ok()?;
    let msgs = unsafe { MsgRepository::instance() }.ok();
    let mut rows = Vec::new();
    let mut read = 0;
    for (id, row) in repo.rows::<ActionButtonParam>() {
        let text_id = row.text_id();
        let text = msgs.and_then(|m| message(m, text_id));
        read += usize::from(text.is_some());
        if is_telescope(text_id, text.as_deref(), ids, words) {
            rows.push(Row { id, text_id, region: [row.radius(), row.depth(), row.width(), row.height()] });
        }
    }
    Some((rows, read))
}

/// Writes the rows: no region (`lock`) or their own; how many differed from what was wanted.
fn write_rows(rows: &[Row], lock: bool) -> u32 {
    // SAFETY: game thread; the singleton lives as long as the game; rows by id, each written as the SDK's setters do.
    let Ok(repo) = (unsafe { SoloParamRepository::instance_mut() }) else { return 0 };
    let mut changed = 0;
    for r in rows {
        let Some(row) = repo.get_mut::<ActionButtonParam>(r.id) else { continue };
        let want = if lock { [0.0; 4] } else { r.region };
        if [row.radius(), row.depth(), row.width(), row.height()] != want {
            row.set_radius(want[0]);
            row.set_depth(want[1]);
            row.set_width(want[2]);
            row.set_height(want[3]);
            changed += 1;
        }
    }
    changed
}

/// Once a frame (FRAME_BEGIN, beside gracelock): the rows are found once a body is in the world, then kept switched off.
pub fn frame(control: &Control) {
    let Ok(mut guard) = STATE.lock() else { return };
    let s = guard.get_or_insert_with(State::default);
    let off = wanted_off(control.flag("telescope.er"));
    if s.rows.is_none() {
        // (the SDK panics on a param that is not loaded yet: the look is made once a body is in the world, and guarded)
        if !off || crate::game::player_age() == 0 || s.tried_at.is_some_and(|t| t.elapsed().as_secs_f32() < 2.0) {
            return;
        }
        s.tried_at = Some(Instant::now());
        s.tries += 1;
        let (ids, words_text) = rule(control);
        let words = parse_words(&words_text);
        match std::panic::catch_unwind(|| find_rows(&ids, &words)).ok().flatten() {
            // (the messages are loaded a little after the params: the rows by text are taken once the texts are readable;
            // by id alone after 30 s)
            Some((rows, read)) if !rows.is_empty() && (read > 0 || s.tries >= 15) => {
                erlog!(
                    "telescope: (TELESCOPE) {} Birdseye Telescope prompt(s) found in ActionButtonParam (text ids {ids:?}, words {words:?}): rows [{}]",
                    rows.len(),
                    rows.iter().map(|r| format!("{} (text {}, radius {:.2} depth {:.2} width {:.2} height {:.2})", r.id, r.text_id, r.region[0], r.region[1], r.region[2], r.region[3])).collect::<Vec<_>>().join(", ")
                );
                s.rows = Some(rows);
            }
            _ if s.tries == 15 => erlog!("!!! telescope: NO TELESCOPE ACTION ROW FOUND in 30 s (text ids {ids:?}, words {words:?}): the game's telescopes are NOT switched off; only the prompt filter is left (telescope.text_ids = <ActionButtonText ids> names them)"),
            _ => {}
        }
        if s.rows.is_none() {
            return;
        }
    }
    let due = s.next_check.is_none_or(|t| Instant::now() >= t);
    if s.locked != Some(off) || due {
        s.next_check = Some(Instant::now() + std::time::Duration::from_millis(500));
        let rows = s.rows.as_deref().unwrap_or(&[]);
        let changed = std::panic::catch_unwind(|| write_rows(rows, off)).unwrap_or(0);
        if s.locked != Some(off) {
            erlog!("telescope: the game's Birdseye Telescopes ({} prompt row(s)) {}", rows.len(), if off { "ARE SWITCHED OFF: their region is nothing (no \"Use Birdseye Telescope\": no prompt, no press, any leader, pad or keyboard; telescope.er = 1 gives them back)" } else { "are the game's again (telescope.er = 1)" });
        } else if changed > 0 {
            s.rewrites += changed;
            erlog!("telescope: {changed} telescope row(s) had their region again (a reload of the params?): switched off again ({} so far)", s.rewrites);
        }
        s.locked = Some(off);
    }
}

/// The payload stops: the rows get their own region back.
pub fn release() {
    if let Ok(mut guard) = STATE.lock() {
        if let Some(s) = guard.as_mut() {
            if let Some(rows) = s.rows.as_deref() {
                let _ = std::panic::catch_unwind(|| write_rows(rows, false));
            }
            s.locked = Some(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telescope_the_prompt_is_known_by_its_text_id_and_its_words() {
        let ids = parse_ids(TEXT_IDS);
        let words = parse_words(WORDS);
        assert_eq!(ids, vec![9270]);
        // by id, whatever the language (or with the messages not loaded yet)
        assert!(is_telescope(9270, None, &ids, &words));
        assert!(is_telescope(9270, Some("Fernrohr benutzen"), &ids, &words));
        // by its line, any case
        assert!(is_telescope(12345, Some("Use Birdseye Telescope"), &ids, &words));
        assert!(is_telescope(12345, Some("use birdseye TELESCOPE"), &ids, &words));
        // not a telescope
        assert!(!is_telescope(9240, Some("Stargaze"), &ids, &words));
        assert!(!is_telescope(1800, Some("Touch grace"), &ids, &words));
        assert!(!is_telescope(3301, Some("Descend"), &ids, &words));
        assert!(!is_telescope(-1, None, &ids, &words));
        assert!(!is_telescope(-1, Some(""), &ids, &[]), "no words: no match by text");
        assert_eq!(parse_ids("9270, 9240,x"), vec![9270, 9240]);
        assert_eq!(parse_words(" telescope | | scope "), vec!["telescope", "scope"]);
    }

    #[test]
    fn telescope_switched_off_by_default() {
        assert!(wanted_off(None), "absent: off");
        assert!(wanted_off(Some(false)));
        assert!(!wanted_off(Some(true)), "telescope.er = 1: the game's own");
    }

    /// Every line of the game's ActionButtonText (the extracted copy, when present): only "Use Birdseye Telescope" is taken.
    #[test]
    fn telescope_only_the_birdseye_line_of_action_button_text() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/er/conv/game/txt/all");
        let (ids, words) = (parse_ids(TEXT_IDS), parse_words(WORDS));
        let mut taken = Vec::new();
        for file in ["ActionButtonText.tsv", "ActionButtonText_dlc01.tsv"] {
            let Ok(text) = std::fs::read_to_string(root.join(file)) else { continue };
            for line in text.lines() {
                let mut cols = line.split('\t');
                let (Some(id), Some(t)) = (cols.next().and_then(|i| i.trim().parse::<i32>().ok()), cols.next()) else { continue };
                if is_telescope(id, Some(t), &ids, &words) {
                    taken.push((id, t.trim().to_string()));
                }
            }
        }
        if !taken.is_empty() {
            assert_eq!(taken, vec![(9270, "Use Birdseye Telescope".to_string())]);
        }
    }
}
