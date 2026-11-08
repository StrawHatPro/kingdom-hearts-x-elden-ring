//! (GOOFY) THE GOOFY OVERHAUL (documentation/GOOFY_OVERHAUL.md, build/sora_tmp/party/GOOFY/PLAN.md): the shield of the
//! party, a parry fighter. Read, deflect, break, finish: every parry takes 0 damage and fills the enemy's Stagger, and Goofy
//! Breaker on the BROKEN enemy is his big damage (THE USER 2026-11-02: "Goofy's big damage should solely come from his guard
//! break attacks from 6.3"). The state and the rules live here, on `FriendStatus` (an AI Goofy has the same state); the pad
//! layer is pad/goofy*.rs, as Donald's is laid out.
//!
//! - [`tune`]: the numbers (sheet party-kit-goofy).
//! - [`learn`]: the rank of a technique or an Art (the store is GF-STATS' `growth::RankBank`).
//! - [`abilities`]: his own abilities, AP, levels, save names.
//! - [`parry`]: which technique answers what, the windows, the one judgement of a hit.
//! - [`core`]: the tap clock, Resolve, deflect strings, the evasion book.
//! - [`hooks`]: the party's step for every Goofy (pad or AI).
//! - [`ai`]: AI Goofy's brain and its buttons.
//! - [`seams`]: what the later lanes plug into.
//! - [`stagger`]: the enemy's posture, one gauge per target (GF-BREAK).
//! - [`guide`]: the camp menu's Parry Techniques page (learned / locked, ranks, when, how), from his live level and ranks.

pub mod abilities;
pub mod ai;
#[cfg(test)]
mod bounce_tests; // (THE USER 2026-11-02) enemies bounce off his shield on a perfect deflect, pad and AI
pub mod core;
pub mod guide; // (GOOFY, THE USER 2026-11-02) the camp menu's Parry Techniques page: what he has, when, how
pub mod hooks;
pub mod learn;
pub mod parry;
pub mod seams;
pub mod stagger;
pub mod threat;
#[cfg(test)]
mod threat_tests;
pub mod tune;

pub use self::core::{GoofyCore, Resolve, Watch, RESOLVE_FULL};
pub use abilities::{GoofyAbilities, GoofyAbility};
pub use hooks::PartyGoofy;
pub use learn::{GoofyMove, RankBank, MAX_RANK};
pub use parry::{Parry, StickRel, Tech, Verdict};
pub use seams::{GoofyEvent, GoofyHud, StaggerWhy, ThreatWhy};
pub use stagger::{StaggerBook, StaggerFacts, StaggerView};
pub use threat::{ThreatBook, ThreatRow};
pub use tune::K;

use kh_core::host::{AttackClass, TargetId};

/// One enemy attack the host sees coming (the host's sensors in Elden Ring, script words in the test level).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Telegraph {
    pub attacker: TargetId,
    pub class: AttackClass,
    pub perilous: bool,
    /// Seconds until the hit window opens (0 when `active`).
    pub eta: f32,
    /// Its hit window is open now.
    pub active: bool,
}

/// One Goofy's overhaul state (`FriendStatus::goofy`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct GoofyState {
    pub core: GoofyCore,
    pub abilities: GoofyAbilities,
    /// The running moves (pad/goofy.rs): the same for the pad and for AI Goofy.
    pub run: crate::pad::goofy::GoofyRun,
}

impl GoofyState {
    /// A fresh Goofy.
    pub fn new() -> GoofyState {
        GoofyState::default()
    }

    /// The test level's loadout: every ability (the ranks are `FriendStatus::growth`'s).
    pub fn test_loadout() -> GoofyState {
        GoofyState { abilities: GoofyAbilities::test_all(), ..GoofyState::default() }
    }
}
