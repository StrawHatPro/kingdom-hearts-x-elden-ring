//! (SHOT-A pellet) Atomic Deluge (Hero's Origin, `ETresShootFlowKind` 4, the MAX kind of Drain Shock): the trace
//! lock-on -> fire -> M_B481 (Counter Shield's transform unit w_so031_wep; the SphereL window) -> one SF_SHIELDSHOT per
//! lock at the locked targets ((6 - n) / 60 s apart) -> ShootFlowEnd with the LV3 shield -> back to normal
//! (magic/shotlocks/pellet.rs, update 0x1413B9910, spawner 0x1413C1E40).
//!
//! While Drain Shock (kind 3, SHOT-A's drain family) is still pending, SelectKind's base kind falls back to the Kingdom
//! Key's and its MAX to Ragnarok; the bench then sets KH3's own debug override of SelectKind (ctrl+0xA34,
//! `AimMode::debug_kind`) to 3, the base kind KH3 gives weapon type 2, so MAX gives kind 4 as it does in KH3.

mod shotlocks_common;

use std::collections::HashSet;

use kh_core::test_world::TestWorld;
use kh_core::{AnimId, Pose, Sora, StateId, TargetId, Vec3, SORA_PARAMS};
use shotlocks_common::{dummy, level99_with, seconds, Bench, DT};

/// The bench holding Hero's Origin, with Drain Shock as the base kind (see the module text).
fn hero_s_origin(targets: &[kh_core::TargetInfo]) -> Bench {
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_01"), targets);
    if !kh_core::magic::shotlocks::ready(3) {
        b.sora.magic_mut().aim.debug_kind = 3;
    }
    b
}

fn units_end_with(units: &[&str], blueprint: &str) -> bool {
    units.iter().any(|u| u.ends_with(blueprint))
}

