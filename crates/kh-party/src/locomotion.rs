//! (party K1) THE AI MEMBERS' LOCOMOTION: how a friend on a MoveTo runs, starts, stops and turns, and how fast.
//!
//! KH3 (read, with addresses):
//! - The friends' move locomotion `n_ex00x_Move1` is a `UTresNpcLocomotionDefinitionLand_FollowPlayer` (pack class
//!   dump: native TresNpcLocomotionDefinitionLand_FollowPlayer, only m_MinSpeed 140 / m_MaxSpeed 680 /
//!   m_RunLoopInterpolationTime set). Its constructor 0x141203230: m_Acceleration 500, m_MaxRotationSpeed.Yaw 180,
//!   m_InViewportSpeedRate 1.1, m_OutViewportSpeedRate 2.5, m_WarpStartDistance 3000, m_DecelerationTime 0.5,
//!   m_RangeSamePlayerSpeed 300, m_CorridorCheckInterval 5.
//! - THE FOLLOW SPEED (0x1412196C0, every tick while following, FUN_141214F90: out of battle, or in battle with the
//!   battle-follow mode): rate = in view ? 1.1 : 2.5 (+0x35C = FUN_140501500); within m_RangeSamePlayerSpeed (+ the
//!   definition's +0x15C, not identified: 0) of the leader rate = distance / that range; the reference = the leader's
//!   MaxWalkSpeed (his movement +0x388), times his movement's vt+0xC08 within 750 cm (read as AnalogInputModifier
//!   [inf]); the follow speed +0x34C = max(m_MinSpeed, reference x rate); the max speed getter 0x14120ECC0 answers
//!   +0x34C while following, else m_MaxSpeed (0x1404FF640). Branches not identified (controller +0x790 / +0x7A8,
//!   FUN_141257870 / FUN_141258540 / FUN_1412692D0, the movement +0x3A0 product test and leader vt+0x1B30) are left
//!   out [?]. THE PORT DID NOT HAVE THIS RULE: the friends ran at 680 behind a 980 sprint and fell 300 cm a second
//!   behind until kh-ai's Warp state (3000 cm) stood them still for 2 s and moved them.
//! - UTresAICharState_Run (the AI_MOVE state; tick 0x140442430, the movement / blend update 0x14045AAF0, the stop
//!   0x1404432D0, the clip names through the pawn: start vt+0x1C80 = 0x1412645F0 "RunStartN/W/B", loop vt+0x1C70 =
//!   0x141264520 "WalkRunN/W/B", stop vt+0x1C78 = 0x141264350 "RunEndR/L(W/B)" / "WalkEndR/L(W/B)" by the leg):
//!   sub-step 1 plays the start blend space at the yaw delta (blend 0.1) then the loop; the loop is the 2D WalkRun
//!   blend space (X = the turn, Y = the speed); a reversal over 135 degrees plays "SharpTurnL/R" (Donald's and
//!   Goofy's sets have none: they turn in the loop); the stop clip when the input ends. The velocity heading turns
//!   at m_MaxRotationSpeed.Yaw (0x1404F2590), the facing follows it at 360 deg/s (0x14045AAF0).
//!
//! PORT DESIGN (marked K1; numbers in sheets/port/party-follow.json `k1.*`):
//! - the reference speed is at least the leader's actual speed (a dodge, a glide); a cap;
//! - THE LOOP IS SPEED-MATCHED (the pad friends' dec016 loop, crate::pad): the walk and run cycles of WalkRunN / B
//!   with the lean samples on one phase whose rate makes the stride match the ground actually covered, also above
//!   the run speed (KH3 stretches nothing: its feet slide at a catch-up speed);
//! - NO CYCLE WITHOUT MOVEMENT: under 14 cm/s actual the stand clip shows; blocked (the body covers < 10 % of what the
//!   movement asks) -> a step aside, then the MoveTo fails and he stands (a moment's memory keeps him from running
//!   into the same wall again at once);
//! - the start clip's root motion (the blend space's samples) turns and moves him; the stop clip below 200 cm/s is
//!   left out;
//! - standing, a focus more than 30 degrees off plays the turn clip scaled to the delta (KH3's Idle rotates the actor
//!   at 180 deg/s under the idle clip); nearer is not turned to.

use crate::command::{LocomotionDef, MemberCommand};
use crate::host::PartyHost;
use crate::pad::{self, LocoPose};
use crate::pawn::{forward, horizontal, turn_rate, wrap180, yaw_of, NpcPawn, StepCtx, TurnKind};
use crate::states::NpcStateId;
use crate::tables;
use glam::{Vec2, Vec3};

include!(concat!(env!("KH_PARTY_GEN_DIR"), "/party_follow.rs"));

/// The sheet's rows (sheets/port/party-follow.json).
pub const K: PartyFollowParams = PARTY_FOLLOW_PARAMS;

/// The AI locomotion's run of one member.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct AiLoco {
    /// 0 the loop, 2 the start clip (UTresAICharState_Run sub-steps), 3 the stop clip (played in Idle).
    pub sub: u8,
    /// The speed-matched loop's phase (0..1) and the pose the host draws (None = the slot's clip).
    pub phase: f32,
    pub pose: Option<LocoPose>,
    /// The start clip's axis and the root time already applied (normalised).
    pub start_axis: f32,
    pub root_t: f32,
    /// The velocity heading (KH3: turns at m_MaxRotationSpeed.Yaw), deg.
    pub heading: f32,
    /// The last follow speed (cm/s) and whether the follow rule ran.
    pub follow_speed: f32,
    pub following: bool,
    /// The stand clip shows (under k1.stand_speed for k1.stand_after).
    pub standing: bool,
    pub slow_for: f32,
    /// Blocked: seconds; the step aside (time left, direction); the memory after a give-up (time left, direction).
    pub blocked: f32,
    pub side: f32,
    pub side_dir: Vec3,
    pub memory: f32,
    pub memory_dir: Vec3,
    /// The idle turn: (delta, the clip's total yaw, the fraction applied).
    pub idle_turn: Option<(f32, f32, f32)>,
    /// KH3's stop at the path's end (FollowPlayer tick 0x141215A00): the speed easing to m_MinSpeed, and its rate.
    pub decel_v: f32,
    pub decel_a: f32,
    /// The stop just ended: the arrival is taken (wherever the goal is now).
    pub decel_done: bool,
    /// (K1) the hold after an arrival behind a running leader: seconds left (the command is finished).
    pub hold: f32,
    /// (SM) the smoothed run: the speed the movement is given (cm/s) and its rate (cm/s2) (`sm_speed_to`).
    pub sm_speed: f32,
    pub sm_accel: f32,
    /// (SM) the velocity heading's turn rate (deg/s) of the critically damped steering (`sm_turn`).
    pub sm_turn_rate: f32,
    /// (SM) in the camera's view, eased (1 in view .. 0 out; None = not measured yet): the follow rate's 1.1 / 2.5.
    pub sm_view: Option<f32>,
    /// (FOL2) the no-progress guard: the goal the MoveTo heads for, the nearest it came (cm), seconds since it last came nearer.
    pub fol_goal: Vec3,
    pub fol_best: f32,
    pub fol_since: f32,
    /// (FOL2) the reaction delay left before a start behind a leader who just set off (s).
    pub fol_delay: f32,
    /// (FOL2 r2) the deceleration allowed now beyond KH3's (cm/s2; 0 = KH3's): the leader has stopped dead.
    pub fol_brake: f32,
    /// (FOL2 r2) the speed factor of the order hand-off (a mate just ahead in his way: 1 = none).
    pub fol_yield: f32,
    /// (FOL2 r2) the leader's speed last step (cm/s): his deceleration.
    pub fol_lead_v: f32,
    /// (FOL2 r2) seconds since the leader last ran faster than the hold speed.
    pub fol_ran: f32,
    /// (FOL2 r2) seconds since the MoveTo's goal last moved (the AI refreshes a moving goal every 0.1 s: a staircase).
    pub fol_goal_age: f32,
    pub fol_goal_last: Vec3,
    /// (FOL2 r2) the mates' push in his steering, smoothed (sideways only).
    pub fol_push: Vec3,
    /// (FOL2 r3) the MoveTo's goal in the running leader's frame (goal - leader at the refresh): the live goal travels with
    /// him each fixed step (None = the AI's point as it is).
    pub fol_rel: Option<Vec3>,
    /// (FOL2 r3) the leader's way when the live goal was taken (unit): the offset turns with him from it.
    pub fol_rel_dir: Vec3,
    /// (FOL2 r3) the leader's feet last step and his measured velocity (cm/s): what he covers, not what his movement asks (a
    /// leader running into a wall asks 980 cm/s and covers nothing).
    pub fol_lead_p: Option<Vec3>,
    pub fol_lead_vel: Vec3,
}

