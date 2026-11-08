//! (SHOT-A gate) Warp Trick (Happy Gear, `ETresShootFlowKind` 13 MI_GATE): KH3's update 0x1413B7B40 and its spawner
//! 0x1413C02D0 (magic/shotlocks/gate.rs). SelectKind picks it at MAX (Snakebite 12's Param0 18). Trace: lock-on -> MAX ->
//! fire -> Twin Yo-Yos on, M_4481 (0.8667 s) -> M_4482 looped (the gates: 0.5 s in, up to 6 per tick, each locked target
//! its lock count, 0.1 s between targets; each gate 300 cm from the target's aim point above it, flying through it) ->
//! 0.5 s after the last gate M_4483 (0.6667 s, root motion back) -> Finish -> the Keyblade back -> back to normal.

mod shotlocks_common;

use std::collections::HashSet;

use kh_core::{AnimId, StateId, TargetId, TargetInfo, Vec3};
use shotlocks_common::{dummy, level99_with, seconds, Bench};

/// One gate as it appeared: the step, its position and flight direction.
struct Gate {
    step: usize,
    position: Vec3,
    forward: Vec3,
}

/// Steps while ShootFlow runs, recording every new projectile on its spawn step (the projectiles tick before the state:
/// a new one has not moved yet) and the clip of every step.
fn run_gates(b: &mut Bench, max_steps: usize) -> (Vec<AnimId>, Vec<Vec3>, Vec<Gate>) {
    let mut clips = Vec::new();
    let mut positions = Vec::new();
    let mut gates = Vec::new();
    let mut seen: HashSet<u32> = HashSet::new();
    for k in 0..max_steps {
        let o = b.step(&[]);
        clips.push(o.anim.name);
        positions.push(o.position);
        for v in b.sora.magic().projectile_views() {
            if seen.insert(v.id) {
                gates.push(Gate { step: k, position: v.position, forward: v.forward });
            }
        }
        if o.state != StateId::Shootflow {
            return (clips, positions, gates);
        }
    }
    panic!("ShootFlow did not end within {max_steps} steps");
}

/// Every gate starts 300 cm from a target's aim point, above it (elevation 0..60 deg), flying straight at it.
fn assert_round(g: &Gate, t: &TargetInfo) {
    let off = t.center - g.position;
    assert!((off.length() - 300.0).abs() < 0.5, "300 cm from the aim point: {:?} ({})", g.position, off.length());
    assert!(g.position.z >= t.center.z - 0.01, "above the aim point: {:?}", g.position);
    let elevation = (-off.z / off.length()).asin().to_degrees();
    assert!((-0.01..=60.01).contains(&elevation), "elevation {elevation}");
    assert!(g.forward.dot(off.normalize()) > 0.999, "flying at the aim point: {:?}", g.forward);
}

