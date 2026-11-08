//! (weapons F-STAFF) `TresPlayerState_ComboWA` (id 83): the attack state of Mirage Staff (0x88) (pawn slot 1337 0x141432F00:
//! form 0x88 -> class 0x53). A subclass of `TresPlayerState_Attack` (attack.rs, which this wraps) with the vtable 0x146913968
//! (read from the exe image; the slots that differ from the Attack class's):
//!
//! | slot | address | what |
//! |---|---|---|
//! | 95 Reset | 0x141352EC0 | Attack's Reset, then +0x234 \|= 0x400000: slot 130 takes the controller's target when the homing target is lost |
//! | 96 Begin | 0x14134F4D0 | Attack's Begin 0x141333210 (PreDecide, ComboWA's Decide, PlayAttack), then for 0x80 with combo > 0: the beam at once when the clip is past 0.3 s (anim vt+0x228 = the shot handler); m_bEnableInput |
//! | 122 Decide | 0x1413642B0 | `combat::forms::staff::decide` |
//! | 129 | 0x14041F340 | returns 0: no weapon recoil |
//! | 131 | 0x141355520 | the stick steering of the kinds with +0x16C (the Attack class's 0x141341870 decays by 0.5, this by 0.98) |
//! | 132 | 0x14134E420 | kind 0x80 with the next attack queued and a ComboWA queued: m_bEnableChange (the beams chain at once) |
//! | 133 | 0x1413527E0 | after the first window: m_bEnableInput, the attack button held and the images out: every 0.133333 s ExecuteCommand(0x1B) (the next attack) |
//! | anim 212 (+0x228) | 0x141359700 | the shot handler: WA_BEAM (0x80), WA_FINISH (0x81), WA_BRAID (0x86) |
//!
//! Tick 0x141341340 and End 0x141335120 are the Attack class's (thunks 0x1413417E0 / 0x141335DF0). PlayAttack (slot 120
//! 0x1413378B0) is the Attack class's: its cases 0x80 .. 0x87 are the rows `attack.*` of `sheets/port/sora-form-staff.json`.
//!
//! Also here: the body of FINISH_WA, `TresPlayerState_StyleFinishWA` (id 37): Begin 0x1413CC360 (StyleFinish's, then pawn
//! 0x1405D4D90(1, 0)), Tick 0x1413D7670 (the tower), End 0x1413CF310 (0x1413CFE90: the tower gone, Sora detached). The
//! framework runs every finish command in STYLEFINISH (33) with the package's hooks (combat/form.rs `start_style`).

use super::attack::Attack;
use super::common::{ground_probe, on_ground};
use super::style_finish::StyleFinish;
use super::{PlayerState, StateData, StateId};
use crate::anim::{AnimId, Notifies, NotifyClass, NotifyPhase, PlayArgs};
use crate::combat::forms::staff::{self as wa, Homing, StaffDecideIn};
use crate::combat::{form, tables::P};
use crate::enums::ETresAbilityKind;
use crate::host::TargetId;
use crate::input::PadButton;
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;
use glam::{Vec2, Vec3};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ComboWa {
    /// The Attack state's fields.
    pub(crate) a: Attack,
    /// +0x16C: the stick steering is on (PlayAttack of 0x82 / 0x83 / 0x84 / 0x86).
    steer: bool,
    /// +0x174 the steering's speed scale, +0x17C its yaw.
    steer_scale: f32,
    steer_yaw: f32,
    /// +0x194: the held-button repeat's clock (slot 133).
    repeat: f32,
    /// MaxAcceleration before PlayAttack wrote 3672 (End puts the class default back).
    accel: f32,
    /// state+0x13C bit 1: slot 115 counted once (the braid's "SCWA_BRAID").
    counted: bool,
}

impl ComboWa {
    /// Reset (0x141352EC0).
    pub(crate) fn new(a: Attack) -> ComboWa {
        ComboWa { a, steer: false, steer_scale: 0.0, steer_yaw: 0.0, repeat: 0.0, accel: 0.0, counted: false }
    }

