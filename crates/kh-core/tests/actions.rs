//! The action states: docs/sora-actions-spec.md Part II (II.0.4 tier 1; the tests of II.1, II.2
//! and II.3, cited by their numbers). 60 Hz fixed step, the level-99 loadout (every ability),
//! plain Keyblade, flat floor at Z = 0, `Tuning::GAME_EXACT` unless a test says otherwise.
//! "Frame N" = the step whose controller sees the press; `S(N)` begins the requested state, so
//! the output of step N already shows it. "After M(N + k)" = the output of step N + k.

use kh_core::enums::{ETresAbilityKind as Ability, ETresDamageKind as Kind};
use kh_core::host::{EventKind, NullHost, TargetInfo};
use kh_core::states::HitOnPlayer;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{AnimId, HitRecord, HitResult, HurtShape, Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, Sora, StateId, Status, TargetId, Targets, Tuning, Vec2, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;
const FORWARD: Vec2 = Vec2::new(0.0, 1.0);
const RIGHT: Vec2 = Vec2::new(1.0, 0.0);

fn cell(json: &serde_json::Value) -> Option<LoadoutValue> {
    match json {
        serde_json::Value::Number(n) => n.as_f64().map(LoadoutValue::Number),
        serde_json::Value::String(s) => Some(LoadoutValue::Text(s.clone())),
        serde_json::Value::Array(items) => items.iter().map(|i| i.as_str().map(str::to_string)).collect::<Option<Vec<_>>>().map(LoadoutValue::List),
        serde_json::Value::Object(map) => map.iter().map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect::<Option<Vec<_>>>().map(LoadoutValue::Map),
        _ => None,
    }
}

fn level99() -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        loadout.set(row[0].as_str().unwrap(), &cell(&row[1]).unwrap()).unwrap();
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
}

/// A raw stick value whose magnitude after the game's 0.25 dead zone is `magnitude`.
fn raw(direction: Vec2, magnitude: f32) -> Vec2 {
    direction * (0.25 + magnitude * 0.75)
}

fn buttons(list: &[PadButton]) -> PadButtons {
    list.iter().fold(PadButtons::NONE, |set, b| set.with(*b))
}

/// An input: the stick, the buttons pressed this step, the buttons held (the pressed ones too).
fn input(stick: Vec2, pressed: &[PadButton], held: &[PadButton]) -> Input {
    let pressed = buttons(pressed);
    Input { move_stick: stick, ..Input::default() }.with_pad(PadState { held: PadButtons(buttons(held).0 | pressed.0), pressed, released: PadButtons::NONE })
}

fn idle() -> Input {
    Input::default()
}

fn guard(stick: Vec2) -> Input {
    input(stick, &[PadButton::GUARD], &[])
}

fn jump_press(stick: Vec2) -> Input {
    input(stick, &[PadButton::JUMP], &[])
}

fn jump_hold(stick: Vec2) -> Input {
    input(stick, &[], &[PadButton::JUMP])
}

struct Bench {
    sora: Sora,
    world: TestWorld,
    outs: Vec<Output>,
}

impl Bench {
    fn with(status: Status, world: TestWorld, position: Vec3, tuning: Tuning) -> Bench {
        let origin = position + Vec3::Z * Sora::origin_height_on_floor(&SORA_PARAMS);
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(tuning).with_status(status);
        let mut bench = Bench { sora, world, outs: Vec::new() };
        bench.run(5);
        bench
    }

    fn new() -> Bench {
        Bench::with(level99(), TestWorld::floor(), Vec3::ZERO, Tuning::GAME_EXACT)
    }

    fn step(&mut self, input: Input) -> Output {
        let out = self.sora.step(DT, &input, &self.world);
        self.outs.push(out.clone());
        out
    }

    fn run(&mut self, steps: usize) -> Output {
        self.hold(idle(), steps)
    }

    fn hold(&mut self, input: Input, steps: usize) -> Output {
        for _ in 0..steps {
            self.step(input);
        }
        self.last().clone()
    }

    /// Steps with `input` until `done`; returns the steps taken (the step that satisfied it is 1).
    fn until(&mut self, input: Input, limit: usize, done: impl Fn(&Output, &Sora) -> bool) -> usize {
        for k in 1..=limit {
            let out = self.step(input);
            if done(&out, &self.sora) {
                return k;
            }
        }
        panic!("not reached within {limit} steps: state {:?} sub {} at {:?}", self.last().state, self.last().sub_step, self.last().position);
    }

    fn last(&self) -> &Output {
        self.outs.last().unwrap()
    }

    /// Jumps (button held) and steps to the first falling tick of the Jump state (sub-step 4).
    fn jump_to_apex(&mut self) -> Output {
        self.step(jump_press(Vec2::ZERO));
        self.until(jump_hold(Vec2::ZERO), 120, |o, _| o.state == StateId::Jump && o.sub_step == 4);
        self.last().clone()
    }
}

/// A hit of `kind` from an attacker 150 cm away in direction `from_yaw` (degrees from his
/// facing +X), pushing him away from it.
fn hit_from(from_yaw: f32, kind: Kind, amount: f32, own: Vec3) -> HitOnPlayer {
    let a = from_yaw.to_radians();
    let to = Vec3::new(a.cos(), a.sin(), 0.0);
    HitOnPlayer { direction: -to, causer_yaw: from_yaw + 180.0, ..HitOnPlayer::new(kind, amount, TargetId(1), own + to * 150.0) }
}

fn near(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance
}

// ============================================================================ II.1 Dodge Roll

#[test]
fn dodge_roll_travels_600_cm_on_the_root_track_of_0051() {
    // II.1 test 1: state 16 from S(N); X after M(N + k) = the root track at k / 60 s; final 600 cm;
    // reaction type 11 from S(N), default from S(N + 28); Idle from S(N + 42) (+- 1).
    let mut bench = Bench::new();
    let x0 = bench.last().position.x;
    let out = bench.step(guard(FORWARD));
    assert_eq!((out.state, bench.sora.actions().reaction), (StateId::Dodgeroll, 11));
    let mut restored = None;
    let mut ended = None;
    for k in 1..=60 {
        let out = bench.step(idle());
        let x = out.position.x - x0;
        match k {
            2 => assert!(near(x, 45.57, 0.6), "M(N+2): {x}"),
            8 => assert!(near(x, 234.27, 0.6), "M(N+8): {x}"),
            20 => assert!(near(x, 479.12, 0.6), "M(N+20): {x}"),
            38 => assert!(near(x, 600.0, 0.6), "M(N+38): {x}"),
            _ => {}
        }
        if restored.is_none() && bench.sora.actions().reaction == 0 {
            restored = Some(k);
        }
        if ended.is_none() && out.state == StateId::Idle {
            ended = Some(k);
        }
    }
    assert!(near(bench.last().position.x - x0, 600.0, 0.6), "the roll is {} cm", bench.last().position.x - x0);
    assert_eq!(out.anim.name, AnimId::DodgeRoll);
    // TresEnableChange at 0.4659 s fires in M(N + 28); the tick of S(N + 28) restores the type.
    assert_eq!(restored, Some(28), "II.1: i-frames from S(N) to S(N + 28)");
    assert!(matches!(ended, Some(42 | 43)), "the 0.7 s clip ends -> Idle at S(N + 42): {ended:?}");
}

#[test]
fn dodge_roll_faces_the_stick_only_above_half_tilt() {
    // II.1 test 2 (Push 0x141444A50: comiss mag,[0.5]; jbe skip): magnitude 0.5 at yaw 90: the
    // roll goes along the current facing; 0.51: he snaps to yaw 90 in S(N) and rolls along +Y.
    let mut bench = Bench::new();
    let out = bench.step(guard(raw(RIGHT, 0.5)));
    assert_eq!((out.state, out.yaw), (StateId::Dodgeroll, 0.0));
    bench.run(45);
    assert!(bench.last().position.x > 590.0 && bench.last().position.y.abs() < 1.0, "{:?}", bench.last().position);
    let mut bench = Bench::new();
    let out = bench.step(guard(raw(RIGHT, 0.51)));
    assert_eq!(out.state, StateId::Dodgeroll);
    assert!(near(out.yaw, 90.0, 0.01), "snapped in S(N): {}", out.yaw);
    bench.run(45);
    assert!(bench.last().position.y > 590.0 && bench.last().position.x.abs() < 1.0, "{:?}", bench.last().position);
}

#[test]
fn hits_do_not_exist_during_the_i_frames_of_a_dodge_roll() {
    // II.1 test 3: hits resolved in frames N + 1 ... N + 27 produce no damage and no reaction;
    // frame N + 29 produces damage.
    for (frame, dodged) in [(1usize, true), (14, true), (27, true), (29, false)] {
        let mut bench = Bench::new();
        let hp = bench.sora.status().hp;
        bench.step(guard(FORWARD));
        bench.run(frame - 1);
        let own = bench.last().position;
        bench.sora.take_hit(hit_from(0.0, Kind::Small, 30.0, own));
        let out = bench.step(idle());
        let dropped = out.events.iter().any(|e| matches!(e.kind, EventKind::PlayerDodged { .. }));
        assert_eq!(dropped, dodged, "frame N + {frame}");
        assert_eq!(bench.sora.status().hp, if dodged { hp } else { hp - 30 }, "frame N + {frame}");
        assert_eq!(out.state == StateId::Damage, !dodged, "frame N + {frame}: {:?}", out.state);
    }
}

