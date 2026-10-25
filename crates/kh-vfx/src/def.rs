//! Effect definitions: one particle system of the effect pack
//! (`build/sora_pack/vfx/effects/<name>.json`, written by tools/sora_vfx_pack.js) parsed into
//! what the evaluator runs. Property names are the game's own (UE 4.17 Cascade with the Square
//! Enix additions); what each means and how sure that is: `sheets/port/sora-vfx-modules.json`
//! and docs/sora-vfx-spec.md 8.
//!
//! A module class the evaluator has no code for is not an error: it is listed in
//! `EmitterDef::unknown` and the emitter runs without it (`World` logs it once per effect).

use glam::{Vec2, Vec3};
use serde_json::Value;

use crate::dist::Dist;
use crate::math::RotationPriority;

/// What an emitter draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Camera-facing quads (also the GPU sprite emitters, simulated on the CPU with the same
    /// modules).
    Sprite,
    /// One instance of a static mesh per particle.
    Mesh,
    /// `TypeDataSQEX_Control`: a node that draws nothing (parent / transform carrier).
    Control,
    /// Stock AnimTrail: a ribbon between two sockets the host samples every step.
    AnimTrail,
    /// `TypeDataSQEX_TailPolygon`: a ribbon through points set at equal distances along the path
    /// the node has travelled (tail helper 0x1442A7650 / 0x144296090 [C]).
    Tail,
    /// `TypeDataSQEX_TailShape` / `TailShapeEx`: the same points, one sprite on each.
    TailSprites,
    /// `TypeDataSQEX_Decal`: drawn as a flat quad on the decal's plane (no projection).
    Decal,
    /// (XW-d) `TypeDataSQEX_FragmentMesh`: a mesh in pieces (SubMeshLists), one instance per piece per particle, that
    /// bursts apart at ImpactTime (see `FragmentDef`).
    Fragment,
    /// Anything else (grass force, skeletal mesh ...): simulated as a node, nothing drawn.
    Unsupported,
}

/// `EParticleScreenAlignment`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alignment {
    Square,
    Rectangle,
    Velocity,
    AwayFromCenter,
    FacingCameraPosition,
    TypeSpecific,
}

/// How a mesh particle (or a node) faces the camera (`SQEXBillboardType` / `DisplayType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Billboard {
    #[default]
    None,
    /// Faces the camera position.
    Camera,
    /// Turns about its own X / Y / Z axis only.
    AxisX,
    AxisY,
    AxisZ,
    /// Parallel to the screen.
    Screen,
}

impl Billboard {
    fn parse(name: &str) -> Billboard {
        if name.ends_with("SCREEN_BILLBOARD") {
            Billboard::Screen
        } else if name.ends_with("X_CAMERA_BILLBOARD") {
            Billboard::AxisX
        } else if name.ends_with("Y_CAMERA_BILLBOARD") {
            Billboard::AxisY
        } else if name.ends_with("Z_CAMERA_BILLBOARD") {
            Billboard::AxisZ
        } else if name.ends_with("CAMERA_BILLBOARD") {
            Billboard::Camera
        } else {
            Billboard::None
        }
    }
}

/// `SQEX_TAKEOVER_TYPE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Takeover {
    None,
    Always,
    SpawnTimeOnly,
}

impl Takeover {
    fn parse(v: Option<&Value>, default: Takeover) -> Takeover {
        match v.and_then(Value::as_str) {
            Some(s) if s.ends_with("_NONE") => Takeover::None,
            Some(s) if s.ends_with("_SPAWN_TIME_ONLY") => Takeover::SpawnTimeOnly,
            Some(s) if s.ends_with("_ALWAYS") => Takeover::Always,
            _ => default,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Burst {
    pub count: i32,
    /// -1: no range.
    pub count_low: i32,
    /// Fraction of `EmitterDuration`.
    pub time: f32,
}

/// One curve of `SQEX_EmitterTransforms` with its two flags.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub dist: Dist,
    /// `bCurveParameterDirectX`: the value IS the offset; otherwise it is a rate per second.
    pub direct: bool,
    /// `bInitializeCurveXParameterUseOnly`: sampled on the first tick only.
    pub init_only: bool,
}

/// `ParticleModuleSQEX_EmitterTransforms`: the node's transform track (0x144294B90 [C]).
#[derive(Clone, Debug, PartialEq)]
pub struct Transforms {
    pub position: Option<Track>,
    /// Degrees about X, Y, Z.
    pub rotation: Option<Track>,
    pub scale: Option<Track>,
    pub velocity: Option<Track>,
    pub gravity: Option<Dist>,
    /// `bIsUseTransformScale`: the local offset is multiplied by the node's own scale track.
    pub use_transform_scale: bool,
    pub display: Billboard,
}

/// `ParticleModuleSQEX_EmitterInitializeTransforms`: sampled once.
#[derive(Clone, Debug, PartialEq)]
pub struct InitTransforms {
    pub position: Option<Dist>,
    pub rotation: Option<Dist>,
    pub scale: Option<Dist>,
}

/// `ParticleModuleSQEX_EmitterTakeover` + `SQEX_ComponentCutoff`: what the node inherits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inherit {
    pub component_position: Takeover,
    pub component_rotation: Takeover,
    pub component_scale: Takeover,
    pub parent_position: Takeover,
    pub parent_rotation: Takeover,
    pub parent_scale: Takeover,
    pub parent_color: Takeover,
    pub parent_alpha: Takeover,
    /// `bLeavePlaceInWorld`: the node stays where the component was on its first tick.
    pub leave_place: bool,
    /// `bFadeTakeoverType`: the component's fade value multiplies the emitter.
    pub fade: bool,
    /// `ParticleSizeScaleTakeoverType` not NONE: the node's own scale (init x track) is part of
    /// the particle size multiplier (instance+0x4B8, 0x1442A3E80 [C]).
    pub particle_size: bool,
    /// `ParentParticleSizeScaleTakeoverType`: the parent's particle size multiplier is inherited.
    pub parent_particle_size: Takeover,
    /// The emitter has a `SQEX_EmitterTakeover` module at all. Without one a child takes its
    /// parent's position and rotation always and no scale (0x144290A60 / 0x1442A3E80 [C]).
    pub takeover: bool,
    /// `SQEX_ComponentCutoff`: its rotation / scale types; `None` = no such module.
    pub cutoff_rotation: Option<Takeover>,
    pub cutoff_scale: Option<Takeover>,
}

impl Default for Inherit {
    fn default() -> Inherit {
        Inherit { component_position: Takeover::Always, component_rotation: Takeover::Always, component_scale: Takeover::Always, parent_position: Takeover::Always, parent_rotation: Takeover::Always, parent_scale: Takeover::None, parent_color: Takeover::Always, parent_alpha: Takeover::Always, leave_place: false, fade: true, particle_size: true, parent_particle_size: Takeover::None, takeover: false, cutoff_rotation: None, cutoff_scale: None }
    }
}

/// One `ChildrenData` entry of `ParticleModuleSQEX_Parent` (0x14437A3B0 [C]).
#[derive(Clone, Debug, PartialEq)]
pub struct ChildDef {
    /// Index of the template emitter in `EffectDef::emitters` (`GenerateEmitterName`).
    pub emitter: usize,
    /// (GenerateTime, GenerateNumber).
    pub times: Vec<(f32, u32)>,
    pub max_count: u32,
    /// `LoopNumber`: in how many loops of the parent the entry generates (0 = every loop).
    pub loop_number: u32,
    pub loop_end_time: f32,
    pub offset_position: Vec3,
    /// Euler degrees (X roll, Y pitch, Z yaw).
    pub offset_rotation: Vec3,
    pub offset_scale: Vec3,
    /// `bUseSpawnModule`: children start at the parent's spawn-shape points.
    pub use_spawn_module: bool,
}

/// `EVARTEX_ODER_TYPE` of `SQEX_Ring`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RingOrder {
    All,
    Order,
    Random,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ShapeKind {
    /// `SQEX_Range` (0x144356A00 [C]): a spherical cap of half-angle `angle` about +X, turned by
    /// `Direction`; radius = sqrt(f(Deviation)) x RangeRadius (f: 0x144352710 [I]).
    Range { radius: Dist, angle: Option<Dist>, deviation: Option<Dist>, scale: Option<Dist> },
    /// `SQEX_Ring` (0x14434FCB0 / 0x1443484B0 / 0x144355000 [C]): a polygon of `vertices` corners
    /// in the XY plane (corner k = (radius, 0, 0) turned k x `angle` / vertices about Z, plus
    /// `offset_position`); a point sits `j / interval` of the way from corner k to corner k + 1.
    /// With a second radius the corners are picked between the two rings at random.
    Ring { vertices: Dist, radius: Dist, radius_two: Option<Dist>, interval: Dist, angle: Option<Dist>, order: RingOrder, offset_position: Option<Dist>, offset_position_two: Option<Dist>, offset_rotation: Option<Dist>, offset_scale: Option<Dist>, attach: bool },
    /// `SQEX_Quadrangle` (0x1443565C0 [C]): a point on one face of the box of half extents
    /// sqrt(f(Deviation)) x `size`.
    Quad { size: Vec3, scale: Option<Dist>, deviation: Option<Dist> },
    /// (VFXR) A point set in the node's frame: `SQEX_TexSpawn` (the lit pixels of its texture, baked: tools vfxr_bake.js),
    /// `SQEX_VertexSpawn` (the vertices of its vertex-data asset, baked), `SQEX_AnyVertexSpawn` (a generated cylinder /
    /// sphere). Each point: position, normal, colour. `attach_mesh`: `bUseAttachMeshVertex` (the vertices of what the
    /// effect hangs on: the host's bones stand in for them).
    Points { points: std::sync::Arc<Vec<[f32; 10]>>, order: RingOrder, attach_mesh: bool },
}

/// `EPARTICLE_DIR_TYPE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirType {
    None,
    Velocity,
    VertexNormal,
    EmitterNormal,
}