    /// PlayAttack (slot 120 0x1413378B0) of the kinds 0x80 .. 0x87 (attack.rs `play_attack` for the others).
    fn play(&mut self, cx: &mut Ctx) {
        let kind = self.a.kind;
        let Some(row) = wa::attack_row(kind) else {
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
        // The defaults before the case: m_bFaceAt, the XY channel only, stop distance 160, z gain 3, air mode 0.
        a.face_at = true;
        (a.chan_xy, a.chan_root, a.phase_xy, a.phase_root, a.phase_z) = (true, false, 0, 0, 0);
        (a.stop_dist, a.z_gain, a.air_mode) = (row.stop_dist, row.z_gain, row.air_mode);
        let flag = |f: &str| wa::has_flag(row, f);
        a.keep_face = flag("keep_face");
        if flag("dive") {
            // 0x84 (LAB_1413393CF): +0x234 |= 8, +0x23C |= 0x10, +0x1DE / +0x1DF = 1, DirZ = -4000.
            (a.no_vz, a.keep_dir_z, a.dive) = (true, true, true);
        }
        self.steer = flag("steer");
        // Case 0x80: with combo > 0 and M_A411 playing, a new play only past 0.5 s (from 0.333333, m_bEnableInput).
        let beam_clip = AnimId::from_name(row.clip);
        let mut seek = None;
        let mut play = true;
        if kind == wa::KIND_BEAM && a.combo >= 1 && beam_clip.is_some_and(|c| pawn.anim.name == c) {
            if pawn.anim.time > 0.5 {
                a.enable_input = true;
                seek = Some(0.333_333);
            } else {
                play = false;
            }
        }
        // The tail (LAB_14133A87A): ground: gravity on, DirZ 0; air mode: +0x234 |= 4 | 0x10, GravityScale 0.1, gravity
        // off, Flying, +0x18C = 0.6.
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
        if flag("dir_z") {
            a.dir_z = row.values.get(3).copied().unwrap_or(0.0);
        }
        if self.steer {
            // MaxWalkSpeed = the class default x 0.9 (vt+0x1588); MaxAcceleration 3672 (vt+0x1640).
            let (walk, accel) = (steer_value(4), steer_value(5));
            let p = *pawn.params();
            self.accel = pawn.cmc.max_acceleration;
            pawn.cmc.max_walk_speed = p.speed_run * walk;
            pawn.cmc.max_acceleration = accel;
        }
        pawn.combat.swing_hit = false;
        pawn.root_scale = (1.0, 1.0);
        if play {
            match beam_clip {
                Some(anim) => pawn.anim.play_with(anim, PlayArgs::mode(2, -1.0)),
                None => pawn.note("a Mirage Staff attack clip is not in the generated animation table (tools/gen_port.js FORM_ANIM_SETS)"),
            }
            if let Some(t) = seek {
                pawn.anim.set_time(t);
            }
        }
        a.face_yaw = pawn.yaw;
        self.steer_yaw = pawn.yaw;
        a.sync_ctx(pawn);
    }

    /// The shot handler (anim-interface slot 212 0x141359700): +0x1F4 bit 4 and +0x234 bit 3 off, the first window
    /// (+0x164); by kind WA_BEAM / WA_FINISH / WA_BRAID (0x37: the Attack class's); the images fire with it; from weapon 0's
    /// muzzle (0x86: Sora's root + 20 up, level), at the homing target's lock-on point (homing on it), else 1000 cm ahead
    /// of his centre; pawn+0xE28 = 1; 0x86: slot 115 ("SCWA_BRAID", the combo count).
    fn shot(&mut self, cx: &mut Ctx) {
        self.a.keep_face = false;
        self.a.no_vz = false;
        self.a.swing_started = true;
        let name = match self.a.kind {
            0x37 => {
                self.a.shot_handler(cx.pawn);
                return;
            }
            wa::KIND_BEAM => "WA_BEAM",
            wa::KIND_BEAM_FINISH => "WA_FINISH",
            wa::KIND_BRAID => "WA_BRAID",
            _ => return,
        };
        if wa::avatars_out(&cx.pawn.combat) {
            wa::avatar_command(cx, if self.a.kind != wa::KIND_BEAM { 2 } else { 1 });
        }
        if self.a.target_info(cx).is_none() {
            self.a.target = cx.pawn.combat.lockon.current;
        }
        let (sin, cos) = cx.pawn.yaw.to_radians().sin_cos();
        let forward = Vec3::new(cos, sin, 0.0);
        let mut origin = crate::combat::hit::shot_origin(cx);
        let target = self.a.target_info(cx);
        let mut dir = match target {
            Some(t) => (t.center - origin).normalize_or_zero(),
            None => forward,
        };
        if self.a.kind == wa::KIND_BRAID {
            origin = cx.pawn.cmc.location + Vec3::Z * 20.0;
            dir.z = 0.0;
        }
        let homing = match target {
            Some(t) => Homing::Target(t.id),
            None => Homing::Point(cx.pawn.cmc.location + Vec3::Z * cx.pawn.params().capsule_center_z + forward * 1000.0),
        };
        self.a.sync_ctx(cx.pawn);
        let info = cx.pawn.combat.attack_ctx;
        if wa::fire(cx, name, origin, dir, homing, info) {
            self.a.shots += 1;
        }
        // pawn vt+0x1D80(1): pawn+0xE28, the hit confirm.
        cx.pawn.combat.hit_flag = true;
        if self.a.kind == wa::KIND_BRAID && !self.counted {
            // Slot 115 0x141332790("SCWA_BRAID", 1, 1, 1): +0x12C (a physical row, unless Berserk Charge runs) and +0x138.
            self.counted = true;
            let magic = crate::combat::tables::attack_row("SCWA_BRAID").is_some_and(|(_, r)| r.magic);
            let s = &cx.pawn.status;
            let frozen = if magic { s.abilities.has(ETresAbilityKind::EndlessMagic) } else { s.abilities.has(ETresAbilityKind::ChargeBerserk) && s.mp_charging };
            if !frozen {
                self.a.counters[0] += 1;
            }
            self.a.counters[3] += 1;
        }
    }

    /// Slot 131 (0x141355520): with +0x16C, the stick steers the attack.
    ///
    /// ```text
    /// no stick (or below 0.5): +0x174 *= 0.98; v = dir(+0x17C) x MaxWalkSpeed x +0x174
    /// else: +0x17C += wrap(stick yaw - +0x17C) x clamp(dt x 6, 0, 1); +0x174 = min(+0x174 + dt x 7.5, 1);
    ///       v = dir(+0x17C) x MySprintSpeed x +0x174
    /// SetVelXY(v)
    /// ```
    fn steering(&mut self, cx: &mut Ctx, dt: f32) {
        if !self.steer {
            return;
        }
        let s = &mut *cx.pawn;
        let (stick, magnitude) = (s.stick_yaw, s.stick_magnitude);
        let speed = if magnitude < steer_value(3) {
            self.steer_scale *= steer_value(0);
            s.cmc.max_walk_speed
        } else {
            let rate = (dt * steer_value(2)).clamp(0.0, 1.0);
            self.steer_yaw = crate::sora::wrap_degrees(self.steer_yaw + crate::sora::wrap_degrees(stick - self.steer_yaw) * rate);
            self.steer_scale = (self.steer_scale + dt * steer_value(1)).min(1.0);
            s.params().speed_sprint
        };
        let a = self.steer_yaw.to_radians();
        s.cmc.set_velocity_xy(Vec2::new(a.cos(), a.sin()) * speed * self.steer_scale);
    }

    /// The chain step of a request (TryAttack 0x141437330 behind a running attack): queued behind this one.
    fn chain(&mut self, s: &mut Pawn, target: TargetId) {
        self.a.next_queued = true;
        let next = Attack { combo: self.a.combo + 1, air: self.a.air, target, reach: self.a.reach, ..Attack::RESET };
        s.combat.combo = next.combo;
        s.queue.push_state(ComboWa::new(next));
    }
}

/// A number of the `steer` row: 0 decay, 1 ramp, 2 turn, 3 stick minimum, 4 walk scale, 5 acceleration.
fn steer_value(i: usize) -> f32 {
    wa::row("steer").and_then(|r| r.values.get(i).copied()).unwrap_or(0.0)
}

impl PlayerState for ComboWa {
    const ID: StateId = StateId::ComboWa;

