//! (MIMIC-BOSS hotfix 3, MB3-FOGRISE; THE USER 2026-11-06: "Have the boss itself activate as soon as you enter the fog gate
//! opposed to having to walk up to it") The rise at the fog gate: Elden Ring's own event, as traced (Nokron 12022860, the
//! Stray 30202810), from the user's log numbers. On Nokron's first visit the event waits for the walk-up into its
//! first-encounter box; the wake (SpEffectParam 3051, a state info 6 on the pool, one of the event's own conditions) starts
//! it at the fog pass instead. The Stray and Nokron's return visits start at the fog by themselves.

use crate::mimicboss::{er_rise_starts, in_msb_box, pool_wake_due, rise_route, PoolWake, RiseFacts, RiseInputs, RiseRoute, WakeAsk, MIMIC_BOSSES, POOL_WAKE_RETRY_S, POOL_WAKE_SPEFFECT, POOL_WAKE_STATE_INFO, POOL_WAKE_STATE_INFOS, POOL_WAKE_TRIES};

/// An MSB box region: its place, Y rotation (degrees), width / depth / height (`trace/msb_m12_02.js`, `msb_m30_20.js`).
type Region = ([f32; 3], f32, [f32; 3]);

/// Nokron: 12022850 "前座ボス開始領域" (the fog's slab), 12022851 "前座ボス初戦開始領域" (the first encounter's box).
const NOKRON_FOG_REGION: Region = ([965.491, -617.627, 1163.893], 29.895, [2.0, 11.6, 2.0]);
const NOKRON_FIRST_REGION: Region = ([1020.755, -618.381, 1129.591], -60.0, [52.0, 65.0, 15.0]);
/// Nokron's fog AEG099_002_9000 (entity 12021850).
const NOKRON_FOG: [f32; 3] = [964.576, -617.557, 1164.48];
/// The Stray: 30202800 "ボス戦開始領域" (the fog's own region) and its fog AEG099_001_9000 (entity 30201800).
const STRAY_FOG_REGION: Region = ([-89.134, 1278.265, -243.392], 0.0, [5.0, 3.0, 2.0]);
const STRAY_FOG: [f32; 3] = [-88.897, 1278.725, -241.683];

fn inside(r: Region, p: [f32; 3]) -> bool {
    in_msb_box(r.0, r.1, r.2, p)
}

/// The user's run (er/logs/kh-er.prev.log) in its physics frame after the origin shift at 406 s, to the MSB's: the body's
/// first place, at its pool's dummy (411.642 s: (17.33, -0.79, -17.01)), is the pool's MSB place (1001.327, _, 1142.989), and
/// its nest read at 329.335 s ((45.65, -25.31, 14.02)) is its own MSB place (1029.647, -641.311, 1174.018).
fn msb_of(er: [f32; 3]) -> [f32; 3] {
    [er[0] + 984.0, er[1] - 616.0, er[2] + 1160.0]
}

/// That run: the fog flag 12022855 set at 398.726 s (the fog's walk, anim 60060, that second); the leader's body each second
/// after the shift; the boss's body enabled at 411.642 s.
const FOG_AT: f64 = 398.726;
const TRACK: [(f64, [f32; 3]); 3] = [(406.699, [0.796, -1.510, -7.427]), (407.698, [6.656, -1.510, -10.875]), (408.715, [10.150, -1.510, -12.931])];
const BODY_AT: f64 = 411.642;
/// The leader at the fog pass, on the floor in the fog's slab.
const FOG_PASS: [f32; 3] = [965.491, -617.51, 1163.893];

fn nokron() -> &'static crate::mimicboss::MimicBossRow {
    &MIMIC_BOSSES[0]
}

fn stray() -> &'static crate::mimicboss::MimicBossRow {
    &MIMIC_BOSSES[1]
}

