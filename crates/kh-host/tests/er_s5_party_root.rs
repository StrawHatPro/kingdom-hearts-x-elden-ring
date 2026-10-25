//! (ER party S5) THE PARTY'S SECOND EFFECT ROOT in the effect side kh-er runs (`kh_host::vfx::EffectHost`):
//! - the party's effects come from PACK/party/vfx, their own materials / meshes named `party:<name>` (the renderer's
//!   second table), the main pack's names never (the main shading table stays the only one Sora's effects see);
//! - `Attach::Party` sits on the member's bones of the step (`set_party`), the party's spell actors' body effects on the
//!   actors;
//! - Sora's "a new core counts from 1" rule and his animation change never end the party's effects (before S5 the next
//!   spawn of Sora's after one of the party's ended every effect alive);
//! - a member's endless one-shot ends with his anim-set row; `party_loops` counts what still runs; `stop_party`.
//! Skipped, saying so, without the packs.

use std::collections::HashMap;
use std::path::PathBuf;

use glam::{Quat, Vec3};
use kh_core::host::party::{PartyMember, SeamHandles};
use kh_core::{Attach, EffectHandle, EffectId, EffectSpawn};
use kh_host::audio::PoseTap;
use kh_host::seam::EffectCommand;
use kh_host::vfx::{EffectHost, PartyMemberFx, Scene, PARTY_NAMESPACE};

fn project() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn spawn(name: &'static str, at: Attach, persistent: bool) -> EffectSpawn {
    EffectSpawn { id: EffectId(0), name, at, rotation: Vec3::ZERO, scale: 1.0, persistent }
}

