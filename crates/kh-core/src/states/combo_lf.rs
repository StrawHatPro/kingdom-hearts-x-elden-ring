//! (weapons F-LIGHTDARK) `TresPlayerState_ComboLF` (id 159): the attack state of Light Form (0xDD), and the parts of the
//! three Re Mind classes COMBO_LF / COMBO_DF (`combo_df.rs`, 160) / COMBO_TF (`combo_tf.rs`, 161) that touch the Attack
//! state's fields (pawn slot 1337 0x141432F00: 0xDD -> 0x9F, 0xDE -> 0xA0, 0xDF / 0xE0 -> 0xA1).
//!
//! The classes (vtables 0x146910708 / 0x14690D2A8 / 0x1469130A8, read from base/KH3.exe) are the Attack class with:
//!
//! | slot | LF | DF | TF | what |
//! |---|---|---|---|---|
//! | 95 / 96 / 97 / 98 / 120 | thunks to Attack's 0x14133D230 / 0x141333210 / 0x141341340 / 0x141335120 / 0x1413378B0 | = | = | Reset, Begin, Tick, End, PlayAttack |
//! | 122 Decide | 0x141362300 | 0x1413606F0 | 0x141363CE0 | `combat::forms::lightdark::decide` |
//! | 136 the warp | Attack's | 0x141352160 | Attack's | Dark Break (0xD0): `lightdark::dark_break_warp` |
//! | anim interface 69 (0x228) the shot handler | Attack's 0x141344580 | Attack's | 0x1413590B0 | `lightdark::shot` |
//!
//! So the port's Attack state runs them, carrying the class in `Attack::remind` (as ComboLimit's `limit`): its Begin calls
//! the class's Decide, its PlayAttack the class's cases (`play_remind`, the rows `attack.<form>.<kind>` of
//! sheets/port/sora-form-lightdark.json), its warp countdown Dark Break's warp. What the Attack state of the port does not
//! carry is kept here: the invincible time of PlayAttack's tail (pawn vt+0x2068(1); Tick 0x141341340 turns it off at
//! m_bEnableChange or when +0x25C runs out) and the notify's projectile name for the shot handlers.

use super::attack::Attack;
use super::{PlayerState, StateData, StateId};
use crate::anim::{AnimId, NotifyClass, NotifyData, NotifyPhase, Notifies, PlayArgs};
use crate::combat::forms::lightdark::{self as ld, Class};
use crate::combat::tables::P;
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;

/// PlayAttack's tail of the cases with `pUStackX_10 = 1` (the counters 0x25; Dark Break's warp: pawn vt+0x2068(1)): the
/// invincible bit and its time (+0x25C; 0 = until m_bEnableChange).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(crate) struct Invincible {
    pub on: bool,
    pub timer: f32,
}

/// One of the three classes: the Attack state's fields and the invincible bit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ComboLf {
    pub(crate) a: Attack,
    pub(crate) inv: Invincible,
}

/// Queues the attack of a class (attack.rs `queue_attack`).
pub(crate) fn queue(s: &mut Pawn, class: Class, next: Attack, push: bool) {
    let next = Attack { remind: class.value(), limit: false, ..next };
    let state: StateData = match class {
        Class::Lf => ComboLf { a: next, inv: Invincible::default() }.into(),
        Class::Df => super::combo_df::ComboDf(ComboLf { a: next, inv: Invincible::default() }).into(),
        Class::Tf => super::combo_tf::ComboTf(ComboLf { a: next, inv: Invincible::default() }).into(),
    };
    if push {
        s.queue.push_state(state);
    } else {
        s.queue.set_state(state);
    }
}

impl Attack {
    /// The class's Decide (slot 122): `lightdark::decide`; Dark Break asks for the warp (+0x1E1).
    pub(super) fn remind_decide(&mut self, input: &crate::combat::decide::DecideIn, abilities: &crate::combat::decide::ComboAbilities) -> crate::combat::decide::Decision {
        let Some(class) = Class::from_value(self.remind) else { return crate::combat::decide::decide(input, abilities) };
        let r = ld::decide(class, input, abilities);
        self.warp_start = r.warp;
        r.d
    }

