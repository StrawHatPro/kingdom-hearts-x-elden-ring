//! `TresPlayerState_SlopeSlide` (id 10) and its request `TrySlopeSlide`:
//! docs/sora-actions-spec.md II.6 "Slope Slide" (constants at instruction level, the tick's flow
//! decompiled C there). OWNER: the actions implementer. Plain Keyblade: the weapon-form variant
//! is not here.
//!
//! A floor component flagged `SQEX_bEnableSlopeSlide` that descends at 25 degrees or more under
//! him while he faces downhill (`actions::probes::slope_start`): landing on it always slides;
//! running or standing on it slides only while the latch of a previous slide is still set (or,
//! from Run, while he really runs). Walking mode; the state overwrites the velocity every tick:
//! its magnitude ramps from the entry speed to 1600 cm/s in 0.5 s, its direction is his facing,
//! which eases 2 % per tick toward the stick. When the floor flattens the speed drops; with the
//! stick held he breaks into a run under 980 cm/s, without it he glides until 680 and stops with
//! `LightLandR`. The jump button jumps.
//!
//! PORT: the game's increments are per tick (the ramp scales with the frame rate); here they
//! are scaled by `dt x 60`, so the slide is the 60 Hz one at any step (user rule: fixed-step
//! physics, frame-rate independent). SIMPLIFIED: the body's pitch to the slope (slope angle +
//! 25 degrees) is not applied: the actor has no pitch in this port.

use glam::Vec2;

use super::common::on_ground;
use super::{hopping, jump, run, PlayerState, StateData, StateId};
use crate::actions::probes::{self, dir};
use crate::actions::{self, A};
use crate::anim::{AnimId, PlayArgs};
use crate::host::EventKind;
use crate::pawn::{Ctx, Pawn};
use crate::sora::{wrap_degrees, MovementState};

/// The fields of II.6's table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SlopeSlide {
    /// 0 sliding, 1 the end clip.
    pub(super) sub: u8,
    /// +0x88 stick this frame, +0x89 sliding, +0x8C target yaw, +0x94 speed increment per 60 Hz
    /// tick, +0x9C the slope angle of the last probe, +0xA0 time in state.
    stick: bool,
    sliding: bool,
    pub(super) yaw: f32,
    increment: f32,
    angle: f32,
    time: f32,
    /// The state is ending into a run (its own `TryRun` call is being made).
    pub(super) run_exit: bool,
    /// PORT OPTION `Tuning::slide_any_direction` (not a field of the game's state): the fall
    /// line's yaw the slide was started on. Begin turns him to it and the stick's target yaw is
    /// kept within `Tuning::slide_steer_cone` of it. `None`: the game's rule is in force.
    pub(super) fall_yaw: Option<f32>,
    cone: f32,
}

impl SlopeSlide {
    /// Reset (0x1413D0BA0).
    pub const RESET: SlopeSlide = SlopeSlide { sub: 0, stick: false, sliding: false, yaw: 0.0, increment: 0.0, angle: 0.0, time: 0.0, run_exit: false, fall_yaw: None, cone: 0.0 };
}

/// The gate (iface slot 187 0x14142DE50) and `TrySlopeSlide(bRunArg)` (slot 188 0x141441490):
/// the start probe; then by the state: a forced floor always; IDLE with the latch; RUN with
/// `run_arg` or the latch; JUMP (the landing) always. The latch (pawn+0xD80 bit 2) is kept as the
/// function keeps it. The queue is kept: the caller chains.
pub(crate) fn try_slope_slide(state: &StateData, cx: &mut Ctx, run_arg: bool) -> bool {
    let (w, s) = (cx.world, &mut *cx.pawn);
    let latch = s.actions.traversal.slope_latch;
    let (is_idle, is_run, is_jump) = (matches!(state, StateData::Idle(_)), matches!(state, StateData::Run(_)), matches!(state, StateData::Jump(_)));
    if s.queue.queued_id().is_none() {
        // PORT OPTION `Tuning::slide_any_direction` (user decision 2026-10-12): no facing test,
        // no latch / run condition: a flagged slope under him always slides, down its fall line.
        let any = s.tuning.slide_any_direction;
        let probe = probes::slope_start_facing(s, w, any);
        if probe.ok {
            let start = any || probe.forced || (is_idle && latch) || (is_run && (run_arg || latch)) || is_jump || !(is_idle || is_run);
            if start {
                s.queue.push_state(SlopeSlide { fall_yaw: any.then_some(probe.fall_yaw), cone: s.tuning.slide_steer_cone, ..SlopeSlide::RESET });
                s.actions.traversal.slope_latch = probe.forced || (is_jump && !latch);
                return true;
            }
        }
        if is_idle || is_run {
            s.actions.traversal.slope_latch = false;
        } else if is_jump && latch {
            s.actions.traversal.slope_latch = probe.forced;
        }
        return false;
    }
    false
}

/// `MoveInput` SLOPESLIDE branch: +0x8C = stick yaw, +0x88 = 1 for any active stick.
pub(crate) fn move_input(state: &mut StateData, yaw: f32) {
    if let StateData::SlopeSlide(d) = state {
        // PORT OPTION `Tuning::slide_any_direction`: a stick pushed UPHILL (more than 90 degrees
        // off the fall line) is no steering and no run request: he keeps sliding as he was.
        if d.fall_yaw.is_some_and(|fall| wrap_degrees(yaw - fall).abs() > 90.0) {
            return;
        }
        (d.yaw, d.stick) = (d.steer(yaw), true);
    }
}