/// The `SQEX_LocationBase` family: where a particle (or a child node) starts and its initial
/// velocity.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub kind: ShapeKind,
    /// `Direction` as an Euler vector (X roll, Y pitch, Z yaw), degrees.
    pub direction: Vec3,
    pub dir_type: DirType,
    pub speed: Option<Dist>,
    pub normal_direction: f32,
    /// `bApplyOwnerScale`: the speed is multiplied by the component scale x `SQEXBaseScale`.
    pub owner_scale: bool,
    /// `bApplySpawnLocationOwnerScale`: the point is multiplied by the node's world scale.
    pub location_scale: bool,
    /// Not `bNoMeshRotation`, `SpawnRotCalcMode` TANGENT: a mesh particle is turned so that its Z
    /// axis looks along the spawn direction (0x14436F560 / 0x144356E70 [C]).
    pub mesh_rotation: bool,
}

/// (ZETTA) A stock location primitive: the particle starts at a random point of a cylinder (about `height_axis`) or a
/// sphere, restricted to the positive / negative half of each axis that is on, optionally on the surface only; with
/// `velocity` it is pushed outward (radially for a cylinder with `radial_velocity`) at `velocity_scale`.
#[derive(Clone, Debug, PartialEq)]
pub struct Primitive {
    pub cylinder: bool,
    pub radius: Dist,
    pub height: Dist,
    pub height_axis: usize,
    pub radial_velocity: bool,
    /// +X, -X, +Y, -Y, +Z, -Z
    pub axes: [bool; 6],
    pub surface_only: bool,
    pub velocity: bool,
    pub velocity_scale: Dist,
    pub start_location: Dist,
}

