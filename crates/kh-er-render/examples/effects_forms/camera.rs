//! (AUDIT-A HARNESS) `--game-camera` / `--form-camera`: THE PORT'S CAMERA STACK AS kh-er cam.rs RUNS IT (`KhCam::frame` /
//! `step` / `post`), on the harness's flat floor: the CameraRig (field + lock-on + the attack camera's rig modes, fed by the
//! core's `CameraLock` / `AttackCamera` / `AimMode` events), Mirage Staff's distance scale (`field_distance_scale`), the aim
//! overlay, KH3's attack camera modes 1 / 2 / 8 and the form cameras (`kh_camera::forms::select` + `FormCameras`:
//! ShooterMode, the multi-lock, Counter Shield's finish Fix and ride) with their cuts and blends, the port's shot camera, the
//! effects' camera shakes (the pack's host.json classes), KH3's lens (100 deg horizontal, or the form camera's).
//! Not here (no such thing in the harness): Elden Ring's camera and the blend to it, the party's team cameras, the
//! conversation camera, the boss finish's cuts, the outfit's camera height (boc), the right stick (the script has none).
//! One line per step: `cam: step N | mode M | eye (x, y, z) look (x, y, z) | yaw / pitch / roll | fov_h F | cut`.

use std::collections::HashMap;

use glam::{Vec2, Vec3};
use kh_camera::aim::AimOverlay;
use kh_camera::shake::{Oscillator, ShakeDef, ShakeSet};
use kh_camera::shot::ShotCamera;
use kh_camera::{CameraInput, CameraRig, CameraTarget, CameraView, Rot, TargetState, CAMERA_PARAMS};
use kh_core::{EventKind, Output, Sora, StateId, TargetId};
use kh_host::rig::{Rig, STEP};

/// kh-er cam.rs `FOV_KH3`: KH3's horizontal field of view (degrees).
pub const FOV_KH3: f32 = 100.0;

/// The view to draw this frame (KH = the harness's game space, cm): eye, look point, the axes (game space), FOV.
#[derive(Clone, Copy, Debug)]
pub struct Shown {
    pub eye: Vec3,
    pub look: Vec3,
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    /// Vertical, radians (kh-er cam.rs `fov_vertical` of the horizontal one).
    pub fov_v: f32,
}

pub struct GameCamera {
    rig: Option<CameraRig>,
    aim: AimOverlay,
    shot: ShotCamera,
    forms: kh_camera::forms::FormCameras,
    forms_fov: Option<f32>,
    forms_label: Option<&'static str>,
    previous: CameraView,
    view: CameraView,
    relock_pending: bool,
    shakes: ShakeSet,
    defs: HashMap<String, ShakeDef>,
    mode: &'static str,
    cut: bool,
    before: Option<Output>,
    last_mode: &'static str,
    /// (AUDIT-C) Sora's formchange / finisher cameras of KH3's state-event blueprints (kh_camera::form_bp), as kh-er cam.rs.
    form_bp: kh_camera::form_bp::FormBpCameras,
    form_bp_fov: Option<f32>,
    form_bp_shake: kh_camera::shake::ShakeOffset,
    /// (AUDIT-B F-HAMMER) the hammer forms' camera blueprints (camera_toy_key / Timlesskey: kh_camera::forms_hammer), over
    /// the form_bp layer as kh-er cam.rs steps them
    hammer: kh_camera::forms_hammer::HammerCameras,
    hammer_fov: Option<f32>,
    /// (AUDIT-A WORLDTIME) KH3's world time as kh-er worldtime.rs keeps it (WorldSlow's table, tate_finish's hit-stops):
    /// its changes are logged (`cam: step N: (AUDIT-A WORLDTIME) ...`).
    world: kh_host::worldtime::WorldClock,
}

/// kh-er cam.rs `target_of`: what the camera is told about Sora.
fn target_of(out: &Output, before: &Output, sora: &Sora) -> CameraTarget {
    let mut target = CameraTarget {
        position: out.position,
        yaw: out.yaw,
        velocity: out.displacement_velocity,
        state: match out.state {
            StateId::Idle => TargetState::Idle,
            StateId::Run => TargetState::Run,
            StateId::Wallrun => TargetState::WallRun(0),
            StateId::RailSlide => TargetState::RailSlide,
            StateId::Slopeslide => TargetState::SlopeSlide,
            _ => TargetState::Other,
        },
        collision_half_height: sora.params().capsule_half_height,
    };
    if let kh_core::CameraState::WallRun(class) = sora.camera_state() {
        target.state = TargetState::WallRun(class);
    }
    if !matches!(target.state, TargetState::Idle | TargetState::Run | TargetState::Other) && target.velocity == Vec3::ZERO {
        target.velocity = (out.position - before.position) * 60.0;
    }
    target
}

