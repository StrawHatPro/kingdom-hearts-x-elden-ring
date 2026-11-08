//! (VFX-2) `TypeDataSQEX_SkeletalMesh` emitters (kh-vfx def.rs `SkeletalDef`, sim.rs draw_system): one skeletal mesh
//! component per emitter instance on the node, its clip entry, its parent's socket and its material modules, on a
//! hand-written effect in the pack's shape and on the real Ghost Horde afterimage (vw_so050_stf_tsn1) when the pack is on
//! disk.

use std::sync::Arc;

use glam::Vec3;
use kh_vfx::{AnimChange, Camera, DrawList, EffectDef, SkelAnim, SkelInstance, SpawnOptions, World, Xform};
use serde_json::{json, Value};

const DT: f32 = 1.0 / 60.0;

fn constant(v: Value) -> Value {
    json!({"distribution": {"type": "constant", "constant": v}})
}

fn skeletal(name: &str, typedata: Value, modules: Vec<Value>, children: Option<Value>) -> Value {
    let mut modules = modules;
    if let Some(c) = children {
        modules.push(json!({"class": "ParticleModuleSQEX_Parent", "enabled": true, "params": {"ChildrenData": c}}));
    }
    json!({
        "name": name, "enabled": true, "typedata": {"kind": "sqex_skeletalmesh", "params": typedata},
        "required": {
            "material": "/Game/Effects/wep/w_so050/mat/smt_so050_stf_tsn_00k.smt_so050_stf_tsn_00k", "screen_alignment": "PSA_Square",
            "emitter_blend_type": "BLEND_Additive", "use_local_space": false, "kill_on_completed": true, "duration": 1.0, "loops": 1,
            "sqex_emitter_time": {"start_time": 0.0, "loop_time": 0.0, "loop_duration": 1.0, "duration_time": 1.0, "use_legacy_emitter_time": false},
            "sub_uv": {"horizontal": 1, "vertical": 1, "interpolation": "PSUVIM_None"},
            "emitter_origin": {"x": 0, "y": 0, "z": 0}, "emitter_rotation": {"pitch": 0, "yaw": 0, "roll": 0},
            "params": {"bAutoAddToComponent": name != "key", "EmitterBlendType": "BLEND_Additive", "bUseDynamicMaterialInstance": true}
        },
        "spawn": {"enabled": true, "rate": constant(json!(0.0)), "bursts": []},
        "modules": modules
    })
}

fn drawn(world: &World) -> DrawList {
    let mut list = DrawList::default();
    world.draw(1.0, &Camera::DEFAULT, &mut list);
    list
}

#[test]
fn a_skeletal_emitter_draws_one_posed_mesh_with_its_material_modules_and_its_child_in_its_socket() {
    let body = skeletal(
        "body",
        json!({"SkeletalMesh": "/Game/Character/pc/p_ex001/mdl/p_ex001_0.p_ex001_0", "bOverrideMaterial": true, "RotaionOffset": {"pitch": 0, "yaw": 90, "roll": 0},
               "AnimationData": [{"AnimChangeType": "SQEX_SKELETAL_MESH_PARTICLE_ANIM_CHANGE_TYPE_LOOP", "AnimSequence": "/Game/Character/pc/p_ex001/anm/rt/body/1316.1316", "bStopAnimation": true, "StopTime": 0.7}]}),
        vec![
            json!({"class": "ParticleModuleSQEX_Material_Scalar", "enabled": true, "params": {"ParamName": "alpha", "MaterialScalarParam": constant(json!(0.25))}}),
            json!({"class": "ParticleModuleSQEX_Material_Vector", "enabled": true, "params": {"ParamName": "Col_Shift", "MaterialVectorParam": constant(json!([0.6, 1.2, 2.0])), "MaterialAlphaParam": constant(json!(0.5))}}),
        ],
        Some(json!([{"GenerateEmitterName": "key", "GenerationConditions": "ESQEX_CREATE_GENERATE_TYPE_TIME", "TimeDatas": [{"GenerateTime": 0.0, "GenerateNumber": 1}], "MaxGenerateCount": 0, "LoopNumber": 1}])),
    );
    let key = skeletal("key", json!({"SkeletalMesh": "/Game/Character/wep/w_so050/mdl/w_so050_0.w_so050_0", "bOverrideMaterial": true, "bAttachParentObject": true, "AttachBone": "R_wep_skt"}), vec![], None);
    let def = EffectDef::from_json(&json!({"name": "skel", "system": {"params": {}}, "emitters": [body, key]})).unwrap();
    assert!(def.unknown().is_empty(), "the type data is evaluated: {:?}", def.unknown());
    let mut world = World::new();
    world.register(def);
    world.spawn("skel", Xform::at(Vec3::new(100.0, 0.0, 0.0)), SpawnOptions::default()).unwrap();
    for _ in 0..30 {
        world.step(DT);
    }
    let list = drawn(&world);
    assert!(list.quads.is_empty() && list.meshes.is_empty(), "its particles draw nothing");
    assert_eq!(list.skeletal.len(), 2, "{:?}", list.skeletal);
    let (b, k) = (&list.skeletal[0], &list.skeletal[1]);
    assert_eq!((b.mesh.as_str(), b.parent.clone(), b.blend), ("p_ex001_0", None, kh_vfx::Blend::Additive));
    assert_eq!(world.material_name(b.material), Some("smt_so050_stf_tsn_00k"));
    // on the node, turned 90 degrees by RotaionOffset (its X axis now along world +Y)
    assert!((Vec3::from(b.transform.translation) - Vec3::new(100.0, 0.0, 0.0)).length() < 1.0e-3);
    assert!((Vec3::from(b.transform.matrix3.x_axis) - Vec3::Y).length() < 1.0e-4, "{:?}", b.transform);
    // the clip held at its StopTime, whatever its length
    assert_eq!(b.clip_at(&|_| Some(3.0)), Some(("1316".to_string(), 0.7)));
    // the material modules at the emitter time, the vector with its alpha
    let alpha = b.params.iter().find(|p| p.0 == "alpha").expect("alpha");
    assert!((alpha.1[0] - 0.25).abs() < 1.0e-6 && !alpha.2, "{alpha:?}");
    assert_eq!(b.params.iter().find(|p| p.0 == "Col_Shift").map(|p| (p.1, p.2)), Some(([0.6, 1.2, 2.0, 0.5], true)));
    // the child: the keyblade in the body's R_wep_skt, no clip
    assert_eq!((k.mesh.as_str(), k.parent.clone()), ("w_so050_0", Some((0, "R_wep_skt".to_string()))));
    assert_eq!(k.clip_at(&|_| Some(1.0)), None);
}

