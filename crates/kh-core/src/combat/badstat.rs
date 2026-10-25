//! The bad status ON SORA: the pawn's status byte and timer (pawn+0x85D / +0x860), its
//! application, its per-tick rules and its cure (docs/sora-combat-spec.md 9.9, rewritten
//! 2026-10-12 from the decompiled C of 0x1405B7C50, 0x1405A2350, 0x1405A5C40, 0x1405AFF90,
//! 0x1405B77B0, 0x1405ADBF0; constants and branch conditions named [I] there were read in the
//! disassembly). The states it leads to are `states/badstat_{stun,freeze,burn}.rs`; the sneeze
//! is an attack kind (0x1E: `combat/decide.rs`, `states/attack.rs`).
//!
//! One status at a time: a new one replaces the old one. What each does to Sora:
//!
//! | kind | effect while it lasts | ends |
//! |---|---|---|
//! | 7 FREEZE | state 123: frozen in place, pose paused, gravity on | the hit's time, counted by the state; any flinching hit |
//! | 8 STOP | his own time runs at 0.1 | the time, in real seconds |
//! | 12 STUN | state 122 once he stands on the ground: no action | the time, counted only inside state 122; each stick flick takes 0.3 s off; any damaging hit cures it |
//! | 13 SNEEZE | the Attack command (and every spell but Cure) is a sneeze, attack kind 0x1E | the time |
//! | 14 HONEY | his own time slows by the row's parameter per application, floor 0.1 | the time, in real seconds |
//! | 15 CLOUD, 16 DISCHARGE | an "adhere" actor of the game mode follows him (the host's; its behaviour was not read) | the time |
//! | 17 BURN | state 124: he hops about in random directions, 5 % of max HP per hop after the first | the time, counted only inside state 124; water |
//! | 18 HP_BREAK | max HP = the HP he had when it landed | the time |
//! | 19 TIME_BREAK | not a status: the form's time left drops by the time | at once |
//! | 6 ENERGYBURST | stored and timed, nothing else on the pawn side | the time |
//!
//! Kinds 1 - 5 and 9 - 11 (death, catch, the binds, magnet, drain, aspir) are not statuses of
//! the pawn: applying one clears the current status.
//!
//! Abilities: `BURN_GUARD` / `CLOUD_GUARD` / `SNEEZE_GUARD` / `FREEZE_GUARD` / `DISCHARGE_GUARD`
//! / `STUN_GUARD` (63 - 68) and Panacea's immunity refuse the status (`Status::
//! resists_bad_status`, the status implementer's); a form blocks every status and cures the
//! current one (pawn+0x634 bit 0).
//!
//! NOT READ (each a `TODO(spec)` below and an open question of spec 9.9): the adhere actors of
//! CLOUD / DISCHARGE, the swimming branches, who calls the stun trigger (interface +0x440 slot
//! 78), the key of `0x144445000` that also counts as a stick flick.

use crate::host::{Attach, EffectHandle, EffectId, EffectSpawn, EventKind};
use crate::pawn::Ctx;
use crate::states::{StateData, StateId};

/// `ETresBadStatusType`.
pub mod kind {
    pub const NONE: u8 = 0;
    pub const ENERGYBURST: u8 = 6;
    pub const FREEZE: u8 = 7;
    pub const STOP: u8 = 8;
    pub const STUN: u8 = 12;
    pub const SNEEZE: u8 = 13;
    pub const HONEY: u8 = 14;
    pub const CLOUD: u8 = 15;
    pub const DISCHARGE: u8 = 16;
    pub const BURN: u8 = 17;
    pub const HP_BREAK: u8 = 18;
    pub const TIME_BREAK: u8 = 19;
}

/// The value of a row's `m_BadStatusKind` without its prefix ("FREEZE", "STUN" ...); 0 for
/// "NONE" and for names the enum does not have.
const NAMES: [&str; 20] = ["NONE", "DEATH", "CATCH", "DRILL_BIND", "YO_BIND", "RALPH_BIND", "ENERGYBURST", "FREEZE", "STOP", "MAGNET", "DRAIN", "ASPIR", "STUN", "SNEEZE", "HONEY", "CLOUD", "DISCHARGE", "BURN", "HP_BREAK", "TIME_BREAK"];

/// The name of a status value (what `HitOnPlayer::bad_status` / an attack row carries).
pub fn name_of(kind: u8) -> &'static str {
    NAMES.get(kind as usize).copied().unwrap_or("NONE")
}

