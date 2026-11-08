//! (F-HUD) The formchange cameras, traced from the KH3 decomp (decompiled C in re/decomp, the blend blocks read as
//! `FViewTargetTransitionParams` {BlendTime, BlendFunction, BlendExp, ...} as docs/sora-magic-spec.md 2870 decodes them):
//!
//! - THE SHOOTER CAMERA (`ATresCameraShooterMode`, constructor 0x1405FD4E0: a `TresCameraNormal` with distance 200, pitch
//!   limits -45 / 45, start pitch -10, `m_CameraLocalOffset` (0, -50, 0), stick mode 2, priority 120 (`0x1405D7840(0x78)`
//!   writes camera+0x3E8)). ShooterMode's SetCamera (slot 114 0x1413BED50) spawns it, `SetFieldOfView(60)` (0x1405D6F40:
//!   the camera component's FieldOfView, clamped 1 .. 179: `aim::CAMERA_VALUE` IS THE FOV), `m_CameraLocalOffset` =
//!   (30, -65, 45) (0x140626FC0 writes camera+0x66C = `m_CameraLocalOffset`, build/types: ATresCameraShooterMode 1644; the
//!   offset is turned by the camera rotation and added to the target point, docs/sora-camera-spec.md 2.3.1), the look-at
//!   point camera+0x9E0 = eye + Rot(pitch, yaw) x distance; it is put on the stack with {0.0 s, ease-out, 2.0} and removed
//!   with the SAME block: BlendTime 0.0 = A CUT both ways (the 2.0 that F-ARROWGUN's `aim::CAMERA_BLEND_OUT` holds is the
//!   block's BlendExp, not a time). The camera's rotation follows the state's look-at point (the state writes +0x9E0 every
//!   tick from its aim; the right stick goes to the state, `Sora::set_shooter_look`) [I: the reader of +0x9E0 in the
//!   TresCameraNormal update is not traced; the camera's own stick part 0x140608130 is not ported, its input being the same
//!   right stick the state turns the aim with]. Target point: the socket `Zup_Cam` (102.338 above the actor origin), no
//!   `m_TargetOffset` (the native class has no blueprint) [I].
//! - COUNTER SHIELD'S ULTIMATE FINISH (FINISH_SH, StyleFinishSH Tick 0x1413D5E50): sub-step 0: an `ATresCameraFix` on Sora
//!   (vt+0x788(pawn, ctrl)), `SetFieldOfView(100)`, priority 120, {0.0 s, ease-out, 2.0} = a cut: it holds the view it
//!   starts on (`ATresCameraFix` 0x1405EE200 copies the current camera) [I for the copy]; sub-step 2: an `ATresCameraNormal`
//!   on Pegasus: `m_TargetInfo.m_TargetOffset` (camera+0x678, 0x140627E30) = (0, 0, 100), `m_Rotation` (+0x828, 0x14060A640)
//!   = (pitch -25, 0, 0) (the rest rotation behind the target), distance 700 (0x1405EFB60), priority 120, a cut in; the Fix
//!   is removed with {0.5 s, ease-out, 2.0}; the cleanup (0x1413CFB60) removes both with {0.5 s, ease-out, 2.0}. No FOV is
//!   set on the CameraNormal: the camera component's default 90 [I: UE's UCameraComponent default]. Sora is linked to
//!   Pegasus (X_Link_skt) during the ride, so the camera follows Sora's actor [I: within the socket's offset].
//!   CORRECTION to F-SHIELD's `shield::finish_camera` values: its "distance 100" is the Fix's FOV 100, its "fov 120" is the
//!   priority 0x78, its "2 s blend in" is the BlendExp of a 0.0 s block. This module keeps its own traced constants and uses
//!   `finish_camera` only for the phase.
//! - MIRAGE STAFF (StartStyle case 0x88 0x14141A9D0): the normal camera's distance scale camera+0x804 = 1.35 while the form
//!   lasts (ctrl+0xAE0 = 1); the attack camera's "off" (0x1414122A0) restores 1.35, not 1.0, while ctrl+0xAE0 is set; mode
//!   5 sets 1.6. [`field_distance_scale`].
//! - THE MULTI-LOCK CAMERA (`ATresCameraMultiLock`, constructor 0x1405E2F50: a `TresCameraNormal` with `m_TargetOffset`
//!   (0, 0, 48), SetFieldOfView(100), `m_AutoTurningPower` 2.5, priority 120), put on the stack by slot 790 0x141419C00
//!   with {0.5 s, ease-out, 2.0} and removed by slot 791 0x1413FA990 with the same block. Its rest rotation (slot 299
//!   0x1405EBF60) is `m_Rotation` with the yaw of (centre - its target) while the controller has a centre (slot 0x18D8
//!   0x1413FF060: the mean of the listed targets' points) and this camera is the current one; else the actor's yaw.

use glam::{Vec2, Vec3};
use kh_core::combat::forms::arrowgun::aim::{Aim, ShooterView, CAMERA_OFFSET, CAMERA_VALUE};
use kh_core::combat::forms::shield;
use kh_core::World;

use crate::rig::{blend_views, ease_out};
use crate::{normalize_axis, CameraInput, CameraParams, CameraTarget, CameraView, FieldCamera, Rot, CAMERA_PARAMS};

/// A camera-stack blend block (`FViewTargetTransitionParams`): BlendTime seconds, BlendExp of `VTBlend_EaseOut` (3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blend {
    pub time: f32,
    pub exponent: f32,
}

