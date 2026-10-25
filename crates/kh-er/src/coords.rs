//! THE one place where KH3 space and Elden Ring space meet.
//!
//! KH3 (`kh-core`, UE4): centimetres, degrees; X forward, Y right, Z up; left-handed; yaw about Z
//! from +X toward +Y.
//!
//! Elden Ring physics space ("Havok position"): metres; X right, Y up, Z forward; left-handed
//! (measured in the game, see `build/sora_tmp/er/IMPL_ER1_STATE.md` "Conventions measured": the
//! camera matrix rows are right / up / forward with right x up = forward, and a drawn axis cross
//! agrees). A character's MODEL faces its own -Z: its facing is `orientation * (0, 0, -1)`.
//!
//! Both are left-handed, so the map is a pure axis relabelling with a scale (no mirror):
//!
//! ```text
//! er = (kh.y, kh.z, kh.x) / 100  + origin        kh = (er.z, er.x, er.y) * 100, er taken relative to origin
//! ```
//!
//! `origin` is the Elden Ring position of KH3's world origin. The open world moves its physics
//! origin under the player; `Frame::rebase` shifts `origin` by the same amount, so KH3 positions
//! stay continuous (the simulation never sees the shift).

use glam::{Quat, Vec3};

/// Centimetres per metre.
pub const CM_PER_M: f32 = 100.0;

/// A direction or displacement: KH3 cm to Elden Ring m.
#[inline]
pub fn dir_kh_to_er(v: Vec3) -> Vec3 {
    Vec3::new(v.y, v.z, v.x) / CM_PER_M
}

/// A direction or displacement: Elden Ring m to KH3 cm.
#[inline]
pub fn dir_er_to_kh(v: Vec3) -> Vec3 {
    Vec3::new(v.z, v.x, v.y) * CM_PER_M
}

/// A unit vector (normal): axes only, no scale.
#[inline]
pub fn unit_kh_to_er(v: Vec3) -> Vec3 {
    Vec3::new(v.y, v.z, v.x)
}

/// A unit vector (normal): axes only, no scale.
#[inline]
pub fn unit_er_to_kh(v: Vec3) -> Vec3 {
    Vec3::new(v.z, v.x, v.y)
}

/// A character model's forward in its own space (measured: the player faces away from the camera
/// when `orientation * +Z` points at it).
pub const MODEL_FORWARD: Vec3 = Vec3::NEG_Z;

/// KH3 yaw (degrees, about Z up, 0 = +X, positive toward +Y = to the right) to the angle in
/// radians of the rotation about Elden Ring's +Y that turns a character model (facing -Z) to the
/// same facing.
#[inline]
pub fn yaw_kh_to_er(yaw_deg: f32) -> f32 {
    yaw_deg.to_radians() + std::f32::consts::PI
}

/// The facing (unit, horizontal) of a KH3 yaw, in Elden Ring axes.
#[inline]
pub fn facing_er(yaw_deg: f32) -> Vec3 {
    let a = yaw_deg.to_radians();
    Vec3::new(a.sin(), 0.0, a.cos())
}

/// The orientation quaternion (x, y, z, w) Elden Ring wants for a character with this KH3 yaw.
#[inline]
pub fn quat_er_from_yaw(yaw_deg: f32) -> Quat {
    Quat::from_rotation_y(yaw_kh_to_er(yaw_deg))
}

/// KH3 yaw (degrees, -180 ..= 180) of an Elden Ring direction (its horizontal part).
#[inline]
pub fn yaw_kh_of_er_dir(d: Vec3) -> f32 {
    d.x.atan2(d.z).to_degrees()
}

/// KH3 yaw of an Elden Ring character orientation (the facing of its model forward).
#[inline]
pub fn yaw_kh_of_er_quat(q: Quat) -> f32 {
    yaw_kh_of_er_dir(q * MODEL_FORWARD)
}

/// The moving frame between the two spaces: where KH3's origin is in Elden Ring's physics space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub origin: Vec3,
}

impl Frame {
    /// A frame whose KH3 origin is at this Elden Ring position.
    pub fn at(origin_er: Vec3) -> Self {
        Self { origin: origin_er }
    }

    #[inline]
    pub fn pos_kh_to_er(&self, p: Vec3) -> Vec3 {
        self.origin + dir_kh_to_er(p)
    }

    #[inline]
    pub fn pos_er_to_kh(&self, p: Vec3) -> Vec3 {
        dir_er_to_kh(p - self.origin)
    }

    /// The physics origin of the game moved: every Elden Ring position changed by `shift_er`
    /// (new = old + shift). KH3 positions are unchanged by this.
    pub fn rebase(&mut self, shift_er: Vec3) {
        self.origin += shift_er;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-4
    }

    #[test]
    fn axes_map_forward_right_up() {
        // KH3 forward (+X) is Elden Ring +Z, right (+Y) is +X, up (+Z) is +Y; 100 cm = 1 m.
        assert!(close(dir_kh_to_er(Vec3::new(100.0, 0.0, 0.0)), Vec3::Z));
        assert!(close(dir_kh_to_er(Vec3::new(0.0, 100.0, 0.0)), Vec3::X));
        assert!(close(dir_kh_to_er(Vec3::new(0.0, 0.0, 100.0)), Vec3::Y));
    }

    #[test]
    fn round_trips() {
        let v = Vec3::new(123.0, -45.5, 678.25);
        assert!(close(dir_er_to_kh(dir_kh_to_er(v)), v));
        assert!(close(unit_er_to_kh(unit_kh_to_er(v)), v));
        let frame = Frame::at(Vec3::new(10.0, -3.0, 250.0));
        assert!((frame.pos_er_to_kh(frame.pos_kh_to_er(v)) - v).length() < 1e-2);
    }

    #[test]
    fn handedness_is_kept() {
        // Left-handed on both sides: forward x right = up with the same algebraic cross product.
        let (f, r, u) = (Vec3::X, Vec3::Y, Vec3::Z);
        assert!(close(f.cross(r), u));
        let (fe, re, ue) = (unit_kh_to_er(f), unit_kh_to_er(r), unit_kh_to_er(u));
        assert!(close(fe.cross(re), ue));
    }

    #[test]
    fn yaw_matches_facing() {
        for yaw in [0.0f32, 30.0, 90.0, 135.0, -90.0, 180.0] {
            let a = yaw.to_radians();
            let kh_facing = Vec3::new(a.cos(), a.sin(), 0.0);
            assert!(close(unit_kh_to_er(kh_facing), facing_er(yaw)), "yaw {yaw}");
            assert!(close(quat_er_from_yaw(yaw) * MODEL_FORWARD, facing_er(yaw)), "quat {yaw}");
            let back = yaw_kh_of_er_quat(quat_er_from_yaw(yaw));
            let diff = (back - yaw + 540.0).rem_euclid(360.0) - 180.0;
            assert!(diff.abs() < 1e-3, "yaw {yaw} came back as {back}");
        }
        // Yaw 90 is to the right of yaw 0 in KH3; in Elden Ring that is +X.
        assert!(close(facing_er(90.0), Vec3::X));
    }

    #[test]
    fn rebase_keeps_kh_positions() {
        let mut frame = Frame::at(Vec3::new(5.0, 1.0, -7.0));
        let er = Vec3::new(40.0, 2.0, 13.0);
        let kh = frame.pos_er_to_kh(er);
        let shift = Vec3::new(-256.0, 0.0, 512.0);
        frame.rebase(shift);
        assert!((frame.pos_er_to_kh(er + shift) - kh).length() < 1e-2);
        assert!(close(frame.pos_kh_to_er(kh), er + shift));
    }
}