#[test]
fn anim_end_entries_hand_over_when_their_clip_has_run_and_the_last_wraps_to_the_first() {
    let a = |clip: &str, change: AnimChange| SkelAnim { clip: Some(clip.to_string()), change, stop: None };
    let mut s = SkelInstance { mesh: "m".into(), material: 0, transform: glam::Affine3A::IDENTITY, parent: None, anims: Arc::from(vec![a("A", AnimChange::AnimEnd), a("B", AnimChange::AnimEnd)]), anim: 0, anim_time: 0.5, owner_pose: false, params: Vec::new(), blend: kh_vfx::Blend::Material, time: 0.0 };
    let len = |c: &str| Some(if c == "A" { 1.0 } else { 2.0 });
    assert_eq!(s.clip_at(&len), Some(("A".to_string(), 0.5)));
    s.anim_time = 1.25;
    assert_eq!(s.clip_at(&len), Some(("B".to_string(), 0.25)));
    s.anim_time = 3.5;
    assert_eq!(s.clip_at(&len), Some(("A".to_string(), 0.5)));
    // a LOOP entry loops its own clip; an unknown length plays on unwrapped
    s.anims = Arc::from(vec![a("A", AnimChange::Loop)]);
    s.anim_time = 2.25;
    assert_eq!(s.clip_at(&len), Some(("A".to_string(), 0.25)));
    assert_eq!(s.clip_at(&|_| None), Some(("A".to_string(), 2.25)));
}

/// The real Ghost Horde afterimage (Favorite Deputy's shotlock), when the keyblades pack is on disk: Sora's body p_ex001_0
/// held at 1316's 0.7 s with smt_so050_stf_tsn_00k (additive), its Favorite Deputy w_so050_0 in the body's R_wep_skt, and the
/// body's alpha / Col_Shift / bdy0_wep1 / Object Height drives.
#[test]
fn the_ghost_horde_afterimage_is_a_held_body_with_its_keyblade() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/keyblades/vfx/effects/vw_so050_stf_tsn1.json");
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    let def = EffectDef::parse(&text).unwrap();
    assert!(!def.unknown().iter().any(|(_, m)| m.contains("sqex_skeletalmesh")));
    let mut world = World::new();
    world.register(def);
    world.spawn("vw_so050_stf_tsn1", Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
    let mut seen = Vec::new();
    for _ in 0..12 {
        world.step(DT);
        let list = drawn(&world);
        if let Some(b) = list.skeletal.iter().find(|s| s.mesh == "p_ex001_0") {
            seen.push(b.params.iter().find(|p| p.0 == "alpha").map_or(-1.0, |p| p.1[0]));
            assert_eq!(b.clip_at(&|_| Some(2.0)), Some(("1316".to_string(), 0.7)));
            assert_eq!(world.material_name(b.material), Some("smt_so050_stf_tsn_00k"));
            assert_eq!(b.params.iter().find(|p| p.0 == "bdy0_wep1").map(|p| p.1[0]), Some(0.0));
            if let Some(k) = list.skeletal.iter().find(|s| s.mesh == "w_so050_0") {
                assert_eq!(k.parent.as_ref().map(|p| p.1.as_str()), Some("R_wep_skt"));
                assert_eq!(k.params.iter().find(|p| p.0 == "bdy0_wep1").map(|p| p.1[0]), Some(1.0));
            }
        }
    }
    assert!(!seen.is_empty(), "the body is drawn");
    assert!(seen.windows(2).any(|w| w[1] > w[0]), "its alpha rises over the first tenth of a second: {seen:?}");
}
