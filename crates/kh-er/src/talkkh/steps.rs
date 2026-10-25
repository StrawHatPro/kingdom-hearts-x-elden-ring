//! (CV2) THE STEPS the adapter gives CV1's conversation runner (docs/party-contracts.md C.2 / C.4), pure: one `Line`
//! per subtitle line of the game (ONE SUBTITLE LINE = ONE BUBBLE), one external `Choice` per menu / dialog of the game.
//!
//! - A line: the NPC speaks to the leader; its voice is the game's (the key `er:talk:<TalkParam row>:<voice id>`, its
//!   length the line's own; the runner's `Line` event for it is NOT played by us: the game plays the voice), KH's
//!   typewriter, `auto_after` 0: the line goes on as soon as the game's voice is over (the adapter reports the game's
//!   voice every frame), A (after the text is all shown) = the runner's `StopVoice` = the game's own skip.
//! - A choice: asked by the leader (the player chooses: the tail at the leader) with the dialog's question as its
//!   prompt (none for menus), the options in the script's order, B = the cancel option ("Leave" / "No" / "OK").

use kh_host::conv::{Actor, ActorId, Branch, Choice, ChoiceOption, Line, Role, Step, Voice};

use super::rules::Ask;

/// The adapter's actor ids: the leader, the NPC whose talk it is.
pub const LEADER: ActorId = 1;
pub const NPC: ActorId = 2;

pub fn actors(leader_name: &str, npc_name: &str) -> [Actor; 2] {
    [Actor { id: LEADER, name: leader_name.to_string(), role: Role::Leader }, Actor { id: NPC, name: npc_name.to_string(), role: Role::Npc }]
}

/// The voice key of a game line (the runner hands it back in its `Line` event; we only log it).
pub fn voice_key(row: i32, voice: i32) -> String {
    format!("er:talk:{row}:{voice}")
}

/// ONE SUBTITLE LINE of the game = ONE `Line` step (`seconds`: the line's length as the game set it, the runner's
/// fallback when the voice is not reported).
pub fn line(text: &str, row: i32, voice: i32, seconds: f32) -> Step {
    Step::Line(Line {
        speaker: NPC,
        listener: Some(LEADER),
        text: text.to_string(),
        voice: Some(Voice { key: voice_key(row, voice), seconds: (seconds.is_finite() && seconds > 0.0).then_some(seconds) }),
        reveal_cps: None,
        auto_after: Some(0.0),
        ..Line::default()
    })
}

/// (CV2-b) ONE SUBTITLE LINE of an AMBIENT talk (proximity, taunt, battle): no listener, the game's voice, goes on as
/// soon as the game's voice is over and the text is all shown (the ambient runner never waits for A).
pub fn ambient_line(text: &str, row: i32, voice: i32, seconds: f32) -> Step {
    match line(text, row, voice, seconds) {
        Step::Line(mut l) => {
            l.listener = None;
            Step::Line(l)
        }
        other => other,
    }
}

/// A CHOICE of the game as an external `Choice` step (the pick comes back as `ConvEvent::Chosen { option }`, the
/// option in this order: [`super::rules::Talks::answer`] takes it).
pub fn choice(ask: &Ask) -> Step {
    Step::Choice(Choice {
        speaker: Some(LEADER),
        prompt: ask.prompt.clone(),
        options: ask.options.iter().map(|o| ChoiceOption { text: o.text.clone(), branch: Branch::Next }).collect(),
        start: 0,
        cancel: ask.cancel,
        external: true,
    })
}

#[cfg(test)]
mod tests {
    use super::super::rules::{AskKind, Opt};
    use super::*;

    #[test]
    fn a_game_line_is_one_line_step_with_the_games_voice() {
        let Step::Line(l) = line("Oh, yes... Tarnished, are we?", 30_101_100, 301_011_000, 3.2) else { panic!() };
        assert_eq!((l.speaker, l.listener, l.text.as_str()), (NPC, Some(LEADER), "Oh, yes... Tarnished, are we?"));
        assert_eq!(l.voice, Some(Voice { key: "er:talk:30101100:301011000".into(), seconds: Some(3.2) }));
        assert_eq!(l.auto_after, Some(0.0), "goes on when the game's voice is over");
        let Step::Line(l) = line("x", 1, 2, -1.0) else { panic!() };
        assert_eq!(l.voice.unwrap().seconds, None, "the line's length unknown: the reported voice decides");
    }

    #[test]
    fn a_game_choice_is_an_external_choice() {
        let ask = Ask { talk_id: 301_006_000, ins: 1, event_obj: 2, kind: AskKind::Choices, prompt: String::new(), options: vec![Opt { index: 1, msg: 23_011_000, text: "I'll fly straight and true!".into() }, Opt { index: 2, msg: 23_011_001, text: "I don't need a seat".into() }], cancel: None, cmd: 76, args: vec![0], serial: 4 };
        let Step::Choice(c) = choice(&ask) else { panic!() };
        assert!(c.external && c.cancel.is_none() && c.speaker == Some(LEADER));
        assert_eq!(c.options.iter().map(|o| o.text.as_str()).collect::<Vec<_>>(), ["I'll fly straight and true!", "I don't need a seat"]);
    }
}
