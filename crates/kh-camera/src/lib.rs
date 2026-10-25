//! Kingdom Hearts III's field camera, engine-free: the whole of `FUN_1406033C0`, the update of
//! `TresCameraNormal`, as `docs/sora-camera-spec.md` describes it. Constants are generated from
//! `sheets/port/sora-camera.json` (`CameraParams`). `glam` and `std` only; no globals, no clock:
//! the same inputs give bit-identical outputs.
//!
//! Conventions as in `kh_core`: centimetres, seconds, degrees; X forward, Y right, Z up; yaw 0
//! looks along +X and grows toward +Y; **pitch positive = the camera looks up**.
//!
//! # How a host uses it (spec 1.3, 1.4, 4)
//!
//! The game runs the camera once per frame AFTER the pawn, and the controller steers by the yaw
//! the camera had at the end of the PREVIOUS frame. In a fixed 1/60 s step:
//!
//! ```text
//! input.camera_yaw = camera.input_yaw();          // last step's camera, pitch zeroed
//! output = sora.step(dt, &input, &world);
//! view   = camera.step(dt, &camera_input, &target_of(output), &world);
//! ```
//!
//! and the drawn camera is interpolated between the two latest views like the pawn.
//!
//! # What is in it, by spec section
//!
//! 2.2 the order of one update · 2.3 target point, look-point lag, distance · 2.4 rotation
//! smoothing · 2.5 right stick · 2.6 automatic follow, slope pitch, long-fall tilt · 2.7
//! wall-avoiding rotation with its path probe · 2.8 reset · 2.9 start state · 3.1 look-point
//! sweep · 3.3 camera sweep. Each function names its section and address.
//!
//! # Stubbed, as the spec's section 5 lists for stage 1
//!
//! Lock-on, battle distance, wall-run, dive, camera shake (probe radius term 0), other cameras
//! on the stack, level volumes, water (step 14), body-collision push-out and see-through fades
//! (3.3, 3.4), the outside-wall check (step 10: never runs for the field camera, 3.2).
//!
//! # Open points implemented as the code reads (spec 6.2), to confirm in the game
//!
//! - Raw right-stick Y is taken as positive for "up"; the code negates it, so stick up lowers the
//!   pitch (the camera rises and looks down).
//! - The pawn's collision cylinder used by the look-point sweep (3.1) was not read: the host
//!   passes a half height in [`CameraTarget::collision_half_height`].
//! - The camera sweep ignores components that already block a sphere at the look point (3.3). The
//!   host's `World` has no component identities, so here a sweep that STARTS overlapping is
//!   treated as hitting nothing.
//! - [`CameraInput::extra_rotation`] is NOT game behaviour: a convenience for mouse look, added to
//!   the stick's rotation of the frame. The game's mouse path was not specified (spec 2.5).

#![forbid(unsafe_code)]

use glam::{Vec2, Vec3};
use kh_core::World;

include!(env!("KH_SORA_CAMERA_PARAMS_RS"));

pub mod aim;
/// (party stream D) KH3's CameraAnim player and the team-attack state-event cameras.
pub mod camera_anim;
// (CV1) the conversation camera: shots, the shot director, collision, the blend in / out
pub mod conv;
/// (boss finish) The three cuts of KH3's boss finish (docs/sora-bossfinish-spec.md).
pub mod finish;
pub mod lockon;
pub mod rig;
// (vfx implementer 3) camera shakes as an additive offset on the drawn view.
pub mod shake;
// (fixes pass 1) The shot camera of the port: a port-side design, not game behaviour.
pub mod shot;
// (NF) KH3's near-camera / see-through fade of the characters (TresCameraManager's shut-out fade)
pub mod camfade;

pub use lockon::{LockTarget, LockonCamera};
pub use rig::CameraRig;

/// `FRotator`: degrees.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rot {
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
}

impl Rot {
    pub const fn new(pitch: f32, yaw: f32, roll: f32) -> Rot {
        Rot { pitch, yaw, roll }
    }

    /// The unit vector the rotation looks along.
    pub fn forward(self) -> Vec3 {
        let (p, y) = (self.pitch.to_radians(), self.yaw.to_radians());
        Vec3::new(p.cos() * y.cos(), p.cos() * y.sin(), p.sin())
    }

    /// `FRotator::GetNormalized`: every component in (-180, 180].
    pub fn normalized(self) -> Rot {
        Rot { pitch: normalize_axis(self.pitch), yaw: normalize_axis(self.yaw), roll: normalize_axis(self.roll) }
    }

