//! Where a sound is: the bones of each step's pose in the world (cm, core axes), the sockets of
//! Sora's skeleton, and the detector's per-step samples (footsteps, jump / landing, cloth).
//! (Moved here from kh-app/src/audio.rs, unchanged.)

use std::collections::HashMap;

use glam::{Quat, Vec3};

/// The bones of the latest poses, in the world (cm, core axes): filled once per simulation step
/// (`record`), read by the audio side.
#[derive(Default)]
pub struct PoseTap {
    /// One entry per step not yet consumed.
    pub samples: Vec<BoneSample>,
    index: HashMap<String, usize>,
    /// Every bone's world position as of the latest step.
    latest: Vec<Vec3>,
    /// Every bone's world transform as of the latest step (for sockets and bone-frame offsets).
    latest_world: Vec<kh_anim::Trs>,
}

/// One step's input of the automatic motion sounds.
#[derive(Clone, Debug)]
pub struct BoneSample {
    pub step: u64,
    /// The bones of `kh_audio::autose::BONES`.
    pub positions: [Vec3; 14],
    pub grounded: bool,
    pub anim: &'static str,
    pub anim_time: f32,
}

impl PoseTap {
    /// Called right after the pose of step `step` was evaluated.
    pub fn record(&mut self, step: u64, out: &kh_core::Output, skeleton: &kh_anim::Skeleton, pose: &kh_anim::Pose, component_to_world: &kh_anim::Trs) {
        if self.index.is_empty() {
            self.index = skeleton.bones.iter().enumerate().map(|(i, b)| (b.name.clone(), i)).collect();
        }
        self.latest.clear();
        self.latest.extend(pose.component.iter().map(|t| component_to_world.transform_point(t.translation)));
        self.latest_world.clear();
        self.latest_world.extend(pose.component.iter().map(|t| t.then(component_to_world)));
        let mut positions = [out.position; 14];
        for (k, name) in kh_audio::autose::BONES.iter().enumerate() {
            if let Some(p) = self.bone(name) {
                positions[k] = p;
            }
        }
        self.samples.push(BoneSample { step, positions, grounded: out.on_floor, anim: out.anim_ticked.name.name(), anim_time: out.anim_ticked.time });
        // Nobody consumes them (the engine is off): keep the newest few.
        if self.samples.len() > 64 {
            self.samples.remove(0);
        }
    }

    /// A bone's world position as of the latest step.
    pub fn bone(&self, name: &str) -> Option<Vec3> {
        self.latest.get(*self.index.get(name)?).copied()
    }

    /// A bone's world transform as of the latest step (the effect side's sockets hang on these).
    pub fn bone_world(&self, name: &str) -> Option<kh_anim::Trs> {
        self.latest_world.get(*self.index.get(name)?).copied()
    }

    /// (INT) Takes `other`'s latest pose (bones; the index when this tap has none), no samples: the effect side keeps the
    /// step before this way without a new allocation each step.
    pub fn copy_latest_from(&mut self, other: &PoseTap) {
        if self.index.len() != other.index.len() {
            self.index = other.index.clone();
        }
        self.latest.clone_from(&other.latest);
        self.latest_world.clone_from(&other.latest_world);
        self.samples.clear();
    }

    /// A pose has been recorded.
    pub fn has_pose(&self) -> bool {
        !self.latest_world.is_empty()
    }

    /// A bone's or a socket's world transform as of the latest step (the effect side attaches to
    /// the same sockets).
    pub fn frame(&self, name: &str, sockets: &HashMap<String, kh_audio::pack::Socket>) -> Option<kh_anim::Trs> {
        if let Some(&bone) = self.index.get(name) {
            return self.latest_world.get(bone).copied();
        }
        let socket = sockets.get(name)?;
        let parent = self.latest_world.get(*self.index.get(&socket.bone)?)?;
        Some(socket_transform(socket).then(parent))
    }

    /// Where `offset` (cm, in the frame of the bone or socket `name`) is in the world as of the
    /// latest step. A SOCKET is its parent bone's transform applied to the socket's relative
    /// location and rotation (`sockets`: the audio pack's table of Sora's skeleton sockets).
    pub fn point(&self, name: &str, offset: Vec3, sockets: &HashMap<String, kh_audio::pack::Socket>) -> Option<Vec3> {
        if let Some(&bone) = self.index.get(name) {
            return Some(self.latest_world.get(bone)?.transform_point(offset));
        }
        let socket = sockets.get(name)?;
        let parent = self.latest_world.get(*self.index.get(&socket.bone)?)?;
        Some(socket_transform(socket).then(parent).transform_point(offset))
    }
}

/// A socket's transform relative to its parent bone: the UE rotator (pitch, yaw, roll; degrees)
/// as `FRotator::Quaternion` builds it, and the relative location (cm).
pub fn socket_transform(socket: &kh_audio::pack::Socket) -> kh_anim::Trs {
    let [pitch, yaw, roll] = socket.rotation.map(|d| (d * 0.5).to_radians());
    let ((sp, cp), (sy, cy), (sr, cr)) = (pitch.sin_cos(), yaw.sin_cos(), roll.sin_cos());
    let rotation = Quat::from_xyzw(cr * sp * sy - sr * cp * cy, -cr * sp * cy - sr * cp * sy, cr * cp * sy - sr * sp * cy, cr * cp * cy + sr * sp * sy);
    kh_anim::Trs { translation: Vec3::from(socket.location), rotation, scale: Vec3::ONE }
}
