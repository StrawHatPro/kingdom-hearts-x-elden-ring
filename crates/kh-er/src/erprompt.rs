//! (RC, the user 2026-10-20: "Make every Y interaction you do in the game use the same HUD element that Save now uses";
//! the coordinator's addition: "Will talking have the Y interaction? It should for every character, including our custom
//! Donald interaction") EVERY Y INTERACTION AS KH3'S GREEN REACTION COMMAND. State note:
//! build/sora_tmp/party/IMPL_RC_STATE.md.
//!
//! WHAT ELDEN RING DOES (eldenring.exe 2.7.1.0, READ in the file with iced-x86, build/sora_tmp/party/rc/tools/erdis.js;
//! not seen in the game):
//! - Every object, character and talk script that offers an action asks `CSActionButtonManImp` each frame (the SDK's
//!   `execute_action_button`, RVA 0xA641C0, behind the locking wrapper 0xA645E0): the ActionButtonParam row is read
//!   (text id `+0x34`, category `+0x01`, text box type `+0x30`), and a CANDIDATE is put into the manager's lists
//!   (0xA653B0: a 0x78-byte node; `+0x08` row id, `+0x0C` category, `+0x10` TEXT ID (the row's plus the caller's offset),
//!   `+0x14` priority, `+0x18` distance, `+0x20` flags, the lists at manager `+0x08 / +0x10 / +0x18`, sorted by priority
//!   then distance).
//! - The manager's frame update (0xA64850) picks the one shown: manager `+0x20` = THAT CANDIDATE, `+0x2C` = ITS TEXT ID
//!   (set to -1 first every update; the candidate's own text, or the override of its `+0x48 / +0x4C`), `+0x2A` = shown,
//!   `+0x2B` = greyed. The press is answered for the candidate whose flag 4 is set (0xA64FA0, manager `+0x81` = pressed):
//!   the press does not read `+0x2A / +0x2C`.
//! - The text: message `text id` of ActionButtonText, msgbnd file 32 (base), 365 (dlc01), 465 (dlc02) (READ in the
//!   game's menu msgbnd copies under build/sora_tmp/er/conv/game/msg/engus).
//!
//! WHAT WE DO:
//! - `read` (once a frame, sora.rs `watch_ui`): the shown text id and row id, the text from the game's own messages.
//! - `kh_name`: the text as a short KH3 reaction command: KH3's own command kinds where KH3 has the word (Talk 77, Open 78,
//!   Examine 79, Ride 84), else a KH3-style short name ("Pick Up", "Climb", "Rest" ...). Pure, tested over every line of
//!   ActionButtonText.
//! - `reactions`: THE LIST OF Y INTERACTIONS IN Y'S OWN ORDER (sora.rs `YOwner::at_down` with CV3's `er_prompt_may_take_y`):
//!   the save point's Save, then a party member's talk (CV3's `conv::offer_talk` owners: Donald's recruit, every later
//!   one), then Elden Ring's prompt. The first is what Y does: KH3's green event button; the others stack under it as KH3's
//!   situation rows (kh_host `Hud::set_host_reactions`).
//! - ELDEN RING'S OWN BOX: `prompt.er_hide` (default 0, UNTESTED IN GAME): 1 = after reading, the shown flag `+0x2A` and the
//!   text id `+0x2C` are cleared in our frame task, which the press does not read; whether the game's prompt box reads
//!   them, and whether our task runs between the manager's update and the box's draw, was NOT READ. So by default BOTH
//!   are visible (ours and the game's box), as the brief allows. With 1, the Y routing's "prompt is up" also takes the
//!   manager's own state read before the clear (so Y still reaches the game if the box goes).

use std::sync::Mutex;

/// What the manager showed at the last read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shown {
    /// ActionButtonParam row of the shown candidate (-1: none readable).
    pub row: i32,
    /// The text id shown (-1: none this frame).
    pub text_id: i32,
    /// The game's own text (its language), when found.
    pub text: Option<String>,
}

static SHOWN: Mutex<Option<Shown>> = Mutex::new(None);
/// Elden Ring's action prompt is up (sora.rs `watch_ui`: its interface element, or the manager with `prompt.er_hide`).
static ER_UP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// (sora.rs, once a frame) Elden Ring's action prompt is up / not.
pub fn set_er_up(up: bool) {
    ER_UP.store(up, std::sync::atomic::Ordering::Relaxed);
}

