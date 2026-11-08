//! (AUDIT-A CAMS) The formchange / finisher cameras END TO END: the recorder (`kh_host::audit_a`: the real core with the
//! level-99 loadout and the live pack) with its camera layer on (`Session::cams_on`: kh-camera `form_bp::FormBpCameras`
//! stepped exactly as kh-er cam.rs and the effects harness step it: the step's core events, `command` = pawn+0xE2C, the
//! held keyblade's weapon id, the field camera's reset / takeover). Each session checks that the cameras KH3's blueprints
//! give start, in order, at the BpEvent / command that starts them, and end as decoded:
//! - Second Form (the coordinator released its BpEvents on 2026-11-05, golden re-recorded): form_change's change_limit_form
//!   and its shake at 1401 / 1403, stan_impact01 (1421) / 02 (1422, Dead of Night from the air), sonic_rave_01 (1426),
//!   ripple_drive's FOV (1413), LastAlcanam_End (1433 only: 1434 has none), Starlight's Ancient Light (1435,
//!   UnionDiscode's Limit_UnionDiscode, then back_camera).
//! - The gun forms (kh-camera forms_guns.rs): 2401 2shotguns_start1, CHANGE_BZ bazooka_start, Rising Blast 5413, FINISH_BZ
//!   Bazooka_end01 / 03; Hunny Spout G401 Po_2ShitGuns, H401 Po_bazooka, FINISH_BZ Bazooka_cannon_po02.
//!
//! Logs: build/sora_tmp/weapons/AUDIT-A/CAMS/logs/cams_{second_l1,second_l2,second_l5,guns_ss,guns_hs}.log (every BpEvent of the
//! core and every line of the layer, with the step, state and clip). Skipped without the pack.

use glam::Vec3;
use kh_core::{PadButton, StateId};
use kh_host::audit_a::Session;

fn press(s: &mut Session, b: PadButton) {
    s.step(&[b]);
    s.step(&[]);
}

fn cam_session(weapon: &str, models: &[&str], sound: &str, log: &str) -> Option<Session> {
    let mut s = Session::new(weapon, models, sound, Some(Vec3::new(250.0, 0.0, 0.0)), log)?;
    s.cams_on();
    s.idle(5);
    Some(s)
}

/// Fills the gauge, Y on the offer, waits out STYLECHANGE (audit_a_second.rs's).
fn transform(s: &mut Session) -> i32 {
    s.rig.sora.debug_add_style(60.0);
    s.idle(2);
    s.rig.sora.debug_add_style(40.0);
    s.idle(15);
    press(s, PadButton::SITUATION);
    assert!(s.until(400, |s| s.rig.out.state == StateId::Stylechange), "STYLECHANGE starts");
    assert!(s.until(400, |s| s.rig.out.state != StateId::Stylechange), "STYLECHANGE ends");
    assert!(s.until(200, |s| s.rig.out.state == StateId::Idle), "back in IDLE");
    s.idle(5);
    s.rig.sora.combat().form.cmd
}

