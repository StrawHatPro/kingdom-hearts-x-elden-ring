//! Leg IK: `TresAnimNode_TwoBoneIKMultiped` (docs/sora-animation-spec.md section 10; change list
//! B1; full algorithm in `build/sora_tmp/native_D_legik.md` section 4, whose step numbers the
//! comments cite). It runs after the saved pose is taken, on a copy; its result never feeds back.
//!
//! Per evaluation: one sphere sweep per foot (cached every other frame while standing), a
//! smoothed foot delta, a hips drop through a damped controller, the Maya two-bone solve with a
//! pole vector and a knee limit, a foot tilt toward the ground normal, and a blend of the eight
//! bones by the alpha. Everything is per-update state: it must be stepped once per simulation
//! step and never re-evaluated at another instant.

use glam::{Quat, Vec3};

use crate::blend::{find_between, normalise, slerp_not_normalised, transform_blend, Rotator};
use crate::json::Json;
use crate::pack::Skeleton;
use crate::Trs;

/// A sweep's first blocking contact (the parts of `FHitResult` the node reads), UE world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SweepHit {
    /// Fraction of the way from start to end: 0 to 1.
    pub time: f32,
    /// The point of the surface that is touched.
    pub impact_point: Vec3,
    /// Unit normal of the surface touched.
    pub impact_normal: Vec3,
    /// The sphere already overlapped something at the start.
    pub start_penetrating: bool,
}

/// What the host answers for the leg IK: the game's query on collision channel 22
/// (`TresIKCollision`), simple collision, the owner ignored (0x14063ACB0).
pub trait IkWorld {
    /// Sweeps a sphere of `radius` cm from `start` to `end` (its centre; world space, cm, Z up)
    /// and returns the first blocking hit.
    fn sweep_sphere(&self, start: Vec3, end: Vec3, radius: f32) -> Option<SweepHit>;

    /// (party A2) The same query with the node's own shape (`IkSweep` 0x14063ACB0, 4.3): a sphere (radius = Size.X,
    /// rotation identity), a capsule (radius Size.X, half length of the cylinder Size.Z, along the shape's local X) or
    /// a box (half extents = Size), at `rotation`. Also answers the depth of an initial overlap (`PenetrationDepth`, the
    /// stock MTD of `ConvertOverlappedShapeToImpactHit`; 0 for an ordinary hit). The default answers spheres through
    /// [`IkWorld::sweep_sphere`] and nothing else (a host without box / capsule sweeps: Donald's CollTwoBoneIK sees no
    /// ground and solves to the animated feet).
    fn sweep_shape(&self, start: Vec3, end: Vec3, shape: IkShape, rotation: Quat) -> Option<ShapeHit> {
        let _ = rotation;
        match shape {
            IkShape::Sphere(radius) => self.sweep_sphere(start, end, radius).map(|hit| ShapeHit { hit, penetration_depth: 0.0 }),
            _ => None,
        }
    }
}

/// (party A2) The shapes of `ETresIkCollision` (SPHERE 0, CAPSULE 1, BOX 2), sized as `IkSweep` builds them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IkShape {
    Sphere(f32),
    Capsule { radius: f32, half_height: f32 },
    /// Half extents along the shape's local axes.
    Box(Vec3),
}

/// (party A2) A shape sweep's first blocking hit and the depth of an initial overlap (`FHitResult::PenetrationDepth`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeHit {
    pub hit: SweepHit,
    pub penetration_depth: f32,
}

/// (party A2) An oriented box: centre and its three half-extent vectors (axis direction times half size).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Obb {
    pub centre: Vec3,
    pub half_axes: [Vec3; 3],
}

impl Obb {
    pub fn new(centre: Vec3, rotation: Quat, half: Vec3) -> Obb {
        Obb { centre, half_axes: [rotation * Vec3::X * half.x, rotation * Vec3::Y * half.y, rotation * Vec3::Z * half.z] }
    }

    fn radius_on(&self, axis: Vec3) -> f32 {
        self.half_axes.iter().map(|h| h.dot(axis).abs()).sum()
    }
}

