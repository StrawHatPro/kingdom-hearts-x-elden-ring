//! The shotlock aim camera (docs/sora-magic-spec.md part F A5).
use glam::{Vec2, Vec3};
use kh_camera::aim::{AimCamera, AimOverlay};
use kh_camera::{CameraTarget, FieldCamera, TargetState, CAMERA_PARAMS};
use kh_core::test_world::TestWorld;

fn target() -> CameraTarget {
    CameraTarget { position: Vec3::new(0.0, 0.0, 2.15), yaw: 30.0, velocity: Vec3::ZERO, state: TargetState::Idle, collision_half_height: 89.0 }
}

#[test]
fn geometry_start_and_stick() {
    let world = TestWorld::floor();
    let t = target();
    let field = FieldCamera::new(CAMERA_PARAMS, &t);
    let mut aim = AimCamera::start(&t, &field.view());
    // Behind Sora at pitch -10, whatever the field camera showed.
    assert_eq!((aim.rotation.pitch, aim.rotation.yaw), (-10.0, 30.0));
    let v = aim.view();
    // 150 cm behind and 100 cm right of the point 152.3 above his origin, along the view axes.
    let (f, yaw) = (v.rotation.forward(), 30.0f32.to_radians());
    let right = Vec3::new(-yaw.sin(), yaw.cos(), 0.0);
    let p = t.position + Vec3::Z * 152.338;
    assert!((v.position - (p - f * 150.0 + right * 100.0)).length() < 1.0e-3, "{:?}", v.position);
    // Right stick full right: 120 degrees / s on the target rotation; no sensitivity setting.
    for _ in 0..60 {
        aim.step(1.0 / 60.0, Vec2::new(1.0, 0.0), Vec2::ZERO, &t, &world);
    }
    assert!((aim.rotation_target.yaw - 150.0).abs() < 0.01, "{}", aim.rotation_target.yaw);
    // The left stick turns the view while the right stick is at rest; up = look up; +-70.
    for _ in 0..120 {
        aim.step(1.0 / 60.0, Vec2::ZERO, Vec2::new(0.0, 1.0), &t, &world);
    }
    assert_eq!(aim.rotation_target.pitch, 70.0);
    // Below the dead zones nothing moves (0.25 engine, then 0.1).
    let before = aim.rotation_target;
    aim.step(1.0 / 60.0, Vec2::new(0.3, 0.0), Vec2::ZERO, &t, &world);
    assert_eq!(aim.rotation_target, before);
}

#[test]
fn overlay_blends_in_half_a_second() {
    let world = TestWorld::floor();
    let t = target();
    let field = FieldCamera::new(CAMERA_PARAMS, &t);
    let under = field.view();
    let mut overlay = AimOverlay::default();
    assert_eq!(overlay.step(1.0 / 60.0, Vec2::ZERO, Vec2::ZERO, &t, &world, &under), under);
    overlay.open(&t, &under);
    let aim = overlay.camera.unwrap().view();
    let first = overlay.step(1.0 / 60.0, Vec2::ZERO, Vec2::ZERO, &t, &world, &under);
    assert!((first.position - under.position).length() > 0.0 && (first.position - aim.position).length() > 1.0);
    for _ in 0..30 {
        overlay.step(1.0 / 60.0, Vec2::ZERO, Vec2::ZERO, &t, &world, &under);
    }
    let full = overlay.step(1.0 / 60.0, Vec2::ZERO, Vec2::ZERO, &t, &world, &under);
    assert!((full.position - aim.position).length() < 1.0e-3);
    overlay.close();
    for _ in 0..31 {
        overlay.step(1.0 / 60.0, Vec2::ZERO, Vec2::ZERO, &t, &world, &under);
    }
    assert_eq!(overlay.step(1.0 / 60.0, Vec2::ZERO, Vec2::ZERO, &t, &world, &under), under);
}
