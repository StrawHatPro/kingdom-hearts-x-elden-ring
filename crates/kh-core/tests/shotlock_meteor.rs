//! (SHOT-A meteor) Meteor Shower (Shooting Star, `ETresShootFlowKind` 5 SF_SHOOTINGSTAR): KH3's update 0x1413BA090 with
//! the stream spawner 0x1413C24D0. One clip M_1475 (2.9 s); its TresShot at 1.2 s starts the stream: ONE projectile
//! (w_so020_ProjSet SF_SHOOTINGSTAR = TresProjSS_Shoot_C, a TresProj_Ragnarok: the helix) per spawner call, every 4 / 2 / 1
//! ticks for N < 8 / < 15 / more, homing on locked[i mod lockedTargets], row SF_SHOOTINGSTAR (SIDESPIN). Within 100 cm of
//! the floor Sora is lifted to 100 cm and the state finishes at M_1475's end (sub 4 runs the Finish branch); from higher
//! up ShootFlowEnd plays first. Traces: lock-on -> fire -> finish -> back to normal.

mod shotlocks_common;

use kh_core::test_world::TestWorld;
use kh_core::{AnimId, PadButton, StateId, TargetId, Vec3};
use shotlocks_common::{dummy, level99_with, seconds, Bench, TraceStep, DT};

/// The steps (indices into the trace) on which the projectile count grew, and by how much.
fn spawns(trace: &[TraceStep]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut before = 0;
    for (i, s) in trace.iter().enumerate() {
        if s.projectiles > before {
            out.push((i, s.projectiles - before));
        }
        before = s.projectiles;
    }
    out
}

/// Aims `locks` locks with Shooting Star on two dummies, fires, and traces the state; returns (bench, trace, N, the time
/// the state began, his height before).
fn meteor(locks: i32) -> (Bench, Vec<TraceStep>, usize, f32, f32) {
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_02"), &[dummy(1, 800.0, 0.0), dummy(2, 800.0, 150.0)]);
    // Lock-on: Shooting Star's base kind is 5 (SelectKind: weapon type 3), MAX at Param0 18 locks.
    let kind = b.aim_until(locks, 400);
    assert_eq!(kind, 5, "Meteor Shower while aiming");
    assert_eq!(b.sora.aim().max_locks, 18, "TresShootFlowData row 5 Param0");
    let n = b.sora.aim().total as usize;
    assert!(n >= locks as usize && n < 18, "N = {n}");
    let z = b.step(&[PadButton::RightShoulder]).position.z;
    b.fire();
    let start = b.time - DT;
    // The trace (as Bench::trace_shoot_flow) with the classes of the projectiles in flight.
    let mut trace = Vec::new();
    for _ in 0..600 {
        let o = b.step(&[]);
        for p in b.sora.magic().projectile_views() {
            assert_eq!(p.class, "TresProjSS_Shoot_C", "Shooting Star's SF_SHOOTINGSTAR");
        }
        trace.push(TraceStep { t: b.time, state: o.state, sub: o.sub_step, clip: o.anim.name, position: o.position, projectiles: b.sora.magic().projectile_views().count(), hits: b.host.hits().len() });
        if o.state != StateId::Shootflow {
            break;
        }
    }
    assert_ne!(trace.last().map(|s| s.state), Some(StateId::Shootflow), "the state ends");
    (b, trace, n, start, z)
}

/// The stream: N projectiles, the first at M_1475's TresShot (1.2 s), one per spawn every `every` ticks.
fn assert_stream(trace: &[TraceStep], n: usize, start: f32, every: usize) {
    let sp = spawns(trace);
    assert!(sp.iter().all(|&(_, k)| k == 1), "one projectile a call: {sp:?}");
    assert_eq!(sp.len(), n, "N projectiles: {sp:?}");
    let first = trace[sp[0].0].t - start;
    assert!((first - 1.2).abs() <= 2.5 * DT, "the stream starts at the TresShot (1.2 s): {first}");
    let gaps: Vec<usize> = sp.windows(2).map(|w| w[1].0 - w[0].0).collect();
    assert!(gaps.iter().all(|&g| g == every), "every {every} ticks: {gaps:?}");
}

