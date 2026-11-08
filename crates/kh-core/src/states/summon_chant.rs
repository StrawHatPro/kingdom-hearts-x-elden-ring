//! (THE USER 2026-11-02: "Use the summon animations for Sora and camera work from KH3 when initiating the summon.") SORA'S
//! SUMMON CHANT: KH3's Link call, clip `M_1571` (p_ex001, 3.3 s: face `LI_1_EL`, the light of every Link's start effect
//! on `vfx_root`, Sora's random call voice `Sora_Link/bt151001x`; the clip KH3 plays when Sora calls a Link, the audio
//! pack's `summon_` voice lines are mapped to it). The camera is KH3's own Link chant camera (`Camera/sora/Skill/link/
//! LINK_START`, started by `BP_TresPlayerStateEventCamera_Attraction_Chant` as "Link_Chant_cam": blend in 0.8 s ease
//! in-out exp 2, attached to the player, priority 200): kh-camera starts it on the `Summon { Chant }` event.
//!
//! PORT STATE (KH3's links run their own `Dm*` states per summon; there is no shared one to port): id
//! `TSID_STATE_PLAY_MOTION` (72, "play a motion"), unused by the port's Sora otherwise.
//! - Begin: movement stops (in the air gravity is off, as the item state does), the clip plays once, `Summon { Chant }`.
//! - Tick: at `summon.chant.spawn_at` (the call light's end in the clip) `Summon { Spawn }`; the clip's end finishes.
//! - End: a chant cut short (a hit) still calls the spirit (the MP was paid at the press); gravity back.

use super::common::on_ground;
use super::{PlayerState, StateId};
use crate::anim::{AnimId, PlayArgs};
use crate::command::summon::SummonPhase;
use crate::host::EventKind;
use crate::magic::tables::Sheet;
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;

/// The clip.
pub const CLIP: &str = "M_1571";

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SummonChant {
    sub: u8,
    pub slot: u8,
    pub time: f32,
    pub called: bool,
    airborne: bool,
    /// The MP cost paid at the press (the refund's base: the events carry it to the host).
    pub cost: i32,
}

impl SummonChant {
    pub const RESET: SummonChant = SummonChant { sub: 0, slot: 0, time: 0.0, called: false, airborne: false, cost: 0 };

    pub fn new(slot: u8, cost: i32) -> SummonChant {
        SummonChant { slot, cost, ..SummonChant::RESET }
    }

    fn call(&mut self, cx: &mut Ctx) {
        if !self.called {
            self.called = true;
            cx.pawn.event(EventKind::Summon { phase: SummonPhase::Spawn, slot: self.slot, mp: self.cost });
        }
    }
}

/// The states the chant may start from (PORT DESIGN after `CanUseItem`, use_item.rs: the item command's set, a quick
/// command from the menu): idle, run, glide, a jump (not an athletic flow), an attack's cancel window, the end of a
/// dodge roll or an item use; never over a queued state other than an attack.
pub(crate) fn can_summon(state: &super::StateData, cx: &Ctx) -> bool {
    use super::StateData;
    let s = &*cx.pawn;
    if s.status.hp < 1 || s.magic.command_latch || !matches!(s.queue.queued_id(), None | Some(StateId::Attack)) {
        return false;
    }
    match state {
        StateData::Idle(_) | StateData::Run(_) | StateData::Glide(_) => true,
        StateData::Jump(j) => !j.flow,
        StateData::Attack(a) => a.cancel,
        StateData::DodgeRoll(_) | StateData::UseItem(_) => s.queue.flags.enable_change,
        _ => false,
    }
}

impl PlayerState for SummonChant {
    const ID: StateId = StateId::PlayMotion;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    fn begin(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        s.cmc.stop_movement();
        if !on_ground(s) {
            s.cmc.gravity_off = true;
            self.airborne = true;
        }
        match AnimId::from_name(CLIP) {
            Some(clip) => s.anim.play_with(clip, PlayArgs::mode(2, -1.0)),
            None => s.note("the summon chant clip M_1571 is not in the generated animation table"),
        }
        s.command.calling = true;
        self.sub = 1;
        cx.pawn.event(EventKind::Summon { phase: SummonPhase::Chant, slot: self.slot, mp: self.cost });
    }

    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        self.time += dt;
        if !self.called && self.time >= Sheet::Mp.num("summon.chant.spawn_at") {
            self.call(cx);
            self.sub = 2;
        }
        if cx.pawn.anim.ended {
            cx.pawn.queue.do_chain();
        }
    }

    fn end(&mut self, cx: &mut Ctx) {
        self.call(cx);
        let s = &mut *cx.pawn;
        let p = *s.params();
        s.cmc.gravity_off = false;
        s.cmc.gravity_scale = p.gravity_scale;
        s.command.calling = false;
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}
