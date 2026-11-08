//! (AUDIT-A SECOND) THE SECOND FORM FAMILY, 1:1 WITH KH3, THROUGH THE RECORDER: Second Form (form.L1, Kingdom Key, 0x94),
//! Second Form S (form.L2, Starlight, 0x95) and Second Form D (form.L5, Dead of Night, 0x98). Per Keyblade one session of
//! three forms, each through the real core, the real effect side (main + keyblade effect roots, the held Keyblade's effects
//! let through as kh-er does) and the real audio side (the audio pack's routes, the held Keyblade's swing / hit sets):
//! - form 1: the offer (the gauge, Y) -> STYLECHANGE (1401 / 1402 / 1403, wear form 5, the charge effect) -> Stun Impact at
//!   once (finish level 1; Dead of Night's from the air: M_1422) -> the end of the form;
//! - form 2: the transformation -> a ground combo and an air combo (Second Form's COMBO_LIMIT moves; their hits fill the
//!   gauge to finish level 2) -> Sonic Rave with every extra (M_1424, M_1425 x6, M_1426) -> end;
//! - form 3: the transformation -> two more full gauges -> finish level 3 with every extra: Last Arcanum (M_1427 .. M_1433 /
//!   M_1434) with the Kingdom Key and Dead of Night, Ancient Light (M_1435, 0xAE) with Starlight -> end.
//!
//! Every sound / voice / weapon-SE / effect notify of every clip played must be emitted by the seam AND resolved by the host
//! (`coverage`: drawn / played). Reports: build/sora_tmp/weapons/AUDIT-A/logs/second_{l1,l2,l5}.report.txt, the step events
//! of interest in second_{..}.events.txt. Skipped without the pack.

use std::fmt::Write as _;

use glam::Vec3;
use kh_core::{PadButton, StateId};
use kh_host::audit_a::{clips, coverage, path_in_logs, write_report, Session};

/// One press: down for a step, up for a step.
fn press(s: &mut Session, b: PadButton) {
    s.step(&[b]);
    s.step(&[]);
}

/// Fills the gauge, waits for the offer and takes it (Y), then waits out STYLECHANGE. Returns the form command.
fn transform(s: &mut Session) -> i32 {
    s.rig.sora.debug_add_style(60.0);
    s.idle(2);
    s.rig.sora.debug_add_style(40.0);
    s.idle(15);
    press(s, PadButton::SITUATION);
    assert!(s.until(400, |s| s.rig.out.state == StateId::Stylechange), "STYLECHANGE starts");
    assert!(s.until(400, |s| s.rig.out.state != StateId::Stylechange), "STYLECHANGE ends");
    // (the fall / landing that follows: CanStyleChange refuses a command while a state is queued)
    assert!(s.until(200, |s| s.rig.out.state == StateId::Idle), "back in IDLE");
    s.idle(5);
    s.rig.sora.combat().form.cmd
}

/// Triangle on every other step while a finish extra is on show, until STYLEFINISH ends.
fn run_finish(s: &mut Session) {
    // (the command cool-down of a whiffed air attack, 0.6 s, refuses a press: press again until STYLEFINISH begins)
    for _ in 0..60 {
        press(s, PadButton::SITUATION);
        if s.rig.out.state == StateId::Stylefinish || s.rig.sora.finish().is_some() {
            break;
        }
    }
    let mut down = false;
    for _ in 0..2000 {
        let extra = s.rig.sora.combat().situation.extra.is_some();
        down = extra && !down;
        let state = if down { s.step(&[PadButton::SITUATION]) } else { s.step(&[]) };
        if state != StateId::Stylefinish && s.rig.sora.finish().is_none() {
            break;
        }
    }
}

/// A ground combo then an air combo (attack presses every other step; a jump first for the air part).
fn combos(s: &mut Session) {
    for _ in 0..120 {
        press(s, PadButton::ATTACK);
    }
    assert!(s.until(300, |s| s.rig.out.state == StateId::Idle), "the ground combo ends");
    s.idle(5);
    press(s, PadButton::JUMP);
    s.idle(10);
    for _ in 0..60 {
        press(s, PadButton::ATTACK);
    }
    assert!(s.until(400, |s| s.rig.out.state == StateId::Idle), "the air combo ends");
    s.idle(5);
}

