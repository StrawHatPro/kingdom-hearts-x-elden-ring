//! (SHOT-C) SHOT-C's kinds that run in families of other owners' code (traces: lock-on -> fire -> finish -> back to normal):
//! - Dead of Night (weapon type 19): kind 34 `SE_SHOT` with the Kingdom Key's King of Hearts update 0x1413B8C90 and kind 35
//!   `SE_RAGNAROK` with its Ragnarok update 0x1413B88F0 (Begin 0x1413A5CD0 cases 0x22 / 0x23: delegates vt+0x408 /
//!   vt+0x410, groups 0x631 / 0). Its projectile set w_so120_ProjSet: TresProjSE_Shot_C / TresProjSE_Ragnarok_C, whose
//!   class defaults equal TresProjKC_Shot / TresProjKC_Ragnarok (rows SF_KCSHOT / SF_KCRAGNAROK). So Dead of Night's
//!   shotlocks are King of Hearts and Ragnarok with kind numbers 34 / 35 (its display abilities are KB00_00 / KB00_01).
//! - Starlight (weapon type 18) at MAX: kind 28 `SL_RAGNAROK` with the Ragnarok update, effect group 0x938 (the clip's
//!   charge effect of that group is vw_so150_rgn_tam00), TresProjSL_Ragnarok_C (Starlight's particles; row SF_KCRAGNAROK).
//!   Its base kind is 27 (burst); the MAX kind comes from weapon type 18 (SelectKind 0x141411B80).
//! - Starlight (18) and Oblivion (14), base: kinds 27 `SL_SHOT` (group 0x938) and 41 `DF_BURST` (group 0x898) with the
//!   burst update 0x1413BADB0 (SHOT-B's family): TresProjSL_Shot_C / TresProjDF_Burst_C, rows SF_SLBURST / SF_DFBURST.
//! - Oathkeeper (weapon type 13): kind 40 `LF_SHOOT` with Meteor Shower's update 0x1413BA090 (SHOT-A's meteor family),
//!   group 0x834, TresProjLF_Shoot_C (a TresProj_Ragnarok subclass), row SF_LFSHOOT.

mod shotlocks_common;

use kh_core::enums::ETresDamageKind;
use kh_core::{AnimId, StateId, TargetId, TargetInfo};
use shotlocks_common::{dummy, level99_with, seconds, Bench};

fn two() -> [TargetInfo; 2] {
    [dummy(1, 800.0, 0.0), dummy(2, 800.0, 150.0)]
}

/// Dead of Night, base: King of Hearts' timeline (M_1473 -> the volley at 2.5 s -> M_1474 -> ShootFlowEnd, about 4.0 s,
/// lifted 500 cm), one TresProjSE_Shot per lock, SF_KCSHOT hits with the ShootFlow command and kind 34.
#[test]
fn dead_of_night_king_of_hearts_kind_34() {
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_18"), &two());
    let kind = b.aim_until(8, 300);
    assert_eq!(kind, 34, "Dead of Night's base kind (weapon type 19)");
    assert_eq!(b.sora.aim().max_locks, 24, "TresShootFlowData row 34 Param0");
    let n = b.sora.aim().total as usize;
    b.fire();
    let trace = b.trace_shoot_flow(600);
    assert!(trace.iter().any(|s| s.clip == AnimId::M1473) && trace.iter().any(|s| s.clip == AnimId::M1474) && trace.iter().any(|s| s.clip == AnimId::ShootFlowEnd));
    let first = trace.iter().position(|s| s.projectiles > 0).expect("the volley");
    assert!((seconds(first + 1) - 2.5).abs() < 0.1, "set-up at {}", seconds(first + 1));
    assert!((seconds(trace.len()) - 4.0).abs() < 0.15, "ended at {}", seconds(trace.len()));
    assert!(trace.iter().map(|s| s.position.z).fold(0.0, f32::max) > 450.0, "lifted 500 cm");
    let classes: Vec<&str> = b.sora.magic().projectile_views().map(|p| p.class).collect();
    assert!(!classes.is_empty() && classes.iter().all(|c| *c == "TresProjSE_Shot_C"), "{classes:?}");
    b.run(300);
    let hits = b.hits();
    assert_eq!(hits.len(), n, "one hit per projectile");
    assert!(hits.iter().all(|h| h.attack_name == "SF_KCSHOT" && h.flags == 0xFD | 34 << 8), "{:?}", hits.iter().map(|h| (h.attack_name, h.flags)).collect::<Vec<_>>());
    assert!(hits.iter().any(|h| h.target == TargetId(1)) && hits.iter().any(|h| h.target == TargetId(2)));
    b.assert_back_to_normal();
    assert_ne!(b.step(&[]).state, StateId::Shootflow);
}