/// (party A2) A moving box `mover` (from its centre to `mover.centre + motion`) against a still box `solid`, by the
/// separating-axis test over the 15 axes (the geometric answer PhysX's box-box sweep gives): `Some((time 0..1, normal
/// pointing from the solid toward the mover, initial depth))`. An initial overlap answers time 0 with the minimum
/// translation depth and its axis (the MTD of `PxGeometryQuery::computePenetration`, SAT form).
pub fn sweep_obb(mover: &Obb, motion: Vec3, solid: &Obb) -> Option<(f32, Vec3, f32)> {
    let mut axes: Vec<Vec3> = Vec::with_capacity(15);
    let unit = |v: Vec3| v.normalize_or_zero();
    for h in mover.half_axes.iter().chain(&solid.half_axes) {
        let a = unit(*h);
        if a != Vec3::ZERO {
            axes.push(a);
        }
    }
    for a in &mover.half_axes {
        for b in &solid.half_axes {
            let c = unit(*a).cross(unit(*b));
            if c.length_squared() > 1.0e-6 {
                axes.push(c.normalize());
            }
        }
    }
    let (mut t_first, mut t_last, mut first_axis) = (f32::NEG_INFINITY, f32::INFINITY, Vec3::ZERO);
    let (mut overlap_min, mut overlap_axis) = (f32::INFINITY, Vec3::ZERO);
    for axis in axes {
        let r = mover.radius_on(axis) + solid.radius_on(axis);
        let s0 = (solid.centre - mover.centre).dot(axis);
        let d = motion.dot(axis);
        let depth = r - s0.abs();
        if depth < overlap_min {
            overlap_min = depth;
            // from the solid toward the mover
            overlap_axis = if s0 > 0.0 { -axis } else { axis };
        }
        if d.abs() < 1.0e-9 {
            if s0.abs() > r {
                return None;
            }
            continue;
        }
        let (a, b) = ((s0 - r) / d, (s0 + r) / d);
        let (enter, exit) = if a < b { (a, b) } else { (b, a) };
        if enter > t_first {
            t_first = enter;
            first_axis = if d > 0.0 { -axis } else { axis };
        }
        t_last = t_last.min(exit);
        if t_first > t_last {
            return None;
        }
    }
    if overlap_min > 0.0 && t_first <= 0.0 {
        return Some((0.0, overlap_axis, overlap_min));
    }
    if t_first > 1.0 || t_last < 0.0 || t_first > t_last {
        return None;
    }
    Some((t_first.max(0.0), first_axis, 0.0))
}

/// A world without anything: no foot ever hits.
pub struct NoIkWorld;

impl IkWorld for NoIkWorld {
    fn sweep_sphere(&self, _: Vec3, _: Vec3, _: f32) -> Option<SweepHit> {
        None
    }
}

/// `FMath::VInterpTo`.
fn v_interp_to(current: Vec3, target: Vec3, dt: f32, speed: f32) -> Vec3 {
    if speed <= 0.0 {
        return target;
    }
    let distance = target - current;
    if distance.length_squared() < 1.0e-4 {
        return target;
    }
    current + distance * (dt * speed).clamp(0.0, 1.0)
}

/// `FMath::FInterpTo`.
fn f_interp_to(current: f32, target: f32, dt: f32, speed: f32) -> f32 {
    if speed <= 0.0 {
        return target;
    }
    let distance = target - current;
    if distance * distance < 1.0e-8 {
        return target;
    }
    current + distance * (dt * speed).clamp(0.0, 1.0)
}

fn safe_reciprocal(v: Vec3) -> Vec3 {
    let one = |x: f32| if x.abs() <= 1.0e-8 { 0.0 } else { 1.0 / x };
    Vec3::new(one(v.x), one(v.y), one(v.z))
}

/// One `TresTwoBoneLegIKDefinition` with its bones resolved.
#[derive(Clone, Debug)]
struct Leg {
    foot: usize,
    joint: usize,
    root: usize,
    /// `JointTargetLocation`, in the bone space of `joint_target_bone`.
    joint_target: Vec3,
    joint_target_bone: usize,
    effector_location: Vec3,
    check_min: f32,
    check_max: f32,
    radius: f32,
    mimic: Option<usize>,
    maintain_hit_normal: bool,
    limit_rot: Vec3,
    /// cos of `LimitTwoBoneIkInternalAngle`; `None` when the limit is off.
    cos_limit: Option<f32>,
    /// (party A2) `CollisionOffset` (foot-bone space; 0 for Sora, (0.2, 0, 0) for Goofy): the sweep centre (4.2).
    collision_offset: Vec3,
    /// (party A2) Read by the stock-solver leg (0x14054FC10) only: `bAllowTwist` (+0x68), `TwistAxis` (+0x6C, local
    /// space: rotated by FTransform::Identity, i.e. as given), `bAllowStretching` (+0x52), `StartStretchRatio` (+0x54),
    /// `MaxStretchScale` (+0x58).
    allow_twist: bool,
    twist_axis: Vec3,
    allow_stretching: bool,
    start_stretch_ratio: f32,
    max_stretch_scale: f32,

    // Runtime (reset by Initialize).
    /// The cached hit; `blocking` false until a sweep hit.
    cache: SweepHit,
    cache_blocking: bool,
    foot_cache: (Quat, Vec3),
    root_cache: Trs,
    counter: u32,
    smoothed: Vec3,
    foot_quat: Quat,
    /// The effector in the foot bone's space (definition +0x2C).
    effector: Vec3,
    world_target: Vec3,
}

/// What one evaluation did, for tests and the debug text.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LegIkReport {
    /// The alpha the node used (after the 0.99 cap); 0 when it did not run.
    pub alpha: f32,
    /// Per foot (left, right): a sweep was made this evaluation (false: the cache was used).
    pub swept: [bool; 2],
    pub hit: [bool; 2],
    /// Per foot: the smoothed delta, cm, world axes.
    pub delta: [Vec3; 2],
    /// The hips drop used this frame (`cur`), cm.
    pub hips_drop: f32,
    /// The hips target of this frame, cm.
    pub hips_target: f32,
}

