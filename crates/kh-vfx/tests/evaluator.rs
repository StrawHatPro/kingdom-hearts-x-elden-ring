//! The evaluator on small hand-written effects (the pack's JSON shape), plus one pass over the
//! real pack when it is on disk: every effect parses, spawns and runs without a panic.

use glam::Vec3;
use kh_vfx::{Camera, DrawList, EffectDef, Kind, SpawnOptions, World, Xform};
use serde_json::{json, Value};

const DT: f32 = 1.0 / 60.0;

fn constant(v: Value) -> Value {
    json!({"distribution": {"type": "constant", "constant": v}})
}

fn uniform(min: Value, max: Value) -> Value {
    json!({"distribution": {"type": "uniform", "min": min, "max": max}})
}

fn module(class: &str, params: Value) -> Value {
    json!({"class": class, "enabled": true, "params": params})
}

/// One emitter in the pack's shape. `time` = (start, loop_time, loop_duration, duration_time).
fn emitter(name: &str, kind: &str, loops: u32, time: (f32, f32, f32, f32), rate: f32, bursts: Value, modules: Vec<Value>, extra: Value) -> Value {
    let mut required = json!({
        "material": "/Game/Effects/test/mat/m_test.m_test", "screen_alignment": "PSA_Square", "use_local_space": false,
        "kill_on_completed": false, "duration": time.3, "loops": loops,
        "sqex_emitter_time": {"start_time": time.0, "loop_time": time.1, "loop_duration": time.2, "duration_time": time.3, "use_legacy_emitter_time": false},
        "sub_uv": {"horizontal": 1, "vertical": 1, "interpolation": "PSUVIM_None"},
        "use_max_draw_count": true, "max_draw_count": 500,
        "emitter_origin": {"x": 0, "y": 0, "z": 0}, "emitter_rotation": {"pitch": 0, "yaw": 0, "roll": 0},
        "params": {"bAutoAddToComponent": true, "RotationPriority": "ROTATION_PRIORITY_XZY"}
    });
    if let Some(map) = extra.as_object() {
        for (k, v) in map {
            if k == "auto_add" {
                required["params"]["bAutoAddToComponent"] = v.clone();
            } else {
                required[k] = v.clone();
            }
        }
    }
    json!({
        "name": name, "enabled": true, "typedata": {"kind": kind, "mesh": if kind == "mesh" { json!("/Game/Effects/test/mdl/ring.ring") } else { Value::Null }},
        "required": required,
        "spawn": {"enabled": true, "process_spawn_rate": true, "process_burst_list": true, "rate": constant(json!(rate)), "rate_scale": constant(json!(1.0)), "burst_scale": constant(json!(1.0)), "bursts": bursts},
        "modules": modules
    })
}

fn effect(name: &str, emitters: Vec<Value>) -> EffectDef {
    EffectDef::from_json(&json!({"name": name, "system": {"params": {"SQEXBaseColor": {"r": 1, "g": 1, "b": 1, "a": 1}}}, "emitters": emitters})).unwrap()
}

fn world_with(def: EffectDef) -> World {
    let mut world = World::new();
    world.register(def);
    world
}

fn run(world: &mut World, steps: usize) {
    for _ in 0..steps {
        world.step(DT);
    }
}

fn drawn(world: &World) -> DrawList {
    let mut list = DrawList::default();
    world.draw(1.0, &Camera::DEFAULT, &mut list);
    list
}

#[test]
fn burst_lifetime_and_end_of_a_one_shot() {
    let def = effect("one_shot", vec![emitter("e", "sprite", 1, (0.0, 0.0, 0.1, 0.1), 0.0, json!([{"count": 5, "count_low": -1, "time": 0.0}]), vec![module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(0.5))})), module("ParticleModuleSize", json!({"StartSize": constant(json!([20.0, 20.0, 20.0]))}))], json!({}))]);
    let mut world = world_with(def);
    let id = world.spawn("one_shot", Xform::at(Vec3::new(100.0, 0.0, 50.0)), SpawnOptions::default()).unwrap();
    world.step(DT);
    assert_eq!(world.stats(), (1, 1, 5));
    let list = drawn(&world);
    assert_eq!(list.quads.len(), 5);
    // A 20 cm square around the spawn point, facing the camera (which looks along +X).
    let q = &list.quads[0];
    let center = (q.corners[0] + q.corners[2]) * 0.5;
    assert!((center - Vec3::new(100.0, 0.0, 50.0)).length() < 1e-3);
    assert!((q.corners[1] - q.corners[0]).length() - 20.0 < 1e-3 && (q.corners[0].z - q.corners[3].z - 20.0).abs() < 1e-3);
    assert_eq!(world.material_name(q.material), Some("m_test"));
    // Alive until the particles are 0.5 s old, then the effect removes itself.
    run(&mut world, 27);
    assert!(world.is_alive(id));
    run(&mut world, 5);
    assert!(!world.is_alive(id));
    assert_eq!(world.stats(), (0, 0, 0));
}

