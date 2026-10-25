//! The node classes the friends' trees use: stock UE 4.17 nodes (as KH3's fork changed them) and
//! the Tres / TresNpc classes, transcribed from the decompiled C where it was read (address in the
//! comment, evidence [C]) and from the class's name and fields otherwise ([D]). The per-class list
//! with addresses and evidence is `CLASSES` (also `sheets/port/party-ai-nodes.json`).
//!
//! Decorators override only `CalculateRawConditionValue` unless noted: in KH3 only decorators with
//! a TickNode or a blackboard observer abort on their own (BoolPropertyCheck 0x140476E30,
//! Blackboard, Cooldown 0x143C6D200, TagCooldown 0x143C6D450, TimeLimit 0x143C66760); the
//! FlowAbortMode of the other Tres decorators has no runtime effect in the game either.

use crate::api::{ActorRef, ActorView, AiCommand, AiCommandStatus, AiGoal};
use crate::bb::{is_valid_location, BbObject, BbValue};
use crate::bt::{AbortMode, Ctx, DecoEffect, DecoratorNode, Mem, Res, ServiceNode, TaskNode};
use crate::eqs::{self, Params, Querier};
use crate::pack::{enum_value, parr, passet, pb, pf, pi, pkey, pnative, pobj, ps, ptag, AiPack, Props};
use glam::Vec3;

pub mod validate;

// ---- helpers ---------------------------------------------------------------------------------

pub fn self_ref(c: &Ctx) -> ActorRef {
    ActorRef::Member(c.st.member)
}

pub fn self_view(c: &Ctx) -> ActorView {
    let b = c.body.view();
    ActorView {
        actor: self_ref(c),
        uid: b.uid,
        position: b.position,
        center: b.position + Vec3::Z * b.half_height,
        yaw: b.yaw,
        velocity: b.velocity,
        radius: b.radius,
        half_height: b.half_height,
        alive: !b.ko,
        hostile: false,
        state: b.state,
        hp: b.hp as f32,
        hp_max: b.hp_max as f32,
        ..Default::default()
    }
}

pub fn view(c: &Ctx, a: ActorRef) -> Option<ActorView> {
    match a {
        ActorRef::None => None,
        ActorRef::Player => Some(c.world.player().actor),
        x if x == self_ref(c) => Some(self_view(c)),
        x => c.world.actor(x),
    }
}

fn key(c: &Ctx, k: &Option<String>) -> Option<crate::bb::KeyId> {
    k.as_ref().and_then(|n| c.bb.key_id(n))
}

pub fn key_obj(c: &Ctx, k: &Option<String>) -> BbObject {
    key(c, k).map(|id| c.bb.object(id)).unwrap_or_default()
}

pub fn key_actor(c: &Ctx, k: &Option<String>) -> ActorRef {
    key_obj(c, k).actor()
}

/// A key's location: actor origin, a component's centre, or a vector (None = unset / invalid).
pub fn key_pos(c: &Ctx, k: &Option<String>) -> Option<Vec3> {
    let id = key(c, k)?;
    match c.bb.get(id)? {
        BbValue::Vector(v) => is_valid_location(*v).then_some(*v),
        BbValue::Object(BbObject::Actor(a)) => view(c, *a).map(|v| v.position),
        BbValue::Object(BbObject::Component(a, _)) => view(c, *a).map(|v| v.center),
        _ => None,
    }
}

fn goal_pos(c: &Ctx, g: AiGoal) -> Option<Vec3> {
    match g {
        AiGoal::Point(p) => Some(p),
        AiGoal::Actor(a) => view(c, a).map(|v| v.position),
    }
}

fn set_key(c: &mut Ctx, k: &Option<String>, v: BbValue) {
    if let Some(id) = key(c, k) {
        c.bb.set(id, v);
    }
}

/// A blueprint function / property of an actor by name (BoolPropertyCheck, ArithmeticComparison
/// ActorFunction). The pawn answers for itself through `AiBody::function`; the rest are answered
/// from the views [D: the functions' bodies are blueprint / native code not transcribed here].
pub fn call_fn(c: &Ctx, actor: ActorRef, name: &str) -> f32 {
    let b = |x: bool| x as i32 as f32;
    let p = c.world.player();
    if actor == self_ref(c) || actor == ActorRef::None {
        if let Some(v) = c.body.function(name) {
            return v;
        }
        let bv = c.body.view();
        return match name {
            // ATresNpcPawnBase::CanBattle 0x141257340 [C]: false with the disable byte +0xCA9 or in a
            // ride; otherwise the pawn's own battle flag (vt+0x1740), the first time only once the
            // player's (+0xC68) or its own (0x140541660) battle time passed 0.5 s (latch +0xCA8).
            // The player's battle flag alone never answers it (B's view carries the pawn's rule).
            "CanBattle" => b(bv.can_battle),
            "IsRideOn" => b(bv.ride_on),
            "IsSwimming" => b(bv.swimming),
            "IsStateMoving" => b(bv.state == "TSID_STATE_AI_MOVE" || bv.state == "TSID_STATE_AI_SWIM"),
            "GetMagicPoint" => bv.mp as f32,
            "IsBattleFollowMode" => b(c.st.battle_follow_mode),
            // Goofy's root runs an endless Wait under "IsActionOff() != true": in normal play the
            // function must answer true [D from the tree's logic; B answers it for real]
            "IsActionOff" => 1.0,
            "IsHostileTarget" => 0.0,
            "IsPlayerBeingAttackedByEnemy" => b(p.attacked_by_enemies),
            "IsPlayerAttackNow" => b(p.attacking),
            "IsSameTargetAsPlayer" => {
                let t = c.bb.key_id("TargetActor").map(|k| c.bb.actor(k)).unwrap_or_default();
                b(!t.is_none() && t == p.lock_on)
            }
            _ => 0.0,
        };
    }
    let v = view(c, actor);
    match name {
        "IsHostileTarget" => b(v.map(|v| v.hostile && v.alive).unwrap_or(false)),
        "IsSwimming" => b(if actor == ActorRef::Player { p.swimming } else { false }),
        "IsRideOn" => b(actor == ActorRef::Player && p.ride_on),
        "IsStateMoving" => b(v.map(|v| v.velocity.truncate().length() > 1.0).unwrap_or(false)),
        "IsSameTargetAsPlayer" => b(!actor.is_none() && actor == p.lock_on),
        "IsPlayerBeingAttackedByEnemy" => b(p.attacked_by_enemies),
        "IsPlayerAttackNow" => b(p.attacking),
        _ => 0.0,
    }
}

fn arith(op: &str, a: f32, b: f32) -> bool {
    match op {
        "NotEqual" => a != b,
        "Less" => a < b,
        "LessOrEqual" => a <= b,
        "Greater" => a > b,
        "GreaterOrEqual" => a >= b,
        _ => a == b,
    }
}

fn yaw_dir(yaw: f32) -> Vec3 {
    let r = yaw.to_radians();
    Vec3::new(r.cos(), r.sin(), 0.0)
}

fn locomotion(c: &Ctx, use_bb: bool, bb_key: &Option<String>, fixed: &Option<String>) -> String {
    // FUN_1404AEAB0: BB class key when use_bb, else the fixed class; none -> the pawn's active
    // locomotion +0x9E8 (0x1404B2C30) = the ActiveLocomotion key kh-ai keeps [C]
    let chosen = if use_bb { key(c, bb_key).and_then(|k| c.bb.class(k)) } else { fixed.clone() };
    chosen
        .or_else(|| c.bb.key_id("ActiveLocomotion").and_then(|k| c.bb.class(k)))
        .unwrap_or_else(|| format!("{}_Move1", c.st.pawn))
}

fn poll(c: &Ctx, ticket: u32) -> Option<Res> {
    match c.body.command_status(ticket) {
        AiCommandStatus::Running => None,
        AiCommandStatus::Succeeded => Some(Res::Succeeded),
        _ => Some(Res::Failed),
    }
}

// ---- tasks -----------------------------------------------------------------------------------

#[derive(Debug)]
struct Wait {
    time: f32,
    dev: f32,
}
impl TaskNode for Wait {
    // ExecuteTask 0x143C5BD40 / TickTask 0x143C6D990 [C]
    fn execute(&self, c: &mut Ctx, m: &mut Mem) -> Res {
        m.f[0] = c.st.rng.deviate(self.time, self.dev);
        Res::InProgress
    }
    fn tick(&self, _c: &mut Ctx, m: &mut Mem, dt: f32) -> Option<Res> {
        m.f[0] -= dt;
        (m.f[0] <= 0.0).then_some(Res::Succeeded)
    }
    fn describe(&self) -> String {
        format!("wait {}±{}", self.time, self.dev)
    }
}

#[derive(Debug)]
struct Noop;
impl TaskNode for Noop {
    // ExecuteTask 0x1404AC250 (8 bytes: return Succeeded) [C]
    fn execute(&self, _c: &mut Ctx, _m: &mut Mem) -> Res {
        Res::Succeeded
    }
}

/// `TresBTTask_Action` (Execute 0x140494D70, update 0x1404A2370, Abort 0x140492CB0, Tick
/// 0x1404A4540) [C]: asks the pawn's state component to start the action every tick until it
/// accepts (the task stays InProgress meanwhile), then waits for the action's end message;
/// `m_AbortTime` > 0 cancels it after that long; AbortTask cancels the action.
/// `TresNpcBTTask_NpcCureAction` (Execute 0x14120DF60) shares it with the Cure command.
#[derive(Debug)]
struct Action {
    target: Option<String>,
    use_bb_def: bool,
    bb_def: Option<String>,
    def: Option<String>,
    abort_time: f32,
    cure: bool,
}
impl Action {
    fn try_start(&self, c: &mut Ctx, m: &mut Mem) {
        if m.flag & 1 != 0 {
            return;
        }
        let def = if self.use_bb_def { key(c, &self.bb_def).and_then(|k| c.bb.class(k)) } else { None }.or_else(|| self.def.clone());
        let Some(def) = def else { return };
        let target = key_actor(c, &self.target);
        let location = key(c, &self.target).and_then(|k| match c.bb.get(k) {
            Some(BbValue::Vector(v)) if is_valid_location(*v) => Some(*v),
            _ => None,
        });
        let cmd = if self.cure { AiCommand::Cure { target } } else { AiCommand::Action { def: def.clone(), target, location } };
        let t = c.body.command(cmd);
        if c.body.command_status(t) != AiCommandStatus::Failed {
            m.ticket = t;
            m.flag |= 1;
            m.f[0] = c.st.time as f32;
            m.s = def;
        }
    }
}
impl TaskNode for Action {
    fn execute(&self, c: &mut Ctx, m: &mut Mem) -> Res {
        m.flag = 0;
        m.f[0] = c.st.time as f32;
        if self.cure {
            // NpcCureAction Execute 0x14120DF60 [C]: the MetaAI record is cleared (+0x290..+0x2A8)
            // before the Cure action runs
            c.shared.cure_info = None;
        }
        self.try_start(c, m);
        Res::InProgress
    }
    fn tick(&self, c: &mut Ctx, m: &mut Mem, _dt: f32) -> Option<Res> {
        if m.flag & 1 != 0 {
            if self.abort_time > 0.0 && self.abort_time <= c.st.time as f32 - m.f[0] {
                c.body.command(AiCommand::Stop);
                return Some(Res::Failed);
            }
            return poll(c, m.ticket);
        }
        self.try_start(c, m);
        None
    }
    fn abort(&self, c: &mut Ctx, m: &mut Mem) -> Res {
        if m.flag & 1 != 0 && c.body.command_status(m.ticket) == AiCommandStatus::Running {
            c.body.command(AiCommand::Stop);
        }
        Res::Aborted
    }
    fn describe(&self) -> String {
        format!("action {:?}{}", self.def.as_deref().or(self.bb_def.as_deref()), if self.cure { " (cure)" } else { "" })
    }
}