// (FOL2) THE NATURAL FOLLOW (OUR DESIGN unless marked; the user, 2026-10-20: "when you stop they will walk around you
// sometimes which is correct, but then they will start quickly running between two points ... Make their movements and
// "feel" of following you much more natural"). Measured causes (build/sora_tmp/party/IMPL_FOL2_STATE.md):
// - every Move1 MoveTo begun standing played RunStartN, whose root motion carries him at ~600 cm/s, also for the 2 m walk back
//   to his spot after KH3's random walk round the leader (walk out, sprint back);
// - the leader's AnalogInputModifier was read as 1 while he stands (KH3 / UE: the input's magnitude, 0 standing), so a friend
//   200-300 cm from a standing leader was given 450-680 cm/s;
// - a MoveTo toward a spot he cannot reach (behind a wall: the EQS's spot was accepted when the cast budget ran out) slid him
//   along the wall and back for ever: the blocked guard counts speed, not progress.
/// (FOL2) THE SWITCH (on by default): on = the natural follow below; off = the follow as before FOL2 (FINAL15). The host sets it every frame
/// (kh-er: ctl `party.follow2`); tests set it themselves.
pub static FOL2_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
/// (FOL2) The switch now.
pub fn fol2() -> bool {
    FOL2_ON.load(std::sync::atomic::Ordering::Relaxed)
}
/// (FOL2) A start from standing plays the run start clip only when the follow speed is a run (cm/s) ...
pub const FOL2_RUN_START_SPEED: f32 = 400.0;
/// (FOL2) ... and the goal is at least this far (cm); else he sets off walking (the speed spring from 0).
pub const FOL2_RUN_START_DIST: f32 = 300.0;
/// (FOL2) A walking start / a slow reversal more than this far off (deg) turns on the spot first (the turn clip, as the idle turn).
pub const FOL2_PRETURN_MIN: f32 = 60.0;
/// (FOL2) Not nearer to an unmoving goal by FOL2_PROGRESS_CM within this (s): the MoveTo ends (KH3's MoveTo fails without a path).
pub const FOL2_NO_PROGRESS: f32 = 1.2;
pub const FOL2_PROGRESS_CM: f32 = 20.0;
/// (FOL2) The goal moved more than this (cm): the progress guard starts again.
pub const FOL2_GOAL_MOVED: f32 = 60.0;
/// (FOL2) Ending without progress within this of the goal counts as arrived (cm), farther as failed.
pub const FOL2_NEAR_ENOUGH: f32 = 200.0;
/// (FOL2) The reaction delay of a friend when the leader sets off: base + step x his place in the roster + a fixed per-member
/// jitter (s): the friends do not start in lockstep.
pub const FOL2_REACT_BASE: f32 = 0.08;
pub const FOL2_REACT_STEP: f32 = 0.14;
pub const FOL2_REACT_JITTER: f32 = 0.06;
/// (FOL2) The heading snap for a near goal behind him (K1) only above this speed (cm/s); slower he turns in an arc.
pub const FOL2_NEAR_SNAP_SPEED: f32 = 250.0;
/// (FOL2) Formation behind a moving leader: within this of the goal (cm) the speed is his plus the gap along his way over
/// FOL2_GAP_TIME (s); a goal run past is approached along his way, its sideways offset over FOL2_LATERAL_CM.
pub const FOL2_FORM_RANGE: f32 = 600.0;
/// (FOL2) The leader's analog modifier is full from this share of his max speed on (below it: his speed / that).
pub const FOL2_ANALOG_FULL: f32 = 0.85;
pub const FOL2_GAP_TIME: f32 = 0.8;
pub const FOL2_LATERAL_CM: f32 = 200.0;
/// (FOL2) A goal less than this far ahead along the leader's way is approached along his way (cm).
pub const FOL2_ALONG_AHEAD: f32 = 100.0;
/// (FOL2 r2) Behind a leader who has stopped (or brakes hard) the friend may brake this hard (cm/s2; KH3's 0.5 s from a sprint
/// is ~1700, which carried him 1-2 m past his spot): from a sprint he stops in about 2 m.
pub const FOL2_STOP_DECEL: f32 = 2600.0;
/// (FOL2 r2) The move's end eases on below m_MinSpeed down to this (cm/s) over FOL2_WALK_STOP_TIME (s): no one-step stop.
pub const FOL2_STOP_FLOOR: f32 = 25.0;
pub const FOL2_WALK_STOP_TIME: f32 = 0.2;
/// (FOL2 r2) Closing on a running leader from far behind (KH3's 2.5x out of view), the speed above his is at most what this
/// deceleration (cm/s2) can take off before the follow distance: the catch-up eases off before he comes back into view.
pub const FOL2_CLOSE_DECEL: f32 = 1400.0;
/// (FOL2 r2) The leader's own deceleration that counts as stopping (cm/s2), ahead of his speed falling under the hold speed.
pub const FOL2_LEADER_BRAKING: f32 = 1500.0;
/// (FOL2 r2) Mates nearer than both radii + this push each other apart in their steering (cm), with this gain.
pub const FOL2_SEP_MARGIN: f32 = 60.0;
pub const FOL2_SEP_GAIN: f32 = 1.0;
pub const FOL2_SEP_TIME: f32 = 0.25;
/// (FOL2 r2) The order hand-off: a mate of a lower id within this (cm, past both radii) ahead in his way slows him (to at least
/// FOL2_YIELD_MIN of his speed) so the two go through a narrow place one after the other.
pub const FOL2_YIELD_RANGE: f32 = 120.0;
pub const FOL2_YIELD_MIN: f32 = 0.45;

/// (FOL2 r3) The MoveTo goal behind a running leader travels with him between the AI's 0.1 s refreshes (`live_goal`).
pub const FOL2_LIVE_GOAL: bool = true;
/// (FOL2 r3) ... only for a spot at least this far behind him along his way (cm).
pub const FOL2_LIVE_BEHIND: f32 = 60.0;

/// (FOL2 r3) THE LIVE GOAL: the AI refreshes its follow point every 0.1 s, so behind a running leader the point the MoveTo heads
/// for lagged him by up to a refresh; the friend "reached" it, KH3's path-end deceleration began, and the next refresh
/// cancelled it (a speed sawtooth). The goal is kept as an offset from the leader taken at the refresh and placed again at
/// every fixed step. Not while a waypoint steers him (the host's route is made for the AI's point). `p` = the AI's point.
pub fn live_goal(pw: &mut NpcPawn, cx: &StepCtx, p: Vec3) -> Vec3 {
    match pw.loco.fol_rel {
        Some(rel) if fol2() && pw.steer.is_none() => {
            // the offset turns with the way the leader runs (his frame): round a curve the spot stays behind him
            let lv = horizontal(cx.sora.velocity);
            let ls = lv.length();
            let rel = if ls > 1.0 && pw.loco.fol_rel_dir.length_squared() > 0.5 {
                let a = pw.loco.fol_rel_dir;
                let b = lv / ls;
                let (s, c) = (a.x * b.y - a.y * b.x, a.x * b.x + a.y * b.y);
                Vec3::new(rel.x * c - rel.y * s, rel.x * s + rel.y * c, rel.z)
            } else {
                rel
            };
            // kept back by the leader's stopping distance at FOL2_STOP_DECEL: if he stops dead, the friend can brake onto his spot
            // (station-keeping exactly on it left no room: he ran up to the leader)
            let back = if ls > 1.0 { lv / ls * (ls * ls / (2.0 * FOL2_STOP_DECEL)) } else { Vec3::ZERO };
            let g = cx.sora.position + rel - back;
            Vec3::new(g.x, g.y, p.z)
        }
        _ => p,
    }
}

/// (FOL2 r3) The leader's measured velocity this step (his feet's displacement; the first step: his movement's velocity), at most
/// what his movement asks.
pub fn fol2_measure_leader(pw: &mut NpcPawn, cx: &StepCtx) {
    let asked = horizontal(cx.sora.velocity);
    let p = cx.sora.position;
    let v = match pw.loco.fol_lead_p {
        Some(q) if cx.dt > 0.0 && horizontal(p - q).length() < 200.0 => horizontal(p - q) / cx.dt,
        _ => asked,
    };
    pw.loco.fol_lead_p = Some(p);
    pw.loco.fol_lead_vel = if v.length() > asked.length() { asked } else { v };
}

/// (FOL2 r3) Keeping station: with the live goal behind a running leader the MoveTo is not "arrived" (the friend holds his
/// place at the leader's speed; K1's hold and KH3's path-end deceleration are for a goal that stays put).
pub fn keeps_station(pw: &NpcPawn, cx: &StepCtx) -> bool {

    let _ = cx;
    fol2() && pw.loco.fol_rel.is_some() && pw.steer.is_none() && horizontal(cx.sora.velocity).length() > K.k1_hold_leader_speed
}

