//! `TresPlayerState_Run` and the request that queues it, `TryRun` (docs/sora-movement-spec.md
//! revision 2, sections 2.5, 3.1; change list D). Addresses are the exe's. Behaviour pinned bit
//! for bit by tests/golden.rs; the port options (`Tuning`) are marked PORT OPTION.

use super::common::{ground_probe, on_ground, set_movement_input, turn};
use super::{hopping, jump, rail_slide, slope_slide, swaying, PlayerState, StateData, StateId};
use crate::actions::{self, Suffix};
use crate::anim::{AnimId, LocoSet, PlayArgs};
use crate::pawn::{Ctx, Pawn};
use crate::sora::{wrap_degrees, MovementState};
use crate::world::World;

/// `TresPlayerState_Run` (3.1): +0x88 request, +0x8C yaw, +0x90 mag, +0x94 walk, +0x98 grace,
/// +0xB8 sprint timer, +0xD4 flags (bit 1 initial start, bit 2 probe cached). Reset zeroes all.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Run {
    /// 0 loop, 1 start requested, 2 start clip playing, 4 stop clip.
    pub(super) sub: u8,
    request: bool,
    pub(super) yaw: f32,
    magnitude: f32,
    pub(super) walk: bool,
    grace: f32,
    sprint_timer: f32,
    /// Flag bit 1: the run start from sub-step 1, which a stop cannot interrupt.
    initial_start: bool,
    /// The probe's distance when it has been taken this tick (flag bit 2).
    probe: Option<f32>,
    // ---- bookkeeping of the port option `Tuning::continuous_turning` (not the game's fields)
    /// This Run state was entered from Idle (from not moving).
    from_rest: bool,
    /// Seconds of state ticks since it began.
    age: f32,
    /// It has had a run tick (the run start or a tick of the loop with the walk flag clear).
    had_run: bool,
    /// The target yaws of the latest loop ticks, newest last (`ring_len` of them are valid).
    ring: [f32; RING],
    ring_len: u8,
    // ---- bookkeeping of the port option `Tuning::speed_ramp`
    /// Where the speed limit is between the walk gait (0) and the run gait (1); negative = not
    /// set yet (the first loop tick takes the gait it finds).
    gait: f32,
}

/// Length of the target-yaw history of the reversal test (`Tuning::reversal_window` is at most
/// this minus one).
const RING: usize = 16;

impl Run {
    /// The Run state's world-hit handler (0x1413B2BE0), the part before `TryWallRun`: not
    /// walking, sub-step 0, the stick magnitude above 0.5.
    pub(crate) fn wall_run_ok(&self, s: &Pawn) -> bool {
        !self.walk && self.sub == 0 && s.stick_magnitude > 0.5
    }

    /// Reset (0x1413AD350).
    pub const RESET: Run = Run { sub: 0, request: false, yaw: 0.0, magnitude: 0.0, walk: false, grace: 0.0, sprint_timer: 0.0, initial_start: false, probe: None, from_rest: false, age: 0.0, had_run: false, ring: [0.0; RING], ring_len: 0, gait: -1.0 };
}

/// The Run state's probe (slot 113 0x1413A3D20), change D2: fresh = `hit && dist < 2.0`; cached
/// for the tick, and a cached read returns `dist < 2.0` without the hit flag.
fn run_probe(r: &mut Run, s: &Pawn, w: &dyn World) -> bool {
    let p = *s.params();
    if let Some(distance) = r.probe {
        return distance < p.probe_near_ground_max;
    }
    let (hit, distance, _) = ground_probe(s, w, p.probe_range);
    r.probe = Some(distance);
    hit && distance < p.probe_near_ground_max
}

