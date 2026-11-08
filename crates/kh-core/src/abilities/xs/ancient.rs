//! (XSI) ANCIENT LIGHT (XS 2.13; ability ANCIENT_LIGHT 267, Morgott). Second Form's `LM_UNIONDISCODE` finisher
//! (`kh3_system.situation.lmex_ux` "Ancient Light", command 174; KH3 offers it only at level 3 of one Second Form
//! variant, never with the Kingdom Key) outside the form, OUR rule:
//! - with a full formchange gauge (the Second Form offer of `AwardAtMax`) and the lock on a powerful foe (a boss or a
//!   Giant body), the situation slot also offers Ancient Light (slot B; LT switches between it and Second Form);
//! - Y spends the whole gauge on it (that Second Form offer goes) instead of entering the form: attack kind 0xE2, clip
//!   `M_1435` (5.03 s, Sora rises 350 cm): five pillars `LM_UNIONDISCODE` 0.2 TOSS, the pull `LM_UNIONDISCODE2`
//!   0.25 MAGNET, `LM_UNIONDISCODE3` 0.1, the blast `LM_UNIONDISCODE4` 6.0 BLOW;
//! - invincible throughout; one use deals at most `spectacle.cap_share` of the target's maximum HP.
//!
//! The clip's collision groups `L2_Pillar1` .. `L2_Pillar5` and `L2_Dark` belong to the Second Form weapon of
//! another Keyblade (not in the Kingdom Key's component, sheet sora-combat-collisions): their shapes are OURS
//! ([`group_shape`]: pillars around the target, a sphere around Sora). `Range1400` and `ImpactLL` are the Kingdom
//! Key's own.
//!
//! (F-SECOND) THE OFFER BY KEYBLADE (KH3's rule for "Second Form" behind the gauge): AwardAtMax 0x1413F0ED0 offers the HELD
//! Keyblade's own variant of Second Form (weapon type 1 Kingdom Key -> CHANGE_L1, 18 Starlight -> CHANGE_L2 Second Form S,
//! 19 Dead of Night -> CHANGE_L5 Second Form D; every other Keyblade its own formchange, no Second Form). So the offer stands
//! beside whichever Second Form family command (0x94 ..= 0x98) the gauge put in the queue (`forms::second::gauge_offer`), and
//! Y spends that one; with a Keyblade of another form there is no Second Form beside which to offer it. (Inside Second Form S
//! the same command 174 is the form's own finish level 3: combat/situation.rs routes it to the form when a form is held.)

use glam::Vec3;

use super::{has, kind, powerful, Spectacle, X};
use crate::combat::forms::second::gauge_offer; // (F-SECOND)
use crate::combat::situation;
use crate::enums::ETresAbilityKind as A;
use crate::host::TargetId;
use crate::pawn::Ctx;
use crate::states::StateData;

/// `LM_UNIONDISCODE` (ETresCommandKind 174): the command whose HUD name is "Ancient Light".
pub const COMMAND: i32 = 174;


fn offer_wanted(cx: &Ctx) -> bool {
    // (F-SECOND) beside the gauge's Second Form offer of the held Keyblade (was CHANGE_L1 only)
    has(cx.pawn, A::AncientLight) && cx.pawn.combat.form.cmd == 0 && gauge_offer(cx).is_some() && powerful(cx, cx.pawn.combat.lockon.current)
}

/// Every step (outside a form): the offer stands exactly while its conditions hold.
pub(crate) fn reconcile_offer(cx: &mut Ctx) {
    if cx.pawn.combat.form.cmd != 0 {
        return;
    }
    let wanted = offer_wanted(cx);
    let shown = cx.pawn.combat.situation.has(COMMAND);
    if wanted && !shown {
        let time = gauge_offer(cx).and_then(|cmd| cx.pawn.combat.situation.find(cmd)).map_or(-1.0, |(_, e)| if e.remaining > 0.0 { e.remaining } else { -1.0 });
        situation::enqueue(cx, COMMAND, time);
    } else if !wanted && shown {
        situation::delete(cx, COMMAND);
    }
}

/// THE COMMAND (the executor, combat/situation.rs `execute`, kind 174 outside a form): the gauge's offer is spent
/// and the attack starts. False = refused (the entry stays).
pub(crate) fn execute(state: &mut StateData, cx: &mut Ctx) -> bool {
    if !offer_wanted(cx) || !crate::combat::form::state_gate(state, cx) {
        return false;
    }
    let target = cx.pawn.combat.lockon.current;
    if let Some(cmd) = gauge_offer(cx) {
        situation::delete(cx, cmd); // (F-SECOND) the held Keyblade's Second Form offer
    }
    crate::states::attack::xs_forced_attack(cx.pawn, target, kind::ANCIENT_LIGHT, false);
    true
}

/// PlayAttack of 0xE2: invincible, the cap ledger opens.
pub(crate) fn begin(cx: &mut Ctx) {
    cx.pawn.status.damage_invalid = true;
    cx.pawn.combat.xs.spectacle = Some(Spectacle { kind: kind::ANCIENT_LIGHT, ..Spectacle::default() });
}

pub(crate) fn end(cx: &mut Ctx) {
    cx.pawn.status.damage_invalid = false;
    if cx.pawn.combat.xs.spectacle.as_ref().is_some_and(|s| s.kind == kind::ANCIENT_LIGHT) {
        cx.pawn.combat.xs.spectacle = None;
    }
}

/// The shapes of the clip's groups the Kingdom Key does not have (ours): (end 1, end 2, radius), world.
pub(crate) fn group_shape(cx: &Ctx, group: &str, target: TargetId) -> Option<(Vec3, Vec3, f32)> {
    let own = cx.pawn.cmc.location;
    let at = (target != TargetId::NONE).then(|| cx.target(target)).flatten().map_or_else(
        || {
            let a = cx.pawn.yaw.to_radians();
            own + Vec3::new(a.cos(), a.sin(), 0.0) * X.ancient_no_target_ahead
        },
        |t| t.position,
    );
    if let Some(n) = group.strip_prefix("L2_Pillar").and_then(|n| n.parse::<u32>().ok()) {
        // Pillar n of five on a ring around the target (72 degrees apart; the first toward Sora).
        let to_sora = (own - at) * Vec3::new(1.0, 1.0, 0.0);
        let base = to_sora.y.atan2(to_sora.x) + (n.saturating_sub(1) as f32) * std::f32::consts::TAU / 5.0;
        let foot = at + Vec3::new(base.cos(), base.sin(), 0.0) * X.ancient_pillar_ring;
        return Some((foot, foot + Vec3::Z * X.ancient_pillar_height, X.ancient_pillar_radius));
    }
    if group == "L2_Dark" {
        let c = own + Vec3::Z * cx.pawn.params().capsule_center_z;
        return Some((c, c, X.ancient_dark_radius));
    }
    None
}
