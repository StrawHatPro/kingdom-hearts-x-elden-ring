//! (COMBAT-FIX) THE USER, 2026-11-06: "rage form's normal attacks do no damage, fix this".
//!
//! Rage Form's combo (COMBO_ANTI, kinds 0xBF..0xC5), RISKCHARGE and FINISH_AN on a dummy through the whole hosted path (the
//! rig: pose, notifies, hit windows): every hit must reach the dummy with damage, through the normal hit path with KH3's row
//! (`p_ex001_AttackData` AN_*: the record's power is the row's), the same formula as the base combo's KB_* rows. Before the
//! fix only Crimson Blast's shots hit: AnimSet13's clips had no socket track (their KB_* windows had no shape) and the pawn's
//! AN_LHAND / AN_Finish / AN_FinishEnd had no place (build/sora_tmp/weapons/COMBAT-FIX/STATE.md).
//!
//! `KH_COMBAT_FIX_TRACE=1` prints every step (state, clip, open windows, the dummy).

use std::path::PathBuf;

use glam::Vec3;
use kh_core::host::EventKind;
use kh_core::{Input, PadButton, PadButtons, PadState, StateId};
use kh_host::seam::{AudioQueue, EffectQueue};

/// One hit: (row, damage asked, damage dealt, the record's power).
type Hit = (String, f32, f32, f32);

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
    Some(kh_host::rig::Rig::new(&pack, &loadout, Some(Vec3::new(250.0, 0.0, 0.0))).expect("Sora's rig"))
}

/// Keeps the dummy 150 cm in front of Sora (his moves carry him) and at full HP.
fn keep_dummy(rig: &mut kh_host::rig::Rig) {
    let (at, yaw) = (rig.out.position, rig.out.yaw.to_radians());
    if let Some(d) = rig.dummy.as_mut() {
        d.info.position = Vec3::new(at.x + 150.0 * yaw.cos(), at.y + 150.0 * yaw.sin(), 0.0);
        d.info.center = d.info.position + Vec3::Z * 90.0;
        d.info.hp = d.info.hp_max;
    }
}

/// One step with `held`; the step's hits.
fn step(rig: &mut kh_host::rig::Rig, held: &[PadButton]) -> Vec<Hit> {
    keep_dummy(rig);
    let buttons = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
    let input = Input::default().with_pad(PadState::from_held(buttons, PadButtons::NONE));
    let (mut audio, mut effects) = (AudioQueue::default(), EffectQueue::default());
    let out = rig.step_with(&input, &mut audio, &mut effects);
    let hits = out.events.iter().filter_map(|e| if let EventKind::Hit { record, result } = &e.kind { Some((record.attack_name.to_string(), record.damage, result.damage_dealt, record.info.power)) } else { None }).collect();
    if std::env::var_os("KH_COMBAT_FIX_TRACE").is_some() {
        let w: Vec<_> = rig.sora.combat().windows.iter().map(|w| (w.group, w.row_name)).collect();
        eprintln!("{:?} {:?} t {:.2} pos {:?} win {w:?}", rig.out.state, rig.out.anim.name, rig.out.anim.time, rig.out.position);
    }
    hits
}

fn idle(rig: &mut kh_host::rig::Rig, n: usize) -> Vec<Hit> {
    (0..n).flat_map(|_| step(rig, &[])).collect()
}

fn until(rig: &mut kh_host::rig::Rig, n: usize, done: impl Fn(&kh_host::rig::Rig) -> bool) -> Vec<Hit> {
    let mut hits = Vec::new();
    for _ in 0..n {
        if done(rig) {
            break;
        }
        hits.extend(step(rig, &[]));
    }
    hits
}

fn standing(rig: &kh_host::rig::Rig) -> bool {
    rig.out.on_floor && rig.out.state == StateId::Idle
}

/// An attack press every 6 steps for `steps` steps; the hits.
fn combo(rig: &mut kh_host::rig::Rig, steps: usize) -> Vec<Hit> {
    (0..steps).flat_map(|k| step(rig, if k % 6 == 0 { &[PadButton::ATTACK] } else { &[] })).collect()
}

fn enter_rage(rig: &mut kh_host::rig::Rig) {
    rig.sora.force_battle(true);
    idle(rig, 10);
    let max = rig.sora.status().max_hp();
    let mut tries = 0;
    while !rig.sora.combat().situation.has(0x8B) && tries < 300 {
        rig.sora.status_mut().hp = max;
        idle(rig, 1);
        rig.sora.status_mut().hp = 1;
        idle(rig, 1);
        tries += 1;
    }
    assert!(rig.sora.combat().situation.has(0x8B), "Rage Form offered");
    idle(rig, 60);
    step(rig, &[PadButton::SITUATION]);
    until(rig, 900, |r| r.sora.combat().form.cmd == 0x8B && standing(r));
    assert_eq!(rig.sora.combat().form.cmd, 0x8B, "Rage Form is on");
}

