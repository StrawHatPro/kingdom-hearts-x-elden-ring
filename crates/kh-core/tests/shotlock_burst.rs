//! (SHOT-B burst) Diamond Dust (Crystal Snow, `ETresShootFlowKind` 14 FZ_BURST) and Blade Storm (Wheel of Fate, 16
//! CA_BURST): KH3's update 0x1413BADB0 with the spawner 0x1413C2A70. M_1466 (0.8 s, 500 cm lift) -> the set-up -> M_1467 /
//! M_1468 / M_1469 while fewer than N are out -> ShootFlowEnd -> Finish. The first bolt 0.5 s after the set-up (tick 77 at
//! 60 Hz), then one every 4 ticks from the blade tip, homing on the locked targets LOCK BY LOCK. Traces: lock-on -> fire
//! -> finish -> back to normal.

mod shotlocks_common;

use kh_core::host::Call;
use kh_core::{AnimId, StateId, TargetId};
use shotlocks_common::{dummy, level99_with, Bench, TraceStep, DT};

/// One step: the trace plus the bolts spawned so far (their persistent body effects, named by the class).
struct Run {
    b: Bench,
    trace: Vec<TraceStep>,
    /// Seconds after Begin at which each bolt spawned.
    spawns: Vec<f32>,
    /// The homing target of each bolt, in spawn order.
    homing: Vec<TargetId>,
    n: usize,
    locks: Vec<(TargetId, i32)>,
}

fn burst(key: &str, locks: i32, want_kind: u8, max_locks: i32, class: &str) -> Run {
    let mut b = Bench::new(level99_with(key), &[dummy(1, 800.0, -150.0), dummy(2, 800.0, 150.0)]);
    let kind = b.aim_until(locks, 600);
    assert_eq!(kind, want_kind, "the base kind of the Keyblade while aiming (SelectKind)");
    assert_eq!(b.sora.aim().max_locks, max_locks, "TresShootFlowData Param0");
    let n = b.sora.aim().total as usize;
    let locks = b.sora.aim().locks.clone();
    b.fire();
    let start = b.time - DT;
    let mut trace = Vec::new();
    let mut spawns = Vec::new();
    let mut seen = b.host.calls.len();
    let mut ids: Vec<u32> = Vec::new();
    let mut homing = Vec::new();
    for _ in 0..600 {
        let o = b.step(&[]);
        for p in &b.sora.magic().projectiles {
            if !ids.contains(&p.id) {
                ids.push(p.id);
                homing.push(p.target);
            }
        }
        for c in &b.host.calls[seen..] {
            if let Call::Spawn(_, e) = c {
                if e.persistent && e.name == class {
                    spawns.push(b.time - start);
                }
            }
        }
        seen = b.host.calls.len();
        trace.push(TraceStep { t: b.time - start, state: o.state, sub: o.sub_step, clip: o.anim.name, position: o.position, projectiles: spawns.len(), hits: b.host.hits().len() });
        if o.state != StateId::Shootflow {
            break;
        }
    }
    assert_ne!(trace.last().unwrap().state, StateId::Shootflow, "the state ended");
    Run { b, trace, spawns, homing, n, locks }
}

/// The clips in the order the state played them, with the time each started (s after Begin).
fn played(trace: &[TraceStep]) -> Vec<(AnimId, f32)> {
    let mut out: Vec<(AnimId, f32)> = Vec::new();
    for s in trace.iter().filter(|s| s.state == StateId::Shootflow) {
        if out.last().map(|l| l.0) != Some(s.clip) {
            out.push((s.clip, s.t));
        }
    }
    out
}

fn clips(trace: &[TraceStep]) -> Vec<AnimId> {
    played(trace).into_iter().map(|c| c.0).collect()
}

/// Every clip ran its length (+- 1.5 ticks: the port's clip end lands one tick after KH3's at most), and the state ended
/// at the last one's end.
fn assert_clip_lengths(trace: &[TraceStep]) {
    let p = played(trace);
    let ends: Vec<f32> = p.iter().skip(1).map(|c| c.1).chain(std::iter::once(trace.last().unwrap().t)).collect();
    for ((clip, start), end) in p.iter().zip(ends) {
        let length = clip.data().length;
        assert!((end - start - length).abs() <= 1.5 * DT, "{}: {} s, KH3 {length} s", clip.data().name, end - start);
    }
}

