//! (AUDIT-C LD) LIGHT FORM (form.LF, Oathkeeper, CHANGE_LF 0xDD, wear form 8) AND DARK FORM (form.DF, Oblivion, CHANGE_DF
//! 0xDE, wear form 9), 1:1 WITH KH3, THROUGH AUDIT-A's RECORDER (`kh_host::audit_a`): the real core, the real effect side
//! (main + keyblade effect roots, the held Keyblade's `vw_so200` / `vw_so210` effects let through as kh-er does) and the real
//! audio side (the audio pack's routes, the form weapon's swing / hit sets). One session per form:
//! - the offer (a full gauge, Y) -> STYLECHANGE (StyleChange / StyleChangeLoop / StyleChangeEnd, the charge effect, wear form);
//! - a walk, a jump and its landing (AnimSet22's LandB 8034 / LightLand* rows);
//! - a ground combo near the dummy (9 0x0A 0x0B, then Zantetsuken 0xCD, Finishing Leap 0xCE, Black Hole 0xCF / Dark Break
//!   0xD0) and an air combo (the air kinds, then Triple Edge 0xD1, Spinning Rush 0xD2, Spiral Charge 0xD3);
//! - Light / Dark Guard (Reflec 8279) hit from behind, then Light / Dark Burst (kind 0x25, BarrierCracker = MagicCast 1341 and
//!   its BARRIERCRACKER projectile); the same in the air (AirReflec 8280, AirBarrierCracker = MagicAirCast 1351);
//! - Sonic Dodge (DodgeRollB = M051) in battle;
//! - the form's end by its time (30 s): EndStyle, wear form 0.
//!
//! Every sound / voice / weapon-SE / effect notify of every clip played must be emitted by the seam AND resolved by the host
//! (`coverage`). Reports: build/sora_tmp/weapons/AUDIT-C/LD/logs/{lf,df}.report.txt and {lf,df}.events.txt. Skipped without
//! the pack.

use std::fmt::Write as _;
use std::path::PathBuf;

use glam::{Vec2, Vec3};
use kh_core::enums::ETresDamageKind;
use kh_core::states::HitOnPlayer;
use kh_core::{PadButton, StateId, TargetId};
use kh_host::audit_a::{clips, coverage, project, report, Session};

fn logs() -> PathBuf {
    let dir = project().join("build/sora_tmp/weapons/AUDIT-C/LD/logs");
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
    s.idle(15);
    press(s, PadButton::SITUATION);
    assert!(s.until(400, |s| s.rig.out.state == StateId::Stylechange), "STYLECHANGE starts");
    assert!(s.until(400, |s| s.rig.out.state != StateId::Stylechange), "STYLECHANGE ends");
    assert!(s.until(200, |s| s.rig.out.state == StateId::Idle), "back in IDLE");
    s.idle(5);
    s.rig.sora.combat().form.cmd
}

/// A hit from behind him (Light / Dark Guard guards every direction: Body reaction 3).
fn hit_from_behind(s: &mut Session) {
    let own = s.rig.sora.output().position;
    let from = own - Vec3::new(150.0, 0.0, 0.0);
    let d = (own - from).normalize_or_zero();
    s.rig.sora.take_hit(HitOnPlayer { direction: d, causer_yaw: 180.0, ..HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), from) });
}

/// A point head cm in front of him at height z.
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
                let _ = writeln!(t, "step {:5} {:?} {} @{:.3} form {:#x} wear {} grp {} z {:.1} | {short}", r.step, r.state, r.clip, r.time, r.form_cmd, r.wear_form, r.effect_group, r.position.z);
            }
        }
    }
    t
}

struct Done {
    kinds: Vec<u8>,
    clips: Vec<&'static str>,
    fails: Vec<String>,
}

