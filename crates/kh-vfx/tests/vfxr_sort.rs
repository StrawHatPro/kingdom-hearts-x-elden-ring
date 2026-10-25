//! (VFXR sort) KH3's translucency order of effects: the component's sort priority (`bOverrideTranslucencySortPriority` /
//! `bSQEXTranslucencySortPriority`), its distance, and the emitter order inside one component, as `DrawGroup`s in the draw
//! list (kh-vfx draw.rs); the emitters' own `SortMode`. Zettaflare's beam on the pack when it is on disk.

use glam::Vec3;
use kh_vfx::{group_order, Camera, DrawList, EffectDef, SpawnOptions, World, Xform};
use serde_json::{json, Value};

const DT: f32 = 1.0 / 60.0;

fn constant(v: Value) -> Value {
    json!({"distribution": {"type": "constant", "constant": v}})
}

fn emitter(name: &str, sort: &str) -> Value {
    json!({
        "name": name, "enabled": true, "typedata": {"kind": "sprite"},
        "required": {
            "material": "/Game/Effects/test/mat/m_test.m_test", "screen_alignment": "PSA_Square", "use_local_space": false,
            "kill_on_completed": false, "duration": 10.0, "loops": 1,
            "sqex_emitter_time": {"start_time": 0.0, "loop_time": 0.0, "loop_duration": 10.0, "duration_time": 10.0, "use_legacy_emitter_time": false},
            "sub_uv": {"horizontal": 1, "vertical": 1, "interpolation": "PSUVIM_None"},
            "use_max_draw_count": true, "max_draw_count": 500,
            "emitter_origin": {"x": 0, "y": 0, "z": 0}, "emitter_rotation": {"pitch": 0, "yaw": 0, "roll": 0},
            "params": {"bAutoAddToComponent": true, "SortMode": sort}
        },
        "spawn": {"enabled": true, "process_spawn_rate": true, "process_burst_list": true, "rate": constant(json!(60.0)), "rate_scale": constant(json!(1.0)), "burst_scale": constant(json!(1.0)), "bursts": []},
        "modules": [
            {"class": "ParticleModuleLifetime", "enabled": true, "params": {"Lifetime": constant(json!(2.0))}},
            {"class": "ParticleModuleSize", "enabled": true, "params": {"StartSize": constant(json!({"x": 10, "y": 10, "z": 10}))}},
            {"class": "ParticleModuleVelocity", "enabled": true, "params": {"StartVelocity": constant(json!({"x": 100, "y": 0, "z": 0}))}}
        ]
    })
}

fn effect(name: &str, system: Value, emitters: Vec<Value>) -> EffectDef {
    EffectDef::from_json(&json!({"name": name, "system": {"params": system}, "emitters": emitters})).unwrap()
}

fn drawn(world: &World, camera: &Camera) -> DrawList {
    let mut list = DrawList::default();
    world.draw(1.0, camera, &mut list);
    list
}

#[test]
fn the_sort_priority_is_read_as_kh3_writes_it() {
    let plain = effect("a", json!({}), vec![emitter("e", "PSORTMODE_None")]);
    assert_eq!((plain.sort_priority, plain.sort_offset), (0, 0.0));
    // the override and SQEX's priority (an int in a field named like a bool)
    let over = effect("b", json!({"bOverrideTranslucencySortPriority": true, "bSQEXTranslucencySortPriority": 1, "SQEXTranslucencySortOffset": -10.0}), vec![emitter("e", "PSORTMODE_None")]);
    assert_eq!((over.sort_priority, over.sort_offset), (1, -10.0));
    // without the override, SQEX's value is not the priority
    let off = effect("c", json!({"bOverrideTranslucencySortPriority": false, "bSQEXTranslucencySortPriority": 150}), vec![emitter("e", "PSORTMODE_None")]);
    assert_eq!(off.sort_priority, 0);
}

#[test]
fn every_draw_is_in_its_emitters_group_and_the_order_is_priority_distance_emitter() {
    let camera = Camera::DEFAULT;
    let mut w = World::new();
    w.register(effect("near", json!({}), vec![emitter("first", "PSORTMODE_None"), emitter("second", "PSORTMODE_None")]));
    w.register(effect("far", json!({}), vec![emitter("only", "PSORTMODE_None")]));
    w.register(effect("over", json!({"bOverrideTranslucencySortPriority": true, "bSQEXTranslucencySortPriority": 1}), vec![emitter("only", "PSORTMODE_None")]));
    let at = |d: f32| Xform { position: camera.position + camera.forward * d, ..Xform::IDENTITY };
    let opt = || SpawnOptions { seed: Some(3), ..SpawnOptions::default() };
    let near = w.spawn("near", at(300.0), opt()).unwrap();
    let far = w.spawn("far", at(3000.0), opt()).unwrap();
    let over = w.spawn("over", at(3000.0), opt()).unwrap();
    for _ in 0..10 {
        w.step(DT);
    }
    let list = drawn(&w, &camera);
    let table = list.group_table(0, list.quads.len());
    assert!(!list.quads.is_empty() && table.iter().all(|g| *g != usize::MAX), "every quad has its group");
    // one group per emitter here (sprites only)
    assert_eq!(list.groups.len(), 4);
    let mut order: Vec<_> = list.groups.clone();
    order.sort_by(|a, b| group_order(a, b, camera.position));
    let named: Vec<(u64, u32)> = order.iter().map(|g| (g.system, g.emitter)).collect();
    // far first, then the near effect's emitters in their order, then the priority-1 effect over everything
    assert_eq!(named, vec![(far.0, 0), (near.0, 0), (near.0, 1), (over.0, 0)]);
}

