//! (new game) REWARDS: the one seam through which a running Sora gains something by "other means" than
//! the level table: an ability, a magic tier, a max-HP / max-MP bonus, an equipment or item slot, AP, a
//! shotlock, an item. A host calls [`crate::Sora::grant`]; what it changes is save data (the host's save
//! captures it), and the popup it raises is the game's own victory-bonus panel.
//!
//! WHAT THE GAME DOES WITH A VICTORY BONUS (`TresGetVictoryBonus` -> 0x140E9F040 [C]): per (bonus kind,
//! ability) pair of the row it calls the pawn's slot 676 0x1405A06B0 -> 0x140574450 -> 0x140E86BC0 ->
//! 0x140E86B30: the kind's counter `record+0x960 + kind` rises, the ability gets one more OWNED copy with
//! its NEW mark (bit 2 / 6 / 10) and its victory-bonus counter, NOT the equipped bit (the equipped bit is
//! written only by the camp menu, 0x140E95E90 from 0x1415C50C0 / 0x1415C59B0, and by `FixEquipped`); the
//! bonus sums are recomputed and the current HP / MP rise by the new amount. Then the HUD panel
//! `HudRightGetBonus` (0x1415874F0) gets (kind, value) entries: 5 max HP + amount, 6 max MP + amount,
//! 9 / 10 / 11 armour / accessory / item slot, 8 the magic command learned (which is also put into a
//! magic slot, 0x14159EE70), 7 the ability. Texts `kh3_system.general.bonus_0010` .. `0070`, title
//! `window_0040` "BONUS!".
//!
//! NOT VICTORY BONUSES IN THE GAME (the same panel is used here, LOCAL): AP (the game raises AP by the
//! level table and by AP Boost items, `record+0x983`), a shotlock (the game ties shotlocks to keyblades).
//! An item shows the game's item popup (`HudLeftGetItem`).

use super::{ability_set, BonusCounts, ItemKind, KnownMagic, Status};
use crate::enums::ETresAbilityKind;
use crate::magic::spells::Element;

/// Something given to Sora.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reward {
    /// `copies` more owned copies of an ability (at most three are owned). As in the game the copies
    /// are NOT equipped: the player installs them on the Abilities screen when the AP allow.
    Ability { kind: ETresAbilityKind, copies: u8 },
    /// A spell: `tier` 1..=3 = the element up to that tier, 0 = one tier more than he has.
    Magic { element: Element, tier: u8 },
    /// One victory bonus of an `ETresVictoryBonusKind` (`BonusCounts::HP_UP10`, `MP_UP5`,
    /// `ITEM_SLOT_UP1` ...): the game's own steps of max HP and max MP (3 / 5 / 10 / 15 / 30 and
    /// 3 / 5 / 10 / 20; Critical has its own amounts) and the three slot kinds.
    Bonus(usize),
    /// `n` more AP (the AP Boost byte `record+0x983`).
    Ap(u8),
    /// A shotlock by its name. (A record only: the shotlocks he can fire are the keyblade's.)
    Shotlock(&'static str),
    /// `count` of an item into the stock.
    Item { item: ItemKind, count: u8 },
}

impl Reward {
    /// The max-HP bonus of exactly `amount` (the Standard amounts 3 / 5 / 10 / 15 / 30).
    pub fn max_hp(amount: i32) -> Option<Reward> {
        Some(Reward::Bonus(match amount {
            3 => BonusCounts::HP_UP3,
            5 => BonusCounts::HP_UP5,
            10 => BonusCounts::HP_UP10,
            15 => BonusCounts::HP_UP15,
            30 => BonusCounts::HP_UP30,
            _ => return None,
        }))
    }

    /// The max-MP bonus of exactly `amount` (3 / 5 / 10 / 20).
    pub fn max_mp(amount: i32) -> Option<Reward> {
        Some(Reward::Bonus(match amount {
            3 => BonusCounts::MP_UP3,
            5 => BonusCounts::MP_UP5,
            10 => BonusCounts::MP_UP10,
            20 => BonusCounts::MP_UP20,
            _ => return None,
        }))
    }
}

/// One line of the bonus panel.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum BonusLine {
    #[default]
    None,
    /// Panel kind 5: "Maximum HP increased!" (`bonus_0010`); the amount.
    MaxHp(i32),
    /// Panel kind 6: "Maximum MP increased!" (`bonus_0020`).
    MaxMp(i32),
    /// Panel kinds 9 / 10 / 11: `bonus_0030` / `0040` / `0050`.
    ArmorSlot,
    AccessorySlot,
    ItemSlot,
    /// Panel kind 8: the command learned (`ETresCommandKind`: 29 Fire ...), shown by its name (`bonus_0060`).
    Magic(u8),
    /// Panel kind 7: the ability's name (`bonus_0070`).
    Ability(ETresAbilityKind),
    /// LOCAL: "Maximum AP increased!" (the level-up panel's line `levelup_0040`).
    MaxAp(i32),
    /// LOCAL: a shotlock's name.
    Shotlock(&'static str),
    /// LOCAL (boss rewards, IMPL_ER31): a host's own line (the summary of several rewards given at once).
    Text(&'static str),
}

/// What a grant did.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct RewardOutcome {
    /// False: nothing changed (he had it all: three copies, tier 3, the counter at its cap).
    pub changed: bool,
    /// The lines of the bonus panel (the first `None` ends them).
    pub lines: [BonusLine; 2],
    /// The item popup, when the reward was an item.
    pub item: Option<ItemKind>,
    /// The magic command learned (for the command system's slot).
    pub learned: Option<u8>,
    /// An item slot was added (for the pouch).
    pub item_slot: bool,
}