/// `TresBTTask_MoveTo` (Execute 0x1404ABCE0, PerformMoveTask 0x1404B56C0, Tick 0x1404BD470,
/// Abort 0x1404A8700, OnTaskFinished 0x1404B5140, messages 0x1404B8B50; ctor 0x1404A5B20) /
/// `TresNpcBTTask_NpcMoveTo` (Execute 0x14120E040 / OnTaskFinished 0x141218880: m_bWalk sets
/// the battle controller's walk byte +0x22D for the task's run = `walk` of the command) [C]:
/// - goal: a valid vector key, an actor (followed), or a scene component (lock-on component):
///   its location projected to the floor (FUN_140542040, 5000 below) as a point goal, re-pathed
///   once the component moved more than `m_GoalComponetLocationTetherDistance` (default 50)
///   from it (Tick); no goal -> Failed;
/// - locomotion (FUN_1404AEAB0 / 0x1404B2C30): the BB class key when `m_bUseBBLocomotionDefinition`
///   (default off), else `m_LocomotionDefinition`; none -> the pawn's active locomotion (+0x9E8:
///   the `ActiveLocomotion` key kh-ai keeps);
/// - controller MoveTo result: AlreadyAtGoal -> Succeeded, request -> InProgress, else Failed
///   [U: the at-goal test is the path-following component's; kh-ai: 2D distance <= radius];
/// - Tick: `m_bFailOnDamageReaction` (default on) and the pawn in TSID_STATE_DAMAGE (0x13) ->
///   StopMovement + Failed;
/// - Abort: StopMovement, then the pawn keeps moving for `m_AbortMoveTimeToKeepMoving` (default
///   0.1 s; movement +0x448 vt+0x120): kh-ai sends the Stop that much later unless another
///   command came first (`AiState::keep_moving_until`).
#[derive(Debug)]
struct MoveTo {
    key: Option<String>,
    radius: f32,
    use_bb_loco: bool,
    bb_loco: Option<String>,
    loco: Option<String>,
    walk: bool,
    follow_path: bool,
    fail_on_damage: bool,
    keep_moving: f32,
    tether: f32,
}
impl MoveTo {
    /// The goal and, for a component goal, the component's current centre.
    fn goal(&self, c: &Ctx) -> Option<(AiGoal, Option<Vec3>)> {
        let id = key(c, &self.key)?;
        match c.bb.get(id)? {
            BbValue::Vector(v) => is_valid_location(*v).then_some((AiGoal::Point(*v), None)),
            BbValue::Object(BbObject::Component(a, _)) => {
                let cc = view(c, *a)?.center;
                let floor = c.world.nav().project(cc, 0.0, 5000.0).unwrap_or(cc);
                Some((AiGoal::Point(floor), Some(cc)))
            }
            BbValue::Object(o) if !o.is_none() => Some((AiGoal::Actor(o.actor()), None)),
            _ => None,
        }
    }
    fn request(&self, c: &mut Ctx, m: &mut Mem) -> Res {
        let Some((goal, comp)) = self.goal(c) else { return Res::Failed };
        m.v = comp.unwrap_or(Vec3::ZERO);
        m.flag = comp.is_some() as u32;
        // (party K1) a vector goal is observed (bit 2, the goal in m.v): see tick
        if let (AiGoal::Point(p), None) = (goal, comp) {
            m.v = p;
            m.flag |= 2;
        }
        let me = c.body.view().position;
        if let Some(gp) = goal_pos(c, goal) {
            c.shared.destinations.insert(c.st.member, gp);
            if (gp - me).truncate().length() <= self.radius {
                return Res::Succeeded;
            }
        }
        let locomotion = locomotion(c, self.use_bb_loco, &self.bb_loco, &self.loco);
        m.ticket = c.body.command(AiCommand::MoveTo { goal, locomotion, walk: self.walk, acceptable_radius: self.radius, follow_path: self.follow_path });
        c.st.keep_moving_until = None;
        Res::InProgress
    }
}
impl TaskNode for MoveTo {
    fn execute(&self, c: &mut Ctx, m: &mut Mem) -> Res {
        self.request(c, m)
    }
    fn tick(&self, c: &mut Ctx, m: &mut Mem, _dt: f32) -> Option<Res> {
        if self.fail_on_damage && c.body.view().state == "TSID_STATE_DAMAGE" {
            c.body.command(AiCommand::Stop);
            return Some(Res::Failed);
        }
        // (party K1) THE BLACKBOARD OBSERVER: Execute 0x1404ABCE0 registers FUN_1404B3610 on the task's key (FUN_143C410D0);
        // a vector key that moved more than 0.95 x the tolerance (+0xB8, read as m_AcceptableRadius [inf]) from the
        // running goal re-paths the move to it (PerformMoveTask 0x1404B56C0) while the task keeps running: the friends
        // chase the Destination their EQS service refreshes every 0.1 s instead of arriving, stopping and starting again.
        if m.flag & 2 != 0 {
            if let Some((AiGoal::Point(p), None)) = self.goal(c) {
                let tol = self.radius.max(1.0);
                if p.distance_squared(m.v) > tol * tol * 0.9025 {
                    return match self.request(c, m) {
                        Res::InProgress => None,
                        r => Some(r),
                    };
                }
            }
        }
        if m.flag & 1 != 0 {
            if let Some((_, Some(cc))) = self.goal(c) {
                if cc.distance_squared(m.v) > self.tether * self.tether {
                    return match self.request(c, m) {
                        Res::InProgress => None,
                        r => Some(r),
                    };
                }
            }
        }
        poll(c, m.ticket)
    }
    fn abort(&self, c: &mut Ctx, _m: &mut Mem) -> Res {
        if self.keep_moving > 0.0 {
            c.st.keep_moving_until = Some(c.st.time + self.keep_moving as f64);
        } else {
            c.body.command(AiCommand::Stop);
        }
        Res::Aborted
    }
    fn describe(&self) -> String {
        format!("move to {:?} r{}", self.key, self.radius)
    }
}

/// `TresBTTask_TurnTo` (Execute 0x1404AD040, update 0x1404B69B0, Tick 0x1404BE2C0; ctor 0x1404A6680)
/// [C]: Execute draws one random offset FRand in [-RandomAngleLimit, +RandomAngleLimit]; every
/// tick until the state component accepts the turn: the desired yaw = direction to the key +
/// `Offset` + the random offset; within 0.1 deg of the facing -> Succeeded; no valid key (or the
/// DANGLE state) -> Failed; else request the turn (state comp vt+0x3C0) and, once accepted, wait
/// for its TurnFinished message (= the command's Succeeded).
#[derive(Debug)]
struct TurnTo {
    key: Option<String>,
    offset: f32,
    random: f32,
}
impl TurnTo {
    fn update(&self, c: &mut Ctx, m: &mut Mem) -> Option<Res> {
        let b = c.body.view();
        if b.state == "TSID_STATE_DANGLE" {
            return Some(Res::Failed);
        }
        let Some(tp) = key_pos(c, &self.key) else { return Some(Res::Failed) };
        let me = b.position + Vec3::Z * b.half_height;
        let (_, base) = validate::rotation(tp - me);
        let yaw = base + self.offset + m.f[0];
        if validate::norm(yaw - b.yaw).abs() <= 0.1 {
            return Some(Res::Succeeded);
        }
        let t = c.body.command(AiCommand::TurnTo { goal: AiGoal::Point(b.position + yaw_dir(yaw) * 100.0) });
        if c.body.command_status(t) != AiCommandStatus::Failed {
            m.ticket = t;
            m.flag = 1;
        }
        None
    }
}
impl TaskNode for TurnTo {
    fn execute(&self, c: &mut Ctx, m: &mut Mem) -> Res {
        m.flag = 0;
        m.f[0] = c.st.rng.frand() * (2.0 * self.random) - self.random;
        self.update(c, m).unwrap_or(Res::InProgress)
    }
    fn tick(&self, c: &mut Ctx, m: &mut Mem, _dt: f32) -> Option<Res> {
        if m.flag == 0 {
            return self.update(c, m);
        }
        poll(c, m.ticket)
    }
    fn abort(&self, c: &mut Ctx, _m: &mut Mem) -> Res {
        c.body.command(AiCommand::Stop);
        Res::Aborted
    }
}

/// The idle the tasks start runs until the pawn leaves it (the game's IdleFinished message comes
/// from the pawn's idle, not from a timer) [D: the sender of IdleFinished was not located; B's
/// `Idle { time }` finishes after `time`, so kh-ai asks for an unbounded one].
const IDLE_UNTIL_FINISHED: f32 = f32::MAX;

/// `TresBTTask_Idle` (Execute 0x1404ABCC0, update 0x1404B5600, Tick 0x1404BD410) [C]: for up to
/// 2 s, once the pawn is in TSID_STATE_IDLE, start the idle (pawn vt+0x16E8) and wait for its
/// IdleFinished message (the command's Succeeded); 2 s without IDLE -> Failed.
#[derive(Debug)]
struct Idle;
impl Idle {
    fn try_start(c: &mut Ctx, m: &mut Mem) {
        if c.body.view().state == "TSID_STATE_IDLE" {
            m.ticket = c.body.command(AiCommand::Idle { time: IDLE_UNTIL_FINISHED });
            m.flag |= 1;
        }
    }
}
impl TaskNode for Idle {
    fn execute(&self, c: &mut Ctx, m: &mut Mem) -> Res {
        m.flag = 0;
        m.f[0] = 2.0;
        Idle::try_start(c, m);
        Res::InProgress
    }
    fn tick(&self, c: &mut Ctx, m: &mut Mem, dt: f32) -> Option<Res> {
        if m.flag & 1 != 0 {
            return poll(c, m.ticket);
        }
        m.f[0] -= dt;
        if m.f[0] <= 0.0 {
            return Some(Res::Failed);
        }
        Idle::try_start(c, m);
        None
    }
    fn abort(&self, _c: &mut Ctx, _m: &mut Mem) -> Res {
        Res::Aborted
    }
}

/// `TresBTTask_QuickTurnIdling` (Execute 0x1404AC320, update 0x1404B5E50, facing test 0x1404B2520,
/// Tick 0x1404BDAA0, message 0x1404B4260; ctor 0x1404A5FC0) [C]: a 2 s window; facing the key
/// within `TurnLimit` (dot >= cos(TurnLimit) - 0.01) -> idle until IdleFinished; else turn toward
/// it by ±(`Offset` + random in ±`RandomAngleLimit`) on the side of the turn and finish with the
/// turn; after a turn with `CoolDownTimer` > 0 the next Execute only idles until the cooldown ran.
#[derive(Debug)]
struct QuickTurnIdling {
    key: Option<String>,
    offset: f32,
    random: f32,
    limit: f32,
    cooldown: f32,
}
impl QuickTurnIdling {
    fn facing(&self, c: &Ctx) -> bool {
        let b = c.body.view();
        let Some(tp) = key_pos(c, &self.key) else { return true };
        let to = (tp - b.position).truncate().normalize_or_zero();
        yaw_dir(b.yaw).truncate().dot(to) - self.limit.to_radians().cos() >= -0.01
    }
    fn update(&self, c: &mut Ctx, m: &mut Mem) -> Option<Res> {
        if m.f[1] < 0.0 {
            return Some(Res::Failed);
        }
        if self.facing(c) {
            if m.flag & 2 == 0 {
                m.ticket = c.body.command(AiCommand::Idle { time: IDLE_UNTIL_FINISHED });
                m.flag |= 2;
            }
            return None;
        }
        let b = c.body.view();
        let tp = key_pos(c, &self.key)?;
        let (_, base) = validate::rotation(tp - b.position);
        let add = self.offset + m.f[2];
        let yaw = if validate::norm(base - b.yaw) <= 0.0 { base - add } else { base + add };
        let t = c.body.command(AiCommand::TurnTo { goal: AiGoal::Point(b.position + yaw_dir(yaw) * 100.0) });
        if c.body.command_status(t) != AiCommandStatus::Failed {
            m.ticket = t;
            m.flag = 1;
        }
        None
    }
}
impl TaskNode for QuickTurnIdling {
    fn execute(&self, c: &mut Ctx, m: &mut Mem) -> Res {
        let cooling = m.i[0] != 0 && (c.st.time as f32 - m.f[0]) < self.cooldown;
        m.flag = 0;
        if cooling {
            m.ticket = c.body.command(AiCommand::Idle { time: IDLE_UNTIL_FINISHED });
            m.flag = 2;
            return Res::InProgress;
        }
        m.i[0] = 0;
        m.f[1] = 2.0;
        m.f[2] = c.st.rng.frand() * (2.0 * self.random) - self.random;
        self.update(c, m).unwrap_or(Res::InProgress)
    }
    fn tick(&self, c: &mut Ctx, m: &mut Mem, dt: f32) -> Option<Res> {
        m.f[1] -= dt;
        if m.flag & 1 != 0 {
            let r = poll(c, m.ticket);
            if r == Some(Res::Succeeded) && self.cooldown > 0.0 {
                // 0x1404B4260: TurnFinished starts the cooldown
                m.i[0] = 1;
                m.f[0] = c.st.time as f32;
            }
            return r;
        }
        if m.flag & 2 != 0 {
            return poll(c, m.ticket);
        }
        self.update(c, m)
    }
}

