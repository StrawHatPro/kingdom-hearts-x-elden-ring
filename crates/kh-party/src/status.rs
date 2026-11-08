//! A friend's status (contract B.8): level from the SHARED EXP total with the character's own
//! table, stats, weapon, abilities, AP, HP / MP and the MP charge, items, AI settings.
//!
//! Sources: `n_ex00x_BaseDataTable`, `n_ex00x_LevelUpData`, `TresChrInitData` `m_FriendDonald` /
//! `m_FriendGoofy`, `TresEquipItemData` I03021.. / I03041.., `TresAbilityData`, `TresVBonusData`
//! (plan 2.3). The stat sums follow the player's formula (stats spec 8: level row + equipment +
//! bonuses) for status kind 4 [D: plan 2.3 asks to confirm 0x140576E70 uses the same sums].

use crate::ids::Chara;
use crate::tables;
use crate::view::ItemSlot;
use kh_core::enums::ETresAbilityKind;
use kh_core::host::TargetDefense;
use kh_core::status::{AbilitySet, Attacker, BonusCounts, Defender, DefenderKind, GameLevel, ItemKind, Side};

/// The camp-menu AI settings of a friend (party record +0x15A..+0x15C, UI spec 10.6; plan 2.8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AiSettings {
    /// `ETresFNpcAIStyle_Battle`: 0 TOGETHER (Stick By Sora), 1 FREE, 2 SUPPORT (Hands Off), 3 FOCUS.
    pub battle: u8,
    /// `ETresFNpcAIStyle_Ability`: 0 FREE (Go Wild), 1 BALANCE (Watch MP), 2 LESS (Conserve).
    pub ability: u8,
    /// `ETresFNpcAIStyle_Cure` (record +0x15A).
    pub cure: u8,
    /// Cure target bits 0..2: On Sora / On Self / On Others (record +0x15B; F.2). Record bytes +0x158 battle,
    /// +0x159 ability, +0x15A cure, +0x15B targets (setters 0x140E97230 / 220 / 240, writer 0x1415EA860).
    pub cure_targets: u8,
}

impl Default for AiSettings {
    /// Plan N4 defaults: Stick By Sora, Watch MP, the cure style 0 with all three target bits.
    fn default() -> AiSettings {
        AiSettings { battle: 0, ability: 1, cure: 0, cure_targets: 0b111 }
    }
}