/// Elden Ring's action prompt is up (as Y's routing reads it).
pub fn er_up() -> bool {
    ER_UP.load(std::sync::atomic::Ordering::Relaxed)
}

/// The last read (None: nothing shown, or not readable).
pub fn shown() -> Option<Shown> {
    SHOWN.lock().ok().and_then(|g| g.clone())
}

/// `CSActionButtonManImp` offsets (2.7.1.0, READ: 0xA64E16 .. 0xA64EB3; the shown node at +0x20 is not read).
const SHOWN_FLAG: usize = 0x2A;
const TEXT_ID: usize = 0x2C;
/// ActionButtonText msgbnd files: base, dlc01, dlc02.
const TEXT_FILES: [u32; 3] = [32, 365, 465];

/// (game thread) READS what the game's action button manager shows now; with `hide` (key `prompt.er_hide`) clears its
/// shown flag and text id afterwards. Returns whether a prompt is shown by the manager (before any clear).
#[cfg(windows)]
pub fn read(hide: bool) -> bool {
    use fromsoftware_shared::FromStatic;
    let got = std::panic::catch_unwind(|| {
        // SAFETY: game thread; the singleton lives as long as the game; the fields read are plain words of the manager
        // (READ offsets for 2.7.1.0, the version this DLL is gated to); the node pointer is checked for null and alignment.
        let Ok(man) = (unsafe { eldenring::cs::CSActionButtonMan::instance_mut() }) else { return None };
        let base = man as *mut eldenring::cs::CSActionButtonMan as *mut u8;
        unsafe {
            let text_id = (base.add(TEXT_ID) as *const i32).read_unaligned();
            // (the shown candidate node +0x20 / its row +0x08 are NOT dereferenced: a node may be freed between frames)
            let row = -1;
            if hide && text_id >= 0 {
                base.add(SHOWN_FLAG).write(0);
                (base.add(TEXT_ID) as *mut i32).write_unaligned(-1);
            }
            Some((text_id, row))
        }
    })
    .ok()
    .flatten();
    let shown = got.filter(|(id, _)| *id >= 0).map(|(text_id, row)| Shown { row, text_id, text: message(text_id) });
    let up = shown.is_some();
    if let Ok(mut g) = SHOWN.lock() {
        *g = shown;
    }
    up
}

/// The game's own ActionButtonText line.
#[cfg(windows)]
fn message(id: i32) -> Option<String> {
    if id < 0 {
        return None;
    }
    // SAFETY: game thread; the SDK checks the file table's bounds.
    let msgs = unsafe { <eldenring::cs::MsgRepository as fromsoftware_shared::FromStatic>::instance() }.ok()?;
    for file in TEXT_FILES {
        if let Some(units) = msgs.get_msg(file, id as u32) {
            let end = units.iter().position(|c| *c == 0).unwrap_or(units.len()).min(200);
            let t = String::from_utf16_lossy(&units[..end]).trim().to_string();
            if !t.is_empty() && !t.contains("[ERROR]") {
                return Some(t);
            }
        }
    }
    None
}

/// KH3's reaction command kinds (the pack's commands.json rows, `kh3_system.situation.*`).
pub const KIND_TALK: i64 = 77;
pub const KIND_OPEN: i64 = 78;
pub const KIND_EXAMINE: i64 = 79;
pub const KIND_RIDE: i64 = 84;

