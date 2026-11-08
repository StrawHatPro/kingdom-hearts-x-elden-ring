//! (AUDIT-B F-NANO) NANO GEAR'S CAMERA BLUEPRINT `Blueprints/Camera/p_ex001_StateEvent/change_key/
//! BP_TresPlayerStateEvent_camera_bx_key` as data: FINISH_GM's camera, the transformation's, the combo finish's and the Zone
//! Connector's (bytecode read with build/sora_tmp/party/tools/bpdump.js: build/sora_tmp/weapons/AUDIT-B/F-NANO/traces/
//! bp_bx_key_cam.txt; offsets `@n` are its ubergraph's).
//!
//! - `ReceiveAnimNotifyStartBpEvent(SendActor = Sora, AnimSeqName, ..)` (@95): by the sequence of the BpEvent notify (every one
//!   of Nano Gear's clips has one at its start, kh-core `combat::forms::nano::on_notifies` publishes them):
//!   - 8416 (FINISH_GM's first clip, `Gimmick-key_Finish` @5571 -> @4411): "FINISH_CAM" = `Gimmick-key_Finish_cam` (4.9 s:
//!     the three clips 1.4333 + 2.4667 + 1.0) on Sora (`BP_SettingTresCamera(GetLocalTresPlayerPawnBase)`),
//!     `EnableCollision(true)`, `SetPriority(200)`, `BP_AttachTresCameraTarget(false)`, in {0, 4, 2, lock} = a cut,
//!     `StartTresCameraAnim(anim, bLoop false, bTakeOver false, in, bAutoEnd true, false, end {0, 4, 2, lock} = a cut)`,
//!     `BP_SetTresCameraAnimPlayZeroFrame(true)`, `SetTresCameraTargetInfluenceLocationXYZ(false, true)`: X / Y taken once,
//!     Z follows; the yaw once (the CameraAnim default, camera_anim.rs).
//!   - 8401 (StyleChange8, `bx_change_start` @2014): when `BP_TresIsGameEnablePerformanceCamera` and none of the four line
//!     traces (channel 1) from his location + (0, 0, 100) to `TransformLocation(his transform, (342.2, 168.7, 210.3))`, (-342.2,
//!     -168.7, 210.3), (168.7, 342.2, 210.3), (-168.7, -342.2, 210.3) hits: "FINISH_CAM" = `Gimmick-key_Change` (4.9 s),
//!     the same settings, in a cut, autoEnd with a cut, influence XYZ (true, true) (else `RemoteEvent("Cnage_Key_Cancel")`
//!     without a performance camera, and nothing when a trace hits).
//!   - 8415 (the Rocket Fist punch, `bx_combo_finish` @4953): with the performance camera, "COMBO_FINISH_CAM" =
//!     `Gimmick-key_ComboFinish_cut01` (1.667 s), in a cut, autoEnd, end {0.3, 4 ease in-out, 2, lock}.
//!   - 8481 (the Zone Connector's start, `BX_Yoyo_Gate_Start` @1413): "mi_camera_walk_the_dog" = `YoYO_Gate` (3 s), in a cut,
//!     autoEnd, end {0.33, 4, 2, lock}, `SetTresCameraTargetInfluenceRotation(true, ..)`: the yaw follows. 8483
//!     (`BX_Yoyo_Gate_End` @21947): `EndAllSameBPTresCamera` {0, 3, 2} = a cut.
//!   - 8411 .. 8414, 8417 .. 8421: nothing (a debug print only).
//! - `ReceivePlayerDtorState` (@14124): STYLECHANGE 31, STYLEFINISH 33, STYLEFINISH_GM 36 -> @21824 `EndAllSameBPTresCamera`
//!   (a cut) + `ResetTresCameraPrevious`; WEAPONCHANGE 32 -> @10 the same end without the reset.
//! - `ReceivePlayerCtorState` (@6246): DAMAGE 19 -> @13946 `Camera_Anim.EndTresCamera` (a cut).
//! - `BP_TresIsGameEnablePerformanceCamera` (0x1422227D0 -> 0x140EAB300): in STYLECHANGE the config bit (+0xB59 bit 2) unless
//!   the form is 0x99 / 0x9A; in STYLEFINISH .. (33 .. 39) always; else the config +0xB5B (2 = off). The port has no such
//!   option: on ([`NanoCameras::performance`], true).
//! - Not used by any branch: `Gimmick-key_YoYo` (`bx_yoyo_backspin` has no caller), `Gimmick-key_Big_Stump(_sky)`,
//!   `Drill_Finish`, `Gimmick_Drill_tosshin_finish`, `bx_tunede_crusher` (their entries return at once).
//!
//! THE POLL: hosts call [`crate::forms::select`] with Sora: [`NanoShot`] is his state, his clip's sequence and play id this
//! step; a new play of one of the sequences above is the BpEvent's start (every one begins at 0 / 0.0001 s), a state change
//! the dtor / ctor. The CameraAnim's compose and collision pass are camera_anim.rs's.

