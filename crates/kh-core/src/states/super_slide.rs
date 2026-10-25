//! `TresPlayerState_SuperSlide` (id 21) and the flow trigger `TryFlow`: docs/sora-actions-spec.md
//! II.2 "Superslide" (instruction level; addresses are the exe's). Tier 2: kind 1 (the plain
//! Superslide) out of a flow jump or a wall kick; the pole launch (aim at a target), the rail
//! and wall-run entries and the pole probe of the tick come with those states.
//!
//! Flying mode, gravity off, the velocity written every tick along the facing: 4000 cm/s
//! falling by 1000 cm/s^2 to 3000 over the 1.0 s of the slide (3492 cm at 60 Hz), then `SlideEnd`
//! (426 cm of root motion, 0.7667 s, no cancel). The stick steers: 2.5 % of the yaw error per
//! 60 Hz tick. A wall met after the first 0.1 s is kicked (`actions::move_blocked`).

use super::common::{on_ground, turn};
use super::{PlayerState, StateData, StateId};
use crate::actions::{self, A};
use crate::anim::{AnimId, PlayArgs};
use crate::enums::ETresAbilityKind as Ability;
use crate::pawn::{Ctx, Pawn};
use crate::sora::{wrap_degrees, MovementState};

/// +0x88 stick flag, +0x8C target yaw, +0x90 time left, +0x94 sliding, +0x9C / +0xA0 / +0xA4
/// speed / minimum speed / brake, +0xA8 bit 1 yaw given.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SuperSlide {
    sub: u8,
    stick: bool,
    yaw: f32,
    time_left: f32,
    sliding: bool,
    speed: f32,
    min_speed: f32,
    brake: f32,
    yaw_given: bool,
}

impl SuperSlide {
    /// The state object after `Reset` (0x1413D0E40) and `PushSuperSlide(kind 1, null)`.
    pub const RESET: SuperSlide = SuperSlide { sub: 0, stick: false, yaw: 0.0, time_left: 0.0, sliding: false, speed: 0.0, min_speed: 0.0, brake: 0.0, yaw_given: false };

    /// The move-blocked handler's condition (0x1413DCA6A): more than 0.1 s into the slide and
    /// still sliding (+0x94).
    pub(crate) fn may_wall_kick(&self, _s: &Pawn) -> bool {
        self.sliding && A.superslide_time - A.superslide_probe_delay > self.time_left
    }
}

/// The flow gate (slot 71 0x14142E320) and `TryFlow(1, 0)` (slot 72 0x1414417B0): the FIRST
/// request of the guard button. JUMP: a flow jump (Jump+0xF8), pawn bit 4 and SUPERSLIDE (19):
/// ClearQueue, push the Superslide. WALL_KICK: the Superslide latch. (WALLRUN latches and
/// returns false; POLE_TURN and RAIL: with those states.)
pub(crate) fn try_flow(state: &mut StateData, cx: &mut Ctx) -> bool {
    let s = &mut *cx.pawn;
    if !s.actions.flow_unlocked || !actions::has(s, Ability::Superslide) {
        return false;
    }
    match state {
        StateData::Jump(j) if j.flow && s.actions.superslide_ok => {
            s.queue.clear();
            s.queue.push_state(SuperSlide::RESET);
            true
        }
        StateData::WallKick(k) => {
            k.slide_latch = true;
            true
        }
        // WALLRUN (case 0x141441841): flags |= 0x40 and the function returns FALSE: the same press
        // goes on to the air slide / guard / dodge; the next accepted probe turns it into a kick.
        StateData::WallRun(r) => {
            r.guard_latch = true;
            false
        }
        // POLE_TURN (case 0x1414418F9): the guard latch (state slot 114).
        StateData::PoleTurn(p) => {
            super::pole_turn::flow_latch(p, s);
            true
        }
        // RAIL_SLIDE: ClearQueue; a plain Superslide from the rail position.
        StateData::RailSlide(r) => super::rail_slide::flow(r, s),
        _ => false,
    }
}

