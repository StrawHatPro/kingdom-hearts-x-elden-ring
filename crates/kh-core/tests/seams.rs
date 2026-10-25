//! The integration pass (build/sora_tmp/IMPL_INTEGRATION_STATE.md, 2026-10-12): the airstep's
//! arrival hand-offs to the traversal states (magic spec H 4.6: `PoleCheck` -> POLE_TURN,
//! `EndCheck` -> DANGLE) and the HUD model fields the world layer draws from (UI spec 7.1, 7.3:
//! soft target, push-button prompt, shotlock aim). Everything is driven through the pad.

use kh_core::magic::{AirstepPoint, View};
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::ui_model::{ButtonPrompt, PromptKind};
use kh_core::{surface, CameraState, Input, Loadout, Output, PadButton, PadButtons, PadState, Pole, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec2, Vec3, SORA_PARAMS};

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

/// The level-99 loadout with exactly these abilities equipped.
fn level99(equipped: &[&str]) -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let key = row[0].as_str().unwrap();
        let value = if key == "abilities.equipped" { LoadoutValue::List(equipped.iter().map(|s| s.to_string()).collect()) } else { cell(&row[1]).unwrap() };
        loadout.set(key, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
}

fn dummy(id: u32, x: f32, y: f32) -> TargetInfo {
    TargetInfo { id: TargetId(id), position: Vec3::new(x, y, 0.0), center: Vec3::new(x, y, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    held: PadButtons,
    view: Option<View>,
}

impl Bench {
    fn new(status: Status, world: TestWorld, dummies: &[TargetInfo]) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(status);
        let mut host = RecordingHost::new();
        host.targets.extend_from_slice(dummies);
        let mut bench = Bench { sora, host, world, held: PadButtons::NONE, view: Some(View::from_camera(Vec3::new(-300.0, 0.0, 150.0), 0.0, 0.0, 90.0, 1280, 720)) };
        bench.hold(&[], 5);
        bench
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        let now = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let pad = PadState::from_held(now, self.held);
        self.held = now;
        let mut input = Input::default().with_pad(pad);
        input.view = self.view;
        input.move_stick = Vec2::ZERO;
        let (sora, world) = (&mut self.sora, &self.world);
        self.host.step(world, |h| sora.step_hosted(DT, &input, h))
    }

    fn hold(&mut self, held: &[PadButton], steps: usize) -> Output {
        let mut out = self.sora.output();
        for _ in 0..steps {
            out = self.step(held);
        }
        out
    }

    /// Looks at `point` from behind him, aims (RB held) and airsteps (X) at what is in the reticle.
    fn airstep_at(&mut self, point: Vec3) {
        let eye = Vec3::new(-300.0, 0.0, 150.0);
        let d = point - eye;
        let pitch = d.z.atan2(d.truncate().length()).to_degrees();
        self.view = Some(View::from_camera(eye, d.y.atan2(d.x).to_degrees(), pitch, 90.0, 1280, 720));
        self.hold(&[PadButton::RightShoulder], 40);
        assert!(self.sora.aim().can_airstep, "no airstep target in the reticle");
        self.step(&[PadButton::RightShoulder, PadButton::GUARD]);
        for _ in 0..3 {
            if self.step(&[]).state == StateId::Athleticflow {
                return;
            }
        }
        panic!("the airstep did not start: {:?}", self.sora.output().state);
    }

    /// Steps until the state is `id`; the states passed on the way.
    fn until_state(&mut self, id: StateId, max: usize) -> Vec<StateId> {
        let mut seen = Vec::new();
        for _ in 0..max {
            let state = self.step(&[]).state;
            if seen.last() != Some(&state) {
                seen.push(state);
            }
            if state == id {
                return seen;
            }
        }
        panic!("the state never became {id:?}: {seen:?}");
    }
}

/// Magic spec H 4.6 `PoleCheck`: an airstep whose LAST target is a kind-6 point on a pole hands
/// over to `TresPlayerState_PoleTurn` (24) through `TryPoleTurn`; without the pole's ability the
/// in-state pole stop runs instead and the state ends by itself.
#[test]
fn an_airstep_to_a_pole_point_hands_over_to_the_pole_turn() {
    let world = || {
        let mut world = TestWorld::floor();
        world.poles.push(Pole { id: 4, base1: Vec3::new(1500.0, 0.0, 150.0), base2: Vec3::new(1500.0, 0.0, 1100.0), check_radius: 50.0, attach_radius: 30.0, horizontal: false, active: true });
        world
    };
    let point = Vec3::new(1500.0, 0.0, 450.0);
    let mut b = Bench::new(level99(&["POLE_SPIN", "POLE_SWING", "SUPERSLIDE"]), world(), &[]);
    b.sora.magic_mut().airstep_points.push(AirstepPoint::pole(7, point));
    b.airstep_at(point);
    // The aim's airstep mark is in the HUD model while the chain exists (the state clears it at its End).
    let seen = b.until_state(StateId::PoleTurn, 200);
    assert_eq!(seen, vec![StateId::Athleticflow, StateId::PoleTurn], "straight from the arrival into the pole turn");
    // He is on the pole: within the attach radius of its axis, and the spin leaves by a Superslide.
    b.hold(&[], 20);
    let out = b.sora.output();
    assert!((out.position.truncate() - Vec2::new(1500.0, 0.0)).length() < 40.0, "{:?}", out.position);
    b.until_state(StateId::SuperSlide, 200);

    // Without POLE_SPIN: `TryPoleTurn` refuses; the first-arrival flag gives the in-state stop.
    let mut b = Bench::new(level99(&[]), world(), &[]);
    b.sora.magic_mut().airstep_points.push(AirstepPoint::pole(7, point));
    b.airstep_at(point);
    let mut pole_stop = false;
    for _ in 0..300 {
        b.step(&[]);
        match b.sora.airstep() {
            Some(view) => pole_stop |= view.mode == 2,
            None => break,
        }
    }
    assert!(pole_stop, "mode 2 (the in-state pole stop) never ran");
    assert!(b.sora.airstep().is_none() && b.sora.output().state != StateId::PoleTurn);
}

/// Magic spec H 4.6 `EndCheck`: the last target is a point in front of a hangable ledge: the
/// ledge probe finds the edge and DANGLE (6) is pushed with the climb-up variant; he ends on top.
#[test]
fn an_airstep_to_a_ledge_point_hangs_and_climbs() {
    let ledge = || TestWorld::floor().with_box(Vec3::new(1500.0, -300.0, 0.0), Vec3::new(2100.0, 300.0, 300.0)).flagged(surface::DANGLE);
    let point = Vec3::new(1430.0, 0.0, 200.0);
    let mut b = Bench::new(level99(&[]), ledge(), &[]);
    b.sora.magic_mut().airstep_points.push(AirstepPoint::wall(8, point));
    b.airstep_at(point);
    let seen = b.until_state(StateId::Dangle, 200);
    assert_eq!(seen, vec![StateId::Athleticflow, StateId::Dangle]);
    b.until_state(StateId::Idle, 400);
    let out = b.sora.output();
    assert!(out.on_floor && (out.position.z - 300.0).abs() < 2.0 && out.position.x > 1500.0, "on top of the ledge: {:?}", out.position);

    // The same point at a wall that is not hangable: `SlideEnd`, no hang.
    let mut b = Bench::new(level99(&[]), TestWorld::floor().with_box(Vec3::new(1500.0, -300.0, 0.0), Vec3::new(2100.0, 300.0, 300.0)), &[]);
    b.sora.magic_mut().airstep_points.push(AirstepPoint::wall(8, point));
    b.airstep_at(point);
    let mut states = Vec::new();
    for _ in 0..200 {
        states.push(b.step(&[]).state);
    }
    assert!(!states.contains(&StateId::Dangle));
}

/// UI spec 7.1 / 7.3 and the prompt function 0x1416793E0: what the snapshot carries for the
/// world layer of the HUD.
#[test]
fn the_snapshot_carries_soft_target_aim_and_prompts() {
    let mut b = Bench::new(level99(&["SONIC_SLASH"]), TestWorld::floor(), &[dummy(1, 500.0, 0.0)]);
    b.hold(&[], 10);
    let snapshot = b.sora.ui_snapshot();
    // The soft target is the search's current target while nothing is locked.
    assert_eq!(snapshot.soft_target, b.sora.combat().lockon.current);
    assert!(snapshot.soft_target == TargetId(1) && snapshot.lock_on == TargetId::NONE && snapshot.prompt.is_none() && !snapshot.aim.open());
    // A manual lock (RB tap) moves it to `lock_on`.
    b.step(&[PadButton::RightShoulder]);
    b.hold(&[], 3);
    let snapshot = b.sora.ui_snapshot();
    assert_eq!((snapshot.lock_on, snapshot.soft_target), (TargetId(1), TargetId::NONE));
    b.step(&[PadButton::RightShoulder]);
    b.hold(&[], 3);

    // Aiming: the scope is open with the entry's aim time; the airstep mark is the dummy.
    b.hold(&[PadButton::RightShoulder], 40);
    let aim = b.sora.ui_snapshot().aim;
    assert!(aim.open() && aim.mode == 1 && aim.aim_time > 0.0, "{aim:?}");
    assert_eq!(aim.airstep.iter().map(|m| m.target).collect::<Vec<_>>(), vec![TargetId(1)]);
    // The airstep: at the enemy stop the "ATTACK!" prompt is up while the lock flag is.
    b.step(&[PadButton::RightShoulder, PadButton::GUARD]);
    let mut prompted = false;
    for _ in 0..200 {
        b.step(&[]);
        let Some(view) = b.sora.airstep() else { break };
        let prompt = b.sora.ui_snapshot().prompt;
        assert_eq!(prompt.is_some(), view.flags & 0x01 != 0);
        if let Some(prompt) = prompt {
            assert_eq!(prompt, ButtonPrompt { kind: PromptKind::AirstepAttack, target: TargetId(1) });
            prompted = true;
        }
    }
    assert!(prompted, "the attack prompt never showed");

    // The risk-dodge prompt follows the pawn flag the Reflect state sets (FUN_141414170).
    b.sora.actions_mut().risk_prompt = true;
    b.sora.actions_mut().risk_chosen = Some((TargetId(1), 1));
    assert_eq!(b.sora.ui_snapshot().prompt, Some(ButtonPrompt { kind: PromptKind::RiskDodge, target: TargetId(1) }));
    assert_eq!(b.sora.camera_state(), CameraState::None);
}