use glam::Vec3;
use kh_core::{StateId, World};

use crate::camera_anim::{camera_anim, collide, frame_target, sweep_start, AnimView, CameraAnimDef, Transition};
use crate::{CameraTarget, CameraView, Rot};

/// The CameraAnims the blueprint plays (build/gen/rust/party_team_cameras.rs, `tools/party_team_pack.js --cameras`).
pub const FINISH_ANIM: &str = "Gimmick-key_Finish_cam";
pub const CHANGE_ANIM: &str = "Gimmick-key_Change";
pub const COMBO_FINISH_ANIM: &str = "Gimmick-key_ComboFinish_cut01";
pub const GATE_ANIM: &str = "YoYO_Gate";
/// A cut (`SetTresCameraTransitionParams(0, 4, 2, true, ..)` / `{0, 3, 2}`).
pub const CUT: Transition = Transition::new(0.0, 4, 2.0, true);
/// 8415's end (@5270 `SetTresCameraTransitionParams(0.3, 4, 2, true, 1, 0)`).
pub const COMBO_FINISH_END: Transition = Transition::new(0.3, 4, 2.0, true);
/// 8481's end (@1673 `SetTresCameraTransitionParams(0.33, 4, 2, true, 0, 0)`).
pub const GATE_END: Transition = Transition::new(0.33, 4, 2.0, true);
/// The line traces of `bx_change_start` (@2214 .. @3585): from his location + this ...
pub const CHANGE_TRACE_FROM: Vec3 = Vec3::new(0.0, 0.0, 100.0);
/// ... to these points of his transform (x forward, y right, z up).
pub const CHANGE_TRACE_TO: [Vec3; 4] = [Vec3::new(342.214_3, 168.703_3, 210.266_2), Vec3::new(-342.214_3, -168.703_3, 210.266_2), Vec3::new(168.703_3, 342.214_3, 210.266_2), Vec3::new(-168.703_3, -342.214_3, 210.266_2)];

/// What the blueprint hears from Sora this step (the poll).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NanoShot {
    pub state: StateId,
    /// His body clip's sequence ("8416" ...) and its play id.
    pub seq: &'static str,
    pub play_id: u32,
}

impl NanoShot {
    pub fn of(sora: &kh_core::Sora) -> NanoShot {
        let out = sora.output();
        NanoShot { state: out.state, seq: out.anim_ticked.body_samples()[0].0, play_id: out.anim_ticked.play_id }
    }
}

/// The running CameraAnim.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Cam {
    name: &'static str,
    def: &'static CameraAnimDef,
    time: f32,
    end: Transition,
    /// `SetTresCameraTargetInfluenceLocationXYZ(xy, z)`: follow every frame, else taken at the start.
    xy_always: bool,
    z_always: bool,
    /// `SetTresCameraTargetInfluenceRotation(true, ..)`: the yaw follows.
    yaw_always: bool,
    base: Option<(Vec3, f32)>,
}

/// The blueprint's state (one per Sora).
#[derive(Clone, Debug, PartialEq)]
pub struct NanoCameras {
    cam: Option<Cam>,
    /// The view being blended away from, its FOV, the transition, the time.
    out: Option<(AnimView, Transition, f32)>,
    last: Option<AnimView>,
    state: Option<StateId>,
    play: u32,
    /// `BP_TresIsGameEnablePerformanceCamera` (the port: on).
    pub performance: bool,
    /// What the blueprint did (tests, a host's log).
    pub log: Vec<String>,
}

