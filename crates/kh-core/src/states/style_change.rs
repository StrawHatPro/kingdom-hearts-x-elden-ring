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
//! (weapons P3) EVERY OTHER FORM from its row (sheets/port/sora-keyblade-forms.json; Begin / Tick read in decompiled C):
//! - Light / Dark / Double Form (0xDD .. 0xE0, `wear_at` style_change): the same three clips, their wear form (8 / 9 /
//!   10) at sub 3 with the weapon variant;
//! - the others (`wear_at` weapon_change): sub 0 plays the row's `transform_clip` (`StyleChange2` .. `StyleChange21`,
//!   Rage Form `StyleChange13`), then at once (sub 1 -> 5) the weapon variant with its wear form; sub 5 is sub 4: the
//!   world slow ends at `TresEnableChange`, the state at the clip's end. A clip the animation table lacks ends it on the
//!   next tick (as a missing clip does for Second Form). Not here for them: the 0.2 s turn toward the target of the tail
//!   (facing timer +0x9C, rate x 1.0) and Begin's start section (+0xA4 = 2 for Counter Shield).
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
        // (weapons P3) through the held form's replacement anim set (combat/forms `replace_clip`).
        match AnimId::from_name(crate::combat::forms::clip(self.form, name)) {
            // Mode 2 (one-shot), default blend.
            Some(clip) => s.anim.play_with(clip, PlayArgs::mode(2, -1.0)),
            None => {
                self.missing = true;
                s.note("a formchange clip (StyleChange / StyleChangeLoop / StyleChangeEnd) is not in the generated animation table");
            }
        }
    }

    /// (weapons P3) The form takes the Second Form family's three clips (flags 0x4A: 0x94 .. 0x98, 0xDD .. 0xE0).
    fn three_clips(&self) -> bool {
        form::is_second_form(self.form) || form::row_of(self.form).is_some_and(|r| r.wear_at == "style_change")
    }

    /// Sub 3 (and `End` when the state was left before it): the wear form, the end clip.
    fn transform(&mut self, cx: &mut Ctx) {
        if self.wear_form != 0x0B {
            form::apply_wear_form(cx, self.wear_form);
        }
        // (AUDIT-B W-CORE) Tick sub 3 / End sub 2..3: "if !(+0xA0 & 2) && (+0xA0 & 0x50): pawn vt+0x13E0(form, +0x99, 0, 1.0)": the
        // Light / Dark / Double Form units (not the Second Form family's: +0xA0 bit 1).
        if !form::is_second_form(self.form) {
            cx.pawn.combat.form.equip = form::FormEquip::Weapons;
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
        } else if let Some(row) = form::row_of(self.form).filter(|r| r.wear_at == "style_change") {
            // (weapons P3) 0xDD .. 0xE0: +0x98 = 8 / 9 / 10 (Light, Dark, Double Form).
            self.wear_form = row.wear_form as u8;
        }
        cx.pawn.combat.form.bad_status_block = true;
        // (AUDIT-B W-CORE) StartStyle put the form's Keyblade in his hand (its base form); nothing of the form until the Tick's
        // equip (form.rs `FormEquip`).
        cx.pawn.combat.form.equip = form::FormEquip::Keyblade;
        // (AUDIT-A SECOND) pawn vt+0x2068(1) (0x1413CBA70, slot 1037 = 0x1405B9120): pawn+0x630 bit 2 = m_bIsInvincible,
        // which IsInvincible vt+0x2060 (0x141435A90 -> 0x1405AD120) reads: invincible through the transformation (End
        // clears it). The port's bit is `Status::damage_invalid` (states/damage.rs `take_hit`).
        cx.pawn.status.damage_invalid = true;
        form::world_slow(cx, true);
    }

    /// `Tick` (0x1413D3AC0).
    fn tick(&mut self, cx: &mut Ctx, _dt: f32) {
        match self.sub {
            0 => {
                // pawn.slot741(0): gravity off: he hangs in place.
                cx.pawn.cmc.gravity_off = true;
                cx.pawn.cmc.stop_movement();
                if self.three_clips() {
                    self.play(cx.pawn, "StyleChange");
                    self.sub = 2;
                } else {
                    // (weapons P3) the other forms: their own clip, then at once the weapon variant (pawn.vt+0x13E0) with
                    // the wear form the variant names (m_OwnerWearForm: the row's `wear_form`).
                    let clip = form::row_of(self.form).map_or("StyleChange", |r| r.transform_clip);
                    self.play(cx.pawn, clip);
                    // (AUDIT-A SHIELD) Begin's start section (0x1413CBA70 case 0x89: +0xA4 = 2), read here: Tick sub 0
                    // (0x1413D3AC0) plays the clip (group 0) and, when it started, SetVoiceGroupNo(+0xA4) (anim interface
                    // vt+0x1F0). StyleChange11's voice notify is the group sora_atk4, whose unit 2 is sora_atk_ran6.
                    if let Some(n) = crate::combat::forms::shield::style_change_voice_group(self.form).filter(|_| !self.missing) {
                        cx.set_voice_group_no(n);
                    }
                    if let Some(row) = form::row_of(self.form) {
                        form::apply_wear_form(cx, row.wear_form as u8);
                    }
                    // (AUDIT-B W-CORE) LAB_1413D417B: pawn vt+0x13E0(vt+0x388(cmd), +0x99, bTransform = 1, 1.0): the transform
                    // units alone (w_so051 carries Hyper Hammer's head in itself: its own 3401 grows bone w_so052 at 2.6 - 2.8 s).
                    cx.pawn.combat.form.equip = form::FormEquip::Transform;
                    self.sub = 5;
                }
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
                // "if m_bEnableChange: WorldSlow(off); m_bEnableChange = 0": the bit is cleared in
                // the MainProc the notify set it in, so no gate ever sees it. (weapons P3) The world slow
                // ends here for the forms of sub 5 (Second Form's ended at sub 3: no change).
                if cx.pawn.queue.flags.enable_change {
                    form::world_slow(cx, false);
                    cx.pawn.queue.flags.enable_change = false;
                }
                let s = &mut *cx.pawn;
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
        // (AUDIT-A SECOND) pawn vt+0x2068(0) (0x1413CE760): the transformation's invincibility ends.
        cx.pawn.status.damage_invalid = false;
        if !on_ground(cx.pawn) {
            cx.pawn.cmc.set_falling();
        }
        // pawn.slot948(): OnPlayerEndCommandAction, pawn+0xE2C = 0.
        cx.pawn.combat.form.command_in_action = 0;
        if matches!(self.sub, 2 | 3) {
            // Left before sub 3 ran (a damage reaction): the wear form is applied all the same.
            self.transform(cx);
        }
        // (AUDIT-B W-CORE) End's pawn vt+0x1408 (equipment slot 164 0x14177ED40): the transform units give way to the form's own.
        cx.pawn.combat.form.equip = form::FormEquip::Weapons;
        form::world_slow(cx, false);
        // ctrl.slot867(1, 0) -> SetStyleFinishCommand(true, false) (0x1413CEA6D): finish level 1.
        form::set_style_finish_command(cx, true, false);
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}
