//! (AUDIT-B F-HAMMER) THE HAMMER FORMS' CAMERA BLUEPRINTS, as data: Favorite Deputy's ("toy key": Hyper Hammer -> Drill
//! Punch) `Blueprints/Camera/p_ex001_StateEvent/change_key/BP_TresPlayerStateEvent_camera_toy_key` and Classic Tone's
//! ("timeless key": Boom Hammer -> Clock Drill) `.../change_key/BP_TresPlayerStateEventCamera_Timlesskey`. Bytecode read in
//! full 2026-11-03 with build/sora_tmp/party/tools/bpdump.js (build/sora_tmp/weapons/AUDIT-B/F-HAMMER/traces/
//! bp_toy_key_cam.txt, bp_timlesskey_cam.txt); `@n` = the ubergraph's bytecode offset. Both blueprints are in Sora's load
//! list (Load/Player/TresPlayerPawnLoad_ex001): they run with every keyblade; the toy key one has no weapon test, so it
//! also films the clips Classic Tone's forms share (3416 / 3417 / 6419 / 6424 / 6483). The clips' BpEvents
//! (`EventKind::AnimBpEvent`, kh-core `combat::forms::hammer::CAMERA_SEQUENCES`) come from kh-core's generic sender
//! `states::bp_event::emit` and, for Drill Dive's / Noise Flux's M_6483, the shotlock's `rush::on_notify`.
//!
//! Every camera below is `BP_SpawnTresCameraAnim(name)` + `BP_SettingTresCamera(GetLocalTresPlayerPawnBase)` (Sora) +
//! `EnableCollision(true)` + `SetPriority(200)` + `StartTresCameraAnim(anim, bLoop false, bTakeover, in, bAutoEnd,
//! bEndTakeover, end)` + `BP_SetTresCameraAnimPlayZeroFrame(true)`; "yaw always" = `SetTresCameraTargetInfluenceRotation(
//! bAlways true, 0, bYaw true, ..)` (else the yaw is taken once, the CameraAnim default). Every blend block is
//! `SetTresCameraTransitionParams(time, 4 ease in-out, 2, bLockOutgoing true, ..)`; "cut" = time 0. perf =
//! `BP_TresIsGameEnablePerformanceCamera` (form_bp.rs `PerfCamera`). A trace = `BP_TresLineTraceByChannel(Sora's location +
//! (0, 0, 100) -> TransformLocation(Sora's transform, P), channel 1)`; "hit" = it returned true.
//!
//! # BP_TresPlayerStateEvent_camera_toy_key
//! `ReceiveAnimNotifyStartBpEvent` (@12647): `self.name = AnimSeqName` (before the sender test), Sora only, then a sequence:
//! the name chain, `Drill_Finish` (@14188), `Drill_combo_finish` (@14306), `Drill_tosshin_finish` (@14481).
//! - 3401 (StyleChange3, Hyper Hammer) @6287: no perf -> `RemoteEvent("Cnage_Key_Cancel")` (@8536). T1 (240.99672,
//!   39.875473, 28.099335) hit -> none. T2 (1000, 45, 150) hit: T3 (-98, 5.4, 126) hit -> start04, clear -> start03; T2 clear:
//!   T4 (-98, 5.4, 126) hit -> start04, clear -> toy_key_hammer_start. start / start04 (@7609): "none", BP_AttachTresCamera
//!   Target(true), in cut, no autoEnd, end cut, yaw always (@5781). start03 (@8593): "none", attached, bTakeover, in 0.33,
//!   no autoEnd, end cut, yaw once. Both -> `self.Camera_Anim`.
//! - 3416 (Giant Stamp) @5362: perf: `ResetTresCameraNormal(true)`; T (323, -106, 625) hit -> big_stump02, clear ->
//!   big_stump. 3417 (@5865): perf: ResetTresCameraNormal, big_stump_sky. "Big_Stump" (@4679): in cut, autoEnd, end 0.4,
//!   yaw always; -> Camera_Anim.
//! - 6401 (StyleChange6, Drill Punch) @9156: no perf -> RemoteEvent. (-442.9769, 30.424683, 85.89545), (22.385437,
//!   316.7919, 37.75528), (347.89185, 73.52869, 142.20088): any hit -> none. ResetTresCameraNormal(true). T5 (900, 200, 150)
//!   hit: T (-242, 4.8, 255) hit -> drill_start03, clear -> drill_start02; T5 clear: the same point hit -> drill_start03,
//!   clear -> drill_start01. "Drill": 01 / 03 in cut, autoEnd, end cut; 02 bTakeover, in 0.33, autoEnd, end 0.33; all yaw
//!   always (@1626); NOT Camera_Anim.
//! - 6419 (`Drill_combo_finish`, the combo finish) @6157: perf: `TresWaitCycle(1)` -> @15: "none"
//!   toy_key_drill_combo_finish, bTakeover, in 1.0, autoEnd, bEndTakeover, end 0.3; yaw once; -> Camera_Anim.
//! - 6424 (`Drill_combo_finish`) @5976: perf: the ACTIVE camera's `SetFieldOfView(130, 0.2)`.
//! - 6421 (`Drill_Finish`, FINISH_DR's M_6421, no perf test) @13725: T (286, -50, 42) hit -> toy_key_drill_finish01_NG,
//!   clear -> toy_key_drill_finish01 (@1901): "Drill", in cut, no autoEnd, end cut, yaw always; -> Camera_Anim and the
//!   variable `ReturnValue3`.
//! - 6423 (`Drill_Finish`, M_6423) @12227: T (-575, -315, 395) hit, or T (946, 72, 1148) hit -> @2498 `End Tres Camera
//!   Normal(ReturnValue3, take over true, 0.5, 4, lock, ..)` (FunctionLibrary_Camera: EndTresCamera(true, {0.5, ease
//!   in-out, 2}); None -> EndAllBPTresCamera). Both clear -> @2557 ReturnValue3.EndTresCamera(false, cut) and "Drill"
//!   toy_key_drill_finish02, in cut, autoEnd, end cut, yaw always; -> Camera_Anim.
//! - 6483 (`Drill_tosshin_finish`, Drill Dash) @5454: perf: "drill_tosshin_finish" toy_key_drill_tosshin_finish01, attached,
//!   in cut, autoEnd, end cut, yaw once; -> Camera_Anim; `WaitPlayEnd` -> @567 `ResetTresCameraPrevious`.
//! `ReceiveAnimNotifyEndBpEvent` (@13065, Sora only): 3401 -> if Camera_Anim is valid: `EndTresCamera(false, {0, 3, 2})`
//! (a cut) and `GetTresCameraNormal().ResetCamera(true)`; then (every end) `Drill_cmbo_finish_end` (@14424): if
//! `self.name` (the LAST START's name) == 6424 and perf: the active camera's `SetFieldOfView(GetDefaultFieldOfView(), 1)`.
//! `ReceivePlayerCtorState` (@14538): DAMAGE 19 -> `Delay(0.2)` with no continuation (nothing). `ReceivePlayerDtorState`
//! (@21631): STYLECHANGE 31, WEAPONCHANGE 32, STYLEFINISH 33, COMBO_HM 76, COMBO_DR 79 -> @28663 `EndAllSameBPTresCamera(
//! false, {0, 3, 2})` (a cut).
//!
//! # BP_TresPlayerStateEventCamera_Timlesskey
//! `ReceiveAnimNotifyStartBpEvent` (@909): Sora only and `BP_GetEquipWeaponID() == 12` (WEP_KEYBLADE10 = Classic Tone,
//! @1067 .. @1590), then:
//! - L421 (FINISH_DR's Clock, no perf test) @3983 -> `Timeless_Finish_Cam` (@4692): Timeless_Drill_Finish_Cam, Blend 0.33.
//! - K401 (StyleChange20, Boom Hammer) @82: no perf -> RemoteEvent; `Timeless_Hunmer_Change` (@11911): T (682.8938,
//!   39.177628, 141.47034) hit -> none; T (603.8623, -335.187, 78.39303) hit -> none; else Timeless_Hunmer_Change_Cam,
//!   Blend 0.
//! - L401 (StyleChange21, Clock Drill) @837: no perf -> RemoteEvent; `Timeless_Drill_Change` (@12332): T (1056.373,
//!   172.34149, 214.9997) hit -> none; T (97.24945, -618.88904, 168.98631) hit -> none; else Timeless_Drill_Change_Cam,
//!   Blend 0.
//! All (@4630 -> @211): "LemyKey_Finish_Cam", in {Blend}, autoEnd, end cut, yaw always (@680) -> `camera_anim0`, On = true;
//! `WaitPlayEnd` -> @27: if On: On = false, ResetTresCameraPrevious. Ctor (@4739): DAMAGE 19 -> @11755 On = false,
//! camera_anim0.EndTresCamera(false, cut). Dtor (@13211): 31 / 33 / 34 -> @20239 EndAllSameBPTresCamera({0, 3, 2}), On =
//! false, ResetTresCameraPrevious; 32 -> @20395 the same end, On = false.
//!
//! # Natively (decompiled C, re/decomp; read 2026-11-03): no camera of their own
//! StyleFinish Begin 0x1413CBF70 case 0x9E (FINISH_DR) registers 0x1404EFA80 (= the state's vt+0x4A0, slot 148
//! 0x1413DF1F0) only in form 0x84; StyleFinishDR Begin 0x1413CC300 = that Begin; Reset 0x1413D0DC0 (+0x2E0 / +0x2E8 / +0x2EC
//! / +0x164 = 0); Tick 0x1413D4990 (the Clock spawned by 0x1405D9130, Sora put at it by SetActorLocation 0x143EAC3D0, M_L421
//! on both, sub-steps 0 / 2 / 5 / 6 / 7 / 8); End 0x1413CF0C0 (cleanup 0x1413CFA10 + vt+0x378); slot 148 (M_6421 / slot 127
//! facing / the catch on X_link_skt / M_6423). None of them calls a camera (no vt+0x788 spawn, no SetFieldOfView, no
//! attack-camera request) nor the world slow 0x14140E500 (re/calls.tsv: its 59 callers are other states). FINISH_DR's
//! cameras are the blueprints' (6421 / 6423 / L421) above.
//!
//! # Port notes
//! - The CameraAnim compose, its collision pass and the blends are camera_anim.rs's / form_bp.rs's (bTakeover at the start
//!   only seeds what the anim overwrites; bEndTakeover hands the view to the field camera: [`HammerOutput::takeover`];
//!   PlayZeroFrame: the first tick after the start uses dt 0).
//! - [I] `BP_AttachTresCameraTarget(true)` (vt+0x880, not traced): as camera_anim.rs, the composition is the same (the base
//!   follows Sora's location every frame; the yaw once / always as set).
//! - [I] 6424's FOV applies to the field camera (the active camera when none of these is up); on a running CameraAnim the
//!   anim's own FOV track is kept. `TresWaitCycle(1)`: the next step. `RemoteEvent("Cnage_Key_Cancel")` goes to the level
//!   script (no listener in the port's levels): counted in [`HammerOutput::cancel_events`].
//! - `WaitPlayEnd`'s continuation runs when the anim reaches its end (not when a dtor ended the camera first).