    /// `FVector::Rotation`: the rotation that looks along `v` (roll 0).
    pub fn looking_along(v: Vec3) -> Rot {
        Rot { pitch: v.z.atan2(Vec2::new(v.x, v.y).length()).to_degrees(), yaw: v.y.atan2(v.x).to_degrees(), roll: 0.0 }
    }
}

/// An angle wrapped to (-180, 180].
pub fn normalize_axis(a: f32) -> f32 {
    let r = a.rem_euclid(360.0);
    if r > 180.0 {
        r - 360.0
    } else {
        r
    }
}

/// What the camera needs to know about the pawn's state (the states the update tests for).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetState {
    /// Player state id 1: the velocity is taken as 0 (2.6, 2.7).
    Idle,
    /// Player state id 2: the slope pitch applies (2.6.3).
    Run,
    /// Anything else of stage 1 (Jump).
    Other,
    /// (integration pass; spec section 5 row 3, decompiled C of 0x1406068F0 / 0x140608790 /
    /// slot 290 0x140606390) Player state id 8, the wall run, with `WallRun+0xA8`: 0 up, 1 along
    /// (base yaw +40), 2 the mirror (-40). Base pitch 0, pitch limit 60, distance toward
    /// `m_WallDistance`.
    WallRun(u8),
    /// (integration pass) The rail slide (pawn vt+0x2B38 in 0x1406068F0): base pitch 5.
    RailSlide,
    /// (integration pass) Player state id 10, the slope slide: the slope pitch applies as in Run.
    SlopeSlide,
}

/// The wall-run camera's constants [decompiled C: 0x1406068F0 writes 40.0 / -40.0 into the base
/// yaw and 0 into the base pitch; slot 290 0x140606390 caps the pitch at 60.0 and divides by
/// (base pitch + 80.0)]; the rail slide's base pitch 5.0 (0x1406068F0). The spec's parameter
/// table has `distance.wall_run` 700 and the note "60 during wall-run" only: these are not
/// sheet rows yet (listed in build/sora_tmp/IMPL_INTEGRATION_STATE.md).
pub const WALL_RUN_BASE_YAW: f32 = 40.0;
pub const WALL_RUN_PITCH_MAX: f32 = 60.0;
pub const WALL_RUN_DISTANCE_PITCH_SPAN: f32 = 80.0;
pub const RAIL_SLIDE_BASE_PITCH: f32 = 5.0;

/// The followed pawn, as of THIS frame's movement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraTarget {
    /// The actor origin (foot level), cm.
    pub position: Vec3,
    /// The actor's yaw, degrees.
    pub yaw: f32,
    /// The measured displacement velocity of the movement component's last tick
    /// (`MyMovement + 0x9A0`, spec 2.6.2): `(location now - location before) / dt`, cm/s. Not
    /// `Velocity`: it is 0 against a wall and includes root-motion displacement.
    pub velocity: Vec3,
    pub state: TargetState,
    /// Half height of the pawn's collision cylinder, for the look-point sweep (3.1; not read in
    /// the game: Sora's capsule half height, 89, is the natural value).
    pub collision_half_height: f32,
}

/// One step's camera input.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct CameraInput {
    /// Right stick, RAW axis values, each -1..1: `x` = RightX (right), `y` = RightY (up). The
    /// engine's dead zone (0.25) and the camera's own (0.3) are applied here.
    pub right_stick: Vec2,
    /// R3 (`ActionCameraReset`) this step: a non-snapping reset (2.8).
    pub recentre: bool,
    /// NOT GAME BEHAVIOUR: degrees of (yaw, pitch) added to the stick's rotation of this step, for
    /// mouse look in a test host. Counts as stick input (it holds the automatic follow off).
    pub extra_rotation: Vec2,
}

/// The camera after a step, with the internal values a trace wants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraView {
    /// Camera position, cm.
    pub position: Vec3,
    /// The rotation shown (`rotCur`).
    pub rotation: Rot,
    /// The rotation asked for (`rotTarget`).
    pub rotation_target: Rot,
    /// `m_LookPosition`: the look point after its lag and collision.
    pub look_point: Vec3,
    /// `m_CurrentDistance`: the real camera-to-look-point distance.
    pub distance: f32,
    pub auto_power: f32,
    pub stick_wait: f32,
    pub still_wait: f32,
    pub reset_wait: f32,
    /// Mean of the slope-pitch ring.
    pub slope_pitch: f32,
    pub stick_active: bool,
    /// The automatic follow acted this step.
    pub followed: bool,
    /// The camera sweep (3.3) shortened the distance this step.
    pub collided: bool,
}

