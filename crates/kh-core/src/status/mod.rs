//! OWNER: the stats implementer (docs/sora-stats-spec.md). Sora's status: level, EXP, the three
//! gauges, stats, abilities, magic, equipment, items, and the rules that read and write them.
//!
//! THE MODEL (stats spec section 0): nothing about his level or stats is stored except the total
//! EXP. `Status` holds the SAVE DATA (EXP, difficulty, the two opening choices, the victory-bonus
//! counters, boosts, equipment, the ability words) and the RUNTIME values of the pawn (HP, MP,
//! focus, the MP-charge and Withstand Combo timers); everything else is recomputed by
//! [`Status::rebuild`] (the game's 0x140575C80) and by the stat getters (spec 2.7), which are
//! methods because some of them depend on the moment (Defender on the HP percent, Berserk Charge
//! on the MP charge).
//!
//! | file | what | spec |
//! |---|---|---|
//! | `tables.rs` | the generated sheets: levels, abilities, difficulty, equipment, rule constants | 2.2, 3, 5.3 |
//! | `math.rs` | the engine's `ceil` / `floor` forms | header |
//! | `ability_set.rs` | owned / equipped / worn / meal copies and the count query | 5.1 |
//! | `mod.rs` (here) | `Status`, `rebuild`, the stat getters, the AP budget | 1, 2, 5.2 |
//! | `loadout.rs` | the initial save data (`sheets/port/sora-loadout-99.json`) | 8, 9 |
//! | `exp.rs` | `give_exp`: reward, award caps, level-up | 4 |
//! | `damage.rs` | `compute_damage`: the damage function 0x140632240 | 7 |
//! | `runtime.rs` | HP / MP / focus rules: damage to Sora, heals, MP use and charge, prizes, items | 6, magic spec C, D, E |
//!
//! TYPES NARROWED (2026-10-11, as this file's first version announced): HP, MP, focus and their
//! maxima are `i32` (the pawn's `m_HitPoint` ... are int32), the EXP is `i32` (GW+0x7E00).
//!
//! WHAT OTHER SYSTEMS CALL (the public API of the status side):
//! - abilities: `status.has(kind)` / `status.count(kind)` (with the player gates of spec 5.2);
//! - combat: [`damage::compute_damage`], `status.as_attacker()`, `status.as_defender()`,
//!   `status.apply_damage_to_sora(..)`, `status.in_damage_state` (Withstand Combo's hook);
//! - magic: `status.effective_mp_cost`, `can_cast`, `use_mp`, `damage_invalid`, `use_item`,
//!   the focus aim API (`focus_aim_begin` / `FocusAim::tick` / `focus_aim_end`);
//! - hosts: `Sora::give_exp`, `Sora::status_request` (sora.rs), which raise the HUD popups.

pub mod ability_set;
pub mod damage;
pub mod exp;
pub mod loadout;
pub mod math;
// (new game) the two choices of the opening
pub mod newgame;
pub mod requests;
// (new game) rewards: abilities, magic, bonuses given by other means than the level table
pub mod reward;
pub mod runtime;
pub mod tables;

pub use ability_set::{AbilitySet, ABILITY_SLOTS};
pub use damage::{compute_damage, Attacker, DamageAttack, DamageContext, DamageResult, Defender, DefenderKind, Side};
pub use exp::{ExpAward, ExpPlayer, LevelUp}; // (party P2) ExpPlayer
pub use loadout::{Loadout, LoadoutValue};
pub use newgame::{OpeningChoice, Power};
pub use reward::{BonusLine, Reward, RewardOutcome};
pub use requests::StatusRequest;
pub use runtime::{DamageOutcome, FocusAim, HitOnSora, ItemKind, ItemOutcome, MpCost, MpUse, PrizeKind, Survived};
pub use tables::GameLevel;

use crate::enums::{ETresAbilityKind, ETresDamageAttribute};
use math::ue_ceil;

/// The stat block as the menu shows it: the FINAL Strength / Magic / Defense (spec 2.7) and the
/// AP in use / available. A snapshot refreshed by `rebuild` and every `tick`; rules call the
/// getters (`status.strength()` ...), which are always current.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Stats {
    pub strength: i32,
    pub magic: i32,
    pub defense: i32,
    /// Ability points in use and available.
    pub ap: i32,
    pub ap_max: i32,
}

/// The four numbers of a level row (`bp+0x5C..+0x68`), absolute values.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct LevelStats {
    pub strength: i32,
    pub magic: i32,
    pub defense: i32,
    pub ap: i32,
}

/// The first choice of the opening (spec 2.3). It acts ONLY through one of three victory-bonus
/// rows (the value the game also stores at GW+0x7E0C is never read).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Growth {
    /// First choice 0: `Vbonus_079`, `HP_UP30`.
    Vitality,
    /// First choice 1: `Vbonus_080`, `HP_UP15` + `MP_UP10`.
    Balance,
    /// First choice 2: `Vbonus_081`, `MP_UP20`.
    Wisdom,
}

impl Growth {
    pub const ALL: [Growth; 3] = [Growth::Vitality, Growth::Balance, Growth::Wisdom];