/// `CanRun` (slot 38 0x14142CE00), spec 2.5. A state without a row here refuses: its own spec
/// adds the row when it is implemented.
fn can_run(state: &mut StateData, s: &Pawn, w: &dyn World) -> bool {
    match state {
        // Idle.timer(+0x9C) <= 0 (always) && OnGround
        StateData::Idle(_) => on_ground(s),
        StateData::Run(r) => on_ground(s) || run_probe(r, s, w),
        StateData::Jump(j) => j.sub == 3 && (j.step_down || on_ground(s)),
        // (actions implementer) SLOPESLIDE and SWAYING end into a run with `TryRun(yaw, 1.0)` (II.6).
        // (The rows are open only for the state's own call: the controller's TryRun of every
        // stick frame must not end them.)
        StateData::SlopeSlide(d) => d.run_exit && on_ground(s),
        StateData::Swaying(w) => w.sub == 3 && on_ground(s),
        // (magic implementer) FIREMAGIC: only out of a moving cast after TresEnableChange (magic spec A 5.4).
        StateData::FireMagic(m) => m.can_run(s) && on_ground(s),
        #[allow(unreachable_patterns)]
        _ => false,
    }
}

/// `TryRun` (slot 40 0x141440030), spec 2.5.
pub(crate) fn try_run(state: &mut StateData, cx: &mut Ctx, yaw: f32, magnitude: f32) {
    let (w, s) = (cx.world, &mut *cx.pawn);
    let p = *s.params();
    if !can_run(state, s, w) {
        return;
    }
    // walk = mag < 0.8 (0x141440092); the restriction overrides are 0.
    let mut walk = magnitude < p.input_walk_run_threshold;
    let continuous = s.tuning.continuous_turning;
    let start_window = s.tuning.start_lunge_window;
    match state {
        // IDLE: SetState(RUN) (clears the queue: a Jump requested earlier in this frame is lost,
        // change D5); sub = 1 unless walking.
        StateData::Idle(_) => {
            s.queue.set_state(Run { sub: if walk { 0 } else { 1 }, request: true, yaw, magnitude, walk, from_rest: true, ..Run::RESET });
        }
        // (magic implementer) Out of a moving cast: straight into the loop.
        StateData::FireMagic(_) => {
            s.queue.set_state(Run { sub: 0, request: true, yaw, magnitude, walk: false, ..Run::RESET });
        }
        // RUN: the live state.
        StateData::Run(r) => {
            if !walk {
                if r.walk {
                    // PORT OPTION `Tuning::continuous_turning`: stay in the loop (the tick below
                    // switches to the run animation and speed); the game replays the run start.
                    // The option keeps the game's path for the START from standstill: a real
                    // stick takes a few frames from the centre to the rim, so the Run state
                    // begins with walk requests (no run start for a walk) and THIS is where the
                    // game plays the start lunge. It counts as the start while the state came
                    // from Idle, has had no run tick yet and is younger than the window.
                    let start = r.from_rest && !r.had_run && r.age <= start_window;
                    if r.sub == 0 && (!continuous || start) {
                        r.sub = 1; // walk -> run goes through the run start
                    }
                } else if r.sub == 4 {
                    r.sub = 1; // a run request during the stop clip
                }
            } else {
                walk = r.sub == 0; // a walk request is honoured only in sub 0
            }
            r.yaw = yaw;
            r.magnitude = magnitude;
            r.walk = walk;
            r.request = true;
        }
        // JUMP (sub 3 with the step-down flag or on the ground): SetState(RUN), walk = 0: the
        // Jump is replaced at the next MainProc by a fresh Run in its loop (change D6).
        StateData::Jump(_) | StateData::SlopeSlide(_) | StateData::Swaying(_) => {
            s.queue.set_state(Run { sub: 0, request: true, yaw, magnitude, walk: false, ..Run::RESET });
        }
        // (Unreachable: `can_run` refuses every other state.)
        #[allow(unreachable_patterns)]
        _ => {}
    }
}

impl PlayerState for Run {
    const ID: StateId = StateId::Run;

    fn sub_step(&self) -> u8 {
        self.sub
    }

    /// Run begin 0x1413A58A0: no animation in the normal path.
    fn begin(&mut self, _s: &mut Ctx) {}

    /// `TresPlayerState_Run::Tick` 0x1413AED70 (3.1): the body is `run_tick` below; a `Draw(1)` it
    /// asks for (the stance rules) is made here, in the same tick.
    fn tick(&mut self, cx: &mut Ctx, dt: f32) {
        let mut draw = false;
        run_tick(self, cx, dt, &mut draw);
        if draw {
            actions::draw(cx, true);
        }
    }

