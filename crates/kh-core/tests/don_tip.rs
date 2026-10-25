//! (DON-MAGIC tip) The user, 2026-10-22: "Sora's spells need to come from the tip of his keyblade". Every spell's fire
//! position (its cast flash and its actor's fire effects, the seam's `FIRE_EFFECTS_ID` spawn) is the Keyblade's tip
//! (socket ATK_01 of the playing clip) at the shot frame, and a bolt leaves from there; a raised blade (Thunder, Aero) is
//! no longer held below actor Z + 163 for its flash.

use kh_core::command::CommandRequest;
use kh_core::host::{Attach, Call};
use kh_core::magic::projectile::FIRE_EFFECTS_ID;
use kh_core::magic::spells::cmd;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Pose, RecordingHost, Sora, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};

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
        let key = row[0].as_str().unwrap();
        let value = if key == "abilities.equipped" { LoadoutValue::List(Vec::new()) } else { cell(&row[1]).unwrap() };
        loadout.set(key, &value).unwrap();
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
}

fn dummy(id: u32, x: f32, y: f32) -> TargetInfo {
    TargetInfo { id: TargetId(id), position: Vec3::new(x, y, 0.0), center: Vec3::new(x, y, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
}

/// Casts `kind` at a dummy 800 cm ahead; the fire position of the shot and the Keyblade's tip at that frame.
fn shot_and_tip(kind: u8) -> (Vec3, Vec3) {
    let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
    let mut sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(level99());
    let mut host = RecordingHost::new();
    host.targets.push(dummy(1, 800.0, 0.0));
    let world = TestWorld::floor();
    let mut step = |sora: &mut Sora, host: &mut RecordingHost| {
        let input = Input::default();
        host.step(&world, |h| sora.step_hosted(DT, &input, h));
    };
    for _ in 0..5 {
        step(&mut sora, &mut host);
    }
    sora.status_mut().set_mp(300);
    sora.command_request(CommandRequest::Execute(kind));
    for _ in 0..120 {
        let before = host.calls.len();
        // the tip of the frame the shot is taken in: the clip's time this step (the shot reads it before the clip moves on)
        let tip_before = sora.keyblade_tip();
        step(&mut sora, &mut host);
        let tip_after = sora.keyblade_tip();
        for c in &host.calls[before..] {
            if let Call::Spawn(_, s) = c {
                if s.id.0 == FIRE_EFFECTS_ID {
                    let Attach::World(at) = s.at else { panic!("the fire effects at a world point") };
                    let tip = [tip_before, tip_after].into_iter().flatten().min_by(|a, b| (*a - at).length().total_cmp(&(*b - at).length())).expect("the clip has a tip track");
                    return (at, tip);
                }
            }
        }
    }
    panic!("command {kind}: no shot");
}

#[test]
fn every_spells_fire_position_is_the_keyblade_tip_at_the_shot() {
    for (kind, name) in [(cmd::FIRE, "Fire"), (cmd::FIRE + 2, "Firaga"), (cmd::BLIZZARD, "Blizzard"), (cmd::THUNDER, "Thunder"), (cmd::THUNDER + 2, "Thundaga"), (cmd::WATER, "Water"), (cmd::AERO, "Aero")] {
        let (at, tip) = shot_and_tip(kind);
        eprintln!("{name}: fired at {at:?}, tip {tip:?}");
        assert!((at - tip).length() < 12.0, "{name}: fired at {at:?}, the Keyblade's tip at {tip:?}");
    }
}

#[test]
fn a_raised_blades_flash_is_not_held_below_163() {
    // Thunder's clip raises the blade: its tip is above actor Z + 163 at the shot, and the flash is there
    let (at, tip) = shot_and_tip(cmd::THUNDER);
    let foot = Sora::origin_height_on_floor(&SORA_PARAMS);
    if tip.z > foot + 163.0 + 5.0 {
        assert!(at.z > foot + 163.0 + 5.0, "the flash is at the raised tip ({at:?}), not held at 163");
    }
}
