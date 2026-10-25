//! The formchange gauge ("style points") on the controller: combat strand F 10.2
//! (`build/sora_tmp/combat/P2_F_section.md`): `AddStylePoint` (controller slot 848 0x1413E9000),
//! the per-frame `StyleGaugeTick` (slot 829 0x14141C4F0) with the form's clock, and `AwardAtMax`
//! (slot 830 0x1413F0ED0). The form itself (start, finish commands, end) is `form.rs`; the
//! queue the commands go to is `situation.rs`.
//!
//! Not here: the anti form (controller+0x888 == 0x8B), the game flags of the tick whose meaning
//! is open (F open question 3: taken as "formchanges available"), swimming, the periodic add of
//! `AwardAtMax` step 2 (its flag is not identified).
//!
//! FOR THE MAGIC IMPLEMENTER: `roll_grand_magic` below is the hook of `AwardAtMax` for your
//! roll (`RollGrandMagic(bFinisher, bForced)`, controller slot 838 0x1413ED500, magic spec I 7.2):
//! it is called at every full gauge BEFORE the formchange command is offered and gets the
//! per-attribute point arrays (`Style::points` / `last`: controller+0xB98 / +0xBA8). It returns
//! true when the main roll offered a grand magic and neither the random tail (physical points)
//! nor CRITICAL_CONVERTER cleared that (CC, `magic::grand::roll_full`): the formchange command is
//! then not offered.

use super::form;
use super::situation;
use super::tables::{command_row, P};
use crate::enums::{ETresAbilityKind, ETresCommandKind};
use crate::pawn::Ctx;
use crate::states::{StateData, StateId};

/// `CHANGE_L1`: the Kingdom Key's formchange command (Second Form).
pub const CHANGE_L1: i32 = 0x94;
/// `TresCmdData[148].Param0`: the command's lifetime in the queue, seconds.
pub const CHANGE_L1_LIFETIME: f32 = 30.0;

/// controller +0xB40 ... +0xB50, +0xB98, +0xBA8.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Style {
    /// +0xB40: the gauge, 0 ..= `MyStylePointMax` (100).
    pub gauge: f32,
    /// +0xB44: points added since the last gauge tick.
    pub pending: f32,
    /// +0xB48: the chain factor of simultaneous adds (1, 1/2, 1/4 ...).
    pub chain: f32,
    /// +0xB4C / +0xB50: time until the decay starts / until its next step.
    pub dec_start: f32,
    pub dec_interval: f32,
    /// +0xB98 / +0xBA8: points by the command row's `Attr` (0 physical, 1 fire ... 7 cure) and
    /// the highest command kind that scored in it: the inputs of the grand-magic roll.
    pub points: [f32; 8],
    pub last: [i32; 8],
    /// Full gauges consumed so far (for tests and the debug text).
    pub awards: u32,
}

impl Style {
    /// "gauge = 0; decStart = decInterval = 0; points[] = last[] = 0" (several callers).
    pub(crate) fn clear(&mut self) {
        (self.gauge, self.dec_start, self.dec_interval) = (0.0, 0.0, 0.0);
        (self.points, self.last) = ([0.0; 8], [0; 8]);
    }
}

