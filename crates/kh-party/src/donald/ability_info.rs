//! (DON-PROG) DONALD'S ABILITIES, THE REST OF EACH ONE (documentation/DONALD_OVERHAUL.md 9, 10): the in-game
//! description, the AP cost (Sora's KH3 cost where KH3 has the ability, ours otherwise), the level he learns it at and
//! the test level's loadout order, plus THE PROGRESSION SHEET every DON-PROG rule reads (aggro 5.4, auras 4.2, AI Donald
//! 11): `sheets/port/party-don-progression.json`, compiled in and tunable WITHOUT A REBUILD (the host reads the sheet
//! again and lays the `don.*` ctl keys on top: kh-partyhost `donald::install_all`, then [`install_prog`]).
//!
//! THE LEVEL TREE ("as many as Sora's", 10): every ability but Aero Float (the Red Wolf of Radagon's reward) is learned
//! by level, in level order: Scan at the start (1), Low Profile 10, Hide Behind 25, Combo Plus 30, Swift Attune 33,
//! Distraction 40, Elemental Insight 48, Taunt Pass 55, Spell Echo 61, Out of Sight 70, Instant Attune 72, Dual
//! Attunement 80, the rest spread between ([`FriendStatus::don_sync_level`]): an auto-levelled join is given every
//! ability up to his level at once and wears them in level order while his AP lasts (the early ones first).
//!
//! THE KH3-STANDARD ONES are wired to KH3's own effects: Second Chance (LAST_LEAVE), Withstand Combo (COMBO_LEAVE), MP
//! Haste / Hastera / Hastega, Lucky Lucky (LUCK_UP), Treasure Magnet (PRIZE_DRAW), Item Boost (ITEM_UP) and Scan
//! (LIBRA) count as one copy of the KH3 ability in `FriendStatus::count` while owned and equipped
//! ([`DonAbility::kh3_kind`], [`FriendStatus::don_kh3_count`]), so every place that reads the KH3 ability (the prize
//! rules, the item effect, the Scan gauge) sees them. Second Chance / Withstand Combo / MP Haste / MP Rage on a friend
//! are DON-PROG's own small rules here (KH3's friends never had them): [`FriendStatus::don_survive`],
//! [`FriendStatus::don_mp_haste`], [`FriendStatus::don_mp_rage`].

use std::sync::{Arc, RwLock};

use kh_core::enums::ETresAbilityKind as A;

use super::abilities::DonAbility;

/// (DON-PROG) His KH3 spell abilities, replaced by the overhaul's magic (doc 13): not given any more, dropped from an old
/// save (`FriendStatus::don_drop_kh3_spells`); his tree's requests named after them are gated by `don_tree_gate`.
pub const KH3_SPELLS: [&str; 5] = ["DONALD_FIRE", "DONALD_BLIZZARD", "DONALD_THUNDER", "DONALD_AERO", "DONALD_CURE"];

include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_don_progression.rs"));

/// The progression sheet's rows, tunable at run time.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgData {
    pub rows: Vec<DonProgRow>,
}

impl Default for ProgData {
    fn default() -> ProgData {
        ProgData::sheet()
    }
}

impl ProgData {
    /// The compiled sheet.
    pub fn sheet() -> ProgData {
        ProgData { rows: DON_PROG.to_vec() }
    }

    pub fn row(&self, id: &str) -> Option<&DonProgRow> {
        self.rows.iter().find(|r| r.id == id)
    }

    /// A rule's number (`aggro.low_profile`); 0 when the sheet has no such row.
    pub fn value(&self, id: &str) -> f32 {
        self.row(id).map_or(0.0, |r| r.value)
    }

    /// One value by its key: a rule row (`aggro.low_profile`, the `value` column) or an ability's column
    /// (`ability.combo_plus.ap` / `.level` / `.priority` / `.value`); a `don.` prefix is allowed. False when the key
    /// names nothing.
    pub fn set(&mut self, key: &str, v: f32) -> bool {
        let key = key.trim();
        let key = key.strip_prefix("don.").unwrap_or(key);
        if let Some(row) = self.rows.iter_mut().find(|r| r.id == key) {
            row.value = v;
            return true;
        }
        let Some((id, col)) = key.rsplit_once('.') else { return false };
        let Some(row) = self.rows.iter_mut().find(|r| r.id == id) else { return false };
        match col {
            "ap" => row.ap = v.round() as i32,
            "level" => row.level = v.max(0.0).round() as u32,
            "priority" => row.priority = v.max(0.0).round() as u32,
            "value" => row.value = v,
            _ => return false,
        }
        true
    }
}

