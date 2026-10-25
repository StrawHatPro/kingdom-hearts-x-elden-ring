//! The core against docs/sora-movement-spec.md revision 2 (section numbers cited as "spec") and
//! docs/sora-movement-spec-changes.md ("change A4" etc.), in the analytic test world, at the
//! reference rate of 60 steps a second. Expected values are the specification's, not tuned ones.
//!
//! Frame numbering as in the spec: frame N runs C(N) -> M(N) -> S(N); `outs[N]` is the state at
//! the end of frame N.

use kh_core::anim::FIRST_UPDATE_HOLDS_POSITION;
use kh_core::test_world::TestWorld;
use kh_core::{AnimId, Input, MovementState, Output, Pose, Sora, StateId, Tuning, Vec2, Vec3, World, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;

/// Extra frames before a clip's clock starts, under the animation spec's reading of the first
/// update (see `kh_core::anim::FIRST_UPDATE_HOLDS_POSITION`): 0 with the movement spec's.
const HOLD: usize = FIRST_UPDATE_HOLDS_POSITION as usize;

/// The game's rules as read: every port option off. (The options have their own tests at the end.)
fn spawn_at(x: f32, y: f32, z: f32, yaw: f32) -> Sora {
    Sora::new(SORA_PARAMS, Pose { position: Vec3::new(x, y, z), yaw }).with_tuning(Tuning::GAME_EXACT)
}

fn spawn() -> Sora {
    spawn_at(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS), 0.0)
}

fn stick(x: f32, y: f32) -> Input {
    Input { move_stick: Vec2::new(x, y), ..Input::default() }
}

/// The raw stick that, after the game's per-axis dead zone, points `degrees` to the right of
/// forward with full magnitude.
fn stick_toward(degrees: f32) -> Input {
    let a = degrees.to_radians();
    let raw = |v: f32| if v.abs() < 1.0e-6 { 0.0 } else { v.signum() * (v.abs() * 0.75 + 0.25) };
    stick(raw(a.sin()), raw(a.cos()))
}

fn forward() -> Input {
    stick(0.0, 1.0)
}

fn speed(o: &Output) -> f32 {
    Vec2::new(o.velocity.x, o.velocity.y).length()
}

/// Steps `n` frames with the input `f(frame)` gives; returns every output.
fn run(sora: &mut Sora, world: &dyn World, n: usize, f: impl Fn(usize) -> Input) -> Vec<Output> {
    (0..n).map(|i| sora.step(DT, &f(i), world)).collect()
}

fn bits(o: &Output) -> [u32; 13] {
    [
        o.position.x.to_bits(),
        o.position.y.to_bits(),
        o.position.z.to_bits(),
        o.velocity.x.to_bits(),
        o.velocity.y.to_bits(),
        o.velocity.z.to_bits(),
        o.yaw.to_bits(),
        o.anim.time.to_bits(),
        o.anim.axis.to_bits(),
        o.anim.play_id,
        o.state as u32,
        o.sub_step as u32,
        o.anim.name as u32,
    ]
}

/// A fixed, varied input sequence: run, turn, jump while running, stop, turn around, hop.
fn scripted(step: usize) -> Input {
    let t = step as f32 * DT;
    let move_stick = if t < 0.5 {
        Vec2::ZERO
    } else if t < 2.5 {
        Vec2::new(0.0, 1.0)
    } else if t < 4.0 {
        Vec2::new(0.8, 0.8)
    } else if t < 4.5 {
        Vec2::ZERO
    } else if t < 6.0 {
        Vec2::new(-1.0, 0.1)
    } else if t < 7.0 {
        Vec2::new(0.5, 0.0)
    } else {
        Vec2::new(0.0, -1.0)
    };
    let press = step == 150 || step == 340 || step == 420;
    Input { move_stick, jump_pressed: press, jump_held: press || (150..175).contains(&step) || (340..343).contains(&step), camera_yaw: 30.0 * (t * 0.5).sin(), ..Input::default() }
}

// ------------------------------------------------------------------ determinism, frame slicing

#[test]
fn same_inputs_give_bit_identical_outputs() {
    let world = TestWorld::floor().with_plane(Vec3::NEG_X, -2500.0).with_box(Vec3::new(600.0, -200.0, 0.0), Vec3::new(900.0, 200.0, 30.0)).with_plane_through(Vec3::new(0.0, 1500.0, 0.0), Vec3::new(0.0, -0.5, 0.866));
    let go = || {
        let mut sora = spawn();
        run(&mut sora, &world, 600, scripted)
    };
    let (a, b) = (go(), go());
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        assert_eq!(bits(x), bits(y), "outputs differ at frame {i}");
    }
    // ... and the run did something worth comparing.
    for state in [MovementState::Run, MovementState::RunStart, MovementState::RunStop, MovementState::JumpRise, MovementState::Fall, MovementState::Land, MovementState::Walk] {
        assert!(a.iter().any(|o| o.movement_state == state), "{state:?} never seen");
    }
    assert!(a.iter().all(|o| o.position.is_finite() && o.velocity.is_finite() && o.yaw.is_finite()));
}

#[test]
fn frame_slicing_by_the_host_does_not_change_the_steps() {
    // The host steps at the fixed reference rate whatever the display rate: step k gets input k.
    let world = TestWorld::floor();
    let reference = {
        let mut sora = spawn();
        run(&mut sora, &world, 300, scripted)
    };
    for frame_nanos in [16_666_667u64, 6_944_444, 33_333_333, 11_111_111] {
        let (mut sora, mut accumulated, mut k, step_nanos) = (spawn(), 0u64, 0usize, 16_666_666u64);
        let mut outputs = Vec::new();
        while k < 300 {
            accumulated += frame_nanos;
            while accumulated >= step_nanos && k < 300 {
                accumulated -= step_nanos;
                outputs.push(sora.step(DT, &scripted(k), &world));
                k += 1;
            }
        }
        assert!(outputs.iter().zip(&reference).all(|(a, b)| bits(a) == bits(b)), "display frames of {frame_nanos} ns changed the result");
    }
}

#[test]
fn dt_is_clamped_to_max_delta_time() {
    // Change A7: never more than 0.05 s (MaxDeltaTime).
    let world = TestWorld::default();
    let fall = |dt: f32| {
        let mut sora = spawn_at(0.0, 0.0, 5000.0, 0.0);
        run(&mut sora, &world, 3, |_| Input::default());
        sora.step(dt, &Input::default(), &world).velocity.z - sora.step(dt, &Input::default(), &world).velocity.z
    };
    assert_eq!(fall(0.05), fall(0.5));
    assert!((fall(0.05) - 1470.0 * 0.05).abs() < 0.01);
}

// ------------------------------------------------------------------ standing, idle, input

#[test]
fn stands_still_on_the_floor() {
    let world = TestWorld::floor();
    let mut sora = spawn_at(0.0, 0.0, 1.0, 0.0);
    let out = run(&mut sora, &world, 120, |_| Input::default()).last().unwrap().clone();
    assert!(out.on_floor);
    assert_eq!((out.state, out.movement_state, out.anim.name), (StateId::Idle, MovementState::Idle, AnimId::IdleN));
    // Capsule bottom (origin + 2) floats 1.9 .. 2.4 cm above the floor, aimed at 2.15 (spec 3.4).
    assert!((out.position.z + 2.0 - 2.15).abs() < 0.02, "floor clearance {}", out.position.z + 2.0);
    assert_eq!(out.velocity, Vec3::ZERO);
    assert_eq!(out.capsule_center.z - out.position.z, 91.0);
}

#[test]
fn idle_replays_and_fidgets_on_the_third_end() {
    // Spec 3.1: IdleN (3.0 s) replayed on its end, IdleBreakN on the third end.
    let world = TestWorld::floor();
    let mut sora = spawn();
    let outs = run(&mut sora, &world, (9.6 / DT) as usize, |_| Input::default());
    let first_break = outs.iter().position(|o| o.anim.name == AnimId::IdleBreakN).expect("IdleBreakN");
    let t = first_break as f32 * DT;
    assert!((t - 9.0).abs() < 0.12, "IdleBreakN at {t} s");
    assert!(outs[..first_break].iter().all(|o| o.anim.name == AnimId::IdleN));
}

#[test]
fn dead_zone_and_active_threshold() {
    // Spec 2.1, 2.3: per-axis dead zone 0.25; active unless magnitude < 0.1 after it.
    let world = TestWorld::floor();
    let moved = |raw: f32| {
        let mut sora = spawn();
        run(&mut sora, &world, 30, |_| stick(0.0, raw)).last().unwrap().state != StateId::Idle
    };
    assert!(!moved(0.25));
    assert!(!moved(0.32)); // (0.32 - 0.25) / 0.75 = 0.093 < 0.1
    assert!(moved(0.33)); // 0.107
}

#[test]
fn stick_is_relative_to_the_camera() {
    // Spec 2.3: moveYaw = cameraYaw + atan2(LeftX, LeftY).
    let world = TestWorld::floor();
    let go = |camera_yaw: f32, x: f32, y: f32| {
        let mut sora = spawn();
        run(&mut sora, &world, 120, |_| Input { move_stick: Vec2::new(x, y), camera_yaw, ..Input::default() }).last().unwrap().clone()
    };
    let a = go(0.0, 0.0, 1.0);
    assert!(a.position.x > 800.0 && a.position.y.abs() < 1.0 && a.yaw.abs() < 0.5, "{a:?}");
    let b = go(0.0, 1.0, 0.0);
    assert!(b.position.y > 800.0 && (b.yaw - 90.0).abs() < 0.5, "{b:?}");
    let c = go(90.0, 0.0, 1.0);
    assert!(c.position.y > 800.0 && (c.yaw - 90.0).abs() < 0.5, "{c:?}");
    let d = go(0.0, -1.0, 0.0);
    assert!(d.position.y < -800.0 && (d.yaw + 90.0).abs() < 0.5, "{d:?}");
}

// ------------------------------------------------------------------ run start, speeds, stop

#[test]
fn run_start_moves_him_by_its_root_track() {
    // Spec 3.6 / reconciliation A2: RunStartN from idle: 0.2333 s, 180 cm by root motion
    // (11.9, 26, 40.4, 56.6, 75, 104, 180 cm on frames 1-7 at 30 fps); no input during it.
    // Frame order: the stick in C(0) starts Run in S(0), which plays the clip; its clock first
    // advances in M(1) (change A2): outs[k] is at clip time k/60.
    let world = TestWorld::floor();
    let mut sora = spawn();
    let outs = run(&mut sora, &world, 40, |_| forward());
    assert_eq!((outs[0].movement_state, outs[0].anim.name, outs[0].position.x), (MovementState::RunStart, AnimId::RunStartN, 0.0));
    assert!(outs[0].anim.root_motion);
    for (frame30, expected) in [(1usize, 11.91), (2, 26.0), (3, 40.36), (4, 56.63), (5, 75.0), (6, 104.0), (7, 180.0)] {
        let x = outs[2 * frame30 + HOLD].position.x;
        assert!((x - expected).abs() < 0.3, "at clip time {}/30 s: x = {x}, the track says {expected}", frame30);
    }
    let end = 14 + HOLD;
    assert!(outs[..end].iter().all(|o| o.movement_state == MovementState::RunStart && speed(o) == 0.0));
    // The displacement velocity (what the camera reads) does include the root motion.
    assert!(outs[4].displacement_velocity.x > 300.0);
    // S(14): the clip has ended -> the loop, and the first input; M(15): full speed in one tick
    // (MaxAcceleration = MaxWalkSpeed x 60).
    assert_eq!((outs[end].movement_state, outs[end].anim.name, speed(&outs[end])), (MovementState::Run, AnimId::RunLoopN, 0.0));
    assert!((speed(&outs[end + 1]) - 680.0).abs() < 0.01, "first loop tick: {}", speed(&outs[end + 1]));
}