impl CameraView {
    /// The view to draw between two consecutive steps: `alpha` 0 is `previous`, 1 is `self`.
    /// Returns (position, rotation); angles take the short way round.
    pub fn lerp(&self, previous: &CameraView, alpha: f32) -> (Vec3, Rot) {
        let mix = |a: f32, b: f32| normalize_axis(a + normalize_axis(b - a) * alpha);
        let (p, c) = (previous.rotation, self.rotation);
        (previous.position.lerp(self.position, alpha), Rot { pitch: mix(p.pitch, c.pitch), yaw: mix(p.yaw, c.yaw), roll: mix(p.roll, c.roll) })
    }
}

/// Projection constants (spec 1.5): 100 degrees horizontal on a constrained 16:9 frame (bars at
/// other window shapes), near plane 3 cm. Returns (horizontal FOV degrees, aspect ratio, near cm).
pub fn projection(p: &CameraParams) -> (f32, f32, f32) {
    (p.fov_horizontal, p.fov_aspect_ratio, p.clip_near)
}

/// `TresCameraNormal`'s state (spec 2.1).
#[derive(Clone, Debug, PartialEq)]
pub struct FieldCamera {
    p: CameraParams,
    rot_target: Rot,
    rot_cur: Rot,
    rot_prev: Rot,
    look_smooth: Vec3,
    look_position: Vec3,
    dist_smooth: f32,
    current_distance: f32,
    auto_power: f32,
    stick_wait: f32,
    still_wait: f32,
    reset_wait: f32,
    slope_ring: [f32; 16],
    slope_index: usize,
    slope_mean: f32,
    /// "First update / snap" bit (this+0x468 bit 0).
    snap: bool,
    position: Vec3,
    view: CameraView,
    /// The pawn state of the update in progress (for the state branches of the base rotation,
    /// the pitch limit and the distance).
    mode: TargetState,
}

impl FieldCamera {
    /// `StartTresCamera` (slot 242 0x14060BA20), spec 2.9: directly behind the pawn, yaw = his
    /// yaw, pitch -10, look point at the target point, no blend; the snap bit makes the first
    /// update jump the distance and the look point to their targets.
    pub fn new(p: CameraParams, target: &CameraTarget) -> FieldCamera {
        let base = Rot::new(p.rotation_base_pitch, normalize_axis(target.yaw + p.rotation_base_yaw_offset), 0.0);
        let look = target_point(&p, target);
        let position = look - base.forward() * p.distance_base;
        let view = CameraView {
            position,
            rotation: base,
            rotation_target: base,
            look_point: look,
            distance: p.distance_base,
            auto_power: 0.0,
            stick_wait: 0.0,
            still_wait: 0.0,
            reset_wait: 0.0,
            slope_pitch: 0.0,
            stick_active: false,
            followed: false,
            collided: false,
        };
        FieldCamera {
            p,
            rot_target: base,
            rot_cur: base,
            rot_prev: base,
            look_smooth: look,
            look_position: look,
            dist_smooth: p.distance_base,
            current_distance: p.distance_current_initial,
            auto_power: 0.0,
            stick_wait: 0.0,
            still_wait: 0.0,
            reset_wait: 0.0,
            slope_ring: [0.0; 16],
            slope_index: 0,
            slope_mean: 0.0,
            snap: true,
            position,
            view,
            mode: TargetState::Other,
        }
    }

    /// (integration pass) The user option "Camera Work": Auto = the automatic follow (2.6) and
    /// the wall-avoiding rotation (2.7) may act; Manual = neither does (spec 2.6.1: the option
    /// bit read by 0x1406086B0 gates `FUN_1405FFA60` and `FUN_140603FF0`).
    pub fn set_auto_camera(&mut self, on: bool) {
        self.p.auto_option_default = on;
    }

    pub fn auto_camera(&self) -> bool {
        self.p.auto_option_default
    }

    pub fn params(&self) -> &CameraParams {
        &self.p
    }

    /// (combat implementer, 2026-10-12) For the attack camera's writes on the normal camera
    /// (`CameraRig::set_attack_mode`: look-point rate / lag limit, distance scale).
    pub fn params_mut(&mut self) -> &mut CameraParams {
        &mut self.p
    }

    /// The latest view (what the last `step` returned; the start state before any step).
    pub fn view(&self) -> CameraView {
        self.view
    }

    /// The yaw the controller steers by (spec 1.4): the rotation shown at the end of the LAST
    /// camera update, pitch zeroed. Read it before the pawn's step, step the camera after.
    pub fn input_yaw(&self) -> f32 {
        self.rot_prev.yaw
    }

