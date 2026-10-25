//! (DON-KIT) PORT ADDITION: Donald's Aero aura on Sora (`Sora::set_aura`, documentation/DONALD_OVERHAUL.md 4.2 "Party
//! evasion and recovery speed up"). At 0 / 0 (no aura) KH3's behaviour is unchanged, frame for frame; with the aura the
//! Dodge Roll's / Air Slide's invincible window is x (1 + evasion) and the ground hit reaction runs x (1 + recovery).

use kh_core::enums::ETresDamageKind as Kind;
use kh_core::host::EventKind;
use kh_core::states::HitOnPlayer;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, PadButton, PadButtons, PadState, Pose, Sora, StateId, Status, TargetId, Tuning, Vec2, Vec3, SORA_PARAMS};

const DT: f32 = 1.0 / 60.0;

fn level99() -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let v = match &row[1] {
            serde_json::Value::Number(n) => LoadoutValue::Number(n.as_f64().unwrap()),
            serde_json::Value::String(s) => LoadoutValue::Text(s.clone()),
            serde_json::Value::Array(items) => LoadoutValue::List(items.iter().filter_map(|i| i.as_str().map(str::to_string)).collect()),
            serde_json::Value::Object(map) => LoadoutValue::Map(map.iter().filter_map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect()),
            _ => continue,
        };
        loadout.set(row[0].as_str().unwrap(), &v).unwrap();
    }
    loadout.resolve().unwrap();
    Status::from_loadout(&loadout)
}

fn sora(aura: Option<(f32, f32)>) -> Sora {
    let origin = Vec3::Z * Sora::origin_height_on_floor(&SORA_PARAMS);
    let mut s = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(level99());
    if let Some((e, r)) = aura {
        s.set_aura(e, r);
    }
    s
}

fn press(b: PadButton, stick: Vec2) -> Input {
    let p = PadButtons::NONE.with(b);
    Input { move_stick: stick, ..Input::default() }.with_pad(PadState { held: p, pressed: p, released: PadButtons::NONE })
}

fn hit_at(s: &Sora, kind: Kind) -> HitOnPlayer {
    let own = s.output().position;
    HitOnPlayer { direction: Vec3::new(-1.0, 0.0, 0.0), causer_yaw: 180.0, ..HitOnPlayer::new(kind, 10.0, TargetId(1), own + Vec3::new(150.0, 0.0, 0.0)) }
}

/// The first frame after the roll's press at which a hit lands (is not dodged).
fn first_hit_frame_after_roll(aura: Option<(f32, f32)>) -> usize {
    let world = TestWorld::floor();
    for f in 1..80 {
        let mut s = sora(aura);
        for _ in 0..5 {
            s.step(DT, &Input::default(), &world);
        }
        s.step(DT, &press(PadButton::GUARD, Vec2::new(0.0, 1.0)), &world);
        for _ in 1..f {
            s.step(DT, &Input::default(), &world);
        }
        let h = hit_at(&s, Kind::Small);
        s.take_hit(h);
        let out = s.step(DT, &Input::default(), &world);
        if !out.events.iter().any(|e| matches!(e.kind, EventKind::PlayerDodged { .. })) {
            return f;
        }
    }
    999
}

#[test]
fn no_aura_is_kh3_frame_for_frame() {
    // a roll, hits during and after it, a heavy hit with its get-up, attacks: the outputs with set_aura(0, 0) are the
    // outputs of a Sora that never had it
    let world = TestWorld::floor();
    let (mut a, mut b) = (sora(None), sora(Some((0.0, 0.0))));
    for i in 0..400 {
        let input = match i {
            10 => press(PadButton::GUARD, Vec2::new(0.0, 1.0)),
            100 | 108 | 116 | 124 => press(PadButton::ATTACK, Vec2::ZERO),
            _ => Input::default(),
        };
        if matches!(i, 20 | 40 | 200 | 260) {
            let kind = if i == 200 { Kind::Blow } else { Kind::Small };
            let (ha, hb) = (hit_at(&a, kind), hit_at(&b, kind));
            a.take_hit(ha);
            b.take_hit(hb);
        }
        let (oa, ob) = (a.step(DT, &input, &world), b.step(DT, &input, &world));
        assert_eq!(format!("{oa:?}"), format!("{ob:?}"), "frame {i}");
    }
    assert_eq!(b.aura(), (0.0, 0.0));
}

