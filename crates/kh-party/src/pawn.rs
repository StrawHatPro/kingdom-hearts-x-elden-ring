//! One party member: `ATresNpcPawn_n_ex00x` as the party steps it (contract B.3): command intake
//! -> movement with the pose tick and its notifies (kh-core's movement component, the same one
//! Sora runs) -> the state's MainProc.
//!
//! Every rule cites the function it was read from; departures are marked STAND-IN with the
//! reason (also listed in `build/sora_tmp/party/IMPL_B_STATE.md`).

use crate::actions::{self, ActionRun};
use crate::anim::{ClipNotifyData, NpcAnim};
use crate::command::{CommandStatus, CommandTicket, MemberCommand, MoveGoal, TargetRef};
use crate::host::PartyHost;
use crate::ids::{Chara, MemberId};
use crate::projectile::Projectile;
use crate::rng::Rng;
use crate::states::NpcStateId;
use crate::status::FriendStatus;
use crate::tables;
use crate::view::{AnimView, MemberView, PartyEventKind, SoraView};
use glam::{Vec2, Vec3};
use kh_core::cmc::Cmc;
use kh_core::host::{HitRecord, HitResult, TargetId, TargetInfo};
use kh_core::params::{SoraParams, SORA_PARAMS};
use kh_core::status::{compute_damage, DamageAttack, DamageContext};
use kh_react::{Body, BodyClass, CapsuleIn, EnemyReaction, HitOutcome, Phase, Pose, ReactionRequest};

/// Things a member asks of the party / host that cross members (heals, MP on Sora).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Request {
    Heal { target: TargetRef, amount: i32, effect: u8 },
    /// (party D5) The heal's effect on a member (Recover 0x1405B57D0 -> 0x1405B8C30(+0x440, 4, kind) on the healed
    /// pawn): `actions::recover_effect(kind)`, spawned by `Party::step` with that member's seam handles.
    HealEffect { member: MemberId, kind: u8 },
    /// (GOOFY-IT) A Recover kind's effect on "the player" (Sora, or the friend leading in his place): an item a member
    /// used on him. `Party::step` hands it to the host as a `SoraHeal` with no HP / MP and that kind, whose effect the
    /// host plays on whoever the player is (`Party::heal_effect_on_sora` / `heal_effect_on`).
    SoraEffect { kind: u8 },
    Mp { target: TargetRef, amount: i32 },
    /// (B4) An item on another member: ItemEffect with the user's ITEM_UP boost (the target halves it in Die).
    Item { target: TargetRef, item: crate::ItemKind, boost: f32 },
    Projectile(Projectile),
}

/// What the member knows of the others this step.
pub struct StepCtx<'a> {
    pub sora: &'a SoraView,
    /// (id, position) of every member, spawn order.
    pub members: &'a [(MemberId, Vec3, u8)],
    pub dt: f32,
    /// (B5) Sora's Blizzard ice-rail nodes (`Party::set_ice_rails`): rails for the friends' rail probe.
    pub ice_rails: &'a [kh_core::magic::RailNode],
    /// (ZETTA) Zettaflare's zones the AI members keep out of (`Party::don_zetta_zones`).
    pub zones: &'a [crate::donald::zettaflare::AvoidZone],
}

/// `UTresNpcState_Warp` (Init slot 95 0x141266C10, Begin 96 0x14125BAE0, Tick 97 0x14126DD90, End 98
/// 0x14125E7C0; the EQS callback 0x141274600; requested every 2 s by the pawn tick 0x14127B980, which sets
/// +0x88 = 2.0). Offsets of the state.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct WarpRun {
    /// m_nSubStep of the pose: 0 in the air (fall loop), 1 landing, 2 standing (idle clip).
    pub sub: u8,
    /// +0x88 the wait (2.0 from 0x14127B980); +0x8C the time limit (= +0x88 at Begin, 2 x +0x88 once the
    /// point is found); +0x98 the wait after the point; +0x9C the path re-check timer (Init: random 0..0.1).
    pub wait: f32,
    pub limit: f32,
    pub after_point: f32,
    pub recheck: f32,
    /// +0xA0: a path to the leader shorter than this ends the warp (1000 during the battle-start window of
    /// the friend component +0x314, else 3000).
    pub threshold: f32,
    /// +0x90 the point is found; +0xA4 the point (the EQS result: the command's goal, C runs the query).
    pub found: bool,
    pub point: Vec3,
    pub yaw: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
struct DieRun {
    timer: f32,
    sub: u8,
}

/// One member.
#[derive(Clone, Debug)]
pub struct NpcPawn {
    pub id: MemberId,
    pub chara: Chara,
    pub status: FriendStatus,
    pub params: SoraParams,
    pub cmc: Cmc,
    pub yaw: f32,
    pub anim: NpcAnim,
    pub state: NpcStateId,
    pub state_time: f32,
    cmd: Option<(CommandTicket, MemberCommand)>,
    cmd_new: bool,
    last: (CommandTicket, CommandStatus),
    history: Vec<(CommandTicket, CommandStatus)>,
    next_ticket: u32,
    pub action: Option<ActionRun>,
    pub(crate) react: EnemyReaction, // (GOOFY-HR) pub(crate): the shared hit-reaction path (hurt.rs) reads its clip
    die: DieRun,
    pub warp: WarpRun,
    /// The bad status on the pawn (ATresCharPawnBase +0x85D kind, +0x860 seconds left; ApplyBadStatus
    /// 0x1405B7C50, kinds of kh-core `combat::badstat::kind`), and the bad-status state's sub-step.
    pub bad: (u8, f32),
    pub bad_sub: u8,
    /// (party I1) movement +0xB70 bit 4 (vt+0x998, set by the jump tick 0x1404408A0 for a STUN-ed NPC): the movement
    /// ignores the overlapped forces in the sky (vt+0x990 0x140589250 false). No force volumes exist in the port.
    pub ignore_overlap_force: bool,
    /// (B4) Burn: the hops made and the yaw the air turn eases to (0x141248FC0 +0xA0).
    pub burn_hops: u8,
    pub burn_yaw: f32,
    /// (B4) An item used, applied after the delay (FUN_1405BD190 with 0.5 s -> vt+0x1200): (seconds left, item, target).
    pub item_pending: Option<(f32, crate::ItemKind, TargetRef)>,
    /// (B4, C4 relay) Accepted damage reactions so far (TakeDamage 0x1405BB850 -> the reaction gate -> vt+0x20F0):
    /// `Party::function("GetDamageReactionCount")`.
    pub damage_reactions: u32,
    /// (B4, C4 relay) UTresNpcState_Idle's IdleFinished timer (tick 0x14124CD40: in battle once per idle-clip length)
    /// and the anim clock's wrap of this tick (out of battle: the clip's end).
    idle_timer: f32,
    anim_wrapped: bool,
    /// A repelled weapon this step (the point it was repelled at): the Reflect state next.
    pub reflect_from: Option<Vec3>,
    reflect_dir: Vec3,
    /// Seconds the pawn has not moved (movement +0xC68, GetTimeMotionless).
    pub motionless: f32,
    /// (party P) the pad drive (stream P); None = AI-controlled.
    pub pad: Option<crate::pad::PadState>,
    pub in_battle: bool,
    /// Seconds since the battle flag came on, and the CanBattle latch (0x141257340, +0xCA8).
    pub battle_time: f32,
    pub can_battle_latch: bool,
    pub battle_follow: bool,
    pub rng: Rng,
    pub events: Vec<PartyEventKind>,
    pub requests: Vec<Request>,
    /// Targets touched by the open attack window (one hit per window per target).
    pub hit_list: Vec<TargetId>,
    pub hits_this_action: u32,
    pub(crate) scratch_targets: Vec<TargetInfo>,
    /// Stream D's drive of the FR state, applied at the next step.
    pub fr_drive: Option<crate::team::FrDrive>,
    pub fr_visible: bool,
    /// The controller focus (`MemberCommand::Focus`): Idle faces it.
    pub focus: TargetRef,
    /// The Turn state's clip run.
    pub turn: TurnRun,
    /// The direction index of the last reaction (damage_dir_index).
    pub dmg_index: u8,
    /// Stream E's handle counter of this member (E.2) and the voices to play at the next step (E.4).
    pub seam: kh_core::host::party::SeamHandles,
    pub voices: Vec<(kh_core::host::party::VoiceGroup, i32, kh_core::host::party::VoiceMode)>,
    /// E.3: the notify handler of this member's clips (what its notify states started) and its scratch events.
    pub notify_seam: kh_core::host::party::PartyNotify,
    notify_events: Vec<kh_core::host::party::PartyNotifyEvent>,
    /// (B4) the traversal / situation states' data (`traverse`).
    pub trav: crate::traverse::Traverse,
    /// (LADDER2) the ladder jump (port design, crate::ladder).
    pub ladder: crate::ladder::Leap,
    /// (party ER S1) THE HOST'S WAYPOINT (port design: the Elden Ring host's breadcrumb trail; KH3 steers by its navmesh
    /// path): `Some((goal, waypoint))` while the straight line to the running MoveTo's goal is blocked. In the Move state
    /// the member heads for `waypoint` instead of the goal while his goal is still within `STEER_GOAL_SLACK` of `goal`;
    /// the acceptance radius stays on the real goal. None (every host but kh-er) = the straight line, as before.
    pub steer: Option<(Vec3, Vec3)>,
    /// (party K1) the AI locomotion (crate::locomotion): follow speed, start / loop / stop, blocked guard, idle turn.
    pub loco: crate::locomotion::AiLoco,
    /// (GOOFY-HR) the shared hit-reaction path (crate::hurt): a lethal blow, the hit effect, the recovery gate.
    pub hurt: crate::hurt::HurtRun,
    /// (DON-MAGIC) Donald's cast under way (crate::donald::cast): its clip, shot and end; the interruptions.
    pub don_cast: crate::donald::cast::PawnCast,
    /// (ZETTA) The face-clip slot (crate::face): a face clip over the body pose (Zettaflare's lip track; the FACE stream's).
    pub face: crate::face::FaceSlot,
    /// (ZETTA) Zettaflare / the ultimate casts' side of the pawn (donald/zettaflare.rs: the ultimate's request, the hold).
    pub don_zetta: crate::donald::zettaflare::PawnZetta,
    /// (DON-KIT) Donald's Ward Step barrier (pad/donald_shots.rs): the barrier (its number) that will absorb this member's
    /// next hit, set every party step while he stands in one; the last barrier that absorbed one (once per member each).
    pub don_ward_offer: u32,
    pub don_ward_used: u32,
    /// (DON-KIT) What Donald's party auras give this member this step (donald/aura.rs `PartyAura::at`, set by
    /// pad/donald.rs `aura_to_members` before the members step): Thunder's movement, Aero's evasion / recovery.
    pub don_aura: crate::donald::aura::AuraEffect,
}

/// (party ER S1) How far the running goal may have moved from the one the host's waypoint was made for (cm).
pub const STEER_GOAL_SLACK: f32 = 250.0;

/// The movement component's parameters for an NPC: Sora's set with the pawn's own root capsule
/// (TresRoot CollisionShapesSrc[0], built by 0x1405DF310 as Sora's: Donald r 30 / half height 49 / centre 51,
/// Goofy 40 / 89 / 91),
/// gravity scale and walk speed (`n_ex00x_PawnBase` class defaults, sheet party-params). The
/// remaining movement constants of `TresAICharMovementComponent` are the shared UE / Tres
/// component's (same class chain as Sora's `TresCharMovementComponent`).
pub fn npc_params(chara: Chara) -> SoraParams {
    let r = tables::cparam(chara, "capsule.radius");
    let hh = tables::cparam(chara, "capsule.half_height");
    // The jump-phase gravity of the NPC movement component: UTresCharMovementComponent ctor 0x140570EB0
    // (m_JumpUpGravityScale 1, m_MaxJumpDurationTime 0, m_JumpDurationGravityScale 1), not changed by
    // UTresAICharMovementComponent 0x140512380 nor n_ex00x_PawnBase (Sora's values come from his BP).
    SoraParams {
        capsule_radius: r,
        capsule_half_height: hh,
        capsule_center_z: tables::cparam(chara, "capsule.center_z"),
        gravity_scale: tables::cparam(chara, "move.gravity_scale"),
        speed_run: tables::cparam(chara, "move.max_walk_speed"),
        gravity_jump_up_scale: tables::param("move.jump_up_gravity_scale"),
        gravity_jump_duration_time: tables::param("move.max_jump_duration_time"),
        gravity_jump_duration_scale: tables::param("move.jump_duration_gravity_scale"),
        move_spawn_frame_skip: false,
        ..SORA_PARAMS
    }
}

