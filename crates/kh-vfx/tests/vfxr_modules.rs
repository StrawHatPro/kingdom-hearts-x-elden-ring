//! (VFXR) The modules stream VFXR added to the evaluator, on small hand-written effects in the pack's shape: the maths of
//! each (kh-vfx def.rs / vfxr_modules.rs / sim.rs / screen.rs), and the real pack's users of each when it is on disk.

use glam::Vec3;
use kh_vfx::{Camera, DrawList, EffectDef, ScreenKind, SpawnOptions, World, Xform};
use serde_json::{json, Value};

const DT: f32 = 1.0 / 60.0;

fn constant(v: Value) -> Value {
    json!({"distribution": {"type": "constant", "constant": v}})
}

fn module(class: &str, params: Value) -> Value {
    json!({"class": class, "enabled": true, "params": params})
}

fn emitter(name: &str, kind: &str, rate: f32, bursts: Value, modules: Vec<Value>) -> Value {
    json!({
        "name": name, "enabled": true, "typedata": {"kind": kind},
        "required": {
            "material": "/Game/Effects/test/mat/m_test.m_test", "screen_alignment": "PSA_Square", "use_local_space": false,
            "kill_on_completed": false, "duration": 10.0, "loops": 1,
            "sqex_emitter_time": {"start_time": 0.0, "loop_time": 0.0, "loop_duration": 10.0, "duration_time": 10.0, "use_legacy_emitter_time": false},
            "sub_uv": {"horizontal": 1, "vertical": 1, "interpolation": "PSUVIM_None"},
            "use_max_draw_count": true, "max_draw_count": 500,
            "emitter_origin": {"x": 0, "y": 0, "z": 0}, "emitter_rotation": {"pitch": 0, "yaw": 0, "roll": 0},
            "params": {"bAutoAddToComponent": true, "RotationPriority": "ROTATION_PRIORITY_XZY"}
        },
        "spawn": {"enabled": true, "process_spawn_rate": true, "process_burst_list": true, "rate": constant(json!(rate)), "rate_scale": constant(json!(1.0)), "burst_scale": constant(json!(1.0)), "bursts": bursts},
        "modules": modules
    })
}

fn effect(name: &str, emitters: Vec<Value>) -> EffectDef {
    EffectDef::from_json(&json!({"name": name, "system": {"params": {}}, "emitters": emitters})).unwrap()
}

fn drawn(world: &World) -> DrawList {
    let mut list = DrawList::default();
    world.draw(1.0, &Camera::DEFAULT, &mut list);
    list
}

fn lifetime(s: f32) -> Value {
    module("ParticleModuleLifetime", json!({"Lifetime": constant(json!(s))}))
}
fn size(s: f32) -> Value {
    module("ParticleModuleSize", json!({"StartSize": constant(json!([s, s, s]))}))
}
fn velocity(v: [f32; 3]) -> Value {
    module("ParticleModuleVelocity", json!({"StartVelocity": constant(json!(v)), "bInWorldSpace": true}))
}
fn location(v: [f32; 3]) -> Value {
    module("ParticleModuleLocation", json!({"StartLocation": constant(json!(v))}))
}

fn run_one(def: EffectDef, at: Vec3, steps: usize, ground: Option<f32>) -> (World, DrawList) {
    let name = def.name.clone();
    let mut world = World::new();
    world.set_ground(ground);
    world.register(def);
    world.spawn(&name, Xform::at(at), SpawnOptions::default()).unwrap();
    for _ in 0..steps {
        world.step(DT);
    }
    let list = drawn(&world);
    (world, list)
}

#[test]
fn sphere_force_internal_keeps_the_particles_in_its_ball_and_scales_them_once() {
    let sf = module("ParticleModuleSQEX_SphereForce", json!({"RadiusScale": constant(json!(1.0)), "CollPos": constant(json!([0, 0, 0])), "ForceData": {"HitParticleSizeScale": 0.5, "ForceRadius": 50, "ForceType": "SQEX_PARTICLE_SPHERE_COLL_HIT_TYPE_REFLECTION_INTERNAL"}}));
    let def = effect("sf", vec![emitter("e", "sprite", 0.0, json!([{"count": 8, "count_low": -1, "time": 0.0}]), vec![lifetime(5.0), size(10.0), velocity([400.0, 0.0, 0.0]), sf])]);
    let (_, list) = run_one(def, Vec3::new(0.0, 0.0, 100.0), 60, None);
    assert_eq!(list.quads.len(), 8);
    for q in &list.quads {
        assert!(q.center.distance(Vec3::new(0.0, 0.0, 100.0)) <= 50.0 + 400.0 * DT + 0.01, "{:?}", q.center);
        assert!((q.size - 5.0).abs() < 0.01, "size {}", q.size);
    }
}