#[test]
fn dodge_roll_gates_cancel_no_second_roll_no_roll_out_of_jump_and_the_ability() {
    // II.1 test 4: a guard press (stick neutral) at C(N + 20) is refused (m_bEnableChange clear),
    // at C(N + 29) it is accepted; a dodge press at any frame of a roll is refused.
    let mut bench = Bench::new();
    bench.step(guard(FORWARD));
    bench.run(19);
    assert_eq!(bench.step(guard(Vec2::ZERO)).state, StateId::Dodgeroll);
    assert_eq!(bench.step(guard(FORWARD)).state, StateId::Dodgeroll, "no roll out of a roll");
    bench.run(7);
    assert_eq!(bench.outs.len(), 5 + 29);
    assert_eq!(bench.step(guard(FORWARD)).state, StateId::Dodgeroll, "no roll out of a roll after TresEnableChange either");
    let mut bench = Bench::new();
    bench.step(guard(FORWARD));
    bench.run(28);
    assert_eq!(bench.step(guard(Vec2::ZERO)).state, StateId::Guard, "C(N + 29): the guard is accepted");
    // II.1 test 5: `TryDodge` returns false for state JUMP: no roll during a landing clip; the
    // same press on the first Idle frame rolls.
    let mut bench = Bench::new();
    bench.step(jump_press(Vec2::ZERO));
    bench.until(idle(), 200, |o, _| o.state == StateId::Jump && o.sub_step == 11);
    assert_eq!(bench.step(guard(FORWARD)).state, StateId::Jump);
    bench.until(idle(), 60, |o, _| o.state == StateId::Idle);
    assert_eq!(bench.step(guard(FORWARD)).state, StateId::Dodgeroll);
    // Ability DODGE (22): without it the request is refused (0x141427DA5) and nothing else
    // happens on that press (with the stick active the button means dodge only).
    let mut bench = Bench::new();
    bench.sora.status_mut().abilities.set(Ability::Dodge, 1, 0);
    assert_eq!(bench.step(guard(FORWARD)).state, StateId::Run, "no DODGE: the press does nothing, the stick runs");
}

// ============================================================================ II.1 Guard

#[test]
fn guard_holds_its_parts_for_0_6_s_and_ends_with_the_clip() {
    // II.1 test 6: state 15 from S(N); the parts `Guard` / `GuardBody` on from S(N) until
    // M(N + 36) (TresTiming(0) at 0.6 s); velocity zero; Idle from S(N + 52); a jump press is
    // refused (the gate needs a guarded hit).
    let mut bench = Bench::new();
    let out = bench.step(guard(Vec2::ZERO));
    assert_eq!(out.state, StateId::Guard);
    assert!(bench.sora.actions().guard_parts && bench.sora.actions().reaction == 0 && bench.sora.actions().weapon_out);
    assert_eq!(bench.sora.actions().last_pose, 1, "Guard::Begin writes the BATTLE pose");
    let (mut off, mut ended) = (None, None);
    for k in 1..=60 {
        let out = bench.step(if k == 20 { jump_press(Vec2::ZERO) } else { idle() });
        if out.state == StateId::Guard {
            assert_eq!(out.velocity, Vec3::ZERO);
            assert_eq!(out.anim.name, AnimId::Guard);
        }
        if off.is_none() && !bench.sora.actions().guard_parts {
            off = Some(k);
        }
        if ended.is_none() && out.state != StateId::Guard {
            assert_eq!(out.state, StateId::Idle, "the jump press at N + 20 must not start a jump");
            ended = Some(k);
        }
    }
    assert_eq!(off, Some(36), "guard window: Begin -> M(N + 36)");
    assert!(matches!(ended, Some(52 | 53)), "clip 0.8667 s -> Idle at S(N + 52): {ended:?}");
    // Ability REFLECT_GUARD (3): without it the press does nothing.
    let mut bench = Bench::new();
    bench.sora.status_mut().abilities.set(Ability::ReflectGuard, 1, 0);
    assert_eq!(bench.step(guard(Vec2::ZERO)).state, StateId::Idle);
}

#[test]
fn a_guarded_hit_from_the_front_costs_no_hp_and_pushes_back_300_decaying() {
    // II.1 test 7: a hit from +X (damage kind 1, power scale 1) at frame N + 10: no HP loss; Guard
    // re-entered at the next S (H) with clip `GuardHit`; push-back 300 -> 285 cm/s along the hit
    // direction in S(H), x 0.95 per tick (about 90 cm by the clip end); reaction type 3 on the
    // body until M(H + 42); the counter window 1.0 s. Damage kind 2: 1000 (first write 950).
    for (kind, first, travel) in [(Kind::Small, 285.0, 90.0), (Kind::Blow, 950.0, 300.0)] {
        let mut bench = Bench::new();
        let hp = bench.sora.status().hp;
        let mp = bench.sora.status().mp;
        bench.sora.status_mut().set_mp(mp - 10);
        bench.step(guard(Vec2::ZERO));
        bench.run(9);
        let own = bench.last().position;
        bench.sora.take_hit(hit_from(0.0, kind, 40.0, own));
        let out = bench.step(idle());
        assert_eq!((out.state, out.anim.name), (StateId::Guard, AnimId::GuardHit), "{kind:?}");
        assert_eq!(bench.sora.status().hp, hp, "no HP loss");
        assert!(out.events.iter().any(|e| matches!(e.kind, EventKind::PlayerGuarded { success: true, .. })));
        assert!(near(out.velocity.x, -first, 0.01) && out.velocity.y.abs() < 0.01, "first write: {:?}", out.velocity);
        let a = bench.sora.actions();
        assert_eq!((a.reaction, a.guard_parts, a.counter_command), (3, false, 3), "hit entry: type 3 on Body; GUARD_COUNTER only: command id 3");
        assert!(near(a.counter_window, 1.0, 0.02) && a.counter_attacker == TargetId(1));
        // GUARD_REGENE (74): Recover(0, 3, 0, 0).
        assert_eq!(bench.sora.status().mp, mp - 10 + 3, "Block Replenisher");
        let next = bench.step(idle());
        assert!(near(next.velocity.x, -first * 0.95, 0.01) && near(next.displacement_velocity.x, -first, 0.5), "{:?}", next.velocity);
        let mut restored = None;
        for k in 2..=70 {
            let out = bench.step(idle());
            if restored.is_none() && bench.sora.actions().reaction == 0 {
                restored = Some(k);
            }
            if out.state != StateId::Guard {
                break;
            }
        }
        assert_eq!(restored, Some(42), "the hit clip's TresEnableChange (0.7 s) in M(H + 42)");
        assert_eq!(bench.last().state, StateId::Idle);
        let pushed = own.x - bench.last().position.x;
        assert!(near(pushed, travel, travel * 0.06), "{kind:?}: pushed {pushed} cm");
    }
    // The command id by ability (FUN_1414145C0): 5 with REVENGEDIVE, else 4 with REVENGEIMPACT.
    for (ability, id) in [(Ability::Revengedive, 5), (Ability::Revengeimpact, 4)] {
        let mut bench = Bench::new();
        bench.sora.status_mut().abilities.set(ability, 1, 1);
        bench.step(guard(Vec2::ZERO));
        bench.run(9);
        let own = bench.last().position;
        bench.sora.take_hit(hit_from(0.0, Kind::Small, 40.0, own));
        bench.step(idle());
        assert_eq!(bench.sora.actions().counter_command, id);
    }
}

#[test]
fn the_guard_covers_120_degrees_either_side_and_only_inside_its_window() {
    // II.1 test 8 ("Guard direction" [D]): from 110 degrees off the facing: guarded; from 130
    // degrees and from behind: a normal hit (the weak-guard flag clear).
    for (from, guarded) in [(0.0f32, true), (110.0, true), (-110.0, true), (130.0, false), (-130.0, false), (180.0, false)] {
        let mut bench = Bench::new();
        let hp = bench.sora.status().hp;
        bench.step(guard(Vec2::ZERO));
        bench.run(9);
        let own = bench.last().position;
        bench.sora.take_hit(hit_from(from, Kind::Small, 30.0, own));
        let out = bench.step(idle());
        assert_eq!(bench.sora.status().hp == hp, guarded, "from {from} degrees");
        assert_eq!(out.state, if guarded { StateId::Guard } else { StateId::Damage }, "from {from} degrees");
    }
    // After TresTiming(0) (0.6 s, M(N + 36)) the parts are off: a hit at N + 40 is a normal hit.
    let mut bench = Bench::new();
    let hp = bench.sora.status().hp;
    bench.step(guard(Vec2::ZERO));
    bench.run(39);
    let own = bench.last().position;
    bench.sora.take_hit(hit_from(0.0, Kind::Small, 30.0, own));
    assert_eq!((bench.step(idle()).state, bench.sora.status().hp), (StateId::Damage, hp - 30));
}