/// {0.0, 3, 2.0, 2}: the shooter camera's in and out, the shield finish cameras' in: a cut.
pub const BLEND_CUT: Blend = Blend { time: 0.0, exponent: 2.0 };
/// {0.5, 3, 2.0, 2}: the shield finish cameras' removal (0x1413D5E50 sub 2 for the Fix, 0x1413CFB60 for both).
pub const BLEND_HALF: Blend = Blend { time: 0.5, exponent: 2.0 };

/// ATresCameraShooterMode's constructor (0x1405FD4E0) and ShooterMode's SetCamera (0x1413BED50).
pub const SHOOTER_DISTANCE: f32 = 200.0;
pub const SHOOTER_PITCH_LIMIT: f32 = 45.0;
pub const SHOOTER_FOV: f32 = CAMERA_VALUE;
pub const SHOOTER_LOCAL_OFFSET: Vec3 = CAMERA_OFFSET;
pub const CAMERA_PRIORITY: i32 = 0x78;
/// The sweep radius of the camera's collision (the port's camera probe, as aim.rs).
const PROBE_RADIUS: f32 = 20.0;

/// StyleFinishSH's cameras (0x1413D5E50).
pub const SHIELD_FIX_FOV: f32 = 100.0;
pub const SHIELD_RIDE_DISTANCE: f32 = 700.0;
pub const SHIELD_RIDE_PITCH: f32 = -25.0;
pub const SHIELD_RIDE_TARGET_OFFSET_Z: f32 = 100.0;
/// The native camera component's FieldOfView (no SetFieldOfView on the ride camera) [I].
pub const NATIVE_FOV: f32 = 90.0;

/// StartStyle 0x88's camera+0x804 and the attack camera's wide mode 5 (combat/camera.rs).
pub const STAFF_DISTANCE_SCALE: f32 = 1.35;
pub const ATTACK_WIDE_SCALE: f32 = 1.6;

/// The distance scale of the normal camera (camera+0x804) for this step: the attack camera's wide mode 5 (1.6) and its modes
/// 4 / 6 / 7 (1.0, written by 0x1414122A0 with their lag) win; else
/// the form's scale (`kh_core::combat::forms::staff::camera_distance_scale`: 1.35 in Mirage Staff, else 1.0), which is
/// also what the attack camera's "off" restores (0x1414122A0 with ctrl+0xAE0). Hosts write it into
/// `FieldCamera::params_mut().distance_scale` EVERY step (the rig's `set_attack_mode` writes absolute values).
pub fn field_distance_scale(attack_mode: u8, form_scale: f32) -> f32 {
    match attack_mode {
        5 => ATTACK_WIDE_SCALE,
        4 | 6 | 7 => 1.0,
        _ => form_scale,
    }
}

/// The rotation matrix's axes of `r` (UE: X forward, Y right, Z up).
fn axes(r: Rot) -> (Vec3, Vec3, Vec3) {
    let forward = r.forward();
    let yaw = r.yaw.to_radians();
    let right = Vec3::new(-yaw.sin(), yaw.cos(), 0.0);
    let up = -right.cross(forward);
    (forward, right, up)
}

/// The shooter camera's look point for `rotation`: the target point + RotationMatrix(rotation) x `m_CameraLocalOffset`.
pub fn shooter_look_point(target: &CameraTarget, rotation: Rot) -> Vec3 {
    look_point_with(target, rotation, SHOOTER_LOCAL_OFFSET)
}

/// A TresCameraShooterMode's look point with the `m_CameraLocalOffset` `offset`.
pub fn look_point_with(target: &CameraTarget, rotation: Rot, offset: Vec3) -> Vec3 {
    let (f, r, u) = axes(rotation);
    target.position + Vec3::Z * CAMERA_PARAMS.target_socket_height + f * offset.x + r * offset.y + u * offset.z
}

/// The rotation whose forward ray from the look point passes through `look_at` (the state's +0x9E0), pitch within the
/// camera's limits (the look point depends on the rotation: fixed-point steps).
pub fn shooter_rotation(target: &CameraTarget, look_at: Vec3, start: Rot) -> Rot {
    rotation_through(target, look_at, start, SHOOTER_LOCAL_OFFSET)
}

/// [`shooter_rotation`] for a camera of that class with the `m_CameraLocalOffset` `offset`.
pub fn rotation_through(target: &CameraTarget, look_at: Vec3, start: Rot, offset: Vec3) -> Rot {
    let mut rotation = Rot::new(start.pitch.clamp(-SHOOTER_PITCH_LIMIT, SHOOTER_PITCH_LIMIT), start.yaw, 0.0);
    // Damped steps: with a large offset (mode 8's (-300, -200, 100)) the look point turns with the camera, and full steps
    // overshoot.
    for _ in 0..64 {
        let d = look_at - look_point_with(target, rotation, offset);
        let flat = Vec2::new(d.x, d.y).length();
        if flat < 1.0 {
            break;
        }
        let aim = Rot::new(d.z.atan2(flat).to_degrees(), d.y.atan2(d.x).to_degrees(), 0.0);
        let next = Rot::new((rotation.pitch + (aim.pitch - rotation.pitch) * 0.5).clamp(-SHOOTER_PITCH_LIMIT, SHOOTER_PITCH_LIMIT), normalize_axis(rotation.yaw + normalize_axis(aim.yaw - rotation.yaw) * 0.5), 0.0);
        let moved = normalize_axis(next.yaw - rotation.yaw).abs() + (next.pitch - rotation.pitch).abs();
        rotation = next;
        if moved < 1.0e-3 {
            break;
        }
    }
    rotation
}

