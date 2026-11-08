//! (AUDIT-C) RAGE FORM, THE CORE AUDIT (build/sora_tmp/weapons/AUDIT-C/AN/PART.md).
//!
//! ROUND-2 F16: "Rage Form was never offered" (30 `dbg.hurt` hits at 22 % HP in a fight). The offer is KH3's roll in the
//! HP setter's tail (0x14140CE00) on every HP LOSS to <= 25 % MaxHP. Here the hits come the way the Elden Ring host hands
//! them over (kh-er sora.rs `damage_in` / `dbg.hurt` -> `Sora::take_hit`), never by writing the HP.

use std::path::PathBuf;

use glam::Vec3;
use kh_core::enums::ETresDamageKind;
use kh_core::states::HitOnPlayer;
use kh_core::{Input, TargetId};
use kh_host::seam::{AudioQueue, EffectQueue};

fn project() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn rig() -> Option<kh_host::rig::Rig> {
    let pack = project().join("build").join("sora_pack");
    let loadout = project().join("sheets").join("port").join("sora-loadout-99.json");
    if !pack.join("manifest.json").is_file() {
        eprintln!("SKIPPED: no pack under {}", pack.display());
        return None;
    }
    Some(kh_host::rig::Rig::new(&pack, &loadout, Some(Vec3::new(300.0, 0.0, 0.0))).expect("Sora's rig"))
}

fn step(rig: &mut kh_host::rig::Rig, n: usize) {
    for _ in 0..n {
        let (mut audio, mut effects) = (AudioQueue::default(), EffectQueue::default());
        rig.step_with(&Input::default(), &mut audio, &mut effects);
    }
}

/// An enemy's hit as kh-er hands it over (`dbg.hurt` / `damage_in`): `take_hit` with a causer 150 cm in front.
fn enemy_hit(rig: &mut kh_host::rig::Rig, amount: f32) {
    let at = rig.out.position;
    let from = at + Vec3::new(150.0, 0.0, 0.0);
    rig.sora.take_hit(HitOnPlayer { direction: (at - from).normalize_or_zero(), ..HitOnPlayer::new(ETresDamageKind::Small, amount, TargetId::NONE, from) });
}

#[test]
fn f16_hits_at_low_hp_roll_the_offer() {
    let Some(mut rig) = rig() else { return };
    rig.sora.force_battle(true);
    step(&mut rig, 30);
    let max = rig.sora.status().max_hp();
    rig.sora.status_mut().hp = max * 22 / 100;
    step(&mut rig, 5);
    let (rolls0, hp0) = (rig.sora.combat().form.rage.rolls, rig.sora.status().hp);
    for k in 0..30 {
        enemy_hit(&mut rig, 1.0);
        step(&mut rig, 90);
        eprintln!("hit {k}: hp {} state {} rolls {} offers {} situation 0x8B {}", rig.sora.status().hp, rig.out.state.value(), rig.sora.combat().form.rage.rolls, rig.sora.combat().form.rage.offers, rig.sora.combat().situation.has(0x8B));
        if rig.sora.combat().situation.has(0x8B) {
            break;
        }
    }
    let r = rig.sora.combat().form.rage;
    assert!(rig.sora.status().hp < hp0, "the hits took HP");
    assert!(r.rolls > rolls0, "each loss at <= 25 % rolls the offer (rolls {} -> {})", rolls0, r.rolls);
    assert!(rig.sora.combat().situation.has(0x8B) || r.offers > 0, "Rage Form offered within 30 hits (13 % each)");
}

// ------------------------------------------------------------------------------------------------ the whole session

use kh_core::{PadButton, StateId};
use kh_host::audit_a::{clips, coverage, write_report, Session};

