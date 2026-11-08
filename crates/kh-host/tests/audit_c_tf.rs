//! (AUDIT-C TF) DOUBLE FORM (form.TF, CHANGE_TF 0xDF out of Light Form with Oathkeeper; form.TF2, CHANGE_TF2 0xE0 out of Dark
//! Form with Oblivion; wear form 10, both Keyblades w_so200_Wep + w_so210_Wep, AnimSet23) AND ITS ULTIMATE FINISH (FINISH_TF
//! 0xE1 / FINISH_TF2 0xE2: M_N424 -> M_N425), 1:1 WITH KH3, THROUGH AUDIT-A's RECORDER (`kh_host::audit_a`): the real core, the
//! real effect side (main + keyblade + XS effect roots, the form's `vw_so200` / `vw_so210` / `vw_so220` effects let through as
//! kh-er does) and the real audio side (the audio pack's routes, the hand Keyblade's swing / hit sets). One session per
//! Keyblade:
//! - stage 1 (Light / Dark Form) by the gauge, then stage 2 by the next full gauge inside it (STYLECHANGE with AnimSet23's
//!   StyleChangeEnd N401, wear form 10, the two weapons);
//! - a walk, a jump and its landing (AnimSet23's locomotion rows);
//! - a ground combo near the dummy (0xD4 0xD5 0xD7, then Equilibrium Burst 0xDD / Double Blast 0xDA, Flash Charge 0xDB,
//!   Mirage Thrust 0xDC with Joint Struggle's projectiles), the launchers by the jump button in a chain (Launching Thrust 0xDF
//!   / Groundbreaker 0xE0), the far approach 0xD6 (Reverse Cross), an air combo (0xD8 0xD9 ...), the rise 0x1F (M_N420) at a
//!   target above him and the dive 0xDE (Spinning Strike) at a target below him in the air;
//! - Dual Guard (N279 / ReflecHit N277) hit from behind, then Dual Smasher (kind 0x25, M_N426); the same in the air (N280 /
//!   N278);
//! - the Ultimate Finish on slot B: N424 (the start-point search, Flying, root motion up 300) -> N425 (R_Finish spheres, world
//!   slow off at its first window) -> the 0.5 s hold -> DoChain -> the form's end (wear 0).
//!
//! Every sound / voice / weapon-SE / effect notify of every clip played must be emitted by the seam AND resolved by the host
//! (`coverage`). Reports: build/sora_tmp/weapons/AUDIT-C/TF/logs/{okp,obv}.report.txt and {okp,obv}.events.txt. Skipped without
//! the pack.

use std::fmt::Write as _;
use std::path::PathBuf;

use glam::{Vec2, Vec3};
use kh_core::enums::ETresDamageKind;
use kh_core::states::HitOnPlayer;
use kh_core::{PadButton, StateId, TargetId};
use kh_host::audit_a::{clips, coverage, project, report, Session};
use kh_host::rig::Dummy;

fn logs() -> PathBuf {
    let dir = project().join("build/sora_tmp/weapons/AUDIT-C/TF/logs");
    std::fs::create_dir_all(&dir).ok();
    dir
}

fn press(s: &mut Session, b: PadButton) {
    s.step(&[b]);
    s.step(&[]);
}

/// The full gauge, Y, STYLECHANGE to its end and back to IDLE. Returns the form command.
fn transform(s: &mut Session) -> i32 {
    s.rig.sora.debug_add_style(60.0);
    s.idle(2);
    s.rig.sora.debug_add_style(40.0);
    s.idle(45);
    press(s, PadButton::SITUATION);
    assert!(s.until(400, |s| s.rig.out.state == StateId::Stylechange), "STYLECHANGE starts");
    assert!(s.until(400, |s| s.rig.out.state != StateId::Stylechange), "STYLECHANGE ends");
    assert!(s.until(200, |s| s.rig.out.state == StateId::Idle), "back in IDLE");
    s.idle(5);
    s.rig.sora.combat().form.cmd
}

/// A hit from behind him (the form guard guards every direction: Body reaction 3).
fn hit_from_behind(s: &mut Session) {
    let own = s.rig.sora.output().position;
    let from = own - Vec3::new(150.0, 0.0, 0.0);
    let d = (own - from).normalize_or_zero();
    s.rig.sora.take_hit(HitOnPlayer { direction: d, causer_yaw: 180.0, ..HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), from) });
}

