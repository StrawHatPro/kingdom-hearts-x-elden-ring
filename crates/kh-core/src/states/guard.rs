//! `TresPlayerState_Guard` (id 15), its request `TryGuard`, and the guarded hit `OnGuardedHit`:
//! docs/sora-actions-spec.md II.1 "Guard" (instruction level; addresses are the exe's). Plain
//! Keyblade (weapon kind 1): the form guards (slots 118 / 119) and the swim variant are not here.
//!
//! A player-requested guard guards with the collision parts `Guard` / `GuardBody` from Begin to
//! `TresTiming(0)` (0.6 s); a guard entered BY a guarded hit has reaction type 3 on `Body` (all
//! directions) until its `TresEnableChange` (0.7 s; air 0.6667 s). What a hit meets is decided in
//! `actions::classify`; the counter (reprisal) window and the just-guard timer are controller
//! fields in `PawnActions` (the counter attacks are the combat implementer's).

use glam::Vec3;

use super::common::{on_ground, turn};
use super::{HitOnPlayer, PlayerState, StateData, StateId};
use crate::actions::{self, reaction, A};
use crate::anim::{AnimId, Notifies, NotifyClass, NotifyData, NotifyPhase, PlayArgs};
use crate::enums::ETresAbilityKind as Ability;
use crate::host::{EventKind, TargetId};
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;

/// COORDINATOR DECISION 2026-10-11: `false`, the decompiled code's reading (the jump out of a
/// guarded hit starts at once from `TresEnableInput`; probe request G-J1 in
/// docs/sora-runtime-probe.md 8b). The first-pass text of II.1 said a
/// jump requested out of a guard is QUEUED and starts when the guard clip ends, because the
/// Guard `TresEnableChange` handler never sets `m_bEnableChange` (slot 117: "3 JUMP: DoChain
/// only if state+0x84 & 1"). But `JumpRequest` 0x14143D8F0 (cases 15 / 16 -> LAB_14143e424) calls
/// `ClearQueue` 0x1405A8010 first, which empties the WHOLE queue (its loop starts at index 0 and
/// the count becomes 0), and `NeedChange` (movement spec 1.3) then replaces the running state at
/// the next `MainProc`. `true` = the spec's reading (the jump is queued behind the guard);
/// `false` = the code as read (the jump starts at once from `TresEnableInput`).
pub(crate) const JUMP_OUT_OF_GUARD_AT_CLIP_END: bool = false;

/// State +0xE8 flags and the fields of II.1's table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Guard {
    sub: u8,
    /// +0xDC: entered by a guarded hit.
    pub(super) hit_entry: bool,
    /// Flag bit 0: a player request. Bit 1: success (a player guard caught the hit).
    player_request: bool,
    success: bool,
    /// Bit 3: push-back active. Bit 4: airborne.
    pushback: bool,
    air: bool,
    /// m_bEnableInput (state +0x84 bit 5): the hit clip's `TresEnableInput` has fired.
    pub(super) enable_input: bool,
    /// +0xF4 / +0xF8: turn rate and turn timer (Reset 0.2 / 0.2).
    turn_rate: f32,
    turn_timer: f32,
    /// +0xFC / +0x100: push-back speed and its decay per 1/60 s (Reset 0 / 0.95).
    push_speed: f32,
    decay: f32,
    /// +0x10C: hit strength 1 / 2.
    strength: u8,
    /// +0x110: the attacker.
    attacker: TargetId,
    attacker_position: Vec3,
    /// +0x128 look-at point, +0x134 push-back direction, +0x14C the hit direction flattened.
    look_at: Vec3,
    push_dir: Vec3,
    hit_dir: Vec3,
    /// The copied damage info: `m_DamagePowerScale`, `m_DamageBrakeParam`, `m_DamageKind`.
    power_scale: f32,
    brake: f32,
    damage_kind: u8,
}

impl Guard {
    /// The state object after `Reset` (0x14138F510).
    pub const RESET: Guard = Guard {
        sub: 0,
        hit_entry: false,
        player_request: false,
        success: false,
        pushback: false,
        air: false,
        enable_input: false,
        turn_rate: 0.2,
        turn_timer: 0.2,
        push_speed: 0.0,
        decay: 0.95,
        strength: 0,
        attacker: TargetId::NONE,
        attacker_position: Vec3::ZERO,
        look_at: Vec3::ZERO,
        push_dir: Vec3::ZERO,
        hit_dir: Vec3::ZERO,
        power_scale: 1.0,
        brake: 0.0,
        damage_kind: 0,
    };

