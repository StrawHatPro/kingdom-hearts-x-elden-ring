//! (THE USER 2026-11-02: "Move the Outfits selection to Boc, replacing his armor alteration menu with it completely
//! (this means it is only accessible by speaking with Boc)") BOC'S "ALTER GARMENTS" -> THE KH OUTFITS SCREEN: pure rules
//! of the talk hooks (hooks.rs feeds them on the game's thread; boc.rs `talk_frame` opens the screen and says when it
//! closed). The same pattern as the merchants' windows (shop.rs).
//!
//! READ in the game's own talk scripts (read-only copies in build/sora_tmp/er/conv/game/esd; every script scanned with
//! build/sora_tmp/er/conv/tools/esdparse.js for command 142, 2026-11-02):
//! - BOC'S FIVE SCRIPTS (t223006000 the open world, t223001100, t223001105, t223001400, t223003115), his talk list
//!   (group x18 / x8): entry 1 is "Alter garments" (EventTextForTalk 22230000). Chosen, state 6 runs
//!   `c1_142(110000, 110399)` (the game's equipment alteration window over those ShopLineupParam rows), and state 7 waits
//!   while `CheckSpecificPersonMenuIsOpen(26, 0)` (query 59, menu type 26) reads 1 with no generic dialog open, then goes
//!   back to state 1: his talk list again. Nothing else of his talk (his quest, his other options) touches 142 or type 26.
//! - THE ONLY OTHER SCRIPT WITH COMMAND 142 is the site of grace's (t000001000, talk id 1000: `c1_142(111000, 111399)`,
//!   its "Alter garments" once the player holds the needles Boc's quest ends with). A SYSTEM SCRIPT (talk ids below
//!   100000; the KH save point replaces the grace's menu, gracelock.rs): NOT TAKEN, so Boc stays the only way in.
//!
//! THE RULE (`boc.outfits`, default on): command 142 in a CHARACTER'S talk script (talk id 100000 and up) is NOT RUN
//! (Elden Ring's alteration window never opens), a request for the Outfits screen is left, and query 59 for menu type 26
//! (or "any", -1) reads 1 for that talk until the screen has closed; then the game's own answer (0, nothing of its own
//! is open) lets the script go back to Boc's talk list. It is the COMMAND that is recognised, not Boc's id or place:
//! wherever he stands (any of his scripts, a later one too) it works. `boc.outfits = 0`: his window is Elden Ring's.
//! CONFIDENCE: the numbers are read from the scripts (how they use them); NOT SEEN IN THE GAME.

use super::rules::{env, Decision, Val};

/// The talk command that opens Elden Ring's equipment alteration window (`c1_142(first row, last row)`).
pub const OPEN_ALTER: i32 = 142;
/// CheckSpecificPersonMenuIsOpen's menu type of that window (query 59's first argument after 142).
pub const MENU_ALTER: i32 = 26;
/// Talk ids below this are the system scripts (the grace's 1000 ...), left to the game (talkkh/mod.rs).
pub const FIRST_CHARACTER_TALK: i32 = 100_000;

/// A character's request for the Outfits screen (his alteration window, swallowed).
#[derive(Clone, Debug, PartialEq)]
pub struct AlterRequest {
    pub talk_id: i32,
    /// The talk object (its character).
    pub ins: usize,
    /// The rows the script named (for the log).
    pub rows: (i32, i32),
    pub serial: u32,
    /// boc.rs opened the Outfits screen for it.
    pub opened: bool,
}

#[derive(Clone, Debug, Default)]
pub struct AlterTalk {
    /// `boc.outfits` on (default) and outfits to offer.
    pub on: bool,
    /// The request waiting for / shown by the Outfits screen (at most one).
    pub request: Option<AlterRequest>,
    serial: u32,
    /// Log lines for the frame (`boc:` lines).
    pub notes: Vec<String>,
}

impl AlterTalk {
    /// A COMMAND of a talk script: `Some(Swallow)` = the game does not run it (the Outfits screen stands in).
    pub fn on_event(&mut self, talk_id: i32, ins: usize, id: i32, args: &[Val]) -> Option<Decision> {
        if id != OPEN_ALTER || !self.on || talk_id < FIRST_CHARACTER_TALK {
            return None;
        }
        let int = |k: usize| args.get(k).and_then(Val::int).unwrap_or(-1);
        self.serial = self.serial.wrapping_add(1);
        self.notes.push(format!("talk {talk_id}: Elden Ring's alteration window (command {id} {}..{}) -> the Kingdom Hearts Outfits screen", int(0), int(1)));
        self.request = Some(AlterRequest { talk_id, ins, rows: (int(0), int(1)), serial: self.serial, opened: false });
        Some(Decision::Swallow)
    }

