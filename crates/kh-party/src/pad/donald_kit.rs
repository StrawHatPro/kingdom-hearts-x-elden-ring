//! (party X3D -> DON-PROG) THE X3D KIT ABILITIES, NOW A MIGRATION SHIM (documentation/DONALD_OVERHAUL.md 13: "The
//! XD_PLAYABLE_KITS Donald abilities. Replaced by section 9"; build/sora_tmp/party/DON_PLAN.md section 5).
//!
//! What X3D gave Donald (Spell Weaver, Counter Blast, Quick Chant, Mana Rebound, Meteor Finale, Blink Step, Aero Float
//! and the three Unisons as finisher kit abilities) is replaced by his overhaul abilities (`crate::donald::DonAbility`):
//! - KIT_SPELL_WEAVER -> DON_SPELL_WEAVER, KIT_AERO_FLOAT -> DON_AERO_FLOAT ([`KitAbility::to_don`]);
//! - the others are dropped (Counter Blast, Quick Chant, Mana Rebound, Meteor Finale, Blink Step: their moves are gone;
//!   the Unisons stay as team attacks with friends, no longer his finisher abilities).
//!
//! The types stay so that an OLD SAVE still loads unchanged (its `KIT_*` entries of the v5 friends record land here, save.rs)
//! and so that code not yet moved to the overhaul still builds: [`FriendStatus::kit_migrate`] (called by the party host
//! every step, kh-partyhost `sync_donald`) moves what an old save had onto his overhaul abilities, once, and empties the
//! kit. Nothing gives a kit ability any more: the level sync and the Unison sync are inert, a `kit:` reward of a mapped
//! ability becomes the overhaul one. A reward token is `don:<name>` now (`kit:AERO_FLOAT` is still read).

use crate::donald::DonAbility;

/// One of the X3D kit's abilities (kept for old saves and old reward tokens).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KitAbility {
    SpellWeaver,
    CounterBlast,
    QuickChant,
    ManaRebound,
    MeteorFinale,
    BlinkStep,
    AeroFloat,
    UnisonFire,
    UnisonBlizzard,
    UnisonThunder,
}

impl KitAbility {
    pub const ALL: [KitAbility; 10] = [
        KitAbility::SpellWeaver,
        KitAbility::CounterBlast,
        KitAbility::QuickChant,
        KitAbility::ManaRebound,
        KitAbility::MeteorFinale,
        KitAbility::BlinkStep,
        KitAbility::AeroFloat,
        KitAbility::UnisonFire,
        KitAbility::UnisonBlizzard,
        KitAbility::UnisonThunder,
    ];

    /// (DON-PROG) None is learned by level any more (his level tree is the overhaul's: `DonAbility::level`).
    pub const OWN: [KitAbility; 0] = [];

    const fn bit(self) -> u16 {
        1 << self as u16
    }

    /// The save / reward name (`KIT_SPELL_WEAVER` ...).
    pub const fn name(self) -> &'static str {
        match self {
            KitAbility::SpellWeaver => "KIT_SPELL_WEAVER",
            KitAbility::CounterBlast => "KIT_COUNTER_BLAST",
            KitAbility::QuickChant => "KIT_QUICK_CHANT",
            KitAbility::ManaRebound => "KIT_MANA_REBOUND",
            KitAbility::MeteorFinale => "KIT_METEOR_FINALE",
            KitAbility::BlinkStep => "KIT_BLINK_STEP",
            KitAbility::AeroFloat => "KIT_AERO_FLOAT",
            KitAbility::UnisonFire => "KIT_UNISON_FIRE",
            KitAbility::UnisonBlizzard => "KIT_UNISON_BLIZZARD",
            KitAbility::UnisonThunder => "KIT_UNISON_THUNDER",
        }
    }

    /// The menu's English name.
    pub const fn label(self) -> &'static str {
        match self {
            KitAbility::SpellWeaver => "Spell Weaver",
            KitAbility::CounterBlast => "Counter Blast",
            KitAbility::QuickChant => "Quick Chant",
            KitAbility::ManaRebound => "Mana Rebound",
            KitAbility::MeteorFinale => "Meteor Finale",
            KitAbility::BlinkStep => "Blink Step",
            KitAbility::AeroFloat => "Aero Float",
            KitAbility::UnisonFire => "Unison Fire",
            KitAbility::UnisonBlizzard => "Unison Blizzard",
            KitAbility::UnisonThunder => "Unison Thunder",
        }
    }

    /// By the save name, with or without `KIT_`, any case (`AERO_FLOAT`, `kit_aero_float`).
    pub fn from_name(name: &str) -> Option<KitAbility> {
        let n = name.trim().to_ascii_uppercase();
        let n = n.strip_prefix("KIT_").unwrap_or(&n).to_string();
        KitAbility::ALL.into_iter().find(|k| k.name().trim_start_matches("KIT_") == n)
    }

    /// (DON-PROG) THE MIGRATION: the overhaul ability that replaces it (doc 13), None when it is dropped.
    pub const fn to_don(self) -> Option<DonAbility> {
        match self {
            KitAbility::SpellWeaver => Some(DonAbility::SpellWeaver),
            KitAbility::AeroFloat => Some(DonAbility::AeroFloat),
            _ => None,
        }
    }

    /// The old AP (X3D's values, fixed here: the sheet's old ability rows are gone with the overhaul). Only an old
    /// save's kit carries them until [`FriendStatus::kit_migrate`] empties it.
    pub fn ap(self) -> i32 {
        match self {
            KitAbility::SpellWeaver | KitAbility::CounterBlast => 3,
            KitAbility::QuickChant | KitAbility::ManaRebound => 4,
            KitAbility::MeteorFinale => 5,
            KitAbility::BlinkStep => 2,
            KitAbility::AeroFloat => 0,
            KitAbility::UnisonFire | KitAbility::UnisonBlizzard | KitAbility::UnisonThunder => 3,
        }
    }

    /// (DON-PROG) None: no kit ability is learned by level any more.
    pub fn level(self) -> Option<u32> {
        None
    }

    pub const UNISONS: [KitAbility; 3] = [KitAbility::UnisonFire, KitAbility::UnisonBlizzard, KitAbility::UnisonThunder];
}

