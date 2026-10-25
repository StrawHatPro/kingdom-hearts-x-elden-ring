//! Tests of the second pass (fidelity): tails by path length, tail sprites, sprite rectangles,
//! the pack's own file names, sub meshes, nested systems, the ground ray, lights, decals, the
//! emitter render mode.

use glam::Vec3;
use kh_vfx::{Blend, Camera, DrawList, EffectDef, Kind, SpawnOptions, World, Xform};
use serde_json::{json, Value};

const DT: f32 = 1.0 / 60.0;

fn constant(v: Value) -> Value {
    json!({"distribution": {"type": "constant", "constant": v}})
}

fn module(class: &str, params: Value) -> Value {
    json!({"class": class, "enabled": true, "params": params})
}

/// One emitter in the pack's shape: endless (loops 0), one particle at time 0 unless `burst` is 0.
fn emitter(name: &str, kind: &str, burst: u32, modules: Vec<Value>, typedata: Value, required: Value) -> Value {
    let mut req = json!({
        "material": "/Game/Effects/test/mat/m_test.m_test", "screen_alignment": "PSA_Square", "use_local_space": false,
        "kill_on_completed": false, "duration": 10.0, "loops": 0,
        "sqex_emitter_time": {"start_time": 0.0, "loop_time": 0.0, "loop_duration": 10.0, "duration_time": 10.0, "use_legacy_emitter_time": false},
        "sub_uv": {"horizontal": 1, "vertical": 1, "interpolation": "PSUVIM_None"},
        "use_max_draw_count": true, "max_draw_count": 500,
        "emitter_origin": {"x": 0, "y": 0, "z": 0}, "emitter_rotation": {"pitch": 0, "yaw": 0, "roll": 0},
        "params": {"bAutoAddToComponent": true, "RotationPriority": "ROTATION_PRIORITY_XZY"}
    });
    for (k, v) in required.as_object().into_iter().flatten() {
        if k == "params" {
            for (pk, pv) in v.as_object().into_iter().flatten() {
                req["params"][pk] = pv.clone();
            }
        } else {
            req[k] = v.clone();
        }
    }
    let mut td = json!({"kind": kind, "params": {}});
    for (k, v) in typedata.as_object().into_iter().flatten() {
        td[k] = v.clone();
    }
    json!({
        "name": name, "enabled": true, "render_mode": "ERM_Normal", "typedata": td, "required": req,
        "spawn": {"enabled": true, "process_spawn_rate": true, "process_burst_list": true, "rate": constant(json!(0.0)), "rate_scale": constant(json!(1.0)), "burst_scale": constant(json!(1.0)), "bursts": if burst > 0 { json!([{"count": burst, "count_low": -1, "time": 0.0}]) } else { json!([]) }},
        "modules": modules
    })
}

fn effect(name: &str, emitters: Vec<Value>) -> EffectDef {
    EffectDef::from_json(&json!({"name": name, "system": {"params": {"SQEXBaseColor": {"r": 1, "g": 1, "b": 1, "a": 1}}}, "emitters": emitters})).unwrap()
}

fn draw(world: &World) -> DrawList {
    let mut list = DrawList::default();
    world.draw(1.0, &Camera { position: Vec3::new(0.0, -1000.0, 0.0), forward: Vec3::Y, right: Vec3::X, up: Vec3::Z }, &mut list);
    list
}

fn tail_modules() -> Vec<Value> {
    vec![
        module("ParticleModuleSize", json!({"StartSize": constant(json!([20.0, 0.0, 0.0]))})),
        module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(0.0))})),
        module(
            "ParticleModuleSQEX_TailShape",
            json!({"TailNumber": constant(json!(5.0)), "TailInterval": constant(json!(50.0)), "TailScale": json!({"distribution": {"type": "constant_curve", "lookup_table": {"op": 1, "entry_count": 2, "entry_stride": 1, "sub_entry_stride": 0, "time_scale": 1.0, "time_bias": 0.0, "lock_flag": 0, "values": [1.0, 0.0]}}})}),
        ),
    ]
}

