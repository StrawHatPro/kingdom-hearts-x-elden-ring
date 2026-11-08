//! (GOOFY) GOOFY'S ABILITIES (documentation/GOOFY_OVERHAUL.md 11.3, ruling R16): the ids, the owned / equipped set, the save
//! names (`GOF_*`, the friends record's ability list), AP and learn levels (rows `ability.*` of party-kit-goofy). They replace
//! the old `KIT_*` kit: those names are dropped on load, which frees their AP.
//!
//! The doc's KH3 standard abilities (Lucky Strike, Treasure Magnet, Item Boost, Second Chance, Withstand Combo and the boss
//! list's Team Attack Extender, Share Prizes, Hyper Healing, Damage Control, More Team Attacks) stay KH3's: they live in
//! `FriendStatus::abilities` with their shared rows.

use super::learn::{self, GoofyMove};
use super::tune::K;
use crate::status::FriendStatus;
use kh_core::enums::ETresAbilityKind as A;

/// One of Goofy's own abilities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GoofyAbility {
    // parry
    DangerSense,
    SteadyGuard,
    IronPosture,
    /// (THE USER 2026-11-02: "Remove countering with 'A' after a parry technique") Counter Chain+'s slot, AP and level: the
    /// counter chain went with the counters. An old save's `GOF_COUNTER_CHAIN_PLUS` is read as this ([`GoofyAbility::from_name`]).
    StaggerBoost,
    SecondDeflect,
    RevengeBash,
    AllyGuard,
    // Breaker
    PartyBreaker,
    LaunchBreaker,
    RallyBreaker,
    DeepBreaker,
    // Resolve
    ResolveKeeper,
    ZoneExtender,
    ForwardDeflectPlus,
    // aggro and support
    LoudMouth,
    Bodyguard,
    SecondWind,
}

impl GoofyAbility {
    pub const ALL: [GoofyAbility; 17] = [
        GoofyAbility::DangerSense,
        GoofyAbility::SteadyGuard,
        GoofyAbility::IronPosture,
        GoofyAbility::StaggerBoost,
        GoofyAbility::SecondDeflect,
        GoofyAbility::RevengeBash,
        GoofyAbility::AllyGuard,
        GoofyAbility::PartyBreaker,
        GoofyAbility::LaunchBreaker,
        GoofyAbility::RallyBreaker,
        GoofyAbility::DeepBreaker,
        GoofyAbility::ResolveKeeper,
        GoofyAbility::ZoneExtender,
        GoofyAbility::ForwardDeflectPlus,
        GoofyAbility::LoudMouth,
        GoofyAbility::Bodyguard,
        GoofyAbility::SecondWind,
    ];

    const fn bit(self) -> u32 {
        1 << self as u32
    }