/// Places a camera `distance` behind `look` along `rotation`, with the probe sweep from the look point.
fn place(look: Vec3, rotation: Rot, distance: f32, like: &CameraView, world: Option<&dyn World>) -> CameraView {
    let wanted = look - rotation.forward() * distance;
    let (position, collided) = match world.and_then(|w| w.sweep_capsule(look, wanted, PROBE_RADIUS, PROBE_RADIUS)) {
        Some(hit) => (look + (wanted - look) * hit.time, true),
        None => (wanted, false),
    };
    CameraView { position, rotation, rotation_target: rotation, look_point: look, distance: (look - position).length(), stick_active: false, followed: false, collided, ..*like }
}

/// The shooter camera's view for this step (while `view.camera`).
pub fn shooter_view(target: &CameraTarget, view: &ShooterView, previous: Option<Rot>, like: &CameraView, world: Option<&dyn World>) -> CameraView {
    let start = previous.unwrap_or(Rot::new(view.pitch, view.yaw, 0.0));
    let rotation = shooter_rotation(target, view.look_at, start);
    place(shooter_look_point(target, rotation), rotation, SHOOTER_DISTANCE, like, world)
}

/// The phase of a form camera this step (what [`FormCameras::step`] reads from the core).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FormShot {
    /// ShooterMode's camera (the state's view).
    Shooter(ShooterView),
    /// FINISH_SH sub 0 .. 1: the fixed camera.
    ShieldFix,
    /// FINISH_SH sub 2 .. 4: the camera behind Pegasus.
    ShieldRide,
    /// The multi-lock mode is on (ctrl+0x9C4): its camera, turned toward `centre` = the mean of the listed targets'
    /// points when the list has any (controller slot 0x18D8 0x1413FF060; None = an empty list: behind Sora).
    MultiLock(Option<Vec3>),
    /// (SHOT-FIX follow-up) The attack camera's modes with a camera of their own (controller 0x1414122A0 cases 1, 2, 8).
    Attack(AttackShot),
    /// (AUDIT-B F-NANO) Nano Gear is held (or Nano Arms): its camera blueprint polls Sora (kh_camera::forms_nano).
    Nano(crate::forms_nano::NanoShot),
}

/// The attack camera of modes 1 / 2 / 8 as the core holds it (`sora.combat().attack_cam`): the mode, its target and that
/// target's point now (its lock-on point; None = gone), the request's anchor (`pPosition`, controller+0xAC8).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttackShot {
    pub mode: u8,
    pub target: kh_core::host::TargetId,
    pub point: Option<Vec3>,
    pub anchor: Option<Vec3>,
}

impl FormShot {
    /// The attack camera's own cameras (modes 1, 2, 8; the other modes are the rig's `set_attack_mode` / `set_lockon`).
    /// Hosts ask this first: these run during shotlocks and finishers, when no form camera is up.
    pub fn attack(cam: &kh_core::combat::camera::AttackCam, point_of: &dyn Fn(kh_core::host::TargetId) -> Option<Vec3>) -> Option<FormShot> {
        let point = (cam.target != kh_core::host::TargetId::NONE).then(|| point_of(cam.target)).flatten();
        matches!(cam.mode, 1 | 2 | 8).then_some(FormShot::Attack(AttackShot { mode: cam.mode, target: cam.target, point, anchor: cam.anchor }))
    }

    /// From the core: `aim` = `sora.combat().arrowgun.aim`, `finish` = `sora.finish()`, `form_cmd` =
    /// `sora.combat().form.cmd`, `point_of` = a listed target's point (its lock-on point; None = gone). The ShooterMode
    /// camera first, then the finish, then the multi-lock (all three priority 120: the newest on top; the three never
    /// run together in KH3: ShooterMode ends the multi-lock (aim.rs step 6), the finish ends the form's aim).
    pub fn of(aim: &Aim, finish: Option<kh_core::states::FinishView>, form_cmd: i32, point_of: &dyn Fn(kh_core::host::TargetId) -> Option<Vec3>) -> Option<FormShot> {
        if aim.view.camera {
            return Some(FormShot::Shooter(aim.view));
        }
        if let Some(c) = shield::finish_camera(finish, form_cmd) {
            return Some(if c.on_pegasus { FormShot::ShieldRide } else { FormShot::ShieldFix });
        }
        aim.multilock.then(|| FormShot::MultiLock(multilock_centre(&aim.switch_list, point_of)))
    }
}

/// What a host's camera takes from the core each step (kh-er cam.rs, the test app): KH3's attack camera of modes 1 / 2 / 8
/// first (it runs inside shotlocks and finishers), then the form cameras ([`FormShot::of`]); `attack_own` = KH3's attack
/// camera has the view, so the port's own shot camera (`kh_camera::shot`, a port design) must not draw over it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Selection {
    pub shot: Option<FormShot>,
    pub attack_own: bool,
}

/// The selection of this step for Sora (`friend_leads`: the camera follows a friend: nothing of Sora's own).
pub fn select(sora: &kh_core::Sora, friend_leads: bool, point_of: &dyn Fn(kh_core::host::TargetId) -> Option<Vec3>) -> Selection {
    if friend_leads {
        return Selection { shot: None, attack_own: false };
    }
    let combat = sora.combat();
    let attack = FormShot::attack(&combat.attack_cam, point_of);
    let attack_own = attack.is_some();
    // (AUDIT-B F-NANO) Nano Gear's camera blueprint (priority 200) listens while Nano Gear / Nano Arms is held
    let nano = (combat.form.cmd == kh_core::combat::forms::nano::CHANGE_GM || sora.status().weapon == "WEP_KEYBLADE_SO_08").then(|| FormShot::Nano(crate::forms_nano::NanoShot::of(sora)));
    Selection { shot: attack.or(nano).or_else(|| FormShot::of(&combat.arrowgun.aim, sora.finish(), combat.form.cmd, point_of)), attack_own }
}

