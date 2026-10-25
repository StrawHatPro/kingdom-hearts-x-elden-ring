//! (CV2) THE ELDEN RING TALK ADAPTER, PURE RULES: what a talk-script command or query means, which talks are an Elden
//! Ring character's, the choice that replaces Elden Ring's menu and the answer the script reads back, and the
//! conversation that turns the game's subtitle lines into KH speech bubbles (ONE SUBTITLE LINE = ONE BUBBLE, the user's
//! rule of 2026-10-16). Nothing here touches the game; hooks.rs / game.rs feed it and do what it says.
//!
//! READ IN THE GAME'S FILES (eldenring.exe 2.7.1.0, a copy; build/sora_tmp/er/IMPL_CV2_STATE.md section 1 has the
//! addresses): the talk-script command and query numbers below are the cases of the game's own command function
//! (`CSEzStateTalkEvent` slot 1, RVA 0xea7100) and query function (`CSEzStateTalkEnv` slot 1, RVA 0xea24a0), checked
//! against how 359 character scripts use them (conv/tools/talkwalk.js) and against the SDK's own example (invoke-esd:
//! 10 / 19 / 20 / 23 / 58 / 59).

/// Talk-script commands (bank 1).
pub mod cmd {
    /// TalkToPlayer(talk param id, -1, -1, flag): plays the block of TalkParam rows id, id + 1, ... one line each.
    #[allow(dead_code)] // (the line itself is read from the talk object: hooks.rs `update_hook`)
    pub const TALK: i32 = 1;
    /// ShowShopMessage(1): the NPC's talk list (menu type 1). Other arguments are left to the game.
    pub const SHOW_SHOP_MESSAGE: i32 = 10;
    /// OpenGenericDialog(type, text, buttons, ?, ?): buttons 3 = Yes / No (the answer: query 22), 1 = OK.
    pub const GENERIC_DIALOG: i32 = 17;
    /// ForceCloseGenericDialog() (the scripts call it when 58 says a dialog is open and the talk is interrupted).
    pub const CLOSE_DIALOG: i32 = 18;
    /// AddTalkListData(index, EventTextForTalk id, event flag or -1): the option is added only when the flag is -1 or
    /// on (RVA 0xea7100 case 0x13: the flag is tested through the event-flag manager first).
    pub const ADD_LIST: i32 = 19;
    /// ClearTalkListData().
    pub const CLEAR_LIST: i32 = 20;
    /// ForceCloseMenu() (called when 59 says a menu is open and the talk is interrupted).
    pub const CLOSE_MENU: i32 = 67;
    /// OpenConversationChoicesMenu(0): the conversation's own choices (menu type 12).
    pub const CHOICES: i32 = 76;
    /// ChangeCamera(1000000 | -1): the talk camera, on while the player is in the conversation (317 scripts).
    pub const CAMERA: i32 = 138;
    /// AddTalkListData with more arguments (index, text, flag, ?, ?): 32 uses; the same option rule.
    pub const ADD_LIST_ALT: i32 = 149;
}

/// Talk-script queries.
pub mod env {
    /// The generic dialog's button: 1 = Yes / OK, 2 = No.
    pub const DIALOG_RESULT: i32 = 22;
    /// GetTalkListEntryResult(): the index of the option taken (the popup menu's + 0x178).
    pub const LIST_RESULT: i32 = 23;
    /// CheckSpecificPersonGenericDialogIsOpen(0).
    pub const DIALOG_OPEN: i32 = 58;
    /// CheckSpecificPersonMenuIsOpen(menu type, 0); type -1 = any.
    pub const MENU_OPEN: i32 = 59;
}

/// `MenuType` (SDK): the talk list, the conversation choices.
pub const MENU_TALK_LIST: i32 = 1;
pub const MENU_CHOICES: i32 = 12;
/// EventTextForTalk 20000009 = "Leave": B takes it in a talk list.
pub const MSG_LEAVE: i32 = 20_000_009;
/// The generic dialog's buttons.
pub const BUTTONS_YES_NO: i32 = 3;
pub const BUTTONS_OK: i32 = 1;
pub const ANSWER_YES: i32 = 1;
pub const ANSWER_NO: i32 = 2;

/// An Elden Ring CHARACTER's talk: every script but the grace's and the other system scripts of m00 (talk ids below
/// 100000: 1000 the grace menu, which savepoint.rs drives; 1100, 2000, 2006, 2012, 2100), with Melina's 3000 a
/// character's. `skip`: the key `talk.kh.skip` (more talk ids left to Elden Ring).
pub fn is_character_talk(talk_id: i32, skip: &[i32]) -> bool {
    (talk_id >= 100_000 || talk_id == 3000) && !skip.contains(&talk_id)
}

/// A talk-script value (EzState: 1 = float, 2 = int, 3 = string).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Val {
    Float(f32),
    Int(i32),
    Other,
}

impl Val {
    pub fn int(&self) -> Option<i32> {
        match *self {
            Val::Int(i) => Some(i),
            Val::Float(f) if f.is_finite() && f.fract() == 0.0 => Some(f as i32),
            _ => None,
        }
    }
}

