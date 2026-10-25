//! What the host answers: the collision queries of the world Sora stands in
//! (docs/sora-movement-spec.md section 6).
//!
//! All values are in the core's conventions (centimetres; X forward, Y right, Z up). A host whose
//! world is in other units or axes converts on the way in and on the way out.
//!
//! One query shape serves every use: an upright capsule (its axis is Z) swept along a segment.
//! `half_height` is half its total height, caps included, so `half_height == radius` is a SPHERE
//! and both 0 is a line. Sweep positions are the shape's CENTRE. The core asks for:
//!
//! | use (spec) | shape |
//! |---|---|
//! | every move, the floor sweep (3.4, 5.2) | Sora's body: radius 60, half height 89 (centre 91 above the actor origin) |
//! | perch probe (5.2) | radius 1.0 / half height 13.6, then 0.8499 / 2.4 |
//! | edge check (5.4) | sphere of radius 25, ALL hits |
//! | the states' ground probe (4.5) | sphere of radius 50, ALL hits |
//! | floor line, perch line, edge lines | line |
//!
//! "What blocks": the `TresPlayerPawn` collision profile (WorldStatic, WorldDynamic, ...; other
//! pawns never block).
//!
//! RUNTIME-CONFIRMATION ITEM R4: what PhysX reports as `ImpactNormal` when the capsule's round
//! end touches a box edge or a mesh edge decides every edge contact (walkable or not). The test
//! worlds here return, for a box, the face most opposed to the move among the faces the touched
//! point lies on (UE's `FindBoxOpposingNormal`): the tread when coming down onto an edge, the riser
//! when moving into it sideways.

use glam::{Quat, Vec3};

/// One contact of a sweep or a line (the parts of UE's `FHitResult` the movement code reads).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// Fraction of the way from `start` to `end` at which contact is made: 0 to 1. No pull-back:
    /// the core applies the engine's.
    pub time: f32,
    /// The point of the surface that is touched (UE `ImpactPoint`), cm.
    pub position: Vec3,
    /// Unit normal of the SWEEP at the contact (UE `Normal`): from the touched point toward the
    /// shape's surface centre there (it tilts on the round ends). For a line: the surface's.
    pub normal: Vec3,
    /// Unit normal of the SURFACE touched (UE `ImpactNormal`), pointing out of it. See R4 above.
    pub impact_normal: Vec3,
    /// True when the shape already overlapped the surface at `start` (then `time` is 0).
    pub start_penetrating: bool,
    /// How deep that overlap is along `normal`, cm; 0 when not penetrating.
    pub penetration_depth: f32,
    /// Which thing was hit: an id the host chooses, the same for every hit of one component
    /// (one collision primitive / one mesh). Used to ignore "the floor component" in the edge
    /// check. 0 when the host has no such notion.
    pub component: u32,
}

/// The traversal flags of a collision component (`UPrimitiveComponent`+0x854; docs/sora-actions-spec.md
/// II.4 "Common engine pieces", II.6): what `World::surface_flags` answers for `Hit::component`.
/// Added by the actions implementer (tier 2).
pub mod surface {
    /// `SQEX_bEnableDangle` (bit 2): its walkable top edge can be hung from.
    pub const DANGLE: u32 = 1 << 2;
    /// `SQEX_bEnableWallRun` (bit 3): the wall can be run on.
    pub const WALL_RUN: u32 = 1 << 3;
    /// `SQEX_bEnableWallKick` (bit 4): the wall can be kicked off.
    pub const WALL_KICK: u32 = 1 << 4;
    /// `SQEX_bEnableAutoRun` (bit 5): a top surface a wall run may run out onto.
    pub const AUTO_RUN: u32 = 1 << 5;
    /// `SQEX_bEnableSlopeSlide` (bit 6): a floor he slides down when it is steep enough (25 degrees).
    pub const SLOPE_SLIDE: u32 = 1 << 6;
    /// `SQEX_bEnableForceSlopeSlide` (bit 8): slides from 15 degrees, whatever he does.
    pub const FORCE_SLOPE_SLIDE: u32 = 1 << 8;
    /// The native defaults of every mesh and BSP component (`UMeshComponent` 0x144009D38): Dangle
    /// and WallKick on, WallRun, AutoRun and the slope flags off.
    pub const MESH_DEFAULT: u32 = DANGLE | WALL_KICK;
}