/// `TresBTTask_Blackboard*Modifier` (Bool 0x140494FD0, Class 0x140495060, Object 0x140495720,
/// Vector 0x140495920) [D: sets the key to the node's value; Object without a value clears it].
#[derive(Debug)]
struct BbModifier {
    key: Option<String>,
    value: BbValue,
    vector_from: Option<String>,
}
impl TaskNode for BbModifier {
    fn execute(&self, c: &mut Ctx, _m: &mut Mem) -> Res {
        let v = match &self.vector_from {
            Some(_) => match key_pos(c, &self.vector_from) {
                Some(p) => BbValue::Vector(p),
                None => BbValue::Vector(Vec3::ZERO),
            },
            None => self.value.clone(),
        };
        set_key(c, &self.key, v);
        Res::Succeeded
    }
    fn describe(&self) -> String {
        format!("{:?} = {:?}", self.key, self.value)
    }
}

/// `BTTask_RunEQSQuery` (Execute 0x143C5BB80) [U+C]: runs the query, the result arrives as the
/// game's asynchronous callback would (next tick): Succeeded with an item, Failed without.
#[derive(Debug)]
struct RunEqsTask {
    query: Option<String>,
    query_key: Option<String>,
    result: Option<String>,
    params: Params,
}
impl TaskNode for RunEqsTask {
    fn execute(&self, c: &mut Ctx, m: &mut Mem) -> Res {
        let q = self.query.clone().or_else(|| match key_obj(c, &self.query_key) {
            BbObject::Asset(a) => Some(a),
            _ => None,
        });
        let Some(q) = q else { return Res::Failed };
        let qr = querier(c);
        let r = eqs::run_apply(c, &q, &qr, &self.params);
        match r.best() {
            Some(it) => {
                m.flag = 1;
                m.v = it.pos;
                m.actor = it.actor;
                m.i[0] = it.comp as i32;
            }
            None => m.flag = 2,
        }
        Res::InProgress
    }
    fn tick(&self, c: &mut Ctx, m: &mut Mem, _dt: f32) -> Option<Res> {
        if m.flag == 1 {
            let v = item_value(c, &self.result, m.v, m.actor, m.i[0] as u8);
            set_key(c, &self.result, v);
            Some(Res::Succeeded)
        } else {
            Some(Res::Failed)
        }
    }
}

fn item_value(c: &Ctx, k: &Option<String>, pos: Vec3, actor: ActorRef, comp: u8) -> BbValue {
    match key(c, k).and_then(|id| c.bb.key_type(id)) {
        Some(crate::bb::KeyType::Vector) => BbValue::Vector(pos),
        _ if comp > 0 => BbValue::Object(BbObject::Component(actor, comp)),
        _ if actor.is_none() => BbValue::Object(BbObject::None),
        _ => BbValue::Object(BbObject::Actor(actor)),
    }
}

pub fn querier(c: &Ctx) -> Querier {
    let b = c.body.view();
    Querier { pos: b.position, yaw: b.yaw, radius: b.radius, half_height: b.half_height, actor: self_ref(c), member: c.st.member }
}

fn query_params(p: &Props) -> Params {
    let req = pobj(p, "EQSRequest");
    let cfg = req.and_then(|r| r.get("QueryConfig")).and_then(|v| v.as_array()).cloned().unwrap_or_default();
    cfg.iter()
        .filter_map(|x| x.as_object())
        .map(|o| (o.get("ParamName").and_then(|v| v.as_str()).unwrap_or("").to_string(), o.get("Value").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32))
        .collect()
}

/// `FEQSParametrizedQueryExecutionRequest`: the template, unless `bUseBBKeyForQueryTemplate`
/// names a blackboard key holding the query [U].
fn query_template(p: &Props) -> Option<String> {
    let r = pobj(p, "EQSRequest")?;
    if r.get("bUseBBKeyForQueryTemplate").and_then(|v| v.as_bool()).unwrap_or(false) {
        return None;
    }
    r.get("QueryTemplate").and_then(|v| v.as_str()).map(|s| crate::pack::asset_name(s).to_string())
}

fn query_bb_key(p: &Props) -> Option<String> {
    let r = pobj(p, "EQSRequest")?;
    if !r.get("bUseBBKeyForQueryTemplate").and_then(|v| v.as_bool()).unwrap_or(false) {
        return None;
    }
    r.get("EQSQueryBlackboardKey").and_then(|v| v.as_object()).and_then(|o| o.get("SelectedKeyName")).and_then(|v| v.as_str()).map(String::from)
}

/// `TresBTTask_FindAttackLocation` (Execute 0x140497130, finished 0x1404A20B0, Abort 0x140492F80;
/// ctor 0x140491190) [C4]: the attack definition = `AttackDefinitionOverride` unless
/// `bUseBlackboardQuery` (ctor: on) -> the BB class key `AttackDefinition`; not a
/// `UTresAttackDefinitionBase` -> Failed; its CDO without `m_ActionEQSQuery` (+0x108) -> Result =
/// the pawn's location, Succeeded; else the query runs with the CDO's parameters (vt+0x480
/// 0x140438C90 with the pawn and the controller's TargetActor, `validate::attack_query_params`)
/// in `RunMode` and the task waits: the first item -> Result, Succeeded; none -> Failed (the
/// result reaches the tree one tick later, as the callback does); a request id < 0 -> Failed.
#[derive(Debug)]
struct FindAttackLocation {
    def_key: Option<String>,
    def: Option<String>,
    use_bb: bool,
    result: Option<String>,
}
impl TaskNode for FindAttackLocation {
    fn execute(&self, c: &mut Ctx, m: &mut Mem) -> Res {
        match attack_query(c, &self.def_key, &self.def, self.use_bb) {
            AttackQuery::NotAttack => Res::Failed,
            AttackQuery::NoQuery => {
                let b = c.body.view();
                set_key(c, &self.result, BbValue::Vector(b.position + Vec3::Z * b.half_height));
                Res::Succeeded
            }
            AttackQuery::Query(q, params) => {
                let qr = querier(c);
                let r = eqs::run_apply(c, &q, &qr, &params);
                match r.best() {
                    Some(it) => {
                        m.flag = 1;
                        m.v = it.pos;
                    }
                    None => m.flag = 2,
                }
                Res::InProgress
            }
        }
    }
    fn tick(&self, c: &mut Ctx, m: &mut Mem, _dt: f32) -> Option<Res> {
        // the finished callback 0x1404A20B0: the first item stored (item type StoreInBlackboard)
        if m.flag == 1 {
            set_key(c, &self.result, BbValue::Vector(m.v));
            Some(Res::Succeeded)
        } else {
            Some(Res::Failed)
        }
    }
    fn abort(&self, _c: &mut Ctx, _m: &mut Mem) -> Res {
        // 0x140492F80: the query aborted
        Res::Aborted
    }
}

enum AttackQuery {
    NotAttack,
    NoQuery,
    Query(String, Params),
}

/// The definition, its `m_ActionEQSQuery` and its query parameters (task 0x140497130 / service
/// 0x14048ED70 [C4]).
fn attack_query(c: &Ctx, def_key: &Option<String>, def: &Option<String>, use_bb: bool) -> AttackQuery {
    let def = if use_bb { key(c, def_key).and_then(|k| c.bb.class(k)) } else { def.clone() };
    let Some(d) = def.and_then(|d| c.pack.class(&d)) else { return AttackQuery::NotAttack };
    let native = d.native.as_deref().unwrap_or("");
    if !validate::is_attack_definition(native) {
        return AttackQuery::NotAttack;
    }
    let Some(q) = passet(&d.p, "m_ActionEQSQuery") else { return AttackQuery::NoQuery };
    let check = validate::ActionCheck::from_class(native, &d.p);
    // vt+0x940 -> vt+0x970: the controller's blackboard TargetActor (key id +0x747)
    let target = c.bb.key_id("TargetActor").map(|k| c.bb.actor(k)).and_then(|a| view(c, a));
    AttackQuery::Query(q, check.attack_query_params(target.as_ref()))
}

/// Simple command tasks (SetBattleFollowMode 0x141224610) [D].
#[derive(Debug)]
struct SetBattleFollowMode(bool);
impl TaskNode for SetBattleFollowMode {
    fn execute(&self, c: &mut Ctx, _m: &mut Mem) -> Res {
        c.st.battle_follow_mode = self.0;
        c.body.command(AiCommand::SetBattleFollowMode(self.0));
        Res::Succeeded
    }
}

/// `TresNpcBTTask_EnableAttackFlowWait` (0x14120DA10) [D]: the attack-flow wait flag the
/// `CanAttackFlowWait` decorator reads.
#[derive(Debug)]
struct EnableAttackFlowWait(bool);
impl TaskNode for EnableAttackFlowWait {
    fn execute(&self, c: &mut Ctx, _m: &mut Mem) -> Res {
        // 0x141239090 [C]: on a change, +0x238 = on; switching on clears the phase +0x239
        if c.st.attack_flow_wait != self.0 {
            c.st.attack_flow_wait = self.0;
            if self.0 {
                c.st.attack_flow_phase = false;
                c.st.attack_flow_wait_since = c.st.time;
            }
        }
        Res::Succeeded
    }
}

/// `TresNpcBTTask_SelectSideMoveDirction` (Execute 0x141223C60) [C]: `SideMoveDirction` = -1 when
/// the normalised yaw difference is <= 0, else +1, where the difference is
/// - with `m_bAwayFromPlayer`: yaw(self - player) - self yaw (player = pawn vt+0x2190 ->
///   FUN_141264CB0),
/// - else: the reference yaw - yaw(target - self), target = the LockOnTargetComponent's location,
///   else TargetActor's; the reference = the event battle point direction when
///   `m_bUseEventBattlePoint` and it is >= `m_LimitLength` away (no event battle points in the
///   port), otherwise the pawn's own yaw.
#[derive(Debug)]
struct SelectSideMove(bool);
impl TaskNode for SelectSideMove {
    fn execute(&self, c: &mut Ctx, _m: &mut Mem) -> Res {
        let b = c.body.view();
        let me = b.position + Vec3::Z * b.half_height;
        let d = if self.0 {
            let pp = c.world.player().actor.center;
            validate::norm(validate::rotation(me - pp).1 - b.yaw)
        } else {
            let comp = c.bb.key_id("LockOnTargetComponent").map(|k| c.bb.actor(k)).unwrap_or_default();
            let tgt = c.bb.key_id("TargetActor").map(|k| c.bb.actor(k)).unwrap_or_default();
            let tp = view(c, comp).or_else(|| view(c, tgt)).map(|v| v.center).unwrap_or(Vec3::ZERO);
            let to = (tp - me).with_z(0.0);
            validate::norm(b.yaw - validate::rotation(to).1)
        };
        c.bb.set_by_name("SideMoveDirction", BbValue::Float(if d <= 0.0 { -1.0 } else { 1.0 }));
        Res::Succeeded
    }
}

/// `TresNpcBTTask_NpcUseItem` (Execute 0x1412234A0, Tick 0x14123C9D0) [C partly: uses the
/// MetaAI cure record's command (item kind) on its target]: the item from the record, then the
/// UseItem command.
#[derive(Debug)]
struct NpcUseItem;
impl TaskNode for NpcUseItem {
    fn execute(&self, c: &mut Ctx, m: &mut Mem) -> Res {
        let Some(info) = c.shared.cure_info.clone().filter(|i| i.curer == c.st.member) else { return Res::Failed };
        let item = info.command.trim_start_matches("TRES_CMD_KIND_");
        let held = c.body.view().items.iter().flatten().find(|i| **i == item).copied();
        // Execute 0x1412234A0 [C]: the item is used on the record's target (FUN_141274340) and the
        // record cleared (FUN_141231C40) at once
        c.shared.cure_info = None;
        let Some(item) = held else { return Res::Failed };
        m.ticket = c.body.command(AiCommand::UseItem { item, target: info.target });
        Res::InProgress
    }
    fn tick(&self, c: &mut Ctx, m: &mut Mem, _dt: f32) -> Option<Res> {
        poll(c, m.ticket)
    }
}

/// `TresNpcBTTask_SetCureTargetBB` (0x141224780) [D]: SupportTargetActor = the cure record's
/// target when this member is the curer.
#[derive(Debug)]
struct SetCureTargetTask;
impl TaskNode for SetCureTargetTask {
    fn execute(&self, c: &mut Ctx, _m: &mut Mem) -> Res {
        match c.shared.cure_info.clone().filter(|i| i.curer == c.st.member) {
            Some(i) => {
                c.bb.set_by_name("SupportTargetActor", BbValue::Object(BbObject::Actor(i.target)));
                Res::Succeeded
            }
            None => Res::Failed,
        }
    }
}

