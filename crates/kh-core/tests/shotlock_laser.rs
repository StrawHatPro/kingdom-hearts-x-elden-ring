//! (SHOT-A laser) Spectral Rays (Ever After, `ETresShootFlowKind` 11, the MAX kind of Shimmering Drops): the trace
//! lock-on -> fire -> Mirage Staff's M_A481 (lifted 200 cm; the SF_Range window 1.6 .. 3.6 s) -> one SF_REFLECTIVELASER
//! per lock at the locked targets, the first 0.5 s after the window opened, then 0.1 s apart (7 ticks at 60 Hz), until
//! the state ends -> ShootFlowEnd -> the Keyblade back -> back to normal (magic/shotlocks/laser.rs, update 0x1413B9600, spawner 0x1413C1A90). The same code
//! runs the Customize screen's Spectral Rays (XSI's port addition) with another Keyblade.

mod shotlocks_common;

use std::collections::HashSet;

use kh_core::enums::ETresAbilityKind as A;
use kh_core::test_world::TestWorld;
use kh_core::{AnimId, PadButton, Pose, Sora, StateId, TargetId, Vec3, SORA_PARAMS};
use shotlocks_common::{dummy, level99_with, seconds, Bench, DT};

const RAY: &str = "TresProjRA_ReflectiveLaser_C";

/// What one shotlock did, step by step from its first ShootFlow step.
struct Run {
    ground: f32,
    top: f32,
    start: usize,
    t0: f32,
    spawns: Vec<(usize, Vec3)>,
    range_opened: Option<usize>,
    end_clip: usize,
    weapon_back: usize,
    aim_closed: usize,
    ended: usize,
}

/// Fire the shotlock aimed in `b` and trace it to its end.
fn trace(b: &mut Bench) -> Run {
    let ground = b.step(&[PadButton::RightShoulder]).position.z;
    b.fire();
    let mut top = f32::MIN;
    let (mut start, mut t0, mut range_opened, mut end_clip, mut weapon_back, mut aim_closed, mut ended) = (None, 0.0, None, None, None, None, None);
    let mut spawns = Vec::new();
    let mut seen: HashSet<u32> = HashSet::new();
    for n in 0..900 {
        let out = b.step(&[]);
        if start.is_none() && out.anim.name == AnimId::A481 {
            start = Some(n);
            t0 = out.anim.time;
            assert_eq!(b.sora.magic().shot_weapon_form, 10, "Mirage Staff's weapon form (vt+0x13C8(type, 10))");
            assert!(b.sora.status().damage_invalid, "invincible");
        }
        for p in b.sora.magic().projectile_views() {
            if p.class == RAY && seen.insert(p.id) {
                spawns.push((n, p.position));
            }
        }
        if range_opened.is_none() && b.hits().iter().any(|h| h.attack_name == "SF_REFLECTIVELASER_RANGE") {
            range_opened = Some(n);
        }
        if end_clip.is_none() && out.anim.name == AnimId::ShootFlowEnd {
            end_clip = Some(n);
        }
        if end_clip.is_some() && weapon_back.is_none() && b.sora.magic().shot_weapon_form == 0 {
            weapon_back = Some(n);
        }
        if end_clip.is_some() && aim_closed.is_none() && b.sora.aim().mode == 0 {
            aim_closed = Some(n);
        }
        if out.state != StateId::Shootflow {
            ended = Some(n);
            break;
        }
        top = top.max(out.position.z);
    }
    // The aim mode's release (End's slot 809) may finish after the state.
    let mut n = ended.unwrap_or(0);
    while aim_closed.is_none() && n < 2000 {
        n += 1;
        b.step(&[]);
        if b.sora.aim().mode == 0 {
            aim_closed = Some(n);
        }
    }
    Run { ground, top, start: start.expect("M_A481"), t0, spawns, range_opened, end_clip: end_clip.expect("ShootFlowEnd"), weapon_back: weapon_back.expect("the weapon back"), aim_closed: aim_closed.expect("the aim closed"), ended: ended.expect("the state ended") }
}

