//! (DON-MAGIC tint) Per-spawn recolours (`World::set_tint`): the draws of a tinted effect (and of the nested systems it
//! spawns) are listed in `DrawList::tints` for the renderers, whose shaders recolour the final colour (rgb = max(r, g, b)
//! x tint x intensity, alpha kept); the vertex colours stay as the effect made them (a material may read them as data).

use glam::Vec3;
use kh_vfx::{Camera, DrawList, EffectDef, SpawnOptions, World, Xform};

const DT: f32 = 1.0 / 60.0;

fn pack_effect(name: &str) -> Option<EffectDef> {
    let file = format!("build/sora_pack/vfx/effects/{name}.json");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().map(|a| a.join(&file)).find(|p| p.is_file())?;
    EffectDef::parse(&std::fs::read_to_string(path).ok()?).ok()
}

#[test]
fn the_recolour_keeps_the_brightness_and_replaces_the_hue() {
    let c = kh_vfx::recolor([1.0, 0.5, 0.1, 0.7], [0.72, 0.38, 1.0, 1.0]);
    assert_eq!(c, [0.72, 0.38, 1.0, 0.7], "fire orange -> violet at the same brightness, alpha kept");
    let c = kh_vfx::recolor([0.5, 0.5, 0.5, 1.0], [0.45, 1.0, 0.5, 2.0]);
    assert!((c[1] - 1.0).abs() < 1e-6 && (c[0] - 0.45).abs() < 1e-6, "intensity x2 on a half-grey: {c:?}");
}

#[test]
fn a_tinted_effect_lists_its_draws_as_tinted_and_keeps_its_vertex_colours() {
    let name = "vm_fir_blt3";
    let Some(def) = pack_effect(name) else {
        eprintln!("(no pack effect {name}: skipped)");
        return;
    };
    let draw = |tint: Option<[f32; 4]>, neighbour: bool| {
        let mut w = World::new();
        w.register(def.clone());
        let at = |x: f32| Xform { position: Vec3::new(x, 0.0, 100.0), ..Xform::default() };
        let id = w.spawn(name, at(0.0), SpawnOptions { seed: Some(3), ..SpawnOptions::default() }).unwrap();
        if neighbour {
            w.spawn(name, at(500.0), SpawnOptions { seed: Some(3), ..SpawnOptions::default() }).unwrap();
        }
        w.set_tint(id, tint);
        for _ in 0..20 {
            w.step(DT);
        }
        let mut out = DrawList::default();
        w.draw(1.0, &Camera::DEFAULT, &mut out);
        out
    };
    let own = draw(None, false);
    let violet = draw(Some([0.72, 0.38, 1.0, 1.0]), false);
    assert!(own.tints.is_empty());
    assert_eq!(own.quads.iter().map(|q| q.color).collect::<Vec<_>>(), violet.quads.iter().map(|q| q.color).collect::<Vec<_>>(), "vertex colours untouched");
    for k in 0..violet.quads.len() {
        assert_eq!(violet.tint_of(0, k), Some([0.72, 0.38, 1.0, 1.0]), "quad {k} tinted");
    }
    for k in 0..violet.meshes.len() {
        assert_eq!(violet.tint_of(2, k), Some([0.72, 0.38, 1.0, 1.0]), "mesh {k} tinted");
    }
    // with an untinted neighbour: only the tinted one's draws are listed
    let both = draw(Some([0.72, 0.38, 1.0, 1.0]), true);
    let tinted = (0..both.quads.len()).filter(|k| both.tint_of(0, *k).is_some()).count();
    assert_eq!(tinted, violet.quads.len(), "the neighbour keeps its own colours");
    assert!(both.quads.len() > tinted);
}