static PROG: RwLock<Option<Arc<ProgData>>> = RwLock::new(None);

/// The progression data every DON-PROG rule reads (the compiled sheet until a host [`install_prog`]s its own).
pub fn prog() -> Arc<ProgData> {
    if let Some(d) = PROG.read().unwrap_or_else(|e| e.into_inner()).as_ref() {
        return d.clone();
    }
    let mut w = PROG.write().unwrap_or_else(|e| e.into_inner());
    w.get_or_insert_with(|| Arc::new(ProgData::sheet())).clone()
}

/// The host's progression data (the sheet read again, the ctl overrides) for every later [`prog`].
pub fn install_prog(d: ProgData) {
    *PROG.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(d));
}

impl DonAbility {
    /// The sheet's key (`combo_plus`, `combo_plus_2`, `aero_float`): the save name without `DON_`, lower case.
    pub fn key(self) -> String {
        self.name().trim_start_matches("DON_").to_ascii_lowercase()
    }

    fn prog_row(self) -> Option<DonProgRow> {
        prog().row(&format!("ability.{}", self.key())).copied()
    }

    /// The menu's help line (KH3's ability help style).
    pub fn description(self) -> &'static str {
        match self {
            DonAbility::ComboPlus | DonAbility::ComboPlus2 => "Add one more magic missile to your ground combo before the finishing slam.",
            DonAbility::AirComboPlus => "Add one more magic missile to your aerial combo.",
            DonAbility::ChargedMissilePlus => "Charge a Charged Missile faster: hold the attack button for 0.5 seconds instead of 0.8.",
            DonAbility::MissileVolley => "Your combo's finishing slam fires a volley of five homing missiles.",
            DonAbility::ArcaneBolt => "Your combo's finishing slam becomes one large missile that pierces enemies.",
            DonAbility::Starfall => "End your aerial combo with a rain of missiles on the enemies below.",
            DonAbility::WandBonkPlus => "Knock enemies further back with the Wand Bonk.",
            DonAbility::CastGuard => "Gives a 30% chance that being hit does not interrupt your spell.",
            DonAbility::SecondChance => kh_gendata::line("kh3_system.ability.detail_sup_0210"),
            DonAbility::WithstandCombo => kh_gendata::line("kh3_system.ability.detail_sup_0220"),
            DonAbility::FireBoost => "Deal 20% more damage with fire attacks.",
            DonAbility::BlizzardBoost => kh_gendata::line("kh3_system.ability.detail_sup_0120"),
            DonAbility::ThunderBoost => kh_gendata::line("kh3_system.ability.detail_sup_0130"),
            DonAbility::WaterBoost => kh_gendata::line("kh3_system.ability.detail_sup_0140"),
            DonAbility::AeroBoost => "Deal 20% more damage with aero attacks.",
            DonAbility::CureBoost => "Restore 20% more HP with healing magic.",
            DonAbility::MpHaste => kh_gendata::line("kh3_system.ability.detail_sup_0380"),
            DonAbility::MpHastera => kh_gendata::line("kh3_system.ability.detail_sup_0390"),
            DonAbility::MpHastega => kh_gendata::line("kh3_system.ability.detail_sup_0400"),
            DonAbility::MpRage => "Restore MP when you take damage.",
            DonAbility::SpellWeaver => "Magic missiles fired right after a spell deal 20% more damage.",
            DonAbility::ReactionPlus => "Elemental Reactions deal 30% more damage.",
            DonAbility::ResonanceKeeper => "Resonance drains more slowly out of battle.",
            DonAbility::SwiftAttune => "Attune to an element in half the time.",
            DonAbility::InstantAttune => "Attune to an element instantly.",
            DonAbility::DualAttunement => "Attune to two elements at once. Each has its own Resonance, and their hybrid spell becomes available.",
            DonAbility::QuickCast => "While holding the shortcut button, see each spell's MP cost and which one would trigger a Reaction on your target.",
            DonAbility::SpellEcho => "After casting a -za spell, your next base spell of the same element costs no MP.",
            DonAbility::Scan => kh_gendata::line("kh3_system.ability.detail_sup_0010"),
            DonAbility::ElementalInsight => "See the target's weak and resisted elements, and the best element to attune to against it.",
            DonAbility::ArcaneFlowPlus => "Arcane Flow charges last 30 seconds instead of 20.",
            DonAbility::ArcaneBoost => "Comet, Gravity, Meteor and Ultima deal 20% more damage.",
            DonAbility::LowProfile => "In a party, enemies are 20% less likely to target you.",
            DonAbility::HideBehind => "In a party, enemies are 40% less likely to target you while an ally is within 8 m.",
            DonAbility::Distraction => "In a party, an enemy that targets you has a 30% chance to switch to your nearest ally.",
            DonAbility::TauntPass => "In a party, when an ally blocks or guards, enemies near you turn on that ally.",
            DonAbility::OutOfSight => "In a party, enemies lose interest in you while you stand still and cast from over 15 m away.",
            DonAbility::AuraRangePlus => "Your attunement's party aura reaches farther.",
            DonAbility::LuckyLucky => "Increase how often enemies drop items.",
            DonAbility::TreasureMagnet => kh_gendata::line("kh3_system.ability.detail_sup_0170"),
            DonAbility::ItemBoost => kh_gendata::line("kh3_system.ability.detail_sup_0570"),
            DonAbility::AeroFloat => "Hold the jump button while falling to float down slowly.",
        }
    }

    /// AP to equip it (sheet `ability.<key>` column `ap`).
    pub fn ap(self) -> i32 {
        self.prog_row().map_or(0, |r| r.ap.max(0))
    }

    /// The level he learns it at (sheet column `level`; None: not learned by level, e.g. Aero Float from the Red Wolf of
    /// Radagon).
    pub fn level(self) -> Option<u32> {
        self.prog_row().map(|r| r.level).filter(|l| *l > 0)
    }

    /// Its place in the test level's loadout (1 = first; None: not equipped there by default).
    pub fn priority(self) -> Option<u32> {
        self.prog_row().map(|r| r.priority).filter(|p| *p > 0)
    }

    /// The KH3 ability whose effect it is (counted as one copy of it while owned and equipped); None for the overhaul's
    /// own abilities (the element Boosts are DON-MAGIC's own rule, not KH3's FIRE_UP: no double count).
    pub fn kh3_kind(self) -> Option<A> {
        Some(match self {
            DonAbility::SecondChance => A::LastLeave,
            DonAbility::WithstandCombo => A::ComboLeave,
            DonAbility::MpHaste => A::MpHaste,
            DonAbility::MpHastera => A::MpHastera,
            DonAbility::MpHastega => A::MpHastega,
            DonAbility::LuckyLucky => A::LuckUp,
            DonAbility::TreasureMagnet => A::PrizeDraw,
            DonAbility::ItemBoost => A::ItemUp,
            DonAbility::Scan => A::Libra,
            _ => return None,
        })
    }

    /// The abilities learned by level, in level order (ties in menu order).
    pub fn level_order() -> Vec<DonAbility> {
        let mut order: Vec<DonAbility> = DonAbility::ALL.into_iter().filter(|k| k.level().is_some()).collect();
        order.sort_by_key(|k| k.level().unwrap_or(u32::MAX));
        order
    }

    /// The abilities a level-up from `from` to `to` teaches (the LEVEL UP! window's names), in level order.
    pub fn learned_between(from: u32, to: u32) -> Vec<DonAbility> {
        DonAbility::level_order().into_iter().filter(|k| k.level().is_some_and(|l| l > from && l <= to)).collect()
    }
}