pub fn kind_of(name: &str) -> u8 {
    NAMES.iter().position(|n| n.eq_ignore_ascii_case(name)).unwrap_or(0) as u8
}

/// The common bad-status effects: rows of the attach-data asset `ead_com_abn0`
/// (`m_CmnBadStatesEffect`), by their id (the second argument of 0x141773240).
pub mod effect {
    /// 0xBCC sneeze, 0xBF4 the generic status aura, 0xC08 burn, 0xC1C the ice, 0xC1F the ice
    /// breaking, 0xC58 stun, 0xC67 stop.
    pub const SNEEZE: u32 = 0xBCC;
    pub const GENERIC: u32 = 0xBF4;
    pub const BURN: u32 = 0xC08;
    pub const ICE: u32 = 0xC1C;
    pub const ICE_BREAK: u32 = 0xC1F;
    pub const STUN: u32 = 0xC58;
    pub const STOP: u32 = 0xC67;

    /// The particle systems of a row, each with its socket and offset [A: sheet `sora-effects`,
    /// rows `pawn.badstate.g<id>.*` read from `ead_com_abn0`]. The seam gets one `EffectSpawn`
    /// per particle: `name` = the particle, `id` = the row id.
    pub fn particles(id: u32) -> &'static [(&'static str, &'static str, [f32; 3])] {
        match id {
            SNEEZE => &[("vs_abn_snz0", "vfx_abn_atama", [0.0, 0.0, 10.0])],
            GENERIC => &[("vs_abn_thd0", "vfx_abn_center", [0.0, 0.0, 0.0])],
            BURN => &[("vs_abn_brn0", "koshi", [10.0, -15.0, 0.0])],
            ICE => &[("vs_abn_frz0", "vfx_abn_root", [0.0, 0.0, 0.0]), ("vs_abn_frz_in", "vfx_abn_root", [0.0, 0.0, 0.0])],
            ICE_BREAK => &[("vs_abn_frz3", "vfx_abn_root", [0.0, 0.0, 0.0]), ("vs_abn_frz_out0", "vfx_abn_root", [0.0, 0.0, 0.0])],
            STUN => &[("vs_abn_stn1", "vfx_abn_stn", [0.0, 0.0, -5.0])],
            STOP => &[("vs_abn_slw00", "vfx_abn_center", [0.0, 0.0, 0.0])],
            _ => &[],
        }
    }
}

/// The time a stick flick takes off a stun (request interface slot 37 0x141440570 adds it to
/// state+0x88; [I] 0x1414405DD: 0.3).
pub const STUN_SHAKE: f32 = 0.3;
/// STOP's time dilation ([I] 0x1405B7E18: 0.1) and the floor of HONEY's ([I]: 0.1; its step is
/// at least 0.01).
pub const STOP_RATE: f32 = 0.1;
pub const HONEY_FLOOR: f32 = 0.1;
pub const HONEY_STEP_MIN: f32 = 0.01;

/// pawn+0x85D, +0x860, +0x61C, +0x868 bit 1, +0x908 and the actor's `CustomTimeDilation`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BadStatus {
    /// pawn+0x85D: `ETresBadStatusType`, 0 = none.
    pub kind: u8,
    /// pawn+0x860: seconds left.
    pub time: f32,
    /// pawn+0x61C: the max-HP cap of HP_BREAK (0 = none).
    pub hp_cap: i32,
    /// pawn+0x868 bit 1: frozen (the pose is paused; 0x1405B77B0).
    pub frozen: bool,
    /// The actor's `CustomTimeDilation` as STOP / HONEY set it (1.0 = normal): Sora's own step
    /// runs at this rate on top of the object-type table (host/time.rs).
    pub dilation: f32,
    /// pawn+0x908: the status effect attached (group 2 of the effect interface).
    effect: Option<(u32, [Option<EffectHandle>; 2])>,
    /// A HUD lock asked for by a bad-status state (`FUN_141413910(ctrl, 1, 1)`): the command
    /// menu is greyed and takes no input.
    pub hud_locked: bool,
    /// pawn+0x630 bit 3 `m_bIsNoDamageResponse` as the burn state sets it (pawn vt+0x2078): hits
    /// take HP without a reaction.
    pub no_reaction: bool,
    /// Statuses applied / cured so far (for tests and the debug text).
    pub applied: u32,
    pub cured: u32,
}

