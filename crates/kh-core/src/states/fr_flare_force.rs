//! OWNER: party stream D. `TresPlayerState_FRFlareForce` (state 136, Flare Force, command 208):
//! Reset 0x14136E5F0, Begin 0x141369090, Tick 0x1413755D0, End 0x14136BBC0 (decompiled C).
//!
//! ```text
//! Begin: vt+0x330 (command bookkeeping); pawn vt+0x16D8(1); link pawn = FUN_14127A780(friend
//!        manager, 208); if it exists: SpecificAction(0, 0); vt+0x358 (the same shell as FRCallMeteor)
//! Tick:  sub 1: wait while the link pawn runs (it holds Sora; flag +0x84 bit 7 or the link gone) -> 3
//!        sub 3: detach (vt+0x23B0(0)), the link's end (vt+0x2E8, once),
//!               play "M_1501" (blend 0), command notify (+0x448 vt+0x170) -> 4
//!        sub 4: vt+0xDC8(dt x 15); at the clip's end (on the ground: controller vt+0x518) DoFinish
//! End:   vt+0x370; the link pawn's end (FUN_1412D0040(0)) unless it ended itself
//! ```
//! The link pawn (`ATresFriendLinkPawnFlareForce`) is on the friends' side (`kh_party::team`).

use super::{PlayerState, StateId};
use crate::anim::Notifies;
use crate::combat::friend::FrLink;
use crate::pawn::Ctx;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FrFlareForce {
    pub(super) link: FrLink,
}

impl FrFlareForce {
    pub fn new(link: FrLink) -> FrFlareForce {
        FrFlareForce { link }
    }
}

impl PlayerState for FrFlareForce {
    const ID: StateId = StateId::FrFlareforce;

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