/// The node's parameters and state.
#[derive(Clone, Debug)]
pub struct LegIk {
    /// `bUseMayaSolver` (+0x70): true for Sora; false = the stock two-bone solver (Goofy).
    maya: bool,
    hips: usize,
    legs: Vec<Leg>,
    hips_offset_percentage: f32,
    limit_hips_height: bool,
    cache_translation: f32,
    cache_rotation: f32,
    force_frame_rate: u32,
    cache_root_displacement: bool,
    keep_highest: bool,
    keep_highest_xy: f32,
    keep_highest_z: f32,
    smoothing_velocity: f32,
    time_start_smoothing: f32,
    gain: f32,
    damping: f32,
    max_velocity: f32,

    first_frame: bool,
    elapsed: f32,
    controller_value: f32,
    controller_velocity: f32,
    pub report: LegIkReport,
}

/// The alpha never exceeds this (0x145D0260C).
pub const ALPHA_CAP: f32 = 0.99;

impl LegIk {
    /// From `data/legik.json` (`two_bone_ik_multiped[0].properties`) and the skeleton.
    pub fn from_json(data: &Json, skeleton: &Skeleton) -> Result<LegIk, String> {
        let p = data.get("two_bone_ik_multiped").at(0).get("properties");
        if p.is_null() {
            return Err("legik.json has no two_bone_ik_multiped[0].properties".to_string());
        }
        let bone = |name: &Json| -> Result<usize, String> {
            let name = name.as_str().ok_or("leg IK: a bone name is missing")?;
            skeleton.index_of(name).ok_or(format!("leg IK: the skeleton has no bone {name}"))
        };
        let f = |o: &Json, key: &str| o.get(key).as_f32().ok_or(format!("leg IK: {key} is missing"));
        let v = |o: &Json| Vec3::new(o.get("x").as_f32().unwrap_or(0.0), o.get("y").as_f32().unwrap_or(0.0), o.get("z").as_f32().unwrap_or(0.0));
        let mut legs = Vec::new();
        let definitions = p.get("IKDefs").items();
        let feet: Vec<usize> = definitions.iter().map(|d| bone(d.get("IKBone").get("BoneName"))).collect::<Result<_, _>>()?;
        for d in definitions {
            let foot = bone(d.get("IKBone").get("BoneName"))?;
            let joint = skeleton.bones[foot].parent.ok_or("leg IK: the foot has no parent")?;
            let root = skeleton.bones[joint].parent.ok_or("leg IK: the shin has no parent")?;
            if d.get("CollisionShape").as_str() != Some("ETresIkCollision::SPHERE") || d.get("EffectorLocationSpace").as_str() != Some("BCS_BoneSpace") || d.get("JointTargetLocationSpace").as_str() != Some("BCS_BoneSpace") {
                return Err("leg IK: only the sphere shape and bone-space effector / joint target of Sora's asset are modelled".to_string());
            }
            if bone(d.get("EffectorSpaceBoneName"))? != foot {
                return Err("leg IK: the effector space bone is not the foot".to_string());
            }
            let mimic = d.get("ShouldMimicOtherLimbWhenNoCollision").as_bool().unwrap_or(false).then(|| d.get("LimbsToMimicWhenNoCollision").items().iter().filter_map(|n| bone(n.get("BoneName")).ok()).filter_map(|b| feet.iter().position(|f| *f == b)).next()).flatten();
            let angle = f(d, "LimitTwoBoneIkInternalAngle")?;
            legs.push(Leg {
                foot,
                joint,
                root,
                joint_target: v(d.get("JointTargetLocation")),
                joint_target_bone: bone(d.get("JointTargetSpaceBoneName"))?,
                effector_location: v(d.get("EffectorLocation")),
                check_min: f(d, "CheckMin")?,
                check_max: f(d, "CheckMax")?,
                radius: d.get("CollisionSize").get("x").as_f32().ok_or("leg IK: CollisionSize.x is missing")?,
                mimic,
                maintain_hit_normal: d.get("bMaintainEffectorHitNormal").as_bool().unwrap_or(false),
                limit_rot: Vec3::new(f(d, "MaintainEffectorLimitRotX")?, f(d, "MaintainEffectorLimitRotY")?, f(d, "MaintainEffectorLimitRotZ")?),
                cos_limit: (angle > 0.0 && angle < 180.0).then(|| angle.to_radians().cos()),
                collision_offset: v(d.get("CollisionOffset")),
                allow_twist: d.get("bAllowTwist").as_bool().unwrap_or(false),
                twist_axis: if d.get("TwistAxis").is_null() { Vec3::X } else { v(d.get("TwistAxis").get("Axis")) },
                allow_stretching: d.get("bAllowStretching").as_bool().unwrap_or(false),
                start_stretch_ratio: d.get("StartStretchRatio").as_f32().unwrap_or(1.0),
                max_stretch_scale: d.get("MaxStretchScale").as_f32().unwrap_or(1.2),
                cache: SweepHit { time: 1.0, impact_point: Vec3::ZERO, impact_normal: Vec3::ZERO, start_penetrating: false },
                cache_blocking: false,
                foot_cache: (Quat::IDENTITY, Vec3::ZERO),
                root_cache: Trs::IDENTITY,
                counter: 0,
                smoothed: Vec3::ZERO,
                foot_quat: Quat::IDENTITY,
                effector: v(d.get("EffectorLocation")),
                world_target: Vec3::ZERO,
            });
        }
        if legs.len() != 2 {
            return Err(format!("leg IK: {} legs; the two-leg path is the one modelled", legs.len()));
        }
        // The node-level flag (+0x70) picks the per-leg solver: Maya (0x140551F90, Sora) or the stock one (0x14054FC10,
        // Goofy: party A2). The per-definition bUseMayaSolver is not read.
        let maya = p.get("bUseMayaSolver").as_bool().unwrap_or(false);
        if !maya {
            for d in definitions {
                if d.get("bTakeRotationFromEffectorSpace").as_bool() == Some(true) || d.get("bMaintainEffectorRelRot").as_bool() == Some(true) {
                    return Err("leg IK (stock solver): bTakeRotationFromEffectorSpace / bMaintainEffectorRelRot are not modelled".to_string());
                }
            }
        }
        let defaults = data.get("schema").get("native_defaults").get("TresAnimNode_TwoBoneIKMultiped").get("HipsController");
        Ok(LegIk {
            maya,
            hips: bone(p.get("HipsTransform").get("BoneName"))?,
            legs,
            hips_offset_percentage: f(p, "HipsOffsetPercentage")?,
            limit_hips_height: p.get("bLimitHipsTransHeight").as_bool().unwrap_or(false),
            cache_translation: f(p, "RaycastCacheMaximumTranslation")?,
            cache_rotation: f(p, "RaycastCacheMaximumRotation")?,
            force_frame_rate: f(p, "RaycastForceFrameRate")? as u32,
            cache_root_displacement: p.get("m_RaycastCacheRootDisplacement").as_bool().unwrap_or(false),
            keep_highest: p.get("m_KeepHighestRaycastHitInSmallRadius").as_bool().unwrap_or(false),
            keep_highest_xy: f(p, "m_KeepHighest_XY_MaxRadius")?,
            keep_highest_z: f(p, "m_KeepHighest_Z_Displacement")?,
            smoothing_velocity: f(p, "FootSmoothingVelocity")?,
            time_start_smoothing: f(p, "TimeStartSmoothing")?,
            // Initialize (0x1405453D0) forces the gain; damping and maximum velocity are native.
            gain: defaults.get("Gain").as_f32().unwrap_or(140.0),
            damping: defaults.get("damping").as_f32().unwrap_or(23.6643),
            max_velocity: defaults.get("maxVelocity").as_f32().unwrap_or(6000.0),
            first_frame: true,
            elapsed: 0.0,
            controller_value: 0.0,
            controller_velocity: 0.0,
            report: LegIkReport::default(),
        })
    }

