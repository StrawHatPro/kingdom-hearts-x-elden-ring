//! (LADDER2 2026-10-25) THE LADDER JUMP: PORT DESIGN, KH3 HAS NO LADDERS.
//!
//! The user (2026-10-25): "change how ladders work, make them jump up the entire height of the ladder and safely land on
//! top of it when Y is pressed instead of climbing it ... Regardless of orientation, the party leader and his party must
//! end up where the elden ring character would normally finish climbing the ladder facing the same way as the tarnished."
//!
//! The game host (kh-er) reads the landing (the game's own ladder top, its heading) when the leader's Y starts the game's
//! ladder; everybody then LEAPS there on one arc ([`Leap`], pure, a function of time only: frame-rate independent):
//! 1. WAIT `delay` s (the allies go one after the other, right after the leader);
//! 2. START: KH3's own `JumpNormalStart` on the floor for `START_S` s, turning toward the landing's heading;
//! 3. RISE / FALL: a parabola under `GRAVITY` whose top is `CLEAR` cm above the higher end (KH3's `JumpNormalRiseLoop`,
//!    then `JumpNormalDownLoop`). He goes straight up first: the horizontal move to the landing starts only once he is
//!    `FOOT` cm above the landing floor, so the arc never cuts through the ledge the ladder leans on;
//! 4. LAND: on the landing, facing its heading, KH3's `Land` for `LAND_S` s; then he is himself again (Idle).
//!
//! The arc is kh-core's (`kh_core::leap`). The whole leap is kinematic (no collision: the landing is the game's own
//! exit of the ladder) and the member refuses hits and commands meanwhile (only Warp ends it; the host ends it with
//! [`cancel`]). `MemberCommand::LadderJump` gives it to an AI ally; the host gives it to a pad-driven leader directly
//! ([`begin`]).

use glam::Vec3;

pub use kh_core::leap::{wrap180, Leap, Phase, Sample, CLEAR, FOOT, GRAVITY, LAND_S, START_S};

/// The command: the landing (the floor point the feet land on, KH3 space), the heading he lands facing (deg), and how
/// long he waits before he jumps (s).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct LadderJump {
    pub to: Vec3,
    pub facing: f32,
    pub delay: f32,
}

/// KH3's clip for a phase (the friends' NPC jump names, crate::jump `anim_name`; looped: the rise and the fall).
pub fn clip(phase: Phase) -> Option<(&'static str, bool)> {
    match phase {
        Phase::Start => Some(("JumpNormalStart", false)),
        Phase::Rise => Some(("JumpNormalRiseLoop", true)),
        Phase::Fall => Some(("JumpNormalDownLoop", true)),
        Phase::Land => Some(("Land", false)),
        Phase::Wait | Phase::Done => None,
    }
}

/// The member's origin above the floor his feet stand on (his movement's floor distance).
pub fn lift(pw: &crate::pawn::NpcPawn) -> f32 {
    let p = &pw.params;
    (p.floor_dist_target - (p.capsule_center_z - p.capsule_half_height)).max(0.0)
}

/// THE LEAP BEGINS (the command, or the host for a pad-driven leader): from where he is, to the landing.
pub fn begin(pw: &mut crate::pawn::NpcPawn, jump: LadderJump) {
    let to = jump.to + Vec3::Z * lift(pw);
    pw.ladder = Leap::new(pw.position(), pw.yaw, to, jump.facing, jump.delay);
    pw.enter(crate::states::NpcStateId::Ladder);
    pw.action = None;
    pw.steer = None;
    pw.cmc.stop_movement();
    if jump.delay <= 0.0 {
        pw.play("JumpNormalStart", -1.0);
    }
}

/// THE HOST ENDS A LEAP (a reseat, a warp of its own): the command aborted, the member idle where he is now.
pub fn cancel(pw: &mut crate::pawn::NpcPawn) {
    if pw.state == crate::states::NpcStateId::Ladder {
        pw.abort_command_pub();
        pw.to_idle();
    }
}

/// One tick (after the movement component: the actor is placed by the leap).
pub fn tick(pw: &mut crate::pawn::NpcPawn, dt: f32) {
    let before = pw.ladder.sample().phase;
    let s = pw.ladder.advance(dt);
    if s.phase != before {
        match clip(s.phase) {
            Some((name, true)) => pw.play_looped(name, -1.0),
            Some((name, false)) => pw.play(name, -1.0),
            None => {}
        }
    }
    pw.cmc.location = s.position;
    pw.cmc.stop_movement();
    pw.yaw = s.yaw;
    if s.phase == Phase::Done {
        pw.finish_command_pub(true);
        pw.to_idle();
    }
}

