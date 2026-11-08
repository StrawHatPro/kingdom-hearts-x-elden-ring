//! (weapons F-LIGHTDARK) `TresPlayerState_ComboTF` (id 161): the attack state of Double Form (0xDF out of Light Form, 0xE0
//! out of Dark Form). The Attack class with ComboTF's Decide (0x141363CE0) and shot handler (0x1413590B0):
//! states/combo_lf.rs (the shared parts) and combat/forms/lightdark.rs.

use super::combo_lf::{self, ComboLf};
use super::style_finish::StyleFinish;
use super::{PlayerState, StateId};
use crate::anim::Notifies;
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;

// ================================================================================================ FINISH_TF / FINISH_TF2

/// FINISH_TF / FINISH_TF2 Begin (STYLEFINISH Begin 0x1413CBF70 binds slot 166 for 0xE1 / 0xE2): StyleFinish's Begin (done by
/// the caller); slot 166's own fields start at 0 (+0x234 bit 2 = `a.air` here, +0x2D8 = `root_len`).
pub(crate) fn finish_begin(f: &mut StyleFinish, _cx: &mut Ctx) {
    f.a.sub = 0;
    f.root_len = 0.0;
    f.a.air = false;
}

/// STYLEFINISH slot 166 0x1413E0860 (Ultimate Finish): true = done (chained on itself).
pub(crate) fn finish_tick(f: &mut StyleFinish, cx: &mut Ctx, dt: f32) -> bool {
    // (AUDIT-C TF) the pawn tick's head (0x141448AA0 -> FUN_143EAC3D0): the point FUN_1414466F0 left at pawn+0x1560 (sub 1:
    // the start point, M_N424's root motion undone as M_N425 begins)
    if let Some(at) = cx.pawn.combat.lightdark.finish_warp.take() {
        cx.pawn.cmc.location = at;
    }
    match f.a.sub {
        0 => {
            // FUN_1405AD850 (on the ground) false: +0x234 |= 4; Flying (pawn vt+0x17A0(5, 1)); m_bFaceAt; +0x2B4 = his
            // location; FUN_1413F4EF0(+0x2B4, 300, 1, 1, pawn) (a spot with 300 cm of room: lightdark::finish_start_point) +
            // FUN_143EAC3D0 (he is put there); Play(M_N424, blend -1, mode 2), voice 0.
            f.a.air = !super::common::on_ground(cx.pawn);
            cx.pawn.cmc.set_flying();
            f.a.face_at = true;
            let to = crate::combat::forms::lightdark::finish_start_point(cx);
            cx.pawn.cmc.location = to;
            cx.pawn.combat.lightdark.finish_start = to; // (AUDIT-C TF) +0x2B4: the start point, kept for sub 1
            f.play(cx.pawn, "M_N424", -1.0, 2);
            cx.set_voice_group_no(0);
            f.a.sub = 1;
        }
        1 => {
            if cx.pawn.anim.ended {
                // (AUDIT-C TF) FUN_1414466F0(pawn, +0x2B4): pawn+0x1560 = the start point, +0x155C = 1; the next pawn tick puts
                // him there (`finish_warp`, above)
                cx.pawn.combat.lightdark.finish_warp = Some(cx.pawn.combat.lightdark.finish_start);
                // Play(M_N425,
                // blend -1, mode 2), voice 0; on the ground: gravity on (vt+0x1728(1)), Walking (vt+0x17A0(1, 1)).
                f.play(cx.pawn, "M_N425", -1.0, 2);
                cx.set_voice_group_no(0);
                if !f.a.air {
                    cx.pawn.cmc.gravity_off = false;
                    cx.pawn.cmc.set_falling();
                }
                f.a.sub = 2;
            }
        }
        2 | 5 => {
            // sub 2: a hit window of M_N425 has begun (+0x164): WorldSlow(ctrl, 0), -> 5, and on into 5's test the same tick.
            if f.a.sub == 2 && f.a.swing_started {
                crate::combat::form::world_slow(cx, false);
                f.a.sub = 5;
            }
            if f.a.sub == 5 && cx.pawn.anim.ended {
                f.root_len = crate::combat::forms::lightdark::finish_hold();
                f.a.sub = 0xC;
            }
        }
        _ => {
            // sub 0xC: +0x2D8 -= dt; at 0: pawn vt+0x1E48(1.0), DoChain.
            f.root_len -= dt;
            if f.root_len <= 0.0 {
                crate::states::damage::set_invincible(cx, 1.0); // (SHOT-C) pawn vt+0x1E48(1.0)
                return f.finish(cx.pawn);
            }
        }
    }
    false
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ComboTf(pub(crate) ComboLf);

impl PlayerState for ComboTf {
    const ID: StateId = StateId::ComboTf;

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