#[test]
fn spawn_rate_velocity_acceleration_and_drag() {
    let def = effect(
        "rate",
        vec![emitter(
            "e",
            "sprite",
            1,
            (0.0, 0.0, 1.0, 1.0),
            30.0,
            json!([]),
            vec![
                module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(10.0))})),
                module("ParticleModuleSize", json!({"StartSize": constant(json!([1.0, 1.0, 1.0]))})),
                module("ParticleModuleVelocity", json!({"StartVelocity": constant(json!([100.0, 0.0, 0.0])), "bInWorldSpace": false})),
                module("ParticleModuleAccelerationConstant", json!({"Acceleration": {"x": 0, "y": 0, "z": -600}})),
            ],
            json!({}),
        )],
    );
    let mut world = world_with(def);
    // The component is yawed 90 degrees: a local +X velocity flies along world +Y.
    world.spawn("rate", Xform::from_rotator(Vec3::ZERO, 0.0, 90.0, 0.0, 1.0), SpawnOptions::default()).unwrap();
    run(&mut world, 60);
    // 30 per second for one second (the fraction is carried from step to step).
    let (_, _, particles) = world.stats();
    assert!((29..=31).contains(&particles), "{particles}");
    let list = drawn(&world);
    let oldest = list.quads.iter().map(|q| (q.corners[0] + q.corners[2]) * 0.5).fold(Vec3::ZERO, |a, c| if c.y > a.y { c } else { a });
    // About one second of flight: y = 100 t, z = -300 t^2 (semi-implicit Euler is a little off).
    assert!((oldest.y - 100.0).abs() < 5.0 && oldest.x.abs() < 1e-2, "{oldest}");
    assert!((oldest.z + 300.0).abs() < 15.0, "{oldest}");
}

#[test]
fn drag_slows_a_particle() {
    let def = effect(
        "drag",
        vec![emitter(
            "e",
            "sprite",
            1,
            (0.0, 0.0, 0.1, 0.1),
            0.0,
            json!([{"count": 1, "count_low": -1, "time": 0.0}]),
            vec![module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(5.0))})), module("ParticleModuleSize", json!({"StartSize": constant(json!([1.0, 1.0, 1.0]))})), module("ParticleModuleVelocity", json!({"StartVelocity": constant(json!([100.0, 0.0, 0.0]))})), module("ParticleModuleAccelerationDrag", json!({"DragCoefficientRaw": constant(json!(2.0))}))],
            json!({}),
        )],
    );
    let mut world = world_with(def);
    world.spawn("drag", Xform::IDENTITY, SpawnOptions::default()).unwrap();
    run(&mut world, 120);
    let x = (drawn(&world).quads[0].corners[0] + drawn(&world).quads[0].corners[2]).x * 0.5;
    // v = 100 e^(-2t): x -> 50 (1 - e^(-4)) = 49.1 after two seconds.
    assert!((x - 49.1).abs() < 2.0, "{x}");
}

#[test]
fn over_life_curves_scale_size_and_colour() {
    let ramp = |from: f32, to: f32| json!({"distribution": {"type": "constant_curve", "lookup_table": {"op": 1, "entry_count": 2, "entry_stride": 1, "time_scale": 1.0, "time_bias": 0.0, "lock_flag": 0, "values": [from, to]}}});
    let def = effect(
        "curves",
        vec![emitter(
            "e",
            "sprite",
            1,
            (0.0, 0.0, 0.1, 0.1),
            0.0,
            json!([{"count": 1, "count_low": -1, "time": 0.0}]),
            vec![
                module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(1.0))})),
                module("ParticleModuleSize", json!({"StartSize": constant(json!([10.0, 10.0, 10.0]))})),
                module("ParticleModuleSizeScale", json!({"SizeScale": ramp(1.0, 3.0), "EnableX": true, "EnableY": true, "EnableZ": true})),
                module("ParticleModuleColor", json!({"StartColor": constant(json!([2.0, 1.0, 0.5])), "StartAlpha": constant(json!(1.0))})),
                module("ParticleModuleColorScaleOverLife", json!({"ColorScaleOverLife": constant(json!([1.0, 1.0, 1.0])), "AlphaScaleOverLife": ramp(1.0, 0.0)})),
            ],
            json!({}),
        )],
    );
    let mut world = world_with(def);
    world.spawn("curves", Xform::IDENTITY, SpawnOptions::default()).unwrap();
    run(&mut world, 31);
    let q = drawn(&world).quads[0];
    // Half way through its life: size 10 x 2, alpha 0.5, colour as authored (HDR kept).
    let width = (q.corners[1] - q.corners[0]).length();
    assert!((width - 20.0).abs() < 0.5, "{width}");
    assert!((q.color[3] - 0.5).abs() < 0.03 && (q.color[0] - 2.0).abs() < 1e-4, "{:?}", q.color);
}

#[test]
fn a_looping_emitter_runs_until_loop_end_then_plays_out() {
    let def = effect("loop", vec![emitter("e", "sprite", 0, (0.0, 0.1, 0.3, 0.5), 60.0, json!([]), vec![module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(0.1))})), module("ParticleModuleSize", json!({"StartSize": constant(json!([1.0, 1.0, 1.0]))}))], json!({}))]);
    assert!(def.endless());
    let mut world = world_with(def);
    let id = world.spawn("loop", Xform::IDENTITY, SpawnOptions::default()).unwrap();
    run(&mut world, 600);
    assert!(world.is_alive(id), "a looping effect lives until it is stopped");
    assert!(world.stats().2 >= 5);
    world.stop(id, false);
    // Ending = loop_duration..duration_time (0.2 s at most from anywhere in the loop) + the last particles' 0.1 s.
    run(&mut world, 30);
    assert!(!world.is_alive(id));
    // An immediate stop removes it at once.
    let id = world.spawn("loop", Xform::IDENTITY, SpawnOptions::default()).unwrap();
    run(&mut world, 10);
    world.stop(id, true);
    world.step(DT);
    assert!(!world.is_alive(id));
}

#[test]
fn start_time_delays_an_emitter() {
    let def = effect("late", vec![emitter("e", "sprite", 1, (0.5, 0.0, 0.1, 0.1), 0.0, json!([{"count": 3, "count_low": -1, "time": 0.0}]), vec![module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(0.2))})), module("ParticleModuleSize", json!({"StartSize": constant(json!([1.0, 1.0, 1.0]))}))], json!({}))]);
    let mut world = world_with(def);
    world.spawn("late", Xform::IDENTITY, SpawnOptions::default()).unwrap();
    run(&mut world, 28);
    assert_eq!(world.stats().2, 0);
    run(&mut world, 4);
    assert_eq!(world.stats().2, 3);
}