    /// By the loadout sheet's word (vitality / balance / wisdom), any case.
    pub fn from_name(name: &str) -> Option<Growth> {
        match name.trim().to_ascii_lowercase().as_str() {
            "vitality" => Some(Growth::Vitality),
            "balance" => Some(Growth::Balance),
            "wisdom" => Some(Growth::Wisdom),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Growth::Vitality => "vitality",
            Growth::Balance => "balance",
            Growth::Wisdom => "wisdom",
        }
    }

    /// The `ETresVictoryBonusKind`s of its bonus row.
    pub const fn bonus_kinds(self) -> &'static [usize] {
        match self {
            Growth::Vitality => &[BonusCounts::HP_UP30],
            Growth::Balance => &[BonusCounts::HP_UP15, BonusCounts::MP_UP10],
            Growth::Wisdom => &[BonusCounts::MP_UP20],
        }
    }
}

/// The victory-bonus counters of the save record (`record+0x960 + kind`, 32 bytes): how many
/// bonus rows of each `ETresVictoryBonusKind` have their game flag set (`RecountBonuses`
/// 0x140E90EE0). Index = the enum value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct BonusCounts(pub [u8; 32]);

impl BonusCounts {
    pub const HP_UP3: usize = 1;
    pub const HP_UP5: usize = 2;
    pub const HP_UP10: usize = 3;
    pub const HP_UP15: usize = 4;
    pub const HP_UP30: usize = 5;
    pub const MP_UP3: usize = 8;
    pub const MP_UP5: usize = 9;
    pub const MP_UP10: usize = 10;
    pub const MP_UP20: usize = 11;
    pub const DEF_SLOT_UP1: usize = 16;
    pub const ACC_SLOT_UP1: usize = 17;
    pub const ITEM_SLOT_UP1: usize = 18;
    /// `MELEM_FIRE` .. `MELEM_CURE` = 24..29: magic element tiers.
    pub const MELEM_FIRE: usize = 24;

    pub const NONE: BonusCounts = BonusCounts([0; 32]);

    /// Sora's story rows `Vbonus_001..084` WITHOUT the three opening-choice rows (spec 2.4 table,
    /// [A] `TresVBonusData`): 15 x HP_UP10, 2 x HP_UP5, 7 x MP_UP5, 2 armour slots, 2 accessory
    /// slots, 3 item slots, 3 tiers of each of the six magic elements.
    pub const STORY: BonusCounts = {
        let mut c = [0u8; 32];
        c[Self::HP_UP10] = 15;
        c[Self::HP_UP5] = 2;
        c[Self::MP_UP5] = 7;
        c[Self::DEF_SLOT_UP1] = 2;
        c[Self::ACC_SLOT_UP1] = 2;
        c[Self::ITEM_SLOT_UP1] = 3;
        let mut element = 0;
        while element < 6 {
            c[Self::MELEM_FIRE + element] = 3;
            element += 1;
        }
        BonusCounts(c)
    };

    /// The story rows plus Re Mind's `VBonus_DLC_001..015` (8 x MP_UP3, 2 item slots): what the
    /// level-99 numbers of spec section 9 are written for.
    pub const STORY_AND_REMIND: BonusCounts = {
        let mut c = Self::STORY.0;
        c[Self::MP_UP3] = 8;
        c[Self::ITEM_SLOT_UP1] += 2;
        BonusCounts(c)
    };

    /// The enum value of a kind by its name (`HP_UP10`, `MP_UP3`, `ITEM_SLOT_UP1` ...).
    pub fn kind_by_name(name: &str) -> Option<usize> {
        Some(match name.trim().to_ascii_uppercase().as_str() {
            "HP_UP3" => Self::HP_UP3,
            "HP_UP5" => Self::HP_UP5,
            "HP_UP10" => Self::HP_UP10,
            "HP_UP15" => Self::HP_UP15,
            "HP_UP30" => Self::HP_UP30,
            "MP_UP3" => Self::MP_UP3,
            "MP_UP5" => Self::MP_UP5,
            "MP_UP10" => Self::MP_UP10,
            "MP_UP20" => Self::MP_UP20,
            "DEF_SLOT_UP1" => Self::DEF_SLOT_UP1,
            "ACC_SLOT_UP1" => Self::ACC_SLOT_UP1,
            "ITEM_SLOT_UP1" => Self::ITEM_SLOT_UP1,
            "MELEM_FIRE" => 24,
            "MELEM_BLIZZARD" => 25,
            "MELEM_THUNDER" => 26,
            "MELEM_WATER" => 27,
            "MELEM_AERO" => 28,
            "MELEM_CURE" => 29,
            _ => return None,
        })
    }

    /// One more row of `kind` (a bonus awarded: 0x140E86BC0).
    pub fn add(&mut self, kind: usize) {
        if kind < 32 {
            self.0[kind] = self.0[kind].saturating_add(1);
        }
    }

