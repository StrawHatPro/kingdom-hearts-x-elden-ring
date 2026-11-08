//! (AUDIT-B F-CLAWS) Happy Gear's and Crystal Snow's camera blueprints (mi_key / fz_key, kh-camera forms_claws.rs) END TO
//! END: the real core (Sora with the level-99 loadout, the forms' sessions offer -> transformation -> combo -> second stage
//! -> combo -> finisher -> end) -> its events (`AnimBpEvent` of kh-core states/bp_event.rs, `StateChange`) -> the hosts' one
//! entry `form_bp::FormBpCameras::step` (kh-er cam.rs and the effects harness call it so). Every camera start / end is
//! written to build/sora_tmp/weapons/AUDIT-B/F-CLAWS/traces/claws_cameras.trace.

use kh_camera::camera_anim::AnimView;
use kh_camera::form_bp::{FormBpCameras, FormBpFrame, PerfCamera};
use kh_camera::Rot;
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, Vec3, SORA_PARAMS};

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

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    cams: FormBpCameras,
    trace: Vec<String>,
    said: usize,
    step: u32,
    /// Steps a claws camera was drawn on.
    shown: u32,
}

impl Bench {
    fn new(key: &str) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding(key));
        let mut host = RecordingHost::new();
        host.targets.push(TargetInfo { id: TargetId(1), position: Vec3::new(200.0, 0.0, 0.0), center: Vec3::new(200.0, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 });
        let mut b = Bench { sora, host, world: TestWorld::floor(), cams: FormBpCameras::new(), trace: vec![format!("# {key}")], said: 0, step: 0, shown: 0 };
        b.idle(5);
        b
    }

    fn press(&mut self, held: &[PadButton]) -> StateId {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input::default().with_pad(PadState::from_held(set, PadButtons::NONE));
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        self.step += 1;
        let frame = FormBpFrame { sora: (out.position, out.yaw), field: AnimView { location: out.position + Vec3::new(-400.0, 0.0, 200.0), rotation: Rot::new(-10.0, 0.0, 0.0), fov: 67.0 }, state: out.state, ultima: false, perf: PerfCamera::default() };
        let o = self.cams.step(DT, &out.events, &frame, Some(&self.world));
        if self.cams.claws.active() {
            self.shown += 1;
        }
        let clip = out.anim_ticked.body_samples()[0].0;
        for e in out.events.iter().filter(|e| matches!(e.kind, kh_core::host::EventKind::AnimBpEvent { .. })) {
            self.trace.push(format!("step {:5} {:?} {clip} event {:?}", self.step, out.state, e.kind));
        }
        for l in &self.cams.claws.log[self.said..] {
            self.trace.push(format!("step {:5} {:?} {clip} camera {l} | view {:?} reset {} takeover {}", self.step, out.state, o.view.map(|v| (v.location.round(), v.fov.round())), o.reset_field_camera, o.takeover.is_some()));
        }
        self.said = self.cams.claws.log.len();
        out.state
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.press(&[]);
        }
    }

    fn transform(&mut self, cmd: i32) {
        self.sora.debug_add_style(100.0);
        self.idle(15);
        for _ in 0..120 {
            if self.sora.combat().situation.list.iter().any(|e| e.kind == cmd && e.appear <= 0.0 && e.start_delay <= 0.0) {
                break;
            }
            self.press(&[]);
        }
        self.trace.push(format!("# transform {cmd:#X}"));
        self.press(&[PadButton::SITUATION]);
        for _ in 0..600 {
            if self.press(&[]) != StateId::Stylechange && self.sora.combat().form.cmd == cmd {
                break;
            }
        }
        assert_eq!(self.sora.combat().form.cmd, cmd, "the form {cmd:#X}");
        self.idle(30);
    }

    fn combo(&mut self) {
        self.trace.push("# combo".to_string());
        for k in 0..900 {
            let s = if k % 2 == 0 { self.press(&[PadButton::ATTACK]) } else { self.press(&[]) };
            if k > 30 && matches!(s, StateId::Idle | StateId::Run | StateId::Jump) {
                break;
            }
        }
        self.idle(40);
    }

    fn finish(&mut self) {
        self.trace.push("# finish".to_string());
        self.press(&[PadButton::SITUATION]);
        for _ in 0..1500 {
            self.press(&[]);
            if self.sora.combat().form.cmd == 0 && self.sora.finish().is_none() {
                break;
            }
        }
        self.idle(60);
    }

    fn started(&self) -> Vec<String> {
        self.cams.claws.log.iter().filter_map(|l| l.strip_prefix("start ").map(str::to_string)).collect()
    }

    fn write(&self, name: &str) {
        let dir = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../build/sora_tmp/weapons/AUDIT-B/F-CLAWS/traces"));
        std::fs::create_dir_all(&dir).ok();
        std::fs::write(dir.join(name), self.trace.join("\n") + "\n").ok();
    }
}

fn tables() -> bool {
    let ok = kh_camera::camera_anim::camera_anim("YoYO_spiral_shredder").is_some();
    if !ok {
        eprintln!("SKIPPED: no generated camera tables (node tools/party_team_pack.js --cameras)");
    }
    ok
}

#[test]
fn happy_gear_cameras_through_the_real_core() {
    if !tables() {
        return;
    }
    let mut b = Bench::new("WEP_KEYBLADE_SO_05");
    b.transform(0x8A);
    b.combo();
    b.transform(0x82);
    b.combo();
    b.idle(10);
    b.finish();
    b.write("claws_cameras_mi.trace");
    let started = b.started();
    for want in ["Tsume_Start_Cmaera Tsume_Start", "Tornado_crusher mi_Wscissors", "YoYo_Start_Cmaera Yo-Yo_Start", "mi_camera_walk_the_dog YoYO_spiral_shredder"] {
        assert!(started.iter().any(|s| s == want), "{want} started: {started:?}\n{}", b.trace.join("\n"));
    }
    assert!(!b.cams.claws.active(), "every camera ended");
}

#[test]
fn crystal_snow_cameras_through_the_real_core() {
    if !tables() {
        return;
    }
    let mut b = Bench::new("WEP_KEYBLADE_SO_06");
    b.transform(0x90);
    b.combo();
    b.transform(0x85);
    b.combo();
    b.idle(10);
    b.finish();
    b.write("claws_cameras_fz.trace");
    let started = b.started();
    for want in ["Po_2Shot__Start_Cmaera fz-key_change", "fz_finish02 Fz_FInish_Cam02", "fz_finish01 Fz_FInish_Cam01"] {
        assert!(started.iter().any(|s| s == want), "{want} started: {started:?}\n{}", b.trace.join("\n"));
    }
    // 7423's param-0 window cut Cam01 away (Cam02 shows on)
    assert!(b.cams.claws.log.iter().any(|l| l.contains("EndTresCamera(Fz_FInish_Cam01)")), "{}", b.trace.join("\n"));
    assert!(!b.cams.claws.active(), "every camera ended");
}
