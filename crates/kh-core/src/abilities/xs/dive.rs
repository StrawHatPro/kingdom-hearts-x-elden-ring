//! (XSI) DIVING STRIKE (XS 2.1; ability DIVING_STRIKE 256, level 22 on all three paths). KH3's dive fall
//! (Jump sub-steps 7 - 9, `m_bEnableDiveFall`, off in every level of the game) and its attack `DiveFallAttack`
//! (kind 0x35, clip 1050, rows DIVEFALL_ATTACK / DIVEFALL_ATTACK_END) with OUR rules:
//! - the head-first pose: after `dive.pose_fall_time` of free fall with `dive.pose_min_height` or more below him,
//!   Jump plays `DiveFallStart` (0048) into the `DiveFallLoop` blend space (0049 / 0049L / 0049R), steered at
//!   `dive.pose_steer_speed` (states/jump.rs sub-steps 7 / 8); B (the glide: KH3's own gate takes sub-step 8) or X
//!   (the air dodge) leave it;
//! - the strike: Attack while falling (never while rising) with `dive.min_drop` below him and an enemy in KH3's
//!   cone (`MyDiveFallLockonRangeXY` 1000 horizontally, 0 .. `MyDiveFallLockonLimitZ` 3500 below): the lock-on target
//!   first, else the nearest in the cone. `1050` plays from its start; at `dive.hold_time` its pose is held while he
//!   homes on the target at `dive.home_speed`; on contact (or the ground) the rest plays: the BEAT hit, the shock
//!   ring, the 5.0 VERTICALROLL END hit, +`dive.bonus_per_step` per full `dive.bonus_step` fallen beyond
//!   `dive.min_drop` (at most `dive.bonus_max`). Invincible from the press until the END hit's window closes.

use glam::Vec3;

use super::{has, kind, X};
use crate::enums::ETresAbilityKind as A;
use crate::host::{TargetFlags, TargetId, TargetInfo};
use crate::pawn::{Ctx, Pawn};
use crate::states::common::{ground_probe, on_ground};
use crate::states::StateData;

/// The END hit's window closes here in 1050 (DIVEFALL_ATTACK_END 1.133 - 1.400 s): the invincibility ends.
pub const END_WINDOW_CLOSE: f32 = 1.4;

/// One Diving Strike while it runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiveRun {
    pub target: TargetId,
    /// Actor Z at the press.
    pub start_z: f32,
    /// cm fallen from the press to the release (the END hit's bonus).
    pub fallen: f32,
    /// The pose of `dive.hold_time` is held (homing).
    pub holding: bool,
    pub released: bool,
    /// Seconds of homing.
    pub homed: f32,
}

/// The END hit's power bonus for `fallen` cm.
pub fn fall_bonus(fallen: f32) -> f32 {
    if fallen <= X.dive_min_drop || X.dive_bonus_step <= 0.0 {
        return 0.0;
    }
    (((fallen - X.dive_min_drop) / X.dive_bonus_step).floor() * X.dive_bonus_per_step).min(X.dive_bonus_max)
}

/// The target in KH3's dive cone (combat spec 7.4: the candidates below Sora within `MyDiveFallLockonRangeXY`
/// horizontally and `MyDiveFallLockonLimitZ` below): the lock-on target when it is in the cone, else the nearest.
pub(crate) fn cone_target(cx: &Ctx) -> Option<TargetId> {
    let own = cx.pawn.cmc.location;
    let in_cone = |t: &TargetInfo| {
        let d = t.center - own;
        t.flags.has(TargetFlags::ALIVE | TargetFlags::LOCKABLE) && d.truncate().length() <= X.dive_lockon_xy && d.z <= 0.0 && d.z >= -X.dive_lockon_limit_z
    };
    let lock = cx.pawn.combat.lockon.current;
    if let Some(t) = (lock != TargetId::NONE).then(|| cx.target(lock)).flatten().filter(in_cone) {
        return Some(t.id);
    }
    let mut all = Vec::new();
    cx.targets(&mut all);
    all.into_iter().filter(|t| in_cone(t)).min_by(|a, b| (a.center - own).length_squared().total_cmp(&(b.center - own).length_squared())).map(|t| t.id)
}