#[test]
fn er_s5_the_party_root_is_a_second_namespace_and_its_effects_live_apart_from_soras() {
    let pack = project().join("build").join("sora_pack");
    let loadout = project().join("sheets").join("port").join("sora-loadout-99.json");
    if !pack.join("vfx").join("effects").is_dir() || !pack.join("party").join("vfx").join("effects").is_dir() {
        eprintln!("SKIPPED: no effect packs under {}", pack.display());
        return;
    }
    let rig = kh_host::rig::Rig::new(&pack, &loadout, None).expect("Sora's rig");
    let mut host = EffectHost::load(&pack).expect("the main effect pack");
    // the main pack alone knows no party effect
    let party_effect = "vn_ex001_fir_cal00";
    let (main_materials_before, _) = host.assets();
    assert!(!host.has_party_root());
    let line = host.load_party_root(&pack).expect("the party root");
    assert!(line.contains("effects"), "{line}");
    let parsed = host.preload_party();
    assert!(parsed >= 90, "{parsed} party effects parsed");
    let (materials, meshes) = host.assets();
    // the party's own materials / meshes carry the namespace; every main material keeps its name
    let party_materials: Vec<&String> = materials.iter().map(|m| &m.0).filter(|n| n.starts_with(PARTY_NAMESPACE)).collect();
    assert!(!party_materials.is_empty(), "the party root's own materials are named party:<name>");
    for m in &party_materials {
        let bare = m.strip_prefix(PARTY_NAMESPACE).unwrap();
        assert!(pack.join("party").join("vfx").join("materials").join(format!("{bare}.json")).is_file(), "{m}: its file is in the party root");
        assert!(!pack.join("vfx").join("materials").join(format!("{bare}.json")).is_file(), "{m}: the main pack has no such material");
    }
    for (name, _, _) in materials.iter().filter(|m| !m.0.starts_with(PARTY_NAMESPACE)) {
        assert!(pack.join("vfx").join("materials").join(format!("{name}.json")).is_file() || main_materials_before.iter().any(|m| &m.0 == name), "{name}: a main name is a main material");
    }
    assert!(meshes.iter().any(|m| m.starts_with(PARTY_NAMESPACE)), "the party root's own meshes are named party:<name>");
    // the main shading table is untouched (sha 2dd1d564: the user's working effects)
    let main_table = std::fs::read(pack.join("vfx").join("shaders").join("shading.json")).unwrap();
    assert!(main_table.len() == 811_711, "PACK/vfx/shaders/shading.json changed size: {}", main_table.len());

    // every effect the party's tables name (sheet party-effects: notifies, spell actors, team link pawns; the spell actor
    // and team classes of party_host.json / team_host.json) resolves in the main pack or the party root
    let sheet: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(project().join("sheets/port/party-effects.json")).unwrap()).unwrap();
    let mut missing = Vec::new();
    let mut names: Vec<String> = sheet["rows"].as_array().unwrap().iter().filter_map(|r| r[0].as_str().map(str::to_string)).collect();
    for file in [pack.join("party/vfx/party_host.json"), pack.join("party/team/team_host.json")] {
        let j: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
        for roles in j["projectiles"].as_object().unwrap().values() {
            for list in roles.as_object().unwrap().values() {
                names.extend(list.as_array().unwrap().iter().filter_map(|n| n.as_str().map(str::to_string)));
            }
        }
    }
    for n in names.iter().filter(|n| !n.ends_with("_C")) {
        if !host.has_effect(n) {
            missing.push(n.clone());
        }
    }
    println!("{} party effect names checked, missing: {missing:?}", names.len());
    assert!(missing.is_empty(), "party effects missing from both roots: {missing:?}");

    // ---- a step with Donald posed: his effect on his staff socket, Sora's effects beside it
    let mut bones = HashMap::new();
    bones.insert("wep:vfx_call".to_string(), (Vec3::new(130.0, -200.0, 150.0), Quat::IDENTITY));
    let donald = PartyMemberFx { position: Vec3::new(100.0, -200.0, 0.0), yaw: 0.0, bones, anim: "MagicFire".to_string() };
    host.set_party(vec![(0, donald.clone())], Vec::new());
    let mut handles = SeamHandles::for_member(PartyMember::DONALD);
    let (cast, persistent) = (handles.effect(), handles.effect());
    let tap = PoseTap::default();
    let none = |_: kh_core::TargetId| None;
    let scene = Scene { out: &rig.out, sora: &rig.sora, tap: &tap, target: &none, ground: Some(0.0), eye: Vec3::new(-500.0, 0.0, 200.0) };
    let commands = vec![
        EffectCommand::Spawn(EffectHandle(5), spawn("vm_cur_hed0", Attach::World(Vec3::new(0.0, 0.0, 100.0)), false)),
        EffectCommand::Spawn(cast, spawn(party_effect, Attach::Party { member: PartyMember::DONALD, bone: Some("wep:vfx_call"), offset: Vec3::ZERO }, false)),
        EffectCommand::Spawn(persistent, spawn("vn_ex001_fir_blt00", Attach::Party { member: PartyMember::DONALD, bone: None, offset: Vec3::Z * 100.0 }, true)),
    ];
    host.step(1.0 / 60.0, &commands, &scene);
    let live = host.world.live();
    let cal = live.iter().find(|l| l.name.eq_ignore_ascii_case(party_effect)).expect("Donald's cast effect is alive");
    let probe = host.world.probe(cal.id);
    let near = probe.iter().map(|e| e.position.distance(Vec3::new(130.0, -200.0, 150.0))).fold(f32::MAX, f32::min);
    assert!(near < 60.0, "on his staff socket (nearest emitter node {near:.1} cm away): {probe:?}");
    assert_eq!(host.party_loops(), vec![(0, 1)], "the persistent effect is Donald's running loop");
    // a new Sora spawn with a LOWER handle (a new core of Sora's): Sora's old effects end, the party's stay
    host.step(1.0 / 60.0, &[EffectCommand::Spawn(EffectHandle(1), spawn("vm_cur_hed0", Attach::World(Vec3::ZERO), false))], &scene);
    assert_eq!(host.party_loops(), vec![(0, 1)], "Sora's new core does not end the party's effects");
    assert!(host.world.live().iter().any(|l| l.name.eq_ignore_ascii_case(party_effect)));
    // the core's End (the notify state's): the loop is over
    host.step(1.0 / 60.0, &[EffectCommand::Stop(persistent, false)], &scene);
    assert_eq!(host.party_loops(), Vec::<(u8, usize)>::new(), "after the move no party loop runs");
    // stop_party: everything the party started ends
    let n = host.stop_party(true);
    assert!(n >= 1, "{n}");
    host.step(1.0 / 60.0, &[], &scene);
    assert!(!host.world.live().iter().any(|l| l.name.eq_ignore_ascii_case(party_effect)));
}
