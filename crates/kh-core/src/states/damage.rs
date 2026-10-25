//! Sora being hit: the victim side of `TakeDamage` (0x1405BB850), the reaction gate (request
//! interface slot 88 0x141439A50), `TresPlayerState_Damage` (id 19) with its Small / Blow / Toss
//! / Launch / Parabola / Beat handlers, and `TresPlayerState_Die` (id 59). Written from the
//! second research pass, strand E (`build/sora_tmp/combat/P2_E_section.md`, which replaces
//! sections 8.4 and 9 of docs/sora-combat-spec.md; "E 9.x" below).
//!
//! The HP rule (`ApplyHP` 0x1405A2A00: Second Chance, Withstand Combo) is the status
//! implementer's `Status::apply_damage_to_sora`; this file calls it and keeps its
//! `in_damage_state` hook current.
//!
//! Second pass (2026-10-11): what a hit meets is classified here first (`combat::guard`: the
//! 33-row table with Sora as the victim; results REFLECT / GUARD_REFLECT -> the weapon recoil,
//! GUARD -> `OnGuardedHit`), the wall hit of Blow / Parabola (event interface slot 9
//! 0x141356930 on the movement's blocking hit), Beat's slide-out, the auto-revive item, the
//! bad-status states as named stubs (`states/badstat_*.rs`), the reaction-block timer.
//!
//! Not here (each a TODO with its owner):
//! - the Frozen, Suction and Eat handlers; rails under Sora, swimming exits, the "floating" flag;
//! - Aerial Recovery: the state `AIR_SLIDE` (20) is the actions implementer's; the window in
//!   which it may be requested is published as `DamageView::recoverable` and a jump press in it
//!   is noted, nothing else happens;
//! - the auto-revive item (Kupo Coin, `Survive()` slot 575), RESCUE / GAMEOVER: the host gets the
//!   `PlayerDied` event and decides.

use glam::Vec3;

use super::common::{on_ground, turn};
use super::{PlayerState, StateData, StateId};
use crate::anim::{AnimId, PlayArgs};
use crate::enums::{ETresDamageAttribute, ETresDamageKind};
use crate::combat::tables::P;
use crate::host::{EventKind, ReactionResult, TargetId};
use crate::pawn::{Ctx, Pawn};
use crate::sora::{wrap_degrees, MovementState};
use crate::status::HitOnSora;

/// `PlayerDamageCommonParam` (the game mode's `m_PlayerDamageCommonParam`; E 9.3 "Parameters":
/// native constructor 0x14064C130 overlaid with the asset; getters slots 127 - 150).
pub mod param {
    pub const SMALL_POWER: f32 = 550.0;
    pub const SMALL_BRAKE: f32 = 0.9;
    pub const SMALL_START_FALLING_SPEED: f32 = 150.0;
    pub const AIR_SMALL_VELOCITY_Z: f32 = 0.0;
    /// `m_AirBlowDamagePower`: read on the ground and in the air alike.
    pub const BLOW_POWER: f32 = 2000.0;
    pub const BLOW_VELOCITY_Z: f32 = 300.0;
    pub const BLOW_BRAKE: f32 = 0.95;
    pub const BLOW_START_FALLING_SPEED: f32 = 700.0;
    pub const TOSS_POWER: f32 = 1500.0;
    pub const LAUNCH_POWER: f32 = 2500.0;
    pub const FLIGHT_DURATION: f32 = 0.2;
    pub const FALL_GRAVITY_SCALE: f32 = 1.0;
    pub const PARABOLA_POWER: f32 = 1500.0;
    pub const PARABOLA_ANGLE: f32 = 45.0;
    pub const PARABOLA_BRAKE: f32 = 0.98;
    pub const PARABOLA_LIMIT_BRAKE: f32 = 300.0;
    pub const BEAT_POWER: f32 = 2500.0;
    pub const BEAT_REBOUND: f32 = 1000.0;
    pub const BEAT_REBOUND_GRAVITY_SCALE: f32 = 3.0;
    /// state+0x138: Aerial Recovery may be requested from this long after the hit.
    pub const RECOVER_DELAY: f32 = 0.2;
    /// Every handler turns with alpha = dt x 30.
    pub const TURN_RATE: f32 = 30.0;
    /// Die: `SetVel3(knock x 2000, z = 125)`, horizontal x 0.95 per 60 Hz frame.
    pub const DIE_POWER: f32 = 2000.0;
    pub const DIE_VELOCITY_Z: f32 = 125.0;
    pub const DIE_BRAKE: f32 = 0.95;
}

