//! (AUDIT-B F-SHIP) Wheel of Fate's camera blueprint (ca_key, kh-camera forms_ship.rs) END TO END: the real core (Sora with
//! the level-99 loadout and Wheel of Fate: offer -> StyleChange14 -> COMBO_HL -> StyleChange15 -> COMBO_FL -> FINISH_FL ->
//! the end) -> its events (`AnimBpEvent` of kh-core states/bp_event.rs, `StateChange`) -> the hosts' one entry
//! `form_bp::FormBpCameras::step` (kh-er cam.rs and the effects harness call it so). Every camera start / end and the camera
//! path of every step it shows are written to build/sora_tmp/weapons/AUDIT-B/F-SHIP/traces/ship_cameras.trace.

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
    /// (step, camera name) of every start.
    starts: Vec<(u32, String)>,
}

impl Bench {
    fn new() -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding("WEP_KEYBLADE_SO_09"));
        let mut host = RecordingHost::new();
        host.targets.push(TargetInfo { id: TargetId(1), position: Vec3::new(200.0, 0.0, 0.0), center: Vec3::new(200.0, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e6, hp_max: 1.0e6 });
        let mut b = Bench { sora, host, world: TestWorld::floor(), cams: FormBpCameras::new(), trace: vec!["# Wheel of Fate (WEP_KEYBLADE_SO_09): ca_key, step / state / clip / camera / view (location, rotation, fov)".to_string()], said: 0, step: 0, starts: Vec::new() };
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
        let clip = out.anim_ticked.body_samples()[0].0;
        for l in &self.cams.log[self.said..] {
            if let Some(name) = l.strip_prefix("ca_key: start ") {
                self.starts.push((self.step, name.to_string()));
            }
            self.trace.push(format!("step {:5} {:?} {clip} camera {l} | reset {}", self.step, out.state, o.reset_field_camera));
        }
        self.said = self.cams.log.len();
        if self.cams.ship.active() {
            if let Some(v) = o.view {
                self.trace.push(format!("view {:5} {clip}@{:.3} {:?} loc ({:.1}, {:.1}, {:.1}) rot ({:.1}, {:.1}, {:.1}) fov {:.1} sora ({:.1}, {:.1}, {:.1}) yaw {:.1}", self.step, out.anim_ticked.time, self.cams.ship.cameras(), v.location.x, v.location.y, v.location.z, v.rotation.pitch, v.rotation.yaw, v.rotation.roll, v.fov, out.position.x, out.position.y, out.position.z, out.yaw));
            }
        }
        out.state
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.press(&[]);
        }
    }

    fn transform(&mut self, cmd: i32) {
        self.sora.debug_add_style(100.0);
        self.idle(20);
        assert!(self.sora.combat().situation.has(cmd), "{cmd:#x} offered");
        self.press(&[PadButton::SITUATION]);
        for _ in 0..600 {
            if self.press(&[]) != StateId::Stylechange && self.sora.combat().form.cmd == cmd {
                return;
            }
        }
        panic!("no end of STYLECHANGE");
    }

    fn combo(&mut self) {
        for k in 0..1200 {
            let s = if k % 2 == 0 { self.press(&[PadButton::ATTACK]) } else { self.press(&[]) };
            if k > 30 && matches!(s, StateId::Idle | StateId::Run | StateId::Jump) {
                break;
            }
        }
        self.idle(60);
    }

    fn place(&mut self, ahead: f32) {
        let p = self.sora.output().position;
        let yaw = self.sora.output().yaw.to_radians();
        let x = Vec3::new(p.x + yaw.cos() * ahead, p.y + yaw.sin() * ahead, 0.0);
        let t = &mut self.host.targets[0];
        (t.position, t.center) = (x, x + Vec3::Z * 90.0);
    }
}

#[test]
fn wheel_of_fate_cameras_follow_the_ca_key_blueprint() {
    let mut b = Bench::new();
    b.transform(0x8C);
    b.idle(120);
    b.place(200.0);
    b.combo();
    b.idle(60);
    b.transform(0x8D);
    b.idle(120);
    b.place(200.0);
    b.combo();
    b.idle(60);
    b.place(200.0);
    b.press(&[PadButton::SITUATION]);
    for _ in 0..1200 {
        if b.press(&[]) != StateId::Stylefinish && b.sora.combat().form.cmd == 0 {
            break;
        }
    }
    b.idle(120);
    let dir = format!("{}/../../build/sora_tmp/weapons/AUDIT-B/F-SHIP/traces", env!("CARGO_MANIFEST_DIR"));
    std::fs::create_dir_all(&dir).ok();
    std::fs::write(format!("{dir}/ship_cameras.trace"), b.trace.join("\n") + "\n").ok();
    let names: Vec<&str> = b.starts.iter().map(|s| s.1.as_str()).collect();
    assert_eq!(names, ["YoYo_Start_Cmaera", "back_camera", "halberd_combo_finish", "Tsume_Start_Cmaera", "back_camera", "ca_flag_combo_finish", "ca_flag_combo_finish"], "the ca_key cameras in KH3's order");
    assert!(!b.cams.ship.active(), "every camera ended with the form");
}