#[test]
fn an_emitters_sort_mode_orders_its_own_particles() {
    let camera = Camera::DEFAULT;
    let mut w = World::new();
    // the particles fly along +X from the camera's side: the far-first emitter draws the farthest first
    w.register(effect("s", json!({}), vec![emitter("far_first", "PSORTMODE_DistanceToView")]));
    w.register(effect("n", json!({}), vec![emitter("newest", "PSORTMODE_Age_NewestFirst")]));
    let at = Xform { position: camera.position + camera.forward * 500.0, ..Xform::IDENTITY };
    w.spawn("s", at, SpawnOptions { seed: Some(5), ..SpawnOptions::default() }).unwrap();
    for _ in 0..30 {
        w.step(DT);
    }
    let list = drawn(&w, &camera);
    let d: Vec<f32> = list.quads.iter().map(|q| q.center.distance(camera.position)).collect();
    assert!(d.len() > 3 && d.windows(2).all(|p| p[0] >= p[1] - 1.0e-3), "far first: {d:?}");
    let mut w2 = World::new();
    w2.register(effect("n", json!({}), vec![emitter("newest", "PSORTMODE_Age_NewestFirst")]));
    w2.spawn("n", at, SpawnOptions { seed: Some(5), ..SpawnOptions::default() }).unwrap();
    for _ in 0..30 {
        w2.step(DT);
    }
    let rel: Vec<f32> = drawn(&w2, &camera).quads.iter().map(|q| q.rel).collect();
    assert!(rel.len() > 3 && rel.windows(2).all(|p| p[0] <= p[1] + 1.0e-6), "newest (youngest) first: {rel:?}");
}

/// Zettaflare's beam (pack effects vd_kg702_92_zfr_01 and 94_zfr_01): the black backing card c_laser_mdl_white00010 is drawn
/// before every glow layer of the beam, whatever the camera.
#[test]
fn zettaflares_backing_card_is_drawn_before_its_glows() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().map(|a| a.join("build/play/donpack/party/vfx/effects")).find(|p| p.is_dir());
    let Some(root) = root else {
        eprintln!("(no pack: skipped)");
        return;
    };
    let read = |n: &str| std::fs::read_to_string(root.join(format!("{n}.json"))).ok().and_then(|t| EffectDef::parse(&t).ok());
    let (Some(a), Some(b)) = (read("vd_kg702_92_zfr_01"), read("vd_kg702_94_zfr_01")) else {
        eprintln!("(no Zettaflare beam effects: skipped)");
        return;
    };
    assert_eq!((a.sort_priority, b.sort_priority), (0, 1), "92 at the default priority, 94 over it (KH3's data)");
    let card = a.emitters.iter().position(|e| e.name == "c_laser_mdl_white00010").expect("the backing card");
    let glows: Vec<u32> = a.emitters.iter().enumerate().filter(|(k, e)| *k != card && e.name.starts_with("laser_mdl")).map(|(k, _)| k as u32).collect();
    assert!(!glows.is_empty() && glows.iter().all(|g| *g > card as u32), "the glows come after the card in the component");
    // the group order at several cameras: card (system a, emitter `card`) first among a's, every b group after all of a's
    for cam in [Vec3::new(-2000.0, 0.0, 200.0), Vec3::new(2000.0, 500.0, 100.0), Vec3::new(0.0, -3000.0, 1500.0)] {
        let g = |system: u64, emitter: u32, priority: i32, origin: Vec3| kh_vfx::DrawGroup { kind: 2, from: 0, to: 1, system, emitter, priority, origin, offset: 0.0 };
        let mut groups = vec![g(2, 0, 1, Vec3::new(5000.0, 0.0, 0.0)), g(1, card as u32, 0, Vec3::ZERO)];
        groups.extend(glows.iter().map(|e| g(1, *e, 0, Vec3::ZERO)));
        groups.sort_by(|x, y| group_order(x, y, cam));
        assert_eq!((groups[0].system, groups[0].emitter), (1, card as u32), "the card first at {cam:?}");
        assert_eq!(groups.last().map(|x| x.system), Some(2), "the priority-1 effect last at {cam:?}");
    }
}
