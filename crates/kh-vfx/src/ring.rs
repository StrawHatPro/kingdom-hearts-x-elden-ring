//! (VFX-RING) `ParticleModuleSQEX_Ring` (and the `SQEX_LocationBase` code it shares) as KH3 runs it, the parts the
//! evaluator did not have: how many placements one spawn makes, and the rotation a placed CHILD NODE gets.
//!
//! The module's virtual table (vtable 0x147750598, `SQEX_LocationBase` slots from 0x368):
//! - slot 0x3A0 = 0x14434FCB0: ONE placement. It reads VertexNumber (0 -> 1), RangeRadius, RangeRadiusTwo, Interval (0 -> 1)
//!   at the emitter time (vt+0x2C8), draws both counters anew for `EVARTEX_ODER_TYPE_RANDOM` (instance +0x70C step,
//!   +0x710 vertex), takes the point of 0x1443484B0 (corners 0x144355000) and either sets a PARTICLE's place (slot 0x380,
//!   payload: vertex number, vertex, step / Interval) or creates a CHILD EMITTER (template vt+0x238, 0x14434A3C0: its
//!   spawn place +0x474 = the point x the component scale; slot 0x370: its spawn rotation). Then step += 1; at Interval the
//!   step wraps and the vertex moves on; at VertexNumber the vertex wraps. kh-vfx `sim.rs` `sample_shape` has all of that.
//! - slot 0x3A8 = 0x144376860: the PARTICLE spawn. It calls slot 0x3A0 ONCE for the particle (no child: arg 2 = 0).
//!   So `EVARTEX_ODER_TYPE_ALL` does NOT multiply the particles: ALL and ORDER place particles the same way, one corner
//!   after the other. [D] the data agrees everywhere: every ALL ring emitter of the pack bursts VertexNumber x Interval
//!   particles (vw_so022_smk0 14 on 14 corners, VW_SO022_CHG6 500 on 50 x 5, VW_SO021_gat2 20 on 20 ...) and its saved
//!   PeakActiveParticles is that count + 1 .. 3 (16 for vw_so022_smk0), never the square that a multiplication gives.
//! - slot 0x390 = 0x14434E8B0: the CHILD generation, called by `SQEX_Parent::Update` 0x14437A3B0 for every GenerateNumber
//!   of a `bUseSpawnModule` entry. For ALL it calls slot 0x3A0 while i < VertexNumber x Interval (both raw, at the emitter
//!   time: 15.5 x 1 makes 16, 0 makes none and the generation does not count); any other order calls it once.
//! - slot 0x370 = 0x14435BE00 (LocationBase): the child's SPAWN ROTATION, written to its instance +0x290 (a 4 x 4 the node
//!   transform 0x144290A60 multiplies after the node's own rotation: rows of FUN_14427DAA0's matrix x +0x290), through
//!   0x144356E70 (below, `child_rotation`).
//! - Not in the child path: any velocity. 0x14434A3C0 writes the place only and 0x14435BE00 the rotation only; `Speed` is
//!   the particle path's (slot 0x380 -> 0x14435B130). A child node moves only by its own modules.

use glam::{Mat3, Quat, Vec3};
use serde_json::Value;

use crate::def::{DirType, Shape};
use crate::dist::Dist;
use crate::math::rotator;
use crate::rng::Rng;

/// `ESQEX_SPAWN_ROT_CALC_MODE` and the `SQEX_LocationBase` axes its QUAT_Normals mode reads (constructor 0x144345E10:
/// NormalAxis (0,0,1) +0xB8, NormalUpAxis (0,0,1) +0xC4, LocalSpaceTopAxis (0,0,1) +0xD0, LocalSpaceUpAxis (0,1,0) +0xDC,
/// LocalSpaceFacingAxis (0,0,1) +0xE8).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpawnRot {
    /// 0 TANGENT, 1 QUAT_Normals, anything else: no rotation.
    pub mode: u8,
    /// `NormalDirection` as authored (0x144356E70 clamps it to [-1, 1] and takes 0 as 1).
    pub sign: f32,
    pub normal_axis: Vec3,
    pub normal_up_axis: Vec3,
    pub top_axis: Vec3,
    pub up_axis: Vec3,
    pub facing_axis: Vec3,
}

impl Default for SpawnRot {
    fn default() -> SpawnRot {
        SpawnRot { mode: 0, sign: 1.0, normal_axis: Vec3::Z, normal_up_axis: Vec3::Z, top_axis: Vec3::Z, up_axis: Vec3::Y, facing_axis: Vec3::Z }
    }
}