impl Default for BadStatus {
    fn default() -> BadStatus {
        BadStatus { kind: 0, time: 0.0, hp_cap: 0, frozen: false, dilation: 1.0, effect: None, hud_locked: false, no_reaction: false, applied: 0, cured: 0 }
    }
}

impl BadStatus {
    pub fn is(&self, kind: u8) -> bool {
        self.kind == kind
    }
}

/// `Immune(kind)` (pawn vt+0xF50 0x1405ADBF0): TIME_BREAK is never refused; a form refuses
/// everything; else the base parameter's table (the `*_GUARD` abilities, Panacea).
/// (Not ported: the swimming branch for kinds 7, 8, 12 - 17 and burn in water, pawn vt+0x1990.)
pub(crate) fn immune(cx: &Ctx, kind: u8) -> bool {
    if kind == kind::TIME_BREAK {
        return false;
    }
    cx.pawn.combat.form.bad_status_block || cx.pawn.status.resists_bad_status(kind)
}

fn spawn_row(cx: &mut Ctx, id: u32, persistent: bool) -> [Option<EffectHandle>; 2] {
    let mut handles = [None; 2];
    for (k, &(name, bone, offset)) in effect::particles(id).iter().enumerate().take(2) {
        let at = Attach::Sora { bone: Some(bone), offset: glam::Vec3::from(offset) };
        handles[k] = Some(cx.spawn_effect(EffectSpawn { id: EffectId(id), name, at, rotation: glam::Vec3::ZERO, scale: 1.0, persistent }));
    }
    handles
}

/// The status effect of group 2 (pawn+0x908): one row at a time, replaced or removed.
pub(crate) fn set_effect(cx: &mut Ctx, id: Option<u32>) {
    if cx.pawn.combat.bad.effect.map(|e| e.0) == id {
        return;
    }
    if let Some((_, handles)) = cx.pawn.combat.bad.effect.take() {
        for handle in handles.into_iter().flatten() {
            cx.stop_effect(handle, false);
        }
    }
    if let Some(id) = id {
        let handles = spawn_row(cx, id, true);
        cx.pawn.combat.bad.effect = Some((id, handles));
    }
}

/// A one-shot row of the same asset (the ice breaking).
fn one_shot(cx: &mut Ctx, id: u32) {
    spawn_row(cx, id, false);
}

fn set_dilation(cx: &mut Ctx, rate: f32) {
    cx.pawn.combat.bad.dilation = rate;
}

fn changed(cx: &mut Ctx, from: u8, to: u8) {
    if from != to {
        let time = cx.pawn.combat.bad.time;
        cx.pawn.event(EventKind::BadStatus { from, to, time });
    }
}

/// `ClearBadStatus(only)` (0x1405A5C40): with `only` 0 whatever is on him is removed, else only
/// that kind. STOP / HONEY give his time back; the effect, the adhere actor, the timer and the
/// HP cap go.
pub(crate) fn clear(cx: &mut Ctx, only: u8) {
    let cur = cx.pawn.combat.bad.kind;
    if only != 0 && only != cur {
        return;
    }
    if cur != 0 {
        if matches!(cur, kind::HONEY | kind::STOP) {
            set_dilation(cx, 1.0);
        }
        set_effect(cx, None);
        let bad = &mut cx.pawn.combat.bad;
        bad.kind = 0;
        bad.cured += 1;
    }
    let bad = &mut cx.pawn.combat.bad;
    (bad.time, bad.hp_cap) = (0.0, 0);
    changed(cx, cur, 0);
}

