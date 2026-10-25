//! (XSI, 2026-10-16) THE 16 XS ABILITIES (build/sora_tmp/party/XS_SORA_ABILITIES.md, approved by the user with the
//! 9 recommended answers; "don't start with critical counter": Critical Counter is a level C 18 / Godskin Noble
//! ability, not part of Critical Mode's start). Attraction Flow stays excluded (dec029).
//!
//! Every number is a row of `sheets/port/sora-xs-abilities` ([`X`]); Hidden Potential's region levels are
//! `sheets/port/sora-xs-battle-levels` ([`BATTLE_LEVELS`]). Both are OUR DESIGN unless a row names a KH3 source. The
//! abilities' kinds are the port's own `ETresAbilityKind` values 256 .. 269 (`sheets/port/sora-enum-ext`), their
//! attack kinds 0xE0 .. (`sheets/port/sora-combat-attacks`, tools/sora_combat_sheet.js), their KH3 rows the
//! ones of `p_ex001_AttackData` they name (`sora-combat-attack-data`).
//!
//! | ability | where |
//! |---|---|
//! | Diving Strike | `dive.rs`; the head-first pose is Jump sub-steps 7 / 8 (states/jump.rs), the strike attack kind 0x35 |
//! | Sonic Dash | `sonic.rs` (the air-dodge request, attack kinds 0xE0 / 0xE1) |
//! | Ancient Light | `ancient.rs` (the situation offer LM_UNIONDISCODE 174, attack kind 0xE2) |
//! | Spectral Rays | `spectral.rs` (the shotlock kind 11, states/shoot_flow.rs; the Customize choice `CommandState::shotlock`) |
//! | Critical Counter | KH3's code (states/guard.rs, status/damage.rs); the Elden Ring addition here ([`just_guard`]) |
//! | Hidden Potential | [`overcome_stats`] (combat/hit.rs `damage`, magic/projectile.rs `damage_of`) |
//!
//! THE HIT HOOKS (combat/hit.rs `apply`): [`pre_hit`] before the classification (reaction power), [`adjust_damage`]
//! after the damage function (the dive bonus, the refraction share, the spectacle cap).

pub mod ancient;
pub mod chandelier;
pub mod dive;
pub mod finishers;
pub mod light_wave;
pub mod myriad;
pub mod rage;
pub mod sonic;
pub mod spectral;

use crate::combat::Window;
use crate::enums::ETresAbilityKind as A;
use crate::host::{BodyReaction, HitInfo, TargetId, TargetInfo};
use crate::pawn::{Ctx, Pawn};
use crate::sora::Input;
use crate::states::StateData;

include!(concat!(env!("KH_GEN_DIR"), "/sora_xs_abilities.rs"));
include!(concat!(env!("KH_GEN_DIR"), "/sora_xs_battle_levels.rs"));

/// The numbers (`sheets/port/sora-xs-abilities`).
pub const X: XsAbilityParams = XS_ABILITY_PARAMS;
/// Hidden Potential's battle levels by region depth (`sheets/port/sora-xs-battle-levels`).
pub const BATTLE_LEVELS: XsBattleLevels = XS_BATTLE_LEVELS_PARAMS;

/// The port's attack kinds (`sora-combat-attacks` kind_e0 ..).
pub mod kind {
    /// KH3's own dive-fall attack (DiveFallAttack 1050): Diving Strike forces it.
    pub const DIVE: u8 = 0x35;
    pub const SONIC_DASH: u8 = 0xE0;
    pub const SONIC_LAUNCH: u8 = 0xE1;
    pub const ANCIENT_LIGHT: u8 = 0xE2;
}

