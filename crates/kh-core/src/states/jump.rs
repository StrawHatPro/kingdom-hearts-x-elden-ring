//! `TresPlayerState_Jump` (jump, fall, landing) and its requests: `JumpRequest`, `JumpHeld`,
//! `MoveInput`, `Fall` (docs/sora-movement-spec.md revision 2, sections 2.5, 4; change list D).
//! Addresses are the exe's. Behaviour of the type-0 jump with the weapon away is pinned bit for
//! bit by tests/golden.rs.
//!
//! Beyond stage 1 (docs/sora-actions-spec.md II.3, the actions implementer): the jump types
//! (0 normal 1480, 1 High Jump 1950, 2 super jump 2500, 3 Doubleflight 2300) with the type and
//! power resolver, the second-jump rule (pawn bit 0, Jump flag 8 = "Velocity.Z <= 500"), the
//! High Jump's own height cut in sub-step 1, the landing variants (`RollLand`, `B` names by
//! weapon out), the land-pose landing of a fall begun on a floor, the keep-clip fall out of an
//! Air Slide (flag 0x40), the requests out of Guard / DodgeRoll / AirSlide / Damage (Aerial
//! Recovery). Not here: the rail / ledge / hop / floating / water exits (tier 2 and 3), the
//! fall slide (sub-steps 5 / 6: its flag has no known writer) and the dive fall (7 - 9: off by
//! default).

use glam::Vec3;

use super::common::{ground_probe, on_ground, set_movement_input, turn};
use super::{air_slide, dangle, floating, guard, hopping, rail_slide, slope_slide, wall_run, PlayerState, StateData, StateId};
use crate::actions::{self, land_pose, A};
use crate::anim::{AnimId, Notifies, PlayArgs};
use crate::enums::ETresAbilityKind as Ability;
use crate::pawn::{Ctx, Pawn};
use crate::sora::MovementState;

/// `TresPlayerState_Jump` (4.2): +0x88 request, +0x8C yaw, +0x90 mag, +0x94 held, +0xA4 power,
/// +0xAC fall time, +0xB0 fall height, +0xB4 last Z, +0xDC probe distance, +0xE0 probe ground Z /
/// last actor Z, +0xE8 deferred down loop, +0x114 flags (0x20 step-down, 0x1000 probe cached),
/// +0x120 step-down timer. Actions spec II.3: +0x9C no further air jump, +0xA0 type, +0xF4 start
/// Z, +0xF8 flow jump, +0xFC bit 0 risen, +0x100 / +0x10C / +0x110 inertial push, flags 8 second
/// jump enabled, 0x10 light landing, 0x40 keep clip, 0x2000 `B` names.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Jump {
    /// 0 start clip, 1 after launch, 2 rise loop, 3 fall entry, 4 falling, 10 / 11 landing.
    pub(super) sub: u8,
    request: bool,
    pub(super) yaw: f32,
    magnitude: f32,
    held: bool,
    power: f32,
    fall_time: f32,
    fall_height: f32,
    last_z: f32,
    probe_distance: f32,
    ground_z: f32,
    deferred_down_loop: bool,
    pub(super) step_down: bool,
    probe_cached: bool,
    step_down_timer: f32,
    /// +0xA0: 0 normal, 1 high, 2 super, 3 double flight.
    kind: u8,
    /// +0x9C: `bAir` of the request; set on every landing and by a fall Begin on a floor.
    no_air_jump: bool,
    /// +0xF8: a flow jump (type 2) is running.
    pub(super) flow: bool,
    /// Flag 8: the second jump is enabled (slot 113).
    second_enabled: bool,
    /// Flag 0x40: the clip of the state before (an Air Slide) plays to its end first.
    keep_clip: bool,
    /// Flag 0x2000: the weapon was out at Begin: the `B` jump names.
    b_names: bool,
    start_z: f32,
    risen: bool,
    /// +0x100 direction, +0x10C speed, +0x110 brake per 60 Hz tick of the super jump's push.
    push_dir: Vec3,
    push_speed: f32,
    push_brake: f32,
}

impl Jump {
    /// The state object after `Reset` (0x14138F740) and `RequestJump(yaw, type, bFall)`
    /// (0x141444FA0): `sub` 0 for a jump, 3 for a fall.
    fn requested(yaw: f32, power: f32, sub: u8) -> Jump {
        Jump {
            sub,
            request: false,
            yaw,
            magnitude: 0.0,
            held: false,
            power,
            fall_time: 0.0,
            fall_height: 0.0,
            last_z: 0.0,
            probe_distance: 0.0,
            ground_z: 0.0,
            deferred_down_loop: false,
            step_down: false,
            probe_cached: false,
            step_down_timer: 0.0,
            kind: 0,
            no_air_jump: false,
            flow: false,
            second_enabled: false,
            keep_clip: false,
            b_names: false,
            start_z: 0.0,
            risen: false,
            push_dir: Vec3::ZERO,
            push_speed: 0.0,
            push_brake: A.jump_momentum_keep,
        }
    }