/// The ticks between two rays: +0xA0 = 0.1 after a spawn, counted down by dt (f32) until it is no longer > 0.
fn ray_gap() -> usize {
    let mut t = 0.1_f32;
    let mut ticks = 1;
    loop {
        t -= DT;
        if t > 0.0 {
            ticks += 1;
        } else {
            return ticks;
        }
    }
}

#[test]
fn spectral_rays_lock_on_fire_finish_back_to_normal() {
    let targets = [dummy(1, 800.0, 0.0), dummy(2, 800.0, 150.0)];
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_04"), &targets);
    b.world = TestWorld::floor();
    // LOCK-ON: Shimmering Drops (kind 10, Param0 28) while aiming, Spectral Rays (kind 11) at MAX.
    assert_eq!(b.aim_until(1, 400), 10, "Shimmering Drops while aiming");
    assert_eq!(b.aim_until(0, 3000), 11, "Spectral Rays at MAX");
    let aim = b.sora.aim();
    assert_eq!((aim.total, aim.max_locks, aim.is_max), (28, 28, true), "{aim:?}");
    let locks = aim.locks.clone();
    assert_eq!(locks.iter().map(|l| l.1).sum::<i32>(), 28);
    // FIRE.
    let r = trace(&mut b);
    let at = |n: usize| seconds(n - r.start) + r.t0;
    // LiftCheck(200): lifted 200 cm (LiftStep's swept move), flying with gravity off.
    assert!((r.top - r.ground - 200.0).abs() < 15.0, "lifted {}", r.top - r.ground);
    // The SF_Range window opens at 1.6 s (it hits both dummies at once: the 10 m sphere 5 m under him).
    let opened = r.range_opened.expect("the SF_Range window hit");
    assert!((at(opened) - 1.6).abs() <= DT + 1.0e-3, "window at {}", at(opened));
    // The rays: the first 0.5 s after the window (the TresAtk begin's +0xA0), then +0xA0 = 0.1 counted down by dt: in
    // f32 at 60 Hz 0.1 - 6 x 0.016666668 is still > 0, so 7 ticks (0.1167 s) apart. One ray per lock while the state
    // lasts: 28 locks would need 2.1 + 27 x 0.1167 = 5.25 s, the state ends at 4.3667 + 0.8 = 5.1667 s, so at 60 Hz
    // KH3's code fires 27 (the spawner simply stops with the state; at a higher frame rate all 28 fit).
    assert!((seconds(r.spawns[0].0 - opened) - 0.5).abs() <= DT + 1.0e-3, "first ray {} s after the window", seconds(r.spawns[0].0 - opened));
    let gaps: Vec<usize> = r.spawns.windows(2).map(|w| w[1].0 - w[0].0).collect();
    assert_eq!(ray_gap(), 7);
    assert!(gaps.iter().all(|&g| g == ray_gap()), "{gaps:?}");
    let due = (0..28).filter(|k| r.spawns[0].0 + k * ray_gap() <= r.ended).count();
    assert_eq!((r.spawns.len(), due), (27, 27), "one ray per lock until the state ends");
    // The last rays come during ShootFlowEnd (M_A481 ends at 4.3667 s): the spawner runs in every sub.
    assert!(r.spawns.last().unwrap().0 > r.end_clip, "the last ray during ShootFlowEnd");
    // Each at its target's lock-on point, the locked targets in list order, each its own lock count.
    let order: Vec<TargetId> = locks.iter().flat_map(|&(target, count)| std::iter::repeat_n(target, count as usize)).collect();
    for (i, s) in r.spawns.iter().enumerate() {
        let centre = targets.iter().find(|t| t.id == order[i]).unwrap().center;
        assert!((s.1 - centre).length() < 1.0, "ray {i} at {:?}, target {:?} at {centre:?}", s.1, order[i]);
    }
    // The clips: M_A481 4.3667 s, ShootFlowEnd 0.8 s; the Keyblade and the aim's release at its TresEnableChange (0.3 s).
    assert!((at(r.end_clip) - 4.3667).abs() < 0.04, "ShootFlowEnd at {}", at(r.end_clip));
    assert!((seconds(r.weapon_back - r.end_clip) - 0.3).abs() < 0.04, "weapon back {} s into ShootFlowEnd", seconds(r.weapon_back - r.end_clip));
    // The aim mode closes with the state (End's release handshake, slot 809), not before: the spawner needs the list.
    assert!(r.aim_closed >= r.ended, "aim closed at {} (state ended {})", r.aim_closed, r.ended);
    assert!((at(r.ended) - 5.1667).abs() < 0.06, "ended at {}", at(r.ended));
    // The hits: one SF_REFLECTIVELASER per ray (on the ray's target), flags 0xFD | 11 << 8; the SF_Range window
    // (interval 1 s) hits each dummy at 1.6 s and 2.6 s.
    b.run(90);
    let hits = b.hits();
    let rays: Vec<_> = hits.iter().filter(|h| h.attack_name == "SF_REFLECTIVELASER").collect();
    assert_eq!(rays.len(), r.spawns.len(), "{:?}", hits.iter().map(|h| h.attack_name).collect::<Vec<_>>());
    assert!(rays.iter().all(|h| h.flags == 0xFD | 11 << 8), "{:?}", rays.iter().map(|h| h.flags).collect::<Vec<_>>());
    for &(target, _) in &locks {
        let fired = order[..r.spawns.len()].iter().filter(|t| **t == target).count();
        assert_eq!(rays.iter().filter(|h| h.target == target).count(), fired, "{target:?}");
    }
    let range: Vec<_> = hits.iter().filter(|h| h.attack_name == "SF_REFLECTIVELASER_RANGE").collect();
    for id in [TargetId(1), TargetId(2)] {
        assert_eq!(range.iter().filter(|h| h.target == id).count(), 2, "SF_Range on {id:?}");
    }
    // BACK TO NORMAL.
    b.assert_back_to_normal();
    assert_eq!(b.sora.magic().shot_weapon_form, 0);
}