#[test]
fn node_tracks_move_rotate_and_glue_particles() {
    // The node moves 60 cm/s along its own X (a rate track) and the single particle is attached.
    let def = effect(
        "node",
        vec![emitter(
            "e",
            "mesh",
            1,
            (0.0, 0.0, 2.0, 2.0),
            0.0,
            json!([{"count": 1, "count_low": -1, "time": 0.0}]),
            vec![
                module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(2.0))})),
                module("ParticleModuleSize", json!({"StartSize": constant(json!([2.0, 2.0, 2.0]))})),
                module("ParticleModuleSQEX_EmitterTransforms", json!({"LocalAddPosition": constant(json!([60.0, 0.0, 0.0])), "bCurveParameterDirectPosition": false, "LocalAddRotation": constant(json!([0.0, 0.0, 90.0])), "bCurveParameterDirectRotation": true, "LocalAddScale": constant(json!([3.0, 3.0, 3.0])), "bCurveParameterDirectScale": true, "DisplayType": "DISPLAY_TYPE_NONE"})),
                // The node's scale reaches the particle through the emitter's size multiplier, which
                // this module applies at spawn (0x1442F2770).
                module("ParticleModuleSQEX_InitSizeScale", json!({"bIsScale": true})),
                module("ParticleModuleSQEX_AttachParticle", json!({"bIsAttachPosition": true, "bIsAttachRotation": true})),
            ],
            json!({}),
        )],
    );
    let mut world = world_with(def);
    let id = world.spawn("node", Xform::at(Vec3::new(0.0, 0.0, 100.0)), SpawnOptions::default()).unwrap();
    run(&mut world, 60);
    let list = drawn(&world);
    assert_eq!(list.meshes.len(), 1);
    let m = &list.meshes[0];
    assert_eq!(world.mesh_name(m.mesh), Some("ring"));
    // One second at 60 cm/s (the first tick integrates too), at the component's height.
    let at = Vec3::from(m.transform.translation);
    assert!((at.x - 60.0).abs() < 1.5 && (at.z - 100.0).abs() < 1e-3, "{at}");
    // The node is yawed 90 degrees and scaled 3: the mesh's +X axis points along world +Y, 6 long.
    let x_axis = m.transform.transform_vector3(Vec3::X);
    assert!((x_axis - Vec3::new(0.0, 6.0, 0.0)).length() < 1e-2, "{x_axis}");
    // The component moves: the node (and the attached particle) follows.
    world.set_transform(id, Xform::at(Vec3::new(0.0, 500.0, 100.0)));
    world.step(DT);
    let at = Vec3::from(drawn(&world).meshes[0].transform.translation);
    assert!((at.y - 500.0).abs() < 1e-2, "{at}");
}

#[test]
fn a_parent_creates_its_child_emitters_at_their_times() {
    let child = emitter("kid", "sprite", 1, (0.0, 0.0, 0.2, 0.2), 0.0, json!([{"count": 2, "count_low": -1, "time": 0.0}]), vec![module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(0.2))})), module("ParticleModuleSize", json!({"StartSize": constant(json!([1.0, 1.0, 1.0]))}))], json!({"auto_add": false}));
    let parent = emitter(
        "ctl",
        "sqex_control",
        1,
        (0.0, 0.0, 1.0, 1.0),
        0.0,
        json!([]),
        vec![module("ParticleModuleSQEX_Parent", json!({"AllLoopEndTime": 0, "ChildrenData": [{"GenerationConditions": "ESQEX_CREATE_GENERATE_TYPE_TIME", "GenerateEmitterName": "kid", "MaxGenerateCount": 0, "TimeDatas": [{"GenerateTime": 0.25, "GenerateNumber": 3}], "LoopNumber": 1, "LoopEndTime": 0, "OffsetPosition": {"x": 0, "y": 0, "z": 40}, "OffsetRotation": {"pitch": 0, "yaw": 0, "roll": 0}, "OffsetScale": {"x": 1, "y": 1, "z": 1}, "bUseSpawnModule": false}]}))],
        json!({}),
    );
    let def = effect("family", vec![parent, child]);
    assert_eq!(def.emitters[0].kind, Kind::Control);
    assert!(!def.emitters[1].auto_add);
    let mut world = world_with(def);
    world.spawn("family", Xform::at(Vec3::new(10.0, 0.0, 0.0)), SpawnOptions::default()).unwrap();
    run(&mut world, 14);
    assert_eq!(world.stats(), (1, 1, 0), "the child template does not run by itself");
    run(&mut world, 4);
    // Three child instances, two particles each, 40 cm above the parent node.
    assert_eq!(world.stats(), (1, 4, 6));
    let q = drawn(&world).quads[0];
    let center = (q.corners[0] + q.corners[2]) * 0.5;
    assert!((center - Vec3::new(10.0, 0.0, 40.0)).length() < 1e-3, "{center}");
}