fn session(weapon: &str, model: &str, name: &str, cmd: i32, wear: u8, third: u8) -> Option<Done> {
    let mut s = Session::new(weapon, &[model], model, Some(Vec3::new(300.0, 0.0, 0.0)), &format!("audit_c_{name}"))?;
    s.load_xsi_root(); // (kh-er's roots: fx.xsi is on by default)
    s.idle(5);
    // ---- the offer, the transformation
    assert_eq!(transform(&mut s), cmd, "{name}: the held Keyblade's own form");
    let f = s.rig.sora.combat().form;
    assert_eq!(f.wear_form, wear, "{name}: wear form");
    assert!(f.weapon_units()[0].ends_with(&format!("{model}_Wep")), "{name}: {:?}", f.weapon_units());
    // ---- locomotion: a walk away and back, a jump and its landing
    s.stick = Vec2::new(0.0, -1.0);
    s.idle(50);
    s.stick = Vec2::new(0.0, 1.0);
    s.idle(50);
    s.stick = Vec2::ZERO;
    s.idle(10);
    press(&mut s, PadButton::JUMP);
    assert!(s.until(200, |s| s.rig.out.state == StateId::Idle), "{name}: landed");
    s.idle(10);
    // ---- the ground combo
    let first = s.rig.sora.combat().history.len();
    for _ in 0..140 {
        press(&mut s, PadButton::ATTACK);
    }
    assert!(s.until(300, |s| s.rig.out.state == StateId::Idle), "{name}: the ground combo ends");
    s.idle(10);
    // ---- the air combo against a target in the air (a ground dummy pulls the air attacks down to its centre and he lands
    // before the third air finisher; KH3's flying enemies keep him up)
    s.rig.dummy = Some(kh_host::rig::Dummy::at(Vec3::new(250.0, 0.0, 220.0)));
    press(&mut s, PadButton::JUMP);
    s.idle(16);
    press(&mut s, PadButton::JUMP);
    s.idle(10);
    for _ in 0..240 {
        press(&mut s, PadButton::ATTACK);
    }
    assert!(s.until(400, |s| s.rig.out.state == StateId::Idle), "{name}: the air combo ends");
    s.rig.dummy = Some(kh_host::rig::Dummy::at(Vec3::new(300.0, 0.0, 0.0)));
    s.idle(10);
    let kinds = s.rig.sora.combat().history[first..].to_vec();
    assert!(kinds.contains(&0xCD) && kinds.contains(&0xCE) && kinds.contains(&third), "{name}: the ground finishers: {kinds:x?}");
    // ---- Aerial Sweep (0x1F, 1309: the base rise kind with the form's rows) at a target above him
    s.rig.dummy = Some(kh_host::rig::Dummy::at(in_front(&s, 150.0, 250.0)));
    for _ in 0..30 {
        press(&mut s, PadButton::ATTACK);
    }
    assert!(s.until(400, |s| s.rig.out.state == StateId::Idle), "{name}: down after Aerial Sweep");
    // ---- the form guard, hit from behind, the reprisal (ground): the dummy in front, in the burst's 550 cm
    s.rig.dummy = Some(kh_host::rig::Dummy::at(in_front(&s, 180.0, 0.0)));
    // ---- the form guard, hit from behind, the reprisal (ground)
    s.idle(40);
    press(&mut s, PadButton::GUARD);
    hit_from_behind(&mut s);
    s.idle(3);
    press(&mut s, PadButton::ATTACK);
    s.idle(100);
    // ---- the same in the air
    press(&mut s, PadButton::JUMP);
    s.idle(12);
    press(&mut s, PadButton::GUARD);
    hit_from_behind(&mut s);
    s.idle(3);
    press(&mut s, PadButton::ATTACK);
    assert!(s.until(300, |s| s.rig.out.state == StateId::Idle), "{name}: down after the air counter");
    s.idle(20);
    // ---- Sonic Dodge in battle (CanDodge 0x141427D98: weapon forms 0x16 / 0x17 need no DODGE)
    s.rig.sora.force_battle(true);
    s.idle(2);
    s.stick = Vec2::new(0.0, 1.0);
    press(&mut s, PadButton::GUARD);
    s.stick = Vec2::ZERO;
    s.idle(60);
    let history = s.rig.sora.combat().history.clone();
    assert!(history.contains(&0x25), "{name}: the counter kind 0x25: {history:x?}");
    // ---- the end by time
    assert!(s.until(2400, |s| s.rig.sora.combat().form.cmd == 0), "{name}: the form ends by its time");
    s.idle(60);
    assert_eq!(s.rig.sora.combat().form.wear_form, 0, "{name}: the normal clothes after the form");
    s.finish();
    let text = report(&s.rows);
    std::fs::write(logs().join(format!("{name}.report.txt")), &text).ok();
    std::fs::write(logs().join(format!("{name}.events.txt")), events_text(&s)).ok();
    let fails = coverage(&s.rows).into_iter().filter(|c| !c.ok).map(|c| format!("{} @{:.3} {} {} -> {}", c.clip, c.time, c.want.kind(), c.want.seam_name(), c.resolved)).collect();
    let played = clips(&s.rows).into_iter().map(|c| c.0).collect();
    Some(Done { kinds: s.rig.sora.combat().history.clone(), clips: played, fails })
}