/// The camera log's name of what the form cameras show (None: nothing of theirs this step).
pub fn mode_label(selection: &Selection, shown: bool) -> Option<&'static str> {
    shown.then_some(if selection.attack_own { "attack" } else { "form" })
}

/// Controller slot 0x18D8 (0x1413FF060): with the multi-lock on and a list, the mean of the listed targets' points
/// (vt+0xB88 of each live target not tagged "Gimmick"; the port's targets are the lock-on's, none a gimmick).
pub fn multilock_centre(list: &[kh_core::host::TargetId], point_of: &dyn Fn(kh_core::host::TargetId) -> Option<Vec3>) -> Option<Vec3> {
    let points: Vec<Vec3> = list.iter().filter_map(|t| point_of(*t)).collect();
    (!points.is_empty()).then(|| points.iter().copied().sum::<Vec3>() / points.len() as f32)
}

/// The view a host shows for a form camera: the view, its horizontal FOV (degrees), and whether the right stick is the
/// form's (ShooterMode: it goes to `Sora::set_shooter_look`, not to the orbit).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FormView {
    pub view: CameraView,
    pub fov: f32,
    pub stick_taken: bool,
    /// This step's view does not continue the last one (a cut): no interpolation between the two.
    pub cut: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum Running {
    Shooter(Rot),
    Fix(CameraView),
    Ride(Box<FieldCamera>),
    MultiLock(Box<FieldCamera>),
    /// Modes 1 / 2: `ATresCameraRotTargetAuto` on the target (its absolute rotation, distance, last target point).
    Orbit { mode: u8, target: kh_core::host::TargetId, rotation: Rot, point: Vec3 },
    /// Mode 8: an `ATresCameraShooterMode` on Sora looking at a fixed point (its rotation of the last step).
    Over { look_at: Vec3, rotation: Rot },
}

impl Running {
    fn same(&self, shot: &FormShot) -> bool {
        matches!((self, shot), (Running::Shooter(_), FormShot::Shooter(_)) | (Running::Fix(_), FormShot::ShieldFix) | (Running::Ride(_), FormShot::ShieldRide) | (Running::MultiLock(_), FormShot::MultiLock(_)))
            || matches!((self, shot), (Running::Orbit { mode, target, .. }, FormShot::Attack(a)) if *mode == a.mode && *target == a.target)
            || matches!((self, shot), (Running::Over { .. }, FormShot::Attack(a)) if a.mode == 8)
    }

    /// The block it leaves with.
    fn blend_out(&self) -> Blend {
        match self {
            Running::Shooter(_) => BLEND_CUT,
            Running::Fix(_) | Running::Ride(_) | Running::MultiLock(_) | Running::Orbit { .. } | Running::Over { .. } => BLEND_HALF,
        }
    }
}

/// The form cameras on top of the field camera (the host keeps one beside its rig, steps it after the rig and shows
/// what it returns).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct FormCameras {
    running: Option<Running>,
    /// The blend in of the running camera: (seconds, block).
    inn: Option<(f32, Blend)>,
    /// The blend out in progress: (last view of the form camera, its FOV, seconds, the block).
    out: Option<(CameraView, f32, f32, Blend)>,
    last: Option<(CameraView, f32)>,
    /// Mode 2's pitch (controller+0xAC0, param_1[0x158]), the target it was drawn for (+0xAD8), the draws' state.
    attack_pitch: f32,
    attack_target: u32,
    rng: u32,
    /// (AUDIT-B F-NANO) Nano Gear's camera blueprint.
    pub nano: crate::forms_nano::NanoCameras,
}

/// The attack camera of modes 1 / 2 / 8 (controller 0x1414122A0, decompiled C):
/// - 1: `ATresCameraRotTargetAuto` (spawned by 0x1413E66D0) on the target actor, the lock-on camera set on it
///   (0x1405F0140), `m_Distance` 750, `m_Rotation` (-20, 0, 0), no offsets (0x14060B9C0 writes +0x800 / +0x828 / +0x678 /
///   +0x684), the auto rotation +0x834 / +0x838 / +0x83C = (0, -30, 0) degrees a second (0x1405FE810: `m_Rotation` +=
///   rate x dt each update), put on with {1.0 s, ease-out, 2.0}.
/// - 2: the same class on the target's lock-on component (0x14061B1D0: +0x6A0), distance 600, `m_Rotation` (pitch, 0, 0)
///   with the controller's pitch +0xAC0, which a NEW target moves by a draw of -20 .. 0 (FRand x 20 - 20) and wraps by +45
///   below -45; with an anchor the outside-wall check type 2 at it (0x1406275A0: +0x6B4 = 2, +0x6B8 = the anchor); the
///   same auto rotation; on with {0.1 s, ease-out, 2.0}. Only made when none is up (`if (param_1[0x157] != 0) break`):
///   the shotlocks switch target with an off call first.
/// - 8: an `ATresCameraShooterMode` on Sora: FOV 60, `m_CameraLocalOffset` (-300, -200, 100), the look-at +0x9E0 = his
///   location + his forward x 240 (set once), on with {1.0 s, ease-out, 2.0}.
/// - Off: both cameras (+0xAB8, +0xAD0) leave with {0.5 s, ease-out, 2.0}.
///
/// [I]: the orbit camera's start yaw is the view's (0x140608CF0 takes the root's yaw when the camera starts; where the
/// manager puts it is not traced); its FOV is the component's 90; the outside-wall check (0x1406223B0) is the plain sweep
/// from the look point here; the draw is the port's generator (KH3: the global FRandomStream).
pub const ATTACK_ORBIT_DISTANCE: [f32; 2] = [750.0, 600.0];
pub const ATTACK_ORBIT_PITCH_1: f32 = -20.0;
pub const ATTACK_ORBIT_YAW_RATE: f32 = -30.0;
/// Blend in of modes 1, 2, 8.
pub const ATTACK_BLEND_IN: [Blend; 3] = [Blend { time: 1.0, exponent: 2.0 }, Blend { time: 0.1, exponent: 2.0 }, Blend { time: 1.0, exponent: 2.0 }];
pub const ATTACK_OVER_OFFSET: Vec3 = Vec3::new(-300.0, -200.0, 100.0);
pub const ATTACK_OVER_AHEAD: f32 = 240.0;

