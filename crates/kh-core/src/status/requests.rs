//! Status requests: how a host (or another system that has no `Ctx` at hand) asks for a change
//! of Sora's status. `Sora::status_request` queues one; the next step applies it through the
//! rules of `status/` and raises the HUD popups it causes through the seam, so that they are in
//! the step's event list and `UiSnapshot` like everything else the core tells the host.
//! States inside the core call the `Status` methods directly (`cx.status.use_mp(..)` ...).

use super::{HitOnSora, ItemKind, MpCost, PrizeKind};
use crate::abilities::passive;
use crate::pawn::Ctx;
use crate::ui_model::{PopupDetail, UiPopup};
use glam::Vec3;

/// A change of status asked for from outside the step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StatusRequest {
    /// An enemy was defeated (docs/sora-stats-spec.md 4): `Status::give_exp(base_exp, rate)`.
    /// Popups, in the UI spec's order (section 8): the EXP point at `position` (when given and a
    /// next level existed), the level-up panel, "Next LV".
    GiveExp { base_exp: i32, rate: f32, position: Option<Vec3> },
    /// (party P2) `GiveExp` with another pawn as "player 0" (a host whose controlled pawn is a friend): `AdjustExp`
    /// asks `player` (`Status::adjust_exp_for`); `panel` false: Sora's level-up panel is not shown (CheckLevelUp
    /// 0x140EBE400 shows it only for a pawn that is not hidden, vt+0x1978: Sora out of the party). Same popups else.
    GiveExpAs { base_exp: i32, rate: f32, position: Option<Vec3>, player: super::ExpPlayer, panel: bool },
    /// A hit on Sora: `Status::apply_damage_to_sora(amount, &hit)`; `amount` is what
    /// `status::compute_damage` returned. (Reactions and death are the combat strand's.)
    Damage { amount: f32, hit: HitOnSora },
    /// `Recover(hp, mp, focus, kind)`.
    Recover { hp: i32, mp: i32, focus: i32, kind: u8 },
    FullRecover,
    /// MP of a cast: `Status::spend_command_mp`.
    UseMp { cost: MpCost, combo: i32 },
    /// One item from the pouch on himself.
    UseItem(ItemKind),
    /// An item picked up: into the pouch, with the "item obtained" popup (a stub).
    ObtainItem(ItemKind),
    /// A prize picked up: converted by the prize abilities, then applied; munny shows its popup.
    Prize(PrizeKind),
    /// (prize) An orb a host's prize simulation (crates/kh-prize) brought to him: `ApplyPrize`
    /// 0x1405AB950 as the game calls it when the orb arrives. NOT converted here (the game converts
    /// a prize when it SPAWNS, 0x141472DC0: the orb is already what it is) and silent (the orb's get
    /// sound was played 0.2 s before it arrived, at the orb). `amount` 0 = the kind's own value;
    /// above 0 a munny orb is worth that much instead (a host whose currency is on another scale).
    PrizeCollected { kind: PrizeKind, amount: i32 },
    /// (new game) The popups of a reward that `Sora::grant` already applied: the bonus panel and / or the
    /// item popup. (A host calls `Sora::grant`, not this.)
    RewardShown(super::RewardOutcome),
}