#[test]
fn run_start_to_the_side_turns_him_by_its_root_track() {
    // The -90 sample (0007L): 180 cm to his left with a 90 degree turn to the left; the rotation
    // is applied to the actor after the physics (change B11).
    let world = TestWorld::floor();
    let mut sora = spawn();
    let outs = run(&mut sora, &world, 16 + HOLD, |_| stick(-1.0, 0.0));
    assert!((outs[1].anim.axis + 90.0).abs() < 1.0e-3, "axis {}", outs[1].anim.axis);
    let end = &outs[14 + HOLD];
    assert!((end.position.y + 180.0).abs() < 0.5 && end.position.x.abs() < 0.5, "ended at {:?}", end.position);
    assert!((end.yaw + 90.0).abs() < 0.1, "yaw {}", end.yaw);
    // Halfway between two samples the blend space mixes them: -135 = half 0007BL, half 0007L.
    assert_eq!(AnimId::RunStartN.sample_weights(-135.0), [("0007BL", 0.5), ("0007L", 0.5)]);
    assert_eq!(AnimId::RunLoopN.sample_weights(2.5), [("0003", 0.75), ("0003R", 0.25)]);
    assert_eq!(AnimId::IdleN.sample_weights(0.0)[0], ("0001", 1.0));
}

#[test]
fn run_680_sprint_980_after_more_than_one_second_walk_140() {
    let world = TestWorld::floor();
    // Run 680 (spec 3.1). Sprint: the timer counts the sub-0 ticks and needs > 1.0 (strict);
    // SetMaxWalkSpeed(980) in that state tick acts in the next movement tick.
    let mut sora = spawn();
    let outs = run(&mut sora, &world, 130, |_| forward());
    let loop_start = outs.iter().position(|o| o.movement_state == MovementState::Run).unwrap();
    let sprint = outs.iter().position(|o| o.movement_state == MovementState::Sprint).expect("Sprint");
    let ticks = sprint - loop_start; // sub-0 state ticks with the timer running before it passed 1.0
    assert!(ticks == 60 || ticks == 61, "the sprint timer passed 1.0 after {ticks} ticks of 1/60 s");
    assert!(outs[loop_start + 2..=sprint].iter().all(|o| (speed(o) - 680.0).abs() < 0.01));
    assert!((speed(&outs[sprint + 1]) - 980.0).abs() < 0.01, "the tick after: {}", speed(&outs[sprint + 1]));
    // Walk: stick below 0.8 -> walk mode, 680 x 0.205882 = 140 cm/s; no run start; one tick.
    let mut sora = spawn();
    let outs = run(&mut sora, &world, 120, |_| stick(0.0, 0.7)); // (0.7 - 0.25) / 0.75 = 0.6
    assert!(outs.iter().all(|o| o.movement_state == MovementState::Walk));
    assert_eq!(outs[0].anim.name, AnimId::WalkLoopN);
    assert!((speed(&outs[1]) - 140.0).abs() < 0.05 && (speed(outs.last().unwrap()) - 140.0).abs() < 0.05, "walk speed {}", speed(&outs[1]));
    // The threshold itself: 0.8 after the dead zone is raw 0.85.
    let mut sora = spawn();
    assert_eq!(run(&mut sora, &world, 5, |_| stick(0.0, 0.86)).last().unwrap().movement_state, MovementState::RunStart);
    let mut sora = spawn();
    assert_eq!(run(&mut sora, &world, 5, |_| stick(0.0, 0.84)).last().unwrap().movement_state, MovementState::Walk);
}

#[test]
fn run_keeps_its_loop_for_the_grace_time_when_the_stick_drops() {
    // Spec 3.1: grace = 0.2 on run ticks, max(grace - dt, 0) on walk ticks; walk mode and the
    // loop follow isRun = grace > 0 || !walk taken at tick start (change D11).
    let world = TestWorld::floor();
    let mut sora = spawn();
    let outs = run(&mut sora, &world, 90, |i| if i < 40 { forward() } else { stick(0.0, 0.7) });
    let walk = outs.iter().position(|o| o.movement_state == MovementState::Walk).expect("Walk");
    let after = (walk - 40) as f32 * DT;
    assert!((after - 0.2).abs() <= 2.0 * DT, "walk mode {after} s after the stick dropped");
    assert!(outs[walk + 2].anim.name == AnimId::WalkLoopN);
    // Walk -> run goes through the run start.
    let outs = run(&mut sora, &world, 3, |_| forward());
    assert_eq!(outs[0].movement_state, MovementState::RunStart);
}

#[test]
fn a_turn_of_75_degrees_or_more_replays_the_run_start() {
    // Spec 3.1 (0x1413AF8E0): |delta| >= 75 -> StopMovement, RunStartN with blendX = delta.
    let world = TestWorld::floor();
    let turn = |degrees: f32| {
        let mut sora = spawn();
        run(&mut sora, &world, 40, |_| forward());
        run(&mut sora, &world, 32, |_| stick_toward(degrees))
    };
    let outs = turn(180.0);
    assert_eq!((outs[0].movement_state, outs[0].anim.name), (MovementState::RunStart, AnimId::RunStartN));
    assert_eq!(speed(&outs[0]), 0.0, "velocity is zeroed");
    assert!((outs[1].anim.axis.abs() - 180.0).abs() < 0.01, "axis {}", outs[1].anim.axis);
    // The turn itself is the animation's: 180 degrees and 180 cm back along the new direction.
    let x_at_turn = outs[0].position.x;
    let end = outs.iter().find(|o| o.movement_state == MovementState::Run).expect("back in the loop");
    assert!((end.yaw.abs() - 180.0).abs() < 1.0, "yaw {}", end.yaw);
    assert!((x_at_turn - end.position.x - 180.0).abs() < 1.0, "moved {} cm back", x_at_turn - end.position.x);
    let outs = turn(80.0);
    assert_eq!(outs[0].movement_state, MovementState::RunStart);
    assert!((outs[1].anim.axis - 80.0).abs() < 0.01);
    // Below the threshold he steers in the loop: Turn(yaw, 0.1 x dt x 60) = 10 % of the
    // remaining angle per 60 Hz tick.
    let outs = turn(70.0);
    assert_eq!(outs[0].movement_state, MovementState::Run);
    assert!((outs[0].yaw - 7.0).abs() < 0.01 && (outs[1].yaw - 13.3).abs() < 0.01, "yaw {} then {}", outs[0].yaw, outs[1].yaw);
    // ... and the run loop leans by the remaining yaw error clamped to +-10, through the blend
    // space's 0.4 s filter (animation change A6): it rises, then falls back as he turns.
    let lean: Vec<f32> = outs.iter().map(|o| o.anim.axis).collect();
    assert!(lean[1] > 0.0 && lean.iter().all(|a| (0.0..=10.0).contains(a)), "{lean:?}");
    assert!(lean[1] < 1.0, "the newest input has weight dt / window: {}", lean[1]);
}

#[test]
fn letting_go_brakes_one_tick_then_plays_the_stop_then_idle() {
    // Spec 3.1 "Timing": last stick frame N-1; S(N) sees no request but a non-zero Acceleration:
    // turn only; M(N+1) has no input: one braking tick, 680 -> 464.5 (spec 3.3); S(N+1) stops.
    let world = TestWorld::floor();
    let mut sora = spawn();
    run(&mut sora, &world, 60, |_| forward());
    let outs = run(&mut sora, &world, 60, |_| Input::default());
    assert_eq!(outs[0].movement_state, MovementState::Run);
    assert!((speed(&outs[0]) - 680.0).abs() < 0.01);
    // Frame N+1: the braking tick moved him at 464.5, then the state zeroed the velocity.
    assert_eq!(outs[1].movement_state, MovementState::RunStop);
    assert!(matches!(outs[1].anim.name, AnimId::RunEndR | AnimId::RunEndL));
    assert_eq!(speed(&outs[1]), 0.0);
    let braked = (outs[1].position.x - outs[0].position.x) / DT;
    assert!((braked - 464.533).abs() < 0.05, "the braking tick moved him at {braked} cm/s (464.5)");
    // RunEnd is 0.5 s = 30 clock advances from the frame after the play; then Idle in that frame.
    let idle = outs.iter().position(|o| o.state == StateId::Idle).expect("Idle");
    assert_eq!(idle, 31 + HOLD, "Idle at frame N+{idle}");
    assert_eq!(outs[idle].anim.name, AnimId::IdleN);
    // No root motion: he stays where the stop began (reconciliation A1).
    assert!((outs[idle].position - outs[1].position).length() < 0.01);
}

#[test]
fn the_stop_foot_follows_the_reverse_leg_notify_state() {
    // L while the TresReverseLeg state of the loop (0.100 - 0.334 s of 0.533 s) is listed in the
    // frame's notify window, R otherwise (animation change A10).
    let world = TestWorld::floor();
    let mut seen = [0, 0];
    for run_frames in 40..72 {
        let mut sora = spawn();
        run(&mut sora, &world, run_frames, |_| forward());
        let outs = run(&mut sora, &world, 3, |_| Input::default());
        // The loop's clock in the frame the stop is decided (S(N+1)): outs[0] is frame N.
        let t = (outs[0].anim.time + DT) % 0.533_333_4;
        let stop = outs.iter().find(|o| o.movement_state == MovementState::RunStop).unwrap();
        let inside = t + 1.0e-4 >= 0.100371 && t - DT + 1.0e-4 < 0.333671;
        if (t - 0.100371).abs() < 0.003 || (t - DT - 0.333671).abs() < 0.003 {
            continue; // on the boundary of the window
        }
        assert_eq!(stop.anim.name, if inside { AnimId::RunEndL } else { AnimId::RunEndR }, "loop time {t}");
        seen[inside as usize] += 1;
    }
    assert!(seen[0] > 3 && seen[1] > 3, "{seen:?}");
}

#[test]
fn a_new_run_request_interrupts_the_stop_and_a_walk_request_does_not() {
    let world = TestWorld::floor();
    let mut sora = spawn();
    run(&mut sora, &world, 60, |_| forward());
    let outs = run(&mut sora, &world, 12, |i| if i < 8 { Input::default() } else { forward() });
    assert_eq!(outs[7].movement_state, MovementState::RunStop);
    assert_eq!((outs[8].movement_state, outs[8].anim.name), (MovementState::RunStart, AnimId::RunStartN));
    let mut sora = spawn();
    run(&mut sora, &world, 60, |_| forward());
    let outs = run(&mut sora, &world, 20, |i| if i < 8 { Input::default() } else { stick(0.0, 0.7) });
    assert!(outs[8..].iter().all(|o| o.movement_state == MovementState::RunStop));
}

// ------------------------------------------------------------------ jump

/// Presses jump in frame 0 and holds it for `held` frames; returns the outputs.
fn jump(sora: &mut Sora, world: &dyn World, frames: usize, held: usize, move_stick: Vec2) -> Vec<Output> {
    run(sora, world, frames, |i| Input { move_stick, jump_pressed: i == 0, jump_held: i < held, camera_yaw: 0.0, ..Input::default() })
}