/// `AddStylePoint(amount, type, cmd)`: `ty` 1 = a hit, 0 = "without hit" (a debug property),
/// 3 = a raw add without the rates. `cmd` = the hit's command kind (0 = none -> ATTACK 27).
pub(crate) fn add(cx: &mut Ctx, amount: f32, ty: u8, cmd: i32) {
    // "pawn state id == 138 (RAILSLIDE_MI): return" has no state here.
    let status = &cx.pawn.status;
    let mut amount = amount;
    let mut m = 1.0;
    let mut cmd = cmd;
    if ty != 3 {
        if ty == 0 {
            // `MyStylePointAddWithoutHit` (pawn+0xCDC) is false.
            amount = 0.0;
        } else {
            if cmd == 0 {
                cmd = P.form_gauge_default_cmd as i32;
            }
            // lv = TresCmdData[form_cmd].Param1: 0 outside a form, 1 in Second Form.
            let lv = cx.pawn.combat.form.level;
            if status.game_level.is_critical() {
                amount *= match lv {
                    1 => P.form_gauge_critical_add_rate_lv1,
                    2 => P.form_gauge_critical_add_rate_lv2,
                    _ => P.form_gauge_critical_add_rate_lv0,
                } * P.form_gauge_critical_add_rate;
                if status.abilities.count(ETresAbilityKind::CriticalCharge) > 0 && status.mp_charging {
                    m = P.form_gauge_critical_charge_rate;
                }
            } else {
                amount *= match lv {
                    1 => P.form_gauge_add_rate_lv1,
                    2 => P.form_gauge_add_rate_lv2,
                    _ => P.form_gauge_add_rate_lv0,
                } * P.form_gauge_add_rate;
            }
        }
    }
    // By command (jump table 0x1413E9384): grand-magic casts add nothing.
    if matches!(cmd, 32 | 36 | 40 | 44 | 48 | 110..=124 | 126 | 231) || (cmd == 125 && !status.abilities.has(ETresAbilityKind::PowerCure)) {
        amount = 0.0;
    }
    if status.abilities.has(ETresAbilityKind::FormUp) {
        m += P.form_gauge_form_up_bonus;
    }
    let style = &mut cx.pawn.combat.style;
    if style.pending <= 0.0 {
        style.chain = P.form_gauge_chain_first;
    } else {
        style.chain *= P.form_gauge_chain_decay;
    }
    amount = amount * m * style.chain;
    if amount > 0.0 {
        // (party stream D) `ctrl.slot899(amount)` 0x1413E8D70 (combat spec 8.5.2): the friend gauge.
        cx.pawn.combat.friend.gauge_add(amount);
        let style = &mut cx.pawn.combat.style;
        style.pending += amount;
        style.dec_start = P.form_gauge_dec_start_time;
        style.dec_interval = 0.0;
        if cmd != 0 {
            if let Some(e) = command_row(cmd).map(|r| r.attr).filter(|e| (0..8).contains(e)) {
                style.points[e as usize] += amount;
                style.last[e as usize] = style.last[e as usize].max(cmd);
            }
        }
    }
}

/// A damaging hit of Sora (the damage notify 0x141442DE0: `((float)info.m_FormPoint, 1,
/// info.m_CommandKind)`; strand C 8.5.2). Keyblade swings carry no command kind (-> ATTACK).
pub(crate) fn add_hit(cx: &mut Ctx, form_point: i32) {
    add(cx, form_point as f32, 1, 0);
}

/// `RollGrandMagic(mp_charge_running, 0)` (see the module text): true = no formchange offer.
fn roll_grand_magic(cx: &mut Ctx) -> bool {
    // (magic implementer) 0x1413F0ED0: rolled = vt+0x1A30(alt = pawn.IsMpCharge(), forced = 0);
    // each offer = AddSituationCommand(cmd, time = -1, 0) (magic/grand.rs; magic spec I 7.2).
    // (CC) The return value is the function's own (magic/grand.rs `roll_full`, transcribed in
    // full): "any command of the main pass was given to AddSituationCommand" (its answer is not
    // read; the MAGIC_ROULETTE pre-roll's offer does not count), then cleared by the random tail
    // when physical points were scored, and always by CRITICAL_CONVERTER (0x1413EDFED): a false
    // result offers the formchange below next to the grand magic.
    let alt = cx.pawn.status.mp_charging;
    let rolled = crate::magic::grand::roll_full(cx, alt);
    for &kind in &rolled.commands {
        situation::enqueue(cx, kind as i32, -1.0);
    }
    rolled.result
}

/// The grand-magic bookkeeping is cleared when such a command was used (magic spec J 5.5:
/// controller+0xB40, +0xB4C, arrays +0xB98 / +0xBA8).
#[allow(dead_code)]
pub(crate) fn clear_grand(cx: &mut Ctx) {
    cx.pawn.combat.style.clear();
}

/// `AwardAtMax(dt)`: returns the new "formchange command pending" byte (+0xB5C).
fn award_at_max(cx: &mut Ctx) -> bool {
    let max = if P.form_gauge_max > 0.0 { P.form_gauge_max } else { 100.0 };
    // 1. the pending flag against the HUD: outside a form it stands while the HUD still has 0x94.
    {
        let has = cx.pawn.combat.situation.has(CHANGE_L1);
        let f = &mut cx.pawn.combat.form;
        if !f.finish_pending && f.award_pending && f.level == 0 {
            f.award_pending = has;
        }
    }
    // 2. (the periodic add: not identified.) The command cool-down blocks the rest.
    if cx.pawn.combat.command_cooldown > 0.0 {
        return cx.pawn.combat.form.award_pending;
    }
    // 3. the full gauge.
    if cx.pawn.combat.style.gauge < max {
        return cx.pawn.combat.form.award_pending;
    }
    cx.pawn.combat.style.awards += 1;
    // Only acts while a form is active: the next finish level (F 10.6).
    form::set_style_finish_command(cx, true, false);
    let offered = roll_grand_magic(cx);
    if !offered && !cx.pawn.combat.form.finish_pending {
        let f = cx.pawn.combat.form;
        // No second formchange inside Second Form; weapon type 1 (the Kingdom Key): 0x94.
        if !form::is_second_form(f.cmd) && f.level == 0 {
            situation::enqueue(cx, CHANGE_L1, -1.0);
        }
    }
    cx.pawn.combat.style.clear();
    !cx.pawn.combat.form.finish_pending
}