/// The kit's abilities of one friend: owned and equipped, a bit each (an old save's, until [`FriendStatus::kit_migrate`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct KitAbilities {
    owned: u16,
    equipped: u16,
}

impl KitAbilities {
    /// Every kit ability owned and equipped (old tests of the X3D moves).
    pub fn test_all() -> KitAbilities {
        let all = KitAbility::ALL.iter().fold(0, |m, k| m | k.bit());
        KitAbilities { owned: all, equipped: all }
    }

    pub fn owned(&self, k: KitAbility) -> bool {
        self.owned & k.bit() != 0
    }

    /// Owned and equipped.
    pub fn has(&self, k: KitAbility) -> bool {
        self.owned & self.equipped & k.bit() != 0
    }

    pub fn equipped(&self, k: KitAbility) -> bool {
        self.equipped & k.bit() != 0
    }

    /// The AP of the equipped (and owned) abilities.
    pub fn ap_in_use(&self) -> i32 {
        KitAbility::ALL.iter().filter(|k| self.has(**k)).map(|k| k.ap()).sum()
    }

    /// Owns `k` (not equipped). Returns true when it is new.
    pub fn give(&mut self, k: KitAbility) -> bool {
        let new = !self.owned(k);
        self.owned |= k.bit();
        new
    }

    pub fn take(&mut self, k: KitAbility) {
        self.owned &= !k.bit();
        self.equipped &= !k.bit();
    }

    /// Equips / unequips an owned ability. False when not owned.
    pub fn set_equipped(&mut self, k: KitAbility, on: bool) -> bool {
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

    /// Every owned ability, in the menu's order, with its equipped bit.
    pub fn list(&self) -> Vec<(KitAbility, bool)> {
        KitAbility::ALL.into_iter().filter(|k| self.owned(*k)).map(|k| (k, self.equipped(k))).collect()
    }

    /// The save record's entries (an old save's kit until migrated; empty afterwards).
    pub fn record(&self) -> Vec<(String, u8, u8)> {
        self.list().into_iter().map(|(k, on)| (k.name().to_string(), 1, on as u8)).collect()
    }

    /// One entry of a save record onto the set; false when the name is not a kit ability (the caller tries KH3's).
    pub fn apply_record(&mut self, name: &str, owned: u8, mask: u8) -> bool {
        if !name.starts_with("KIT_") {
            return false;
        }
        let Some(k) = KitAbility::from_name(name) else { return true };
        if owned > 0 {
            self.give(k);
            self.set_equipped(k, mask & 1 != 0);
        } else {
            self.take(k);
        }
        true
    }
}

impl crate::status::FriendStatus {
    /// (DON-PROG) The X3D kit is gone: an empty kit (the test loadout of the overhaul is `don_test_loadout`).
    pub fn kit_test_loadout(&mut self, _bonus: &[&str]) {
        self.kit = KitAbilities::default();
    }

    /// (DON-PROG) A kit ability given (an old `kit:` reward token): its overhaul ability when it has one (owned, equipped
    /// while his AP allows). Returns (new, equipped); (false, false) for a dropped one.
    pub fn kit_grant(&mut self, k: KitAbility) -> (bool, bool) {
        k.to_don().and_then(|d| self.don_grant(d)).unwrap_or((false, false))
    }

    /// (DON-PROG) Inert: nothing is learned into the kit by level (his level tree: `don_sync_level`).
    pub fn kit_sync_level(&mut self) -> Vec<KitAbility> {
        Vec::new()
    }

    /// (DON-PROG) Inert: the Unisons are no longer his finisher abilities (doc 13; their team attacks stay).
    pub fn kit_sync_unisons(&mut self, _sora: [u8; 3]) {}

    /// (DON-PROG) THE OLD SAVE'S KIT ONTO THE OVERHAUL (once: the kit is emptied): KIT_SPELL_WEAVER -> DON_SPELL_WEAVER,
    /// KIT_AERO_FLOAT -> DON_AERO_FLOAT with their equipped bits (worn only while his AP allows); the others dropped.
    /// Returns the abilities it gave. Nothing for Goofy (his kit is `goofy_kit`) or an empty kit.
    pub fn kit_migrate(&mut self) -> Vec<DonAbility> {
        let mut out = Vec::new();
        if self.donald.is_none() || self.kit == KitAbilities::default() {
            return out;
        }
        let old = std::mem::take(&mut self.kit);
        for (k, on) in old.list() {
            let Some(d) = k.to_don() else { continue };
            let fits = self.ap_in_use() + d.ap() <= self.max_ap();
            if let Some(st) = self.donald.as_mut() {
                st.abilities.give(d);
                st.abilities.set_equipped(d, on && fits);
            }
            out.push(d);
        }
        out
    }
}