    /// The menu's English name (the doc's).
    pub const fn label(self) -> &'static str {
        match self {
            GoofyAbility::DangerSense => "Danger Sense",
            GoofyAbility::SteadyGuard => "Steady Guard",
            GoofyAbility::IronPosture => "Iron Posture",
            GoofyAbility::StaggerBoost => "Stagger Boost",
            GoofyAbility::SecondDeflect => "Second Deflect",
            GoofyAbility::RevengeBash => "Revenge Bash",
            GoofyAbility::AllyGuard => "Ally Guard",
            GoofyAbility::PartyBreaker => "Party Breaker",
            GoofyAbility::LaunchBreaker => "Launch Breaker",
            GoofyAbility::RallyBreaker => "Rally Breaker",
            GoofyAbility::DeepBreaker => "Deep Breaker",
            GoofyAbility::ResolveKeeper => "Resolve Keeper",
            GoofyAbility::ZoneExtender => "Zone Extender",
            GoofyAbility::ForwardDeflectPlus => "Forward Deflect+",
            GoofyAbility::LoudMouth => "Loud Mouth",
            GoofyAbility::Bodyguard => "Bodyguard",
            GoofyAbility::SecondWind => "Second Wind",
        }
    }

    /// The menu's help line (KH3's ability help style).
    pub const fn description(self) -> &'static str {
        match self {
            // COMPOSED (THE USER 2026-11-02: "Danger Sense ability should indicate what kind of attack it will be"; 2026-11-05:
            // "should not make the cue come earlier")
            GoofyAbility::DangerSense => "The danger cue names the attack.",
            GoofyAbility::SteadyGuard => "Guarding costs less GUARD.",
            GoofyAbility::IronPosture => "GUARD refills faster.",
            // COMPOSED (THE USER 2026-11-02: "The parries are there to make the enemy's stagger go up fast")
            GoofyAbility::StaggerBoost => "Parries fill the enemy's Stagger gauge 20% more.",
            GoofyAbility::SecondDeflect => "Forgives one mistimed deflect per string of attacks.",
            GoofyAbility::RevengeBash => "After a Guard Break, the next block counts as a perfect deflect: full Stagger and MP back.",
            GoofyAbility::AllyGuard => "Automatically deflects a blow aimed at a nearby friend.",
            GoofyAbility::PartyBreaker => "Nearby friends join in on Goofy Breaker.",
            GoofyAbility::LaunchBreaker => "Goofy Breaker launches the enemy for air juggles.",
            GoofyAbility::RallyBreaker => "Goofy Breaker restores the party's HP.",
            GoofyAbility::DeepBreaker => "Increases the damage of Goofy Breaker.",
            GoofyAbility::ResolveKeeper => "A hit only removes one pip of Resolve.",
            GoofyAbility::ZoneExtender => "In the Zone lasts through one hit.",
            GoofyAbility::ForwardDeflectPlus => "Deflecting while moving toward the enemy loses less timing.",
            GoofyAbility::LoudMouth => "Draws more of the enemies' attention.",
            GoofyAbility::Bodyguard => "Interpose costs no MP when a friend is in danger.",
            GoofyAbility::SecondWind => "Restores GUARD when HP runs low.",
        }
    }

    /// The Abilities page's category (`ETresAbilityCategory`: 0 Actions, 2 Support).
    pub const fn category(self) -> u8 {
        match self {
            GoofyAbility::StaggerBoost | GoofyAbility::SecondDeflect | GoofyAbility::RevengeBash | GoofyAbility::AllyGuard | GoofyAbility::PartyBreaker | GoofyAbility::LaunchBreaker | GoofyAbility::RallyBreaker | GoofyAbility::DeepBreaker => 0,
            _ => 2,
        }
    }

    /// The save / reward name (`GOF_DANGER_SENSE` ...).
    pub fn name(self) -> String {
        let s = format!("{self:?}");
        let mut out = String::from("GOF_");
        for (i, c) in s.chars().enumerate() {
            if c.is_ascii_uppercase() && i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_uppercase());
        }
        out
    }

    /// By the save name, with or without `GOF_` / `kit:`, any case.
    pub fn from_name(name: &str) -> Option<GoofyAbility> {
        let n = name.trim().to_ascii_uppercase();
        let n = n.strip_prefix("KIT:").unwrap_or(&n);
        let n = n.strip_prefix("GOF_").unwrap_or(n);
        // (THE USER 2026-11-02) an old save's Counter Chain+ is Stagger Boost now: owned and worn as it was, the same AP
        let n = if n == "COUNTER_CHAIN_PLUS" { "STAGGER_BOOST" } else { n };
        GoofyAbility::ALL.into_iter().find(|k| k.name().trim_start_matches("GOF_") == n)
    }

    /// AP (sheet `ability.*.ap`).
    pub fn ap(self) -> i32 {
        (match self {
            GoofyAbility::DangerSense => K.ability_danger_sense_ap,
            GoofyAbility::SteadyGuard => K.ability_steady_guard_ap,
            GoofyAbility::IronPosture => K.ability_iron_posture_ap,
            GoofyAbility::StaggerBoost => K.ability_stagger_boost_ap,
            GoofyAbility::SecondDeflect => K.ability_second_deflect_ap,
            GoofyAbility::RevengeBash => K.ability_revenge_bash_ap,
            GoofyAbility::AllyGuard => K.ability_ally_guard_ap,
            GoofyAbility::PartyBreaker => K.ability_party_breaker_ap,
            GoofyAbility::LaunchBreaker => K.ability_launch_breaker_ap,
            GoofyAbility::RallyBreaker => K.ability_rally_breaker_ap,
            GoofyAbility::DeepBreaker => K.ability_deep_breaker_ap,
            GoofyAbility::ResolveKeeper => K.ability_resolve_keeper_ap,
            GoofyAbility::ZoneExtender => K.ability_zone_extender_ap,
            GoofyAbility::ForwardDeflectPlus => K.ability_forward_deflect_plus_ap,
            GoofyAbility::LoudMouth => K.ability_loud_mouth_ap,
            GoofyAbility::Bodyguard => K.ability_bodyguard_ap,
            GoofyAbility::SecondWind => K.ability_second_wind_ap,
        }) as i32
    }

    /// The level he learns it at (sheet `ability.*.level`).
    pub fn level(self) -> u32 {
        (match self {
            GoofyAbility::DangerSense => K.ability_danger_sense_level,
            GoofyAbility::SteadyGuard => K.ability_steady_guard_level,
            GoofyAbility::IronPosture => K.ability_iron_posture_level,
            GoofyAbility::StaggerBoost => K.ability_stagger_boost_level,
            GoofyAbility::SecondDeflect => K.ability_second_deflect_level,
            GoofyAbility::RevengeBash => K.ability_revenge_bash_level,
            GoofyAbility::AllyGuard => K.ability_ally_guard_level,
            GoofyAbility::PartyBreaker => K.ability_party_breaker_level,
            GoofyAbility::LaunchBreaker => K.ability_launch_breaker_level,
            GoofyAbility::RallyBreaker => K.ability_rally_breaker_level,
            GoofyAbility::DeepBreaker => K.ability_deep_breaker_level,
            GoofyAbility::ResolveKeeper => K.ability_resolve_keeper_level,
            GoofyAbility::ZoneExtender => K.ability_zone_extender_level,
            GoofyAbility::ForwardDeflectPlus => K.ability_forward_deflect_plus_level,
            GoofyAbility::LoudMouth => K.ability_loud_mouth_level,
            GoofyAbility::Bodyguard => K.ability_bodyguard_level,
            GoofyAbility::SecondWind => K.ability_second_wind_level,
        }) as u32
    }

    /// In the order of their levels.
    pub fn level_order() -> [GoofyAbility; 17] {
        let mut all = GoofyAbility::ALL;
        all.sort_by_key(|k| k.level());
        all
    }
}

