//! (THE USER 2026-11-02, DECIDED: "Roderika's spirit tuning opens a KH3-STYLE SCREEN, not Elden Ring's menu ... the way Boc's
//! 'Alter garments' was") RODERIKA'S "SPIRIT TUNING" -> THE KH SPIRIT TUNING SCREEN: pure rules of the talk hooks (hooks.rs
//! feeds them on the game's thread; spirittune.rs `talk_frame` opens the screen and says when it closed). The pattern of
//! Boc's alteration (alter.rs).
//!
//! READ in the game's own talk scripts (read-only copies in build/sora_tmp/er/conv/game/esd, scanned 2026-11-02):
//! - RODERIKA'S TWO SCRIPTS (t320001110 at the Roundtable Hold, t320006000): her talk list (groups x8 / x9 / x10) has entry 1
//!   "Spirit Tuning" (EventTextForTalk 23200000). Chosen, a state runs `c1_136()` (the game's spirit-tuning window) and
//!   `c1_141(23)`, then waits while `CheckSpecificPersonMenuIsOpen(23, 0)` (query 59, menu type 23) reads 1 with no generic
//!   dialog open (query 58), then goes on (her talk list again). Command 136 is in NO OTHER SCRIPT of the 365.
//!
//! THE RULE (`roderika.kh`, default on): command 136 in a CHARACTER'S talk script (talk id 100000 and up) is NOT RUN (Elden
//! Ring's window never opens), a request for the Spirit Tuning screen is left, and query 59 for menu type 23 (or "any", -1)
//! reads 1 for that talk until the screen has closed; then the game's own answer (0) lets the script go back to her talk
//! list. It is the COMMAND that is recognised, not Roderika's id or place. `roderika.kh = 0`: her window is Elden Ring's.
//! CONFIDENCE: the numbers are read from the scripts (how they use them); NOT SEEN IN THE GAME.

use super::alter::FIRST_CHARACTER_TALK;
use super::rules::{env, Decision, Val};

/// The talk command that opens Elden Ring's spirit-tuning window (`c1_136()`).
pub const OPEN_TUNING: i32 = 136;
/// CheckSpecificPersonMenuIsOpen's menu type of that window (query 59's first argument after 136).
pub const MENU_TUNING: i32 = 23;

/// A character's request for the Spirit Tuning screen (her window, swallowed).
#[derive(Clone, Debug, PartialEq)]
pub struct TuneRequest {
    pub talk_id: i32,
    /// The talk object (its character: the tuner's name).
    pub ins: usize,
    pub serial: u32,
    /// spirittune.rs opened the screen for it.
    pub opened: bool,
}

#[derive(Clone, Debug, Default)]
pub struct TuningTalk {
    /// `roderika.kh` on (default).
    pub on: bool,
    /// The request waiting for / shown by the screen (at most one).
    pub request: Option<TuneRequest>,
    serial: u32,
    /// Log lines for the frame (`tuning:` lines).
    pub notes: Vec<String>,
}

impl TuningTalk {
    /// A COMMAND of a talk script: `Some(Swallow)` = the game does not run it (the Spirit Tuning screen stands in).
    pub fn on_event(&mut self, talk_id: i32, ins: usize, id: i32) -> Option<Decision> {
        if id != OPEN_TUNING || !self.on || talk_id < FIRST_CHARACTER_TALK {
            return None;
        }
        self.serial = self.serial.wrapping_add(1);
        self.notes.push(format!("talk {talk_id}: Elden Ring's spirit-tuning window (command {id}) -> the KH Spirit Tuning screen"));
        self.request = Some(TuneRequest { talk_id, ins, serial: self.serial, opened: false });
        Some(Decision::Swallow)
    }

    /// A QUERY AFTER THE GAME ANSWERED IT: `Some(1)` = the window reads open while the screen stands in for it (that talk
    /// only, type 23 or "any").
    pub fn on_env(&self, talk_id: i32, id: i32, args: &[Val]) -> Option<i32> {
        if id != env::MENU_OPEN {
            return None;
        }
        self.request.as_ref().filter(|r| r.talk_id == talk_id)?;
        let wanted = args.first().and_then(Val::int).unwrap_or(-1);
        (wanted == -1 || wanted == MENU_TUNING).then_some(1)
    }