/// kh-er cam.rs `fov_vertical`: the vertical FOV (radians) of a horizontal one (degrees) on KH3's 16:9 frame.
pub fn fov_vertical(horizontal_degrees: f32) -> f32 {
    2.0 * ((horizontal_degrees.to_radians() * 0.5).tan() / CAMERA_PARAMS.fov_aspect_ratio).atan()
}

impl GameCamera {
    /// The shake classes of the effect pack (kh-er cam.rs `KhCam::new`).
    pub fn new(pack: &std::path::Path) -> GameCamera {
        let mut defs = HashMap::new();
        if let Some(host) = std::fs::read_to_string(pack.join("vfx").join("host.json")).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) {
            if let Some(shakes) = host.get("camera_shakes").and_then(serde_json::Value::as_object) {
                let num = |v: &serde_json::Value, key: &str| v.get(key).and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
                let osc = |v: Option<&serde_json::Value>| v.map_or(Oscillator::default(), |v| Oscillator { amplitude: num(v, "amplitude"), frequency: num(v, "frequency"), start_at_zero: v.get("start_at_zero").and_then(serde_json::Value::as_bool).unwrap_or(false) });
                let three = |v: Option<&serde_json::Value>| [0, 1, 2].map(|k| osc(v.and_then(|v| v.get(k))));
                for (class, d) in shakes {
                    defs.insert(class.clone(), ShakeDef { name: class.clone(), duration: num(d, "duration"), blend_in: num(d, "blend_in"), blend_out: num(d, "blend_out"), rot: three(d.get("rot")), loc: three(d.get("loc")), fov: osc(d.get("fov")), single_instance: d.get("single_instance").and_then(serde_json::Value::as_bool).unwrap_or(false) });
                }
            }
        }
        println!("effects: (AUDIT-A) --game-camera: the port's camera stack (kh-er cam.rs); {} camera shake class(es) from the effect pack", defs.len());
        let nothing = CameraView { position: Vec3::ZERO, rotation: Rot::default(), rotation_target: Rot::default(), look_point: Vec3::ZERO, distance: 0.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
        GameCamera { rig: None, aim: AimOverlay::default(), shot: ShotCamera::default(), forms: kh_camera::forms::FormCameras::default(), forms_fov: None, forms_label: None, previous: nothing, view: nothing, relock_pending: false, shakes: ShakeSet::new(), defs, mode: "field", cut: false, before: None, last_mode: "", form_bp: kh_camera::form_bp::FormBpCameras::new(), form_bp_fov: None, form_bp_shake: kh_camera::shake::ShakeOffset::default() /* (AUDIT-C) */, hammer: kh_camera::forms_hammer::HammerCameras::new(), hammer_fov: None /* (AUDIT-B F-HAMMER) */, world: kh_host::worldtime::WorldClock::default() /* (AUDIT-A WORLDTIME) */ }
    }

    /// kh-er `KhCam::input_yaw` (the camera has the view at once here: no Elden Ring camera to blend from).
    /// (AUDIT-A WORLDTIME) The global game speed of the world clock's last step.
    pub fn world_rate(&self) -> f32 {
        self.world.rates().global
    }

    pub fn input_yaw(&self) -> Option<f32> {
        self.rig.as_ref().map(|_| self.view.rotation.yaw)
    }

    /// kh-er `KhCam::core_view`.
    pub fn core_view(&self) -> Option<kh_core::magic::View> {
        self.rig.as_ref().map(|_| kh_core::magic::View::from_camera(self.view.position, self.view.rotation.yaw, self.view.rotation.pitch, self.forms_fov.unwrap_or(FOV_KH3), 1280, 720))
    }

