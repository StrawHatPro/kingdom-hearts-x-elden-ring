//! The constants of docs/sora-audio-spec.md, generated from `sheets/port/sora-sound-rules.json`.

include!(concat!(env!("KH_GEN_DIR"), "/sora_sound_rules.rs"));

/// The sheet's values.
pub const RULES: SoundRules = SOUND_RULES_PARAMS;