    /// `Reset(snap)` (slot 244 0x140609C40), spec 2.8: `rotTarget` = behind the pawn, pitch -10.
    /// Without `snap` (the R3 button) the view eases to it and for 0.5 s the wall-avoiding
    /// rotation is off; with `snap` everything jumps on the next update.
    pub fn reset(&mut self, target: &CameraTarget, snap: bool) {
        if !self.p.reset_allowed {
            return;
        }
        self.rot_target = self.base(target);
        if snap {
            self.rot_cur = self.rot_target;
            self.rot_prev = self.rot_target;
            self.snap = true;
        } else {
            self.reset_wait = self.p.reset_wait;
        }
    }

    /// The base rotation (slot 299 0x1405EBDF0): `m_Rotation` (pitch -10) plus the actor's yaw.
    /// The source of `m_Rotation` (0x1406068F0, decompiled C) has two player-state branches: the
    /// wall run (pitch 0, yaw +40 / -40 by the run's class) and the rail slide (pitch 5).
    fn base(&self, target: &CameraTarget) -> Rot {
        let (pitch, yaw) = match target.state {
            TargetState::WallRun(class) => (0.0, match class {
                1 => WALL_RUN_BASE_YAW,
                2 => -WALL_RUN_BASE_YAW,
                _ => self.p.rotation_base_yaw_offset,
            }),
            TargetState::RailSlide => (RAIL_SLIDE_BASE_PITCH, self.p.rotation_base_yaw_offset),
            _ => (self.p.rotation_base_pitch, self.p.rotation_base_yaw_offset),
        };
        Rot::new(pitch, normalize_axis(yaw + target.yaw), 0.0)
    }

    /// The pitch clamp (slot 260 0x140608970): a hard clamp to [-70, 50], widened to contain the
    /// base pitch.
    fn clamp_pitch(&self, pitch: f32, base: Rot) -> f32 {
        // (60 during a wall run: the note of `rotation.pitch_max`.)
        let max = if matches!(self.mode, TargetState::WallRun(_)) { WALL_RUN_PITCH_MAX } else { self.p.rotation_pitch_max };
        normalize_axis(pitch).clamp(base.pitch.min(self.p.rotation_pitch_min), base.pitch.max(max))
    }

    /// Probe radius `FUN_1405CCFD0`: this+0x464 = 20 (no shake, no override).
    fn radius(&self) -> f32 {
        self.p.collision_radius
    }