pub(crate) fn horizontal(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.y, 0.0)
}

pub(crate) fn yaw_of(d: Vec3) -> f32 {
    d.y.atan2(d.x).to_degrees()
}

pub fn wrap180(a: f32) -> f32 {
    let mut a = a % 360.0;
    if a > 180.0 {
        a -= 360.0;
    }
    if a < -180.0 {
        a += 360.0;
    }
    a
}

pub(crate) fn forward(yaw: f32) -> Vec3 {
    let (s, c) = yaw.to_radians().sin_cos();
    Vec3::new(c, s, 0.0)
}

/// The turn rates of the friends' states, deg/s (sheet party-params `turn.*`; none is overridden by
/// n_ex00x_PawnBase or the NPC constructors):
/// - Idle: `m_IdleMaxRotationSpeed` 180 (m_bUseIdleMaxRotationSpeed on; Idle tick 0x140445880 clamps
///   the per-tick rotation to dt x 180).
/// - Turn without a turn clip: `m_FixedPointTurnSpeed` 360 (Turn tick 0x140437890 sub-step 3, vt+0xDE8).
/// - AI move: the locomotion definition's `m_MaxRotationSpeed.Yaw` 180 (ctor 0x141203230; steering 0x1404F2590).
/// - AttackBase's TurnToTarget window: pawn vt+0x2408 = 0x140544270 = 360 (tick 0x1404404B0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnKind {
    Idle,
    FixedPoint,
    Locomotion,
    Attack,
}

/// The Turn request's yaw tolerance (UTresAICharState_Turn +0x9C): Init 0x14043E730 sets 0, and the request
/// (pawn interface vt+0x3C0 = 0x140548220, param 3) is called by UTresBTTask_TurnTo 0x1404B69B0 with 0 (+0xA0 = -1):
/// the state ends at Begin only when the delta is exactly 0.
pub const TURN_DONE: f32 = 0.0;

/// The fixed-rate turn (vt+0xDE8) answers done when the remaining yaw is gone (float rest).
const TURN_FIXED_DONE: f32 = 1.0e-3;

pub fn turn_rate(kind: TurnKind) -> f32 {
    tables::param(match kind {
        TurnKind::Idle => "turn.idle_max_rotation_speed",
        TurnKind::FixedPoint => "turn.fixed_point_speed",
        TurnKind::Locomotion => "turn.loco_max_rotation_yaw",
        TurnKind::Attack => "turn.attack_rate",
    })
}

/// The Turn state's clip run (UTresAICharState_Turn tick 0x140437890 sub-step 0: the turn clip of
/// vt+0x1CE8 (TurnN, TurnB in battle) is played and the montage is given the yaw delta (+0x120),
/// clamped to the clip's own range (+0x128) with the rest (+0xA8) played again; the clip's root yaw
/// curve carries the rotation, scaled to the delta [inf: the montage slot's scaling is read as linear]).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct TurnRun {
    /// Yaw still to turn after this clip, degrees.
    pub rest: f32,
    /// The delta this clip turns, and the clip's own total root yaw.
    pub delta: f32,
    pub clip_yaw: f32,
    /// The fraction of the clip's yaw applied so far.
    pub applied: f32,
    /// No turn clip: sub-step 3 (turn at `m_FixedPointTurnSpeed`).
    pub fixed: bool,
}

impl NpcPawn {
    pub fn new(id: MemberId, chara: Chara, status: FriendStatus, position: Vec3, yaw: f32, seed: u64) -> NpcPawn {
        let params = npc_params(chara);
        let mut react_body = Body::of(BodyClass::Medium);
        react_body.radius = params.capsule_radius;
        react_body.params = actions::friend_damage_params(params.gravity_scale);
        NpcPawn {
            id,
            chara,
            status,
            params,
            cmc: Cmc::new(&params, position),
            yaw,
            anim: NpcAnim::new(tables::clip(chara, "IdleN")),
            state: NpcStateId::Idle,
            state_time: 0.0,
            cmd: None,
            cmd_new: false,
            last: (CommandTicket(0), CommandStatus::Unknown),
            history: Vec::new(),
            next_ticket: 1,
            action: None,
            react: EnemyReaction::new(react_body),
            die: DieRun::default(),
            warp: WarpRun::default(),
            bad: (0, 0.0),
            bad_sub: 0,
            ignore_overlap_force: false,
            burn_hops: 0,
            burn_yaw: 0.0,
            item_pending: None,
            damage_reactions: 0,
            idle_timer: 0.0,
            anim_wrapped: false,
            reflect_from: None,
            reflect_dir: Vec3::ZERO,
            pad: None,
            motionless: 0.0,
            in_battle: false,
            battle_time: 0.0,
            can_battle_latch: false,
            battle_follow: false,
            rng: Rng::new(seed),
            events: Vec::new(),
            requests: Vec::new(),
            hit_list: Vec::new(),
            hits_this_action: 0,
            scratch_targets: Vec::new(),
            fr_drive: None,
            fr_visible: true,
            focus: TargetRef::None,
            turn: TurnRun::default(),
            dmg_index: 0,
            seam: kh_core::host::party::SeamHandles::for_member(kh_core::host::PartyMember(id.0)),
            voices: Vec::new(),
            notify_seam: kh_core::host::party::PartyNotify::new(),
            notify_events: Vec::new(),
            trav: crate::traverse::Traverse::default(),
            ladder: crate::ladder::Leap::default(),
            steer: None, // (party ER S1)
            loco: Default::default(), // (party K1)
            hurt: Default::default(), // (GOOFY-HR)
            don_cast: Default::default(), // (DON-MAGIC)
            face: Default::default(),      // (ZETTA)
            don_zetta: Default::default(), // (ZETTA)
            don_ward_offer: 0, // (DON-KIT)
            don_ward_used: 0, // (DON-KIT)
            don_aura: crate::donald::aura::AuraEffect::NEUTRAL, // (DON-KIT)
        }
    }

    /// Queues a state-driven voice (E.4 `voice_play`); played through the host at the next step.
    pub fn queue_voice(&mut self, group: kh_core::host::party::VoiceGroup, number: i32, mode: kh_core::host::party::VoiceMode) {
        self.events.push(PartyEventKind::Voice { group: group.name(kh_core::host::PartyMember(self.id.0)) });
        self.voices.push((group, number, mode));
    }

    fn play_voices(&mut self, host: &mut PartyHost) {
        let m = kh_core::host::PartyMember(self.id.0);
        for (g, n, mode) in std::mem::take(&mut self.voices) {
            if let Some(play) = kh_core::host::party::voice_play(m, g, n, mode) {
                self.seam.play(host.audio, &play);
            }
        }
    }

    pub fn position(&self) -> Vec3 {
        self.cmc.location
    }

    pub fn capsule_centre(&self) -> Vec3 {
        self.cmc.location + Vec3::Z * self.params.capsule_center_z
    }

    pub fn on_ground(&self) -> bool {
        self.cmc.on_ground()
    }

    // ------------------------------------------------------------------ commands

    pub fn command(&mut self, cmd: MemberCommand) -> CommandTicket {
        let t = CommandTicket(self.next_ticket);
        self.next_ticket += 1;
        let fr_ok = (matches!(cmd, MemberCommand::EnterFrState { .. }) && self.state != NpcStateId::Die) || (matches!(self.state, NpcStateId::Fr(_)) && cmd == MemberCommand::Stop);
        // (LADDER2) in a ladder jump (port design): only Warp (the host's stuck fallback) ends it; the AI's Stop does not
        // (its tree stops a member who waits for his turn); the host ends it with `ladder::cancel`
        let ladder_ok = self.state == NpcStateId::Ladder && matches!(cmd, MemberCommand::Warp { .. });
        if self.state.refuses_commands() && !fr_ok && !ladder_ok {
            self.record(t, CommandStatus::Failed);
            return t;
        }
        // (party I1) The game's request gates (replacing B5's inferred "refused until CanChange" rule):
        // - Action / Cure: RequestAction (iface vt+0x4B0 0x140547B40) needs CanAction (vt+0x4A8 0x141258EE0: !pawn
        //   vt+0x26F8 0x141267590 && pawn vt+0x25A0 0x140530A50, both about the QUEUED state; the port queues none), then
        //   either the running state is an action whose AI_ActionAbortTiming window is open AND whose request carried
        //   an abort id (FUN_140520410: def +0xCA set by TresAnimNotifyState_AI_ActionAbortTiming via iface slot
        //   0x408 0x140521E70, +0xCB = the BT task's m_AIActionAbortTimingID or 15 with m_AbortTime > 0; the friends'
        //   trees set neither, so never), or the definition's CanStart (CDO vt+0x3C0 = 0x140518070): IDLE with nothing
        //   queued, else the running state's id in the asset's m_ViableStates (every Donald / Goofy action lists
        //   TSID_STATE_AI_MOVE, the magic / Tornado / Turbo / Discus also TSID_STATE_AI_SWIM; Attack8/9 Water* and
        //   Goofy's Water* only AI_SWIM: `actions::viable_in`). So an action is refused in a jump, a climb, a hang,
        //   a slide, a turn and while another action runs.
        // - TurnTo: iface vt+0x3C0 0x140548220 needs vt+0x3B8 0x14125A420 (!vt+0x26F8 && FUN_140530C80: IDLE with
        //   nothing queued, TURN 0x2C, or the swim locomotion with FUN_141208C80).
        // - MoveTo: its state switch is the locomotion gate iface vt+0x498 0x141258F80 -> FUN_140530710 (JUMP 3 /
        //   NAVLINK_JUMP 0x2B: the state's CanChange vt+0x388 0x1404325F0 = sub 6 && m_bEnableChange; CLIMBING 7:
        //   0x1412405A0 = sub 4 && m_bEnableChange; DANGLE and the slides: FUN_1405459A0 = false), but the path
        //   following still feeds the running state (iface vt+0x118 0x141270D90): the command is taken as the feed
        //   (`begin_command` keeps those states).
        // - Warp: the pawn tick 0x14127B980 sets state 125 directly (FUN_1405B9FE0, no gate) once FUN_141258790
        //   passes: never refused here (the controller's warp check is the gate).
        // - UseItem: its request path (FUN_141274340 -> ...) was not followed: the B5 rule stays for it [inf].
        // A pad-driven member (stream P: the player leads him, KH3 has no such pawn) keeps the port design: its
        // combo chains actions and turns as Sora does, so the AI gates do not apply to it.
        let ai = self.pad.is_none();
        // (DON-MAGIC part A) an AI Donald casting (crate::donald::cast) keeps his cast clip: no move, turn, action or motion
        // command runs it over (his AI asks again once the cast is over)
        let casting = ai && self.status.donald.is_some() && crate::donald::cast::casting(self);
        let refused = casting && matches!(cmd, MemberCommand::MoveTo { .. } | MemberCommand::TurnTo { .. } | MemberCommand::Action { .. } | MemberCommand::UseItem { .. } | MemberCommand::Cure { .. } | MemberCommand::PlayMotion { .. } | MemberCommand::Idle { .. } | MemberCommand::Stop);
        let refused = refused || match cmd {
            MemberCommand::Action { def, .. } => ai && !crate::actions::viable_in(def, self.state),
            MemberCommand::Cure { .. } => ai && !crate::actions::viable_in(crate::ActionDef::DonaldCure, self.state),
            MemberCommand::TurnTo { .. } => ai && !matches!(self.state, NpcStateId::Idle | NpcStateId::Turn | NpcStateId::Pad(_)),
            MemberCommand::UseItem { .. } | MemberCommand::PlayMotion { .. } | MemberCommand::Idle { .. } => crate::traverse::refuses_now(self),
            _ => false,
        };
        if refused {
            self.record(t, CommandStatus::Failed);
            return t;
        }
        // A pending EnterFrState (D's team attack, not yet taken at this step's intake) wins over any
        // later command but another EnterFrState / Stop (stream D relay 2026-10-15). READ (B3): the state
        // component has NO request priority: every request (0x1405B9FE0 -> 0x141513260) first releases the
        // pending ones (0x14150A660, vt+0x290 each), so the latest request of a frame wins. The game gets the
        // same result because the friend's AI is stopped while a team attack runs (its BT issues nothing);
        // the port's controllers may still tick in that step, hence this rule [inf: the AI stop is C/D's].
        if self.cmd_new && matches!(self.cmd, Some((_, MemberCommand::EnterFrState { .. }))) && !matches!(cmd, MemberCommand::EnterFrState { .. } | MemberCommand::Stop) {
            self.record(t, CommandStatus::Failed);
            return t;
        }
        // (party ER S1, LADDER2) the same for the host's ladder jump (port design): a pending LadderJump wins over the AI's
        // commands of the same step (its tree re-issues its MoveTo or a Stop at once when the host's command aborts it); only
        // Warp replaces it
        if self.cmd_new && matches!(self.cmd, Some((_, MemberCommand::LadderJump(_)))) && !matches!(cmd, MemberCommand::Warp { .. }) {
            self.record(t, CommandStatus::Failed);
            return t;
        }
        // An action the pawn cannot start answers Failed at once and is not queued (the BT re-sends
        // it every tick until accepted, 0x1404A2370; stream C relay 2026-10-15): wrong character,
        // or the ability gate FUN_1405A4EA0 (equipped and payable) refuses it.
        let gated = match cmd {
            MemberCommand::Action { def, .. } => Some(def),
            MemberCommand::Cure { .. } => Some(crate::ActionDef::DonaldCure),
            _ => None,
        };
        if let Some(def) = gated {
            // (party X2) a leader's Sora spell parked for this action pays Sora's cost at its Shot, not this ability's
            let payable = crate::leader_magic::parked(self) || actions::ability(def).map_or(true, |k| self.status.don_tree_gate(k).unwrap_or_else(|| self.status.can_pay(k))); // (DON-PROG) Donald's KH3 spell abilities are gone: his state answers the gate
            if def.chara() != self.chara || !payable {
                self.record(t, CommandStatus::Failed);
                return t;
            }
        }
        if let Some((old, _)) = self.cmd.take() {
            self.record(old, CommandStatus::Aborted);
        }
        self.cmd = Some((t, cmd));
        self.cmd_new = true;
        self.last = (t, CommandStatus::Running);
        t
    }