/// THE TEXT A BUBBLE GETS from a message: the game's line breaks become spaces (one subtitle line = one bubble, which
/// wraps by itself), its sound captions `<groan>` become `(groan)` (angle brackets are the KH text markup), its value
/// placeholders `<?name?>` (filled in by the game's own menus) become `...`, runs of spaces one.
pub fn bubble_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        match tail.find('>') {
            Some(end) => {
                let inner = &tail[1..end];
                if inner.starts_with('?') {
                    out.push_str("...");
                } else {
                    out.push('(');
                    out.push_str(inner.trim());
                    out.push(')');
                }
                rest = &tail[end + 1..];
            }
            None => {
                out.push('(');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    let mut clean = String::with_capacity(out.len());
    let mut space = false;
    for c in out.chars() {
        let c = if c == '\n' || c == '\r' || c == '\t' { ' ' } else { c };
        if c == ' ' {
            if !space && !clean.is_empty() {
                clean.push(' ');
            }
            space = true;
        } else {
            clean.push(c);
            space = false;
        }
    }
    clean.trim_end().to_string()
}

/// One option of a choice: the index the script added it with (what it reads back), its text.
#[derive(Clone, Debug, PartialEq)]
pub struct Opt {
    pub index: i32,
    pub msg: i32,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AskKind {
    /// The NPC's talk list (menu type 1): Talk, Purchase, Leave ...
    TalkList,
    /// The conversation's choices (menu type 12): "Accept" / "Refuse" ...
    Choices,
    /// A generic dialog with Yes / No.
    YesNo,
    /// A generic dialog with OK.
    Notice,
}

impl AskKind {
    fn menu_type(self) -> Option<i32> {
        match self {
            AskKind::TalkList => Some(MENU_TALK_LIST),
            AskKind::Choices => Some(MENU_CHOICES),
            _ => None,
        }
    }
    fn dialog(self) -> bool {
        matches!(self, AskKind::YesNo | AskKind::Notice)
    }
}

/// A CHOICE in the bubble in the place of Elden Ring's menu or dialog, until it is answered.
#[derive(Clone, Debug, PartialEq)]
pub struct Ask {
    pub talk_id: i32,
    /// The talk object (`CSNpcTalkIns`) and the command object of the script that asked (to give the command back to
    /// the game, [`Talks::give_back`]).
    pub ins: usize,
    pub event_obj: usize,
    pub kind: AskKind,
    /// The dialog's text (Yes / No, OK); empty for menus.
    pub prompt: String,
    pub options: Vec<Opt>,
    /// The option B takes (talk list: "Leave"; Yes / No: No; OK: OK).
    pub cancel: Option<usize>,
    /// The command as the script gave it (id, integer arguments).
    pub cmd: i32,
    pub args: Vec<i32>,
    pub serial: u32,
}

/// What a command of a character's talk script does to the adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// The game runs it as it is.
    Pass,
    /// The game does not run it: the bubble asks instead (a menu or dialog of Elden Ring's that never opens).
    Swallow,
}

/// What the hooks need from the game while they decide (texts and flags; the hooks run on the game's thread).
pub trait Texts {
    /// The event flag's state (`None`: not readable).
    fn flag(&self, id: u32) -> Option<bool>;
    /// An option's text (EventTextForTalk, the expansion's file second).
    fn option_text(&self, msg: i32) -> Option<String>;
    /// A generic dialog's text (EventTextForTalk, GR_Dialogues, GR_MenuText).
    fn dialog_text(&self, msg: i32) -> Option<String>;
}

/// THE SHARED STATE of the talk scripts' commands and queries (one per game; the hooks write it, the frame reads it).
#[derive(Clone, Debug, Default)]
pub struct Talks {
    /// `talk.kh` (default on): off = every command and query passes untouched.
    pub enabled: bool,
    /// The bubble can be shown right now (the HUD has it, no cutscene, the world loaded): only then is a menu of
    /// Elden Ring's taken over; otherwise Elden Ring's own menu opens.
    pub ready: bool,
    /// `talk.kh.skip`: more talk ids left to Elden Ring.
    pub skip_ids: Vec<i32>,
    /// The options added since the last ClearTalkListData, per talk id.
    building: Vec<(i32, Vec<Opt>)>,
    /// Talk ids whose talk camera is on (ChangeCamera >= 0): the player is in that conversation.
    camera: Vec<i32>,
    /// The choice in the bubble (at most one).
    pub ask: Option<Ask>,
    /// The latest answer per talk id (the script may read it more than once): (talk, kind, value).
    answered: Vec<(i32, AskKind, i32)>,
    serial: u32,
    /// Log lines for the frame (`talk:` lines).
    pub notes: Vec<String>,
}

impl Talks {
    pub fn new() -> Talks {
        Talks { enabled: true, ..Talks::default() }
    }

    fn ours(&self, talk_id: i32) -> bool {
        self.enabled && is_character_talk(talk_id, &self.skip_ids)
    }

    /// The talk camera is on for this talk (the player is in the conversation).
    #[cfg(test)]
    pub fn camera_on(&self, talk_id: i32) -> bool {
        self.camera.contains(&talk_id)
    }

    pub fn camera_talks(&self) -> &[i32] {
        &self.camera
    }

    fn list(&mut self, talk_id: i32) -> &mut Vec<Opt> {
        if let Some(k) = self.building.iter().position(|(t, _)| *t == talk_id) {
            return &mut self.building[k].1;
        }
        self.building.push((talk_id, Vec::new()));
        &mut self.building.last_mut().expect("just pushed").1
    }

    /// A COMMAND of a talk script (`args` without the id). `ins` / `event_obj`: the talk and command objects.
    pub fn on_event(&mut self, talk_id: i32, ins: usize, event_obj: usize, id: i32, args: &[Val], texts: &dyn Texts) -> Decision {
        if !self.ours(talk_id) {
            return Decision::Pass;
        }
        let int = |k: usize| args.get(k).and_then(Val::int);
        match id {
            cmd::CAMERA => {
                let on = int(0).is_some_and(|v| v >= 0);
                let had = self.camera.contains(&talk_id);
                if on && !had {
                    self.camera.push(talk_id);
                } else if !on && had {
                    self.camera.retain(|t| *t != talk_id);
                }
                Decision::Pass
            }
            cmd::CLEAR_LIST => {
                self.list(talk_id).clear();
                Decision::Pass
            }
            cmd::ADD_LIST | cmd::ADD_LIST_ALT => {
                let (Some(index), Some(msg)) = (int(0), int(1)) else { return Decision::Pass };
                let flag = int(2).unwrap_or(-1);
                // the game's own rule: no flag, or the flag on (a flag not readable: shown, the game's list decides)
                if flag != -1 && texts.flag(flag as u32) == Some(false) {
                    return Decision::Pass;
                }
                let text = texts.option_text(msg).map(|t| bubble_text(&t)).unwrap_or_default();
                let list = self.list(talk_id);
                list.retain(|o| o.index != index);
                list.push(Opt { index, msg, text });
                Decision::Pass
            }
            cmd::SHOW_SHOP_MESSAGE | cmd::CHOICES => {
                if id == cmd::SHOW_SHOP_MESSAGE && int(0) != Some(1) {
                    return Decision::Pass;
                }
                let kind = if id == cmd::CHOICES { AskKind::Choices } else { AskKind::TalkList };
                let options = self.list(talk_id).clone();
                if !self.ready || options.is_empty() || options.iter().any(|o| o.text.is_empty()) {
                    // (CV3 round 4) ELDEN RING'S OWN MENU ANSWERS THIS ONE: the script must read ITS answer, never an
                    // older bubble answer of the same talk (Kale 2026-10-17: "Purchase" answered in the bubble -> the
                    // shop; back from the shop the list was left to Elden Ring's menu, B there was read as the old
                    // "Purchase" again -> the shop again, a loop the player could not leave)
                    self.forget(talk_id, false);
                }
                if !self.ready || options.is_empty() {
                    self.notes.push(format!("talk {talk_id}: {kind:?} of {} option(s) left to Elden Ring ({})", options.len(), if options.is_empty() { "no option was seen" } else { "the bubble is not ready" }));
                    return Decision::Pass;
                }
                if options.iter().any(|o| o.text.is_empty()) {
                    self.notes.push(format!("talk {talk_id}: {kind:?} left to Elden Ring: an option has no text ({:?})", options.iter().filter(|o| o.text.is_empty()).map(|o| o.msg).collect::<Vec<_>>()));
                    return Decision::Pass;
                }
                let cancel = if kind == AskKind::TalkList { options.iter().position(|o| o.msg == MSG_LEAVE) } else { None };
                self.open(Ask { talk_id, ins, event_obj, kind, prompt: String::new(), options, cancel, cmd: id, args: args.iter().map(|a| a.int().unwrap_or(0)).collect(), serial: 0 });
                Decision::Swallow
            }
            cmd::GENERIC_DIALOG => {
                // (CV3 round 4) a dialog of this talk opens: an older bubble answer of a dialog is never its answer
                // (the bubble's own answer to THIS dialog is stored again when the player gives it)
                self.forget(talk_id, true);
                let (Some(msg), Some(buttons)) = (int(1), int(2)) else { return Decision::Pass };
                let kind = match buttons {
                    BUTTONS_YES_NO => AskKind::YesNo,
                    BUTTONS_OK => AskKind::Notice,
                    _ => return Decision::Pass,
                };
                let Some(prompt) = texts.dialog_text(msg).map(|t| bubble_text(&t)).filter(|t| !t.is_empty()) else {
                    self.notes.push(format!("talk {talk_id}: dialog {msg} left to Elden Ring (no text for it)"));
                    return Decision::Pass;
                };
                if !self.ready {
                    self.notes.push(format!("talk {talk_id}: dialog {msg} left to Elden Ring (the bubble is not ready)"));
                    return Decision::Pass;
                }
                let yes = texts.dialog_text(YES_TEXT).map(|t| bubble_text(&t)).filter(|t| !t.is_empty()).unwrap_or_else(|| "Yes".into());
                let no = texts.dialog_text(NO_TEXT).map(|t| bubble_text(&t)).filter(|t| !t.is_empty()).unwrap_or_else(|| "No".into());
                let ok = texts.dialog_text(OK_TEXT).map(|t| bubble_text(&t)).filter(|t| !t.is_empty()).unwrap_or_else(|| "OK".into());
                let (options, cancel) = if kind == AskKind::YesNo {
                    (vec![Opt { index: ANSWER_YES, msg: YES_TEXT, text: yes }, Opt { index: ANSWER_NO, msg: NO_TEXT, text: no }], Some(1))
                } else {
                    (vec![Opt { index: ANSWER_YES, msg: OK_TEXT, text: ok }], Some(0))
                };
                self.open(Ask { talk_id, ins, event_obj, kind, prompt, options, cancel, cmd: id, args: args.iter().map(|a| a.int().unwrap_or(0)).collect(), serial: 0 });
                Decision::Swallow
            }
            cmd::CLOSE_MENU | cmd::CLOSE_DIALOG => {
                let dialog = id == cmd::CLOSE_DIALOG;
                if self.ask.as_ref().is_some_and(|a| a.talk_id == talk_id && a.kind.dialog() == dialog) {
                    let a = self.ask.take().expect("checked");
                    self.notes.push(format!("talk {talk_id}: the script closed its {:?} (the talk was interrupted): the bubble's choice is withdrawn", a.kind));
                }
                Decision::Pass
            }
            _ => Decision::Pass,
        }
    }

    /// (CV3 round 4) Drop the bubble's stored answer of `talk_id`'s menus (`dialog` false) or dialogs (true): from now on
    /// the script reads the game's own answer.
    fn forget(&mut self, talk_id: i32, dialog: bool) {
        self.answered.retain(|(t, k, _)| !(*t == talk_id && k.dialog() == dialog));
    }

    fn open(&mut self, mut ask: Ask) {
        self.serial = self.serial.wrapping_add(1);
        ask.serial = self.serial;
        self.forget(ask.talk_id, ask.kind.dialog());
        self.notes.push(format!("talk {}: {:?} in the bubble in Elden Ring's place: {}", ask.talk_id, ask.kind, ask.options.iter().map(|o| format!("{} \"{}\"", o.index, o.text)).collect::<Vec<_>>().join(", ")));
        self.ask = Some(ask);
    }

    /// A QUERY of a talk script (`args` without the id), after the game answered it: `Some(v)` = the script reads `v`
    /// (an integer) instead.
    pub fn on_env(&mut self, talk_id: i32, id: i32, args: &[Val]) -> Option<i32> {
        if !self.ours(talk_id) {
            return None;
        }
        let pending = self.ask.as_ref().filter(|a| a.talk_id == talk_id);
        match id {
            env::MENU_OPEN => {
                let wanted = args.first().and_then(Val::int).unwrap_or(-1);
                let open = pending.and_then(|a| a.kind.menu_type()).is_some_and(|t| wanted == -1 || wanted == t);
                open.then_some(1)
            }
            env::DIALOG_OPEN => pending.is_some_and(|a| a.kind.dialog()).then_some(1),
            env::LIST_RESULT => self.answered.iter().rev().find(|(t, k, _)| *t == talk_id && !k.dialog()).map(|(_, _, v)| *v),
            env::DIALOG_RESULT => self.answered.iter().rev().find(|(t, k, _)| *t == talk_id && k.dialog()).map(|(_, _, v)| *v),
            _ => None,
        }
    }

    /// THE PLAYER ANSWERED the choice `serial` with its option `k` (the bubble's order): the script reads the option's
    /// index (the menus) or the button (the dialogs) from now on, and the menu or dialog reads as closed.
    pub fn answer(&mut self, serial: u32, k: usize) -> Option<i32> {
        let a = self.ask.as_ref().filter(|a| a.serial == serial)?;
        let value = a.options.get(k)?.index;
        let a = self.ask.take().expect("checked");
        self.notes.push(format!("talk {}: {:?} answered: option {} \"{}\" -> the script reads {value}", a.talk_id, a.kind, k, a.options[k].text));
        self.answered.retain(|(t, kk, _)| !(*t == a.talk_id && kk.dialog() == a.kind.dialog()));
        self.answered.push((a.talk_id, a.kind, value));
        Some(value)
    }

    /// THE BUBBLE CANNOT SHOW the pending choice any more (a cutscene, the HUD gone): it is taken back, to be given to
    /// Elden Ring's own menu (the hooks run the command for the game).
    pub fn give_back(&mut self) -> Option<Ask> {
        let a = self.ask.take()?;
        self.forget(a.talk_id, a.kind.dialog()); // (CV3 round 4) Elden Ring's menu answers it now
        self.notes.push(format!("talk {}: {:?} given back to Elden Ring's own menu (the bubble can no longer show it)", a.talk_id, a.kind));
        Some(a)
    }

    pub fn clear(&mut self) {
        let (enabled, ready, skip) = (self.enabled, self.ready, std::mem::take(&mut self.skip_ids));
        *self = Talks { enabled, ready, skip_ids: skip, serial: self.serial, ..Talks::default() };
    }
}

/// The generic dialog's button texts: "OK" = GR_Dialogues 1 (READ in the menu msgbnd copy: the game's language);
/// "Yes" / "No" were NOT FOUND in any message file of the game (its dialog draws them from its menu layout): the bubble
/// says "Yes" / "No" (English in every language: an open item).
pub const YES_TEXT: i32 = -1;
pub const NO_TEXT: i32 = -2;
pub const OK_TEXT: i32 = 1;

// ------------------------------------------------------------------------------------------------ the conversation

/// ONE TALK OBJECT's line this frame, as the game's talk update left it (hooks.rs reads `CSTalkIns` after the update:
/// + 0x2d talking, + 4 the block's first TalkParam id, + 8 the line's index in the block, + 0x34 the subtitle's
/// TalkMsg id, + 0x2f the TalkToPlayer flag that the game's skip respects, + 0x54 / + 0x58 the line's elapsed time and
/// length).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineSnap {
    pub ins: usize,
    pub talk_id: i32,
    /// The TalkParam row playing = block + index: ONE BUBBLE PER ROW.
    pub row: i32,
    /// The TalkMsg id of the subtitle (-1: none this frame).
    pub msg: i32,
    /// The line is talking (+ 0x2d) and has a subtitle.
    pub active: bool,
    /// The game's own skip is refused for this line (+ 0x2f: TalkToPlayer's flag 1, the scripts' ambient lines).
    pub unskippable: bool,
    pub elapsed: f32,
    pub duration: f32,
    /// Frames since the game last updated this talk object (0 = this frame).
    pub age: u32,
}

