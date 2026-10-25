//! (XW-d) `TypeDataSQEX_FragmentMesh`: the pieces of a fragment mesh particle are drawn at their offsets, never its
//! placeholder mesh; from ImpactTime on they fly apart and fall. Plus the real Chandelier shatter when the pack is on disk.

use glam::Vec3;
use kh_vfx::{Camera, DrawList, EffectDef, Kind, SpawnOptions, World, Xform};
use serde_json::{json, Value};

const DT: f32 = 1.0 / 60.0;

fn constant(v: Value) -> Value {
    json!({"distribution": {"type": "constant", "constant": v}})
}

fn piece(mesh: &str, x: f32, y: f32, z: f32) -> Value {
    json!({"SubMesh": format!("/Game/Effects/test/mdl/{mesh}.{mesh}"), "Offset": {"Rotation": {"x": 0, "y": 0, "z": 0, "w": 1}, "Translation": {"x": x, "y": y, "z": z}, "Scale3D": {"x": 1, "y": 1, "z": 1}}})
}

fn fragment_effect() -> EffectDef {
    let typedata = json!({
        "kind": "sqex_fragmentmesh", "mesh": "/Game/Effects/trescom/mdl/tmd_xyz_00.tmd_xyz_00", "mesh_file": "meshes/tmd_xyz_00.glb",
        "params": {
            "Mesh": "/Game/Effects/trescom/mdl/tmd_xyz_00.tmd_xyz_00", "bUseSubMeshs": true, "ImpactTime": 0.1, "ImpactPower": 300, "RandomImpactPower": 0,
            "ImpactLocationOffset": {"x": 0, "y": 0, "z": 0}, "Gravity": -2000, "Force": {"x": 0, "y": 0, "z": 0},
            "RandomImpactRotation": constant(json!([90, 0, 0])), "LocationOffsetScale": constant(json!(1)), "SubMeshsSetIndex": constant(json!(0)),
            "SubMeshLists": [{"SubMeshList": [piece("p0", 100.0, 0.0, 0.0), piece("p1", 0.0, 100.0, 0.0), piece("p2", 0.0, 0.0, 100.0)]}]
        },
        "sub_mesh_files": [["meshes/p0.glb", "meshes/p1.glb", null]]
    });
    let emitter = json!({
        "name": "frag", "enabled": true, "typedata": typedata,
        "required": {
            "material": "/Game/Effects/test/mat/m_test.m_test", "screen_alignment": "PSA_Square", "use_local_space": false, "kill_on_completed": true, "duration": 2.0, "loops": 1,
            "sqex_emitter_time": {"start_time": 0.0, "loop_time": 0.0, "loop_duration": 2.0, "duration_time": 2.0, "use_legacy_emitter_time": false},
            "sub_uv": {"horizontal": 1, "vertical": 1, "interpolation": "PSUVIM_None"}, "use_max_draw_count": true, "max_draw_count": 500,
            "emitter_origin": {"x": 0, "y": 0, "z": 0}, "emitter_rotation": {"pitch": 0, "yaw": 0, "roll": 0},
            "params": {"bAutoAddToComponent": true, "RotationPriority": "ROTATION_PRIORITY_XZY"}
        },
        "spawn": {"enabled": true, "process_spawn_rate": true, "process_burst_list": true, "rate": constant(json!(0.0)), "rate_scale": constant(json!(1.0)), "burst_scale": constant(json!(1.0)), "bursts": [{"count": 1, "count_low": -1, "time": 0.0}]},
        "modules": [{"class": "ParticleModuleLifetime", "enabled": true, "params": {"Lifetime": constant(json!(1.0))}}, {"class": "ParticleModuleSize", "enabled": true, "params": {"StartSize": constant(json!([1.0, 1.0, 1.0]))}}]
    });
    EffectDef::from_json(&json!({"name": "frag_test", "system": {"params": {}}, "emitters": [emitter]})).unwrap()
}

fn drawn(world: &World) -> DrawList {
    let mut list = DrawList::default();
    world.draw(1.0, &Camera::DEFAULT, &mut list);
    list
}