impl crate::status::FriendStatus {
    /// (DON-PROG) The AP Donald's overhaul abilities take (owned and equipped).
    pub fn don_ap_in_use(&self) -> i32 {
        let Some(d) = self.donald.as_ref() else { return 0 };
        DonAbility::ALL.into_iter().filter(|k| d.abilities.has(*k)).map(|k| k.ap()).sum()
    }

    /// (DON-PROG) A KH3 ability's copies his overhaul abilities stand for (one per owned and equipped KH3-standard one).
    pub fn don_kh3_count(&self, kind: A) -> u32 {
        let Some(d) = self.donald.as_ref() else { return 0 };
        DonAbility::ALL.into_iter().filter(|k| k.kh3_kind() == Some(kind) && d.abilities.has(*k)).count() as u32
    }

    /// (DON-PROG) KH3'S ABILITY GATE OF HIS TREE'S SPELL REQUESTS WITHOUT THE ABILITY (the coordinator, 2026-10-22: his KH3
    /// spell abilities are removed, the overhaul's magic replaced them). KH3's gate FUN_1405A4EA0 was "equipped and payable";
    /// the request is now the overhaul's to carry out (`donald::ai::party_pass`), so the gate answers from his state: not
    /// recharging MP and his MP at KH3's `m_UseMP` of that ability (all MP: any MP), the numbers it had, so AI Donald's
    /// requests come exactly as before. None: not his (Goofy, a non-spell key) or he still holds the KH3 ability (KH3's own
    /// gate then: the tests of that machinery).
    pub fn don_tree_gate(&self, key: &str) -> Option<bool> {
        if self.donald.is_none() || !KH3_SPELLS.contains(&key) {
            return None;
        }
        if A::from_game_name(key).is_some_and(|k| self.abilities.owned(k) > 0) {
            return None;
        }
        let (_, use_mp) = crate::tables::ability_cost(key)?;
        Some(self.mp_charge_left <= 0.0 && if use_mp < 0 { self.mp > 0 } else { self.mp >= use_mp })
    }