    fn record(&mut self, t: CommandTicket, s: CommandStatus) {
        if self.last.0 == t {
            self.last.1 = s;
        }
        self.history.push((t, s));
        if self.history.len() > 32 {
            self.history.remove(0);
        }
    }

    pub fn command_status(&self, t: CommandTicket) -> CommandStatus {
        if self.cmd.is_some_and(|(c, _)| c == t) {
            return CommandStatus::Running;
        }
        self.history.iter().rev().find(|(c, _)| *c == t).map(|&(_, s)| s).unwrap_or(CommandStatus::Unknown)
    }

    fn finish_command(&mut self, ok: bool) {
        if let Some((t, _)) = self.cmd.take() {
            self.record(t, if ok { CommandStatus::Succeeded } else { CommandStatus::Failed });
        }
    }

    /// (B4) the running command, for the traversal states (the MoveTo they follow).
    /// (ZETTA 2026-10-23) The running command (the hosts' clean slate at Zettaflare's hand-back, the loop guard's line).
    pub fn command_running(&self) -> Option<MemberCommand> {
        self.cmd.map(|(_, c)| c)
    }

    pub(crate) fn command_now(&self) -> Option<MemberCommand> {
        self.cmd.map(|(_, c)| c)
    }

    /// (DON-PROG) The command the AI gave since the last step that the pawn has not taken yet (AI Donald's spell / melee
    /// requests, crate::donald::ai::party_pass).
    pub(crate) fn don_pending(&self) -> Option<MemberCommand> {
        self.cmd.filter(|_| self.cmd_new).map(|c| c.1)
    }

    /// (DON-PROG) That command is answered by the overhaul: Succeeded (its choice started) or Failed, and not taken.
    pub(crate) fn don_take_pending(&mut self, ok: bool) {
        if !self.cmd_new {
            return;
        }
        if let Some((t, _)) = self.cmd.take() {
            self.cmd_new = false;
            self.record(t, if ok { CommandStatus::Succeeded } else { CommandStatus::Failed });
        }
    }

    pub(crate) fn finish_command_pub(&mut self, ok: bool) {
        self.finish_command(ok);
    }

    pub(crate) fn abort_command_pub(&mut self) {
        self.abort_command();
    }

    /// IdleFinished (UTresNpcState_Idle 0x141245380 / 0x14124CD40) -> the Idle task (C) succeeds.
    fn idle_finished(&mut self) {
        if matches!(self.cmd, Some((_, MemberCommand::Idle { .. }))) {
            self.finish_command(true);
        }
    }

    fn abort_command(&mut self) {
        // (B4) leaving Idle sends IdleFinished first (End 0x141245380): the Idle task succeeds, not aborted
        if self.state == NpcStateId::Idle {
            self.idle_finished();
        }
        if let Some((t, _)) = self.cmd.take() {
            self.record(t, CommandStatus::Aborted);
        }
    }

    // ------------------------------------------------------------------ states

    pub fn enter(&mut self, s: NpcStateId) {
        // (B4, C4 relay) UTresNpcState_Idle End 0x141245380 sends IdleFinished: a running Idle task succeeds.
        if self.state == NpcStateId::Idle && s != NpcStateId::Idle {
            self.idle_finished();
        }
        if s == NpcStateId::Idle && self.state != NpcStateId::Idle {
            self.idle_timer = 0.0;
        }
        // (party K1) the Move state's loop pose goes with it; any other state ends the idle turn / stop clip
        if self.state == NpcStateId::Move && s != NpcStateId::Move {
            crate::locomotion::leave(self);
        }
        if s != NpcStateId::Idle {
            self.loco.idle_turn = None;
            if self.loco.sub == 3 {
                self.loco.sub = 0;
            }
        }
        if self.state != s {
            self.events.push(PartyEventKind::StateChanged { from: self.state, to: s });
        }
        self.state = s;
        self.state_time = 0.0;
    }

    pub fn play(&mut self, name: &str, blend: f32) {
        let c = tables::clip(self.chara, name);
        self.anim.play(c, blend);
        self.hit_list.clear();
    }

    /// (party P3) PlayAnim with a loop mode (mode 1 / 3 of the anim interface's PlayAnim, sora-actions-spec 2:
    /// "mode 1 loop, 2 one-shot with root motion, 3 loop with root motion"): the clock wraps whatever the row says.
    /// Tornado's M_1322 and Turbo's M_1325 (mode 1, 0x1411C61D0 / 0x1411C6CB0) and the Discus flight loop M_1328 /
    /// M_1331 (mode 3, 0x1411C7670) are such plays: as one-shots their pose stood on the last frame.
    pub fn play_looped(&mut self, name: &str, blend: f32) {
        let c = tables::clip(self.chara, name);
        self.anim.play_loop(c, blend);
        self.hit_list.clear();
    }

    /// (party P3) The End of every notify state a play cut (`NpcAnim::take_cut`), through the seam: what those states
    /// started (looping sounds, persistent effects) stops, with their LoopEndSound.
    pub fn end_cut_states(&mut self, host: &mut PartyHost) {
        let cut = self.anim.take_cut();
        if cut.is_empty() {
            return;
        }
        let m = kh_core::host::PartyMember(self.id.0);
        let at = self.position();
        let mut evs = std::mem::take(&mut self.notify_events);
        for (clip, ends) in cut {
            crate::notify_seam::events_of(self.chara, clip.sequence, clip.notifies, &ends, &mut evs);
            if !evs.is_empty() {
                self.notify_seam.dispatch(m, &mut self.seam, &evs, at, 0, host.audio, host.effects);
            }
        }
        self.notify_events = evs;
    }

    /// (party P3) Stops everything the member's open notify states started (removed, despawned).
    pub fn stop_notify_loops(&mut self, host: &mut PartyHost) {
        let _ = self.anim.take_cut();
        self.notify_seam.stop_all(host.audio, host.effects);
    }

    /// The running definition's End on an interrupt (state exit): the jump's 0x1412253B0.
    pub fn end_action(&mut self) {
        if self.state == NpcStateId::Action && actions::is_jump(self) {
            actions::jump_end(self);
        }
        // (B4) a traversal state's End (also on an interrupt)
        if crate::traverse::is_traverse(self.state) {
            crate::traverse::end(self);
        }
    }

    /// Back to Idle / battle idle.
    pub fn to_idle(&mut self) {
        self.enter(NpcStateId::Idle);
        let idle = if self.in_battle { "IdleB" } else { "IdleN" };
        self.play(idle, -1.0);
        self.action = None;
    }

    // ------------------------------------------------------------------ goals

    pub fn goal_point(&self, goal: MoveGoal, cx: &StepCtx, host: &PartyHost) -> Option<Vec3> {
        let raw = self.goal_point_raw(goal, cx, host)?;
        // (ZETTA) an AI member keeps out of Zettaflare's zones (its target, the beam): out of one first, round one
        if self.pad.is_none() && !cx.zones.is_empty() && cx.zones.iter().all(|z| z.caster != self.id) {
            return Some(crate::donald::zettaflare::zone_goal(cx.zones, self.position(), raw));
        }
        Some(raw)
    }

    /// The goal's point as the command names it (`goal_point` keeps it out of Zettaflare's zones).
    fn goal_point_raw(&self, goal: MoveGoal, cx: &StepCtx, host: &PartyHost) -> Option<Vec3> {
        match goal {
            MoveGoal::Point(p) => Some(p),
            MoveGoal::Sora => Some(cx.sora.position),
            MoveGoal::Member(m) => cx.members.iter().find(|(id, _, _)| *id == m).map(|&(_, p, _)| p),
            MoveGoal::Target(t) => host.targets.info(t).map(|i| i.position),
        }
    }

    pub fn target_point(&self, t: TargetRef, cx: &StepCtx, host: &PartyHost) -> Option<Vec3> {
        match t {
            TargetRef::None => None,
            TargetRef::Sora => Some(cx.sora.position),
            TargetRef::Member(m) => self.goal_point(MoveGoal::Member(m), cx, host),
            TargetRef::Enemy(id) => host.targets.info(id).map(|i| i.center),
            TargetRef::Point(p) => Some(p),
        }
    }

    pub fn turn_toward(&mut self, point: Vec3, dt: f32, rate: f32) -> f32 {
        let d = horizontal(point - self.position());
        if d.length_squared() < 1e-4 {
            return 0.0;
        }
        let want = yaw_of(d);
        let diff = wrap180(want - self.yaw);
        let step = (rate * dt).min(diff.abs());
        self.yaw = wrap180(self.yaw + step * diff.signum());
        wrap180(want - self.yaw)
    }

    /// Turn Begin + tick sub-step 0 (0x140437890): the delta to the goal (FUN_1405CDB70); within the
    /// request's yaw tolerance (+0x9C) the state ends at once; else the turn clip (TurnN / TurnB) with
    /// the delta, or the fixed-rate turn when the set has no turn clip with root yaw.
    fn begin_turn(&mut self, goal: MoveGoal, cx: &StepCtx, host: &PartyHost) {
        self.enter(NpcStateId::Turn);
        self.turn = TurnRun::default();
        let Some(p) = self.goal_point(goal, cx, host) else {
            self.finish_command(false);
            self.to_idle();
            return;
        };
        let d = horizontal(p - self.position());
        let delta = if d.length_squared() < 1e-4 { 0.0 } else { wrap180(yaw_of(d) - self.yaw) };
        if delta.abs() <= TURN_DONE {
            self.finish_command(true);
            self.to_idle();
            return;
        }
        self.cmc.stop_movement();
        self.start_turn_clip(delta);
    }