#[test]
fn range_and_ring_place_particles_on_their_shapes() {
    let shape = |class: &str, params: Value| {
        emitter(
            "e",
            "sprite",
            1,
            (0.0, 0.0, 0.1, 0.1),
            0.0,
            json!([{"count": 64, "count_low": -1, "time": 0.0}]),
            vec![module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(1.0))})), module("ParticleModuleSize", json!({"StartSize": constant(json!([1.0, 1.0, 1.0]))})), module(class, params)],
            json!({}),
        )
    };
    let range = effect("range", vec![shape("ParticleModuleSQEX_Range", json!({"RangeRadius": constant(json!(50.0)), "DirectionAngle": constant(json!(30.0)), "Deviation": constant(json!(1.0)), "RangeScale": constant(json!([1.0, 1.0, 1.0])), "Direction": {"pitch": 90, "yaw": 0, "roll": 0}, "DirectionType": "EPARTICLE_DIR_TYPE_EMITTER_NORMAL", "Speed": constant(json!(0.0)), "NormalDirection": 1}))]);
    let ring = effect("ring", vec![shape("ParticleModuleSQEX_Ring", json!({"VertexNumber": constant(json!(8.0)), "RangeRadius": constant(json!(30.0)), "VartexGenerateType": "EVARTEX_ODER_TYPE_ORDER", "Direction": {"pitch": 0, "yaw": 0, "roll": 0}, "DirectionType": "EPARTICLE_DIR_TYPE_EMITTER_NORMAL", "Speed": constant(json!(120.0)), "NormalDirection": 1, "bAttachParticle": false}))]);
    let mut world = World::new();
    world.register(range);
    world.register(ring);
    world.spawn("range", Xform::IDENTITY, SpawnOptions::default()).unwrap();
    world.step(DT);
    let list = drawn(&world);
    assert_eq!(list.quads.len(), 64);
    for q in &list.quads {
        // On the sphere of 50 cm (Deviation 1 = the surface, 0x144352710), inside the 30 degree cap about +Z (Direction pitch 90 lifts +X to +Z).
        let c = (q.corners[0] + q.corners[2]) * 0.5;
        assert!((c.length() - 50.0).abs() < 1e-2, "{c}");
        assert!(c.z / 50.0 >= 30.0f32.to_radians().cos() - 1e-3, "{c}");
    }
    let mut world2 = World::new();
    world2.register(world.def("ring").unwrap().clone());
    world2.spawn("ring", Xform::IDENTITY, SpawnOptions::default()).unwrap();
    world2.step(DT);
    let list = drawn(&world2);
    assert_eq!(list.quads.len(), 64);
    let mut distinct: Vec<Vec3> = Vec::new();
    for q in &list.quads {
        let c = (q.corners[0] + q.corners[2]) * 0.5;
        // On a circle of 30 cm in the XY plane: a ring's corners are (radius, 0, 0) turned about Z (0x144355000).
        assert!((Vec3::new(c.x, c.y, 0.0).length() - 30.0).abs() < 1e-2 && c.z.abs() < 1e-2, "{c}");
        if !distinct.iter().any(|d| d.distance(c) < 1e-2) {
            distinct.push(c);
        }
    }
    assert_eq!(distinct.len(), 8);
    // They fly outward at 120 cm/s.
    run(&mut world2, 30);
    let c = (drawn(&world2).quads[0].corners[0] + drawn(&world2).quads[0].corners[2]) * 0.5;
    assert!((c.length() - 90.0).abs() < 3.0, "{c}");
}

#[test]
fn an_anim_trail_is_a_ribbon_between_the_sockets() {
    let def = effect("trail", vec![emitter("e", "anim_trail", 1, (0.0, 0.05, 0.08, 0.18), 20.0, json!([]), vec![module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(0.2))})), module("ParticleModuleSize", json!({"StartSize": constant(json!([25.0, 25.0, 25.0]))})), module("ParticleModuleColor", json!({"StartColor": constant(json!([3.0, 3.0, 3.0])), "StartAlpha": constant(json!(1.0))}))], json!({}))]);
    let mut world = world_with(def);
    let id = world.spawn("trail", Xform::IDENTITY, SpawnOptions::default()).unwrap();
    for k in 0..30 {
        let x = k as f32 * 10.0;
        world.set_trail(id, Vec3::new(x, 0.0, 80.0), Vec3::new(x, 0.0, 15.0));
        world.step(DT);
    }
    let list = drawn(&world);
    assert_eq!(list.ribbons.len(), 1);
    let ribbon = &list.ribbons[0];
    // 0.2 s of samples at 60 Hz; the head is the newest sample, u runs to the tail.
    assert!((11..=13).contains(&ribbon.points.len()), "{}", ribbon.points.len());
    assert_eq!(ribbon.points[0].a, Vec3::new(290.0, 0.0, 80.0));
    assert_eq!(ribbon.points[0].b, Vec3::new(290.0, 0.0, 15.0));
    assert!(ribbon.points.windows(2).all(|w| w[0].u <= w[1].u && w[0].a.x > w[1].a.x));
    // Fade out: gone after the fade time.
    world.fade_out(id, 0.15);
    run(&mut world, 5);
    assert!(world.is_alive(id));
    let alpha = drawn(&world).ribbons[0].points[0].color[3];
    assert!((alpha - (1.0 - 5.0 * DT / 0.15)).abs() < 0.02, "{alpha}");
    run(&mut world, 6);
    assert!(!world.is_alive(id));
}

#[test]
fn an_unknown_module_is_skipped_and_logged_once() {
    let def = effect(
        "odd",
        vec![emitter("e", "sprite", 1, (0.0, 0.0, 0.1, 0.1), 0.0, json!([{"count": 1, "count_low": -1, "time": 0.0}]), vec![module("ParticleModuleLifetime", json!({"Lifetime": uniform(json!(0.2), json!(0.4))})), module("ParticleModuleSQEX_NobodyKnows", json!({"Strength": 3})), module("ParticleModuleSQEX_NobodyKnows", json!({"Strength": 4}))], json!({}))],
    );
    assert_eq!(def.unknown(), vec![("e".to_string(), "SQEX_NobodyKnows".to_string())]);
    let mut world = World::new();
    world.register(def.clone());
    world.register(def);
    let log = world.take_log();
    assert_eq!(log.len(), 1, "{log:?}");
    assert!(log[0].contains("odd") && log[0].contains("SQEX_NobodyKnows"));
    // It still runs.
    world.spawn("odd", Xform::IDENTITY, SpawnOptions::default()).unwrap();
    run(&mut world, 5);
    assert_eq!(world.stats().2, 1);
    // An effect nobody registered is refused quietly, with one line.
    assert!(world.spawn("missing", Xform::IDENTITY, SpawnOptions::default()).is_none());
    assert!(world.spawn("missing", Xform::IDENTITY, SpawnOptions::default()).is_none());
    assert_eq!(world.take_log().len(), 1);
}