    /// Run::End 0x1413AB9A0: SetWalkMode(false); ResetMaxWalkSpeed; no StopMovement.
    fn end(&mut self, cx: &mut Ctx) {
        let s = &mut *cx.pawn;
        let p = *s.params();
        s.cmc.walk_mode = false;
        s.cmc.max_walk_speed = p.speed_run;
    }

    fn movement_state(&self, pawn: &Pawn) -> MovementState {
        match self.sub {
            1 | 2 => MovementState::RunStart,
            4 => MovementState::RunStop,
            _ if pawn.cmc.walk_mode => MovementState::Walk,
            _ if pawn.cmc.max_walk_speed > pawn.params().speed_run => MovementState::Sprint,
            _ => MovementState::Run,
        }
    }
}

/// The body of `TresPlayerState_Run::Tick` 0x1413AED70 (3.1; the stance parts: actions spec II.3
/// "Run"). `draw` = the tick asked for `Draw(1, 1, 1.0)`.
fn run_tick(r: &mut Run, cx: &mut Ctx, dt: f32, draw: &mut bool) {
    // ---- the traversal exits of tier 2 (actions spec II.5 R1 / H2, II.6; no effect in a world
    // without rails, slide floors, hop points and other pawns in the way).
    if on_ground(cx.pawn) {
        let me = StateData::Run(*r);
        // The rail probe (0x1413AF427): range 50; the stick's yaw when requested, else the facing.
        let want = if r.request { r.yaw } else { cx.pawn.yaw };
        // `TrySlopeSlide(Run+0x98 > 0 || Run+0x94 == 0)` (0x1413AF4C8); `TryHopping(stick yaw,
        // m_bReverseLeg, 0)` while the stick magnitude is >= 0.1 (0x1413AF62F); the bump into
        // another pawn (handler 0x1413B2960: sub-step 0, not walking).
        if rail_slide::probe_and_try(&me, cx, crate::actions::A.rail_check_range, want)
            || slope_slide::try_slope_slide(&me, cx, r.grace > 0.0 || !r.walk)
            || (cx.pawn.stick_magnitude >= crate::actions::A.hopping_run_stick_min && hopping::try_hopping(&me, cx, r.yaw, cx.pawn.anim.reverse_leg))
            || (r.sub == 0 && !r.walk && swaying::run_contact(cx, r.yaw))
        {
            cx.pawn.queue.do_chain();
            return;
        }
    }
    let (w, s) = (cx.world, &mut *cx.pawn);
    let p = *s.params();
    // Not on ground: clear the probe cache; if the probe fails: Fall(yaw) -> DoChain (4.5).
    if !on_ground(s) {
        r.probe = None;
        if !run_probe(r, s, w) {
            jump::fall(s, r.yaw);
            return;
        }
    }
    // Value at tick start, used below (change D11).
    let is_run = r.grace > 0.0 || !r.walk;
    let delta = wrap_degrees(r.yaw - s.yaw);
    // (Port option `speed_matched_anim`: the locomotion loop was started in this tick.)
    let mut loco_fresh = false;
    // Bookkeeping of the port option (no effect with it off).
    r.age += dt;
    if r.sub == 1 || (r.sub == 0 && !r.walk) {
        r.had_run = true;
    }
    // The reversal test of the option: how far the target yaw has swung within the window. The
    // history holds the target yaw of each loop tick; on a tick without a request `r.yaw` is
    // still the last active one, so a flick through the centre counts.
    let mut swing = 0.0f32;
    if r.sub == 0 {
        let window = (s.tuning.reversal_window as usize).min(RING - 1);
        let have = r.ring_len as usize;
        for old in &r.ring[have.saturating_sub(window)..have] {
            swing = swing.max(wrap_degrees(r.yaw - old).abs());
        }
        if have == RING {
            r.ring.copy_within(1.., 0);
            r.ring[RING - 1] = r.yaw;
        } else {
            r.ring[have] = r.yaw;
            r.ring_len += 1;
        }
    }
    // The animation set (Part I 9.1 / 9.3): `N` weapon away, `W` weapon out, `B` battle pose; the
    // `W` / `B` names take the play arguments of their `N` twin. The stance inputs at tick start.
    let set = actions::suffix(s);
    let (battle, battle_anim) = (s.actions.battle, actions::battle_anim(s));
    let loop_twin = if is_run { AnimId::RunLoopN } else { AnimId::WalkLoopN };
    let loop_name = run_name(loop_twin, set);
    // PORT OPTION `Tuning::speed_matched_anim` blends the walk and run cycles of the set in use
    // (coordinator decision 2026-10-11: also with the Keyblade out, the `W` / `B` sets).
    let matched = s.tuning.speed_matched_anim;
    let loco_set = match set {
        Suffix::N => LocoSet::N,
        Suffix::W => LocoSet::W,
        Suffix::B => LocoSet::B,
    };
    // One dispatch on the sub-step at tick start (change D12).
    match r.sub {
        0 => {
            if !r.walk {
                // PORT OPTION `Tuning::continuous_turning`: the turn-around only for a FLICK to
                // the opposite direction (the target yaw swung by `reversal_angle` or more within
                // `reversal_window` ticks), not for a stick rolled round; otherwise the common
                // code below goes on easing the facing and writing the input, as below 75 degrees.
                let flick = swing >= s.tuning.reversal_angle;
                if delta.abs() >= p.turn_turn_around_angle && (!s.tuning.continuous_turning || flick) {
                    r.ring_len = 0; // it cannot trigger again during or right after the clip
                    r.gait = 1.0; // (speed ramp: after the clip the loop is at run speed)
                    // Turn-around (0x1413AF8E0).
                    s.cmc.stop_movement();
                    actions::play_as(s, run_name(AnimId::RunStartN, set), AnimId::RunStartN);
                    s.anim.set_axis(delta);
                    r.sprint_timer = 0.0;
                    s.cmc.max_walk_speed = p.speed_run;
                    r.sub = 2;
                } else {
                    r.sprint_timer += dt;
                    if r.sprint_timer > p.speed_sprint_delay {
                        s.cmc.max_walk_speed = p.speed_sprint;
                    }
                    // Running, no turn-around (0x1413AFA34): `battleAnim` != battle flag. Battle
                    // began: Draw, then "RunBattleStartW" (the name test follows the Draw, so
                    // the `W` clip always); battle over with the weapon out: "RunBattleEnd".
                    // Their `TresLastAnimPose` notifies at t = 0 write the pose.
                    if battle_anim != battle {
                        if battle {
                            *draw = true;
                            s.anim.play_with(AnimId::RunBattleStartW, PlayArgs::mode(2, -1.0));
                            r.sub = 3;
                        } else if s.actions.weapon_out {
                            s.anim.play_with(AnimId::RunBattleEnd, PlayArgs::mode(2, -1.0));
                            r.sub = 3;
                        }
                    }
                }
            } else {
                r.sprint_timer = 0.0;
                s.cmc.max_walk_speed = p.speed_run;
                // Walking (0x1413AFF3B): battle began: the Keyblade is drawn, no clip; battle
                // over: nothing for the plain Keyblade.
                if battle_anim != battle && battle {
                    *draw = true;
                }
            }
            if r.sub == 0 {
                if matched {
                    // PORT OPTION: one continuous locomotion loop; no play call when the gait changes.
                    if !s.anim.in_locomotion() {
                        s.anim.play_locomotion(loco_set, is_run);
                        loco_fresh = true;
                    } else if s.anim.locomotion_set() != Some(loco_set) {
                        // The set changed under the loop (Keyblade drawn / put away, battle pose).
                        s.anim.retarget_locomotion(loco_set);
                    }
                } else if s.anim.name != loop_name || s.anim.in_locomotion() {
                    actions::play_as(s, loop_name, loop_twin);
                }
            }
        }
        1 => {
            actions::play_as(s, run_name(AnimId::RunStartN, set), AnimId::RunStartN);
            s.anim.set_axis(delta);
            r.initial_start = true;
            r.sub = 2;
            r.gait = 1.0; // (speed ramp: the start is not ramped; the loop begins at run speed)
        }
        // 3 = 2 (the same table entry 0x1413B025F): `RunBattleStartW` / `RunBattleEnd` playing.
        2 | 3 => {
            if s.anim.ended {
                if matched {
                    s.anim.play_locomotion(loco_set, is_run);
                    loco_fresh = true;
                } else {
                    actions::play_as(s, loop_name, loop_twin);
                }
                r.initial_start = false;
                r.sub = 0;
            }
        }
        _ => {
            // 4: the stop clip. Turn(yaw, dt x 12); when it ends: DoChain (-> IDLE). The request
            // latch is not cleared here.
            turn(s, r.yaw, dt * p.turn_run_stop_rate);
            if s.anim.ended {
                s.queue.do_chain();
            }
            return;
        }
    }
    // COMMON (sub 0..3 at tick start).
    if !r.request {
        if matches!(r.sub, 0 | 3) || (r.sub == 2 && !r.initial_start) {
            // Component Acceleration (from this frame's movement tick), 0x1413B0876.
            let a = s.cmc.acceleration;
            if a.x * a.x + a.y * a.y <= p.ground_stop_accel_sq_max {
                s.cmc.stop_movement();
                let stop = match (is_run, s.anim.reverse_leg) {
                    (true, false) => AnimId::RunEndR,
                    (true, true) => AnimId::RunEndL,
                    (false, false) => AnimId::WalkEndR,
                    (false, true) => AnimId::WalkEndL,
                };
                actions::play_as(s, run_name(stop, set), stop);
                r.grace = 0.0;
                r.sub = 4;
                return;
            }
        }
        if matches!(r.sub, 0 | 3) {
            // Still turns on the no-request tick before the stop (change D13).
            turn(s, r.yaw, p.turn_run_alpha_per_60hz_frame * dt * 60.0);
        }
    } else {
        s.cmc.walk_mode = !is_run;
        r.grace = if !r.walk { p.input_run_grace_time } else { (r.grace - dt).max(0.0) };
        if matches!(r.sub, 0 | 3) {
            // blendX = walk ? 0 : clamp(delta, axis range of the playing asset) (change D14).
            if let Some((low, high)) = s.anim.axis_range() {
                s.anim.set_axis(if r.walk { 0.0 } else { wrap_degrees(r.yaw - s.yaw).clamp(low, high) });
            }
            turn(s, r.yaw, p.turn_run_alpha_per_60hz_frame * dt * 60.0);
            s.cmc.max_acceleration = s.cmc.max_walk_speed * p.accel_ground_factor;
            set_movement_input(s, r.yaw);
        }
    }
    // PORT OPTION `Tuning::speed_ramp` (a user decision, not the game's code): in the loop the
    // speed limit moves linearly between the walk gait's and the run gait's instead of jumping
    // when the gait switches (`is_run` at tick start, i.e. after the game's 0.2 s grace). The
    // gait's own limits are the game's: MaxWalkSpeed x 0.205882 (140) and MaxWalkSpeed (680, or
    // 980 while sprinting: the sprint switch itself is not ramped, and a walk tick resets the
    // sprint as in the game). The limit is held on a tick without a request.
    if s.tuning.speed_ramp && r.sub == 0 {
        let target = if is_run { 1.0 } else { 0.0 };
        if r.gait < 0.0 || !(s.tuning.speed_ramp_time > 0.0) {
            r.gait = target;
        } else if r.request {
            let step = dt / s.tuning.speed_ramp_time;
            r.gait = if r.gait < target { (r.gait + step).min(target) } else { (r.gait - step).max(target) };
        }
        let (walk, run) = (s.cmc.max_walk_speed * p.speed_walk_rate, s.cmc.max_walk_speed);
        s.cmc.speed_limit_override = Some(walk + (run - walk) * r.gait);
    }
    // PORT OPTION `Tuning::speed_matched_anim` (a user decision, not the game's playback): in the
    // loop the walk cycle and the run blend space are blended by his actual speed and share one
    // phase whose rate makes the feet's stride match the ground covered: stride per cycle =
    // speed x cycle length at each gait's own speed (walk 140 x 1.0667, run 680 x 0.5333), blended
    // by the same weight; rate = speed / stride. At exactly the walk or the run speed each clip
    // plays at its own rate 1.0, as in the game. Below the walk speed the rate is the walk's (the
    // cycle never freezes); above the run speed (sprint) the game's playback is kept unless
    // `speed_match_sprint`. The tick that starts the loop takes the gait it starts in (after the
    // run start the velocity is still 0 for that one tick).
    if s.tuning.speed_matched_anim && r.sub == 0 && s.anim.in_locomotion() {
        // (party K1) AI Sora's catch-up (Sora::set_speed_scale) raises his speeds: the strides stay his own (the base
        // run speed), and above it the cycle is speed-matched too (no foot slide); scale 1 = unchanged
        let boosted = p.speed_run > crate::params::SORA_PARAMS.speed_run + 0.5;
        let p_run = if boosted { crate::params::SORA_PARAMS.speed_run } else { p.speed_run };
        let (walk_speed, run_speed) = (p_run * p.speed_walk_rate, p_run);
        // (The cycle lengths of the set that plays: `WalkLoopB` is 1.2 s, the others as `N`.)
        let playing = s.anim.locomotion_set().unwrap_or(loco_set);
        let (walk_length, run_length) = (playing.walk.data().length, playing.run.data().length);
        let velocity = s.cmc.velocity;
        let speed = (velocity.x * velocity.x + velocity.y * velocity.y).sqrt();
        let weight = if loco_fresh { if is_run { 1.0 } else { 0.0 } } else { ((speed - walk_speed) / (run_speed - walk_speed)).clamp(0.0, 1.0) };
        let stride = walk_speed * walk_length + (run_speed * run_length - walk_speed * walk_length) * weight;
        let rate = if loco_fresh {
            1.0 / if is_run { run_length } else { walk_length }
        } else if speed > run_speed && !s.tuning.speed_match_sprint && !boosted {
            1.0 / run_length
        } else {
            speed.max(walk_speed) / stride
        };
        s.anim.set_locomotion(weight, rate);
    }
    r.request = false;
}

