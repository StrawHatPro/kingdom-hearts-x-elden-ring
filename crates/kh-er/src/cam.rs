//! KINGDOM HEARTS III'S CAMERA IN ELDEN RING (IMPL_ER7_STATE.md): `kh_camera` (the field camera,
//! the lock-on camera, the attack cameras, the shotlock aim camera, the port's shot camera, the
//! effects' camera shakes) is stepped with Sora's fixed steps, exactly as `kh-app/src/player.rs`
//! steps it, and its view replaces Elden Ring's while Sora is in control.
//!
//! HOW THE VIEW GETS INTO THE GAME (after erfps2's `hooks.rs`, MIT OR Apache-2.0): the game's
//! camera step (`CAMERA_STEP_UPDATE`, found by pattern, 2.7.1.0 RVA 0x3bc5f0) is detoured. The
//! ORIGINAL IS ALWAYS CALLED: Elden Ring's own camera keeps being computed (cutscenes, the death
//! camera, the blend back). Then, while a view of ours is posted and fresh, `CSCamera::pers_cam_1`
//! (what the frame is drawn, culled and listened with) and `ChrCam::pers_cam` (what the game's
//! own lock-on code and anything else that asks "the player camera" read) are overwritten with
//! blend(Elden Ring's view, ours, weight). The posted view lapses 250 ms after the frame task
//! last renewed it, so a loading screen, a dead payload or a hot reload give the camera back by
//! themselves. A second detour hands the game's audio listener (`SET_WWISE_LISTENER`) the same
//! view when the matrix the game passes is its camera's.
//!
//! WHO HAS THE CAMERA: Sora, while he is alive and in control (`hud_state` = gameplay, the world
//! runs) and `cam.kh` is not 0. Elden Ring's menus, map, grace, cutscenes, dialogue, death and
//! loading have Elden Ring's camera; the change is a blend of `BLEND` seconds either way.
//!
//! Units and axes go through `coords.rs` only.

use std::collections::HashMap;
use std::ffi::c_void;
use std::io::Write;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

use eldenring::cs::{CSCam, CSCamera, WorldChrMan};
use fromsoftware_shared::FromStatic;
use glam::{Mat3, Quat, Vec2, Vec3};
use kh_camera::aim::AimOverlay;
use kh_camera::shake::{Oscillator, ShakeDef, ShakeOffset, ShakeSet};
use kh_camera::shot::ShotCamera;
use kh_camera::{CameraInput, CameraRig, CameraTarget, CameraView, Rot, TargetState, CAMERA_PARAMS};
use kh_core::{EventKind, Output, Sora, StateId, TargetId};
use windows_sys::Win32::System::SystemInformation::GetTickCount64;

use crate::control::Control;
use crate::coords::{self, Frame};
use crate::erlog;
use crate::game::plausible;
use crate::havok::{self, locate, Located};
use crate::targets::ErTargets;
use crate::word;
use crate::world::ErWorld;

/// (ER party S7) WHAT THE PARTY TELLS KH3'S CAMERA, every fixed step (party/team.rs implements it over the party core;
/// sora.rs passes `None` while the party is off, `party.er = 0`, or the `party` feature is not compiled: then every
/// line of `KhCam::step` is exactly as before the party).
pub trait PartyCamera {
    /// A FRIEND LEADS: the field / lock-on camera's target is his (kh-partyhost `camera_target`); `None` = Sora leads.
    /// `dt` = the world's step.
    fn leader_target(&self, dt: f32) -> Option<CameraTarget>;
    /// A friend leading changed his RB lock this step: `Some(None)` = let go, `Some(Some((id, point)))` = locked on.
    fn lock_change(&mut self) -> Option<Option<(u32, Vec3)>>;
    /// The friend leader's lock now (a camera made while he holds a lock).
    fn leader_lock(&self) -> Option<TargetId>;
    /// THE TEAM ATTACKS' CAMERAS (`StateEventCameras`: CameraAnims, the orbit cameras, the steerable shield_01, the
    /// collision pass against `world` = the camera's own filter of Elden Ring's world) laid over `view` (the field / lock-on
    /// camera of this step). True = put the field camera behind the leader again (the blueprints' ResetTresCameraNormal).
    #[allow(clippy::too_many_arguments)]
    fn team_layer(&mut self, dt: f32, events: &[kh_core::Event], sora: &Output, view: &mut CameraView, world: &dyn kh_core::World, stick: CameraInput) -> bool;
    /// What the drawn lens adds after `team_layer`: the CameraAnim's horizontal FOV (degrees; `None` = the field's) and the
    /// blueprints' camera shakes.
    fn team_lens(&self) -> (Option<f32>, ShakeOffset);
}

/// Seconds of the blend between Elden Ring's camera and ours (KH3's own camera blends are 0.5 s).
pub const BLEND: f32 = 0.5;
/// KH3's field of view: 100 degrees horizontal on the 16:9 frame (camera spec 1.5).
pub const FOV_KH3: f32 = 100.0;

pub(crate) const STEP_FN: Located = Located { name: "camera step update", pattern: "48 89 5c 24 08 57 48 83 ec 30 f3 0f 10 05 ? ? ? ? 48 8b fa 0f 29 74 24 20 48 8b d9 0f 57 f6 0f 2e c6 7a 16 75 14 b9 0f 00 00 00", capture: 0, rva_2_7_1_0: 0x003b_c5f0, rva_2_7_2_0: 0x003b_c5f0 /* (PORT) */ };
pub(crate) const LISTENER_FN: Located = Located { name: "audio listener (set_wwise_listener)", pattern: "48 83 ec 68 0f 28 42 10 0f 28 4a 20 48 8b 0d ? ? ? ? 0f 29 44 24 20 0f 28 42 30 0f 29 4c 24 30 0f 28 4a 40", capture: 0, rva_2_7_1_0: 0x00da_b120, rva_2_7_2_0: 0x00da_c810 /* (PORT) */ };

/// A camera as the game holds it: rows right / up / forward / position (Elden Ring space, m), the
/// vertical field of view in radians.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawView {
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
    pub position: Vec3,
    pub fov: f32,
}

impl RawView {
    fn quat(&self) -> Quat {
        Quat::from_mat3(&Mat3::from_cols(self.right, self.up, self.forward)).normalize()
    }

    /// The view between `self` (0) and `to` (1).
    pub fn blend(&self, to: &RawView, alpha: f32) -> RawView {
        let m = Mat3::from_quat(self.quat().slerp(to.quat(), alpha));
        RawView { right: m.x_axis, up: m.y_axis, forward: m.z_axis, position: self.position.lerp(to.position, alpha), fov: self.fov + (to.fov - self.fov) * alpha }
    }

    fn finite(&self) -> bool {
        self.right.is_finite() && self.up.is_finite() && self.forward.is_finite() && self.position.is_finite() && self.fov.is_finite() && self.fov > 0.01 && self.fov < 3.0
    }
}

/// What the frame task posts for the detours.
#[derive(Clone, Copy)]
struct Share {
    view: RawView,
    /// 0 = Elden Ring's camera, 1 = ours (already eased).
    weight: f32,
    /// `GetTickCount64` until which it holds.
    until: u64,
    /// Also write `ChrCam::pers_cam` (`cam.chr = 0` leaves it alone).
    chr_cam: bool,
    listener: bool,
    /// (ER62) Elden Ring's position of KH3's origin when `view` was made (the listener of Sora's own sounds is taken
    /// from `view` in KH3 space with it, so an origin shift between the post and its use moves nothing).
    origin: Vec3,
    /// (ER62) `audio.listener_fix` (default on): the game's audio listener gets the view on screen in EVERY call.
    listener_fix: bool,
}

