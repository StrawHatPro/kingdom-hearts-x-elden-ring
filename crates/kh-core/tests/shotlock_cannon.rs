//! (SHOT-A) Cluster Cannonade (Shooting Star, `ETresShootFlowKind` 7 BZ_CANNON): KH3's update 0x1413B6960 and its cluster
//! call-back 0x1413B3BF0 (magic/shotlocks/cannon.rs). SelectKind picks it at MAX when 0x1413ECC80 is false (here: three
//! locked targets). Trace: lock-on -> MAX -> fire -> ShootFlowStart (the Magic Launcher on, the 200 cm lift) -> M_5482
//! (its TresShot: ONE TresProjSS_Cannon_C from the launcher's muzzle along his facing) -> the shell's 0.5 s life ends ->
//! N TresProjSS_Cannon2_C in a 45 degree cone at its place + its TresProjSS_CannonEx_C blast -> ShootFlowEnd (the weapon
//! back at its TresEnableChange) -> Finish -> back to normal.

mod shotlocks_common;

use kh_core::host::Call;
use kh_core::{AnimId, StateId, TargetId, TargetInfo, Vec3, SORA_PARAMS};
use shotlocks_common::{dummy, level99_with, seconds, Bench};

fn three() -> [TargetInfo; 3] {
    [dummy(1, 800.0, 0.0), dummy(2, 800.0, 150.0), dummy(3, 800.0, -150.0)]
}

/// The bench at Cluster Cannonade's MAX: Shooting Star, three dummies (0x1413ECC80 is false with more than 2 locked).
fn at_max() -> Bench {
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_02"), &three());
    let kind = b.aim_until(0, 1200);
    assert_eq!(kind, 7, "Cluster Cannonade at MAX ({:?})", b.sora.aim());
    assert_eq!(b.sora.aim().locked_targets, 3);
    b
}

fn count(b: &Bench, class: &str) -> usize {
    b.sora.magic().projectile_views().filter(|p| p.class == class).count()
}