    /// One update: `FUN_1406033C0(this, dt)`, spec 2.2, steps numbered as there.
    pub fn step(&mut self, dt: f32, input: &CameraInput, target: &CameraTarget, world: &dyn World) -> CameraView {
        let p = self.p;
        self.mode = target.state;
        if input.recentre {
            self.reset(target, false);
        }
        let base = self.base(target);

        // 1. Stick (slot 301 0x140607ED0): rotTarget += stickDelta, clamp.
        let stick_delta = self.stick_delta(input, dt);
        let stick_active = stick_delta != Rot::default();
        self.rot_target.yaw += stick_delta.yaw;
        self.rot_target.pitch += stick_delta.pitch;
        self.rot_target.pitch = self.clamp_pitch(self.rot_target.pitch, base);
        self.rot_target = self.rot_target.normalized();

        // 2. Automatic follow (slot 300 -> FUN_1405FFA60).
        let followed = self.auto_follow(target, base, dt, stick_active);

        // 3. Rotation smoothing (FUN_1405D8170), spec 2.4. With a gamepad every rate is 8 /s, so
        //    "fast" (stick active, stickWait > 0 or resetWait > 0) changes nothing.
        let fast = stick_active || self.stick_wait > 0.0 || self.reset_wait > 0.0;
        let rate = if fast { p.rotation_lerp_rate_fast_gamepad } else { p.rotation_lerp_rate };
        if p.rotation_smoothing_enabled {
            let from = if self.snap { self.rot_target } else { self.rot_prev };
            self.rot_cur = rot_lerp_step(&p, from, self.rot_target, rate, dt);
        } else {
            self.rot_cur = self.rot_target;
        }
        // 4.
        let rot = self.rot_cur;

        // 5. Distance (slot 290 0x140606390), spec 2.3.3. Flag 0x08: the smoothing restarts each
        //    frame from m_CurrentDistance, the real distance after last frame's collision.
        let want = self.wanted_distance();
        self.dist_smooth = if self.snap { want } else { lerp_step(&p, self.current_distance, want, p.distance_lerp_rate, dt) };
        self.current_distance = self.dist_smooth;

        // 6. Target point (slot 303), spec 2.3.1.
        let raw_target = target_point(&p, target);

        // 7. Look point (FUN_1405D0B30), spec 2.3.2: lag 8 /s, then the 100 cm sphere.
        if self.snap {
            self.look_smooth = raw_target;
        } else {
            self.look_smooth = lerp_step_vec(&p, self.look_smooth, raw_target, p.look_lerp_rate, dt);
            let behind = self.look_smooth - raw_target;
            if p.look_lag_limit >= 0.0 && behind.length() > p.look_lag_limit {
                self.look_smooth = raw_target + behind.normalize() * p.look_lag_limit;
            }
        }

        // 8. Look-point collision (FUN_1406054D0), spec 3.1.
        let corrected = self.look_point_collision(target, world);
        self.look_position = self.look_smooth;

        // 9. camPos = look - Forward(rot) x distance. (10. the outside-wall check never runs.)
        let mut cam_pos = self.look_position - rot.forward() * self.current_distance;
        // 11.
        let mut look_pt = self.look_position;

        // 12. Conditions of the wall-avoiding rotation.
        let away = self.facing(target) >= p.auto_facing_threshold; // FUN_140608440(this, -0.8)
        let mut do_avoid = followed;
        if (self.look_position - raw_target).length_squared() > p.avoid_look_moved_sq && !stick_active && away && self.stick_wait <= 0.0 {
            do_avoid = true;
        }
        if self.reset_wait > 0.0 {
            self.reset_wait = (self.reset_wait - dt).max(0.0);
            do_avoid = false;
        }
        if corrected {
            do_avoid = false;
        }

        // 13. Wall-avoiding rotation (FUN_140603FF0), spec 2.7. (14. water: no water.)
        if self.wall_avoid(target, base, world, cam_pos, look_pt, do_avoid, away, dt) {
            cam_pos = self.look_position - self.rot_cur.forward() * self.current_distance;
            look_pt = cam_pos + self.rot_cur.forward() * self.current_distance;
        }

        // 15. Final collision (FUN_14061CB40), spec 3.3: sphere(r) from the look point to the
        //     camera; the first blocking hit is the camera's place, with no margin.
        let mut collided = false;
        if !p.collision_no_collision_config {
            let r = self.radius();
            if let Some(hit) = world.sweep_capsule(look_pt, cam_pos, r, r) {
                // A sweep that starts inside something: that component is one the game ignores.
                if !hit.start_penetrating {
                    cam_pos = look_pt + (cam_pos - look_pt) * hit.time.clamp(0.0, 1.0);
                    collided = true;
                }
            }
        }
        // 16.
        self.current_distance = (look_pt - cam_pos).length();
        self.position = cam_pos;

        // Back in CalcCamera (0x1405C8FD0): rotPrev = the rotation shown; the snap bit is cleared.
        self.rot_prev = self.rot_cur;
        self.snap = false;

        self.view = CameraView {
            position: cam_pos,
            rotation: self.rot_cur,
            rotation_target: self.rot_target,
            look_point: self.look_position,
            distance: self.current_distance,
            auto_power: self.auto_power,
            stick_wait: self.stick_wait,
            still_wait: self.still_wait,
            reset_wait: self.reset_wait,
            slope_pitch: self.slope_mean,
            stick_active,
            followed,
            collided,
        };
        self.view
    }

    /// The right stick's rotation of this frame (slots 259 / 258 / 266), spec 2.5.
    fn stick_delta(&self, input: &CameraInput, dt: f32) -> Rot {
        let p = &self.p;
        let raw = if input.right_stick.is_finite() { input.right_stick } else { Vec2::ZERO };
        // Engine axis config: DeadZone 0.25, Sensitivity 1, Exponent 1 (stock MassageAxisInput).
        let engine = |r: f32| r.signum() * (r.abs() - p.stick_engine_dead_zone).max(0.0) / (1.0 - p.stick_engine_dead_zone);
        let (rx, mut ry) = (engine(raw.x), engine(raw.y));
        // "ry = -ry: right stick only" [I 0x1405CD525]
        if p.stick_y_sign_flip {
            ry = -ry;
        }
        // The camera's own dead zone, per axis, linear (a cross, not a circle).
        let play = |r: f32, dead: f32| r.signum() * ((r.abs() - dead) / (1.0 - dead)).max(0.0);
        let (mut x, mut y) = (play(rx, p.stick_camera_dead_zone_x), play(ry, p.stick_camera_dead_zone_y));
        if p.stick_reverse_x_default {
            x = -x;
        }
        if p.stick_reverse_y_default {
            y = -y;
        }
        let (sx, sy) = (sensitivity(p.stick_sensitivity_x_default), sensitivity(p.stick_sensitivity_y_default));
        let extra = if input.extra_rotation.is_finite() { input.extra_rotation } else { Vec2::ZERO };
        Rot { yaw: x * sx * p.stick_yaw_speed * dt + extra.x, pitch: y * sy * p.stick_pitch_speed * dt + extra.y, roll: 0.0 }
    }