    fn sub_step(&self) -> u8 {
        self.a.sub
    }

    /// Begin (0x14134F4D0): Attack's Begin 0x141333210 with ComboWA's Decide, then the beam's tail.
    fn begin(&mut self, cx: &mut Ctx) {
        // ---- Attack's Begin
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
        // ---- Decide (slot 122 0x1413642B0)
        let pawn = &mut *cx.pawn;
        let input = StaffDecideIn { combo: self.a.combo, reach: self.a.reach, last_kind: pawn.combat.last_kind, num_enemies: self.a.num_enemies, dist: self.a.dist, dz: self.a.dz, avatars: wa::avatars_out(&pawn.combat), finish_count: pawn.combat.finish_count };
        let d = wa::decide(&input);
        if d.last_reset {
            pawn.combat.last_kind = 0;
        }
        if self.a.wall_run.is_some() {
            self.a.air = true;
        }
        (self.a.kind, self.a.reach) = (d.kind, d.reach);
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
        // ---- ComboWA's tail: 0x80 with combo > 0: the beam at once past 0.3 s of the clip; m_bEnableInput.
        if self.a.kind == wa::KIND_BEAM && self.a.combo > 0 {
            if cx.pawn.anim.time > 0.3 {
                self.shot(cx);
            }
            self.a.enable_input = true;
        }
    }

