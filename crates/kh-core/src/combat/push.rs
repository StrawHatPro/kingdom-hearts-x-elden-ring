//! The soft body push between "push bodies" (combat spec 6.5 / strand B.4): the capsules of
//! pawns never block each other; what separates them is `UTresBodyCollComponent` 0x140637430,
//! which sums, over every overlapping pair of push bodies, `w x penetrationVector(mine, other)`
//! with a weight by the two characters' `m_BodyPushPower` levels.
//!
//! EVIDENCE: the solver was read in DECOMPILED C ONLY (verdict same-partial); the weights are
//! sheet `sora-combat-push-weight` ("inferred"). Implemented as read and marked:
//! - Sora's push body [A]: capsule radius 50, half height 90 at mesh (0, 0, 90); level `PPL_50` (4);
//! - weights: my level lower than the other's 1.0, equal 0.5, higher 0;
//! - the penetration vector is taken as the minimum translation between two upright capsules
//!   (0x144524AB0 was not read: full MTD or centre direction is open question 1 of strand B);
//! - the push is flattened to XY (`m_bBodyCollPushVectorConstraintXY`: the flag's value for Sora
//!   was not read; an XY push keeps him on the floor) and applied as ONE extra sweep after the
//!   movement of the tick (the game adds it to the same sweep as velocity and root motion:
//!   Walking / Falling as a velocity `push / dt`, Flying as a displacement): world geometry
//!   still blocks it.
//!
//! FOURTH PASS (2026-10-12, P3 H5; decompiled C of the whole solver, the weight constants are the
//! immediates 0.5 / 1.0 / 0.0): the penetration vector IS the full minimum translation -
//! 0x144524AB0 passes both primitives' world transforms to 0x144524150 (SQEX_PhysXCollision.cpp:
//! the PhysX penetration query), which returns a direction and a depth, and the push adds
//! `direction x depth x w`; an OTHER body whose owner is in its "unpushable" state
//! (0x1405AD300 of the owner's movement interface) weighs 1.0 whatever the levels; an other that
//! is not a character pawn weighs 0.5. A game-object flag (object 0x141522B30, byte +0x84C)
//! switches to a sequential variant for player pawns (each pair measured with his transform
//! already moved by the push so far): which mode sets it was not found. The push level during a
//! formchange finish is 8: the command begin event calls `MyBodyColl.vt+0x720(8)`
//! (0x14150A339) and the end event's pawn.vt+0x23B8 restores the base parameter's level.
//!
//! The host supplies each target's push body (`Targets::push_body`); without one Sora passes
//! through the target, exactly as the capsules do.

use glam::Vec3;

use super::tables::P;
use crate::host::{PushBody, TargetFlags, TargetInfo};
use crate::pawn::Ctx;

/// Sora's push body: half height (the radius is `body_push.shape_radius`) and its centre above
/// the actor origin.
pub const HALF_HEIGHT: f32 = 90.0;
pub const CENTRE_Z: f32 = 90.0;

/// The weight of a pair (sheet `sora-combat-push-weight`): level 9 (`PPL_MAX`) is never pushed.
pub fn weight(mine: u8, other: u8) -> f32 {
    if mine >= 9 {
        0.0
    } else if mine < other {
        1.0
    } else if mine == other {
        0.5
    } else {
        0.0
    }
}

/// The vector that moves upright capsule `a` out of upright capsule `b` (zero when apart).
pub fn penetration(a: &PushBody, b: &PushBody) -> Vec3 {
    let (sa, sb) = ((a.half_height - a.radius).max(0.0), (b.half_height - b.radius).max(0.0));
    // The closest points of the two vertical axes.
    let (alo, ahi, blo, bhi) = (a.center.z - sa, a.center.z + sa, b.center.z - sb, b.center.z + sb);
    let (za, zb) = if ahi < blo {
        (ahi, blo)
    } else if bhi < alo {
        (alo, bhi)
    } else {
        let z = (alo.max(blo) + ahi.min(bhi)) * 0.5;
        (z, z)
    };
    let d = Vec3::new(a.center.x - b.center.x, a.center.y - b.center.y, za - zb);
    let depth = a.radius + b.radius - d.length();
    if depth <= 0.0 {
        return Vec3::ZERO;
    }
    // Coincident axes: out along +X (any direction separates).
    let direction = if d.length_squared() < 1.0e-8 { Vec3::X } else { d.normalize() };
    direction * depth
}

/// His push body touches the push body of `target` (the pair is on the body-collision
/// component's overlap list: 0x140637430; each pair on it is told to both pawns through the notify
/// interface's slot 8 by 0x140633EE0, which is how the Attack state's rush-suppression handler
/// 0x141343160 is reached: P3 H4).
pub(crate) fn touching(cx: &Ctx, target: crate::host::TargetId) -> bool {
    if cx.pawn.actions.body_collision_off {
        return false;
    }
    let mine = PushBody { center: cx.pawn.cmc.location + Vec3::Z * CENTRE_Z, radius: P.body_push_shape_radius, half_height: HALF_HEIGHT, level: P.body_push_level_sora as u8 };
    match cx.host.targets.push_body(target) {
        Some(other) => penetration(&mine, &other) != Vec3::ZERO,
        None => false,
    }
}

/// The push of this tick (movement component slot 414 0x14057ABD0: `+0xBA8`).
pub(crate) fn compute(cx: &Ctx) -> Vec3 {
    // (ACTIONS IMPLEMENTER) pawn vt+0x1010(0): Risk Dodge switches his pawn-to-pawn body off.
    if cx.pawn.actions.body_collision_off {
        return Vec3::ZERO;
    }
    let mine = PushBody { center: cx.pawn.cmc.location + Vec3::Z * CENTRE_Z, radius: P.body_push_shape_radius, half_height: HALF_HEIGHT, level: P.body_push_level_sora as u8 };
    let mut targets: Vec<TargetInfo> = Vec::new();
    cx.targets(&mut targets);
    let mut push = Vec3::ZERO;
    for t in &targets {
        if !t.flags.has(TargetFlags::ALIVE) {
            continue;
        }
        let Some(other) = cx.host.targets.push_body(t.id) else { continue };
        push += weight(mine.level, other.level) * penetration(&mine, &other);
    }
    push.z = 0.0;
    push
}

#[cfg(test)]
mod tests {
    use super::*;

    // Combat spec 6.5 (strand B.4, decompiled C): the weights and the separation vector.
    #[test]
    fn weights_by_level_and_the_separation_of_two_capsules() {
        assert_eq!((weight(4, 5), weight(4, 4), weight(4, 3), weight(9, 9)), (1.0, 0.5, 0.0, 0.0));
        let body = |x: f32, z: f32| PushBody { center: Vec3::new(x, 0.0, z), radius: 50.0, half_height: 90.0, level: 4 };
        // 70 cm apart, radii 50 + 50: 30 cm of overlap, out along -X.
        assert!((penetration(&body(0.0, 90.0), &body(70.0, 90.0)) - Vec3::new(-30.0, 0.0, 0.0)).length() < 1.0e-4);
        assert_eq!(penetration(&body(0.0, 90.0), &body(100.0, 90.0)), Vec3::ZERO);
        // One above the other, caps overlapping by 10: out along Z.
        let p = penetration(&body(0.0, 90.0 + 170.0), &body(0.0, 90.0));
        assert!((p - Vec3::new(0.0, 0.0, 10.0)).length() < 1.0e-4, "{p}");
    }
}
