//! (XSI) MYRIAD (XS 2.14; ability MYRIAD 268, Promised Consort Radahn): Second Form's finish command (Stun Impact,
//! Sonic Rave or Last Arcanum by form level, already ported) becomes Ultimate Form's finale: the summon of
//! `StyleChange9` (9401: SCSW_CHANGE, the Ultima weapon's sparkle), then `M_9419` (SCSW_MYRIAD multi-hit, SCSW_MYRIAD_END
//! 6.0 VERTICALROLL armour 10 rp 5), and the form ends with it (as KH3's finish does: `form::finish_ended`). Invincible
//! throughout (ours); one use deals at most `spectacle.cap_share` of the target's maximum HP (Q7). The groups ChangeSW,
//! SW_Myriad, SW_Myriad2 are another Keyblade's: their spheres around Sora are ours (`myriad.*`).

use glam::Vec3;

use super::{has, Spectacle, X};
use crate::anim::{AnimId, PlayArgs};
use crate::enums::ETresAbilityKind as A;
use crate::pawn::Ctx;
use crate::states::StateData;

pub const MYRIAD: u8 = 0xE8;

/// The finish command is Myriad (situation.rs `execute`, kinds 171 .. 173 with the finish pending).
pub(crate) fn wanted(cx: &Ctx) -> bool {
    has(cx.pawn, A::Myriad) && cx.pawn.combat.form.finish_pending && crate::combat::form::is_second_form(cx.pawn.combat.form.cmd)
}

pub(crate) fn execute(_state: &mut StateData, cx: &mut Ctx) -> bool {
    let target = cx.pawn.combat.lockon.current;
    crate::states::attack::xs_forced_attack(cx.pawn, target, MYRIAD, false);
    true
}

pub(crate) fn begin(cx: &mut Ctx) {
    cx.pawn.status.damage_invalid = true;
    cx.pawn.combat.xs.spectacle = Some(Spectacle { kind: MYRIAD, ..Spectacle::default() });
}

/// The summon ended: the finale.
pub(crate) fn tick(cx: &mut Ctx) {
    if cx.pawn.anim.name == AnimId::StyleChange9 && cx.pawn.anim.ended {
        cx.pawn.anim.play_with(AnimId::M_9419, PlayArgs::mode(2, -1.0));
    }
}

pub(crate) fn end(cx: &mut Ctx) {
    cx.pawn.status.damage_invalid = false;
    if cx.pawn.combat.xs.spectacle.as_ref().is_some_and(|s| s.kind == MYRIAD) {
        cx.pawn.combat.xs.spectacle = None;
    }
    // The form ends with its finale (StyleFinish End 0x1413CEBD0: ctrl.slot859, the finish effects).
    if crate::combat::form::is_second_form(cx.pawn.combat.form.cmd) {
        crate::combat::form::finish_ended(cx);
        crate::combat::form::end_finish_effects(cx);
    }
}

pub(crate) fn group_shape(cx: &Ctx, group: &str) -> Option<(Vec3, Vec3, f32)> {
    let c = cx.pawn.cmc.location + Vec3::Z * cx.pawn.params().capsule_center_z;
    let r = match group {
        "ChangeSW" => X.myriad_change_radius,
        "SW_Myriad" => X.myriad_radius,
        "SW_Myriad2" => X.myriad_end_radius,
        _ => return None,
    };
    Some((c, c, r))
}
