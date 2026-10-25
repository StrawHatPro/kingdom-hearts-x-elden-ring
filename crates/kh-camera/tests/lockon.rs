//! The lock-on camera (docs/sora-combat-spec.md 7.12): base rotation, stick scale, limits, the
//! automatic turn, the off-screen snap, the 0.5 s blends, the position step.

use glam::{Vec2, Vec3};
use kh_camera::lockon::{base_rotation, clamp_rotation, look_at, stick_scale, LockTarget, LockonCamera};
use kh_camera::rig::{ease_out, CameraRig};
use kh_camera::{target_point, CameraInput, CameraTarget, FieldCamera, Rot, TargetState, CAMERA_PARAMS};
use kh_core::test_world::TestWorld;

const DT: f32 = 1.0 / 60.0;

fn pawn(position: Vec3, yaw: f32, state: TargetState) -> CameraTarget {
    CameraTarget { position, yaw, velocity: Vec3::ZERO, state, collision_half_height: 89.0 }
}

/// Is `point` inside the frame of `view` (100 degrees horizontal, 16:9)?
fn in_frame(view: &kh_camera::CameraView, point: Vec3) -> bool {
    let forward = view.rotation.forward();
    let right = Rot::new(0.0, view.rotation.yaw + 90.0, 0.0).forward();
    let up = forward.cross(right);
    let d = point - view.position;
    let w = d.dot(forward);
    let half = 50.0f32.to_radians().tan();
    w > 0.0 && (d.dot(right) / (w * half)).abs() <= 1.0 && (d.dot(up) / (w * half * 9.0 / 16.0)).abs() <= 1.0
}

#[test]
fn base_rotation_looks_at_the_target_20_degrees_down_with_the_floor() {
    // 7.12 `base` (slot 299): look-at yaw (+ 0.08), pitch = look.pitch - 20, never below -10 when
    // the target is level or above; a target more than 10 degrees below: straight at it.
    let player = Vec3::new(0.0, 0.0, 150.0);
    let level = base_rotation(player, Vec3::new(500.0, 0.0, 150.0), 0.0);
    assert!((level.pitch - -10.0).abs() < 1.0e-4 && (level.yaw - 0.08).abs() < 1.0e-4, "{level:?}");
    let above = base_rotation(player, Vec3::new(500.0, 500.0, 650.0), 0.0);
    let elevation = (500.0f32).atan2(707.1068).to_degrees();
    assert!((above.pitch - (elevation - 20.0)).abs() < 1.0e-3 && (above.yaw - 45.08).abs() < 1.0e-3, "{above:?}");
    let below = base_rotation(player, Vec3::new(500.0, 0.0, -150.0), 0.0);
    assert!((below.pitch - (-300.0f32).atan2(500.0).to_degrees()).abs() < 1.0e-3, "straight at a target below: {below:?}");
    // Closer than 1 cm horizontally: pitch 0 and the actor's yaw.
    assert_eq!(look_at(player, player + Vec3::Z * 300.0, 33.0), Rot::new(0.0, 33.0, 0.0));
}

#[test]
fn the_stick_slows_with_the_distance_and_the_rotation_is_limited() {
    // Stick scale (slot 266): 1.0 within 500 cm, 0.45 beyond 2500, linear between.
    assert_eq!((stick_scale(100.0), stick_scale(500.0), stick_scale(2500.0), stick_scale(9000.0)), (1.0, 1.0, 0.45, 0.45));
    assert!((stick_scale(1500.0) - 0.725).abs() < 1.0e-5);
    // The clamp: pitch into [-70, min(L.pitch, 20) or 0], yaw into L.yaw +- 120.
    let (player, target) = (Vec3::new(0.0, 0.0, 150.0), Vec3::new(600.0, 0.0, 150.0));
    let c = clamp_rotation(Rot::new(-85.0, 170.0, 0.0), player, target, 0.0);
    assert!((c.pitch - -70.0).abs() < 1.0e-4 && (c.yaw - 120.08).abs() < 1.0e-3, "{c:?}");
    let c = clamp_rotation(Rot::new(35.0, -170.0, 0.0), player, target, 0.0);
    assert!(c.pitch.abs() < 1.0e-4 && (c.yaw - -119.92).abs() < 1.0e-3, "level target: no upward pitch: {c:?}");
    let high = clamp_rotation(Rot::new(35.0, 0.0, 0.0), player, Vec3::new(600.0, 0.0, 1350.0), 0.0);
    assert!((high.pitch - 20.0).abs() < 1.0e-4, "a high target: up to +20: {high:?}");
}

