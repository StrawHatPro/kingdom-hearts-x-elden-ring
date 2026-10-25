//! (stream SP, 2026-10-16) "IS SORA FASTER IN OUR PORT THAN IN VANILLA KH3?" The port's own
//! numbers, measured by stepping the core as the hosts do (60 Hz fixed step, flat floor, camera
//! yaw 0, forward = +X) with the SHIPPED port options (`Tuning::USER`: dec013 - dec016, dec014)
//! and with `Tuning::GAME_EXACT`, at the level-99 loadout and at the Elden Ring save's level-11
//! loadout (slot0: Air Slide x1, Superslide x1, Dodge, Aerial Dodge; no Glide / High Jump /
//! Doubleflight). Every bound is KH3's value from the game's data (sheets/port/sora-movement.json,
//! sora-actions.json; addresses there): the test fails if any state moves him faster or farther
//! than KH3 does. `cargo test -p kh-core --test sp_speed -- --nocapture` prints the table
//! (build/sora_tmp/party/IMPL_SP_STATE.md holds it with the KH3 column).

use kh_core::enums::ETresAbilityKind as Ability;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, Sora, StateId, Status, Tuning, Vec2, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;
const FORWARD: Vec2 = Vec2::new(0.0, 1.0);

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

/// The movement abilities of the Elden Ring save er\saves\slot0 (level 11): Air Slide x1,
/// Superslide x1, Dodge, Aerial Dodge equipped; Glide, High Jump, Doubleflight not owned.
fn level11_movement() -> Status {
    let mut s = level99();
    s.abilities.set(Ability::Airslide, 1, 1);
    s.abilities.set(Ability::Superslide, 1, 1);
    s.abilities.set(Ability::Glide, 0, 0);
    s.abilities.set(Ability::Highjump, 0, 0);
    s.abilities.set(Ability::Doubleflight, 0, 0);
    s
}

/// A raw stick value whose magnitude after the game's 0.25 dead zone is `magnitude`.
fn raw(direction: Vec2, magnitude: f32) -> Vec2 {
    direction * (0.25 + magnitude * 0.75)
}

fn buttons(list: &[PadButton]) -> PadButtons {
    list.iter().fold(PadButtons::NONE, |set, b| set.with(*b))
}

fn input(stick: Vec2, pressed: &[PadButton], held: &[PadButton]) -> Input {
    let pressed = buttons(pressed);
    Input { move_stick: stick, ..Input::default() }.with_pad(PadState { held: PadButtons(buttons(held).0 | pressed.0), pressed, released: PadButtons::NONE })
}

fn stick(direction: Vec2, magnitude: f32) -> Input {
    input(raw(direction, magnitude), &[], &[])
}

fn h(o: &Output) -> f32 {
    o.velocity.truncate().length()
}

struct Bench {
    sora: Sora,
    world: TestWorld,
    outs: Vec<Output>,
}

impl Bench {
    fn new(status: Status, tuning: Tuning) -> Bench {
        let origin = Vec3::Z * Sora::origin_height_on_floor(&SORA_PARAMS);
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(tuning).with_status(status);
        let mut b = Bench { sora, world: TestWorld::floor(), outs: Vec::new() };
        b.hold(Input::default(), 5);
        b
    }

    fn step(&mut self, input: Input) -> Output {
        let out = self.sora.step(DT, &input, &self.world);
        self.outs.push(out.clone());
        out
    }

    fn hold(&mut self, input: Input, steps: usize) -> Output {
        for _ in 0..steps {
            self.step(input);
        }
        self.outs.last().unwrap().clone()
    }

    fn until(&mut self, input: Input, limit: usize, done: impl Fn(&Output) -> bool) -> usize {
        for k in 1..=limit {
            if done(&self.step(input)) {
                return k;
            }
        }
        panic!("not reached in {limit} steps: {:?}/{}", self.outs.last().unwrap().state, self.outs.last().unwrap().sub_step);
    }

    fn pos(&self) -> Vec3 {
        self.outs.last().unwrap().position
    }

    /// The highest horizontal speed seen from output `from` on.
    fn top(&self, from: usize) -> f32 {
        self.outs[from..].iter().map(h).fold(0.0, f32::max)
    }

    /// The highest horizontal DISPLACEMENT speed (position change per step / dt, root motion
    /// included) seen from output `from` on.
    fn top_displacement(&self, from: usize) -> f32 {
        self.outs[from.max(1)..].iter().zip(&self.outs[from.max(1) - 1..]).map(|(a, b)| (a.position - b.position).truncate().length() / DT).fold(0.0, f32::max)
    }
}

/// Run straight on from rest for `seconds`: (distance, top velocity, top displacement speed).
fn straight(status: Status, tuning: Tuning, magnitude: f32, seconds: f32) -> (f32, f32, f32, Bench) {
    let mut b = Bench::new(status, tuning);
    let (x0, n0) = (b.pos().x, b.outs.len());
    b.hold(stick(FORWARD, magnitude), (seconds * 60.0).round() as usize);
    let d = b.pos().x - x0;
    let (top, disp) = (b.top(n0), b.top_displacement(n0));
    (d, top, disp, b)
}