#[test]
fn the_same_spawns_give_the_same_particles() {
    let make = || {
        let def = effect(
            "random",
            vec![emitter(
                "e",
                "sprite",
                1,
                (0.0, 0.0, 0.5, 0.5),
                40.0,
                json!([]),
                vec![module("ParticleModuleLifetime", json!({"Lifetime": uniform(json!(0.2), json!(0.6))})), module("ParticleModuleSize", json!({"StartSize": uniform(json!([1.0, 0.0, 0.0]), json!([9.0, 0.0, 0.0]))})), module("ParticleModuleVelocity", json!({"StartVelocity": uniform(json!([-50.0, -50.0, 0.0]), json!([50.0, 50.0, 90.0]))}))],
                json!({}),
            )],
        );
        let mut world = world_with(def);
        world.spawn("random", Xform::IDENTITY, SpawnOptions::default()).unwrap();
        run(&mut world, 25);
        drawn(&world)
    };
    let (a, b) = (make(), make());
    assert!(!a.quads.is_empty());
    assert_eq!(a, b);
}

/// Every effect of the real pack: parses, spawns, runs two seconds, is asked to end, runs out.
/// Skipped when the pack is not on disk.
#[test]
fn every_effect_of_the_pack_runs() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/vfx/effects");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        eprintln!("no effect pack at {}: skipped", dir.display());
        return;
    };
    let mut world = World::new();
    let (mut effects, mut drew, mut peak, mut lingering) = (0, 0, 0usize, Vec::new());
    let mut list = DrawList::default();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let def = EffectDef::parse(&std::fs::read_to_string(&path).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let name = def.name.clone();
        world.register(def);
        let id = world.spawn(&name, Xform::at(Vec3::new(0.0, 0.0, 100.0)), SpawnOptions::default()).unwrap();
        let mut any = false;
        for step in 0..360 {
            if step == 120 {
                world.stop(id, false);
            }
            world.set_trail(id, Vec3::new(step as f32, 0.0, 180.0), Vec3::new(step as f32, 0.0, 115.0));
            world.step(DT);
            if step % 10 == 0 {
                list.clear();
                world.draw(0.5, &Camera::DEFAULT, &mut list);
                any |= !list.is_empty();
                for q in &list.quads {
                    assert!(q.corners.iter().all(|c| c.is_finite()) && q.color.iter().all(|c| c.is_finite()), "{name}: a quad is not finite");
                }
                for m in &list.meshes {
                    assert!(m.transform.is_finite(), "{name}: a mesh transform is not finite");
                }
            }
            peak = peak.max(world.stats().2);
        }
        if world.is_alive(id) {
            lingering.push(name);
            world.stop(id, true);
            world.step(DT);
        }
        effects += 1;
        drew += any as usize;
    }
    let skipped = world.take_log();
    eprintln!("pack: {effects} effects ran, {drew} drew something, peak {peak} particles in one effect, {} still alive 4 s after loop end, {} skipped-module lines", lingering.len(), skipped.len());
    eprintln!("lingering: {}", lingering.iter().take(40).cloned().collect::<Vec<_>>().join(" "));
    assert!(effects > 300, "{effects}");
}

// ---------------------------------------------------------------------------------------------
// Third pass: the traced node chain, spawn shapes, noise, decals, camera shake requests.

fn centers(list: &DrawList) -> Vec<Vec3> {
    list.quads.iter().map(|q| (q.corners[0] + q.corners[2]) * 0.5).collect()
}

fn burst_of(count: u32, modules: Vec<Value>, extra: Value) -> Value {
    let mut all = vec![module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(5.0))})), module("ParticleModuleSize", json!({"StartSize": constant(json!([1.0, 1.0, 1.0]))}))];
    all.extend(modules);
    emitter("e", "sprite", 1, (0.0, 0.0, 0.1, 0.1), 0.0, json!([{"count": count, "count_low": -1, "time": 0.0}]), all, extra)
}

#[test]
fn the_deviation_of_a_range_is_the_games() {
    // 0x144352710: with Deviation d the squared radius fraction lies in [d, 1] with probability d,
    // else in [0, d]. 0 = everything at the centre, 0.5 = uniform over the area, 1 = the surface.
    let range = |d: f64| effect("r", vec![burst_of(180, vec![module("ParticleModuleSQEX_Range", json!({"RangeRadius": constant(json!(100.0)), "DirectionAngle": constant(json!(180.0)), "Deviation": constant(json!(d)), "DirectionType": "EPARTICLE_DIR_TYPE_NONE"}))], json!({}))]);
    let fractions = |d: f64| {
        let mut world = world_with(range(d));
        world.spawn("r", Xform::IDENTITY, SpawnOptions::default()).unwrap();
        world.step(DT);
        centers(&drawn(&world)).iter().map(|c| (c.length() / 100.0).powi(2)).collect::<Vec<f32>>()
    };
    assert!(fractions(0.0).iter().all(|f| *f < 1.0e-6), "Deviation 0: the centre");
    assert!(fractions(1.0).iter().all(|f| (*f - 1.0).abs() < 1.0e-3), "Deviation 1: the surface");
    let half = fractions(0.5);
    let mean = half.iter().sum::<f32>() / half.len() as f32;
    assert!((mean - 0.5).abs() < 0.08, "Deviation 0.5: uniform in the squared radius, mean {mean}");
    // 0.8: four in five lie in the outer shell [0.8, 1].
    let outer = fractions(0.8).iter().filter(|f| **f >= 0.8 - 1.0e-4).count() as f32 / 180.0;
    assert!((outer - 0.8).abs() < 0.1, "{outer}");
}

