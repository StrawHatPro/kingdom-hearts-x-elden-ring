//! (DON1) THE DONALD OVERHAUL'S SHARED FOUNDATION (documentation/DONALD_OVERHAUL.md; the plan and the four phase-2
//! streams: build/sora_tmp/party/DON_PLAN.md; state note IMPL_DON_STATE.md). The user (2026-10-22): "Okay, implement the
//! entire doc. He needs to play just as the doc specifies".
//!
//! Engine-free (glam / std / kh-core only, no files, no clocks): the rules every stream builds on.
//! - [`element`]: the six elements and a set of them.
//! - [`tune`]: the numbers (sheets party-don-core / party-don-spells), tunable at run time ([`tune::install`]).
//! - [`spells`]: what he casts ([`SpellRef`]), the MP price rule (6.2), his menu / shortcut command ids ([`DonCmd`]).
//! - [`learn`]: what he has learned and the boss steps (9), Sora's share of a shared magic boss.
//! - [`attune`], [`combat`]: attunement (single and Dual), Resonance, full power, periods, the Release, Arcane Flow, the
//!   cast rule and the Y list ([`DonCore`]).
//! - [`status`], [`react`]: the enemy statuses and the Elemental Reactions (4.5).
//! - [`arcane`]: the Arcane Flow meter.
//! - [`abilities`]: his ability ids, the owned / equipped set and its save names.
//!
//! WHERE THE STATE LIVES: [`DonaldState`] in `FriendStatus::donald` (Some for Donald): his combat core, his copy of the
//! roster's learned magic (kh-partyhost keeps the roster's one, saves it as `donald_magic` and copies it onto him), his
//! abilities. The enemies' [`status::StatusBook`] is party-wide (stream DON-MAGIC places it on `Party`).

pub mod abilities;
pub mod arcane;
pub mod attune;
pub mod combat;
pub mod element;
pub mod hooks;
pub mod learn;
pub mod react;
pub mod spells;
pub mod status;
pub mod tune;
pub mod cast; // (DON-MAGIC)
pub mod allyfx; // (DON-MAGIC)
pub mod magic_fx; // (DON-MAGIC)
pub mod spell_look; // (DON-SPELLB) part B's cast presentation
pub mod spell_payload; // (DON-SPELLB) the ultimates' and Releases' big payloads
pub mod yfx; // (YFX) the Y Arcane spells' look and sound (Comet, Gravity, Meteor, Ultima)
pub mod hybrid_look; // (HYB) the 15 hybrids' look and sound
#[cfg(test)]
mod spellb_tests; // (DON-SPELLB)
pub mod sora_magic; // (DON-MAGIC part A) the magic Donald shares with Sora
pub mod zettaflare; // (ZETTA) Zettaflare as the KH3 cutscene shows it
pub mod cast_look; // (FACE) where he looks while he casts (the cast's target; not during Zettaflare)
#[cfg(test)]
mod magic_tests; // (DON-MAGIC)
pub mod hud; // (DON-UI)
pub mod ability_info; // (DON-PROG)
pub mod aggro; // (DON-PROG)
pub mod aura; // (DON-PROG)
pub mod ai; // (DON-PROG)
#[cfg(test)]
mod prog_tests; // (DON-PROG)
#[cfg(test)]
mod tests;

pub use abilities::{DonAbilities, DonAbility, DonMods};
pub use arcane::{ArcaneFlow, ArcaneGain};
pub use attune::{Attunement, Slot};
pub use combat::{CastPlan, DonCore, DonEvent, Refusal, UltMark, YOffer};
pub use element::{ElemSet, Element};
pub use learn::{sora_gain, DonLearned, DonStep, Grant, SoraGain};
pub use react::{resolve, HitOutcome, HitTag, Reaction};
pub use spells::{ArcaneLine, DonCmd, SpellRef};
pub use status::{Apply, FullFlags, Size, StatusBook, StatusEvent, TargetStatus};
pub use tune::{data, install, DonData, DonSpellRow, DonTune};

/// One Donald's overhaul state (`FriendStatus::donald`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DonaldState {
    pub core: DonCore,
    /// His copy of the roster's learned magic (kh-partyhost `PartyRuntime::don_learned`, synced every step).
    pub learned: DonLearned,
    pub abilities: DonAbilities,
}

impl DonaldState {
    /// A fresh Donald with the start's magic (10).
    pub fn new() -> DonaldState {
        DonaldState { core: DonCore::default(), learned: DonLearned::start(), abilities: DonAbilities::default() }
    }

    /// The full-power flags of his attunement (status lengths, 4.2).
    pub fn full_flags(&self, d: &DonData) -> FullFlags {
        FullFlags { blizzard: self.core.full_power(Element::Blizzard, d), water: self.core.full_power(Element::Water, d), aero: self.core.full_power(Element::Aero, d) }
    }
}
