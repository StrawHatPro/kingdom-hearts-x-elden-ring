//! The matrices between the three spaces of the renderer. Pure; unit-tested.
//!
//! - PACK space: the glTF files of the Sora pack. Metres, +Y up, right-handed, the model faces +Z.
//! - GAME space: KH3's world as `kh-core` simulates it. Centimetres, X forward, Y right, Z up,
//!   left-handed. The translated material shaders work in it.
//! - HOST space: Elden Ring's physics / camera space. Metres, X right, Y up, Z forward,
//!   left-handed (measured: IMPL_ER1_STATE.md). `origin` is where KH3's origin lies in it.
//!
//! Matrices are glam's (column vectors: `M * v`), which is also how the WGSL writes them.
//!
//! Clip space is Direct3D's: x right, y up, z in 0..1, and here REVERSED-Z (near plane = 1, far
//! plane = 0), which is what Elden Ring's own depth buffer holds (measured: IMPL_ER2_STATE.md).

use glam::{Mat4, Vec3, Vec4};

/// The camera of the host, as Elden Ring's `CSCam` gives it: the rows of its matrix and the lens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
    pub position: Vec3,
    /// Radians. VERTICAL field of view (seen: the mesh overlays the game's debug skeleton).
    pub fov: f32,
    /// Width / height.
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

/// HOST world -> view (x right, y up, z forward: left-handed, as the host).
pub fn view_from_host(c: &Camera) -> Mat4 {
    Mat4::from_cols(
        Vec4::new(c.right.x, c.up.x, c.forward.x, 0.0),
        Vec4::new(c.right.y, c.up.y, c.forward.y, 0.0),
        Vec4::new(c.right.z, c.up.z, c.forward.z, 0.0),
        Vec4::new(-c.right.dot(c.position), -c.up.dot(c.position), -c.forward.dot(c.position), 1.0),
    )
}

/// View -> clip: a left-handed perspective with reversed Z (depth 1 at `near`, 0 at `far`),
/// `fov_y` the vertical field of view in radians. `w` is the view depth.
pub fn clip_from_view(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let f = 1.0 / (fov_y * 0.5).tan();
    let range = far - near;
    Mat4::from_cols(
        Vec4::new(f / aspect, 0.0, 0.0, 0.0),
        Vec4::new(0.0, f, 0.0, 0.0),
        Vec4::new(0.0, 0.0, -near / range, 1.0),
        Vec4::new(0.0, 0.0, near * far / range, 0.0),
    )
}

/// The two numbers that turn a reversed-Z device depth back into a view depth:
/// `view_z = 1 / (device_z * a + b)`.
pub fn depth_to_view(near: f32, far: f32) -> (f32, f32) {
    ((far - near) / (near * far), 1.0 / far)
}

/// GAME (cm) -> HOST (m): `host = origin + (game.y, game.z, game.x) / 100`.
pub fn host_from_game(origin: Vec3) -> Mat4 {
    Mat4::from_cols(Vec4::new(0.0, 0.0, 0.01, 0.0), Vec4::new(0.01, 0.0, 0.0, 0.0), Vec4::new(0.0, 0.01, 0.0, 0.0), origin.extend(1.0))
}

/// A HOST direction in GAME axes (unit vectors stay unit).
pub fn game_dir_of_host(v: Vec3) -> Vec3 {
    Vec3::new(v.z, v.x, v.y)
}

/// A HOST position in GAME space (cm).
pub fn game_pos_of_host(p: Vec3, origin: Vec3) -> Vec3 {
    game_dir_of_host(p - origin) * 100.0
}

/// PACK -> the mesh component's space (cm, Z up, the model faces +Y): `(x, z, y) * 100`.
pub fn component_from_pack() -> Mat4 {
    Mat4::from_cols(Vec4::new(100.0, 0.0, 0.0, 0.0), Vec4::new(0.0, 0.0, 100.0, 0.0), Vec4::new(0.0, 100.0, 0.0, 0.0), Vec4::W)
}

/// A transform of `kh-anim` (scale, then rotation, then translation) as a matrix.
pub fn matrix_of(t: &kh_anim::Trs) -> Mat4 {
    Mat4::from_scale_rotation_translation(t.scale, t.rotation, t.translation)
}

/// PACK -> GAME for a mesh whose component stands at `component_to_world`.
pub fn game_from_pack(component_to_world: &kh_anim::Trs) -> Mat4 {
    matrix_of(component_to_world) * component_from_pack()
}

/// The joint matrix of a bone in PACK space from its component-space pose: what the skin's
/// inverse bind matrix is multiplied with (the pack's nodes carry exactly these transforms).
pub fn joint_in_pack(component: &kh_anim::Trs) -> Mat4 {
    matrix_of(&kh_anim::pack::trs_to_pack(component))
}

