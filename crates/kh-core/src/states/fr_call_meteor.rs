//! OWNER: party stream D. `TresPlayerState_FRCallMeteor` (state 135, Donald Meteor, command 207):
//! Reset 0x14136E5B0, Begin 0x141368F90, Tick 0x141375420, End 0x14136BB10 (decompiled C).
//!
//! ```text
//! Begin: vt+0x330 (command bookkeeping); pawn vt+0x16D8(1); link pawn = FUN_14127A780(friend
//!        manager, 207); if it exists: its start (FUN_1412E4A20) and SpecificAction(0, 0); vt+0x358
//! Tick:  sub 1: wait while the link pawn runs (it holds Sora; flag +0x84 bit 7 or the link gone) -> 3
//!        sub 3: pawn vt+0xD98(1), detach (vt+0x23B0(0)), the link's end (vt+0x2E8, once),
//!               play "M_1501" (blend 0), command notify (+0x448 vt+0x170) -> 4
//!        sub 4: vt+0xDC8(dt x 15); at the clip's end (on the ground: controller vt+0x518) DoFinish
//! End:   pawn vt+0xD98(1); vt+0x370; the link pawn's end (FUN_1412CF9A0(0)) unless it ended itself
//! ```
//! The link pawn (`ATresFriendLinkPawnCallMeteor`) is on the friends' side (`kh_party::team`).

use super::{PlayerState, StateId};
use crate::anim::Notifies;
use crate::combat::friend::FrLink;
use crate::pawn::Ctx;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FrCallMeteor {
    pub(super) link: FrLink,
}

impl FrCallMeteor {
    pub fn new(link: FrLink) -> FrCallMeteor {
        FrCallMeteor { link }
    }
}

impl PlayerState for FrCallMeteor {
    const ID: StateId = StateId::FrCallmeteor;

    fn sub_step(&self) -> u8 {
        self.link.sub
    }

    fn begin(&mut self, cx: &mut Ctx) {
        self.link.begin(cx, Self::ID);
    }

    fn tick(&mut self, cx: &mut Ctx, _dt: f32) {
        match self.link.sub {
            1 => {
                if self.link.hold(cx, Self::ID) {
                    self.link.sub = 3;
                }
            }
            3 => {
                self.link.detach(cx);
                self.link.play_end(cx, "M_1501", 0.0);
                self.link.sub = 4;
            }
            4 => {
                if cx.pawn.anim.ended {
                    self.link.sub = 2;
                    cx.pawn.queue.do_finish();
                }
            }
            _ => {}
        }
    }

    fn end(&mut self, cx: &mut Ctx) {
        self.link.end(cx);
    }

    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        self.link.on_notifies(cx, notifies);
    }
}
