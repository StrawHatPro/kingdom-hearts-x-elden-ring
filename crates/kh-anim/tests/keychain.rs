//! The Kingdom Key's keychain (crates/kh-anim/src/keychain.rs) on the real weapon pack
//! (build/sora_pack/weapon): it hangs under gravity and settles, a swing of the weapon stays
//! bounded with no NaN, lags and settles again, and every frame equals the JS reference evaluator
//! (build/sora_tmp/keychain/swing_crosscheck.json, written by
//! `node build/sora_tmp/keychain/dump_swing.js`). Without the pack or the dump it says so.

use std::path::PathBuf;

use glam::{Quat, Vec3};
use kh_anim::json::Json;
use kh_anim::{Keychain, Trs};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn keychain_hangs_swings_and_equals_the_js() {
    let dir = root().join("build/sora_pack/weapon");
    if !dir.join("data/bonamik.json").is_file() {
        eprintln!("SKIPPED: no weapon pack at {}", dir.display());
        return;
    }
    let mut chain = Keychain::load(&dir).expect("the keychain sets up");
    assert_eq!(chain.skeleton().len(), 9);
    let names: Vec<&str> = chain.driven_bones().iter().map(|&i| chain.skeleton().bones[i].name.as_str()).collect();
    assert_eq!(names, ["chain1", "chain2", "chain3", "chain4", "chain5", "chain6", "chain7"], "the bones the solver may write");
    let reference = chain.reference().to_vec();
    let length: f32 = (3..9).map(|i| (reference[i].translation - reference[i - 1].translation).length()).sum();

    // The weapon's ComponentToWorld per 60 Hz step: 2 s at rest blade up, then 180 degrees about
    // world Y in 0.4 s around a point 60 cm up the blade, then held (blade down).
    let dump = std::fs::read_to_string(root().join("build/sora_tmp/keychain/swing_crosscheck.json")).ok().map(|t| Json::parse(&t).expect("the dump parses"));
    let frames: Vec<(Trs, Option<(u64, Vec<f32>)>)> = match &dump {
        Some(d) => d
            .get("frames")
            .items()
            .iter()
            .map(|f| {
                let (q, t) = (f.get("q").floats(), f.get("t").floats());
                (Trs { translation: Vec3::new(t[0], t[1], t[2]), rotation: Quat::from_xyzw(q[0], q[1], q[2], q[3]), scale: Vec3::ONE }, Some((f.get("hash").as_f64().unwrap() as u64, f.get("tip").floats())))
            })
            .collect(),
        None => {
            eprintln!("NOT COMPARED with the JS: no build/sora_tmp/keychain/swing_crosscheck.json");
            (0..420)
                .map(|i: i32| {
                    let t = ((i - 120).max(0) as f32 / 60.0).min(0.4) / 0.4;
                    let q = Quat::from_rotation_y((180.0 * t).to_radians()) * Quat::from_rotation_y((-90.0f32).to_radians());
                    (Trs { translation: Vec3::new(0.0, 0.0, 160.0) - q * Vec3::new(60.0, 0.0, 0.0), rotation: q, scale: Vec3::ONE }, None)
                })
                .collect()
        }
    };

    let (mut differ, mut compared) = (0, 0);
    let (mut max_reach, mut max_lag, mut last_move) = (0.0f32, 0.0f32, f32::MAX);
    let mut previous_tip = Vec3::ZERO;
    for (i, (world, want)) in frames.iter().enumerate() {
        let pose = chain.step(1.0 / 60.0, world, None).to_vec();
        for (bone, t) in pose.iter().enumerate() {
            assert!(t.translation.is_finite() && t.rotation.is_finite(), "frame {i}: bone {bone} is not finite");
        }
        // kh_sk and wep are not driven: bit-identical to the animated pose
        assert!(pose[0] == reference[0] && pose[1] == reference[1], "frame {i}: a bone the solver does not drive changed");
        let (root, tip) = (pose[2].translation, pose[8].translation);
        max_reach = max_reach.max((tip - root).length());
        if (120..150).contains(&i) {
            max_lag = max_lag.max((tip - reference[8].translation).length());
        }
        if i == 119 {
            // at rest, blade up: the chain hangs straight down (world -Z) at its full length
            let (root_w, tip_w) = (world.transform_point(root), world.transform_point(tip));
            let down = (root_w.z - tip_w.z) / (tip_w - root_w).length();
            assert!(down > 0.999, "the chain hangs towards world -Z at rest (cos {down})");
            for b in 3..9 {
                let (got, rest) = ((pose[b].translation - pose[b - 1].translation).length(), (reference[b].translation - reference[b - 1].translation).length());
                assert!((got - rest).abs() < 1.0e-3, "segment {b}: length {got} against {rest} at rest");
            }
            assert!((tip - previous_tip).length() < 1.0e-3, "the chain has settled after 2 s at rest");
        }
        last_move = (tip - previous_tip).length();
        previous_tip = tip;
        if let Some((hash, tip_js)) = want {
            compared += 1;
            let same_tip = tip_js.iter().zip(tip.to_array()).all(|(a, b)| a.to_bits() == b.to_bits());
            if chain.solver().state_hash() != *hash || !same_tip {
                differ += 1;
            }
        }
    }
    println!(
        "keychain: {} frames at 60 Hz, {} evaluations; chain length {length:.3} cm, largest root-tip distance {max_reach:.3} cm, largest tip lag during the swing {max_lag:.3} cm, tip motion on the last frame {last_move:.2e} cm; compared with the JS on {compared} frames, {differ} differ",
        frames.len(),
        chain.evaluations()
    );
    assert!(max_reach <= 2.0 * length, "the chain stays bounded");
    assert!(max_lag > 1.0, "the chain lags behind the swinging weapon");
    assert!(last_move < 0.05, "the chain comes to rest after the swing (it creeps over the guard collider for about 5 s)");
    assert_eq!(differ, 0, "frames whose solver state or tip bone differ from the JS evaluator");
}