#[test]
fn the_launch_is_five_frames_after_the_press() {
    // Spec 4.1: press in C(0); S(0) begins Jump and plays JumpNormalStart; the clock passes the
    // TresJumpStart notify (0.0666 s) in M(4), whose handler sets bPressedJump and the second
    // latch; CheckJumpInput of M(5) does DoJump. Walking mode on the ground until then.
    let world = TestWorld::floor();
    let mut sora = spawn();
    run(&mut sora, &world, 10, |_| Input::default());
    let outs = jump(&mut sora, &world, 12, 12, Vec2::ZERO);
    assert_eq!((outs[0].movement_state, outs[0].anim.name), (MovementState::JumpStart, AnimId::JumpNormalStart));
    let notify = 4 + HOLD;
    assert!(outs[..notify].iter().all(|o| o.sub_step == 0 && o.on_floor));
    assert!(outs[notify].sub_step == 1 && outs[notify].on_floor, "the notify frame: sub-step {}", outs[notify].sub_step);
    let takeoff = outs.iter().position(|o| !o.on_floor).expect("left the ground");
    assert_eq!(takeoff, 5 + HOLD, "DoJump frame");
    // DoJump: Velocity.Z = 1480, then that frame is a pure falling tick with the rise gravity.
    assert!((outs[takeoff].velocity.z - (1480.0 - 4116.0 * DT)).abs() < 0.01, "vz {}", outs[takeoff].velocity.z);
}

#[test]
fn jump_held_reaches_266_cm_in_0_36_s() {
    // Spec 4.3: launch 1480, rising gravity -4116: 266 cm, rise 0.360 s.
    let world = TestWorld::floor();
    let mut sora = spawn();
    run(&mut sora, &world, 10, |_| Input::default());
    let ground = sora.output().position.z;
    let outs = jump(&mut sora, &world, 120, 120, Vec2::ZERO);
    let takeoff = outs.iter().position(|o| !o.on_floor).unwrap();
    let apex = outs.iter().enumerate().max_by(|a, b| a.1.position.z.total_cmp(&b.1.position.z)).unwrap();
    let height = apex.1.position.z - ground;
    assert!((height - 266.1).abs() <= 1.0, "apex {height} cm");
    let rise = (apex.0 + 1 - takeoff) as f32 * DT;
    assert!((rise - 0.360).abs() <= DT, "rise time {rise} s");
    // Rise loop when the 0.1 s start clip has ended; fall loop once the hang is over.
    assert!(outs.iter().any(|o| o.anim.name == AnimId::JumpNormalRiseLoop));
    assert_eq!(outs[apex.0 + 12].anim.name, AnimId::JumpNormalDownLoop);
}

#[test]
fn jump_gravity_has_three_phases_and_the_hang_is_8_steps_at_60_hz() {
    // Spec 4.3: -4116 while Velocity.Z > 0 at the start of the step, -147 while the accumulated
    // hang time stays strictly below 0.15 (8 steps at 60 Hz), then -1470. The step that crosses
    // the apex runs entirely with the rise gravity.
    let world = TestWorld::floor();
    let mut sora = spawn();
    let outs = jump(&mut sora, &world, 80, 80, Vec2::ZERO);
    let vz: Vec<f32> = outs.iter().map(|o| o.velocity.z).collect();
    let takeoff = outs.iter().position(|o| !o.on_floor).unwrap();
    let apex = takeoff + vz[takeoff..].iter().position(|v| *v <= 0.0).unwrap();
    let drop = |i: usize| vz[i - 1] - vz[i];
    assert!((takeoff + 1..=apex).all(|i| (drop(i) - 4116.0 * DT).abs() < 0.01), "rise steps");
    let hang: Vec<usize> = (apex + 1..apex + 20).filter(|&i| (drop(i) - 147.0 * DT).abs() < 0.01).collect();
    assert_eq!(hang, (apex + 1..=apex + 8).collect::<Vec<_>>(), "hang steps");
    assert!((apex + 9..apex + 20).all(|i| (drop(i) - 1470.0 * DT).abs() < 0.01), "fall steps");
    // The state restores GravityScale in the apex frame, so the hang always uses 1.5; and the
    // fall loop is played in the frame the hang ends.
    assert_eq!(outs[apex].sub_step, 4);
    let down = outs.iter().position(|o| o.anim.name == AnimId::JumpNormalDownLoop).unwrap();
    assert_eq!(down, apex + 9);
}

#[test]
fn the_short_hop_starts_on_the_first_rise_loop_tick_with_the_button_up() {
    // Spec 4.2: the sub-1 test is dead; the hop starts on the first sub-2 state tick with
    // Velocity.Z > 0 and the button not down; GravityScale 3.0 acts from the next movement tick
    // (rising gravity -8232). A tap and any release before sub 2 give the same hop.
    let world = TestWorld::floor();
    let peak = |held: usize| {
        let mut sora = spawn();
        let ground = sora.step(DT, &Input::default(), &world).position.z;
        let outs = jump(&mut sora, &world, 120, held, Vec2::ZERO);
        (outs.iter().map(|o| o.position.z).fold(f32::MIN, f32::max) - ground, outs)
    };
    let (full, _) = peak(120);
    let (tap, outs) = peak(1);
    assert!(tap < full - 60.0, "tap {tap} cm, full {full} cm");
    // Frames: DoJump M(5); the start clip ends in M(6) -> S(6) plays the rise loop (sub 2);
    // S(7) is the first sub-2 tick -> 3.0; M(8) is the first -8232 step.
    let vz: Vec<f32> = outs.iter().map(|o| o.velocity.z).collect();
    let first = (1..vz.len()).find(|&i| ((vz[i - 1] - vz[i]) - 8232.0 * DT).abs() < 0.01).expect("a -8232 step");
    assert!(first == 8 + HOLD || first == 9 + HOLD, "first -8232 step in frame {first}");
    // Held through C(first - 1) (the frame whose S makes the test): no hop yet in that frame.
    let (released_before, _) = peak(first - 1);
    assert_eq!(released_before, tap, "a release before the first sub-2 tick is the same hop");
    let (held_longer, _) = peak(first + 3);
    assert!(held_longer > tap + 5.0 && held_longer < full, "held 4 frames longer: {held_longer} cm against {tap}");
}

#[test]
fn no_coyote_time_and_no_buffering() {
    // Spec 2.4: a refused press is gone.
    let world = TestWorld::floor();
    let mut sora = spawn();
    jump(&mut sora, &world, 20, 20, Vec2::ZERO);
    assert!(!sora.output().on_floor);
    let outs = run(&mut sora, &world, 200, |i| Input { jump_pressed: i == 0, jump_held: i < 150, ..Input::default() });
    let landed = outs.iter().position(|o| o.on_floor).unwrap();
    assert!(outs[landed..].iter().all(|o| o.on_floor), "jumped again without a press on the ground");
}

#[test]
fn a_jump_press_in_idle_with_the_stick_active_is_lost() {
    // Spec 2.5 "Consequences" (change D5): the jump is dispatched first; TryRun afterwards does
    // SetState(RUN), clearing the queued Jump.
    let world = TestWorld::floor();
    let mut sora = spawn();
    run(&mut sora, &world, 5, |_| Input::default());
    let outs = run(&mut sora, &world, 30, |i| Input { move_stick: Vec2::new(0.0, 1.0), jump_pressed: i == 0, jump_held: true, ..Input::default() });
    assert_eq!(outs[0].state, StateId::Run);
    assert!(outs.iter().all(|o| o.state == StateId::Run && o.on_floor));
    // From Run the same press works (TryRun updates the live Run state; the Jump stays queued).
    let outs = run(&mut sora, &world, 3, |i| Input { move_stick: Vec2::new(0.0, 1.0), jump_pressed: i == 0, jump_held: true, ..Input::default() });
    assert_eq!(outs[0].state, StateId::Jump);
}

#[test]
fn the_take_off_dip() {
    // Spec 4.2: jump from Run, press in frame N: M(N) accelerates normally; S(N) ends Run and
    // begins Jump, whose first tick has no request; M(N+1): one braking tick in Walking mode,
    // 680 -> 464.5; from S(N+1) the Jump state steers with 3672 cm/s^2 (+61.2 per tick, cap 680).
    let world = TestWorld::floor();
    let mut sora = spawn();
    run(&mut sora, &world, 40, |_| forward());
    let outs = jump(&mut sora, &world, 12, 12, Vec2::new(0.0, 1.0));
    assert!((speed(&outs[0]) - 680.0).abs() < 0.01 && outs[0].state == StateId::Jump);
    assert!((speed(&outs[1]) - 464.533).abs() < 0.05, "frame N+1: {}", speed(&outs[1]));
    assert!((speed(&outs[2]) - (464.533 + 61.2)).abs() < 0.05, "frame N+2: {}", speed(&outs[2]));
    assert!((speed(&outs[3]) - (464.533 + 122.4)).abs() < 0.05, "frame N+3: {}", speed(&outs[3]));
    assert!(outs[..5 + HOLD].iter().all(|o| o.on_floor));
    assert!(outs.iter().all(|o| speed(o) <= 680.01));
}

#[test]
fn air_control_and_braking() {
    // Spec 4.4: air input acceleration 3672 x 0.95 = 3488 cm/s^2, cap 680; no input: braking
    // 2048 cm/s^2 with friction 0.
    let world = TestWorld::floor();
    let mut sora = spawn();
    let outs = jump(&mut sora, &world, 40, 40, Vec2::ZERO);
    let takeoff = outs.iter().position(|o| !o.on_floor).unwrap();
    assert_eq!(speed(&outs[takeoff + 3]), 0.0);
    // Push the stick in the air: the Jump state's input of S(k) acts in M(k+1).
    let mut sora = spawn();
    let outs = run(&mut sora, &world, 40, |i| Input { move_stick: if i > takeoff { Vec2::new(0.0, 1.0) } else { Vec2::ZERO }, jump_pressed: i == 0, jump_held: true, camera_yaw: 0.0, ..Input::default() });
    let gain = speed(&outs[takeoff + 3]) - speed(&outs[takeoff + 2]);
    assert!((gain - 3488.4 * DT).abs() < 0.05, "air acceleration step {gain}");
    // A sprint jump (980) is braked toward 680 and never below it while pushing forward.
    let mut sora = spawn();
    run(&mut sora, &world, 100, |_| forward());
    assert!((speed(&sora.output()) - 980.0).abs() < 0.01);
    let outs = jump(&mut sora, &world, 60, 60, Vec2::new(0.0, 1.0));
    let takeoff = outs.iter().position(|o| !o.on_floor).unwrap();
    assert!(outs[takeoff..takeoff + 40].iter().all(|o| speed(o) >= 679.9), "dropped below 680 in the air");
    // No input in the air: 2048 cm/s^2.
    let mut sora = spawn();
    run(&mut sora, &world, 40, |_| forward());
    let outs = run(&mut sora, &world, 40, |i| Input { move_stick: if i < 3 { Vec2::new(0.0, 1.0) } else { Vec2::ZERO }, jump_pressed: i == 0, jump_held: true, camera_yaw: 0.0, ..Input::default() });
    let takeoff = outs.iter().position(|o| !o.on_floor).unwrap();
    let loss = speed(&outs[takeoff + 3]) - speed(&outs[takeoff + 4]);
    assert!((loss - 2048.0 * DT).abs() < 0.05, "air braking step {loss}");
}

