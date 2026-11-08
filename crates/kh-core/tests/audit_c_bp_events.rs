//! (AUDIT-C) THE FORM CLIPS' BpEvent WINDOWS REACH THE HOSTS (states/bp_event.rs), at the clip times KH3's notifies give,
//! for the state-event camera blueprints (kh-camera form_bp.rs): Ultimate Form (9401 StyleChange9, 9418 COMBO_SW's last
//! finisher, 9419 FINISH_SW), Light Form -> Double Form (1401 / 1403 StyleChange / StyleChangeEnd, the Ultimate Finish's
//! N424 / N425 with the Blue Hour particle vw_so220_fin_fsg0), Rage Form (D401 StyleChange13, D417 Crimson Blast, D418
//! FINISH_AN; RISKCHARGE's D419 has no BpEvent). Second Form's are sent too (since 2026-11-05).
//! `KH_AUDITC_TRACE=<dir>` writes the event lists (build/sora_tmp/weapons/AUDIT-C/CAM/traces).

use std::fmt::Write as _;

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
        loadout.set(row[0].as_str().unwrap(), &cell(&row[1]).unwrap()).unwrap_or_else(|e| panic!("{e}"));
    }
    loadout.resolve().unwrap();
    let mut s = Status::from_loadout(&loadout);
    s.weapon = key.to_string();
    s
}

fn dummy(id: u32, x: f32) -> TargetInfo {
    TargetInfo { id: TargetId(id), position: Vec3::new(x, 0.0, 0.0), center: Vec3::new(x, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e9, hp_max: 1.0e9 }
}

/// One BpEvent as the hosts saw it: (step, begin, seq, the clip and its time, the state).
#[derive(Clone, Debug)]
struct Bp {
    step: usize,
    begin: bool,
    seq: &'static str,
    clip: &'static str,
    time: f32,
    state: StateId,
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    held: PadButtons,
    steps: usize,
    bp: Vec<Bp>,
    /// (step, effect name, spawned) of the Blue Hour particle.
    fsg: Vec<(usize, bool)>,
    trace: String,
}

impl Bench {
    fn new(key: &str, at: f32) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding(key));
        let mut host = RecordingHost::new();
        host.targets.push(dummy(1, at));
        let mut b = Bench { sora, host, world: TestWorld::floor(), held: PadButtons::NONE, steps: 0, bp: Vec::new(), fsg: Vec::new(), trace: String::new() };
        b.idle(5);
        b
    }

    fn step(&mut self, held: &[PadButton]) -> Output {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input::default().with_pad(PadState::from_held(set, self.held));
        self.held = set;
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        self.steps += 1;
        let mut fsg_handle = None;
        for e in &out.events {
            match e.kind {
                EventKind::AnimBpEvent { seq, begin, .. } => {
                    let b = Bp { step: self.steps, begin, seq, clip: out.anim.name.name(), time: out.anim.time, state: out.state };
                    let _ = writeln!(self.trace, "{:5} {:?} {} {} clip {} @ {:.3} s | form {:#x} | at ({:.0}, {:.0}, {:.0}) yaw {:.1}", b.step, b.state, if begin { "START" } else { "END  " }, seq, b.clip, b.time, self.sora.combat().form.cmd, out.position.x, out.position.y, out.position.z, out.yaw);
                    self.bp.push(b);
                }
                EventKind::EffectSpawn { handle, spawn } if spawn.name == kh_core::states::bp_event::BLUE_HOUR_PARTICLE => {
                    fsg_handle = Some(handle);
                    let _ = writeln!(self.trace, "{:5} spawn {} at ({:.0}, {:.0}, {:.0})", self.steps, spawn.name, out.position.x, out.position.y, out.position.z);
                    self.fsg.push((self.steps, true));
                }
                EventKind::EffectStop { handle, immediate } if self.sora.combat().bp_events.blue_hour.is_none() && fsg_handle.is_none() && self.fsg.last().is_some_and(|f| f.1) => {
                    let _ = writeln!(self.trace, "{:5} stop effect {} immediate {immediate}", self.steps, handle.0);
                    self.fsg.push((self.steps, false));
                }
                _ => {}
            }
        }
        out
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    fn transform(&mut self, cmd: i32) {
        self.sora.debug_add_style(100.0);
        self.idle(45);
        let list: Vec<i32> = self.sora.combat().situation.list.iter().map(|e| e.kind).collect();
        assert!(list.contains(&cmd), "{cmd:#x} on offer: {list:?}");
        self.step(&[PadButton::SITUATION]);
        for _ in 0..600 {
            if self.step(&[]).state != StateId::Stylechange && self.sora.combat().form.cmd == cmd {
                return;
            }
        }
        panic!("the transformation did not end");
    }

    fn combo(&mut self) {
        for k in 0..1800 {
            let out = if k % 2 == 0 { self.step(&[PadButton::ATTACK]) } else { self.step(&[]) };
            if k > 30 && matches!(out.state, StateId::Idle | StateId::Run | StateId::Jump) {
                break;
            }
        }
    }

    fn finish(&mut self) {
        self.step(&[PadButton::SITUATION]);
        for _ in 0..3000 {
            let out = self.step(&[]);
            if self.sora.finish().is_none() && out.state != StateId::Stylefinish {
                break;
            }
        }
    }

    fn seqs(&self) -> Vec<(&'static str, bool)> {
        self.bp.iter().map(|b| (b.seq, b.begin)).collect()
    }

    fn of(&self, seq: &str, begin: bool) -> &Bp {
        self.bp.iter().find(|b| b.seq == seq && b.begin == begin).unwrap_or_else(|| panic!("{seq} {} not sent: {:?}", if begin { "start" } else { "end" }, self.seqs()))
    }

    fn dump(&self, name: &str) {
        if let Some(dir) = std::env::var_os("KH_AUDITC_TRACE") {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(std::path::Path::new(&dir).join(format!("{name}.trace")), &self.trace).unwrap();
        }
    }
}

