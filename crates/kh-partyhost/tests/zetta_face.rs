//! (ZETTA) The member's face-clip slot drawn by the poser (kh_party::face -> AnimView::face -> PosedMember::set_pose_view):
//! a play puts the face clip on both face slots over the body, its end puts the face it had back. Needs a party pack with
//! Zettaflare's face clips: `KH_ZETTA_PACK` (a pack root), else build/sora_tmp/party/ZETTA/testpack2, else
//! build/play/donpack; skipped without one.

use kh_party::face::{FaceClip, FaceView};
use kh_partyhost::poser::{self, PosedMember};

fn project(p: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(p)
}

fn pack_root() -> Option<std::path::PathBuf> {
    let mut list: Vec<std::path::PathBuf> = std::env::var("KH_ZETTA_PACK").ok().map(std::path::PathBuf::from).into_iter().collect();
    list.push(project("build/sora_tmp/party/ZETTA/testpack2"));
    list.push(project("build/play/donpack"));
    list.into_iter().map(|r| r.join("party")).find(|p| p.join("n_ex001/face/n_ex001_70291w_face.glb").is_file())
}

#[test]
fn the_face_clip_slot_plays_over_the_body_and_gives_the_face_back() {
    let Some(root) = pack_root() else {
        println!("no party pack with the Zettaflare face clips: skipped");
        return;
    };
    let dir = root.join("n_ex001");
    let read = |p: &std::path::Path| -> serde_json::Value { serde_json::from_str(std::fs::read_to_string(p).unwrap().trim_start_matches('\u{feff}')).unwrap() };
    let manifest = read(&dir.join("manifest.json"));
    let index = read(&root.join("party.json"));
    let entry = index["members"].as_array().into_iter().flatten().find(|m| m["member"].as_u64() == Some(poser::DONALD as u64)).cloned().unwrap_or(serde_json::Value::Null);
    let animator = kh_anim::Animator::new(kh_anim::Pack::load(&dir).unwrap(), kh_anim::Settings { leg_ik: false, ..kh_anim::Settings::default() }).unwrap();
    let mut m = PosedMember::new(poser::DONALD, animator, &manifest, &entry).unwrap();
    let mut view = kh_party::AnimView { clip: "n_ex001_70291w", sequence: "n_ex001_70291w", time: 0.0, weight: 1.0, ..Default::default() };
    let dt = 1.0 / 60.0;
    let step = |m: &mut PosedMember, v: &kh_party::AnimView| {
        m.set_pose_view(poser::DONALD as u8, glam::Vec3::ZERO, 0.0, dt, v, true, true, &kh_anim::NoIkWorld);
    };
    for _ in 0..5 {
        step(&mut m, &view);
        view.time += dt;
    }
    let before: Vec<Option<String>> = m.animator.face_playing().iter().map(|f| f.map(|(c, _)| c.to_string())).collect();
    let mouth_bones = m.animator.face_subtrees()[1].to_vec();
    assert!(!mouth_bones.is_empty(), "the Mouth slot's bones");
    let mouth = |m: &PosedMember| mouth_bones.iter().map(|&b| m.current.local[b].rotation).collect::<Vec<_>>();
    let rest = mouth(&m);
    for ja in [false, true] {
        let name = kh_party::donald::zettaflare::face_clip(ja);
        view.face = FaceView { clip: Some(FaceClip { sequence: name, time: 0.0, blend_in: 0.1, length: 3.1, looping: false, serial: 1 + ja as u32 }), blend_out: 0.15, ..Default::default() /* (FACE) */ };
        for _ in 0..60 {
            step(&mut m, &view);
            view.time += dt;
        }
        assert!(m.animator.face_playing().iter().all(|f| f.is_some_and(|(c, w)| c == name && w > 0.99)), "{name} on both face slots: {:?}", m.animator.face_playing());
        let moved = mouth(&m).iter().zip(&rest).map(|(a, b)| a.angle_between(*b)).fold(0.0f32, f32::max);
        assert!(moved > 0.01, "{name}: the beak moves ({moved} rad)");
    }
    // the slot empties: the face it had comes back
    view.face = FaceView { clip: None, blend_out: 0.15, ..Default::default() /* (FACE) */ };
    for _ in 0..30 {
        step(&mut m, &view);
        view.time += dt;
    }
    let after: Vec<Option<String>> = m.animator.face_playing().iter().map(|f| f.map(|(c, _)| c.to_string())).collect();
    assert!(after.iter().all(|a| !a.as_deref().is_some_and(|c| c.starts_with("n_ex001_70291w_face"))), "the lip track is gone: {after:?}");
    assert!(before.iter().zip(&after).all(|(b, a)| b.is_none() || b == a), "the earlier plays are back: {before:?} -> {after:?}");
}
