//! (SHOT-C) Infinity Circle (kind 29, Ultima Weapon): the trace lock-on -> fire -> M_9481 with its SF_Range window ->
//! ShootFlowEnd -> back to normal (magic/shotlocks/ultima.rs, update 0x1413BB3C0).

use kh_core::host::HitRecord;
use kh_core::magic::View;
use kh_core::status::{Keyblades, LoadoutValue};
use kh_core::test_world::TestWorld;
use kh_core::{AnimId, Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};

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

/// The level-99 loadout (no abilities equipped) holding Ultima Weapon.
fn level99_ultima() -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let key = row[0].as_str().unwrap();
        let value = if key == "abilities.equipped" { LoadoutValue::List(Vec::new()) } else { cell(&row[1]).unwrap() };
        loadout.set(key, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    let mut status = Status::from_loadout(&loadout);
    let (k, _) = Keyblades::from_keys(&[Some("WEP_KEYBLADE_SO_14"), Some("WEP_KEYBLADE_SO_00"), None], 0, &[]);
    status.set_keyblades(k);
    status
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
    fn new(status: Status, dummies: &[TargetInfo]) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(status);
        let mut host = RecordingHost::new();
        host.targets.extend_from_slice(dummies);
        let mut bench = Bench { sora, host, world: TestWorld::floor(), held: PadButtons::NONE, view: Some(View::from_camera(Vec3::new(-300.0, 0.0, 150.0), 0.0, 0.0, 90.0, 1280, 720)) };
        bench.run(5);
        bench
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

    fn run(&mut self, steps: usize) {
        for _ in 0..steps {
            self.step(&[]);
        }
    }

    fn hits(&self) -> Vec<HitRecord> {
        self.host.hits().into_iter().map(|h| h.0).collect()
    }
}

fn seconds(steps: usize) -> f32 {
    steps as f32 * DT
}

