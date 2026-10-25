//! The shot camera of the port (kh_camera::shot: a port-side design, not game behaviour).
use glam::Vec3;
use kh_camera::shot::{self, ShotCamera, ShotPhase};
use kh_camera::{CameraTarget, CameraView, FieldCamera, TargetState, CAMERA_PARAMS};
use kh_core::test_world::TestWorld;

const DT: f32 = 1.0 / 60.0;

fn target() -> CameraTarget {
    CameraTarget { position: Vec3::new(0.0, 0.0, 2.15), yaw: 0.0, velocity: Vec3::ZERO, state: TargetState::Other, collision_half_height: 89.0 }
}

/// Degrees between where the view looks and the point.
fn off_axis(view: &CameraView, point: Vec3) -> f32 {
    view.rotation.forward().dot((point - view.position).normalize()).clamp(-1.0, 1.0).acos().to_degrees()
}

#[test]
fn close_shot_then_wide_shot_then_back_behind_him() {
    let world = TestWorld::floor();
    let t = target();
    let mut field = FieldCamera::new(CAMERA_PARAMS, &t);
    let under = field.view();
    let mut cam = ShotCamera::default();
    assert!(!cam.active() && cam.step(DT, &t, None, false, &world, &under) == under, "off: the camera underneath shows");
    cam.start(&under);
    assert!(cam.owns_view());
    // The first step is still close to the view it started from; after the blend it is the close
    // shot: in FRONT of him, low, looking at him.
    let first = cam.step(DT, &t, None, false, &world, &under);
    assert!((first.position - under.position).length() < 100.0, "{:?}", first.position);
    let mut view = first;
    for _ in 0..30 {
        view = cam.step(DT, &t, None, false, &world, &under);
    }
    let chest = t.position + Vec3::Z * shot::CLOSE_LOOK_HEIGHT;
    assert!(view.position.x > 100.0 && view.position.z < chest.z, "in front of him and below his chest: {:?}", view.position);
    assert!(off_axis(&view, chest) < 0.5 && (view.distance - 217.0).abs() < 12.0, "{} {}", off_axis(&view, chest), view.distance);
    // It orbits: half a second later it has moved round him at the same distance.
    let before = view.position;
    for _ in 0..30 {
        view = cam.step(DT, &t, None, false, &world, &under);
    }
    assert!((view.position - before).length() > 20.0 && cam.phase == ShotPhase::Close);
    // The volley: the wide shot frames him and the targets' centroid (both well inside the
    // app's 100 degree horizontal / 67 degree vertical view).
    let focus = Vec3::new(900.0, 300.0, 100.0);
    for _ in 0..30 {
        view = cam.step(DT, &t, Some(focus), true, &world, &under);
    }
    assert_eq!(cam.phase, ShotPhase::Wide);
    assert!(off_axis(&view, t.position + Vec3::Z * 100.0) < 30.0 && off_axis(&view, focus) < 30.0, "{} {}", off_axis(&view, t.position + Vec3::Z * 100.0), off_axis(&view, focus));
    assert!(view.position.z > 150.0 && (view.position - t.position).dot(focus - t.position) < 0.0, "above and behind him as seen from the targets: {:?}", view.position);
    // The end: the host puts the camera underneath behind him (the R3 reset, snapped); 0.5 s
    // later that camera shows, behind his facing at its default pitch.
    cam.end();
    field.reset(&t, true);
    assert!(cam.active() && !cam.owns_view());
    let mut under = field.view();
    for _ in 0..31 {
        under = field.step(DT, &Default::default(), &t, &world);
        view = cam.step(DT, &t, Some(focus), true, &world, &under);
    }
    assert!(!cam.active() && view == under);
    assert!(view.rotation.yaw.abs() < 1.0 && view.position.x < -100.0 && view.position.y.abs() < 20.0, "behind him: {:?} {:?}", view.position, view.rotation);
}

#[test]
fn the_level_keeps_the_shot_camera_out_of_a_wall() {
    // A wall 100 cm in front of him: the close shot (210 cm out) stops short of it.
    let world = TestWorld::floor().with_plane_through(Vec3::new(100.0, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0));
    let t = target();
    let like = FieldCamera::new(CAMERA_PARAMS, &t).view();
    let view = shot::close_shot(&t, 0.0, &world, &like);
    assert!(view.collided && view.position.x < 100.0 - shot::PROBE_RADIUS + 1.0, "{:?}", view.position);
    let open = shot::close_shot(&t, 0.0, &TestWorld::floor(), &like);
    assert!(!open.collided && open.position.x > 150.0);
}