/// (ZETTA) `SQEX_BoneAttach` (the event kg702b's MatAura glows on Donald's bones): particle k sits on socket k (modulo the
/// list) of the skeleton the effect hangs on, at the socket's offset in the bone's frame; `follow`: every step (else only
/// where it was born). Without bones from the host the particles stay where the emitter put them.
#[derive(Clone, Debug, PartialEq)]
pub struct BoneAttachDef {
    pub sockets: Vec<(String, Vec3)>,
    pub follow: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DynParam {
    pub value: Dist,
    pub spawn_only: bool,
    pub emitter_time: bool,
}

/// A per-particle module the evaluator runs, in the emitter's module order.
#[derive(Clone, Debug, PartialEq)]
pub enum Module {
    Lifetime(Dist),
    Size(Dist),
    SizeScale { scale: Dist, axes: [bool; 3], emitter_time: bool },
    SizeMultiplyLife { multiplier: Dist, axes: [bool; 3] },
    Color { color: Dist, alpha: Dist },
    ColorOverLife { color: Dist, alpha: Dist },
    ColorScaleOverLife { color: Dist, alpha: Dist, emitter_time: bool },
    Velocity { start: Dist, radial: Option<Dist>, world: bool, owner_scale: bool },
    VelocityOverLife { scale: Dist, absolute: bool },
    AccelConst(Vec3),
    Accel(Dist),
    Drag(Dist),
    /// Turns.
    Rotation(Dist),
    /// Turns per second.
    RotationRate(Dist),
    /// Turns per axis.
    MeshRotation(Dist),
    MeshRotationRate(Dist),
    SubUv(Dist),
    /// (ZA) + `StartingFrame` (1 = the first image, 0 = a random one) and the emitter's image count: UE 4.17's movie
    /// LOOPS through the grid (it was clamped on the last image).
    SubUvMovie { fps: Dist, start: i32, frames: u32 },
    Location(Dist),
    /// (ZETTA) the stock `LocationPrimitiveCylinder` / `LocationPrimitiveSphere` (UE 4.17 SpawnEx, see sim.rs `primitive`).
    Primitive(Primitive),
    CameraOffset { offset: Dist, spawn_only: bool },
    PivotOffset(Vec2),
    Dynamic([Option<DynParam>; 4]),
    Orbit { offset: Dist, rotation: Dist, rate: Dist },
    /// `SQEX_AttachParticle` (0x1442F20C0 / 0x1442FB700 [C]).
    AttachParticle { position: bool, rotation: bool },
    /// `SQEX_ParticleRotation` (0x1442EF550 [C]): degrees about X, Y, Z.
    ParticleRotation { start: Option<Dist>, rotation: Option<Dist>, scale: Option<Dist>, direct: bool, spawn_only: bool, emitter_time: bool, module_time: bool },
    /// `SQEX_ParticleScale`: per-particle size multiplier (by its properties; same shape as
    /// ParticleRotation).
    ParticleScale { start: Option<Dist>, scale: Option<Dist>, scale_scale: Option<Dist>, direct: bool, emitter_time: bool },
    Shape(Shape),
    /// `SQEX_DepthOffset` (0x1442F2650 [C]: payload = |node scale| x DepthOffset): cm TOWARD the
    /// camera. (IMPL_ER20) The sign was read the other way round before. Evidence: the sprite vertex
    /// shader (jmb_bsc_ma, FParticleSpriteVertexFactory: view depth = z - cb3[10].x, guarded by
    /// offset < z, screen position kept) and the pack's census: 384 emitters carry a positive value
    /// (flashes, lens flares, hit glows, highlights), 36 a negative one (glw_bg, c_BG_black, drk,
    /// blk_glw: backgrounds). The CPU path between the payload and the shader constant was not traced.
    DepthOffset(f32),
    /// `SQEX_CurlNoise`: parameters as authored; the noise itself is this crate's own.
    CurlNoise { intensity: Dist, position_scale: Dist, time_scale: Dist, axis_scale: Option<Dist>, particle_time: bool, time_axis: u8, no_offset: bool },
    /// `SQEX_LocalParticle` (Spawn 0x144373420, post-spawn 0x14428DA00 [C]): with `position` the
    /// particle lives in the node's frame (carried by the node's position, and by its rotation
    /// with `local_rotation`); `rotation`: a mesh particle turns with the node.
    LocalParticle { position: bool, rotation: bool, local_rotation: bool },
    /// `SQEX_InitSizeScale` (0x1442F2770 [C]): size *= the emitter's particle size multiplier at spawn.
    InitSizeScale,
    /// `SQEX_MeshPivotOffset`: the mesh is moved by this, in units of its own size.
    MeshPivotOffset(Vec3),
    /// (VFXR) `SQEX_SphereForce`: a collision sphere at the node (+ CollPos) of ForceRadius x RadiusScale: REFLECTION keeps the
    /// particles out of it, REFLECTION_INTERNAL inside it (the velocity mirrored on the surface); a hit scales the particle
    /// by HitParticleSizeScale once [A: by the properties; the reader was not traced].
    SphereForce { center: Dist, radius: f32, radius_scale: Dist, internal: bool, hit_size: f32 },
    /// (VFXR) The stock `AttractorPoint` (UE 4.17 Update [U]): within Range of Position (node space, or world) the velocity
    /// gains Strength x dt toward it (x (Range - d) / Range with StrengthByDistance), per axis half if allowed.
    Attractor { position: Dist, range: Dist, strength: Dist, by_distance: bool, world: bool, affect_base: bool, override_velocity: bool, axes: [bool; 6] },
    /// (VFXR) The stock GPU `AttractorPointGravity` (ParticleSimulationShader [U]): within Radius of Position (node space) a
    /// constant acceleration of Strength toward it.
    AttractorGravity { position: Vec3, radius: f32, strength: Dist },
    /// (VFXR) The stock CPU `Collision`, against the host's ground (World::set_ground): the velocity mirrored and damped
    /// (DampingFactor) on a hit; after MaxCollisions the completion option (Kill, Freeze, HaltCollisions).
    Collision { max: Dist, damping: Dist, completion: u8 },
    /// (VFXR) The stock GPU `CollisionGPU` (scene-depth collision) against the host's ground: Bounce (Resilience x scale
    /// over life, Friction), Stop or Kill; the particle's radius = size x RadiusScale + RadiusBias.
    CollisionGpu { resilience: Dist, resilience_scale: Dist, friction: f32, radius_scale: f32, radius_bias: f32, response: u8 },
    /// (VFXR) The stock `SizeScaleBySpeed`: size x clamp(speed x SpeedScale, 1, MaxScale) on X and Y.
    SizeBySpeed { speed_scale: Vec2, max: Vec2 },
    /// (VFXR) The stock `AccelerationOverLifetime`: AccelOverLife(relative time) added to the velocity (world axes with
    /// bAlwaysInWorldSpace).
    AccelOverLife { accel: Dist, world: bool },
    /// (VFXR) `SQEX_ParticleLocation`: the particle is moved to its spawn place + Location(relative time) x StartScale (node
    /// axes, or world), on the axes that are on [A: by the properties].
    ParticleLocation { location: Dist, start_scale: Dist, world: bool, axes: [bool; 3] },
}

/// (VFXR) `SpawnPerUnit` (stock [U]): particles per unit of the node's travel.
#[derive(Clone, Debug, PartialEq)]
pub struct SpawnPerUnitDef {
    pub per_unit: Dist,
    pub unit: f32,
    pub tolerance: f32,
    pub ignore_rate_when_moving: bool,
    pub axes: [bool; 3],
    pub max_frame_distance: f32,
}

/// (VFXR) One event of the stock `EventGenerator`: kind 0 spawn, 1 death, 2 collision; its name.
#[derive(Clone, Debug, PartialEq)]
pub struct EventDef {
    pub kind: u8,
    pub name: String,
}

/// (VFXR) The stock `EventReceiverSpawn`: SpawnCount particles where an event of that name (and kind, 255 = any) happened.
#[derive(Clone, Debug, PartialEq)]
pub struct ReceiverDef {
    pub name: String,
    pub kind: u8,
    pub count: Dist,
    pub inherit_velocity: Option<Dist>,
}

/// (VFXR) `SQEX_TailShapeExData_Wave`: the tail's points drift by MoveVector (cm/s) and a random vector of RandomMoveVector
/// drawn anew every RandomInterval seconds [A: by the properties].
#[derive(Clone, Debug, PartialEq)]
pub struct TailWaveDef {
    pub move_vector: Vec3,
    pub random: Option<Dist>,
    pub interval: f32,
}

/// `SQEX_TailShape` on a tail emitter, with the ribbon options of `TypeDataSQEX_TailPolygon`.
///
/// Read from the tail helper [C]: its update 0x1442A7650 walks the recorded node transforms from
/// the newest back, adding up the distance between them, and sets tail point k (0 = the head)
/// where the path is `k x interval` long; 0x144296090 gives point k its place and multiplies its
/// size by `TailScale(f)`, its colour by `TailColor(f)` and its alpha by `TailAlpha(f)`, with
/// `f = k / (TailNumber - 1)` (times the scroll set sampled at `frac(f + scroll)` when
/// `bUseTailScroll`). The interval is `TailInterval x |component scale|` (0x144287CC0): a
/// length in cm, so a tail has a fixed length in space however fast its node flies.
#[derive(Clone, Debug, PartialEq)]
pub struct TailDef {
    /// `TailNumber`: points of the tail.
    pub number: Dist,
    /// `TailInterval`: cm between two points along the path.
    pub interval: Dist,
    /// `TailScale`: size multiplier along the tail (0 = head, 1 = end).
    pub scale: Dist,
    pub color: Option<Dist>,
    pub alpha: Option<Dist>,
    /// `HistoryPositionOffset`: the recorded point in the node's space.
    pub offset: Vec3,
    /// `HistoryMaxDataNumber` (0 = the helper's own limit).
    pub history_max: usize,
    /// `bUseTailScroll`: a second set sampled at a position that moves along the tail.
    pub scroll: Option<TailScroll>,
    /// `TailPolygonColor` / `TailPolygonAlpha` of the ribbon type data, along the tail.
    pub polygon_color: Option<Dist>,
    pub polygon_alpha: Option<Dist>,
    /// `TextureUScale` / `TextureVScale`, reversals, and the quarter turn of `TailUVType`.
    pub u_scale: f32,
    pub v_scale: f32,
    pub u_reverse: bool,
    pub v_reverse: bool,
    pub uv_turned: bool,
    /// `RenderAxis` Trails_WorldUp: the sheet stands on the world's up axis instead of facing the camera.
    pub world_up: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TailScroll {
    /// `TailScrollTimeScale`: tail lengths per second.
    pub time_scale: f32,
    pub scale: Option<Dist>,
    pub color: Option<Dist>,
    pub alpha: Option<Dist>,
}

/// `ParticleModuleSQEX_SpawnParticleSystem` (Update 0x1442FE100 [C]): once the emitter time has
/// passed `SpawnTime` the emitter spawns another particle system, attached to the same component
/// at the emitter node, and from then on hands it the node's location / rotation / scale and its
/// colour / alpha every tick (the `bGive*` bits); it gets loop end when the emitter time reaches
/// `LoopEndTime`, or, with `LoopEndTime` 0, when the emitter itself is told to end.
#[derive(Clone, Debug, PartialEq)]
pub struct NestedDef {
    /// The pack's effect name (`effects/<name>.json`).
    pub asset: String,
    pub spawn_time: f32,
    pub loop_end_time: f32,
    pub give_location: bool,
    pub give_rotation: bool,
    pub give_scale: bool,
    pub give_color: bool,
    pub give_alpha: bool,
}

/// `ParticleModuleSQEX_Raycast`: the node is put where a ray from it to `RayEndOffset` hits the
/// level (+ `SpawnLocationOffset`). Modelled from its properties [?] (the body, slot 114 / 117,
/// was not traced); the host supplies the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaycastDef {
    pub end_offset: Vec3,
    pub world: bool,
    pub offset: Vec3,
    /// `ESQEX_RAYCAST_SPAWN_THEME_FORCE_SPAWN`: the emitter plays where it is when nothing is hit.
    pub force: bool,
}

/// `TypeDataMesh` with `bUseSubMeshs` (a Square Enix addition): the particles draw the meshes of
/// `SubMeshLists[SubMeshsSetIndex].SubMeshList` instead of `Mesh` (which is then a placeholder,
/// usually the axis gizmo `tmd_xyz_00`). Which one, modelled from the properties and the data
/// [?] (the reader was not traced): with `MeshIndexIntervalTime` 0 a particle keeps one mesh
/// (a random one with `bUseRandomStart`: rock chunks); otherwise `SubMeshsIndex` is sampled at
/// `time / interval` (wrapping unless `bNoLoop`) and picks the mesh, a flipbook of meshes (the
/// Cure flower opening over 0.7 s, lightning flickering between two shapes every 0.05 s).
#[derive(Clone, Debug, PartialEq)]
pub struct SubMeshes {
    /// Pack mesh names (`meshes/<name>.glb`).
    pub list: Vec<String>,
    pub index: Dist,
    pub interval: f32,
    pub no_loop: bool,
    pub random_start: bool,
    pub emitter_time: bool,
}

/// (XW-d, PORT ADDITION) `TypeDataSQEX_FragmentMesh` (the Chandelier's shatter vw_so088_fin_hahen00..03, its ice flowers
/// zmn0 c_ice_hana*): the particle is a whole object in pieces. Every piece of the chosen SubMeshList (its mesh at its
/// `Offset`, x LocationOffsetScale + RandomLocationOffsetScale) is drawn at the particle's transform; from ImpactTime on
/// (particle seconds) each flies off the impact point (ImpactLocationOffset, particle space) along the line through
/// itself at ImpactPower + RandomImpactPower x [0,1], plus ImpactPowerVecotr (particle space, world with
/// bImpactPowerVecotrWorldSpace), spins at RandomImpactRotation (deg/s per axis) and falls with Gravity (cm/s^2 along
/// world Z) plus Force (cm/s^2, particle space). The class's reader was not traced (reflection-only): this is read from
/// the property names and the data [?]. The particle itself (its modules: size, velocity, colour) moves the whole.
#[derive(Clone, Debug, PartialEq)]
pub struct FragmentDef {
    pub pieces: Vec<FragmentPiece>,
    pub impact_time: f32,
    pub impact_power: f32,
    pub random_impact_power: f32,
    pub impact_vector: Vec3,
    pub impact_vector_world: bool,
    pub impact_location: Vec3,
    pub impact_rotation: Option<Dist>,
    pub gravity: f32,
    pub force: Vec3,
    pub location_scale: Dist,
    pub random_location_scale: Dist,
}

/// (XW-d) One piece of a FragmentDef: `mesh` indexes `EmitterDef::sub_meshes`'s list (None: not in the pack, not drawn).
#[derive(Clone, Debug, PartialEq)]
pub struct FragmentPiece {
    pub mesh: Option<usize>,
    pub translation: Vec3,
    pub rotation: glam::Quat,
    pub scale: Vec3,
}

/// `ParticleModuleLight` (stock [U]): a point light per particle.
#[derive(Clone, Debug, PartialEq)]
pub struct LightDef {
    pub color: Dist,
    pub brightness: Dist,
    pub radius: Dist,
    /// (IMPL_ER56) `LightExponent` (the falloff pow(1 - d^2 / r^2, exponent) of a light without inverse-squared
    /// falloff; stock default 1): the fire lights carry 2 - 3, the save point's 16 - 25.
    pub exponent: Dist,
}

/// `TypeDataSQEX_Decal`: a deferred decal on the node. Stock decal geometry [U]: the box is
/// `DecalSize` to either side of the node on each axis and projects along its local X;
/// `DecalRotation` (X roll, Y pitch, Z yaw: (0, -90, 0) on every ground decal of the pack turns
/// X down) is applied inside the node. The reader of these properties was not traced [?].
#[derive(Clone, Debug, PartialEq)]
pub struct DecalDef {
    pub material: String,
    pub size: Dist,
    pub rotation: Option<Dist>,
    pub color: Option<Dist>,
    pub opacity: Option<Dist>,
    pub custom: [Option<Dist>; 3],
    pub z_fade: [f32; 2],
    /// `bItUseScaleRotMatrixTransfrom`: the size vector is turned by `DecalRotation` first
    /// (|size x R|), so it is given in the node's axes, not the decal's (0x144281110 [C]).
    pub scale_rot: bool,
}

/// `ParticleModuleSQEX_CameraShake` (Update 0x1442FC5E0 [C]): once the emitter time reaches
/// `ShakePlayTime` the camera manager plays the `Shake` class at `CameraShakeScale(emitter time)`
/// times a factor that falls with the camera's distance from the emitter node.
#[derive(Clone, Debug, PartialEq)]
pub struct CameraShakeDef {
    /// Base name of the shake blueprint class (`BP_thdgn_cam0_C`).
    pub class: String,
    pub play_time: f32,
    pub scale: Dist,
    /// `OscillationDuration` when `OverrideOscillationDuration`.
    pub duration: Option<f32>,
    /// `ShakeScaleDistance`, `ShakeScaleDistanceOffset`, `ShakeScaleDistanceMin`.
    pub distance: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmitterDef {
    pub name: String,
    pub kind: Kind,
    /// The pack's name of the type data (`sprite`, `mesh`, `gpu_sprite`, `sqex_control` ...).
    pub kind_name: String,
    pub enabled: bool,
    /// `bAutoAddToComponent`: false = only instantiated as a child by `SQEX_Parent`.
    pub auto_add: bool,
    /// Base name of the material (`materials/<name>.json` of the pack); empty when none.
    pub material: String,
    /// Base name of the mesh of a mesh emitter (`meshes/<name>.glb`).
    pub mesh: Option<String>,
    pub mesh_billboard: Billboard,
    /// (fourth pass) bUseSQEXBillboradRotaionAxis: an axis billboard turns about the NODE's
    /// rotated axis; without it about the component's own axis (a Thunder bolt strip hangs along Z
    /// in an emitter rolled 90 degrees and is upright in the game: real-game capture) [D].
    pub billboard_node_axis: bool,
    /// SQEXBillboardRotaionOffset (pitch, yaw, roll), applied inside the billboard frame [D].
    pub billboard_offset: Vec3,
    /// (IMPL_ER20) THE LENS RULE, A JUDGEMENT (the game's transform builder was not traced): a
    /// camera-facing mesh emitter of the stock path (`bCameraFacing`, no SQEX billboard type) with
    /// `bNoLocalRotation`, whose particle size is X = Z with a flat Y (below 0.35 x), is turned a
    /// quarter about Z so that the flat axis is the view axis. One emitter of the pack meets it: the
    /// HP prize's ball (vs_plz_hpb0 OneMesh_lp0, a unit sphere sized 3.6 x 1 x 3.6, not locked):
    /// with mesh +X at the camera, as every flat mesh of the pack needs, it would show a 10 x 36 cm
    /// sliver in front of its own outline disc (glw_RB0: a 35 cm disc 10 cm behind). The rule shows
    /// the 36 cm circle that disc is the outline of. `sim::set_lens_rule(false)` = the literal reading.
    pub lens_rule: bool,
    /// (fifth pass) The mesh type data's UV selectors as the mesh-particle vertex shader reads them
    /// (its uniform buffer row 3, read from the compiled shader): `UVType` (0 none, 1 RIGHT_ROT:
    /// (u, v) -> (v, -u), 2 LIGHT_ROT: (-v, u)) + 4 x `bTexUReverse` (u -> -u, before the turn) + 8 x
    /// `bTexVReverse`.
    pub mesh_uv_mode: f32,
    pub alignment: Alignment,
    /// `EmitterBlendType`.
    pub blend: crate::draw::Blend,
    /// `bUseLocalSpace`: particles live in the component's space (the node is `EmitterToSimulation`).
    pub local_space: bool,
    /// `bUseEmitterLocalSpace` (with `bUseLocalSpace`): particles live in the node's own space.
    pub emitter_local: bool,
    /// A `SizeScale` module with `bUseEmitterTime`: its value at emitter time is part of the
    /// emitter's particle size multiplier (0x144294B90 [C]).
    pub size_scale_emitter: Option<Dist>,
    /// `EmitterDuration` (stock), seconds.
    pub duration: f32,
    /// `EmitterLoops`: 0 = until loop end is requested.
    pub loops: u32,
    /// The Square Enix timeline, seconds: the emitter starts at `start_time`; relative to that,
    /// `[0, loop_time]` is the intro, `[loop_time, loop_duration]` the loop, and
    /// `[loop_duration, duration_time]` the ending.
    pub start_time: f32,
    pub loop_time: f32,
    pub loop_duration: f32,
    pub duration_time: f32,
    pub kill_on_completed: bool,
    pub kill_on_deactivate: bool,
    pub sub_h: u32,
    pub sub_v: u32,
    /// Sub-UV interpolation blends two frames (Linear_Blend / Random_Blend).
    pub sub_blend: bool,
    pub sub_random: bool,
    pub origin: Vec3,
    /// `EmitterRotation` as an Euler vector (X roll, Y pitch, Z yaw), degrees.
    pub rotation: Vec3,
    pub priority: RotationPriority,
    pub max_draw: usize,
    /// How many particles the emitter can hold. A GPU sprite emitter: `EmitterInfo.MaxParticleCount`
    /// of its type data, the capacity the cooker computed for its simulation tiles [A] (stock: the
    /// GPU simulation spawns no more, and `MaxDrawCount` does not apply to it [U]); others: the
    /// evaluator's own guard.
    pub max_particles: usize,
    pub rate: Dist,
    pub rate_scale: Dist,
    pub bursts: Vec<Burst>,
    pub burst_scale: Dist,
    pub modules: Vec<Module>,
    pub transforms: Option<Transforms>,
    pub init: Option<InitTransforms>,
    pub inherit: Inherit,
    pub children: Vec<ChildDef>,
    /// `Parent.AllLoopEndTime`.
    pub all_loop_end_time: f32,
    /// `SQEX_AllParticleKill.KillEmitterTime`.
    pub kill_time: Option<f32>,
    pub tail: Option<TailDef>,
    pub light: Option<LightDef>,
    pub sub_meshes: Option<SubMeshes>,
    /// (XW-d) `TypeDataSQEX_FragmentMesh`'s pieces (their meshes are `sub_meshes`'s list).
    pub fragments: Option<FragmentDef>,
    /// `EmitterRenderMode`: false for ERM_LightsOnly (only its light is seen) and ERM_None.
    pub render: bool,
    pub nested: Option<NestedDef>,
    pub raycast: Option<RaycastDef>,
    /// (ZETTA) `SQEX_BoneAttach`: its particles sit on bones of what the effect is attached to (see [`BoneAttachDef`]).
    pub bone_attach: Option<BoneAttachDef>,
    pub decal: Option<DecalDef>,
    pub camera_shake: Option<CameraShakeDef>,
    /// (fourth pass) `ParticleModuleSQEX_ChangeSceneColor.LightIntensity`: the scene's light scale
    /// over the emitter's time (all 15 users of the pack: 1 -> 0.3 -> 1; no other field of the
    /// module is set in any of them), and the time of its last key (the emitter is kept until then).
    pub scene_light: Option<(Dist, f32)>,
    /// (VFXR) `SpawnPerUnit`, the events it sends and receives, `SQEX_EmitterDirectEX`'s emitter, the tail's inertia and wave.
    pub spawn_per_unit: Option<SpawnPerUnitDef>,
    pub events: Vec<EventDef>,
    pub receivers: Vec<ReceiverDef>,
    pub direct_ex: Option<String>,
    pub tail_inertia: Option<Dist>,
    pub tail_wave: Option<TailWaveDef>,
    /// (VFXR) the screen-wide passes the emitter asks for (`crate::screen`).
    pub screen: Vec<crate::screen::ScreenDef>,
    /// (VFXR) the character-material drives (`SQEX_Material_*`).
    pub char_params: Vec<crate::screen::CharParamDef>,
    /// (VFXR sort) `ParticleModuleRequired.SortMode`: how the emitter's own particles are ordered (`SortMode`).
    pub sort_mode: SortMode,
    /// Module classes present and enabled that the evaluator skips.
    pub unknown: Vec<String>,
}

impl EmitterDef {
    /// The first spawn-shape module, if any.
    pub fn shape(&self) -> Option<&Shape> {
        self.modules.iter().find_map(|m| if let Module::Shape(s) = m { Some(s) } else { None })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EffectDef {
    pub name: String,
    pub emitters: Vec<EmitterDef>,
    /// `SQEXBaseColor`.
    pub base_color: [f32; 4],
    /// `bLoopEndAtComplete`.
    pub loop_end_at_complete: bool,
    /// `SQEXBaseScale` (template+0x7C): multiplies node scales, particle sizes and shape speeds
    /// (0x1442A3E80, 0x14435B130 [C]).
    pub base_scale: Vec3,
    /// `bUseBaseScaleBugFix`: the base scale multiplies a shape's speed even without owner scale.
    pub base_scale_bugfix: bool,
    /// (IMPL_ER53, PORT ADDITION, no field of the game) THE HELD CLOCK: when set, a spawned system stops its own time at
    /// this many seconds (its particles neither age nor move on; they still follow what they are attached to) until it
    /// is stopped. `None` for every effect of the pack; a host makes a held copy with `EffectDef::variant`.
    pub hold: Option<f32>,
    /// (IMPL_ER53) A host's copy (`variant`): its disabled emitters are not made as children either.
    pub variant: bool,
    /// (VFXR sort) the component's translucency sort priority: `bSQEXTranslucencySortPriority` when
    /// `bOverrideTranslucencySortPriority`, else the stock `TranslucencySortPriority` (0). A higher priority draws after
    /// (over) every lower one, whatever the distance (UE 4.17 FTranslucentPrimSet: priority before distance).
    pub sort_priority: i32,
    /// (VFXR sort) `SQEXTranslucencySortOffset` (cm, toward the camera [A]: subtracted from the sort distance).
    pub sort_offset: f32,
}

impl EffectDef {
    /// True when some auto-added emitter loops until told to end (the effect never finishes by
    /// itself).
    pub fn endless(&self) -> bool {
        self.emitters.iter().any(|e| e.enabled && e.auto_add && e.loops == 0)
    }

    /// The effects this one spawns from inside (`SQEX_SpawnParticleSystem`): the host registers them too.
    pub fn nested_assets(&self) -> Vec<String> {
        self.emitters.iter().filter_map(|e| e.nested.as_ref().map(|n| n.asset.clone())).collect()
    }

    /// (emitter, module class) of everything skipped, in order.
    pub fn unknown(&self) -> Vec<(String, String)> {
        self.emitters.iter().flat_map(|e| e.unknown.iter().map(move |m| (e.name.clone(), m.clone()))).collect()
    }

    /// (IMPL_ER53, PORT ADDITION) A copy named `name` in which only the emitters named in `keep` (and the children of
    /// a kept parent) stay enabled, with the held clock `hold`. Indices are kept (a disabled emitter stays in the list),
    /// so parent / child references stay right. Returns the names of `keep` that are no emitter of it as well.
    pub fn variant(&self, name: &str, keep: &[&str], hold: Option<f32>) -> (EffectDef, Vec<String>) {
        let mut out = self.clone();
        out.name = name.to_string();
        out.hold = hold;
        out.variant = true;
        let missing = keep.iter().filter(|k| !self.emitters.iter().any(|e| e.name == **k)).map(|k| k.to_string()).collect();
        for e in &mut out.emitters {
            if !keep.contains(&e.name.as_str()) {
                e.enabled = false;
            }
        }
        (out, missing)
    }
}

// ------------------------------------------------------------------------------------ parsing

/// (XW-d) The pieces of a `TypeDataSQEX_FragmentMesh` (SubMeshLists[SubMeshsSetIndex], the pack's `sub_mesh_files`); the
/// meshes the pack has go to `def.sub_meshes` (so the hosts load and name them as any sub mesh).
fn fragments_of(typedata: &Value, tp: &P, def: &mut EmitterDef) -> FragmentDef {
    let set = tp.dist("SubMeshsSetIndex").map_or(0, |d| d.float(0.0, &mut crate::rng::Rng::new(1)).max(0.0) as usize);
    let lists = tp.get("SubMeshLists").and_then(Value::as_array);
    let pieces_json = lists.and_then(|l| l.get(set).or(l.first())).and_then(|l| l.get("SubMeshList")).and_then(Value::as_array);
    let files = typedata.get("sub_mesh_files").and_then(Value::as_array).and_then(|l| l.get(set).or(l.first())).and_then(Value::as_array);
    let mut list: Vec<String> = Vec::new();
    let mut pieces = Vec::new();
    let mut missing = 0;
    for (k, piece) in pieces_json.map(Vec::as_slice).unwrap_or(&[]).iter().enumerate() {
        let file = files.and_then(|f| f.get(k)).and_then(Value::as_str).map(file_stem);
        let mesh = file.map(|m| match list.iter().position(|x| x == m) {
            Some(i) => i,
            None => {
                list.push(m.to_string());
                list.len() - 1
            }
        });
        missing += usize::from(mesh.is_none());
        let null = Value::Null;
        let offset = piece.get("Offset").unwrap_or(&null);
        let r = offset.get("Rotation").unwrap_or(&null);
        let rotation = glam::Quat::from_xyzw(num(r, "x", 0.0), num(r, "y", 0.0), num(r, "z", 0.0), num(r, "w", 1.0)).normalize();
        let translation = offset.get("Translation").map_or(Vec3::ZERO, |v| vec3_of(v, Vec3::ZERO));
        let scale = offset.get("Scale3D").map_or(Vec3::ONE, |v| vec3_of(v, Vec3::ONE));
        pieces.push(FragmentPiece { mesh, translation, rotation: if rotation.is_finite() { rotation } else { glam::Quat::IDENTITY }, scale });
    }
    if missing > 0 {
        def.unknown.push(format!("{missing} of {} fragment meshes (not in the pack)", pieces.len()));
    }
    if !list.is_empty() {
        def.sub_meshes = Some(SubMeshes { list, index: Dist::constant(0.0), interval: 0.0, no_loop: false, random_start: false, emitter_time: false });
    }
    FragmentDef {
        pieces,
        impact_time: tp.f32("ImpactTime", 0.0),
        impact_power: tp.f32("ImpactPower", 0.0),
        random_impact_power: tp.f32("RandomImpactPower", 0.0),
        impact_vector: tp.vec3("ImpactPowerVecotr", Vec3::ZERO),
        impact_vector_world: tp.bool("bImpactPowerVecotrWorldSpace", false),
        impact_location: tp.vec3("ImpactLocationOffset", Vec3::ZERO),
        impact_rotation: tp.dist("RandomImpactRotation"),
        gravity: tp.f32("Gravity", 0.0),
        force: tp.vec3("Force", Vec3::ZERO),
        location_scale: tp.dist_or("LocationOffsetScale", 1.0),
        random_location_scale: tp.dist_or("RandomLocationOffsetScale", 0.0),
    }
}

struct P<'a>(&'a Value);

impl<'a> P<'a> {
    fn get(&self, name: &str) -> Option<&'a Value> {
        self.0.get(name)
    }
    fn dist(&self, name: &str) -> Option<Dist> {
        self.get(name).and_then(Dist::parse)
    }
    fn dist_or(&self, name: &str, default: f32) -> Dist {
        self.dist(name).unwrap_or(Dist::constant(default))
    }
    fn dist_vec_or(&self, name: &str, default: Vec3) -> Dist {
        self.dist(name).unwrap_or(Dist::constant_vec(default))
    }
    fn bool(&self, name: &str, default: bool) -> bool {
        self.get(name).and_then(Value::as_bool).unwrap_or(default)
    }
    fn f32(&self, name: &str, default: f32) -> f32 {
        self.get(name).and_then(Value::as_f64).map_or(default, |v| v as f32)
    }
    fn str(&self, name: &str) -> &'a str {
        self.get(name).and_then(Value::as_str).unwrap_or("")
    }
    fn vec3(&self, name: &str, default: Vec3) -> Vec3 {
        self.get(name).map_or(default, |v| vec3_of(v, default))
    }
    /// A rotator property as an Euler vector (X roll, Y pitch, Z yaw).
    fn rotator(&self, name: &str) -> Vec3 {
        self.get(name).map_or(Vec3::ZERO, rotator_of)
    }
}

fn num(v: &Value, key: &str, default: f32) -> f32 {
    v.get(key).and_then(Value::as_f64).map_or(default, |x| x as f32)
}

fn vec3_of(v: &Value, default: Vec3) -> Vec3 {
    if v.get("x").is_none() {
        return default;
    }
    Vec3::new(num(v, "x", default.x), num(v, "y", default.y), num(v, "z", default.z))
}

fn rotator_of(v: &Value) -> Vec3 {
    Vec3::new(num(v, "roll", 0.0), num(v, "pitch", 0.0), num(v, "yaw", 0.0))
}

/// The base name of a `/Game/.../name.name` path.
pub fn base_name(path: &str) -> &str {
    let tail = path.rsplit('/').next().unwrap_or(path);
    tail.split('.').next().unwrap_or(tail)
}

/// `materials/name.json` -> `name`.
fn file_stem(file: &str) -> &str {
    let tail = file.rsplit('/').next().unwrap_or(file);
    tail.rsplit_once('.').map_or(tail, |(stem, _)| stem)
}

fn track(p: &P, dist: &str, direct: &str, init_only: &str) -> Option<Track> {
    Some(Track { dist: p.dist(dist)?, direct: p.bool(direct, false), init_only: p.bool(init_only, false) })
}

fn shape_common(p: &P, kind: ShapeKind) -> Shape {
    let dir = p.str("DirectionType");
    Shape {
        kind,
        direction: p.rotator("Direction"),
        dir_type: if dir.ends_with("EMITTER_NORMAL") {
            DirType::EmitterNormal
        } else if dir.ends_with("VERTEX_NORMAL") {
            DirType::VertexNormal
        } else if dir.ends_with("VELOCITY") {
            DirType::Velocity
        } else {
            DirType::None
        },
        speed: p.dist("Speed"),
        normal_direction: p.f32("NormalDirection", 1.0).clamp(-1.0, 1.0),
        owner_scale: p.bool("bApplyOwnerScale", true),
        location_scale: p.bool("bApplySpawnLocationOwnerScale", true),
        mesh_rotation: !p.bool("bNoMeshRotation", false) && !p.str("SpawnRotCalcMode").ends_with("QUAT_Normals"),
    }
}

impl EffectDef {
    /// Parses one effect file of the pack. `Err` only for a file that is not an effect at all;
    /// anything unknown inside is skipped and recorded.
    pub fn parse(text: &str) -> Result<EffectDef, String> {
        let v: Value = serde_json::from_str(text).map_err(|e| format!("not JSON: {e}"))?;
        EffectDef::from_json(&v)
    }