    /// (DON-PROG) `HasAbility:DONALD_*` of his tree (C3, pawn vt+0x14F0): his overhaul magic answers it (he knows some of
    /// it from the start; all five were always equipped before, so the tree's choices are as before). None: not one of them.
    pub fn don_has_ability(&self, kind: A) -> Option<bool> {
        let d = self.donald.as_ref()?;
        if !KH3_SPELLS.contains(&kind.game_name().trim_start_matches("ETresAbilityKind::")) || self.abilities.owned(kind) > 0 {
            return None;
        }
        Some(!d.learned.elements().is_empty())
    }

    /// (DON-PROG) AN OLD SAVE'S KH3 SPELL ABILITIES (`["DONALD_FIRE", 1, 1]` ... in the v5 friends record): dropped, once (the
    /// next save no longer lists them). They took no AP with the overhaul, so nothing else changes. Returns the names.
    pub fn don_drop_kh3_spells(&mut self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.donald.is_none() {
            return out;
        }
        for name in KH3_SPELLS {
            if let Some(k) = A::from_game_name(name).filter(|k| self.abilities.owned(*k) > 0) {
                self.abilities.set(k, 0, 0);
                out.push(name);
            }
        }
        out
    }

    /// (DON-PROG) His KH3 spell abilities (DONALD_FIRE ... DONALD_CURE): with the overhaul they are only the AI's gate
    /// for its spell requests (the overhaul's magic replaced them, doc 13), so they take no AP.
    pub fn don_free_kh3(&self, kind: A) -> bool {
        self.donald.is_some() && kind.game_name().contains("DONALD_")
    }

    /// (DON-PROG) Donald's abilities his level has reached that he does not own yet, in the order of their levels:
    /// owned, and equipped while his AP allows (an auto-levelled join wears the early ones first). Returns the new ones.
    pub fn don_sync_level(&mut self) -> Vec<DonAbility> {
        let mut out = Vec::new();
        if self.donald.is_none() {
            return out;
        }
        let level = self.level;
        for k in DonAbility::level_order() {
            if !k.level().is_some_and(|l| level >= l) || self.donald.as_ref().is_some_and(|d| d.abilities.owned(k)) {
                continue;
            }
            let fits = self.ap_in_use() + k.ap() <= self.max_ap();
            let Some(d) = self.donald.as_mut() else { return out };
            d.abilities.give(k);
            if fits {
                d.abilities.set_equipped(k, true);
            }
            out.push(k);
        }
        out
    }

    /// (DON-PROG) Gives one of his abilities (a boss's `don:` reward): owned, equipped while his AP allows. Returns
    /// (new, equipped); `None` when he is not Donald.
    pub fn don_grant(&mut self, k: DonAbility) -> Option<(bool, bool)> {
        let fits = self.ap_in_use() + k.ap() <= self.max_ap();
        let d = self.donald.as_mut()?;
        let new = d.abilities.give(k);
        if new && fits {
            d.abilities.set_equipped(k, true);
        }
        Some((new, d.abilities.equipped(k)))
    }

    /// (DON-PROG) Equips / unequips one with the AP check (the camp menu). False = refused (not owned, no AP).
    pub fn don_equip(&mut self, k: DonAbility, on: bool) -> bool {
        let over = on && self.donald.as_ref().is_some_and(|d| !d.abilities.has(k)) && self.ap_in_use() + k.ap() > self.max_ap();
        if over {
            return false;
        }
        self.donald.as_mut().is_some_and(|d| d.abilities.set_equipped(k, on))
    }