fn assert_schedule(r: &Run) {
    assert_eq!(r.spawns.len(), r.n, "one bolt per lock");
    // tick(i) = 77 + 4 (i - 1) after Begin (spec/burst.md 3), +- 2 ticks for the notify / anim-end convention.
    for (i, t) in r.spawns.iter().enumerate() {
        let want = (77 + 4 * i) as f32 * DT;
        assert!((t - want).abs() <= 2.5 * DT, "bolt {i}: {t} s, KH3 {want} s ({:?})", r.spawns);
    }
    let gaps: Vec<i32> = r.spawns.windows(2).map(|w| ((w[1] - w[0]) / DT).round() as i32).collect();
    assert!(gaps.iter().all(|&g| g == 4), "every 4 ticks (0.066666 s): {gaps:?}");
}

fn assert_hits(r: &mut Run, row: &str, kind: u8) {
    r.b.run(400);
    let hits = r.b.hits();
    assert_eq!(hits.len(), r.n, "every bolt hits once");
    assert!(hits.iter().all(|h| h.attack_name == row && h.flags == 0xFD | (kind as u32) << 8), "{:?}", hits.iter().map(|h| (h.attack_name, h.flags)).collect::<Vec<_>>());
    // LOCK BY LOCK: the first target's locks first, then the next one's (the bolts fly the same distance).
    let first = r.locks[0];
    for (t, n) in &r.locks {
        assert_eq!(hits.iter().filter(|h| h.target == *t).count(), *n as usize, "target {t:?}: its lock count of bolts ({:?})", r.locks);
    }
    let want: Vec<TargetId> = r.locks.iter().flat_map(|&(t, n)| std::iter::repeat_n(t, n as usize)).collect();
    assert_eq!(r.homing, want, "locked[0] takes its {} bolts first, then locked[1] ({first:?})", first.1);
}

#[test]
fn diamond_dust_fires_lock_by_lock_from_the_blade_tip() {
    let mut r = burst("WEP_KEYBLADE_SO_06", 10, 14, 24, "TresProjFZ_Burst_C");
    assert!(r.n >= 10 && r.n <= 17);
    // M_1466 -> M_1467 -> M_1468 -> ShootFlowEnd (N <= 17: no M_1469).
    let played = clips(&r.trace);
    let names: Vec<&str> = played.iter().map(|c| c.data().name).collect();
    assert_eq!(names, ["M_1466", "M_1467", "M_1468", "ShootFlowEnd"], "{names:?}");
    // The set-up at M_1466's end (0.8 s): M_1467 starts there.
    let set_up = r.trace.iter().find(|s| s.clip.data().name == "M_1467").unwrap().t;
    assert!((set_up - 0.8).abs() <= 2.5 * DT, "set-up at 0.8 s: {set_up}");
    // Lifted: his centre 500 cm above the floor (the floor sweep's distance), held there (gravity off).
    let top = r.trace.iter().map(|s| s.position.z).fold(f32::MIN, f32::max);
    assert!((top - 500.0).abs() < 1.0, "lifted to 500: {top}");
    assert!(r.trace.iter().take(r.trace.len() - 1).skip(2).all(|s| (s.position.z - top).abs() < 1.0), "held at 500 until the end");
    assert_schedule(&r);
    // Finish at ShootFlowEnd's end: 0.8 + 0.7333 + 0.8667 + 0.8 = 3.2 s in KH3.
    assert_clip_lengths(&r.trace);
    assert_hits(&mut r, "SF_FZBURST", 14);
    r.b.assert_back_to_normal();
}

#[test]
fn blade_storm_plays_m_1469_from_18_locks() {
    let mut r = burst("WEP_KEYBLADE_SO_09", 20, 16, 32, "TresProjCA_Burst_C");
    assert!(r.n >= 18 && r.n <= 30);
    let names: Vec<&str> = clips(&r.trace).iter().map(|c| c.data().name).collect();
    assert_eq!(names, ["M_1466", "M_1467", "M_1468", "M_1469", "ShootFlowEnd"], "{names:?}");
    assert_schedule(&r);
    // Finish: 0.8 + 0.7333 + 0.8667 + 0.8333 + 0.8 = 4.0333 s in KH3.
    assert_clip_lengths(&r.trace);
    assert_hits(&mut r, "SF_CABURST", 16);
    r.b.assert_back_to_normal();
}

#[test]
fn a_few_locks_end_after_m_1467() {
    let mut r = burst("WEP_KEYBLADE_SO_06", 3, 14, 24, "TresProjFZ_Burst_C");
    assert!(r.n <= 4, "N = {}", r.n);
    let names: Vec<&str> = clips(&r.trace).iter().map(|c| c.data().name).collect();
    assert_eq!(names, ["M_1466", "M_1467", "ShootFlowEnd"], "{names:?}");
    assert_schedule(&r);
    // 0.8 + 0.7333 + 0.8 = 2.333 s in KH3.
    assert_clip_lengths(&r.trace);
    assert_hits(&mut r, "SF_FZBURST", 14);
    r.b.assert_back_to_normal();
}
