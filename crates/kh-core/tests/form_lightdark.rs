//! (weapons F-LIGHTDARK) Light Form (Oathkeeper, CHANGE_LF 0xDD), Dark Form (Oblivion, CHANGE_DF 0xDE) and their second
//! stage Double Form (CHANGE_TF 0xDF / CHANGE_TF2 0xE0): the trace of each from the offer to its end
//! (build/sora_tmp/weapons/F-LIGHTDARK/STATE.md):
//! offer -> STYLECHANGE (StyleChange / StyleChangeLoop / StyleChangeEnd, wear form 8 / 9) -> COMBO_LF / COMBO_DF (1 2 3, then
//! 0xCD 0xCE 0xCF / 0xD0 Dark Break with its warp) -> the next full gauge offers Double Form inside the form -> STYLECHANGE
//! (wear form 10, AnimSet23's StyleChangeEnd) -> COMBO_TF (0xD4 0xD5 0xD7, then 0xDA 0xDB 0xDC) -> FINISH_TF / FINISH_TF2
//! (Ultimate Finish: M_N424 -> M_N425) -> the form's end. Also: the reprisal (Light Burst / Dual Smasher, kind 0x25), the
//! form guard (Reflec, every direction) and Sonic Dodge (M051).
//!
//! With `KH_LD_TRACE=<dir>` every session's step log is written there (the STATE.md traces).

use kh_core::enums::ETresDamageKind;
use kh_core::host::recording::Call;
use kh_core::host::EventKind;
use kh_core::states::HitOnPlayer;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, Output, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};
use std::fmt::Write as _;

const DT: f32 = 1.0 / 60.0;
const OATHKEEPER: &str = "WEP_KEYBLADE_SO_12";
const OBLIVION: &str = "WEP_KEYBLADE_SO_13";

fn cell(json: &serde_json::Value) -> Option<LoadoutValue> {
    match json {
        serde_json::Value::Number(n) => n.as_f64().map(LoadoutValue::Number),
        serde_json::Value::String(s) => Some(LoadoutValue::Text(s.clone())),
        serde_json::Value::Array(items) => items.iter().map(|i| i.as_str().map(str::to_string)).collect::<Option<Vec<_>>>().map(LoadoutValue::List),
        serde_json::Value::Object(map) => map.iter().map(|(k, v)| v.as_f64().map(|n| (k.clone(), n))).collect::<Option<Vec<_>>>().map(LoadoutValue::Map),
        _ => None,
    }
}