    /// Slot 113 (0x1413837F0): flag 8 = "Jump+0x9C == 0 && Velocity.Z <= 500" (the literal 500;
    /// `MyJump2ndEnableMinimumVelocityZ` has no reader).
    /// The Jump state's world-hit handler (0x14139A730), the part before `TryWallRun`:
    /// `Velocity.Z < 0` at the hit; sub-step 3, 4 (7, 8), or sub-step 1 of a flow jump with the
    /// clip at 0.2 s or later; the stick request of this frame (Jump+0x88).
    pub(crate) fn wall_run_ok(&self, s: &Pawn, velocity_z: f32) -> bool {
        velocity_z < 0.0 && self.request && (matches!(self.sub, 3 | 4) || (self.sub == 1 && self.flow && s.anim.time >= 0.2))
    }

    fn update_second_enabled(&mut self, s: &Pawn) {
        self.second_enabled = !self.no_air_jump && s.cmc.velocity.z <= A.jump_second_enable_max_velocity_z;
    }
}

/// The type and power resolver (pawn slot 1347 0x141434010), II.3: a type whose ability is
/// missing falls back (2 -> 3 with DOUBLEFLIGHT, else 0); with HIGHJUMP every type 0 becomes 1.
/// Powers: `MyJumpPowerNormal` 1480, `MyJumpPowerHighLv3` 1950 (no ability level selects the
/// power), super 2500, double 2300.
fn resolve(s: &Pawn, requested: u8) -> (u8, f32) {
    let has = |kind| actions::has(s, kind);
    let mut kind = requested;
    let bad = (kind == 2 && !has(Ability::Superjump)) || (kind == 3 && !has(Ability::Doubleflight)) || (kind == 1 && !has(Ability::Highjump));
    if bad {
        kind = if kind == 2 && has(Ability::Doubleflight) { 3 } else { 0 };
    }
    if kind == 0 && has(Ability::Highjump) {
        kind = 1;
    }
    let power = match kind {
        1 => A.jump_power_high,
        2 => A.jump_power_super,
        3 => A.jump_power_double,
        _ => s.params().jump_power_normal,
    };
    (kind, power)
}

/// The jump name selector (pawn slot 913 0x141434150) `(type, phase, bB)`: phase 0 start, 1
/// rise, 2 fall. Type 2 has no rise loop of its own (and never asks for one).
fn jump_name(kind: u8, phase: u8, b: bool) -> AnimId {
    match (kind, phase, b) {
        (1, 0, false) => AnimId::JumpHighStart,
        (1, 0, true) => AnimId::JumpHighStartB,
        (1 | 2, 1, false) => AnimId::JumpHighRiseLoop,
        (1 | 2, 1, true) => AnimId::JumpHighRiseLoopB,
        (2, 0, _) => AnimId::JumpSuperStart,
        (3, 0, false) => AnimId::JumpDoubleFlightStart,
        (3, 0, true) => AnimId::JumpDoubleFlightStartB,
        (3, 1, false) => AnimId::JumpDoubleFlightRiseLoop,
        (3, 1, true) => AnimId::JumpDoubleFlightRiseLoopB,
        (_, 0, false) => AnimId::JumpNormalStart,
        (_, 0, true) => AnimId::JumpNormalStartB,
        (_, 1, false) => AnimId::JumpNormalRiseLoop,
        (_, 1, true) => AnimId::JumpNormalRiseLoopB,
        (_, _, false) => AnimId::JumpNormalDownLoop,
        (_, _, true) => AnimId::JumpNormalDownLoopB,
    }
}

/// Plays a jump clip: the stage-1 names with their stage-1 play call (golden), the `B` names
/// with the same arguments as their stage-1 twin, every other name with `mode`.
fn play_jump(s: &mut Pawn, name: AnimId, mode: u8) {
    let twin = match name {
        AnimId::JumpNormalStart | AnimId::JumpNormalStartB => Some(AnimId::JumpNormalStart),
        AnimId::JumpNormalRiseLoop | AnimId::JumpNormalRiseLoopB => Some(AnimId::JumpNormalRiseLoop),
        AnimId::JumpNormalDownLoop | AnimId::JumpNormalDownLoopB => Some(AnimId::JumpNormalDownLoop),
        _ => None,
    };
    match twin {
        Some(twin) => actions::play_as(s, name, twin),
        None => s.anim.play_with(name, PlayArgs::mode(mode, -1.0)),
    }
}