#[test]
fn the_camera_frames_sora_and_the_target_from_at_least_400() {
    let world = TestWorld::floor();
    let sora = pawn(Vec3::ZERO, 0.0, TargetState::Idle);
    for distance in [300.0f32, 800.0, 2000.0] {
        let target = Vec3::new(distance, 0.0, 90.0);
        let field = FieldCamera::new(CAMERA_PARAMS, &sora);
        let mut camera = LockonCamera::start(CAMERA_PARAMS, &sora, &LockTarget { look_at: target });
        let start = camera.view();
        // Start: the wanted rotation = base, the look point = the midpoint, distance 400.
        let player = target_point(&CAMERA_PARAMS, &sora);
        assert!((start.look_point - (player + target) * 0.5).length() < 1.0e-3 && (start.distance - 400.0).abs() < 1.0e-3);
        let mut view = start;
        for _ in 0..180 {
            view = camera.step(DT, &CameraInput::default(), &field, &sora, Some(&LockTarget { look_at: target }), &world);
        }
        // Third pass (the solver itself, 0x1405E0590): looking along the line to the target, the
        // far point is brought onto the near one's depth plane, so the pair is short on screen
        // and the fit asks for less than the minimum: the distance stays on m_Distance (400).
        // (The second pass used the solver's start value |T - P| x 0.5 / sin(50 deg) as the result.)
        assert!((view.distance - 400.0).abs() < 2.0, "{distance}: distance {}", view.distance);
        assert!(view.distance >= 399.9);
        assert!(in_frame(&view, player) && in_frame(&view, target), "{distance}: both in frame");
        // Yaw toward the target; the pitch: -10 (the floor), or straight at a target that is more
        // than 10 degrees below the player point (the dummy's centre at 300 cm: -11.4).
        assert!(view.rotation.yaw.abs() < 0.2 && view.rotation.pitch < -4.0 && view.rotation.pitch >= -11.5, "{:?}", view.rotation);
    }
    // Seen from the side (the stick has orbited the camera to 90 degrees off the line) the pair
    // lies across the screen and the fit asks for more than the minimum: 1.1 x (half + 50) / tan(50 deg).
    let (a, b) = (Vec3::new(0.0, 0.0, 150.0), Vec3::new(2000.0, 0.0, 150.0));
    let side = Rot::new(0.0, 90.0, 0.0);
    let screen = kh_camera::lockon::Screen { position: Vec3::new(1000.0, -1500.0, 150.0), rotation: side, fov: 100.0, aspect: 9.0 / 16.0 };
    let (look, distance) = kh_camera::lockon::frame2(a, b, side, &screen, (a + b) * 0.5);
    assert!((look - Vec3::new(1000.0, 0.0, 150.0)).length() < 0.5 && (distance - 1.1 * 1050.0 / 50.0f32.to_radians().tan()).abs() < 0.5, "{look:?} {distance}");
}

