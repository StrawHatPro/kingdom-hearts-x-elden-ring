//! (VFX-RING) TypeDataSQEX_TailDecal on the pack's one user, keyblades/vw_so032_fns1 `tail` (emitter 10): the type data is
//! evaluated (nothing skipped), and while the bolt node travels its tail lays one glitter decal (omi_dcl_glt_00) on every
//! tail point, at most TailNumber (13), TailInterval (12 cm) apart, each turned by DecalRotation (0, -90, 0) (projecting
//! straight down) with half extents DecalSize (400, 70, 70) x the node scale x the particle size, its colour the particle's
//! (15, 2, 0.5) and its opacity DecalOpacity at the emitter time. Skipped without the pack.

use glam::Vec3;
use kh_vfx::{Camera, DrawList, EffectDef, SpawnOptions, World, Xform};

#[test]
fn vw_so032_fns1_lays_its_tail_of_decals() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/keyblades/vfx/effects/vw_so032_fns1.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        println!("vfxring_taildecal: no {} : skipped", path.display());
        return;
    };
    let def = EffectDef::parse(&text).expect("parse");
    let tail = def.emitters.iter().position(|e| e.name == "tail" && e.kind_name == "sqex_taildecal").expect("the tail-decal emitter");
    assert!(def.unknown().iter().all(|(_, m)| !m.contains("taildecal")), "{:?}", def.unknown());
    assert!(def.emitters[tail].decal.as_ref().is_some_and(|d| d.material == "omi_dcl_glt_00"), "its decal material");
    let mut world = World::new();
    world.register(def);
    // the component sweeps along X at 6 m/s (the bolt's tail is the travelled path)
    let at = |step: usize| Xform::at(Vec3::new(step as f32 * 10.0, 0.0, 100.0));
    let id = world.spawn("vw_so032_fns1", at(0), SpawnOptions::default()).expect("spawn");
    let mut list = DrawList::default();
    let (mut peak, mut steps_with) = (0usize, 0usize);
    let mut seen_down = false;
    for step in 0..90 {
        world.set_transform(id, at(step + 1));
        world.step(1.0 / 60.0);
        list.clear();
        world.draw(1.0, &Camera::DEFAULT, &mut list);
        let mine: Vec<_> = list.groups.iter().filter(|g| g.kind == 3 && g.emitter as usize == tail).flat_map(|g| g.from as usize..g.to as usize).collect();
        if !mine.is_empty() {
            steps_with += 1;
        }
        peak = peak.max(mine.len());
        for &k in &mine {
            let d = &list.decals[k];
            // projection axis X straight down, 400 cm half extent at node scale 1
            let x = Vec3::from(d.transform.matrix3.x_axis);
            assert!(x.normalize().dot(-Vec3::Z) > 0.999, "{x:?}");
            assert!(x.length() > 1.0 && x.length() <= 400.5, "{}", x.length());
            assert!((d.color[0] / d.color[1] - 7.5).abs() < 0.01, "the particle's colour (15, 2, 0.5): {:?}", d.color);
            seen_down = true;
        }
    }
    println!("vfxring_taildecal: peak {peak} decals, on {steps_with} steps");
    assert!(seen_down && steps_with > 0, "no tail decal drawn");
    assert!((2..=13).contains(&peak), "at most TailNumber 13 decals: {peak}");
}
