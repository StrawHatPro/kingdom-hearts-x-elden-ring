//! (LDI) `World::prewarm`: an effect warmed by N seconds right after its spawn is in the state N seconds of the world's
//! own steps leave it in (same seed, same fixed step), so a save point's idle effect is first drawn already full.

use glam::Vec3;
use kh_vfx::{EffectDef, SpawnOptions, World, Xform};

const DT: f32 = 1.0 / 60.0;

fn pack_effect(name: &str) -> Option<EffectDef> {
    // (the project's pack, also from a work copy deeper in the tree: the first ancestor that has it)
    let file = format!("build/sora_pack/vfx/effects/{name}.json");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().map(|a| a.join(&file)).find(|p| p.is_file())?;
    let text = std::fs::read_to_string(path).ok()?;
    EffectDef::parse(&text).ok()
}

fn at() -> Xform {
    Xform { position: Vec3::new(100.0, -50.0, 20.0), ..Xform::default() }
}

#[test]
fn a_warmed_effect_is_where_its_own_steps_would_have_taken_it() {
    for name in ["vg_com_svp_nml00", "vg_com_svp_nml01"] {
        let Some(def) = pack_effect(name) else {
            eprintln!("(no pack effect {name} on disk: skipped)");
            continue;
        };
        let options = SpawnOptions { seed: Some(7), ..SpawnOptions::default() };
        // the world's own steps, 3 s
        let mut stepped = World::new();
        stepped.register(def.clone());
        let a = stepped.spawn(name, at(), options).unwrap();
        for _ in 0..180 {
            stepped.step(DT);
        }
        // warmed by 3 s at its spawn
        let mut warmed = World::new();
        warmed.register(def);
        let b = warmed.spawn(name, at(), options).unwrap();
        let fresh = warmed.stats();
        warmed.prewarm(b, 3.0);
        assert!(stepped.is_alive(a) && warmed.is_alive(b), "{name}: a looping idle effect stays alive");
        assert_eq!(warmed.stats(), stepped.stats(), "{name}: the warmed effect holds what 3 s of steps made");
        assert!(warmed.stats().2 > fresh.2, "{name}: particles are out at once ({:?} -> {:?})", fresh, warmed.stats());
        // and it goes on from there as any effect
        warmed.step(DT);
        stepped.step(DT);
        assert_eq!(warmed.stats(), stepped.stats(), "{name}: the next step is the same");
    }
}

#[test]
fn prewarm_of_an_unknown_or_zero_time_changes_nothing() {
    let mut world = World::new();
    world.prewarm(kh_vfx::EffectId(12345), 3.0);
    assert_eq!(world.stats(), (0, 0, 0));
    if let Some(def) = pack_effect("vg_com_svp_nml00") {
        world.register(def);
        let id = world.spawn("vg_com_svp_nml00", at(), SpawnOptions { seed: Some(1), ..SpawnOptions::default() }).unwrap();
        let before = world.stats();
        world.prewarm(id, 0.0);
        assert_eq!(world.stats(), before);
    }
}
