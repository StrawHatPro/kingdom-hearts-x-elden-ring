//! (VFX-RING) THE RING CENSUS: every effect of the four roots (PACK/vfx, party/vfx, xsi/vfx, keyblades/vfx) that has a
//! `ParticleModuleSQEX_Ring` is parsed, spawned at (0, 0, 100) with its nested `SQEX_SpawnParticleSystem` effects registered
//! (from any root), and stepped for six seconds at 60 Hz (loop end at 4 s: the end-section children of vw_so088_fin_zmn0 start at 4 s) over a floor at z = 0 while its component walks
//! along X at 3 m/s, drawn every other step. Each ring emitter's
//! draws are its own quads / ribbons / meshes / decals plus those of everything it places: the children it generates
//! (`SQEX_Parent`, any depth) and the nested systems its subtree starts. With `KH_RING_CENSUS=<out.json>` it also writes, for
//! every effect of the four roots, what the evaluator skips (`EffectDef::unknown`). Skipped without the pack.
//!
//! `cargo test -p kh-vfx --release --test vfxring_census -- --nocapture` prints the ring emitters that draw nothing; the
//! test fails on any that is not in `EXPECTED_SILENT` (each with the reason it draws nothing offline, from STATE.md).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use glam::Vec3;
use kh_vfx::{Camera, DrawList, EffectDef, Kind, SpawnOptions, World, Xform};
use serde_json::{json, Value};

const ROOTS: [(&str, &str); 4] = [("main", "vfx"), ("party", "party/vfx"), ("xsi", "xsi/vfx"), ("keyblades", "keyblades/vfx")];

/// Ring emitters that draw nothing OFFLINE for a reason outside the ring (root/effect [index]); see VFX-RING/STATE.md.
const EXPECTED_SILENT: &[&str] = &[
    // not generated at all: auto-add off and no parent names it (Control's child "Particle Emitter" names no emitter;
    // SQEX_Parent::Update 0x14437A3B0 skips a name that does not resolve)
    "main/vl_dw02_hmn2 [3]",
    // disabled in the asset (enabled false)
    "main/vs_com_haj_hit00 [5]",
    // Control nodes with no child and no nested system: nothing to place (the effect draws through its other emitters)
    "party/VN_slg_hvy00 [0]",
    "keyblades/vm_waw_blt1 [4]",
    "main/vw_so093_ran_snc3 [0]",
    "party/vm_thc_rng3 [2]",
    "main/vw_so200_bac00 [1]",
    // the only child is named "c_Glw1", which no emitter of the effect is (vw_so053's sprite also has no material)
    "xsi/vw_so052_bst2_1 [0]",
    "keyblades/vw_so053_bst_stf1 [3]",
    // its child gpu1 spawns but is EmitterRenderMode ERM_LightsOnly with no Light module
    "keyblades/vw_so032_hbn1 [0]",
    // SQEX_Raycast ONHIT checking the physical surface (SurfaceType32-37, 39: water) at the root of the chain: the offline
    // floor is surface 0; in game it plays over water only
    "keyblades/vw_so053_jwr_f080 [29]",
    "keyblades/vw_so053_jwr_f080 [30]",
    "keyblades/vw_so053_jwr_f080 [31]",
    "keyblades/vw_so220_warp_st0 [7]",
    "keyblades/vw_so220_warp_st0 [8]",
    // SQEX_Raycast ONHIT with RayEndOffset (0, 0, -50): plays within 50 cm of a floor (the census spawns at 100 cm; at 20 cm
    // both draw 87 particles)
    "keyblades/vw_so142_fns_fpr00 [7]",
    "keyblades/vw_so145_fsh_fir01 [6]",
    // honey drops cast 600 cm down from the firework's burst at 12 - 13 m: no floor in reach
    "keyblades/vw_so044_fsh_hit02 [6]",
];

fn pack() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack")
}