/// The first frame ER's event starts the rise along the track (the fog pass at FOG_AT, then the leader's places), with or
/// without the wake on the pool at the fog pass.
fn er_start(wake: bool) -> Option<f64> {
    let row = nokron();
    let at_fog = RiseInputs { first_done: false, fog_flag: true, in_fog_region: inside(NOKRON_FOG_REGION, FOG_PASS), in_first_region: inside(NOKRON_FIRST_REGION, FOG_PASS), pool_damaged: false, pool_state_info: wake.then_some(POOL_WAKE_STATE_INFO) };
    if er_rise_starts(row, &at_fog) {
        return Some(FOG_AT);
    }
    TRACK.iter().find(|(_, er)| er_rise_starts(row, &RiseInputs { in_first_region: inside(NOKRON_FIRST_REGION, msb_of(*er)), ..at_fog })).map(|(t, _)| *t)
}

#[test]
fn mimicrise_the_regions_fit_the_maps_and_the_users_log() {
    // the fog objects stand just outside their own slab, the slab runs inward (Nokron 2 m, the Stray 3 m)
    assert!(!inside(NOKRON_FOG_REGION, NOKRON_FOG));
    assert!(inside(NOKRON_FOG_REGION, FOG_PASS));
    assert!(!inside(STRAY_FOG_REGION, STRAY_FOG));
    assert!(inside(STRAY_FOG_REGION, [-89.0, 1278.3, -243.4]));
    // the pool is in the first encounter's box, the fog pass is far outside it
    assert!(inside(NOKRON_FIRST_REGION, nokron().pool_at));
    assert!(!inside(NOKRON_FIRST_REGION, FOG_PASS));
    // the user's run: outside at 406.7 and 407.7 s, inside at 408.7 s; the body was enabled 2.9 s later (the pool's own rise)
    let ins: Vec<bool> = TRACK.iter().map(|(_, er)| inside(NOKRON_FIRST_REGION, msb_of(*er))).collect();
    assert_eq!(ins, vec![false, false, true]);
    let rise = BODY_AT - TRACK[2].0;
    assert!((2.5..4.0).contains(&rise), "{rise}");
}

#[test]
fn mimicrise_nokron_first_visit_rises_at_the_fog_instead_of_after_the_walk_up() {
    let row = nokron();
    assert_eq!(rise_route(row, false), RiseRoute::FirstVisit);
    // Elden Ring alone: the rise waits for the walk-up into the box, 9-10 s after the fog
    let alone = er_start(false).expect("the walk-up starts it");
    assert!(alone - FOG_AT > 8.9, "{}", alone - FOG_AT);
    // the wake at the fog frame: the event's own condition holds at once
    let mut w = PoolWake::default();
    let at_fog = RiseFacts { defeated: false, fog_flag: true, first_done: false, body_enabled: false };
    assert_eq!(w.step(row, &at_fog, true, FOG_AT), WakeAsk::Apply(1));
    assert_eq!(er_start(true), Some(FOG_AT));
    // the next frame the event has set its first-encounter flag: the wake is over, nothing more is given
    let next = FOG_AT + 1.0 / 60.0;
    assert!(matches!(w.step(row, &RiseFacts { first_done: true, ..at_fog }, true, next), WakeAsk::Took { after } if after < 0.02));
    assert_eq!(w.step(row, &RiseFacts { first_done: true, ..at_fog }, true, next + 1.0), WakeAsk::Idle);
}

#[test]
fn mimicrise_the_users_three_runs_rise_at_the_fog_not_9_to_13_s_later() {
    // ER's 8 s (12022860: WaitFixedTimeSeconds 8 from the rise's start to the boss bar, which asks the music): the first run's
    // music ask at 173.25 s puts its start at 165.25 s, and the pool's own rise (anim 20010 to its SpEffect 16307) at 3.31 s
    let pool_rise = 168.56 - (173.25 - 8.0);
    assert!((3.0..3.6).contains(&pool_rise), "{pool_rise}");
    // (the fog flag, the body in the world): the first run, the prev log's run above, the latest run
    for (fog, body) in [(155.96, 168.56), (FOG_AT, BODY_AT), (95.789, 105.133)] {
        let walk_up = body - fog - pool_rise;
        assert!(walk_up > 5.0, "{fog}: {walk_up}");
        // with the wake at the fog frame: the rise starts there and the body is in the world after the pool's own rise
        let mut w = PoolWake::default();
        assert_eq!(w.step(nokron(), &RiseFacts { fog_flag: true, ..RiseFacts::default() }, true, fog), WakeAsk::Apply(1));
        let body_now = fog + pool_rise;
        assert!(body - body_now > 5.0 && body - body_now < 10.0, "{fog}: {}", body - body_now);
    }
}