    fn start_turn_clip(&mut self, delta: f32) {
        let name = if self.in_battle { "TurnB" } else { "TurnN" };
        let clip = tables::clip(self.chara, name);
        let clip_yaw = clip.root_at(clip.length).yaw;
        if clip_yaw.abs() < 1.0 {
            self.turn = TurnRun { fixed: true, ..TurnRun::default() };
            return;
        }
        // +0x128: the clip's turn range; a delta beyond it keeps the rest for the next play.
        let range = clip_yaw.abs();
        let now = delta.clamp(-range, range);
        self.turn = TurnRun { rest: delta - now, delta: now, clip_yaw, applied: 0.0, fixed: false };
        self.play(name, -1.0);
        // (party K1) the blend space's axis = the delta: the drawing shows the left / right / 180 samples (it showed
        // the 0-degree sample, whatever the turn)
        self.anim.axis = now;
    }

    // ------------------------------------------------------------------ hits ON the member

    /// A hit from an enemy (contract B.3 `receive_hit`): `hit.damage` is final (the host ran
    /// `compute_damage` with `defender()`, which caps it at 24).
    pub fn receive_hit(&mut self, hit: &HitRecord) -> HitResult {
        // (party X3D) a pad Donald's kit first: the Hop / Blink / Flutter / finishers' invincibility, the Magic Block
        if let Some(r) = crate::pad::donald::on_hit(self, hit) {
            return r;
        }
        // (party X2) a leader's grand magic / Finish End cast: invincible until its Shot (Sora's damage_invalid rule)
        if crate::leader_magic::invincible(self) {
            return HitResult { accepted: false, ..HitResult::default() };
        }
        // (LADDER2) the ladder jump (port design) is a scripted leap: no hit takes him out of the air
        if self.state == NpcStateId::Die || matches!(self.state, NpcStateId::Fr(_)) || self.state == NpcStateId::Ladder {
            // Die Begin 0x1412413C0: invincible (vt+0x2068 1) and no collision (vt+0x6A0 0).
            return HitResult { accepted: false, ..HitResult::default() };
        }
        // (GOOFY-HR) a lethal blow is playing: Die's invincibility from the hit that knocked him out
        if self.hurt.dying {
            return HitResult { accepted: false, ..HitResult::default() };
        }
        // (party X3G) a pad Goofy's kit: the Hop / Surf / finishers' invincibility, the shield guard and Perfect Guard
        if let Some(r) = crate::pad::goofy::on_hit(self, hit) {
            return r;
        }
        let takes = hit.takes_hp();
        let dmg = if takes { hit.damage.max(0.0).floor() as i32 } else { 0 };
        let before = self.status.hp;
        self.status.hp = (self.status.hp - dmg).max(0);
        self.status.hp = self.status.don_survive(before, self.status.hp, self.state == NpcStateId::Damage); // (DON-PROG) Second Chance / Withstand Combo on Donald
        let _ = self.status.don_mp_rage(before - self.status.hp); // (DON-PROG) MP Rage
        let dealt = before - self.status.hp;
        // QuarterHP (E.4): 0 < hp_after < hp_before and hp_after < CeilToInt(max_hp x 0.25) [the voice gate
        // FUN_141267740 is not modelled].
        if self.status.hp > 0 && self.status.hp < before && (self.status.hp as f32) < (self.status.max_hp() as f32 * 0.25).ceil() {
            let n = kh_core::host::party::pick_number(kh_core::host::party::VOICE_VARIATIONS, self.rng.next_u32());
            self.queue_voice(kh_core::host::party::VoiceGroup::QuarterHp, n, kh_core::host::party::VoiceMode::Replace);
        }
        self.events.push(PartyEventKind::Damaged { amount: dealt, hp: self.status.hp });
        let killed = self.status.hp < 1;
        // (GOOFY-HR) KH3's hit effect at the contact of a hit that took HP
        if dealt > 0 {
            crate::hurt::impact(self, hit);
        }
        // ApplyBadStatus 0x1405B7C50 (the CharPawnBase rule kh-core ports for Sora): kinds 1..5 / 9..11 are
        // none; the same kind again only renews the time; another kind replaces it.
        let bad = kh_core::combat::badstat::kind_of(hit.info.bad_status);
        let bad = if matches!(bad, 1..=5 | 9..=11) { 0 } else { bad };
        if !killed && bad != 0 && hit.info.bad_time > 0.0 {
            self.bad = (bad, hit.info.bad_time);
        }
        // (B4) TakeDamage 0x1405BB850 (combat spec 8.4): a victim result REFLECT (3) -> iface vt+0x448(3, causer),
        // GUARD_REFLECT (4) -> vt+0x458 else vt+0x448(4) [inf: vt+0x458 is read as declining]: the Reflect state
        // (0x141270B20) from the causer; no HP (takes_hp) and no damage reaction.
        let reflected = matches!(hit.info.victim_result, kh_core::host::ReactionResult::Reflect | kh_core::host::ReactionResult::GuardReflect);
        if reflected && !killed && !matches!(self.state, NpcStateId::Reflect) {
            let from = if hit.attacker.valid { hit.attacker.position } else { self.position() - hit.direction * 100.0 };
            self.begin_reflect(from);
            return HitResult { accepted: true, damage_dealt: dealt as f32, hp_after: self.status.hp as f32, killed, guarded: false };
        }
        if killed {
            self.bad = (0, 0.0);
            // (GOOFY-HR) a lethal HEAVY hit plays its blow first; the KO comes at the down clip's mark (hurt.rs)
            let burning = self.state == NpcStateId::BadStatBurn;
            if crate::hurt::HP.hurt_lethal_blow && !burning && crate::hurt::heavy(hit.damage_kind) && self.begin_reaction(hit, true) {
                self.hurt.dying = true;
                self.hurt.log.push(("lethal_blow", self.anim.clip.name));
            } else {
                self.start_die();
            }
        } else {
            // the friends run the status as their own state (enter_bad_state), not as a reaction phase
            let mut h = *hit;
            if self.bad.0 != 0 {
                h.info.clear_bad_status();
            }
            // burning: m_bIsNoDamageResponse (pawn vt+0x2078): HP without a reaction
            let no_reaction = self.state == NpcStateId::BadStatBurn || crate::pad::goofy::no_reaction(self, hit); // (party X3G) Stalwart, Shield Guardian
            if no_reaction {
            } else if self.begin_reaction(&h, dealt > 0) {
                // (GOOFY-HR) begin_reaction: the reaction with the character's own clip lengths and its first clip
            } else {
                // no reaction (a hit without damage response): the status's state comes at once
                self.enter_bad_state();
            }
        }
        HitResult { accepted: true, damage_dealt: dealt as f32, hp_after: self.status.hp as f32, killed, guarded: false }
    }

    /// (GOOFY-HR) The damage state's start, the same for every playable character: kh-react's reaction with the
    /// character's own KH3 clip lengths for this hit, its first clip (hurt.rs `started`). False = the hit starts none.
    fn begin_reaction(&mut self, hit: &HitRecord, hp_lost: bool) -> bool {
        let req = ReactionRequest::from_hit(hit, hp_lost, false);
        let pose = Pose { position: self.position(), yaw: self.yaw, grounded: self.on_ground() };
        crate::hurt::prepare(self, hit); // (GOOFY-HR)
        let HitOutcome::Started { .. } = self.react.hit(&req, pose) else { return false };
        self.damage_reactions = self.damage_reactions.wrapping_add(1);
        self.abort_command();
        self.end_action();
        self.action = None;
        self.enter(NpcStateId::Damage);
        self.dmg_index = damage_dir_index(hit.direction, self.yaw);
        crate::hurt::started(self);
        true
    }

    /// (party X3D) The running hit reaction is cut (Donald's Flutter: the aerial recovery out of a blow).
    pub fn release_reaction(&mut self) {
        self.react.release();
    }

    /// `UTresNpcState_Die` Begin 0x1412413C0.
    pub fn start_die(&mut self) {
        self.hurt.dying = false; // (GOOFY-HR)
        self.abort_command();
        self.end_action();
        self.action = None;
        self.react.release();
        self.enter(NpcStateId::Die);
        self.die = DieRun { timer: self.status.revive_time(), sub: if self.on_ground() { 0 } else { 3 } };
        self.cmc.stop_movement();
        // Anim names 0x141246E00: index 0 BadStatStunStart (air: BadStatStuAirStart).
        let clip = if self.on_ground() { "BadStatStunStart" } else { "JumpNormalDownLoop" };
        self.play(clip, -1.0);
        self.events.push(PartyEventKind::Ko);
    }

    // ------------------------------------------------------------------ the step