    /// Tick (0x141341340 with ComboWA's slots 129 .. 133).
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        if !(wa::KIND_BEAM..=wa::KIND_BEHIND).contains(&self.a.kind) {
            // A forced kind (counters, flow attacks): the Attack class's own tick.
            self.a.tick(cx, dt);
            return;
        }
        // Slot 129: no recoil (0x14041F340 returns 0). Slot 130 (0x14134D020): a target that stopped being targetable is
        // dropped; with +0x234 bit 22 the controller's current target is taken.
        if self.a.target != TargetId::NONE && self.a.target_info(cx).is_none() {
            self.a.target = TargetId::NONE;
        }
        if self.a.target == TargetId::NONE {
            self.a.target = cx.pawn.combat.lockon.current;
        }
        cx.pawn.combat.recoil = None;
        self.a.facing(cx, dt);
        self.a.traction(cx, dt);
        // Slot 131: the steering.
        self.steering(cx, dt);
        // Slot 132 (0x14134E420): the beam with a ComboWA queued: m_bEnableChange.
        if self.a.kind == wa::KIND_BEAM && self.a.next_queued && cx.pawn.queue.queued_id() == Some(StateId::ComboWa) {
            cx.pawn.queue.flags.enable_change = true;
        }
        let s = &mut *cx.pawn;
        // Slot 128's tail: the hover of an air attack ends at TresEnableChange.
        if s.queue.flags.enable_change && self.a.hover {
            self.a.hover = false;
            let p = *s.params();
            s.cmc.gravity_scale = p.gravity_scale;
            s.cmc.gravity_off = false;
            if s.cmc.is_flying() {
                s.cmc.set_falling();
            }
        }
        if s.anim.ended || (s.queue.flags.enable_change && (self.a.next_queued || s.queue.queued_id() == Some(StateId::Weaponchange))) {
            s.queue.do_chain();
            return;
        }
        if self.a.swing_started {
            if s.queue.flags.enable_change {
                self.a.enable_input = true;
            }
            if !self.a.cancel {
                if !self.a.finish_end {
                    self.a.cancel_timer += dt;
                    if self.a.cancel_timer > P.swim_exit_delay {
                        self.a.cancel = true;
                    }
                }
                if s.queue.flags.enable_change {
                    self.a.cancel = true;
                }
            }
            // Slot 133 (0x1413527E0): m_bEnableInput, the attack button held (FUN_14150E670 on the attack key) and the
            // images out: every 0.133333 s the attack command again (ExecuteCommand(0x1B) -> TryAttack: the chain).
            let held = s.input.pad.held.has(PadButton::ATTACK);
            if self.a.enable_input && held && wa::avatars_out(&s.combat) {
                self.repeat += dt;
                if self.repeat > 0.133_333 {
                    self.repeat = 0.0;
                    let target = s.combat.lockon.current;
                    if can_chain(&self.a, s) {
                        s.combat.lockon.attack_target = target;
                        self.chain(s, target);
                    }
                }
            } else {
                self.repeat = 0.0;
            }
        }
        self.a.sync_ctx(cx.pawn);
    }