#[test]
fn attractor_point_gravity_pulls_toward_its_point_inside_its_radius() {
    let g = module("ParticleModuleAttractorPointGravity", json!({"StrengthRaw": constant(json!(600.0)), "Radius": 150, "Position": {"x": 0, "y": 0, "z": 0}}));
    let near = effect("g", vec![emitter("e", "sprite", 0.0, json!([{"count": 1, "count_low": -1, "time": 0.0}]), vec![lifetime(5.0), size(10.0), location([100.0, 0.0, 0.0]), g.clone()])]);
    let (_, list) = run_one(near, Vec3::ZERO, 20, None);
    // a = 600 cm/s^2 toward the node for 1/3 s: about 33 cm closer
    let x = list.quads[0].center.x;
    assert!(x < 75.0 && x > 55.0, "x {x}");
    let far = effect("g2", vec![emitter("e", "sprite", 0.0, json!([{"count": 1, "count_low": -1, "time": 0.0}]), vec![lifetime(5.0), size(10.0), location([300.0, 0.0, 0.0]), g])]);
    let (_, list) = run_one(far, Vec3::ZERO, 20, None);
    assert!((list.quads[0].center.x - 300.0).abs() < 1.0e-3);
}

#[test]
fn attractor_point_strength_by_distance() {
    let a = module("ParticleModuleAttractorPoint", json!({"Negative_Z": true, "Negative_Y": true, "Negative_X": true, "Positive_Z": true, "Positive_Y": true, "Positive_X": true, "bUseWorldSpacePosition": false, "bOverrideVelocity": false, "bAffectBaseVelocity": true, "StrengthByDistance": false, "Strength": constant(json!(300.0)), "Range": constant(json!(400.0)), "Position": constant(json!([0.0, 0.0, 0.0]))}));
    let def = effect("ap", vec![emitter("e", "sprite", 0.0, json!([{"count": 1, "count_low": -1, "time": 0.0}]), vec![lifetime(5.0), size(10.0), location([200.0, 0.0, 0.0]), a])]);
    let (_, list) = run_one(def, Vec3::ZERO, 30, None);
    assert!(list.quads[0].center.x < 200.0 - 30.0, "{}", list.quads[0].center.x);
}

#[test]
fn a_collision_kill_sends_a_death_event_and_the_receiver_spawns_there() {
    let rain = emitter("ame", "sprite", 0.0, json!([{"count": 1, "count_low": -1, "time": 0.0}]), vec![
        lifetime(5.0), size(10.0), velocity([0.0, 0.0, -600.0]),
        module("ParticleModuleCollision", json!({"MaxCollisions": constant(json!(0.0)), "DampingFactor": constant(json!([0, 0, 0])), "CollisionCompletionOption": "EPCC_Kill"})),
        module("ParticleModuleEventGenerator", json!({"Events": [{"Type": "EPET_Death", "CustomName": "sbk"}]})),
    ]);
    let splash = emitter("sibuki", "sprite", 0.0, json!([]), vec![lifetime(1.0), size(20.0), module("ParticleModuleEventReceiverSpawn", json!({"EventName": "sbk", "EventGeneratorType": "EPET_Any", "SpawnCount": constant(json!(3.0)), "bInheritVelocity": false}))]);
    let def = effect("rain", vec![rain, splash]);
    let (world, list) = run_one(def, Vec3::new(0.0, 0.0, 100.0), 20, Some(0.0));
    // the drop fell 600 cm/s: it reached the ground at 1/6 s, died, and three splashes were born there
    assert_eq!(list.quads.len(), 3, "{:?}", world.live());
    for q in &list.quads {
        assert!(q.center.z.abs() < 15.0, "{:?}", q.center);
    }
}

#[test]
fn a_collision_bounces_with_damping_until_its_count() {
    let def = effect("bounce", vec![emitter("e", "sprite", 0.0, json!([{"count": 1, "count_low": -1, "time": 0.0}]), vec![
        lifetime(5.0), size(10.0), velocity([0.0, 0.0, -300.0]),
        module("ParticleModuleCollision", json!({"MaxCollisions": constant(json!(5.0)), "DampingFactor": constant(json!([0.5, 0.5, 0.5])), "CollisionCompletionOption": "EPCC_Kill"})),
    ])]);
    let (_, list) = run_one(def, Vec3::new(0.0, 0.0, 50.0), 30, Some(0.0));
    assert_eq!(list.quads.len(), 1);
    // it hit at 1/6 s and goes up at 150 cm/s since
    assert!(list.quads[0].center.z > 0.0 && list.quads[0].center.z < 60.0, "{:?}", list.quads[0].center);
}