    pub fn step(&mut self, cx: &StepCtx, host: &mut PartyHost) {
        let dt = cx.dt;
        self.status.tick(dt);
        // CanBattle 0x141257340: the latch +0xCA8 is set once the battle has run for more than 0.5 s
        // (FUN_140541660: world time - battle start +0xA7C) and cleared with the battle flag (vt+0x1740).
        if self.in_battle {
            self.battle_time += dt;
            if self.battle_time > 0.5 {
                self.can_battle_latch = true;
            }
        } else {
            self.battle_time = 0.0;
            self.can_battle_latch = false;
        }
        self.state_time += dt;
        if self.cmd_new {
            self.cmd_new = false;
            self.begin_command(cx, host);
        }
        // (party P)
        if self.pad.is_some() {
            crate::pad::pre_move(self, cx, host);
        }
        self.play_voices(host);
        crate::hurt::flush(self, host); // (GOOFY-HR) the hit effect of the last hit
        // (B4) the delayed item effect
        if let Some((t, item, target)) = self.item_pending {
            if t - dt <= 0.0 {
                self.item_pending = None;
                actions::item_effect(self, item, target);
            } else {
                self.item_pending = Some((t - dt, item, target));
            }
        }
        // (party P3) the notify states a play cut (the clip they belong to was replaced) end now: their looping sounds
        // and persistent effects stop (NotifyEnd of UE's active-notify update), whatever replaced the clip (the
        // action's next clip, a hit reaction, KO, a cancel, Idle).
        self.end_cut_states(host);
        // movement with the pose tick inside (Sora's order).
        self.cmc.tick_head(host.world, &self.params, dt);
        let tick = self.anim.tick(dt);
        self.anim_wrapped = tick.wrapped;
        // E.3: the seam part of the fired notifies (sound, weapon SE, auto SE, voice, actor voice, effects) through
        // E's PartyNotify on Attach::Party, before B's own gameplay handling below.
        if !tick.fired.is_empty() {
            let mut evs = std::mem::take(&mut self.notify_events);
            crate::notify_seam::events_of(self.chara, self.anim.clip.sequence, self.anim.clip.notifies, &tick.fired, &mut evs);
            if !evs.is_empty() {
                let m = kh_core::host::PartyMember(self.id.0);
                let at = self.position();
                self.notify_seam.dispatch(m, &mut self.seam, &evs, at, 0, host.audio, host.effects);
            }
            self.notify_events = evs;
        }
        for f in &tick.fired {
            self.events.push(PartyEventKind::Notify { clip: self.anim.clip.name, sequence: self.anim.clip.sequence, index: f.index, phase: f.phase });
            match (f.notify.data, f.phase) {
                (ClipNotifyData::Attack { .. }, kh_core::anim::NotifyPhase::Begin) => self.hit_list.clear(),
                // (B5) TresAnimNotifyState_Traction -> TresStateBase m_bEnableTraction (+0x84): the Climbing lift window
                (ClipNotifyData::State { name: "Traction" }, kh_core::anim::NotifyPhase::Begin) if self.state == NpcStateId::Climbing => self.trav.climb.traction = true,
                (ClipNotifyData::State { name: "Traction" }, kh_core::anim::NotifyPhase::End) if self.state == NpcStateId::Climbing => self.trav.climb.traction = false,
                (ClipNotifyData::Raw("Shot"), _) | (ClipNotifyData::Raw("ThrowWeapon"), _) => actions::on_shot(self, cx, host),
                // TresAnimNotify_JumpStart (0x140560120 -> anim interface vt+0x2A0): the jump definition's launch.
                (ClipNotifyData::Raw("JumpStart"), _) => {
                    if self.state == NpcStateId::BadStatBurn {
                        self.burn_launch();
                    } else if self.state == NpcStateId::NavLinkJump {
                        // (B5) the JumpCodeDriven launch vt+0x3F8 0x140433FD0 (+0x130: JumpNormalStart has it)
                        crate::jump::launch(self, host.world);
                    } else {
                        actions::on_jump_start(self, cx, host);
                    }
                }
                // TresAnimNotify_EnableChange (0x14055F920): the definition's m_bEnableChange.
                (ClipNotifyData::Raw("EnableChange"), _) => {
                    // (B5) TresStateBase +0x84 m_bEnableChange of the jump / ledge states (their vt+0x388 reads it)
                    match self.state {
                        NpcStateId::NavLinkJump => self.trav.jump.enable_change = true,
                        NpcStateId::Climbing => self.trav.climb.enable_change = true,
                        NpcStateId::Dangle => self.trav.dangle.enable_change = true,
                        _ => {}
                    }
                    if let Some(mut r) = self.action {
                        r.jump.enable_change = true;
                        self.action = Some(r);
                    }
                }
                // (B4) TresAnimNotify_Timing (0x140561CC0 -> anim interface vt+0x348(m_Param)): the jump definition's
                // handler 0x14122DE40 (def+0x40 vtable 0x14678B410 slot 0x348) sets +0x25A (the decel) for m_Param 0.
                // M_1311 / M_1312 of both sets carry one with no m_Param (class default 0, ctor 0x14055ADA0).
                // (B4) In UseItem the Timing notify is the use: UTresCharState_UseItem +0x40 vt+0x348 = 0x141512C80 ->
                // FUN_1405BCBE0(user, item, target, 1, 0.5): the pouch item is spent now, its effect lands 0.5 s
                // later (FUN_1405BD190 -> target vt+0x1200); target+0xBC8 = the user's uid.
                (ClipNotifyData::Raw("Timing"), _) if self.state == NpcStateId::UseItem => {
                    if let Some(mut run) = self.action {
                        if let Some(item) = run.item.filter(|_| !run.success) {
                            if self.status.take_item(item) {
                                self.item_pending = Some((0.5, item, run.target));
                                self.events.push(PartyEventKind::ItemUsed { item, target: run.target });
                            }
                            run.success = true;
                            self.action = Some(run);
                        }
                    }
                }
                (ClipNotifyData::Raw("Timing"), _) => {
                    if self.state == NpcStateId::Action && actions::is_jump(self) {
                        if let Some(mut r) = self.action {
                            r.jump.decel = true;
                            self.action = Some(r);
                        }
                    }
                }
                _ => {}
            }
        }
        // The Turn state rotates by its own scaled curve (TurnRun), not the clip's raw root yaw.
        let turning = self.state == NpcStateId::Turn || crate::locomotion::scales_root_yaw(self); // (party K1) the idle turn
        let root = tick.root.map(|(dx, dy, dz, dyaw)| {
            let dyaw = if turning { 0.0 } else { dyaw };
            let f = forward(self.yaw);
            let r = Vec3::new(-f.y, f.x, 0.0);
            (f * dx + r * dy + Vec3::Z * dz, dyaw)
        });
        if self.state == NpcStateId::Damage {
            let w = CapsuleIn { world: host.world, radius: self.params.capsule_radius, half_height: self.params.capsule_half_height };
            let out = self.react.step_dt(&w, dt);
            if out.suspended {
                self.cmc.location = out.position;
                self.cmc.velocity = out.velocity;
                self.yaw = out.yaw;
            }
            // (GOOFY-HR) the clip of what the reaction does (KH3's Blow / Small / launch / wall rows, loops looped), the
            // end with the clip, a lethal blow's KO (hurt.rs `tick`)
            match crate::hurt::tick(self, &out) {
                crate::hurt::Tick::Go => {}
                crate::hurt::Tick::Done => {
                    self.to_idle();
                    // the status trigger brings its state once he stands again
                    self.enter_bad_state();
                }
                crate::hurt::Tick::Ko => {
                    self.react.release();
                    self.hurt.log.push(("ko", self.anim.clip.name));
                    self.start_die();
                }
            }
        } else {
            // (DON-KIT) Donald's Thunder aura (4.2 "Party movement +10 %"): the speed limits of this movement only (run,
            // sprint, the AI's moves; root motion is the clip's own), restored after so no state's value compounds
            let aura_move = self.don_aura.move_mult;
            let speeds = (self.cmc.max_walk_speed, self.cmc.speed_limit_override);
            if aura_move != 1.0 {
                self.cmc.max_walk_speed *= aura_move;
                self.cmc.speed_limit_override = self.cmc.speed_limit_override.map(|v| v * aura_move);
            }
            // (DON-KIT) a spell cast holds Donald in place (the user 2026-10-22: "Ensure Donald does not ever walk while
            // casting a spell, his animation takes priority"): pad and AI alike, his ground position is kept
            let cast_hold = crate::pad::donald::cast_holds_still(self).then_some((self.cmc.location.x, self.cmc.location.y));
            self.cmc.perform_movement(host.world, &self.params, dt, root, &mut self.yaw);
            if let Some((x, y)) = cast_hold {
                (self.cmc.location.x, self.cmc.location.y) = (x, y);
                (self.cmc.velocity.x, self.cmc.velocity.y) = (0.0, 0.0);
            }
            if aura_move != 1.0 {
                (self.cmc.max_walk_speed, self.cmc.speed_limit_override) = speeds;
            }
        }
        // attack windows
        if self.anim.open_attack().is_some() {
            actions::hit_test(self, cx, host);
        }
        if let Some(at) = self.reflect_from.take() {
            // (B4) 0x141270B20: from any state the request may replace (not Die / FR; already in Reflect: kept)
            if !matches!(self.state, NpcStateId::Die | NpcStateId::Fr(_) | NpcStateId::Reflect) {
                self.begin_reflect(at);
            }
        }
        // GetTimeMotionless (movement +0xC68): [inf] under 10 cm/s counts
        if self.speed2d() < 10.0 {
            self.motionless += dt;
        } else {
            self.motionless = 0.0;
        }
        self.main_proc(cx, host);
    }

    fn begin_command(&mut self, cx: &StepCtx, host: &mut PartyHost) {
        let Some((_, cmd)) = self.cmd else { return };
        // (ZETTA) an AI member does not attack an enemy in Zettaflare's zone (its target, the beam): it disengages
        if self.pad.is_none() && !cx.zones.is_empty() {
            if let MemberCommand::Action { target: TargetRef::Enemy(t), .. } = cmd {
                if host.targets.info(t).is_some_and(|i| cx.zones.iter().any(|z| z.inside(i.position))) {
                    self.finish_command(false);
                    return;
                }
            }
        }
        // (B4) the slides, the rail and the hop points keep running under a new MoveTo (the path following
        // feeds them, iface vt+0x118 0x141270D90); any other state change ends them first.
        let keeps = matches!(cmd, MemberCommand::Focus(_) | MemberCommand::SetBattleFollowMode(_)) || (matches!(cmd, MemberCommand::MoveTo { .. }) && matches!(self.state, NpcStateId::SlopeSlide | NpcStateId::SuperSlide | NpcStateId::RailSlide | NpcStateId::Hopping | NpcStateId::NavLinkJump | NpcStateId::Climbing | NpcStateId::Dangle));
        // (B5) a Stop while a jump / ledge state may not be left (traverse::refuses_now): the command ends, the state runs on
        if matches!(cmd, MemberCommand::Stop) && crate::traverse::refuses_now(self) {
            self.finish_command(true);
            return;
        }
        if crate::traverse::is_traverse(self.state) && !keeps {
            crate::traverse::end(self);
            if !matches!(cmd, MemberCommand::Stop) {
                self.enter(NpcStateId::Idle);
            }
        }
        if keeps && matches!(cmd, MemberCommand::MoveTo { .. }) {
            return;
        }
        match cmd {
            MemberCommand::Stop => {
                self.finish_command(true);
                if !self.state.refuses_commands() || matches!(self.state, NpcStateId::Fr(_) | NpcStateId::Ladder) { // (LADDER2) Stop ends the ladder jump
                    // AbortTask 0x140492CB0 cancels the running action (stream C relay): the
                    // definition's movement overrides go (gravity scale of the jump, Tornado's
                    // hover / Turbo's drive); a member in the air falls.
                    if self.state == NpcStateId::Action {
                        self.end_action();
                        self.cmc.gravity_scale = self.params.gravity_scale;
                        self.cmc.direct_xy = None;
                        self.cmc.direct_z = None;
                        if !self.on_ground() {
                            self.cmc.set_falling();
                        }
                    }
                    self.fr_drive = None;
                    self.to_idle();
                }
            }
            MemberCommand::MoveTo { walk, .. } => {
                let was_moving = self.state == NpcStateId::Move;
                if self.state != NpcStateId::Move {
                    self.enter(NpcStateId::Move);
                    let set = if self.in_battle { "RunLoopB" } else if walk { "WalkLoopN" } else { "RunLoopN" };
                    self.play(set, -1.0);
                }
                // (party K1) the start clip from standing; a move into the side he just gave up on fails
                if self.pad.is_none() {
                    crate::locomotion::begin_move(self, cx, host, was_moving);
                }
            }
            MemberCommand::TurnTo { goal } => self.begin_turn(goal, cx, host),
            MemberCommand::Action { def, target } => {
                let run = ActionRun::new(def, target);
                self.action = Some(run);
                self.hits_this_action = 0;
                self.enter(NpcStateId::Action);
                if !actions::begin(self, cx, host) {
                    self.action = None;
                    self.finish_command(false);
                    self.to_idle();
                }
            }
            MemberCommand::Cure { target } => {
                self.cmd = Some((self.cmd.unwrap().0, MemberCommand::Action { def: crate::ActionDef::DonaldCure, target }));
                self.begin_command(cx, host);
            }
            MemberCommand::UseItem { item, target } => {
                if self.status.items.iter().any(|s| s.present && s.item == Some(item)) {
                    self.enter(NpcStateId::UseItem);
                    // anim names 0x141263D30: ItemUse / ItemAirUse.
                    actions::set_item_voice(self, target, cx, host);
                    let clip = if self.on_ground() { "ItemUse" } else { "ItemAirUse" };
                    self.play(clip, -1.0);
                    self.action = Some(ActionRun::item(item, target));
                    self.cmc.stop_movement();
                } else {
                    self.finish_command(false);
                }
            }
            MemberCommand::Warp { goal, yaw } => {
                self.enter(NpcStateId::Warp);
                // Init 0x141266C10: +0x9C = random [0, 0.1); the request 0x14127B980: +0x88 = 2.0
                let wait = tables::param("warp.wait");
                let recheck = self.rng.unit() * tables::param("warp.recheck");
                // Begin 0x14125BAE0: +0xA0 = 1000 inside the friend component's battle-start window (+0x314 > 0),
                // else 3000 (no component: FLT_MAX; +0xF0 override unused).
                let window = self.in_battle && self.battle_time < tables::param("warp.battle_window");
                let threshold = tables::param(if window { "warp.threshold_battle" } else { "warp.threshold" });
                // in the air (FUN_1405AD790): the jump anim index 2 (fall loop), sub 0; else the idle clip, sub 2
                let sub = if self.on_ground() { 2 } else { 0 };
                self.play(if sub == 2 { self.idle_clip() } else { "JumpNormalDownLoop" }, -1.0);
                self.cmc.stop_movement();
                // +0x8C = +0x88; the EQS runs in C (the same query asset): its result is the command's goal
                self.warp = WarpRun { sub, wait, limit: wait, after_point: 0.0, recheck, threshold, found: false, point: goal, yaw };
            }
            MemberCommand::SetBattleFollowMode(on) => {
                self.battle_follow = on;
                self.finish_command(true);
            }
            MemberCommand::PlayMotion { name, .. } => {
                self.enter(NpcStateId::PlayMotion);
                self.play(name, -1.0);
            }
            MemberCommand::Idle { .. } => {
                if !self.state.refuses_commands() {
                    self.to_idle();
                }
            }
            MemberCommand::Focus(t) => {
                self.focus = t;
                self.finish_command(true);
            }
            MemberCommand::EnterFrState { state } => {
                // Stream D's FR states run in `team/`; the pawn only holds the state id.
                self.action = None;
                self.enter(NpcStateId::Fr(state));
            }
            // (LADDER2) port design: the ladder jump
            MemberCommand::LadderJump(jump) => crate::ladder::begin(self, jump),
        }
    }

