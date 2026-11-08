//! (SHOT-B vacuum) Frozen Crescents (Crystal Snow at MAX, `ETresShootFlowKind` 15 FZ_VACUUMWAVE) and Flag Rampage (Wheel
//! of Fate at MAX, 17 CA_VACUUMWAVE): KH3's update 0x1413BBEE0 with the spawner 0x1413C2E70. The Keyblade turns into its
//! form weapon for the shotlock (Blizzard Blades 7 / Storm Flag 0xF), Sora floats 200 cm up and dances one 5.4333 s clip
//! (M_7481 / M_F481); each TresShot queues LocationIndex hits and the spawner drops one stationary sphere ON a locked target
//! every 0.1 s (7 ticks), lock by lock; ShootFlowEnd; the weapon form back at its TresEnableChange (0.3 s). Traces: lock-on
//! -> fire -> finish -> back to normal.

mod shotlocks_common;

use kh_core::host::Call;
use kh_core::{AnimId, StateId, TargetId};
use shotlocks_common::{dummy, level99_with, Bench, DT};

struct Run {
    b: Bench,
    /// (s after Begin) of every sphere.
    spawns: Vec<f32>,
    /// (s after Begin) per step, the clip, the weapon form, his height, the state, (SHOT-FIX) the aim mode.
    steps: Vec<(f32, AnimId, u8, f32, StateId, u8)>,
    locks: Vec<(TargetId, i32)>,
    n: usize,
    before_z: f32,
}

fn vacuum(key: &str, want_kind: u8, class: &str, dummies: &[kh_core::TargetInfo]) -> Run {
    let mut b = Bench::new(level99_with(key), dummies);
    // MAX: the base kind's Param0 locks -> the MAX kind (SelectKind 0x141411B80).
    let kind = b.aim_until(0, 900);
    assert_eq!(kind, want_kind, "the MAX kind");
    let n = b.sora.aim().total as usize;
    let locks = b.sora.aim().locks.clone();
    let before_z = b.step(&[kh_core::PadButton::RightShoulder]).position.z;
    b.fire();
    let start = b.time - DT;
    let mut spawns = Vec::new();
    let mut steps = Vec::new();
    let mut seen = b.host.calls.len();
    for _ in 0..900 {
        let o = b.step(&[]);
        for c in &b.host.calls[seen..] {
            if let Call::Spawn(_, e) = c {
                if e.persistent && e.name == class {
                    spawns.push(b.time - start);
                }
            }
        }
        seen = b.host.calls.len();
        steps.push((b.time - start, o.anim.name, b.sora.magic().shot_weapon_form, o.position.z, o.state, b.sora.aim().mode));
        if o.state != StateId::Shootflow {
            break;
        }
    }
    assert_ne!(steps.last().unwrap().4, StateId::Shootflow, "the state ended");
    Run { b, spawns, steps, locks, n, before_z }
}

/// The spawn times of KH3 at 60 Hz (spec/vacuum.md 3, the f32 schedule of `scripts/sim.js`): queue from the TresShots, one
/// sphere per 7 ticks.
fn kh3_schedule(times: &[f32], counts: &[f32], n: usize) -> Vec<f32> {
    let mut out = Vec::new();
    let (mut queue, mut timer, mut t) = (0, 0.0f32, 0.0f32);
    let mut next = 0;
    while out.len() < n && t < 6.0 {
        t += DT;
        while next < times.len() && times[next] <= t + 1.0e-4 {
            queue += counts[next] as i32;
            next += 1;
        }
        if timer > 0.0 {
            timer -= DT;
            if timer > 0.0 {
                continue;
            }
            timer = 0.0;
        }
        if queue >= 1 {
            out.push(t);
            queue -= 1;
            timer = 0.1;
        }
    }
    out
}

