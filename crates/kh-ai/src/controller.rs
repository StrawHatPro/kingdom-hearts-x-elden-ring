//! The friend AI controller (contract C.4): `ATresNpcControllerBase` : `ATresAIController_BT`
//! with its `UTresNpcAIBattleController` and the parts of `UTresNpcMetaAI` the trees reach.
//!
//! Per tick: the controller's own blackboard upkeep (possess-time keys 0x141274880 / 0x141275EA0
//! [C]; `bBattleMode` mirrored from the player 0x14127B660 [C]; TargetActor / TargetActorParent
//! from the LockOnTargetComponent key 0x14123EBD0 [C]; PlayerLockonActor / PlayerLockedonTime),
//! the MetaAI cure record (who cures whom with what: the MetaAI tick 0x14123E440 over each pawn's
//! decision 0x141264E30, transcribed in crate::cure [C]), the warp decision
//! (`TresNpcState_Warp`; trigger distance / time [?]), then the behaviour tree.

use crate::api::{ActorRef, AiBody, AiCommand, AiSettings, AiWorld, BattleStyle, CureStyle};
use crate::bb::{BbObject, BbValue, Blackboard};
use crate::bt::{BtComponent, Ctx, Library};
use crate::cure;
use crate::eqs;
use crate::pack::AiPack;
use crate::rng::Rng;
use std::collections::BTreeMap;
use std::sync::Arc;

pub use crate::cure::CureInfo;

/// State shared by the party's controllers.
#[derive(Debug, Default)]
pub struct AiShared {
    /// Attackers per target (coordinator), by target.
    pub attackers: BTreeMap<ActorRef, u8>,
    /// Each member's current move destination (EQS context `AllyDestinations`).
    pub destinations: BTreeMap<u8, glam::Vec3>,
    /// The MetaAI cure record (`UTresNpcMetaAI` +0x290..+0x2A8; crate::cure).
    pub cure_info: Option<CureInfo>,
    /// `ETresFNpcAICombiState` (0 DISABLE: no combination partner in the test level).
    pub combi_state: i32,
    pub combi_phase: i32,
    /// MetaAI tick 0x14123E440: the 2 s evaluation timer (+0x2AC), the world time it last ran,
    /// each member's latest record (`FUN_141264e30`) with its curer-target distance, and the
    /// random stream of its far pick.
    pub meta_timer: f32,
    pub meta_clock: f64,
    pub cure_candidates: BTreeMap<u8, (CureInfo, f32)>,
    pub meta_rng: Option<Rng>,
    /// (FOL2) the hallway round the leader as last measured (crate::formation; queries read it through `&Ctx`).
    pub corridor: std::sync::Mutex<Option<crate::formation::Corridor>>,
    /// (FOL2) each member's last random-walk spot and when it was picked (the EQS context `AllyActionDestinations`).
    pub action_dest: std::sync::Mutex<BTreeMap<u8, (glam::Vec3, f64)>>,
}

impl AiShared {
    pub fn new() -> AiShared {
        AiShared::default()
    }
}

/// The motion record a `UTresCharMovementComponent` keeps (FUN_140591690, called from its tick
/// 0x140598190 / FUN_14058c770) [C4]: v = (root location - last root location) / dt (kept when dt
/// <= 0); swimming at the surface (vt+0xC98 0x140589930) with no horizontal motion counts as v =
/// 0; a = dt / m_AverageDirectionInterpolationTime (+0xC34, ctor 0x140570EB0: 3.0), w = max(1 - a,
/// 0); "moving" = some |v component| > 0.0001; the average direction (+0xC4C) = w x avg + a x
/// unit(v) (0 when not moving), the average velocity (+0xC58) = w x avg + a x v; TimeMoving
/// (+0xC64) and TimeMotionless (+0xC68, `GetTimeMotionless` 0x141927B00) count the seconds of
/// each and reset the other.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MotionTrack {
    prev: Option<glam::Vec3>,
    velocity: glam::Vec3,
    pub avg_dir: glam::Vec3,
    pub avg_vel: glam::Vec3,
    pub time_moving: f32,
    pub time_motionless: f32,
}

impl MotionTrack {
    pub fn step(&mut self, pos: glam::Vec3, dt: f32, swim_surface: bool) {
        if let Some(prev) = self.prev {
            if dt > 0.0 {
                self.velocity = (pos - prev) / dt;
            }
        }
        self.prev = Some(pos);
        let mut v = self.velocity;
        if swim_surface && v.x * v.x + v.y * v.y <= 0.0 {
            v = glam::Vec3::ZERO;
        }
        let moving = v.x.abs() > 0.0001 || v.y.abs() > 0.0001 || v.z.abs() > 0.0001;
        let dir = if !moving {
            glam::Vec3::ZERO
        } else {
            let l2 = v.length_squared();
            if l2 == 1.0 {
                v
            } else if l2 >= 1e-8 {
                v / l2.sqrt()
            } else {
                glam::Vec3::ZERO
            }
        };
        let a = dt / 3.0;
        let w = (1.0 - a).max(0.0);
        self.avg_dir = self.avg_dir * w + dir * a;
        self.avg_vel = self.avg_vel * w + v * a;
        if moving {
            self.time_moving += dt;
            self.time_motionless = 0.0;
        } else {
            self.time_moving = 0.0;
            self.time_motionless += dt;
        }
    }
}