/// A hit on Sora, as a host delivers it (`Sora::take_hit`): the fields of the game's hit record
/// the victim side reads (`FTresPointDamageEvent` + `TresDamageInfo`: strand C 8.7, E 8.4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitOnPlayer {
    /// The damage, final (`status::compute_damage` with Sora as the defender, or the host's own).
    pub amount: f32,
    pub kind: ETresDamageKind,
    pub attribute: ETresDamageAttribute,
    /// The knockback direction (`ShotDirection`); zero = along the causer's facing.
    pub direction: Vec3,
    /// The attacker: the host's id, its position, facing (degrees) and body radius.
    pub causer: TargetId,
    pub causer_position: Vec3,
    pub causer_yaw: f32,
    pub causer_radius: f32,
    /// `m_DamagePowerScale`, `m_DamageBrakeParam` (0 = the default), `m_DamageParabolaAngle`
    /// (0 = default), `m_DamageMoveLength`, `m_DamageEffectTime`, `m_AttackSrcPower`.
    pub power_scale: f32,
    pub brake: f32,
    pub parabola_angle: f32,
    pub move_length: f32,
    pub effect_time: f32,
    pub src_power: f32,
    /// `info+0x60`, the body-collision reaction type the hit was resolved with: 2 = default
    /// (damage and reaction), 1 = no reaction, 3 - 6 = guarded / invincible (no damage).
    pub reaction_type: u8,
    // ---- the row facts the classification needs (strand C 8.2.6; all "no" by default)
    /// `m_bIsIvalidGuard` (info+0x50 bit 3): the attack ignores guards.
    pub ignore_guard: bool,
    /// `m_bIsEnableWeakGuardAttack` (info+0x51 bit 3): a WEAK_GUARD part guards it.
    pub weak_guard: bool,
    /// `m_bRapidFire` (info+0x50 bit 4).
    pub rapid_fire: bool,
    /// `m_ReflectLevel` (info+0x63): > 0 = the attacker can be repelled by a guard; >= 3 breaks
    /// a guard into the weapon recoil.
    pub reflect_level: i8,
    /// `m_ReflectReaction` (info+0x62): "DEFAULT", "STAGGER", "GUARD", "NOREACTION".
    pub reflect_reaction: &'static str,
    /// The bad status asked for (info+0x34 / +0x38): "NONE", "FREEZE", "BURN", "STUN" ... and its time.
    pub bad_status: &'static str,
    pub bad_time: f32,
    /// `m_BadStatusParam` (info+0x3C): HONEY's slow-down per application.
    pub bad_param: f32,
}

impl HitOnPlayer {
    /// A hit of `kind` for `amount` from a causer at `causer_position`, pushing away from it.
    pub fn new(kind: ETresDamageKind, amount: f32, causer: TargetId, causer_position: Vec3) -> HitOnPlayer {
        HitOnPlayer { amount, kind, attribute: ETresDamageAttribute::Physical, direction: Vec3::ZERO, causer, causer_position, causer_yaw: 0.0, causer_radius: 40.0, power_scale: 1.0, brake: 0.0, parabola_angle: 0.0, move_length: 0.0, effect_time: 0.0, src_power: 1.0, reaction_type: 2, ignore_guard: false, weak_guard: false, rapid_fire: false, reflect_level: 0, reflect_reaction: "DEFAULT", bad_status: "NONE", bad_time: 0.0, bad_param: 0.0 }
    }
}

/// Which handler the damage kind selected (E 9.3, jump table 0x1413662A8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handler {
    Small,
    Blow,
    Toss,
    Launch,
    Parabola,
    Beat,
}