/// (FOL2) The reaction delay of member `id` (roster-driven: any member id).
pub fn fol2_react(id: u32) -> f32 {
    let rank = (id % 4) as f32;
    let jitter = ((id.wrapping_mul(2654435761) >> 16) & 0xff) as f32 / 255.0;
    FOL2_REACT_BASE + FOL2_REACT_STEP * rank + FOL2_REACT_JITTER * jitter
}

/// (FOL2) The turn clip on the spot toward `delta` (deg), as K1's idle turn (`idle_tick`); false = no turn clip with root yaw.
fn fol2_turn_clip(pw: &mut NpcPawn, delta: f32) -> bool {
    let name = if pw.in_battle { "TurnB" } else { "TurnN" };
    let clip = tables::clip(pw.chara, name);
    let clip_yaw = clip.root_at(clip.length).yaw;
    if clip_yaw.abs() < 1.0 {
        return false;
    }
    pw.play(name, 0.15);
    pw.anim.axis = delta;
    if delta.abs() < 90.0 {
        pw.anim.speed = K.k1_idle_turn_rate;
    }
    pw.loco.idle_turn = Some((delta.clamp(-clip_yaw.abs(), clip_yaw.abs()), clip_yaw, 0.0));
    true
}

/// (FOL2) The turn clip's progress (K1's idle-turn rule); true when it has ended.
fn fol2_turn_step(pw: &mut NpcPawn) -> bool {
    let Some((delta, clip_yaw, applied)) = pw.loco.idle_turn else { return true };
    let len = pw.anim.clip.length.max(1e-4);
    let f = if pw.anim.finished { 1.0 } else { (pw.anim.clip.root_at(pw.anim.time.min(len)).yaw / clip_yaw).clamp(0.0, 1.0) };
    if f > applied {
        pw.yaw = wrap180(pw.yaw + delta * (f - applied));
    }
    pw.loco.idle_turn = Some((delta, clip_yaw, f.max(applied)));
    if pw.anim.finished || !pw.anim.clip.name.starts_with("Turn") || f >= 0.999 {
        pw.loco.idle_turn = None;
        pw.anim.speed = 1.0;
        return true;
    }
    false
}

// (SM) THE SMOOTHED RUN (OUR DESIGN; the user: "make sure their movement is smooth and natural looking"). The port set the
// movement's speed and the velocity heading anew every step: KH3's follow rule jumps 10 % at m_RangeSamePlayerSpeed
// (distance / 300 inside, the view rate 1.1 outside) and 1.1 <-> 2.5 when the friend crosses the frustum's edge, so at
// the formation distance the speed chattered 978 <-> 1078 every step (up to 390 swings / 10 s); the heading turned at 180 deg/s
// toward each new MoveTo goal and stopped dead (a kink every EQS refresh). KH3 itself eases the follow speed (its
// definition's acceleration +0x350 = follow speed - m_MinSpeed, else m_Acceleration 500: `follow.accel_margin`,
// `follow.acceleration`) and moves its pawns with UE's acceleration; the port's movement has none (instant).
// - the speed: a critically damped spring toward the follow rule's speed (SM_SPEED_OMEGA), its acceleration at most KH3's
//   (+0x350: follow speed - m_MinSpeed, at least m_Acceleration 500) and its deceleration at most KH3's path-end rate
//   ((speed - m_MinSpeed) / m_DecelerationTime);
// - the follow rate: continuous across m_RangeSamePlayerSpeed (KH3's own slope distance / 300 goes on until it meets
//   the view rate) and across the frustum's edge (the view eased over SM_VIEW_TIME s);
// - the heading: a critically damped turn (SM_TURN_OMEGA) toward the waypoint / goal, its rate at most KH3's
//   m_MaxRotationSpeed.Yaw 180 deg/s; the turn slow-down eased in over 20 .. 40 degrees (K1's cos rule beyond).
/// (SM) The speed spring's angular frequency (1/s): settles in about 0.3 s, without overshoot (fast enough not to make KH3.s distance rule overshoot the formation gap).
pub const SM_SPEED_OMEGA: f32 = 14.0;
/// (SM) The heading spring's angular frequency (1/s): a goal moved by a few degrees is taken over about 0.3 s.
pub const SM_TURN_OMEGA: f32 = 11.0;
/// (SM) The time the follow rate takes from the in-view rate to the out-of-view rate and back (s).
pub const SM_VIEW_TIME: f32 = 0.4;

/// (SM) The heading `want_yaw` taken by the critically damped turn, at most `max` deg/s.
pub fn sm_turn(loco: &mut AiLoco, want_yaw: f32, dt: f32, max: f32) {
    let err = wrap180(want_yaw - loco.heading);
    let w = SM_TURN_OMEGA;
    let acc = w * w * err - 2.0 * w * loco.sm_turn_rate;
    let mut rate = (loco.sm_turn_rate + acc * dt).clamp(-max, max);
    // never past the goal in one step (a small error is taken, not overshot)
    if (rate * dt).abs() > err.abs() && rate.signum() == err.signum() {
        rate = err / dt.max(1e-4);
    }
    loco.sm_turn_rate = rate;
    loco.heading = wrap180(loco.heading + rate * dt);
}

/// (SM) The movement's speed eased toward `target` (cm/s): KH3's acceleration up, its path-end deceleration down.
pub fn sm_speed_to(loco: &mut AiLoco, target: f32, min: f32, dt: f32) -> f32 {
    let up = (target - min).max(K.follow_acceleration);
    let down = ((loco.sm_speed.max(target) - min) / K.follow_decel_time.max(0.05)).max(K.follow_acceleration);
    let down = down.max(loco.fol_brake); // (FOL2 r2) the stop behind a leader who stopped dead
    let w = SM_SPEED_OMEGA;
    // x'' = w^2 (target - x) - 2 w x' (x = the speed, x' = its rate), the rate within KH3's acceleration / deceleration
    let jerk = w * w * (target - loco.sm_speed) - 2.0 * w * loco.sm_accel;
    loco.sm_accel = (loco.sm_accel + jerk * dt).clamp(-down, up);
    let next = loco.sm_speed + loco.sm_accel * dt;
    // the spring never crosses the target (critically damped: it settles on it)
    loco.sm_speed = if (loco.sm_speed - target) * (next - target) < 0.0 { loco.sm_accel = 0.0; target } else { next.max(0.0) };
    loco.sm_speed
}

/// (SM) The run's speed spring and heading turn rate taken over from what the body does now (a start clip's end, a
/// stop, a new start, the deceleration).
pub fn sm_sync(loco: &mut AiLoco, speed: f32) {
    loco.sm_speed = speed.max(0.0);
    loco.sm_accel = 0.0;
    loco.sm_turn_rate = 0.0;
}

/// (SM) The eased view (1 in view .. 0 out) after this step.
fn sm_view_step(pw: &mut NpcPawn, cx: &StepCtx) {
    let lv = cx.sora;
    let seen = lv.camera.map_or(true, |c| c.sees(pw.capsule_centre(), pw.params.capsule_radius + pw.params.capsule_half_height));
    let to = if seen { 1.0 } else { 0.0 };
    let v = pw.loco.sm_view.unwrap_or(to);
    let step = cx.dt / SM_VIEW_TIME.max(1e-3);
    pw.loco.sm_view = Some(if to > v { (v + step).min(to) } else { (v - step).max(to) });
}

/// KH3's follow speed (0x1412196C0) for a Move1 MoveTo, or None when the rule does not run (FUN_141214F90: in
/// battle without the battle-follow mode).
pub fn follow_speed(pw: &NpcPawn, cx: &StepCtx) -> Option<f32> {
    follow_speed_with(pw, cx, None, false)
}

/// (SM) KH3's follow speed with the port's eased edges (`view`: the eased in-view 1 .. 0, None = the frustum test now;
/// `continuous`: beyond m_RangeSamePlayerSpeed distance / range goes on until it meets the view rate; false = KH3's step).
pub fn follow_speed_with(pw: &NpcPawn, cx: &StepCtx, view: Option<f32>, continuous: bool) -> Option<f32> {
    if pw.in_battle && !pw.battle_follow {
        return None;
    }
    let lv = cx.sora;
    let min = tables::cparam(pw.chara, "loco.move1.min_speed");
    let dist = horizontal(lv.position - pw.position()).length();
    // +0x35C: in the drawn camera's view (no camera known: in view, the lower factor)
    let in_view = view.unwrap_or_else(|| if lv.camera.map_or(true, |c| c.sees(pw.capsule_centre(), pw.params.capsule_radius + pw.params.capsule_half_height)) { 1.0 } else { 0.0 });
    let mut rate = K.follow_out_view_rate + (K.follow_in_view_rate - K.follow_out_view_rate) * in_view.clamp(0.0, 1.0);
    let range = K.follow_same_speed_range;
    if dist <= range {
        rate = dist / range;
    } else if continuous {
        // (SM) KH3's own slope goes on (distance / range) until it meets the view rate: no step at the range
        rate = rate.min(dist / range).max(1.0);
    }
    let actual = horizontal(lv.velocity).length();
    let max = if lv.max_speed > 0.0 { lv.max_speed } else { actual };
    // (FOL2) the leader's AnalogInputModifier is the input's magnitude (UE, [inf] for KH3's vt+0xC08): 0 while he stands,
    // his walk share while he walks; the port reads it as his actual speed / his max (at most the host's walk rate)
    // (full from FOL2_ANALOG_FULL of his max on: his run / sprint keeps the steady reference, no stride jitter in it)
    let analog = if fol2() && max > 1.0 { lv.analog.min((actual / (FOL2_ANALOG_FULL * max)).min(1.0)) } else { lv.analog };
    let mut reference = if dist < K.follow_near_analog { max * analog } else { max };
    // (K1) at least what he really covers
    if K.k1_use_leader_velocity != 0.0 {
        reference = reference.max(actual);
    }
    Some((reference * rate).max(min).min(K.k1_speed_cap))
}

