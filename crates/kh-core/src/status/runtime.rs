//! HP / MP / focus at run time: the pawn's setters and the rules around them.
//! docs/sora-stats-spec.md section 6 (damage application, low HP, heals, prizes, passive gains)
//! and docs/sora-magic-spec.md part C sections 1 - 3 (MP storage, cost, charge), part D S1
//! (focus gauge) and part E I5 (item amounts). Constants are the sheets' rows where one exists
//! (`sora-stats-rules`, `sora-difficulty`); the others are named constants here citing the
//! address the specs read them at (their sheets `sora-magic-mp` / `sora-magic-items` are still
//! being written by the research side and are not compiled against).

use super::math::{round_half_up, ue_ceil, ue_floor};
use super::tables;
use super::{ItemStack, Status};
use crate::enums::{ETresAbilityKind, ETresDamageAttribute};

// ---------------------------------------------------------------- constants
/// MP charge rate per copy of MP Haste / Hastera / Hastega (0x1405AB4D0; [I] 0x1405AB4EB,
/// 0x1405AB50B, 0x1405AB52F; `sora-magic-mp` rows `ability.mp_haste.rate` ...).
pub const MP_HASTE_RATE: [f32; 3] = [0.1, 0.2, 0.3];
/// MP Thrift: `cost -= floor(0.2 * n * cost + 0.9)` (0x1405A4B90; rows `ability.mp_save.coeff` / `.bias`).
pub const MP_SAVE_COEFF: f32 = 0.2;
pub const MP_SAVE_BIAS: f32 = 0.9;
/// The `Mp` column value that means "all remaining MP" (Cure).
pub const MP_COST_ALL: u8 = 255;
/// `MyFocusPointConvertRate`: seconds of aiming one focus point buys (magic spec S1.3).
pub const FOCUS_CONVERT_RATE: f32 = 0.05;
/// Item amounts (magic spec I5; sheet `sora-magic-items` rows `item.potion.hp` ...).
pub const POTION_HP: f32 = 80.0;
pub const HI_POTION_HP: f32 = 160.0;
pub const ETHER_MP: f32 = 100.0;
pub const ETHER_CHARGE_FRACTION: f32 = 0.5;
pub const REFOCUSER_FRACTION: f32 = 0.4;
pub const ITEM_UP_PER_COPY: f32 = 0.5;
pub const PANACEA_IMMUNITY: f32 = 60.0;
/// Walk regeneration (0x1405AF5B0): the delay, the cap of the timer, the grace after stopping.
pub const WALK_REGEN_DELAY: f32 = 5.0;
pub const WALK_REGEN_CAP: f32 = 10.0;
pub const WALK_REGEN_GRACE: f32 = 0.25;

/// Walk-regeneration timers (pawn+0x838..+0x844).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct WalkRegen {
    pub timer: f32,
    pub grace: f32,
    pub hp_acc: f32,
    pub mp_acc: f32,
}

/// Which rule kept a lethal hit from killing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Survived {
    /// The pawn's HP-limit flag or the hit's kill rule (`info+0x66`).
    LeaveOne,
    /// Second Chance (LAST_LEAVE 44): HP was above 1 before the hit.
    SecondChance,
    /// Withstand Combo (COMBO_LEAVE 45): the 0.6 s timer ran or he was in the damage state.
    WithstandCombo,
    /// The pawn is invincible / in a cinematic (`vt+0xA48`).
    Invincible,
}

/// What `ApplyHpDamage` needs to know about a hit on Sora besides the amount.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitOnSora {
    /// `bForce`: the survival rules of step 2 are skipped and the Withstand Combo timer is not set.
    pub forced: bool,
    /// The hit's kill rule (`info+0x66`) says this victim keeps 1 HP (resolved by the combat side).
    pub leave_one: bool,
    /// `info.m_DamageAttribute` (the attribute syphons read it).
    pub attribute: ETresDamageAttribute,
    /// `info.m_AttackSrcPower` (the attribute syphons multiply by it).
    pub src_power: f32,
    /// `info+0x65 != 0`: percent damage (the syphons then use 1 for the power).
    pub percent: bool,
}

impl Default for HitOnSora {
    fn default() -> HitOnSora {
        HitOnSora { forced: false, leave_one: false, attribute: ETresDamageAttribute::Physical, src_power: 0.0, percent: false }
    }
}

/// What a hit did to him.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageOutcome {
    /// False: nothing happened (no damage, or he was already at 0).
    pub applied: bool,
    pub hp_before: i32,
    pub hp_after: i32,
    /// HP really lost.
    pub lost: i32,
    pub survived: Option<Survived>,
    /// HP reached 0: the caller starts the die state (`TresPlayerState_Die`, combat strand).
    pub dead: bool,
    /// MP gained through Damage Syphon and the attribute syphons.
    pub mp_gained: i32,
}