/// `MELEM_*` index of an element: Fire 24, Blizzard 25, Thunder 26, Water 27, Aero 28, Cure 29.
pub const fn melem_kind(element: Element) -> usize {
    BonusCounts::MELEM_FIRE
        + match element {
            Element::Fire => 0,
            Element::Blizzard => 1,
            Element::Thunder => 2,
            Element::Water => 3,
            Element::Aero => 4,
            Element::Cure => 5,
        }
}

/// The element of a `MELEM_*` kind.
pub fn element_of_kind(kind: usize) -> Option<Element> {
    Element::ALL.into_iter().find(|e| melem_kind(*e) == kind)
}

impl Status {
    /// The tier he knows of an element (0 = not learned).
    pub fn magic_tier(&self, element: Element) -> u32 {
        self.magic.iter().find(|m| Element::from_name(&m.name) == Some(element)).map_or(0, |m| m.tier.min(3))
    }

    /// THE STATUS PART OF A REWARD (everything but the command slots and the pouch, which
    /// [`crate::Sora::grant`] does): see the file header for what the game does.
    pub fn apply_reward(&mut self, reward: Reward) -> RewardOutcome {
        let mut out = RewardOutcome::default();
        if self.placeholder {
            return out;
        }
        match reward {
            Reward::Ability { kind, copies } => {
                let slot = kind.value() as usize;
                // (X1) an ability the port excludes (decision dec029) gives nothing and raises no panel.
                if kind == ETresAbilityKind::None || slot >= super::ABILITY_SLOTS || super::tables::is_excluded(kind) {
                    return out;
                }
                let before = self.abilities.owned(kind);
                self.base_owned[slot] = self.base_owned[slot].saturating_add(copies).min(ability_set::MAX_OWNED);
                self.rebuild();
                out.changed = self.abilities.owned(kind) > before;
                if out.changed {
                    out.lines[0] = BonusLine::Ability(kind);
                }
            }
            Reward::Magic { element, tier } => {
                let have = self.magic_tier(element);
                let want = if tier == 0 { have + 1 } else { tier as u32 }.min(3);
                if want > have {
                    match self.magic.iter_mut().find(|m| Element::from_name(&m.name) == Some(element)) {
                        Some(known) => known.tier = want,
                        None => self.magic.push(KnownMagic { name: element.name().to_string(), tier: want }),
                    }
                    // The counter of the element's `MELEM_*` kind = the tier (`record+0x960 + kind`).
                    self.bonuses.0[melem_kind(element)] = want as u8;
                    let command = element.command(want as u8);
                    (out.changed, out.learned, out.lines[0]) = (true, Some(command), BonusLine::Magic(command));
                }
            }
            Reward::Bonus(kind) => {
                if let Some(element) = element_of_kind(kind) {
                    return self.apply_reward(Reward::Magic { element, tier: 0 });
                }
                if kind == 0 || kind >= 32 || self.bonuses.0[kind] == u8::MAX {
                    return out;
                }
                let (hp, mp) = BonusCounts::amount(kind, self.game_level);
                let line = match kind {
                    BonusCounts::DEF_SLOT_UP1 => BonusLine::ArmorSlot,
                    BonusCounts::ACC_SLOT_UP1 => BonusLine::AccessorySlot,
                    BonusCounts::ITEM_SLOT_UP1 => BonusLine::ItemSlot,
                    _ if hp > 0 => BonusLine::MaxHp(hp),
                    _ if mp > 0 => BonusLine::MaxMp(mp),
                    _ => return out,
                };
                // Counter, totals, and the current HP / MP rise by the new amount (0x1405A06B0).
                self.award_bonus(kind);
                if kind == BonusCounts::ITEM_SLOT_UP1 {
                    self.item_slots = (self.item_slots + 1).min(8);
                    out.item_slot = true;
                }
                (out.changed, out.lines[0]) = (true, line);
            }
            Reward::Ap(n) => {
                let before = self.boosts.ap;
                self.boosts.ap = self.boosts.ap.saturating_add(n);
                self.refresh();
                out.changed = self.boosts.ap > before;
                if out.changed {
                    out.lines[0] = BonusLine::MaxAp((self.boosts.ap - before) as i32);
                }
            }
            Reward::Shotlock(name) => {
                if !name.is_empty() && !self.shotlocks.iter().any(|s| s == name) {
                    self.shotlocks.push(name.to_string());
                    (out.changed, out.lines[0]) = (true, BonusLine::Shotlock(name));
                }
            }
            Reward::Item { item, count } => {
                if count > 0 {
                    (out.changed, out.item) = (true, Some(item));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::{Loadout, LoadoutValue};
    use ETresAbilityKind as A;

    fn start() -> Status {
        let mut loadout = Loadout::new("test");
        for (key, value) in [("level", LoadoutValue::Number(1.0)), ("game_level", LoadoutValue::Text("STANDARD".into())), ("choice.default", LoadoutValue::Text("balance + ability path A".into())), ("weapon", LoadoutValue::Text("WEP_KEYBLADE_SO_00".into())), ("item_slots", LoadoutValue::Number(3.0))] {
            loadout.set(key, &value).unwrap();
        }
        loadout.set("abilities.owned", &LoadoutValue::List(vec!["DODGE".into()])).unwrap();
        loadout.set("abilities.equipped", &LoadoutValue::List(vec!["DODGE".into()])).unwrap();
        loadout.set("magic.known", &LoadoutValue::Map(vec![("fire".into(), 1.0)])).unwrap();
        Status::from_loadout(&loadout)
    }

    #[test]
    fn an_ability_is_owned_and_not_equipped() {
        let mut status = start();
        let (ap, used) = (status.max_ap(), status.ap_in_use());
        let out = status.apply_reward(Reward::Ability { kind: A::Glide, copies: 1 });
        assert_eq!((out.changed, out.lines[0]), (true, BonusLine::Ability(A::Glide)));
        assert_eq!((status.abilities.owned(A::Glide), status.abilities.equipped(A::Glide), status.has(A::Glide)), (1, 0, false));
        assert_eq!((status.max_ap(), status.ap_in_use()), (ap, used));
        // The player installs it; copies stack to three and no further.
        assert!(status.equip(A::Glide).is_ok() && status.has(A::Glide));
        assert!(status.apply_reward(Reward::Ability { kind: A::Airslide, copies: 5 }).changed);
        assert_eq!(status.abilities.owned(A::Airslide), 3);
        assert!(!status.apply_reward(Reward::Ability { kind: A::Airslide, copies: 1 }).changed);
        // A copy from the level table comes on top of a granted one (Combo Plus: level 2 on path A).
        status.apply_reward(Reward::Ability { kind: A::ComboPlus, copies: 1 });
        status.give_exp(80, 1.0);
        assert_eq!((status.level, status.abilities.owned(A::ComboPlus)), (2, 2));
    }

    #[test]
    fn hp_mp_ap_and_slots() {
        let mut status = start();
        status.hp -= 20;
        let (hp_max, hp, mp_max) = (status.max_hp(), status.hp, status.max_mp());
        let out = status.apply_reward(Reward::max_hp(10).unwrap());
        assert_eq!(out.lines[0], BonusLine::MaxHp(10));
        // Max and CURRENT HP rise by the amount (0x1405A06B0).
        assert_eq!((status.max_hp(), status.hp), (hp_max + 10, hp + 10));
        assert_eq!(status.apply_reward(Reward::max_mp(5).unwrap()).lines[0], BonusLine::MaxMp(5));
        assert_eq!(status.max_mp(), mp_max + 5);
        assert!(Reward::max_hp(7).is_none() && Reward::max_mp(30).is_none());
        let ap = status.max_ap();
        assert_eq!(status.apply_reward(Reward::Ap(3)).lines[0], BonusLine::MaxAp(3));
        assert_eq!(status.max_ap(), ap + 3);
        let out = status.apply_reward(Reward::Bonus(BonusCounts::ITEM_SLOT_UP1));
        assert_eq!((out.lines[0], out.item_slot, status.item_slots), (BonusLine::ItemSlot, true, 4));
        assert_eq!(status.apply_reward(Reward::Bonus(BonusCounts::ACC_SLOT_UP1)).lines[0], BonusLine::AccessorySlot);
        assert_eq!(status.bonuses.0[BonusCounts::ACC_SLOT_UP1], 1);
        assert!(status.apply_reward(Reward::Shotlock("King of Hearts")).changed && !status.apply_reward(Reward::Shotlock("King of Hearts")).changed);
    }

    #[test]
    fn magic_tiers() {
        let mut status = start();
        // One tier more of what he has; a new element; up to a tier; never down, never past 3.
        let out = status.apply_reward(Reward::Magic { element: Element::Fire, tier: 0 });
        assert_eq!((out.learned, status.magic_tier(Element::Fire)), (Some(Element::Fire.command(2)), 2));
        let out = status.apply_reward(Reward::Bonus(melem_kind(Element::Cure)));
        assert_eq!((out.lines[0], status.magic_tier(Element::Cure)), (BonusLine::Magic(Element::Cure.command(1)), 1));
        assert!(status.apply_reward(Reward::Magic { element: Element::Cure, tier: 3 }).changed);
        assert!(!status.apply_reward(Reward::Magic { element: Element::Cure, tier: 2 }).changed);
        assert!(!status.apply_reward(Reward::Magic { element: Element::Cure, tier: 0 }).changed);
        assert_eq!((status.magic_tier(Element::Cure), status.bonuses.0[melem_kind(Element::Cure)]), (3, 3));
    }
}
