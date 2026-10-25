//! The automatic motion sounds (docs/sora-audio-spec.md section 4.2) on RECORDED bone
//! trajectories of the port (tests/data/sora_walk_run_jump.csv: khgame.exe, `--audio-bones`,
//! one row per 60 Hz simulation step) and on small synthetic ones for the thresholds.

use glam::Vec3;
use kh_audio::autose::{ArmBones, AutoSe, AutoSeEvent, BoneFrame, FootBones, Motion, Part};
use kh_audio::RULES;

const DT: f32 = 1.0 / 60.0;

struct Row {
    step: u32,
    anim: String,
    time: f32,
    frame: BoneFrame,
}

fn recorded() -> Vec<Row> {
    include_str!("data/sora_walk_run_jump.csv")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            let mut p = [Vec3::ZERO; 14];
            for (k, slot) in p.iter_mut().enumerate() {
                let f = |i: usize| c[4 + 3 * k + i].parse::<f32>().unwrap();
                *slot = Vec3::new(f(0), f(1), f(2));
            }
            Row { step: c[0].parse().unwrap(), anim: c[2].to_string(), time: c[3].parse().unwrap(), frame: BoneFrame::from_positions(&p, c[1] == "1") }
        })
        .collect()
}

/// Runs the detector over the recording; the listener stays 4 m from him.
fn detect(rows: &[Row], enabled: impl Fn(&Row) -> bool) -> Vec<(u32, String, f32, AutoSeEvent)> {
    let mut auto = AutoSe::new();
    let mut out = Vec::new();
    for row in rows {
        for event in auto.step(DT, &row.frame, 4.0, enabled(row)) {
            out.push((row.step, row.anim.clone(), row.time, event));
        }
    }
    out
}

#[test]
fn a_walk_loop_makes_one_step_sound_per_footfall() {
    let rows = recorded();
    assert_eq!(rows.len(), 720);
    let events = detect(&rows, |_| true);
    // WalkLoopN is 1.0667 s long = two footfalls: one Walk event every 32 steps, feet
    // alternating, always at the same two phases of the clip (half a cycle apart).
    let walk: Vec<_> = events.iter().filter(|e| e.3.motion == Motion::Walk && e.1 == "WalkLoopN").collect();
    assert!(walk.len() >= 6, "{}", walk.len());
    for pair in walk.windows(2) {
        assert_eq!(pair[1].0 - pair[0].0, 32, "steps {} -> {}", pair[0].0, pair[1].0);
        assert_ne!(pair[0].3.part, pair[1].3.part, "the feet alternate");
        assert!(((pair[1].2 - pair[0].2).abs() - 1.0666667 / 2.0).abs() < 1.5 * DT, "phases {} and {}", pair[0].2, pair[1].2);
    }
    let phase_of = |foot: usize| -> Vec<f32> { walk.iter().filter(|e| e.3.part == Part::Foot(foot)).map(|e| e.2).collect() };
    for foot in 0..2 {
        let phases = phase_of(foot);
        let (lo, hi) = phases.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        assert!(phases.len() >= 3 && hi - lo < 1.5 * DT, "foot {foot}: {phases:?}");
    }
    // Volume from the waist speed (1.4 m/s of 0 .. 1.5) plus the foot's energy term: near full.
    assert!(walk.iter().all(|e| e.3.volume > 0.85 && e.3.volume <= 1.0), "{:?}", walk.iter().map(|e| e.3.volume).collect::<Vec<_>>());
    // Not running: the walk is 1.4 m/s, under the 4.1667 m/s threshold.
    assert!(!events.iter().any(|e| e.3.motion == Motion::Run && e.1 == "WalkLoopN"));
    assert_eq!(RULES.autose_run_speed, 4.1666675);
}