    /// End (0x141335120, the Attack class's): with +0x16C on the ground SetMovementMode(Falling, keep the velocity); the
    /// chain test knows the attack classes (a ComboWA queued keeps the chain memory); MaxAcceleration back.
    fn end(&mut self, cx: &mut Ctx) {
        let queued_wa = cx.pawn.queue.queued_id() == Some(StateId::ComboWa);
        let before = cx.pawn.combat.clone();
        let landed = cx.pawn.combat.swing_hit;
        let velocity = cx.pawn.cmc.velocity;
        let grounded = on_ground(cx.pawn);
        self.a.end(cx);
        if self.steer {
            cx.pawn.cmc.max_acceleration = self.accel;
            if grounded {
                cx.pawn.cmc.set_falling();
                cx.pawn.cmc.velocity = velocity;
            }
        }
        if queued_wa {
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
    }

    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        // `TresAnimNotify_Shot` -> anim vt+0x228: ComboWA's handler (the Attack class's runs only for 0x37, inside it).
        let shot = notifies.events.iter().any(|e| e.class() == NotifyClass::TresAnimNotifyShot && e.phase == NotifyPhase::Instant);
        let kind = self.a.kind;
        if shot && kind == 0x37 {
            self.a.on_notifies(cx, notifies);
            return;
        }
        self.a.on_notifies(cx, notifies);
        if shot {
            self.shot(cx);
            self.a.sync_ctx(cx.pawn);
        }
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}

/// CanAttack's chain row for a running ComboWA (T, attack.rs) with ComboWA's MaxCombo.
fn can_chain(a: &Attack, s: &Pawn) -> bool {
    let (max, _) = wa::max_combo(&s.combat);
    s.combat.command_cooldown <= 0.0 && s.queue.nothing_queued() && !a.next_queued && a.swing_started && a.enable_input && a.combo < max && (s.combat.hit_flag || s.status.abilities.has(ETresAbilityKind::ComboMaster)) && !s.combat.finish_end
}

/// `CanAttack` (slot 44 0x141426330) in Mirage Staff: attack.rs's rows with COMBO_WA and its MaxCombo. None = a state whose
/// row is attack.rs's own (the traversal states: their forced flow kinds).
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
    let (max, _) = wa::max_combo(&s.combat);
    let chain = |a: &Attack| s.queue.nothing_queued() && !a.next_queued && a.swing_started && a.enable_input && a.combo < max && s.combat.hit_flag && !s.combat.finish_end;
    Some(match state {
        StateData::ComboWa(c) => chain(&c.a),
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

/// `TryAttack(target, 0)` (slot 45 0x141437330) in Mirage Staff: the request queues COMBO_WA (the form package's `attack`
/// hook). None = attack.rs's own path (the traversal states).
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
    match state {
        StateData::ComboWa(c) => {
            c.chain(s, target);
            return Some(true);
        }
        StateData::Attack(a) => {
            a.next_queued = true;
            let next = Attack { combo: a.combo + 1, air: a.air, target, reach: a.reach, ..Attack::RESET };
            s.combat.combo = next.combo;
            s.queue.push_state(ComboWa::new(next));
            return Some(true);
        }
        StateData::ComboLimit(c) => {
            c.0.next_queued = true;
            let next = Attack { combo: c.0.combo + 1, air: c.0.air, target, reach: c.0.reach, ..Attack::RESET };
            s.combat.combo = next.combo;
            s.queue.push_state(ComboWa::new(next));
            return Some(true);
        }
        _ => {}
    }
    if let StateData::FireMagic(m) = state {
        let (combo, air) = m.attack_follows();
        s.combat.combo = combo;
        s.queue.push_state(ComboWa::new(Attack { combo, air, target, ..Attack::RESET }));
    } else if let StateData::WeaponChange(_) = state {
        let next = Attack { combo: s.combat.combo + 1, air: true, target, ..Attack::RESET };
        s.combat.pass_on = s.combat.carry;
        s.combat.combo = next.combo;
        s.queue.set_state(ComboWa::new(next));
    } else {
        // IDLE, RUN, JUMP, GUARD ...: SetState (RISKDODGE / AIR_SLIDE: PushState); the carry timer of a Keyblade switch.
        let carried = s.magic.combo_keep > 0.0 && matches!(state, StateData::Idle(_) | StateData::Run(_) | StateData::Jump(_) | StateData::Glide(_) | StateData::Floating(_));
        let combo = if carried { s.combat.combo + 1 } else { 0 };
        if carried {
            s.combat.pass_on = s.combat.carry;
        }
        let next = ComboWa::new(Attack { combo, air: !on_ground(s), target, forced_kind, just_guard, ..Attack::RESET });
        s.combat.combo = combo;
        if push {
            s.queue.push_state(next);
        } else {
            s.queue.set_state(next);
        }
    }
    Some(true)
}

// ================================================================================================ FINISH_WA

/// The tower's numbers: m_Param_MaxTime (`pawn.w_so073` values[0]) and its hit windows (`a-b@interval`, rows `row@group`).
fn tower_time() -> f32 {
    wa::row("pawn.w_so073").and_then(|r| r.values.first().copied()).unwrap_or(0.0)
}

/// StyleFinishWA's Begin (0x1413CC360): StyleFinish's (done by the caller), then pawn 0x1405D4D90(1, 0) (the HUD / camera
/// of a finish: no core part). The clock +0x2E8 is `root_len`, the windows the tower opened `count` (bits).
pub(crate) fn finish_begin(f: &mut StyleFinish, cx: &mut Ctx) {
    f.a.sub = 0;
    f.root_len = 0.0;
    f.count = 0;
    cx.pawn.combat.staff.tower = None;
}

/// StyleFinishWA Tick (0x1413D7670): FINISH_WA's tower. True = done (chained on itself).
pub(crate) fn finish_tick(f: &mut StyleFinish, cx: &mut Ctx, dt: f32) -> bool {
    f.root_len += dt;
    if let Some(t) = cx.pawn.combat.staff.tower.as_mut() {
        t.time += dt;
    }
    tower_hits(f, cx);
    match f.a.sub {
        0 => {
            // The accompany pawn of 0xA2 (w_so073) at Sora's root with his yaw; shown; it and Sora play M_A417 (mode 2).
            let (position, yaw) = (cx.pawn.cmc.location, cx.pawn.yaw);
            cx.pawn.combat.staff.tower = Some(wa::Tower { position, yaw, time: 0.0, running: false });
            f.play(cx.pawn, "M_A417", -1.0, 2);
            f.a.swing_started = false;
            f.a.sub = 1;
            f.root_len = 0.0;
            // (AUDIT-A STAFF) the sub's last call, pawn vt+0x1038(0, 0) (0x1413D7670 sub 0, after both plays): the
            // OnPlayerSpecificAction(0, 0) broadcast; Rapunzel_key (@31001 .. @43568: action 0 in TSID_STATE_STYLEFINISH_WA 37)
            // takes the tower as `Tres Char Pawn base` (GetSpecificActionAccompanyPawn(0)) for FINISH_WA's cameras.
            cx.pawn.event(crate::host::EventKind::PlayerSpecificAction { state: StateId::StylefinishWa, action: 0, sub: 0 });
        }
        1 => {
            // Attached to the tower's X_link01_skt (pawn vt+0x23A8, the rules DAT_148750ED4: snapped to the socket): he rides
            // its baked track (track.tower.X_link01_skt) until the cleanup of sub 6.
            ride(cx);
            f.a.sub = 2;
        }
        2 | 5 => {
            ride(cx);
            if f.a.sub == 2 {
                // Once the tower runs (+0xB48): the world slow off (0x14140E500(ctrl, 0)).
                // (AUDIT-A STAFF) +0xB48's writer, traced: the tower's anim interface (tower+0x440, vtable 0x146971278) slot 64
                // (+0x200) 0x14140E770 = the base handler 0x1405B3FE0, then [this+0x708] = tower+0xB48 = 1. Slot +0x200 is what
                // TresAnimNotifyState_Attack's NotifyBegin (its vtable 0x145D24E60 slot 73, 0x140561F40 @+71) calls: the
                // tower's first attack window, FinishWA at 6.2 s of its A417 (`tower_hits` sets it). Until then the world slow
                // stays on (F-STAFF had taken the tower's first tick).
                let Some(t) = cx.pawn.combat.staff.tower.as_mut() else { return false };
                if !t.running {
                    return false;
                }
                form::world_slow(cx, false);
                f.a.sub = 5;
            }
            let tower = cx.pawn.combat.staff.tower.is_some();
            if tower && f.root_len < tower_time() && !cx.pawn.queue.flags.enable_change && !cx.pawn.anim.ended {
                return false;
            }
            f.a.sub = 6;
        }
        6 => {
            // 0x1413CFE90: the tower gone, Sora detached; M_1501 (the landing).
            finish_cleanup(cx); // (AUDIT-A STAFF) with its SpecificAction(65)
            cx.pawn.cmc.set_falling();
            cx.pawn.cmc.gravity_off = false;
            f.play(cx.pawn, "M_1501", -1.0, 2);
            f.a.sub = 7;
        }
        7 => f.a.sub = 8,
        _ => {
            if cx.pawn.anim.ended {
                return f.finish(cx.pawn);
            }
        }
    }
    false
}

/// (AUDIT-A STAFF) 0x1413CFE90, the cleanup of sub 6 and of End 0x1413CF310 (style_finish.rs `end`): once (state +0x2EC)
/// pawn vt+0x1038(0x41) = OnPlayerSpecificAction(65, 0) (no blueprint of Mirage Staff answers 65: Rapunzel_key's
/// ReceivePlayerSpecificAction @31001 compares ActionID 0 only); pawn vt+0x23B0(0): detached from the tower; the tower's
/// vt+0x2E8, its anim stopped (0x1405A1DC0(+0x440, 0, 0)), its Disappear (vt+0x2590): gone. The tower is the once-guard
/// here (sub 0 always spawns it; End's second call finds it gone).
pub(crate) fn finish_cleanup(cx: &mut Ctx) {
    if cx.pawn.combat.staff.tower.take().is_some() {
        cx.pawn.event(crate::host::EventKind::PlayerSpecificAction { state: StateId::StylefinishWa, action: 0x41, sub: 0 });
    }
}

/// Sora on the tower: his actor snapped to X_link01_skt (its baked track under the tower's A417: place and facing in the
/// tower's actor space), gravity off, no velocity of his own.
fn ride(cx: &mut Ctx) {
    let Some(t) = cx.pawn.combat.staff.tower else { return };
    let Some(k) = wa::tower_track("X_link01_skt", t.time) else { return };
    let (sin, cos) = t.yaw.to_radians().sin_cos();
    let world = |x: f32, y: f32| Vec2::new(x * cos - y * sin, x * sin + y * cos);
    let p = world(k[0], k[1]);
    let s = &mut *cx.pawn;
    s.cmc.stop_movement();
    s.cmc.gravity_off = true;
    s.cmc.location = t.position + Vec3::new(p.x, p.y, k[2]);
    let fw = world(k[3], k[4]);
    if fw.length_squared() > 1.0e-6 {
        s.yaw = crate::sora::wrap_degrees(fw.y.atan2(fw.x).to_degrees());
    }
}

/// The tower's hit windows (its own A417: FinishWA SCWA_TOWER 6.2 .. 8.6 s, its hit list cleared every 0.8 s; FinishWA2
/// SCWA_TOWER2 8.8 .. 9.0 s) with its TresAtkColl0 CAPSULE1 shapes placed by their baked tracks (track.tower.<group>:
/// the centre and the axis of ATK_00 + m_RelativeLocation1 turned by m_RelativeRocation, under A417), half length
/// m_Size.Z, radius m_Size.X growing by m_IncSize.X / s from the window's activation to m_IncMaxSize.X (FinishWA: 500 ->
/// 3000; FinishWA2: 3000). One shot per hit-list period, the radius going on growing from where the window had it.
fn tower_hits(f: &mut StyleFinish, cx: &mut Ctx) {
    let Some(tower) = cx.pawn.combat.staff.tower else { return };
    let Some(pawn) = wa::row("pawn.w_so073") else { return };
    for (k, (window, rowgroup)) in pawn.windows.iter().zip(pawn.rows.iter()).enumerate() {
        let Some((span, interval)) = window.split_once('@') else { continue };
        let Some((a, b)) = span.split_once('-') else { continue };
        let (a, b, interval): (f32, f32, f32) = (a.parse().unwrap_or(0.0), b.parse().unwrap_or(0.0), interval.parse().unwrap_or(0.0));
        let (row, group) = rowgroup.split_once('@').unwrap_or((rowgroup, "FinishWA"));
        let Some(shape) = wa::row(&format!("shape.w_so073.{group}")) else { continue };
        let v = |i: usize| shape.values.get(i).copied().unwrap_or(0.0);
        // Activations: at a, then every interval while the window is open (bit per activation in `count`).
        let n = if interval > 0.0 { ((b - a) / interval).ceil() as i32 } else { 1 };
        for i in 0..n.clamp(0, 15) {
            let at = a + i as f32 * interval.max(0.0);
            let bit = 1i32 << (k.min(1) * 15 + i as usize);
            if tower.time < at || tower.time >= b || f.count & bit != 0 {
                continue;
            }
            f.count |= bit;
            // (AUDIT-A STAFF) the window's NotifyBegin (its first activation): the tower's anim interface +0x200 sets +0xB48
            if i == 0 {
                if let Some(t) = cx.pawn.combat.staff.tower.as_mut() {
                    t.running = true;
                }
            }
            let Some(k) = wa::tower_track(group, tower.time) else { continue };
            let grown = v(0) + v(6) * (tower.time - a);
            let max = if v(9) > 0.0 { v(9) } else { grown };
            let size = (grown.min(max), v(6), max);
            let life = if interval > 0.0 { interval.min(b - at) } else { b - at };
            let (sin, cos) = tower.yaw.to_radians().sin_cos();
            let yawed = |x: f32, y: f32, z: f32| Vec3::new(x * cos - y * sin, x * sin + y * cos, z);
            let centre = tower.position + yawed(k[0], k[1], k[2]);
            let axis = yawed(k[3], k[4], k[5]) * v(2);
            let row: &'static str = crate::combat::tables::attack_row(row).map_or("None", |(_, r)| r.id);
            f.a.sync_ctx(cx.pawn);
            wa::spawn_capsule_shot(cx, group_name(group), row, centre, axis, size, life);
        }
    }
}

fn group_name(group: &str) -> &'static str {
    if group == "FinishWA2" { "FinishWA2" } else { "FinishWA" }
}