#[test]
fn a_ring_is_a_polygon_about_z_with_steps_along_its_edges() {
    // Four corners, two steps per edge, in order: corners and edge midpoints of a square in the
    // XY plane (0x144355000: (radius, 0, 0) turned about Z; 0x1443484B0: the step along the edge).
    let def = effect("ring", vec![burst_of(8, vec![module("ParticleModuleSQEX_Ring", json!({"VertexNumber": constant(json!(4.0)), "RangeRadius": constant(json!(100.0)), "Interval": constant(json!(2.0)), "VartexGenerateType": "EVARTEX_ODER_TYPE_ORDER", "DirectionType": "EPARTICLE_DIR_TYPE_VERTEX_NORMAL", "Speed": constant(json!(60.0)), "NormalDirection": 1}))], json!({}))]);
    let mut world = world_with(def);
    world.spawn("ring", Xform::IDENTITY, SpawnOptions::default()).unwrap();
    world.step(DT);
    let points = centers(&drawn(&world));
    assert_eq!(points.len(), 8);
    let expected = [Vec3::new(100.0, 0.0, 0.0), Vec3::new(50.0, 50.0, 0.0), Vec3::new(0.0, 100.0, 0.0), Vec3::new(-50.0, 50.0, 0.0), Vec3::new(-100.0, 0.0, 0.0), Vec3::new(-50.0, -50.0, 0.0), Vec3::new(0.0, -100.0, 0.0), Vec3::new(50.0, -50.0, 0.0)];
    for (p, e) in points.iter().zip(expected) {
        assert!(p.distance(e) < 1.0e-2, "{p} != {e}");
    }
    // VERTEX_NORMAL of a ring is its axis: they rise along +Z at 60 cm/s.
    run(&mut world, 60);
    for p in centers(&drawn(&world)) {
        assert!((p.z - 60.0).abs() < 1.5, "{p}");
    }
}

#[test]
fn a_quadrangle_puts_points_on_the_faces_of_its_box() {
    // 0x1443565C0: half extents = QuadrangleSize (Deviation 1), one coordinate at +- the extent.
    let def = effect("quad", vec![burst_of(120, vec![module("ParticleModuleSQEX_Quadrangle", json!({"QuadrangleSize": {"x": 100.0, "y": 50.0, "z": 20.0}, "Deviation": constant(json!(1.0)), "DirectionType": "EPARTICLE_DIR_TYPE_NONE"}))], json!({}))]);
    let mut world = world_with(def);
    world.spawn("quad", Xform::IDENTITY, SpawnOptions::default()).unwrap();
    world.step(DT);
    let mut faces = [0; 3];
    for p in centers(&drawn(&world)) {
        let n = (p / Vec3::new(100.0, 50.0, 20.0)).abs();
        assert!(n.max_element() <= 1.0 + 1.0e-4, "{p}");
        let on: Vec<usize> = (0..3).filter(|k| (n[*k] - 1.0).abs() < 1.0e-4).collect();
        assert!(!on.is_empty(), "{p} is on no face");
        faces[on[0]] += 1;
    }
    assert!(faces.iter().all(|f| *f > 15), "every pair of faces is used: {faces:?}");
}

#[test]
fn the_noise_is_perlins_reference() {
    // 0x1443B8F30 with the table at 0x147763A50 = Ken Perlin's improved noise; his reference value.
    assert!((kh_vfx::perlin(3.14, 42.0, 7.0) - 0.136_919_96).abs() < 1.0e-5, "{}", kh_vfx::perlin(3.14, 42.0, 7.0));
    assert_eq!(kh_vfx::perlin(1.0, 2.0, 3.0), 0.0);
    assert!(kh_vfx::perlin(0.5, 0.25, 0.75).abs() <= 1.0);
}

#[test]
fn the_base_scale_and_the_node_scale_reach_points_and_sizes_as_traced() {
    // 0x1442A3E80: world scale = component x chain x SQEXBaseScale multiplies the spawn point
    // (0x1442881B0); the particle size takes the emitter multiplier only through SizeScale /
    // InitSizeScale (own scale x base scale, not the component's: the renderer adds that one).
    let shape = module("ParticleModuleSQEX_Range", json!({"RangeRadius": constant(json!(100.0)), "DirectionAngle": constant(json!(180.0)), "Deviation": constant(json!(1.0)), "DirectionType": "EPARTICLE_DIR_TYPE_NONE"}));
    let node = module("ParticleModuleSQEX_EmitterTransforms", json!({"LocalAddScale": constant(json!([2.0, 2.0, 2.0])), "bCurveParameterDirectScale": true, "LocalAddPosition": constant(json!([10.0, 0.0, 0.0])), "bCurveParameterDirectPosition": true, "bIsUseTransformScale": true}));
    let scaled = module("ParticleModuleSizeScale", json!({"SizeScale": constant(json!([3.0, 3.0, 3.0]))}));
    let json = json!({"name": "s", "system": {"params": {"SQEXBaseScale": {"x": 0.5, "y": 0.5, "z": 0.5}}}, "emitters": [burst_of(16, vec![node, shape, scaled], json!({}))]});
    let mut world = world_with(EffectDef::from_json(&json).unwrap());
    world.spawn("s", Xform { position: Vec3::new(0.0, 0.0, 100.0), scale: Vec3::splat(4.0), ..Xform::IDENTITY }, SpawnOptions::default()).unwrap();
    world.step(DT);
    let list = drawn(&world);
    // World scale 4 x 2 x 0.5 = 4: the node's own offset 10 -> 40, the shell's radius 100 -> 400.
    let origin = Vec3::new(40.0, 0.0, 100.0);
    for q in &list.quads {
        let c = (q.corners[0] + q.corners[2]) * 0.5;
        assert!((c.distance(origin) - 400.0).abs() < 0.5, "{c}");
        // Size 1 x SizeScale 3 x (own 2 x base 0.5), drawn x the component's 4.
        assert!((q.corners[0].distance(q.corners[1]) - 12.0).abs() < 1.0e-3);
    }
}

