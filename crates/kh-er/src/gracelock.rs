//! (SV, the user 2026-10-17 01:00: "when a save point is disabled due to combat, you can still interact with the site of
//! grace, I told you to REMOVE sites of grace so they are never accessible. Fix this") THE GAME'S OWN GRACE ACTIONS ARE
//! SWITCHED OFF AT THEIR SOURCE.
//!
//! THE BUG (the user's picture build/sora_tmp/er/fix4/ref/goofy_grace_glow.jpg: "(Y) :Touch grace" beside Goofy; their
//! log kh-er_final3_session.log 723 s: "IN COMBAT ... the 1 save point(s) in reach FADE OUT", 949 s "THE GAME'S OWN REST
//! STARTED WITHOUT US ... the nearest save point is 0.90 m from him"): the only thing that kept the game's prompt from
//! being answered was savepoint.rs's Y GUARD, which looked at save points that are SHOWN. In combat they are hidden
//! (IMPL_ER57), so near a grace in a fight the guard was off and Y went to the game. And the guard is a pad rule only: the
//! keyboard's E always reached the game.
//!
//! EVERY ROUTE THE PROMPT COMES FROM: the game offers its grace actions ("Touch grace" at an undiscovered grace, "Rest at
//! site of grace" at a discovered one) as ACTION BUTTONS: rows of ActionButtonParam (the prompt text is the row's
//! `textId` in the game's ActionButtonText messages; its region is the row's `radius` / `depth` / `width` / `height`
//! around the grace). The prompt and the press come from those rows for every leader, every input device, every state
//! (combat, undiscovered, the first frames after a load) - so the fix is made there:
//! - THE ROWS ARE FOUND BY THEIR TEXT, from the game's own data: every message of every loaded message file whose text is
//!   one of the grace prompts (`grace.lock.texts`, default "Touch grace|Rest at site of grace|Rest at grace|Site of
//!   Grace|Touch Grace") gives its id; every ActionButtonParam row whose `textId` is one of those ids is a grace action.
//!   `grace.lock.ids = id,id` adds rows by number (MEASURED numbers go there if a language has other words).
//! - THEIR REGION IS SET TO NOTHING (radius, depth, width, height = 0, in memory: the regulation in the game's memory, not
//!   the file): nobody is ever inside it, so the game neither shows the prompt nor takes the press. Applied as soon as the
//!   params are loaded (before the world), checked twice a second (a reload of the params puts them back: they are
//!   written again).
//! - THE ROWS GET THEIR REGION BACK ONLY WHILE OUR OWN FLOWS NEED THE GAME'S REST (the story's driven rest of IMPL_ER33 /
//!   IMPL_ER36 under its black cover, the debugging keys `savepoint.sit = 1` / `savepoint.rest_chord`): `allow`.
//! - `grace.lock = 0`: as before (the rows untouched; the Y guard alone).
//! The Y guard stays as the second line (savepoint_rules.rs: near EVERY grace now, also hidden / undiscovered ones).
//! Discovery is ours (running into the save point writes the discovery flag, IMPL_ER26) and the map / fast travel read
//! that flag, so nothing of those needs the game's "Touch grace".

use std::sync::Mutex;
use std::time::Instant;

use eldenring::cs::{ActionButtonParam, MsgRepository, SoloParamRepository};
use fromsoftware_shared::FromStatic;

use crate::control::Control;
use crate::erlog;

/// The prompt texts of the game's grace actions (English; `grace.lock.texts` replaces them, `|` between).
/// (SV) The grace action rows MEASURED in the game (ActionButtonParam 6100 "Touch grace", 6101 "Rest at site of grace").
pub const MEASURED_IDS: &str = "6100,6101";

pub const TEXTS: &str = "Touch grace|Rest at site of grace|Rest at grace|Touch Grace|Rest at Site of Grace";

/// (pure) Does a message say one of the wanted prompts (trimmed; case and inner spaces as written)?
pub fn is_grace_text(text: &str, wanted: &[&str]) -> bool {
    let t = text.trim();
    wanted.iter().any(|w| t == w.trim())
}

