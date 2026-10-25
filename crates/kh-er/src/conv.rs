//! (CV1) THE CONVERSATION SYSTEM IN ELDEN RING: one conversation at a time, run by kh_host::conv's runner against the
//! HUD's speech bubble (kh_ui::bubble), for any owner: S8's Donald at the Finger Reader (unlock/host.rs) and CV2's
//! adapter of Elden Ring's own real-time NPC talk. API: docs/party-contracts.md, section "Conversation" (C.4). State
//! note: build/sora_tmp/er/IMPL_CV1_STATE.md. EVERYTHING HERE IS UNTESTED IN THE GAME (agents never launch it); the
//! runner, the bubble and the camera are proven offline.
//!
//! WHAT IT DOES EACH FRAME (sora.rs, after the HUD's frame): finds every participant (the leader, a party member, an
//! Elden Ring character by handle / entity id, or a pose its owner places), projects their faces onto the HUD's stage
//! (the bubble's tail follows the speaker's, the bubble keeps off the others'), steps the runner (A / B / D-pad come
//! from the HUD's frame), hands its events to the owner (`take_events`), the camera's shots to KH3's camera (cam.rs:
//! `camera_scene`), and HOLDS: the leader's pad (`note_pad`: sticks and buttons zeroed after the HUD saw them) and the
//! party's AI (`hold_party`: stopped, standing, looking at the speaker), until the end.
//!
//! KEYS (`kh-er.ini`; (INI) once `kh-er.ctl`): `conv.on` (1; 0 = no conversation can begin, a running one ends), `conv.camera` (1; 0 = no
//! conversation camera), `conv.log` (0; 1 = the participants' places once a second).
//!
//! (CV3, THE USER 2026-10-17: "remove the cinematic camera from talking and allow the player to move the camera and their
//! character, though A is reserved for moving dialogue along while in conversation unless in combat"; supersedes the
//! leader's lock and the camera above): THE PLAYER IS FREE in a conversation: his stick, his camera (the right stick)
//! and every button stay his, except A while the bubble takes it (out of a fight: A advances, it does nothing else) and,
//! while a choice is up, the D-pad (the glove) and B (the cancel option). In a fight the conversation goes on HANDS-FREE
//! (A attacks; lines go on by themselves; a choice waits shown until the fight is over). Walking away (farther than
//! `conv.walk_away` m from the others) ends it gracefully. The party is held (standing, looking at the speaker) only
//! while the leader stays within `conv.hold_near` m of the others; farther they follow him again. No conversation
//! camera by default. TALK PRIORITY (`offer_talk`): a party member's talk prompt (Donald at the crone, any later one)
//! wins Y over Elden Ring's own action prompt (sora.rs asks `er_prompt_may_take_y`).
//! CV3 KEYS: `conv.free` (1; 0 = CV1's full lock of the leader), `conv.camera` (NOW 0 by default: 1 = CV1's shots),
//! `conv.walk_away` (5.0 m; 0 = never), `conv.combat_auto` (1.0 s), `conv.hold_near` (3.5 m), `conv.b_cancel` (1; 0 = B
//! stays the jump at a choice too).

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Mutex;

use eldenring::cs::{ChrIns, ChrSet, FieldInsHandle, WorldChrMan};
use fromsoftware_shared::FromStatic;
use glam::{Vec2, Vec3};
use kh_host::conv::{ConvInput, Conversation};
pub use kh_host::conv::{Actor, ActorId, Branch, Choice, ChoiceOption, CombatPolicy, ConvEvent, Directive, EndReason, Facing, Line, Role, ShotCue, ShotKind, Step, Voice};
pub use kh_host::party::PartyChara;
use kh_ui::bubble::Face;

use crate::control::Control;
use crate::coords::{self, Frame};
use crate::erlog;
use crate::game::{self, plausible};
use crate::word;

pub type ConvId = u32;

/// Where a participant is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Body {
    /// The party's leader (Sora, or the friend leading).
    Leader,
    /// A party member (the party runtime's).
    Member(PartyChara),
    /// An Elden Ring character by its handle.
    Chr(FieldInsHandle),
    /// An Elden Ring character by its event entity id.
    Entity(u32),
    /// The owner gives the pose every frame ([`place`]).
    Placed,
}

#[derive(Clone, Debug)]
pub struct Participant {
    pub actor: Actor,
    pub body: Body,
    /// The height (cm) his face is found at (feet + 0.93 of it); None: the leader / members their own, others 170.
    pub height_cm: Option<f32>,
}

/// A pose in KH3 space (cm, degrees).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub feet_kh: Vec3,
    pub yaw: f32,
    pub height_cm: f32,
}

impl Pose {
    pub fn head(&self) -> Vec3 {
        self.feet_kh + Vec3::Z * self.height_cm * 0.93
    }
}

pub struct Begin {
    /// Who runs it (logs, `running`).
    pub owner: &'static str,
    pub participants: Vec<Participant>,
    pub steps: Vec<Step>,
    /// Steps pushed while it runs; out of steps it waits until the owner ends it.
    pub open_ended: bool,
    pub policy: CombatPolicy,
    /// The conversation camera (also needs `conv.camera`).
    pub camera: bool,
    /// The party's AI held for the whole conversation (stopped, standing, looking at the speaker).
    pub hold_party: bool,
    /// (C.5) The leader held (his pad the bubble's). FALSE = AN AMBIENT CONVERSATION (proximity lines, taunts, battle
    /// lines): nobody held, the pad stays the player's, no camera, no A (every line goes on by itself), no fight ends it.
    pub hold_leader: bool,
}

impl Begin {
    /// A script with the defaults: (CV3) hands-free in a fight, no camera of its own (`conv.camera` = 1 and `camera`
    /// give CV1's shots), the party held while the leader stays near.
    pub fn new(owner: &'static str, participants: Vec<Participant>, steps: Vec<Step>) -> Begin {
        Begin { owner, participants, steps, open_ended: false, policy: CombatPolicy::HandsFree, camera: true, hold_party: true, hold_leader: true }
    }

    /// (C.5) AN AMBIENT CONVERSATION: nobody held, no camera, lines going on by themselves.
    pub fn ambient(owner: &'static str, participants: Vec<Participant>, steps: Vec<Step>) -> Begin {
        Begin { owner, participants, steps, open_ended: false, policy: CombatPolicy::End, camera: false, hold_party: false, hold_leader: false }
    }

    pub fn is_ambient(&self) -> bool {
        !self.hold_leader
    }
}

/// What sora.rs gives each frame.
pub struct Scene {
    pub frame: Frame,
    pub dt: f32,
    pub leader: Pose,
    pub leader_name: String,
    /// The party's members other than the leader (KH3 poses).
    pub members: Vec<(PartyChara, Pose)>,
    /// A fight is on (KH3's battle mode, a member in battle).
    pub combat: bool,
    /// A menu / cutscene of the game has the screen: the conversation waits.
    pub hidden: bool,
    /// (INT) The leader's and the members' feet as DRAWN this frame (between the steps): the bubbles' faces are there
    /// (the poses above stay the steps': the conversation's camera reads them). None / missing = the pose's.
    pub drawn_leader: Option<Vec3>,
    pub drawn_members: Vec<(PartyChara, Vec3)>,
}

/// What KH3's camera reads every step (cam.rs).
#[derive(Clone, Debug, Default)]
pub struct CamScene {
    pub running: bool,
    pub subjects: Vec<kh_camera::conv::Subject>,
    /// The new shots since the last read.
    pub cues: Vec<ShotCue>,
}

struct Running {
    id: ConvId,
    owner: &'static str,
    conv: Conversation,
    parts: Vec<Participant>,
    placed: Vec<(ActorId, Pose)>,
    poses: Vec<(ActorId, Pose)>,
    /// Elden Ring characters found: (actor, pointer).
    chr: Vec<(ActorId, usize)>,
    look_in: u32,
    outbox: Vec<ConvEvent>,
    camera: bool,
    hold_party: bool,
    /// (C.5) nobody held, no camera, no A.
    ambient: bool,
    voice: Option<bool>,
    sec: f32,
}

