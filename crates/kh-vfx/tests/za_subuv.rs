//! (ZA) Sub-UV images that the renderer used to get wrong:
//! - a SubUV material on a MESH particle (Firaza's fireball body, vm_fir_blt3 FiGan_PartsA / B, tmi_fir_8x8_00sa):
//!   UE's FMeshParticleVertexFactory squeezes the mesh's UV into the particle's image of the emitter's grid
//!   (SubUV0AndTexCoord0 / SubUV1AndLerp). The hosts had no image for a mesh, so the whole 8 x 8 atlas was spread over
//!   the fireball (a spray of 64 little flames). `MeshInstance::sub` now carries it (`sub_image_code`);
//! - `SubUVMovie` loops round the grid (UE 4.17) and starts on `StartingFrame` (0 = a random image); it was clamped on
//!   the last image (Blizzard's burst clouds froze after 2.1 s).

use glam::Vec3;
use kh_vfx::{sub_image_code, Camera, DrawList, EffectDef, SpawnOptions, World, Xform};
use serde_json::{json, Value};

const DT: f32 = 1.0 / 60.0;

/// The hosts' WGSL decode (`kh_mesh_subuv`), mirrored: (image, h, v, lerp).
fn decode(code: f32) -> (u32, u32, u32, f32) {
    let c = (-code).round() as u32 - 1;
    (c >> 7 & 255, (c >> 15 & 15) + 1, (c >> 19 & 15) + 1, (c & 127) as f32 / 127.0)
}

#[test]
fn the_image_code_round_trips() {
    assert_eq!(sub_image_code(5.0, 1, 1, false), 0.0, "one image: no code");
    for (image, h, v, blend) in [(0.0, 8, 8, false), (63.9, 8, 8, false), (26.4, 8, 4, true), (200.0, 16, 16, true), (3.0, 4, 1, false)] {
        let code = sub_image_code(image, h, v, blend);
        assert!(code < -0.5, "{code}");
        let (i, hh, vv, lerp) = decode(code);
        let want = (image as f32).clamp(0.0, (h * v - 1) as f32);
        assert_eq!((i, hh, vv), (want.floor() as u32, h, v), "{image} {h}x{v}");
        let want_lerp = if blend { want - want.floor() } else { 0.0 };
        assert!((lerp - want_lerp).abs() < 0.01, "{lerp} vs {want_lerp}");
        // exact in an f32 even after an interpolation's rounding error
        assert_eq!(decode(code + 0.3).0, i);
    }
}

fn constant(v: Value) -> Value {
    json!({"distribution": {"type": "constant", "constant": v}})
}

fn movie(start: i32) -> EffectDef {
    let em = json!({
        "name": "movie", "enabled": true, "typedata": {"kind": "sprite"},
        "required": {
            "material": "/Game/Effects/test/mat/m_test.m_test", "screen_alignment": "PSA_Square", "use_local_space": false,
            "kill_on_completed": false, "duration": 10.0, "loops": 1,
            "sqex_emitter_time": {"start_time": 0.0, "loop_time": 0.0, "loop_duration": 10.0, "duration_time": 10.0, "use_legacy_emitter_time": false},
            "sub_uv": {"horizontal": 8, "vertical": 8, "interpolation": "PSUVIM_Linear"},
            "use_max_draw_count": true, "max_draw_count": 500,
            "emitter_origin": {"x": 0, "y": 0, "z": 0}, "emitter_rotation": {"pitch": 0, "yaw": 0, "roll": 0},
            "params": {"bAutoAddToComponent": true}
        },
        "spawn": {"enabled": true, "process_spawn_rate": true, "process_burst_list": true, "rate": constant(json!(0.0)), "rate_scale": constant(json!(1.0)), "burst_scale": constant(json!(1.0)), "bursts": [{"count": 1, "count_low": -1, "time": 0.0}]},
        "modules": [
            {"class": "ParticleModuleLifetime", "enabled": true, "params": {"Lifetime": constant(json!(8.0))}},
            {"class": "ParticleModuleSize", "enabled": true, "params": {"StartSize": constant(json!({"x": 10, "y": 10, "z": 10}))}},
            {"class": "ParticleModuleSubUVMovie", "enabled": true, "params": {"FrameRate": constant(json!(30.0)), "StartingFrame": start}}
        ]
    });
    EffectDef::from_json(&json!({"name": format!("movie{start}"), "system": {"params": {}}, "emitters": [em]})).unwrap()
}

/// The image a quad shows (its UV rect's corner on the 8 x 8 grid).
fn image_of(list: &DrawList) -> u32 {
    let q = list.quads.first().expect("a quad");
    let (u, v) = (q.uv[0].min(q.uv[2]), q.uv[1].min(q.uv[3]));
    (v * 8.0).round() as u32 * 8 + (u * 8.0).round() as u32
}

#[test]
fn a_movie_loops_round_its_grid_and_starts_where_it_says() {
    let mut world = World::new();
    world.register(movie(1));
    world.register(movie(0));
    let a = world.spawn("movie1", Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
    let mut list = DrawList::default();
    for _ in 0..180 {
        world.step(DT);
    }
    list.clear();
    world.draw(1.0, &Camera::DEFAULT, &mut list);
    // 3 s at 30 images a second = image 90 = 26 of the second pass (it was held on 63)
    let got = image_of(&list);
    assert!((25..=27).contains(&got), "image {got}");
    world.stop(a, true);
    // StartingFrame 0: a random start, so two spawns rarely agree at their first step
    let starts: std::collections::BTreeSet<u32> = (0..6)
        .map(|k| {
            let id = world.spawn("movie0", Xform::at(Vec3::ZERO), SpawnOptions { seed: Some(1000 + k), ..SpawnOptions::default() }).unwrap();
            world.step(DT);
            list.clear();
            world.draw(1.0, &Camera::DEFAULT, &mut list);
            let i = image_of(&list);
            world.stop(id, true);
            world.step(DT);
            i
        })
        .collect();
    assert!(starts.len() > 1, "random starts {starts:?}");
}

/// Firaza's fireball on the pack: its two flame shells are mesh particles of an 8 x 8 SubUV material, and every one of
/// their instances now names its image (before: none, so the whole atlas was drawn).
#[test]
fn firazas_flame_shells_name_their_image() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/vfx/effects");
    let Ok(text) = std::fs::read_to_string(dir.join("vm_fir_blt3.json")) else { return };
    let mut world = World::new();
    world.register(EffectDef::parse(&text).unwrap());
    world.spawn("vm_fir_blt3", Xform::at(Vec3::new(0.0, 0.0, 100.0)), SpawnOptions::default()).unwrap();
    for _ in 0..12 {
        world.step(DT);
    }
    let mut list = DrawList::default();
    world.draw(1.0, &Camera::DEFAULT, &mut list);
    let shells: Vec<_> = list.meshes.iter().filter(|m| world.material_name(m.material) == Some("tmi_fir_8x8_00sa")).collect();
    assert_eq!(shells.len(), 2, "FiGan_PartsA / B");
    for m in shells {
        let (image, h, v, lerp) = decode(m.sub);
        assert_eq!((h, v, lerp), (8, 8, 0.0));
        // SubUVMovie at 45 a second from the first image: 0.2 s -> image 9
        assert!((8..=10).contains(&image), "image {image}");
    }
    // the other meshes have one image: no code
    assert!(list.meshes.iter().filter(|m| world.material_name(m.material) != Some("tmi_fir_8x8_00sa")).all(|m| m.sub == 0.0));
}
