//! (AUDIT-B W-CORE) THE GENERIC ATTACK / FORM FIXES, END TO END: the real core -> the effect seam -> `kh_host::vfx::EffectHost`
//! with the live pack (AUDIT-A's `kh_host::audit_a::Session`). Proven here, each with its trace in
//! build/sora_tmp/weapons/AUDIT-B/W-CORE/traces/:
//! 1. his push power level (MyBodyColl +0x4A4, kh-core `PawnActions::push_level`): PPL_90 (8) while PlayAttack's case 0x25
//!    (Boom Hammer's Barrier Cracker, a guard counter) and case 0x6A (Clock Drill's giga upper) run, back to the base
//!    parameter's (None = PPL_50, 4) at their End; what the push solver then weighs against an enemy body of each class
//!    kh-er gives (2 / 5 / 8);
//! 2. Final Blow's chase warp (Begin 0x141333210: +0x22C = 0xD): ead_pc_aura_com0 group 13 (vp_gct_ctr_blr00,
//!    vs_ddg_wrp_in01 / 03 / 04) with JumpNormalStart at its head, group 14 (vs_ddg_wrp_out01 / 03 / 04) when the 0.2 s
//!    countdown teleports him, then ChaseAttack; with no form (the Kingdom Key) and in a form class of its own (Clock Drill's
//!    COMBO_HM), each effect drawn by the effect side;
//! 3. the form's equipment as KH3 swaps it (kh-core `FormEquip`): the Keyblade until StyleChange's first Tick, then the
//!    transform units ALONE (Hyper Hammer: w_so051, which carries the hammer head in itself), the form's own units (w_so052)
//!    from StyleChange's End: one hammer on screen, never two.
//!
//! Skipped, saying so, without the pack.

use std::fmt::Write as _;
use std::path::PathBuf;

use glam::Vec3;
use kh_core::combat::form::FormEquip;
use kh_core::enums::ETresDamageKind as Kind;
use kh_core::states::HitOnPlayer;
use kh_core::{PadButton, StateId, TargetId};
use kh_host::audit_a::{self, Session};

fn traces() -> PathBuf {
    let dir = audit_a::project().join("build").join("sora_tmp").join("weapons").join("AUDIT-B").join("W-CORE").join("traces");
    std::fs::create_dir_all(&dir).ok();
    dir
}

/// The gauge full, Triangle on the offered formchange, the transformation to its end; one line per step of it.
fn transform(s: &mut Session, cmd: i32, log: &mut String) {
    s.rig.sora.debug_add_style(100.0);
    s.idle(20);
    assert!(s.rig.sora.combat().situation.has(cmd), "{cmd:#x} offered");
    s.step(&[PadButton::SITUATION]);
    for _ in 0..900 {
        let state = s.step(&[]);
        let f = &s.rig.sora.combat().form;
        let _ = writeln!(log, "step {} state {state:?} form {:#x} clip {} t {:.3} equip {:?}", s.rig.out.step, f.cmd, s.rig.out.anim.name.name(), s.rig.out.anim.time, f.equip);
        if state != StateId::Stylechange && f.cmd == cmd {
            return;
        }
    }
    panic!("STYLECHANGE to {cmd:#x} did not end");
}

/// A guarded hit of the dummy, then Attack in the counter window: the guard counter.
fn guard_counter(s: &mut Session) {
    s.idle(30);
    s.step(&[PadButton::GUARD]);
    s.idle(8);
    let at = s.rig.dummy.as_ref().map_or(Vec3::new(200.0, 0.0, 0.0), |d| d.info.position);
    let from = (at - s.rig.out.position).truncate().normalize_or_zero().extend(0.0);
    s.rig.sora.take_hit(HitOnPlayer { direction: -from, causer_yaw: 180.0, ..HitOnPlayer::new(Kind::Small, 30.0, TargetId(1), at) });
    s.step(&[]);
    s.step(&[PadButton::ATTACK]);
}