#[derive(Default)]
struct State {
    running: Option<Running>,
    /// The ended conversations' last events for their owners (the newest 16).
    ended: Vec<(ConvId, Vec<ConvEvent>)>,
    /// (C.5) Ambient conversations waiting for the one running (an ambient one): (id, begin, seconds waited).
    queue: Vec<(ConvId, Begin, f32)>,
    next: ConvId,
    on: bool,
    camera_on: bool,
    log: bool,
    cam: CamScene,
    /// (CV3) the keys: `conv.free`, `conv.walk_away`, `conv.combat_auto`, `conv.hold_near`, `conv.b_cancel`.
    keys: FreeKeys,
}

/// (CV3) The free conversation's keys (defaults: the user's 2026-10-17 rule).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FreeKeys {
    pub free: bool,
    pub walk_away: f32,
    pub combat_auto: f32,
    pub hold_near: f32,
    pub b_cancel: bool,
    /// Seconds A stays the bubble's after a held conversation ended by its script or its owner (a mashed A must not
    /// swing at the NPC: seen in Elden Ring 2026-10-17, the crone hit by the A after Donald's last line).
    pub a_grace: f32,
}

impl Default for FreeKeys {
    fn default() -> FreeKeys {
        FreeKeys { free: true, walk_away: kh_host::conv::WALK_AWAY, combat_auto: kh_host::conv::COMBAT_AUTO, hold_near: 3.5, b_cancel: true, a_grace: 0.5 }
    }
}

impl FreeKeys {
    pub fn read(control: &Control) -> FreeKeys {
        let d = FreeKeys::default();
        let f = |k: &str, v: f32| control.f32(k).filter(|x| x.is_finite() && *x >= 0.0).unwrap_or(v);
        FreeKeys { free: control.flag("conv.free") != Some(false), walk_away: f("conv.walk_away", d.walk_away), combat_auto: f("conv.combat_auto", d.combat_auto), hold_near: f("conv.hold_near", d.hold_near), b_cancel: control.flag("conv.b_cancel") != Some(false), a_grace: f("conv.a_grace", d.a_grace) }
    }
}

// ------------------------------------------------------------------------------------------- (CV3) the pure rules

/// (CV3) WHAT A CONVERSATION TAKES OF THE LEADER'S PAD this frame (after the HUD saw it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadRule {
    /// Nothing (no held conversation, an ambient one, hands-free in a fight: A attacks).
    Free,
    /// A only (out of a fight: A advances the dialogue and does nothing else).
    A,
    /// A choice is up: A, the D-pad (the glove) and, with `b`, B (its cancel option). The stick stays his movement.
    Choice { b: bool },
    /// `conv.free = 0`: CV1's lock (sticks and buttons zeroed).
    All,
}

impl PadRule {
    fn code(self) -> u8 {
        match self {
            PadRule::Free => 0,
            PadRule::A => 1,
            PadRule::Choice { b: false } => 2,
            PadRule::Choice { b: true } => 3,
            PadRule::All => 4,
        }
    }
    fn of(code: u8) -> PadRule {
        match code {
            1 => PadRule::A,
            2 => PadRule::Choice { b: false },
            3 => PadRule::Choice { b: true },
            4 => PadRule::All,
            _ => PadRule::Free,
        }
    }
}

/// (CV3) The rule from the running conversation's state.
pub fn pad_rule(keys: &FreeKeys, held_running: bool, takes_a: bool, in_choice: bool) -> PadRule {
    if !held_running {
        PadRule::Free
    } else if !keys.free {
        PadRule::All
    } else if in_choice {
        PadRule::Choice { b: keys.b_cancel }
    } else if takes_a {
        PadRule::A
    } else {
        PadRule::Free
    }
}

/// (CV3) The rule applied to the leader's pad (the HUD saw it first: the bubble had its keys).
pub fn apply_pad(rule: PadRule, stick: &mut Vec2, right_stick: &mut Vec2, held: &mut kh_core::PadButtons) {
    use kh_core::PadButton as B;
    match rule {
        PadRule::Free => {}
        PadRule::A => held.set(B::FaceBottom, false),
        PadRule::Choice { b } => {
            for k in [B::FaceBottom, B::DPadUp, B::DPadDown, B::DPadLeft, B::DPadRight] {
                held.set(k, false);
            }
            if b {
                held.set(B::FaceRight, false);
            }
        }
        PadRule::All => {
            *stick = Vec2::ZERO;
            *right_stick = Vec2::ZERO;
            *held = kh_core::PadButtons::NONE;
        }
    }
}

/// (CV3) Metres (horizontal) from the leader's feet to the nearest other participant's (None: no leader / nobody else).
pub fn apart_m(leader: Option<Vec3>, others: &[Vec3]) -> Option<f32> {
    let l = leader?;
    others.iter().map(|o| (*o - l).truncate().length() / coords::CM_PER_M).fold(None, |m: Option<f32>, d| Some(m.map_or(d, |m| m.min(d))))
}

/// (CV3) THE LEADER'S FIGHT, whoever leads: Sora's battle flag counts only while he is in the active party (out of it
/// he is not stepped and his flag is frozen: the cause of "I cannot talk to Donald" in the user's session of
/// 2026-10-16, Goofy leading after Sora's staged Margit fight: every conversation ended at once "by combat" and
/// Donald's prompt was never offered); the party's members' battle state always counts.
pub fn leader_combat(sora_in_party: bool, sora_battle: bool, members_battle: bool) -> bool {
    (sora_in_party && sora_battle) || members_battle
}

/// (CV3 round 4) WHAT THE SAVE POINT READS OF THE LEADER, whoever leads: on the floor, in a fight, alive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeaderFacts {
    pub on_floor: bool,
    pub battle: bool,
    pub alive: bool,
}

/// (CV3 round 4) THE LEADER'S FACTS FOR THE SAVE POINT (the user's FINAL5 session 2026-10-17: "Unlocking Donald seemed
/// to lock me out of using save points"). Sora out of the active party is not stepped: after his staged Margit fight
/// his output stayed frozen at Attack, off the floor, so the save point read "he is in the air" and never offered Save
/// to Goofy leading. The leader's own view counts (`leader_on_floor`, `leader_ko`: kh-partyhost's LeaderView, which is
/// Sora's output when he leads); the fight is [`leader_combat`]. Pure.
pub fn leader_facts(sora_in_party: bool, sora_battle: bool, members_battle: bool, leader_on_floor: bool, leader_ko: bool) -> LeaderFacts {
    LeaderFacts { on_floor: leader_on_floor, battle: leader_combat(sora_in_party, sora_battle, members_battle), alive: !leader_ko }
}

/// (CV3 round 4, sora.rs) [`leader_facts`] from the party runtime (None: Sora alone, his own output).
#[cfg(feature = "party")]
pub fn leader_facts_of(core: Option<&crate::party::glue::PartyCore>, out: &kh_core::Output, sora: &kh_core::Sora) -> LeaderFacts {
    let Some(core) = core else { return LeaderFacts { on_floor: out.on_floor, battle: sora.actions().battle, alive: sora.status().hp > 0 } };
    let lv = core.rt.leader_view_of(out, sora.status());
    leader_facts(!core.sora_skipped(), sora.actions().battle, core.rt.current.iter().any(|v| v.in_battle), lv.on_floor, lv.ko)
}

/// (CV3) TALK PRIORITY: Elden Ring's own action prompt gets Y only while no party member's talk is offered and no held
/// conversation of a party owner runs (Y would start the NPC's talk under it).
pub fn er_prompt_wins(er_prompt_up: bool, party_talk_offered: bool, party_conv_running: bool) -> bool {
    er_prompt_up && !party_talk_offered && !party_conv_running
}

/// (CV3) A GRACE after the end: a held conversation that ended by its script or its owner keeps A from the leader for
/// `a_grace` seconds (the A that mashed through the last line); walking away, a fight or an ambient one: none.
pub fn a_grace_after(keys: &FreeKeys, ambient: bool, why: Option<&EndReason>) -> f32 {
    match why {
        _ if ambient || !keys.free => 0.0,
        Some(EndReason::Script | EndReason::Host(_)) => keys.a_grace,
        _ => 0.0,
    }
}

/// (CV3) The party is held (standing, facing the speaker) while the leader stays near the others.
pub fn party_held(keys: &FreeKeys, hold_party: bool, apart: Option<f32>) -> bool {
    hold_party && (!keys.free || apart.is_none_or(|a| a <= keys.hold_near))
}

