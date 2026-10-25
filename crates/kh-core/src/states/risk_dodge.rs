//! `TresPlayerState_RiskDodge` (id 17) and its request `TryRiskDodge`: docs/sora-actions-spec.md
//! II.1 "Risk Dodge" (Begin / End / Setup / the helpers and types 1 and 3 at instruction level;
//! types 2 and 4 at flow level). OWNER: the actions implementer.
//!
//! The offer: an enemy attack clip carries `TresAnimNotify_DamageCaution` with a
//! `ETresRiskDodgeType`; the controller keeps the offers (+0xC50). THE HOST registers them
//! (`Sora::offer_risk_dodge(attacker, type, seconds)`: its enemy's warning window). When a guarded
//! hit of that attacker breaks the guard into the weapon recoil (`m_ReflectLevel` >= 3,
//! `states/reflect.rs`), the Reflect state shows the prompt for 0.6 s; the JUMP button then
//! starts this state (`TryRiskDodge`, tried before the launch attack and the jump).
//!
//! Types (`ETresRiskDodgeType`): 1 UNDERSLIDE (`UnderSlide`: 800 cm of root motion scaled so that
//! he ends behind the attacker, turned to face it), 2 OVERSLIDE (`OverSlide`: over it), 3
//! SLIDETURN (`SlideTurnR/L`: an orbit round it), 4 WARP (`ShootFlowStart`, 0.2 s, then he stands
//! behind it). Reaction type 11 (the hit does not exist) from Begin to End for every type. Once
//! he has passed the attacker the world slows and the counter command (reaction command 7 - 10,
//! forced attack kind 0x28 - 0x2B: `combat.guard.offer`, the combat side's `TryAttack`) is
//! offered until the clip's `TresEnableChange`.
//!
//! Simplifications, each marked SIMPLIFIED below: the attacker's bounds are the host's target
//! capsule; `RequestDirectMove` of types 2 / 3 is a direct velocity; the warp destination is the
//! point behind the attacker without the two traces of slot 113; the camera requests are not made.

use glam::{Vec2, Vec3};

use super::common::{on_ground, turn};
use super::{PlayerState, StateData, StateId};
use crate::actions::{self, reaction, A};
use crate::anim::{AnimId, NotifyClass, PlayArgs};
use crate::combat::guard::CounterOffer;
use crate::enums::ETresAbilityKind as Ability;
use crate::host::TargetId;
use crate::pawn::{Ctx, Pawn};
use crate::sora::{wrap_degrees, MovementState};

/// `ETresRiskDodgeType`.
pub mod kind {
    pub const UNDERSLIDE: u8 = 1;
    pub const OVERSLIDE: u8 = 2;
    pub const SLIDETURN: u8 = 3;
    pub const WARP: u8 = 4;
}

/// The fields of II.1's table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RiskDodge {
    sub: u8,
    /// +0x88.
    kind: u8,
    /// +0xD0 and where it stood at the request.
    attacker: TargetId,
    attacker_position: Vec3,
    attacker_yaw: f32,
    attacker_radius: f32,
    attacker_half_height: f32,
    /// +0x90 start, +0x9C working target point, +0xA8 destination, +0xB4 distance, +0xB8 root scale.
    start: Vec3,
    point: Vec3,
    destination: Vec3,
    distance: f32,
    root_scale: f32,
    /// +0xBC / +0xC0: the orbit angle and its goal (SLIDETURN).
    angle: f32,
    angle_goal: f32,
    /// +0xC4.
    warp_delay: f32,
    /// +0xC8 bit 1 slow motion on, bit 2 counter used; +0xD8 bit 0 counter offered, bit 1 critical.
    slow: bool,
    used: bool,
    offered: bool,
    critical: bool,
    /// +0xDC / +0xE0: the reaction command id 7 - 10 and the attack kind 0x28 - 0x2B.
    command: i32,
    attack_kind: u8,
    /// State +0x84 bit 2: `TresTraction` (set by the type ticks, cleared when the notify state ends).
    traction: bool,
    /// The body collision was switched off (pawn vt+0x1010(0)).
    ghost_body: bool,
}