/// The cost of a command's `Mp` column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MpCost {
    /// 255: all remaining MP (Cure); never blocked by MP Safety.
    All,
    Points(i32),
}

impl MpCost {
    /// From a `TresCommandKindData.Mp` byte.
    pub fn from_row(mp: u8) -> MpCost {
        if mp == MP_COST_ALL {
            MpCost::All
        } else {
            MpCost::Points(mp as i32)
        }
    }
}

/// What spending MP did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct MpUse {
    /// MP really spent.
    pub used: i32,
    /// The MP charge began with this cast.
    pub charge_started: bool,
    /// HP returned by Wizard's Ruse.
    pub healed: i32,
}

/// `ApplyPrize` kinds (0x1405AB950).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PrizeKind {
    HpSmall = 0,
    HpLarge = 1,
    MpSmall = 2,
    MpLarge = 3,
    Munny1 = 4,
    Munny5 = 5,
    Munny10 = 6,
    FocusSmall = 7,
    FocusLarge = 8,
}

impl PrizeKind {
    pub const ALL: [PrizeKind; 9] = [PrizeKind::HpSmall, PrizeKind::HpLarge, PrizeKind::MpSmall, PrizeKind::MpLarge, PrizeKind::Munny1, PrizeKind::Munny5, PrizeKind::Munny10, PrizeKind::FocusSmall, PrizeKind::FocusLarge];

    pub fn from_value(value: u8) -> Option<PrizeKind> {
        PrizeKind::ALL.get(value as usize).copied()
    }
}

/// The battle items by the game's item id (magic spec I5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum ItemKind {
    Potion = 0x101,
    HiPotion = 0x102,
    MegaPotion = 0x103,
    Ether = 0x104,
    MegaEther = 0x105,
    Elixir = 0x106,
    Megalixir = 0x107,
    Refocuser = 0x108,
    HiRefocuser = 0x109,
    Panacea = 0x10A,
    HiEther = 0x10B,
    Tent = 0x201,
}

impl ItemKind {
    pub const ALL: [ItemKind; 12] = [ItemKind::Potion, ItemKind::HiPotion, ItemKind::MegaPotion, ItemKind::Ether, ItemKind::MegaEther, ItemKind::Elixir, ItemKind::Megalixir, ItemKind::Refocuser, ItemKind::HiRefocuser, ItemKind::Panacea, ItemKind::HiEther, ItemKind::Tent];

    /// The pouch name. `ITEM_POTION`, `ITEM_HIGHPOTION`, `ITEM_ETHER` are the loadout sheet's;
    /// the others follow `ETresDropItemID` in upper case (`ITEM_FocusSupply` = Refocuser,
    /// `ITEM_AllCure` = Panacea); `ITEM_MEGAELIXIR` and `ITEM_TENT` are this port's spelling
    /// (no sheet names them yet).
    pub const fn name(self) -> &'static str {
        match self {
            ItemKind::Potion => "ITEM_POTION",
            ItemKind::HiPotion => "ITEM_HIGHPOTION",
            ItemKind::MegaPotion => "ITEM_MEGAPOTION",
            ItemKind::Ether => "ITEM_ETHER",
            ItemKind::MegaEther => "ITEM_MEGAETHER",
            ItemKind::Elixir => "ITEM_ELIXIR",
            ItemKind::Megalixir => "ITEM_MEGAELIXIR",
            ItemKind::Refocuser => "ITEM_FOCUSSUPPLY",
            ItemKind::HiRefocuser => "ITEM_HIGHFOCUSSUPPLY",
            ItemKind::Panacea => "ITEM_ALLCURE",
            ItemKind::HiEther => "ITEM_HIGHETHER",
            ItemKind::Tent => "ITEM_TENT",
        }
    }

    /// By pouch name (any case) or item id ("0x101", "257").
    pub fn from_name(name: &str) -> Option<ItemKind> {
        let name = name.trim();
        let id = name.strip_prefix("0x").or_else(|| name.strip_prefix("0X")).map_or_else(|| name.parse::<u16>().ok(), |hex| u16::from_str_radix(hex, 16).ok());
        ItemKind::ALL.into_iter().find(|kind| kind.name().eq_ignore_ascii_case(name) || id == Some(*kind as u16))
    }
}

/// What an item did (differences of the gauges; `charge_ended` when it ended the MP charge).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ItemOutcome {
    pub hp: i32,
    pub mp: i32,
    pub focus: i32,
    /// Seconds taken off the MP charge.
    pub charge_seconds: f32,
    pub charge_ended: bool,
}