#[test]
fn sp_ground_speeds_are_kh3s_with_the_users_options() {
    for (name, tuning) in [("USER", Tuning::USER), ("GAME_EXACT", Tuning::GAME_EXACT)] {
        // WALK: stick 0.5 (below 0.8): 680 x WalkSpeedRate 0.205882 = 140 cm/s.
        let (d, top, disp, _) = straight(level99(), tuning, 0.5, 3.0);
        println!("{name:10} walk 3 s: {d:7.1} cm, top velocity {top:6.1}, top displacement {disp:6.1} cm/s");
        assert!(top <= 140.0 + 0.5, "{name}: walk velocity {top}");
        // RUN: the first second (RunStartN's 180 cm root track, then MaxWalkSpeed 680).
        let (d1, top1, disp1, _) = straight(level99(), tuning, 1.0, 1.0);
        println!("{name:10} run 1 s:  {d1:7.1} cm, top velocity {top1:6.1}, top displacement {disp1:6.1} cm/s");
        // (The displacement peak is RunStartN's own root track 0007: 104 -> 180 cm between its last two 30 Hz keys = 2280
        // cm/s for one 1/30 s, KH3's data, root motion only, velocity 0; reconciliation A2.)
        assert!(top1 <= 680.0 + 0.5 && disp1 <= 2280.0 + 1.0, "{name}: run velocity {top1} / {disp1}");
        // SPRINT: MySprintSpeed 980 after MySprintDelay 1.0 s of running (0x1413AED70).
        let (d4, top4, disp4, b) = straight(level99(), tuning, 1.0, 4.0);
        let at = b.outs.iter().position(|o| h(o) > 680.5).map(|i| i as f32 * DT);
        println!("{name:10} run 4 s:  {d4:7.1} cm, top velocity {top4:6.1}, top displacement {disp4:6.1} cm/s, above 680 from {at:?} s (incl. 5 idle steps)");
        let disp_loop = b.top_displacement(5 + 20);
        println!("{name:10}          top displacement after the run start: {disp_loop:6.1} cm/s");
        assert!(top4 <= 980.0 + 0.5 && disp_loop <= 980.0 + 1.0, "{name}: sprint {top4} / {disp_loop}");
        // STOP from the sprint: one braking tick, then the stop clip (no root motion).
        let mut b = b;
        let x = b.pos().x;
        b.hold(Input::default(), 60);
        println!("{name:10} stop from 980: {:5.1} cm", b.pos().x - x);
    }
}

#[test]
fn sp_rolled_stick_keeps_the_sprint_only_with_continuous_turning() {
    // dec013's effect on the speed. (a) A stick rolled round the rim at 240 deg/s after a 1.5 s
    // sprint: the facing eases 0.1 a tick and stays within 75 degrees, so the game makes no
    // turn-around either: the two rules give the same path. (b) The stick snapped 90 degrees to
    // the right at a sprint (a swing below dec013's 135-in-5-ticks flick): the game's turn-around
    // (>= 75 degrees: StopMovement, RunStartN, sprint timer 0, 0x1413AF8E0) restarts him from 0 and
    // he is back at 980 only 1 s later; dec013 keeps him at 980 through the turn. The top speed
    // is 980 both ways; the time spent at it is longer with dec013.
    for (name, tuning) in [("USER", Tuning::USER), ("GAME_EXACT", Tuning::GAME_EXACT)] {
        let mut b = Bench::new(level99(), tuning);
        b.hold(stick(FORWARD, 1.0), 90);
        let n0 = b.outs.len();
        let p0 = b.pos();
        for k in 0..60 {
            let a = (k as f32 * 4.0).to_radians(); // 240 deg/s
            b.step(stick(Vec2::new(a.sin(), a.cos()), 1.0));
        }
        let path: f32 = b.outs[n0..].iter().zip(&b.outs[n0 - 1..]).map(|(a, b)| (a.position - b.position).truncate().length()).sum();
        let top = b.top(n0);
        println!("{name:10} stick rolled 240 deg/s for 1 s at a sprint: path {path:6.1} cm, top {top:5.1}, net {:5.1} cm", (b.pos() - p0).truncate().length());
        assert!(top <= 980.0 + 0.5);
        // (b) the 90-degree snap.
        let mut b = Bench::new(level99(), tuning);
        b.hold(stick(FORWARD, 1.0), 90);
        let n0 = b.outs.len();
        let p0 = b.pos();
        b.hold(stick(Vec2::new(1.0, 0.0), 1.0), 60);
        let path: f32 = b.outs[n0..].iter().zip(&b.outs[n0 - 1..]).map(|(a, b)| (a.position - b.position).truncate().length()).sum();
        let below = b.outs[n0..].iter().filter(|o| h(o) < 979.5).count();
        println!("{name:10} stick snapped 90 deg at a sprint, next 1 s: path {path:6.1} cm, {below} of 60 steps below 980, top {:5.1}, net {:5.1} cm", b.top(n0), (b.pos() - p0).truncate().length());
        assert!(b.top(n0) <= 980.0 + 0.5);
    }
}