impl RiskDodge {
    /// Reset (0x1413AD2A0).
    pub const RESET: RiskDodge = RiskDodge {
        sub: 0,
        kind: 0,
        attacker: TargetId::NONE,
        attacker_position: Vec3::ZERO,
        attacker_yaw: 0.0,
        attacker_radius: 0.0,
        attacker_half_height: 0.0,
        start: Vec3::ZERO,
        point: Vec3::ZERO,
        destination: Vec3::ZERO,
        distance: 0.0,
        root_scale: 1.0,
        angle: 0.0,
        angle_goal: 0.0,
        warp_delay: 0.0,
        slow: false,
        used: false,
        offered: false,
        critical: false,
        command: 0,
        attack_kind: 0,
        traction: false,
        ghost_body: false,
    };

    /// Half the XY diagonal of the attacker's body bounds. SIMPLIFIED: the host's target capsule
    /// (bounds box extent (r, r)), not the root component's bounds.
    fn half_diagonal(&self) -> f32 {
        self.attacker_radius * std::f32::consts::SQRT_2
    }

    /// The attacker now (the host's target while it exists).
    fn refresh(&mut self, cx: &Ctx) {
        if let Some(t) = cx.target(self.attacker) {
            (self.attacker_position, self.attacker_yaw, self.attacker_radius, self.attacker_half_height) = (t.position, t.yaw, t.radius, t.half_height);
        }
    }

    /// Slot 115 `OfferCounter(b)` (0x1413BD170).
    fn offer_counter(&mut self, cx: &mut Ctx, on: bool) {
        if self.command == 0 {
            return;
        }
        if on {
            cx.pawn.combat.guard.offer = Some(CounterOffer { source: StateId::Riskdodge, kind: self.attack_kind, target: self.attacker, just_guard: self.critical, push: true });
            cx.pawn.combat.guard.counter_used = false;
            // 0x141414350(controller, +0xDC, 2.0, 0): the reaction command prompt.
            crate::combat::situation::set_reaction(cx, self.command, A.riskdodge_counter_command_time);
            self.offered = true;
        } else if self.offered {
            if cx.pawn.combat.guard.offer.is_some_and(|o| o.source == StateId::Riskdodge) {
                cx.pawn.combat.guard.offer = None;
            }
            crate::combat::situation::clear_reaction(cx, self.command);
            cx.pawn.actions.critical = false;
            self.offered = false;
        }
    }

    /// Slot 116 `SlowMotion(b)` (0x1413C31E0): the world slow (0x14140E500) and the pawn's own
    /// dilation call 0x1405DA4E0(pawn, 1, 1, 0.25, 0.05) (arguments not understood: not applied).
    fn slow_motion(&mut self, cx: &mut Ctx, on: bool) {
        if self.command == 0 {
            return;
        }
        if on {
            crate::combat::form::world_slow(cx, true);
            self.slow = true;
        } else if self.slow {
            crate::combat::form::world_slow(cx, false);
            self.slow = false;
        }
    }

    /// Slot 117 `CounterUsed` (0x1413BE760), noticed here: the combat side's `TryAttack` took the
    /// offer (`combat.guard.counter_used`).
    fn poll_counter_used(&mut self, cx: &mut Ctx) {
        if self.offered && cx.pawn.combat.guard.counter_used {
            self.used = true;
            crate::combat::situation::clear_reaction(cx, self.command);
            self.offered = false;
            self.slow_motion(cx, false);
        }
    }

    /// pawn vt+0x1010(b, None): the body collision back on.
    fn body_on(&mut self, s: &mut Pawn) {
        s.actions.body_collision_off = false;
        self.ghost_body = false;
    }

    fn yaw_to(from: Vec3, to: Vec3, fallback: f32) -> f32 {
        let d = (to - from).truncate();
        if d.length_squared() > 1.0e-8 { d.y.atan2(d.x).to_degrees() } else { fallback }
    }

    fn dir2(from: Vec3, to: Vec3) -> Vec2 {
        (to - from).truncate().normalize_or_zero()
    }