/// The trace of his push power level: (step, attack kind or 0, level the solver uses).
fn push_trace(s: &mut Session, steps: usize, press: bool, log: &mut String, seen: &mut Vec<(u8, Option<u8>)>) {
    for k in 0..steps {
        let state = if press && k % 2 == 0 { s.step(&[PadButton::ATTACK]) } else { s.step(&[]) };
        let kind = s.rig.sora.attack().map_or(0, |a| a.kind);
        let level = s.rig.sora.actions().push_level;
        seen.push((kind, level));
        let _ = writeln!(log, "step {} state {state:?} kind {kind:#04x} clip {} push_level {level:?}", s.rig.out.step, s.rig.out.anim.name.name());
    }
}

#[test]
fn the_counters_and_the_giga_upper_raise_his_push_power_until_their_end() {
    let models = ["w_so110", "w_so112", "w_so111", "w_so113", "w_so114"];
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_11", &models, "w_so110", Some(Vec3::new(200.0, 0.0, 0.0)), "auditb_core_push") else { return };
    let mut log = String::from("# (AUDIT-B W-CORE) his push power level (MyBodyColl +0x4A4; None = the base parameter's PPL_50)\n");
    s.idle(5);
    transform(&mut s, 0x92, &mut log);
    let mut seen = Vec::new();
    log.push_str("# Boom Hammer: a guard counter (Barrier Cracker, PlayAttack case 0x25)\n");
    guard_counter(&mut s);
    push_trace(&mut s, 240, false, &mut log, &mut seen);
    transform(&mut s, 0x93, &mut log);
    log.push_str("# Clock Drill: combos with one enemy (Decide: enemies < 3 -> the giga upper 0x6A)\n");
    for _ in 0..4 {
        push_trace(&mut s, 400, true, &mut log, &mut seen);
        s.idle(40);
        if seen.iter().any(|(k, _)| *k == 0x6A) {
            break;
        }
    }
    // the solver's weight of HIS push against an Elden Ring enemy body of each class (kh-er ermap.rs: small 2, medium 5, large 8)
    log.push_str("# combat/push.rs weight(his level, the enemy's): 1 = he is pushed fully, 0.5 half, 0 not\n");
    for lv in [4u8, 8] {
        let _ = writeln!(log, "his level {lv}: small {} medium {} large {}", kh_core::combat::push::weight(lv, 2), kh_core::combat::push::weight(lv, 5), kh_core::combat::push::weight(lv, 8));
    }
    std::fs::write(traces().join("push_level.trace"), &log).ok();
    assert!(seen.iter().any(|(k, _)| *k == 0x25), "the guard counter ran (Barrier Cracker): see push_level.trace");
    for (kind, level) in &seen {
        match kind {
            0x25 | 0x6A => assert_eq!(*level, Some(8), "PPL_90 while kind {kind:#x} runs"),
            0 => assert_eq!(*level, None, "back to the base parameter's after the attack's End"),
            _ => {}
        }
    }
    assert!(seen.iter().any(|(k, _)| *k == 0x6A), "the giga upper 0x6A ran: see push_level.trace");
    assert_eq!((kh_core::combat::push::weight(8, 5), kh_core::combat::push::weight(8, 8), kh_core::combat::push::weight(4, 5)), (0.0, 0.5, 1.0));
}

