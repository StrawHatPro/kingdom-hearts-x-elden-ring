//! (SHOT-A) Diving Barrage (Shooting Star, `ETresShootFlowKind` 6 DW_BARRAGE): KH3's update 0x1413B6320
//! (magic/shotlocks/barrage.rs). SelectKind picks it instead of Cluster Cannonade (7) at MAX when 0x1413ECC80 holds (at
//! most two locked targets, the sweep to 1000 cm above the first one clear). Trace: lock-on -> MAX -> fire -> M_2481
//! (the rise to 1000 cm + his half height above the target, Double Arrowguns on) -> M_2482 (the dive, a pair of
//! TresProjSS_Barrage per TresShot while spawned < N) -> M_2483 (the weapon back at its TresEnableChange) -> Finish ->
//! back to normal.

mod shotlocks_common;

use kh_core::{AnimId, StateId, TargetId, SORA_PARAMS};
use shotlocks_common::{dummy, level99_with, seconds, Bench};

/// (SHOT-FIX) 0x1413ECC80's sky sweep (pawn vt+0x2750 = 0x141431630, "Warp") blocks on object types WorldStatic,
/// WorldDynamic and TresGimmick only (response params: every channel Ignore, then 0 / 1 / 0x12 Block): an enemy standing
/// in the sweep's way (a pawn) does not block it, as in the lock's line of sight. A tall enemy right in the path, not
/// locked: still Diving Barrage.
#[test]
fn an_enemy_in_the_way_does_not_block_the_sky() {
    let mut pillar = dummy(2, 400.0, 0.0);
    (pillar.center.z, pillar.half_height, pillar.radius) = (700.0, 700.0, 250.0);
    pillar.flags = kh_core::TargetFlags(kh_core::TargetFlags::ALIVE | kh_core::TargetFlags::HITTABLE | kh_core::TargetFlags::ENEMY);
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_02"), &[dummy(1, 800.0, 0.0), pillar]);
    assert_eq!(b.aim_until(0, 1200), 6, "Diving Barrage with an enemy in the sweep's way ({:?})", b.sora.aim());
    assert!(b.sora.aim().locks.iter().all(|l| l.0 == TargetId(1)), "the pillar is not locked");
}

#[test]
fn diving_barrage_lock_on_fire_finish_back_to_normal() {
    let target = dummy(1, 800.0, 0.0);
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_02"), &[target]);
    // Lock-on: Shooting Star's base kind is Meteor Shower (5); at MAX (Param0 18) with one locked target and a clear sky
    // above it, 0x1413ECC80 gives the alternative MAX kind 6.
    let kind = b.aim_until(0, 1200);
    assert_eq!(kind, 6, "Diving Barrage at MAX ({:?})", b.sora.aim());
    let n = b.sora.aim().total as usize;
    assert_eq!(n, 18, "TresShootFlowData row 5 Param0");
    // Fire.
    b.fire();
    let start = b.time;
    assert_eq!(b.sora.magic().shot_weapon_form, 2, "Double Arrowguns on (vt+0x13C8(type, 2))");
    let trace = b.trace_shoot_flow(600);
    let at = |clip: AnimId| trace.iter().position(|s| s.clip == clip).unwrap_or_else(|| panic!("no {clip:?}"));
    let (rise, dive, land) = (at(AnimId::A2481), at(AnimId::A2482), at(AnimId::A2483));
    // The KH3 timeline: M_2481 1.2 s, M_2482 2.1667 s, M_2483 0.4333 s, then Finish.
    assert!(rise <= 1, "M_2481 at once ({rise})");
    assert!((seconds(dive - rise) - 1.2).abs() < 0.05, "M_2482 {} s after M_2481", seconds(dive - rise));
    assert!((seconds(land - dive) - 2.1667).abs() < 0.05, "M_2483 {} s after M_2482", seconds(land - dive));
    assert!((seconds(trace.len() - land) - 0.4333).abs() < 0.05, "ended {} s after M_2483", seconds(trace.len() - land));
    assert!(trace[..land].iter().all(|s| s.state == StateId::Shootflow));
    // The rise: to 1000 cm + his half height above the target's centre (sub 4: 5 x the offset per second).
    let half = SORA_PARAMS.capsule_half_height;
    let centre_z = |s: &shotlocks_common::TraceStep| s.position.z + SORA_PARAMS.capsule_center_z;
    let top = trace[dive - 1].clone();
    let want = target.center.z + 1000.0 + half;
    assert!((centre_z(&top) - want).abs() < 30.0, "risen to {} (want {want})", centre_z(&top));
    assert!((top.position.truncate() - target.position.truncate()).length() < 60.0, "over the target: {:?}", top.position);
    // The dive: the point 800 cm lower, z velocity 0.5 x the offset: 800 x (1 - e^(-2.1667 / 2)) ~ 530 cm by M_2483.
    let low = trace[land - 1].clone();
    let fell = centre_z(&top) - centre_z(&low);
    assert!((fell - 800.0 * (1.0 - (-2.1667f32 / 2.0).exp())).abs() < 40.0, "fell {fell} cm in the dive");
    assert!(trace[dive..land].windows(2).all(|w| w[1].position.z <= w[0].position.z + 0.01), "only down in the dive");
    // The projectiles: none before M_2482; a pair per TresShot (weapon 0, then 1) until N.
    assert!(trace[..dive].iter().all(|s| s.projectiles == 0), "no shot in M_2481");
    let first = trace.iter().position(|s| s.projectiles > 0).expect("the barrage");
    assert!(first >= dive && seconds(first - dive) < 0.05, "the first pair with M_2482's first TresShot ({})", seconds(first - dive));
    assert_eq!(trace[first].projectiles, 2, "a pair");
    // spawned so far = alive + hit (a TresProjSS_Barrage dies on its hit only: m_MainLife 0, no distance cap)
    let counts: Vec<usize> = trace.iter().map(|s| s.projectiles + s.hits).collect();
    let full = counts.iter().position(|&c| c >= n).unwrap_or_else(|| panic!("N spawned {counts:?}"));
    assert_eq!(counts.last(), Some(&n), "N in all: {counts:?}");
    // The 9th TresShot of M_2482 (1.0666 s) completes 18.
    assert!((seconds(full - dive) - 1.0666).abs() < 0.05, "18 spawned {} s into M_2482", seconds(full - dive));
    assert!(counts.iter().all(|c| c.is_multiple_of(2)), "pairs only: {counts:?}");
    // The weapon form: on until M_2483's TresEnableChange (0.2995 s), then Begin's back.
    assert_eq!(b.sora.magic().shot_weapon_form, 0, "the Keyblade back");
    b.run(300);
    let hits = b.hits();
    assert_eq!(hits.len(), n, "one hit per projectile");
    assert!(hits.iter().all(|h| h.attack_name == "SF_BARRAGE" && h.flags == 0xFD | 6 << 8 && h.target == TargetId(1)), "{:?}", hits.iter().map(|h| (h.attack_name, h.flags)).collect::<Vec<_>>());
    let classes: Vec<&str> = b.sora.magic().projectile_views().map(|p| p.class).collect();
    assert!(classes.iter().all(|c| *c == "TresProjSS_Barrage_C"), "{classes:?}");
    // Back to normal.
    assert!(b.time - start > 3.7, "about 3.8 s of shotlock");
    b.assert_back_to_normal();
    assert_eq!(b.sora.magic().shot_weapon_form, 0);
}