/// (pure) AN ELDEN RING PROMPT AS A KH3 REACTION COMMAND: (KH3 command kind whose own name is shown, if KH3 has the
/// word; the short name). Unknown text: its first word, capitalised (KH3's commands are one or two words).
pub fn kh_name(text: &str) -> (Option<i64>, String) {
    let t = text.trim();
    let l = t.to_lowercase();
    let starts = |w: &str| l == w || l.starts_with(&format!("{w} "));
    let named = |s: &str| (None, s.to_string());
    // KH3's own reaction commands
    if starts("talk") {
        return (Some(KIND_TALK), "Talk".into());
    }
    if starts("open") && !l.contains("only to") || l == "search treasure chest" {
        return (Some(KIND_OPEN), "Open".into());
    }
    if starts("ride") {
        return (Some(KIND_RIDE), "Ride".into());
    }
    if starts("examine") || l.contains("blocked shut") || l == "locked" || l.contains("does not open") || l.contains("not destined to open") || l.contains("opens only to") || l.contains("does not move") || l.starts_with("where did") {
        return (Some(KIND_EXAMINE), "Examine".into());
    }
    // KH3-style short names for Elden Ring's other actions
    let table: &[(&str, &str)] = &[
        ("pick up", "Pick Up"),
        ("put on", "Put On"),
        ("rest", "Rest"),
        ("touch grace", "Touch"),
        ("retrieve", "Retrieve"),
        ("touch", "Touch"),
        ("pull", "Pull"),
        ("push", "Push"),
        ("climb", "Climb"),
        ("descend", "Descend"),
        ("kick", "Kick"),
        ("traverse", "Enter"),
        ("enter", "Enter"),
        ("read", "Read"),
        ("check", "Check"),
        ("summon", "Summon"),
        ("be summoned", "Join"),
        ("invade", "Invade"),
        ("pillage", "Search"),
        ("acquire", "Gather"),
        ("light", "Light"),
        ("use", "Use"),
        ("sound", "Ring"),
        ("steal", "Steal"),
        ("lie down", "Lie Down"),
        ("kneel", "Kneel"),
        ("be held", "Embrace"),
        ("hoist", "Hoist"),
        ("offer", "Offer"),
        ("hand over", "Give"),
        ("return to", "Return"),
        ("return", "Give"),
        ("go back", "Return"),
        ("administer", "Give"),
        ("dye", "Dye"),
        ("receive", "Receive"),
        ("accept", "Accept"),
        ("transplant", "Use"),
        ("become", "Accept"),
        ("remove", "Remove"),
        ("restore", "Restore"),
    ];
    for (w, n) in table.iter().copied() {
        if starts(w) {
            return named(n);
        }
    }
    if l == "ok" {
        return named("OK");
    }
    let first: String = t.split_whitespace().next().unwrap_or("").chars().filter(|c| c.is_alphanumeric() || *c == '-').collect();
    let mut c = first.chars();
    match c.next() {
        Some(f) => named(&(f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase())),
        None => (Some(KIND_EXAMINE), "Examine".into()),
    }
}

/// What the frame offers on Y.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct YOffers {
    /// The save point's Save command is offered (savepoint.rs `PROMPT`).
    pub save: bool,
    /// The party talks offered now (CV3's `conv::offer_talk` owners, in order of offer).
    pub party_talks: usize,
    /// Elden Ring's own action prompt is up, with its text when read.
    pub er_prompt: Option<Option<String>>,
}

/// (pure) THE Y INTERACTIONS IN Y'S ORDER (sora.rs: the save point's command first, then a party talk, which wins Y over
/// Elden Ring's prompt (CV3), then Elden Ring's prompt): the first is the green event button.
pub fn reactions(o: &YOffers) -> Vec<kh_host::hud::HostReaction> {
    use kh_host::hud::{Hud, HostReaction};
    let mut out = Vec::new();
    if o.save {
        out.push(HostReaction::kind(Hud::SAVE_COMMAND));
    }
    for _ in 0..o.party_talks.min(4) {
        out.push(HostReaction::kind(KIND_TALK));
    }
    if let Some(text) = &o.er_prompt {
        let (kind, name) = text.as_deref().map(kh_name).unwrap_or((Some(KIND_EXAMINE), "Examine".to_string()));
        out.push(HostReaction { kind, name });
    }
    out
}

