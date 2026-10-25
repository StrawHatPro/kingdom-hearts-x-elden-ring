//! (B5) The friends' jump and fall: `UTresAICharState_NavLinkJumpCodeDriven` (`TSID_STATE_NAVLINK_JUMP`
//! 0x2B, ctor 0x14042FA70) over `UTresAICharState_JumpCodeDriven` / `UTresAICharState_Jump`, transcribed
//! from the decompiled C. The NPC base pawn registers it (state list of ctor 0x141254FF0) next to
//! `UTresNpcState_Jump` (id 3), and every jump request of a friend lands here:
//! - `RequestJump` (iface vt+0x510 0x14054C2E0) asks for 0x2B first, 3 only when 0x2B is missing, and calls
//!   vt+0x398 (0x140458D00) with FLT_MAX (no target: the FALL) and vt+0x3B0 (the yaw; a no-op 0x14041F120
//!   for NavLinkJump). Callers: the Run tick 0x140442430 and the Idle tick 0x14124CD40 when he is not on the
//!   ground, the SlopeSlide tick, the Hopping hop-off.
//! - `JumpCodeDriven` (iface vt+0x3A8 0x14054C6A0) asks for 0x2B and calls vt+0x390 (0x140459150) with the
//!   target (the RailSlide leave, slot 145 0x141252210; a host's nav link).
//!
//! The state's functions (vtable 0x145C963E8): Init slot 0x2F8 0x14043E470, Begin 0x300 0x140433610, Tick
//! 0x308 0x1404408A0 (UTresAICharState_Jump's), End 0x310 0x140435040, CanChange 0x388 0x1404325F0, target
//! 0x390 0x140459150 (base 0x140458E80), fall 0x398 0x140458D00 (base 0x140458820), fall-with-yaw 0x3A0
//! 0x140459610, feed 0x3A8 0x140456DE0, sub-steps 0x3B8 0x140459D70, air phase 0x3C0 0x14045A5E0 (Jump's),
//! air probes 0x3D0 0x140446070, done 0x3F0 0x14043EAD0, launch 0x3F8 0x140433FD0, stuck 0x400 0x140438BD0
//! (base 0x1404386F0). Clips: the NPC anim names vt+0x1C88 = 0x141263A00 (0 JumpNormalStart, 1
//! JumpNormalRiseLoop, 2 JumpNormalDownLoop, 3 JumpNormalDownStart, 4 Land, 5 LightLand), `RollLand`
//! (FName DAT_1487C7420, static init 0x14009A3A0), `LightLandR` / `LightLandL` after a hop clip.

use crate::host::PartyHost;
use kh_core::World;
use crate::pawn::{forward, horizontal, wrap180, yaw_of, NpcPawn, StepCtx};
use crate::states::NpcStateId;
use crate::tables;
use glam::{Vec2, Vec3};
use kh_core::cmc::MovementMode;

fn p(id: &str) -> f32 {
    tables::param(id)
}

/// The NPC jump anim names (ATresNpcPawnBase vt+0x1C88 = 0x141263A00).
pub fn anim_name(i: u8) -> &'static str {
    match i {
        0 => "JumpNormalStart",
        1 => "JumpNormalRiseLoop",
        2 => "JumpNormalDownLoop",
        3 => "JumpNormalDownStart",
        4 => "Land",
        5 => "LightLand",
        _ => "",
    }
}

