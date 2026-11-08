//! (AUDIT-A THROW) The THROWN parts' own sounds through the recorder (`kh_host::audit_a`: the real core, the real effect
//! side and audio side): KH3 plays a weapon's own-clip AttachSound / PlayWeaponSwingSE on the WEAPON actor, which flies
//! (kh-core placement::ThrownWeapon). Counter Shield's raid: the shield's hum se02003_061 (`pawn:...B414.0.thrown@B413`,
//! SQEXSEADAnimNotifyState_AttachSound on the weapon); Storm Flag's flag throw: the flag's SWING10 (`kb:w_so093:..:vfx_top`,
//! TresAnimNotifyState_PlayWeaponSwingSE m_bFollow). Each live sound must sit at the effect side's point of its anchor on
//! the FLYING part, every step it flies (and that point must be on the part, metres from Sora). Writes
//! build/sora_tmp/weapons/AUDIT-A/logs/audit_a_throw_*.report.txt. Skipped without the pack.

use glam::Vec3;
use kh_core::{PadButton, StateId};
use kh_host::audit_a::{self, Session};
use kh_host::vfx::Scene;
use std::fmt::Write as _;

/// One step of a thrown part's sound: (step, Sora's clip, the part's place, the anchor's point, the sound's position).
type Line = (u64, &'static str, Vec3, Vec3, Option<Vec3>, Vec3);

/// The hand unit's weapon SE sets, as kh-er kbdraw `select_sounds` (tests/audit_b_ship.rs does the same).
fn select_sounds(s: &mut Session, base: &str) {
    let model = s.rig.sora.status().attack_model;
    let model = if model.is_empty() { base } else { model };
    if s.audio.engine.routes.weapon.as_deref() != Some(model) && !s.audio.engine.routes.select_weapon(model) {
        s.audio.engine.routes.weapon = Some(model.to_string());
    }
}

/// Steps until the part is recalled; per step the live sounds of `cause` against the effect side's point of `anchor`.
fn follow(s: &mut Session, model: &str, anchor: &str, cause: &str, limit: usize, base: &str) -> (Vec<Line>, String) {
    let mut lines = Vec::new();
    let mut text = String::new();
    let mut flew = false;
    for _ in 0..limit {
        select_sounds(s, base);
        s.step(&[]);
        let out = s.rig.out.clone();
        let Some(t) = s.rig.sora.combat().thrown.iter().find(|t| t.model == model).copied() else {
            if flew {
                let _ = writeln!(text, "{} {}@{:.3}: recalled", out.step, out.anim.name.name(), out.anim.time);
                break;
            }
            continue;
        };
        flew = true;
        // the effect side's point of the anchor this step (a fresh tap of this step's pose, as kh-er's effect step has it)
        let mut tap = kh_host::audio::PoseTap::default();
        tap.record(out.step, &out, s.rig.animator.skeleton(), &s.rig.pose, &kh_host::rig::component_to_world(&out));
        let target = |_: kh_core::TargetId| None;
        let scene = Scene { out: &out, sora: &s.rig.sora, tap: &tap, target: &target, ground: Some(0.0), eye: out.position };
        let want = s.fx.socket_point(&scene, anchor);
        let live: Vec<Option<Vec3>> = s.audio.engine.active().iter().filter(|a| a.cause.split('.').next() == Some(cause) && !a.stopping).map(|a| a.position).collect();
        for p in &live {
            lines.push((out.step, out.anim.name.name(), t.position, want, *p, out.position));
        }
        let _ = writeln!(text, "{} {}@{:.3}: {model} at {:?} ({}@{:.3}); anchor {anchor} at {want:?} ({:.1} cm from the part, {:.0} cm from Sora); live `{cause}`: {live:?}", out.step, out.anim.name.name(), out.anim.time, t.position, t.clip().0, t.clip_time(), want.distance(t.position), want.distance(out.position));
    }
    (lines, text)
}

/// `reach`: how far the anchor may be from the part's origin (the shield's `wep` 20 cm; the flag's vfx_top at the end of its
/// 2.4 m pole).
fn judge(lines: &[Line], what: &str, reach: f32) {
    assert!(lines.len() > 10, "{what}: live while the part flies ({} steps)", lines.len());
    for (step, clip, part, want, got, _) in lines {
        let got = got.unwrap_or_else(|| panic!("{what} step {step} {clip}: a 3D sound with a position"));
        assert!(got.distance(*want) < 1.0, "{what} step {step} {clip}: the sound at {got:?}, its anchor on the flying part at {want:?}");
        assert!(want.distance(*part) < reach, "{what} step {step}: the anchor is on the part ({} cm)", want.distance(*part));
    }
    assert!(lines.iter().any(|l| l.3.distance(l.5) > 300.0), "{what}: it followed the part metres from Sora");
}

const HERO: &[&str] = &["w_so030", "w_so036", "w_so035", "w_so032", "w_so031", "w_so033", "w_so037", "w_so034"];

#[test]
fn the_thrown_shields_hum_follows_it() {
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_01", HERO, "w_so030", Some(Vec3::new(500.0, 0.0, 0.0)), "audit_a_throw_shield") else { return };
    s.idle(5);
    s.rig.sora.debug_add_style(100.0);
    s.idle(15);
    s.step(&[PadButton::SITUATION]);
    assert!(s.until(60, |s| s.rig.out.state == StateId::Stylechange), "StyleChange11");
    assert!(s.until(600, |s| s.rig.out.state != StateId::Stylechange && s.rig.sora.combat().form.cmd == 0x89), "Counter Shield");
    s.idle(20);
    // one press: the far target (500 cm) gives the raid (M_B413 -> M_B414 -> M_B415)
    s.step(&[PadButton::ATTACK]);
    let (lines, text) = follow(&mut s, "w_so032", "pawn:pnotify.SH.w_so032.B414.0.thrown@B413", "seam se02003_061", 240, "w_so030");
    s.idle(10);
    s.finish();
    let report = format!("# (AUDIT-A THROW) Counter Shield's raid: the hum se02003_061 on the flying shield, per step\n{text}# {} live steps\n{}", lines.len(), audit_a::report(&s.rows));
    std::fs::write(audit_a::path_in_logs("audit_a_throw_shield.report.txt"), report).ok();
    judge(&lines, "se02003_061", 60.0);
}

#[test]
fn the_thrown_flags_swing_follows_it() {
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_09", &["w_so090", "w_so091", "w_so092", "w_so093", "w_so094"], "w_so090", Some(Vec3::new(200.0, 0.0, 0.0)), "audit_a_throw_flag") else { return };
    s.idle(5);
    for cmd in [0x8C, 0x8D] {
        s.rig.sora.debug_add_style(100.0);
        s.idle(15);
        s.step(&[PadButton::SITUATION]);
        assert!(s.until(60, |s| s.rig.out.state == StateId::Stylechange), "StyleChange to {cmd:#x}");
        assert!(s.until(600, |s| s.rig.out.state != StateId::Stylechange && s.rig.sora.combat().form.cmd == cmd), "form {cmd:#x}");
        s.idle(30);
    }
    // the dummy 8 m ahead (> 6 m: the flag throw 0xA4)
    let (p, yaw) = (s.rig.out.position, s.rig.out.yaw);
    let (sin, cos) = yaw.to_radians().sin_cos();
    if let Some(d) = s.rig.dummy.as_mut() {
        let at = Vec3::new(p.x + cos * 800.0, p.y + sin * 800.0, 0.0);
        (d.info.position, d.info.center) = (at, at + Vec3::Z * 90.0);
    }
    s.idle(5);
    s.step(&[PadButton::ATTACK]);
    let (lines, text) = follow(&mut s, "w_so093", "kb:w_so093:R_wep6_skt:vfx_top", "seam SWING10", 240, "w_so090");
    s.idle(10);
    s.finish();
    let report = format!("# (AUDIT-A THROW) Storm Flag's flag throw: the flag's SWING10 on the flying flag, per step\n{text}# {} live steps\n{}", lines.len(), audit_a::report(&s.rows));
    std::fs::write(audit_a::path_in_logs("audit_a_throw_flag.report.txt"), report).ok();
    assert_eq!(s.rig.sora.combat().history.iter().rev().find(|k| (0xA3..=0xA9).contains(*k)), Some(&0xA4), "far: the flag throw");
    judge(&lines, "SWING10", 300.0);
}