/// `TresNpcBTTask_GetCameraActor` (0x14120DB30) [D]: the camera has no actor in the port:
/// the key is cleared (only the photo-mode branch reads it).
#[derive(Debug)]
struct GetCameraActor(Option<String>);
impl TaskNode for GetCameraActor {
    fn execute(&self, c: &mut Ctx, _m: &mut Mem) -> Res {
        set_key(c, &self.0, BbValue::Object(BbObject::None));
        Res::Failed
    }
}

/// `TresNpcBTTask_SetCombinationPhase` (0x141224730) [D]: the MetaAI combination phase.
#[derive(Debug)]
struct SetCombinationPhase(i32);
impl TaskNode for SetCombinationPhase {
    fn execute(&self, c: &mut Ctx, _m: &mut Mem) -> Res {
        c.shared.combi_phase = self.0;
        Res::Succeeded
    }
}

/// Tasks of world-specific modes (lead, NPC free walk, ride Gigas): their conditions are false in
/// the test level (plan N5); they run as data and fail [D].
#[derive(Debug)]
struct WorldModeTask(String);
impl TaskNode for WorldModeTask {
    fn execute(&self, _c: &mut Ctx, _m: &mut Mem) -> Res {
        Res::Failed
    }
    fn describe(&self) -> String {
        format!("{} (world mode, fails outside its world)", self.0)
    }
}

/// A class without an implementation (listed by the reachability report; must stay empty).
#[derive(Debug)]
pub struct Missing(pub String);
impl TaskNode for Missing {
    fn execute(&self, _c: &mut Ctx, _m: &mut Mem) -> Res {
        Res::Failed
    }
    fn describe(&self) -> String {
        format!("MISSING {}", self.0)
    }
}
impl DecoratorNode for Missing {}
impl ServiceNode for Missing {}

// ---- decorators ------------------------------------------------------------------------------

/// A pure condition decorator.
struct Cond {
    f: Box<dyn Fn(&mut Ctx) -> bool + Send + Sync>,
    name: &'static str,
}
impl std::fmt::Debug for Cond {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name)
    }
}
impl DecoratorNode for Cond {
    fn calc(&self, c: &mut Ctx, _m: &mut Mem) -> bool {
        (self.f)(c)
    }
}

fn cond(name: &'static str, f: impl Fn(&mut Ctx) -> bool + Send + Sync + 'static) -> Box<dyn DecoratorNode> {
    Box::new(Cond { f: Box::new(f), name })
}

/// `TresBTDecorator_BoolPropertyCheck` (Calc 0x140462400, Tick 0x140476E30) [C]: the pawn's
/// function / property by name; ticks and requests execution when the result disagrees with
/// whether its branch executes.
#[derive(Debug)]
struct BoolPropertyCheck {
    name: String,
    inverse: bool,
}
impl DecoratorNode for BoolPropertyCheck {
    fn calc(&self, c: &mut Ctx, _m: &mut Mem) -> bool {
        call_fn(c, ActorRef::None, &self.name) != 0.0
    }
    fn ticks(&self) -> bool {
        true
    }
    fn tick(&self, c: &mut Ctx, m: &mut Mem, _dt: f32, executing: bool) -> bool {
        let v = self.calc(c, m) != self.inverse;
        v != executing
    }
}

/// `BTDecorator_Blackboard` (Calc 0x143C52FA0, key change 0x143C65530) [C+U].
#[derive(Debug)]
struct BbDecorator {
    key: Option<String>,
    arith: Option<(String, f32)>,
    not_set: bool,
}
impl BbDecorator {
    fn eval(&self, c: &Ctx) -> bool {
        let Some(id) = key(c, &self.key) else { return false };
        match (&self.arith, c.bb.get(id)) {
            (Some((op, v)), Some(BbValue::Float(_) | BbValue::Int(_) | BbValue::Enum(_))) => arith(op, c.bb.float(id), *v),
            (_, Some(BbValue::Vector(v))) => is_valid_location(*v) != self.not_set,
            (_, Some(BbValue::Object(o))) => !o.is_none() != self.not_set,
            (_, Some(BbValue::Class(cl))) => cl.is_some() != self.not_set,
            (_, Some(BbValue::Bool(b))) => *b != self.not_set,
            _ => false,
        }
    }
}
impl DecoratorNode for BbDecorator {
    fn calc(&self, c: &mut Ctx, _m: &mut Mem) -> bool {
        self.eval(c)
    }
    fn observed_key(&self) -> Option<&str> {
        self.key.as_deref()
    }
    fn on_key_change(&self, c: &mut Ctx, _m: &mut Mem, executing: bool, inverse: bool) -> bool {
        (self.eval(c) != inverse) != executing
    }
}

/// `BTDecorator_Cooldown` (KH3 fork: RandomDeviation; init 0x143C63470, Calc 0x143C53390,
/// Deactivation 0x143C66900, Tick 0x143C6D200) [C].
#[derive(Debug)]
struct Cooldown {
    time: f32,
    dev: f32,
}
impl Cooldown {
    fn init(&self, c: &mut Ctx, m: &mut Mem) {
        if !m.initialized {
            m.initialized = true;
            m.f[1] = -f32::MAX;
            m.f[0] = c.st.rng.deviate(self.time, self.dev);
        }
    }
}
impl DecoratorNode for Cooldown {
    fn calc(&self, c: &mut Ctx, m: &mut Mem) -> bool {
        self.init(c, m);
        m.f[0] <= c.st.time as f32 - m.f[1]
    }
    fn on_deactivation(&self, c: &mut Ctx, m: &mut Mem, _r: Res, _s: u32) -> DecoEffect {
        self.init(c, m);
        m.flag &= !1;
        m.f[1] = c.st.time as f32;
        m.f[0] = c.st.rng.deviate(self.time, self.dev);
        DecoEffect::default()
    }
    fn ticks(&self) -> bool {
        true
    }
    fn tick(&self, c: &mut Ctx, m: &mut Mem, _dt: f32, _e: bool) -> bool {
        self.init(c, m);
        if m.flag & 1 == 0 && m.f[0] <= c.st.time as f32 - m.f[1] {
            m.flag |= 1;
            return true;
        }
        false
    }
}

/// `BTDecorator_TagCooldown` (Calc 0x143C53DB0, Deactivation 0x143C66BC0, Tick 0x143C6D450)
/// and `BTDecorator_SetTagCooldown` (Deactivation 0x143C66AE0) [C].
#[derive(Debug)]
struct TagCooldown {
    tag: String,
    duration: f32,
    dev: f32,
    add: bool,
    activates: bool,
    check: bool,
}
fn tag_end(c: &Ctx, tag: &str) -> f64 {
    c.st.tag_cooldowns.get(tag).copied().unwrap_or(0.0)
}
impl DecoratorNode for TagCooldown {
    fn calc(&self, c: &mut Ctx, _m: &mut Mem) -> bool {
        if !self.check {
            return true;
        }
        let e = tag_end(c, &self.tag);
        e == 0.0 || e <= c.st.time
    }
    fn on_deactivation(&self, c: &mut Ctx, m: &mut Mem, _r: Res, _s: u32) -> DecoEffect {
        m.flag &= !1;
        if self.activates {
            let d = c.st.rng.deviate(self.duration, self.dev) as f64;
            let now = c.st.time;
            let cur = tag_end(c, &self.tag);
            let end = if self.add && cur > now { cur + d } else { now + d };
            c.st.tag_cooldowns.insert(self.tag.clone(), end);
        }
        DecoEffect::default()
    }
    fn ticks(&self) -> bool {
        self.check
    }
    fn tick(&self, c: &mut Ctx, m: &mut Mem, _dt: f32, _e: bool) -> bool {
        if m.flag & 1 == 0 {
            let e = tag_end(c, &self.tag);
            if e == 0.0 || e <= c.st.time {
                m.flag |= 1;
                return true;
            }
        }
        false
    }
}

/// `BTDecorator_TimeLimit` (KH3: Min/Max; Activation 0x143C66760) [C]: aborts its branch after a
/// random limit in [Min, Max].
#[derive(Debug)]
struct TimeLimit {
    min: f32,
    max: f32,
}
impl DecoratorNode for TimeLimit {
    fn on_activation(&self, c: &mut Ctx, m: &mut Mem, _s: u32, _fresh: bool) {
        m.next_tick = c.st.rng.frand() * (self.max - self.min) + self.min;
        m.accum = 0.0;
        m.flag &= !0x8000_0000;
    }
    fn tick_intervals(&self) -> bool {
        true
    }
    fn tick(&self, _c: &mut Ctx, _m: &mut Mem, _dt: f32, executing: bool) -> bool {
        executing
    }
}

/// `BTDecorator_Loop` (KH3: bTerminateIfChildFails, SearchId guard; Activation 0x143C666A0,
/// Deactivation 0x143C66A00) [C].
#[derive(Debug)]
struct Loop {
    infinite: bool,
    count: i32,
    timeout: f32,
    terminate_on_fail: bool,
}
impl DecoratorNode for Loop {
    fn on_activation(&self, c: &mut Ctx, m: &mut Mem, _s: u32, fresh: bool) {
        if fresh {
            m.i[0] = self.count;
            m.f[0] = c.st.time as f32;
        }
    }
    fn on_deactivation(&self, c: &mut Ctx, m: &mut Mem, r: Res, search_id: u32) -> DecoEffect {
        if self.terminate_on_fail && matches!(r, Res::Failed | Res::Aborted) {
            return DecoEffect::default();
        }
        let mut again = false;
        if self.infinite {
            if m.search_id != search_id && (self.timeout < 0.0 || m.f[0] + self.timeout > c.st.time as f32) {
                again = true;
            }
            m.search_id = search_id;
        } else {
            m.i[0] -= 1;
            again = m.i[0] > 0;
        }
        DecoEffect { repeat_child: again }
    }
}

/// `BTDecorator_ForceSuccess` (OnNodeProcessed 0x142F5F760) [C].
#[derive(Debug)]
struct ForceSuccess;
impl DecoratorNode for ForceSuccess {
    fn on_processed(&self, _c: &mut Ctx, _m: &mut Mem, r: &mut Res) {
        *r = Res::Succeeded;
    }
}

/// `TresBTDecorator_RangeCheck` / `TresNpcBTDecorator_NpcRangeCheck` (Calc 0x14047F760, distance
/// 0x14047D170 / 0x141205900, bounds 0x140481FE0) [C for the comparison, D for the distance
/// details: bounds subtract the capsule radii, the lock-on location option uses the target's
/// centre].
#[derive(Debug)]
struct RangeCheck {
    source: Option<String>,
    target: Option<String>,
    src_bounds: bool,
    dst_bounds: bool,
    mode: String,
    use_min: bool,
    use_max: bool,
    min: f32,
    max: f32,
    min_key: Option<String>,
    max_key: Option<String>,
    lockon: bool,
    simple: bool,
}
impl DecoratorNode for RangeCheck {
    fn calc(&self, c: &mut Ctx, _m: &mut Mem) -> bool {
        let sa = key_actor(c, &self.source);
        let ta = key_actor(c, &self.target);
        let (Some(sp), Some(mut tp)) = (key_pos(c, &self.source), key_pos(c, &self.target)) else { return false };
        if self.lockon {
            if let Some(v) = view(c, ta) {
                tp = v.center;
            }
        }
        let mut d = match self.mode.as_str() {
            "DirectDistance" => sp.distance(tp),
            "DirectDistanceZ" => (sp.z - tp.z).abs(),
            _ => (sp - tp).truncate().length(),
        };
        if !self.simple {
            if self.src_bounds {
                d -= view(c, sa).map(|v| v.radius).unwrap_or(0.0);
            }
            if self.dst_bounds {
                d -= view(c, ta).map(|v| v.radius).unwrap_or(0.0);
            }
        }
        let min = self.min_key.as_ref().and_then(|k| c.bb.key_id(k)).map(|k| c.bb.float(k)).unwrap_or(self.min);
        let max = self.max_key.as_ref().and_then(|k| c.bb.key_id(k)).map(|k| c.bb.float(k)).unwrap_or(self.max);
        !((self.use_min && d < min) || (self.use_max && max < d))
    }
}

/// `TresBTDecorator_AngleCheck` (Calc 0x1404622D0) [C]: `cos(Tolerance) <= dot(source facing,
/// direction to target)` (2D unless Use3D_Angle) [the dot helper 0x1404615D0: D].
#[derive(Debug)]
struct AngleCheck {
    source: Option<String>,
    target: Option<String>,
    tol: f32,
}
impl DecoratorNode for AngleCheck {
    fn calc(&self, c: &mut Ctx, _m: &mut Mem) -> bool {
        let sa = key_actor(c, &self.source);
        let (Some(sv), Some(tp)) = (view(c, sa), key_pos(c, &self.target)) else { return false };
        let to = (tp - sv.position).truncate().normalize_or_zero();
        let dot = yaw_dir(sv.yaw).truncate().dot(to);
        self.tol.to_radians().cos() <= dot
    }
}

