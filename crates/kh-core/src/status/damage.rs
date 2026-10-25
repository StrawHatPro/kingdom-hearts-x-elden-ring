//! THE DAMAGE FUNCTION: `CalcDamage` 0x140632240, written from its disassembly
//! (`build/sora_tmp/stats/dis_140632240.txt`), which docs/sora-stats-spec.md section 7 and
//! docs/sora-combat-spec.md 8.3 both describe. Every step is done in `f32` with the engine's
//! `ceil` / `floor` forms, in the instruction order, so the results are the game's.
//!
//! FOR THE COMBAT AND MAGIC IMPLEMENTERS: THIS IS THE ONE DAMAGE FUNCTION OF THE PORT. Replace any
//! local placeholder by
//!
//! ```text
//! use crate::status::{compute_damage, DamageAttack, DamageContext, Defender};
//! let attacker = cx.status.as_attacker();                        // Sora hits
//! let defender = Defender::of_target(cx.host.targets.defense(id).as_ref());   // host data, or "no status object"
//! let result = compute_damage(&attacker, &defender, &attack, &DamageContext::of(&cx.status));
//! record.damage = result.amount;                                 // HitRecord::damage, already final
//! ```
//!
//! and, for a hit ON Sora, `compute_damage(&Attacker::enemy(attack_stat), &status.as_defender(),
//! &attack, &DamageContext::of(&status))` followed by `status.apply_damage_to_sora(amount, &hit)`.
//! `DamageAttack` is the damage-relevant part of an attack row + `FTresDamageInfo`; fill it from
//! your generated attack table.
//!
//! WHERE THE TWO SPECS DISAGREE WITH THE LISTING (the listing is followed):
//! - the difficulty factor 0x140642820 is applied to victims WITH and WITHOUT a status object
//!   (both paths reach 0x14063285D); the combat spec's pseudocode has it in an `else`;
//! - a RECOVER hit (0x14064E770 true) returns the attack stat (`max(A, 0)`, 0.1 when 0), not 0.1;
//!   `power <= 0` returns 0.1.

use super::ability_set::AbilitySet;
use super::math::{ue_ceil, ue_floor};
use super::tables::GameLevel;
use super::Status;
use crate::enums::{ETresAbilityKind, ETresDamageAttribute};
use crate::host::TargetDefense;

/// Which side the attacker is on for the difficulty factor (0x140642820): the player side is
/// actor kind 1 or 10, or an actor of kind 3 / 5 / 11 owned by one; everything else is the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Player,
    Other,
}

/// The attacker: whose stat is used (`stat source`) and whose abilities count (`abilities`).
#[derive(Clone, Copy, Debug)]
pub struct Attacker<'a> {
    /// False: no status object (the stat is then 0.0, as the function's null path).
    pub has_status: bool,
    /// `Strength(owner)` 0x140580E70 and `MagicPower(owner)` 0x1405836E0, final values.
    pub strength: i32,
    pub magic: i32,
    /// The ability source's set; `None` = no status object: the whole ability block is skipped.
    pub abilities: Option<&'a AbilitySet>,
    pub side: Side,
    /// The attacker actor's `vt+0x98` answer: x1.5 (a pawn buff state; meaning not determined,
    /// spec open question 6). False unless the owner of that state says otherwise.
    pub state_boost: bool,
}

impl Attacker<'static> {
    /// An enemy with an attack stat and no abilities (a hit on Sora).
    pub fn enemy(attack: i32) -> Attacker<'static> {
        Attacker { has_status: true, strength: attack, magic: attack, abilities: None, side: Side::Other, state_boost: false }
    }
}

/// `bp+0x7C`, as the function tests it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DefenderKind {
    /// Kind 0: the player.
    Player,
    /// Kinds 3 and 4: friends (0x1405891A0 groups them with the player).
    Friend,
    /// Kinds 1 and 5: the enemy kinds (0x140589180).
    Enemy,
    /// Anything else.
    #[default]
    Other,
}

