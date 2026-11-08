//! (SHOT-A rush) Drill Dive (Favorite Deputy, `ETresShootFlowKind` 9, the MAX kind of Ghost Horde): the trace lock-on ->
//! MAX -> fire -> M_6481 (Drill Punch's weapon on, the 200 cm lift) -> one dive per lock through the locked targets in
//! list order (M_6482 each: from 800 cm before the target's point - 100 z to 800 cm past it in 0.3 s, each dive 120
//! degrees further round, the StickL window hitting SF_WARPRUSH once per dive) -> M_6483 where he started ->
//! ShootFlowEnd (the weapon back, the world slow off) -> back to normal (magic/shotlocks/rush.rs, update 0x1413BC550).
//!
//! While Ghost Horde (kind 8, SHOT-A's avatar family) is still pending, SelectKind's base kind falls back to the Kingdom
//! Key's and its MAX to Ragnarok; the bench then sets KH3's own debug override of SelectKind (ctrl+0xA34,
//! `AimMode::debug_kind`) to 8, the base kind KH3 gives weapon type 4, so MAX gives kind 9 as it does in KH3.

mod shotlocks_common;

use kh_core::host::Call;
use kh_core::{AnimId, Pose, Sora, StateId, TargetInfo, Vec3, SORA_PARAMS};
use shotlocks_common::{dummy, level99_with, seconds, Bench, DT};

/// The bench holding Favorite Deputy, with Ghost Horde as the base kind (see the module text).
fn favorite_deputy(targets: &[TargetInfo]) -> Bench {
    let mut b = Bench::new(level99_with("WEP_KEYBLADE_SO_03"), targets);
    if !kh_core::magic::shotlocks::ready(8) {
        b.sora.magic_mut().aim.debug_kind = 8;
    }
    b
}

fn centre(position: Vec3) -> Vec3 {
    position + Vec3::Z * SORA_PARAMS.capsule_center_z
}

