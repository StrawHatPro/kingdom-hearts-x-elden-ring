//! (KEYCHAIN-DUAL) Sora's clip Bonamik states reach the weapons (KH3's "children" calls, crates/kh-anim/src/keychain.rs ROUTED
//! NOTIFIES): the Animator lists the CharaWind / SpeedRate states active in the clips he plays (every clip's, not only
//! 0033's: `Animator::child_bonamik_states`), and `Keychain::route_states` routes them: clip 0852S's wind names `KEY` and
//! blows the Kingdom Key's chain. On the real pack; without it the test says so.

use std::path::PathBuf;

use glam::{Quat, Vec3};
use kh_anim::{Animator, BodySlot, Frame, Keychain, NoIkWorld, Pack, Settings, Trs};

const DT: f32 = 1.0 / 60.0;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack")
}

/// The weapon held still, blade up (tests/keychain_routing.rs).
fn held() -> Trs {
    let q = Quat::from_rotation_y((-90.0f32).to_radians());
    Trs { translation: Vec3::new(0.0, 0.0, 160.0) - q * Vec3::new(60.0, 0.0, 0.0), rotation: q, scale: Vec3::ONE }
}

/// One step of the Animator on `sequence` at `time` (its notify window the step before it).
fn play(animator: &mut Animator, sequence: &str, time: f32) {
    let sequences = [sequence];
    let body = BodySlot { samples: [(sequence, 1.0), (sequence, 0.0)], blend_with: None, notify_sequences: &sequences, time, weight: 1.0, active: true, notify_window: (time - DT, time) };
    animator.step(&Frame { dt: DT, body, component_to_world: Trs::IDENTITY }, &NoIkWorld);
}

#[test]
fn keychain_dual_sora_clip_winds_naming_key_reach_the_keychain() {
    if !root().join("manifest.json").is_file() || !root().join("weapon/data/bonamik.json").is_file() {
        eprintln!("SKIPPED: no pack at {}", root().display());
        return;
    }
    let pack = Pack::load(&root()).expect("the pack loads");
    let mut animator = Animator::new(pack, Settings { leg_ik: false, kine_driver: false, face: false, ..Settings::default() }).expect("the animator");
    play(&mut animator, "0852S", 1.0);
    let states: Vec<(u64, String, Vec<String>)> = animator.child_bonamik_states().iter().map(|(k, c, p)| (*k, c.to_string(), p.get("m_Groups").items().iter().filter_map(|g| g.as_str().map(str::to_string)).collect())).collect();
    assert!(states.iter().any(|s| s.1 == "TresAnimNotifyState_Bonamik_CharaWind" && s.2.iter().any(|g| g == "KEY")), "0852S's wind (naming KEY) is listed: {states:?}");

    let (mut still, mut blown) = (Keychain::load(&root().join("weapon")).expect("the keychain"), Keychain::load(&root().join("weapon")).expect("the keychain"));
    let world = held();
    for _ in 0..180 {
        still.step(DT, &world, None);
        blown.step(DT, &world, None);
    }
    for _ in 0..60 {
        play(&mut animator, "0852S", 1.0);
        blown.route_states(&animator.child_bonamik_states());
        blown.step(DT, &world, None);
        still.step(DT, &world, None);
    }
    assert!(blown.routed().0, "the wind is routed");
    let tip = |c: &Keychain| world.transform_point(c.pose().last().map_or(Vec3::ZERO, |t| t.translation));
    let moved = (tip(&blown) - tip(&still)).length();
    eprintln!("0852S's KEY wind moves the chain's end {moved:.1} cm");
    // (these clips' KEY winds are light: base 45, model space (0, -0.5, -0.7), mostly along gravity: about 0.1 cm here)
    assert!(moved > 0.01, "the wind naming KEY reaches and moves the Kingdom Key's chain ({moved:.2} cm)");
    // the clip over: its states end, the wind with them
    blown.route_states(&[]);
    assert!(!blown.routed().0, "no wind once the clip's states end");
}