#[test]
fn exits_from_the_guard_hit_clip_open_at_its_enable_input() {
    // II.1 test 9: TresEnableInput at 0.2667 s (M(H + 16)): a dodge press at C(H + 16) is refused,
    // at C(H + 17) accepted (the roll begins S(H + 17)). A jump press at C(H + 17) is accepted and,
    // starts AT ONCE (coordinator decision: `JumpRequest` calls `ClearQueue`, which drops the running Guard's entry).
    let setup = || {
        let mut bench = Bench::new();
        bench.step(guard(Vec2::ZERO));
        bench.run(9);
        let own = bench.last().position;
        bench.sora.take_hit(hit_from(0.0, Kind::Small, 30.0, own));
        assert_eq!(bench.step(idle()).anim.name, AnimId::GuardHit); // frame H
        bench
    };
    let mut bench = setup();
    bench.run(15);
    assert_eq!(bench.step(guard(FORWARD)).state, StateId::Guard, "C(H + 16): refused");
    assert_eq!(bench.step(guard(FORWARD)).state, StateId::Dodgeroll, "C(H + 17): accepted");
    let mut bench = setup();
    bench.run(15);
    assert_eq!(bench.step(jump_press(Vec2::ZERO)).state, StateId::Guard, "C(H + 16): the jump is refused");
    let mut bench = setup();
    bench.run(16);
    assert_eq!(bench.step(jump_press(Vec2::ZERO)).state, StateId::Jump, "C(H + 17): accepted, the jump begins S(H + 17)");
    // The counter window is still open when the guard ends into the jump? No: a next state that
    // is not a hit-entry Guard closes it (End 0x14138CC39).
    assert_eq!(bench.sora.actions().counter_window, 0.0);
}

#[test]
fn air_guard_hangs_and_starts_its_cooldown_at_enable_change() {
    // II.1 test 10: guard at the apex: `AirGuard`, velocity 0 and gravity off from S(N) to
    // M(N + 38); pawn+0xE48 bit 2 = 0 from S(N); pawn+0xE3C = 0.3333 at M(N + 38); a second air
    // guard is refused until 20 frames later.
    let mut bench = Bench::new();
    bench.jump_to_apex();
    let out = bench.step(guard(Vec2::ZERO));
    assert_eq!((out.state, out.anim.name), (StateId::Guard, AnimId::AirGuard));
    assert!(!bench.sora.actions().air_guard_ok);
    let z = out.position.z;
    for k in 1..=37 {
        let out = bench.step(idle());
        assert_eq!(out.velocity, Vec3::ZERO, "frame N + {k}");
        assert!(near(out.position.z, z, 0.001), "he hangs: frame N + {k}");
    }
    assert_eq!(bench.sora.combat().air_guard_cooldown, 0.0);
    bench.step(idle()); // M(N + 38): TresEnableChange at 0.6333 s
    assert!(near(bench.sora.combat().air_guard_cooldown, 0.3333, 1.0e-4));
    // Gravity is back: he falls while the clip plays out; the state ends with the clip.
    let ended = 38 + bench.until(idle(), 30, |o, _| o.state != StateId::Guard);
    assert!(bench.last().position.z < z - 1.0);
    assert!(matches!(ended, 52 | 53), "{ended}");
    // The cool-down is written again at End (0x14138CB6A): 20 ticks of refusal from there.
    let mut refused = 0;
    for _ in 0..40 {
        if bench.last().on_floor {
            break;
        }
        if bench.step(guard(Vec2::ZERO)).state == StateId::Guard {
            break;
        }
        refused += 1;
    }
    assert!(bench.last().state == StateId::Guard, "the second air guard was never accepted before landing");
    assert!(matches!(refused, 19..=21), "refused for {refused} frames (0.3333 s = 20)");
}

// ============================================================================ II.2 Air Slide

/// Jump to the apex, then guard + stick forward: returns the bench at the request frame (frame 0).
fn air_slide(status: Status, battle: bool) -> (Bench, Output) {
    let mut bench = Bench::with(status, TestWorld::floor(), Vec3::ZERO, Tuning::GAME_EXACT);
    bench.sora.force_battle(battle);
    bench.jump_to_apex();
    let out = bench.step(guard(FORWARD));
    (bench, out)
}

fn with_air_slide_copies(copies: u8) -> Status {
    let mut status = level99();
    status.abilities.set(Ability::Airslide, 3, copies);
    status
}

#[test]
fn air_slide_level_3_travels_1610_cm_level_and_invincible() {
    // II.2 test 1: AIR_SLIDE in frame 0; reaction type 11; cumulative forward distance at ticks
    // 1, 2, 4, 10, 20, 30, 44 = 142.5, 285.0, 490.8, 942.2, 1353.8, 1513.0, 1610.25 cm; Z constant;
    // stick neutral after the press: ends at tick 44 -> JUMP sub 3, velocity 0, type back to the
    // default, pawn+0xE38 = 0.6.
    let (mut bench, out) = air_slide(level99(), false);
    assert_eq!((out.state, bench.sora.actions().reaction), (StateId::AirSlide, 11));
    assert!(!bench.sora.actions().weapon_out, "a plain slide out of battle does not draw");
    let start = out.position;
    let expected = [(1, 142.5), (2, 285.0), (4, 490.8), (10, 942.2), (20, 1353.8), (30, 1513.0), (44, 1610.25)];
    let mut end = None;
    for tick in 1..=50 {
        let out = bench.step(idle());
        if out.state != StateId::AirSlide {
            end = Some(tick);
            break;
        }
        assert_eq!(bench.sora.actions().reaction, 11, "tick {tick}");
        assert!(near(out.position.z, start.z, 0.001), "tick {tick}: Z {} vs {}", out.position.z, start.z);
        if let Some((_, distance)) = expected.iter().find(|(t, _)| *t == tick) {
            assert!(near(out.position.x - start.x, *distance, 0.8), "tick {tick}: {} vs {distance}", out.position.x - start.x);
        }
    }
    assert!(matches!(end, Some(44 | 45)), "{end:?}");
    let out = bench.last().clone();
    assert_eq!((out.state, out.sub_step, out.velocity.truncate()), (StateId::Jump, 4, Vec2::ZERO), "Fall -> Jump (sub 3 -> 4 in its first tick), from rest");
    assert!(near(out.position.x - start.x, 1610.25, 0.8));
    assert_eq!(bench.sora.actions().reaction, 0);
    assert!(near(bench.sora.combat().air_slide_cooldown, 0.6, 1.0e-4));
}

#[test]
fn air_slide_levels_are_819_1215_and_1610_cm() {
    // II.2 test 3: totals 819.25 and 1214.75 cm with 1 and 2 AIRSLIDE; tick 1: 72.5 and 107.5 cm.
    for (copies, first, total) in [(1u8, 72.5, 819.25), (2, 107.5, 1214.75), (3, 142.5, 1610.25)] {
        let (mut bench, out) = air_slide(with_air_slide_copies(copies), false);
        let x0 = out.position.x;
        assert!(near(bench.step(idle()).position.x - x0, first, 0.5), "{copies} copies, tick 1");
        bench.until(idle(), 60, |o, _| o.state != StateId::AirSlide);
        assert!(near(bench.last().position.x - x0, total, 0.8), "{copies} copies: {}", bench.last().position.x - x0);
    }
    // No AIRSLIDE, not in battle: the request is refused; the stick is active, so nothing else
    // (no guard, no roll in the air).
    let (bench, out) = air_slide(with_air_slide_copies(0), false);
    assert_eq!(out.state, StateId::Jump);
    drop(bench);
}

#[test]
fn air_slide_stick_exit_and_cooldown() {
    // II.2 test 2: the stick held (magnitude 1): the state ends on the first state tick with
    // m_bEnableChange (clip time >= 0.5: tick 30 or 31) -> JUMP sub 3 keeping the clip.
    let (mut bench, out) = air_slide(level99(), false);
    let x0 = out.position.x;
    let tick = bench.until(input(FORWARD, &[], &[]), 50, |o, _| o.state != StateId::AirSlide);
    assert!(matches!(tick, 30 | 31), "tick {tick}");
    assert_eq!((bench.last().state, bench.last().sub_step, bench.last().anim.name), (StateId::Jump, 3, AnimId::AirSlide));
    assert!(bench.last().position.x - x0 >= 1512.0);
    // II.2 test 4: after a slide ended at frame E (in the air): guard + stick on frames E + 1 ...
    // E + 35 are refused (pawn+0xE38 > 0); accepted on frame E + 36 or E + 37.
    let (mut bench, _) = air_slide(level99(), false);
    bench.until(idle(), 60, |o, _| o.state != StateId::AirSlide);
    let mut accepted = None;
    for frame in 1..=40 {
        if bench.step(guard(FORWARD)).state == StateId::AirSlide {
            accepted = Some(frame);
            break;
        }
    }
    assert!(matches!(accepted, Some(36 | 37)), "{accepted:?}");
}

#[test]
fn aerial_dodge_in_battle_travels_700_cm_and_a_neutral_stick_guards() {
    // II.2 test 5: battle flag on, AIRDODGE: clip `AirDodge`: forward distance at ticks 2, 20, 42
    // = 100, 604.7, 700 cm; ends at tick 42; reaction type 11 throughout; `Draw` in Begin;
    // cool-down 0.6 at End.
    let (mut bench, out) = air_slide(level99(), true);
    assert_eq!(out.state, StateId::AirSlide);
    assert!(bench.sora.actions().weapon_out, "Aerial Dodge draws the Keyblade");
    let x0 = out.position.x;
    let mut end = None;
    for tick in 1..=50 {
        let out = bench.step(idle());
        if out.state != StateId::AirSlide {
            end = Some(tick);
            break;
        }
        assert_eq!((out.anim.name, bench.sora.actions().reaction), (AnimId::AirDodge, 11));
        match tick {
            2 => assert!(near(out.position.x - x0, 100.0, 0.6), "{}", out.position.x - x0),
            20 => assert!(near(out.position.x - x0, 604.7, 0.6), "{}", out.position.x - x0),
            _ => {}
        }
    }
    assert!(matches!(end, Some(42 | 43)), "{end:?}");
    assert!(near(bench.last().position.x - x0, 700.0, 0.6));
    assert!(near(bench.sora.combat().air_slide_cooldown, 0.6, 1.0e-4));
    // In battle without AIRDODGE but with AIRSLIDE: an Air Slide.
    let mut status = level99();
    status.abilities.set(Ability::Airdodge, 1, 0);
    let (mut bench, _) = air_slide(status, true);
    assert_eq!(bench.step(idle()).anim.name, AnimId::AirSlide);
    // II.2 test 6: in the air with the stick neutral the press is an air guard, not a slide
    // (`TryAirSlide`: weapon kind 1 needs magnitude >= 0.1).
    let mut bench = Bench::new();
    bench.jump_to_apex();
    assert_eq!(bench.step(guard(Vec2::ZERO)).state, StateId::Guard);
}