/// A friend's status.
#[derive(Clone, Debug, PartialEq)]
pub struct FriendStatus {
    pub chara: Chara,
    pub level: u32,
    /// The shared total the level was last computed from.
    pub total_exp: i32,
    pub hp: i32,
    pub mp: i32,
    /// MP charge running (MP 0 after a spell, or Cure's "all MP"), seconds left.
    pub mp_charge_left: f32,
    /// Weapon key (`WEP_DONALD_00` ...).
    pub weapon: &'static str,
    pub abilities: AbilitySet,
    /// KH3's victory-bonus counts of an old save, kept in the record and read by nothing: (GF-STATS) `growth` replaced them.
    pub bonuses: BonusCounts,
    pub items: [ItemSlot; MAX_SLOTS],
    /// Item slots owned: TresChrInitData m_FriendDonald m_InitItemSlot 2; Goofy does not serialize it -> the
    /// UTresChrInitData ctor 0x140571C00 default 3; (GF-STATS) + his boss rewards (`growth`).
    pub item_slots: u8,
    /// Armour (protector) and accessory slots: equipment row ids of kh-core's EQUIPMENT table.
    pub armour: [Option<&'static str>; MAX_SLOTS],
    pub accessories: [Option<&'static str>; MAX_SLOTS],
    /// Slots owned: `TresChrInitEquip` m_InitProtectorSlot / m_InitAccessorySlot (Donald accessory 2, Goofy protector 2)
    /// + (GF-STATS) his boss rewards (`growth`). A count the record does not serialize is the struct default,
    ///   `INIT_SLOT_DEFAULT` 1 (0x140570AC0).
    pub armour_slots: u8,
    pub accessory_slots: u8,
    pub ai: AiSettings,
    pub game_level: GameLevel,
    /// (party X3D) the port's own abilities of the playable kit (Donald's: crate::pad::donald_kit); empty for Goofy.
    pub kit: crate::pad::donald_kit::KitAbilities,
    /// (GOOFY-GG) Goofy's GUARD gauge (crate::guard): `Some` for Goofy only (Donald has none; Sora keeps his Focus). Its
    /// max follows `level` (synced in `tick`).
    pub guard: Option<crate::guard::Guard>,
    /// (IT) the roster's permanent stat boosts (crate::boosts; the party host copies its one total onto every friend):
    /// added on top of the stat tables by the getters below.
    pub roster: crate::boosts::RosterBoosts,
    /// (GF-STATS) what the bosses gave him, raw amounts (crate::growth; the party host keeps the roster's copy): max HP, max MP
    /// and the slots are derived from it.
    pub growth: crate::growth::Growth,
    /// (EQ) AUTO-RELOAD per item slot (Sora's pouch flag bit 2, kh-core magic/items.rs `PouchSlot::auto_reload`): a used
    /// slot with it keeps its item (`present` false, "used, waiting for refill") and the camp menu's close refills it
    /// from the shared stock; without it the slot is emptied on use (J 7.1). Off at the start (no slot refilled before).
    pub item_reload: [bool; MAX_SLOTS],
    /// (DON1) the Donald overhaul's state (crate::donald): Some for Donald (his combat core, his copy of the roster's
    /// learned magic, his abilities), None for Goofy.
    pub donald: Option<crate::donald::DonaldState>,
    /// (GOOFY) the Goofy overhaul's state (crate::goofy): Some for Goofy, pad or AI (his parry rules, his copy of the roster's
    /// ranks, his abilities, his running moves), None for Donald.
    pub goofy: Option<crate::goofy::GoofyState>,
}

/// The FTresChrInitEquip default protector / accessory slot count: 1 (struct ctor 0x140570AC0 via
/// TCppStructOps Construct 0x141DA4C50, and UTresChrInitData ctor 0x140571C00 for m_FriendDonald / m_FriendGoofy);
/// the item slot default is 3 (`INIT_ITEM_SLOT_DEFAULT`). Sheet rows `init.*`.
pub const INIT_SLOT_DEFAULT: u8 = 1;
pub const INIT_ITEM_SLOT_DEFAULT: u8 = 3;

/// (GF-STATS) The party's limit of each slot kind (KH3's friends stop at 4; GOOFY_OVERHAUL.md 2, DONALD_OVERHAUL.md 2).
pub const MAX_SLOTS: usize = 5;

/// (GF-UI) THE AP OF A KH3 ABILITY ON A FRIEND: the party's row, else KH3's own table, the number the Abilities page shows. (Goofy's
/// Item Boost has no party row: it took no AP while the page counted 2, and his page read 28 / 26.)
pub fn kh3_ability_ap(k: kh_core::enums::ETresAbilityKind) -> i32 {
    tables::ability_cost(k.game_name().trim_start_matches("ETresAbilityKind::")).map(|c| c.0).or_else(|| kh_core::status::tables::ABILITIES.iter().find(|r| r.kind == k).and_then(|r| r.ap)).unwrap_or(0)
}

/// (GF-STATS) The (item, armour, accessory) slots a friend starts with: TresChrInitData, as the fields above say.
pub const fn start_slots(chara: Chara) -> (u8, u8, u8) {
    match chara {
        Chara::Donald => (2, INIT_SLOT_DEFAULT, 2),
        Chara::Goofy => (INIT_ITEM_SLOT_DEFAULT, 2, INIT_SLOT_DEFAULT),
    }
}

fn uid_kind(name: &str) -> Option<ETresAbilityKind> {
    ETresAbilityKind::from_game_name(name)
}

/// (DF, 2026-10-15) KH3 CRITICAL FOR THE FRIENDS: the base-param init 0x140575C80 halves the base max HP and MP
/// (`floor(x * 0.5)`, at least 1; 0x140575D73) for a data set of type 0 CDTS_TYPE_PLAYER when the game level is above 2.
/// Donald's and Goofy's `n_ex00x_DataTableSet` assets carry no `m_DataType`, so they are the class default PLAYER
/// (`TresChrDataTableSet` constructor 0x140571B88) [A]+[C]: the friends' bases are halved like Sora's. Their bonus
/// amounts follow the same Critical table (`BonusCounts::totals`), enemy damage on them is x2 (0x140642820, by the
/// attacker), and KH3 gives them no Critical AP or abilities (`m_FriendDonald` / `m_FriendGoofy` have no `m_Critical*`).
pub fn critical_base(base: i32, game_level: GameLevel) -> i32 {
    if game_level.is_critical() {
        ((base as f32 * 0.5).floor() as i32).max(1)
    } else {
        base
    }
}

impl FriendStatus {
    /// The status `TresChrInitData` gives (`m_FriendDonald` / `m_FriendGoofy`, plan 2.3), at the
    /// level the shared total gives.
    pub fn new(chara: Chara, total_exp: i32) -> FriendStatus {
        let (weapon, abilities, items): (&str, &[&str], [ItemKind; 2]) = match chara {
            Chara::Donald => ("WEP_DONALD_00", &["PRIZE_DRAW"] /* (DON-PROG) KH3's DONALD_FIRE .. CURE are gone: the overhaul's magic replaced them (donald::ability_info::KH3_SPELLS) */, [ItemKind::Potion, ItemKind::Ether]),
            Chara::Goofy => ("WEP_GOOFY_00", &["PRIZE_DRAW"] /* (GOOFY, R11) KH3's GOOFY_* seeds are gone: goofy::abilities::KH3_SEEDS */, [ItemKind::Potion, ItemKind::Potion]),
        };
        let mut set = AbilitySet::EMPTY;
        for a in abilities {
            if let Some(k) = uid_kind(a) {
                set.set(k, 1, 1);
            }
        }
        let mut items_arr = [ItemSlot::default(); MAX_SLOTS];
        let (item_slots, armour_slots, accessory_slots) = start_slots(chara);
        for (i, it) in items.iter().enumerate() {
            items_arr[i] = ItemSlot { item: Some(*it), present: true };
        }
        let mut s = FriendStatus { chara, level: 1, total_exp: 0, hp: 0, mp: 0, mp_charge_left: 0.0, weapon: tables::weapon(weapon).map(|w| w.1).unwrap_or("WEP_DONALD_00"), abilities: set, bonuses: BonusCounts::NONE, items: items_arr, item_slots, armour: [None; MAX_SLOTS], accessories: [None; MAX_SLOTS], armour_slots, accessory_slots, ai: AiSettings::default(), game_level: GameLevel::Standard, kit: Default::default() /* (party X3D) */, guard: (chara == Chara::Goofy).then(crate::guard::Guard::default) /* (GOOFY-GG) */, roster: crate::boosts::RosterBoosts::NONE /* (IT) */, growth: Default::default() /* (GF-STATS) */, item_reload: [false; MAX_SLOTS] /* (EQ) */, donald: (chara == Chara::Donald).then(crate::donald::DonaldState::new) /* (DON1) */, goofy: (chara == Chara::Goofy).then(crate::goofy::GoofyState::new) /* (GOOFY) */ };
        s.level = tables::level_for_exp(chara, total_exp);
        s.total_exp = total_exp;
        s.hp = s.max_hp();
        s.mp = s.max_mp();
        s.guard = s.guard.map(|_| crate::guard::Guard::for_level(s.level)); // (GOOFY-GG) full at his level
        s.don_sync_level(); // (DON-PROG) Donald's start (Scan, doc 10) and the abilities of his level, worn while his AP allows
        s.gof_sync_level(); // (GOOFY) the abilities of his level, worn while his AP allows
        s
    }

    /// Plan N2: the level-99 test loadout: (GF-STATS) every boss reward of the docs with their abilities granted and
    /// equipped while AP allows.
    pub fn test_level99(chara: Chara) -> FriendStatus {
        let mut s = FriendStatus::new(chara, 1_527_640);
        s.set_growth(crate::growth::Growth::full(chara));
        let bonus_abilities: &[&str] = match chara {
            Chara::Donald => &["MP_HASTE", "FIRE_UP", "BLIZZARD_UP", "THUNDER_UP", "AERO_UP", "COMBI_TIME", "COMBI_UP", "DAMAGE_ASPIR", "HYPER_HEALING"],
            Chara::Goofy => &["LUCK_UP", "COMBI_TIME", "COMBI_UP", "LAST_LEAVE", "SHARE_PRIZE", "HYPER_HEALING", "CRITICAL_HALF", "COMBO_LEAVE"],
        };
        for a in bonus_abilities {
            if let Some(k) = uid_kind(a) {
                let owned = s.abilities.owned(k).saturating_add(1);
                let equipped = s.abilities.equipped(k);
                s.abilities.set(k, owned, equipped);
                if s.ap_in_use() + tables::ability_cost(a).map(|c| c.0).unwrap_or(0) <= s.max_ap() {
                    s.abilities.set(k, owned, equipped + 1);
                }
            }
        }
        // (party X3D) the test loadout's kit: every one of Donald's port abilities owned, equipped while AP allows (plan
        // N2); the test level's host equips them all for the playtest (kh-partyhost lead.rs, `KitAbilities::test_all`)
        if chara == Chara::Donald {
            s.kit_test_loadout(bonus_abilities);
            s.don_test_loadout(bonus_abilities); // (DON-PROG) the overhaul's abilities within his AP (the X3D kit migrated: pad/donald_kit.rs)
        }
        // (GOOFY) the test loadout: every ability, every rank
        if chara == Chara::Goofy {
            s.goofy = Some(crate::goofy::GoofyState::test_loadout());
        }
        s.hp = s.max_hp();
        s.mp = s.max_mp();
        s
    }

    fn row(&self) -> (i32, i32, i32, i32, i32) {
        tables::level_row(self.chara, self.level)
    }

    /// (GOOFY-BAL) Strength / Magic / Defense before the weapon: his row of party-balance-levels (crate::balance).
    /// (ONESTAT, the user 2026-10-27: "there should be no party ally/leader seperation, their stats should be their stats same for Goofy and every KH character added later"): the same row leading or not (KH3's AI row was the friend's before).
    fn lead_row(&self) -> (i32, i32, i32) {
        crate::balance::base_stats(self.chara, self.level)
    }

    /// (GF-STATS) What the bosses have given him now (the party host's copy of the roster's record, a catch-up, the test
    /// loadout): max HP and MP follow, a full bar stays full and a gain is added to the gauges (a KO stays a KO). Slots are
    /// derived and never added to: an old save's saved count stands only until the record reaches it, so nothing counts twice.
    pub fn set_growth(&mut self, growth: crate::growth::Growth) {
        let (hp0, mp0) = (self.max_hp(), self.max_mp());
        self.growth = growth;
        let (item, armour, accessory) = start_slots(self.chara);
        let limit = |n: u8| n.min(MAX_SLOTS as u8);
        self.item_slots = limit(self.item_slots.max(item + growth.item_slots));
        self.armour_slots = limit(self.armour_slots.max(armour + growth.armour_slots));
        self.accessory_slots = limit(self.accessory_slots.max(accessory + growth.accessory_slots));
        let (hp1, mp1) = (self.max_hp(), self.max_mp());
        if self.hp > 0 {
            self.hp = (self.hp + (hp1 - hp0).max(0)).min(hp1);
        }
        self.mp = if self.mp_charge_left > 0.0 { self.mp } else { (self.mp + (mp1 - mp0).max(0)).min(mp1) };
    }

    fn weapon_stats(&self) -> (i32, i32, i32) {
        tables::weapon(self.weapon).map(|w| (w.3, w.4, w.5)).unwrap_or((0, 0, 0))
    }

    /// (GF-STATS) Max HP = his fixed start (Critical halves it) + what the bosses gave him (`growth`) + the roster's steps.
    /// (ONESTAT) leading or not. KH3's `bonuses` of an old save count for nothing here: the catch-up rebuilds them as `growth`.
    pub fn max_hp(&self) -> i32 {
        critical_base(crate::balance::start_pools(self.chara).0, self.game_level) + self.growth.hp // (DF) Critical halves the start
            + self.roster.hp_mp(self.game_level).0 // (IT) the roster's max-HP steps
    }

    pub fn max_mp(&self) -> i32 {
        critical_base(crate::balance::start_pools(self.chara).1, self.game_level) + self.growth.mp // (DF)
            + self.roster.hp_mp(self.game_level).1 // (IT) the roster's max-MP steps
            + crate::pad::donald::mp_bonus(self) // (DON-KIT) doc 2: Donald's Max MP +3 steps
    }

    /// Strength = level row + weapon [D].
    pub fn strength(&self) -> i32 {
        // (2026-10-27, the user: "lower Donald's physical damage as well, it should be something like 10 at level 1", "I mean
        // the KH3 attack stat in the status screen": ONESTAT; 2026-10-30, the user: Donald's ratio 0.3, 5 at level 1 with his starting staff)
        self.lead_row().0 + self.weapon_stats().0 // (GOOFY-BAL) his row of party-balance-levels
            + self.roster.strength as i32 // (IT) the roster's Strength Boosts
            + self.growth.boosts[0] as i32 // (weapons P11) the Strength Boosts used on him from the camp menu
    }

    pub fn magic(&self) -> i32 {
        if crate::balance::base_stats(self.chara, self.level).1 == 0 /* (ONESTAT) leading or not */ {
            return self.roster.magic as i32 + self.growth.boosts[1] as i32; // (GOOFY-BAL) a leader without magic (Goofy): no table / weapon magic; (IT) the roster's boosts stay on top; (weapons P11) and his own Magic Boosts
        }
        self.lead_row().1 + self.weapon_stats().1 // (GOOFY-BAL)
            + self.roster.magic as i32 // (IT) the roster's Magic Boosts
            + self.growth.boosts[1] as i32 // (weapons P11) the Magic Boosts used on him from the camp menu
    }

    pub fn defense(&self) -> i32 {
        self.lead_row().2 + self.weapon_stats().2 + self.worn().map(|r| r.defense).sum::<i32>() // (GOOFY-BAL) lead_row
            + self.roster.defense as i32 // (IT) the roster's Defense Boosts
            + self.growth.boosts[2] as i32 // (weapons P11) the Defense Boosts used on him from the camp menu
    }

    /// The worn armour and accessory rows.
    pub fn worn(&self) -> impl Iterator<Item = &'static kh_core::status::tables::EquipmentRow> + '_ {
        self.armour.iter().chain(self.accessories.iter()).flatten().filter_map(|id| kh_core::status::tables::equipment_row(id))
    }

    /// Resist percent by `ETresDamageAttribute` 0..7: the sum over worn equipment (base row resists are 0).
    pub fn resists(&self) -> [i32; 8] {
        let mut r = [0; 8];
        for row in self.worn() {
            for (i, v) in row.resist.iter().enumerate().take(8) {
                r[i] += v;
            }
        }
        r
    }

    /// Camp menu (F.2): armour slot `slot` (None = empty); refused beyond the owned slots or for a non-armour row.
    pub fn set_armour(&mut self, slot: usize, id: Option<&str>) -> bool {
        self.set_gear(slot, id, true)
    }

    pub fn set_accessory(&mut self, slot: usize, id: Option<&str>) -> bool {
        self.set_gear(slot, id, false)
    }

    fn set_gear(&mut self, slot: usize, id: Option<&str>, armour: bool) -> bool {
        let max = if armour { self.armour_slots } else { self.accessory_slots } as usize;
        if slot >= max.min(MAX_SLOTS) {
            return false;
        }
        let row = match id {
            Some(i) => match kh_core::status::tables::equipment_row(i) {
                Some(r) if r.kind == if armour { "armour" } else { "accessory" } => Some(r.id),
                _ => return false,
            },
            None => None,
        };
        if armour {
            self.armour[slot] = row;
        } else {
            self.accessories[slot] = row;
        }
        self.hp = self.hp.min(self.max_hp());
        true
    }

    pub fn max_ap(&self) -> i32 {
        self.row().4
            + self.roster.ap as i32 // (IT) the roster's AP Boosts
            + self.growth.boosts[3] as i32 // (weapons P11) the AP Boosts used on him from the camp menu
    }

    pub fn ap_in_use(&self) -> i32 {
        self.abilities.iter_equipped().filter(|(k, _)| !self.don_free_kh3(*k)) /* (DON-PROG) his KH3 spell gates take no AP */ .map(|(k, n)| kh3_ability_ap(k) * n as i32).sum::<i32>() + self.kit.ap_in_use() + self.gof_ap_in_use() /* (GOOFY) */ + self.don_ap_in_use() /* (DON-PROG) Donald's overhaul abilities */ // (party X3D) + the kit's; (party X3G) + Goofy's kit
    }

    pub fn count(&self, kind: ETresAbilityKind) -> u32 {
        self.abilities.count(kind) + self.don_kh3_count(kind) // (DON-PROG) his KH3-standard abilities (Second Chance, MP Haste, Lucky Lucky ...) count as KH3's
    }

    /// The level the shared total gives; returns the new level when it rose.
    pub fn set_total_exp(&mut self, total: i32) -> Option<u32> {
        self.total_exp = total;
        let lv = tables::level_for_exp(self.chara, total);
        if lv > self.level {
            let (hp_frac_full, mp_full) = (self.hp >= self.max_hp(), self.mp >= self.max_mp());
            self.level = lv;
            if hp_frac_full {
                self.hp = self.max_hp();
            }
            if mp_full {
                self.mp = self.max_mp();
            }
            if let Some(g) = self.guard.as_mut() {
                g.set_level(lv); // (GOOFY-GG) GUARD max grows with the level
            }
            Some(lv)
        } else {
            self.level = self.level.max(lv);
            None
        }
    }

    /// The attacker for `compute_damage` (friends are the player side for the difficulty factor).
    pub fn as_attacker(&self) -> Attacker<'_> {
        Attacker { has_status: true, strength: self.strength(), magic: self.magic(), abilities: Some(&self.abilities), side: Side::Player, state_boost: false }
    }

    /// The defender: kind Friend (bp+0x7C 4), the base row's floor / cap (24).
    pub fn as_defender(&self) -> Defender {
        Defender {
            has_status: true,
            defense: self.defense(),
            max_hp: self.max_hp(),
            hp_percent: 100.0 * self.hp.max(0) as f32 / self.max_hp().max(1) as f32,
            kind: DefenderKind::Friend,
            damage_min: tables::cparam(self.chara, "base.damage_min") as i32,
            damage_max: tables::cparam(self.chara, "base.damage_max") as i32,
            resist: self.resists(),
            second_resist: 0,
            cut: 0,
            quarter: false,
            bio_type: 0,
            critical_half: self.count(ETresAbilityKind::CriticalHalf),
        }
    }

    /// For a host's `Targets::defense` answer.
    pub fn target_defense(&self) -> TargetDefense {
        let d = self.as_defender();
        TargetDefense { defense: d.defense, max_hp: d.max_hp, hp_percent: d.hp_percent, enemy_kind: false, damage_min: d.damage_min, damage_max: d.damage_max, resist: d.resist, bio_type: 0 }
    }

    /// MP cost of an ability (`m_UseMP`; -1 = all MP); `None` when the ability is not equipped.
    pub fn mp_cost(&self, key: &str) -> Option<i32> {
        let kind = ETresAbilityKind::from_game_name(key)?;
        if self.abilities.equipped(kind) == 0 && self.gof_tree_gate(key) != Some(true) /* (GOOFY) his old tree's Tornado, Turbo, Discus: the Art is learned */ {
            return None;
        }
        let (_, mp) = tables::ability_cost(key)?;
        Some(if mp < 0 { self.mp.max(1) } else { mp })
    }

    /// Spends MP for an ability; starts the charge when MP reaches 0. False when it cannot be paid
    /// (charging, not enough MP, not equipped). `FUN_1405A4EA0(pawn, kind, 1)` is the game's gate
    /// (Tornado / Turbo Begin 0x1411C2130 / 0x1411C2530) [C].
    /// `pay` without spending: whether the gate would let the ability start now.
    /// (THE USER 2026-11-02: "let Goofy use his last MP and go into MP reserve") the party's MP rule (crate::mp_rule): any MP
    /// above 0 pays, a dearer ability takes the last of it into MP Charge; never while the charge runs.
    pub fn can_pay(&self, key: &str) -> bool {
        if self.mp_charge_left > 0.0 {
            return false;
        }
        self.mp_cost(key).is_some_and(|cost| crate::mp_rule::may_pay(self, cost))
    }

    pub fn pay(&mut self, key: &str) -> bool {
        if self.mp_charge_left > 0.0 {
            return false;
        }
        let Some(cost) = self.mp_cost(key) else { return false };
        crate::mp_rule::spend(self, cost).is_some() // (THE USER 2026-11-02) the party's MP rule: 0 starts MP Charge
    }

    /// Advances the MP charge; MP refills when it ends.
    pub fn tick(&mut self, dt: f32) {
        // (GOOFY-GG) GUARD: its max follows the level (every route that sets `level`), then the refill / rearm
        let level = self.level;
        if let Some(g) = self.guard.as_mut() {
            if g.level != level.clamp(1, 99) {
                g.set_level(level);
            }
            g.tick(dt);
        }
        if self.mp_charge_left > 0.0 {
            self.mp_charge_left -= dt * self.don_mp_haste(); // (DON-PROG) MP Haste / Hastera / Hastega on Donald (KH3's rate)
            if self.mp_charge_left <= 0.0 {
                self.mp_charge_left = 0.0;
                self.mp = self.max_mp();
            }
        }
    }

    /// (GOOFY-GG) The GUARD view for the HUDs and menus (Goofy only).
    pub fn guard_view(&self) -> Option<crate::guard::GuardView> {
        self.guard.as_ref().map(|g| g.view())
    }

    /// (GOOFY-GG) GUARD full and unbroken (save point / grace / a join), at the current level.
    pub fn refill_guard(&mut self) {
        let level = self.level;
        if let Some(g) = self.guard.as_mut() {
            g.set_level(level);
            g.refill();
        }
    }

    pub fn ko(&self) -> bool {
        self.hp < 1
    }

    /// (B4) `ItemEffect` 0x1405BE820 on this friend (the shared pawn function kh-core ports for Sora as
    /// `Status::item_effect`), with the multiplier the caller computed: 0.5 when the target is in Die (state
    /// 0x3B; the friends' pawn type +0x3B0 = 10 (ctor 0x141254FF0) lets the item through, any other pawn type in
    /// Die / Gameover gets nothing), times 1 + 0.5 x the USER's ITEM_UP count (FUN_1405BCBE0 passes
    /// user vt+0x1500(0x28)). Amounts are round-half-up of mult x amount; Ether / Hi-Ether shorten an MP charge
    /// instead (by total x 0.5 x mult / total x mult). Returns the HP gained.
    pub fn item_effect(&mut self, item: ItemKind, mult: f32) -> i32 {
        use kh_core::status::math::round_half_up;
        use kh_core::status::runtime::{ETHER_CHARGE_FRACTION, ETHER_MP, HI_POTION_HP, POTION_HP};
        let before = self.hp.max(0);
        let total = crate::mp_rule::charge_time(self); // (THE USER 2026-11-02) his own charge length (Donald's R7 one)
        let charge = |s: &mut FriendStatus, seconds: f32| {
            s.mp_charge_left -= seconds;
            if s.mp_charge_left <= 0.0 {
                s.mp_charge_left = 0.0;
                s.mp = s.max_mp();
            }
        };
        match item {
            ItemKind::Potion => self.hp = (before + round_half_up(mult * POTION_HP)).min(self.max_hp()),
            ItemKind::HiPotion | ItemKind::MegaPotion => self.hp = (before + round_half_up(mult * HI_POTION_HP)).min(self.max_hp()),
            ItemKind::Ether => {
                if self.mp_charge_left > 0.0 {
                    charge(self, total * ETHER_CHARGE_FRACTION * mult);
                } else {
                    self.mp = (self.mp + round_half_up(mult * ETHER_MP)).min(self.max_mp());
                }
            }
            ItemKind::HiEther | ItemKind::MegaEther => {
                if self.mp_charge_left > 0.0 {
                    charge(self, total * mult);
                } else {
                    self.mp = (self.mp + round_half_up(self.max_mp() as f32 * mult)).min(self.max_mp());
                }
            }
            ItemKind::Elixir | ItemKind::Megalixir => {
                self.hp = self.max_hp();
                if self.mp_charge_left > 0.0 {
                    self.mp_charge_left = 0.0;
                }
                self.mp = self.max_mp();
            }
            _ => {}
        }
        self.hp - before
    }

    /// Heals `amount` (halved, rounded up, while KO'd: plan 2.5); returns the HP gained.
    pub fn heal(&mut self, amount: i32) -> i32 {
        let amount = if self.ko() { (amount + 1) / 2 } else { amount };
        let before = self.hp.max(0);
        self.hp = (before + amount).min(self.max_hp());
        self.hp - before
    }

    /// The revive amount (0x141264140): max(1, RoundToInt(maxHP x (n + 1) x 25 x 0.01)),
    /// n = min(count HYPER_HEALING, 3).
    pub fn revive_hp(&self) -> i32 {
        revive_amount(self.max_hp(), self.count(ETresAbilityKind::HyperHealing))
    }

    /// The KO timer IN PARTY COMBAT: (HUD-REVIVE, THE USER 2026-11-07: "They revive in combat after 60 seconds as a baseline")
    /// row `revive.in_combat_time` (60 s) divided by 1 + min(count HYPER_HEALING, 3) as KH3 divides its own (Die Begin
    /// 0x1412413C0: 30 s, Donald 40 s, Goofy 35 s: rows `n_ex00x.revive_time`, kept as the record of KH3's numbers).
    pub fn revive_time(&self) -> f32 {
        revive_time_in_combat(self.count(ETresAbilityKind::HyperHealing))
    }

    /// (HUD-REVIVE) KH3's own Die timer of this friend (row `n_ex00x.revive_time` / (1 + n)): the record, not the rule.
    pub fn kh3_revive_time(&self) -> f32 {
        let n = self.count(ETresAbilityKind::HyperHealing).min(tables::param("revive.hyper_healing_cap") as u32) as f32;
        tables::cparam(self.chara, "revive_time") / (n + 1.0)
    }

    /// Camp menu (F.2): equips a weapon of this character by item row or key.
    pub fn equip_weapon(&mut self, id: &str) -> Result<(), &'static str> {
        let w = tables::weapon(id).ok_or("no such weapon")?;
        if w.2 != self.chara.uid() {
            return Err("another character's weapon");
        }
        self.weapon = w.1;
        self.hp = self.hp.min(self.max_hp());
        Ok(())
    }

