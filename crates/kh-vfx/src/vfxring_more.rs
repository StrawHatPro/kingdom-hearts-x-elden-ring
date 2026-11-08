//! (VFX-RING) The particle modules the evaluator skipped until now, traced from the KH3 executable (addresses [C]):
//!
//! - `ParticleModuleLocationWorldOffset` (Spawn 0x1442F5180, the stock UE 4.17 SpawnEx): StartLocation at the EMITTER
//!   TIME (vt+0x2C8) added to the particle's place as a WORLD vector: as it is when the emitter simulates in the world
//!   (Required +0x94 bit 0 = bUseLocalSpace off), through the inverse of the component's matrix WITH its scale (the
//!   quaternion / scale at component +0x2A0 / +0x2C0, FUN_140587FB0 = InverseFast) when it simulates in the component's
//!   space. Unlike `ParticleModuleLocation` the node's rotation does not turn it.
//! - `ParticleModuleAccelerationDragScaleOverLife` (its only work, CompileModule 0x1442B56A0: the GPU emitter build info
//!   +0x290 = DragScaleRaw; PostInitProperties 0x1442C16F0 makes the default constant 1): no CPU Spawn / Update (slots
//!   0x240 / 0x248 are the base's), so it acts on GPU sprite emitters only, where it scales the drag coefficient over the
//!   particle's life (UE 4.17 GPU simulation: drag = DragCoefficient x DragScale(RelativeTime)). kh-vfx runs GPU sprites
//!   on the CPU, so `finish` folds it into that emitter's `AccelerationDrag` (`More::ScaledDrag`); without a drag module
//!   it changes nothing, as in the game (no coefficient to scale).
//! - `ParticleModuleSQEX_ParticleColor` (Spawn 0x1442F27F0, Update 0x1442FD120; payload: StartParticleColor rgb, then
//!   StartParticleAlpha, then its own time): the time is the EMITTER time +0x12C (the raw time, not less the start time)
//!   with bEmitterTime, else the particle's RelativeTime. c = ParticleColorScale(t) (1 without a distribution) x
//!   ParticleColor(t), a = ParticleAlpha(t) x ParticleAlphaScale(t) (1 without). With bCurveParameterDirectColor:
//!   Color.rgb = start.rgb x BaseColor.rgb x c x E.rgb, Color.a = BaseColor.a x start.a x a x E.a; without it the same
//!   products are ADDED to Color, and Color.a also gets BaseColor.a once more (the FVector -> FLinearColor conversions
//!   FUN_142D34A30 give A = 1). E is the emitter instance's colour (+0x480 / +0x48C: the takeover / system colour), which
//!   kh-vfx multiplies in when it draws (`draw_system` tint): taken as 1 here so it is not applied twice. Spawn writes
//!   the same colour once at birth; the module runs again every tick after the stock colour reset.

use glam::{Affine3A, Vec3};
use serde_json::Value;

use crate::def::{BeamPoint, EmitterDef, Module, ShapeKind};
use crate::dist::Dist;
use crate::rng::Rng;

/// The modules of this file (`Module::More`).
#[derive(Clone, Debug, PartialEq)]
pub enum More {
    /// `ParticleModuleLocationWorldOffset`: StartLocation.
    WorldOffset(Dist),
    /// `ParticleModuleSQEX_ParticleColor`.
    ParticleColor(Box<ParticleColor>),
    /// `ParticleModuleAccelerationDragScaleOverLife` as parsed (DragScaleRaw); `finish` folds it into the drag.
    DragScale(Dist),
    /// `AccelerationDrag` x `AccelerationDragScaleOverLife` on a GPU sprite emitter (both at RelativeTime).
    ScaledDrag { coefficient: Dist, scale: Dist },
}