#[test]
fn the_second_jump_survives_a_slide_that_ends_in_a_fall() {
    // II.2 test 7 (a): jump, slide, let it end in the fall, press jump: double flight (bit 0 was
    // kept because the slide's next state was JUMP; the fall Begin sets flag 8).
    let (mut bench, _) = air_slide(level99(), false);
    bench.until(idle(), 60, |o, _| o.state == StateId::Jump);
    assert!(bench.sora.actions().second_jump);
    bench.step(jump_press(Vec2::ZERO));
    assert_eq!(bench.sora.jump_kind(), Some(3));
    assert!(!bench.sora.actions().second_jump, "Jump::Begin of type 3 consumes the bit");
    // (c): jump, double flight, slide, fall: a jump press is refused (bit 0 clear).
    bench.until(jump_hold(Vec2::ZERO), 120, |o, _| o.state == StateId::Jump && o.sub_step == 4);
    bench.run(40); // past the air-slide cool-down
    assert_eq!(bench.step(guard(FORWARD)).state, StateId::AirSlide);
    bench.until(idle(), 60, |o, _| o.state == StateId::Jump);
    let before = bench.last().clone();
    bench.step(jump_press(Vec2::ZERO));
    assert_eq!((bench.last().sub_step, bench.sora.jump_kind()), (before.sub_step, Some(0)), "refused: still the fall");
}

#[test]
fn aerial_recovery_from_0_2_s_into_a_blow() {
    // II.2 test 8: DAMAGE type 2 (blow), airborne, attacker at yaw 90 from Sora; a jump press at
    // 0.1 s is refused by the recovery branch; at >= 0.2 s + one tick: AIR_SLIDE with the recovery
    // flag, facing snapped toward the attacker, velocity 0, clip `AirRecovery`, reaction type 11,
    // the air-slide bit still set; with no input -> Fall at tick 30 (0.5 s), no cool-down.
    let blow = |bench: &mut Bench| {
        let own = bench.last().position;
        bench.sora.take_hit(hit_from(90.0, Kind::Blow, 20.0, own));
        bench.step(idle());
        assert_eq!(bench.last().state, StateId::Damage);
    };
    let mut bench = Bench::new();
    blow(&mut bench);
    bench.run(5);
    bench.step(jump_press(Vec2::ZERO));
    assert_eq!(bench.last().state, StateId::Damage, "0.1 s: not recoverable yet");
    let mut bench = Bench::new();
    blow(&mut bench);
    bench.until(idle(), 40, |_, sora| sora.damage_reaction().is_some_and(|d| d.recoverable));
    assert!(!bench.last().on_floor);
    let out = bench.step(jump_press(Vec2::ZERO));
    assert_eq!(out.state, StateId::AirSlide);
    assert!(near(out.yaw, 90.0, 12.0), "faces the attacker: {}", out.yaw);
    let a = bench.sora.actions();
    assert_eq!((a.reaction, a.air_slide_ok, a.reaction_command.map(|c| c.0)), (11, true, Some(2)), "BLOW_COUNTER armed");
    let next = bench.step(idle());
    assert_eq!((next.anim.name, next.velocity), (AnimId::AirRecovery, Vec3::ZERO));
    let ticks = 1 + bench.until(idle(), 40, |o, _| o.state != StateId::AirSlide);
    assert!(matches!(ticks, 30 | 31), "{ticks}");
    assert_eq!(bench.last().state, StateId::Jump);
    assert_eq!((bench.sora.combat().air_slide_cooldown, bench.sora.actions().reaction_command), (0.0, None));
    // Without AIR_RECOVERY (1): no recovery.
    let mut bench = Bench::new();
    bench.sora.status_mut().abilities.set(Ability::AirRecovery, 1, 0);
    blow(&mut bench);
    bench.until(idle(), 40, |_, sora| sora.damage_reaction().is_some_and(|d| d.recoverable));
    assert_eq!(bench.step(jump_press(Vec2::ZERO)).state, StateId::Damage);
}

// ============================================================================ II.2 Glide

#[test]
fn glide_starts_in_the_fall_at_1425_then_1500_and_sinks_at_0_98() {
    // II.2 test 9: jump with the button held from the press (stick neutral): GLIDE on the first
    // frame after Jump sub-step 4; velocity (0, 0, 0) at Begin; |V_xy| = 1425 after tick 1, 1500
    // from tick 2; forward distance 11.875, 36.25, then + 25 cm per tick; height loss < 13 cm
    // after 300 ticks; pawn+0xE38 = 0.6 and pawn+0xE3C = 0.3333 at Begin.
    let mut bench = Bench::new();
    bench.step(jump_press(Vec2::ZERO));
    bench.until(jump_hold(Vec2::ZERO), 120, |o, _| o.state == StateId::Jump && o.sub_step == 4);
    let out = bench.step(jump_hold(Vec2::ZERO));
    assert_eq!((out.state, out.velocity, out.anim.name), (StateId::Glide, Vec3::ZERO, AnimId::GlideLoop));
    assert!(near(bench.sora.combat().air_slide_cooldown, 0.6, 1.0e-4) && near(bench.sora.combat().air_guard_cooldown, 0.3333, 1.0e-4));
    let start = out.position;
    let t1 = bench.step(jump_hold(Vec2::ZERO));
    assert!(near(t1.velocity.truncate().length(), 1425.0, 0.01) && near(t1.position.x - start.x, 11.875, 0.01), "{:?}", t1.velocity);
    let t2 = bench.step(jump_hold(Vec2::ZERO));
    assert!(near(t2.velocity.truncate().length(), 1500.0, 0.01) && near(t2.position.x - start.x, 36.25, 0.01));
    let t3 = bench.step(jump_hold(Vec2::ZERO));
    assert!(near(t3.position.x - t2.position.x, 25.0, 0.01));
    bench.hold(jump_hold(Vec2::ZERO), 297);
    let out = bench.last().clone();
    assert_eq!(out.state, StateId::Glide);
    let lost = start.z - out.position.z;
    assert!(lost > 5.0 && lost < 13.0, "height lost after 300 ticks: {lost}");
    assert!(near(out.velocity.z, -0.98 * 5.0, 0.3), "V.z ~ -0.98 t: {}", out.velocity.z);
    // Release: velocity (0, 0, 0) in that frame, state JUMP (sub 3 -> 4 in its first tick),
    // GravityScale and MaxWalkSpeed back to the defaults (he falls normally).
    let out = bench.step(idle());
    assert_eq!((out.state, out.velocity), (StateId::Jump, Vec3::ZERO));
    let fall = bench.hold(idle(), 20);
    assert!(fall.velocity.z < -300.0);
    // Without GLIDE (20): holding the button keeps the Jump state.
    let mut bench = Bench::new();
    bench.sora.status_mut().abilities.set(Ability::Glide, 1, 0);
    bench.step(jump_press(Vec2::ZERO));
    bench.until(jump_hold(Vec2::ZERO), 120, |o, _| o.state == StateId::Jump && o.sub_step == 4);
    assert_eq!(bench.step(jump_hold(Vec2::ZERO)).state, StateId::Jump);
}

#[test]
fn glide_turns_3_percent_of_the_yaw_error_per_tick() {
    // II.2 test 9, steering: stick 90 degrees right at magnitude 1: facing error after n ticks =
    // 90 x 0.97^n (45 degrees after 23 ticks); the travel follows the facing.
    let mut bench = Bench::new();
    bench.step(jump_press(Vec2::ZERO));
    bench.until(jump_hold(Vec2::ZERO), 120, |o, _| o.state == StateId::Glide);
    for n in 1..=23 {
        let out = bench.step(jump_hold(RIGHT));
        assert!(near(90.0 - out.yaw, 90.0 * 0.97f32.powi(n), 0.01), "tick {n}: yaw {}", out.yaw);
    }
    assert!(near(bench.last().yaw, 45.0, 1.0));
    let v = bench.last().velocity;
    assert!(v.y > 300.0 && near(v.truncate().length(), 1500.0, 0.5), "{v:?}");
    // A stick at half tilt does not steer (comiss mag,0.5; jbe skip).
    let yaw = bench.last().yaw;
    let mut bench2 = Bench::new();
    bench2.step(jump_press(Vec2::ZERO));
    bench2.until(jump_hold(Vec2::ZERO), 120, |o, _| o.state == StateId::Glide);
    bench2.hold(jump_hold(raw(RIGHT, 0.5)), 10);
    assert_eq!(bench2.last().yaw, 0.0);
    let _ = yaw;
}