#[test]
fn a_child_hangs_on_its_parents_matrix_without_its_scale() {
    // 0x144290A60: the child's position goes through the parent's local matrix (rotation and
    // translation); the parent's scale reaches the child's offset only through the child's own
    // world scale with bIsUseTransformScale.
    let child = emitter("kid", "sprite", 1, (0.0, 0.0, 1.0, 1.0), 0.0, json!([{"count": 1, "count_low": -1, "time": 0.0}]), vec![module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(1.0))})), module("ParticleModuleSize", json!({"StartSize": constant(json!([1.0, 1.0, 1.0]))}))], json!({"auto_add": false}));
    let parent = emitter(
        "ctl",
        "sqex_control",
        1,
        (0.0, 0.0, 1.0, 1.0),
        0.0,
        json!([]),
        vec![
            module("ParticleModuleSQEX_EmitterTransforms", json!({"LocalAddRotation": constant(json!([0.0, 0.0, 90.0])), "bCurveParameterDirectRotation": true, "LocalAddScale": constant(json!([5.0, 5.0, 5.0])), "bCurveParameterDirectScale": true, "LocalAddPosition": constant(json!([0.0, 0.0, 30.0])), "bCurveParameterDirectPosition": true})),
            module("ParticleModuleSQEX_Parent", json!({"ChildrenData": [{"GenerationConditions": "ESQEX_CREATE_GENERATE_TYPE_TIME", "GenerateEmitterName": "kid", "MaxGenerateCount": 0, "TimeDatas": [{"GenerateTime": 0.0, "GenerateNumber": 1}], "LoopNumber": 1, "LoopEndTime": 0, "OffsetPosition": {"x": 100, "y": 0, "z": 0}, "OffsetRotation": {"pitch": 0, "yaw": 0, "roll": 0}, "OffsetScale": {"x": 1, "y": 1, "z": 1}, "bUseSpawnModule": false}]})),
        ],
        json!({}),
    );
    let mut world = world_with(effect("family", vec![parent, child]));
    world.spawn("family", Xform::at(Vec3::new(1000.0, 0.0, 0.0)), SpawnOptions::default()).unwrap();
    run(&mut world, 3);
    let c = centers(&drawn(&world))[0];
    // Offset (100, 0, 0) turned by the parent's yaw 90 -> +Y, plus the parent's own (0, 0, 30); not x 5.
    assert!(c.distance(Vec3::new(1000.0, 100.0, 30.0)) < 1.0e-2, "{c}");
}

#[test]
fn a_local_particle_stays_in_the_nodes_frame() {
    // 0x144373420 / 0x14428DA00: location = node origin + its place in the node's frame.
    let def = effect(
        "local",
        vec![burst_of(
            1,
            vec![
                module("ParticleModuleSQEX_EmitterTransforms", json!({"LocalAddPosition": constant(json!([60.0, 0.0, 0.0])), "bCurveParameterDirectPosition": false, "LocalAddRotation": constant(json!([0.0, 0.0, 90.0])), "bCurveParameterDirectRotation": false})),
                module("ParticleModuleLocation", json!({"StartLocation": constant(json!([10.0, 0.0, 0.0]))})),
                module("ParticleModuleSQEX_LocalParticle", json!({"bIsPosition": true, "bIsRotation": false, "bIsLocalRotation": true})),
            ],
            json!({}),
        )],
    );
    let mut world = world_with(def);
    world.spawn("local", Xform::IDENTITY, SpawnOptions::default()).unwrap();
    run(&mut world, 60);
    let c = centers(&drawn(&world))[0];
    // After a second the node has moved 60 cm and turned 90 degrees: the particle sits 10 cm
    // along the node's turned X (= world +Y) from it.
    assert!(c.distance(Vec3::new(60.0, 10.0, 0.0)) < 1.5, "{c}");
}

#[test]
fn a_decal_with_the_scale_rot_flag_has_its_size_in_the_nodes_axes() {
    // 0x144281110: |size x R(DecalRotation)|; (1100, 1100, 500) turned by (0, -90, 0) projects
    // 500 deep (the decal's X) over 1100 x 1100.
    let decal = |flag: bool| {
        let mut e = emitter("d", "sqex_decal", 1, (0.0, 0.0, 1.0, 1.0), 0.0, json!([]), vec![], json!({}));
        e["typedata"]["params"] = json!({"DecalMaterial": "/Game/Effects/test/mat/m_decal.m_decal", "DecalSize": constant(json!([1100.0, 1100.0, 500.0])), "DecalRotation": constant(json!([0.0, -90.0, 0.0])), "bItUseScaleRotMatrixTransfrom": flag});
        let mut world = world_with(effect("decal", vec![e]));
        world.spawn("decal", Xform::IDENTITY, SpawnOptions::default()).unwrap();
        world.step(DT);
        let list = drawn(&world);
        assert_eq!(list.decals.len(), 1);
        let m = list.decals[0].transform.matrix3;
        // The box's X axis is the projection: its world direction and half depth.
        (Vec3::from(m.x_axis), Vec3::from(m.y_axis).length(), Vec3::from(m.z_axis).length())
    };
    let (x, y, z) = decal(true);
    assert!((x.length() - 500.0).abs() < 1.0e-2 && x.normalize().z.abs() > 0.999, "{x}");
    assert!((y - 1100.0).abs() < 1.0e-2 && (z - 1100.0).abs() < 1.0e-2);
    let (x, _, z) = decal(false);
    assert!((x.length() - 1100.0).abs() < 1.0e-2 && (z - 500.0).abs() < 1.0e-2);
}