/// The ring emitters of one effect file (by the JSON: the module class), enabled ones only.
fn ring_emitters(v: &Value) -> Vec<usize> {
    v["emitters"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .filter(|(_, e)| e["modules"].as_array().into_iter().flatten().any(|m| m["class"] == "ParticleModuleSQEX_Ring" && m["enabled"] != false))
        .map(|(k, _)| k)
        .collect()
}

/// Every effect file of the four roots by its lower-case name (the first root wins).
fn index() -> HashMap<String, PathBuf> {
    let mut map = HashMap::new();
    for (_, dir) in ROOTS {
        for f in std::fs::read_dir(pack().join(dir).join("effects")).into_iter().flatten().flatten().map(|e| e.path()) {
            if f.extension().is_some_and(|x| x == "json") {
                if let Some(stem) = f.file_stem() {
                    map.entry(stem.to_string_lossy().to_ascii_lowercase()).or_insert(f);
                }
            }
        }
    }
    map
}

/// What one emitter did over the run.
#[derive(Clone, Copy, Default)]
struct Peak {
    particles: usize,
    drawn: usize,
    /// It reached its own timeline (emitter time > 0) at least once.
    started: bool,
}

/// Peak particles and draws of every emitter of `def` over four seconds; the draws of the nested systems are put on the
/// emitters that start them (by asset name).
fn run(def: EffectDef, files: &HashMap<String, PathBuf>) -> Vec<Peak> {
    let name = def.name.clone();
    let names: Vec<String> = def.emitters.iter().map(|e| e.name.clone()).collect();
    let nested: Vec<Option<String>> = def.emitters.iter().map(|e| e.nested.as_ref().map(|n| n.asset.to_ascii_lowercase())).collect();
    let mut world = World::new();
    let mut todo = def.nested_assets();
    world.register(def);
    while let Some(asset) = todo.pop() {
        if world.has(&asset) {
            continue;
        }
        if let Some(d) = files.get(&asset.to_ascii_lowercase()).and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| EffectDef::parse(&t).ok()) {
            todo.extend(d.nested_assets());
            world.register(d);
        }
    }
    let n = names.len();
    let mut peaks = vec![Peak::default(); n];
    // a floor at z = 0 for the casts (`SQEX_Raycast`), and the component walking along X at 3 m/s (what a carried effect
    // sees: speed lines and tails draw only from travel)
    world.set_ground(Some(0.0));
    let at = |step: usize| Xform::at(Vec3::new(step as f32 * 5.0, 0.0, 100.0));
    let Some(id) = world.spawn(&name, at(0), SpawnOptions::default()) else { return peaks };
    let mut list = DrawList::default();
    for step in 0..360 {
        if step == 240 {
            world.stop(id, false);
        }
        world.set_transform(id, at(step + 1));
        world.set_trail(id, Vec3::new(step as f32 * 2.0, 0.0, 180.0), Vec3::new(step as f32 * 2.0, 0.0, 115.0));
        world.step(1.0 / 60.0);
        if step % 2 == 1 {
            continue;
        }
        let live = world.live();
        let mut particles = vec![0usize; n];
        let mut system_name: HashMap<u64, String> = HashMap::new();
        for s in &live {
            system_name.insert(s.id.0, s.name.to_ascii_lowercase());
            if s.id != id {
                continue;
            }
            for (emitter, time, _, count) in &s.emitters {
                if let Some(k) = names.iter().position(|x| x == emitter) {
                    particles[k] += count;
                    if *time > 0.0 {
                        peaks[k].started = true;
                    }
                }
            }
        }
        list.clear();
        world.draw(1.0, &Camera::DEFAULT, &mut list);
        let mut drawn = vec![0usize; n];
        for g in &list.groups {
            let items = (g.to - g.from) as usize;
            if g.system == id.0 {
                if let Some(d) = drawn.get_mut(g.emitter as usize) {
                    *d += items;
                }
            } else if let Some(asset) = system_name.get(&g.system) {
                for (k, nest) in nested.iter().enumerate() {
                    if nest.as_deref() == Some(asset.as_str()) {
                        drawn[k] += items;
                    }
                }
            }
        }
        for k in 0..n {
            peaks[k].particles = peaks[k].particles.max(particles[k]);
            peaks[k].drawn = peaks[k].drawn.max(drawn[k]);
        }
    }
    peaks
}

