//! (THE USER 2026-11-02, a screenshot of Goofy mid-lunge: "this effect is not aligned properly") the shield bash's
//! golden hexagon VN_slg_bsh0 sat low and behind him, at his feet. KH3's AttachEffect row puts it on vfx_root at
//! LocationOffset (100, 0, 80); the seam dropped the offset. This test poses SGuardShieldBash (the combo's Shove 3 and
//! Counter Bash) at the row's window on his real skeleton with his shield mounted, and places the effect by the hosts'
//! rule (kh_host::vfx::PartyMemberFx::xform: the socket's frame, the row's offset in it): at the shield, ahead of him.
//! Skipped without the party pack.

use glam::Vec3;
use kh_core::host::party::SeamNotify;
use kh_core::anim::NotifyPhase;
use kh_party::anim::FiredNotify;
use kh_party::ids::Chara;
use kh_partyhost::poser::{PosedMember, GOOFY};

/// The shield bash's hexagon row as the seam hands it: (socket, offset).
fn bash_row() -> (&'static str, Vec3) {
    let c = kh_party::tables::clip(Chara::Goofy, "SGuardShieldBash");
    let fired: Vec<FiredNotify> = c.notifies.iter().enumerate().filter(|(_, n)| n.class.contains("AttachEffect")).map(|(i, n)| FiredNotify { index: i as u16, phase: NotifyPhase::Begin, notify: n }).collect();
    let mut out = Vec::new();
    kh_party::notify_seam::events_of(Chara::Goofy, c.sequence, c.notifies, &fired, &mut out);
    out.iter()
        .find_map(|e| match e.data {
            SeamNotify::Effect { effect, socket, offset, .. } if effect.ends_with("VN_slg_bsh0") => Some((socket, offset)),
            _ => None,
        })
        .expect("SGuardShieldBash spawns VN_slg_bsh0")
}

#[test]
fn the_shield_bash_hexagon_sits_at_his_shield_ahead_of_him() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/sora_pack/party");
    let dir = root.join("n_ex002");
    let weapon = root.join("weapons/w_gu010/manifest.json");
    if !dir.join("manifest.json").is_file() || !weapon.is_file() {
        println!("no build/sora_pack/party: skipped");
        return;
    }
    let read = |p: &std::path::Path| -> serde_json::Value { serde_json::from_str(std::fs::read_to_string(p).unwrap().trim_start_matches('\u{feff}')).unwrap() };
    let manifest = read(&dir.join("manifest.json"));
    let index = read(&root.join("party.json"));
    let entry = index["members"].as_array().into_iter().flatten().find(|m| m["member"].as_u64() == Some(GOOFY as u64)).cloned().unwrap_or(serde_json::Value::Null);
    let animator = kh_anim::Animator::new(kh_anim::Pack::load(&dir).unwrap(), kh_anim::Settings { leg_ik: false, ..kh_anim::Settings::default() }).unwrap();
    let mut m = PosedMember::new(GOOFY, animator, &manifest, &entry).unwrap();
    let def = m.weapon_def("w_gu010").unwrap();
    m.mount_weapon(&def, &read(&weapon)).unwrap();
    m.debug = Some(m.debug_pose("SGuardShieldBash", 0.0).unwrap());
    // into the row's window (0.449 .. 0.517 s of 1630) in 1/600 s steps
    let time = 0.48;
    let steps = (time * 600.0f32).round() as u64;
    let tap = m.step_debug(steps, time / steps as f32, &kh_anim::NoIkWorld).pop().unwrap();
    let (socket, offset) = bash_row();
    assert_eq!((socket, offset), ("vfx_root", Vec3::new(100.0, 0.0, 80.0)));
    let fx = kh_host::vfx::PartyMemberFx { position: tap.position, yaw: tap.yaw, bones: tap.bones.clone(), anim: String::new() };
    let at = fx.xform(Some(socket), offset).0;
    let before = fx.xform(Some(socket), Vec3::ZERO).0;
    let shield = tap.bones["wep:vfx_call"].0;
    let forward = Vec3::new(tap.yaw.to_radians().cos(), tap.yaw.to_radians().sin(), 0.0);
    let ahead = |p: Vec3| (p - tap.position).dot(forward);
    let up = |p: Vec3| p.z - tap.position.z;
    println!("hexagon at {at:?} ({:.0} cm ahead, {:.0} up), {:.0} cm from the shield {shield:?} ({:.0} ahead, {:.0} up); without the offset {:.0} cm from it", ahead(at), up(at), at.distance(shield), ahead(shield), up(shield), before.distance(shield));
    // the socket's frame is his: the offset is a metre along his facing and 80 cm up
    assert!((ahead(at) - 100.0).abs() < 5.0 && (up(at) - 80.0).abs() < 5.0, "{at:?}");
    // on the shield he holds out (it is ahead of him and up off the floor), much nearer than his root was
    assert!(ahead(shield) > 40.0 && up(shield) > 30.0, "the lunge holds the shield out: {shield:?}");
    assert!(at.distance(shield) < 60.0, "{:.0} cm from the shield", at.distance(shield));
    assert!(before.distance(shield) > at.distance(shield) + 50.0, "the old place (his root) was the fault");
}