#[test]
fn the_aero_aura_lengthens_the_rolls_invincible_window() {
    let plain = first_hit_frame_after_roll(None);
    let aura = first_hit_frame_after_roll(Some((0.15, 0.0)));
    let want = 0.4659 * 0.15 / DT;
    eprintln!("roll: the first landed hit at frame {plain} (KH3) / {aura} (Aero aura 0.15)");
    assert!(((aura - plain) as f32 - want).abs() <= 1.5, "{plain} -> {aura} (+{want:.1} frames)");
}

#[test]
fn the_aero_aura_speeds_the_ground_hit_reaction() {
    let world = TestWorld::floor();
    let frames = |aura: Option<(f32, f32)>, kind: Kind| {
        let mut s = sora(aura);
        for _ in 0..5 {
            s.step(DT, &Input::default(), &world);
        }
        let h = hit_at(&s, kind);
        s.take_hit(h);
        let mut n = 0;
        for _ in 0..600 {
            let out = s.step(DT, &Input::default(), &world);
            if out.state != StateId::Damage && n > 0 {
                break;
            }
            if out.state == StateId::Damage {
                n += 1;
            }
        }
        n
    };
    for kind in [Kind::Small, Kind::Blow] {
        let (plain, fast) = (frames(None, kind), frames(Some((0.0, 0.2)), kind));
        eprintln!("{kind:?}: {plain} frames in Damage (KH3) / {fast} with the Aero aura's recovery 0.2");
        assert!(fast < plain, "{kind:?}");
        if kind == Kind::Small {
            // the whole flinch is on the ground: x 1 / 1.2
            assert!(((plain as f32 / 1.2) - fast as f32).abs() <= 2.0, "{plain} / 1.2 vs {fast}");
        }
    }
}

#[test]
fn set_aura_is_clamped() {
    let mut s = sora(None);
    s.set_aura(f32::NAN, -1.0);
    assert_eq!(s.aura(), (0.0, 0.0));
    s.set_aura(9.0, 0.2);
    assert_eq!(s.aura(), (2.0, 0.2));
}

#[test]
fn the_aero_aura_lengthens_the_air_slides_window() {
    let world = TestWorld::floor();
    // a hit `k` frames after the Air Slide ended: dodged only in the aura's added time
    let dodged_after = |aura: Option<(f32, f32)>, k: usize| {
        let mut s = sora(aura);
        for _ in 0..5 {
            s.step(DT, &Input::default(), &world);
        }
        let mut slid = false;
        for i in 0..120 {
            let input = match i {
                0 => Input { jump_pressed: true, jump_held: true, ..Input::default() },
                1..=8 => Input { jump_held: true, ..Input::default() },
                14 => press(PadButton::GUARD, Vec2::new(0.0, 1.0)),
                _ => Input { move_stick: Vec2::new(0.0, 1.0), ..Input::default() },
            };
            let out = s.step(DT, &input, &world);
            if out.state == StateId::AirSlide {
                slid = true;
            } else if slid {
                break;
            }
        }
        assert!(slid, "the Air Slide ran");
        for _ in 1..k {
            s.step(DT, &Input::default(), &world);
        }
        let h = hit_at(&s, Kind::Small);
        s.take_hit(h);
        s.step(DT, &Input::default(), &world).events.iter().any(|e| matches!(e.kind, EventKind::PlayerDodged { .. }))
    };
    assert!(!dodged_after(None, 1), "KH3: the window ends with the slide");
    assert!(dodged_after(Some((0.15, 0.0)), 1), "the aura's added time");
    assert!(!dodged_after(Some((0.15, 0.0)), 30), "and then it ends");
}
