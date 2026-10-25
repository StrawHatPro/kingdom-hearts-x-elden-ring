//! (DON1) DONALD'S ABILITIES (documentation/DONALD_OVERHAUL.md 9, 5.4, 10): the ids, the owned / equipped set, the save
//! names (`DON_*`, the v5 friends record's ability list) and the switches the shared rules read ([`DonMods`]).
//!
//! The ids and labels are the doc's. Stream DON-PROG owns the rest of each ability (description, AP, the level it is
//! learned at, the KH3 standard ones' wiring) in its own file (`donald/ability_info.rs`), and the replacement of the
//! X3D kit abilities (pad/donald_kit.rs KIT_*: an old save's KIT_SPELL_WEAVER / KIT_AERO_FLOAT map onto these).

/// One of Donald's abilities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DonAbility {
    // combat
    ComboPlus,
    ComboPlus2,
    AirComboPlus,
    ChargedMissilePlus,
    MissileVolley,
    ArcaneBolt,
    Starfall,
    WandBonkPlus,
    CastGuard,
    SecondChance,
    WithstandCombo,
    // magic
    FireBoost,
    BlizzardBoost,
    ThunderBoost,
    WaterBoost,
    AeroBoost,
    CureBoost,
    MpHaste,
    MpHastera,
    MpHastega,
    MpRage,
    SpellWeaver,
    ReactionPlus,
    ResonanceKeeper,
    SwiftAttune,
    InstantAttune,
    DualAttunement,
    QuickCast,
    SpellEcho,
    Scan,
    ElementalInsight,
    ArcaneFlowPlus,
    ArcaneBoost,
    // support (party)
    LowProfile,
    HideBehind,
    Distraction,
    TauntPass,
    OutOfSight,
    AuraRangePlus,
    LuckyLucky,
    TreasureMagnet,
    ItemBoost,
    // movement (8: kept from X3D, the Red Wolf of Radagon reward)
    AeroFloat,
}

impl DonAbility {
    pub const ALL: [DonAbility; 43] = [
        DonAbility::ComboPlus,
        DonAbility::ComboPlus2,
        DonAbility::AirComboPlus,
        DonAbility::ChargedMissilePlus,
        DonAbility::MissileVolley,
        DonAbility::ArcaneBolt,
        DonAbility::Starfall,
        DonAbility::WandBonkPlus,
        DonAbility::CastGuard,
        DonAbility::SecondChance,
        DonAbility::WithstandCombo,
        DonAbility::FireBoost,
        DonAbility::BlizzardBoost,
        DonAbility::ThunderBoost,
        DonAbility::WaterBoost,
        DonAbility::AeroBoost,
        DonAbility::CureBoost,
        DonAbility::MpHaste,
        DonAbility::MpHastera,
        DonAbility::MpHastega,
        DonAbility::MpRage,
        DonAbility::SpellWeaver,
        DonAbility::ReactionPlus,
        DonAbility::ResonanceKeeper,
        DonAbility::SwiftAttune,
        DonAbility::InstantAttune,
        DonAbility::DualAttunement,
        DonAbility::QuickCast,
        DonAbility::SpellEcho,
        DonAbility::Scan,
        DonAbility::ElementalInsight,
        DonAbility::ArcaneFlowPlus,
        DonAbility::ArcaneBoost,
        DonAbility::LowProfile,
        DonAbility::HideBehind,
        DonAbility::Distraction,
        DonAbility::TauntPass,
        DonAbility::OutOfSight,
        DonAbility::AuraRangePlus,
        DonAbility::LuckyLucky,
        DonAbility::TreasureMagnet,
        DonAbility::ItemBoost,
        DonAbility::AeroFloat,
    ];

    const fn bit(self) -> u64 {
        1 << self as u64
    }