/// The focus cost of a shotlock is the time spent aiming (magic spec S1.3). The shotlock
/// implementer drives this: `let mut aim = status.focus_aim_begin();` at aim start,
/// `aim.tick(dt, time_dilation)` every tick, `status.focus_aim_end(&aim)` at release.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FocusAim {
    /// Focus at aim start.
    pub focus_at_start: i32,
    /// `aimTime`: seconds of aiming left (focus x 0.05 at the start; 20 points burn per real second).
    pub time: f32,
}

impl FocusAim {
    /// `aimTime -= dt / actorTimeDilation`, floor 0.
    pub fn tick(&mut self, dt: f32, time_dilation: f32) {
        self.time = (self.time - dt / if time_dilation > 0.0 { time_dilation } else { 1.0 }).max(0.0);
    }

    /// The focus the gauge would have if released now: `int(aimTime / 0.05)`.
    pub fn focus(&self) -> i32 {
        (self.time / FOCUS_CONVERT_RATE) as i32
    }

    /// No further lock can be added once the time is used up.
    pub fn exhausted(&self) -> bool {
        self.time <= 0.0
    }
}

impl Status {
    // ================================================================ HP
    /// `SetHitPoint` 0x1405B9700 (sheet row `hp.set`): `HP = clamp(v, 0, max HP)`.
    pub fn set_hp(&mut self, value: i32) {
        self.hp = value.clamp(0, self.max_hp().max(0));
    }

    /// `IsPinch` 0x1405AD600 (sheet row `hp.pinch`): `HP <= maxHP * 25 / 100`, integer arithmetic.
    pub fn is_pinch(&self) -> bool {
        !self.placeholder && self.hp <= self.max_hp() * 25 / 100
    }

    /// DAMAGE TO SORA: `ApplyHpDamage` 0x1405A2A00 (stats spec 6.1; sheet rows `hp.damage_apply`,
    /// `hp.combo_leave_timer`, abilities LAST_LEAVE, COMBO_LEAVE). `damage` is the amount the
    /// damage function returned ([`super::compute_damage`]).
    ///
    /// 1. nothing when `damage <= 0` (unless forced) or HP < 1; `new = HP - floor(damage)`;
    /// 2. not forced and lethal: 1 HP is left by the pawn's HP-limit flag or the hit's kill rule;
    /// 3. SECOND CHANCE: has the ability, HP > 1 before the hit, lethal -> 1 (so it needs HP >= 2);
    /// 4. WITHSTAND COMBO: has the ability, lethal, and the timer runs or he is in the damage
    ///    state (`in_damage_state`, the combat strand's hook) -> 1, also from HP 1;
    /// 5. the timer is set to 0.6 s by every non-forced hit;
    /// 6. an invincible pawn survives at 1;
    /// 7. `SetHitPoint`, then the damaged notify (Damage Syphon, attribute syphons).
    ///
    /// Death is not handled here: when `dead` the caller starts the die state. Neither rule
    /// depends on the difficulty.
    pub fn apply_damage_to_sora(&mut self, damage: f32, hit: &HitOnSora) -> DamageOutcome {
        let before = self.hp;
        let mut outcome = DamageOutcome { applied: false, hp_before: before, hp_after: before, lost: 0, survived: None, dead: false, mp_gained: 0 };
        if self.placeholder || (!(damage > 0.0) && !hit.forced) || before < 1 {
            return outcome;
        }
        let mut new = before - ue_floor(damage);
        if !hit.forced && new < 1 && (self.hp_limit_one || hit.leave_one) {
            new = 1;
            outcome.survived = Some(Survived::LeaveOne);
        }
        // [I] 0x1405A2B2C
        if new < 1 && self.has(ETresAbilityKind::LastLeave) && before > 1 {
            new = 1;
            outcome.survived = Some(Survived::SecondChance);
        }
        // [I] 0x1405A2B4C, 0x1405A2B6F
        if new < 1 && self.has(ETresAbilityKind::ComboLeave) && (self.combo_leave_timer > 0.0 || self.in_damage_state) {
            new = 1;
            outcome.survived = Some(Survived::WithstandCombo);
        }
        // [I] 0x1405A2BA2
        if !hit.forced {
            self.combo_leave_timer = tables::RULES.hp_combo_leave_timer;
        }
        if new < 1 && self.invincible {
            new = 1;
            outcome.survived = Some(Survived::Invincible);
        }
        self.set_hp(new);
        outcome.applied = true;
        outcome.hp_after = self.hp;
        outcome.lost = before - self.hp;
        outcome.dead = self.hp == 0;
        outcome.mp_gained = self.on_damaged(outcome.lost, hit);
        self.refresh();
        outcome
    }