    /// The wanted distance (slot 290), spec 2.3.3: 400 for every pitch <= 0, shrinking linearly
    /// to 200 at pitch +50.
    fn wanted_distance(&self) -> f32 {
        let p = &self.p;
        let mut d = p.distance_base;
        let pitch = self.rot_cur.pitch;
        if matches!(self.mode, TargetState::WallRun(_)) {
            // The wall-run branch of slot 290 (0x140606390, decompiled C):
            // t = clamp((m_Rotation.pitch - min(pitch, 60)) / (m_Rotation.pitch + 80), 0, 1);
            // d = m_WallDistance + (d - m_WallDistance) * t: 700 at and above the base pitch.
            let t = ((p.rotation_base_pitch - pitch.min(WALL_RUN_PITCH_MAX)) / (p.rotation_base_pitch + WALL_RUN_DISTANCE_PITCH_SPAN)).clamp(0.0, 1.0);
            d = p.distance_wall_run + (d - p.distance_wall_run) * t;
        } else if pitch > 0.0 && p.distance_near > 0.0 {
            let t = ((p.rotation_pitch_max - p.rotation_pitch_max.min(pitch)) / p.rotation_pitch_max).clamp(0.0, 1.0);
            d = p.distance_near + (d - p.distance_near) * t;
        }
        d = d * p.distance_scale + p.distance_offset;
        if d <= 0.0 {
            d = p.distance_min;
        }
        d
    }

    /// `dot(normalize(camForward.x, camForward.y, actorForward.z), actorForward)` (2.6).
    fn facing(&self, target: &CameraTarget) -> f32 {
        let actor = Rot::new(0.0, target.yaw, 0.0).forward();
        let cam = self.rot_cur.forward();
        Vec3::new(cam.x, cam.y, actor.z).normalize_or_zero().dot(actor)
    }

    /// The velocity the camera uses (2.6.2); Idle forces it to 0.
    fn target_velocity(target: &CameraTarget) -> Vec3 {
        if target.state == TargetState::Idle {
            Vec3::ZERO
        } else {
            target.velocity
        }
    }

    /// Automatic follow, `FUN_1405FFA60(this, &rotTarget, base, dt, stickActive, 1, -0.8)`, spec 2.6.
    fn auto_follow(&mut self, target: &CameraTarget, base: Rot, dt: f32, stick_active: bool) -> bool {
        let p = self.p;
        if stick_active {
            self.stick_wait = p.auto_wait_after_stick; // [I 0x1405FFAB4]
        } else if self.stick_wait > 0.0 {
            self.stick_wait -= dt;
            if self.stick_wait <= 0.0 {
                self.stick_wait = 0.0;
                self.auto_power = 0.0;
            }
        }
        let allowed = p.auto_option_default && p.auto_enabled_flag && self.stick_wait <= 0.0;
        if !allowed || stick_active {
            return false;
        }
        let v = Self::target_velocity(target);
        let (speed2, speed3) = (Vec2::new(v.x, v.y).length(), v.length());
        if speed3 <= p.auto_min_speed {
            self.auto_power = 0.0; // [I 0x1405FFEF3]
            return false;
        }
        self.auto_power = (self.auto_power + p.auto_power_max * dt).min(p.auto_power_max);
        let rate = speed2 / p.auto_speed_reference * self.auto_power; // not clamped
        let facing = self.facing(target);

        let mut want = base;
        if v.z > p.auto_fall_speed_threshold {
            if matches!(target.state, TargetState::Run | TargetState::SlopeSlide) {
                want.pitch += self.slope_pitch(target.position, self.position, v, base);
            }
        } else {
            // Long fall: tilt down at up to 50 degrees a second, never below -50.
            let tilt = (v.z * p.auto_fall_tilt_gain).max(p.auto_fall_tilt_floor);
            let limit = p.auto_fall_tilt_floor.min(base.pitch);
            if self.rot_target.pitch >= limit {
                self.rot_target.pitch += tilt * dt;
            }
            want.pitch = self.rot_target.pitch;
        }
        want.roll = self.rot_target.roll;
        if facing < p.auto_facing_threshold {
            want.yaw = self.rot_target.yaw; // running toward the camera: yaw is left alone
        }
        self.rot_target = rot_lerp_step(&p, self.rot_target, want, rate, dt);
        self.rot_target.pitch = self.clamp_pitch(self.rot_target.pitch, base);
        self.rot_target = self.rot_target.normalized();
        true
    }