/// Losses to 1 HP until the roll gives the offer (34 %), then Y and the transformation.
fn enter_rage(s: &mut Session) {
    s.rig.sora.force_battle(true);
    s.idle(10);
    let max = s.rig.sora.status().max_hp();
    let mut tries = 0;
    while !s.rig.sora.combat().situation.has(0x8B) && tries < 300 {
        s.rig.sora.status_mut().hp = max;
        s.idle(1);
        s.rig.sora.status_mut().hp = 1;
        s.idle(1);
        tries += 1;
    }
    assert!(s.rig.sora.combat().situation.has(0x8B), "Rage Form offered");
    s.idle(60);
    s.step(&[PadButton::SITUATION]);
    assert!(s.until(60, |s| s.rig.out.state == StateId::Stylechange), "the transformation starts");
    assert!(s.until(400, |s| s.rig.out.state != StateId::Stylechange && s.rig.sora.combat().form.cmd == 0x8B), "Rage Form starts");
}

/// The spawns of an effect by name so far (the seam's commands).
fn spawned(s: &Session, name: &str) -> usize {
    s.rows.iter().flat_map(|r| r.effects.iter()).filter(|e| e.stop.is_none() && e.name.eq_ignore_ascii_case(name)).count()
}

fn take_finish(s: &mut Session, cmd: i32) {
    s.idle(40);
    s.rig.sora.debug_add_style(100.0);
    assert!(s.until(30, |s| s.rig.sora.combat().situation.has(cmd)), "{cmd:#x} offered");
    s.idle(50);
    s.step(&[PadButton::SITUATION]);
    s.until(600, |s| s.rig.out.state != StateId::Stylefinish && s.rig.sora.combat().form.finish_cmd != cmd);
}