/// The fields of `UTresAICharState_JumpCodeDriven` the port runs (Init 0x14043E470).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JumpRun {
    /// +0x88 the air phase of 0x14045A5E0: 0 start, 1 wait for the launch, 2 in the air, 3 landed, 4 the
    /// stuck escape.
    pub phase: u8,
    /// +0x8C..+0x94 the target (FLT_MAX = none: a fall).
    pub target: Option<Vec3>,
    /// +0x98 / +0x9C: FUN_140578330's h0 (100) and h_min (0; 400 from the rail leave).
    pub h0: f32,
    pub h_min: f32,
    /// +0xA0: the yaw he steers along (toward the target, else the actor yaw at the request).
    pub yaw: f32,
    /// +0xA4: the AI move fed this tick (vt+0x3A8), cleared at the tick's end.
    pub input: bool,
    /// +0xA8 (+0xA9): the air steering on regardless of the feed (0x101 from the rail leave 0x141252210).
    pub steer: bool,
    /// +0xA5: a fall request (vt+0x398 / vt+0x3A0); +0xA6: launched (DoJump's answer, or set by the fall).
    pub fall: bool,
    pub jumped: bool,
    /// +0x100 the time of the last height sample, +0x104 / +0x108 / +0x110 the ring of 10 heights.
    pub sample_at: f32,
    pub samples: [f32; 10],
    pub sample_next: usize,
    pub sample_count: usize,
    /// +0x124: the stuck escape's time left.
    pub escape_left: f32,
    /// +0x128: set by vt+0x3A0 (the LightLand landing first).
    pub fall_kind: bool,
    /// +0x12C: the sub-step of 0x140459D70 (0 start, 1 start clip, 2 rising, 3 down start, 4 falling, 6
    /// landing clip, 9 done).
    pub sub: u8,
    /// +0x130: JumpNormalStart carries a TresAnimNotify_JumpStart (the launch waits for it).
    pub has_jump_start: bool,
    /// +0x131: falling while the jump duration still runs (the down clip once it is over).
    pub wait_duration: bool,
    /// +0x132: the launch aimed at a ledge (the air probe may grab it, Dangle).
    pub dangle_ok: bool,
    /// +0x148 the height fallen in sub-step 4, +0x14C the last Z.
    pub fall_height: f32,
    pub last_z: f32,
    /// +0x151: the anim set has `RollLand` (FUN_1405ACE80 = anim vt+0x88 HasAnim(DAT_1487C7420)).
    pub roll_ok: bool,
    /// TresStateBase +0x84 m_bEnableChange (the clips' TresAnimNotify_EnableChange).
    pub enable_change: bool,
    /// The state's clock (UWorld TimeSeconds for the sample timer).
    pub clock: f32,
}

impl Default for JumpRun {
    /// Init 0x14043E470 (+ the UTresAICharState_Jump ctor 0x14042F7D0 fields it keeps).
    fn default() -> JumpRun {
        JumpRun {
            phase: 0,
            target: None,
            h0: p("jump.h0"),
            h_min: 0.0,
            yaw: 0.0,
            input: false,
            steer: false,
            fall: false,
            jumped: false,
            // +0x100 = 0 while the world clock is far past 0.5: the first sample is taken at once
            sample_at: f32::MIN / 4.0,
            samples: [0.0; 10],
            sample_next: 0,
            sample_count: 0,
            escape_left: 0.0,
            fall_kind: false,
            sub: 0,
            has_jump_start: false,
            wait_duration: false,
            dangle_ok: false,
            fall_height: 0.0,
            last_z: 0.0,
            roll_ok: false,
            enable_change: false,
            clock: 0.0,
        }
    }
}

fn has(pw: &NpcPawn, name: &str) -> bool {
    tables::anim_set(pw.chara).get(name).is_some()
}

/// PlayAnim through the state's anim interface (+0x40 vt+0x18): > 0 when the row exists. Mode 3 rows loop.
fn play(pw: &mut NpcPawn, name: &'static str, looping: bool) -> bool {
    if !has(pw, name) {
        return false;
    }
    let c = tables::clip(pw.chara, name);
    if looping {
        pw.anim.play_loop(c, -1.0);
    } else {
        pw.anim.play(c, -1.0);
    }
    pw.hit_list.clear();
    true
}

fn falling(pw: &NpcPawn) -> bool {
    pw.cmc.mode == MovementMode::Falling
}

/// The pending state request (FUN_1405B4F80): the running state's End, then Init (0x14043E470).
fn push(pw: &mut NpcPawn) {
    pw.end_action();
    pw.enter(NpcStateId::NavLinkJump);
    pw.trav.jump = JumpRun::default();
}