    /// True when the node runs the Maya solver (Sora); false: the stock solver (Goofy).
    pub fn maya(&self) -> bool {
        self.maya
    }

    /// The bones the node writes: hips, then per leg thigh, shin, foot.
    pub fn bones(&self) -> Vec<usize> {
        let mut out = vec![self.hips];
        for leg in &self.legs {
            out.extend([leg.root, leg.joint, leg.foot]);
        }
        out
    }

    /// One evaluation. `local` is the saved pose and `component` its component-space form (both
    /// unmodified); `alpha` the pawn's leg-IK alpha; `c2w` the mesh component's transform in the
    /// world. Changes the local transforms of the hips, thighs, shins and feet in `local`.
    pub fn evaluate(&mut self, skeleton: &Skeleton, local: &mut [Trs], component: &[Trs], alpha: f32, dt: f32, c2w: &Trs, world: &dyn IkWorld) {
        self.report = LegIkReport::default();
        // Stock shell (4.1): not evaluated at all at or below 1e-5; the state stays frozen.
        if !(alpha > 1.0e-5) {
            return;
        }
        let alpha = alpha.min(ALPHA_CAP);
        self.report.alpha = alpha;
        let inverse_c2w = c2w.rotation.conjugate();
        let mut hit_flag = [false; 2];
        let mut hit_normal = [Vec3::ZERO; 2];
        let mut world_foot = [Vec3::ZERO; 2];

        // ---- 4.2 foot traces
        for i in 0..self.legs.len() {
            let (smoothed_other, other_blocking) = {
                let leg = &self.legs[i];
                match leg.mimic.filter(|j| *j != i) {
                    Some(j) => (self.legs[j].smoothed, self.legs[j].cache_blocking),
                    None => (Vec3::ZERO, false),
                }
            };
            let first_frame = self.first_frame;
            let elapsed = self.elapsed;
            let leg = &mut self.legs[i];
            let (root_p, joint_p, foot_cs) = (component[leg.root].translation, component[leg.joint].translation, component[leg.foot]);
            let leg_length = (joint_p - root_p).length() + (foot_cs.translation - joint_p).length();
            // W = E * C2W: the foot in the world.
            let w = foot_cs.then(c2w);
            world_foot[i] = w.translation;
            // center = Wpos + Wrot.Rotate(Wscale * CollisionOffset) (4.2; the offset is 0 for Sora).
            let centre = w.translation + w.rotation * (w.scale * leg.collision_offset);
            let start_offset = c2w.rotation * Vec3::new(0.0, 0.0, leg.check_max * c2w.scale.z);
            let end_offset = c2w.rotation * Vec3::new(0.0, 0.0, leg.check_min * c2w.scale.z);
            let radius = c2w.scale.z * leg.radius;

            // Sweep or cache.
            let (reference_rotation, reference_position) = if self.cache_root_displacement { (c2w.rotation, c2w.translation) } else { (w.rotation, w.translation) };
            let (cached_rotation, cached_position) = if self.cache_root_displacement { (leg.root_cache.rotation, leg.root_cache.translation) } else { leg.foot_cache };
            let mut do_sweep = self.cache_translation * self.cache_translation <= (reference_position - cached_position).length_squared();
            if !do_sweep {
                let d = cached_rotation.dot(reference_rotation);
                let angle = (2.0 * d * d - 1.0).clamp(-1.0, 1.0).acos() * 57.295_776;
                do_sweep = self.cache_rotation <= angle || self.force_frame_rate <= leg.counter;
            }
            let mut hit = SweepHit { time: 1.0, impact_point: Vec3::ZERO, impact_normal: Vec3::ZERO, start_penetrating: false };
            let mut blocking = false;
            let mut use_cache = !do_sweep;
            if do_sweep {
                if let Some(found) = world.sweep_sphere(centre + start_offset, centre + end_offset, radius) {
                    hit = found;
                    blocking = true;
                }
                let moved = w.translation - leg.foot_cache.1;
                let accept = !self.keep_highest
                    || leg.cache.impact_point.z <= hit.impact_point.z
                    || self.keep_highest_xy * self.keep_highest_xy <= moved.x * moved.x + moved.y * moved.y
                    || self.keep_highest_z * self.keep_highest_z <= moved.z * moved.z;
                if accept && !hit.start_penetrating {
                    leg.cache = hit;
                    leg.cache_blocking = blocking;
                    leg.foot_cache = (w.rotation, w.translation);
                    leg.root_cache = *c2w;
                    leg.counter = 0;
                    self.report.swept[i] = true;
                } else {
                    use_cache = true;
                }
            }
            if use_cache {
                hit = leg.cache;
                // Only the impact point is shifted; the time is not recomputed.
                hit.impact_point += c2w.translation - leg.root_cache.translation;
                blocking = leg.cache_blocking;
                leg.counter += 1;
            }

            let v = self.smoothing_velocity;
            let clamp_to_leg = |delta: Vec3, percentage: f32| {
                if delta.length() > leg_length {
                    let clamped = delta.normalize_or_zero() * leg_length;
                    if percentage > 0.0 {
                        clamped * percentage
                    } else {
                        clamped
                    }
                } else {
                    delta
                }
            };
            let mut target_delta = None;
            if blocking {
                let offset = start_offset + (end_offset - start_offset) * hit.time;
                let mut delta = Vec3::new(offset.x, offset.y, offset.z + foot_cs.translation.z * c2w.scale.z - radius);
                if first_frame {
                    leg.smoothed = delta;
                }
                if v > 0.0 && dt > 0.0 {
                    let d2 = (leg.smoothed - delta).length_squared();
                    if v * dt < d2.sqrt() && d2 < 10000.0 && self.time_start_smoothing < elapsed {
                        delta = v_interp_to(leg.smoothed, delta, dt, v);
                    }
                }
                leg.smoothed = delta;
                target_delta = Some(clamp_to_leg(delta, self.hips_offset_percentage));
                hit_normal[i] = hit.impact_normal;
                hit_flag[i] = true;
            } else if leg.mimic.is_some() && other_blocking {
                leg.smoothed.z = f_interp_to(leg.smoothed.z, smoothed_other.z, dt, v);
                target_delta = Some(clamp_to_leg(leg.smoothed, self.hips_offset_percentage));
            } else {
                // The effector stored in the definition is NOT rewritten: the solver reuses it (4.6).
                leg.smoothed = v_interp_to(leg.smoothed, Vec3::ZERO, dt, v);
            }
            if let Some(delta) = target_delta {
                leg.effector = leg.effector_location + (w.rotation.conjugate() * delta) * safe_reciprocal(w.scale);
                leg.world_target = w.translation + w.rotation * (w.scale * leg.effector_location) + delta;
            }
            self.report.hit[i] = hit_flag[i];
            self.report.delta[i] = leg.smoothed;
        }

        // ---- 4.4 hips
        let hips = component[self.hips];
        let (mut max_drop, mut min_range, mut limited) = (0.0f32, 999_999.9f32, false);
        for (i, leg) in self.legs.iter().enumerate() {
            if !hit_flag[i] {
                continue;
            }
            let drop = world_foot[i].z - leg.world_target.z;
            if self.limit_hips_height {
                let range = (leg.check_max - leg.check_min) * leg.cache.time;
                if range < min_range {
                    min_range = range;
                    limited = true;
                }
            }
            if max_drop < drop {
                max_drop = drop * self.hips_offset_percentage;
            }
        }
        let mut target = max_drop;
        if limited {
            target = max_drop.min((min_range * self.hips_offset_percentage).max(0.0));
        }
        if self.first_frame && target > 0.0 {
            self.controller_value = target;
        }
        let mut current = self.controller_value;
        if target >= 0.0 && self.time_start_smoothing < self.elapsed {
            let step = dt.min(0.0333);
            self.controller_velocity += ((target - current) * self.gain - self.controller_velocity * self.damping) * step;
            if self.controller_velocity.abs() > self.max_velocity {
                self.controller_velocity = self.max_velocity;
            }
            current = self.controller_value + step * self.controller_velocity;
            // The STORED value is scaled by the alpha; this frame uses `current`.
            self.controller_value = current * alpha;
        }
        self.report.hips_drop = current;
        self.report.hips_target = target;
        let hips_delta = Vec3::new(0.0, 0.0, -current);
        let mut out: Vec<(usize, Trs)> = vec![(self.hips, Trs { translation: hips.translation + hips_delta, ..hips })];

        // ---- 4.5 leg solve
        let foot_weight = self.smoothing_velocity * dt * 0.5;
        for (i, leg) in self.legs.iter_mut().enumerate() {
            let (root, joint, foot) = (component[leg.root], component[leg.joint], component[leg.foot]);
            let (root_p, joint_p, foot_p) = (root.translation, joint.translation, foot.translation);
            let (l1, l2) = ((joint_p - root_p).length(), (foot_p - joint_p).length());
            // The effector from the foot's bone space into component space, then into the frame
            // before the hips move.
            let effector = foot.transform_point(leg.effector);
            let mut desired = effector - hips_delta;
            let joint_target = component[leg.joint_target_bone].transform_point(leg.joint_target);
            if let Some(cos_limit) = leg.cos_limit {
                if l1 > 0.0 && l2 > 0.0 {
                    let max_square = (l1 * l1 + l2 * l2 - 2.0 * l1 * l2 * cos_limit).max((foot_p - root_p).length_squared());
                    if (desired - root_p).length_squared() > max_square {
                        desired = root_p + (desired - root_p).normalize_or_zero() * max_square.sqrt();
                    }
                }
            }
            let (foot_position, mut foot_rotation);
            if self.maya {
                let (q_start, q_mid) = maya_solve(root_p, joint_p, foot_p, desired, joint_target - root_p, 0.0);
                out.push((leg.root, Trs { rotation: q_start * root.rotation, ..root }));
                out.push((leg.joint, Trs { rotation: q_start * q_mid * joint.rotation, translation: root_p + q_start * (joint_p - root_p), scale: joint.scale }));
                foot_position = if (desired - root_p).length() < 0.8 * (l1 + l2) { desired } else { root_p + q_start * ((joint_p - root_p) + q_mid * (foot_p - joint_p)) };
                foot_rotation = foot.rotation;
            } else {
                // (party A2) the stock-solver leg 0x14054FC10 [C]: AnimationCore::SolveTwoBoneIK (0x1437C6D50 ->
                // 0x1437C5380, stock 4.17 by shape [U]) on the unmodified root / joint / end transforms, then (without
                // bAllowTwist) the stock RemoveTwist (0x14052C9B0 [C]) on the thigh against its parent and on the shin
                // against the solved thigh; the foot keeps its rotation and takes the solved end position.
                let (mut r, mut j, mut e) = (root, joint, foot);
                solve_two_bone_ik(&mut r, &mut j, &mut e, joint_target, desired, leg.allow_stretching, leg.start_stretch_ratio, leg.max_stretch_scale);
                if !leg.allow_twist {
                    if let Some(parent) = skeleton.bones[leg.root].parent {
                        remove_twist(&component[parent], &mut r, &local[leg.root], leg.twist_axis);
                    }
                    remove_twist(&r, &mut j, &local[leg.joint], leg.twist_axis);
                }
                out.push((leg.root, r));
                out.push((leg.joint, j));
                foot_position = e.translation;
                foot_rotation = e.rotation;
            }
            if leg.maintain_hit_normal && hit_flag[i] && hit_normal[i].length_squared() > 0.0 {
                let normal = (inverse_c2w * hit_normal[i]) * safe_reciprocal(c2w.scale);
                let mut r = Rotator::from_quat(find_between(Vec3::Z, normal));
                r.roll = r.roll.clamp(-leg.limit_rot.x, leg.limit_rot.x);
                r.pitch = r.pitch.clamp(-leg.limit_rot.y, leg.limit_rot.y);
                r.yaw = r.yaw.clamp(-leg.limit_rot.z, leg.limit_rot.z);
                leg.foot_quat = normalise(slerp_not_normalised(leg.foot_quat, r.quat(), foot_weight));
                foot_rotation = leg.foot_quat * foot_rotation;
            }
            out.push((leg.foot, Trs { rotation: foot_rotation, translation: foot_position, scale: foot.scale }));
        }
        self.first_frame = false;
        self.elapsed += dt;

        // ---- stock FCSPose::LocalBlendCSBoneTransforms (4.17), the blending path (alpha < 1): the
        // list is sorted by bone index; each transform is made local against its parent's NEW
        // transform when the parent is the entry just before it in the list (shin after thigh,
        // foot after shin), else against the parent's transform in the unmodified input pose
        // (hips; thighs, whose parent `koshi` is not listed and follows the hips through the
        // hierarchy); then blended in local space. Children that are not listed follow.
        // (native_D_legik.md 4.1 words this as "against the unmodified parent" for all of them;
        // that would throw the foot off its target whenever the knee angle changes.)
        out.sort_by_key(|(bone, _)| *bone);
        for i in 0..out.len() {
            let (bone, transform) = out[i];
            let target = match skeleton.bones[bone].parent {
                Some(parent) if i > 0 && out[i - 1].0 == parent => transform.relative_to(&out[i - 1].1),
                Some(parent) => transform.relative_to(&component[parent]),
                None => transform,
            };
            local[bone] = transform_blend(&local[bone], &target, alpha.clamp(0.0, 1.0));
        }
    }
}