#[test]
fn drill_dive_lock_on_fire_finish_back_to_normal() {
    // Two targets far enough apart (1200 cm) that no dive's 1600 cm line through one passes the other (>= 890 cm off).
    let targets = [dummy(1, 800.0, 0.0), dummy(2, 2000.0, 0.0)];
    let mut b = favorite_deputy(&targets);
    // LOCK-ON: Ghost Horde (8, Param0 12) while aiming, Drill Dive (9) at MAX.
    assert_eq!(b.aim_until(1, 200), 8, "Ghost Horde while aiming");
    assert_eq!(b.aim_until(0, 900), 9, "Drill Dive at MAX");
    let aim = b.sora.aim();
    assert_eq!((aim.total, aim.max_locks, aim.is_max), (12, 12, true), "{aim:?}");
    let locks = aim.locks.clone();
    assert_eq!(locks.iter().map(|l| l.1).sum::<i32>(), 12);
    assert_eq!(locks.len(), 2, "both targets locked: {locks:?}");
    // FIRE.
    b.fire();
    assert_eq!(b.sora.magic().shot_weapon_form, 6, "Drill Punch's weapon (vt+0x13C8(type, 6))");
    let mut steps = Vec::new();
    for _ in 0..900 {
        let o = b.step(&[]);
        steps.push((o.state, o.sub_step, o.anim.name, o.anim.time, o.position, o.yaw, b.sora.magic().shot_weapon_form, b.sora.time_dilation(2), b.hits().len()));
        if o.state != StateId::Shootflow {
            break;
        }
    }
    assert_ne!(steps.last().unwrap().0, StateId::Shootflow, "ended");
    let at = |clip: AnimId| steps.iter().position(|s| s.2 == clip).unwrap_or_else(|| panic!("no {clip:?}"));
    let (rise, dive, land, end) = (at(AnimId::A6481), at(AnimId::A6482), at(AnimId::A6483), at(AnimId::ShootFlowEnd));
    // M_6481 (0.6 s): the 200 cm lift (LiftCheck(200) on the floor) and its root motion.
    assert!(rise <= 1, "M_6481 at once ({rise})");
    assert!((seconds(dive - rise) - 0.6).abs() < 0.05, "the first dive {} s after M_6481", seconds(dive - rise));
    // (Type 2 is at the aim mode's 0.4 until End releases it.)
    assert!(steps[..dive].iter().all(|s| s.6 == 6 && s.7 > 0.3), "the drill on, no world slow yet: {:?}", steps[..dive].iter().map(|s| (s.1, s.6, s.7)).collect::<Vec<_>>());
    // The dives: one per lock, 0.3 s each (+0xA4 = 0.3 counted down by dt: 18 or 19 ticks).
    let dives = seconds(land - dive);
    assert!((dives - 12.0 * 0.3).abs() < 12.0 * DT + 0.02, "12 dives in {dives} s");
    assert!(steps[dive..land].iter().all(|s| s.7 < 0.2), "the world slow (0.1) during the dives");
    // Each dive starts 800 cm before the target's point (its centre - 100 z) and passes through it; the targets in the
    // aim's list order, each as often as its locks; each start turned a further 120 degrees (another 120 after the last
    // lock of a target).
    let mut starts = Vec::new();
    for k in dive..land {
        // A warp: a jump far beyond one dash step (1600 cm / 18 ticks).
        if k == dive || (steps[k].4 - steps[k - 1].4).length() > 300.0 {
            starts.push(k);
        }
    }
    assert_eq!(starts.len(), 12, "a warp per dive: {starts:?}");
    assert!(starts.windows(2).all(|w| matches!(w[1] - w[0], 18 | 19)), "0.3 s per dive: {starts:?}");
    // +0xAC: his yaw at M_6481's end (the tick before the first warp's).
    let mut yaw = steps[dive - 1].5;
    let mut i = 0;
    for &(target, count) in &locks {
        let t = targets.iter().find(|t| t.id == target).unwrap();
        let p = t.center - Vec3::Z * 100.0;
        for n in 0..count {
            yaw -= 120.0;
            let (sin, cos) = yaw.to_radians().sin_cos();
            let d = Vec3::new(cos, sin, 0.0);
            let k = starts[i];
            // The first tick of a dive: the warp put him at +0xD8, the dash of that tick moved him one step on.
            let c = centre(steps[k].4);
            let along = (c - p).dot(d);
            assert!((-800.0..-650.0).contains(&along), "dive {i} starts {along} cm along the line (target {target:?})");
            assert!((c - p - d * along).length() < 1.0, "dive {i} on the line through the target: {c:?} / {p:?}");
            let facing = kh_core::Vec3::new(steps[k].5.to_radians().cos(), steps[k].5.to_radians().sin(), 0.0);
            assert!(facing.dot(d) > 0.999, "dive {i} faces along its line");
            // ... and ends 800 cm past it.
            let last = starts.get(i + 1).copied().unwrap_or(land) - 1;
            let along_end = (centre(steps[last].4) - p).dot(d);
            // (0x1413BE650 places him with +0xA4 BEFORE its countdown: the last place a step shows is at +0xA4 = 2 dt,
            // 800 - 1600 x 2 dt / 0.3 = 622 cm; the step that reaches 0 warps on.)
            assert!((600.0..=800.0).contains(&along_end), "dive {i} ends {along_end} cm past the target");
            i += 1;
            if n + 1 == count {
                yaw -= 120.0;
            }
        }
    }
    // M_6483 (0.9667 s) where he started (+0xE4) with his rotation (+0x104), then ShootFlowEnd (0.8 s).
    let lifted = centre(steps[dive - 1].4);
    // (+0xE4 is taken in the update of M_6481's last tick, after that tick's root motion: tens of cm on.)
    let back_at = centre(steps[land].4);
    assert!((back_at.truncate() - lifted.truncate()).length() < 60.0 && (back_at.z - lifted.z).abs() < 1.0, "back at the start: {back_at:?} / {lifted:?}");
    assert!((steps[land].5 - steps[dive - 1].5).abs() < 1.0e-3, "his rotation back (+0x104)");
    assert!((seconds(end - land) - 0.9667).abs() < 0.05, "ShootFlowEnd {} s after M_6483", seconds(end - land));
    assert!((seconds(steps.len() - end) - 0.8).abs() < 0.05, "ended {} s after ShootFlowEnd", seconds(steps.len() - end));
    // The weapon of Begin and the world speed back at ShootFlowEnd's TresEnableChange (0.3 s).
    let back = steps.iter().position(|s| s.6 == 0).expect("the Keyblade back");
    assert!(back > end && (seconds(back - end) - 0.3).abs() < 0.04, "weapon back {} s into ShootFlowEnd", seconds(back - end));
    assert!(steps[back].7 > 0.3 && steps[back - 1].7 < 0.2, "the world slow off with it");
    // THE HITS: SF_WARPRUSH, one per dive (12), as many on each target as its locks.
    b.run(30);
    let hits = b.hits();
    assert_eq!(hits.len(), 12, "{:?}", hits.iter().map(|h| (h.attack_name, h.target)).collect::<Vec<_>>());
    assert!(hits.iter().all(|h| h.attack_name == "SF_WARPRUSH"), "{:?}", hits.iter().map(|h| h.attack_name).collect::<Vec<_>>());
    // (the generic window path: HitRecord::flags carry no command there, as Infinity Circle's; the row is SIDESPIN)
    assert!(hits.iter().all(|h| h.flags == 0 && h.damage_kind == kh_core::enums::ETresDamageKind::Sidespin && h.damage > 0.0), "{:?}", hits.iter().map(|h| (h.flags, h.damage_kind, h.damage)).collect::<Vec<_>>());
    for &(target, count) in &locks {
        assert_eq!(hits.iter().filter(|h| h.target == target).count() as i32, count, "{target:?}");
    }
    assert!(steps[..dive].iter().all(|s| s.8 == 0), "no hit before the first dive");
    // The effect of every warp: WarpRush (ead_so053_stf0, group 0x686) on the drill's socket.
    let warps = b.host.calls.iter().filter(|c| matches!(c, Call::Spawn(_, e) if e.name == "ead_so053_stf0" && e.id.0 == 0x686)).count();
    assert_eq!(warps, 12, "one WarpRush per warp");
    // BACK TO NORMAL.
    b.assert_back_to_normal();
    assert_eq!(b.sora.magic().shot_weapon_form, 0);
    assert!(!b.sora.magic().shot_body_hidden);
}

/// A state dropped in the middle (a teleport) gives the weapon back and the world speed (End 0x1413ABA70).
#[test]
fn a_dropped_drill_dive_gives_the_weapon_back() {
    let mut b = favorite_deputy(&[dummy(1, 800.0, 0.0)]);
    assert_eq!(b.aim_until(0, 900), 9);
    b.fire();
    b.run(60);
    assert_eq!(b.sora.magic().shot_weapon_form, 6);
    b.sora.teleport(Pose { position: Vec3::new(0.0, 2000.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 });
    b.run(3);
    assert_eq!((b.sora.aim().mode, b.sora.magic().shot_weapon_form), (0, 0));
    assert_eq!(b.sora.time_dilation(2), 1.0);
}
