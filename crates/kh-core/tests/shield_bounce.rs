//! (THE USER 2026-11-02: "fix 'Shields don't count as guarding.' and in fact make the character who attacks a shield bounce
//! off playing their animation from KH3. This means Sora, Donald, Goofy and whoever we add in the future") Sora's side: a
//! host's raised shield (`TargetBody::shield`) guards a swing from its front with KH3's guard effect and repels him into
//! KH3's own recoil (`TresPlayerState_Reflect`, `AttackReflect`), every swing of a combo; from behind it is a plain hit; a
//! body that does not guard is hit as before. (The friends' side: crates/kh-party/tests/shield_bounce.rs.)

use kh_core::host::{EventKind, ReactionResult, Shield, TargetBody};
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

/// The level-99 loadout sheet with `equipped` as its equipped abilities.
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

/// A dummy 200 cm in front of Sora (he faces +X), turned toward him (yaw 180).
fn dummy() -> TargetInfo {
    TargetInfo { id: TargetId(1), position: Vec3::new(200.0, 0.0, 0.0), center: Vec3::new(200.0, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
}

/// A raised shield facing `yaw` (90 degrees to either side).
fn shield(yaw: i16) -> TargetBody {
    TargetBody { guarding: true, shield: Some(Shield { yaw, half_arc: 90 }), ..TargetBody::DEFAULT }
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    outs: Vec<Output>,
}

impl Bench {
    fn new(equipped: &[&str]) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(level99(equipped));
        let mut host = RecordingHost::new();
        host.targets.push(dummy());
        let mut bench = Bench { sora, host, world: TestWorld::floor(), outs: Vec::new() };
        for _ in 0..5 {
            bench.step(false);
        }
        bench
    }

    fn step(&mut self, attack: bool) -> Output {
        let held = if attack { PadButtons::NONE.with(PadButton::ATTACK) } else { PadButtons::NONE };
        let input = Input::default().with_pad(PadState::from_held(held, PadButtons::NONE));
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        self.outs.push(out.clone());
        out
    }

    fn effects(&self) -> Vec<&'static str> {
        self.outs.iter().flat_map(|o| o.events.iter()).filter_map(|e| if let EventKind::EffectSpawn { spawn, .. } = &e.kind { Some(spawn.name) } else { None }).collect()
    }

    fn reflected(&self) -> bool {
        self.outs.iter().any(|o| o.state == StateId::Reflect && o.anim.name.name() == "AttackReflect")
    }
}

#[test]
fn a_shield_from_the_front_guards_sora_and_he_bounces_off_with_kh3s_recoil() {
    let mut bench = Bench::new(&[]);
    bench.host.bodies.push((TargetId(1), shield(180)));
    bench.step(true);
    for _ in 0..80 {
        bench.step(false);
    }
    let hits = bench.host.hits();
    assert_eq!(hits.len(), 1);
    let (record, result) = hits[0];
    assert_eq!((record.info.victim_result, record.info.attacker_result), (ReactionResult::Guard, ReactionResult::Reflect));
    assert!(result.guarded && result.damage_dealt == 0.0, "{result:?}");
    // KH3's guard spark (it carries se00001_037), not the chooser's hit effect; KH3's recoil clip
    assert!(bench.effects().contains(&"vs_com_haj00") && !bench.effects().contains(&"vw_so010_hit0"), "{:?}", bench.effects());
    assert!(bench.reflected(), "Sora bounced off into Reflect / AttackReflect");
    assert!(!bench.sora.combat().hit_flag, "a repelled swing is not a landed hit");
}

#[test]
fn a_shield_raised_mid_combo_bounces_the_next_swing_too() {
    // KH3's own guard repels only a combo's first swing (combat spec 8.2.6 last paragraph, tests/combat2.rs); a host's
    // shield repels the swing it meets wherever it is in the combo, and the recoil ends the combo
    let mut bench = Bench::new(&["COMBO_MASTER"]);
    for _ in 0..400 {
        if bench.host.hits().len() == 1 && bench.host.bodies.is_empty() {
            bench.host.bodies.push((TargetId(1), shield(180)));
        }
        let out = bench.step(true);
        if bench.reflected() && out.state != StateId::Reflect {
            break;
        }
    }
    let results: Vec<_> = bench.host.hits().iter().map(|(r, _)| (r.info.victim_result, r.info.attacker_result)).collect();
    assert_eq!(results[0], (ReactionResult::Damage, ReactionResult::NoReaction));
    assert_eq!(results[1], (ReactionResult::Guard, ReactionResult::Reflect), "{results:?}");
    assert_eq!(results.len(), 2, "the bounce interrupts the combo: {results:?}");
    assert!(bench.reflected());
}

#[test]
fn a_shield_turned_away_is_a_plain_hit_and_an_open_body_is_unchanged() {
    // the shield faces away from Sora (yaw 0: he strikes its back)
    let mut bench = Bench::new(&[]);
    bench.host.bodies.push((TargetId(1), shield(0)));
    bench.step(true);
    for _ in 0..60 {
        bench.step(false);
    }
    let (record, result) = bench.host.hits()[0];
    assert_eq!((record.info.victim_result, record.info.attacker_result), (ReactionResult::Damage, ReactionResult::NoReaction));
    assert!(result.damage_dealt > 0.0 && !result.guarded);
    assert!(!bench.reflected() && !bench.effects().contains(&"vs_com_haj00"));
    // a body that does not guard: the same plain hit
    let mut open = Bench::new(&[]);
    open.step(true);
    for _ in 0..60 {
        open.step(false);
    }
    let (r2, x2) = open.host.hits()[0];
    assert_eq!((r2.info.victim_result, r2.info.attacker_result, x2.damage_dealt), (record.info.victim_result, record.info.attacker_result, result.damage_dealt));
    assert!(!open.reflected());
}
