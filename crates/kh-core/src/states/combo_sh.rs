//! (weapons F-SHIELD) `TresPlayerState_ComboSH` (id 84): the attack state of Counter Shield (0x89) and Frying Pan (0x91)
//! (pawn slot 1337 0x141432F00: forms 0x89 / 0x91 -> class 0x54). A subclass of `TresPlayerState_Attack` (attack.rs, which
//! this wraps) with the vtable 0x146910FF8; it overrides (audit/vfuncs.json):
//!
//! | slot | address | what |
//! |---|---|---|
//! | 95 Reset | 0x141352E90 | Attack's Reset, then +0x280 (the grow the attack spends) = 0, +0x250 = 2000.0 (no reader in the class) |
//! | 96 Begin | 0x14134F440 | Attack's Begin (its PreDecide / Decide / PlayAttack: below), then for kinds 0x55 / 0x56: invincible (pawn vt+0x2068(1) = pawn+0x630 bit 2, `Status::damage_invalid`) and the world slowed (`WorldSlow(ctrl, 1)`) |
//! | 97 Tick | 0x141353A80 | Attack's Tick, then at `TresEnableChange` of 0x55 / 0x56 with +0x280 set: +0x280 = 0 and the weapon back to grow LV1 (pawn vt+0x13E8(0, 0, 0)) |
//! | 98 End | 0x141350750 | while M_B414 plays: equipment vt+0x560(7) (each weapon part vt+0x10D0); kinds 0x55 / 0x56: the weapon reset as in Tick if still pending, the world slow off; Attack's End; invincible off |
//! | 122 Decide | 0x1413632A0 | the combo: `combat::forms::shield::decide` |
//! | 125 | 0x141366990 | the raid chain of kind 0x52: M_B413 -> M_B414 -> M_B415 |
//!
//! PlayAttack (slot 120 0x1413378B0) is the Attack class's, cases 0x50 .. 0x57: here from the rows `attack.<form>.<kind>`
//! of `sheets/port/sora-form-shield.json` (tools/sora_form_shield.js). MaxCombo (pawn slot 928 0x141433570) is 5 in both
//! forms. The attack request in the forms is `try_attack` below (the form package's `attack` hook, combat/forms): KH3's
//! TryAttack 0x141437330 with the class the form asks for; the rows of `CanAttack` are attack.rs's.
//!
//! (weapons F-SHIELD) Also here: the body of FINISH_SH, `TresPlayerState_StyleFinishSH` (id 38; TryStyleFinish 0x141447840
//! case 0xA3): Reset 0x1413D0DF0, Begin 0x1413CC390, Tick 0x1413D5E50 (Counter Shield: Pegasus and the Chariot), slot 167
//! 0x1413E1210 (Frying Pan, +0x310 = form 0x91), End 0x1413CF260. The framework queues STYLEFINISH (33) for every finish
//! command (combat/form.rs `start_style`); the body runs there through the form package's finish hooks; the state 38
//! below is the same body for the day `start_style` takes TryStyleFinish's class (build/sora_tmp/weapons/F-SHIELD/STATE.md).

use super::attack::Attack;
use super::common::{ground_probe, on_ground};
use super::style_finish::StyleFinish;
use super::{PlayerState, StateData, StateId};
use crate::anim::{AnimId, Notifies, PlayArgs};
use crate::combat::forms::shield::{self as sh, ShieldDecideIn};
use crate::combat::{form, tables::P};
use crate::enums::ETresAbilityKind;
use crate::host::TargetId;
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;
use glam::Vec3;

/// MaxCombo (pawn slot 928 0x141433570) in forms 0x89 / 0x91.
pub(crate) const MAX_COMBO: i32 = 5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ComboSh {
    /// The Attack state's fields.
    pub(crate) a: Attack,
    /// +0x280: the grow this attack spends (equipment vt+0x448: 1 = LV2, 2 = LV3; here `Form::grow` - 1).
    pub(crate) spend: u8,
}

impl ComboSh {
    /// Reset (0x141352E90).
    pub(crate) fn new(a: Attack) -> ComboSh {
        ComboSh { a, spend: 0 }
    }

    /// The raid chain (slot 125 0x141366990), every tick before the Attack's own.
    fn raid(&mut self, cx: &mut Ctx) {
        let cmd = cx.pawn.combat.form.cmd;
        let playing = cx.pawn.anim.name.name();
        let (b413, b414) = (sh::clip(cmd, "M_B413"), sh::clip(cmd, "M_B414"));
        let hit = cx.pawn.combat.hit_count > 0;
        if playing == b413 {
            if hit {
                self.a.enable_input = true;
            }
            if !cx.pawn.anim.ended {
                return;
            }
            // Play("M_B414", blend 0, mode 2); SetVoiceGroupNo(0).
            if let Some(id) = AnimId::from_name(b414) {
                cx.pawn.anim.play_with(id, PlayArgs::mode(2, 0.0));
                cx.set_voice_group_no(0);
            }
        } else if playing == b414 {
            if hit {
                self.a.enable_input = true;
            }
            if !cx.pawn.anim.ended && !self.a.next_queued {
                return;
            }
            // pawn vt+0x13B8(7) (equipment vt+0x560(7): every weapon part's vt+0x10D0): no seam in the core.
            if let Some(id) = AnimId::from_name(sh::clip(cmd, "M_B415")) {
                cx.pawn.anim.play_with(id, PlayArgs::mode(2, -1.0));
                cx.set_voice_group_no(0);
            }
        } else {
            return;
        }
        // "if +0x143: m_bEnableChange = 1": a queued attack chains at once.
        if self.a.next_queued {
            cx.pawn.queue.flags.enable_change = true;
        }
    }