/// MoveTo's Begin in the port's Move state (pawn `begin_command`): the start clip from standing (sub-step 1), or a
/// refused move into the side he just gave up on. Returns false when the command was refused (it is failed).
pub fn begin_move(pw: &mut NpcPawn, cx: &StepCtx, host: &PartyHost, was_moving: bool) -> bool {
    let Some(MemberCommand::MoveTo { goal, locomotion, walk, .. }) = pw.command_now() else { return true };
    let Some(p) = pw.goal_point(goal, cx, host) else { return true };
    // (ER S1's waypoint, as the Move state reads it: the start turns toward the way he will run)
    let p = match pw.steer {
        Some((g, via)) if horizontal(g - p).length() <= crate::pawn::STEER_GOAL_SLACK && horizontal(via - pw.position()).length() > 1.0 => via,
        _ => p,
    };
    let d = horizontal(p - pw.position());
    if pw.loco.memory > 0.0 && d.length() > 1.0 && d.normalize().dot(pw.loco.memory_dir) > K.k1_blocked_cone.to_radians().cos() {
        pw.finish_command_pub(false);
        if pw.state == NpcStateId::Move {
            pw.to_idle();
        }
        return false;
    }
    pw.loco.decel_v = 0.0;
    pw.loco.decel_done = false;
    pw.loco.hold = 0.0;
    // (FOL2) the progress guard: a new goal (or a new move) starts it again
    let goal_now = pw.goal_point(goal, cx, host).unwrap_or(p);
    if horizontal(goal_now - pw.loco.fol_goal_last).length() > 1.0 {
        pw.loco.fol_goal_last = goal_now;
        pw.loco.fol_goal_age = 0.0;
        // (FOL2 r3) refreshed behind a running leader: the goal from now on travels with him
        let lead_runs = horizontal(cx.sora.velocity).length() > K.k1_hold_leader_speed;
        // (only a spot behind him: a spot the EQS put beside or ahead of him stays the AI's point)
        let rel = goal_now - cx.sora.position;
        let lvh = horizontal(cx.sora.velocity);
        let behind = lvh.length() > 1.0 && horizontal(rel).dot(lvh / lvh.length()) < -FOL2_LIVE_BEHIND;
        pw.loco.fol_rel = (fol2() && FOL2_LIVE_GOAL && lead_runs && behind && locomotion == LocomotionDef::Move1 && !walk).then_some(rel);
        pw.loco.fol_rel_dir = lvh.normalize_or_zero();
    }
    if !was_moving || horizontal(goal_now - pw.loco.fol_goal).length() > FOL2_GOAL_MOVED {
        pw.loco.fol_goal = goal_now;
        pw.loco.fol_best = horizontal(goal_now - pw.position()).length();
        pw.loco.fol_since = 0.0;
    }
    if was_moving {
        return true;
    }
    pw.loco.blocked = 0.0;
    pw.loco.side = 0.0;
    pw.loco.standing = false;
    pw.loco.slow_for = 0.0;
    pw.loco.idle_turn = None;
    pw.loco.heading = pw.yaw;
    sm_sync(&mut pw.loco, horizontal(pw.cmc.velocity).length()); // (SM) the run's springs start from what he does
    pw.loco.phase = 0.0;
    pw.loco.pose = None;
    pw.loco.sub = 0;
    let slow = walk || locomotion == LocomotionDef::MoveSlow;
    // (FOL2) the reaction delay: the leader just set off and he is near and had settled (stood a second): he stands a moment
    // longer (sub-step 5), then sets off (`start_from_standing`); a re-start while the party runs takes none
    let lv = horizontal(cx.sora.velocity).length();
    pw.loco.fol_delay = if fol2() && !slow && pw.motionless >= 1.0 && lv > 60.0 && horizontal(cx.sora.position - pw.position()).length() < 700.0 { fol2_react(pw.id.0 as u32) } else { 0.0 };
    if pw.loco.fol_delay > 0.0 {
        pw.loco.standing = true;
        let idle = if pw.in_battle { "IdleB" } else { "IdleN" };
        if pw.anim.clip.name != idle {
            pw.play(idle, 0.15);
        }
        pw.loco.sub = 5;
        return true;
    }
    start_from_standing(pw, cx, locomotion, walk, d);
    true
}

/// The start from standing: KH3's run start clip (K1), or (FOL2) for a walk the loop from standing after a turn on the spot.
fn start_from_standing(pw: &mut NpcPawn, cx: &StepCtx, locomotion: LocomotionDef, walk: bool, d: Vec3) {
    let slow = walk || locomotion == LocomotionDef::MoveSlow;
    let battle = pad::battle_set(pw);
    // (FOL2) the run start only for a run: the follow speed now a run and the goal far enough; a walk sets off in the loop
    // (the speed spring from standing), turning on the spot first when the goal is well off his facing
    let max = tables::cparam(pw.chara, "loco.move1.max_speed");
    let begin_speed = if slow { 0.0 } else if locomotion == LocomotionDef::Move1 { follow_speed_with(pw, cx, pw.loco.sm_view, true).unwrap_or(max) } else { max };
    let runs = !fol2() || (begin_speed >= FOL2_RUN_START_SPEED && d.length() >= FOL2_RUN_START_DIST);
    if !runs && d.length() > 1.0 {
        let delta = wrap180(yaw_of(d) - pw.yaw);
        if delta.abs() >= FOL2_PRETURN_MIN && horizontal(pw.cmc.velocity).length() < 50.0 && fol2_turn_clip(pw, delta) {
            pw.loco.sub = 4;
        }
        return;
    }
    if K.k1_start_clip != 0.0 && !slow && d.length() > 1.0 && pad::start_length(pw.chara, battle, 0.0).is_some() && horizontal(pw.cmc.velocity).length() < 50.0 {
        let delta = wrap180(yaw_of(d) - pw.yaw);
        pw.anim.play(pad::start_clip(pw.chara, battle, delta), -1.0);
        pw.anim.axis = delta;
        pw.anim.axis_y = 0.0;
        pw.loco.start_axis = delta;
        pw.loco.root_t = 0.0;
        pw.loco.heading = yaw_of(d);
        pw.loco.sub = 2;
    }
}

