//! `TresPlayerState_ComboLimit` (id 90): the attack state of Second Form (combat spec 10.5). The
//! class overrides ONLY `Decide` (slot 122 0x141362AB0 -> `combat::decide::decide_limit`);
//! everything else is `TresPlayerState_Attack` (attack.rs), which this wraps. Which class an
//! attack request creates is the pawn's (slot 1337 0x141432F00: 90 while the form's command is
//! of the Second Form family): `attack::queue_attack`.

use super::attack::Attack;
use super::{PlayerState, StateId};
use crate::anim::Notifies;
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ComboLimit(pub Attack);

impl PlayerState for ComboLimit {
    const ID: StateId = StateId::ComboLimit;

    fn sub_step(&self) -> u8 {
        self.0.sub_step()
    }

    fn begin(&mut self, cx: &mut Ctx) {
        self.0.limit = true;
        self.0.begin(cx);
    }

    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        self.0.tick(cx, dt);
    }

    fn end(&mut self, cx: &mut Ctx) {
        self.0.end(cx);
    }

    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        self.0.on_notifies(cx, notifies);
    }

    fn movement_state(&self, pawn: &Pawn) -> MovementState {
        self.0.movement_state(pawn)
    }
}