impl LineSnap {
    fn shows(&self) -> bool {
        self.active && self.msg > 0 && self.age <= 2
    }
}

/// What the bubble shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bubble {
    /// A line: (talk object, its row) is the bubble's identity: a new row = a new bubble.
    Line { ins: usize, talk_id: i32, row: i32, msg: i32, skippable: bool },
    /// The pending choice.
    Ask { serial: u32, talk_id: i32 },
}

/// A frame's facts for [`Conversation::step`].
#[derive(Clone, Debug, Default)]
pub struct FrameIn {
    pub lines: Vec<LineSnap>,
    /// The pending choice (serial, talk id).
    pub ask: Option<(u32, i32)>,
    /// Talk ids with the talk camera on.
    pub camera_talks: Vec<i32>,
    /// Sora (the leader) is in a battle: an NPC's lines are only bubbles then (nothing is locked).
    pub in_battle: bool,
    /// The bubble can show (see [`Talks::ready`]).
    pub ready: bool,
    /// (CV3) The talk of a driven story rest (Melina at a grace): engaged while it speaks or asks (it turns no talk
    /// camera on there), lingering out as the others once it is quiet.
    pub story_talk: Option<i32>,
    pub dt: f32,
}

/// What the frame does.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameOut {
    /// The bubble now (None: none).
    pub bubble: Option<Bubble>,
    /// It is a new bubble (show its text from the start).
    pub new_bubble: bool,
    /// The player is in a conversation: its participants are locked, the conversation camera has the view.
    pub engaged: bool,
    /// The pad is the conversation's (Sora takes nothing, Elden Ring sees nothing of it).
    pub holds_pad: bool,
    /// The conversation ended this frame (the bubble closes, the lock and camera end).
    pub ended: bool,
    /// A log line.
    pub note: Option<String>,
}