/// The clip time of a window's start: the notify's own (sora_anim.rs NotifyDef time), within a frame.
fn at(b: &Bp, t: f32) {
    assert!((b.time - t).abs() <= DT + 1e-3 || b.time == 0.0 && t < DT, "{} at {:.3} s, the notify says {t:.3} s", b.seq, b.time);
}

/// A window's end: at the notify's end time in its clip, or (a window that closes with its clip) on the next clip.
fn ends(start: &Bp, end: &Bp, t: f32) {
    assert!(end.clip != start.clip || (end.time - t).abs() <= DT + 1e-3, "{} ends at {:.3} s of {}, the notify says {t:.3} s of {}", end.seq, end.time, end.clip, start.clip);
    assert!(end.step >= start.step + ((t - start.time) / DT) as usize - 2, "{}: {} -> {} steps for a {t:.3} s window", end.seq, start.step, end.step);
}

#[test]
fn ultimate_form_sends_9401_9418_9419() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_14", 150.0);
    b.transform(0x87);
    let s = b.of("9401", true);
    assert_eq!((s.state, s.clip), (StateId::Stylechange, "StyleChange9"));
    b.idle(60);
    b.combo();
    let s = b.of("9418", true).clone();
    assert_eq!((s.state, s.clip), (StateId::ComboSw, "9418"));
    at(&s, 0.0);
    let e = b.of("9418", false).clone();
    ends(&s, &e, 1.766_666_7);
    b.idle(30);
    b.sora.debug_add_style(100.0);
    b.idle(60);
    b.finish();
    let s = b.of("9419", true);
    assert_eq!((s.state, s.clip), (StateId::Stylefinish, "M_9419"), "the XS overlay row M_9419 = sequence 9419");
    let s = s.clone();
    b.idle(120);
    let e = b.of("9419", false).clone();
    ends(&s, &e, 4.866_667);
    b.dump("ultimate");
}

#[test]
fn light_form_to_the_ultimate_finish_sends_1401_1403_n424_n425_and_the_blue_hour() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_12", 200.0);
    b.transform(0xDD);
    let s = b.of("1401", true);
    assert_eq!(s.state, StateId::Stylechange);
    let s = b.of("1403", true).clone();
    at(&s, 0.419_607);
    let e = b.of("1403", false).clone();
    ends(&s, &e, 1.3999);
    b.idle(30);
    b.combo();
    // the Light Form finishers (M_M411 .. M_M413) carry no BpEvent: nothing is sent in the combo
    assert!(b.bp.iter().all(|x| x.state != StateId::ComboLf), "{:?}", b.seqs());
    b.idle(30);
    b.transform(0xDF);
    // Double Form's StyleChangeEnd N401 has a BpEvent; no blueprint reads it (it is sent: KH3 sends it too)
    assert!(b.bp.iter().any(|x| x.seq == "N401" && x.begin));
    b.idle(70);
    assert_eq!(b.sora.combat().situation.slot_b.map(|e| e.kind), Some(0xE1));
    b.finish();
    let s = b.of("N424", true).clone();
    assert_eq!((s.state, s.clip), (StateId::Stylefinish, "N424"));
    let s = b.of("N425", true).clone();
    assert_eq!(s.clip, "N425");
    at(&s, 0.0);
    let e = b.of("N425", false).clone();
    assert!(e.step > s.step + 150, "N425's window lasts 3.3 s: {} -> {}", s.step, e.step);
    assert_eq!(b.fsg.iter().map(|f| f.1).collect::<Vec<_>>(), [true, false], "vw_so220_fin_fsg0 spawned at N425's start, faded at its end: {:?}", b.fsg);
    assert_eq!(b.fsg[0].0, s.step);
    assert_eq!(b.fsg[1].0, e.step);
    b.idle(20);
    assert_eq!(b.sora.combat().form.cmd, 0);
    assert!(b.sora.combat().bp_events.open.is_empty() && b.sora.combat().bp_events.blue_hour.is_none());
    b.dump("light_double");
}