/// THE STRIKE REQUEST (`TryAttack` out of JUMP, states/attack.rs): `Some(target)` when the attack press is a
/// Diving Strike.
pub(crate) fn strike_target(state: &StateData, cx: &Ctx) -> Option<TargetId> {
    let StateData::Jump(_) = state else { return None };
    let s = &*cx.pawn;
    if !has(s, A::DivingStrike) || on_ground(s) || s.cmc.velocity.z > 0.0 || !falling(state) {
        return None;
    }
    let (hit, distance, _) = ground_probe(s, cx.world, X.dive_lockon_limit_z.max(X.dive_min_drop) + 100.0);
    if hit && distance < X.dive_min_drop {
        return None;
    }
    cone_target(cx)
}

/// The Jump state is in a falling sub-step (3, 4, 7, 8).
pub(crate) fn falling(state: &StateData) -> bool {
    matches!(crate::states::jump::sub_of(state), Some(3 | 4 | 7 | 8))
}

/// The head-first pose may start (Jump sub-step 4): the ability, the fall time and the height below.
pub(crate) fn pose_ok(s: &Pawn, fall_time: f32, probe_distance: f32) -> bool {
    has(s, A::DivingStrike) && fall_time >= X.dive_pose_fall_time && probe_distance >= X.dive_pose_min_height && s.cmc.velocity.z <= 0.0
}

/// PlayAttack of kind 0x35 under Diving Strike: no traction, no gravity, invincible; the run is recorded.
pub(crate) fn begin(cx: &mut Ctx, target: TargetId) {
    let s = &mut *cx.pawn;
    s.cmc.gravity_off = true;
    s.cmc.stop_movement();
    s.cmc.set_flying();
    s.status.damage_invalid = true;
    s.combat.xs.dive = Some(DiveRun { target, start_z: s.cmc.location.z, fallen: 0.0, holding: false, released: false, homed: 0.0 });
}

/// The attack state's tick for kind 0x35 (after its traction).
pub(crate) fn tick(cx: &mut Ctx, dt: f32) {
    let Some(mut run) = cx.pawn.combat.xs.dive else { return };
    let time = cx.pawn.anim.time;
    if !run.released && !run.holding && time >= X.dive_hold_time {
        run.holding = true;
        cx.pawn.anim.hold = true;
    }
    if run.holding {
        run.homed += dt;
        let own = cx.pawn.cmc.location;
        let target = (run.target != TargetId::NONE).then(|| cx.target(run.target)).flatten().filter(|t| t.flags.has(TargetFlags::ALIVE));
        // The aim: the target's centre; without a target (gone): straight down.
        let aim = target.map_or(own - Vec3::Z * 1000.0, |t| t.center);
        let to = aim - own;
        let (hit, ground, _) = ground_probe(cx.pawn, cx.world, X.dive_contact + 50.0);
        let reached = target.is_some_and(|t| to.length() <= X.dive_contact + t.radius) || (hit && ground <= X.dive_contact) || on_ground(cx.pawn) || run.homed >= X.dive_home_timeout;
        if reached {
            run.holding = false;
            run.released = true;
            run.fallen = (run.start_z - own.z).max(0.0);
            cx.pawn.anim.hold = false;
            cx.pawn.cmc.stop_movement();
        } else {
            let dir = to.normalize_or_zero();
            cx.pawn.cmc.velocity = dir * X.dive_home_speed;
            if dir.truncate().length_squared() > 1.0e-6 {
                cx.pawn.yaw = dir.y.atan2(dir.x).to_degrees();
            }
        }
    }
    if run.released && time >= END_WINDOW_CLOSE {
        cx.pawn.status.damage_invalid = false;
    }
    cx.pawn.combat.xs.dive = Some(run);
}

/// The attack state's End for kind 0x35.
pub(crate) fn end(cx: &mut Ctx) {
    if cx.pawn.combat.xs.dive.take().is_some() {
        cx.pawn.anim.hold = false;
        cx.pawn.status.damage_invalid = false;
        cx.pawn.cmc.gravity_off = false;
        if cx.pawn.cmc.is_flying() {
            cx.pawn.cmc.set_falling();
        }
    }
}

/// The attack kind Diving Strike forces.
pub const KIND: u8 = kind::DIVE;