/// THE CONVERSATION over the game's lines: one bubble per subtitle line (the row the game plays), the choice when one
/// is pending, the engaged state with a short linger between the blocks of one talk so that the lock and the camera do
/// not flicker.
#[derive(Clone, Debug, Default)]
pub struct Conversation {
    pub current: Option<Bubble>,
    engaged: bool,
    /// Seconds with nothing to show (the bubble closes after `LINGER`).
    idle: f32,
    /// Seconds since the engaged talk was last seen engaged.
    since_engaged: f32,
}

/// Seconds a bubble stays after its line ended with nothing new (the next line of a block follows within a frame;
/// a script's next block within a few frames).
pub const LINGER: f32 = 0.35;
/// Seconds the conversation stays engaged after its camera / choice was last seen (between a line and the menu).
pub const ENGAGED_LINGER: f32 = 0.6;

impl Conversation {
    pub fn step(&mut self, f: &FrameIn) -> FrameOut {
        let mut out = FrameOut::default();
        let was = self.current.is_some() || self.engaged;
        if !f.ready {
            if self.current.is_some() || self.engaged {
                out.ended = true;
                out.note = Some("the bubble cannot show (no HUD, a cutscene, a load): Elden Ring's own subtitles and menus".into());
            }
            *self = Conversation::default();
            return out;
        }
        // what to show: the choice first, else the newest line (a line of an engaged talk before an ambient one)
        let want = if let Some((serial, talk_id)) = f.ask {
            Some(Bubble::Ask { serial, talk_id })
        } else {
            let same = match self.current {
                Some(Bubble::Line { ins, .. }) => f.lines.iter().find(|l| l.ins == ins && l.shows()),
                _ => None,
            };
            let pick = same.or_else(|| {
                f.lines
                    .iter()
                    .filter(|l| l.shows())
                    .max_by_key(|l| (f.camera_talks.contains(&l.talk_id), std::cmp::Reverse(l.age), l.row))
            });
            pick.map(|l| Bubble::Line { ins: l.ins, talk_id: l.talk_id, row: l.row, msg: l.msg, skippable: !l.unskippable })
        };
        match want {
            Some(b) => {
                self.idle = 0.0;
                if self.current != Some(b) {
                    out.new_bubble = true;
                }
                self.current = Some(b);
            }
            None => {
                self.idle += f.dt;
                if self.current.is_some() && self.idle >= LINGER {
                    self.current = None;
                }
            }
        }
        // engaged: a choice, or a line / linger of a talk whose camera is on (never by an NPC's line in a battle)
        let talk_of = |b: &Bubble| match *b {
            Bubble::Line { talk_id, .. } | Bubble::Ask { talk_id, .. } => talk_id,
        };
        let engaged_now = f.ask.is_some() || (!f.in_battle && (self.current.as_ref().is_some_and(|b| f.camera_talks.contains(&talk_of(b)) || Some(talk_of(b)) == f.story_talk) || (self.engaged && !f.camera_talks.is_empty())));
        if engaged_now {
            self.since_engaged = 0.0;
            self.engaged = true;
        } else if self.engaged {
            self.since_engaged += f.dt;
            if self.since_engaged >= ENGAGED_LINGER || f.in_battle {
                self.engaged = false;
            }
        }
        out.ended = was && self.current.is_none() && !self.engaged;
        out.bubble = self.current;
        out.engaged = self.engaged;
        out.holds_pad = self.engaged || f.ask.is_some();
        out
    }
}

// ------------------------------------------------------------------------------------- ambient lines (CV2-b, C.5)

/// What the ambient book asks of CV1's host this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmbientAct {
    /// Begin an AMBIENT conversation for this talk object (its NPC the speaker; nobody held, no camera, no A).
    Begin { ins: usize, talk_id: i32 },
    /// Push this line (one subtitle line = one bubble) to the talk object's ambient conversation.
    Line { ins: usize, row: i32, msg: i32 },
    /// The talk object has said nothing for [`AMBIENT_LINGER`]: its ambient conversation ends.
    End { ins: usize },
}

/// Seconds an ambient conversation stays after its talk object's last line (its block's next line follows at once; a
/// taunt's next block within a few frames).
pub const AMBIENT_LINGER: f32 = 1.0;

/// AMBIENT LINES (C.5): every line that is NOT the held conversation's (proximity lines, a hostile NPC's taunts, any line
/// in a battle: the talk camera off, or a battle) gets an ambient conversation of its talk object, one Line step per
/// subtitle line. CV1 decides what shows (a held conversation wins: an ambient begin is then refused = dropped; ambient
/// begins queue behind each other). Pure.
#[derive(Clone, Debug, Default)]
pub struct AmbientBook {
    /// Per talk object: the latest row pushed (or refused), seconds without a line, began (its begin was asked).
    open: Vec<(usize, i32, f32)>,
}

impl AmbientBook {
    /// `held_ins`: the talk object of the held conversation of ours (its lines are the held one's); `held_talk`: a talk
    /// whose lines are held ones (its camera is on and no battle) even before its conversation began.
    pub fn step(&mut self, lines: &[LineSnap], camera_talks: &[i32], in_battle: bool, held_ins: Option<usize>, dt: f32) -> Vec<AmbientAct> {
        self.step_with(lines, camera_talks, in_battle, held_ins, None, dt)
    }

    /// (CV3) `dismissed`: the talk object whose held conversation the leader WALKED AWAY from: its further lines are
    /// ambient (the NPC goes on talking as the player leaves, as in Elden Ring), whatever its talk camera says.
    pub fn step_with(&mut self, lines: &[LineSnap], camera_talks: &[i32], in_battle: bool, held_ins: Option<usize>, dismissed: Option<usize>, dt: f32) -> Vec<AmbientAct> {
        let mut acts = Vec::new();
        for l in lines.iter().filter(|l| l.shows()) {
            if Some(l.ins) == held_ins || (!in_battle && camera_talks.contains(&l.talk_id) && Some(l.ins) != dismissed) {
                continue;
            }
            match self.open.iter_mut().find(|(i, _, _)| *i == l.ins) {
                Some(o) => {
                    o.2 = 0.0;
                    if o.1 != l.row {
                        o.1 = l.row;
                        acts.push(AmbientAct::Line { ins: l.ins, row: l.row, msg: l.msg });
                    }
                }
                None => {
                    self.open.push((l.ins, l.row, 0.0));
                    acts.push(AmbientAct::Begin { ins: l.ins, talk_id: l.talk_id });
                    acts.push(AmbientAct::Line { ins: l.ins, row: l.row, msg: l.msg });
                }
            }
        }
        for o in self.open.iter_mut() {
            let speaking = lines.iter().any(|l| l.ins == o.0 && l.shows());
            if !speaking || Some(o.0) == held_ins {
                o.2 += dt;
            }
        }
        for o in &self.open {
            if o.2 >= AMBIENT_LINGER || Some(o.0) == held_ins {
                acts.push(AmbientAct::End { ins: o.0 });
            }
        }
        self.open.retain(|o| o.2 < AMBIENT_LINGER && Some(o.0) != held_ins);
        acts
    }

