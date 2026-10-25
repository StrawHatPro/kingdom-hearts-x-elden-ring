//! THE BOSS-FINISH CAMERA (docs/sora-bossfinish-spec.md 3.3): the three hard cuts of KH3's boss
//! finish. In the game each cut is an `ATresCameraRotTarget` named "Battle_End" (priority 210,
//! started with blend time 0) whose target is a note actor the level blueprint puts at a bone
//! (`t_camera`) of the defeated enemy or of Sora, with that pawn's rotation:
//!
//! ```text
//! look point = note position + TargetLocalOffset rotated by the note's rotation
//! rotation   = m_Rotation + (0, note yaw, 0)          (camera spec 2.6: the base rotation)
//! position   = look point - forward(rotation) x distance
//! SetBaseRotation(end, 0.5) / SetDistance(end, 0.5): interpolator type 5 on the camera's
//! (world) time: the script (`kh_core::combat::finish`) evaluates it, this file places the view.
//! ```
//!
//! The note actor is PUT there, not attached: the look point of a cut is where the bone was when
//! the cut began ([`FinishCamera`] keeps it). The stick does not reach these cameras
//! (`bDisableCameraMove`). KH3 switches the first cut's collision off; the port sweeps every cut
//! from its look point when the host asks for it (sheet row `port.camera_collision`).
//!
//! The shake of the script (`no_speed_kokizami_1s`: rotation only, real time) is [`shake_def`],
//! played into a [`crate::shake::ShakeSet`] the host steps with the REAL frame time.

use glam::Vec3;
use kh_core::combat::finish::{FinishShot, Subject, P};
use kh_core::World;

use crate::shake::{Oscillator, ShakeDef};
use crate::{CameraView, Rot};

/// Where a subject's camera bone is and which way the pawn faces (core axes, cm, degrees).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FinishSubject {
    pub point: Vec3,
    pub yaw: f32,
}

/// `/Game/Camera/shake/no_speed_kokizami_1s` (class defaults read from the asset): 1 s, no blend
/// in, blend out 3 s (longer than the shake: it fades from its first frame), pitch 3 and yaw 1
/// at frequency 50. (`m_AmplitudeRandScaleMin` 0.5 is not evaluated: shake.rs.)
pub fn shake_def() -> ShakeDef {
    let osc = |v: [f32; 2]| Oscillator { amplitude: v[0], frequency: v[1], start_at_zero: false };
    ShakeDef { name: "no_speed_kokizami_1s".to_string(), duration: P.shake_duration, blend_in: P.shake_blend_in, blend_out: P.shake_blend_out, rot: [osc(P.shake_pitch), osc(P.shake_yaw), Oscillator::default()], loc: [Oscillator::default(); 3], fov: Oscillator::default(), single_instance: false }
}

/// The view of one cut for a subject frozen at `at`.
pub fn shot_view(shot: &FinishShot, at: &FinishSubject, world: Option<&dyn World>, like: &CameraView) -> CameraView {
    let yaw = at.yaw.to_radians();
    let [x, y, z] = shot.local_offset;
    // (X forward, Y right, Z up; yaw turns +X toward +Y.)
    let look = at.point + Vec3::new(x * yaw.cos() - y * yaw.sin(), x * yaw.sin() + y * yaw.cos(), z);
    let rotation = Rot::new(shot.rotation[0], shot.rotation[1] + at.yaw, shot.rotation[2]).normalized();
    let wanted = look - rotation.forward() * shot.distance;
    let radius = P.port_probe_radius;
    let (position, collided) = match world.and_then(|w| w.sweep_capsule(look, wanted, radius, radius)) {
        Some(hit) => (look + (wanted - look) * hit.time, true),
        None => (wanted, false),
    };
    CameraView { position, rotation, rotation_target: rotation, look_point: look, distance: (look - position).length(), stick_active: false, followed: false, collided, ..*like }
}

/// The camera of a running boss finish: a host keeps one beside its camera stack and calls
/// [`FinishCamera::step`] every step with the script's shot; `None` back = the host's own camera
/// shows.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct FinishCamera {
    /// The cut on screen and where its note actor was put.
    anchor: Option<(u8, FinishSubject)>,
    /// Cuts shown so far.
    pub cuts: u32,
}

impl FinishCamera {
    /// It has the view.
    pub fn active(&self) -> bool {
        self.anchor.is_some()
    }

    pub fn clear(&mut self) {
        self.anchor = None;
    }