/// `RequestJump` (iface vt+0x510 0x14054C2E0) -> the fall: vt+0x398 0x140458D00 over the base 0x140458820
/// with FLT_MAX: +0xA0 = the actor yaw, +0xA5 = +0xA6 = 1; a horizontal speed above the default
/// MaxWalkSpeed (vt+0x1588) is cut to it (vt+0x1578 resets MaxWalkSpeed, vt+0x16C0 the velocity); then
/// `JumpNormalDownStart` (mode 2, sub 3) when the set has it, else `JumpNormalDownLoop` (mode 3, sub 4).
/// Begin follows (the switch of the pending state).
pub fn request_fall(pw: &mut NpcPawn, w: &dyn World) {
    push(pw);
    let j = &mut pw.trav.jump;
    j.target = None;
    j.fall = true;
    j.jumped = true;
    j.yaw = pw.yaw;
    let max = pw.params.speed_run;
    let v = Vec2::new(pw.cmc.velocity.x, pw.cmc.velocity.y);
    if v.length() > max {
        pw.cmc.max_walk_speed = max;
        let c = v.normalize_or_zero() * max;
        pw.cmc.velocity.x = c.x;
        pw.cmc.velocity.y = c.y;
    }
    if play(pw, anim_name(3), false) {
        pw.trav.jump.sub = 3;
    } else {
        play(pw, anim_name(2), true);
        pw.trav.jump.sub = 4;
    }
    begin(pw, w);
}

/// `JumpCodeDriven` (iface vt+0x3A8 0x14054C6A0) -> vt+0x390 0x140459150 (base 0x140458E80): the target,
/// +0xA0 = the yaw toward it (XY), sub 0. `h_min` is the caller's +0x9C and `steer` its +0xA8 (the rail leave
/// writes 400 and 0x101: the air steering on whatever the feed says).
pub fn request_code_driven(pw: &mut NpcPawn, target: Vec3, h_min: f32, steer: bool, w: &dyn World) {
    push(pw);
    let d = horizontal(target - pw.position());
    let j = &mut pw.trav.jump;
    j.target = Some(target);
    j.h_min = h_min;
    j.yaw = if d.length_squared() >= 1.0e-8 { yaw_of(d) } else { 0.0 };
    j.sub = 0;
    j.steer = steer;
    begin(pw, w);
}

/// Begin 0x140433610: +0x130 = JumpNormalStart has a JumpStart notify; +0x151 = HasAnim(RollLand); coming
/// from a hop clip (`HopR` / `HopL` playing) onto a walkable floor (FUN_1405ADAA0): `LightLandR` / `L`
/// (mode 2), sub 6; +0x14C = Z; a swimming pawn falls (not on land).
fn begin(pw: &mut NpcPawn, w: &dyn World) {
    let start = tables::clip(pw.chara, anim_name(0));
    pw.trav.jump.has_jump_start = start.notifies.iter().any(|n| n.class == "TresAnimNotify_JumpStart");
    pw.trav.jump.roll_ok = has(pw, "RollLand");
    let land = match pw.anim.clip.name {
        "HopR" => Some("LightLandR"),
        "HopL" => Some("LightLandL"),
        _ => None,
    };
    if let Some(name) = land {
        if pw.cmc.floor_under(w, &pw.params) && play(pw, name, false) {
            pw.trav.jump.sub = 6;
        }
    }
    pw.trav.jump.last_z = pw.position().z;
}

/// End 0x140435040: the rotation back to its yaw only (FUN_143EAD160); vt+0x3D8 / 0x3E0 are no-ops here.
pub fn end(pw: &mut NpcPawn) {
    pw.trav.pitch = 0.0;
}

/// The AI move's feed (iface vt+0x118 0x141270D90, state 0x2B -> vt+0x3A8(1) = 0x140456DE0, then vt+0x3B0,
/// a no-op): with a target, +0xA4 = (+0xA5 ? 1 : +0xA6). A fall (no target) takes no input.
pub fn feed(pw: &mut NpcPawn) {
    let j = &mut pw.trav.jump;
    if j.target.is_some() {
        j.input = j.fall || j.jumped;
    }
}

/// vt+0x388 0x1404325F0: the landing clip with m_bEnableChange.
pub fn can_change(pw: &NpcPawn) -> bool {
    pw.trav.jump.sub == 6 && pw.trav.jump.enable_change
}