fn check(d: Option<Done>, name: &str, must: &[&str]) {
    let Some(d) = d else { return };
    for c in must {
        assert!(d.clips.contains(c), "{name}: {c} not played: {:?}", d.clips);
    }
    assert!(d.fails.is_empty(), "{name}: {} notify(s) not emitted / resolved (kinds {:x?}):\n{}", d.fails.len(), d.kinds, d.fails.join("\n"));
}

#[test]
fn light_form_every_notify_is_played() {
    let d = session("WEP_KEYBLADE_SO_12", "w_so200", "lf", 0xDD, 8, 0xCF);
    check(d, "lf", &["1401", "1403", "M411", "M412", "M413", "M414", "M415", "M416", "8279", "1341", "8280", "1351", "M051", "8034", "1309"]);
}

#[test]
fn dark_form_every_notify_is_played() {
    let d = session("WEP_KEYBLADE_SO_13", "w_so210", "df", 0xDE, 9, 0xD0);
    check(d, "df", &["1401", "1403", "M411", "M412", "M420", "M414", "M415", "M416", "8279", "1341", "8280", "1351", "M051", "8034", "1309"]);
}

/// KH3's own Light / Dark Form rows in every clip: the effect notifies of group 0x834 (2100) / 0x898 (2200) (the forms' effect
/// groups: PlayAttack +0x198, Guard / DodgeRoll +0x90). Written to logs/inventory.txt (the inventory's source list).
fn form_rows(group: i32) -> Vec<(&'static str, f32, &'static str, &'static str)> {
    let mut out = Vec::new();
    for seq in kh_core::anim::SEQUENCES.iter() { // (PACKGEN) a run-time table in the release build (kh-gendata)
        for n in seq.notifies {
            if let kh_core::anim::NotifyData::Effect { entries } = n.data {
                for e in entries.iter().filter(|e| e.group_id == group) {
                    out.push((seq.name, n.time, e.effect.unwrap_or("-"), e.socket));
                }
            }
        }
    }
    out
}

#[test]
fn inventory_of_the_forms_own_effect_rows() {
    let mut t = String::new();
    for (form, group) in [("LF", 2100), ("DF", 2200)] {
        for (clip, time, effect, socket) in form_rows(group) {
            let _ = writeln!(t, "{form} {clip} @{time:.4} {effect} {socket}");
        }
    }
    std::fs::write(logs().join("inventory.txt"), &t).ok();
    assert!(!t.is_empty());
}

/// Every notify of the forms' own clips (class, time, end, data), for the inventory: logs/clip_notifies.txt.
#[test]
fn inventory_of_the_forms_clip_notifies() {
    let mut t = String::new();
    for name in ["1401", "1402", "1403", "M411", "M412", "M413", "M420", "M414", "M415", "M416", "8279", "8280", "1341", "1351", "M051", "1309", "8034", "8120", "8109", "8110"] {
        let Some(seq) = kh_core::anim::SEQUENCES.iter().find(|s| s.name == name) else {
            let _ = writeln!(t, "{name}: NOT IN SEQUENCES");
            continue;
        };
        let _ = writeln!(t, "== {name} length {:.4}", seq.length);
        for n in seq.notifies {
            let data: String = format!("{:?}", n.data).chars().take(300).collect();
            let _ = writeln!(t, "  {:.4}{} {:?} {data}", n.time, if n.state { "+" } else { "" }, n.class);
        }
    }
    std::fs::write(logs().join("clip_notifies.txt"), &t).ok();
}