/// The hits: N, each SF_SHOOTINGSTAR with the ShootFlow command and kind 5, on both dummies.
fn assert_hits(b: &mut Bench, n: usize) {
    b.run(480);
    let hits = b.hits();
    assert_eq!(hits.len(), n, "one hit per projectile");
    assert!(hits.iter().all(|h| h.attack_name == "SF_SHOOTINGSTAR" && h.flags == 0xFD | 5 << 8), "{:?}", hits.iter().map(|h| (h.attack_name, h.flags)).collect::<Vec<_>>());
    assert!(hits.iter().any(|h| h.target == TargetId(1)) && hits.iter().any(|h| h.target == TargetId(2)));
}

#[test]
fn meteor_shower_on_the_floor_lifts_and_finishes_at_m_1475_s_end() {
    let (mut b, trace, n, start, floor) = meteor(5);
    assert!(n < 8);
    // M_1475 only: lifted (sub 1 -> 3 -> 4), the Finish branch at its end without ShootFlowEnd: 2.9 s.
    assert!(trace.iter().all(|s| s.clip == AnimId::M1475 || s.state != StateId::Shootflow), "M_1475 only");
    assert!(!trace.iter().any(|s| s.clip == AnimId::ShootFlowEnd));
    assert!(trace.iter().any(|s| s.sub == 4), "after the lift: sub 4");
    let end = trace.last().unwrap().t - start;
    assert!((end - 2.9).abs() <= 2.5 * DT, "finished at M_1475's end: {end}");
    // LiftCheck(100): his origin (0.15 cm above the floor) goes to 100 cm above the floor point on the second tick, and
    // stays there (gravity off) until the state ends.
    let top = trace.iter().map(|s| s.position.z).fold(f32::MIN, f32::max);
    assert!(floor < 1.0 && (top - 100.0).abs() < 0.5, "lifted to 100 cm above the floor: {floor} -> {top}");
    let held: Vec<f32> = trace.iter().take(trace.len() - 1).map(|s| s.position.z).collect();
    assert!(held.iter().all(|z| (z - top).abs() < 0.5), "held there: {held:?}");
    // The stream every 4 ticks (N < 8).
    assert_stream(&trace, n, start, 4);
    assert_hits(&mut b, n);
    b.assert_back_to_normal();
}

#[test]
fn meteor_shower_streams_every_2_ticks_from_8_locks_and_every_tick_from_15() {
    for (locks, every) in [(10, 2), (15, 1)] {
        let (mut b, trace, n, start, _) = meteor(locks);
        assert!(n >= 8 && (every == 2) == (n < 15), "N = {n}");
        assert_stream(&trace, n, start, every);
        assert_hits(&mut b, n);
        b.assert_back_to_normal();
    }
}

#[test]
fn meteor_shower_high_in_the_air_plays_shoot_flow_end() {
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_02"), &[dummy(1, 800.0, 0.0), dummy(2, 800.0, 150.0)]);
    assert_eq!(b.aim_until(3, 400), 5);
    // The floor drops 20 m away under him while he aims: no floor within 100 + 300 cm, airborne: no lift (sub 1).
    let mut world = TestWorld::floor().with_plane(Vec3::Z, -2000.0);
    world.planes.remove(0);
    b.world = world;
    b.hold(&[PadButton::RightShoulder], 10);
    let n = b.sora.aim().total as usize;
    assert!((3..8).contains(&n) && b.sora.aim().kind == 5, "still aiming in the air: N = {n}");
    b.fire();
    let start = b.time - DT;
    let trace = b.trace_shoot_flow(600);
    let subs: Vec<u8> = trace.iter().filter(|s| s.state == StateId::Shootflow).map(|s| s.sub).collect();
    assert!(subs.iter().all(|s| matches!(s, 1 | 2)) && subs.contains(&2), "no lift: sub 1, then 2: {subs:?}");
    let ends: Vec<f32> = trace.iter().filter(|s| s.clip == AnimId::ShootFlowEnd).map(|s| s.t - start).collect();
    assert!(!ends.is_empty() && (ends[0] - 2.9).abs() <= 2.5 * DT, "ShootFlowEnd at M_1475's end: {:?}", ends.first());
    let end = trace.last().unwrap().t - start;
    assert!((end - 3.7).abs() <= 2.5 * DT, "finished at ShootFlowEnd's end (2.9 + 0.8 s): {end}");
    // Held in the air (gravity off) until then.
    let z = trace[0].position.z;
    assert!(trace.iter().take(trace.len() - 1).all(|s| (s.position.z - z).abs() < 1.0), "no fall while the state runs");
    assert_stream(&trace, n, start, 4);
    assert!(seconds(trace.len()) > 3.5);
}