/// Goofy's abilities: owned and equipped, a bit each.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub struct GoofyAbilities {
    owned: u32,
    equipped: u32,
}

impl GoofyAbilities {
    /// Everything owned and equipped (the test level).
    pub fn test_all() -> GoofyAbilities {
        let all = GoofyAbility::ALL.iter().fold(0, |m, k| m | k.bit());
        GoofyAbilities { owned: all, equipped: all }
    }

    pub fn owned(&self, k: GoofyAbility) -> bool {
        self.owned & k.bit() != 0
    }

    pub fn equipped(&self, k: GoofyAbility) -> bool {
        self.equipped & k.bit() != 0
    }

    /// Owned and equipped: it works.
    pub fn has(&self, k: GoofyAbility) -> bool {
        self.owned & self.equipped & k.bit() != 0
    }

    /// Owns `k` (not equipped). True when new.
    pub fn give(&mut self, k: GoofyAbility) -> bool {
        let new = !self.owned(k);
        self.owned |= k.bit();
        new
    }

    pub fn take(&mut self, k: GoofyAbility) {
        self.owned &= !k.bit();
        self.equipped &= !k.bit();
    }

    /// Equips / unequips an owned one (the caller checks the AP). False when not owned.
    pub fn set_equipped(&mut self, k: GoofyAbility, on: bool) -> bool {
        if !self.owned(k) {
            return false;
        }
        if on {
            self.equipped |= k.bit();
        } else {
            self.equipped &= !k.bit();
        }
        true
    }

    /// Every owned one in menu order, with its equipped bit.
    pub fn list(&self) -> Vec<(GoofyAbility, bool)> {
        GoofyAbility::ALL.into_iter().filter(|k| self.owned(*k)).map(|k| (k, self.equipped(k))).collect()
    }

    pub fn ap_in_use(&self) -> i32 {
        GoofyAbility::ALL.iter().filter(|k| self.has(**k)).map(|k| k.ap()).sum()
    }

    /// The save record's entries (name, 1 copy, equipped bit).
    pub fn record(&self) -> Vec<(String, u8, u8)> {
        self.list().into_iter().map(|(k, on)| (k.name(), 1, on as u8)).collect()
    }

    /// One record entry onto the set; false when the name is not one of these (`GOF_*`): the caller tries the others.
    pub fn apply_record(&mut self, name: &str, owned: u8, mask: u8) -> bool {
        if !name.trim().to_ascii_uppercase().starts_with("GOF_") {
            return false;
        }
        let Some(k) = GoofyAbility::from_name(name) else { return true };
        if owned > 0 {
            self.give(k);
            self.set_equipped(k, mask & 1 != 0);
        } else {
            self.take(k);
        }
        true
    }
}

/// KH3's ability seeds that drove his old AI trees and his old list (R11): no longer given; an old save's are dropped.
pub const KH3_SEEDS: [&str; 5] = ["GOOFY_TORNADE", "GOOFY_TURBO", "GOOFY_FLIP", "DEFENDER", "HARVEST"];

