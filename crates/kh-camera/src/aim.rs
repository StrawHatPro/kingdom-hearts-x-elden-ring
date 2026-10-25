//! The shotlock aim camera `ATresCameraShootFlow` (docs/sora-magic-spec.md part F A5; OWNER:
//! the magic implementer). A second camera put on top of the camera stack while the aim mode is
//! open, with a 0.5 s ease-out (exponent 2) blend in and out.
//!
//! It is the field camera's update with this parameter set (constructor 0x1405FD440):
//! distance 200 (constant: no pitch-dependent distance), look point shifted 50 cm forward and
//! 100 cm right in view space (over the right shoulder), start rotation = behind Sora at pitch
//! -10, pitch limits +-70, no automatic follow, no wall-avoiding rotation, no reset, look-point
//! lag 60 /s (it reaches the target within one 1/60 s slice), rotation lag 8 /s. The view is
//! turned by the RIGHT stick, or by the LEFT stick while the right stick is at rest (stick mode
//! 2), at 120 degrees / s per unit deflection with no sensitivity setting; the camera's own dead
//! zone is 0.1 after the engine's 0.25.
//!
//! So the camera sits 150 cm behind and 100 cm to the right of the point 152.3 cm above Sora's
//! origin, looking parallel to the aim direction. It is object type CAMERA: never slowed by the
//! aim mode's slow motion (the host steps it with the real dt).
//!
//! Open (spec F): the field of view (the component default 90 is assumed by the spec; a host
//! may keep its own), the collision details (here: one sphere sweep of radius 20 from the look
//! point back to the camera).

use glam::{Vec2, Vec3};
use kh_core::World;

use crate::{normalize_axis, CameraTarget, CameraView, Rot};

/// The constants of F A5.1 (sheet `sora-magic-shotlocks`, rows `aim2.camera.*`).
pub const AIM_DISTANCE: f32 = 200.0;
pub const AIM_LOCAL_OFFSET: Vec3 = Vec3::new(50.0, 100.0, 0.0);
/// Socket `Zup_Cam` (actor origin + 102.338) + `m_TargetOffset` (0, 0, 50).
pub const AIM_TARGET_HEIGHT: f32 = 152.338;
pub const AIM_START_PITCH: f32 = -10.0;
pub const AIM_PITCH_LIMIT: f32 = 70.0;
pub const AIM_ROTATION_SPEED: f32 = 120.0;
pub const AIM_ROTATION_LERP: f32 = 8.0;
pub const AIM_STICK_DEAD_ZONE: f32 = 0.1;
pub const AIM_ENGINE_DEAD_ZONE: f32 = 0.25;
pub const AIM_PROBE_RADIUS: f32 = 20.0;
/// The blend block: 0.5 s, `VTBlend_EaseOut`, exponent 2.
pub const AIM_BLEND_TIME: f32 = 0.5;
pub const AIM_BLEND_EXPONENT: f32 = 2.0;
/// The camera component's native field of view (assumed by the spec; open).
pub const AIM_FOV: f32 = 90.0;

/// The aim camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AimCamera {
    pub rotation: Rot,
    pub rotation_target: Rot,
    view: CameraView,
}

/// The look point of the aim camera for `rotation` (Sora's point shifted over his right shoulder in view space).
fn look_point(target: &CameraTarget, rotation: Rot) -> Vec3 {
    let forward = rotation.forward();
    let yaw = rotation.yaw.to_radians();
    let right = Vec3::new(-yaw.sin(), yaw.cos(), 0.0);
    let up = right.cross(forward) * -1.0;
    target.position + Vec3::Z * AIM_TARGET_HEIGHT + forward * AIM_LOCAL_OFFSET.x + right * AIM_LOCAL_OFFSET.y + up * AIM_LOCAL_OFFSET.z
}

/// (IMPL_ER37) The aim rotation whose screen-centre ray passes through `point`: the look point depends on the rotation
/// (the shoulder offset), so a few fixed-point steps (each one aims from the previous look point; converges in 3-5 steps
/// beyond ~2 m). Pitch clamped to the aim's limits; a point on top of Sora keeps his facing.
pub fn rotation_toward(target: &CameraTarget, point: Vec3) -> Rot {
    let mut rotation = Rot::new(AIM_START_PITCH, target.yaw, 0.0);
    for _ in 0..12 {
        let d = point - look_point(target, rotation);
        let flat = Vec2::new(d.x, d.y).length();
        if flat < 1.0 {
            break;
        }
        let next = Rot::new(d.z.atan2(flat).to_degrees().clamp(-AIM_PITCH_LIMIT, AIM_PITCH_LIMIT), normalize_axis(d.y.atan2(d.x).to_degrees()), 0.0);
        let moved = normalize_axis(next.yaw - rotation.yaw).abs() + (next.pitch - rotation.pitch).abs();
        rotation = next;
        if moved < 1.0e-3 {
            break;
        }
    }
    rotation
}

