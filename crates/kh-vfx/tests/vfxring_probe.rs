//! (VFX-RING) A probe, not a test: `KH_RING_PROBE=<path to effect json> cargo test -p kh-vfx --test vfxring_probe -- --nocapture`
//! runs that effect file at (0, 0, 100) for 120 steps (loop end at step 80) and prints the emitters, the live particles every
//! tenth step and the evaluator's log. Without the variable it does nothing.

use glam::Vec3;
use kh_vfx::{Camera, DrawList, EffectDef, SpawnOptions, World, Xform};

#[test]
fn ring_probe() {
    let Ok(path) = std::env::var("KH_RING_PROBE") else { return };
    let def = EffectDef::parse(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap_or_else(|e| panic!("{path}: {e}"));
    let name = def.name.clone();
    for e in &def.emitters {
        println!("emitter {:<22} {:?} loops {} time {:.3}/{:.3}/{:.3}/{:.3} material {} modules {} unknown {:?}", e.name, e.kind, e.loops, e.start_time, e.loop_time, e.loop_duration, e.duration_time, e.material, e.modules.len(), e.unknown);
    }
    let mut world = World::new();
    world.register(def);
    let id = world.spawn(&name, Xform::at(Vec3::new(0.0, 0.0, 100.0)), SpawnOptions::default()).unwrap_or_else(|| panic!("{name}: no spawn"));
    let mut list = DrawList::default();
    for step in 0..120 {
        if step == 80 {
            world.stop(id, false);
        }
        world.step(1.0 / 60.0);
        if step % 10 == 0 {
            list.clear();
            world.draw(1.0, &Camera::DEFAULT, &mut list);
            let probes = world.probe(id);
            let parts: Vec<String> = probes.iter().map(|e| format!("{}:{} d{:.0} s{:.0}", e.name, e.particles, e.max_distance, e.max_size)).collect();
            println!("step {step:3}: quads {} meshes {} ribbons {} | {}", list.quads.len(), list.meshes.len(), list.ribbons.len(), parts.join(" "));
        }
    }
    for line in world.take_log() {
        println!("log: {line}");
    }
}
