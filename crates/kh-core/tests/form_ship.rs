//! (weapons F-SHIP) Wheel of Fate's formchanges, end to end on the public API (build/sora_tmp/weapons/F-SHIP/STATE.md):
//! Highwind (CHANGE_HL 0x8C) from the offer through StyleChange14, the COMBO_HL chain (E411 .. E414, the warp above the
//! target and the spear rain of E416), then the chain into Storm Flag (CHANGE_FL 0x8D, StyleChange15, COMBO_FL with the
//! flag throw), FINISH_FL (the Kraken) and the end of the form. Each test also writes its step log to
//! build/sora_tmp/weapons/F-SHIP/<name>.trace (for the record; not compared).

use kh_core::host::EventKind;
use kh_core::status::keyblade::KeybladeId;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};
use std::fmt::Write as _;

const DT: f32 = 1.0 / 60.0;
const WHEEL_OF_FATE: &str = "WEP_KEYBLADE_SO_09";

fn cell(json: &serde_json::Value) -> Option<LoadoutValue> {
    match json {
        serde_json::Value::Number(n) => n.as_f64().map(LoadoutValue::Number),
        serde_json::Value::String(s) => Some(LoadoutValue::Text(s.clone())),
        serde_json::Value::Array(items) => items.iter().map(|i| i.as_str().map(str::to_string)).collect::<Option<Vec<_>>>().map(LoadoutValue::List),
        serde_json::Value::Object(map) => map.iter().map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect::<Option<Vec<_>>>().map(LoadoutValue::Map),
        _ => None,
    }
}

fn holding(key: &str) -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let k = row[0].as_str().unwrap();
        let value = if k == "abilities.equipped" { LoadoutValue::List(Vec::new()) } else { cell(&row[1]).unwrap() };
        loadout.set(k, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    let mut s = Status::from_loadout(&loadout);
    s.weapon = key.to_string();
    s
}