    /// Type 1 UNDERSLIDE, slot 118 (0x1413BB660).
    fn under_slide(&mut self, cx: &mut Ctx) {
        match self.sub {
            0 => {
                let s = &mut *cx.pawn;
                s.cmc.stop_movement();
                // comiss +0xB4,[300]; jae -> else 300; comiss +0xB4,[1000]; jbe -> else 1000.
                self.distance = self.half_diagonal().clamp(A.riskdodge_underslide_distance_min, A.riskdodge_underslide_distance_max);
                let a = self.attacker_yaw.to_radians();
                // The point +0xB4 behind the attacker's back.
                self.destination = self.attacker_position - Vec3::new(a.cos(), a.sin(), 0.0) * self.distance;
                // +0xB8 = |(+0xA8 - +0x90).xy| x 0.00125 (1 / the clip's 800 cm).
                self.root_scale = (self.destination - self.start).truncate().length() * A.riskdodge_underslide_root_scale;
                s.root_scale = (self.root_scale, 1.0);
                s.anim.play_with(AnimId::UnderSlide, PlayArgs::mode(2, -1.0));
                s.yaw = wrap_degrees(Self::yaw_to(self.start, self.destination, s.yaw));
                self.sub = 1;
            }
            1 => {
                if cx.pawn.queue.flags.enable_change {
                    self.body_on(cx.pawn);
                    self.sub = 3;
                    return;
                }
                self.point = self.attacker_position;
                let own = cx.pawn.cmc.location;
                let (a, b) = (Self::dir2(own, self.attacker_position), Self::dir2(own, self.destination));
                // comiss dot,0; jae skip: he has passed the attacker.
                if a.dot(b) < 0.0 {
                    if self.root_scale > 1.0 {
                        cx.pawn.root_scale = (1.0, 1.0);
                    }
                    self.slow_motion(cx, true);
                    self.offer_counter(cx, true);
                    self.sub = 2;
                }
            }
            2 => {
                if cx.pawn.queue.flags.enable_change {
                    if self.used {
                        cx.pawn.queue.do_chain();
                        return;
                    }
                    self.slow_motion(cx, false);
                    self.offer_counter(cx, false);
                    self.body_on(cx.pawn);
                    self.sub = 3;
                }
                if cx.pawn.anim.ended {
                    cx.pawn.queue.do_chain();
                }
            }
            _ => {
                if cx.pawn.anim.ended {
                    cx.pawn.queue.do_chain();
                }
            }
        }
    }