/// GAME -> clip for a camera of the host.
pub fn clip_from_game(camera: &Camera, origin: Vec3) -> Mat4 {
    clip_from_view(camera.fov, camera.aspect, camera.near, camera.far) * view_from_host(camera) * host_from_game(origin)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Quat;

    fn camera() -> Camera {
        // Looking along host +Z from (10, 2, -5), level.
        Camera { right: Vec3::X, up: Vec3::Y, forward: Vec3::Z, position: Vec3::new(10.0, 2.0, -5.0), fov: 0.8378, aspect: 16.0 / 9.0, near: 0.05, far: 10000.0 }
    }

    fn ndc(m: Mat4, p: Vec3) -> Vec3 {
        let c = m * p.extend(1.0);
        c.truncate() / c.w
    }

    #[test]
    fn the_view_is_left_handed_with_z_forward() {
        let c = camera();
        let v = view_from_host(&c);
        let p = v.transform_point3(c.position + c.forward * 3.0 + c.right * 1.0 + c.up * 0.5);
        assert!((p - Vec3::new(1.0, 0.5, 3.0)).length() < 1.0e-5);
        // A turned camera: rows right / up / forward of a yaw of 90 degrees (forward = +X).
        let turned = Camera { right: Vec3::NEG_Z, up: Vec3::Y, forward: Vec3::X, ..c };
        assert!((turned.right.cross(turned.up) - turned.forward).length() < 1.0e-6, "right x up = forward (left-handed)");
        let p = view_from_host(&turned).transform_point3(turned.position + Vec3::X * 4.0);
        assert!((p - Vec3::new(0.0, 0.0, 4.0)).length() < 1.0e-5);
    }

    #[test]
    fn the_projection_is_vertical_fov_reversed_z() {
        let c = camera();
        let m = clip_from_view(c.fov, c.aspect, c.near, c.far);
        // Centre of the picture, depth 1 at the near plane and 0 at the far plane.
        assert!((ndc(m, Vec3::new(0.0, 0.0, c.near)) - Vec3::new(0.0, 0.0, 1.0)).length() < 1.0e-5);
        assert!(ndc(m, Vec3::new(0.0, 0.0, c.far)).z.abs() < 1.0e-6);
        // A point at the top edge of the vertical field of view lands on y = +1 (up = up);
        // one at the right edge of the horizontal field on x = +1.
        let t = (c.fov * 0.5).tan();
        let top = ndc(m, Vec3::new(0.0, t * 7.0, 7.0));
        let right = ndc(m, Vec3::new(t * c.aspect * 7.0, 0.0, 7.0));
        assert!((top.y - 1.0).abs() < 1.0e-5 && (right.x - 1.0).abs() < 1.0e-5);
        // w is the view depth, and the device depth converts back to it.
        let clip = m * Vec4::new(0.3, -0.2, 12.5, 1.0);
        assert!((clip.w - 12.5).abs() < 1.0e-6);
        let (a, b) = depth_to_view(c.near, c.far);
        let device = clip.z / clip.w;
        assert!((1.0 / (device * a + b) - 12.5).abs() < 1.0e-2, "{}", 1.0 / (device * a + b));
        // Nearer = larger depth.
        assert!(ndc(m, Vec3::new(0.0, 0.0, 2.0)).z > ndc(m, Vec3::new(0.0, 0.0, 3.0)).z);
    }

    #[test]
    fn game_and_host_axes() {
        let origin = Vec3::new(5.0, 1.0, -7.0);
        let m = host_from_game(origin);
        // KH3 forward (+X, 100 cm) is host +Z (1 m), right (+Y) is +X, up (+Z) is +Y.
        assert!((m.transform_point3(Vec3::new(100.0, 0.0, 0.0)) - (origin + Vec3::Z)).length() < 1.0e-5);
        assert!((m.transform_point3(Vec3::new(0.0, 100.0, 0.0)) - (origin + Vec3::X)).length() < 1.0e-5);
        assert!((m.transform_point3(Vec3::new(0.0, 0.0, 100.0)) - (origin + Vec3::Y)).length() < 1.0e-5);
        let p = Vec3::new(12.0, 3.0, 40.0);
        assert!((m.transform_point3(game_pos_of_host(p, origin)) - p).length() < 1.0e-4);
        assert_eq!(game_dir_of_host(Vec3::Z), Vec3::X);
        // Both left-handed: no mirror between them.
        assert!(m.determinant() > 0.0);
    }

    #[test]
    fn the_pack_model_stands_up_and_faces_the_yaw() {
        // The pack's model faces pack +Z with +Y up; the component faces +Y with Z up; the pawn
        // turns the component by yaw - 90 degrees, so at yaw 0 the model faces game +X.
        let yaw = 0.0f32;
        let l2w = kh_anim::Trs { translation: Vec3::new(1000.0, 2000.0, 50.0), rotation: Quat::from_rotation_z((yaw - 90.0).to_radians()), scale: Vec3::ONE };
        let m = game_from_pack(&l2w);
        let head = m.transform_point3(Vec3::new(0.0, 1.5, 0.0));
        let nose = m.transform_point3(Vec3::new(0.0, 1.5, 0.1));
        assert!((head - Vec3::new(1000.0, 2000.0, 200.0)).length() < 1.0e-2, "{head:?}");
        assert!((nose - head - Vec3::new(10.0, 0.0, 0.0)).length() < 1.0e-2, "{:?}", nose - head);
        // pack (right-handed) -> game (left-handed) is a mirror: the winding turns round.
        assert!(m.determinant() < 0.0);
        // A joint at a component-space pose: the pack-space matrix puts the bone's origin where
        // the pose says, in pack units.
        let bone = kh_anim::Trs { translation: Vec3::new(10.0, 20.0, 150.0), rotation: Quat::IDENTITY, scale: Vec3::ONE };
        assert!((joint_in_pack(&bone).transform_point3(Vec3::ZERO) - Vec3::new(0.1, 1.5, 0.2)).length() < 1.0e-5);
        assert!((component_from_pack().transform_point3(Vec3::new(0.1, 1.5, 0.2)) - bone.translation).length() < 1.0e-3);
    }

    #[test]
    fn a_point_in_front_of_the_camera_projects_to_the_centre() {
        let c = camera();
        let origin = Vec3::new(3.0, 0.0, 9.0);
        let host = c.position + c.forward * 6.0;
        let game = game_pos_of_host(host, origin);
        let n = ndc(clip_from_game(&c, origin), game);
        assert!(n.x.abs() < 1.0e-4 && n.y.abs() < 1.0e-4 && n.z > 0.0 && n.z < 1.0, "{n:?}");
    }
}