fn dummy(id: u32, x: f32) -> TargetInfo {
    TargetInfo { id: TargetId(id), position: Vec3::new(x, 0.0, 0.0), center: Vec3::new(x, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 }
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    /// Every step: (state, sub-step, clip, the events).
    log: Vec<(StateId, u8, &'static str, Vec<EventKind>)>,
    trace: String,
    /// The last step's position and yaw.
    at: (Vec3, f32),
}

impl Bench {
    fn new(x: f32) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding(WHEEL_OF_FATE));
        let mut host = RecordingHost::new();
        host.targets.push(dummy(1, x));
        let mut b = Bench { sora, host, world: TestWorld::floor(), log: Vec::new(), trace: String::new(), at: (Vec3::ZERO, 0.0) };
        b.idle(5);
        b
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input::default().with_pad(PadState::from_held(set, PadButtons::NONE));
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        let c = self.sora.combat();
        let _ = write!(self.trace, "{} {:?}/{} p={:.1?} a={} form {} {:.2} k={:?} shots {}", self.log.len(), out.state, out.sub_step, out.position, out.anim.name.name(), c.form.cmd, c.form.time_left, self.sora.attack().map(|a| a.kind), c.shots.len());
        for e in &out.events {
            if let EventKind::Hit { record, .. } = e.kind {
                let _ = write!(self.trace, " | hit {} {:?}", record.attack_name, record.attribute);
            }
        }
        self.trace.push('\n');
        self.at = (out.position, out.yaw);
        self.log.push((out.state, out.sub_step, out.anim.name.name(), out.events.iter().map(|e| e.kind).collect()));
        out
    }

    /// The dummy `ahead` cm in front of him (the transformations move him by their root motion).
    fn place(&mut self, ahead: f32) {
        let (p, yaw) = self.at;
        let (sin, cos) = yaw.to_radians().sin_cos();
        let x = Vec3::new(p.x + cos * ahead, p.y + sin * ahead, 0.0);
        let t = &mut self.host.targets[0];
        (t.position, t.center) = (x, x + Vec3::Z * 90.0);
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    fn clips(&self, from: usize) -> Vec<&'static str> {
        let mut out: Vec<&'static str> = Vec::new();
        for (_, _, clip, _) in &self.log[from..] {
            if out.last() != Some(clip) {
                out.push(clip);
            }
        }
        out
    }

    /// The attack rows of the hits since `from`.
    fn hits(&self, from: usize) -> Vec<(&'static str, String)> {
        self.log[from..].iter().flat_map(|(_, _, _, e)| e.iter().filter_map(|k| if let EventKind::Hit { record, .. } = k { Some((record.attack_name, format!("{:?}", record.attribute))) } else { None })).collect()
    }

    /// The offer, Triangle, the transformation; returns the step the form began at.
    fn transform(&mut self, cmd: i32) -> usize {
        self.sora.debug_add_style(100.0);
        self.idle(15);
        let list: Vec<i32> = self.sora.combat().situation.list.iter().map(|e| e.kind).collect();
        assert_eq!(list, [cmd], "the formchange is offered");
        let from = self.log.len();
        self.step(&[PadButton::SITUATION]);
        assert_eq!(self.step(&[]).state, StateId::Stylechange);
        for _ in 0..600 {
            if self.step(&[]).state != StateId::Stylechange {
                return from;
            }
        }
        panic!("STYLECHANGE did not end");
    }

    /// Attack presses until the chain ends (back to IDLE / JUMP / RUN); the kinds played.
    fn combo(&mut self) -> Vec<u8> {
        let first = self.sora.combat().history.len();
        for k in 0..1200 {
            let out = if k % 2 == 0 { self.step(&[PadButton::ATTACK]) } else { self.step(&[]) };
            if k > 30 && matches!(out.state, StateId::Idle | StateId::Run | StateId::Jump) {
                break;
            }
        }
        self.sora.combat().history[first..].to_vec()
    }

    fn save(&self, name: &str) {
        let dir = format!("{}/../../build/sora_tmp/weapons/F-SHIP", env!("CARGO_MANIFEST_DIR"));
        if std::path::Path::new(&dir).is_dir() {
            let _ = std::fs::write(format!("{dir}/{name}.trace"), &self.trace);
        }
    }
}