/// The launch, vt+0x3F8 0x140433FD0 (sub 0 without a JumpStart notify, else the JumpStart notify): once
/// (+0xA6). With a target more than `ATresNpcPawnBase +0xB34` (500, ctor 0x141254FF0) above him and the
/// Dangle state present: the ledge search along the arc (`ledge_arc`), which may aim the jump at the ledge
/// and set +0x132. Otherwise the target's jump velocity (FUN_140578330 with +0x98 / +0x9C =
/// `actions::jump_velocity`): MaxWalkSpeed = |v.xy| (vt+0x1570), JumpZVelocity = v.z (vt+0x1A28), DoJump
/// (movement vt+0x618). No target: DoJump with the JumpZVelocity there is.
pub fn launch(pw: &mut NpcPawn, w: &dyn World) -> bool {
    if pw.trav.jump.jumped {
        return true;
    }
    if let Some(target) = pw.trav.jump.target {
        if target.z - pw.position().z > p("jump.dangle_height") {
            if let Some(ok) = crate::climb::ledge_arc(pw, target, w) {
                pw.trav.jump.jumped = ok;
                return ok;
            }
        }
    }
    pw.trav.jump.jumped = true;
    if let Some(target) = pw.trav.jump.target {
        let g = pw.cmc.gravity_z(&pw.params);
        let (h0, hm) = (pw.trav.jump.h0, pw.trav.jump.h_min);
        let v = crate::actions::jump_velocity(pw.position(), target, h0, hm, g, pw.params.gravity_jump_up_scale, pw.params.gravity_jump_duration_scale, pw.params.gravity_jump_duration_time);
        if v == Vec3::ZERO {
            return false;
        }
        pw.cmc.max_walk_speed = Vec2::new(v.x, v.y).length();
        pw.cmc.jump_z_velocity = v.z;
    }
    pw.cmc.do_jump();
    true
}

/// Tick 0x1404408A0 (UTresAICharState_Jump's, run by the NavLinkJump state).
pub fn tick(pw: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let dt = cx.dt;
    pw.trav.jump.clock += dt;
    // The rail in the air (state 0x16): the probe from the location to 30 above, range 150, want = +0xB0
    // (zero for the friends) -> Rot(0, +0xA0, 0) with input, else the facing; a hit -> RailSlide (vt+0x260).
    if crate::traverse::has_rails(host.world, cx) {
        let loc = pw.position();
        let want = if pw.trav.jump.input { forward(pw.trav.jump.yaw) } else { forward(pw.yaw) };
        if let Some(hit) = crate::traverse::rail_probe(&mut pw.trav, host.world, cx.ice_rails, loc, loc + Vec3::Z * 30.0, p("jump.rail_range"), want) {
            crate::traverse::begin_rail(pw, hit);
            return;
        }
    }
    // (party I1) an ATresNpcPawnBase whose bad status (pawn +0x85D, FUN_1405A9DD0) is 12 STUN calls pawn vt+0x19D8(1)
    // (0x1405B9310 -> movement vt+0x998 0x1405943C0: +0xB70 bit 4 = 1), so the movement's "overlapped force in the
    // sky" getter vt+0x990 (0x140589250) answers false: a stunned friend in the air ignores force volumes. The port
    // has no force volumes (kh-core cmc: "left out ... force volumes"), so the flag is all there is to it.
    if pw.bad.0 == kh_core::combat::badstat::kind::STUN {
        pw.ignore_overlap_force = true;
    }
    sub_steps(pw, cx, host);
    if pw.state != NpcStateId::NavLinkJump {
        return;
    }
    let keep = air_phase(pw, cx, host);
    if keep || pw.trav.jump.sub != 9 {
        air_probes(pw, cx, host);
        if pw.state != NpcStateId::NavLinkJump {
            return;
        }
        // vt+0xDC8(dt x 6): the roll / pitch eased level (yaw only here); +0xA4 = 0; +0x118 = the location.
        pw.trav.jump.input = false;
        return;
    }
    // vt+0x270: done -> the default state
    crate::traverse::back_pub(pw);
}