    pub fn from_json(v: &Value) -> Result<EffectDef, String> {
        let name = v.get("name").and_then(Value::as_str).ok_or("no `name`")?.to_string();
        let emitters_json = v.get("emitters").and_then(Value::as_array).ok_or("no `emitters`")?;
        let names: Vec<&str> = emitters_json.iter().map(|e| e.get("name").and_then(Value::as_str).unwrap_or("")).collect();
        let emitters = emitters_json.iter().map(|e| parse_emitter(e, &names)).collect();
        let sys = v.get("system").and_then(|s| s.get("params"));
        let c = sys.and_then(|s| s.get("SQEXBaseColor"));
        let base_color = c.map_or([1.0; 4], |c| [num(c, "r", 1.0), num(c, "g", 1.0), num(c, "b", 1.0), num(c, "a", 1.0)]);
        let base_scale = sys.and_then(|s| s.get("SQEXBaseScale")).map_or(Vec3::ONE, |v| vec3_of(v, Vec3::ONE));
        let base_scale_bugfix = sys.and_then(|s| s.get("bUseBaseScaleBugFix")).and_then(Value::as_bool).unwrap_or(false);
        // (VFXR sort) the component's sort priority and offset
        let number = |k: &str| sys.and_then(|s| s.get(k)).and_then(|v| v.as_f64().or_else(|| v.as_bool().map(|b| if b { 1.0 } else { 0.0 })));
        let overridden = sys.and_then(|s| s.get("bOverrideTranslucencySortPriority")).and_then(Value::as_bool).unwrap_or(false);
        let sort_priority = if overridden { number("bSQEXTranslucencySortPriority").unwrap_or(0.0) } else { number("TranslucencySortPriority").unwrap_or(0.0) } as i32;
        let sort_offset = number("SQEXTranslucencySortOffset").unwrap_or(0.0) as f32;
        Ok(EffectDef { name, emitters, base_color, base_scale, base_scale_bugfix, loop_end_at_complete: sys.and_then(|s| s.get("bLoopEndAtComplete")).and_then(Value::as_bool).unwrap_or(false), hold: None, variant: false, sort_priority, sort_offset })
    }
}

fn parse_emitter(e: &Value, names: &[&str]) -> EmitterDef {
    let null = Value::Null;
    let required = e.get("required").unwrap_or(&null);
    let rp = P(required.get("params").unwrap_or(&null));
    let typedata = e.get("typedata").unwrap_or(&null);
    let tp = P(typedata.get("params").unwrap_or(&null));
    let kind_name = typedata.get("kind").and_then(Value::as_str).unwrap_or("sprite").to_string();
    let kind = match kind_name.as_str() {
        "sprite" | "gpu_sprite" => Kind::Sprite,
        "mesh" => Kind::Mesh,
        "sqex_control" => Kind::Control,
        "anim_trail" => Kind::AnimTrail,
        "sqex_tail_polygon" => Kind::Tail,
        "sqex_tail_shape" | "sqex_tailshapeex" => Kind::TailSprites,
        "sqex_decal" => Kind::Decal,
        "sqex_fragmentmesh" => Kind::Fragment,
        // (VFXR) a tail drawn as a tube / as meshes along it: drawn as the tail ribbon (an approximation); a grass force pushes
        // foliage and draws nothing (the hosts' levels have no foliage to push)
        "sqex_tailtube" | "sqex_tailshapetypemesh" => Kind::Tail,
        "sqex_grassforce" => Kind::Control,
        _ => Kind::Unsupported,
    };
    let time = required.get("sqex_emitter_time").unwrap_or(&null);
    let duration = num(required, "duration", 1.0);
    let legacy = time.get("use_legacy_emitter_time").and_then(Value::as_bool).unwrap_or(time.is_null());
    // Legacy timing (no emitter of the pack uses it): the stock duration is the whole life.
    let (start_time, loop_time, loop_duration, duration_time) = if legacy { (num(required, "delay", 0.0), 0.0, duration, duration) } else { (num(time, "start_time", 0.0), num(time, "loop_time", 0.0), num(time, "loop_duration", duration), num(time, "duration_time", duration)) };
    let sub = required.get("sub_uv").unwrap_or(&null);
    let interp = sub.get("interpolation").and_then(Value::as_str).unwrap_or("");
    let spawn = e.get("spawn").unwrap_or(&null);
    let sp = P(spawn);
    let spawn_enabled = sp.bool("enabled", true);
    let bursts = spawn
        .get("bursts")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(|b| Burst { count: num(b, "count", 0.0) as i32, count_low: num(b, "count_low", -1.0) as i32, time: num(b, "time", 0.0) }).collect())
        .unwrap_or_default();
    let alignment = match required.get("screen_alignment").and_then(Value::as_str).unwrap_or("") {
        "PSA_Rectangle" => Alignment::Rectangle,
        "PSA_Velocity" => Alignment::Velocity,
        "PSA_AwayFromCenter" => Alignment::AwayFromCenter,
        "PSA_FacingCameraPosition" | "PSA_FacingCameraDistanceBlend" => Alignment::FacingCameraPosition,
        "PSA_TypeSpecific" => Alignment::TypeSpecific,
        _ => Alignment::Square,
    };
    let mut def = EmitterDef {
        name: e.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
        kind,
        kind_name,
        enabled: e.get("enabled").and_then(Value::as_bool).unwrap_or(true),
        auto_add: rp.bool("bAutoAddToComponent", true),
        // The pack's own file names: two assets may share a base name (`tmi_ora_01fsla` is a water
        // material under PC/sea and a fire one under trescom), and the pack then suffixes one file.
        material: required.get("material_file").and_then(Value::as_str).map(file_stem).unwrap_or_else(|| base_name(required.get("material").and_then(Value::as_str).unwrap_or(""))).to_string(),
        mesh: typedata.get("mesh_file").and_then(Value::as_str).map(file_stem).or(typedata.get("mesh").and_then(Value::as_str).or(tp.get("Mesh").and_then(Value::as_str)).map(base_name)).map(str::to_string).filter(|m| !m.is_empty()),
        mesh_billboard: if tp.bool("bCameraFacing", false) && Billboard::parse(tp.str("SQEXBillboardType")) == Billboard::None { Billboard::Camera } else { Billboard::parse(tp.str("SQEXBillboardType")) },
        billboard_node_axis: tp.bool("bUseSQEXBillboradRotaionAxis", false),
        lens_rule: tp.bool("bCameraFacing", false) && Billboard::parse(tp.str("SQEXBillboardType")) == Billboard::None && tp.bool("bNoLocalRotation", false),
        billboard_offset: tp.get("SQEXBillboardRotaionOffset").map_or(Vec3::ZERO, |v| Vec3::new(num(v, "pitch", 0.0), num(v, "yaw", 0.0), num(v, "roll", 0.0))),
        mesh_uv_mode: {
            let turn = match tp.str("UVType") {
                t if t.ends_with("RIGHT_ROT") => 1.0,
                t if t.ends_with("LIGHT_ROT") => 2.0,
                _ => 0.0,
            };
            turn + if tp.bool("bTexUReverse", false) { 4.0 } else { 0.0 } + if tp.bool("bTexVReverse", false) { 8.0 } else { 0.0 }
        },
        alignment,
        blend: match rp.str("EmitterBlendType") {
            "BLEND_Translucent" => crate::draw::Blend::Translucent,
            "BLEND_Additive" => crate::draw::Blend::Additive,
            _ => crate::draw::Blend::Material,
        },
        local_space: required.get("use_local_space").and_then(Value::as_bool).unwrap_or(false),
        emitter_local: required.get("use_local_space").and_then(Value::as_bool).unwrap_or(false) && required.get("use_emitter_local_space").and_then(Value::as_bool).unwrap_or(false),
        size_scale_emitter: None,
        duration,
        loops: num(required, "loops", 1.0).max(0.0) as u32,
        start_time,
        loop_time,
        loop_duration,
        duration_time,
        kill_on_completed: required.get("kill_on_completed").and_then(Value::as_bool).unwrap_or(false),
        kill_on_deactivate: required.get("kill_on_deactivate").and_then(Value::as_bool).unwrap_or(false),
        sub_h: (num(sub, "horizontal", 1.0) as u32).max(1),
        sub_v: (num(sub, "vertical", 1.0) as u32).max(1),
        sub_blend: interp.ends_with("_Blend"),
        sub_random: interp.contains("Random"),
        origin: required.get("emitter_origin").map_or(Vec3::ZERO, |v| vec3_of(v, Vec3::ZERO)),
        rotation: required.get("emitter_rotation").map_or(Vec3::ZERO, rotator_of),
        priority: RotationPriority::parse(rp.str("RotationPriority")),
        max_draw: if required.get("use_max_draw_count").and_then(Value::as_bool).unwrap_or(true) { num(required, "max_draw_count", 500.0).max(1.0) as usize } else { 2000 },
        max_particles: 1000,
        rate: if spawn_enabled && sp.bool("process_spawn_rate", true) { sp.dist_or("rate", 0.0) } else { Dist::constant(0.0) },
        rate_scale: sp.dist_or("rate_scale", 1.0),
        bursts: if spawn_enabled && sp.bool("process_burst_list", true) { bursts } else { Vec::new() },
        burst_scale: sp.dist_or("burst_scale", 1.0),
        modules: Vec::new(),
        transforms: None,
        init: None,
        inherit: Inherit::default(),
        children: Vec::new(),
        all_loop_end_time: 0.0,
        kill_time: None,
        tail: None,
        light: None,
        sub_meshes: None,
        fragments: None,
        render: !matches!(e.get("render_mode").and_then(Value::as_str), Some("ERM_LightsOnly") | Some("ERM_None")),
        nested: None,
        raycast: None,
        bone_attach: None, // (ZETTA)
        decal: None,
        camera_shake: None,
        scene_light: None,
        screen: Vec::new(),      // (VFXR)
        spawn_per_unit: None,    // (VFXR)
        events: Vec::new(),
        receivers: Vec::new(),
        direct_ex: None,
        tail_inertia: None,
        tail_wave: None,
        char_params: Vec::new(), // (VFXR)
        sort_mode: SortMode::of(rp.str("SortMode")), // (VFXR sort)
        unknown: Vec::new(),
    };
    if def.kind_name == "gpu_sprite" {
        let capacity = tp.get("EmitterInfo").and_then(|i| i.get("MaxParticleCount")).and_then(Value::as_f64).unwrap_or(0.0) as usize;
        if capacity > 0 {
            // Diagnosis: KH_VFX_GPU_CAP=n holds every GPU sprite emitter to n particles (the hand-over
            // build held them to 1000 simulated, MaxDrawCount drawn).
            static CAP: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
            let cap = *CAP.get_or_init(|| std::env::var("KH_VFX_GPU_CAP").ok().and_then(|v| v.parse().ok()).unwrap_or(8000));
            def.max_particles = capacity.min(cap);
            def.max_draw = def.max_draw.max(def.max_particles);
        }
    }
    if kind == Kind::Mesh && tp.bool("bUseSubMeshs", false) {
        let set = tp.dist("SubMeshsSetIndex").map_or(0, |d| d.float(0.0, &mut crate::rng::Rng::new(1)).max(0.0) as usize);
        let files = typedata.get("sub_mesh_files").and_then(Value::as_array).and_then(|lists| lists.get(set).or(lists.first())).and_then(Value::as_array);
        let list: Vec<String> = files.map(|a| a.iter().filter_map(|m| m.as_str().map(|m| file_stem(m).to_string())).collect()).unwrap_or_default();
        if !list.is_empty() {
            def.sub_meshes = Some(SubMeshes { list, index: tp.dist_or("SubMeshsIndex", 0.0), interval: tp.f32("MeshIndexIntervalTime", 0.0), no_loop: tp.bool("bNoLoop", false), random_start: tp.bool("bUseRandomStart", false), emitter_time: tp.bool("bUseEmitterTime", false) });
        } else {
            def.unknown.push("sub meshes (not in the pack)".to_string());
        }
    }
    if kind == Kind::Fragment {
        def.fragments = Some(fragments_of(typedata, &tp, &mut def));
    }
    if kind == Kind::Unsupported {
        def.unknown.push(format!("type data {}", def.kind_name));
    }
    if kind == Kind::Decal {
        let decal_file = e.get("materials").and_then(Value::as_array).and_then(|a| a.iter().find(|m| m.get("material").and_then(Value::as_str) == Some(tp.str("DecalMaterial")))).and_then(|m| m.get("file")).and_then(Value::as_str).map(file_stem);
        def.decal = Some(DecalDef { material: decal_file.unwrap_or_else(|| base_name(tp.str("DecalMaterial"))).to_string(), size: tp.dist_vec_or("DecalSize", Vec3::splat(100.0)), rotation: tp.dist("DecalRotation"), color: tp.dist("DecalColor"), opacity: tp.dist("DecalOpacity"), custom: [tp.dist("CustomParam0"), tp.dist("CustomParam1"), tp.dist("CustomParam2")], z_fade: [tp.f32("ZFadePow", 2.0), tp.f32("ZFadeSegment", 1.0)], scale_rot: tp.bool("bItUseScaleRotMatrixTransfrom", false) });
    }
    for m in e.get("modules").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
        if !m.get("enabled").and_then(Value::as_bool).unwrap_or(true) {
            continue;
        }
        let class = m.get("class").and_then(Value::as_str).unwrap_or("");
        let short = class.strip_prefix("ParticleModule").unwrap_or(class);
        // A seeded module is its plain twin with a per-instance seed; the port's random stream
        // is its own anyway.
        let short = short.strip_suffix("_Seeded").unwrap_or(short);
        let p = P(m.get("params").unwrap_or(&null));
        let axes = |x: &str, y: &str, z: &str| [p.bool(x, true), p.bool(y, true), p.bool(z, true)];
        // (ZETTA) SQEX_BoneAttach: an emitter-wide rule (the hosts feed the bones: World::set_bones)
        if short == "SQEX_BoneAttach" {
            let list = p.0.get("AttachData").and_then(Value::as_array).cloned().unwrap_or_default();
            let sockets = list
                .iter()
                .filter_map(|a| {
                    let name = a.get("SocketName").and_then(Value::as_str)?.to_string();
                    let o = a.get("OffsetDatas").and_then(Value::as_array).and_then(|l| l.first()).and_then(|o| o.get("LocationOffset"));
                    let f = |k: &str| o.and_then(|o| o.get(k)).and_then(Value::as_f64).unwrap_or(0.0) as f32;
                    let r = |k: &str| if a.get(k).and_then(Value::as_bool).unwrap_or(false) { -1.0 } else { 1.0 };
                    Some((name, Vec3::new(f("x") * r("ReverseLocationOffsetX"), f("y") * r("ReverseLocationOffsetY"), f("z") * r("ReverseLocationOffsetZ"))))
                })
                .collect();
            def.bone_attach = Some(BoneAttachDef { sockets, follow: !p.bool("bNoUpdateParticleLocation", false) });
            continue;
        }
        // (VFXR) the screen-wide passes and the character-material drives
        if let Some(sd) = crate::screen::parse_screen(short, p.0) {
            def.screen.push(sd);
            continue;
        }
        if let Some(cd) = crate::screen::parse_char(short, p.0) {
            def.char_params.push(cd);
            continue;
        }
        // (VFXR) the particle modules VFXR evaluates (vfxr_modules.rs)
        match crate::vfxr_modules::parse(short, p.0, &mut def) {
            Some(Some(m)) => {
                def.modules.push(m);
                continue;
            }
            Some(None) => continue,
            None => {}
        }
        let module = match short {
            "Lifetime" => Some(Module::Lifetime(p.dist_or("Lifetime", 1.0))),
            "Size" => Some(Module::Size(p.dist_vec_or("StartSize", Vec3::ONE))),
            "SizeScale" => {
                if p.bool("bUseEmitterTime", false) {
                    def.size_scale_emitter = p.dist("SizeScale");
                }
                Some(Module::SizeScale { scale: p.dist_vec_or("SizeScale", Vec3::ONE), axes: axes("EnableX", "EnableY", "EnableZ"), emitter_time: p.bool("bUseEmitterTime", false) })
            }
            "SizeMultiplyLife" => Some(Module::SizeMultiplyLife { multiplier: p.dist_vec_or("LifeMultiplier", Vec3::ONE), axes: axes("MultiplyX", "MultiplyY", "MultiplyZ") }),
            "Color" => Some(Module::Color { color: p.dist_vec_or("StartColor", Vec3::ONE), alpha: p.dist_or("StartAlpha", 1.0) }),
            "ColorOverLife" => Some(Module::ColorOverLife { color: p.dist_vec_or("ColorOverLife", Vec3::ONE), alpha: p.dist_or("AlphaOverLife", 1.0) }),
            "ColorScaleOverLife" => Some(Module::ColorScaleOverLife { color: p.dist_vec_or("ColorScaleOverLife", Vec3::ONE), alpha: p.dist_or("AlphaScaleOverLife", 1.0), emitter_time: p.bool("bEmitterTime", false) }),
            "Velocity" => Some(Module::Velocity { start: p.dist_vec_or("StartVelocity", Vec3::ZERO), radial: p.dist("StartVelocityRadial").filter(|d| d.max_abs() != 0.0), world: p.bool("bInWorldSpace", false), owner_scale: p.bool("bApplyOwnerScale", false) }),
            "VelocityOverLifetime" => Some(Module::VelocityOverLife { scale: p.dist_vec_or("VelOverLife", Vec3::ONE), absolute: p.bool("Absolute", false) }),
            "AccelerationConstant" => Some(Module::AccelConst(p.vec3("Acceleration", Vec3::ZERO))),
            "Acceleration" => Some(Module::Accel(p.dist_vec_or("Acceleration", Vec3::ZERO))),
            "AccelerationDrag" => Some(Module::Drag(p.dist("DragCoefficientRaw").or(p.dist("DragCoefficient")).unwrap_or(Dist::constant(0.0)))),
            "Rotation" => Some(Module::Rotation(p.dist_or("StartRotation", 0.0))),
            "RotationRate" => Some(Module::RotationRate(p.dist_or("StartRotationRate", 0.0))),
            "MeshRotation" => Some(Module::MeshRotation(p.dist_vec_or("StartRotation", Vec3::ZERO))),
            "MeshRotationRate" => Some(Module::MeshRotationRate(p.dist_vec_or("StartRotationRate", Vec3::ZERO))),
            "SubUV" => Some(Module::SubUv(p.dist_or("SubImageIndex", 0.0))),
            "SubUVMovie" => Some(Module::SubUvMovie { fps: p.dist_or("FrameRate", 30.0), start: p.f32("StartingFrame", 1.0) as i32, frames: def.sub_h * def.sub_v }), // (ZA)
            "Location" => Some(Module::Location(p.dist_vec_or("StartLocation", Vec3::ZERO))),
            // (ZETTA) the stock primitives (UE 4.17 defaults: StartRadius 50, StartHeight 50, every axis on)
            "LocationPrimitiveCylinder" | "LocationPrimitiveSphere" => Some(Module::Primitive(Primitive {
                cylinder: short == "LocationPrimitiveCylinder",
                radius: p.dist_or("StartRadius", 50.0),
                height: p.dist_or("StartHeight", 50.0),
                height_axis: match p.str("HeightAxis").rsplit('_').next().unwrap_or("Z") { "X" => 0, "Y" => 1, _ => 2 },
                radial_velocity: p.bool("RadialVelocity", true),
                axes: [p.bool("Positive_X", true), p.bool("Negative_X", true), p.bool("Positive_Y", true), p.bool("Negative_Y", true), p.bool("Positive_Z", true), p.bool("Negative_Z", true)],
                surface_only: p.bool("SurfaceOnly", false),
                velocity: p.bool("Velocity", false),
                velocity_scale: p.dist_or("VelocityScale", 1.0),
                start_location: p.dist_vec_or("StartLocation", Vec3::ZERO),
            })),
            "CameraOffset" => Some(Module::CameraOffset { offset: p.dist_or("CameraOffset", 0.0), spawn_only: p.bool("bSpawnTimeOnly", false) }),
            "PivotOffset" => {
                let v = p.get("PivotOffset");
                Some(Module::PivotOffset(Vec2::new(v.map_or(0.0, |v| num(v, "x", 0.0)), v.map_or(0.0, |v| num(v, "y", 0.0)))))
            }
            "ParameterDynamic" => {
                let mut params: [Option<DynParam>; 4] = [None, None, None, None];
                for (k, d) in p.get("DynamicParams").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]).iter().take(4).enumerate() {
                    let dp = P(d);
                    if dp.str("ValueMethod") == "EDPV_UserSet" {
                        params[k] = dp.dist("ParamValue").map(|value| DynParam { value, spawn_only: dp.bool("bSpawnTimeOnly", false), emitter_time: dp.bool("bUseEmitterTime", false) });
                    }
                }
                Some(Module::Dynamic(params))
            }
            "Orbit" => Some(Module::Orbit { offset: p.dist_vec_or("OffsetAmount", Vec3::ZERO), rotation: p.dist_vec_or("RotationAmount", Vec3::ZERO), rate: p.dist_vec_or("RotationRateAmount", Vec3::ZERO) }),
            "SQEX_AttachParticle" => Some(Module::AttachParticle { position: p.bool("bIsAttachPosition", false), rotation: p.bool("bIsAttachRotation", false) }),
            "SQEX_ParticleRotation" => Some(Module::ParticleRotation {
                start: p.dist("StartParticleRotation"),
                rotation: p.dist("ParticleRotation"),
                scale: p.dist("ParticleRotationScale"),
                direct: p.bool("bCurveParameterDirectRotation", false),
                spawn_only: p.bool("bParticleRotationSpawnTimeOnly", false),
                emitter_time: p.bool("bEmitterTime", false),
                module_time: p.bool("bUseParticleModuleTime", false),
            }),
            "SQEX_ParticleScale" => Some(Module::ParticleScale { start: p.dist("StartParticleScale"), scale: p.dist("ParticleScale"), scale_scale: p.dist("ParticleScaleScale"), direct: p.bool("bCurveParameterDirectScale", true), emitter_time: p.bool("bEmitterTime", false) }),
            "SQEX_Range" => Some(Module::Shape(shape_common(&p, ShapeKind::Range { radius: p.dist_or("RangeRadius", 0.0), angle: p.dist("DirectionAngle"), deviation: p.dist("Deviation"), scale: p.dist("RangeScale") }))),
            "SQEX_Ring" => {
                let order = p.str("VartexGenerateType");
                Some(Module::Shape(shape_common(
                    &p,
                    ShapeKind::Ring {
                        vertices: p.dist_or("VertexNumber", 1.0),
                        radius: p.dist_or("RangeRadius", 0.0),
                        radius_two: p.dist("RangeRadiusTwo").filter(|d| d.max_abs() != 0.0),
                        interval: p.dist_or("Interval", 1.0),
                        angle: p.dist("DirectionAngle"),
                        offset_position_two: p.dist("RingOffsetPositionTow"),
                        order: if order.ends_with("_RANDOM") {
                            RingOrder::Random
                        } else if order.ends_with("_ALL") {
                            RingOrder::All
                        } else {
                            RingOrder::Order
                        },
                        offset_position: p.dist("RingOffsetPosition"),
                        offset_rotation: p.dist("RingOffsetRotation"),
                        offset_scale: p.dist("RingOffsetScale"),
                        attach: p.bool("bAttachParticle", false),
                    },
                )))
            }
            "SQEX_Quadrangle" => Some(Module::Shape(shape_common(&p, ShapeKind::Quad { size: p.vec3("QuadrangleSize", Vec3::ZERO), scale: p.dist("QuadrangleScale"), deviation: p.dist("Deviation") }))),
            "SQEX_DepthOffset" => Some(Module::DepthOffset(p.f32("DepthOffset", 0.0))),
            "SQEX_CurlNoise" => Some(Module::CurlNoise { intensity: p.dist_or("Intensity", 0.0), position_scale: p.dist_or("PositionScale", 0.01), time_scale: p.dist_or("TimeScale", 1.0), axis_scale: p.dist("CurlNoiseScale"), particle_time: p.str("TimeType").ends_with("PARTICLE_TIME"), time_axis: match p.str("TimeScaleAxisType").rsplit('_').next().unwrap_or("") { "X" => 1, "Y" => 2, "Z" => 3, _ => 0 }, no_offset: p.bool("NoRandomPositionOffset", false) }),
            "SQEX_LocalParticle" => Some(Module::LocalParticle { position: p.bool("bIsPosition", false), rotation: p.bool("bIsRotation", false), local_rotation: p.bool("bIsLocalRotation", false) }),
            "SQEX_InitSizeScale" => {
                if !p.bool("bIsScale", true) {
                    continue;
                }
                Some(Module::InitSizeScale)
            }
            "SQEX_MeshPivotOffset" => Some(Module::MeshPivotOffset(p.vec3("PivotOffset", Vec3::ZERO))),
            // ---- node-level modules: cached on the emitter, not run per particle
            "SQEX_EmitterTransforms" => {
                def.transforms = Some(Transforms {
                    position: track(&p, "LocalAddPosition", "bCurveParameterDirectPosition", "bInitializeCurvePositionParameterUseOnly"),
                    rotation: track(&p, "LocalAddRotation", "bCurveParameterDirectRotation", "bInitializeCurveRotationParameterUseOnly"),
                    scale: track(&p, "LocalAddScale", "bCurveParameterDirectScale", "bInitializeCurveScaleParameterUseOnly"),
                    velocity: track(&p, "Velocity", "bCurveParameterDirectVelocity", "bInitializeCurveVelocityParameterUseOnly").filter(|t| t.dist.max_abs() != 0.0),
                    gravity: p.dist("Gravity").filter(|d| d.max_abs() != 0.0),
                    use_transform_scale: p.bool("bIsUseTransformScale", false),
                    display: Billboard::parse(p.str("DisplayType")),
                });
                continue;
            }
            "SQEX_EmitterInitializeTransforms" => {
                def.init = Some(InitTransforms { position: p.dist("InitPosition"), rotation: p.dist("InitRotation"), scale: p.dist("InitScale") });
                continue;
            }
            "SQEX_EmitterTakeover" => {
                let i = &mut def.inherit;
                i.takeover = true;
                i.component_position = Takeover::parse(p.get("ComponentPositionTakeoverType"), Takeover::Always);
                // Read by nothing in the node chain [C]: the component's rotation and scale are cut by
                // `SQEX_ComponentCutoff` alone.
                i.component_rotation = Takeover::parse(p.get("ComponentRotationTakeoverType"), i.component_rotation);
                i.component_scale = Takeover::parse(p.get("ComponentScaleTakeoverType"), i.component_scale);
                i.parent_particle_size = Takeover::parse(p.get("ParentParticleSizeScaleTakeoverType"), Takeover::Always);
                i.parent_position = Takeover::parse(p.get("ParentPositionTakeoverType"), Takeover::Always);
                i.parent_rotation = Takeover::parse(p.get("ParentRotationTakeoverType"), Takeover::Always);
                i.parent_scale = Takeover::parse(p.get("ParentScaleTakeoverType"), Takeover::Always);
                i.parent_color = Takeover::parse(p.get("ParentParticleColorTakeoverType"), Takeover::Always);
                i.parent_alpha = Takeover::parse(p.get("ParentParticleAlphaTakeoverType"), Takeover::Always);
                i.leave_place |= p.bool("bLeavePlaceInWorld", false);
                i.fade = p.bool("bFadeTakeoverType", true);
                i.particle_size = Takeover::parse(p.get("ParticleSizeScaleTakeoverType"), Takeover::Always) != Takeover::None;
                continue;
            }
            "SQEX_ComponentCutoff" => {
                let i = &mut def.inherit;
                i.cutoff_rotation = Some(Takeover::parse(p.get("ComponentRotationTakeoverType"), Takeover::Always));
                i.cutoff_scale = Some(Takeover::parse(p.get("ComponentScaleTakeoverType"), Takeover::Always));
                i.fade = p.bool("bFadeTakeoverType", i.fade);
                continue;
            }
            "SQEX_Parent" => {
                def.all_loop_end_time = p.f32("AllLoopEndTime", 0.0);
                for c in p.get("ChildrenData").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
                    let cp = P(c);
                    let Some(emitter) = names.iter().position(|n| *n == cp.str("GenerateEmitterName")) else { continue };
                    if !cp.str("GenerationConditions").ends_with("_TIME") {
                        def.unknown.push("SQEX_Parent (PARTICLE_DEATH generation)".to_string());
                        continue;
                    }
                    def.children.push(ChildDef {
                        emitter,
                        times: c.get("TimeDatas").and_then(Value::as_array).map(|a| a.iter().map(|t| (num(t, "GenerateTime", 0.0), num(t, "GenerateNumber", 1.0).max(0.0) as u32)).collect()).unwrap_or_default(),
                        max_count: cp.f32("MaxGenerateCount", 0.0).max(0.0) as u32,
                        loop_number: cp.f32("LoopNumber", 1.0).max(0.0) as u32,
                        loop_end_time: cp.f32("LoopEndTime", 0.0),
                        offset_position: cp.vec3("OffsetPosition", Vec3::ZERO),
                        offset_rotation: cp.rotator("OffsetRotation"),
                        offset_scale: cp.vec3("OffsetScale", Vec3::ONE),
                        use_spawn_module: cp.bool("bUseSpawnModule", false),
                    });
                }
                continue;
            }
            "SQEX_AllParticleKill" => {
                def.kill_time = Some(p.f32("KillEmitterTime", 0.0));
                continue;
            }
            "SQEX_TailShape" => {
                let uv = tp.str("TailUVType");
                def.tail = Some(TailDef {
                    number: p.dist_or("TailNumber", 16.0),
                    interval: p.dist_or("TailInterval", 10.0),
                    scale: p.dist_or("TailScale", 1.0),
                    color: p.dist("TailColor"),
                    alpha: p.dist("TailAlpha"),
                    offset: p.vec3("HistoryPositionOffset", Vec3::ZERO),
                    history_max: p.f32("HistoryMaxDataNumber", 0.0).max(0.0) as usize,
                    scroll: p.bool("bUseTailScroll", false).then(|| TailScroll { time_scale: p.f32("TailScrollTimeScale", 1.0), scale: p.dist("TailScrollScale"), color: p.dist("TailScrollColor"), alpha: p.dist("TailScrollAlpha") }),
                    polygon_color: tp.dist("TailPolygonColor"),
                    polygon_alpha: tp.dist("TailPolygonAlpha"),
                    u_scale: tp.f32("TextureUScale", 1.0),
                    v_scale: tp.f32("TextureVScale", 1.0),
                    u_reverse: tp.bool("bTailTexUReverse", false),
                    v_reverse: tp.bool("bTailTexVReverse", false),
                    uv_turned: !uv.is_empty() && !uv.ends_with("_NONE"),
                    world_up: tp.str("RenderAxis") == "Trails_WorldUp",
                });
                continue;
            }
            "SQEX_SpawnParticleSystem" => {
                let asset = base_name(p.str("ParticleSystemAsset")).to_string();
                if !asset.is_empty() {
                    def.nested = Some(NestedDef { asset, spawn_time: p.f32("SpawnTime", 0.0), loop_end_time: p.f32("LoopEndTime", 0.0), give_location: p.bool("bGiveLocation", true), give_rotation: p.bool("bGiveRotation", true), give_scale: p.bool("bGiveScale", true), give_color: p.bool("bGiveColor", true), give_alpha: p.bool("bGiveAlpha", true) });
                }
                continue;
            }
            "SQEX_Raycast" => {
                def.raycast = Some(RaycastDef { end_offset: p.vec3("RayEndOffset", Vec3::new(0.0, 0.0, -1000.0)), world: p.bool("bInWorldSpace", false), offset: p.vec3("SpawnLocationOffset", Vec3::ZERO), force: p.str("SpawnTheme").ends_with("FORCE_SPAWN") });
                continue;
            }
            "SQEX_CameraShake" => {
                let class = p.str("Shake").rsplit('.').next().unwrap_or("").to_string();
                if !class.is_empty() {
                    def.camera_shake = Some(CameraShakeDef { class, play_time: p.f32("ShakePlayTime", 0.0), scale: p.dist_or("CameraShakeScale", 1.0), duration: p.bool("OverrideOscillationDuration", false).then(|| p.f32("OscillationDuration", 0.0)), distance: [p.f32("ShakeScaleDistance", 0.0), p.f32("ShakeScaleDistanceOffset", 0.0), p.f32("ShakeScaleDistanceMin", 1.0)] });
                }
                continue;
            }
            "SQEX_ChangeSceneColor" => {
                let last = p.get("LightIntensity").and_then(|v| v.get("distribution")).and_then(|d| d.get("curve")).and_then(|c| c.get("keys")).and_then(Value::as_array).and_then(|k| k.last()).and_then(|k| k.get("t")).and_then(Value::as_f64).unwrap_or(0.0) as f32;
                if let Some(dist) = p.dist("LightIntensity") {
                    def.scene_light = Some((dist, last));
                }
                continue;
            }
            "Light" => {
                def.light = Some(LightDef { color: p.dist_vec_or("ColorScaleOverLife", Vec3::ONE), brightness: p.dist_or("BrightnessOverLife", 1.0), radius: p.dist_or("RadiusScale", 1.0), exponent: p.dist_or("LightExponent", 1.0) });
                continue;
            }
            _ => None,
        };
        match module {
            Some(module) => def.modules.push(module),
            None => {
                if !def.unknown.iter().any(|u| u == short) {
                    def.unknown.push(short.to_string());
                }
            }
        }
    }
    if rp.bool("bLeavePlaceInWorld", false) {
        def.inherit.leave_place = true;
    }
    def
}

/// (VFXR sort) `ParticleModuleRequired.SortMode`: the order of one emitter's particles (UE 4.17 ParticleSortMode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortMode {
    /// As simulated (`PSORTMODE_None`).
    #[default]
    None,
    /// Far first, by view depth (`PSORTMODE_ViewProjDepth`) or by distance to the camera (`PSORTMODE_DistanceToView`).
    FarFirst,
    /// `PSORTMODE_Age_OldestFirst`: spawn order.
    OldestFirst,
    /// `PSORTMODE_Age_NewestFirst`: the reverse.
    NewestFirst,
}

impl SortMode {
    pub fn of(name: &str) -> SortMode {
        match name {
            "PSORTMODE_ViewProjDepth" | "PSORTMODE_DistanceToView" => SortMode::FarFirst,
            "PSORTMODE_Age_OldestFirst" => SortMode::OldestFirst,
            "PSORTMODE_Age_NewestFirst" => SortMode::NewestFirst,
            _ => SortMode::None,
        }
    }
}