/// The tail's points are a fixed distance apart along the path the node has flown (0x1442A7650 /
/// 0x144287CC0 [C]), whatever its speed, and there are at most TailNumber of them; its width
/// follows TailScale along the tail (0x144296090 [C]).
#[test]
fn a_tail_has_points_every_interval_along_the_path() {
    for speed in [600.0f32, 3000.0] {
        let mut world = World::new();
        world.register(effect("tail", vec![emitter("t", "sqex_tail_polygon", 0, tail_modules(), json!({}), json!({}))]));
        let id = world.spawn("tail", Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
        for step in 1..=60 {
            world.set_transform(id, Xform::at(Vec3::X * speed * DT * step as f32));
            world.step(DT);
        }
        let list = draw(&world);
        assert_eq!(list.ribbons.len(), 1, "speed {speed}");
        let points = &list.ribbons[0].points;
        assert_eq!(points.len(), 5, "TailNumber points at speed {speed}");
        for pair in points.windows(2) {
            let (a, b) = ((pair[0].a + pair[0].b) * 0.5, (pair[1].a + pair[1].b) * 0.5);
            assert!((a.distance(b) - 50.0).abs() < 0.5, "points 50 cm apart at speed {speed}: {}", a.distance(b));
        }
        // Head full width (20 cm), end of the tail none.
        assert!((points[0].a.distance(points[0].b) - 20.0).abs() < 0.1);
        assert!(points[4].a.distance(points[4].b) < 0.1);
        assert_eq!(points[0].u, 0.0);
        assert!((points[4].u - 1.0).abs() < 1.0e-5);
    }
}

/// A tail that has not flown far yet is shorter: no point lies beyond the recorded path.
#[test]
fn a_young_tail_is_short() {
    let mut world = World::new();
    world.register(effect("tail", vec![emitter("t", "sqex_tail_polygon", 0, tail_modules(), json!({}), json!({}))]));
    let id = world.spawn("tail", Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
    for step in 1..=6 {
        world.set_transform(id, Xform::at(Vec3::X * 20.0 * step as f32));
        world.step(DT);
    }
    // 100 cm flown (the first step records the start): the head and two points.
    let list = draw(&world);
    assert_eq!(list.ribbons[0].points.len(), 3);
}

/// `TypeDataSQEX_TailShape`: the same points, one sprite on each.
#[test]
fn a_tail_shape_draws_a_sprite_on_every_point() {
    let mut world = World::new();
    world.register(effect("tail", vec![emitter("t", "sqex_tail_shape", 0, tail_modules(), json!({}), json!({}))]));
    assert_eq!(world.def("tail").unwrap().emitters[0].kind, Kind::TailSprites);
    let id = world.spawn("tail", Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
    for step in 1..=60 {
        world.set_transform(id, Xform::at(Vec3::X * 10.0 * step as f32));
        world.step(DT);
    }
    let list = draw(&world);
    assert!(list.ribbons.is_empty());
    // The fifth point has TailScale 0: four sprites, 50 cm apart, shrinking.
    assert_eq!(list.quads.len(), 4);
    assert!((list.quads[0].center.distance(list.quads[1].center) - 50.0).abs() < 0.5);
    assert!(list.quads[0].size > list.quads[1].size);
}

/// Stock sprite size: only PSA_Square (and FacingCameraPosition) copy X into Y. A
/// PSA_TypeSpecific sprite of (8000, 60) is a streak, not an 80 m square.
#[test]
fn a_type_specific_sprite_is_x_by_y() {
    let size = module("ParticleModuleSize", json!({"StartSize": constant(json!([8000.0, 60.0, 1.0]))}));
    let life = module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(5.0))}));
    for (alignment, height) in [("PSA_TypeSpecific", 60.0f32), ("PSA_Square", 8000.0), ("PSA_Rectangle", 60.0)] {
        let mut world = World::new();
        world.register(effect("s", vec![emitter("e", "sprite", 1, vec![size.clone(), life.clone()], json!({}), json!({"screen_alignment": alignment}))]));
        world.spawn("s", Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
        world.step(DT);
        let list = draw(&world);
        let q = &list.quads[0];
        assert!((q.corners[0].distance(q.corners[1]) - 8000.0).abs() < 0.5, "{alignment}");
        assert!((q.corners[1].distance(q.corners[2]) - height).abs() < 0.5, "{alignment}: {}", q.corners[1].distance(q.corners[2]));
    }
}

