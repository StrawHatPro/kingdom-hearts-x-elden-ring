//! `TresPlayerState_Floating` (id 5): docs/sora-actions-spec.md II.2 "Floating" (instruction
//! level). OWNER: the actions implementer (tier 3).
//!
//! The floating mode is a switch on the pawn (pawn+0xD80 bit 0, bit 1 = "special"), written only
//! by the script function `BP_NotifySetFloatingMode(IsOn, IsSpecial)`: a scripted set piece of
//! one map (bt_04), not a field mechanic. THE HOST sets it: `Sora::set_floating(on, special)`.
//! While it is on "on ground" is false for every caller; Idle and Jump push this state. He
//! hangs in Falling mode at gravity scale 0.01 (special: 0.6), `GlideIdle`; the stick above 0.5
//! moves him (`GlideLoop`, 680 cm/s, special 1500) along a heading that lags the stick; released,
//! he coasts, the speed decaying 2 % per 60 Hz tick. Jump (a Doubleflight), guard and air slide
//! work out of it; the state ends when the switch is cleared.

use glam::Vec2;

use super::common::{ground_probe, set_movement_input, turn};
use super::{PlayerState, StateData, StateId};
use crate::actions::probes::dir;
use crate::actions::{self, A};
use crate::anim::{AnimId, PlayArgs};
use crate::host::EventKind;
use crate::pawn::{Ctx, Pawn};
use crate::sora::{wrap_degrees, MovementState};

/// The fields of II.2's table.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Floating {
    /// 0 start, 1 hanging (`GlideIdle`), 2 moving (`GlideLoop`).
    sub: u8,
    /// +0x88 stick flag (magnitude > 0.5), +0x8C target yaw, +0x90 magnitude.
    stick: bool,
    yaw: f32,
    magnitude: f32,
    /// +0x98: the smoothed travel heading (yaw).
    heading: f32,
    /// +0xA4 coasting, +0xA8 coast speed.
    coasting: bool,
    coast_speed: f32,
}

impl Floating {
    /// Reset (0x14136ED60).
    pub const RESET: Floating = Floating { sub: 0, stick: false, yaw: 0.0, magnitude: 0.0, heading: 0.0, coasting: false, coast_speed: 0.0 };
}

/// The entries (`PushState(5)` when `Floating()`: `Idle::Begin` / tick, `Jump::Begin` / tick
/// tail). The caller chains.
pub(crate) fn try_enter(state: &StateData, s: &mut Pawn) -> bool {
    if !s.actions.traversal.floating || !matches!(state, StateData::Idle(_) | StateData::Jump(_)) || s.queue.queued_id().is_some() {
        return false;
    }
    s.queue.push_state(Floating::RESET);
    true
}

/// `MoveInput` case FLOATING (0x141441099): magnitude > 0.5: +0x8C = yaw, +0x88 = 1; +0x90 = mag always.
pub(crate) fn move_input(state: &mut StateData, yaw: f32, magnitude: f32) {
    if let StateData::Floating(f) = state {
        if magnitude > A.floating_stick_min {
            (f.yaw, f.stick) = (yaw, true);
        }
        f.magnitude = magnitude;
    }
}

/// Pawn vt+0x1AE0 / vt+0x1AE8: the speed and the gravity scale by the "special" bit.
fn speed_and_gravity(s: &Pawn) -> (f32, f32) {
    if s.actions.traversal.floating_special {
        (A.glide_speed, A.floating_gravity_scale_special)
    } else {
        (A.floating_speed, A.floating_gravity_scale)
    }
}

impl PlayerState for Floating {
    const ID: StateId = StateId::Floating;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// Begin 0x14136ABE0: SetMovementMode(3 Falling, bStop 0): the entry velocity is KEPT.
    fn begin(&mut self, cx: &mut Ctx) {
        actions::sheathe(cx);
        let s = &mut *cx.pawn;
        s.cmc.set_falling();
        s.cmc.gravity_scale = speed_and_gravity(s).1;
        self.yaw = s.yaw;
        s.event(EventKind::Traversal { kind: "floating", id: 0, on: true });
    }

    /// Tick 0x14137A680.
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        let (w, s) = (cx.world, &mut *cx.pawn);
        if !s.actions.traversal.floating {
            s.queue.do_chain();
            return;
        }
        // Near the ground (the probe's distance <= 30): all five availability bits are refreshed.
        let (hit, distance, _) = ground_probe(s, w, A.floating_ground_probe_range);
        if hit && distance <= A.floating_ground_refresh_dist {
            let a = &mut s.actions;
            (a.second_jump, a.air_slide_ok, a.air_guard_ok, a.flow_jump, a.superslide_ok) = (true, true, true, true, true);
        }
        let per_tick = dt * 60.0;
        match self.sub {
            0 => {
                s.anim.play_with(AnimId::GlideIdle, PlayArgs::mode(3, -1.0));
                (self.coasting, self.coast_speed, self.sub) = (false, 0.0, 1);
            }
            1 => {
                if self.stick {
                    self.coasting = false;
                    s.cmc.direct_xy = None;
                    s.anim.play_with(AnimId::GlideLoop, PlayArgs::mode(3, -1.0));
                    s.anim.set_axis(0.0);
                    self.heading = s.yaw;
                    self.sub = 2;
                }
                turn(s, self.yaw, dt * A.floating_turn_rate);
                if self.coasting {
                    // +0xA8 *= 0.98 per tick; the direct velocity XY along the heading.
                    self.coast_speed *= A.floating_coast_decay.powf(per_tick);
                    let f = dir(self.heading);
                    s.cmc.set_velocity_xy(Vec2::new(f.x, f.y) * self.coast_speed);
                }
            }
            _ => {
                if self.stick {
                    let d = wrap_degrees(self.yaw - s.yaw);
                    if let Some((low, high)) = s.anim.axis_range() {
                        s.anim.set_axis(d.clamp(low, high));
                    }
                    turn(s, self.yaw, dt * A.floating_turn_rate);
                    // The heading slerps toward the stick yaw by clamp01(dt x 3).
                    self.heading = wrap_degrees(self.heading + wrap_degrees(self.yaw - self.heading) * (dt * A.floating_heading_rate).clamp(0.0, 1.0));
                    let speed = speed_and_gravity(s).0;
                    s.cmc.max_acceleration = speed * 60.0;
                    s.cmc.max_walk_speed = speed;
                    set_movement_input(s, self.heading);
                } else {
                    // Released: coast at the measured horizontal speed.
                    self.coasting = true;
                    self.coast_speed = s.cmc.displacement_velocity.truncate().length();
                    let f = dir(self.heading);
                    s.cmc.set_velocity_xy(Vec2::new(f.x, f.y) * self.coast_speed);
                    s.anim.play_with(AnimId::GlideIdle, PlayArgs::mode(3, -1.0));
                    self.sub = 1;
                }
            }
        }
        self.stick = false;
    }

    /// End 0x14136CF30.
    fn end(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        let p = *s.params();
        s.cmc.direct_xy = None;
        s.cmc.max_walk_speed = p.speed_run;
        s.cmc.gravity_scale = p.gravity_scale;
        s.cmc.gravity_off = false;
        s.cmc.set_falling();
        s.event(EventKind::Traversal { kind: "floating", id: 0, on: false });
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}
