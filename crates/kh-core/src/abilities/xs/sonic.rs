//! (XSI) SONIC DASH (XS 2.2; ability SONIC_DASH 257, Flying Dragon Agheel). Second Form's Sonic Blade dashes
//! (`kh3_system.situation.lm_sonicrave`: `M_1425` with `LM_SONICRAVE` 1.2 SMALL, `M_1426` with `LM_SONICRAVE_END`
//! 1.5 TOSS; both already in the pack) given to Sora in the air, OUR input:
//! - in the air, locked on a target `sonic.range_min` .. `sonic.range_max` away, X with the stick toward it
//!   (within acos(`sonic.stick_cos`), at least half tilted): instead of the Air Slide / Aerial Dodge, attack kind
//!   0xE0 flashes through it: `M_1425`'s root motion scaled to `sonic.dash_length`, gravity off and invincible for
//!   `sonic.dash_time` (its attack and traction windows);
//! - charges per airtime: one per equipped Air Slide copy, at least one (KH3's count rule of the Air Slide);
//! - after the 2nd dash, Attack within `sonic.follow_window` plays `M_1426` (kind 0xE1), the TOSS launch;
//! - without a lock, too near / far, or with the stick away from the target, X is the Air Slide as before.

use super::{has, kind, X};
use crate::enums::ETresAbilityKind as A;
use crate::host::{TargetFlags, TargetId};
use crate::pawn::Ctx;
use crate::states::common::on_ground;
use crate::states::StateData;

/// The root motion of `M_1425` (manifest root_motion.tracks: 1099.31 cm forward).
pub const DASH_ROOT_CM: f32 = 1099.309;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SonicState {
    /// Dashes made this airtime.
    pub used: u32,
    /// `XsState::clock` when the latest dash ended (its dash part); -1 = none.
    pub last_end: f32,
    /// The dash that runs: (target, clock at its start).
    pub running: Option<(TargetId, f32)>,
    /// Dashes made so far (for tests and the debug text).
    pub total: u32,
    /// The latest dash's target (the launch's).
    pub last_target: TargetId,
    /// A new dash was requested and has not begun yet: the End of the attack it replaces leaves `running` alone.
    pub starting: bool,
}

/// Dashes an airtime allows.
pub fn charges(count_airslide: u32) -> u32 {
    count_airslide.max(1)
}

/// The per-step part: a landing gives the charges back.
pub(crate) fn tick(_state: &StateData, cx: &mut Ctx) {
    if on_ground(cx.pawn) && cx.pawn.combat.xs.sonic.running.is_none() {
        cx.pawn.combat.xs.sonic.used = 0;
    }
}

/// THE REQUEST (the GUARD button in the controller update, before `TryAirSlide`): true when the press became a
/// Sonic Dash.
pub(crate) fn try_sonic_dash(state: &mut StateData, cx: &mut Ctx, stick_yaw: f32, magnitude: f32) -> bool {
    let s = &*cx.pawn;
    if !has(s, A::SonicDash) || on_ground(s) || magnitude < X.sonic_stick_min || !crate::states::air_slide::gate(state, s, magnitude) {
        return false;
    }
    if s.combat.xs.sonic.used >= charges(s.status.count(A::Airslide)) {
        return false;
    }
    let lock = s.combat.lockon;
    if !lock.locked || lock.current == TargetId::NONE {
        return false;
    }
    // (The lock-on keeps its own target valid: a target in a reaction may drop LOCKABLE for a moment.)
    let Some(t) = cx.target(lock.current).filter(|t| t.flags.has(TargetFlags::ALIVE)) else { return false };
    let d = t.center - s.cmc.location;
    let distance = d.length();
    if distance < X.sonic_range_min || distance > X.sonic_range_max || d.truncate().length_squared() < 1.0 {
        return false;
    }
    let to_target = d.y.atan2(d.x).to_degrees();
    if crate::sora::wrap_degrees(stick_yaw - to_target).to_radians().cos() < X.sonic_stick_cos {
        return false;
    }
    let clock = cx.pawn.combat.xs.clock;
    let sonic = &mut cx.pawn.combat.xs.sonic;
    sonic.used += 1;
    sonic.total += 1;
    sonic.running = Some((t.id, clock));
    sonic.starting = true;
    sonic.last_target = t.id;
    crate::states::attack::xs_forced_attack(cx.pawn, t.id, kind::SONIC_DASH, true);
    true
}

/// The launch request: an Attack press within `sonic.follow_window` of the 2nd dash's end.
pub(crate) fn launch_due(cx: &Ctx) -> bool {
    let sonic = cx.pawn.combat.xs.sonic;
    has(cx.pawn, A::SonicDash) && sonic.used >= 2 && sonic.last_end >= 0.0 && cx.pawn.combat.xs.clock - sonic.last_end <= X.sonic_follow_window
}

/// PlayAttack of 0xE0 / 0xE1: the facing, the gravity, the root scale, the invincibility.
pub(crate) fn begin(cx: &mut Ctx, attack_kind: u8, target: TargetId) {
    let own = cx.pawn.cmc.location;
    if let Some(t) = cx.target(target) {
        let d = t.center - own;
        if d.truncate().length_squared() > 1.0 {
            cx.pawn.yaw = d.y.atan2(d.x).to_degrees();
        }
    }
    let s = &mut *cx.pawn;
    s.combat.xs.sonic.starting = false;
    s.cmc.stop_movement();
    s.cmc.gravity_off = true;
    s.cmc.set_flying();
    if attack_kind == kind::SONIC_DASH {
        s.root_scale = (X.sonic_dash_length / DASH_ROOT_CM, 1.0);
        s.status.damage_invalid = true;
    } else {
        s.combat.xs.sonic.last_end = -1.0;
    }
}

/// The attack state's tick for 0xE0 / 0xE1.
pub(crate) fn attack_tick(cx: &mut Ctx, attack_kind: u8) {
    let time = cx.pawn.anim.time;
    if attack_kind == kind::SONIC_DASH {
        if cx.pawn.combat.xs.sonic.running.is_some() && time >= X.sonic_dash_time {
            let clock = cx.pawn.combat.xs.clock;
            let s = &mut *cx.pawn;
            s.combat.xs.sonic.running = None;
            s.combat.xs.sonic.last_end = clock;
            s.status.damage_invalid = false;
            s.cmc.gravity_off = false;
            s.cmc.stop_movement();
            if s.cmc.is_flying() {
                s.cmc.set_falling();
            }
            s.root_scale = (1.0, 1.0);
        }
    } else if cx.pawn.queue.flags.enable_change && cx.pawn.cmc.gravity_off {
        // The launch: gravity back at its TresEnableChange (1.20 s).
        let s = &mut *cx.pawn;
        s.cmc.gravity_off = false;
        if s.cmc.is_flying() {
            s.cmc.set_falling();
        }
    }
}

/// The attack state's End for 0xE0 / 0xE1.
pub(crate) fn end(cx: &mut Ctx, attack_kind: u8) {
    if attack_kind == kind::SONIC_DASH && !cx.pawn.combat.xs.sonic.starting && cx.pawn.combat.xs.sonic.running.take().is_some() {
        let clock = cx.pawn.combat.xs.clock;
        cx.pawn.combat.xs.sonic.last_end = clock;
    }
    let s = &mut *cx.pawn;
    s.status.damage_invalid = false;
    s.root_scale = (1.0, 1.0);
}
