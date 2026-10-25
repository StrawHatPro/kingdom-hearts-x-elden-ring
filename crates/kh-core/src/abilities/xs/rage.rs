//! (XSI) RAGE FORM'S MOVES OUTSIDE THE FORM (OUR rules on KH3's AnimSet13 clips and rows):
//! - RAGE RAID (XS 2.11, 265; Mohg, the Omen): after a successful Block against a body whose revenge runs
//!   (`Targets::in_revenge`), or a Dodge Roll, Risk Dodge or Air Slide begun with such a body within `rage_raid.near`,
//!   the reaction command Raid (`ANTIRAID` 13, "Raid") is offered for `rage_raid.offer_time`; Y or Attack -> `M_D420`
//!   (kind 0xEB: AN_COUNTER x n, AN_COUNTER_END 1.0 TOSS armour 10 rp 5), invincible to `rage_raid.invincible_to`; the
//!   END hit ends the body's revenge (`Targets::end_revenge`).
//! - RISKCHARGE (XS 2.12, 266; Mohg, Lord of Blood): at `riskcharge.hp_share` of max HP or less the situation slot offers
//!   Riskcharge (179, `kh3_system.situation.riskcharge`); Y -> `M_D419` (0xEC, invincible): `riskcharge.cost` of the
//!   current HP (never lethal), the charge level +1 (at most `riskcharge.max_level`); for `riskcharge.duration` the
//!   ground finisher is Crimson Blast `M_D417` (0xED) whose shot at `riskcharge.blast_time` takes the row of the level
//!   (AN_CRIMSONBLAST_RC1 .. RC3); one use per `riskcharge.cooldown`.
//! - Q4: the red Rage aura (`vp_ant_bdy_ora00`) on Sora while Raid plays and while a Riskcharge lasts.

use glam::Vec3;

use super::{has, X};
use crate::combat::situation;
use crate::enums::ETresAbilityKind as A;
use crate::host::{Attach, EffectHandle, EffectId, EffectSpawn, TargetId};
use crate::pawn::Ctx;
use crate::states::{StateData, StateId};

pub const RAGE_RAID: u8 = 0xEB;
pub const RISKCHARGE: u8 = 0xEC;
/// The situation command Riskcharge.
pub const RISKCHARGE_COMMAND: i32 = 179;
pub const AURA: &str = "vp_ant_bdy_ora00";

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RageState {
    /// Raid is offered until this clock (0 = not), against this body.
    pub raid_until: f32,
    pub raid_target: TargetId,
    /// Riskcharge: the charge level (0 .. max), the clock its finisher lasts until, the clock of its next use.
    pub level: u8,
    pub charge_until: f32,
    pub cooldown_until: f32,
    /// Crimson Blast's shot left.
    pub blast_fired: bool,
    /// The aura while it shows.
    pub aura: Option<EffectHandle>,
    /// The state of the last step (to see a dodge begin).
    pub last_state: Option<StateId>,
    /// Raids and Riskcharges so far (for tests).
    pub raids: u32,
    pub charges: u32,
}

fn aura(cx: &mut Ctx, on: bool) {
    let shown = cx.pawn.combat.xs.rage.aura;
    match (on, shown) {
        (true, None) => {
            let h = cx.spawn_effect(EffectSpawn { id: EffectId(crate::notify::notify_asset_id(AURA)), name: AURA, at: Attach::Sora { bone: Some("koshi"), offset: Vec3::ZERO }, rotation: Vec3::ZERO, scale: 1.0, persistent: true });
            cx.pawn.combat.xs.rage.aura = Some(h);
        }
        (false, Some(h)) => {
            cx.stop_effect(h, false);
            cx.pawn.combat.xs.rage.aura = None;
        }
        _ => {}
    }
}

/// A body near Sora whose revenge runs.
fn revenge_body_near(cx: &Ctx) -> Option<TargetId> {
    let own = cx.pawn.cmc.location;
    let mut all = Vec::new();
    cx.targets(&mut all);
    all.into_iter().filter(|t| (t.position - own).length() <= X.rage_raid_near && cx.host.targets.in_revenge(t.id)).min_by(|a, b| (a.position - own).length_squared().total_cmp(&(b.position - own).length_squared())).map(|t| t.id)
}

