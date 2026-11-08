//! (AUDIT-A CAMS) Hero's Origin's and Grand Chef's camera blueprints (tate_finish / Lemylkey: kh-camera forms_shield.rs) END
//! TO END: the real core (Sora with the level-99 loadout and the keyblade: offer -> CHANGE_SH / CHANGE_SH2 (StyleChange11 /
//! 19) -> the COMBO_SH combo (B417) -> one block and the grown attack 0x55 (B418) -> two blocks and 0x56 (B426) -> FINISH_SH
//! (B423 / B424 / the ride, or the pan's J423) -> the end), the hosts' entry: AUDIT-C's `form_bp::FormBpCameras::step` with
//! the core's events and `command` = `combat().form.command_in_action` (kh-er cam.rs and the effects harness do the same),
//! over a real field camera. Every camera start / end of the two blueprints is written to
//! build/sora_tmp/weapons/AUDIT-A/CAMS/logs/cams_shield_{sh,sh2}.log with the step, state and Sora's clip.

use glam::Vec3;
use kh_camera::camera_anim::AnimView;
use kh_camera::form_bp::{FormBpCameras, FormBpFrame, PerfCamera};
use kh_camera::{CameraInput, CameraRig, CameraTarget, TargetState, CAMERA_PARAMS};
use kh_core::enums::ETresDamageKind;
use kh_core::states::HitOnPlayer;
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
        // the whole level-99 loadout, its equipped abilities too (kh-host audit_a::Session's rig: the shield guard needs them)
        let value = cell(&row[1]).unwrap();
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
    rig: Option<CameraRig>,
    form_bp: FormBpCameras,
    step: u32,
    /// (step, line): the blueprints' log and the BpEvents / specific actions the core sent.
    log: Vec<(u32, String)>,
    /// Steps the layer's view was shown.
    shown: u32,
    /// The buttons held in the step before (the pad's edges).
    held: PadButtons,
}

impl Bench {
    fn new(key: &str) -> Bench {
        let origin = Vec3::new(0.0, 0.0, Sora::origin_height_on_floor(&SORA_PARAMS));
        let sora = Sora::new(SORA_PARAMS, Pose { position: origin, yaw: 0.0 }).with_tuning(Tuning::GAME_EXACT).with_status(holding(key));
        let mut host = RecordingHost::new();
        host.targets.push(TargetInfo { id: TargetId(1), position: Vec3::new(250.0, 0.0, 0.0), center: Vec3::new(250.0, 0.0, 90.0), velocity: Vec3::ZERO, yaw: 180.0, radius: 40.0, half_height: 90.0, flags: TargetFlags(TargetFlags::ALIVE | TargetFlags::HITTABLE | TargetFlags::LOCKABLE | TargetFlags::ENEMY), hp: 1.0e9, hp_max: 1.0e9 });
        let mut b = Bench { sora, host, world: TestWorld::floor(), rig: None, form_bp: FormBpCameras::new(), step: 0, log: Vec::new(), shown: 0, held: PadButtons::NONE };
        b.idle(5);
        b
    }