/// The player pawn's focus-break bit: +0x7C0 bit 0, read by vt+0x1170 0x140545D70, written by
/// vt+0x1160 0x14054E980 [C4]: SetFocusPoint 0x1405B94F0 sets it when the focus goes from >= 1 to
/// <= 0 (and the pawn's vt+0x1800 says so: true for the player pawns, 0x14041F1E0), the empty-all
/// 0x1405B95A0 sets it, and it is cleared when the focus is back at its max (0x1405B94F0, the
/// regeneration 0x1405BE820). kh-ai follows it from `AiWorld::player_focus`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FocusBreak {
    prev: Option<i32>,
    pub on: bool,
}

impl FocusBreak {
    pub fn observe(&mut self, focus: i32, max: i32) {
        if let Some(prev) = self.prev {
            if prev >= 1 && focus <= 0 {
                self.on = true;
            } else if self.on && focus >= max {
                self.on = false;
            }
        }
        self.prev = Some(focus);
    }
}

/// The controller's own state that nodes read and write.
#[derive(Debug)]
pub struct AiState {
    /// Seconds since the controller started (sum of dt; UE world time).
    pub time: f64,
    pub rng: Rng,
    pub settings: AiSettings,
    pub stop_ai: bool,
    pub member: u8,
    /// "n_ex001" / "n_ex002".
    pub pawn: String,
    /// Tag cooldowns: tag -> end time (`UBehaviorTreeComponent` cooldown tags).
    pub tag_cooldowns: BTreeMap<String, f64>,
    pub focus: ActorRef,
    pub battle_follow_mode: bool,
    pub attack_flow_wait: bool,
    pub attack_flow_wait_since: f64,
    /// Battle controller +0x239 (attack phase running) and +0x23C (its timer), 0x14123CDA0.
    pub attack_flow_phase: bool,
    pub attack_flow_timer: f32,
    /// Blackboard writes of asynchronous queries, applied at the start of the next tick.
    pub pending_bb: Vec<(String, BbValue)>,
    /// The player's and this pawn's movement-component motion record (FUN_140591690 [C4]):
    /// TimeMotionless (+0xC68) and the average direction (+0xC4C).
    pub player_motion: MotionTrack,
    pub self_motion: MotionTrack,
    /// The player's focus-break bit (`FocusBreak`).
    pub focus_break: FocusBreak,
    pub lock_time: f64,
    /// Warp check timer (+0xC98), the player's battle flag seen last, the 3 s window after it came on.
    pub warp_timer: f32,
    pub warp_battle_on: bool,
    pub warp_battle_window: f32,
    pub warp_ticket: u32,
    /// `TresBTTask_MoveTo` abort (0x1404A8700): the Stop that ends `m_AbortMoveTimeToKeepMoving`
    /// of continued motion (dropped when another command is issued first).
    pub keep_moving_until: Option<f64>,
    /// Random stream of the EQS tests that draw (RandomAngle: the global FRand in the game).
    pub eqs_rng: std::sync::Mutex<Rng>,
    /// The draw of the current EQS test run (RandomAngle's reference yaw), cleared per test.
    pub eqs_draw: std::sync::Mutex<Option<f32>>,
}

impl AiState {
    /// A fresh state (the controller's; tests build EQS / node contexts with it).
    pub fn new(pawn: &str, member: u8, seed: u64) -> AiState {
        AiState {
            time: 0.0,
            rng: Rng::new(seed),
            settings: AiSettings::default(),
            stop_ai: false,
            member,
            pawn: pawn.to_string(),
            tag_cooldowns: BTreeMap::new(),
            focus: ActorRef::None,
            battle_follow_mode: false,
            attack_flow_wait: false,
            attack_flow_wait_since: 0.0,
            attack_flow_phase: false,
            attack_flow_timer: 0.0,
            pending_bb: Vec::new(),
            player_motion: MotionTrack::default(),
            self_motion: MotionTrack::default(),
            focus_break: FocusBreak::default(),
            lock_time: 0.0,
            warp_timer: 0.0,
            warp_battle_on: false,
            warp_battle_window: 0.0,
            warp_ticket: 0,
            keep_moving_until: None,
            eqs_rng: std::sync::Mutex::new(Rng::new(seed ^ 0x45_51_53)),
            eqs_draw: std::sync::Mutex::new(None),
        }
    }
}

/// What F's `party-trace` word shows.
#[derive(Clone, Debug, Default)]
pub struct AiTrace {
    pub path: Vec<String>,
    pub task: String,
    pub style: BattleStyle,
    pub target: ActorRef,
    pub support: ActorRef,
    pub cure: Option<String>,
    pub log: Vec<String>,
}

