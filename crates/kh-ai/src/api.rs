//! The seam between the AI and its hosts (contract C of `docs/party-contracts.md`).
//!
//! kh-ai does not depend on kh-party or kh-core: a pawn is driven through [`AiBody`] (stream B
//! implements it in `kh-party/src/ai_body.rs` over `Party::view` / `function` / `command`), the
//! world is read through [`AiWorld`] (the app builds it from Sora's output, the party views and the
//! host's targets), and navigation through [`Nav`] (open note N1: the test level has no navmesh).
//!
//! Units and axes are KH3's: cm, seconds, degrees, X forward, Y right, Z up, yaw from +X to +Y.
//! Every answer feeds the simulation: hosts keep them deterministic (same order, same values).

use glam::Vec3;

/// An actor as the AI names it. `Enemy` carries the host's target handle (`kh_core::TargetId.0`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum ActorRef {
    #[default]
    None,
    /// The player pawn (Sora).
    Player,
    /// A party member by its `kh_party::MemberId.0`.
    Member(u8),
    /// A host target (enemy, training dummy, gimmick) by `TargetId.0` (never 0).
    Enemy(u32),
}

impl ActorRef {
    pub fn is_none(self) -> bool {
        matches!(self, ActorRef::None)
    }
}

/// `ETresChrUniqueID` of the two friends [A].
pub const UID_DONALD: i32 = 11;
pub const UID_GOOFY: i32 = 12;

/// What the AI reads of any actor (enemy, ally, the player).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActorView {
    pub actor: ActorRef,
    /// `ETresChrUniqueID` for the player / friends, `ETresEnemyUniqueID` for enemies (-1 unknown).
    pub uid: i32,
    /// Actor origin (foot level), cm.
    pub position: Vec3,
    /// Centre of its bounds (lock-on point by default).
    pub center: Vec3,
    pub yaw: f32,
    pub velocity: Vec3,
    /// Bounds as an upright capsule (radius, half height) about `center`.
    pub radius: f32,
    pub half_height: f32,
    pub alive: bool,
    /// Hostile to the player's team (`IsHostileTarget`).
    pub hostile: bool,
    /// It can be locked on to (has lock-on target components).
    pub lockable: bool,
    /// Flying enemy (`TresNpcBTDecorator_IsFlyingEnemy`).
    pub flying: bool,
    /// Off the ground now.
    pub airborne: bool,
    /// Gimmick obstacle (EQS `TresEnvQueryContext_GimmickObstacles`).
    pub gimmick: bool,
    pub hp: f32,
    pub hp_max: f32,
    /// `ETresStateID` name of its current state when the host knows it ("" otherwise): the AI's
    /// `TresStateCheck` decorators read this for the player / allies (e.g. "TSID_STATE_IDLE").
    pub state: &'static str,
    /// Resistance per `ETresDamageAttribute` (index = enum value; 1.0 = normal, 0 = immune; the
    /// enemy's `m_DamageRate*` columns). `IsTargetAttrResistKind` reads it.
    pub resist: [f32; 12],
}

impl Default for ActorView {
    fn default() -> ActorView {
        ActorView {
            actor: ActorRef::None,
            uid: -1,
            position: Vec3::ZERO,
            center: Vec3::ZERO,
            yaw: 0.0,
            velocity: Vec3::ZERO,
            radius: 34.0,
            half_height: 90.0,
            alive: true,
            hostile: false,
            lockable: false,
            flying: false,
            airborne: false,
            gimmick: false,
            hp: 1.0,
            hp_max: 1.0,
            state: "",
            resist: [1.0; 12],
        }
    }
}