/// The Move state's movement after KH3's probes: speed, steering, the blocked guard, the clip. `p` / `d` = the point
/// he heads for (the host's waypoint or the goal) and the offset to it.
pub fn move_tick(pw: &mut NpcPawn, cx: &StepCtx, locomotion: LocomotionDef, walk: bool, d: Vec3, goal_dist: f32) {
    let dt = cx.dt;
    fol2_measure_leader(pw, cx);
    let slow = walk || locomotion == LocomotionDef::MoveSlow;
    let (min, max) = (tables::cparam(pw.chara, "loco.move1.min_speed"), tables::cparam(pw.chara, "loco.move1.max_speed"));
    // the speed: Move_Slow / walk 140; Move1 the follow rule while following, else m_MaxSpeed
    // (SM) the rule's edges eased (the view over SM_VIEW_TIME, the range continuous)
    sm_view_step(pw, cx);
    let follow = if slow || locomotion != LocomotionDef::Move1 { None } else { follow_speed_with(pw, cx, pw.loco.sm_view, true) };
    pw.loco.following = follow.is_some();
    let mut speed = if slow { min } else { follow.unwrap_or(max) };
    // KH3's stop (FollowPlayer tick 0x141215A00): toward the end of the path the speed eases from the follow speed to
    // m_MinSpeed over m_DecelerationTime (+0x350 = -(speed - min) / 0.5); the port puts it as a speed envelope on the
    // goal's distance (KH3 starts it at the path's end after a navmesh raycast FUN_143ED09B0 [inf]): he arrives at the
    // walk speed, and behind a moving Destination he settles at the distance where the envelope meets its speed
    // (K1) only while the leader is slow or standing: behind a running leader the Destination runs on (an arrival is the
    // hold, see arrive) and a Destination the EQS puts near him must not hold him to a crawl
    // (FOL2 r2) THE STOP BEHIND A LEADER WHO STOPS DEAD: from his braking (or his speed under the hold speed) on, the friend may
    // brake at FOL2_STOP_DECEL, the envelope toward where the goal will come to rest (the goal now + the leader's own stopping
    // distance at his present deceleration): he comes to rest near his spot instead of 1-2 m past it
    let lspeed = horizontal(cx.sora.velocity).length();
    // (r3: a leader whose movement asks for a run but covers nothing (into a wall) counts as stopped)
    let lcovers = if fol2() { pw.loco.fol_lead_vel.length() } else { lspeed };
    let lbrake = (pw.loco.fol_lead_v - lspeed) / dt.max(1e-4);
    pw.loco.fol_lead_v = lspeed;
    let slow_leader = lspeed.min(lcovers) <= K.k1_hold_leader_speed;
    pw.loco.fol_ran = if slow_leader { pw.loco.fol_ran + dt } else { 0.0 };
    // (only just after a run: a walking leader keeps KH3's gentle stop)
    let braking = fol2() && follow.is_some() && pw.loco.sm_speed > FOL2_RUN_START_SPEED && ((slow_leader && pw.loco.fol_ran < 1.5) || lbrake > FOL2_LEADER_BRAKING);
    // (latched through the stop: until he is down to the walk or the leader runs again)
    let braking = braking || (fol2() && pw.loco.fol_brake > 0.0 && (slow_leader || lbrake > 0.0) && pw.loco.sm_speed > min + 10.0);
    pw.loco.fol_brake = if braking { FOL2_STOP_DECEL } else { 0.0 };
    // (FOL2 r3) the leader stopping: the live goal ends (it is for a leader who runs on; the AI's point is the stop's goal, as in r2)
    if braking {
        pw.loco.fol_rel = None;
    }
    if follow.is_some() && speed > min && (slow_leader || braking) {
        let a = ((speed - min) / K.follow_decel_time.max(0.05)).max(1.0).max(pw.loco.fol_brake);
        let rest = if braking && !slow_leader { lspeed * lspeed / (2.0 * lbrake.max(1.0)) } else { 0.0 };
        // (r3: braking, a goal already behind him counts as reached: he stops at once rather than at the envelope of its distance)
        let ahead = if braking && d.length_squared() > 1e-4 && d.dot(forward(pw.loco.heading)) < 0.0 { 0.0 } else { goal_dist };
        speed = speed.min((min * min + 2.0 * a * (ahead + rest).max(0.0)).sqrt());
    }
    // (FOL2 r2) THE CATCH-UP EASES OFF: closing on a running leader from far behind (KH3's 2.5x out of view), his speed above
    // the leader's is at most what FOL2_CLOSE_DECEL takes off before KH3's follow distance (no 2000 cm/s arrival into the view)
    if fol2() && follow.is_some() && !slow_leader {
        let dl = horizontal(cx.sora.position - pw.position()).length();
        let cap = lspeed + (2.0 * FOL2_CLOSE_DECEL * (dl - K.follow_same_speed_range).max(0.0)).sqrt();
        speed = speed.min(cap.max(lspeed));
    }
    // (FOL2) IN FORMATION BEHIND A MOVING LEADER: the speed is his plus the gap to the goal along his way over FOL2_GAP_TIME
    // (at least half his, at most the follow rule's), and a goal he ran past is not turned back for: he eases off along the
    // leader's way until it comes up (KH3's distance rule alone pushed a friend whose spot is farther back than its 300 cm
    // through the spot and round again)
    let lvv = horizontal(cx.sora.velocity);
    let ls = lvv.length();
    let mut match_dir: Option<Vec3> = None;
    let mut match_w = 0.0f32;
    let mut d_form: Option<Vec3> = None;
    let straight = (d.length() - goal_dist).abs() < 1.0;
    // (r3: with the live goal also behind a walking leader: never back to it while he goes on)
    let live = pw.loco.fol_rel.is_some() && pw.steer.is_none();
    let lcov = pw.loco.fol_lead_vel.length();
    if fol2() && follow.is_some() && lcov > 0.5 * ls && (ls > K.k1_hold_leader_speed || (live && ls > 50.0)) && goal_dist < FOL2_FORM_RANGE && straight {
        let ld = lvv / ls;
        pw.loco.fol_goal_age = (pw.loco.fol_goal_age + dt).min(0.25);
        // (FOL2 r2) the goal as it stands now: it moves with the leader between the AI's 0.1 s refreshes
        let err = d.dot(ld) + if live { 0.0 } else { ls * pw.loco.fol_goal_age };
        // (FOL2 r2) and steered for there: chasing the refreshed point as it nears him swung his heading and (K1's turn slow-down)
        // his speed at every refresh
        // (r3: the live goal already moves with him; the extrapolation only for the AI's point)
        d_form = Some(if live { d } else { d + ld * ls * pw.loco.fol_goal_age });
        // only once he is level with or past it (ahead of it the follow rule stands: the EQS spot jumps at its refresh, which
        // a gap-driven speed would turn into surges)
        if err < FOL2_ALONG_AHEAD { // (FOL2 r2) from the last FOL2_ALONG_AHEAD on: continuous through level with it
            speed = speed.min((ls + err / FOL2_GAP_TIME).max(0.5 * ls));
        }
        // level with it, past it or beside it: along the leader's way, the sideways offset taken gently (no 90-degree dash)
        if err < FOL2_ALONG_AHEAD {
            let lateral = d - ld * d.dot(ld);
            match_dir = Some((ld + lateral / FOL2_LATERAL_CM).normalize_or_zero());
            // (FOL2 r2) blended in over the last FOL2_ALONG_AHEAD before level with it (no switch of the way he heads)
            match_w = ((FOL2_ALONG_AHEAD - err) / FOL2_ALONG_AHEAD).clamp(0.0, 1.0);
        }
    }
    pw.loco.follow_speed = speed;
    pw.cmc.max_walk_speed = speed;
    let actual = horizontal(pw.cmc.displacement_velocity).length();
    pw.loco.memory = (pw.loco.memory - dt).max(0.0);
    if pw.loco.sub == 2 {
        start_tick(pw, dt);
        if pw.loco.sub == 2 {
            return;
        }
    }
    // (FOL2) the turn on the spot before a walk (begin_move / a slow reversal): no movement until it ends
    if pw.loco.sub == 4 {
        pw.cmc.stop_movement();
        pw.cmc.max_walk_speed = 0.0;
        if !fol2_turn_step(pw) {
            return;
        }
        pw.loco.sub = 0;
        pw.loco.heading = pw.yaw;
        sm_sync(&mut pw.loco, 0.0);
        let idle = if pw.in_battle { "IdleB" } else { "IdleN" };
        pw.play(idle, 0.1);
        pw.loco.standing = true;
        pw.loco.slow_for = K.k1_stand_after;
    }
    // (FOL2) the reaction delay (sub-step 5): he stands a moment longer when the leader sets off, then sets off
    if pw.loco.sub == 5 {
        pw.loco.fol_delay -= dt;
        pw.cmc.stop_movement();
        pw.cmc.max_walk_speed = 0.0;
        sm_sync(&mut pw.loco, 0.0);
        if pw.loco.fol_delay > 0.0 {
            return;
        }
        pw.loco.sub = 0;
        pw.loco.fol_delay = 0.0;
        start_from_standing(pw, cx, locomotion, walk, d);
        if pw.loco.sub != 0 {
            return;
        }
    }
    // (FOL2) THE PROGRESS GUARD: no nearer to a goal that stays put for FOL2_NO_PROGRESS s (a spot behind a wall, sliding along
    // it and back): the MoveTo ends, arrived when near enough, else failed with K1's memory of that side
    // (only on the straight way to it: a waypoint round an obstacle, a step aside may lead away for a while)
    // (nor toward a goal on another level: a ledge, a ladder's top; the host's stuck fallback handles those)
    let level = (pw.loco.fol_goal.z - pw.position().z).abs() < 100.0;
    if fol2() && horizontal(cx.sora.velocity).length() <= K.k1_hold_leader_speed && straight && level && pw.loco.side <= 0.0 {
        if goal_dist < pw.loco.fol_best - FOL2_PROGRESS_CM {
            pw.loco.fol_best = goal_dist;
            pw.loco.fol_since = 0.0;
        } else {
            pw.loco.fol_since += dt;
        }
        if pw.loco.fol_since >= FOL2_NO_PROGRESS {
            pw.loco.fol_since = 0.0;
            let ok = goal_dist <= FOL2_NEAR_ENOUGH;
            if !ok {
                pw.loco.memory = K.k1_blocked_memory * 2.0;
                pw.loco.memory_dir = if d.length_squared() > 1e-4 { d.normalize() } else { forward(pw.yaw) };
            }
            pw.loco.blocked = 0.0;
            pw.loco.side = 0.0;
            pw.finish_command_pub(ok);
            stop_now(pw, actual);
            return;
        }
    } else {
        pw.loco.fol_since = 0.0;
        pw.loco.fol_best = goal_dist;
    }
    // the blocked guard (K1): asked to move, not moving
    let d_aim = d_form.unwrap_or(d); // (FOL2 r2)
    let mut want = if d_aim.length_squared() > 1e-4 { d_aim.normalize() } else { forward(pw.yaw) };
    if let Some(m) = match_dir.filter(|m| m.length_squared() > 0.5) {
        // (FOL2) past his goal behind a moving leader: along the leader's way
        let w = (want * (1.0 - match_w) + m * match_w).normalize_or_zero();
        if w.length_squared() > 0.5 {
            want = w;
        }
    }
    // (FOL2 r2) THE MATES: a soft push apart in the steering from a mate nearer than both radii + FOL2_SEP_MARGIN, and the order
    // hand-off: a mate of a lower id just ahead in his way slows him, so two friends go through a hallway's mouth one after
    // the other instead of through each other (the port has no friend-friend collision)
    pw.loco.fol_yield = 1.0;
    if fol2() {
        let me = pw.position();
        let r = pw.params.capsule_radius;
        let mut push = Vec3::ZERO;
        // (r3: the leader too, as a mate the friend never runs or slides through; no hand-off with him)

        let others = cx.members.iter().map(|m| (Some(m.0), m.1)).chain(std::iter::once((None, cx.sora.position)));
        for (oid, at) in others {
            if oid == Some(pw.id) {
                continue;
            }
            let off = horizontal(me - at);
            let dist = off.length();
            let touch = r + 40.0;
            let reach = touch + FOL2_SEP_MARGIN;
            if dist > 1.0 && dist < reach {
                push += off / dist * (1.0 - (dist - touch).max(0.0) / FOL2_SEP_MARGIN);
            }
            let lower = oid.is_none_or(|id| id.0 < pw.id.0);
            if lower && dist > 1.0 && dist < touch + FOL2_YIELD_RANGE && (-off / dist).dot(want) > 0.5 {
                let f = ((dist - touch) / FOL2_YIELD_RANGE).clamp(0.0, 1.0);
                // (r3: toward the leader down to a stop at touching)
                let floor = if oid.is_none() { 0.0 } else { FOL2_YIELD_MIN };
                pw.loco.fol_yield = pw.loco.fol_yield.min(floor + (1.0 - floor) * f);
            }
        }
        // sideways only (it never brakes or drives him: the hand-off does that) and eased over FOL2_SEP_TIME (no push-pull wobble
        // when two friends round a corner close together)
        let side = push - want * push.dot(want);
        let k = (dt / FOL2_SEP_TIME).min(1.0);
        pw.loco.fol_push += (side - pw.loco.fol_push) * k;
        if pw.loco.fol_push.length_squared() > 1e-6 {
            let w = (want + pw.loco.fol_push * FOL2_SEP_GAIN).normalize_or_zero();
            if w.length_squared() > 0.5 {
                want = w;
            }
        }
    }
    // the movement was asked for the speed last tick (the input is written every tick; its acceleration is instant)
    let asked = if pw.cmc.max_walk_speed > 0.0 && pw.state == NpcStateId::Move { speed } else { 0.0 };
    if pw.loco.side > 0.0 {
        pw.loco.side -= dt;
        want = pw.loco.side_dir;
    }
    if asked > 0.3 * speed && actual < K.k1_blocked_frac * speed {
        pw.loco.blocked += dt;
    } else if actual >= K.k1_blocked_frac * speed {
        pw.loco.blocked = (pw.loco.blocked - 2.0 * dt).max(0.0);
    }
    // (FOL2 r3) not blocked when what stopped him is behind his way to the goal (he ran into a wall past it and only has to turn
    // round): K1's step aside then slid him along the wall, into the leader
    if fol2() && pw.loco.side <= 0.0 && pw.cmc.last_block.is_some_and(|(h, _)| horizontal(h.normal).normalize_or_zero().dot(want) > 0.2) {
        pw.loco.blocked = 0.0;
    }
    if pw.loco.blocked >= K.k1_blocked_give_up {
        // give up: the MoveTo fails, he stands; a MoveTo the same way fails for a moment
        pw.loco.memory = K.k1_blocked_memory;
        pw.loco.memory_dir = if d.length_squared() > 1e-4 { d.normalize() } else { forward(pw.yaw) };
        pw.loco.blocked = 0.0;
        pw.loco.side = 0.0;
        pw.cmc.stop_movement();
        pw.finish_command_pub(false);
        pw.to_idle();
        return;
    }
    if pw.loco.blocked >= K.k1_blocked_after && pw.loco.side <= 0.0 {
        // step aside: along what blocks him, else square to it toward the leader
        let n = pw.cmc.last_block.map(|(h, _)| horizontal(h.normal).normalize_or_zero()).unwrap_or(Vec3::ZERO);
        let along = want - n * want.dot(n);
        let to_leader = horizontal(cx.sora.position - pw.position()).normalize_or_zero();
        let side = if along.length() > 0.3 {
            along.normalize()
        } else {
            let perp = Vec3::new(-n.y, n.x, 0.0);
            if perp.dot(to_leader) >= 0.0 { perp } else { -perp }
        };
        if side.length_squared() > 0.5 {
            pw.loco.side = K.k1_sidestep_time;
            pw.loco.side_dir = side;
            want = side;
        }
    }
    // KH3's steering: the velocity heading turns at m_MaxRotationSpeed.Yaw (0x1404F2590), the facing follows it at
    // 360 deg/s (0x14045AAF0); the input is the heading (he moves where he runs, not sideways)
    let yaw0 = pw.yaw;
    let want_yaw = yaw_of(want);
    let hstep = turn_rate(TurnKind::Locomotion) * dt;
    let hd = wrap180(want_yaw - pw.loco.heading);
    // a goal close beside or behind him: the heading may not orbit it (port: the turn is taken at once)
    // (FOL2) only at a run: walking he turns toward it in an arc (the damped turn)
    let near = d.length() < 150.0 && hd.abs() > 90.0 && (!fol2() || (pw.loco.sm_speed > FOL2_NEAR_SNAP_SPEED && match_dir.is_none()));
    // (FOL2) a reversal at a walk: standing nearly still -> the turn on the spot; else the damped turn (an arc), no run start
    // (FOL2) the run-start reversal only toward a far goal at a run; nearer or slower: the arc, or the turn on the spot
    let walking = fol2() && (speed < FOL2_RUN_START_SPEED || goal_dist < FOL2_RUN_START_DIST);
    if walking && hd.abs() >= 135.0 && pw.loco.side <= 0.0 {
        if actual < 40.0 && fol2_turn_clip(pw, wrap180(want_yaw - pw.yaw)) {
            pw.cmc.stop_movement();
            pw.loco.pose = None;
            pw.loco.sub = 4;
            return;
        }
    }
    // (K1) a reversal: KH3's Run plays "SharpTurnL/R" over 135 degrees (0x140442430), which Donald's and Goofy's sets
    // do not have; the port turns round with the start blend space at the delta (its 180-degree samples), as the
    // player's turn-around (0x1413AF8E0) and the pad friends' do, instead of a wide arc at the catch-up speed
    let battle = pad::battle_set(pw);
    if !near && !walking && hd.abs() >= 135.0 && pw.loco.side <= 0.0 && K.k1_start_clip != 0.0 && pad::start_length(pw.chara, battle, 0.0).is_some() {
        let delta = wrap180(want_yaw - pw.yaw);
        pw.cmc.stop_movement();
        pw.loco.pose = None;
        pw.anim.play(pad::start_clip(pw.chara, battle, delta), -1.0);
        pw.anim.axis = delta;
        pw.anim.axis_y = 0.0;
        pw.loco.start_axis = delta;
        pw.loco.root_t = 0.0;
        pw.loco.heading = want_yaw;
        sm_sync(&mut pw.loco, 0.0); // (SM)
        pw.loco.sub = 2;
        return;
    }
    // (K1) slower while the heading swings (KH3 keeps the speed: at a catch-up speed its 180 deg/s heading orbits)
    // (SM) eased in between 20 and 40 degrees (K1's cos rule from 40 on: no 13 % step at 30 degrees), the speed then
    // through the speed spring (sm_speed_to)
    let mut target = speed;
    let ease = ((hd.abs() - 20.0) / 20.0).clamp(0.0, 1.0);
    if ease > 0.0 {
        let k1 = (speed * hd.abs().min(90.0).to_radians().cos().max(0.3)).max(min.min(speed));
        target = speed + (k1 - speed) * ease * ease * (3.0 - 2.0 * ease);
    }
    if fol2() {
        let floor = if pw.loco.fol_yield < FOL2_YIELD_MIN { 0.0 } else { min.min(target) }; // (r3: the leader ahead: down to a stop)
        target = (target * pw.loco.fol_yield).max(floor); // (FOL2 r2) the order hand-off
    }
    pw.cmc.max_walk_speed = sm_speed_to(&mut pw.loco, target, min, dt);
    // (SM) the heading by the critically damped turn (at most KH3's 180 deg/s); a near goal behind him at once (K1)
    if near {
        pw.loco.heading = want_yaw;
        pw.loco.sm_turn_rate = 0.0;
    } else {
        sm_turn(&mut pw.loco, want_yaw, dt, hstep / dt.max(1e-6));
    }
    let fd = wrap180(pw.loco.heading - pw.yaw);
    let fstep = 360.0 * dt;
    pw.yaw = wrap180(pw.yaw + fd.clamp(-fstep, fstep));
    let h = forward(pw.loco.heading);
    pw.cmc.control_input = h;
    loop_pose(pw, dt, yaw0, actual, min, max);
}

