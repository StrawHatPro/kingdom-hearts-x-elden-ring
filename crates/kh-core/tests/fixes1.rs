//! Fixes pass 1 (2026-10-12, the user's play reports): the Keyblade in the hand at once with the
//! first attack, and the slope slide from any direction. Both are PORT OPTIONS of `Tuning`
//! (user decisions); each test also runs the game's rule (`Tuning::GAME_EXACT`).
//! 60 Hz fixed step, the level-99 loadout.

use kh_core::host::{EventKind, NullHost};
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{surface, Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, Sora, StateId, Status, Tuning, Vec2, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;

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

struct Bench {
    sora: Sora,
    world: TestWorld,
}

impl Bench {
    fn new(world: TestWorld, position: Vec3, yaw: f32, tuning: Tuning) -> Bench {
        let origin = position + Vec3::Z * Sora::origin_height_on_floor(&SORA_PARAMS);
        let mut bench = Bench { sora: Sora::new(SORA_PARAMS, Pose { position: origin, yaw }).with_tuning(tuning).with_status(level99()), world };
        for _ in 0..5 {
            bench.step(Input::default());
        }
        bench
    }

    fn step(&mut self, input: Input) -> Output {
        let mut null = NullHost::default();
        let mut host = null.host(&self.world);
        self.sora.step_hosted(DT, &input, &mut host)
    }
}

fn press(button: PadButton) -> Input {
    let pressed = PadButtons::NONE.with(button);
    Input::default().with_pad(PadState { held: pressed, pressed, released: PadButtons::NONE })
}

/// The `WeaponVisible { visible: true }` events of the steps from an attack press on, as
/// (steps after the press, instant), until the attack state has run for a while.
fn weapon_events(tuning: Tuning) -> (Vec<(usize, bool)>, usize) {
    let mut bench = Bench::new(TestWorld::floor(), Vec3::ZERO, 0.0, tuning);
    let mut shown = Vec::new();
    let mut attack_began = usize::MAX;
    for step in 0..40 {
        let out = bench.step(if step == 0 { press(PadButton::ATTACK) } else { Input::default() });
        if out.state == StateId::Attack && attack_began == usize::MAX {
            attack_began = step;
        }
        for event in &out.events {
            if let EventKind::WeaponVisible { visible: true, instant } = event.kind {
                shown.push((step, instant));
            }
        }
    }
    (shown, attack_began)
}

#[test]
fn the_first_attack_draws_the_keyblade_in_its_first_frame() {
    // USER OPTION (default): the event of the step in which the Attack state begins asks for the
    // mesh AT ONCE. No enemy is near and nothing was hit: the draw is the attack's own
    // (Attack Begin 0x141333210: `Draw(1, 1, 1.0)`), not the battle flag's.
    let (shown, began) = weapon_events(Tuning::USER);
    assert!(began <= 2, "the attack begins with the press: step {began}");
    assert_eq!(shown, vec![(began, true)], "one draw, in the attack's first step, instant");
    // The game's rule: the same draw in the same step, with the appear sequence (the host shows
    // the mesh 0.22 s later).
    let (shown, began) = weapon_events(Tuning::GAME_EXACT);
    assert_eq!(shown, vec![(began, false)]);
}

/// A 30 degree slide floor that descends toward +X (fall line yaw 0), flagged as the test
/// level's ramp is; the flat floor is its run-out.
fn slope() -> TestWorld {
    let n = Vec3::new(30.0f32.to_radians().sin(), 0.0, 30.0f32.to_radians().cos());
    TestWorld::floor().with_plane_through(Vec3::new(0.0, 0.0, 1500.0), n).with_flags(2, surface::WALL_KICK | surface::SLOPE_SLIDE).with_flags(1, surface::WALL_KICK | surface::SLOPE_SLIDE)
}

/// He stands on the slope at x = 600 (z = 1500 - 600 tan 30) facing UPHILL and the stick is held
/// uphill (camera yaw 0: stick back = -X) for two seconds. Returns (lowest x reached, x at the
/// end, slid at all).
fn walk_up(tuning: Tuning) -> (f32, f32, bool) {
    let x0 = 600.0f32;
    let mut bench = Bench::new(slope(), Vec3::new(x0, 0.0, 1500.0 - x0 * 30.0f32.to_radians().tan() + 0.2), 180.0, tuning);
    let uphill = Input { move_stick: Vec2::new(0.0, -1.0), ..Input::default() };
    let (mut lowest, mut slid, mut last) = (f32::MAX, false, x0);
    for _ in 0..120 {
        let out = bench.step(uphill);
        lowest = lowest.min(out.position.x);
        slid |= out.state == StateId::Slopeslide;
        last = out.position.x;
    }
    (lowest, last, slid)
}

#[test]
fn a_flagged_slope_cannot_be_walked_up() {
    // USER OPTION (default): stepping onto the slide floor slides him down it whatever he faces
    // and however hard the stick is held uphill; he never gains ground uphill.
    let (lowest, last, slid) = walk_up(Tuning::USER);
    assert!(slid, "the slide starts");
    assert!(lowest > 600.0 - 15.0, "no ground gained uphill: lowest x {lowest}");
    assert!(last > 600.0 + 500.0, "he went down the slope: x {last}");
    // The game's rule (start probe 0x14063D9C0: he must face downhill within 50 degrees): the
    // same input walks up the slope.
    let (lowest, last, slid) = walk_up(Tuning::GAME_EXACT);
    assert!(!slid && last < 600.0 - 300.0 && lowest <= last + 1.0, "game-exact: he walks up: x {last}");
}

#[test]
fn the_slide_option_steers_only_within_its_cone() {
    // Sliding with the stick held to the side: the facing stays within the cone round the fall
    // line (50 degrees), so he cannot curve back uphill.
    let mut bench = Bench::new(slope(), Vec3::new(0.0, 0.0, 1500.2), 90.0, Tuning::USER);
    let side = Input { move_stick: Vec2::new(-1.0, -0.4), ..Input::default() };
    let mut worst = 0.0f32;
    let mut slid = false;
    for _ in 0..90 {
        let out = bench.step(side);
        if out.state == StateId::Slopeslide {
            slid = true;
            worst = worst.max(out.yaw.abs());
        }
    }
    assert!(slid && worst <= Tuning::USER.slide_steer_cone + 0.01, "{worst}");
}
