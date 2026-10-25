//! The field camera against the numbers of docs/sora-camera-spec.md.

use kh_camera::{target_point, CameraInput, CameraTarget, CameraView, FieldCamera, Rot, TargetState, CAMERA_PARAMS};
use kh_core::test_world::TestWorld;
use kh_core::{Vec2, Vec3, World};

const DT: f32 = 1.0 / 60.0;

fn standing(yaw: f32) -> CameraTarget {
    CameraTarget { position: Vec3::new(0.0, 0.0, 0.15), yaw, velocity: Vec3::ZERO, state: TargetState::Idle, collision_half_height: 89.0 }
}

/// A pawn that moves with a constant velocity, facing where it moves, in the Run state.
struct Runner {
    target: CameraTarget,
}

impl Runner {
    fn new(velocity: Vec3) -> Runner {
        Runner { target: CameraTarget { position: Vec3::new(0.0, 0.0, 0.15), yaw: velocity.y.atan2(velocity.x).to_degrees(), velocity, state: TargetState::Run, collision_half_height: 89.0 } }
    }

    fn advance(&mut self) -> CameraTarget {
        self.target.position += self.target.velocity * DT;
        self.target
    }
}

fn stick(x: f32, y: f32) -> CameraInput {
    CameraInput { right_stick: Vec2::new(x, y), ..CameraInput::default() }
}

fn steps(camera: &mut FieldCamera, n: usize, input: CameraInput, target: &CameraTarget, world: &dyn World) -> CameraView {
    let mut view = camera.view();
    for _ in 0..n {
        view = camera.step(DT, &input, target, world);
    }
    view
}

#[test]
fn start_state() {
    // Spec 2.9: directly behind Sora, yaw = his yaw, pitch -10, distance 400, no blend.
    let world = TestWorld::floor();
    let target = standing(35.0);
    let mut camera = FieldCamera::new(CAMERA_PARAMS, &target);
    assert_eq!(camera.input_yaw(), 35.0);
    let view = camera.step(DT, &CameraInput::default(), &target, &world);
    assert_eq!(view.rotation, Rot::new(-10.0, 35.0, 0.0));
    assert_eq!(view.rotation_target, view.rotation);
    assert!((view.distance - 400.0).abs() < 1.0e-3, "distance {}", view.distance);
    // Look point = actor origin + 102.338 + 48 (spec 2.3.1).
    assert!((view.look_point - (target.position + Vec3::Z * 150.338)).length() < 1.0e-3);
    assert!(((view.look_point - view.position).length() - view.distance).abs() < 1.0e-3);
    // Behind him: the camera is back along his facing, and above the look point (it looks down).
    let back = Rot::new(0.0, 35.0, 0.0).forward();
    assert!((view.position - view.look_point).dot(back) < -390.0 && view.position.z > view.look_point.z + 60.0);
    assert_eq!((view.auto_power, view.stick_wait, view.reset_wait), (0.0, 0.0, 0.0));
    // Nothing moves while he stands.
    let later = steps(&mut camera, 120, CameraInput::default(), &target, &world);
    assert_eq!(later.rotation, view.rotation);
    assert!((later.position - view.position).length() < 1.0e-3);
}

#[test]
fn projection_constants() {
    // Spec 1.5: 100 degrees horizontal, 16:9 constrained, near plane 3.
    assert_eq!(kh_camera::projection(&CAMERA_PARAMS), (100.0, 1.777778, 3.0));
}

#[test]
fn look_point_trails_by_74_cm_at_run_speed_and_never_by_more_than_100() {
    // Spec 2.3.2: steady lag v x (1/(60 x 0.1333) - 1/60) = 73.7 cm at 680 cm/s; capped at 100.
    let world = TestWorld::default();
    let lag = |speed: f32| {
        let mut runner = Runner::new(Vec3::new(speed, 0.0, 0.0));
        let mut camera = FieldCamera::new(CAMERA_PARAMS, &runner.target);
        let mut view = camera.view();
        for _ in 0..300 {
            let target = runner.advance();
            view = camera.step(DT, &CameraInput::default(), &target, &world);
        }
        (target_point(&CAMERA_PARAMS, &runner.target) - view.look_point).length()
    };
    let run = lag(680.0);
    assert!((run - 73.67).abs() < 0.5, "lag at 680 cm/s: {run} cm");
    let sprint = lag(980.0);
    assert!((sprint - 100.0).abs() < 0.01, "lag at 980 cm/s: {sprint} cm");
    let walk = lag(140.0);
    assert!((walk - 140.0 * 0.10833).abs() < 0.2, "lag at 140 cm/s: {walk} cm");
}