#[test]
fn nothing_ends_a_glide_on_the_ground_by_the_code_as_read() {
    // II.2 test 10 (question Q2, TO CONFIRM IN THE RUNNING GAME): a glide carried onto rising
    // ground: the movement walks (mode Walking) while the state stays GLIDE at 1500 cm/s as long
    // as the button is held; on release: Falling, velocity 0, then the landing. The same under
    // the user's options. The port option `glide_ends_on_ground` ends it on contact instead.
    let ramp = || TestWorld::floor().with_plane_through(Vec3::new(1500.0, 0.0, 0.0), Vec3::new(-(20.0f32.to_radians().sin()), 0.0, 20.0f32.to_radians().cos()));
    for tuning in [Tuning::GAME_EXACT, Tuning::USER] {
        let mut bench = Bench::with(level99(), ramp(), Vec3::ZERO, tuning);
        bench.step(jump_press(Vec2::ZERO));
        bench.until(jump_hold(Vec2::ZERO), 150, |o, _| o.state == StateId::Glide);
        bench.until(jump_hold(Vec2::ZERO), 200, |o, _| o.on_floor);
        let out = bench.hold(jump_hold(Vec2::ZERO), 30);
        assert_eq!((out.state, out.on_floor), (StateId::Glide, true), "he keeps the Glide state on the ground");
        assert!(near(out.velocity.truncate().length(), 1500.0, 1.0), "{:?}", out.velocity);
        let released = bench.step(idle());
        assert_eq!((released.velocity, released.on_floor), (Vec3::ZERO, false), "End: Falling, velocity zeroed");
        bench.until(idle(), 30, |o, _| o.state == StateId::Jump && o.sub_step >= 10);
        bench.until(idle(), 90, |o, _| o.state == StateId::Idle);
    }
    let mut bench = Bench::with(level99(), ramp(), Vec3::ZERO, Tuning { glide_ends_on_ground: true, ..Tuning::USER });
    bench.step(jump_press(Vec2::ZERO));
    bench.until(jump_hold(Vec2::ZERO), 150, |o, _| o.state == StateId::Glide);
    bench.until(jump_hold(Vec2::ZERO), 200, |o, _| o.on_floor && o.state == StateId::Idle);
    assert_eq!(bench.hold(jump_hold(Vec2::ZERO), 30).state, StateId::Idle, "the option ends the glide on ground contact, for good");
}

// ============================================================================ II.3 jump types

/// The highest actor Z reached over the next `steps` steps with `input`, relative to `floor_z`.
fn apex(bench: &mut Bench, input: Input, steps: usize, floor_z: f32) -> f32 {
    (0..steps).map(|_| bench.step(input).position.z - floor_z).fold(0.0, f32::max)
}

#[test]
fn high_jump_is_every_ground_jump_with_the_ability() {
    // II.3 J1: Idle, jump held: type 1, `JumpHighStart`; Velocity.Z = 1950 in M(5); apex 461.9 cm.
    let mut bench = Bench::new();
    let floor = bench.last().position.z;
    let out = bench.step(jump_press(Vec2::ZERO));
    assert_eq!((out.state, out.anim.name, bench.sora.jump_kind()), (StateId::Jump, AnimId::JumpHighStart, Some(1)));
    bench.hold(jump_hold(Vec2::ZERO), 4);
    assert_eq!(bench.last().velocity.z, 0.0);
    let launch = bench.step(jump_hold(Vec2::ZERO));
    assert!(launch.velocity.z > 1800.0 && launch.velocity.z <= 1950.0, "DoJump in M(5): {}", launch.velocity.z);
    // The glide would take over at the apex: without it the plain fall follows.
    bench.sora.status_mut().abilities.set(Ability::Glide, 1, 0);
    let top = apex(&mut bench, jump_hold(Vec2::ZERO), 80, floor);
    assert!(near(top, 461.9, 1.0), "apex {top}");
    // Without HIGHJUMP (16): type 0, 1480 (266 cm).
    let mut bench = Bench::new();
    bench.sora.status_mut().abilities.set(Ability::Highjump, 1, 0);
    bench.sora.status_mut().abilities.set(Ability::Glide, 1, 0);
    let out = bench.step(jump_press(Vec2::ZERO));
    assert_eq!((out.anim.name, bench.sora.jump_kind()), (AnimId::JumpNormalStart, Some(0)));
    let top = apex(&mut bench, jump_hold(Vec2::ZERO), 80, floor);
    assert!(near(top, 266.1, 1.0), "apex {top}");
}

#[test]
fn a_tapped_high_jump_is_cut_in_sub_step_1() {
    // II.3 J2: jump tapped (1 frame): GravityScale 3.0 from S(7) (clip time 0.1167 > 0.1), rise
    // gravity -8232 from M(8); apex about 277 cm (held: 462).
    let mut bench = Bench::new();
    let floor = bench.last().position.z;
    bench.step(jump_press(Vec2::ZERO));
    let top = apex(&mut bench, idle(), 80, floor);
    assert!(near(top, 277.0, 6.0), "tap apex {top}");
}

#[test]
fn doubleflight_needs_velocity_z_at_most_500_and_is_one_per_airtime() {
    // II.3 J3: a second press at frame 20 is refused (flag 8 clear: Velocity.Z > 500). J4: at
    // frame 27 it is accepted: type 3, `JumpDoubleFlightStart`; Velocity.Z = 2300 in M(31)
    // (press + 4); apex 642.6 cm above the launch point; bit 0 = 0 from the Begin; releasing the
    // button does not shorten it. J5: a third press anywhere in the air is refused. J6: landing
    // restores the bit.
    let mut bench = Bench::new();
    bench.sora.status_mut().abilities.set(Ability::Glide, 1, 0);
    bench.step(jump_press(Vec2::ZERO)); // frame 0
    bench.hold(jump_hold(Vec2::ZERO), 19);
    bench.step(jump_press(Vec2::ZERO)); // frame 20
    assert_eq!(bench.sora.jump_kind(), Some(1), "J3: refused");
    // (LITERAL: on a press frame `JumpHeld` is not called (movement spec 2.4), so this refused
    // re-press counts as a release for the High Jump's sub-step-1 cut: a fresh run for J4.)
    let mut bench = Bench::new();
    bench.sora.status_mut().abilities.set(Ability::Glide, 1, 0);
    bench.step(jump_press(Vec2::ZERO)); // frame 0
    bench.hold(jump_hold(Vec2::ZERO), 26);
    assert!(bench.last().velocity.z <= 500.0 && bench.last().velocity.z > 400.0, "after M(26): {}", bench.last().velocity.z);
    let out = bench.step(jump_press(Vec2::ZERO)); // frame 27
    assert_eq!((bench.sora.jump_kind(), out.anim.name, out.sub_step), (Some(3), AnimId::JumpDoubleFlightStart, 0), "J4: accepted");
    assert!(!bench.sora.actions().second_jump);
    bench.run(3);
    let launch_z = bench.last().position.z;
    let launch = bench.step(idle()); // frame 31
    assert!(launch.velocity.z > 2150.0 && launch.velocity.z <= 2300.0, "DoJump with 2300 in M(31): {}", launch.velocity.z);
    // Button released the whole time: no height cut for a double flight.
    let mut top = 0.0f32;
    for k in 0..90 {
        let out = bench.step(if k == 30 { jump_press(Vec2::ZERO) } else { idle() });
        top = top.max(out.position.z - launch_z);
        assert_ne!(out.sub_step, 0, "J5: no third jump");
    }
    assert!(near(top, 642.6, 1.5), "apex above the launch point: {top}");
    bench.until(idle(), 200, |o, _| o.on_floor && o.state == StateId::Jump);
    assert!(bench.sora.actions().second_jump, "J6: SetSecondJump(1) on the landing tick");
    // Without DOUBLEFLIGHT (17): the press in the air does nothing.
    let mut bench = Bench::new();
    bench.sora.status_mut().abilities.set(Ability::Doubleflight, 1, 0);
    bench.sora.status_mut().abilities.set(Ability::Glide, 1, 0);
    bench.step(jump_press(Vec2::ZERO));
    bench.hold(jump_hold(Vec2::ZERO), 30);
    bench.step(jump_press(Vec2::ZERO));
    assert_eq!(bench.sora.jump_kind(), Some(1));
}

#[test]
fn a_fall_off_a_ledge_may_double_jump_at_once() {
    // II.3 J7: walk off a ledge (no jump), press on the first airborne frame after the Jump state
    // began: accepted at once (the fall Begin sets flag 8).
    let world = TestWorld::floor().with_box(Vec3::new(-500.0, -500.0, 0.0), Vec3::new(300.0, 500.0, 600.0));
    let mut bench = Bench::with(level99(), world, Vec3::new(0.0, 0.0, 600.0), Tuning::GAME_EXACT);
    bench.until(input(FORWARD, &[], &[]), 200, |o, _| o.state == StateId::Jump);
    bench.step(jump_press(Vec2::ZERO));
    assert_eq!(bench.sora.jump_kind(), Some(3));
}

