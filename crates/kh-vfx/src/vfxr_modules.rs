//! (VFXR) The parsing of the particle modules the evaluator did not know before stream VFXR (def.rs calls `parse` before
//! its own table). Each module's meaning and how sure it is: its variant in def.rs; the evaluation: sim.rs (VFXR).

use std::sync::Arc;

use glam::{Vec2, Vec3};
use serde_json::Value;

use crate::def::{DirType, EmitterDef, EventDef, Module, ReceiverDef, RingOrder, Shape, ShapeKind, SpawnPerUnitDef, TailWaveDef};
use crate::dist::Dist;
use crate::math::rotator;
use crate::rng::Rng;

fn dist(p: &Value, name: &str) -> Option<Dist> {
    p.get(name).and_then(Dist::parse)
}
fn dist_or(p: &Value, name: &str, d: f32) -> Dist {
    dist(p, name).unwrap_or(Dist::constant(d))
}
fn num(p: &Value, name: &str, d: f32) -> f32 {
    p.get(name).and_then(Value::as_f64).map_or(d, |v| v as f32)
}
fn flag(p: &Value, name: &str, d: bool) -> bool {
    p.get(name).and_then(Value::as_bool).unwrap_or(d)
}
fn s<'a>(p: &'a Value, name: &str) -> &'a str {
    p.get(name).and_then(Value::as_str).unwrap_or("")
}
fn vec3(v: Option<&Value>, d: Vec3) -> Vec3 {
    match v {
        Some(v) if v.get("x").is_some() => Vec3::new(num(v, "x", d.x), num(v, "y", d.y), num(v, "z", d.z)),
        _ => d,
    }
}
fn vec2(v: Option<&Value>, d: Vec2) -> Vec2 {
    match v {
        Some(v) if v.get("x").is_some() => Vec2::new(num(v, "x", d.x), num(v, "y", d.y)),
        _ => d,
    }
}

/// The shape fields every SQEX_LocationBase module carries (as def.rs `shape_common`).
fn shape(p: &Value, kind: ShapeKind) -> Shape {
    let dir = s(p, "DirectionType");
    let r = p.get("Direction");
    Shape {
        kind,
        direction: r.map_or(Vec3::ZERO, |r| Vec3::new(num(r, "roll", 0.0), num(r, "pitch", 0.0), num(r, "yaw", 0.0))),
        dir_type: if dir.ends_with("EMITTER_NORMAL") {
            DirType::EmitterNormal
        } else if dir.ends_with("VERTEX_NORMAL") {
            DirType::VertexNormal
        } else if dir.ends_with("VELOCITY") {
            DirType::Velocity
        } else {
            DirType::None
        },
        speed: dist(p, "Speed"),
        normal_direction: num(p, "NormalDirection", 1.0).clamp(-1.0, 1.0),
        owner_scale: flag(p, "bApplyOwnerScale", true),
        location_scale: flag(p, "bApplySpawnLocationOwnerScale", true),
        mesh_rotation: !flag(p, "bNoMeshRotation", false) && !s(p, "SpawnRotCalcMode").ends_with("QUAT_Normals"),
    }
}

fn order_of(p: &Value) -> RingOrder {
    let o = s(p, "VartexGenerateType");
    if o.ends_with("_ORDER") {
        RingOrder::Order
    } else if o.ends_with("_ALL") {
        RingOrder::All
    } else {
        RingOrder::Random
    }
}

