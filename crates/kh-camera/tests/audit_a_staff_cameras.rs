//! (AUDIT-A STAFF) Mirage Staff's camera blueprint (Rapunzel_key, kh-camera forms_staff.rs) END TO END: the real core (Sora
//! with the level-99 loadout and Ever After: offer -> STYLECHANGE (StyleChange10 = A401) -> FINISH_WA (A417, the tower) -> the
//! end) -> the hosts' entry, AUDIT-C's `form_bp::FormBpCameras::step` with the core's events (BpEvents, state changes,
//! the specific actions) over a real field camera (kh-er cam.rs and the effects
//! harness call them so). The camera path of both is written to build/sora_tmp/weapons/AUDIT-A/STAFF/traces/
//! transform_camera.csv and finish_camera.csv (location / rotation relative to Sora's root and yaw at A401's start, and to the
//! tower's root and yaw).

use glam::Vec3;
use kh_camera::camera_anim::AnimView;
use kh_camera::form_bp::{FormBpCameras, FormBpFrame, PerfCamera};
use kh_camera::forms;
use kh_camera::forms_staff as fs;
use kh_camera::{CameraInput, CameraRig, CameraTarget, TargetState, CAMERA_PARAMS};
use kh_core::status::LoadoutValue;
use kh_core::test_world::TestWorld;
use kh_core::{Input, Loadout, PadButton, PadButtons, PadState, Pose, RecordingHost, Sora, StateId, Status, TargetFlags, TargetId, TargetInfo, Tuning, SORA_PARAMS};

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

/// The frame a path is written in: a root and a yaw.
#[derive(Clone, Copy)]
struct Frame {
    at: Vec3,
    yaw: f32,
}

impl Frame {
    fn local(&self, p: Vec3) -> Vec3 {
        let d = p - self.at;
        let (s, c) = self.yaw.to_radians().sin_cos();
        Vec3::new(d.x * c + d.y * s, -d.x * s + d.y * c, d.z)
    }
}

struct Bench {
    sora: Sora,
    host: RecordingHost,
    world: TestWorld,
    rig: Option<CameraRig>,
    form_bp: FormBpCameras,
    /// The last view location shown (the CSV's cut test).
    last: Option<Vec3>,
    step: u32,
    /// Every camera start / end of the blueprint: (step, line).
    log: Vec<(u32, String)>,
    /// The CSV being written: its frame, its start step, the rows.
    csv: Option<(Frame, u32, Vec<String>)>,
}

impl Bench {
    fn new() -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding("WEP_KEYBLADE_SO_04"));
        let mut host = RecordingHost::new();
        host.targets.push(TargetInfo { id: TargetId(1), position: Vec3::new(200.0, 0.0, 0.0), center: Vec3::new(200.0, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e9, hp_max: 1.0e9 });
        let mut b = Bench { sora, host, world: TestWorld::floor(), rig: None, form_bp: FormBpCameras::new(), last: None, step: 0, log: Vec::new(), csv: None };
        b.idle(5);
        b
    }

    fn press(&mut self, held: &[PadButton]) -> StateId {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input::default().with_pad(PadState::from_held(set, PadButtons::NONE));
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        self.step += 1;
        let target = CameraTarget { position: out.position, yaw: out.yaw, velocity: out.displacement_velocity, state: TargetState::Other, collision_half_height: SORA_PARAMS.capsule_half_height };
        let rig = self.rig.get_or_insert_with(|| CameraRig::new(CAMERA_PARAMS, &target));
        // kh-er cam.rs: the form's distance scale, the field camera, then the form cameras
        rig.field.params_mut().distance_scale = forms::field_distance_scale(rig.attack_camera().mode, kh_core::combat::forms::staff::camera_distance_scale(self.sora.combat()));
        let under = rig.step(DT, &CameraInput::default(), &target, None, &self.world);
        // kh-er cam.rs: AUDIT-C's state-event layer with the core's events (Rapunzel_key inside it)
        let field_fov = kh_camera::projection(&CAMERA_PARAMS).0;
        let frame = FormBpFrame { sora: (out.position, out.yaw), field: AnimView { location: under.position, rotation: under.rotation, fov: field_fov }, state: out.state, ultima: false, perf: PerfCamera::default() };
        let o = self.form_bp.step(DT, &out.events, &frame, Some(&self.world));
        if let Some(t) = o.takeover {
            rig.field.take_over(t.location, t.rotation);
        }
        let shake = o.shake;
        let clip = out.anim_ticked.body_samples()[0];
        for l in self.form_bp.log.drain(..) {
            if let Some(l) = l.strip_prefix("Rapunzel_key: ") {
                self.log.push((self.step, format!("{:?} {}@{:.3}: {l}", out.state, clip.0, clip.2)));
            }
        }
        if let Some((frame, from, rows)) = self.csv.as_mut() {
            let (view, fov, camera) = match o.view {
                Some(v) => (v, v.fov, self.form_bp.staff.running().map_or("(blend out)", |r| r.0)),
                None => (AnimView { location: under.position, rotation: under.rotation, fov: field_fov }, field_fov, "(field)"),
            };
            // a cut: the view jumps by more than a metre in one step (the layer's output carries no cut flag)
            let cut = self.last.is_some_and(|l: Vec3| (l - view.location).length() > 100.0);
            self.last = Some(view.location);
            let view = kh_camera::CameraView { position: view.location, rotation: view.rotation, ..under };
            let p = frame.local(view.position);
            let s = frame.local(out.position);
            rows.push(format!(
                "{:.4},{},{:?},{},{:.4},{},{},{:.2},{:.2},{:.2},{:.3},{:.3},{:.3},{:.2},{:.2},{:.2},{:.2},{:.3},{:.3}",
                (self.step - *from) as f32 * DT,
                self.step,
                out.state,
                clip.0,
                clip.2,
                camera,
                u8::from(cut),
                p.x,
                p.y,
                p.z,
                view.rotation.pitch,
                kh_camera::normalize_axis(view.rotation.yaw - frame.yaw),
                view.rotation.roll,
                fov,
                s.x,
                s.y,
                s.z,
                shake.rotation.x,
                shake.rotation.y
            ));
        }
        out.state
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.press(&[]);
        }
    }

    fn begin_csv(&mut self, frame: Frame) {
        self.csv = Some((frame, self.step, Vec::new()));
    }

    fn write_csv(&mut self, name: &str, about: &str) -> Vec<String> {
        let (_, _, rows) = self.csv.take().expect("a CSV");
        let dir = format!("{}/../../build/sora_tmp/weapons/AUDIT-A/STAFF/traces", env!("CARGO_MANIFEST_DIR"));
        std::fs::create_dir_all(&dir).ok();
        let head = format!("# {about}\nt,step,state,clip,clip_t,camera,cut,x,y,z,pitch,yaw,roll,fov_h,sora_x,sora_y,sora_z,shake_pitch,shake_yaw\n");
        std::fs::write(format!("{dir}/{name}"), head + &rows.join("\n") + "\n").ok();
        rows
    }
}

