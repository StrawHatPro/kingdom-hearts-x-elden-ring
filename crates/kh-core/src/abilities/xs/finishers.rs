//! (XSI) THE XS GROUND FINISHERS (OUR choice rule on KH3's clips and rows; the clips of other anim sets come from the
//! overlay of tools/xsi_pack.js):
//! - SPIRAL RUSH (XS 2.3, 258; level A 12 / Tree Sentinel): the target `spiral.range_min` .. `spiral.range_max` away ->
//!   Light Form's drill charge `M_M416` (kind 0xE3: SCLF_SPIRALRUSH_START / _ / _END 1.0 VERTICALROLL).
//! - ZANTETSUKEN (XS 2.4, 259; Rellana): Attack held `zantetsuken.hold_min` or more when the finisher begins ->
//!   `M_M411` (0xE4); its draw stance (`zantetsuken.stance_time`) is held while Attack stays down (at most
//!   `zantetsuken.hold_max`); a charge of `zantetsuken.full_charge` gives power x`full_power` and reaction power
//!   `full_rp`.
//! - EVENT HORIZON (XS 2.5, 260; Metyr): takes the ground Magic Flash's place (kind 7 -> `M_M413`, 0xE5): the KH3
//!   finisher choice decides when; its ring (group LF_EventHolizon, ours) has `event_horizon.radius`.
//! - GIANT STAMP (XS 2.6, 261; Elemer): against a STRONG (Heavy / Giant) body -> Hyper Hammer's `M_3416` (0xE6),
//!   invincible `giant_stamp.invincible_from` .. `_to`, the shockwave StampL (ours) `giant_stamp.radius` in front.
//! - CRIMSON BLAST (XS 2.12, Riskcharge's): the ground finisher while a Riskcharge runs (rage.rs).
//! Precedence (ours): Crimson Blast, Giant Stamp, Zantetsuken, Spiral Rush, Event Horizon. Air finishers and Second
//! Form's (ComboLimit) are KH3's.

use glam::Vec3;

use super::{has, kind, X};
use crate::enums::ETresAbilityKind as A;
use crate::host::{BodyReaction, TargetId};
use crate::pawn::Ctx;

pub const SPIRAL_RUSH: u8 = 0xE3;
pub const ZANTETSUKEN: u8 = 0xE4;
pub const EVENT_HORIZON: u8 = 0xE5;
pub const GIANT_STAMP: u8 = 0xE6;
pub const CRIMSON_BLAST: u8 = 0xED;
/// KH3's ground Magic Flash.
pub const MAGIC_FLASH: u8 = 7;

/// Zantetsuken while it runs: the charge (seconds the stance was held) and whether the stance was let go.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Zanto {
    pub charge: f32,
    pub released: bool,
}

/// THE CHOICE (states/attack.rs Begin, after KH3's `Decide`): the decided kind, or an XS finisher.
pub(crate) fn refine(cx: &Ctx, decided: u8, finish: bool, air: bool, limit: bool, target: TargetId) -> u8 {
    if !finish || air || limit {
        return decided;
    }
    // (FX1) AI SORA: A FRIEND LEADER'S ACCEPTED TEAM MOVE IS NEVER REPLACED. While a friend leads (`sora_is_ai`) his own
    // combos roll no unison, so rule 7's kinds 0x3F..0x43 can only be the leader's latch, whose pad was told "yes":
    // before, a target in Spiral Rush's band (or Giant Stamp's, a Rage charge, a held A for Zantetsuken) turned that
    // fusion into his own XS move and the friend got nothing. With Sora leading, his XS finishers keep their place over
    // his own unison roll as before (XSI; tour 2's second Zantetsuken at 7.75 s is that case).
    if cx.pawn.combat.friend.sora_is_ai && (0x3F..=0x43).contains(&decided) {
        return decided;
    }
    let s = &*cx.pawn;
    let xs = &s.combat.xs;
    // DEBUG: a forced XS finisher (captures), if its ability is equipped.
    if let Some(k) = xs.debug_finisher {
        let ability = match k {
            SPIRAL_RUSH => A::SpiralRush,
            ZANTETSUKEN => A::Zantetsuken,
            EVENT_HORIZON => A::EventHorizon,
            GIANT_STAMP => A::GiantStamp,
            _ => A::Riskcharge,
        };
        if has(s, ability) {
            return k;
        }
    }
    if has(s, A::Riskcharge) && xs.rage.charge_until > xs.clock {
        return CRIMSON_BLAST;
    }
    if has(s, A::GiantStamp) && target != TargetId::NONE && cx.host.targets.body(target, 0).reaction == BodyReaction::Strong {
        return GIANT_STAMP;
    }
    if has(s, A::Zantetsuken) && xs.attack_held >= X.zantetsuken_hold_min {
        return ZANTETSUKEN;
    }
    if has(s, A::SpiralRush) {
        if let Some(t) = (target != TargetId::NONE).then(|| cx.target(target)).flatten() {
            let d = (t.center - s.cmc.location).truncate().length();
            if (X.spiral_range_min..=X.spiral_range_max).contains(&d) {
                return SPIRAL_RUSH;
            }
        }
    }
    if has(s, A::EventHorizon) && decided == MAGIC_FLASH {
        return EVENT_HORIZON;
    }
    decided
}

