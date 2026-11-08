//! (weapons F-HAMMER) Hyper Hammer (Favorite Deputy, CHANGE_HM 0x81) -> Drill Punch (CHANGE_DR 0x84) and Boom Hammer (Classic
//! Tone, CHANGE_HM2 0x92) -> Clock Drill (CHANGE_DR2 0x93): the trace of each pair from the offer to its end
//! (build/sora_tmp/weapons/F-HAMMER/STATE.md): offer -> STYLECHANGE (StyleChange3 / 20, the wear form) -> the COMBO_HM combo
//! (0x58 0x5B 0x5A 0x59 0x5D) -> the second stage's offer inside the first -> STYLECHANGE (StyleChange6 / 21) -> the COMBO_DR
//! combo (0x62 0x63 0x61 0x64 0x6A) -> FINISH_DR (the drill / the Clock) -> the form's end.

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
    fn new(key: &str) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding(key));
        let mut host = RecordingHost::new();
        host.targets.push(dummy(1, 200.0));
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

    /// The offer of `cmd` (30 s), Triangle, the transformation; returns the step the form began at.
    fn transform(&mut self, cmd: i32) -> usize {
        self.sora.debug_add_style(100.0);
        self.idle(15);
        let list: Vec<(i32, f32)> = self.sora.combat().situation.list.iter().map(|e| (e.kind, e.total)).collect();
        assert_eq!(list, [(cmd, 30.0)], "the formchange is offered for its 30 s");
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

    /// Attack presses until the chain ends (back to IDLE); the kinds played.
    fn combo(&mut self) -> Vec<u8> {
        let first = self.sora.combat().history.len();
        for k in 0..1200 {
            let out = if k % 2 == 0 { self.step(&[PadButton::ATTACK]) } else { self.step(&[]) };
            if k > 30 && matches!(out.state, StateId::Idle | StateId::Run | StateId::Jump) {
                break;
            }
        }
        self.sora.combat().history[first.min(self.sora.combat().history.len())..].to_vec()
    }

    /// Triangle on the finish command, until STYLEFINISH ends; the sub-steps.
    fn finish(&mut self) -> Vec<u8> {
        self.step(&[PadButton::SITUATION]);
        let mut subs = Vec::new();
        for _ in 0..3000 {
            let out = self.step(&[]);
            match self.sora.finish() {
                Some(f) if subs.last() != Some(&f.sub) => subs.push(f.sub),
                Some(_) => {}
                None if !matches!(out.state, StateId::Stylefinish | StateId::StylefinishDr) => break,
                None => {}
            }
        }
        subs
    }
}

/// The trace of a pair: (the Keyblade, the first stage, the second stage, their transformation clips, the wear form, the
/// first weapon's model, the second's).
fn the_pair(key: &str, hm: i32, dr: i32, clips: [&str; 2], wear: u8, models: [&str; 2]) -> Bench {
    let mut b = Bench::new(key);
    assert_eq!(kh_core::combat::forms::moveset_name(if hm == 0x81 { "form.HM" } else { "form.HM2" }), "hammer");
    // ---- offer -> transform (the first stage)
    let from = b.transform(hm);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.level, f.time_full, f.wear_form, f.grow), (hm, 1, 30.0, wear, 1));
    assert!(b.clips(from).contains(&clips[0]), "{:?}", b.clips(from));
    assert!(f.weapon_units()[0].ends_with(models[0]), "{:?}", f.weapon_units());
    // ---- the COMBO_HM combo near one target: 0x58 0x5B 0x5A 0x59, the Giant Stamp
    b.idle(30);
    let from = b.log.len();
    let kinds = b.combo();
    assert_eq!(kinds, [0x58, 0x5B, 0x5A, 0x59, 0x5D], "the hammer combo");
    assert!(b.log[from..].iter().any(|l| l.0 == StateId::ComboHm), "the attack class is COMBO_HM");
    let clips_played = b.clips(from);
    for c in ["3411", "3414", "3413", "3412", "M_3416"] {
        assert!(clips_played.contains(&c), "{c} not played: {clips_played:?}");
    }
    assert!(b.host.hits().iter().any(|(h, _)| h.target == TargetId(1)), "the hammer's shapes hit the dummy");
    // ---- the second stage inside the first: a full gauge offers it, Triangle
    b.idle(60);
    let from = b.transform(dr);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.level, f.time_full, f.wear_form), (dr, 2, 25.0, wear), "the second stage: level 2, 25 s");
    assert!(b.clips(from).contains(&clips[1]), "{:?}", b.clips(from));
    assert!(f.weapon_units()[0].ends_with(models[1]), "{:?}", f.weapon_units());
    assert_eq!(b.sora.combat().situation.slot_b.map(|e| e.kind), Some(0x9E), "FINISH_DR on offer");
    // ---- the COMBO_DR combo: 0x62 0x63 0x61 0x64, the giga upper
    b.idle(30);
    let from = b.log.len();
    assert_eq!(b.combo(), [0x62, 0x63, 0x61, 0x64, 0x6A], "the drill combo");
    assert!(b.log[from..].iter().any(|l| l.0 == StateId::ComboDr), "the attack class is COMBO_DR");
    b.idle(30);
    b
}

