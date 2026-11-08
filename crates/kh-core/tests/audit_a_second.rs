//! (AUDIT-A SECOND) THE SECOND FORM FAMILY'S INVINCIBILITY, as KH3 has it (decompiled C, asm checked):
//! - STYLECHANGE Begin 0x1413CBA70 calls pawn vt+0x2068(1) (slot 1037 = 0x1405B9120: pawn+0x630 bit 2, `m_bIsInvincible`,
//!   read by IsInvincible vt+0x2060 0x141435A90 -> 0x1405AD120); its End 0x1413CE760 calls vt+0x2068(0);
//! - every finish command's begin event (UTresStateBase 0x14150A1F0, STYLEFINISH Begin's vt+0x338) calls vt+0x2068(1); the
//!   end event 0x14150C440 first calls pawn vt+0x23B8 = 0x1405A5E00, which calls vt+0x2068(0) (`xor edx, edx` before the
//!   call at 0x1405A5E8F..).
//!
//! The port's bit is `Status::damage_invalid` (states/damage.rs `take_hit`: a hit that is not RECOVER does nothing). A hit
//! during the transformation and during Stun Impact costs no HP and does not end the state; the same hit after the form
//! does. Trace: build/sora_tmp/weapons/AUDIT-A/logs/second_invincible.trace.

use std::fmt::Write as _;

use kh_core::enums::ETresDamageKind as Kind;
use kh_core::states::HitOnPlayer;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};

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
    trace: String,
    steps: usize,
    /// The state after the last step.
    state: StateId,
}

impl Bench {
    fn new() -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(level99());
        let mut host = RecordingHost::new();
        let flags = TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY);
        host.targets.push(TargetInfo { id: TargetId(1), position: Vec3::new(300.0, 0.0, 0.0), center: Vec3::new(300.0, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags, hp: 1.0e6, hp_max: 1.0e6 });
        let mut b = Bench { sora, host, world: TestWorld::floor(), held: PadButtons::NONE, trace: String::new(), steps: 0, state: StateId::Idle };
        b.idle(5);
        b
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        let now = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input::default().with_pad(PadState::from_held(now, self.held));
        self.held = now;
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        self.steps += 1;
        self.state = out.state;
        out
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    fn press(&mut self, b: PadButton) {
        self.step(&[b]);
        self.step(&[]);
    }

    /// A 30-point small hit from the dummy, applied at the next step; returns (state before, state after, HP lost).
    fn hit(&mut self, what: &str) -> (StateId, StateId, i32) {
        let before = (self.state, self.sora.status().hp, self.sora.status().damage_invalid);
        let from = Vec3::new(300.0, 0.0, 0.0);
        self.sora.take_hit(HitOnPlayer { direction: Vec3::new(-1.0, 0.0, 0.0), causer_yaw: 180.0, ..HitOnPlayer::new(Kind::Small, 30.0, TargetId(1), from) });
        let out = self.step(&[]);
        let lost = before.1 - self.sora.status().hp;
        let _ = writeln!(self.trace, "step {:4} {what}: state {:?} -> {:?}, invincible {}, HP {} -> {} (lost {lost})", self.steps, before.0, out.state, before.2, before.1, self.sora.status().hp);
        (before.0, out.state, lost)
    }
}

#[test]
fn second_form_is_invincible_through_the_transformation_and_its_finish() {
    let mut b = Bench::new();
    b.sora.debug_add_style(60.0);
    b.idle(2);
    b.sora.debug_add_style(40.0);
    b.idle(15);
    b.press(PadButton::SITUATION);
    for _ in 0..60 {
        if b.state == StateId::Stylechange {
            break;
        }
        b.step(&[]);
    }
    b.idle(20);
    assert!(b.sora.status().damage_invalid, "STYLECHANGE: pawn+0x630 bit 2");
    let (s0, s1, lost) = b.hit("hit in STYLECHANGE");
    assert_eq!((s0, s1, lost), (StateId::Stylechange, StateId::Stylechange, 0), "no HP lost, the transformation goes on");
    for _ in 0..400 {
        if b.state == StateId::Idle {
            break;
        }
        b.step(&[]);
    }
    assert!(!b.sora.status().damage_invalid, "STYLECHANGE End clears it");
    assert_eq!(b.sora.combat().form.cmd, 0x94);
    b.idle(5);
    b.press(PadButton::SITUATION);
    b.idle(10);
    assert_eq!(b.state, StateId::Stylefinish, "Stun Impact");
    assert!(b.sora.status().damage_invalid, "the finish's begin event");
    let (s0, s1, lost) = b.hit("hit in STYLEFINISH (Stun Impact)");
    assert_eq!((s0, s1, lost), (StateId::Stylefinish, StateId::Stylefinish, 0), "no HP lost, the finish goes on");
    for _ in 0..400 {
        if b.state != StateId::Stylefinish {
            break;
        }
        b.step(&[]);
    }
    assert!(!b.sora.status().damage_invalid, "the end event's vt+0x23B8 clears it");
    assert_eq!(b.sora.combat().form.cmd, 0, "the form ended with its finish");
    b.idle(60);
    let (_, s1, lost) = b.hit("the same hit after the form");
    assert!(lost > 0 && s1 == StateId::Damage, "out of the form the hit lands: {lost} {s1:?}");
    let dir = format!("{}/../../build/sora_tmp/weapons/AUDIT-A/logs", env!("CARGO_MANIFEST_DIR"));
    std::fs::create_dir_all(&dir).ok();
    std::fs::write(format!("{dir}/second_invincible.trace"), &b.trace).ok();
}
