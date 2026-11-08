//! (VFX-RING) SQEX_TailNoise in the pack: VW_so021_tam0's tail0 (Shooting Star's shot, keyblades root) is the one enabled user
//! of the four roots (vp_ath_tgt0's two modules are disabled in the asset). Flown at 6 m/s and loop-ended, its tail breaks up
//! as NoiseIntensity rises after emitter time 0.8 (0 -> 7 at 1.0): fewer and fainter sprites than the same tail without the
//! module, the same before 0.8. Skipped without the pack.

use std::path::Path;

use glam::Vec3;
use kh_vfx::{Camera, DrawList, EffectDef, SpawnOptions, World, Xform};

fn tail0(noise: bool) -> Option<Vec<(f32, usize, f32)>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/keyblades/vfx/effects/VW_so021_tam0.json");
    let mut def = EffectDef::parse(&std::fs::read_to_string(path).ok()?).ok()?;
    let k = def.emitters.iter().position(|e| e.name == "tail0")?;
    assert!(def.emitters[k].tail_noise.is_some(), "tail0 carries its TailNoise");
    assert!(def.emitters[k].unknown.iter().all(|u| !u.contains("TailNoise")), "evaluated: {:?}", def.emitters[k].unknown);
    if !noise {
        def.emitters[k].tail_noise = None;
    }
    let mut world = World::new();
    world.register(def);
    let at = |step: usize| Xform::at(Vec3::new(step as f32 * 10.0, 0.0, 100.0));
    let id = world.spawn("VW_so021_tam0", at(0), SpawnOptions::default())?;
    let mut list = DrawList::default();
    let mut out = Vec::new();
    for step in 0..58 {
        if step == 30 {
            world.stop(id, false); // the shot ends: the emitter plays its end section (0.6 .. 1.0)
        }
        world.set_transform(id, at(step + 1));
        world.step(1.0 / 60.0);
        list.clear();
        world.draw(1.0, &Camera::DEFAULT, &mut list);
        let rel = world.live().iter().find(|s| s.id == id).and_then(|s| s.emitters.iter().find(|x| x.0 == "tail0").map(|x| x.1)).unwrap_or(-1.0);
        let quads: Vec<f32> = list.groups.iter().filter(|g| g.system == id.0 && g.emitter as usize == k && g.kind == 0).flat_map(|g| list.quads[g.from as usize..g.to as usize].iter().map(|q| q.color[3])).collect();
        out.push((rel, quads.len(), quads.iter().sum::<f32>()));
    }
    Some(out)
}

#[test]
fn tam0s_tail_breaks_up_as_the_noise_rises() {
    let (Some(with), Some(without)) = (tail0(true), tail0(false)) else {
        println!("vfxring_tailnoise: no pack: skipped");
        return;
    };
    let mut late = 0;
    for ((rel, n, a), (rel2, n2, a2)) in with.iter().zip(&without) {
        assert!((rel - rel2).abs() < 1.0e-5);
        if *rel < 0.79 {
            assert_eq!((n, a), (n2, a2), "intensity 0 before 0.8 (rel {rel}): no change");
        } else if *rel > 0.9 && *n2 > 0 {
            late += 1;
            assert!(n < n2 || a < &(a2 * 0.9), "rel {rel}: with noise {n} sprites alpha {a}, without {n2} / {a2}");
        }
        println!("rel {rel:.3}: with noise {n} sprites (alpha {a:.3}), without {n2} ({a2:.3})");
    }
    assert!(late > 0, "the run reached the noisy end of the tail");
}

#[test]
fn a_disabled_tailnoise_is_not_taken() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/vfx/effects/vp_ath_tgt0.json");
    let Ok(text) = std::fs::read_to_string(path) else { return };
    let def = EffectDef::parse(&text).unwrap_or_else(|e| panic!("{e}"));
    assert!(def.emitters.iter().all(|e| e.tail_noise.is_none()), "vp_ath_tgt0's TailNoise modules are disabled");
}
