//! The lock-on camera, `TresCameraNormalLoc` (vtable 0x145D5D3E0): docs/sora-combat-spec.md 7.12
//! (strand D; it replaces the lock-on stub of docs/sora-camera-spec.md section 5). A second
//! camera with its own wanted rotation, look point and distance; the game spawns it on
//! `Camera.SetLockon(target)` and blends to and from the field camera over 0.5 s (ease-out,
//! exponent 2): the blend is `rig.rs`.
//!
//! Constants: the `lockon_cam.*` rows of `sheets/port/sora-combat-params` (through
//! `kh_core::combat::tables::P`) and the field camera's own (`CameraParams`).
//!
//! IMPLEMENTED AS SPECIFIED (instruction-level constants, decompiled-C control flow):
//! - base rotation (slot 299 0x140606190): look-at from the player point to the target's look-at
//!   position (yaw + 0.08), plus the pitch offset -20 with the floor -10 when the target is not
//!   more than 10 degrees below; straight at it otherwise;
//! - the stick (slot 266 0x140607390): the field camera's stick rotation scaled by the distance
//!   to the target: 1.0 within 500 cm, 0.45 beyond 2500 cm, linear between;
//! - the automatic turn (0x1405FFA60, the lock-on branch): the follow velocity is blended toward
//!   (`m_TargetMaxSpeed`, 0, 0) by k = (d - 300) / (3000 - 300) clamped and the rate multiplied by
//!   1 + 5 k; the 1.5 s stick wait, the `autoPower` ramp and the "speed > 1" test apply to it;
//! - the off-screen snap: without stick input and with the stick wait over, a target outside the
//!   frame (or behind the camera) puts the wanted rotation back on `base` at once;
//! - the clamp (0x140611760): pitch into [-70, min(L.pitch, 20) or 0 when L.pitch < 0], yaw into
//!   L.yaw +- 120, L = the look-at from the player point to the midpoint of player and target;
//! - rotation smoothing at 8 /s; the distance smoothing at 8 /s (the field camera's is 4);
//!   start: wanted rotation = base, look point = the midpoint, distance 400; a target that jumps
//!   more than 3000 cm/s is approached with rate 6.
//!
//! THIRD PASS (2026-10-12; docs/sora-combat-spec.md 7.12 "Position step", rewritten):
//! - the two-point framing solver (`frame2` / `fit` below: 0x1405DEFA0 -> 0x1405E0590 x 4, read
//!   in the decompiled C, the tail of the fit at instruction level): the farther of the two
//!   points is brought onto the nearer one's depth plane along its ray from the camera, the
//!   segment between them is lengthened by 50 cm at each end (both "sizes" are the constant
//!   50.0, 0x1406245C0), the look point is its middle and the distance is what the frustum needs
//!   for its half length: 1.1 x half / tan(FOV / 2) when it lies across the screen, half x
//!   cot(FOV / 2) / aspect when it stands upright, blended by the segment's angle on screen;
//!   three more rounds from the camera position the result gives;
//! - the velocity lead (0x1406100D0): each point is shifted along its owner's velocity by
//!   0.24 x (the length of its clamped screen position), only when moving on would carry it
//!   AWAY from the other on screen; smoothed with the look rate;
//! - the obstruction blend (0x14061BA10): the camera is pulled in when the line from the player
//!   point to it is blocked, and when the obstruction ends it eases back over 0.6 s (the start of
//!   the ease travels with the camera).
//!
//! (Fourth pass: the ease curve of that 0.6 s is interpolator type 2 = sin(t pi/2), `interp_curve`.)
//! STILL NOT READ: the second obstruction test of the camera class (slot 283: the base class
//! answers "no"), `SetLockonNearLimit`, the water plane, the look-point collision.

use glam::{Vec2, Vec3};
use kh_core::combat::tables::P as C;
use kh_core::World;

use crate::{lerp_step, lerp_step_vec, normalize_axis, rot_lerp_step, target_point, CameraInput, CameraParams, CameraTarget, CameraView, FieldCamera, Rot, TargetState};

/// The locked target as the camera needs it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LockTarget {
    /// `LookAtPosition()` (component slot 362): the camera's aim point on the target.
    pub look_at: Vec3,
}