/// Port parameters not read from the game [?] (recorded in `sheets/port/party-ai-nodes.json`).
pub mod params {
    /// Warp (0x141258790 / 0x14127B980) [C]: path length to the player that warps (1000 cm in the
    /// first 3 s of a battle), checked every 2 s.
    pub const WARP_DISTANCE: f32 = 3000.0;
    pub const WARP_CHECK_PERIOD: f32 = 2.0;
    /// MetaAI tick 0x14123E440 [C]: evaluation period (s) and the near distance (cm, XY).
    pub const CURE_EVAL_PERIOD: f32 = 2.0;
    pub const CURE_NEAR_DISTANCE: f32 = 2000.0;
    /// Donald's m_AppendCure (ctor 0x1411BA820 `+0xD00 = 0x78`) [C]; Goofy has none (vt+0x2920 -1).
    pub const DONALD_APPEND_CURE: i32 = 120;
    /// (party AIX, OUR DESIGN; K1 -> AIX request) Out of battle, no cure / item decision while the player moves faster
    /// than this (cm/s: Sora walks at most ~270, runs 680, sprints 980): KH3 casts a field Cure standing (CureFlow
    /// TurnTo -> NpcCureAction; MagicCast 1.2 s, no cast on the move, no deferral of its own) and its friends then
    /// fell 25 m behind a sprinting leader; the decision waits until the leader slows. In battle: KH3's rule.
    pub const FIELD_CURE_DEFER_SPEED: f32 = 300.0;
}

pub struct AiController {
    pack: Arc<AiPack>,
    bt: Option<BtComponent>,
    bb: Blackboard,
    st: AiState,
    started: bool,
}

impl AiController {
    pub fn new(pack: Arc<AiPack>, pawn: &str, member: u8, seed: u64) -> AiController {
        let lib = Arc::new(Library::compile(&pack));
        AiController::with_library(pack, lib, pawn, member, seed)
    }

    /// Several controllers can share one compiled library.
    pub fn with_library(pack: Arc<AiPack>, lib: Arc<Library>, pawn: &str, member: u8, seed: u64) -> AiController {
        let roots = pack.roots.get(pawn).cloned().unwrap_or_default();
        let bb_name = pack.trees.get(&roots.tree).and_then(|t| t.blackboard.clone()).unwrap_or_default();
        let bb = Blackboard::new(&pack, &bb_name);
        let mut bt = BtComponent::new(lib, &roots.tree);
        bt.dynamic = roots.dynamic.clone();
        AiController {
            pack,
            bt: Some(bt),
            bb,
            started: false,
            st: AiState::new(pawn, member, seed),
        }
    }

    pub fn settings(&self) -> AiSettings {
        self.st.settings
    }
    pub fn set_settings(&mut self, s: AiSettings) {
        self.st.settings = s;
    }
    /// IsStopAI (team attacks, events): the tree is restarted on every change so the root's
    /// IsStopAI gate (no abort mode) takes effect at once [D: KH3 stops / restarts the brain
    /// logic around such hand-overs].
    pub fn set_stop_ai(&mut self, stop: bool) {
        if self.st.stop_ai != stop {
            self.st.stop_ai = stop;
            if let Some(bt) = &mut self.bt {
                bt.start();
            }
        }
    }
    pub fn blackboard(&self) -> &Blackboard {
        &self.bb
    }
    pub fn state(&self) -> &AiState {
        &self.st
    }
    pub fn set_log(&mut self, on: bool) {
        if let Some(bt) = &mut self.bt {
            bt.log_enabled = on;
        }
    }
    /// (party AIX) The tree's log lines since the last call ("time text"; `set_log(true)` first): the scenario
    /// harness counts failed / aborted tasks and restarts with it.
    pub fn drain_log(&mut self) -> Vec<(f64, String)> {
        self.bt.as_mut().map(|bt| bt.log.drain(..).map(|l| (l.time, l.text)).collect()).unwrap_or_default()
    }

    /// Restart the root tree and clear the blackboard (View reset / respawn).
    pub fn reset(&mut self) {
        let name = self.bb.asset.clone();
        self.bb = Blackboard::new(&self.pack, &name);
        self.started = false;
        self.st.tag_cooldowns.clear();
        if let Some(bt) = &mut self.bt {
            bt.start();
        }
    }

    fn possess(&mut self, body: &dyn AiBody) {
        let roots = self.pack.roots.get(&self.st.pawn).cloned().unwrap_or_default();
        let me = ActorRef::Member(self.st.member);
        let bb = &mut self.bb;
        bb.set_by_name("SelfActor", BbValue::Object(BbObject::Actor(me)));
        bb.set_by_name("SpawnLocation", BbValue::Vector(body.view().position));
        bb.set_by_name("PlayerActor", BbValue::Object(BbObject::Actor(ActorRef::Player)));
        if let Some(q) = &roots.follow_query {
            bb.set_by_name("FollowPlayerEQSQuery", BbValue::Object(BbObject::Asset(q.clone())));
        }
        bb.set_by_name("WalkLocomotion", BbValue::Class(roots.walk.clone()));
        // ActiveLocomotion = the pawn's m_DefaultLocomotionDefinition [D: writer not found]
        bb.set_by_name("ActiveLocomotion", BbValue::Class(roots.locomotion.clone()));
        bb.set_by_name("PrecedeGridSize", BbValue::Float(2000.0));
        bb.set_by_name("PrecedeSpaceBetween", BbValue::Float(500.0));
        self.started = true;
    }