    /// Percent damage 0x1405A3970: a ratio of max HP (`of_current` false, kind 1) or of the
    /// current HP (kind 2); Damage Control applies (the same rule as in the damage function),
    /// then `ApplyHpDamage`. The rounding inside 0x1405A3970 was not read: the product is
    /// passed on as a float (floored by `ApplyHpDamage`).
    pub fn apply_percent_damage(&mut self, ratio: f32, of_current: bool, hit: &HitOnSora) -> DamageOutcome {
        let mut amount = ratio * if of_current { self.hp } else { self.max_hp() } as f32;
        let copies = self.abilities.count(ETresAbilityKind::CriticalHalf);
        if copies > 0 && self.hp_percent() <= 25.0 {
            amount = ue_ceil(amount * if copies > 1 { 0.333_333_34 } else { 0.5 }) as f32;
        }
        self.apply_damage_to_sora(amount, &HitOnSora { percent: true, ..*hit })
    }

    /// The damaged notify 0x1405B58D0 (stats spec 6.4; sheet rows DAMAGE_ASPIR, FIRE_ASPIR ..
    /// DARK_ASPIR): not while charging or dead. Damage Syphon: `mp = ceil(lost / maxHP * 100 * n)`;
    /// attribute 1..6 -> ids 84..89: `mp += ceil(p * 3 * n)`, p = the hit's source power (must
    /// be > 0), 1 for percent damage; `Recover(0, mp, 0, 0)`. Returns the MP gained.
    fn on_damaged(&mut self, lost: i32, hit: &HitOnSora) -> i32 {
        if self.mp_charging || self.hp < 1 {
            return 0;
        }
        let mut mp = 0;
        let n = self.count(ETresAbilityKind::DamageAspir);
        if n > 0 && self.max_hp() > 0 {
            mp = ue_ceil(lost as f32 / self.max_hp() as f32 * 100.0 * n as f32);
        }
        let syphon = match hit.attribute {
            ETresDamageAttribute::Fire => Some(ETresAbilityKind::FireAspir),
            ETresDamageAttribute::Blizzard => Some(ETresAbilityKind::BlizzardAspir),
            ETresDamageAttribute::Thunder => Some(ETresAbilityKind::ThunderAspir),
            ETresDamageAttribute::Water => Some(ETresAbilityKind::WaterAspir),
            ETresDamageAttribute::Aero => Some(ETresAbilityKind::AeroAspir),
            ETresDamageAttribute::Dark => Some(ETresAbilityKind::DarkAspir),
            _ => None,
        };
        if let Some(kind) = syphon {
            let n = self.count(kind);
            let p = if hit.percent { 1.0 } else { hit.src_power };
            if n > 0 && p > 0.0 {
                mp += ue_ceil(p * 3.0 * n as f32);
            }
        }
        if mp <= 0 {
            return 0;
        }
        let before = self.mp;
        self.recover(0, mp, 0, 0);
        self.mp - before
    }

    /// `Recover(hp, mp, focus, kind)` 0x1405B57D0 (stats spec 6.3): `HP += hp`; MP: with kind 1
    /// while charging the charge timer is reduced by `mp` seconds, otherwise `MP += mp` (which
    /// the MP setter ignores while charging); `focus += focus`. The player override drops the
    /// MP / focus part while the action flags have MP / focus disabled (bits 2 / 4 of
    /// GW+0x95B0). The recover effect of `kind` is the VFX strand's.
    pub fn recover(&mut self, hp: i32, mp: i32, focus: i32, kind: u8) {
        if self.placeholder {
            return;
        }
        if hp != 0 {
            self.set_hp(self.hp + hp);
        }
        if mp != 0 && self.action_flags & (1 << 2) != 0 {
            if kind == 1 && self.mp_charging {
                self.mp_charge_timer = (self.mp_charge_timer - mp as f32).max(0.0);
            } else {
                self.set_mp(self.mp + mp);
            }
        }
        if focus != 0 && self.action_flags & (1 << 4) != 0 {
            self.set_focus(self.focus + focus);
        }
        self.refresh();
    }

    /// `FullRecover` 0x1405B5700: HP, MP, focus = max; the MP charge flag and timer cleared.
    pub fn full_recover(&mut self) {
        if self.placeholder {
            return;
        }
        self.mp_charging = false;
        self.mp_charge_timer = 0.0;
        self.focus_empty = false;
        self.hp = self.max_hp();
        self.mp = self.max_mp();
        self.focus = self.max_focus();
        self.refresh();
    }

    // ================================================================ MP
    /// `SetMagicPoint` 0x1405B9800: ignored while charging; `MP = v < 0 ? 0 : min(v, max)`; a
    /// negative value (with the old MP >= 0) starts the charge.
    pub fn set_mp(&mut self, value: i32) {
        if self.mp_charging {
            return;
        }
        let old = self.mp;
        self.mp = if value < 0 { 0 } else { value.min(self.max_mp()) };
        if value < 0 && old >= 0 {
            self.set_mp_charge(true, 1.0);
        }
    }

