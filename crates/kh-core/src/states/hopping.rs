//! `TresPlayerState_Hopping` (id 9) and its request `TryHopping`: docs/sora-actions-spec.md II.5
//! "Hopping" H0 - H4 (call sites and constants at instruction level, the flow decompiled C
//! there). OWNER: the actions implementer. Plain hop points only: the ordered chains and the
//! `m_bIsDangle` / `m_bIsHassou` / `m_bIsClosing` variants are not in the seam.
//!
//! A hop point of the host's registry (`World::hop_points`) within reach (100 + its radius
//! standing or 120 cm ahead of a run, 220 + its radius in the air) pulls him onto it: `HopL` /
//! `HopR` (0.4333 s), a straight move of his origin to the point at walk speed or with the clip,
//! whichever is nearer, attached (movement mode Custom here). On the point he balances
//! (`BalanceL/R`, `Balance`, `IdleN` by the point's idle type). The stick above 0.85 looks for
//! the next point 150 - 350 cm ahead and hops on, or, without one, hops off forward and falls.
//! The jump button jumps off.

use glam::{Vec2, Vec3};

use super::common::turn;
use super::{jump, PlayerState, StateData, StateId};
use crate::actions::probes::{self, dir, yaw_of};
use crate::actions::A;
use crate::anim::{AnimId, PlayArgs};
use crate::host::EventKind;
use crate::pawn::{Ctx, Pawn};
use crate::sora::{wrap_degrees, MovementState};
use crate::world::HopPoint;

/// The fields of II.5 H3.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Hopping {
    /// 0 start, 1 the hop, 2 on the point, 3 hopping off, 4 an end point's clip.
    pub(super) sub: u8,
    /// +0x90 (the registry id) and the point's facts at the push.
    point: u32,
    target: Vec3,
    idle_motion: u8,
    end_point: bool,
    end_roll: bool,
    /// State +0x84 bit 1 `m_bReverseLeg`: which foot hops next.
    reverse_leg: bool,
    /// +0x98 start location relative to the point; +0xA4 entered from Idle / Jump; +0xA5 stick
    /// this frame; +0xA8 yaw; +0xAC stick magnitude; +0xB0 moving; +0xB4 hop counter; +0xB8 walk
    /// speed in use; +0xC1 chained hop.
    start: Vec3,
    from_rest: bool,
    stick: bool,
    pub(super) yaw: f32,
    magnitude: f32,
    moving: bool,
    counter: u32,
    speed: f32,
    chained: bool,
}

impl Hopping {
    /// Reset (0x14138F650): all cleared; +0xB8 = 600.0.
    pub const RESET: Hopping = Hopping { sub: 0, point: 0, target: Vec3::ZERO, idle_motion: 1, end_point: false, end_roll: false, reverse_leg: false, start: Vec3::ZERO, from_rest: false, stick: false, yaw: 0.0, magnitude: 0.0, moving: false, counter: 0, speed: 600.0, chained: false };

    fn of(point: &HopPoint) -> Hopping {
        Hopping { point: point.id, target: point.position, idle_motion: point.idle_motion, end_point: point.end_point, end_roll: point.end_roll, ..Hopping::RESET }
    }
}

/// The gate (slot 183 0x14142A140) and `TryHopping(yaw, bReverseLeg, 0)` (slot 184 0x14143D7F0)
/// with its push (slot 186): nothing queued; the probe by the state: IDLE radius 100 at the
/// origin; RUN, SLOPESLIDE, SWAYING radius 100 at the point 120 cm ahead; JUMP radius 220; any
/// other state radius 0. The queue is kept: the caller chains or finishes.
pub(crate) fn try_hopping(state: &StateData, cx: &mut Ctx, yaw: f32, reverse_leg: bool) -> bool {
    let (w, s) = (cx.world, &mut *cx.pawn);
    if w.hop_points().is_empty() || s.queue.queued_id().is_some() {
        return false;
    }
    let mut pos = s.cmc.location;
    let radius = match state {
        StateData::Idle(_) => A.hopping_probe_radius_idle,
        StateData::Run(_) | StateData::SlopeSlide(_) | StateData::Swaying(_) => {
            pos += dir(s.yaw) * 120.0;
            A.hopping_probe_radius_run
        }
        StateData::Jump(_) => A.hopping_probe_radius_jump,
        _ => 0.0,
    };
    let Some((point, toward)) = probes::hop_probe(s, w, pos, yaw, radius) else { return false };
    let from_rest = matches!(state, StateData::Idle(_) | StateData::Jump(_));
    s.queue.push_state(Hopping { reverse_leg, from_rest, yaw: if from_rest { toward } else { yaw }, ..Hopping::of(&point) });
    true
}