static PAD_RULE: AtomicU8 = AtomicU8::new(0);
/// (CV3) A stays the bubble's until then after a held conversation ended ([`a_grace_after`]).
static A_GRACE: Mutex<Option<std::time::Instant>> = Mutex::new(None);
/// The owners whose party-talk prompt is up now ([`offer_talk`]).
static OFFERS: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

/// (CV3) A PARTY MEMBER'S TALK PROMPT is up (`up`) / gone for this owner: while any is up it wins Y over Elden Ring's
/// own action prompt.
pub fn offer_talk(owner: &'static str, up: bool) {
    let mut o = OFFERS.lock().unwrap_or_else(|e| e.into_inner());
    o.retain(|x| *x != owner);
    if up {
        o.push(owner);
    }
}

/// (RC) How many party members' talk prompts are up (each one a KH3 "Talk" reaction command on the HUD, erprompt.rs).
pub fn party_talks_offered() -> usize {
    OFFERS.lock().unwrap_or_else(|e| e.into_inner()).len()
}

/// (CV3) Some party member's talk prompt is up.
pub fn party_talk_offered() -> bool {
    !OFFERS.lock().unwrap_or_else(|e| e.into_inner()).is_empty()
}

/// (CV3, sora.rs's Y routing) Elden Ring's action prompt (up: `er_prompt_up`) may take Y: not while a party member's
/// talk is offered or a held conversation of a party owner (not Elden Ring's own talk) runs.
pub fn er_prompt_may_take_y(er_prompt_up: bool) -> bool {
    let party_conv = STATE.try_lock().ok().and_then(|g| g.as_ref().and_then(|s| s.running.as_ref().map(|r| !r.ambient && r.owner != "talk"))).unwrap_or(false);
    er_prompt_wins(er_prompt_up, party_talk_offered(), party_conv)
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
static HOLD_PAD: AtomicBool = AtomicBool::new(false);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let s = guard.get_or_insert_with(|| State { on: true, camera_on: true, ..State::default() });
    f(s)
}

// ------------------------------------------------------------------------------------------------- the API (C.4)

/// A conversation begins. Refused with `conv.on = 0`, without the HUD's bubble, without a participant.
/// (C.5) PRIORITY: a HELD conversation wins. A held begin ends a running ambient one (and drops the ambient queue); an
/// ambient begin while a held one runs is REFUSED (dropped: the owner shows nothing); an ambient begin while an ambient
/// one runs is QUEUED (at most [`QUEUE`]; the oldest dropped; one that waited [`QUEUE_WAIT`] s is dropped) and starts
/// when it ends; a held begin while a held one runs is refused.
pub fn begin(b: Begin) -> Result<ConvId, String> {
    let bubble = crate::hud::with_hud(|hud| hud.bubble_available()).unwrap_or(false);
    if !b.is_ambient() {
        let displace = with(|s| s.running.as_ref().filter(|r| r.ambient).map(|r| r.id));
        if let Some(id) = displace {
            end(id, "a held conversation began");
        }
        with(|s| {
            let dropped: Vec<(ConvId, Begin, f32)> = s.queue.drain(..).collect();
            for (id, b, _) in dropped {
                erlog!("conv: #{id} ({}) dropped from the queue: a held conversation began", b.owner);
                keep_ended(s, id, vec![ConvEvent::Ended(EndReason::Host("dropped: a held conversation began".into()))]);
            }
        });
    }
    with(|s| {
        if !s.on {
            return Err("conv.on = 0".to_string());
        }
        if !bubble {
            return Err("the HUD has no speech bubble (the UI pack lacks its art, or the HUD is off)".to_string());
        }
        if b.participants.is_empty() {
            return Err("no participant".to_string());
        }
        if let Some((rid, rowner, ramb)) = s.running.as_ref().map(|r| (r.id, r.owner, r.ambient)) {
            if b.is_ambient() && ramb {
                s.next += 1;
                let id = s.next;
                if s.queue.len() >= QUEUE {
                    let (old, ob, _) = s.queue.remove(0);
                    erlog!("conv: #{old} ({}) dropped from the queue (full)", ob.owner);
                    keep_ended(s, old, vec![ConvEvent::Ended(EndReason::Host("dropped: the queue was full".into()))]);
                }
                erlog!("conv: #{id} ({}) QUEUED behind ambient #{rid}", b.owner);
                s.queue.push((id, b, 0.0));
                return Ok(id);
            }
            return Err(format!("{} conversation runs ({rowner} #{rid}){}", if ramb { "an ambient" } else { "a held" }, if b.is_ambient() { ": ambient lines are dropped while a held conversation runs" } else { "" }));
        }
        s.next += 1;
        let id = s.next;
        s.running = Some(start(id, b));
        Ok(id)
    })
}

/// (C.5) Ambient conversations queued at most, and the seconds one may wait.
pub const QUEUE: usize = 3;
pub const QUEUE_WAIT: f32 = 6.0;

fn start(id: ConvId, b: Begin) -> Running {
    let actors: Vec<Actor> = b.participants.iter().map(|p| p.actor.clone()).collect();
    let ambient = b.is_ambient();
    let mut conv = Conversation::new(actors, b.steps);
    conv.open_ended = b.open_ended;
    conv.policy = b.policy;
    conv.ambient = ambient;
    erlog!("conv: #{id} BEGINS ({}): {} | {}{}", b.owner, b.participants.iter().map(|p| format!("{} {:?} = {:?}", p.actor.name, p.actor.role, p.body)).collect::<Vec<_>>().join(", "), if b.open_ended { "open-ended" } else { "a script" }, if ambient { ", AMBIENT (nobody held, no camera, no A)" } else { ", the player free (CV3: A advances out of a fight; no camera unless conv.camera = 1)" });
    Running { id, owner: b.owner, conv, parts: b.participants, placed: Vec::new(), poses: Vec::new(), chr: Vec::new(), look_in: 0, outbox: Vec::new(), camera: b.camera && !ambient, hold_party: b.hold_party && !ambient, ambient, voice: None, sec: 0.0 }
}

fn keep_ended(s: &mut State, id: ConvId, events: Vec<ConvEvent>) {
    s.ended.push((id, events));
    if s.ended.len() > 16 {
        s.ended.remove(0);
    }
}

/// A step more (an open-ended conversation's next line / choice).
pub fn push(id: ConvId, step: Step) -> bool {
    with(|s| match s.running.as_mut().filter(|r| r.id == id) {
        Some(r) => {
            r.conv.push(step);
            true
        }
        None => match s.queue.iter_mut().find(|(q, _, _)| *q == id) {
            Some((_, b, _)) => {
                b.steps.push(step);
                true
            }
            None => false,
        },
    })
}

/// The owner's copy of every event since the last call (the last ones also after the end).
pub fn take_events(id: ConvId) -> Vec<ConvEvent> {
    with(|s| {
        if let Some(r) = s.running.as_mut().filter(|r| r.id == id) {
            return std::mem::take(&mut r.outbox);
        }
        match s.ended.iter().position(|(e, _)| *e == id) {
            Some(k) => s.ended.remove(k).1,
            None => Vec::new(),
        }
    })
}

/// The owner performed the directive of `actor` (a `Direct { wait: true }`).
pub fn directive_done(id: ConvId, actor: ActorId) {
    with(|s| {
        if let Some(r) = s.running.as_mut().filter(|r| r.id == id) {
            r.conv.directive_done(actor);
        }
    });
}

/// The current line's voice plays (Some(true)), ended (Some(false)), not known (None: its `Voice::seconds`).
pub fn voice_playing(id: ConvId, playing: Option<bool>) {
    with(|s| {
        if let Some(r) = s.running.as_mut().filter(|r| r.id == id) {
            r.voice = playing;
        }
    });
}

/// A `Body::Placed` participant's pose this frame.
pub fn place(id: ConvId, actor: ActorId, pose: Pose) {
    with(|s| {
        if let Some(r) = s.running.as_mut().filter(|r| r.id == id) {
            r.placed.retain(|(a, _)| *a != actor);
            r.placed.push((actor, pose));
        }
    });
}

