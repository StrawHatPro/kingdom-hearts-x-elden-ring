//! The interpolation and blend rules of the game (docs/sora-animation-spec.md sections 7 and 8;
//! change list A4, A5). None of them is a spherical interpolation.

use glam::{Quat, Vec3};

use crate::Trs;

/// The slot counts as empty at or below this weight, and as full at or above one minus it
/// (`ZERO_ANIMWEIGHT_THRESH`).
pub const ZERO_WEIGHT: f32 = 1.0e-5;

fn scaled(q: Quat, s: f32) -> [f32; 4] {
    [q.x * s, q.y * s, q.z * s, q.w * s]
}

fn dot4(a: [f32; 4], b: [f32; 4]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
}

fn normalised(q: [f32; 4]) -> Quat {
    let square = dot4(q, q);
    if square > 1.0e-8 {
        let k = 1.0 / square.sqrt();
        Quat::from_xyzw(q[0] * k, q[1] * k, q[2] * k, q[3] * k)
    } else {
        Quat::IDENTITY
    }
}

/// Rotation between two keys: normalised linear blend along the shorter arc (`FQuat::FastLerp`,
/// then normalise; spec 7, change A4).
pub fn key_rotation(a: Quat, b: Quat, t: f32) -> Quat {
    let bias = if a.dot(b) >= 0.0 { 1.0 } else { -1.0 };
    let (p, q) = (scaled(b, t), scaled(a, bias * (1.0 - t)));
    normalised([p[0] + q[0], p[1] + q[1], p[2] + q[2], p[3] + q[3]])
}

/// Scale between two keys: geometric per component, `a x (b / a)^t` when `a x b > 0`, else linear
/// (spec 7, change A5).
pub fn key_scale(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    let one = |a: f32, b: f32| if a * b > 0.0 { a * (b / a).powf(t) } else { a + (b - a) * t };
    Vec3::new(one(a.x, b.x), one(a.y, b.y), one(a.z, b.z))
}

fn ln(v: Vec3) -> Option<Vec3> {
    (v.x > 0.0 && v.y > 0.0 && v.z > 0.0).then(|| Vec3::new(v.x.ln(), v.y.ln(), v.z.ln()))
}

/// Several transforms by weight, in the order given (the game sorts: montage or heaviest sample
/// first, source last): rotation accumulated with the sign that keeps it on the side of the
/// running sum, then normalised; translation summed; scale in log space (spec 8).
pub fn weighted(parts: &[(Trs, f32)]) -> Trs {
    let (first, w0) = parts[0];
    let mut rotation = scaled(first.rotation, w0);
    let mut translation = first.translation * w0;
    let mut log_scale = ln(first.scale).map(|l| l * w0);
    let mut linear_scale = first.scale * w0;
    for &(part, w) in &parts[1..] {
        let q = scaled(part.rotation, w);
        let sign = if dot4(rotation, q) >= 0.0 { 1.0 } else { -1.0 };
        for i in 0..4 {
            rotation[i] += sign * q[i];
        }
        translation += part.translation * w;
        log_scale = match (log_scale, ln(part.scale)) {
            (Some(sum), Some(l)) => Some(sum + l * w),
            _ => None,
        };
        linear_scale += part.scale * w;
    }
    // A scale that is not positive has no logarithm: such a bone is blended linearly.
    let scale = match log_scale {
        Some(l) => Vec3::new(l.x.exp(), l.y.exp(), l.z.exp()),
        None => linear_scale,
    };
    Trs { translation, rotation: normalised(rotation), scale }
}

/// The slot node: the montage pose `m` at weight `w` over the source pose `p` (spec 8 "Slot
/// combine").
pub fn slot(m: &Trs, w: f32, p: &Trs) -> Trs {
    if w <= ZERO_WEIGHT {
        return *p;
    }
    if w >= 1.0 - ZERO_WEIGHT {
        // The source is not evaluated: the montage pose times its weight, rotation renormalised.
        return Trs { translation: m.translation * w, rotation: normalised(scaled(m.rotation, w)), scale: m.scale * w };
    }
    weighted(&[(*m, w), (*p, 1.0 - w)])
}

/// `FTransform::Blend` as the skeletal controls use it: translation and scale linear, rotation
/// `FastLerp` normalised.
pub fn transform_blend(a: &Trs, b: &Trs, alpha: f32) -> Trs {
    Trs { translation: a.translation.lerp(b.translation, alpha), rotation: key_rotation(a.rotation, b.rotation, alpha), scale: a.scale.lerp(b.scale, alpha) }
}

