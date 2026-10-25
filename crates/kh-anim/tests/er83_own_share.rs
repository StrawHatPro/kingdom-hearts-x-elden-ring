//! (ER83) The share of the worn outfit's own clips in the shown pose (kh-er-render's hand contact IK leaves those alone).
//! Without a pack with the kh1 outfit on disk the test says so and passes.

use kh_anim::{Animator, BodySlot, Frame, NoIkWorld, Pack, Settings, Trs};

fn step(a: &mut Animator, clip: &str, t: f32) {
    let seqs = [clip];
    let body = BodySlot { samples: [(clip, 1.0), (clip, 0.0)], blend_with: None, notify_sequences: &seqs, time: t, weight: 1.0, active: true, notify_window: (0.0, 0.0) };
    a.step(&Frame { dt: 1.0 / 60.0, body, component_to_world: Trs::IDENTITY }, &NoIkWorld);
}

#[test]
fn the_own_clips_share_follows_what_plays() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack");
    if !dir.join("outfits/kh1/anims").is_dir() {
        eprintln!("SKIPPED: no pack with the kh1 outfit");
        return;
    }
    let mut a = Animator::new(Pack::load(&dir).unwrap(), Settings::default()).unwrap();
    step(&mut a, "0003", 0.1);
    assert_eq!(a.own_share(), 0.0, "no outfit clips: KH3's");
    a.set_own_clips_except(Some(&dir.join("outfits/kh1")), &kh_anim::outfit_clips::kh3_clips(kh_anim::outfit_clips::default_for("kh1")));
    step(&mut a, "0003", 0.2);
    assert!((a.own_share() - 1.0).abs() < 1.0e-6, "KH1's own run: {}", a.own_share());
    step(&mut a, "1015", 0.0);
    assert!(a.own_share() < 1.0e-6, "KH3's battle stance: {}", a.own_share());
}