/// The ride camera's parameters: a native `ATresCameraNormal` (the field camera's update) with KH3's values.
fn ride_params() -> CameraParams {
    let mut p = CAMERA_PARAMS;
    p.distance_base = SHIELD_RIDE_DISTANCE;
    // `m_NearDistance` is the blueprint's (BP_TresCameraNormal_ex001): the native default 0 = off.
    p.distance_near = 0.0;
    p.rotation_base_pitch = SHIELD_RIDE_PITCH;
    // The target is Pegasus's root (no Zup_Cam socket on its skeleton: the root fallback 0x140621E00) [I] + (0, 0, 100).
    p.target_socket_height = 0.0;
    p.target_offset_z = SHIELD_RIDE_TARGET_OFFSET_Z;
    p
}

/// The multi-lock camera's parameters (constructor 0x1405E2F50: a `TresCameraNormal` with `m_TargetOffset` (0, 0, 48),
/// FOV 100, `m_AutoTurningPower` 2.5, priority 120; the rest the native defaults = the blueprint's except
/// `m_NearDistance`, native 0).
fn multilock_params() -> CameraParams {
    let mut p = CAMERA_PARAMS;
    p.target_offset_z = MULTILOCK_TARGET_OFFSET_Z;
    p.auto_power_max = MULTILOCK_AUTO_POWER;
    p.distance_near = 0.0;
    p
}

/// ATresCameraMultiLock's constructor values and its blend block {0.5, 3, 2.0} (slot 790 in, slot 791 out).
pub const MULTILOCK_TARGET_OFFSET_Z: f32 = 48.0;
pub const MULTILOCK_AUTO_POWER: f32 = 2.5;
pub const MULTILOCK_FOV: f32 = 100.0;
pub const MULTILOCK_BLEND: Blend = BLEND_HALF;

impl FormCameras {
    /// Mode 2's draw for a new target (0x1414122A0 case 2).
    fn draw_pitch(&mut self, target: u32) {
        if target != self.attack_target {
            // xorshift (the port's stand-in for the engine's stream), a value in 0 .. 1.
            let mut x = if self.rng == 0 { 0x2545_F491 } else { self.rng };
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            self.rng = x;
            let r = (x >> 8) as f32 / (1u32 << 24) as f32;
            self.attack_pitch += r * 20.0 - 20.0;
            if self.attack_pitch < -45.0 {
                self.attack_pitch += 45.0;
            }
        }
        self.attack_target = target;
    }

    /// A form camera is on (or blending out).
    pub fn active(&self) -> bool {
        self.running.is_some() || self.out.is_some()
    }