    /// (DON-PROG) THE TEST LEVEL'S LOADOUT (X3D's rule, never past his max AP): every ability owned; equipped in the
    /// sheet's priority order while his AP allows; then the KH3 bonus abilities `bonus` (the level-99 loadout's) while AP
    /// is left. His magic is the roster's (kh-partyhost: `DonLearned::all()` in the test level).
    pub fn don_test_loadout(&mut self, bonus: &[&str]) {
        if self.donald.is_none() {
            return;
        }
        let kinds: Vec<A> = bonus.iter().filter_map(|n| A::from_game_name(n)).filter(|k| !k.game_name().contains("DONALD_")).collect();
        for k in &kinds {
            let owned = self.abilities.owned(*k);
            self.abilities.set(*k, owned, 0);
        }
        self.kit = Default::default(); // the X3D kit is migrated (pad/donald_kit.rs): it takes no AP here
        if let Some(d) = self.donald.as_mut() {
            d.abilities = super::abilities::DonAbilities::default();
            for k in DonAbility::ALL {
                d.abilities.give(k);
            }
        }
        // his overhaul ones in the sheet's order, then the KH3 ones while AP is left (X3D's order)
        let equip_kh3 = |s: &mut Self, k: A| {
            let cost = crate::tables::ability_cost(k.game_name().trim_start_matches("ETresAbilityKind::")).map_or(0, |c| c.0);
            let (owned, eq) = (s.abilities.owned(k), s.abilities.equipped(k));
            if owned > eq && s.ap_in_use() + cost <= s.max_ap() {
                s.abilities.set(k, owned, eq + 1);
            }
        };
        let mut order: Vec<DonAbility> = DonAbility::ALL.into_iter().filter(|k| k.priority().is_some()).collect();
        order.sort_by_key(|k| k.priority().unwrap_or(u32::MAX));
        for k in order {
            if self.ap_in_use() + k.ap() <= self.max_ap() {
                if let Some(d) = self.donald.as_mut() {
                    d.abilities.set_equipped(k, true);
                }
            }
        }
        for k in kinds {
            equip_kh3(self, k);
        }
    }

    /// (DON-PROG) Second Chance / Withstand Combo on Donald (KH3's `ApplyHpDamage` 0x1405A2A00 rules, kh-core
    /// `apply_damage_to_sora`): a lethal hit leaves 1 HP when he had 2 or more (Second Chance), or while an enemy's
    /// combo runs on him (`in_combo`: his damage reaction / the 0.6 s after a hit; Withstand Combo, also from 1 HP).
    /// Returns the HP to keep (`new_hp` unchanged when no rule saves him).
    pub fn don_survive(&self, before: i32, new_hp: i32, in_combo: bool) -> i32 {
        if new_hp >= 1 || self.donald.is_none() {
            return new_hp;
        }
        if before > 1 && self.don_kh3_count(A::LastLeave) > 0 {
            return 1;
        }
        if before >= 1 && in_combo && self.don_kh3_count(A::ComboLeave) > 0 {
            return 1;
        }
        new_hp
    }

    /// (DON-PROG) The MP Charge's speed on Donald (KH3's MP Haste rule: 1 + 0.1 / 0.2 / 0.3 per copy of MP Haste /
    /// Hastera / Hastega, kh-core `MP_HASTE_RATE`; his KH3 copies and his overhaul ones alike). 1 for anyone else.
    pub fn don_mp_haste(&self) -> f32 {
        if self.donald.is_none() {
            return 1.0;
        }
        let r = kh_core::status::runtime::MP_HASTE_RATE;
        // (ZETTA 2026-10-23, the user: "it makes your MP regen slower by 30%") after Zettaflare, until his MP has recharged
        let slow = if self.donald.as_ref().is_some_and(|d| d.core.zetta_slow) { crate::donald::zettaflare::zetta().zetta_regen_mult.clamp(0.05, 1.0) } else { 1.0 };
        (1.0 + r[0] * self.count(A::MpHaste) as f32 + r[1] * self.count(A::MpHastera) as f32 + r[2] * self.count(A::MpHastega) as f32) * slow
    }

    /// (DON-PROG) MP Rage: MP restored for `lost` HP (sheet `kh3.mp_rage` MP per HP), not while the MP Charge runs.
    pub fn don_mp_rage(&mut self, lost: i32) -> i32 {
        let on = self.donald.as_ref().is_some_and(|d| d.abilities.has(DonAbility::MpRage));
        if !on || lost <= 0 || self.mp_charge_left > 0.0 {
            return 0;
        }
        let gain = ((lost as f32) * prog().value("kh3.mp_rage")).round() as i32;
        let before = self.mp;
        self.mp = (self.mp + gain.max(0)).min(self.max_mp());
        self.mp - before
    }
}
