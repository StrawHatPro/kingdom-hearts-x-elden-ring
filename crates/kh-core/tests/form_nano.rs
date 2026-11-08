//! (weapons F-NANO) Nano Arms (Nano Gear, CHANGE_GM 0x86): the trace of the form from the offer to its end
//! (build/sora_tmp/weapons/F-NANO/STATE.md): offer -> STYLECHANGE (StyleChange8, wear form 4 Grace, w_so172) -> the
//! COMBO_GM combo (the blade 830x, then the crash 0xB5, the giant stamp 0xB7, the Rocket Fist 0xB9 and its flight) -> the
//! far hookshot 0xBC -> 0xBD (the pull) -> the Nano Counter 0x25 out of the guard -> FINISH_GM (the gate, Sora hidden,
//! SCGM_PORTAL / SCGM_PORTAL2) -> the form's end.

use kh_core::enums::ETresDamageKind;
use kh_core::host::EventKind;
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

fn holding(key: &str) -> Status {
    let path = format!("{}/../../sheets/port/sora-loadout-99.json", env!("CARGO_MANIFEST_DIR"));
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap();
    let mut loadout = Loadout::new("sora-loadout-99");
    for row in json["rows"].as_array().unwrap() {
        let k = row[0].as_str().unwrap();
        // Only REFLECT_GUARD and GUARD_COUNTER (the guard and its counter window need them): no combo ability changes the decision
        let value = if k == "abilities.equipped" { LoadoutValue::List(vec!["REFLECT_GUARD".to_string(), "GUARD_COUNTER".to_string()]) } else { cell(&row[1]).unwrap() };
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
}

impl Bench {
    fn new(x: f32) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding("WEP_KEYBLADE_SO_08"));
        let mut host = RecordingHost::new();
        host.targets.push(dummy(1, x));
        let mut b = Bench { sora, host, world: TestWorld::floor(), log: Vec::new() };
        b.idle(5);
        b
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input::default().with_pad(PadState::from_held(set, PadButtons::NONE));
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        self.log.push((out.state, out.sub_step, out.anim.name.name(), out.events.iter().map(|e| e.kind).collect()));
        out
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

    /// The attack rows that hit since `from`.
    fn hits(&self, from: usize) -> Vec<&'static str> {
        self.log[from..].iter().flat_map(|(_, _, _, e)| e.iter().filter_map(|k| if let EventKind::Hit { record, .. } = k { Some(record.attack_name) } else { None })).collect()
    }

    fn transform(&mut self) -> usize {
        self.sora.debug_add_style(100.0);
        self.idle(15);
        let list: Vec<(i32, f32)> = self.sora.combat().situation.list.iter().map(|e| (e.kind, e.total)).collect();
        assert_eq!(list, [(0x86, 30.0)], "Nano Arms is offered for its 30 s");
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

    /// Attack presses until the chain ends (back to IDLE / RUN / JUMP); the kinds played.
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

    fn finish(&mut self) -> Vec<u8> {
        self.step(&[PadButton::SITUATION]);
        let mut subs = Vec::new();
        for _ in 0..3000 {
            let out = self.step(&[]);
            match self.sora.finish() {
                Some(f) if subs.last() != Some(&f.sub) => subs.push(f.sub),
                Some(_) => {}
                None if out.state != StateId::Stylefinish => break,
                None => {}
            }
        }
        subs
    }

    fn move_dummy(&mut self, x: f32) {
        self.host.targets[0] = dummy(1, x);
    }
}

