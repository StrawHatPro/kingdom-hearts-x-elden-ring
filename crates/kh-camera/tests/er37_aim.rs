//! (IMPL_ER37, user request) The shotlock aim starts on the lock-on target remembered at entry; without one, behind
//! Sora as KH3 (StartTresCamera 0x14060CA80).
use glam::{Vec2, Vec3};
use kh_camera::aim::{start_point, AimCamera, AimOverlay};
use kh_camera::{CameraTarget, FieldCamera, TargetState, CAMERA_PARAMS};
use kh_core::test_world::TestWorld;

fn target() -> CameraTarget {
    CameraTarget { position: Vec3::new(0.0, 0.0, 2.15), yaw: 30.0, velocity: Vec3::ZERO, state: TargetState::Idle, collision_half_height: 89.0 }
}

/// Distance of `p` from the screen-centre ray of a view (camera position along its forward).
fn off_centre(view: &kh_camera::CameraView, p: Vec3) -> f32 {
    let f = view.rotation.forward();
    let d = p - view.position;
    assert!(d.dot(f) > 0.0, "the point is behind the camera");
    (d - f * d.dot(f)).length()
}

#[test]
fn a_target_at_an_angle_is_under_the_reticle_at_entry() {
    let world = TestWorld::floor();
    let t = target();
    let under = FieldCamera::new(CAMERA_PARAMS, &t).view();
    // Enemies left, right, behind, above and below Sora's facing (30), near and far: the lock-on points.
    for point in [Vec3::new(800.0, -900.0, 120.0), Vec3::new(-600.0, 300.0, 90.0), Vec3::new(200.0, 1500.0, 900.0), Vec3::new(3000.0, 200.0, -400.0), Vec3::new(-250.0, -150.0, 100.0)] {
        let aim = AimCamera::start_toward(&t, Some(point), &under);
        let view = aim.view();
        assert!(off_centre(&view, point) < 1.0, "{point:?}: {} cm off the reticle, rotation {:?}", off_centre(&view, point), aim.rotation);
        assert_eq!(aim.rotation, aim.rotation_target, "no swing after entry");
        // It stays there with the sticks at rest.
        let mut aim = aim;
        for _ in 0..30 {
            aim.step(1.0 / 60.0, Vec2::ZERO, Vec2::ZERO, &t, &world);
        }
        assert!(off_centre(&aim.view(), point) < 1.0);
    }
    // Pitch inside the aim's limits even straight up.
    let aim = AimCamera::start_toward(&t, Some(Vec3::new(10.0, 0.0, 5000.0)), &under);
    assert!(aim.rotation.pitch <= 70.0);
}

#[test]
fn without_a_target_it_starts_behind_sora_as_before() {
    let t = target();
    let under = FieldCamera::new(CAMERA_PARAMS, &t).view();
    assert_eq!(AimCamera::start_toward(&t, None, &under), AimCamera::start(&t, &under));
    assert_eq!(AimCamera::start_toward(&t, Some(Vec3::NAN), &under), AimCamera::start(&t, &under));
    let (mut a, mut b) = (AimOverlay::default(), AimOverlay::default());
    a.open(&t, &under);
    b.open_toward(&t, &under, None);
    assert_eq!(a, b);
    let mut c = AimOverlay::default();
    c.open_toward(&t, &under, Some(Vec3::new(-600.0, 300.0, 90.0)));
    assert!(off_centre(&c.camera.unwrap().view(), Vec3::new(-600.0, 300.0, 90.0)) < 1.0);
}

/// (IMPL_ER42) With no hard lock the aim starts on the soft target, with the same reticle placement; a hard lock wins over
/// the soft target; neither = behind Sora.
#[test]
fn the_soft_target_is_used_without_a_hard_lock_and_a_hard_lock_wins() {
    let t = target();
    let under = FieldCamera::new(CAMERA_PARAMS, &t).view();
    let (hard, soft) = (Vec3::new(800.0, -900.0, 120.0), Vec3::new(-600.0, 300.0, 90.0));
    assert_eq!(start_point(None, Some(soft)), Some(soft));
    assert_eq!(start_point(Some(hard), Some(soft)), Some(hard));
    assert_eq!(start_point(Some(hard), None), Some(hard));
    assert_eq!(start_point(None, None), None);
    assert_eq!(start_point(Some(Vec3::NAN), Some(soft)), Some(soft), "a broken hard point falls to the soft one");
    // Soft only: the soft target is under the reticle, exactly as a hard lock on it would be.
    let mut a = AimOverlay::default();
    a.open_toward(&t, &under, start_point(None, Some(soft)));
    assert!(off_centre(&a.camera.unwrap().view(), soft) < 1.0);
    let mut as_hard = AimOverlay::default();
    as_hard.open_toward(&t, &under, start_point(Some(soft), None));
    assert_eq!(a, as_hard);
    // Both: the hard lock is under the reticle, not the soft target.
    let mut b = AimOverlay::default();
    b.open_toward(&t, &under, start_point(Some(hard), Some(soft)));
    let view = b.camera.unwrap().view();
    assert!(off_centre(&view, hard) < 1.0);
    let to_soft = (soft - view.position).normalize();
    assert!(to_soft.dot(view.rotation.forward()) < 0.9, "the soft target is not under the reticle");
    // Neither: behind Sora as KH3.
    let (mut c, mut d) = (AimOverlay::default(), AimOverlay::default());
    c.open_toward(&t, &under, start_point(None, None));
    d.open(&t, &under);
    assert_eq!(c, d);
}