    /// One fixed step AFTER Sora's (kh-er `KhCam::frame` + `KhCam::step`).
    pub fn step(&mut self, r: &Rig, shakes: Vec<kh_host::vfx::Shake>) {
        let (out, sora) = (&r.out, &r.sora);
        let before = self.before.take().unwrap_or_else(|| out.clone());
        let dt = STEP * self.world_rate(); // (AUDIT-A WORLDTIME) kh-er cam.rs: the cameras on the world's clock (`dt x world_rate`)
        let dummy = r.dummy.map(|d| d.info);
        let point_of = |id: TargetId| dummy.filter(|d| d.id == id && id != TargetId::NONE).map(|d| d.position + Vec3::Z * d.half_height);
        // ---- frame: the camera made behind Sora (KH3's start: behind him) the first time it has the view
        if self.rig.is_none() {
            let target = target_of(out, &before, sora);
            let rig = CameraRig::new(CAMERA_PARAMS, &target);
            self.view = rig.view();
            self.previous = self.view;
            self.rig = Some(rig);
            self.relock_pending = true;
        }
        // ---- step
        for request in shakes {
            match self.defs.get(&request.class) {
                Some(def) => {
                    self.shakes.play(def, request.scale, request.duration);
                    println!("cam: step {}: camera shake {} x{:.2} played", out.step, request.class, request.scale);
                }
                None => println!("cam: camera shake class {} is not in the effect pack's host.json: not played", request.class),
            }
        }
        self.shakes.step(STEP); // (AUDIT-A WORLDTIME) real time, as kh-er cam.rs (its shakes before `dt x world_rate`)
        let Some(rig) = self.rig.as_mut() else { return };
        let target = target_of(out, &before, sora);
        if std::mem::take(&mut self.relock_pending) {
            let lock = sora.combat().lockon;
            if lock.locked && lock.current != TargetId::NONE {
                rig.set_lockon(&target, point_of(lock.current).map(|p| (lock.current.0, p)));
            }
        }
        for event in &out.events {
            match event.kind {
                EventKind::CameraLock { target: id } => rig.set_lockon(&target, point_of(id).map(|p| (id.0, p))),
                EventKind::AttackCamera { mode, restore } => rig.set_attack_mode(mode, restore),
                EventKind::AimMode { open: true, .. } => {
                    let aim = &sora.magic().aim;
                    let pick = |id: TargetId| (id != TargetId::NONE).then(|| point_of(id)).flatten();
                    let point = kh_camera::aim::start_point(pick(aim.saved_lock), pick(aim.saved_soft));
                    self.aim.open_toward(&target, &self.view, point);
                }
                EventKind::AimMode { open: false, reason } => {
                    self.aim.close();
                    match reason {
                        2 => self.shot.start(&self.view),
                        3 => {
                            self.shot.end();
                            rig.field.reset(&target, true);
                        }
                        1 => rig.field.reset(&target, true),
                        5 => rig.field.reset(&target, false),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        self.previous = self.view;
        let aiming = self.aim.active();
        let stick = Vec2::ZERO;
        let field_input = CameraInput::default();
        let lock_at = rig.locked().and_then(|id| point_of(TargetId(id)));
        rig.field.set_auto_camera(true);
        // (F-HUD) the distance scale of this step: the attack camera's modes, else the form's (Mirage Staff 1.35)
        let form_scale = kh_core::combat::forms::staff::camera_distance_scale(sora.combat());
        rig.field.params_mut().distance_scale = kh_camera::forms::field_distance_scale(rig.attack_camera().mode, form_scale);
        let under = rig.step(dt, &field_input, &target, lock_at, &r.world);
        let mut view = self.aim.step(dt, stick, Vec2::ZERO, &target, &r.world, &under);
        // (F-HUD) the attack camera's own cameras (modes 1 / 2 / 8) first, then the form cameras
        let selection = kh_camera::forms::select(sora, false, &point_of);
        let attack_own = selection.attack_own;
        let form_under = if attack_own && self.shot.active() { self.previous } else { view };
        let form_shown = self.forms.step(dt, selection.shot, &target, &form_under, stick, &r.world);
        self.forms_label = kh_camera::forms::mode_label(&selection, form_shown.is_some());
        self.cut = false;
        match form_shown {
            Some(shown) => {
                view = shown.view;
                self.forms_fov = Some(shown.fov);
                if shown.cut {
                    self.previous = shown.view;
                    self.cut = true;
                }
            }
            None => self.forms_fov = None,
        }
        let in_state = out.state == StateId::Shootflow;
        if self.shot.owns_view() && !in_state && before.state != StateId::Shootflow && sora.aim().mode != 2 {
            self.shot.end();
            rig.field.reset(&target, true);
        }
        if self.shot.active() && !attack_own {
            let locks: Vec<Vec3> = sora.aim().locks.iter().filter_map(|(id, _)| point_of(*id)).collect();
            let focus = (!locks.is_empty()).then(|| locks.iter().sum::<Vec3>() / locks.len() as f32);
            let wide = in_state && matches!(out.sub_step, 5 | 2 | 6);
            let below = if self.shot.owns_view() { rig.view() } else { view };
            view = self.shot.step(dt, &target, focus, wide, &r.world, &below);
        }
        // (AUDIT-C) the state-event blueprints' form cameras over the field / form cameras (kh-er cam.rs after the team layer)
        let field_fov = kh_camera::projection(&CAMERA_PARAMS).0;
        let frame = kh_camera::form_bp::FormBpFrame {
            sora: (out.position, out.yaw),
            field: kh_camera::camera_anim::AnimView { location: view.position, rotation: view.rotation, fov: field_fov },
            state: out.state,
            ultima: kh_core::combat::form::equipped_keyblade(sora.status()) == "WEP_KEYBLADE_SO_14",
            perf: kh_camera::form_bp::PerfCamera::default(),
        };
        self.form_bp.command = sora.combat().form.command_in_action; // (AUDIT-A CAMS) ReceivePlayerBeginCommandAction, as kh-er cam.rs
        self.form_bp.guns.weapon = kh_core::combat::form::weapon_type(sora.status()); // (AUDIT-A CAMS) BP_GetEquipWeaponID
        let bp_out = self.form_bp.step(dt, &out.events, &frame, Some(&r.world));
        self.form_bp_fov = None;
        if let Some(v) = bp_out.view {
            view.position = v.location;
            view.rotation = v.rotation;
            view.look_point = v.location + v.rotation.forward() * 300.0;
            self.form_bp_fov = Some(v.fov);
        }
        self.form_bp_shake = bp_out.shake;
        if bp_out.reset_field_camera {
            rig.field.reset(&target, true);
        }
        if let Some(t) = bp_out.takeover {
            rig.field.take_over(t.location, t.rotation);
        }
        // (AUDIT-A CAMS) the core's BpEvent windows the blueprints read, beside their log
        for e in &out.events {
            if let kh_core::host::EventKind::AnimBpEvent { seq, param, begin } = e.kind {
                println!("cam: step {}: (AUDIT-A CAMS) core BpEvent {seq} param {param} {}", out.step, if begin { "begin" } else { "end" });
            }
        }
        for l in self.form_bp.log.drain(..) {
            println!("cam: step {}: (AUDIT-C) form_bp: {l}", out.step);
        }
        // (AUDIT-A WORLDTIME) the world clock: the table as this step left it, the blueprint's SetGlobalGameSpeed of the step
        self.world.request(self.form_bp.shield.game_speed.take());
        self.world.step(STEP /* real time */, 1.0, |t| sora.time_dilation(t));
        for l in self.world.take_log() {
            println!("cam: step {}: (AUDIT-A WORLDTIME) {l}", out.step);
        }
        if let Some(on) = bp_out.blue_hour {
            println!("cam: step {}: (AUDIT-C) the Blue Hour {}", out.step, if on { "BEGINS" } else { "ENDS" });
            kh_er_render::bluehour::set(on); // (AUDIT-C TF) the map hide, as kh-er cam.rs
        }
        // (AUDIT-B W-FIN) FINISH_SK's ceiling look, as kh-er cam.rs
        if let Some(on) = bp_out.tower {
            println!("cam: step {}: (AUDIT-B W-FIN) the ceiling look {}", out.step, if on { "BEGINS" } else { "ENDS" });
            kh_er_render::bluehour::set_tower(on);
        }
        // (AUDIT-B F-HAMMER) the hammer forms' cameras over that layer (kh-er cam.rs, after form_bp): Sora's transformations,
        // Giant Stamp, the drill's combo finish / FOV kick, FINISH_DR's drill and Clock cameras
        let frame = kh_camera::forms_hammer::HammerFrame {
            sora: (out.position, out.yaw),
            field: kh_camera::camera_anim::AnimView { location: view.position, rotation: view.rotation, fov: self.form_bp_fov.unwrap_or(field_fov) },
            state: out.state,
            classic_tone: kh_core::combat::form::equipped_keyblade(sora.status()) == kh_camera::forms_hammer::CLASSIC_TONE,
            perf: kh_camera::form_bp::PerfCamera::default(),
        };
        let hm = self.hammer.step(dt, &out.events, &frame, Some(&r.world));
        self.hammer_fov = None;
        if let Some(v) = hm.view {
            view.position = v.location;
            view.rotation = v.rotation;
            view.look_point = v.location + v.rotation.forward() * 300.0;
            self.hammer_fov = Some(v.fov);
        }
        if hm.reset_field_camera {
            rig.field.reset(&target, true);
        }
        if let Some(t) = hm.takeover {
            rig.field.take_over(t.location, t.rotation);
        }
        for l in self.hammer.log.drain(..) {
            println!("cam: step {}: (AUDIT-B F-HAMMER) {l}", out.step);
        }
        // (AUDIT-A STAFF) Mirage Staff's blueprint (Rapunzel_key, stepped inside form_bp: its view, shake and takeover are
        // in bp_out, its log in form_bp's), as kh-er cam.rs: the cramped finish's void
        if let Some(on) = self.form_bp.staff.void.take() {
            println!("cam: step {}: (AUDIT-A STAFF) FINISH_WA's void {}", out.step, if on { "BEGINS" } else { "ENDS" });
        }
        self.mode = if self.hammer_fov.is_some() { "form-hammer" /* (AUDIT-B F-HAMMER) */ } else if self.form_bp_fov.is_some() { "form-bp" } else if let Some(label) = self.forms_label { label } else if self.shot.active() { "shot" } else if aiming { "aim" } else if rig.locked().is_some() { "lock-on" } else if rig.attack_camera().mode != 0 { "attack" } else { "field" };
        // a camera whose view jumps by more than a metre between two steps: a cut (kh-er interp.rs camera_cut's rule is
        // for drawing between steps; here it is only reported)
        if (view.position - self.previous.position).length() > 100.0 {
            self.cut = true;
        }
        self.view = view;
        self.before = Some(out.clone());
    }

    /// kh-er `KhCam::post` at alpha 1 (the harness draws each step): the view with the shakes, the lens. None before the
    /// first step.
    pub fn shown(&mut self, step: usize) -> Option<Shown> {
        self.rig.as_ref()?;
        let (position, mut rotation) = (self.view.position, self.view.rotation);
        let mut shake = self.shakes.offset();
        shake.rotation += self.form_bp_shake.rotation; // (AUDIT-C)
        shake.location += self.form_bp_shake.location;
        rotation.pitch += shake.rotation.x;
        rotation.yaw += shake.rotation.y;
        rotation.roll += shake.rotation.z;
        // (kh-er: the form camera's FOV x cam.fov / 100; cam.fov = KH3's 100 here)
        let fov_h = self.hammer_fov.or(self.form_bp_fov).or(self.forms_fov).unwrap_or(FOV_KH3); // (AUDIT-B F-HAMMER) its lens over form_bp's // (AUDIT-C) the blueprint camera's lens first
        // kh-er cam.rs `raw_of` (KH space: X forward, Y right, Z up; left-handed: up = forward x right)
        let forward = rotation.forward();
        let yaw = rotation.yaw.to_radians();
        let flat_right = Vec3::new(-yaw.sin(), yaw.cos(), 0.0);
        let flat_up = forward.cross(flat_right);
        let (s, c) = rotation.roll.to_radians().sin_cos();
        let right = flat_right * c - flat_up * s;
        let up = flat_up * c + flat_right * s;
        let eye = position + forward * shake.location.x + right * shake.location.y + up * shake.location.z;
        let look = self.view.look_point;
        if self.mode != self.last_mode {
            println!("cam: step {step}: CAMERA {} -> {}", if self.last_mode.is_empty() { "(none)" } else { self.last_mode }, self.mode);
            self.last_mode = self.mode;
        }
        println!(
            "cam: step {step} | mode {} | eye ({:.0}, {:.0}, {:.0}) look ({:.0}, {:.0}, {:.0}) | yaw {:.1} pitch {:.1} roll {:.2} | distance {:.0} | fov_h {:.1} | {}{}",
            self.mode,
            eye.x,
            eye.y,
            eye.z,
            look.x,
            look.y,
            look.z,
            rotation.yaw,
            rotation.pitch,
            rotation.roll,
            self.view.distance,
            fov_h,
            if self.cut { "CUT" } else { "-" },
            if self.shakes.is_empty() { "" } else { " | shaking" }
        );
        Some(Shown { eye, look, forward, right, up, fov_v: fov_vertical(fov_h) })
    }
}