/// `ParticleModuleSQEX_ParticleColor`'s properties.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleColor {
    pub start_color: Dist,
    pub start_alpha: Dist,
    pub color: Dist,
    pub alpha: Dist,
    pub color_scale: Option<Dist>,
    pub alpha_scale: Option<Dist>,
    pub emitter_time: bool,
    pub direct: bool,
    /// The emitter's EmitterStartTime (set by `finish`): +0x12C is the raw time, kh-vfx's emitter time is less it.
    pub start_time: f32,
}

fn dist(p: &Value, name: &str) -> Option<Dist> {
    p.get(name).and_then(Dist::parse)
}

/// The module of class `short` (without "ParticleModule" and "_Seeded"), or None when it is not one of this file's.
pub fn parse(short: &str, p: &Value) -> Option<Module> {
    let flag = |name: &str, d: bool| p.get(name).and_then(Value::as_bool).unwrap_or(d);
    Some(Module::More(match short {
        "LocationWorldOffset" => More::WorldOffset(dist(p, "StartLocation").unwrap_or(Dist::constant_vec(Vec3::ZERO))),
        // PostInitProperties 0x1442C16F0: a missing DragScaleRaw is a constant 1
        "AccelerationDragScaleOverLife" => More::DragScale(dist(p, "DragScaleRaw").or(dist(p, "DragScale")).unwrap_or(Dist::constant(1.0))),
        "SQEX_ParticleColor" => More::ParticleColor(Box::new(ParticleColor {
            start_color: dist(p, "StartParticleColor").unwrap_or(Dist::constant_vec(Vec3::ONE)),
            start_alpha: dist(p, "StartParticleAlpha").unwrap_or(Dist::constant(1.0)),
            color: dist(p, "ParticleColor").unwrap_or(Dist::constant_vec(Vec3::ONE)),
            alpha: dist(p, "ParticleAlpha").unwrap_or(Dist::constant(1.0)),
            color_scale: dist(p, "ParticleColorScale"),
            alpha_scale: dist(p, "ParticleAlphaScale"),
            emitter_time: flag("bEmitterTime", false),
            direct: flag("bCurveParameterDirectColor", false),
            start_time: 0.0,
        })),
        _ => return None,
    }))
}

/// After all modules of an emitter are parsed: the drag scale folded into the drag of a GPU sprite emitter (dropped on
/// any other emitter, and with no drag), and the emitter's start time handed to its colour modules.
pub fn finish(def: &mut EmitterDef) {
    let scale = def.modules.iter().find_map(|m| if let Module::More(More::DragScale(d)) = m { Some(d.clone()) } else { None });
    def.modules.retain(|m| !matches!(m, Module::More(More::DragScale(_))));
    if let (Some(scale), true) = (scale, def.kind_name == "gpu_sprite") {
        for m in &mut def.modules {
            if let Module::Drag(c) = m {
                *m = Module::More(More::ScaledDrag { coefficient: c.clone(), scale: scale.clone() });
            }
        }
    }
    // TypeDataSQEX_Thunder MY_MODULE: evaluated with an SQEX_Ring (sim.rs my_module_ends = 0x14424FAF0); the Range and
    // AnyVertexSpawn rules (0x14424F620 / 0x14424D6E0) have no user in the pack
    let my_module = def.beam.as_ref().is_some_and(|b| b.start == BeamPoint::MyModule || b.end == BeamPoint::MyModule);
    if my_module && !matches!(def.shape().map(|s| &s.kind), Some(ShapeKind::Ring { .. })) {
        def.unknown.push("sqex_thunder MY_MODULE without an SQEX_Ring (the Range / AnyVertexSpawn rules are not evaluated)".to_string());
    }
    let start = def.start_time;
    for m in &mut def.modules {
        if let Module::More(More::ParticleColor(c)) = m {
            c.start_time = start;
        }
    }
}