/// The defender's numbers. `has_status == false` is a victim without a KH3 status object (a
/// foreign host's enemy that supplies nothing): Defense 0, no floors, caps or resists.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Defender {
    pub has_status: bool,
    /// `Defense(victim)` 0x140581830.
    pub defense: i32,
    pub max_hp: i32,
    /// HP percent 0..100 (0x1405822D0), for Damage Control.
    pub hp_percent: f32,
    pub kind: DefenderKind,
    /// `m_DamageMin` / `m_DamageMax` of the base row (`bp+0xA4` / `bp+0xA8`).
    pub damage_min: i32,
    pub damage_max: i32,
    /// Attribute resist percent by `ETresDamageAttribute` 0..7 (0x1405850B0).
    pub resist: [i32; 8],
    /// `bp+0xDC` through 0x140585BC0: the second resist, applied to hits flagged `info+0x50` bit 4.
    pub second_resist: i32,
    /// 0x1405845D0 (`bp+0x114`): a percent cut applied to hits flagged `info+0x51` bit 6.
    pub cut: i32,
    /// 0x140589170 (`bp+0x89` bit 0): damage x0.25.
    pub quarter: bool,
    /// `bp+0x7D`: 1 Heartless, 2 Nobody (the buster abilities); 0 otherwise.
    pub bio_type: u8,
    /// Count of CRITICAL_HALF (Damage Control) on the defender.
    pub critical_half: u32,
}

impl Defender {
    /// A victim without a status object.
    pub const NONE: Defender = Defender { has_status: false, defense: 0, max_hp: 0, hp_percent: 100.0, kind: DefenderKind::Other, damage_min: 0, damage_max: 0, resist: [0; 8], second_resist: 0, cut: 0, quarter: false, bio_type: 0, critical_half: 0 };

    /// From what the host says about a target (`Targets::defense`); `None` = no status object.
    pub fn of_target(defense: Option<&TargetDefense>) -> Defender {
        match defense {
            None => Defender::NONE,
            Some(d) => Defender {
                has_status: true,
                defense: d.defense,
                max_hp: d.max_hp,
                hp_percent: d.hp_percent,
                kind: if d.enemy_kind { DefenderKind::Enemy } else { DefenderKind::Other },
                damage_min: d.damage_min,
                damage_max: d.damage_max,
                resist: d.resist,
                second_resist: 0,
                cut: 0,
                quarter: false,
                bio_type: d.bio_type,
                critical_half: 0,
            },
        }
    }
}

/// `info+0x65`: how the amount is computed.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum CalcType {
    /// 0: the stat formula.
    #[default]
    Stats,
    /// 1: `ceil(power * defender max HP)` (percentage attacks).
    MaxHpRatio,
    /// 2: `ceil(power * value)`, value = the defender actor's `vt+0xB8` result (not determined
    /// which quantity; the caller supplies it).
    ValueRatio(i32),
}

/// The damage-relevant part of an attack: the row's `m_Power` and the `FTresDamageInfo` fields
/// the function reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageAttack {
    /// `m_Power` (primitive+0x980).
    pub power: f32,
    /// `info+0x0B`.
    pub attribute: ETresDamageAttribute,
    /// `info+0x50` bit 0: the Magic stat is used and the magic combo boost applies.
    pub magic: bool,
    /// `info+0x50` bit 4: the defender's second resist applies ("rapid fire" by reflection order).
    pub second_resist: bool,
    /// `info+0x50` bit 7: aerial (Air Combo Boost instead of Combo Boost).
    pub aerial: bool,
    /// `info+0x51` bit 1: a counter (reprisal) attack.
    pub counter: bool,
    /// `info+0x51` bit 2: just guard.
    pub just_guard: bool,
    /// `info+0x51` bit 4: cast at full MP.
    pub full_mp: bool,
    /// `info+0x51` bit 6: the defender's percent cut applies.
    pub cut: bool,
    /// `info+0x54` (i8): the combo value of the hit; used when > 0.
    pub combo_step: i8,
    pub calc: CalcType,
    /// 0x14064E770(info): the info says "no damage" (damage kind RECOVER): the function returns
    /// the attack stat.
    pub recover: bool,
}

