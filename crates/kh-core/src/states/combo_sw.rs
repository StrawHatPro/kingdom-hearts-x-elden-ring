//! (weapons F-ULTIMATE) `TresPlayerState_ComboSW` (id 82): the attack state of Ultimate Form (0x87) (pawn slot 1337
//! 0x141432F00: form 0x87 -> class 0x52). A subclass of `TresPlayerState_Attack` (attack.rs, which this wraps) with the vtable
//! 0x146912370; it overrides only slot 122 Decide 0x141363810 (`combat::forms::ultimate::decide`; sheets/exe/vfuncs.json).
//! Its Reset 0x141352EC0, Begin 0x141333480, Tick 0x1413417E0 and End 0x141335DF0 are the Attack class's: here the parts of
//! them that the form's kinds reach, with the data of `sheets/port/sora-form-ultimate.json` (tools/sora_form_ultimate.js):
//!
//! - PlayAttack (slot 120 0x1413378B0) cases 0xC7 .. 0xCC and 0x25: the rows `attack.SW.*` (`play`);
//! - its head with +0x1E1 (the teleport Decide asked for): JumpNormalStart, air mode, gravity off, Flying, invincible
//!   (pawn vt+0x2068(1)), +0x258 = 0.2: slot 117 0x14134AA80 counts it down, warps (slot 136 0x141337320 with 150 / 100, as
//!   the chase attack's: attack.rs) and plays the kind; Attack Tick 0x141341340: +0x25C (0.5 s) counts down after the warp,
//!   then (or at TresEnableChange) the invincibility goes;
//! - the shot handler 0x141344580 case 0x25: BARRIERCRACKER (`ultimate::counter_shot`).
//!
//! The attack request in the form is `try_attack` below (the form package's `attack` hook): KH3's TryAttack 0x141437330
//! with the class the form asks for and its guard counter (forced kind 0x25, 0x141437E1A); the rows of `CanAttack` are
//! attack.rs's with MaxCombo 6.
//!
//! (weapons F-ULTIMATE) Also here: the body of FINISH_SW, StyleFinish slot 151 0x1413E06B0 (`finish_tick`).

use super::attack::Attack;
use super::common::{ground_probe, on_ground};
use super::style_finish::StyleFinish;
use super::{PlayerState, StateData, StateId};
use crate::anim::{AnimId, NotifyClass, NotifyPhase, Notifies, PlayArgs};
use crate::combat::forms::ultimate::{self as sw, SwDecideIn};
use crate::combat::{form, tables::P};
use crate::enums::ETresAbilityKind;
use crate::host::TargetId;
use crate::pawn::{Ctx, Pawn};
use crate::sora::{wrap_degrees, MovementState};
use glam::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ComboSw {
    /// The Attack state's fields.
    pub(crate) a: Attack,
    /// +0x1E1: the teleport is still to be made (the countdown is the Attack's +0x258 `warp_delay`).
    pub(crate) warp: bool,
    /// +0x25C: the invincibility after the teleport (0.5 s; -1 = ran out).
    pub(crate) invincible_timer: f32,
    /// The state made him invincible (pawn vt+0x2068(1)): the teleport, the counter.
    pub(crate) invincible: bool,
}

impl ComboSw {
    /// Reset (0x141352EC0, the Attack's 0x14133D230): +0x25C = 0.
    pub(crate) fn new(a: Attack) -> ComboSw {
        ComboSw { a, warp: false, invincible_timer: 0.0, invincible: false }
    }

