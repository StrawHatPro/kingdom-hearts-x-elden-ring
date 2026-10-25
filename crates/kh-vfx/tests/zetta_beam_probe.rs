//! (ZETTA) probe: where the event's beam meshes extend at an identity placement (local axes, lengths)
use glam::Vec3;
use kh_vfx::{Camera, DrawList, EffectDef, SpawnOptions, World, Xform};

#[test]
#[ignore]
fn probe_beams() {
    for name in ["vd_kg702_92_zfr_01", "vd_kg702_92_zfr_03", "vd_kg702_93_zfr_00", "vd_kg702_94_zfr_01", "vd_kg702_94_zfr_00", "vd_kg702_92_zfr_02", "vd_kg702_91_zfr_00", "vd_kg702_94_exp_00"] {
        let file = format!("build/sora_tmp/party/ZETTA/pack/vfx/effects/{name}.json");
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().map(|a| a.join(&file)).find(|p| p.is_file()).unwrap();
        let def = EffectDef::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
        let mut w = World::new();
        w.register(def);
        w.spawn(name, Xform::default(), SpawnOptions { seed: Some(1), ..SpawnOptions::default() }).unwrap();
        for t in 0..90 {
            w.step(1.0 / 60.0);
            if t % 30 != 29 {
                continue;
            }
            let mut out = DrawList::default();
            w.draw(1.0, &Camera::DEFAULT, &mut out);
            for m in &out.meshes {
                let mesh = w.mesh_name(m.mesh).unwrap_or("?");
                let c = [Vec3::new(0.0, -25.0, -25.0), Vec3::new(0.0, 25.0, 25.0), Vec3::new(0.0, 25.0, -25.0), Vec3::new(0.0, -25.0, 25.0), Vec3::ZERO, Vec3::new(0.0, 0.0, 25.0), Vec3::new(0.0, 0.0, -25.0)];
                let p: Vec<Vec3> = c.iter().map(|v| m.transform.transform_point3(*v)).collect();
                let (lo, hi) = p.iter().fold((Vec3::splat(1e9), Vec3::splat(-1e9)), |a, v| (a.0.min(*v), a.1.max(*v)));
                println!("{name} t{:.2} {mesh} centre {:?} z+ {:?} z- {:?} box {:?}..{:?} axes X{:?} Y{:?} Z{:?}", (t + 1) as f32 / 60.0, p[4].round(), p[5].round(), p[6].round(), lo.round(), hi.round(), m.transform.matrix3.x_axis.round(), m.transform.matrix3.y_axis.round(), m.transform.matrix3.z_axis.round());
            }
            let qs: Vec<Vec3> = out.quads.iter().map(|q| (q.corners[0] + q.corners[2]) * 0.5).collect();
            if !qs.is_empty() {
                let (lo, hi) = qs.iter().fold((Vec3::splat(1e9), Vec3::splat(-1e9)), |a, v| (a.0.min(*v), a.1.max(*v)));
                println!("{name} t{:.2} quads {} box {:?}..{:?}", (t + 1) as f32 / 60.0, qs.len(), lo.round(), hi.round());
            }
        }
    }
}