impl Default for NanoCameras {
    fn default() -> NanoCameras {
        NanoCameras { cam: None, out: None, last: None, state: None, play: u32::MAX, performance: true, log: Vec::new() }
    }
}

/// The view the blueprint shows this step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NanoView {
    pub view: AnimView,
    /// The camera's name ("FINISH_CAM" ...), None while it blends back to the field camera.
    pub camera: Option<&'static str>,
    /// This step's view does not continue the last one.
    pub cut: bool,
}

impl NanoCameras {
    /// A camera of the blueprint is on (or blending out).
    pub fn active(&self) -> bool {
        self.cam.is_some() || self.out.is_some()
    }

    /// The running camera's anim and its time.
    pub fn running(&self) -> Option<(&'static str, f32)> {
        self.cam.map(|c| (c.def.name, c.time))
    }

    fn end(&mut self, transition: Transition, why: &str) {
        if let Some(c) = self.cam.take() {
            self.log.push(format!("end {} ({why})", c.name));
            self.out = match (self.last, transition.time > 0.0) {
                (Some(v), true) => Some((v, transition, 0.0)),
                _ => None,
            };
        }
    }

    fn start(&mut self, name: &'static str, anim: &'static str, end: Transition, xy_always: bool, yaw_always: bool) {
        let Some(def) = camera_anim(anim) else {
            self.log.push(format!("{anim} is not in the generated camera tables (node tools/party_team_pack.js --cameras)"));
            return;
        };
        self.log.push(format!("start {name} {anim}"));
        // the in-transition of every camera of the blueprint is a cut
        self.out = None;
        self.cam = Some(Cam { name, def, time: 0.0, end, xy_always, z_always: true, yaw_always, base: None });
    }