    fn main_proc(&mut self, cx: &StepCtx, host: &mut PartyHost) {
        // (DON-KIT) while a spell cast holds Donald still the locomotion states do not tick (their clips and moves would
        // take over the cast's clip): Idle (its turn clips), the AI's Move, the pad's run
        if matches!(self.state, NpcStateId::Idle | NpcStateId::Move | NpcStateId::Pad(crate::pad::RUN)) && crate::pad::donald::cast_holds_still(self) {
            return;
        }
        // (party P)
        if self.pad.is_some() && crate::pad::owns(self) {
            return crate::pad::tick(self, cx, host);
        }
        let dt = cx.dt;
        match self.state {
            NpcStateId::Idle => {
                // (B5) UTresNpcState_Idle tick 0x14124CD40: not on the ground (FUN_1405AD850) -> RequestJump (iface
                // vt+0x510, the actor yaw) -> vt+0x270: the NavLinkJump fall (crate::jump).
                // (party I1) in water (pawn vt+0x1A80) -> RequestSwim (iface vt+0x548) -> the swim locomotion
                if crate::jump::in_water(self, host.world) && crate::jump::request_swim(self, host.world) {
                    return;
                }
                if !self.on_ground() {
                    crate::jump::request_fall(self, host.world);
                    return;
                }
                if let Some((_, MemberCommand::Idle { time })) = self.cmd {
                    if self.state_time >= time {
                        self.finish_command(true);
                    }
                }
                // (B4, C4 relay) IdleFinished from the idle tick 0x14124CD40: in battle (pawn +0x6E4 == 1) each
                // time the playing idle clip's length has run (the timer reset to that length), out of battle
                // when the idle clip ends.
                if self.in_battle {
                    if self.idle_timer <= 0.0 {
                        self.idle_timer = self.anim.clip.length.max(1.0e-3);
                    }
                    self.idle_timer -= dt;
                    if self.idle_timer <= 0.0 {
                        self.idle_finished();
                    }
                } else if (self.anim.finished || self.anim_wrapped) && self.loco.idle_turn.is_none() && self.loco.sub != 3 {
                    self.idle_finished();
                }
                // (party K1) the facing by the turn clip (KH3 0x140445880: 180 deg/s under the idle clip), the stop clip
                let focus = self.target_point(self.focus, cx, host);
                crate::locomotion::idle_tick(self, focus, dt);
                if self.in_battle && self.anim.clip.name == "IdleN" {
                    self.play("IdleB", -1.0);
                } else if !self.in_battle && self.anim.clip.name == "IdleB" {
                    self.play("IdleN", -1.0);
                }
            }
            NpcStateId::Move => {
                // (party K1) the hold after an arrival behind a running leader (crate::locomotion::arrive)
                if self.cmd.is_none() && self.pad.is_none() && crate::locomotion::hold_tick(self, cx) {
                    return;
                }
                let Some((_, MemberCommand::MoveTo { goal, locomotion, walk, acceptable_radius, .. })) = self.cmd else {
                    self.to_idle();
                    return;
                };
                let Some(p) = self.goal_point(goal, cx, host) else {
                    self.finish_command(false);
                    self.to_idle();
                    return;
                };
                let p = if self.pad.is_none() { crate::locomotion::live_goal(self, cx, p) } else { p }; // (FOL2 r3) the goal travels with a running leader
                let d = horizontal(p - self.position());
                // (party K1) KH3's stop at the path's end: the easing speed runs on before the arrival (crate::locomotion)
                // (party K1) the reach test: UE's path following counts the agent's radius in (bReachTestIncludesAgentRadius, on by
                // default for a MoveTo [inf: TresBTTask_MoveTo keeps the default]), and a goal passed within this step's
                // movement is reached (at 680 cm/s a step is 11 cm: with a 5 cm radius he stepped over it and back forever)
                let reached = if self.pad.is_none() { crate::locomotion::reached(self, d, acceptable_radius, cx.dt) } else { d.length() <= acceptable_radius.max(1.0) };
                let reached = reached && !(self.pad.is_none() && crate::locomotion::keeps_station(self, cx)); // (FOL2 r3)
                if self.pad.is_none() && (self.loco.decel_v > 0.0 || reached) && crate::locomotion::decelerate(self, cx) {
                    return;
                }
                if reached || std::mem::take(&mut self.loco.decel_done) {
                    self.finish_command(true);
                    if self.pad.is_none() { crate::locomotion::arrive(self, cx) } else { self.to_idle() } // (party K1) the stop clip
                    return;
                }
                let goal_dist = d.length(); // (party K1) the real goal's distance (the deceleration)
                // (party ER S1) the host's waypoint round what blocks the straight line (None: unchanged)
                let (p, d) = match self.steer {
                    Some((goal, via)) if horizontal(goal - p).length() <= STEER_GOAL_SLACK && horizontal(via - self.position()).length() > 1.0 => (via, horizontal(via - self.position())),
                    _ => (p, d),
                };
                // (party I1) the Land locomotion 0x1412194F0: with m_Swimming set and in water -> RequestSwim (iface vt+0x548)
                if crate::jump::in_water(self, host.world) && crate::jump::request_swim(self, host.world) {
                    return;
                }
                // (B4) the Run tick's probes (0x140442430): rail, hop point, slope -> their states
                if crate::traverse::move_checks(self, cx, host, d) {
                    return;
                }
                // (party K1) an AI member: KH3's FollowPlayer speed, the steering and the speed-matched loop (crate::locomotion)
                if self.pad.is_none() {
                    crate::locomotion::move_tick(self, cx, locomotion, walk, d, goal_dist);
                    return;
                }
                // Locomotion definition n_ex00x_Move1 (140..680) / Move_Slow (140) [A].
                let speed = match (locomotion, walk) {
                    (crate::LocomotionDef::MoveSlow, _) | (_, true) => tables::cparam(self.chara, "loco.move1.min_speed"),
                    _ => tables::cparam(self.chara, "loco.move1.max_speed"),
                };
                self.cmc.max_walk_speed = speed;
                let yaw0 = self.yaw;
                let want_delta = wrap180(yaw_of(d) - yaw0);
                self.turn_toward(p, dt, turn_rate(TurnKind::Locomotion));
                self.cmc.control_input = d.normalize_or_zero();
                let speed = horizontal(self.cmc.velocity).length();
                if matches!(self.anim.clip.name, "WalkRunN" | "WalkRunW" | "WalkRunB") {
                    // the 2D rows: UTresAICharState_Run 0x14045AAF0 -> anim vt+0x120(X, |Velocity|): X = the yaw
                    // delta move direction - facing before this tick's turn (deg), clamped to the row's X range
                    // (vt+0x128: WalkRunN BlendParameters[0], Donald -300..300, Goofy -180..180), Y = the speed.
                    self.anim.axis = walkrun_axis_x(self.chara, want_delta);
                    self.anim.axis_y = self.cmc.velocity.length();
                } else {
                    self.anim.axis = speed;
                    self.anim.axis_y = 0.0;
                }
            }
            NpcStateId::Turn => {
                let Some((_, MemberCommand::TurnTo { goal })) = self.cmd else {
                    self.to_idle();
                    return;
                };
                if self.turn.fixed {
                    // sub-step 3: vt+0xDE8 toward the goal at m_FixedPointTurnSpeed until it answers done.
                    match self.goal_point(goal, cx, host) {
                        Some(p) => {
                            if self.turn_toward(p, dt, turn_rate(TurnKind::FixedPoint)).abs() <= TURN_FIXED_DONE {
                                self.finish_command(true);
                                self.to_idle();
                            }
                        }
                        None => {
                            self.finish_command(false);
                            self.to_idle();
                        }
                    }
                    return;
                }
                // sub-steps 1 / 2: the clip's root yaw, scaled to the delta (no clamp: m_bUseFixedTurnMaxRotationSpeed off).
                let len = self.anim.clip.length.max(1e-4);
                let f = if self.anim.finished { 1.0 } else { (self.anim.clip.root_at(self.anim.time.min(len)).yaw / self.turn.clip_yaw).clamp(0.0, 1.0) };
                if f > self.turn.applied {
                    self.yaw = wrap180(self.yaw + self.turn.delta * (f - self.turn.applied));
                    self.turn.applied = f;
                }
                if self.anim.finished {
                    if self.turn.rest.abs() > 0.0 {
                        // the rest (+0xA8) is played again with the same clip
                        let rest = self.turn.rest;
                        self.start_turn_clip(rest);
                    } else {
                        self.finish_command(true);
                        self.to_idle();
                    }
                }
            }
            NpcStateId::Action => {
                if actions::tick(self, cx, host) {
                    let ok = self.hits_this_action > 0 || self.action.is_some_and(|a| a.success);
                    self.action = None;
                    self.finish_command(ok || true);
                    self.to_idle();
                }
            }
            NpcStateId::UseItem => {
                if self.anim.finished {
                    // (a clip without the Timing notify: the use at its end, no delay)
                    if let Some(run) = self.action.filter(|r| !r.success) {
                        if let Some(item) = run.item {
                            if self.status.take_item(item) {
                                actions::item_effect(self, item, run.target);
                                self.events.push(PartyEventKind::ItemUsed { item, target: run.target });
                            }
                        }
                    }
                    self.finish_command(true);
                    self.to_idle();
                }
            }
            NpcStateId::Warp => self.warp_tick(cx, host),
            NpcStateId::PlayMotion => {
                if self.anim.finished {
                    self.finish_command(true);
                    self.to_idle();
                }
            }
            NpcStateId::Die => self.die_tick(dt),
            // (B4)
            NpcStateId::SlopeSlide | NpcStateId::SuperSlide | NpcStateId::RailSlide | NpcStateId::Hopping | NpcStateId::SituationAction | NpcStateId::NavLinkJump | NpcStateId::Climbing | NpcStateId::Dangle => crate::traverse::tick(self, cx, host),
            NpcStateId::Damage => {}
            // (B5) TresNpcState_Jump (id 3) is never entered by the friends (RequestJump takes 0x2B)
            NpcStateId::Jump => {}
            NpcStateId::Reflect => self.reflect_tick(dt),
            // (party P)
            NpcStateId::Pad(_) => crate::pad::tick(self, cx, host),
            // (LADDER2) port design: the ladder jump
            NpcStateId::Ladder => crate::ladder::tick(self, dt),
            NpcStateId::BadStatStun | NpcStateId::BadStatSneeze | NpcStateId::BadStatFreeze | NpcStateId::BadStatBurn => self.bad_tick(dt),
            NpcStateId::Fr(_) => {
                // Stream D drives the pawn (team/mod.rs `FrDrive`): transform and motion.
                if let Some(d) = self.fr_drive.take() {
                    if let Some((pos, yaw)) = d.transform {
                        self.cmc.location = pos;
                        self.cmc.stop_movement();
                        self.yaw = yaw;
                    }
                    if let Some((name, blend, _looping)) = d.motion {
                        self.play(name, blend);
                    }
                    self.fr_visible = d.visible;
                }
            }
        }
    }

