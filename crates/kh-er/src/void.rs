//! (IMPL_ER15) THE VOID OF KH3'S GAME-OVER SCREEN IN ELDEN RING: Sora floating in the dark under the game-over menu
//! (docs/sora-gameover-spec.md; the sequence and its numbers are `kh_host::gameover`; the test app's version is
//! kh-app/src/gameover_scene.rs).
//!
//! A SECOND, separately posed Sora with its own `kh_anim::Animator` (the simulation's Sora lies where he died, held by
//! the pause): the sequence `0270` looped in place with its own notifies (the face row: eyes closed), the clip's Bonamik
//! states applied at the scene's start (speed rate 0.3, the upward character wind, mass / LFCoeff of the hair and cloth
//! groups). The pawn stands at the origin of a space of its own (his FEET; rotation zero = facing +X), the camera is the
//! camera anim's key in the pawn's space, blended in from the camera of the death, the lights are the effect's three
//! particle lights. What comes out is a `kh_er_render::VoidFrame`: the renderer clears the whole back buffer to black
//! at Present and draws only him (its own depth buffer), under the 2D overlay's plate and menu.
//!
//! The pure part (the camera of a view in host space) is unit-tested below.

use glam::{Mat3, Quat, Vec3};
use kh_host::gameover::{GameOverCamera, GameOverScene, MESH_YAW_DEG, ROOT_ABOVE_FEET};

/// LOCAL: the factor in front of the tone curve (the test app's value, matched by eye to the reference capture's skin;
/// the game's eye adaptation in the void was not read). `gameover.exposure` overrides.
pub const EXPOSURE: f32 = 1.6;

/// A camera in the renderer's host space (metres, X right, Y up, Z forward, left-handed): rows right / up / forward.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HostCamera {
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
    pub position: Vec3,
    /// Vertical field of view over the 16:9 stage, radians.
    pub fov: f32,
}

/// GAME (KH3: cm, X forward, Y right, Z up) -> the renderer's host space with the game's origin at the host's.
pub fn host_dir(v: Vec3) -> Vec3 {
    Vec3::new(v.y, v.z, v.x)
}

pub fn host_pos(p: Vec3) -> Vec3 {
    host_dir(p) * 0.01
}

/// The camera anim's key for a pawn whose FEET are at the game origin (rotation zero).
pub fn camera_of(camera: &GameOverCamera) -> HostCamera {
    let (p, y) = (camera.pitch_deg.to_radians(), camera.yaw_deg.to_radians());
    // A rotator's axes (roll 0): forward, right (the engine's Y axis), up.
    let forward = Vec3::new(p.cos() * y.cos(), p.cos() * y.sin(), p.sin());
    let right = Vec3::new(-y.sin(), y.cos(), 0.0);
    let up = Vec3::new(-p.sin() * y.cos(), -p.sin() * y.sin(), p.cos());
    let eye = Vec3::new(0.0, 0.0, ROOT_ABOVE_FEET) + Vec3::from(camera.position);
    // The asset's field of view spans the stage's width (16:9); the renderer's is the vertical one.
    let fov = 2.0 * ((camera.fov_deg.to_radians() * 0.5).tan() * 9.0 / 16.0).atan();
    HostCamera { right: host_dir(right), up: host_dir(up), forward: host_dir(forward), position: host_pos(eye), fov }
}

/// `from` blended to `to` by `a` (0..1): position and field of view linearly, the axes on the shortest arc.
pub fn blend(from: &HostCamera, to: &HostCamera, a: f32) -> HostCamera {
    let a = a.clamp(0.0, 1.0);
    let q = |c: &HostCamera| Quat::from_mat3(&Mat3::from_cols(c.right, c.up, c.forward)).normalize();
    let m = Mat3::from_quat(q(from).slerp(q(to), a));
    HostCamera { right: m.x_axis, up: m.y_axis, forward: m.z_axis, position: from.position.lerp(to.position, a), fov: from.fov + (to.fov - from.fov) * a }
}

#[cfg(windows)]
pub use game_side::*;