/// The start clip (sub-step 2): the blended samples' root motion as the pad's start (crate::pad), its end -> the loop.
fn start_tick(pw: &mut NpcPawn, dt: f32) {
    // the start clip was replaced (a stop, a hold, another clip): the loop takes over
    if !pw.anim.clip.name.starts_with("RunStart") {
        pw.loco.sub = 0;
        pw.loco.heading = pw.yaw;
        sm_sync(&mut pw.loco, horizontal(pw.cmc.displacement_velocity).length()); // (SM)
        return;
    }
    let battle = pad::battle_set(pw);
    let len = pw.anim.clip.length.max(1e-4);
    let t1 = (pw.anim.time / len).clamp(0.0, 1.0);
    let a = pad::start_root(pw.chara, battle, pw.loco.start_axis, pw.loco.root_t);
    let b = pad::start_root(pw.chara, battle, pw.loco.start_axis, t1);
    pw.loco.root_t = t1;
    let (s, c) = (-a.2).to_radians().sin_cos();
    let (dx, dy) = ((b.0 - a.0) * c - (b.1 - a.1) * s, (b.0 - a.0) * s + (b.1 - a.1) * c);
    let f = forward(pw.yaw);
    let right = Vec3::new(-f.y, f.x, 0.0);
    let world = f * dx + right * dy;
    if dt > 0.0 {
        pw.cmc.direct_xy = Some(Vec2::new(world.x / dt, world.y / dt));
        sm_sync(&mut pw.loco, horizontal(world).length() / dt); // (SM) the loop's speed spring goes on from the clip's
    }
    pw.yaw = wrap180(pw.yaw + (b.2 - a.2));
    pw.loco.heading = pw.yaw;
    if pw.anim.finished || t1 >= 1.0 {
        pw.loco.sub = 0;
        pw.loco.phase = 0.0;
        pw.loco.heading = pw.yaw;
    }
}

