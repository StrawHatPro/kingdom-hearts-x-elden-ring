//! (weapons F-STAFF) Mirage Staff (Ever After, CHANGE_WA 0x88): the trace of the form from the offer to its end
//! (build/sora_tmp/weapons/F-STAFF/STATE.md): offer -> STYLECHANGE (StyleChange10, wear form 2 Wisdom, w_so072 on R_wep7_skt)
//! -> the COMBO_WA combo (0x82 0x83 0x85, the braid 0x86 with its WA_BRAID, 0x87) -> the dodge leaves a mirror image (Avatar
//! Shift) -> with the images out the beam (0x80 x 9, 0x81: WA_BEAM / WA_FINISH from Sora and every image) -> the images go
//! -> FINISH_WA (the tower, its hits) -> the form's end.

use kh_core::host::EventKind;
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
        let value = if k == "abilities.equipped" { LoadoutValue::List(Vec::new()) } else { cell(&row[1]).unwrap() };
        loadout.set(k, &value).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    let mut s = Status::from_loadout(&loadout);
    s.weapon = key.to_string();
    s
}

fn dummy(id: u32, x: f32) -> TargetInfo {
    TargetInfo { id: TargetId(id), position: Vec3::new(x, 0.0, 0.0), center: Vec3::new(x, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e9, hp_max: 1.0e9 }
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    /// Every step: (state, sub-step, clip, the events).
    log: Vec<(StateId, u8, &'static str, Vec<EventKind>)>,
}

impl Bench {
    fn new(key: &str, at: f32) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding(key));
        let mut host = RecordingHost::new();
        host.targets.push(dummy(1, at));
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

    fn events(&self, from: usize) -> Vec<String> {
        self.log[from..].iter().flat_map(|(_, _, _, e)| e.iter().map(|k| format!("{k:?}"))).collect()
    }

    fn transform(&mut self, cmd: i32) -> usize {
        self.sora.debug_add_style(100.0);
        self.idle(15);
        let list: Vec<i32> = self.sora.combat().situation.list.iter().map(|e| e.kind).collect();
        assert_eq!(list, [cmd], "the formchange is offered");
        let from = self.log.len();
        self.step(&[PadButton::SITUATION]);
        assert_eq!(self.step(&[]).state, StateId::Stylechange);
        for _ in 0..400 {
            if self.step(&[]).state != StateId::Stylechange {
                return from;
            }
        }
        panic!("STYLECHANGE did not end");
    }

    /// Attack presses (every other step, or held) until the chain ends; the kinds played.
    fn combo(&mut self, hold: bool) -> Vec<u8> {
        let first = self.sora.combat().history.len();
        for k in 0..1800 {
            let out = if hold || k % 2 == 0 { self.step(&[PadButton::ATTACK]) } else { self.step(&[]) };
            if k > 30 && matches!(out.state, StateId::Idle | StateId::Run | StateId::Jump) {
                break;
            }
        }
        self.sora.combat().history[first..].to_vec()
    }

    /// Triangle on the finish command until STYLEFINISH ends: the sub-steps and Sora's highest point on the way.
    fn finish(&mut self) -> (Vec<u8>, f32) {
        self.step(&[PadButton::SITUATION]);
        let mut subs = Vec::new();
        let mut top = f32::MIN;
        for _ in 0..3000 {
            let out = self.step(&[]);
            top = top.max(out.position.z);
            match self.sora.finish() {
                Some(f) if subs.last() != Some(&f.sub) => subs.push(f.sub),
                Some(_) => {}
                None if out.state != StateId::Stylefinish => break,
                None => {}
            }
        }
        (subs, top)
    }

    fn hits_with(&self, row: &str) -> usize {
        self.host.hits().iter().filter(|(h, _)| h.attack_name.eq_ignore_ascii_case(row)).count()
    }
}

#[test]
fn mirage_staff_from_the_offer_to_its_end() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_04", 200.0);
    assert_eq!(kh_core::combat::forms::moveset_name("form.WA"), "staff");
    // ---- offer -> transform
    let from = b.transform(0x88);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.level, f.time_full, f.wear_form, f.grow), (0x88, 1, 20.0, 2, 1), "Mirage Staff: level 1, 20 s, wear form 2 (Wisdom)");
    assert!(b.clips(from).contains(&"StyleChange10"), "{:?}", b.clips(from));
    assert!(f.weapon_units()[0].contains("w_so072"), "{:?}", f.weapon_units());
    assert_eq!(b.sora.combat().situation.slot_b.map(|e| e.kind), Some(0xA2), "FINISH_WA on offer");
    assert_eq!(b.sora.combat().lockon.soft_range, 1800.0, "StartStyle 0x88: the soft-target range x 2");
    // ---- the combo: COMBO_WA, 0x82 0x83 0x85, the braid 0x86, 0x87
    b.idle(30);
    let from = b.log.len();
    let fired = b.sora.combat().staff.beams.fired;
    let kinds = b.combo(false);
    assert_eq!(kinds, [0x82, 0x83, 0x85, 0x86, 0x87], "the ground combo near one target");
    assert!(b.log[from..].iter().any(|l| l.0 == StateId::ComboWa), "the attack class is COMBO_WA");
    let clips = b.clips(from);
    for c in ["A414", "A415", "A419", "A413", "A416"] {
        assert!(clips.contains(&c), "{c} not played: {clips:?}");
    }
    assert_eq!(b.sora.combat().staff.beams.fired, fired + 1, "the braid's WA_BRAID");
    // (its CAPSULE1: radius 150, half length 400 upright; the shot is gone by now)
    for row in ["SCWA_FRONT", "SCWA_BREAK", "SCWA_BRITZ", "SCWA_BRAID"] {
        assert!(b.hits_with(row) > 0, "{row} hit the dummy");
    }
    // ---- FINISH_WA: the tower, its hits; the form ends
    let from = b.log.len();
    let ground = b.step(&[]).position.z;
    let (subs, top) = b.finish();
    assert_eq!(subs, [1, 2, 5, 6, 7, 8], "StyleFinishWA Tick's sub-steps");
    assert!(top > ground + 2000.0, "he rides X_link01_skt up the tower: {ground} -> {top}");
    let clips = b.clips(from);
    assert!(clips.contains(&"A417") && clips.contains(&"M_1501"), "{clips:?}");
    assert!(b.hits_with("SCWA_TOWER") > 0, "the tower's light");
    b.idle(5);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.wear_form), (0, 0), "the finish ends the form");
    assert_eq!(b.sora.combat().lockon.soft_range, 900.0, "EndStyle: the range back");
}