    fn upkeep(&mut self, dt: f32, body: &dyn AiBody, world: &dyn AiWorld) {
        for (k, v) in std::mem::take(&mut self.st.pending_bb) {
            self.bb.set_by_name(&k, v);
        }
        let p = world.player();
        self.bb.set_by_name("bBattleMode", BbValue::Bool(p.in_battle));
        // TargetActor from the lock-on component (0x14123EBD0)
        if let Some(k) = self.bb.key_id("LockOnTargetComponent") {
            let a = self.bb.actor(k);
            if !a.is_none() && world.actor(a).map(|v| v.alive).unwrap_or(false) {
                self.bb.set_by_name("TargetActor", BbValue::Object(BbObject::Actor(a)));
                self.bb.set_by_name("TargetActorParent", BbValue::Object(BbObject::Actor(a)));
            }
        }
        // a dead target is dropped; (FX5) so is an enemy the host no longer lists as hostile (a bystander: it stopped
        // threatening the party, kh-partyhost aiworld.rs `threatens`): the trees choose again among the enemies
        if let Some(k) = self.bb.key_id("TargetActor") {
            let a = self.bb.actor(k);
            let keep = |v: crate::api::ActorView| v.alive && (v.hostile || !matches!(a, ActorRef::Enemy(_)));
            if !a.is_none() && !world.actor(a).map(keep).unwrap_or(false) {
                self.bb.clear(k);
                self.bb.set_by_name("LockOnTargetComponent", BbValue::Object(BbObject::None));
            }
        }
        if !p.lock_on.is_none() {
            self.st.lock_time += dt as f64;
        } else {
            self.st.lock_time = 0.0;
        }
        self.bb.set_by_name("PlayerLockonActor", BbValue::Object(BbObject::Actor(p.lock_on)));
        self.bb.set_by_name("PlayerLockedonTime", BbValue::Float(self.st.lock_time as f32));
        // the movement components' motion records (FUN_140591690 [C4], every movement tick)
        self.st.player_motion.step(p.actor.position, dt, p.swimming);
        if let Some((f, max)) = world.player_focus() {
            self.st.focus_break.observe(f, max);
        }
        let bv = body.view();
        self.st.self_motion.step(bv.position, dt, bv.swimming);
    }

    /// The friend's side of the MetaAI cure record (crate::cure, 0x14123E440 / 0x141264E30 [C]):
    /// this member's own record every tick while there is none, the MetaAI's 2 s pick once per
    /// world step (the first controller to tick in it), and, while this member is the curer, the
    /// record's upkeep (dropped when no longer needed, when the Cure was cast, out of battle unless
    /// it is a Cure).
    fn cure_decision(&mut self, body: &dyn AiBody, world: &dyn AiWorld, shared: &mut AiShared) {
        let me = self.st.member;
        let bv = body.view();
        // (party AIX) the field deferral (params::FIELD_CURE_DEFER_SPEED): this member makes no record and drops one he
        // holds that was not cast yet (NpcCureAction / NpcUseItem clear it at their Execute), while the player runs out
        // of battle; the MetaAI pick waits with him
        let leader_speed = world.player().actor.velocity.truncate().length();
        if leader_speed > params::FIELD_CURE_DEFER_SPEED && !cure::in_battle(body, world) {
            if shared.cure_info.as_ref().is_some_and(|i| i.curer == me) {
                shared.cure_info = None;
            }
            shared.cure_candidates.remove(&me);
            return;
        }
        let curer = self.curer(body, world);
        if let Some(info) = shared.cure_info.clone() {
            if info.curer == me {
                let dying = bv.ko || bv.state == "TSID_STATE_DIE";
                let t = cure::target_stat(info.target, me, body, world);
                let cast = info.command == cure::CMD_CURE && bv.mp_charging && matches!(bv.state, "TSID_STATE_IDLE" | "TSID_STATE_TURN" | "TSID_STATE_AI_MOVE");
                let keep = !dying && t.map(|t| curer.still_needed(&info, &t)).unwrap_or(false) && !cast && (curer.in_battle || info.command == cure::CMD_CURE || info.command == cure::CMD_NONE);
                if !keep {
                    shared.cure_info = None;
                }
            }
            shared.cure_candidates.clear();
            return;
        }
        // FUN_141264e30 for this pawn (disabled / DIE pawns build none)
        let rec = if bv.ko || bv.state == "TSID_STATE_DIE" {
            None
        } else {
            let targets = cure::candidates(me, self.st.settings.cure_targets, body, world);
            curer.decide(&targets).map(|(i, command, score, flags)| {
                let t = targets[i];
                let d = (t.position - bv.position).truncate().length();
                (CureInfo { curer: me, target: t.actor, command, score, flags, since: self.st.time }, d)
            })
        };
        match rec {
            Some(r) => {
                shared.cure_candidates.insert(me, r);
            }
            None => {
                shared.cure_candidates.remove(&me);
            }
        }
        // the MetaAI tick, once per world step
        if self.st.time <= shared.meta_clock {
            return;
        }
        let dt = (self.st.time - shared.meta_clock) as f32;
        shared.meta_clock = self.st.time;
        shared.meta_timer += dt;
        if shared.meta_timer <= params::CURE_EVAL_PERIOD {
            return;
        }
        shared.meta_timer = 0.0;
        let mut near = params::CURE_NEAR_DISTANCE;
        let mut best: Option<CureInfo> = None;
        let mut far: Vec<CureInfo> = Vec::new();
        for (_, (info, d)) in &shared.cure_candidates {
            if near <= *d {
                far.push(info.clone());
            } else {
                near = *d;
                best = Some(info.clone());
            }
        }
        if best.is_none() && !far.is_empty() {
            let rng = shared.meta_rng.get_or_insert_with(|| Rng::new(0x4D45_5441));
            let i = ((rng.next_u32() >> 1) as usize) % far.len();
            best = Some(far[i].clone());
        }
        if let Some(mut b) = best {
            b.since = self.st.time;
            shared.cure_info = Some(b);
        }
    }

