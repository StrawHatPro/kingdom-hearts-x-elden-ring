//! A probe, not a test: `KH_VFX_PROBE=vs_fir_hit0 cargo test -p kh-vfx --test probe -- --nocapture`
//! runs one effect of the pack at (0, 0, 100) for `KH_VFX_STEPS` steps (default 90, loop end at
//! two thirds) and prints what it draws every sixth step. Without the variable it does nothing.

use glam::Vec3;
use kh_vfx::{Camera, DrawList, EffectDef, SpawnOptions, World, Xform};

#[test]
fn probe() {
    let Ok(name) = std::env::var("KH_VFX_PROBE") else { return };
    let steps: usize = std::env::var("KH_VFX_STEPS").ok().and_then(|s| s.parse().ok()).unwrap_or(90);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/vfx/effects").join(format!("{name}.json"));
    let def = EffectDef::parse(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap();
    for e in &def.emitters {
        println!("emitter {:<22} {:?} ({}){}{} loops {} time {:.3}/{:.3}/{:.3}/{:.3} material {} mesh {:?} modules {} unknown {:?}", e.name, e.kind, e.kind_name, if e.auto_add { "" } else { " CHILD" }, if e.local_space { " LOCAL" } else { "" }, e.loops, e.start_time, e.loop_time, e.loop_duration, e.duration_time, e.material, e.mesh, e.modules.len(), e.unknown);
    }
    let mut world = World::new();
    world.register(def);
    let id = world.spawn(&name, Xform::at(Vec3::new(0.0, 0.0, 100.0)), SpawnOptions::default()).unwrap();
    let mut list = DrawList::default();
    for step in 0..steps {
        if step == steps * 2 / 3 {
            world.stop(id, false);
            println!("-- loop end");
        }
        world.set_trail(id, Vec3::new(step as f32 * 5.0, 0.0, 180.0), Vec3::new(step as f32 * 5.0, 0.0, 115.0));
        world.step(1.0 / 60.0);
        if std::env::var("KH_VFX_PROBE_AT").ok().and_then(|v| v.parse::<usize>().ok()) == Some(step) {
            for (k, e) in world.probe(id).iter().enumerate() {
                println!("  [{k:3}] {:<24} parent {:?} rel {:.2} at {:?} scale {:?} particles {} max size {:.1} max dist {:.1}{}", e.name, e.parent, e.rel, e.position.round(), (e.scale * 100.0).round() / 100.0, e.particles, e.max_size, e.max_distance, if e.done { " DONE" } else { "" });
            }
        }
        if step % 6 == 0 {
            list.clear();
            world.draw(1.0, &Camera::DEFAULT, &mut list);
            let (_, emitters, particles) = world.stats();
            println!("step {step:3}: alive {} emitters {emitters} particles {particles} | quads {} ribbons {} meshes {}", world.is_alive(id), list.quads.len(), list.ribbons.len(), list.meshes.len());
            let mut seen = std::collections::BTreeMap::new();
            for q in &list.quads {
                let e = seen.entry(q.material).or_insert((0usize, *q));
                e.0 += 1;
            }
            for (material, (count, q)) in seen {
                println!("      {count:4} quads {:<24} first: centre {:?} width {:.1} colour [{:.2} {:.2} {:.2} {:.2}]", world.material_name(material).unwrap_or("-"), ((q.corners[0] + q.corners[2]) * 0.5).round(), q.corners[0].distance(q.corners[1]), q.color[0], q.color[1], q.color[2], q.color[3]);
            }
            for m in list.meshes.iter().take(6) {
                let s = Vec3::new(m.transform.matrix3.x_axis.length(), m.transform.matrix3.y_axis.length(), m.transform.matrix3.z_axis.length());
                println!("      mesh {:<22} {:<22} at {:?} scale {:?} x-axis {:?} colour [{:.2} {:.2} {:.2} {:.2}]", world.mesh_name(m.mesh).unwrap_or("-"), world.material_name(m.material).unwrap_or("-"), Vec3::from(m.transform.translation).round(), (s * 100.0).round() / 100.0, (Vec3::from(m.transform.matrix3.x_axis).normalize_or_zero() * 100.0).round() / 100.0, m.color[0], m.color[1], m.color[2], m.color[3]);
            }
            for r in list.ribbons.iter().take(4) {
                println!("      ribbon {:<22} {} points, head {:?} / {:?} tail {:?} colour [{:.2} {:.2} {:.2} {:.2}]", world.material_name(r.material).unwrap_or("-"), r.points.len(), r.points[0].a.round(), r.points[0].b.round(), r.points[r.points.len() - 1].a.round(), r.points[0].color[0], r.points[0].color[1], r.points[0].color[2], r.points[0].color[3]);
            }
        }
    }
    for line in world.take_log() {
        println!("{line}");
    }
}

/// A census, not a test: `KH_VFX_PEAKS=1 cargo test --release -p kh-vfx --test probe peaks -- --nocapture`
/// runs every effect of the pack (loop end after 1.5 s, 4 s in all) and compares the most particles each
/// emitter held with `peak_active_particles` of the cooked data (the cooker's own simulation: a GPU sprite
/// emitter's hard capacity, a CPU emitter's first allocation). KH_VFX_GPU_CAP=100000 lifts the GPU cap first.
#[test]
fn peaks() {
    if std::env::var_os("KH_VFX_PEAKS").is_none() {
        return;
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/vfx/effects");
    let (mut emitters, mut over, mut far_over, mut under) = (0, 0, 0, 0);
    let mut worst: Vec<(f32, String)> = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        let text = std::fs::read_to_string(entry.path()).unwrap();
        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        let Ok(def) = EffectDef::from_json(&json) else { continue };
        let name = def.name.clone();
        let cooked: std::collections::HashMap<String, (f32, String)> = json["emitters"].as_array().unwrap().iter().map(|e| (e["name"].as_str().unwrap_or("").to_string(), (e["peak_active_particles"].as_f64().unwrap_or(0.0) as f32, e["typedata"]["kind"].as_str().unwrap_or("").to_string()))).collect();
        let mut world = World::new();
        world.register(def);
        let Some(id) = world.spawn(&name, Xform::at(Vec3::new(0.0, 0.0, 100.0)), SpawnOptions::default()) else { continue };
        let mut peak: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for step in 0..240 {
            if step == 90 {
                world.stop(id, false);
            }
            world.step(1.0 / 60.0);
            // Children of one template add up, as they share nothing in the game either: per instance.
            for e in world.probe(id) {
                let entry = peak.entry(e.name).or_default();
                *entry = (*entry).max(e.particles);
            }
        }
        for (emitter, most) in peak {
            let Some((cooked_peak, kind)) = cooked.get(&emitter) else { continue };
            if !matches!(kind.as_str(), "sprite" | "gpu_sprite" | "mesh") || *cooked_peak < 1.0 {
                continue;
            }
            emitters += 1;
            let ratio = most as f32 / cooked_peak;
            if ratio > 1.25 {
                over += 1;
            }
            if ratio > 3.0 {
                far_over += 1;
                worst.push((ratio, format!("{name}:{emitter} ({kind}) {most} against {cooked_peak}")));
            }
            if ratio < 0.5 {
                under += 1;
            }
        }
    }
    worst.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!("peaks: {emitters} sprite / mesh emitters with a cooked peak; the evaluator held more than 1.25 x in {over}, more than 3 x in {far_over}, less than half in {under}");
    for (ratio, line) in worst.iter().take(25) {
        println!("  {ratio:6.1} x  {line}");
    }
}

/// (fourth pass) THE LINGERING-EFFECT AUDIT over the whole pack (skipped without the pack): every
/// effect, spawned, asked to end after 1.5 s (`stop(id, false)` = the game's EnableLoopEnd) and
/// stepped 30 s more, is gone, with every system it spawned from inside. Before this pass 1,298
/// emitters held a particle without a lifetime for ever (the Ragnarok charge star on the Keyblade).
#[test]
fn every_effect_of_the_pack_ends_after_loop_end() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/vfx/effects");
    let Ok(entries) = std::fs::read_dir(&dir) else { return };
    let mut world = kh_vfx::World::new();
    let mut names = Vec::new();
    for entry in entries.flatten() {
        let Ok(text) = std::fs::read_to_string(entry.path()) else { continue };
        if let Ok(def) = kh_vfx::EffectDef::parse(&text) {
            names.push(def.name.clone());
            world.register(def);
        }
    }
    world.set_ground(Some(0.0));
    let mut lingering = Vec::new();
    for name in &names {
        let Some(id) = world.spawn(name, kh_vfx::Xform::at(glam::Vec3::new(0.0, 0.0, 100.0)), kh_vfx::SpawnOptions::default()) else { continue };
        for _ in 0..90 {
            world.step(1.0 / 60.0);
        }
        world.stop(id, false);
        for _ in 0..1800 {
            world.step(1.0 / 60.0);
            if world.stats().0 == 0 {
                break;
            }
        }
        for l in world.live() {
            lingering.push(format!("{} (spawned as {name}): {:?}", l.name, l.emitters));
            world.stop(l.id, true);
        }
        world.step(1.0 / 60.0);
    }
    assert!(lingering.is_empty(), "{} of {} effects never end:\n{}", lingering.len(), names.len(), lingering.join("\n"));
}

