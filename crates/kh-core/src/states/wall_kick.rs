//! `TresCharState_WallKick` (id 23) and its request `TryWallKick`: docs/sora-actions-spec.md
//! II.4 "Wall Kick" (instruction level; addresses are the exe's). Tier 2, the direct entry only
//! (`bDirect` = 1: every caller passes it): out of an Air Slide or a Superslide whose move was
//! blocked by a wall (`actions::move_blocked`). The wall-run caller comes with the wall run.
//!
//! One tick after Begin he is turned by the wall normal and plays `WallKickF` (facing or stick
//! into the wall within 35 degrees: he turns around) or `WallKickR` / `WallKickL` (turned
//! parallel to the wall: 286 cm along it, 112 cm away, by the clip's root motion), in Flying
//! mode. During the clip the jump button latches a super jump (type 2, from clip time 0.1 s) and
//! the guard button a Superslide (at the clip end).
//!
//! HOST: a kickable wall is any blocking component with `SQEX_bEnableWallKick`, which is ON by
//! default for every mesh (II.4 "Host affordances"): `World` has no surface flags yet, so every
//! wall of the level is kickable. Its steepness is tested here (impact normal pitch within 30
//! degrees).

use glam::Vec3;

use super::{jump, super_slide, PlayerState, StateData, StateId};
use crate::actions::{self, A};
use crate::anim::{AnimId, PlayArgs};
use crate::enums::ETresAbilityKind as Ability;
use crate::pawn::{Ctx, Pawn};
use crate::sora::{wrap_degrees, MovementState};

/// +0x98 wall normal, +0xA4 direct entry, +0xA5 Superslide latch, +0xA6 jump latch, +0xA7 chain
/// latch (set by the attack request), +0xA8 stick latch, +0xAC stick yaw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WallKick {
    pub(super) sub: u8,
    normal: Vec3,
    direct: bool,
    pub(super) slide_latch: bool,
    pub(super) jump_latch: bool,
    /// For the combat side's attack request (slot 45 0x1414385DD): no caller yet.
    #[allow(dead_code)]
    pub(crate) chain_latch: bool,
    stick: bool,
    stick_yaw: f32,
}

impl WallKick {
    /// The state object after `Reset` (0x14150E2F0).
    pub const RESET: WallKick = WallKick { sub: 0, normal: Vec3::ZERO, direct: false, slide_latch: false, jump_latch: false, chain_latch: false, stick: false, stick_yaw: 0.0 };
}

fn yaw_of(v: Vec3) -> f32 {
    v.y.atan2(v.x).to_degrees()
}

/// The gate (slot 81 0x14142F990) and `TryWallKick(actor, component, &normal, bDirect 1, bKeep
/// 0)` (slot 82 0x141442480) with its push (slot 83 0x141445D80).
pub(crate) fn try_wall_kick(state: &mut StateData, cx: &mut Ctx, normal: Vec3) -> bool {
    let s = &*cx.pawn;
    // NextQueued == 0; the FreeFlow bit; HasAbility(WALL_KICK 97) (0x14142FA90).
    if s.queue.queued_id().is_some() || !s.actions.flow_unlocked || !actions::has(s, Ability::WallKick) {
        return false;
    }
    // Current state in {WALLRUN, DODGEROLL, AIR_SLIDE, SUPER_SLIDE, ATHLETICFLOW}.
    // (magic implementer: ATHLETICFLOW is in the gate's mask, but nothing in that state makes the
    // request: actions spec II.5 "Follow-ups out of the state".)
    if !matches!(state, StateData::WallRun(_) | StateData::DodgeRoll(_) | StateData::AirSlide(_) | StateData::SuperSlide(_) | StateData::AthleticFlow(_)) {
        return false;
    }
    probe_and_push(cx, normal, false)
}

/// The wall run's guard-button latch (WallRun tick 0x1413D981D): `TryWallKick(actor, component,
/// &N, 1, 0)` on the accepted probe; the queued kick's Superslide latch (+0xA5) is set: the kick
/// ends in a Superslide.
pub(crate) fn try_wall_kick_latched(cx: &mut Ctx, normal: Vec3) -> bool {
    let s = &*cx.pawn;
    if s.queue.queued_id().is_some() || !s.actions.flow_unlocked || !actions::has(s, Ability::WallKick) {
        return false;
    }
    probe_and_push(cx, normal, true)
}