fn offer_raid(cx: &mut Ctx, target: TargetId) {
    let clock = cx.pawn.combat.xs.clock;
    cx.pawn.combat.xs.rage.raid_until = clock + X.rage_raid_offer_time;
    cx.pawn.combat.xs.rage.raid_target = target;
    situation::set_reaction(cx, X.rage_raid_command as i32, X.rage_raid_offer_time);
}

/// A guarded hit (states/guard.rs `OnGuardedHit`, success): the causer in its revenge -> Raid.
pub(crate) fn on_guarded(cx: &mut Ctx, causer: TargetId) {
    if has(cx.pawn, A::RageRaid) && causer != TargetId::NONE && cx.host.targets.in_revenge(causer) {
        offer_raid(cx, causer);
    }
}

/// THE RAID OFFERED NOW (TryAttack's question): its body.
pub(crate) fn raid_due(cx: &Ctx) -> Option<TargetId> {
    let r = cx.pawn.combat.xs.rage;
    (has(cx.pawn, A::RageRaid) && r.raid_until > cx.pawn.combat.xs.clock).then_some(r.raid_target)
}

/// Every step: the dodges that count for Raid, the Riskcharge offer, the end of a charge.
pub(crate) fn tick(state: &StateData, cx: &mut Ctx) {
    let clock = cx.pawn.combat.xs.clock;
    // A dodge begun against a revenge.
    let id = state.id();
    let last = cx.pawn.combat.xs.rage.last_state.replace(id);
    if last != Some(id) && matches!(id, StateId::Dodgeroll | StateId::Riskdodge | StateId::AirSlide) && has(cx.pawn, A::RageRaid) {
        if let Some(t) = revenge_body_near(cx) {
            offer_raid(cx, t);
        }
    }
    // The Raid offer runs out.
    let r = cx.pawn.combat.xs.rage;
    if r.raid_until > 0.0 && r.raid_until <= clock {
        cx.pawn.combat.xs.rage.raid_until = 0.0;
        situation::clear_reaction(cx, X.rage_raid_command as i32);
    }
    // The charge runs out: its aura goes (unless Raid plays).
    if r.charge_until > 0.0 && r.charge_until <= clock {
        cx.pawn.combat.xs.rage.charge_until = 0.0;
        if !matches!(state.attack().map(|a| a.kind), Some(RAGE_RAID)) {
            aura(cx, false);
        }
    }
    // Out of danger (HP above the offer's share): the charge level starts again from 0 (ours: the levels are a last stand's).
    if cx.pawn.status.hp as f32 > cx.pawn.status.max_hp() as f32 * X.riskcharge_hp_share {
        cx.pawn.combat.xs.rage.level = 0;
    }
    // The Riskcharge offer stands exactly while its conditions hold (outside a form: slot B is the form's there).
    let s = &*cx.pawn;
    let wanted = has(s, A::Riskcharge) && s.combat.form.cmd == 0 && s.status.hp > 0 && (s.status.hp as f32) <= s.status.max_hp() as f32 * X.riskcharge_hp_share && r.cooldown_until <= clock;
    let shown = s.combat.situation.has(RISKCHARGE_COMMAND);
    if wanted && !shown {
        situation::enqueue(cx, RISKCHARGE_COMMAND, -1.0);
    } else if !wanted && shown && cx.pawn.combat.form.cmd == 0 {
        situation::delete(cx, RISKCHARGE_COMMAND);
    }
}

/// The command Riskcharge (situation.rs `execute`, 179 outside a form).
pub(crate) fn execute_riskcharge(state: &mut StateData, cx: &mut Ctx) -> bool {
    let s = &*cx.pawn;
    let clock = s.combat.xs.clock;
    if !has(s, A::Riskcharge) || s.combat.xs.rage.cooldown_until > clock || !crate::combat::form::state_gate(state, cx) {
        return false;
    }
    let target = cx.pawn.combat.lockon.current;
    crate::states::attack::xs_forced_attack(cx.pawn, target, RISKCHARGE, false);
    true
}