    /// One step. `enemy` / `player`: where their camera bones are NOW (read when a cut begins);
    /// `world`: `Some` = the cut is kept out of the level. Returns (the view, this step is a hard
    /// cut: the host must not interpolate from the view before).
    pub fn step(&mut self, shot: Option<&FinishShot>, enemy: &FinishSubject, player: &FinishSubject, world: Option<&dyn World>, like: &CameraView) -> Option<(CameraView, bool)> {
        let Some(shot) = shot else {
            self.anchor = None;
            return None;
        };
        let fresh = shot.cut || self.anchor.is_none_or(|(index, _)| index != shot.index);
        if fresh {
            let at = match shot.subject {
                Subject::Enemy => *enemy,
                Subject::Player => *player,
            };
            self.anchor = Some((shot.index, at));
            self.cuts += 1;
        }
        let (_, at) = self.anchor?;
        Some((shot_view(shot, &at, world, like), fresh))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_core::combat::finish::{FinishSequence, FinishTuning};
    use kh_core::host::FinishKind;

    fn like() -> CameraView {
        let r = Rot::new(0.0, 0.0, 0.0);
        CameraView { position: Vec3::ZERO, rotation: r, rotation_target: r, look_point: Vec3::ZERO, distance: 0.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false }
    }

    #[test]
    fn the_first_cut_frames_the_enemy_from_its_side() {
        let shot = FinishShot { index: 0, subject: Subject::Enemy, distance: 200.0, rotation: [5.0, -30.0, 5.0], local_offset: [0.0, 0.0, 50.0], collision: false, cut: true };
        // The enemy stands at (1000, 0, 100) facing +Y (yaw 90).
        let at = FinishSubject { point: Vec3::new(1000.0, 0.0, 100.0), yaw: 90.0 };
        let v = shot_view(&shot, &at, None, &like());
        assert_eq!(v.look_point, Vec3::new(1000.0, 0.0, 150.0));
        assert!((v.rotation.yaw - 60.0).abs() < 1.0e-4 && v.rotation.pitch == 5.0 && v.rotation.roll == 5.0);
        // 200 cm back along the view direction: the camera looks at the look point.
        assert!(((v.look_point - v.position).length() - 200.0).abs() < 1.0e-2);
        assert!((v.look_point - v.position).normalize().dot(v.rotation.forward()) > 0.9999);
        // A local offset to the pawn's left (-Y) with yaw 90 is world +X.
        let left = FinishShot { local_offset: [0.0, -50.0, 0.0], ..shot };
        let v = shot_view(&left, &at, None, &like());
        assert!((v.look_point - Vec3::new(1050.0, 0.0, 100.0)).length() < 1.0e-3, "{:?}", v.look_point);
    }

    #[test]
    fn a_cut_keeps_its_look_point_and_the_cuts_follow_the_script() {
        let mut seq = FinishSequence::default();
        seq.start(FinishKind::Boss, FinishTuning::default());
        let mut camera = FinishCamera::default();
        let (mut enemy, player) = (FinishSubject { point: Vec3::new(500.0, 0.0, 120.0), yaw: 180.0 }, FinishSubject { point: Vec3::new(0.0, 0.0, 130.0), yaw: 0.0 });
        let (mut cuts, mut looks, mut time) = (Vec::new(), Vec::new(), 0.0f32);
        while seq.active() && time < 10.0 {
            let frame = seq.step(1.0 / 60.0);
            time += 1.0 / 60.0;
            // the enemy drifts away (blown off by the last hit): the cut does not follow it
            enemy.point.x += 0.5;
            if let Some((view, cut)) = camera.step(frame.shot.as_ref(), &enemy, &player, None, &like()) {
                if cut {
                    cuts.push((time, view.look_point));
                }
                looks.push((frame.shot.unwrap().index, view.look_point, view.distance));
            }
        }
        assert_eq!((cuts.len(), camera.cuts, camera.active()), (3, 3, false));
        // Each cut's look point never moves; cut 2 looks at Sora (50 cm to his left), cut 3 at the enemy again.
        for index in 0..3u8 {
            let of: Vec<_> = looks.iter().filter(|(i, _, _)| *i == index).collect();
            assert!(of.len() > 20 && of.iter().all(|(_, p, _)| *p == of[0].1), "cut {index}");
        }
        assert!((cuts[1].1 - Vec3::new(0.0, -50.0, 130.0)).length() < 1.0e-3);
        assert!(cuts[2].1.x > cuts[0].1.x + 20.0 && (cuts[2].1.z - 170.0).abs() < 1.0e-3);
        // The first cut pulls back (200 -> 219), the third moves in (600 -> 532).
        let dist = |index: u8| { let of: Vec<f32> = looks.iter().filter(|(i, _, _)| *i == index).map(|(_, _, d)| *d).collect(); (of[0], *of.last().unwrap()) };
        let (a, b) = dist(0);
        assert!((a - 200.0).abs() < 1.0 && (b - 219.0).abs() < 3.0, "{a} {b}");
        let (a, b) = dist(2);
        assert!((a - 600.0).abs() < 1.5 && (b - 532.0).abs() < 5.0, "{a} {b}");
    }

    #[test]
    fn the_shake_is_the_assets() {
        let def = shake_def();
        assert_eq!((def.duration, def.blend_in, def.blend_out), (1.0, 0.0, 3.0));
        assert_eq!((def.rot[0].amplitude, def.rot[0].frequency, def.rot[1].amplitude, def.rot[2].amplitude), (3.0, 50.0, 1.0, 0.0));
        let mut set = crate::shake::ShakeSet::new();
        set.play(&def, 1.0, None);
        let mut peak = 0.0f32;
        for _ in 0..59 {
            set.step(1.0 / 60.0);
            peak = peak.max(set.offset().rotation.x.abs());
        }
        assert!(peak > 0.3 && peak <= 3.0 && set.offset().location == Vec3::ZERO, "{peak}");
        for _ in 0..5 {
            set.step(1.0 / 60.0);
        }
        assert!(set.is_empty());
    }
}