    /// A QUERY AFTER THE GAME ANSWERED IT: `Some(1)` = the alteration window reads open while the Outfits screen
    /// stands in for it (that talk only, type 26 or "any").
    pub fn on_env(&self, talk_id: i32, id: i32, args: &[Val]) -> Option<i32> {
        if id != env::MENU_OPEN {
            return None;
        }
        self.request.as_ref().filter(|r| r.talk_id == talk_id)?;
        let wanted = args.first().and_then(Val::int).unwrap_or(-1);
        (wanted == -1 || wanted == MENU_ALTER).then_some(1)
    }

    /// The Outfits screen closed (or could not open): the script reads the game's own "closed" from now on.
    pub fn done(&mut self, why: &str) -> Option<AlterRequest> {
        let r = self.request.take()?;
        self.notes.push(format!("talk {}: the Outfits screen is done ({why}): Boc's talk goes on", r.talk_id));
        Some(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOC: i32 = 223_006_000;
    const GRACE: i32 = 1000;
    const KALE: i32 = 800_006_000;

    fn on() -> AlterTalk {
        AlterTalk { on: true, ..AlterTalk::default() }
    }

    /// (THE USER 2026-11-02) BOC'S "ALTER GARMENTS": his script's c1_142(110000, 110399) is not run (Elden Ring's window
    /// never opens) and leaves a request for the Outfits screen; his state 7 (query 59 type 26) reads the window open
    /// until the screen has closed, then the game's own answer: back to his talk list. Every one of his scripts, the
    /// same command elsewhere in a character's script too; never the grace's, never with `boc.outfits = 0`.
    #[test]
    fn boc_s_alteration_command_opens_the_outfits_screen_instead() {
        let mut t = on();
        assert_eq!(t.on_event(BOC, 7, OPEN_ALTER, &[Val::Int(110_000), Val::Int(110_399)]), Some(Decision::Swallow), "the game's window is not opened");
        let r = t.request.clone().expect("a request for the Outfits screen");
        assert_eq!((r.talk_id, r.ins, r.rows, r.opened), (BOC, 7, (110_000, 110_399), false));
        assert_eq!(t.on_env(BOC, env::MENU_OPEN, &[Val::Int(MENU_ALTER), Val::Int(0)]), Some(1), "state 7 waits while the screen is up");
        assert_eq!(t.on_env(BOC, env::MENU_OPEN, &[Val::Int(-1)]), Some(1), "type -1 = any");
        assert_eq!(t.on_env(BOC, env::MENU_OPEN, &[Val::Int(1), Val::Int(0)]), None, "his talk list's own query is the game's");
        assert_eq!(t.on_env(KALE, env::MENU_OPEN, &[Val::Int(MENU_ALTER)]), None, "another talk");
        assert_eq!(t.on_env(BOC, env::LIST_RESULT, &[]), None, "only query 59");
        // the screen closes: the game's answer (0) sends his script back to his talk list (state 1)
        assert!(t.done("closed").is_some());
        assert_eq!(t.on_env(BOC, env::MENU_OPEN, &[Val::Int(MENU_ALTER), Val::Int(0)]), None, "closed: the game's own answer");
        assert!(t.done("again").is_none(), "done once");
        // his other scripts (other places in the playthrough): the command, not his id, is what is recognised
        for script in [223_001_100, 223_001_105, 223_001_400, 223_003_115] {
            assert_eq!(on().on_event(script, 3, OPEN_ALTER, &[Val::Int(110_000), Val::Int(110_399)]), Some(Decision::Swallow), "{script}");
        }
        // the grace's own "Alter garments" (a system script) and the key off: Elden Ring's
        let mut g = on();
        assert_eq!(g.on_event(GRACE, 1, OPEN_ALTER, &[Val::Int(111_000), Val::Int(111_399)]), None);
        assert!(g.request.is_none());
        let mut off = AlterTalk::default();
        assert_eq!(off.on_event(BOC, 7, OPEN_ALTER, &[Val::Int(110_000), Val::Int(110_399)]), None, "boc.outfits = 0");
        // his other commands (the talk list, the shop's) are not this rule's
        assert_eq!(on().on_event(BOC, 7, super::super::rules::cmd::SHOW_SHOP_MESSAGE, &[Val::Int(1)]), None);
        assert_eq!(on().on_event(BOC, 7, 22, &[Val::Int(0), Val::Int(1)]), None);
    }
}