    /// One step: `shot` = Sora this step; `target` = his camera target (feet, yaw); `under` = the view below (the field / lock-on
    /// camera), `under_fov` its horizontal FOV; `world` for the change camera's traces and the collision pass. None = nothing
    /// of the blueprint is shown.
    pub fn step(&mut self, dt: f32, shot: &NanoShot, target: &CameraTarget, under: &CameraView, under_fov: f32, world: &dyn World) -> Option<NanoView> {
        // ReceivePlayerDtorState / CtorState
        if self.state != Some(shot.state) {
            match self.state {
                Some(StateId::Stylechange | StateId::Stylefinish | StateId::StylefinishGm) => self.end(CUT, "dtor: EndAllSameBPTresCamera + ResetTresCameraPrevious"),
                Some(StateId::Weaponchange) => self.end(CUT, "dtor WEAPONCHANGE: EndAllSameBPTresCamera"),
                _ => {}
            }
            if shot.state == StateId::Damage {
                self.end(CUT, "ctor DAMAGE: EndTresCamera");
            }
            self.state = Some(shot.state);
        }
        // ReceiveAnimNotifyStartBpEvent: a new play of the sequence (its BpEvent starts at 0)
        let mut cut = false;
        if shot.play_id != self.play {
            self.play = shot.play_id;
            let before = self.cam.map(|c| c.name);
            match shot.seq {
                "8416" => self.start("FINISH_CAM", FINISH_ANIM, CUT, false, false),
                "8401" if self.performance => {
                    let (sin, cos) = target.yaw.to_radians().sin_cos();
                    let from = target.position + CHANGE_TRACE_FROM;
                    let blocked = CHANGE_TRACE_TO.iter().any(|p| world.line_trace(from, target.position + Vec3::new(p.x * cos - p.y * sin, p.x * sin + p.y * cos, p.z)).is_some());
                    if blocked {
                        self.log.push("8401: a line trace hit: no change camera".to_string());
                    } else {
                        self.start("FINISH_CAM", CHANGE_ANIM, CUT, true, false);
                    }
                }
                "8415" if self.performance => self.start("COMBO_FINISH_CAM", COMBO_FINISH_ANIM, COMBO_FINISH_END, true, false),
                "8481" => self.start("mi_camera_walk_the_dog", GATE_ANIM, GATE_END, true, true),
                "8483" => self.end(CUT, "8483: EndAllSameBPTresCamera"),
                _ => {}
            }
            cut = self.cam.map(|c| c.name) != before || self.cam.is_some_and(|c| c.time == 0.0);
        }
        // AdvanceAnim (`BP_SetTresCameraAnimPlayZeroFrame(true)`: the start shows frame 0); bAutoEnd at the anim's end
        if let Some(c) = self.cam.as_mut() {
            if !cut {
                c.time += dt;
            }
            if c.time >= c.def.length {
                c.time = c.def.length;
                let end = c.end;
                self.end(end, "autoEnd");
            }
        }
        let shown = self.cam.as_mut().map(|c| {
            // the base: his location (X / Y once or always, Z always) and his yaw (once or always)
            let base = match c.base {
                Some((p, yaw)) => {
                    let xy = if c.xy_always { (target.position.x, target.position.y) } else { (p.x, p.y) };
                    let z = if c.z_always { target.position.z } else { p.z };
                    (Vec3::new(xy.0, xy.1, z), if c.yaw_always { target.yaw } else { yaw })
                }
                None => (target.position, target.yaw),
            };
            c.base = Some(base);
            let s = c.def.sample(c.time);
            let (sin, cos) = base.1.to_radians().sin_cos();
            let at = base.0 + Vec3::new(s.location.x * cos - s.location.y * sin, s.location.x * sin + s.location.y * cos, s.location.z);
            let v = AnimView { location: at, rotation: Rot { pitch: s.rotation.pitch, yaw: s.rotation.yaw + base.1, roll: s.rotation.roll }.normalized(), fov: s.fov };
            // EnableCollision(true): camera_anim.rs's pass (the outside-wall framing, then the sweep from the look point)
            let pivot = base.0;
            let v = match frame_target(&v, pivot, world) {
                Some(f) if f.distance > 0.0 => {
                    let start = f.location + f.rotation.forward() * f.distance;
                    AnimView { location: collide(start, f.location, world), rotation: f.rotation, ..v }
                }
                _ => AnimView { location: collide(sweep_start(&v, pivot), v.location, world), ..v },
            };
            (v, c.name)
        });
        let below = AnimView { location: under.position, rotation: under.rotation, fov: under_fov };
        let result = match (shown, self.out.as_mut()) {
            (Some((v, name)), _) => Some(NanoView { view: v, camera: Some(name), cut }),
            (None, Some((from, t, elapsed))) => {
                *elapsed += dt;
                let a = t.alpha(*elapsed);
                let v = from.blend(&below, a);
                if a >= 1.0 {
                    self.out = None;
                    None
                } else {
                    Some(NanoView { view: v, camera: None, cut: false })
                }
            }
            (None, None) => None,
        };
        self.last = result.map(|r| r.view);
        result
    }
}

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

    /// A wall everywhere for the line traces only.
    struct Walled;
    impl World for Walled {
        fn sweep_capsule(&self, _: Vec3, _: Vec3, _: f32, _: f32) -> Option<Hit> {
            None
        }
        fn line_trace(&self, a: Vec3, _: Vec3) -> Option<Hit> {
            Some(Hit { time: 0.5, position: a, normal: Vec3::X, impact_normal: Vec3::X, start_penetrating: false, penetration_depth: 0.0, component: 0 })
        }
    }

    fn target(x: f32, z: f32, yaw: f32) -> CameraTarget {
        CameraTarget { position: Vec3::new(x, 0.0, z), yaw, velocity: Vec3::ZERO, state: TargetState::Idle, collision_half_height: 89.0 }
    }

    fn under() -> CameraView {
        CameraView { position: Vec3::new(-400.0, 0.0, 200.0), rotation: Rot::new(-10.0, 0.0, 0.0), rotation_target: Rot::new(-10.0, 0.0, 0.0), look_point: Vec3::ZERO, distance: 400.0, auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided: false }
    }

    fn shot(state: StateId, seq: &'static str, play_id: u32) -> NanoShot {
        NanoShot { state, seq, play_id }
    }

    #[test]
    fn the_finish_camera_cuts_in_at_8416_and_ends_with_the_finish() {
        let Some(def) = camera_anim(FINISH_ANIM) else {
            eprintln!("SKIPPED: no generated camera tables");
            return;
        };
        assert!((def.length - (1.433_333 + 2.466_667 + 1.0)).abs() < 0.01, "Gimmick-key_Finish_cam spans the three clips: {}", def.length);
        let mut c = NanoCameras::default();
        let dt = 1.0 / 60.0;
        assert!(c.step(dt, &shot(StateId::Idle, "8301", 1), &target(0.0, 0.0, 30.0), &under(), 100.0, &Open).is_none());
        let first = c.step(dt, &shot(StateId::Stylefinish, "8416", 2), &target(0.0, 0.0, 30.0), &under(), 100.0, &Open).expect("FINISH_CAM");
        assert!(first.cut && first.camera == Some("FINISH_CAM"));
        // frame 0 of the anim, turned by his yaw, on his feet
        let s = def.sample(0.0);
        let (sin, cos) = 30f32.to_radians().sin_cos();
        let want = Vec3::new(s.location.x * cos - s.location.y * sin, s.location.x * sin + s.location.y * cos, s.location.z);
        assert!((first.view.location - want).length() < 1.0e-2, "{:?} vs {want:?}", first.view.location);
        // X / Y taken once, Z follows; the yaw once
        let moved = c.step(dt, &shot(StateId::Stylefinish, "8416", 2), &target(500.0, 80.0, 90.0), &under(), 100.0, &Open).unwrap();
        let s = def.sample(dt);
        let want = Vec3::new(s.location.x * cos - s.location.y * sin, s.location.x * sin + s.location.y * cos, s.location.z + 80.0);
        assert!((moved.view.location - want).length() < 1.0e-2, "{:?} vs {want:?}", moved.view.location);
        // runs through 8417 / 8418 (new plays that start nothing)
        for (seq, play) in [("8417", 3), ("8418", 4)] {
            let v = c.step(dt, &shot(StateId::Stylefinish, seq, play), &target(0.0, 0.0, 30.0), &under(), 100.0, &Open).unwrap();
            assert_eq!(v.camera, Some("FINISH_CAM"), "{seq}");
        }
        // the finish's state ends: a cut back
        assert!(c.step(dt, &shot(StateId::Idle, "1001", 5), &target(0.0, 0.0, 30.0), &under(), 100.0, &Open).is_none());
        assert!(!c.active());
    }

    #[test]
    fn the_change_camera_needs_room_and_the_combo_finish_blends_out() {
        if camera_anim(CHANGE_ANIM).is_none() {
            return;
        }
        let dt = 1.0 / 60.0;
        let mut c = NanoCameras::default();
        assert!(c.step(dt, &shot(StateId::Stylechange, "8401", 1), &target(0.0, 0.0, 0.0), &under(), 100.0, &Walled).is_none(), "a trace hit: no camera");
        let mut c = NanoCameras::default();
        assert_eq!(c.step(dt, &shot(StateId::Stylechange, "8401", 1), &target(0.0, 0.0, 0.0), &under(), 100.0, &Open).and_then(|v| v.camera), Some("FINISH_CAM"));
        assert_eq!(c.running().map(|r| r.0), Some(CHANGE_ANIM));
        // the combo finish: autoEnd with 0.3 s ease in-out back to the view below
        let mut c = NanoCameras::default();
        c.step(dt, &shot(StateId::ComboGm, "8415", 7), &target(0.0, 0.0, 0.0), &under(), 100.0, &Open).expect("COMBO_FINISH_CAM");
        let len = camera_anim(COMBO_FINISH_ANIM).map_or(0.0, |d| d.length);
        let mut blended = 0;
        for _ in 0..((len + 0.6) / dt) as usize {
            if let Some(v) = c.step(dt, &shot(StateId::ComboGm, "8415", 7), &target(0.0, 0.0, 0.0), &under(), 100.0, &Open) {
                blended += usize::from(v.camera.is_none());
            }
        }
        assert!((16..=19).contains(&blended), "0.3 s of blend: {blended} steps");
        assert!(!c.active());
        // damage ends it at once
        let mut c = NanoCameras::default();
        c.step(dt, &shot(StateId::Stylefinish, "8416", 9), &target(0.0, 0.0, 0.0), &under(), 100.0, &Open).expect("FINISH_CAM");
        assert!(c.step(dt, &shot(StateId::Damage, "8416", 9), &target(0.0, 0.0, 0.0), &under(), 100.0, &Open).is_none());
    }
}