/// The speed-matched loop (K1, after the pad friends' dec016): the walk and run cycles on one phase at the actual
/// speed, the lean samples by the turn; the stand clip without movement.
fn loop_pose(pw: &mut NpcPawn, dt: f32, yaw0: f32, actual: f32, min: f32, max: f32) {
    pw.loco.sub = 0;
    let idle = if pw.in_battle { "IdleB" } else { "IdleN" };
    if actual < K.k1_stand_speed {
        pw.loco.slow_for += dt;
    } else {
        pw.loco.slow_for = 0.0;
    }
    if pw.loco.slow_for >= K.k1_stand_after {
        if !pw.loco.standing || pw.anim.clip.name != idle {
            pw.loco.standing = true;
            pw.loco.pose = None;
            pw.play(idle, 0.2);
            pw.anim.speed = 1.0;
        }
        pw.anim.axis = 0.0;
        pw.anim.axis_y = 0.0;
        return;
    }
    pw.loco.standing = false;
    let (walk_seq, walk_len, runs, run_len, lean) = pad::loco_set(pw);
    let (walk_speed, run_speed) = (K.k1_walk_speed.max(1.0).min(min.max(1.0)), max);
    let weight = ((actual - walk_speed) / (run_speed - walk_speed).max(1.0)).clamp(0.0, 1.0);
    let mut stride = walk_speed * walk_len + (run_speed * run_len - walk_speed * walk_len) * weight;
    let mid = pad::loco_mid(pw).map(|(seq, len, at)| (seq, len, ((at - walk_speed) / (run_speed - walk_speed).max(1.0)).clamp(0.0, 1.0), at));
    if let Some((_, mid_len, wm, at)) = mid {
        stride = if weight <= wm {
            let t = if wm > 0.0 { weight / wm } else { 1.0 };
            walk_speed * walk_len + (at * mid_len - walk_speed * walk_len) * t
        } else {
            let t = if wm < 1.0 { (weight - wm) / (1.0 - wm) } else { 1.0 };
            at * mid_len + (run_speed * run_len - at * mid_len) * t
        };
    }
    // cycles a second so the stride covers the ground he covers (also above the run speed: no slide)
    let rate = actual / stride.max(1.0);
    pw.loco.phase = (pw.loco.phase + rate * dt).fract();
    let turn = if dt > 0.0 { wrap180(pw.yaw - yaw0) / dt } else { 0.0 };
    let x = (turn / lean).clamp(-1.0, 1.0);
    let samples = if x < 0.0 { [(runs[1], 1.0 + x), (runs[0], -x)] } else { [(runs[1], 1.0 - x), (runs[2], x)] };
    let phase = pw.loco.phase;
    pw.loco.pose = Some(match mid {
        Some((mid_seq, mid_len, wm, _)) => pad::loco_three((walk_seq, walk_len), (mid_seq, mid_len), samples, run_len, phase, weight, wm),
        None => LocoPose { walk: walk_seq, walk_time: phase * walk_len, run: samples, run_time: phase * run_len, run_weight: weight },
    });
    // the slot's own clip is the dominant cycle on the phase (its notifies: footsteps, face, ReverseLeg)
    let dominant = if weight >= 0.5 { pad::row(pw, "RunLoopN", "RunLoopB") } else { pad::row(pw, "WalkLoopN", "WalkLoopB") };
    if pw.anim.clip.name != dominant {
        let blend = if pw.anim.clip.name.starts_with("Idle") || pw.anim.clip.name.starts_with("RunStart") { 0.15 } else { 0.0 };
        pw.play(dominant, blend);
    }
    let len = pw.anim.clip.length.max(1e-4);
    pw.anim.speed = (rate * len) / pw.anim.clip.rate.max(1e-4);
    let want_t = phase * len;
    if (pw.anim.time - want_t).abs() > 0.05 * len {
        pw.anim.time = want_t;
    }
    pw.anim.axis = turn;
    pw.anim.axis_y = actual;
}

/// (K1) The MoveTo's reach test: within the acceptance radius plus the capsule radius (UE path following's
/// bReachTestIncludesAgentRadius), or the goal passed within this step's movement (the offset to it points back along
/// the step and is no longer than the step plus that radius).
pub fn reached(pw: &NpcPawn, d: Vec3, radius: f32, dt: f32) -> bool {
    let reach = radius.max(1.0) + pw.params.capsule_radius;
    if d.length() <= reach {
        return true;
    }
    let moved = horizontal(pw.cmc.displacement_velocity) * dt;
    moved.length() > 1e-3 && d.dot(moved) < 0.0 && d.length() <= reach + moved.length()
}