    /// CV1 ended (or dropped) this talk object's ambient conversation: a later line of it begins a new one.
    pub fn forget(&mut self, ins: usize) {
        self.open.retain(|o| o.0 != ins);
    }

    pub fn clear(&mut self) {
        self.open.clear();
    }
}

/// (CV3) The talks the player is in: those with their talk camera on, and the talk of a driven story rest (Melina at a
/// grace: her talk camera is not turned on there, so her lines and choice were taken for ambient ones). Pure.
pub fn engaged_talks(mut camera_talks: Vec<i32>, story_talk: Option<i32>) -> Vec<i32> {
    if let Some(t) = story_talk.filter(|t| !camera_talks.contains(t)) {
        camera_talks.push(t);
    }
    camera_talks
}

// ------------------------------------------------------------------------------------ (CV3) the walked-away talk

/// Seconds without a line or a choice of a dismissed talk after which it is forgotten.
pub const DISMISS_QUIET: f32 = 1.5;
/// Seconds with its talk camera off (and nothing said) after which a dismissed talk is forgotten (its script ended it;
/// talking to the NPC again starts a new held conversation).
pub const DISMISS_ENDED: f32 = 0.3;

/// (CV3) A TALK THE LEADER WALKED AWAY FROM (or a fight ended under CombatPolicy::End): no held conversation of ours
/// begins for it again while it goes on (that was the begin / end loop of the user's session: #6 ... #107 in 3 s);
/// its lines are ambient bubbles (the NPC goes on talking as the player leaves, as in Elden Ring); a choice of it is
/// answered with its cancel option ("Leave"). Forgotten when the talk is over. Pure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dismissal {
    pub ins: usize,
    pub talk_id: i32,
    quiet: f32,
    ended: f32,
}

impl Dismissal {
    pub fn new(ins: usize, talk_id: i32) -> Dismissal {
        Dismissal { ins, talk_id, quiet: 0.0, ended: 0.0 }
    }

