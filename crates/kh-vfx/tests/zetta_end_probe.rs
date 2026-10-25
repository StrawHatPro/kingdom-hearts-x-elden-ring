//! (ZETTA) probe: the ending's beam (kg702c shot 960, vd_kg702b_96_zfr_00) at an identity placement: each mesh's
//! transform over time (its long axis, its thickness), so the shrink and the beam's direction can be read
use glam::Vec3;
use kh_vfx::{Camera, DrawList, EffectDef, SpawnOptions, World, Xform};

#[test]
#[ignore]
fn probe_end_beam() {
    let names: Vec<String> = std::env::var("ZETTA_PROBE").map(|s| s.split(',').map(String::from).collect()).unwrap_or_else(|_| vec!["vd_kg702b_96_zfr_00".into()]);
    for name in &names {
        let file = format!("build/sora_tmp/party/ZETTA/pack/vfx/effects/{name}.json");
        // (round 5c) ZETTA_PROBE_DIR: probe the effect files of another folder (a candidate fix)
        let path = match std::env::var("ZETTA_PROBE_DIR") {
            Ok(d) => std::path::Path::new(&d).join(format!("{name}.json")),
            Err(_) => std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().map(|a| a.join(&file)).find(|p| p.is_file()).unwrap(),
        };
        let def = EffectDef::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
        let mut w = World::new();
        w.register(def);
        w.spawn(name, Xform::default(), SpawnOptions { seed: Some(1), ..SpawnOptions::default() }).unwrap();
        for t in 0..480 {
            w.step(1.0 / 60.0);
            if t % 15 != 14 {
                continue;
            }
            let mut out = DrawList::default();
            w.draw(1.0, &Camera::DEFAULT, &mut out);
            for m in &out.meshes {
                let mesh = w.mesh_name(m.mesh).unwrap_or("?");
                let a = m.transform.matrix3;
                println!("{name} t{:.2} {mesh} o {:?} X{:?}|{:.1} Y{:?}|{:.1} Z{:?}|{:.1} rgba {:?}", (t + 1) as f32 / 60.0, m.transform.translation.round(), a.x_axis.normalize_or_zero(), a.x_axis.length(), a.y_axis.normalize_or_zero(), a.y_axis.length(), a.z_axis.normalize_or_zero(), a.z_axis.length(), m.color);
            }
            println!("{name} t{:.2} quads {} meshes {}", (t + 1) as f32 / 60.0, out.quads.len(), out.meshes.len());
        }
    }
}

/// (ZETTA round 5) the bone glows of the channel (vd_kg702_86_zfr_02 / 03): their sprites' size and colour at an identity
/// placement (the event: 90 cm x the attach's scale 1.2, colour 6 / 4.5 / 3, translucent, the material's soft fade)
#[test]
#[ignore]
fn probe_bone_glows() {
    for name in ["vd_kg702_86_zfr_02", "vd_kg702_86_zfr_03", "vd_kg702b_97_zfr_00"] {
        let file = format!("build/sora_tmp/party/ZETTA/pack/vfx/effects/{name}.json");
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().map(|a| a.join(&file)).find(|p| p.is_file()).unwrap();
        let def = EffectDef::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
        let mut w = World::new();
        w.register(def);
        w.spawn(name, Xform::default(), SpawnOptions { seed: Some(1), ..SpawnOptions::default() }).unwrap();
        for t in 0..120 {
            w.step(1.0 / 60.0);
            if t % 30 != 29 {
                continue;
            }
            let mut out = DrawList::default();
            w.draw(1.0, &Camera::DEFAULT, &mut out);
            let n = out.quads.len();
            let sizes: Vec<f32> = out.quads.iter().map(|q| (q.corners[0] - q.corners[2]).length() / std::f32::consts::SQRT_2).collect();
            let max = sizes.iter().copied().fold(0.0f32, f32::max);
            println!("{name} t{:.2} quads {n} size max {max:.1} first colour {:?} blend {:?}", (t + 1) as f32 / 60.0, out.quads.first().map(|q| q.color), out.quads.first().map(|q| q.blend));
        }
    }
}