/// The Customize screen's Spectral Rays (abilities/xs/spectral.rs, a port addition) runs the same KH3 code with the
/// Kingdom Key in hand: Mirage Staff, M_A481, one ray per lock (SF_REFLECTIVELASER from weapon form 10's set w_so070).
#[test]
fn the_customize_screen_s_spectral_rays_runs_kh3_s_kind_11() {
    let mut status = level99_with("WEP_KEYBLADE_SO_00");
    let owned = status.abilities.owned(A::ShotlockSpectralRays).max(1);
    status.abilities.set(A::ShotlockSpectralRays, owned, 1);
    let mut b = Bench::new(status, &[dummy(1, 800.0, 0.0), dummy(2, 800.0, 150.0)]);
    b.sora.command_mut().shotlock = 11;
    assert_eq!(b.aim_until(4, 400), 11, "the chosen shotlock while aiming");
    let total = b.sora.aim().total;
    let r = trace(&mut b);
    assert_eq!(r.spawns.len() as i32, total, "one ray per lock");
    assert!((r.top - r.ground - 200.0).abs() < 15.0, "lifted {}", r.top - r.ground);
    b.run(60);
    assert_eq!(b.hits().iter().filter(|h| h.attack_name == "SF_REFLECTIVELASER").count() as i32, total);
    b.assert_back_to_normal();
    assert_eq!(b.sora.magic().shot_weapon_form, 0);
}

/// A state dropped in the middle (a teleport) gives the Keyblade back (End 0x1413ABA70: +0xB0 != 0).
#[test]
fn a_dropped_spectral_rays_gives_the_weapon_back() {
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_04"), &[dummy(1, 800.0, 0.0)]);
    assert_eq!(b.aim_until(0, 3000), 11);
    b.fire();
    b.run(30);
    assert_eq!(b.sora.magic().shot_weapon_form, 10);
    b.sora.teleport(Pose { position: Vec3::new(0.0, 2000.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 });
    b.run(3);
    assert_eq!((b.sora.aim().mode, b.sora.magic().shot_weapon_form), (0, 0));
}