    /// Slot 116 `SetGuardParts(bOn)` (0x1413A0650).
    fn set_parts(s: &mut Pawn, on: bool) {
        s.actions.guard_parts = on;
    }

    /// Push-back (slot 115 0x14138E820): `m_GuardHitBackPowerLv1 / Lv2` = 300 / 1000 by the hit
    /// strength (the asset does not override the native 300 / 1000 / 3000; Begin produces
    /// strengths 1 and 2 only). pawn+0xD84 (which selects a second formula) is not known: the
    /// `else` branch is used (reported under "still needs the running game").
    fn start_pushback(&mut self) {
        self.decay = if self.brake > 0.0 { self.brake } else { A.guard_pushback_decay };
        let base = if self.strength == 2 { 1000.0 } else { 300.0 };
        self.push_speed = self.push_speed.max(base);
        if self.power_scale != 1.0 {
            self.push_speed *= self.power_scale;
        }
        self.pushback = true;
    }

    /// Slot 117 (0x14139C310), the Keyblade tick.
    fn keyblade_tick(&mut self, s: &mut Pawn) {
        if self.sub == 0 {
            if !self.hit_entry {
                // A player request: "AirGuard" when airborne, else "Guard" (blend-in -1, mode 2).
                s.anim.play_with(if self.air { AnimId::AirGuard } else { AnimId::Guard }, PlayArgs::mode(2, -1.0));
                self.turn_rate = 1.0;
                self.sub = 1;
                return;
            }
            // The hit entry (0x14139C6D8).
            s.anim.play_with(if self.air { AnimId::AirGuardHit } else { AnimId::GuardHit }, PlayArgs::mode(2, -1.0));
            self.sub = 2;
            if self.success {
                self.turn_rate = 1.0;
                // +0x134 = (+0x14C != 0) ? +0x14C : -forward2D(pawn).
                self.push_dir = if self.hit_dir != Vec3::ZERO {
                    self.hit_dir
                } else {
                    let a = s.yaw.to_radians();
                    -Vec3::new(a.cos(), a.sin(), 0.0)
                };
            } else {
                // Without success the direction stays zero: he is held in place.
                (self.turn_rate, self.turn_timer) = (0.05, 0.05);
            }
            self.start_pushback();
            self.sub = 3;
            return;
        }
        if self.sub == 2 {
            self.start_pushback();
            self.sub = 3;
        }
        // Every tick from sub-step 1 on (0x14139C65B).
        if s.anim.ended {
            s.queue.do_chain();
            return;
        }
        match s.queue.queued_id() {
            // JUMP: only with m_bEnableChange, which the Guard never gets: at the clip end.
            Some(StateId::Jump) if s.queue.flags.enable_change => s.queue.do_chain(),
            Some(StateId::Dodgeroll) | Some(StateId::AirSlide) => s.queue.do_chain(),
            _ => {}
        }
    }
}

/// `CanGuard` (slot 54 0x1414294C0), the rows of the states this port has.
fn can_guard(state: &StateData, s: &Pawn) -> bool {
    // Air guard blocked (vt+0x1E30): pawn+0xE3C > 0 || !(pawn+0xE48 bit 2).
    if s.combat.air_guard_cooldown > 0.0 || !s.actions.air_guard_ok {
        return false;
    }
    // Next queued: nothing, an attack, or a Guard (table 0x141429B14).
    if !matches!(s.queue.queued_id(), None | Some(StateId::Attack) | Some(StateId::Guard)) {
        return false;
    }
    // Weapon kind 1: HasAbility(3 REFLECT_GUARD) (0x1414295C2).
    if !actions::has(s, Ability::ReflectGuard) {
        return false;
    }
    match state {
        StateData::Idle(_) | StateData::Run(_) | StateData::Glide(_) | StateData::Floating(_) => true,
        // JUMP: Jump+0xF8 == 0 (not a flow jump).
        StateData::Jump(j) => !j.flow,
        // Attack states: Attack+0x234 bit 7 (0x1414296EF).
        StateData::Attack(a) => a.cancel,
        StateData::Guard(g) => g.hit_entry && g.enable_input,
        StateData::DodgeRoll(_) | StateData::AirSlide(_) => s.queue.flags.enable_change,
        // (magic implementer) FIREMAGIC 18: !(+0xD8 & 0x10) && m_bEnableInput && !(+0xD8 & 0x40)
        // && m_bEnableChange; USE_ITEM 111: m_bEnableChange && OnGround (actions spec II.1 table).
        StateData::FireMagic(m) => m.can_guard() && s.queue.flags.enable_change,
        StateData::UseItem(_) => s.queue.flags.enable_change && on_ground(s),
        // DAMAGE: +0x90 == 0 && kinds 1, 2, 9 && m_bEnableChange (table 0x141429CBC).
        StateData::Damage(d) => matches!(d.kind.value(), 1 | 2 | 9) && s.queue.flags.enable_change,
        #[allow(unreachable_patterns)]
        _ => false,
    }
}

