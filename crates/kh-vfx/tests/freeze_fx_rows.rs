//! (FREEZE-FX, the user 2026-11-07: "make sure frozen effects are applied to anything that gets frozen by blizzard") KH3's frozen
//! look as the effect side runs it, on the pack's own effects when the pack is on disk: the ice row's `vs_abn_frz_in` drives the
//! victim's "Ice" material parameter to 1 and holds it for as long as the (persistent) row lives, the break's `vs_abn_frz_out0`
//! takes it back to 0; `vs_abn_frz0`'s smoke starts on the victim's bones (`SQEX_VertexSpawn` bUseAttachMeshVertex).

use std::collections::HashMap;
use std::sync::Arc;

use glam::{Quat, Vec3};
use kh_vfx::{Camera, DrawList, EffectDef, SpawnOptions, World, Xform};

const DT: f32 = 1.0 / 60.0;

fn pack_def(name: &str) -> Option<EffectDef> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../build/sora_pack/vfx/effects/{name}.json"));
    let text = std::fs::read_to_string(path).ok()?;
    Some(EffectDef::parse(&text).expect("the pack's effect parses"))
}

/// The last "Ice" drive of the frame (the latest spawn's), asserted kept (UseLastParameter).
fn ice(world: &World) -> Option<f32> {
    let mut list = DrawList::default();
    world.draw(1.0, &Camera::DEFAULT, &mut list);
    let p = list.char_params.iter().rev().find(|p| p.name == "Ice")?;
    assert!(p.keep, "UseLastParameter");
    Some(p.value[0])
}

/// KH3's ice row drives the victim's "Ice" material parameter 0 -> 1 over 0.3 s and its break back to 0, both as
/// UseLastParameter drives (`CharParam::keep`): the host keeps the in-row's 1 for the whole freeze (kh-host tests
/// freeze_fx_ice_drive.rs).
#[test]
fn freeze_fx_the_ice_rows_drive_ice_in_and_out_as_kept_drives() {
    let (Some(fin), Some(fout)) = (pack_def("vs_abn_frz_in"), pack_def("vs_abn_frz_out0")) else {
        eprintln!("SKIP: build/sora_pack not on disk");
        return;
    };
    let mut world = World::new();
    world.register(fin);
    world.register(fout);
    let _ = world.spawn("vs_abn_frz_in", Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
    let mut seen = Vec::new();
    for _ in 0..17 {
        world.step(DT);
        seen.push(ice(&world));
    }
    assert!(seen[0].is_some_and(|x| x < 0.2) && seen[16].is_some_and(|x| x > 0.95), "0 -> 1 over 0.3 s: {seen:?}");
    let _ = world.spawn("vs_abn_frz_out0", Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
    let mut after = Vec::new();
    for _ in 0..40 {
        world.step(DT);
        after.push(ice(&world));
    }
    assert!(after[10..18].iter().all(|v| v.is_some_and(|x| x < 0.05)), "the break takes it back to 0: {after:?}");
}

#[test]
fn freeze_fx_the_frost_smoke_starts_on_the_victims_bones() {
    let Some(def) = pack_def("vs_abn_frz0") else {
        eprintln!("SKIP: build/sora_pack not on disk");
        return;
    };
    let mut world = World::new();
    world.register(def);
    let id = world.spawn("vs_abn_frz0", Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
    assert!(world.wants_bones(id), "its smoke starts on the attached mesh: the host hands it bones");
    // a tall body: bones 3 to 5 m up, 2 m to the side
    let bones: HashMap<String, (Vec3, Quat)> = (0..6).map(|k| (format!("b{k}"), (Vec3::new(0.0, 200.0, 300.0 + 40.0 * k as f32), Quat::IDENTITY))).collect();
    world.set_bones(id, Some(Arc::new(bones)));
    let mut list = DrawList::default();
    for _ in 0..20 {
        world.step(DT);
    }
    world.draw(1.0, &Camera::DEFAULT, &mut list);
    assert!(!list.quads.is_empty(), "smoke drawn");
    let near = list.quads.iter().filter(|q| q.center.z > 200.0 && q.center.y > 100.0).count();
    assert!(near * 2 > list.quads.len(), "the smoke is on the bones, not at the root: {near} of {}", list.quads.len());
}