    /// PlayAttack (slot 120 0x1413378B0) of the form's kinds (the rows `attack.SW.*`); any other kind: the Attack class's.
    fn play(&mut self, cx: &mut Ctx) {
        let kind = self.a.kind;
        let Some(row) = sw::attack_row(kind, self.a.air) else {
            self.a.play_attack(cx);
            return;
        };
        let pawn = &mut *cx.pawn;
        pawn.combat.last_kind = kind;
        if pawn.combat.history.len() >= 16 {
            pawn.combat.history.remove(0);
        }
        pawn.combat.history.push(kind);
        let a = &mut self.a;
        let flag = |f: &str| row.flags.contains(&f);
        // The head: m_bFaceAt, the XY channel on and the root-scale channel off, stop distance 160, z gain 3; the case's.
        a.face_at = true;
        (a.chan_xy, a.chan_root) = (row.chan_xy, row.chan_root);
        (a.stop_dist, a.z_gain, a.air_mode) = (row.stop_dist, row.z_gain, row.air_mode);
        a.root_len = row.root_len;
        a.keep_face = flag("keep_face");
        if flag("dive") {
            // LAB_1413393CF: +0x234 |= 8, +0x23C |= 0x10, +0x1DE = +0x1DF = 1, DirZ = -4000 (the row's values[2]).
            (a.dive, a.no_vz, a.keep_dir_z) = (true, flag("no_vz"), flag("keep_dir_z"));
        }
        if flag("swing_started") {
            // case 0x25: +0x164 = 1.
            a.swing_started = true;
        }
        // The tail (LAB_14133A87A): ground: gravity on, DirZ 0; air mode: +0x234 |= 4 | 0x10, GravityScale 0.1, Flying,
        // +0x18C = 0.6.
        if !a.air_mode {
            pawn.cmc.gravity_off = false;
            a.dir_z = 0.0;
        } else {
            pawn.cmc.gravity_scale = P.air_gravity_scale;
            pawn.cmc.gravity_off = true;
            pawn.cmc.set_flying();
            a.hover = true;
            a.air_hold = P.air_hold_time;
        }
        if a.dive {
            a.dir_z = row.values.get(2).copied().unwrap_or(0.0);
        }
        pawn.combat.swing_hit = false;
        pawn.root_scale = (1.0, 1.0);
        // Play(name, 0, 0, mode 2, +0x198 (the effect group), 1.0, scale).
        if row.group != 0 {
            pawn.next_effect_group = Some(row.group);
        }
        match AnimId::from_name(row.clip) {
            Some(anim) => pawn.anim.play_with(anim, PlayArgs::mode(2, -1.0)),
            None => pawn.note("an Ultimate Form attack clip is not in the generated animation table (tools/gen_port.js FORM_ANIM_SETS_LATE)"),
        }
        // pUStackX_10 (case 0x25): pawn vt+0x2068(1).
        if flag("invincible") {
            pawn.status.damage_invalid = true;
            self.invincible = true;
        }
        // (AUDIT-B W-CORE) case 0x25 outside 0xDF / 0xE0: +0x234 |= 0x2800, 0x1405B8B30(pawn+0x460, 8) (attack.rs `counter_body`).
        if kind == 0x25 {
            a.counter_body(pawn);
        }
        a.face_yaw = pawn.yaw;
        a.sync_ctx(pawn);
        // The play's voice group number: +0x19C (case 0x25: 0x34), else the kind's.
        let voice = row.values.get(3).copied().unwrap_or(0.0) as i32;
        cx.set_voice_group_no(if voice != 0 { voice } else { crate::host::audio::attack_voice_group_no(kind as i32) });
    }

    /// PlayAttack's head with +0x1E1 (0x1413378B0, decompiled C): +0x21C = 1, +0x234 = (+0x234 & ~2) | 4, +0x224 = 0, air
    /// mode, gravity off, Flying, Play("JumpNormalStart", blend -1, mode 2), +0x258 = 0.2, m_bFaceAt, invincible, the warp
    /// delegate (slot 117).
    fn vanish(&mut self, cx: &mut Ctx) {
        let a = &mut self.a;
        (a.chan_root, a.phase_root, a.air_mode, a.face_at) = (false, 0, true, true);
        let pawn = &mut *cx.pawn;
        pawn.cmc.gravity_off = true;
        pawn.cmc.set_flying();
        pawn.anim.play_with(AnimId::JumpNormalStart, PlayArgs::mode(2, -1.0));
        a.warp_delay = P.chase_warp_delay;
        pawn.status.damage_invalid = true;
        self.invincible = true;
        a.face_yaw = pawn.yaw;
        a.sync_ctx(pawn);
        // (AUDIT-C SW) line 110: the anim interface's vt+0x1A0(m_CmnAuraEffect, +0x22C = 0xF, 0): the vanish's aura (group 15)
        sw::warp_aura(cx, sw::WARP_AURA_GROUP);
    }

    /// Slot 117 0x14134AA80 at 0 with +0x1E1: slot 136 (150, 100): 150 cm short of the target's lock point on the line
    /// from him, 100 cm below it, each leg swept against the level (as the chase attack's teleport, attack.rs).
    fn teleport(&mut self, cx: &mut Ctx) {
        let Some(t) = self.a.target_info(cx) else { return };
        let own = cx.pawn.cmc.location;
        let dir = ((t.center - own) * Vec3::new(1.0, 1.0, 0.0)).normalize_or_zero();
        // (F-CLAWS for F-ULTIMATE) ComboSW slot 136 = the Attack's 0x141337320 (150 / 100), straight mode (state 0x52): the shared
        // slot-136 line tests from HIM (states/attack.rs `warp_point`), placed swept (`swept_warp`)
        let end = super::attack::warp_point(cx, &t, P.chase_warp_distance, P.chase_warp_drop);
        cx.pawn.cmc.location = super::attack::swept_warp(cx, end);
        cx.pawn.cmc.stop_movement();
        if dir != Vec3::ZERO {
            cx.pawn.yaw = wrap_degrees(dir.y.atan2(dir.x).to_degrees());
        }
    }
}