#[test]
fn right_stick_dead_zones_and_speed() {
    // Spec 2.5: engine dead zone 0.25 then the camera's 0.3: nothing below 0.475 raw; full
    // deflection turns the target rotation at 120 x 1.2424 = 149.09 deg/s, yaw and pitch alike.
    let world = TestWorld::floor();
    let target = standing(0.0);
    let turn = |x: f32, y: f32| {
        let mut camera = FieldCamera::new(CAMERA_PARAMS, &target);
        steps(&mut camera, 1, CameraInput::default(), &target, &world);
        let before = camera.view().rotation_target;
        let after = steps(&mut camera, 6, stick(x, y), &target, &world).rotation_target;
        ((after.yaw - before.yaw) / (6.0 * DT), (after.pitch - before.pitch) / (6.0 * DT))
    };
    assert_eq!(turn(0.47, 0.0), (0.0, 0.0));
    assert_eq!(turn(0.0, -0.47), (0.0, 0.0));
    let (yaw, _) = turn(0.48, 0.0);
    assert!(yaw > 0.0 && yaw < 3.0, "just above the dead zones: {yaw} deg/s");
    let (yaw, pitch) = turn(1.0, 0.0);
    assert!((yaw - 149.09).abs() < 0.05 && pitch == 0.0, "full right: {yaw} deg/s");
    let (yaw, _) = turn(-1.0, 0.0);
    assert!((yaw + 149.09).abs() < 0.05);
    // Linear in the rescaled deflection: raw 0.7375 is half way between 0.475 and 1.
    let (yaw, _) = turn(0.7375, 0.0);
    assert!((yaw - 149.09 * 0.5).abs() < 0.1, "half deflection: {yaw} deg/s");
    // The Y sign flip [I 0x1405CD525]: raw up (taken as positive) lowers the pitch.
    let (_, pitch) = turn(0.0, 1.0);
    assert!((pitch + 149.09).abs() < 0.05, "full up: {pitch} deg/s");
    assert!((kh_camera::sensitivity(50.0) - 1.2424).abs() < 1.0e-4);
    assert_eq!((kh_camera::sensitivity(1.0), kh_camera::sensitivity(100.0)), (0.5, 2.4));
}

#[test]
fn the_view_trails_the_target_rotation_at_8_per_second() {
    // Spec 2.4: each 1/60 s slice moves 13.33 % of the remaining angle; while the stick turns at
    // 149 deg/s the view trails by about 16 degrees and coasts to the target after release.
    let world = TestWorld::floor();
    let target = standing(0.0);
    let mut camera = FieldCamera::new(CAMERA_PARAMS, &target);
    let held = steps(&mut camera, 90, stick(1.0, 0.0), &target, &world);
    let trail = kh_camera::normalize_axis(held.rotation_target.yaw - held.rotation.yaw);
    assert!((trail - 149.09 * 0.10833).abs() < 0.2, "trail {trail} deg");
    let one = camera.step(DT, &CameraInput::default(), &target, &world);
    let left = kh_camera::normalize_axis(one.rotation_target.yaw - one.rotation.yaw);
    assert!((left / trail - (1.0 - 8.0 / 60.0)).abs() < 1.0e-3, "one step keeps {} of the gap", left / trail);
    let settled = steps(&mut camera, 33, CameraInput::default(), &target, &world);
    let gap = kh_camera::normalize_axis(settled.rotation_target.yaw - settled.rotation.yaw).abs();
    assert!(gap < trail * 0.011, "after 0.57 s the gap is {gap} deg of {trail}");
}