/// `TresBTDecorator_ArithmeticComparison` (Calc 0x140462330) [C for the shape; operand helper
/// 0x14046E670: D].
#[derive(Debug)]
struct ArithmeticComparison {
    a: (String, Option<String>, Option<String>, f32),
    b: (String, Option<String>, Option<String>, f32),
    op: String,
}
/// An operand; a function with an actor parameter (IsHostileTarget) gets the other side's key
/// actor as its argument ("SelfActor.IsHostileTarget(TargetActor)") [D].
fn operand(c: &Ctx, o: &(String, Option<String>, Option<String>, f32), arg: &Option<String>) -> f32 {
    let (ty, k, prop, lit) = o;
    match ty.as_str() {
        "ActorFunction" | "ActorProperty" => {
            let name = prop.as_deref().unwrap_or("");
            if name == "IsHostileTarget" {
                let t = key_actor(c, arg);
                return view(c, t).map(|v| (v.hostile && v.alive) as i32 as f32).unwrap_or(0.0);
            }
            call_fn(c, key_actor(c, k), name)
        }
        "LiteralValue" => *lit,
        _ => key(c, k).map(|id| c.bb.float(id)).unwrap_or(0.0),
    }
}
impl DecoratorNode for ArithmeticComparison {
    fn calc(&self, c: &mut Ctx, _m: &mut Mem) -> bool {
        arith(&self.op, operand(c, &self.a, &self.b.1), operand(c, &self.b, &self.a.1))
    }
}

/// `TresBTDecorator_ValidateAction` (Calc 0x14047FB20) [C]: the definition from the BB class key
/// (`bUseBlackboardQuery`, ctor 0x140479B30 default on) or `ActionDefinitionOverride`; location /
/// orientation / execution checks of its CDO (`validate.rs`), each when its flag is on (ctor:
/// all three on).
#[derive(Debug)]
struct ValidateAction {
    target: Option<String>,
    def_key: Option<String>,
    def: Option<String>,
    use_bb: bool,
    location: bool,
    orientation: bool,
    execution: bool,
}

/// The decorator side of `validate::CheckWorld`.
struct CheckCtx<'a, 'b>(&'a Ctx<'b>);
impl validate::CheckWorld for CheckCtx<'_, '_> {
    fn in_viewport(&self, p: Vec3) -> bool {
        crate::controller::in_view(&self.0.world.camera(), p, 0.0)
    }
    fn direct_path(&self, from: Vec3, to: Vec3) -> bool {
        self.0.world.nav().path_exists(from, to)
    }
    fn line_of_sight(&self, from: Vec3, to: Vec3) -> bool {
        !self.0.world.nav().trace_blocked(from, to)
    }
    fn has_ability(&self, kind: &str) -> bool {
        // pawn vt+0x14F0 (kind, equipped): B answers "HasAbility:<ETresAbilityKind>" (C.2);
        // fallback: equipped
        self.0.body.function(&format!("HasAbility:{kind}")).map(|v| v != 0.0).unwrap_or(true)
    }
}

pub fn me_for_checks(c: &Ctx) -> validate::Me {
    let b = c.body.view();
    validate::Me {
        center: b.position + Vec3::Z * b.half_height,
        half_height: b.half_height,
        yaw: b.yaw,
        swimming: c.body.function("IsSwimming").map(|v| v != 0.0).unwrap_or(b.swimming),
        bad_status_bound: c.body.function("IsBadStatusBound").map(|v| v != 0.0).unwrap_or(false),
    }
}

impl DecoratorNode for ValidateAction {
    fn calc(&self, c: &mut Ctx, _m: &mut Mem) -> bool {
        let def = if self.use_bb { key(c, &self.def_key).and_then(|k| c.bb.class(k)) } else { self.def.clone() };
        let Some(d) = def.and_then(|d| c.pack.class(&d)) else { return false };
        let check = validate::ActionCheck::from_class(d.native.as_deref().unwrap_or(""), &d.p);
        // an object key that holds no actor passes a null target (0x14047FCEF); a vector key
        // (any value) runs the vector variants vt+0x3E0 / 0x3F0 / 0x400, which are 0x14041F1E0
        // (`mov al,1`) for every friend definition [C4]: all enabled checks pass
        if let Some(BbValue::Vector(_)) = key(c, &self.target).and_then(|k| c.bb.get(k).cloned()) {
            return true;
        }
        let target = view(c, key_actor(c, &self.target));
        let me = me_for_checks(c);
        let w = CheckCtx(c);
        (!self.location || check.location(&me, target.as_ref(), &w))
            && (!self.orientation || check.orientation(&me, target.as_ref()))
            && (!self.execution || check.execution(&me, target.as_ref(), &w))
    }
}

#[derive(Debug)]
struct RandomEntry {
    key: Option<String>,
    lit: f32,
}
impl DecoratorNode for RandomEntry {
    // Calc 0x14047F630 [D]: passes with probability = the key's float (or the literal)
    fn calc(&self, c: &mut Ctx, _m: &mut Mem) -> bool {
        let p = key(c, &self.key).map(|k| c.bb.float(k)).unwrap_or(self.lit);
        c.st.rng.frand() < p
    }
}

/// `TresNpcBTDecorator_CoverWaitCheck` (Calc 0x141206980) [C]: the player pawn is in
/// TSID_STATE_COVER (0x8E) and |player - TargetActor| - `m_AdjustLength` < |self - TargetActor|
/// (3D, root locations).
#[derive(Debug)]
struct CoverWaitCheck(f32);
impl DecoratorNode for CoverWaitCheck {
    fn calc(&self, c: &mut Ctx, _m: &mut Mem) -> bool {
        let p = c.world.player().actor;
        if p.state != "TSID_STATE_COVER" {
            return false;
        }
        let t = c.bb.key_id("TargetActor").map(|k| c.bb.actor(k)).unwrap_or_default();
        let Some(tv) = view(c, t) else { return false };
        let b = c.body.view();
        let me = b.position + Vec3::Z * b.half_height;
        p.center.distance(tv.center) - self.0 < me.distance(tv.center)
    }
}

/// `TresNpcBTDecorator_EnemyCountCheck` (Calc 0x141206CB0) [C]: the enemy manager's enemies
/// (lockable ones only with `m_bEnableLockOn`, minus `m_IgnoreEnemiesUID`) whose 2D distance to
/// the source minus both collision radii is <= `m_Radius` (all of them when `m_Radius` <= 0),
/// counted and compared `count <op> m_EnemyNum` (EArithmeticKeyOperation `m_ArithmeticOperation`).
#[derive(Debug)]
struct EnemyCountCheck {
    source: Option<String>,
    radius: f32,
    num: i32,
    op: String,
    ignore: Vec<i64>,
    lockon: bool,
}
impl DecoratorNode for EnemyCountCheck {
    fn calc(&self, c: &mut Ctx, _m: &mut Mem) -> bool {
        let sa = key_actor(c, &self.source);
        let Some(sv) = view(c, sa) else { return false };
        let mut en = Vec::new();
        c.world.enemies(&mut en);
        let n = en
            .iter()
            .filter(|e| e.alive && e.hostile && (!self.lockon || e.lockable) && !self.ignore.contains(&(e.uid as i64)))
            .filter(|e| self.radius <= 0.0 || (e.center - sv.center).truncate().length() - (e.radius + sv.radius) <= self.radius)
            .count() as i32;
        arith(&self.op, n as f32, self.num as f32)
    }
}

/// `TresBTDecorator_EntryLimit` (Calc 0x1404632B0, memory init 0x14046FA20, Deactivation
/// 0x1404727F0; ctor 0x14045DD70: m_MaxEntries 1) [C]: passes while the remaining-entries count
/// (set to m_MaxEntries when the memory is initialised, again on a subtree init with
/// `m_bInitSubtree`) is > 0; every finish of the branch (only successes with
/// `m_IncrementOnlyOnSuccess`) takes one. (With bInverseCondition the Calc itself takes one.)
#[derive(Debug)]
struct EntryLimit {
    max: i32,
    only_success: bool,
    init_subtree: bool,
    inverse: bool,
}
impl EntryLimit {
    fn init(&self, m: &mut Mem) {
        if !m.initialized {
            m.initialized = true;
            m.i[0] = self.max;
        }
    }
}
impl DecoratorNode for EntryLimit {
    fn calc(&self, _c: &mut Ctx, m: &mut Mem) -> bool {
        self.init(m);
        let n = m.i[0];
        if n > 0 && self.inverse {
            m.i[0] = n - 1;
        }
        n > 0
    }
    // m_bInitSubtree (0x14046FA20 init type 1 = RestoreSubtree) resets the count when a
    // RunBehavior subtree's memory is restored [kh-ai: node memory lives with the tree instance and
    // is created once, i.e. the Initialize case; `init_subtree` kept for the restore path]
    fn on_deactivation(&self, _c: &mut Ctx, m: &mut Mem, r: Res, _s: u32) -> DecoEffect {
        self.init(m);
        let _ = self.init_subtree;
        if !self.only_success || r == Res::Succeeded {
            m.i[0] = (m.i[0] - 1).max(0);
        }
        DecoEffect::default()
    }
}

/// `TresBTDecorator_Interrupt` (Calc 0x140464170; OnDamage / OnReflect / OnRevenge hooks
/// 0x140472CB0 / 0x140472CC0 / 0x140472CD0 set the memory's interrupted byte +0xC for the node's
/// `EventType`; ctor 0x14045E350: CustomAbortBehavior DenyOnNormalExecution) [C for the Calc]:
/// interrupted -> false only with DenyOnInterrupt (2); not interrupted -> false only with
/// DenyOnNormalExecution (1) (the same gate sits in KH3's WrappedCanExecute 0x143C49020 for
/// decorators flagged +0x78 bit 8, i.e. this class).
/// The hook's caller [C4]: `ATresCharPawnBase` damage 0x1405BB850 -> when the pawn's state side
/// (+0x450 vt+0x2C0) accepts the damage reaction, pawn vt+0x20F0 = `ATresAIPawnBase` 0x14054ACD0
/// -> (controller +0x604 < 1) BT component OnDamage 0x143C3DC20: every active aux node of every
/// instance gets aux slot 86 (0x143C49250) = slot 93 (this hook) then slot 92 0x143C3D510, which
/// with the byte set calls RequestExecution(decorator) 0x143C42790 unconditionally (a
/// LowerPriority branch already running restarts). The byte is cleared by KH3's
/// WrappedOnNodeDeactivation 0x143C491A0 (every decorator: memory +0xC = 0 when its branch
/// deactivates) and at memory init 0x143C39DA0. Port: the accepted damage reaction =
/// `AiBody::function("GetDamageReactionCount")` changing (B: a counter of accepted damage
/// reactions), fallback the pawn entering TSID_STATE_DAMAGE.
#[derive(Debug)]
struct Interrupt {
    on_damage: bool,
    behavior: u8,
}
impl Interrupt {
    fn eval(&self, m: &Mem) -> bool {
        if m.flag & 1 != 0 {
            self.behavior != 2
        } else {
            self.behavior != 1
        }
    }
}
impl DecoratorNode for Interrupt {
    fn calc(&self, _c: &mut Ctx, m: &mut Mem) -> bool {
        self.eval(m)
    }
    fn ticks(&self) -> bool {
        self.on_damage
    }
    fn tick(&self, c: &mut Ctx, m: &mut Mem, _dt: f32, _executing: bool) -> bool {
        let edge = match c.body.function("GetDamageReactionCount") {
            Some(n) => {
                // m.f[0]: the count seen last (flag bit 2: seen once)
                let seen = m.flag & 4 != 0;
                let changed = seen && n != m.f[0];
                m.f[0] = n;
                m.flag |= 4;
                changed
            }
            None => {
                let dmg = c.body.view().state == "TSID_STATE_DAMAGE";
                let edge = dmg && m.flag & 2 == 0;
                m.flag = (m.flag & !2) | if dmg { 2 } else { 0 };
                edge
            }
        };
        if edge {
            // hook 0x140472CB0 sets the byte; slot 92 0x143C3D510 requests execution
            m.flag |= 1;
            return true;
        }
        false
    }
    fn on_deactivation(&self, _c: &mut Ctx, m: &mut Mem, _r: Res, _s: u32) -> DecoEffect {
        m.flag &= !1;
        DecoEffect::default()
    }
}