    /// The bad-status states. The status trigger requests the state of the kind on the pawn when it is free
    /// (after the damage reaction / at once without one) [inf: the trigger table is the CharPawnBase rule
    /// kh-core ports for Sora]. Stun (12): `TresNpcState_BadStatStun` Begin 0x141240EB0 -> the shared
    /// UTresCharState_BadStatStun Begin 0x141508930 (StopMovement, the start clip), Tick 0x14150EE70, End
    /// 0x141244320 -> 0x14150B820 + pawn vt+0x19D8(0); clips by 0x1412470D0 (BadStatStunStart / Loop / End,
    /// M_0241 / 42 / 43 when the set lacks them). Sneeze (13): Begin 0x141240DE0 plays BadStatSneeze (mode 2),
    /// Tick 0x1412495C0 ends the state when it is no longer playing or it has ended. Freeze (7): Begin
    /// 0x1404330F0 (StopMovement, Falling in the air), Tick 0x140440660: +0x88 counts down, the state ends
    /// when the status is no longer FREEZE or +0x88 runs out (+0x88 = the status time [inf]); the pose is held
    /// (pawn +0x868 bit 1, 0x1405B77B0).
    fn enter_bad_state(&mut self) {
        use kh_core::combat::badstat::kind;
        if self.state == NpcStateId::Die {
            return;
        }
        let (k, _) = self.bad;
        let st = match k {
            kind::STUN => NpcStateId::BadStatStun,
            kind::SNEEZE => NpcStateId::BadStatSneeze,
            kind::FREEZE => NpcStateId::BadStatFreeze,
            kind::BURN => NpcStateId::BadStatBurn,
            _ => return,
        };
        self.abort_command();
        self.end_action();
        self.action = None;
        self.cmc.stop_movement();
        self.enter(st);
        self.bad_sub = 0;
        match st {
            NpcStateId::BadStatStun => {
                let c = self.stun_clip(0);
                self.play(c, -1.0);
            }
            NpcStateId::BadStatSneeze => self.play("BadStatSneeze", -1.0),
            NpcStateId::BadStatBurn => {
                // Begin 0x141508620: MaxWalkSpeed / gravity scale of the burn, the Start clip
                self.burn_hops = 0;
                self.cmc.max_walk_speed = tables::param("damage.burn_move_speed");
                self.cmc.gravity_scale = tables::param("damage.burn_gravity_scale");
                self.play("BadStatBurnStart", -1.0)
            }
            _ => {
                if !self.on_ground() {
                    self.cmc.set_falling();
                }
                self.anim.speed = 0.0;
            }
        }
    }

    /// Burn (17): TresNpcState_BadStatBurn Begin 0x141240DC0 -> the shared UTresCharState_BadStatBurn Begin
    /// 0x141508620 (yaw-only rotation, StopMovement, MaxWalkSpeed = the move speed getter, gravity scale = the
    /// gravity getter (vt+0x1650), no damage response on (vt+0x2078(1)), `BadStatBurnStart` mode 2), End
    /// 0x141244310 -> 0x14150B720; the hop parameters by the NPC getters 0x141246730 (height max = the damage
    /// param +0xC0) / 0x1412467B0 (min +0xC4) over FriendDamageCommonParam (move 300, gravity scale 2, height
    /// 200 / 200; sheet damage.burn_*); clips by 0x141247060 (0 Start, 2 End, else Loop). The Start clip's
    /// JumpStart notify launches the hop (the shared base's launch as kh-core ports it for Sora, 0x141512610:
    /// each hop after the first costs Ceil(5 % max HP)). The NPC Tick 0x141248FC0 (B4):
    /// - sub 0 / 1: the clip's end -> a random yaw r in [-180, 180) (the pawn RNG; game: the global FRand
    ///   stream FUN_142CFF460), taken as it is within 70 deg of the facing, else the facing +-70; the Loop clip
    ///   (mode 2); sub 2;
    /// - sub 2 in the air: AddMovementInput(facing), vt+0xDB0 toward that yaw with alpha dt x 3;
    ///   on the ground: the status no longer BURN (FUN_1405A9DD0 != 0x11) -> StopMovement, the jump input cleared
    ///   (vt+0x1098), the End clip, m_bEnableChange, sub 3, no damage response off; still burning: wait while
    ///   Velocity.Z > 0 (vt+0x1568), then StopMovement, the jump input cleared, the Start clip again unless it
    ///   is already playing, sub 0;
    /// - sub 3: the clip's end -> the state ends (vt+0x270).
    fn burn_launch(&mut self) {
        let h = tables::param("damage.burn_jump_height_max");
        let gs = tables::param("damage.burn_gravity_scale");
        self.cmc.gravity_scale = gs;
        let g = self.cmc.gravity_z(&self.params).abs();
        if self.burn_hops > 0 {
            let cost = (self.status.max_hp() as f32 * 0.05).ceil() as i32;
            self.status.hp = (self.status.hp - cost).max(0);
        }
        self.burn_hops = self.burn_hops.saturating_add(1);
        self.cmc.jump_z_velocity = (2.0 * g * h).sqrt();
        self.cmc.do_jump();
    }

    fn burn_tick(&mut self, dt: f32) {
        use kh_core::combat::badstat::kind;
        match self.bad_sub {
            0 | 1 => {
                if self.anim.finished {
                    let r = self.rng.unit() * 360.0 - 180.0;
                    let d = wrap180(r - self.yaw);
                    let lim = 70.0;
                    self.burn_yaw = if d.abs() > lim { wrap180(self.yaw + lim * d.signum()) } else { r };
                    self.play("BadStatBurnLoop", -1.0);
                    self.bad_sub = 2;
                }
            }
            2 => {
                if !self.on_ground() {
                    self.cmc.max_walk_speed = tables::param("damage.burn_move_speed");
                    self.cmc.control_input = forward(self.yaw);
                    let a = (dt * 3.0).clamp(0.0, 1.0);
                    self.yaw = wrap180(self.yaw + wrap180(self.burn_yaw - self.yaw) * a);
                    return;
                }
                if self.status.hp < 1 {
                    self.cmc.gravity_scale = self.params.gravity_scale;
                    self.start_die();
                    return;
                }
                if self.bad.0 != kind::BURN {
                    self.cmc.stop_movement();
                    self.cmc.control_input = Vec3::ZERO;
                    self.cmc.stop_jumping();
                    self.play("BadStatBurnEnd", -1.0);
                    self.bad_sub = 3;
                    return;
                }
                if self.cmc.velocity.z > 0.0 {
                    return;
                }
                self.cmc.stop_movement();
                self.cmc.control_input = Vec3::ZERO;
                self.cmc.stop_jumping();
                if self.anim.clip.name != "BadStatBurnStart" {
                    self.play("BadStatBurnStart", -1.0);
                }
                self.bad_sub = 0;
            }
            _ => {
                if self.anim.finished {
                    self.cmc.gravity_scale = self.params.gravity_scale;
                    self.to_idle();
                }
            }
        }
    }