/// `JumpRequest(0)` (slot 42 0x14143D8F0) behind its gate (slot 41 0x14142A3D0), spec 2.5:
/// IDLE / RUN on the ground; JUMP: nothing queued and sub == 11 (no `m_bEnableChange` test,
/// change D7), or the second jump (actions spec II.3: pawn bit 0, DOUBLEFLIGHT, Jump flag 8).
/// A state without a row here refuses.
pub(crate) fn jump_request(state: &mut StateData, cx: &mut Ctx) {
    // DAMAGE (combat strand E 9.5, `JumpRequest` 0x14143E022; actions spec II.2 "Aerial
    // Recovery"): in the recoverable window (0.2 s after the hit) of a blow / toss / launch /
    // parabola, with AIR_RECOVERY, the press is Aerial Recovery: SetState(AIR_SLIDE) with flag
    // 0x20 and the attacker as its target.
    if let StateData::Damage(d) = state {
        if d.recoverable && matches!(d.kind.value(), 2 | 7 | 8 | 10) && actions::has(cx.pawn, Ability::AirRecovery) {
            let target = actions::attacker_position(cx);
            cx.pawn.queue.set_state(air_slide::AirSlide::recovery(target));
            return;
        }
    }
    let s = &mut *cx.pawn;
    // The flow states (actions spec II.2 / II.4): pawn+0xE48 bit 3 && SUPERJUMP (18).
    let flow_jump = s.actions.flow_jump && actions::has(s, Ability::Superjump);
    match state {
        // WALL_KICK (gate 0x14142A757: sub-step > 2; `JumpRequest` 0x14143DF4F): the jump latch;
        // the kick's tick starts the super jump from clip time 0.1 s.
        StateData::WallKick(k) => {
            if flow_jump && k.sub > 2 {
                k.jump_latch = true;
            }
            return;
        }
        // SUPER_SLIDE (gate 0x14142A7EA): type 2 with the facing as the launch direction.
        StateData::SuperSlide(_) => {
            if flow_jump {
                let a = s.yaw.to_radians();
                let yaw = s.yaw;
                s.queue.clear();
                push_super_jump(s, yaw, Vec3::new(a.cos(), a.sin(), 0.0));
            }
            return;
        }
        _ => {}
    }
    // The traversal states of tier 2 (actions spec II.4 - II.6), each by its own gate row.
    match state {
        StateData::WallRun(r) => return wall_run::jump_request(r, s),
        // POLE_TURN (gate 0x14142A808: sub-step > 1 && pawn+0xE48 bit 3): the jump latch (slot 113).
        StateData::PoleTurn(p) => {
            if p.sub > 1 && s.actions.flow_jump {
                p.jump_latch = true;
            }
            return;
        }
        StateData::RailSlide(r) => return rail_slide::jump_request(r, s),
        StateData::Dangle(d) => return dangle::jump_request(d, s),
        _ => {}
    }
    let air_jump = |s: &Pawn| s.actions.second_jump && actions::has(s, Ability::Doubleflight);
    // (yaw, requested type, bAir, queued behind the running state)
    let (yaw, kind, air, behind) = match state {
        // DAMAGE (jump gate 0x14142AA57): kinds 1, 2, 9 at m_bEnableChange.
        StateData::Damage(d) if s.queue.nothing_queued() && matches!(d.kind.value(), 1 | 2 | 9) && s.queue.flags.enable_change => (s.yaw, 0, false, false),
        StateData::Idle(_) if on_ground(s) => (s.yaw, 0, false, false),
        StateData::Run(r) if on_ground(s) => (r.yaw, 0, false, false),
        StateData::Jump(j) if s.queue.nothing_queued() && j.sub == 11 => (j.yaw, 0, false, false),
        // The second jump (gate 0x14142A562 - 0x14142A589): pawn+0xE48 bit 0, DOUBLEFLIGHT, and
        // Jump flag 8 SET. `JumpRequest`: yaw = Jump+0x8C, type 3, bAir.
        StateData::Jump(j) if s.queue.nothing_queued() && air_jump(s) && j.second_enabled => (j.yaw, 3, true, false),
        // GUARD (gate case 15, read in the decompiled C): nothing queued, entered by a guarded
        // hit, m_bEnableInput; airborne: the air-jump rule. `JumpRequest` cases 15 / 16: on the
        // ground type 0, else type 3 with bAir.
        StateData::Guard(g) if s.queue.nothing_queued() && g.hit_entry && g.enable_input && (on_ground(s) || air_jump(s)) => (s.yaw, if on_ground(s) { 0 } else { 3 }, !on_ground(s), guard::JUMP_OUT_OF_GUARD_AT_CLIP_END),
        // DODGEROLL (gate case 16): nothing queued, m_bEnableChange.
        StateData::DodgeRoll(_) if s.queue.nothing_queued() && s.queue.flags.enable_change && (on_ground(s) || air_jump(s)) => (s.yaw, if on_ground(s) { 0 } else { 3 }, !on_ground(s), false),
        // AIR_SLIDE (gate 0x14142A595): bit 0 && DOUBLEFLIGHT && m_bEnableChange; yaw = AirSlide+0x8C, type 3.
        StateData::AirSlide(a) if s.queue.nothing_queued() && air_jump(s) && s.queue.flags.enable_change => (a.yaw, 3, true, false),
        // FLOATING (gate table 0x14142AF78: passes unconditionally; `JumpRequest`: type 3).
        StateData::Floating(_) if s.queue.nothing_queued() => (s.yaw, 3, true, false),
        // SLOPESLIDE (on the ground): yaw = the slide's target yaw; HOPPING (sub-steps 2, 3): its yaw.
        StateData::SlopeSlide(d) if slope_slide::jump_yaw(d, s).is_some() => (d.yaw, 0, false, false),
        StateData::Hopping(h) if hopping::jump_yaw(h).is_some() => (h.yaw, 0, false, false),
        // (GLIDE has no row: a new press needs a release first, and the release ended the glide.)
        _ => return,
    };
    let (kind, power) = resolve(s, kind);
    // ClearQueue; RequestJump(yaw, type, bFall = false).
    if behind {
        s.queue.drop_queued();
    } else {
        s.queue.clear();
    }
    s.queue.push_state(Jump { kind, no_air_jump: air, ..Jump::requested(yaw, power, 0) });
}