    /// PlayAttack's head with +0x1E1 for Dark Break (0x1413378B0, decompiled C): the phases reset, +0x234 = (+0x234 & ~2) | 4,
    /// +0x1DF = 1, gravity off, Flying, Play("JumpNormalStart", blend -1, mode 2), the warp countdown +0x258 = 0.2, m_bFaceAt,
    /// invincible; the teleport and the real play come when the countdown ends (Tick's 0x14134AA80 -> `dark_break_warp`).
    /// The homing target: the controller's current one when it has none (ComboDF Decide vt+0x16F0). False = no warp.
    pub(super) fn remind_warp_begin(&mut self, cx: &mut Ctx) -> bool {
        if !(self.warp_start && Class::from_value(self.remind) == Some(Class::Df) && self.kind == ld::KIND_DARK_BREAK) {
            self.warp_start = false;
            return false;
        }
        if self.target_info(cx).is_none() {
            self.target = cx.pawn.combat.lockon.current;
        }
        let (delay, _) = ld::dark_break_times();
        self.warp_delay = delay;
        (self.chan_root, self.air_mode, self.face_at) = (false, true, true);
        let s = &mut *cx.pawn;
        s.cmc.gravity_off = true;
        s.cmc.set_flying();
        match AnimId::from_name("JumpNormalStart") {
            Some(id) => s.anim.play_with(id, PlayArgs::mode(2, -1.0)),
            None => s.note("JumpNormalStart is not in the generated animation table"),
        }
        self.face_yaw = s.yaw;
        self.sync_ctx(s);
        // (AUDIT-B W-CORE) the head's vt+0x1A0(m_CmnAuraEffect, +0x22C, 0): ComboDF writes no +0x22C (exe-wide: its only writers
        // are the Attack Reset 0xB, Begin's chase 0xD, ComboAnti 0x11, ComboSW 0xF, AttackTemplate's data), so ead_pc_aura_com0
        // group 11 now and 12 when the countdown ends (attack.rs tick) or End comes first.
        super::attack::cmn_aura(cx, super::attack::RESET_WARP_AURA);
        self.warp_aura_end = Some(super::attack::RESET_WARP_AURA + 1);
        self.sub = 1;
        true
    }

    /// PlayAttack (0x1413378B0) of the classes' cases (the sheet's `attack.<form>.<kind>` rows: 0x1F, 0x25, 0xCD .. 0xE0);
    /// false = the kind is the Attack class's own (attack.rs plays it). Called after PlayAttack's defaults.
    pub(super) fn play_remind(&mut self, cx: &mut Ctx) -> bool {
        let Some(class) = Class::from_value(self.remind) else { return false };
        let kind = self.kind;
        let Some(row) = ld::attack_row(class, kind, self.air) else { return false };
        let flag = |f: &str| ld::has_flag(row, f);
        // (PlayAttack's head - the last kind, the history - is attack.rs's, before this.)
        self.face_at = true;
        (self.chan_xy, self.chan_root) = (row.chan_xy, row.chan_root);
        (self.stop_dist, self.z_gain, self.air_mode, self.keep_face) = (row.stop_dist, row.z_gain, row.air_mode, row.keep_face);
        self.root_len = row.root_len;
        if flag("no_vz") {
            self.no_vz = true;
        }
        if flag("keep_dir_z") {
            self.keep_dir_z = true;
        }
        if flag("dive") {
            self.dive = true;
        }
        if flag("swing_started") {
            // +0x164 = 1: the counter may chain at once.
            self.swing_started = true;
        }
        // (AUDIT-C LD) case 0xD0: +0x23C |= 0x20: the traction window's end lands Dark Break and fires SCDF_DARKBREAK
        // (attack.rs traction, 0x14134A786).
        if flag("b23c_20") {
            self.land_at_traction_end = true;
        }
        // (AUDIT-C LD) case 0x25 outside Double Form (BarrierCracker / AirBarrierCracker): +0x234 |= 0x2800 and
        // 0x1405B8B30(pawn+0x460, dl = 8): bit 11 until End, End's air branch plain (bit 13).
        // (AUDIT-B W-CORE) 0x1405B8B30 is MyBodyColl's push power level (PPL_90), not the body reaction (pawn+0x619): attack.rs.
        if kind == ld::KIND_BARRIER_CRACKER && class != Class::Tf {
            self.counter_body(cx.pawn);
        }
        if flag("camera:5") {
            // thunk_FUN_1414122A0(ctrl, 0, 0, 0, 0), then (ctrl, 1, 5, target, 0): the attack camera, mode 5.
            let target = self.target;
            crate::combat::camera::off(cx);
            crate::combat::camera::request(cx, true, 5, target);
        }
        let pawn = &mut *cx.pawn;
        // The tail (LAB_14133A87A).
        if !self.air_mode {
            pawn.cmc.gravity_off = false;
            self.dir_z = 0.0;
        } else {
            pawn.cmc.gravity_scale = P.air_gravity_scale;
            pawn.cmc.gravity_off = true;
            pawn.cmc.set_flying();
            self.hover = true;
            self.air_hold = P.air_hold_time;
        }
        pawn.combat.swing_hit = false;
        pawn.root_scale = (1.0, 1.0);
        // Play(name, 0, 0, mode 2, +0x198 (the effect group: 0x834 Light, 0x898 Dark), 1.0, scale).
        if row.group != 0 {
            pawn.next_effect_group = Some(row.group);
        }
        match AnimId::from_name(row.clip) {
            Some(anim) => pawn.anim.play_with(anim, PlayArgs::mode(2, -1.0)),
            None => pawn.note("a Light / Dark / Double Form clip is not in the generated animation table (tools/gen_port.js FORM_ANIM_SETS)"),
        }
        self.face_yaw = pawn.yaw;
        self.sync_ctx(pawn);
        true
    }
}