/// Two assets of the pack may share a base name (`tmi_ora_01fsla` is a water material and a fire
/// material); the effect names the pack's FILE, and that is what the evaluator hands the host.
#[test]
fn materials_and_meshes_are_named_by_their_pack_file() {
    let def = effect(
        "m",
        vec![emitter(
            "e",
            "mesh",
            1,
            vec![],
            json!({"mesh": "/Game/Effects/a/mdl/ring.ring", "mesh_file": "meshes/ring__Effects_b_mdl.glb"}),
            json!({"material": "/Game/Effects/trescom/mat_mdl/tmi_ora_01fsla.tmi_ora_01fsla", "material_file": "materials/tmi_ora_01fsla__Effects_trescom_mat_mdl.json"}),
        )],
    );
    assert_eq!(def.emitters[0].material, "tmi_ora_01fsla__Effects_trescom_mat_mdl");
    assert_eq!(def.emitters[0].mesh.as_deref(), Some("ring__Effects_b_mdl"));
    // Without the file fields the base name of the asset path still works.
    let def = effect("m", vec![emitter("e", "mesh", 1, vec![], json!({"mesh": "/Game/Effects/a/mdl/ring.ring"}), json!({}))]);
    assert_eq!(def.emitters[0].material, "m_test");
    assert_eq!(def.emitters[0].mesh.as_deref(), Some("ring"));
}

/// `bUseSubMeshs`: the particles draw the listed meshes, not the placeholder `Mesh`.
#[test]
fn sub_meshes_replace_the_placeholder_mesh() {
    let life = module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(5.0))}));
    let size = module("ParticleModuleSize", json!({"StartSize": constant(json!([1.0, 1.0, 1.0]))}));
    let typedata = json!({
        "mesh": "/Game/Effects/trescom/mdl/tmd_xyz_00.tmd_xyz_00", "mesh_file": "meshes/tmd_xyz_00.glb",
        "sub_mesh_files": [["meshes/rock_a.glb", "meshes/rock_b.glb", "meshes/rock_c.glb"]],
        "params": {"bUseSubMeshs": true, "bUseRandomStart": true, "MeshIndexIntervalTime": 0.0, "SubMeshsSetIndex": constant(json!(0.0)), "SubMeshsIndex": constant(json!(0.0))}
    });
    let mut world = World::new();
    world.register(effect("rocks", vec![emitter("e", "mesh", 40, vec![life, size], typedata, json!({}))]));
    world.spawn("rocks", Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
    world.step(DT);
    let list = draw(&world);
    assert_eq!(list.meshes.len(), 40);
    let names: std::collections::BTreeSet<&str> = list.meshes.iter().map(|m| world.mesh_name(m.mesh).unwrap()).collect();
    assert!(!names.contains("tmd_xyz_00"), "the placeholder is never drawn: {names:?}");
    assert_eq!(names.len(), 3, "a random start picks among all three: {names:?}");
    // A particle keeps its mesh (interval 0).
    let before: Vec<u32> = list.meshes.iter().map(|m| m.mesh).collect();
    for _ in 0..30 {
        world.step(DT);
    }
    assert_eq!(before, draw(&world).meshes.iter().map(|m| m.mesh).collect::<Vec<_>>());
}

/// `SQEX_SpawnParticleSystem` (0x1442FE100 [C]): past SpawnTime the emitter spawns the named
/// system at its node, which then follows the node; it is told to end when its spawner is.
#[test]
fn an_emitter_spawns_a_nested_system_that_follows_its_node() {
    let life = module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(100.0))}));
    let size = module("ParticleModuleSize", json!({"StartSize": constant(json!([10.0, 10.0, 10.0]))}));
    let attach = module("ParticleModuleSQEX_AttachParticle", json!({"bIsAttachPosition": true, "bIsAttachRotation": false}));
    let spawner = module("ParticleModuleSQEX_SpawnParticleSystem", json!({"ParticleSystemAsset": "/Game/Effects/test/par/inner.inner", "SpawnTime": 0.1, "LoopEndTime": 0.0, "bGiveLocation": true, "bGiveRotation": true, "bGiveScale": true, "bGiveColor": true, "bGiveAlpha": true}));
    let outer = effect("outer", vec![emitter("ctl", "sqex_control", 0, vec![spawner], json!({}), json!({"emitter_origin": {"x": 0, "y": 0, "z": 50}}))]);
    assert_eq!(outer.nested_assets(), vec!["inner".to_string()]);
    let mut world = World::new();
    world.register(outer);
    world.register(effect("inner", vec![emitter("s", "sprite", 1, vec![life, size, attach], json!({}), json!({}))]));
    let id = world.spawn("outer", Xform::at(Vec3::new(100.0, 0.0, 0.0)), SpawnOptions::default()).unwrap();
    for _ in 0..5 {
        world.step(DT);
    }
    assert!(draw(&world).quads.is_empty(), "nothing before SpawnTime");
    for _ in 0..10 {
        world.step(DT);
    }
    let list = draw(&world);
    assert_eq!(list.quads.len(), 1);
    assert!(list.quads[0].center.distance(Vec3::new(100.0, 0.0, 50.0)) < 0.01, "at the spawner's node: {:?}", list.quads[0].center);
    // It follows the node.
    world.set_transform(id, Xform::at(Vec3::new(300.0, 0.0, 0.0)));
    world.step(DT);
    world.step(DT);
    assert!(draw(&world).quads[0].center.distance(Vec3::new(300.0, 0.0, 50.0)) < 0.01);
    // Destroying the spawner ends the nested system (its endless emitter gets loop end and runs out).
    world.stop(id, true);
    for _ in 0..3 {
        world.step(DT);
    }
    assert_eq!(world.stats().0, 1, "only the nested system is left");
}

