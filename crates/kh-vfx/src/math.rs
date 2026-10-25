//! KH3 / UE4 space maths: centimetres, degrees, X forward, Y right, Z up, left-handed. glam's
//! types are used as plain containers; every rotation is built from UE's own formulas, never
//! from glam's handedness-dependent helpers.

use glam::{Affine3A, Mat3, Vec3};

/// `FRotationMatrix(FRotator(pitch, yaw, roll))` [U]: the columns are the rotated X, Y, Z axes.
pub fn rotator(pitch: f32, yaw: f32, roll: f32) -> Mat3 {
    let (sp, cp) = pitch.to_radians().sin_cos();
    let (sy, cy) = yaw.to_radians().sin_cos();
    let (sr, cr) = roll.to_radians().sin_cos();
    Mat3::from_cols(Vec3::new(cp * cy, cp * sy, sp), Vec3::new(sr * sp * cy - cr * sy, sr * sp * sy + cr * cy, -sr * cp), Vec3::new(-(cr * sp * cy + sr * sy), cy * sr - cr * sp * sy, cr * cp))
}

/// `FRotator::MakeFromEuler` [U]: an Euler vector is (roll about X, pitch about Y, yaw about Z).
pub fn euler(e: Vec3) -> Mat3 {
    rotator(e.y, e.z, e.x)
}

/// The order the three axis rotations of an emitter node are applied in:
/// `ParticleModuleRequired.RotationPriority` (0x14427DAA0 [C]: the enum's name is the order,
/// first letter first; the default XZY = roll, then yaw, then pitch).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RotationPriority {
    #[default]
    Xzy,
    Xyz,
    Zxy,
    Zyx,
    Yxz,
    Yzx,
}

impl RotationPriority {
    pub fn parse(name: &str) -> RotationPriority {
        match name.rsplit('_').next().unwrap_or("") {
            "XYZ" => RotationPriority::Xyz,
            "ZXY" => RotationPriority::Zxy,
            "ZYX" => RotationPriority::Zyx,
            "YXZ" => RotationPriority::Yxz,
            "YZX" => RotationPriority::Yzx,
            _ => RotationPriority::Xzy,
        }
    }

    /// The node rotation of an Euler vector (degrees about X, Y, Z). Each axis rotation is a
    /// single-component `FRotationMatrix`; UE multiplies row vectors, so "X first" is the
    /// rightmost factor here.
    pub fn matrix(self, e: Vec3) -> Mat3 {
        let x = rotator(0.0, 0.0, e.x);
        let y = rotator(e.y, 0.0, 0.0);
        let z = rotator(0.0, e.z, 0.0);
        match self {
            RotationPriority::Xzy => y * z * x,
            RotationPriority::Xyz => z * y * x,
            RotationPriority::Zxy => y * x * z,
            RotationPriority::Zyx => x * y * z,
            RotationPriority::Yxz => z * x * y,
            RotationPriority::Yzx => x * z * y,
        }
    }
}

/// Where an effect component is: position (cm), rotation (columns = its axes) and scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xform {
    pub position: Vec3,
    pub rotation: Mat3,
    pub scale: Vec3,
}

impl Default for Xform {
    fn default() -> Xform {
        Xform::IDENTITY
    }
}

impl Xform {
    pub const IDENTITY: Xform = Xform { position: Vec3::ZERO, rotation: Mat3::IDENTITY, scale: Vec3::ONE };

    pub fn at(position: Vec3) -> Xform {
        Xform { position, ..Xform::IDENTITY }
    }

    /// Position + a UE rotator (degrees) + uniform scale.
    pub fn from_rotator(position: Vec3, pitch: f32, yaw: f32, roll: f32, scale: f32) -> Xform {
        Xform { position, rotation: rotator(pitch, yaw, roll), scale: Vec3::splat(scale) }
    }

    pub fn affine(&self) -> Affine3A {
        Affine3A::from_mat3_translation(self.rotation * Mat3::from_diagonal(self.scale), self.position)
    }

    /// `child` expressed in this transform's space, as a world transform (scale multiplies).
    pub fn then(&self, child: &Xform) -> Xform {
        Xform { position: self.position + self.rotation * (self.scale * child.position), rotation: self.rotation * child.rotation, scale: self.scale * child.scale }
    }

    pub fn lerp(&self, to: &Xform, t: f32) -> Xform {
        Xform { position: self.position.lerp(to.position, t), rotation: lerp_mat3(&self.rotation, &to.rotation, t), scale: self.scale.lerp(to.scale, t) }
    }
}

/// A rotation and a translation: the form of every matrix of the game's emitter node chain
/// (scales are kept beside it, 0x1442A0CA0 / 0x144290A60 [C]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rigid {
    pub rot: Mat3,
    pub pos: Vec3,
}

impl Rigid {
    pub const IDENTITY: Rigid = Rigid { rot: Mat3::IDENTITY, pos: Vec3::ZERO };

