//! (VFX-RING M) TypeDataSQEX_TailModelMarch on the pack's three users (keyblades root: vw_so170_shf_blt00 [3]
//! TailPoly_lp10110, vw_so172_blc00 [4] til, vw_so220_brs_blk00 [10] chain): each is evaluated (no "type data" skip), and
//! while its component moves its meshes march along the track, W x H of them on every point, as `kh_vfx::tailmodel`
//! places them. Skipped without the pack. `cargo test -p kh-vfx --test vfxring_tailmodel -- --nocapture` prints the counts.

use std::path::{Path, PathBuf};

use glam::Vec3;
use kh_vfx::{Camera, DrawList, EffectDef, SpawnOptions, World, Xform};

const USERS: [(&str, usize, usize); 3] = [("vw_so170_shf_blt00", 3, 4), ("vw_so172_blc00", 4, 10), ("vw_so220_brs_blk00", 10, 1)];

fn effect(name: &str) -> Option<EffectDef> {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/keyblades/vfx/effects").join(format!("{name}.json"));
    let text = std::fs::read_to_string(path).ok()?;
    EffectDef::parse(&text).ok()
}

/// Peak meshes the emitter `index` draws while the component moves `speed` cm per step along +X for `steps` steps, and
/// their spread across the track (largest |y| of a mesh).
fn run(def: EffectDef, index: usize, speed: f32, steps: usize) -> (usize, f32) {
    let name = def.name.clone();
    let mut world = World::new();
    world.register(def);
    let at = |k: usize| Xform::at(Vec3::new(k as f32 * speed, 0.0, 100.0));
    let Some(id) = world.spawn(&name, at(0), SpawnOptions::default()) else { return (0, 0.0) };
    let (mut peak, mut spread) = (0usize, 0.0f32);
    let mut list = DrawList::default();
    for k in 0..steps {
        world.set_transform(id, at(k + 1));
        world.step(1.0 / 60.0);
        list.clear();
        world.draw(1.0, &Camera::DEFAULT, &mut list);
        let mut n = 0usize;
        for g in list.groups.iter().filter(|g| g.kind == 2 && g.emitter as usize == index) {
            n += (g.to - g.from) as usize;
            for m in &list.meshes[g.from as usize..g.to as usize] {
                spread = spread.max(m.transform.translation.y.abs());
            }
        }
        peak = peak.max(n);
    }
    (peak, spread)
}

#[test]
fn the_tail_model_marches_are_evaluated_and_draw_their_grids() {
    for (name, index, per_point) in USERS {
        let Some(def) = effect(name) else {
            println!("vfxring_tailmodel: no {name} in the pack: skipped");
            return;
        };
        let e = def.emitters[index].clone();
        let tm = e.tail_models.as_ref().unwrap_or_else(|| panic!("{name} [{index}] is no TailModelMarch"));
        assert_eq!(tm.per_point(), per_point, "{name}");
        assert!(!e.unknown.iter().any(|u| u.contains("tailmodelmarch")), "{name}: still skipped: {:?}", e.unknown);
        let (peak, spread) = run(def, index, 6.0, 40);
        println!("vfxring_tailmodel: {name} [{index}] {}: {per_point} model(s) a point, peak {peak} meshes drawn, spread {spread:.1} cm", e.name);
        assert!(peak >= per_point, "{name}: {peak} meshes");
        assert_eq!(peak % per_point, 0, "{name}: whole points only ({peak})");
    }
}

#[test]
fn never_more_than_w_x_h_x_tail_number() {
    for (name, index, per_point) in USERS {
        let Some(def) = effect(name) else { return };
        let number = def.emitters[index].tail.as_ref().map_or(0.0, |t| t.number.float(0.0, &mut kh_vfx::Rng::new(1)));
        let (peak, _) = run(def, index, 20.0, 60);
        assert!(peak <= per_point * number.max(1.0) as usize * 2, "{name}: {peak} for {per_point} x {number}");
    }
}