/// (pure) The parsed `grace.lock.ids` list.
pub fn parse_ids(text: Option<&str>) -> Vec<u32> {
    text.unwrap_or("").split(',').filter_map(|t| t.trim().parse::<u32>().ok()).collect()
}

/// One grace action row with its own region (put back with `allow`).
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub id: u32,
    pub text_id: i32,
    pub text: String,
    /// radius, depth, width, height as the game had them.
    pub region: [f32; 4],
}

#[derive(Default)]
struct State {
    rows: Option<Vec<Row>>,
    tried_at: Option<Instant>,
    tries: u32,
    next_check: Option<Instant>,
    /// The rows are set to no region now.
    locked: Option<bool>,
    rewrites: u32,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
static ALLOW: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The game's grace actions are switched off right now (their prompt cannot come: savepoint.rs's rest rule, which reads
/// "the game lets him rest" from that prompt, then reads it from the grace's own flame and our combat rule instead).
pub fn locked() -> bool {
    LOCKED.load(std::sync::atomic::Ordering::Relaxed)
}
static LOCKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// (savepoint.rs, every frame) One of our own flows needs the game's rest now (the story's driven rest, the debugging rest).
pub fn set_allow(on: bool) {
    ALLOW.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// The message ids (of any file) whose text is a grace prompt: (file, id, text).
fn grace_texts(wanted: &[&str]) -> Vec<(u32, u32, String)> {
    let found = std::panic::catch_unwind(|| {
        let mut out = Vec::new();
        // SAFETY: game thread; the singleton lives as long as the game; the SDK checks the tables it walks.
        let Ok(msgs) = (unsafe { MsgRepository::instance() }) else { return out };
        for file in 0..msgs.file_capacity.min(1024) {
            let Some(fmg) = msgs.get_file(file) else { continue };
            for group in fmg.msg_groups().iter().take(100_000) {
                if group.last_id < group.first_id || group.last_id - group.first_id > 2_000_000 {
                    continue;
                }
                for id in group.first_id..=group.last_id {
                    if let Some(units) = fmg.get_msg(id) {
                        if units.len() <= 40 {
                            let text = String::from_utf16_lossy(units);
                            if is_grace_text(&text, wanted) {
                                out.push((file, id, text.trim().to_string()));
                            }
                        }
                    }
                }
            }
        }
        out
    });
    found.unwrap_or_default()
}

/// The grace action rows (by the texts found, plus `extra` ids).
fn find_rows(wanted: &[&str], extra: &[u32]) -> Option<Vec<Row>> {
    let texts = grace_texts(wanted);
    // SAFETY: game thread; the singleton lives as long as the game.
    let repo = unsafe { SoloParamRepository::instance() }.ok()?;
    let mut rows = Vec::new();
    for (id, row) in repo.rows::<ActionButtonParam>() {
        let text_id = row.text_id();
        let by_text = texts.iter().find(|(_, t, _)| text_id >= 0 && *t == text_id as u32);
        if by_text.is_some() || extra.contains(&id) {
            rows.push(Row { id, text_id, text: by_text.map_or_else(|| "(grace.lock.ids)".to_string(), |t| t.2.clone()), region: [row.radius(), row.depth(), row.width(), row.height()] });
        }
    }
    erlog!(
        "gracelock: the game's grace prompts: {} message(s) found [{}]; {} ActionButtonParam row(s) are grace actions: [{}]",
        texts.len(),
        texts.iter().map(|(f, id, t)| format!("file {f} id {id} \"{t}\"")).collect::<Vec<_>>().join(", "),
        rows.len(),
        rows.iter().map(|r| format!("row {} (text {} \"{}\", radius {:.2} depth {:.2} width {:.2} height {:.2})", r.id, r.text_id, r.text, r.region[0], r.region[1], r.region[2], r.region[3])).collect::<Vec<_>>().join(", ")
    );
    Some(rows)
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

/// Once a frame (FRAME_BEGIN, from the first frame: the params are loaded before the world). While [`set_allow`] says one of
/// our own flows needs the game's rest, its rows get their region back.
pub fn frame(control: &Control) {
    let allow = ALLOW.load(std::sync::atomic::Ordering::Relaxed);
    let Ok(mut guard) = STATE.lock() else { return };
    let s = guard.get_or_insert_with(State::default);
    let on = control.on("savepoint.kh") && control.flag("grace.lock") != Some(false) && control.flag("savepoint.sit") != Some(true);
    if s.rows.is_none() {
        // (the SDK panics on a param that is not loaded yet: the look is made once a body is in the world, and guarded; the
        // first look comes in the first frame of the world, before the game could show a prompt)
        if !on || crate::game::player_age() == 0 || s.tried_at.is_some_and(|t| t.elapsed().as_secs_f32() < 2.0) {
            return;
        }
        s.tried_at = Some(Instant::now());
        s.tries += 1;
        let wanted_text = control.str("grace.lock.texts").map(str::to_string).unwrap_or_else(|| TEXTS.to_string());
        let wanted: Vec<&str> = wanted_text.split('|').filter(|t| !t.trim().is_empty()).collect();
        // (MEASURED 2026-10-17, eldenring.exe 2.7.1.0: rows 6100 "Touch grace" and 6101 "Rest at site of grace", both radius 1.5 m,
        // height 3 m, texts 1800 / 1810 of message file 32: taken by number too, whatever the language)
        let extra = parse_ids(control.str("grace.lock.ids").or(Some(MEASURED_IDS)));
        match std::panic::catch_unwind(|| find_rows(&wanted, &extra)).ok().flatten() {
            Some(rows) if !rows.is_empty() => s.rows = Some(rows),
            // (the messages are loaded a little after the params: tried again every 2 s, then said once)
            _ if s.tries == 15 => erlog!("!!! gracelock: NO GRACE ACTION ROW FOUND in 30 s (texts {wanted:?}, ids {extra:?}): the game's grace prompts are NOT switched off; the Y guard is the only line (grace.lock.ids = <ActionButtonParam rows> names them)"),
            _ => {}
        }
        if s.rows.is_none() {
            return;
        }
    }
    let lock = on && !allow;
    let due = s.next_check.is_none_or(|t| Instant::now() >= t);
    if s.locked != Some(lock) || due {
        s.next_check = Some(Instant::now() + std::time::Duration::from_millis(500));
        let rows = s.rows.as_deref().unwrap_or(&[]);
        let changed = std::panic::catch_unwind(|| write_rows(rows, lock)).unwrap_or(0);
        if s.locked != Some(lock) {
            erlog!("gracelock: the game's grace actions ({} row(s)) {}", rows.len(), if lock { "ARE SWITCHED OFF: their region is nothing (no \"Touch grace\", no \"Rest at site of grace\": no prompt, no press, any leader, pad or keyboard)" } else if on { "have their region back: one of our flows needs the game's rest now (the story's driven rest / savepoint.sit / the rest chord)" } else { "are the game's again (grace.lock = 0 / savepoint.kh = 0)" });
        } else if changed > 0 {
            s.rewrites += changed;
            erlog!("gracelock: {changed} grace action row(s) had their region again (a reload of the params?): switched off again ({} so far)", s.rewrites);
        }
        s.locked = Some(lock);
        LOCKED.store(lock, std::sync::atomic::Ordering::Relaxed);
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
    LOCKED.store(false, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grace_prompts_are_known_by_their_words() {
        let wanted: Vec<&str> = TEXTS.split('|').collect();
        assert!(is_grace_text("Touch grace", &wanted));
        assert!(is_grace_text(" Rest at site of grace ", &wanted));
        assert!(!is_grace_text("Open", &wanted));
        assert!(!is_grace_text("Touch grace to rest", &wanted));
        assert_eq!(parse_ids(Some("4000, 4001,x")), vec![4000, 4001]);
        assert!(parse_ids(None).is_empty());
    }
}