/// The XS abilities' per-pawn state (part of `PawnCombat`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct XsState {
    /// Seconds since the pawn was made (the timers below are stamps of it).
    pub clock: f32,
    /// Diving Strike while it runs.
    pub dive: Option<dive::DiveRun>,
    /// Sonic Dash: charges used this airtime, when the last dash ended.
    pub sonic: sonic::SonicState,
    /// A spectacle move's cap ledger while it runs (Ancient Light, Myriad).
    pub spectacle: Option<Spectacle>,
    /// Hits the XS hooks changed so far (for tests and the debug text): dive bonuses, refractions, capped hits.
    pub adjusted: u32,
    /// Seconds the Attack button has been held (0 when it is up): Zantetsuken's choice.
    pub attack_held: f32,
    /// Zantetsuken while it runs.
    pub zanto: Option<finishers::Zanto>,
    /// Rage Raid and Riskcharge.
    pub rage: rage::RageState,
    /// Light Wave.
    pub wave: light_wave::WaveState,
    /// Chandelier while it runs.
    pub chandelier: Option<chandelier::Chandelier>,
    /// Break Boost: hits whose reaction power it raised (for tests).
    pub boosted: u32,
    /// DEBUG (`Sora::debug_xs_finisher`, the test level's script word `xs-finisher`): the next ground finisher is this XS
    /// kind whatever its condition, once (the ability must still be equipped). For captures of the moves.
    pub debug_finisher: Option<u8>,
}

/// (XSI) `HitRecord::flags` bit of a Break Boost hit (XS 2.16): kh-react keeps the endurance open
/// `break_boost.open_time`, the Elden Ring host adds `break_boost.poise` to its poise damage.
pub const FLAG_BREAK_BOOST: u32 = 1 << 24;

/// The 25 % cap of one use of a spectacle move (XS section 1, Q7; sheet row `spectacle.cap_share`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Spectacle {
    /// The attack kind whose hits are counted.
    pub kind: u8,
    /// KH3 damage dealt per target by this use.
    pub dealt: Vec<(TargetId, f32)>,
    /// Hits the cap cut.
    pub capped: u32,
}

pub(crate) fn has(s: &Pawn, ability: A) -> bool {
    s.status.has(ability)
}

/// A POWERFUL FOE (our rule, XS 2.6 / 2.13 / 2.15): a STRONG body (Heavy / Giant of kh-react's class table) or a
/// target the host calls a boss or Giant (`Targets::foe_level`).
pub(crate) fn powerful(cx: &Ctx, id: TargetId) -> bool {
    id != TargetId::NONE && (cx.host.targets.body(id, 0).reaction == BodyReaction::Strong || cx.host.targets.foe_level(id).is_some())
}

/// The per-step part (controller update, after the formchange gauge's tick): the clock, the airtime of Sonic
/// Dash, the Ancient Light offer.
pub(crate) fn tick(state: &mut StateData, cx: &mut Ctx, input: &Input, dt: f32) {
    cx.pawn.combat.xs.clock += dt;
    let xs = &mut cx.pawn.combat.xs;
    xs.attack_held = if input.pad.held.has(crate::input::PadButton::ATTACK) { xs.attack_held + dt } else { 0.0 };
    sonic::tick(state, cx);
    ancient::reconcile_offer(cx);
    rage::tick(state, cx);
    light_wave::tick(cx, input, dt);
}

// ------------------------------------------------------------------------------------------------ attacks

/// `TryAttack`'s question (states/attack.rs): an XS attack instead of the decided one, `(kind, target)`.
pub(crate) fn forced_attack(state: &StateData, cx: &Ctx) -> Option<(u8, TargetId)> {
    if let Some(t) = rage::raid_due(cx) {
        return Some((rage::RAGE_RAID, t));
    }
    if sonic::launch_due(cx) {
        let target = cx.pawn.combat.xs.sonic.last_target;
        return Some((kind::SONIC_LAUNCH, target));
    }
    dive::strike_target(state, cx).map(|t| (kind::DIVE, t))
}