/// A MAX volley of the Ragnarok code (24 locks): M_1481 -> 24 projectiles of `class` -> M_1482 -> ShootFlowEnd, about
/// 4.133 s; every hit SF_KCRAGNAROK (TOSS) with the ShootFlow command and `kind`.
fn ragnarok_volley(mut b: Bench, kind: u8, class: &str) {
    let k = b.aim_until(0, 600);
    assert_eq!(k, kind, "the MAX kind");
    assert_eq!(b.sora.aim().total, 24);
    b.fire();
    // Up to the volley (the projectiles home in and are gone before the state ends): their classes.
    let mut before = 0;
    let mut classes: Vec<&str> = Vec::new();
    while classes.len() < 24 && before < 300 {
        b.step(&[]);
        before += 1;
        classes = b.sora.magic().projectile_views().map(|p| p.class).collect();
    }
    assert!(classes.len() == 24 && classes.iter().all(|c| *c == class), "{classes:?}");
    assert!((seconds(before + 1) - 2.3333).abs() < 0.1, "set-up at {}", seconds(before + 1));
    let trace = b.trace_shoot_flow(600);
    assert!(trace.iter().any(|s| s.clip == AnimId::M1482) && trace.iter().any(|s| s.clip == AnimId::ShootFlowEnd));
    assert!((seconds(before + trace.len()) - 4.133).abs() < 0.15, "ended at {}", seconds(before + trace.len()));
    b.run(500);
    let hits = b.hits();
    assert_eq!(hits.len(), 24, "one hit per projectile");
    assert!(hits.iter().all(|h| h.attack_name == "SF_KCRAGNAROK" && h.damage_kind == ETresDamageKind::Toss && h.flags == 0xFD | (kind as u32) << 8), "{:?}", hits.iter().map(|h| (h.attack_name, h.flags)).collect::<Vec<_>>());
    b.assert_back_to_normal();
}

#[test]
fn dead_of_night_ragnarok_kind_35() {
    let b = Bench::new(level99_with("WEP_KEYBLADE_SO_18"), &[dummy(1, 900.0, 0.0)]);
    ragnarok_volley(b, 35, "TresProjSE_Ragnarok_C");
}

#[test]
fn starlight_union_ragnarok_kind_28() {
    let b = Bench::new(level99_with("WEP_KEYBLADE_SO_17"), &[dummy(1, 900.0, 0.0)]);
    ragnarok_volley(b, 28, "TresProjSL_Ragnarok_C");
}