/// `TresBTDecorator_FloorHeightCheck` (Calc 0x1404632D0, height 0x140461C60; ctor 0x14045DDC0:
/// TestHeight 5000, TestMode Navmesh, MinRange 0) [C]: the height of the source (an actor's
/// bounds origin, else the key's location) above the floor found within `TestHeight` below it
/// (FUN_140542040); no floor -> -100 -> false; else `height <Operation> MinRange`. The override
/// keys are written only with `m_bUseOverrideKey` (off in every friend tree).
#[derive(Debug)]
struct FloorHeightCheck {
    source: Option<String>,
    op: String,
    min: f32,
    test_height: f32,
}
impl DecoratorNode for FloorHeightCheck {
    fn calc(&self, c: &mut Ctx, _m: &mut Mem) -> bool {
        let a = key_actor(c, &self.source);
        let Some(p) = view(c, a).map(|v| v.center).or_else(|| key_pos(c, &self.source)) else { return false };
        let Some(f) = c.world.nav().project(p, 0.0, self.test_height) else { return false };
        arith(&self.op, p.distance(f), self.min)
    }
}

/// `TresBTDecorator_TimeMotionlessCheck` (value 0x14046EE10, base Calc 0x1404621A0 of
/// `TresBTDecorator_ActorValueCheckBase`) [C]: the BB actor's (m_BBActorName, SelfActor) movement
/// component +0xC68 (`ATresAIPawnBase::GetTimeMotionless` 0x141927B00) `<m_Operation>`
/// (`m_bUseBB` ? the key's float : `m_CheckValue`); the accumulator is the movement tick's
/// FUN_140591690 (C4: `controller::MotionTrack`, kept by kh-ai from the pawn's positions; the
/// friends' instances only test SelfActor).
#[derive(Debug)]
struct TimeMotionless {
    op: String,
    use_bb: bool,
    key: Option<String>,
    value: f32,
}
impl DecoratorNode for TimeMotionless {
    fn calc(&self, c: &mut Ctx, _m: &mut Mem) -> bool {
        let t = c.st.self_motion.time_motionless;
        let v = if self.use_bb { key(c, &self.key).map(|k| c.bb.float(k)).unwrap_or(0.0) } else { self.value };
        arith(&self.op, t, v)
    }
}

// ---- services --------------------------------------------------------------------------------

/// `BTService_RunEQS` (Tick 0x143C6D550) [U] / `TresBTService_RunEQSQuery` [C4] (TickNode
/// 0x1404A3FC0, finished 0x1404A1DF0, InitializeFromAsset 0x14049F310, OnCeaseRelevant 0x1404A1B70,
/// ctor 0x14048FFB0): on its interval, unless a request is still running, the query (the
/// `EQSQueryOverride`; `bUseBlackboardQuery` is set at init exactly when there is no override ->
/// the BB key `EQSQuery`'s asset) runs with `QueryParams` for the controller's pawn; the finished
/// callback stores the first item in `Result`, or with no item clears it when
/// `bInvalidateBlackboardKeyOnFailure` (ctor off). The result reaches the blackboard on the next
/// tick (the callback of the manager tick); aborted queries store nothing.
#[derive(Debug)]
struct RunEqsService {
    query: Option<String>,
    query_key: Option<String>,
    result: Option<String>,
    params: Params,
    invalidate_on_failure: bool,
}
impl ServiceNode for RunEqsService {
    fn tick(&self, c: &mut Ctx, _m: &mut Mem, _dt: f32) {
        let q = self.query.clone().or_else(|| match key_obj(c, &self.query_key) {
            BbObject::Asset(a) => Some(a),
            _ => None,
        });
        let Some(q) = q else { return };
        let qr = querier(c);
        let r = eqs::run_apply(c, &q, &qr, &self.params);
        let Some(k) = &self.result else { return };
        if let Some(it) = r.best() {
            let v = item_value(c, &self.result, it.pos, it.actor, it.comp);
            c.st.pending_bb.push((k.clone(), v));
        } else if self.invalidate_on_failure {
            if let Some(ty) = c.bb.key_id(k).and_then(|id| c.bb.key_type(id)) {
                c.st.pending_bb.push((k.clone(), crate::bb::default_value(ty)));
            }
        }
    }
}

/// `FEnvNamedValue` array (`QueryParams`: ParamName, ParamType, Value) of the Tres EQS service.
fn named_values(p: &Props, k: &str) -> Params {
    parr(p, k)
        .iter()
        .filter_map(|x| x.as_object())
        .map(|o| (o.get("ParamName").and_then(|v| v.as_str()).unwrap_or("").to_string(), o.get("Value").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32))
        .collect()
}

/// `TresBTService_GameplayFocus` (ctor: 0.2 s, call on search start; Tick 0x14048F280, cease
/// 0x14048BFB0) [C for the defaults, D for the body]: the controller focus follows the key.
#[derive(Debug)]
struct GameplayFocus(Option<String>);
impl ServiceNode for GameplayFocus {
    fn tick(&self, c: &mut Ctx, _m: &mut Mem, _dt: f32) {
        let a = key_actor(c, &self.0);
        if a != c.st.focus {
            c.st.focus = a;
            c.body.command(AiCommand::Focus(a));
        }
    }
    fn on_cease_relevant(&self, c: &mut Ctx, _m: &mut Mem) {
        c.st.focus = ActorRef::None;
        c.body.command(AiCommand::Focus(ActorRef::None));
    }
}

/// `TresNpcBTService_SetCureTargetBB` (Tick 0x14121B340) [C]: when the MetaAI cure record names
/// this member as the curer and SupportTargetActor is empty, SupportTargetActor = its target.
#[derive(Debug)]
struct SetCureTargetService;
impl ServiceNode for SetCureTargetService {
    fn tick(&self, c: &mut Ctx, _m: &mut Mem, _dt: f32) {
        if let Some(i) = c.shared.cure_info.clone() {
            if i.curer == c.st.member {
                let cur = c.bb.key_id("SupportTargetActor").map(|k| c.bb.actor(k)).unwrap_or_default();
                if cur.is_none() {
                    c.bb.set_by_name("SupportTargetActor", BbValue::Object(BbObject::Actor(i.target)));
                }
            }
        }
    }
}

/// `TresBTService_FindAttackLocation` [C4] (TickNode 0x14048ED70, finished 0x14048D580,
/// OnCeaseRelevant 0x14048BF60, ctor 0x14047A300: bUseBlackboardQuery on): on its interval,
/// unless its request still runs, the task's query (`attack_query`): a definition without
/// `m_ActionEQSQuery` sets Result = the pawn's location at once; the finished callback stores
/// the first item in Result, or with no item sets Result to the invalid location.
#[derive(Debug)]
struct FindAttackLocationService {
    def_key: Option<String>,
    def: Option<String>,
    use_bb: bool,
    result: Option<String>,
}
impl ServiceNode for FindAttackLocationService {
    fn tick(&self, c: &mut Ctx, _m: &mut Mem, _dt: f32) {
        let Some(k) = self.result.clone() else { return };
        match attack_query(c, &self.def_key, &self.def, self.use_bb) {
            AttackQuery::NotAttack => {}
            AttackQuery::NoQuery => {
                let b = c.body.view();
                c.bb.set_by_name(&k, BbValue::Vector(b.position + Vec3::Z * b.half_height));
            }
            AttackQuery::Query(q, params) => {
                let qr = querier(c);
                let r = eqs::run_apply(c, &q, &qr, &params);
                let v = r.best().map(|i| i.pos).unwrap_or(crate::bb::INVALID_LOCATION);
                c.st.pending_bb.push((k, BbValue::Vector(v)));
            }
        }
    }
}

/// `TresBTService_InterpretPlayerIntent` (Tick 0x14048F440, keys 0x14048A8B0) [C for the
/// formulas]: each tick
/// - CurrentDestinationPreference = NotMovingTimeCurve(own TimeMotionless, movement +0xC68)
///   + PlayerIntentWeightCurve(|the player movement's average direction +0xC4C, 2D|),
/// - CurrentLocationPreference = PlayerNotMovingTimeCurve(player TimeMotionless),
/// - FollowInFormationPreference = (dot(player forward, camera forward) + 1) / 2
///   * ForwardPreferencePlayerNotMovingCurve(player TimeMotionless),
/// - WaterSurfaceLocation only while both swim (movement mode 4) [not modelled: nobody swims].
/// The TimeMotionless / average-direction accumulators are the movement components'
/// FUN_140591690 (C4: `controller::MotionTrack` of the player and of this pawn).
#[derive(Debug)]
struct InterpretPlayerIntent {
    intent: Option<String>,
    not_moving: Option<String>,
    player_not_moving: Option<String>,
    forward: Option<String>,
}
impl ServiceNode for InterpretPlayerIntent {
    fn tick(&self, c: &mut Ctx, _m: &mut Mem, _dt: f32) {
        let ev = |name: &Option<String>, t: f32| name.as_ref().and_then(|n| c.pack.curves.get(n)).map(|cv| cv.eval(0, t)).unwrap_or(0.0);
        let p_still = c.st.player_motion.time_motionless;
        let me_still = c.st.self_motion.time_motionless;
        let avg = c.st.player_motion.avg_dir.truncate().length();
        let dest = ev(&self.not_moving, me_still) + ev(&self.intent, avg);
        let loc = ev(&self.player_not_moving, p_still);
        let p = c.world.player().actor;
        let cam = c.world.camera();
        let pf_ = yaw_dir(p.yaw);
        let (sp, cp) = cam.pitch.to_radians().sin_cos();
        let (sy, cy) = cam.yaw.to_radians().sin_cos();
        let cf = Vec3::new(cp * cy, cp * sy, sp);
        let form = (pf_.dot(cf) + 1.0) * 0.5 * ev(&self.forward, p_still);
        c.bb.set_by_name("CurrentDestinationPreference", BbValue::Float(dest));
        c.bb.set_by_name("CurrentLocationPreference", BbValue::Float(loc));
        c.bb.set_by_name("FollowInFormationPreference", BbValue::Float(form));
    }
}

#[derive(Debug)]
struct NoopService;
impl ServiceNode for NoopService {}

// ---- factories -------------------------------------------------------------------------------