    /// The menu's English name (the doc's).
    pub const fn label(self) -> &'static str {
        match self {
            DonAbility::ComboPlus | DonAbility::ComboPlus2 => "Combo Plus",
            DonAbility::AirComboPlus => "Air Combo Plus",
            DonAbility::ChargedMissilePlus => "Charged Missile+",
            DonAbility::MissileVolley => "Missile Volley",
            DonAbility::ArcaneBolt => "Arcane Bolt",
            DonAbility::Starfall => "Air Finisher: Starfall",
            DonAbility::WandBonkPlus => "Wand Bonk+",
            DonAbility::CastGuard => "Cast Guard",
            DonAbility::SecondChance => "Second Chance",
            DonAbility::WithstandCombo => "Withstand Combo",
            DonAbility::FireBoost => "Fire Boost",
            DonAbility::BlizzardBoost => "Blizzard Boost",
            DonAbility::ThunderBoost => "Thunder Boost",
            DonAbility::WaterBoost => "Water Boost",
            DonAbility::AeroBoost => "Aero Boost",
            DonAbility::CureBoost => "Cure Boost",
            DonAbility::MpHaste => "MP Haste",
            DonAbility::MpHastera => "MP Hastera",
            DonAbility::MpHastega => "MP Hastega",
            DonAbility::MpRage => "MP Rage",
            DonAbility::SpellWeaver => "Spell Weaver",
            DonAbility::ReactionPlus => "Reaction Plus",
            DonAbility::ResonanceKeeper => "Resonance Keeper",
            DonAbility::SwiftAttune => "Swift Attune",
            DonAbility::InstantAttune => "Instant Attune",
            DonAbility::DualAttunement => "Dual Attunement",
            DonAbility::QuickCast => "Quick Cast",
            DonAbility::SpellEcho => "Spell Echo",
            DonAbility::Scan => "Scan",
            DonAbility::ElementalInsight => "Elemental Insight",
            DonAbility::ArcaneFlowPlus => "Arcane Flow+",
            DonAbility::ArcaneBoost => "Arcane Boost",
            DonAbility::LowProfile => "Low Profile",
            DonAbility::HideBehind => "Hide Behind",
            DonAbility::Distraction => "Distraction",
            DonAbility::TauntPass => "Taunt Pass",
            DonAbility::OutOfSight => "Out of Sight",
            DonAbility::AuraRangePlus => "Aura Range+",
            DonAbility::LuckyLucky => "Lucky Lucky",
            DonAbility::TreasureMagnet => "Treasure Magnet",
            DonAbility::ItemBoost => "Item Boost",
            DonAbility::AeroFloat => "Aero Float",
        }
    }

    /// The save / reward name (`DON_COMBO_PLUS`, `DON_COMBO_PLUS_2` ...; a reward token is `don:<name>`).
    pub fn name(self) -> String {
        let s = format!("{self:?}");
        let mut out = String::from("DON_");
        for (i, c) in s.chars().enumerate() {
            if c.is_ascii_uppercase() && i > 0 {
                out.push('_');
            }
            if c.is_ascii_digit() {
                out.push('_');
            }
            out.push(c.to_ascii_uppercase());
        }
        out
    }

    /// By the save name, with or without `DON_`, any case.
    pub fn from_name(name: &str) -> Option<DonAbility> {
        let n = name.trim().to_ascii_uppercase();
        let n = n.strip_prefix("DON_").unwrap_or(&n).to_string();
        DonAbility::ALL.into_iter().find(|k| k.name().trim_start_matches("DON_") == n)
    }
}

/// The switches the shared rules read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct DonMods {
    pub swift_attune: bool,
    pub instant_attune: bool,
    pub dual: bool,
    pub resonance_keeper: bool,
    pub arcane_flow_plus: bool,
    pub reaction_plus: bool,
    pub spell_echo: bool,
}

/// One Donald's abilities: owned and equipped, a bit each.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub struct DonAbilities {
    owned: u64,
    equipped: u64,
}

impl DonAbilities {
    /// Everything owned and equipped (the test level).
    pub fn test_all() -> DonAbilities {
        let all = DonAbility::ALL.iter().fold(0, |m, k| m | k.bit());
        DonAbilities { owned: all, equipped: all }
    }

    pub fn owned(&self, k: DonAbility) -> bool {
        self.owned & k.bit() != 0
    }

    pub fn equipped(&self, k: DonAbility) -> bool {
        self.equipped & k.bit() != 0
    }

    /// Owned and equipped: it works.
    pub fn has(&self, k: DonAbility) -> bool {
        self.owned & self.equipped & k.bit() != 0
    }

    /// Owns `k` (not equipped). True when new.
    pub fn give(&mut self, k: DonAbility) -> bool {
        let new = !self.owned(k);
        self.owned |= k.bit();
        new
    }

    pub fn take(&mut self, k: DonAbility) {
        self.owned &= !k.bit();
        self.equipped &= !k.bit();
    }

    /// Equips / unequips an owned one (the caller checks the AP). False when not owned.
    pub fn set_equipped(&mut self, k: DonAbility, on: bool) -> bool {
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
    pub fn list(&self) -> Vec<(DonAbility, bool)> {
        DonAbility::ALL.into_iter().filter(|k| self.owned(*k)).map(|k| (k, self.equipped(k))).collect()
    }

    /// The switches of the shared rules.
    pub fn mods(&self) -> DonMods {
        DonMods {
            swift_attune: self.has(DonAbility::SwiftAttune),
            instant_attune: self.has(DonAbility::InstantAttune),
            dual: self.has(DonAbility::DualAttunement),
            resonance_keeper: self.has(DonAbility::ResonanceKeeper),
            arcane_flow_plus: self.has(DonAbility::ArcaneFlowPlus),
            reaction_plus: self.has(DonAbility::ReactionPlus),
            spell_echo: self.has(DonAbility::SpellEcho),
        }
    }

    /// The save record's entries (name, 1 copy, equipped bit), the v5 friends record's shape.
    pub fn record(&self) -> Vec<(String, u8, u8)> {
        self.list().into_iter().map(|(k, on)| (k.name(), 1, on as u8)).collect()
    }

    /// One record entry onto the set; false when the name is not one of these (`DON_*`): the caller tries the others.
    pub fn apply_record(&mut self, name: &str, owned: u8, mask: u8) -> bool {
        if !name.trim().to_ascii_uppercase().starts_with("DON_") {
            return false;
        }
        let Some(k) = DonAbility::from_name(name) else { return true };
        if owned > 0 {
            self.give(k);
            self.set_equipped(k, mask & 1 != 0);
        } else {
            self.take(k);
        }
        true
    }
}