/// The Maya devkit `ik2Bsolver` routine as compiled into the exe (0x140551010): rotations for
/// the start joint and the middle joint that put the end at `target`, bending in the plane the
/// `pole` vector fixes. Bone lengths are kept; an unreachable target leaves the leg straight.
pub fn maya_solve(root: Vec3, joint: Vec3, foot: Vec3, target: Vec3, pole: Vec3, twist: f32) -> (Quat, Quat) {
    const PARALLEL: f32 = 0.999_845;
    let mut v1 = joint - root;
    let mut v2 = foot - joint;
    let vh = target - root;
    let mut ve = foot - root;
    let (l1, l2, lh) = (v1.length(), v2.length(), vh.length());
    let ve_square = ve.dot(ve);
    let vo = if ve_square > 0.0 { v1 - ve * (v1.dot(ve) / ve_square) } else { v1 };
    let a12 = 2.0 * find_between(v1, v2).w.clamp(-1.0, 1.0).acos();
    let cross = v1.cross(v2);
    let c12 = if cross.length_squared() > 1.0e-8 { cross.normalize() } else { cross };
    let cos_theta = (((lh as f64) * (lh as f64) - (l1 as f64) * (l1 as f64) - (l2 as f64) * (l2 as f64)) / (2.0 * (l1 as f64) * (l2 as f64))).clamp(-1.0, 1.0);
    let theta = cos_theta.acos() as f32;
    let q12 = axis_angle(c12, theta - a12);
    v2 = q12 * v2;
    ve = v1 + v2;
    let q_eh = find_between(ve, vh);
    v1 = q_eh * v1;
    if v1.normalize_or_zero().dot(vh.normalize_or_zero()).abs() >= PARALLEL {
        v1 = q_eh * vo;
    }
    let mut q_np = Quat::IDENTITY;
    if vh.normalize_or_zero().dot(pole.normalize_or_zero()).abs() < PARALLEL && lh * lh != 0.0 {
        let vn = v1 - vh * (v1.dot(vh) / (lh * lh));
        let vp = pole - vh * (pole.dot(vh) / (lh * lh));
        let lengths = vn.length() * vp.length();
        if lengths > 0.0 && (vn.dot(vp) / lengths + 1.0).abs() < 1.0e-8 {
            // A half turn about vH (the exe stores cos(pi/2) as -6.121647e-08).
            q_np = Quat::from_xyzw(vh.x, vh.y, vh.z, -6.121_647e-8);
        } else {
            q_np = find_between(vn, vp);
        }
    }
    let q_twist = if twist == 0.0 { Quat::IDENTITY } else { axis_angle(vh, twist) };
    // (start joint, middle joint)
    (normalise(q_twist * (q_np * q_eh)), normalise(q12))
}

