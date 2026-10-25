//! OWNER: party stream D. `TresPlayerState_FRShieldGuardian` (state 30, Trinity Guard, command 193):
//! Reset 0x14136E980 (command 0xC1, sub 0), Begin 0x141369E40, Tick 0x141377AC0, End 0x14136C480.
//!
//! ```text
//! Begin: the FR shell (link pawn ATresFriendLinkPawnShieldGuardian, SpecificAction(0, 0))
//! Tick:  sub 0: the link pawn is ready (vt+0x26D8) -> vt+0x358 -> 1
//!        sub 1: its own command menu: once NotifySetActionCommand(0xD6 = 214, -1) (the situation
//!               slot's cancel entry; Triangle / pad Y); the attack press (input action 1) -> link
//!               vt+0x27D0 Shield Bash, the jump press -> link vt+0x2618 Jump; the stick -> link
//!               vt+0x2600 (move); the menu's cancel (vt+0x2B8) or the time -> 4
//!        sub 2: the finish (link vt+0x27E0) -> 3;  sub 3: until the link's end -> 4
//!        sub 4: detach; "M_1501" (blend -1) -> 5;  sub 5: at its end DoFinish
//! ```
//! Pad: A = Shield Bash, B = Jump, Y = cancel (the 214 entry), left stick = move.

use super::{PlayerState, StateId};
use crate::anim::Notifies;
use crate::combat::friend::FrLink;
use crate::host::targets::FriendLinkRequest;
use crate::pawn::Ctx;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FrShieldGuardian {
    pub(super) link: FrLink,
    /// +0x9C: the cancel entry (214) was put in the situation slot.
    cancel_shown: bool,
}

impl FrShieldGuardian {
    pub fn new(link: FrLink) -> FrShieldGuardian {
        FrShieldGuardian { link, cancel_shown: false }
    }
}

impl PlayerState for FrShieldGuardian {
    const ID: StateId = StateId::FrShieldGuardian;

    fn sub_step(&self) -> u8 {
        self.link.sub
    }

    fn begin(&mut self, cx: &mut Ctx) {
        self.link.begin(cx, Self::ID);
    }

    fn tick(&mut self, cx: &mut Ctx, _dt: f32) {
        match self.link.sub {
            1 => {
                if !self.cancel_shown && self.link.linked {
                    crate::combat::situation::enqueue(cx, 214, -1.0);
                    self.cancel_shown = true;
                }
                let (_, _, _, cancel) = cx.pawn.combat.friend.input;
                if cancel && self.link.linked {
                    crate::combat::friend::request(cx, FriendLinkRequest::Action { kind: self.link.kind, action: 2 });
                }
                if self.link.hold(cx, Self::ID) {
                    self.link.sub = 4;
                }
            }
            4 => {
                self.link.detach(cx);
                self.link.play_end(cx, "M_1501", -1.0);
                self.link.sub = 5;
            }
            5 => {
                if cx.pawn.anim.ended {
                    self.link.sub = 6;
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