#[test]
fn rage_form_session_every_clip_effect_and_sound() {
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_00", &[], "w_so010", Some(Vec3::new(250.0, 0.0, 0.0)), "audit_c_an") else { return };
    // the XS effects root kh-er's fx.rs loads too (vp_ant_rsk_rsn00 / vp_ant_in_sho01 of M_D419 live there)
    s.load_xsi_root();
    enter_rage(&mut s);
    // on the floor first (StyleChange13 ends in the air)
    assert!(s.until(300, |s| s.rig.out.on_floor && s.rig.out.state == StateId::Idle), "he lands");
    // the ground combo: Crimson Upper, Vertical Drop, Blade Turn, Warp Thrust, Crimson Blast (an attack press every 6 steps)
    let mut kinds: Vec<u8> = Vec::new();
    let combo = |s: &mut Session, steps: usize, kinds: &mut Vec<u8>| {
        for k in 0..steps {
            s.step(if k % 6 == 0 { &[PadButton::ATTACK] } else { &[] });
            if let Some(a) = s.rig.sora.attack() {
                if kinds.last() != Some(&a.kind) {
                    kinds.push(a.kind);
                    eprintln!("  step {k}: kind {:#x} combo {} anim {:?}", a.kind, a.combo, s.rig.out.anim.name);
                }
            } else if k % 30 == 0 {
                eprintln!("  step {k}: state {:?} anim {:?}", s.rig.out.state, s.rig.out.anim.name);
            }
        }
    };
    combo(&mut s, 480, &mut kinds);
    eprintln!("ground kinds: {kinds:x?}");
    assert_eq!(kinds[..5], [0xBF, 0xC0, 0xC1, 0xC2, 0xC5], "the ground chain of Decide 0x141360000");
    // COMBO_ANTI's shot handler 0x141357AF0 with no Riskcharge: Crimson Blast's AN_CRIMSONBLAST, no AN_UPPER
    assert!(spawned(&s, "vp_ant_crm_bst00") > 0 && spawned(&s, "vp_ant_crm_upp0") == 0, "no Riskcharge: AN_CRIMSONBLAST only");
    s.idle(90);
    assert!(s.until(300, |s| s.rig.out.on_floor && s.rig.out.state == StateId::Idle), "back on the floor");
    // the air combo: Speed Slash, Vertical Slash, Blade Turn, Warp Thrust, Crimson Blast
    s.step(&[PadButton::JUMP]);
    s.idle(12);
    let mut air: Vec<u8> = Vec::new();
    combo(&mut s, 360, &mut air);
    eprintln!("air kinds: {air:x?}");
    assert_eq!(air[..5], [0xC3, 0xC4, 0xC1, 0xC2, 0xC5], "the air chain");
    s.idle(120);
    assert!(s.until(400, |s| s.rig.out.on_floor && s.rig.out.state == StateId::Idle), "landed");
    // a far target: the approach warp
    if let Some(d) = s.rig.dummy.as_mut() {
        d.info.position = s.rig.out.position + Vec3::new(800.0, 0.0, 0.0);
        d.info.center = d.info.position + Vec3::Z * 90.0;
    }
    s.idle(5);
    let w = (spawned(&s, "vs_ant_wrp_in01"), spawned(&s, "vs_ant_wrp_out01"));
    s.step(&[PadButton::ATTACK]);
    s.idle(60);
    // (AUDIT-C) the approach warp: the vanish (ead_pc_aura_com0 group 17) then, 0.2 s later, the reappearance (18), each on his
    // vfx_center and on the held Keyblade's vfx_bottom
    assert_eq!((spawned(&s, "vs_ant_wrp_in01") - w.0, spawned(&s, "vs_ant_wrp_out01") - w.1), (2, 2), "the warp's aura groups 17 / 18");
    // RISKCHARGE x 3, then FINISH_AN
    for n in 1..=3 {
        take_finish(&mut s, 0xB3);
        assert_eq!(s.rig.sora.combat().form.rage.riskcharges, n, "Riskcharge {n}");
        assert_eq!(s.rig.sora.combat().form.cmd, 0x8B, "the form goes on");
    }
    // three Riskcharges: Crimson Upper's three AN_UPPER (LocationIndex 1 .. 3 <= 3) and AN_CRIMSONBLAST3
    assert!(s.until(400, |s| s.rig.out.on_floor && s.rig.out.state == StateId::Idle), "on the floor for the combo");
    let before = (spawned(&s, "vp_ant_crm_upp0"), spawned(&s, "vp_ant_crm_bst03"));
    let mut charged: Vec<u8> = Vec::new();
    combo(&mut s, 360, &mut charged);
    eprintln!("charged kinds: {charged:x?}");
    assert_eq!(spawned(&s, "vp_ant_crm_upp0") - before.0, 3, "three AN_UPPER after three Riskcharges");
    assert!(spawned(&s, "vp_ant_crm_bst03") > before.1, "AN_CRIMSONBLAST3 after three Riskcharges");
    s.idle(60);
    take_finish(&mut s, 0xA5);
    assert!(s.until(600, |s| s.rig.sora.combat().form.cmd == 0), "FINISH_AN ends the form");
    // (AUDIT-C) EndStyle 0x14141CA20 case 0x8B: in battle the face row ANTI_02 (9997EM) on both face slots, through the pose host
    let face_events: Vec<String> = s.rows.iter().flat_map(|r| r.events.iter()).filter(|e| e.contains("FaceAnim")).cloned().collect();
    assert!(face_events.len() == 1 && face_events[0].contains("ANTI_02"), "one face play ANTI_02 (in battle) at the end: {face_events:?}");
    // the pose host's face path with the pack's face/face_extra.json: the row plays 9997EM on both slots
    assert!(s.rig.animator.play_face_row(Some("ANTI_02"), "NORMAL_AUTO", [true, true], -1.0, false), "ANTI_02 is a face row of the pack");
    assert!(s.rig.animator.play_face_row(Some("ANTI_01"), "NORMAL_AUTO", [true, true], -1.0, false), "ANTI_01 is a face row of the pack");
    assert_eq!(s.rig.animator.face_playing().map(|f| f.map(|(clip, _)| clip.to_string())), [Some("9998EM".to_string()), Some("9998EM".to_string())], "ANTI_01 = 9998EM on both slots");
    s.idle(120);
    let (form_after, state_after, fin_after) = (s.rig.sora.combat().form.cmd, s.rig.out.state, s.rig.sora.combat().form.finish_cmd);
    s.finish();
    let text = write_report(&s.rows, "audit_c_an");
    let dir = kh_host::audit_a::project().join("build/sora_tmp/weapons/AUDIT-C/AN/logs");
    std::fs::create_dir_all(&dir).ok();
    std::fs::write(dir.join("audit_c_an.report.txt"), &text).ok();
    assert_eq!(form_after, 0, "FINISH_AN ended the form (state {state_after:?}, finish {fin_after:#x})");
    let played: Vec<&str> = clips(&s.rows).iter().map(|c| c.0).collect();
    for want in ["D401", "D411", "D412", "D413", "D415", "D416", "D417", "D419", "D418"] {
        assert!(played.contains(&want), "{want} played: {played:?}");
    }
    let fails: Vec<String> = coverage(&s.rows).iter().filter(|c| !c.ok).map(|c| format!("{c:?}")).collect();
    eprintln!("{text}");
    assert!(fails.is_empty(), "notifies not played / not resolved:\n{}", fails.join("\n"));
}

