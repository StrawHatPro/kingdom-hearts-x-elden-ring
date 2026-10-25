//! `TresPlayerState_StyleChange` (id 31): the transformation into Second Form (combat spec 10.4:
//! Reset 0x1413D0C50, Begin 0x1413CBA70, Tick 0x1413D3AC0, End 0x1413CE760). Requested by
//! `combat::form::start_style` (`TryStyleChange` = `PushState(31)` after `ClearQueue`).
//!
//! The Kingdom Key only (the 0x94 branch): `StyleChange` (1401, 0.8333 s) with the world slowed
//! to 0.1 -> `StyleChangeLoop` (1402) for one tick -> `StyleChangeEnd` (1403, 1.4 s): the wear
//! form is applied when it starts, the 550 cm blast `LM_CHANGE` is its hit window (group
//! `StyleChange` of the pawn's own attack component, 0.3667 - 0.9667 s), and when the state
//! ends the finish command of level 1 appears. No root motion in the three clips.
//!
//! Not here: the short version of the option "Keyblade Cinematics: Basic" (flag bit 4: the save
//! default is Normal), the other keyblades' clips, the HUD banner calls (the UI seam publishes
//! the form: `UiSnapshot::situation.form`), and the invincible bit pawn+0x630 bit 2 that Begin
//! sets and End clears (`m_bIsInvincible`: its test belongs to the callers of pawn vt+0x2060,
//! which spec 9.6 leaves unread: TODO(spec); a hit during the transformation here replaces the
//! state, and `End` then applies the wear form, as the last paragraph of 10.4 describes).

use super::common::on_ground;
use super::{PlayerState, StateId};
use crate::anim::{AnimId, PlayArgs};
use crate::combat::form;
use crate::combat::tables::P;
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct StyleChange {
    pub(super) sub: u8,
    /// +0x94: the form's command.
    form: i32,
    /// +0x98 / +0x99: the wear form to apply (0x0B = none).
    wear_form: u8,
    /// +0xA0 bit 0: the animation is missing.
    missing: bool,
}

impl StyleChange {
    /// Reset (0x1413D0C50).
    pub const RESET: StyleChange = StyleChange { sub: 0, form: 0, wear_form: 0x0B, missing: false };

    fn play(&mut self, s: &mut Pawn, name: &str) {
        match AnimId::from_name(name) {
            // Mode 2 (one-shot), default blend.
            Some(clip) => s.anim.play_with(clip, PlayArgs::mode(2, -1.0)),
            None => {
                self.missing = true;
                s.note("a formchange clip (StyleChange / StyleChangeLoop / StyleChangeEnd) is not in the generated animation table");
            }
        }
    }

    /// Sub 3 (and `End` when the state was left before it): the wear form, the end clip.
    fn transform(&mut self, cx: &mut Ctx) {
        if self.wear_form != 0x0B {
            form::apply_wear_form(cx, self.wear_form);
        }
        self.play(cx.pawn, "StyleChangeEnd");
        self.sub = 4;
        form::world_slow(cx, false);
    }
}

impl PlayerState for StyleChange {
    const ID: StateId = StateId::Stylechange;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// `Begin` (0x1413CBA70).
    fn begin(&mut self, cx: &mut Ctx) {
        cx.pawn.cmc.stop_movement();
        self.form = cx.pawn.combat.form.cmd;
        if form::is_second_form(self.form) {
            // 0x1413CBC54 `mov word [rbx+98h], 105h`: TRES_WEAR_FORM_05.
            self.wear_form = P.form_wear_form as u8;
        }
        cx.pawn.combat.form.bad_status_block = true;
        form::world_slow(cx, true);
    }

    /// `Tick` (0x1413D3AC0).
    fn tick(&mut self, cx: &mut Ctx, _dt: f32) {
        match self.sub {
            0 => {
                // pawn.slot741(0): gravity off: he hangs in place.
                cx.pawn.cmc.gravity_off = true;
                cx.pawn.cmc.stop_movement();
                self.play(cx.pawn, "StyleChange");
                self.sub = 2;
            }
            2 => {
                if cx.pawn.anim.ended || self.missing {
                    self.play(cx.pawn, "StyleChangeLoop");
                    self.sub = 3;
                }
            }
            // The next tick, no condition.
            3 => self.transform(cx),
            _ => {
                let s = &mut *cx.pawn;
                // "if m_bEnableChange: WorldSlow(off); m_bEnableChange = 0": the bit is cleared in
                // the MainProc the notify set it in, so no gate ever sees it.
                if s.queue.flags.enable_change {
                    s.queue.flags.enable_change = false;
                }
                if s.anim.ended || self.missing {
                    // iface.Fall(actor yaw) (slot 172 0x141444C00): JUMP at its falling sub-step;
                    // on the ground it lands at once.
                    let yaw = s.yaw;
                    super::jump::fall(s, yaw);
                    s.queue.do_chain();
                }
            }
        }
    }

    /// `End` (0x1413CE760).
    fn end(&mut self, cx: &mut Ctx) {
        cx.pawn.cmc.gravity_off = false;
        if !on_ground(cx.pawn) {
            cx.pawn.cmc.set_falling();
        }
        // pawn.slot948(): OnPlayerEndCommandAction, pawn+0xE2C = 0.
        cx.pawn.combat.form.command_in_action = 0;
        if matches!(self.sub, 2 | 3) {
            // Left before sub 3 ran (a damage reaction): the wear form is applied all the same.
            self.transform(cx);
        }
        form::world_slow(cx, false);
        // ctrl.slot867(1, 0) -> SetStyleFinishCommand(true, false) (0x1413CEA6D): finish level 1.
        form::set_style_finish_command(cx, true, false);
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}
