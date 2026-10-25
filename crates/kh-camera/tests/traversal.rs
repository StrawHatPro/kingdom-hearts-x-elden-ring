//! The field camera's player-state branches and the Camera Work option (integration pass,
//! 2026-10-12): docs/sora-camera-spec.md section 5 rows 3 and 20, 2.6.1; decompiled C of the base
//! rotation source 0x1406068F0 and of the distance slot 290 0x140606390.

use kh_camera::{CameraInput, CameraTarget, FieldCamera, TargetState, CAMERA_PARAMS};
use kh_core::test_world::TestWorld;
use kh_core::{Vec2, Vec3};

const DT: f32 = 1.0 / 60.0;

fn moving(state: TargetState, velocity: Vec3, yaw: f32) -> CameraTarget {
    CameraTarget { position: Vec3::new(0.0, 0.0, 0.15), yaw, velocity, state, collision_half_height: 89.0 }
}

/// Steps the camera for `seconds` behind a pawn that moves with its velocity.
fn follow(camera: &mut FieldCamera, mut target: CameraTarget, seconds: f32, input: CameraInput) -> kh_camera::CameraView {
    let world = TestWorld::floor();
    let mut view = camera.view();
    for _ in 0..(seconds / DT) as usize {
        target.position += target.velocity * DT;
        view = camera.step(DT, &input, &target, &world);
    }
    view
}

#[test]
fn camera_work_manual_switches_the_automatic_follow_off() {
    // He runs to the camera's right: with Auto the yaw is pulled round behind him (2.6), with
    // Manual it stays where the player left it.
    let run = moving(TargetState::Run, Vec3::new(0.0, 600.0, 0.0), 90.0);
    let start = moving(TargetState::Idle, Vec3::ZERO, 0.0);
    let mut auto = FieldCamera::new(CAMERA_PARAMS, &start);
    assert!(auto.auto_camera(), "the shipped default is Auto");
    let turned = follow(&mut auto, run, 4.0, CameraInput::default());
    assert!(turned.rotation.yaw > 30.0 && turned.auto_power > 0.0, "auto: yaw {}", turned.rotation.yaw);
    let mut manual = FieldCamera::new(CAMERA_PARAMS, &start);
    manual.set_auto_camera(false);
    let still = follow(&mut manual, run, 4.0, CameraInput::default());
    assert_eq!((still.rotation.yaw, still.rotation.pitch, still.auto_power), (0.0, -10.0, 0.0));
    // The stick still turns it.
    let by_hand = follow(&mut manual, run, 0.5, CameraInput { right_stick: Vec2::new(1.0, 0.0), ..CameraInput::default() });
    assert!(by_hand.rotation.yaw.abs() > 10.0);
}

#[test]
fn a_wall_run_takes_base_pitch_0_yaw_40_and_distance_700() {
    // Up the wall (class 0): the distance goes to m_WallDistance 700 (the camera is at or above the base pitch -10).
    let up = moving(TargetState::WallRun(0), Vec3::new(0.0, 0.0, 1200.0), 0.0);
    let mut camera = FieldCamera::new(CAMERA_PARAMS, &moving(TargetState::Idle, Vec3::ZERO, 0.0));
    let view = follow(&mut camera, up, 3.0, CameraInput::default());
    assert!((view.distance - 700.0).abs() < 5.0, "distance {}", view.distance);
    // (No horizontal speed: the automatic follow has no pull, 2.6, so the rotation stays.)
    // Along the wall: the base yaw is his yaw +40 (class 1) / -40 (class 2).
    for (class, sign) in [(1u8, 1.0f32), (2, -1.0)] {
        let along = moving(TargetState::WallRun(class), Vec3::new(600.0, 0.0, 0.0), 0.0);
        let mut camera = FieldCamera::new(CAMERA_PARAMS, &moving(TargetState::Idle, Vec3::ZERO, 0.0));
        let view = follow(&mut camera, along, 6.0, CameraInput::default());
        assert!((view.rotation_target.yaw - sign * 40.0).abs() < 6.0, "class {class}: yaw {}", view.rotation_target.yaw);
        assert!((view.rotation_target.pitch).abs() < 3.0, "class {class}: pitch {}", view.rotation_target.pitch);
    }
    // The pitch limit is 60 in a wall run (50 otherwise): the stick held up for 3 s.
    let stick = CameraInput { right_stick: Vec2::new(0.0, -1.0), ..CameraInput::default() };
    let other = CameraInput { right_stick: Vec2::new(0.0, 1.0), ..CameraInput::default() };
    let mut limits = Vec::new();
    for state in [TargetState::WallRun(0), TargetState::Other] {
        let mut highest = f32::MIN;
        for input in [stick, other] {
            let mut camera = FieldCamera::new(CAMERA_PARAMS, &moving(TargetState::Idle, Vec3::ZERO, 0.0));
            highest = highest.max(follow(&mut camera, moving(state, Vec3::ZERO, 0.0), 3.0, input).rotation_target.pitch);
        }
        limits.push(highest);
    }
    assert_eq!(limits, vec![60.0, 50.0]);
}

#[test]
fn a_rail_slide_takes_base_pitch_5() {
    let rail = moving(TargetState::RailSlide, Vec3::new(2000.0, 0.0, 0.0), 0.0);
    let mut camera = FieldCamera::new(CAMERA_PARAMS, &moving(TargetState::Idle, Vec3::ZERO, 0.0));
    let view = follow(&mut camera, rail, 5.0, CameraInput::default());
    assert!((view.rotation_target.pitch - 5.0).abs() < 1.0, "pitch {}", view.rotation_target.pitch);
    // The same run in the Jump / other state returns to -10.
    let mut camera = FieldCamera::new(CAMERA_PARAMS, &moving(TargetState::Idle, Vec3::ZERO, 0.0));
    let view = follow(&mut camera, moving(TargetState::Other, Vec3::new(2000.0, 0.0, 0.0), 0.0), 5.0, CameraInput::default());
    assert!((view.rotation_target.pitch + 10.0).abs() < 1.0, "pitch {}", view.rotation_target.pitch);
}