impl DamageAttack {
    /// A plain physical ground hit of `power`.
    pub const fn physical(power: f32) -> DamageAttack {
        DamageAttack { power, attribute: ETresDamageAttribute::Physical, magic: false, second_resist: false, aerial: false, counter: false, just_guard: false, full_mp: false, cut: false, combo_step: 0, calc: CalcType::Stats, recover: false }
    }

    /// A magic hit of `power` and `attribute`.
    pub const fn magic(power: f32, attribute: ETresDamageAttribute) -> DamageAttack {
        DamageAttack { magic: true, attribute, ..DamageAttack::physical(power) }
    }
}

/// What the function reads from the game instance and its two flag arguments.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageContext {
    pub game_level: GameLevel,
    /// `GW+0x7E09` through 0x140EA85D0 (1 when the game instance is missing).
    pub battle_level: i32,
    /// Code-menu option B0 (0x140EAB160): off in a port without that menu.
    pub option_b0: bool,
    /// The function's last argument: no ability bonus that depends on the hit (combo boosts,
    /// Double Team, Full MP Blast are skipped).
    pub no_bonus: bool,
    /// The function's sixth argument: with it a hit whose info is not "no damage" returns 0.1.
    pub flag_only: bool,
    /// Double Team (JOIN_FORCE 100): the defender is in the attacker team's shared target list.
    pub join_force_target: bool,
}

impl DamageContext {
    pub const fn new(game_level: GameLevel, battle_level: i32) -> DamageContext {
        DamageContext { game_level, battle_level, option_b0: false, no_bonus: false, flag_only: false, join_force_target: false }
    }

    /// The context of Sora's status: its difficulty and battle level.
    pub fn of(status: &Status) -> DamageContext {
        DamageContext::new(status.game_level, status.battle_level as i32)
    }
}

/// The result, with the intermediate values for tests and debug text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageResult {
    /// THE AMOUNT `TakeDamage` receives. An exact 0 is returned as 0.1 (floored to 0 HP by the
    /// HP rule); otherwise a whole number.
    pub amount: f32,
    /// `max(S - D, 1)` after the defender's floors and caps, before the power (step 5).
    pub base: f32,
    /// After `ceil((base - 1e-6) * (power - 1e-6))` (step 6).
    pub after_power: f32,
    /// After the defender's resists and the difficulty factor (step 8).
    pub after_difficulty: f32,
}

/// `DifficultyFactor` 0x140642820 (stats spec section 3; sheet `sora-difficulty` columns
/// `player_damage` / `enemy_damage`). [I] 0x1406429E1, 0x1406429EB, 0x1406429D3, 0x1406429C5.
pub fn difficulty_factor(side: Side, game_level: GameLevel) -> f32 {
    let row = game_level.row();
    match side {
        Side::Player => row.player_damage,
        Side::Other => row.enemy_damage,
    }
}

fn nz(x: f32) -> f32 {
    if x == 0.0 {
        0.1
    } else {
        x
    }
}