impl PlayerState for ComboSw {
    const ID: StateId = StateId::ComboSw;

    fn sub_step(&self) -> u8 {
        self.a.sub
    }

    /// Begin (the Attack's 0x141333210 with ComboSW's Decide 0x141363810).
    fn begin(&mut self, cx: &mut Ctx) {
        // ---- Attack's Begin (attack.rs `begin`, the parts that apply to the class)
        crate::actions::draw_for_attack(cx, self.a.combo == 0);
        cx.pawn.actions.last_pose = 1;
        let passed = std::mem::take(&mut cx.pawn.combat.pass_on);
        if self.a.combo > 0 {
            (self.a.counters[0], self.a.counters[3]) = passed;
        }
        self.a.counters[1] = self.a.counters[0];
        self.a.counters[2] = self.a.counters[3];
        cx.pawn.combat.carry = (0, 0);
        if self.a.target_info(cx).is_none() {
            self.a.target = cx.pawn.combat.lockon.current;
        }
        cx.pawn.cmc.stop_movement();
        // PreDecide (slot 121 0x14134C420).
        (self.a.target_on_ground, self.a.dist, self.a.dz) = (true, 0.0, 0.0);
        self.a.target_metrics(cx);
        self.a.air = true;
        let (hit, distance, _) = ground_probe(cx.pawn, cx.world, P.decide_ground_probe_range);
        if hit && distance <= P.decide_ground_probe_max {
            self.a.air = false;
        }
        self.a.num_enemies = cx.pawn.combat.lockon.crowd;
        // ---- Decide (slot 122 0x141363810)
        let pawn = &mut *cx.pawn;
        let input = SwDecideIn {
            combo: self.a.combo,
            reach: self.a.reach,
            last_kind: pawn.combat.last_kind,
            num_enemies: self.a.num_enemies,
            dist: self.a.dist,
            dz: self.a.dz,
            target_high: self.a.target_high,
            air: self.a.air || self.a.wall_run.is_some(),
            forced_kind: self.a.forced_kind,
            soft_range: pawn.combat.lockon.soft_range,
            finish_count: pawn.combat.finish_count,
        };
        let d = sw::decide(&input);
        if d.last_reset {
            pawn.combat.last_kind = 0;
        }
        (self.a.kind, self.a.reach, self.a.air) = (d.kind, d.reach, input.air);
        (self.a.finish, self.a.finish_end) = (d.finish, d.finish_end);
        pawn.combat.finish_count = d.finish_count;
        pawn.combat.finish_end |= d.finish_end;
        // A teleport needs a target to reach (slot 136 without one moves nothing).
        self.warp = d.warp && self.a.target != TargetId::NONE;
        if self.warp {
            self.invincible_timer = sw::warp_invincible_time();
        }
        // "if forcedKind(+0x1E8) != 0: kind = forcedKind" (the guard counter 0x25, the flow kinds).
        if self.a.forced_kind != 0 {
            self.a.kind = self.a.forced_kind;
        }
        self.a.sub = 1;
        // (AUDIT-B W-CORE) the Attack's Begin 0x141333210: Final Blow's chase over Decide's teleport (+0x22C = 0xD over its 0xF).
        if self.a.chase_begin(cx) {
            self.warp = false;
            return;
        }
        // ---- PlayAttack
        if self.warp {
            self.vanish(cx);
            return;
        }
        self.play(cx);
    }