    /// The curer's side of the decision (style, battle, Cure, command list).
    fn curer<'a>(&'a self, body: &dyn AiBody, world: &dyn AiWorld) -> cure::Curer<'a> {
        let bv = body.view();
        // vt+0x2918: Donald 0x1411CA900 (magic not restricted, ability 0xCD usable: its cost is
        // all MP, so it is usable whenever MP is not recharging); Goofy: the base 0x14126C790 with
        // vt+0x2920 = -1 (never)
        let donald = self.st.pawn == "n_ex001";
        let can_cure = body.function("CanCastCure").map(|v| v != 0.0).unwrap_or(donald && !bv.mp_charging && bv.mp > 0);
        // FUN_141263cc0: vt+0x2920 (m_AppendCure) + max(1, magic) [magic from the body, 1 if unknown]
        let magic = body.function("GetMagic").map(|v| v as i32).unwrap_or(1).max(1);
        let cure_amount = if can_cure { params::DONALD_APPEND_CURE + magic } else { 0 };
        // FUN_1405ad320(pawn, 6) (restriction component vt+0x8C0): item commands restricted ->
        // the item list is emptied (0x141264E30); B answers "IsCommandRestricted:6" (fallback no)
        let items_off = body.function("IsCommandRestricted:6").map(|v| v != 0.0).unwrap_or(false);
        let mut commands: Vec<String> = if items_off { Vec::new() } else { bv.items.iter().flatten().map(|i| format!("TRES_CMD_KIND_{i}")).collect() };
        let item_up = body.function("GetAbilityNum:ITEM_UP").map(|v| v as i32).unwrap_or(0);
        if can_cure {
            commands.push(cure::CMD_CURE.to_string());
        }
        cure::Curer { pack: &self.pack, style: self.st.settings.cure, in_battle: cure::in_battle(body, world), can_cure, cure_amount, commands, item_up, focus_break: self.st.focus_break.on }
    }

    /// The warp to the player (`ATresNpcPawnBase` tick 0x14127B980 -> `FUN_141258790`, state
    /// TSID_STATE_WARP 125 = `UTresNpcState_Warp`) [C, read 2026-10-15 by stream C2]: checked every
    /// 2.0 s (+0xC98 counts down from 2.0; the warp state gets +0x88 = 2.0). It warps when
    /// - the pawn may (`FUN_141267fd0`: SetEnableWarp +0xB96, default on; in battle only when the
    ///   pawn lies outside +-120 deg of the camera's yaw), is alive (HP > 0), has no bad status, is
    ///   in IDLE / JUMP / HOPPING / AI_MOVE / NAVLINK_HOPPING / NAVLINK_JUMP (not in a rail-leave
    ///   jump; party I1; BURN only in a phase not modelled), the player is not in GLIDE / WALLRUN / HOPPING / RAIL_SLIDE /
    ///   RAILSLIDE_MI (flowmotion here), and the pawn's bounds are outside the player camera's view
    ///   frustum (`FUN_140e5ac70`: the camera manager POV planes, near 3 / far 2097152 cm, C4);
    /// - and: no floor within 5000 cm under the pawn (when it does not swim), or the player swims
    ///   and is >= D away (3D), or no floor under the player, or the nav path to the player is
    ///   >= D long (no path: infinite). D = 3000 cm, 1000 cm in the first 3 s of a battle
    ///   (`UTresFriendComponent` +0x314, set to 3.0 when the battle starts 0x14127BC10).
    /// The goal is the best point of the game's `EQSQuery_NpcWarp` (FriendData.WarpQuery) [A].
    fn warp_decision(&mut self, dt: f32, body: &mut dyn AiBody, world: &dyn AiWorld, shared: &mut AiShared) -> bool {
        let bv = body.view();
        let p = world.player();
        // UTresFriendComponent +0x312 / +0x314: the 3 s window after the battle starts
        if p.in_battle && !self.st.warp_battle_on {
            self.st.warp_battle_window = 3.0;
        }
        self.st.warp_battle_on = p.in_battle;
        self.st.warp_battle_window = (self.st.warp_battle_window - dt).clamp(0.0, 3.0);
        if self.st.warp_ticket != 0 {
            if body.command_status(self.st.warp_ticket) == crate::api::AiCommandStatus::Running {
                return true;
            }
            self.st.warp_ticket = 0;
            if let Some(bt) = &mut self.bt {
                bt.start();
            }
        }
        if bv.ko || self.st.stop_ai {
            return false;
        }
        self.st.warp_timer -= dt;
        if self.st.warp_timer > 0.0 {
            return false;
        }
        self.st.warp_timer = params::WARP_CHECK_PERIOD;
        if !self.warp_wanted(body, world) {
            return false;
        }
        let query = self.pack.data.get("FriendData").and_then(|d| crate::pack::passet(d, "WarpQuery")).unwrap_or_else(|| "EQSQuery_NpcWarp".into());
        let goal = {
            let c = Ctx { pack: &self.pack, bb: &mut self.bb, body, world, shared, st: &mut self.st };
            let r = eqs::run(&c, &query, &crate::nodes::querier(&c), &Vec::new());
            r.best().map(|i| i.pos)
        };
        let goal = goal.unwrap_or(eqs_fallback(world));
        let to = p.actor.position - goal;
        let yaw = to.y.atan2(to.x).to_degrees();
        self.st.warp_ticket = body.command(AiCommand::Warp { goal, yaw });
        true
    }

    /// (party B5) A state's own warp query (the NPC RailSlide's off-rail warp, 0x14124E890 -> FEnvQueryRequest
    /// 0x143C5A420 over `TresFriendData.WarpQuery` +0x60 (FUN_141260020), SingleResult, querier = the pawn, no
    /// query params; the delegate 0x141250A00 keeps the first item): the same query asset and querier as
    /// `warp_decision`, run when the body asks (`AiBody::wants_warp_query`); the answer goes back at once (the
    /// manager finishes the friends' small queries in the frame they start, C2 item 7).
    fn state_warp_query(&mut self, body: &mut dyn AiBody, world: &dyn AiWorld, shared: &mut AiShared) {
        if !body.wants_warp_query() {
            return;
        }
        let query = self.pack.data.get("FriendData").and_then(|d| crate::pack::passet(d, "WarpQuery")).unwrap_or_else(|| "EQSQuery_NpcWarp".into());
        let best = {
            let c = Ctx { pack: &self.pack, bb: &mut self.bb, body: &mut *body, world, shared, st: &mut self.st };
            let r = eqs::run(&c, &query, &crate::nodes::querier(&c), &Vec::new());
            r.best().map(|i| i.pos)
        };
        body.warp_query_result(best);
    }

    /// The attack phase of the attack-flow wait (`UTresNpcAIBattleController` tick 0x14123CDA0
    /// [C, read by C2]), while the wait is on (+0x238). N = this pawn, P = the player, T = the
    /// player's lock-on target, E = this friend's TargetActor (0x141228A30):
    /// - the player attacking (0x14126BC60: ATTACK / FIREMAGIC / ATTACK1..9 / COMBO_* /
    ///   COMMON_ATTACK) with a target T restarts the timer (+0x23C); with the phase off and N
    ///   within 1200 cm of P (XY, N != P) it starts the phase;
    /// - with the phase off it also starts when T exists and E is within 1000 cm of N (XY);
    /// - the phase (CanAttackFlowWait false) ends after 10 s, without T, when T is more than
    ///   1500 cm from P, or when N is more than 1200 cm from P.
    fn attack_flow_tick(&mut self, dt: f32, body: &dyn AiBody, world: &dyn AiWorld) {
        if !self.st.attack_flow_wait {
            return;
        }
        let n = body.view().position;
        let p = world.player();
        let pp = p.actor.position;
        let t = (!p.lock_on.is_none()).then(|| world.actor(p.lock_on)).flatten().map(|v| v.position);
        let e = self.bb.key_id("TargetActor").map(|k| self.bb.actor(k)).filter(|a| !a.is_none()).and_then(|a| world.actor(a)).map(|v| v.position);
        let d_np = n - pp;
        let near_player = d_np.truncate().length_squared() < 1200.0 * 1200.0;
        let mut start = false;
        if p.attacking {
            if t.is_some() {
                self.st.attack_flow_timer = 0.0;
                if !self.st.attack_flow_phase && d_np != glam::Vec3::ZERO && near_player {
                    start = true;
                }
            }
        }
        if !self.st.attack_flow_phase {
            let d_ne = e.map(|e| n - e).unwrap_or(glam::Vec3::ZERO);
            let enemy_near = t.is_some() && d_ne != glam::Vec3::ZERO && d_ne.truncate().length_squared() < 1000.0 * 1000.0;
            if !(start || enemy_near) {
                return;
            }
            // the phase starts and is checked in the same tick
            self.st.attack_flow_timer = 0.0;
            self.st.attack_flow_phase = true;
        }
        self.st.attack_flow_timer += dt;
        if self.st.attack_flow_timer < 10.0 {
            if let Some(t) = t {
                let far = (pp - t).truncate().length_squared() > 1500.0 * 1500.0;
                if d_np.truncate().length_squared() <= 1200.0 * 1200.0 && !far {
                    return;
                }
            }
        }
        self.st.attack_flow_phase = false;
        self.st.attack_flow_timer = 0.0;
    }

    /// `FUN_141267fd0` && `FUN_141258790` (see warp_decision).
    fn warp_wanted(&self, body: &dyn AiBody, world: &dyn AiWorld) -> bool {
        let bv = body.view();
        let p = world.player();
        let cam = world.camera();
        let enabled = body.function("IsEnableWarp").map(|v| v != 0.0).unwrap_or(true);
        if !enabled || bv.hp <= 0 || cure_bad_state(bv.state) {
            return false;
        }
        let center = bv.position + glam::Vec3::Z * bv.half_height;
        if bv.in_battle {
            let d = center - cam.position;
            let yaw = d.y.atan2(d.x).to_degrees();
            if angle_diff(yaw, cam.yaw).abs() < 120.0 {
                return false;
            }
        }
        if p.flowmotion || matches!(p.actor.state, "TSID_STATE_GLIDE" | "TSID_STATE_WALLRUN" | "TSID_STATE_HOPPING" | "TSID_STATE_RAIL_SLIDE" | "TSID_STATE_RAILSLIDE_MI") {
            return false;
        }
        // (party I1) the state switch of FUN_141258790 (state id - 1 at 0x141258882): IDLE 1, JUMP 3,
        // HOPPING 9, AI_MOVE 98, NAVLINK_HOPPING 107 pass; NAVLINK_JUMP 0x2B passes unless the state object
        // IsA UTresAICharState_Jump (UTresAICharState_NavLinkJumpCodeDriven is one: classes.json parents
        // JumpCodeDriven -> Jump) with byte +0xA9 set - only the rail leave 0x141252210 sets it (word
        // +0xA8 = 0x101 at 0x141252462; Init 0x14043E470 clears it): kh-party answers "IsRailLeaveJump";
        // BURN 124 passes only with UTresNpcState_BadStatBurn +0xAC set (written by the NPC hit handlers
        // 0x14126FBD0 / 0x141270520 on a projectile hit while burning; not modelled: BURN stays out);
        // every other state (CLIMBING 7, DANGLE 6, ...) fails (the switch default).
        let state_ok = match bv.state {
            "TSID_STATE_IDLE" | "TSID_STATE_JUMP" | "TSID_STATE_HOPPING" | "TSID_STATE_AI_MOVE" | "TSID_STATE_NAVLINK_HOPPING" => true,
            "TSID_STATE_NAVLINK_JUMP" => body.function("IsRailLeaveJump").map(|v| v == 0.0).unwrap_or(true),
            _ => false,
        };
        if !state_ok {
            return false;
        }
        // FUN_143e98810 bounds: the largest extent as the sphere radius
        // the frustum FUN_140e5ac70 -> FUN_14118f0a0 (C4): the object is the APlayerCameraManager
        // (POV location +0x7C0, rotation +0x7CC, aspect +0x7E8); vt+0x7F8 FOV (0x144432370: the
        // locked FOV else POV.FOV), vt+0x800 / 0x808 near / far = POV m_NearClipPlane /
        // m_FarClipPlane (0x144434580 / 0x144432390: +0x7EC / +0x7F0), filled from the view
        // target's UCameraComponent (ctor 0x143FA3B70: 3 / 2097152 cm)
        let far = world.camera_far().unwrap_or(CAMERA_FAR);
        if in_frustum(&cam, center, bv.radius.max(bv.half_height), CAMERA_NEAR, far) {
            return false;
        }
        let nav = world.nav();
        if !bv.swimming && nav.project(bv.position, 0.0, 5000.0).is_none() {
            return true;
        }
        let d = if self.st.warp_battle_window > 0.0 { 1000.0 } else { params::WARP_DISTANCE };
        if p.swimming {
            return bv.position.distance(p.actor.position) >= d;
        }
        if nav.project(p.actor.position, 0.0, 5000.0).is_none() {
            return true;
        }
        nav.path_length(bv.position, p.actor.position).unwrap_or(f32::MAX) >= d
    }

    pub fn tick(&mut self, dt: f32, body: &mut dyn AiBody, world: &dyn AiWorld, shared: &mut AiShared) {
        self.st.time += dt as f64;
        if !self.started {
            self.possess(body);
        }
        self.upkeep(dt, body, world);
        self.cure_decision(body, world, shared);
        self.attack_flow_tick(dt, body, world);
        self.state_warp_query(body, world, shared);
        if self.warp_decision(dt, body, world, shared) {
            return;
        }
        let Some(mut bt) = self.bt.take() else { return };
        let mut tracked = Tracking { inner: body, issued: false };
        {
            let mut c = Ctx { pack: &self.pack, bb: &mut self.bb, body: &mut tracked, world, shared, st: &mut self.st };
            bt.tick(&mut c, dt);
        }
        self.bt = Some(bt);
        // MoveTo abort 0x1404A8700: the pawn kept moving for m_AbortMoveTimeToKeepMoving; the
        // Stop lands then unless a newer command took over
        if tracked.issued {
            if self.st.keep_moving_until.map(|t| t > self.st.time).unwrap_or(false) {
                // a command issued in the same tick as the abort replaces the motion
                self.st.keep_moving_until = None;
            }
        }
        if let Some(t) = self.st.keep_moving_until {
            if t <= self.st.time {
                self.st.keep_moving_until = None;
                tracked.inner.command(AiCommand::Stop);
            }
        }
    }

    pub fn trace(&self) -> AiTrace {
        let mut t = AiTrace { style: self.st.settings.battle, ..Default::default() };
        t.target = self.bb.key_id("TargetActor").map(|k| self.bb.actor(k)).unwrap_or_default();
        t.support = self.bb.key_id("SupportTargetActor").map(|k| self.bb.actor(k)).unwrap_or_default();
        if let Some(bt) = &self.bt {
            t.path = bt.active_path();
            t.task = bt.active_task();
            t.log = bt.log.iter().rev().take(32).map(|l| format!("{:.2} {}", l.time, l.text)).collect();
        }
        t
    }

    /// The compiled library (reachability report).
    pub fn library(&self) -> Option<&Library> {
        self.bt.as_ref().map(|b| b.library())
    }

    pub fn cure_style(&self) -> CureStyle {
        self.st.settings.cure
    }
}