// ------------------------------------------------------------------ falling, landing

#[test]
fn a_fall_not_started_by_a_jump_has_plain_gravity_from_its_first_tick() {
    // Spec 4.3: only DoJump opens the three-phase curve. Spawned in the air: the first physics
    // call is skipped (m_bDisableMoveOnSpawnFrame), the walking tick finds no floor and switches
    // to Falling with no time left, gravity starts in the next tick (spec 3.4).
    let world = TestWorld::floor();
    let mut sora = spawn_at(0.0, 0.0, 1000.0, 0.0);
    let outs = run(&mut sora, &world, 14, |_| Input::default());
    let falling = outs.iter().position(|o| !o.on_floor).unwrap();
    assert_eq!(outs[falling].velocity.z, 0.0, "the frame the floor is lost: no gravity yet");
    for i in falling + 1..falling + 8 {
        assert!((outs[i - 1].velocity.z - outs[i].velocity.z - 1470.0 * DT).abs() < 0.01, "frame {i}");
    }
    // Idle sees "not on ground" in the same frame: Fall -> Jump at sub 3 -> 4, the fall loop.
    assert_eq!((outs[falling].state, outs[falling].sub_step, outs[falling].anim.name), (StateId::Jump, 4, AnimId::JumpNormalDownLoop));
    assert!(outs.iter().all(|o| o.movement_state != MovementState::JumpRise));
}

#[test]
fn running_off_a_high_ledge() {
    // Spec 5.4: for an edge higher than the perch distance (59.4 cm) the edge checks run and he
    // starts falling about 1 cm after the axis passes the edge; spec 4.5: the Run state keeps
    // going while its probe (a 50 cm sphere) still rests on the lip, then Fall -> Jump sub 3.
    let world = TestWorld::floor().with_box(Vec3::new(-500.0, -500.0, 0.0), Vec3::new(300.0, 500.0, 400.0));
    let mut sora = spawn_at(0.0, 0.0, 400.15, 0.0);
    let outs = run(&mut sora, &world, 120, |_| forward());
    let off = outs.iter().position(|o| !o.on_floor).expect("left the ledge");
    let x_off = outs[off].position.x;
    assert!(x_off > 300.0 && x_off < 300.0 + 980.0 * DT + 2.0, "left Walking at x = {x_off} (edge at 300)");
    assert!(outs[off].position.z > 399.0, "did not sink round the edge: z = {}", outs[off].position.z);
    // Plain gravity from the tick after.
    for i in off + 1..off + 6 {
        assert!((outs[i - 1].velocity.z - outs[i].velocity.z - 1470.0 * DT).abs() < 0.01, "frame {i}");
    }
    // The Run state for the frames in which its probe sphere still touches the lip, then Jump.
    let jump_at = outs.iter().position(|o| o.state == StateId::Jump).expect("Jump");
    assert!(jump_at >= off && outs[jump_at].position.x < 300.0 + 50.0 + 980.0 * DT + 2.0, "Jump state at x = {}", outs[jump_at].position.x);
    assert!(outs[jump_at..].iter().any(|o| o.anim.name == AnimId::JumpNormalDownLoop));
    let end = outs.last().unwrap();
    assert!(end.on_floor && end.position.z < 1.0);
}

#[test]
fn kerbs_up_to_the_perch_distance_are_rolled_down_without_leaving_walking() {
    // Spec 5.3 / 5.4: stepping off a kerb of height <= 59.4 cm the perch probe finds the lower
    // ground, the floor stays walkable and the capsule rolls down round the edge; higher: falling.
    for (height, stays_walking) in [(30.0, true), (45.0, true), (58.0, true), (62.0, false), (120.0, false)] {
        let world = TestWorld::floor().with_box(Vec3::new(-500.0, -500.0, 0.0), Vec3::new(300.0, 500.0, height));
        let mut sora = spawn_at(100.0, 0.0, height + 0.15, 0.0);
        let outs = run(&mut sora, &world, 150, |_| stick(0.0, 0.7));
        let left = outs.iter().any(|o| !o.on_floor);
        assert_eq!(!left, stays_walking, "kerb of {height} cm: left Walking = {left}");
        let end = outs.last().unwrap();
        assert!(end.on_floor && end.position.z < 1.0 && end.position.x > 380.0, "kerb of {height} cm: ended at {:?}", end.position);
        if stays_walking {
            // Z follows the round bottom: monotonically down, no jump of more than the hemisphere allows.
            assert!(outs.windows(2).all(|w| w[1].position.z <= w[0].position.z + 0.3), "kerb of {height} cm: went up on the way down");
        }
    }
}

/// Falls from `height` cm onto the floor with the given input; returns the outputs.
fn drop_from(height: f32, input: Input) -> Vec<Output> {
    let world = TestWorld::floor();
    let mut sora = spawn_at(0.0, 0.0, height, 0.0);
    run(&mut sora, &world, 240, |_| input)
}

#[test]
fn landing_light_up_to_75_cm_and_heavy_above() {
    // Spec 4.6 (0x14139628B): light = !(fallHeight > 75). The fall height is summed at the end of
    // the Jump tick while sub == 4, so the landing tick's own drop is not in it. The state sees
    // the landing in the same frame and zeroes the velocity.
    let landing = |height: f32| {
        let outs = drop_from(height, Input::default());
        let at = outs.iter().position(|o| o.movement_state == MovementState::Land).expect("landed");
        assert!(!outs[at - 1].on_floor && outs[at].on_floor, "the state lands in the frame the movement does");
        outs[at].clone()
    };
    assert_eq!(landing(60.0).anim.name, AnimId::LightLand);
    assert_eq!(landing(150.0).anim.name, AnimId::Land);
    assert_eq!(landing(150.0).velocity, Vec3::ZERO);
    // A full jump on flat ground falls 266 cm: Land.
    let world = TestWorld::floor();
    let mut sora = spawn();
    let outs = jump(&mut sora, &world, 150, 150, Vec2::ZERO);
    assert_eq!(outs.iter().find(|o| o.movement_state == MovementState::Land).unwrap().anim.name, AnimId::Land);
    // A fall of 800 cm or more with the stick held is RollLand (actions spec II.3 J8; it was
    // `Land` and a note while stage 1 had no roll landing: tests/actions.rs covers the rule).
    let mut sora = spawn_at(0.0, 0.0, 1200.0, 0.0);
    let outs = run(&mut sora, &world, 240, |_| forward());
    assert_eq!(outs.iter().find(|o| o.movement_state == MovementState::Land).unwrap().anim.name, AnimId::RollLand);
    assert_eq!(sora.notes().len(), 0);
}

#[test]
fn landing_snaps_the_origin_onto_the_ground() {
    // Spec 4.6: in sub-step 10 a swept SetActorLocation puts the origin on the probe's ground
    // point: lower by FloorDist - 2 = 0.15 cm on flat ground.
    let outs = drop_from(150.0, Input::default());
    let land = outs.iter().position(|o| o.movement_state == MovementState::Land).unwrap();
    // (FloorDist anywhere in the accepted band 1.9 .. 2.4 is left alone, so "about 0.15".)
    assert!((outs[land].position.z - 0.15).abs() < 0.26 && outs[land].position.z > 0.0 && outs[land].sub_step == 10, "touchdown frame: z {}", outs[land].position.z);
    assert!(outs[land + 1].position.z.abs() < 0.01 && outs[land + 1].sub_step == 11, "the frame after: z {}", outs[land + 1].position.z);
}

#[test]
fn control_returns_at_the_enable_change_notify() {
    // Spec 4.6: TresEnableChange at 0.0991 s of Land / LightLand; else the clip's end, 0.4 s.
    // Touchdown frame T plays the clip; its clock passes 0.0991 in M(T+6); S(T+6) leaves.
    let landing = |input: Input| {
        let outs = drop_from(150.0, input);
        let land = outs.iter().position(|o| o.movement_state == MovementState::Land).unwrap();
        let frames = outs[land..].iter().position(|o| o.movement_state != MovementState::Land).unwrap();
        (frames, outs[land + frames].clone(), outs[land + frames + 1].clone())
    };
    let (frames, after, next) = landing(forward());
    assert_eq!(frames, 6 + HOLD, "with the stick held the landing lasted {frames} frames");
    // The Jump state finishes, Idle begins; the next request starts Run through RunStartN.
    assert_eq!(after.state, StateId::Idle);
    assert_eq!((next.movement_state, next.anim.name), (MovementState::RunStart, AnimId::RunStartN));
    let (frames, after, _) = landing(Input::default());
    assert!(frames == 24 + HOLD || frames == 25 + HOLD, "without the stick the landing lasted {frames} frames (0.4 s)");
    assert_eq!(after.state, StateId::Idle);
}

#[test]
fn a_jump_during_the_landing_needs_sub_step_11() {
    // Spec 4.6 (0x14142A552): the gate accepts state JUMP as soon as sub == 11, without the
    // notify. Touchdown in frame T (sub 10); a press in frame T+1 (the controller still sees sub
    // 10) is refused and lost; from frame T+2 a press starts a new jump.
    let world = TestWorld::floor();
    let try_press = |after_touchdown: usize| {
        let mut sora = spawn_at(0.0, 0.0, 150.0, 0.0);
        let mut touchdown = None;
        let mut jumped = false;
        for i in 0..200 {
            let press = touchdown.is_some_and(|t| i == t + after_touchdown);
            let out = sora.step(DT, &Input { jump_pressed: press, jump_held: press, ..Input::default() }, &world);
            if touchdown.is_none() && out.movement_state == MovementState::Land {
                touchdown = Some(i);
            }
            jumped |= touchdown.is_some() && out.movement_state == MovementState::JumpStart;
        }
        jumped
    };
    assert!(!try_press(1), "a press in the frame after touchdown must be lost");
    assert!(try_press(2), "a press two frames after touchdown must start a jump");
    assert!(try_press(10));
}

// ------------------------------------------------------------------ walls, steps, slopes

#[test]
fn walking_into_a_wall_stops_him_but_not_the_commanded_speed() {
    // Spec 3.4: velocity is never recomputed from the move.
    let world = TestWorld::floor().with_plane(Vec3::NEG_X, -900.0);
    let mut sora = spawn();
    let outs = run(&mut sora, &world, 180, |_| forward());
    let out = outs.last().unwrap();
    assert!(out.position.x <= 900.0 - 60.0 && out.position.x > 900.0 - 62.5, "x = {}", out.position.x);
    assert!((outs[178].position - out.position).length() < 0.01, "still moving");
    assert!(speed(out) > 600.0, "commanded speed kept: {}", speed(out));
    assert!(out.displacement_velocity.length() < 0.5, "the displacement velocity is 0 against a wall");
    assert!(out.on_floor);
}

#[test]
fn a_ceiling_hit_does_not_zero_the_vertical_velocity() {
    // Change C1: after an air impact Velocity stays what CalcVelocity and gravity made it; only
    // the position slides.
    let world = TestWorld::floor().with_plane(Vec3::NEG_Z, -300.0); // solid above z = 300: head room 120 cm
    let mut sora = spawn();
    let outs = jump(&mut sora, &world, 60, 60, Vec2::ZERO);
    let top = outs.iter().map(|o| o.position.z).fold(f32::MIN, f32::max);
    assert!(top <= 300.0 - 180.0 + 0.01 && top > 115.0, "stopped at the ceiling: origin at {top}");
    let stuck = outs.iter().position(|o| o.position.z > top - 1.0).unwrap();
    assert!(outs[stuck + 1].velocity.z > 500.0, "Velocity.Z after the hit: {}", outs[stuck + 1].velocity.z);
    assert!((outs[stuck + 1].velocity.z - outs[stuck + 2].velocity.z - 4116.0 * DT).abs() < 0.01, "it decays under gravity");
}