/// The super jump out of a flow move (`JumpRequest` from SUPER_SLIDE and the rail fallback):
/// type 2 (2500), bAir, the inertial push along `direction` (pawn+0x644 = 175 cm/s per tick,
/// x 0.92 per tick, cancelled by any stick input; none for a zero direction: the wall kick's
/// `RequestJump`, slot 170). `PushState` only: the caller clears the queue or chains.
pub(crate) fn push_super_jump(s: &mut Pawn, yaw: f32, direction: Vec3) {
    let (kind, power) = resolve(s, 2);
    let push_speed = if direction == Vec3::ZERO { 0.0 } else { A.jump_flow_inertial_velocity };
    s.queue.push_state(Jump { kind, no_air_jump: true, push_dir: direction.normalize_or_zero(), push_speed, ..Jump::requested(yaw, power, 0) });
}

/// The same as a request: `ClearQueue`, then the push (`JumpRequest` from SUPER_SLIDE; also the
/// entry the magic side's airstep arrival uses).
pub(crate) fn super_jump_request(cx: &mut Ctx, yaw: f32, direction: Vec3) {
    let s = &mut *cx.pawn;
    s.queue.clear();
    push_super_jump(s, yaw, direction);
}

/// `JumpHeld` (slot 43 0x14143E600): if the running state is JUMP: `held` (+0x94) = 1.
pub(crate) fn jump_held(state: &mut StateData) {
    if let StateData::Jump(j) = state {
        j.held = true;
    }
}

/// `MoveInput(yaw, mag)` (slot 35 0x141440600): only when the running state is JUMP (any
/// sub-step).
pub(crate) fn move_input(state: &mut StateData, yaw: f32, magnitude: f32) {
    if let StateData::Jump(j) = state {
        j.yaw = yaw;
        j.request = true;
        j.magnitude = magnitude;
    }
}

/// The Jump state between its launch and its apex (sub-step 1: the start clip after the launch
/// notify; 2: the rise loop). For the port option `Tuning::jump_cut_on_release`, which covers
/// the jumps the game itself lets the player shorten: the normal jump and the High Jump. A
/// Doubleflight and a super jump have no height cut in the game (II.3) and get none here.
pub(crate) fn rising_from_a_jump(state: &StateData) -> bool {
    matches!(state, StateData::Jump(j) if (j.sub == 1 || j.sub == 2) && j.kind < 2)
}

/// `Fall(yaw)` (slot 172 0x141444C00, `same`): `RequestJump(yaw, 0, bFall = true)`: Jump begins
/// at sub 3; the callers follow it with `DoChain`, so both are done here.
pub(crate) fn fall(s: &mut Pawn, yaw: f32) {
    let power = s.params().jump_power_normal;
    s.queue.push_state(Jump::requested(yaw, power, 3));
    s.queue.do_chain();
}

/// `RequestJump(yaw, 0, bFall = 1)` without the chain: for a request (the ledge hang's drop).
pub(crate) fn push_fall(s: &mut Pawn, yaw: f32) {
    let power = s.params().jump_power_normal;
    s.queue.push_state(Jump::requested(yaw, power, 3));
}

/// The Air Slide's stick exit (0x141340BC1): `Fall(yaw)` with Jump flag 0x40: the slide's clip
/// plays to its end before the down loop.
pub(crate) fn fall_keeping_clip(s: &mut Pawn, yaw: f32) {
    let power = s.params().jump_power_normal;
    s.queue.push_state(Jump { keep_clip: true, ..Jump::requested(yaw, power, 3) });
    s.queue.do_chain();
}