/// `StyleGaugeTick(dt)`: every controller frame.
pub(crate) fn tick(state: &StateData, cx: &mut Ctx, dt: f32) {
    // "current state is of class StyleChange: return": points wait during the transformation.
    if state.id() == StateId::Stylechange {
        return;
    }
    let max = if P.form_gauge_max > 0.0 { P.form_gauge_max } else { 100.0 };
    {
        let style = &mut cx.pawn.combat.style;
        style.gauge = (style.gauge + style.pending).min(max);
        style.pending = 0.0;
    }
    let pending = award_at_max(cx);
    cx.pawn.combat.form.award_pending = pending;
    if cx.pawn.combat.form.style_fix {
        return;
    }
    cx.pawn.combat.form.time_up = false;
    if !cx.pawn.combat.form.paused && cx.pawn.combat.form.cmd != 0 {
        cx.pawn.combat.form.time_left -= dt;
        if cx.pawn.combat.form.time_left <= 0.0 {
            if form::can_end_style(state, cx) {
                form::end_style(cx, false, false);
            } else {
                // Stays at <= 0 and is retried next frame.
                cx.pawn.combat.form.time_left += dt;
            }
            cx.pawn.combat.form.time_up = true;
        }
    }
    // The decay (also during the form): after `MyStylePointDecStartTime` (20 s) without gain,
    // `MyStylePointDec` (5) every `MyStylePointDecInterval` (1 s).
    let style = &mut cx.pawn.combat.style;
    style.dec_start -= dt;
    if style.dec_start > 0.0 {
        return;
    }
    style.dec_start = 0.0;
    style.dec_interval -= dt;
    if style.dec_interval <= 0.0 {
        style.dec_interval = P.form_gauge_dec_interval;
        style.gauge -= P.form_gauge_dec_amount;
    }
    if style.gauge <= 0.0 {
        style.clear();
    }
}

/// The kind as the enum, for events.
pub(crate) fn kind_of(kind: i32) -> ETresCommandKind {
    ETresCommandKind::from_value(kind).unwrap_or(ETresCommandKind::None)
}

// (CC) AwardAtMax with RollGrandMagic's result: CRITICAL_CONVERTER puts the formchange next to the
// grand magic (0x1413EDFED -> 0x1413F0ED0).
#[cfg(test)]
mod tests {
    use super::*;
    use crate::anim::{AnimId, AnimPlayer};
    use crate::cmc::Cmc;
    use crate::host::NullHost;
    use crate::params::SORA_PARAMS;
    use crate::pawn::Pawn;
    use crate::test_world::TestWorld;
    use glam::Vec3;

    fn full_gauge(converter: bool) -> Pawn {
        let mut p = Pawn::new(SORA_PARAMS, Cmc::new(&SORA_PARAMS, Vec3::ZERO), 0.0, AnimPlayer::new(AnimId::IdleN));
        if converter {
            p.status.abilities.set(ETresAbilityKind::CriticalConverter, 1, 1);
        }
        // A certain S_FIRAGAN roll (100 Fire points), no physical points.
        p.combat.style.gauge = 100.0;
        p.magic.grand.element_points[1] = 100.0;
        let world = TestWorld::floor();
        let mut null = NullHost::default();
        let mut host = null.host(&world);
        let mut cx = Ctx::new(&mut p, &mut host);
        award_at_max(&mut cx);
        p
    }

    #[test]
    fn critical_converter_offers_the_formchange_with_the_grand_magic() {
        let plain = full_gauge(false);
        assert!(plain.combat.situation.has(112) && !plain.combat.situation.has(CHANGE_L1));
        let cc = full_gauge(true);
        assert!(cc.combat.situation.has(112) && cc.combat.situation.has(CHANGE_L1));
        assert_eq!((cc.combat.style.gauge, cc.combat.style.awards), (0.0, 1));
    }
}