impl SlopeSlide {
    /// PORT OPTION `Tuning::slide_any_direction`: the stick's yaw kept within the cone round
    /// the fall line (he cannot steer back uphill). The game's rule: the yaw as it is.
    fn steer(&self, yaw: f32) -> f32 {
        match self.fall_yaw {
            Some(fall) => wrap_degrees(fall + wrap_degrees(yaw - fall).clamp(-self.cone, self.cone)),
            None => yaw,
        }
    }
}

impl PlayerState for SlopeSlide {
    const ID: StateId = StateId::Slopeslide;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// Begin 0x1413CB400.
    fn begin(&mut self, cx: &mut Ctx) {
        actions::sheathe(cx);
        let s = &mut *cx.pawn;
        s.anim.play_with(AnimId::SlopeSlide, PlayArgs::mode(1, A.slope_blend_in));
        self.sliding = true;
        // T = MySlopeSlideAccelTime x 60; +0x94 = (MySlopeSlideSpeed - |Velocity|) / T.
        let ticks = A.slope_accel_time * 60.0;
        if ticks > 0.0 {
            self.increment = (A.slope_speed - s.cmc.velocity.length()) / ticks;
        }
        self.yaw = s.yaw;
        // PORT OPTION `Tuning::slide_any_direction`: he is turned to the fall line at once (the
        // state moves him along his facing), and what speed he had uphill is not carried.
        if let Some(fall) = self.fall_yaw {
            let off = wrap_degrees(s.yaw - fall).abs();
            if off > 90.0 {
                s.cmc.stop_movement();
                if ticks > 0.0 {
                    self.increment = A.slope_speed / ticks;
                }
            }
            if off > self.cone {
                s.yaw = fall;
            }
            self.yaw = s.yaw;
        }
        s.actions.traversal.slope_latch = true;
        (s.actions.second_jump, s.actions.flow_jump, s.actions.superslide_ok) = (true, true, true);
        s.actions.traversal.begun[4] += 1;
        s.event(EventKind::Traversal { kind: "slope", id: 0, on: true });
    }

    /// Tick 0x1413D1EA0.
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        let per_tick = dt * 60.0;
        // (The rail probe of this tick cannot start a rail: the gate takes Idle / Run / Jump only.)
        let speed_now = cx.pawn.cmc.velocity.length();
        if self.sub == 1 {
            let s = &mut *cx.pawn;
            if s.anim.ended || (self.stick && s.queue.flags.enable_change) {
                s.queue.do_finish();
            }
            self.stick = false;
            return;
        }
        // Sub 0.
        if hopping::try_hopping(&StateData::SlopeSlide(*self), cx, self.yaw, false) {
            cx.pawn.queue.do_finish();
            return;
        }
        let (w, s) = (cx.world, &mut *cx.pawn);
        let mut end = false;
        let mut ok = false;
        // Blocked: the actual movement under half of the commanded speed.
        if s.cmc.displacement_velocity.length() < speed_now * A.slope_blocked_ratio && self.time > 0.0 {
            end = true;
        } else {
            let probe = probes::slope_continue(s, w);
            if probe.no_floor {
                let yaw = s.yaw;
                jump::fall(s, yaw);
                return;
            }
            ok = probe.ok;
            if ok {
                self.angle = probe.angle;
            } else if self.angle >= A.slope_uphill_end_angle {
                end = true;
            } else if self.stick && speed_now < A.slope_run_exit_speed {
                self.run_exit = true;
                // (With the port option the run goes on the way the slide went; the stick then
                // steers the run as usual.)
                run::try_run(&mut StateData::SlopeSlide(*self), cx, self.yaw, 1.0);
                cx.pawn.queue.do_chain();
                return;
            } else if speed_now >= A.slope_min_speed {
                self.angle = probe.angle;
            } else {
                end = true;
            }
        }
        let s = &mut *cx.pawn;
        if end {
            self.sliding = false;
            self.sub = 1;
            s.cmc.stop_movement();
            s.cmc.direct_xy = None;
            s.anim.play_with(AnimId::LightLandR, PlayArgs::mode(2, A.slope_blend_in));
            s.queue.flags.enable_change = false;
        }
        // TAIL: the yaw eases 2 % per tick toward the target yaw.
        s.yaw = wrap_degrees(s.yaw + wrap_degrees(self.yaw - s.yaw) * (A.slope_yaw_rate * per_tick).min(1.0));
        let mut speed = if end { 0.0 } else { speed_now };
        if !end {
            if !ok {
                let (dec, target) = (A.slope_decel_factor, 0.0f32);
                if speed > target {
                    speed = (speed - dec * self.increment.abs() * per_tick).max(target);
                }
            } else {
                speed = (speed + self.increment * per_tick).min(A.slope_speed.max(speed));
            }
            // MaxWalkSpeed = speed; Velocity.XY = speed x the facing (the walking move follows the floor).
            s.cmc.max_walk_speed = speed;
            let f = dir(s.yaw);
            s.cmc.set_velocity_xy(Vec2::new(f.x, f.y) * speed);
        }
        self.stick = false;
        self.time += dt;
    }

    /// End 0x1413CE320.
    fn end(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        s.cmc.max_walk_speed = s.params().speed_run;
        s.cmc.direct_xy = None;
        s.event(EventKind::Traversal { kind: "slope", id: 0, on: false });
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}

/// The jump gate's SLOPESLIDE branch (on the ground) and `JumpRequest`: yaw = +0x8C, type 0 (1
/// with HIGHJUMP).
pub(super) fn jump_yaw(d: &SlopeSlide, s: &Pawn) -> Option<f32> {
    on_ground(s).then_some(d.yaw)
}