/// What the AI reads of the player beyond [`ActorView`].
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct PlayerView {
    pub actor: ActorView,
    /// Floor under the player (the EQS context `PlayerFloorPositionFromNavmeshAndCollisionCheck`).
    pub floor_position: Vec3,
    pub on_ground: bool,
    pub hp: i32,
    pub hp_max: i32,
    pub mp: i32,
    pub mp_max: i32,
    pub dead: bool,
    /// The game is in battle mode (enemies engaged: BGM / HUD battle state).
    pub in_battle: bool,
    /// `IsPlayerAttackNow`: the player is in an attack state.
    pub attacking: bool,
    /// `IsPlayerBeingAttackedByEnemy`.
    pub attacked_by_enemies: bool,
    /// The player's lock-on target (`ActorRef::None` when none).
    pub lock_on: ActorRef,
    pub swimming: bool,
    /// Riding a Gigas / vehicle (`PlayerRideGigas`): never in the test level.
    pub ride_on: bool,
    /// Flowmotion / free-flow active (the friends follow by warping).
    pub flowmotion: bool,
    /// `IsInHoldCameraTrickAction` (photo mode).
    pub hold_camera_trick: bool,
}

/// The player camera (EQS contexts `PlayerCamera`, `IsInViewport`).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct CameraView {
    pub position: Vec3,
    /// Pitch / yaw / roll in degrees (UE `FRotator` order).
    pub pitch: f32,
    pub yaw: f32,
    /// Horizontal field of view, degrees.
    pub fov: f32,
    /// Viewport aspect (width / height).
    pub aspect: f32,
}

/// The pawn as the AI reads it (B fills it from `MemberView`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyView {
    pub member: u8,
    /// `ETresChrUniqueID` (11 Donald, 12 Goofy).
    pub uid: i32,
    pub position: Vec3,
    pub yaw: f32,
    pub velocity: Vec3,
    pub on_ground: bool,
    /// Capsule radius / half height.
    pub radius: f32,
    pub half_height: f32,
    /// `ETresStateID` name of the running NPC state ("TSID_STATE_IDLE", "TSID_STATE_AI_MOVE",
    /// "TSID_STATE_TURN", "TSID_STATE_DAMAGE", ...): `TresBTDecorator_TresStateCheck`.
    pub state: &'static str,
    pub hp: i32,
    pub hp_max: i32,
    pub mp: i32,
    pub mp_max: i32,
    pub mp_charging: bool,
    pub ko: bool,
    pub in_battle: bool,
    pub swimming: bool,
    pub ride_on: bool,
    pub can_battle: bool,
    /// The pawn is in a damage reaction (MoveTo `m_bFailOnDamageReaction`).
    pub in_damage_reaction: bool,
    /// Item slots: `FriendEvaluationData` row names ("POTION", "ETHER", ...) of the items still
    /// held, `None` for empty / used slots.
    pub items: [Option<&'static str>; 4],
}

impl Default for BodyView {
    fn default() -> BodyView {
        BodyView {
            member: 0,
            uid: UID_DONALD,
            position: Vec3::ZERO,
            yaw: 0.0,
            velocity: Vec3::ZERO,
            on_ground: true,
            radius: 34.0,
            half_height: 70.0,
            state: "TSID_STATE_IDLE",
            hp: 1,
            hp_max: 1,
            mp: 0,
            mp_max: 0,
            mp_charging: false,
            ko: false,
            in_battle: false,
            swimming: false,
            ride_on: false,
            can_battle: false,
            in_damage_reaction: false,
            items: [None; 4],
        }
    }
}

/// Where a move / turn goes; moving goals are re-read by the body every step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AiGoal {
    Point(Vec3),
    Actor(ActorRef),
}

