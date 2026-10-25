//! (party X3G) GOOFY'S OWN ABILITIES (build/sora_tmp/party/XD_PLAYABLE_KITS.md 2.2 / 2.5 / 2.6, approved by the user
//! 2026-10-15): Iron Wall, Payback Bash, Shield Charge, Stalwart, Rally, Ricochet, Over Here! (his own AP), Tornado
//! Copter (no AP: it comes with Sora's Glide, decision 8) and the two Fusions Sora has learned (decision 3: Goofy wears
//! them for his own AP). KH3 has none of these for Goofy: they are THE PORT'S abilities, kept apart from KH3's
//! `ETresAbilityKind` set (no game enum value is reused). Every number is a row of `sheets/port/party-kit-goofy.json`.
//!
//! Where they live: `FriendStatus::goofy_kit` (one per friend; only Goofy's is ever filled). The AP they take is part of
//! `FriendStatus::ap_in_use`. Saves: `GoofyKit::record` / `apply_record` (names `KIT_*`; the same shape as Donald's
//! `KitAbilities`, pad/donald_kit.rs).

include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_kit_goofy.rs"));

/// The kit's numbers (sheet `party-kit-goofy`).
pub const K: PartyKitGoofyParams = PARTY_KIT_GOOFY_PARAMS;

/// One of Goofy's kit abilities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GoofyAbility {
    IronWall,
    PaybackBash,
    ShieldCharge,
    Stalwart,
    Rally,
    Ricochet,
    OverHere,
    TornadoCopter,
    FusionSpin,
    FusionRocket,
    // (GOOFY-GK) the tank rework's combo abilities (build/sora_tmp/party/GK_GOOFY_DESIGN.md section 8)
    ComboPlus,
    AirComboPlus,
    ShieldQuake,
    ComboPlus2,
    ShieldUpper,
}

impl GoofyAbility {
    pub const ALL: [GoofyAbility; 15] = [
        GoofyAbility::IronWall,
        GoofyAbility::PaybackBash,
        GoofyAbility::ShieldCharge,
        GoofyAbility::Stalwart,
        GoofyAbility::Rally,
        GoofyAbility::Ricochet,
        GoofyAbility::OverHere,
        GoofyAbility::TornadoCopter,
        GoofyAbility::FusionSpin,
        GoofyAbility::FusionRocket,
        GoofyAbility::ComboPlus, // (GOOFY-GK)
        GoofyAbility::AirComboPlus,
        GoofyAbility::ShieldQuake,
        GoofyAbility::ComboPlus2,
        GoofyAbility::ShieldUpper,
    ];

    /// Goofy's own (learned by level): X3G's seven and (GOOFY-GK) the five combo abilities.
    pub const OWN: [GoofyAbility; 12] = [GoofyAbility::IronWall, GoofyAbility::PaybackBash, GoofyAbility::ShieldCharge, GoofyAbility::Stalwart, GoofyAbility::Rally, GoofyAbility::Ricochet, GoofyAbility::OverHere, GoofyAbility::ComboPlus, GoofyAbility::AirComboPlus, GoofyAbility::ShieldQuake, GoofyAbility::ComboPlus2, GoofyAbility::ShieldUpper];

    /// The two Fusions (Sora's abilities, decision 3).
    pub const FUSIONS: [GoofyAbility; 2] = [GoofyAbility::FusionSpin, GoofyAbility::FusionRocket];

    const fn bit(self) -> u16 {
        1 << self as u16
    }

