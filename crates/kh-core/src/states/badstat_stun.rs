//! `TresPlayerState_BadStatStun` (id 122, vtable 0x1469098E0; base `TresCharState_BadStatStun`):
//! the bad status STUN (12) as a state. docs/sora-combat-spec.md 9.9.2, from the decompiled C of
//! Begin 0x14134E7E0 -> 0x141508930, Tick 0x141353140 -> 0x14150EE70, End 0x141350300 ->
//! 0x14150B820 (sub-step order and the status tests checked in the disassembly).
//!
//! He staggers (`BadStatStunStart` 1.37 s), sways (`BadStatStunLoop`, looping) for as long as the
//! status is on him, and recovers (`BadStatStunEnd` 1.07 s, `TresEnableChange` at 0.70 s). The
//! status's time counts down only while this state runs (`combat/badstat.rs`); every stick flick
//! takes 0.3 s more off it. A damaging hit cures the stun (and replaces the state by DAMAGE).
//! Out of the end clip he may attack / cast / use an item from `TresEnableChange` on (the
//! `CanAttack` row of state 122).

use super::{PlayerState, StateId};
use crate::anim::{AnimId, PlayArgs};
use crate::combat::badstat::{self, kind};
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BadstatStun {
    sub: u8,
    /// state+0x88: seconds the stick flicks have earned since the last tick (request interface
    /// slot 37 0x141440570 adds 0.3 per flick; the loop sub-step hands it to pawn vt+0x20D0).
    pub shake: f32,
}

impl BadstatStun {
    pub fn new() -> BadstatStun {
        BadstatStun { sub: 0, shake: 0.0 }
    }
}

pub(super) fn play_clip(s: &mut Pawn, name: &str, mode: u8) {
    match AnimId::from_name(name) {
        Some(clip) => s.anim.play_with(clip, PlayArgs::mode(mode, -1.0)),
        None => s.note("a bad-status clip (BadStatStun* / BadStatBurn*) is not in the generated animation table"),
    }
}

impl PlayerState for BadstatStun {
    const ID: StateId = StateId::BadstatStun;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// `Begin`: the actor is levelled (yaw kept), movement input off (movement vt+0xA40(1)),
    /// `StopMovement`, `Play(BadStatStunStart, mode 2)`, the stun effect (group 2, row 0xC58),
    /// and the player part: the HUD lock.
    fn begin(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        s.cmc.stop_movement();
        s.cmc.control_input = glam::Vec3::ZERO;
        play_clip(s, "BadStatStunStart", 2);
        badstat::hud_lock(cx, true);
    }

    /// `Tick`. (The player override first: in water with the SWIM state available the stun
    /// state finishes into SWIM: not ported, no swimming.)
    fn tick(&mut self, cx: &mut Ctx, _dt: f32) {
        match self.sub {
            // 0: the start clip ended -> the loop (mode 3).
            0 => {
                if cx.pawn.anim.ended {
                    play_clip(cx.pawn, "BadStatStunLoop", 3);
                    self.sub = 1;
                }
            }
            // 1: until the status is gone; the flicks' time is taken off it.
            1 => {
                if !cx.pawn.combat.bad.is(kind::STUN) {
                    play_clip(cx.pawn, "BadStatStunEnd", 2);
                    cx.pawn.queue.flags.enable_change = true;
                    self.sub = 2;
                    return;
                }
                if self.shake > 0.0 {
                    badstat::shorten(cx, self.shake);
                    self.shake = 0.0;
                }
            }
            // 2: the end clip.
            _ => {
                if cx.pawn.anim.ended {
                    cx.pawn.queue.do_finish();
                }
            }
        }
    }

    /// `End`: the HUD lock goes, movement input on, the rail-slide work is reset. (The status
    /// itself is not touched: a state change with the stun still on him - a jump pad, a hit
    /// without damage - leaves it, and the trigger brings the state back when he stands.)
    fn end(&mut self, cx: &mut Ctx) {
        badstat::hud_lock(cx, false);
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}