    /// The amount one bonus of `kind` gives on a game level (`BonusAmount` 0x1405864C0): the
    /// difficulty sheet's `hp_up` (kinds 1..5) and `mp_up` (kinds 8..11) columns; Critical has
    /// its own table (1, 2, 5, 7, 15 / 1, 2, 5, 10). Returns (HP, MP).
    pub fn amount(kind: usize, game_level: GameLevel) -> (i32, i32) {
        let row = game_level.row();
        match kind {
            1..=5 => (row.hp_up[kind - 1], 0),
            8..=11 => (0, row.mp_up[kind - 8]),
            _ => (0, 0),
        }
    }

    /// `BonusTotals` 0x140E8CE20: the max-HP and max-MP bonus sums (`bp+0x6C` / `bp+0x6E`).
    pub fn totals(&self, game_level: GameLevel) -> (i32, i32) {
        let (mut hp, mut mp) = (0, 0);
        for kind in [1, 2, 3, 4, 5, 8, 9, 10, 11] {
            let (h, m) = Self::amount(kind, game_level);
            hp += h * self.0[kind] as i32;
            mp += m * self.0[kind] as i32;
        }
        (hp, mp)
    }
}

/// The boost-item bytes of the save record (`record+0x980..+0x983`): added by the stat getters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Boosts {
    pub strength: u8,
    pub magic: u8,
    pub defense: u8,
    /// AP Boost items, and on Critical the start value 30 (`m_CriticalUseAPUpNum`, 0x140E92280).
    pub ap: u8,
}

/// The active meal (`bp+0x138`): A STUB INPUT. Cooking is out of scope (spec 2.5); a host or a
/// test sets one to exercise the meal terms of the getters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Meal {
    pub hp: i32,
    pub mp: i32,
    pub strength: i32,
    pub magic: i32,
    pub defense: i32,
    /// The full-course bonus ability (`+0x30`, one of ids 170..186).
    pub ability: Option<ETresAbilityKind>,
}

/// A spell he knows, by the loadout sheet's name ("fire", "cure") and its tier (1 = Fire,
/// 2 = Fira, 3 = Firaga ...).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KnownMagic {
    pub name: String,
    pub tier: u32,
}

/// A number of one item, by the game's item name ("ITEM_POTION").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemStack {
    pub item: String,
    pub count: u32,
}

/// Sora's status. Plain data: part of `Sora`'s save state.
#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    // ------------------------------------------------------------------ save data
    /// Difficulty (`GW+0x7DF0`).
    pub game_level: GameLevel,
    /// TOTAL EXP (`GW+0x7E00`), 0..=9,999,999: the only stored number of his progression.
    pub exp: i32,
    /// The first opening choice, when one was made (it acts through `bonuses` only).
    pub growth: Option<Growth>,
    /// The second opening choice: the ability path 0 / 1 / 2 (`GW+0x7E0D`).
    pub ability_path: u8,
    /// Battle level of the area (`GW+0x7E09`, 1..=99): EXP Incentive and the damage floors read it.
    pub battle_level: u8,
    /// Script action flags (`GW+0x95B0`): gates of some abilities (spec 5.2). A free test level
    /// presents all bits as set.
    pub action_flags: u8,
    /// Victory-bonus counters, the opening-choice row included.
    pub bonuses: BonusCounts,
    pub boosts: Boosts,
    /// The active meal (stub input).
    pub meal: Option<Meal>,
    /// The keyblade's name in the equipment sheet ("WEP_KEYBLADE_SO_00") and its upgrade level.
    pub weapon: String,
    pub weapon_upgrade: u8,
    pub armour: Vec<String>,
    pub accessories: Vec<String>,
    /// Owned / equipped / worn copies. `rebuild` recounts the owned copies (base + level grants)
    /// and the worn ones; the equipped copies are the player's choice.
    pub abilities: AbilitySet,
    /// Owned copies that do NOT come from the level table (start set, victory bonuses).
    base_owned: [u8; ABILITY_SLOTS],
    pub magic: Vec<KnownMagic>,
    pub shotlocks: Vec<String>,
    /// The shortcut pages of the loadout (command names; read by command/mod.rs).
    pub shortcuts: [Vec<String>; 3],
    /// How many item slots he has, and what is in them.
    pub item_slots: u32,
    pub items: Vec<ItemStack>,
    pub munny: i32,
    // ------------------------------------------------------------------ recomputed by `rebuild`
    /// `bp+0x40`: the level of `exp`.
    pub level: u32,
    /// `bp+0x48`: total EXP of the current level's row.
    pub exp_level: i32,
    /// `bp+0x44`: total EXP of the next level's row; 0 at level 99.
    pub exp_next: i32,
    /// The level row's Strength / Magic / Defense / AP.
    pub base: LevelStats,
    /// `bp+0x50 / +0x54 / +0x58`: base max HP / MP / focus (HP and MP halved on Critical).
    pub hp_base: i32,
    pub mp_base: i32,
    pub focus_base: i32,
    /// `bp+0x6C / +0x6E`: the victory-bonus sums.
    pub hp_bonus: i32,
    pub mp_bonus: i32,
    pub hp_max: i32,
    pub mp_max: i32,
    pub focus_max: i32,
    pub stats: Stats,
    // ------------------------------------------------------------------ runtime (the pawn)
    /// `m_HitPoint` (pawn+0x5FC).
    pub hp: i32,
    /// `m_MagicPoint` (pawn+0x604).
    pub mp: i32,
    /// `m_FocusPoint` (pawn+0x614): the focus (shotlock) gauge.
    pub focus: i32,
    /// pawn+0x7C0 bit 1: the MP charge runs.
    pub mp_charging: bool,
    /// pawn+0x7C4: seconds of MP charge left.
    pub mp_charge_timer: f32,
    /// pawn+0x7C0 bit 0: "focus empty" (set when the gauge reaches 0, cleared when it is full).
    pub focus_empty: bool,
    /// pawn+0x848: the Withstand Combo timer, seconds (0.6 after every hit; counts down only
    /// while COMBO_LEAVE is counted).
    pub combo_leave_timer: f32,
    /// THE HOOK OF THE DAMAGE STATE (combat strand): true while the state queue's current or next
    /// state is `TSID_STATE_DAMAGE` (19). Withstand Combo reads it (0x1405A2B6F).
    pub in_damage_state: bool,
    /// pawn+0x630 bit 2 "damage invalid" (Leaf Bracer during a Cure, grand magic): set and
    /// cleared by the casting state; the hit path (`TakeDamage`, combat strand) must ask
    /// before applying damage.
    pub damage_invalid: bool,
    /// The pawn's "invincible / cinematic" answer (`vt+0xA48`): a lethal hit leaves 1 HP.
    pub invincible: bool,
    /// `m_bIsHpLimitOne`-style flag (`vt+0x2090`): a lethal hit leaves 1 HP.
    pub hp_limit_one: bool,
    /// `bp+0x88`: buff level 0..3 (x5 Strength). Source not determined (spec open question 5): 0
    /// unless a host sets it.
    pub buff_level: u8,
    /// Panacea's bad-status immunity, seconds left (pawn+0x864; magic spec I5).
    pub status_immunity: f32,
    /// (host bookkeeping, not a field of the game) How many times each battle item's effect has
    /// landed on him (`item_effect`, by the index of the item in `ItemKind::ALL`): from the item
    /// command and from the Items screen alike. A host compares it with what it saw before.
    pub item_effects: [u32; 12],
    /// (host choice, not a field of the game; IMPL_ER61) A factor on every spell's MP cost before MP Thrift /
    /// Magic Combo Thrift (`effective_mp_cost`): 1 = KH3's own costs (the test app). The Elden Ring host sets it
    /// from its sheet row `magic.mp_cost_scale`. A cost of "all MP" (Cure) is not touched.
    pub mp_cost_scale: f32,
    /// Walk-regeneration bookkeeping (pawn+0x838..+0x844).
    pub(crate) walk: runtime::WalkRegen,
    /// The port's own deterministic random stream for the status rules that roll (Wizard's
    /// Ruse). The game's generator is not reproduced.
    pub(crate) rng: u32,
    /// Things said once about this status (unknown equipment names ...).
    pub notes: Vec<String>,
    /// True while the values come from `Loadout::empty_placeholder` (no loadout was given):
    /// every number is 0 and nothing is equipped. Hosts should say so on screen.
    pub placeholder: bool,
}