    /// Slope pitch (slot 302 0x140606B10 and `FUN_140605BD0`), spec 2.6.3. One sample per call
    /// into a 16-entry ring: per FRAME, not per second.
    fn slope_pitch(&mut self, pawn: Vec3, camera: Vec3, v: Vec3, base: Rot) -> f32 {
        let p = self.p;
        let to_pawn = (pawn - camera).truncate().normalize_or_zero();
        let moving = v.truncate().normalize_or_zero();
        // Moving toward the camera: mirrored.
        let d = if to_pawn.dot(moving) > p.slope_toward_camera_dot { v } else { -v };
        let mut a = 0.0;
        if d.abs().max_element() >= 1.0e-4 {
            a = d.z.atan2(d.truncate().length()).to_degrees();
            let high = (60.0 - base.pitch).max(60.0); // 70 for base pitch -10
            if a > high {
                a = high;
            } else if a < p.slope_min {
                a = p.slope_min;
            } else if a > -p.slope_dead_zone && a < p.slope_dead_zone {
                a = 0.0;
            }
        }
        let size = self.slope_ring.len();
        self.slope_index = (self.slope_index + 1) % size;
        self.slope_ring[self.slope_index] = a;
        // FUN_140605BD0(this, |normalize(d).z| < 0.9): with the flag, the 7 samples before the
        // newest must all "agree" with it, or the previous mean stands.
        let check = d.normalize_or_zero().z.abs() < 0.9;
        if check {
            let dead = p.slope_dead_zone;
            let agrees = |s: f32| (s == 0.0 && a == 0.0) || (s > dead && a > dead && s <= a) || (s < -dead && a < -dead && s >= a);
            let count = p.slope_agree_count as usize;
            let agreeing = (1..=count).filter(|k| agrees(self.slope_ring[(self.slope_index + size - k) % size])).count();
            if agreeing < count {
                return self.slope_mean;
            }
        }
        self.slope_mean = self.slope_ring.iter().sum::<f32>() / size as f32;
        self.slope_mean
    }

    /// Look-point collision (`FUN_1406054D0`), spec 3.1: a sphere of radius r + 1 from above the
    /// pawn's root to the look point; a hit puts the look point there. True when corrected.
    fn look_point_collision(&mut self, target: &CameraTarget, world: &dyn World) -> bool {
        let p = &self.p;
        if p.collision_no_collision_config {
            return false;
        }
        let r1 = self.radius() + p.collision_look_radius_add;
        let from = target.position + Vec3::Z * r1.max(target.collision_half_height);
        let Some(mut hit) = world.sweep_capsule(from, self.look_smooth, r1, r1) else { return false };
        let mut start = from;
        if hit.start_penetrating {
            // Starts inside: pushed out along the normal by the penetration depth, swept again.
            start = from + hit.normal * hit.penetration_depth;
            match world.sweep_capsule(start, self.look_smooth, r1, r1) {
                Some(again) if !again.start_penetrating => hit = again,
                _ => return false,
            }
        }
        self.look_smooth = start + (self.look_smooth - start) * hit.time.clamp(0.0, 1.0);
        true
    }

    /// Wall-avoiding rotation, `FUN_140603FF0`, spec 2.7. True when it turned the camera.
    #[allow(clippy::too_many_arguments)]
    fn wall_avoid(&mut self, target: &CameraTarget, base: Rot, world: &dyn World, cam_pos: Vec3, look_pt: Vec3, do_avoid: bool, away: bool, dt: f32) -> bool {
        let p = self.p;
        let v = Self::target_velocity(target);
        let speed2 = v.truncate().length();
        let mut do_avoid = do_avoid;
        let mut along_axis = false;
        if speed2 < p.avoid_still_speed && v.z.abs() < 1.0 {
            self.still_wait = p.avoid_still_wait; // standing: re-arm the delay
            do_avoid = false;
        } else if self.still_wait > 0.0 {
            self.still_wait -= dt;
            do_avoid = false;
        } else {
            // Moving straight along the view axis.
            let axis = Rot::new(0.0, self.rot_cur.yaw, 0.0).forward().truncate();
            along_axis = v.truncate().normalize_or_zero().dot(axis).abs() > p.avoid_axis_dot;
        }
        if p.collision_no_collision_config || !p.auto_option_default || !do_avoid {
            return false;
        }
        let Some(new_cam) = self.path_probe(world, look_pt, cam_pos) else { return false };

        let want = Rot::looking_along(self.look_position - new_cam);
        if !along_axis {
            self.rot_target.yaw = want.yaw;
        }
        if want.pitch.abs() < base.pitch.abs() || v.z.abs() >= 50.0 || (self.rot_target.pitch < want.pitch && want.pitch <= 0.0) {
            self.rot_target.pitch = want.pitch;
        }
        self.rot_target.pitch = self.clamp_pitch(self.rot_target.pitch, base);
        let rate = p.avoid_rate_max * (speed2 / p.auto_speed_reference).clamp(0.0, 1.0); // 8.5 /s at 680
        self.rot_cur = rot_lerp_step(&p, self.rot_cur, self.rot_target, rate, dt);
        if !away || speed2 < p.avoid_slow_speed {
            self.rot_target = rot_lerp_step(&p, self.rot_cur, self.rot_target, p.avoid_return_rate, dt);
        }
        true
    }