    pub fn point(&self, p: Vec3) -> Vec3 {
        self.rot * p + self.pos
    }

    /// `child` given in this frame, as a transform of the frame this one is given in.
    pub fn then(&self, child: &Rigid) -> Rigid {
        Rigid { rot: self.rot * child.rot, pos: self.rot * child.pos + self.pos }
    }
}

/// Column-wise blend, re-orthonormalised (good enough between two 60 Hz steps).
pub fn lerp_mat3(a: &Mat3, b: &Mat3, t: f32) -> Mat3 {
    let x = a.x_axis.lerp(b.x_axis, t).normalize_or_zero();
    let y = a.y_axis.lerp(b.y_axis, t);
    let y = (y - x * y.dot(x)).normalize_or_zero();
    let z = a.z_axis.lerp(b.z_axis, t);
    let z = (z - x * z.dot(x) - y * z.dot(y)).normalize_or_zero();
    if x == Vec3::ZERO || y == Vec3::ZERO || z == Vec3::ZERO {
        return *b;
    }
    Mat3::from_cols(x, y, z)
}

pub fn lerp_affine(a: &Affine3A, b: &Affine3A, t: f32) -> Affine3A {
    Affine3A::from_mat3_translation(
        Mat3::from_cols(Vec3::from(a.matrix3.x_axis).lerp(Vec3::from(b.matrix3.x_axis), t), Vec3::from(a.matrix3.y_axis).lerp(Vec3::from(b.matrix3.y_axis), t), Vec3::from(a.matrix3.z_axis).lerp(Vec3::from(b.matrix3.z_axis), t)),
        Vec3::from(a.translation).lerp(Vec3::from(b.translation), t),
    )
}

/// The lengths of the three axes of a matrix (its scale).
pub fn scale_of(m: &Affine3A) -> Vec3 {
    Vec3::new(m.matrix3.x_axis.length(), m.matrix3.y_axis.length(), m.matrix3.z_axis.length())
}

/// A matrix whose X axis is `x` (any length > 0), with an arbitrary but stable Y and Z.
pub fn basis_from_x(x: Vec3) -> Mat3 {
    let x = x.normalize_or_zero();
    if x == Vec3::ZERO {
        return Mat3::IDENTITY;
    }
    let up = if x.z.abs() < 0.999 { Vec3::Z } else { Vec3::X };
    let y = up.cross(x).normalize();
    let z = x.cross(y);
    Mat3::from_cols(x, y, z)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotator_axes_follow_unreal() {
        // Yaw +90 turns +X to +Y (to the right, seen from above in a left-handed Z-up frame).
        let m = rotator(0.0, 90.0, 0.0);
        assert!((m * Vec3::X - Vec3::Y).length() < 1e-6);
        // Pitch +90 lifts +X to +Z.
        let m = rotator(90.0, 0.0, 0.0);
        assert!((m * Vec3::X - Vec3::Z).length() < 1e-6);
        // Roll +90: UE's Y axis goes to -Z.
        let m = rotator(0.0, 0.0, 90.0);
        assert!((m * Vec3::Y - Vec3::NEG_Z).length() < 1e-6);
        // A full rotator equals roll, then pitch, then yaw (UE: v * Roll * Pitch * Yaw).
        let full = rotator(20.0, 30.0, 40.0);
        let composed = rotator(0.0, 30.0, 0.0) * rotator(20.0, 0.0, 0.0) * rotator(0.0, 0.0, 40.0);
        assert!((full.x_axis - composed.x_axis).length() < 1e-5 && (full.z_axis - composed.z_axis).length() < 1e-5);
    }

    #[test]
    fn rotation_priority_is_the_order_of_its_name() {
        let e = Vec3::new(90.0, 0.0, 90.0);
        // XZY: roll about X first (+Y -> -Z), then yaw about Z leaves -Z.
        let m = RotationPriority::Xzy.matrix(e);
        assert!((m * Vec3::Y - Vec3::NEG_Z).length() < 1e-5);
        // ZYX: yaw first (+Y -> -X), then roll about X leaves it.
        let m = RotationPriority::Zyx.matrix(e);
        assert!((m * Vec3::Y - Vec3::NEG_X).length() < 1e-5);
        assert_eq!(RotationPriority::parse("ROTATION_PRIORITY_YZX"), RotationPriority::Yzx);
    }

    #[test]
    fn xform_composes_like_an_attachment() {
        let parent = Xform::from_rotator(Vec3::new(100.0, 0.0, 0.0), 0.0, 90.0, 0.0, 2.0);
        let child = Xform::at(Vec3::new(10.0, 0.0, 0.0));
        let world = parent.then(&child);
        assert!((world.position - Vec3::new(100.0, 20.0, 0.0)).length() < 1e-4);
        assert!((parent.affine().transform_point3(Vec3::new(10.0, 0.0, 0.0)) - world.position).length() < 1e-4);
    }
}
