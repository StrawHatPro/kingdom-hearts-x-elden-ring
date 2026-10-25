//! The host bundle for one party step (contract B.6).

use kh_core::{Audio, Effects, Targets, World};

/// Everything the host lends the party for one step.
pub struct PartyHost<'a> {
    /// Collision (the same world Sora uses).
    pub world: &'a dyn World,
    /// The ENEMIES the friends can hit (never Sora, never each other).
    pub targets: &'a mut dyn Targets,
    pub audio: &'a mut dyn Audio,
    pub effects: &'a mut dyn Effects,
}

/// A host with a world only: no targets, silent, invisible (tests).
#[derive(Default)]
pub struct NullPartyHost {
    pub targets: kh_core::host::NullTargets,
    pub audio: kh_core::host::audio::NullAudio,
    pub effects: kh_core::host::effects::NullEffects,
}

impl NullPartyHost {
    pub fn host<'a>(&'a mut self, world: &'a dyn World) -> PartyHost<'a> {
        PartyHost { world, targets: &mut self.targets, audio: &mut self.audio, effects: &mut self.effects }
    }
}