fn handler_of(kind: ETresDamageKind) -> Handler {
    match kind.value() {
        2 | 21 => Handler::Blow,
        7 | 22 => Handler::Toss,
        8 => Handler::Launch,
        10 => Handler::Parabola,
        9 => Handler::Beat,
        // TODO(combat E): 19 SUCTION, 20 EAT, 41 PUDDING_EAT have handlers of their own.
        _ => Handler::Small,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Damage {
    pub(super) sub: u8,
    /// +0x88: the damage kind (after the dead-conversion).
    pub kind: ETresDamageKind,
    handler: Handler,
    /// +0x8D, +0x8E.
    stick: bool,
    pub recoverable: bool,
    /// The info's power scale, brake, parabola angle, move length, effect time.
    scale: f32,
    info_brake: f32,
    info_angle: f32,
    move_length: f32,
    effect_time: f32,
    /// +0x110: the hit direction, normalised in XY.
    dir: Vec3,
    /// +0x12C, +0x130.
    since: f32,
    timer: f32,
    /// +0x13C bit 0, +0x140, +0x1A0.
    killed: bool,
    pub hits: i32,
    variant: bool,
    clip: Option<AnimId>,
    /// +0x150, +0x15C, +0x168, +0x174, +0x178, +0x17C, +0x190.
    start: Vec3,
    knock: Vec3,
    facing: f32,
    power: f32,
    brake: f32,
    vel_z: f32,
    index: u8,
    /// +0x1C0 bits 0 - 2.
    was_flying: bool,
    brake_on: bool,
    landed: bool,
    causer_position: Vec3,
    causer_radius: f32,
    /// +0x8F: the wall hit is armed (Blow sub 3 -> 4, Parabola sub 3 -> 4).
    wall_armed: bool,
    /// The wall clip plays (sub 10).
    pub wall: bool,
    causer: TargetId,
    /// +0x90: a bad status (freeze / burn / stun) is on the pawn: no attack out of the state, and
    /// it ends at its first exit point (spec 9.3 Setup, 9.9).
    pub bad: bool,
}

pub(super) fn yaw_of(v: Vec3) -> f32 {
    v.y.atan2(v.x).to_degrees()
}

/// The direction index (pawn vt+0x23F0 0x1405A94E0; E 9.3): where the attacker is. 0 in front,
/// 1 behind, 2 to the right, 3 to the left; the two-way variant gives 0 / 1.
pub fn direction_index(dir: Vec3, actor_yaw: f32, two_way: bool) -> u8 {
    if dir.truncate().length_squared() < 1.0e-8 {
        return 0;
    }
    let delta = wrap_degrees(yaw_of(-dir) - actor_yaw);
    if two_way {
        return if (-90.0..=90.0).contains(&delta) { 0 } else { 1 };
    }
    if (-45.0..=45.0).contains(&delta) {
        0
    } else if delta > 45.0 && delta <= 135.0 {
        2
    } else if (-135.0..-45.0).contains(&delta) {
        3
    } else {
        1
    }
}

fn anim(number: u32) -> Option<AnimId> {
    AnimId::from_name(&format!("{number:04}"))
}

/// `Small(index, variant)` (pawn vt+0x1CB0 0x141435200).
fn small_clip(index: u8, variant: bool, grounded: bool) -> Option<AnimId> {
    if !grounded {
        return anim(207 + index as u32);
    }
    match (index, variant) {
        (0, true) => anim(205),
        (1, true) => anim(206),
        _ => anim(201 + index as u32),
    }
}

/// `Wall(index)` (pawn vt+0x1CC8 0x141433F10): `M_0223` - `M_0226`.
fn wall_clip(index: u8) -> Option<AnimId> {
    AnimId::from_name(&format!("M_{:04}", 223 + index as u32))
}

/// `Blow(index, phase)` (pawn vt+0x1CB8 0x1414333A0): start, loop, down and get-up.
pub(super) fn blow_clip(index: u8, phase: u32) -> Option<AnimId> {
    anim(211 + 3 * index as u32 + phase)
}

impl Damage {
    fn play(&mut self, s: &mut Pawn, clip: Option<AnimId>, mode: u8, blend_in: f32) -> bool {
        self.clip = clip;
        match clip {
            Some(clip) => {
                s.anim.play_with(clip, PlayArgs::mode(mode, blend_in));
                true
            }
            None => false,
        }
    }

    fn set_index(&mut self, s: &Pawn, two_way: bool) {
        self.index = direction_index(self.dir, s.yaw, two_way);
        let a = s.yaw.to_radians();
        self.knock = if self.dir.truncate().length_squared() < 1.0e-8 { -Vec3::new(a.cos(), a.sin(), 0.0) } else { self.dir };
        self.facing = if self.index == 1 { yaw_of(self.knock) } else { yaw_of(-self.knock) };
    }

    /// `Brake(b)` (0x140431E10).
    fn brake_tick(&self, s: &mut Pawn, b: f32, dt: f32) {
        let v = s.cmc.velocity.truncate() * b.powf(dt * 60.0);
        s.cmc.set_velocity_xy(v);
    }

    /// The move-length start (0x14135FD80), at the end of every sub-step 0.
    fn move_length_start(&self, s: &mut Pawn) {
        if !(self.move_length > 0.0) {
            return;
        }
        let mut u = self.knock * self.power;
        if self.vel_z != 0.0 {
            u.z = self.vel_z;
        }
        let u = u.normalize_or_zero();
        if u.z > 0.0 {
            s.cmc.gravity_off = true;
            s.cmc.set_falling();
        }
        let v = u * self.move_length * 60.0;
        s.cmc.set_velocity_xy(v.truncate());
        s.cmc.set_velocity_z(v.z);
    }

    /// The common exits (E 9.3; the tail of every get-up sub-step): true = the state finishes.
    fn exits(&self, s: &mut Pawn) -> bool {
        if s.queue.flags.enable_change && (self.stick || self.bad) {
            s.cmc.stop_movement();
            return true;
        }
        false
    }

    /// One call of the kind's handler; true = finished.
    fn handle(&mut self, cx: &mut Ctx, dt: f32) -> bool {
        // The wall hit (event interface slot 9 0x141356930, called with the blocking hit;
        // thresholds at instruction level, the rest decompiled C): armed, horizontal speed >= 50,
        // knock . hitNormal <= -cos(30 deg).
        if self.wall_armed && matches!(self.handler, Handler::Blow | Handler::Parabola) {
            if let Some((hit, velocity)) = cx.pawn.cmc.last_block {
                let knock = self.knock.truncate().normalize_or_zero();
                let normal = Vec3::new(hit.impact_normal.x, hit.impact_normal.y, 0.0);
                if velocity.truncate().length() >= P.hit_wall_min_speed && normal.length_squared() > 1.0e-8 && knock.dot(normal.truncate().normalize()) <= -P.hit_wall_angle.to_radians().cos() {
                    let s = &mut *cx.pawn;
                    // Facing from the wall normal (index 1 faces the wall).
                    self.facing = if self.index == 1 { yaw_of(-normal) } else { yaw_of(normal) };
                    self.brake_on = false;
                    s.cmc.stop_movement();
                    s.cmc.gravity_off = true;
                    (self.recoverable, self.wall_armed, self.wall) = (false, false, true);
                    self.sub = 10;
                    let clip = wall_clip(self.index);
                    return !self.play(s, clip, 2, -1.0);
                }
            }
        }
        let s = &mut *cx.pawn;
        let brake = if self.info_brake > 0.0 { self.info_brake } else { self.brake };
        match self.handler {
            Handler::Small => {
                match self.sub {
                    0 => {
                        self.set_index(s, false);
                        self.power = param::SMALL_POWER * self.scale;
                        self.brake = param::SMALL_BRAKE;
                        self.vel_z = if on_ground(s) { 0.0 } else { param::AIR_SMALL_VELOCITY_Z };
                        self.move_length_start(s);
                        self.sub = 1;
                    }
                    1 => {
                        let grounded = on_ground(s);
                        let clip = small_clip(self.index, self.variant, grounded);
                        let v = self.knock.truncate() * self.power;
                        if !grounded && !s.cmc.is_flying() {
                            s.cmc.gravity_off = true;
                            s.cmc.set_velocity_xy(v);
                            s.cmc.set_velocity_z(self.vel_z);
                        } else {
                            if !grounded {
                                s.cmc.gravity_off = true;
                            }
                            s.cmc.set_velocity_xy(v);
                            self.brake_on = true;
                        }
                        self.sub = 4;
                        return !self.play(s, clip, 2, -1.0);
                    }
                    4 => {
                        turn(s, self.facing, dt * param::TURN_RATE);
                        if s.cmc.gravity_off {
                            if !on_ground(s) && !s.cmc.is_flying() {
                                let z = s.cmc.velocity.z * param::SMALL_BRAKE.powf(dt * 60.0);
                                s.cmc.set_velocity_z(z);
                            }
                            if s.cmc.velocity.truncate().length() <= param::SMALL_START_FALLING_SPEED {
                                s.cmc.gravity_off = false;
                            }
                        }
                        if on_ground(s) && self.clip != small_clip(self.index, self.variant, true) {
                            // Landed while an air clip plays.
                            if s.queue.flags.enable_change || s.anim.ended {
                                let land = AnimId::from_name("Land");
                                self.play(s, land, 2, -1.0);
                                s.queue.flags.enable_change = true;
                                self.sub = 5;
                            }
                        } else if s.anim.ended {
                            return true;
                        }
                    }
                    _ => {
                        if s.anim.ended {
                            return true;
                        }
                    }
                }
                // Tail.
                if self.exits(s) {
                    return true;
                }
                if self.brake_on {
                    self.brake_tick(s, param::SMALL_BRAKE, dt);
                }
                false
            }
            Handler::Blow => {
                match self.sub {
                    0 => {
                        self.set_index(s, self.killed);
                        self.power = param::BLOW_POWER * self.scale;
                        self.brake = param::BLOW_BRAKE;
                        self.vel_z = param::BLOW_VELOCITY_Z;
                        self.move_length_start(s);
                        self.sub = 1;
                        return false;
                    }
                    1 => {
                        s.cmc.gravity_off = true;
                        s.cmc.set_falling();
                        s.cmc.set_velocity_xy(self.knock.truncate() * self.power);
                        s.cmc.set_velocity_z(self.vel_z);
                        self.brake_on = true;
                        self.timer = 0.0; // AirBlowStartBrakeTime
                        self.sub = 2;
                        let clip = blow_clip(self.index, 0);
                        return !self.play(s, clip, 2, -1.0); // (no brake this tick)
                    }
                    2 | 3 => {
                        self.sub = 3;
                        turn(s, self.facing, dt * param::TURN_RATE);
                        // THE CODE AS READ (decompiled C of 0x14135B300, case 3, lines 253 - 279
                        // of build/sora_tmp/combat/E/14135B300.c.txt): while the START clip is the
                        // one playing and it has not ended, the case BREAKS before the timer
                        // test. Strand E's pseudocode leaves that break out, which (with
                        // AirBlowStartBrakeTime 0) would go to sub 4 on the first tick and never
                        // play the loop clip. So: the start clip (0.1 s) plays out, the loop
                        // starts, and only then the 0.6 s of flight and the wall hit are armed.
                        let mut wait = false;
                        if self.clip == blow_clip(self.index, 0) {
                            if s.anim.ended {
                                let clip = blow_clip(self.index, 1);
                                self.play(s, clip, 3, -1.0);
                            } else {
                                wait = true;
                            }
                        }
                        if !wait && self.timer <= 0.0 {
                            self.timer = 0.6;
                            // (pawn+0x15B4 != 0x12)
                            self.wall_armed = true;
                            self.sub = 4;
                        }
                    }
                    4 => {
                        turn(s, self.facing, dt * param::TURN_RATE);
                        if self.since > param::RECOVER_DELAY {
                            self.recoverable = true;
                        }
                        if s.cmc.gravity_off {
                            let z = s.cmc.velocity.z * brake.powf(dt * 60.0);
                            s.cmc.set_velocity_z(z);
                            if s.cmc.velocity.truncate().length() <= param::BLOW_START_FALLING_SPEED {
                                s.cmc.gravity_off = false;
                            }
                        } else if self.timer <= 0.0 {
                            if !self.landed {
                                if s.cmc.location.z < self.start.z - 50.0 {
                                    // Dropped off an edge: free fall.
                                    self.sub = 6;
                                    return true;
                                }
                            } else {
                                if self.was_flying {
                                    s.cmc.gravity_off = true;
                                    s.cmc.set_velocity_z(0.0);
                                }
                                let clip = blow_clip(self.index, 2);
                                self.play(s, clip, 2, -1.0);
                                self.sub = 5;
                                self.recoverable = false;
                            }
                        }
                    }
                    5 => {
                        turn(s, self.facing, dt * param::TURN_RATE);
                        // "if +0x90 || (m_bEnableChange && +0x8D): StopMovement; finish": with a
                        // bad status on him the get-up is cut at once (spec 9.4 Blow, 9.9.1).
                        if self.bad {
                            s.cmc.stop_movement();
                            return true;
                        }
                        if s.anim.ended || self.exits(s) {
                            return true;
                        }
                    }
                    // The wall hit: SetActorRotation(+0x168); the state finishes with the clip.
                    10 => {
                        s.yaw = wrap_degrees(self.facing);
                        return s.anim.ended;
                    }
                    _ => return true,
                }
                if self.brake_on && matches!(self.sub, 3 | 4 | 5) {
                    self.brake_tick(s, brake, dt);
                }
                false
            }
            Handler::Toss | Handler::Launch => {
                let launch = self.handler == Handler::Launch;
                match self.sub {
                    0 => {
                        self.power = if launch { param::LAUNCH_POWER } else { param::TOSS_POWER } * self.scale;
                        if launch && self.effect_time > 0.0 {
                            self.power = self.effect_time;
                        }
                        self.vel_z = self.power;
                        self.move_length_start(s);
                        self.sub = 1;
                    }
                    1 => {
                        s.cmc.gravity_off = false;
                        s.cmc.set_falling();
                        s.cmc.set_velocity_z(self.power);
                        s.cmc.gravity_scale = 3.0;
                        self.sub = 12;
                        return !self.play(s, AnimId::from_name("M_0227"), 0, -1.0);
                    }
                    12 => {
                        if s.anim.ended {
                            self.play(s, AnimId::from_name("M_0228"), 1, -1.0);
                            self.sub = 13;
                        }
                    }
                    13 => {
                        self.recoverable = self.since > param::RECOVER_DELAY;
                        if s.cmc.velocity.z <= 0.0 {
                            s.cmc.gravity_scale = 0.25;
                            self.timer = param::FLIGHT_DURATION;
                            self.sub = 14;
                        }
                    }
                    14 => {
                        self.recoverable = self.since > param::RECOVER_DELAY;
                        // (Tick subtracts dt as well: the 0.2 lasts 0.1 s.)
                        self.timer -= dt;
                        if self.timer <= 0.0 {
                            self.play(s, AnimId::from_name("M_0237"), 1, -1.0);
                            s.cmc.gravity_off = false;
                            s.cmc.gravity_scale = param::FALL_GRAVITY_SCALE;
                            self.timer = if launch { 1.2 } else { 1.0 };
                            self.sub = 15;
                        }
                    }
                    15 => {
                        self.recoverable = self.since > param::RECOVER_DELAY;
                        if self.landed {
                            s.cmc.stop_movement();
                            self.play(s, AnimId::from_name("0213"), 0, -1.0);
                            self.sub = 6;
                            self.recoverable = false;
                        } else if self.timer <= 0.0 {
                            // Still airborne: the state ends in the air.
                            s.cmc.stop_movement();
                            return true;
                        }
                    }
                    _ => {
                        if s.anim.ended || self.exits(s) {
                            return true;
                        }
                    }
                }
                false
            }
            Handler::Parabola => {
                let brake = if self.info_brake > 0.0 { self.info_brake } else { param::PARABOLA_BRAKE };
                match self.sub {
                    0 => {
                        self.set_index(s, self.killed);
                        self.power = param::PARABOLA_POWER * self.scale;
                        let angle = if self.info_angle > 0.0 { self.info_angle } else { param::PARABOLA_ANGLE }.to_radians();
                        self.vel_z = self.power * angle.sin();
                        self.move_length_start(s);
                        self.sub = 1;
                        return false;
                    }
                    1 => {
                        let angle = if self.info_angle > 0.0 { self.info_angle } else { param::PARABOLA_ANGLE }.to_radians();
                        s.cmc.gravity_off = false;
                        s.cmc.set_falling();
                        s.cmc.set_velocity_xy(self.knock.truncate().normalize_or_zero() * self.power * angle.cos());
                        s.cmc.set_velocity_z(self.power * angle.sin());
                        self.brake_on = true;
                        self.sub = 3;
                        let clip = blow_clip(self.index, 0);
                        return !self.play(s, clip, 2, -1.0);
                    }
                    3 => {
                        turn(s, self.facing, dt * param::TURN_RATE);
                        if s.anim.ended {
                            let clip = blow_clip(self.index, 1);
                            self.play(s, clip, 3, -1.0);
                            self.timer = 0.5;
                            self.sub = 4;
                            self.wall_armed = true;
                        }
                    }
                    10 => {
                        s.yaw = wrap_degrees(self.facing);
                        return s.anim.ended;
                    }
                    4 => {
                        self.recoverable = self.since > param::RECOVER_DELAY;
                        turn(s, self.facing, dt * param::TURN_RATE);
                        if self.timer <= 0.0 {
                            s.queue.flags.enable_change = true;
                            self.timer = 2.0;
                            self.sub = 5;
                        }
                    }
                    5 => {
                        turn(s, self.facing, dt * param::TURN_RATE);
                        if !on_ground(s) {
                            if self.timer <= 0.0 {
                                s.cmc.stop_movement();
                                self.brake_on = false;
                                self.wall_armed = false;
                                return true;
                            }
                        } else {
                            let clip = blow_clip(self.index, 2);
                            self.play(s, clip, 2, -1.0);
                            self.sub = 6;
                            self.recoverable = false;
                        }
                    }
                    _ => {
                        turn(s, self.facing, dt * param::TURN_RATE);
                        if s.anim.ended || self.exits(s) {
                            return true;
                        }
                    }
                }
                // "every tick: if +0x1C0 bit 1: if |Velocity.xy| <= LimitBrake() (300): +0x8F = 0
                // else Brake(brake)".
                if self.brake_on {
                    if s.cmc.velocity.truncate().length() <= param::PARABOLA_LIMIT_BRAKE {
                        self.wall_armed = false;
                    } else {
                        self.brake_tick(s, brake, dt);
                    }
                }
                false
            }
            Handler::Beat => {
                match self.sub {
                    0 => {
                        self.set_index(s, false);
                        self.power = param::BEAT_POWER * self.scale;
                        self.vel_z = -self.power;
                        self.move_length_start(s);
                        self.sub = 1;
                    }
                    1 => {
                        s.cmc.gravity_off = false;
                        s.cmc.set_falling();
                        s.cmc.set_velocity_z(-self.power);
                        // The slide out from under the attacker (decompiled C for the arithmetic,
                        // constants at instruction level: spec 9.4 Beat): with the instigator
                        // alive and ground within 2500 cm straight down, t = the time to reach it
                        // from the quadratic with the component's gravity, and Velocity.xy =
                        // knock x (0.7071 x own radius + 0.7071 x instigator radius) / t.
                        let alive = cx.host.targets.info(self.causer).is_some_and(|t| t.flags.has(crate::host::TargetFlags::ALIVE));
                        let (hit, distance, _) = super::common::ground_probe(s, cx.world, P.hit_beat_trace_range);
                        if alive && hit && distance > 0.0 {
                            let g = P.gravity_base * s.cmc.gravity_scale;
                            let t = if g > 0.0 { (-self.power + (self.power * self.power + 2.0 * g * distance).sqrt()) / g } else { distance / self.power.max(1.0) };
                            if t > 0.0 {
                                let own = s.params().capsule_radius;
                                let slide = self.knock.truncate().normalize_or_zero() * (P.hit_beat_radius_factor * own + P.hit_beat_radius_factor * self.causer_radius) / t;
                                s.cmc.set_velocity_xy(slide);
                            }
                        }
                        self.sub = 15;
                        return !self.play(s, AnimId::from_name("M_0230"), 1, -1.0);
                    }
                    15 => {
                        if self.landed || on_ground(s) {
                            s.cmc.stop_movement();
                            self.sub = 5;
                            self.recoverable = false;
                            self.play(s, AnimId::from_name("M_0227"), 0, 0.13);
                        }
                    }
                    5 => {
                        if s.anim.ended {
                            self.power = param::BEAT_REBOUND;
                            self.sub = 12;
                            self.play(s, AnimId::from_name("M_0228"), 1, -1.0);
                        }
                    }
                    12 => {
                        s.cmc.set_falling();
                        s.cmc.set_velocity_z(self.power);
                        s.cmc.gravity_scale = param::BEAT_REBOUND_GRAVITY_SCALE;
                        self.sub = 13;
                    }
                    _ => {
                        if s.cmc.velocity.z <= 0.0 {
                            self.sub = 6;
                            return true;
                        }
                    }
                }
                false
            }
        }
    }
}

/// The victim side of `TakeDamage` (0x1405BB850; E 8.4) with the reaction gate (E 9.1) and the
/// Damage state's `Setup` (slot 114 0x141365930; E 9.3). Called at the start of a step for
/// every hit the host delivered.
pub(crate) fn take_hit(state: &mut StateData, cx: &mut Ctx, hit: &HitOnPlayer) {
    if cx.pawn.status.hp < 1 {
        return;
    }
    // (DON-KIT) Donald's Aero aura: the added invincible time after a dodge drops the hit as reaction type 11 does
    if cx.pawn.aura_ghost > 0.0 && hit.kind != ETresDamageKind::Recover {
        cx.pawn.actions.hits_dodged += 1;
        cx.pawn.event(EventKind::PlayerDodged { hit: *hit });
        return;
    }
    // (magic implementer) `ShouldTakeDamage` 0x1405BA540 (magic spec I 3.2): while the pawn is
    // invincible (pawn+0x630 bit 2 = `Status::damage_invalid`: a finisher or grand cast before
    // its shot, the Water barrier, Cure with Leaf Bracer, the shotlock and airstep states) a hit
    // that is not RECOVER is registered for the attacker and does nothing: no HP loss, no
    // reaction, no status.
    if cx.pawn.status.damage_invalid && hit.kind != ETresDamageKind::Recover {
        cx.pawn.magic.hits_refused += 1;
        let hp = cx.pawn.status.hp;
        cx.pawn.event(EventKind::PlayerHit { hit: *hit, hp_lost: 0, hp_after: hp });
        return;
    }
    let mut amount = hit.amount;
    if hit.kind == ETresDamageKind::Kill {
        amount = cx.pawn.status.hp as f32;
    }
    if hit.kind == ETresDamageKind::Recover {
        // TODO(stats): `SetHP(HP + Ceil(amount))` is the status implementer's Recover request.
        return;
    }
    // ---- what the hit meets (strand C 8.2.6 with Sora as the victim; combat/guard.rs)
    let (c, part, _) = crate::combat::guard::classify_on_sora(cx.pawn, hit);
    let (mut apply, mut react) = (true, true);
    match c.victim {
        // Dropped (reaction type 11: Dodge Roll, Air Slide ...) or blocked without a reaction
        // (invincible, nullified): the hit does not happen for him.
        ReactionResult::None => {
            cx.pawn.actions.hits_dodged += 1;
            cx.pawn.event(EventKind::PlayerDodged { hit: *hit });
            return;
        }
        ReactionResult::NoReaction => react = false,
        // R 3 -> interface +0x448 (slot 137 0x14143FE00): the weapon recoil. R 4 -> +0x458 (slot
        // 139 0x14143CA90, verdict same): `if !CounterAllowed(0): false; ClearQueue; return
        // GuardReaction(causer) != null` - the guard reaction (slot 140 0x141444C30: a successful
        // guard-hit entry, the 1.0 s counter window); on false +0x448, the recoil. No HP loss
        // (0x1405BBB5F).
        ReactionResult::Reflect | ReactionResult::GuardReflect => {
            cx.pawn.event(EventKind::PlayerGuard { hit: *hit, victim: c.victim, attacker: c.attacker });
            if !matches!(state, StateData::Die(_)) {
                if c.victim == ReactionResult::GuardReflect && crate::combat::guard::reaction_allowed(state.id(), cx.pawn) {
                    cx.pawn.queue.clear();
                    if super::guard::guard_reaction(cx, hit.causer, hit.causer_position) {
                        return;
                    }
                }
                super::reflect::request(cx, hit.causer, hit.causer_position, Vec3::ZERO, false);
            }
            return;
        }
        // R 5 -> +0x470 `OnGuardedHit`.
        ReactionResult::Guard => {
            if c.attacker != ReactionResult::NoReaction {
                cx.pawn.event(EventKind::PlayerGuard { hit: *hit, victim: c.victim, attacker: c.attacker });
            }
            crate::combat::guard::guarded(state, cx, hit, c.attacker, part);
            return;
        }
        // R 6 (ARMOR_BREAK; Sora wears none): no HP loss, the normal reaction.
        ReactionResult::ArmorBreak => apply = false,
        _ => {}
    }
    cx.pawn.actions.last_attacker = Some((hit.causer, hit.causer_position));
    // `IsNoDamage` (pawn vt+0x2080): pawn+0x638 > 0 (1.0 s after the auto-revive): no HP loss,
    // the reaction still happens.
    if cx.pawn.combat.no_damage_timer > 0.0 {
        apply = false;
    }
    if hit.kind == ETresDamageKind::Without {
        (react, apply) = (false, true);
    }
    // `m_bIsNoDamageResponse` (pawn+0x630 bit 3; `IsNoReaction` pawn vt+0x2070): the burn state
    // sets it: HP loss without a reaction (spec 9.6, 9.9.4).
    if cx.pawn.combat.bad.no_reaction {
        react = false;
    }
    let mut killed = false;
    if apply {
        let outcome = cx.pawn.status.apply_damage_to_sora(amount, &HitOnSora { attribute: hit.attribute, src_power: hit.src_power, ..HitOnSora::default() });
        killed = outcome.dead || cx.pawn.status.hp < 1;
        cx.pawn.event(EventKind::PlayerHit { hit: *hit, hp_lost: outcome.lost, hp_after: outcome.hp_after });
    }
    // `Survive()` (pawn slot 575 0x14142F640; spec 9.8): not dying already and an auto-revive
    // item (0xA01) in the pouch: the hit is processed as a normal reaction with the bad status
    // removed, then the item's effect: HP = Ceil(MaxHP x 0.5), no damage for 1.0 s. (The item is
    // not one of the status side's battle items: a pouch stack named ITEM_KUPOCOIN is taken.)
    let mut survived = false;
    if killed && !matches!(state, StateData::Die(_)) {
        if let Some(stack) = cx.pawn.status.items.iter_mut().find(|stack| stack.count > 0 && stack.item.eq_ignore_ascii_case("ITEM_KUPOCOIN")) {
            stack.count -= 1;
            (killed, survived) = (false, true);
        }
    }
    let dir = {
        let flat = Vec3::new(hit.direction.x, hit.direction.y, 0.0);
        if flat.length_squared() < 1.0e-8 {
            // A hit without a direction pushes along the attacker's facing.
            let a = hit.causer_yaw.to_radians();
            Vec3::new(a.cos(), a.sin(), 0.0)
        } else {
            flat.normalize()
        }
    };
    if killed {
        // The die path (0x1405A35D0, request interface slot 129 0x14143A190; E 9.8).
        if matches!(state, StateData::Die(_)) {
            return;
        }
        let to_causer = hit.causer_position - cx.pawn.cmc.location;
        let behind = wrap_degrees(yaw_of(to_causer) - cx.pawn.yaw).abs() > 90.0;
        let knock = {
            let away = Vec3::new(-to_causer.x, -to_causer.y, 0.0);
            if away.length_squared() < 1.0e-8 {
                let a = cx.pawn.yaw.to_radians();
                -Vec3::new(a.cos(), a.sin(), 0.0)
            } else {
                away.normalize()
            }
        };
        // (0x1405A35D0: the bad statuses are cleared.)
        if cx.pawn.combat.bad.frozen {
            crate::combat::badstat::set_frozen(cx, false);
        }
        crate::combat::badstat::clear(cx, 0);
        cx.pawn.queue.clear();
        cx.pawn.queue.push_state(super::die::Die::new(knock, behind));
        cx.pawn.event(EventKind::PlayerDied { causer: hit.causer });
        return;
    }
    if survived {
        let hp = (cx.pawn.status.hp_max as f32 * P.revive_hp_fraction).ceil() as i32;
        cx.pawn.status.hp = hp.max(1);
        cx.pawn.combat.no_damage_timer = P.revive_no_damage_time;
        cx.pawn.event(EventKind::AutoRevived { hp: cx.pawn.status.hp });
    }
    // ---- the hit's bad status (spec 9.9.1). An auto-revive removes it from the hit
    // (0x14064CFD0). With a reaction: 0x1405A2350 - the hit cures a stun, then the row's status is
    // applied unless he is immune (FREEZE is left to the gate's state). Without one: the status
    // alone (0x1405B7C50), then request-interface slot 90 (0x14143EEB0) as its reaction: the
    // freeze / burn states only.
    use crate::combat::badstat::{self, kind as bad};
    let bad_kind = if survived { 0 } else { badstat::kind_of(hit.bad_status) };
    let gate_open = !matches!(state, StateData::Die(_));
    let freezes = |cx: &Ctx| bad_kind == bad::FREEZE && hit.bad_time > 0.0 && !badstat::immune(cx, bad::FREEZE);
    let burns = |cx: &Ctx| bad_kind == bad::BURN && hit.bad_time > 0.0 && !badstat::immune(cx, bad::BURN);
    if !react {
        if bad_kind != 0 && bad_kind != bad::FREEZE && !badstat::immune(cx, bad_kind) {
            badstat::apply(cx, bad_kind, hit.bad_time, hit.bad_param);
        }
        if gate_open {
            if freezes(cx) {
                if !super::badstat_freeze::refresh(state, hit.bad_time) {
                    cx.pawn.queue.set_state(super::badstat_freeze::BadstatFreeze::new(hit.bad_time));
                }
            } else if burns(cx) && !matches!(state, StateData::BadstatBurn(_)) {
                cx.pawn.queue.set_state(super::badstat_burn::BadstatBurn::new());
            }
        }
        return;
    }
    let has_status = badstat::on_damaging_hit(cx, bad_kind, hit.bad_time, hit.bad_param);
    // ---- the reaction gate (iface slot 88 0x141439A50; spec 9.1)
    // ctrl+0x8F4 > 0: the reaction-block timer (0.1 s after a command was executed).
    if cx.pawn.combat.reaction_block > 0.0 {
        return;
    }
    // FREEZE (7) with a time and no immunity (pawn vt+0x20C0) -> state 123, the hit's time in
    // state+0x88 (a running freeze state only gets the new time); BURN (0x11) with a time, no
    // immunity and damage kind NONE (vt+0x20C8) -> state 124.
    if freezes(cx) {
        if !super::badstat_freeze::refresh(state, hit.bad_time) {
            cx.pawn.queue.set_state(super::badstat_freeze::BadstatFreeze::new(hit.bad_time));
        }
        return;
    }
    if burns(cx) && hit.kind == ETresDamageKind::None {
        if !matches!(state, StateData::BadstatBurn(_)) {
            cx.pawn.queue.set_state(super::badstat_burn::BadstatBurn::new());
        }
        return;
    }
    let _ = has_status;
    let mut carried = 0;
    match state {
        StateData::Die(_) => return,
        StateData::Damage(cur) => carried = cur.hits,
        _ => {}
    }
    // (The no-flinch cases of the light kinds - flowmotion states, attack kinds 0x36 - 0x39, the
    // reaction-block timer, the command lock - have no state in this port yet.)
    let s = &mut *cx.pawn;
    s.queue.clear();
    // ---- Setup
    let mut kind = hit.kind;
    let (mut scale, mut info_brake) = (hit.power_scale, hit.brake);
    if killed && kind == ETresDamageKind::Small {
        (kind, scale, info_brake) = (ETresDamageKind::Blow, 1.0, 0.0);
    }
    let p = *s.params();
    s.cmc.stop_movement();
    // FUN_14135FF70: speeds and gravity back to the class defaults, gravity on.
    s.cmc.max_walk_speed = p.speed_run;
    s.cmc.gravity_scale = p.gravity_scale;
    s.cmc.gravity_off = false;
    let mut damage = Damage {
        sub: 0,
        kind,
        handler: handler_of(kind),
        stick: false,
        recoverable: false,
        scale,
        info_brake,
        info_angle: hit.parabola_angle,
        move_length: hit.move_length,
        effect_time: hit.effect_time,
        dir,
        since: 0.0,
        timer: 0.0,
        killed,
        hits: carried + 1,
        variant: carried & 1 == 1,
        clip: None,
        start: s.cmc.location,
        knock: dir,
        facing: s.yaw,
        power: 0.0,
        brake: 0.0,
        vel_z: 0.0,
        index: 0,
        was_flying: s.cmc.is_flying(),
        brake_on: false,
        landed: false,
        causer_position: hit.causer_position,
        causer_radius: hit.causer_radius,
        wall_armed: false,
        wall: false,
        causer: hit.causer,
        // Setup: the pawn's status byte in {7, 0x11, 12} -> +0x90 = 1 and the HUD lock.
        bad: matches!(s.combat.bad.kind, bad::FREEZE | bad::BURN | bad::STUN),
    };
    if damage.bad {
        s.combat.bad.hud_locked = true;
    }
    s.status.in_damage_state = true;
    // The handler's sub-step 0 runs at once.
    damage.handle(cx, 0.0);
    cx.pawn.queue.push_state(damage);
}

/// `MoveInput` (0x141440B9E): the stick is active this frame (+0x8D).
pub(crate) fn move_input(state: &mut StateData) {
    if let StateData::Damage(d) = state {
        d.stick = true;
    }
}

impl PlayerState for Damage {
    const ID: StateId = StateId::Damage;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// `Begin` (slot 96 0x14134F750).
    fn begin(&mut self, cx: &mut Ctx) {
        self.was_flying = cx.pawn.cmc.is_flying();
        cx.pawn.status.in_damage_state = true;
    }

    /// `Tick` (slot 97 0x1413553F0).
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        // "Landed" (+0x1C0 bit 2, event interface slot 75): the movement is walking again after
        // the handler took him off the ground.
        if self.since > 0.0 && on_ground(cx.pawn) {
            self.landed = true;
        }
        if self.handle(cx, dt) {
            // Tick's finish (0x1413553F0): with the flag and a burn on him the burn state follows.
            if self.bad && cx.pawn.combat.bad.is(crate::combat::badstat::kind::BURN) {
                cx.pawn.queue.push_state(super::badstat_burn::BadstatBurn::new());
            }
            cx.pawn.queue.do_finish();
        } else {
            if self.timer > 0.0 {
                self.timer = (self.timer - dt).max(0.0);
            }
            self.since += dt;
            self.stick = false;
        }
    }

    /// `End` (slot 98 0x141350C20).
    fn end(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        let p = *s.params();
        s.cmc.max_walk_speed = p.speed_run;
        s.cmc.gravity_scale = p.gravity_scale;
        s.cmc.gravity_off = false;
        s.cmc.direct_xy = None;
        s.cmc.direct_z = None;
        // (Only while no other Damage instance is queued behind this one: a re-entry keeps it.)
        s.status.in_damage_state = false;
        if self.bad {
            s.combat.bad.hud_locked = false;
        }
        let _ = (self.causer_position, self.causer_radius, self.effect_time);
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}

/// For hosts and tests: the running damage reaction (`Sora::damage_reaction`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DamageView {
    /// The wall-hit clip plays (sub 10).
    pub wall: bool,
    pub kind: ETresDamageKind,
    pub handler: Handler,
    pub sub: u8,
    /// Aerial Recovery may be requested now (+0x8E).
    pub recoverable: bool,
    /// Consecutive hits (+0x140).
    pub hits: i32,
    /// 0 front, 1 behind, 2 right, 3 left.
    pub index: u8,
    pub since: f32,
}

pub(crate) fn view(state: &StateData) -> Option<DamageView> {
    match state {
        StateData::Damage(d) => Some(DamageView { wall: d.wall, kind: d.kind, handler: d.handler, sub: d.sub, recoverable: d.recoverable, hits: d.hits, index: d.index, since: d.since }),
        _ => None,
    }
}