    /// PlayAttack (slot 120 0x1413378B0) of the shield kinds 0x50 .. 0x57 (attack.rs `play_attack` for the others).
    fn play(&mut self, cx: &mut Ctx) {
        let cmd = cx.pawn.combat.form.cmd;
        let kind = self.a.kind;
        let Some(row) = sh::attack_row(cmd, kind) else {
            // A forced kind (flow attacks, counters): the Attack class's own case.
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
        a.face_at = true;
        (a.chan_xy, a.chan_root) = (row.chan_xy, row.chan_root);
        (a.stop_dist, a.z_gain, a.air_mode, a.keep_face) = (row.stop_dist, row.z_gain, row.air_mode, false);
        a.root_len = row.root_len;
        if row.traction {
            a.enable_traction = true;
        }
        let flag = |f: &str| row.flags.contains(&f);
        if flag("counter_close") {
            // 0x1414145C0(ctrl, 0.0, 0, 0, 0): the reaction prompt and the just-guard timer are dropped.
            crate::actions::close_counter_window(pawn);
        }
        if flag("finish") {
            a.finish = true;
        }
        if flag("finish_end") {
            // +0x141 = 1; pawn vt+0x2740(1): the pawn's finish-end bit.
            a.finish_end = true;
            pawn.combat.finish_end = true;
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
        pawn.combat.swing_hit = false;
        pawn.root_scale = (1.0, 1.0);
        // Play(name, 0, 0, mode 2, +0x198 (the effect group: Frying Pan's 0x867 ..), 1.0, scale).
        if row.group != 0 {
            pawn.next_effect_group = Some(row.group);
        }
        match AnimId::from_name(row.clip) {
            Some(anim) => pawn.anim.play_with(anim, PlayArgs::mode(2, -1.0)),
            None => pawn.note("a shield-form attack clip is not in the generated animation table (tools/gen_port.js FORM_ANIM_SETS)"),
        }
        a.face_yaw = pawn.yaw;
        a.sync_ctx(pawn);
    }
}

impl PlayerState for ComboSh {
    const ID: StateId = StateId::ComboSh;

    fn sub_step(&self) -> u8 {
        self.a.sub
    }

    /// Begin (0x14134F440): Attack's Begin 0x141333210 with ComboSH's Decide, then the grown attacks' invincibility.
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
        // ---- Decide (slot 122 0x1413632A0)
        let pawn = &mut *cx.pawn;
        let input = ShieldDecideIn {
            combo: self.a.combo,
            reach: self.a.reach,
            last_kind: pawn.combat.last_kind,
            dist: self.a.dist,
            dz: self.a.dz,
            target_high: self.a.target_high,
            air: self.a.air || self.a.wall_run.is_some(),
            grow: pawn.combat.form.grow,
            finish_count: pawn.combat.finish_count,
        };
        let d = sh::decide(&input);
        self.spend = pawn.combat.form.grow.saturating_sub(1).min(2);
        if d.last_reset {
            pawn.combat.last_kind = 0;
            pawn.combat.stocks = (0, 0);
        }
        (self.a.kind, self.a.reach, self.a.air) = (d.kind, d.reach, input.air);
        (self.a.finish, self.a.finish_end) = (d.finish, d.finish_end);
        pawn.combat.finish_count = d.finish_count;
        pawn.combat.finish_end |= d.finish_end;
        // "if forcedKind(+0x1E8) != 0: kind = forcedKind" (the guard / risk-dodge counters, the flow kinds).
        if self.a.forced_kind != 0 {
            self.a.kind = self.a.forced_kind;
        }
        // ---- PlayAttack
        // (AUDIT-B W-CORE) the Attack's Begin 0x141333210 (this class's slot 96 thunk): Final Blow's chase over Decide's kind.
        if self.a.chase_begin(cx) {
            return;
        }
        self.play(cx);
        cx.set_voice_group_no(crate::host::audio::attack_voice_group_no(self.a.kind as i32));
        self.a.sub = 1;
        // ---- ComboSH's tail: the grown attacks are invincible, with the world slowed.
        if sh::is_grown_kind(self.a.kind) {
            cx.pawn.status.damage_invalid = true;
            form::world_slow(cx, true);
        }
    }

    /// Tick (0x141353A80).
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        self.raid(cx);
        self.a.tick(cx, dt);
        if cx.pawn.queue.flags.enable_change && sh::is_grown_kind(self.a.kind) && self.spend != 0 {
            self.spend = 0;
            sh::weapon_reset(cx);
        }
    }

    /// End (0x141350750).
    fn end(&mut self, cx: &mut Ctx) {
        if sh::is_grown_kind(self.a.kind) {
            if self.spend != 0 {
                self.spend = 0;
                sh::weapon_reset(cx);
            }
            form::world_slow(cx, false);
        }
        // Attack's End. Its chain test knows the classes Attack / ComboLimit / FireMagic as "the chain goes on": with a
        // ComboSH queued (KH3: any attack class) the chain memory it would reset is kept, and the counters pass on.
        let queued_sh = cx.pawn.queue.queued_id() == Some(StateId::ComboSh);
        let before = cx.pawn.combat.clone();
        let landed = cx.pawn.combat.swing_hit;
        self.a.end(cx);
        if queued_sh {
            let c = &mut cx.pawn.combat;
            (c.finish_count, c.combo, c.hit_count, c.hit_flag, c.finish_end, c.last_kind) = (before.finish_count, before.combo, before.hit_count, before.hit_flag, before.finish_end, before.last_kind);
            (c.sneeze_next, c.stocks, c.command_cooldown) = (before.sneeze_next, before.stocks, before.command_cooldown);
            c.friend.unison = before.friend.unison;
            c.lockon.attack_target = before.lockon.attack_target;
            (c.revenge, c.chase_armed) = (before.revenge, before.chase_armed);
            c.carry = before.carry;
            c.pass_on = if landed { (self.a.counters[0], self.a.counters[3]) } else { (0, 0) };
            c.chase_next = c.chase_armed;
        }
        // pawn vt+0x2068(0).
        cx.pawn.status.damage_invalid = false;
    }

    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        self.a.on_notifies(cx, notifies);
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}