/// KH3'S STOP AT THE PATH'S END (FollowPlayer tick 0x141215A00, while following): with the speed (+0x2A4) above
/// m_MinSpeed + 100 the arrival is not taken at once: +0x350 = -(speed - min) / m_DecelerationTime and the move goes
/// on along its heading at the easing speed until it is down to m_MinSpeed + 1 (then FUN_1404F5B70 ends it) [inf: the
/// navmesh raycast FUN_143ED09B0 to the stop point is taken as clear]. A new MoveTo meanwhile (the observed Destination,
/// kh-ai) carries on from it. True while it runs (the pawn keeps the command running); false = arrive now.
pub fn decelerate(pw: &mut NpcPawn, cx: &StepCtx) -> bool {
    let dt = cx.dt;
    let (min, max) = (tables::cparam(pw.chara, "loco.move1.min_speed"), tables::cparam(pw.chara, "loco.move1.max_speed"));
    let actual = horizontal(pw.cmc.displacement_velocity).length();
    // (FOL2 r2) the stop eases on below m_MinSpeed to FOL2_STOP_FLOOR over FOL2_WALK_STOP_TIME (KH3 ends the move at 140 cm/s:
    // one step from a walk to standing), also for a walking arrival and a walk round the leader
    let soft = fol2();
    let floor = if soft { FOL2_STOP_FLOOR } else { min };
    let soft_a = (min - FOL2_STOP_FLOOR).max(1.0) / FOL2_WALK_STOP_TIME;
    if pw.loco.decel_v <= 0.0 {
        if soft && actual > FOL2_STOP_FLOOR + 10.0 && actual <= min + 100.0 {
            pw.loco.decel_v = actual;
            pw.loco.decel_a = soft_a;
        } else if !pw.loco.following || actual <= min + 100.0 {
            return false;
        } else {
            pw.loco.decel_v = actual;
            pw.loco.decel_a = ((actual - min) / K.follow_decel_time.max(0.05)).max(pw.loco.fol_brake); // (FOL2 r2) the stop behind a leader who stopped dead
        }
    }
    // (FOL2 r3) the easing run stopped by a wall (the body covers under a fifth of it): the stop is over (it pushed him into the
    // wall and K1's step aside then slid him along it)
    if soft && pw.loco.decel_v > 0.0 && actual < 0.2 * pw.loco.decel_v && pw.loco.decel_v > FOL2_STOP_FLOOR + 20.0 {
        pw.loco.decel_v = 0.0;
        pw.loco.decel_done = true;
        return false;
    }
    pw.loco.decel_v -= pw.loco.decel_a * dt;
    if soft && pw.loco.decel_v <= min && pw.loco.decel_v > floor + 1.0 {
        pw.loco.decel_a = pw.loco.decel_a.min(soft_a.max((pw.loco.decel_v - floor) / FOL2_WALK_STOP_TIME));
    }
    if pw.loco.decel_v <= floor + 1.0 {
        pw.loco.decel_v = 0.0;
        pw.loco.decel_done = true;
        return false;
    }
    pw.cmc.max_walk_speed = pw.loco.decel_v;
    let v = pw.loco.decel_v;
    sm_sync(&mut pw.loco, v); // (SM) KH3's own ease: the spring goes on from it
    pw.cmc.control_input = forward(pw.loco.heading);
    let yaw0 = pw.yaw;
    loop_pose(pw, dt, yaw0, actual, min, max);
    true
}

/// The MoveTo reached its goal: Idle, with the stop clip when he arrives running (K1 gate; the names ATresNpcPawnBase
/// 0x141264350, played by UTresAICharState_Run 0x1404432D0).
pub fn arrive(pw: &mut NpcPawn, cx: &StepCtx) {
    let actual = horizontal(pw.cmc.displacement_velocity).length();
    // (K1) THE HOLD: behind a leader who runs on, an arrival at the Destination does not stop him (KH3's friends
    // chase a Destination that precedes the player: kh-ai's EQS lands it beside them now and then, and every arrival
    // was a stop and a new start at the formation distance). He runs on at the follow speed along the leader's way
    // for at most k1.hold_time, until the next MoveTo carries on from it; the time up -> the stop.
    if pw.loco.following && horizontal(cx.sora.velocity).length() > K.k1_hold_leader_speed && actual > K.k1_stand_speed {
        pw.loco.hold = K.k1_hold_time;
        return;
    }
    stop_now(pw, actual);
}

/// The Move state without a command during the hold (after arrive). True when it handled the tick.
pub fn hold_tick(pw: &mut NpcPawn, cx: &StepCtx) -> bool {
    if pw.loco.hold <= 0.0 {
        return false;
    }
    let dt = cx.dt;
    pw.loco.hold -= dt;
    let actual = horizontal(pw.cmc.displacement_velocity).length();
    let lv = horizontal(cx.sora.velocity);
    if pw.loco.hold <= 0.0 || lv.length() <= K.k1_hold_leader_speed {
        pw.loco.hold = 0.0;
        stop_now(pw, actual);
        return true;
    }
    let (min, max) = (tables::cparam(pw.chara, "loco.move1.min_speed"), tables::cparam(pw.chara, "loco.move1.max_speed"));
    // (SM) the eased follow rule, the speed spring and the critically damped turn, as in move_tick
    sm_view_step(pw, cx);
    let target = follow_speed_with(pw, cx, pw.loco.sm_view, true).unwrap_or(max);
    pw.cmc.max_walk_speed = sm_speed_to(&mut pw.loco, target, min, dt);
    let want = yaw_of(lv);
    sm_turn(&mut pw.loco, want, dt, turn_rate(TurnKind::Locomotion));
    let yaw0 = pw.yaw;
    let fd = wrap180(pw.loco.heading - pw.yaw);
    pw.yaw = wrap180(pw.yaw + fd.clamp(-360.0 * dt, 360.0 * dt));
    pw.cmc.control_input = forward(pw.loco.heading);
    loop_pose(pw, dt, yaw0, actual, min, max);
    true
}

/// The stop: Idle, the stop clip when he arrives running.
fn stop_now(pw: &mut NpcPawn, actual: f32) {
    let max = tables::cparam(pw.chara, "loco.move1.max_speed");
    let min = tables::cparam(pw.chara, "loco.move1.min_speed");
    pw.to_idle();
    pw.loco.pose = None;
    pw.loco.standing = false;
    if actual >= K.k1_stop_clip_speed {
        let stop = pad::stop_clip(pw, actual > 0.5 * (min + max));
        if tables::anim_set(pw.chara).get(stop).is_some() {
            pw.play(stop, -1.0);
            pw.loco.sub = 3;
        }
    }
    pw.cmc.stop_movement();
}

/// Leaving the Move state any other way: the loop's pose goes.
pub fn leave(pw: &mut NpcPawn) {
    pw.loco.hold = 0.0;
    pw.loco.decel_v = 0.0;
    pw.loco.decel_done = false;
    pw.loco.pose = None;
    pw.loco.standing = false;
    pw.anim.speed = 1.0;
    if pw.loco.sub == 2 {
        pw.loco.sub = 0;
    }
    // (FOL2) the turn on the spot and the reaction delay end with the Move state
    if pw.loco.sub == 5 {
        pw.loco.sub = 0;
    }
    if pw.loco.sub == 4 {
        pw.loco.sub = 0;
        pw.loco.idle_turn = None;
    }
    pw.loco.fol_rel = None; // (FOL2 r3)
    pw.loco.fol_delay = 0.0;
}

/// The Idle tick's facing (KH3 0x140445880 rotates at 180 deg/s under the idle clip; K1: the turn clip) and the stop
/// clip's end. `focus` = the point to face (the controller focus), None = none.
pub fn idle_tick(pw: &mut NpcPawn, focus: Option<Vec3>, dt: f32) {
    pw.loco.memory = (pw.loco.memory - dt).max(0.0);
    let idle = if pw.in_battle { "IdleB" } else { "IdleN" };
    if pw.loco.sub == 3 {
        if pw.anim.finished || !pw.anim.clip.name.contains("End") {
            pw.loco.sub = 0;
            if pw.anim.clip.name.contains("End") {
                pw.play(idle, -1.0);
            }
        } else {
            return;
        }
    }
    if let Some((delta, clip_yaw, applied)) = pw.loco.idle_turn {
        let len = pw.anim.clip.length.max(1e-4);
        let f = if pw.anim.finished { 1.0 } else { (pw.anim.clip.root_at(pw.anim.time.min(len)).yaw / clip_yaw).clamp(0.0, 1.0) };
        if f > applied {
            pw.yaw = wrap180(pw.yaw + delta * (f - applied));
        }
        pw.loco.idle_turn = Some((delta, clip_yaw, f.max(applied)));
        if pw.anim.finished || !pw.anim.clip.name.starts_with("Turn") {
            pw.loco.idle_turn = None;
            if pw.anim.clip.name.starts_with("Turn") {
                pw.play(idle, -1.0);
            }
        }
        return;
    }
    let Some(p) = focus else { return };
    let d = horizontal(p - pw.position());
    if d.length_squared() < 1e-4 {
        return;
    }
    let delta = wrap180(yaw_of(d) - pw.yaw);
    if delta.abs() < K.k1_idle_turn_min {
        return;
    }
    let name = if pw.in_battle { "TurnB" } else { "TurnN" };
    let clip = tables::clip(pw.chara, name);
    let clip_yaw = clip.root_at(clip.length).yaw;
    if clip_yaw.abs() < 1.0 {
        // no turn clip with root yaw: KH3's idle rotation
        pw.turn_toward(p, dt, turn_rate(TurnKind::Idle));
        return;
    }
    pw.play(name, -1.0);
    pw.anim.axis = delta;
    if delta.abs() < 90.0 {
        pw.anim.speed = K.k1_idle_turn_rate;
    }
    pw.loco.idle_turn = Some((delta.clamp(-clip_yaw.abs(), clip_yaw.abs()), clip_yaw, 0.0));
}

/// The clip's own root yaw must not turn him while the K1 idle turn scales it (as the Turn state's).
pub fn scales_root_yaw(pw: &NpcPawn) -> bool {
    pw.loco.idle_turn.is_some()
}