/// A command a BT task gives the pawn (maps 1:1 onto `kh_party::MemberCommand`).
#[derive(Clone, Debug, PartialEq)]
pub enum AiCommand {
    /// `TresBTTask_MoveTo` / `TresNpcBTTask_NpcMoveTo`. `locomotion` is the
    /// `TresLocomotionDefinitionBase` asset name (e.g. "n_ex001_Move1", "n_ex001_Move_Slow",
    /// "n_ex001_FollowPlayer"); B maps it to `LocomotionDef`.
    MoveTo { goal: AiGoal, locomotion: String, walk: bool, acceptable_radius: f32, follow_path: bool },
    /// `TresBTTask_TurnTo`.
    TurnTo { goal: AiGoal },
    /// `TresBTTask_Action`: the action-definition asset name without "_C"
    /// (e.g. "n_ex001_Attack4_MagFire"; B: `ActionDef::from_asset(chara, name minus "n_ex00x_")`).
    Action { def: String, target: ActorRef, location: Option<Vec3> },
    /// `TresNpcBTTask_NpcUseItem`: a `FriendEvaluationData` row name ("POTION" ...).
    UseItem { item: &'static str, target: ActorRef },
    /// `TresNpcBTTask_NpcCureAction` (Donald's Cure on `target`).
    Cure { target: ActorRef },
    /// Warp next to the player (`TresNpcState_Warp`).
    Warp { goal: Vec3, yaw: f32 },
    /// `TresNpcBTTask_SetBattleFollowMode`.
    SetBattleFollowMode(bool),
    /// Look-at / focus target of the controller (`TresBTService_GameplayFocus`); the body turns
    /// its idle facing to it. `ActorRef::None` clears it. Instant (no ticket wait).
    Focus(ActorRef),
    /// `TresBTTask_Idle`: stand (battle idle when in battle) for `time` seconds (<= 0: one step).
    Idle { time: f32 },
    /// Abort the running command, back to Idle.
    Stop,
}

/// What became of a command (the BT task's `EBTNodeResult`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AiCommandStatus {
    Running,
    Succeeded,
    Failed,
    Aborted,
    #[default]
    Unknown,
}

/// The pawn side (stream B implements it).
pub trait AiBody {
    fn view(&self) -> BodyView;
    /// A blueprint function / property by name, for `TresBTDecorator_BoolPropertyCheck` and
    /// `ArithmeticComparison` with `ActorFunction` sources on SelfActor: `CanBattle`, `IsRideOn`,
    /// `IsSwimming`, `IsStateMoving`, `GetMagicPoint`, `IsBattleFollowMode`, `IsActionOff`,
    /// `IsBreakPoseNow`, `CanbeSwirlUpDamageReactionByEnemy`, `CanLead`, `CanNpcMode` ... (bools as
    /// 0 / 1). `None` = the body does not know the name (the AI then answers from its own
    /// fallback, see `nodes::functions`).
    fn function(&self, name: &str) -> Option<f32>;
    /// Issue a command; returns its ticket (counts up from 1).
    fn command(&mut self, cmd: AiCommand) -> u32;
    fn command_status(&self, ticket: u32) -> AiCommandStatus;
    /// (party B5, added with a default) The pawn's own state asks for the friend data's warp query
    /// (`TresFriendData.WarpQuery` +0x60): the NPC RailSlide tick 0x14124E890 runs it through
    /// FEnvQueryRequest::Execute 0x143C5A420 (SingleResult, querier = the pawn) and keeps the best item in its
    /// delegate 0x141250A00 (the off-rail warp). The controller runs the query in its tick while this answers
    /// true and hands the result to `warp_query_result`. Default: never asks.
    fn wants_warp_query(&self) -> bool {
        false
    }
    /// (party B5, added with a default) The query's best item location (`None`: no item passed).
    fn warp_query_result(&mut self, best: Option<Vec3>) {
        let _ = best;
    }
}

