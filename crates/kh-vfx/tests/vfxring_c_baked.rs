//! (VFX-RING C) THE KEYBLADES' VERTEX-SPAWN POINTS AND THE XS FRAGMENT MESHES.
//!
//! 1. Every keyblade effect the bake staged (`KH_C_STAGE`, default `VFX-RING/stage/C_bake`: tools/C/vfxr_bake_kb.js over
//!    PACK/keyblades/vfx, the VFXR bake that packgen's `replay-bake` step runs over vfx / party/vfx / xsi/vfx only) parses with
//!    no `SQEX_VertexSpawn` left in `unknown()` (the live pack's file still has it), and each of its vertex-spawn emitters
//!    places its particles on the asset's vertices: spread out from the node (not all at one point) and drawn.
//! 2. The XS Chandelier effects the host reads from `effects_xwd/` (kh-host vfx.rs XS_OWN_FILES) have every fragment mesh:
//!    nothing "not in the pack" (the `effects/` copies, which the census read, list them as null).
//!
//! `cargo test --release --test vfxring_c_baked -- --nocapture`. Skipped without the pack or the stage.

use std::path::{Path, PathBuf};

use glam::Vec3;
use kh_vfx::{Camera, DrawList, EffectDef, SpawnOptions, World, Xform};
use serde_json::Value;

fn here() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn pack() -> PathBuf {
    here().join("../../build/sora_pack")
}

fn stage() -> PathBuf {
    std::env::var("KH_C_STAGE").map(PathBuf::from).unwrap_or_else(|_| here().join("../../stage/C_bake"))
}