/// (party I1) Where a collision component is and whether it moves: `USceneComponent` ComponentToWorld (+0x2A0 rotation,
/// +0x2B0 location, +0x2C0 scale) and `Mobility` (+0x348) == Movable (2). What `World::component_motion` answers;
/// the Dangle state's moving ledges read it (Begin 0x141508DE0, tick 0x14150F220; docs/sora-actions-spec.md II.6).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ComponentMotion {
    pub location: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    /// Mobility Movable (a moving platform); false = Static / Stationary.
    pub movable: bool,
}

impl ComponentMotion {
    /// ComponentToWorld applied to a point of the component's space.
    pub fn to_world(&self, local: Vec3) -> Vec3 {
        self.location + self.rotation * (local * self.scale)
    }
    /// The inverse (UE `InverseTransformPosition`).
    pub fn to_local(&self, world: Vec3) -> Vec3 {
        let s = Vec3::new(if self.scale.x != 0.0 { self.scale.x } else { 1.0 }, if self.scale.y != 0.0 { self.scale.y } else { 1.0 }, if self.scale.z != 0.0 { self.scale.z } else { 1.0 });
        (self.rotation.inverse() * (world - self.location)) / s
    }
}

/// A pole (`UTresPoleComponent`, actions spec II.4 "The pole component and its probe"): a
/// registry entry, not collision. World space, cm.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pole {
    /// The host's id (not 0), stable while the pole exists.
    pub id: u32,
    /// The world positions of `m_BaseOffset1` / `m_BaseOffset2`: the segment the probe measures to.
    pub base1: Vec3,
    pub base2: Vec3,
    /// `m_CheckRadius` (default 50): added to the probe's range of 100.
    pub check_radius: f32,
    /// `m_AttachRadius`: how far from the axis he hangs / spins.
    pub attach_radius: f32,
    /// `m_bIsHorizontality`: a bar he swings round (POLE_SWING); else a vertical pole he spins
    /// round (POLE_SPIN).
    pub horizontal: bool,
    /// component+0x868 bit 0.
    pub active: bool,
}

/// One node of a rail (`ATresRailSlideActor`, actions spec II.5 R0): the path of his CAPSULE
/// CENTRE. Segments between consecutive nodes are straight here (the game's are two-point cubic
/// curves: a host lays a curved rail as more nodes).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RailNode {
    pub position: Vec3,
    /// `m_bSetSlideSpeed` with `m_SetSlideSpeed` (= the reverse speed here); None = the pawn's 2000.
    pub speed: Option<f32>,
    /// `m_DetectRangePlus`: added to the probe range (50 on the ground, 150 in the air).
    pub detect_range_plus: f32,
}

/// A rail: a chain of nodes (actions spec II.5 "Rail Slide"). A registry entry, not collision:
/// on a rail he ignores the level.
#[derive(Clone, Debug, PartialEq)]
pub struct Rail {
    /// The host's id (not 0).
    pub id: u32,
    pub nodes: Vec<RailNode>,
    /// `m_StartDir`: 0 BOTH, 1 FORWORD, 2 REVERSE.
    pub start_dir: u8,
    /// `m_bDisableReverse`, `m_bDisableActJump`, `m_bIsNarrow` (which switches every action off).
    pub no_reverse: bool,
    pub no_jump: bool,
    pub narrow: bool,
}

/// A hop point (`UTresHopComponent`, actions spec II.5 H0): where his capsule CENTRE rests.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HopPoint {
    /// The host's id (not 0).
    pub id: u32,
    /// The component's location + `m_AttachOffset`.
    pub position: Vec3,
    /// `m_CheckRadius` (default 100).
    pub check_radius: f32,
    /// `m_IdleMotionType`: 1 HOPPING (`BalanceR/L`), 2 SURFING (`Balance`), 3 NORMAL (`IdleN`).
    pub idle_motion: u8,
    /// `m_bIsEndPoint` with `m_EndMotionType` 5 (`RollLand`) or any other (the state just ends).
    pub end_point: bool,
    pub end_roll: bool,
    /// `m_bIsGimmickActive`.
    pub active: bool,
}