#[test]
fn pitch_is_clamped_to_minus_70_and_plus_50() {
    let world = TestWorld::default();
    let target = standing(0.0);
    let mut camera = FieldCamera::new(CAMERA_PARAMS, &target);
    let up = steps(&mut camera, 120, stick(0.0, -1.0), &target, &world);
    assert_eq!(up.rotation_target.pitch, 50.0);
    let down = steps(&mut camera, 180, stick(0.0, 1.0), &target, &world);
    assert_eq!(down.rotation_target.pitch, -70.0);
}

#[test]
fn distance_shrinks_from_400_to_200_as_the_pitch_goes_from_0_to_50() {
    // Spec 2.3.3; the smoothed distance follows at 4 /s.
    let world = TestWorld::default();
    let target = standing(0.0);
    let mut camera = FieldCamera::new(CAMERA_PARAMS, &target);
    let level = steps(&mut camera, 60, CameraInput::default(), &target, &world);
    assert!((level.distance - 400.0).abs() < 0.01);
    let top = steps(&mut camera, 600, stick(0.0, -1.0), &target, &world);
    assert!((top.rotation.pitch - 50.0).abs() < 0.01 && (top.distance - 200.0).abs() < 0.5, "pitch {} distance {}", top.rotation.pitch, top.distance);
    // Half way up: pitch +25 asks for 300.
    let mut camera = FieldCamera::new(CAMERA_PARAMS, &target);
    steps(&mut camera, 1, CameraInput::default(), &target, &world);
    let mut view = camera.view();
    while view.rotation_target.pitch < 25.0 {
        view = camera.step(DT, &stick(0.0, -1.0), &target, &world);
    }
    let held = steps(&mut camera, 600, CameraInput::default(), &target, &world);
    let wanted = 200.0 + 200.0 * (50.0 - held.rotation.pitch) / 50.0;
    assert!((held.distance - wanted).abs() < 0.5 && held.rotation.pitch > 24.0, "pitch {} distance {} (wanted {wanted})", held.rotation.pitch, held.distance);
    // Looking down changes nothing.
    let down = steps(&mut camera, 600, stick(0.0, 1.0), &target, &world);
    assert!((down.distance - 400.0).abs() < 0.5);
}

#[test]
fn automatic_follow_waits_1_5_s_after_the_stick_and_pulls_at_1_02_per_second() {
    // Spec 2.6: stickWait 1.5 s from the last stick frame; then autoPower ramps 1.2 /s up to 1.2;
    // rate = (speed2 / 800) x power = 1.02 /s at 680 cm/s.
    let world = TestWorld::default();
    let mut runner = Runner::new(Vec3::new(0.0, 680.0, 0.0)); // runs along +Y, facing yaw 90
    let mut camera = FieldCamera::new(CAMERA_PARAMS, &CameraTarget { yaw: 0.0, ..runner.target });
    // A touch of the stick, then hands off.
    let touched = camera.step(DT, &stick(1.0, 0.0), &runner.advance(), &world);
    assert!(touched.stick_active && touched.stick_wait == 1.5);
    let mut first = None;
    let mut views = Vec::new();
    for i in 0..400 {
        let view = camera.step(DT, &CameraInput::default(), &runner.advance(), &world);
        if first.is_none() && view.followed {
            first = Some(i);
        }
        views.push(view);
    }
    // The wait counts down by dt from the frame after the stick frame: 90 steps of 1/60 s.
    let first = first.expect("the follow never started");
    assert!((89..=91).contains(&first), "follow began {first} steps after the stick frame (1.5 s = 90)");
    assert!(views[..first].iter().all(|v| v.auto_power == 0.0 && v.rotation_target == views[0].rotation_target));
    // The ramp: 1.2 per second.
    assert!((views[first + 29].auto_power - 1.2 * 30.0 * DT).abs() < 1.0e-3, "power after 30 steps: {}", views[first + 29].auto_power);
    assert!((views[first + 60].auto_power - 1.2).abs() < 1.0e-6);
    // Full power: the yaw gap to "behind him" (90) closes by rate x dt per step.
    let gap = |v: &CameraView| kh_camera::normalize_axis(90.0 - v.rotation_target.yaw);
    let (a, b) = (gap(&views[first + 70]), gap(&views[first + 71]));
    let rate = (1.0 - b / a) / DT;
    assert!((rate - 1.02).abs() < 0.005, "follow rate {rate} /s");
    // ... and the pitch returns to -10.
    assert!((views.last().unwrap().rotation_target.pitch + 10.0).abs() < 0.5);
}