impl SpawnRot {
    /// From a LocationBase module's `params`.
    pub fn parse(params: &Value) -> SpawnRot {
        let d = SpawnRot::default();
        let axis = |name: &str, default: Vec3| {
            params.get(name).map_or(default, |v| {
                let n = |k: &str, x: f32| v.get(k).and_then(Value::as_f64).map_or(x, |f| f as f32);
                Vec3::new(n("x", default.x), n("y", default.y), n("z", default.z))
            })
        };
        let mode = match params.get("SpawnRotCalcMode").and_then(Value::as_str).unwrap_or("") {
            m if m.ends_with("_TANGENT") || m.is_empty() => 0,
            m if m.ends_with("_QUAT_Normals") => 1,
            _ => 2,
        };
        let sign = params.get("NormalDirection").and_then(Value::as_f64).map_or(1.0, |f| f as f32).clamp(-1.0, 1.0);
        SpawnRot {
            mode,
            sign: if sign == 0.0 { 1.0 } else { sign },
            normal_axis: axis("NormalAxis", d.normal_axis),
            normal_up_axis: axis("NormalUpAxis", d.normal_up_axis),
            top_axis: axis("LocalSpaceTopAxis", d.top_axis),
            up_axis: axis("LocalSpaceUpAxis", d.up_axis),
            facing_axis: axis("LocalSpaceFacingAxis", d.facing_axis),
        }
    }
}

/// 0x14434E8B0 (slot 0x390, `EVARTEX_ODER_TYPE_ALL`): the placements of one child generation, VertexNumber x Interval at
/// the emitter time `t`, rounded up (the loop runs while i < the product); none when it is not above 0.
pub fn all_children(vertices: &Dist, interval: &Dist, t: f32, rng: &mut Rng) -> u32 {
    let n = vertices.float(t, rng) * interval.float(t, rng);
    if n > 0.0 {
        n.ceil() as u32
    } else {
        0
    }
}

/// `FQuat::FindBetween` (FUN_142D7C000): the shortest rotation taking `from` onto `to`.
fn between(from: Vec3, to: Vec3) -> Quat {
    match (from.try_normalize(), to.try_normalize()) {
        (Some(a), Some(b)) => Quat::from_rotation_arc(a, b),
        _ => Quat::IDENTITY,
    }
}

/// The inverse of `FMatrix::LookAt(0, dir, up)` (FUN_144349EB0 then FUN_14051FC30): Z along `dir`, X = up x Z, Y = Z x X
/// (a degenerate axis stays zero, as the game's normalise leaves it).
fn look_along(dir: Vec3, up: Vec3) -> Mat3 {
    let z = dir.normalize_or_zero();
    let x = up.cross(z).normalize_or_zero();
    Mat3::from_cols(x, z.cross(x), z)
}

/// The nudge 0x144356E70 adds to the spawn direction before its look-at (so that an exact axis never meets `up`).
const NUDGE: Vec3 = Vec3::new(0.01, -0.01, 0.001);