/// `SQEX_Raycast` (modelled [?]): the node goes to where its ray meets the ground the host gave.
#[test]
fn a_ray_puts_the_node_on_the_ground() {
    let life = module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(5.0))}));
    let size = module("ParticleModuleSize", json!({"StartSize": constant(json!([10.0, 10.0, 10.0]))}));
    let ray = |theme: &str| module("ParticleModuleSQEX_Raycast", json!({"RayEndOffset": {"x": 0, "y": 0, "z": -500}, "SpawnLocationOffset": {"x": 0, "y": 0, "z": 5}, "bInWorldSpace": false, "SpawnTheme": theme}));
    for (theme, ground, expect) in [("ESQEX_RAYCAST_SPAWN_THEME_ONHIT", Some(0.0f32), Some(5.0f32)), ("ESQEX_RAYCAST_SPAWN_THEME_ONHIT", None, None), ("ESQEX_RAYCAST_SPAWN_THEME_FORCE_SPAWN", None, Some(200.0)), ("ESQEX_RAYCAST_SPAWN_THEME_ONHIT", Some(-900.0), None)] {
        let mut world = World::new();
        world.set_ground(ground);
        world.register(effect("r", vec![emitter("e", "sprite", 1, vec![life.clone(), size.clone(), ray(theme)], json!({}), json!({}))]));
        world.spawn("r", Xform::at(Vec3::new(10.0, 20.0, 200.0)), SpawnOptions::default()).unwrap();
        world.step(DT);
        let list = draw(&world);
        match expect {
            Some(z) => assert!((list.quads[0].center - Vec3::new(10.0, 20.0, z)).length() < 0.01, "{theme} {ground:?}: {:?}", list.quads[0].center),
            None => assert!(list.quads.is_empty(), "{theme} {ground:?}"),
        }
    }
}

/// `ParticleModuleLight` and `EmitterRenderMode` ERM_LightsOnly: the light is handed to the host,
/// the sprite is not drawn.
#[test]
fn a_lights_only_emitter_gives_a_light_and_no_sprite() {
    let life = module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(5.0))}));
    let size = module("ParticleModuleSize", json!({"StartSize": constant(json!([100.0, 100.0, 100.0]))}));
    let light = module("ParticleModuleLight", json!({"ColorScaleOverLife": constant(json!([1.0, 0.5, 0.25])), "BrightnessOverLife": constant(json!(8.0)), "RadiusScale": constant(json!(6.0))}));
    let mut e = emitter("l", "sprite", 1, vec![life, size, light], json!({}), json!({}));
    e["render_mode"] = json!("ERM_LightsOnly");
    let mut world = World::new();
    world.register(effect("light", vec![e]));
    world.spawn("light", Xform::at(Vec3::new(0.0, 0.0, 100.0)), SpawnOptions::default()).unwrap();
    world.step(DT);
    let list = draw(&world);
    assert!(list.quads.is_empty());
    assert_eq!(list.lights.len(), 1);
    assert!((list.lights[0].radius - 600.0).abs() < 0.01);
    assert!((list.lights[0].color - Vec3::new(8.0, 4.0, 2.0)).length() < 1.0e-4);
    assert!(list.lights[0].position.distance(Vec3::new(0.0, 0.0, 100.0)) < 0.01);
}