use crate::camera_anim::{camera_anim, collide, frame_target, sweep_start, AnimView, CameraAnimDef, Transition};
use crate::form_bp::PerfCamera;
use crate::Rot;
use glam::Vec3;
use kh_core::host::EventKind;
use kh_core::StateId;

/// The two blueprints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bp {
    /// BP_TresPlayerStateEvent_camera_toy_key (Favorite Deputy).
    ToyKey,
    /// BP_TresPlayerStateEventCamera_Timlesskey (Classic Tone).
    Timeless,
}

/// `ETresItemDefWeapon` 12 (WEP_KEYBLADE10): Classic Tone's item key, for hosts (`combat::form::equipped_keyblade`).
pub const CLASSIC_TONE: &str = "WEP_KEYBLADE_SO_11";

/// What the blueprints read, every step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HammerFrame {
    /// Sora's actor location (feet) and yaw.
    pub sora: (Vec3, f32),
    /// The view under these cameras now (blends from / to it); its FOV is the field camera's default.
    pub field: AnimView,
    /// The pawn's state at the end of the step.
    pub state: StateId,
    /// `BP_GetEquipWeaponID() == 12`: Classic Tone is held ([`CLASSIC_TONE`]).
    pub classic_tone: bool,
    pub perf: PerfCamera,
}

/// One step's result.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct HammerOutput {
    /// The view to draw (None = the one under).
    pub view: Option<AnimView>,
    /// ResetTresCameraNormal(true) / ResetCamera(true) / ResetTresCameraPrevious: the field camera behind Sora at once.
    pub reset_field_camera: bool,
    /// bEndTakeover / `End Tres Camera Normal(take over true)`: the field camera takes over this view.
    pub takeover: Option<AnimView>,
    /// `RemoteEvent("Cnage_Key_Cancel")` sent this step (no performance camera at a transformation).
    pub cancel_events: u32,
}

