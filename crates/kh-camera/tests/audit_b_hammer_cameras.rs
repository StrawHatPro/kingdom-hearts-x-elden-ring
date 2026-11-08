//! (AUDIT-B F-HAMMER) The hammer forms' camera blueprints end to end: the real core (kh-core's Sora with Favorite Deputy /
//! Classic Tone held, the offer, the transformations, the combos, FINISH_DR) publishes its clips' BpEvents
//! (`EventKind::AnimBpEvent`: kh-core `states::bp_event::emit`, Drill Dive's `rush::on_notify`), ONCE each, and kh-camera's `forms_hammer::HammerCameras` runs
//! the two blueprints on them as kh-er's cam.rs does. The trace (per step: state, clip, BpEvents, the cameras running) is
//! written to build/sora_tmp/weapons/AUDIT-B/F-HAMMER/traces/cam_trace_<keyblade>.txt.

use kh_camera::camera_anim::{camera_anim, AnimView};
use kh_camera::form_bp::PerfCamera;
use kh_camera::forms_hammer::{self as fh, HammerCameras, HammerFrame};
use kh_core::host::EventKind;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_camera::Rot;
use kh_core::{Input, Loadout, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};
use std::fmt::Write as _;

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

/// One step: (state, clip, its BpEvents (sequence, begin), the anims running after it, a view drawn).
type Row = (StateId, &'static str, Vec<(&'static str, bool)>, Vec<&'static str>, bool);

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    cams: HammerCameras,
    classic_tone: bool,
    /// Every step: (state, clip, the BpEvents, the anims running after the step, a view drawn).
    log: Vec<Row>,
}

impl Bench {
    fn new(key: &str) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding(key));
        let mut host = RecordingHost::new();
        host.targets.push(dummy(1, 200.0));
        let mut b = Bench { sora, host, world: TestWorld::floor(), cams: HammerCameras::new(), classic_tone: key == fh::CLASSIC_TONE, log: Vec::new() };
        b.idle(5);
        b
    }

    fn step(&mut self, held: &[PadButton]) -> StateId {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input::default().with_pad(PadState::from_held(set, PadButtons::NONE));
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        // what kh-er cam.rs hands the blueprints: his feet and yaw, the view under (here a fixed field view), his state
        let field = AnimView { location: out.position + Vec3::new(-400.0, 0.0, 200.0), rotation: Rot::default(), fov: 100.0 };
        let frame = HammerFrame { sora: (out.position, out.yaw), field, state: out.state, classic_tone: self.classic_tone, perf: PerfCamera::default() };
        let shown = self.cams.step(DT, &out.events, &frame, Some(&self.world)).view.is_some();
        let bp: Vec<(&'static str, bool)> = out.events.iter().filter_map(|e| match e.kind {
            EventKind::AnimBpEvent { seq, begin, .. } => Some((seq, begin)),
            _ => None,
        }).collect();
        self.log.push((out.state, out.anim.name.name(), bp, self.cams.anims(), shown));
        out.state
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.step(&[]);
        }
    }

    fn transform(&mut self) {
        self.sora.debug_add_style(100.0);
        self.idle(15);
        self.step(&[PadButton::SITUATION]);
        assert_eq!(self.step(&[]), StateId::Stylechange);
        for _ in 0..600 {
            if self.step(&[]) != StateId::Stylechange {
                return;
            }
        }
        panic!("STYLECHANGE did not end");
    }

    fn combo(&mut self) {
        for k in 0..1200 {
            let s = if k % 2 == 0 { self.step(&[PadButton::ATTACK]) } else { self.step(&[]) };
            if k > 30 && matches!(s, StateId::Idle | StateId::Run | StateId::Jump) {
                break;
            }
        }
    }

    fn finish(&mut self) {
        self.step(&[PadButton::SITUATION]);
        for _ in 0..3000 {
            let s = self.step(&[]);
            if self.sora.finish().is_none() && !matches!(s, StateId::Stylefinish | StateId::StylefinishDr) {
                break;
            }
        }
    }

    /// The anims shown, in order of their first step, from step `from`.
    fn shown_from(&self, from: usize) -> Vec<&'static str> {
        let mut out = Vec::new();
        for (_, _, _, anims, _) in &self.log[from..] {
            for a in anims {
                if !out.contains(a) {
                    out.push(*a);
                }
            }
        }
        out
    }

    fn bp_events(&self, from: usize) -> Vec<(&'static str, bool)> {
        self.log[from..].iter().flat_map(|l| l.2.iter().copied()).collect()
    }

    /// The step of the first BpEvent of `seq` (begin or end) from `from`.
    fn at(&self, from: usize, seq: &str, begin: bool) -> Option<usize> {
        (from..self.log.len()).find(|i| self.log[*i].2.contains(&(seq, begin)))
    }

    /// Every BpEvent of a step comes once (one sender per clip).
    fn assert_once(&self) {
        for (i, l) in self.log.iter().enumerate() {
            for (k, e) in l.2.iter().enumerate() {
                assert!(!l.2[k + 1..].contains(e), "step {i}: {e:?} sent twice: {:?}", l.2);
            }
        }
    }

    fn write(&self, name: &str) {
        let mut t = String::new();
        let _ = writeln!(t, "# (AUDIT-B F-HAMMER) {name}: step, state, clip, BpEvents (+begin / -end), the blueprints' anims running, a view drawn");
        for (i, (state, clip, bp, anims, shown)) in self.log.iter().enumerate() {
            let ev: Vec<String> = bp.iter().map(|(s, b)| format!("{}{s}", if *b { '+' } else { '-' })).collect();
            let _ = writeln!(t, "{i}\t{state:?}\t{clip}\t{}\t{}\t{}", ev.join(" "), anims.join(" "), u8::from(*shown));
        }
        let _ = writeln!(t, "# the blueprints' log:");
        for l in &self.cams.log {
            let _ = writeln!(t, "{l}");
        }
        let dir = format!("{}/../../build/sora_tmp/weapons/AUDIT-B/F-HAMMER/traces", env!("CARGO_MANIFEST_DIR"));
        if std::fs::create_dir_all(&dir).is_ok() {
            let _ = std::fs::write(format!("{dir}/cam_trace_{name}.txt"), t);
        }
    }
}