#[test]
fn a_run_loop_makes_one_run_sound_per_footfall() {
    let events = detect(&recorded(), |_| true);
    // RunLoopN is 0.5333 s long = two footfalls: one Run event every 16 steps.
    let run: Vec<_> = events.iter().filter(|e| e.3.motion == Motion::Run && e.1 == "RunLoopN").collect();
    assert!(run.len() >= 7, "{}", run.len());
    for pair in run.windows(2) {
        assert_eq!(pair[1].0 - pair[0].0, 16, "steps {} -> {}", pair[0].0, pair[1].0);
        assert_ne!(pair[0].3.part, pair[1].3.part);
    }
    for foot in 0..2 {
        let phases: Vec<f32> = run.iter().filter(|e| e.3.part == Part::Foot(foot)).map(|e| e.2).collect();
        let (lo, hi) = phases.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        assert!(hi - lo < 1.5 * DT, "foot {foot}: {phases:?}");
    }
    // 6.8 m/s of 0 .. 5: full volume.
    assert!(run.iter().all(|e| e.3.volume == 1.0));
    // While running (waist at or above 4.1667 m/s) the arm rustle is the SHORT one and the leg
    // rustle the foot-short one.
    let in_run = |m: Motion| events.iter().filter(|e| e.3.motion == m && e.1 == "RunLoopN").count();
    eprintln!("rustles in the run loop: short {} long {} foot short {} foot long {}", in_run(Motion::RustleShort), in_run(Motion::RustleLong), in_run(Motion::RustleFootShort), in_run(Motion::RustleFootLong));
    // (The long forms appear only while the run loop plays under the running speed: the port speed
    // ramp at its start and end.)
    assert!(in_run(Motion::RustleShort) >= 6 && in_run(Motion::RustleLong) <= 3 && in_run(Motion::RustleFootShort) >= 6 && in_run(Motion::RustleFootLong) <= 4);
    // Never more than two events in a step.
    let mut per_step = std::collections::HashMap::new();
    for e in &events {
        *per_step.entry(e.0).or_insert(0) += 1;
    }
    assert!(per_step.values().all(|n| *n <= RULES.autose_events_per_frame));
}

#[test]
fn a_jump_and_its_landing_fire_once() {
    let rows = recorded();
    let events = detect(&rows, |_| true);
    let of = |m: Motion| events.iter().filter(|e| e.3.motion == m).map(|e| (e.0, e.1.as_str())).collect::<Vec<_>>();
    // Take-off: the step the movement mode leaves walking, in the jump start clip.
    let jumps = of(Motion::Jump);
    assert_eq!(jumps.len(), 1, "{jumps:?}");
    assert_eq!(jumps[0].1, "JumpNormalStart");
    let take_off = rows.windows(2).find(|w| w[0].frame.grounded && !w[1].frame.grounded).unwrap()[1].step;
    assert_eq!(jumps[0].0, take_off);
    // Touch-down after about a second in the air: LandNormal (>= 0.3 s, < 1.5 s), once.
    let lands = of(Motion::LandNormal);
    let touch_down = rows.windows(2).find(|w| !w[0].frame.grounded && w[1].frame.grounded).unwrap()[1].step;
    assert_eq!(lands.iter().map(|l| l.0).collect::<Vec<_>>(), [touch_down]);
    assert!(of(Motion::LandHard).is_empty());
    let air = (touch_down - take_off) as f32 * DT;
    assert!(air >= RULES.autose_land_normal_air_time && air < RULES.autose_land_hard_air_time, "{air}");

    // With the enable windows of the clips (spec 4.3: the fall loop and the landing clips switch
    // the detectors off) the detector's own landing is suppressed: the landing sound is then the
    // explicit PlayAutoSe notify of `Land`. The jump still sounds.
    let windowed = detect(&rows, |row| !matches!(row.anim.as_str(), "JumpNormalDownLoop" | "Land" | "LightLand"));
    assert_eq!(windowed.iter().filter(|e| e.3.motion == Motion::Jump).count(), 1);
    assert_eq!(windowed.iter().filter(|e| matches!(e.3.motion, Motion::LandNormal | Motion::LandHard)).count(), 0);
}

/// A standing figure: feet under the hips, arms at the sides (cm).
fn standing(at: Vec3) -> BoneFrame {
    let foot = |y: f32| FootBones { femur: at + Vec3::new(0.0, y, 85.0), tibia: at + Vec3::new(0.0, y, 45.0), foot: at + Vec3::new(0.0, y, 9.0), toe: at + Vec3::new(12.0, y, 2.0) };
    let arm = |y: f32| ArmBones { shoulder: at + Vec3::new(0.0, y, 130.0), elbow: at + Vec3::new(0.0, y, 105.0) };
    BoneFrame { base: at, waist: at + Vec3::new(0.0, 0.0, 90.0), feet: [foot(-10.0), foot(10.0)], arms: [arm(-20.0), arm(20.0)], grounded: true, foot_contact: [None; 2], hidden: false }
}

