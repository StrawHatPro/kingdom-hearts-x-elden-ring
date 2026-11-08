//! (MIMIC-STANCE) Sora's stance break (combat/stance.rs; THE USER 2026-11-05: "make sure Goofy's mimic tear boss ... can stagger
//! you by breaking your stance"): KH3's STUN state (122) for the window, no action, hits and stick flicks do not end it, the
//! release ends it, the end clip and Idle after.

use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::EventKind;
use kh_core::states::HitOnPlayer;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, Tuning, Vec2, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;

fn level99() -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}")).trim_start_matches('\u{feff}')).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let v = match &row[1] {
            serde_json::Value::Number(n) => LoadoutValue::Number(n.as_f64().unwrap()),
            serde_json::Value::String(s) => LoadoutValue::Text(s.clone()),
            serde_json::Value::Array(a) => LoadoutValue::List(a.iter().map(|i| i.as_str().unwrap().to_string()).collect()),
            serde_json::Value::Object(m) => LoadoutValue::Map(m.iter().map(|(k, v)| (k.clone(), v.as_f64().unwrap())).collect()),
            _ => continue,
        };
        loadout.set(row[0].as_str().unwrap(), &v).unwrap();
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    outs: Vec<Output>,
}

impl Bench {
    fn new() -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(level99());
        let mut bench = Bench { sora, host: RecordingHost::new(), world: TestWorld::floor(), outs: Vec::new() };
        bench.idle(5);
        bench
    }

    fn step_with(&mut self, held: &[PadButton], stick: Vec2) -> Output {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input { move_stick: stick, ..Input::default() }.with_pad(PadState::from_held(set, PadButtons::NONE));
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        self.outs.push(out.clone());
        out
    }

    fn idle(&mut self, steps: usize) -> Output {
        let mut last = self.step_with(&[], Vec2::ZERO);
        for _ in 1..steps {
            last = self.step_with(&[], Vec2::ZERO);
        }
        last
    }

    fn until(&mut self, limit: usize, what: &str, test: impl Fn(&Bench, &Output) -> bool) -> usize {
        for n in 1..=limit {
            let out = self.step_with(&[], Vec2::ZERO);
            if test(self, &out) {
                return n;
            }
        }
        panic!("{what}: not within {limit} steps; last state {:?}", self.outs.last().map(|o| o.state));
    }

    fn clips(&self, from: usize) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for o in &self.outs[from..] {
            if out.last() != Some(&o.anim.name.name()) {
                out.push(o.anim.name.name());
            }
        }
        out
    }
}

#[test]
fn stance_a_break_holds_sora_in_the_stun_state_for_its_window_and_he_recovers() {
    let mut b = Bench::new();
    assert!(b.sora.stance_breakable());
    assert!(b.sora.stance_break(2.5));
    let from = b.outs.len();
    b.until(3, "the stun state", |_, o| o.state == StateId::BadstatStun);
    // no action while broken: the attack, the jump, the guard do nothing
    for k in 0..60 {
        let button = [PadButton::ATTACK, PadButton::JUMP, PadButton::GUARD][k % 3];
        let o = b.step_with(&[button], Vec2::ZERO);
        b.step_with(&[], Vec2::ZERO);
        assert_eq!(o.state, StateId::BadstatStun, "no action out of a broken stance (step {k})");
    }
    let lasted = 120 + b.until(200, "the break's end", |b, _| !b.sora.stance().broken());
    assert!((148..=152).contains(&lasted), "the break lasted {lasted} steps (2.5 s)");
    b.until(200, "Idle after the break", |_, o| o.state == StateId::Idle);
    assert_eq!(b.clips(from)[..3], ["BadStatStunStart", "BadStatStunLoop", "BadStatStunEnd"], "KH3's stun reaction");
    assert_eq!(b.sora.combat().bad.kind, 0, "no status was put on him");
    let events: Vec<&EventKind> = b.outs.iter().flat_map(|o| o.events.iter().map(|e| &e.kind)).collect();
    assert!(!events.iter().any(|e| matches!(e, EventKind::BadStatus { .. })), "no bad status event");
    assert!(!events.iter().any(|e| matches!(e, EventKind::EffectSpawn { spawn, .. } if spawn.id.0 == 0xC58)), "not the STUN status's stars");
    // free again: the attack starts
    let attacked = (0..30).any(|_| {
        let o = b.step_with(&[PadButton::ATTACK], Vec2::ZERO);
        b.step_with(&[], Vec2::ZERO);
        o.state == StateId::Attack
    });
    assert!(attacked, "he acts again after the break");
}

#[test]
fn stance_hits_and_flicks_do_not_end_a_break_its_hold_lengthens_it_and_the_release_ends_it() {
    let mut b = Bench::new();
    assert!(b.sora.stance_break(2.5));
    b.until(3, "the stun state", |_, o| o.state == StateId::BadstatStun);
    b.idle(30);
    // stick flicks shorten a STUN status, never a break
    let before = b.sora.stance().left;
    for k in 0..10 {
        b.step_with(&[], Vec2::new(if k % 2 == 0 { 1.0 } else { -1.0 }, 0.0));
    }
    let taken = before - b.sora.stance().left;
    assert!((taken - 10.0 * DT).abs() < 1e-3, "only the clock ran: {taken}");
    // a damaging hit: its reaction plays, then the break goes on
    b.sora.take_hit(HitOnPlayer { direction: Vec3::new(-1.0, 0.0, 0.0), ..HitOnPlayer::new(Kind::Small, 5.0, kh_core::TargetId(1), Vec3::new(200.0, 0.0, 0.0)) });
    b.until(5, "the hit's reaction", |_, o| o.state == StateId::Damage);
    assert!(b.sora.stance().broken(), "a hit does not end the break (the enemy side's BROKEN rule)");
    b.until(120, "back in the stun", |_, o| o.state == StateId::BadstatStun);
    // a Breaker winds up: the hold
    b.sora.stance_hold(3.0);
    assert!((b.sora.stance().left - 3.0).abs() < 1e-4);
    b.idle(60);
    assert!(b.sora.stance().broken());
    // the slam is about to land: the release
    assert!(b.sora.stance_release());
    assert!(!b.sora.stance_release(), "once");
    b.until(120, "Idle after the release", |_, o| o.state == StateId::Idle);
}

#[test]
fn stance_a_knocked_out_sora_is_not_broken() {
    let mut b = Bench::new();
    b.sora.status_mut().hp = 0;
    assert!(!b.sora.stance_breakable() && !b.sora.stance_break(2.5));
    b.sora.status_mut().hp = 10;
    assert!(!b.sora.stance_break(0.0), "no time, no break");
    assert!(b.sora.stance_break(1.0));
}