/// The step events the audit reads besides the notifies (states, form, wear, hits, cameras, time, bp events ...).
fn events_text(s: &Session) -> String {
    let mut t = String::new();
    for r in &s.rows {
        for e in &r.events {
            let keep = ["StateChange", "FormBegin", "FormEnd", "WearForm", "Hit {", "TimeDilation", "AttackCamera", "CameraLock", "AnimBpEvent", "ForceFeedback", "SituationSlot", "SituationRemoved", "VoiceGroupNo", "PlayerHit"];
            if keep.iter().any(|k| e.contains(k)) {
                let short: String = e.chars().take(420).collect();
                let _ = writeln!(t, "step {:5} {:?} {} @{:.3} form {:#x} fin {:#x} wear {} z {:.1} | {short}", r.step, r.state, r.clip, r.time, r.form_cmd, r.finish_cmd, r.wear_form, r.position.z);
            }
        }
    }
    t
}

/// The whole session of one Keyblade. `air_stun`: Stun Impact from the air (M_1422).
fn session(weapon: &str, models: &[&str], sound: &str, name: &str, form: i32, level3: i32, air_stun: bool) {
    let Some(mut s) = Session::new(weapon, models, sound, Some(Vec3::new(300.0, 0.0, 0.0)), name) else { return };
    s.idle(5);
    // ---- form 1: offer -> transformation -> Stun Impact at once (hits would raise the finish level) -> end
    assert_eq!(transform(&mut s), form, "the held Keyblade's own Second Form");
    assert_eq!(s.rig.sora.combat().form.wear_form, 5, "wear form 5");
    assert_eq!(s.rig.sora.combat().form.finish_cmd, 0xAB, "finish level 1 = Stun Impact");
    if air_stun {
        press(&mut s, PadButton::JUMP);
        s.idle(8);
    }
    run_finish(&mut s);
    assert!(s.until(300, |s| s.rig.sora.combat().form.cmd == 0), "the form ends with Stun Impact");
    s.idle(120);
    // ---- form 2: the transformation -> combos (their hits fill the gauge: finish level 2) -> Sonic Rave -> end
    assert_eq!(transform(&mut s), form);
    combos(&mut s);
    if s.rig.sora.combat().form.finish_cmd != 0xAC {
        s.rig.sora.debug_add_style(100.0);
        s.idle(20);
    }
    assert_eq!(s.rig.sora.combat().form.finish_cmd, 0xAC, "finish level 2 = Sonic Rave");
    run_finish(&mut s);
    assert!(s.until(300, |s| s.rig.sora.combat().form.cmd == 0), "the form ends with Sonic Rave");
    s.idle(120);
    // ---- form 3: level 3
    assert_eq!(transform(&mut s), form);
    s.rig.sora.debug_add_style(100.0);
    s.idle(3);
    s.rig.sora.debug_add_style(100.0);
    s.idle(20);
    assert_eq!(s.rig.sora.combat().form.finish_cmd, level3, "finish level 3");
    run_finish(&mut s);
    assert!(s.until(300, |s| s.rig.sora.combat().form.cmd == 0), "the form ends with its level-3 finish");
    s.idle(180);
    s.finish();
    let text = write_report(&s.rows, name);
    std::fs::write(path_in_logs(&format!("{name}.events.txt")), events_text(&s)).ok();
    let played: Vec<&str> = clips(&s.rows).iter().map(|c| c.0).collect();
    for want in ["1401", "1402", "1403"] {
        assert!(played.contains(&want), "{name}: {want} played");
    }
    let finish_clips: &[&str] = match (level3, air_stun) {
        (0xAE, _) => &["1421", "1424", "1425", "1426", "1435"],
        (_, true) => &["1422", "1424", "1425", "1426", "1427", "1428", "1429", "1430", "1431", "1432"],
        _ => &["1421", "1424", "1425", "1426", "1427", "1428", "1429", "1430", "1431", "1432"],
    };
    for want in finish_clips {
        assert!(played.contains(want), "{name}: {want} played: {played:?}");
    }
    let cov = coverage(&s.rows);
    let failed: Vec<String> = cov.iter().filter(|c| !c.ok).map(|c| format!("{} {} @{:.3} {} {} -> {}", c.step, c.clip, c.time, c.want.kind(), c.want.seam_name(), c.resolved)).collect();
    assert!(failed.is_empty(), "{name}: {} of {} notifies FAIL:\n{}\n(report {})", failed.len(), cov.len(), failed.join("\n"), text.lines().count());
}

#[test]
fn second_form_kingdom_key() {
    session("WEP_KEYBLADE_SO_00", &[], "w_so010", "second_l1", 0x94, 0xAD, false);
}

#[test]
fn second_form_s_starlight() {
    session("WEP_KEYBLADE_SO_17", &["w_so150"], "w_so150", "second_l2", 0x95, 0xAE, false);
}

#[test]
fn second_form_d_dead_of_night() {
    session("WEP_KEYBLADE_SO_18", &["w_so121"], "w_so121", "second_l5", 0x98, 0xAD, true);
}