/// The body as the tree sees it: notes whether a command other than Focus was issued (the
/// delayed MoveTo-abort Stop is dropped then).
struct Tracking<'a> {
    inner: &'a mut dyn AiBody,
    issued: bool,
}
impl AiBody for Tracking<'_> {
    fn view(&self) -> crate::api::BodyView {
        self.inner.view()
    }
    fn function(&self, name: &str) -> Option<f32> {
        self.inner.function(name)
    }
    fn command(&mut self, cmd: AiCommand) -> u32 {
        if !matches!(cmd, AiCommand::Focus(_) | AiCommand::Stop) {
            self.issued = true;
        }
        self.inner.command(cmd)
    }
    fn command_status(&self, ticket: u32) -> crate::api::AiCommandStatus {
        self.inner.command_status(ticket)
    }
}

/// The bad-status states (`FUN_141269db0` / +0x85D stand-in from the state name).
fn cure_bad_state(state: &str) -> bool {
    matches!(state, "TSID_STATE_BADSTAT_STUN" | "TSID_STATE_BADSTAT_FREEZE" | "TSID_STATE_BADSTAT_BURN" | "TSID_STATE_BADSTAT_SNEEZE")
}

/// Degrees a - b normalised to (-180, 180].
fn angle_diff(a: f32, b: f32) -> f32 {
    let mut d = (a - b) % 360.0;
    if d > 180.0 {
        d -= 360.0;
    } else if d <= -180.0 {
        d += 360.0;
    }
    d
}