#[test]
fn mimicrise_the_stray_and_nokrons_return_visit_start_at_the_fog_by_themselves() {
    // Nokron after its first encounter: the fog flag and the slab
    let n = nokron();
    assert_eq!(rise_route(n, true), RiseRoute::FogPass);
    assert!(er_rise_starts(n, &RiseInputs { first_done: true, fog_flag: true, in_fog_region: inside(NOKRON_FOG_REGION, FOG_PASS), ..RiseInputs::default() }));
    // the Stray: no first-encounter branch; its fog pass is in its own region
    let s = stray();
    assert_eq!(rise_route(s, false), RiseRoute::FogPass);
    assert!(er_rise_starts(s, &RiseInputs { fog_flag: true, in_fog_region: inside(STRAY_FOG_REGION, [-89.0, 1278.3, -243.4]), ..RiseInputs::default() }));
    // nothing is given to either pool
    for (row, first) in [(n, true), (s, false)] {
        let f = RiseFacts { fog_flag: true, first_done: first, ..RiseFacts::default() };
        assert!(!pool_wake_due(row, &f));
        assert_eq!(PoolWake::default().step(row, &f, true, 1.0), WakeAsk::Idle);
    }
}

#[test]
fn mimicrise_the_wake_only_when_it_is_needed_and_never_without_end() {
    let row = nokron();
    let due = RiseFacts { fog_flag: true, ..RiseFacts::default() };
    assert!(pool_wake_due(row, &due));
    // the fog not passed, the boss beaten, the body already up: no wake
    for f in [RiseFacts { fog_flag: false, ..due }, RiseFacts { defeated: true, ..due }, RiseFacts { body_enabled: true, ..due }] {
        assert!(!pool_wake_due(row, &f));
        assert_eq!(PoolWake::default().step(row, &f, true, 1.0), WakeAsk::Idle);
    }
    // no pool in the world: nothing given
    assert_eq!(PoolWake::default().step(row, &due, false, 1.0), WakeAsk::NoPool);
    // the event never taking it: asked every 0.5 s, at most 10 times, then Elden Ring's own walk-up remains
    let mut w = PoolWake::default();
    let mut t = 0.0;
    let mut asks = 0;
    let mut gave_up = false;
    for _ in 0..2000 {
        match w.step(row, &due, true, t) {
            WakeAsk::Apply(_) => asks += 1,
            WakeAsk::GaveUp => gave_up = true,
            _ => {}
        }
        t += 1.0 / 60.0;
    }
    assert_eq!(asks, POOL_WAKE_TRIES);
    assert!(gave_up);
    assert!(t > f64::from(POOL_WAKE_TRIES) * POOL_WAKE_RETRY_S);
    // a reload clears the fog flag: the next pass wakes again
    assert_eq!(w.step(row, &RiseFacts { fog_flag: false, ..due }, true, t), WakeAsk::Idle);
    assert_eq!(w.step(row, &due, true, t + 1.0), WakeAsk::Apply(1));
}

#[test]
fn mimicrise_the_wake_is_one_of_the_events_own_conditions() {
    // SpEffectParam 3051's stateInfo is 6 (bleed), one of the five the event reads on the pool
    assert_eq!(POOL_WAKE_SPEFFECT, 3051);
    assert!(POOL_WAKE_STATE_INFOS.contains(&POOL_WAKE_STATE_INFO));
    let row = nokron();
    assert_eq!(row.pool_entity, 12_020_860);
    let base = RiseInputs { fog_flag: true, ..RiseInputs::default() };
    assert!(!er_rise_starts(row, &base));
    assert!(er_rise_starts(row, &RiseInputs { pool_state_info: Some(6), ..base }));
    assert!(!er_rise_starts(row, &RiseInputs { pool_state_info: Some(1), ..base }));
    assert!(er_rise_starts(row, &RiseInputs { pool_damaged: true, ..base }));
}

// ------------------------------------------------ (MB3-FOGRISE 2) the Stray, and the fog pass as Elden Ring checks it