/// `TresCameraNormalLoc`'s state.
#[derive(Clone, Debug, PartialEq)]
pub struct LockonCamera {
    p: CameraParams,
    rot_target: Rot,
    rot_cur: Rot,
    look_smooth: Vec3,
    /// +0x9E4.
    dist_smooth: f32,
    auto_power: f32,
    stick_wait: f32,
    /// +0xA74: the last known look-at position (used when the target is gone).
    last_target: Vec3,
    position: Vec3,
    first: bool,
    view: CameraView,
    /// +0x9E8 / +0x9F4: the smoothed leads of the player point and of the target.
    lead_player: Vec3,
    lead_target: Vec3,
    /// The target's look-at position of the previous update (its velocity is the difference).
    target_before: Vec3,
    /// +0xA80 .. +0xA9C: the obstruction blend: where the ease starts, the free position of the
    /// previous update, seconds since the obstruction ended (>= 0.6: no ease).
    blocked_from: Vec3,
    free_before: Vec3,
    release: f32,
    /// +0x888 `m_LimitYawMax` (120; the attack camera's mode 4 sets 0 through the parent's
    /// 0x14060A7D0 -> vt+0x940: a limit of 0 switches the yaw clamp off).
    yaw_limit: f32,
}

/// The size the solver gives each point (camera slots 285 / 286, 0x1406245C0: the constant 50.0).
pub const FRAME_POINT_SIZE: f32 = 50.0;
/// The lead factor and the release time of the obstruction blend ([I] 0x1406100D0).
pub const LEAD_FACTOR: f32 = 0.24;
pub const OBSTRUCTION_RELEASE: f32 = 0.6;

/// A view for the solver's screen tests: the camera of the previous update.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Screen {
    pub position: Vec3,
    pub rotation: Rot,
    /// Horizontal field of view, degrees, and height / width.
    pub fov: f32,
    pub aspect: f32,
}

impl Screen {
    /// The projected point (x right, y up, both 1.0 at the frame's edge) and whether it is in
    /// front of the camera.
    pub fn project(&self, point: Vec3) -> (Vec2, bool) {
        let forward = self.rotation.forward();
        let right = Rot::new(0.0, self.rotation.yaw + 90.0, 0.0).forward();
        let up = right.cross(forward) * -1.0;
        let d = point - self.position;
        let w = d.dot(forward);
        if w <= 0.0 {
            return (Vec2::ZERO, false);
        }
        let half = (self.fov * 0.5).to_radians().tan();
        (Vec2::new(d.dot(right) / (w * half), d.dot(up) / (w * half * self.aspect)), true)
    }

    /// The clamped screen position the lead uses: each axis into [-1, 1]; zero behind the camera.
    fn clamped(&self, point: Vec3) -> Vec2 {
        let (s, front) = self.project(point);
        if front { s.clamp(Vec2::splat(-1.0), Vec2::splat(1.0)) } else { Vec2::ZERO }
    }
}

/// One fit (0x1405E0590 with its flag set): from camera position `cam` looking along `rot`, the
/// look point and the distance that frame the points `a` and `b`.
pub fn fit(cam: Vec3, a: Vec3, b: Vec3, rot: Rot, screen: &Screen) -> (Vec3, f32) {
    let (pa, pb) = (a - cam, b - cam);
    let (far, near) = if pa.length() <= pb.length() { (pb, pa) } else { (pa, pb) };
    let back = -rot.forward();
    // The far point on the near point's depth plane, along its ray from the camera.
    let mirrored = -far;
    let v = far - mirrored;
    let den = v.dot(back);
    let q = if den >= 0.0 {
        far
    } else {
        let s = (near.dot(back) - mirrored.dot(back)) / den;
        if (0.0..=1.0).contains(&s) { mirrored + v * s } else { far }
    };
    let u = (near - q).normalize_or_zero();
    let (e1, e2) = (q - u * FRAME_POINT_SIZE, near + u * FRAME_POINT_SIZE);
    let look = (e1 + e2) * 0.5 + cam;
    let half = (e2 - e1).length() * 0.5;
    // How upright the segment stands on screen: 0 across, 1 upright.
    let on_screen = (screen.project(e2 + cam).0 - screen.project(e1 + cam).0).normalize_or_zero();
    let w = 1.0 - (on_screen.y.abs() * std::f32::consts::FRAC_PI_2).cos();
    let a0 = std::f32::consts::FRAC_PI_2 - (screen.fov * 0.5).to_radians();
    let upright = (a0.tan() * half).atan2(screen.aspect * half);
    let angle = a0 + (upright - a0) * w;
    (look, angle.tan() * half * (1.1 - 0.1 * w))
}