/// A point `ahead` cm in front of him at height z.
fn in_front(s: &Session, ahead: f32, z: f32) -> Vec3 {
    let (sin, cos) = s.rig.out.yaw.to_radians().sin_cos();
    let p = s.rig.out.position + Vec3::new(cos, sin, 0.0) * ahead;
    Vec3::new(p.x, p.y, z)
}

fn events_text(s: &Session) -> String {
    let mut t = String::new();
    for r in &s.rows {
        for e in &r.events {
            let keep = ["StateChange", "FormBegin", "FormEnd", "WearForm", "Hit {", "TimeDilation", "AttackCamera", "CameraLock", "AnimBpEvent", "ForceFeedback", "Situation", "VoiceGroupNo", "PlayerHit", "Invincib", "Projectile", "Shot"];
            if keep.iter().any(|k| e.contains(k)) {
                let short: String = e.chars().take(420).collect();
                let _ = writeln!(t, "step {:5} {:?} {} @{:.3} form {:#x} fin {:#x} wear {} grp {} p ({:.0}, {:.0}, {:.1}) | {short}", r.step, r.state, r.clip, r.time, r.form_cmd, r.finish_cmd, r.wear_form, r.effect_group, r.position.x, r.position.y, r.position.z);
            }
        }
    }
    t
}

/// Attack presses (every other step) until the chain is back in IDLE; `jump_at`: press Jump instead at those presses (the
/// launchers: CanLaunch during a chain after a landed hit).
fn chain(s: &mut Session, presses: usize, jump_at: &[usize]) {
    for k in 0..presses {
        press(s, if jump_at.contains(&k) { PadButton::JUMP } else { PadButton::ATTACK });
    }
    assert!(s.until(400, |s| s.rig.out.state == StateId::Idle), "the chain ends in IDLE: {:?}", s.rig.out.state);
    s.idle(10);
}

/// (test aid) the form time back to its full 30 s before each section, so one session holds every move: the time is not
/// what is audited here (P3 / F-LIGHTDARK traced it: CHANGE_TF Param3 30 s).
fn refill(s: &mut Session) {
    let full = s.rig.sora.combat().form.time_full;
    s.rig.sora.combat_mut().form.time_left = full;
}

/// Every row of vw_so220_warp (kh-core `lightdark::attach_rows`) has a live system no farther from its socket than its own
/// LocationOffset (+ 2 cm: the system's node is the socket moved by the offset in the bone's frame).
fn warp_rows_on_their_sockets(s: &Session, name: &str) {
    let none = |_: TargetId| None;
    let scene = kh_host::vfx::Scene { out: &s.rig.out, sora: &s.rig.sora, tap: &s.audio.tap, target: &none, ground: Some(0.0), eye: s.rig.out.position + Vec3::new(-500.0, 0.0, 200.0) };
    let live = s.fx.world.live();
    for r in kh_core::combat::forms::lightdark::attach_rows("vw_so220_warp", 0) {
        let socket = s.fx.socket_point(&scene, r.socket);
        let best = live.iter().filter(|l| l.name.eq_ignore_ascii_case(r.system)).filter_map(|l| s.fx.world.latest_nodes(l.id)).map(|(x, _)| x.position.distance(socket)).fold(f32::MAX, f32::min);
        assert!(best <= r.offset.length() + 2.0, "{name}: {} (row {}) on {}: {best:.1} cm from its socket, offset {:?}", r.system, r.index, r.socket, r.offset);
    }
}

struct Done {
    kinds: Vec<u8>,
    clips: Vec<&'static str>,
    fails: Vec<String>,
    finish_subs: Vec<u8>,
    max_z: f32,
}