/// `TryGuard` (slot 55 0x14143C6D0): the gate; a queued attack is dropped; `SetState(15)` with
/// flag bit 0 (a player request).
pub(crate) fn try_guard(state: &mut StateData, cx: &mut Ctx) -> bool {
    let s = &mut *cx.pawn;
    if !can_guard(state, s) {
        return false;
    }
    // controller+0x878 bit 8 (a just guard pending): cleared (FUN_1414142D0(ctl, 0)).
    s.actions.critical = false;
    s.actions.guard_hit_pending = false;
    s.queue.set_state(Guard { player_request: true, ..Guard::RESET });
    true
}

/// `OnGuardedHit(info, _, causer, hitDir)` (slot 142 0x14143CAF0), called for hit result 5.
/// Returns false when its gate (slot 141 0x141429E70) refuses. Not here: the Reflect branch
/// (`m_ReflectLevel` >= 3 -> state 66; the hit record has no reflect level, and Reflect is tier
/// 2), the automatic-guard option, the Attack state's guard bit (+0x23C bit 1).
pub(crate) fn on_guarded_hit(state: &mut StateData, cx: &mut Ctx, hit: &HitOnPlayer) -> bool {
    let s = &mut *cx.pawn;
    // Gate: next queued none or a Guard; current GUARD passes; every other state would need the
    // automatic-guard option (off) or the attack's guard bit.
    if !matches!(s.queue.queued_id(), None | Some(StateId::Guard)) {
        return false;
    }
    let StateData::Guard(cur) = state else { return false };
    let (mut success, mut old) = (false, None);
    let critical = if cur.player_request && !cur.hit_entry {
        // A player guard that has not guarded yet (0x14143D0D3).
        success = true;
        s.actions.just_guard_timer > 0.0
    } else {
        old = Some((cur.push_speed, cur.push_dir, cur.look_at));
        s.actions.critical
    };
    // FUN_1414145C0(controller, 1.0, attacker, success, critical): the counter window.
    actions::open_counter_window(s, A.guard_counter_window, hit.causer, critical);
    // The hit direction, flattened and normalised (zero if |xy|^2 < 1e-8); a record without a
    // direction pushes away from the attacker.
    let mut dir = Vec3::new(hit.direction.x, hit.direction.y, 0.0);
    if dir.length_squared() < 1.0e-8 {
        let away = s.cmc.location - hit.causer_position;
        dir = Vec3::new(away.x, away.y, 0.0);
    }
    let hit_dir = if dir.length_squared() < 1.0e-8 { Vec3::ZERO } else { dir.normalize() };
    let mut next = Guard {
        hit_entry: true,
        // Flag bit 0 survives only in the instance the player requested: the new one is a hit entry.
        success,
        attacker: hit.causer,
        attacker_position: hit.causer_position,
        hit_dir,
        power_scale: hit.power_scale,
        brake: hit.brake,
        damage_kind: hit.kind.value() as u8,
        ..Guard::RESET
    };
    // The push-back is carried over consecutive hits.
    if let Some((speed, dir, look_at)) = old {
        if speed > 0.0 {
            (next.push_speed, next.push_dir, next.look_at) = (speed, dir, look_at);
        }
    }
    s.actions.guard_hit_pending = true;
    s.actions.hits_guarded += 1;
    s.actions.last_attacker = Some((hit.causer, hit.causer_position));
    s.queue.set_state(next);
    // GUARD_REGENE (74, Block Replenisher): pawn.vt+0x1878(0, 3, 0, 0, 0).
    let mp = crate::abilities::passive::block_replenisher_mp(&s.status);
    if mp > 0 {
        s.status.recover(0, mp, 0, 0);
    }
    s.event(EventKind::PlayerGuarded { hit: *hit, success, critical });
    // (XSI) CRITICAL COUNTER's Elden Ring addition (XS 2.10): a just guard wears a STRONG body's endurance.
    if critical {
        crate::abilities::xs::just_guard(cx, hit.causer);
    }
    // (XSI) RAGE RAID (XS 2.11): a block against a body whose revenge runs offers Raid.
    if success {
        crate::abilities::xs::rage::on_guarded(cx, hit.causer);
    }
    true
}