/// The framing solver (0x1405DEFA0): the start camera sits behind the midpoint at
/// |b - a| x 0.5 / sin(FOV / 2); one fit from there and three more, each from the camera position
/// the previous one gives. Points less than 1 cm apart horizontally keep `look` and get the
/// start distance.
pub fn frame2(a: Vec3, b: Vec3, rot: Rot, screen: &Screen, look: Vec3) -> (Vec3, f32) {
    let mid = (a + b) * 0.5;
    let start = (b - a).length() * 0.5 / (screen.fov * 0.5).to_radians().sin();
    if (b - a).truncate().length_squared() < 1.0 {
        return (look, start);
    }
    let (mut look, mut distance) = fit(mid - rot.forward() * start, a, b, rot, screen);
    for _ in 0..3 {
        (look, distance) = fit(look - rot.forward() * distance, a, b, rot, screen);
    }
    (look, distance)
}

/// `LookAt` (slot 312 0x140613300): pitch = atan2(dz, |dxy|), yaw = atan2(dy, dx) + 0.08;
/// closer than 1 cm horizontally: pitch 0 and the followed actor's yaw.
pub fn look_at(from: Vec3, to: Vec3, actor_yaw: f32) -> Rot {
    let d = to - from;
    let flat = Vec2::new(d.x, d.y);
    if flat.length_squared() < 1.0 {
        return Rot::new(0.0, actor_yaw, 0.0);
    }
    Rot::new(d.z.atan2(flat.length()).to_degrees(), normalize_axis(d.y.atan2(d.x).to_degrees() + C.lockon_cam_look_yaw_bias), 0.0)
}

/// The base rotation (slot 299 0x140606190).
pub fn base_rotation(player: Vec3, target: Vec3, actor_yaw: f32) -> Rot {
    let look = look_at(player, target, actor_yaw);
    if look.pitch >= C.lockon_cam_base_pitch_floor {
        let mut base = Rot::new(look.pitch + C.lockon_cam_base_pitch_offset, look.yaw, 0.0);
        if base.pitch < C.lockon_cam_base_pitch_floor {
            base.pitch = C.lockon_cam_base_pitch_floor;
        }
        base
    } else {
        // The target is more than 10 degrees below: straight at it.
        look
    }
}

/// The stick-speed scale (slot 266 0x140607390).
pub fn stick_scale(distance: f32) -> f32 {
    let (hi, lo, far) = (C.lockon_cam_stick_far_dist.max(C.lockon_cam_stick_near_dist), C.lockon_cam_stick_near_dist, C.lockon_cam_stick_far_rate);
    if hi > lo {
        ((hi - lo) - (distance.clamp(lo, hi) - lo)) / (hi - lo) * (1.0 - far) + far
    } else {
        far
    }
}

/// The clamp (0x140611760, decompiled C).
pub fn clamp_rotation(rot: Rot, player: Vec3, target: Vec3, actor_yaw: f32) -> Rot {
    clamp_rotation_limited(rot, player, target, actor_yaw, C.lockon_cam_yaw_limit)
}

/// The clamp with the camera's own yaw limit (+0x888): "skipped when the limit is 0" (7.12).
pub fn clamp_rotation_limited(rot: Rot, player: Vec3, target: Vec3, actor_yaw: f32, yaw_limit: f32) -> Rot {
    let l = look_at(player, (player + target) * 0.5, actor_yaw);
    let max = if l.pitch >= 0.0 { l.pitch.min(C.lockon_cam_pitch_max) } else { 0.0 };
    let pitch = normalize_axis(rot.pitch).clamp(C.lockon_cam_pitch_min, max.max(C.lockon_cam_pitch_min));
    if yaw_limit <= 0.0 {
        return Rot::new(pitch, normalize_axis(rot.yaw), rot.roll);
    }
    let off = normalize_axis(rot.yaw - l.yaw).clamp(-yaw_limit, yaw_limit);
    Rot::new(pitch, normalize_axis(l.yaw + off), rot.roll)
}