#[test]
fn atomic_deluge_lock_on_fire_finish_back_to_normal() {
    let targets = [dummy(1, 800.0, 0.0), dummy(2, 800.0, 150.0)];
    let mut b = hero_s_origin(&targets);
    b.world = TestWorld::floor();
    // LOCK-ON: Drain Shock (kind 3, Param0 16) while aiming, Atomic Deluge (kind 4) at MAX.
    assert_eq!(b.aim_until(1, 200), 3, "Drain Shock while aiming");
    assert_eq!(b.aim_until(0, 900), 4, "Atomic Deluge at MAX");
    let aim = b.sora.aim();
    assert_eq!((aim.total, aim.max_locks, aim.is_max), (16, 16, true), "{aim:?}");
    let locks = aim.locks.clone();
    assert_eq!(locks.iter().map(|l| l.1).sum::<i32>(), 16);
    // FIRE.
    b.fire();
    let mut z = (f32::MAX, f32::MIN);
    let (mut start, mut end_clip, mut weapon_back, mut ended) = (None, None, None, None);
    let mut spawns: Vec<(usize, u32, Vec3)> = Vec::new();
    let mut seen: HashSet<u32> = HashSet::new();
    let mut saw_transform = false;
    let mut saw_lv3 = false;
    let mut blow_opened = None;
    let mut t0 = 0.0;
    for n in 0..900 {
        let out = b.step(&[]);
        if start.is_none() && out.anim.name == AnimId::B481 {
            start = Some(n);
            t0 = out.anim.time;
            assert_eq!(b.sora.magic().shot_weapon_form, 11, "Counter Shield's weapon form");
            assert!(b.sora.status().damage_invalid, "invincible");
        }
        if blow_opened.is_none() && b.hits().iter().any(|h| h.attack_name == "SF_SHIELDSHOT_BLOW") {
            blow_opened = Some(n);
        }
        let m = b.sora.magic();
        for p in m.projectile_views() {
            if p.class == "TresProjHE_ShieldShot_C" && seen.insert(p.id) {
                spawns.push((n, p.id, p.position));
            }
        }
        if out.anim.name == AnimId::B481 {
            saw_transform |= units_end_with(m.shot_weapon_units, "w_so031_wep");
        }
        if end_clip.is_none() && out.anim.name == AnimId::ShootFlowEnd {
            end_clip = Some(n);
        }
        if end_clip.is_some() && m.shot_weapon_form == 11 {
            saw_lv3 |= units_end_with(m.shot_weapon_units, "w_so034_wep") && units_end_with(m.shot_weapon_units, "w_so037_wep");
        }
        if weapon_back.is_none() && end_clip.is_some() && m.shot_weapon_form == 0 {
            weapon_back = Some(n);
            assert!(m.shot_weapon_units.is_empty());
        }
        if out.state != StateId::Shootflow {
            ended = Some(n);
            break;
        }
        z = (z.0.min(out.position.z), z.1.max(out.position.z));
    }
    let start = start.expect("M_B481");
    let (end_clip, weapon_back, ended) = (end_clip.expect("ShootFlowEnd"), weapon_back.expect("weapon back"), ended.expect("ended"));
    // M_B481's time at step n.
    let at = |n: usize| seconds(n - start) + t0;
    assert!(saw_transform, "the transform unit w_so031_wep during M_B481");
    // No lift: only M_B481's root motion (271 cm up at most, flying with gravity off).
    assert!((200.0..300.0).contains(&(z.1 - z.0)), "rise {}", z.1 - z.0);
    assert!(saw_lv3, "the LV3 shield (w_so034_wep + w_so037_wep) during ShootFlowEnd");
    // The window (2.1 s) hits at once; 16 shots: the first 0.3 s after it opened (the TresAtk begin's +0xA0), then
    // +0xA0 = (6 - n) * 0.016666668 counted down by dt: in f32 at 60 Hz that is 6, 5, 4, 2, 1 ticks, then one a tick.
    let blow_opened = blow_opened.expect("the SphereL blow");
    assert!((at(blow_opened) - 2.1).abs() <= DT + 1.0e-3, "window at {}", at(blow_opened));
    assert_eq!(spawns.len(), 16, "one shot per lock");
    assert!((seconds(spawns[0].0 - blow_opened) - 0.3).abs() <= DT + 1.0e-3, "first shot {} s after the window", seconds(spawns[0].0 - blow_opened));
    let gaps: Vec<usize> = spawns.windows(2).map(|w| w[1].0 - w[0].0).collect();
    let want: Vec<usize> = (1..16)
        .map(|k: i32| {
            let mut t = ((6 - k) as f32 * (1.0 / 60.0_f32)).max(0.0);
            let mut ticks = 1;
            while t > 0.0 {
                t -= DT;
                if t > 0.0 {
                    ticks += 1;
                }
            }
            ticks
        })
        .collect();
    assert_eq!(&want[..6], &[6, 5, 4, 2, 1, 1]);
    assert_eq!(gaps, want);
    // Each at its target's lock-on point, the locked targets in list order, each its own lock count.
    let mut i = 0;
    for &(target, count) in &locks {
        let centre = targets.iter().find(|t| t.id == target).unwrap().center;
        for _ in 0..count {
            assert!((spawns[i].2 - centre).length() < 1.0, "shot {i} at {:?}, target {target:?} at {centre:?}", spawns[i].2);
            i += 1;
        }
    }
    // The clips: M_B481 4.7 s, ShootFlowEnd 0.8 s; the weapon of Begin comes back at its TresEnableChange (0.3 s).
    assert!((at(end_clip) - 4.7).abs() < 0.04, "ShootFlowEnd at {}", at(end_clip));
    assert!((seconds(weapon_back - end_clip) - 0.3).abs() < 0.04, "weapon back {} s into ShootFlowEnd", seconds(weapon_back - end_clip));
    assert!((at(ended) - 5.5).abs() < 0.06, "ended at {}", at(ended));
    // The hits: one SF_SHIELDSHOT per shot (as many per target as its locks) and M_B481's SphereL blow once per target.
    b.run(90);
    let hits = b.hits();
    let shots: Vec<_> = hits.iter().filter(|h| h.attack_name == "SF_SHIELDSHOT").collect();
    assert_eq!(shots.len(), 16, "{:?}", hits.iter().map(|h| h.attack_name).collect::<Vec<_>>());
    assert!(shots.iter().all(|h| h.flags == 0xFD | 4 << 8), "{:?}", shots.iter().map(|h| h.flags).collect::<Vec<_>>());
    for &(target, count) in &locks {
        assert_eq!(shots.iter().filter(|h| h.target == target).count() as i32, count, "{target:?}");
    }
    let blows: Vec<_> = hits.iter().filter(|h| h.attack_name == "SF_SHIELDSHOT_BLOW").collect();
    assert_eq!(blows.len(), 2, "the 10 m sphere hits both targets once");
    assert!(blows.iter().any(|h| h.target == TargetId(1)) && blows.iter().any(|h| h.target == TargetId(2)));
    // BACK TO NORMAL.
    b.assert_back_to_normal();
    assert_eq!(b.sora.magic().shot_weapon_form, 0);
    assert!(b.sora.magic().shot_weapon_units.is_empty());
}

/// A state dropped in the middle (a teleport) gives the weapon back (End 0x1413ABA70).
#[test]
fn a_dropped_atomic_deluge_gives_the_weapon_back() {
    let mut b = hero_s_origin(&[dummy(1, 800.0, 0.0)]);
    assert_eq!(b.aim_until(0, 900), 4);
    b.fire();
    b.run(30);
    assert_eq!(b.sora.magic().shot_weapon_form, 11);
    b.sora.teleport(Pose { position: Vec3::new(0.0, 2000.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 });
    b.run(3);
    assert_eq!((b.sora.aim().mode, b.sora.magic().shot_weapon_form), (0, 0));
    assert!(b.sora.magic().shot_weapon_units.is_empty());
}