/// The dodge leaves mirror images (Avatar Shift); with them out the combo is the beam, Sora and every image fire; the
/// finish sends them away, else 10 s after the last spawn.
#[test]
fn mirage_staff_mirror_images_and_beams() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_04", 200.0);
    b.transform(0x88);
    b.idle(30);
    // ---- the dodge in battle without the stick: the back slide and a mirror image (Avatar Shift)
    b.idle(60);
    b.sora.force_battle(true);
    let from = b.log.len();
    b.step(&[PadButton::GUARD]);
    b.idle(60);
    assert!(b.clips(from).contains(&"A058"), "BackSlide of AnimSet10: {:?}", b.clips(from));
    assert_eq!(b.sora.combat().staff.avatars.list.len(), 1, "one image");
    assert!(b.sora.combat().staff.avatars.out);
    for _ in 0..5 {
        b.step(&[PadButton::GUARD]);
        b.idle(60);
    }
    assert_eq!(b.sora.combat().staff.avatars.list.len(), 4, "at most four");
    // ---- with the images out: the beam, held, then its finisher; every image fires with Sora
    let (fired, av) = (b.sora.combat().staff.beams.fired, b.sora.combat().staff.avatars.fired);
    let from = b.log.len();
    let kinds = b.combo(true);
    assert!(kinds.len() >= 2 && kinds[..kinds.len() - 1].iter().all(|k| *k == 0x80) && kinds.last() == Some(&0x81), "the beam chain: {kinds:x?}");
    assert!(b.clips(from).contains(&"A411") && b.clips(from).contains(&"A412"));
    assert!(b.sora.combat().staff.beams.fired > fired + 4, "Sora's and the images' beams: {} -> {}", fired, b.sora.combat().staff.beams.fired);
    assert!(b.sora.combat().staff.avatars.fired > av, "the images fired");
    let ev = b.events(from);
    assert!(ev.iter().any(|e| e.contains("vw_so072_mrg_blt0")) && ev.iter().any(|e| e.contains("vw_so072_mrg_fia1")), "the beam's particle and fire effect");
    assert!(b.hits_with("SCWA_BEAM") > 0 && b.hits_with("SCWA_FINISH") > 0, "the beams hit");
    // The finish sends them away 0.5 s after it (state 4 -> slot 868(3)).
    b.idle(120);
    assert!(b.sora.combat().staff.avatars.list.is_empty() && !b.sora.combat().staff.avatars.out, "the images went after the finish");
    // ---- one image again: it goes 10 s after its spawn
    b.step(&[PadButton::GUARD]);
    b.idle(30);
    assert_eq!(b.sora.combat().staff.avatars.list.len(), 1);
    b.idle(600);
    assert!(b.sora.combat().staff.avatars.list.is_empty(), "10 s");
}

/// The images out with nothing playing: one tap of the attack gives the beam (MaxCombo 10, Finishers 1).
#[test]
fn the_images_turn_the_combo_into_the_beam() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_04", 600.0);
    b.transform(0x88);
    b.idle(30);
    b.sora.force_battle(true);
    b.step(&[PadButton::GUARD]);
    b.idle(60);
    assert!(b.sora.combat().staff.avatars.out);
    b.step(&[PadButton::ATTACK]);
    b.step(&[]);
    assert_eq!(b.sora.combat().history.last(), Some(&0x80));
    b.idle(120);
    // Without them the far target (600 cm: reach 1) begins with the rising spin.
    b.idle(700);
    assert!(!b.sora.combat().staff.avatars.out);
    b.step(&[PadButton::ATTACK]);
    b.step(&[]);
    assert_eq!(b.sora.combat().history.last(), Some(&0x85), "reach 1: 0x85 first");
}
