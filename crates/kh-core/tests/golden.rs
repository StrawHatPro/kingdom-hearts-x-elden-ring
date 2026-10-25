//! REGRESSION GUARD, not a specification test: two long pseudo-random sessions (20 000 steps
//! each: stick held in random directions and magnitudes, taps and held jumps, a drifting camera,
//! boxes and ledges of several heights to run into, climb, fall off and land on), once with the
//! game's rules (`Tuning::GAME_EXACT`) and once with the user's options (`Tuning::USER`), hashed
//! over every bit of every `Output`.
//!
//! The two constants were produced by the code as it stood BEFORE the state-machine host was
//! generalised (round 4, 2026-10-11: `states.rs` with `enum State { Idle, Run, Jump }`), so a
//! refactoring that is meant to change no behaviour must leave them alone. A change that is
//! MEANT to alter Idle / Run / Jump, the movement component or the animation clock has to update
//! them and say so in its report (run with `-- --nocapture` to see the new values).

use kh_core::test_world::TestWorld;
use kh_core::{AnimRequest, Input, Output, Pose, Sora, Tuning, Vec2, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;
const STEPS: usize = 20_000;

const GOLDEN_GAME_EXACT: u64 = 0x82dfab5569952659;
const GOLDEN_USER: u64 = 0x3b56dfcf6b73d286;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }

    /// 0 <= value < 1, in steps of 1 / 1024 (exact in f32).
    fn unit(&mut self) -> f32 {
        (self.next() % 1024) as f32 / 1024.0
    }
}

struct Hash(u64);

impl Hash {
    fn u32(&mut self, v: u32) {
        for byte in v.to_le_bytes() {
            self.0 = (self.0 ^ byte as u64).wrapping_mul(0x100000001b3);
        }
    }

    fn f32(&mut self, v: f32) {
        self.u32(v.to_bits());
    }

    fn vec3(&mut self, v: Vec3) {
        self.f32(v.x);
        self.f32(v.y);
        self.f32(v.z);
    }

    fn anim(&mut self, a: &AnimRequest) {
        self.u32(a.name as u32);
        self.u32(a.play_id);
        self.f32(a.time);
        self.u32(a.active as u32);
        self.f32(a.weight);
        self.u32(a.looping as u32);
        self.f32(a.blend_in);
        self.f32(a.axis);
        self.u32(a.root_motion as u32);
        self.f32(a.notify_window.0);
        self.f32(a.notify_window.1);
        match a.locomotion {
            None => self.u32(0),
            Some(l) => {
                self.u32(1);
                self.f32(l.run_weight);
                self.f32(l.phase);
                self.f32(l.rate);
                self.f32(l.walk_time);
                self.f32(l.run_time);
            }
        }
    }

    fn output(&mut self, o: &Output) {
        self.vec3(o.position);
        self.vec3(o.capsule_center);
        self.f32(o.yaw);
        self.vec3(o.velocity);
        self.u32(o.state as u32);
        self.u32(o.sub_step as u32);
        self.u32(o.movement_state as u32);
        self.u32(o.on_floor as u32);
        self.vec3(o.displacement_velocity);
        self.anim(&o.anim);
        self.anim(&o.anim_ticked);
    }
}

/// A floor with things to meet within a few metres of the start: kerbs, steps at the step-up
/// limit, a wall-high box, a tall platform with a lower one beside it, a pit edge.
fn world() -> TestWorld {
    TestWorld::floor()
        .with_box(Vec3::new(300.0, -200.0, 0.0), Vec3::new(500.0, 200.0, 30.0))
        .with_box(Vec3::new(500.0, -200.0, 0.0), Vec3::new(700.0, 200.0, 77.0))
        .with_box(Vec3::new(-700.0, -300.0, 0.0), Vec3::new(-400.0, 300.0, 100.0))
        .with_box(Vec3::new(-1000.0, -300.0, 0.0), Vec3::new(-700.0, 300.0, 200.0))
        .with_box(Vec3::new(-200.0, 400.0, 0.0), Vec3::new(200.0, 600.0, 48.0))
        .with_box(Vec3::new(-200.0, 600.0, 0.0), Vec3::new(200.0, 900.0, 400.0))
        .with_box(Vec3::new(-300.0, -900.0, 0.0), Vec3::new(300.0, -500.0, 250.0))
        .with_box(Vec3::new(900.0, -1200.0, -50.0), Vec3::new(1000.0, 1200.0, 600.0))
}

fn session(tuning: Tuning, seed: u64) -> u64 {
    let mut seen = std::collections::BTreeMap::new();
    let world = world();
    let start = Pose { position: Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 };
    let mut sora = Sora::new(SORA_PARAMS, start).with_tuning(tuning);
    let mut rng = Rng(seed);
    let mut hash = Hash(0xcbf29ce484222325);
    let (mut stick, mut stick_left) = (Vec2::ZERO, 0u32);
    let (mut held_left, mut camera_yaw, mut camera_rate) = (0u32, 0.0f32, 0.0f32);
    for step in 0..STEPS {
        if stick_left == 0 {
            // A new stick position, held for 1 to 90 steps: centred, a walk, a run, or anything.
            stick_left = 1 + rng.next() % 90;
            let angle = rng.unit() * std::f32::consts::TAU;
            let radius = match rng.next() % 8 {
                0 | 1 => 0.0,
                2 => 0.6,
                3 => rng.unit() * 1.1,
                _ => 1.0,
            };
            stick = Vec2::new(angle.sin() * radius, angle.cos() * radius);
            camera_rate = (rng.unit() - 0.5) * 3.0;
        }
        stick_left -= 1;
        let mut pressed = false;
        if held_left > 0 {
            held_left -= 1;
        } else if rng.next() % 70 == 0 {
            pressed = true;
            held_left = match rng.next() % 4 {
                0 => 0,
                1 => 3,
                2 => 12,
                _ => 40,
            };
        }
        camera_yaw += camera_rate;
        let input = Input { move_stick: stick, jump_pressed: pressed, jump_held: pressed || held_left > 0, camera_yaw, ..Input::default() };
        let out = sora.step(DT, &input, &world);
        hash.output(&out);
        *seen.entry((out.movement_state.name(), out.sub_step, out.anim.name.name())).or_insert(0u32) += 1;
        // Far out or fallen through: back to the start (the same in every run).
        if out.position.length() > 2500.0 || step % 5000 == 4999 {
            sora.teleport(start);
            sora.set_tuning(tuning);
        }
    }
    println!("golden: visited {seen:?}");
    hash.0
}

#[test]
fn idle_run_jump_are_bit_identical_to_the_round_3_implementation() {
    let (exact, user) = (session(Tuning::GAME_EXACT, 0x1234_5678_9abc_def0), session(Tuning::USER, 0x0fed_cba9_8765_4321));
    println!("golden: GAME_EXACT {exact:#018x}  USER {user:#018x}");
    assert_eq!((exact, user), (GOLDEN_GAME_EXACT, GOLDEN_USER), "Idle / Run / Jump no longer behave bit for bit as before (see this file's header)");
}