static SHARE: Mutex<Option<Share>> = Mutex::new(None);
/// Elden Ring's own view of the latest camera step (before we overwrote anything), and what we wrote.
static ER_VIEW: Mutex<Option<RawView>> = Mutex::new(None);
static WRITTEN: Mutex<Option<RawView>> = Mutex::new(None);
static ORIGINAL_STEP: AtomicUsize = AtomicUsize::new(0);
static ORIGINAL_LISTENER: AtomicUsize = AtomicUsize::new(0);
/// Camera steps of the game that went through the detour / in which our view was written; listener
/// calls seen / given our view / whose matrix was NOT the game's camera (for the log).
pub static STEPS: AtomicU32 = AtomicU32::new(0);
pub static WRITES: AtomicU32 = AtomicU32::new(0);
pub static LISTENER_CALLS: AtomicU32 = AtomicU32::new(0);
pub static LISTENER_OURS: AtomicU32 = AtomicU32::new(0);
pub static LISTENER_OTHER: AtomicU32 = AtomicU32::new(0);
/// (ER62) Listener calls whose matrix was replaced by the view on screen; the latest matrix the game's listener got.
pub static LISTENER_REPLACED: AtomicU32 = AtomicU32::new(0);
static GAME_LISTENER: Mutex<Option<RawView>> = Mutex::new(None);
static LAST_STEP_TICK: AtomicU64 = AtomicU64::new(0);

fn read_cam(cam: &CSCam) -> RawView {
    let m = &cam.matrix;
    RawView { right: Vec3::new(m.0 .0, m.0 .1, m.0 .2), up: Vec3::new(m.1 .0, m.1 .1, m.1 .2), forward: Vec3::new(m.2 .0, m.2 .1, m.2 .2), position: Vec3::new(m.3 .0, m.3 .1, m.3 .2), fov: cam.fov }
}

fn write_cam(cam: &mut CSCam, v: &RawView) {
    let m = &mut cam.matrix;
    (m.0 .0, m.0 .1, m.0 .2) = (v.right.x, v.right.y, v.right.z);
    (m.1 .0, m.1 .1, m.1 .2) = (v.up.x, v.up.y, v.up.z);
    (m.2 .0, m.2 .1, m.2 .2) = (v.forward.x, v.forward.y, v.forward.z);
    (m.3 .0, m.3 .1, m.3 .2) = (v.position.x, v.position.y, v.position.z);
    cam.fov = v.fov;
}

fn fresh_share() -> Option<Share> {
    let share = (*SHARE.try_lock().ok()?)?;
    // SAFETY: plain getter.
    (unsafe { GetTickCount64() } < share.until && share.weight > 0.0 && share.view.finite()).then_some(share)
}

/// After the game's own camera step: its view is remembered, ours (blended) written over it.
fn after_camera_step() {
    STEPS.fetch_add(1, Ordering::Relaxed);
    // SAFETY: plain getter.
    LAST_STEP_TICK.store(unsafe { GetTickCount64() }, Ordering::Relaxed);
    // SAFETY: the game thread, inside the game's camera step; every pointer is checked.
    let Ok(cs) = (unsafe { CSCamera::instance_mut() }) else { return };
    if !plausible(word!(cs.pers_cam_1)) {
        return;
    }
    let now = read_cam(&cs.pers_cam_1);
    if !now.finite() {
        return;
    }
    // (a step in which the game did not write its camera leaves OUR last write there: not its view)
    let ours_still = WRITTEN.try_lock().ok().and_then(|w| *w).is_some_and(|w| w == now);
    let er = if ours_still { ER_VIEW.try_lock().ok().and_then(|v| *v).unwrap_or(now) } else { now };
    if let Ok(mut slot) = ER_VIEW.try_lock() {
        *slot = Some(er);
    }
    let Some(share) = fresh_share() else {
        if let Ok(mut w) = WRITTEN.try_lock() {
            *w = None;
        }
        return;
    };
    let out = if share.weight >= 1.0 { share.view } else { er.blend(&share.view, share.weight) };
    if !out.finite() {
        return;
    }
    write_cam(&mut cs.pers_cam_1, &out);
    if share.chr_cam {
        // SAFETY: as above.
        if let Ok(wcm) = unsafe { WorldChrMan::instance_mut() } {
            if let Some(mut cam) = crate::game::chr_cam(wcm) /* (AGGRO) at its place for the running version (2.7.2.0 moved it) */ {
                {
                    // SAFETY: the game's player camera object, alive with the world.
                    write_cam(unsafe { &mut cam.as_mut().pers_cam }, &out);
                }
            }
        }
    }
    if let Ok(mut w) = WRITTEN.try_lock() {
        *w = Some(read_cam(&cs.pers_cam_1));
    }
    WRITES.fetch_add(1, Ordering::Relaxed);
}

unsafe extern "C" fn camera_step_hook(this: *mut c_void, time: *const c_void) {
    let original: unsafe extern "C" fn(*mut c_void, *const c_void) = std::mem::transmute(ORIGINAL_STEP.load(Ordering::Relaxed));
    original(this, time);
    let _ = catch_unwind(after_camera_step);
}

/// (ER62) What the game's audio listener is handed while a view of ours is posted: THE VIEW ON SCREEN, whatever matrix
/// the game passed. The game hands it its OWN camera of this frame, computed before our write: in the user's log of
/// 2026-10-14, 96 of 138 seconds under KH3's camera read `given our view 0, not the camera's matrix 60` (the old test
/// compared that matrix with Elden Ring's view of the PREVIOUS step, so it matched only while Elden Ring's camera stood
/// still: the game's sounds were heard from Elden Ring's own camera, which looks elsewhere, while the camera moved and
/// from ours while it stood: they jumped from side to side). `given` stands for Elden Ring's view of the frame in a
/// blend, as in `after_camera_step`; a matrix that is already our written view is kept.
pub fn listener_view(given: &RawView, written: Option<RawView>, share_view: &RawView, weight: f32) -> RawView {
    let mut out = if weight >= 1.0 {
        *share_view
    } else {
        match written {
            Some(w) if (w.position - given.position).length() < 0.02 => w,
            _ => given.blend(share_view, weight.max(0.0)),
        }
    };
    out.fov = given.fov;
    out
}