    /// Tick (the Attack's 0x141341340, with the teleport's slot 117 and +0x25C).
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        if self.a.warp_delay > 0.0 {
            self.a.warp_delay -= dt;
            if self.a.warp_delay > 0.0 {
                // (AUDIT-C SW) Attack Tick 0x141341340 runs its invincibility test after the delegates in every tick, the
                // vanish's too (+0x25C is not counted while +0x1E1 is set): m_bEnableChange ends it.
                if cx.pawn.queue.flags.enable_change && self.invincible {
                    cx.pawn.status.damage_invalid = false;
                    self.invincible = false;
                }
                return;
            }
            self.a.warp_delay = 0.0;
            if std::mem::take(&mut self.warp) {
                self.teleport(cx);
                // (AUDIT-C SW) slot 117 0x14134AA80 at the warp's end: vt+0x1A0(m_CmnAuraEffect, +0x22C + 1, 0) (group 16)
                sw::warp_aura(cx, sw::WARP_AURA_GROUP + 1);
            } else {
                // (AUDIT-B W-CORE) Final Blow's chase: the Attack's teleport and group 14 (attack.rs `warp_arrive`).
                self.a.warp_arrive(cx);
            }
            self.play(cx);
            return;
        }
        // +0x25C: counted down once the teleport is made; at 0 (or at TresEnableChange) the invincibility goes.
        if self.invincible_timer > 0.0 && !self.warp {
            self.invincible_timer -= dt;
            if self.invincible_timer <= 0.0 {
                self.invincible_timer = -1.0;
            }
        }
        if (cx.pawn.queue.flags.enable_change || self.invincible_timer == -1.0) && self.invincible {
            cx.pawn.status.damage_invalid = false;
            self.invincible = false;
        }
        self.a.tick(cx, dt);
    }

    /// End (the Attack's 0x141335DF0 / 0x141335120).
    fn end(&mut self, cx: &mut Ctx) {
        // (AUDIT-C SW) End 0x141335120 with the warp still pending: the reappearance's aura (vt+0x1A0(.., +0x22C + 1, 0))
        if std::mem::take(&mut self.warp) {
            sw::warp_aura(cx, sw::WARP_AURA_GROUP + 1);
        }
        self.a.end(cx);
        if self.invincible {
            cx.pawn.status.damage_invalid = false;
            self.invincible = false;
        }
    }

    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        self.a.on_notifies(cx, notifies);
        // The shot handler 0x141344580 case 0x25 (`TresAnimNotify_Shot` of BarrierCracker / AirBarrierCracker).
        if self.a.kind == sw::KIND_COUNTER && notifies.events.iter().any(|e| e.class() == NotifyClass::TresAnimNotifyShot && e.phase == NotifyPhase::Instant) {
            self.a.sync_ctx(cx.pawn);
            let sequence = crate::anim::SEQUENCES.get(cx.pawn.anim.name.data().samples.first().map_or(usize::MAX, |s| s.1)).map_or("-", |s| s.name);
            sw::counter_shot(cx, sequence);
            self.a.shots += 1;
        }
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}

/// `CanAttack` (slot 44 0x141426330) in Ultimate Form: attack.rs's rows with COMBO_SW as the attack class and MaxCombo 6.
/// None = a state whose row is attack.rs's own business (the traversal states: their forced flow kinds).
fn can_attack(state: &StateData, s: &mut Pawn) -> Option<bool> {
    if s.combat.command_cooldown > 0.0 {
        return Some(false);
    }
    if s.status.abilities.has(ETresAbilityKind::ComboMaster) {
        s.combat.hit_flag = true;
    }
    let s = &*s;
    match state.id() {
        StateId::Guard if s.actions.counter_window > 0.0 => return Some(true),
        StateId::Guard if super::guard::attack_out_of_hit(state, s) => return Some(true),
        StateId::AirSlide if s.actions.reaction_command.is_some() => return Some(true),
        id if s.combat.guard.offer.is_some_and(|o| o.source == id) => return Some(true),
        _ => {}
    }
    let max = sw::max_combo_and_finishers().0;
    let chain = |a: &Attack| s.queue.nothing_queued() && !a.next_queued && a.swing_started && a.enable_input && a.combo < max && s.combat.hit_flag && !s.combat.finish_end;
    Some(match state {
        StateData::ComboSw(c) => chain(&c.a),
        StateData::Attack(a) => chain(a),
        StateData::ComboLimit(c) => chain(&c.0),
        StateData::FireMagic(m) => m.can_attack(s, max) && s.combat.hit_flag && !s.combat.finish_end,
        StateData::UseItem(_) => s.queue.flags.enable_change,
        StateData::Idle(_) | StateData::Run(_) | StateData::Jump(_) | StateData::Glide(_) | StateData::Floating(_) | StateData::Swaying(_) => true,
        StateData::DodgeRoll(_) | StateData::AirSlide(_) => s.queue.flags.enable_change,
        StateData::Damage(d) => s.queue.nothing_queued() && !d.bad && matches!(d.kind.value(), 1 | 2 | 7 | 8 | 9 | 10) && s.queue.flags.enable_change,
        StateData::BadstatStun(_) => s.queue.nothing_queued() && s.queue.flags.enable_change,
        StateData::WeaponChange(w) => w.changed && s.combat.hit_flag,
        StateData::WallRun(_) | StateData::Hopping(_) | StateData::SlopeSlide(_) | StateData::SuperSlide(_) | StateData::WallKick(_) | StateData::PoleTurn(_) | StateData::RailSlide(_) => return None,
        _ => false,
    })
}