/// Sub-steps 0x140459D70.
fn sub_steps(pw: &mut NpcPawn, cx: &StepCtx, host: &mut PartyHost) {
    let _ = cx;
    let finished = pw.anim.finished;
    let duration_over = pw.cmc.jump_duration_finished(&pw.params);
    match pw.trav.jump.sub {
        0 => {
            // vt+0x16E8: StopMovement and the input cleared
            pw.cmc.stop_movement();
            pw.cmc.control_input = Vec3::ZERO;
            if !pw.trav.jump.has_jump_start {
                launch(pw, host.world);
            }
            if play(pw, anim_name(0), false) {
                pw.trav.jump.sub = 1;
                return;
            }
            play(pw, anim_name(1), true);
            pw.trav.jump.sub = 2;
        }
        1 => {
            if !finished {
                return;
            }
            play(pw, anim_name(1), true);
            pw.trav.jump.sub = 2;
        }
        2 => {
            // vt+0x1568: Velocity.Z
            if pw.cmc.velocity.z > 0.0 {
                return;
            }
            if !duration_over {
                pw.trav.jump.sub = 4;
                pw.trav.jump.wait_duration = true;
            } else if play(pw, anim_name(3), false) {
                pw.trav.jump.sub = 3;
                pw.trav.jump.wait_duration = false;
            } else {
                play(pw, anim_name(2), true);
                pw.trav.jump.sub = 4;
                pw.trav.jump.wait_duration = false;
            }
            // (the C falls through to sub-steps 3 and 4)
            down_start(pw);
            falling_step(pw, host);
        }
        3 => {
            down_start(pw);
            falling_step(pw, host);
        }
        4 | 7 | 8 => falling_step(pw, host),
        6 => {
            if !finished {
                return;
            }
            pw.trav.jump.sub = 9;
            if pw.trav.jump.phase <= 1 {
                pw.trav.jump.phase = 2;
            }
        }
        _ => {}
    }
}

/// Sub-step 3: the down start's end -> the down loop, sub 4.
fn down_start(pw: &mut NpcPawn) {
    if pw.trav.jump.sub == 3 && pw.anim.finished {
        play(pw, anim_name(2), true);
        pw.trav.jump.sub = 4;
    }
}

/// Sub-steps 4 / 7 / 8: in the air the down clip once the jump duration is over (+0x131); landed (+0x88 ==
/// 3): m_bEnableChange off, then `RollLand` when the set has it and the fall was 2000 cm or more (800 with
/// input) and the root-motion path is clear (FUN_143ED09B0, a navigation raycast from the location to
/// where RollLand's root motion ends), else `LightLand` (only after vt+0x3A0) or `Land`; sub 6, or 9 with
/// no clip.
fn falling_step(pw: &mut NpcPawn, host: &PartyHost) {
    let _ = host;
    let j = pw.trav.jump;
    if j.phase != 3 {
        if !j.wait_duration || !pw.cmc.jump_duration_finished(&pw.params) {
            return;
        }
        if play(pw, anim_name(3), false) {
            pw.trav.jump.sub = 3;
        } else {
            play(pw, anim_name(2), true);
        }
        pw.trav.jump.wait_duration = false;
        return;
    }
    pw.trav.jump.enable_change = false;
    let big = j.fall_height >= p("jump.roll_height") || (j.input && j.fall_height >= p("jump.roll_height_input"));
    if j.roll_ok && big {
        // The navigation raycast over RollLand's root motion: the port has no navmesh (kh-ai StraightNav
        // answers every ray as clear) [D: no navmesh in the test level].
        if play(pw, "RollLand", false) {
            pw.trav.jump.sub = 6;
            return;
        }
    }
    if j.fall_kind && play(pw, anim_name(5), false) {
        pw.trav.jump.sub = 6;
        return;
    }
    pw.trav.jump.sub = if play(pw, anim_name(4), false) { 6 } else { 9 };
}

