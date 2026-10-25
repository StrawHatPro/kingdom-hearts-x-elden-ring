//! `TresPlayerState_Swaying` (the side-step around a pawn in the way, id 11) and its request
//! `TrySway`: docs/sora-actions-spec.md II.6 "Swaying" (instruction level). OWNER: the actions
//! implementer.
//!
//! While he runs at full speed (stick above 0.5, not in battle) and bumps into another pawn that
//! is within 50 degrees of his facing, he side-steps round it: `RunSwayR` (the pawn to his left)
//! or `RunSwayL`, 0.9 s, full run speed along the facing plus the clip's lateral root motion (out
//! 109.5 cm and back). With the stick released it ends at `TresEnableChange` (0.39 s); held, it
//! runs to the clip end and on into Run.
//!
//! THE BUMP: the engine event behind the Run state's handler (slot 8 of its second interface,
//! 0x1413B2960) was not traced (II.6 "S1"). Here: the Run tick asks the host's targets for a
//! living pawn whose body (its push body, else its bounds) touches his own body circle of radius
//! 50 (`run_contact`).

use glam::Vec2;

use super::common::{on_ground, turn};
use super::{hopping, jump, run, slope_slide, PlayerState, StateData, StateId};
use crate::actions::probes::dir;
use crate::actions::A;
use crate::anim::{AnimId, PlayArgs};
use crate::host::{EventKind, TargetFlags, TargetId, TargetInfo};
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;

/// +0x88 the other pawn, +0x90 side (1 `RunSwayR`, 2 `RunSwayL`), +0x94 stick this frame, +0x98
/// travel yaw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Swaying {
    /// 0 the clip, 3 over.
    pub(super) sub: u8,
    other: TargetId,
    side: u8,
    stick: bool,
    yaw: f32,
}

impl Swaying {
    /// Reset (0x1413D0EA0).
    pub const RESET: Swaying = Swaying { sub: 0, other: TargetId::NONE, side: 0, stick: false, yaw: 0.0 };
}

/// The gate (slot 177 0x14142E570): nothing queued, not in battle, the other pawn within 50
/// degrees of the facing (dot >= 0.643); the side by the cross product.
fn gate(s: &Pawn, other: &TargetInfo) -> Option<u8> {
    if s.queue.queued_id().is_some() || s.actions.battle {
        return None;
    }
    let d = (other.position - s.cmc.location).truncate().normalize_or_zero();
    let f = dir(s.yaw);
    if f.x * d.x + f.y * d.y < A.sway_cone_dot {
        return None;
    }
    Some(if f.x * d.y - f.y * d.x >= 0.0 { 2 } else { 1 })
}

/// `TrySway(yaw, other)` (slot 178 0x141441B20): the gate; `PushState(11)`. The caller chains.
pub(crate) fn try_sway(s: &mut Pawn, yaw: f32, other: &TargetInfo) -> bool {
    let Some(side) = gate(s, other) else { return false };
    s.queue.push_state(Swaying { other: other.id, side, stick: true, yaw, ..Swaying::RESET });
    true
}

/// The Run state's bump handler (0x1413B2960) with this port's contact test (see the file
/// header): horizontal speed >= 0.9 x MaxWalkSpeed, stick magnitude > 0.5, then `TrySway(Run yaw,
/// other)`. The caller has checked: Run sub-step 0, not walking.
pub(crate) fn run_contact(cx: &mut Ctx, run_yaw: f32) -> bool {
    let s = &*cx.pawn;
    if s.actions.traversal.sway_cooldown > 0.0 || s.actions.battle || s.stick_magnitude <= A.sway_min_stick || s.cmc.velocity.truncate().length() < A.sway_min_speed_ratio * s.cmc.max_walk_speed {
        return false;
    }
    let mut targets: Vec<TargetInfo> = Vec::new();
    cx.targets(&mut targets);
    let own = s.cmc.location;
    for t in &targets {
        // (er-host, IMPL_ER32) a gimmick (a breakable prop of the host's) is not a body he sways around
        if !t.flags.has(TargetFlags::ALIVE) || t.flags.has(TargetFlags::GIMMICK) {
            continue;
        }
        let radius = cx.host.targets.push_body(t.id).map_or(t.radius, |b| b.radius);
        let flat = (t.position - own).truncate();
        // His own body circle is 50 cm; the vertical ranges must overlap.
        if flat.length() > radius + 50.0 + 2.0 || (t.center.z - (own.z + 90.0)).abs() > t.half_height + 90.0 {
            continue;
        }
        if try_sway(cx.pawn, run_yaw, t) {
            return true;
        }
    }
    false
}

/// `MoveInput`: +0x98 = stick yaw, +0x94 = 1 for any active stick.
pub(crate) fn move_input(state: &mut StateData, yaw: f32) {
    if let StateData::Swaying(w) = state {
        (w.yaw, w.stick) = (yaw, true);
    }
}

impl PlayerState for Swaying {
    const ID: StateId = StateId::Swaying;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// Begin 0x1413CC7B0.
    fn begin(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        s.anim.play_with(if self.side == 1 { AnimId::RunSwayR } else { AnimId::RunSwayL }, PlayArgs::mode(2, A.sway_blend_in));
        s.actions.traversal.begun[5] += 1;
        s.event(EventKind::Traversal { kind: "sway", id: self.other.0, on: true });
    }

    /// Tick 0x1413D8110.
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        if !on_ground(cx.pawn) {
            jump::fall(cx.pawn, self.yaw);
            return;
        }
        let me = StateData::Swaying(*self);
        if self.sub == 0 {
            if slope_slide::try_slope_slide(&me, cx, false) {
                cx.pawn.queue.do_chain();
                return;
            }
            if hopping::try_hopping(&me, cx, self.yaw, cx.pawn.anim.reverse_leg) {
                cx.pawn.queue.do_chain();
                return;
            }
        }
        if cx.pawn.anim.ended || (!self.stick && cx.pawn.queue.flags.enable_change) {
            // The clip end (stick held) or TresEnableChange (stick released): on into Run.
            self.sub = 3;
            if self.stick || !cx.pawn.anim.ended {
                run::try_run(&mut StateData::Swaying(*self), cx, self.yaw, 1.0);
            }
            cx.pawn.queue.do_chain();
            return;
        }
        let s = &mut *cx.pawn;
        if self.stick {
            turn(s, self.yaw, dt * A.sway_turn_rate);
        }
        // Velocity.XY = GetMaxWalkSpeed() x the actor's forward.
        let f = dir(s.yaw);
        s.cmc.set_velocity_xy(Vec2::new(f.x, f.y) * s.cmc.max_walk_speed);
        self.stick = false;
    }

    /// End 0x1413CF3B0.
    fn end(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        s.cmc.direct_xy = None;
        s.cmc.walk_mode = false;
        s.actions.traversal.sway_cooldown = 0.5;
        s.event(EventKind::Traversal { kind: "sway", id: self.other.0, on: false });
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}