#[test]
fn landings_roll_from_800_cm_with_the_stick_and_take_the_b_names_with_the_weapon_out() {
    // II.3 J8: a fall of 800 cm or more with the stick held: `RollLand` (0.7667 s, 300 cm of root
    // motion, control back at 0.4325 s); less: `Land`; <= 75 cm: `LightLand`.
    let fall = |height: f32, stick: Vec2, weapon: bool| {
        // (Kerbs up to 59.4 cm are walked down without leaving Walking: movement spec 5.3.)
        let edge = if height < 100.0 { 100.0 } else { 600.0 };
        let world = TestWorld::floor().with_box(Vec3::new(-500.0, -500.0, 0.0), Vec3::new(edge, 500.0, height));
        let mut status = level99();
        status.abilities.set(Ability::Glide, 1, 0);
        let mut bench = Bench::with(status, world, Vec3::new(0.0, 0.0, height), Tuning::GAME_EXACT);
        if weapon {
            // A guard draws the Keyblade; then on with the run.
            bench.step(guard(Vec2::ZERO));
            bench.until(idle(), 80, |o, _| o.state == StateId::Idle);
        }
        bench.until(input(FORWARD, &[], &[]), 200, |o, _| o.state == StateId::Jump);
        bench.until(input(stick, &[], &[]), 400, |o, _| o.state == StateId::Jump && o.sub_step >= 10);
        bench
    };
    let mut bench = fall(900.0, FORWARD, false);
    assert_eq!(bench.last().anim.name, AnimId::RollLand);
    let x0 = bench.last().position.x;
    let ticks = bench.until(input(FORWARD, &[], &[]), 80, |o, _| o.state != StateId::Jump);
    assert!(matches!(ticks, 26..=28), "control returns at TresEnableChange 0.4325 s: {ticks}");
    assert_eq!(bench.last().state, StateId::Idle);
    assert_eq!(bench.step(input(FORWARD, &[], &[])).state, StateId::Run);
    assert!(bench.last().position.x - x0 > 200.0 && bench.last().position.x - x0 < 300.0);
    let mut bench = fall(900.0, FORWARD, false);
    let x0 = bench.last().position.x;
    // (Stick released on the landing tick's successors: the whole clip plays: 300 cm.)
    bench.until(idle(), 80, |o, _| o.state != StateId::Jump);
    assert!(near(bench.last().position.x - x0, 300.0, 1.0), "RollLand root motion: {}", bench.last().position.x - x0);
    assert_eq!(fall(900.0, Vec2::ZERO, false).last().anim.name, AnimId::Land, "no stick: Land");
    assert_eq!(fall(700.0, FORWARD, false).last().anim.name, AnimId::Land, "under 800 cm: Land");
    assert_eq!(fall(70.0, FORWARD, false).last().anim.name, AnimId::LightLand);
    assert_eq!(fall(70.0, Vec2::ZERO, true).last().anim.name, AnimId::LightLandB);
    // S7: with the Keyblade out the names are `LandB` / `LightLandB` (the same sequences).
    assert_eq!(fall(700.0, Vec2::ZERO, true).last().anim.name, AnimId::LandB);
    // (LightLandB: the land-pose test below.)
}

#[test]
fn the_land_pose_of_an_air_guard_lands_with_light_land() {
    // II.3 "Land pose mechanism": `AirGuard` carries TresLandAnimPose (LIGHT) at 0; nothing is
    // played after it, so the fall Begin on the floor plays `LightLand(B)` at once. Guard close
    // to the ground so that he touches down while the clip still plays.
    let mut bench = Bench::new();
    bench.sora.status_mut().abilities.set(Ability::Highjump, 1, 0);
    bench.step(jump_press(Vec2::ZERO));
    bench.until(idle(), 200, |o, _| o.state == StateId::Jump && o.sub_step == 4 && o.position.z < 60.0);
    assert_eq!(bench.step(guard(Vec2::ZERO)).state, StateId::Guard);
    bench.until(idle(), 80, |o, _| o.state != StateId::Guard);
    bench.until(idle(), 10, |o, _| o.state == StateId::Jump);
    assert_eq!((bench.last().anim.name, bench.last().sub_step), (AnimId::LightLandB, 11));
}

// ============================================================================ II.3 stance

/// A host whose only answer is the engaged-enemy count.
struct Engaged(u32);

impl Targets for Engaged {
    fn enumerate(&self, _out: &mut Vec<TargetInfo>) {}
    fn info(&self, _id: TargetId) -> Option<TargetInfo> {
        None
    }
    fn hurt_shapes(&self, _id: TargetId, _out: &mut Vec<HurtShape>) {}
    fn apply_hit(&mut self, _hit: &HitRecord) -> HitResult {
        HitResult::default()
    }
    fn engaged_count(&self) -> u32 {
        self.0
    }
}

#[test]
fn the_battle_flag_follows_the_engaged_count_with_a_1_s_linger() {
    // II.3 "The inputs" 1: battle = any engaged enemy, kept 1.0 s after the last one.
    let world = TestWorld::floor();
    let origin = Vec3::Z * Sora::origin_height_on_floor(&SORA_PARAMS);
    let mut sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(level99());
    let mut step = |sora: &mut Sora, engaged: u32| {
        let mut null = NullHost::default();
        let mut targets = Engaged(engaged);
        let mut host = null.host(&world);
        host.targets = &mut targets;
        sora.step_hosted(DT, &Input::default(), &mut host)
    };
    step(&mut sora, 0);
    assert!(!sora.actions().battle);
    let out = step(&mut sora, 1);
    assert!(sora.actions().battle && out.events.iter().any(|e| matches!(e.kind, EventKind::BattleMode { on: true })));
    let mut off = None;
    for k in 1..=70 {
        step(&mut sora, 0);
        if off.is_none() && !sora.actions().battle {
            off = Some(k);
        }
    }
    assert!(matches!(off, Some(60 | 61)), "the flag drops 1.0 s after the count: {off:?}");
}

#[test]
fn idle_draws_the_keyblade_for_battle_and_puts_it_away_after_the_idle_break() {
    // II.3 S1: Idle (`IdleN`), battle flag turned on: the same tick: Keyblade drawn with its
    // effect, `BattleStart` (0.4333 s) -> `IdleB`; pose BATTLE from the next movement tick.
    let mut bench = Bench::new();
    bench.sora.force_battle(true);
    let out = bench.step(idle());
    assert_eq!((out.state, out.anim.name, bench.sora.actions().weapon_out), (StateId::Idle, AnimId::BattleStart, true));
    assert!(out.events.iter().any(|e| matches!(e.kind, EventKind::WeaponVisible { visible: true, .. })));
    assert!(out.events.iter().any(|e| matches!(&e.kind, EventKind::EffectSpawn { spawn, .. } if spawn.name == "VW_SO010_APP0")));
    bench.step(idle());
    assert_eq!(bench.sora.actions().last_pose, 1);
    let ticks = 1 + bench.until(idle(), 40, |o, _| o.anim.name == AnimId::IdleB);
    assert!(matches!(ticks, 26 | 27), "BattleStart is 0.4333 s: {ticks}");
    // S2: flag off: `IdleB` keeps playing to its end (<= 5.9333 s), then `BattleEnd` (1.1 s; pose
    // NORMAL at 0.1 s), then `IdleN` with the Keyblade in hand; after 3 x `IdleN` the break
    // `IdleBreakW` (9.6333 s); at its end the Keyblade is put away with its effect.
    bench.run(60);
    bench.sora.force_battle(false);
    bench.run(70); // the 1.0 s linger
    assert!(!bench.sora.actions().battle);
    assert_eq!(bench.last().anim.name, AnimId::IdleB, "waits for the clip's end");
    bench.until(idle(), 400, |o, _| o.anim.name == AnimId::BattleEnd);
    let ticks = bench.until(idle(), 100, |o, _| o.anim.name == AnimId::IdleN);
    assert!(matches!(ticks, 66 | 67), "BattleEnd is 1.1 s: {ticks}");
    assert!((bench.sora.actions().weapon_out, bench.sora.actions().last_pose) == (true, 0), "the Keyblade stays in hand, pose NORMAL");
    let ticks = bench.until(idle(), 700, |o, _| o.anim.name == AnimId::IdleBreakW);
    assert!(matches!(ticks, 538..=546), "3 x IdleN (3 x 3.0 s): {ticks}");
    let ticks = bench.until(idle(), 700, |_, sora| !sora.actions().weapon_out);
    assert!(matches!(ticks, 576..=582), "IdleBreakW is 9.6333 s: {ticks}");
    assert!(bench.last().events.iter().any(|e| matches!(e.kind, EventKind::WeaponVisible { visible: false, .. })));
    assert_eq!(bench.last().anim.name, AnimId::IdleN);
}