/// The air phase, vt+0x3C0 = UTresAICharState_Jump 0x14045A5E0: true while the state should go on.
fn air_phase(pw: &mut NpcPawn, cx: &StepCtx, host: &PartyHost) -> bool {
    let dt = cx.dt;
    let mut phase = pw.trav.jump.phase;
    if phase == 0 {
        let j = pw.trav.jump;
        if !j.fall {
            if falling(pw) && !j.jumped {
                return true;
            }
        } else if !falling(pw) {
            pw.trav.jump.phase = 2;
            return true;
        }
        phase = 1;
        pw.trav.jump.phase = 1;
    }
    if phase == 1 {
        if !falling(pw) && !pw.trav.jump.jumped {
            return true;
        }
        // vt+0x3E8(3): a no-op here
        phase = 2;
        pw.trav.jump.phase = 2;
    }
    match phase {
        2 => {
            // vt+0xDB0 toward (0, +0xA0, 0), alpha dt x 6; input: MaxAcceleration 1e6, AddMovementInput(dir, 1)
            let yaw = pw.trav.jump.yaw;
            pw.yaw = interp(pw.yaw, yaw, dt * p("jump.turn_rate"));
            if pw.trav.jump.input || pw.trav.jump.steer {
                pw.cmc.max_acceleration = p("jump.air_accel");
                pw.cmc.control_input = forward(yaw);
            } else {
                pw.cmc.control_input = Vec3::ZERO;
            }
            if !falling(pw) {
                // landed: vt+0x3E8(1) / vt+0x3E0(0) no-ops; StopMovement + input cleared; phase 3
                pw.cmc.stop_movement();
                pw.cmc.control_input = Vec3::ZERO;
                pw.trav.jump.phase = 3;
                return false;
            }
            stuck_check(pw, host);
            true
        }
        3 => false,
        4 => {
            let yaw = pw.trav.jump.yaw;
            pw.yaw = interp(pw.yaw, yaw, dt * p("jump.turn_rate"));
            pw.cmc.max_acceleration = p("jump.air_accel");
            pw.cmc.control_input = forward(yaw);
            pw.trav.jump.escape_left -= dt;
            if pw.trav.jump.escape_left <= 0.0 || !falling(pw) {
                pw.trav.jump.phase = 2;
            }
            true
        }
        _ => true,
    }
}

fn interp(yaw: f32, target: f32, alpha: f32) -> f32 {
    wrap180(yaw + wrap180(target - yaw) * alpha.clamp(0.0, 1.0))
}

/// The stuck check of phase 2: every 0.5 s of world time the height goes into a ring of 10; with 10 samples,
/// not rising (vt+0x1568 <= 0) and every sample within 5 cm of their mean: vt+0x400 (the escape), phase 4,
/// +0x124 = 4 s, the ring emptied.
fn stuck_check(pw: &mut NpcPawn, host: &PartyHost) {
    let j = &mut pw.trav.jump;
    if j.sample_at + p("jump.stuck_period") >= j.clock {
        return;
    }
    j.sample_at = j.clock;
    let z = pw.cmc.location.z;
    j.samples[j.sample_next] = z;
    j.sample_count = (j.sample_count + 1).min(10);
    j.sample_next = (j.sample_next + 1) % 10;
    if j.sample_count < 10 || pw.cmc.velocity.z > 0.0 {
        return;
    }
    let mean = j.samples.iter().sum::<f32>() / 10.0;
    if j.samples.iter().any(|s| (s - mean).abs() > p("jump.stuck_tolerance")) {
        return;
    }
    escape(pw, host);
    let j = &mut pw.trav.jump;
    j.phase = 4;
    j.escape_left = p("jump.escape_time");
    j.sample_count = 0;
    j.sample_next = 0;
}

/// vt+0x400 0x140438BD0 over 0x1404386F0: MaxWalkSpeed 500; JumpZVelocity = sqrt(-2 g h), h = 3 x the
/// collision half height (GetSimpleCollisionCylinder x 2 x 1.5); DoJump (else Velocity.Z = it); from the top
/// of that jump (a line up, channel 3, stops at a hit) 8 lines of 1000 cm at +0xA0 + k x 45 deg: +0xA0 = the
/// yaw of the longest free one (the first of equals); `JumpNormalRiseLoop` (mode 3), sub 2.
fn escape(pw: &mut NpcPawn, host: &PartyHost) {
    let w = host.world;
    let h = pw.params.capsule_half_height * 2.0 * 1.5;
    pw.cmc.max_walk_speed = p("jump.escape_speed");
    let g = pw.cmc.gravity_z(&pw.params);
    let vz = (g * -2.0 * h).max(0.0).sqrt();
    pw.cmc.jump_z_velocity = vz;
    pw.cmc.do_jump();
    let loc = pw.position();
    let top = w.line_trace(loc, loc + Vec3::Z * h).map_or(loc + Vec3::Z * h, |hit| hit.position);
    let mut best_yaw = pw.trav.jump.yaw;
    let mut best = 0.0f32;
    let mut yaw = pw.trav.jump.yaw;
    for _ in 0..8 {
        let end = top + forward(yaw) * 1000.0;
        let reach = w.line_trace(top, end).map_or(end, |hit| hit.position);
        let d2 = (reach - top).length_squared();
        if best < d2 {
            best_yaw = yaw;
        }
        best = best.max(d2);
        yaw = wrap180(yaw + 45.0);
    }
    pw.trav.jump.yaw = best_yaw;
    play(pw, anim_name(1), true);
    pw.trav.jump.sub = 2;
}

