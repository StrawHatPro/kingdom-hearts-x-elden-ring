//! (vfx implementer 2) What the core hands the effect seam beyond a name: the rotation of a hit
//! effect from the hit normal (docs/sora-vfx-spec.md 2.6, `rot = Rotation(hit.Normal)`).

use kh_core::combat::hit::normal_rotation;
use kh_core::Vec3;

/// The rotator whose X axis is the normal, as (pitch, yaw, roll) degrees.
#[test]
fn a_hit_effect_is_turned_to_the_hit_normal() {
    let close = |a: Vec3, b: Vec3| (a - b).abs().max_element() < 1.0e-3;
    assert!(close(normal_rotation(Vec3::X), Vec3::ZERO));
    assert!(close(normal_rotation(Vec3::Y), Vec3::new(0.0, 90.0, 0.0)));
    assert!(close(normal_rotation(Vec3::new(-1.0, 0.0, 0.0)), Vec3::new(0.0, 180.0, 0.0)));
    assert!(close(normal_rotation(Vec3::Z), Vec3::new(90.0, 0.0, 0.0)));
    assert!(close(normal_rotation(Vec3::new(1.0, 1.0, 0.0)), Vec3::new(0.0, 45.0, 0.0)));
    let up_back = normal_rotation(Vec3::new(-1.0, 0.0, 1.0));
    assert!(close(up_back, Vec3::new(45.0, 180.0, 0.0)), "{up_back:?}");
    // No normal: no rotation.
    assert_eq!(normal_rotation(Vec3::ZERO), Vec3::ZERO);
}

// ---------------------------------------------------------------------------------------------
// (vfx implementer 3) The EndEffect calls at the end of a finish command.

mod finish_effects {
    use kh_core::host::EventKind;
    use kh_core::status::LoadoutValue;
    use kh_core::test_world::TestWorld;
    use kh_core::{Input, Loadout, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};

    const DT: f32 = 1.0 / 60.0;

    fn level99() -> Status {
        let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let mut loadout = Loadout::new("sora-loadout-99");
        for row in json["rows"].as_array().unwrap() {
            let key = row[0].as_str().unwrap();
            let value = match (key, &row[1]) {
                // No equipped abilities: the plain rules.
                ("abilities.equipped", _) => LoadoutValue::List(Vec::new()),
                (_, serde_json::Value::Number(n)) => LoadoutValue::Number(n.as_f64().unwrap()),
                (_, serde_json::Value::String(s)) => LoadoutValue::Text(s.clone()),
                (_, serde_json::Value::Array(items)) => LoadoutValue::List(items.iter().map(|i| i.as_str().unwrap().to_string()).collect()),
                (_, serde_json::Value::Object(map)) => LoadoutValue::Map(map.iter().map(|(k, v)| (k.clone(), v.as_f64().unwrap())).collect()),
                (key, other) => panic!("{key}: {other}"),
            };
            loadout.set(key, &value).unwrap();
        }
        loadout.resolve().unwrap();
        Status::from_loadout(&loadout)
    }

    /// Second Form's charge effect (`vs_com_lv2` on the Keyblade) is spawned with the form and is
    /// stopped by the finish command's end: `(pawn+0x598)+0x200 .vt+0x1A8(-1, 1)`.
    #[test]
    fn the_charge_effect_ends_with_the_finish_command() {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let mut sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(level99());
        let mut host = RecordingHost::new();
        let (x, z) = (300.0, 0.0);
        host.targets.push(TargetInfo { id: TargetId(1), position: Vec3::new(x, 0.0, z), center: Vec3::new(x, 0.0, z + 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE), hp: 100000.0, hp_max: 100000.0 });
        let world = TestWorld::floor();
        let mut events: Vec<EventKind> = Vec::new();
        let step = |sora: &mut Sora, host: &mut RecordingHost, events: &mut Vec<EventKind>, button: Option<PadButton>| {
            let held = button.map_or(PadButtons::NONE, |b| PadButtons::NONE.with(b));
            let input = Input::default().with_pad(PadState::from_held(held, PadButtons::NONE));
            let out = host.step(&world, |h| sora.step_hosted(DT, &input, h));
            events.extend(out.events.iter().map(|e| e.kind.clone()));
            out.state
        };
        for _ in 0..5 {
            step(&mut sora, &mut host, &mut events, None);
        }
        // A full gauge, the formchange, then the finish command.
        sora.debug_add_style(100.0);
        for _ in 0..15 {
            step(&mut sora, &mut host, &mut events, None);
        }
        step(&mut sora, &mut host, &mut events, Some(PadButton::SITUATION));
        for _ in 0..220 {
            step(&mut sora, &mut host, &mut events, None);
        }
        let charge = events.iter().find_map(|e| if let EventKind::EffectSpawn { spawn, handle } = e { (spawn.name == "vs_com_lv2").then_some(*handle) } else { None }).expect("the form's charge effect");
        let stopped = |events: &[EventKind]| events.iter().any(|e| matches!(e, EventKind::EffectStop { handle, .. } if *handle == charge));
        assert!(!stopped(&events), "the charge effect lasts while the form does");
        step(&mut sora, &mut host, &mut events, Some(PadButton::SITUATION));
        let mut state = step(&mut sora, &mut host, &mut events, None);
        assert_eq!(state, StateId::Stylefinish);
        for _ in 0..1500 {
            if state != StateId::Stylefinish {
                break;
            }
            state = step(&mut sora, &mut host, &mut events, None);
        }
        assert_ne!(state, StateId::Stylefinish, "the finish command ended");
        assert!(stopped(&events), "EndEffect on the weapons at the finish end");
    }
}