pub(crate) fn begin(cx: &mut Ctx, attack_kind: u8) {
    let clock = cx.pawn.combat.xs.clock;
    match attack_kind {
        RAGE_RAID => {
            cx.pawn.status.damage_invalid = true;
            let r = &mut cx.pawn.combat.xs.rage;
            r.raid_until = 0.0;
            r.raids += 1;
            situation::clear_reaction(cx, X.rage_raid_command as i32);
            aura(cx, true);
        }
        RISKCHARGE => {
            let s = &mut *cx.pawn;
            s.status.damage_invalid = true;
            // The price: a share of the current HP, never the last point.
            let cost = ((s.status.hp as f32 * X.riskcharge_cost).ceil() as i32).max(1);
            let hp = (s.status.hp - cost).max(1);
            s.status.set_hp(hp);
            let r = &mut s.combat.xs.rage;
            r.level = (r.level + 1).min(X.riskcharge_max_level as u8);
            r.charge_until = clock + X.riskcharge_duration;
            r.cooldown_until = clock + X.riskcharge_cooldown;
            r.charges += 1;
            aura(cx, true);
        }
        _ => {}
    }
}

pub(crate) fn tick_attack(cx: &mut Ctx, attack_kind: u8) {
    if attack_kind == RAGE_RAID && cx.pawn.anim.time >= X.rage_raid_invincible_to {
        cx.pawn.status.damage_invalid = false;
    }
}

pub(crate) fn end(cx: &mut Ctx, attack_kind: u8) {
    match attack_kind {
        RAGE_RAID => {
            cx.pawn.status.damage_invalid = false;
            if cx.pawn.combat.xs.rage.charge_until <= cx.pawn.combat.xs.clock {
                aura(cx, false);
            }
        }
        RISKCHARGE => cx.pawn.status.damage_invalid = false,
        _ => {}
    }
}

/// Crimson Blast's shot (finishers.rs tick): at `riskcharge.blast_time`, the row of the charge level, in front.
pub(crate) fn blast_tick(cx: &mut Ctx) {
    if cx.pawn.combat.xs.rage.blast_fired || cx.pawn.anim.time < X.riskcharge_blast_time {
        return;
    }
    cx.pawn.combat.xs.rage.blast_fired = true;
    let row: &'static str = match cx.pawn.combat.xs.rage.level {
        0 => "AN_CRIMSONBLAST_IMPACT",
        1 => "AN_CRIMSONBLAST_RC1",
        2 => "AN_CRIMSONBLAST_RC2",
        _ => "AN_CRIMSONBLAST_RC3",
    };
    let own = cx.pawn.cmc.location + Vec3::Z * cx.pawn.params().capsule_center_z;
    let a = cx.pawn.yaw.to_radians();
    let ahead = own + Vec3::new(a.cos(), a.sin(), 0.0) * X.riskcharge_blast_ahead;
    let target = cx.pawn.combat.attack_ctx.target;
    let at = (target != TargetId::NONE).then(|| cx.target(target)).flatten().map(|t| t.center).filter(|c| (*c - own).length() < X.riskcharge_blast_ahead).unwrap_or(ahead);
    crate::combat::hit::spawn_xs_shot(cx, "XS_CRIMSON_BLAST", row, at, X.riskcharge_blast_radius, X.riskcharge_blast_life, Some("vp_ant_rsk_crg_01"));
}

/// Rage Raid's END hit landed (mod.rs `adjust_damage`): the body's revenge ends.
pub(crate) fn raid_end_hit(cx: &mut Ctx, target: TargetId) {
    cx.host.targets.end_revenge(target);
}

pub(crate) fn owns(attack_kind: u8) -> bool {
    matches!(attack_kind, RAGE_RAID | RISKCHARGE)
}