/// The curve of the game's interpolator by its type (0x1411A0E70, the dispatch read in the
/// disassembly: 2 -> sin(t pi/2), 3 -> 1 - cos(t pi/2), 4 -> (1 - cos(t pi)) / 2, 5 -> 1 - (1 - t)^2,
/// 6 -> t^2, anything else linear). The obstruction release uses type 2, the attack camera's
/// lag tweens type 5.
pub fn interp_curve(kind: u8, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match kind {
        2 => (t * std::f32::consts::FRAC_PI_2).sin(),
        3 => 1.0 - (t * std::f32::consts::FRAC_PI_2).cos(),
        4 => (1.0 - (t * std::f32::consts::PI).cos()) * 0.5,
        5 => 1.0 - (1.0 - t) * (1.0 - t),
        6 => t * t,
        _ => t,
    }
}

impl LockonCamera {
    /// `StartTresCamera` (slot 242 0x14060BD30): wanted rotation = base, the shown rotation = it,
    /// look point = the midpoint, distance = `m_Distance` (400).
    pub fn start(p: CameraParams, target: &CameraTarget, lock: &LockTarget) -> LockonCamera {
        let player = target_point(&p, target);
        let base = base_rotation(player, lock.look_at, target.yaw);
        let look = (player + lock.look_at) * 0.5;
        let distance = C.lockon_cam_distance_min;
        let position = look - base.forward() * distance;
        let view = CameraView { position, rotation: base, rotation_target: base, look_point: look, distance, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
        LockonCamera { p, rot_target: base, rot_cur: base, look_smooth: look, dist_smooth: distance, auto_power: 0.0, stick_wait: 0.0, last_target: lock.look_at, position, first: true, view, lead_player: Vec3::ZERO, lead_target: Vec3::ZERO, target_before: lock.look_at, blocked_from: position, free_before: position, release: OBSTRUCTION_RELEASE, yaw_limit: C.lockon_cam_yaw_limit }
    }

    pub fn view(&self) -> CameraView {
        self.view
    }

    /// `Reset` (R3; slot 244 0x140609DF0): the wanted rotation back on `base` (the distance
    /// restored): R3 while locked re-centres on the target.
    pub fn reset(&mut self, target: &CameraTarget) {
        let player = target_point(&self.p, target);
        self.rot_target = base_rotation(player, self.last_target, target.yaw);
    }

    /// The target is on screen for the view of the last step (the projection saved from the
    /// local player): |x| <= 1, |y| <= 1, w > 0.
    fn on_screen(&self, point: Vec3) -> bool {
        let forward = self.rot_cur.forward();
        let right = Rot::new(0.0, self.rot_cur.yaw + 90.0, 0.0).forward();
        let up = right.cross(forward) * -1.0;
        let d = point - self.position;
        let w = d.dot(forward);
        if w <= 0.0 {
            return false;
        }
        let half = (self.p.fov_horizontal * 0.5).to_radians().tan();
        let (x, y) = (d.dot(right) / (w * half), d.dot(up) / (w * half / self.p.fov_aspect_ratio));
        let limit = C.lockon_cam_offscreen_snap;
        x.abs() <= limit && y.abs() <= limit
    }

    /// The attack camera's writes on this camera (0x1405C63A0 / 0x1405D4730: the look-point rate
    /// +0x450 and lag limit +0x44C; 0x14060A7D0 -> vt+0x940: the yaw limit; `None` = 120).
    pub fn set_attack_params(&mut self, look_lerp_rate: f32, look_lag_limit: f32, yaw_limit: Option<f32>) {
        (self.p.look_lerp_rate, self.p.look_lag_limit) = (look_lerp_rate, look_lag_limit);
        self.yaw_limit = yaw_limit.unwrap_or(C.lockon_cam_yaw_limit);
    }

    /// One update: `0x14060EDB0(this, dt)`: the rotation step 0x140611170, then the position step
    /// 0x1406100D0. `lock` = `None` when the target is gone (its last position stands).
    pub fn step(&mut self, dt: f32, input: &CameraInput, field: &FieldCamera, target: &CameraTarget, lock: Option<&LockTarget>, world: &dyn World) -> CameraView {
        let p = self.p;
        let player = target_point(&p, target);
        // The target point; a jump of more than 3000 cm/s is approached with rate 6.
        if let Some(lock) = lock {
            let jump = (lock.look_at - self.last_target).length();
            self.last_target = if !self.first && jump > C.lockon_cam_target_jump_speed * dt { lerp_step_vec(&p, self.last_target, lock.look_at, 6.0, dt) } else { lock.look_at };
        }
        let t = self.last_target;
        if input.recentre {
            self.reset(target);
        }
        let base = base_rotation(player, t, target.yaw);

        // ---- rotation step
        let d3 = (player - t).length();
        let delta = field.stick_delta(input, dt);
        let stick_active = delta != Rot::default();
        let scale = stick_scale(d3);
        self.rot_target.yaw += delta.yaw * scale;
        self.rot_target.pitch += delta.pitch * scale;
        self.rot_target = self.rot_target.normalized();
        let followed = self.auto_follow(target, base, player, t, dt, stick_active);
        if !stick_active && self.stick_wait <= 0.0 && !self.first && !self.on_screen(t) {
            // The target left the screen: face it at once.
            self.rot_target = base;
        }
        self.rot_target = clamp_rotation_limited(self.rot_target, player, t, target.yaw, self.yaw_limit);
        self.rot_cur = if self.first { self.rot_target } else { rot_lerp_step(&p, self.rot_cur, self.rot_target, p.rotation_lerp_rate, dt) };

        // ---- position step (0x1406100D0)
        let screen = Screen { position: self.position, rotation: self.view.rotation, fov: p.fov_horizontal, aspect: 1.0 / p.fov_aspect_ratio };
        // The lead: each point along its owner's velocity by 0.24 x |its clamped screen
        // position|, only when moving on carries it away from the other on screen; smoothed.
        let (mut pp, mut tt) = (player, t);
        if !self.first {
            let (sp, st) = (screen.clamped(player), screen.clamped(t));
            let v = target.velocity;
            let lead = if (sp - st).length() < (screen.clamped(player + v) - st).length() { v } else { Vec3::ZERO } * LEAD_FACTOR * sp.length();
            self.lead_player = lerp_step_vec(&p, self.lead_player, lead, p.look_lerp_rate, dt);
            let vt = if dt > 0.0 && lock.is_some() { (t - self.target_before) / dt } else { Vec3::ZERO };
            let lead = if (screen.clamped(t + vt) - sp).length() <= (st - sp).length() { Vec3::ZERO } else { vt } * LEAD_FACTOR * st.length();
            self.lead_target = lerp_step_vec(&p, self.lead_target, lead, p.look_lerp_rate, dt);
            pp += self.lead_player;
            tt += self.lead_target;
        }
        self.target_before = t;
        // The framing solver (0x1405DEFA0), then the minimum distance `m_Distance` (400).
        let (look, distance) = frame2(tt, pp, self.rot_cur, &screen, self.look_smooth);
        let distance = distance.max(C.lockon_cam_distance_min);
        if self.first {
            (self.look_smooth, self.dist_smooth) = (look, C.lockon_cam_distance_min.max(self.dist_smooth));
        } else {
            self.look_smooth = lerp_step_vec(&p, self.look_smooth, look, p.look_lerp_rate, dt);
            let behind = self.look_smooth - look;
            if p.look_lag_limit >= 0.0 && behind.length() > p.look_lag_limit {
                self.look_smooth = look + behind.normalize() * p.look_lag_limit;
            }
        }
        self.dist_smooth = lerp_step(&p, self.dist_smooth, distance, C.lockon_cam_distance_rate, dt);
        let mut cam = self.look_smooth - self.rot_cur.forward() * self.dist_smooth;
        let mut collided = false;
        // The obstruction blend (0x14061BA10): the camera collision from the player point to
        // the camera; blocked: the camera is pulled in and the 0.6 s release ease is armed;
        // free again: it eases from where it was held to the free position, the start of the
        // ease travelling with the camera.
        if !p.collision_no_collision_config {
            let r = p.collision_radius;
            let free = cam;
            let blocked = world.sweep_capsule(player, cam, r, r).filter(|hit| !hit.start_penetrating);
            if let Some(hit) = blocked {
                cam = player + (cam - player) * hit.time.clamp(0.0, 1.0);
                (self.blocked_from, self.release, collided) = (cam, 0.0, true);
            } else if self.release < OBSTRUCTION_RELEASE {
                self.blocked_from += free - self.free_before;
                self.release += dt;
                let x = (self.release / OBSTRUCTION_RELEASE).clamp(0.0, 1.0);
                // Interpolator type 2 (0x141194110(.., 2, from, to, 0.6) -> 0x1411A0E70): sin(x pi/2).
                cam = self.blocked_from.lerp(free, interp_curve(2, x));
            }
            self.free_before = free;
        }
        // The final camera collision (0x14061CB40, camera spec 3.3).
        if !p.collision_no_collision_config {
            let r = p.collision_radius;
            if let Some(hit) = world.sweep_capsule(self.look_smooth, cam, r, r) {
                if !hit.start_penetrating {
                    cam = self.look_smooth + (cam - self.look_smooth) * hit.time.clamp(0.0, 1.0);
                    collided = true;
                }
            }
        }
        self.position = cam;
        self.first = false;
        self.view = CameraView {
            position: cam,
            rotation: self.rot_cur,
            rotation_target: self.rot_target,
            look_point: self.look_smooth,
            distance: (self.look_smooth - cam).length(),
            auto_power: self.auto_power,
            stick_wait: self.stick_wait,
            still_wait: 0.0,
            reset_wait: 0.0,
            slope_pitch: 0.0,
            stick_active,
            followed,
            collided,
        };
        self.view
    }

    /// `AutoFollow` (0x1405FFA60) with the lock-on branch (0x1405FFC8E - 0x1405FFEBA).
    fn auto_follow(&mut self, target: &CameraTarget, base: Rot, player: Vec3, t: Vec3, dt: f32, stick_active: bool) -> bool {
        let p = self.p;
        if stick_active {
            self.stick_wait = p.auto_wait_after_stick;
        } else if self.stick_wait > 0.0 {
            self.stick_wait -= dt;
            if self.stick_wait <= 0.0 {
                self.stick_wait = 0.0;
                self.auto_power = 0.0;
            }
        }
        if !(p.auto_option_default && p.auto_enabled_flag && self.stick_wait <= 0.0) || stick_active {
            return false;
        }
        // The velocity that drives the follow rate: 0 in Idle; blended toward (max speed, 0, 0)
        // by the distance to the target (XY: `m_bLockonAutoRotationDist3` is false).
        let mut v = if target.state == TargetState::Idle { Vec3::ZERO } else { target.velocity };
        let (low, high) = (C.lockon_cam_auto_low_dist, C.lockon_cam_auto_high_dist);
        let mut gain = 1.0;
        if high > 0.0 && low > 0.0 && high > low {
            let d = (t - player).truncate().length();
            let k = if d < low { 0.0 } else if d > high { 1.0 } else { (d - low) / (high - low) };
            v += (Vec3::new(p.auto_speed_reference, 0.0, 0.0) - v) * k;
            gain = 1.0 + (C.lockon_cam_auto_rate_gain - 1.0) * k;
        }
        if v.length() <= p.auto_min_speed {
            self.auto_power = 0.0;
            return false;
        }
        self.auto_power = (self.auto_power + p.auto_power_max * dt).min(p.auto_power_max);
        let rate = v.truncate().length() / p.auto_speed_reference * self.auto_power * gain;
        let mut want = base;
        want.roll = self.rot_target.roll;
        // The "running toward the camera" exception compares against the base yaw unchanged.
        let actor = Rot::new(0.0, target.yaw, 0.0).forward();
        let cam = self.rot_cur.forward();
        if Vec3::new(cam.x, cam.y, 0.0).normalize_or_zero().dot(actor) < p.auto_facing_threshold {
            want.yaw = self.rot_target.yaw;
        }
        self.rot_target = rot_lerp_step(&p, self.rot_target, want, rate, dt).normalized();
        true
    }
}