/// UE `KINDA_SMALL_NUMBER`.
const KINDA_SMALL: f32 = 1.0e-4;

/// `FVector::GetSafeNormal` (tolerance 1e-8 on the squared size; zero vector below it).
fn safe_normal(v: Vec3) -> Vec3 {
    let s = v.length_squared();
    if s == 1.0 {
        v
    } else if s < 1.0e-8 {
        Vec3::ZERO
    } else {
        v / s.sqrt()
    }
}

/// `FVector::FindBestAxisVectors`.
fn find_best_axis_vectors(v: Vec3) -> (Vec3, Vec3) {
    let (nx, ny, nz) = (v.x.abs(), v.y.abs(), v.z.abs());
    let axis1 = if nz > nx && nz > ny { Vec3::X } else { Vec3::Z };
    let axis1 = (axis1 - v * axis1.dot(v)).normalize_or_zero();
    (axis1, axis1.cross(v))
}

/// Stock `AnimationCore::SolveTwoBoneIK` (positions; UE 4.17 TwoBoneIK.cpp [U], exe 0x1437C7290): the joint and end
/// positions that reach `effector` with the bend in the plane of `joint_target`.
#[allow(clippy::too_many_arguments)]
pub fn solve_two_bone_positions(root: Vec3, effector: Vec3, joint_target: Vec3, mut upper: f32, mut lower: f32, allow_stretching: bool, start_stretch_ratio: f32, max_stretch_scale: f32) -> (Vec3, Vec3) {
    let desired_delta = effector - root;
    let mut desired_length = desired_delta.length();
    let mut max_length = lower + upper;
    let mut desired_dir = if desired_length < KINDA_SMALL {
        desired_length = KINDA_SMALL;
        Vec3::X
    } else {
        safe_normal(desired_delta)
    };
    let target_delta = joint_target - root;
    let bend_dir;
    if target_delta.length_squared() < KINDA_SMALL * KINDA_SMALL {
        bend_dir = Vec3::Y;
    } else {
        let normal = desired_dir.cross(target_delta);
        if normal.length_squared() < KINDA_SMALL * KINDA_SMALL {
            let (_, b) = find_best_axis_vectors(desired_dir);
            bend_dir = b;
        } else {
            bend_dir = (target_delta - target_delta.dot(desired_dir) * desired_dir).normalize_or_zero();
        }
    }
    if allow_stretching {
        let range = max_stretch_scale - start_stretch_ratio;
        if range > KINDA_SMALL && max_length > KINDA_SMALL {
            let reach = desired_length / max_length;
            let factor = (max_stretch_scale - 1.0) * ((reach - start_stretch_ratio) / range).clamp(0.0, 1.0);
            if factor > KINDA_SMALL {
                lower *= 1.0 + factor;
                upper *= 1.0 + factor;
                max_length *= 1.0 + factor;
            }
        }
    }
    if desired_length >= max_length {
        return (root + upper * desired_dir, root + max_length * desired_dir);
    }
    let two_ab = 2.0 * upper * desired_length;
    let cos_angle = if two_ab != 0.0 { (upper * upper + desired_length * desired_length - lower * lower) / two_ab } else { 0.0 };
    let reverse = cos_angle < 0.0;
    let angle = cos_angle.clamp(-1.0, 1.0).acos();
    let line_dist = upper * angle.sin();
    let proj_sq = upper * upper - line_dist * line_dist;
    let mut proj = if proj_sq > 0.0 { proj_sq.sqrt() } else { 0.0 };
    if reverse {
        proj = -proj;
    }
    desired_dir = desired_dir.normalize_or_zero();
    (root + proj * desired_dir + line_dist * bend_dir, effector)
}

