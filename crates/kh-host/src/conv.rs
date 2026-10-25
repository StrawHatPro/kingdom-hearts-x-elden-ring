//! (CV1) THE CONVERSATION RUNNER, engine-free, independent of who talks: a conversation is a list of steps (a line, a
//! choice, a wait, a move / turn directive, a camera shot, a label / goto, the end) run against a speech-bubble view
//! (kh_ui::bubble through [`ConvView`]; kh_host's `Hud` is one) with the pad's A / D-pad / stick / B going to the bubble.
//! The host reads [`ConvEvent`]s (play this voice / animation, stop the voice, perform this directive, this camera
//! shot, a choice was made, the end) and answers through [`ConvInput`] and the methods below. Steps may be given all at
//! once (a script: Donald's talk) or pushed while it runs (an open-ended conversation: the Elden Ring talk adapter
//! feeds the game's talk script's lines and choices one by one).
//!
//! THE RULES (the user, 2026-10-16):
//! - "During a conversation, those in the conversation should not be able to move unless specifically directed by the
//!   conversation": every participant is HELD for the whole conversation ([`Conversation::holds`]), except while a
//!   directive releases or moves him ([`Directive`]).
//! - "Elden Ring character's audio should still play, and is skipped if you press A before the voice line finishes": a
//!   line's voice plays (the host plays it on [`ConvEvent::Line`]); A on the line while it still plays gives
//!   [`ConvEvent::StopVoice`] and the conversation goes on. (The first A on a line still being typed shows all of it,
//!   as KH; a line with `reveal_cps = Some(0.0)` is shown at once, so its first A skips.)
//! - "This conversation system needs to allow for choices": [`Step::Choice`] with branches (labels) for scripts, and
//!   `external` choices whose pick goes to the host ([`ConvEvent::Chosen`]), which pushes what follows.
//! - Combat: an enemy fight ends the conversation (or suspends it, by [`CombatPolicy`]) gracefully.
//! - The camera: every line gets a shot from the shot director (kh_camera::conv), unless the step names one.
//!
//! (CV3) THE USER, 2026-10-17: "remove the cinematic camera from talking and allow the player to move the camera and
//! their character, though A is reserved for moving dialogue along while in conversation unless in combat". So:
//! - THE LEADER IS NEVER HELD ([`Conversation::holds`] is false for [`Role::Leader`]): the host leaves him his stick,
//!   his camera and every button but A ([`Conversation::takes_a`]); at a choice also the D-pad and B
//!   ([`Conversation::in_choice`]). The others (NPCs, staged friends) are held as before (they face the speaker).
//! - IN COMBAT ([`CombatPolicy::HandsFree`], THE DEFAULT): the conversation goes on HANDS-FREE: the bubble takes no key
//!   (A is the leader's attack again), each line goes on by itself ([`Conversation::combat_auto`] s after its voice and
//!   its text), a choice waits (shown, passive) until the fight is over, then takes A / the D-pad again.
//! - WALKING AWAY ([`Conversation::walk_away`], metres; the host gives [`ConvInput::apart`]): farther than that from
//!   the others ends it gracefully (the bubble pops out, [`EndReason::WalkedAway`]).
//! - The camera: the shots are still picked (logs, an opt-in host camera); the Elden Ring host shows none by default.

use std::collections::HashSet;

pub use kh_camera::conv::{ShotCue, ShotKind};
/// The whole conversation camera (framing, director, collision) for hosts and harnesses without kh-camera of their own.
pub use kh_camera::conv as camera;
use kh_camera::conv::ShotDirector;

/// A participant's id: chosen by the host (unique in the conversation).
pub type ActorId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// The party's leader (the player).
    Leader,
    /// A party member who is not the leader.
    Party,
    /// Anyone else (an NPC, a staged friend).
    Npc,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Actor {
    pub id: ActorId,
    /// The name shown in yellow (empty: no name line).
    pub name: String,
    pub role: Role,
}

