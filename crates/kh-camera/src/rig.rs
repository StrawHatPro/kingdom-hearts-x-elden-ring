//! The camera a host steps: the field camera with the lock-on camera on top and the 0.5 s blends
//! between them (docs/sora-combat-spec.md 7.12 "Entry": `Camera.SetLockon(comp)` 0x1405F0140 ->
//! `TresCameraNormal::SetLockonTarget` 0x14060AB70: an existing lock-on camera is ended and a new
//! one started with the transition {0.5 s, function 3 (ease-out), exponent 2.0}; a switch of
//! target spawns a fresh lock-on camera and blends from the old one).
//!
//! The field camera keeps existing underneath (it is stepped every frame without stick input
//! while the lock-on camera has the view, so that it is where the game's would be when the lock
//! ends). A mode a later implementer adds (the shotlock aim camera of the magic spec) is one
//! more `Option` here with the same blend.

use glam::Vec3;
use kh_core::combat::tables::P as C;
use kh_core::World;

use crate::lockon::{LockTarget, LockonCamera};
use crate::{normalize_axis, CameraInput, CameraParams, CameraTarget, CameraView, FieldCamera, Rot};

/// The view blend of `SetViewTargetWithBlend`: position, rotation (the short way round) and the
/// rest taken from the incoming view.
pub fn blend_views(from: &CameraView, to: &CameraView, alpha: f32) -> CameraView {
    let mix = |a: f32, b: f32| normalize_axis(a + normalize_axis(b - a) * alpha);
    CameraView {
        position: from.position.lerp(to.position, alpha),
        rotation: Rot { pitch: mix(from.rotation.pitch, to.rotation.pitch), yaw: mix(from.rotation.yaw, to.rotation.yaw), roll: mix(from.rotation.roll, to.rotation.roll) },
        look_point: from.look_point.lerp(to.look_point, alpha),
        distance: from.distance + (to.distance - from.distance) * alpha,
        ..*to
    }
}

/// Function 3 (`VTBlend_EaseOut`) with the exponent: 1 - (1 - t)^exp.
pub fn ease_out(t: f32, exponent: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powf(exponent)
}

#[derive(Clone, Debug, PartialEq)]
pub struct CameraRig {
    pub field: FieldCamera,
    /// The lock-on camera and the host's id of its target.
    pub lock: Option<(u32, LockonCamera)>,
    /// A blend in progress: the view it started from (frozen: the outgoing camera's last view)
    /// and the seconds elapsed.
    blend: Option<(CameraView, f32)>,
    view: CameraView,
    /// The attack camera (docs/sora-combat-spec.md 7.12.1; `set_attack_mode`).
    attack: AttackCamera,
}

/// The attack camera's modes that act on the normal camera (controller function 0x1414122A0,
/// thunk 0x14140DF20; the mode is controller+0xAB0; decompiled C, the constants are immediates):
///
/// | mode | on | off |
/// |---|---|---|
/// | 3 | `Camera.SetLockon(target)`: the lock-on camera on the attack's target | re-lock on the manual lock, else unlock |
/// | 4 | look-point rate (+0x450) 2.0, lag limit (+0x44C) 2000 (0x1405C63A0, also on the lock-on camera); lock yaw limit 0 (0x14060A7D0); distance scale (+0x804) 1.0 | rate / limit back to the class defaults at once (0x1405D4730(-1)); yaw limit back (0x140600BD0); scale 1.0 |
/// | 5 | distance scale 1.6 | scale 1.0 |
/// | 6 | 3 and 4 | as 4, the lag back over 1.0 s (interpolator type 5); as 3 |
/// | 7 | rate 0.2, lag limit 3000; yaw limit 0; scale 1.0 | the lag back over 0.1 s |
///
/// Modes 1, 2 and 8 spawn cameras of their own (shotlock / other keyblades' forms): not here.
/// The lock half of modes 3 / 6 reaches the rig as `set_lockon` (the core sends
/// `EventKind::CameraLock`).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct AttackCamera {
    pub mode: u8,
    /// The class defaults of the normal camera: (look-point rate, lag limit).
    defaults: (f32, f32),
    /// The lag returning to the defaults: (rate from, limit from, elapsed, duration).
    tween: Option<(f32, f32, f32, f32)>,
    /// The lock-on yaw limit in force (`None` = the class default, 120).
    lock_yaw_limit: Option<f32>,
}

pub const ATTACK_CAM_BURN_LAG: (f32, f32) = (2.0, 2000.0);
pub const ATTACK_CAM_MODE7_LAG: (f32, f32) = (0.2, 3000.0);
pub const ATTACK_CAM_WIDE_SCALE: f32 = 1.6;

impl CameraRig {
    pub fn new(p: CameraParams, target: &CameraTarget) -> CameraRig {
        let field = FieldCamera::new(p, target);
        let view = field.view();
        let attack = AttackCamera { defaults: (p.look_lerp_rate, p.look_lag_limit), ..AttackCamera::default() };
        CameraRig { field, lock: None, blend: None, view, attack }
    }

    pub fn view(&self) -> CameraView {
        self.view
    }

    /// The yaw the controller steers by: the rotation shown at the end of the last update.
    pub fn input_yaw(&self) -> f32 {
        self.view.rotation.yaw
    }