fn session(weapon: &str, models: &[&str], sound: &str, name: &str, stage1: i32, stage2: i32, finish: i32) -> Option<Done> {
    let mut s = Session::new(weapon, models, sound, Some(Vec3::new(300.0, 0.0, 0.0)), &format!("audit_c_{name}"))?;
    s.load_xsi_root(); // (kh-er's roots: fx.xsi is on by default)
    s.idle(5);
    // ---- stage 1, then Double Form inside it
    assert_eq!(transform(&mut s), stage1, "{name}: stage 1");
    s.idle(30);
    assert_eq!(transform(&mut s), stage2, "{name}: Double Form by the next full gauge");
    let f = s.rig.sora.combat().form;
    assert_eq!((f.level, f.wear_form), (2, 10), "{name}: level 2, wear form 10");
    assert_eq!(f.weapon_units().len(), 2, "{name}: both Keyblades {:?}", f.weapon_units());
    // ---- locomotion
    s.stick = Vec2::new(0.0, -1.0);
    s.idle(50);
    s.stick = Vec2::new(0.0, 1.0);
    s.idle(50);
    s.stick = Vec2::ZERO;
    s.idle(10);
    press(&mut s, PadButton::JUMP);
    assert!(s.until(200, |s| s.rig.out.state == StateId::Idle), "{name}: landed");
    s.idle(10);
    let first = s.rig.sora.combat().history.len();
    // ---- the ground chain to its finishers
    refill(&mut s);
    s.rig.dummy = Some(Dummy::at(in_front(&s, 200.0, 0.0)));
    chain(&mut s, 160, &[]);
    // ---- the launchers (CanLaunch 0x14142B3F0: Jump in a chain after a landed hit, each stock once: Launching Thrust
    // 0xDF, then Groundbreaker 0xE0 from it)
    refill(&mut s);
    s.rig.dummy = Some(Dummy::at(in_front(&s, 200.0, 0.0)));
    s.idle(45); // (the 0.6 s command cool-down of the whiffed air chain before)
    press(&mut s, PadButton::ATTACK);
    assert!(s.until(90, |s| s.rig.out.state == StateId::ComboTf && s.rig.sora.combat().hit_flag), "{name}: the chain lands a hit");
    for _ in 0..40 {
        press(&mut s, PadButton::JUMP);
        if s.rig.out.anim.name.name() == "N422" {
            break;
        }
    }
    // Groundbreaker: an air step of the chain that lands a hit, then Jump again (the second stock)
    s.until(30, |s| s.rig.out.anim.name.name() != "N422" || s.rig.out.anim.time > 0.5);
    press(&mut s, PadButton::ATTACK);
    s.until(60, |s| s.rig.out.anim.name.name() != "N422" && s.rig.sora.combat().hit_flag);
    for _ in 0..40 {
        press(&mut s, PadButton::JUMP);
        if s.rig.out.anim.name.name() == "N423" {
            break;
        }
    }
    assert!(s.until(400, |s| s.rig.out.state == StateId::Idle), "{name}: down after the launchers");
    s.idle(10);
    // ---- the far approach 0xD6
    refill(&mut s);
    s.rig.dummy = Some(Dummy::at(in_front(&s, 600.0, 0.0)));
    chain(&mut s, 8, &[]);
    // ---- the rise 0x1F at a target above him
    refill(&mut s);
    s.rig.dummy = Some(Dummy::at(in_front(&s, 150.0, 260.0)));
    chain(&mut s, 30, &[]);
    // ---- an air combo against a target in the air
    refill(&mut s);
    s.rig.dummy = Some(Dummy::at(in_front(&s, 250.0, 220.0)));
    press(&mut s, PadButton::JUMP);
    s.idle(16);
    press(&mut s, PadButton::JUMP);
    s.idle(10);
    chain(&mut s, 240, &[]);
    // ---- the dive 0xDE at a target below him in the air
    refill(&mut s);
    s.rig.dummy = Some(Dummy::at(in_front(&s, 150.0, 0.0)));
    press(&mut s, PadButton::JUMP);
    s.idle(16);
    press(&mut s, PadButton::JUMP);
    s.idle(14);
    chain(&mut s, 20, &[]);
    // ---- Double Blast 0xDA (ComboTF Decide 0x141363CE0: the finisher n = 0 when NOT in the air, field_0x142 = 0; every
    // chain of the form reaches its finishers airborne, 0xD7 being an air-mode kind). The chain carried over a Keyblade
    // switch (TryAttack: IDLE with the carry timer pawn+0x10E4 running: combo = pawn+0xE18 + 1) is the ground path: the test
    // sets what the switch leaves (combo 2, the carry timer) and presses Attack on the ground near a ground target.
    refill(&mut s);
    s.idle(45);
    s.rig.dummy = Some(Dummy::at(in_front(&s, 180.0, 0.0)));
    s.idle(5);
    s.rig.sora.combat_mut().combo = 2;
    s.rig.sora.magic_mut().combo_keep = 1.0;
    for _ in 0..20 {
        press(&mut s, PadButton::ATTACK);
    }
    assert!(s.until(400, |s| s.rig.out.state == StateId::Idle), "{name}: the Double Blast chain ends");
    s.idle(10);
    // ---- Dual Guard from behind, Dual Smasher (ground, then air)
    refill(&mut s);
    s.rig.dummy = Some(Dummy::at(in_front(&s, 180.0, 0.0)));
    s.idle(40);
    press(&mut s, PadButton::GUARD);
    hit_from_behind(&mut s);
    s.idle(3);
    press(&mut s, PadButton::ATTACK);
    s.idle(100);
    press(&mut s, PadButton::JUMP);
    s.idle(12);
    press(&mut s, PadButton::GUARD);
    hit_from_behind(&mut s);
    s.idle(3);
    press(&mut s, PadButton::ATTACK);
    assert!(s.until(300, |s| s.rig.out.state == StateId::Idle), "{name}: down after the air counter");
    s.idle(20);
    // ---- Sonic Step (AnimSet23's AirDodge N054): in the air, Guard with the stick
    refill(&mut s);
    s.rig.sora.force_battle(true);
    press(&mut s, PadButton::JUMP);
    s.idle(10);
    s.stick = Vec2::new(0.0, 1.0);
    press(&mut s, PadButton::GUARD);
    s.stick = Vec2::ZERO;
    // vw_so220_warp's twelve rows, each on ITS socket at ITS offset (the attach data asset's own rows)
    assert!(s.until(30, |s| s.rig.out.anim.name.name() == "N054" && s.rig.out.anim.time > 0.08), "{name}: Sonic Step N054");
    warp_rows_on_their_sockets(&s, name);
    assert!(s.until(300, |s| s.rig.out.state == StateId::Idle), "{name}: down after the air dodge");
    s.idle(20);
    let kinds = s.rig.sora.combat().history[first..].to_vec();
    // ---- the Ultimate Finish
    refill(&mut s);
    s.rig.dummy = Some(Dummy::at(in_front(&s, 250.0, 0.0)));
    s.rig.sora.debug_add_style(100.0);
    s.idle(45);
    if s.rig.sora.combat().situation.slot_b.map(|e| e.kind) != Some(finish) {
        std::fs::write(logs().join(format!("{name}.events.txt")), events_text(&s)).ok();
        panic!("{name}: no finish on slot B: form {:#x} time {:.2} kinds {:x?}", s.rig.sora.combat().form.cmd, s.rig.sora.combat().form.time_left, kinds);
    }
    assert_eq!(s.rig.sora.combat().situation.slot_b.map(|e| e.kind), Some(finish), "{name}: the Ultimate Finish on slot B");
    let mut subs: Vec<u8> = Vec::new();
    let mut max_z = f32::MIN;
    press(&mut s, PadButton::SITUATION);
    let start_z = s.rig.out.position.z;
    let mut n425_z: Option<f32> = None;
    let mut n425_steps = 0;
    for _ in 0..1200 {
        s.step(&[]);
        max_z = max_z.max(s.rig.out.position.z);
        // (FUN_1414466F0 at sub 1: the next pawn tick puts him back at the start point as M_N425 begins)
        if s.rig.out.anim.name.name() == "N425" {
            n425_steps += 1;
            if n425_steps == 2 {
                n425_z = Some(s.rig.out.position.z);
            }
        }
        match s.rig.sora.finish() {
            Some(f) if subs.last() != Some(&f.sub) => subs.push(f.sub),
            Some(_) => {}
            None if s.rig.out.state != StateId::Stylefinish => break,
            None => {}
        }
    }
    s.idle(30);
    assert_eq!((s.rig.sora.combat().form.cmd, s.rig.sora.combat().form.wear_form), (0, 0), "{name}: the form ends with the finish");
    let back = n425_z.expect("M_N425 played");
    assert!((back - start_z).abs() < 30.0, "{name}: M_N425 begins at the start point (z {back:.1}, start {start_z:.1}), not where N424's root motion left him");
    // the projectiles' hits: their own component's entries only (no Kingdom Key spark on Brave Shoot)
    let sparks: Vec<String> = s.rows.iter().filter(|r| matches!(r.clip, "N416" | "N418")).flat_map(|r| r.effects.iter().filter(|e| e.stop.is_none() && matches!(e.name.as_str(), "vs_non_hit0" | "vw_so010_hit0" | "vw_so010_str0")).map(move |e| format!("step {} {} {}", r.step, r.clip, e.name))).collect();
    assert!(sparks.is_empty(), "{name}: default sparks on the form's projectiles: {sparks:?}");
    s.idle(180);
    s.finish();
    let text = report(&s.rows);
    std::fs::write(logs().join(format!("{name}.report.txt")), &text).ok();
    std::fs::write(logs().join(format!("{name}.events.txt")), events_text(&s)).ok();
    let cov = coverage(&s.rows);
    let mut fails: Vec<String> = cov.iter().filter(|c| !c.ok).map(|c| format!("{} @{:.3} {} {} -> {}", c.clip, c.time, c.want.kind(), c.want.seam_name(), c.resolved)).collect();
    // every weapon SE notify of Double Form's clips plays from ITS hand's weapon set: RH_WEAPON Oathkeeper's 02020, LH_WEAPON
    // Oblivion's 02021 (PlayWeaponSwingSE / PlayWeaponHitSE: the notify's unit)
    let (mut lh, mut rh) = (0, 0);
    for c in cov.iter().filter(|c| c.clip.starts_with('N') && matches!(c.want, kh_host::audit_a::Want::WeaponSe { .. })) {
        let Some(seq) = kh_core::anim::SEQUENCES.iter().find(|q| q.name == c.clip) else { continue };
        let unit = seq.notifies.iter().find_map(|n| match n.data {
            kh_core::anim::NotifyData::WeaponSe { alias, unit, .. } if (n.time - c.time).abs() < 1.0e-4 && alias == c.want.seam_name() => Some(unit),
            _ => None,
        });
        let want = if unit == Some("LH_WEAPON") { "02021" } else { "02020" };
        if unit == Some("LH_WEAPON") { lh += 1 } else { rh += 1 }
        if !c.resolved.contains(want) && !c.resolved.contains("silent") {
            fails.push(format!("{} @{:.3} {} {} ({unit:?}) -> {} (wants set {want})", c.clip, c.time, c.want.kind(), c.want.seam_name(), c.resolved));
        }
    }
    assert!(lh > 0 && rh > 0, "{name}: both hands' weapon SEs fired ({lh} LH, {rh} RH)");
    let played = clips(&s.rows).into_iter().map(|c| c.0).collect();
    Some(Done { kinds, clips: played, fails, finish_subs: subs, max_z })
}