/// (IMPL_ER20) The HP prize of the pack, drawn at mid-loop: the layers stand in the order the data
/// asks for (outline disc glw_RB0 `DepthOffset` -10 BEHIND the ball, glint glw_Y +30 in FRONT), and
/// the ball shows the camera its 36 cm circle (the lens rule) with its 10 cm axis along the view.
/// The munny gem and the MP ball keep their own sizes.
#[test]
fn the_prize_orbs_stand_in_their_layers() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/vfx/effects");
    let load = |name: &str| EffectDef::from_json(&serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{name}.json"))).unwrap()).unwrap()).unwrap();
    let camera = Camera { position: Vec3::new(-500.0, 0.0, 0.0), forward: Vec3::X, right: Vec3::Y, up: Vec3::Z };
    let run = |name: &str| {
        let mut world = World::new();
        world.register(load(name));
        world.spawn(name, Xform::at(Vec3::ZERO), SpawnOptions::default()).unwrap();
        for _ in 0..30 {
            world.step(1.0 / 60.0);
        }
        let mut list = kh_vfx::DrawList::default();
        world.draw(1.0, &camera, &mut list);
        list
    };
    let depth = |p: Vec3| (p - camera.position).dot(camera.forward);
    let hp = run("vs_plz_hpb0");
    assert_eq!((hp.meshes.len(), hp.quads.len()), (1, 2), "the ball, its outline disc and its glint");
    let ball = &hp.meshes[0];
    let disc = hp.quads.iter().find(|q| q.mode == kh_vfx::Blend::Translucent).expect("glw_RB0");
    let glint = hp.quads.iter().find(|q| q.mode == kh_vfx::Blend::Additive).expect("glw_Y");
    let at = depth(Vec3::from(ball.transform.translation));
    assert!((depth(disc.center) - at - 10.0).abs() < 0.05, "the outline disc is 10 cm behind the ball: {}", depth(disc.center) - at);
    assert!((depth(glint.center) - at + 30.0).abs() < 0.5, "the glint is 30 cm in front: {}", depth(glint.center) - at);
    // The disc is 65 cm across (its texture's solid part about 35), the ball 36 x 36 on screen and 10 deep.
    assert!((disc.size - 65.0).abs() < 0.01);
    let m = ball.transform.matrix3;
    let extent = |axis: Vec3| (Vec3::from(m.x_axis).dot(axis).abs() + Vec3::from(m.y_axis).dot(axis).abs() + Vec3::from(m.z_axis).dot(axis).abs()) * 100.0;
    assert!((extent(camera.right) - 36.0).abs() < 0.1 && (extent(camera.up) - 36.0).abs() < 0.1 && (extent(camera.forward) - 10.0).abs() < 0.1, "{} x {} x {}", extent(camera.right), extent(camera.up), extent(camera.forward));
    // The switches give the readings before this pass.
    kh_vfx::set_lens_rule(false);
    kh_vfx::set_depth_offset_toward_camera(false);
    let old = run("vs_plz_hpb0");
    kh_vfx::set_lens_rule(true);
    kh_vfx::set_depth_offset_toward_camera(true);
    let m = old.meshes[0].transform.matrix3;
    let wide = (Vec3::from(m.x_axis).dot(camera.right).abs() + Vec3::from(m.y_axis).dot(camera.right).abs() + Vec3::from(m.z_axis).dot(camera.right).abs()) * 100.0;
    assert!((wide - 10.0).abs() < 0.1, "the literal reading: a 10 cm sliver ({wide})");
    let disc = old.quads.iter().find(|q| q.mode == kh_vfx::Blend::Translucent).unwrap();
    assert!(depth(disc.center) < depth(Vec3::from(old.meshes[0].transform.translation)));
    // MP: a 36 cm ball, its 25 cm rim shell 10 cm nearer (stock CameraOffset), the glow 10 cm behind, the glint in front.
    let mp = run("vs_plz_mpb0");
    assert_eq!((mp.meshes.len(), mp.quads.len()), (2, 2));
    let glow = mp.quads.iter().find(|q| (q.size - 65.0).abs() < 0.01).expect("glw_bg0");
    assert!(depth(glow.center) > 9.9 + depth(Vec3::ZERO) && mp.quads.iter().any(|q| depth(q.center) < depth(Vec3::ZERO) - 29.0));
    // Munny: one gem, 0.35 of its mesh (183.6 cm tip to tip: 64 cm at the large prize's scale 1).
    let munny = run("VS_PLZ_MNY0");
    assert_eq!((munny.meshes.len(), munny.quads.len()), (1, 0));
    let m = munny.meshes[0].transform.matrix3;
    assert!((Vec3::from(m.z_axis).length() - 0.35).abs() < 1.0e-3, "{}", Vec3::from(m.z_axis).length());
}