#[test]
fn nano_arms_from_the_offer_to_its_end() {
    let mut b = Bench::new(200.0);
    assert_eq!(kh_core::combat::forms::moveset_name("form.GM"), "nano");
    // ---- offer -> transform
    let from = b.transform();
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.level, f.time_full, f.wear_form, f.grow), (0x86, 1, 30.0, 4, 1), "Nano Arms: level 1, 30 s, wear form 4 (Grace)");
    assert!(b.clips(from).contains(&"StyleChange8"), "{:?}", b.clips(from));
    assert!(f.weapon_units()[0].ends_with("w_so172_Wep"), "{:?}", f.weapon_units());
    assert_eq!(b.sora.combat().situation.slot_b.map(|e| e.kind), Some(0xA0), "FINISH_GM on offer");
    // ---- the combo near one target: the blade, then the crash, the giant stamp, the Rocket Fist
    b.idle(30);
    let from = b.log.len();
    let kinds = b.combo();
    // The crash (0xB5, air mode: the traction's Z channel lifts him toward the dummy's lock-on point 90 cm up) leaves him
    // more than 30 cm above the floor: PreDecide's bAir, so the second finisher is the yo-yos 0xB8 (on a lower target 0xB7).
    assert_eq!(kinds.len(), 6, "{kinds:?}");
    assert_eq!(kinds[..4], [1, 2, 3, 0xB5], "the ground combo near one target");
    assert!(matches!(kinds[4], 0xB7 | 0xB8) && kinds[5] == 0xB9, "{kinds:?}");
    assert!(b.log[from..].iter().any(|l| l.0 == StateId::ComboGm), "the attack class is COMBO_GM");
    let clips = b.clips(from);
    for c in ["8301", "8302", "8303", "8411", if kinds[4] == 0xB7 { "8413" } else { "8414" }, "8415"] {
        assert!(clips.contains(&c), "{c} not played: {clips:?}");
    }
    let hits = b.hits(from);
    for row in ["KB_VERTICAL", "SCGM_CRASH", if kinds[4] == 0xB7 { "SCGM_GIANTSTAMP" } else { "SCGM_BACK" }] {
        assert!(hits.contains(&row), "{row} never hit (the weapon's tracked shapes): {hits:?}");
    }
    // The punch (SCGM_FINISH on the fist) or its Rocket Fist: the yo-yos' root motion (5.5 m) carries him past the test dummy
    // (TestWorld has no push bodies), so the homing fist is what lands.
    assert!(hits.contains(&"SCGM_FINISH") || hits.contains(&"SCGM_FINISH_ROCKET"), "{hits:?}");
    // ---- the Rocket Fist: the blast, then the flying fist (homing, 2000 cm/s) hits the dummy
    assert!(b.sora.combat().nano.rockets_fired >= 1, "BX_ROCKET");
    let from = b.log.len();
    b.idle(120);
    assert!(b.hits(from).contains(&"SCGM_FINISH_ROCKET") || hits.contains(&"SCGM_FINISH_ROCKET"), "the fist hits");
    assert!(b.sora.combat().nano.rockets.is_empty() || b.sora.combat().nano.rockets.iter().all(|r| r.stage == 1), "the blast respawned as the fist");
    // ---- far: the hookshot, the pull, the kick
    b.idle(60);
    let x = b.step(&[]).position.x;
    b.move_dummy(x + 700.0);
    b.idle(20);
    let from = b.log.len();
    let first = b.sora.combat().history.len();
    b.step(&[PadButton::ATTACK]);
    b.idle(150);
    let kinds = b.sora.combat().history[first..].to_vec();
    assert_eq!(kinds.first(), Some(&0xBC), "far: the hookshot");
    assert!(kinds.contains(&0xBD), "its hit on the target pulls him in: {kinds:?}");
    assert!(b.clips(from).contains(&"8424"), "the kick M_8424");
    assert!(b.hits(from).contains(&"SCGM_HOOKSHOT"), "{:?}", b.hits(from));
    // ---- (F-POLISH: back in the main session, P3 fixed the finish after a counter) the Nano Counter out of the guard's
    // counter window: 0x25, BARRIERCRACKER
    b.idle(60);
    let out = b.step(&[]);
    let (me, fwd) = (out.position, Vec3::new(out.yaw.to_radians().cos(), out.yaw.to_radians().sin(), 0.0));
    let at = Vec3::new(me.x, me.y, 0.0) + fwd * 200.0;
    b.host.targets[0] = TargetInfo { position: at, center: at + Vec3::Z * 90.0, ..dummy(1, 0.0) };
    b.idle(20);
    // A guard (Nano Shield: the guard itself), a hit from the dummy guarded: the counter window opens.
    b.step(&[PadButton::GUARD]);
    b.idle(10);
    assert_eq!(b.log.last().map(|l| l.0), Some(StateId::Guard), "the guard");
    let d = (Vec3::new(me.x, me.y, 0.0) - at).normalize_or_zero();
    b.sora.take_hit(HitOnPlayer { direction: d, causer_yaw: d.y.atan2(d.x).to_degrees(), ..HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), at) });
    b.idle(1);
    assert!(b.sora.actions().counter_window > 0.0, "the guard's counter window");
    let from = b.log.len();
    let first = b.sora.combat().history.len();
    b.step(&[PadButton::ATTACK]);
    b.idle(90);
    assert_eq!(b.sora.combat().history[first..].first(), Some(&0x25), "the guard counter is the Nano Counter");
    assert!(b.clips(from).contains(&"MagicCast"), "BarrierCracker = sequence 1341: {:?}", b.clips(from));
    assert_eq!(b.sora.combat().nano.counters_fired, 1, "BARRIERCRACKER");
    assert!(b.hits(from).contains(&"SCGM_COUNTER"), "{:?}", b.hits(from));
    // ---- FINISH_GM (after the counter): the gate, Sora hidden while it fires, the portal's hits; the form ends
    b.idle(60);
    let from = b.log.len();
    let subs = b.finish();
    assert_eq!(subs, [2, 5, 6], "StyleFinishGM Tick's sub-steps");
    let clips = b.clips(from);
    for c in ["8416", "8417", "8418"] {
        assert!(clips.contains(&c), "{c}: {clips:?}");
    }
    assert!(b.sora.combat().nano.portal_shots > 10, "SCGM_PORTAL re-opens every 0.1 s: {}", b.sora.combat().nano.portal_shots);
    let hits = b.hits(from);
    assert!(hits.contains(&"SCGM_PORTAL") && hits.contains(&"SCGM_PORTAL2"), "{hits:?}");
    assert!(b.sora.combat().nano.gate.is_none() && !b.sora.combat().nano.hide_sora, "the gate gone, Sora shown");
    b.idle(5);
    assert_eq!(b.sora.combat().form.cmd, 0, "the finish ends the form");
}

#[test]
fn the_air_combo_of_nano_arms() {
    let mut b = Bench::new(200.0);
    b.transform();
    b.idle(30);
    // In the air level with an airborne dummy: 0x10, 0x17, 0x12, the drill, the yo-yos, the Rocket Fist.
    b.host.targets[0] = TargetInfo { position: Vec3::new(200.0, 0.0, 300.0), center: Vec3::new(200.0, 0.0, 390.0), flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY | TargetFlags::AIRBORNE), ..dummy(1, 200.0) };
    b.step(&[PadButton::JUMP]);
    for _ in 0..12 {
        b.step(&[PadButton::JUMP]);
    }
    let first = b.sora.combat().history.len();
    let kinds = b.combo();
    let kinds = &kinds[..kinds.len().min(6)];
    assert!(kinds.contains(&0xB6) && kinds.contains(&0xB8), "the air finishers: {kinds:?} (from {first})");
}