impl Default for Status {
    /// The explicitly empty placeholder (see `Loadout::empty_placeholder`).
    fn default() -> Status {
        Status {
            game_level: GameLevel::Standard,
            exp: 0,
            growth: None,
            ability_path: 0,
            battle_level: 1,
            action_flags: 0xFF,
            bonuses: BonusCounts::NONE,
            boosts: Boosts::default(),
            meal: None,
            weapon: String::new(),
            weapon_upgrade: 0,
            armour: Vec::new(),
            accessories: Vec::new(),
            abilities: AbilitySet::EMPTY,
            base_owned: [0; ABILITY_SLOTS],
            magic: Vec::new(),
            shotlocks: Vec::new(),
            shortcuts: Default::default(),
            item_slots: 0,
            items: Vec::new(),
            munny: 0,
            level: 0,
            exp_level: 0,
            exp_next: 0,
            base: LevelStats::default(),
            hp_base: 0,
            mp_base: 0,
            focus_base: 0,
            hp_bonus: 0,
            mp_bonus: 0,
            hp_max: 0,
            mp_max: 0,
            focus_max: 0,
            stats: Stats::default(),
            hp: 0,
            mp: 0,
            focus: 0,
            mp_charging: false,
            mp_charge_timer: 0.0,
            focus_empty: false,
            combo_leave_timer: 0.0,
            in_damage_state: false,
            damage_invalid: false,
            invincible: false,
            hp_limit_one: false,
            buff_level: 0,
            status_immunity: 0.0,
            item_effects: [0; 12],
            mp_cost_scale: 1.0,
            walk: runtime::WalkRegen::default(),
            rng: 0x2545_F491,
            notes: Vec::new(),
            placeholder: true,
        }
    }
}