    fn press(&mut self, held: &[PadButton]) -> StateId {
        let set = held.iter().fold(PadButtons::NONE, |all, b| all.with(*b));
        let input = Input::default().with_pad(PadState::from_held(set, self.held));
        self.held = set;
        let (sora, world) = (&mut self.sora, &self.world);
        let out = self.host.step(world, |h| sora.step_hosted(DT, &input, h));
        self.step += 1;
        let target = CameraTarget { position: out.position, yaw: out.yaw, velocity: out.displacement_velocity, state: TargetState::Other, collision_half_height: SORA_PARAMS.capsule_half_height };
        let rig = self.rig.get_or_insert_with(|| CameraRig::new(CAMERA_PARAMS, &target));
        let under = rig.step(DT, &CameraInput::default(), &target, None, &self.world);
        let field_fov = kh_camera::projection(&CAMERA_PARAMS).0;
        let frame = FormBpFrame { sora: (out.position, out.yaw), field: AnimView { location: under.position, rotation: under.rotation, fov: field_fov }, state: out.state, ultima: false, perf: PerfCamera::default() };
        // kh-er cam.rs / the harness: pawn+0xE2C before the step (ReceivePlayerBeginCommandAction)
        self.form_bp.command = self.sora.combat().form.command_in_action;
        let o = self.form_bp.step(DT, &out.events, &frame, Some(&self.world));
        if let Some(t) = o.takeover {
            rig.field.take_over(t.location, t.rotation);
        }
        if o.reset_field_camera {
            rig.field.reset(&target, true);
        }
        if o.view.is_some() && self.form_bp.shield.active() {
            self.shown += 1;
        }
        // the ride's RotTargets: where the view stands from his actor (the look point is his actor + his yaw x local offset)
        if let (Some(v), [name]) = (o.view, self.form_bp.shield.cameras().as_slice()) {
            if matches!(*name, "CUt02" | "Cut03") && self.step.is_multiple_of(20) {
                let line = format!("{name}: view {:.0} cm from his actor ({:.0}, {:.0}, {:.0}), pitch {:.1}", (v.location - out.position).length(), out.position.x, out.position.y, out.position.z, v.rotation.pitch);
                self.log.push((self.step, line));
            }
        }
        let clip = out.anim_ticked.body_samples()[0];
        for e in &out.events {
            match e.kind {
                kh_core::host::EventKind::AnimBpEvent { seq, param, begin } => self.log.push((self.step, format!("{:?} {}@{:.3}: core BpEvent {seq} param {param} {}", out.state, clip.0, clip.2, if begin { "begin" } else { "end" }))),
                kh_core::host::EventKind::PlayerSpecificAction { state, action, .. } => self.log.push((self.step, format!("{:?}: core PlayerSpecificAction {action} in {state:?}", out.state))),
                _ => {}
            }
        }
        for l in self.form_bp.log.drain(..) {
            if let Some(l) = l.strip_prefix("tate_finish/Lemylkey: ") {
                self.log.push((self.step, format!("{:?} {}@{:.3}: {l}", out.state, clip.0, clip.2)));
            }
        }
        out.state
    }

    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.press(&[]);
        }
    }

    fn until(&mut self, n: usize, f: impl Fn(&Bench) -> bool) -> bool {
        for _ in 0..n {
            self.press(&[]);
            if f(self) {
                return true;
            }
        }
        false
    }

    fn block(&mut self) {
        let (own, yaw) = (self.sora.output().position, self.sora.output().yaw);
        let (s, c) = yaw.to_radians().sin_cos();
        let ahead = Vec3::new(c, s, 0.0);
        self.sora.take_hit(HitOnPlayer { direction: -ahead, causer_yaw: yaw + 180.0, ..HitOnPlayer::new(ETresDamageKind::Small, 30.0, TargetId(1), own + ahead * 150.0) });
        for k in 0..110 {
            let st = self.press(&[PadButton::GUARD]);
            if k == 2 {
                let line = format!("{st:?}: block, form {:#X} grow {}", self.sora.combat().form.cmd, self.sora.combat().form.grow);
                self.log.push((self.step, line));
            }
        }
    }

    fn grown_attack(&mut self) {
        assert!(self.until(200, |b| matches!(b.sora.output().state, StateId::Idle | StateId::Run)), "back on his feet");
        self.press(&[PadButton::ATTACK]);
        let line = format!("grown attack: history {:?} grow {}", self.sora.combat().history.last(), self.sora.combat().form.grow);
        self.log.push((self.step, line));
        self.idle(300);
    }

    fn has(&self, what: &str) -> bool {
        self.log.iter().any(|l| l.1.contains(what))
    }

    fn write(&self, name: &str) {
        let dir = format!("{}/../../build/sora_tmp/weapons/AUDIT-A/CAMS/logs", env!("CARGO_MANIFEST_DIR"));
        std::fs::create_dir_all(&dir).ok();
        let text: Vec<String> = self.log.iter().map(|(s, l)| format!("step {s:5} t {:7.3}: {l}", *s as f32 * DT)).collect();
        std::fs::write(format!("{dir}/{name}"), text.join("\n") + "\n").ok();
    }
}

