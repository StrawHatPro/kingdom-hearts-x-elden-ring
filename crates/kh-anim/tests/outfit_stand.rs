//! (ER79) KH1 Sora's standing pose: KH3 Sora's stand clips in place of the KH1 outfit's own, on the real pack.
//! Without a pack on disk the tests say so and pass.

use std::path::PathBuf;

use kh_anim::pose::component_space;
use kh_anim::{Animator, Pack, Settings};

fn pack_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack")
}

fn animator() -> Option<Animator> {
    let dir = pack_dir();
    if !dir.join("manifest.json").is_file() {
        eprintln!("SKIPPED: no pack at {}", dir.display());
        return None;
    }
    Some(Animator::new(Pack::load(&dir).expect("the pack loads"), Settings::default()).expect("animator"))
}

#[test]
#[ignore]
fn probe_stand_clips() {
    let Some(mut a) = animator() else { return };
    let sk = a.skeleton().clone();
    let bone = |n: &str| sk.index_of(n).unwrap();
    let (hand, buki, hips, la, ra, head) = (bone("R_te"), bone("R_buki"), bone("koshi"), bone("L_ashi1"), bone("R_ashi1"), bone("atama"));
    let report = |a: &Animator, label: &str, clip: &str| {
        let Some(info) = a.clip_info(clip) else { println!("{label} {clip}: none"); return };
        let len = info.length;
        for t in [0.0, len * 0.5, len] {
            let c = component_space(&sk, &a.sample(clip, t));
            let p = |i: usize| c[i].translation;
            println!(
                "{label} {clip} len {len:.2} t {t:.2}: buki-hand {:.1} buki {:.0?} hand {:.0?} hips {:.0?} feet {:.0?} {:.0?} head {:.0?}",
                (p(buki) - p(hand)).length(),
                p(buki).to_array(),
                p(hand).to_array(),
                p(hips).to_array(),
                p(la).to_array(),
                p(ra).to_array(),
                p(head).to_array()
            );
        }
    };
    let clips = ["0001", "0004", "1004", "1015", "1018", "1014", "1028", "0005", "0006", "0011", "0012", "0034", "0120", "0007", "0003"];
    for c in clips {
        report(&a, "KH3", c);
    }
    let own = a.set_own_clips(Some(&pack_dir().join("outfits").join("kh1")));
    println!("own: {own:?}");
    for c in clips {
        if own.iter().any(|o| o == c) {
            report(&a, "KH1", c);
        }
    }
}