    pub fn locked(&self) -> Option<u32> {
        self.lock.as_ref().map(|l| l.0)
    }

    /// `Camera.SetLockon(target)`: `Some((id, look-at position))` locks (or switches), `None`
    /// unlocks. What `EventKind::CameraLock` asks for.
    pub fn set_lockon(&mut self, target: &CameraTarget, lock: Option<(u32, Vec3)>) {
        match lock {
            Some((id, look_at)) => {
                if self.locked() == Some(id) {
                    return;
                }
                self.blend = Some((self.view, 0.0));
                let mut camera = LockonCamera::start(*self.field.params(), target, &LockTarget { look_at });
                // (The copy of the parameters 0x14060A930 takes the parent's lag and yaw limit as they stand.)
                let p = *self.field.params();
                camera.set_attack_params(p.look_lerp_rate, p.look_lag_limit, self.attack.lock_yaw_limit);
                self.lock = Some((id, camera));
            }
            None => {
                if self.lock.take().is_some() {
                    self.blend = Some((self.view, 0.0));
                }
            }
        }
    }

    pub fn attack_camera(&self) -> &AttackCamera {
        &self.attack
    }

    /// What `EventKind::AttackCamera { mode, restore }` asks for: a mode of the table above, or 0
    /// = off with `restore` = the seconds the lag takes back to its defaults (negative: at once).
    pub fn set_attack_mode(&mut self, mode: u8, restore: f32) {
        let defaults = self.attack.defaults;
        let lag = |rig: &mut CameraRig, rate: f32, limit: f32, yaw: Option<f32>| {
            let p = rig.field.params_mut();
            (p.look_lerp_rate, p.look_lag_limit) = (rate, limit);
            if let Some((_, camera)) = &mut rig.lock {
                camera.set_attack_params(rate, limit, yaw);
            }
            (rig.attack.lock_yaw_limit, rig.attack.tween) = (yaw, None);
        };
        match mode {
            4 | 6 => {
                lag(self, ATTACK_CAM_BURN_LAG.0, ATTACK_CAM_BURN_LAG.1, Some(0.0));
                self.field.params_mut().distance_scale = 1.0;
            }
            7 => {
                lag(self, ATTACK_CAM_MODE7_LAG.0, ATTACK_CAM_MODE7_LAG.1, Some(0.0));
                self.field.params_mut().distance_scale = 1.0;
            }
            5 => self.field.params_mut().distance_scale = ATTACK_CAM_WIDE_SCALE,
            0 => {
                let now = *self.field.params();
                if restore >= 0.0 && (now.look_lerp_rate, now.look_lag_limit) != defaults {
                    // 0x1405D4730(cam, t >= 0): two interpolators of type 5 from the current values.
                    lag(self, now.look_lerp_rate, now.look_lag_limit, None);
                    self.attack.tween = Some((now.look_lerp_rate, now.look_lag_limit, 0.0, restore));
                } else {
                    lag(self, defaults.0, defaults.1, None);
                }
                self.field.params_mut().distance_scale = 1.0;
            }
            // 3: only the lock (set_lockon); 1, 2, 8: cameras of their own, not ported.
            _ => {}
        }
        self.attack.mode = mode;
    }

    /// One update after the pawn's. `lock_look_at` = the locked target's look-at position now
    /// (`None` when it no longer exists: the camera holds its last position until the unlock).
    pub fn step(&mut self, dt: f32, input: &CameraInput, target: &CameraTarget, lock_look_at: Option<Vec3>, world: &dyn World) -> CameraView {
        // The attack camera's lag on its way back to the defaults (interpolator type 5).
        if let Some((rate, limit, elapsed, duration)) = self.attack.tween {
            let elapsed = elapsed + dt;
            let a = if duration > 0.0 { crate::lockon::interp_curve(5, elapsed / duration) } else { 1.0 };
            let (to_rate, to_limit) = self.attack.defaults;
            let (r, l) = (rate + (to_rate - rate) * a, limit + (to_limit - limit) * a);
            let p = self.field.params_mut();
            (p.look_lerp_rate, p.look_lag_limit) = (r, l);
            let yaw = self.attack.lock_yaw_limit;
            if let Some((_, camera)) = &mut self.lock {
                camera.set_attack_params(r, l, yaw);
            }
            self.attack.tween = if elapsed >= duration { None } else { Some((rate, limit, elapsed, duration)) };
        }
        let active = match &mut self.lock {
            Some((_, camera)) => {
                // The field camera underneath: no stick, no reset.
                self.field.step(dt, &CameraInput::default(), target, world);
                let lock = lock_look_at.map(|look_at| LockTarget { look_at });
                camera.step(dt, input, &self.field, target, lock.as_ref(), world)
            }
            None => self.field.step(dt, input, target, world),
        };
        self.view = match &mut self.blend {
            Some((from, elapsed)) => {
                *elapsed += dt;
                let t = *elapsed / C.lockon_cam_blend_time;
                let view = blend_views(from, &active, ease_out(t, C.lockon_cam_blend_exponent));
                if t >= 1.0 {
                    self.blend = None;
                    active
                } else {
                    view
                }
            }
            None => active,
        };
        self.view
    }
}