/// (AUDIT-C LD) THE BEHAVIOUR ROWS through the same recorder: STYLECHANGE is invincible (Begin 0x1413CBA70 vt+0x2068(1));
/// Dark Break's warp and plunge are invincible (Decide +0x25C 0.5 s, PlayAttack head +0x1E1) and run the attack camera mode
/// 5 (PlayAttack case 0xD0 thunk_FUN_1414122A0 -> (ctrl, 1, 5, target)), off at End; its traction end lands it and the
/// eruption SCDF_DARKBREAK (TresProjDF_DarkBreak, vw_so210_dbr_hit0) hits with SCDF_DARKBREAK_END; Dark Burst's body is GHOST
/// (8) until End, then the default; the hit rows of every finisher land on the dummy. Writes logs/behaviour.txt.
#[test]
fn dark_form_behaviour_rows() {
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_13", &["w_so210"], "w_so210", Some(Vec3::new(300.0, 0.0, 0.0)), "audit_c_df_behaviour") else { return };
    s.load_xsi_root();
    s.idle(5);
    let mut t = String::new();
    // ---- STYLECHANGE invincible
    s.rig.sora.debug_add_style(100.0);
    s.idle(15);
    press(&mut s, PadButton::SITUATION);
    assert!(s.until(400, |s| s.rig.out.state == StateId::Stylechange), "STYLECHANGE starts");
    s.idle(20);
    let hp = s.rig.sora.status().hp;
    hit_from_behind(&mut s);
    s.idle(2);
    let _ = writeln!(t, "STYLECHANGE hit: HP {hp} -> {} state {:?}", s.rig.sora.status().hp, s.rig.out.state);
    assert_eq!(s.rig.sora.status().hp, hp, "the transformation is invincible");
    assert!(s.until(400, |s| s.rig.out.state == StateId::Idle), "back in IDLE");
    s.idle(10);
    // ---- the ground combo up to Dark Break; a hit during its warp / plunge
    let mut warp_hit = None;
    for k in 0..400 {
        let held: &[PadButton] = if k % 2 == 0 { &[PadButton::ATTACK] } else { &[] };
        s.step(held);
        if warp_hit.is_none() && s.rig.out.anim_ticked.body_samples()[0].0 == "0031" && s.rig.sora.combat().history.last() == Some(&0xD0) {
            let hp = s.rig.sora.status().hp;
            hit_from_behind(&mut s);
            s.idle(2);
            warp_hit = Some((hp, s.rig.sora.status().hp));
        }
        if k > 60 && s.rig.out.state == StateId::Idle {
            break;
        }
    }
    let (before, after) = warp_hit.expect("Dark Break's warp ran");
    let _ = writeln!(t, "Dark Break warp hit: HP {before} -> {after}");
    assert_eq!(before, after, "Dark Break's warp is invincible");
    // ---- Dark Burst's body
    s.rig.dummy = Some(kh_host::rig::Dummy::at(in_front(&s, 180.0, 0.0)));
    s.idle(40);
    press(&mut s, PadButton::GUARD);
    hit_from_behind(&mut s);
    s.idle(3);
    press(&mut s, PadButton::ATTACK);
    s.idle(10);
    // (AUDIT-B W-CORE) 0x1405B8B30(pawn+0x460, 8) sets MyBodyColl +0x4A4, his PUSH POWER LEVEL (PPL_90), not the body reaction
    // (pawn+0x619, which stays the default): kh-core `PawnActions::push_level` (AUDIT-B/W-CORE/SECTION.md row 1a).
    let during = (s.rig.sora.actions().reaction, s.rig.sora.actions().push_level);
    assert!(s.until(300, |s| s.rig.out.state == StateId::Idle), "Dark Burst ends");
    let after = (s.rig.sora.actions().reaction, s.rig.sora.actions().push_level);
    let _ = writeln!(t, "Dark Burst body reaction / push level: during {during:?}, after {after:?}");
    assert_eq!((during, after), ((0, Some(8)), (0, None)), "PPL_90 during Dark Burst, the base parameter's after");
    // ---- the rows: every accepted hit of the session, the attack camera events, the eruption
    for r in &s.rows {
        for e in &r.events {
            if e.contains("AttackCamera") || e.contains("Hit {") {
                let short: String = e.chars().take(260).collect();
                let _ = writeln!(t, "step {} {:?} {} | {short}", r.step, r.state, r.clip);
            }
        }
    }
    let events: Vec<&String> = s.rows.iter().flat_map(|r| r.events.iter()).collect();
    assert!(events.iter().any(|e| e.contains("AttackCamera { mode: 5")), "the attack camera mode 5");
    for row in ["SCLF_ZANTETSUKEN", "SCLF_FINISHRISE", "SCDF_DARKBREAK\"", "SCDF_DARKBREAK_END", "SCGM_COUNTER"] {
        assert!(events.iter().any(|e| e.contains(&format!("attack_name: \"{}", row.trim_end_matches('"'))) && (!row.ends_with('"') || e.contains("\"SCDF_DARKBREAK\""))), "{row} hit");
    }
    assert!(s.rows.iter().any(|r| r.effects.iter().any(|e| e.name == "vw_so210_dbr_hit0" && e.drawn > 0)), "the eruption is drawn");
    std::fs::write(logs().join("behaviour.txt"), &t).ok();
}