/// The guard reaction (request interface slot 140 0x141444C30, decompiled C; called by the Attack
/// state's recoil for attacker result GUARD_REFLECT behind the counter-allowed gate, combat spec
/// 5.6.3): `SetState(15)`; the counter window `FUN_1414145C0(controller, 1.0, causer's pawn, 0, 0)`;
/// the new Guard: flags |= 2 (success), +0x110 = causer, +0xDC = 1 (a hit entry: `GuardHit`,
/// reaction type 3 on the body, the push-back away from the causer); pawn vt+0x1048(0, 0, 0).
/// Returns false when no Guard state can be set (never here).
pub(crate) fn guard_reaction(cx: &mut Ctx, causer: TargetId, causer_position: Vec3) -> bool {
    let s = &mut *cx.pawn;
    actions::open_counter_window(s, A.guard_counter_window, causer, false);
    let away = (s.cmc.location - causer_position) * Vec3::new(1.0, 1.0, 0.0);
    let hit_dir = if away.length_squared() < 1.0e-8 { Vec3::ZERO } else { away.normalize() };
    s.actions.guard_hit_pending = true;
    s.actions.last_attacker = Some((causer, causer_position));
    s.queue.set_state(Guard { hit_entry: true, success: true, attacker: causer, attacker_position: causer_position, hit_dir, damage_kind: 1, ..Guard::RESET });
    true
}

/// `CanAttack` case GUARD without an open counter window (0x141426832, combat spec 3.2): a plain
/// attack out of the guard-hit clip after its `TresEnableInput`: "(queued is not an Attack &&
/// Guard+0xDC != 0 && Guard+0x84 & 0x20) || (queued == null && ...)".
pub(crate) fn attack_out_of_hit(state: &StateData, s: &Pawn) -> bool {
    matches!(state, StateData::Guard(g) if g.hit_entry && g.enable_input) && !matches!(s.queue.queued_id(), Some(StateId::Attack))
}

impl PlayerState for Guard {
    const ID: StateId = StateId::Guard;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// Begin 0x1413851D0.
    fn begin(&mut self, cx: &mut Ctx) {
        cx.pawn.actions.last_pose = 1;
        actions::draw(cx, true);
        // The look-at point: 100 cm ahead; the attacker (a successful hit entry); else the
        // controller's lock-on target.
        let lock = cx.pawn.combat.lockon.current;
        let lock_position = (lock != TargetId::NONE).then(|| cx.target(lock)).flatten().map(|t| t.position);
        let attacker_now = (self.attacker != TargetId::NONE).then(|| cx.target(self.attacker)).flatten().map(|t| t.position);
        let s = &mut *cx.pawn;
        s.actions.guard_hit_pending = false;
        s.cmc.stop_movement();
        if !on_ground(s) {
            self.air = true;
            s.actions.air_guard_ok = false;
        }
        actions::set_reaction(s, reaction::GUARD);
        let a = s.yaw.to_radians();
        let carried = self.look_at != Vec3::ZERO;
        if !carried {
            self.look_at = s.cmc.location + Vec3::new(a.cos(), a.sin(), 0.0) * A.guard_lookat_distance;
        }
        if self.attacker != TargetId::NONE {
            if self.success {
                self.look_at = attacker_now.unwrap_or(self.attacker_position);
            }
        } else if let Some(position) = lock_position {
            self.look_at = position;
        }
        // The Keyblade variant (0x1413856EE): a player guard takes the reaction type back at
        // once and switches the parts instead.
        if !self.hit_entry {
            actions::restore_reaction(s);
            Guard::set_parts(s, true);
        }
        // +0x10C = (m_DamageKind in {2,3,4,5,7,8,9,10}) ? 2 : 1 (table 0x1413858DC).
        self.strength = if matches!(self.damage_kind, 2 | 3 | 4 | 5 | 7 | 8 | 9 | 10) { 2 } else { 1 };
        if self.air {
            s.cmc.gravity_off = true;
        }
        if !self.hit_entry {
            // FUN_141414B70: the just-guard timer, with CRITICAL_COUNTER and not swimming.
            if s.status.count(Ability::CriticalCounter) != 0 {
                s.actions.just_guard_timer = A.guard_just_guard_time;
                s.actions.critical = false;
            }
        } else {
            // pawn.vt+0x1E28(0.0): the air-guard cool-down is cleared.
            s.combat.air_guard_cooldown = 0.0;
        }
    }