/// The weapon comes back at M_2483's TresEnableChange (0.2995 s), before the clip's end (0.4333 s).
#[test]
fn diving_barrage_gives_the_weapon_back_at_the_landing_clip_s_enable_change() {
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_02"), &[dummy(1, 800.0, 0.0)]);
    assert_eq!(b.aim_until(0, 1200), 6);
    b.fire();
    let mut land = None;
    let mut back = None;
    let mut pair_seen = false;
    for k in 0..600 {
        let o = b.step(&[]);
        let views: Vec<_> = b.sora.magic().projectile_views().collect();
        if !pair_seen && !views.is_empty() {
            // The first pair: weapon 0's then weapon 1's ATK_01 (the two arrowguns, one in each hand), each flying out
            // from his centre through its muzzle.
            pair_seen = true;
            assert_eq!(views.len(), 2);
            let centre = o.position + kh_core::Vec3::Z * SORA_PARAMS.capsule_center_z;
            let apart = views[0].position - views[1].position;
            assert!(apart.y.abs() > 50.0 && apart.length() < 200.0, "the two hands: {:?} / {:?}", views[0].position, views[1].position);
            for v in &views {
                assert!((v.position - centre).length() < 150.0, "at his gun: {:?} (centre {centre:?})", v.position);
                assert!(v.forward.dot((v.position - centre).normalize()) > 0.9, "outward: {:?}", v.forward);
            }
        }
        if land.is_none() && o.anim.name == AnimId::A2483 {
            land = Some(k);
        }
        if land.is_some() && back.is_none() && b.sora.magic().shot_weapon_form == 0 {
            back = Some(k);
        }
        if land.is_none() {
            assert_eq!(b.sora.magic().shot_weapon_form, 2, "the arrowguns until M_2483 ({k})");
        }
        if o.state != StateId::Shootflow {
            break;
        }
    }
    assert!(pair_seen);
    let (land, back) = (land.expect("M_2483"), back.expect("the weapon back"));
    assert!((seconds(back - land) - 0.2995).abs() < 0.04, "weapon back {} s into M_2483", seconds(back - land));
}

/// A state dropped in the middle (a teleport) gives the weapon back (End 0x1413ABA70).
#[test]
fn a_dropped_diving_barrage_gives_the_weapon_back() {
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_02"), &[dummy(1, 800.0, 0.0)]);
    assert_eq!(b.aim_until(0, 1200), 6);
    b.fire();
    b.run(30);
    assert_eq!(b.sora.magic().shot_weapon_form, 2);
    b.sora.teleport(kh_core::Pose { position: kh_core::Vec3::new(0.0, 2000.0, kh_core::Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 });
    b.run(3);
    assert_eq!((b.sora.aim().mode, b.sora.magic().shot_weapon_form), (0, 0));
}