pub(crate) fn begin(cx: &mut Ctx, attack_kind: u8) {
    cx.pawn.combat.xs.debug_finisher = None;
    match attack_kind {
        ZANTETSUKEN => cx.pawn.combat.xs.zanto = Some(Zanto::default()),
        CRIMSON_BLAST => cx.pawn.combat.xs.rage.blast_fired = false,
        _ => {}
    }
}

pub(crate) fn tick(cx: &mut Ctx, attack_kind: u8, dt: f32) {
    let time = cx.pawn.anim.time;
    match attack_kind {
        ZANTETSUKEN => {
            let Some(mut z) = cx.pawn.combat.xs.zanto else { return };
            let held = cx.pawn.input.pad.held.has(crate::input::PadButton::ATTACK);
            if !z.released && time >= X.zantetsuken_stance_time {
                if held && z.charge < X.zantetsuken_hold_max {
                    cx.pawn.anim.hold = true;
                    z.charge += dt;
                } else {
                    cx.pawn.anim.hold = false;
                    z.released = true;
                }
            }
            cx.pawn.combat.xs.zanto = Some(z);
        }
        GIANT_STAMP => {
            cx.pawn.status.damage_invalid = (X.giant_stamp_invincible_from..X.giant_stamp_invincible_to).contains(&time);
        }
        CRIMSON_BLAST => super::rage::blast_tick(cx),
        _ => {}
    }
}

pub(crate) fn end(cx: &mut Ctx, attack_kind: u8) {
    match attack_kind {
        ZANTETSUKEN => {
            cx.pawn.combat.xs.zanto = None;
            cx.pawn.anim.hold = false;
        }
        GIANT_STAMP => cx.pawn.status.damage_invalid = false,
        _ => {}
    }
}

/// A full Zantetsuken charge (for the hit hooks).
pub(crate) fn full_charge(cx: &Ctx) -> bool {
    cx.pawn.combat.xs.zanto.is_some_and(|z| z.charge >= X.zantetsuken_full_charge)
}

/// The groups of these clips the Kingdom Key's component lacks (ours).
pub(crate) fn group_shape(cx: &Ctx, group: &str) -> Option<(Vec3, Vec3, f32)> {
    let own = cx.pawn.cmc.location;
    let a = cx.pawn.yaw.to_radians();
    let forward = Vec3::new(a.cos(), a.sin(), 0.0);
    let centre = own + Vec3::Z * cx.pawn.params().capsule_center_z;
    match group {
        "LF_EventHolizon" => Some((centre, centre, X.event_horizon_radius)),
        "StampL" => {
            let p = centre + forward * X.giant_stamp_ahead;
            Some((p, p, X.giant_stamp_radius))
        }
        _ => None,
    }
}

/// The kinds of this file (for the dispatch of mod.rs).
pub(crate) fn owns(attack_kind: u8) -> bool {
    matches!(attack_kind, SPIRAL_RUSH | ZANTETSUKEN | EVENT_HORIZON | GIANT_STAMP | CRIMSON_BLAST)
}

#[allow(dead_code)]
const _: u8 = kind::DIVE;