#[test]
fn the_stick_orbits_then_the_camera_drifts_back_faster_from_far_away() {
    let world = TestWorld::floor();
    // Idle: the follow velocity is 0, blended toward the reference speed by the distance.
    let drift = |distance: f32| {
        let sora = pawn(Vec3::ZERO, 0.0, TargetState::Idle);
        let target = Vec3::new(distance, 0.0, 150.338);
        let field = FieldCamera::new(CAMERA_PARAMS, &sora);
        let mut camera = LockonCamera::start(CAMERA_PARAMS, &sora, &LockTarget { look_at: target });
        let lock = LockTarget { look_at: target };
        // Half a second of full right stick.
        let stick = CameraInput { right_stick: Vec2::new(1.0, 0.0), ..CameraInput::default() };
        let mut view = camera.view();
        for _ in 0..30 {
            view = camera.step(DT, &stick, &field, &sora, Some(&lock), &world);
        }
        let turned = view.rotation_target.yaw;
        // 1.5 s without stick: nothing; then the automatic turn.
        for _ in 0..85 {
            view = camera.step(DT, &CameraInput::default(), &field, &sora, Some(&lock), &world);
        }
        let held = view.rotation_target.yaw;
        for _ in 0..60 {
            view = camera.step(DT, &CameraInput::default(), &field, &sora, Some(&lock), &world);
        }
        (turned, held, view.rotation_target.yaw)
    };
    let (near_turned, near_held, near_after) = drift(250.0);
    let (far_turned, far_held, far_after) = drift(3000.0);
    // The stick is slower far away: 0.45 of the near turn.
    assert!(near_turned.abs() > 20.0 && (far_turned / near_turned - 0.45).abs() < 0.02, "{near_turned} {far_turned}");
    assert_eq!((near_turned, far_turned), (near_held, far_held), "held for the 1.5 s stick wait");
    // Within 300 cm in Idle the camera does not turn by itself; at 3000 cm it returns.
    assert_eq!(near_after, near_held);
    assert!(far_after.abs() < far_held.abs() * 0.2, "{far_held} -> {far_after}");
}

#[test]
fn locking_and_unlocking_are_half_second_ease_out_blends_and_an_offscreen_target_snaps() {
    assert_eq!((ease_out(0.0, 2.0), ease_out(0.5, 2.0), ease_out(1.0, 2.0)), (0.0, 0.75, 1.0));
    let world = TestWorld::floor();
    let sora = pawn(Vec3::ZERO, 0.0, TargetState::Idle);
    // The target is behind him and to the side: the field camera looks along +X.
    let target = Vec3::new(-600.0, 300.0, 90.0);
    let mut rig = CameraRig::new(CAMERA_PARAMS, &sora);
    for _ in 0..10 {
        rig.step(DT, &CameraInput::default(), &sora, None, &world);
    }
    let before = rig.view();
    rig.set_lockon(&sora, Some((7, target)));
    let mut yaws = Vec::new();
    for _ in 0..40 {
        yaws.push(rig.step(DT, &CameraInput::default(), &sora, Some(target), &world).rotation.yaw);
    }
    // After 0.25 s the blend is 75 % done; after 0.5 s the lock-on camera has the view.
    let wanted = base_rotation(target_point(&CAMERA_PARAMS, &sora), target, 0.0).yaw;
    let part = kh_camera::normalize_axis(yaws[14] - before.rotation.yaw) / kh_camera::normalize_axis(wanted - before.rotation.yaw);
    assert!((part - 0.75).abs() < 0.03, "{part}");
    assert!((yaws[30] - wanted).abs() < 0.5 && rig.locked() == Some(7), "{} wanted {wanted}", yaws[30]);
    let locked = rig.view();
    assert!(in_frame(&locked, target) && in_frame(&locked, target_point(&CAMERA_PARAMS, &sora)));
    // The controller steers by the shown camera.
    assert_eq!(rig.input_yaw(), locked.rotation.yaw);
    // Unlock: back to the field camera in 0.5 s.
    rig.set_lockon(&sora, None);
    let mut last = locked;
    for _ in 0..31 {
        last = rig.step(DT, &CameraInput::default(), &sora, None, &world);
    }
    assert!(rig.locked().is_none() && (last.position - rig.field.view().position).length() < 1.0e-3);

    // The off-screen snap: the target jumps behind the camera: the wanted rotation is back on
    // `base` at once (not after the 1.5 s the automatic turn would need in Idle up close).
    let field = FieldCamera::new(CAMERA_PARAMS, &sora);
    let near = Vec3::new(250.0, 0.0, 150.0);
    let mut camera = LockonCamera::start(CAMERA_PARAMS, &sora, &LockTarget { look_at: near });
    for _ in 0..30 {
        camera.step(DT, &CameraInput::default(), &field, &sora, Some(&LockTarget { look_at: near }), &world);
    }
    let behind = Vec3::new(-250.0, 10.0, 150.0);
    let mut view = camera.view();
    // (A jump of more than 3000 cm/s is approached with rate 6: the aim point crosses over in
    // a few tenths of a second and leaves the frame on the way.)
    for _ in 0..60 {
        view = camera.step(DT, &CameraInput::default(), &field, &sora, Some(&LockTarget { look_at: behind }), &world);
    }
    // (From 400 cm behind the midpoint a 100-degree frame holds a near target at almost any yaw:
    // the snap only has to bring it back when it really left: it is in frame at the end.)
    assert!(in_frame(&view, behind), "{:?}", view.rotation_target);
}

