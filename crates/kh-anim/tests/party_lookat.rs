//! (party I2) The friends' head look-at (TresAnimNode_ExLookAt 0x1405384E0 and its feed, kh-anim src/lookat.rs) on the
//! real packs: without a focus the node is a pass-through; with a focus point beside / above him the head turns to it
//! within the graph's limits at the node's speed; a look-at notify window (m_ReverseTrg) turns it off.
//!
//! `cargo test --release -p kh-anim --test party_lookat -- --nocapture`

use std::path::PathBuf;

use glam::Vec3;
use kh_anim::{Animator, BodySlot, Frame, NoIkWorld, Pack, Settings, Trs};

fn pack(npc: &str) -> Option<Animator> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/party").join(npc);
    if !dir.join("data/lookat.json").is_file() {
        println!("no {} (node tools/party_lookat_pack.js): skipped", dir.join("data/lookat.json").display());
        return None;
    }
    Some(Animator::new(Pack::load(&dir).unwrap(), Settings { leg_ik: false, bonamik: false, ..Settings::default() }).unwrap())
}

/// `steps` of `clip` at 60 Hz with the focus point (None = no focus); returns the head's component transform.
fn run(a: &mut Animator, clip: &str, steps: usize, focus: Option<Vec3>) -> Trs {
    let sequences = [clip];
    let mut time = 0.0f32;
    let head = a.skeleton().index_of("atama").unwrap();
    let mut out = Trs::IDENTITY;
    for step in 0..steps {
        let dt = 1.0 / 60.0;
        let from = if step == 0 { -1.0 } else { time };
        time += dt;
        a.set_look_focus(focus);
        let body = BodySlot { samples: [(clip, 1.0), (clip, 0.0)], blend_with: None, notify_sequences: &sequences, time, weight: 1.0, active: true, notify_window: (from, time) };
        out = a.step(&Frame { dt, body, component_to_world: Trs::IDENTITY }, &NoIkWorld).component[head];
    }
    out
}

/// The head's facing yaw about +Z in degrees, 0 = the mesh's +Y (positive toward -X, the node's sign).
fn facing_yaw(head: &Trs, rest: &Trs) -> f32 {
    // the head's own rotation change against the pass-through pose, applied to +Y
    let f = (head.rotation * rest.rotation.conjugate()) * Vec3::Y;
    (-f.x).atan2(f.y).to_degrees()
}

#[test]
fn without_a_focus_the_look_at_is_a_pass_through() {
    for npc in ["n_ex001", "n_ex002"] {
        let (Some(mut a), Some(mut b)) = (pack(npc), pack(npc)) else { return };
        let with = run(&mut a, "0001", 60, None);
        // a pack without the node for the same clip: the same animator with the focus never set gives rate 0
        let without = run(&mut b, "0001", 60, None);
        assert_eq!(with, without);
        assert_eq!(a.look_at().unwrap().rate, 0.0, "{npc}: no focus -> LookAtRate 0");
    }
}

#[test]
fn the_head_turns_to_a_focus_point_within_the_limits() {
    for npc in ["n_ex001", "n_ex002"] {
        let (Some(mut a), Some(mut rest_a)) = (pack(npc), pack(npc)) else { return };
        a.look_at_mut().unwrap().set_eye_share(0.0); // (FACE) KH3's own node (the eyes' share 0)
        let rest = run(&mut rest_a, "0001", 120, None);
        // 45 degrees to his side (toward -X), level with the head
        let point = Vec3::new(-300.0, 300.0, rest.translation.z);
        let head = run(&mut a, "0001", 120, Some(point));
        let look = a.look_at().unwrap();
        println!("{npc}: rate {:.3} target {:?} current {:?} eye {:?} head yaw {:.1}", look.rate, look.target, look.current, look.eye, facing_yaw(&head, &rest));
        // SetLookAt(1, 0.5) every tick restarts the 0.5 s ramp: the value closes 1/30 of the gap per 60 Hz step
        // (1 - (29/30)^120 = 0.983 after 2 s)
        assert!(look.rate > 0.98, "{npc}: the focus interpolator approaches 1: {}", look.rate);
        assert!(look.target.y > 30.0 && look.target.y <= 60.0 + 1e-3, "{npc}: target yaw toward -X within the 60 deg limit: {:?}", look.target);
        assert!((look.current - look.target).length() < 0.5, "{npc}: turned at the node speed within 2 s: {:?}", look.current);
        let yaw = facing_yaw(&head, &rest);
        assert!(yaw > 20.0, "{npc}: the head faces the point: {yaw}");
        // far behind him to the side beyond the limit: the target clamps at 60 and the eye keeps the rest
        let behind = Vec3::new(-300.0, -100.0, rest.translation.z);
        let _ = run(&mut a, "0001", 120, Some(behind));
        let look = a.look_at().unwrap();
        assert!((look.target.y - 60.0).abs() < 1e-3 && look.eye.y > 10.0, "{npc}: clamped at the limit, eye rest {:?} {:?}", look.target, look.eye);
    }
}

#[test]
fn a_reverse_look_at_window_turns_it_off() {
    // Donald's 0002 (or the first clip with an m_ReverseTrg look-at state from its start) keeps the rate at 0.
    let Some(mut a) = pack("n_ex001") else { return };
    let point = Vec3::new(-300.0, 300.0, 100.0);
    let _ = run(&mut a, "0001", 60, Some(point));
    assert!(a.look_at().unwrap().rate > 0.8);
    // A clip with a TresLookAt (m_ReverseTrg) state from 0 for 8 s: 0004.
    let _ = run(&mut a, "0004", 40, Some(point));
    let rate = a.look_at().unwrap().rate;
    println!("rate in 0004: {rate}");
    assert!(rate < 0.5, "the attack clip's reverse look-at window turned it off: {rate}");
}

#[test]
fn the_eye_look_blend_space_is_centred_without_a_look() {
    // NO_0_Eye (n_ex001_BS_id): at (0, 0) the grid gives ECC00 alone (the clip the face slot played before the port of
    // the eye look), at the X edge the side sample.
    let Some(a) = pack("n_ex001") else { return };
    let look = a.look_at().unwrap();
    let space = look.eye_spaces.iter().find(|e| e.row == "NO_0_Eye").expect("NO_0_Eye is an eye blend space");
    let centre = space.weights(0.0, 0.0);
    assert_eq!(centre.len(), 1, "{centre:?}");
    assert_eq!(centre[0].0, "ECC00");
    let side = space.weights(50.0, 0.0);
    assert_eq!(side[0].0, "ECL00", "{side:?}");
    // no look yet: the rate is 0, so the row's input is (0, 0)
    assert_eq!(look.eye_weights("NO_0_Eye").unwrap()[0].0, "ECC00");
}