/// The toy key's anims (build/gen/rust/party_team_cameras.rs, `tools/party_team_pack.js --cameras`).
pub const HAMMER_START: &str = "toy_key_hammer_start";
pub const HAMMER_START03: &str = "toy_key_hammer_start03";
pub const HAMMER_START04: &str = "toy_key_hammer_start04";
pub const BIG_STUMP: &str = "toy_key_hammer_big_stump";
pub const BIG_STUMP02: &str = "toy_key_hammer_big_stump02";
pub const BIG_STUMP_SKY: &str = "toy_key_hammer_big_stump_sky";
pub const DRILL_START01: &str = "toy_key_drill_start01";
pub const DRILL_START02: &str = "toy_key_drill_start02";
pub const DRILL_START03: &str = "toy_key_drill_start03";
pub const DRILL_COMBO_FINISH: &str = "toy_key_drill_combo_finish";
pub const DRILL_FINISH01: &str = "toy_key_drill_finish01";
pub const DRILL_FINISH01_NG: &str = "toy_key_drill_finish01_NG";
pub const DRILL_FINISH02: &str = "toy_key_drill_finish02";
pub const DRILL_TOSSHIN: &str = "toy_key_drill_tosshin_finish01";
/// The timeless key's.
pub const TIMELESS_HAMMER_CHANGE: &str = "Timeless_Hunmer_Change_Cam";
pub const TIMELESS_DRILL_CHANGE: &str = "Timeless_Drill_Change_Cam";
pub const TIMELESS_DRILL_FINISH: &str = "Timeless_Drill_Finish_Cam";

/// The toy key's line traces (Sora's local points; from his location + (0, 0, 100)).
pub const T_3401_ROOM: Vec3 = Vec3::new(240.99672, 39.875473, 28.099335);
pub const T_3401_FAR: Vec3 = Vec3::new(1000.0, 45.0, 150.0);
pub const T_3401_BACK: Vec3 = Vec3::new(-98.0, 5.4, 126.0);
pub const T_3416: Vec3 = Vec3::new(323.0, -106.0, 625.0);
pub const T_6401_ROOM: [Vec3; 3] = [Vec3::new(-442.9769, 30.424683, 85.89545), Vec3::new(22.385437, 316.7919, 37.75528), Vec3::new(347.89185, 73.52869, 142.20088)];
pub const T_6401_FAR: Vec3 = Vec3::new(900.0, 200.0, 150.0);
pub const T_6401_BACK: Vec3 = Vec3::new(-242.0, 4.8, 255.0);
pub const T_6421: Vec3 = Vec3::new(286.0, -50.0, 42.0);
pub const T_6423: [Vec3; 2] = [Vec3::new(-575.0, -315.0, 395.0), Vec3::new(946.0, 72.0, 1148.0)];
/// The timeless key's.
pub const T_K401: [Vec3; 2] = [Vec3::new(682.8938, 39.177628, 141.47034), Vec3::new(603.8623, -335.187, 78.39303)];
pub const T_L401: [Vec3; 2] = [Vec3::new(1056.373, 172.34149, 214.9997), Vec3::new(97.24945, -618.88904, 168.98631)];

/// 6424's `SetFieldOfView(130, 0.2)` and `SetFieldOfView(GetDefaultFieldOfView(), 1)` at the end.
pub const DRILL_FOV: f32 = 130.0;
pub const DRILL_FOV_IN: f32 = 0.2;
pub const DRILL_FOV_OUT: f32 = 1.0;

/// A blend block {time, ease in-out (4), exp 2, bLockOutgoing true}.
const fn eased(time: f32) -> Transition {
    Transition::new(time, 4, 2.0, true)
}

/// EndAllSameBPTresCamera / EndTresCamera's {0, ease out (3), 2}: a cut.
const CUT_OUT: Transition = Transition::new(0.0, 3, 2.0, false);

/// What a `WaitPlayEnd` resumes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OnEnd {
    /// toy key @567: ResetTresCameraPrevious.
    ResetPrevious,
    /// timeless @27: if On: On = false, ResetTresCameraPrevious.
    TimelessEnd,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Cam {
    /// A handle for the blueprints' object variables (Camera_Anim, ReturnValue3, camera_anim0).
    id: u32,
    name: &'static str,
    bp: Bp,
    def: &'static CameraAnimDef,
    time: f32,
    auto_end: bool,
    end: Transition,
    end_takeover: bool,
    zero_frame: bool,
    yaw_always: bool,
    on_end: Option<OnEnd>,
    base_yaw: Option<f32>,
}

impl Cam {
    /// World = Anim x Base: the base is Sora's location every frame, his yaw once or always.
    fn view(&mut self, sora: (Vec3, f32)) -> AnimView {
        let (loc, yaw) = sora;
        if self.base_yaw.is_none() || self.yaw_always {
            self.base_yaw = Some(yaw);
        }
        let by = self.base_yaw.unwrap_or(yaw);
        let s = self.def.sample(self.time);
        let (sn, cs) = by.to_radians().sin_cos();
        let p = s.location;
        AnimView { location: loc + Vec3::new(p.x * cs - p.y * sn, p.x * sn + p.y * cs, p.z), rotation: Rot { pitch: s.rotation.pitch, yaw: s.rotation.yaw + by, roll: s.rotation.roll }.normalized(), fov: s.fov }
    }
}

/// One branch's StartTresCameraAnim and setters.
#[derive(Clone, Copy, Debug)]
struct Start {
    name: &'static str,
    anim: &'static str,
    blend_in: Transition,
    end: Transition,
    auto_end: bool,
    end_takeover: bool,
    yaw_always: bool,
    on_end: Option<OnEnd>,
}

impl Start {
    /// The common shape: in / end blends, autoEnd, yaw always; no takeover at the end, no continuation.
    const fn new(name: &'static str, anim: &'static str, blend_in: f32, end: f32, auto_end: bool, yaw_always: bool) -> Start {
        Start { name, anim, blend_in: eased(blend_in), end: eased(end), auto_end, end_takeover: false, yaw_always, on_end: None }
    }
}

/// `SetFieldOfView`'s interpolation of the active camera: (from, to, time, elapsed); `release` = it ends at its target.
#[derive(Clone, Copy, Debug, PartialEq)]
struct FovRamp {
    from: f32,
    to: f32,
    time: f32,
    elapsed: f32,
    release: bool,
}

impl FovRamp {
    /// SetFieldOfView 0x1405D6F40 -> FUN_141194190(type 5): the ease-out 1 - (1 - x)^2 (camera_anim.rs `Ramp`).
    fn value(&self) -> f32 {
        if self.time <= 0.0 {
            self.to
        } else {
            self.from + (self.to - self.from) * crate::lockon::interp_curve(5, self.elapsed / self.time)
        }
    }
}

/// The two blueprints' cameras.
#[derive(Clone, Debug, Default)]
pub struct HammerCameras {
    cams: Vec<Cam>,
    next_id: u32,
    blend: Option<(AnimView, Transition, f32)>,
    last: Option<AnimView>,
    state: Option<StateId>,
    /// toy key: `self.name` (the last BpEvent start's sequence), `self.Camera_Anim`, `ReturnValue3` (6421's camera).
    name: Option<&'static str>,
    camera_anim: Option<u32>,
    finish01: Option<u32>,
    /// toy key @6210: `TresWaitCycle(1)` -> @15 pending.
    wait_cycle: bool,
    /// timeless: `On`, `camera_anim0`.
    on: bool,
    camera_anim0: Option<u32>,
    fov: Option<FovRamp>,
    pending: HammerOutput,
    /// A readable log (tests, traces).
    pub log: Vec<String>,
}