impl Status {
    /// The status a loadout starts him with ("constructing level 99 with everything", spec
    /// section 8): the save data as listed, everything else computed by the rules, the gauges
    /// full and no MP charge.
    pub fn from_loadout(loadout: &Loadout) -> Status {
        if loadout.placeholder {
            return Status::default();
        }
        let mut status = Status { placeholder: false, ..Status::default() };
        let mut notes = Vec::new();
        status.game_level = match GameLevel::from_name(&loadout.game_level) {
            Some(level) => level,
            None => {
                if !loadout.game_level.is_empty() {
                    notes.push(format!("game_level \"{}\" is not BEGINNER / STANDARD / PROUD / CRITICAL: STANDARD is used", loadout.game_level));
                }
                GameLevel::Standard
            }
        };
        status.growth = loadout.growth();
        status.ability_path = loadout.ability_path();
        status.battle_level = loadout.battle_level.clamp(1, 99);
        // 1. EXP: the loadout's, or the level's own total when it does not belong to the level.
        let level = loadout.level.clamp(1, tables::LEVEL_MAX);
        let exp = loadout.exp.min(tables::RULES.exp_total_cap as u64) as i32;
        status.exp = if tables::level_from_exp(exp) == level { exp } else { tables::level_row(level).exp_total };
        // 2. Victory bonuses: the listed counters plus the opening-choice row.
        status.bonuses = loadout.bonuses.unwrap_or(BonusCounts::NONE);
        for &kind in status.growth.map_or(&[][..], Growth::bonus_kinds) {
            status.bonuses.add(kind);
        }
        // 3. Abilities. The lists are written for `listed_level`: what the level table granted
        //    up to there is taken off, so that `rebuild` can grant by the actual level.
        let listed = if loadout.listed_level == 0 { level } else { loadout.listed_level };
        let granted = level_grant_counts(listed, status.ability_path);
        for &(kind, owned, equipped) in &loadout.abilities {
            let slot = kind.value() as usize;
            // (X1) an ability the port excludes (an old save's Attraction Extender ...) is dropped here:
            // not owned, not equipped, its AP free (decision dec029; `AbilitySet` refuses it as well).
            if slot < ABILITY_SLOTS && !tables::is_excluded(kind) {
                status.base_owned[slot] = owned.saturating_sub(granted[slot]);
            }
            status.abilities.set(kind, owned, equipped);
        }
        // Critical: the start set of the difficulty row (spec 2.6: `m_CriticalEquipAbility` =
        // Critical Counter and Critical Recharge, owned and equipped; `m_CriticalHaveAbility` =
        // Critical Converter, owned) and `m_CriticalUseAPUpNum` AP.
        let difficulty = status.game_level.row();
        for name in difficulty.start_abilities {
            // (DF, dec032) the port's exclusions (Critical Counter) are not given
            if let Some(kind) = ETresAbilityKind::from_game_name(name).filter(|k| !tables::critical_start_excluded(*k)) {
                let slot = kind.value() as usize;
                status.base_owned[slot] = status.base_owned[slot].max(1);
                let equipped = if kind == ETresAbilityKind::CriticalConverter { status.abilities.equipped(kind) } else { status.abilities.equipped(kind).max(1) };
                status.abilities.set(kind, status.abilities.owned(kind).max(1), equipped);
            }
        }
        status.boosts.ap = status.boosts.ap.saturating_add(difficulty.ap_add.clamp(0, 100) as u8);
        // 4. Equipment, magic, items.
        status.weapon = loadout.weapon.clone();
        status.weapon_upgrade = loadout.weapon_upgrade;
        status.armour = loadout.armour.clone();
        status.accessories = loadout.accessories.clone();
        status.magic = loadout.magic.clone();
        status.shotlocks = loadout.shotlocks.clone();
        status.shortcuts = loadout.shortcuts.clone();
        status.item_slots = loadout.item_slots;
        status.items = loadout.items.clone();
        status.notes = notes;
        status.rebuild();
        // At another level than the lists were written for (`Loadout::set_level`) the equipped
        // set may exceed the level's AP: copies are unequipped, highest ability id first, until
        // it fits. (A choice of this port for its test configurations, not a game rule: the
        // game's menu never lets the set exceed the AP.)
        if listed != level && !status.ap_within_budget() {
            let mut removed = 0;
            for &kind in ETresAbilityKind::ALL.iter().rev() {
                while status.abilities.equipped(kind) > 0 && tables::ability_ap(kind) > 0 && !status.ap_within_budget() {
                    status.abilities.set(kind, status.abilities.owned(kind), status.abilities.equipped(kind) - 1);
                    removed += 1;
                }
            }
            status.notes.push(format!("level {level}: {removed} equipped copies were unequipped to fit {} AP (the lists are written for level {listed})", status.max_ap()));
        }
        // 6. HP = max HP, MP = max MP, focus = max focus, no MP charge.
        status.full_recover();
        status
    }