    /// The weapon row (row, key, uid, strength, magic, defense, abilities, model).
    pub fn weapon_row(id: &str) -> Option<&'static tables::PartyWeaponRow> {
        tables::weapon(id)
    }

    /// Camp menu: sets item slot `slot` (0..MAX_SLOTS) to `item` (None = empty).
    pub fn set_item_slot(&mut self, slot: usize, item: Option<ItemKind>) {
        if let Some(s) = self.items.get_mut(slot) {
            *s = ItemSlot { item, present: item.is_some() };
        }
    }

    /// Camp menu: equips / unequips copy `copy` of an ability; refused over the AP budget.
    pub fn set_ability(&mut self, kind: ETresAbilityKind, copy: u8, on: bool) -> bool {
        let before = self.abilities;
        if !self.abilities.set_copy(kind, copy, on) {
            return false;
        }
        if self.ap_in_use() > self.max_ap() {
            self.abilities = before;
            return false;
        }
        true
    }

    /// Uses the first present item of `kind`; false when there is none.
    pub fn take_item(&mut self, kind: ItemKind) -> bool {
        match self.items.iter().position(|s| s.present && s.item == Some(kind)) {
            Some(k) => {
                self.items[k].present = false;
                // (EQ) Sora's pouch rule (J 7.1, kh-core `Pouch::use_item`): without auto-reload the slot is emptied
                if !self.item_reload[k] {
                    self.items[k].item = None;
                }
                true
            }
            None => false,
        }
    }

    /// (EQ) The camp menu's item-slot write with Sora's rules (kh-core customize `equip_item`): the same item again (a
    /// refill of a used auto-reload slot) keeps the slot's auto-reload; another item takes the setting of the other
    /// slots that hold it; empty clears it.
    pub fn set_item_slot_kh3(&mut self, slot: usize, item: Option<ItemKind>) {
        if slot >= MAX_SLOTS {
            return;
        }
        let same = item.is_some() && self.items[slot].item == item;
        let reload = match item {
            _ if same => self.item_reload[slot],
            Some(new) => (0..MAX_SLOTS).any(|k| k != slot && self.items[k].item == Some(new) && self.item_reload[k]),
            None => false,
        };
        self.items[slot] = ItemSlot { item, present: item.is_some() };
        self.item_reload[slot] = reload;
    }

    /// (EQ) Triangle on an item row (kh-core customize `toggle_auto_reload`): Auto-Reload of `item` on every slot that
    /// holds it (used or not); turning it off empties a used slot. False when no slot holds the item.
    pub fn set_auto_reload(&mut self, item: ItemKind, on: bool) -> bool {
        let mut any = false;
        for k in 0..MAX_SLOTS {
            if self.items[k].item == Some(item) {
                any = true;
                self.item_reload[k] = on;
                if !on && !self.items[k].present {
                    self.items[k] = ItemSlot::default();
                }
            }
        }
        any
    }

    /// Refills used item slots (the pouch rule: after a battle / pause; the host decides when).
    pub fn refill_items(&mut self) {
        for s in self.items.iter_mut() {
            if s.item.is_some() {
                s.present = true;
            }
        }
    }
}

