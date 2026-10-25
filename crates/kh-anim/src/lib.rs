//! Sora's pose, engine-free: `glam` and `std` only, so that it can be compiled into a host that
//! is not Bevy. Input: the Sora pack's skeleton and clips as plain keyframe arrays ([`pack`]) and,
//! per simulation step, what the simulation says is playing. Output: every bone's local and
//! component-space transform in KH3's conventions (centimetres, Z up, left-handed; the mesh
//! component's space, in which the character faces +Y). A host only converts and uploads joints.
//!
//! Specification: `docs/sora-animation-spec.md`; change list `docs/sora-animation-spec-changes.md`.

#![forbid(unsafe_code)]

pub mod blend;
pub mod bonamik;
// (XW) the XS form weapons: their own skeleton and clip
pub mod formweapon;
pub mod json;
pub mod keychain;
pub mod kine;
pub mod legik;
// (party A2) TresAnimNode_CollTwoBoneIK (Donald's legs)
pub mod legik_coll;
// (party I2) TresAnimNode_ExLookAt (the friends' head look-at)
pub mod lookat;
// (ER79) which of an outfit's own clips are played (`boc.<outfit>.use_kh3`)
pub mod outfit_clips;
// (FACE) the host face layer (voice lip-sync, talk face, the NPC blink timer) and the lip track of a voice line
pub mod face_layer;
pub mod lip;
pub mod pack;
pub mod pose;
// (IMPL_ER16) a pose of another rig shown on this skeleton
pub mod retarget;

use glam::{Quat, Vec3};

pub use bonamik::{Bonamik, BonamikHost, BonamikOptions, Jitter};
pub use keychain::Keychain;
pub use legik::{sweep_obb, IkShape, IkWorld, NoIkWorld, Obb, ShapeHit, SweepHit};
pub use pack::{Bone, Clip, Pack, Skeleton};
pub use pose::{Animator, BodySlot, Frame, Pose, Settings};

/// One bone transform: UE's `FTransform` (scale, then rotation, then translation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trs {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Trs {
    pub const IDENTITY: Trs = Trs { translation: Vec3::ZERO, rotation: Quat::IDENTITY, scale: Vec3::ONE };

    /// `child * parent` in UE's order: this transform expressed in the parent's parent space.
    pub fn then(&self, parent: &Trs) -> Trs {
        Trs { translation: parent.translation + parent.rotation * (parent.scale * self.translation), rotation: (parent.rotation * self.rotation).normalize(), scale: parent.scale * self.scale }
    }

    /// The transform that, followed by `parent`, gives this one (`FTransform::GetRelativeTransform`).
    pub fn relative_to(&self, parent: &Trs) -> Trs {
        let one = |x: f32| if x.abs() <= 1.0e-8 { 0.0 } else { 1.0 / x };
        let reciprocal = Vec3::new(one(parent.scale.x), one(parent.scale.y), one(parent.scale.z));
        let inverse = parent.rotation.conjugate();
        Trs { translation: (inverse * (self.translation - parent.translation)) * reciprocal, rotation: (inverse * self.rotation).normalize(), scale: self.scale * reciprocal }
    }

    pub fn transform_point(&self, p: Vec3) -> Vec3 {
        self.translation + self.rotation * (self.scale * p)
    }
}