    /// `GetMpChargeTime` 0x1405AB250: `MyMpChargeTime` 50 s, +20 s on Critical (the difficulty
    /// sheet's `mp_charge_time`; `sora-stats-rules` row `mp.charge_time`).
    pub fn mp_charge_time(&self) -> f32 {
        self.game_level.row().mp_charge_time
    }

    /// `GetMpChargeRate` 0x1405AB4D0: `1 + 0.1 a + 0.2 b + 0.3 c` for the counts of MP Haste /
    /// Hastera / Hastega; all three add and each stacks per copy. (The x0.5 of code-menu option
    /// A4 is off: the port has no code menu.)
    pub fn mp_charge_rate(&self) -> f32 {
        1.0 + MP_HASTE_RATE[0] * self.count(ETresAbilityKind::MpHaste) as f32 + MP_HASTE_RATE[1] * self.count(ETresAbilityKind::MpHastera) as f32 + MP_HASTE_RATE[2] * self.count(ETresAbilityKind::MpHastega) as f32
    }

    /// `SetMpCharge(on, ratio)` 0x1405B9C60. On: the flag and `timer = GetMpChargeTime()` (with
    /// a charge time of 0 the off path runs at once). Off: flag and timer cleared and
    /// `MP = ratio >= 1 ? max : trunc(max * clamp(ratio, 0, 1))`.
    pub fn set_mp_charge(&mut self, on: bool, ratio: f32) {
        if on == self.mp_charging {
            return;
        }
        if on && self.mp_charge_time() > 0.0 {
            self.mp_charging = true;
            self.mp_charge_timer = self.mp_charge_time();
            return;
        }
        self.mp_charging = false;
        self.mp_charge_timer = 0.0;
        self.mp = if ratio >= 1.0 { self.max_mp() } else { (self.max_mp() as f32 * ratio.clamp(0.0, 1.0)) as i32 };
    }

    /// `ForceMpZero` 0x1405B9D30: MP = 0 and the charge starts.
    pub fn force_mp_zero(&mut self) {
        self.set_mp(-1);
    }

    /// Player slot 562 0x141446160: the remaining charge time is halved.
    pub fn halve_mp_charge(&mut self) {
        self.mp_charge_timer -= 0.5 * self.mp_charge_timer;
    }

    /// The HUD's charge gauge (`vt+0x17F8`): `timer / T * maxMP`, MP units still to refill; 0
    /// when not charging.
    pub fn mp_charge_gauge(&self) -> f32 {
        let total = self.mp_charge_time();
        if !self.mp_charging || total <= 0.0 {
            return 0.0;
        }
        self.mp_charge_timer / total * self.max_mp() as f32
    }

    /// `TickMpCharge` 0x1405A8080: while charging and the timer is above 0 it loses
    /// `rate * dt` (not while `paused`: the link-summon states, magic spec C 3); at 0 or below
    /// the charge ends with full MP.
    pub fn tick_mp_charge(&mut self, dt: f32, paused: bool) {
        if !self.mp_charging {
            return;
        }
        if self.mp_charge_timer > 0.0 && !paused {
            self.mp_charge_timer -= self.mp_charge_rate() * dt;
        }
        if self.mp_charge_timer <= 0.0 {
            self.set_mp_charge(false, 1.0);
        }
    }

    /// `EffCost` 0x1405A4B90 (sheet rows MP_SAVE, MAGIC_COMBO_SAVE): for a cost > 0, MP Thrift
    /// `cost = max(cost - floor(0.2 * n * cost + 0.9), 1)`, then Magic Combo Thrift
    /// `cost = max(cost - combo, 1)` (`combo` = the pawn's running combo counter, pawn+0xE18).
    pub fn effective_mp_cost(&self, cost: i32, combo: i32) -> i32 {
        let mut cost = cost;
        if cost > 0 {
            // (host choice, IMPL_ER61) the host's factor on the row cost; 1 = KH3 exactly
            if self.mp_cost_scale > 0.0 && self.mp_cost_scale != 1.0 {
                cost = ((cost as f32 * self.mp_cost_scale).round() as i32).max(1);
            }
            let n = self.count(ETresAbilityKind::MpSave);
            if n > 0 {
                cost = (cost - ue_floor(MP_SAVE_COEFF * n as f32 * cost as f32 + MP_SAVE_BIAS)).max(1);
            }
            if self.has(ETresAbilityKind::MagicComboSave) {
                cost = (cost - combo).max(1);
            }
        }
        cost
    }