    /// 0x1412470D0: index 0 / 1 / 2 = Start / Loop / End, else the M_024x rows.
    fn stun_clip(&self, i: usize) -> &'static str {
        let set = tables::anim_set(self.chara);
        let (a, b) = [("BadStatStunStart", "M_0241"), ("BadStatStunLoop", "M_0242"), ("BadStatStunEnd", "M_0243")][i.min(2)];
        if set.get(a).is_some() { a } else { b }
    }

    fn bad_tick(&mut self, dt: f32) {
        use kh_core::combat::badstat::kind;
        // the status time counts down while its state runs (as kh-core's shared rule for Sora)
        if self.bad.0 != 0 {
            self.bad.1 -= dt;
            if self.bad.1 <= 0.0 {
                self.bad = (0, 0.0);
            }
        }
        match self.state {
            NpcStateId::BadStatStun => match self.bad_sub {
                0 if self.anim.finished => {
                    let c = self.stun_clip(1);
                    self.play(c, -1.0);
                    self.bad_sub = 1;
                }
                1 if self.bad.0 != kind::STUN => {
                    let c = self.stun_clip(2);
                    self.play(c, -1.0);
                    self.bad_sub = 2;
                }
                2 if self.anim.finished => self.to_idle(),
                _ => {}
            },
            NpcStateId::BadStatBurn => self.burn_tick(dt),
            NpcStateId::BadStatSneeze => {
                if self.anim.clip.name != "BadStatSneeze" || self.anim.finished {
                    // one sneeze per application [inf: the sneeze status is not re-triggered here]
                    if self.bad.0 == kind::SNEEZE {
                        self.bad = (0, 0.0);
                    }
                    self.to_idle();
                }
            }
            _ => {
                if self.bad.0 != kind::FREEZE {
                    self.anim.speed = 1.0;
                    self.to_idle();
                }
            }
        }
    }

    /// UTresAICharState_Reflect (Init 0x14043E620: +0xA0 = 750, +0xA4 = 3072; Tick 0x140441FF0; NPC Tick
    /// 0x14124F080 adds the turn to the causer when m_bEnableTurnToTarget): sub-step 0 plays the clip of
    /// vt+0x1CA8 = 0x141264270 (AttackReflect, AttackAirReflect in the air when the set has it) with blend 0.1,
    /// the knock direction from the causer, StopMovement, MaxWalkSpeed 750, the move along it (vt+0x1668) with
    /// braking deceleration 3072 and no ground friction; the clip's end ends the state. [inf: vt+0x1668 read
    /// as a launch at the max walk speed; the turn-to-target flag is off (not set by the friends)]
    fn begin_reflect(&mut self, at: Vec3) {
        self.end_action();
        self.action = None;
        self.abort_command();
        self.enter(NpcStateId::Reflect);
        let set = tables::anim_set(self.chara);
        let clip = if !self.on_ground() && set.get("AttackAirReflect").is_some() { "AttackAirReflect" } else { "AttackReflect" };
        self.play(clip, 0.1);
        self.reflect_dir = horizontal(self.position() - at).normalize_or_zero();
        if self.reflect_dir == Vec3::ZERO {
            self.reflect_dir = -forward(self.yaw);
        }
        self.cmc.stop_movement();
        let v = self.reflect_dir * tables::param("reflect.speed");
        self.cmc.set_velocity_xy(Vec2::new(v.x, v.y));
    }

    fn reflect_tick(&mut self, dt: f32) {
        let s = (horizontal(self.cmc.velocity).length() - tables::param("reflect.braking") * dt).max(0.0);
        let v = self.reflect_dir * s;
        self.cmc.set_velocity_xy(Vec2::new(v.x, v.y));
        if self.anim.finished {
            self.cmc.direct_xy = None;
            self.cmc.stop_movement();
            self.to_idle();
        }
    }

    /// The idle clip (vt+0x1C68 with the battle flag).
    fn idle_clip(&self) -> &'static str {
        if self.in_battle { "IdleB" } else { "IdleN" }
    }

    /// `UTresNpcState_Warp` Tick 0x14126DD90 (there is no fade: the friend is moved while neither he nor the
    /// point is on screen):
    /// - the pose: sub 0 (air) lands -> the jump anim index 4 (Land), 1; 1 its end -> idle, 2; 2 falling ->
    ///   the fall loop, 0;
    /// - every 0.1 s (+0x9C): a path to the leader shorter than +0xA0 ends the state, no warp (the path:
    ///   `leader_path_length`, FindPathSync; no path found -> no end);
    /// - +0x8C counts down: out -> the state ends, no warp;
    /// - no point yet: the EQS (FUN_143C5A420 with the friend data's +0x60 query) -> the callback
    ///   0x141274600 keeps the point when FUN_1412189C0 allows it: +0x90 = 1, +0x98 = +0x88, +0x8C = 2 x +0x88;
    /// - the point: +0x98 counts down; then the friend's bounds sphere (max(radius, half height)) at his
    ///   bounds centre and the same sphere at the point + Z x radius must both be out of the camera
    ///   (FUN_140E5AC70); then vt+0x4D0 teleports to the point facing the leader, and the state ends.
    fn warp_tick(&mut self, cx: &StepCtx, _host: &mut PartyHost) {
        let dt = cx.dt;
        let fall = !self.on_ground();
        match self.warp.sub {
            0 if !fall => {
                self.play("Land", -1.0);
                self.warp.sub = 1;
            }
            1 if self.anim.finished => {
                self.play(self.idle_clip(), -1.0);
                self.warp.sub = 2;
            }
            2 if fall => {
                self.play("JumpNormalDownLoop", -1.0);
                self.warp.sub = 0;
            }
            _ => {}
        }
        self.warp.recheck -= dt;
        if self.warp.recheck <= 0.0 {
            self.warp.recheck = tables::param("warp.recheck");
            if leader_path_length(self.position(), cx.sora.position).is_some_and(|l| l < self.warp.threshold) {
                self.end_warp(false, true);
                return;
            }
        }
        self.warp.limit -= dt;
        if self.warp.limit <= 0.0 {
            self.end_warp(false, false);
            return;
        }
        if !self.warp.found {
            // the query's answer (one step later in the game: asynchronous EQS) [inf: FUN_1412189C0 passes]
            self.warp.found = true;
            self.warp.after_point = self.warp.wait;
            self.warp.limit = self.warp.wait * 2.0;
            return;
        }
        if self.warp.after_point > 0.0 {
            self.warp.after_point -= dt;
            return;
        }
        let r = self.params.capsule_radius.max(self.params.capsule_half_height);
        if let Some(cam) = cx.sora.camera {
            if cam.sees(self.capsule_centre(), r) || cam.sees(self.warp.point + Vec3::Z * r, r) {
                return;
            }
        }
        let from = self.position();
        let to = self.warp.point;
        // rotation: from the point toward the leader's root (yaw only)
        let d = horizontal(cx.sora.position - to);
        self.yaw = if d.length_squared() > 1e-4 { yaw_of(d) } else { self.warp.yaw };
        self.cmc.location = to;
        self.cmc.stop_movement();
        self.events.push(PartyEventKind::Warped { from, to });
        self.end_warp(true, true);
    }

    /// The state's end (vt+0x270; End 0x14125E7C0 aborts a pending query, clears pawn[1]+0x1D5).
    fn end_warp(&mut self, warped: bool, ok: bool) {
        let _ = warped;
        self.finish_command(ok);
        self.to_idle();
    }

    /// `UTresNpcState_Die` Tick 0x141249F80 / End 0x141244670.
    fn die_tick(&mut self, dt: f32) {
        self.die.timer -= dt;
        if self.die.timer <= 0.0 {
            self.die.timer = 0.0;
            if self.status.hp < 1 {
                // vt+0x2198 (0x141264140) amount through vt+0x1878 (recover).
                self.status.hp = self.status.revive_hp();
            }
        }
        match self.die.sub {
            0 => {
                if self.anim.finished {
                    self.play("BadStatStunLoop", -1.0);
                    self.die.sub = 1;
                }
            }
            1 => {
                if self.status.hp > 0 {
                    self.play("BadStatStunEnd", -1.0);
                    self.die.sub = 2;
                }
            }
            2 => {
                if self.anim.finished {
                    // End 0x141244670: invincibility off, collision on.
                    self.events.push(PartyEventKind::Revived);
                    self.to_idle();
                }
            }
            _ => {
                // 3: KO'd in the air: wait for the ground (or HP), then the down clip (index 0).
                if self.on_ground() || self.status.hp > 0 {
                    self.play("BadStatStunStart", -1.0);
                    self.die.sub = 0;
                }
            }
        }
    }

    /// `ATresNpcPawnBase::CanBattle` (exec 0x1427E8F00 -> vt+0x2880 = 0x141257340): false while the
    /// disable byte +0xCA9 is set or the pawn is in a ride state (FUN_140545FD0, not modelled: no rides);
    /// otherwise the battle flag (vt+0x1740 = bit 1 of +0x630) once the latch +0xCA8 is set, which
    /// happens when the battle has lasted more than 0.5 s. Out of battle: false. The KO state is not
    /// part of the rule.
    pub fn can_battle(&self) -> bool {
        self.in_battle && self.can_battle_latch
    }

    pub fn revive_left(&self) -> f32 {
        if self.state == NpcStateId::Die {
            self.die.timer.max(0.0)
        } else {
            0.0
        }
    }

    pub fn view(&self) -> MemberView {
        let (prev_clip, prev_seq, prev_time) = self.anim.prev.map(|(c, t)| (c.name, c.sequence, t)).unwrap_or(("", "", 0.0));
        MemberView {
            id: self.id,
            chara: self.chara,
            state: self.state,
            state_name: self.state.class(),
            position: self.position(),
            yaw: self.yaw,
            velocity: self.cmc.velocity,
            on_ground: self.on_ground(),
            hp: self.status.hp,
            hp_max: self.status.max_hp(),
            mp: self.status.mp,
            mp_max: self.status.max_mp(),
            mp_charging: self.status.mp_charge_left > 0.0,
            mp_charge: if self.status.mp_charge_left > 0.0 { 1.0 - self.status.mp_charge_left / tables::cparam(self.chara, "mp_charge_time") } else { 0.0 },
            level: self.status.level,
            ko: self.state == NpcStateId::Die,
            revive_left: self.revive_left(),
            in_battle: self.in_battle,
            swimming: false,
            ride_on: false,
            can_battle: self.can_battle(),
            items: self.status.items,
            command: self.last,
            cooldowns: Default::default(),
            anim: AnimView { clip: self.anim.clip.name, sequence: self.anim.clip.sequence, time: self.anim.time, weight: self.anim.weight, prev_clip, prev_sequence: prev_seq, prev_time, blend_axis: self.anim.axis, blend_axis_y: self.anim.axis_y, face: self.face.view() /* (ZETTA) */ },
            capsule: (self.params.capsule_radius, self.params.capsule_half_height),
            // (B5)
            pitch: self.trav.pitch,
            focus: self.focus, // (party I2) the controller focus for the head look-at
        }
    }

    /// Velocity in the plane (for the AI and the HUD).
    pub fn speed2d(&self) -> f32 {
        Vec2::new(self.cmc.velocity.x, self.cmc.velocity.y).length()
    }
}

/// (B4) The 2D WalkRun rows' blend X (UTresAICharState_Run 0x14045AAF0): the yaw delta (deg) between the move
/// direction and the facing, clamped to the row's X range (anim vt+0x128; WalkRunN BlendParameters[0]).
pub fn walkrun_axis_x(chara: Chara, yaw_delta: f32) -> f32 {
    yaw_delta.clamp(tables::cparam(chara, "walkrun.x_min"), tables::cparam(chara, "walkrun.x_max"))
}

/// (B4) The Warp tick's path to the leader (0x14126DD90): FindPathSync (0x143EC6010) from the friend's location to
/// the leader's with UNavFilter_AIControllerDefault (query FUN_143E8C810); on success (result 3) the path's length
/// (path vt+0x40 from point 0), else None (the warp is not ended). The port asks the same nav stand-in the AI
/// uses (`kh_ai::Nav::path_length` of `StraightNav`, the app's nav: the straight distance; no nav mesh).
pub fn leader_path_length(from: Vec3, to: Vec3) -> Option<f32> {
    #[cfg(feature = "ai")]
    {
        use kh_ai::Nav;
        kh_ai::StraightNav { floor_z: None }.path_length(from, to)
    }
    #[cfg(not(feature = "ai"))]
    {
        Some(from.distance(to))
    }
}

/// The 4-way index of a hit direction against the actor's yaw (front 0, back 1, left 2, right 3), as
/// Sora's damage state (kh-core states/damage.rs `direction_index`, the same rule for the NPC pawns).
pub fn damage_dir_index(dir: Vec3, actor_yaw: f32) -> u8 {
    if Vec2::new(dir.x, dir.y).length_squared() < 1.0e-8 {
        return 0;
    }
    let delta = wrap180(yaw_of(-dir) - actor_yaw);
    if (-45.0..=45.0).contains(&delta) {
        0
    } else if delta > 45.0 && delta <= 135.0 {
        2
    } else if (-135.0..-45.0).contains(&delta) {
        3
    } else {
        1
    }
}

/// The friend's damage clip of a reaction phase, by the NPC pawn's anim-name vfuncs (each falls back
/// to its index-0 row when the set lacks the row, FUN_1405ACE80):
/// - Small (vt+0x1CB0 = 0x1412646C0): ground M_0201 + index (variant M_0205 / M_0206 for 0 / 1, not
///   used by kh-react), air M_0207 + index;
/// - Blow (vt+0x1CB8 = 0x14125FC00): M_0211 + 3 x index + phase (0 START, 1 the flight LOOP, 2 landing + lying + get-up
///   in one clip; (GOOFY-HR) it was read here as "0 thrown, 1 down, 2 get up": the lying showed the flight loop frozen);
/// - (GOOFY-HR) the pawn plays the reaction by kh-react's clip family, not by this phase summary: crate::hurt::pick;
/// - Wall (vt+0x1CC8 = 0x141262420): phase 0 M_0223 (index 0, 2, 3) / M_0224 (index 1), else M_0211;
///   phase 1 JumpNormalDownLoop, phase 2 Land;
/// - the plain fall of the air flinch: JumpNormalDownLoop / Land (UTresAICharState_Damage 0x140449060).
/// Frozen / Stunned keep the bad-status loop (their own states).
pub fn damage_clip(chara: Chara, phase: Phase, index: u8) -> &'static str {
    let set = tables::anim_set(chara);
    let pick = |name: &'static str, fallback: &'static str| if set.get(name).is_some() { name } else { fallback };
    const SMALL_GROUND: [&str; 4] = ["M_0201", "M_0202", "M_0203", "M_0204"];
    const SMALL_AIR: [&str; 4] = ["M_0207", "M_0208", "M_0209", "M_0210"];
    const BLOW: [[&str; 3]; 4] = [["M_0211", "M_0212", "M_0213"], ["M_0214", "M_0215", "M_0216"], ["M_0217", "M_0218", "M_0219"], ["M_0220", "M_0221", "M_0222"]];
    let i = (index as usize).min(3);
    match phase {
        Phase::FlinchGround | Phase::None => pick(SMALL_GROUND[i], "M_0201"),
        Phase::FlinchAir => pick(SMALL_AIR[i], "M_0207"),
        Phase::Falling => "JumpNormalDownLoop",
        Phase::LandFeet => "Land",
        Phase::BlownFlying | Phase::SpunAway | Phase::LaunchedRising | Phase::LaunchedHover | Phase::BlownFalling | Phase::LaunchedFalling | Phase::SlammedDown => pick(BLOW[i][0], BLOW[0][0]),
        Phase::Down => pick(BLOW[i][2], BLOW[0][2]), // (GOOFY-HR) the down clip, not the flight loop
        Phase::GettingUp => pick(BLOW[i][2], BLOW[0][2]),
        Phase::WallHit => pick(if i == 1 { "M_0224" } else { "M_0223" }, "M_0211"),
        Phase::Frozen | Phase::Stunned => "BadStatStunLoop",
    }
}

/// Damage of a friend's hit on a target (`CalcDamage` 0x140632240 with the friend as attacker).
pub fn friend_damage(status: &FriendStatus, row: &tables::AttackRow, defender: &kh_core::status::Defender) -> f32 {
    let attribute = kh_core::enums::ETresDamageAttribute::from_game_name(row.attribute).unwrap_or(kh_core::enums::ETresDamageAttribute::Physical);
    let magic = row.magic || attribute != kh_core::enums::ETresDamageAttribute::Physical;
    let mut attack = if magic { DamageAttack::magic(row.power, attribute) } else { DamageAttack::physical(row.power) };
    attack.recover = row.damage_kind == "TRES_DMG_KIND_RECOVER";
    compute_damage(&status.as_attacker(), defender, &attack, &DamageContext::new(status.game_level, 1)).amount
}