#[test]
fn sp_jumps_dodge_air_slide_glide_are_kh3s() {
    for (level, status) in [("L99", level99()), ("L11", level11_movement())] {
        for (name, tuning) in [("USER", Tuning::USER), ("GAME_EXACT", Tuning::GAME_EXACT)] {
            // JUMP from a sprint, B held: height, air time, distance, top horizontal speed.
            let mut b = Bench::new(status.clone(), tuning);
            b.hold(stick(FORWARD, 1.0), 90);
            let (p0, n0) = (b.pos(), b.outs.len());
            b.step(input(raw(FORWARD, 1.0), &[PadButton::JUMP], &[]));
            b.until(input(raw(FORWARD, 1.0), &[], &[PadButton::JUMP]), 30, |o| !o.on_floor);
            // B held to the apex (a full jump), let go in the fall (no glide).
            let rise = b.until(input(raw(FORWARD, 1.0), &[], &[PadButton::JUMP]), 120, |o| o.state == StateId::Jump && o.sub_step == 4);
            let steps = rise + b.until(stick(FORWARD, 1.0), 300, |o| o.on_floor);
            let apex = b.outs[n0..].iter().map(|o| o.position.z).fold(f32::MIN, f32::max) - p0.z;
            let (d, top) = (b.pos().x - p0.x, b.top(n0));
            println!("{level} {name:10} sprint jump held: apex {apex:5.1} cm, air {:4.2} s, {d:6.1} cm forward, top horizontal {top:5.1} cm/s", steps as f32 * DT);
            let apex_max = if status.has(Ability::Highjump) { 461.9 } else { 266.1 };
            assert!(apex <= apex_max + 1.0 && top <= 980.0 + 0.5, "{level} {name}: {apex} / {top}");
            // DODGE ROLL from a sprint: StopMovement at Begin, then clip 0051's 600 cm root track.
            let mut b = Bench::new(status.clone(), tuning);
            b.hold(stick(FORWARD, 1.0), 90);
            let x = b.pos().x;
            let n = b.outs.len();
            b.step(input(raw(FORWARD, 1.0), &[PadButton::GUARD], &[]));
            b.until(Input::default(), 90, |o| o.state != StateId::Dodgeroll);
            let roll = b.pos().x - x;
            println!("{level} {name:10} dodge roll from a sprint: {roll:6.1} cm, top displacement {:6.1} cm/s", b.top_displacement(n + 1));
            assert!(roll <= 600.0 + 980.0 * DT + 1.0, "{level} {name}: roll {roll}");
            // AIR SLIDE at the apex: 1130 cm root x MyAirSlideSpeedScale (0.725 / 1.075 / 1.425).
            let mut b = Bench::new(status.clone(), tuning);
            b.step(input(Vec2::ZERO, &[PadButton::JUMP], &[]));
            b.until(input(Vec2::ZERO, &[], &[PadButton::JUMP]), 120, |o| o.state == StateId::Jump && o.sub_step == 4);
            let x = b.pos().x;
            let n = b.outs.len();
            b.step(input(raw(FORWARD, 1.0), &[PadButton::GUARD], &[]));
            b.until(Input::default(), 90, |o| o.state != StateId::AirSlide);
            let slide = b.pos().x - x;
            let expect = match status.count(Ability::Airslide) {
                0 | 1 => 819.25,
                2 => 1214.75,
                _ => 1610.25,
            };
            println!("{level} {name:10} air slide: {slide:6.1} cm (KH3 {expect}), top displacement {:6.1} cm/s", b.top_displacement(n + 1));
            assert!(slide <= expect + 1.0, "{level} {name}: air slide {slide}");
            // GLIDE (L99 only): MyGlideSpeed 1500 cm/s.
            if status.has(Ability::Glide) {
                let mut b = Bench::new(status.clone(), tuning);
                b.step(input(Vec2::ZERO, &[PadButton::JUMP], &[]));
                b.until(input(Vec2::ZERO, &[], &[PadButton::JUMP]), 120, |o| o.state == StateId::Jump && o.sub_step == 4);
                let n = b.outs.len();
                let x = b.pos().x;
                b.hold(input(Vec2::ZERO, &[], &[PadButton::JUMP]), 120);
                let top = b.top(n);
                println!("{level} {name:10} glide 2 s: {:6.1} cm, top {top:6.1} cm/s", b.pos().x - x);
                assert!(top <= 1500.0 + 0.5, "{level} {name}: glide {top}");
            }
        }
    }
}
