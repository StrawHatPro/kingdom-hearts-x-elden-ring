//! `TresPlayerState_Glide` (id 4) and its request `TryGlide`: docs/sora-actions-spec.md II.2
//! "Glide" (instruction level; addresses are the exe's).
//!
//! Movement mode Falling at GravityScale 0.001, MaxWalkSpeed 1500, MaxAcceleration 90000, the
//! input vector = the facing: 1425 cm/s after the first movement tick, 1500 from the second;
//! the facing eases toward the stick yaw by 3 % of the error per 60 Hz tick.
//!
//! WHAT ENDS A GLIDE (II.2 "What actually ends a glide", question Q2): by the code as read,
//! ONLY releasing the button (or a state that replaces it). There is no ground test, no wall
//! test and no timer in the tick, and the landed handler is empty: carried onto rising ground
//! the movement component walks (mode Walking, 1500 cm/s) while the state stays GLIDE; on release
//! `End` forces Falling with zero velocity and he lands through the normal landing. This literal
//! reading is implemented, under `Tuning::GAME_EXACT` and under the user's options alike. The
//! port option `Tuning::glide_ends_on_ground` (default OFF) ends the glide on ground contact
//! instead; it is not the game's code. TO CONFIRM against the running game.

use super::common::{on_ground, set_movement_input, turn};
use super::{PlayerState, StateData, StateId};
use crate::actions::{self, A};
use crate::anim::{AnimId, PlayArgs};
use crate::enums::ETresAbilityKind as Ability;
use crate::pawn::{Ctx, Pawn};
use crate::sora::{wrap_degrees, MovementState};

/// State +0x88 "jump held this frame", +0x8C target yaw, +0x90 stick magnitude (stored, not
/// read), +0x98 last yaw delta (the blend-space X).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Glide {
    sub: u8,
    held: bool,
    yaw: f32,
    magnitude: f32,
    delta: f32,
    /// PORT OPTION `Tuning::glide_ends_on_ground` ended it: End leaves him standing.
    grounded: bool,
}

impl Glide {
    /// The state object after `Reset` (0x14138F4E0).
    pub const RESET: Glide = Glide { sub: 0, held: false, yaw: 0.0, magnitude: 0.0, delta: 0.0, grounded: false };
}

/// The gate (slot 66 0x141429390).
fn can_glide(state: &StateData, s: &Pawn) -> bool {
    // !(pawn+0xE34 > 0): the 0.6 s lock pawn slot 981 writes (the combat side's command
    // cool-down is that same field); HasAbility(GLIDE 0x14); nothing queued.
    if s.combat.command_cooldown > 0.0 || !actions::has(s, Ability::Glide) || !s.queue.nothing_queued() {
        return false;
    }
    match state {
        // JUMP: Jump+0xF8 == 0 && sub in {4, 6, 8} (the falling sub-steps; never while rising).
        StateData::Jump(j) => !j.flow && matches!(j.sub, 4 | 6 | 8),
        StateData::Glide(_) => true,
        #[allow(unreachable_patterns)]
        _ => false,
    }
}

/// `TryGlide(yaw, mag)` (slot 67 0x14143C5B0): the jump key is down and was not pressed this
/// frame. From JUMP: `SetState(GLIDE)`; in GLIDE: the running state. Then `+0x88 = 1`,
/// `+0x8C = yaw` when mag > 0.5, `+0x90 = mag`.
pub(crate) fn try_glide(state: &mut StateData, cx: &mut Ctx, yaw: f32, magnitude: f32) -> bool {
    let s = &mut *cx.pawn;
    if !can_glide(state, s) {
        return false;
    }
    let steer = magnitude > A.glide_yaw_stick_min;
    match state {
        StateData::Glide(g) => {
            g.held = true;
            if steer {
                g.yaw = yaw;
            }
            g.magnitude = magnitude;
        }
        _ => s.queue.set_state(Glide { held: true, yaw: if steer { yaw } else { 0.0 }, magnitude, ..Glide::RESET }),
    }
    true
}

impl PlayerState for Glide {
    const ID: StateId = StateId::Glide;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// Begin 0x141384D20.
    fn begin(&mut self, cx: &mut Ctx) {
        actions::sheathe(cx);
        let s = &mut *cx.pawn;
        // StopMovement: the entry velocity is discarded on all axes. Falling, gravity on.
        s.cmc.stop_movement();
        s.cmc.set_falling();
        s.cmc.gravity_off = false;
        s.cmc.gravity_scale = A.glide_gravity_scale;
        s.cmc.max_walk_speed = A.glide_speed;
        self.sub = 1;
        // +0x8C = actor yaw (0x141384E6E): the request's yaw of the entry frame is overwritten.
        self.yaw = s.yaw;
        // Play("GlideLoop", blend-in -1, mode 3); blend X = 0.
        s.anim.play_with(AnimId::GlideLoop, PlayArgs::mode(3, -1.0));
        s.anim.set_axis(0.0);
        // The air-slide (0.6 s) and air-guard (0.3333 s) cool-downs start (0x141385176 / 0x14138518B).
        s.combat.air_slide_cooldown = A.airslide_cooldown;
        s.combat.air_guard_cooldown = A.guard_air_cooldown;
    }

    /// Tick 0x141391D00.
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        let s = &mut *cx.pawn;
        // PORT OPTION `Tuning::glide_ends_on_ground` (not the game's code; default off).
        if s.tuning.glide_ends_on_ground && on_ground(s) {
            self.grounded = true;
            s.queue.do_chain();
            return;
        }
        if !self.held {
            // The button is not held this frame (0x141391E09).
            s.queue.do_chain();
        } else {
            let d = wrap_degrees(self.yaw - s.yaw);
            self.delta = d;
            s.anim.set_axis(d);
            // Turn((0, +0x8C, 0), MyGlideTurnRate * dt * 60).
            turn(s, self.yaw, A.glide_turn_rate * dt * 60.0);
            // SetMaxAcceleration(MyGlideSpeed * 60); the input = the facing after the turn.
            s.cmc.max_acceleration = A.glide_speed * 60.0;
            let yaw = s.yaw;
            set_movement_input(s, yaw);
        }
        self.held = false;
    }

    /// End 0x14138C840: `SetMovementMode(3 Falling, bStop)`: the velocity is zeroed, also on the
    /// ground; ResetMaxWalkSpeed; ResetGravityScale; gravity on.
    fn end(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        let p = *s.params();
        s.cmc.stop_movement();
        if !self.grounded {
            s.cmc.set_falling();
        }
        s.cmc.max_walk_speed = p.speed_run;
        s.cmc.gravity_scale = p.gravity_scale;
        s.cmc.gravity_off = false;
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}