/// LocationWorldOffset (0x1442F5180): the offset at emitter time `t`, from the world into the simulation space
/// (`to_world`: simulation -> world; the identity for a world-space emitter).
pub fn world_offset(d: &Dist, t: f32, rng: &mut Rng, to_world: &Affine3A) -> Vec3 {
    let v = d.vec3(t, rng);
    if *to_world == Affine3A::IDENTITY {
        v
    } else {
        to_world.inverse().transform_vector3(v)
    }
}

impl ParticleColor {
    /// The module's time: the raw emitter time (`t` + EmitterStartTime) with bEmitterTime, else the particle's.
    fn time(&self, t: f32, rel: f32) -> f32 {
        if self.emitter_time {
            t + self.start_time
        } else {
            rel
        }
    }

    /// Spawn 0x1442F27F0: the payload (StartParticleColor rgb, StartParticleAlpha) for a particle born at emitter time
    /// `t` with RelativeTime `rel`.
    pub fn start(&self, t: f32, rel: f32, rng: &mut Rng) -> [f32; 4] {
        let time = self.time(t, rel);
        let c = self.start_color.vec3(time, rng);
        [c.x, c.y, c.z, self.start_alpha.float(time, rng)]
    }

    /// Spawn 0x1442F27F0 / Update 0x1442FD120: the particle's colour from its payload `start`, its BaseColor `base` and
    /// the colour it has now (`now`, the stock reset to BaseColor and what the modules before this one made of it).
    pub fn apply(&self, start: [f32; 4], base: [f32; 4], now: [f32; 4], t: f32, rel: f32, rng: &mut Rng) -> [f32; 4] {
        let time = self.time(t, rel);
        let scale = self.color_scale.as_ref().map_or(Vec3::ONE, |d| d.vec3(time, rng));
        let alpha_scale = self.alpha_scale.as_ref().map_or(1.0, |d| d.float(time, rng));
        let c = scale * self.color.vec3(time, rng);
        let a = self.alpha.float(time, rng);
        let rgb = [start[0] * base[0] * c.x, start[1] * base[1] * c.y, start[2] * base[2] * c.z];
        let alpha = base[3] * start[3] * a * alpha_scale;
        if self.direct {
            [rgb[0], rgb[1], rgb[2], alpha]
        } else {
            [now[0] + rgb[0], now[1] + rgb[1], now[2] + rgb[2], now[3] + base[3] + alpha]
        }
    }
}

