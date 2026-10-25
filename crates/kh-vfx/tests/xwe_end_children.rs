//! (XW-e) SQEX_Parent entries timed in the parent's END section (at or past EmitterLoopDuration) generate once after the
//! parent's loop ends, whatever its loop count; entries timed in the looped section still generate in their first
//! LoopNumber passes. Plus Chandelier's ice flowers (vw_so088_fin_zmn0) breaking into their pieces once stopped.

use glam::Vec3;
use kh_vfx::{Camera, DrawList, EffectDef, SpawnOptions, World, Xform};
use serde_json::{json, Value};

const DT: f32 = 1.0 / 60.0;

fn constant(v: Value) -> Value {
    json!({"distribution": {"type": "constant", "constant": v}})
}

fn emitter(name: &str, kind: &str, loops: u32, time: (f32, f32, f32, f32), bursts: Value, modules: Vec<Value>) -> Value {
    json!({
        "name": name, "enabled": true, "typedata": {"kind": kind},
        "required": {
            "material": "/Game/Effects/test/mat/m_test.m_test", "screen_alignment": "PSA_Square", "use_local_space": false, "kill_on_completed": true, "duration": time.3, "loops": loops,
            "sqex_emitter_time": {"start_time": time.0, "loop_time": time.1, "loop_duration": time.2, "duration_time": time.3, "use_legacy_emitter_time": false},
            "sub_uv": {"horizontal": 1, "vertical": 1, "interpolation": "PSUVIM_None"}, "use_max_draw_count": true, "max_draw_count": 500,
            "emitter_origin": {"x": 0, "y": 0, "z": 0}, "emitter_rotation": {"pitch": 0, "yaw": 0, "roll": 0},
            "params": {"bAutoAddToComponent": true, "RotationPriority": "ROTATION_PRIORITY_XZY"}
        },
        "spawn": {"enabled": true, "process_spawn_rate": true, "process_burst_list": true, "rate": constant(json!(0.0)), "rate_scale": constant(json!(1.0)), "burst_scale": constant(json!(1.0)), "bursts": bursts},
        "modules": modules
    })
}

fn child(name: &str, time: f32, loop_number: u32) -> Value {
    json!({"GenerationConditions": "ESQEX_CREATE_GENERATE_TYPE_TIME", "GenerateEmitterName": name, "MaxGenerateCount": 0, "TimeDatas": [{"GenerateTime": time, "GenerateNumber": 1}], "LoopNumber": loop_number, "LoopEndTime": 0})
}

/// A looping control (looped section [0, 0.1], played to 2 s once its loop ends) with an end child at 1.4 s and a loop
/// child at 0.05 s, both LoopNumber 1; each child bursts one sprite of 0.3 s.
fn effect() -> EffectDef {
    let parent = {
        let mut p = emitter("Control_lp", "sqex_control", 0, (0.0, 0.0, 0.1, 2.0), json!([]), vec![]);
        p["modules"] = json!([{"class": "ParticleModuleSQEX_Parent", "enabled": true, "params": {"ChildrenData": [child("end_child", 1.4, 1), child("loop_child", 0.05, 1)], "AllLoopEndTime": 0}}]);
        p
    };
    // (templates of the parent only: bAutoAddToComponent false)
    let sprite = |name: &str| { let mut e = emitter(name, "sprite", 1, (0.0, 0.0, 0.3, 0.3), json!([{"count": 1, "count_low": -1, "time": 0.0}]), vec![json!({"class": "ParticleModuleLifetime", "enabled": true, "params": {"Lifetime": constant(json!(0.3))}}), json!({"class": "ParticleModuleSize", "enabled": true, "params": {"StartSize": constant(json!([10.0, 10.0, 10.0]))}})]); e["required"]["params"]["bAutoAddToComponent"] = json!(false); e };
    EffectDef::from_json(&json!({"name": "end_children", "system": {"params": {}}, "emitters": [parent, sprite("end_child"), sprite("loop_child")]})).unwrap()
}

fn emitters_named(world: &World, name: &str) -> usize {
    world.live().iter().flat_map(|l| l.emitters.iter()).filter(|e| e.0 == name).count()
}

#[test]
fn an_end_section_child_generates_once_the_loop_ends() {
    let mut world = World::new();
    world.register(effect());
    let id = world.spawn("end_children", Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
    let mut loop_children = 0;
    for _ in 0..120 {
        world.step(DT);
        loop_children = loop_children.max(emitters_named(&world, "loop_child"));
    }
    // two seconds of looping: the loop child came in the first pass only, the end child never
    assert_eq!(loop_children, 1);
    assert_eq!(emitters_named(&world, "end_child"), 0);
    // its loop ends: the end section runs from where the loop was to 2 s, and the end child comes at 1.4 s
    world.stop(id, false);
    let mut seen = false;
    for _ in 0..120 {
        world.step(DT);
        seen |= emitters_named(&world, "end_child") > 0;
    }
    assert!(seen, "the end child was generated after the loop ended");
}

#[test]
fn the_chandelier_ice_flowers_break_into_their_pieces() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let candidates = [std::path::PathBuf::from(std::env::var("KH_XWE_ZMN0").unwrap_or_default()), repo.join("build/sora_tmp/er/xwd/pack/xsi/vfx/effects_xwd/vw_so088_fin_zmn0.json")];
    let Some(text) = candidates.iter().find_map(|p| std::fs::read_to_string(p).ok()) else {
        eprintln!("no vw_so088_fin_zmn0.json of XW-d: skipped");
        return;
    };
    let mut world = World::new();
    world.register(EffectDef::parse(&text).unwrap());
    let id = world.spawn("vw_so088_fin_zmn0", Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
    let pieces = |world: &World| {
        let mut list = DrawList::default();
        world.draw(1.0, &Camera::DEFAULT, &mut list);
        list.meshes.iter().filter(|m| world.mesh_name(m.mesh).is_some_and(|n| n.contains("_br0"))).count()
    };
    let mut before = 0;
    for _ in 0..100 {
        world.step(DT);
        before = before.max(pieces(&world));
    }
    assert_eq!(before, 0, "no pieces while the flowers stand");
    world.stop(id, false);
    let mut after = 0;
    for _ in 0..200 {
        world.step(DT);
        after = after.max(pieces(&world));
    }
    assert!(after > 0, "the flowers break into their pieces once stopped");
}