impl PlayerState for Jump {
    const ID: StateId = StateId::Jump;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// `TresPlayerState_Jump::Begin` 0x1413868C0 (4.2, 4.5; actions spec II.3).
    fn begin(&mut self, cx: &mut Ctx) {
        // Flag 0x2000: the weapon is out (0x1413869B7).
        self.b_names = cx.pawn.actions.weapon_out;
        if self.kind == 2 {
            // The flow jump draws the weapon (0x141386DE1).
            actions::draw(cx, true);
        }
        let (j, w, s) = (self, cx.world, &mut *cx.pawn);
        let p = *s.params();
        if j.sub != 3 {
            // SetSecondJump(type != 3) (0x141386DB0): a double flight consumes the bit, any
            // other jump sets it.
            s.actions.second_jump = j.kind != 3;
        }
        if j.sub == 3 {
            // m = stick magnitude; if m >= 0.1: mag = m; request = 1 (0x141386A52).
            if s.stick_magnitude >= p.input_stick_active_min {
                j.magnitude = s.stick_magnitude;
                j.request = true;
            }
            let (hit, distance, ground_z) = ground_probe(s, w, p.probe_range);
            j.probe_distance = distance;
            if hit {
                j.ground_z = ground_z;
            }
            j.probe_cached = true;
            // The step-down flag: the run loop is playing, a request, and the stored probe distance
            // <= 40, whatever the hit flag (change D10).
            j.step_down = matches!(s.anim.name, AnimId::RunLoopN | AnimId::RunLoopW | AnimId::RunLoopB) && j.request && j.probe_distance <= p.fall_step_down_height;
            if s.cmc.floor_under(w, &p) {
                // Standing on a walkable floor (component vt+0x9D8, 0x141386BED): no air jump
                // from here; a stored land pose plays its landing clip at once (II.3 "Landing
                // variants and the land pose").
                j.no_air_jump = true;
                s.actions.second_jump = true;
                if let Some(name) = actions::land_name(actions::land_pose_of(s), s.actions.weapon_out) {
                    s.cmc.stop_movement();
                    s.anim.play_with(name, PlayArgs::mode(2, -1.0));
                    s.queue.flags.enable_change = false;
                    j.sub = 10;
                    j.keep_clip = false;
                }
            } else {
                // 0x141386D48: a fall may jump at once.
                j.second_enabled = true;
            }
        }
        if j.kind == 2 {
            // +0xF8 = 1; +0x9C = 1; the flow-jump bit 3 is consumed; SetSecondJump(0).
            j.flow = true;
            j.no_air_jump = true;
            s.actions.flow_jump = false;
            s.actions.second_jump = false;
        }
        // SetMaxWalkSpeed(default 680); SetMaxAcceleration(680 x 5.4 = 3672) (0x141386E5D).
        s.cmc.max_walk_speed = p.speed_run;
        s.cmc.max_acceleration = p.speed_run * p.accel_air_factor;
        j.start_z = s.cmc.location.z;
        if j.sub == 0 {
            // SetSubStep(0): Play(name(type, 0, B), blend-in -1, mode 0).
            play_jump(s, jump_name(j.kind, 0, j.b_names), 0);
        }
        j.last_z = s.cmc.location.z;
    }

