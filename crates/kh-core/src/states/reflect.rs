//! `TresPlayerState_Reflect` (id 66): the weapon-recoil state, clips `AttackReflect` /
//! `AttackAirReflect` (docs/sora-actions-spec.md II.1 "Reflect": Reset 0x1413AD220, Begin
//! 0x1413A4FF0, Tick 0x1413AE9A0, End 0x1413AB6F0, all read at instruction level there).
//!
//! WRITTEN BY THE COMBAT IMPLEMENTER because both of combat's recoil paths end here:
//! - an attack of Sora that was repelled (Attack slot 129 0x141331AF0; combat strand A 7.3):
//!   flags bit 0, causer, speed 400 (`recoil.travel`);
//! - a hit on Sora with victim result REFLECT / GUARD_REFLECT (request interface slot 137
//!   0x14143FE00; strand E 8.4): the same entry;
//! - a guarded hit whose row has `m_ReflectLevel` >= 3 (`OnGuardedHit`): flags 9, direction from
//!   the hit.
//!
//! FOR THE ACTIONS IMPLEMENTER (tier 2 of your spec lists Reflect with Risk Dodge): the risk-dodge
//! offer (flags bit 2: `controller.vt+0x1CB0(attacker)`, the 0.6 s window +0x9C, the HUD prompt
//! `FUN_141414170`, the jump -> RISKDODGE exit) is NOT here; `Reflect::risk_window` is the field
//! for it. Take the file over when you get there.

use glam::Vec3;

use super::common::{on_ground, turn};
use super::damage::yaw_of;
use super::{PlayerState, StateId};
use crate::anim::{AnimId, PlayArgs};
use crate::combat::tables::P;
use crate::host::TargetId;
use crate::pawn::Ctx;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Reflect {
    pub(super) sub: u8,
    /// +0x88 bit 0: push-back; bit 1: started airborne; bit 3: entered from a guarded hit.
    push: bool,
    airborne: bool,
    pub from_guard: bool,
    /// +0x8C, +0x98.
    direction: Vec3,
    pub speed: f32,
    /// +0x9C (bit 2 of the flags while > 0): the risk-dodge window. Never opened here.
    pub risk_window: f32,
    /// +0xAC: the hit direction, flattened and normalised.
    hit_dir: Vec3,
    /// +0xB8 `m_pTargetActor` and where it stood at the request.
    pub causer: TargetId,
    causer_position: Vec3,
    /// (ACTIONS IMPLEMENTER) +0x88 bit 2: a risk dodge is offered (the window +0x9C runs).
    risk: bool,
}

impl Reflect {
    /// Reset (0x1413AD220).
    pub const RESET: Reflect = Reflect { sub: 0, push: false, airborne: false, from_guard: false, direction: Vec3::ZERO, speed: 0.0, risk_window: 0.0, hit_dir: Vec3::ZERO, causer: TargetId::NONE, causer_position: Vec3::ZERO, risk: false };

    /// (ACTIONS IMPLEMENTER) Flags bit 2: the gate of `TryRiskDodge` (slot 60 0x14142CD20).
    pub(super) fn risk_offered(&self) -> bool {
        self.risk
    }
}

/// The request: `SetState(66)`; flags |= 1, +0xB8 = causer, +0x98 = 400.0 (0x141331B86 /
/// 0x14143CE75 / slot 137). `hit_dir` is zero for the attack recoil; `from_guard` adds bit 3.
pub(crate) fn request(cx: &mut Ctx, causer: TargetId, causer_position: Vec3, hit_dir: Vec3, from_guard: bool) {
    // (ACTIONS IMPLEMENTER, risk dodge) `OnGuardedHit`'s Reflect entry (0x14143CE75): "if
    // controller.vt+0x1CB0(attackerPawn): st+0x88 |= 4": a risk-dodge offer of this attacker.
    let risk = from_guard && super::risk_dodge::choose_offer(cx.pawn, causer);
    let state = Reflect { push: true, from_guard, risk, speed: P.recoil_travel, hit_dir, causer, causer_position, ..Reflect::RESET };
    cx.pawn.queue.set_state(state);
}