/// `SQEX_TexSpawn`'s baked pixels (u, v, colour) as points of a SpawnMaxSizeX x SpawnMaxSizeY rectangle in the node's XY
/// plane (texture up = +X forward, right = +Y right; centred with OnCenterTextrue) [A], turned by RotatorOffset and moved by
/// LocationOffset. The particle's normal is the plane's (+Z).
pub fn tex_points(p: &Value) -> Option<Vec<[f32; 10]>> {
    let baked = p.get("kh_points")?.get("points")?.as_array()?;
    let (sx, sy) = (num(p, "SpawnMaxSizeX", 100.0) * num(p, "TexScaleU", 1.0), num(p, "SpawnMaxSizeY", 100.0) * num(p, "TexScaleV", 1.0));
    let centre = flag(p, "OnCenterTextrue", true);
    let r = p.get("RotatorOffset");
    let turn = rotator(r.map_or(0.0, |r| num(r, "pitch", 0.0)), r.map_or(0.0, |r| num(r, "yaw", 0.0)), r.map_or(0.0, |r| num(r, "roll", 0.0)));
    let offset = vec3(p.get("LocationOffset"), Vec3::ZERO);
    let normal = turn * Vec3::Z;
    Some(
        baked
            .iter()
            .filter_map(|q| {
                let q = q.as_array()?;
                let f = |k: usize| q.get(k).and_then(Value::as_f64).unwrap_or(0.0) as f32;
                let (u, v) = (f(0), f(1));
                let (u, v) = if centre { (u - 0.5, v - 0.5) } else { (u, v) };
                let local = turn * Vec3::new(-v * sy, u * sx, 0.0) + offset;
                Some([local.x, local.y, local.z, normal.x, normal.y, normal.z, f(2), f(3), f(4), f(5)])
            })
            .collect(),
    )
}

/// `SQEX_VertexSpawn`'s baked vertices (mesh space, cm), as they are.
pub fn vertex_points(p: &Value) -> Option<Vec<[f32; 10]>> {
    let baked = p.get("kh_points")?.get("points")?.as_array()?;
    Some(
        baked
            .iter()
            .filter_map(|q| {
                let q = q.as_array()?;
                let f = |k: usize| q.get(k).and_then(Value::as_f64).unwrap_or(0.0) as f32;
                Some([f(0), f(1), f(2), f(3), f(4), f(5), f(6), f(7), f(8), f(9)])
            })
            .collect(),
    )
}