/// `CanAttack` (slot 44 0x141426330) in the shield forms: attack.rs's rows with COMBO_SH as the attack class and MaxCombo
/// 5. None = a state whose row is attack.rs's own business (the traversal states: their forced flow kinds).
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
    let chain = |a: &Attack| s.queue.nothing_queued() && !a.next_queued && a.swing_started && a.enable_input && a.combo < MAX_COMBO && s.combat.hit_flag && !s.combat.finish_end;
    Some(match state {
        StateData::ComboSh(c) => chain(&c.a),
        StateData::Attack(a) => chain(a),
        StateData::ComboLimit(c) => chain(&c.0),
        StateData::FireMagic(m) => m.can_attack(s, MAX_COMBO) && s.combat.hit_flag && !s.combat.finish_end,
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

/// `TryAttack(target, 0)` (slot 45 0x141437330) in the shield forms: the request queues COMBO_SH (the form package's
/// `attack` hook). None = attack.rs's own path (the traversal states).
pub(crate) fn try_attack(state: &mut StateData, cx: &mut Ctx, target: TargetId) -> Option<bool> {
    let s = &mut *cx.pawn;
    if !can_attack(state, s)? {
        return Some(false);
    }
    let forced = crate::combat::guard::forced_attack(state.id(), s);
    let (target, forced_kind, just_guard, push) = match forced {
        Some(o) => (if o.target != TargetId::NONE { o.target } else { target }, o.kind, o.just_guard, o.push),
        None => (target, 0, false, false),
    };
    s.combat.lockon.attack_target = target;
    let chained = match state {
        StateData::ComboSh(c) => Some(&mut c.a),
        StateData::Attack(a) => Some(a),
        StateData::ComboLimit(c) => Some(&mut c.0),
        _ => None,
    };
    if let Some(cur) = chained {
        // The combo step: queued behind the running attack.
        cur.next_queued = true;
        let next = Attack { combo: cur.combo + 1, air: cur.air, target, reach: cur.reach, ..Attack::RESET };
        s.combat.combo = next.combo;
        s.queue.push_state(ComboSh::new(next));
    } else if let StateData::FireMagic(m) = state {
        let (combo, air) = m.attack_follows();
        s.combat.combo = combo;
        s.queue.push_state(ComboSh::new(Attack { combo, air, target, ..Attack::RESET }));
    } else if let StateData::WeaponChange(_) = state {
        let next = Attack { combo: s.combat.combo + 1, air: true, target, ..Attack::RESET };
        s.combat.pass_on = s.combat.carry;
        s.combat.combo = next.combo;
        s.queue.set_state(ComboSh::new(next));
    } else {
        // IDLE, RUN, JUMP, GUARD ...: SetState (RISKDODGE / AIR_SLIDE: PushState); the carry timer of a Keyblade switch.
        let carried = s.magic.combo_keep > 0.0 && matches!(state, StateData::Idle(_) | StateData::Run(_) | StateData::Jump(_) | StateData::Glide(_) | StateData::Floating(_));
        let combo = if carried { s.combat.combo + 1 } else { 0 };
        if carried {
            s.combat.pass_on = s.combat.carry;
        }
        let next = ComboSh::new(Attack { combo, air: !on_ground(s), target, forced_kind, just_guard, ..Attack::RESET });
        s.combat.combo = combo;
        if push {
            s.queue.push_state(next);
        } else {
            s.queue.set_state(next);
        }
    }
    Some(true)
}

// ================================================================================================ FINISH_SH

/// StyleFinishSH Begin 0x1413CC390 (StyleFinish's Begin has run): the sub-step 0, +0x300 = 0 (`root_len`).
pub(crate) fn finish_begin(f: &mut StyleFinish, _cx: &mut Ctx) {
    f.a.sub = 0;
    f.root_len = 0.0;
}

/// The hook of the STYLEFINISH (33) path. FINISH_SH runs as STYLEFINISH_SH (38, below: `combat/form.rs start_style`
/// takes TryStyleFinish's class), whose Tick is its own; a 33 instance (never queued for 0xA3) simply ends.
pub(crate) fn finish_tick(_f: &mut StyleFinish, cx: &mut Ctx, _dt: f32) -> bool {
    cx.pawn.queue.do_chain();
    true
}

/// The pawn numbers of the sheet's `pawn.<form>.*` row (`values`).
fn pawn_values(form: &'static str) -> &'static [f32] {
    crate::combat::forms::shield::pawn_values(form)
}

/// Pegasus in flight (TresStylePawnSHPegasus, its Move state 0x141409CE0; fields by their offsets): it flies a Hermite
/// spline through control points it picks as it goes (an enemy near the next point, else a random turn), at a speed that
/// swells and falls, with Sora on the Chariot (X_Link_skt): his velocity is the pawn's.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(crate) struct Ride {
    /// +0xBD8 P1, +0xBE4 P2, +0xBF0 P3 (the segment P1 -> P2 and the next point); +0xBFC T1, +0xC08 T2 (the tangents).
    p1: Vec3,
    p2: Vec3,
    p3: Vec3,
    t1: Vec3,
    t2: Vec3,
    /// +0xC14: the spline point of the last tick; +0xC20 u along the segment; +0xC28 the speed phase; +0xC30 / +0xC34 the
    /// two ramps; +0x0BC8 the time in the state; +0xC2C the thunder cool-down.
    prev: Vec3,
    u: f32,
    phase: f32,
    ramp: f32,
    accel: f32,
    time: f32,
    cooldown: f32,
    /// +0xC40 m_pTargetActor: the enemy the spline heads for. (AUDIT-A SHIELD) +0xC48, the enemies already flown at, is
    /// `PawnCombat::shield_visited` (KH3's unbounded TArray).
    target: TargetId,
    /// The delta yaw of the last tick (`GetDeltaYaw`: M_B425's first axis) and the pitch (its second).
    pub(crate) delta_yaw: f32,
    pub(crate) pitch: f32,
    /// Where the pawn is and faces (Sora rides it): for the host's drawing (`shield::finisher_pawns`).
    pub(crate) yaw: f32,
    /// The ride runs (Move state).
    pub(crate) flying: bool,
    /// (AUDIT-A SHIELD) the last thunder's body effect (VW_SO035_THD0) and the life it has left, s.
    thunder_fx: Option<(crate::host::EffectHandle, f32)>,
    /// (AUDIT-A SHIELD) Pegasus's own actor location (the spline moves IT; Sora rides the Chariot behind it).
    pub(crate) at: Vec3,
    /// (AUDIT-A CAMS) +0x304: the cleanup 0x1413CFB60 sent its OnPlayerSpecificAction(0x40) (once: sub 5 / slot 167 sub 5, or
    /// End 0x1413CF260).
    pub(crate) specific_end: bool,
}

/// (AUDIT-A SHIELD) Sora's place in Pegasus's frame while he plays `anim` (M_B425's two heaviest samples' links blended by
/// their weights; one clip: its link): (point, yaw).
fn link_blend(anim: &crate::anim::AnimRequest) -> (Vec3, f32) {
    let mut out = (Vec3::ZERO, 0.0);
    let mut total = 0.0;
    for (seq, w, t) in anim.body_samples() {
        if let Some((_, _, s, s_yaw)) = sh::ride_link(seq, t) {
            out = (out.0 + s * w, out.1 + s_yaw * w);
            total += w;
        }
    }
    if total > 0.0 {
        (out.0 / total, out.1 / total)
    } else {
        out
    }
}

/// (AUDIT-A SHIELD) Pegasus's actor for Sora at `sora` facing `yaw` on clip `seq` at `time` (the inverse of the links).
fn pegasus_at(sora: Vec3, yaw: f32, seq: &str, time: f32) -> Vec3 {
    let Some((_, _, s, s_yaw)) = sh::ride_link(seq, time) else { return sora };
    let (sn, cs) = (yaw - s_yaw).to_radians().sin_cos();
    sora - Vec3::new(s.x * cs - s.y * sn, s.x * sn + s.y * cs, s.z)
}

/// (AUDIT-A SHIELD) FINISH_SH_RUSH's capsule axis in the world: CAPSULE1 of half height `half` turned m_RelativeRocation
/// (0, 0, 90) (StylePawn/w_so036 TresAtkColl0): the roll lays its Z axis along the pawn's side (across Pegasus), turned with
/// Pegasus's yaw (TRES_ACRA_NORMAL).
fn rush_axis(yaw: f32, half: f32) -> Vec3 {
    let (s, c) = yaw.to_radians().sin_cos();
    Vec3::new(-s, c, 0.0) * half
}

impl Ride {
    /// ATresStylePawnSHPegasus's ctor 0x1413E8300 (+0xC30 = 1, +0xC34 = 0.1; its constant points are overwritten) and
    /// FUN_1413ECE80 (the Move state on), whose Begin 0x1413EBF20 (AUDIT-A SHIELD, read in the decompiled C; before, the
    /// ctor's constants were taken relative to the start: [I]) lays the spline in the WORLD from Pegasus's place `at` and
    /// forward: P0 (+0xBCC) = at - forward x 500, P1 = at, P2 = the target point (vt+0x6E0) of FUN_1413FC260(at, forward,
    /// 500) (m_pTargetActor) else at + forward x 500, P3 = that of FUN_1413FC260(P2, dir(P2 - P1) in the plane, 500)
    /// (m_pNextActor) else FUN_1413FC750's turn (700 + rand x 100), T1 / T2 = FUN_1413EC5C0 of (P0, P1, P2) / (P1, P2, P3).
    fn start(cx: &mut Ctx, at: Vec3, yaw: f32) -> Ride {
        let v = pawn_values("SH");
        let a = yaw.to_radians();
        let fwd = Vec3::new(a.cos(), a.sin(), 0.0);
        let mut r = Ride { p1: at, p2: at, p3: at, t1: Vec3::ZERO, t2: Vec3::ZERO, prev: at, u: 0.0, phase: 0.0, ramp: 1.0, accel: v[13], time: 0.0, cooldown: 0.0, target: TargetId::NONE, delta_yaw: 0.0, pitch: 0.0, yaw, flying: true, thunder_fx: None, at, specific_end: false /* (AUDIT-A CAMS) */ };
        let p0 = at - fwd * 500.0;
        cx.pawn.combat.shield_visited.clear();
        let mut enemies = Vec::new();
        cx.host.targets.enumerate(&mut enemies);
        let first = search(&enemies, &cx.pawn.combat.shield_visited, at, fwd, 500.0);
        r.p2 = match first {
            Some(t) => {
                cx.pawn.combat.shield_visited.push(t.id);
                r.target = t.id;
                t.center
            }
            None => at + fwd * 500.0,
        };
        let dir = (r.p2 - r.p1).truncate().normalize_or_zero().extend(0.0);
        r.p3 = match search(&enemies, &cx.pawn.combat.shield_visited, r.p2, dir, 500.0) {
            Some(t) => {
                cx.pawn.combat.shield_visited.push(t.id);
                t.center
            }
            None => next_point(p0, r.p1, r.p2, v[14] + roll(cx) as f32),
        };
        r.t1 = tangent(p0, r.p1, r.p2);
        r.t2 = tangent(r.p1, r.p2, r.p3);
        r
    }


    /// One tick of the Move state 0x141409CE0: returns the pawn's velocity (SetVelocity, vt+0x16B0).
    fn step(&mut self, cx: &mut Ctx, dt: f32) -> Vec3 {
        let v = pawn_values("SH");
        // The speed: (sin(phase x pi) x 700 + 900) x accel; the phase, the ramps.
        let mut speed = ((self.phase * std::f32::consts::PI).sin() * v[10] + v[9]) * self.accel;
        self.phase += dt * v[11];
        if self.phase > 1.0 {
            self.phase = 0.0;
        }
        self.ramp = (self.ramp + dt * v[12]).min(1.0);
        self.accel = (self.accel + dt * v[12]).min(1.0);
        if speed <= 0.0 {
            speed = 0.01;
        }
        // Along the segment; at its end the next point.
        let seg = (self.p2 - self.p1).length() / speed;
        let advanced = seg > 0.0 && {
            self.u += dt / seg;
            self.u < 1.0
        };
        if !advanced {
            self.u = 0.0;
            let next = next_point(self.p1, self.p2, self.p3, v[14] + roll(cx) as f32);
            let next = if roll(cx) < 30 { next + Vec3::Z * v[15] } else { next };
            (self.p1, self.p2, self.t1) = (self.p2, self.p3, self.t2);
            let heading = (self.p2 - self.p1).normalize_or_zero();
            let mut enemies = Vec::new();
            cx.host.targets.enumerate(&mut enemies);
            let visited = &cx.pawn.combat.shield_visited;
            let found = search(&enemies, visited, self.p2, heading, v[16]).or_else(|| search(&enemies, visited, self.p2, heading, 0.0));
            self.p3 = match found {
                Some(t) => {
                    cx.pawn.combat.shield_visited.push(t.id);
                    self.target = t.id;
                    t.center
                }
                None => {
                    cx.pawn.combat.shield_visited.clear();
                    self.target = TargetId::NONE;
                    let r = roll(cx);
                    if r < 50 {
                        self.p2 + (self.p2 - self.p1).normalize_or_zero() * (500.0 + 2.0 * roll(cx) as f32)
                    } else if r <= 69 {
                        next
                    } else {
                        self.p2 - (next - self.p2).normalize_or_zero() * (1400.0 + 2.0 * roll(cx) as f32)
                    }
                }
            };
        }
        // (AUDIT-A SHIELD) Move tick 0x141409CE0's tail: with m_pTargetActor (+0xC40), P3 = its target point now (vt+0x6E0)
        if self.target != TargetId::NONE {
            if let Some(t) = cx.target(self.target) {
                self.p3 = t.center;
            }
        }
        // The tangent length, T2 (FUN_1413EC5C0), the Hermite point.
        let len = ((self.p1 - self.p2).length() * v[20]).min(v[19]).max(v[18]);
        self.t2 = tangent(self.p1, self.p2, self.p3);
        let u = self.u;
        let (u2, u3) = (u * u, u * u * u);
        let point = self.p1 * (2.0 * u3 - 3.0 * u2 + 1.0) + self.t1 * len * (u3 - 2.0 * u2 + u) + self.p2 * (3.0 * u2 - 2.0 * u3) + self.t2 * len * (u3 - u2);
        let delta = point - self.prev;
        let mut vel = Vec3::ZERO;
        if dt > 0.0 && self.time > v[21] {
            vel = delta.normalize_or_zero() * (delta.length() * 0.99 / dt).min(speed * v[17]);
        }
        // The rotation toward the spline's heading (pitch within +-45), slerped by ramp x 5 x accel x dt.
        if delta.length_squared() > 1.0e-8 {
            let want_yaw = delta.y.atan2(delta.x).to_degrees();
            let want_pitch = delta.z.atan2(delta.truncate().length()).to_degrees().clamp(-45.0, 45.0);
            let a = (self.ramp * 5.0 * self.accel * dt).clamp(0.0, 1.0);
            let before = self.yaw;
            self.yaw = crate::sora::wrap_degrees(self.yaw + crate::sora::wrap_degrees(want_yaw - self.yaw) * a);
            self.pitch += (want_pitch - self.pitch) * a;
            self.delta_yaw = crate::sora::wrap_degrees(self.yaw - before);
        }
        self.prev = point;
        self.time += dt;
        self.cooldown -= dt;
        vel
    }
}

/// FUN_1413FC260(this, point, heading, distance) (AUDIT-A SHIELD, re-read): of the living, targetable "Enemy" pawns
/// (vt+0x698, vt+0x17B0 > 0: here ER's lock-on accepts it) not in the visited list, those whose target point (vt+0x6E0)
/// is at least `distance` away in the plane and not behind (dot(dir, heading) >= -0.2 in the plane), the nearest.
/// (Before: within the radius, strictly ahead.)
fn search(enemies: &[crate::host::TargetInfo], visited: &[TargetId], from: Vec3, heading: Vec3, radius: f32) -> Option<crate::host::TargetInfo> {
    use crate::host::TargetFlags as F;
    enemies
        .iter()
        .filter(|t| t.flags.has(F::ALIVE | F::LOCKABLE | F::ENEMY) && !visited.contains(&t.id))
        .filter(|t| (t.center - from).truncate().length() >= radius && (t.center - from).truncate().normalize_or_zero().dot(heading.truncate()) >= -0.2)
        .min_by(|a, b| (a.center - from).truncate().length_squared().total_cmp(&(b.center - from).truncate().length_squared()))
        .copied()
}

/// FUN_1413EC5C0: the tangent at P2 of (P1, P2, P3).
fn tangent(p1: Vec3, p2: Vec3, p3: Vec3) -> Vec3 {
    ((p3 - p2).normalize_or_zero() - (p1 - p2).normalize_or_zero()).normalize_or_zero()
}

/// FUN_1413FC750: the point `distance` beside P3, to the side the turn P1 -> P2 -> P3 does not bend to (the heading P2 ->
/// P3 turned +-90 degrees in the plane), at P3's height.
fn next_point(p1: Vec3, p2: Vec3, p3: Vec3, distance: f32) -> Vec3 {
    let a = (p2 - p1).truncate().normalize_or_zero();
    let h = (p3 - p2).normalize_or_zero();
    let left = glam::Vec2::new(-h.y, h.x).normalize_or_zero();
    let right = -left;
    let side = if a.dot(left) >= 0.0 { right } else { left };
    Vec3::new(p3.x + side.x * distance, p3.y + side.y * distance, p3.z)
}

/// The engine's `rand() % 100` from the pawn's stream (as combat/forms/rage.rs draws it).
fn roll(cx: &mut Ctx) -> i32 {
    let rng = &mut cx.pawn.combat.rng;
    if *rng == 0 {
        *rng = 0x9E37_79B9;
    }
    *rng ^= *rng << 13;
    *rng ^= *rng >> 17;
    *rng ^= *rng << 5;
    ((*rng >> 8) % 100) as i32
}

/// (AUDIT-A CAMS) The cleanup 0x1413CFB60's first act: once per finish (+0x304), pawn vt+0x1038(0x40) =
/// OnPlayerSpecificAction(64, 0); tate_finish (@9766 -> @15228) ends its six cameras and clears ON.
fn specific_end(cx: &mut Ctx, ride: &mut Ride) {
    if !std::mem::replace(&mut ride.specific_end, true) {
        cx.pawn.event(crate::host::EventKind::PlayerSpecificAction { state: StateId::StylefinishSh, action: 0x40, sub: 0 });
    }
}

/// `TresPlayerState_StyleFinishSH` (id 38): STYLEFINISH with the FINISH_SH body and its fields (+0x2E0 .. +0x318: the
/// pawns, the cameras, +0x300 the timer, +0x310 Frying Pan): Reset 0x1413D0DF0, Begin 0x1413CC390, Tick 0x1413D5E50,
/// slot 167 0x1413E1210, End 0x1413CF260.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct StyleFinishSh(pub(crate) StyleFinish, pub(crate) Ride);

impl StyleFinishSh {
    /// Reset 0x1413D0DF0 (StyleFinish's Reset, the pawns and cameras none, +0x300 = 0).
    pub(crate) fn new(f: StyleFinish) -> StyleFinishSh {
        StyleFinishSh(f, Ride::default())
    }

    /// Tick 0x1413D5E50: Counter Shield's Ultimate Finish, Pegasus and the Chariot. True = done.
    fn pegasus(&mut self, cx: &mut Ctx, dt: f32) -> bool {
        let (f, ride) = (&mut self.0, &mut self.1);
        match f.a.sub {
            0 => {
                // FUN_1405D4D90(pawn, 1, 0); Play("M_B423") + voice 0; Flying (pawn vt+0x17A0(5)); Pegasus and the Chariot
                // spawned beside him, hidden; the fixed camera on him (CameraFix: distance 100, fov 120 -> `camera`).
                f.play(cx.pawn, "M_B423", -1.0, 2);
                cx.set_voice_group_no(0);
                cx.pawn.cmc.set_flying();
                f.root_len = 0.0;
                ride.yaw = cx.pawn.yaw;
                f.a.sub = 1;
                // (AUDIT-A CAMS) the sub's last call, pawn vt+0x1038(0, 0) (0x1413D5E50 L326): OnPlayerSpecificAction(0, 0); tate_finish
                // (@9766 -> @15738, ActionID 0 with GetStateID() == 38) takes GetSpecificActionAccompanyPawn(0) = Pegasus as its pawn.
                cx.pawn.event(crate::host::EventKind::PlayerSpecificAction { state: StateId::StylefinishSh, action: 0, sub: 0 });
            }
            1 => {
                // pawn vt+0xDD0(dt x 16); at M_B423's end the three play M_B424.
                if cx.pawn.anim.ended {
                    f.play(cx.pawn, "M_B424", -1.0, 2);
                    f.a.sub = 2;
                    f.root_len = 0.0;
                }
            }
            2 => {
                // The Chariot linked to Pegasus (X_Link_skt), Sora to the Chariot, his weapon put away; the pawns shown;
                // the camera on Pegasus (CameraNormal: offset (0, 0, 100), pitch -25, distance 700, fov 120).
                crate::actions::draw(cx, false);
                f.a.sub = 3;
                f.root_len = 0.0;
            }
            3 => {
                if cx.pawn.anim.ended {
                    // M_B425 on the three (a 2D blend space: the delta yaw, the pitch); Pegasus's Move state; +0x300 =
                    // m_Param_MaxTime; WorldSlow(off); extra(0xAA). Pegasus's own FINISH_SH_RUSH capsule, auto-activated.
                    f.play(cx.pawn, "M_B425", -1.0, 2);
                    let v = pawn_values("SH");
                    f.root_len = v[0];
                    // (AUDIT-A SHIELD) Pegasus stands where the links put it: Sora rides the Chariot behind it (sh::ride_link)
                    let peg = pegasus_at(cx.pawn.cmc.location, cx.pawn.yaw, "B425", 0.0);
                    *ride = Ride::start(cx, peg, cx.pawn.yaw);
                    ride.at = peg;
                    f.a.sub = 4;
                    form::world_slow(cx, false);
                    f.extra(cx, sh::FINISH_EXTRA_THUNDER);
                    let at = peg;
                    // (AUDIT-A SHIELD) its m_HitEffects: vw_so020_hit0 (StylePawn/w_so036 TresAtkColl0), not the row's
                    crate::combat::hit::spawn_xs_shot(cx, "FINISH_SH_RUSH", "FINISH_SH_RUSH", at, v[4], v[0], Some(sh::RUSH_HIT_EFFECT));
                    let yaw = cx.pawn.yaw;
                    if let Some(shot) = cx.pawn.combat.shots.last_mut() {
                        shot.window.interval = v[8];
                        shot.offset = Vec3::new(0.0, v[6], v[7]);
                        shot.axis = rush_axis(yaw, v[5]);
                    }
                }
            }
            4 => {
                // The ride: the timer runs (the HUD gauge controller vt+0x1B38(timer, -1): `ride_gauge`); the extra fires
                // Pegasus's thunder when it can (0x141419A50: its cool-down out); ends with the timer.
                f.root_len = (f.root_len - dt).max(0.0);
                let vel = ride.step(cx, dt);
                // FUN_1413E34B0: M_B425's axes from Pegasus's turn.
                cx.pawn.anim.set_axis_2d(ride.delta_yaw, ride.pitch); // (F-LOCO) both axes (Y: the pitch, -25 .. 25)
                // (AUDIT-A SHIELD) Pegasus flies its spline (SetVelocity on ITS actor); Sora is attached to the Chariot, the
                // Chariot to Pegasus's X_Link_skt (Tick sub 2): his place is Pegasus's x the links under the clip (blended
                // as M_B425's two heaviest samples), his facing the Chariot's (FUN_1413D0070 -> vt+0x21A8 every tick).
                ride.at += vel * dt;
                let (s, s_yaw) = link_blend(&cx.pawn.anim.request());
                let (sn, cs) = ride.yaw.to_radians().sin_cos();
                let want = ride.at + Vec3::new(s.x * cs - s.y * sn, s.x * sn + s.y * cs, s.z);
                cx.pawn.cmc.velocity = if dt > 0.0 { (want - cx.pawn.cmc.location) / dt } else { Vec3::ZERO };
                cx.pawn.yaw = crate::sora::wrap_degrees(ride.yaw + s_yaw);
                let (at, yaw) = (ride.at, ride.yaw);
                let half = pawn_values("SH")[5];
                if let Some(shot) = cx.pawn.combat.shots.iter_mut().find(|s| s.name == "FINISH_SH_RUSH") {
                    (shot.position, shot.yaw) = (at, yaw);
                    shot.axis = rush_axis(yaw, half); // (AUDIT-A SHIELD) the capsule turns with Pegasus
                }
                // (AUDIT-A SHIELD) the thunder's body ends with its m_MainLife (the actor is destroyed)
                if let Some((h, life)) = ride.thunder_fx {
                    let left = life - dt;
                    ride.thunder_fx = if left > 0.0 { Some((h, left)) } else {
                        cx.stop_effect(h, false);
                        None
                    };
                }
                if f.root_len > 0.0 {
                    if f.pending == sh::FINISH_EXTRA_THUNDER && ride.cooldown <= 0.0 {
                        let v = pawn_values("SH");
                        // (AUDIT-A SHIELD) THE AIM, from the asm of 0x141419A50 (the decompile lost the arguments): Pegasus
                        // vt+0x900 (0x1405CF780: vt+0x8E0 -> +0x44E8, the ATresProjectileManager) -> FUN_141483430(manager,
                        // m_ProjClass +0xB50, Pegasus, &loc, &FRotator::ZeroRotator, m_TeamNo (vt+0xD90 0x1405CF120), false)
                        // -> manager vt+0x788 0x141483480 = the spawn (location = loc, rotation zero, team, Fire at loc); loc =
                        // Pegasus's RootComponent location (+0x188 -> +0x2B0, else FVector::ZeroVector 0x149B5C798). ProjSHThunder
                        // has InitialSpeed / MaxSpeed 0 and its FINISH_SH_THUNDER SPHERE m_Size 700 at its root (0, 0, 0): a 7 m
                        // strike AROUND PEGASUS where it is, no target (before: at the target, else 700 cm ahead of Pegasus).
                        let point = at;
                        crate::combat::hit::spawn_xs_shot(cx, "FINISH_SH_THUNDER", "FINISH_SH_THUNDER", point, v[2], v[3], None);
                        // (AUDIT-A SHIELD) TresStyleProjSHThunder's ParticleComp (StylePawn/ProjSHThunder): VW_SO035_THD0 at the
                        // actor for its m_MainLife (v[3] = 0.5 s); the hit effect stays the row's vs_thd_hit0
                        if let Some((h, _)) = ride.thunder_fx.take() {
                            cx.stop_effect(h, false);
                        }
                        let h = cx.spawn_effect(crate::host::EffectSpawn { id: crate::host::EffectId(crate::notify::notify_asset_id(sh::THUNDER_BODY)), name: sh::THUNDER_BODY, at: crate::host::Attach::World(point), rotation: Vec3::ZERO, scale: 1.0, persistent: true });
                        ride.thunder_fx = Some((h, v[3]));
                        ride.cooldown = v[1];
                        f.pending = 0;
                    }
                    return false;
                }
                f.remove_extra(cx);
                cx.pawn.combat.shots.retain(|s| s.name != "FINISH_SH_RUSH");
                ride.flying = false;
                f.a.sub = 5;
            }
            5 => {
                // 0x1413CFB60: the pawns and cameras cleaned up; Play("M_1501").
                specific_end(cx, ride); // (AUDIT-A CAMS)
                // (AUDIT-A SHIELD) the pawns' ReceiveAccompanyPawnDisappear: Pegasus vw_so035_dis_top, the Chariot
                // vw_so035_dis_end (SqExSpawnEmitterAttached on each, where they stand: at Sora, the rider)
                for name in [sh::DISAPPEAR_PEGASUS, sh::DISAPPEAR_CHARIOT] {
                    let at = cx.pawn.cmc.location;
                    cx.spawn_effect(crate::host::EffectSpawn { id: crate::host::EffectId(crate::notify::notify_asset_id(name)), name, at: crate::host::Attach::World(at), rotation: Vec3::new(0.0, cx.pawn.yaw, 0.0), scale: 1.0, persistent: false });
                }
                if let Some((h, _)) = ride.thunder_fx.take() {
                    cx.stop_effect(h, false);
                }
                cx.pawn.cmc.set_falling();
                cx.pawn.cmc.velocity = Vec3::ZERO;
                f.play(cx.pawn, "M_1501", -1.0, 2);
                f.a.sub = 6;
            }
            6 => f.a.sub = 7,
            _ => {
                // pawn vt+0xDB0 / +0xDC8 (dt x 18); at M_1501's end the finish is over.
                if cx.pawn.anim.ended {
                    return f.finish(cx.pawn);
                }
            }
        }
        false
    }

    /// Slot 167 0x1413E1210: Frying Pan's Ultimate Finish, the pan. True = done.
    fn frying_pan(&mut self, cx: &mut Ctx) -> bool {
        let (f, ride) = (&mut self.0, &mut self.1);
        if f.a.target == TargetId::NONE {
            f.a.target = cx.pawn.combat.lockon.current;
        }
        match f.a.sub {
            0 => {
                // The pan spawned at Sora with his rotation, shown, M_J423; Sora's weapon away; M_J423; m_bFaceAt; +0x164 = 0.
                crate::actions::draw(cx, false);
                f.play(cx.pawn, "M_J423", -1.0, 2);
                f.a.face_at = true;
                (ride.prev, ride.yaw, ride.flying) = (cx.pawn.cmc.location, cx.pawn.yaw, false);
                f.a.sub = 2;
                f.root_len = 0.0;
                // (AUDIT-A CAMS) slot 167 sub 0's last call (0x1413E1210 L229): pawn vt+0x1038(0, 0) = OnPlayerSpecificAction(0, 0)
                cx.pawn.event(crate::host::EventKind::PlayerSpecificAction { state: StateId::StylefinishSh, action: 0, sub: 0 });
            }
            2 => {
                // pan +0xB48 = 1 (it starts); the next tick the world slow ends.
                if f.root_len == 0.0 {
                    f.root_len = 1.0;
                } else {
                    form::world_slow(cx, false);
                    f.a.sub = 4;
                }
            }
            4 => {
                // The pan's own hit window (its M_J423: FinishSH2, the sheet's `track.SH2.pan.FinishSH2` at its spawn).
                if let Some((from, to)) = sh::pan_window() {
                    if f.a.shots == 0 && cx.pawn.anim.time >= from {
                        f.a.shots = 1;
                        let v = pawn_values("SH2");
                        if let Some((at, _, r)) = sh::track_shape(sh::CHANGE_SH2, "FinishSH2", "pan", cx.pawn.anim.time, ride.prev, ride.yaw) {
                            crate::combat::hit::spawn_xs_shot(cx, "FinishSH2", "SH2_FRYINGPAN", at, r.max(v[1]), to - from, None);
                        }
                    }
                }
                // Until TresEnableChange of M_J423 or its end (the pan's own time is not counted down here).
                if cx.pawn.queue.flags.enable_change || cx.pawn.anim.ended {
                    f.a.sub = 5;
                }
            }
            5 => {
                specific_end(cx, ride); // (AUDIT-A CAMS) slot 167 sub 5: 0x1413CFB60
                f.a.sub = 6;
            }
            _ => {
                if cx.pawn.anim.ended {
                    return f.finish(cx.pawn);
                }
            }
        }
        false
    }
}

impl PlayerState for StyleFinishSh {
    const ID: StateId = StateId::StylefinishSh;

    fn sub_step(&self) -> u8 {
        self.0.a.sub
    }

    fn begin(&mut self, cx: &mut Ctx) {
        self.0.begin(cx);
    }

    /// Tick 0x1413D5E50 (and its slot 167 for form 0x91), inside StyleFinish's Tick frame: the target refresh first, the
    /// facing after.
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        if self.0.a.target != TargetId::NONE && self.0.a.target_info(cx).is_none() {
            self.0.a.target = TargetId::NONE;
        }
        if self.0.a.target == TargetId::NONE {
            self.0.a.target = cx.pawn.combat.lockon.current;
        }
        let done = if cx.pawn.combat.form.cmd == sh::CHANGE_SH2 { self.frying_pan(cx) } else { self.pegasus(cx, dt) };
        if !done {
            self.0.a.facing(cx, dt);
            self.0.a.sync_ctx(cx.pawn);
        }
    }

    /// End 0x1413CF260: 0x1413CFB60 (the pawns and the rush cleaned up), then StyleFinish's End.
    fn end(&mut self, cx: &mut Ctx) {
        specific_end(cx, &mut self.1); // (AUDIT-A CAMS) End 0x1413CF260 -> 0x1413CFB60
        cx.pawn.combat.shots.retain(|s| s.name != "FINISH_SH_RUSH");
        self.1.flying = false;
        // (AUDIT-A SHIELD) the pawns' notify states and the thunder's body go with the pawns
        sh::pawn_notifies_end(cx);
        if let Some((h, _)) = self.1.thunder_fx.take() {
            cx.stop_effect(h, false);
        }
        self.0.end(cx);
    }

    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        self.0.on_notifies(cx, notifies);
        // (AUDIT-A SHIELD) the pawns' own clip notifies run every pose tick from sora.rs (shield::pawn_notifies)
    }

    fn movement_state(&self, pawn: &Pawn) -> MovementState {
        self.0.movement_state(pawn)
    }
}