#[test]
fn cluster_cannonade_lock_on_fire_finish_back_to_normal() {
    let mut b = at_max();
    let n = b.sora.aim().total as usize;
    assert_eq!(n, 18, "TresShootFlowData row 5 Param0");
    let locked: Vec<TargetId> = b.sora.aim().locks.iter().map(|l| l.0).collect();
    let floor_z = kh_core::Sora::origin_height_on_floor(&SORA_PARAMS);
    b.fire();
    let start = b.time;
    assert_eq!(b.sora.magic().shot_weapon_form, 5, "the Magic Launcher on (vt+0x13C8(type, 5))");
    // Step by hand (the trace bench's steps plus the projectiles of each step).
    let (mut clips, mut shell, mut cluster): (Vec<(usize, AnimId)>, Option<usize>, Option<usize>) = (Vec::new(), None, None);
    let mut z_max = floor_z;
    let mut k = 0;
    loop {
        let o = b.step(&[]);
        if clips.last().is_none_or(|c| c.1 != o.anim.name) {
            clips.push((k, o.anim.name));
        }
        z_max = z_max.max(o.position.z);
        let (shells, twos, exs) = (count(&b, "TresProjSS_Cannon_C"), count(&b, "TresProjSS_Cannon2_C"), count(&b, "TresProjSS_CannonEx_C"));
        if shell.is_none() && shells > 0 {
            shell = Some(k);
            // ONE shell, from the launcher's ATK_01 (FK of M_5482 at 0.552 s: about (169, 8, 156) in his frame), along
            // his facing, homing target locked[0] (its class flies straight: m_bEnableHoming false).
            assert_eq!(shells, 1);
            let p = b.sora.magic().projectiles.iter().find(|p| p.def.class_name == "TresProjSS_Cannon_C").unwrap();
            let (sin, cos) = o.yaw.to_radians().sin_cos();
            let local = p.position - o.position;
            let local = Vec3::new(local.x * cos + local.y * sin, -local.x * sin + local.y * cos, local.z);
            assert!((local - Vec3::new(169.4, 7.9, 155.6)).length() < 25.0, "the muzzle: {local:?} (one tick of 1000 cm/s at most ahead)");
            assert!(p.forward.dot(Vec3::new(cos, sin, 0.0)) > 0.999, "along his facing: {:?}", p.forward);
            assert_eq!(p.target, locked[0]);
        }
        if cluster.is_none() && twos > 0 {
            cluster = Some(k);
            // The cluster: N bomblets at the shell's last place, a 45 degree cone around its flight line (i = 0 at the
            // top), homing on locked[(1 + i) mod 3] (the counter goes on from the shell's 1); the CannonEx blast there.
            assert_eq!((shells, twos, exs), (0, n, 1), "the shell gone, N Cannon2, one CannonEx");
            let ps: Vec<_> = b.sora.magic().projectiles.iter().filter(|p| p.def.class_name == "TresProjSS_Cannon2_C").collect();
            let ex = b.sora.magic().projectiles.iter().find(|p| p.def.class_name == "TresProjSS_CannonEx_C").unwrap();
            let (sin, cos) = o.yaw.to_radians().sin_cos();
            let ahead = Vec3::new(cos, sin, 0.0);
            for (i, p) in ps.iter().enumerate() {
                assert!((p.position - ex.position).length() < 0.01, "at the shell's place");
                assert!((p.forward.dot(ahead) - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-3, "45 degrees off the line: {i} {:?}", p.forward);
                let phi = (360.0 / n as f32 * i as f32).to_radians();
                let want = ahead * std::f32::consts::FRAC_1_SQRT_2 + (Vec3::new(-sin, cos, 0.0) * phi.sin() + Vec3::Z * phi.cos()) * std::f32::consts::FRAC_1_SQRT_2;
                assert!((p.forward - want).length() < 1e-3, "{i}: {:?} (want {want:?})", p.forward);
                assert_eq!(p.target, locked[(1 + i) % locked.len()], "{i}");
            }
            assert!(ps[0].forward.z > 0.7, "i = 0 at the top");
            let travelled = (ex.position - o.position).truncate().length();
            assert!(travelled > 500.0 && travelled < 750.0, "about 5 m ahead of the muzzle: {travelled}");
        }
        k += 1;
        if o.state != StateId::Shootflow {
            break;
        }
        assert!(k < 600, "the state did not end");
    }
    // The KH3 timeline: ShootFlowStart 0.6667 s, M_5482 1.7 s, ShootFlowEnd 0.8 s, then Finish.
    let at = |clip: AnimId| clips.iter().find(|c| c.1 == clip).map(|c| c.0).unwrap_or_else(|| panic!("no {clip:?}: {clips:?}"));
    let (s, f, e) = (at(AnimId::ShootFlowStart), at(AnimId::A5482), at(AnimId::ShootFlowEnd));
    assert!(s <= 1, "ShootFlowStart at once ({s})");
    assert!((seconds(f - s) - 0.6667).abs() < 0.05, "M_5482 {} s after ShootFlowStart", seconds(f - s));
    assert!((seconds(e - f) - 1.7).abs() < 0.05, "ShootFlowEnd {} s after M_5482", seconds(e - f));
    assert!((seconds(k - e) - 0.8).abs() < 0.05, "ended {} s after ShootFlowEnd", seconds(k - e));
    // LiftCheck(200): from the floor 200 cm up (one swept move), then held (Flying, no gravity).
    assert!((z_max - floor_z - 200.0).abs() < 5.0, "lifted {} cm", z_max - floor_z);
    // The shell at M_5482's TresShot (0.552 s); its 0.5 s life (m_bExplodeOnTimeOut) ends 30 / 31 ticks later.
    let (shell, cluster) = (shell.expect("the shell"), cluster.expect("the cluster"));
    assert!((seconds(shell - f) - 0.552).abs() < 0.04, "the shell {} s into M_5482", seconds(shell - f));
    assert!((seconds(cluster - shell) - 0.5).abs() < 0.04, "the cluster {} s after the shell", seconds(cluster - shell));
    // N + 1 + 1 spawned in all: the shell, its blast, N bomblets.
    let spawned = |class: &str| b.host.calls.iter().filter(|c| matches!(c, Call::Spawn(_, e) if e.persistent && e.name == class)).count();
    assert_eq!((spawned("TresProjSS_Cannon_C"), spawned("TresProjSS_CannonEx_C"), spawned("TresProjSS_Cannon2_C")), (1, 1, n));
    // The weapon form: on until ShootFlowEnd's TresEnableChange (0.3 s), then Begin's back.
    assert_eq!(b.sora.magic().shot_weapon_form, 0, "the Keyblade back");
    b.run(600);
    // Every hit is SF_CANNON (BLOW) of the ShootFlow command and kind 7, on the locked dummies.
    let hits = b.hits();
    assert!(hits.iter().all(|h| h.attack_name == "SF_CANNON" && h.flags == 0xFD | 7 << 8 && locked.contains(&h.target)), "{:?}", hits.iter().map(|h| (h.attack_name, h.flags, h.target)).collect::<Vec<_>>());
    // The blast (r 100 + 800 /s, up to 350) reaches all three dummies about 1.3 m behind it, once each; each bomblet hits
    // at most once (it dies on its hit; the lower half of the cone meets the floor before its homing starts at 0.3 s).
    for t in &locked {
        assert!(hits.iter().any(|h| h.target == *t), "{t:?} hit");
    }
    assert!(hits.len() > 3 && hits.len() <= n + 3, "{} hits", hits.len());
    assert_eq!(b.sora.magic().projectile_views().count(), 0, "every bomblet gone");
    // Back to normal.
    assert!(b.time - start > 3.0, "about 3.2 s of shotlock");
    b.assert_back_to_normal();
    assert_eq!(b.sora.magic().shot_weapon_form, 0);
}

/// The weapon comes back at ShootFlowEnd's TresEnableChange (0.3 s), before the clip's end (0.8 s).
#[test]
fn cluster_cannonade_gives_the_weapon_back_at_the_end_clip_s_enable_change() {
    let mut b = at_max();
    b.fire();
    let (mut end, mut back) = (None, None);
    for k in 0..600 {
        let o = b.step(&[]);
        if end.is_none() && o.anim.name == AnimId::ShootFlowEnd {
            end = Some(k);
        }
        if end.is_some() && back.is_none() && b.sora.magic().shot_weapon_form == 0 {
            back = Some(k);
        }
        if end.is_none() {
            assert_eq!(b.sora.magic().shot_weapon_form, 5, "the launcher until ShootFlowEnd ({k})");
        }
        if o.state != StateId::Shootflow {
            break;
        }
    }
    let (end, back) = (end.expect("ShootFlowEnd"), back.expect("the weapon back"));
    assert!((seconds(back - end) - 0.3).abs() < 0.04, "weapon back {} s into ShootFlowEnd", seconds(back - end));
}

/// A state dropped after the shot (a teleport) gives the weapon back (End 0x1413ABA70) and spawns no cluster: the
/// call-back runs only while the state is the current one (the shell still explodes into its blast).
#[test]
fn a_dropped_cluster_cannonade_gives_the_weapon_back_and_spawns_no_cluster() {
    let mut b = at_max();
    b.fire();
    for _ in 0..200 {
        b.step(&[]);
        if count(&b, "TresProjSS_Cannon_C") > 0 {
            break;
        }
    }
    assert_eq!(count(&b, "TresProjSS_Cannon_C"), 1, "the shell is out");
    assert_eq!(b.sora.magic().shot_weapon_form, 5);
    b.sora.teleport(kh_core::Pose { position: Vec3::new(0.0, 2000.0, kh_core::Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 });
    b.run(60);
    assert_eq!((b.sora.aim().mode, b.sora.magic().shot_weapon_form), (0, 0));
    let spawned = |class: &str| b.host.calls.iter().filter(|c| matches!(c, Call::Spawn(_, e) if e.persistent && e.name == class)).count();
    assert_eq!((spawned("TresProjSS_CannonEx_C"), spawned("TresProjSS_Cannon2_C")), (1, 0), "the blast, no cluster");
}