/// The owner ends it (the bubble closes, every hold is let go).
pub fn end(id: ConvId, why: &str) {
    let running = with(|s| {
        if let Some(k) = s.queue.iter().position(|(q, _, _)| *q == id) {
            s.queue.remove(k);
            keep_ended(s, id, vec![ConvEvent::Ended(EndReason::Host(why.to_string()))]);
        }
        s.running.as_ref().is_some_and(|r| r.id == id)
    });
    if !running {
        return;
    }
    let mut ended: Option<Running> = with(|s| s.running.take());
    if let Some(r) = ended.as_mut() {
        let mut view = Closer;
        let _ = crate::hud::with_hud(|hud| r.conv.end(hud, why)).or_else(|| {
            r.conv.end(&mut view, why);
            Some(())
        });
        finish(r);
        with(|s| keep_ended(s, r.id, std::mem::take(&mut r.outbox)));
    }
}

/// The conversation running now and its owner.
pub fn running() -> Option<(ConvId, &'static str)> {
    with(|s| s.running.as_ref().map(|r| (r.id, r.owner)))
}

/// (CV3, sora.rs once a frame) A driven story rest holds the pad (savepoint.rs: Melina's talks at a grace).
pub fn set_story_rest(on: bool) {
    STORY_REST.store(on, Ordering::Relaxed);
}

/// (CV3) The bubble gets the pad's keys although a driven story rest holds the pad: its talk is a conversation of ours
/// that takes A (the forced "menu" of the story rest would otherwise keep every key from it: her Accept / Refuse never
/// answerable, SV's run2 262 s).
pub fn story_bubble_keys() -> bool {
    STORY_REST.load(Ordering::Relaxed) && HOLD_PAD.load(Ordering::Relaxed)
}

static STORY_REST: AtomicBool = AtomicBool::new(false);

/// (CV3) A driven story rest holds the pad and a conversation of ours (held or ambient: her lines) runs: the HUD and its
/// bubble are shown (the story rest's forced "menu" covered the HUD: SV's run2, her talk with no bubble).
pub fn story_bubble_shown() -> bool {
    STORY_REST.load(Ordering::Relaxed) && STATE.try_lock().ok().is_some_and(|g| g.as_ref().is_some_and(|s| s.running.is_some()))
}

/// (CV3) This conversation runs and is a held one (not ambient lines).
pub fn is_held(id: ConvId) -> bool {
    with(|s| s.running.as_ref().is_some_and(|r| r.id == id && !r.ambient))
}

/// THE NPC LOCK (CV2): this Elden Ring character is in the running conversation and held.
pub fn holds_chr(handle: FieldInsHandle) -> bool {
    with(|s| s.running.as_ref().is_some_and(|r| r.parts.iter().any(|p| p.body == Body::Chr(handle) && r.conv.holds(p.actor.id))))
}

/// ... by event entity id.
pub fn holds_entity(entity: u32) -> bool {
    with(|s| s.running.as_ref().is_some_and(|r| r.parts.iter().any(|p| p.body == Body::Entity(entity) && r.conv.holds(p.actor.id))))
}

/// A participant is held (any body).
pub fn holds(actor: ActorId) -> bool {
    with(|s| s.running.as_ref().is_some_and(|r| r.conv.holds(actor)))
}

/// The speaker now (the owner's actor id).
pub fn speaker() -> Option<ActorId> {
    with(|s| s.running.as_ref().and_then(|r| r.conv.speaker()))
}

/// (hud.rs `HudHost::input`, after the HUD saw the pad) (CV3) WHAT THE CONVERSATION TAKES OF THE LEADER'S PAD
/// ([`PadRule`]): out of a fight A (the bubble's: it advances the dialogue and does nothing else), at a choice also the
/// D-pad and B; his sticks (movement, camera) and the other buttons stay his. `conv.free = 0`: CV1's lock (nothing).
pub fn note_pad(stick: &mut Vec2, right_stick: &mut Vec2, held: &mut kh_core::PadButtons) {
    let grace = A_GRACE.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|until| std::time::Instant::now() < until);
    let rule = if HOLD_PAD.load(Ordering::Relaxed) { PadRule::of(PAD_RULE.load(Ordering::Relaxed)) } else if grace { PadRule::A } else { PadRule::Free };
    apply_pad(rule, stick, right_stick, held);
}

/// A conversation takes something of the pad (A at least).
pub fn holds_pad() -> bool {
    HOLD_PAD.load(Ordering::Relaxed)
}

/// (cam.rs, every step) The camera's part: the subjects of this frame and the new shots (None: no conversation camera).
pub fn camera_scene() -> Option<CamScene> {
    let mut guard = STATE.try_lock().ok()?;
    let s = guard.as_mut()?;
    if !s.cam.running && s.cam.cues.is_empty() && s.cam.subjects.is_empty() {
        return None;
    }
    let scene = CamScene { running: s.cam.running, subjects: s.cam.subjects.clone(), cues: std::mem::take(&mut s.cam.cues) };
    if !s.cam.running {
        s.cam.subjects.clear();
    }
    Some(scene)
}

// ------------------------------------------------------------------------------------------------- the frame

/// A view that only closes (the HUD gone).
struct Closer;

impl kh_host::conv::ConvView for Closer {
    fn conv_line(&mut self, _: Option<&str>, _: &str, _: Option<f32>) {}
    fn conv_choice(&mut self, _: Option<&str>, _: &str, _: &[String], _: usize, _: Option<usize>) {}
    fn conv_cue(&mut self, _: Option<bool>) {}
    fn conv_close(&mut self) {}
    fn conv_events(&mut self) -> Vec<kh_ui::bubble::BubbleEvent> {
        Vec::new()
    }
    fn conv_line_shown(&self) -> bool {
        true
    }
}

fn finish(r: &mut Running) {
    HOLD_PAD.store(false, Ordering::Relaxed);
    PAD_RULE.store(0, Ordering::Relaxed);
    let keys = with(|s| s.keys);
    let grace = a_grace_after(&keys, r.ambient, r.conv.end_reason());
    *A_GRACE.lock().unwrap_or_else(|e| e.into_inner()) = (grace > 0.0).then(|| std::time::Instant::now() + std::time::Duration::from_secs_f32(grace));
    erlog!("conv: #{} ENDED ({}): {:?}, {} line(s) shown", r.id, r.owner, r.conv.end_reason(), r.conv.lines_shown);
}

/// Every character pointer of the game's sets (unlock/host.rs's walk).
fn each_chr(f: &mut dyn FnMut(usize)) {
    // SAFETY: game thread; the singleton lives as long as the game; every pointer is checked before it is followed.
    let Ok(wcm) = (unsafe { WorldChrMan::instance_mut() }) else { return };
    unsafe fn each<T>(set: &ChrSet<T>, f: &mut dyn FnMut(usize))
    where
        T: fromsoftware_shared::Subclass<ChrIns> + 'static,
    {
        let entries = word!(set.entries);
        let capacity = set.capacity as usize;
        if !plausible(entries) || capacity == 0 || capacity > 8192 {
            return;
        }
        for k in 0..capacity {
            let p = std::ptr::read_unaligned((entries + k * 16) as *const usize);
            if plausible(p) {
                f(p);
            }
        }
    }
    // SAFETY: as above.
    unsafe {
        for k in 0..wcm.chr_sets.len() {
            let w = word!(wcm.chr_sets[k]);
            if plausible(w) {
                each(&*(w as *const ChrSet<ChrIns>), f);
            }
        }
        each(&wcm.open_field_chr_set.base, f);
    }
}

fn find_chr(body: Body) -> Option<usize> {
    let mut found = 0usize;
    each_chr(&mut |p| {
        if found == 0 {
            // SAFETY: a character-set entry pointer, checked plausible; fields read only after completeness.
            let chr = unsafe { &*(p as *const ChrIns) };
            if game::chr_complete(chr) && match body {
                Body::Chr(h) => chr.field_ins_handle == h,
                Body::Entity(e) => e != 0 && chr.event_entity_id == e,
                _ => false,
            } {
                found = p;
            }
        }
    });
    (found != 0).then_some(found)
}