    /// One step. `shot` = [`FormShot::of`] this step; `under` = the view of the cameras below (the field / lock-on
    /// camera, after its own step); `right_stick` raw; `dt` real seconds. None = nothing of the forms is shown (the host
    /// keeps `under`).
    pub fn step(&mut self, dt: f32, shot: Option<FormShot>, target: &CameraTarget, under: &CameraView, right_stick: Vec2, world: &dyn World) -> Option<FormView> {
        // (AUDIT-B F-NANO) Nano Gear's blueprint cameras (CameraAnims, their own cuts / blends)
        if let Some(FormShot::Nano(n)) = shot {
            return self.nano.step(dt, &n, target, under, CAMERA_PARAMS_FOV, world).map(|v| {
                let forward = v.view.rotation.forward();
                let view = CameraView { position: v.view.location, rotation: v.view.rotation, rotation_target: v.view.rotation, look_point: v.view.location + forward * 100.0, distance: 100.0, stick_active: false, followed: false, ..*under };
                FormView { view, fov: v.view.fov, stick_taken: false, cut: v.cut }
            });
        }
        let old = self.running.take();
        let fresh = match (&old, &shot) {
            (Some(o), Some(s)) => !o.same(s),
            (None, Some(_)) => true,
            _ => false,
        };
        if let (Some(o), true) = (&old, fresh || shot.is_none()) {
            // The camera that was on leaves with its block (under a new one it is hidden by the cut of the new one).
            if let Some((view, fov)) = self.last {
                self.out = Some((view, fov, 0.0, o.blend_out()));
            }
        }
        let old = if fresh { None } else { old };
        let mut cut = false;
        let shown = match shot {
            Some(FormShot::Shooter(view)) => {
                let previous = match old {
                    Some(Running::Shooter(r)) => Some(r),
                    _ => None,
                };
                let v = shooter_view(target, &view, previous, under, Some(world));
                self.running = Some(Running::Shooter(v.rotation));
                Some((v, SHOOTER_FOV, true, BLEND_CUT))
            }
            Some(FormShot::ShieldFix) => {
                // The fixed camera copies the view on show when it starts (the one blending out, else the one below).
                let held = match old {
                    Some(Running::Fix(v)) => v,
                    _ => self.out.map(|(v, _, _, _)| v).unwrap_or(*under),
                };
                self.running = Some(Running::Fix(held));
                Some((held, SHIELD_FIX_FOV, false, BLEND_CUT))
            }
            Some(FormShot::ShieldRide) => {
                let mut camera = match old {
                    Some(Running::Ride(c)) => c,
                    _ => Box::new(FieldCamera::new(ride_params(), target)),
                };
                let v = camera.step(dt, &CameraInput { right_stick, ..CameraInput::default() }, target, world);
                self.running = Some(Running::Ride(camera));
                Some((v, NATIVE_FOV, false, BLEND_CUT))
            }
            Some(FormShot::MultiLock(centre)) => {
                let mut camera = match old {
                    Some(Running::MultiLock(c)) => c,
                    _ => Box::new(FieldCamera::new(multilock_params(), target)),
                };
                // Slot 299 0x1405EBF60: the rest yaw toward the targets' centre (else the actor's yaw).
                let toward = centre.map(|c| c - target.position).filter(|d| d.x.abs() + d.y.abs() > 1.0e-3);
                camera.params_mut().rotation_base_yaw_offset = toward.map_or(0.0, |d| normalize_axis(d.y.atan2(d.x).to_degrees() - target.yaw));
                let v = camera.step(dt, &CameraInput { right_stick, ..CameraInput::default() }, target, world);
                self.running = Some(Running::MultiLock(camera));
                Some((v, MULTILOCK_FOV, false, MULTILOCK_BLEND))
            }
            Some(FormShot::Attack(a)) if a.mode == 8 => {
                let (look_at, previous) = match old {
                    Some(Running::Over { look_at, rotation }) => (look_at, Some(rotation)),
                    // vt+0x710: the actor location (the capsule centre) + his forward x 240.
                    _ => (target.position + Vec3::Z * target.collision_half_height + Rot::new(0.0, target.yaw, 0.0).forward() * ATTACK_OVER_AHEAD, None),
                };
                let rotation = rotation_through(target, look_at, previous.unwrap_or(Rot::new(0.0, target.yaw, 0.0)), ATTACK_OVER_OFFSET);
                let v = place(look_point_with(target, rotation, ATTACK_OVER_OFFSET), rotation, SHOOTER_DISTANCE, under, Some(world));
                self.running = Some(Running::Over { look_at, rotation });
                Some((v, SHOOTER_FOV, false, ATTACK_BLEND_IN[2]))
            }
            Some(FormShot::Attack(a)) => {
                let i = usize::from(a.mode == 2);
                let (mut rotation, mut point) = match old {
                    Some(Running::Orbit { rotation, point, .. }) => (rotation, point),
                    _ => {
                        let pitch = if a.mode == 2 {
                            self.draw_pitch(a.target.0);
                            self.attack_pitch
                        } else {
                            ATTACK_ORBIT_PITCH_1
                        };
                        (Rot::new(pitch, under.rotation.yaw, 0.0), a.point.unwrap_or(under.look_point))
                    }
                };
                if let Some(p) = a.point {
                    point = p;
                }
                rotation.yaw = normalize_axis(rotation.yaw + ATTACK_ORBIT_YAW_RATE * dt);
                let v = place(point, rotation, ATTACK_ORBIT_DISTANCE[i], under, Some(world));
                self.running = Some(Running::Orbit { mode: a.mode, target: a.target, rotation, point });
                Some((v, NATIVE_FOV, false, ATTACK_BLEND_IN[i]))
            }
            // (AUDIT-B F-NANO) handled at the top
            Some(FormShot::Nano(_)) | None => None,
        };
        if let Some((view, fov, stick_taken, blend_in)) = shown {
            if fresh {
                self.inn = (blend_in.time > 0.0).then_some((0.0, blend_in));
                cut = blend_in.time <= 0.0;
                if cut {
                    self.out = None;
                }
            }
            self.last = Some((view, fov));
            // Under a cut nothing blends; a blend in mixes from what was shown (the outgoing form camera or `under`).
            let from = self.out.map_or((*under, CAMERA_PARAMS_FOV), |(v, f, _, _)| (v, f));
            if let Some((t, b)) = self.inn {
                let t = t + dt;
                if t >= b.time {
                    self.inn = None;
                    self.out = None;
                } else {
                    self.inn = Some((t, b));
                    let w = ease_out(t / b.time, b.exponent);
                    return Some(FormView { view: blend_views(&from.0, &view, w), fov: from.1 + (fov - from.1) * w, stick_taken, cut: false });
                }
            }
            self.out = None;
            return Some(FormView { view, fov, stick_taken, cut });
        }
        self.inn = None;
        self.last = None;
        let (from, fov, t, blend) = self.out?;
        let t = t + dt;
        if blend.time <= 0.0 || t >= blend.time {
            self.out = None;
            return None;
        }
        self.out = Some((from, fov, t, blend));
        let w = ease_out(t / blend.time, blend.exponent);
        Some(FormView { view: blend_views(&from, under, w), fov: fov + (CAMERA_PARAMS_FOV - fov) * w, stick_taken: false, cut: false })
    }
}

