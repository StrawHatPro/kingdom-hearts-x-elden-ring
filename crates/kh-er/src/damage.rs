//! DAMAGE OUT: how a hit of Sora's reaches an Elden Ring character (IMPL_ER4_STATE.md item 2,
//! IMPL_ER5_STATE.md).
//!
//! - `Mode::Bullet` (THE DEFAULT, `dmg.mode = bullet`): the game's own pipeline. Each hit is a
//!   bullet of the player's with a per-hit attack row (bullet.rs; made in `ErTargets::apply_hit`):
//!   the game applies defences, poise, the hit reaction of the damage level asked for, aggro and
//!   kill credit. A bullet the game refuses, or one that takes nothing off its target within a
//!   few frames (`dmg.fallback`, default on), falls back to the HP write below for that one hit.
//! - `Mode::Hp` (`dmg.mode = hp`) — THE OLD STOPGAP, NOT THE GAME'S PIPELINE: the number is taken
//!   straight off `CSChrDataModule::hp` of the target, and `ChrIns::last_hit_by` is set to the
//!   player's handle. The game notices HP 0 by itself (death animation, runes, drops). It does
//!   NOT do: defences, poise and stagger, hit reactions, guard by the target, aggro, status.
//! - `anim` (`dmg.anim`, off by default; HP mode only): after a hit the target's event module is
//!   asked for an animation id (`CSChrEventModule::request_animation_id`).

use eldenring::cs::{ChrIns, FieldInsHandle};
use kh_core::HitRecord;

use crate::coords::Frame;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Bullet,
    Hp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Route {
    pub mode: Mode,
    /// `dmg.fallback` (default on): a bullet that took nothing is written to the HP directly.
    pub fallback: bool,
    /// `dmg.landing`: the attack value of the knock-down bullet of a landing puppet (default 0).
    pub landing_damage: u16,
    /// `dmg.anim`: the animation id asked of a target that was hit and lives (0 or less: none).
    pub anim: i32,
    /// `dmg.anim_big`: the id for a hit of a blow kind (0 or less: `anim`).
    pub anim_big: i32,
    /// `dmg.credit` (default on): write the player's handle to the target's `last_hit_by`.
    pub credit: bool,
    /// `dmg.off`: hits are accepted and logged but take nothing (for watching the targeting only).
    pub off: bool,
}

impl Default for Route {
    fn default() -> Route {
        Route { mode: Mode::Bullet, fallback: true, landing_damage: 0, anim: 0, anim_big: 0, credit: true, off: false }
    }
}

pub struct Outcome {
    /// Elden Ring HP actually removed.
    pub dealt: i32,
    pub hp_after: i32,
    pub route: &'static str,
}

/// Takes `wanted` Elden Ring hit points straight off `chr` (the stopgap).
///
/// # Safety
/// `chr` must be a complete, live character (targets.rs checked it this frame); game thread.
pub unsafe fn deal_hp(route: Route, chr: &mut ChrIns, attacker: Option<FieldInsHandle>, wanted: i32, big: bool) -> Outcome {
    let before = chr.modules.data.hp;
    if route.off || wanted <= 0 || before <= 0 {
        return Outcome { dealt: 0, hp_after: before, route: "nothing (dmg.off)" };
    }
    let after = (before - wanted).max(0);
    chr.modules.data.as_mut().hp = after;
    if let (true, Some(attacker)) = (route.credit, attacker) {
        chr.last_hit_by = attacker;
    }
    if after > 0 {
        let anim = if big && route.anim_big > 0 { route.anim_big } else { route.anim };
        if anim > 0 && crate::game::plausible(crate::word!(chr.modules.event)) {
            chr.modules.event.as_mut().request_animation_id = anim;
        }
    }
    Outcome { dealt: before - after, hp_after: after, route: "direct HP write (STOPGAP)" }
}

/// The stopgap for one hit record.
///
/// # Safety
/// As `deal_hp`.
pub unsafe fn deal(route: Route, chr: &mut ChrIns, attacker: Option<FieldInsHandle>, wanted: i32, hit: &HitRecord, _frame: Frame) -> Outcome {
    let big = matches!(hit.damage_kind.value(), 2 | 3 | 4 | 7 | 8 | 9 | 10);
    // SAFETY: the caller's contract.
    unsafe { deal_hp(route, chr, attacker, wanted, big) }
}