#[test]
fn the_framing_solver_fits_the_lengthened_segment_into_the_frustum() {
    // 7.12 position step (third pass): 0x1405DEFA0 -> 0x1405E0590. Two points side by side at
    // the same depth, the camera square on: the look point is their middle and the distance is
    // 1.1 x (half their separation + 50) / tan(FOV / 2); a pair standing upright on screen needs
    // (half + 50) / tan(FOV / 2) / aspect.
    use kh_camera::lockon::{fit, frame2, Screen};
    let rot = Rot::new(0.0, 0.0, 0.0);
    let screen = Screen { position: Vec3::new(-1000.0, 0.0, 100.0), rotation: rot, fov: 100.0, aspect: 9.0 / 16.0 };
    let (a, b) = (Vec3::new(0.0, -300.0, 100.0), Vec3::new(0.0, 300.0, 100.0));
    let (look, distance) = fit(screen.position, a, b, rot, &screen);
    let across = 1.1 * 350.0 / 50.0f32.to_radians().tan();
    assert!((look - Vec3::new(0.0, 0.0, 100.0)).length() < 0.01 && (distance - across).abs() < 0.01, "{look:?} {distance} (expected {across})");
    let (a, b) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 600.0));
    let (look, distance) = fit(Vec3::new(-1000.0, 0.0, 300.0), a, b, rot, &Screen { position: Vec3::new(-1000.0, 0.0, 300.0), ..screen });
    let upright = 350.0 / 50.0f32.to_radians().tan() / (9.0 / 16.0);
    assert!((look.z - 300.0).abs() < 0.01 && (distance - upright).abs() < 0.05, "{look:?} {distance} (expected {upright})");
    // The whole solver, from an oblique angle: both points end up inside the frame.
    let rot = Rot::new(-15.0, 35.0, 0.0);
    let (a, b) = (Vec3::new(0.0, 0.0, 140.0), Vec3::new(900.0, 200.0, 260.0));
    let screen = Screen { position: (a + b) * 0.5 - rot.forward() * 600.0, rotation: rot, fov: 100.0, aspect: 9.0 / 16.0 };
    let (look, distance) = frame2(a, b, rot, &screen, (a + b) * 0.5);
    let view = Screen { position: look - rot.forward() * distance, ..screen };
    for point in [a, b] {
        let (s, front) = view.project(point);
        assert!(front && s.x.abs() <= 1.0 && s.y.abs() <= 1.0, "{point:?} at {s:?} from {distance}");
    }
}