/// KH3's rows that answer his old tree's requests (`HasAbility:`, the MP gate), and the Art that stands for each now.
const TREE_ARTS: [(&str, GoofyMove); 3] = [("GOOFY_TORNADE", GoofyMove::GoofyTornado), ("GOOFY_TURBO", GoofyMove::GoofyTurbo), ("GOOFY_FLIP", GoofyMove::ShieldToss)];

impl FriendStatus {
    /// (GOOFY) The ability is his, worn and working.
    pub fn gof_has(&self, k: GoofyAbility) -> bool {
        self.goofy.as_ref().is_some_and(|g| g.abilities.has(k))
    }

    /// (GOOFY) The rank of a technique or an Art now (0: not learned; 0 for anyone but Goofy).
    pub fn gof_rank(&self, m: GoofyMove) -> u8 {
        if self.goofy.is_some() {
            learn::rank(&self.growth.ranks, m, self.level)
        } else {
            0
        }
    }

    /// (GOOFY) His own abilities' AP in use.
    pub fn gof_ap_in_use(&self) -> i32 {
        self.goofy.as_ref().map_or(0, |g| g.abilities.ap_in_use())
    }

    /// (GOOFY) Wear / take off one of his abilities, with the AP check. False = refused.
    pub fn gof_equip(&mut self, k: GoofyAbility, on: bool) -> bool {
        let Some(g) = self.goofy.as_ref() else { return false };
        if on && !g.abilities.equipped(k) && self.ap_in_use() + k.ap() > self.max_ap() {
            return false;
        }
        self.goofy.as_mut().is_some_and(|g| g.abilities.set_equipped(k, on))
    }

    /// (GOOFY) The abilities his level has reached that he does not own yet, in the order of their levels: owned, and worn
    /// while his AP allows (as KH3's friends' new abilities come on). Item Boost, KH3's, comes by level too (R16). Returns the
    /// new ones.
    pub fn gof_sync_level(&mut self) -> Vec<GoofyAbility> {
        let mut out = Vec::new();
        if self.goofy.is_none() {
            return out;
        }
        let level = self.level;
        for k in GoofyAbility::level_order() {
            if level < k.level() || self.goofy.as_ref().is_some_and(|g| g.abilities.owned(k)) {
                continue;
            }
            let fits = self.ap_in_use() + k.ap() <= self.max_ap();
            let Some(g) = self.goofy.as_mut() else { return out };
            g.abilities.give(k);
            if fits {
                g.abilities.set_equipped(k, true);
            }
            out.push(k);
        }
        if level >= K.ability_item_boost_level as u32 && self.abilities.owned(A::ItemUp) == 0 {
            let on = self.ap_in_use() + crate::status::kh3_ability_ap(A::ItemUp) <= self.max_ap();
            self.abilities.set(A::ItemUp, 1, on as u8);
        }
        out
    }

    /// (GOOFY) Second Chance and Withstand Combo on Goofy, KH3's rule: a blow that would knock him out from above 1 HP leaves 1; a
    /// blow in a running reaction too, with Withstand Combo.
    pub fn gof_survive(&self, before: i32, new_hp: i32, in_combo: bool) -> i32 {
        if new_hp >= 1 || self.goofy.is_none() {
            return new_hp;
        }
        if (before > 1 && self.count(A::LastLeave) > 0) || (before >= 1 && in_combo && self.count(A::ComboLeave) > 0) {
            return 1;
        }
        new_hp
    }

    /// (GOOFY) An old save's KH3 seeds (R11): dropped, once (the next save no longer lists them, their AP is free). Returns
    /// the names.
    pub fn gof_drop_kh3(&mut self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.goofy.is_none() {
            return out;
        }
        for name in KH3_SEEDS {
            if let Some(k) = A::from_game_name(name).filter(|k| self.abilities.owned(*k) > 0) {
                self.abilities.set(k, 0, 0);
                out.push(name);
            }
        }
        out
    }

    /// (GOOFY) His old tree's ability gate: the Art that stands for the KH3 row is learned. None: not one of those rows.
    pub fn gof_tree_gate(&self, key: &str) -> Option<bool> {
        self.goofy.as_ref()?;
        TREE_ARTS.iter().find(|(row, _)| *row == key).map(|(_, m)| self.gof_rank(*m) > 0)
    }

    /// (GOOFY) `HasAbility:` of his tree for KH3's rows ([`Self::gof_tree_gate`]).
    pub fn gof_has_ability(&self, kind: A) -> Option<bool> {
        self.gof_tree_gate(kind.game_name().trim_start_matches("ETresAbilityKind::"))
    }
}
