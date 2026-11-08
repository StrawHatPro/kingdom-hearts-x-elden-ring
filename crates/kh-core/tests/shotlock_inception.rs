//! (SHOT-C) Stellar Inception (kind 42, the MAX shotlock of Oathkeeper and of Oblivion): the trace lock-on -> MAX ->
//! fire -> Double Form's weapons and M_N481 -> the R_Finish window -> the Timing at 2.333 s -> 0.3 s later one bolt a
//! tick, each locked target as many as its locks -> ShootFlowEnd -> back to normal (magic/shotlocks/inception.rs, update
//! 0x1413B7F10, spawner 0x1413C0850).
//!
//! The base kinds of the two Keyblades (40 Sunray Blast: SHOT-A's meteor family; 41 Bladefury Eclipse: SHOT-B's burst)
//! may still be pending; SelectKind's base kind then falls back to the Kingdom Key's and its MAX to Ragnarok. The test
//! sets KH3's own debug override of SelectKind (ctrl+0xA34, `AimMode::debug_kind`) to 42 so the MAX kind is reached as
//! KH3 reaches it once the base kind runs (at Param0 = 32 locks).

mod shotlocks_common;

use kh_core::enums::ETresDamageKind;
use kh_core::{AnimId, StateId, TargetId, Vec3};
use shotlocks_common::{dummy, level99_with, Bench, DT};

const KIND: u8 = 42;