    /// Type 3 SLIDETURN, slot 120 (0x1413BA340).
    fn slide_turn(&mut self, cx: &mut Ctx, dt: f32) {
        let own = cx.pawn.cmc.location;
        let face = Self::yaw_to(own, self.attacker_position, cx.pawn.yaw);
        match self.sub {
            0 => {
                let s = &mut *cx.pawn;
                s.cmc.stop_movement();
                s.cmc.set_flying();
                let d = Self::dir2(own, self.attacker_position);
                let a = s.yaw.to_radians();
                let c = a.cos() * d.y - a.sin() * d.x;
                // comiss c,0; jae other.
                if c < 0.0 {
                    s.anim.play_with(AnimId::SlideTurnR, PlayArgs::mode(2, -1.0));
                    (self.angle, self.angle_goal) = (120.0, -24.0);
                } else {
                    s.anim.play_with(AnimId::SlideTurnL, PlayArgs::mode(2, -1.0));
                    (self.angle, self.angle_goal) = (-120.0, 24.0);
                }
                self.traction = true;
                self.distance = self.half_diagonal().max(A.riskdodge_slideturn_radius_min);
                self.destination = self.attacker_position + Vec3::new(d.x, d.y, 0.0) * self.distance;
                self.sub = 1;
                return;
            }
            1 => {
                if !self.traction {
                    self.slow_motion(cx, true);
                    self.offer_counter(cx, true);
                    self.sub = 2;
                } else {
                    let yaw = (Self::yaw_to(self.attacker_position, self.destination, 0.0) + self.angle).to_radians();
                    self.point = self.attacker_position + Vec3::new(yaw.cos(), yaw.sin(), 0.0) * self.distance;
                    // +0xBC moves toward +0xC0 by dt x 540, clamped.
                    let step = dt * A.riskdodge_slideturn_angle_rate;
                    self.angle = if self.angle < self.angle_goal { (self.angle + step).min(self.angle_goal) } else { (self.angle - step).max(self.angle_goal) };
                    // pawn.vt+0x1668(normalise2D(+0x9C - pawnPos) x dt x 240000, 0): RequestDirectMove.
                    // SIMPLIFIED: the requested velocity, capped at the state's MaxWalkSpeed 3000, as a
                    // direct velocity.
                    let speed = (dt * A.riskdodge_slideturn_input).min(A.riskdodge_max_walk_speed);
                    cx.pawn.cmc.set_velocity_xy(Self::dir2(own, self.point) * speed);
                }
            }
            2 => {
                let s = &mut *cx.pawn;
                if s.queue.flags.enable_change {
                    if self.used {
                        s.queue.do_chain();
                        return;
                    }
                    self.slow_motion(cx, false);
                    self.offer_counter(cx, false);
                    let s = &mut *cx.pawn;
                    // SetMovementMode(1, 1): Walking with the stop (the movement finds the floor).
                    s.cmc.set_falling();
                    s.cmc.stop_movement();
                    s.cmc.direct_xy = None;
                    self.body_on(s);
                    self.sub = 3;
                } else {
                    // velocity x pow(0.9, dt x 60), written back (0x1413BA455).
                    let v = s.cmc.velocity.truncate() * 0.9f32.powf(dt * 60.0);
                    s.cmc.set_velocity_xy(v);
                }
                if cx.pawn.anim.ended {
                    cx.pawn.queue.do_chain();
                }
            }
            _ => {
                if cx.pawn.anim.ended {
                    cx.pawn.queue.do_chain();
                }
                return;
            }
        }
        turn(cx.pawn, face, dt * 12.0);
    }

    /// Type 2 OVERSLIDE, slot 119 (0x1413B8280; the steering arithmetic is decompiled C only).
    fn over_slide(&mut self, cx: &mut Ctx, dt: f32) {
        match self.sub {
            0 => {
                let s = &mut *cx.pawn;
                s.cmc.stop_movement();
                s.cmc.set_flying();
                s.cmc.gravity_off = true;
                s.anim.play_with(AnimId::OverSlide, PlayArgs::mode(2, -1.0));
                self.traction = true;
                self.point = s.cmc.location + Vec3::Z * 150.0;
                self.sub = 1;
            }
            1 => {
                let s = &mut *cx.pawn;
                if self.traction {
                    // +0x9C = the attacker's position with z + (150 - bounds z extent - 20).
                    self.point = self.attacker_position + Vec3::Z * (150.0 - self.attacker_half_height - 20.0);
                    // While clip time x 4.2857 < 1 (the notify's 0.2333 s): a small direct move toward it.
                    if s.anim.time * 4.2857 < 1.0 {
                        let d = (self.point - s.cmc.location).normalize_or_zero();
                        s.cmc.set_velocity_xy(d.truncate() * dt * 600.0);
                    }
                    // (The side test against the attacker's rotation that ends traction early: not ported.)
                } else {
                    s.cmc.direct_xy = None;
                    s.root_scale = (1.0, 1.0);
                    self.slow_motion(cx, true);
                    self.offer_counter(cx, true);
                    self.sub = 2;
                }
            }
            2 => {
                if cx.pawn.queue.flags.enable_change {
                    if self.used {
                        cx.pawn.queue.do_chain();
                        return;
                    }
                    self.slow_motion(cx, false);
                    self.offer_counter(cx, false);
                    let s = &mut *cx.pawn;
                    s.cmc.set_falling();
                    s.cmc.gravity_off = false;
                    self.body_on(s);
                    self.sub = 3;
                }
                if cx.pawn.anim.ended {
                    cx.pawn.queue.do_chain();
                }
            }
            _ => {
                if cx.pawn.anim.ended {
                    cx.pawn.queue.do_chain();
                }
            }
        }
    }