    /// REBUILDS THE BASE PARAM from the tables and the save data (pawn `vt+0x1960` 0x1405A3E10 ->
    /// 0x140575C80; spec 2.2, 2.4, 2.5): the level from the EXP, the level row, base HP / MP by
    /// difficulty, the bonus sums, the level table's ability grants (regranted on every rebuild:
    /// no "level reached" state exists), the equipment's abilities, then the maxima. The current
    /// HP / MP / focus are not refilled (they are only clamped to the new maxima).
    pub fn rebuild(&mut self) {
        if self.placeholder {
            return;
        }
        let difficulty = self.game_level.row();
        self.level = tables::level_from_exp(self.exp);
        let row = tables::level_row(self.level);
        self.exp_level = row.exp_total;
        self.exp_next = if self.level < tables::LEVEL_MAX { tables::level_row(self.level + 1).exp_total } else { 0 };
        self.base = LevelStats { strength: row.attack, magic: row.magic, defense: row.defense, ap: row.ap };
        // Base row 90 / 100 / 100; on Critical `floor(x * 0.5)`, at least 1 (0x140575D73): the
        // difficulty sheet carries the halved values.
        self.hp_base = difficulty.base_hp.max(1);
        self.mp_base = difficulty.base_mp;
        self.focus_base = tables::RULES.base_max_focus as i32;
        (self.hp_bonus, self.mp_bonus) = self.bonuses.totals(self.game_level);
        // Owned copies = the base copies + the level table's grants up to the level, at most
        // three (`SetLevelAbilities` 0x140E90E00, `Normalize` 0x140E98CF0, `FixEquipped`).
        let granted = level_grant_counts(self.level, self.ability_path);
        for &kind in ETresAbilityKind::ALL {
            let slot = kind.value() as usize;
            if slot < ABILITY_SLOTS && (self.base_owned[slot] > 0 || granted[slot] > 0 || self.abilities.owned(kind) > 0) {
                self.abilities.set_owned(kind, self.base_owned[slot].saturating_add(granted[slot]));
            }
        }
        // Copies of worn equipment (`ClearEquipCounters`, `AddEquipAbilities` 0x140E87590).
        self.abilities.clear_worn();
        let mut unknown = Vec::new();
        for row in self.equipment_rows(&mut unknown) {
            for name in row.abilities {
                if let Some(kind) = ETresAbilityKind::from_game_name(name) {
                    self.abilities.add_worn(kind);
                }
            }
        }
        for name in unknown {
            let note = format!("equipment \"{name}\" is not a row of sheets/port/sora-equipment: it gives nothing");
            if !self.notes.contains(&note) {
                self.notes.push(note);
            }
        }
        self.abilities.set_meal(self.meal.and_then(|meal| meal.ability));
        self.refresh();
        self.hp = self.hp.clamp(0, self.hp_max);
        self.mp = self.mp.clamp(0, self.mp_max);
        self.focus = self.focus.clamp(0, self.focus_max);
    }

    /// Refreshes the cached maxima and the `stats` snapshot from the getters.
    pub fn refresh(&mut self) {
        if self.placeholder {
            return;
        }
        self.hp_max = self.max_hp();
        self.mp_max = self.max_mp();
        self.focus_max = self.max_focus();
        self.stats = Stats { strength: self.strength(), magic: self.magic(), defense: self.defense(), ap: self.abilities.ap_in_use(), ap_max: self.max_ap() };
    }

