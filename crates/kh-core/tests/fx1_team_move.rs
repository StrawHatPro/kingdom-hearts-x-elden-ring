//! (FX1) AI Sora and a friend leader's team move (contract D.6 / FX1, docs/party-contracts.md):
//! - `Sora::can_start_leader_unison`: true idle on the ground, false mid-attack, in the air, with a command cool-down,
//!   with a latch already armed;
//! - an accepted unison is never replaced by his own XS finisher (Spiral Rush with a locked target in its band was
//!   what turned the latch's attack into his own move).

use kh_core::abilities::xs::{self, X};
use kh_core::enums::ETresAbilityKind as A;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};

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

/// Level 99 (sheet sora-loadout-99) with the five team abilities and Spiral Rush equipped.
fn level99() -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(std::fs::read_to_string(&path).unwrap().trim_start_matches('\u{feff}')).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        loadout.set(row[0].as_str().unwrap(), &cell(&row[1]).unwrap()).unwrap();
    }
    loadout.resolve().unwrap();
    let mut status = Status::from_loadout(&loadout);
    for kind in [A::UnisonFire, A::UnisonBlizzard, A::UnisonThunder, A::FusionSpin, A::FusionRocket, A::SpiralRush] {
        let owned = status.abilities.owned(kind).max(1);
        status.abilities.set(kind, owned, 1);
    }
    status
}

fn dummy(at: Vec3) -> TargetInfo {
    TargetInfo { id: TargetId(1), position: at, center: at + Vec3::Z * 90.0, velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    held: PadButtons,
}

impl Bench {
    fn new(target_at: Vec3) -> Bench {
        let origin = Vec3::Z * Sora::origin_height_on_floor(&SORA_PARAMS);
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(level99());
        let mut host = RecordingHost::new();
        host.targets.push(dummy(target_at));
        let mut b = Bench { sora, host, world: TestWorld::floor(), held: PadButtons::NONE };
        for _ in 0..5 {
            b.step(&[]);
        }
        b
    }

    fn step(&mut self, held: &[PadButton]) -> kh_core::Output {
        let now = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input::default().with_pad(PadState::from_held(now, self.held));
        self.held = now;
        let (sora, world) = (&mut self.sora, &self.world);
        self.host.step(world, |h| sora.step_hosted(DT, &input, h))
    }

    fn roll(&mut self, uid: u8) -> bool {
        let abilities = self.sora.status().abilities.clone();
        let at = self.sora.output().position;
        self.sora.combat_mut().friend.sora_is_ai = true;
        self.sora.combat_mut().friend.roll_leader_unison(&abilities, uid, at + Vec3::new(0.0, 150.0, 0.0), at)
    }
}

#[test]
fn fx1_can_start_leader_unison_only_when_free_on_the_ground() {
    let mut b = Bench::new(Vec3::new(5000.0, 0.0, 0.0));
    assert_eq!(b.sora.output().state, StateId::Idle);
    assert!(b.sora.can_start_leader_unison(11) && b.sora.can_start_leader_unison(12), "idle on the ground");
    // mid-attack
    b.step(&[PadButton::ATTACK]);
    b.step(&[]);
    b.step(&[]);
    assert_eq!(b.sora.output().state, StateId::Attack);
    assert!(!b.sora.can_start_leader_unison(12), "mid-attack");
    // back to idle, then a jump
    for _ in 0..240 {
        b.step(&[]);
    }
    assert!(b.sora.can_start_leader_unison(12), "idle again: {:?}", b.sora.output().state);
    b.step(&[PadButton::JUMP]);
    for _ in 0..6 {
        b.step(&[PadButton::JUMP]);
    }
    assert!(!b.sora.output().on_floor);
    assert!(!b.sora.can_start_leader_unison(11), "in the air: {:?}", b.sora.output().state);
    for _ in 0..240 {
        b.step(&[]);
    }
    assert!(b.sora.can_start_leader_unison(11));
    // a command cool-down
    b.sora.combat_mut().command_cooldown = 0.6;
    assert!(!b.sora.can_start_leader_unison(11), "cool-down");
    b.sora.combat_mut().command_cooldown = 0.0;
    // a latch already armed
    assert!(b.roll(11));
    assert!(!b.sora.can_start_leader_unison(11), "the latch is armed");
}

#[test]
fn fx1_an_accepted_fusion_is_not_replaced_by_spiral_rush() {
    // the locked dummy in Spiral Rush's band: the plain finisher there would be his own XS move
    let mid = (X.spiral_range_min + X.spiral_range_max) / 2.0;
    let mut b = Bench::new(Vec3::new(mid, 0.0, 0.0));
    {
        let lock = &mut b.sora.combat_mut().lockon;
        lock.locked = true;
        lock.current = TargetId(1);
    }
    assert!(b.roll(12), "Goofy 150 cm away, five team abilities, chance 100");
    for _ in 0..30 {
        b.step(&[]);
    }
    let history = b.sora.combat().history.clone();
    assert!(history.iter().any(|k| (0x42..=0x43).contains(k)), "the fusion played: {history:x?}");
    assert!(!history.contains(&xs::finishers::SPIRAL_RUSH), "not his own XS move: {history:x?}");
    assert_eq!(b.sora.combat().friend.unisons, 1);
}