/// The run / walk names by animation set (Part I 9.3): `WalkLoopW/B`, `RunLoopW/B`, `RunStartW/B`,
/// `WalkEndR/L W/B`, `RunEndR/L W/B`.
/// (`RunStartB`'s samples differ in length: `AnimPlayer` plays such a blend space on a normalised
/// clock since 2026-10-11, so the former fallback to `RunStartN` is gone.)
fn run_name(twin: AnimId, set: Suffix) -> AnimId {
    match (twin, set) {
        (_, Suffix::N) => twin,
        (AnimId::RunLoopN, Suffix::W) => AnimId::RunLoopW,
        (AnimId::RunLoopN, Suffix::B) => AnimId::RunLoopB,
        (AnimId::WalkLoopN, Suffix::W) => AnimId::WalkLoopW,
        (AnimId::WalkLoopN, Suffix::B) => AnimId::WalkLoopB,
        (AnimId::RunStartN, Suffix::W) => AnimId::RunStartW,
        (AnimId::RunStartN, Suffix::B) => AnimId::RunStartB,
        (AnimId::RunEndR, Suffix::W) => AnimId::RunEndRW,
        (AnimId::RunEndR, Suffix::B) => AnimId::RunEndRB,
        (AnimId::RunEndL, Suffix::W) => AnimId::RunEndLW,
        (AnimId::RunEndL, Suffix::B) => AnimId::RunEndLB,
        (AnimId::WalkEndR, Suffix::W) => AnimId::WalkEndRW,
        (AnimId::WalkEndR, Suffix::B) => AnimId::WalkEndRB,
        (AnimId::WalkEndL, Suffix::W) => AnimId::WalkEndLW,
        (AnimId::WalkEndL, Suffix::B) => AnimId::WalkEndLB,
        _ => twin,
    }
}