/// (THE USER 2026-11-07: "the other mimic tear boss must start as soon as you enter the fog gate as well") The shared region
/// table is the MSBs' (the same boxes the tests above read).
#[test]
fn mimicrise_the_region_table_is_the_msbs() {
    use crate::mimicboss::rise_region;
    assert_eq!(rise_region(12_022_850), Some(NOKRON_FOG_REGION));
    assert_eq!(rise_region(12_022_851), Some(NOKRON_FIRST_REGION));
    assert_eq!(rise_region(30_202_800), Some(STRAY_FOG_REGION));
    assert_eq!(rise_region(nokron().fog_region), Some(NOKRON_FOG_REGION));
    assert_eq!(rise_region(stray().fog_region), Some(STRAY_FOG_REGION));
    assert_eq!(rise_region(1), None);
}

/// No Stray log exists (er/logs has no pass of its fog): from its events and MSB. Its fog event (9005800, X8 = 30202800) sets
/// the fog flag 30202805 only after the walk, with the leader in 30202800 (3 s at most, else it starts over without the
/// flag); 30202810 waits on that flag AND the leader in 30202800: true on its next check, so the pool rises at the fog pass,
/// the body ~3.3 s later, well before the late watch.
#[test]
fn mimicrise_the_strays_rise_starts_on_the_fog_flags_own_frame() {
    use crate::mimicboss::{in_fog_region, rise_late, POOL_RISE_S};
    let s = stray();
    // where the walk leaves the leader: past the fog object (just outside the box), inside the 5 x 3 m box
    let walk_end = [-89.0, 1278.3, -243.4];
    assert!(!in_fog_region(s, STRAY_FOG));
    assert!(in_fog_region(s, walk_end));
    // the fog event's condition = the leader in the region; the same frame's facts satisfy the rise's event
    let fog_frame = RiseInputs { fog_flag: true, in_fog_region: in_fog_region(s, walk_end), ..RiseInputs::default() };
    assert!(er_rise_starts(s, &fog_frame));
    // a KH leader running on at 8 m/s moves 0.13 m in the next frame: still in the 3 m box from anywhere but its last 0.13 m
    let next = [walk_end[0], walk_end[1], walk_end[2] - 8.0 / 60.0];
    assert!(er_rise_starts(s, &RiseInputs { in_fog_region: in_fog_region(s, next), ..fog_frame }));
    // nothing to wake; the body due at the pool's own rise, before the late watch
    assert!(!pool_wake_due(s, &RiseFacts { fog_flag: true, ..RiseFacts::default() }));
    assert!(!rise_late(false, POOL_RISE_S, false));
    assert!(rise_late(false, POOL_RISE_S + 3.0, false));
}

/// The late watch on the user's pre-wake Nokron run (fog flag 398.726 s, body 411.642 s): it would have been told at 404.7 s;
/// with the body in the world or the boss beaten never.
#[test]
fn mimicrise_the_late_watch_tells_a_waiting_rise_only() {
    use crate::mimicboss::{rise_late, RISE_LATE_S};
    assert!(!rise_late(false, 5.9, false));
    assert!(rise_late(false, 6.1, false));
    assert!(!rise_late(false, 30.0, true));
    assert!(!rise_late(true, 30.0, false));
    let told = FOG_AT + RISE_LATE_S;
    assert!(told < BODY_AT && (told - 404.726).abs() < 1e-6);
}

/// The leader's MSB place by the pool's physics place (the user's Nokron run's translation): his place at 408.715 s is in the
/// first-encounter box, the fog pass in the fog's slab.
#[test]
fn mimicrise_the_leaders_msb_place_by_the_pool() {
    use crate::mimicboss::{in_fog_region, msb_place};
    let n = nokron();
    let pool_er = [n.pool_at[0] - 984.0, n.pool_at[1] + 616.0, n.pool_at[2] - 1160.0];
    let (_, leader_er) = TRACK[2];
    let p = msb_place(pool_er, n.pool_at, leader_er);
    let q = msb_of(leader_er);
    assert!((0..3).all(|k| (p[k] - q[k]).abs() < 1e-3), "{p:?} {q:?}");
    assert!(inside(NOKRON_FIRST_REGION, p));
    assert!(in_fog_region(n, FOG_PASS));
    assert!(!in_fog_region(n, p));
}