/// A voice line: the host's key for it (a KH3 voice group, an Elden Ring talk / sound id ...), its length when known.
#[derive(Clone, Debug, PartialEq)]
pub struct Voice {
    pub key: String,
    /// Seconds; None = the host says when it ends ([`ConvInput::voice_playing`]).
    pub seconds: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Line {
    pub speaker: ActorId,
    /// Who he talks to (the camera's other; None: the leader, or the speaker before).
    pub listener: Option<ActorId>,
    /// The words (markup allowed: `<color(x)>..<color>`, `<icon(Name)>`). `{leader}` = the leader's name.
    pub text: String,
    pub voice: Option<Voice>,
    /// An animation / face for the host to play on the speaker.
    pub anim: Option<String>,
    pub face: Option<String>,
    /// Typewriter speed (characters a second); None = KH's; Some(0.0) = all at once.
    pub reveal_cps: Option<f32>,
    /// The line goes on by itself this many seconds after its voice ended (or, without a voice, after it was all
    /// shown); None = it waits for A.
    pub auto_after: Option<f32>,
    /// This line's camera (None: the shot director's pick).
    pub shot: Option<ShotKind>,
}

impl Line {
    pub fn new(speaker: ActorId, text: &str) -> Line {
        Line { speaker, text: text.to_string(), ..Line::default() }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Branch {
    /// The step after the choice.
    Next,
    Goto(String),
    End,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChoiceOption {
    pub text: String,
    pub branch: Branch,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    /// Who asks (the bubble's tail and name; None: no tail, no name).
    pub speaker: Option<ActorId>,
    pub prompt: String,
    pub options: Vec<ChoiceOption>,
    pub start: usize,
    /// B takes this option (None: B does nothing).
    pub cancel: Option<usize>,
    /// The pick is the host's to handle ([`ConvEvent::Chosen`]; branches are ignored, the conversation goes on with the
    /// next step, which the host pushes).
    pub external: bool,
}

/// Where a turn faces.
#[derive(Clone, Debug, PartialEq)]
pub enum Facing {
    Actor(ActorId),
    /// KH3 yaw, degrees.
    Yaw(f32),
    /// A point (the host's space).
    Point([f32; 3]),
}

/// THE ONLY TIMES PARTICIPANTS MOVE.
#[derive(Clone, Debug, PartialEq)]
pub enum Directive {
    Turn { actor: ActorId, face: Facing },
    MoveTo { actor: ActorId, to: [f32; 3], run: bool },
    Play { actor: ActorId, anim: String },
    /// The actor may move freely (his own will / AI) until `Hold`.
    Release { actor: ActorId },
    Hold { actor: ActorId },
}

impl Directive {
    pub fn actor(&self) -> ActorId {
        match self {
            Directive::Turn { actor, .. } | Directive::MoveTo { actor, .. } | Directive::Play { actor, .. } | Directive::Release { actor } | Directive::Hold { actor } => *actor,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Label(String),
    Line(Line),
    Choice(Choice),
    Wait(f32),
    /// `wait`: the conversation waits until the host says it is done ([`Conversation::directive_done`]) or 8 s passed.
    Direct { directive: Directive, wait: bool },
    Camera(ShotCue),
    Goto(String),
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombatPolicy {
    /// A fight ends it.
    End,
    /// (CV3, THE DEFAULT) A fight makes it hands-free: the bubble takes no key (A attacks), lines go on by themselves
    /// ([`Conversation::combat_auto`]), a choice waits shown until the fight is over.
    HandsFree,
    /// A fight hides it; it comes back (the line it was on) `RESUME_AFTER` seconds after the fight.
    Suspend,
}

pub const RESUME_AFTER: f32 = 1.5;
/// (C.5) Seconds an ambient line stays after its voice ended / it was all shown, when it names no `auto_after`.
pub const AMBIENT_AUTO: f32 = 1.2;
pub const DIRECTIVE_TIMEOUT: f32 = 8.0;
/// (CV3) Seconds a line stays in a fight after its voice ended / it was all shown (hands-free), unless it names
/// `auto_after`.
pub const COMBAT_AUTO: f32 = 1.0;
/// (CV3) Metres from the others at which walking away ends a held conversation (the host's key may change it).
pub const WALK_AWAY: f32 = 5.0;

#[derive(Clone, Debug, PartialEq)]
pub enum EndReason {
    /// The script's end (or `Step::End`, or a branch to the end).
    Script,
    /// A fight began (policy End).
    Combat,
    /// The host ended it ([`Conversation::end`]): its words.
    Host(String),
    /// (CV3) The leader walked away (farther than [`Conversation::walk_away`] from the others).
    WalkedAway,
}

/// What the host does.
#[derive(Clone, Debug, PartialEq)]
pub enum ConvEvent {
    Started,
    /// A line is up: play its voice / animation / face on the speaker.
    Line { index: usize, speaker: ActorId, listener: Option<ActorId>, voice: Option<Voice>, anim: Option<String>, face: Option<String> },
    /// A was pressed while the line's voice played: stop it.
    StopVoice { speaker: ActorId },
    /// The line is over (A, or by itself).
    LineDone { index: usize },
    ChoiceUp { index: usize },
    /// An option was taken.
    Chosen { index: usize, option: usize },
    /// Perform it (the participant moves / turns / plays for it).
    Direct(Directive),
    /// The camera's next shot.
    Shot(ShotCue),
    Suspended,
    Resumed,
    Ended(EndReason),
}

/// The bubble's side, as the runner sees it (kh_host's `Hud` implements it over kh_ui::bubble).
pub trait ConvView {
    fn conv_line(&mut self, name: Option<&str>, text: &str, reveal_cps: Option<f32>);
    fn conv_choice(&mut self, name: Option<&str>, prompt: &str, options: &[String], start: usize, cancel: Option<usize>);
    /// The Keyblade cue (None: when a line is all shown).
    fn conv_cue(&mut self, cue: Option<bool>);
    fn conv_close(&mut self);
    fn conv_events(&mut self) -> Vec<kh_ui::bubble::BubbleEvent>;
    /// The line is all shown.
    fn conv_line_shown(&self) -> bool;
    /// (C.5) An AMBIENT line: the bubble takes no key (no A, no cursor) and shows no Keyblade cue.
    fn conv_passive(&mut self, _passive: bool) {}
}

/// What the host tells the runner each frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ConvInput {
    pub dt: f32,
    /// A fight is on (enemies have engaged the party).
    pub combat: bool,
    /// The current line's voice: Some(true) while it plays, Some(false) once it ended, None = not known (the line's
    /// `Voice::seconds` is used).
    pub voice_playing: Option<bool>,
    /// (CV3) Metres from the leader to the nearest other participant (None: not known / no leader in it).
    pub apart: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
enum Run {
    /// Runs the next steps.
    Next,
    Line { index: usize, voice_left: Option<f32>, auto: Option<f32>, shown_for: f32 },
    Choice { index: usize },
    Wait(f32),
    Directing { actor: ActorId, left: f32 },
    Ended(EndReason),
}

pub struct Conversation {
    pub actors: Vec<Actor>,
    steps: Vec<Step>,
    pc: usize,
    run: Run,
    /// More steps may come ([`Conversation::push`]): out of steps it waits instead of ending.
    pub open_ended: bool,
    pub policy: CombatPolicy,
    /// The camera's shots are picked for the lines (false: only `Step::Camera`).
    pub auto_camera: bool,
    /// (C.5) AN AMBIENT CONVERSATION (proximity lines, taunts, battle lines): nobody is held, no shots are picked, A does
    /// nothing (the bubble is passive), every line goes on by itself (`auto_after`, else [`AMBIENT_AUTO`] seconds after
    /// its voice / its text), a choice is skipped (its cancel option, else the first, is taken).
    pub ambient: bool,
    /// (CV3) Seconds a line stays in a fight (hands-free) after its voice / text, unless it names `auto_after`.
    pub combat_auto: f32,
    /// (CV3) Walking farther than this (m, [`ConvInput::apart`]) ends it; None: never.
    pub walk_away: Option<f32>,
    /// (CV3) A fight is on and the policy is HandsFree: the bubble is passive.
    hands_free: bool,
    suspended: Option<f32>,
    released: HashSet<ActorId>,
    directed: Option<Directive>,
    events: Vec<ConvEvent>,
    director: ShotDirector,
    speaker: Option<ActorId>,
    listener: Option<ActorId>,
    started: bool,
    /// The leader's name for `{leader}`.
    pub leader_name: String,
    /// Lines shown so far (testing / logs).
    pub lines_shown: usize,
}

impl Conversation {
    /// A conversation of these participants and steps.
    pub fn new(actors: Vec<Actor>, steps: Vec<Step>) -> Conversation {
        let leader_name = actors.iter().find(|a| a.role == Role::Leader).map_or(String::new(), |a| a.name.clone());
        Conversation { actors, steps, pc: 0, run: Run::Next, open_ended: false, policy: CombatPolicy::HandsFree, auto_camera: true, ambient: false, combat_auto: COMBAT_AUTO, walk_away: Some(WALK_AWAY), hands_free: false, suspended: None, released: HashSet::new(), directed: None, events: Vec::new(), director: ShotDirector::default(), speaker: None, listener: None, started: false, leader_name, lines_shown: 0 }
    }

    /// An open-ended conversation (steps pushed while it runs).
    pub fn open(actors: Vec<Actor>) -> Conversation {
        let mut c = Conversation::new(actors, Vec::new());
        c.open_ended = true;
        c
    }

    pub fn push(&mut self, step: Step) {
        self.steps.push(step);
    }

    pub fn is_over(&self) -> bool {
        matches!(self.run, Run::Ended(_))
    }

    pub fn end_reason(&self) -> Option<&EndReason> {
        match &self.run {
            Run::Ended(r) => Some(r),
            _ => None,
        }
    }

    pub fn suspended(&self) -> bool {
        self.suspended.is_some()
    }

    /// Who speaks now (the bubble's tail) and to whom.
    pub fn speaker(&self) -> Option<ActorId> {
        self.speaker
    }

    pub fn listener(&self) -> Option<ActorId> {
        self.listener
    }

    pub fn actor(&self, id: ActorId) -> Option<&Actor> {
        self.actors.iter().find(|a| a.id == id)
    }

    /// THE LOCK: this participant may not move, attack or be moved by his AI now. Only a `Release` directive (until a
    /// `Hold`) frees him; a directive being performed moves him ([`Conversation::directed`]). (CV3) NEVER THE LEADER:
    /// the player keeps his character and his camera (only A, and at a choice the D-pad / B, are the bubble's).
    pub fn holds(&self, actor: ActorId) -> bool {
        !self.ambient && !self.is_over() && self.actors.iter().any(|a| a.id == actor && a.role != Role::Leader) && !self.released.contains(&actor)
    }

    /// (CV3) A IS THE BUBBLE'S now (a held conversation, out of a fight, not suspended): the host keeps it from the
    /// leader's action (outside combat A does nothing else).
    pub fn takes_a(&self) -> bool {
        !self.ambient && !self.is_over() && !self.hands_free && self.suspended.is_none()
    }

    /// (CV3) A choice is up and takes keys: the D-pad moves the glove, A takes, B takes the cancel option (the stick
    /// stays the leader's movement).
    pub fn in_choice(&self) -> bool {
        self.takes_a() && matches!(self.run, Run::Choice { .. })
    }

    /// (CV3) A fight made it hands-free (the bubble passive, lines going on by themselves, a choice waiting).
    pub fn hands_free(&self) -> bool {
        self.hands_free
    }

    /// The directive being performed (the host moves its actor for it).
    pub fn directed(&self) -> Option<&Directive> {
        self.directed.as_ref()
    }

    /// The host performed the directive of `actor`.
    pub fn directive_done(&mut self, actor: ActorId) {
        if let Run::Directing { actor: a, .. } = self.run {
            if a == actor {
                self.directed = None;
                self.run = Run::Next;
            }
        }
    }

    /// The host ends it (the view closes).
    pub fn end(&mut self, view: &mut dyn ConvView, why: &str) {
        if !self.is_over() {
            view.conv_close();
            self.finish(EndReason::Host(why.to_string()));
        }
    }

    pub fn take_events(&mut self) -> Vec<ConvEvent> {
        std::mem::take(&mut self.events)
    }

    fn finish(&mut self, why: EndReason) {
        self.directed = None;
        self.events.push(ConvEvent::Ended(why.clone()));
        self.run = Run::Ended(why);
    }

    fn label(&self, name: &str) -> Option<usize> {
        self.steps.iter().position(|s| matches!(s, Step::Label(l) if l == name))
    }

    fn name_of(&self, id: Option<ActorId>) -> Option<String> {
        id.and_then(|id| self.actor(id)).map(|a| a.name.clone()).filter(|n| !n.is_empty())
    }

    /// The listener of a line: the one named, else the one who spoke before (if another), else the leader.
    fn listener_for(&self, l: &Line) -> Option<ActorId> {
        l.listener.or(self.speaker.filter(|s| *s != l.speaker)).or_else(|| self.actors.iter().find(|a| a.role == Role::Leader && a.id != l.speaker).map(|a| a.id)).or_else(|| self.actors.iter().find(|a| a.id != l.speaker).map(|a| a.id))
    }

    /// Shows the current step again (after a suspension).
    fn reshow(&mut self, view: &mut dyn ConvView) {
        match self.run.clone() {
            Run::Line { index, .. } => {
                if let Some(Step::Line(l)) = self.steps.get(index).cloned() {
                    view.conv_line(self.name_of(Some(l.speaker)).as_deref(), &l.text.replace("{leader}", &self.leader_name), Some(0.0));
                }
            }
            Run::Choice { index } => {
                if let Some(Step::Choice(c)) = self.steps.get(index).cloned() {
                    let options: Vec<String> = c.options.iter().map(|o| o.text.clone()).collect();
                    let named = c.speaker.and_then(|s| self.actor(s)).is_some_and(|a| a.role != Role::Leader); // (CV3)
                    view.conv_choice(if named { self.name_of(c.speaker) } else { None }.as_deref(), &c.prompt, &options, c.start, c.cancel);
                }
            }
            _ => {}
        }
    }

    /// ONE FRAME.
    pub fn step(&mut self, view: &mut dyn ConvView, input: &ConvInput) {
        if self.is_over() {
            return;
        }
        if !self.started {
            self.started = true;
            self.events.push(ConvEvent::Started);
        }
        // ---- (CV3) walking away ends it gracefully
        if let (Some(limit), Some(apart), false) = (self.walk_away, input.apart, self.ambient) {
            if apart > limit {
                view.conv_close();
                self.finish(EndReason::WalkedAway);
                return;
            }
        }
        // ---- (CV3) a fight makes it hands-free (the default policy); its end gives the bubble its keys again
        let free_now = input.combat && !self.ambient && self.policy == CombatPolicy::HandsFree;
        if free_now != self.hands_free {
            self.hands_free = free_now;
            view.conv_passive(free_now);
            view.conv_cue(if free_now { Some(false) } else { None });
            let _ = view.conv_events(); // (a key of the frame the fight began or ended is nobody's)
        }
        // ---- a fight
        if input.combat && !self.ambient && self.policy != CombatPolicy::HandsFree {
            // (C.5) an ambient conversation goes on in a fight (battle lines)
            match self.policy {
                CombatPolicy::HandsFree => {}
                CombatPolicy::End => {
                    view.conv_close();
                    self.finish(EndReason::Combat);
                    return;
                }
                CombatPolicy::Suspend => {
                    if self.suspended.is_none() {
                        view.conv_close();
                        self.events.push(ConvEvent::Suspended);
                    }
                    self.suspended = Some(0.0);
                    let _ = view.conv_events();
                    return;
                }
            }
        }
        if let Some(t) = self.suspended {
            let t = t + input.dt;
            if t < RESUME_AFTER {
                self.suspended = Some(t);
                let _ = view.conv_events();
                return;
            }
            self.suspended = None;
            self.events.push(ConvEvent::Resumed);
            self.reshow(view);
        }
        // ---- the bubble's events ((CV3) none while hands-free: the keys are the fight's)
        let bubble_events = view.conv_events();
        for e in bubble_events.into_iter().filter(|_| !self.hands_free) {
            use kh_ui::bubble::BubbleEvent as B;
            match (e, self.run.clone()) {
                (B::Advanced, Run::Line { index, voice_left, .. }) => {
                    let playing = input.voice_playing.unwrap_or(voice_left.is_some_and(|v| v > 0.0));
                    let has_voice = matches!(self.steps.get(index), Some(Step::Line(l)) if l.voice.is_some());
                    if playing && has_voice {
                        if let Some(s) = self.speaker {
                            self.events.push(ConvEvent::StopVoice { speaker: s });
                        }
                    }
                    self.events.push(ConvEvent::LineDone { index });
                    self.pc = index + 1;
                    self.run = Run::Next;
                }
                (B::Chosen(option), Run::Choice { index }) => {
                    self.events.push(ConvEvent::Chosen { index, option });
                    let Some(Step::Choice(c)) = self.steps.get(index).cloned() else { continue };
                    self.pc = index + 1;
                    self.run = Run::Next;
                    if !c.external {
                        match c.options.get(option).map(|o| o.branch.clone()) {
                            Some(Branch::Goto(l)) => match self.label(&l) {
                                Some(k) => self.pc = k,
                                None => {
                                    view.conv_close();
                                    self.finish(EndReason::Script);
                                    return;
                                }
                            },
                            Some(Branch::End) => {
                                view.conv_close();
                                self.finish(EndReason::Script);
                                return;
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        // ---- the running step
        match &mut self.run {
            Run::Line { index, voice_left, auto, shown_for } => {
                let index = *index;
                if let Some(v) = voice_left.as_mut() {
                    *v -= input.dt;
                }
                let voice_over = match input.voice_playing {
                    Some(p) => !p,
                    None => voice_left.is_none_or(|v| v <= 0.0),
                };
                if view.conv_line_shown() {
                    *shown_for += input.dt;
                }
                // (CV3) hands-free in a fight: the line goes on by itself (its own auto_after, else combat_auto)
                let wait = if self.hands_free { Some(auto.unwrap_or(self.combat_auto)) } else { *auto };
                if let Some(wait) = wait {
                    if voice_over && view.conv_line_shown() && *shown_for >= wait.max(0.0) {
                        self.events.push(ConvEvent::LineDone { index });
                        self.pc = index + 1;
                        self.run = Run::Next;
                    }
                }
            }
            Run::Wait(t) => {
                *t -= input.dt;
                if *t <= 0.0 {
                    self.run = Run::Next;
                }
            }
            Run::Directing { left, .. } => {
                *left -= input.dt;
                if *left <= 0.0 {
                    self.directed = None;
                    self.run = Run::Next;
                }
            }
            _ => {}
        }
        // ---- the next steps (as many as do not wait)
        let mut guard = 0;
        while self.run == Run::Next && guard < 256 {
            guard += 1;
            let Some(step) = self.steps.get(self.pc).cloned() else {
                if !self.open_ended {
                    view.conv_close();
                    self.finish(EndReason::Script);
                }
                return;
            };
            match step {
                Step::Label(_) => self.pc += 1,
                Step::Goto(l) => match self.label(&l) {
                    Some(k) => self.pc = k + 1,
                    None => {
                        view.conv_close();
                        self.finish(EndReason::Script);
                        return;
                    }
                },
                Step::End => {
                    view.conv_close();
                    self.finish(EndReason::Script);
                    return;
                }
                Step::Wait(t) => {
                    self.pc += 1;
                    self.run = Run::Wait(t);
                }
                Step::Camera(cue) => {
                    self.pc += 1;
                    self.events.push(ConvEvent::Shot(cue));
                }
                Step::Direct { directive, wait } => {
                    self.pc += 1;
                    match &directive {
                        Directive::Release { actor } => {
                            self.released.insert(*actor);
                        }
                        Directive::Hold { actor } => {
                            self.released.remove(actor);
                        }
                        _ => {}
                    }
                    self.events.push(ConvEvent::Direct(directive.clone()));
                    if wait && !matches!(directive, Directive::Release { .. } | Directive::Hold { .. }) {
                        self.run = Run::Directing { actor: directive.actor(), left: DIRECTIVE_TIMEOUT };
                        self.directed = Some(directive);
                    }
                }
                Step::Line(l) => {
                    let index = self.pc;
                    let listener = self.listener_for(&l);
                    self.speaker = Some(l.speaker);
                    self.listener = listener;
                    let text = l.text.replace("{leader}", &self.leader_name);
                    view.conv_line(self.name_of(Some(l.speaker)).as_deref(), &text, l.reveal_cps);
                    view.conv_passive(self.ambient || self.hands_free);
                    view.conv_cue(if self.ambient || self.hands_free { Some(false) } else { None });
                    if self.auto_camera && !self.ambient {
                        let mut cue = self.director.line(l.speaker, listener, kh_ui::talk::visible_len(&text));
                        if let Some(kind) = l.shot {
                            cue.kind = kind;
                        }
                        self.events.push(ConvEvent::Shot(cue));
                    }
                    self.events.push(ConvEvent::Line { index, speaker: l.speaker, listener, voice: l.voice.clone(), anim: l.anim.clone(), face: l.face.clone() });
                    self.lines_shown += 1;
                    let auto = if self.ambient { Some(l.auto_after.unwrap_or(AMBIENT_AUTO)) } else { l.auto_after };
                    self.run = Run::Line { index, voice_left: l.voice.as_ref().and_then(|v| v.seconds), auto, shown_for: 0.0 };
                }
                Step::Choice(c) if self.ambient => {
                    // (C.5) no choices in an ambient conversation: its cancel option (else the first) is taken at once
                    let index = self.pc;
                    let option = c.cancel.unwrap_or(0);
                    self.events.push(ConvEvent::Chosen { index, option });
                    self.pc = index + 1;
                    if !c.external {
                        match c.options.get(option).map(|o| o.branch.clone()) {
                            Some(Branch::Goto(l)) => match self.label(&l) {
                                Some(k) => self.pc = k,
                                None => {
                                    view.conv_close();
                                    self.finish(EndReason::Script);
                                    return;
                                }
                            },
                            Some(Branch::End) => {
                                view.conv_close();
                                self.finish(EndReason::Script);
                                return;
                            }
                            _ => {}
                        }
                    }
                }
                Step::Choice(c) => {
                    let index = self.pc;
                    if let Some(s) = c.speaker {
                        self.speaker = Some(s);
                    }
                    let options: Vec<String> = c.options.iter().map(|o| o.text.replace("{leader}", &self.leader_name)).collect();
                    // (CV3) a choice the leader makes shows no name line (the player does not name himself; its tail
                    // still points at him)
                    let chooser_named = c.speaker.and_then(|s| self.actor(s)).is_some_and(|a| a.role != Role::Leader);
                    let name = if chooser_named { self.name_of(c.speaker) } else { None };
                    view.conv_choice(name.as_deref(), &c.prompt.replace("{leader}", &self.leader_name), &options, c.start, c.cancel);
                    if self.hands_free {
                        view.conv_passive(true); // (CV3) shown, waiting for the fight's end
                    }
                    if self.auto_camera && !self.ambient {
                        if let Some(s) = c.speaker {
                            let chooser = self.actors.iter().find(|a| a.role == Role::Leader).map(|a| a.id);
                            self.events.push(ConvEvent::Shot(self.director.choice(s, chooser)));
                        }
                    }
                    self.events.push(ConvEvent::ChoiceUp { index });
                    self.run = Run::Choice { index };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_ui::bubble::BubbleEvent;

    /// A view that records what it was asked and hands out the events a test gives it.
    #[derive(Default)]
    struct Fake {
        shown: Vec<String>,
        pending: Vec<BubbleEvent>,
        closed: u32,
        all_shown: bool,
        passive: bool,
    }

    impl ConvView for Fake {
        fn conv_line(&mut self, name: Option<&str>, text: &str, _: Option<f32>) {
            self.shown.push(format!("{}: {text}", name.unwrap_or("-")));
            self.all_shown = false;
        }
        fn conv_choice(&mut self, _: Option<&str>, prompt: &str, options: &[String], _: usize, _: Option<usize>) {
            self.shown.push(format!("? {prompt} [{}]", options.join("|")));
        }
        fn conv_cue(&mut self, _: Option<bool>) {}
        fn conv_close(&mut self) {
            self.closed += 1;
        }
        fn conv_events(&mut self) -> Vec<BubbleEvent> {
            std::mem::take(&mut self.pending)
        }
        fn conv_line_shown(&self) -> bool {
            self.all_shown
        }
        fn conv_passive(&mut self, passive: bool) {
            self.passive = passive;
        }
    }

    fn actors() -> Vec<Actor> {
        vec![Actor { id: 0, name: "Sora".into(), role: Role::Leader }, Actor { id: 1, name: "Donald".into(), role: Role::Npc }, Actor { id: 2, name: "Goofy".into(), role: Role::Party }]
    }

    fn frame(c: &mut Conversation, v: &mut Fake, input: ConvInput) -> Vec<ConvEvent> {
        c.step(v, &input);
        c.take_events()
    }

    const DT: ConvInput = ConvInput { dt: 1.0 / 60.0, combat: false, voice_playing: None, apart: None };

    #[test]
    fn a_script_runs_line_by_line_with_a_and_ends() {
        let mut c = Conversation::new(actors(), vec![Step::Line(Line::new(1, "Aw, phooey!")), Step::Line(Line { listener: Some(0), ..Line::new(1, "{leader}, I'm comin' with ya!") })]);
        let mut v = Fake::default();
        let e = frame(&mut c, &mut v, DT);
        assert!(e.contains(&ConvEvent::Started));
        assert!(e.iter().any(|e| matches!(e, ConvEvent::Shot(_))), "every line gets a shot");
        assert_eq!(v.shown, vec!["Donald: Aw, phooey!"]);
        assert_eq!(c.speaker(), Some(1));
        // nothing goes on without A
        for _ in 0..300 {
            frame(&mut c, &mut v, DT);
        }
        assert_eq!(v.shown.len(), 1);
        v.pending.push(BubbleEvent::Advanced);
        let e = frame(&mut c, &mut v, DT);
        assert!(e.contains(&ConvEvent::LineDone { index: 0 }));
        assert_eq!(v.shown[1], "Donald: Sora, I'm comin' with ya!");
        v.pending.push(BubbleEvent::Advanced);
        let e = frame(&mut c, &mut v, DT);
        assert!(e.contains(&ConvEvent::Ended(EndReason::Script)));
        assert!(c.is_over() && v.closed == 1);
        assert!(!c.holds(0), "nobody is held after the end");
    }

    #[test]
    fn everyone_is_held_unless_a_directive_releases_him() {
        let mut c = Conversation::new(actors(), vec![
            Step::Direct { directive: Directive::Turn { actor: 1, face: Facing::Actor(0) }, wait: true },
            Step::Line(Line::new(1, "Hey!")),
            Step::Direct { directive: Directive::Release { actor: 2 }, wait: false },
            Step::Line(Line::new(1, "Goofy, go on.")),
            Step::Direct { directive: Directive::Hold { actor: 2 }, wait: false },
            Step::Line(Line::new(1, "Back.")),
        ]);
        let mut v = Fake::default();
        assert!(!c.holds(0), "(CV3) the leader is never held: the player keeps his character and camera");
        assert!(c.holds(1) && c.holds(2));
        assert!(!c.holds(9), "not a participant");
        let e = frame(&mut c, &mut v, DT);
        assert!(e.contains(&ConvEvent::Direct(Directive::Turn { actor: 1, face: Facing::Actor(0) })));
        assert!(matches!(c.directed(), Some(Directive::Turn { actor: 1, .. })), "being turned");
        assert!(v.shown.is_empty(), "waits for the turn");
        c.directive_done(1);
        frame(&mut c, &mut v, DT);
        assert_eq!(v.shown.len(), 1);
        v.pending.push(BubbleEvent::Advanced);
        frame(&mut c, &mut v, DT);
        assert!(!c.holds(2) && c.holds(1), "Goofy released");
        v.pending.push(BubbleEvent::Advanced);
        frame(&mut c, &mut v, DT);
        assert!(c.holds(2), "held again");
    }

    #[test]
    fn a_before_the_voice_ends_stops_it() {
        let mut c = Conversation::new(actors(), vec![Step::Line(Line { voice: Some(Voice { key: "v1".into(), seconds: Some(3.0) }), ..Line::new(1, "A long voice.") }), Step::Line(Line { voice: Some(Voice { key: "v2".into(), seconds: Some(0.5) }), ..Line::new(1, "Short.") }), Step::Line(Line::new(1, "End."))]);
        let mut v = Fake::default();
        let e = frame(&mut c, &mut v, DT);
        assert!(e.iter().any(|e| matches!(e, ConvEvent::Line { voice: Some(Voice { key, .. }), .. } if key == "v1")));
        frame(&mut c, &mut v, DT);
        v.pending.push(BubbleEvent::Advanced);
        let e = frame(&mut c, &mut v, DT);
        assert!(e.contains(&ConvEvent::StopVoice { speaker: 1 }), "{e:?}");
        // the second line's voice is over (the host says so): A does not stop anything
        for _ in 0..60 {
            frame(&mut c, &mut v, ConvInput { voice_playing: Some(false), ..DT });
        }
        v.pending.push(BubbleEvent::Advanced);
        let e = frame(&mut c, &mut v, ConvInput { voice_playing: Some(false), ..DT });
        assert!(!e.iter().any(|e| matches!(e, ConvEvent::StopVoice { .. })));
        assert_eq!(v.shown.len(), 3);
    }

    #[test]
    fn auto_lines_go_on_after_their_voice() {
        let mut c = Conversation::new(actors(), vec![Step::Line(Line { voice: Some(Voice { key: "v".into(), seconds: None }), auto_after: Some(0.5), ..Line::new(1, "Auto.") }), Step::Line(Line::new(1, "Next."))]);
        let mut v = Fake::default();
        frame(&mut c, &mut v, ConvInput { voice_playing: Some(true), ..DT });
        v.all_shown = true;
        for _ in 0..120 {
            frame(&mut c, &mut v, ConvInput { voice_playing: Some(true), ..DT });
        }
        assert_eq!(v.shown.len(), 1, "waits for the voice");
        for _ in 0..40 {
            frame(&mut c, &mut v, ConvInput { voice_playing: Some(false), ..DT });
        }
        assert_eq!(v.shown.len(), 2);
    }

    #[test]
    fn choices_branch_by_label_and_external_ones_go_to_the_host() {
        let script = vec![
            Step::Choice(Choice { speaker: Some(1), prompt: "Coming?".into(), options: vec![ChoiceOption { text: "Yes".into(), branch: Branch::Goto("yes".into()) }, ChoiceOption { text: "No".into(), branch: Branch::End }], start: 0, cancel: Some(1), external: false }),
            Step::Line(Line::new(1, "skipped")),
            Step::Label("yes".into()),
            Step::Line(Line::new(1, "Great!")),
        ];
        let mut c = Conversation::new(actors(), script.clone());
        let mut v = Fake::default();
        frame(&mut c, &mut v, DT);
        v.pending.push(BubbleEvent::Chosen(0));
        let e = frame(&mut c, &mut v, DT);
        assert!(e.contains(&ConvEvent::Chosen { index: 0, option: 0 }));
        assert_eq!(v.shown.last().unwrap(), "Donald: Great!");
        let mut c = Conversation::new(actors(), script);
        let mut v = Fake::default();
        frame(&mut c, &mut v, DT);
        v.pending.push(BubbleEvent::Chosen(1));
        let e = frame(&mut c, &mut v, DT);
        assert!(e.contains(&ConvEvent::Ended(EndReason::Script)));
        // an external choice in an open-ended conversation: the host pushes what follows
        let mut c = Conversation::open(actors());
        c.push(Step::Choice(Choice { speaker: Some(1), prompt: String::new(), options: vec![ChoiceOption { text: "Purchase".into(), branch: Branch::Next }, ChoiceOption { text: "Leave".into(), branch: Branch::Next }], start: 0, cancel: Some(1), external: true }));
        let mut v = Fake::default();
        frame(&mut c, &mut v, DT);
        v.pending.push(BubbleEvent::Chosen(1));
        let e = frame(&mut c, &mut v, DT);
        assert!(e.contains(&ConvEvent::Chosen { index: 0, option: 1 }));
        for _ in 0..30 {
            frame(&mut c, &mut v, DT);
        }
        assert!(!c.is_over(), "waits for the host");
        c.push(Step::Line(Line::new(1, "Farewell.")));
        frame(&mut c, &mut v, DT);
        assert_eq!(v.shown.last().unwrap(), "Donald: Farewell.");
        c.end(&mut v, "the game's talk ended");
        assert!(matches!(c.end_reason(), Some(EndReason::Host(_))));
    }

    /// (C.5) AN AMBIENT CONVERSATION: nobody held, no shot, no fight ends it, each line goes on by itself (after its
    /// voice / text + AMBIENT_AUTO), a choice is skipped (its cancel option).
    #[test]
    fn ambient_lines_hold_nobody_and_go_on_by_themselves() {
        let mut c = Conversation::new(actors(), vec![
            Step::Line(Line { voice: Some(Voice { key: "taunt".into(), seconds: None }), ..Line::new(1, "You there!") }),
            Step::Choice(Choice { speaker: Some(1), prompt: "?".into(), options: vec![ChoiceOption { text: "a".into(), branch: Branch::Next }, ChoiceOption { text: "b".into(), branch: Branch::Goto("b".into()) }], start: 0, cancel: Some(1), external: false }),
            Step::Line(Line::new(1, "skipped")),
            Step::Label("b".into()),
            Step::Line(Line { auto_after: Some(0.2), ..Line::new(1, "Begone.") }),
        ]);
        c.ambient = true;
        let mut v = Fake::default();
        let e = frame(&mut c, &mut v, ConvInput { combat: true, voice_playing: Some(true), ..DT });
        assert!(!e.iter().any(|e| matches!(e, ConvEvent::Shot(_) | ConvEvent::Ended(_))), "no shot, a fight does not end it: {e:?}");
        assert!(!c.holds(0) && !c.holds(1), "nobody is held");
        v.all_shown = true;
        for _ in 0..120 {
            frame(&mut c, &mut v, ConvInput { voice_playing: Some(true), ..DT });
        }
        assert_eq!(v.shown.len(), 1, "waits for the voice");
        let mut all = Vec::new();
        for _ in 0..(AMBIENT_AUTO * 60.0) as usize + 2 {
            all.extend(frame(&mut c, &mut v, ConvInput { voice_playing: Some(false), ..DT }));
        }
        assert!(all.contains(&ConvEvent::Chosen { index: 1, option: 1 }), "the choice is skipped by its cancel option");
        assert_eq!(v.shown.last().unwrap(), "Donald: Begone.");
        v.all_shown = true;
        let mut end = Vec::new();
        for _ in 0..30 {
            end.extend(frame(&mut c, &mut v, ConvInput { voice_playing: Some(false), ..DT }));
        }
        assert!(end.contains(&ConvEvent::Ended(EndReason::Script)));
    }

    #[test]
    fn a_fight_ends_or_suspends_it() {
        let mut c = Conversation::new(actors(), vec![Step::Line(Line::new(1, "One.")), Step::Line(Line::new(1, "Two."))]);
        c.policy = CombatPolicy::End; // (CV3: no longer the default, see a_fight_makes_it_hands_free)
        let mut v = Fake::default();
        frame(&mut c, &mut v, DT);
        let e = frame(&mut c, &mut v, ConvInput { combat: true, ..DT });
        assert!(e.contains(&ConvEvent::Ended(EndReason::Combat)));
        assert!(!c.holds(0));
        let mut c = Conversation::new(actors(), vec![Step::Line(Line::new(1, "One.")), Step::Line(Line::new(1, "Two."))]);
        c.policy = CombatPolicy::Suspend;
        let mut v = Fake::default();
        frame(&mut c, &mut v, DT);
        let e = frame(&mut c, &mut v, ConvInput { combat: true, ..DT });
        assert!(e.contains(&ConvEvent::Suspended) && c.suspended());
        for _ in 0..120 {
            frame(&mut c, &mut v, DT);
        }
        assert!(!c.suspended());
        assert_eq!(v.shown.last().unwrap(), "Donald: One.", "the line it was on comes back");
    }

    /// (CV3) THE A ROUTING: out of a fight A is the bubble's (`takes_a`), at a choice also the D-pad / B (`in_choice`);
    /// the leader is never held.
    #[test]
    fn out_of_a_fight_a_is_the_bubbles_and_the_leader_is_free() {
        let mut c = Conversation::new(actors(), vec![
            Step::Line(Line::new(1, "Phooey!")),
            Step::Choice(Choice { speaker: Some(1), prompt: "Come?".into(), options: vec![ChoiceOption { text: "Yes".into(), branch: Branch::Next }, ChoiceOption { text: "No".into(), branch: Branch::End }], start: 0, cancel: Some(1), external: false }),
            Step::Line(Line::new(1, "Great!")),
        ]);
        let mut v = Fake::default();
        frame(&mut c, &mut v, DT);
        assert!(c.takes_a() && !c.in_choice() && !c.holds(0) && c.holds(1));
        v.pending.push(BubbleEvent::Advanced);
        frame(&mut c, &mut v, DT);
        assert!(c.takes_a() && c.in_choice(), "a choice: A, the D-pad and B are the bubble's");
        v.pending.push(BubbleEvent::Chosen(0));
        frame(&mut c, &mut v, DT);
        assert!(c.takes_a() && !c.in_choice());
        assert_eq!(v.shown.last().unwrap(), "Donald: Great!");
        v.pending.push(BubbleEvent::Advanced);
        frame(&mut c, &mut v, DT);
        assert!(c.is_over() && !c.takes_a(), "over: A is the leader's again");
    }

    /// (CV3) IN A FIGHT (HandsFree, the default): it goes on, the bubble passive (A attacks), A's events ignored, each
    /// line goes on by itself after its voice + combat_auto; a choice waits shown until the fight is over.
    #[test]
    fn a_fight_makes_it_hands_free() {
        let mut c = Conversation::new(actors(), vec![
            Step::Line(Line { voice: Some(Voice { key: "v".into(), seconds: None }), ..Line::new(1, "Watch out!") }),
            Step::Choice(Choice { speaker: Some(1), prompt: "Run?".into(), options: vec![ChoiceOption { text: "Yes".into(), branch: Branch::Next }, ChoiceOption { text: "No".into(), branch: Branch::Next }], start: 0, cancel: Some(1), external: false }),
            Step::Line(Line::new(1, "Done.")),
        ]);
        assert_eq!(c.policy, CombatPolicy::HandsFree);
        let mut v = Fake::default();
        frame(&mut c, &mut v, DT);
        let fight = ConvInput { combat: true, ..DT };
        let e = frame(&mut c, &mut v, ConvInput { voice_playing: Some(true), ..fight });
        assert!(!e.iter().any(|e| matches!(e, ConvEvent::Ended(_))), "a fight does not end it");
        assert!(c.hands_free() && v.passive && !c.takes_a(), "A is the leader's attack in a fight");
        v.all_shown = true;
        v.pending.push(BubbleEvent::Advanced);
        let e = frame(&mut c, &mut v, ConvInput { voice_playing: Some(true), ..fight });
        assert!(!e.iter().any(|e| matches!(e, ConvEvent::LineDone { .. } | ConvEvent::StopVoice { .. })), "A is not the bubble's: {e:?}");
        let mut all = Vec::new();
        for _ in 0..(COMBAT_AUTO * 60.0) as usize + 3 {
            all.extend(frame(&mut c, &mut v, ConvInput { voice_playing: Some(false), ..fight }));
        }
        assert!(all.contains(&ConvEvent::LineDone { index: 0 }), "the line went on by itself: {all:?}");
        assert!(all.contains(&ConvEvent::ChoiceUp { index: 1 }));
        assert!(v.passive, "the choice is shown passive in the fight");
        v.pending.push(BubbleEvent::Chosen(0));
        for _ in 0..120 {
            frame(&mut c, &mut v, fight);
        }
        assert!(!c.in_choice() && v.shown.last().unwrap().starts_with('?'), "the choice waits for the fight's end");
        let e = frame(&mut c, &mut v, DT);
        assert!(!c.hands_free() && !v.passive && c.in_choice(), "the fight is over: the choice takes keys again ({e:?})");
        v.pending.push(BubbleEvent::Chosen(1));
        let e = frame(&mut c, &mut v, DT);
        assert!(e.contains(&ConvEvent::Chosen { index: 1, option: 1 }));
        assert_eq!(v.shown.last().unwrap(), "Donald: Done.");
    }

    /// (CV3) WALKING AWAY: farther than walk_away from the others ends a held conversation gracefully (the bubble pops
    /// out); an ambient one is never ended by it; walk_away None = never.
    #[test]
    fn walking_away_ends_it_gracefully() {
        let script = vec![Step::Line(Line::new(1, "Hey, wait!")), Step::Line(Line::new(1, "Two."))];
        let mut c = Conversation::new(actors(), script.clone());
        let mut v = Fake::default();
        frame(&mut c, &mut v, ConvInput { apart: Some(1.5), ..DT });
        for d in [2.0, 3.0, 4.0, WALK_AWAY] {
            let e = frame(&mut c, &mut v, ConvInput { apart: Some(d), ..DT });
            assert!(!e.iter().any(|e| matches!(e, ConvEvent::Ended(_))), "{d} m: still on");
        }
        let e = frame(&mut c, &mut v, ConvInput { apart: Some(WALK_AWAY + 0.1), ..DT });
        assert!(e.contains(&ConvEvent::Ended(EndReason::WalkedAway)) && v.closed == 1 && c.is_over());
        let mut c = Conversation::new(actors(), script.clone());
        c.ambient = true;
        let e = frame(&mut c, &mut v, ConvInput { apart: Some(50.0), ..DT });
        assert!(!e.iter().any(|e| matches!(e, ConvEvent::Ended(_))), "ambient lines are not ended by distance");
        let mut c = Conversation::new(actors(), script);
        c.walk_away = None;
        let e = frame(&mut c, &mut v, ConvInput { apart: Some(50.0), ..DT });
        assert!(!e.iter().any(|e| matches!(e, ConvEvent::Ended(_))));
    }

    /// (CV3) A choice the leader makes (the Elden Ring talk adapter: "asked by the leader") shows no name line, now
    /// that the leader actor carries the leader's name; an NPC's question keeps his.
    #[test]
    fn a_leaders_choice_shows_no_name_line() {
        let opts = vec![ChoiceOption { text: "Accept".into(), branch: Branch::Next }, ChoiceOption { text: "Refuse".into(), branch: Branch::Next }];
        let mut c = Conversation::open(actors());
        c.push(Step::Choice(Choice { speaker: Some(0), prompt: String::new(), options: opts.clone(), start: 0, cancel: Some(1), external: true }));
        let mut v = Names::default();
        c.step(&mut v, &DT);
        assert_eq!(v.0, vec![None]);
        let mut c = Conversation::open(actors());
        c.push(Step::Choice(Choice { speaker: Some(1), prompt: "Coming?".into(), options: opts, start: 0, cancel: Some(1), external: true }));
        c.step(&mut v, &DT);
        assert_eq!(v.0, vec![None, Some("Donald".to_string())]);
    }

    #[derive(Default)]
    struct Names(Vec<Option<String>>);

    impl ConvView for Names {
        fn conv_line(&mut self, _: Option<&str>, _: &str, _: Option<f32>) {}
        fn conv_choice(&mut self, name: Option<&str>, _: &str, _: &[String], _: usize, _: Option<usize>) {
            self.0.push(name.map(str::to_string));
        }
        fn conv_cue(&mut self, _: Option<bool>) {}
        fn conv_close(&mut self) {}
        fn conv_events(&mut self) -> Vec<BubbleEvent> {
            Vec::new()
        }
        fn conv_line_shown(&self) -> bool {
            true
        }
    }
}