fn tables() -> bool {
    let ok = camera_anim(fh::HAMMER_START).is_some();
    if !ok {
        eprintln!("SKIPPED: no generated camera tables (node tools/party_team_pack.js --cameras)");
    }
    ok
}

/// Favorite Deputy: 3401 -> toy_key_hammer_start until the end of 3401; Giant Stamp 3416 -> big_stump; 6401 ->
/// drill_start01; FINISH_DR 6421 -> drill_finish01, 6423 -> drill_finish02 (open ground: the traces are clear).
#[test]
fn favorite_deputy_s_cameras_from_the_real_core() {
    if !tables() {
        return;
    }
    let mut b = Bench::new("WEP_KEYBLADE_SO_03");
    let from = b.log.len();
    b.transform();
    let begin = b.at(from, "3401", true).expect("3401's BpEvent begins");
    let end = b.at(from, "3401", false).expect("and ends");
    assert!(b.log[begin].3.contains(&fh::HAMMER_START), "the transformation camera at 3401: {:?}", b.log[begin]);
    assert!((begin..end).all(|i| b.log[i].3.contains(&fh::HAMMER_START)), "held for the whole BpEvent");
    assert!(!b.log[end].3.contains(&fh::HAMMER_START), "cut at its end");
    b.idle(30);
    let from = b.log.len();
    b.combo();
    let stamp = b.at(from, "3416", true).expect("Giant Stamp's BpEvent");
    assert!(b.log[stamp].3.contains(&fh::BIG_STUMP), "{:?}", b.log[stamp]);
    b.idle(60);
    let from = b.log.len();
    b.transform();
    let start = b.at(from, "6401", true).expect("6401's BpEvent");
    assert!(b.log[start].3.contains(&fh::DRILL_START01), "{:?}", b.log[start]);
    b.idle(30);
    let from = b.log.len();
    b.combo();
    let combo = b.shown_from(from);
    b.idle(30);
    // FINISH_DR: put the dummy 6 m ahead (the warhead's catch)
    let (yaw, position) = (b.sora.output().yaw, b.sora.output().position);
    let a = yaw.to_radians();
    let at = position + Vec3::new(a.cos(), a.sin(), 0.0) * 600.0;
    (b.host.targets[0].position, b.host.targets[0].center) = (Vec3::new(at.x, at.y, 0.0), Vec3::new(at.x, at.y, 90.0));
    let from = b.log.len();
    b.finish();
    let finish = b.shown_from(from);
    let events = b.bp_events(from);
    b.idle(30);
    b.write("favorite_deputy");
    b.assert_once();
    assert!(events.contains(&("6421", true)) && events.contains(&("6423", true)), "{events:?}");
    assert_eq!(finish, [fh::DRILL_FINISH01, fh::DRILL_FINISH02], "FINISH_DR's cameras (combo: {combo:?})");
    assert!(b.cams.anims().is_empty(), "all ended");
}

/// Classic Tone: K401 -> Timeless_Hunmer_Change_Cam, L401 -> Timeless_Drill_Change_Cam, L421 -> Timeless_Drill_Finish_Cam;
/// the toy key one films the shared Giant Stamp (3416) too.
#[test]
fn classic_tone_s_cameras_from_the_real_core() {
    if !tables() {
        return;
    }
    let mut b = Bench::new(fh::CLASSIC_TONE);
    let from = b.log.len();
    b.transform();
    let k = b.at(from, "K401", true).expect("K401's BpEvent");
    assert!(b.log[k].3.contains(&fh::TIMELESS_HAMMER_CHANGE), "{:?}", b.log[k]);
    assert!(b.at(from, "3401", true).is_none(), "Boom Hammer's StyleChange20 replaces 3401");
    b.idle(30);
    let from = b.log.len();
    b.combo();
    let combo = b.shown_from(from);
    b.idle(60);
    let from = b.log.len();
    b.transform();
    let l = b.at(from, "L401", true).expect("L401's BpEvent");
    assert!(b.log[l].3.contains(&fh::TIMELESS_DRILL_CHANGE), "{:?}", b.log[l]);
    b.idle(30);
    b.combo();
    b.idle(30);
    let from = b.log.len();
    b.finish();
    let finish = b.shown_from(from);
    b.idle(30);
    b.write("classic_tone");
    b.assert_once();
    assert!(combo.contains(&fh::BIG_STUMP) || combo.contains(&fh::BIG_STUMP02), "the shared Giant Stamp: {combo:?}");
    assert_eq!(finish, [fh::TIMELESS_DRILL_FINISH], "the Clock's camera");
    assert!(b.cams.anims().is_empty(), "all ended");
}