    /// Type 4 WARP, slot 121 (0x1413BC250).
    fn warp(&mut self, cx: &mut Ctx, dt: f32) {
        match self.sub {
            0 => {
                let s = &mut *cx.pawn;
                s.cmc.gravity_off = true;
                s.cmc.stop_movement();
                s.cmc.set_flying();
                s.anim.play_with(AnimId::ShootFlowStart, PlayArgs::mode(2, -1.0));
                self.warp_delay = A.riskdodge_warp_delay;
                self.sub = 1;
            }
            1 => {
                self.warp_delay -= dt;
                if self.warp_delay > 0.0 {
                    return;
                }
                // Slot 113: the destination by two traces of pawn.vt+0x2750. SIMPLIFIED: the point
                // behind the attacker's back at the under-slide distance, at his own height, swept
                // there by the capsule so that a wall stops him.
                let a = self.attacker_yaw.to_radians();
                let behind = self.attacker_position - Vec3::new(a.cos(), a.sin(), 0.0) * self.half_diagonal().clamp(A.riskdodge_underslide_distance_min, A.riskdodge_underslide_distance_max);
                self.point = Vec3::new(behind.x, behind.y, cx.pawn.cmc.location.z);
                let (w, s) = (cx.world, &mut *cx.pawn);
                let p = *s.params();
                let centre = s.cmc.capsule_centre(&p);
                let delta = self.point - s.cmc.location;
                let free = w.sweep_capsule(centre + Vec3::Z * 5.0, centre + Vec3::Z * 5.0 + delta, p.capsule_radius, p.capsule_half_height).map_or(1.0, |h| (h.time - 0.02).max(0.0));
                s.cmc.location += delta * free;
                s.yaw = wrap_degrees(Self::yaw_to(s.cmc.location, self.attacker_position, s.yaw));
                s.queue.flags.enable_change = true;
                self.slow_motion(cx, true);
                self.offer_counter(cx, true);
                self.sub = 2;
            }
            _ => {
                if self.used || cx.pawn.anim.ended {
                    cx.pawn.queue.do_chain();
                }
            }
        }
    }
}

/// `controller.vt+0x1CB0(attackerPawn)` (0x1413ED1E0), called on the Reflect entry of a guarded
/// hit: needs RISKDODGE (8); picks the offer of the guarded attacker (type != 0, type 1 only when
/// Sora is on the ground), stores it (controller+0xC40 / +0xC48) and empties the list.
pub(crate) fn choose_offer(s: &mut Pawn, attacker: TargetId) -> bool {
    if !actions::has(s, Ability::Riskdodge) {
        return false;
    }
    let ground = on_ground(s);
    let found = s.actions.risk_offers.iter().find(|o| o.attacker == attacker && o.kind != 0 && (o.kind != kind::UNDERSLIDE || ground)).copied();
    let Some(offer) = found else { return false };
    s.actions.risk_chosen = Some((offer.attacker, offer.kind));
    s.actions.risk_offers.clear();
    true
}

/// The gate (slot 60 0x14142CD20) and `TryRiskDodge(yaw, mag, target, type)` (slot 61
/// 0x14143FF10): nothing queued, RISKDODGE, the running state is a Reflect whose risk-dodge
/// window is open; a target and a type chosen. `ClearQueue`; `SetState(17)`; `Setup`.
pub(crate) fn try_risk_dodge(state: &mut StateData, cx: &mut Ctx) -> bool {
    let StateData::Reflect(reflect) = state else { return false };
    if !reflect.risk_offered() || !cx.pawn.queue.nothing_queued() || !actions::has(cx.pawn, Ability::Riskdodge) {
        return false;
    }
    let Some((attacker, kind)) = cx.pawn.actions.risk_chosen else { return false };
    if attacker == TargetId::NONE || kind == 0 {
        return false;
    }
    let info = cx.target(attacker);
    let s = &mut *cx.pawn;
    // Setup (slot 114 0x1413C44C0).
    let (command, attack_kind) = match kind {
        kind::OVERSLIDE => (8, 0x29),
        kind::SLIDETURN => (9, 0x2A),
        kind::WARP => (10, 0x2B),
        _ => (7, 0x28),
    };
    let mut next = RiskDodge { kind, attacker, start: s.cmc.location, critical: s.actions.critical, command, attack_kind, ..RiskDodge::RESET };
    if let Some(t) = info {
        (next.attacker_position, next.attacker_yaw, next.attacker_radius, next.attacker_half_height) = (t.position, t.yaw, t.radius, t.half_height);
        // Turn(rot(0, yaw(target - pawn), 0), 1.0).
        s.yaw = wrap_degrees(RiskDodge::yaw_to(s.cmc.location, t.position, s.yaw));
    } else {
        return false;
    }
    s.queue.set_state(next);
    true
}