/// The tail of `PlayAttack` (states/attack.rs) for the XS kinds.
pub(crate) fn on_play_attack(cx: &mut Ctx, attack_kind: u8, target: TargetId) {
    match attack_kind {
        kind::DIVE => dive::begin(cx, target),
        kind::SONIC_DASH | kind::SONIC_LAUNCH => sonic::begin(cx, attack_kind, target),
        kind::ANCIENT_LIGHT => ancient::begin(cx),
        k if finishers::owns(k) => finishers::begin(cx, k),
        k if rage::owns(k) => rage::begin(cx, k),
        myriad::MYRIAD => myriad::begin(cx),
        light_wave::TAP | light_wave::HOLD => light_wave::begin(cx),
        _ => {}
    }
    // The effect group of the clip's play (notify.rs, the effect-group rule): the Light Form clips' Oathkeeper rows,
    // Riskcharge's rows of its charge level. The other XS clips' rows are group 0 (the plain play's).
    match attack_kind {
        finishers::SPIRAL_RUSH | finishers::ZANTETSUKEN | finishers::EVENT_HORIZON => cx.pawn.next_effect_group = Some(X.light_form_effect_group as i32),
        rage::RISKCHARGE => cx.pawn.next_effect_group = Some(X.riskcharge_effect_group_base as i32 + cx.pawn.combat.xs.rage.level as i32),
        _ => {}
    }
}

/// The attack state's tick (after its traction) for the XS kinds.
pub(crate) fn attack_tick(cx: &mut Ctx, attack_kind: u8, dt: f32) {
    match attack_kind {
        kind::DIVE => dive::tick(cx, dt),
        kind::SONIC_DASH | kind::SONIC_LAUNCH => sonic::attack_tick(cx, attack_kind),
        k if finishers::owns(k) => finishers::tick(cx, k, dt),
        k if rage::owns(k) => rage::tick_attack(cx, k),
        myriad::MYRIAD => myriad::tick(cx),
        chandelier::CHANDELIER => chandelier::tick(cx),
        light_wave::TAP | light_wave::HOLD => light_wave::attack_tick(cx),
        _ => {}
    }
}

/// The attack state's End for the XS kinds.
pub(crate) fn attack_end(cx: &mut Ctx, attack_kind: u8) {
    match attack_kind {
        kind::DIVE => dive::end(cx),
        kind::SONIC_DASH | kind::SONIC_LAUNCH => sonic::end(cx, attack_kind),
        kind::ANCIENT_LIGHT => ancient::end(cx),
        k if finishers::owns(k) => finishers::end(cx, k),
        k if rage::owns(k) => rage::end(cx, k),
        myriad::MYRIAD => myriad::end(cx),
        chandelier::CHANDELIER => chandelier::end(cx),
        _ => {}
    }
}

/// The HUD's name of a command the XS abilities use that KH3 has no text for (Light Wave on the unused Reserve248).
pub fn command_name(kind: i64) -> Option<&'static str> {
    (kind == X.light_wave_command as i64).then_some("Light Wave")
}

/// The shapes of the XS clips' collision groups that the Kingdom Key's component lacks (combat/hit.rs `shape`).
pub(crate) fn group_shape(cx: &Ctx, group: &str, target: TargetId) -> Option<(glam::Vec3, glam::Vec3, f32)> {
    ancient::group_shape(cx, group, target).or_else(|| finishers::group_shape(cx, group)).or_else(|| myriad::group_shape(cx, group)).or_else(|| chandelier::group_shape(cx, group))
}

/// BREAK BOOST's hits (XS 2.16): finishers, reprisals and launchers (the attack kinds' role) on a STRONG body.
fn break_boosted(cx: &Ctx, window: &Window, target: TargetId) -> bool {
    if !cx.pawn.status.has(A::BreakBoost) || cx.host.targets.body(target, 0).reaction != BodyReaction::Strong {
        return false;
    }
    let role = crate::combat::tables::attack_kind(window.info.kind).map_or("", |r| r.role);
    matches!(role, "finisher" | "counter" | "launch") || window.info.finish_end
}

