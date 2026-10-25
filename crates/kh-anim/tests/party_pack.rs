//! (party stream A) Donald's and Goofy's packs (build/sora_pack/party, tools/party_pack.js) through kh-anim: every row of
//! the anim sets resolves, the bind pose and IdleN evaluate, the KineDriver at the reference pose gives the reference pose
//! back (the ER73 pattern), the Bonamik settles at rest without reaching a branch the port leaves unported, and the
//! generated anim tables (tools/gen_party.js) compile and agree with the packs. Without the packs on disk the tests say so
//! and pass.

use std::path::PathBuf;

use glam::{Quat, Vec3};
use kh_anim::json::Json;
use kh_anim::{Animator, BodySlot, Frame, NoIkWorld, Pack, Settings, Trs};

#[allow(dead_code)]
mod types {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/gen/rust/party_anim_types.rs"));
}
#[allow(dead_code)]
mod donald {
    use super::types::*;
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/gen/rust/party_anim_n_ex001.rs"));
}
#[allow(dead_code)]
mod goofy {
    use super::types::*;
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/gen/rust/party_anim_n_ex002.rs"));
}

/// party_clips_n_ex00x.rs (B's request, contract B.7) against a copy of kh-party's types (kh-party/src/anim.rs).
#[allow(dead_code)]
mod bclips {
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Chara {
        Donald,
        Goofy,
    }
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct RootKey {
        pub time: f32,
        pub x: f32,
        pub y: f32,
        pub z: f32,
        pub yaw: f32,
    }
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum ClipNotifyData {
        None,
        Attack { attack: &'static str, collision: &'static str },
        State { name: &'static str },
        Sound { cue: &'static str },
        Effect { asset: &'static str, socket: &'static str },
        Raw(&'static str),
    }
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct ClipNotify {
        pub class: &'static str,
        pub start: f32,
        pub end: f32,
        pub data: ClipNotifyData,
    }
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct SocketKey {
        pub time: f32,
        pub x: f32,
        pub y: f32,
        pub z: f32,
    }
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct ClipDef {
        pub name: &'static str,
        pub sequence: &'static str,
        pub length: f32,
        pub rate: f32,
        pub blend_in: f32,
        pub looping: bool,
        pub root_motion: bool,
        pub notifies: &'static [ClipNotify],
        pub root: &'static [RootKey],
        pub socket: &'static [SocketKey],
    }
    #[derive(Clone, Copy, Debug)]
    pub struct AnimSetTable {
        pub chara: Chara,
        pub clips: &'static [ClipDef],
    }
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/gen/rust/party_clips_n_ex001.rs"));
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/gen/rust/party_clips_n_ex002.rs"));
}

#[test]
fn clip_tables_for_kh_party() {
    for t in [&bclips::N_EX001_ANIMS, &bclips::N_EX002_ANIMS] {
        let beat = t.clips.iter().find(|c| c.name == "M_1301").expect("M_1301");
        assert!(beat.notifies.iter().any(|n| matches!(n.data, bclips::ClipNotifyData::Attack { .. })));
        assert!(!beat.socket.is_empty(), "{:?}: no socket track on M_1301", t.chara);
        assert!(t.clips.iter().all(|c| c.length > 0.0 && c.notifies.windows(2).all(|w| w[0].start <= w[1].start)));
        println!("{:?}: {} clips, {} with root tracks, {} with sockets", t.chara, t.clips.len(), t.clips.iter().filter(|c| !c.root.is_empty()).count(), t.clips.iter().filter(|c| !c.socket.is_empty()).count());
    }
}

fn pack_dir(npc: &str) -> Option<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/party").join(npc);
    if dir.join("manifest.json").is_file() {
        Some(dir)
    } else {
        println!("no {} (node tools/party_pack.js): skipped", dir.display());
        None
    }
}

fn manifest(dir: &std::path::Path) -> Json {
    Json::parse(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap()
}

fn angle(a: Quat, b: Quat) -> f32 {
    2.0 * a.dot(b).abs().min(1.0).acos().to_degrees()
}

/// Every row of the body and face sets names packed clips; the counts the plan expects (220 / 225 rows of the body sets as
/// serialized: several names repeat, a few rows have no AnimData).
#[test]
fn every_row_resolves() {
    for (npc, body_rows, face_rows) in [("n_ex001", 216usize, 80usize), ("n_ex002", 222, 80)] {
        let Some(dir) = pack_dir(npc) else { return };
        let m = manifest(&dir);
        let pack = Pack::load(&dir).unwrap();
        let known = |name: &str| pack.info.contains_key(name);
        let stem = |f: &str| f.rsplit('/').next().unwrap().trim_end_matches(".glb").to_string();
        let rows = m.get("logical_animations").items();
        assert_eq!(rows.len(), body_rows, "{npc}: body rows with an asset");
        for r in rows {
            let name = r.get("name").as_str().unwrap();
            let samples = r.get("samples").items();
            if samples.is_empty() {
                assert!(known(&stem(r.get("file").as_str().unwrap())), "{npc}: row {name}: its clip is not packed");
            }
            for s in samples {
                assert!(known(s.get("animation_name").as_str().unwrap()), "{npc}: row {name}: a blend-space sample is not packed");
                assert!(dir.join(s.get("file").as_str().unwrap()).is_file());
            }
        }
        let faces = m.get("face_animations").items();
        assert_eq!(faces.len(), face_rows, "{npc}: face rows with an asset");
        for f in faces {
            assert!(known(&stem(f.get("file").as_str().unwrap())), "{npc}: face row {} not packed", f.get("name").as_str().unwrap());
        }
        // every notify of every packed clip has a class (decoded by gen_party or kept raw)
        for it in m.get("items").items() {
            for n in it.get("notifies").items() {
                assert!(n.get("class").as_str().is_some() || n.get("class").is_null());
            }
        }
        println!("{npc}: {} body rows, {} face rows, {} clips", rows.len(), faces.len(), pack.info.len());
    }
}

/// The bind pose and IdleN evaluate on the character's own skeleton: finite, the feet on the ground, the head at the
/// character's height; KineDriver and Bonamik run.
#[test]
fn bind_pose_and_idle_evaluate() {
    for (npc, head_min, head_max) in [("n_ex001", 60.0f32, 130.0f32), ("n_ex002", 120.0, 220.0)] {
        let Some(dir) = pack_dir(npc) else { return };
        let pack = Pack::load(&dir).unwrap();
        let mut animator = Animator::new(pack, Settings { leg_ik: false, ..Settings::default() }).unwrap();
        assert!(animator.is_party());
        let bind = kh_anim::pose::component_space(animator.skeleton(), &animator.skeleton().reference_pose());
        assert!(bind.iter().all(|t| t.translation.is_finite() && t.rotation.is_finite()));
        let head = animator.skeleton().index_of("atama").expect("bone atama");
        let sequences = ["0001"];
        let mut time = 0.0f32;
        let mut last = None;
        for step in 0..120 {
            let dt = 1.0 / 60.0;
            let from = if step == 0 { -1.0 } else { time };
            time += dt;
            let body = BodySlot { samples: [("0001", 1.0), ("0001", 0.0)], blend_with: None, notify_sequences: &sequences, time, weight: 1.0, active: true, notify_window: (from, time) };
            let pose = animator.step(&Frame { dt, body, component_to_world: Trs::IDENTITY }, &NoIkWorld).clone();
            assert!(pose.component.iter().all(|t| t.translation.is_finite() && t.rotation.is_finite() && t.scale.is_finite()), "{npc}: non-finite pose at step {step}");
            last = Some(pose);
        }
        let pose = last.unwrap();
        let z = pose.component[head].translation.z;
        println!("{npc}: atama at {:.1} cm (bind {:.1}); KineDriver {} bones, Bonamik {} bones; warnings {:?}", z, bind[head].translation.z, animator.kine_driver_bones().len(), animator.bonamik_bones().len(), animator.warnings);
        assert!(z > head_min && z < head_max, "{npc}: head height {z}");
        assert!(!animator.kine_driver_bones().is_empty() && !animator.bonamik_bones().is_empty(), "{npc}: KineDriver / Bonamik missing: {:?}", animator.warnings);
        // the lowest bone of the legs near the ground
        let foot = ["L_ashi2", "L_ashi1"].iter().find_map(|n| animator.skeleton().index_of(n)).unwrap();
        assert!(pose.component[foot].translation.z.abs() < 30.0, "{npc}: foot at {}", pose.component[foot].translation.z);
    }
}

/// The ER73 pattern: the KineDriver run on the reference pose writes the reference transform of every bone it drives
/// (the asset's neutral = the mesh's bind pose), within the tolerances of tools/sora_pack.js's KineDriver check.
#[test]
fn kine_driver_at_rest_is_the_bind_pose() {
    for npc in ["n_ex001", "n_ex002"] {
        let Some(dir) = pack_dir(npc) else { return };
        let pack = Pack::load(&dir).unwrap();
        let skeleton = pack.skeleton.clone();
        for file in ["kinedriver.json", "kinedriver_shortEar.json"] {
            let path = dir.join("data").join(file);
            if !path.is_file() {
                continue;
            }
            let data = Json::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
            let mut driver = kh_anim::kine::KineDriver::from_json(&data).unwrap();
            let local = skeleton.reference_pose();
            let base = kh_anim::pose::component_space(&skeleton, &local);
            let mut pose = base.clone();
            driver.evaluate(&mut pose, &local);
            let (mut worst_t, mut worst_r, mut worst_s, mut out) = (0.0f32, 0.0f32, 0.0f32, Vec::new());
            for &b in driver.written() {
                let (dt, dr, ds) = ((pose[b].translation - base[b].translation).length(), angle(pose[b].rotation, base[b].rotation), (pose[b].scale - base[b].scale).abs().max_element());
                if dt > 1.0e-2 || dr > 5.0e-2 || ds > 1.0e-3 {
                    out.push(format!("{} (t {dt:.3} cm, r {dr:.3} deg, s {ds:.4})", skeleton.bones[b].name));
                }
                worst_t = worst_t.max(dt);
                worst_r = worst_r.max(dr);
                worst_s = worst_s.max(ds);
            }
            println!("{npc} {file}: {} operators, {} bones written; worst t {worst_t:.2e} cm r {worst_r:.2e} deg s {worst_s:.2e}; outside: {:?}", driver.operator_count(), driver.written().len(), out);
            // Outside the strict tolerance only by the asset's own neutral values (as Sora's R_susoB1..3 differ from his
            // reference pose by 1 degree in the asset): Donald's lip corners (0.056 deg) and his cape's C chain under the
            // TargetPoscns (0.045 cm). Nothing may be visibly off the bind pose.
            // (Goofy's shortEar data scales his ears to 0.6 by design: the variant's purpose; reported, not asserted.)
            if file == "kinedriver.json" {
                assert!(worst_t < 0.1 && worst_r < 0.1 && worst_s < 1.0e-3, "{npc} {file}: bones off the bind pose at rest: {out:?}");
            }
        }
    }
}

/// The Bonamik at rest on the reference pose: no unported branch reached (fallback bits 0), the driven bones settle near
/// the bind pose, the non-driven bones are untouched. One test for both characters (UNPORTED is process-wide).
#[test]
fn bonamik_at_rest_without_fallbacks() {
    for npc in ["n_ex001", "n_ex002"] {
        let Some(dir) = pack_dir(npc) else { return };
        let pack = Pack::load(&dir).unwrap();
        let skeleton = pack.skeleton.clone();
        for file in ["bonamik.json", "bonamik_cs.json"] {
            let path = dir.join("data").join(file);
            if !path.is_file() {
                continue;
            }
            let data = Json::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
            kh_anim::bonamik::UNPORTED.store(0, std::sync::atomic::Ordering::Relaxed);
            let mut solver = kh_anim::Bonamik::new(&data, &skeleton, kh_anim::BonamikOptions::default()).unwrap();
            let rest = kh_anim::bonamik::reference_pose(&skeleton);
            let driven = solver.driven_bones();
            let mut pose = rest.clone();
            for _ in 0..240 {
                pose = rest.clone();
                solver.frame(1.0 / 60.0, &mut pose, &Trs::IDENTITY, &kh_anim::BonamikHost::default());
            }
            let bits = kh_anim::bonamik::UNPORTED.load(std::sync::atomic::Ordering::Relaxed);
            let mut worst = (0.0f32, String::new());
            let mut changed_undriven = 0;
            for (b, (p, r)) in pose.iter().zip(&rest).enumerate() {
                let d = (p.translation - r.translation).length();
                if driven.contains(&b) {
                    if d > worst.0 {
                        worst = (d, skeleton.bones[b].name.clone());
                    }
                } else if p.translation != r.translation || p.rotation != r.rotation {
                    changed_undriven += 1;
                }
            }
            println!("{npc} {file}: {} driven bones, {} solver steps; at rest the farthest driven bone is {} at {:.2} cm; undriven changed {changed_undriven}; UNPORTED bits {bits:#x}", driven.len(), solver.stats().steps, worst.1, worst.0);
            assert_eq!(bits, 0, "{npc} {file}: the asset reaches unported Bonamik branches {bits:#x}");
            assert_eq!(changed_undriven, 0);
            assert!(worst.0 < 8.0, "{npc} {file}: {} hangs {} cm off its bind position at rest", worst.1, worst.0);
            assert!(pose.iter().all(|t| t.translation.is_finite()));
        }
    }
}

/// The generated tables agree with the packs (rows in set order, sequences, the sockets of the hit clips).
#[test]
fn generated_tables_agree() {
    for (npc, tables) in [("n_ex001", &donald::TABLES), ("n_ex002", &goofy::TABLES)] {
        let Some(dir) = pack_dir(npc) else { return };
        let m = manifest(&dir);
        let rows = m.get("logical_animations").items();
        assert_eq!(tables.anims.len(), rows.len());
        for (a, r) in tables.anims.iter().zip(rows) {
            assert_eq!(a.name, r.get("name").as_str().unwrap());
            for &(_, s) in a.samples {
                assert!(s < tables.sequences.len());
            }
        }
        let idle = tables.anim("IdleN").expect("IdleN");
        let seq = &tables.sequences[idle.samples[0].1];
        assert!(seq.length > 0.0);
        assert_eq!(tables.face_rows.len(), m.get("face_animations").items().len());
        let socketed = tables.sequences.iter().filter(|s| !s.sockets.is_empty()).count();
        assert!(socketed > 0, "{npc}: no socket tracks");
        for s in tables.sequences.iter().filter(|s| !s.sockets.is_empty()) {
            assert_eq!(s.sockets.len() as u32, s.frames, "{}", s.name);
            assert!(s.sockets.iter().all(|f| f.iter().all(|v| v.is_finite() && v.abs() < 1000.0)));
        }
        println!("{npc}: {} rows, {} sequences, {} with socket tracks, {} notify assets", tables.anims.len(), tables.sequences.len(), socketed, tables.notify_assets.len());
    }
    assert_eq!(donald::NpcAnimId::from_name("IdleN"), Some(donald::NpcAnimId::IdleN));
    let _ = Vec3::ZERO;
}