    /// The MP part of the command availability test 0x14142F330 (sheet rows MP_SAFETY,
    /// MP_LEAVE): MP must be usable (action flag bit 2); not while charging; with MP Safety a
    /// spell whose effective cost exceeds the MP is refused, unless Extra Cast is counted and
    /// MP > 0; Cure (cost 255) is never blocked by MP Safety. Without MP Safety a spell can be
    /// cast with any MP, even 0 (it then triggers the charge). The state conditions of that
    /// function are the magic strand's.
    pub fn can_cast(&self, cost: MpCost, combo: i32) -> bool {
        if self.placeholder || self.action_flags & (1 << 2) == 0 || self.mp_charging {
            return false;
        }
        if let MpCost::Points(points) = cost {
            if self.has(ETresAbilityKind::MpSafety) && self.mp < self.effective_mp_cost(points, combo) && !(self.has(ETresAbilityKind::MpLeave) && self.mp > 0) {
                return false;
            }
        }
        true
    }

    /// `UseMagicPoint(cost)` 0x1405BCDC0 with the cost already effective (`cost < 0` = all):
    /// nothing while charging or when the cost is 0. All MP: MP 0 and the charge starts.
    /// Overdraw (`MP - cost < 0`): with Extra Cast and MP > 0, MP goes to 0 WITHOUT the charge;
    /// otherwise MP 0 and the charge starts. Else `MP -= cost`: LANDING EXACTLY ON 0 DOES NOT
    /// START THE CHARGE; the next cast does. Then Wizard's Ruse (WIZZARD_STAR 38): with
    /// probability (n + 1) / 8 the MP spent returns as HP (the port's own random stream).
    pub fn use_mp(&mut self, cost: i32) -> MpUse {
        if self.placeholder || self.mp_charging || cost == 0 {
            return MpUse::default();
        }
        let current = self.mp;
        let used;
        if cost < 0 {
            self.set_mp(-1);
            used = current;
        } else if current - cost < 0 {
            if self.has(ETresAbilityKind::MpLeave) && current > 0 {
                self.set_mp(0);
            } else {
                self.set_mp(-1);
            }
            used = current;
        } else {
            self.set_mp(current - cost);
            used = cost;
        }
        let mut healed = 0;
        let n = self.count(ETresAbilityKind::WizzardStar);
        if used > 0 && n > 0 && self.hp > 0 {
            let roll = self.next_random() & 7;
            if roll < n + 1 {
                let before = self.hp;
                self.set_hp(self.hp + used);
                healed = self.hp - before;
            }
        }
        self.refresh();
        MpUse { used, charge_started: self.mp_charging, healed }
    }

    /// The MP of a cast, as the controller helper 0x14140D560 spends it at the shot: a row cost
    /// of 255 spends everything, any other the effective cost.
    pub fn spend_command_mp(&mut self, cost: MpCost, combo: i32) -> MpUse {
        match cost {
            MpCost::All => self.use_mp(-1),
            MpCost::Points(points) => {
                let effective = self.effective_mp_cost(points, combo);
                self.use_mp(effective)
            }
        }
    }

    /// The port's deterministic random stream (xorshift32).
    fn next_random(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x >> 8
    }

    // ================================================================ focus
    /// `SetFocusPoint` 0x1405B94F0: `clamp(v, 0, max)`; reaching 0 from >= 1 sets the "focus
    /// empty" flag (player pawns), a full gauge clears it.
    pub fn set_focus(&mut self, value: i32) {
        let old = self.focus;
        self.focus = value.clamp(0, self.max_focus().max(0));
        if old >= 1 && value <= 0 {
            self.focus_empty = true;
        }
        if self.focus_empty && self.focus >= self.max_focus() {
            self.focus_empty = false;
        }
    }

    /// `AddFocus(n)` (controller slot 817 0x1413E8CA0): ignored while the aim mode is active;
    /// otherwise `Set(Get + n)` for n > 0.
    pub fn add_focus(&mut self, n: i32, aiming: bool) {
        if !aiming && n > 0 {
            self.set_focus(self.focus + n);
        }
    }

    /// Aim start: `aimTime = focus * MyFocusPointConvertRate`.
    pub fn focus_aim_begin(&self) -> FocusAim {
        FocusAim { focus_at_start: self.focus, time: self.focus as f32 * FOCUS_CONVERT_RATE }
    }

    /// Release: `focus = int(aimTime / 0.05)` (the shotlock kinds 36 and 37 leave it unchanged:
    /// the caller then does not call this).
    pub fn focus_aim_end(&mut self, aim: &FocusAim) {
        self.set_focus(aim.focus());
    }