#[test]
fn a_camera_shake_is_asked_for_once() {
    // 0x1442FC5E0: from ShakePlayTime on, once per instance, with the scale of the emitter time.
    let def = effect("shake", vec![emitter("c", "sqex_control", 1, (0.0, 0.0, 1.0, 1.0), 0.0, json!([]), vec![module("ParticleModuleSQEX_CameraShake", json!({"Shake": "/Game/Effects/x/BP_cam0.BP_cam0_C", "ShakePlayTime": 0.1, "CameraShakeScale": constant(json!(0.5)), "OverrideOscillationDuration": false, "ShakeScaleDistance": 2000.0, "ShakeScaleDistanceOffset": 1000.0, "ShakeScaleDistanceMin": 0.2}))], json!({}))]);
    let mut world = world_with(def);
    world.spawn("shake", Xform::at(Vec3::new(5.0, 0.0, 0.0)), SpawnOptions::default()).unwrap();
    run(&mut world, 5);
    assert!(world.take_shakes().is_empty());
    run(&mut world, 30);
    let shakes = world.take_shakes();
    assert_eq!(shakes.len(), 1);
    assert_eq!((shakes[0].class.as_str(), shakes[0].scale, shakes[0].duration), ("BP_cam0_C", 0.5, None));
    assert_eq!(shakes[0].distance, [2000.0, 1000.0, 0.2]);
    assert!(shakes[0].position.distance(Vec3::new(5.0, 0.0, 0.0)) < 1.0e-3);
}

/// (IMPL_ER53) A held copy: only the kept emitter runs, its clock stops at the hold time (particles neither age nor
/// move, the effect stays), it still follows its component, and a soft stop lets it play out as authored.
#[test]
fn a_held_variant_keeps_its_emitters_frozen_until_stopped() {
    let moving = |name: &str| {
        emitter(
            name,
            "sprite",
            1,
            (0.0, 0.0, 0.1, 0.1),
            0.0,
            json!([{"count": 1, "count_low": -1, "time": 0.0}]),
            vec![
                module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(0.5))})),
                module("ParticleModuleSize", json!({"StartSize": constant(json!([10.0, 10.0, 10.0]))})),
                module("ParticleModuleVelocity", json!({"StartVelocity": constant(json!([0.0, 0.0, 100.0])), "bInWorldSpace": false})),
            ],
            json!({}),
        )
    };
    let base = effect("base", vec![moving("keep"), moving("drop")]);
    let (held, missing) = base.variant("base#held", &["keep", "nothing"], Some(0.2));
    assert_eq!(missing, ["nothing"]);
    assert!(held.emitters[0].enabled && !held.emitters[1].enabled && held.hold == Some(0.2));
    let mut world = world_with(held);
    let id = world.spawn("base#held", Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
    run(&mut world, 12);
    let at_hold = drawn(&world);
    assert_eq!(at_hold.quads.len(), 1, "only the kept emitter");
    let z = |l: &DrawList| (l.quads[0].corners[0] + l.quads[0].corners[2]).z * 0.5;
    // Far past the particle's own life: still there, not moved.
    run(&mut world, 120);
    let later = drawn(&world);
    assert!(world.is_alive(id));
    assert!((z(&later) - z(&at_hold)).abs() < 1e-3, "frozen: {} vs {}", z(&later), z(&at_hold));
    // Stopped softly: its clock runs again and it ends as authored.
    world.stop(id, false);
    run(&mut world, 40);
    assert!(!world.is_alive(id));
}

/// (IMPL_ER75) The trail's newest sample is where the sockets were given this step; a cut drops the history and the
/// next sample starts a new ribbon at the blade.
#[test]
fn a_trail_head_follows_the_sockets_and_a_cut_drops_the_history() {
    let def = effect("trail", vec![emitter("e", "anim_trail", 1, (0.0, 0.05, 0.08, 0.18), 20.0, json!([]), vec![module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(0.2))})), module("ParticleModuleSize", json!({"StartSize": constant(json!([25.0, 25.0, 25.0]))}))], json!({}))]);
    let mut world = world_with(def);
    let id = world.spawn("trail", Xform::IDENTITY, SpawnOptions::default()).unwrap();
    for k in 0..5 {
        let tip = Vec3::new(10.0 * k as f32, 0.0, 100.0);
        world.set_trail(id, tip, tip - Vec3::Z * 60.0);
        world.step(DT);
        let (head, samples) = world.trail_head(id).expect("a ribbon");
        assert!(head.distance(tip) < 1e-3, "the newest sample is at the tip");
        assert_eq!(samples, k + 1);
    }
    world.clear_trail(id);
    let far = Vec3::new(5000.0, 0.0, 100.0);
    world.set_trail(id, far, far - Vec3::Z * 60.0);
    world.step(DT);
    assert_eq!(world.trail_head(id).map(|(_, n)| n), Some(1), "a new ribbon from the cut on");
}