/// `SQEX_AnyVertexSpawn`'s generated vertices [A: modelled from its properties; the generator was not traced]: HNumber
/// rows (a cylinder: rows from -R to +R along Z with the radius going from RangeRadius to RangeRadiusTwo when that is set;
/// a sphere: latitude rows), WNumber points around each over DirectionAngle, `Interval` points along each edge between two
/// of them, the poles / end rows left out with bNoTopAndFinalPoint; then OffsetScale, OffsetRotation (X roll, Y pitch,
/// Z yaw) and OffsetPosition. Normal: away from the shape's axis (cylinder) or centre (sphere).
pub fn any_vertex_points(p: &Value) -> Vec<[f32; 10]> {
    let mut rng = Rng::new(1);
    let f = |name: &str, d: f32, rng: &mut Rng| dist(p, name).map_or(d, |x| x.float(0.0, rng));
    let sphere = s(p, "ShapeType").ends_with("_SPHERE");
    let h = num(p, "HNumber", 2.0).max(1.0) as usize;
    let w = num(p, "WNumber", 4.0).max(1.0) as usize;
    let r1 = f("RangeRadius", 50.0, &mut rng);
    let r2 = f("RangeRadiusTwo", 0.0, &mut rng);
    let angle = f("DirectionAngle", 360.0, &mut rng).to_radians();
    let interval = f("Interval", 1.0, &mut rng).round().max(1.0) as usize;
    let vscale = f("VertexHorizontalScale", 1.0, &mut rng);
    let no_ends = flag(p, "bNoTopAndFinalPoint", false);
    let same = flag(p, "bTowRadiusSameHeightOneRadius", false);
    let off_s = dist(p, "OffsetScale").map_or(Vec3::ONE, |d| d.vec3(0.0, &mut rng));
    let off_r = dist(p, "OffsetRotation").map_or(Vec3::ZERO, |d| d.vec3(0.0, &mut rng));
    let off_p = dist(p, "OffsetPosition").map_or(Vec3::ZERO, |d| d.vec3(0.0, &mut rng));
    let turn = crate::math::euler(off_r);
    let mut out = Vec::new();
    let rows: Vec<usize> = (0..h).filter(|k| !(no_ends && sphere && (*k == 0 || *k + 1 == h) && h > 2)).collect();
    for k in rows {
        let t = if h > 1 { k as f32 / (h - 1) as f32 } else { 0.5 };
        let (radius, z) = if sphere {
            let lat = (t - 0.5) * std::f32::consts::PI;
            (r1 * lat.cos(), r1 * lat.sin())
        } else {
            let r = if r2 != 0.0 && !same { r1 + (r2 - r1) * t } else { r1 };
            (r, (t * 2.0 - 1.0) * r1 * vscale)
        };
        let corners = if angle >= std::f32::consts::TAU - 1.0e-3 { w } else { w + 1 };
        for c in 0..corners {
            for j in 0..interval {
                let a = (c as f32 + j as f32 / interval as f32) / w as f32 * angle;
                if c + 1 == corners && j > 0 && corners != w {
                    break;
                }
                let local = Vec3::new(a.cos() * radius, a.sin() * radius, z);
                let normal = if sphere { local.normalize_or_zero() } else { Vec3::new(a.cos(), a.sin(), 0.0) };
                let pt = turn * (local * off_s) + off_p;
                let n = (turn * normal).normalize_or_zero();
                out.push([pt.x, pt.y, pt.z, n.x, n.y, n.z, 1.0, 1.0, 1.0, 1.0]);
            }
        }
    }
    if out.is_empty() {
        out.push([off_p.x, off_p.y, off_p.z, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
    }
    out
}

/// The module `short` (class without `ParticleModule`): Some(Some(m)) a per-particle module, Some(None) one kept on the
/// emitter (or nothing to do), None: not one of VFXR's.
pub fn parse(short: &str, p: &Value, def: &mut EmitterDef) -> Option<Option<Module>> {
    let axes6 = |p: &Value| [flag(p, "Positive_X", true), flag(p, "Negative_X", true), flag(p, "Positive_Y", true), flag(p, "Negative_Y", true), flag(p, "Positive_Z", true), flag(p, "Negative_Z", true)];
    Some(match short {
        "SQEX_TexSpawn" | "SQEX_VertexSpawn" | "SQEX_AnyVertexSpawn" => {
            let attach_mesh = short == "SQEX_VertexSpawn" && flag(p, "bUseAttachMeshVertex", false);
            let points = match short {
                "SQEX_TexSpawn" => tex_points(p),
                "SQEX_VertexSpawn" if attach_mesh => Some(Vec::new()),
                "SQEX_VertexSpawn" => vertex_points(p),
                _ => Some(any_vertex_points(p)),
            };
            match points {
                Some(points) => Some(Module::Shape(shape(p, ShapeKind::Points { points: Arc::new(points), order: order_of(p), attach_mesh }))),
                None => {
                    // (a pack that was not baked: tools vfxr_bake.js) the particles start at the node, as before
                    def.unknown.push(format!("{short} (its points are not baked: build/sora_tmp/vfxr/tools/vfxr_bake.js)"));
                    None
                }
            }
        }
        "SQEX_SphereForce" => {
            let fd = p.get("ForceData").cloned().unwrap_or(Value::Null);
            Some(Module::SphereForce {
                center: dist(p, "CollPos").unwrap_or(Dist::constant_vec(vec3(fd.get("ForcePosition"), Vec3::ZERO))),
                radius: num(&fd, "ForceRadius", 50.0),
                radius_scale: dist_or(p, "RadiusScale", 1.0),
                internal: s(&fd, "ForceType").ends_with("_INTERNAL"),
                hit_size: num(&fd, "HitParticleSizeScale", 1.0),
            })
        }
        "AttractorPoint" => Some(Module::Attractor {
            position: dist(p, "Position").unwrap_or(Dist::constant_vec(Vec3::ZERO)),
            range: dist_or(p, "Range", 0.0),
            strength: dist_or(p, "Strength", 0.0),
            by_distance: flag(p, "StrengthByDistance", true),
            world: flag(p, "bUseWorldSpacePosition", false),
            affect_base: flag(p, "bAffectBaseVelocity", false),
            override_velocity: flag(p, "bOverrideVelocity", false),
            axes: axes6(p),
        }),
        "AttractorPointGravity" => Some(Module::AttractorGravity { position: vec3(p.get("Position"), Vec3::ZERO), radius: num(p, "Radius", 50.0), strength: dist(p, "StrengthRaw").or(dist(p, "Strength")).unwrap_or(Dist::constant(0.0)) }),
        "Collision" => Some(Module::Collision {
            max: dist_or(p, "MaxCollisions", 0.0),
            damping: dist(p, "DampingFactor").unwrap_or(Dist::constant_vec(Vec3::ONE)),
            completion: match s(p, "CollisionCompletionOption") {
                "EPCC_Freeze" | "EPCC_FreezeTranslation" | "EPCC_FreezeMovement" => 1,
                "EPCC_HaltCollisions" | "EPCC_FreezeRotation" => 2,
                _ => 0,
            },
        }),
        "CollisionGPU" => Some(Module::CollisionGpu {
            resilience: dist_or(p, "Resilience", 0.0),
            resilience_scale: dist_or(p, "ResilienceScaleOverLife", 1.0),
            friction: num(p, "Friction", 0.0),
            radius_scale: num(p, "RadiusScale", 1.0),
            radius_bias: num(p, "RadiusBias", 0.0),
            response: match s(p, "Response") {
                r if r.ends_with("Stop") => 1,
                r if r.ends_with("Kill") => 2,
                _ => 0,
            },
        }),
        "SizeScaleBySpeed" => Some(Module::SizeBySpeed { speed_scale: vec2(p.get("SpeedScale"), Vec2::ONE), max: vec2(p.get("MaxScale"), Vec2::ONE) }),
        "AccelerationOverLifetime" => Some(Module::AccelOverLife { accel: dist(p, "AccelOverLife").unwrap_or(Dist::constant_vec(Vec3::ZERO)), world: flag(p, "bAlwaysInWorldSpace", false) }),
        "SQEX_ParticleLocation" => Some(Module::ParticleLocation {
            location: dist(p, "Location").unwrap_or(Dist::constant_vec(Vec3::ZERO)),
            start_scale: dist(p, "StartScale").unwrap_or(Dist::constant_vec(Vec3::ONE)),
            world: flag(p, "bInWorldSpace", false),
            axes: [flag(p, "EnableX", true), flag(p, "EnableY", true), flag(p, "EnableZ", true)],
        }),
        "SpawnPerUnit" => {
            def.spawn_per_unit = Some(SpawnPerUnitDef {
                per_unit: dist_or(p, "SpawnPerUnit", 0.0),
                unit: num(p, "UnitScalar", 50.0).max(1.0e-3),
                tolerance: num(p, "MovementTolerance", 0.1),
                ignore_rate_when_moving: flag(p, "bIgnoreSpawnRateWhenMoving", false),
                axes: [!flag(p, "bIgnoreMovementAlongX", false), !flag(p, "bIgnoreMovementAlongY", false), !flag(p, "bIgnoreMovementAlongZ", false)],
                max_frame_distance: num(p, "MaxFrameDistance", 0.0),
            });
            None
        }
        "EventGenerator" => {
            for e in p.get("Events").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
                let kind = match s(e, "Type") {
                    "EPET_Spawn" => 0,
                    "EPET_Death" => 1,
                    "EPET_Collision" => 2,
                    _ => 3,
                };
                def.events.push(EventDef { kind, name: s(e, "CustomName").to_string() });
            }
            None
        }
        "EventReceiverSpawn" => {
            def.receivers.push(ReceiverDef {
                name: s(p, "EventName").to_string(),
                kind: match s(p, "EventGeneratorType") {
                    "EPET_Spawn" => 0,
                    "EPET_Death" => 1,
                    "EPET_Collision" => 2,
                    _ => 255,
                },
                count: dist_or(p, "SpawnCount", 1.0),
                inherit_velocity: flag(p, "bInheritVelocity", false).then(|| dist(p, "InheritVelocityScale").unwrap_or(Dist::constant_vec(Vec3::ONE))),
            });
            None
        }
        // a global vector field: the hosts' levels have none (every user of the pack sets scale 0, no override): nothing moves
        "VectorFieldGlobal" => None,
        "SQEX_EmitterDirectEX" => {
            let name = s(p, "EmitterName").to_string();
            if !name.is_empty() {
                def.direct_ex = Some(name);
            }
            None
        }
        "SQEX_TailShapeExData_Inertia" => {
            def.tail_inertia = dist(p, "TailInertia");
            None
        }
        "SQEX_TailShapeExData_Wave" => {
            def.tail_wave = Some(TailWaveDef { move_vector: vec3(p.get("MoveVector"), Vec3::ZERO), random: dist(p, "RandomMoveVector").filter(|d| d.max_abs() != 0.0), interval: num(p, "RandomInterval", 0.0) });
            None
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tex_points_lie_in_the_spawn_rectangle() {
        let p: Value = serde_json::from_str(r#"{"SpawnMaxSizeX":320,"SpawnMaxSizeY":320,"TexScaleU":1,"TexScaleV":1,"OnCenterTextrue":true,
            "RotatorOffset":{"pitch":0,"yaw":0,"roll":0},"LocationOffset":{"x":0,"y":0,"z":0},
            "kh_points":{"points":[[0.5,0.5,1,1,1,1],[1.0,0.0,1,0,0,1],[0.0,1.0,0,1,0,1]]}}"#).unwrap();
        let pts = tex_points(&p).unwrap();
        assert_eq!(pts.len(), 3);
        assert_eq!(&pts[0][..3], &[0.0, 0.0, 0.0]);
        // texture right = +Y, texture up = +X
        assert_eq!(&pts[1][..3], &[160.0, 160.0, 0.0]);
        assert_eq!(&pts[2][..3], &[-160.0, -160.0, 0.0]);
        assert_eq!(&pts[1][3..6], &[0.0, 0.0, 1.0]);
        assert_eq!(&pts[2][6..10], &[0.0, 1.0, 0.0, 1.0]);
    }

    #[test]
    fn any_vertex_cylinder_and_sphere() {
        let c: Value = serde_json::from_str(r#"{"ShapeType":"SQEX_ANY_VERTEX_SHAPE_TYPE_CYLINDER","HNumber":3,"WNumber":2,"RangeRadius":0.3,"RangeRadiusTwo":0,"DirectionAngle":360,"Interval":1,"VertexHorizontalScale":1,"bNoTopAndFinalPoint":false}"#).unwrap();
        let pts = any_vertex_points(&c);
        assert_eq!(pts.len(), 6);
        assert!(pts.iter().all(|q| (Vec3::new(q[0], q[1], 0.0).length() - 0.3).abs() < 1.0e-4));
        let sph: Value = serde_json::from_str(r#"{"ShapeType":"SQEX_ANY_VERTEX_SHAPE_TYPE_SPHERE","HNumber":5,"WNumber":4,"RangeRadius":45,"DirectionAngle":360,"Interval":1,"bNoTopAndFinalPoint":true}"#).unwrap();
        let pts = any_vertex_points(&sph);
        assert_eq!(pts.len(), 3 * 4);
        assert!(pts.iter().all(|q| (Vec3::new(q[0], q[1], q[2]).length() - 45.0).abs() < 1.0e-3));
    }

    #[test]
    fn modules_parse_into_their_variants() {
        let mut def = crate::def::EffectDef::parse(r#"{"name":"x","emitters":[{"name":"e","required":{},"modules":[]}]}"#).unwrap().emitters.remove(0);
        let sf: Value = serde_json::from_str(r#"{"ForceData":{"HitParticleSizeScale":0.3,"ForceRadius":50,"ForceType":"SQEX_PARTICLE_SPHERE_COLL_HIT_TYPE_REFLECTION_INTERNAL"},"RadiusScale":1}"#).unwrap();
        assert!(matches!(parse("SQEX_SphereForce", &sf, &mut def), Some(Some(Module::SphereForce { internal: true, radius, .. })) if radius == 50.0));
        let ev: Value = serde_json::from_str(r#"{"Events":[{"Type":"EPET_Death","CustomName":"sbk"}]}"#).unwrap();
        assert!(matches!(parse("EventGenerator", &ev, &mut def), Some(None)));
        assert_eq!(def.events, vec![EventDef { kind: 1, name: "sbk".into() }]);
        let unbaked: Value = serde_json::from_str(r#"{"SpawnMaxSizeX":10}"#).unwrap();
        assert!(matches!(parse("SQEX_TexSpawn", &unbaked, &mut def), Some(None)));
        assert!(def.unknown.iter().any(|u| u.contains("not baked")));
        assert!(parse("Lifetime", &Value::Null, &mut def).is_none());
    }
}
