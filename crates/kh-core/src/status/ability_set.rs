//! Which abilities work: the per-ability word of the save record, as counts
//! (docs/sora-stats-spec.md 5.1). An ability works when it is OWNED AND EQUIPPED, or supplied by
//! worn equipment, or is the active meal's bonus ability; owned-only copies do nothing.
//!
//! | game (bits of `record+0x160 + 4*id`) | here |
//! |---|---|
//! | 0 / 4 / 8 owned, 1 / 5 / 9 equipped: three have / equipped pairs | `owned` 0..3; `equipped`: ONE BIT PER COPY (bit n = copy n is equipped), as the game's three pairs |
//! | 18..24 copies from worn equipment and the weapon's form (cap 99, no AP) | `worn` |
//! | meal object `bp+0x138`, `+0x30` bonus ability id | `meal` |
//!
//! The permanent / victory-bonus / level split of the owned copies (bits 3 / 7 / 11, 12..14,
//! 15..17) is kept by `Status` (its base copies plus the level table's grants), not here.
//!
//! (X1, 2026-10-15) AN ABILITY THE PORT EXCLUDES ([`tables::is_excluded`]: the Attraction Flow
//! abilities, decision dec029) IS NEVER HELD: every setter below leaves it at 0, so a save, a loadout
//! sheet, a level-up, a reward or equipment that names one gives nothing, no menu lists it and it
//! costs no AP (an old save's equipped copy is dropped, its AP free again).

use super::tables;
use crate::enums::ETresAbilityKind;

/// Highest `ETresAbilityKind` value + 1 that the set can hold: the game's array has 0x200 words.
/// (XSI, 2026-10-16) 512 (was 256): the port's own abilities (sheets/port/sora-enum-ext) are 256 .. 269.
pub const ABILITY_SLOTS: usize = 512;
/// Copies of one ability that can be owned: three have / equipped bit pairs (0x140E8AAB0).
pub const MAX_OWNED: u8 = 3;
/// Cap of the equipment counter, bits 18..24 (0x140E87590).
pub const MAX_WORN: u8 = 99;

/// Owned, equipped and worn copies per `ETresAbilityKind`, and the meal's bonus ability.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AbilitySet {
    owned: [u8; ABILITY_SLOTS],
    /// (fixes pass 1) A BIT PER COPY, not a count: the game keeps an equipped bit beside each
    /// of the three have bits, and the Abilities screen toggles the copy under the cursor. With
    /// a count the menu could only say "one more / one fewer", and the list (which shows the
    /// first n copies as equipped) then lit the row ABOVE the one the player had chosen.
    equipped: [u8; ABILITY_SLOTS],
    worn: [u8; ABILITY_SLOTS],
    meal: Option<ETresAbilityKind>,
    /// Copies supplied by the active wear form (the form ability set of the character:
    /// p_ex001_FormAbility, combat strand F 10.5). In the game these are equipment copies too
    /// (bits 18..24, added by the equipment rebuild when the wear form changes); they are kept
    /// apart here so that an equipment rebuild during a form does not lose them. No AP.
    /// (Added by the combat implementer, 2026-10-11.)
    form: [u8; ABILITY_SLOTS],
}

impl Default for AbilitySet {
    fn default() -> AbilitySet {
        AbilitySet::EMPTY
    }
}

impl AbilitySet {
    pub const EMPTY: AbilitySet = AbilitySet { owned: [0; ABILITY_SLOTS], equipped: [0; ABILITY_SLOTS], worn: [0; ABILITY_SLOTS], meal: None, form: [0; ABILITY_SLOTS] };

    fn slot(kind: ETresAbilityKind) -> usize {
        (kind.value() as usize).min(ABILITY_SLOTS - 1)
    }

    /// Copies owned (equipped or not), 0..=3.
    pub fn owned(&self, kind: ETresAbilityKind) -> u8 {
        self.owned[Self::slot(kind)]
    }

    /// Copies owned and equipped: the ones that cost AP.
    pub fn equipped(&self, kind: ETresAbilityKind) -> u8 {
        self.equipped[Self::slot(kind)].count_ones() as u8
    }

    /// Which copies are equipped: bit n = copy n (0 .. 2).
    pub fn equipped_mask(&self, kind: ETresAbilityKind) -> u8 {
        self.equipped[Self::slot(kind)]
    }