/// `PushSuperSlide(kind 1, pYaw)` (iface slot 73 0x141445B90): `PushState(21)`; with a yaw: flag 2 and
/// +0x8C (Begin then snaps to it). THE ENTRY CALL for other states (wall kick; the airstep arrival of
/// the magic side, pole, rail); the caller clears the queue or chains as its own code says.
#[allow(dead_code)]
pub(crate) fn push(s: &mut Pawn, yaw: Option<f32>) {
    s.queue.push_state(SuperSlide { yaw: yaw.unwrap_or(0.0), yaw_given: yaw.is_some(), ..SuperSlide::RESET });
}

/// `MoveInput` case SUPER_SLIDE (0x141440A15): +0x8C = yaw; +0x88 = 1: the stick steers.
pub(crate) fn move_input(state: &mut StateData, yaw: f32) {
    if let StateData::SuperSlide(s) = state {
        (s.yaw, s.stick) = (yaw, true);
    }
}

impl PlayerState for SuperSlide {
    const ID: StateId = StateId::SuperSlide;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// Begin 0x1413CC460.
    fn begin(&mut self, cx: &mut Ctx) {
        // pawn bit 4 = 0; StopMovement; SetMovementMode(5 Flying, bStop); gravity off; Draw.
        cx.pawn.actions.superslide_ok = false;
        cx.pawn.cmc.stop_movement();
        cx.pawn.cmc.set_flying();
        cx.pawn.cmc.gravity_off = true;
        actions::draw(cx, true);
        let s = &mut *cx.pawn;
        self.sub = 0;
        self.sliding = true;
        // The time by SUPERSLIDE copies (all three levels are 1.0 s).
        self.time_left = A.superslide_time;
        self.min_speed = A.superslide_min_speed;
        self.brake = A.superslide_brake.max(0.0);
        self.speed = A.superslide_speed.max(self.min_speed);
        // Play("SuperSlideLoop", blend-in 0.0, mode 1).
        s.anim.play_with(AnimId::SuperSlideLoop, PlayArgs::mode(1, 0.0));
        if self.yaw_given {
            s.yaw = wrap_degrees(self.yaw);
        } else {
            self.yaw = s.yaw;
        }
    }

    /// Tick 0x1413D7C40.
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        // The pole probe from 0.1 s on (II.4 "The pole component and its probe"; Part I 8).
        if self.sub == 0 && self.sliding && self.may_wall_kick(cx.pawn) && super::pole_turn::probe_and_try(&StateData::SuperSlide(*self), cx) {
            return;
        }
        let s = &mut *cx.pawn;
        if self.sub != 0 {
            // The end clip.
            if s.anim.ended {
                if !on_ground(s) {
                    // pawn+0xE38 = 0.6 (0x1413D7D1D).
                    s.combat.air_slide_cooldown = A.superslide_end_cooldown;
                }
                s.queue.do_chain();
            }
        } else if !(self.time_left > 0.0) {
            s.cmc.stop_movement();
            s.anim.play_with(AnimId::SlideEnd, PlayArgs::mode(2, 0.05));
            self.sub = 2;
            self.sliding = false;
        } else {
            turn(s, self.yaw, dt * A.superslide_turn_rate);
            self.speed = self.min_speed.max(self.speed - dt * self.brake);
            // component vt+0x9E8(v): the direct velocity on all three axes, along the facing.
            let a = s.yaw.to_radians();
            let v = self.speed.max(0.1);
            s.cmc.set_velocity_xy(glam::Vec2::new(a.cos(), a.sin()) * v);
            s.cmc.set_velocity_z(0.0);
            self.time_left -= dt;
            // (The pole probe from 0.1 s on: with the pole state.)
        }
    }

    /// End 0x1413CF350: SetMovementMode(3 Falling, bStop): velocity zeroed; gravity on.
    fn end(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        s.cmc.stop_movement();
        s.cmc.direct_xy = None;
        s.cmc.direct_z = None;
        s.cmc.set_falling();
        s.cmc.gravity_off = false;
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}
