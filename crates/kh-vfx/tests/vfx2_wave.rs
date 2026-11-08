//! (VFX-2) `SQEX_TailShapeExData_Wave` as the tail helper runs it (kh-vfx def.rs `TailWaveDef`): MoveVector in cm per frame
//! (dt x 60.024), the displacement kept per point and handed down the tail. The real Rage Form aura (vp_ant_bdy_ora00,
//! MoveVector (0, 0, 1)) rises at a still anchor when the pack is on disk.

use glam::Vec3;
use kh_vfx::{Camera, DrawList, EffectDef, SpawnOptions, World, Xform};

const DT: f32 = 1.0 / 60.0;

fn drawn(world: &World) -> DrawList {
    let mut list = DrawList::default();
    world.draw(1.0, &Camera::DEFAULT, &mut list);
    list
}

#[test]
fn the_rage_aura_rises_about_a_centimetre_a_frame_at_a_still_anchor() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/vfx/effects/vp_ant_bdy_ora00.json");
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    let def = EffectDef::parse(&text).unwrap();
    let wave = def.emitters.iter().find_map(|e| e.tail_wave.as_ref().filter(|w| w.move_vector.z > 0.0)).expect("the rising tail's wave");
    assert_eq!((wave.move_vector, wave.random_range), (Vec3::new(0.0, 0.0, 1.0), Some((Vec3::new(-1.0, -1.0, 0.0), Vec3::new(1.0, 1.0, 0.0)))));
    let mut world = World::new();
    world.register(def);
    world.spawn("vp_ant_bdy_ora00", Xform::at(Vec3::new(0.0, 0.0, 100.0)), SpawnOptions::default()).unwrap();
    for _ in 0..60 {
        world.step(DT);
    }
    let list = drawn(&world);
    // a second at a still anchor: the points rise 1 cm a frame, so the tail (its first TailNumber x TailInterval cm of
    // path) stands well above the anchor; with cm / s (the reading before) it rose 1 cm in the second
    let top = list.ribbons.iter().flat_map(|r| r.points.iter()).map(|p| p.a.z.max(p.b.z)).fold(f32::MIN, f32::max);
    assert!(!list.ribbons.is_empty(), "the aura's tails are drawn at a still anchor");
    assert!(top > 100.0 + 15.0, "the tail rises: top {top}");
}