/// Navigation (open note N1). The test level has no navmesh / Mercuna: `StraightNav` answers
/// with straight lines and floor projections; a host with collision implements the sweeps.
pub trait Nav {
    /// The floor under `p` within `up` above / `down` below (EQS projection, `Navigation` trace
    /// mode), `None` when there is no floor.
    fn project(&self, p: Vec3, up: f32, down: f32) -> Option<Vec3> {
        let _ = (up, down);
        Some(p)
    }
    /// A walkable path exists from `from` to `to` (EQS `Pathfinding` tests, `PathingGrid`):
    /// the stand-in says "a capsule sweep from `from` to `to` is clear".
    fn path_exists(&self, from: Vec3, to: Vec3) -> bool {
        let _ = (from, to);
        true
    }
    /// Path length (straight distance in the stand-in).
    fn path_length(&self, from: Vec3, to: Vec3) -> Option<f32> {
        if self.path_exists(from, to) {
            Some(from.distance(to))
        } else {
            None
        }
    }
    /// A visibility trace hits something between `from` and `to` (EQS `Trace` test).
    fn trace_blocked(&self, from: Vec3, to: Vec3) -> bool {
        let _ = (from, to);
        false
    }
    /// Distance from `p` to the nearest wall within `max` (EQS `WallDistance`), `None` if none.
    fn wall_distance(&self, p: Vec3, max: f32) -> Option<f32> {
        let _ = (p, max);
        None
    }
    /// The path points from `from` to `to` (C4: EQS `TresEnvQueryTest_DistanceToPath` 0x1404E0F50
    /// asks FindPathSync for every start / end pair and keeps the successful paths), `false`
    /// when no path is found. The stand-in: the straight segment when `path_exists`.
    fn path(&self, from: Vec3, to: Vec3, out: &mut Vec<Vec3>) -> bool {
        if self.path_exists(from, to) {
            out.push(from);
            out.push(to);
            true
        } else {
            false
        }
    }
    /// (FOL2) A friend walks straight from feet `from` to feet `to`, asked by a cast that is never answered optimistically
    /// (crate::formation: the follow spots' winner). None = not known (the stand-in: KH3's pick stands).
    fn reachable_strict(&self, from: Vec3, to: Vec3) -> Option<bool> {
        let _ = (from, to);
        None
    }
    /// (FOL2) A line from `from` to `to`: the distance to the first thing it meets (cm), None = nothing / not known
    /// (crate::formation's hallway probes).
    fn ray(&self, from: Vec3, to: Vec3) -> Option<f32> {
        let _ = (from, to);
        None
    }
}

/// A gimmick obstacle as `TresEnvQueryTest_ActorOverlap` (0x1404DCB50) reads one
/// `UTresGimmickObstacleComponent`: its location (+0x2B0, the cylinder's bottom), radius
/// (FUN_140f95760) and half height (FUN_140f95340); the test's cylinder is centred at
/// `base.z + half_height` (C4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GimmickObstacle {
    pub base: Vec3,
    pub radius: f32,
    pub half_height: f32,
}

/// The world side (the app implements it; the ER host later).
pub trait AiWorld {
    fn player(&self) -> PlayerView;
    fn camera(&self) -> CameraView;
    /// Every host target that is an enemy candidate (alive or dying), stable order.
    fn enemies(&self, out: &mut Vec<ActorView>);
    /// The party members (all of them, including the asking one), stable order.
    fn allies(&self, out: &mut Vec<ActorView>);
    /// One actor by reference (the player, a member or a target).
    fn actor(&self, actor: ActorRef) -> Option<ActorView>;
    /// `ETresGameLevel` of the save (0 Beginner .. 3 Critical; the coordinator curves).
    fn game_level(&self) -> u8 {
        1
    }
    fn nav(&self) -> &dyn Nav;
    /// Every gimmick obstacle component of the world (C4: the EQS context
    /// `TresEnvQueryContext_GimmickObstacles` 0x1404D5540 = the AI manager's obstacle actors, whose
    /// `UTresGimmickObstacleComponent`s `TresEnvQueryTest_ActorOverlap` with `bUseGimmickObstacle`
    /// reads). None in the test level.
    fn gimmick_obstacles(&self, out: &mut Vec<GimmickObstacle>) {
        let _ = out;
    }
    /// The volumes of an actor class as boxes (min, max): `TresEnvQueryTest_Volume` 0x1404EC590
    /// (C4: every actor of `VolumeClass`, e.g. "TresBattleVolume", whose bounds overlap the items;
    /// an item passes when it is inside one: bounds then `EncompassesPoint` — a box brush here).
    /// None in the test level.
    fn volumes(&self, class: &str, out: &mut Vec<(Vec3, Vec3)>) {
        let _ = (class, out);
    }
    /// The player's focus gauge (current, max): the cure decision's focus need
    /// (`FUN_141269f60`: player vt+0x1808 / vt+0x1830). None = not known (no focus need).
    fn player_focus(&self) -> Option<(i32, i32)> {
        None
    }
    /// Another party member's MP (current, max, recharging, seconds of recharge left): the cure
    /// decision's MP need for them (`FUN_14126af20` / `FUN_14126b550`). None = not known (no MP
    /// need, kh-ai's earlier behaviour).
    fn member_mp(&self, member: u8) -> Option<(i32, i32, bool, f32)> {
        let _ = member;
        None
    }
    /// The camera's far clip distance in cm (the frustum test of the warp rule, `FUN_140e5ac70`:
    /// C4 read it as the `APlayerCameraManager` CameraCache POV `m_FarClipPlane`, slot 257
    /// 0x144432390 = +0x7F0, filled from the view target's `UCameraComponent` (ctor 0x143FA3B70:
    /// near 3, far 2097152)). None = 2097152 cm.
    fn camera_far(&self) -> Option<f32> {
        None
    }
}