/// Level 99 with no equipped abilities (the forms bring their own: p_ex001_FormAbility), holding `key`.
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
    TargetInfo { id: TargetId(id), position: Vec3::new(x, 0.0, 0.0), center: Vec3::new(x, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e7, hp_max: 1.0e7 }
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    log: Vec<(StateId, u8, &'static str, Vec<EventKind>)>,
    held: PadButtons,
    trace: String,
}

impl Bench {
    fn new(key: &str) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding(key));
        let mut host = RecordingHost::new();
        host.targets.push(dummy(1, 200.0));
        let mut b = Bench { sora, host, world: TestWorld::floor(), log: Vec::new(), held: PadButtons::NONE, trace: String::new() };
        b.idle(5);
        b
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let before = self.held;
        self.held = set;
        let input = Input::default().with_pad(PadState::from_held(set, before));
        let (sora, world) = (&mut self.sora, &self.world);
        let calls = self.host.calls.len();
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        let f = self.sora.combat().form;
        let a = self.sora.attack();
        let _ = write!(self.trace, "{} {:?}/{} {} p=({:.0},{:.0},{:.0}) | form {:#x} lv{} t={:.2} wear {} fin {:#x} | atk {}", self.log.len(), out.state, out.sub_step, out.anim.name.name(), out.position.x, out.position.y, out.position.z, f.cmd, f.level, f.time_left, f.wear_form, f.finish_cmd, a.map_or("-".to_string(), |a| format!("{:#x}#{}", a.kind, a.combo)));
        for c in &self.host.calls[calls..] {
            if let Call::Hit(h, r) = c {
                if r.accepted {
                    let _ = write!(self.trace, " | hit {}", h.attack_name);
                }
            }
        }
        if let Some(fv) = self.sora.finish() {
            let _ = write!(self.trace, " | finish {:#x} sub {}", fv.cmd, fv.sub);
        }
        self.trace.push('\n');
        self.log.push((out.state, out.sub_step, out.anim.name.name(), out.events.iter().map(|e| e.kind).collect()));
        out
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    fn mark(&mut self, what: &str) {
        let _ = writeln!(self.trace, "== {what}");
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

    fn hit_rows(&self, from: usize) -> Vec<&'static str> {
        self.host.calls[from..].iter().filter_map(|c| match c { Call::Hit(h, r) if r.accepted => Some(h.attack_name), _ => None }).collect()
    }

    fn states(&self, from: usize) -> Vec<StateId> {
        let mut out = Vec::new();
        for l in &self.log[from..] {
            if out.last() != Some(&l.0) {
                out.push(l.0);
            }
        }
        out
    }

    /// The offer, Triangle, the transformation; returns the step it began at.
    fn transform(&mut self, cmd: i32) -> usize {
        self.sora.debug_add_style(100.0);
        // (past the 0.6 s command cool-down of an air attack that ended without a follow-up)
        self.idle(45);
        let list: Vec<i32> = self.sora.combat().situation.list.iter().map(|e| e.kind).collect();
        assert!(list.contains(&cmd), "{cmd:#x} on offer: {list:?}");
        let from = self.log.len();
        self.step(&[PadButton::SITUATION]);
        let state = self.step(&[]).state;
        if state != StateId::Stylechange {
            self.dump("failed");
        }
        assert_eq!(state, StateId::Stylechange);
        for _ in 0..400 {
            if self.step(&[]).state != StateId::Stylechange {
                return from;
            }
        }
        panic!("STYLECHANGE did not end");
    }

    /// Attack presses until the chain ends; the kinds played.
    fn combo(&mut self) -> Vec<u8> {
        let first = self.sora.combat().history.len();
        for k in 0..900 {
            let out = if k % 2 == 0 { self.step(&[PadButton::ATTACK]) } else { self.step(&[]) };
            if k > 30 && matches!(out.state, StateId::Idle | StateId::Run | StateId::Jump) {
                break;
            }
        }
        self.sora.combat().history[first..].to_vec()
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
                None if out.state != StateId::Stylefinish => break,
                None => {}
            }
        }
        subs
    }

    fn dump(&self, name: &str) {
        if let Some(dir) = std::env::var_os("KH_LD_TRACE") {
            let path = std::path::Path::new(&dir).join(format!("{name}.trace"));
            std::fs::create_dir_all(std::path::Path::new(&dir)).unwrap();
            std::fs::write(&path, &self.trace).unwrap();
        }
    }
}

fn hit_in_front(own: Vec3, from: Vec3) -> HitOnPlayer {
    let d = (own - from).normalize_or_zero();
    HitOnPlayer { direction: d, causer_yaw: 180.0, ..HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), from) }
}