/// `TypeDataSQEX_Decal`: a box of DecalSize to either side of the node, turned by DecalRotation
/// ((0, -90, 0) turns its projection axis X down), with the colour and opacity curves.
#[test]
fn a_decal_emitter_gives_its_box() {
    let typedata = json!({"params": {"DecalMaterial": "/Game/Effects/test/mat/d_test.d_test", "DecalSize": constant(json!([256.0, 700.0, 700.0])), "DecalRotation": constant(json!([0.0, -90.0, 0.0])), "DecalColor": constant(json!([2.0, 1.0, 0.5])), "DecalOpacity": constant(json!(0.5)), "ZFadePow": 2.0, "ZFadeSegment": 1.0}});
    let mut world = World::new();
    world.register(effect("d", vec![emitter("e", "sqex_decal", 0, vec![], typedata, json!({}))]));
    world.spawn("d", Xform::at(Vec3::new(0.0, 0.0, 10.0)), SpawnOptions::default()).unwrap();
    world.step(DT);
    let list = draw(&world);
    assert_eq!(list.decals.len(), 1);
    let d = &list.decals[0];
    assert_eq!(world.material_name(d.material), Some("d_test"));
    assert_eq!(d.color, [2.0, 1.0, 0.5, 0.5]);
    assert_eq!(d.z_fade, [2.0, 1.0]);
    // Local X (the projection axis, half depth 256) points down or up the world's Z; Y and Z span 700.
    let x = d.transform.transform_vector3(Vec3::X);
    assert!((x.length() - 256.0).abs() < 0.01 && x.z.abs() > 255.9, "{x:?}");
    assert!((d.transform.transform_vector3(Vec3::Y).length() - 700.0).abs() < 0.01);
    assert!(Vec3::from(d.transform.translation).distance(Vec3::new(0.0, 0.0, 10.0)) < 0.01);
}

/// `EmitterBlendType` reaches the host with every draw item.
#[test]
fn the_emitter_blend_type_is_passed_on() {
    let life = module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(5.0))}));
    let size = module("ParticleModuleSize", json!({"StartSize": constant(json!([10.0, 10.0, 10.0]))}));
    for (name, expect) in [("BLEND_Additive", Blend::Additive), ("BLEND_Translucent", Blend::Translucent), ("BLEND_Opaque", Blend::Material)] {
        let mut world = World::new();
        world.register(effect("b", vec![emitter("e", "sprite", 1, vec![life.clone(), size.clone()], json!({}), json!({"params": {"EmitterBlendType": name}}))]));
        world.spawn("b", Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
        world.step(DT);
        assert_eq!(draw(&world).quads[0].mode, expect);
    }
}