/// The straight-line navigation stand-in of open note N1: every path is clear, the floor is
/// the given height plane (or the point itself when `floor_z` is `None`).
#[derive(Clone, Copy, Debug, Default)]
pub struct StraightNav {
    pub floor_z: Option<f32>,
}

impl Nav for StraightNav {
    fn project(&self, p: Vec3, up: f32, down: f32) -> Option<Vec3> {
        match self.floor_z {
            None => Some(p),
            Some(z) if z <= p.z + up && z >= p.z - down => Some(Vec3::new(p.x, p.y, z)),
            Some(_) => None,
        }
    }
}

/// `ETresFNpcAIStyle_Battle` [A] (camp menu "Combat Style", UI spec 10.6; order-to-value [D]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BattleStyle {
    /// TOGETHER 0 "Stick By Sora" (いっしょにたたかえ).
    #[default]
    Together = 0,
    /// FREE 1 "Be Yourself" (じゆうにたたかえ).
    Free = 1,
    /// SUPPORT 2 "Hands Off" (ソラにまかせろ).
    Support = 2,
    /// FOCUS 3 "Finish the Job" (ねらいをさだめろ).
    Focus = 3,
}

/// `ETresFNpcAIStyle_Ability` [A] ("Ability Use").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AbilityStyle {
    /// FREE 0 "Go Wild".
    Free = 0,
    /// BALANCE 1 "Watch MP".
    #[default]
    Balance = 1,
    /// LESS 2 "Conserve".
    Less = 2,
}

/// `ETresFNpcAIStyle_Cure` [A] ("Recovery").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CureStyle {
    /// OFTEN 0 "Use Often".
    Often = 0,
    /// PINCH 1 "Use As Needed".
    #[default]
    Pinch = 1,
    /// ALWAYS 2 ... (third entry of the enum).
    Always = 2,
}

/// The friend AI settings of one member (save record bytes +0x15A..+0x15C, F writes them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AiSettings {
    pub battle: BattleStyle,
    pub ability: AbilityStyle,
    pub cure: CureStyle,
    /// Recovery targets (+0x15B bits 0..2): 1 On Sora, 2 On Self, 4 On Others.
    pub cure_targets: u8,
}

impl Default for AiSettings {
    /// Plan N4 defaults: Stick By Sora, Watch MP, Use As Needed, all three targets on.
    fn default() -> AiSettings {
        AiSettings { battle: BattleStyle::Together, ability: AbilityStyle::Balance, cure: CureStyle::Pinch, cure_targets: 7 }
    }
}

pub const CURE_TARGET_PLAYER: u8 = 1;
pub const CURE_TARGET_SELF: u8 = 2;
pub const CURE_TARGET_OTHERS: u8 = 4;