    /// Tick 0x141392080.
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        let s = &mut *cx.pawn;
        self.keyblade_tick(s);
        // comiss 0,[+0xF8]; jae skip.
        if self.turn_timer > 0.0 {
            let d = (self.look_at - s.cmc.location).truncate();
            if d.length_squared() > 1.0e-8 {
                turn(s, d.y.atan2(d.x).to_degrees(), dt * self.turn_rate * 60.0);
            }
            self.turn_timer = (self.turn_timer - dt).max(0.0);
        }
        if self.pushback {
            // +0xFC = pow(+0x100, dt * 60) * +0xFC; pawn.vt+0x16C0(+0xFC * +0x134): velocity XY.
            self.push_speed *= self.decay.powf(dt * 60.0);
            s.cmc.set_velocity_xy((self.push_dir * self.push_speed).truncate());
        }
    }

    /// End 0x14138CA10.
    fn end(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        let p = *s.params();
        s.cmc.gravity_off = false;
        s.cmc.gravity_scale = p.gravity_scale;
        s.cmc.max_walk_speed = p.speed_run;
        s.cmc.direct_xy = None;
        if self.air {
            // SetMovementMode(3 Falling, 0), also when he has landed meanwhile: the movement
            // lands him again next tick (and the land pose of `AirGuard` gives `LightLand`).
            s.cmc.set_falling();
            if !self.hit_entry {
                s.combat.air_guard_cooldown = A.guard_air_cooldown;
            }
        }
        // A next state that is not a hit-entry Guard closes an open counter window.
        if s.queue.queued_id().is_some() && !s.actions.guard_hit_pending && s.actions.counter_window > 0.0 {
            actions::close_counter_window(s);
        }
        actions::restore_reaction(s);
        Guard::set_parts(s, false);
    }

    /// `TresTiming(param)` 0x14139AD70 (only param 0), `TresEnableChange` 0x14139A4E0,
    /// `TresEnableInput` (the base handler).
    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        let s = &mut *cx.pawn;
        for event in &notifies.events {
            if event.phase != NotifyPhase::Instant {
                continue;
            }
            match event.class() {
                NotifyClass::TresAnimNotifyTiming => {
                    if let NotifyData::Param { param } = event.def().data {
                        if param.unwrap_or(0) == 0 {
                            actions::restore_reaction(s);
                            Guard::set_parts(s, false);
                        }
                    }
                }
                NotifyClass::TresAnimNotifyEnableInput => self.enable_input = true,
                NotifyClass::TresAnimNotifyEnableChange => {
                    actions::restore_reaction(s);
                    Guard::set_parts(s, false);
                    // Gravity on, gravity scale reset; airborne and not a hit entry: the cool-down.
                    s.cmc.gravity_off = false;
                    s.cmc.gravity_scale = s.params().gravity_scale;
                    if !on_ground(s) && !self.hit_entry {
                        s.combat.air_guard_cooldown = A.guard_air_cooldown;
                    }
                }
                _ => {}
            }
        }
        // THE HANDLER DOES NOT SET m_bEnableChange (no write to interface+0x44): the host set it
        // for every state before this call; taken back.
        if notifies.enable_change {
            s.queue.flags.enable_change = false;
        }
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}