#[test]
fn automatic_follow_needs_movement_and_not_idle() {
    let world = TestWorld::default();
    let mut camera = FieldCamera::new(CAMERA_PARAMS, &standing(0.0));
    // Idle forces the velocity to 0, whatever it is.
    let idle = CameraTarget { yaw: 90.0, velocity: Vec3::new(0.0, 680.0, 0.0), ..standing(0.0) };
    let view = steps(&mut camera, 240, CameraInput::default(), &idle, &world);
    assert!(!view.followed && view.auto_power == 0.0 && view.rotation_target.yaw == 0.0);
    // Speed of 1 cm/s or less: nothing.
    let crawling = CameraTarget { state: TargetState::Run, velocity: Vec3::new(0.0, 1.0, 0.0), ..idle };
    assert!(!steps(&mut camera, 60, CameraInput::default(), &crawling, &world).followed);
}

#[test]
fn running_toward_the_camera_leaves_the_yaw_alone() {
    // Spec 2.6: facing < -0.8 (more than 143.13 degrees between the camera's heading and his
    // facing): only the pitch is pulled.
    let world = TestWorld::default();
    let run = |facing_yaw: f32| {
        let a = facing_yaw.to_radians();
        let mut runner = Runner::new(Vec3::new(680.0 * a.cos(), 680.0 * a.sin(), 0.0));
        let mut camera = FieldCamera::new(CAMERA_PARAMS, &CameraTarget { yaw: 0.0, ..runner.target });
        let mut view = camera.view();
        for _ in 0..120 {
            view = camera.step(DT, &CameraInput::default(), &runner.advance(), &world);
        }
        view
    };
    assert_eq!(run(180.0).rotation_target.yaw, 0.0);
    assert_eq!(run(150.0).rotation_target.yaw, 0.0);
    assert!(run(140.0).rotation_target.yaw > 20.0);
}

#[test]
fn slope_pitch_adds_the_slope_angle_while_running() {
    // Spec 2.6.3: slopes under 15 degrees do nothing; a 20 degree climb away from the camera asks
    // for pitch -10 + 20 = +10, after the ring has agreed (7 frames) and filled (16 frames).
    let world = TestWorld::default();
    let climb = |degrees: f32, state: TargetState| {
        let a = degrees.to_radians();
        let mut runner = Runner::new(Vec3::new(680.0 * a.cos(), 0.0, 680.0 * a.sin()));
        runner.target.yaw = 0.0;
        runner.target.state = state;
        let mut camera = FieldCamera::new(CAMERA_PARAMS, &runner.target);
        let mut view = camera.view();
        for _ in 0..600 {
            view = camera.step(DT, &CameraInput::default(), &runner.advance(), &world);
        }
        view
    };
    let up = climb(20.0, TargetState::Run);
    assert!((up.slope_pitch - 20.0).abs() < 0.01 && (up.rotation_target.pitch - 10.0).abs() < 0.2, "slope {} pitch {}", up.slope_pitch, up.rotation_target.pitch);
    let down = climb(-20.0, TargetState::Run);
    assert!((down.rotation_target.pitch + 30.0).abs() < 0.2, "pitch {}", down.rotation_target.pitch);
    let gentle = climb(14.0, TargetState::Run);
    assert!(gentle.slope_pitch == 0.0 && (gentle.rotation_target.pitch + 10.0).abs() < 0.01);
    // Not in the Run state: no slope term.
    let airborne = climb(20.0, TargetState::Other);
    assert!((airborne.rotation_target.pitch + 10.0).abs() < 0.01);
}