/// (IMPL_ER42, USER REQUEST "shotlock should aim at softlock target") Where the aim starts: the HARD lock-on target's point
/// (`AimMode::saved_lock`) wins; without one the SOFT target's point (`AimMode::saved_soft`, the target Sora's own targeting
/// had chosen); neither (or no finite point) = `None`: behind Sora as KH3. Both hosts pass the result to [`AimOverlay::open_toward`].
pub fn start_point(hard: Option<Vec3>, soft: Option<Vec3>) -> Option<Vec3> {
    hard.filter(|p| p.is_finite()).or(soft.filter(|p| p.is_finite()))
}

fn shape(v: f32) -> f32 {
    // Engine dead zone 0.25, then the camera's own 0.1 (`m_AnalogStickPlayX / Y`).
    let engine = v.signum() * (v.abs() - AIM_ENGINE_DEAD_ZONE).max(0.0) / (1.0 - AIM_ENGINE_DEAD_ZONE);
    engine.signum() * ((engine.abs() - AIM_STICK_DEAD_ZONE) / (1.0 - AIM_STICK_DEAD_ZONE)).max(0.0)
}

impl AimCamera {
    /// `StartTresCamera` (0x14060CA80): the view always starts behind Sora at pitch -10,
    /// whatever the field camera was showing. `like` supplies the fields of the view that the
    /// aim camera does not have (timers of the field camera's trace).
    pub fn start(target: &CameraTarget, like: &CameraView) -> AimCamera {
        let rotation = Rot::new(AIM_START_PITCH, target.yaw, 0.0);
        let mut camera = AimCamera { rotation, rotation_target: rotation, view: *like };
        camera.place(target, None);
        camera
    }

    pub fn view(&self) -> CameraView {
        self.view
    }

    /// (IMPL_ER37, USER REQUEST: "shotlock camera needs to auto target the current lockon when first initiated, otherwise it
    /// just faces where the camera is currently facing") NOT KH3 BEHAVIOUR AS READ: KH3's aim starts behind Sora (above);
    /// at entry it only REMEMBERS the lock-on target (ctrl+0x9A4 in 0x1414165E0, kh-core `AimMode::saved_lock`) and clears
    /// the lock. With a remembered lock-on target the port starts the aim so that the reticle (the screen centre: the
    /// camera's forward through the over-the-shoulder look point) lies on `point`, the target's lock-on point (the same
    /// point the lock-on camera is given), pitch within the aim's limits. Without one: [`AimCamera::start`] unchanged.
    pub fn start_toward(target: &CameraTarget, point: Option<Vec3>, like: &CameraView) -> AimCamera {
        let mut camera = AimCamera::start(target, like);
        if let Some(point) = point.filter(|p| p.is_finite()) {
            let rotation = rotation_toward(target, point);
            (camera.rotation, camera.rotation_target) = (rotation, rotation);
            camera.place(target, None);
        }
        camera
    }

    fn place(&mut self, target: &CameraTarget, world: Option<&dyn World>) {
        let forward = self.rotation.forward();
        let look = look_point(target, self.rotation);
        let wanted = look - forward * AIM_DISTANCE;
        let (position, collided) = match world.and_then(|w| w.sweep_capsule(look, wanted, AIM_PROBE_RADIUS, AIM_PROBE_RADIUS)) {
            Some(hit) => (look + (wanted - look) * hit.time, true),
            None => (wanted, false),
        };
        self.view = CameraView { position, rotation: self.rotation, rotation_target: self.rotation_target, look_point: look, distance: (look - position).length(), stick_active: false, followed: false, collided, ..self.view };
    }