pub(crate) fn apply(cx: &mut Ctx<'_, '_>, request: StatusRequest) {
    match request {
        StatusRequest::GiveExp { .. } | StatusRequest::GiveExpAs { .. } => {
            // (party P2) the award with Sora or the host's player pawn as player 0
            let (award, position, panel) = match request {
                StatusRequest::GiveExpAs { base_exp, rate, position, player, panel } => {
                    let reward = super::Status::enemy_reward(base_exp, rate);
                    let gain = cx.pawn.status.adjust_exp_for(reward, player);
                    (cx.pawn.status.give_exp_gain(reward, gain), position, panel)
                }
                StatusRequest::GiveExp { base_exp, rate, position } => (cx.pawn.status.give_exp(base_exp, rate), position, true),
                _ => unreachable!(),
            };
            if award.added <= 0 {
                return;
            }
            // ORDER (docs/sora-spec-reconciliation-2.md item 13, the stats spec's reading of the
            // call order in 0x140E9E960: the level-up check comes before the EXP point): level-up
            // -> EXP point -> Next LV. (Changed 2026-10-11 by the combat implementer on the
            // coordinator's instruction; it was EXP point first, the UI spec's order.)
            // 1. The level-up panel: one per award, with the final level (0x141589330).
            if let Some(level_up) = award.level_up.filter(|_| panel) {
                cx.popup(UiPopup { detail: PopupDetail::LevelUp(level_up), ..UiPopup::new(UiPopup::KIND_LEVEL_UP, "", level_up.level as f32, None) });
            }
            // 2. The floating EXP point: only with a position and when a next level existed
            //    before the award (0x141575840).
            if let (Some(position), true) = (position, award.show_exp_point) {
                cx.popup(UiPopup::new(UiPopup::KIND_EXP_POINT, "", award.added as f32, Some(position)));
            }
            // 3. Next LV: while a next level exists (0x141589570).
            if let Some(next) = award.next_lv {
                cx.popup(UiPopup::new(UiPopup::KIND_NEXT_LV, "", next as f32, None));
            }
        }
        StatusRequest::Damage { amount, hit } => {
            cx.pawn.status.apply_damage_to_sora(amount, &hit);
        }
        StatusRequest::Recover { hp, mp, focus, kind } => cx.pawn.status.recover(hp, mp, focus, kind),
        StatusRequest::FullRecover => cx.pawn.status.full_recover(),
        StatusRequest::UseMp { cost, combo } => {
            cx.pawn.status.spend_command_mp(cost, combo);
        }
        StatusRequest::UseItem(item) => {
            let _ = cx.pawn.status.use_item(item);
        }
        StatusRequest::ObtainItem(item) => {
            cx.pawn.status.add_item(item, 1);
            cx.popup(UiPopup::new(UiPopup::KIND_ITEM, item.name(), 1.0, None));
        }
        StatusRequest::Prize(kind) => {
            // (audio implementer) The collected orb's own sound (TresPrizeBase.m_SoundGet of its
            // class, before any conversion by an ability): audio spec 5.6 `code.prize`.
            cx.play_sound(crate::host::SoundPlay::at_sora(crate::host::SoundId(u32::MAX), crate::host::audio::prize_get_sound(kind as u8)));
            let kind = passive::convert_prize(&cx.pawn.status, kind);
            let munny = cx.pawn.status.apply_prize(kind);
            if munny > 0 {
                cx.popup(UiPopup::new(UiPopup::KIND_MUNNY, "", munny as f32, None));
            }
        }
        // (new game)
        StatusRequest::RewardShown(outcome) => {
            if outcome.lines[0] != super::BonusLine::None {
                cx.popup(UiPopup { detail: PopupDetail::Bonus(outcome.lines), ..UiPopup::new(UiPopup::KIND_BONUS, "", 0.0, None) });
            }
            if let Some(item) = outcome.item {
                cx.popup(UiPopup::new(UiPopup::KIND_ITEM, item.name(), 1.0, None));
            }
        }
        // (prize)
        StatusRequest::PrizeCollected { kind, amount } => {
            let mut munny = cx.pawn.status.apply_prize(kind);
            // (IMPL_ER20) A HOST'S OWN VALUE OF AN HP ORB (amount > 0; KH3's own +1 / +5 with amount 0): the rest on top.
            if let (Some(own), true) = (match kind { PrizeKind::HpSmall => Some(1), PrizeKind::HpLarge => Some(5), _ => None }, amount > 0) {
                if !cx.pawn.status.placeholder {
                    let hp = cx.pawn.status.hp;
                    cx.pawn.status.set_hp(hp + amount - own);
                }
            }
            if munny > 0 && amount > 0 {
                cx.pawn.status.munny += amount - munny;
                munny = amount;
            }
            if munny > 0 {
                cx.popup(UiPopup::new(UiPopup::KIND_MUNNY, "", munny as f32, None));
            }
        }
    }
}
