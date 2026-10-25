//! Passive abilities that are a number handed to another system (docs/sora-stats-spec.md 5.4).
//! Each function cites its row of `sheets/port/sora-abilities` and the address its number was
//! read at. They read the counts through `Status::count` (the player pawn's query).

use crate::enums::ETresAbilityKind;
use crate::status::math::ue_ceil;
use crate::status::{PrizeKind, Status};

/// TREASURE MAGNET (PRIZE_DRAW 41) / MASTER TREASURE MAGNET (MASTER_DRAW 61), 0x1414794C0: the
/// prize pick-up radius is 300 / 600 / 900 cm for 0 / 1 / 2+ copies (table 0x1469AB6D8, squared);
/// with the master ability there is no distance limit (`None`).
pub fn prize_draw_radius(status: &Status) -> Option<f32> {
    if status.has(ETresAbilityKind::MasterDraw) {
        return None;
    }
    Some(match status.count(ETresAbilityKind::PrizeDraw) {
        0 => 300.0,
        1 => 600.0,
        _ => 900.0,
    })
}

/// PRIZE CONVERSION 0x141472DC0 (rows HP_CONVERTER 79, MP_CONVERTER 80, MUNNY_CONVERTER 81,
/// FP_CONVERTER 83, MILLIONAIRE 176): HP small -> large; MP small -> large; munny 1 -> 5 and
/// 5 -> 10; focus small -> large; Rags to Riches: kinds 0, 2, 7 -> munny 1, kinds 1, 3, 4 ->
/// munny 5, kinds 5 and 6 -> munny 10. The spec does not say in which order the function applies
/// them when several are counted: Rags to Riches (the wider rule) is applied first here, then
/// the converters, each once.
pub fn convert_prize(status: &Status, kind: PrizeKind) -> PrizeKind {
    use PrizeKind::*;
    let mut kind = kind;
    if status.has(ETresAbilityKind::Millionaire) {
        kind = match kind {
            HpSmall | MpSmall | FocusSmall => Munny1,
            HpLarge | MpLarge | Munny1 => Munny5,
            Munny5 | Munny10 => Munny10,
            FocusLarge => FocusLarge,
        };
        return kind;
    }
    match kind {
        HpSmall if status.has(ETresAbilityKind::HpConverter) => HpLarge,
        MpSmall if status.has(ETresAbilityKind::MpConverter) => MpLarge,
        Munny1 if status.has(ETresAbilityKind::MunnyConverter) => Munny5,
        Munny5 if status.has(ETresAbilityKind::MunnyConverter) => Munny10,
        FocusSmall if status.has(ETresAbilityKind::FpConverter) => FocusLarge,
        other => other,
    }
}

/// Which situation command a new timer belongs to (0x14140DA40).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandTimer {
    /// FORM_TIME 49: x(1 + 0.2 n). [I] site 0x14140DB76
    Formchange,
    /// MAGIC_TIME 72: x(1 + 0.2 n).
    GrandMagic,
    /// ATTRACTION_TIME 47: x(1 + 0.2 n) (command kinds 0xBA..0xBF).
    Attraction,
}

/// The longest situation-command timer, seconds (0x14140DA40).
pub const COMMAND_TIMER_CAP: f32 = 99.9;

/// The timer of a NEW situation command with the extender abilities (rows FORM_TIME,
/// MAGIC_TIME, ATTRACTION_TIME): `base * (1 + 0.2 * count)`, at most 99.9 s. (Team attacks use
/// the friend's COMBI_TIME, x(1 + 0.5 n): not Sora's.)
pub fn command_timer(status: &Status, command: CommandTimer, base_seconds: f32) -> f32 {
    let kind = match command {
        CommandTimer::Formchange => ETresAbilityKind::FormTime,
        CommandTimer::GrandMagic => ETresAbilityKind::MagicTime,
        CommandTimer::Attraction => ETresAbilityKind::AttractionTime,
    };
    (base_seconds * (1.0 + 0.2 * status.count(kind) as f32)).min(COMMAND_TIMER_CAP)
}

/// LINK EXTENDER (LINK_BOOST 48), 0x1413F9990: the link timer runs at 0.8x speed.
pub fn link_timer_speed(status: &Status) -> f32 {
    if status.has(ETresAbilityKind::LinkBoost) {
        0.8
    } else {
        1.0
    }
}