fn probe_and_push(cx: &mut Ctx, normal: Vec3, slide_latch: bool) -> bool {
    let (w, s) = (cx.world, &mut *cx.pawn);
    // A sphere of radius 5 swept from the actor location + (0, 0, 75) by -105 x normal: the
    // first blocking hit, its `Rot(ImpactNormal).Pitch` within [-30, 30] (the wall's steepness).
    let start = s.cmc.location + Vec3::Z * A.wallkick_probe_height;
    let r = A.wallkick_probe_radius;
    let Some(hit) = w.sweep_capsule(start, start - normal.normalize_or_zero() * A.wallkick_probe_len, r, r) else { return false };
    let pitch = hit.impact_normal.z.clamp(-1.0, 1.0).asin().to_degrees();
    if pitch.abs() > A.wallkick_surface_pitch_max {
        return false;
    }
    // The component must have `SQEX_bEnableWallKick` (on by default for every mesh).
    if w.surface_flags(hit.component) & crate::world::surface::WALL_KICK == 0 {
        return false;
    }
    s.queue.clear();
    // bDirect and stick magnitude > 0.5 (0x141445E4E): +0xAC = stick yaw, +0xA8 = 1.
    let stick = s.stick_magnitude > A.wallkick_stick_min;
    s.queue.push_state(WallKick { normal: hit.impact_normal, direct: true, stick, stick_yaw: s.stick_yaw, slide_latch, ..WallKick::RESET });
    true
}

/// `MoveInput` case 0x141440D03: magnitude > 0.5: +0xAC = yaw, +0xA8 = 1.
pub(crate) fn move_input(state: &mut StateData, yaw: f32, magnitude: f32) {
    if let StateData::WallKick(k) = state {
        if magnitude > A.wallkick_stick_min {
            (k.stick_yaw, k.stick) = (yaw, true);
        }
    }
}

impl PlayerState for WallKick {
    const ID: StateId = StateId::WallKick;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// Begin 0x14150A0B0.
    fn begin(&mut self, cx: &mut Ctx) {
        // pawn vt+0x1E98(1): Superslide out of a flow jump allowed.
        cx.pawn.actions.superslide_ok = true;
        if self.direct {
            cx.pawn.cmc.stop_movement();
            cx.pawn.cmc.set_flying();
            actions::draw(cx, true);
            // Play(WallKickF, -1, mode 2): replaced by the direction clip in this same MainProc call.
            cx.pawn.next_effect_group = Some(0x139); // (vfx implementer 2) the play's effect group (actions spec: 0x139)
            cx.pawn.anim.play_with(AnimId::WallKickF, PlayArgs::mode(2, -1.0));
            self.sub = 2;
        }
    }

    /// Tick 0x141511E90.
    fn tick(&mut self, cx: &mut Ctx, _dt: f32) {
        let s = &mut *cx.pawn;
        if self.sub == 2 {
            // ONE tick, no waiting (0x14151203B): D = the facing, or the stick's direction.
            let n = self.normal;
            let a = (if self.stick { self.stick_yaw } else { s.yaw }).to_radians();
            let d = Vec3::new(a.cos(), a.sin(), 0.0);
            // dir 0 = into the wall within 35 degrees (D . N < -0.819); else by the side.
            let dir = if d.dot(n) >= A.wallkick_front_dot { if d.x * n.y - d.y * n.x >= 0.0 { 2 } else { 1 } } else { 0 };
            let (turn, clip) = match dir {
                1 => (90.0, AnimId::WallKickR),
                2 => (-90.0, AnimId::WallKickL),
                _ => (180.0, AnimId::WallKickF),
            };
            s.yaw = wrap_degrees(yaw_of(n) + turn);
            s.cmc.stop_movement();
            s.cmc.set_flying();
            s.next_effect_group = Some(0x139); // (vfx implementer 2) effect group 0x139 (actions spec, WallKickF / R / L)
            s.anim.play_with(clip, PlayArgs::mode(2, -1.0));
            self.sub = 3;
        } else if self.sub >= 3 {
            if s.queue.nothing_queued() {
                // The jump latch from clip time 0.1 s: RequestJump(actor yaw, type 2, bAir) (slot 170).
                if self.jump_latch && s.anim.time > A.wallkick_jump_min_time {
                    let yaw = s.yaw;
                    jump::push_super_jump(s, yaw, Vec3::ZERO);
                    s.queue.do_chain();
                    return;
                }
            } else if self.chain_latch && s.queue.flags.enable_change {
                s.queue.do_chain();
                return;
            }
            if s.anim.ended {
                if s.queue.nothing_queued() && self.slide_latch {
                    // Push SuperSlide (slot 73, kind 1, no yaw).
                    s.queue.push_state(super_slide::SuperSlide::RESET);
                }
                s.queue.do_chain();
            }
        }
        self.stick = false;
    }

    /// End 0x14150C3E0: SetMovementMode(Falling).
    fn end(&mut self, cx: &mut Ctx) {
        cx.pawn.cmc.set_falling();
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        MovementState::Other
    }
}