pub fn make_task(class: &str, p: &Props, pack: &AiPack) -> (Box<dyn TaskNode>, bool) {
    let _ = pack;
    let t: Box<dyn TaskNode> = match class {
        "BTTask_Wait" => Box::new(Wait { time: pf(p, "WaitTime", 5.0), dev: pf(p, "RandomDeviation", 0.0) }),
        "TresBTTask_Noop" => Box::new(Noop),
        "TresBTTask_Action" | "TresNpcBTTask_NpcCureAction" => Box::new(Action {
            target: pkey(p, "m_Target"),
            use_bb_def: pb(p, "m_bUseBlackboardActionDefinition", false),
            bb_def: pkey(p, "m_BlackboardActionDefinition"),
            def: passet(p, "m_ActionDefinition"),
            abort_time: pf(p, "m_AbortTime", -1.0),
            cure: class == "TresNpcBTTask_NpcCureAction",
        }),
        // ctor 0x1404A5B20 defaults [C]
        "TresBTTask_MoveTo" | "TresNpcBTTask_NpcMoveTo" => Box::new(MoveTo {
            key: pkey(p, "BlackboardKey"),
            // the AI system's AcceptanceRadius (ctor reads +0x80 of the AISystem CDO; UE 5) [U]
            radius: pf(p, "m_AcceptableRadius", 5.0),
            use_bb_loco: pb(p, "m_bUseBBLocomotionDefinition", false),
            bb_loco: pkey(p, "m_BBLocomotionDefinitionKey"),
            loco: passet(p, "m_LocomotionDefinition"),
            walk: pb(p, "m_bWalk", false),
            follow_path: pb(p, "m_bUsePathFollowing", true),
            fail_on_damage: pb(p, "m_bFailOnDamageReaction", true),
            keep_moving: pf(p, "m_AbortMoveTimeToKeepMoving", 0.1),
            tether: pf(p, "m_GoalComponetLocationTetherDistance", 50.0),
        }),
        // ctor 0x1404A6680: Offset 0, RandomAngleLimit 0 [C]
        "TresBTTask_TurnTo" => Box::new(TurnTo { key: pkey(p, "BlackboardKey"), offset: pf(p, "Offset", 0.0), random: pf(p, "RandomAngleLimit", 0.0) }),
        "TresBTTask_Idle" => Box::new(Idle),
        // ctor 0x1404A5FC0: all four 0 [C]
        "TresBTTask_QuickTurnIdling" => Box::new(QuickTurnIdling {
            key: pkey(p, "BlackboardKey"),
            offset: pf(p, "Offset", 0.0),
            random: pf(p, "RandomAngleLimit", 0.0),
            limit: pf(p, "TurnLimit", 0.0),
            cooldown: pf(p, "CoolDownTimer", 0.0),
        }),
        "TresBTTask_BlackboardBoolValueModifier" => Box::new(BbModifier { key: pkey(p, "m_BlackboardKey"), value: BbValue::Bool(pb(p, "m_bValue", false)), vector_from: None }),
        "TresBTTask_BlackboardClassModifier" => Box::new(BbModifier { key: pkey(p, "m_BlackboardKey"), value: BbValue::Class(passet(p, "m_Value")), vector_from: None }),
        "TresBTTask_BlackboardObjectModifier" => Box::new(BbModifier {
            key: pkey(p, "m_BlackboardKey"),
            value: BbValue::Object(passet(p, "m_Value").map(BbObject::Asset).unwrap_or(BbObject::None)),
            vector_from: None,
        }),
        // Execute 0x140495920 [C]: m_bUseBlackboard (ctor 0x1404909A0: off) ? key B's vector /
        // actor location (vt+0x6B8) : m_Value; then the base modifier 0x1404958A0 (Set)
        "TresBTTask_BlackboardVectorValueModifier" => {
            let lit = pobj(p, "m_Value").map(|o| Vec3::new(o.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32, o.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32, o.get("z").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32)).unwrap_or(Vec3::ZERO);
            Box::new(BbModifier { key: pkey(p, "m_BlackboardKey"), value: BbValue::Vector(lit), vector_from: if pb(p, "m_bUseBlackboard", false) { pkey(p, "m_BlackboardKeyValueB") } else { None } })
        }
        "BTTask_RunEQSQuery" => Box::new(RunEqsTask { query: query_template(p), query_key: query_bb_key(p), result: pkey(p, "BlackboardKey"), params: query_params(p) }),
        "TresBTTask_FindAttackLocation" => Box::new(FindAttackLocation { def_key: pkey(p, "AttackDefinition"), def: passet(p, "AttackDefinitionOverride"), use_bb: pb(p, "bUseBlackboardQuery", true), result: pkey(p, "Result") }),
        "TresNpcBTTask_SetBattleFollowMode" => Box::new(SetBattleFollowMode(pb(p, "m_bFollowMode", false))),
        "TresNpcBTTask_EnableAttackFlowWait" => Box::new(EnableAttackFlowWait(pb(p, "m_bEnable", false))),
        "TresNpcBTTask_SelectSideMoveDirction" => Box::new(SelectSideMove(pb(p, "m_bAwayFromPlayer", false))),
        "TresNpcBTTask_NpcUseItem" => Box::new(NpcUseItem),
        "TresNpcBTTask_SetCureTargetBB" => Box::new(SetCureTargetTask),
        "TresNpcBTTask_GetCameraActor" => Box::new(GetCameraActor(pkey(p, "m_BlackboardValue"))),
        "TresNpcBTTask_SetCombinationPhase" => Box::new(SetCombinationPhase(pi(p, "m_PhaseIndex", 0))),
        "TresNpcBTTask_CalcLeadGoalTurnPointBB" | "TresNpcBTTask_CallLeadGoalTurnFinish" | "TresNpcBTTask_ReleaseFreeWalkPointUseFlagAll" | "TresNpcBTTask_SetFreeWalkAreaRadiusBB" | "TresNpcBTTask_SetFreeWalkLookAtLocationBB" | "TresNpcBTTask_SetFreeWalkMoveType" | "TresNpcBTTask_SetFreeWalkPointUseFlag" | "TresNpcBTTask_SetPlayerRideGigasBB" | "TresNpcBTTask_UpdateLeadPoint" => {
            Box::new(WorldModeTask(class.to_string()))
        }
        _ => return (Box::new(Missing(class.to_string())), false),
    };
    (t, true)
}

pub fn make_decorator(class: &str, p: &Props, pack: &AiPack) -> (Box<dyn DecoratorNode>, bool) {
    let d: Box<dyn DecoratorNode> = match class {
        "BTDecorator_Blackboard" => {
            // OperationType holds the key type's operation: EBasicKeyOperation (0 Set, 1 NotSet) for
            // object / vector / bool keys, EArithmeticKeyOperation for numbers [U; the packed
            // CachedDescription "... is Is Not Set" confirms 1 = NotSet]
            const OPS: [&str; 6] = ["Equal", "NotEqual", "Less", "LessOrEqual", "Greater", "GreaterOrEqual"];
            let op = pi(p, "OperationType", 0);
            let arith = Some((OPS[(op as usize).min(5)].to_string(), pf(p, "FloatValue", pi(p, "IntValue", 0) as f32)));
            let not_set = op == 1;
            Box::new(BbDecorator { key: pkey(p, "BlackboardKey"), arith, not_set })
        }
        "BTDecorator_Cooldown" => Box::new(Cooldown { time: pf(p, "CoolDownTime", 5.0), dev: pf(p, "RandomDeviation", 0.0) }),
        "BTDecorator_TagCooldown" | "BTDecorator_SetTagCooldown" => Box::new(TagCooldown {
            tag: ptag(p, "CooldownTag").unwrap_or_default(),
            duration: pf(p, "CooldownDuration", 5.0),
            dev: pf(p, "RandomDeviation", 0.0),
            add: pb(p, "bAddToExistingDuration", false),
            activates: pb(p, "bActivatesCooldown", true),
            check: class == "BTDecorator_TagCooldown",
        }),
        "BTDecorator_TimeLimit" => Box::new(TimeLimit { min: pf(p, "MinTimeLimit", 5.0), max: pf(p, "MaxTimeLimit", 5.0) }),
        "BTDecorator_Loop" => Box::new(Loop {
            infinite: pb(p, "bInfiniteLoop", false),
            count: pi(p, "NumLoops", 3),
            timeout: pf(p, "InfiniteLoopTimeoutTime", -1.0),
            terminate_on_fail: pb(p, "bTerminateIfChildFails", false),
        }),
        "BTDecorator_ForceSuccess" => Box::new(ForceSuccess),
        "TresBTDecorator_BoolPropertyCheck" => Box::new(BoolPropertyCheck { name: ps(p, "PropertyName").unwrap_or("").to_string(), inverse: pb(p, "bInverseCondition", false) }),
        "TresBTDecorator_ArithmeticComparison" => {
            let side = |s: &str, default_ty: &str| {
                (
                    ps(p, &format!("ValueType{s}")).map(enum_value).unwrap_or(default_ty).to_string(),
                    pkey(p, &format!("BlackboardKey{s}")),
                    ps(p, &format!("PropertyName{s}")).map(String::from),
                    pf(p, &format!("LiteralValue{s}"), 0.0),
                )
            };
            Box::new(ArithmeticComparison { a: side("A", "Blackboard"), b: side("B", "LiteralValue"), op: ps(p, "ArithmeticOperation").map(enum_value).unwrap_or("Equal").to_string() })
        }
        "TresBTDecorator_RangeCheck" | "TresNpcBTDecorator_NpcRangeCheck" => {
            let setting = ps(p, "m_RangeValueSetting").map(enum_value).unwrap_or("Range");
            let min_bb = pb(p, "m_bUseBlackboardMinRange", false) || ps(p, "m_MinRangeValueType").map(enum_value) == Some("Blackboard");
            let max_bb = pb(p, "m_bUseBlackboardMaxRange", false) || ps(p, "m_MaxRangeValueType").map(enum_value) == Some("Blackboard");
            Box::new(RangeCheck {
                source: pkey(p, "Source"),
                target: pkey(p, "Target"),
                src_bounds: pb(p, "UseDistanceFromSourceBounds", false),
                dst_bounds: pb(p, "UseDistanceToTargetBounds", false),
                mode: ps(p, "RangeMode").map(enum_value).unwrap_or("DirectDistance2D").to_string(),
                use_min: pb(p, "m_bUseMinRange", setting != "Maximum"),
                use_max: pb(p, "m_bUseMaxRange", setting != "Minimum"),
                min: pf(p, "MinRange", 0.0),
                max: pf(p, "MaxRange", 0.0),
                min_key: if min_bb { pkey(p, "m_BlackboardKeyMinRange") } else { None },
                max_key: if max_bb { pkey(p, "m_BlackboardKeyMaxRange") } else { None },
                lockon: pb(p, "m_bUseLockOnTargetLocation", false),
                simple: pb(p, "m_bSimpleCalculateRange", false),
            })
        }
        "TresBTDecorator_AngleCheck" => Box::new(AngleCheck { source: pkey(p, "Source"), target: pkey(p, "Target"), tol: pf(p, "Tolerance", 45.0) }),
        "TresBTDecorator_ValidateAction" => Box::new(ValidateAction {
            target: pkey(p, "Target"),
            def_key: pkey(p, "ActionDefinition"),
            def: passet(p, "ActionDefinitionOverride"),
            use_bb: pb(p, "bUseBlackboardQuery", true),
            location: pb(p, "bValidateLocation", true),
            orientation: pb(p, "bValidateOrientation", true),
            execution: pb(p, "bValidateExecution", true),
        }),
        "TresBTDecorator_RandomEntry" => Box::new(RandomEntry { key: pkey(p, "m_BlackboardKey"), lit: pf(p, "m_LiteralValue", 0.5) }),
        "TresNpcBTDecorator_CoverWaitCheck" => Box::new(CoverWaitCheck(pf(p, "m_AdjustLength", 300.0))),
        "TresNpcBTDecorator_EnemyCountCheck" => Box::new(EnemyCountCheck {
            source: pkey(p, "m_Source"),
            radius: pf(p, "m_Radius", 150.0),
            num: pi(p, "m_EnemyNum", 1),
            // EArithmeticKeyOperation enum value 0 = Equal when not serialised [U: ctor not read]
            op: ps(p, "m_ArithmeticOperation").map(enum_value).unwrap_or("Equal").to_string(),
            ignore: parr(p, "m_IgnoreEnemiesUID").iter().filter_map(|v| v.as_str()).map(|s| pack.enum_value("ETresEnemyUniqueID", s)).collect(),
            lockon: pb(p, "m_bEnableLockOn", false),
        }),
        "TresBTDecorator_EntryLimit" => Box::new(EntryLimit {
            max: pi(p, "m_MaxEntries", 1),
            only_success: pb(p, "m_IncrementOnlyOnSuccess", false),
            init_subtree: pb(p, "m_bInitSubtree", false),
            inverse: pb(p, "bInverseCondition", false),
        }),
        "TresBTDecorator_Interrupt" => Box::new(Interrupt {
            on_damage: matches!(ps(p, "EventType").map(enum_value), None | Some("IE_OnDamage")),
            behavior: match ps(p, "CustomAbortBehavior").map(enum_value) {
                Some("DenyOnInterrupt") => 2,
                Some("DenyOnNormalExecution") | None => 1,
                _ => 0,
            },
        }),
        "TresBTDecorator_FloorHeightCheck" => Box::new(FloorHeightCheck {
            source: pkey(p, "Source"),
            op: ps(p, "Operation").map(enum_value).unwrap_or("Equal").to_string(),
            min: pf(p, "MinRange", 0.0),
            test_height: pf(p, "TestHeight", 5000.0),
        }),
        "TresBTDecorator_TimeMotionlessCheck" => Box::new(TimeMotionless {
            op: ps(p, "m_Operation").map(enum_value).unwrap_or("Equal").to_string(),
            use_bb: pb(p, "m_bUseBB", false),
            key: pkey(p, "m_CheckValueBBKey"),
            value: pf(p, "m_CheckValue", 0.0),
        }),
        // 0x14047FA50 [C]: the source pawn's state id is one of m_States
        "TresBTDecorator_TresStateCheck" => {
            let src = pkey(p, "m_Source");
            let states: Vec<String> = parr(p, "m_States").iter().filter_map(|v| v.as_str().map(String::from)).collect();
            cond("TresStateCheck", move |c| {
                let a = key_actor(c, &src);
                let st = if a == self_ref(c) { c.body.view().state } else { view(c, a).map(|v| v.state).unwrap_or("") };
                !st.is_empty() && states.iter().any(|s| s == st) || (a == ActorRef::Player && st.is_empty() && states.iter().any(|s| s == "TSID_STATE_IDLE"))
            })
        }
        // 0x1412078C0 [C]: the source pawn's unique id is in m_EnemiesUID
        "TresNpcBTDecorator_IsEnemy" => {
            let src = pkey(p, "m_Source");
            let uids: Vec<i64> = parr(p, "m_EnemiesUID").iter().filter_map(|v| v.as_str()).map(|s| pack.enum_value("ETresEnemyUniqueID", s)).collect();
            cond("IsEnemy", move |c| view(c, key_actor(c, &src)).map(|v| uids.contains(&(v.uid as i64))).unwrap_or(false))
        }
        "TresBTDecorator_IsPlayerTeamCharacter" => {
            // 0x14047D600 [D]: the source's character uid is one of m_CharactorUID
            let src = pkey(p, "m_Source");
            let uids: Vec<i64> = parr(p, "m_CharactorUID").iter().filter_map(|v| v.as_str()).map(|s| pack.enum_value("ETresChrUniqueID", s)).collect();
            cond("IsPlayerTeamCharacter", move |c| view(c, key_actor(c, &src)).map(|v| uids.contains(&(v.uid as i64))).unwrap_or(false))
        }
        // 0x141208AD0 [C]
        "TresNpcBTDecorator_ObjectComparison" => {
            let (a, b, null) = (pkey(p, "m_SourceA"), pkey(p, "m_SourceB"), pb(p, "m_bNullCheck", false));
            cond("ObjectComparison", move |c| {
                let x = key_actor(c, &a);
                if null {
                    x.is_none()
                } else {
                    x == key_actor(c, &b)
                }
            })
        }
        // 0x1412063B0 [C]: the pawn's ability lock (vt+0x17E0) is off; the port reads it as "MP
        // is not recharging" [D]
        "TresNpcBTDecorator_CanExecuteNpcAbility" => cond("CanExecuteNpcAbility", |c| !c.body.view().mp_charging),
        // 0x141208260 [C]: TargetActor's resist kind for m_Attribute is in the list
        "TresNpcBTDecorator_IsTargetAttrResistKind" => {
            let attr = pack.enum_value("ETresDamageAttribute", ps(p, "m_Attribute").unwrap_or("")).max(0) as usize;
            let kinds: Vec<String> = parr(p, "m_AttrResistKinds").iter().filter_map(|v| v.as_str()).map(|s| enum_value(s).to_string()).collect();
            cond("IsTargetAttrResistKind", move |c| {
                let t = c.bb.key_id("TargetActor").map(|k| c.bb.actor(k)).unwrap_or_default();
                let Some(v) = view(c, t) else { return false };
                let r = v.resist.get(attr).copied().unwrap_or(1.0);
                // ETresAIAttrResistKind from the rate [D]: 0 Invalid, <1 High, >1 Low, else Normal
                let k = if r <= 0.0 { "Invalid" } else if r < 1.0 { "High" } else if r > 1.0 { "Low" } else { "Normal" };
                kinds.iter().any(|x| x == k)
            })
        }
        // 0x141205EF0 / 0x141205E60 [C]: the pawn's AI style == the node's
        "TresNpcBTDecorator_AIStyle_Battle" => {
            let want = match ps(p, "m_BattleStyle").map(enum_value) {
                Some("TRES_FNPC_AI_STYLE_BATTLE_FREE") => 1,
                Some("TRES_FNPC_AI_STYLE_BATTLE_SUPPORT") => 2,
                Some("TRES_FNPC_AI_STYLE_BATTLE_FOCUS") => 3,
                _ => 0,
            };
            cond("AIStyle_Battle", move |c| c.st.settings.battle as i32 == want)
        }
        "TresNpcBTDecorator_AIStyle_Ability" => {
            let want = match ps(p, "m_AbilityStyle").map(enum_value) {
                Some("TRES_FNPC_AI_STYLE_ABILITY_BALANCE") => 1,
                Some("TRES_FNPC_AI_STYLE_ABILITY_LESS") => 2,
                _ => 0,
            };
            cond("AIStyle_Ability", move |c| c.st.settings.ability as i32 == want)
        }
        "TresNpcBTDecorator_AIStyle_Cure" => {
            let want = match ps(p, "m_CureStyle").map(enum_value) {
                Some("TRES_FNPC_AI_STYLE_CURE_PINCH") => 1,
                Some("TRES_FNPC_AI_STYLE_CURE_ALWAYS") => 2,
                _ => 0,
            };
            cond("AIStyle_Cure", move |c| c.st.settings.cure as i32 == want)
        }
        // 0x141207870 [C]: the MetaAI cure record's command kind == m_Command (default NONE)
        "TresNpcBTDecorator_IsCureInfoCommand" => {
            let want = ps(p, "m_Command").map(enum_value).unwrap_or("TRES_CMD_KIND_NONE").to_string();
            cond("IsCureInfoCommand", move |c| c.shared.cure_info.as_ref().map(|i| i.command.as_str()).unwrap_or("TRES_CMD_KIND_NONE") == want)
        }
        // 0x1412062B0 -> 0x14121DAC0 [C]: the wait is on (+0x238) and the attack phase (+0x239,
        // the battle controller tick 0x14123CDA0: controller::attack_flow_tick) is not running
        "TresNpcBTDecorator_CanAttackFlowWait" => cond("CanAttackFlowWait", |c| c.st.attack_flow_wait && !c.st.attack_flow_phase),
        // [D] the player's lock-on target has lock-on components (any hostile lockable enemy)
        "TresNpcBTDecorator_ExistsLockonTargetComponents" => cond("ExistsLockonTargetComponents", |c| {
            let mut en = Vec::new();
            c.world.enemies(&mut en);
            en.iter().any(|e| e.alive && e.hostile && e.lockable)
        }),
        "TresNpcBTDecorator_IsFlyingEnemy" => {
            let src = pkey(p, "m_Source");
            cond("IsFlyingEnemy", move |c| view(c, key_actor(c, &src)).map(|v| v.flying).unwrap_or(false))
        }
        "TresNpcBTDecorator_IsPlayer" => {
            let src = pkey(p, "m_Source");
            cond("IsPlayer", move |c| key_actor(c, &src) == ActorRef::Player)
        }
        "TresBTDecorator_IsStopAI" => cond("IsStopAI", |c| c.st.stop_ai),
        "TresNpcBTDecorator_IsInHoldCameraTrickAction" => cond("IsInHoldCameraTrickAction", |c| c.world.player().hold_camera_trick),
        "TresNpcBTDecorator_IsCombiState" => {
            let w = match ps(p, "m_State").map(enum_value) {
                Some("STATE_COMBI_ENABLE") => 1,
                Some("STATE_COMBI_STANDBY") => 2,
                Some("STATE_COMBI_EXECUTE") => 3,
                _ => 0,
            };
            cond("IsCombiState", move |c| c.shared.combi_state == w)
        }
        "TresNpcBTDecorator_IsCombiPhase" => {
            let ph = pi(p, "m_PhaseIndex", 0);
            cond("IsCombiPhase", move |c| c.shared.combi_phase == ph)
        }
        // combinations need a combination partner (Mike, Snow ...): never in the test level
        "TresNpcBTDecorator_IsCombiRole" => cond("IsCombiRole", |_| false),
        // Calc 0x1404645F0 [C]: m_Target's location (vector key, or the actor's target location
        // vt+0x6E0) projects into the player's viewport (FUN_14443B030 + FUN_140E5AF80)
        "TresBTDecorator_IsInViewport" => {
            let k = pkey(p, "m_Target");
            cond("IsInViewport", move |c| {
                let a = key_actor(c, &k);
                let Some(pt) = view(c, a).map(|v| v.center).or_else(|| key_pos(c, &k)) else { return false };
                crate::controller::in_view(&c.world.camera(), pt, 0.0)
            })
        }
        // conditions of world gimmicks / NPC modes / photo mode: false in the test level (plan N5)
        "TresNpcBTDecorator_IsActionOn" | "TresNpcBTDecorator_IsAscensionMode_e_ex301" | "TresNpcBTDecorator_IsInnerWaterCurrent" | "TresNpcBTDecorator_IsPlayingFieldVoice" | "TresNpcBTDecorator_IsSmaphoTurnToCamera" | "TresNpcBTDecorator_CanSmaphoCameraAction" | "TresNpcBTDecorator_CanLeadGoalTurn" | "TresNpcBTDecorator_CanUseFreeWalkPoint" | "TresNpcBTDecorator_HasMyUseFreeWalkPoint" | "TresNpcBTDecorator_IsNpcModeMoveType" | "TresNpcBTDecorator_IsNpcModeType" => cond("world-mode condition (false)", |_| false),
        _ => return (Box::new(Missing(class.to_string())), false),
    };
    (d, true)
}

