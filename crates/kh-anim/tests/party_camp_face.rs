//! (party eyes) The friends' camp-menu face: the UI actor plays `Idle_CM_Face` / `IdleBreak_CM_Face`
//! (n_ex00x_CM_Face_AnimSet) on both face slots with the camp body clip (FUN_1415440E0 -> vt+0x440 0x14056BA40 ->
//! face by name 0x14055DD00). The camp body clips have no FaceAnim notify, so without that play the face subtrees hold
//! the saved pose's IdleN frame 0 (the user's "eyes closed" in the camp menu).
//!
//! `cargo test --release -p kh-anim --test party_camp_face -- --nocapture`

use std::path::PathBuf;

use kh_anim::{Animator, BodySlot, Frame, NoIkWorld, Pack, Settings, Trs};

fn pack(npc: &str) -> Option<Animator> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/party").join(npc);
    if !dir.join("face/0001CM_face.glb").is_file() {
        println!("no {}: skipped", dir.join("face/0001CM_face.glb").display());
        return None;
    }
    Some(Animator::new(Pack::load(&dir).unwrap(), Settings { leg_ik: false, bonamik: false, ..Settings::default() }).unwrap())
}

/// `steps` of `clip` at 60 Hz; the component pose of the last step.
fn run(a: &mut Animator, clip: &str, steps: usize) -> Vec<Trs> {
    let sequences = [clip];
    let mut time = 0.0f32;
    for step in 0..steps {
        let dt = 1.0 / 60.0;
        let from = if step == 0 { -1.0 } else { time };
        time += dt;
        let body = BodySlot { samples: [(clip, 1.0), (clip, 0.0)], blend_with: None, notify_sequences: &sequences, time, weight: 1.0, active: true, notify_window: (from, time) };
        a.step(&Frame { dt, body, component_to_world: Trs::IDENTITY }, &NoIkWorld);
    }
    a.pose().component.clone()
}

/// Upper-to-lower lid distance of both eyes (cm, component space): the eye opening.
fn opening(a: &Animator, pose: &[Trs]) -> f32 {
    let s = a.skeleton();
    let d = |u: &str, l: &str| (pose[s.index_of(u).unwrap()].translation - pose[s.index_of(l).unwrap()].translation).length();
    d("RU_eyelid_e", "RD_eyelid_b") + d("LU_eyelid_e", "LD_eyelid_b")
}

#[test]
fn the_camp_face_clip_opens_the_eyes_as_in_the_field() {
    for npc in ["n_ex001", "n_ex002"] {
        let (Some(mut body_only), Some(mut camp), Some(mut field)) = (pack(npc), pack(npc), pack(npc)) else { return };
        let before = run(&mut body_only, "0001CM", 30);
        assert!(body_only.face_playing().iter().all(Option::is_none), "{npc}: the camp body clip plays no face");
        assert!(camp.play_face_clip("Idle_CM_Face", "0001CM_face", 0.0), "{npc}: 0001CM_face is in the pack");
        let after = run(&mut camp, "0001CM", 30);
        let in_field = run(&mut field, "0001", 30);
        let (o_before, o_after, o_field) = (opening(&body_only, &before), opening(&camp, &after), opening(&field, &in_field));
        println!("{npc}: eye opening body-only {o_before:.3} cm, with Idle_CM_Face {o_after:.3} cm, field IdleN {o_field:.3} cm; face {:?}", camp.face_playing());
        assert_eq!(camp.face_playing().map(|f| f.map(|(c, _)| c.to_string())), [Some("0001CM_face".to_string()), Some("0001CM_face".to_string())]);
        // the face subtrees follow the camp face clip
        let clip = camp.sample("0001CM_face", 30.0 / 60.0);
        let lid = camp.skeleton().index_of("RU_eyelid_e").unwrap();
        let local = &camp.pose().local[lid];
        assert!((local.rotation.dot(clip[lid].rotation).abs() - 1.0).abs() < 1e-4, "{npc}: the lid is the face clip's");
        assert!(o_after > o_before * 1.2, "{npc}: the face clip opens the eyes ({o_before} -> {o_after})");
    }
}

/// The I2 look-at's eye input (NO_0_Eye blend-space grid) keeps the eyes open in the field: a focus far beside him
/// (beyond the head's limits, so the eye rotation is not zero) and one in front.
#[test]
fn the_field_look_at_keeps_the_eyes_open() {
    for npc in ["n_ex001", "n_ex002"] {
        for focus in [glam::Vec3::new(-500.0, 50.0, 100.0), glam::Vec3::new(0.0, 300.0, 300.0), glam::Vec3::new(400.0, -200.0, -50.0)] {
            let Some(mut a) = pack(npc) else { return };
            let sequences = ["0001"];
            let mut time = 0.0f32;
            for step in 0..90 {
                let dt = 1.0 / 60.0;
                let from = if step == 0 { -1.0 } else { time };
                time += dt;
                a.set_look_focus(Some(focus));
                let body = BodySlot { samples: [("0001", 1.0), ("0001", 0.0)], blend_with: None, notify_sequences: &sequences, time, weight: 1.0, active: true, notify_window: (from, time) };
                a.step(&Frame { dt, body, component_to_world: Trs::IDENTITY }, &NoIkWorld);
            }
            let pose = a.pose().component.clone();
            let o = opening(&a, &pose);
            let eye = a.look_at().map(|l| (l.rate, l.eye));
            println!("{npc}: focus {focus:?}: look-at (rate, eye) {eye:?}, eye opening {o:.3} cm");
            assert!(o > 20.0, "{npc}: eyes open with the look-at ({o})");
        }
    }
}