/// (HUD-REVIVE) THE KO TIMER IN PARTY COMBAT for any member (a friend, AI Sora, any later one): row `revive.in_combat_time`
/// (60 s, the user's baseline) / (1 + min(`hyper_healing`, revive.hyper_healing_cap)), KH3's divisor (Die Begin 0x1412413C0).
pub fn revive_time_in_combat(hyper_healing: u32) -> f32 {
    let n = hyper_healing.min(tables::param("revive.hyper_healing_cap") as u32) as f32;
    tables::param("revive.in_combat_time") / (n + 1.0)
}

/// (HUD-REVIVE) KH3'S REVIVE AMOUNT for any member (0x141264140): max(1, RoundToInt(maxHP x (n + 1) x 25 x 0.01)),
/// n = min(`hyper_healing`, 3). The timer's revive and the party combat's end both give it; the save point gives full HP.
pub fn revive_amount(max_hp: i32, hyper_healing: u32) -> i32 {
    let n = hyper_healing.min(tables::param("revive.hyper_healing_cap") as u32) as f32;
    let x = (max_hp as f32 * (n + 1.0) * tables::param("revive.hp_percent")) * 0.01;
    ((x + 0.5).floor() as i32).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_99_test_loadout() {
        let d = FriendStatus::test_level99(Chara::Donald);
        let g = FriendStatus::test_level99(Chara::Goofy);
        assert_eq!((d.level, g.level), (99, 99));
        // (GF-STATS) every reward of the docs
        assert_eq!((d.max_hp(), d.max_mp(), g.max_hp(), g.max_mp()), (232, 218, 376, 136));
        assert_eq!((d.item_slots, d.armour_slots, d.accessory_slots, g.item_slots, g.armour_slots, g.accessory_slots), (5, 2, 5, 5, 5, 3));
        assert_eq!(d.strength(), crate::balance::base_stats(Chara::Donald, 99).0 + 3);
        assert_eq!(d.magic(), crate::balance::base_stats(Chara::Donald, 99).1 + 4);
        assert_eq!(g.strength(), crate::balance::base_stats(Chara::Goofy, 99).0 + 4);
        assert!(d.ap_in_use() <= d.max_ap());
        assert_eq!(d.abilities.owned(ETresAbilityKind::HyperHealing), 1); // owned; equipped only while AP allows (plan N2)
    }

    #[test]
    fn ko_revive_numbers() {
        let mut d = FriendStatus::new(Chara::Donald, 0);
        let g = FriendStatus::new(Chara::Goofy, 0);
        // (HUD-REVIVE) the user's 60 s in party combat for every member; KH3's own 40 / 35 s stay on record
        assert_eq!(d.revive_time(), 60.0);
        assert_eq!(g.revive_time(), 60.0);
        assert_eq!((d.kh3_revive_time(), g.kh3_revive_time()), (40.0, 35.0));
        let quarter = ((d.max_hp() as f32 * 0.25 + 0.5).floor() as i32).max(1);
        assert_eq!(d.revive_hp(), quarter); // max HP x 25 % (ONESTAT: his balanced max HP)
        d.abilities.set(ETresAbilityKind::HyperHealing, 1, 1);
        assert_eq!(d.revive_time(), 30.0);
        assert_eq!(d.kh3_revive_time(), 20.0);
        assert_eq!(d.revive_hp(), ((d.max_hp() as f32 * 0.5 + 0.5).floor() as i32).max(1));
        d.hp = 0;
        assert_eq!(d.heal(9), 5); // halved, rounded up, while KO'd
    }

    #[test]
    fn mp_and_charge() {
        let mut d = FriendStatus::new(Chara::Donald, 0);
        // (DON-PROG) KH3's ability gate itself (the machinery Goofy's abilities use in play): Donald no longer starts with
        // his KH3 spell abilities (the overhaul replaced them), so the test gives them
        d.abilities.set(ETresAbilityKind::DonaldFire, 1, 1);
        d.abilities.set(ETresAbilityKind::DonaldCure, 1, 1);
        let full = d.max_mp(); // (ONESTAT) his balanced max MP
        assert!(d.pay("DONALD_FIRE"));
        assert_eq!(d.mp, full - 10);
        assert!(d.pay("DONALD_CURE")); // all MP
        assert_eq!(d.mp, 0);
        assert_eq!(d.mp_charge_left, crate::mp_rule::charge_time(&d)); // (THE USER 2026-11-02) his own length (R7), at once
        assert!(d.mp_charge_left > 0.0 && d.mp_charge_left <= 30.0);
        assert!(!d.pay("DONALD_FIRE"));
        for _ in 0..(30 * 60 + 1) {
            d.tick(1.0 / 60.0);
        }
        assert_eq!(d.mp, full);
        assert!(!d.pay("GOOFY_TURBO")); // not his
    }

    #[test]
    fn shared_exp_levels_with_own_tables() {
        let mut d = FriendStatus::new(Chara::Donald, 0);
        let mut g = FriendStatus::new(Chara::Goofy, 0);
        assert_eq!(d.set_total_exp(28_500), Some(20)); // Donald's row 20 needs 28,242
        assert_eq!(g.set_total_exp(28_500), Some(19)); // Goofy's needs 28,841
        assert_eq!(g.set_total_exp(28_841), Some(20));
    }

    #[test]
    fn damage_max_24_limits_the_base() {
        // CalcDamage 0x140632240 step 5 (kh-core damage.rs, instruction level): a base above
        // m_DamageMax (24) becomes floor((24 + S - D) / 2); friends: base at most 80.
        use kh_core::status::{compute_damage, DamageAttack, DamageContext};
        let d = FriendStatus::new(Chara::Donald, 0);
        let def = d.as_defender();
        assert_eq!((def.damage_max, def.kind), (24, DefenderKind::Friend));
        let cx = DamageContext::new(GameLevel::Standard, 1);
        let r = compute_damage(&Attacker::enemy(40), &def, &DamageAttack::physical(1.0), &cx);
        assert_eq!(r.base, ((24 + 40 - d.defense()) / 2) as f32); // 40 - 11 = 29 > 24 -> 26
        let r = compute_damage(&Attacker::enemy(20), &def, &DamageAttack::physical(1.0), &cx);
        assert_eq!(r.base, (20 - d.defense()) as f32); // under the max: S - D
        let r = compute_damage(&Attacker::enemy(400), &def, &DamageAttack::physical(1.0), &cx);
        assert_eq!(r.base, 80.0);
    }
}