    /// One step: `right_stick` / `left_stick` raw (-1..1, x right, y up); `dt` is REAL time.
    pub fn step(&mut self, dt: f32, right_stick: Vec2, left_stick: Vec2, target: &CameraTarget, world: &dyn World) -> CameraView {
        let finite = |v: Vec2| if v.is_finite() { v } else { Vec2::ZERO };
        let (right, left) = (finite(right_stick), finite(left_stick));
        // Mode 2: the left stick while the right stick is at rest (after the engine's dead zone).
        let engine = |v: f32| (v.abs() - AIM_ENGINE_DEAD_ZONE).max(0.0);
        let use_left = engine(right.x) <= 1.0e-8 && engine(right.y) <= 1.0e-8;
        let (x, y) = if use_left {
            // Left stick up = pitch up.
            (shape(left.x), shape(left.y))
        } else {
            // "if !left: ry = -ry": right stick up = pitch down, as the field camera reads it.
            (shape(right.x), shape(-right.y))
        };
        self.rotation_target.yaw = normalize_axis(self.rotation_target.yaw + x * AIM_ROTATION_SPEED * dt);
        self.rotation_target.pitch = (self.rotation_target.pitch + y * AIM_ROTATION_SPEED * dt).clamp(-AIM_PITCH_LIMIT, AIM_PITCH_LIMIT);
        // rotCur follows rotTarget (rate 8 /s, in slices of 1/60 s).
        let mut left_dt = dt;
        while left_dt > 0.0 {
            let step = left_dt.min(1.0 / 60.0);
            left_dt -= 1.0 / 60.0;
            let a = (step * AIM_ROTATION_LERP).clamp(0.0, 1.0);
            self.rotation.pitch += normalize_axis(self.rotation_target.pitch - self.rotation.pitch) * a;
            self.rotation.yaw = normalize_axis(self.rotation.yaw + normalize_axis(self.rotation_target.yaw - self.rotation.yaw) * a);
        }
        self.place(target, Some(world));
        self.view.stick_active = x != 0.0 || y != 0.0;
        self.view
    }
}

/// The aim camera on top of another camera with its blends: what a host keeps beside its field
/// camera (or `CameraRig`). `open(...)` when the core's `AimMode { open: true }` event arrives,
/// `close()` at `AimMode { open: false }`; `step` returns the view to show.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct AimOverlay {
    pub camera: Option<AimCamera>,
    /// Blend weight of the aim camera, 0..1 (linear time; the ease is applied when mixing).
    weight: f32,
    /// The last view of the aim camera, for the blend out.
    last: Option<CameraView>,
}

impl AimOverlay {
    pub fn open(&mut self, target: &CameraTarget, like: &CameraView) {
        self.camera = Some(AimCamera::start(target, like));
    }

    /// (IMPL_ER37, user request) [`AimOverlay::open`] pointed at the lock-on target remembered at the aim's entry
    /// (`lock_point`: its lock-on point; `None` = no lock-on: behind Sora as KH3). See [`AimCamera::start_toward`].
    pub fn open_toward(&mut self, target: &CameraTarget, like: &CameraView, lock_point: Option<Vec3>) {
        self.camera = Some(AimCamera::start_toward(target, lock_point, like));
    }

    pub fn close(&mut self) {
        if let Some(camera) = self.camera.take() {
            self.last = Some(camera.view());
        }
    }

    pub fn active(&self) -> bool {
        self.camera.is_some()
    }

    /// Steps the aim camera (when open) and mixes it over `under` (the view of the camera
    /// below). `dt` real.
    pub fn step(&mut self, dt: f32, right_stick: Vec2, left_stick: Vec2, target: &CameraTarget, world: &dyn World, under: &CameraView) -> CameraView {
        let rate = dt / AIM_BLEND_TIME;
        match &mut self.camera {
            Some(camera) => {
                let view = camera.step(dt, right_stick, left_stick, target, world);
                self.last = Some(view);
                self.weight = (self.weight + rate).min(1.0);
            }
            None => self.weight = (self.weight - rate).max(0.0),
        }
        match self.last {
            Some(aim) if self.weight > 0.0 => {
                // Ease-out with exponent 2 in the direction of the blend in progress.
                let alpha = if self.camera.is_some() { crate::rig::ease_out(self.weight, AIM_BLEND_EXPONENT) } else { 1.0 - crate::rig::ease_out(1.0 - self.weight, AIM_BLEND_EXPONENT) };
                crate::rig::blend_views(under, &aim, alpha)
            }
            _ => {
                self.last = None;
                *under
            }
        }
    }
}