/// Indices of the emitters with an enabled `SQEX_VertexSpawn` that reads a vertex-data asset (not the attached mesh).
fn vertex_emitters(v: &Value) -> Vec<usize> {
    v["emitters"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .filter(|(_, e)| e["modules"].as_array().into_iter().flatten().any(|m| m["class"] == "ParticleModuleSQEX_VertexSpawn" && m["enabled"] != false && m["params"]["bUseAttachMeshVertex"] != true))
        .map(|(k, _)| k)
        .collect()
}

/// Peak particles, peak drawn items and the largest spread of particle places (cm) of each emitter over three seconds.
fn run(def: EffectDef) -> Vec<(usize, usize, f32)> {
    let name = def.name.clone();
    let names: Vec<String> = def.emitters.iter().map(|e| e.name.clone()).collect();
    let mut world = World::new();
    world.register(def);
    let mut out = vec![(0usize, 0usize, 0.0f32); names.len()];
    let Some(id) = world.spawn(&name, Xform::at(Vec3::new(0.0, 0.0, 100.0)), SpawnOptions::default()) else { return out };
    let mut list = DrawList::default();
    for step in 0..180 {
        if step == 120 {
            world.stop(id, false);
        }
        world.step(1.0 / 60.0);
        for e in world.probe(id) {
            if let Some(k) = names.iter().position(|x| *x == e.name) {
                out[k].0 = out[k].0.max(e.particles);
                if e.particles > 1 {
                    out[k].2 = out[k].2.max(e.max_distance);
                }
            }
        }
        list.clear();
        world.draw(1.0, &Camera::DEFAULT, &mut list);
        let mut drawn = vec![0usize; names.len()];
        for g in list.groups.iter().filter(|g| g.system == id.0) {
            if let Some(d) = drawn.get_mut(g.emitter as usize) {
                *d += (g.to - g.from) as usize;
            }
        }
        for (k, d) in drawn.into_iter().enumerate() {
            out[k].1 = out[k].1.max(d);
        }
    }
    out
}

fn skipped_vertex(def: &EffectDef) -> usize {
    def.unknown().iter().filter(|(_, m)| m.starts_with("SQEX_VertexSpawn")).count()
}

#[test]
fn keyblade_vertex_spawns_are_baked_and_place_their_particles() {
    let dir = stage().join("keyblades/vfx/effects");
    let Ok(read) = std::fs::read_dir(&dir) else {
        println!("vfxring_c_baked: no stage {}: skipped", dir.display());
        return;
    };
    let mut files: Vec<PathBuf> = read.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
    files.sort();
    let (mut effects, mut emitters, mut failed) = (0usize, 0usize, Vec::new());
    for f in &files {
        let staged: Value = serde_json::from_str(&std::fs::read_to_string(f).unwrap_or_default()).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let live_file = pack().join("keyblades/vfx/effects").join(f.file_name().unwrap_or_default());
        let live = EffectDef::parse(&std::fs::read_to_string(&live_file).unwrap_or_else(|e| panic!("{}: {e}", live_file.display()))).unwrap_or_else(|e| panic!("{e}"));
        let def = EffectDef::from_json(&staged).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        let name = def.name.clone();
        let list = vertex_emitters(&staged);
        if list.is_empty() {
            println!("  {name}: no vertex-data spawn (attached-mesh only): the bake changes nothing the evaluator reads");
            continue;
        }
        effects += 1;
        assert!(skipped_vertex(&live) > 0, "{name}: the live file was expected to be unbaked");
        if skipped_vertex(&def) > 0 {
            failed.push(format!("{name}: still unbaked"));
            continue;
        }
        let peaks = run(def);
        for k in list {
            emitters += 1;
            let (particles, drawn, spread) = peaks[k];
            let emitter = staged["emitters"][k]["name"].as_str().unwrap_or("").to_string();
            println!("  {name} [{k}] {emitter}: peak particles {particles}, drawn {drawn}, spread {spread:.1} cm");
            if particles > 1 && spread < 0.5 {
                failed.push(format!("{name} [{k}] {emitter}: {particles} particles all at the node"));
            }
        }
    }
    println!("vfxring_c_baked: {effects} effects, {emitters} vertex-spawn emitters baked; {} failed", failed.len());
    assert!(failed.is_empty(), "{failed:#?}");
    assert!(effects >= 20, "the stage holds {effects} baked effects");
}

#[test]
fn xs_chandelier_effects_have_every_fragment_mesh() {
    let dir = pack().join("xsi/vfx/effects_xwd");
    if !dir.is_dir() {
        println!("vfxring_c_baked: no {}: skipped", dir.display());
        return;
    }
    for name in ["vw_so088_fin_hahen02", "vw_so088_fin_hahen03", "vw_so088_fin_zmn0"] {
        let own = EffectDef::parse(&std::fs::read_to_string(dir.join(format!("{name}.json"))).unwrap_or_default()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let missing: Vec<String> = own.unknown().into_iter().filter(|(_, m)| m.contains("fragment meshes")).map(|(e, m)| format!("{e}: {m}")).collect();
        assert!(missing.is_empty(), "{name} (effects_xwd): {missing:?}");
        // every piece names a mesh file that is in the pack
        let v: Value = serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{name}.json"))).unwrap_or_default()).unwrap_or_default();
        let mut pieces = 0usize;
        for e in v["emitters"].as_array().into_iter().flatten() {
            for set in e["typedata"]["sub_mesh_files"].as_array().into_iter().flatten() {
                for m in set.as_array().into_iter().flatten() {
                    let m = m.as_str().unwrap_or_else(|| panic!("{name}: a null piece"));
                    assert!(pack().join("xsi/vfx").join(m).is_file(), "{name}: {m} is not in the pack");
                    pieces += 1;
                }
            }
        }
        let shared = EffectDef::parse(&std::fs::read_to_string(pack().join("xsi/vfx/effects").join(format!("{name}.json"))).unwrap_or_default()).unwrap_or_else(|e| panic!("{name}: {e}"));
        let shared_missing = shared.unknown().into_iter().filter(|(_, m)| m.contains("fragment meshes")).count();
        println!("  {name}: effects_xwd {pieces} pieces, all in the pack; the effects/ copy (not read while fx.xs_own is on) lacks them on {shared_missing} emitter(s)");
    }
}
