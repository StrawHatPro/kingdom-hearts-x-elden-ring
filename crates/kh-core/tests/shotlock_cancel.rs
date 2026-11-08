//! (THE USER 2026-11-02: "shotlock's startup sound effect continues to play its audio even after it's cancelled, make it
//! stop if shotlock gets cancelled") EVERY WAY OUT OF THE SHOTLOCK AIM leaves the aim mode (what the HUD's one stop rule
//! for the aim's sound keys on: `UiSnapshot::aim.open()`, kh-host hud.rs `scope_sound_rule`) and ends the slow's sound
//! state (STATE_SLOW_END): R1 let go, a hit that is not a small flinch, HP gone (death), the command menu's slow option,
//! a warp / load that drops the state (`Sora::teleport`), and the shot itself.

use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::EventKind;
use kh_core::magic::View;
use kh_core::states::HitOnPlayer;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};

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
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let key = row[0].as_str().unwrap();
        let value = if key == "abilities.equipped" { LoadoutValue::List(Vec::new()) } else { cell(&row[1]).unwrap() };
        loadout.set(key, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    held: PadButtons,
    view: Option<View>,
}

impl Bench {
    /// Sora aiming (aim mode 1, locks on a dummy 8 m ahead).
    fn aiming() -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(level99());
        let mut host = RecordingHost::new();
        let flags = TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY);
        host.targets.push(TargetInfo { id: TargetId(1), position: Vec3::new(800.0, 0.0, 0.0), center: Vec3::new(800.0, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags, hp: 1.0e6, hp_max: 1.0e6 });
        let mut b = Bench { sora, host, world: TestWorld::floor(), held: PadButtons::NONE, view: Some(View::from_camera(Vec3::new(-300.0, 0.0, 150.0), 0.0, 0.0, 90.0, 1280, 720)) };
        for _ in 0..5 {
            b.step(&[]);
        }
        let opened: Vec<Output> = (0..60).map(|_| b.step(&[PadButton::RightShoulder])).collect();
        assert!(b.sora.aim().mode == 1 && b.sora.aim().total > 0, "aiming with locks");
        assert!(b.sora.ui_snapshot().aim.open());
        assert!(slow_states(&opened).contains(&"STATE_SLOW_START"));
        b
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        let now = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let pad = PadState::from_held(now, self.held);
        self.held = now;
        let mut input = Input::default().with_pad(pad);
        input.view = self.view;
        let (sora, world) = (&mut self.sora, &self.world);
        self.host.step(world, |h| sora.step_hosted(DT, &input, h))
    }

    /// `n` steps with R1 held (or not): their outputs.
    fn steps(&mut self, held: &[PadButton], n: usize) -> Vec<Output> {
        (0..n).map(|_| self.step(held)).collect()
    }
}

fn slow_states(outs: &[Output]) -> Vec<&'static str> {
    outs.iter().flat_map(|o| o.events.iter()).filter_map(|e| if let EventKind::SoundState { name } = e.kind { Some(name) } else { None }).collect()
}

/// The aim is left (the HUD's scope closes, its sound stops) and the slow's sound state ends.
fn left(b: &Bench, outs: &[Output], route: &str) {
    assert_eq!(b.sora.aim().mode, 0, "{route}: the aim mode is left");
    assert!(!b.sora.ui_snapshot().aim.open(), "{route}: the HUD's aim is closed");
    assert!(slow_states(outs).contains(&"STATE_SLOW_END"), "{route}: STATE_SLOW_END ({:?})", slow_states(outs));
}

#[test]
fn r1_let_go_cancels() {
    let mut b = Bench::aiming();
    let outs = b.steps(&[], 2);
    left(&b, &outs, "R1 released");
}

#[test]
fn a_hit_that_is_not_a_small_flinch_cancels() {
    let mut b = Bench::aiming();
    b.sora.take_hit(HitOnPlayer { direction: Vec3::new(-1.0, 0.0, 0.0), causer_yaw: 180.0, ..HitOnPlayer::new(Kind::Blow, 10.0, TargetId(1), Vec3::new(150.0, 0.0, 0.0)) });
    let outs = b.steps(&[PadButton::RightShoulder], 3);
    left(&b, &outs, "a Blow hit");
}

#[test]
fn death_cancels() {
    let mut b = Bench::aiming();
    b.sora.status_mut().hp = 0;
    let outs = b.steps(&[PadButton::RightShoulder], 2);
    left(&b, &outs, "HP 0");
}

#[test]
fn the_command_menus_slow_option_cancels() {
    let mut b = Bench::aiming();
    b.sora.command_mut().slow_applied = true;
    let outs = b.steps(&[PadButton::RightShoulder], 2);
    left(&b, &outs, "the command menu's slow");
}

/// A warp / a load that puts him elsewhere drops the state without its exit: the aim is closed at once and the slow's
/// sound state, owed, goes out with the next step (before: never: the footsteps stayed at pitch 0.3).
#[test]
fn a_warp_during_the_aim_cancels_and_ends_the_slow() {
    let mut b = Bench::aiming();
    b.sora.teleport(Pose { position: Vec3::new(0.0, 2000.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 });
    assert!(!b.sora.ui_snapshot().aim.open(), "closed at the teleport");
    let outs = b.steps(&[], 2);
    left(&b, &outs, "a teleport");
    // owed once, not again
    assert!(!slow_states(&b.steps(&[], 5)).contains(&"STATE_SLOW_END"));
}

/// The shot: the aim leaves 1 for 2 (KH3's scope state `select`: the aim's sound stops there too) and the slow's sound
/// state ends with it.
#[test]
fn the_shot_leaves_the_aim_and_ends_the_slow() {
    let mut b = Bench::aiming();
    let outs = vec![b.step(&[PadButton::RightShoulder, PadButton::ATTACK])];
    assert_eq!(b.sora.aim().mode, 2);
    assert!(!b.sora.ui_snapshot().aim.open());
    assert!(slow_states(&outs).contains(&"STATE_SLOW_END"));
}

/// (PARTY-AI, AUDIT-A WORLDTIME) KH3's aim slow on the PARTY: `SetTypeRates(CH_PLAYER, 0x4D9E, aim2.slow.rate 0.4)` puts the
/// FRIEND type (10, Donald / Goofy / every AI friend) at x0.4 with Sora and the world while he aims; the fire puts only PLAYER
/// back to 1 (the friends stay slowed through the volley); the shotlock's end (`ResetTypeRates`) gives everything back.
#[test]
fn the_aim_slows_the_friend_type_to_0_4_until_the_shotlock_ends() {
    const FRIEND: u32 = 10;
    let mut b = Bench::aiming();
    assert_eq!((b.sora.time_dilation(1), b.sora.time_dilation(FRIEND), b.sora.time_dilation(2)), (0.4, 0.4, 0.4), "aiming: Sora, the friends and the enemies");
    b.step(&[PadButton::RightShoulder, PadButton::ATTACK]);
    assert_eq!((b.sora.time_dilation(1), b.sora.time_dilation(FRIEND)), (1.0, 0.4), "the fire: only Sora is back");
    let mut ended = false;
    for _ in 0..1200 {
        b.step(&[]);
        if b.sora.time_dilation(FRIEND) == 1.0 {
            ended = true;
            break;
        }
    }
    assert!(ended && b.sora.aim().mode == 0, "the shotlock's end gives the friends back (aim mode {})", b.sora.aim().mode);
}