/// (fourth pass) A particle without a lifetime is the emitter's own body: it goes when the
/// emitter's timeline has ended (`bKillOnCompleted`), and the system with it; a looping emitter
/// keeps it until loop end is asked for.
#[test]
fn a_particle_without_a_lifetime_ends_with_its_emitter() {
    let timed = json!({"kill_on_completed": true, "loops": 1, "duration": 0.5, "sqex_emitter_time": {"start_time": 0.0, "loop_time": 0.5, "loop_duration": 0.5, "duration_time": 0.5, "use_legacy_emitter_time": false}});
    let looping = json!({"kill_on_completed": true, "loops": 0, "duration": 0.5, "sqex_emitter_time": {"start_time": 0.0, "loop_time": 0.2, "loop_duration": 0.4, "duration_time": 0.5, "use_legacy_emitter_time": false}});
    let mut world = World::new();
    world.register(effect("once", vec![emitter("body", "sprite", 1, vec![], json!({}), timed)]));
    world.register(effect("loop", vec![emitter("body", "sprite", 1, vec![], json!({}), looping)]));
    let once = world.spawn("once", Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
    let looped = world.spawn("loop", Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
    for _ in 0..20 {
        world.step(DT);
    }
    assert_eq!(world.live().iter().map(|l| l.emitters.iter().map(|e| e.3).sum::<usize>()).sum::<usize>(), 2, "both bodies live while their emitters run");
    for _ in 0..40 {
        world.step(DT);
    }
    assert!(!world.is_alive(once), "the one-shot's body went with its timeline");
    assert!(world.is_alive(looped), "the looping one stays until loop end");
    world.stop(looped, false);
    for _ in 0..40 {
        world.step(DT);
    }
    assert!(!world.is_alive(looped), "loop end: the ending plays and the body goes");
    assert!(world.live().is_empty());
}

/// (fourth pass) A Z camera billboard turns about the COMPONENT's Z axis even when the emitter
/// itself is rolled 90 degrees (Thunder's bolt strip: upright in the game, capture of 2026-10-12),
/// about the node's rotated axis only with `bUseSQEXBillboradRotaionAxis`; and
/// `SQEXBillboardRotaionOffset` turns the mesh inside the billboard frame.
#[test]
fn an_axis_billboard_keeps_the_components_axis() {
    let life = || module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(100.0))}));
    let size = || module("ParticleModuleSize", json!({"StartSize": constant(json!([1.0, 1.0, 1.0]))}));
    let rolled = json!({"emitter_rotation": {"pitch": 0, "yaw": 0, "roll": 90}, "use_local_space": true});
    let td = |axis: bool, pitch: f32| json!({"mesh": "/Game/Effects/test/mdl/strip.strip", "params": {"SQEXBillboardType": "ESQEX_MESH_PAR_BILLBORAD_TYPE_Z_CAMERA_BILLBOARD", "bUseSQEXBillboradRotaionAxis": axis, "SQEXBillboardRotaionOffset": {"pitch": pitch, "yaw": 0, "roll": 0}}});
    let mut world = World::new();
    world.register(effect("bolt", vec![emitter("strip", "mesh", 1, vec![life(), size()], td(false, 0.0), rolled.clone())]));
    world.register(effect("axis", vec![emitter("strip", "mesh", 1, vec![life(), size()], td(true, 0.0), rolled.clone())]));
    world.register(effect("spike", vec![emitter("strip", "mesh", 1, vec![life(), size()], td(false, 90.0), json!({}))]));
    for name in ["bolt", "axis", "spike"] {
        world.spawn(name, Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
    }
    for _ in 0..3 {
        world.step(DT);
    }
    let list = draw(&world);
    assert_eq!(list.meshes.len(), 3);
    let z = |k: usize| Vec3::from(list.meshes[k].transform.matrix3.z_axis).normalize();
    let x = |k: usize| Vec3::from(list.meshes[k].transform.matrix3.x_axis).normalize();
    assert!(z(0).distance(Vec3::Z) < 1.0e-3, "the strip's long axis stays upright: {:?}", z(0));
    // The camera of `draw` is at -Y: the mesh's X (its face) looks at it.
    assert!(x(0).distance(-Vec3::Y) < 1.0e-3, "its face looks at the camera: {:?}", x(0));
    assert!(z(1).dot(Vec3::Z).abs() < 1.0e-3, "with the rotation-axis flag the rolled node's Z is the axis: {:?}", z(1));
    assert!(x(2).distance(Vec3::Z) < 1.0e-3, "pitched 90 inside the billboard frame: the mesh's X stands up: {:?}", x(2));
}