#[test]
fn light_form_to_double_form_to_ultimate_finish() {
    let mut b = Bench::new(OATHKEEPER);
    assert_eq!(kh_core::combat::forms::moveset_name("form.LF"), "lightdark");
    b.mark("LF: offer, transformation");
    let from = b.transform(0xDD);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.level, f.time_full, f.wear_form), (0xDD, 1, 30.0, 8), "Light Form: level 1, 30 s, wear form 8");
    let clips = b.clips(from);
    assert!(clips.contains(&"StyleChange") && clips.contains(&"StyleChangeEnd"), "the Second Form clips: {clips:?}");
    assert!(f.weapon_units()[0].ends_with("w_so200_Wep"), "{:?}", f.weapon_units());
    b.idle(30);
    b.mark("LF: the combo");
    let calls = b.host.calls.len();
    let from = b.log.len();
    let kinds = b.combo();
    assert_eq!(kinds, [9, 0x0A, 0x0B, 0xCD, 0xCE, 0xCF], "Light Form near one target: the wear form's upgrades, then Zantetsuken, Finishing Leap, Black Hole");
    assert!(b.states(from).contains(&StateId::ComboLf), "the attack class is COMBO_LF: {:?}", b.states(from));
    let rows = b.hit_rows(calls);
    for r in ["SCLF_ZANTETSUKEN", "SCLF_FINISHRISE", "SCLF_EVENTHORIZON"] {
        assert!(rows.contains(&r), "{r} did not hit: {rows:?}");
    }
    let clips = b.clips(from);
    for c in ["M_M411", "M412", "M_M413"] {
        assert!(clips.contains(&c), "{c} not played: {clips:?}");
    }
    // The next full gauge offers Double Form inside Light Form (AwardAtMax at form level 1).
    b.idle(30);
    b.mark("TF: offer inside Light Form, transformation");
    let from = b.transform(0xDF);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.level, f.wear_form), (0xDF, 2, 10), "Double Form: level 2, wear form 10");
    assert!(b.clips(from).contains(&"N401"), "AnimSet23's StyleChangeEnd: {:?}", b.clips(from));
    assert_eq!(f.weapon_units().len(), 2, "both Keyblades: {:?}", f.weapon_units());
    b.idle(30);
    b.mark("TF: the combo");
    let calls = b.host.calls.len();
    let from = b.log.len();
    let kinds = b.combo();
    assert_eq!(kinds, [0xD4, 0xD5, 0xD7, 0xDD, 0xDB, 0xDC], "Double Form: its chain (0xD7 is an air attack: PlayAttack case 0xD7 breaks to +0x1DF = 1), Equilibrium Burst (air), Flash Charge, Mirage Thrust");
    assert!(b.states(from).contains(&StateId::ComboTf));
    let rows = b.hit_rows(calls);
    for r in ["SCTF_COMBOSLASH", "SCTF_RUSHCLOSS", "SCTF_SWING", "SCTF_BURSTSTREAM", "SCTF_SONICSTRIKE", "SCTF_JOINTSTRUGGLE", "SCTF_JOINTSTRUGGLE_END"] {
        assert!(rows.contains(&r), "{r} did not hit: {rows:?}");
    }
    b.idle(30);
    b.idle(40);
    b.mark("TF: Ultimate Finish");
    assert_eq!(b.sora.combat().situation.slot_b.map(|e| e.kind), Some(0xE1), "FINISH_TF on offer");
    let calls = b.host.calls.len();
    let from = b.log.len();
    let subs = b.finish();
    assert_eq!(subs, [1, 2, 5, 12], "slot 166's sub-steps (sub 0 runs in the Begin tick)");
    let clips = b.clips(from);
    assert!(clips.contains(&"N424") && clips.contains(&"N425"), "{clips:?}");
    assert!(b.hit_rows(calls).contains(&"SCTF_FINISH"), "{:?}", b.hit_rows(calls));
    b.idle(10);
    assert_eq!((b.sora.combat().form.cmd, b.sora.combat().form.wear_form), (0, 0), "the form ends with the finish");
    b.dump("light_double");
}

#[test]
fn dark_form_dark_break_and_double_form() {
    let mut b = Bench::new(OBLIVION);
    b.mark("DF: offer, transformation");
    b.transform(0xDE);
    let f = b.sora.combat().form;
    assert_eq!((f.cmd, f.wear_form), (0xDE, 9));
    assert!(f.weapon_units()[0].ends_with("w_so210_Wep"));
    b.idle(30);
    b.mark("DF: the combo, Dark Break's warp");
    let calls = b.host.calls.len();
    let from = b.log.len();
    let kinds = b.combo();
    assert_eq!(kinds, [9, 0x0A, 0x0B, 0xCD, 0xCE, 0xD0], "Dark Form: Dark Break third");
    let clips = b.clips(from);
    let warp = clips.iter().position(|c| *c == "JumpNormalStart");
    let dark_break = clips.iter().position(|c| *c == "M420");
    assert!(warp.is_some() && warp < dark_break, "the warp's JumpNormalStart, then M_M420: {clips:?}");
    let rows = b.hit_rows(calls);
    assert!(rows.contains(&"SCDF_DARKBREAK"), "{rows:?}");
    assert!(b.states(from).contains(&StateId::ComboDf));
    b.idle(30);
    b.mark("TF2: offer inside Dark Form");
    b.transform(0xE0);
    assert_eq!(b.sora.combat().form.cmd, 0xE0);
    b.idle(30);
    assert_eq!(b.sora.combat().situation.slot_b.map(|e| e.kind), Some(0xE2), "FINISH_TF2 on offer");
    b.idle(40);
    b.mark("TF2: Ultimate Finish");
    let subs = b.finish();
    assert_eq!(subs, [1, 2, 5, 12]);
    b.idle(10);
    assert_eq!(b.sora.combat().form.cmd, 0);
    b.dump("dark_double");
}