    /// The save / reward name (`KIT_IRON_WALL` ...; a reward token would be `kit:TORNADO_COPTER`).
    pub const fn name(self) -> &'static str {
        match self {
            GoofyAbility::IronWall => "KIT_IRON_WALL",
            GoofyAbility::PaybackBash => "KIT_PAYBACK_BASH",
            GoofyAbility::ShieldCharge => "KIT_SHIELD_CHARGE",
            GoofyAbility::Stalwart => "KIT_STALWART",
            GoofyAbility::Rally => "KIT_RALLY",
            GoofyAbility::Ricochet => "KIT_RICOCHET",
            GoofyAbility::OverHere => "KIT_OVER_HERE",
            GoofyAbility::TornadoCopter => "KIT_TORNADO_COPTER",
            GoofyAbility::FusionSpin => "KIT_FUSION_SPIN",
            GoofyAbility::FusionRocket => "KIT_FUSION_ROCKET",
            GoofyAbility::ComboPlus => "KIT_COMBO_PLUS",
            GoofyAbility::AirComboPlus => "KIT_AIR_COMBO_PLUS",
            GoofyAbility::ShieldQuake => "KIT_SHIELD_QUAKE",
            GoofyAbility::ComboPlus2 => "KIT_COMBO_PLUS_2",
            GoofyAbility::ShieldUpper => "KIT_SHIELD_UPPER",
        }
    }

    /// The menu's English name.
    pub const fn label(self) -> &'static str {
        match self {
            GoofyAbility::IronWall => "Iron Wall",
            GoofyAbility::PaybackBash => "Payback Bash",
            GoofyAbility::ShieldCharge => "Shield Charge",
            GoofyAbility::Stalwart => "Stalwart",
            GoofyAbility::Rally => "Rally",
            GoofyAbility::Ricochet => "Ricochet",
            GoofyAbility::OverHere => "Over Here!",
            GoofyAbility::TornadoCopter => "Tornado Copter",
            GoofyAbility::FusionSpin => "Fusion Spin",
            GoofyAbility::FusionRocket => "Fusion Rocket",
            GoofyAbility::ComboPlus | GoofyAbility::ComboPlus2 => "Combo Plus",
            GoofyAbility::AirComboPlus => "Air Combo Plus",
            GoofyAbility::ShieldQuake => "Shield Quake",
            GoofyAbility::ShieldUpper => "Shield Upper",
        }
    }

    /// (GOOFY-GK) The menu's English description (KH3's ability help style).
    pub const fn description(self) -> &'static str {
        match self {
            GoofyAbility::IronWall => "Widens the window for a Perfect Guard.",
            GoofyAbility::PaybackBash => "Counter with a powerful bash after blocking any attack.",
            GoofyAbility::ShieldCharge => "Move the stick and attack while guarding to charge forward behind the shield.",
            GoofyAbility::Stalwart => "Goofy doesn't flinch from light attacks while attacking or guarding.",
            GoofyAbility::Rally => "Perfect Guards restore MP.",
            GoofyAbility::Ricochet => "Goofy Discus bounces to one more enemy.",
            GoofyAbility::OverHere => "Shield bashes draw nearby enemies' attention to Goofy.",
            GoofyAbility::TornadoCopter => "Hold jump while falling to glide.",
            GoofyAbility::FusionSpin => "Finish a ground combo among several enemies with a whirling Fusion Spin.",
            GoofyAbility::FusionRocket => "Push the stick toward a distant target on a finishing blow to rocket into it.",
            GoofyAbility::ComboPlus | GoofyAbility::ComboPlus2 => "Adds one more hit to Goofy's ground combo.",
            GoofyAbility::AirComboPlus => "Adds a shield bash to the end of Goofy's air combo.",
            GoofyAbility::ShieldQuake => "Hold the attack button on a combo's finishing blow to slam the ground with Heavy Bash.",
            GoofyAbility::ShieldUpper => "Hold the stick away from the target on a finishing blow to launch it skyward.",
        }
    }

    pub fn from_name(name: &str) -> Option<GoofyAbility> {
        let n = name.trim();
        let n = n.strip_prefix("kit:").unwrap_or(n);
        GoofyAbility::ALL.into_iter().find(|k| k.name() == n || k.name().trim_start_matches("KIT_") == n)
    }

    /// AP (sheet `ability.*.ap`).
    pub fn ap(self) -> i32 {
        (match self {
            GoofyAbility::IronWall => K.ability_iron_wall_ap,
            GoofyAbility::PaybackBash => K.ability_payback_bash_ap,
            GoofyAbility::ShieldCharge => K.ability_shield_charge_ap,
            GoofyAbility::Stalwart => K.ability_stalwart_ap,
            GoofyAbility::Rally => K.ability_rally_ap,
            GoofyAbility::Ricochet => K.ability_ricochet_ap,
            GoofyAbility::OverHere => K.ability_over_here_ap,
            GoofyAbility::TornadoCopter => K.ability_tornado_copter_ap,
            GoofyAbility::FusionSpin | GoofyAbility::FusionRocket => K.ability_fusion_ap,
            GoofyAbility::ComboPlus => K.ability_combo_plus_ap, // (GOOFY-GK)
            GoofyAbility::AirComboPlus => K.ability_air_combo_plus_ap,
            GoofyAbility::ShieldQuake => K.ability_shield_quake_ap,
            GoofyAbility::ComboPlus2 => K.ability_combo_plus_2_ap,
            GoofyAbility::ShieldUpper => K.ability_shield_upper_ap,
        }) as i32
    }

    /// The level Goofy learns one of his own at (sheet `ability.*.level`); None = not learned by level.
    pub fn level(self) -> Option<u32> {
        Some(match self {
            GoofyAbility::IronWall => K.ability_iron_wall_level,
            GoofyAbility::PaybackBash => K.ability_payback_bash_level,
            GoofyAbility::ShieldCharge => K.ability_shield_charge_level,
            GoofyAbility::Stalwart => K.ability_stalwart_level,
            GoofyAbility::Rally => K.ability_rally_level,
            GoofyAbility::Ricochet => K.ability_ricochet_level,
            GoofyAbility::OverHere => K.ability_over_here_level,
            GoofyAbility::ComboPlus => K.ability_combo_plus_level, // (GOOFY-GK)
            GoofyAbility::AirComboPlus => K.ability_air_combo_plus_level,
            GoofyAbility::ShieldQuake => K.ability_shield_quake_level,
            GoofyAbility::ComboPlus2 => K.ability_combo_plus_2_level,
            GoofyAbility::ShieldUpper => K.ability_shield_upper_level,
            _ => return None,
        } as u32)
    }

    /// Sora's ability whose ownership brings this one (decision 3 / 8): FUSION_SPIN / FUSION_ROCKET / GLIDE.
    pub fn sora_source(self) -> Option<&'static str> {
        match self {
            GoofyAbility::FusionSpin => Some("FUSION_SPIN"),
            GoofyAbility::FusionRocket => Some("FUSION_ROCKET"),
            GoofyAbility::TornadoCopter => Some(K.copter_unlock),
            _ => None,
        }
    }
}

