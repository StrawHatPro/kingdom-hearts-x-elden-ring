//! (FOCUSORB) WHERE SORA'S FOCUS COMES FROM IN KH3, against the port (docs/sora-magic-spec.md S1.2 and part G 5; the trace
//! of build/sora_tmp/weapons/FOCUSORB/STATE.md). KH3's sources: focus prizes (+3 / +5, `ApplyPrize` 0x1405AB950 ->
//! `Recover` 0x1405B57D0), items (Refocuser 40 %, Hi-Refocuser 100 %, Elixirs), the Focus Syphon shotlock (FOCUS_ASPIR 46),
//! the save point's `Recover(n, n, n, 1)`, and the aim time left when a shotlock is released (0x14140CCB0). NOT sources:
//! landed hits (the hit-dealt notify 0x141442DE0 adds `MyFocusPointAddHit` = 0 for Sora) and time (the +7 per second of
//! 0x1413F8EF0 runs only with game option B bit 6, 0x140EAB160, a cheat-style option that is off in a normal game).

use kh_core::status::{LoadoutValue, PrizeKind};
use kh_core::test_world::TestWorld;
use kh_core::ui_model::Gauges;
use kh_core::{Input, Loadout, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StatusRequest, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;

fn level99() -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let value = match &row[1] {
            serde_json::Value::Number(n) => n.as_f64().map(LoadoutValue::Number),
            serde_json::Value::String(s) => Some(LoadoutValue::Text(s.clone())),
            serde_json::Value::Array(items) => items.iter().map(|i| i.as_str().map(str::to_string)).collect::<Option<Vec<_>>>().map(LoadoutValue::List),
            serde_json::Value::Object(map) => map.iter().map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect::<Option<Vec<_>>>().map(LoadoutValue::Map),
            _ => None,
        };
        loadout.set(row[0].as_str().unwrap(), &value.unwrap()).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
}

fn dummy(x: f32) -> TargetInfo {
    TargetInfo { id: TargetId(1), position: Vec3::new(x, 0.0, 0.0), center: Vec3::new(x, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
}

impl Bench {
    fn new() -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(level99());
        let mut host = RecordingHost::new();
        host.targets.push(dummy(200.0));
        let mut bench = Bench { sora, host, world: TestWorld::floor() };
        bench.steps(5, false);
        bench
    }

    fn step(&mut self, attack: bool) {
        let held = if attack { PadButtons::NONE.with(PadButton::ATTACK) } else { PadButtons::NONE };
        let input = Input::default().with_pad(PadState::from_held(held, PadButtons::NONE));
        let (sora, world) = (&mut self.sora, &self.world);
        self.host.step(world, |h| sora.step_hosted(DT, &input, h));
    }

    fn steps(&mut self, n: usize, attack: bool) {
        for _ in 0..n {
            self.step(attack);
        }
    }
}

/// KH3 0x141442DE0: a hit Sora lands adds `MyFocusPointAddHit` (0 for him) and nothing else to the focus gauge; ten
/// seconds of idling add nothing either (no timed regeneration without option B6). The landed hits are real: the dummy
/// took damage and the style gauge (the same notify's other half) rose.
#[test]
fn landed_hits_and_time_give_sora_no_focus() {
    let mut b = Bench::new();
    let max = b.sora.status().max_focus();
    assert_eq!(max, 100, "KH3's max focus is 100, constant (magic spec part G 5)");
    b.sora.status_mut().set_focus(40);
    // Two full combos into the dummy, pressing attack every other step.
    for _ in 0..360 {
        b.step(true);
        b.step(false);
    }
    let dealt: f32 = b.host.hits().iter().map(|(r, _)| r.damage).sum();
    assert!(b.host.hits().len() >= 4 && dealt > 0.0, "the combo landed: {} hit(s), {dealt} damage", b.host.hits().len());
    assert!(b.sora.combat().style.gauge > 0.0, "the hit notify ran (style points)");
    assert_eq!(b.sora.status().focus, 40, "KH3: MyFocusPointAddHit is 0 for Sora: no focus per hit");
    b.steps(600, false);
    assert_eq!(b.sora.status().focus, 40, "KH3: no timed refill in a normal game (0x1413F8EF0 needs option B6)");
}

/// A focus orb collected (`ApplyPrize` 7 / 8: `Recover(0, 0, 3 / 5, 2)`) fills the gauge by KH3's amounts, clamped at the
/// maximum, and the HUD's focus gauge (`pcExGg`, `Gauges::of`) rises with it; the "focus empty" flag clears only at full.
#[test]
fn a_focus_orb_fills_the_gauge_by_kh3s_amounts_and_the_hud_follows() {
    let mut b = Bench::new();
    b.sora.status_mut().set_focus(1);
    b.sora.status_mut().set_focus(0);
    assert!(b.sora.status().focus_empty);
    let frame = |s: &Status| Gauges::of(s, 0.0, 0.0).focus_frame;
    let before = frame(b.sora.status());
    b.sora.status_request(StatusRequest::PrizeCollected { kind: PrizeKind::FocusSmall, amount: 0 });
    b.steps(1, false);
    assert_eq!(b.sora.status().focus, 3, "a small focus orb: +3");
    b.sora.status_request(StatusRequest::PrizeCollected { kind: PrizeKind::FocusLarge, amount: 0 });
    b.steps(1, false);
    assert_eq!(b.sora.status().focus, 8, "a large focus orb: +5");
    assert!(frame(b.sora.status()) > before, "the HUD's focus gauge rose");
    assert!(b.sora.status().focus_empty, "a partial refill leaves the focus-empty flag (0x1405B94F0)");
    for _ in 0..30 {
        b.sora.status_request(StatusRequest::PrizeCollected { kind: PrizeKind::FocusLarge, amount: 0 });
        b.steps(1, false);
    }
    assert_eq!(b.sora.status().focus, 100, "clamped at the maximum");
    assert!(!b.sora.status().focus_empty && Gauges::of(b.sora.status(), 0.0, 0.0).focus_is_max);
}