#[test]
fn infinity_circle() {
    // 1 the lock target ahead; 2 an unlocked one 25 m away (inside the 30 m sphere); 3 one 35 m away (outside).
    let mut b = Bench::new(level99_ultima(), &[dummy(1, 800.0, 0.0), dummy(2, 2500.0, 0.0), dummy(3, 3500.0, 0.0)]);
    // LOCK-ON: Param0 1 -> one lock and MAX at once; weapon type 15 has no MAX kind: Infinity Circle stays.
    b.step(&[PadButton::RightShoulder]);
    for _ in 0..80 {
        b.step(&[PadButton::RightShoulder]);
    }
    let aim = b.sora.aim();
    assert_eq!((aim.mode, aim.kind, aim.total, aim.max_locks, aim.is_max), (1, 29, 1, 1, true), "{aim:?}");
    assert_eq!(aim.locks, vec![(TargetId(1), 1)]);
    let focus = b.sora.status().focus;
    // FIRE.
    b.step(&[PadButton::RightShoulder, PadButton::ATTACK]);
    assert!(b.sora.status().focus < focus, "focus is spent by the aim time");
    let mut out = b.step(&[]);
    let mut k = 1;
    while out.state != StateId::Shootflow {
        out = b.step(&[]);
        k += 1;
        assert!(k < 5, "no ShootFlow");
    }
    // Begin + sub 0: weapon form 9, M_9481, flying, invincible.
    out = b.step(&[]);
    assert_eq!(out.anim.name, AnimId::A9481);
    assert_eq!(b.sora.magic().shot_weapon_form, 9, "Ultimate Form's weapon on");
    assert!(b.sora.status().damage_invalid);
    let (mut first_hit, mut end_clip, mut weapon_back, mut aim_closed, mut ended) = (0, 0, 0, 0, 0);
    let mut hit_times: Vec<(u32, usize)> = Vec::new();
    let mut seen = 0;
    for n in 2..600 {
        let out = b.step(&[]);
        let hits = b.hits();
        for h in &hits[seen..] {
            assert_eq!(h.attack_name, "SF_ULTIMA", "{h:?}");
            hit_times.push((h.target.0, n));
        }
        seen = hits.len();
        if first_hit == 0 && !hits.is_empty() {
            first_hit = n;
        }
        if end_clip == 0 && out.anim.name == AnimId::ShootFlowEnd {
            end_clip = n;
        }
        if weapon_back == 0 && end_clip > 0 && b.sora.magic().shot_weapon_form == 0 {
            weapon_back = n;
        }
        if aim_closed == 0 && b.sora.aim().mode == 0 {
            aim_closed = n;
        }
        if out.state != StateId::Shootflow {
            ended = n;
            break;
        }
        assert!(b.sora.status().damage_invalid, "invincible throughout");
    }
    // The window opens 3.7 s into M_9481 (its interval 0.2667 s until 5.0333 s).
    assert!((seconds(first_hit) - 3.7).abs() < 0.05, "first hit at {}", seconds(first_hit));
    let on = |id: u32| hit_times.iter().filter(|h| h.0 == id).map(|h| h.1).collect::<Vec<_>>();
    let (t1, t2) = (on(1), on(2));
    assert!((5..=6).contains(&t1.len()), "locked target: {} hits", t1.len());
    assert_eq!(t1.len(), t2.len(), "the 30 m sphere hits the unlocked target alike");
    assert!(on(3).is_empty(), "35 m away: outside the sphere");
    for w in t1.windows(2) {
        assert!((seconds(w[1] - w[0]) - 0.2667).abs() < 0.04, "interval {}", seconds(w[1] - w[0]));
    }
    // M_9481 (5.1667 s) -> ShootFlowEnd; 0.3 s into it the weapon comes back (and WorldSlow off); its end finishes.
    assert!((seconds(end_clip) - 5.1667).abs() < 0.05, "ShootFlowEnd at {}", seconds(end_clip));
    assert!((seconds(weapon_back - end_clip) - 0.3).abs() < 0.05, "weapon back {} s into ShootFlowEnd", seconds(weapon_back - end_clip));
    assert!((seconds(ended) - (5.1667 + 0.8)).abs() < 0.08, "ended at {}", seconds(ended));
    // (SHOT-FIX) 0x14140E500 at the weapon's return is WorldSlow, not the release: the aim mode closes with the state
    // (End's slot 809), not 0.3 s into ShootFlowEnd.
    let mut n = ended;
    while aim_closed == 0 && n < ended + 10 {
        b.step(&[]);
        n += 1;
        if b.sora.aim().mode == 0 {
            aim_closed = n;
        }
    }
    assert!(aim_closed >= ended && aim_closed <= ended + 3, "aim closed at {aim_closed}, the state ended at {ended}, weapon back at {weapon_back}");
    // BACK TO NORMAL.
    b.run(1);
    assert_eq!((b.sora.aim().mode, b.sora.magic().shot_weapon_form), (0, 0));
    assert!(!b.sora.status().damage_invalid);
    assert_eq!((b.sora.time_dilation(1), b.sora.time_dilation(2)), (1.0, 1.0));
}

/// A state dropped in the middle (a teleport) gives the weapon back (End 0x1413ABA70).
#[test]
fn a_dropped_infinity_circle_gives_the_weapon_back() {
    let mut b = Bench::new(level99_ultima(), &[dummy(1, 800.0, 0.0)]);
    for _ in 0..81 {
        b.step(&[PadButton::RightShoulder]);
    }
    b.step(&[PadButton::RightShoulder, PadButton::ATTACK]);
    b.run(30);
    assert_eq!(b.sora.magic().shot_weapon_form, 9);
    b.sora.teleport(Pose { position: Vec3::new(0.0, 2000.0, Sora::origin_height_on_floor(&SORA_PARAMS)), yaw: 0.0 });
    b.run(3);
    assert_eq!((b.sora.aim().mode, b.sora.magic().shot_weapon_form), (0, 0));
}