#[test]
fn warp_trick_lock_on_fire_finish_back_to_normal() {
    let target = dummy(1, 800.0, 0.0);
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_05"), &[target]);
    // Lock-on: Happy Gear's base kind is Snakebite (12); at MAX (Param0 18) SelectKind gives 13.
    let kind = b.aim_until(0, 1200);
    assert_eq!(kind, 13, "Warp Trick at MAX ({:?})", b.sora.aim());
    let n = b.sora.aim().total as usize;
    assert_eq!(n, 18, "TresShootFlowData row 12 Param0");
    b.fire();
    let start = b.time;
    assert_eq!(b.sora.magic().shot_weapon_form, 4, "Twin Yo-Yos on (vt+0x13C8(type, 4))");
    let (clips, positions, gates) = run_gates(&mut b, 600);
    let floating = positions[0];
    let at = |clip: AnimId| clips.iter().position(|c| *c == clip).unwrap_or_else(|| panic!("no {clip:?}"));
    let (charge, lp, end) = (at(AnimId::A4481), at(AnimId::A4482), at(AnimId::A4483));
    // The KH3 timeline: M_4481 0.8667 s, then M_4482 looped while the gates come, M_4483 0.6667 s, Finish.
    assert!(charge <= 1, "M_4481 at once ({charge})");
    assert!((seconds(lp - charge) - 0.8667).abs() < 0.05, "M_4482 {} s after M_4481", seconds(lp - charge));
    assert!(clips[lp..end].iter().all(|c| *c == AnimId::A4482), "M_4482 loops until M_4483");
    assert!((seconds(clips.len() - 1 - end) - 0.6667).abs() < 0.05, "ended {} s after M_4483", seconds(clips.len() - 1 - end));
    // The gates: none before 0.5 s into M_4482 (the first on the 30th spawner call), then 6, 6, 6 on three ticks.
    assert_eq!(gates.len(), n, "N gates");
    let first = gates[0].step;
    assert!((seconds(first - lp) - 0.4833).abs() < 0.02, "the first gates {} s into M_4482", seconds(first - lp));
    let per_step: Vec<usize> = (first..first + 3).map(|s| gates.iter().filter(|g| g.step == s).count()).collect();
    assert_eq!(per_step, vec![6, 6, 6], "up to 6 per tick");
    for g in &gates {
        assert_round(g, &target);
    }
    let spread = gates.iter().map(|g| g.position).fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p.y), hi.max(p.y)));
    assert!(spread.1 - spread.0 > 100.0, "the gates come from every side: {spread:?}");
    // M_4483 0.5 s after the last spawning tick (+0xA4 = 0.5).
    let last = gates.last().unwrap().step;    assert!((seconds(end - last) - 0.5).abs() < 0.02, "M_4483 {} s after the last gate", seconds(end - last));
    // Sora floats in place (no lift, no velocity) until M_4483, whose root motion drifts him back along his facing (-X).
    assert!(positions[..end].iter().all(|p| (*p - floating).length() < 1.0), "he stays where he fired");
    let drift = positions[end] - positions[clips.len() - 2];
    assert!(drift.x > 100.0 && drift.x < 200.0 && drift.y.abs() < 5.0, "M_4483's root motion back: {drift:?}");
    // The weapon form until End; then the Keyblade.
    assert_eq!(b.sora.magic().shot_weapon_form, 0, "the Keyblade back at End");
    // Each gate pierces through the target: one SF_MIGATE hit each, the shotlock's flags, the generic projectile path.
    b.run(60);
    let hits = b.hits();
    assert_eq!(hits.len(), n, "one hit per gate");
    assert!(hits.iter().all(|h| h.attack_name == "SF_MIGATE" && h.flags == 0xFD | 13 << 8 && h.target == TargetId(1)), "{:?}", hits.iter().map(|h| (h.attack_name, h.flags)).collect::<Vec<_>>());
    assert_eq!(b.sora.magic().projectile_views().count(), 0, "600 cm and done");
    assert!(b.time - start > 2.5, "about 2.6 s of shotlock");
    b.assert_back_to_normal();
}

/// With two locked targets each one gets exactly its lock count of gates, in lock order, and the spawner pauses 0.1 s
/// (7 ticks) before the next target.
#[test]
fn warp_trick_serves_each_target_its_locks_with_a_pause_between() {
    let (a, c) = (dummy(1, 800.0, -150.0), dummy(2, 800.0, 150.0));
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_05"), &[a, c]);
    assert_eq!(b.aim_until(0, 1200), 13);
    let locks = b.sora.aim().locks.clone();
    assert_eq!(locks.len(), 2, "both locked: {locks:?}");
    assert_eq!(locks.iter().map(|l| l.1).sum::<i32>(), 18);
    b.fire();
    let (_, _, gates) = run_gates(&mut b, 600);
    let of = |id: TargetId| if id == a.id { a } else { c };
    let mut at = 0;
    for &(id, count) in &locks {
        let t = of(id);
        let mine = &gates[at..at + count as usize];
        for g in mine {
            assert_round(g, &t);
        }
        if at > 0 {
            let gap = mine[0].step - gates[at - 1].step;
            assert!(gap == 7 || gap == 8, "a 0.1 s pause before the next target ({gap} ticks)");
        }
        at += count as usize;
    }
    assert_eq!(at, gates.len());
    // Each target is hit at least by its own gates (a gate pierces: one aimed at the other target may cross it too).
    b.run(60);
    for &(id, count) in &locks {
        let got = b.hits().iter().filter(|h| h.target == id).count();
        assert!(got >= count as usize && got <= 18, "{id:?}: {got} hits for {count} locks");
    }
}

/// A state dropped in the middle (a teleport) gives the Keyblade back (End 0x1413ABA70).
#[test]
fn a_dropped_warp_trick_gives_the_weapon_back() {
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_05"), &[dummy(1, 800.0, 0.0)]);
    assert_eq!(b.aim_until(0, 1200), 13);
    b.fire();
    b.run(30);
    assert_eq!(b.sora.magic().shot_weapon_form, 4);
    b.sora.teleport(kh_core::Pose { position: Vec3::new(0.0, 2000.0, kh_core::Sora::origin_height_on_floor(&kh_core::SORA_PARAMS)), yaw: 0.0 });
    b.run(3);
    assert_eq!((b.sora.aim().mode, b.sora.magic().shot_weapon_form), (0, 0));
}