/// ATTRACTION ENHANCER (ATTRACTION_UP 62), 0x141432610: the chance of the green attraction
/// marker is `0.2 + 0.05 * count`; x0.5 on Critical (unless the flag of 0x140EAB9F0, which the
/// caller passes as `critical_exempt`); 0 with Critical Converter (CRITICAL_CONVERTER 106).
pub fn attraction_marker_chance(status: &Status, critical_exempt: bool) -> f32 {
    if status.has(ETresAbilityKind::CriticalConverter) {
        return 0.0;
    }
    let chance = 0.2 + 0.05 * status.count(ETresAbilityKind::AttractionUp) as f32;
    if status.game_level.is_critical() && !critical_exempt {
        chance * 0.5
    } else {
        chance
    }
}

/// CURE CONVERTER (POWER_CURE 103), FireMagic shot [I] 0x14137C8F5: at a Cure, before the MP is
/// spent, the situation gauge gets `15 + ceil((15 n + 20) * MP / maxMP)` points; `None` without
/// the ability.
pub fn cure_converter_points(status: &Status) -> Option<i32> {
    let n = status.count(ETresAbilityKind::PowerCure);
    if n == 0 || status.max_mp() <= 0 {
        return None;
    }
    Some(15 + ue_ceil((15 * n as i32 + 20) as f32 * status.mp as f32 / status.max_mp() as f32))
}

/// BLOCK REPLENISHER (GUARD_REGENE 74), 0x141331AF0: a successful block calls
/// `Recover(0, 3, 0, 0)`. The MP the guard state gives; 0 without the ability.
pub fn block_replenisher_mp(status: &Status) -> i32 {
    if status.has(ETresAbilityKind::GuardRegene) {
        3
    } else {
        0
    }
}

/// SITUATION BOOST (FORM_UP 71), 0x1413E9000: +0.2 on the situation-gauge gain factor; CRITICAL
/// RECHARGE (CRITICAL_CHARGE 105): while the MP charge runs the gain uses
/// `CriticalChargeStylePointAddRate` 1.3. Returns (added factor, rate while charging); how the
/// gauge code combines them is the combat strand's.
pub fn situation_gauge_terms(status: &Status) -> (f32, f32) {
    (if status.has(ETresAbilityKind::FormUp) { 0.2 } else { 0.0 }, if status.has(ETresAbilityKind::CriticalCharge) && status.mp_charging { 1.3 } else { 1.0 })
}

/// SUPERSLIDE 19 (pawn `vt+0x1AA8` 0x1414354B0): the dash lasts 0.2 / 0.35 / 0.5 s for fewer
/// than 2 / 2 / 3 or more copies (at `MySuperSlideSpeed` 4000 cm/s). For the action state.
pub fn superslide_time(status: &Status) -> f32 {
    match status.count(ETresAbilityKind::Superslide) {
        0 | 1 => 0.2,
        2 => 0.35,
        _ => 0.5,
    }
}

/// AIRSLIDE 23 (pawn `vt+0x1B90` 0x141432EB0): root-motion speed scale 0.725 / 1.075 / 1.425 for
/// fewer than 2 / 2 / 3 or more copies. For the action state.
pub fn airslide_speed_scale(status: &Status) -> f32 {
    match status.count(ETresAbilityKind::Airslide) {
        0 | 1 => 0.725,
        2 => 1.075,
        _ => 1.425,
    }
}

/// FOCUS SYPHON (FOCUS_ASPIR 46), 0x141433C90: absorb rate `MyFocusAspirRateLv1..3` = 1.4 / 1.9 /
/// 2.4 for 1 / 2 / 3 copies; `None` without the ability. For the shotlock implementer.
pub fn focus_syphon_rate(status: &Status) -> Option<f32> {
    match status.count(ETresAbilityKind::FocusAspir) {
        0 => None,
        1 => Some(1.4),
        2 => Some(1.9),
        _ => Some(2.4),
    }
}

/// HIGH JUMP 16 (pawn `vt+0x1AA0` 0x141434320): a held jump launches at `MyJumpPowerHighLv3`
/// 1950 cm/s instead of `MyJumpPowerNormal` 1480. Whether a held jump is a high jump (the jump
/// kind selection 0x141434010) is the movement / actions side's; this is only the gate.
pub fn has_high_jump(status: &Status) -> bool {
    status.has(ETresAbilityKind::Highjump)
}