static LAST: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// The list as the HUD gets it is logged when it changes (`prompt:` lines; the game's own text beside its KH3 name).
pub fn log_change(list: &[kh_host::hud::HostReaction]) {
    let names: Vec<String> = list.iter().map(|r| r.name.clone()).collect();
    let Ok(mut last) = LAST.lock() else { return };
    if *last != names {
        let er = shown().map(|s| format!("; the game's prompt: row {} text {} {:?}", s.row, s.text_id, s.text.unwrap_or_default())).unwrap_or_default();
        crate::erlog!("prompt: (RC) Y interactions on KH3's reaction command: {}{er}", if names.is_empty() { "none".to_string() } else { names.join(" > ") });
        *last = names;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rc_every_elden_ring_prompt_becomes_a_short_kh3_command() {
        for (text, kind, name) in [
            ("Talk", Some(KIND_TALK), "Talk"),
            ("Open", Some(KIND_OPEN), "Open"),
            ("Open door", Some(KIND_OPEN), "Open"),
            ("Search treasure chest", Some(KIND_OPEN), "Open"),
            ("Examine", Some(KIND_EXAMINE), "Examine"),
            ("Examine altar", Some(KIND_EXAMINE), "Examine"),
            ("Locked", Some(KIND_EXAMINE), "Examine"),
            ("Opens only to those carrying a finger", Some(KIND_EXAMINE), "Examine"),
            ("Pick up item", None, "Pick Up"),
            ("Rest at site of grace", None, "Rest"),
            ("Touch grace", None, "Touch"),
            ("Pull lever", None, "Pull"),
            ("Climb", None, "Climb"),
            ("Traverse the mist", None, "Enter"),
            ("Read message", None, "Read"),
            ("Retrieve lost runes", None, "Retrieve"),
            ("Ride", Some(KIND_RIDE), "Ride"),
            ("Summon Blaidd the Half-Wolf", None, "Summon"),
            ("Be summoned to assist Millicent", None, "Join"),
            ("Use smithing table", None, "Use"),
            ("Something new", None, "Something"),
        ] {
            assert_eq!(kh_name(text), (kind, name.to_string()), "{text}");
        }
    }

    /// Every line of the game's ActionButtonText (the extracted copy, when present): a short name, one or two words.
    #[test]
    fn rc_every_line_of_action_button_text_has_a_short_name() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_tmp/er/conv/game/txt/all");
        let mut n = 0;
        for file in ["ActionButtonText.tsv", "ActionButtonText_dlc01.tsv"] {
            let Ok(text) = std::fs::read_to_string(root.join(file)) else { continue };
            for line in text.lines() {
                let Some(t) = line.split('\t').nth(1) else { continue };
                let (_, name) = kh_name(t);
                assert!(!name.is_empty() && name.len() <= 12 && name.split(' ').count() <= 2, "{t:?} -> {name:?}");
                n += 1;
            }
        }
        eprintln!("{n} prompt lines named");
    }

    /// (RC, the coordinator's addition: "including our custom Donald interaction") DONALD'S RECRUIT TALK AT THE FINGER READER
    /// (CV3's `offer_talk(DONALD_OWNER)`): KH3's green "Talk" is his, Y is his over the crone's own "Talk" prompt (which
    /// stacks under it), and it goes with his offer. Whoever leads: the HUD's one command model (harness rc_y_commands).
    #[cfg(all(windows, feature = "party"))]
    #[test]
    fn rc_donalds_recruit_talk_is_the_green_talk_bar_and_wins_y() {
        let owner = crate::unlock::DONALD_OWNER;
        crate::conv::offer_talk(owner, true);
        let offers = YOffers { save: false, party_talks: crate::conv::party_talks_offered(), er_prompt: Some(Some("Talk".into())) };
        let list = reactions(&offers);
        assert!(offers.party_talks >= 1);
        assert_eq!(list.first().map(|r| (r.kind, r.name.as_str())), Some((Some(KIND_TALK), "Talk")), "his Talk is the green bar");
        assert_eq!(list.last().map(|r| (r.kind, r.name.as_str())), Some((Some(KIND_TALK), "Talk")), "the crone's prompt stacks under it");
        assert!(!crate::conv::er_prompt_may_take_y(true), "Y is his, not the crone's");
        crate::conv::offer_talk(owner, false);
    }

    #[test]
    fn rc_the_y_interactions_stack_in_ys_order() {
        let names = |o: &YOffers| reactions(o).iter().map(|r| (r.kind, r.name.clone())).collect::<Vec<_>>();
        assert!(reactions(&YOffers::default()).is_empty());
        // the save point first, then a party talk (it wins Y over the game's prompt), then the game's prompt
        let all = YOffers { save: true, party_talks: 1, er_prompt: Some(Some("Talk".into())) };
        assert_eq!(names(&all), vec![(Some(76), "Save".into()), (Some(KIND_TALK), "Talk".into()), (Some(KIND_TALK), "Talk".into())]);
        let er = YOffers { er_prompt: Some(Some("Pick up item".into())), ..YOffers::default() };
        assert_eq!(names(&er), vec![(None, "Pick Up".into())]);
        // a prompt whose text was not readable: KH3's Examine
        let unread = YOffers { er_prompt: Some(None), ..YOffers::default() };
        assert_eq!(names(&unread), vec![(Some(KIND_EXAMINE), "Examine".into())]);
    }
}