/// A full gauge, the finish command `cmd` on Y; the hits until it is over.
fn finish(rig: &mut kh_host::rig::Rig, cmd: i32) -> Vec<Hit> {
    idle(rig, 40);
    rig.sora.debug_add_style(100.0);
    until(rig, 30, |r| r.sora.combat().situation.has(cmd));
    assert!(rig.sora.combat().situation.has(cmd), "{cmd:#x} offered");
    idle(rig, 50);
    let mut hits = step(rig, &[PadButton::SITUATION]);
    hits.extend(until(rig, 60, |r| r.out.state == StateId::Stylefinish));
    assert_eq!(rig.out.state, StateId::Stylefinish, "{cmd:#x} taken");
    hits.extend(until(rig, 600, |r| r.out.state != StateId::Stylefinish));
    hits.extend(until(rig, 400, standing));
    hits
}

/// Every hit dealt damage, with its row's power (KH3's p_ex001_AttackData row through the normal hit path).
fn all_damage(what: &str, hits: &[Hit]) {
    for (row, damage, dealt, power) in hits {
        let want = kh_core::combat::tables::ATTACK_DATA.iter().find(|r| r.id.eq_ignore_ascii_case(row)).map(|r| r.power);
        assert_eq!(want, Some(*power), "{what}: {row} hit with its row's power");
        assert!(*damage > 0.0 && *dealt == *damage, "{what}: {row} (power {power}) dealt {dealt} of {damage}");
    }
}

fn rows(hits: &[Hit]) -> Vec<&str> {
    let mut r: Vec<&str> = hits.iter().map(|h| h.0.as_str()).collect();
    r.dedup();
    r
}

#[test]
fn rage_forms_attacks_deal_kh3s_damage_on_every_hit() {
    let Some(mut rig) = rig() else { return };
    // the base combo first: the reference (KB_* rows through the same formula)
    rig.sora.force_battle(true);
    idle(&mut rig, 30);
    let base = combo(&mut rig, 240);
    eprintln!("base combo hits: {base:?}");
    assert!(!base.is_empty(), "the base combo hits");
    all_damage("base", &base);
    idle(&mut rig, 120);
    enter_rage(&mut rig);
    // the ground chain: Crimson Upper, Vertical Drop, Blade Turn, Warp Thrust, Crimson Blast
    let ground = combo(&mut rig, 480);
    eprintln!("rage ground hits: {ground:?}");
    all_damage("ground", &ground);
    for row in ["AN_CRIMSONUPPER", "AN_CRIMSONUPPER_END", "AN_VERTICALDROP", "AN_VERTICALDROP_END", "AN_BLADETURN", "AN_WARPTHRUST", "AN_CRIMSONBLAST"] {
        assert!(ground.iter().any(|h| h.0 == row), "ground: {row} hits: {:?}", rows(&ground));
    }
    idle(&mut rig, 120);
    until(&mut rig, 400, standing);
    // the air chain: Speed Slash, Vertical Slash, Blade Turn, Warp Thrust, Crimson Blast
    step(&mut rig, &[PadButton::JUMP]);
    idle(&mut rig, 12);
    let air = combo(&mut rig, 360);
    eprintln!("rage air hits: {air:?}");
    all_damage("air", &air);
    for row in ["AN_SPEEDSLASH", "AN_SPEEDSLASH_SCRATCH", "AN_VERTICALSLASH", "AN_VERTICALSLASH_END"] {
        assert!(air.iter().any(|h| h.0 == row), "air: {row} hits: {:?}", rows(&air));
    }
    idle(&mut rig, 120);
    until(&mut rig, 400, standing);
    // the Y moves: RISKCHARGE x 3, then FINISH_AN
    for n in 1..=3 {
        let risk = finish(&mut rig, 0xB3);
        eprintln!("riskcharge {n} hits: {risk:?}");
        all_damage("riskcharge", &risk);
        assert!(risk.iter().any(|h| h.0 == "AN_RISKCHARGE"), "Riskcharge {n}: AN_RISKCHARGE hits: {:?}", rows(&risk));
    }
    let fin = finish(&mut rig, 0xA5);
    eprintln!("finish hits: {fin:?}");
    all_damage("finish", &fin);
    for row in ["AN_FINISH", "AN_FINISH_BOMB", "AN_FINISH_END"] {
        assert!(fin.iter().any(|h| h.0 == row), "FINISH_AN: {row} hits: {:?}", rows(&fin));
    }
}