/// (fourth pass) `SQEX_ChangeSceneColor.LightIntensity` reaches the host as `World::scene_light`,
/// and plays to its last key although the emitter's own timeline is shorter.
#[test]
fn an_effect_darkens_the_scene_by_its_light_curve() {
    let curve = json!({"distribution": {"type": "constant_curve", "curve": {"keys": [{"t": 0.0, "interp": "CIM_Linear", "value": [1.0]}, {"t": 0.1, "interp": "CIM_Linear", "value": [0.3]}, {"t": 0.4, "interp": "CIM_Linear", "value": [0.3]}, {"t": 0.6, "interp": "CIM_Linear", "value": [1.0]}]}, "lookup_table": {"op": 1, "entry_count": 7, "entry_stride": 1, "sub_entry_stride": 0, "time_scale": 10.0, "time_bias": 0.0, "lock_flag": 0, "values": [1.0, 0.3, 0.3, 0.3, 0.3, 0.65, 1.0]}}});
    let dark = module("ParticleModuleSQEX_ChangeSceneColor", json!({"LightIntensity": curve}));
    let timed = json!({"loops": 1, "duration": 0.3, "sqex_emitter_time": {"start_time": 0.0, "loop_time": 0.3, "loop_duration": 0.3, "duration_time": 0.3, "use_legacy_emitter_time": false}});
    let mut world = World::new();
    world.register(effect("dark", vec![emitter("ctl", "sqex_control", 0, vec![dark], json!({}), timed)]));
    assert_eq!(world.scene_light(), 1.0);
    let id = world.spawn("dark", Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
    for _ in 0..12 {
        world.step(DT);
    }
    assert!((world.scene_light() - 0.3).abs() < 0.02, "dark at 0.2 s: {}", world.scene_light());
    for _ in 0..18 {
        world.step(DT);
    }
    let rising = world.scene_light();
    assert!(world.is_alive(id) && rising > 0.35 && rising < 0.95, "past the emitter's 0.3 s the curve still plays: {rising}");
    for _ in 0..12 {
        world.step(DT);
    }
    assert!(!world.is_alive(id));
    assert_eq!(world.scene_light(), 1.0);
}

/// (fourth pass) A parent whose spawn shape is a ring of order ALL puts one child on every vertex
/// per generation (Water's cast ring: three ribbons).
#[test]
fn a_ring_of_order_all_generates_a_child_on_every_vertex() {
    let ring = module("ParticleModuleSQEX_Ring", json!({"VertexNumber": constant(json!(3.0)), "RangeRadius": constant(json!(100.0)), "Interval": constant(json!(1.0)), "DirectionAngle": constant(json!(360.0)), "VartexGenerateType": "EVARTEX_ODER_TYPE_ALL"}));
    let parent = module("ParticleModuleSQEX_Parent", json!({"ChildrenData": [{"GenerationConditions": "ESQEX_CREATE_GENERATE_TYPE_TIME", "GenerateEmitterName": "kid", "bUseSpawnModule": true, "MaxGenerateCount": 0, "LoopNumber": 1, "TimeDatas": [{"GenerateTime": 0.0, "GenerateNumber": 1}], "OffsetPosition": {"x": 0, "y": 0, "z": 0}, "OffsetRotation": {"pitch": 0, "yaw": 0, "roll": 0}, "OffsetScale": {"x": 1, "y": 1, "z": 1}}]}));
    let life = module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(100.0))}));
    let size = module("ParticleModuleSize", json!({"StartSize": constant(json!([10.0, 10.0, 10.0]))}));
    let mut world = World::new();
    world.register(effect("ring", vec![emitter("ctl", "sqex_control", 0, vec![ring, parent], json!({}), json!({})), emitter("kid", "sprite", 1, vec![life, size], json!({}), json!({"params": {"bAutoAddToComponent": false}}))]));
    world.spawn("ring", Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
    for _ in 0..4 {
        world.step(DT);
    }
    let list = draw(&world);
    assert_eq!(list.quads.len(), 3, "one child per vertex");
    for q in &list.quads {
        assert!((q.center.length() - 100.0).abs() < 1.0, "on the ring: {:?}", q.center);
    }
    assert!(list.quads[0].center.distance(list.quads[1].center) > 100.0, "on different vertices");
}

/// (fifth pass) The mesh type data's UV selectors (read from the mesh-particle vertex shader's uniform row 3):
/// `UVType` RIGHT_ROT = 1, LIGHT_ROT = 2, + 4 for `bTexUReverse`, + 8 for `bTexVReverse`; the drawn instance carries the code.
#[test]
fn a_mesh_emitter_carries_its_uv_turn_and_reverse() {
    let mesh = |params: Value| effect("m", vec![emitter("e", "mesh", 1, vec![module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(1.0))}))], json!({"mesh": "/Game/Effects/a/mdl/ring.ring", "params": params}), json!({}))]);
    assert_eq!(mesh(json!({})).emitters[0].mesh_uv_mode, 0.0);
    assert_eq!(mesh(json!({"UVType": "SQEX_TAILPOL_UV_ROT_TYPE_RIGHT_ROT"})).emitters[0].mesh_uv_mode, 1.0);
    assert_eq!(mesh(json!({"UVType": "SQEX_TAILPOL_UV_ROT_TYPE_LIGHT_ROT", "bTexUReverse": true})).emitters[0].mesh_uv_mode, 6.0);
    let def = mesh(json!({"UVType": "SQEX_TAILPOL_UV_ROT_TYPE_NONE", "bTexVReverse": true}));
    assert_eq!(def.emitters[0].mesh_uv_mode, 8.0);
    let mut world = World::new();
    world.register(def);
    world.spawn("m", Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
    world.step(1.0 / 60.0);
    let list = draw(&world);
    assert!(!list.meshes.is_empty() && list.meshes.iter().all(|m| m.uv_mode == 8.0));
}