#[cfg(windows)]
mod game_side {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    use kh_anim::bonamik::notify::{CharaWind, GroupParam};
    use kh_anim::{Animator, BodySlot, Frame, NoIkWorld, Settings, Trs};

    use crate::erlog;

    struct Void {
        animator: Animator,
        /// The scene's seconds at the last pose step (`None`: the scene is not up).
        last: Option<f32>,
        frames: u32,
    }

    // SAFETY: the animator holds plain data; it is made on a thread of its own and from then on only used under the mutex.
    unsafe impl Send for Void {}

    static VOID: Mutex<Option<Void>> = Mutex::new(None);
    static LOADING: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);
    /// The camera of the death relative to the dying Sora's feet (host space): what the blend starts from.
    static FROM: Mutex<Option<HostCamera>> = Mutex::new(None);

    /// The copy's own pose evaluator is read (once, on a thread of its own: 0.8 s): call when Sora dies.
    pub fn prepare(pack: &Path) {
        if VOID.lock().is_ok_and(|v| v.is_some()) {
            return;
        }
        let Ok(mut loading) = LOADING.lock() else { return };
        if loading.is_some() {
            return;
        }
        let pack: PathBuf = pack.to_path_buf();
        *loading = std::thread::Builder::new()
            .name("kh-void-load".into())
            .spawn(move || {
                let started = std::time::Instant::now();
                let made = std::panic::catch_unwind(|| kh_anim::Pack::load(&pack).and_then(|read| Animator::new(read, Settings { leg_ik: false, kine_driver: true, face: true, bonamik: true, first_update_holds_position: kh_core::anim::FIRST_UPDATE_HOLDS_POSITION })));
                match made {
                    Ok(Ok(animator)) => {
                        erlog!("game over: the void's own copy of Sora (pose evaluator) is read in {:.2} s", started.elapsed().as_secs_f32());
                        *VOID.lock().unwrap_or_else(|e| e.into_inner()) = Some(Void { animator, last: None, frames: 0 });
                    }
                    Ok(Err(e)) => erlog!("game over: the void's copy of Sora could not be read ({e}): the void stays empty"),
                    Err(_) => erlog!("game over: PANIC while reading the void's copy of Sora: the void stays empty"),
                }
            })
            .ok();
    }

    /// The camera of the death (the game's camera now), relative to where his feet are (both host space).
    pub fn set_death_camera(camera: Option<&crate::game::Camera>, feet_host: Vec3) {
        if let Ok(mut from) = FROM.lock() {
            *from = camera.filter(|c| c.position.is_finite() && c.right.is_finite() && c.fov > 0.01).map(|c| HostCamera { right: c.right, up: c.up, forward: c.forward, position: c.position - feet_host, fov: c.fov });
        }
    }

    /// The clip's Bonamik notify states, as far as the solver takes them (gameover_scene.rs says which are not).
    fn begin_cloth(animator: &mut Animator) {
        let Some(solver) = animator.bonamik_mut() else { return };
        let groups: Vec<String> = ["HAIR", "ACS", "SLEEVE", "HOOD", "JCT", "jip"].iter().map(|g| g.to_string()).collect();
        solver.set_speed_rate(0.3, 0.0);
        solver.set_group_param(&[GroupParam { groups: groups.clone(), parameter: "Mass".to_string(), value: 0.05 }, GroupParam { groups, parameter: "LFCoeff".to_string(), value: 0.1 }]);
        solver.set_chara_wind(&CharaWind { direction_space: 2, direction_vec: [0.0, 2.0, 8.0], base_strength: 1200.0, wave_amplitude: 15.0, wave_period: 0.1, groups: Vec::new() });
    }

    /// ONE FRAME OF THE VOID: the copy is posed at the view's time and handed to the renderer. `None` scene: the void is off.
    pub fn frame(scene: Option<&GameOverScene>, dt: f32, exposure: f32, light_scale: f32, camera_dz_cm: f32) {
        let Ok(mut guard) = VOID.lock() else { return };
        let Some(scene) = scene else {
            if let Some(void) = guard.as_mut() {
                if void.last.take().is_some() {
                    kh_er_render::set_void(None);
                    erlog!("game over: the void is taken off the screen (shown for {} frames)", void.frames);
                    void.frames = 0;
                }
            }
            return;
        };
        let Some(void) = guard.as_mut() else { return };
        // (ER party S6) a fallen friend leader: his copy is posed (handed to the renderer once it can draw him: S4)
        // (ER party S4) ... and handed to the renderer (his KO pose in the void; Sora's copy is then not drawn)
        #[cfg(feature = "party")]
        let friend_drawn = crate::partydraw::void_char(friend_pose(scene, dt));
        #[cfg(not(feature = "party"))]
        let friend_drawn = false;
        let first = void.last.is_none();
        if first {
            begin_cloth(&mut void.animator);
            erlog!("game over: THE VOID IS UP (sequence {}, face {}, effect {} = {} lights, camera FOV {} over the stage's width; nothing of Elden Ring's is drawn: the back buffer is cleared to black at Present)", scene.clip, scene.face, scene.effect, scene.lights.len(), scene.camera.fov_deg);
        }
        let component_to_world = Trs { translation: Vec3::ZERO, rotation: Quat::from_rotation_z(MESH_YAW_DEG.to_radians()), scale: Vec3::ONE };
        let (from, to) = (void.last.unwrap_or(-1.0).rem_euclid(scene.clip_seconds), scene.clip_time);
        let sequences = [scene.clip];
        let body = BodySlot { samples: [(scene.clip, 1.0), (scene.clip, 0.0)], blend_with: None, notify_sequences: &sequences, time: to, weight: 1.0, active: true, notify_window: if first { (-1.0, to.max(0.001)) } else { (from, to) } };
        let pose = void.animator.step(&Frame { dt: dt.clamp(0.0, 0.1), body, component_to_world }, &NoIkWorld).clone();
        void.last = Some(scene.seconds);
        void.frames += 1;
        let mut key = camera_of(&scene.camera);
        key.position += host_pos(Vec3::new(0.0, 0.0, camera_dz_cm));
        let camera = match FROM.lock().ok().and_then(|f| *f).filter(|_| scene.camera_blend < 1.0) {
            Some(from) => blend(&from, &key, scene.camera_blend),
            None => key,
        };
        kh_er_render::set_void(Some(kh_er_render::VoidFrame {
            input: kh_er_render::FrameInput {
                camera: kh_er_render::Camera { right: camera.right, up: camera.up, forward: camera.forward, position: camera.position, fov: camera.fov, aspect: 16.0 / 9.0, near: 0.3, far: 60.0 },
                origin: Vec3::ZERO,
                clock: scene.seconds,
                body: (!friend_drawn).then(|| kh_er_render::Body { component_to_world, local: pose.local, component: pose.component }),
                weapon: None,
                forms: Vec::new(),
            },
            lights: scene.lights.iter().map(|l| kh_look::PointLight { position: Vec3::new(0.0, 0.0, ROOT_ABOVE_FEET) + Vec3::from(l.position), color: Vec3::from(l.colour) * light_scale, radius: l.radius, exponent: l.exponent }).collect(),
            exposure,
            khs: kh_host::gameover::KHS_PRESET.to_string(),
        }));
    }

    // ------------------------------------------------------------------------------------ (ER party S6) the leader

    /// (ER party S6) THE FALLEN FRIEND LEADER IN THE VOID (the user: "the death screen should show the party leader in
    /// their death pose"; the test app's model: kh-app gameover_scene.rs `FriendCopy`): Donald's / Goofy's own pose
    /// evaluator from the party pack (`PACK/party/n_ex00x`), posed in his KO clip (KH3 has no game-over clip for them:
    /// the loop of their Die state, `BadStatStunLoop` = sequence 0242 of both sets, looped) with his feet
    /// [`FRIEND_FEET_UP`] above the void's origin; the camera, lights and fade are Sora's.
    #[cfg(feature = "party")]
    struct FriendVoid {
        chara: kh_host::party::PartyChara,
        animator: Option<Animator>,
        clip: (String, f32),
        last: Option<f32>,
        told: bool,
    }

    // SAFETY: as `Void`: plain data, made on a thread of its own, then only used under the mutex.
    #[cfg(feature = "party")]
    unsafe impl Send for FriendVoid {}

    #[cfg(feature = "party")]
    static FRIEND: Mutex<Option<FriendVoid>> = Mutex::new(None);

    /// The row of the friends' KO pose (their Die state's loop) and where their feet stand in the void (kh-app
    /// gameover_scene.rs: LOCAL, by eye, their stun pose lies on the ground where Sora floats).
    #[cfg(feature = "party")]
    pub const FRIEND_KO_ROW: &str = "BadStatStunLoop";
    #[cfg(feature = "party")]
    pub const FRIEND_FEET_UP: f32 = 30.0;

    /// (ER party S6) Whose death the void shows: `Some(friend)` = that friend leader's copy is read (on a thread of its
    /// own) and posed; `None` / Sora = Sora's own copy, as before.
    #[cfg(feature = "party")]
    pub fn set_leader(pack: &Path, leader: Option<kh_host::party::PartyChara>) {
        use kh_host::party::PartyChara;
        let Ok(mut guard) = FRIEND.lock() else { return };
        let Some(chara) = leader.filter(|c| *c != PartyChara::Sora) else {
            *guard = None;
            return;
        };
        if guard.as_ref().is_some_and(|f| f.chara == chara) {
            return;
        }
        *guard = Some(FriendVoid { chara, animator: None, clip: ("0242".to_string(), 1.0), last: None, told: false });
        drop(guard);
        // (DTH) his model from the party's one registry (the drawing's: kh_partyhost::poser), not a Donald-or-Goofy choice here
        let Some(npc) = kh_partyhost::poser::npc_of(kh_partyhost::poser::draw_id(chara)) else {
            erlog!("game over: {} has no model in the party's registry: the void shows Sora's copy", chara.name());
            if let Ok(mut guard) = FRIEND.lock() {
                *guard = None;
            }
            return;
        };
        let dir: PathBuf = pack.join("party").join(npc);
        let _ = std::thread::Builder::new().name("kh-void-friend".into()).spawn(move || {
            let started = std::time::Instant::now();
            let made = std::panic::catch_unwind(|| kh_anim::Pack::load(&dir).and_then(|read| Animator::new(read, Settings { leg_ik: false, kine_driver: true, face: true, bonamik: true, first_update_holds_position: kh_core::anim::FIRST_UPDATE_HOLDS_POSITION })));
            let manifest: Option<serde_json::Value> = std::fs::read_to_string(dir.join("manifest.json")).ok().and_then(|t| serde_json::from_str(t.trim_start_matches('﻿')).ok());
            let seq = manifest.as_ref().and_then(|m| m["logical_animations"].as_array()?.iter().find(|r| r["name"] == FRIEND_KO_ROW)?["asset"].as_str().map(|a| a.rsplit('.').next().unwrap_or(a).to_string())).unwrap_or_else(|| "0242".to_string());
            let Ok(mut guard) = FRIEND.lock() else { return };
            let Some(f) = guard.as_mut().filter(|f| f.chara == chara) else { return };
            match made {
                Ok(Ok(animator)) => {
                    let len = animator.clip_info(&seq).map_or(1.0, |c| c.length);
                    erlog!("game over: the void's copy of {} ({npc}, KO clip {seq} {len:.2} s) is read in {:.2} s", chara.name(), started.elapsed().as_secs_f32());
                    f.clip = (seq, len);
                    f.animator = Some(animator);
                }
                Ok(Err(e)) => erlog!("game over: the void's copy of {} could not be read from {} ({e}): Sora's copy is shown", chara.name(), dir.display()),
                Err(_) => erlog!("game over: PANIC while reading the void's copy of {}: Sora's copy is shown", chara.name()),
            }
        });
    }

    /// (ER party S6) The fallen friend leader's pose at the scene's time (component to world in the void's space, the
    /// local and component poses of HIS skeleton), or `None` (Sora's death, or his copy not read yet).
    #[cfg(feature = "party")]
    pub fn friend_pose(scene: &GameOverScene, dt: f32) -> Option<(kh_host::party::PartyChara, Trs, kh_anim::Pose)> {
        let mut guard = FRIEND.lock().ok()?;
        let f = guard.as_mut()?;
        let animator = f.animator.as_mut()?;
        let len = f.clip.1.max(1.0e-3);
        let to = scene.seconds % len;
        let from = f.last.map_or(-1.0, |l| l % len);
        let c2w = Trs { translation: Vec3::new(0.0, 0.0, FRIEND_FEET_UP), rotation: Quat::from_rotation_z(MESH_YAW_DEG.to_radians()), scale: Vec3::ONE };
        let seq = f.clip.0.clone();
        let sequences = [seq.as_str()];
        let body = BodySlot { samples: [(&seq, 1.0), (&seq, 0.0)], blend_with: None, notify_sequences: &sequences, time: to, weight: 1.0, active: true, notify_window: if f.last.is_none() { (-1.0, to.max(0.001)) } else { (from, to) } };
        let pose = animator.step(&Frame { dt: dt.clamp(0.0, 0.1), body, component_to_world: c2w }, &NoIkWorld).clone();
        f.last = Some(scene.seconds);
        if !f.told {
            f.told = true;
            erlog!("game over: THE FALLEN LEADER {} IS POSED in his KO clip {} (feet {FRIEND_FEET_UP} cm up). (ER party S4: drawn by the renderer in Sora's place, partydraw.rs `void_char`)", f.chara.name(), f.clip.0);
        }
        Some((f.chara, c2w, pose))
    }

    /// The payload goes away.
    pub fn shutdown() {
        kh_er_render::set_void(None);
        #[cfg(feature = "party")]
        if let Ok(mut f) = FRIEND.lock() {
            *f = None;
        }
        if let Some(handle) = LOADING.lock().ok().and_then(|mut l| l.take()) {
            let _ = handle.join();
        }
        if let Ok(mut guard) = VOID.lock() {
            *guard = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_void_camera_looks_back_at_him_from_his_front() {
        // game_over_01: 393 cm in front of the pawn, 82 cm up, looking back and 7.6 degrees down.
        let c = camera_of(&kh_host::gameover::CAMERA);
        assert!((c.position - Vec3::new(0.069_055_67, 0.817_649_6, 3.933_685)).length() < 1.0e-4, "{:?}", c.position);
        // A left-handed frame: right x up = forward; all unit.
        assert!((c.right.cross(c.up) - c.forward).length() < 1.0e-5);
        assert!((c.forward.length() - 1.0).abs() < 1.0e-5 && c.up.y > 0.99);
        // Forward = toward the pawn (host -Z = game -X) and slightly down.
        assert!(c.forward.z < -0.99 && c.forward.y < -0.13 && c.forward.y > -0.14);
        // Screen right is the pawn's left (game -Y = host -X): his head, toward +Y in the clip, is on the left of the picture.
        assert!(c.right.x < -0.99);
        // FOV 60 over the stage's width = 36 degrees vertical.
        assert!((c.fov.to_degrees() - 35.98).abs() < 0.05);
    }

    #[test]
    fn the_blend_starts_at_the_camera_of_the_death_and_ends_at_the_key() {
        let key = camera_of(&kh_host::gameover::CAMERA);
        let from = HostCamera { right: Vec3::X, up: Vec3::Y, forward: Vec3::Z, position: Vec3::new(0.0, 2.0, -4.0), fov: 0.8378 };
        let (a, b) = (blend(&from, &key, 0.0), blend(&from, &key, 1.0));
        assert!((a.position - from.position).length() < 1.0e-5 && (a.forward - from.forward).length() < 1.0e-4 && (a.fov - from.fov).abs() < 1.0e-6);
        assert!((b.position - key.position).length() < 1.0e-5 && (b.forward - key.forward).length() < 1.0e-4 && (b.right - key.right).length() < 1.0e-4);
        // Half way the frame is still orthonormal and left-handed.
        let h = blend(&from, &key, 0.5);
        assert!((h.right.cross(h.up) - h.forward).length() < 1.0e-4);
    }
}