/// The air probes, vt+0x3D0 0x140446070: +0x148 += the height lost in sub-step 4; +0x14C = Z; (sub >= 3 and
/// the water test pawn vt+0x1A80 -> movement vt+0x5A0 with iface vt+0x548: never on land); with +0x132 and
/// falling (Velocity.Z < 0) the ledge probe FUN_140638740 (up = 275 - m_DangleOffsetPos.Z - 150, down 50,
/// radius 1.5 x the larger root extent) -> RequestDangle (iface vt+0x208 0x141276580) -> vt+0x270.
fn air_probes(pw: &mut NpcPawn, _cx: &StepCtx, host: &PartyHost) {
    let z = pw.position().z;
    if pw.trav.jump.sub == 4 {
        pw.trav.jump.fall_height += pw.trav.jump.last_z - z;
    }
    pw.trav.jump.last_z = z;
    // (party I1) sub-step >= 3 (+0x12C) and in water (pawn vt+0x1A80 0x1405AD980 -> movement vt+0x5A0 IsInWater)
    // and RequestSwim (iface vt+0x548 0x1412763D0) answers a state -> vt+0x270 (the jump ends into AI_SWIM)
    if pw.trav.jump.sub >= 3 && in_water(pw, host.world) && request_swim(pw, host.world) {
        crate::traverse::back_pub(pw);
        return;
    }
    if !pw.trav.jump.dangle_ok || pw.cmc.velocity.z >= 0.0 {
        return;
    }
    let up = crate::climb::dangle_probe_up(pw.chara);
    if let Some(ledge) = crate::climb::ledge_probe(pw, host.world, pw.position(), pw.yaw, up, p("dangle.probe_down")) {
        crate::climb::request_dangle(pw, ledge, false, host.world);
    }
}

/// (party I1) `IsInWater` (pawn vt+0x1A80 0x1405AD980 -> movement vt+0x5A0 0x140589410): the movement's physics
/// volume is a water volume (UE: the volume holding the updated component, the capsule centre). The port's water is
/// `World::water_surface` (inside a water volume).
pub fn in_water(pw: &NpcPawn, w: &dyn World) -> bool {
    w.water_surface(pw.capsule_centre()).is_some()
}

/// (party I1) `RequestSwim` (iface vt+0x548 = 0x1412763D0): the swim locomotion of `ATresNpcPawnBase::m_Swimming`
/// (+0xB60; n_ex001_PawnBase / n_ex002_PawnBase name n_ex00x_Swimming = UTresNpcLocomotionDefinitionSwim, state
/// TSID_STATE_AI_SWIM 143, ctor 0x141203410: MOVE_Swimming, surface 200..370 cm/s, under water 300..500) through
/// FUN_1405B4F60, +0x188 = 1 with the given point. NOT PORTED, so this answers no state (as the game does for a
/// pawn without m_Swimming): the locomotion runs on UE's PhysSwimming (kh-core's movement leaves swimming out, and
/// Sora's own Swim state 102 is not ported either), on the Mercuna 3D path following of its parent
/// UTresLocomotionDefinitionMercunaBase (the avoidance / desired-velocity function 0x141203B40), and under the
/// friends' swimming AI controller n_Common_SwimmingAIController; the water attacks (Attack8_WaterBeat /
/// Attack9_WaterSwing / Attack4_WaterUpper / Attack5_WaterRush) and water magic (n_ex001_ProjectileMagWater*) only
/// run in it. Every water entry point of the friends (the jump's air probes and stuck escape, Idle 0x14124CD40, the
/// Land locomotion 0x1412194F0, its slope slide 0x141219B80, Damage 0x1404463F0 / 0x141250B30) asks this.
pub fn request_swim(_pw: &mut NpcPawn, _w: &dyn World) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_npc_jump_anim_names_follow_0x141263a00() {
        assert_eq!(anim_name(0), "JumpNormalStart");
        assert_eq!(anim_name(3), "JumpNormalDownStart");
        assert_eq!(anim_name(5), "LightLand");
        assert_eq!(anim_name(6), "");
    }
}