/// The `HitRecord::flags` the XS abilities add (combat/hit.rs `apply`).
pub(crate) fn record_flags(cx: &Ctx, window: &Window, body: &crate::host::TargetBody, _info: &HitInfo) -> u32 {
    let strong = body.reaction == BodyReaction::Strong;
    if strong && _info.reaction_power >= 1 && cx.pawn.status.has(A::BreakBoost) && matches!(crate::combat::tables::attack_kind(window.info.kind).map_or("", |r| r.role), "finisher" | "counter" | "launch") {
        FLAG_BREAK_BOOST
    } else {
        0
    }
}

// ------------------------------------------------------------------------------------------------ hits

/// Before a hit is classified (combat/hit.rs `apply`): what the XS abilities change of its reaction power.
pub(crate) fn pre_hit(cx: &mut Ctx, window: &Window, target: TargetId, info: &mut HitInfo) {
    // Zantetsuken's full charge (XS 2.4): reaction power `zantetsuken.full_rp`.
    if window.info.kind == finishers::ZANTETSUKEN && finishers::full_charge(cx) {
        info.reaction_power = info.reaction_power.max(X.zantetsuken_full_rp as i32);
    }
    // Break Boost: +`break_boost.rp` on a STRONG body for rows that have reaction power (XS 2.16).
    if info.reaction_power >= 1 && break_boosted(cx, window, target) {
        info.reaction_power += X.break_boost_rp as i32;
        cx.pawn.combat.xs.boosted += 1;
    }
}

/// After the damage function (combat/hit.rs `apply`): the XS changes of the amount, in KH3 damage points.
pub(crate) fn adjust_damage(cx: &mut Ctx, window: &Window, target: &TargetInfo, amount: f32) -> f32 {
    let mut amount = amount;
    // Diving Strike: the END hit gains with the fall (XS 2.1, Q8).
    if window.info.kind == kind::DIVE && window.row_name == "DIVEFALL_ATTACK_END" {
        if let Some(d) = cx.pawn.combat.xs.dive {
            let k = 1.0 + dive::fall_bonus(d.fallen);
            if k != 1.0 {
                amount *= k;
                cx.pawn.combat.xs.adjusted += 1;
            }
        }
    }
    // Zantetsuken's full charge: power x`zantetsuken.full_power` (XS 2.4).
    if window.info.kind == finishers::ZANTETSUKEN && finishers::full_charge(cx) {
        amount *= X.zantetsuken_full_power;
        cx.pawn.combat.xs.adjusted += 1;
    }
    // Chandelier: x the tier cast's factor (XS 2.7).
    if window.info.kind == chandelier::CHANDELIER {
        amount *= chandelier::scale(cx);
    }
    // Light Wave: x (1 + 0.25 per -ga) (XS 2.9).
    if window.group == "XS_WAVE" {
        amount *= light_wave::power(cx);
    }
    // Rage Raid's END hit: the body's revenge ends (XS 2.11).
    if window.info.kind == rage::RAGE_RAID && window.row_name == "AN_COUNTER_END" {
        rage::raid_end_hit(cx, target.id);
    }
    // Spectral Rays: a refracted beam (XS 2.8).
    if window.group == spectral::GROUP_REFRACT {
        amount *= X.spectral_refract_scale;
        cx.pawn.combat.xs.adjusted += 1;
    }
    // The spectacle cap (XS 1, Q7): at most a share of the target's maximum HP per use.
    if let Some(sp) = cx.pawn.combat.xs.spectacle.as_mut().filter(|sp| sp.kind == window.info.kind) {
        let per_hp = cx.host.targets.damage_to_hp().max(1.0e-6);
        if target.hp_max > 0.0 {
            let cap = X.spectacle_cap_share * target.hp_max / per_hp;
            let at = match sp.dealt.iter().position(|(id, _)| *id == target.id) {
                Some(at) => at,
                None => {
                    sp.dealt.push((target.id, 0.0));
                    sp.dealt.len() - 1
                }
            };
            let left = (cap - sp.dealt[at].1).max(0.0);
            if amount > left {
                amount = left;
                sp.capped += 1;
            }
            sp.dealt[at].1 += amount;
        }
    }
    amount
}