    // ================================================================ prizes and items
    /// `ApplyPrize` 0x1405AB950 (stats spec 6.3; sheet row `prize.values`): HP +1 / +5; MP +3 /
    /// +5, or 2 s / 6 s off the charge while charging (not clamped: the next tick ends the
    /// charge); munny 1 / 5 / 10; focus +3 / +5. Returns the munny gained.
    pub fn apply_prize(&mut self, kind: PrizeKind) -> i32 {
        if self.placeholder {
            return 0;
        }
        let mut munny = 0;
        match kind {
            PrizeKind::HpSmall => self.set_hp(self.hp + 1),
            PrizeKind::HpLarge => self.set_hp(self.hp + 5),
            PrizeKind::MpSmall | PrizeKind::MpLarge => {
                let (seconds, points) = if kind == PrizeKind::MpSmall { (2.0, 3) } else { (6.0, 5) };
                if self.mp_charging {
                    self.mp_charge_timer -= seconds;
                } else {
                    self.set_mp(self.mp + points);
                }
            }
            PrizeKind::Munny1 => munny = 1,
            PrizeKind::Munny5 => munny = 5,
            PrizeKind::Munny10 => munny = 10,
            PrizeKind::FocusSmall => self.set_focus(self.focus + 3),
            PrizeKind::FocusLarge => self.set_focus(self.focus + 5),
        }
        self.munny += munny;
        self.refresh();
        munny
    }

    /// The item multiplier (magic spec I5; sheet row ITEM_UP): `1 + 0.5 * count(ITEM_UP)` of the
    /// user. (x0.5 for a target in the DIE state concerns KO'd friends; nothing is applied to a
    /// dead Sora.)
    pub fn item_multiplier(&self) -> f32 {
        1.0 + ITEM_UP_PER_COPY * self.count(ETresAbilityKind::ItemUp) as f32
    }

    /// `ItemEffect` 0x1405BE820 on Sora, without touching the pouch (magic spec I5 table).
    /// Amounts are `round half up` of `mult * amount`.
    pub fn item_effect(&mut self, item: ItemKind) -> ItemOutcome {
        let mut outcome = ItemOutcome::default();
        if self.placeholder || self.hp <= 0 {
            return outcome;
        }
        let mult = self.item_multiplier();
        // (host bookkeeping) one more landing of this item's effect
        if let Some(k) = ItemKind::ALL.iter().position(|i| *i == item) {
            self.item_effects[k] = self.item_effects[k].wrapping_add(1);
        }
        let (hp, mp, focus, charging) = (self.hp, self.mp, self.focus, self.mp_charging);
        let total = self.mp_charge_time();
        let mut charge = |status: &mut Status, seconds: f32| {
            status.mp_charge_timer -= seconds;
            outcome.charge_seconds = seconds;
            if status.mp_charge_timer <= 0.0 {
                status.set_mp_charge(false, 1.0);
            }
        };
        match item {
            ItemKind::Potion => self.set_hp(hp + round_half_up(mult * POTION_HP)),
            ItemKind::HiPotion | ItemKind::MegaPotion => self.set_hp(hp + round_half_up(mult * HI_POTION_HP)),
            ItemKind::Ether => {
                if charging {
                    charge(self, total * ETHER_CHARGE_FRACTION * mult);
                } else {
                    self.set_mp(mp + round_half_up(mult * ETHER_MP));
                }
            }
            ItemKind::HiEther | ItemKind::MegaEther => {
                if charging {
                    charge(self, total * mult);
                } else {
                    self.set_mp(mp + round_half_up(self.max_mp() as f32 * mult));
                }
            }
            ItemKind::Elixir | ItemKind::Megalixir | ItemKind::Tent => {
                self.set_hp(hp + self.max_hp());
                if charging {
                    self.set_mp_charge(false, 1.0);
                } else {
                    self.set_mp(mp + self.max_mp());
                }
                self.focus_empty = false;
                self.set_focus(focus + self.max_focus());
            }
            ItemKind::Refocuser => {
                self.focus_empty = false;
                self.set_focus(focus + round_half_up(self.max_focus() as f32 * REFOCUSER_FRACTION * mult));
            }
            ItemKind::HiRefocuser => self.set_focus(focus + round_half_up(self.max_focus() as f32 * mult)),
            ItemKind::Panacea => {
                if self.status_immunity <= 0.0 {
                    self.status_immunity = PANACEA_IMMUNITY;
                }
            }
        }
        outcome.hp = self.hp - hp;
        outcome.mp = self.mp - mp;
        outcome.focus = self.focus - focus;
        outcome.charge_ended = charging && !self.mp_charging;
        self.refresh();
        outcome
    }

    /// How many of an item the pouch holds.
    pub fn item_count(&self, item: ItemKind) -> u32 {
        self.items.iter().filter(|stack| ItemKind::from_name(&stack.item) == Some(item)).map(|stack| stack.count).sum()
    }

