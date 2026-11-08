//! (F-SECOND) Ancient Light's clip effects (M_1435's TresAnimNotify_AttachEffect rows: vw_so150_wrp0, vw_so150_udis_pil1 /
//! sph1 / fir0, all `WhenChangeAnimationType` END): an effect that does not finish by itself is `endless` (so a host
//! loop-ends it at the next animation change, as KH3's `Set` 0x141770D30 -> `EnableLoopEnd` 0x1442626D0 does), and a
//! loop end (`World::stop(id, false)`) ends it with everything it nests. vw_so150_udis_pil1's child emitters c_red ..
//! c_blue (EmitterLoops 0, generated once) were missed by `endless` and stayed on screen in the in-game round.
//! Skipped, saying so, without the pack.

use glam::Vec3;
use kh_vfx::{EffectDef, SpawnOptions, World, Xform};

const DT: f32 = 1.0 / 60.0;

fn pack_effect(name: &str) -> Option<EffectDef> {
    let file = format!("build/sora_pack/vfx/effects/{name}.json");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().map(|a| a.join(&file)).find(|p| p.is_file())?;
    EffectDef::parse(&std::fs::read_to_string(path).ok()?).ok()
}

/// The effect and every effect it nests, registered.
fn register(world: &mut World, name: &str) -> bool {
    let Some(def) = pack_effect(name) else { return false };
    let nested: Vec<String> = def.nested_assets();
    world.register(def);
    for n in nested {
        let short = n.rsplit(['/', '.']).next().unwrap_or(&n).to_string();
        register(world, &short);
    }
    true
}

fn steps(world: &mut World, seconds: f32) {
    for _ in 0..(seconds / DT).round() as usize {
        world.step(DT);
    }
}

#[test]
fn ancient_lights_effects_are_endless_when_they_never_end_and_a_loop_end_ends_them() {
    for name in ["vw_so150_udis_pil1", "vw_so150_udis_sph1", "vw_so150_udis_fir0", "vw_so150_wrp0"] {
        let mut world = World::new();
        if !register(&mut world, name) {
            eprintln!("SKIPPED {name}: not in the pack");
            continue;
        }
        let def = pack_effect(name).expect("registered");
        let options = SpawnOptions { seed: Some(1), ..SpawnOptions::default() };
        // left alone for 6 s (M_1435 is 5.03 s): what is still alive never ends by itself
        let id = world.spawn(name, Xform { position: Vec3::ZERO, ..Xform::default() }, options).expect("spawned");
        steps(&mut world, 6.0);
        let lasting = world.stats().0 > 0;
        assert_eq!(def.endless(), lasting, "{name}: endless() must name an effect that outlives its clip ({:?})", world.stats());
        // a loop end ends it and its nested systems
        world.stop(id, false);
        steps(&mut world, 3.0);
        assert_eq!(world.stats(), (0, 0, 0), "{name}: a loop end leaves nothing");
    }
}