#[test]
fn run_plays_the_battle_start_and_end_clips_and_takes_the_b_and_w_sets() {
    // II.3 S3: running (not walking), flag turned on: sub-step 0: Keyblade drawn,
    // `RunBattleStartW` (0.5333 s), sub 3 -> loop `RunLoopB`. S4: flag off: `RunBattleEnd`
    // (0.5333 s) -> the run loop of the `W` set with the Keyblade still out, for any length of
    // running. S5: walking when the flag turns on: Keyblade drawn, no clip.
    let run = input(FORWARD, &[], &[]);
    let mut bench = Bench::new();
    bench.hold(run, 60);
    assert_eq!(bench.last().anim.name, AnimId::RunLoopN);
    bench.sora.force_battle(true);
    let out = bench.step(run);
    assert_eq!((out.anim.name, out.sub_step, bench.sora.actions().weapon_out), (AnimId::RunBattleStartW, 3, true));
    let ticks = bench.until(run, 60, |o, _| o.anim.name != AnimId::RunBattleStartW);
    assert!(matches!(ticks, 32 | 33), "{ticks}");
    assert_eq!((bench.last().anim.name, bench.last().sub_step, bench.sora.actions().last_pose), (AnimId::RunLoopB, 0, 1));
    assert!(bench.last().velocity.x > 600.0, "he kept running through the clip: {:?}", bench.last().velocity);
    bench.sora.force_battle(false);
    bench.until(run, 100, |o, _| o.anim.name == AnimId::RunBattleEnd);
    bench.until(run, 60, |o, _| o.anim.name != AnimId::RunBattleEnd);
    assert_eq!((bench.last().anim.name, bench.sora.actions().last_pose), (AnimId::RunLoopW, 0));
    bench.hold(run, 900);
    assert!(bench.sora.actions().weapon_out && bench.last().anim.name == AnimId::RunLoopW, "Run never puts the plain Keyblade away");
    // The stop clip of the `W` set.
    bench.until(idle(), 10, |o, _| o.sub_step == 4);
    assert!(matches!(bench.last().anim.name, AnimId::RunEndRW | AnimId::RunEndLW));
    // S5.
    let walk = input(raw(FORWARD, 0.5), &[], &[]);
    let mut bench = Bench::new();
    bench.hold(walk, 40);
    bench.sora.force_battle(true);
    let out = bench.step(walk);
    assert_eq!((out.anim.name, bench.sora.actions().weapon_out, bench.sora.actions().last_pose), (AnimId::WalkLoopN, true, 0));
    assert_eq!(bench.step(walk).anim.name, AnimId::WalkLoopW);
}

// ============================================================================ request order, options

#[test]
fn the_guard_button_requests_in_the_controllers_order() {
    // Part I 2.1 (0x1413F1AB0): GUARD pressed: air slide -> (stick inactive) guard / (stick
    // active) dodge roll; then JUMP pressed: jump; JUMP held: glide; then the stick.
    let mut bench = Bench::new();
    // Guard and jump pressed in the same frame, stick neutral: the guard wins and returns.
    assert_eq!(bench.step(input(Vec2::ZERO, &[PadButton::GUARD, PadButton::JUMP], &[])).state, StateId::Guard);
    // Stick active on the ground: the dodge roll, and the stick is not forwarded that frame.
    let mut bench = Bench::new();
    assert_eq!(bench.step(input(FORWARD, &[PadButton::GUARD, PadButton::JUMP], &[])).state, StateId::Dodgeroll);
    // In the air with the stick: the air slide comes before the guard and the dodge.
    let mut bench = Bench::new();
    bench.jump_to_apex();
    assert_eq!(bench.step(guard(FORWARD)).state, StateId::AirSlide);
    // The gate out of an attack is the attack's cancel bit (`Attack.cancel`, +0x234 bit 7), not
    // m_bEnableChange (II.0.2 point 1): a roll pressed on the first frames of a swing is refused.
    let mut bench = Bench::new();
    bench.step(input(Vec2::ZERO, &[PadButton::ATTACK], &[]));
    bench.run(2);
    assert_eq!(bench.last().state, StateId::Attack);
    assert_eq!(bench.step(guard(FORWARD)).state, StateId::Attack, "no cancel bit yet");
    assert_eq!(bench.step(guard(Vec2::ZERO)).state, StateId::Attack, "the guard gate tests the same bit");
    let n = bench.until(guard(FORWARD), 90, |o, _| o.state != StateId::Attack);
    assert_eq!(bench.last().state, StateId::Dodgeroll, "after {n} frames the cancel bit lets the roll through");
}

#[test]
fn the_users_jump_cut_applies_to_the_high_jump_and_not_to_the_double_flight() {
    // PORT OPTION `Tuning::jump_cut_on_release` with the level-99 loadout (High Jump): a tap
    // reaches the minimum height (110 cm: enough to land on the 1 m block), a held jump the full
    // 462 cm; the game's own sub-step-1 cut (GravityScale 3.0) is not applied on top. A double
    // flight keeps the game's rule (no cut at all).
    let mut bench = Bench::with(level99(), TestWorld::floor(), Vec3::ZERO, Tuning::USER);
    bench.sora.status_mut().abilities.set(Ability::Glide, 1, 0);
    let floor = bench.last().position.z;
    bench.step(jump_press(Vec2::ZERO));
    let top = apex(&mut bench, idle(), 90, floor);
    assert!(top >= 110.0 && top < 111.0, "tap: {top}");
    bench.until(idle(), 100, |o, _| o.state == StateId::Idle);
    bench.step(jump_press(Vec2::ZERO));
    let top = apex(&mut bench, jump_hold(Vec2::ZERO), 90, floor);
    assert!(near(top, 461.9, 1.0), "held: {top}");
    bench.until(idle(), 200, |o, _| o.state == StateId::Idle);
    // Tap, then the second jump tapped as well: the double flight still rises its full 642.6 cm.
    bench.step(jump_press(Vec2::ZERO));
    bench.until(idle(), 60, |o, _| o.sub_step == 4);
    bench.step(jump_press(Vec2::ZERO));
    assert_eq!(bench.sora.jump_kind(), Some(3));
    bench.run(3);
    let launch_z = bench.last().position.z;
    let top = apex(&mut bench, idle(), 90, launch_z);
    assert!(near(top, 642.6, 1.5), "double flight under the user's options: {top}");
}

// ============================================================================ tier 2: wall kick, Superslide

/// Jump to the apex 600 cm in front of a wall (its face at x = 1200) and air-slide into it along
/// `stick`; returns the bench on the frame the slide's move was blocked.
fn slide_into_wall(status: Status, stick: Vec2) -> Bench {
    let world = TestWorld::floor().with_plane(Vec3::NEG_X, -1200.0);
    let mut bench = Bench::with(status, world, Vec3::new(600.0, 0.0, 0.0), Tuning::GAME_EXACT);
    bench.jump_to_apex();
    assert_eq!(bench.step(guard(stick)).state, StateId::AirSlide);
    bench.until(idle(), 40, |o, _| o.state != StateId::AirSlide || o.displacement_velocity.x < 100.0);
    bench
}

#[test]
fn an_air_slide_into_a_wall_kicks_off_it_and_the_jump_button_is_a_super_jump() {
    // II.4 test 7: air slide into a wall, stick neutral after the push: on the contact frame
    // WALL_KICK begins and its first tick plays `WallKickF` (the slide faced the wall within 35
    // degrees). A jump press during the clip: the super jump (type 2, 2500) from clip time 0.1 s.
    let mut bench = slide_into_wall(level99(), FORWARD);
    assert_eq!((bench.last().state, bench.last().anim.name, bench.last().sub_step), (StateId::WallKick, AnimId::WallKickF, 3));
    assert!(bench.last().position.x > 1100.0 && bench.sora.actions().weapon_out, "{:?}", bench.last().position);
    bench.step(jump_press(Vec2::ZERO));
    let ticks = bench.until(idle(), 20, |o, _| o.state == StateId::Jump);
    assert!(ticks <= 8, "the latch fires from clip time 0.1 s: {ticks}");
    assert_eq!(bench.sora.jump_kind(), Some(2));
    assert_eq!(bench.last().anim.name, AnimId::JumpSuperStart);
    let z0 = bench.last().position.z;
    let top = apex(&mut bench, idle(), 120, z0);
    // (He sinks a few cm in the 5 frames of the start clip before the launch.)
    assert!(near(top, 759.2, 5.0), "super jump: 2500^2 / 8232 = 759.2 cm above the launch: {top}");
    // Without WALL_KICK (97): no kick: the slide goes on against the wall.
    let mut status = level99();
    status.abilities.set(Ability::WallKick, 1, 0);
    let bench = slide_into_wall(status, FORWARD);
    assert_eq!(bench.last().state, StateId::AirSlide);
}

#[test]
fn a_wall_kick_along_the_wall_and_the_superslide_of_ii_2() {
    // II.4: the slide meets the wall at 45 degrees: D . N >= -0.819: he is turned parallel to the
    // wall and plays `WallKickR` / `WallKickL` (0.3333 s). The guard button during the kick
    // latches a Superslide, which starts at the clip end (needs SUPERSLIDE).
    let mut bench = slide_into_wall(level99(), Vec2::new(0.75, 0.75));
    assert_eq!(bench.last().state, StateId::WallKick);
    assert!(matches!(bench.last().anim.name, AnimId::WallKickR | AnimId::WallKickL), "{:?}", bench.last().anim.name);
    assert!(near(bench.last().yaw.abs(), 90.0, 0.01), "parallel to the wall: {}", bench.last().yaw);
    bench.step(guard(Vec2::ZERO));
    assert_eq!(bench.last().state, StateId::WallKick, "the guard press is the Superslide latch");
    let ticks = 1 + bench.until(idle(), 30, |o, _| o.state == StateId::SuperSlide);
    assert!(matches!(ticks, 19..=22), "at the end of the 0.3333 s clip: {ticks}");
    // II.2 test 12: mode Flying, gravity off, `SuperSlideLoop`; speeds 3983.3 -> 3000 cm/s over
    // 60 ticks, 3491.7 cm, Z constant; then `SlideEnd`: + 426 cm over 46 ticks; in the air at
    // the end: pawn+0xE38 = 0.6, Falling, velocity 0.
    let start = bench.last().position;
    assert_eq!(bench.last().anim.name, AnimId::SuperSlideLoop);
    assert!(near(bench.last().velocity.length(), 3983.33, 0.1), "{:?}", bench.last().velocity);
    assert!(!bench.sora.actions().superslide_ok);
    bench.run(30);
    assert!(near((bench.last().position - start).length(), 1870.8, 2.0), "after 30 ticks: {}", (bench.last().position - start).length());
    let ticks = 30 + bench.until(idle(), 40, |o, _| o.anim.name == AnimId::SlideEnd);
    assert!(matches!(ticks, 61 | 62), "{ticks}");
    let slid = (bench.last().position - start).length();
    assert!(near(slid, 3491.7, 60.0) && near(bench.last().position.z, start.z, 0.01), "slid {slid}");
    let ticks = bench.until(idle(), 60, |o, _| o.state != StateId::SuperSlide);
    assert!(matches!(ticks, 46..=48), "SlideEnd is 0.7667 s: {ticks}");
    let total = (bench.last().position - start).length();
    assert!(near(total, 3918.0, 62.0), "3492 + 426 cm: {total}");
    assert_eq!((bench.last().state, bench.last().velocity.truncate()), (StateId::Jump, Vec2::ZERO));
    assert!(near(bench.sora.combat().air_slide_cooldown, 0.6, 1.0e-4));
}