/// Y on every other step while a finish extra is up, until STYLEFINISH ends (audit_a_second.rs's).
fn run_finish(s: &mut Session) {
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

fn lines(s: &Session) -> Vec<String> {
    s.cams.as_ref().map(|c| c.log.iter().map(|(step, state, at, l)| format!("step {step:5} {state:?} {at}: {l}")).collect()).unwrap_or_default()
}

fn write(s: &Session, name: &str) -> Vec<String> {
    let l = lines(s);
    let dir = kh_host::audit_a::project().join("build/sora_tmp/weapons/AUDIT-A/CAMS/logs");
    std::fs::create_dir_all(&dir).ok();
    std::fs::write(dir.join(format!("{name}.log")), l.join("\n") + "\n").ok();
    l
}

/// `want` appear in this order (each after the one before), every one in a line of the log.
fn in_order(log: &[String], want: &[&str], name: &str) {
    let mut from = 0;
    for w in want {
        match log[from..].iter().position(|l| l.contains(w)) {
            Some(i) => from += i + 1,
            None => panic!("{name}: `{w}` missing (or out of order) after line {from}:\n{}", log.join("\n")),
        }
    }
}

/// The step of the first line containing `what`.
fn step_of(log: &[String], what: &str) -> Option<u64> {
    log.iter().find(|l| l.contains(what)).and_then(|l| l.split_whitespace().nth(1)).and_then(|n| n.parse().ok())
}

/// The layer's view was the one shown on these steps (not the field camera's).
fn shown(s: &Session, from: u64, to: u64) -> usize {
    s.cams.as_ref().map_or(0, |c| c.views.iter().filter(|(st, v)| (from..to).contains(st) && v.is_some()).count())
}

fn second_form(weapon: &str, models: &[&str], sound: &str, name: &str, air_stun: bool) -> Option<Vec<String>> {
    let mut s = cam_session(weapon, models, sound, name)?;
    transform(&mut s);
    if air_stun {
        press(&mut s, PadButton::JUMP);
        s.idle(8);
    }
    run_finish(&mut s);
    assert!(s.until(300, |s| s.rig.sora.combat().form.cmd == 0), "the form ends with Stun Impact");
    s.idle(120);
    transform(&mut s);
    combos(&mut s);
    if s.rig.sora.combat().form.finish_cmd != 0xAC {
        s.rig.sora.debug_add_style(100.0);
        s.idle(20);
    }
    run_finish(&mut s);
    assert!(s.until(300, |s| s.rig.sora.combat().form.cmd == 0), "the form ends with Sonic Rave");
    s.idle(120);
    transform(&mut s);
    s.rig.sora.debug_add_style(100.0);
    s.idle(3);
    s.rig.sora.debug_add_style(100.0);
    s.idle(20);
    run_finish(&mut s);
    assert!(s.until(300, |s| s.rig.sora.combat().form.cmd == 0), "the form ends with its level-3 finish");
    s.idle(240);
    let log = write(&s, &format!("cams_{name}"));
    // every camera the layer started was drawn on the steps after its start
    for start in log.iter().filter(|l| l.contains(": start ")) {
        let at: u64 = start.split_whitespace().nth(1).and_then(|n| n.parse().ok()).unwrap_or(0);
        assert!(shown(&s, at, at + 10) >= 9, "{name}: drawn after `{start}`");
    }
    Some(log)
}

#[test]
fn second_form_kingdom_key_cameras_fire_in_kh3s_order() {
    let Some(log) = second_form("WEP_KEYBLADE_SO_00", &[], "w_so010", "second_l1", false) else { return };
    in_order(
        &log,
        &[
            // form 1: 1401 -> change_limit_form (0.6 s ease in), 1403 -> the shake, the camera's own end, Stun Impact
            "core BpEvent 1401 param 0 begin",
            "start none change_limit_form",
            "core BpEvent 1403 param 0 begin",
            "shake shake_super_kokizami_1sec x1",
            "auto end none",
            "core BpEvent 1421 param 0 begin",
            "start none stan_impact01",
            // form 2: the transformation again, the combo, Sonic Rave's last dash 1426
            "start none change_limit_form",
            "start none sonic_rave_01",
            // form 3: Last Arcanum
            "start none change_limit_form",
        ],
        "second_l1",
    );
    // the takeover of stan_impact01's / sonic_rave_01's own end (bEndTakeover, 0.3 s)
    assert!(log.iter().filter(|l| l.contains("field camera takes over")).count() >= 2, "stan_impact01 / sonic_rave_01 hand the view over");
    // Last Arcanum's end clip: 1433 films LastAlcanam_End, 1434 (the ground probe's) has no camera in form_change
    let la = if log.iter().any(|l| l.contains("core BpEvent 1433 param 0 begin")) { "start none LastAlcanam_End" } else { "core BpEvent 1434 param 0 begin" };
    in_order(&log, &["start none sonic_rave_01", la], "second_l1");
    if la.contains("1434") {
        let at = step_of(&log, "core BpEvent 1434 param 0 begin").unwrap_or(0);
        assert!(!log.iter().any(|l| l.contains("LastAlcanam_End")), "1434 has no camera (form_change names only 1433): from step {at}");
    }
}

#[test]
fn second_form_s_starlight_ancient_light_camera() {
    let Some(log) = second_form("WEP_KEYBLADE_SO_17", &["w_so150"], "w_so150", "second_l2", false) else { return };
    in_order(
        &log,
        &[
            "start none change_limit_form",
            "start none stan_impact01",
            "start none change_limit_form",
            "start none sonic_rave_01",
            "start none change_limit_form",
            "core BpEvent 1435 param 0 begin",
            "UnionDiscode: start Limit_UnionDiscode_Camera",
            "core BpEvent 1435 param 0 end",
            "field camera reset",
            "UnionDiscode: start back_camera",
            "UnionDiscode: end 1 camera(s), top was Some(\"back_camera\")",
        ],
        "second_l2",
    );
}

#[test]
fn second_form_d_air_stun_impact_camera() {
    let Some(log) = second_form("WEP_KEYBLADE_SO_18", &["w_so121"], "w_so121", "second_l5", true) else { return };
    in_order(&log, &["start none change_limit_form", "core BpEvent 1422 param 0 begin", "start none stan_impact02"], "second_l5");
}

/// Fill the gauge, Y on the offer `cmd`, through STYLECHANGE into the form (audit_a_arrowgun.rs's).
fn gun_transform(s: &mut Session, cmd: i32) {
    s.rig.sora.debug_add_style(100.0);
    s.idle(15);
    for _ in 0..40 {
        s.step(&[PadButton::SITUATION]);
        if s.step(&[]) == StateId::Stylechange {
            break;
        }
    }
    for _ in 0..600 {
        if s.step(&[]) != StateId::Stylechange && s.rig.sora.combat().form.cmd == cmd {
            break;
        }
    }
    assert_eq!(s.rig.sora.combat().form.cmd, cmd, "{cmd:#x} began");
    s.idle(20);
}

fn gun_combo(s: &mut Session) {
    let first = s.rig.sora.combat().history.len();
    let (mut attacking, mut finished) = (false, false);
    for k in 0..900 {
        finished |= s.rig.sora.combat().finish_end;
        let press = k % 2 == 0 && (k < 2 || attacking) && !finished;
        let state = if press { s.step(&[PadButton::ATTACK]) } else { s.step(&[]) };
        attacking = matches!(state, StateId::ComboDw | StateId::ComboBz | StateId::Attack);
        if k > 30 && !attacking && s.rig.sora.combat().history.len() > first {
            break;
        }
    }
    s.idle(30);
}

fn guns(weapon: &str, models: &[&str], sound: &str, (dw, bz): (i32, i32), name: &str) -> Option<Vec<String>> {
    let mut s = cam_session(weapon, models, sound, name)?;
    gun_transform(&mut s, dw);
    gun_combo(&mut s);
    gun_transform(&mut s, bz);
    gun_combo(&mut s);
    s.idle(30);
    assert_eq!(s.rig.sora.combat().situation.slot_b.map(|e| e.kind), Some(0x9D), "FINISH_BZ on offer");
    s.step(&[PadButton::SITUATION]);
    for _ in 0..900 {
        if s.step(&[]) != StateId::Stylefinish && s.rig.sora.combat().form.cmd == 0 {
            break;
        }
    }
    s.idle(180);
    let log = write(&s, &format!("cams_{name}"));
    for start in log.iter().filter(|l| l.contains(": start ")) {
        let at: u64 = start.split_whitespace().nth(1).and_then(|n| n.parse().ok()).unwrap_or(0);
        assert!(shown(&s, at, at + 10) >= 9, "{name}: drawn after `{start}`");
    }
    Some(log)
}

#[test]
fn shooting_star_gun_form_cameras() {
    let Some(log) = guns("WEP_KEYBLADE_SO_02", &["w_so020", "w_so021", "w_so022", "w_so023", "w_so024"], "w_so020", (0x80, 0x83), "guns_ss") else { return };
    in_order(
        &log,
        &[
            // StyleChange2: 2401's window holds 2shotguns_start1 (no autoEnd) until the window's end
            "core BpEvent 2401 param 0 begin",
            "start 2SHOT 2shotguns_start1",
            "core BpEvent 2401 param 0 end",
            "field camera reset",
            "end of 2401",
            // CHANGE_BZ (StyleChange5 has no BpEvent): bazooka_start by the command
            "start start bazooka bazooka_start",
            // Rising Blast
            "core BpEvent 5413 param 0 begin",
            "Bazooka_Rush",
            // FINISH_BZ
            "start finish bazooka Bazooka_end0",
        ],
        "guns_ss",
    );
}

#[test]
fn hunny_spout_gun_form_cameras() {
    let Some(log) = guns("WEP_KEYBLADE_SO_07", &["w_so040", "w_so041", "w_so042", "w_so043", "w_so044"], "w_so040", (0x8E, 0x8F), "guns_hs") else { return };
    in_order(
        &log,
        &[
            "core BpEvent G401 param 0 begin",
            "start Po_2Shot__Start_Cmaera Po_2ShitGuns",
            "core BpEvent H401 param 0 begin",
            "start PoBazooka_Start_Cmaera Po_bazooka",
            "core BpEvent 5413 param 0 begin",
            "Bazooka_Rush",
            "start finish bazooka Bazooka_cannon_po02",
        ],
        "guns_hs",
    );
}

/// FINISH_BZ taken in the middle of COMBO_BZ. KH3's order: StartStyle's TryStyleFinish (request interface slot 192
/// 0x141447840) only QUEUES STYLEFINISH (SetState 0x1405B9FE0 -> 0x141513260: applied at the next MainProc), then the executor
/// sends ReceivePlayerBeginCommandAction (0x1414158F0): the Bazooka blueprint starts 'finish bazooka' first and its COMBO_BZ
/// dtor (78 -> @27744 EndAllSameBPTresCamera) cuts it when the queued state replaces COMBO_BZ. So a FINISH_BZ out of the
/// combo has no finish camera in KH3 either; the port does the same (the core's state change also follows the command).
#[test]
fn finish_bz_from_inside_the_combo() {
    let Some(mut s) = cam_session("WEP_KEYBLADE_SO_02", &["w_so020", "w_so021", "w_so022", "w_so023", "w_so024"], "w_so020", "guns_ss_midcombo") else { return };
    gun_transform(&mut s, 0x80);
    gun_transform(&mut s, 0x83);
    s.idle(30);
    assert_eq!(s.rig.sora.combat().situation.slot_b.map(|e| e.kind), Some(0x9D), "FINISH_BZ on offer");
    let mut in_combo = false;
    for _ in 0..40 {
        let st = s.step(&[PadButton::ATTACK]);
        s.step(&[]);
        if st == StateId::ComboBz || s.rig.out.state == StateId::ComboBz {
            in_combo = true;
            break;
        }
    }
    assert!(in_combo, "in COMBO_BZ");
    s.idle(6);
    for _ in 0..30 {
        s.step(&[PadButton::SITUATION]);
        if s.step(&[]) == StateId::Stylefinish {
            break;
        }
    }
    for _ in 0..900 {
        if s.step(&[]) != StateId::Stylefinish && s.rig.sora.combat().form.cmd == 0 {
            break;
        }
    }
    s.idle(60);
    let log = write(&s, "cams_guns_ss_midcombo");
    let states: Vec<String> = log.iter().filter(|l| l.contains("finish bazooka") || l.contains("Bazooka dtor") || l.contains("5417")).cloned().collect();
    eprintln!("{}", states.join("\n"));
    in_order(&log, &["start finish bazooka", "Bazooka dtor @15 / @27744: EndAllSameBPTresCamera", "core BpEvent 5417 param 0 begin"], "guns_ss_midcombo");
    assert!(!log.iter().any(|l| l.contains("auto end finish bazooka")), "the COMBO_BZ dtor ended it");
}