impl PlayerState for Reflect {
    const ID: StateId = StateId::Reflect;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// `Begin` (0x1413A4FF0).
    fn begin(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        s.cmc.stop_movement();
        let clip = if on_ground(s) {
            AnimId::from_name("AttackReflect")
        } else {
            s.cmc.set_falling();
            self.airborne = true;
            AnimId::from_name("AttackAirReflect")
        };
        match clip {
            // Mode 2, default blend, start 0.
            Some(clip) => s.anim.play_with(clip, PlayArgs::mode(2, -1.0)),
            None => s.note("the recoil clip AttackReflect / AttackAirReflect is not in the generated animation table"),
        }
        if self.push {
            let a = s.yaw.to_radians();
            self.direction = if self.hit_dir == Vec3::ZERO { -Vec3::new(a.cos(), a.sin(), 0.0) } else { self.hit_dir };
            if self.direction == Vec3::ZERO {
                let away = Vec3::new(s.cmc.location.x - self.causer_position.x, s.cmc.location.y - self.causer_position.y, 0.0);
                if self.causer != TargetId::NONE && away.length_squared() > 1.0e-8 {
                    self.direction = away.normalize();
                } else {
                    (self.push, self.speed) = (false, 0.0);
                }
            }
        }
        // `FUN_1414145C0(controller, 0.0, null, 0, 0)`: the guard counter window is closed.
        cx.pawn.combat.guard.offer = None;
        // (ACTIONS IMPLEMENTER) 0x1413A56A9: "if flags & 4: FUN_141414170(controller, 0,
        // controller+0x89C > 0); +0x9C = 0.6": the prompt and the window. The critical flag was
        // read before `close_counter_window` cleared it.
        let critical = cx.pawn.actions.just_guard_timer > 0.0;
        crate::actions::close_counter_window(cx.pawn);
        if self.risk {
            crate::actions::risk_prompt(cx.pawn, 0);
            if critical && cx.pawn.status.count(crate::enums::ETresAbilityKind::CriticalCounter) > 0 {
                cx.pawn.actions.critical = true;
            }
            self.risk_window = crate::actions::A.reflect_riskdodge_window;
        }
    }

    /// `Tick` (0x1413AE9A0).
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        let s = &mut *cx.pawn;
        if self.push && self.direction != Vec3::ZERO {
            // Faces against the push.
            turn(s, yaw_of(-self.direction), dt * 30.0);
        }
        if s.anim.ended {
            s.queue.do_chain();
        }
        if self.push {
            self.speed *= 0.95f32.powf(dt * 60.0);
            s.cmc.set_velocity_xy((self.direction * self.speed).truncate());
        }
        // (ACTIONS IMPLEMENTER) 0x1413AEBE1: the risk-dodge window: "+0x9C -= dt; no chosen
        // attacker or type: +0x9C = 0; +0x9C <= 0: FUN_141414170(controller, 1, 0); flags &= ~4".
        if self.risk {
            self.risk_window -= dt;
            if !s.actions.risk_chosen.is_some_and(|(attacker, kind)| attacker != TargetId::NONE && kind != 0) {
                self.risk_window = 0.0;
            }
            if self.risk_window <= 0.0 {
                crate::actions::risk_prompt(s, 1);
                self.risk = false;
            }
        }
    }

    /// `End` (0x1413AB6F0).
    fn end(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        // (ACTIONS IMPLEMENTER) "If flags & 4: clear; FUN_141414170(controller, next id == 17 ? 3 : 1, 0)".
        if std::mem::take(&mut self.risk) {
            let taken = s.queue.queued_id() == Some(StateId::Riskdodge);
            crate::actions::risk_prompt(s, if taken { 3 } else { 1 });
        }
        s.cmc.direct_xy = None;
        if !on_ground(s) {
            s.cmc.set_falling();
        } else if self.airborne {
            // SetMovementMode(3, 1): Falling with the stop; the movement lands him next tick.
            s.cmc.stop_movement();
        }
    }
}

/// For hosts and tests (`Sora::recoil`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReflectView {
    pub speed: f32,
    pub causer: TargetId,
    pub from_guard: bool,
    pub airborne: bool,
}

pub(crate) fn view(state: &super::StateData) -> Option<ReflectView> {
    match state {
        super::StateData::Reflect(r) => Some(ReflectView { speed: r.speed, causer: r.causer, from_guard: r.from_guard, airborne: r.airborne }),
        _ => None,
    }
}
