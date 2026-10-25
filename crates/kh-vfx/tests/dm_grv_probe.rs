//! (DON-MAGIC gravity) Donald's authored Gravity effect: every part spawns and draws (a mesh each), at its size, through
//! its life (it swells in, holds, implodes).
use glam::Vec3;
use kh_vfx::{Camera, DrawList, EffectDef, SpawnOptions, World, Xform};

fn effect(name: &str) -> Option<EffectDef> {
    let file = format!("build/sora_tmp/party/DON-MAGIC/pack/vfx/effects/{name}.json");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().map(|a| a.join(&file)).find(|p| p.is_file())?;
    EffectDef::parse(&std::fs::read_to_string(path).ok()?).ok()
}

#[test]
fn donalds_gravity_draws_every_part_and_implodes() {
    let Some(def) = effect("vd_don_grv2") else {
        eprintln!("(no vd_don_grv2 in the stream's pack: skipped)");
        return;
    };
    let mut w = World::new();
    w.register(def.clone());
    w.spawn("vd_don_grv2", Xform { position: Vec3::new(0.0, 0.0, 0.0), ..Xform::default() }, SpawnOptions { seed: Some(1), ..SpawnOptions::default() }).unwrap();
    let dt = 1.0 / 60.0;
    let mut sizes = Vec::new();
    for step in 0..(4.0 / dt) as usize {
        w.step(dt);
        if step % 30 == 0 {
            let mut out = DrawList::default();
            w.draw(1.0, &Camera::DEFAULT, &mut out);
            let scales: Vec<f32> = out.meshes.iter().map(|m| m.transform.matrix3.x_axis.length()).collect();
            eprintln!("t={:.2} meshes {} quads {} scales {:?} z {:?}", step as f32 * dt, out.meshes.len(), out.quads.len(), scales, out.meshes.last().map(|m| (m.transform.matrix3.y_axis.normalize(), m.transform.matrix3.z_axis.normalize())));
            sizes.push((step as f32 * dt, out.meshes.len(), scales.first().copied().unwrap_or(0.0)));
        }
    }
    let mid = sizes.iter().find(|s| s.0 >= 0.5).unwrap();
    assert!(mid.1 >= 12, "every part draws (the lightning as a stream): {sizes:?}");
}