/// (ER79) Replays the KH1 outfit's Bonamik on poses dumped by the effects harness (`--dump-sim`), tracing one bone.
/// `KH_SIM_DUMP=file KH_SIM_SKIP=n KH_SIM_BONE=name cargo test --release -p kh-anim --test outfit_stand -- --ignored replay`
#[test]
#[ignore]
fn replay_outfit_sim() {
    let Ok(file) = std::env::var("KH_SIM_DUMP") else { return };
    let skip: usize = std::env::var("KH_SIM_SKIP").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let bone_name = std::env::var("KH_SIM_BONE").unwrap_or("L_zubonE2".into());
    let dir = pack_dir().join("outfits").join("kh1");
    let manifest = kh_anim::json::Json::parse(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    let skeleton = kh_anim::Skeleton::from_manifest(&manifest).unwrap();
    let data = kh_anim::json::Json::parse(&std::fs::read_to_string(dir.join("bonamik.json")).unwrap()).unwrap();
    let mut sim = kh_anim::Bonamik::new(&data, &skeleton, kh_anim::BonamikOptions::default()).unwrap();
    let bytes = std::fs::read(file).unwrap();
    let floats: Vec<f32> = bytes.chunks(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    let per = 10 * (1 + skeleton.bones.len());
    let trs = |f: &[f32]| kh_anim::Trs { translation: glam::Vec3::new(f[0], f[1], f[2]), rotation: glam::Quat::from_xyzw(f[3], f[4], f[5], f[6]), scale: glam::Vec3::new(f[7], f[8], f[9]) };
    let bone = skeleton.index_of(&bone_name).unwrap();
    let mut last: Option<Vec<glam::Vec3>> = None;
    for (k, rec) in floats.chunks(per).enumerate().skip(skip) {
        let to_world = trs(&rec[0..10]);
        let mut pose: Vec<kh_anim::Trs> = rec[10..].chunks(10).map(trs).collect();
        let before = pose[bone].translation;
        if std::env::var("KH_SIM_SCALES").is_ok() {
            let names = [bone_name.replace('2', "1"), bone_name.clone(), bone_name.replace('2', "3")];
            let txt: Vec<String> = names.iter().filter_map(|n| skeleton.index_of(n).map(|b| format!("{n} s {:.3?} q|{:.3}| t {:.1?}", pose[b].scale.to_array(), pose[b].rotation.length(), pose[b].translation.to_array()))).collect();
            println!("      input: {}", txt.join(" ; "));
        }
        let info = sim.frame(1.0 / 60.0, &mut pose, &to_world, &kh_anim::BonamikHost::default());
        let after = pose[bone].translation;
        println!("{:4} eval {} dt {:.4} alpha {:.2}: {bone_name} in {:.1?} out {:.1?} (off {:.1})", k - skip, info.did_eval as u8, info.eval_dt, info.alpha, before.to_array(), after.to_array(), (after - before).length());
        let now = sim.particle_positions();
        if let Ok(list) = std::env::var("KH_SIM_PARTICLES") {
            let parts: Vec<String> = list.split(',').filter_map(|x| x.parse::<usize>().ok()).map(|i| format!("p{i} {:.1?}", now[i].to_array())).collect();
            let e1 = skeleton.index_of(&bone_name.replace('2', "1")).map(|b| pose[b].translation.to_array());
            println!("      {} | parent-chain bone {:?}", parts.join(" "), e1.map(|v| format!("{v:.1?}")));
        }
        if let Some(prev) = &last {
            let names = particle_names(&data);
            let jumps: Vec<String> = now.iter().zip(prev.iter()).enumerate().filter(|(_, (a, b))| (**a - **b).length() > 30.0).map(|(i, (a, b))| format!("p{i} {} {:.0} ({:.0?})", names.get(i).map_or("?", |s| s.as_str()), (*a - *b).length(), a.to_array())).collect();
            if !jumps.is_empty() {
                println!("      particles jumped: {}", jumps.join("; "));
            }
        }
        last = Some(now);
    }
}

fn particle_names(data: &kh_anim::json::Json) -> Vec<String> {
    data.get("bodies").items().iter().map(|b| format!("{}:{}{}", b.get("joint").as_str().unwrap_or("?"), b.get("colShape").as_str().unwrap_or("?"), if b.get("kinematic").as_bool() == Some(true) { "/kin" } else { "" })).collect()
}

/// (ER79) KH1's default: KH3 Sora's stand, stops and landings (retargeted by the renderer), its own run, walk, jump.
#[test]
fn kh1_takes_kh3s_standing_pose_and_keeps_its_own_run() {
    let Some(mut a) = animator() else { return };
    let dir = pack_dir().join("outfits").join("kh1");
    if !dir.join("anims").is_dir() {
        return;
    }
    let all = a.set_own_clips(Some(&dir));
    assert!(all.iter().any(|c| c == "0001"), "KH1 has its own idle: {all:?}");
    let kh3 = kh_anim::outfit_clips::kh3_clips(kh_anim::outfit_clips::default_for("kh1"));
    let own = a.set_own_clips_except(Some(&dir), &kh3);
    for c in ["0001", "0005", "0006", "0011", "0012", "0034"] {
        assert!(!own.iter().any(|o| o == c), "{c} must be KH3 Sora's: {own:?}");
    }
    for c in ["0002", "0003", "0007", "0031", "0032", "0033", "0000EM"] {
        assert!(own.iter().any(|o| o == c), "{c} stays KH1's own: {own:?}");
    }
    // KH3's idle is sampled again (not KH1's): its pose equals the pack clip's
    let sk = a.skeleton().clone();
    let hand = sk.index_of("R_te").unwrap();
    let mine = component_space(&sk, &a.sample("0001", 1.0))[hand].translation;
    a.set_own_clips(None);
    let his = component_space(&sk, &a.sample("0001", 1.0))[hand].translation;
    assert!((mine - his).length() < 1.0e-4);
    // `0` = every own clip back
    assert_eq!(a.set_own_clips_except(Some(&dir), &kh_anim::outfit_clips::kh3_clips("0")), all);
}
