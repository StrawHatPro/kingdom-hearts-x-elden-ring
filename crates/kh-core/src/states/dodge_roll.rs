//! `TresPlayerState_DodgeRoll` (id 16) and its request `TryDodge`: docs/sora-actions-spec.md
//! II.1 "Dodge Roll" (instruction level; addresses are the exe's). Plain Keyblade (weapon kind
//! 1) only: the `MoleDodge` form branch and the other characters' branches are not here.
//!
//! The roll is the root motion of clip 0051 (600 cm in 0.7 s) through the walking move; the
//! body-collision reaction type is 11 (GHOST_INVINCIBLE: hits do not exist) from Begin to the
//! first state tick after `TresEnableChange` (0.4659 s).

use super::common::on_ground;
use super::{PlayerState, StateData, StateId};
use crate::actions::{self, reaction, A};
use crate::anim::{AnimId, PlayArgs};
use crate::enums::ETresAbilityKind as Ability;
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;

/// State +0x88 flags (bit 0 clip time >= 0.4, bit 1 >= 0.1, bit 2 snap to +0x8C at Begin, bit 3
/// reaction type 11 is set), +0x8C the yaw to roll toward.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DodgeRoll {
    sub: u8,
    /// Bit 2: written by the push when the stick magnitude is > 0.5.
    snap: bool,
    yaw: f32,
    /// Bit 3.
    invincible: bool,
    /// Bits 1 and 0: the 0.1 s and 0.4 s marks (no reader inside the state).
    mark_early: bool,
    mark_late: bool,
}

impl DodgeRoll {
    /// The state object after `Reset` (0x14136E4A0).
    pub const RESET: DodgeRoll = DodgeRoll { sub: 0, snap: false, yaw: 0.0, invincible: false, mark_early: false, mark_late: false };
}

/// `CanDodge` (slot 57 0x141427CC0), the rows of the states this port has. `CanAct`, the
/// restriction bits and the controller mode are constant in field play.
fn can_dodge(state: &StateData, s: &Pawn) -> bool {
    // Weapon kind 1, in battle or not: HasAbility(22 DODGE) (0x141427DA5 / 0x141427DD2).
    if !actions::has(s, Ability::Dodge) {
        return false;
    }
    // Next queued: nothing, or an attack state (table 0x141428448).
    if !matches!(s.queue.queued_id(), None | Some(StateId::Attack)) {
        return false;
    }
    match state {
        StateData::Idle(_) | StateData::Run(_) => true,
        // JUMP: the gate has a row (OnGround && Jump+0xF8 == 0) but `TryDodge` refuses the state.
        StateData::Jump(_) => false,
        // Attack states: OnGround && Attack+0x234 bit 7, the cancel bit (0x141428190).
        StateData::Attack(a) => on_ground(s) && a.cancel,
        // GUARD: OnGround && entered by a guarded hit && m_bEnableInput (0x141427FEA).
        StateData::Guard(g) => on_ground(s) && g.hit_entry && g.enable_input,
        // DODGEROLL: only pawn kind 0x11: never for Sora.
        StateData::DodgeRoll(_) => false,
        // DAMAGE: OnGround && Damage+0x90 == 0 && kinds 1, 2, 9 at m_bEnableChange. (Damage+0x90
        // is not a field of the port's Damage state: taken as 0.)
        StateData::Damage(d) => on_ground(s) && matches!(d.kind.value(), 1 | 2 | 9) && s.queue.flags.enable_change,
        // AIR_SLIDE: !(flags & 0x80) && m_bEnableChange.
        StateData::AirSlide(a) => !a.aborted() && s.queue.flags.enable_change,
        // (magic implementer) FIREMAGIC 18: OnGround && m_bEnableChange && !(+0xD8 & 0x50) &&
        // the combo is not over; USE_ITEM 111: m_bEnableChange && OnGround (II.1 table).
        StateData::FireMagic(m) => on_ground(s) && m.can_dodge(s),
        StateData::UseItem(_) => s.queue.flags.enable_change && on_ground(s),
        #[allow(unreachable_patterns)]
        _ => false,
    }
}

/// `TryDodge(yaw, mag, bKeep = 0)` (slot 58 0x14143AAA0) with `Push` (slot 59 0x141444A50).
/// Returns whether the request was taken (the controller then returns: no stick this frame).
pub(crate) fn try_dodge(state: &mut StateData, cx: &mut Ctx, yaw: f32, magnitude: f32) -> bool {
    let s = &mut *cx.pawn;
    if !can_dodge(state, s) {
        return false;
    }
    // A queued attack is dropped; bKeep = 0: ClearQueue (0x14143ABF9 / 0x14143AC06).
    s.queue.clear();
    // PushState(16); mag > 0.5 (comiss mag,[0.5]; jbe skip): flags |= 4, +0x8C = yaw.
    let snap = magnitude > A.dodge_direction_stick_min;
    s.queue.push_state(DodgeRoll { snap, yaw: if snap { yaw } else { 0.0 }, ..DodgeRoll::RESET });
    true
}

impl PlayerState for DodgeRoll {
    const ID: StateId = StateId::Dodgeroll;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// Begin 0x141368880.
    fn begin(&mut self, cx: &mut Ctx) {
        cx.pawn.cmc.stop_movement();
        if cx.pawn.actions.battle {
            actions::draw(cx, true);
        } else {
            actions::sheathe(cx);
        }
        let s = &mut *cx.pawn;
        if self.snap {
            // Turn(rot(0, +0x8C, 0), 1.0): a full snap.
            s.yaw = crate::sora::wrap_degrees(self.yaw);
        }
        actions::set_reaction(s, reaction::GHOST_INVINCIBLE);
        self.invincible = true;
    }

    /// Tick 0x141373EE0.
    fn tick(&mut self, cx: &mut Ctx, _dt: f32) {
        let s = &mut *cx.pawn;
        if self.sub == 0 {
            // "DodgeRollB" when the weapon is out (the same sequence 0051), blend-in -1, mode 2.
            let name = if s.actions.weapon_out { AnimId::DodgeRollB } else { AnimId::DodgeRoll };
            s.anim.play_with(name, PlayArgs::mode(2, -1.0));
            self.sub = 1;
            self.snap = false;
            return;
        }
        if s.anim.ended {
            s.queue.do_chain();
            self.snap = false;
            return;
        }
        let t = s.anim.time;
        // comiss t,[0.1]; jb skip / comiss t,[0.4]; jb skip.
        self.mark_early |= t >= 0.1;
        self.mark_late |= t >= 0.4;
        if !s.queue.flags.enable_change {
            self.snap = false;
            return;
        }
        if self.invincible {
            actions::restore_reaction(s);
            self.invincible = false;
            // (DON-KIT) Donald's Aero aura: the window (Begin -> here) x (1 + evasion): the added time drops hits as
            // reaction type 11 does (states/damage.rs `take_hit`); none without the aura
            if s.aura_evasion > 0.0 {
                s.aura_ghost = s.anim.time.max(0.0) * s.aura_evasion;
            }
        }
        if s.queue.queued_id().is_some() {
            if s.actions.battle {
                s.actions.last_pose = 1;
            }
            s.queue.do_chain();
            return;
        }
        if !on_ground(s) {
            s.queue.do_chain();
        }
        self.snap = false;
    }

    /// End 0x14136B8B0.
    fn end(&mut self, cx: &mut Ctx) {
        actions::restore_reaction(cx.pawn);
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}
