//! `TresPlayerState_BadStatFreeze` (id 123, vtable 0x146909558; base
//! `TresCharState_BadStatFreeze`): the bad status FREEZE (7) as a state. docs/sora-combat-spec.md
//! 9.9.3, from the decompiled C of Reset 0x14150DDA0, Begin 0x14134E7A0 -> 0x1415087B0, Tick
//! 0x14150EDF0 (verdict `same`), End 0x1413502D0 -> 0x14150B7B0.
//!
//! The reaction gate starts it for a hit whose row has FREEZE with a time (spec 9.1) and writes
//! the time into state+0x88 (Reset gives 1.0); a second freezing hit while it runs only rewrites
//! the time. He is frozen in place: pose paused, no movement input, gravity on (a flying pawn
//! is set falling), covered by the ice effect. It ends when the time is up or the status is no
//! longer FREEZE (a cure); any other flinching hit replaces the state (the gate's `ClearQueue` +
//! DAMAGE), whose End breaks the ice. No input shortens it.

use super::{PlayerState, StateId};
use crate::combat::badstat::{self, kind};
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BadstatFreeze {
    sub: u8,
    /// state+0x88: seconds left (the hit's `m_BadStatusEffectTime`).
    pub time: f32,
}

impl BadstatFreeze {
    pub fn new(time: f32) -> BadstatFreeze {
        BadstatFreeze { sub: 0, time }
    }
}

impl PlayerState for BadstatFreeze {
    const ID: StateId = StateId::BadstatFreeze;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// `Begin`: movement input off, `StopMovement`, the actor levelled, MaxWalkSpeed /
    /// MaxFlySpeed / MaxAcceleration back to the class defaults, gravity on (pawn vt+0x1728(1)),
    /// a flying pawn -> Falling with the stop, `SetFrozen(1)` (0x1405B77B0), sub-step 1; the
    /// player part: the HUD lock.
    fn begin(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        let p = *s.params();
        s.cmc.stop_movement();
        s.cmc.control_input = glam::Vec3::ZERO;
        s.cmc.max_walk_speed = p.speed_run;
        s.cmc.max_acceleration = p.accel_default;
        s.cmc.gravity_off = false;
        if s.cmc.is_flying() {
            s.cmc.set_falling();
        }
        badstat::set_frozen(cx, true);
        badstat::hud_lock(cx, true);
        self.sub = 1;
    }

    /// `Tick`: the time runs down; the state finishes when the status is no longer FREEZE or
    /// the time is up.
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        self.time -= dt;
        if !cx.pawn.combat.bad.is(kind::FREEZE) {
            self.time = 0.0;
            cx.pawn.queue.do_finish();
        } else if self.time <= 0.0 {
            cx.pawn.queue.do_finish();
        }
    }

    /// `End`: the HUD lock goes, movement input on, `SetFrozen(0)`: the ice breaks (effect
    /// 0xC1F) and the status byte is cleared.
    fn end(&mut self, cx: &mut Ctx) {
        badstat::hud_lock(cx, false);
        badstat::set_frozen(cx, false);
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}

/// The gate's second freezing hit (spec 9.1): the running state's time is rewritten.
pub(crate) fn refresh(state: &mut super::StateData, time: f32) -> bool {
    match state {
        super::StateData::BadstatFreeze(f) => {
            f.time = time;
            true
        }
        _ => false,
    }
}