/// `TryAttack(target, 0)` (slot 45 0x141437330) in Ultimate Form: the request queues COMBO_SW (the form package's `attack`
/// hook); out of GUARD with the counter window open the forced kind is 0x25 (0x141437E1A, combat/guard.rs). None = attack.rs's own path
/// (the traversal states).
pub(crate) fn try_attack(state: &mut StateData, cx: &mut Ctx, target: TargetId) -> Option<bool> {
    let s = &mut *cx.pawn;
    if !can_attack(state, s)? {
        return Some(false);
    }
    let forced = crate::combat::guard::forced_attack(state.id(), s);
    // (combat/guard.rs forced_attack: out of GUARD with the counter window open, 0x25 in form 0x87: 0x141437E1A.)
    let (target, forced_kind, just_guard, push) = match forced {
        Some(o) => (if o.target != TargetId::NONE { o.target } else { target }, o.kind, o.just_guard, o.push),
        None => (target, 0, false, false),
    };
    s.combat.lockon.attack_target = target;
    let chained = match state {
        StateData::ComboSw(c) => Some(&mut c.a),
        StateData::Attack(a) => Some(a),
        StateData::ComboLimit(c) => Some(&mut c.0),
        _ => None,
    };
    if let Some(cur) = chained {
        // The combo step: queued behind the running attack.
        cur.next_queued = true;
        let next = Attack { combo: cur.combo + 1, air: cur.air, target, reach: cur.reach, ..Attack::RESET };
        s.combat.combo = next.combo;
        s.queue.push_state(ComboSw::new(next));
    } else if let StateData::FireMagic(m) = state {
        let (combo, air) = m.attack_follows();
        s.combat.combo = combo;
        s.queue.push_state(ComboSw::new(Attack { combo, air, target, ..Attack::RESET }));
    } else if let StateData::WeaponChange(_) = state {
        let next = Attack { combo: s.combat.combo + 1, air: true, target, ..Attack::RESET };
        s.combat.pass_on = s.combat.carry;
        s.combat.combo = next.combo;
        s.queue.set_state(ComboSw::new(next));
    } else {
        // IDLE, RUN, JUMP, GUARD ...: SetState (RISKDODGE / AIR_SLIDE: PushState); the carry timer of a Keyblade switch.
        let carried = s.magic.combo_keep > 0.0 && matches!(state, StateData::Idle(_) | StateData::Run(_) | StateData::Jump(_) | StateData::Glide(_) | StateData::Floating(_));
        let combo = if carried { s.combat.combo + 1 } else { 0 };
        if carried {
            s.combat.pass_on = s.combat.carry;
        }
        let next = ComboSw::new(Attack { combo, air: !on_ground(s), target, forced_kind, just_guard, ..Attack::RESET });
        s.combat.combo = combo;
        if push {
            s.queue.push_state(next);
        } else {
            s.queue.set_state(next);
        }
    }
    Some(true)
}

// ================================================================================================ FINISH_SW

/// FINISH_SW's Begin: StyleFinish's Begin 0x1413CBF70 (done by the caller) binds slot 151; sub-step 0.
pub(crate) fn finish_begin(f: &mut StyleFinish, _cx: &mut Ctx) {
    f.a.sub = 0;
}

/// StyleFinish slot 151 0x1413E06B0 (FINISH_SW): true = done (chained on itself).
pub(crate) fn finish_tick(f: &mut StyleFinish, cx: &mut Ctx, _dt: f32) -> bool {
    match f.a.sub {
        0 => {
            // SetMovementMode(5 Flying, 1); Play("M_9419", 0, -1.0, mode 2) + voice group 0; m_bFaceAt; sub 4.
            cx.pawn.cmc.set_flying();
            f.play(cx.pawn, "M_9419", -1.0, 2);
            cx.set_voice_group_no(0);
            f.a.face_at = true;
            f.a.sub = 4;
            return false;
        }
        4 => {
            // Once a hit window of M_9419 opened (+0x164): 0x14140E500(ctrl, 0) (the world slow off), m_bFaceAt, sub 5;
            // then the clip-end test of sub-steps 4 / 5 / 0xC.
            if f.a.swing_started {
                form::world_slow(cx, false);
                f.a.face_at = true;
                f.a.sub = 5;
            }
        }
        5 | 0xC => {}
        _ => return false,
    }
    if cx.pawn.anim.ended {
        // SetMovementMode(3 Falling, 1), gravity on (vt+0x1728(1)), the class gravity scale (vt+0x1658), DoChain.
        return f.finish(cx.pawn);
    }
    false
}