#[test]
fn favorite_deputy_hyper_hammer_to_drill_punch() {
    let mut b = the_pair("WEP_KEYBLADE_SO_03", 0x81, 0x84, ["StyleChange3", "StyleChange6"], 1, ["w_so052_Wep", "w_so053_Wep"]);
    // ---- FINISH_DR: the drill (slot 148): M_6421, the warhead (fired 3 .. 9 m ahead) catches the dummy, M_6423, the end
    let o = b.step(&[]);
    let a = o.yaw.to_radians();
    let at = o.position + Vec3::new(a.cos(), a.sin(), 0.0) * 600.0;
    (b.host.targets[0].position, b.host.targets[0].center) = (Vec3::new(at.x, at.y, 0.0), Vec3::new(at.x, at.y, 90.0));
    let from = b.log.len();
    let subs = b.finish();
    assert_eq!(subs, [2, 5], "slot 148's sub-steps (0 runs in Begin's tick)");
    let clips = b.clips(from);
    assert!(clips.contains(&"6421") && clips.contains(&"6423"), "{clips:?}");
    assert!(b.host.hits().iter().any(|(h, _)| h.attack_name.eq_ignore_ascii_case("SCDR_FINISH")), "the warhead's hit (DRILL_BIND)");
    assert!(b.host.hits().iter().any(|(h, _)| h.attack_name.eq_ignore_ascii_case("SCDR_FIN_END")), "M_6423's last impact");
    b.idle(5);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.wear_form), (0, 0), "the finish ends the form");
}

#[test]
fn classic_tone_boom_hammer_to_clock_drill() {
    let mut b = the_pair("WEP_KEYBLADE_SO_11", 0x92, 0x93, ["StyleChange20", "StyleChange21"], 3, ["w_so112_Wep", "w_so113_Wep"]);
    // Classic Tone's rows: HM2_* / DR2_*.
    assert!(b.host.hits().iter().any(|(h, _)| h.attack_name.starts_with("HM2_")), "Boom Hammer hits with HM2_* rows");
    assert!(b.host.hits().iter().any(|(h, _)| h.attack_name.starts_with("DR2_")), "Clock Drill hits with DR2_* rows");
    // Its toon effects (effect group 0x76C).
    let ev = b.events(0);
    assert!(ev.iter().any(|e| e.contains("vw_so112")), "Boom Hammer's own effects");
    // ---- FINISH_DR: the Clock (StyleFinishDR Tick): M_L421, its windows at the Clock, the clean-up
    let shots = b.sora.combat().shots_spawned;
    let from = b.log.len();
    let subs = b.finish();
    assert_eq!(subs, [2, 6], "StyleFinishDR's sub-steps (M_L421 has no TresEnableChange: sub 6 runs on at its end, 8 in the same tick)");
    assert!(b.clips(from).contains(&"L421"));
    assert!(b.sora.combat().shots_spawned >= shots + 3, "the Clock's windows: {} -> {}", shots, b.sora.combat().shots_spawned);
    b.idle(5);
    assert_eq!(b.sora.combat().form.cmd, 0);
}