#[test]
fn mirage_staff_cameras_follow_the_rapunzel_key_blueprint() {
    let mut b = Bench::new();
    // ---- the offer and A401 (StyleChange10)
    b.sora.debug_add_style(100.0);
    b.idle(15);
    let start = Frame { at: b.sora.output().position, yaw: b.sora.output().yaw };
    b.begin_csv(start);
    b.press(&[PadButton::SITUATION]);
    for _ in 0..400 {
        if b.press(&[]) != StateId::Stylechange && b.sora.combat().form.cmd == kh_core::combat::forms::staff::CHANGE_WA {
            break;
        }
    }
    b.idle(30);
    let rows = b.write_csv("transform_camera.csv", "Mirage Staff's transformation (A401, StyleChange10): Rapunzel_key's camera over the field camera; x / y / z cm in Sora's frame at the offer (x forward, y right, z up), yaw relative to his yaw then; fov_h degrees");
    assert_eq!(b.sora.combat().form.cmd, 0x88, "Mirage Staff");
    let starts: Vec<&str> = b.log.iter().map(|l| l.1.as_str()).filter(|l| l.contains(": start ")).collect();
    assert!(starts.iter().any(|l| l.contains("A401") && l.ends_with(fs::START_ANIM)), "the open-ground transformation camera: {:?}", b.log);
    assert!(rows.iter().any(|r| r.contains(fs::START_ANIM)), "its path is in the CSV");
    assert!(!b.form_bp.staff.active(), "it ended with STYLECHANGE / its autoEnd");
    // ---- FINISH_WA (A417, the tower)
    b.idle(30);
    let said = b.log.len();
    b.press(&[PadButton::SITUATION]);
    let mut tower = None;
    let mut began = None;
    for _ in 0..3000 {
        let s = b.press(&[]);
        if let (None, Some(t)) = (tower, kh_core::combat::forms::staff::tower(b.sora.finish(), b.sora.combat())) {
            tower = Some(t);
            began = Some(b.step);
            b.begin_csv(Frame { at: t.position, yaw: t.yaw });
        }
        if began.is_some_and(|s0| b.step - s0 > 620) {
            break;
        }
        if s != StateId::Stylefinish && b.sora.finish().is_none() && tower.is_some() {
            break;
        }
    }
    let tower = tower.expect("FINISH_WA's tower");
    let rows = b.write_csv("finish_camera.csv", &format!("FINISH_WA (A417, the tower at ({:.1}, {:.1}, {:.1}) yaw {:.1}): Rapunzel_key's cameras; x / y / z cm in the tower's frame (x forward, y right, z up), yaw relative to the tower's; fov_h degrees", tower.position.x, tower.position.y, tower.position.z, tower.yaw));
    let began = began.unwrap();
    let finish: Vec<(u32, &str)> = b.log[said..].iter().map(|(s, l)| (*s, l.as_str())).collect();
    let at = |what: &str| finish.iter().find(|l| l.1.contains(what)).map(|l| (l.0 - began) as f32 * DT);
    let cut01 = at(&format!("start camera anim {}", fs::TOWER_CUT01)).expect("the open-ground tower cut");
    let cut02 = at(&format!("start camera anim {}", fs::TOWER_CUT02)).expect("Cut02");
    let beam = at("start beam").expect("the beam");
    let chakudan = at("start chakudan").expect("the chakudan");
    let end = at("end of A417").expect("the end of A417");
    assert!(cut01 < 2.0 * DT, "at A417's start: {cut01}");
    assert!((cut02 - 2.667).abs() < 0.1 && (beam - cut02 - 2.3).abs() < 0.1, "{cut02} {beam}");
    assert!((chakudan - beam - 0.72).abs() < 0.1, "a cycle + 0.2 + 0.5 s: {}", chakudan - beam);
    assert!((end - 9.5).abs() < 0.1, "the BpEvent's end: {end}");
    assert!(rows.len() > 500, "{} rows", rows.len());
    assert!(!b.form_bp.staff.active(), "nothing of the blueprint after the finish");
}
