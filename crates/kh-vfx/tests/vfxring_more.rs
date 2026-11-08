//! (VFX-RING) The modules of `vfxring_more.rs` and the thunder MY_MODULE ends on the pack's own emitters (skipped without the
//! pack):
//! - Ever After's finisher beam `vw_so073_fin_beam0` Beam_in (TypeDataSQEX_Thunder, MY_MODULE -> MY_MODULE, SQEX_Ring of one
//!   corner, RangeRadius 0 -> 1000 over 0.5 s, no second radius): run alone (its SQEX_Parent dropped), its beam runs from the
//!   node (the stock BeamSource, PEB2STM_Default) to the ring corner 1000 cm out along the node's X (0x14424FAF0).
//! - `vw_so080_app0` c_yuki_tubu00 (SQEX_ParticleColor, direct, on emitter time): its particles' colour is the product of
//!   StartParticleColor (one of its two corners) x ParticleColor x the stock BaseColor, not the stock colour alone.
//!
//! `cargo test -p kh-vfx --test vfxring_more -- --nocapture`

use std::path::{Path, PathBuf};

use glam::Vec3;
use kh_vfx::{Camera, DrawList, EffectDef, SpawnOptions, World, Xform};
use serde_json::Value;

fn pack() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack")
}

/// One emitter of a pack effect as an effect of its own (its SQEX_Parent dropped), or None without the pack.
fn alone(file: &str, emitter: &str) -> Option<EffectDef> {
    let text = std::fs::read_to_string(pack().join(file)).ok()?;
    let mut v: Value = serde_json::from_str(&text).ok()?;
    let mut e = v["emitters"].as_array()?.iter().find(|e| e["name"] == emitter)?.clone();
    if let Some(m) = e["modules"].as_array_mut() {
        m.retain(|m| m["class"] != "ParticleModuleSQEX_Parent");
    }
    // a child template (bAutoAddToComponent off) played as a root
    e["required"]["params"]["bAutoAddToComponent"] = Value::Bool(true);
    v["emitters"] = Value::Array(vec![e]);
    v["name"] = Value::String(format!("alone_{emitter}"));
    EffectDef::from_json(&v).ok()
}

#[test]
fn ever_after_beam_runs_from_the_node_to_its_ring_corner() {
    let Some(def) = alone("keyblades/vfx/effects/vw_so073_fin_beam0.json", "Beam_in") else {
        println!("vfxring_more: no pack: skipped");
        return;
    };
    assert!(def.unknown().is_empty(), "MY_MODULE is evaluated now: {:?}", def.unknown());
    let mut world = World::new();
    world.register(def);
    world.spawn("alone_Beam_in", Xform::at(Vec3::new(0.0, 0.0, 100.0)), SpawnOptions::default()).expect("spawn");
    let mut list = DrawList::default();
    let mut longest = 0.0f32;
    for step in 0..40 {
        world.step(1.0 / 60.0);
        if step >= 34 {
            list.clear();
            world.draw(1.0, &Camera::DEFAULT, &mut list);
            for r in &list.ribbons {
                let mid = |k: usize| (r.points[k].a + r.points[k].b) * 0.5;
                longest = longest.max(mid(0).distance(mid(r.points.len() - 1)));
            }
        }
    }
    // at 0.57 s RangeRadius is 1000: the beam is the corner's distance from the node
    assert!((longest - 1000.0).abs() < 25.0, "beam length {longest}");
}

#[test]
fn snow_sparkles_take_their_particle_colour() {
    let Some(def) = alone("keyblades/vfx/effects/vw_so080_app0.json", "c_yuki_tubu00") else {
        println!("vfxring_more: no pack: skipped");
        return;
    };
    assert!(def.unknown().is_empty(), "{:?}", def.unknown());
    let mut world = World::new();
    world.register(def);
    let id = world.spawn("alone_c_yuki_tubu00", Xform::at(Vec3::new(0.0, 0.0, 100.0)), SpawnOptions::default()).expect("spawn");
    let mut list = DrawList::default();
    let mut blue_or_green = 0usize;
    for _ in 0..12 {
        world.step(1.0 / 60.0);
    }
    world.draw(1.0, &Camera::DEFAULT, &mut list);
    assert!(world.is_alive(id));
    for q in &list.quads {
        // StartParticleColor (1, 5, 0.2) .. (0.2, 0.2, 5) x ParticleColor (3, 0.5, 0.8) .. (2, 1, 5): never the stock white
        if (q.color[0] - q.color[1]).abs() > 0.05 || (q.color[1] - q.color[2]).abs() > 0.05 {
            blue_or_green += 1;
        }
    }
    assert!(!list.quads.is_empty(), "no sparkle drawn");
    // (before: the stock colour alone, the same in every channel, on every sparkle; now all but the rare draw whose random
    // channels meet)
    assert!(blue_or_green * 10 >= list.quads.len() * 9, "{blue_or_green} of {} sparkles coloured by SQEX_ParticleColor", list.quads.len());
}