/// `MoveInput` (slot 35): Hopping+0xA8 = stick yaw, +0xAC = magnitude, +0xA5 = 1.
pub(crate) fn move_input(state: &mut StateData, yaw: f32, magnitude: f32) {
    if let StateData::Hopping(h) = state {
        (h.yaw, h.magnitude, h.stick) = (yaw, magnitude, true);
    }
}

/// The jump gate's HOPPING branch (sub-steps 2 and 3); `JumpRequest` uses yaw = Hopping+0xA8.
pub(super) fn jump_yaw(h: &Hopping) -> Option<f32> {
    matches!(h.sub, 2 | 3).then_some(h.yaw)
}

impl PlayerState for Hopping {
    const ID: StateId = StateId::Hopping;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// Begin 0x141385D80.
    fn begin(&mut self, cx: &mut Ctx) {
        crate::actions::sheathe(cx);
        let s = &mut *cx.pawn;
        let p = *s.params();
        s.actions.traversal.last_hop = Some(self.point);
        // Not already sprinting and the current speed <= the base walk speed: the base speed;
        // else the sprint speed 980. (+0xB8 after Reset is 600: the base speed the hop uses.)
        self.speed = if s.cmc.velocity.truncate().length() <= p.speed_run { A.hopping_reset_speed } else { p.speed_sprint };
        s.cmc.max_walk_speed = self.speed;
        s.cmc.max_acceleration = self.speed * A.hopping_accel_factor;
        (s.actions.second_jump, s.actions.flow_jump, s.actions.superslide_ok) = (true, true, true);
        s.actions.traversal.begun[6] += 1;
        s.event(EventKind::Traversal { kind: "hop", id: self.point, on: true });
    }