/// Stock `AnimationCore::SolveTwoBoneIK` on component-space transforms (0x1437C5380 [U by shape]): the upper and the
/// middle bone turn by `FindBetweenNormals(old dir, new dir)`, the middle bone and the end move; the end keeps its
/// rotation.
#[allow(clippy::too_many_arguments)]
pub fn solve_two_bone_ik(root: &mut Trs, joint: &mut Trs, end: &mut Trs, joint_target: Vec3, effector: Vec3, allow_stretching: bool, start_stretch_ratio: f32, max_stretch_scale: f32) {
    let (root_p, joint_p, end_p) = (root.translation, joint.translation, end.translation);
    let (lower, upper) = ((end_p - joint_p).length(), (joint_p - root_p).length());
    let (out_joint, out_end) = solve_two_bone_positions(root_p, effector, joint_target, upper, lower, allow_stretching, start_stretch_ratio, max_stretch_scale);
    let between = |a: Vec3, b: Vec3| find_between(safe_normal(a), safe_normal(b));
    root.rotation = between(joint_p - root_p, out_joint - root_p) * root.rotation;
    root.translation = root_p;
    joint.rotation = between(end_p - joint_p, out_end - out_joint) * joint.rotation;
    joint.translation = out_joint;
    end.translation = out_end;
}