/// `CalcDamage` 0x140632240. See the module text for how to call it.
pub fn compute_damage(attacker: &Attacker, defender: &Defender, attack: &DamageAttack, cx: &DamageContext) -> DamageResult {
    let power = attack.power;
    // S = Strength of the stat source, or Magic when info+0x50 bit 0; D = defender Defense.
    let s_raw = if !attacker.has_status {
        0.0
    } else if attack.magic {
        attacker.magic as f32
    } else {
        attacker.strength as f32
    };
    let d = if defender.has_status { defender.defense as f32 } else { 0.0 };
    let done = |x: f32| DamageResult { amount: nz(x), base: 0.0, after_power: x, after_difficulty: x };
    // 1. 0x1406323F1..0x140632451: "no damage" info keeps the stat; the flagged call zeroes
    //    everything else; power <= 0 zeroes.
    let mut x = s_raw.max(0.0);
    let mut go = true;
    if attack.recover {
        go = false;
    } else if cx.flag_only {
        x = 0.0;
        go = false;
    }
    if !(power > 0.0) {
        x = 0.0;
        go = false;
    }
    if !go {
        return done(x);
    }
    // 2. info+0x65: percentage attacks.
    match attack.calc {
        CalcType::MaxHpRatio => return done(ue_ceil(power.max(0.0) * if defender.has_status { defender.max_hp as f32 } else { 0.0 }) as f32),
        CalcType::ValueRatio(value) => return done(ue_ceil(power.max(0.0) * value as f32) as f32),
        CalcType::Stats => {}
    }
    let count = |kind: ETresAbilityKind| attacker.abilities.map_or(0, |set| set.count(kind)) as i32;
    // 3. Double Team: S += 5. [I] 0x140632542
    if !cx.no_bonus && count(ETresAbilityKind::JoinForce) > 0 && cx.join_force_target {
        x += 5.0;
    }
    // 4. base = max(S - D, 1).
    x -= d;
    if !(x >= 1.0) {
        x = 1.0;
    }
    // 5. The defender's limits.
    let level = cx.battle_level;
    if defender.has_status {
        let mut lo = defender.damage_min as f32;
        let mut hi = (defender.damage_max as f32).max(lo);
        let enemy = defender.kind == DefenderKind::Enemy;
        if lo > 0.0 {
            if x < lo {
                x = lo;
            }
        } else if enemy {
            // min = 1 + ceil(0.1 * battle level)
            lo = 1.0 + ue_ceil(level as f32 * 0.1) as f32;
            if lo > 0.0 && x < lo {
                x = lo;
            }
        } else {
            lo = if level > 70 { 13.0 } else { 1.0 };
            if x < lo {
                x = lo;
            }
        }
        if hi > 0.0 && x > hi {
            // t = floor((max + S - D) * 0.5), raised to min; the raw stat is used here.
            let mut t = ue_floor((hi + s_raw - d) * 0.5) as f32;
            if lo > 0.0 {
                t = t.max(lo);
            }
            if x > t {
                x = t;
            }
        }
        if level > 70 && enemy {
            hi = if cx.option_b0 { hi * 6.0 } else { hi + hi };
            if x > hi && hi > lo {
                x = hi;
            }
        }
        // The player and friends: base at most 80 unless min >= 80. [I] 0x1406326B5
        if matches!(defender.kind, DefenderKind::Player | DefenderKind::Friend) && x > 80.0 && !(lo >= 80.0) {
            x = 80.0;
        }
    }
    let base = x;
    // 6. dmg = ceil((base - 1e-6) * (power - 1e-6)). [I] 0x1406326DF..0x140632705
    const EPS: f32 = 9.999_999_974_752_427e-7;
    x = ue_ceil((x - EPS) * (power - EPS)) as f32;
    let after_power = x;
    // 7. The defender's cuts and resists.
    if defender.has_status {
        if defender.cut > 0 && attack.cut {
            x = ue_ceil((100 - defender.cut.min(100)) as f32 * 0.01 * x) as f32;
        }
        let index = (attack.attribute.value() as usize).min(7);
        let factor = |resist: i32, top: f32| {
            let v = 100.0 - resist as f32;
            (if v < 0.0 { 0.0 } else { v.min(top) } * 0.01).max(0.0)
        };
        x = ue_ceil(factor(defender.resist[index], 200.0) * x) as f32;
        if attack.second_resist {
            x = ue_ceil(factor(defender.second_resist, 100.0) * x) as f32;
        }
        if defender.quarter {
            x = ue_ceil(x * 0.25) as f32;
        }
    }
    // 8. Difficulty, on both paths. [I] 0x14063285D
    x = ue_ceil(difficulty_factor(attacker.side, cx.game_level) * x) as f32;
    let after_difficulty = x;
    // 9. The attacker's abilities, one after another on the running value.
    if attacker.abilities.is_some() {
        let step = attack.combo_step as i32;
        if !cx.no_bonus && step > 0 {
            if !attack.magic {
                // COMBO_UP 0x1F / AIRCOMBO_UP 0x20: dmg += ceil(0.05 * n * c * dmg). [I] 0x1406328BC
                let n = count(if attack.aerial { ETresAbilityKind::AircomboUp } else { ETresAbilityKind::ComboUp });
                if n > 0 {
                    x += ue_ceil((step * n) as f32 * 0.05 * x) as f32;
                }
            } else {
                // MAGIC_COMBO_UP 0x39: dmg += ceil(0.1 * n * c * dmg). [I] 0x1406328DE
                let n = count(ETresAbilityKind::MagicComboUp);
                if n > 0 {
                    x += ue_ceil((n * step) as f32 * 0.1 * x) as f32;
                }
            }
        }
        if attack.counter {
            // COUNTER_UP 0x45: dmg += ceil(0.5 * n * dmg). [I] 0x140632927
            let n = count(ETresAbilityKind::CounterUp);
            if n > 0 {
                x += ue_ceil(n as f32 * 0.5 * x) as f32;
            }
        }
        // CRITICAL_COUNTER 0x68 with counter + just guard: dmg *= 2. [I] 0x140632971
        if attack.counter && attack.just_guard && count(ETresAbilityKind::CriticalCounter) > 0 {
            x += x;
        }
        // FIRE_UP .. AERO_UP 0x21..0x25 by attribute 1..5: dmg += ceil(0.2 * n * dmg). [I] 0x1406329BD
        let boost = match attack.attribute {
            ETresDamageAttribute::Fire => Some(ETresAbilityKind::FireUp),
            ETresDamageAttribute::Blizzard => Some(ETresAbilityKind::BlizzardUp),
            ETresDamageAttribute::Thunder => Some(ETresAbilityKind::ThunderUp),
            ETresDamageAttribute::Water => Some(ETresAbilityKind::WaterUp),
            ETresDamageAttribute::Aero => Some(ETresAbilityKind::AeroUp),
            _ => None,
        };
        if let Some(kind) = boost {
            let n = count(kind);
            if n > 0 {
                x += ue_ceil(n as f32 * 0.2 * x) as f32;
            }
        }
        // HEARTLESS_BUSTER 0x65 / NOBODY_BUSTER 0x66: dmg = ceil((1 + 0.3 n) * dmg). [I] 0x1406329FF, 0x140632A52
        for (kind, bio) in [(ETresAbilityKind::HeartlessBuster, 1), (ETresAbilityKind::NobodyBuster, 2)] {
            let n = count(kind);
            if defender.has_status && defender.bio_type == bio && n > 0 {
                x = ue_ceil((n as f32 * 0.3 + 1.0) * x) as f32;
            }
        }
        // The attacker actor's vt+0x98: x1.5. [I] 0x140632AA2
        if attacker.state_boost {
            x = ue_ceil(x * 1.5) as f32;
        }
        // FULLMP_BURST 0x4D on hits flagged full MP: dmg += ceil(0.5 * n * dmg). [I] 0x140632AE3
        if !cx.no_bonus && attack.full_mp {
            let n = count(ETresAbilityKind::FullmpBurst);
            if n > 0 {
                x += ue_ceil(n as f32 * 0.5 * x) as f32;
            }
        }
    }
    // 10. Damage Control on the defender at HP percent <= 25: x0.5 with one copy, x1/3 with
    //     two or more. [I] 0x140632B3A..0x140632B97
    if defender.has_status && defender.critical_half > 0 && defender.hp_percent <= 25.0 {
        x = ue_ceil(x * if defender.critical_half > 1 { 0.333_333_34 } else { 0.5 }) as f32;
    }
    // 11. A result of exactly 0 is returned as 0.1.
    DamageResult { amount: nz(x), base, after_power, after_difficulty }
}
