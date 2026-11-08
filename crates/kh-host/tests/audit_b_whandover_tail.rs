//! (AUDIT-B W-HANDOVER) NANO ARMS' RE-PARENT TAILS through the real core and the effect side (kh-host vfx.rs `kb:` anchors with
//! kh-core `placement::part_parent` / `clip_parent`): KH3's TresAnimNotifyState_ChangeAttachParent of w_so172's own clips
//! (begin 0x1405633F0 -> the weapon's 0x1417873F0: AttachToComponent(m_EquipAttachParent, kh_root); end 0x140565500 -> the
//! weapon's 0x141785E90: AttachToComponent(m_EquipAttachParent, the equip socket R_wep_skt)). 8415's window closes at 1.1667
//! of its 1.7333 s: from there the weapon (and every effect attached to it) is back on R_wep_skt, posed by its own clip.
//! Per step of M_8415: Sora's clip time, the reader's answer, the weapon's R_wep as the effect side places it, the same bone
//! on his kh_root (the window's parent) and his R_wep_skt. Log: build/sora_tmp/weapons/AUDIT-B/W-HANDOVER/traces/.

use std::fmt::Write as _;

use glam::Vec3;
use kh_core::{PadButton, StateId};
use kh_host::audit_a::{self, Session};
use kh_host::vfx::Scene;

#[test]
fn nano_arms_8415_tail_is_back_on_its_socket() {
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_08", &["w_so170", "w_so172", "w_so173", "w_so174"], "w_so170", Some(Vec3::new(200.0, 0.0, 0.0)), "audit_b_whandover_tail") else { return };
    s.idle(5);
    s.rig.sora.debug_add_style(100.0);
    s.idle(15);
    s.step(&[PadButton::SITUATION]);
    assert!(s.until(600, |s| s.rig.out.state == StateId::Stylechange), "STYLECHANGE");
    assert!(s.until(600, |s| s.rig.out.state != StateId::Stylechange), "the transformation ends");
    assert_eq!(s.rig.sora.combat().form.cmd, 0x86, "Nano Arms");
    s.idle(30);
    let mut log = String::from("# step, Sora's clip @ time, clip_parent, |R_wep (effect side) - R_wep on kh_root| cm, |R_wep - his R_wep_skt| cm\n");
    let (mut inside, mut tail) = (0, 0);
    let mut seen_8415 = false;
    for k in 0..1200 {
        let st = if k % 2 == 0 && !seen_8415 { s.step(&[PadButton::ATTACK]) } else { s.step(&[]) };
        let out = s.rig.out.clone();
        let (seq, time) = (out.anim_ticked.body_samples()[0].0, out.anim_ticked.time);
        if seq != "8415" {
            if seen_8415 {
                break;
            }
            if k > 60 && matches!(st, StateId::Idle | StateId::Run) && k > 900 {
                break;
            }
            continue;
        }
        seen_8415 = true;
        // the effect side's points on this step's pose, as audit_a / kh-er fx.rs resolve the weapon anchors
        let to_world = kh_host::rig::component_to_world(&out);
        let mut tap = kh_host::audio::PoseTap::default();
        tap.record(out.step, &out, s.rig.animator.skeleton(), &s.rig.pose, &to_world);
        let target = |_: kh_core::TargetId| None;
        let scene = Scene { out: &out, sora: &s.rig.sora, tap: &tap, target: &target, ground: Some(0.0), eye: out.position };
        let weapon = s.fx.socket_point(&scene, "kb:w_so172:R_wep_skt:R_wep");
        let on_root = s.fx.socket_point(&scene, "kb:w_so172:root:R_wep");
        let hand = s.fx.socket_point(&scene, "R_wep_skt");
        let parent = kh_core::combat::forms::placement::clip_parent("w_so172", seq, time);
        let (d_root, d_hand) = (weapon.distance(on_root), weapon.distance(hand));
        let _ = writeln!(log, "{}, {seq} @ {time:.3}, {parent:?}, {d_root:.1}, {d_hand:.1}", out.step);
        if time < 1.1666 {
            assert_eq!(parent, Some("kh_root"), "@{time}");
            assert!(d_root < 1.0, "inside the window the weapon hangs on his kh_root (his mesh origin): {d_root} cm @{time}");
            inside += 1;
        } else if time > 1.1668 {
            assert_eq!(parent, None, "@{time}");
            assert!(d_root > 30.0, "the tail is NOT on his kh_root any more: {d_root} cm @{time}");
            assert!(d_hand > 100.0, "the clip poses it from R_wep_skt (its tail's R_wep is 1.7 .. 2.4 m from the bone's origin): {d_hand} cm @{time}");
            tail += 1;
        }
    }
    let dir = audit_a::project().join("build/sora_tmp/weapons/AUDIT-B/W-HANDOVER/traces");
    std::fs::create_dir_all(&dir).ok();
    std::fs::write(dir.join("nano_8415_tail.log"), &log).ok();
    eprintln!("{log}");
    assert!(seen_8415, "the combo reached the Rocket Fist (8415)");
    assert!(inside > 30 && tail > 20, "steps inside the window {inside}, in the tail {tail}");
}