fn warm(auto: &mut AutoSe, frame: &BoneFrame) {
    for _ in 0..6 {
        assert!(auto.step(DT, frame, 3.0, true).is_empty());
    }
}

#[test]
fn landings_are_normal_from_0_3_s_and_hard_from_1_5_s_of_air_time() {
    for (air_steps, expected) in [(10, None), (17, None), (19, Some(Motion::LandNormal)), (60, Some(Motion::LandNormal)), (89, Some(Motion::LandNormal)), (91, Some(Motion::LandHard))] {
        let mut auto = AutoSe::new();
        warm(&mut auto, &standing(Vec3::ZERO));
        let mut events = Vec::new();
        for k in 0..air_steps {
            let mut f = standing(Vec3::new(0.0, 0.0, 50.0 + k as f32));
            f.grounded = false;
            events.extend(auto.step(DT, &f, 3.0, true));
        }
        // Take-off: Jump at the left foot and the long rustle, volume 1.
        assert_eq!(events, [AutoSeEvent { motion: Motion::Jump, part: Part::Foot(0), volume: 1.0 }, AutoSeEvent { motion: Motion::RustleLong, part: Part::Arm(0), volume: 1.0 }]);
        let landed = auto.step(DT, &standing(Vec3::ZERO), 3.0, true);
        let land: Vec<Motion> = landed.iter().map(|e| e.motion).filter(|m| matches!(m, Motion::LandNormal | Motion::LandHard)).collect();
        assert_eq!(land, expected.into_iter().collect::<Vec<_>>(), "{air_steps} steps in the air: {landed:?}");
        // Touch-down also rustles (within the limit of two events).
        // (unless the arm is still in the 0.3333 s cooldown of the take-off rustle).
        assert!(landed.len() <= 2 && (air_steps < 30 || landed.iter().any(|e| e.motion == Motion::RustleLong)));
    }
}

#[test]
fn the_arm_rustle_fires_when_the_elbow_outruns_the_waist_past_its_threshold() {
    // Walking: 0.45 m/s, RustleLong, volume (rel - T) / (1 - T); running: 0.8 m/s, RustleShort.
    let swing = |speed_cm_s: f32| -> Vec<AutoSeEvent> {
        let mut auto = AutoSe::new();
        let mut f = standing(Vec3::ZERO);
        warm(&mut auto, &f);
        f.arms[1].elbow.x += speed_cm_s * DT;
        auto.step(DT, &f, 3.0, true)
    };
    assert!(swing(40.0).is_empty(), "0.40 m/s is under the walking threshold");
    let got = swing(70.0);
    assert_eq!(got.len(), 1);
    assert_eq!((got[0].motion, got[0].part), (Motion::RustleLong, Part::Arm(1)));
    assert!((got[0].volume - (0.70 - 0.45) / (1.0 - 0.45)).abs() < 0.01, "{}", got[0].volume);
    assert_eq!(swing(200.0)[0].volume, 1.0);

    // The cooldown: 0.1333 s on BOTH arms.
    let mut auto = AutoSe::new();
    let mut f = standing(Vec3::ZERO);
    warm(&mut auto, &f);
    f.arms[1].elbow.x += 70.0 * DT;
    assert_eq!(auto.step(DT, &f, 3.0, true).len(), 1);
    assert!(auto.step(DT, &f, 3.0, true).is_empty());
    f.arms[0].elbow.x += 70.0 * DT;
    assert!(auto.step(DT, &f, 3.0, true).is_empty(), "the other arm is in the cooldown too");
    for _ in 0..8 {
        auto.step(DT, &f, 3.0, true);
    }
    f.arms[0].elbow.x += 70.0 * DT;
    assert_eq!(auto.step(DT, &f, 3.0, true).iter().map(|e| e.part).collect::<Vec<_>>(), [Part::Arm(0)]);

    // Beyond 10 m from the listener: no rustle. Beyond 20 m: nothing at all.
    let mut auto = AutoSe::new();
    let mut f = standing(Vec3::ZERO);
    for _ in 0..6 {
        auto.step(DT, &f, 12.0, true);
    }
    f.arms[1].elbow.x += 70.0 * DT;
    assert!(auto.step(DT, &f, 12.0, true).is_empty());
    assert_eq!((RULES.autose_rustle_distance, RULES.autose_clip_distance), (10.0, 20.0));
    let mut f2 = f;
    f2.grounded = false;
    assert_eq!(auto.step(DT, &f2, 12.0, true).iter().map(|e| e.motion).collect::<Vec<_>>(), [Motion::Jump], "a take-off beyond 10 m: the jump without the rustle");
    assert!(auto.step(DT, &standing(Vec3::ZERO), 25.0, true).is_empty());
}

