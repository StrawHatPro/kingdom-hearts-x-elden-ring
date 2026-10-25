//! The audio side of a host, between `kh_core`'s audio seam and `kh_audio`:
//!
//! - [`pose`]: where a sound is (bones and sockets of each step's pose, the detector's samples);
//! - [`session`]: the rules every host applies to the seam's commands and one engine step per
//!   simulation step (the low-HP alarm's one owner, spell actors' hit sounds, REFLECT);
//! - [`device`]: `kh_audio::Mixer` as a rodio source on the default output device, and the
//!   growing WAV file of an offline render;
//! - [`threaded`]: the output for a host that must never block (a game's frame thread): voice
//!   commands go through a queue to a thread of ours that reads the materials and feeds the
//!   device's own callback; the device is reopened when it goes away or the default changes;
//! - [`host`]: all of it as one object ([`HostAudio`]) for a host without an ECS.

pub mod device;
pub mod host;
pub mod pose;
pub mod session;
pub mod threaded;

pub use device::{start_device_thread, WavOut};
pub use host::{HostAudio, Step};
pub use pose::{socket_transform, BoneSample, PoseTap};
pub use session::{reflected_hit_sounds, Alarm, Attaches};
pub use session::party_voice_inputs; // (FACE)
pub use threaded::{Output, Stats};

/// Frames per fixed 60 Hz step at the mixer's rate.
pub const FRAMES_PER_STEP: usize = (kh_audio::mixer::RATE / 60) as usize;

/// Decibels to a linear gain.
pub fn db_to_gain(db: f32) -> f32 {
    10.0f32.powf(db / 20.0)
}