    /// One frame; false: it is over (forget it).
    pub fn step(&mut self, lines: &[LineSnap], camera_talks: &[i32], ask_talk: Option<i32>, dt: f32) -> bool {
        let speaking = lines.iter().any(|l| l.ins == self.ins && l.shows());
        let asking = ask_talk == Some(self.talk_id);
        let camera = camera_talks.contains(&self.talk_id);
        self.quiet = if speaking || asking { 0.0 } else { self.quiet + dt };
        self.ended = if camera || speaking || asking { 0.0 } else { self.ended + dt };
        self.quiet < DISMISS_QUIET && self.ended < DISMISS_ENDED
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct T;
    impl Texts for T {
        fn flag(&self, id: u32) -> Option<bool> {
            Some(id == 777)
        }
        fn option_text(&self, msg: i32) -> Option<String> {
            match msg {
                23_011_000 => Some("I'll fly straight and true!".into()),
                23_011_001 => Some("I don't need a seat".into()),
                20_000_010 => Some("Purchase".into()),
                20_000_011 => Some("Sell".into()),
                MSG_LEAVE => Some("Leave".into()),
                _ => None,
            }
        }
        fn dialog_text(&self, msg: i32) -> Option<String> {
            (msg == 23_221_007).then(|| "Give the item?".into())
        }
    }

    const VARRE: i32 = 301_006_000;
    const KALE: i32 = 800_006_000;

    fn ready() -> Talks {
        Talks { ready: true, ..Talks::new() }
    }

    #[test]
    fn character_talks_are_known_by_their_ids() {
        assert!(is_character_talk(VARRE, &[]));
        assert!(is_character_talk(3000, &[]), "Melina");
        for system in [1000, 1100, 2000, 2006, 2012, 2100] {
            assert!(!is_character_talk(system, &[]), "{system}");
        }
        assert!(!is_character_talk(VARRE, &[VARRE]), "talk.kh.skip");
    }

    #[test]
    fn the_bubble_text_keeps_one_subtitle_line_whole() {
        assert_eq!(bubble_text("You maidenless runt..."), "You maidenless runt...");
        assert_eq!(bubble_text("<groan>"), "(groan)");
        assert_eq!(bubble_text("Absolution costs <?evntAcquittalPrice?> runes."), "Absolution costs ... runes.");
        assert_eq!(bubble_text("first half\nsecond  half"), "first half second half", "the game's break is a space inside the one bubble");
    }

    /// VARRE'S FIRST CHOICE (t301006000 group x50, PARSED by conv/tools/talkwalk.js): ClearTalkListData,
    /// AddTalkListData(1, 23011000, -1), AddTalkListData(2, 23011001, -1), OpenConversationChoicesMenu(0), then the
    /// script waits while 59(12, 0) is 1 and 58(0) is 0 and reads 23.
    #[test]
    fn varres_choice_replaces_the_game_menu_and_its_answer_goes_back() {
        let mut t = ready();
        let i = Val::Int;
        assert_eq!(t.on_event(VARRE, 1, 2, cmd::CLEAR_LIST, &[], &T), Decision::Pass);
        assert_eq!(t.on_event(VARRE, 1, 2, cmd::ADD_LIST, &[i(1), i(23_011_000), i(-1)], &T), Decision::Pass);
        assert_eq!(t.on_event(VARRE, 1, 2, cmd::ADD_LIST, &[i(2), i(23_011_001), i(-1)], &T), Decision::Pass);
        assert_eq!(t.on_event(VARRE, 1, 2, cmd::CHOICES, &[i(0)], &T), Decision::Swallow, "the game's menu does not open");
        let ask = t.ask.clone().expect("asked");
        assert_eq!(ask.kind, AskKind::Choices);
        assert_eq!(ask.options.iter().map(|o| o.text.as_str()).collect::<Vec<_>>(), ["I'll fly straight and true!", "I don't need a seat"]);
        assert_eq!(ask.cancel, None, "conversation choices have no cancel");
        // the script's wait: the menu reads open, no dialog
        assert_eq!(t.on_env(VARRE, env::MENU_OPEN, &[i(12), i(0)]), Some(1));
        assert_eq!(t.on_env(VARRE, env::MENU_OPEN, &[i(-1), i(0)]), Some(1), "type -1 = any");
        assert_eq!(t.on_env(VARRE, env::MENU_OPEN, &[i(1), i(0)]), None, "not the talk list");
        assert_eq!(t.on_env(VARRE, env::DIALOG_OPEN, &[i(0)]), None);
        assert_eq!(t.on_env(VARRE, env::LIST_RESULT, &[]), None, "no answer yet: the game's own value");
        // the player takes the second option: the script reads its index 2, the menu reads closed
        assert_eq!(t.answer(ask.serial, 1), Some(2));
        assert_eq!(t.on_env(VARRE, env::MENU_OPEN, &[i(12), i(0)]), None);
        assert_eq!(t.on_env(VARRE, env::LIST_RESULT, &[]), Some(2));
        assert_eq!(t.on_env(VARRE, env::LIST_RESULT, &[]), Some(2), "read again: the same");
        assert_eq!(t.on_env(KALE, env::LIST_RESULT, &[]), None, "another talk's answer is not this one's");
    }

    #[test]
    fn options_follow_the_games_flag_rule_and_leave_is_the_cancel() {
        let mut t = ready();
        let i = Val::Int;
        t.on_event(KALE, 1, 2, cmd::CLEAR_LIST, &[], &T);
        t.on_event(KALE, 1, 2, cmd::ADD_LIST, &[i(1), i(20_000_010), i(-1)], &T);
        t.on_event(KALE, 1, 2, cmd::ADD_LIST, &[i(2), i(20_000_011), i(778)], &T); // flag off: not added
        t.on_event(KALE, 1, 2, cmd::ADD_LIST, &[i(2), i(20_000_011), i(777)], &T); // flag on
        t.on_event(KALE, 1, 2, cmd::ADD_LIST, &[i(99), i(MSG_LEAVE), i(-1)], &T);
        assert_eq!(t.on_event(KALE, 1, 2, cmd::SHOW_SHOP_MESSAGE, &[i(1)], &T), Decision::Swallow);
        let ask = t.ask.clone().unwrap();
        assert_eq!(ask.kind, AskKind::TalkList);
        assert_eq!(ask.options.iter().map(|o| o.index).collect::<Vec<_>>(), [1, 2, 99]);
        assert_eq!(ask.cancel, Some(2), "B = Leave");
        assert_eq!(t.on_env(KALE, env::MENU_OPEN, &[i(1), i(0)]), Some(1));
        assert_eq!(t.answer(ask.serial, 2), Some(99));
        // the next menu starts afresh, its old answer is not read again while it is open
        t.on_event(KALE, 1, 2, cmd::CLEAR_LIST, &[], &T);
        t.on_event(KALE, 1, 2, cmd::ADD_LIST, &[i(1), i(20_000_010), i(-1)], &T);
        t.on_event(KALE, 1, 2, cmd::SHOW_SHOP_MESSAGE, &[i(1)], &T);
        assert_eq!(t.on_env(KALE, env::LIST_RESULT, &[]), None);
    }

    /// (CV3 round 4) KALE'S SHOP LOOP (the user's FINAL5 session 2026-10-17, kh-er_final5_session.log 1255 s): the
    /// bubble's "Purchase" opened the shop; back from it the talk list was left to Elden Ring's menu (a game window was
    /// still up); B / "Leave" there were read as the OLD "Purchase" -> the shop again, forever. A list or dialog the
    /// game answers itself never reads an older bubble answer.
    #[test]
    fn a_menu_left_to_elden_ring_never_reads_an_old_bubble_answer() {
        let i = Val::Int;
        let mut t = ready();
        let list = |t: &mut Talks| {
            t.on_event(KALE, 1, 2, cmd::CLEAR_LIST, &[], &T);
            t.on_event(KALE, 1, 2, cmd::ADD_LIST, &[i(1), i(20_000_010), i(-1)], &T);
            t.on_event(KALE, 1, 2, cmd::ADD_LIST, &[i(99), i(MSG_LEAVE), i(-1)], &T);
        };
        list(&mut t);
        assert_eq!(t.on_event(KALE, 1, 2, cmd::SHOW_SHOP_MESSAGE, &[i(1)], &T), Decision::Swallow);
        let ask = t.ask.clone().unwrap();
        assert_eq!(t.answer(ask.serial, 0), Some(1), "Purchase");
        assert_eq!(t.on_env(KALE, env::LIST_RESULT, &[]), Some(1), "the script reads Purchase: the shop opens");
        // back from the shop: the bubble cannot show the list (a game window is up), Elden Ring's menu answers it
        t.ready = false;
        list(&mut t);
        assert_eq!(t.on_event(KALE, 1, 2, cmd::SHOW_SHOP_MESSAGE, &[i(1)], &T), Decision::Pass);
        assert_eq!(t.on_env(KALE, env::LIST_RESULT, &[]), None, "the game's own answer (B / Leave), never the old Purchase");
        // the same when the bubble gives a pending list back to Elden Ring
        let mut t = ready();
        list(&mut t);
        t.on_event(KALE, 1, 2, cmd::SHOW_SHOP_MESSAGE, &[i(1)], &T);
        let ask = t.ask.clone().unwrap();
        t.answer(ask.serial, 0);
        list(&mut t);
        t.on_event(KALE, 1, 2, cmd::SHOW_SHOP_MESSAGE, &[i(1)], &T);
        assert!(t.give_back().is_some());
        assert_eq!(t.on_env(KALE, env::LIST_RESULT, &[]), None, "given back: the game's answer");
        // a dialog Elden Ring answers itself (no text for it) never reads an older bubble dialog answer
        let mut t = ready();
        assert_eq!(t.on_event(VARRE, 1, 2, cmd::GENERIC_DIALOG, &[i(8), i(23_221_007), i(3), i(4), i(2)], &T), Decision::Swallow);
        let ask = t.ask.clone().unwrap();
        t.answer(ask.serial, 0);
        assert!(t.on_env(VARRE, env::DIALOG_RESULT, &[]).is_some());
        assert_eq!(t.on_event(VARRE, 1, 2, cmd::GENERIC_DIALOG, &[i(8), i(28_001_001), i(3), i(4), i(2)], &T), Decision::Pass);
        assert_eq!(t.on_env(VARRE, env::DIALOG_RESULT, &[]), None);
    }

    #[test]
    fn menus_stay_elden_rings_when_the_bubble_cannot_show_them() {
        let i = Val::Int;
        let mut t = Talks::new(); // not ready
        t.on_event(VARRE, 1, 2, cmd::CLEAR_LIST, &[], &T);
        t.on_event(VARRE, 1, 2, cmd::ADD_LIST, &[i(1), i(23_011_000), i(-1)], &T);
        assert_eq!(t.on_event(VARRE, 1, 2, cmd::CHOICES, &[i(0)], &T), Decision::Pass);
        let mut t = ready();
        t.on_event(VARRE, 1, 2, cmd::ADD_LIST, &[i(1), i(28_001_003), i(-1)], &T); // no text (cut content)
        assert_eq!(t.on_event(VARRE, 1, 2, cmd::CHOICES, &[i(0)], &T), Decision::Pass, "an option without text: Elden Ring's menu");
        let mut t = ready();
        t.on_event(1000, 1, 2, cmd::ADD_LIST, &[i(1), i(23_011_000), i(-1)], &T);
        assert_eq!(t.on_event(1000, 1, 2, cmd::SHOW_SHOP_MESSAGE, &[i(1)], &T), Decision::Pass, "the grace's menu is savepoint.rs's");
        let mut t = Talks { enabled: false, ready: true, ..Talks::default() };
        t.on_event(VARRE, 1, 2, cmd::ADD_LIST, &[i(1), i(23_011_000), i(-1)], &T);
        assert_eq!(t.on_event(VARRE, 1, 2, cmd::CHOICES, &[i(0)], &T), Decision::Pass, "talk.kh = 0");
        assert_eq!(t.on_env(VARRE, env::MENU_OPEN, &[i(12), i(0)]), None);
    }

    #[test]
    fn a_yes_no_dialog_answers_one_or_two() {
        let i = Val::Int;
        let mut t = ready();
        assert_eq!(t.on_event(VARRE, 1, 2, cmd::GENERIC_DIALOG, &[i(8), i(23_221_007), i(3), i(4), i(2)], &T), Decision::Swallow);
        let ask = t.ask.clone().unwrap();
        assert_eq!((ask.kind, ask.prompt.as_str(), ask.cancel), (AskKind::YesNo, "Give the item?", Some(1)));
        assert_eq!(t.on_env(VARRE, env::DIALOG_OPEN, &[i(0)]), Some(1));
        assert_eq!(t.on_env(VARRE, env::MENU_OPEN, &[i(-1), i(0)]), None, "a dialog is no menu");
        assert_eq!(t.answer(ask.serial, 1), Some(ANSWER_NO));
        assert_eq!(t.on_env(VARRE, env::DIALOG_OPEN, &[i(0)]), None);
        assert_eq!(t.on_env(VARRE, env::DIALOG_RESULT, &[]), Some(ANSWER_NO));
        assert_eq!(t.on_event(VARRE, 1, 2, cmd::GENERIC_DIALOG, &[i(8), i(28_001_001), i(3), i(4), i(2)], &T), Decision::Pass, "no text: Elden Ring's dialog");
    }

    #[test]
    fn an_interrupted_talk_withdraws_the_choice() {
        let i = Val::Int;
        let mut t = ready();
        t.on_event(VARRE, 1, 2, cmd::ADD_LIST, &[i(1), i(23_011_000), i(-1)], &T);
        t.on_event(VARRE, 1, 2, cmd::CHOICES, &[i(0)], &T);
        assert!(t.ask.is_some());
        t.on_event(VARRE, 1, 2, cmd::CLOSE_MENU, &[], &T);
        assert!(t.ask.is_none());
        assert_eq!(t.on_env(VARRE, env::MENU_OPEN, &[i(-1), i(0)]), None);
    }

    #[test]
    fn the_talk_camera_marks_the_conversation() {
        let i = Val::Int;
        let mut t = ready();
        t.on_event(VARRE, 1, 2, cmd::CAMERA, &[i(1_000_000)], &T);
        assert!(t.camera_on(VARRE));
        t.on_event(VARRE, 1, 2, cmd::CAMERA, &[i(-1)], &T);
        assert!(!t.camera_on(VARRE));
    }

    fn line(ins: usize, talk_id: i32, row: i32, elapsed: f32) -> LineSnap {
        LineSnap { ins, talk_id, row, msg: row * 10, active: true, unskippable: false, elapsed, duration: 4.0, age: 0 }
    }

    /// ONE SUBTITLE LINE = ONE BUBBLE, on Varre's first block (t301006000 x44 state 4 -> TalkParam 30101100 .. 30101105,
    /// six lines): every row the game plays is a new bubble, the same row stays the same bubble, nothing is merged.
    #[test]
    fn each_of_varres_lines_is_one_bubble() {
        let mut c = Conversation::default();
        let cam = vec![VARRE];
        let mut bubbles = Vec::new();
        for row in 30_101_100..=30_101_105 {
            for frame in 0..90 {
                let out = c.step(&FrameIn { lines: vec![line(7, VARRE, row, frame as f32 / 60.0)], camera_talks: cam.clone(), ready: true, dt: 1.0 / 60.0, ..FrameIn::default() });
                if out.new_bubble {
                    bubbles.push(out.bubble.unwrap());
                }
                assert!(out.engaged && out.holds_pad);
            }
        }
        assert_eq!(bubbles.len(), 6, "{bubbles:?}");
        for (k, b) in bubbles.iter().enumerate() {
            assert_eq!(*b, Bubble::Line { ins: 7, talk_id: VARRE, row: 30_101_100 + k as i32, msg: (30_101_100 + k as i32) * 10, skippable: true });
        }
    }


    /// Varre's hostile lines (x39: "You will die nameless, without ceremony.", TalkToPlayer flag 1, no talk camera)
    /// and any line in a battle: a bubble only, nothing locked, the pad stays Sora's, A is his.
    #[test]
    fn ambient_and_battle_lines_are_bubbles_only() {
        let mut c = Conversation::default();
        let mut l = line(9, VARRE, 30_180_400, 0.2);
        l.unskippable = true;
        let out = c.step(&FrameIn { lines: vec![l], ready: true, dt: 1.0 / 60.0, ..FrameIn::default() });
        assert!(out.bubble.is_some() && out.new_bubble);
        assert!(!out.engaged && !out.holds_pad);
        let mut c = Conversation::default();
        let out = c.step(&FrameIn { lines: vec![line(9, VARRE, 30_101_100, 0.2)], camera_talks: vec![VARRE], in_battle: true, ready: true, dt: 1.0 / 60.0, ..FrameIn::default() });
        assert!(out.bubble.is_some() && !out.engaged && !out.holds_pad, "in a battle nothing is locked");
    }

    #[test]
    fn a_choice_takes_the_bubble_and_the_pad() {
        let mut c = Conversation::default();
        let out = c.step(&FrameIn { ask: Some((5, VARRE)), ready: true, dt: 1.0 / 60.0, in_battle: true, ..FrameIn::default() });
        assert_eq!(out.bubble, Some(Bubble::Ask { serial: 5, talk_id: VARRE }));
        assert!(out.new_bubble && out.holds_pad, "a pending choice always holds the pad (the script waits for it)");
    }

    #[test]
    fn the_conversation_lingers_between_blocks_and_ends() {
        let mut c = Conversation::default();
        let dt = 1.0 / 60.0;
        c.step(&FrameIn { lines: vec![line(7, VARRE, 30_101_105, 3.9)], camera_talks: vec![VARRE], ready: true, dt, ..FrameIn::default() });
        // a gap of 10 frames between two blocks, the camera still on: the same conversation, the bubble kept
        for _ in 0..10 {
            let out = c.step(&FrameIn { camera_talks: vec![VARRE], ready: true, dt, ..FrameIn::default() });
            assert!(out.engaged && out.bubble.is_some() && !out.ended);
        }
        let out = c.step(&FrameIn { lines: vec![line(7, VARRE, 30_101_200, 0.0)], camera_talks: vec![VARRE], ready: true, dt, ..FrameIn::default() });
        assert!(out.new_bubble);
        // the talk ends: camera off, no line: the bubble goes after LINGER, the conversation after ENGAGED_LINGER
        let mut ended = false;
        for _ in 0..80 {
            let out = c.step(&FrameIn { ready: true, dt, ..FrameIn::default() });
            ended |= out.ended;
        }
        assert!(ended && c.current.is_none());
        assert!(!c.step(&FrameIn { ready: true, dt, ..FrameIn::default() }).engaged);
    }

    #[test]
    fn not_ready_gives_everything_back() {
        let mut c = Conversation::default();
        c.step(&FrameIn { lines: vec![line(7, VARRE, 30_101_100, 0.1)], camera_talks: vec![VARRE], ready: true, dt: 0.016, ..FrameIn::default() });
        let out = c.step(&FrameIn { lines: vec![line(7, VARRE, 30_101_100, 0.2)], camera_talks: vec![VARRE], ready: false, dt: 0.016, ..FrameIn::default() });
        assert!(out.ended && out.bubble.is_none() && !out.holds_pad);
    }

    /// Two NPCs (a multi-NPC scene): the newest line has the bubble; while one speaks the other's older line does not
    /// take it back.
    #[test]
    fn several_speakers_take_turns() {
        let mut c = Conversation::default();
        let dt = 1.0 / 60.0;
        let a = line(1, 310_006_000, 31_000_000, 1.0);
        let mut b = line(2, 311_006_000, 31_100_000, 0.0);
        let out = c.step(&FrameIn { lines: vec![a], camera_talks: vec![310_006_000], ready: true, dt, ..FrameIn::default() });
        assert!(matches!(out.bubble, Some(Bubble::Line { ins: 1, .. })));
        // a's line is over (not active), b speaks
        let mut a2 = a;
        a2.active = false;
        let out = c.step(&FrameIn { lines: vec![a2, b], camera_talks: vec![310_006_000], ready: true, dt, ..FrameIn::default() });
        assert!(matches!(out.bubble, Some(Bubble::Line { ins: 2, .. })) && out.new_bubble);
        b.elapsed = 0.5;
        let out = c.step(&FrameIn { lines: vec![a2, b], camera_talks: vec![310_006_000], ready: true, dt, ..FrameIn::default() });
        assert!(!out.new_bubble);
    }

    /// A HOSTILE VARRE MID-FIGHT (t301006000 x41: block 30180200, three lines, TalkToPlayer flag 1, no talk camera): an
    /// ambient conversation of his, one Line per subtitle line, ended after the linger.
    #[test]
    fn a_hostile_npcs_taunts_mid_fight_are_ambient_bubbles() {
        let mut book = AmbientBook::default();
        let dt = 1.0 / 60.0;
        let mut acts = Vec::new();
        for row in 30_180_200..=30_180_202 {
            for f in 0..60 {
                let mut l = line(9, VARRE, row, f as f32 / 60.0);
                l.unskippable = true;
                acts.extend(book.step(&[l], &[], true, None, dt));
            }
        }
        assert_eq!(acts[0], AmbientAct::Begin { ins: 9, talk_id: VARRE });
        let rows: Vec<i32> = acts.iter().filter_map(|a| if let AmbientAct::Line { row, .. } = a { Some(*row) } else { None }).collect();
        assert_eq!(rows, [30_180_200, 30_180_201, 30_180_202], "one bubble per subtitle line");
        assert_eq!(acts.iter().filter(|a| matches!(a, AmbientAct::Begin { .. })).count(), 1, "one ambient conversation for the block");
        // the talk falls silent: the conversation ends after the linger
        let mut end = Vec::new();
        for _ in 0..70 {
            end.extend(book.step(&[], &[], true, None, dt));
        }
        assert_eq!(end, [AmbientAct::End { ins: 9 }]);
    }

    #[test]
    fn held_talk_lines_are_not_ambient() {
        let mut book = AmbientBook::default();
        // the talk camera on, no battle: the held conversation's
        assert!(book.step(&[line(7, VARRE, 30_101_100, 0.1)], &[VARRE], false, None, 0.016).is_empty());
        // the held conversation's own talk object, even in a battle
        assert!(book.step(&[line(7, VARRE, 30_101_100, 0.1)], &[VARRE], true, Some(7), 0.016).is_empty());
        // the camera on but a battle: ambient (a fight's lines)
        assert!(matches!(book.step(&[line(8, KALE, 80_000_000, 0.1)], &[KALE], true, None, 0.016)[0], AmbientAct::Begin { ins: 8, .. }));
        // a proximity line of another NPC while a held conversation runs: still asked (CV1 refuses = drops it)
        assert!(matches!(book.step(&[line(5, 310_006_000, 31_000_000, 0.1)], &[VARRE], false, Some(7), 0.016)[0], AmbientAct::Begin { ins: 5, .. }));
    }

    #[test]
    fn an_ambient_talk_that_becomes_held_ends() {
        let mut book = AmbientBook::default();
        book.step(&[line(7, VARRE, 30_101_100, 0.1)], &[], false, None, 0.016);
        let acts = book.step(&[line(7, VARRE, 30_101_100, 0.2)], &[VARRE], false, Some(7), 0.016);
        assert_eq!(acts, [AmbientAct::End { ins: 7 }]);
    }

    /// (CV3) WALKED AWAY FROM VARRE: no new held conversation for his talk while it goes on (the begin / end loop of the
    /// user's session), his further lines are ambient bubbles even with his talk camera on, and it is forgotten once
    /// his talk is over (camera off and quiet, or quiet long enough).
    #[test]
    fn cv3_a_talk_walked_away_from_goes_ambient_and_is_forgotten_when_over() {
        let mut d = Dismissal::new(7, VARRE);
        let dt = 1.0 / 60.0;
        // he goes on talking, his camera on: still dismissed; his lines are ambient
        for k in 0..120 {
            assert!(d.step(&[line(7, VARRE, 30_101_101, k as f32 * dt)], &[VARRE], None, dt));
        }
        let mut book = AmbientBook::default();
        let acts = book.step_with(&[line(7, VARRE, 30_101_101, 0.1)], &[VARRE], false, None, Some(7), dt);
        assert!(acts.contains(&AmbientAct::Begin { ins: 7, talk_id: VARRE }), "his line is an ambient bubble: {acts:?}");
        let held = AmbientBook::default().step(&[line(7, VARRE, 30_101_101, 0.1)], &[VARRE], false, None, dt);
        assert!(held.is_empty(), "(not dismissed: a held line, not ambient)");
        // a choice of his pending keeps it
        assert!(d.step(&[], &[VARRE], Some(VARRE), dt));
        // his script ends the talk (camera off, nothing said): forgotten after DISMISS_ENDED
        let mut over = false;
        for _ in 0..((DISMISS_ENDED / dt) as usize + 2) {
            over |= !d.step(&[], &[], None, dt);
        }
        assert!(over, "the talk is over: forgotten");
        // camera left on but nothing said: forgotten after DISMISS_QUIET
        let mut d = Dismissal::new(7, VARRE);
        let n = (0..400).take_while(|_| d.step(&[], &[VARRE], None, dt)).count();
        assert!((n as f32 * dt - DISMISS_QUIET).abs() < 0.05, "{n}");
    }

    /// (CV3) MELINA IN A DRIVEN STORY REST (SV run2, 262 s): no talk camera, so her lines were ambient and her Accept /
    /// Refuse sat in a conversation nobody could step. With the story talk engaged her lines are held bubbles.
    #[test]
    fn cv3_a_story_rest_talk_is_the_players_conversation() {
        const MELINA: i32 = 3000;
        let lines = [line(4, MELINA, 30_000_100, 0.2)];
        let dt = 1.0 / 60.0;
        let plain = engaged_talks(Vec::new(), None);
        let mut c = Conversation::default();
        assert!(!c.step(&FrameIn { lines: lines.to_vec(), camera_talks: plain.clone(), ready: true, dt, ..FrameIn::default() }).engaged, "(before: ambient)");
        assert!(!AmbientBook::default().step(&lines, &plain, false, None, dt).is_empty(), "(before: an ambient bubble A cannot advance)");
        let story = engaged_talks(Vec::new(), Some(MELINA));
        assert_eq!(engaged_talks(story.clone(), Some(MELINA)), vec![MELINA], "once");
        let mut c = Conversation::default();
        assert!(c.step(&FrameIn { lines: lines.to_vec(), story_talk: Some(MELINA), ready: true, dt, ..FrameIn::default() }).engaged, "her line is the held conversation");
        assert!(AmbientBook::default().step(&lines, &story, false, None, dt).is_empty(), "not ambient");
        // her last line is over (an item popup of the game comes next): the conversation ENDS, the pad goes back to the
        // game (before, a story talk in the camera list kept it engaged for good and A / the game's OK were masked)
        let mut ended = false;
        for _ in 0..90 {
            ended |= c.step(&FrameIn { story_talk: Some(MELINA), ready: true, dt, ..FrameIn::default() }).ended;
        }
        assert!(ended, "it ends once she is quiet");
    }
}