#[test]
fn an_obstruction_pulls_the_camera_in_and_releases_over_0_6_s() {
    // 7.12 (0x14061BA10): a wall between the player point and the camera holds the camera in
    // front of it; without the wall the camera eases back out over 0.6 s.
    let sora = pawn(Vec3::new(0.0, 0.0, 2.15), 0.0, TargetState::Idle);
    let lock = LockTarget { look_at: Vec3::new(600.0, 0.0, 100.0) };
    let field = FieldCamera::new(CAMERA_PARAMS, &sora);
    let walled = TestWorld::floor().with_plane(Vec3::X, -200.0);
    let open = TestWorld::floor();
    let mut camera = LockonCamera::start(CAMERA_PARAMS, &sora, &lock);
    let mut view = camera.view();
    for _ in 0..60 {
        view = camera.step(DT, &CameraInput::default(), &field, &sora, Some(&lock), &walled);
    }
    assert!(view.collided && view.position.x > -200.0, "held in front of the wall: {:?}", view.position);
    let held = view.position;
    let mut free = LockonCamera::start(CAMERA_PARAMS, &sora, &lock);
    let mut free_view = free.view();
    for _ in 0..60 {
        free_view = free.step(DT, &CameraInput::default(), &field, &sora, Some(&lock), &open);
    }
    assert!(free_view.position.x < -250.0, "the free camera stands behind where the wall was: {:?}", free_view.position);
    // Released: after 0.3 s it is on its way, after 0.6 s it is where the free camera is.
    for _ in 0..18 {
        view = camera.step(DT, &CameraInput::default(), &field, &sora, Some(&lock), &open);
    }
    assert!(view.position.x < held.x - 5.0 && view.position.x > free_view.position.x + 5.0, "half way: {:?}", view.position);
    for _ in 0..30 {
        view = camera.step(DT, &CameraInput::default(), &field, &sora, Some(&lock), &open);
    }
    assert!((view.position - free_view.position).length() < 2.0, "{:?} vs {:?}", view.position, free_view.position);
}

// ------------------------------------------------------------------------------------ fourth combat pass (2026-10-12)

#[test]
fn the_interpolator_curves_by_type() {
    // 0x1411A0E70 (dispatch read in the disassembly): 2 sin(t pi/2) - the obstruction release -,
    // 3 1 - cos(t pi/2), 4 (1 - cos(t pi)) / 2, 5 1 - (1 - t)^2 - the attack camera's lag tweens -,
    // 6 t^2, else linear.
    use kh_camera::lockon::interp_curve;
    let near = |a: f32, b: f32| (a - b).abs() < 1.0e-5;
    assert!(near(interp_curve(2, 0.5), std::f32::consts::FRAC_1_SQRT_2) && near(interp_curve(3, 0.5), 1.0 - std::f32::consts::FRAC_1_SQRT_2));
    assert!(near(interp_curve(4, 0.25), (1.0 - std::f32::consts::FRAC_1_SQRT_2) * 0.5) && near(interp_curve(5, 0.5), 0.75) && near(interp_curve(6, 0.5), 0.25) && near(interp_curve(0, 0.3), 0.3));
    for kind in 0..8 {
        assert!(near(interp_curve(kind, 0.0), 0.0) && near(interp_curve(kind, 1.0), 1.0) && near(interp_curve(kind, 2.0), 1.0));
    }
}