#[test]
fn a_long_fall_tilts_the_camera_down() {
    // Spec 2.6: falling faster than 800 cm/s: pitch down at max(vz x 0.05, -50) deg/s, not below -50.
    let world = TestWorld::default();
    let mut runner = Runner::new(Vec3::new(0.0, 0.0, -900.0));
    runner.target.yaw = 0.0;
    runner.target.state = TargetState::Other;
    let mut camera = FieldCamera::new(CAMERA_PARAMS, &runner.target);
    let first = camera.step(DT, &CameraInput::default(), &runner.advance(), &world);
    let second = camera.step(DT, &CameraInput::default(), &runner.advance(), &world);
    let rate = (second.rotation_target.pitch - first.rotation_target.pitch) / DT;
    assert!((rate + 45.0).abs() < 0.01, "tilt {rate} deg/s (900 x 0.05 = 45)");
    let mut view = second;
    for _ in 0..240 {
        view = camera.step(DT, &CameraInput::default(), &runner.advance(), &world);
    }
    assert!(view.rotation_target.pitch <= -50.0 && view.rotation_target.pitch > -50.8, "pitch {}", view.rotation_target.pitch);
}

#[test]
fn recentre_eases_and_does_not_snap() {
    // Spec 2.8: rotTarget jumps to behind Sora / pitch -10, resetWait 0.5, the view follows at 8 /s.
    let world = TestWorld::floor();
    let target = standing(0.0);
    let mut camera = FieldCamera::new(CAMERA_PARAMS, &target);
    steps(&mut camera, 60, stick(1.0, -0.6), &target, &world);
    let turned = steps(&mut camera, 60, CameraInput::default(), &target, &world);
    assert!(turned.rotation.yaw > 100.0);
    let pressed = camera.step(DT, &CameraInput { recentre: true, ..CameraInput::default() }, &target, &world);
    assert_eq!(pressed.rotation_target, Rot::new(-10.0, 0.0, 0.0));
    assert!((pressed.reset_wait - (0.5 - DT)).abs() < 1.0e-5);
    let kept = kh_camera::normalize_axis(pressed.rotation.yaw) / turned.rotation.yaw;
    assert!((kept - (1.0 - 8.0 / 60.0)).abs() < 1.0e-3, "first step keeps {kept} of the angle");
    let later = steps(&mut camera, 33, CameraInput::default(), &target, &world);
    assert!(later.rotation.yaw.abs() < turned.rotation.yaw * 0.011 && later.reset_wait == 0.0);
    // The look point is not touched (and the distance only follows the pitch as always).
    assert_eq!(later.look_point, turned.look_point);
}

#[test]
fn collision_pulls_in_at_once_and_eases_out_at_4_per_second() {
    // Spec 3.3: a sphere of radius 20 from the look point to the camera; the hit is the camera's
    // place the same frame, with no margin; the way out is the distance smoothing at 4 /s.
    let target = standing(0.0);
    let open = TestWorld::floor();
    let walled = TestWorld::floor().with_plane(Vec3::X, -200.0); // solid behind x = -200
    let mut camera = FieldCamera::new(CAMERA_PARAMS, &target);
    let free = steps(&mut camera, 5, CameraInput::default(), &target, &open);
    assert!(!free.collided && (free.distance - 400.0).abs() < 0.01);
    let hit = camera.step(DT, &CameraInput::default(), &target, &walled);
    assert!(hit.collided);
    assert!((hit.position.x - (-200.0 + 20.0)).abs() < 0.05, "camera centre at x = {} (wall -200, radius 20)", hit.position.x);
    let expected = 180.0 / 10.0f32.to_radians().cos();
    assert!((hit.distance - expected).abs() < 0.1, "distance {} (expected {expected})", hit.distance);
    assert_eq!(hit.rotation, free.rotation, "the sweep does not turn the camera");
    // Pressed against it every frame while it is there.
    let held = steps(&mut camera, 30, CameraInput::default(), &target, &walled);
    assert!((held.distance - expected).abs() < 0.1);
    // The wall goes: 6.67 % of the remaining gap per 60 Hz frame.
    let a = camera.step(DT, &CameraInput::default(), &target, &open);
    let b = camera.step(DT, &CameraInput::default(), &target, &open);
    assert!(((400.0 - a.distance) / (400.0 - held.distance) - (1.0 - 4.0 / 60.0)).abs() < 1.0e-3);
    assert!(((400.0 - b.distance) / (400.0 - a.distance) - (1.0 - 4.0 / 60.0)).abs() < 1.0e-3);
    let out = steps(&mut camera, 240, CameraInput::default(), &target, &open);
    assert!((out.distance - 400.0).abs() < 0.01);
}