    /// Equips or removes ONE copy (`SetEquipped` of the Abilities screen: the row under the
    /// cursor). False when that copy is not owned.
    pub fn set_copy(&mut self, kind: ETresAbilityKind, copy: u8, on: bool) -> bool {
        let slot = Self::slot(kind);
        if copy >= self.owned[slot] || tables::is_excluded(kind) {
            return false;
        }
        self.equipped[slot] = (self.equipped[slot] & !(1 << copy)) | ((on as u8) << copy);
        true
    }

    /// Copies supplied by worn equipment (no AP).
    pub fn worn(&self, kind: ETresAbilityKind) -> u8 {
        self.worn[Self::slot(kind)]
    }

    /// The active meal's bonus ability.
    pub fn meal(&self) -> Option<ETresAbilityKind> {
        self.meal
    }

    /// THE GAMEPLAY QUERY, `Count(bp, id)` 0x140580810 [I]: equipped copies + equipment copies
    /// + 1 when the active meal's bonus ability is this one.
    pub fn count(&self, kind: ETresAbilityKind) -> u32 {
        if kind == ETresAbilityKind::None {
            return 0;
        }
        self.equipped(kind) as u32 + self.worn(kind) as u32 + self.form[Self::slot(kind)] as u32 + u32::from(self.meal == Some(kind))
    }

    /// `Has(bp, id)` 0x140588690: `count > 0`.
    pub fn has(&self, kind: ETresAbilityKind) -> bool {
        self.count(kind) > 0
    }

    /// Sets the owned copies and HOW MANY of them are equipped (`owned` capped at 3,
    /// `equipped` at `owned`). The copies that are equipped now stay the equipped ones as far
    /// as the number allows: more = the lowest unequipped copies are added, fewer = the highest
    /// equipped copies are removed.
    pub fn set(&mut self, kind: ETresAbilityKind, owned: u8, equipped: u8) {
        let slot = Self::slot(kind);
        // (X1) an excluded ability is never held.
        let owned = if tables::is_excluded(kind) { 0 } else { owned };
        self.owned[slot] = owned.min(MAX_OWNED);
        let wanted = equipped.min(self.owned[slot]) as u32;
        let mut mask = self.equipped[slot] & ((1u8 << self.owned[slot]) - 1);
        for copy in 0..self.owned[slot] {
            if mask.count_ones() < wanted && mask & (1 << copy) == 0 {
                mask |= 1 << copy;
            }
        }
        for copy in (0..MAX_OWNED).rev() {
            if mask.count_ones() > wanted {
                mask &= !(1 << copy);
            }
        }
        self.equipped[slot] = mask;
    }

    /// Sets the owned copies; more equipped than owned are unequipped (`FixEquipped` 0x140E95FC0).
    pub fn set_owned(&mut self, kind: ETresAbilityKind, owned: u8) {
        let slot = Self::slot(kind);
        let owned = if tables::is_excluded(kind) { 0 } else { owned }; // (X1)
        self.owned[slot] = owned.min(MAX_OWNED);
        self.equipped[slot] &= (1u8 << self.owned[slot]) - 1;
    }

    /// Clears the equipment copies of every ability (`ClearEquipCounters` 0x140E87F10).
    pub fn clear_worn(&mut self) {
        self.worn = [0; ABILITY_SLOTS];
    }

    /// One more equipment copy (`AddEquipAbilities` 0x140E87590, cap 99).
    pub fn add_worn(&mut self, kind: ETresAbilityKind) {
        if tables::is_excluded(kind) {
            return; // (X1)
        }
        let slot = Self::slot(kind);
        self.worn[slot] = (self.worn[slot] + 1).min(MAX_WORN);
    }

    /// Copies supplied by the active wear form.
    pub fn form(&self, kind: ETresAbilityKind) -> u8 {
        self.form[Self::slot(kind)]
    }

    /// (CHESTFIX) A wear form supplies any ability now.
    pub fn any_form(&self) -> bool {
        self.form.iter().any(|c| *c > 0)
    }

    /// The wear form changed: its ability list replaces the previous form's (an empty list =
    /// wear form NORMAL). Each entry is (ability, copies); the equipment counter's cap applies.
    pub fn set_form(&mut self, list: &[(ETresAbilityKind, u8)]) {
        self.form = [0; ABILITY_SLOTS];
        for &(kind, copies) in list.iter().filter(|(kind, _)| !tables::is_excluded(*kind)) {
            let slot = Self::slot(kind);
            self.form[slot] = (self.form[slot] + copies).min(MAX_WORN);
        }
    }

    /// The meal's bonus ability (`None` = no meal or a meal without one).
    pub fn set_meal(&mut self, kind: Option<ETresAbilityKind>) {
        self.meal = kind.filter(|&k| !tables::is_excluded(k)); // (X1)
    }