/// `ApplyBadStatus(kind, time, param)` (0x1405B7C50). Returns whether the pawn took it.
pub(crate) fn apply(cx: &mut Ctx, kind: u8, time: f32, param: f32) -> bool {
    if kind == kind::TIME_BREAK {
        // Pawn slot 1052 0x141424700 -> controller slot 876 0x1413E9D30(t): the form's time left
        // and the finish-command time drop by t, not below 0 (spec 10.7).
        let form = &mut cx.pawn.combat.form;
        form.time_left = (form.time_left - time).max(0.0);
        return true;
    }
    // pawn+0x634 bit 0 (a form); (pawn+0x460) vt+0xA0 / vt+0x98 (Panacea's immunity).
    if cx.pawn.combat.form.bad_status_block || cx.pawn.status.status_immunity > 0.0 {
        return false;
    }
    let kind = if matches!(kind, 1..=5 | 9..=11) { 0 } else { kind };
    let cur = cx.pawn.combat.bad.kind;
    if cur == kind {
        match kind {
            0 => return false,
            // A second helping of honey slows him further and keeps the longer time.
            kind::HONEY => {
                let rate = (cx.pawn.combat.bad.dilation - param.max(HONEY_STEP_MIN)).max(HONEY_FLOOR);
                set_dilation(cx, rate);
                let bad = &mut cx.pawn.combat.bad;
                bad.time = bad.time.max(time);
            }
            kind::HP_BREAK => {
                let bad = &mut cx.pawn.combat.bad;
                (bad.hp_cap, bad.time) = (cx.pawn.status.hp.max(0), time);
            }
            _ => cx.pawn.combat.bad.time = time,
        }
        return true;
    }
    clear(cx, 0);
    let bad = &mut cx.pawn.combat.bad;
    bad.kind = kind;
    if kind != 0 {
        bad.time = time;
        bad.applied += 1;
    }
    changed(cx, cur, kind);
    match kind {
        kind::STOP => {
            set_effect(cx, Some(effect::STOP));
            set_dilation(cx, STOP_RATE);
        }
        kind::STUN => set_effect(cx, Some(effect::STUN)),
        kind::SNEEZE => set_effect(cx, Some(effect::SNEEZE)),
        kind::HONEY => {
            let rate = (cx.pawn.combat.bad.dilation - param.max(HONEY_STEP_MIN)).max(HONEY_FLOOR);
            set_dilation(cx, rate);
        }
        // CLOUD: the game mode's `m_BadStatCloudClass` actor is spawned on him and nothing
        // else; DISCHARGE: `m_BadStatDischargeClass` on his `center` socket plus the generic
        // aura. TODO(spec): the adhere actors (`TresAdhereObjBase`) were not read: the host gets
        // the `BadStatus` event and the status keeps its time.
        kind::CLOUD => cx.pawn.note("the CLOUD bad status landed: its rain-cloud actor (TresAdhereObjBase, game-mode asset) was not read; only the status and its time are kept"),
        kind::DISCHARGE => {
            set_effect(cx, Some(effect::GENERIC));
            cx.pawn.note("the DISCHARGE bad status landed: its adhere actor was not read; only the status, its aura and its time are kept");
        }
        kind::HP_BREAK => cx.pawn.combat.bad.hp_cap = cx.pawn.status.hp.max(0),
        _ => {}
    }
    kind != 0
}

/// The bad status of a hit that also damages (0x1405A2350, called by `TakeDamage` before the
/// reaction gate): the hit cures a stun first; then the row's status is applied unless he is
/// immune - except FREEZE, which the gate's state applies. Returns true when the hit carries a
/// status he is not immune to.
pub(crate) fn on_damaging_hit(cx: &mut Ctx, kind: u8, time: f32, param: f32) -> bool {
    clear(cx, kind::STUN);
    if kind == 0 || immune(cx, kind) {
        return false;
    }
    if kind != kind::FREEZE {
        apply(cx, kind, time, param);
    }
    true
}

/// pawn vt+0x20D0 (0x1405BB7E0): takes `by` seconds off the status; at 0 it is cured.
pub(crate) fn shorten(cx: &mut Ctx, by: f32) {
    if cx.pawn.combat.bad.kind == 0 {
        return;
    }
    cx.pawn.combat.bad.time -= by;
    if cx.pawn.combat.bad.time <= 0.0 {
        clear(cx, 0);
    }
}

/// `SetFrozen(on)` (0x1405B77B0 with attribute 8): the pose and the attached effects pause, the
/// ice effect (0xC1C) covers him and the status byte becomes FREEZE; off: the pose runs again,
/// the ice goes, the breaking effect (0xC1F) plays and the status byte is cleared.
pub(crate) fn set_frozen(cx: &mut Ctx, on: bool) {
    if cx.pawn.combat.bad.frozen == on {
        return;
    }
    let cur = cx.pawn.combat.bad.kind;
    if on {
        // A status that slows his time gives it back first.
        if matches!(cur, kind::HONEY | kind::STOP) {
            set_dilation(cx, 1.0);
        }
        set_effect(cx, Some(effect::ICE));
        let bad = &mut cx.pawn.combat.bad;
        (bad.kind, bad.frozen) = (kind::FREEZE, true);
        bad.applied += 1;
        changed(cx, cur, kind::FREEZE);
    } else {
        set_effect(cx, None);
        one_shot(cx, effect::ICE_BREAK);
        let bad = &mut cx.pawn.combat.bad;
        (bad.kind, bad.frozen, bad.time) = (0, false, 0.0);
        bad.cured += 1;
        changed(cx, cur, 0);
    }
}

