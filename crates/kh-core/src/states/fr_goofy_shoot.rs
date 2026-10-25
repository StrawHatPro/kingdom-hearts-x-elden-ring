//! OWNER: party stream D. `TresPlayerState_FRGoofyShoot` (state 101, Goofy Bombardier, command
//! 195): Reset 0x14136E670, Begin 0x141369250, Tick 0x141375B30, End 0x14136BCF0 (decompiled C).
//!
//! ```text
//! Begin: the FR shell (link pawn ATresFriendLinkPawnGoofyShoot, SpecificAction(0, 0))
//! Tick:  sub 1: the link pawn holds Sora (it plays GShootStart / Rise / Hit / Finish on him) -> 2
//!        sub 2: released (link pawn Move 5); at his clip's end: detach (vt+0x23B0(0)), pawn vt+0x16D8(0), the fall clip
//!               (vt+0x1C88(.., 0, 2): JumpNormalDownLoop here [D]) -> 3
//!        sub 3: vt+0xDC8; on the ground: "Land" (mode 2) -> 4
//!        sub 4: at its end: the idle clip (vt+0x1C68; the Idle state's own here) -> DoFinish
//! ```

use super::{PlayerState, StateId};
use crate::anim::{AnimId, Notifies, PlayArgs};
use crate::combat::friend::FrLink;
use crate::pawn::Ctx;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FrGoofyShoot {
    pub(super) link: FrLink,
}

impl FrGoofyShoot {
    pub fn new(link: FrLink) -> FrGoofyShoot {
        FrGoofyShoot { link }
    }
}

impl PlayerState for FrGoofyShoot {
    const ID: StateId = StateId::FrGoofyshoot;

    fn sub_step(&self) -> u8 {
        self.link.sub
    }

    fn begin(&mut self, cx: &mut Ctx) {
        self.link.begin(cx, Self::ID);
    }

    fn tick(&mut self, cx: &mut Ctx, _dt: f32) {
        match self.link.sub {
            1 => {
                // The link pawn lets Sora go at its Move step 5 (Goofy's throw): sub 2 (D3).
                if self.link.hold(cx, Self::ID) {
                    self.link.sub = 2;
                }
            }
            2 => {
                // Sub 2 (0x141375B30): still in the air until his clip (GShootFinish) ends, then
                // detach (vt+0x23B0(0)), gravity (vt+0x16D8(0)) and the fall clip.
                if cx.pawn.anim.ended {
                    self.link.detach(cx);
                    if let Some(anim) = AnimId::from_name("JumpNormalDownLoop") {
                        cx.pawn.anim.play_with(anim, PlayArgs::mode(3, -1.0));
                    }
                    self.link.sub = 3;
                }
            }
            3 => {
                if cx.pawn.cmc.on_ground() {
                    self.link.play_end(cx, "Land", -1.0);
                    self.link.sub = 4;
                }
            }
            4 => {
                if cx.pawn.anim.ended {
                    self.link.sub = 5;
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