#[test]
fn the_leg_rustle_fires_when_the_thigh_closes_on_the_spine_direction() {
    // dot(normalize(Tibia - Femur), normalize(Base - Waist)) crossing upward through 0.6;
    // volume = knee speed / 1.0 m/s; exempt from the cooldown.
    let mut auto = AutoSe::new();
    let mut f = standing(Vec3::ZERO);
    // The left knee raised forward: the thigh is far from the downward spine direction.
    f.feet[0].tibia = f.feet[0].femur + Vec3::new(38.0, 0.0, -12.0);
    f.feet[0].foot = f.feet[0].tibia + Vec3::new(0.0, 0.0, -36.0);
    warm(&mut auto, &f);
    // It comes down: the dot passes 0.6.
    f.feet[0].tibia = f.feet[0].femur + Vec3::new(20.0, 0.0, -34.0);
    f.feet[0].foot = f.feet[0].tibia + Vec3::new(0.0, 0.0, -36.0);
    let got = auto.step(DT, &f, 3.0, true);
    let rustle: Vec<_> = got.iter().filter(|e| e.motion == Motion::RustleFootLong).collect();
    assert_eq!(rustle.len(), 1, "{got:?}");
    assert_eq!((rustle[0].part, rustle[0].volume), (Part::Foot(0), 1.0));
    assert_eq!(RULES.autose_rustle_leg_dot_threshold, 0.6);
}

#[test]
fn a_part_in_its_cooldown_drops_events_and_the_detector_warms_up_after_a_reset() {
    // Three warm-up frames after a reset.
    let mut auto = AutoSe::new();
    let mut f = standing(Vec3::ZERO);
    let mut air = f;
    air.grounded = false;
    auto.step(DT, &f, 3.0, true);
    assert!(auto.step(DT, &air, 3.0, true).is_empty(), "inside the warm-up");
    // Hidden: nothing, and a warm-up again afterwards.
    let mut auto = AutoSe::new();
    warm(&mut auto, &f);
    let mut hidden = air;
    hidden.hidden = true;
    assert!(auto.step(DT, &hidden, 3.0, true).is_empty());
    assert!(auto.step(DT, &air, 3.0, true).is_empty());
    // Disabled by a notify window: nothing, but the history goes on.
    let mut auto = AutoSe::new();
    warm(&mut auto, &f);
    assert!(auto.step(DT, &air, 3.0, false).is_empty());
    assert!(auto.step(DT, &air, 3.0, true).is_empty(), "the take-off edge was in the disabled frame");

    // A foot that steps twice within 0.3333 s sounds once (the geometric rule: a rising edge of
    // foot-grounded with the foot moving faster than 0.125 m/s before).
    let mut auto = AutoSe::new();
    warm(&mut auto, &f);
    let lift = |f: &mut BoneFrame, up: f32| {
        f.feet[1].foot.z = 9.0 + up;
        f.feet[1].tibia.z = 45.0 + up;
    };
    let mut steps = 0;
    for cycle in 0..4 {
        for k in 0..5 {
            lift(&mut f, [12.0, 12.0, 8.0, 4.0, 0.0][k]);
            steps += auto.step(DT, &f, 3.0, true).iter().filter(|e| e.motion == Motion::Walk).count();
        }
        // 5 steps per cycle = 0.083 s: the second, third and fourth touch-downs are dropped.
        assert_eq!(steps, 1, "cycle {cycle}");
    }
    assert_eq!(RULES.autose_part_cooldown, 0.33333334);
    // The host's contact flag, when there is one, makes a step on its rising edge by itself.
    let mut auto = AutoSe::new();
    let mut f = standing(Vec3::ZERO);
    f.foot_contact = [Some(false), Some(false)];
    warm(&mut auto, &f);
    f.foot_contact = [Some(true), Some(false)];
    assert_eq!(auto.step(DT, &f, 3.0, true).iter().map(|e| (e.motion, e.part)).collect::<Vec<_>>(), [(Motion::Walk, Part::Foot(0))]);
}
