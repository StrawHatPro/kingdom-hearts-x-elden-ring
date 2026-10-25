//! The seam queues: `AudioQueue` implements `kh_core::Audio`, `EffectQueue` implements
//! `kh_core::Effects`. The calls of a step are queued in order; the host's audio and effect
//! sides act on them after the step (the core calls the seam in the middle of a fixed step,
//! where a host has neither its audio device nor its renderer at hand).
//! (Moved here from kh-app/src/seam.rs, unchanged, so that every host shares them.)

use kh_core::{EffectHandle, EffectSpawn, SoundHandle, SoundPlay};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AudioCommand {
    Play(SoundHandle, SoundPlay),
    Stop(SoundHandle, f32),
    /// (audio implementer) `kh_core::Audio::voice_group_no`: in order with the plays.
    VoiceGroupNo(i32),
}

#[derive(Default)]
pub struct AudioQueue(pub Vec<AudioCommand>);

impl kh_core::Audio for AudioQueue {
    fn play(&mut self, handle: SoundHandle, play: &SoundPlay) {
        self.0.push(AudioCommand::Play(handle, *play));
    }

    fn stop(&mut self, handle: SoundHandle, fade_out: f32) {
        self.0.push(AudioCommand::Stop(handle, fade_out));
    }

    fn voice_group_no(&mut self, number: i32) {
        self.0.push(AudioCommand::VoiceGroupNo(number));
    }

    /// (party stream E) Carried as a play with handle 0 (kh_core::host::party::PARTY_VOICE_GROUP_NO).
    fn party_voice_group_no(&mut self, member: kh_core::PartyMember, number: i32) {
        self.0.push(AudioCommand::Play(SoundHandle(0), kh_core::host::party::voice_group_no_play(member, number)));
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EffectCommand {
    Spawn(EffectHandle, EffectSpawn),
    Stop(EffectHandle, bool),
    /// (DON-MAGIC tint) the effect of this handle recoloured (`kh_core::Effects::tint`).
    Tint(EffectHandle, kh_core::host::Tint),
    /// (DON-MAGIC orbit) the effect of this handle moved (`kh_core::Effects::place`): a world point, or the new offset of an
    /// attached one.
    Place(EffectHandle, glam::Vec3),
    /// (ZETTA) the effect of this handle moved, turned and scaled (`kh_core::Effects::transform`): world place, rotation
    /// (pitch, yaw, roll) degrees, scale per local axis.
    Transform(EffectHandle, glam::Vec3, glam::Vec3, glam::Vec3),
}

#[derive(Default)]
pub struct EffectQueue(pub Vec<EffectCommand>);

impl kh_core::Effects for EffectQueue {
    fn spawn(&mut self, handle: EffectHandle, spawn: &EffectSpawn) {
        self.0.push(EffectCommand::Spawn(handle, *spawn));
    }

    fn stop(&mut self, handle: EffectHandle, immediate: bool) {
        self.0.push(EffectCommand::Stop(handle, immediate));
    }

    fn tint(&mut self, handle: EffectHandle, tint: kh_core::host::Tint) {
        self.0.push(EffectCommand::Tint(handle, tint)); // (DON-MAGIC tint)
    }

    fn place(&mut self, handle: EffectHandle, at: glam::Vec3) {
        self.0.push(EffectCommand::Place(handle, at)); // (DON-MAGIC orbit)
    }

    fn transform(&mut self, handle: EffectHandle, at: glam::Vec3, rotation: glam::Vec3, scale: glam::Vec3) {
        self.0.push(EffectCommand::Transform(handle, at, rotation, scale)); // (ZETTA)
    }
}

/// Where a point given in Sora's ACTOR frame is in the world (his origin `position`, facing
/// `yaw` degrees): what a host answers for `Attach::Sora` when it has no pose to look a bone
/// up in. A bone attachment without a pose is taken one metre above his origin (the test
/// app's markers.rs `resolve`).
pub fn on_actor(position: glam::Vec3, yaw: f32, at_bone: bool, offset: glam::Vec3) -> glam::Vec3 {
    let (sin, cos) = yaw.to_radians().sin_cos();
    let lift = if at_bone { 100.0 } else { 0.0 };
    position + glam::Vec3::new(offset.x * cos - offset.y * sin, offset.x * sin + offset.y * cos, offset.z + lift)
}