#[test]
fn fragment_pieces_sit_then_fly_apart_and_fall() {
    let def = fragment_effect();
    let e = &def.emitters[0];
    assert_eq!(e.kind, Kind::Fragment);
    let f = e.fragments.as_ref().expect("the pieces");
    assert_eq!(f.pieces.len(), 3);
    assert_eq!(e.sub_meshes.as_ref().unwrap().list, vec!["p0".to_string(), "p1".to_string()]);
    assert!(e.unknown.iter().any(|u| u.contains("1 of 3 fragment meshes")), "{:?}", e.unknown);
    let mut world = World::new();
    world.register(def);
    let origin = Vec3::new(500.0, 0.0, 0.0);
    world.spawn("frag_test", Xform::at(origin), SpawnOptions::default()).unwrap();
    world.step(DT);
    // before the impact: the two pieces the pack has, at their offsets; never the placeholder axis mesh
    let list = drawn(&world);
    assert_eq!(list.meshes.len(), 2);
    let names: Vec<_> = list.meshes.iter().map(|m| world.mesh_name(m.mesh).unwrap().to_string()).collect();
    assert_eq!(names, ["p0", "p1"]);
    assert!((Vec3::from(list.meshes[0].transform.translation) - (origin + Vec3::new(100.0, 0.0, 0.0))).length() < 1e-3);
    assert!((Vec3::from(list.meshes[1].transform.translation) - (origin + Vec3::new(0.0, 100.0, 0.0))).length() < 1e-3);
    // 29 steps on (0.38 s of flight): p0 flew along +X at 300 cm/s, fell under -2000 cm/s^2, and turned
    for _ in 0..29 {
        world.step(DT);
    }
    let list = drawn(&world);
    let t = 29.0 * DT - 0.1; // (born at the first step with age 0)
    let p0 = Vec3::from(list.meshes[0].transform.translation);
    let want = origin + Vec3::new(100.0 + 300.0 * t, 0.0, -1000.0 * t * t);
    assert!((p0 - want).length() < 1.0, "{p0} vs {want}");
    let p1 = Vec3::from(list.meshes[1].transform.translation);
    assert!(p1.y > 100.0 + 300.0 * t - 1.0 && p1.z < -1000.0 * t * t + 1.0);
    let y_axis = Vec3::from(list.meshes[0].transform.matrix3.y_axis); // (a roll of 90 deg/s: about X)
    assert!((y_axis - Vec3::Y).length() > 0.1, "the piece spins");
    // the particle's life ends: nothing left
    for _ in 0..40 {
        world.step(DT);
    }
    assert!(drawn(&world).meshes.is_empty());
}

#[test]
fn the_chandelier_shatter_draws_its_pieces() {
    // the XS root's effect (KH_XWD_HAHEN = its file, e.g. build/sora_tmp/er/xwc/pack/xsi/vfx/effects/vw_so088_fin_hahen00.json,
    // else the XW-c pack beside the repo); skipped when it is not on disk
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let candidates = [std::path::PathBuf::from(std::env::var("KH_XWD_HAHEN").unwrap_or_default()), repo.join("build/sora_tmp/er/xwc/pack/xsi/vfx/effects/vw_so088_fin_hahen00.json")];
    let Some(text) = candidates.iter().find_map(|p| std::fs::read_to_string(p).ok()) else {
        eprintln!("no vw_so088_fin_hahen00.json: skipped");
        return;
    };
    let def = EffectDef::parse(&text).unwrap();
    let mut world = World::new();
    world.register(def);
    world.spawn("vw_so088_fin_hahen00", Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
    for _ in 0..3 {
        world.step(DT);
    }
    let list = drawn(&world);
    let pieces = list.meshes.iter().filter(|m| world.mesh_name(m.mesh).is_some_and(|n| n.starts_with("kmd_so088_a"))).count();
    // (its parent Control_lp is a ring of order ALL: one shanderia_a on each vertex, each with its seven pieces)
    assert!(pieces > 0 && pieces % 7 == 0, "{pieces}: the seven pieces of each shanderia_a");
    assert!(list.meshes.iter().all(|m| world.mesh_name(m.mesh) != Some("tmd_xyz_00")));
}
