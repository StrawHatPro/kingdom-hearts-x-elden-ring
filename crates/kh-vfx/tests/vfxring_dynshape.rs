//! (VFX-RING S) EVERY `TypeDataSQEX_DynamicShape` OF THE FOUR ROOTS DRAWS ITS SPRING: vm_aib_blt0 / 1 / 3 (the keyblades
//! root; the census found no other user) are parsed (no "type data sqex_dynamicshape" left in `unknown()`), spawned and
//! stepped; their spring emitter draws exact ribbons (2 strips x PolygonSplit columns, PolygonNumber + 1 points each)
//! (the geometry itself: the unit tests of src/dynshape.rs). Skipped without the pack.

use std::path::{Path, PathBuf};

use glam::Vec3;
use kh_vfx::{Camera, DrawList, EffectDef, Kind, SpawnOptions, World, Xform};

fn pack() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack")
}

#[test]
fn every_dynamic_shape_draws_its_spring() {
    let dir = pack().join("keyblades").join("vfx").join("effects");
    if !dir.is_dir() {
        println!("vfxring_dynshape: no pack: skipped");
        return;
    }
    for name in ["vm_aib_blt0", "vm_aib_blt1", "vm_aib_blt3"] {
        let def = EffectDef::parse(&std::fs::read_to_string(dir.join(format!("{name}.json"))).unwrap_or_default()).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(def.unknown().iter().all(|(_, m)| !m.contains("dynamicshape") && !m.contains("dynamic shape")), "{name}: {:?}", def.unknown());
        let (k, emitter, shape) = def.emitters.iter().enumerate().find(|(_, e)| e.kind == Kind::DynamicShape).map(|(k, e)| (k, e.name.clone(), e.dynshape.clone())).unwrap_or_else(|| panic!("{name}: no spring emitter"));
        let shape = shape.unwrap_or_else(|| panic!("{name}: not built"));
        let radius = shape.edges.iter().flatten().map(|p| Vec3::new(p.pos.x, p.pos.y, 0.0).length()).fold(0.0f32, f32::max);
        let mut world = World::new();
        world.register(def);
        let at = Vec3::new(0.0, 0.0, 100.0);
        let id = world.spawn(name, Xform::at(at), SpawnOptions::default()).unwrap_or_else(|| panic!("{name}: no spawn"));
        let mut list = DrawList::default();
        let mut best = 0usize;
        for _ in 0..30 {
            world.step(1.0 / 60.0);
            list.clear();
            world.draw(1.0, &Camera::DEFAULT, &mut list);
            let ours: Vec<_> = list.groups.iter().filter(|g| g.kind == 1 && g.emitter as usize == k).collect();
            let ribbons: usize = ours.iter().map(|g| (g.to - g.from) as usize).sum();
            best = best.max(ribbons);
            for g in ours {
                for ri in g.from as usize..g.to as usize {
                    let exact = list.ribbon_exact_of(ri).unwrap_or_else(|| panic!("{name}: ribbon {ri} is not exact"));
                    assert_eq!(exact.uvs.len(), list.ribbons[ri].points.len());
                    for p in &list.ribbons[ri].points {
                        assert!(p.a.is_finite() && p.b.is_finite() && p.a != p.b, "{name}: {p:?}");
                    }
                }
            }
        }
        let strips = if shape.cross { 2 } else { 1 };
        // every instance (vm_aib_blt3 makes two c_rasen children) draws strips x columns ribbons
        assert!(best > 0 && best.is_multiple_of(strips * shape.columns as usize), "{name}: {best} ribbons for {strips} strip(s) x {} column(s)", shape.columns);
        println!("vfxring_dynshape: {name} [{k}] {}: {best} exact ribbon(s) of {} points, radius {radius:.0}", emitter, shape.edges[0].len());
        let _ = id;
    }
}