/// The game's audio listener: while a view of ours is posted it gets the view on screen (`audio.listener_fix = 0`:
/// the rule before ER62, only when the matrix given is the camera's of the previous step).
unsafe extern "C" fn listener_hook(this: *mut c_void, cam: *const CSCam) -> u32 {
    let original: unsafe extern "C" fn(*mut c_void, *const CSCam) -> u32 = std::mem::transmute(ORIGINAL_LISTENER.load(Ordering::Relaxed));
    if cam.is_null() {
        return original(this, cam);
    }
    let replaced = catch_unwind(AssertUnwindSafe(|| {
        LISTENER_CALLS.fetch_add(1, Ordering::Relaxed);
        let share = fresh_share().filter(|s| s.listener)?;
        let given = read_cam(&*cam);
        let er = ER_VIEW.try_lock().ok().and_then(|v| *v);
        let written = WRITTEN.try_lock().ok().and_then(|w| *w);
        let ours = written.is_some_and(|w| (w.position - given.position).length() < 0.02);
        let er_matches = er.is_some_and(|e| (e.position - given.position).length() <= 0.02);
        if share.listener_fix {
            if ours || er_matches {
                LISTENER_OURS.fetch_add(1, Ordering::Relaxed);
            } else {
                LISTENER_OTHER.fetch_add(1, Ordering::Relaxed);
            }
            if !given.finite() {
                return None;
            }
            let view = listener_view(&given, written, &share.view, share.weight);
            if !view.finite() {
                return None;
            }
            LISTENER_REPLACED.fetch_add(1, Ordering::Relaxed);
            if let Ok(mut slot) = GAME_LISTENER.try_lock() {
                *slot = Some(view);
            }
            let mut copy = std::ptr::read(cam);
            write_cam(&mut copy, &view);
            return Some(copy);
        }
        // (the rule before ER62)
        let er = er?;
        if ours {
            // (already the view on screen)
            LISTENER_OURS.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        if !er_matches {
            LISTENER_OTHER.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        LISTENER_OURS.fetch_add(1, Ordering::Relaxed);
        let mut copy = std::ptr::read(cam);
        write_cam(&mut copy, &if share.weight >= 1.0 { share.view } else { er.blend(&share.view, share.weight) });
        copy.fov = given.fov;
        Some(copy)
    }))
    .ok()
    .flatten();
    match replaced {
        Some(copy) => original(this, &copy),
        None => original(this, cam),
    }
}

/// (ER62) The view on screen while a view of ours is posted (blended with Elden Ring's own in a hand-over), with the
/// Elden Ring position of KH3's origin it was made with. `None`: Elden Ring has its own camera.
pub fn rendered_view() -> Option<(RawView, Vec3)> {
    let share = fresh_share()?;
    let view = if share.weight >= 1.0 { share.view } else { er_view()?.blend(&share.view, share.weight) };
    view.finite().then_some((view, share.origin))
}

/// (ER62) The latest matrix the game's audio listener was handed by our detour (`None`: none yet).
pub fn game_listener() -> Option<RawView> {
    GAME_LISTENER.try_lock().ok().and_then(|v| *v)
}

/// Detours the camera step and the audio listener (once per payload; undone by the renderer's
/// shutdown with every other inline hook). False: Elden Ring keeps its camera (logged).
pub fn install() -> bool {
    let Some(step) = locate(&STEP_FN) else {
        erlog!("cam: the game's camera step was not found for this executable (PORT: 2.7.1.0 / 2.7.2.0 table): Elden Ring keeps its own camera");
        return false;
    };
    // SAFETY: the function's start, found by pattern at the recorded address; our hook has its signature.
    match unsafe { kh_er_render::inline_hook(step, camera_step_hook as *const () as usize, &ORIGINAL_STEP, "camera step") } {
        Ok(()) => erlog!("cam: the game's camera step is detoured (the original runs, then KH3's view is written while Sora has the camera)"),
        Err(e) => {
            ORIGINAL_STEP.store(0, Ordering::SeqCst);
            erlog!("cam: the camera step was NOT detoured ({e}): Elden Ring keeps its own camera");
            return false;
        }
    }
    match locate(&LISTENER_FN) {
        // SAFETY: as above.
        Some(f) => match unsafe { kh_er_render::inline_hook(f, listener_hook as *const () as usize, &ORIGINAL_LISTENER, "audio listener") } {
            Ok(()) => erlog!("cam: the game's audio listener is detoured"),
            Err(e) => {
                ORIGINAL_LISTENER.store(0, Ordering::SeqCst);
                erlog!("cam: the audio listener was NOT detoured ({e}): Elden Ring's sounds are heard from where it last put its listener");
            }
        },
        None => erlog!("cam: the audio listener function was not found: left alone"),
    }
    true
}

/// The hooks are gone (the renderer's shutdown removed every inline hook).
pub fn uninstalled() {
    if let Ok(mut s) = SHARE.lock() {
        *s = None;
    }
    ORIGINAL_STEP.store(0, Ordering::SeqCst);
    ORIGINAL_LISTENER.store(0, Ordering::SeqCst);
}

pub fn hooked() -> bool {
    ORIGINAL_STEP.load(Ordering::Relaxed) != 0
}

/// Elden Ring's own view of its latest camera step (not ours), when the detour has seen one.
pub fn er_view() -> Option<RawView> {
    ER_VIEW.try_lock().ok().and_then(|v| *v)
}

/// The view a KH3 camera view is in Elden Ring's space. `fov` vertical, radians.
pub fn raw_of(frame: &Frame, position: Vec3, rotation: Rot, fov: f32) -> RawView {
    let forward = rotation.forward();
    let yaw = rotation.yaw.to_radians();
    let flat_right = Vec3::new(-yaw.sin(), yaw.cos(), 0.0);
    // (left-handed on both sides: right x up = forward, so up = forward x right)
    let flat_up = forward.cross(flat_right);
    // Roll (the shakes'): about the forward axis, positive = clockwise as seen from behind.
    let (s, c) = rotation.roll.to_radians().sin_cos();
    let right = flat_right * c - flat_up * s;
    let up = flat_up * c + flat_right * s;
    RawView { right: coords::unit_kh_to_er(right), up: coords::unit_kh_to_er(up), forward: coords::unit_kh_to_er(forward), position: frame.pos_kh_to_er(position), fov }
}

/// The vertical field of view (radians) of a horizontal one (degrees) on the 16:9 frame: KH3's
/// camera is constrained to 16:9, so at any other window shape the vertical angle is kept.
pub fn fov_vertical(horizontal_degrees: f32) -> f32 {
    2.0 * ((horizontal_degrees.to_radians() * 0.5).tan() / CAMERA_PARAMS.fov_aspect_ratio).atan()
}

/// (ER party S7) The horizontal FOV drawn (degrees): a team camera's CameraAnim FOV plus its shakes' FOV while it has
/// the view (kh-app camera.rs), else the field camera's (`cam.fov`, KH3's 100).
pub fn team_fov_h(field: f32, team: Option<f32>, shake_fov: f32) -> f32 {
    match team {
        Some(f) if f.is_finite() => (f + shake_fov).clamp(5.0, 170.0),
        _ => field,
    }
}

/// (ER party S7) The boss finish's "player" cut at a friend leader: Sora's camera-bone height (`finish.player_height`)
/// scaled by the leader's capsule half height against Sora's (Donald is shorter, Goofy taller).
pub fn leader_cut_height(player_height: f32, leader_half: f32, sora_half: f32) -> f32 {
    if leader_half > 1.0 && sora_half > 1.0 {
        player_height * leader_half / sora_half
    } else {
        player_height
    }
}

/// What the camera is told about the pawn (as kh-app's `camera_target`).
pub fn target_of(out: &Output, before: &Output, sora: &Sora) -> CameraTarget {
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
        collision_half_height: sora.params().capsule_half_height, // (ER69) the outfit's body (was SORA_PARAMS)
    };
    if let kh_core::CameraState::WallRun(class) = sora.camera_state() {
        target.state = TargetState::WallRun(class);
    }
    if !matches!(target.state, TargetState::Idle | TargetState::Run | TargetState::Other) && target.velocity == Vec3::ZERO {
        target.velocity = (out.position - before.position) * 60.0;
    }
    target
}

/// One step's input of the devices for the cameras.
#[derive(Clone, Copy, Debug, Default)]
pub struct StepInput {
    /// Right stick, raw (x right, y up), BEFORE the "stick up = look up" option.
    pub right: Vec2,
    /// Left stick, raw (the aim camera turns by it while the right one rests).
    pub left: Vec2,
    pub recentre: bool,
}

pub struct KhCam {
    rig: Option<CameraRig>,
    aim: AimOverlay,
    shot: ShotCamera,
    shot_on: bool,
    /// (IMPL_ER22) KH3's boss finish: the cut of this step and where its subjects are (set by `set_finish` before
    /// every step), the camera of the cuts, the world's rate our cameras run at.
    finish_shot: Option<kh_core::combat::finish::FinishShot>,
    finish_enemy: kh_camera::finish::FinishSubject,
    finish_player_height: f32,
    finish_collision: bool,
    finish_locked: bool,
    finish: kh_camera::finish::FinishCamera,
    world_rate: f32,
    previous: CameraView,
    view: CameraView,
    world: ErWorld,
    /// Blend weight of our camera over Elden Ring's, in linear time (eased when posted).
    weight: f32,
    owning: bool,
    /// Frames since we last had the view.
    away: u32,
    relock_pending: bool,
    /// The user's host option (kh-app `camera_stick_y`): -1 = stick up looks up (default).
    stick_y: f32,
    fov_h: f32,
    shakes: ShakeSet,
    defs: HashMap<String, ShakeDef>,
    shakes_played: u32,
    diag: bool,
    trace: Option<std::io::BufWriter<std::fs::File>>,
    trace_path: std::path::PathBuf,
    told: Option<bool>,
    /// (IMPL_ER13) A Kingdom Hearts menu holds the pause: the view that was last posted is posted again unchanged.
    frozen: bool,
    posted: Option<(RawView, f32)>,
    frozen_told: bool,
    // per-second numbers
    collided: u32,
    steps: u32,
    mode: &'static str,
    /// (ER party S7) the friend leader's camera target of the last step (`None`: Sora leads, or no party): the
    /// take-over and the "lost" test of `frame` look at him.
    party_target: Option<CameraTarget>,
    /// (WALLS cutscene fix) put the camera behind the leader at once, full weight, as soon as it has the view (`snap_back`)
    snap_due: bool,
    /// (ER party S7) the team attacks' lens of the last step (`PartyCamera::team_lens`) and whether a team camera had
    /// the view (the log's mode).
    team_fov: Option<f32>,
    team_shake: ShakeOffset,
    team_on: bool,
    /// (CV1) THE CONVERSATION CAMERA (kh_camera::conv; conv.rs feeds it): its shots over the field camera while a talk
    /// runs, the blend back at the end; its lens while it has a part in the view.
    conv: kh_camera::conv::ConvCamera,
    conv_fov: Option<f32>,
}

impl KhCam {
    pub fn new(pack_dir: &Path, logs: &Path, frame: Frame) -> KhCam {
        let mut world = ErWorld::new(frame);
        world.filter = havok::FILTER_CAMERA;
        let mut defs = HashMap::new();
        // The camera shake classes of the effect pack (as kh-app/src/vfx.rs reads them).
        if let Some(host) = std::fs::read_to_string(pack_dir.join("vfx").join("host.json")).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) {
            if let Some(shakes) = host.get("camera_shakes").and_then(serde_json::Value::as_object) {
                let num = |v: &serde_json::Value, key: &str| v.get(key).and_then(serde_json::Value::as_f64).unwrap_or(0.0) as f32;
                let osc = |v: Option<&serde_json::Value>| v.map_or(Oscillator::default(), |v| Oscillator { amplitude: num(v, "amplitude"), frequency: num(v, "frequency"), start_at_zero: v.get("start_at_zero").and_then(serde_json::Value::as_bool).unwrap_or(false) });
                let three = |v: Option<&serde_json::Value>| [0, 1, 2].map(|k| osc(v.and_then(|v| v.get(k))));
                for (class, d) in shakes {
                    defs.insert(class.clone(), ShakeDef { name: class.clone(), duration: num(d, "duration"), blend_in: num(d, "blend_in"), blend_out: num(d, "blend_out"), rot: three(d.get("rot")), loc: three(d.get("loc")), fov: osc(d.get("fov")), single_instance: d.get("single_instance").and_then(serde_json::Value::as_bool).unwrap_or(false) });
                }
            }
        }
        erlog!("cam: {} camera shake class(es) read from the effect pack", defs.len());
        let nothing = CameraView { position: Vec3::ZERO, rotation: Rot::default(), rotation_target: Rot::default(), look_point: Vec3::ZERO, distance: 0.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false };
        KhCam { rig: None, aim: AimOverlay::default(), shot: ShotCamera::default(), shot_on: true, finish_shot: None, finish_enemy: kh_camera::finish::FinishSubject { point: Vec3::ZERO, yaw: 0.0 }, finish_player_height: 102.0, finish_collision: true, finish_locked: false, finish: kh_camera::finish::FinishCamera::default(), world_rate: 1.0, previous: nothing, view: nothing, world, weight: 0.0, owning: false, away: u32::MAX, relock_pending: false, stick_y: -1.0, fov_h: FOV_KH3, shakes: ShakeSet::new(), defs, shakes_played: 0, diag: false, trace: None, trace_path: logs.join("cam-trace.csv"), told: None, frozen: false, posted: None, frozen_told: false, collided: 0, steps: 0, mode: "field", party_target: None, snap_due: false /* (WALLS) */, team_fov: None, team_shake: ShakeOffset::default(), team_on: false /* (ER party S7) */, conv: kh_camera::conv::ConvCamera::default(), conv_fov: None /* (CV1) */ }
    }

    /// Sora has the camera (the blend counts: more than half ours).
    pub fn has_view(&self) -> bool {
        self.rig.is_some() && self.weight >= 0.5
    }

    /// The yaw the controller steers by: the camera of the last step (camera spec 1.4).
    pub fn input_yaw(&self) -> Option<f32> {
        self.has_view().then(|| self.view.rotation.yaw)
    }

    /// The view the shotlock reticle test projects with (as kh-app's `MagicHost::view`).
    pub fn core_view(&self) -> Option<kh_core::magic::View> {
        self.has_view().then(|| kh_core::magic::View::from_camera(self.view.position, self.view.rotation.yaw, self.view.rotation.pitch, self.fov_h, 1280, 720))
    }

    pub fn aiming(&self) -> bool {
        self.aim.active()
    }

    /// Once a frame BEFORE the steps: who has the camera, the options. `wanted` = Sora is alive
    /// and in control. A takeover from Elden Ring's camera starts ours behind HIS view.
    /// `frozen` (IMPL_ER13, the user: "the battle and camp pause menu make the camera angle change while it's paused
    /// which looks weird"): A MENU OF OURS HOLDS THE PAUSE (camp menu, battle Pause, save-point menu, the game-over hold,
    /// the choice menu): nothing about the view changes: no hand-back to Elden Ring's camera, no blend, no follow, no
    /// shake; the view posted last is posted again as it is, and play goes on from it when the menu closes.
    pub fn frame(&mut self, control: &Control, wanted: bool, frozen: bool, out: &Output, before: &Output, sora: &Sora, frame: Frame) {
        self.world.frame = frame;
        let frozen = frozen && self.rig.is_some() && self.posted.is_some() && control.flag("cam.freeze") != Some(false);
        if frozen != self.frozen {
            self.frozen = frozen;
            if !self.frozen_told || control.on("cam.diag") {
                self.frozen_told = true;
                erlog!("cam: {}", if frozen { "a Kingdom Hearts menu holds the pause: THE VIEW IS FROZEN as it was posted last (weight unchanged, no hand-back to Elden Ring's camera)" } else { "the menu is closed: the camera goes on from the frozen view (no blend)" });
            }
        }
        if frozen {
            return;
        }
        let on = control.flag("cam.kh") != Some(false) && hooked();
        self.stick_y = if control.on("cam.invert_y") { 1.0 } else { -1.0 };
        self.fov_h = control.f32("cam.fov").filter(|v| (30.0..=140.0).contains(v)).unwrap_or(FOV_KH3);
        self.shot_on = control.flag("cam.shot") != Some(false);
        self.diag = control.on("cam.diag");
        let owning = on && wanted;
        // A SHORT interruption (the pause menu, a pick-up banner) gives back the KH3 view he had: the
        // camera underneath went on following him. After a long one (a grace, a cutscene, a warp:
        // more than 3 s, or he is no longer where it looks) it starts afresh from Elden Ring's view.
        self.away = if self.owning { 0 } else { self.away.saturating_add(1) };
        // (ER party S7) the leader's feet (a friend leading: his, as the last step left them; Sora's as before)
        let lead_feet = self.party_target.map_or(out.position, |t| t.position);
        let lost = self.away > 180 || (self.view.look_point - lead_feet).length() > 1500.0;
        if owning && (self.rig.is_none() || (!self.owning && lost)) {
            // Behind Elden Ring's view (its yaw), KH3's start pitch: the blend starts from his picture.
            let mut target = self.party_target.unwrap_or_else(|| target_of(out, before, sora)); // (ER party S7) the leader's
            // (only a view that IS a camera on him: right after a load the game's camera is not placed yet;
            // then KH3's own start: behind Sora, camera spec 2.9)
            let sora_er = frame.pos_kh_to_er(lead_feet); // (ER party S7) the leader's
            if let Some(er) = er_view().filter(|er| (er.position - sora_er).length() < 30.0 && er.forward.length() > 0.5) {
                target.yaw = coords::yaw_kh_of_er_dir(er.forward);
            }
            let rig = CameraRig::new(CAMERA_PARAMS, &target);
            self.view = rig.view();
            self.previous = self.view;
            self.rig = Some(rig);
            self.aim = AimOverlay::default();
            self.shot = ShotCamera::default();
            self.relock_pending = true;
        }
        if self.told != Some(owning) {
            erlog!("cam: {}", if owning { "KINGDOM HEARTS III's camera has the view (cam.kh = 0 gives Elden Ring's back for good)" } else if !on { "Elden Ring's camera (cam.kh = 0, or the camera step is not detoured)" } else { "Elden Ring's camera (a menu / cutscene / death / the world stands still): KH3's camera blends out" });
            self.told = Some(owning);
        }
        self.owning = owning;
        let tracing = control.on("cam.trace");
        if tracing && self.trace.is_none() {
            self.trace = std::fs::File::create(&self.trace_path).ok().map(|f| {
                let mut w = std::io::BufWriter::new(f);
                let _ = writeln!(w, "step,sora_x,sora_y,sora_z,sora_yaw,state,cam_x,cam_y,cam_z,pitch,yaw,target_pitch,target_yaw,look_x,look_y,look_z,distance,stick_x,stick_y,collided,followed,mode,weight");
                w
            });
        } else if !tracing {
            self.trace = None;
        }
    }

    /// (IMPL_ER22) Before every step: the boss finish's cut (`None` = none), its enemy's camera point, the world's
    /// rate (1.0 outside a sequence), whether the script holds the sticks; `shake` = (stop the shakes, play the
    /// script's shake at this scale).
    pub fn set_finish(&mut self, shot: Option<kh_core::combat::finish::FinishShot>, enemy: kh_camera::finish::FinishSubject, player_height: f32, collision: bool, locked: bool, rate: f32, shake: (bool, Option<f32>)) {
        (self.finish_shot, self.finish_enemy, self.finish_player_height, self.finish_collision, self.finish_locked, self.world_rate) = (shot, enemy, player_height, collision, locked, rate.clamp(0.0, 1.0));
        if shake.0 {
            self.shakes.clear();
        }
        if let Some(scale) = shake.1 {
            self.shakes.play(&kh_camera::finish::shake_def(), scale, None);
            self.shakes_played += 1;
        }
    }

    /// One fixed step, AFTER Sora's (as kh-app/src/player.rs `simulate`).
    /// `party` (ER party S7): the party's side of the camera (a friend leader's field / lock-on target, the team
    /// attacks' cameras); `None` = Sora alone, exactly as before the party.
    #[allow(clippy::too_many_arguments)]
    /// (NF) The look point of KH3's camera while it has the view (KH world = the renderer's game space, cm): where KH3's
    /// collision pass sweeps from (FUN_1405D87F0), for the camera fade (kh-er-render camfade.rs). `None`: Elden Ring's camera.
    pub fn look_point(&self) -> Option<Vec3> {
        (self.owning && self.view.look_point.is_finite()).then_some(self.view.look_point)
    }

    pub fn step(&mut self, dt: f32, input: StepInput, out: &Output, before: &Output, sora: &Sora, targets: &ErTargets, shakes: Vec<kh_host::vfx::Shake>, mut party: Option<&mut dyn PartyCamera>) {
        // The effects' camera shakes (`SQEX_CameraShake`): played on whatever camera is in use.
        for request in shakes {
            match self.defs.get(&request.class) {
                Some(def) => {
                    self.shakes.play(def, request.scale, request.duration);
                    self.shakes_played += 1;
                }
                None => erlog!("cam: camera shake class {} is not in the effect pack's host.json: not played", request.class),
            }
        }
        self.shakes.step(dt);
        // (IMPL_ER22) the shakes above keep real time (KH3's finish shake is m_bRealDeltaTime); the cameras run on the world's clock
        let dt = dt * self.world_rate;
        let Some(rig) = self.rig.as_mut() else { return };
        let sora_target = target_of(out, before, sora);
        // (ER party S7) A FRIEND LEADS: the field and lock-on cameras follow HIM (kh-app player.rs: `party_play::camera_target`)
        let friend = party.as_ref().and_then(|p| p.leader_target(dt));
        self.party_target = friend;
        let target = friend.unwrap_or(sora_target);
        // (WALLS cutscene fix) the snap back behind the leader (`snap_back`), once KH3's camera has the view
        if self.snap_due && self.owning {
            self.snap_due = false;
            *rig = CameraRig::new(CAMERA_PARAMS, &target);
            self.view = rig.view();
            self.previous = self.view;
            self.aim = AimOverlay::default();
            self.shot = ShotCamera::default();
            self.relock_pending = true;
            self.weight = 1.0;
            erlog!("cam: (WALLS) the game handed control back: KH3's camera is put BEHIND THE LEADER at once (yaw {:.1}, pitch {:.1}, {:.0} cm), full weight", self.view.rotation.yaw, self.view.rotation.pitch, self.view.distance);
        }
        // (a camera made while a lock is already on: after a menu, a respawn)
        if std::mem::take(&mut self.relock_pending) {
            match (friend.is_some(), party.as_ref().and_then(|p| p.leader_lock())) {
                // (ER party S7) the friend leader's lock
                (true, Some(id)) => rig.set_lockon(&target, targets.get(id).map(|e| (id.0, e.pos + Vec3::Z * e.half_height))),
                (true, None) => {}
                (false, _) => {
                    let lock = sora.combat().lockon;
                    if lock.locked && lock.current != TargetId::NONE {
                        rig.set_lockon(&target, targets.get(lock.current).map(|e| (lock.current.0, e.pos + Vec3::Z * e.half_height)));
                    }
                }
            }
        }
        // ---- the core's camera events of this step
        for event in &out.events {
            match event.kind {
                // (ER party S7) AI Sora's lock, aim and shot are not the player's camera while a friend leads
                EventKind::CameraLock { .. } | EventKind::AimMode { .. } if friend.is_some() => {}
                EventKind::CameraLock { target: id } => {
                    let at = targets.get(id).map(|e| (id.0, e.pos + Vec3::Z * e.half_height));
                    rig.set_lockon(&target, at);
                }
                EventKind::AttackCamera { mode, restore } => rig.set_attack_mode(mode, restore),
                // (IMPL_ER37, user request) the aim starts on the lock-on target the core remembered at entry (as CameraLock's point);
                // (IMPL_ER42, user request "shotlock should aim at softlock target") without a hard lock on the SOFT target
                EventKind::AimMode { open: true, .. } => {
                    let aim = &sora.magic().aim;
                    let point_of = |id: TargetId| (id != TargetId::NONE).then(|| targets.get(id)).flatten().map(|e| e.pos + Vec3::Z * e.half_height);
                    let (hard, soft) = (point_of(aim.saved_lock), point_of(aim.saved_soft));
                    let point = kh_camera::aim::start_point(hard, soft);
                    let (kind, id) = if hard.is_some() { ("lock-on", aim.saved_lock) } else { ("soft", aim.saved_soft) };
                    erlog!("cam: aim opened {}", point.map_or("behind Sora (no lock-on, no soft target)".to_string(), |p| format!("on {kind} target {} at ({:.0}, {:.0}, {:.0})", id.0, p.x, p.y, p.z)));
                    self.aim.open_toward(&sora_target, &self.view, point);
                }
                EventKind::AimMode { open: false, reason } => {
                    self.aim.close();
                    // The port's shot camera (kh_camera::shot; `cam.shot = 0`: the game's own, none).
                    if self.shot_on {
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
                }
                _ => {}
            }
        }
        // (ER party S7) the friend leader's RB lock moved the lock-on camera (kh-app party_play.rs `after_step`)
        if let Some(change) = party.as_mut().and_then(|p| p.lock_change()) {
            rig.set_lockon(&target, change);
        }
        self.previous = self.view;
        let aiming = self.aim.active();
        // (IMPL_ER21) KH3's Config rows: Vertical / Horizontal Camera Control (flip) and Sensitivity, Camera Work
        let (config_x, config_y, config_auto) = crate::erconfig::camera();
        let stick = Vec2::new(input.right.x * config_x, input.right.y * self.stick_y * config_y);
        let camera_input = CameraInput { right_stick: stick, recentre: input.recentre, extra_rotation: Vec2::ZERO };
        let shot_owns = self.shot.owns_view();
        let field_input = if aiming || shot_owns || !self.owning || self.finish_locked { CameraInput::default() } else { camera_input };
        let lock_at = rig.locked().and_then(|id| targets.get(TargetId(id))).map(|e| e.pos + Vec3::Z * e.half_height);
        rig.field.set_auto_camera(config_auto);
        crate::boc::camera_socket(&mut rig.field.params_mut().target_socket_height); // (IMPL_ER59) the outfit's height (KH1 Sora is shorter)
        let under = rig.step(dt, &field_input, &target, lock_at, &self.world);
        let mut view = self.aim.step(dt, if self.owning { stick } else { Vec2::ZERO }, if self.owning { input.left } else { Vec2::ZERO }, &sora_target, &self.world, &under);
        // (ER party S7) AI Sora's shotlock aim is not the player's camera while a friend leads (kh-app player.rs)
        if friend.is_some() {
            view = under;
        }
        let in_state = out.state == StateId::Shootflow;
        if self.shot.owns_view() && !in_state && before.state != StateId::Shootflow && sora.aim().mode != 2 {
            self.shot.end();
            rig.field.reset(&target, true);
        }
        if self.shot.active() {
            let locks: Vec<Vec3> = sora.aim().locks.iter().filter_map(|(id, _)| targets.get(*id)).map(|e| e.pos + Vec3::Z * e.half_height).collect();
            let focus = (!locks.is_empty()).then(|| locks.iter().sum::<Vec3>() / locks.len() as f32);
            let wide = in_state && matches!(out.sub_step, 5 | 2 | 6);
            let below = if self.shot.owns_view() { rig.view() } else { view };
            view = self.shot.step(dt, &sora_target, focus, wide, &self.world, &below);
        }
        // (ER party S7) THE TEAM ATTACKS' CAMERAS over the field / lock-on camera (kh-app player.rs `team_camera`), with
        // the collision pass against THE CAMERA'S filter of Elden Ring's world (never into the terrain), shield_01
        // steered by the player's stick; their end puts the field camera behind the leader (the blend back). The boss
        // finish's cut below stays on top of them.
        match party.as_mut() {
            Some(p) => {
                let team_stick = if self.owning && !self.finish_locked { camera_input } else { CameraInput::default() };
                if p.team_layer(dt, &out.events, out, &mut view, &self.world, team_stick) {
                    rig.field.reset(&target, true);
                }
                (self.team_fov, self.team_shake) = p.team_lens();
            }
            None => (self.team_fov, self.team_shake) = (None, ShakeOffset::default()),
        }
        self.team_on = self.team_fov.is_some();
        // (CV1) THE CONVERSATION CAMERA over the field camera (its shots, the collision pass against the camera's filter of
        // the game's world, the blend in and back); the boss finish's cut stays on top
        self.conv_fov = None;
        if let Some(scene) = crate::conv::camera_scene() {
            if scene.running && !self.conv.running() && !scene.subjects.is_empty() {
                let a = scene.subjects[0].head;
                let b = scene.subjects.get(1).map_or(a + Vec3::X * 100.0, |s| s.head);
                self.conv.begin(view.position, a, b);
                erlog!("cam: the conversation camera takes the view ({} subject(s))", scene.subjects.len());
            }
            for cue in scene.cues {
                self.conv.shot(cue);
            }
            if !scene.running && self.conv.running() {
                self.conv.end();
                erlog!("cam: the conversation ended: the view blends back to the field camera ({} shot(s), pulled in by the level {} step(s))", self.conv.shots, self.conv.collided);
            }
            if let Some((v, fov)) = self.conv.step(dt, &scene.subjects, Some(&self.world), &view, self.fov_h) {
                view = v;
                self.conv_fov = Some(fov);
            }
        } else if self.conv.active() {
            self.conv.end();
            if let Some((v, fov)) = self.conv.step(dt, &[], Some(&self.world), &view, self.fov_h) {
                view = v;
                self.conv_fov = Some(fov);
            }
        }
        // (IMPL_ER22) THE BOSS FINISH'S CUT on top of every camera of ours: a hard cut in, a hard cut back
        let finish_was = self.finish.active();
        // (ER party S7) the cut of "the player" frames the LEADER (a friend: at his own camera height)
        let player = match friend {
            Some(t) => kh_camera::finish::FinishSubject { point: t.position + Vec3::Z * leader_cut_height(self.finish_player_height, t.collision_half_height, sora.params().capsule_half_height), yaw: t.yaw },
            None => kh_camera::finish::FinishSubject { point: out.position + Vec3::Z * self.finish_player_height, yaw: out.yaw },
        };
        let finish_world: Option<&dyn kh_core::World> = if self.finish_collision { Some(&self.world) } else { None };
        match self.finish.step(self.finish_shot.as_ref(), &self.finish_enemy, &player, finish_world, &view) {
            Some((cut_view, cut)) => {
                view = cut_view;
                if cut {
                    self.previous = cut_view;
                    erlog!("finish: CUT {} on the {:?}: camera KH ({:.0}, {:.0}, {:.0}) looking at ({:.0}, {:.0}, {:.0}), {:.0} cm{}", self.finish_shot.map_or(0, |s| s.index + 1), self.finish_shot.map(|s| s.subject), cut_view.position.x, cut_view.position.y, cut_view.position.z, cut_view.look_point.x, cut_view.look_point.y, cut_view.look_point.z, cut_view.distance, if cut_view.collided { " (pulled in by the level)" } else { "" });
                }
            }
            None if finish_was => self.previous = view,
            None => {}
        }
        self.mode = if self.finish.active() { "finish" } else if self.team_on { "team" /* (ER party S7) */ } else if self.conv_fov.is_some() { "conversation" /* (CV1) */ } else if self.shot.active() { "shot" } else if aiming { "aim" } else if rig.locked().is_some() { "lock-on" } else if rig.attack_camera().mode != 0 { "attack" } else { "field" };
        self.view = view;
        self.steps += 1;
        self.collided += u32::from(view.collided);
        if let Some(trace) = self.trace.as_mut() {
            let _ = writeln!(
                trace,
                "{},{:.2},{:.2},{:.2},{:.2},{:?},{:.2},{:.2},{:.2},{:.3},{:.3},{:.3},{:.3},{:.2},{:.2},{:.2},{:.2},{:.3},{:.3},{},{},{},{:.2}",
                out.step, out.position.x, out.position.y, out.position.z, out.yaw, out.state, view.position.x, view.position.y, view.position.z, view.rotation.pitch, view.rotation.yaw,
                view.rotation_target.pitch, view.rotation_target.yaw, view.look_point.x, view.look_point.y, view.look_point.z, view.distance, input.right.x, input.right.y, view.collided as u8, view.followed as u8, self.mode, self.weight
            );
        }
    }

    /// Once a frame AFTER the steps: the blend weight moves, the view to draw (between the two
    /// latest steps by `alpha`, with the shakes) is posted for the detours.
    pub fn post(&mut self, dt: f32, alpha: f32, control: &Control) {
        if self.frozen {
            if let (Some((view, weight)), Ok(mut s)) = (self.posted, SHARE.try_lock()) {
                // SAFETY: plain getter.
                *s = Some(Share { view, weight, until: unsafe { GetTickCount64() } + 250, chr_cam: control.flag("cam.chr") != Some(false), listener: control.flag("cam.listener") != Some(false), origin: self.world.frame.origin, listener_fix: control.flag("audio.listener_fix") != Some(false) });
            }
            return;
        }
        let rate = dt / BLEND;
        self.weight = if self.owning { (self.weight + rate).min(1.0) } else { (self.weight - rate).max(0.0) };
        if self.rig.is_none() || self.weight <= 0.0 {
            if let Ok(mut s) = SHARE.try_lock() {
                *s = None;
            }
            self.posted = None;
            return;
        }
        // (INT) a CUT between the two latest steps (a conversation shot, a team attack's camera, a jump) is never blended
        let alpha = if crate::interp::camera_cut(&self.previous, &self.view) { 1.0 } else { alpha };
        let (position, mut rotation) = self.view.lerp(&self.previous, alpha.clamp(0.0, 1.0));
        let mut shake: ShakeOffset = if control.flag("cam.shake") == Some(false) { ShakeOffset::default() } else { self.shakes.offset() };
        // (ER party S7) + the team attacks' blueprint shakes (kh-app camera.rs)
        if control.flag("cam.shake") != Some(false) && self.team_shake != ShakeOffset::default() {
            shake.rotation += self.team_shake.rotation;
            shake.location += self.team_shake.location;
        }
        let scale = control.f32("cam.shake_scale").unwrap_or(1.0).clamp(0.0, 4.0);
        rotation.pitch += shake.rotation.x * scale;
        rotation.yaw += shake.rotation.y * scale;
        rotation.roll += shake.rotation.z * scale;
        // (ER party S7) a team camera's CameraAnim FOV (+ its shakes' FOV) replaces the field camera's while it has the view
        let mut raw = raw_of(&self.world.frame, position, rotation, fov_vertical(team_fov_h(self.conv_fov.unwrap_or(self.fov_h) /* (CV1) */, self.team_fov, self.team_shake.fov)));
        // (camera-local centimetres: X forward, Y right, Z up)
        raw.position += (raw.forward * shake.location.x + raw.right * shake.location.y + raw.up * shake.location.z) * scale / coords::CM_PER_M;
        // The ease of KH3's own camera blends (ease-out, exponent 2) in the direction of the change.
        let eased = if self.owning { kh_camera::rig::ease_out(self.weight, 2.0) } else { 1.0 - kh_camera::rig::ease_out(1.0 - self.weight, 2.0) };
        self.posted = Some((raw, eased));
        if let Ok(mut s) = SHARE.try_lock() {
            // SAFETY: plain getter.
            *s = Some(Share { view: raw, weight: eased, until: unsafe { GetTickCount64() } + 250, chr_cam: control.flag("cam.chr") != Some(false), listener: control.flag("cam.listener") != Some(false), origin: self.world.frame.origin, listener_fix: control.flag("audio.listener_fix") != Some(false) });
        }
    }

    /// (WALLS cutscene fix 2026-10-23) THE GAME HANDED CONTROL BACK after a cutscene (played or skipped), a load or a relocation:
    /// at the next step in which KH3's camera has the view, a NEW camera behind the leader (whoever leads), KH3's start framing
    /// (behind him, pitch -10, the default distance), at FULL WEIGHT (no blend from wherever Elden Ring's camera was), the lock
    /// taken again. Key `cam.snap_back = 0`: the blend as before.
    pub fn snap_back(&mut self) {
        self.snap_due = true;
    }

    /// (INT) A discontinuity of what the camera follows: its latest step is drawn until the next one.
    pub fn snap(&mut self) {
        self.previous = self.view;
    }

    /// The once-a-second log line.
    pub fn tick(&mut self) {
        let v = self.view;
        erlog!(
            "cam: {} | weight {:.2} | mode {} | KH pos ({:.0}, {:.0}, {:.0}) pitch {:.1} yaw {:.1} distance {:.0} cm, fov {:.0} deg horizontal | {} step(s), collided in {} | shakes played {} (running: {}) | game camera steps {} (ours written in {}) | listener calls {} (given our view {}, not the camera's matrix {}; replaced by the view on screen {}){}",
            if self.owning { "KH3's camera" } else { "Elden Ring's camera" }, self.weight, self.mode, v.position.x, v.position.y, v.position.z, v.rotation.pitch, v.rotation.yaw, v.distance, self.fov_h,
            self.steps, self.collided, self.shakes_played, !self.shakes.is_empty(),
            STEPS.swap(0, Ordering::Relaxed), WRITES.swap(0, Ordering::Relaxed), LISTENER_CALLS.swap(0, Ordering::Relaxed), LISTENER_OURS.swap(0, Ordering::Relaxed), LISTENER_OTHER.swap(0, Ordering::Relaxed), LISTENER_REPLACED.swap(0, Ordering::Relaxed),
            match er_view() { Some(er) => format!(" | Elden Ring's own view: pos ({:.2}, {:.2}, {:.2}) fov {:.4} rad", er.position.x, er.position.y, er.position.z, er.fov), None => String::new() },
        );
        // `cam.diag = 1`: what the camera's sweep (look point -> where the camera wants to be) touches.
        if self.diag {
            let look = self.world.frame.pos_kh_to_er(v.look_point);
            let wanted = self.world.frame.pos_kh_to_er(v.look_point - v.rotation.forward() * 400.0);
            let mut hits = Vec::new();
            havok::cast_sphere(look, wanted - look, 0.185, self.world.filter, false, &mut hits);
            hits.sort_by(|a, b| a.fraction.total_cmp(&b.fraction));
            erlog!("cam: diag: sweep of 4 m from the look point, filter {:#x}: {} contact(s){}", self.world.filter, hits.len(), hits.iter().take(5).map(|h| format!(" | at {:.2} m owner {} filter {:#x} normal ({:.2}, {:.2}, {:.2})", h.fraction * 4.0, h.owner, h.filter, h.normal.x, h.normal.y, h.normal.z)).collect::<String>());
        }
        self.steps = 0;
        self.collided = 0;
        if let Some(trace) = self.trace.as_mut() {
            let _ = trace.flush();
        }
    }
}

impl Drop for KhCam {
    /// Sora is gone (a loading screen, the payload's release): Elden Ring's camera at once.
    fn drop(&mut self) {
        if let Ok(mut s) = SHARE.lock() {
            *s = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kh3_view_maps_to_elden_rings_rows() {
        let frame = Frame::at(Vec3::new(10.0, 2.0, -5.0));
        // Yaw 0, pitch 0: forward = KH +X = Elden Ring +Z; right = KH +Y = +X; up = +Y.
        let v = raw_of(&frame, Vec3::new(100.0, 0.0, 50.0), Rot::new(0.0, 0.0, 0.0), 1.0);
        assert!((v.forward - Vec3::Z).length() < 1e-5 && (v.right - Vec3::X).length() < 1e-5 && (v.up - Vec3::Y).length() < 1e-5);
        assert!((v.position - Vec3::new(10.0, 2.5, -4.0)).length() < 1e-5);
        // The game's rows: right x up = forward, for any rotation (with roll).
        let v = raw_of(&frame, Vec3::ZERO, Rot::new(-25.0, 130.0, 12.0), 1.0);
        assert!((v.right.cross(v.up) - v.forward).length() < 1e-5);
        // Pitch up looks up; yaw 90 looks along Elden Ring +X.
        assert!(raw_of(&frame, Vec3::ZERO, Rot::new(30.0, 0.0, 0.0), 1.0).forward.y > 0.49);
        assert!((raw_of(&frame, Vec3::ZERO, Rot::new(0.0, 90.0, 0.0), 1.0).forward - Vec3::X).length() < 1e-5);
    }

    /// (WALLS cutscene fix) THE SNAP BACK after a cutscene / a load / a relocation, for every leader (Sora, Donald, Goofy: a
    /// leader is a camera target, his place and facing; their capsules differ): the new camera is BEHIND him (its yaw his),
    /// KH3's start pitch (-10), the default distance, looking at him, whatever Elden Ring's camera did meanwhile.
    #[test]
    fn walls_the_snap_back_camera_is_behind_each_leader_at_the_default_framing() {
        for (who, half) in [("sora", 89.0f32), ("donald", 60.0), ("goofy", 100.0)] {
            for yaw in [0.0f32, 90.0, -135.0, 170.0] {
                let target = CameraTarget { position: Vec3::new(-699.0, -1885.0, -958.0), yaw, velocity: Vec3::ZERO, state: TargetState::Idle, collision_half_height: half };
                let view = CameraRig::new(CAMERA_PARAMS, &target).view();
                let dyaw = (view.rotation.yaw - yaw + 540.0).rem_euclid(360.0) - 180.0;
                assert!(dyaw.abs() < 0.5, "{who} yaw {yaw}: camera yaw {}", view.rotation.yaw);
                assert!((view.rotation.pitch - (-10.0)).abs() < 0.5, "{who}: pitch {}", view.rotation.pitch);
                let flat = (view.position - target.position).truncate();
                let back = Vec2::new(yaw.to_radians().cos(), yaw.to_radians().sin());
                assert!(flat.normalize().dot(back) < -0.9, "{who} yaw {yaw}: the camera is behind him");
                assert!((150.0..800.0).contains(&flat.length()), "{who}: distance {}", flat.length());
            }
        }
    }

    #[test]
    fn the_blend_ends_on_each_side_and_kh3s_fov_is_67_7_vertical() {
        let frame = Frame::at(Vec3::ZERO);
        let a = raw_of(&frame, Vec3::ZERO, Rot::new(0.0, 0.0, 0.0), 0.8378);
        let b = raw_of(&frame, Vec3::new(400.0, 0.0, 0.0), Rot::new(-20.0, 90.0, 0.0), 1.2);
        let (start, end, mid) = (a.blend(&b, 0.0), a.blend(&b, 1.0), a.blend(&b, 0.5));
        assert!((start.forward - a.forward).length() < 1e-4 && (end.forward - b.forward).length() < 1e-4);
        assert!((mid.position - Vec3::new(0.0, 0.0, 2.0)).length() < 1e-4 && (mid.right.cross(mid.up) - mid.forward).length() < 1e-4);
        assert!((fov_vertical(FOV_KH3).to_degrees() - 67.7).abs() < 0.1);
    }

    /// (ER party S7) the team lens: the field's FOV unless a team camera has the view; the leader's cut height.
    #[test]
    fn s7_the_team_cameras_lens_and_the_leaders_finish_height() {
        assert_eq!(team_fov_h(100.0, None, 3.0), 100.0);
        assert_eq!(team_fov_h(90.0, Some(60.0), 2.0), 62.0);
        assert_eq!(team_fov_h(100.0, Some(f32::NAN), 0.0), 100.0);
        assert_eq!(team_fov_h(100.0, Some(400.0), 0.0), 170.0);
        assert_eq!(leader_cut_height(102.0, 89.0, 89.0), 102.0);
        assert!((leader_cut_height(102.0, 49.0, 89.0) - 56.16).abs() < 0.01);
        assert_eq!(leader_cut_height(102.0, 0.0, 89.0), 102.0);
    }
}