#[test]
fn the_look_point_is_swept_from_the_pawn() {
    // Spec 3.1: a ceiling between his body and the look point above his head stops the look point.
    let world = TestWorld::floor().with_plane(Vec3::NEG_Z, -140.0); // solid above z = 140
    let target = standing(0.0);
    let mut camera = FieldCamera::new(CAMERA_PARAMS, &target);
    let view = steps(&mut camera, 3, CameraInput::default(), &target, &world);
    assert!((view.look_point.z - (140.0 - 21.0)).abs() < 0.1, "look point at z = {} (ceiling 140, sphere 21)", view.look_point.z);
}

#[test]
fn the_wall_avoiding_rotation_turns_the_camera_round_an_obstacle() {
    // Spec 2.7: while he moves (not along the view axis) and the 40 cm path probe from the look
    // point to the camera has to slide along something, the target rotation is turned toward the
    // probe's end.
    let wall = TestWorld::floor().with_plane(Vec3::X, -150.0); // a wall behind the start
    let run = |world: &TestWorld| {
        let mut runner = Runner::new(Vec3::new(0.0, 400.0, 0.0)); // along the wall
        runner.target.yaw = 90.0;
        let mut camera = FieldCamera::new(CAMERA_PARAMS, &CameraTarget { yaw: 0.0, ..runner.target });
        // Hold the automatic follow's own pull off the yaw: it only acts from 1.5 s after a stick
        // frame, the avoidance needs "followed" or a moved look point.
        let mut view = camera.view();
        for _ in 0..45 {
            view = camera.step(DT, &CameraInput::default(), &runner.advance(), world);
        }
        view
    };
    let (with_wall, open) = (run(&wall), run(&TestWorld::floor()));
    assert!(with_wall.rotation.yaw > open.rotation.yaw + 5.0, "yaw {} with the wall, {} without", with_wall.rotation.yaw, open.rotation.yaw);
    assert!(with_wall.position.x > -150.0 + 19.9);
}

#[test]
fn the_controller_reads_the_previous_updates_yaw() {
    // Spec 1.4.
    let world = TestWorld::floor();
    let target = standing(0.0);
    let mut camera = FieldCamera::new(CAMERA_PARAMS, &target);
    let mut previous = camera.view();
    for _ in 0..20 {
        assert_eq!(camera.input_yaw(), previous.rotation.yaw);
        previous = camera.step(DT, &stick(1.0, 0.0), &target, &world);
    }
}

#[test]
fn long_frames_are_cut_into_60_hz_slices() {
    // Spec 2.2 / 4: a frame of 2/60 s smooths like two 1/60 s frames.
    let world = TestWorld::default();
    let target = standing(0.0);
    let far = CameraTarget { yaw: 120.0, ..target };
    let mut a = FieldCamera::new(CAMERA_PARAMS, &target);
    let mut b = a.clone();
    a.step(DT, &CameraInput::default(), &target, &world);
    b.step(DT, &CameraInput::default(), &target, &world);
    a.reset(&far, false);
    b.reset(&far, false);
    let one = a.step(2.0 * DT, &CameraInput::default(), &target, &world);
    let keep = 1.0 - 8.0 / 60.0;
    assert!((one.rotation.yaw - 120.0 * (1.0 - keep * keep)).abs() < 1.0e-3, "yaw {}", one.rotation.yaw);
    // (Two separate frames differ only in that the second starts from the first's shown rotation: the same here.)
    b.step(DT, &CameraInput::default(), &target, &world);
    let two = b.step(DT, &CameraInput::default(), &target, &world);
    assert!((one.rotation.yaw - two.rotation.yaw).abs() < 1.0e-3);
}

#[test]
fn deterministic() {
    let world = TestWorld::floor().with_plane(Vec3::X, -300.0);
    let run = || {
        let mut runner = Runner::new(Vec3::new(300.0, 500.0, 0.0));
        let mut camera = FieldCamera::new(CAMERA_PARAMS, &runner.target);
        (0..300).map(|i| camera.step(DT, &if i % 50 < 10 { stick(0.9, -0.7) } else { CameraInput::default() }, &runner.advance(), &world)).collect::<Vec<_>>()
    };
    assert_eq!(run(), run());
}