/// (AUDIT-C) KH3's in-form counter: OnGuardedHit 0x14143CAF0 case 0x8B. A hit on Rage Form's guard is a back slide away
/// from the attacker (AIR_SLIDE flag 0x10 + target); its Begin (flag 0x100) arms reaction command 0xD; Attack then plays
/// Rage Raid (kind 0xC6, M_D420) on the attacker. The aura (group 100) ends for the slide and comes back at its End.
#[test]
fn rage_guard_back_slide_and_rage_raid() {
    let Some(mut s) = Session::new("WEP_KEYBLADE_SO_00", &[], "w_so010", Some(Vec3::new(250.0, 0.0, 0.0)), "audit_c_an_guard") else { return };
    s.load_xsi_root();
    enter_rage(&mut s);
    assert!(s.until(300, |s| s.rig.out.on_floor && s.rig.out.state == StateId::Idle), "he lands");
    s.step(&[PadButton::GUARD]);
    assert!(s.until(20, |s| s.rig.out.state == StateId::Guard), "Rage Form guards");
    let aura_before = s.fx.world.live().iter().filter(|l| l.name.eq_ignore_ascii_case("vp_ant_bdy_ora00") && !l.ending).count();
    let from = s.rig.out.position + Vec3::new(150.0, 0.0, 0.0);
    s.rig.sora.take_hit(HitOnPlayer { direction: Vec3::NEG_X, causer_yaw: 180.0, ..HitOnPlayer::new(ETresDamageKind::Small, 10.0, TargetId(1), from) });
    let hp = s.rig.sora.status().hp;
    assert!(s.until(10, |s| s.rig.out.state == StateId::AirSlide), "the guard became the back slide: {:?}", s.rig.out.state);
    assert_eq!(s.rig.sora.status().hp, hp, "guarded: no HP lost");
    s.idle(2);
    assert_eq!(s.rig.sora.actions().reaction_command.map(|c| c.0), Some(0xD), "Rage Raid's reaction command");
    let aura_during = s.fx.world.live().iter().filter(|l| l.name.eq_ignore_ascii_case("vp_ant_bdy_ora00") && !l.ending).count();
    s.step(&[PadButton::ATTACK]);
    assert!(s.until(120, |s| s.rig.sora.attack().is_some_and(|a| a.kind == 0xC6)), "Rage Raid (0xC6)");
    assert!(s.until(30, |s| format!("{:?}", s.rig.out.anim.name).contains("D420")), "M_D420 plays");
    let aura_after = s.fx.world.live().iter().filter(|l| l.name.eq_ignore_ascii_case("vp_ant_bdy_ora00") && !l.ending).count();
    eprintln!("aura before {aura_before} during {aura_during} after {aura_after}");
    assert!(aura_before >= 4 && aura_during == 0 && aura_after >= 4, "the aura ends for the slide and is back after it");
    s.idle(120);
    s.finish();
    let text = write_report(&s.rows, "audit_c_an_guard");
    std::fs::write(kh_host::audit_a::project().join("build/sora_tmp/weapons/AUDIT-C/AN/logs/audit_c_an_guard.report.txt"), &text).ok();
    let fails: Vec<String> = coverage(&s.rows).iter().filter(|c| !c.ok).map(|c| format!("{c:?}")).collect();
    assert!(fails.is_empty(), "notifies not played / not resolved:\n{}", fails.join("\n"));
}