/// The invincible bit of PlayAttack's tail for the kind just played (the rows' `invincible`; Dark Break: from the warp's
/// head on, +0x25C = 0.5).
fn arm(c: &mut ComboLf, cx: &mut Ctx) {
    let Some(class) = Class::from_value(c.a.remind) else { return };
    let warp = c.a.kind == ld::KIND_DARK_BREAK && class == Class::Df;
    let invincible = ld::attack_row(class, c.a.kind, c.a.air).is_some_and(|r| ld::has_flag(r, "invincible"));
    if invincible || warp {
        cx.pawn.status.damage_invalid = true;
        c.inv = Invincible { on: true, timer: if warp { ld::dark_break_times().1 } else { 0.0 } };
    }
    // +0x19C = 0x34 (case 0x25): the play's voice group number (anim interface vt+0x1F0).
    if let Some(v) = ld::attack_row(class, c.a.kind, c.a.air).and_then(|r| r.values.first()).filter(|_| c.a.kind == ld::KIND_BARRIER_CRACKER && class != Class::Tf) {
        cx.set_voice_group_no(*v as i32);
    }
}

pub(super) fn begin(c: &mut ComboLf, cx: &mut Ctx) {
    c.a.begin(cx);
    arm(c, cx);
}

/// Tick 0x141341340: the Attack's, then the invincible time (+0x25C) and m_bEnableChange turn the bit off.
pub(super) fn tick(c: &mut ComboLf, cx: &mut Ctx, dt: f32) {
    c.a.tick(cx, dt);
    if c.inv.on {
        let ran_out = c.inv.timer > 0.0 && {
            c.inv.timer -= dt;
            c.inv.timer <= 0.0
        };
        if cx.pawn.queue.flags.enable_change || ran_out {
            c.inv.on = false;
            cx.pawn.status.damage_invalid = false;
        }
    }
}

pub(super) fn end(c: &mut ComboLf, cx: &mut Ctx) {
    if c.inv.on {
        c.inv.on = false;
        cx.pawn.status.damage_invalid = false;
    }
    if c.a.kind == ld::KIND_DARK_BREAK {
        // Attack's End: `kind 0xD0 -> 0x14140DF20(ctrl, 0, 0, 0, 0)`: the attack camera off.
        crate::combat::camera::off(cx);
    }
    c.a.end(cx);
}

/// The notifies: the shot handlers of the classes for TresAnimNotify_Shot (its m_ProjectileName / LocationIndex), then the
/// Attack's handling of every notify.
pub(super) fn on_notifies(c: &mut ComboLf, cx: &mut Ctx, notifies: &Notifies) {
    if let Some(class) = Class::from_value(c.a.remind) {
        for event in &notifies.events {
            if (event.class(), event.phase) != (NotifyClass::TresAnimNotifyShot, NotifyPhase::Instant) {
                continue;
            }
            let def = event.def();
            let index = match def.data {
                NotifyData::Shot { location_index, .. } => location_index.unwrap_or(0),
                _ => 0,
            };
            let name = def.props.iter().find(|(k, _)| *k == "m_ProjectileName").map(|(_, v)| v.trim_matches('"'));
            c.a.sync_ctx(cx.pawn);
            let muzzle = crate::combat::hit::shot_origin(cx);
            // (0x141344580 case 0xD0, SCDF_DARKBREAK once landed, is attack.rs's shot_handler: it needs the landing's call.)
            if ld::shot(cx, ld::ShotCall { class, kind: c.a.kind, name, index, muzzle, target: c.a.target, shots: c.a.shots }) {
                c.a.shots += 1;
            }
        }
    }
    c.a.on_notifies(cx, notifies);
}

impl PlayerState for ComboLf {
    const ID: StateId = StateId::ComboLf;

    fn sub_step(&self) -> u8 {
        self.a.sub_step()
    }

    fn begin(&mut self, cx: &mut Ctx) {
        begin(self, cx);
    }

    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        tick(self, cx, dt);
    }

    fn end(&mut self, cx: &mut Ctx) {
        end(self, cx);
    }

    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        on_notifies(self, cx, notifies);
    }

    fn movement_state(&self, pawn: &Pawn) -> MovementState {
        self.a.movement_state(pawn)
    }
}
