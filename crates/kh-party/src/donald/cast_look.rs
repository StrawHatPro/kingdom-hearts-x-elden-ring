//! (FACE) WHERE DONALD LOOKS WHILE HE CASTS. The user (2026-10-22): "Donald should look at his target when he's casting
//! magic not anything else (exception being Zettaflare keep him how he is there)".
//!
//! While a cast is under way (OUR DESIGN over KH3, whose look-at follows the AI controller's focus whatever he does), his
//! head and eyes look at the cast's target and at nothing else:
//! - a spell, hybrid, ultimate, Y spell or Release ([`super::cast::CastRun`], pad and AI alike: both go through
//!   `cast::begin` / `release_gesture`): its target. An enemy: the enemy; an ally spell on a member (Aero Barrier, Ball
//!   Lightning, Cure): that member, Sora included; a point: the point. A Release bursts at the lock-on / soft target, else
//!   his focus enemy (cast.rs `release_aim`), else ahead;
//! - the attune / unattune cast (`Attunement::cast`) and a self-only cast: straight ahead along his aim;
//! - his missiles (the kit's combo steps, air missiles, the Charged Missile): the lock-on target, else the soft target, else
//!   ahead.
//!
//! ZETTAFLARE is left as it is (its channel, its cast, its ending): no cast look.

use glam::Vec3;
use kh_core::host::TargetId;

use crate::ids::MemberId;
use crate::pawn::NpcPawn;
use crate::states::NpcStateId;

use super::cast::{CastTarget, SORA_MEMBER};
use super::spells::{ArcaneLine, SpellRef};

/// What Donald looks at during a cast.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CastLook {
    Enemy(TargetId),
    /// A party member (by MemberId).
    Member(MemberId),
    Sora,
    /// A point (world cm).
    Point(Vec3),
    /// Straight ahead along his aim (a self-only cast, the attune cast, no target).
    Ahead,
}

/// A point closer than this to him (horizontally, cm) is his own place: he looks ahead.
const SELF_RADIUS: f32 = 120.0;

/// The look of the lock-on target, else the soft target, else `fallback`.
fn aim(p: &NpcPawn, fallback: CastLook) -> CastLook {
    match p.pad.as_ref() {
        Some(d) if d.lock != TargetId::NONE => CastLook::Enemy(d.lock),
        Some(d) if d.soft != TargetId::NONE => CastLook::Enemy(d.soft),
        _ => fallback,
    }
}

/// Whether a Zettaflare is under way on him (its channel / cast run, its ending).
pub fn zettaflare(p: &NpcPawn) -> bool {
    p.don_cast.run.is_some_and(|r| matches!(r.spell, SpellRef::Arcane(ArcaneLine::Zettaflare, _))) || p.don_zetta.ending
}

/// The look of the cast under way on `p` (None: no cast, or Zettaflare).
pub fn cast_look(p: &NpcPawn) -> Option<CastLook> {
    if p.status.ko() || zettaflare(p) {
        return None;
    }
    if let Some(r) = p.don_cast.run.filter(|r| r.t < r.end_at) {
        let focus_enemy = match p.focus {
            crate::command::TargetRef::Enemy(t) => CastLook::Enemy(t),
            _ => CastLook::Ahead,
        };
        return Some(match (r.spell, r.target) {
            (SpellRef::Release(_), _) => aim(p, focus_enemy),
            (_, CastTarget::Enemy(t)) => CastLook::Enemy(t),
            (_, CastTarget::Member(m)) if m == SORA_MEMBER => CastLook::Sora,
            (_, CastTarget::Member(m)) if m == p.id => CastLook::Ahead,
            (_, CastTarget::Member(m)) => CastLook::Member(m),
            (_, CastTarget::Point(at)) => {
                let d = at - p.position();
                if Vec3::new(d.x, d.y, 0.0).length() < SELF_RADIUS {
                    CastLook::Ahead
                } else {
                    CastLook::Point(at)
                }
            }
        });
    }
    if p.status.donald.as_ref().is_some_and(|d| d.core.attune.cast.is_some()) {
        return Some(CastLook::Ahead);
    }
    use crate::pad::donald as kit;
    if p.chara == crate::ids::Chara::Donald && matches!(p.state, NpcStateId::Pad(n) if n == kit::STEP || n == kit::AIR || n == kit::CHARGE) {
        return Some(aim(p, CastLook::Ahead));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn face_self_radius_is_his_own_place() {
        assert!(SELF_RADIUS > 50.0 && SELF_RADIUS < 300.0);
    }
}