#[test]
fn slides_along_a_wall_met_at_an_angle() {
    let world = TestWorld::floor().with_plane(Vec3::NEG_X, -400.0);
    let mut sora = spawn();
    let out = run(&mut sora, &world, 180, |_| stick(0.7, 0.7)).last().unwrap().clone();
    assert!(out.position.y > 600.0, "did not slide along it: y = {}", out.position.y);
    assert!(out.position.x <= 400.0 - 60.0);
}

/// A ledge `height` cm high whose riser is at x = 400.
fn ledge_world(height: f32) -> TestWorld {
    TestWorld::floor().with_box(Vec3::new(400.0, -1000.0, -10.0), Vec3::new(2000.0, 1000.0, height))
}

/// Walks (stick 0.7) or runs into the ledge; returns (frames from first contact until the origin
/// is on top, or None; the rise of the origin in the contact frame; the final output).
fn approach_ledge(height: f32, input: Input) -> (Option<usize>, f32, Output) {
    let world = ledge_world(height);
    let mut sora = spawn();
    let outs = run(&mut sora, &world, 700, |_| input);
    let contact = outs.iter().position(|o| o.position.z > 0.25 || (o.position.x > 300.0 && o.displacement_velocity.length() < 1.0)).unwrap_or(outs.len() - 1);
    let on_top = outs.iter().position(|o| o.position.z > height - 0.5 && o.on_floor && o.position.x > 400.0 - 60.0);
    (on_top.map(|t| t.saturating_sub(contact)), outs[contact].position.z - outs[contact.saturating_sub(1)].position.z, outs.last().unwrap().clone())
}

#[test]
fn step_up_accepts_47_cm_and_refuses_48() {
    // Spec 3.4, the fork's StepUp (0x140597570): up 47 - max(FloorDist - 2, 0), forward, down
    // 51.8; reject if the landing impact is more than 47 above origin - max(FloorDist - 2, 0).
    // What StepUp itself decides shows in the frame of first contact: accepted, he is lifted
    // onto the edge at once.
    for height in [30.0, 45.0, 47.0] {
        let (frames, rise, end) = approach_ledge(height, stick(0.0, 0.7));
        assert!(height < 47.0 || rise > 4.0, "ledge of {height} cm: StepUp did not lift him at first contact (rise {rise} cm)");
        assert!(frames.is_some() && end.on_floor && end.position.z > height - 0.5 && end.position.x > 450.0, "ledge of {height} cm: ended at {:?}", end.position);
    }
    let (_, rise, _) = approach_ledge(48.0, stick(0.0, 0.7));
    assert!(rise < 2.0, "ledge of 48 cm: StepUp accepted it at first contact (rise {rise} cm)");
    // A ledge met by the capsule's straight side (above the 62 cm top of its round bottom) is a wall.
    for height in [70.0, 120.0] {
        let (frames, _, end) = approach_ledge(height, stick(0.0, 0.7));
        assert!(frames.is_none() && end.on_floor && end.position.z < 1.0 && end.position.x < 341.0, "ledge of {height} cm: ended at {:?}", end.position);
    }
}

#[test]
fn ledge_heights_30_to_62_cm() {
    // The re-test asked for with the fork StepUp and the perch code (movement change list,
    // "Rides up ledges of 48-58 cm"). With THIS test world's edge normals (the box face most
    // opposed to the move, UE's FindBoxOpposingNormal; runtime item R4):
    //   - up to 47 cm StepUp lifts him at first contact;
    //   - 58 cm: the edge contact is 59.86 cm from the capsule's axis, outside the edge tolerance
    //     (R - 0.15 = 59.85): the floor sweep rejects it; a wall. Running (680 cm/s) into 48 or 50
    //     cm is also a wall: recorded, not derived.
    //   - 48 and 50 cm, walking: StepUp refuses at first contact, but the floor sweep made right after it
    //     meets the ledge's edge under the round bottom (a walkable hit, which the perch probe
    //     confirms because the lower floor is within 59.4 cm), AdjustFloorHeight lifts him to
    //     2.15 cm above that contact, and a tick or two later StepUp's own test
    //     (ImpactPoint.Z - (origin.Z - fd) <= 47) passes because the origin has risen;
    //   - 62 cm: the edge is level with the top of the round bottom: a wall.
    // The outcome for 48 - 58 cm is a consequence of the rules as written plus R4, recorded here
    // so that a change shows up; it is not a statement about the game.
    let mut table = Vec::new();
    for height in [30.0f32, 45.0, 47.0, 48.0, 50.0, 58.0, 62.0] {
        let (walk, walk_rise, _) = approach_ledge(height, stick(0.0, 0.7));
        let (run_frames, _, _) = approach_ledge(height, forward());
        table.push((height, walk, walk_rise, run_frames));
    }
    for (height, walk, rise, run_frames) in &table {
        println!("ledge {height:>4} cm: walking: on top after {walk:?} frames (first-contact rise {rise:.2} cm); running: {run_frames:?} frames");
    }
    for (height, walk, _, run_frames) in &table {
        match *height as i32 {
            30 | 45 | 47 => assert!(walk.is_some() && run_frames.is_some(), "{height} cm must be climbed"),
            58 | 62 => assert!(walk.is_none() && run_frames.is_none(), "{height} cm must not be climbed"),
            _ => assert!(walk.is_some(), "{height} cm: recorded behaviour changed (walking no longer rides up)"),
        }
    }
}

/// A slope rising toward +X that starts at x = 300.
fn slope(degrees: f32) -> TestWorld {
    let a = degrees.to_radians();
    TestWorld::floor().with_plane_through(Vec3::new(300.0, 0.0, 0.0), Vec3::new(-a.sin(), 0.0, a.cos()))
}

#[test]
fn walks_a_49_degree_slope_but_not_a_50_degree_one() {
    // WalkableFloorAngle 49, WalkableFloorZ 0.656059 (spec 5.2).
    let climb = |degrees: f32| {
        let world = slope(degrees);
        let mut sora = spawn();
        run(&mut sora, &world, 240, |_| forward()).last().unwrap().clone()
    };
    let up = climb(49.0);
    assert!(up.on_floor && up.position.z > 500.0, "49 degrees: ended at {:?}", up.position);
    // The horizontal speed is kept on the ramp (ComputeGroundMovementDelta).
    assert!((speed(&up) - 980.0).abs() < 0.01);
    assert!(up.displacement_velocity.z > 900.0, "the displacement velocity has the slope's rise: {}", up.displacement_velocity.z);
    let blocked = climb(50.0);
    assert!(blocked.position.z < 80.0 && blocked.position.x < 420.0, "50 degrees: ended at {:?}", blocked.position);
    let gentle = climb(30.0);
    let expected = (gentle.position.x - 300.0) * 30.0f32.to_radians().tan();
    assert!(gentle.on_floor && (gentle.position.z - expected).abs() < 40.0, "30 degrees: z {} for {}", gentle.position.z, expected);
}

#[test]
fn lands_on_a_platform_he_jumps_onto() {
    let world = TestWorld::floor().with_box(Vec3::new(300.0, -300.0, 0.0), Vec3::new(2000.0, 300.0, 200.0));
    let mut sora = spawn();
    run(&mut sora, &world, 3, |_| Input::default());
    let outs = run(&mut sora, &world, 100, |i| Input { move_stick: if i > 0 { Vec2::new(0.0, 1.0) } else { Vec2::ZERO }, jump_pressed: i == 0, jump_held: true, camera_yaw: 0.0, ..Input::default() });
    let out = outs.last().unwrap();
    assert!(out.on_floor && (out.position.z - 200.0).abs() < 0.5, "ended at {:?}", out.position);
}

// ------------------------------------------------------------------ port option: continuous turning

/// The turning option alone (the tests of one port option switch the later ones off).
const TURNING_ONLY: Tuning = Tuning { speed_ramp: false, speed_matched_anim: false, ..Tuning::GAME_EXACT_WITH_TURNING };

/// With the user's option (the default). Runs forward for a second, then plays `path` (raw stick
/// per frame), then holds its last value for half a second. Returns the outputs from the start
/// of the path and the number of RunStartN plays in them.
fn turning(path: &[Vec2]) -> (Vec<Output>, usize) {
    let world = TestWorld::floor();
    assert!(Tuning::default().continuous_turning, "the default is the user's choice");
    let mut sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, 0.15), yaw: 0.0 }).with_tuning(TURNING_ONLY);
    run(&mut sora, &world, 60, |_| forward());
    let last = *path.last().unwrap();
    let outs = run(&mut sora, &world, path.len() + 30, |i| Input { move_stick: *path.get(i).unwrap_or(&last), ..Input::default() });
    let before = outs.first().map_or(0, |o| o.anim.play_id);
    let _ = before;
    let replays = outs.windows(2).filter(|w| w[1].anim.play_id != w[0].anim.play_id && w[1].anim.name == AnimId::RunStartN).count() + (outs[0].anim.name == AnimId::RunStartN && outs[0].anim.time == 0.0) as usize;
    (outs, replays)
}

fn rim(degrees: f32, radius: f32) -> Vec2 {
    Vec2::new(degrees.to_radians().sin(), degrees.to_radians().cos()) * radius
}

