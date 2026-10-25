//! (ER90, XSI request 1) THE XS ABILITIES' EFFECT ROOT in the effect side kh-er runs (`kh_host::vfx::EffectHost`):
//! - the XS effects come from PACK/xsi/vfx, their own materials / meshes named `xsi:<name>` (the renderer's third table),
//!   the main pack's names never;
//! - the effect-group rule's name filter lets the XS form effects through only with the root read (without it: as before);
//! - every effect the XS clips (xsi_manifest.json) and the XS code name resolves in one of the roots.
//! Uses ER90's hard-linked harness pack (build/sora_tmp/er/er90/hpack: the main pack + the staged XSI pack); skipped,
//! saying so, without it.

use std::path::PathBuf;

use glam::Vec3;
use kh_core::{Attach, EffectHandle, EffectId, EffectSpawn};
use kh_host::audio::PoseTap;
use kh_host::seam::EffectCommand;
use kh_host::vfx::{EffectHost, Scene, PARTY_NAMESPACE, XSI_NAMESPACE};

fn project() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn spawn(name: &'static str) -> EffectSpawn {
    EffectSpawn { id: EffectId(0), name, at: Attach::World(Vec3::new(200.0, 0.0, 50.0)), rotation: Vec3::ZERO, scale: 1.0, persistent: false }
}

/// Every effect a step of these spawns started (lower-case names).
fn started(host: &mut EffectHost, rig: &kh_host::rig::Rig, names: &[&'static str]) -> Vec<String> {
    let tap = PoseTap::default();
    let none = |_: kh_core::TargetId| None;
    let scene = Scene { out: &rig.out, sora: &rig.sora, tap: &tap, target: &none, ground: Some(0.0), eye: Vec3::new(-500.0, 0.0, 200.0) };
    let commands: Vec<EffectCommand> = names.iter().enumerate().map(|(k, n)| EffectCommand::Spawn(EffectHandle(10 + k as u32), spawn(n))).collect();
    host.step(1.0 / 60.0, &commands, &scene);
    host.world.live().iter().map(|l| l.name.to_lowercase()).collect()
}

#[test]
fn er90_the_xsi_root_is_a_third_namespace_and_opens_the_filter_for_the_xs_effects() {
    let pack = project().join("build").join("sora_tmp").join("er").join("er90").join("hpack");
    let loadout = project().join("sheets").join("port").join("sora-loadout-99.json");
    if !pack.join("vfx").join("effects").is_dir() || !pack.join("xsi").join("vfx").join("effects").is_dir() {
        eprintln!("SKIPPED: no harness pack with the XSI root at {}", pack.display());
        return;
    }
    let rig = kh_host::rig::Rig::new(&pack, &loadout, None).expect("Sora's rig");
    let xs = ["vw_so052_bst1_1", "vw_so088_fin_zmn0", "vw_so192_trf_fns0", "vw_so200_spr_cal00", "vw_so072_stf_przm_one0", "vw_so200_spr_tub00"];

    // ---- without the root: the name filter drops every one of them (as before ER90), nothing new is read
    let mut plain = EffectHost::load(&pack).expect("the main effect pack");
    plain.preload();
    let (before, _) = plain.assets();
    assert!(!plain.has_xsi_root());
    let live = started(&mut plain, &rig, &xs);
    assert!(live.is_empty(), "without the XSI root the XS effects stay filtered: {live:?}");
    assert!(!plain.has_effect("vw_so052_bst1_1"), "an XSI-only effect is unknown without the root");

    // ---- with the root
    let mut host = EffectHost::load(&pack).expect("the main effect pack");
    host.preload();
    let line = host.load_xsi_root(&pack).expect("the XSI root");
    assert!(line.contains("39 effects"), "{line}");
    assert_eq!(host.preload_xsi(), 39);
    let (materials, meshes) = host.assets();
    let own: Vec<&String> = materials.iter().map(|m| &m.0).filter(|n| n.starts_with(XSI_NAMESPACE)).collect();
    assert!(!own.is_empty(), "the XSI root's own materials are named xsi:<name>");
    for m in &own {
        let bare = m.strip_prefix(XSI_NAMESPACE).unwrap();
        assert!(pack.join("xsi/vfx/materials").join(format!("{bare}.json")).is_file(), "{m}: its file is in the XSI root");
        assert!(!pack.join("vfx/materials").join(format!("{bare}.json")).is_file(), "{m}: the main pack has no such material");
    }
    for (name, _, _) in materials.iter().filter(|m| !m.0.starts_with(XSI_NAMESPACE)) {
        assert!(!name.starts_with(PARTY_NAMESPACE), "{name}: no party name without the party root");
        assert!(pack.join("vfx/materials").join(format!("{name}.json")).is_file() || before.iter().any(|m| &m.0 == name), "{name}: a bare name is a main material");
    }
    for m in meshes.iter().filter(|m| m.starts_with(XSI_NAMESPACE)) {
        assert!(pack.join("xsi/vfx/meshes").join(format!("{}.glb", m.strip_prefix(XSI_NAMESPACE).unwrap())).is_file(), "{m}");
    }
    // the main materials Sora's own effects use are all still there, with their main names
    for (name, geo, blend) in &before {
        assert!(materials.iter().any(|m| &m.0 == name && m.1 == *geo && m.2 == *blend), "{name}: a main material lost");
    }
    let live = started(&mut host, &rig, &xs);
    for n in xs {
        assert!(live.iter().any(|l| l == n), "{n} started with the root: {live:?}");
    }
    // another keyblade's form effect stays filtered (not an XS form effect)
    let live = started(&mut host, &rig, &["vw_so172_bac00"]);
    assert!(!live.iter().any(|l| l.starts_with("vw_so172")), "{live:?}");

    // ---- every effect the XS clips name (xsi_manifest.json notifies) and the XS code spawns resolves in a root
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(pack.join("xsi_manifest.json")).unwrap()).unwrap();
    let mut names: Vec<String> = Vec::new();
    fn walk(v: &serde_json::Value, out: &mut Vec<String>) {
        match v {
            serde_json::Value::Object(o) => {
                if o.get("name").and_then(|n| n.as_str()) == Some("AttachEffect") {
                    if let Some(n) = o.get("value").and_then(|p| p.as_str()).and_then(|p| p.rsplit('/').next()).and_then(|n| n.split('.').next()) {
                        out.push(n.to_string());
                    }
                }
                o.values().for_each(|x| walk(x, out));
            }
            serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    walk(&manifest, &mut names);
    names.extend(["vg_kg07_pow_bem2", "vp_ant_bdy_ora00", "vw_so072_stf_przm_one0", "vw_so072_stf_przm_one_hit0"].map(str::to_string));
    names.sort();
    names.dedup();
    // (vw_so210_*: Light Form's group-2200 rows of the shared clips, the other keyblade's: never spawned with group 2100)
    let missing: Vec<&String> = names.iter().filter(|n| !n.starts_with("vw_so210_") && !host.has_effect(n)).collect();
    println!("{} XS effect names checked", names.len());
    assert!(missing.is_empty(), "XS effects in neither root: {missing:?}");
}