    /// `TresPlayerState_Jump::Tick` 0x141395450 (4.2, 4.5, 4.6; actions spec II.3).
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        // ---- the traversal exits of tier 2 (actions spec II.5 R1, II.6; no effect in a world
        // without rails, hangable ledges, hop points and slide floors).
        {
            let me = StateData::Jump(*self);
            // Floating mode (the fall entry of Begin 0x141386A27 and the falling tail 0x1413981C1; II.2
            // "Floating"): `PushState(5)`. Not while a jump out of the mode still rises.
            if matches!(self.sub, 3 | 4) && floating::try_enter(&me, cx.pawn) {
                cx.pawn.queue.do_chain();
                return;
            }
            // The rail probe (0x1413957F6): range 150; the stick's yaw when requested, else the facing.
            let want = if self.request { self.yaw } else { cx.pawn.yaw };
            if rail_slide::probe_and_try(&me, cx, A.rail_check_air_range, want) {
                cx.pawn.queue.do_chain();
                return;
            }
            if !on_ground(cx.pawn) && matches!(self.sub, 1 | 2 | 4) {
                // The tail (0x141397FE6 - 0x141398192): the ledge, else a hop point.
                let yaw = cx.pawn.yaw;
                if dangle::try_from_jump(&me, cx) || hopping::try_hopping(&me, cx, yaw, false) {
                    cx.pawn.queue.do_chain();
                    return;
                }
            } else if self.sub == 4 && slope_slide::try_slope_slide(&me, cx, false) {
                // The landing path (0x1413971AC): a slide floor under him.
                cx.pawn.actions.traversal.last_hop = None;
                cx.pawn.queue.do_chain();
                return;
            }
        }
        let mut roll_land = false;
        let j = &mut *self;
        let (w, s) = (cx.world, &mut *cx.pawn);
        let p = *s.params();
        // The probe, once at the top of each tick (not on the first tick after a fall-entry Begin).
        if !j.probe_cached {
            let (hit, distance, ground_z) = ground_probe(s, w, p.probe_range);
            j.probe_distance = distance;
            if hit {
                j.ground_z = ground_z;
            }
        }
        let actor_z = s.cmc.location.z;
        // (XSI) Diving Strike's head-first pose (XS 2.1; KH3's sub-steps 7 / 8 with our entry rule): asked here,
        // before the sub-step code borrows the pawn.
        let dive_pose = j.sub == 4 && !j.flow && !skip_dive_pose(s) && crate::abilities::xs::dive::pose_ok(s, j.fall_time, j.probe_distance);
        let mut chained = false;
        let mut skip_sub4 = false;
        if j.sub == 3 {
            // Fall entry (4.5, change D9).
            if !j.request {
                // flags &= ~0x60 (0x141395F25): the step-down and the keep-clip flags.
                j.step_down = false;
                if j.keep_clip {
                    j.keep_clip = false;
                    s.root_scale = (1.0, 1.0);
                }
            }
            if j.probe_distance > p.fall_step_down_height {
                j.step_down = false;
            }
            if j.keep_clip && !s.anim.ended {
                // Flag 0x40 (0x141395F48 - 0x141395F74): the Air Slide's clip plays to its end.
                skip_sub4 = true;
            } else if j.step_down {
                // Keep the run loop playing; no landing test while the flag is set.
                if s.anim.name != AnimId::RunLoopN {
                    s.anim.play(AnimId::RunLoopN);
                }
                j.step_down_timer += dt;
                if j.step_down_timer > p.fall_step_down_time {
                    j.step_down = false; // acts next tick
                }
                skip_sub4 = true;
            } else {
                if j.keep_clip {
                    j.keep_clip = false;
                    s.root_scale = (1.0, 1.0);
                }
                play_jump(s, jump_name(0, 2, j.b_names), 1);
                j.sub = 4;
                j.last_z = actor_z;
            }
        }
        match j.sub {
            // 0: waiting for the TresJumpStart notify.
            0 => {}
            // The super jump (type 2; 0x1413958E3): the 1.83 s start clip plays while he rises
            // and falls; no rise loop, no height cut.
            1 if j.kind == 2 => {
                if on_ground(s) && j.risen {
                    s.cmc.stop_movement();
                    if j.request && !(p.land_roll_height_min > j.fall_height) {
                        roll_land = true;
                    } else {
                        s.anim.play_with(if j.b_names { AnimId::LandB } else { AnimId::Land }, PlayArgs::mode(2, -1.0));
                    }
                    s.queue.flags.enable_change = false;
                    j.sub = 10;
                } else {
                    if s.anim.ended {
                        play_jump(s, jump_name(2, 2, j.b_names), 1);
                        j.sub = 4;
                    } else if !j.risen && s.cmc.velocity.z > 0.0 {
                        j.risen = true;
                    }
                    j.last_z = actor_z;
                }
            }
            1 => {
                // The end of the start clip is tested first (change D16).
                if s.anim.ended {
                    play_jump(s, jump_name(j.kind, 1, j.b_names), 1);
                    j.sub = 2;
                }
                j.last_z = actor_z;
                // Type 3 (0x141395D00): no height cut, no slot 113.
                if j.kind != 3 {
                    // "held == 0 && AnimTime > 0.1" (0x141395D36): never true for the 0.1 s clip
                    // of the normal jump (the compared constant equals the clip length bit for
                    // bit and the test is strict); LIVE for the High Jump's 0.5 s start clip: the
                    // cut starts with the state tick whose clip time is 0.1167 s.
                    // (PORT OPTION `Tuning::jump_cut_on_release` replaces this rule: see sora.rs.)
                    if j.kind == 1 && !j.held && s.anim.time > A.jump_small_jump_clip_time && !s.tuning.jump_cut_on_release {
                        s.cmc.gravity_scale = p.gravity_small_jump_scale;
                    }
                    j.update_second_enabled(s);
                }
            }
            2 => {
                if s.cmc.velocity.z > 0.0 {
                    // The short hop: the 3.0 acts from the next movement tick (0x141395EE5);
                    // not for a double flight (0x141395EDC).
                    // (PORT OPTION `Tuning::jump_cut_on_release` replaces this rule: see sora.rs.)
                    if j.kind != 3 && !j.held && !s.tuning.jump_cut_on_release {
                        s.cmc.gravity_scale = p.gravity_small_jump_scale;
                    }
                } else {
                    if s.cmc.jump_duration_finished(&p) {
                        play_jump(s, jump_name(j.kind, 2, j.b_names), 1);
                    } else {
                        j.deferred_down_loop = true;
                    }
                    j.sub = 4;
                    j.last_z = actor_z;
                    s.cmc.gravity_scale = p.gravity_scale;
                }
                j.update_second_enabled(s);
            }
            // (XSI) DIVING STRIKE's head-first pose (XS 2.1): KH3's dive fall (sub-steps 7 / 8, Jump tick @0x141396653 -
            // 0x1413966CB) with OUR entry (sora-xs-abilities dive.pose_*) and steering speed: DiveFallStart, then the
            // DiveFallLoop blend space (mode 3); on the ground RollLand (sub 10). The glide (KH3's gate takes 8) or
            // the air dodge leave it; the Attack press is the strike (abilities/xs/dive.rs).
            4 if dive_pose && !skip_sub4 && !on_ground(s) => {
                s.anim.play_with(AnimId::DiveFallStart, PlayArgs::mode(2, -1.0));
                s.cmc.max_walk_speed = crate::abilities::xs::X.dive_pose_steer_speed;
                j.sub = 7;
                j.last_z = actor_z;
            }
            7 | 8 => {
                if on_ground(s) {
                    j.no_air_jump = true;
                    j.second_enabled = false;
                    s.actions.second_jump = true;
                    s.cmc.stop_movement();
                    s.cmc.max_walk_speed = p.speed_run;
                    roll_land = true;
                    s.queue.flags.enable_change = false;
                    j.sub = 10;
                } else if j.sub == 7 && s.anim.ended {
                    s.anim.play_with(AnimId::DiveFallLoop, PlayArgs::mode(3, -1.0));
                    j.sub = 8;
                } else if j.sub == 8 {
                    // The lean: KH3's blend X = stick magnitude x 10 by the side of the turn (here: the request's yaw).
                    let delta = crate::sora::wrap_degrees(j.yaw - s.yaw);
                    let lean = if !j.request || delta.abs() < 15.0 { 0.0 } else { j.magnitude.min(1.0) * 10.0 * delta.signum() };
                    s.anim.set_axis(lean);
                }
                j.last_z = actor_z;
            }
            4 if !skip_sub4 => {
                if j.flow && !(actor_z > j.start_z) {
                    // The flow jump ends when he is back at its start height (0x14139621F).
                    j.flow = false;
                    j.kind = 0;
                }
                if on_ground(s) {
                    // Landing (4.6): the same frame as the movement component landed.
                    j.no_air_jump = true;
                    j.second_enabled = false;
                    s.actions.second_jump = true;
                    s.cmc.stop_movement();
                    let weapon_out = s.actions.weapon_out;
                    if !(j.fall_height > p.land_light_height_max) {
                        // LandName(2, weaponOut): LightLand / LightLandB.
                        actions::play_as(s, if weapon_out { AnimId::LightLandB } else { AnimId::LightLand }, AnimId::LightLand);
                    } else if j.request && j.fall_height >= p.land_roll_height_min {
                        // A stick request on the landing tick and a fall of 800 cm or more
                        // (comiss h,800; jb @0x1413962E3): Sheathe, "RollLand".
                        roll_land = true;
                    } else {
                        actions::play_as(s, if weapon_out { AnimId::LandB } else { AnimId::Land }, AnimId::Land);
                    }
                    s.queue.flags.enable_change = false;
                    j.sub = 10;
                } else {
                    if j.deferred_down_loop && s.cmc.jump_duration_finished(&p) {
                        play_jump(s, jump_name(j.kind, 2, j.b_names), 1);
                        j.deferred_down_loop = false;
                    }
                    j.update_second_enabled(s);
                }
            }
            10 | 11 => {
                if j.sub == 10 {
                    j.sub = 11;
                    // The landing snap (0x141397358..0x141397380): onto the probe's ground point.
                    if actor_z > j.ground_z && actor_z - j.ground_z <= p.land_snap_height {
                        s.cmc.sweep_to_height(w, &p, j.ground_z);
                    }
                    // (A landing ends the airtime: the hop point hit last may be taken again.)
                    s.actions.traversal.last_hop = None;
                    // vt+0x1E88(1); vt+0x1E98(1); flags &= ~8; SetSecondJump(1).
                    s.actions.flow_jump = true;
                    s.actions.superslide_ok = true;
                    j.second_enabled = false;
                    s.actions.second_jump = true;
                    // ... then the sub-11 code in the same tick (change D3).
                }
                if j.flow {
                    j.flow = false;
                    j.kind = 0;
                }
                // Control returns at the TresEnableChange notify; else at the clip's end.
                if (j.request && s.queue.flags.enable_change) || s.anim.ended {
                    s.cmc.stop_movement();
                    chained = true;
                }
            }
            _ => {}
        }
        if roll_land {
            // Sheathe(1, 1.0) (a no-op for the plain Keyblade); Play("RollLand", -1, mode 2):
            // 300 cm of root motion, control back at its TresEnableChange (0.4325 s).
            actions::sheathe(cx);
        }
        let s = &mut *cx.pawn;
        if roll_land {
            s.anim.play_with(AnimId::RollLand, PlayArgs::mode(2, -1.0));
        }
        if !chained {
            // TAIL (every sub-step): Turn(yaw, dt x 6) (0x141397886); input only when requested and
            // not in 10 / 11 (change D8).
            turn(s, j.yaw, dt * p.turn_air_rate);
            if j.request && !matches!(j.sub, 10 | 11) {
                set_movement_input(s, j.yaw);
            }
        }
        // END.
        // The super jump's inertial push (slot 118 0x1413832C0): any stick input cancels it for good.
        if j.push_speed > 0.0 {
            if j.request {
                j.push_speed = 0.0;
            } else if j.sub <= 4 && j.push_dir != Vec3::ZERO {
                // AddImpulse(direction x speed x dt x 60, velocity change): applied by the next movement tick.
                s.cmc.velocity += j.push_dir * j.push_speed * dt * 60.0;
                j.push_speed *= j.push_brake.powf(dt * 60.0);
            }
        }
        j.request = false;
        j.held = false;
        if j.sub == 4 {
            j.fall_time += dt;
            let z = s.cmc.location.z;
            j.fall_height = if s.cmc.velocity.z >= 0.0 { 0.0 } else { j.fall_height + (j.last_z - z) };
            j.last_z = z;
        }
        j.probe_cached = false;
        j.ground_z = s.cmc.location.z;
        if chained {
            s.queue.do_chain();
        }
    }

    /// Jump::End 0x14138D850: clear the pawn jump latch; ResetMaxWalkSpeed; GravityScale =
    /// class default. MaxAcceleration stays 3672. Actions spec II.3: the glide lock pawn+0xE34
    /// is zeroed, and on the ground the air-slide and air-guard cool-downs.
    fn end(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        let p = *s.params();
        s.cmc.stop_jumping();
        s.cmc.max_walk_speed = p.speed_run;
        s.cmc.gravity_scale = p.gravity_scale;
        s.combat.command_cooldown = 0.0;
        if on_ground(s) {
            s.combat.air_slide_cooldown = 0.0;
            s.combat.air_guard_cooldown = 0.0;
        }
        if self.keep_clip {
            s.root_scale = (1.0, 1.0);
        }
    }

    /// TresAnimNotify_JumpStart -> handler 0x14139AB60, only if sub == 0 (4.1): pawn slot 530
    /// (bPressedJump = 1, latch = 1, JumpKeyHoldTime = 0), JumpZVelocity = power, sub = 1. All
    /// types use this same latch -> `CheckJumpInput` -> `DoJump` path and the same gravity hook.
    fn on_notifies(&mut self, cx: &mut Ctx, notifies: &Notifies) {
        if notifies.jump_start && self.sub == 0 {
            // Launched from the ground (the probe: hit && dist < 2) and not a flow jump:
            // vt+0x1E88(1); vt+0x1E98(1); flags &= ~8; SetSecondJump(1) (0x14139ABE3 - 0x14139AC08).
            if !self.flow {
                let p = *cx.pawn.params();
                let (hit, distance, _) = ground_probe(cx.pawn, cx.world, p.probe_range);
                if hit && distance < p.probe_near_ground_max {
                    let a = &mut cx.pawn.actions;
                    (a.flow_jump, a.superslide_ok, a.second_jump) = (true, true, true);
                    self.second_enabled = false;
                }
            }
            cx.pawn.cmc.press_jump(self.power);
            self.sub = 1;
        }
    }

    fn movement_state(&self, _pawn: &Pawn) -> MovementState {
        match self.sub {
            0 => MovementState::JumpStart,
            1 | 2 => MovementState::JumpRise,
            10 | 11 => MovementState::Land,
            _ => MovementState::Fall,
        }
    }
}

/// (XSI) The Jump state's sub-step (Diving Strike asks for the falling ones: 3, 4, 7, 8).
pub(crate) fn sub_of(state: &StateData) -> Option<u8> {
    match state {
        StateData::Jump(j) => Some(j.sub),
        _ => None,
    }
}

/// (XSI) The head-first pose is not taken in water or on a flow jump's way back (KH3's own exclusions of 7).
fn skip_dive_pose(s: &Pawn) -> bool {
    s.status.placeholder
}

/// For hosts and tests: the running jump's type (0 normal, 1 high, 2 super, 3 double flight).
pub(crate) fn kind_of(state: &StateData) -> Option<u8> {
    match state {
        StateData::Jump(j) => Some(j.kind),
        _ => None,
    }
}

#[allow(dead_code)]
const _: u8 = land_pose::NONE;