#[test]
fn continuous_turning_never_replays_the_run_start_while_the_stick_stays_pushed() {
    // User decision 2026-10-11 (kh_core::Tuning::continuous_turning; the original rules:
    // docs/sora-movement-spec-turning.md triggers A and B).
    let mut paths: Vec<(String, Vec<Vec2>)> = Vec::new();
    // Sweeps round the rim at any angular speed, 90 to 360 degrees, down to an instant flip.
    // (Sweeps faster than 135 degrees within 5 ticks are flicks since the second form of the
    // option: they play the turn-around and have their own test.)
    for total in [90.0f32, 180.0, 360.0] {
        for frames in [1usize, 2, 3, 6, 9, 15, 27, 60] {
            // (The per-axis dead zone makes the target yaw sweep up to about 1.5 times faster than
            // the raw stick angle between the axes: 15 raw degrees a tick stays under 135 in 5.)
            if total / frames as f32 > 15.0 {
                continue;
            }
            paths.push((format!("sweep of {total} degrees in {frames} frames"), (1..=frames).map(|k| rim(total * k as f32 / frames as f32, 1.0)).collect()));
        }
    }
    // Rolls at 0.9 of the unit radius (a walk request on the diagonals).
    // (A full circle in 6 frames is 60 degrees a tick: a flick by the option's own definition.)
    for frames in [30usize, 120] {
        paths.push((format!("roll at radius 0.9 in {frames} frames"), (0..=frames).map(|k| rim(360.0 * k as f32 / frames as f32, 0.9)).collect()));
    }
    // Flicks that cut the corner: a straight line from up to right, and from up to down-right.
    // (A flick of 135 degrees within 5 ticks is a reversal by the option's definition: its own test.)
    for (to, frames) in [(90.0f32, 3usize), (90.0, 6), (90.0, 12)] {
        paths.push((format!("straight flick to {to} degrees in {frames} frames"), (1..=frames).map(|k| rim(0.0, 1.0).lerp(rim(to, 1.0), k as f32 / frames as f32)).collect()));
    }
    // Keys: 90 degree switches every 6 frames, and a held diagonal.
    // (90 degrees every 6 frames: under 135 within 5 ticks.)
    let keys = [Vec2::new(1.0, 0.0), Vec2::new(0.0, -1.0), Vec2::new(-1.0, 0.0), Vec2::new(0.0, 1.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)];
    paths.push(("keyboard 90 degree switches".to_string(), keys.iter().flat_map(|k| [*k; 6]).collect()));
    // One frame below the walk / run threshold while running straight.
    paths.push(("one frame at raw 0.84".to_string(), vec![Vec2::new(0.0, 0.84), Vec2::new(0.0, 1.0)]));
    for (name, path) in &paths {
        // The premise: pushed (active) on every frame.
        let dead = |v: f32| if v.abs() < 0.25 { 0.0 } else { v.signum() * (v.abs() - 0.25) / 0.75 };
        assert!(path.iter().all(|s| Vec2::new(dead(s.x), dead(s.y)).length() >= 0.1), "{name}: the path leaves the active range");
        let (outs, replays) = turning(path);
        assert_eq!(replays, 0, "{name}: RunStartN replayed");
        assert!(outs.iter().all(|o| o.state == StateId::Run && matches!(o.anim.name, AnimId::RunLoopN | AnimId::WalkLoopN)), "{name}: left the loop");
        // He never stops: the speed stays at the walk speed or above.
        let slowest = outs.iter().map(speed).fold(f32::MAX, f32::min);
        assert!(slowest >= 139.9, "{name}: slowed to {slowest} cm/s");
        // ... and ends facing where the stick points, by the unchanged ease.
        let last = *path.last().unwrap();
        let target = last.x.atan2(last.y).to_degrees();
        let end = outs.last().unwrap();
        let off = (end.yaw - (dead(last.x)).atan2(dead(last.y)).to_degrees() + 540.0).rem_euclid(360.0) - 180.0;
        // (30 frames after the path's end: at most 180 x 0.9^30 = 7.6 degrees are left.)
        assert!(off.abs() < 8.0, "{name}: facing {} for a stick at {target}", end.yaw);
    }
}

#[test]
fn continuous_turning_eases_a_reversal_with_the_run_loops_turn_and_keeps_the_speed() {
    // The first form of the option (no flick turn-around: reversal_angle out of reach). An
    // instant 180: travel follows the stick at once (the input is written every tick), the
    // facing eases by 10 % of the remaining angle per 60 Hz tick: Turn(target, 0.1 x dt x 60).
    let world = TestWorld::floor();
    let mut sora = user_sora().with_tuning(Tuning { reversal_angle: 999.0, ..Tuning::default() });
    run(&mut sora, &world, 60, |_| forward());
    let outs = run(&mut sora, &world, 31, |_| stick(0.0, -1.0));
    let replays = run_starts(&outs).len() + (outs[0].anim.name == AnimId::RunStartN) as usize;
    assert_eq!(replays, 0);
    // The frame of the flip: S writes the new input; from M of the next frame the movement
    // component turns the velocity round by its own rule (CalcVelocity: friction toward the
    // input direction, acceleration MaxWalkSpeed x 60), without ever stopping him by a state.
    let reversed = outs.iter().position(|o| o.velocity.x < -670.0).expect("runs the other way");
    let slowest = outs[..=reversed].iter().map(speed).fold(f32::MAX, f32::min);
    println!("continuous turning, instant 180: velocity x per frame {:?}; full speed the other way after {reversed} frames, slowest {slowest:.0} cm/s", outs[..=reversed].iter().map(|o| o.velocity.x.round()).collect::<Vec<_>>());
    assert!(reversed <= 6, "took {reversed} frames");
    let remaining: Vec<f32> = outs.iter().map(|o| 180.0 - o.yaw.abs()).collect();
    assert!((remaining[0] - 162.0).abs() < 0.01 && (remaining[1] - 145.8).abs() < 0.01, "{:?}", &remaining[..3]);
    let frames_to_10_degrees = remaining.iter().position(|r| *r < 10.0).unwrap();
    println!("continuous turning, instant 180: facing within 10 degrees after {frames_to_10_degrees} frames; he runs backwards relative to his facing meanwhile");
    assert_eq!(frames_to_10_degrees, 27);
}

#[test]
fn continuous_turning_still_lunges_after_the_stick_was_not_pushed() {
    // "Only when the stick isn't pushed anymore": a centred stick is not pushed and the original
    // stop logic applies unchanged. How many centred frames before a lunge can occur:
    let world = TestWorld::floor();
    let after_centred = |frames: usize| {
        let mut sora = user_sora();
        run(&mut sora, &world, 60, |_| forward());
        let outs = run(&mut sora, &world, frames + 40, |i| if i < frames { Input::default() } else { stick(1.0, 0.0) });
        outs.windows(2).filter(|w| w[1].anim.play_id != w[0].anim.play_id && w[1].anim.name == AnimId::RunStartN).count()
    };
    let table: Vec<(usize, usize)> = [1usize, 2, 3, 10, 40].iter().map(|&n| (n, after_centred(n))).collect();
    println!("continuous turning: RunStartN plays after N centred frames, then a push to the right: {table:?}");
    // One centred frame: the state sees the previous tick's acceleration and only turns: no stop.
    assert_eq!(table[0], (1, 0));
    // Two or more: the stop clip has begun (or Idle was reached): exactly one run start.
    assert!(table[1..].iter().all(|&(_, plays)| plays == 1), "{table:?}");
    // From Idle: as always.
    let mut sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, 0.15), yaw: 0.0 });
    assert_eq!(run(&mut sora, &world, 3, |_| forward())[0].anim.name, AnimId::RunStartN);
}

#[test]
fn continuous_turning_walk_and_run_still_switch_speed_and_animation() {
    let world = TestWorld::floor();
    let mut sora = user_sora();
    run(&mut sora, &world, 60, |_| forward());
    // A long walk (past the 0.2 s grace), then full again: straight back to the run loop.
    let walk = run(&mut sora, &world, 40, |_| stick(0.0, 0.7));
    assert_eq!((walk[39].movement_state, walk[39].anim.name), (MovementState::Walk, AnimId::WalkLoopN));
    assert!((speed(&walk[39]) - 140.0).abs() < 0.05);
    let back = run(&mut sora, &world, 10, |_| forward());
    assert!(back.iter().all(|o| o.anim.name != AnimId::RunStartN));
    assert_eq!((back[2].movement_state, back[2].anim.name), (MovementState::Run, AnimId::RunLoopN));
    assert!((speed(&back[3]) - 680.0).abs() < 0.01, "speed {}", speed(&back[3]));
}

// ------------------------------------------------------------------ port option: jump cut on release

/// A jump with the user's options (the default), the button held for `held` frames from the
/// press. Returns (outputs, the frame he left the ground, the ground height).
fn cut_jump(held: usize) -> (Vec<Output>, usize, f32) {
    let world = TestWorld::floor();
    let mut sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, 0.15), yaw: 0.0 });
    assert!(sora.tuning().jump_cut_on_release, "the default is the user's choice");
    run(&mut sora, &world, 10, |_| Input::default());
    let ground = sora.output().position.z;
    let outs = jump(&mut sora, &world, 150, held, Vec2::ZERO);
    let takeoff = outs.iter().position(|o| !o.on_floor).expect("left the ground");
    (outs, takeoff, ground)
}

#[test]
fn jump_cut_a_held_jump_is_the_games() {
    // User decision 2026-10-11 (kh_core::Tuning::jump_cut_on_release): nothing changes while the
    // button is held: launch on frame 5, 266 cm (movement spec 4.1, 4.3).
    let (outs, takeoff, ground) = cut_jump(150);
    assert_eq!(takeoff, 5 + HOLD);
    let apex = outs.iter().map(|o| o.position.z).fold(f32::MIN, f32::max) - ground;
    assert!((apex - 266.1).abs() <= 1.0, "held: {apex} cm");
}

/// What `release_rise_speed` leaves of the rise after a release: 400^2 / (2 x 4116) = 19.4 cm.
const LEFT_CM: f32 = 400.0 * 400.0 / (2.0 * 4116.0);
/// `Tuning::min_jump_height`'s default.
const MIN_CM: f32 = 110.0;

#[test]
fn jump_cut_caps_the_rise_speed_but_always_reaches_the_minimum_height() {
    // NOT YET RUN (written while the user had asked for no test runs): expectations by formula.
    // Height above the take-off k frames after the launch tick's end: 1480 t - 2058 t^2 with
    // t = (k + 1) / 60: 46 cm at 2 frames, 68 at 4, 89 at 6 (below the minimum: he still rises
    // to 110); 127 at 6 frames after the first air frame... the test reads it from the run.
    let mut table = Vec::new();
    // The button is held through frame `held - 1`; frame `held` is the first with it up.
    for after_launch in [0usize, 2, 4, 6, 10, 15] {
        let (probe, takeoff, _) = cut_jump(150);
        drop(probe);
        let held = takeoff + after_launch;
        let (outs, takeoff, ground) = cut_jump(held);
        let first_up = held.max(takeoff + 1);
        let launch_z = outs[takeoff - 1].position.z;
        let at_release = outs[first_up - 1].position.z - launch_z;
        let top = outs.iter().map(|o| o.position.z).fold(f32::MIN, f32::max);
        let apex = outs.iter().position(|o| o.position.z == top).unwrap();
        let height = top - launch_z;
        if at_release + LEFT_CM < MIN_CM {
            // An early release: the minimum decides. The sampled apex is not below it and at
            // most g dt^2 / 8 = 0.14 cm (plus rounding) above it.
            assert!(height >= MIN_CM && height <= MIN_CM + 1.0, "released {after_launch} frames after the launch at {at_release} cm: apex {height} cm");
        } else {
            // A late release: the 400 cm/s rule: at most 19.4 cm more, and never under the minimum.
            assert!(height <= at_release + LEFT_CM + 1.0 && height >= MIN_CM.min(at_release), "released {after_launch} frames after the launch at {at_release} cm: apex {height} cm");
            assert!(outs[first_up].velocity.z <= 400.0 - 4116.0 * DT + 0.01, "released {after_launch} frames after the launch: Velocity.Z {}", outs[first_up].velocity.z);
        }
        assert!(outs.windows(2).all(|w| ((w[0].velocity.z - w[1].velocity.z) - 8232.0 * DT).abs() > 0.01), "no -8232 step: the game's short-hop gravity is replaced");
        // The game's own descent follows: the hang (-147 cm/s^2), then the plain fall (-1470).
        let fall = |i: usize| outs[i - 1].velocity.z - outs[i].velocity.z;
        assert!((apex + 1..apex + 10).any(|i| (fall(i) - 147.0 * DT).abs() < 0.01), "released {after_launch} frames after the launch: no hang phase after the apex");
        assert!((apex + 10..apex + 30).any(|i| (fall(i) - 1470.0 * DT).abs() < 0.01), "then the plain fall");
        assert!(outs[first_up..].iter().any(|o| o.anim.name == AnimId::JumpNormalDownLoop), "the fall loop is played");
        let end = outs.last().unwrap();
        assert!(end.on_floor && end.state == StateId::Idle, "he lands and returns to Idle");
        table.push((after_launch, top - ground));
    }
    println!("jump cut on release (min 110 cm, keeps 400 cm/s): height by frames the button was held after the launch: {table:?}");
    // Heights never shrink with a longer hold and stay below the full jump.
    assert!(table.windows(2).all(|w| w[1].1 >= w[0].1 - 0.5), "{table:?}");
    assert!(table.last().unwrap().1 < 266.5);
}