#[test]
fn light_guard_light_burst_and_sonic_dodge() {
    let mut b = Bench::new(OATHKEEPER);
    b.transform(0xDD);
    b.idle(30);
    // Light Guard: Reflec (8279), guarded from behind too (Body reaction type 3), no REFLECT_GUARD needed.
    b.mark("LF: Light Guard from behind, Light Burst");
    let from = b.log.len();
    b.step(&[PadButton::GUARD]);
    b.step(&[]);
    assert!(b.clips(from).contains(&"Reflec"), "{:?}", b.clips(from));
    let own = b.sora.output().position;
    let hp = b.sora.status().hp;
    b.sora.take_hit(hit_in_front(own, own - Vec3::new(150.0, 0.0, 0.0)));
    b.idle(3);
    assert_eq!(b.sora.status().hp, hp, "guarded from behind");
    // The reprisal window: 0xD7 (Light Burst) without GUARD_COUNTER.
    assert_eq!(b.sora.actions().counter_command, 0xD7);
    let calls = b.host.calls.len();
    let from = b.log.len();
    b.step(&[PadButton::ATTACK]);
    b.idle(90);
    assert!(b.sora.combat().history.contains(&0x25), "the counter kind 0x25: {:?}", b.sora.combat().history);
    assert!(b.clips(from).contains(&"MagicCast"), "{:?}", b.clips(from));
    assert!(b.hit_rows(calls).contains(&"SCGM_COUNTER"), "BARRIERCRACKER hits: {:?}", b.hit_rows(calls));
    // Sonic Dodge: AnimSet22's DodgeRollB.
    b.mark("LF: Sonic Dodge");
    let from = b.log.len();
    for _ in 0..3 {
        b.step(&[PadButton::GUARD]);
    }
    let input_dodge = |b: &mut Bench| {
        let set = PadButtons::NONE.with(PadButton::GUARD);
        let before = b.held;
        b.held = set;
        let mut input = Input::default().with_pad(PadState::from_held(set, before));
        input.move_stick = kh_core::Vec2::new(0.0, 1.0);
        let (sora, world) = (&mut b.sora, &b.world);
        let out = b.host.step(world, |h| sora.step_hosted(DT, &input, h));
        b.log.push((out.state, out.sub_step, out.anim.name.name(), Vec::new()));
    };
    // (in battle: CanDodge 0x141427D98 lets weapon forms 0x16 / 0x17 dodge without DODGE)
    b.idle(90);
    b.sora.force_battle(true);
    b.idle(2);
    for _ in 0..2 {
        input_dodge(&mut b);
    }
    b.idle(60);
    assert!(b.clips(from).contains(&"M051"), "Sonic Dodge: {:?}", b.clips(from));
    b.dump("light_guard");
}

/// (F-LIGHTDARK for F-HAMMER) CanDodge 0x141427D98: in battle, weapon forms 6 / 0x15 (Drill Punch 0x84, Clock Drill 0x93)
/// dodge without DODGE; outside those forms (and with no DODGE) the guard button with the stick does nothing.
#[test]
fn drill_forms_dodge_without_the_ability() {
    for (cmd, dodges) in [(0x84, true), (0x93, true), (0, false)] {
        let mut b = Bench::new(OATHKEEPER);
        b.idle(90);
        b.sora.combat_mut().form.cmd = cmd;
        b.sora.combat_mut().form.time_left = 60.0;
        b.sora.force_battle(true);
        b.idle(2);
        let set = PadButtons::NONE.with(PadButton::GUARD);
        let mut states = Vec::new();
        for _ in 0..2 {
            let before = b.held;
            b.held = set;
            let mut input = Input::default().with_pad(PadState::from_held(set, before));
            input.move_stick = kh_core::Vec2::new(0.0, 1.0);
            let (sora, world) = (&mut b.sora, &b.world);
            states.push(b.host.step(world, |h| sora.step_hosted(DT, &input, h)).state);
        }
        states.push(b.step(&[]).state);
        let state = if states.contains(&StateId::Dodgeroll) { StateId::Dodgeroll } else { states[2] };        assert_eq!(state == StateId::Dodgeroll, dodges, "form {cmd:#x}: {state:?}");
    }
}
