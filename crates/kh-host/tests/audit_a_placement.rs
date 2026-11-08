//! (AUDIT-A PLACEMENT) The effect side of the parts off their sockets, through the recorder (`kh_host::audit_a`: the real
//! core, the real EffectHost with the keyblade root): Counter Shield's raid THROWS the shield (kh-core
//! placement::ThrownWeapon) and its own M_B414 effects (`pawn:...thrown@B413` rows: vw_so032_atk2 / hbn1 on its `wep`) ride
//! on it where it flies, not at the point of Sora's frame where it left his hand (kh-host vfx.rs `thrown_anchor`); the
//! recall puts it back. Writes build/sora_tmp/weapons/AUDIT-A/logs/audit_a_placement_shield.report.txt. Skipped without
//! the pack.

use glam::Vec3;
use kh_core::{PadButton, StateId};
use kh_host::audit_a::Session;
use kh_host::vfx::Scene;

const HERO: &[&str] = &["w_so030", "w_so036", "w_so035", "w_so032", "w_so031", "w_so033", "w_so037", "w_so034"];

#[test]
fn the_thrown_shields_effects_ride_on_it() {
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_01", HERO, "w_so030", Some(Vec3::new(500.0, 0.0, 0.0)), "audit_a_placement_shield") else { return };
    s.idle(5);
    s.rig.sora.debug_add_style(100.0);
    s.idle(15);
    s.step(&[PadButton::SITUATION]);
    assert!(s.until(60, |s| s.rig.out.state == StateId::Stylechange), "StyleChange11");
    assert!(s.until(600, |s| s.rig.out.state != StateId::Stylechange && s.rig.sora.combat().form.cmd == 0x89), "Counter Shield");
    s.idle(20);
    // one press: the far target (500 cm) gives the raid (M_B413 -> M_B414 -> M_B415)
    s.step(&[PadButton::ATTACK]);
    let mut text = String::new();
    let (mut flying, mut on_it, mut back) = (0, 0, false);
    for _ in 0..200 {
        s.step(&[]);
        let out = &s.rig.out;
        let thrown = s.rig.sora.combat().thrown.first().copied();
        let target = |_: kh_core::TargetId| None::<Vec3>;
        let scene = Scene { out, sora: &s.rig.sora, tap: &s.audio.tap, target: &target, ground: Some(0.0), eye: out.position + Vec3::new(-500.0, 0.0, 200.0) };
        let anchor = "pawn:pnotify.SH.w_so032.B414.1.thrown@B413";
        let at = s.fx.socket_point(&scene, anchor);
        let clip = out.anim.name.name();
        match thrown {
            Some(t) => {
                flying += 1;
                let d = at.distance(t.position);
                // the notify's socket `wep` (20 cm offset) is within the shield's reach of its root
                if d < 60.0 {
                    on_it += 1;
                }
                text.push_str(&format!("{} {clip}@{:.3}: shield at ({:.0}, {:.0}, {:.0}), its M_B414 effect anchor at ({:.0}, {:.0}, {:.0}): {d:.1} cm from it; Sora at ({:.0}, {:.0}, {:.0})\n", s.rig.out.step, out.anim.time, t.position.x, t.position.y, t.position.z, at.x, at.y, at.z, out.position.x, out.position.y, out.position.z));
            }
            None if flying > 0 => {
                back = true;
                text.push_str(&format!("{} {clip}@{:.3}: recalled (back on R_Wep3_skt)\n", s.rig.out.step, out.anim.time));
                break;
            }
            None => {}
        }
    }
    let path = kh_host::audit_a::path_in_logs("audit_a_placement_shield.report.txt");
    std::fs::write(&path, format!("# (AUDIT-A PLACEMENT) Counter Shield's raid: the thrown shield and its own M_B414 effect anchor, per step\n{text}# {on_it} of {flying} flying steps with the effect on the shield; recalled: {back}\n")).ok();
    assert!(flying > 60, "the shield flies through M_B413 / M_B414 ({flying} steps)");
    assert_eq!(on_it, flying, "its M_B414 effects ride on it every step:\n{text}");
    assert!(back, "recalled at M_B415");
}