/// The field camera's FOV (the blueprint's 100, `projection`).
const CAMERA_PARAMS_FOV: f32 = 100.0;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TargetState;
    use kh_core::world::Hit;

    struct Open;
    impl World for Open {
        fn sweep_capsule(&self, _: Vec3, _: Vec3, _: f32, _: f32) -> Option<Hit> {
            None
        }
        fn line_trace(&self, _: Vec3, _: Vec3) -> Option<Hit> {
            None
        }
    }

    fn target() -> CameraTarget {
        CameraTarget { position: Vec3::ZERO, yaw: 0.0, velocity: Vec3::ZERO, state: TargetState::Idle, collision_half_height: 89.0 }
    }

    fn field() -> CameraView {
        FieldCamera::new(CAMERA_PARAMS, &target()).view()
    }

    fn shooter(yaw: f32, pitch: f32) -> ShooterView {
        let eye = Vec3::new(0.0, 0.0, 250.0);
        let dir = Rot::new(pitch, yaw, 0.0).forward();
        ShooterView { camera: true, eye, look_at: eye + dir * 5000.0, yaw, pitch, distance: 5000.0, ..ShooterView::default() }
    }

    #[test]
    fn the_shooter_camera_cuts_in_and_out_and_looks_through_the_aim() {
        let mut c = FormCameras::default();
        let under = field();
        let v = c.step(1.0 / 60.0, Some(FormShot::Shooter(shooter(30.0, 10.0))), &target(), &under, Vec2::ZERO, &Open).expect("on");
        assert!(v.cut && v.stick_taken && (v.fov - 60.0).abs() < 1e-6);
        // The screen centre's ray passes through the state's look-at point.
        let s = shooter(30.0, 10.0);
        let to = s.look_at - v.view.look_point;
        assert!(to.normalize().dot(v.view.rotation.forward()) > 0.9999, "{:?}", v.view.rotation);
        assert!((v.view.distance - SHOOTER_DISTANCE).abs() < 1e-3);
        // The offset puts the look point 65 cm to the camera's left and 45 up of Sora's camera point.
        let (_, right, _) = axes(v.view.rotation);
        let local = v.view.look_point - (Vec3::Z * CAMERA_PARAMS.target_socket_height);
        assert!((local.dot(right) + 65.0).abs() < 1e-2);
        let v2 = c.step(1.0 / 60.0, Some(FormShot::Shooter(shooter(31.0, 10.0))), &target(), &under, Vec2::ZERO, &Open).expect("on");
        assert!(!v2.cut);
        // Off: the block is a cut (BlendTime 0.0): the field camera at once.
        assert_eq!(c.step(1.0 / 60.0, None, &target(), &under, Vec2::ZERO, &Open), None);
        assert!(!c.active());
    }

    #[test]
    fn the_shooter_pitch_stays_within_its_limits() {
        let r = shooter_rotation(&target(), Vec3::new(100.0, 0.0, 9000.0), Rot::default());
        assert!((r.pitch - SHOOTER_PITCH_LIMIT).abs() < 1e-3);
    }

    #[test]
    fn counter_shield_s_finish_holds_then_rides_then_blends_back_in_half_a_second() {
        let mut c = FormCameras::default();
        let under = field();
        let dt = 1.0 / 60.0;
        let fix = c.step(dt, Some(FormShot::ShieldFix), &target(), &under, Vec2::ZERO, &Open).expect("fix");
        assert!(fix.cut && (fix.fov - 100.0).abs() < 1e-6 && fix.view.position == under.position);
        // Sora moves away: the fixed camera stays.
        let far = CameraTarget { position: Vec3::new(500.0, 0.0, 0.0), ..target() };
        let fix2 = c.step(dt, Some(FormShot::ShieldFix), &far, &under, Vec2::ZERO, &Open).expect("fix");
        assert_eq!(fix2.view.position, fix.view.position);
        let ride = c.step(dt, Some(FormShot::ShieldRide), &far, &under, Vec2::ZERO, &Open).expect("ride");
        assert!(ride.cut && (ride.fov - NATIVE_FOV).abs() < 1e-6);
        // Behind and above the ride target, about 700 away, looking down 25.
        assert!((ride.view.rotation.pitch - SHIELD_RIDE_PITCH).abs() < 1.0, "{:?}", ride.view.rotation);
        assert!((ride.view.distance - SHIELD_RIDE_DISTANCE).abs() < 5.0, "{}", ride.view.distance);
        assert!((ride.view.look_point.z - SHIELD_RIDE_TARGET_OFFSET_Z).abs() < 1.0);
        // The end: 0.5 s ease-out back to the camera below.
        let mut steps = 0;
        while c.step(dt, None, &far, &under, Vec2::ZERO, &Open).is_some() {
            steps += 1;
            assert!(steps < 40);
        }
        assert_eq!(steps, 29, "0.5 s at 60 steps per second (the last step lands on the camera below)");
    }

    #[test]
    fn the_multilock_camera_blends_in_turns_toward_the_targets_and_blends_out() {
        let mut c = FormCameras::default();
        let under = field();
        let dt = 1.0 / 60.0;
        // Two targets to Sora's left (+Y): their centre at yaw 90.
        let list = [kh_core::host::TargetId(1), kh_core::host::TargetId(2)];
        let point = |t: kh_core::host::TargetId| Some(Vec3::new(if t.0 == 1 { -100.0 } else { 100.0 }, 800.0, 100.0));
        let centre = multilock_centre(&list, &point);
        assert_eq!(centre, Some(Vec3::new(0.0, 800.0, 100.0)));
        let first = c.step(dt, Some(FormShot::MultiLock(centre)), &target(), &under, Vec2::ZERO, &Open).expect("on");
        assert!(!first.cut, "a 0.5 s blend in, not a cut");
        // Sora runs (the auto turn needs speed): the camera turns toward the centre.
        let mut t = target();
        for i in 0..240 {
            t.position.x = i as f32 * 5.0;
            t.velocity = Vec3::new(300.0, 0.0, 0.0);
            t.state = TargetState::Run;
            c.step(dt, Some(FormShot::MultiLock(centre.map(|p| p + Vec3::X * t.position.x))), &t, &under, Vec2::ZERO, &Open);
        }
        let v = c.step(dt, Some(FormShot::MultiLock(centre.map(|p| p + Vec3::X * t.position.x))), &t, &under, Vec2::ZERO, &Open).expect("on");
        assert!(v.view.rotation.yaw > 20.0, "turned toward +Y: {:?}", v.view.rotation);
        assert!((v.fov - MULTILOCK_FOV).abs() < 1e-6);
        let mut steps = 0;
        while c.step(dt, None, &t, &under, Vec2::ZERO, &Open).is_some() {
            steps += 1;
            assert!(steps < 40);
        }
        assert_eq!(steps, 29);
    }

    fn attack(mode: u8, id: u32, point: Vec3) -> FormShot {
        let mut cam = kh_core::combat::camera::AttackCam { mode, target: kh_core::host::TargetId(id), ..Default::default() };
        cam.anchor = Some(Vec3::ZERO);
        FormShot::attack(&cam, &|_| Some(point)).expect("modes 1 / 2 / 8")
    }

    #[test]
    fn attack_camera_mode_2_orbits_each_target_and_blends_like_kh3() {
        let mut c = FormCameras::default();
        let under = field();
        let dt = 1.0 / 60.0;
        let p1 = Vec3::new(800.0, 0.0, 100.0);
        assert!(FormShot::attack(&kh_core::combat::camera::AttackCam::default(), &|_| None).is_none(), "mode 0: none");
        assert!(FormShot::attack(&kh_core::combat::camera::AttackCam { mode: 5, ..Default::default() }, &|_| None).is_none(), "mode 5 is the rig's");
        // In over 0.1 s (6 steps), then the orbit: 600 cm from the target's point, the pitch drawn in -20 .. 0.
        let first = c.step(dt, Some(attack(2, 1, p1)), &target(), &under, Vec2::ZERO, &Open).expect("on");
        assert!(!first.cut);
        let mut v = first;
        for _ in 0..6 {
            v = c.step(dt, Some(attack(2, 1, p1)), &target(), &under, Vec2::ZERO, &Open).expect("on");
        }
        assert!((v.view.distance - 600.0).abs() < 1e-2 && (v.view.look_point - p1).length() < 1e-3);
        let pitch = v.view.rotation.pitch;
        assert!((-20.0..=0.0).contains(&pitch), "{pitch}");
        assert!((v.fov - NATIVE_FOV).abs() < 1e-6);
        // The auto rotation: -30 degrees a second.
        let yaw = v.view.rotation.yaw;
        for _ in 0..60 {
            v = c.step(dt, Some(attack(2, 1, p1)), &target(), &under, Vec2::ZERO, &Open).expect("on");
        }
        assert!((normalize_axis(v.view.rotation.yaw - yaw) + 30.0).abs() < 0.01, "{}", v.view.rotation.yaw - yaw);
        // The next avatar's target: a new camera (the old one leaves), the pitch moved again (wrapped above -45).
        let p2 = Vec3::new(0.0, 900.0, 100.0);
        for _ in 0..10 {
            v = c.step(dt, Some(attack(2, 2, p2)), &target(), &under, Vec2::ZERO, &Open).expect("on");
        }
        assert!((v.view.look_point - p2).length() < 1e-3);
        assert!(v.view.rotation.pitch < pitch + 1e-3 || v.view.rotation.pitch > -45.0 + 20.0, "drawn down, or wrapped");
        assert!(v.view.rotation.pitch >= -45.0 && v.view.rotation.pitch <= 0.0);
        // Off: 0.5 s back.
        let mut steps = 0;
        while c.step(dt, None, &target(), &under, Vec2::ZERO, &Open).is_some() {
            steps += 1;
            assert!(steps < 40);
        }
        assert_eq!(steps, 29);
    }

    #[test]
    fn attack_camera_modes_1_and_8() {
        let under = field();
        let dt = 1.0 / 60.0;
        let mut c = FormCameras::default();
        let p = Vec3::new(500.0, 0.0, 100.0);
        let mut v = c.step(dt, Some(attack(1, 3, p)), &target(), &under, Vec2::ZERO, &Open).expect("on");
        for _ in 0..60 {
            v = c.step(dt, Some(attack(1, 3, p)), &target(), &under, Vec2::ZERO, &Open).expect("on");
        }
        assert!((v.view.distance - 750.0).abs() < 1e-2 && (v.view.rotation.pitch + 20.0).abs() < 1e-4, "{:?}", v.view.rotation);
        // Mode 8: over Sora's shoulder at FOV 60, looking at the point 240 cm ahead of him; in over 1 s.
        let mut c = FormCameras::default();
        let mut v = c.step(dt, Some(attack(8, 0, p)), &target(), &under, Vec2::ZERO, &Open).expect("on");
        for _ in 0..60 {
            v = c.step(dt, Some(attack(8, 0, p)), &target(), &under, Vec2::ZERO, &Open).expect("on");
        }
        assert!((v.fov - SHOOTER_FOV).abs() < 1e-6);
        let ahead = Vec3::new(ATTACK_OVER_AHEAD, 0.0, target().collision_half_height);
        assert!((ahead - v.view.look_point).normalize().dot(v.view.rotation.forward()) > 0.999, "{:?} {:?} {}", v.view.rotation, v.view.look_point, (ahead - v.view.look_point).normalize().dot(v.view.rotation.forward()));
    }

    #[test]
    fn the_staff_scale_and_the_wide_attack_camera() {
        assert_eq!(field_distance_scale(0, STAFF_DISTANCE_SCALE), 1.35);
        assert_eq!(field_distance_scale(5, STAFF_DISTANCE_SCALE), 1.6);
        assert_eq!(field_distance_scale(4, STAFF_DISTANCE_SCALE), 1.0);
    }
}