impl PlayerState for RiskDodge {
    const ID: StateId = StateId::Riskdodge;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// Begin 0x1413A5730.
    fn begin(&mut self, cx: &mut Ctx) {
        actions::sheathe(cx);
        // (integration pass) RiskDodge Begin 0x1413A5730: the attack camera off, then mode 3 on the
        // attacker (`Camera.SetLockon(attacker)`; combat/camera.rs, combat spec 7.12.1).
        crate::combat::camera::off(cx);
        crate::combat::camera::request(cx, true, 3, self.attacker);
        let s = &mut *cx.pawn;
        s.cmc.stop_movement();
        s.cmc.max_walk_speed = A.riskdodge_max_walk_speed;
        s.cmc.max_acceleration = A.riskdodge_max_acceleration;
        actions::set_reaction(s, reaction::GHOST_INVINCIBLE);
        // pawn.vt+0x1010(0, None): the components of pawn+0x470 off (taken as the pawn-to-pawn body).
        s.actions.body_collision_off = true;
        self.ghost_body = true;
        s.actions.risk_dodges += 1;
        // (The camera requests FUN_14140DF20 toward the attacker: the camera side's.)
    }

    /// Tick 0x1413AED20: the per-type delegate.
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        self.refresh(cx);
        self.poll_counter_used(cx);
        // `TresTraction` begin / end set / clear state+0x84 bit 2: the bit the type tick set stays
        // until the notify state has been seen and has ended.
        if self.traction && self.sub >= 1 && cx.pawn.anim.time > 0.0 && !cx.pawn.anim.state_active(NotifyClass::TresAnimNotifyStateTraction) {
            self.traction = false;
        }
        match self.kind {
            kind::OVERSLIDE => self.over_slide(cx, dt),
            kind::SLIDETURN => self.slide_turn(cx, dt),
            kind::WARP => self.warp(cx, dt),
            _ => self.under_slide(cx),
        }
    }

    /// End 0x1413AB810.
    fn end(&mut self, cx: &mut Ctx) {
        self.slow_motion(cx, false);
        self.offer_counter(cx, false);
        // (integration pass) End 0x1413AB810: the attack camera off (the camera goes back to the
        // manual lock, or unlocks).
        crate::combat::camera::off(cx);
        let s = &mut *cx.pawn;
        let p = *s.params();
        s.cmc.max_walk_speed = p.speed_run;
        actions::restore_reaction(s);
        s.actions.body_collision_off = false;
        s.root_scale = (1.0, 1.0);
        s.cmc.direct_xy = None;
        // On ground ? SetMovementMode(1, 1) : SetMovementMode(3, 0); gravity on.
        if !on_ground(s) {
            s.cmc.set_falling();
        }
        s.cmc.gravity_off = false;
        s.actions.risk_chosen = None;
        // Type not in {1, 3} and the weapon is wanted: Draw(1, 1, 1.0).
        if !matches!(self.kind, kind::UNDERSLIDE | kind::SLIDETURN) {
            actions::draw(cx, true);
        }
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}

/// For hosts and tests (`Sora::risk_dodge`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RiskDodgeView {
    pub kind: u8,
    pub attacker: TargetId,
    pub destination: Vec3,
    pub root_scale: f32,
    pub counter_offered: bool,
    pub slow_motion: bool,
}

pub(crate) fn view(state: &StateData) -> Option<RiskDodgeView> {
    match state {
        StateData::RiskDodge(r) => Some(RiskDodgeView { kind: r.kind, attacker: r.attacker, destination: r.destination, root_scale: r.root_scale, counter_offered: r.offered, slow_motion: r.slow }),
        _ => None,
    }
}
