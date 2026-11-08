//! (weapons F-LIGHTDARK) `TresPlayerState_ComboDF` (id 160): the attack state of Dark Form (0xDE). The Attack class with
//! ComboDF's Decide (0x1413606F0) and warp (slot 136 0x141352160): states/combo_lf.rs (the shared parts) and
//! combat/forms/lightdark.rs.

use super::combo_lf::{self, ComboLf};
use super::{PlayerState, StateId};
use crate::anim::Notifies;
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ComboDf(pub(crate) ComboLf);

impl PlayerState for ComboDf {
    const ID: StateId = StateId::ComboDf;

    fn sub_step(&self) -> u8 {
        self.0.a.sub_step()
    }

    fn begin(&mut self, cx: &mut Ctx) {
        combo_lf::begin(&mut self.0, cx);
    }

    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        combo_lf::tick(&mut self.0, cx, dt);
    }

    fn end(&mut self, cx: &mut Ctx) {
        combo_lf::end(&mut self.0, cx);
    }

    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        combo_lf::on_notifies(&mut self.0, cx, notifies);
    }

    fn movement_state(&self, pawn: &Pawn) -> MovementState {
        self.0.a.movement_state(pawn)
    }
}