fn chr_still(p: usize, body: Body) -> Option<&'static ChrIns> {
    // SAFETY: a pointer of an earlier walk (at most a few frames old), checked complete and still the same character.
    let chr = unsafe { &*(p as *const ChrIns) };
    (game::chr_complete(chr)
        && match body {
            Body::Chr(h) => chr.field_ins_handle == h,
            Body::Entity(e) => chr.event_entity_id == e,
            _ => false,
        })
    .then_some(chr)
}

/// A KH3 point on the HUD's 1920 x 1080 stage with a radius of `r_cm` around it; None behind the camera. The stage is
/// KH3's ExactFit 16:9 viewport in the middle of the picture (on 21:9 a point at the picture's edge is beyond 1920).
pub fn stage_face(frame: &Frame, p_kh: Vec3, r_cm: f32) -> Option<Face> {
    let cam = crate::interp::drawn_camera()?; // (INT) the view drawn in this frame (before: the previous frame's)
    let p = frame.pos_kh_to_er(p_kh);
    let d = p - cam.position;
    let z = d.dot(cam.forward);
    if z < 0.2 {
        return None;
    }
    let t = (cam.fov * 0.5).tan().max(1e-3);
    let aspect = cam.aspect.clamp(0.5, 5.0);
    let (nx, ny) = (d.dot(cam.right) / (z * t * aspect), d.dot(cam.up) / (z * t));
    // the picture's half width / half height in stage pixels: wider than 16:9 the stage is the full height (pillars),
    // narrower it is the full width (letter box)
    let (half_w, half_h) = if aspect >= 16.0 / 9.0 { (540.0 * aspect, 540.0) } else { (960.0, 960.0 / aspect) };
    let (x, y) = (960.0 + nx * half_w, 540.0 - ny * half_h);
    let r = (r_cm / coords::CM_PER_M) / (z * t) * half_h;
    (x.is_finite() && y.is_finite()).then_some(Face { x, y, r: r.clamp(10.0, 160.0) })
}

/// ONCE A FRAME (sora.rs, after the HUD's frame).
pub fn frame(control: &Control, scene: &Scene) {
    // (CV3) `conv.camera` is OFF by default now (the user: "remove the cinematic camera from talking")
    let (on, camera_on, log) = (control.flag("conv.on") != Some(false), control.on("conv.camera"), control.on("conv.log"));
    let keys = FreeKeys::read(control);
    let stop = with(|s| {
        s.on = on;
        s.camera_on = camera_on;
        s.log = log;
        s.keys = keys;
        (!on).then(|| s.running.as_ref().map(|r| r.id)).flatten()
    });
    if let Some(id) = stop {
        end(id, "conv.on = 0");
    }
    // (C.5) the ambient queue: ageing (too old: dropped), the next one starts when nothing runs
    with(|s| {
        for q in s.queue.iter_mut() {
            q.2 += scene.dt;
        }
        let (stale, keep): (Vec<_>, Vec<_>) = s.queue.drain(..).partition(|q| q.2 > QUEUE_WAIT);
        s.queue = keep;
        for (id, b, _) in stale {
            erlog!("conv: #{id} ({}) dropped from the queue: waited {QUEUE_WAIT} s", b.owner);
            keep_ended(s, id, vec![ConvEvent::Ended(EndReason::Host("dropped: waited too long".into()))]);
        }
        if s.running.is_none() && !s.queue.is_empty() && s.on {
            let (id, b, _) = s.queue.remove(0);
            s.running = Some(start(id, b));
        }
    });
    let Some(mut r) = with(|s| s.running.take()) else {
        HOLD_PAD.store(false, Ordering::Relaxed);
        PAD_RULE.store(0, Ordering::Relaxed);
        HOLD_PARTY.store(false, Ordering::Relaxed);
        with(|s| s.cam.running = false);
        return;
    };
    // ---- the participants' poses
    r.poses.clear();
    if r.look_in == 0 {
        r.look_in = 10;
        r.chr.clear();
        for p in &r.parts {
            if matches!(p.body, Body::Chr(_) | Body::Entity(_)) {
                if let Some(ptr) = find_chr(p.body) {
                    r.chr.push((p.actor.id, ptr));
                }
            }
        }
    } else {
        r.look_in -= 1;
    }
    for p in &r.parts {
        let pose = match p.body {
            Body::Leader => Some(Pose { height_cm: p.height_cm.unwrap_or(scene.leader.height_cm), ..scene.leader }),
            Body::Member(c) => scene.members.iter().find(|(m, _)| *m == c).map(|(_, pose)| Pose { height_cm: p.height_cm.unwrap_or(pose.height_cm), ..*pose }),
            Body::Placed => r.placed.iter().find(|(a, _)| *a == p.actor.id).map(|(_, pose)| *pose),
            Body::Chr(_) | Body::Entity(_) => r.chr.iter().find(|(a, _)| *a == p.actor.id).and_then(|(_, ptr)| chr_still(*ptr, p.body)).map(|chr| {
                let er = game::v3(&chr.modules.physics.position);
                Pose { feet_kh: scene.frame.pos_er_to_kh(er), yaw: coords::yaw_kh_of_er_quat(game::quat(&chr.modules.physics.orientation)), height_cm: p.height_cm.unwrap_or(170.0) }
            }),
        };
        if let Some(pose) = pose {
            r.poses.push((p.actor.id, pose));
        }
    }
    r.conv.leader_name = scene.leader_name.clone();
    // ---- the bubble's faces and the runner's step (the HUD's lock held for no game call)
    // (a head with its hair / hat: 22 cm round its face's middle) the party's members who are not in it are kept off too
    // (INT) at the heads AS DRAWN in this frame (the leader's / members' drawn feet; the others where the game has them)
    let drawn_member = |c: PartyChara| scene.drawn_members.iter().find(|(m, _)| *m == c).map(|(_, f)| *f);
    let drawn_head = |a: ActorId, pose: &Pose| {
        let feet = match r.parts.iter().find(|p| p.actor.id == a).map(|p| p.body) {
            Some(Body::Leader) => scene.drawn_leader,
            Some(Body::Member(c)) => drawn_member(c),
            _ => None,
        };
        Pose { feet_kh: feet.unwrap_or(pose.feet_kh), ..*pose }.head()
    };
    let faces: Vec<(ActorId, Option<Face>)> = r.poses.iter().map(|(a, pose)| (*a, stage_face(&scene.frame, drawn_head(*a, pose), 22.0))).collect();
    let bystanders: Vec<Face> = scene.members.iter().filter(|(c, _)| !r.parts.iter().any(|p| p.body == Body::Member(*c))).filter_map(|(c, pose)| stage_face(&scene.frame, Pose { feet_kh: drawn_member(*c).unwrap_or(pose.feet_kh), ..*pose }.head(), 22.0)).collect();
    // (CV3) how far the leader is from the others (walking away ends it; the party is held only while he is near)
    let leader_feet = r.parts.iter().find(|p| p.body == Body::Leader).and_then(|p| r.poses.iter().find(|(a, _)| *a == p.actor.id)).map(|(_, pose)| pose.feet_kh);
    let others: Vec<Vec3> = r.parts.iter().filter(|p| p.body != Body::Leader).filter_map(|p| r.poses.iter().find(|(a, _)| *a == p.actor.id)).map(|(_, pose)| pose.feet_kh).collect();
    let apart = apart_m(leader_feet, &others);
    r.conv.walk_away = (keys.free && keys.walk_away > 0.0).then_some(keys.walk_away);
    r.conv.combat_auto = keys.combat_auto;
    // (C.5) an ambient conversation goes on in a fight (battle lines)
    let input = ConvInput { dt: if scene.hidden { 0.0 } else { scene.dt }, combat: scene.combat && !r.ambient && !STORY_REST.load(Ordering::Relaxed) /* (CV3) a driven story rest: nobody can fight (the body rests), so never hands-free there */, voice_playing: r.voice, apart: if scene.hidden { None } else { apart } };
    let stepped = crate::hud::with_hud(|hud| {
        hud.bubble_dpad_only(keys.free); // (CV3) the stick is the player's movement: the options take the D-pad
        let set_faces = |hud: &mut kh_host::hud::Hud, speaker: Option<ActorId>| {
            hud.bubble_speaker(faces.iter().find(|(a, _)| Some(*a) == speaker).and_then(|(_, f)| *f));
            let mut others: Vec<Face> = faces.iter().filter(|(a, _)| Some(*a) != speaker).filter_map(|(_, f)| *f).collect();
            others.extend(bystanders.iter().copied());
            hud.bubble_faces(&others);
        };
        set_faces(hud, r.conv.speaker());
        if !scene.hidden {
            r.conv.step(hud, &input);
        }
        set_faces(hud, r.conv.speaker());
    });
    if stepped.is_none() {
        let mut view = Closer;
        r.conv.end(&mut view, "the HUD is gone");
    }
    // ---- its events: the owner's copy, the logs, the camera's shots, the members' turns
    let events = r.conv.take_events();
    let mut cues = Vec::new();
    for e in &events {
        match e {
            ConvEvent::Line { index, speaker, voice, anim, .. } => erlog!("conv: #{} line {index} by {} (voice {:?}, anim {:?})", r.id, r.conv.actor(*speaker).map_or("?", |a| a.name.as_str()), voice.as_ref().map(|v| v.key.as_str()), anim),
            ConvEvent::Shot(cue) => {
                if r.camera && camera_on {
                    cues.push(*cue);
                }
                erlog!("conv: #{} shot: {} on {}{}", r.id, cue.kind.name(), r.conv.actor(cue.subject).map_or("?", |a| a.name.as_str()), if cue.cut { " (cut)" } else { "" });
            }
            ConvEvent::Direct(d) => {
                let body = r.parts.iter().find(|p| p.actor.id == d.actor()).map(|p| p.body);
                erlog!("conv: #{} directive {d:?} ({:?})", r.id, body);
                // the leader / a member: done by `hold_party` / not performed (the owner's bodies are the owner's)
                if matches!(body, Some(Body::Leader | Body::Member(_))) {
                    let a = d.actor();
                    let face = match d {
                        Directive::Turn { face, .. } => Some(face.clone()),
                        _ => None,
                    };
                    if let (Some(Body::Member(c)), Some(face)) = (body, face) {
                        TURNS.lock().unwrap_or_else(|e| e.into_inner()).push((c, face_point(&r, &face)));
                    }
                    r.conv.directive_done(a);
                }
            }
            ConvEvent::Chosen { index, option } => erlog!("conv: #{} choice {index}: option {option}", r.id),
            ConvEvent::StopVoice { .. } => erlog!("conv: #{} A before the voice ended: the voice stops", r.id),
            ConvEvent::Suspended | ConvEvent::Resumed => erlog!("conv: #{} {e:?}", r.id),
            _ => {}
        }
    }
    r.outbox.extend(events);
    r.sec += scene.dt;
    if log && r.sec >= 1.0 {
        r.sec = 0.0;
        erlog!("conv: #{} participants: {}", r.id, r.poses.iter().map(|(a, p)| format!("{} at KH ({:.0}, {:.0}, {:.0}) yaw {:.0}", r.conv.actor(*a).map_or("?", |x| x.name.as_str()), p.feet_kh.x, p.feet_kh.y, p.feet_kh.z, p.yaw)).collect::<Vec<_>>().join(", "));
    }
    // ---- the holds, the camera's scene
    let over = r.conv.is_over();
    // (CV3) the leader keeps his pad but A (and at a choice the D-pad / B); hands-free in a fight: nothing is taken
    let rule = pad_rule(&keys, !over && !r.conv.suspended() && !r.ambient, r.conv.takes_a(), r.conv.in_choice());
    if PadRule::of(PAD_RULE.swap(rule.code(), Ordering::Relaxed)) != rule && !over {
        erlog!("conv: #{} the pad: {}", r.id, match rule { PadRule::Free => "the player's (a fight: A attacks, the lines go on by themselves)".to_string(), PadRule::A => "A advances the dialogue (it does nothing else); stick, camera and the other buttons are the player's".to_string(), PadRule::Choice { b } => format!("a choice: D-pad moves, A takes{}; the stick and camera stay the player's", if b { ", B its cancel option" } else { "" }), PadRule::All => "CV1's lock (conv.free = 0)".to_string() });
    }
    HOLD_PAD.store(rule != PadRule::Free, Ordering::Relaxed);
    let subjects: Vec<kh_camera::conv::Subject> = r.poses.iter().map(|(a, p)| kh_camera::conv::Subject { id: *a, feet: p.feet_kh, head: p.head(), yaw: p.yaw }).collect();
    let speaker_head = r.conv.speaker().and_then(|s| r.poses.iter().find(|(a, _)| *a == s)).map(|(_, p)| p.head());
    *SPEAKER_HEAD.lock().unwrap_or_else(|e| e.into_inner()) = (!over).then_some(speaker_head).flatten();
    // (CV3, ERT4 fault 6: Donald stood inside Melina) the bodies a held member keeps clear of: every participant who is
    // not a party member (the leader, the NPCs, a placed figure), with its radius (an Elden Ring character's own)
    {
        let mut bodies = BODIES.lock().unwrap_or_else(|e| e.into_inner());
        bodies.clear();
        if !over {
            for p in &r.parts {
                if matches!(p.body, Body::Member(_)) {
                    continue;
                }
                let Some((_, pose)) = r.poses.iter().find(|(a, _)| *a == p.actor.id) else { continue };
                let radius_cm = match p.body {
                    Body::Chr(_) | Body::Entity(_) => r.chr.iter().find(|(a, _)| *a == p.actor.id).and_then(|(_, ptr)| chr_still(*ptr, p.body)).map_or(NPC_RADIUS_CM, |c| (c.modules.physics.hit_radius * coords::CM_PER_M).clamp(30.0, 150.0)),
                    Body::Leader => LEADER_RADIUS_CM,
                    _ => NPC_RADIUS_CM,
                };
                bodies.push((pose.feet_kh, radius_cm));
            }
        }
    }
    HOLD_PARTY.store(!over && party_held(&keys, r.hold_party, apart) && !r.conv.suspended(), Ordering::Relaxed); // (CV3) only while he stays near
    with(|s| {
        s.cam.running = !over && r.camera && camera_on && !r.conv.suspended();
        s.cam.subjects = subjects;
        s.cam.cues.extend(cues);
    });
    if over {
        finish(&mut r);
        with(|s| keep_ended(s, r.id, std::mem::take(&mut r.outbox)));
    } else {
        with(|s| s.running = Some(r));
    }
}