fn assert_run(r: &Run, clip: AnimId, form: u8, times: &[f32], counts: &[f32]) {
    // The clip, then ShootFlowEnd; the weapon form on from the first tick until 0.3 s into ShootFlowEnd.
    let main_end = r.steps.iter().position(|s| s.1 == AnimId::ShootFlowEnd).expect("ShootFlowEnd");
    assert!(r.steps[..main_end].iter().all(|s| s.1 == clip), "{} only before ShootFlowEnd", clip.data().name);
    let t_end = r.steps[main_end].0;
    assert!((t_end - 5.4333).abs() <= 1.5 * DT, "ShootFlowEnd at the clip's end (5.4333 s): {t_end}");
    assert!(r.steps[..main_end].iter().all(|s| s.2 == form), "weapon form {form} through the clip");
    let back = r.steps.iter().position(|s| s.2 == 0).expect("the weapon form back");
    let t_back = r.steps[back].0;
    assert!((t_back - t_end - 0.3).abs() <= 1.5 * DT, "the weapon back at ShootFlowEnd's TresEnableChange: {} s in", t_back - t_end);
    let t_finish = r.steps.last().unwrap().0;
    assert!((t_finish - t_end - 0.8).abs() <= 1.5 * DT, "Finish at ShootFlowEnd's end: {}", t_finish - t_end);
    // (SHOT-FIX) 0x14140E500 at ShootFlowEnd is WorldSlow off, not the release: the aim mode (the spawner's locked list)
    // stays open through ShootFlowEnd; End's slot 809 closes it.
    assert!(r.steps[main_end..r.steps.len() - 1].iter().all(|s| s.5 != 0), "the aim mode open until End");
    // Floated 200 cm up (his centre 200 above the floor), held there.
    let top = r.steps.iter().map(|s| s.3).fold(f32::MIN, f32::max);
    assert!(r.before_z < 150.0 && (top - 200.0).abs() < 1.0, "lifted {} -> {top}", r.before_z);
    // The spheres: N of them, on KH3's schedule (+- 1 tick).
    assert_eq!(r.spawns.len(), r.n, "one sphere per lock");
    let want = kh3_schedule(times, counts, r.n);
    for (i, (t, w)) in r.spawns.iter().zip(&want).enumerate() {
        assert!((t - w).abs() <= 1.5 * DT, "sphere {i}: {t} s, KH3 {w} s\n{:?}\n{want:?}", r.spawns);
    }
}

fn rows(key: &str) -> (Vec<f32>, Vec<f32>) {
    // The TresShot times / counts of the clip (kh_core's sheet rows are crate-private: the values of tools/shotlocks/vacuum.js).
    match key {
        "k15" => (vec![1.4, 1.766666, 2.1, 2.5, 3.596874, 4.7], vec![2.0, 2.0, 2.0, 7.0, 7.0, 4.0]),
        _ => (vec![1.4, 1.766666, 2.1, 2.5, 3.505814, 4.7], vec![4.0, 4.0, 4.0, 7.0, 7.0, 6.0]),
    }
}

#[test]
fn frozen_crescents_drops_24_spheres_and_its_blades_hit_nothing() {
    // One dummy close in front (inside the dance's reach) and one far.
    let mut r = vacuum("WEP_KEYBLADE_SO_06", 15, "TresProjFZ_VacuumWave_C", &[dummy(1, 250.0, 0.0), dummy(2, 800.0, 150.0)]);
    assert_eq!(r.n, 24, "MAX = Param0 24");
    let (times, counts) = rows("k15");
    assert_run(&r, AnimId::A7481, 7, &times, &counts);
    r.b.run(240);
    let hits = r.b.hits();
    // Only the spheres hit (Blizzard Blades have no KB_Stick* shape): 24, row SF_FZVACUUMWAVE, lock by lock.
    assert_eq!(hits.len(), 24, "{:?}", hits.iter().map(|h| (h.attack_name, h.target)).collect::<Vec<_>>());
    assert!(hits.iter().all(|h| h.attack_name == "SF_FZVACUUMWAVE" && h.flags == 0xFD | 15 << 8));
    let first = r.locks[0];
    assert!(hits.iter().take(first.1 as usize).all(|h| h.target == first.0), "locked[0] {first:?} first");
    r.b.assert_back_to_normal();
}

#[test]
fn flag_rampage_drops_32_spheres_and_the_flag_hits_in_reach() {
    let mut r = vacuum("WEP_KEYBLADE_SO_09", 17, "TresProjCA_VacuumWave_C", &[dummy(1, 250.0, 0.0), dummy(2, 800.0, 150.0)]);
    assert_eq!(r.n, 32, "MAX = Param0 32");
    let (times, counts) = rows("k17");
    assert_run(&r, AnimId::F481, 0xF, &times, &counts);
    r.b.run(240);
    let hits = r.b.hits();
    let spheres = hits.iter().filter(|h| h.flags == 0xFD | 17 << 8).count();
    assert_eq!(spheres, 32, "the 32 spheres");
    assert!(hits.iter().all(|h| h.attack_name == "SF_VACUUMWAVE"));
    // Storm Flag's KB_StickL (r 120) reaches the close dummy in the TresAtk windows: melee hits on top of the spheres.
    let melee: Vec<_> = hits.iter().filter(|h| h.flags != 0xFD | 17 << 8).collect();
    assert!(!melee.is_empty() && melee.len() <= 8, "1..8 flag hits on the close dummy: {}", melee.len());
    assert!(melee.iter().all(|h| h.target == TargetId(1)), "only the close one");
    r.b.assert_back_to_normal();
}

#[test]
fn far_targets_take_only_the_spheres() {
    let mut r = vacuum("WEP_KEYBLADE_SO_09", 17, "TresProjCA_VacuumWave_C", &[dummy(1, 800.0, 0.0), dummy(2, 800.0, 150.0)]);
    r.b.run(240);
    let hits = r.b.hits();
    assert_eq!(hits.len(), r.n, "{:?}", hits.iter().map(|h| (h.attack_name, h.flags)).collect::<Vec<_>>());
    r.b.assert_back_to_normal();
}