#[test]
fn collision_gpu_bounces_off_the_ground_with_its_resilience() {
    let def = effect("gpu", vec![emitter("e", "sprite", 0.0, json!([{"count": 1, "count_low": -1, "time": 0.0}]), vec![
        lifetime(5.0), size(2.0), velocity([0.0, 0.0, -600.0]),
        module("ParticleModuleCollisionGPU", json!({"Response": "EParticleCollisionResponse::Bounce", "RadiusBias": 0, "RadiusScale": 1, "Friction": 0, "ResilienceScaleOverLife": constant(json!(1.0)), "Resilience": constant(json!(0.5))})),
    ])]);
    let (_, list) = run_one(def, Vec3::new(0.0, 0.0, 30.0), 12, Some(0.0));
    // it reached the ground (+ its 1 cm radius) at about 0.05 s and went back up at half its speed: 300 cm/s for 0.15 s
    assert!(list.quads[0].center.z > 25.0 && list.quads[0].center.z < 60.0, "{:?}", list.quads[0].center);
}

#[test]
fn spawn_per_unit_follows_the_travel() {
    let def = effect("trail", vec![emitter("e", "sprite", 0.0, json!([]), vec![lifetime(5.0), size(5.0), module("ParticleModuleSpawnPerUnit", json!({"bIgnoreMovementAlongZ": false, "bIgnoreMovementAlongY": false, "bIgnoreMovementAlongX": false, "MaxFrameDistance": 0, "MovementTolerance": 1, "bIgnoreSpawnRateWhenMoving": false, "SpawnPerUnit": constant(json!(3.0)), "UnitScalar": 100}))])]);
    let mut world = World::new();
    world.register(def);
    let id = world.spawn("trail", Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
    world.step(DT);
    for k in 1..=30 {
        world.set_transform(id, Xform::at(Vec3::new(k as f32 * 20.0, 0.0, 0.0)));
        world.step(DT);
    }
    // 600 cm at 3 per 100 cm: 18 particles, laid along the way
    let list = drawn(&world);
    assert!((list.quads.len() as i32 - 18).abs() <= 1, "{}", list.quads.len());
    let xs: Vec<f32> = list.quads.iter().map(|q| q.center.x).collect();
    assert!(xs.iter().cloned().fold(f32::MAX, f32::min) < 100.0 && xs.iter().cloned().fold(0.0, f32::max) > 500.0);
}

#[test]
fn size_scale_by_speed_and_particle_location() {
    let def = effect("sbs", vec![emitter("e", "sprite", 0.0, json!([{"count": 1, "count_low": -1, "time": 0.0}]), vec![lifetime(1.0), size(10.0), velocity([200.0, 0.0, 0.0]), module("ParticleModuleSizeScaleBySpeed", json!({"MaxScale": {"x": 3.0, "y": 3.0}, "SpeedScale": {"x": 0.01, "y": 0.01}}))])]);
    let (_, list) = run_one(def, Vec3::ZERO, 5, None);
    // 200 cm/s x 0.01 = 2 (under the cap 3): a square sprite (it reads X) twice its size
    let q = list.quads[0];
    let w = (q.corners[1] - q.corners[0]).length();
    assert!((w - 20.0).abs() < 0.2, "{w}");
    let loc = effect("pl", vec![emitter("e", "sprite", 0.0, json!([{"count": 1, "count_low": -1, "time": 0.0}]), vec![lifetime(1.0), size(10.0), module("ParticleModuleSQEX_ParticleLocation", json!({"bApplyOwnerScale": true, "bInWorldSpace": false, "EnableZ": true, "EnableY": true, "EnableX": true, "Location": {"distribution": {"type": "constant", "lookup_table": {"op": 1, "entry_count": 2, "entry_stride": 3, "sub_entry_stride": 0, "time_scale": 1.0, "time_bias": 0.0, "lock_flag": 0, "values": [0, 0, 150, 0, 0, 0]}}}, "StartScale": constant(json!([1, 1, 1]))}))])]);
    let (_, list) = run_one(loc, Vec3::ZERO, 30, None);
    // half way through its life: 75 cm up
    assert!((list.quads[0].center.z - 75.0).abs() < 4.0, "{:?}", list.quads[0].center);
}

#[test]
fn tex_spawn_puts_particles_on_the_baked_pixels() {
    let ts = module("ParticleModuleSQEX_TexSpawn", json!({"SpawnMaxSizeX": 200, "SpawnMaxSizeY": 200, "TexScaleU": 1, "TexScaleV": 1, "OnCenterTextrue": true, "RotatorOffset": {"pitch": 0, "yaw": 0, "roll": 0}, "LocationOffset": {"x": 0, "y": 0, "z": 0}, "DirectionType": "EPARTICLE_DIR_TYPE_NONE", "NormalDirection": 0, "Direction": {"pitch": 0, "yaw": 0, "roll": 0}, "kh_points": {"points": [[0.0, 0.5, 1, 1, 1, 1], [1.0, 0.5, 1, 1, 1, 1]]}}));
    let def = effect("tex", vec![emitter("e", "sprite", 0.0, json!([{"count": 40, "count_low": -1, "time": 0.0}]), vec![lifetime(5.0), size(5.0), ts])]);
    let (_, list) = run_one(def, Vec3::ZERO, 2, None);
    assert_eq!(list.quads.len(), 40);
    // the two lit pixels: the left and right edge of the 200 cm square (texture right = +Y)
    assert!(list.quads.iter().all(|q| (q.center.y.abs() - 100.0).abs() < 1.0e-3 && q.center.x.abs() < 1.0e-3));
    assert!(list.quads.iter().any(|q| q.center.y > 0.0) && list.quads.iter().any(|q| q.center.y < 0.0));
}

#[test]
fn a_radial_blur_emitter_asks_for_its_pass_and_an_in_light_drives_the_character() {
    let rb = module("ParticleModuleSQEX_RadialBlur", json!({"HoleSize": constant(json!(0.0)), "AttenuationDistanceInside": constant(json!(0.0)), "Range": constant(json!(2.0)), "AttenuationDistanceOutside": constant(json!(0.0)), "PosX": constant(json!(0.5)), "PosY": constant(json!(0.5)), "bUseScreenPos": false, "Alpha": constant(json!(0.5)), "Angle": constant(json!(90.0)), "Power": constant(json!(0.01)), "Mode": "SQEX_RADIAL_BLUR_MODE_RADIAL", "EffectPriority": "ScreenEffectPriority_Normal", "EndTime": 1.0, "StartTime": 0.0}));
    let il = module("ParticleModuleSQEX_Material_InLight", json!({"InLightParam": constant(json!(10.0)), "InLightColor": constant(json!([1.0, 0.7, 0.5])), "InLightOutColor": constant(json!([1.0, 0.2, 0.0])), "InLightCenterBounds": constant(json!(3.0)), "ParamType": "SQEX_IN_LIGHT_TYPE_MAIN", "NoSetUseLastParameter": false, "ParamName": "InLight"}));
    let def = effect("rb", vec![emitter("c", "sqex_control", 0.0, json!([]), vec![rb, il])]);
    let (mut world, list) = run_one(def, Vec3::new(500.0, 0.0, 0.0), 10, None);
    assert_eq!(list.screen.len(), 1);
    let fx = list.screen[0];
    assert_eq!(fx.kind, ScreenKind::RadialBlur);
    assert_eq!(fx.world, Some(Vec3::new(500.0, 0.0, 0.0)));
    assert!((fx.alpha - 0.5).abs() < 1.0e-5);
    assert!(list.char_params.iter().any(|p| p.name == "InLight" && p.value[0] == 10.0));
    assert!(list.char_params.iter().any(|p| p.name == "InLightCenterPosition" && (p.value[0] - 500.0).abs() < 1.0e-3));
    // past its EndTime the pass is gone, the drive stays while the emitter lives
    for _ in 0..60 {
        world.step(DT);
    }
    assert!(drawn(&world).screen.is_empty());
}

/// Every effect of the real pack that uses one of VFXR's modules parses with it (no "not evaluated" left for them) and
/// runs a second without a panic or a runaway value (when the pack is on disk).
#[test]
fn the_packs_users_run() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/vfx/effects");
    let Ok(dir) = std::fs::read_dir(&root) else { return };
    let watched = ["SQEX_SphereForce", "AttractorPoint", "Collision", "SpawnPerUnit", "SizeScaleBySpeed", "AccelerationOverLifetime", "EventGenerator", "SQEX_ParticleLocation", "SQEX_RadialBlur", "SQEX_Filter", "SQEX_Material_InLight", "SQEX_GaussBlur", "VectorFieldGlobal", "SQEX_AnyVertexSpawn"];
    let mut ran = 0;
    for f in dir.flatten() {
        let text = std::fs::read_to_string(f.path()).unwrap_or_default();
        if !watched.iter().any(|w| text.contains(w)) {
            continue;
        }
        let def = EffectDef::parse(&text).unwrap();
        for (emitter, module) in def.unknown() {
            assert!(!watched.iter().any(|w| module == *w), "{}: {emitter}: {module} still skipped", def.name);
        }
        let name = def.name.clone();
        let mut world = World::new();
        world.set_ground(Some(0.0));
        world.register(def);
        world.spawn(&name, Xform::at(Vec3::new(0.0, 0.0, 100.0)), SpawnOptions::default()).unwrap();
        for _ in 0..60 {
            world.step(DT);
        }
        let list = drawn(&world);
        assert!(list.quads.iter().all(|q| q.center.is_finite() && q.size.is_finite()), "{name}");
        ran += 1;
    }
    assert!(ran > 20, "{ran}");
}
