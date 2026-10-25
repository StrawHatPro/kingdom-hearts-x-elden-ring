//! (DON-KIT) "Donald should not hover in the air when standing still while holding A, his feet should be planted on the
//! ground" (the user, 2026-10-22): the frames his held press and his charge hold (sheet rows `charge.pose`,
//! `step.wave_l.hold`, `step.wave_r.hold`) have his feet on the floor on his posed skeleton (the pack's own clips: KH3's
//! MagicCast hops from 0.37 s, the waves lift a foot around their shots). Skipped without the play pack.

use kh_party::pad::donald::K;
use kh_party::pad::donald_combo::{def, Step};
use kh_partyhost::poser::{PosedMember, DONALD};

/// The lowest leg bone of Donald above his actor origin (cm) at `time` s of clip row `row`.
fn foot_height(row: &str, time: f32) -> Option<f32> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/play/donpack/party");
    let dir = root.join("n_ex001");
    if !dir.join("manifest.json").is_file() {
        return None;
    }
    let read = |p: &std::path::Path| -> serde_json::Value { serde_json::from_str(std::fs::read_to_string(p).unwrap().trim_start_matches('\u{feff}')).unwrap() };
    let manifest = read(&dir.join("manifest.json"));
    let index = read(&root.join("party.json"));
    let entry = index["members"].as_array().into_iter().flatten().find(|m| m["member"].as_u64() == Some(DONALD as u64)).cloned().unwrap_or(serde_json::Value::Null);
    let animator = kh_anim::Animator::new(kh_anim::Pack::load(&dir).unwrap(), kh_anim::Settings { leg_ik: false, ..kh_anim::Settings::default() }).unwrap();
    let mut m = PosedMember::new(DONALD, animator, &manifest, &entry).unwrap();
    m.debug = Some(m.debug_pose(row, 0.0).ok()?);
    let names: Vec<String> = m.animator.skeleton().bones.iter().map(|b| b.name.clone()).collect();
    // to `time` in 1/600 s steps (the debug pose's clock runs from 0)
    let steps = (time * 600.0).round() as u64;
    m.step_debug(steps.max(1), time / steps.max(1) as f32, &kh_anim::NoIkWorld);
    names.iter().filter(|n| n.contains("ashi")).filter_map(|n| m.bone_world(n)).map(|t| t.translation.z - m.position.1.z).reduce(f32::min)
}

#[test]
fn his_held_frames_have_his_feet_on_the_floor() {
    let Some(idle) = foot_height("IdleB", 0.5) else {
        println!("no build/play/donpack: skipped");
        return;
    };
    let wl = def(Step::WaveL);
    let wr = def(Step::WaveR);
    for (row, t) in [(K.charge_clip, K.charge_pose), (wl.clip, wl.hold), (wr.clip, wr.hold)] {
        let h = foot_height(row, t).unwrap();
        println!("{row} at {t}: lowest foot {h:.1} cm (standing idle {idle:.1})");
        assert!(h < idle + 3.0, "{row} at {t}: his feet {h:.1} cm up (idle {idle:.1})");
    }
    // the old charge frame was in the cast's hop (why he hovered)
    assert!(foot_height(K.charge_clip, 0.42).unwrap() > idle + 10.0);
}
