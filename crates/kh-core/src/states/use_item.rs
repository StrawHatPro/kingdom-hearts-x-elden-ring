//! `TresPlayerState_UseItem` (id 111) with `CanUseItem` / `TryUseItem` (docs/sora-magic-spec.md
//! part E I2 - I4; part J 7 for the pouch). Clip `ItemUse` / `ItemAirUse` (0.8333 s): the
//! `TresTiming` notify at 0.5 s consumes the pouch slot and queues the effect on the target with
//! a 0.5 s delay; leaving is allowed from the same moment (the handler sets `m_bEnableChange`).
//!
//! LITERAL READING NOT FOLLOWED (reported): `TryUseItem` is given as `PushState(111)`. A state
//! pushed behind Idle or Run never begins (those states do not finish by themselves), and the
//! test list (K T8.1) has the item state begin at the press: the port uses `SetState`, which
//! for the attack row also drops the queued attack as the spec says.

use glam::Vec3;

use super::common::on_ground;
use super::{PlayerState, StateData, StateId};
use crate::anim::{AnimId, NotifyClass, NotifyPhase, Notifies, PlayArgs};
use crate::host::{Attach, SoundId, SoundPlay};
use crate::magic::items;
use crate::magic::tables::Sheet;
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UseItem {
    sub: u8,
    /// +0x90: the command kind (58..=68); +0x88 `m_pTargetPawn` is always Sora in the port.
    pub cmd: u8,
    /// +0x94 bit 0: "was airborne".
    airborne: bool,
    /// The `TresTiming` handler ran.
    pub consumed: bool,
}

impl UseItem {
    pub const RESET: UseItem = UseItem { sub: 0, cmd: 0, airborne: false, consumed: false };
}

/// `CanUseItem(kind)` (slot 227 0x14142EBD0; E I2).
pub(crate) fn can_use_item(state: &StateData, cx: &mut Ctx, cmd: u8, check_stock: bool) -> bool {
    let Some(item) = items::item_of_command(cmd) else { return false };
    if cx.pawn.status.hp < 1 || cx.pawn.magic.command_latch {
        return false;
    }
    // The next queued state must be none or an attack.
    if !matches!(cx.pawn.queue.queued_id(), None | Some(StateId::Attack)) {
        return false;
    }
    if check_stock && items::pouch(cx).count(item) < 1 {
        return false;
    }
    let s = &*cx.pawn;
    match state {
        // IDLE 1, RUN 2, GLIDE 4 (FLOATING 5): always.
        StateData::Idle(_) | StateData::Run(_) | StateData::Glide(_) => true,
        StateData::Jump(j) => !j.flow,
        // GUARD 15: Guard+0xDC != 0 and Guard+0x84 bit 5 (m_bEnableInput of the hit clip).
        StateData::Guard(g) => g.hit_entry && g.enable_input,
        // DODGEROLL 16, AIR_SLIDE 20 (and +0xA8 bit 7 clear), BADSTAT_STUN 122: m_bEnableChange.
        StateData::DodgeRoll(_) | StateData::BadstatStun(_) => s.queue.flags.enable_change,
        StateData::AirSlide(a) => !a.aborted() && s.queue.flags.enable_change,
        // Attack+0x234 bit 7: the cancel window.
        StateData::Attack(a) => a.cancel,
        StateData::UseItem(_) => s.queue.flags.enable_change,
        StateData::FireMagic(m) => m.can_use_item(s),
        // DAMAGE: +0x84 bit 0, +0x90 == 0, sub-step in {1, 2, 9}.
        StateData::Damage(d) => s.queue.flags.enable_change && matches!(d.kind.value(), 1 | 2 | 9),
        #[allow(unreachable_patterns)]
        _ => false,
    }
}

/// `TryUseItem(target, kind, 0)` (slot 228 0x141441FF0).
pub(crate) fn try_use_item(state: &mut StateData, cx: &mut Ctx, cmd: u8) -> bool {
    if !can_use_item(state, cx, cmd, true) {
        return false;
    }
    // A queued attack is dropped (FUN_1405A7FF0); the item state replaces the running one.
    cx.pawn.queue.set_state(UseItem { cmd, ..UseItem::RESET });
    true
}

impl PlayerState for UseItem {
    const ID: StateId = StateId::UseItem;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// `Begin` (0x141509E40).
    fn begin(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        s.cmc.stop_movement();
        if !on_ground(s) {
            s.cmc.gravity_off = true;
            self.airborne = true;
        }
        let name = if self.airborne { AnimId::ItemAirUse } else { AnimId::ItemUse };
        // Play(name, 0, -1.0, 2, 0, 1.0): once, the asset's blend-in.
        s.anim.play_with(name, PlayArgs::mode(2, -1.0));
        // Slot 114: the voice variant of (user, target) of group `sora_item`.
        cx.play_sound(SoundPlay { id: SoundId(u32::MAX), name: "sora_item", at: Attach::Sora { bone: Some("atama"), offset: Vec3::ZERO }, volume: 1.0, pitch: 1.0, looping: false, spatial: true });
        self.sub = 1;
    }

    /// `Tick` (0x141511E10): the clip's end finishes the state.
    fn tick(&mut self, cx: &mut Ctx, _dt: f32) {
        if cx.pawn.anim.ended {
            cx.pawn.queue.do_chain();
        }
    }

    /// `End` (0x14150C2D0): gravity on, gravity scale reset.
    fn end(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        let p = *s.params();
        s.cmc.gravity_off = false;
        s.cmc.gravity_scale = p.gravity_scale;
    }

    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        let timing = notifies.events.iter().any(|e| e.class() == NotifyClass::TresAnimNotifyTiming && e.phase == NotifyPhase::Instant);
        if timing && !self.consumed {
            // `OnTiming` (0x141512C80): UseItem(user, kind, target, consume = 1, delay = 0.5);
            // then m_bEnableChange, gravity on, gravity scale reset.
            self.consumed = true;
            items::use_item(cx, self.cmd, Sheet::Items.num("use.effect_delay"));
            let s = &mut *cx.pawn;
            let p = *s.params();
            s.queue.flags.enable_change = true;
            s.cmc.gravity_off = false;
            s.cmc.gravity_scale = p.gravity_scale;
        }
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}