/// Arms Final Blow's chase as the chain's End hands it over (+0x234 bit 19 -> `chase_next`) and presses Attack: the chase's
/// steps logged with the effects of each; returns (first step of the chase, the step ChaseAttack started).
fn chase(s: &mut Session, log: &mut String) -> (u64, u64) {
    s.idle(30);
    s.rig.sora.combat_mut().chase_next = true;
    s.step(&[PadButton::ATTACK]);
    let mut first = 0;
    let mut started = 0;
    for _ in 0..90 {
        let row = s.rows.last().expect("a row");
        let fx: Vec<String> = row.effects.iter().filter(|e| e.stop.is_none()).map(|e| format!("{} @{} drawn {}", e.name, e.attach, e.drawn)).collect();
        let _ = writeln!(log, "step {} state {:?} kind {:#04x} clip {} ({}) t {:.3} | {}", row.step, row.state, s.rig.sora.attack().map_or(0, |a| a.kind), row.clip, s.rig.out.anim.name.name(), row.time, fx.join(" ; "));
        if first == 0 && s.rig.sora.attack().is_some_and(|a| a.kind == 0x22) {
            first = row.step;
        }
        if row.clip == "1290" /* ChaseAttack */ && started == 0 {
            started = row.step;
        }
        if started != 0 && row.step > started + 5 {
            break;
        }
        s.step(&[]);
    }
    (first, started)
}

fn chase_effects(s: &Session, from: u64, to: u64) -> Vec<(u64, String, String, usize)> {
    s.rows.iter().filter(|r| r.step >= from && r.step <= to).flat_map(|r| r.effects.iter().filter(|e| e.stop.is_none()).map(move |e| (r.step, e.name.clone(), e.attach.clone(), e.drawn))).collect()
}

fn check_chase(s: &Session, log: &str, first: u64, started: u64, what: &str) {
    assert!(started > first, "{what}: ChaseAttack played");
    let fx = chase_effects(s, first, started);
    let at = |name: &str| fx.iter().filter(|f| f.1.eq_ignore_ascii_case(name)).map(|f| (f.0, f.3)).collect::<Vec<_>>();
    // group 13 at the head (the first step), group 14 with the teleport (0.2 s later, the step ChaseAttack starts)
    for name in ["vp_gct_ctr_blr00", "vs_ddg_wrp_in03", "vs_ddg_wrp_in04"] {
        let a = at(name);
        assert!(a.iter().any(|(st, drawn)| *st == first && *drawn > 0), "{what}: {name} at the vanish (step {first}) and drawn: {a:?} / {fx:?}");
    }
    assert_eq!(at("vs_ddg_wrp_in01").iter().filter(|(st, _)| *st == first).count(), 2, "{what}: vs_ddg_wrp_in01 twice (root and vfx_center)");
    for name in ["vs_ddg_wrp_out03", "vs_ddg_wrp_out04"] {
        let a = at(name);
        assert!(a.iter().any(|(st, drawn)| *st == started - 1 && *drawn > 0), "{what}: {name} at the reappearance (the step before ChaseAttack) and drawn: {a:?}");
    }
    assert_eq!(at("vs_ddg_wrp_out01").iter().filter(|(st, _)| *st == started - 1).count(), 2, "{what}: vs_ddg_wrp_out01 twice");
    let gap = started - 1 - first;
    assert!((11..=13).contains(&gap), "{what}: the 0.2 s countdown ({gap} steps)");
    let head = format!("step {} ", first + 1);
    assert!(log.lines().any(|l| l.starts_with(&head) && l.contains("(JumpNormalStart)")), "{what}: PlayAttack's head plays JumpNormalStart");
}

