//! (KEYBLADES) EVERY EFFECT OF THE KEYBLADES' ROOT LOADS AND RUNS: each `PACK/keyblades/vfx/effects/<name>.json`
//! (tools/sora_keyblades_pack.js -> tools/sora_vfx_pack.js: the keyblades' appear / form-change / finisher / hit effects and
//! the Moogle shop's that the main, party and XS roots lack) is parsed by `EffectDef::parse`, registered, spawned and stepped
//! for two seconds at 60 Hz (stopped at two thirds), as the probe does. Every effect the keyblade manifest places in another
//! root (`vfx.where`: PACK/vfx, party/vfx, xsi/vfx) is parsed there. Skipped without the pack.
//!
//! `cargo test -p kh-vfx --test keyblade_fx -- --nocapture` prints the counts and every effect that fails.

use std::path::{Path, PathBuf};

use glam::Vec3;
use kh_vfx::{DrawList, EffectDef, SpawnOptions, World, Xform};

fn pack() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack")
}

/// Parse, spawn and step one effect file: Ok(peak live particles) or why it failed.
fn run(file: &Path) -> Result<usize, String> {
    let text = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
    let def = EffectDef::parse(&text)?;
    let name = file.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut world = World::new();
    world.register(def);
    let id = world.spawn(&name, Xform::at(Vec3::new(0.0, 0.0, 100.0)), SpawnOptions::default()).ok_or("spawn returned nothing")?;
    let mut list = DrawList::default();
    let mut peak = 0usize;
    for step in 0..120 {
        if step == 80 {
            world.stop(id, false);
        }
        world.step(1.0 / 60.0);
        let live: usize = world.probe(id).iter().map(|e| e.particles).sum();
        peak = peak.max(live);
        if step % 30 == 0 {
            list.clear();
        }
    }
    Ok(peak)
}

#[test]
fn every_keyblade_effect_loads_and_runs() {
    let dir = pack().join("keyblades").join("vfx").join("effects");
    let Ok(read) = std::fs::read_dir(&dir) else {
        println!("keyblade_fx: no {} (the keyblade pack is not built): skipped", dir.display());
        return;
    };
    let mut files: Vec<PathBuf> = read.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
    files.sort();
    // the effects the keyblade manifest says another root holds
    let manifest: serde_json::Value = std::fs::read_to_string(pack().join("keyblades").join("manifest.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let mut elsewhere = 0usize;
    for (effect, root) in manifest["vfx"]["where"].as_object().into_iter().flatten() {
        let Some(root) = root.as_str().filter(|r| ["vfx", "party/vfx", "xsi/vfx"].contains(r)) else { continue };
        let name = effect.rsplit('/').next().unwrap_or("").to_ascii_lowercase();
        let root_dir = pack().join(root).join("effects");
        if let Some(f) = std::fs::read_dir(&root_dir).ok().into_iter().flatten().flatten().map(|e| e.path()).find(|p| p.file_stem().is_some_and(|s| s.to_string_lossy().to_ascii_lowercase() == name)) {
            files.push(f);
            elsewhere += 1;
        }
    }
    let (mut ok, mut silent, mut failed) = (0usize, 0usize, Vec::new());
    for f in &files {
        match run(f) {
            Ok(0) => silent += 1,
            Ok(_) => ok += 1,
            Err(e) => failed.push(format!("{}: {e}", f.display())),
        }
    }
    println!("keyblade_fx: {} effects ({} of them in the main / party / XS roots): {ok} spawn particles, {silent} spawn none in two seconds (children, mesh-only, event-driven), {} failed", files.len(), elsewhere, failed.len());
    for f in &failed {
        println!("  FAILED {f}");
    }
    assert!(failed.is_empty(), "{} effect(s) did not load or run", failed.len());
}