/// HIDDEN POTENTIAL (OVERCOME 179, KH3's "Gain additional Strength and Magic when battling powerful foes"):
/// against a powerful foe of battle level `bl` Strength and Magic become at least
/// `EnemyBaseStatus[bl].m_BaseDefensePower + ceil(0.15 bl + 1.5)` (0x140580E70 / 0x1405836E0). KH3 applies it to
/// every foe at the world's battle level; the port only against bosses and Giant bodies (XS 2.15, our rule), at the
/// level the host gives (`Targets::foe_level`; Elden Ring: the region band of sora-xs-battle-levels).
pub(crate) fn overcome_stats(cx: &Ctx, target: TargetId, strength: i32, magic: i32) -> (i32, i32) {
    if !cx.pawn.status.has(A::Overcome) || target == TargetId::NONE {
        return (strength, magic);
    }
    let Some(level) = cx.host.targets.foe_level(target) else { return (strength, magic) };
    let floor = overcome_floor(level);
    (strength.max(floor), magic.max(floor))
}

/// The floor of battle level `level` (1 .. 99): `EnemyBaseStatus[level].m_BaseDefensePower + ceil(0.15 level + 1.5)`.
pub fn overcome_floor(level: u8) -> i32 {
    let level = level.clamp(1, 99) as usize;
    let defense = ENEMY_BASE_DEFENSE.get(level - 1).copied().unwrap_or(0);
    defense + crate::status::math::ue_ceil(0.15 * level as f32 + 1.5)
}

/// The battle level of region depth `depth` (0 Limgrave .. 4 the DLC): `sheets/port/sora-xs-battle-levels`.
pub fn region_battle_level(depth: Option<u8>) -> u8 {
    let b = BATTLE_LEVELS;
    let v = match depth {
        Some(0) => b.depth_0,
        Some(1) => b.depth_1,
        Some(2) => b.depth_2,
        Some(3) => b.depth_3,
        Some(_) => b.depth_4,
        None => b.default,
    };
    v.clamp(1.0, 99.0) as u8
}

/// `EnemyBaseStatus` (sheets/entities/Load/Common/EnemyBaseStatus.json) `m_BaseDefensePower` by battle level 1 .. 99
/// [A: read from the game's table; pinned by a test against the sheet].
pub const ENEMY_BASE_DEFENSE: [i32; 99] = [
    8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14, 15, 15, 16, 16, 17, 17, 18, 18, 19, 19, 20, 20, 21, 21, 22, 22, 23, 23, 24, 24, 25, 25, 26, 26, 27, 27, 28, 28, 29, 29, 30, 30, 31, 31, 32, 32, 33, 33, 34, 34, 35, 35, 36, 36, 37, 37, 38, 38, 39, 39, 40, 40, 41, 41, 42, 42, 43, 43, 44, 44, 45, 45, 46, 46, 47, 47, 48, 48, 49, 49, 50, 50, 51, 51, 52, 52, 53, 53, 54, 54, 55, 55, 56, 56, 57,
];

/// CRITICAL COUNTER, the Elden Ring addition (XS 2.10, ours): a just guard against a STRONG body takes
/// `critical_counter.strong_rp` off its reaction endurance (`Targets::take_reaction_power`; no damage). Called by the
/// guard state's `OnGuardedHit` with its "critical" (just-guard) answer.
pub(crate) fn just_guard(cx: &mut Ctx, causer: TargetId) {
    if causer == TargetId::NONE || !cx.pawn.status.has(A::CriticalCounter) {
        return;
    }
    if cx.host.targets.body(causer, 0).reaction == BodyReaction::Strong {
        cx.host.targets.take_reaction_power(causer, X.critical_counter_strong_rp as i32);
        cx.pawn.combat.xs.adjusted += 1;
    }
}

#[cfg(test)]
mod tests;