/// The host's world. Implementations must be deterministic.
pub trait World {
    /// Moves the shape from `start` to `end` (centres, cm) and returns the first blocking
    /// contact, or `None` when the whole move is free.
    ///
    /// A surface the shape merely touches at `start` is reported (with `time` 0) only if the
    /// move goes INTO it; one it moves along or away from is not reported, as in the engine
    /// (initial contacts are ignored when moving out of them).
    fn sweep_capsule(&self, start: Vec3, end: Vec3, radius: f32, half_height: f32) -> Option<Hit>;

    /// The first blocking surface on the segment from `start` to `end`.
    fn line_trace(&self, start: Vec3, end: Vec3) -> Option<Hit>;

    /// Every component the swept shape meets, each with its first contact, ordered by `time`
    /// (UE's multi sweep). The default knows only the first.
    fn sweep_capsule_all(&self, start: Vec3, end: Vec3, radius: f32, half_height: f32) -> Vec<Hit> {
        self.sweep_capsule(start, end, radius, half_height).into_iter().collect()
    }

    /// The `SurfaceType` of the physical material of what `hit` touched (the game enum
    /// `EPhysicalSurface` as its number: 0 default, 1 mud, 2 soil, 4 sand, 6 grass, 9 snow,
    /// 10 rock, 13 wood, 19 metal ...: docs/sora-audio-spec.md 4.1). THE SIMULATION NEVER CALLS
    /// THIS: it exists for the host audio side (the automatic footstep sounds choose their bank
    /// by the surface under him), which asks it about a floor hit of its own. The default is a
    /// world with one surface. (Added by the audio implementer, 2026-10-11.)
    fn surface_type(&self, _hit: &Hit) -> u8 {
        0
    }

    /// (combat implementer, 2026-10-12) What the ACTOR owning `component` answers an attack that
    /// touches it while the swing's map-hit-reflect window is open (`AActor::m_DefaultAttackHitResponse`,
    /// `ESQEX_AttackHitResponseType`: 0 NORMAL, 1 GUARD, 2 REFLECT, 3 CRITICAL; combat spec 8.2.6
    /// first rule): `None` = geometry without an actor (BSP), which repels the swing like GUARD /
    /// REFLECT; `Some(0)` = an ordinary actor, which does not. The default is actor-less geometry.
    fn attack_hit_response(&self, _component: u32) -> Option<u8> {
        None
    }

    // ---- traversal affordances (actions implementer, tier 2 / 3; docs/sora-actions-spec.md II.4 - II.6,
    // "Host affordances"). Every default is a level without the feature.

    /// The traversal flags (`surface`) of the component a hit reported (`Hit::component`). The
    /// default is the engine's default for a mesh: hangable and kickable, nothing else.
    fn surface_flags(&self, _component: u32) -> u32 {
        surface::MESH_DEFAULT
    }

    /// The pole registry.
    fn poles(&self) -> &[Pole] {
        &[]
    }

    /// The rail registry (level rails; the Blizzard spell's ice rails are the core's own).
    fn rails(&self) -> &[Rail] {
        &[]
    }

    /// The hop-point registry.
    fn hop_points(&self) -> &[HopPoint] {
        &[]
    }

    /// (party I1) The transform and mobility of the component a hit reported (`Hit::component`): None = a static
    /// component (or a host without moving geometry), which is the default.
    fn component_motion(&self, _component: u32) -> Option<ComponentMotion> {
        None
    }

    /// The water surface height at `position` when it is inside a water volume (`bWaterVolume`
    /// physics volume, actions spec II.6 "Water"); None = no water there.
    fn water_surface(&self, _position: Vec3) -> Option<f32> {
        None
    }
}