/// The camera's clip planes (UCameraComponent ctor 0x143FA3B70 [C4]: m_NearClipPlane 3,
/// m_FarClipPlane 2097152 cm; KH3's POV carries them to the camera manager).
pub const CAMERA_NEAR: f32 = 3.0;
pub const CAMERA_FAR: f32 = 2_097_152.0;

/// The sphere is inside the camera's frustum between `near` and `far` (`FUN_140e5ac70` ->
/// `FUN_14118f0a0` planes, `FUN_14118b470` sphere test).
pub fn in_frustum(cam: &crate::api::CameraView, center: glam::Vec3, radius: f32, near: f32, far: f32) -> bool {
    let (sp, cp) = cam.pitch.to_radians().sin_cos();
    let (sy, cy) = cam.yaw.to_radians().sin_cos();
    let fwd = glam::Vec3::new(cp * cy, cp * sy, sp);
    let x = (center - cam.position).dot(fwd);
    x - radius <= far && in_view_near(cam, center, radius, near)
}

/// The sphere is inside the camera's view frustum (side planes and a near plane of 10 cm; no far
/// plane): the viewport test of the decorators.
pub fn in_view(cam: &crate::api::CameraView, center: glam::Vec3, radius: f32) -> bool {
    in_view_near(cam, center, radius, 10.0)
}

fn in_view_near(cam: &crate::api::CameraView, center: glam::Vec3, radius: f32, near: f32) -> bool {
    let (sp, cp) = cam.pitch.to_radians().sin_cos();
    let (sy, cy) = cam.yaw.to_radians().sin_cos();
    let fwd = glam::Vec3::new(cp * cy, cp * sy, sp);
    let right = glam::Vec3::new(-sy, cy, 0.0);
    let up = right.cross(fwd).normalize_or_zero() * -1.0;
    let d = center - cam.position;
    let x = d.dot(fwd);
    if x + radius < near {
        return false;
    }
    let th = (cam.fov.max(1.0).to_radians() * 0.5).tan();
    let tv = th / cam.aspect.max(0.1);
    let y = d.dot(right).abs();
    let z = d.dot(up).abs();
    y <= x * th + radius * (1.0 + th * th).sqrt() && z <= x * tv + radius * (1.0 + tv * tv).sqrt()
}

fn eqs_fallback(world: &dyn AiWorld) -> glam::Vec3 {
    let p = world.player();
    let r = (p.actor.yaw + 180.0).to_radians();
    p.floor_position + glam::Vec3::new(r.cos(), r.sin(), 0.0) * 400.0
}
