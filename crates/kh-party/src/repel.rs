//! (THE USER 2026-11-02: "fix 'Shields don't count as guarding.' and in fact make the character who attacks a shield bounce off
//! playing their animation from KH3. This means Sora, Donald, Goofy and whoever we add in the future") THE FRIENDS' BOUNCE OFF
//! A SHIELD: ONE PATH FOR EVERY PARTY MEMBER.
//!
//! - WHETHER: KH3's classification (kh-core `combat::classify`, the same function Sora's hits run) with the victim's body as
//!   the attacker meets it (`TargetBody::seen_from`: a host's shield guards only from inside its arc). A guarded swing into a
//!   host's shield gives the attacker REFLECT (`classify`: a shield repels every swing it guards, the friends' "no clash"
//!   rows too); KH3's own REFLECT / GUARD_REFLECT results (a REFLECT1 body, a clash) repel as they always did.
//! - WHO: only the member's OWN SWING bounces him: a KH3 clip's attack window (`actions::deliver`), a kit shape that rides his
//!   body (Goofy's shoves, counters and Arts: `goofy_shots::Area::follow`), Donald's Wand Bonk (his kit's one physical shape).
//!   A shot that flies on its own (a spell, a missile, a thrown shield) is guarded but never bounces its caster (KH3: a
//!   projectile is not the Attack state's own primitive, kh-core combat/hit.rs).
//! - WHAT HE PLAYS: KH3's TresNpcState_Reflect (`NpcPawn::begin_reflect`): the row `repel.clip` / `repel.air_clip` of HIS OWN
//!   anim set (AttackReflect / AttackAirReflect: every KH3 set has them), else `repel.fallback_clip` (sheet party-hurt), the
//!   knock back from the shield, the command and the combo dropped; first the impact's hold (`repel.hold`). A member added
//!   later gets all of it from his set and these rows, without code.
//! - Sora is the same rule in kh-core: the recoil record of his Attack state -> TresPlayerState_Reflect (AttackReflect).

use crate::hurt::HP;
use crate::pawn::NpcPawn;
use glam::Vec3;
use kh_core::host::ReactionResult;

/// The attacker's result repels him (KH3's on-contact rule of the friends, ATresNpcPawnBase notify slot 15 0x141271720:
/// REFLECT 3 / GUARD_REFLECT 4; GUARD 5 does not).
pub fn repels(attacker: ReactionResult) -> bool {
    matches!(attacker, ReactionResult::Reflect | ReactionResult::GuardReflect)
}

/// The bounce clip of a set (`has` = the set has that clip): the air row in the air when the set has it, else the ground
/// row, else the fallback row; `None`: the set has none of them (the push alone).
pub fn clip(has: impl Fn(&str) -> bool, air: bool) -> Option<&'static str> {
    [(air, HP.repel_air_clip), (true, HP.repel_clip), (true, HP.repel_fallback_clip)].into_iter().find(|(want, c)| *want && !c.is_empty() && has(c)).map(|(_, c)| c)
}

/// THE ONE CALL after a member's own swing was classified: the result repels him -> the Reflect state next step, knocked
/// away from `from` (the shield's body), after the impact's hold. True when he bounces.
pub fn after_swing(p: &mut NpcPawn, attacker: ReactionResult, from: Vec3) -> bool {
    if !repels(attacker) {
        return false;
    }
    p.reflect_from = Some(from);
    p.reflect_hold = HP.repel_hold.max(0.0);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bounce_clip_is_the_sets_own_reflect_row_then_the_fallback() {
        let all = |c: &str| matches!(c, "AttackReflect" | "AttackAirReflect" | "M_0201");
        assert_eq!(clip(all, false), Some("AttackReflect"));
        assert_eq!(clip(all, true), Some("AttackAirReflect"));
        let ground_only = |c: &str| matches!(c, "AttackReflect" | "M_0201");
        assert_eq!(clip(ground_only, true), Some("AttackReflect"), "no air clip: the ground one");
        assert_eq!(clip(|c: &str| c == "M_0201", false), Some("M_0201"), "no reflect clip at all: the fallback row");
        assert_eq!(clip(|_: &str| false, true), None);
        // both party sets in the pack have their own
        for chara in [crate::ids::Chara::Donald, crate::ids::Chara::Goofy] {
            let set = crate::tables::anim_set(chara);
            assert_eq!(clip(|c: &str| set.get(c).is_some(), false), Some("AttackReflect"), "{chara:?}");
            assert_eq!(clip(|c: &str| set.get(c).is_some(), true), Some("AttackAirReflect"), "{chara:?}");
        }
    }

    #[test]
    fn only_reflect_results_repel() {
        assert!(repels(ReactionResult::Reflect) && repels(ReactionResult::GuardReflect));
        for r in [ReactionResult::Guard, ReactionResult::NoReaction, ReactionResult::Damage, ReactionResult::None] {
            assert!(!repels(r), "{r:?}");
        }
    }
}