#[test]
fn jump_cut_a_tap_reaches_the_minimum_height_and_no_more() {
    // NOT YET RUN. The button let go before the launch: the launch still happens on the game's
    // frame (the jump was requested); from the next tick the speed is capped at what reaches
    // 110 cm above the take-off.
    let (outs, takeoff, _) = cut_jump(1);
    assert_eq!(takeoff, 5 + HOLD, "the launch frame is the game's");
    let launch_z = outs[takeoff - 1].position.z;
    let apex = outs.iter().map(|o| o.position.z).fold(f32::MIN, f32::max) - launch_z;
    println!("jump cut on release: smallest jump {apex} cm above the take-off (minimum {MIN_CM})");
    assert!(apex >= MIN_CM && apex <= MIN_CM + 5.0, "tap: {apex} cm");
    assert!(outs.last().unwrap().on_floor);
}

#[test]
fn jump_cut_with_release_speed_0_and_no_minimum_is_a_hard_cut() {
    // NOT YET RUN. The first form of the option: the height does not grow after the release.
    let world = TestWorld::floor();
    let mut sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, 0.15), yaw: 0.0 }).with_tuning(Tuning { release_rise_speed: 0.0, min_jump_height: 0.0, ..Tuning::default() });
    run(&mut sora, &world, 10, |_| Input::default());
    let outs = jump(&mut sora, &world, 150, 9, Vec2::ZERO);
    let top = outs[8].position.z;
    assert!(!outs[8].on_floor && outs[9..].iter().all(|o| o.position.z <= top + 1.0e-3), "still rising after the release");
}

#[test]
fn jump_cut_off_is_the_games_short_hop() {
    // With the option off a release keeps rising under GravityScale 3.0 (movement spec 4.2).
    let world = TestWorld::floor();
    let mut sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, 0.15), yaw: 0.0 }).with_tuning(Tuning { jump_cut_on_release: false, ..Tuning::default() });
    run(&mut sora, &world, 10, |_| Input::default());
    let outs = jump(&mut sora, &world, 120, 1, Vec2::ZERO);
    let vz: Vec<f32> = outs.iter().map(|o| o.velocity.z).collect();
    assert!((1..vz.len()).any(|i| ((vz[i - 1] - vz[i]) - 8232.0 * DT).abs() < 0.01), "the -8232 steps of the game's short hop");
    let rising_after_release = outs.windows(2).filter(|w| w[1].position.z > w[0].position.z + 1.0).count();
    assert!(rising_after_release > 5, "he keeps rising after the release: {rising_after_release} frames");
}

// ------------------------------------------------------------------ continuous turning, second form
// NOT YET RUN (written while the user had asked for no test runs): expectations by reading the code.

/// RunStartN plays in `outs` (a change of play id to it), with the axis of each on its first pose tick.
fn run_starts(outs: &[Output]) -> Vec<f32> {
    (1..outs.len()).filter(|&i| outs[i].anim.play_id != outs[i - 1].anim.play_id && outs[i].anim.name == AnimId::RunStartN).map(|i| outs[(i + 1).min(outs.len() - 1)].anim.axis).collect()
}

fn user_sora() -> Sora {
    Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, 0.15), yaw: 0.0 }).with_tuning(TURNING_ONLY)
}

#[test]
fn continuous_turning_a_slow_push_from_standstill_still_plays_the_start_lunge() {
    // The stick travels from the centre to the rim over 6 frames: the first active frames are
    // walk requests (magnitude < 0.8: Run begins in its loop, no run start), then the run
    // request: the game's walk -> run path plays RunStartN, and the option keeps it as the START.
    let world = TestWorld::floor();
    let mut sora = user_sora();
    run(&mut sora, &world, 10, |_| Input::default());
    let mut outs = vec![sora.output()];
    outs.extend(run(&mut sora, &world, 60, |i| stick(0.0, ((i + 1) as f32 / 6.0).min(1.0))));
    assert_eq!(run_starts(&outs).len(), 1, "exactly one start lunge");
    assert!(outs.last().unwrap().movement_state == MovementState::Run || outs.last().unwrap().movement_state == MovementState::Sprint);
    // The direct case: the first active frame is already a run request.
    let mut sora = user_sora();
    run(&mut sora, &world, 10, |_| Input::default());
    let mut outs = vec![sora.output()];
    outs.extend(run(&mut sora, &world, 40, |_| forward()));
    assert_eq!(run_starts(&outs).len(), 1);
    assert_eq!(outs[1].anim.name, AnimId::RunStartN);
}

#[test]
fn continuous_turning_walking_for_a_second_then_running_has_no_lunge() {
    // Past the start window (0.30 s) a walk -> run switch stays in the loop.
    let world = TestWorld::floor();
    let mut sora = user_sora();
    run(&mut sora, &world, 10, |_| Input::default());
    let mut outs = vec![sora.output()];
    outs.extend(run(&mut sora, &world, 60, |_| stick(0.0, 0.7)));
    outs.extend(run(&mut sora, &world, 40, |_| forward()));
    assert!(run_starts(&outs).is_empty(), "{:?}", run_starts(&outs));
    assert_eq!(outs.last().unwrap().anim.name, AnimId::RunLoopN);
    // ... and once he has had a run tick, a dip and a return inside the window do not lunge again.
    let mut sora = user_sora();
    run(&mut sora, &world, 10, |_| Input::default());
    let mut outs = vec![sora.output()];
    outs.extend(run(&mut sora, &world, 40, |i| if i == 16 { stick(0.0, 0.84) } else { forward() }));
    assert_eq!(run_starts(&outs).len(), 1, "only the start from Idle");
}

#[test]
fn continuous_turning_a_flick_to_the_opposite_direction_plays_the_turn_around() {
    // The game's turn-around (StopMovement + RunStartN, axis = the signed delta) for a flick:
    // the target yaw swings by 135 degrees or more within 5 ticks and is 75 or more off the facing.
    let world = TestWorld::floor();
    for travel in [1usize, 2, 3, 4] {
        for centred in [0usize, 1, 2, 3] {
            let mut sora = user_sora();
            run(&mut sora, &world, 60, |_| forward());
            // The flick: `travel` frames along the straight line through the centre (the frames
            // near the centre are inactive: magnitude < 0.1 after the dead zone), with `centred`
            // extra frames exactly at the centre, then held at the bottom.
            let path = |i: usize| -> Vec2 {
                let half = travel / 2;
                if i < half {
                    Vec2::new(0.0, 1.0 - 2.0 * (i + 1) as f32 / (travel + 1) as f32)
                } else if i < half + centred {
                    Vec2::ZERO
                } else if i < travel + centred {
                    Vec2::new(0.0, 1.0 - 2.0 * (i - centred + 1) as f32 / (travel + 1) as f32)
                } else {
                    Vec2::new(0.0, -1.0)
                }
            };
            let mut outs = vec![sora.output()];
            outs.extend(run(&mut sora, &world, travel + centred + 50, |i| Input { move_stick: path(i), ..Input::default() }));
            let starts = run_starts(&outs);
            assert_eq!(starts.len(), 1, "flick over {travel} frames with {centred} centred: RunStartN plays {starts:?}");
            assert!(starts[0].abs() >= 135.0, "flick over {travel} frames with {centred} centred: axis {}", starts[0]);
            let end = outs.last().unwrap();
            assert!((end.yaw.abs() - 180.0).abs() < 8.0 && end.velocity.x < -600.0, "he ends running the other way: yaw {} velocity {:?}", end.yaw, end.velocity);
        }
    }
}

#[test]
fn continuous_turning_rim_rolls_do_not_play_the_turn_around() {
    // Rolled round the rim over 8 frames or more (at most 45 degrees a tick raw; the dead zone
    // can make the target yaw move faster in places, but not 135 degrees within 5 ticks at these
    // speeds for the slower rolls): no RunStartN. Radius 1.0 and an Xbox pad's 1.1 (clipped per axis).
    let world = TestWorld::floor();
    for radius in [1.0f32, 1.1] {
        for total in [180.0f32, 360.0] {
            for frames in [8usize, 12, 20, 40] {
                // 135 degrees in 5 ticks = 27 degrees a tick: faster rolls are flicks by definition.
                // (The per-axis dead zone makes the target yaw sweep up to about 1.5 times faster than
            // the raw stick angle between the axes: 15 raw degrees a tick stays under 135 in 5.)
            if total / frames as f32 > 15.0 {
                    continue;
                }
                let mut sora = user_sora();
                run(&mut sora, &world, 60, |_| forward());
                let at = |i: usize| {
                    let a = (total * ((i + 1) as f32 / frames as f32).min(1.0)).to_radians();
                    Vec2::new((a.sin() * radius).clamp(-1.0, 1.0), (a.cos() * radius).clamp(-1.0, 1.0))
                };
                let mut outs = vec![sora.output()];
                outs.extend(run(&mut sora, &world, frames + 40, |i| Input { move_stick: at(i), ..Input::default() }));
                assert!(run_starts(&outs).is_empty(), "roll of {total} degrees in {frames} frames at radius {radius}: RunStartN {:?}", run_starts(&outs));
            }
        }
    }
}

// ------------------------------------------------------------------ port option: speed ramp
// NOT YET RUN (written while the user had asked for no test runs): expectations by formula.
// The limit moves by (680 - 140) / 0.25 s = 36 cm/s per 60 Hz tick: 15 ticks for the whole ramp.

/// The speed ramp without the speed-matched loop (so that the game's loop swap can be asserted).
const RAMP_ONLY: Tuning = Tuning { speed_matched_anim: false, ..Tuning::USER };

/// Walks for a second (past the start-lunge window), with the given options.
fn walking(tuning: Tuning) -> (Sora, TestWorld) {
    let world = TestWorld::floor();
    let mut sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, 0.15), yaw: 0.0 }).with_tuning(tuning);
    run(&mut sora, &world, 10, |_| Input::default());
    let outs = run(&mut sora, &world, 60, |_| stick(0.0, 0.7));
    assert!((speed(outs.last().unwrap()) - 140.0).abs() < 0.5, "walking at {}", speed(outs.last().unwrap()));
    (sora, world)
}

#[test]
fn speed_ramp_walk_to_run_rises_over_15_ticks_without_a_run_start() {
    let (mut sora, world) = walking(RAMP_ONLY);
    let outs = run(&mut sora, &world, 30, |_| forward());
    let v: Vec<f32> = outs.iter().map(speed).collect();
    println!("speed ramp, walk -> run: {:?}", v.iter().map(|x| x.round()).collect::<Vec<_>>());
    assert!(outs.iter().all(|o| o.anim.name != AnimId::RunStartN), "no run start");
    assert!(v.windows(2).all(|w| w[1] >= w[0] - 0.01), "monotonic");
    assert!(v.windows(2).all(|w| w[1] - w[0] <= 36.0 + 1.0), "at most 36 cm/s per tick");
    // The state's first run tick is the frame of the push; the movement tick of the next frame
    // has the first step of the limit: half way after about 8 frames, done after 15 to 17.
    assert!(v[8] > 330.0 && v[8] < 480.0, "half way: {}", v[8]);
    assert!((v[17] - 680.0).abs() < 0.5 && v[13] < 680.0 - 1.0, "done by frame 17, not by 13: {} / {}", v[17], v[13]);
    assert_eq!(outs[3].anim.name, AnimId::RunLoopN, "the loop animation swaps at the switch, as in the game");
}