#[test]
fn every_ring_emitter_draws() {
    if !pack().join("vfx").join("effects").is_dir() {
        println!("vfxring_census: no pack: skipped");
        return;
    }
    let files = index();
    let out = std::env::var("KH_RING_CENSUS").ok();
    let mut skipped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut rows = Vec::new();
    let mut silent = Vec::new();
    let mut effects = 0usize;
    for (root, dir) in ROOTS {
        let dir = pack().join(dir).join("effects");
        let mut list: Vec<PathBuf> = std::fs::read_dir(&dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "json")).collect();
        list.sort();
        for f in list {
            // an XS effect is read from effects_xwd/ when it has a copy there, as kh-host vfx.rs does (its fragment pieces)
            let xwd = f.parent().and_then(Path::parent).map(|p| p.join("effects_xwd").join(f.file_name().unwrap_or_default())).filter(|x| root == "xsi" && x.is_file());
            let Ok(text) = std::fs::read_to_string(xwd.as_ref().unwrap_or(&f)) else { continue };
            let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
            let rings = ring_emitters(&v);
            if rings.is_empty() && out.is_none() {
                continue;
            }
            let def = EffectDef::from_json(&v).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
            for (_, module) in def.unknown() {
                skipped.entry(module).or_default().push(format!("{root}/{}", def.name));
            }
            if rings.is_empty() {
                continue;
            }
            effects += 1;
            let kinds: Vec<Kind> = def.emitters.iter().map(|e| e.kind).collect();
            // the children each emitter generates (`SQEX_Parent`): a ring emitter draws through them too (any depth below
            // it): a Control node, or a particle emitter with no material whose particles carry the children
            let children: Vec<Vec<usize>> = def.emitters.iter().map(|e| e.children.iter().map(|c| c.emitter).collect()).collect();
            let name = def.name.clone();
            let peaks = run(def, &files);
            for k in rings {
                let mut drawn = peaks[k].drawn;
                let mut seen = vec![false; peaks.len()];
                let mut todo = children[k].clone();
                while let Some(c) = todo.pop() {
                    if c < peaks.len() && !seen[c] {
                        seen[c] = true;
                        drawn += peaks[c].drawn;
                        todo.extend(children[c].iter().copied());
                    }
                }
                let emitter = v["emitters"][k]["name"].as_str().unwrap_or("").to_string();
                let key = format!("{root}/{name} [{k}]");
                if drawn == 0 {
                    silent.push((key.clone(), format!("{key} {emitter} ({:?}, peak particles {}, {})", kinds[k], peaks[k].particles, if peaks[k].started { "started" } else { "NEVER STARTED" })));
                }
                rows.push(json!({ "root": root, "effect": name, "emitter": emitter, "index": k, "kind": format!("{:?}", kinds[k]), "peak_particles": peaks[k].particles, "peak_drawn": drawn, "started": peaks[k].started }));
            }
        }
    }
    println!("vfxring_census: {effects} effects, {} ring emitters, {} draw nothing ({} of them expected)", rows.len(), silent.len(), silent.iter().filter(|(k, _)| EXPECTED_SILENT.contains(&k.as_str())).count());
    for (k, s) in &silent {
        println!("  {} {s}", if EXPECTED_SILENT.contains(&k.as_str()) { "expected" } else { "SILENT" });
    }
    for (m, list) in &skipped {
        println!("  skipped by the evaluator: {m}: {} emitter(s)", list.len());
    }
    if let Some(path) = out {
        let skipped: BTreeMap<&String, Value> = skipped.iter().map(|(m, l)| (m, json!(l))).collect();
        let silent: Vec<&String> = silent.iter().map(|(_, s)| s).collect();
        std::fs::write(&path, serde_json::to_string_pretty(&json!({ "rings": rows, "silent": silent, "skipped": skipped })).unwrap_or_default()).unwrap_or_else(|e| panic!("{path}: {e}"));
    }
    let unexpected: Vec<&String> = silent.iter().filter(|(k, _)| !EXPECTED_SILENT.contains(&k.as_str())).map(|(_, s)| s).collect();
    assert!(unexpected.is_empty(), "{} ring emitter(s) draw nothing: {unexpected:#?}", unexpected.len());
}