/// 0x14435BE00 -> 0x144356E70: the spawn rotation of a CHILD NODE a LocationBase shape places at `point` (the shape's
/// point in the parent node's frame, before the node's own rotation: what slot 0x3A0 hands slot 0x370). The node's
/// local rotation becomes this x its own (`sim.rs`).
/// - `EPARTICLE_DIR_TYPE_EMITTER_NORMAL`, TANGENT: look along sign x point + NUDGE (normalised), up = Direction's Z.
/// - EMITTER_NORMAL, QUAT_Normals: q1 = FindBetween(LocalSpaceUpAxis, LocalSpaceTopAxis), q2 = FindBetween(q1 x
///   LocalSpaceFacingAxis, the point brought back through Direction, normalised); rotation = Direction x (q2 q1).
/// - `EPARTICLE_DIR_TYPE_VERTEX_NORMAL`, TANGENT: the frame 0x14435BE00 hands over is FRotator(90, 270, 0) then Direction,
///   so: look along sign x Direction's Z + NUDGE, up = Direction's Y.
/// - VERTEX_NORMAL, QUAT_Normals: Direction x FindBetween(NormalUpAxis, NormalAxis).
/// - Any other direction type or mode: none (identity).
pub fn child_rotation(shape: &Shape, rot: &SpawnRot, point: Vec3) -> Mat3 {
    let dir = rotator(shape.direction.y, shape.direction.z, shape.direction.x);
    match (shape.dir_type, rot.mode) {
        (DirType::EmitterNormal, 0) => look_along((point * rot.sign + NUDGE).normalize_or_zero(), dir * Vec3::Z),
        (DirType::EmitterNormal, 1) => {
            let p = (dir.transpose() * (point * rot.sign)).normalize_or_zero();
            let q1 = between(rot.up_axis, rot.top_axis);
            let q2 = between(q1 * rot.facing_axis, p);
            dir * Mat3::from_quat(q2 * q1)
        }
        (DirType::VertexNormal, 0) => look_along(dir * Vec3::Z * rot.sign + NUDGE, dir * Vec3::Y),
        (DirType::VertexNormal, 1) => dir * Mat3::from_quat(between(rot.normal_up_axis, rot.normal_axis)),
        _ => Mat3::IDENTITY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::def::ShapeKind;

    fn shape(dir_type: DirType, direction: Vec3) -> Shape {
        Shape { kind: ShapeKind::Quad { size: Vec3::ZERO, scale: None, deviation: None }, direction, dir_type, speed: None, normal_direction: 1.0, owner_scale: true, location_scale: true, mesh_rotation: true, spawn_rot: SpawnRot::default() }
    }

    #[test]
    fn all_children_is_vertices_times_interval_rounded_up() {
        let mut rng = Rng::new(1);
        let (v, i) = (Dist::constant(15.5), Dist::constant(1.0));
        assert_eq!(all_children(&v, &i, 0.0, &mut rng), 16);
        assert_eq!(all_children(&Dist::constant(50.0), &Dist::constant(5.0), 0.0, &mut rng), 250);
        assert_eq!(all_children(&Dist::constant(0.0), &i, 0.0, &mut rng), 0, "0 corners: no child and no count");
        assert_eq!(all_children(&Dist::constant(6.0), &Dist::constant(0.0), 0.0, &mut rng), 0, "Interval 0 is not taken as 1 here");
    }

    #[test]
    fn emitter_normal_child_looks_outward_with_up_the_ring_axis() {
        let s = shape(DirType::EmitterNormal, Vec3::ZERO);
        let m = child_rotation(&s, &SpawnRot::default(), Vec3::new(0.0, 100.0, 0.0));
        // Z outward (+Y), X = Z(up) x Z = tangent, Y = Z x X = the ring axis
        assert!((m.z_axis - Vec3::Y).length() < 1.0e-3, "{m:?}");
        assert!((m.x_axis - Vec3::new(-1.0, 0.0, 0.0)).length() < 1.0e-3, "{m:?}");
        assert!((m.y_axis - Vec3::Z).length() < 1.0e-3, "{m:?}");
        // NormalDirection -1 turns it inward; a point at the centre looks along the nudge
        let inward = child_rotation(&s, &SpawnRot { sign: -1.0, ..SpawnRot::default() }, Vec3::new(0.0, 100.0, 0.0));
        assert!((inward.z_axis + Vec3::Y).length() < 1.0e-3);
        let centre = child_rotation(&s, &SpawnRot::default(), Vec3::ZERO);
        assert!((centre.z_axis - NUDGE.normalize()).length() < 1.0e-5);
    }

    #[test]
    fn vertex_normal_child_looks_along_the_axis_with_up_y() {
        let s = shape(DirType::VertexNormal, Vec3::ZERO);
        let m = child_rotation(&s, &SpawnRot::default(), Vec3::new(50.0, 0.0, 0.0));
        assert!((m.z_axis - (Vec3::Z + NUDGE).normalize()).length() < 1.0e-5);
        assert!(m.x_axis.dot(Vec3::X) > 0.999, "X = Y x Z = +X: {m:?}");
        // Direction pitch 90 (Y pitch in the Euler vector) turns the axis over
        let p = shape(DirType::VertexNormal, Vec3::new(0.0, 90.0, 0.0));
        let n = child_rotation(&p, &SpawnRot::default(), Vec3::ZERO);
        assert!((n.z_axis - rotator(90.0, 0.0, 0.0) * Vec3::Z).length() < 0.02, "{n:?}");
    }

    #[test]
    fn quat_mode_turns_the_facing_axis_onto_the_point() {
        let s = shape(DirType::EmitterNormal, Vec3::ZERO);
        let rot = SpawnRot { mode: 1, ..SpawnRot::default() };
        let m = child_rotation(&s, &rot, Vec3::new(30.0, 0.0, 0.0));
        // q1 then q2: the facing axis ends on the point's direction
        assert!((m * rot.facing_axis - Vec3::X).length() < 1.0e-4, "{m:?}");
        assert!((m.determinant() - 1.0).abs() < 1.0e-4);
    }

    #[test]
    fn none_and_velocity_place_without_turning() {
        for t in [DirType::None, DirType::Velocity] {
            assert_eq!(child_rotation(&shape(t, Vec3::new(10.0, 20.0, 30.0)), &SpawnRot::default(), Vec3::ONE), Mat3::IDENTITY);
        }
        assert_eq!(child_rotation(&shape(DirType::EmitterNormal, Vec3::ZERO), &SpawnRot { mode: 2, ..SpawnRot::default() }, Vec3::X), Mat3::IDENTITY);
    }

    #[test]
    fn parse_reads_the_mode_and_axes_with_the_constructor_defaults() {
        let v: Value = serde_json::json!({ "SpawnRotCalcMode": "ESQEX_SPAWN_ROT_CALC_MODE_QUAT_Normals", "NormalDirection": 0, "LocalSpaceUpAxis": { "x": 1, "y": 0, "z": 0 } });
        let r = SpawnRot::parse(&v);
        assert_eq!((r.mode, r.sign, r.up_axis, r.facing_axis), (1, 1.0, Vec3::X, Vec3::Z));
        assert_eq!(SpawnRot::parse(&serde_json::json!({})), SpawnRot::default());
    }
}