#[test]
fn final_blows_chase_warp_spawns_aura_groups_13_and_14_with_and_without_a_form() {
    let mut log = String::from("# (AUDIT-B W-CORE) Final Blow's chase warp: ead_pc_aura_com0 groups 13 (head) / 14 (teleport)\n");
    // no form: the Kingdom Key (attack.rs)
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_00", &["w_so010"], "w_so010", Some(Vec3::new(600.0, 0.0, 0.0)), "auditb_core_chase") else { return };
    log.push_str("# no form (Kingdom Key)\n");
    let (first, started) = chase(&mut s, &mut log);
    std::fs::write(traces().join("chase_warp.trace"), &log).ok();
    check_chase(&s, &log, first, started, "no form");
    // a form class of its own: Clock Drill's COMBO_HM (combo_hm.rs Begin)
    let models = ["w_so110", "w_so112", "w_so111", "w_so113", "w_so114"];
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_11", &models, "w_so110", Some(Vec3::new(600.0, 0.0, 0.0)), "auditb_core_chase_hm") else { return };
    let pack = audit_a::project().join("build").join("sora_pack");
    s.fx.load_xsi_root(&pack).expect("the XS effect root");
    let mut tl = String::new();
    transform(&mut s, 0x92, &mut tl);
    log.push_str("# Boom Hammer (COMBO_HM)\n");
    let (first, started) = chase(&mut s, &mut log);
    std::fs::write(traces().join("chase_warp.trace"), &log).ok();
    check_chase(&s, &log, first, started, "Boom Hammer");
}

/// The form's units the pack lists (PACK/keyblades/manifest.json `forms[]`): (weapons, transform units) models.
fn pack_units(keyblade: &str, form: &str) -> (Vec<String>, Vec<String>) {
    let path = audit_a::project().join("build").join("sora_pack").join("keyblades").join("manifest.json");
    let m: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).expect("the keyblades manifest")).expect("json");
    let f = m["keyblades"][keyblade]["forms"].as_array().and_then(|a| a.iter().find(|f| f["form"] == form && f["grow_level"] == "TRES_WEAPON_GROW_LVL01")).cloned().unwrap_or_default();
    let models = |k: &str| f[k].as_array().map(|a| a.iter().filter_map(|u| u["model"].as_str().map(str::to_string)).collect()).unwrap_or_default();
    (models("weapons"), models("transform"))
}

#[test]
fn the_form_equipment_follows_kh3s_equip_calls() {
    let models = ["w_so050", "w_so052", "w_so051", "w_so053"];
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_03", &models, "w_so050", Some(Vec3::new(200.0, 0.0, 0.0)), "auditb_core_equip") else { return };
    let mut log = String::from("# (AUDIT-B W-CORE) the form's equipment through StyleChange3 (Hyper Hammer)\n");
    s.idle(5);
    transform(&mut s, 0x81, &mut log);
    s.idle(10);
    let _ = writeln!(log, "after: equip {:?}", s.rig.sora.combat().form.equip);
    let (weapons, units) = pack_units("w_so050", "TRES_WEAPON_FORM_03");
    let _ = writeln!(log, "pack: weapons {weapons:?} transform units {units:?}");
    // what kh-er-render `equipped_parts` then draws (the same rule: the transform units alone when there are any)
    let drawn = |e: FormEquip| match e {
        FormEquip::Keyblade => vec!["(the hand keyblade w_so050)".to_string()],
        FormEquip::Transform if !units.is_empty() => units.clone(),
        _ => weapons.clone(),
    };
    let lines: Vec<String> = log.lines().filter(|l| l.contains("state Stylechange")).map(str::to_string).collect();
    let _ = writeln!(log, "# drawn during StyleChange: {:?}", lines.iter().filter_map(|l| l.split("equip ").nth(1)).collect::<std::collections::BTreeSet<_>>());
    std::fs::write(traces().join("form_equip.trace"), &log).ok();
    assert_eq!((weapons.as_slice(), units.as_slice()), (&["w_so052".to_string()][..], &["w_so051".to_string()][..]), "Hyper Hammer's units in the pack");
    assert!(!lines.is_empty() && lines.iter().all(|l| l.ends_with("equip Transform")), "the transform units through StyleChange's ticks: {lines:?}");
    assert_eq!(drawn(FormEquip::Transform), vec!["w_so051".to_string()], "one hammer: the unit's own");
    assert_eq!(s.rig.sora.combat().form.equip, FormEquip::Weapons, "StyleChange's End: the form's own units");
    assert_eq!(drawn(s.rig.sora.combat().form.equip), vec!["w_so052".to_string()]);
}