/// Goofy's kit: owned and equipped bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub struct GoofyKit {
    pub owned: u16,
    pub equipped: u16,
}

impl GoofyKit {
    /// The test level's loadout (level 99): everything owned and worn.
    pub fn test_all() -> GoofyKit {
        let all = GoofyAbility::ALL.iter().fold(0u16, |a, k| a | k.bit());
        GoofyKit { owned: all, equipped: all }
    }

    pub fn owned(&self, k: GoofyAbility) -> bool {
        self.owned & k.bit() != 0
    }

    /// Owned AND equipped: the ability acts.
    pub fn has(&self, k: GoofyAbility) -> bool {
        self.owned(k) && self.equipped & k.bit() != 0
    }

    pub fn equipped(&self, k: GoofyAbility) -> bool {
        self.equipped & k.bit() != 0
    }

    pub fn ap_in_use(&self) -> i32 {
        GoofyAbility::ALL.iter().filter(|k| self.has(**k)).map(|k| k.ap()).sum()
    }

    /// Owns it (not worn). Returns true when it is new.
    pub fn give(&mut self, k: GoofyAbility) -> bool {
        let new = !self.owned(k);
        self.owned |= k.bit();
        new
    }

    pub fn take(&mut self, k: GoofyAbility) {
        self.owned &= !k.bit();
        self.equipped &= !k.bit();
    }