impl HammerCameras {
    pub fn new() -> HammerCameras {
        HammerCameras::default()
    }

    /// A camera of the blueprints is on, blending, or the FOV of 6424 is held.
    pub fn active(&self) -> bool {
        !self.cams.is_empty() || self.blend.is_some() || self.fov.is_some()
    }

    /// The running cameras' anims, oldest first.
    pub fn anims(&self) -> Vec<&'static str> {
        self.cams.iter().map(|c| c.def.name).collect()
    }

    /// The running cameras' spawn names, oldest first.
    pub fn cameras(&self) -> Vec<&'static str> {
        self.cams.iter().map(|c| c.name).collect()
    }

    fn running(&self, id: Option<u32>) -> bool {
        id.is_some_and(|id| self.cams.iter().any(|c| c.id == id))
    }

    fn switch(&mut self, t: Transition) {
        self.blend = match (self.last, t.time > 0.0) {
            (Some(from), true) => Some((from, t, 0.0)),
            _ => None,
        };
    }

    fn start(&mut self, bp: Bp, s: Start) -> Option<u32> {
        let Some(def) = camera_anim(s.anim) else {
            self.log.push(format!("{} is not in the generated camera tables (node tools/party_team_pack.js --cameras)", s.anim));
            return None;
        };
        self.log.push(format!("start {} {}", s.name, s.anim));
        self.switch(s.blend_in);
        self.next_id += 1;
        let id = self.next_id;
        self.cams.push(Cam { id, name: s.name, bp, def, time: 0.0, auto_end: s.auto_end, end: s.end, end_takeover: s.end_takeover, zero_frame: true, yaw_always: s.yaw_always, on_end: s.on_end, base_yaw: None });
        Some(id)
    }

    fn end_where(&mut self, t: Transition, takeover: bool, why: &str, pred: impl Fn(&Cam) -> bool) {
        let top = self.cams.last().map(|c| c.id);
        let before = self.cams.len();
        self.cams.retain(|c| !pred(c));
        if self.cams.len() != before {
            self.log.push(format!("end {} camera(s) ({why})", before - self.cams.len()));
            if self.cams.last().map(|c| c.id) != top {
                self.switch(t);
                if takeover && self.cams.is_empty() {
                    self.pending.takeover = self.last;
                }
            }
        }
    }

    fn end_id(&mut self, id: Option<u32>, t: Transition, takeover: bool, why: &str) {
        if let Some(id) = id {
            self.end_where(t, takeover, why, |c| c.id == id);
        }
    }

    fn end_bp(&mut self, bp: Bp, why: &str) {
        self.end_where(CUT_OUT, false, why, |c| c.bp == bp);
    }

    /// `BP_TresLineTraceByChannel(his location + (0, 0, 100), TransformLocation(his transform, local))` hit (no level: clear).
    fn hit(world: Option<&dyn kh_core::World>, sora: (Vec3, f32), local: Vec3) -> bool {
        let Some(w) = world else { return false };
        let (loc, yaw) = sora;
        let (s, c) = yaw.to_radians().sin_cos();
        let to = loc + Vec3::new(local.x * c - local.y * s, local.x * s + local.y * c, local.z);
        w.line_trace(loc + Vec3::Z * 100.0, to).is_some()
    }

    fn perf(&self, frame: &HammerFrame) -> bool {
        frame.perf.enabled(self.state.unwrap_or(frame.state))
    }

    fn cancel(&mut self, seq: &str) {
        self.pending.cancel_events += 1;
        self.log.push(format!("{seq}: no performance camera: RemoteEvent Cnage_Key_Cancel"));
    }

    fn on_event(&mut self, ev: &EventKind, frame: &HammerFrame, world: Option<&dyn kh_core::World>) {
        match *ev {
            EventKind::StateChange { from, to } => {
                self.dtor(from);
                self.ctor(to);
                self.state = Some(to);
            }
            EventKind::AnimBpEvent { seq, begin: true, .. } => {
                self.toy_start(seq, frame, world);
                if frame.classic_tone {
                    self.timeless_start(seq, frame, world);
                }
            }
            EventKind::AnimBpEvent { seq, begin: false, .. } => self.toy_end(seq, frame),
            _ => {}
        }
    }

    fn ctor(&mut self, to: StateId) {
        if to == StateId::Damage {
            // toy key @14538 -> @21554: Delay(0.2) with no continuation. timeless @4739 -> @11755.
            self.on = false;
            let id = self.camera_anim0;
            self.end_id(id, CUT_OUT, false, "timeless ctor DAMAGE: camera_anim0.EndTresCamera");
        }
    }

    fn dtor(&mut self, from: StateId) {
        // toy key @21631: 31 32 33 76 79 -> @28663.
        if matches!(from as i32, 31 | 32 | 33 | 76 | 79) {
            self.end_bp(Bp::ToyKey, "toy dtor: EndAllSameBPTresCamera");
        }
        // timeless @13211: 31 33 34 -> @20239 (+ ResetTresCameraPrevious), 32 -> @20395.
        match from as i32 {
            31 | 33 | 34 => {
                self.end_bp(Bp::Timeless, "timeless dtor: EndAllSameBPTresCamera");
                self.on = false;
                self.pending.reset_field_camera = true;
            }
            32 => {
                self.end_bp(Bp::Timeless, "timeless dtor WEAPONCHANGE: EndAllSameBPTresCamera");
                self.on = false;
            }
            _ => {}
        }
    }

    /// toy key `ReceiveAnimNotifyStartBpEvent` (@12647).
    fn toy_start(&mut self, seq: &'static str, frame: &HammerFrame, world: Option<&dyn kh_core::World>) {
        self.name = Some(seq);
        let perf = self.perf(frame);
        let sora = frame.sora;
        let hit = |p: Vec3| Self::hit(world, sora, p);
        match seq {
            // @6287
            "3401" if !perf => self.cancel(seq),
            "3401" if hit(T_3401_ROOM) => self.log.push("3401: no room (T1 hit): no camera".to_string()),
            "3401" => {
                let start03 = hit(T_3401_FAR) && !hit(T_3401_BACK);
                let s = if start03 {
                    // @8593: bTakeover, in 0.33, yaw once
                    Start::new("none", HAMMER_START03, 0.33, 0.0, false, false)
                } else {
                    // @7609 (start04 when the back point is blocked)
                    let anim = if hit(T_3401_BACK) { HAMMER_START04 } else { HAMMER_START };
                    Start::new("none", anim, 0.0, 0.0, false, true)
                };
                self.camera_anim = self.start(Bp::ToyKey, s);
            }
            // @5362 / @5865
            "3416" | "3417" if perf => {
                self.pending.reset_field_camera = true;
                let anim = match seq {
                    "3417" => BIG_STUMP_SKY,
                    _ if hit(T_3416) => BIG_STUMP02,
                    _ => BIG_STUMP,
                };
                // @4679
                self.camera_anim = self.start(Bp::ToyKey, Start::new("Big_Stump", anim, 0.0, 0.4, true, true));
            }
            // @9156
            "6401" if !perf => self.cancel(seq),
            "6401" if T_6401_ROOM.iter().any(|p| hit(*p)) => self.log.push("6401: no room: no camera".to_string()),
            "6401" => {
                self.pending.reset_field_camera = true;
                let s = match (hit(T_6401_FAR), hit(T_6401_BACK)) {
                    (_, true) => Start::new("Drill", DRILL_START03, 0.0, 0.0, true, true),
                    (true, false) => Start::new("Drill", DRILL_START02, 0.33, 0.33, true, true),
                    (false, false) => Start::new("Drill", DRILL_START01, 0.0, 0.0, true, true),
                };
                self.start(Bp::ToyKey, s);
            }
            _ => {}
        }
        // the sequence's other pins: Drill_Finish (@14188), Drill_combo_finish (@14306), Drill_tosshin_finish (@14481)
        match seq {
            "6421" => {
                // @13725 -> @1901
                let anim = if hit(T_6421) { DRILL_FINISH01_NG } else { DRILL_FINISH01 };
                let id = self.start(Bp::ToyKey, Start::new("Drill", anim, 0.0, 0.0, false, true));
                self.finish01 = id;
                self.camera_anim = id;
            }
            "6423" => {
                if T_6423.iter().any(|p| hit(*p)) {
                    // @2498 End Tres Camera Normal(ReturnValue3, take over, 0.5, ease in-out); None -> EndAllBPTresCamera
                    match self.finish01 {
                        Some(_) => {
                            let id = self.finish01;
                            self.end_id(id, eased(0.5), true, "6423: End Tres Camera Normal (no room for finish02)");
                        }
                        None => self.end_where(eased(0.5), true, "6423: EndAllBPTresCamera", |_| true),
                    }
                } else {
                    // @2557
                    let id = self.finish01;
                    self.end_id(id, CUT_OUT, false, "6423: ReturnValue3.EndTresCamera");
                    self.camera_anim = self.start(Bp::ToyKey, Start::new("Drill", DRILL_FINISH02, 0.0, 0.0, true, true));
                }
            }
            // @6157 -> TresWaitCycle(1) -> @15
            "6419" if perf => self.wait_cycle = true,
            // @5976
            "6424" if perf => {
                let from = self.fov.map_or(frame.field.fov, |f| f.value());
                self.fov = Some(FovRamp { from, to: DRILL_FOV, time: DRILL_FOV_IN, elapsed: 0.0, release: false });
                self.log.push(format!("6424: active camera FOV {DRILL_FOV} in {DRILL_FOV_IN} s"));
            }
            // @5454 -> @5300 -> @601
            "6483" if perf => {
                let s = Start { on_end: Some(OnEnd::ResetPrevious), ..Start::new("drill_tosshin_finish", DRILL_TOSSHIN, 0.0, 0.0, true, false) };
                self.camera_anim = self.start(Bp::ToyKey, s);
            }
            _ => {}
        }
    }

    /// toy key `ReceiveAnimNotifyEndBpEvent` (@13065).
    fn toy_end(&mut self, seq: &str, frame: &HammerFrame) {
        if seq == "3401" && self.running(self.camera_anim) {
            let id = self.camera_anim;
            self.end_id(id, CUT_OUT, false, "end of 3401: Camera_Anim.EndTresCamera");
            // GetTresCameraNormal().ResetCamera(true)
            self.pending.reset_field_camera = true;
        }
        // @13710 Drill_cmbo_finish_end (@14424 -> @5546): by the LAST START's name
        if self.name == Some("6424") && self.perf(frame) {
            let from = self.fov.map_or(frame.field.fov, |f| f.value());
            self.fov = Some(FovRamp { from, to: frame.field.fov, time: DRILL_FOV_OUT, elapsed: 0.0, release: true });
            self.log.push(format!("end after 6424: active camera FOV back to the default in {DRILL_FOV_OUT} s"));
        }
    }

    /// timeless `ReceiveAnimNotifyStartBpEvent` (@909), Classic Tone held.
    fn timeless_start(&mut self, seq: &str, frame: &HammerFrame, world: Option<&dyn kh_core::World>) {
        let perf = self.perf(frame);
        let sora = frame.sora;
        let clear = |ps: &[Vec3]| !ps.iter().any(|p| Self::hit(world, sora, *p));
        let (anim, blend) = match seq {
            "L421" => (TIMELESS_DRILL_FINISH, 0.33),
            "K401" | "L401" if !perf => {
                self.cancel(seq);
                return;
            }
            "K401" if clear(&T_K401) => (TIMELESS_HAMMER_CHANGE, 0.0),
            "L401" if clear(&T_L401) => (TIMELESS_DRILL_CHANGE, 0.0),
            "K401" | "L401" => {
                self.log.push(format!("{seq}: no room: no camera"));
                return;
            }
            _ => return,
        };
        // @4630 -> @211
        let s = Start { on_end: Some(OnEnd::TimelessEnd), ..Start::new("LemyKey_Finish_Cam", anim, blend, 0.0, true, true) };
        if let Some(id) = self.start(Bp::Timeless, s) {
            self.camera_anim0 = Some(id);
            self.on = true;
        }
    }

    fn run_on_end(&mut self, o: OnEnd) {
        match o {
            OnEnd::ResetPrevious => self.pending.reset_field_camera = true,
            OnEnd::TimelessEnd => {
                if self.on {
                    self.on = false;
                    self.pending.reset_field_camera = true;
                }
            }
        }
    }

    /// One step: the step's core events (in order), advance the cameras, the view to draw.
    pub fn step(&mut self, dt: f32, events: &[kh_core::Event], frame: &HammerFrame, world: Option<&dyn kh_core::World>) -> HammerOutput {
        // toy key @15: the TresWaitCycle(1) of the last step's 6419
        if std::mem::take(&mut self.wait_cycle) {
            let s = Start { end_takeover: true, ..Start::new("none", DRILL_COMBO_FINISH, 1.0, 0.3, true, false) };
            self.camera_anim = self.start(Bp::ToyKey, s);
        }
        for e in events {
            self.on_event(&e.kind, frame, world);
        }
        self.state = Some(frame.state);
        // advance (PlayZeroFrame: dt 0 on the first tick); autoEnd / WaitPlayEnd at the anim's end
        let mut ended: Vec<(u32, Transition, bool, bool, Option<OnEnd>)> = Vec::new();
        for c in &mut self.cams {
            let step = if std::mem::take(&mut c.zero_frame) { 0.0 } else { dt };
            let before = c.time;
            c.time = (c.time + step).min(c.def.length);
            if c.time >= c.def.length && before < c.def.length {
                ended.push((c.id, c.end, c.end_takeover, c.auto_end, c.on_end));
            }
        }
        for (id, t, takeover, auto_end, on_end) in ended {
            if auto_end {
                self.end_id(Some(id), t, takeover, "autoEnd");
            }
            if let Some(o) = on_end {
                self.run_on_end(o);
            }
        }
        // 6424's FOV on the active camera
        let fov_now = self.fov.map(|f| f.value());
        if let Some(f) = self.fov.as_mut() {
            f.elapsed += dt;
            if f.release && f.elapsed >= f.time {
                self.fov = None;
            }
        }
        // the top camera's view with the collision pass (EnableCollision(true))
        let sora = frame.sora;
        let top = self.cams.last_mut().map(|c| {
            let v = c.view(sora);
            match world {
                Some(w) => match frame_target(&v, sora.0, w) {
                    Some(f) if f.distance > 0.0 => {
                        let start = f.location + f.rotation.forward() * f.distance;
                        AnimView { location: collide(start, f.location, w), rotation: f.rotation, ..v }
                    }
                    _ => AnimView { location: collide(sweep_start(&v, sora.0), v.location, w), ..v },
                },
                None => v,
            }
        });
        let top = top.or_else(|| fov_now.map(|f| AnimView { fov: f, ..frame.field }));
        let target = top.unwrap_or(frame.field);
        let view = match &mut self.blend {
            Some((from, t, elapsed)) => {
                *elapsed += dt;
                let a = t.alpha(*elapsed);
                let v = from.blend(&target, a);
                if a >= 1.0 {
                    self.blend = None;
                }
                Some(v)
            }
            None => top,
        };
        let view = if self.cams.is_empty() && self.blend.is_none() && fov_now.is_none() { None } else { view };
        self.last = Some(view.unwrap_or(frame.field));
        let mut out = std::mem::take(&mut self.pending);
        out.view = view;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_core::host::Event;
    use kh_core::world::Hit;

    /// The line traces hit at these world points (the sweeps never: open ground).
    struct Walls(Vec<Vec3>);
    impl kh_core::World for Walls {
        fn sweep_capsule(&self, _: Vec3, _: Vec3, _: f32, _: f32) -> Option<Hit> {
            None
        }
        fn line_trace(&self, a: Vec3, b: Vec3) -> Option<Hit> {
            self.0.iter().any(|p| p.distance(b) < 1.0).then_some(Hit { time: 0.5, position: a, normal: Vec3::X, impact_normal: Vec3::X, start_penetrating: false, penetration_depth: 0.0, component: 0 })
        }
    }

    const DT: f32 = 1.0 / 60.0;

    fn frame(state: StateId) -> HammerFrame {
        HammerFrame { sora: (Vec3::ZERO, 0.0), field: AnimView { location: Vec3::new(-400.0, 0.0, 200.0), rotation: Rot::default(), fov: 100.0 }, state, classic_tone: false, perf: PerfCamera::default() }
    }

    fn bp(seq: &'static str, begin: bool) -> Vec<Event> {
        vec![Event { step: 0, kind: EventKind::AnimBpEvent { seq, param: 0, begin } }]
    }

    fn change(from: StateId, to: StateId) -> Vec<Event> {
        vec![Event { step: 0, kind: EventKind::StateChange { from, to } }]
    }

    fn open() -> Walls {
        Walls(Vec::new())
    }

    /// Runs until no view is drawn; the steps it took.
    fn run_out(c: &mut HammerCameras, f: &HammerFrame, w: &Walls) -> usize {
        let mut n = 0;
        while c.step(DT, &[], f, Some(w)).view.is_some() {
            n += 1;
            assert!(n < 2000, "never ends");
        }
        n
    }

    fn tables() -> bool {
        let ok = camera_anim(HAMMER_START).is_some();
        if !ok {
            eprintln!("SKIPPED: no generated camera tables (node tools/party_team_pack.js --cameras)");
        }
        ok
    }

    #[test]
    fn the_seventeen_anims_are_packed() {
        if !tables() {
            return;
        }
        let want = [
            (HAMMER_START, 4.0),
            (HAMMER_START03, 4.0),
            (HAMMER_START04, 4.0),
            (BIG_STUMP, 2.333),
            (BIG_STUMP02, 2.333),
            (BIG_STUMP_SKY, 1.633),
            (DRILL_START01, 3.867),
            (DRILL_START02, 3.867),
            (DRILL_START03, 3.867),
            (DRILL_COMBO_FINISH, 1.833),
            (DRILL_FINISH01, 6.733),
            (DRILL_FINISH01_NG, 6.733),
            (DRILL_FINISH02, 4.367),
            (DRILL_TOSSHIN, 0.967),
            (TIMELESS_HAMMER_CHANGE, 3.9),
            (TIMELESS_DRILL_CHANGE, 4.233),
            (TIMELESS_DRILL_FINISH, 3.833),
        ];
        for (name, len) in want {
            let d = camera_anim(name).unwrap_or_else(|| panic!("{name}"));
            assert!((d.length - len).abs() < 2e-3, "{name}: {}", d.length);
            assert!(d.asset.contains("/TOy_key/") || d.asset.contains("/Timeless/"), "{name}: {}", d.asset);
        }
        // the clip lengths they film: StyleChange3 (3401) 4.0 s = the hammer_start anims' 4.0
        assert!(camera_anim(DRILL_FINISH02).is_some_and(|d| d.look_at_interest));
    }

    /// 3401: the clear case cuts to toy_key_hammer_start (frame 0, Sora's space, the yaw follows), held until the
    /// BpEvent's end, which cuts back and resets the field camera.
    #[test]
    fn hyper_hammer_s_transformation_camera() {
        if !tables() {
            return;
        }
        let w = open();
        let f = frame(StateId::Stylechange);
        let mut c = HammerCameras::new();
        let o = c.step(DT, &bp("3401", true), &f, Some(&w));
        assert_eq!(c.anims(), [HAMMER_START]);
        let def = camera_anim(HAMMER_START).unwrap();
        let s = def.sample(0.0);
        let v = o.view.expect("a cut in");
        assert!((v.location - s.location).length() < 1e-3 && (v.fov - s.fov).abs() < 1e-4, "PlayZeroFrame: frame 0: {v:?} vs {s:?}");
        // yaw always: Sora turns, the camera turns with him
        let turned = HammerFrame { sora: (Vec3::ZERO, 90.0), ..f };
        let v = c.step(DT, &[], &turned, Some(&w)).view.unwrap();
        let s = def.sample(DT);
        assert!((v.location - Vec3::new(-s.location.y, s.location.x, s.location.z)).length() < 1e-2, "{v:?}");
        // not autoEnd: it holds at its last frame
        for _ in 0..300 {
            assert!(c.step(DT, &[], &f, Some(&w)).view.is_some());
        }
        let o = c.step(DT, &bp("3401", false), &f, Some(&w));
        assert!(o.view.is_none() && o.reset_field_camera, "the end of 3401: a cut back, ResetCamera(true)");
        // the traces: T1 blocked = none; T2 blocked, T3 clear = start03 blended in 0.33 s; the back blocked = start04
        let mut c = HammerCameras::new();
        c.step(DT, &bp("3401", true), &f, Some(&Walls(vec![T_3401_ROOM])));
        assert!(c.anims().is_empty());
        let mut c = HammerCameras::new();
        assert!(c.step(DT, &[], &f, Some(&w)).view.is_none(), "a step with the field camera on screen");
        let first = c.step(DT, &bp("3401", true), &f, Some(&Walls(vec![T_3401_FAR]))).view.unwrap();
        assert_eq!(c.anims(), [HAMMER_START03]);
        assert!((first.location - f.field.location).length() < 5.0, "blending in from the field camera: {first:?}");
        let mut c = HammerCameras::new();
        c.step(DT, &bp("3401", true), &f, Some(&Walls(vec![T_3401_BACK])));
        assert_eq!(c.anims(), [HAMMER_START04]);
        // no performance camera (Keyblade Cinematics off in STYLECHANGE): the cancel event, no camera
        let mut c = HammerCameras::new();
        let off = HammerFrame { perf: PerfCamera { keyblade_cinematics: false, ..PerfCamera::default() }, ..f };
        let o = c.step(DT, &bp("3401", true), &off, Some(&w));
        assert!(c.anims().is_empty() && o.cancel_events == 1);
        // the StyleChange state's end ends it (EndAllSameBPTresCamera, a cut)
        let mut c = HammerCameras::new();
        c.step(DT, &bp("3401", true), &f, Some(&w));
        assert!(c.step(DT, &change(StateId::Stylechange, StateId::ComboHm), &f, Some(&w)).view.is_none());
    }

    /// Giant Stamp 3416 / 3417: the field camera reset, a cut in, autoEnd with a 0.4 s blend back.
    #[test]
    fn giant_stamp_cameras() {
        if !tables() {
            return;
        }
        let w = open();
        let f = frame(StateId::ComboHm);
        let mut c = HammerCameras::new();
        let o = c.step(DT, &bp("3416", true), &f, Some(&w));
        assert!(o.reset_field_camera && c.anims() == [BIG_STUMP] && c.cameras() == ["Big_Stump"]);
        let steps = run_out(&mut c, &f, &w);
        let want = (2.333_333 + 0.4) / DT;
        assert!((steps as f32 - want).abs() <= 2.0, "{steps} steps vs {want}");
        let mut c = HammerCameras::new();
        c.step(DT, &bp("3416", true), &f, Some(&Walls(vec![T_3416])));
        assert_eq!(c.anims(), [BIG_STUMP02]);
        let mut c = HammerCameras::new();
        c.step(DT, &bp("3417", true), &f, Some(&w));
        assert_eq!(c.anims(), [BIG_STUMP_SKY]);
        // Combo Cam Off: nothing
        let mut c = HammerCameras::new();
        let off = HammerFrame { perf: PerfCamera { combo_cam: 2, ..PerfCamera::default() }, ..f };
        let o = c.step(DT, &bp("3416", true), &off, Some(&w));
        assert!(c.anims().is_empty() && !o.reset_field_camera);
    }

    /// Drill Punch's transformation 6401: the room tests, the reset, start01 (cut) / start02 (0.33 s) / start03.
    #[test]
    fn drill_punch_s_transformation_camera() {
        if !tables() {
            return;
        }
        let w = open();
        let f = frame(StateId::Stylechange);
        let mut c = HammerCameras::new();
        let o = c.step(DT, &bp("6401", true), &f, Some(&w));
        assert!(o.reset_field_camera && c.anims() == [DRILL_START01]);
        let steps = run_out(&mut c, &f, &w);
        assert!((steps as f32 - 3.866_667 / DT).abs() <= 2.0, "autoEnd with a cut: {steps}");
        for (walls, want) in [(vec![T_6401_FAR], Some(DRILL_START02)), (vec![T_6401_BACK], Some(DRILL_START03)), (vec![T_6401_FAR, T_6401_BACK], Some(DRILL_START03)), (vec![T_6401_ROOM[1]], None)] {
            let mut c = HammerCameras::new();
            c.step(DT, &bp("6401", true), &f, Some(&Walls(walls.clone())));
            assert_eq!(c.anims().first().copied(), want, "{walls:?}");
        }
        // start02 blends out over 0.33 s
        let mut c = HammerCameras::new();
        c.step(DT, &bp("6401", true), &f, Some(&Walls(vec![T_6401_FAR])));
        let steps = run_out(&mut c, &f, &Walls(vec![T_6401_FAR]));
        assert!((steps as f32 - (3.866_667 + 0.33) / DT).abs() <= 2.0, "{steps}");
    }

    /// 6419: one cycle later the combo finish camera (in 1.0 s), autoEnd with 0.3 s and the field camera's takeover.
    #[test]
    fn the_drill_combo_finish_camera() {
        if !tables() {
            return;
        }
        let w = open();
        let f = frame(StateId::ComboDr);
        let mut c = HammerCameras::new();
        assert!(c.step(DT, &bp("6419", true), &f, Some(&w)).view.is_none(), "TresWaitCycle(1)");
        let first = c.step(DT, &[], &f, Some(&w)).view.unwrap();
        assert_eq!(c.anims(), [DRILL_COMBO_FINISH]);
        assert!((first.location - f.field.location).length() < 1.0, "1.0 s ease in-out from the field camera");
        let mut takeover = false;
        let mut n = 0;
        loop {
            let o = c.step(DT, &[], &f, Some(&w));
            takeover |= o.takeover.is_some();
            if o.view.is_none() {
                break;
            }
            n += 1;
        }
        assert!(takeover, "bEndTakeover");
        assert!((n as f32 - (1.833_333 + 0.3) / DT).abs() <= 2.0, "{n}");
    }

    /// FINISH_DR in Drill Punch: 6421 holds finish01; 6423 cuts to finish02 (autoEnd, a cut back), or, with no room,
    /// ends finish01 over 0.5 s with the takeover; 6421 blocked plays finish01_NG; STYLEFINISH's end ends them.
    #[test]
    fn drill_punch_s_ultimate_finish_cameras() {
        if !tables() {
            return;
        }
        let w = open();
        let f = frame(StateId::Stylefinish);
        let mut c = HammerCameras::new();
        c.step(DT, &bp("6421", true), &f, Some(&w));
        assert_eq!(c.anims(), [DRILL_FINISH01]);
        for _ in 0..90 {
            c.step(DT, &[], &f, Some(&w));
        }
        c.step(DT, &bp("6421", false), &f, Some(&w));
        assert_eq!(c.anims(), [DRILL_FINISH01], "the end of 6421 ends nothing");
        let o = c.step(DT, &bp("6423", true), &f, Some(&w));
        assert_eq!(c.anims(), [DRILL_FINISH02]);
        let s = camera_anim(DRILL_FINISH02).unwrap().sample(0.0);
        assert!((o.view.unwrap().location - s.location).length() < 1e-2, "a cut to frame 0");
        let steps = run_out(&mut c, &f, &w);
        assert!((steps as f32 - 4.366_667 / DT).abs() <= 2.0, "{steps}");
        // no room for finish02: finish01 blends out over 0.5 s, the field camera takes over
        let mut c = HammerCameras::new();
        c.step(DT, &bp("6421", true), &f, Some(&w));
        let o = c.step(DT, &bp("6423", true), &f, Some(&Walls(vec![T_6423[1]])));
        assert!(c.anims().is_empty() && o.takeover.is_some() && o.view.is_some());
        let steps = run_out(&mut c, &f, &w);
        assert!((steps as f32 - 0.5 / DT).abs() <= 2.0, "{steps}");
        let mut c = HammerCameras::new();
        c.step(DT, &bp("6421", true), &f, Some(&Walls(vec![T_6421])));
        assert_eq!(c.anims(), [DRILL_FINISH01_NG]);
        assert!(c.step(DT, &change(StateId::Stylefinish, StateId::ComboDr), &f, Some(&w)).view.is_none(), "dtor STYLEFINISH: a cut");
    }

    /// 6424: the active (field) camera's FOV goes to 130 in 0.2 s; the end of the BpEvent (the last start was 6424) brings
    /// it back to the default in 1 s.
    #[test]
    fn the_drill_fov_kick() {
        let w = open();
        let f = frame(StateId::ComboDr);
        let mut c = HammerCameras::new();
        let first = c.step(DT, &bp("6424", true), &f, Some(&w)).view.unwrap();
        assert!((first.fov - 100.0).abs() < 1e-4 && first.location == f.field.location);
        let mut v = first;
        for _ in 0..30 {
            v = c.step(DT, &[], &f, Some(&w)).view.unwrap();
        }
        assert_eq!(v.fov, DRILL_FOV, "held at 130");
        c.step(DT, &bp("6424", false), &f, Some(&w));
        let mut n = 0;
        while let Some(v) = c.step(DT, &[], &f, Some(&w)).view {
            assert!(v.fov <= DRILL_FOV && v.fov >= 100.0);
            n += 1;
            assert!(n < 200);
        }
        assert!((n as f32 - DRILL_FOV_OUT / DT).abs() <= 2.0, "{n}");
        assert!(!c.active());
        // the end of another BpEvent after a newer start is not 6424's
        let mut c = HammerCameras::new();
        c.step(DT, &bp("6424", true), &f, Some(&w));
        c.step(DT, &bp("6403", true), &f, Some(&w));
        c.step(DT, &bp("6424", false), &f, Some(&w));
        for _ in 0..30 {
            c.step(DT, &[], &f, Some(&w));
        }
        for _ in 0..100 {
            assert_eq!(c.step(DT, &[], &f, Some(&w)).view.map(|v| v.fov), Some(DRILL_FOV), "held: no reset for another sequence's end");
        }
    }

    /// Drill Dash 6483: a cut in, autoEnd with a cut, then WaitPlayEnd -> ResetTresCameraPrevious.
    #[test]
    fn the_drill_dash_camera() {
        if !tables() {
            return;
        }
        let w = open();
        let f = frame(StateId::ComboDr);
        let mut c = HammerCameras::new();
        c.step(DT, &bp("6483", true), &f, Some(&w));
        assert_eq!(c.cameras(), ["drill_tosshin_finish"]);
        let mut reset = false;
        let mut n = 0;
        loop {
            let o = c.step(DT, &[], &f, Some(&w));
            reset |= o.reset_field_camera;
            if o.view.is_none() {
                break;
            }
            n += 1;
        }
        assert!(reset && (n as f32 - 0.966_667 / DT).abs() <= 2.0, "{n}");
    }

    /// Classic Tone: K401 / L401 need it held and room; L421 (the Clock) blends in 0.33 s, autoEnds with a cut and resets the
    /// field camera; DAMAGE ends it without the reset; STYLEFINISH_DR's end ends it with the reset.
    #[test]
    fn the_timeless_key_cameras() {
        if !tables() {
            return;
        }
        let w = open();
        let f = HammerFrame { classic_tone: true, ..frame(StateId::Stylechange) };
        let mut c = HammerCameras::new();
        c.step(DT, &bp("K401", true), &frame(StateId::Stylechange), Some(&w));
        assert!(c.anims().is_empty(), "another keyblade: nothing");
        c.step(DT, &bp("K401", true), &f, Some(&w));
        assert_eq!(c.anims(), [TIMELESS_HAMMER_CHANGE]);
        let mut c = HammerCameras::new();
        c.step(DT, &bp("K401", true), &f, Some(&Walls(vec![T_K401[1]])));
        assert!(c.anims().is_empty());
        c.step(DT, &bp("L401", true), &f, Some(&w));
        assert_eq!(c.anims(), [TIMELESS_DRILL_CHANGE]);
        let fin = HammerFrame { state: StateId::StylefinishDr, ..f };
        let mut c = HammerCameras::new();
        assert!(c.step(DT, &[], &fin, Some(&w)).view.is_none(), "a step with the field camera on screen");
        let first = c.step(DT, &bp("L421", true), &fin, Some(&w)).view.unwrap();
        assert!((first.location - f.field.location).length() < 5.0, "0.33 s blend in");
        let mut reset = false;
        let mut n = 0;
        loop {
            let o = c.step(DT, &[], &fin, Some(&w));
            reset |= o.reset_field_camera;
            if o.view.is_none() {
                break;
            }
            n += 1;
        }
        assert!(reset && (n as f32 - 3.833_333 / DT).abs() <= 2.0, "{n}");
        let mut c = HammerCameras::new();
        c.step(DT, &bp("L421", true), &fin, Some(&w));
        let o = c.step(DT, &change(StateId::StylefinishDr, StateId::Damage), &fin, Some(&w));
        assert!(o.view.is_none() && o.reset_field_camera, "dtor 34: EndAll + ResetTresCameraPrevious");
        let mut c = HammerCameras::new();
        c.step(DT, &bp("L421", true), &fin, Some(&w));
        let o = c.step(DT, &change(StateId::ComboDr, StateId::Damage), &fin, Some(&w));
        assert!(o.view.is_none() && !o.reset_field_camera, "ctor DAMAGE: camera_anim0 ends, On off");
        for _ in 0..300 {
            assert!(!c.step(DT, &[], &fin, Some(&w)).reset_field_camera, "no WaitPlayEnd reset after DAMAGE");
        }
    }
}