fn inception(key: &str) {
    let mut b = Bench::new(level99_with(key), &[dummy(1, 800.0, 0.0), dummy(2, 800.0, 150.0)]);
    b.sora.magic_mut().aim.debug_kind = KIND;
    assert_eq!(b.aim_until(0, 400), KIND, "{key}: MAX gives Stellar Inception");
    let aim = b.sora.aim();
    assert_eq!((aim.total, aim.max_locks, aim.is_max), (32, 32, true));
    let per_target: Vec<(TargetId, i32)> = aim.locks.clone();
    assert_eq!(per_target.len(), 2);
    b.fire();
    let start = b.time - DT;
    let hp_before = b.sora.status().hp;
    // The trace, step by step: weapon form, clip, sub-step, new bolts.
    let (mut form_on, mut form_off, mut timing, mut first_bolt, mut last_bolt, mut end_clip) = (None, None, None, None, None, None);
    let mut bolts: Vec<(f32, Vec3, Vec3)> = Vec::new();
    let mut seen: Vec<u32> = Vec::new();
    let mut max_per_step = 0;
    let mut ended = None;
    let mut aim_open_at_end_clip = None; // (SHOT-FIX)
    for k in 0..600 {
        let o = b.step(&[]);
        let t = b.time - start;
        if o.anim.name == AnimId::ShootFlowEnd && aim_open_at_end_clip.is_none() && b.sora.magic().shot_weapon_form == 0 {
            aim_open_at_end_clip = Some(b.sora.aim().mode != 0);
        }
        if k % 15 == 0 {
            b.sora.take_hit(kh_core::states::HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), Vec3::new(800.0, 0.0, 0.0)));
        }
        let wf = b.sora.magic().shot_weapon_form;
        if wf == 0x18 && form_on.is_none() {
            form_on = Some(t);
        }
        if wf == 0 && form_on.is_some() && form_off.is_none() {
            form_off = Some(t);
        }
        if o.sub_step == 5 && timing.is_none() {
            timing = Some(t);
        }
        if o.anim.name == AnimId::ShootFlowEnd && end_clip.is_none() {
            end_clip = Some(t);
        }
        if o.state == StateId::Shootflow && o.sub_step != 0 && t < 5.0 {
            assert_eq!(o.anim.name, AnimId::N481, "M_N481 at {t}");
        }
        let mut new = 0;
        for v in b.sora.magic().projectile_views() {
            if v.class == "TresProjTF_LightInception_C" && !seen.contains(&v.id) {
                seen.push(v.id);
                new += 1;
                bolts.push((t, v.position - o.position, v.forward));
                first_bolt.get_or_insert(t);
                last_bolt = Some(t);
            }
        }
        max_per_step = max_per_step.max(new);
        if o.state != StateId::Shootflow {
            ended = Some(t);
            break;
        }
    }
    let close = |a: Option<f32>, b: f32, tol: f32| a.is_some_and(|a| (a - b).abs() <= tol);
    assert!(close(form_on, DT, 2.0 * DT), "{key}: Double Form's weapons at once ({form_on:?})");
    assert!(close(timing, 2.3333, 2.0 * DT), "{key}: the Timing at 2.333 s ({timing:?})");
    assert!(close(first_bolt, 2.3333 + 0.3, 3.0 * DT), "{key}: the first bolt 0.3 s later ({first_bolt:?})");
    assert_eq!((seen.len(), max_per_step), (32, 1), "{key}: one bolt a tick, one per lock");
    assert!(close(last_bolt, first_bolt.unwrap() + 31.0 * DT, 2.0 * DT), "{key}: 32 ticks of bolts ({last_bolt:?})");
    // Bolt i heads at his yaw + 360 / 32 x i, 30 degrees up, from 300 cm to the left of that heading.
    for (i, (_, offset, forward)) in bolts.iter().enumerate() {
        let yaw = (bolts[0].2.y.atan2(bolts[0].2.x).to_degrees() + 360.0 / 32.0 * i as f32).to_radians();
        let side = Vec3::new(yaw.sin(), -yaw.cos(), 0.0) * 300.0;
        assert!((offset.truncate() - side.truncate()).length() < 25.0, "{key}: bolt {i} at {offset:?}, expected {side:?}");
        assert!((forward.z - 0.5).abs() < 0.02, "{key}: bolt {i} pitched 30 up ({forward:?})");
    }
    assert!(close(end_clip, 5.3333, 2.0 * DT), "{key}: ShootFlowEnd at the clip's end ({end_clip:?})");
    assert!(close(form_off, 5.3333 + 0.3, 3.0 * DT), "{key}: the Keyblade back at TresEnableChange ({form_off:?})");
    assert!(close(ended, 5.3333 + 0.8, 3.0 * DT), "{key}: the state ends after ShootFlowEnd ({ended:?})");
    // (SHOT-FIX) the 0x14140E500 call at ShootFlowEnd is WorldSlow off, not the release: the aim mode (and its locked list)
    // is still open after the weapon's return; End's slot 809 closes it.
    assert_eq!(aim_open_at_end_clip, Some(true), "{key}: the aim mode stays open until End");
    assert_eq!(b.sora.status().hp, hp_before, "{key}: invincible throughout");
    // The hits: the R_Finish sphere once on each dummy (SF_LIGHTINCEPTION, TOSS) and every bolt on its target.
    b.run(400);
    let hits = b.hits();
    let finish: Vec<_> = hits.iter().filter(|h| h.attack_name == "SF_LIGHTINCEPTION").collect();
    assert_eq!(finish.len(), 2, "{key}: R_Finish hits each dummy once");
    // (the generic window path: the row names the shotlock, HitRecord::flags carry no command there, as Infinity Circle's)
    assert!(finish.iter().all(|h| h.damage_kind == ETresDamageKind::Toss && h.damage > 0.0), "{:?}", finish.iter().map(|h| (h.damage_kind, h.flags, h.damage)).collect::<Vec<_>>());
    let bombs: Vec<_> = hits.iter().filter(|h| h.attack_name == "SF_LIGHTINCEPTION_BOMB").collect();
    assert_eq!(bombs.len(), 32, "{key}: every bolt hits");
    // Each entry gets as many bolts as its locks (16 / 16 here); a bolt dies on the first body it touches (no pierce), so
    // a bolt homing on one dummy may hit the other on the way: both are hit, the sum is the locks.
    assert_eq!(per_target.iter().map(|p| p.1).sum::<i32>(), 32);
    for (target, locks) in &per_target {
        let n = bombs.iter().filter(|h| h.target == *target).count() as i32;
        assert!(n >= locks / 2, "{key}: {target:?} got {n} bolts for {locks} locks");
    }
    b.assert_back_to_normal();
    assert_eq!(b.sora.magic().shot_weapon_form, 0);
}

#[test]
fn stellar_inception_oathkeeper() {
    inception("WEP_KEYBLADE_SO_12");
}

#[test]
fn stellar_inception_oblivion() {
    inception("WEP_KEYBLADE_SO_13");
}

/// Without the debug override the Keyblades' MAX kind is 42 in SelectKind's table; the base kind decides whether it is
/// selected now (a pending base family keeps the Kingdom Key's pair).
#[test]
fn select_kind_gives_42_at_max() {
    use kh_core::magic::shotlocks::{base_kind, max_kind, ready};
    for (weapon, base) in [(13, 40), (14, 41)] {
        assert_eq!((base_kind(weapon), max_kind(weapon, false), max_kind(weapon, true)), (base, 42, 42));
    }
    assert!(ready(KIND));
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_12"), &[dummy(1, 800.0, 0.0)]);
    let kind = b.aim_until(0, 400);
    assert_eq!(kind, if ready(40) { KIND } else { 2 }, "MAX kind with the base kind {}", if ready(40) { "ported" } else { "pending" });
}