    /// Uses one item from the pouch on Sora (`UseItem` 0x1405BCBE0 -> `ItemEffect`): needs a
    /// count >= 1. The 0.5 s delay of the effect and the state are the magic strand's.
    pub fn use_item(&mut self, item: ItemKind) -> Result<ItemOutcome, &'static str> {
        let Some(index) = self.items.iter().position(|stack| stack.count > 0 && ItemKind::from_name(&stack.item) == Some(item)) else {
            return Err("the pouch holds none of that item");
        };
        if self.hp <= 0 {
            return Err("he is dead");
        }
        self.items[index].count -= 1;
        if self.items[index].count == 0 {
            self.items.remove(index);
        }
        Ok(self.item_effect(item))
    }

    /// Puts `count` of an item into the pouch (a test and host helper; stock limit 99).
    pub fn add_item(&mut self, item: ItemKind, count: u32) {
        match self.items.iter_mut().find(|stack| ItemKind::from_name(&stack.item) == Some(item)) {
            Some(stack) => stack.count = (stack.count + count).min(99),
            None => self.items.push(ItemStack { item: item.name().to_string(), count: count.min(99) }),
        }
    }

    // ================================================================ status protection
    /// `ResistStatus` 0x140589610 (sheet rows BURN_GUARD .. STUN_GUARD): bad status 17 burn,
    /// 15 cloud, 13 sneeze, 7 freeze, 16 discharge, 12 stun are refused with the matching
    /// ability; Panacea's immunity refuses every one (magic spec I5). The base row has no
    /// resist bits for Sora.
    pub fn resists_bad_status(&self, bad_status: u8) -> bool {
        if self.status_immunity > 0.0 {
            return true;
        }
        let kind = match bad_status {
            17 => ETresAbilityKind::BurnGuard,
            15 => ETresAbilityKind::CloudGuard,
            13 => ETresAbilityKind::SneezeGuard,
            7 => ETresAbilityKind::FreezeGuard,
            16 => ETresAbilityKind::DischargeGuard,
            12 => ETresAbilityKind::StunGuard,
            _ => return false,
        };
        self.abilities.count(kind) > 0
    }

    // ================================================================ the per-step tick
    /// The status part of the pawn tick, once per step: walk regeneration and the Withstand
    /// Combo timer (0x1405AF5B0), then the MP charge (0x1405A8080), Panacea's immunity, and the
    /// `stats` snapshot. `speed_squared` = the movement velocity squared (cm/s)^2.
    pub fn tick(&mut self, dt: f32, speed_squared: f32) {
        if self.placeholder {
            return;
        }
        self.tick_walk_regen(dt, speed_squared);
        // The Withstand Combo timer counts down only while the ability is counted, else it is
        // held at 0 (stats spec 6.1 step 5).
        if self.has(ETresAbilityKind::ComboLeave) {
            self.combo_leave_timer = (self.combo_leave_timer - dt).max(0.0);
        } else {
            self.combo_leave_timer = 0.0;
        }
        self.tick_mp_charge(dt, false);
        self.status_immunity = (self.status_immunity - dt).max(0.0);
        self.refresh();
    }

    /// Walk regeneration 0x1405AF5B0 (stats spec 6.4; sheet rows WALK_HEALING, WALK_REGENE):
    /// while the speed squared is above 1 (0.25 s grace after stopping) a timer runs up to 10 s;
    /// from 5 s on HP Walker gives +1 HP every `1 / (n + 1)` s and MP Walker +1 MP every
    /// `1 / (n + 1)` s (not while charging). Standing still resets the timer.
    fn tick_walk_regen(&mut self, dt: f32, speed_squared: f32) {
        if speed_squared > 1.0 {
            self.walk.grace = WALK_REGEN_GRACE;
        } else {
            self.walk.grace = (self.walk.grace - dt).max(0.0);
        }
        if speed_squared > 1.0 || self.walk.grace > 0.0 {
            self.walk.timer = (self.walk.timer + dt).min(WALK_REGEN_CAP);
        } else {
            self.walk = WalkRegen::default();
        }
        if self.walk.timer < WALK_REGEN_DELAY || self.hp < 1 {
            return;
        }
        let hp_copies = self.count(ETresAbilityKind::WalkHealing);
        if hp_copies > 0 {
            self.walk.hp_acc += dt;
            let period = 1.0 / (hp_copies + 1) as f32;
            while self.walk.hp_acc >= period {
                self.walk.hp_acc -= period;
                self.set_hp(self.hp + 1);
            }
        }
        let mp_copies = self.count(ETresAbilityKind::WalkRegene);
        if mp_copies > 0 && !self.mp_charging {
            self.walk.mp_acc += dt;
            let period = 1.0 / (mp_copies + 1) as f32;
            while self.walk.mp_acc >= period {
                self.walk.mp_acc -= period;
                self.set_mp(self.mp + 1);
            }
        }
    }
}
