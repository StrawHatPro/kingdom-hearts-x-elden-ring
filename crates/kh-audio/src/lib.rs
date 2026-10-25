//! Sora's sound, engine-free: docs/sora-audio-spec.md as data and logic.
//!
//! - `pack`: the audio pack (`build/sora_pack/audio/manifest.json` + `tables.json`) as definitions;
//! - `driver`: what is heard when a sound index is started (the SEAD playback model): selection
//!   by sound type, sequences and tracks, the volume and pitch products, start macros, category
//!   pools with voice stealing, category events, 3D attenuation; output = voice commands per step;
//! - `autose`: the bone-motion detector behind footsteps, jump / landing sounds and cloth rustle;
//! - `routes`: which sound a trigger means (weapon aliases, the surface table, voices by name
//!   per language, voice groups, effect-attached sounds, the lookup for later routes);
//! - `alarm`: the low-HP alarm;
//! - `engine`: all of it behind one object for a host;
//! - `mixer`, `wav`: a software backend over the voice commands (device output and offline render).
//!
//! No audio device, no engine types. Sound never feeds back into the simulation: `kh-core` does
//! not depend on this crate, and the randomness here has its own generators (`rng`), seeded by
//! the host.

pub mod alarm;
pub mod autose;
pub mod driver;
pub mod engine;
pub mod mixer;
pub mod outfit;
pub mod pack;
pub mod rng;
pub mod routes;
pub mod rules;
pub mod wav;

pub use autose::{AutoSe, AutoSeEvent, BoneFrame, Motion, Part};
pub use driver::{pan_of, ActiveSound, Driver, Listener, Note, PlayOutcome, PlayParams, SoundKey, VoiceCommand, VoiceId, VoiceParams};
pub use engine::{ActiveLine, Anchor, Engine, LogLine, SeamPlay};
pub use engine::VoiceLine; // (FACE) a party member's voice line for its face's lip-sync
pub use mixer::{Materials, Mixer};
pub use pack::Pack;
pub use routes::{Language, Routes};
pub use rules::RULES;