/// The whole session of one form (Counter Shield 0x89 / Frying Pan 0x91).
fn session(key: &str, cmd: i32, log: &str) -> Bench {
    let mut b = Bench::new(key);
    b.sora.debug_add_style(100.0);
    b.idle(15);
    b.press(&[PadButton::SITUATION]);
    assert!(b.until(400, |b| b.sora.output().state != StateId::Stylechange && b.sora.combat().form.cmd != 0), "the form starts");
    assert_eq!(b.sora.combat().form.cmd, cmd);
    b.idle(30);
    // the combo to its finisher (B417)
    for k in 0..400 {
        let s = if k % 2 == 0 { b.press(&[PadButton::ATTACK]) } else { b.press(&[]) };
        if k > 40 && matches!(s, StateId::Idle | StateId::Run) {
            break;
        }
    }
    b.idle(90);
    // one block -> 0x55 (B418); two blocks -> 0x56 (B426)
    for _ in 0..40 {
        b.press(&[PadButton::GUARD]);
    }
    b.block();
    b.idle(30);
    b.grown_attack();
    for _ in 0..40 {
        b.press(&[PadButton::GUARD]);
    }
    b.block();
    b.block();
    b.idle(60);
    b.grown_attack();
    // FINISH_SH, its extra pressed whenever offered
    b.press(&[PadButton::SITUATION]);
    for k in 0..3000 {
        let press = b.sora.combat().situation.extra.is_some() && k % 2 == 0;
        let s = if press { b.press(&[PadButton::SITUATION]) } else { b.press(&[]) };
        if k > 30 && b.sora.finish().is_none() && !matches!(s, StateId::Stylefinish | StateId::StylefinishSh) {
            break;
        }
    }
    b.idle(120);
    b.write(log);
    assert_eq!(b.sora.combat().form.cmd, 0, "the finish ended the form");
    assert!(!b.form_bp.shield.active(), "nothing of the blueprints left: {:?}", b.form_bp.shield.cameras());
    b
}

#[test]
fn counter_shield_cameras_follow_tate_finish() {
    let b = session("WEP_KEYBLADE_SO_01", 0x89, "cams_shield_sh.log");
    for what in ["start SH_tate_start", "core BpEvent B417 param 0 begin", "start none RotTarget 600", "core BpEvent B418 param 2 begin", "start BIG_PUNCH ", "start BIG_PUNCH02 BIG_PUNCH02", "game speed x0.1", "game speed x1 in 0.2", "start BIG_PUNCH 100_PUNCH01", "start BIG_PUNCH02 100_PUNCH02", "shake shake_bibibi_8sec", "game speed x0.05", "core PlayerSpecificAction 0 in StylefinishSh", "start none RotTarget 450", "start SH_tate_End SH_tate_cut02", "start CUt02", "core PlayerSpecificAction 64 in StylefinishSh", "specific action 64"] {
        assert!(b.has(what), "{what} missing: see build/sora_tmp/weapons/AUDIT-A/CAMS/logs/cams_shield_sh.log");
    }
    assert!(!b.has("LemyKey"), "Lemylkey films nothing of Hero's Origin");
    assert!(b.shown > 300, "the layer's view was shown {} steps", b.shown);
}

#[test]
fn frying_pan_cameras_follow_lemylkey() {
    let b = session("WEP_KEYBLADE_SO_10", 0x91, "cams_shield_sh2.log");
    for what in ["core BpEvent J401 param 0 begin", "start LemyKey_Finish_Cam LemyKey_Change_Cam", "ResetTresCameraExceptActive", "core BpEvent J423 param 0 begin", "start LemyKey_Finish_Cam LemyKey_Finish_Cam", "core PlayerSpecificAction 0 in StylefinishSh", "core PlayerSpecificAction 64 in StylefinishSh"] {
        assert!(b.has(what), "{what} missing: see build/sora_tmp/weapons/AUDIT-A/CAMS/logs/cams_shield_sh2.log");
    }
    assert!(!b.has("start SH_tate_start"), "CHANGE_SH2 (145) is not tate_finish's command");
}