#[test]
fn rage_form_sends_d401_d417_d418() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_00", 150.0);
    let max = b.sora.status().max_hp();
    let mut tries = 0;
    while !b.sora.combat().situation.has(0x8B) && tries < 300 {
        b.sora.status_mut().hp = max;
        b.step(&[]);
        b.sora.status_mut().hp = 1;
        b.step(&[]);
        tries += 1;
    }
    assert!(b.sora.combat().situation.has(0x8B), "Rage Form offered");
    b.idle(60);
    b.step(&[PadButton::SITUATION]);
    b.idle(200);
    assert_eq!(b.sora.combat().form.cmd, 0x8B);
    let s = b.of("D401", true).clone();
    assert_eq!((s.state, s.clip), (StateId::Stylechange, "StyleChange13"));
    b.combo();
    let s = b.of("D417", true).clone();
    assert_eq!(s.state, StateId::ComboAn);
    // three Riskcharges (D419: no BpEvent), then FINISH_AN
    for _ in 0..3 {
        b.idle(60);
        b.sora.debug_add_style(100.0);
        b.idle(90);
        b.finish();
    }
    assert!(!b.bp.iter().any(|x| x.seq == "D419"), "RISKCHARGE's M_D419 has no BpEvent");
    b.idle(60);
    b.sora.debug_add_style(100.0);
    b.idle(90);
    b.finish();
    let s = b.of("D418", true).clone();
    assert_eq!((s.state, s.clip), (StateId::Stylefinish, "D418"));
    b.dump("rage");
}

#[test]
fn second_form_sends_its_windows_as_kh3() {
    // (AUDIT-C, 2026-11-05) no longer held back: Second Form's StyleChange 1401 window is sent, and closed.
    let mut b = Bench::new("WEP_KEYBLADE_SO_00", 200.0);
    b.transform(0x94);
    b.idle(30);
    b.combo();
    assert!(b.bp.iter().any(|x| x.seq == "1401" && x.begin), "1401 start: {:?}", b.seqs());
    assert!(b.bp.iter().any(|x| x.seq == "1401" && !x.begin), "1401 end: {:?}", b.seqs());
}

/// (AUDIT-C) BP_TresPlayerStateEvent_motion_notify's clips (no form held): 0002, 1050 (the dive-fall attack, its camera
/// DiveFallAttack_camera), 0107 and 1176 (the rail slide's special, its camera specialRail). Each played on the FullBody
/// slot (`debug_play_anim`): the window opens at the clip's start and closes at its notify end (sora_anim.rs).
#[test]
fn motion_notify_clips_send_their_windows() {
    let mut b = Bench::new("WEP_KEYBLADE_SO_00", 600.0);
    b.idle(30);
    // the logical rows that play them (sora_anim.rs: WalkLoopN, DiveFallAttack, ClimbingH, RailSlideStartS)
    for (name, seq, end) in [("WalkLoopN", "0002", 1.066_666_7), ("DiveFallAttack", "1050", 2.166_666_7), ("ClimbingH", "0107", 0.533_333_4), ("RailSlideStartS", "1176", 0.8)] {
        let anim = kh_core::AnimId::from_name(name).unwrap_or_else(|| panic!("{name} is a row"));
        b.sora.debug_play_anim(anim, kh_core::PlayArgs::mode(2, -1.0));
        b.idle(((end + 0.3) / DT) as usize);
        let s = b.of(seq, true).clone();
        assert_eq!(s.clip, name);
        at(&s, 0.0);
        let e = b.of(seq, false).clone();
        ends(&s, &e, end);
        b.idle(30);
    }
    b.dump("motion_notify");
}