/// The HUD lock of the bad-status states (`FUN_141413910(ctrl, on, on)` -> HUD 0x141584140 /
/// 0x1415846F0): the command menu is greyed while he cannot act.
pub(crate) fn hud_lock(cx: &mut Ctx, on: bool) {
    cx.pawn.combat.bad.hud_locked = on;
}

/// The status part of the pawn tick (0x1405AFF90, called from 0x1405BC470), with `dt` Sora's
/// own (dilated) step and `real_dt` the undilated one; then the stun trigger (interface +0x440
/// slot 78 0x1405B2A50) and the max-HP cap of HP_BREAK (the max-HP getter 0x1405AB200).
pub(crate) fn tick(state: &StateData, cx: &mut Ctx, dt: f32, real_dt: f32) {
    let cur = cx.pawn.combat.bad.kind;
    if cur != 0 {
        // Panacea's immunity (or a form) cures whatever is on him.
        if cx.pawn.status.status_immunity > 0.0 || cx.pawn.combat.form.bad_status_block {
            if cx.pawn.combat.bad.frozen {
                set_frozen(cx, false);
            }
            clear(cx, 0);
        } else if cur != kind::FREEZE {
            // FREEZE is counted by its state. STOP and HONEY count real time (0x143EAE830); a
            // stun only while state 122 runs, a burn only while state 124 runs.
            let step = match cur {
                kind::STOP | kind::HONEY => real_dt,
                kind::STUN => if state.id() == StateId::BadstatStun { dt } else { -1.0 },
                kind::BURN => if state.id() == StateId::BadstatBurn { dt } else { 0.0 },
                _ => dt,
            };
            let bad = &mut cx.pawn.combat.bad;
            if step > 0.0 && bad.time > 0.0 {
                bad.time -= step;
                if bad.time <= 0.0 {
                    clear(cx, 0);
                }
            }
        }
    }
    // The stun trigger (0x1405B2A50 [C]; its caller was not found: taken as every pawn tick):
    // stunned, standing on the ground, not already in the state -> SetState(BADSTAT_STUN).
    if cx.pawn.combat.bad.kind == kind::STUN && cx.pawn.cmc.on_ground() && !matches!(state, StateData::BadstatStun(_) | StateData::Die(_)) && cx.pawn.queue.queued_id() != Some(StateId::BadstatStun) {
        cx.pawn.queue.set_state(crate::states::badstat_stun::BadstatStun::new());
    }
    // HP_BREAK: MaxHP() answers the cap while the status lasts.
    let bad = cx.pawn.combat.bad;
    if bad.kind == kind::HP_BREAK && bad.hp_cap >= 1 {
        let status = &mut cx.pawn.status;
        status.hp_max = status.hp_max.min(bad.hp_cap);
        status.hp = status.hp.min(status.hp_max);
    }
}

/// The stick flick of the controller's input update (0x1413F1AB0: controller+0x878 bit 5 ->
/// request interface slot 37 0x141440570): with the stick active, the frame it leaves the
/// centre (previous magnitude < 0.1) or swings by 90 degrees or more from the previous frame.
/// In the stun state each flick adds 0.3 s to the time the state takes off the status.
pub(crate) fn stick_flick(state: &mut StateData, previous_magnitude: f32, magnitude: f32, previous_yaw: f32, yaw: f32) {
    if magnitude < 0.1 {
        return;
    }
    let flick = previous_magnitude < 0.1 || crate::sora::wrap_degrees(yaw - previous_yaw).abs() >= 90.0;
    if flick {
        if let StateData::BadstatStun(stun) = state {
            stun.shake += STUN_SHAKE;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_map_to_the_enum() {
        assert_eq!((kind_of("FREEZE"), kind_of("STUN"), kind_of("BURN"), kind_of("NONE"), kind_of("TIME_BREAK"), kind_of("nonsense")), (7, 12, 17, 0, 19, 0));
    }
}