    /// (testing aid `test.tuning`, spirittune.rs) A request with no talk behind it (talk id -1: no script reads it), so the
    /// screen can be opened in the game without reaching Roderika. Not made while a request stands.
    pub fn test_request(&mut self) -> bool {
        if self.request.is_some() {
            return false;
        }
        self.serial = self.serial.wrapping_add(1);
        self.notes.push("test.tuning: a request without a talk (testing aid)".to_string());
        self.request = Some(TuneRequest { talk_id: -1, ins: 0, serial: self.serial, opened: false });
        true
    }

    /// The screen closed (or could not open): the script reads the game's own "closed" from now on.
    pub fn done(&mut self, why: &str) -> Option<TuneRequest> {
        let r = self.request.take()?;
        self.notes.push(format!("talk {}: the Spirit Tuning screen is done ({why}): her talk goes on", r.talk_id));
        Some(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RODERIKA: i32 = 320_001_110;
    const RODERIKA_SHACK: i32 = 320_006_000;
    const BOC: i32 = 223_006_000;

    fn on() -> TuningTalk {
        TuningTalk { on: true, ..TuningTalk::default() }
    }

    /// (THE USER 2026-11-02) RODERIKA'S "SPIRIT TUNING": her script's c1_136() is not run and leaves a request; her wait
    /// (query 59 type 23) reads the window open until the screen has closed, then the game's own answer: back to her talk
    /// list. Both her scripts; never a system script; never with `roderika.kh = 0`; other commands are not this rule's.
    #[test]
    fn roderika_s_tuning_command_opens_the_kh_screen_instead() {
        let mut t = on();
        assert_eq!(t.on_event(RODERIKA, 7, OPEN_TUNING), Some(Decision::Swallow), "the game's window is not opened");
        let r = t.request.clone().expect("a request for the screen");
        assert_eq!((r.talk_id, r.ins, r.opened), (RODERIKA, 7, false));
        assert_eq!(t.on_env(RODERIKA, env::MENU_OPEN, &[Val::Int(MENU_TUNING), Val::Int(0)]), Some(1), "her wait while the screen is up");
        assert_eq!(t.on_env(RODERIKA, env::MENU_OPEN, &[Val::Int(-1)]), Some(1), "type -1 = any");
        assert_eq!(t.on_env(RODERIKA, env::MENU_OPEN, &[Val::Int(1), Val::Int(0)]), None, "her talk list's own query is the game's");
        assert_eq!(t.on_env(BOC, env::MENU_OPEN, &[Val::Int(MENU_TUNING)]), None, "another talk");
        assert_eq!(t.on_env(RODERIKA, env::LIST_RESULT, &[]), None, "only query 59");
        assert!(t.done("closed").is_some());
        assert_eq!(t.on_env(RODERIKA, env::MENU_OPEN, &[Val::Int(MENU_TUNING), Val::Int(0)]), None, "closed: the game's own answer");
        assert!(t.done("again").is_none(), "done once");
        assert_eq!(on().on_event(RODERIKA_SHACK, 3, OPEN_TUNING), Some(Decision::Swallow), "her other script");
        assert_eq!(on().on_event(1000, 3, OPEN_TUNING), None, "a system script");
        assert_eq!(TuningTalk::default().on_event(RODERIKA, 3, OPEN_TUNING), None, "roderika.kh = 0");
        assert_eq!(on().on_event(RODERIKA, 3, 141), None, "her c1_141(23) is the game's");
        assert_eq!(on().on_event(RODERIKA, 3, super::super::alter::OPEN_ALTER), None);
        // the testing aid: a request no script reads (talk id -1), never over a standing one
        let mut aid = on();
        assert!(aid.test_request() && !aid.test_request());
        assert!(aid.done("closed").is_some_and(|r| r.talk_id == -1));
    }
}
