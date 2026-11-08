//! (F-ARROWGUN) `TresPlayerState_ComboBZ` (id 78): the attack state of Magic Launcher (0x83) and Hunny Launcher (0x8F):
//! `TresPlayerState_Attack` with Decide 0x1413602A0 and the shot handler 0x141357FA0. Its code is shared with ComboDW
//! (states/combo_dw.rs, which documents both classes).

use super::attack::Attack;
use super::{PlayerState, StateId};
use crate::anim::Notifies;
use super::style_finish::StyleFinish;
use crate::combat::forms::arrowgun::{self as ag, Family};
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ComboBz {
    /// The Attack state's fields.
    pub(crate) a: Attack,
}

impl PlayerState for ComboBz {
    const ID: StateId = StateId::ComboBz;

    fn sub_step(&self) -> u8 {
        self.a.sub
    }

    fn begin(&mut self, cx: &mut Ctx) {
        super::combo_dw::begin(&mut self.a, cx, Family::Bz);
    }

    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        self.a.tick(cx, dt);
    }

    fn end(&mut self, cx: &mut Ctx) {
        super::combo_dw::end(&mut self.a, cx);
    }

    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        super::combo_dw::on_notifies(&mut self.a, cx, notifies, Family::Bz);
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}

// ================================================================================================ FINISH_BZ

/// StyleFinish Begin for 0x9D (FINISH_BZ): the common part only (Begin 0x1413CBF70 binds the delegate vt+0x478 = slot 143;
/// nothing else of Begin / End / Reset depends on 0x9D).
pub(crate) fn finish_begin(f: &mut StyleFinish, _cx: &mut Ctx) {
    f.a.sub = 0;
    f.a.shots = 0;
}

/// StyleFinish slot 143 0x1413DF100 (FINISH_BZ): sub 0 plays `M_5417` (blend -1, mode 2), voice variant 0, m_bFaceAt, sub 5;
/// sub 5 DoChains at the clip's end. No gravity / traction / camera writes: gravity stays off from Begin to End. Its
/// TresShot (StyleFinish anim interface 0x1413DD180) at the notify's time: `arrowgun::big_bang`.
pub(crate) fn finish_tick(f: &mut StyleFinish, cx: &mut Ctx, _dt: f32) -> bool {
    let Some(row) = ag::finish_row() else { return true };
    match f.a.sub {
        0 => {
            f.play(cx.pawn, row.logical, -1.0, 2);
            cx.set_voice_group_no(0);
            f.a.face_at = true;
            f.a.sub = 5;
            false
        }
        _ => {
            if f.a.shots == 0 && cx.pawn.anim.time >= ag::v(row, 0) {
                f.a.shots = 1;
                ag::big_bang(cx, row);
            }
            if cx.pawn.anim.ended {
                cx.pawn.queue.do_chain();
                return true;
            }
            false
        }
    }
}