#[test]
fn the_superslide_out_of_a_flow_jump_steers_and_jumps() {
    // II.2: the guard button in a flow jump (Jump+0xF8, pawn bit 4, SUPERSLIDE): `TryFlow` comes
    // before the air slide. The stick steers: facing error x 0.975 per tick. Jump out: type 2
    // with the facing as the launch direction (+175 cm/s per tick, x 0.92).
    let mut bench = slide_into_wall(level99(), FORWARD);
    bench.step(jump_press(Vec2::ZERO));
    bench.until(idle(), 20, |o, _| o.state == StateId::Jump);
    // (The kick's jump keeps the facing of clip time 0.1 s: still toward the wall. Turn away.)
    let back = Vec2::new(0.0, -1.0);
    bench.hold(input(back, &[], &[]), 40);
    assert!(bench.last().yaw.abs() > 170.0, "{}", bench.last().yaw);
    assert_eq!(bench.step(guard(back)).state, StateId::SuperSlide, "the flow trigger wins over the air slide");
    let yaw0 = bench.last().yaw;
    let target = 90.0; // the stick's yaw (RIGHT with the camera at yaw 0): along the wall
    for n in 1..=10 {
        let out = bench.step(input(RIGHT, &[], &[]));
        let error = kh_core::sora::wrap_degrees(target - yaw0) * 0.975f32.powi(n);
        assert!(near(kh_core::sora::wrap_degrees(target - out.yaw), error, 0.05), "tick {n}: yaw {}", out.yaw);
    }
    // The flow-jump bit (pawn+0xE48 bit 3) was consumed by the kick's super jump (Jump::Begin type
    // 2, 0x141386E12) and returns on landing: no second super jump out of this slide.
    bench.step(jump_press(Vec2::ZERO));
    assert_eq!(bench.last().state, StateId::SuperSlide);
    // Kick -> guard latch -> Superslide (the bit is still set): the jump is type 2, at once.
    let mut bench = slide_into_wall(level99(), Vec2::new(0.75, 0.75));
    bench.step(guard(Vec2::ZERO));
    bench.until(idle(), 30, |o, _| o.state == StateId::SuperSlide);
    bench.run(10);
    bench.step(jump_press(Vec2::ZERO));
    assert_eq!((bench.last().state, bench.sora.jump_kind()), (StateId::Jump, Some(2)));
    assert!(!bench.sora.actions().flow_jump && !bench.sora.actions().second_jump);
    // Without SUPERSLIDE (19) the press in a flow jump is not a Superslide.
    let mut status = level99();
    status.abilities.set(Ability::Superslide, 3, 0);
    let mut bench = slide_into_wall(status, FORWARD);
    bench.step(jump_press(Vec2::ZERO));
    bench.until(idle(), 20, |o, _| o.state == StateId::Jump);
    bench.run(30);
    assert_ne!(bench.step(guard(Vec2::ZERO)).state, StateId::SuperSlide);
}

// ============================================================================ successor: decision 1(b)

#[test]
fn speed_matched_locomotion_also_plays_with_the_keyblade_out() {
    // Coordinator decision 2026-10-11: the user option `speed_matched_anim` covers the `W` and `B`
    // clip sets. Running into battle: `RunBattleStartW`, then ONE locomotion loop of the `B` set
    // whose rate matches the ground speed; battle over: `RunBattleEnd`, then the `W` set.
    let run = input(FORWARD, &[], &[]);
    let mut bench = Bench::with(level99(), TestWorld::floor(), Vec3::ZERO, Tuning::USER);
    bench.hold(run, 60);
    let n = bench.last().anim.locomotion.expect("the N loop is speed-matched");
    assert_eq!((n.walk, n.run), (AnimId::WalkLoopN, AnimId::RunLoopN));
    bench.sora.force_battle(true);
    bench.step(run);
    bench.until(run, 60, |o, _| o.anim.name != AnimId::RunBattleStartW);
    bench.hold(run, 20);
    let b = bench.last().anim.locomotion.expect("the B loop is speed-matched too");
    assert_eq!((b.walk, b.run, bench.last().anim.name), (AnimId::WalkLoopB, AnimId::RunLoopB, AnimId::RunLoopB));
    let speed = bench.last().velocity.truncate().length();
    assert!(near(b.rate * AnimId::RunLoopB.data().length, speed.min(680.0) / 680.0, 0.02) && b.run_weight > 0.99, "rate {} at {speed} cm/s", b.rate);
    // Slowing to a walk blends toward `WalkLoopB` (1.2 s) without a play call.
    let play = bench.last().anim.play_id;
    let walk = input(raw(FORWARD, 0.5), &[], &[]);
    bench.hold(walk, 60);
    let w = bench.last().anim.locomotion.expect("still the loop");
    assert_eq!((bench.last().anim.play_id, bench.last().anim.name), (play, AnimId::WalkLoopB));
    assert!(w.run_weight < 0.01 && near(w.rate * AnimId::WalkLoopB.data().length, 1.0, 0.02), "walk: weight {} rate {}", w.run_weight, w.rate);
    // Battle over while running: the set changes to `W` (Keyblade still in hand).
    bench.hold(run, 40);
    bench.sora.force_battle(false);
    bench.until(run, 100, |o, _| o.anim.name == AnimId::RunBattleEnd);
    bench.until(run, 60, |o, _| o.anim.name != AnimId::RunBattleEnd);
    bench.hold(run, 5);
    let l = bench.last().anim.locomotion.expect("the W loop");
    assert_eq!((l.walk, l.run), (AnimId::WalkLoopW, AnimId::RunLoopW));
}

#[test]
fn run_start_b_plays_on_a_normalised_clock() {
    // `RunStartB`'s samples differ in length (the -180 sample 0.3 s, the others 0.2333 s): the
    // clock's length is the blended length of the samples in use. Straight ahead in the battle
    // pose: `RunStartB` (no fallback to `RunStartN`), 14 steps, its root track complete.
    let mut bench = Bench::new();
    bench.sora.force_battle(true);
    bench.until(idle(), 200, |o, _| o.anim.name == AnimId::IdleB);
    let x0 = bench.last().position.x;
    let run = input(FORWARD, &[], &[]);
    assert_eq!(bench.step(run).anim.name, AnimId::RunStartB);
    let steps = bench.until(run, 30, |o, _| o.anim.name != AnimId::RunStartB);
    assert_eq!((steps, bench.last().anim.name), (14, AnimId::RunLoopB));
    let moved = bench.last().position.x - x0;
    let track = AnimId::RunStartN.data();
    assert!(moved > 120.0, "the root motion of the 0.2333 s sample is not cut short ({moved} cm; RunStartN's straight sample moves 180 over {} s)", track.length);
    assert!(near(bench.outs[bench.outs.len() - 3].anim.length, 0.23333, 1.0e-3));
}

#[test]
fn the_hosts_hold_draws_the_keyblade_keeps_it_through_the_idle_break_and_puts_it_away_on_release() {
    // PORT ADDITION (IMPL_ER44): Elden Ring's hero light in the dark. Not the game's code.
    let mut bench = Bench::new();
    bench.step(idle());
    assert!(!bench.sora.actions().weapon_out);
    bench.sora.hold_weapon(true);
    let out = bench.step(idle());
    assert!(bench.sora.actions().weapon_out);
    assert!(out.events.iter().any(|e| matches!(e.kind, EventKind::WeaponVisible { visible: true, .. })));
    assert!(out.events.iter().any(|e| matches!(&e.kind, EventKind::EffectSpawn { spawn, .. } if spawn.name == "VW_SO010_APP0")));
    // out of battle: the idle break comes and goes, the Keyblade stays (without the hold it would be put away)
    bench.until(idle(), 700, |o, _| o.anim.name == AnimId::IdleBreakW);
    bench.run(700);
    assert!(bench.sora.actions().weapon_out, "held through the idle break");
    // released out of battle: put away at the next step, with the game's sequence
    bench.sora.hold_weapon(false);
    let out = bench.step(idle());
    assert!(!bench.sora.actions().weapon_out);
    assert!(out.events.iter().any(|e| matches!(e.kind, EventKind::WeaponVisible { visible: false, .. })));
    // released in battle: it stays out (the game's stance rules)
    bench.sora.force_battle(true);
    bench.sora.hold_weapon(true);
    bench.run(30);
    bench.sora.hold_weapon(false);
    bench.run(2);
    assert!(bench.sora.actions().weapon_out, "in battle it stays out");
}