#[test]
fn the_attack_camera_modes_on_the_normal_camera() {
    // Combat spec 7.12.1 (controller 0x1414122A0): mode 4 (the burn): look-point rate 2.0 and lag
    // limit 2000 on the normal camera and its lock-on camera, lock yaw limit 0; off: back at once.
    // Mode 5 (Buzz Saw, pole spin): distance scale 1.6. Mode 6 off: the lag returns over 1.0 s.
    use kh_camera::rig::{ATTACK_CAM_BURN_LAG, ATTACK_CAM_WIDE_SCALE};
    let sora = pawn(Vec3::new(0.0, 0.0, 2.15), 0.0, TargetState::Idle);
    let world = TestWorld::floor();
    let mut rig = CameraRig::new(CAMERA_PARAMS, &sora);
    let defaults = (CAMERA_PARAMS.look_lerp_rate, CAMERA_PARAMS.look_lag_limit);
    assert_eq!(defaults, (8.0, 100.0));
    rig.set_attack_mode(4, -1.0);
    assert_eq!(((rig.field.params().look_lerp_rate, rig.field.params().look_lag_limit), rig.attack_camera().mode), (ATTACK_CAM_BURN_LAG, 4));
    // The look point now trails a pawn that runs off: after 0.5 s at 1000 cm/s it is far behind
    // (the default camera keeps it within its 100 cm lag limit).
    let lag_after = |rig: &mut CameraRig| {
        let mut view = rig.view();
        for n in 1..=30 {
            let moved = CameraTarget { velocity: Vec3::new(1000.0, 0.0, 0.0), ..pawn(Vec3::new(1000.0 * DT * n as f32, 0.0, 2.15), 0.0, TargetState::Run) };
            view = rig.step(DT, &CameraInput::default(), &moved, None, &world);
        }
        let moved = pawn(Vec3::new(500.0, 0.0, 2.15), 0.0, TargetState::Run);
        (target_point(&CAMERA_PARAMS, &moved) - view.look_point).length()
    };
    let burning = lag_after(&mut rig);
    let mut plain = CameraRig::new(CAMERA_PARAMS, &sora);
    let normal = lag_after(&mut plain);
    assert!(normal <= 100.5 && burning > 250.0, "lag {normal} cm normally, {burning} cm in mode 4");
    // A lock-on camera started in mode 4 has no yaw limit: an orbit of 170 degrees stays.
    rig.set_lockon(&sora, Some((1, Vec3::new(600.0, 0.0, 100.0))));
    assert!(rig.lock.is_some());
    rig.set_attack_mode(0, -1.0);
    assert_eq!((rig.field.params().look_lerp_rate, rig.field.params().look_lag_limit), defaults, "off: the class defaults at once");
    // Mode 5: the distance scale; the camera pulls back to 1.6 x its distance.
    let mut rig = CameraRig::new(CAMERA_PARAMS, &sora);
    let mut near_view = rig.view();
    for _ in 0..240 {
        near_view = rig.step(DT, &CameraInput::default(), &sora, None, &world);
    }
    rig.set_attack_mode(5, -1.0);
    assert_eq!(rig.field.params().distance_scale, ATTACK_CAM_WIDE_SCALE);
    let mut wide = near_view;
    for _ in 0..240 {
        wide = rig.step(DT, &CameraInput::default(), &sora, None, &world);
    }
    assert!((wide.distance / near_view.distance - 1.6).abs() < 0.02, "{} -> {}", near_view.distance, wide.distance);
    rig.set_attack_mode(0, -1.0);
    assert_eq!(rig.field.params().distance_scale, 1.0);
    // Mode 6 off (restore 1.0 s): interpolator type 5, 1 - (1 - t)^2: three quarters back at 0.5 s.
    let mut rig = CameraRig::new(CAMERA_PARAMS, &sora);
    rig.set_attack_mode(6, -1.0);
    rig.set_attack_mode(0, 1.0);
    for _ in 0..30 {
        rig.step(DT, &CameraInput::default(), &sora, None, &world);
    }
    let rate = rig.field.params().look_lerp_rate;
    assert!((rate - (2.0 + (8.0 - 2.0) * 0.75)).abs() < 0.05, "{rate}");
    for _ in 0..31 {
        rig.step(DT, &CameraInput::default(), &sora, None, &world);
    }
    assert_eq!((rig.field.params().look_lerp_rate, rig.field.params().look_lag_limit), defaults);
}

#[test]
fn the_lock_yaw_limit_of_zero_switches_the_clamp_off() {
    use kh_camera::lockon::clamp_rotation_limited;
    let (player, target) = (Vec3::new(0.0, 0.0, 150.0), Vec3::new(600.0, 0.0, 150.0));
    assert!((clamp_rotation_limited(Rot::new(-10.0, 170.0, 0.0), player, target, 0.0, 120.0).yaw - 120.0).abs() < 0.2);
    assert!((clamp_rotation_limited(Rot::new(-10.0, 170.0, 0.0), player, target, 0.0, 0.0).yaw - 170.0).abs() < 1.0e-3);
}