/// `FQuat::Slerp_NotNormalized`.
pub fn slerp_not_normalised(a: Quat, b: Quat, alpha: f32) -> Quat {
    let raw = a.dot(b);
    let cosine = raw.abs();
    let (s0, mut s1) = if cosine < 0.9999 {
        let omega = cosine.clamp(-1.0, 1.0).acos();
        let inverse = 1.0 / omega.sin();
        (((1.0 - alpha) * omega).sin() * inverse, (alpha * omega).sin() * inverse)
    } else {
        (1.0 - alpha, alpha)
    };
    if raw < 0.0 {
        s1 = -s1;
    }
    Quat::from_xyzw(s0 * a.x + s1 * b.x, s0 * a.y + s1 * b.y, s0 * a.z + s1 * b.z, s0 * a.w + s1 * b.w)
}

/// `FQuat::GetNormalized` (identity below the tolerance).
pub fn normalise(q: Quat) -> Quat {
    normalised([q.x, q.y, q.z, q.w])
}

/// `FQuat::FindBetween` for any two vectors (the shortest rotation taking `a` onto `b`).
pub fn find_between(a: Vec3, b: Vec3) -> Quat {
    let norm = (a.length_squared() * b.length_squared()).sqrt();
    let mut w = norm + a.dot(b);
    let xyz = if w >= 1.0e-6 * norm {
        a.cross(b)
    } else {
        // Opposite: a half turn about any axis orthogonal to `a`.
        w = 0.0;
        if a.x.abs() > a.y.abs() {
            Vec3::new(-a.z, 0.0, a.x)
        } else {
            Vec3::new(0.0, -a.z, a.y)
        }
    };
    normalise(Quat::from_xyzw(xyz.x, xyz.y, xyz.z, w))
}

/// UE's rotator (degrees): pitch about Y, yaw about Z, roll about X.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rotator {
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
}

fn normalise_axis(mut degrees: f32) -> f32 {
    degrees %= 360.0;
    if degrees < 0.0 {
        degrees += 360.0;
    }
    if degrees > 180.0 {
        degrees -= 360.0;
    }
    degrees
}

impl Rotator {
    /// `FQuat::Rotator`.
    pub fn from_quat(q: Quat) -> Rotator {
        let singularity = q.z * q.x - q.w * q.y;
        let yaw_y = 2.0 * (q.w * q.z + q.x * q.y);
        let yaw_x = 1.0 - 2.0 * (q.y * q.y + q.z * q.z);
        const THRESHOLD: f32 = 0.499_999_5;
        let yaw = yaw_y.atan2(yaw_x).to_degrees();
        if singularity < -THRESHOLD {
            Rotator { pitch: -90.0, yaw, roll: normalise_axis(-yaw - (2.0 * q.x.atan2(q.w)).to_degrees()) }
        } else if singularity > THRESHOLD {
            Rotator { pitch: 90.0, yaw, roll: normalise_axis(yaw - (2.0 * q.x.atan2(q.w)).to_degrees()) }
        } else {
            Rotator { pitch: (2.0 * singularity).clamp(-1.0, 1.0).asin().to_degrees(), yaw, roll: (-2.0 * (q.w * q.x + q.y * q.z)).atan2(1.0 - 2.0 * (q.x * q.x + q.y * q.y)).to_degrees() }
        }
    }

    /// `FRotator::Quaternion`.
    pub fn quat(self) -> Quat {
        let (sp, cp) = (self.pitch.to_radians() * 0.5).sin_cos();
        let (sy, cy) = (self.yaw.to_radians() * 0.5).sin_cos();
        let (sr, cr) = (self.roll.to_radians() * 0.5).sin_cos();
        Quat::from_xyzw(cr * sp * sy - sr * cp * cy, -cr * sp * cy - sr * cp * sy, cr * cp * sy - sr * sp * cy, cr * cp * cy + sr * sp * sy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotator_round_trip_and_yaw_direction() {
        let r = Rotator { pitch: 10.0, yaw: 25.0, roll: -15.0 };
        let back = Rotator::from_quat(r.quat());
        assert!((back.pitch - r.pitch).abs() < 1.0e-3 && (back.yaw - r.yaw).abs() < 1.0e-3 && (back.roll - r.roll).abs() < 1.0e-3, "{back:?}");
        // A yaw of +90 takes +X to +Y (UE: to the right).
        let v = Rotator { pitch: 0.0, yaw: 90.0, roll: 0.0 }.quat() * Vec3::X;
        assert!((v - Vec3::Y).length() < 1.0e-6, "{v:?}");
        // A positive pitch raises +X.
        let v = Rotator { pitch: 30.0, yaw: 0.0, roll: 0.0 }.quat() * Vec3::X;
        assert!(v.z > 0.49 && v.z < 0.51, "{v:?}");
    }

    #[test]
    fn find_between_takes_a_onto_b() {
        let (a, b) = (Vec3::Z, Vec3::new(0.3, -0.2, 0.9).normalize());
        assert!((find_between(a, b) * a - b).length() < 1.0e-6);
        assert!((find_between(a, -a) * a + a).length() < 1.0e-6);
    }
}