    /// Wear / take off (only an owned one can be worn; the AP check is the caller's: `FriendStatus::kit_goofy_equip`).
    pub fn set_equipped(&mut self, k: GoofyAbility, on: bool) -> bool {
        if on && !self.owned(k) {
            return false;
        }
        if on {
            self.equipped |= k.bit();
        } else {
            self.equipped &= !k.bit();
        }
        true
    }

    /// (ability, worn) of every owned one, in the menu's order.
    pub fn list(&self) -> Vec<(GoofyAbility, bool)> {
        GoofyAbility::ALL.iter().filter(|k| self.owned(**k)).map(|k| (*k, self.equipped(*k))).collect()
    }

    /// The save record: (name, owned 0/1, equipped 0/1) per owned ability.
    pub fn record(&self) -> Vec<(String, u8, u8)> {
        self.list().into_iter().map(|(k, on)| (k.name().to_string(), 1, on as u8)).collect()
    }

    /// One record entry back; false = not one of Goofy's kit names.
    pub fn apply_record(&mut self, name: &str, owned: u8, mask: u8) -> bool {
        let Some(k) = GoofyAbility::from_name(name) else { return false };
        if owned > 0 {
            self.give(k);
            self.set_equipped(k, mask > 0);
        } else {
            self.take(k);
        }
        true
    }
}

impl crate::status::FriendStatus {
    /// Goofy's own abilities his level has reached that he does not own yet: owned, and worn while the AP allows (the
    /// friends' new abilities come on, as KH3's friends' do). Returns the new ones.
    pub fn kit_goofy_sync_level(&mut self) -> Vec<GoofyAbility> {
        let mut out = Vec::new();
        if self.chara != crate::Chara::Goofy {
            return out;
        }
        // (GOOFY-GK) in the order of their levels: an auto-levelled Goofy (a join at a high level) wears the early ones
        // (Combo Plus at 3 ...) first, as a Goofy who levelled up step by step would
        let mut own = GoofyAbility::OWN;
        own.sort_by_key(|k| k.level().unwrap_or(u32::MAX));
        for k in own {
            if k.level().is_some_and(|l| self.level >= l) && self.goofy_kit.give(k) {
                out.push(k);
                if self.ap_in_use() + k.ap() <= self.max_ap() {
                    self.goofy_kit.set_equipped(k, true);
                }
            }
        }
        out
    }

    /// Sora's side of the shared progression (the leader feed's `sora_abilities`): a Fusion or Glide Sora owns brings
    /// Goofy's Fusion / Tornado Copter (owned; worn while the AP allows). Returns the new ones.
    pub fn kit_goofy_sync_sora(&mut self, sora: &kh_core::status::AbilitySet) -> Vec<GoofyAbility> {
        let mut out = Vec::new();
        if self.chara != crate::Chara::Goofy {
            return out;
        }
        for k in [GoofyAbility::FusionSpin, GoofyAbility::FusionRocket, GoofyAbility::TornadoCopter] {
            let Some(src) = k.sora_source().and_then(kh_core::enums::ETresAbilityKind::from_game_name) else { continue };
            if sora.owned(src) > 0 && self.goofy_kit.give(k) {
                out.push(k);
                if self.ap_in_use() + k.ap() <= self.max_ap() {
                    self.goofy_kit.set_equipped(k, true);
                }
            }
        }
        out
    }

    /// Wear / take off one of Goofy's kit abilities, with the AP check. False = refused.
    pub fn kit_goofy_equip(&mut self, k: GoofyAbility, on: bool) -> bool {
        if on && !self.goofy_kit.equipped(k) && self.ap_in_use() + k.ap() > self.max_ap() {
            return false;
        }
        self.goofy_kit.set_equipped(k, on)
    }
}