fn check(d: Option<Done>, name: &str, must: &[&str]) {
    let Some(d) = d else { return };
    for c in must {
        assert!(d.clips.contains(c), "{name}: {c} not played: {:?}", d.clips);
    }
    // (the kinds: each has its own clip, checked above; the core keeps only the last 16 kinds in `history`)
    assert_eq!(d.finish_subs, [1, 2, 5, 12], "{name}: slot 166's sub-steps");
    assert!(d.max_z > 250.0, "{name}: N424's root motion lifts him (max z {})", d.max_z);
    assert!(d.fails.is_empty(), "{name}: {} notify(s) not emitted / resolved (kinds {:x?}):\n{}", d.fails.len(), d.kinds, d.fails.join("\n"));
}

const TF_CLIPS: &[&str] = &["1401", "N401", "N411", "N412", "N413", "N414", "N415", "N416", "N417", "N418", "N419", "N420", "N421", "N422", "N423", "N426", "N427", "N279", "N277", "N280", "N278", "N054", "N424", "N425"];

#[test]
fn double_form_okp_every_notify_is_played() {
    let d = session("WEP_KEYBLADE_SO_12", &["w_so200"], "w_so200", "okp", 0xDD, 0xDF, 0xE1);
    check(d, "okp", TF_CLIPS);
}

#[test]
fn double_form_obv_every_notify_is_played() {
    let d = session("WEP_KEYBLADE_SO_13", &["w_so210"], "w_so210", "obv", 0xDE, 0xE0, 0xE2);
    check(d, "obv", TF_CLIPS);
}

/// Every notify of Double Form's clips (AnimSet23 + the finish), for the inventory: logs/clip_notifies.txt.
#[test]
fn inventory_of_double_forms_clip_notifies() {
    let mut t = String::new();
    for seq in kh_core::anim::SEQUENCES.iter().filter(|s| s.name.starts_with('N') && s.name.len() == 4) {
        let _ = writeln!(t, "== {} length {:.4}", seq.name, seq.length);
        for n in seq.notifies {
            let data: String = format!("{:?}", n.data).chars().take(400).collect();
            let _ = writeln!(t, "  {:.4}{} {:?} {data}", n.time, if n.state { "+" } else { "" }, n.class);
        }
    }
    std::fs::write(logs().join("clip_notifies.txt"), &t).ok();
    assert!(!t.is_empty());
}