#[cfg(test)]
mod gear_tests {
    use super::*;

    #[test]
    fn armour_and_accessories_add_defense_and_resists() {
        let mut d = FriendStatus::new(Chara::Donald, 0);
        assert_eq!((d.armour_slots, d.accessory_slots), (INIT_SLOT_DEFAULT, 2)); // TresChrInitData m_FriendDonald
        let g = FriendStatus::new(Chara::Goofy, 0);
        assert_eq!(g.armour_slots, 2); // m_FriendGoofy m_InitProtectorSlot
        // FTresChrInitEquip ctor 0x140570AC0 / UTresChrInitData ctor 0x140571C00: protector 1, accessory 1, item 3
        assert_eq!((INIT_SLOT_DEFAULT, INIT_ITEM_SLOT_DEFAULT), (1, 3));
        assert_eq!((tables::param("init.protector_slot_default"), tables::param("init.item_slot_default")), (1.0, 3.0));
        assert_eq!((d.item_slots, g.item_slots, g.accessory_slots), (2, 3, 1));
        let armour = kh_core::status::tables::EQUIPMENT.iter().find(|r| r.kind == "armour" && r.resist.iter().any(|&x| x != 0)).unwrap();
        let def0 = d.defense();
        assert!(d.set_armour(0, Some(armour.id)));
        assert_eq!(d.defense(), def0 + armour.defense);
        assert_eq!(d.as_defender().resist[..armour.resist.len().min(8)], armour.resist[..armour.resist.len().min(8)]);
        assert!(!d.set_armour(MAX_SLOTS, Some(armour.id)));
        assert!(!d.set_armour(1, Some(armour.id)), "only his one armour slot is open");
        assert!(!d.set_accessory(0, Some(armour.id)), "an armour row is not an accessory");
        let ai = AiSettings::default();
        assert_eq!(ai.cure_targets, 0b111);
    }
}