    /// Tick 0x141393270.
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        let active = cx.world.hop_points().iter().any(|c| c.id == self.point && c.active);
        if !active {
            cx.pawn.queue.do_chain();
            return;
        }
        match self.sub {
            0 => {
                let s = &mut *cx.pawn;
                self.start = s.cmc.location - self.target;
                s.cmc.stop_movement();
                s.cmc.set_custom();
                s.cmc.gravity_off = true;
                if self.from_rest {
                    self.reverse_leg = true;
                    s.anim.play_with(AnimId::HopL, PlayArgs::mode(0, A.hopping_chain_blend));
                } else {
                    let was = self.reverse_leg;
                    self.reverse_leg = !was;
                    s.anim.play_with(if was { AnimId::HopR } else { AnimId::HopL }, PlayArgs::mode(0, if self.chained { A.hopping_chain_blend } else { -1.0 }));
                }
                (self.sub, self.moving) = (1, true);
            }
            1 => {
                let s = &mut *cx.pawn;
                if self.moving {
                    let length = s.anim.name.data().length;
                    let t = if length > 0.0 { (s.anim.time / length).clamp(0.0, 1.0) } else { 1.0 };
                    let current = s.cmc.location - self.target;
                    // A follows the clip; B is a constant-speed step toward the point.
                    let a = self.start * (1.0 - t);
                    let step = s.cmc.max_walk_speed * dt;
                    let b = current - self.start.normalize_or_zero() * step;
                    let mut p = if b.length_squared() <= a.length_squared() { b } else { a };
                    if p.length_squared() < step * step || current.length_squared() < p.length_squared() {
                        p = Vec3::ZERO;
                        self.moving = false;
                    }
                    s.cmc.location = self.target + p;
                    if !self.from_rest {
                        let flat = -current.truncate();
                        if flat.length_squared() > 1.0 {
                            self.yaw = yaw_of(flat.extend(0.0));
                        }
                    }
                    s.yaw = wrap_degrees(self.yaw);
                }
                if s.anim.ended || !self.moving {
                    s.cmc.location = self.target;
                    s.cmc.stop_movement();
                    if self.end_point {
                        s.actions.traversal.last_hop = None;
                        self.sub = 4;
                        // SetMovementMode(1, 1): the movement finds the floor on its next tick.
                        s.cmc.set_falling();
                        s.cmc.gravity_off = false;
                        s.cmc.max_walk_speed = s.params().speed_run;
                        if self.end_roll {
                            s.anim.play_with(AnimId::RollLand, PlayArgs::mode(2, -1.0));
                        } else {
                            s.queue.do_chain();
                        }
                    } else {
                        let clip = match self.idle_motion {
                            2 => AnimId::Balance,
                            3 => AnimId::IdleN,
                            _ if self.reverse_leg => AnimId::BalanceL,
                            _ => AnimId::BalanceR,
                        };
                        s.anim.play_with(clip, PlayArgs::mode(3, -1.0));
                        self.counter += 1;
                        (self.sub, self.moving) = (2, false);
                    }
                }
            }
            2 => {
                if self.magnitude > A.hopping_next_stick_min {
                    // The next point: probes 150, 200 ... 350 cm ahead along the stick, radius 100;
                    // the highest one found.
                    let (w, s) = (cx.world, &mut *cx.pawn);
                    let mut best: Option<HopPoint> = None;
                    let mut d = A.hopping_next_search_start;
                    while d < A.hopping_next_search_limit {
                        if let Some((found, _)) = probes::hop_probe(s, w, s.cmc.location + dir(self.yaw) * d, self.yaw, A.hopping_next_search_radius) {
                            if best.is_none_or(|b| found.position.z > b.position.z) {
                                best = Some(found);
                            }
                        }
                        d += A.hopping_next_search_step;
                    }
                    if let Some(next) = best {
                        if s.queue.nothing_queued() {
                            s.queue.push_state(Hopping { reverse_leg: self.reverse_leg, yaw: self.yaw, counter: self.counter, chained: true, ..Hopping::of(&next) });
                            s.queue.do_chain();
                            return;
                        }
                    } else {
                        // Hop off: Falling; the other foot's clip; turned to the stick; sub 3.
                        s.cmc.stop_movement();
                        s.cmc.set_falling();
                        s.cmc.gravity_off = false;
                        let was = self.reverse_leg;
                        self.reverse_leg = !was;
                        s.anim.play_with(if was { AnimId::HopR } else { AnimId::HopL }, PlayArgs::mode(0, 0.0));
                        s.yaw = wrap_degrees(self.yaw);
                        self.sub = 3;
                    }
                } else if self.magnitude > A.hopping_turn_stick_min {
                    turn(cx.pawn, self.yaw, dt * A.hopping_turn_rate);
                }
            }
            3 => {
                let s = &mut *cx.pawn;
                let f = dir(s.yaw);
                s.cmc.set_velocity_xy(Vec2::new(f.x, f.y) * s.cmc.max_walk_speed);
                if s.anim.ended {
                    jump::fall(s, self.yaw);
                }
            }
            _ => {
                let s = &mut *cx.pawn;
                if s.anim.ended || (self.stick && s.queue.flags.enable_change) {
                    s.queue.do_chain();
                }
            }
        }
        (self.stick, self.magnitude) = (false, 0.0);
    }

    /// End 0x14138D300.
    fn end(&mut self, cx: &mut Ctx) {
        let next = cx.pawn.queue.queued_id();
        let s = &mut *cx.pawn;
        if s.cmc.is_custom() {
            s.cmc.stop_movement();
            s.cmc.set_falling();
        }
        s.cmc.gravity_off = false;
        s.cmc.direct_xy = None;
        s.cmc.max_walk_speed = s.params().speed_run;
        // m_LastHitHopComponent is cleared unless the next state is HOPPING, DANGLE, or (none /
        // JUMP while not in battle): the point just left cannot be re-taken in the same airtime.
        let keep = matches!(next, Some(StateId::Hopping) | Some(StateId::Dangle)) || (matches!(next, None | Some(StateId::Jump)) && !s.actions.battle);
        if !keep {
            s.actions.traversal.last_hop = None;
        }
        s.event(EventKind::Traversal { kind: "hop", id: self.point, on: false });
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}