pub fn make_service(class: &str, p: &Props, _pack: &AiPack) -> (Box<dyn ServiceNode>, bool) {
    let s: Box<dyn ServiceNode> = match class {
        "BTService_RunEQS" => Box::new(RunEqsService { query: query_template(p), query_key: query_bb_key(p), result: pkey(p, "BlackboardKey"), params: query_params(p), invalidate_on_failure: false }),
        "TresBTService_RunEQSQuery" => {
            // InitializeFromAsset 0x14049F310: bUseBlackboardQuery = (EQSQueryOverride == null)
            let over = passet(p, "EQSQueryOverride");
            let use_bb = over.is_none();
            Box::new(RunEqsService {
                query: if use_bb { None } else { over },
                query_key: if use_bb { pkey(p, "EQSQuery") } else { None },
                result: pkey(p, "Result"),
                params: named_values(p, "QueryParams"),
                invalidate_on_failure: pb(p, "bInvalidateBlackboardKeyOnFailure", false),
            })
        }
        "TresBTService_GameplayFocus" => Box::new(GameplayFocus(pkey(p, "BlackboardKey"))),
        "TresNpcBTService_SetCureTargetBB" => Box::new(SetCureTargetService),
        "TresBTService_FindAttackLocation" => Box::new(FindAttackLocationService { def_key: pkey(p, "AttackDefinition"), def: passet(p, "AttackDefinitionOverride"), use_bb: pb(p, "bUseBlackboardQuery", true), result: pkey(p, "Result") }),
        "TresBTService_InterpretPlayerIntent" => Box::new(InterpretPlayerIntent {
            intent: passet(p, "PlayerIntentWeightCurve"),
            not_moving: passet(p, "NotMovingTimeCurve"),
            player_not_moving: passet(p, "PlayerNotMovingTimeCurve"),
            forward: passet(p, "ForwardPreferencePlayerNotMovingCurve"),
        }),
        "TresNpcBTService_UpdateLeadPoint" => Box::new(NoopService),
        _ => return (Box::new(Missing(class.to_string())), false),
    };
    (s, true)
}

/// Classes whose constructor sets bNotifyTick without overriding TickNode (constructors read:
/// `field_0x70 |= 5` / `|= 4` [C]; EntryLimit, Interrupt, RandomEntry and TimeMotionlessCheck do not
/// set it). Their FlowAbortMode is serviced by the fork's base TickNode 0x140476D90 [C: read by C2;
/// vtable slot 0x2D8 of every census class checked: 0x140476D90, EntryLimit / Interrupt /
/// RandomEntry 0x14041F120 (empty), BoolPropertyCheck 0x140476E30; TimeMotionlessCheck inherits
/// 0x140476D90 but its ctor 0x14045E570 leaves bNotifyTick off].
pub fn decorator_cond_tick(class: &str) -> bool {
    (class.starts_with("TresBTDecorator_") || class.starts_with("TresNpcBTDecorator_"))
        && !matches!(class, "TresBTDecorator_EntryLimit" | "TresBTDecorator_Interrupt" | "TresBTDecorator_RandomEntry" | "TresBTDecorator_TimeMotionlessCheck" | "TresBTDecorator_BoolPropertyCheck")
}

/// The decorator's flow-abort mode (asset value, else the class default).
pub fn decorator_abort_mode(class: &str, p: &Props) -> AbortMode {
    match ps(p, "FlowAbortMode") {
        Some(s) => AbortMode::parse(Some(s)),
        // BTDecorator_TimeLimit: Self (stock ctor) [U]
        None if class == "BTDecorator_TimeLimit" => AbortMode::SelfAbort,
        None => AbortMode::None,
    }
}

/// Class-default service interval: UE 0.5 s / 0.1 s [U]; GameplayFocus 0.2 / 0 (ctor) [C].
pub fn service_default_interval(class: &str) -> f32 {
    match class {
        "TresBTService_GameplayFocus" => 0.2,
        _ => 0.5,
    }
}

pub fn service_default_deviation(class: &str) -> f32 {
    match class {
        "TresBTService_GameplayFocus" => 0.0,
        _ => 0.1,
    }
}

/// Is the class's `pnative` context known (for the report).
pub fn _unused(p: &Props) -> Option<String> {
    pnative(p, "Context")
}