/// Oathkeeper, base: Sunray Blast = Meteor Shower's code (M_1475, the stream) with LF_SHOOT: TresProjLF_Shoot_C, SF_LFSHOOT
/// hits with kind 40.
#[test]
fn oathkeeper_sunray_blast_kind_40() {
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_12"), &two());
    let kind = b.aim_until(8, 300);
    assert_eq!(kind, 40, "Oathkeeper's base kind (weapon type 13)");
    assert_eq!(b.sora.aim().max_locks, 32, "TresShootFlowData row 40 Param0");
    let n = b.sora.aim().total as usize;
    b.fire();
    let trace = b.trace_shoot_flow(600);
    assert!(trace.iter().any(|s| s.clip == AnimId::M1475), "Meteor Shower's clip");
    let spawned = trace.iter().map(|s| s.projectiles).max().unwrap_or(0);
    assert!(spawned >= n.min(8), "the stream: {spawned} of {n}");
    b.run(400);
    let classes: Vec<&str> = b.sora.magic().projectile_views().map(|p| p.class).collect();
    assert!(classes.iter().all(|c| *c == "TresProjLF_Shoot_C"), "{classes:?}");
    let hits = b.hits();
    assert_eq!(hits.len(), n, "one hit per lock");
    assert!(hits.iter().all(|h| h.attack_name == "SF_LFSHOOT" && h.damage_kind == ETresDamageKind::Toss && h.flags == 0xFD | 40 << 8), "{:?}", hits.iter().map(|h| (h.attack_name, h.flags)).collect::<Vec<_>>());
    b.assert_back_to_normal();
}

/// The burst family (M_1466 -> the set-up -> M_1467 .. M_1469 while shooting -> ShootFlowEnd), one projectile of `class`
/// per lock (lock by lock), every hit `row` (SIDESPIN) with the ShootFlow command and `kind`.
fn burst(key: &str, kind: u8, max_locks: i32, class: &str, row: &str) {
    let mut b = Bench::new(level99_with(key), &two());
    assert_eq!(b.aim_until(8, 300), kind, "the base kind");
    assert_eq!(b.sora.aim().max_locks, max_locks, "TresShootFlowData Param0");
    let n = b.sora.aim().total as usize;
    b.fire();
    let mut seen: Vec<&str> = Vec::new();
    let mut steps = 0;
    let mut clips = Vec::new();
    loop {
        let o = b.step(&[]);
        steps += 1;
        clips.push(o.anim.name);
        for p in b.sora.magic().projectile_views() {
            if !seen.contains(&p.class) {
                seen.push(p.class);
            }
        }
        if o.state != StateId::Shootflow || steps > 900 {
            break;
        }
    }
    assert!(steps < 900, "the state ended");
    assert!(clips.contains(&AnimId::M1466) && clips.contains(&AnimId::M1467) && clips.contains(&AnimId::ShootFlowEnd), "the burst clips");
    assert_eq!(seen, vec![class], "the projectile class");
    b.run(400);
    let hits = b.hits();
    assert_eq!(hits.len(), n, "one hit per lock");
    assert!(hits.iter().all(|h| h.attack_name == row && h.damage_kind == ETresDamageKind::Sidespin && h.flags == 0xFD | (kind as u32) << 8), "{:?}", hits.iter().map(|h| (h.attack_name, h.flags)).collect::<Vec<_>>());
    b.assert_back_to_normal();
}

#[test]
fn starlight_blades_of_the_round_kind_27() {
    burst("WEP_KEYBLADE_SO_17", 27, 24, "TresProjSL_Shot_C", "SF_SLBURST");
}

#[test]
fn oblivion_bladefury_eclipse_kind_41() {
    burst("WEP_KEYBLADE_SO_13", 41, 32, "TresProjDF_Burst_C", "SF_DFBURST");
}

/// (SHOT-C) The 1 s invincibility after Sunray Blast (kind 0x28: pawn vt+0x1E48(1.0) at the state's end -> pawn+0xE44,
/// IsInvincible 0x141435A90): a hit right after the state does nothing; one after the second takes HP.
#[test]
fn sunray_blast_leaves_one_second_of_invincibility() {
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_12"), &two());
    assert_eq!(b.aim_until(4, 300), 40);
    b.fire();
    b.trace_shoot_flow(600);
    let hit = || kh_core::states::HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), kh_core::Vec3::new(800.0, 0.0, 0.0));
    let hp = b.sora.status().hp;
    b.sora.take_hit(hit());
    b.run(2);
    assert_eq!(b.sora.status().hp, hp, "invincible just after the state");
    b.run(66);
    b.sora.take_hit(hit());
    b.run(2);
    assert!(b.sora.status().hp < hp, "the timer ran out after 1 s");
}