/// `FQuat::ToSwingTwist` (0x142D91ED0): (swing, twist) with `q = swing * twist`, twist about `axis`.
pub fn to_swing_twist(q: Quat, axis: Vec3) -> (Quat, Quat) {
    let projection = axis.dot(Vec3::new(q.x, q.y, q.z)) * axis;
    let twist = Quat::from_xyzw(projection.x, projection.y, projection.z, q.w);
    let twist = if twist.length_squared() == 0.0 { Quat::IDENTITY } else { twist.normalize() };
    (q * twist.conjugate(), twist)
}

/// The stock two-bone node's `RemoveTwist` (0x14052C9B0 [C]): the bone's new local rotation keeps its swing about
/// `axis` and takes the twist of its ORIGINAL local rotation; the result is put back into component space.
pub fn remove_twist(parent: &Trs, bone: &mut Trs, original_local: &Trs, axis: Vec3) {
    let local = bone.relative_to(parent);
    let (swing, _) = to_swing_twist(local.rotation, axis);
    let (_, old_twist) = to_swing_twist(original_local.rotation, axis);
    bone.rotation = normalise(parent.rotation * normalise(swing) * old_twist);
}

/// `FQuat(Axis, Angle)`: the axis is used as given (the routine passes unnormalised ones).
fn axis_angle(axis: Vec3, angle: f32) -> Quat {
    let (s, c) = (angle * 0.5).sin_cos();
    Quat::from_xyzw(axis.x * s, axis.y * s, axis.z * s, c)
}