    /// The equipment rows that count: the CURRENT keyblade's row (`bp+0x140`), then armour and
    /// accessories (`bp+0x148[3..18]`). Names without a row are appended to `unknown`.
    fn equipment_rows(&self, unknown: &mut Vec<String>) -> Vec<&'static tables::EquipmentRow> {
        let mut rows = Vec::new();
        if !self.weapon.is_empty() {
            match tables::weapon_row(&self.weapon, self.weapon_upgrade) {
                Some(row) => rows.push(row),
                None => unknown.push(if self.weapon_upgrade == 0 { self.weapon.clone() } else { format!("{} upgrade {}", self.weapon, self.weapon_upgrade) }),
            }
        }
        for name in self.armour.iter().chain(&self.accessories) {
            match tables::equipment_row(name) {
                Some(row) => rows.push(row),
                None => unknown.push(name.clone()),
            }
        }
        rows
    }

    fn equipment_sum(&self, field: impl Fn(&tables::EquipmentRow) -> i32) -> i32 {
        self.equipment_rows(&mut Vec::new()).into_iter().map(field).sum()
    }

    /// A meal term: used only when > 0, and `ceil(term * 1.25)` with Hearty Meal (POWER_LUNCH
    /// 171) counted. [I] 0x1405818E6 (1.25); sheet `sora-abilities` row POWER_LUNCH.
    fn meal_term(&self, term: impl Fn(&Meal) -> i32) -> i32 {
        match self.meal.as_ref().map(term) {
            Some(value) if value > 0 => {
                if self.abilities.count(ETresAbilityKind::PowerLunch) > 0 {
                    ue_ceil(value as f32 * 1.25)
                } else {
                    value
                }
            }
            _ => 0,
        }
    }

    // ------------------------------------------------------------------ the stat getters (2.7)

    /// Max HP 0x140583990 = base + victory-bonus sum + meal. No level term, no equipment term,
    /// no cap. Sheet row `stat.max_hp`.
    pub fn max_hp(&self) -> i32 {
        self.hp_base + self.hp_bonus + self.meal_term(|m| m.hp)
    }

    /// Max MP 0x140583B40 = base + victory-bonus sum + meal. Sheet row `stat.max_mp`.
    pub fn max_mp(&self) -> i32 {
        self.mp_base + self.mp_bonus + self.meal_term(|m| m.mp)
    }

    /// Sets the PERMANENT copies of an ability and how many of them are equipped, WITHOUT the AP
    /// test: for loadouts built in code, tests and debug keys (the level table's grants still
    /// come on top of the permanent copies).
    pub fn set_ability(&mut self, kind: ETresAbilityKind, owned: u8, equipped: u8) {
        let slot = kind.value() as usize;
        if slot >= ABILITY_SLOTS || self.placeholder {
            return;
        }
        self.base_owned[slot] = owned.min(ability_set::MAX_OWNED);
        self.rebuild();
        self.abilities.set(kind, self.abilities.owned(kind), equipped);
        self.refresh();
    }

    /// Max focus 0x1405838E0 = base (100) + `bp[0x70]`, for which no writer exists for Sora.
    pub fn max_focus(&self) -> i32 {
        self.focus_base
    }

    /// STRENGTH 0x140580E70, in the spec's order of operations (sheet row `stat.attack`):
    /// level value; + boost items; + weapon and equipment; (Hidden Potential floor: NOT
    /// implemented, it needs `EnemyBaseStatus`, which is not in the port's sheets; no Sora
    /// source owns the ability); + meal; + 1 with Berserk Charge while the MP charge runs;
    /// + 5 x buff level. The code-menu options (A0 "default status", B0 x3) are off.
    pub fn strength(&self) -> i32 {
        let mut s = self.base.strength;
        s += self.boosts.strength as i32;
        s += self.equipment_sum(|row| row.attack);
        s += self.meal_term(|m| m.strength);
        if self.abilities.count(ETresAbilityKind::ChargeBerserk) > 0 && self.mp_charging {
            s += 1;
        }
        s + 5 * self.buff_level as i32
    }

    /// MAGIC 0x1405836E0: the same shape without the Berserk and buff terms (sheet row `stat.magic`).
    pub fn magic(&self) -> i32 {
        self.base.magic + self.boosts.magic as i32 + self.equipment_sum(|row| row.magic) + self.meal_term(|m| m.magic)
    }

    /// DEFENSE 0x140581830 (sheet row `stat.defense`): level value + boost + meal + weapon and
    /// equipment + Defender: +2 when HP% <= 25 with one copy, <= 50 with two, <= 75 with three,
    /// always with four. [I] thresholds 0x140581AA8, 0x140581A4D, 0x1405819F2.
    pub fn defense(&self) -> i32 {
        let mut d = self.base.defense + self.boosts.defense as i32 + self.meal_term(|m| m.defense) + self.equipment_sum(|row| row.defense);
        let threshold = match self.abilities.count(ETresAbilityKind::Defender) {
            0 => None,
            1 => Some(25.0),
            2 => Some(50.0),
            3 => Some(75.0),
            _ => Some(f32::INFINITY),
        };
        if threshold.is_some_and(|t| self.hp_percent() <= t) {
            d += 2;
        }
        d
    }

    /// MAX AP 0x1405808B0 = level AP + boost byte (+30 on Critical) + weapon AP + equipment AP
    /// (sheet row `stat.ap_max`).
    pub fn max_ap(&self) -> i32 {
        self.base.ap + self.boosts.ap as i32 + self.equipment_sum(|row| row.ap)
    }

    /// ATTRIBUTE RESIST 0x1405850B0 (sheet row `stat.resist`): base row resist (0 for Sora) +
    /// `100 - ceil(100 * product(1 - r_i / 100))` over the weapon and equipment rows
    /// (multiplicative stacking; the product is taken as `int(product * 1000) * 0.1`).
    pub fn resist(&self, attribute: ETresDamageAttribute) -> i32 {
        let index = attribute.value() as usize;
        let mut product = 1.0f32;
        for row in self.equipment_rows(&mut Vec::new()) {
            if let Some(&r) = row.resist.get(index) {
                product *= 1.0 - r as f32 / 100.0;
            }
        }
        100 - ue_ceil((product * 1000.0) as i32 as f32 * 0.1)
    }

    /// HP percent 0..100 (pawn `vt+0x17C0`), as a float.
    pub fn hp_percent(&self) -> f32 {
        if self.hp_max <= 0 {
            return 0.0;
        }
        self.hp as f32 / self.hp_max as f32 * 100.0
    }

    // ------------------------------------------------------------------ abilities (5.2)

    /// The action-flag bit an ability also needs (`GW+0x95B0`; player overrides 0x1414358B0,
    /// 0x141424130): Double Flight bit 0; Air Slide and Aerial Dodge bit 1; Focus Syphon bit 4;
    /// Superjump and Superslide bit 7.
    fn gate(kind: ETresAbilityKind) -> u8 {
        match kind {
            ETresAbilityKind::Doubleflight => 1 << 0,
            ETresAbilityKind::Airslide | ETresAbilityKind::Airdodge => 1 << 1,
            ETresAbilityKind::FocusAspir => 1 << 4,
            ETresAbilityKind::Superjump | ETresAbilityKind::Superslide => 1 << 7,
            _ => 0,
        }
    }

    /// THE ABILITY QUERY EVERY SYSTEM USES: the player pawn's `AbilityCount(id)` (`vt+0x1500`):
    /// the script gate of the ability, then equipped + worn + meal copies.
    pub fn count(&self, kind: ETresAbilityKind) -> u32 {
        let gate = Self::gate(kind);
        if gate != 0 && self.action_flags & gate == 0 {
            return 0;
        }
        self.abilities.count(kind)
    }

    /// The player pawn's `HasAbility(id)` (`vt+0x14E0`): `count > 0`.
    pub fn has(&self, kind: ETresAbilityKind) -> bool {
        self.count(kind) > 0
    }

    /// AP in use (every equipped copy x its `m_UseAP`).
    pub fn ap_in_use(&self) -> i32 {
        self.abilities.ap_in_use()
    }

    /// The AP budget test of a loadout: AP in use <= max AP. (The camp menu's own test was not
    /// found in the game's code, spec open question 1; this is the data's rule.)
    pub fn ap_within_budget(&self) -> bool {
        self.ap_in_use() <= self.max_ap()
    }

    /// Equips one more owned copy when the AP allow it. `Err` says why not.
    pub fn equip(&mut self, kind: ETresAbilityKind) -> Result<(), &'static str> {
        let (owned, equipped) = (self.abilities.owned(kind), self.abilities.equipped(kind));
        if equipped >= owned {
            return Err("no unequipped copy is owned");
        }
        if self.ap_in_use() + tables::ability_ap(kind) > self.max_ap() {
            return Err("not enough AP");
        }
        self.abilities.set(kind, owned, equipped + 1);
        self.refresh();
        Ok(())
    }

    /// Unequips one copy; false when none is equipped.
    pub fn unequip(&mut self, kind: ETresAbilityKind) -> bool {
        let (owned, equipped) = (self.abilities.owned(kind), self.abilities.equipped(kind));
        if equipped == 0 {
            return false;
        }
        self.abilities.set(kind, owned, equipped - 1);
        self.refresh();
        true
    }

    /// Gives one more permanent copy of an ability (a victory bonus's ability, script
    /// `AbilityAdd(id, bEquip)`): owned, and equipped when asked and the AP allow.
    pub fn grant_ability(&mut self, kind: ETresAbilityKind, equip: bool) {
        let slot = kind.value() as usize;
        if slot >= ABILITY_SLOTS {
            return;
        }
        self.base_owned[slot] = (self.base_owned[slot] + 1).min(ability_set::MAX_OWNED);
        self.rebuild();
        if equip {
            let _ = self.equip(kind);
        }
    }

    /// A victory bonus awarded during play (`TresGetVictoryBonus` -> pawn slot 676 0x1405A06B0;
    /// sheet row `bonus.apply_now`): the counter rises, the totals are recomputed and the
    /// current HP / MP rise by the amount of the new bonus.
    pub fn award_bonus(&mut self, kind: usize) {
        self.bonuses.add(kind);
        let (hp, mp) = BonusCounts::amount(kind, self.game_level);
        self.rebuild();
        self.set_hp(self.hp + hp);
        if mp != 0 {
            self.set_mp(self.mp + mp);
        }
    }

    /// The max combo length (pawn `vt+0x1D00` 0x141433570; sheet rows COMBO_PLUS, AIRCOMBO_PLUS):
    /// 4 + Combo Plus copies on the ground, 4 + Air Combo Plus copies in the air (base Sora).
    pub fn max_combo(&self, airborne: bool) -> u32 {
        4 + self.count(if airborne { ETresAbilityKind::AircomboPlus } else { ETresAbilityKind::ComboPlus })
    }

    /// The attacker side of a hit BY Sora for [`compute_damage`].
    pub fn as_attacker(&self) -> Attacker<'_> {
        Attacker { has_status: true, strength: self.strength(), magic: self.magic(), abilities: Some(&self.abilities), side: Side::Player, state_boost: false }
    }

    /// The defender side of a hit ON Sora for [`compute_damage`]: kind player, the base row's
    /// `m_DamageMin` 0 / `m_DamageMax` 255, biological type Human, no status resists (spec 2.1).
    pub fn as_defender(&self) -> Defender {
        let mut resist = [0; 8];
        for (i, value) in resist.iter_mut().enumerate() {
            *value = ETresDamageAttribute::from_value(i as i32).map_or(0, |attribute| self.resist(attribute));
        }
        Defender {
            has_status: true,
            defense: self.defense(),
            max_hp: self.max_hp(),
            hp_percent: self.hp_percent(),
            kind: DefenderKind::Player,
            damage_min: 0,
            damage_max: 255,
            resist,
            second_resist: 0,
            cut: 0,
            quarter: false,
            bio_type: 0,
            critical_half: self.abilities.count(ETresAbilityKind::CriticalHalf),
        }
    }
}

/// Copies of each ability the level table grants on a path up to `level` (index = enum value).
fn level_grant_counts(level: u32, path: u8) -> [u8; ABILITY_SLOTS] {
    let mut out = [0u8; ABILITY_SLOTS];
    for l in 1..=level.min(tables::LEVEL_MAX) {
        if let Some(kind) = tables::level_ability(l, path) {
            let slot = kind.value() as usize;
            if slot < ABILITY_SLOTS {
                out[slot] += 1;
            }
        }
    }
    out
}