#[test]
fn highwind_then_storm_flag_from_the_offer_to_the_end() {
    assert_eq!((kh_core::combat::forms::moveset_name("form.HL"), kh_core::combat::forms::moveset_name("form.FL")), ("ship", "ship"));
    let mut b = Bench::new(200.0);
    // ---- Highwind: offer -> StyleChange14 (its hits: SCHL_CHANGE / SCHL_CHANGE_END, water)
    let from = b.transform(0x8C);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.level, f.time_full, f.wear_form, f.grow), (0x8C, 1, 25.0, 1, 1), "Highwind: level 1, 25 s, wear form 1 (Brave)");
    assert!(b.clips(from).contains(&"StyleChange14"), "{:?}", b.clips(from));
    assert!(f.weapon_units()[0].ends_with("w_so092_Wep"), "the spear: {:?}", f.weapon_units());
    let hits = b.hits(from);
    assert!(hits.iter().any(|(r, a)| *r == "SCHL_CHANGE_END" && a.to_uppercase().contains("WATER")), "the ship's water wave: {hits:?}");
    // ---- the combo: COMBO_HL 0x9C 0x9D 0x9E 0x9F 0xA0 (the warp, the dive, the spear rain)
    b.idle(30);
    let from = b.log.len();
    let spawned = b.sora.combat().shots_spawned;
    let kinds = b.combo();
    assert_eq!(kinds, [0x9C, 0x9D, 0x9E, 0x9F, 0xA0], "the combo near one target");
    assert!(b.log[from..].iter().any(|l| l.0 == StateId::ComboHl), "the attack class is COMBO_HL");
    let clips = b.clips(from);
    for c in ["E411", "E412", "E413", "E414", "JumpNormalStart", "E416"] {
        assert!(clips.contains(&c), "{c} not played: {clips:?}");
    }
    assert_eq!(b.sora.combat().shots_spawned - spawned, 15 + 1, "15 spears (and the HL_Finish prism)");
    let hits = b.hits(from);
    for r in ["SCHL_STAB", "SCHL_SPIN", "SCHL_THRUST", "SCHL_FINISH"] {
        assert!(hits.iter().any(|(x, _)| *x == r), "{r} hit nothing: {hits:?}");
    }
    // (F-POLISH) m_bAttachToHitActor: a spear that hits an actor sticks in it and moves with it. The rain fell around the
    // dummy here (no spear met it): the dummy steps onto one that stands in the floor; it hits, sticks, and is carried.
    let stuck = |b: &Bench| -> Vec<Vec3> { b.sora.combat().shots.iter().filter(|s| s.name == "HL_SPEAR" && s.stick.is_some_and(|k| k.stuck())).map(|s| s.position).collect() };
    let spear = b.sora.combat().shots.iter().find(|s| s.name == "HL_SPEAR" && s.stick.is_some()).map(|s| s.centre()).expect("a spear in the floor, armed to stick");
    let home = (b.host.targets[0].position, b.host.targets[0].center);
    let t = &mut b.host.targets[0];
    (t.position, t.center) = (Vec3::new(spear.x, spear.y, 0.0), Vec3::new(spear.x, spear.y, 90.0));
    b.step(&[]);
    let before = stuck(&b);
    assert_eq!(before.len(), 1, "the spear stuck in the dummy");
    let t = &mut b.host.targets[0];
    (t.position, t.center) = (t.position + Vec3::Y * 300.0, t.center + Vec3::Y * 300.0);
    b.step(&[]);
    let after = stuck(&b);
    // (the dummy's move may sweep it onto another spear, which sticks too)
    assert!(after.first().is_some_and(|a| (*a - before[0] - Vec3::Y * 300.0).length() < 1.0e-2), "carried by the dummy: {before:?} -> {after:?}");
    (b.host.targets[0].position, b.host.targets[0].center) = home;
    // ---- the chain into Storm Flag: a full gauge in Highwind offers CHANGE_FL (after the command cool-down of the air
    // attack's end)
    b.idle(60);
    let from = b.transform(0x8D);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.level, f.wear_form), (0x8D, 2, 1), "Storm Flag: level 2");
    assert!(b.clips(from).contains(&"StyleChange15"));
    assert!(f.weapon_units()[0].ends_with("w_so093_Wep"), "the flag: {:?}", f.weapon_units());
    assert_eq!(b.sora.combat().situation.slot_b.map(|e| e.kind), Some(0xA6), "FINISH_FL on offer");
    assert!(b.hits(from).iter().any(|(r, a)| r.starts_with("SCFL_CHANGE") && a.to_uppercase().contains("AERO")), "the flag's wind");
    // ---- COMBO_FL near: 0xA3 0xA5 0xA6 0xA7 0xA8 (the whirlwind)
    b.idle(30);
    let from = b.log.len();
    let kinds = b.combo();
    assert_eq!(kinds, [0xA3, 0xA5, 0xA6, 0xA7, 0xA8]);
    assert!(b.log[from..].iter().any(|l| l.0 == StateId::ComboFl));
    let hits = b.hits(from);
    assert!(hits.iter().any(|(r, a)| r.starts_with("SCFL_FINISH") && a.to_uppercase().contains("AERO")), "the whirlwind: {hits:?}");
    // ---- FINISH_FL: the Kraken (its prisms hit), the landing, the end of the form
    b.idle(30);
    let from = b.log.len();
    b.step(&[PadButton::SITUATION]);
    let mut subs = Vec::new();
    for _ in 0..1200 {
        let out = b.step(&[]);
        match b.sora.finish() {
            Some(f) if subs.last() != Some(&f.sub) => subs.push(f.sub),
            Some(_) => {}
            None if out.state != StateId::Stylefinish => break,
            None => {}
        }
    }
    assert_eq!(subs, [2, 5, 6, 7, 8], "StyleFinishFL Tick's sub-steps");
    assert!(b.clips(from).contains(&"F420") && b.clips(from).contains(&"M_1501"), "{:?}", b.clips(from));
    let hits = b.hits(from);
    for r in ["SCFL_KRAKEN", "SCFL_KRAKEN2", "SCFL_KRAKEN_END"] {
        assert!(hits.iter().any(|(x, _)| *x == r), "{r}: {hits:?}");
    }
    assert!(hits.iter().filter(|(x, _)| *x == "SCFL_KRAKEN2").count() >= 4, "FinishFL2 every 0.6 s for 3 s");
    b.idle(5);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.wear_form), (0, 0), "the finish ends the form");
    b.save("highwind_storm_flag");
}