#[test]
fn speed_ramp_run_to_walk_falls_over_15_ticks_after_the_grace() {
    let world = TestWorld::floor();
    let mut sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, 0.15), yaw: 0.0 }).with_tuning(RAMP_ONLY);
    run(&mut sora, &world, 50, |_| forward());
    let outs = run(&mut sora, &world, 50, |_| stick(0.0, 0.7));
    let v: Vec<f32> = outs.iter().map(speed).collect();
    println!("speed ramp, run -> walk: {:?}", v.iter().map(|x| x.round()).collect::<Vec<_>>());
    // The game's run grace: 0.2 s = 12 ticks at run speed; then the ramp.
    assert!((v[10] - 680.0).abs() < 0.5, "still at run speed inside the grace: {}", v[10]);
    assert!(v.windows(2).all(|w| w[1] <= w[0] + 0.01), "monotonic");
    assert!(v.windows(2).all(|w| w[0] - w[1] <= 36.0 + 1.0), "at most 36 cm/s per tick");
    assert!(v[20] < 680.0 - 100.0 && v[20] > 140.0 + 100.0, "on the way down at frame 20: {}", v[20]);
    assert!((v[32] - 140.0).abs() < 0.5, "at walk speed by frame 32: {}", v[32]);
    assert!(outs.iter().all(|o| o.anim.name != AnimId::RunStartN && o.state == StateId::Run));
}

#[test]
fn speed_ramp_a_switch_back_mid_ramp_ramps_back_from_where_it_is() {
    // Run -> walk: the game's 0.2 s grace (12 ticks) first, then the ramp down; seven ticks into
    // it the stick is back at full: the ramp turns round at once (walk -> run has no grace).
    // (A walk -> run ramp cannot be cut short by a walk request within 0.2 s: the grace keeps the
    // run gait, so that ramp completes; measured: 140 -> 680 in 15 ticks, down again from tick 21.)
    let world = TestWorld::floor();
    let mut sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, 0.15), yaw: 0.0 }).with_tuning(RAMP_ONLY);
    run(&mut sora, &world, 50, |_| forward());
    let outs = run(&mut sora, &world, 60, |i| if i < 19 { stick(0.0, 0.7) } else { forward() });
    let v: Vec<f32> = outs.iter().map(speed).collect();
    println!("speed ramp, reversal mid-ramp: {:?}", v.iter().map(|x| x.round()).collect::<Vec<_>>());
    assert!(v.windows(2).all(|w| (w[1] - w[0]).abs() <= 36.0 + 1.0), "never more than one ramp step per tick");
    let low = v.iter().cloned().fold(f32::MAX, f32::min);
    assert!(low > 140.0 + 100.0 && low < 680.0 - 150.0, "the ramp down was cut short at {low}");
    assert!((v[59] - 680.0).abs() < 0.5, "back at run speed: {}", v[59]);
    assert!(outs.iter().all(|o| o.anim.name != AnimId::RunStartN));
}

#[test]
fn speed_ramp_off_is_the_games_jump_of_the_limit() {
    let (mut sora, world) = walking(Tuning { speed_ramp: false, ..RAMP_ONLY });
    let outs = run(&mut sora, &world, 6, |_| forward());
    // The state's run tick is the frame of the push; the next movement tick has the full limit
    // and MaxAcceleration = MaxWalkSpeed x 60 reaches it in that tick.
    assert!((speed(&outs[1]) - 680.0).abs() < 0.5, "one tick: {:?}", outs.iter().map(speed).collect::<Vec<_>>());
}

#[test]
fn speed_ramp_does_not_touch_the_start_the_sprint_or_the_stop() {
    let world = TestWorld::floor();
    let mut sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, 0.15), yaw: 0.0 });
    let outs = run(&mut sora, &world, 130, |_| forward());
    // From Idle: RunStartN, then the loop at run speed in one tick (as in the game).
    let loop_start = outs.iter().position(|o| o.movement_state == MovementState::Run).unwrap();
    assert!((speed(&outs[loop_start + 1]) - 680.0).abs() < 0.01, "first loop tick: {}", speed(&outs[loop_start + 1]));
    // Sprint: 980 in the tick after the timer passes 1.0 s.
    let sprint = outs.iter().position(|o| o.movement_state == MovementState::Sprint).expect("Sprint");
    assert!((speed(&outs[sprint + 1]) - 980.0).abs() < 0.01);
    // Stop: one braking tick, then the stop clip zeroes the velocity.
    let outs = run(&mut sora, &world, 4, |_| Input::default());
    assert_eq!(speed(&outs[1]), 0.0);
}

// ------------------------------------------------------------------ port option: speed matched animation
// NOT YET RUN (written while the user had asked for no test runs): expectations by formula.

const WALK_SPEED: f32 = 140.0;
const RUN_SPEED: f32 = 680.0;

fn cycle_lengths() -> (f32, f32) {
    (AnimId::WalkLoopN.data().length, AnimId::RunLoopN.data().length)
}

#[test]
fn speed_matched_at_the_walk_and_the_run_speed_each_clip_plays_at_rate_1() {
    // User decision 2026-10-11 (kh_core::Tuning::speed_matched_anim).
    let (walk_length, run_length) = cycle_lengths();
    let (mut sora, world) = walking(Tuning::default());
    let out = sora.output();
    let l = out.anim.locomotion.expect("the locomotion loop plays while walking");
    assert_eq!((l.run_weight, out.anim.name), (0.0, AnimId::WalkLoopN));
    assert!((l.rate * walk_length - 1.0).abs() < 1.0e-3, "walk: play rate {}", l.rate * walk_length);
    assert!((l.walk_time - l.phase * walk_length).abs() < 1.0e-5);
    // Run: the ramp takes him to 680; then weight 1 and the run cycle at rate 1.
    let id = out.anim.play_id;
    let outs = run(&mut sora, &world, 40, |_| forward());
    let end = outs.last().unwrap();
    let l = end.anim.locomotion.expect("still the locomotion loop");
    assert_eq!((l.run_weight, end.anim.name), (1.0, AnimId::RunLoopN));
    assert!((l.rate * run_length - 1.0).abs() < 1.0e-3, "run: play rate {}", l.rate * run_length);
    // One continuous loop: no play call when the gait changed, the slot's weight stays 1.
    assert!(outs.iter().all(|o| o.anim.play_id == id && o.anim.weight == 1.0 && o.anim.active), "a play call during the gait change");
    // The run side is read at the shared phase plus the measured shift of the two cycles.
    let expected = (l.phase + kh_core::anim::RUN_PHASE_SHIFT).rem_euclid(1.0) * run_length;
    assert!((l.run_time - expected).abs() < 1.0e-5);
}

#[test]
fn speed_matched_the_phase_advances_by_speed_over_stride() {
    let (walk_length, run_length) = cycle_lengths();
    let (mut sora, world) = walking(Tuning::default());
    let mut before = sora.output();
    let mut checked = 0;
    for out in run(&mut sora, &world, 30, |_| forward()) {
        let (a, b) = (before.anim.locomotion.unwrap(), out.anim.locomotion.unwrap());
        // The pose tick of this frame used the rate the state set at the end of the previous one.
        let advance = (b.phase - a.phase).rem_euclid(1.0);
        assert!((advance - a.rate * DT).abs() < 1.0e-5, "phase advance {advance}, rate x dt {}", a.rate * DT);
        // The state then sets weight and rate from his speed after this frame's move.
        let v = speed(&out);
        let weight = ((v - WALK_SPEED) / (RUN_SPEED - WALK_SPEED)).clamp(0.0, 1.0);
        let stride = WALK_SPEED * walk_length + (RUN_SPEED * run_length - WALK_SPEED * walk_length) * weight;
        assert!((b.run_weight - weight).abs() < 1.0e-3, "weight {} at {v} cm/s", b.run_weight);
        assert!((b.rate - v.max(WALK_SPEED) / stride).abs() < 1.0e-3, "rate {} at {v} cm/s", b.rate);
        // The dominant side names the animation.
        assert_eq!(out.anim.name, if b.run_weight >= 0.5 { AnimId::RunLoopN } else { AnimId::WalkLoopN });
        checked += (weight > 0.05 && weight < 0.95) as usize;
        before = out;
    }
    assert!(checked >= 8, "the ramp was sampled mid-way {checked} times");
}

#[test]
fn speed_matched_the_sprint_keeps_the_games_playback_unless_asked() {
    let (_, run_length) = cycle_lengths();
    let world = TestWorld::floor();
    let sprint = |tuning: Tuning| {
        let mut sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, 0.15), yaw: 0.0 }).with_tuning(tuning);
        let out = run(&mut sora, &world, 130, |_| forward()).last().unwrap().clone();
        assert!((speed(&out) - 980.0).abs() < 0.01);
        out.anim.locomotion.expect("the locomotion loop")
    };
    let l = sprint(Tuning::default());
    assert!(l.run_weight == 1.0 && (l.rate * run_length - 1.0).abs() < 1.0e-3, "sprint: play rate {}", l.rate * run_length);
    let l = sprint(Tuning { speed_match_sprint: true, ..Tuning::default() });
    assert!((l.rate * run_length - 980.0 / 680.0).abs() < 1.0e-3, "sprint matched: play rate {}", l.rate * run_length);
}

#[test]
fn speed_matched_outside_the_loop_and_with_the_option_off_it_is_the_games_playback() {
    let world = TestWorld::floor();
    // Outside the loop: the run start, the stop and the idle have no locomotion blend.
    let mut sora = Sora::new(SORA_PARAMS, Pose { position: Vec3::new(0.0, 0.0, 0.15), yaw: 0.0 });
    let mut outs = run(&mut sora, &world, 40, |_| forward());
    outs.extend(run(&mut sora, &world, 50, |_| Input::default()));
    for o in &outs {
        let in_loop = matches!(o.movement_state, MovementState::Run | MovementState::Sprint | MovementState::Walk) && matches!(o.anim.name, AnimId::RunLoopN | AnimId::WalkLoopN);
        assert_eq!(o.anim.locomotion.is_some(), in_loop, "{:?} with {:?}", o.movement_state, o.anim.name);
    }
    // Entering the loop after the run start is a play call with the loop's 0.2 s blend-in, in the run gait.
    let first = outs.iter().find(|o| o.anim.locomotion.is_some()).unwrap();
    assert_eq!((first.anim.name, first.anim.locomotion.unwrap().run_weight, first.anim.weight), (AnimId::RunLoopN, 1.0, 0.0));
    // Off: the game's swap: a play call at the switch, no locomotion blend.
    let (mut sora, world) = walking(Tuning { speed_matched_anim: false, ..Tuning::default() });
    let id = sora.output().anim.play_id;
    let outs = run(&mut sora, &world, 20, |_| forward());
    assert!(outs.iter().all(|o| o.anim.locomotion.is_none()));
    assert!(outs[1].anim.play_id != id && outs[1].anim.name == AnimId::RunLoopN, "the game's play call at the switch");
}
