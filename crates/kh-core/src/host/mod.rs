//! THE HOST SEAM beyond collision: what a game hosting Sora supplies so that he can hit things,
//! be heard and be seen doing it, and what it gets back. Engine-free plain data and four traits;
//! nothing here knows about Bevy, files or clocks. The prose contract is section 3 of
//! docs/sora-port-plan.md; these doc comments are the authoritative text.
//!
//! | trait | direction | what |
//! |---|---|---|
//! | [`crate::World`] (world.rs) | core asks | collision sweeps and line traces |
//! | [`Targets`] (targets.rs) | core asks, core tells | who can be locked on / hit, their shapes; a hit is delivered and its result comes back |
//! | [`Audio`] (audio.rs) | core tells | play / stop a sound by id, at a point or attached |
//! | [`Effects`] (effects.rs) | core tells | spawn / stop a visual effect by id, at a transform or attached |
//! | [`UiSink`] (ui.rs) | core tells | one plain-data snapshot per step: gauges, command menu, popups |
//!
//! All five are bundled per step in a [`Host`]. Everything the core TELLS the host is also
//! appended, in the order it happened, to [`crate::Output::events`] as [`Event`] records
//! (events.rs): a host may implement the traits, or leave them null ([`NullHost`]) and act on the
//! event list after the step - both carry the same information. What the core ASKS
//! ([`crate::World`], [`Targets`]) feeds the simulation: those answers must be deterministic.
//!
//! Handles ([`SoundHandle`], [`EffectHandle`]) are allocated by the CORE, counting up from 1, so
//! that a replay of the same inputs produces the same handles whatever the host does.
//!
//! NO GAMEPLAY RULE LIVES HERE. Which sound a state plays, what an attack's hit record says and
//! how a target answers are the business of the specs' implementers (`states/*`, `combat/`,
//! `magic/`); this module is only the containers they speak through.

pub mod audio;
pub mod cinematic; // (VFXR cinematic) a sequence's exposure / key light / bloom threshold
pub mod effects;
pub mod events;
pub mod party;
pub mod recording;
pub mod targets;
pub mod time;
pub mod ui;

pub use audio::{Audio, NullAudio, SoundHandle, SoundId, SoundPlay};
pub use effects::{EffectHandle, EffectId, EffectSpawn, Effects, NullEffects, Tint}; // (DON-MAGIC tint) Tint
pub use events::{Event, EventKind};
pub use party::PartyMember;
pub use recording::{Call, RecordingHost};
pub use targets::{Friend, FriendLinkRequest, FriendLinkView, TeamShot};
pub use targets::{Ally, AttackShape, BodyReaction, FinishKind, HitAttacker, HitInfo,PushBody, ReactionResult, TargetBody, HitRecord, HitResult, HurtShape, NullTargets, TargetDefense, TargetFlags, TargetId, TargetInfo, Targets};
pub use targets::{ScriptFinale, ScriptHold, HIT_FLAG_FORCE}; // (ZETTA)
pub use ui::{NullUi, UiSink};

use crate::world::World;
use glam::Vec3;

/// Where a sound or an effect is: a fixed point, or riding on something.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Attach {
    /// A fixed point of the world, cm, core axes.
    World(Vec3),
    /// On Sora: at a bone or socket of his skeleton (`None` = the actor origin, at his feet),
    /// plus `offset` in that bone's (or the actor's) own frame, cm. The host follows him.
    Sora { bone: Option<&'static str>, offset: Vec3 },
    /// On a target (the host knows where its targets are), `offset` in the target's frame, cm.
    Target { target: TargetId, offset: Vec3 },
    /// (party stream E) On a party member (Donald, Goofy): at a bone or socket of THEIR skeleton
    /// (`None` = the actor origin), `offset` in that bone's (or the actor's) frame, cm. The host
    /// follows the member, and resolves a sound NAME with that member's routes (voice groups,
    /// weapon sound sets, footstep table). Handles: `party::SeamHandles` (party.rs).
    Party { member: party::PartyMember, bone: Option<&'static str>, offset: Vec3 },
}

impl Attach {
    /// At Sora's actor origin.
    pub const SORA: Attach = Attach::Sora { bone: None, offset: Vec3::ZERO };
}

/// Everything the host lends the core for one step.
pub struct Host<'a> {
    pub world: &'a dyn World,
    pub targets: &'a mut dyn Targets,
    pub audio: &'a mut dyn Audio,
    pub effects: &'a mut dyn Effects,
    pub ui: &'a mut dyn UiSink,
}

/// The host that has nothing but a world: no targets, silent, invisible, no UI. `Sora::step`
/// uses it; the step's [`Event`]s are still in the `Output`.
#[derive(Clone, Copy, Debug, Default)]
pub struct NullHost {
    pub targets: NullTargets,
    pub audio: NullAudio,
    pub effects: NullEffects,
    pub ui: NullUi,
}

impl NullHost {
    /// The bundle for one step over `world`.
    pub fn host<'a>(&'a mut self, world: &'a dyn World) -> Host<'a> {
        Host { world, targets: &mut self.targets, audio: &mut self.audio, effects: &mut self.effects, ui: &mut self.ui }
    }
}