#[test]
fn highwind_doubles_the_soft_target_range() {
    // StartStyle case 0x8C: ctrl+0x970 = MyAutoLockonLimit (900) x 2. A target 1300 cm away is no soft target before the
    // form, one in Highwind; Storm Flag's StartStyle resets the range (its case writes none).
    let mut b = Bench::new(1300.0);
    b.idle(10);
    assert_eq!(b.sora.combat().lockon.current, TargetId::NONE, "out of the 900 cm soft range");
    b.transform(0x8C);
    b.place(1300.0);
    b.idle(10);
    assert_eq!(b.sora.combat().lockon.current, TargetId(1), "Highwind: 1800 cm");
    b.transform(0x8D);
    b.place(1300.0);
    b.idle(10);
    assert_eq!(b.sora.combat().lockon.current, TargetId::NONE, "Storm Flag: 900 cm again");
}

/// (F-POLISH) The slot resume 0x14141E2B0 case 0x8C: Highwind's soft-target range x 2 is back after a Keyblade switch away
/// (EndStyle resets it) and back to Wheel of Fate's slot.
#[test]
fn highwind_keeps_its_soft_target_range_across_a_keyblade_switch() {
    let mut b = Bench::new(1300.0);
    b.transform(0x8C);
    b.idle(30);
    // Wheel of Fate in slot 1 and the Kingdom Key in slot 2 (the equip menu's route: Status::set_keyblades).
    let mut slots = *b.sora.status().keyblades();
    slots.set_slot(0, KeybladeId::from_key(WHEEL_OF_FATE)).unwrap();
    slots.set_slot(1, Some(KeybladeId::KINGDOM_KEY)).unwrap();
    b.sora.status_mut().set_keyblades(slots);
    assert_eq!(b.sora.combat().lockon.soft_range, 1800.0, "Highwind: x 2");
    let switch = |b: &mut Bench, button: PadButton| {
        b.step(&[button]);
        b.idle(40);
    };
    switch(&mut b, PadButton::DPadRight);
    assert_eq!((b.sora.combat().form.cmd, b.sora.combat().lockon.soft_range), (0, 900.0), "the Kingdom Key in hand: no form, 900 cm");
    switch(&mut b, PadButton::DPadLeft);
    assert_eq!((b.sora.combat().form.cmd, b.sora.combat().lockon.soft_range), (0x8C, 1800.0), "Highwind back: 1800 cm again");
}

#[test]
fn storm_flag_throws_its_flag_at_a_far_target() {
    // The target 800 cm away (> 600: reach 1): COMBO_FL's first kind is the flag throw 0xA4.
    let mut b = Bench::new(800.0);
    b.transform(0x8C);
    b.idle(30);
    b.transform(0x8D);
    b.idle(30);
    b.place(800.0);
    b.idle(5);
    let from = b.log.len();
    b.step(&[PadButton::ATTACK]);
    b.idle(90);
    assert_eq!(b.sora.combat().history.last(), Some(&0xA4), "far: the flag throw");
    let clips = b.clips(from);
    let at = |c: &str| clips.iter().position(|x| *x == c);
    assert!(at("F412").is_some() && at("F412") < at("F413") && at("F413") < at("F414"), "M_F412 -> M_F413 -> M_F414: {clips:?}");
    b.save("storm_flag_throw");
}