    /// The path probe `FUN_14060F490`: a sphere of twice the probe radius from the look point to
    /// the camera; where it is blocked it slides along the obstacle (direction projected on the
    /// hit plane, 0.1 cm off the surface) for the rest of the length, at most 10 times. Returns
    /// the end of the path when at least one slide happened.
    fn path_probe(&self, world: &dyn World, look_pt: Vec3, cam_pos: Vec3) -> Option<Vec3> {
        let radius = self.radius() * self.p.avoid_probe_radius_factor;
        let mut position = look_pt;
        let mut left = (cam_pos - look_pt).length();
        let mut direction = (cam_pos - look_pt).normalize_or_zero();
        let mut slid = false;
        for _ in 0..self.p.avoid_max_slides as usize {
            if left <= 1.0e-3 || direction == Vec3::ZERO {
                break;
            }
            match world.sweep_capsule(position, position + direction * left, radius, radius) {
                None => {
                    position += direction * left;
                    break;
                }
                Some(hit) => {
                    if hit.start_penetrating {
                        break;
                    }
                    let travelled = left * hit.time.clamp(0.0, 1.0);
                    position += direction * travelled + hit.normal * 0.1;
                    left -= travelled;
                    direction = (direction - hit.normal * direction.dot(hit.normal)).normalize_or_zero();
                    slid = true;
                }
            }
        }
        slid.then_some(position)
    }
}

/// The target point (slot 303 -> slot 284 -> `FUN_140624260`), spec 2.3.1: the socket `Zup_Cam`
/// (bone `t_camera`, 102.338 cm above the mesh origin, not animated in the stage-1 clips) plus
/// `m_TargetOffset` (0, 0, 48): the actor origin + 150.338 up.
pub fn target_point(p: &CameraParams, target: &CameraTarget) -> Vec3 {
    target.position + Vec3::Z * (p.target_socket_height + p.target_offset_z)
}

/// The sensitivity map `FUN_1405EC530`: a setting 1..100 -> 0.5 .. 2.4 (50 -> 1.2424).
pub fn sensitivity(setting: f32) -> f32 {
    let v = (setting - 1.0) / 99.0;
    if v <= 0.5 {
        0.5 + 0.75 * 2.0 * v
    } else {
        1.25 + 1.15 * (2.0 * v - 1.0)
    }
}

/// `FUN_1405D0850` (`same`): first-order approach in slices of at most 1/60 s (spec 2.2).
fn lerp_step(p: &CameraParams, mut current: f32, target: f32, rate: f32, mut dt: f32) -> f32 {
    while dt > 0.0 {
        let step = dt.min(p.lerp_slice);
        dt -= p.lerp_slice;
        current += (target - current) * (step * rate).clamp(0.0, 1.0);
    }
    current
}

/// `FUN_1405D0B30`'s vector lerp.
fn lerp_step_vec(p: &CameraParams, mut current: Vec3, target: Vec3, rate: f32, mut dt: f32) -> Vec3 {
    while dt > 0.0 {
        let step = dt.min(p.lerp_slice);
        dt -= p.lerp_slice;
        current += (target - current) * (step * rate).clamp(0.0, 1.0);
    }
    current
}

/// `FUN_1405D0650`: the rotator lerp, per component, the shortest way round.
fn rot_lerp_step(p: &CameraParams, mut current: Rot, target: Rot, rate: f32, mut dt: f32) -> Rot {
    while dt > 0.0 {
        let step = dt.min(p.lerp_slice);
        dt -= p.lerp_slice;
        let a = (step * rate).clamp(0.0, 1.0);
        current.pitch += normalize_axis(target.pitch - current.pitch) * a;
        current.yaw += normalize_axis(target.yaw - current.yaw) * a;
        current.roll += normalize_axis(target.roll - current.roll) * a;
    }
    current
}