fn face_point(r: &Running, face: &Facing) -> Option<Vec3> {
    match face {
        Facing::Actor(a) => r.poses.iter().find(|(x, _)| x == a).map(|(_, p)| p.head()),
        Facing::Point(p) => Some(Vec3::from_array(*p)),
        Facing::Yaw(_) => None,
    }
}

/// (CV3) Radii (cm) of the bodies a held member keeps clear of when nothing better is known, a member's own, and the gap.
pub const NPC_RADIUS_CM: f32 = 50.0;
pub const LEADER_RADIUS_CM: f32 = 40.0;
pub const MEMBER_RADIUS_CM: f32 = 40.0;
pub const CLEAR_GAP_CM: f32 = 30.0;
/// (CV3) How fast a held member slides to a hold spot that had to move (cm/s).
pub const HOLD_SLIDE: f32 = 120.0;

/// (CV3) THE CLEAR SPOT of a held member (KH3 cm, flat): pushed out of every body it overlaps (its radius + the other's +
/// `gap`), a few rounds so that being pushed out of one does not leave it inside another. None: it already stands clear
/// (or would move by less than 10 cm). Pure.
pub fn clear_spot(me: Vec3, me_r: f32, others: &[(Vec3, f32)], gap: f32) -> Option<Vec3> {
    let mut p = me;
    for round in 0..8 {
        let mut moved = false;
        for (k, (o, r)) in others.iter().enumerate() {
            let need = me_r + r + gap;
            let d = Vec2::new(p.x - o.x, p.y - o.y);
            let dist = d.length();
            if dist + 0.5 < need {
                // straight away from it; exactly on top of it: a fixed side per body (no NaN)
                let dir = if dist > 1.0 {
                    d / dist
                } else {
                    let (s, c) = (k as f32 * 2.399 + round as f32).sin_cos();
                    Vec2::new(c, s)
                };
                let to = Vec2::new(o.x, o.y) + dir * need;
                p.x = to.x;
                p.y = to.y;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    // squeezed between bodies (being pushed out of one puts him into another): the nearest clear point around him
    let clear = |q: Vec3| others.iter().all(|(o, r)| Vec2::new(q.x - o.x, q.y - o.y).length() + 0.5 >= me_r + r + gap);
    if !clear(p) {
        let ring = (1..=24).flat_map(|k| (0..24).map(move |a| (k as f32 * 15.0, a as f32 * 15.0_f32.to_radians())));
        if let Some(q) = ring.map(|(d, a)| me + Vec3::new(a.cos() * d, a.sin() * d, 0.0)).find(|q| clear(*q)) {
            p = q;
        }
    }
    ((p - me).truncate().length() > 10.0).then_some(p)
}

/// The bodies (KH3 feet, radius cm) of the running conversation's participants who are not party members.
static BODIES: Mutex<Vec<(Vec3, f32)>> = Mutex::new(Vec::new());

/// The speaker's head (KH3) while a conversation holds the party: the members look at it.
static SPEAKER_HEAD: Mutex<Option<Vec3>> = Mutex::new(None);
static HOLD_PARTY: AtomicBool = AtomicBool::new(false);
/// Turns asked of members (performed at the next party step).
static TURNS: Mutex<Vec<(PartyChara, Option<Vec3>)>> = Mutex::new(Vec::new());

/// (ER party, party/mod.rs after the party's step) THE PARTY'S LOCK: while a conversation holds the party every member
/// but the leader stands (his AI stopped, an Idle command, no movement) looking at the speaker; a `Turn` directive turns
/// him. At the end their AI is given back as it was.
#[cfg(feature = "party")]
pub fn hold_party(core: &mut crate::party::glue::PartyCore) {
    static HELD: Mutex<Vec<(PartyChara, bool)>> = Mutex::new(Vec::new());
    // (CV3) each held member's HOLD SPOT (KH3 cm): where he stands for the conversation, kept clear of every body; he is
    // pinned there (nothing pushes him into an NPC) and slides to a new clear spot when a body comes too close
    static SPOTS: Mutex<Vec<(PartyChara, Vec3)>> = Mutex::new(Vec::new());
    static SAID: Mutex<f32> = Mutex::new(0.0);
    let mut spots = SPOTS.lock().unwrap_or_else(|e| e.into_inner());
    let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
    let on = HOLD_PARTY.load(Ordering::Relaxed);
    let leader = core.rt.leader_chara();
    if !on {
        spots.clear();
        for (c, prior) in held.drain(..) {
            if let Some(id) = core.rt.id(c) {
                if let Some(ai) = core.rt.ai_of(id) {
                    ai.set_stop_ai(prior);
                }
            }
            erlog!("conv: {} is let go (AI {})", c.name(), if prior { "stays held, as it was" } else { "on again" });
        }
        return;
    }
    let head = *SPEAKER_HEAD.lock().unwrap_or_else(|e| e.into_inner());
    let turns: Vec<(PartyChara, Option<Vec3>)> = std::mem::take(&mut *TURNS.lock().unwrap_or_else(|e| e.into_inner()));
    let bodies: Vec<(Vec3, f32)> = BODIES.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let members: Vec<(PartyChara, Vec3)> = core.rt.current.iter().filter_map(|v| core.rt.chara_of_id(v.id).map(|c| (c, v.position))).collect();
    // (CV3) once a second: where the held members stand and how far the nearest body is (the proof in the log)
    let mut said = SAID.lock().unwrap_or_else(|e| e.into_inner());
    *said += 1.0 / 60.0;
    if *said >= 1.0 {
        *said = 0.0;
        let line: Vec<String> = spots.iter().map(|(c, p)| {
            let me = members.iter().filter(|(m, _)| *m == *c).map(|(_, q)| *q).next().unwrap_or(*p);
            let near = bodies.iter().chain(members.iter().filter(|(m, _)| *m != *c).map(|(_, q)| (*q, MEMBER_RADIUS_CM)).collect::<Vec<_>>().iter()).map(|(o, r)| (me - *o).truncate().length() - r - MEMBER_RADIUS_CM).fold(f32::INFINITY, f32::min);
            format!("{} at KH ({:.0}, {:.0}), {:.0} cm clear of the nearest body", c.name(), me.x, me.y, near)
        }).collect();
        if !line.is_empty() {
            erlog!("conv: held: {}", line.join("; "));
        }
    }
    drop(said);
    for c in [PartyChara::Sora, PartyChara::Donald, PartyChara::Goofy] {
        if c == leader {
            continue;
        }
        let Some(id) = core.rt.id(c) else { continue };
        if !held.iter().any(|(h, _)| *h == c) {
            let prior = core.rt.ai_of(id).map(|ai| ai.state().stop_ai).unwrap_or(false);
            if let Some(ai) = core.rt.ai_of(id) {
                ai.set_stop_ai(true);
            }
            let _ = core.rt.party.command(id, kh_party::MemberCommand::Idle { time: 3600.0 });
            held.push((c, prior));
            erlog!("conv: {} is held (AI stopped, standing)", c.name());
        }
        // (CV3, ERT4 fault 6) NEVER INSIDE A BODY: a held member keeps clear of the NPCs, the leader and the other members.
        // His hold spot starts where he stands, is pushed clear of every body (clear_spot) each step, and he is PINNED on
        // it (before he was only stopped, and pushes / his body's resets carried him into Melina); a spot that moved slides
        // him over at HOLD_SLIDE cm/s.
        let mut others = bodies.clone();
        others.extend(members.iter().filter(|(m, _)| *m != c).map(|(_, p)| (*p, MEMBER_RADIUS_CM)));
        if let Some(m) = core.rt.party.member_mut(id) {
            let k = match spots.iter().position(|(s, _)| *s == c) {
                Some(k) => k,
                None => {
                    spots.push((c, m.cmc.location));
                    spots.len() - 1
                }
            };
            let spot = spots[k].1;
            if let Some(to) = clear_spot(spot, MEMBER_RADIUS_CM, &others, CLEAR_GAP_CM) {
                erlog!("conv: {} would stand inside a body: his hold spot moves {:.0} cm to stand clear (gap {:.0} cm)", c.name(), (to - spot).truncate().length(), CLEAR_GAP_CM);
                spots[k].1 = to;
            }
            let spot = spots[k].1;
            let d = Vec2::new(spot.x - m.cmc.location.x, spot.y - m.cmc.location.y);
            let step = HOLD_SLIDE / 60.0;
            let at = if d.length() <= step { Vec2::new(spot.x, spot.y) } else { Vec2::new(m.cmc.location.x, m.cmc.location.y) + d / d.length() * step };
            m.cmc.location.x = at.x;
            m.cmc.location.y = at.y;
            m.cmc.stop_movement();
            // looking at the speaker (Idle faces the focus; the head look-at follows it); a member speaking looks ahead
            let speaker_is_him = head.is_some_and(|h| (h - m.cmc.location).truncate().length() < 30.0);
            if let (Some(h), false) = (head, speaker_is_him) {
                m.focus = kh_party::TargetRef::Point(h);
            }
            for (_, at) in turns.iter().filter(|(t, _)| *t == c) {
                if let Some(at) = at {
                    let d = *at - m.cmc.location;
                    m.yaw = d.y.atan2(d.x).to_degrees();
                }
            }
        }
    }
}

/// A party character's height for [`Pose::height_cm`] (the face is at 0.93 of it): KH3's models, MEASURED in the effects
/// harness's frames (Donald's face at 76 cm, Sora's at 140 cm standing).
pub fn height_of(c: PartyChara) -> f32 {
    match c {
        PartyChara::Sora => 150.0,
        PartyChara::Donald => 82.0,
        PartyChara::Goofy => 165.0,
    }
}

/// (sora.rs) Sora's pose when he leads alone.
pub fn sora_pose(out: &kh_core::Output) -> Pose {
    Pose { feet_kh: out.position, yaw: out.yaw, height_cm: height_of(PartyChara::Sora) }
}

/// (sora.rs) The leader, the members and whether a member is in battle, from the party runtime (None: Sora alone).
#[cfg(feature = "party")]
/// (CV3) The bool is THE LEADER'S FIGHT ([`leader_combat`]): Sora's battle flag only while he is in the active party.
pub fn party_scene(core: Option<&crate::party::glue::PartyCore>, out: &kh_core::Output, sora: &kh_core::Sora) -> (Pose, Vec<(PartyChara, Pose)>, bool, String) {
    let Some(core) = core else { return (sora_pose(out), Vec::new(), sora.actions().battle, "Sora".to_string()) };
    let lv = core.rt.leader_view_of(out, sora.status());
    let leader = Pose { feet_kh: lv.position, yaw: lv.yaw, height_cm: height_of(lv.chara) };
    let mut members = Vec::new();
    let mut battle = false;
    for v in &core.rt.current {
        let Some(c) = core.rt.chara_of_id(v.id) else { continue };
        battle |= v.in_battle;
        if c != lv.chara {
            members.push((c, Pose { feet_kh: v.position, yaw: v.yaw, height_cm: height_of(c) }));
        }
    }
    (leader, members, leader_combat(!core.sora_skipped(), sora.actions().battle, battle), lv.chara.name().to_string())
}

#[cfg(test)]
mod cv3_tests {
    /// (CV3 round 4) Goofy leads, Sora out of the party frozen off the floor in his Margit attack: the save point sees
    /// Goofy on the floor, not fighting, alive. Sora leading: his own facts.
    #[test]
    fn the_save_point_reads_the_leader_not_a_frozen_sora() {
        use super::{leader_facts, LeaderFacts};
        assert_eq!(leader_facts(false, true, false, true, false), LeaderFacts { on_floor: true, battle: false, alive: true }, "Goofy leads, Sora frozen in battle off the floor");
        assert_eq!(leader_facts(true, true, false, false, false), LeaderFacts { on_floor: false, battle: true, alive: true }, "Sora leads, in the air in a fight");
        assert_eq!(leader_facts(false, false, true, true, true), LeaderFacts { on_floor: true, battle: true, alive: false }, "a member fights; the leader is knocked out");
    }

    use super::*;
    use kh_core::{PadButton as B, PadButtons};

    fn pad(buttons: &[B]) -> PadButtons {
        let mut p = PadButtons::NONE;
        for b in buttons {
            p.set(*b, true);
        }
        p
    }

    /// (CV3) THE A ROUTING: out of a fight A is the bubble's and nothing else of the pad (the sticks move him and his
    /// camera, B jumps); at a choice also the D-pad and B; in a fight (hands-free) nothing; `conv.free = 0` CV1's lock.
    #[test]
    fn a_is_reserved_out_of_a_fight_and_the_rest_stays_the_players() {
        let keys = FreeKeys::default();
        assert_eq!(pad_rule(&keys, false, false, false), PadRule::Free, "no conversation");
        assert_eq!(pad_rule(&keys, true, true, false), PadRule::A, "a line out of a fight");
        assert_eq!(pad_rule(&keys, true, true, true), PadRule::Choice { b: true }, "a choice");
        assert_eq!(pad_rule(&keys, true, false, false), PadRule::Free, "hands-free in a fight: A attacks");
        assert_eq!(pad_rule(&FreeKeys { free: false, ..keys }, true, false, false), PadRule::All, "conv.free = 0");
        assert_eq!(pad_rule(&FreeKeys { b_cancel: false, ..keys }, true, true, true), PadRule::Choice { b: false });
        for rule in [PadRule::Free, PadRule::A, PadRule::Choice { b: false }, PadRule::Choice { b: true }, PadRule::All] {
            assert_eq!(PadRule::of(rule.code()), rule);
        }
        let all = [B::FaceBottom, B::FaceRight, B::FaceLeft, B::FaceTop, B::DPadUp, B::DPadDown, B::RightShoulder];
        let (mut stick, mut right, mut held) = (Vec2::new(0.0, 1.0), Vec2::new(0.7, 0.0), pad(&all));
        apply_pad(PadRule::A, &mut stick, &mut right, &mut held);
        assert_eq!(held, pad(&[B::FaceRight, B::FaceLeft, B::FaceTop, B::DPadUp, B::DPadDown, B::RightShoulder]), "only A is taken");
        assert!(stick == Vec2::new(0.0, 1.0) && right == Vec2::new(0.7, 0.0), "he walks and turns his camera");
        let mut held = pad(&all);
        apply_pad(PadRule::Choice { b: true }, &mut stick, &mut right, &mut held);
        assert_eq!(held, pad(&[B::FaceLeft, B::FaceTop, B::RightShoulder]), "a choice: A, B, the D-pad are the bubble's");
        assert!(stick.length() > 0.9 && right.length() > 0.6, "the sticks stay his at a choice");
        let mut held = pad(&all);
        apply_pad(PadRule::Free, &mut stick, &mut right, &mut held);
        assert_eq!(held, pad(&all), "in a fight A attacks");
        apply_pad(PadRule::All, &mut stick, &mut right, &mut held);
        assert!(held == PadButtons::NONE && stick == Vec2::ZERO && right == Vec2::ZERO);
    }

    /// (CV3, ERT4 fault 6) A held member never stands inside a body: pushed out of Melina, the leader, another member,
    /// also when two overlap at once; one already clear stays.
    #[test]
    fn a_held_member_steps_clear_of_every_body() {
        let melina = (Vec3::new(100.0, 0.0, 0.0), 50.0);
        let leader = (Vec3::new(-60.0, 0.0, 0.0), LEADER_RADIUS_CM);
        let goofy = (Vec3::new(100.0, 120.0, 0.0), MEMBER_RADIUS_CM);
        let others = [melina, leader, goofy];
        let clear = |p: Vec3| others.iter().all(|(o, r)| (p - *o).truncate().length() + 0.6 >= MEMBER_RADIUS_CM + r + CLEAR_GAP_CM);
        for start in [Vec3::new(100.0, 0.0, 0.0), Vec3::new(110.0, 20.0, 0.0), Vec3::new(30.0, 0.0, 0.0), Vec3::new(100.0, 90.0, 0.0)] {
            let to = clear_spot(start, MEMBER_RADIUS_CM, &others, CLEAR_GAP_CM).expect("inside: a step");
            assert!(clear(to), "{start:?} -> {to:?} still inside a body");
            assert!((to - start).truncate().length() < 260.0, "a short step: {start:?} -> {to:?}");
        }
        assert_eq!(clear_spot(Vec3::new(400.0, -300.0, 0.0), MEMBER_RADIUS_CM, &others, CLEAR_GAP_CM), None, "already clear");
    }

    /// (CV3) THE LEADER'S FIGHT whoever leads; how far apart; the party held only while he is near.
    #[test]
    fn the_leaders_fight_the_distance_and_the_party_hold() {
        assert!(!leader_combat(false, true, false), "Sora out of the party with a frozen flag: not a fight (the user's bug)");
        assert!(leader_combat(true, true, false) && leader_combat(false, false, true) && !leader_combat(true, false, false));
        assert_eq!(apart_m(None, &[Vec3::ZERO]), None);
        assert_eq!(apart_m(Some(Vec3::ZERO), &[]), None);
        let d = apart_m(Some(Vec3::new(0.0, 0.0, 50.0)), &[Vec3::new(300.0, 400.0, 0.0), Vec3::new(600.0, 0.0, 0.0)]).unwrap();
        assert!((d - 5.0).abs() < 1e-4, "flat metres to the nearest: {d}");
        let keys = FreeKeys::default();
        assert!(party_held(&keys, true, Some(2.0)) && !party_held(&keys, true, Some(4.0)) && !party_held(&keys, false, Some(1.0)));
        assert!(party_held(&FreeKeys { free: false, ..keys }, true, Some(40.0)), "CV1's lock holds them anywhere");
        // the A grace after the end: the script / the owner ended it (A mashed through the last line); not after walking
        // away, a fight, an ambient one
        assert_eq!(a_grace_after(&keys, false, Some(&EndReason::Script)), keys.a_grace);
        assert_eq!(a_grace_after(&keys, false, Some(&EndReason::Host("the Elden Ring talk ended".into()))), keys.a_grace);
        assert_eq!(a_grace_after(&keys, false, Some(&EndReason::WalkedAway)), 0.0);
        assert_eq!(a_grace_after(&keys, false, Some(&EndReason::Combat)), 0.0);
        assert_eq!(a_grace_after(&keys, true, Some(&EndReason::Script)), 0.0);
    }
}