/// The scaled drag of one tick (UE 4.17 drag: v -= v x coefficient x dt; here x DragScale(RelativeTime)).
pub fn scaled_drag(coefficient: &Dist, scale: &Dist, rel: f32, dt: f32, rng: &mut Rng) -> f32 {
    coefficient.float(rel, rng) * scale.float(rel, rng) * dt
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Quat;
    use serde_json::json;

    /// A two-key linear curve over 0 .. 1 as the pack bakes it (its lookup table).
    fn curve(keys: &[(f32, f32)]) -> Value {
        let values: Vec<f32> = keys.iter().map(|(_, v)| *v).collect();
        json!({ "distribution": { "type": "constant_curve", "lookup_table": { "op": 1, "entry_count": values.len(), "entry_stride": 1, "sub_entry_stride": 0, "time_scale": (values.len() - 1) as f32, "time_bias": 0.0, "lock_flag": 0, "values": values } } })
    }

    #[test]
    fn world_offset_ignores_the_node_turn_and_undoes_the_component_in_local_space() {
        let mut rng = Rng::new(1);
        let d = Dist::constant_vec(Vec3::new(-100.0, 0.0, 0.0));
        assert_eq!(world_offset(&d, 0.0, &mut rng, &Affine3A::IDENTITY), Vec3::new(-100.0, 0.0, 0.0));
        // local space: a component turned 90 degrees about Z and scaled 2: the world -X offset is +Y / 2 in its space
        let comp = Affine3A::from_scale_rotation_translation(Vec3::splat(2.0), Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), Vec3::new(5.0, 6.0, 7.0));
        let o = world_offset(&d, 0.0, &mut rng, &comp);
        assert!((comp.transform_vector3(o) - Vec3::new(-100.0, 0.0, 0.0)).length() < 1.0e-3, "{o:?}");
        assert!((o - Vec3::new(0.0, 50.0, 0.0)).length() < 1.0e-3, "{o:?}");
    }

    #[test]
    fn drag_scale_folds_into_the_drag_of_a_gpu_emitter_only() {
        let p = json!({ "DragScaleRaw": curve(&[(0.0, 0.0), (1.0, 1.0)]) });
        let Some(Module::More(More::DragScale(scale))) = parse("AccelerationDragScaleOverLife", &p) else { panic!("not parsed") };
        let mut rng = Rng::new(1);
        assert!((scale.float(0.5, &mut rng) - 0.5).abs() < 1.0e-3);
        // no DragScaleRaw: PostInitProperties' constant 1
        let Some(Module::More(More::DragScale(one))) = parse("AccelerationDragScaleOverLife", &json!({})) else { panic!() };
        assert_eq!(one.float(0.7, &mut rng), 1.0);
        let c = Dist::constant(2.0);
        assert!((scaled_drag(&c, &scale, 0.25, 0.5, &mut rng) - 0.25).abs() < 1.0e-3, "2 x 0.25 x 0.5");
    }

    #[test]
    fn particle_color_direct_sets_and_otherwise_adds() {
        let p = json!({
            "bEmitterTime": true, "bCurveParameterDirectColor": true,
            "StartParticleColor": { "distribution": { "type": "constant", "value_kind": "vector", "constant": [1.0, 5.0, 0.2] } },
            "StartParticleAlpha": { "distribution": { "type": "constant", "value_kind": "float", "constant": 0.5 } },
            "ParticleColor": { "distribution": { "type": "constant", "value_kind": "vector", "constant": [2.0, 1.0, 5.0] } },
            "ParticleAlpha": curve(&[(0.0, 1.0), (1.0, 0.0)]),
            "ParticleAlphaScale": { "distribution": { "type": "constant", "value_kind": "float", "constant": 1.0 } }
        });
        let Some(Module::More(More::ParticleColor(mut c))) = parse("SQEX_ParticleColor", &p) else { panic!("not parsed") };
        c.start_time = 0.2;
        let mut rng = Rng::new(1);
        let start = c.start(0.0, 0.0, &mut rng);
        assert_eq!(start, [1.0, 5.0, 0.2, 0.5]);
        let base = [0.5, 1.0, 2.0, 1.0];
        // emitter time 0.3 (kh-vfx) = raw 0.5: ParticleAlpha 0.5
        let out = c.apply(start, base, base, 0.3, 0.9, &mut rng);
        assert!((out[0] - 1.0).abs() < 1.0e-4 && (out[1] - 5.0).abs() < 1.0e-4 && (out[2] - 2.0).abs() < 1.0e-4, "{out:?}");
        assert!((out[3] - 0.25).abs() < 1.0e-3, "1 x 0.5 x 0.5 x 1: {out:?}");
        // not direct: added, alpha with BaseColor.a once more
        c.direct = false;
        let now = [0.1, 0.1, 0.1, 0.3];
        let add = c.apply(start, base, now, 0.3, 0.9, &mut rng);
        assert!((add[0] - 1.1).abs() < 1.0e-4 && (add[3] - (0.3 + 1.0 + 0.25)).abs() < 1.0e-3, "{add:?}");
        // without bEmitterTime the particle's own time runs the curves
        c.emitter_time = false;
        c.direct = true;
        let late = c.apply(start, base, base, 0.0, 1.0, &mut rng);
        assert!(late[3].abs() < 1.0e-4, "{late:?}");
    }

    #[test]
    fn other_classes_are_not_this_files() {
        assert!(parse("Location", &json!({})).is_none());
        assert!(parse("SQEX_TailNoise", &json!({})).is_none());
    }
}