    /// AP in use: every equipped copy costs its row's `m_UseAP`; worn and meal copies cost
    /// nothing (stats spec 5.1).
    pub fn ap_in_use(&self) -> i32 {
        self.iter_equipped().map(|(kind, copies)| tables::ability_ap(kind) * copies as i32).sum()
    }

    /// AP that owning-and-equipping every owned copy would cost.
    pub fn ap_all_owned(&self) -> i32 {
        self.iter_owned().map(|(kind, copies)| tables::ability_ap(kind) * copies as i32).sum()
    }

    /// Every equipped ability with its copies, ascending by value.
    pub fn iter_equipped(&self) -> impl Iterator<Item = (ETresAbilityKind, u8)> + '_ {
        ETresAbilityKind::ALL.iter().map(|&kind| (kind, self.equipped(kind))).filter(|&(_, count)| count > 0)
    }

    /// Every owned ability with its copies, ascending by value.
    pub fn iter_owned(&self) -> impl Iterator<Item = (ETresAbilityKind, u8)> + '_ {
        ETresAbilityKind::ALL.iter().map(|&kind| (kind, self.owned(kind))).filter(|&(_, count)| count > 0)
    }

    /// Every ability that works (`count > 0`) with its count, ascending by value.
    pub fn iter_active(&self) -> impl Iterator<Item = (ETresAbilityKind, u32)> + '_ {
        ETresAbilityKind::ALL.iter().map(|&kind| (kind, self.count(kind))).filter(|&(_, count)| count > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ETresAbilityKind as A;

    #[test]
    fn each_copy_has_its_own_equipped_bit() {
        // Fixes pass 1: the camp menu toggles the copy under the cursor (the game's three have /
        // equipped bit pairs), not a count.
        let mut set = AbilitySet::EMPTY;
        set.set(A::Airslide, 3, 0);
        assert!(set.set_copy(A::Airslide, 2, true));
        assert_eq!((set.equipped(A::Airslide), set.equipped_mask(A::Airslide), set.count(A::Airslide)), (1, 0b100, 1));
        assert!(set.set_copy(A::Airslide, 1, true) && set.set_copy(A::Airslide, 2, false));
        assert_eq!((set.equipped(A::Airslide), set.equipped_mask(A::Airslide)), (1, 0b010));
        assert!(!set.set_copy(A::Airslide, 3, true), "a copy that is not owned");
        // The count form keeps the copies that are equipped: one more = the lowest free copy,
        // one fewer = the highest equipped copy; fewer owned drops the copies above.
        set.set(A::Airslide, 3, 2);
        assert_eq!(set.equipped_mask(A::Airslide), 0b011);
        set.set(A::Airslide, 3, 1);
        assert_eq!(set.equipped_mask(A::Airslide), 0b001);
        set.set(A::Airslide, 3, 3);
        set.set_owned(A::Airslide, 2);
        assert_eq!((set.equipped(A::Airslide), set.equipped_mask(A::Airslide)), (2, 0b011));
        set.set(A::ComboPlus, 2, 2);
        assert_eq!(set.ap_in_use(), tables::ability_ap(A::Airslide) * 2 + tables::ability_ap(A::ComboPlus) * 2);
    }

    /// (X1) The Attraction Flow abilities (decision dec029) can be held by no route.
    #[test]
    fn excluded_abilities_are_never_held() {
        assert!(tables::is_excluded(A::AttractionTime) && tables::is_excluded(A::AttractionUp));
        assert!(!tables::is_excluded(A::CriticalConverter) && !tables::is_excluded(A::FormUp) && !tables::is_excluded(A::Airslide));
        let mut set = AbilitySet::EMPTY;
        set.set(A::Airslide, 1, 1);
        let ap = set.ap_in_use();
        for kind in [A::AttractionTime, A::AttractionUp] {
            set.set(kind, 3, 3);
            set.set_owned(kind, 2);
            assert!(!set.set_copy(kind, 0, true));
            set.add_worn(kind);
            set.set_form(&[(kind, 1)]);
            set.set_meal(Some(kind));
            assert_eq!((set.owned(kind), set.equipped(kind), set.worn(kind), set.form(kind), set.count(kind), set.has(kind)), (0, 0, 0, 0, 0, false), "{kind:?}");
        }
        assert_eq!((set.meal(), set.ap_in_use(), set.iter_owned().count()), (None, ap, 1));
    }
}
