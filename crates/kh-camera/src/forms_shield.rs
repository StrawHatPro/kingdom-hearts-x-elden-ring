//! (AUDIT-A CAMS) THE SHIELD AND PAN FORMS' CAMERA BLUEPRINTS as data: Hero's Origin's (Counter Shield) `Blueprints/Camera/
//! p_ex001_StateEvent/change_key/BP_TresPlayerStateEventCamera_tate_finish` and Grand Chef's (Frying Pan)
//! `.../change_key/BP_TresPlayerStateEventCamera_Lemylkey`. Bytecode read in full 2026-11-05 with
//! build/sora_tmp/weapons/AUDIT-C/CAM/bpdis.js (listings build/sora_tmp/weapons/AUDIT-A/CAMS/bp/
//! BP_TresPlayerStateEventCamera_{tate_finish,Lemylkey}.txt); `@n` = the ubergraph's bytecode offset. Both blueprints are in
//! Sora's load list (Load/Player/TresPlayerPawnLoad_ex001: `tate_finish_C`, `Lemylkey_C`): they run with every keyblade.
//!
//! Conventions (as forms_hammer.rs / forms_staff.rs): a trace = `BP_TresLineTraceByChannel(Sora's location + (0, 0, 100) ->
//! TransformLocation(Sora's transform, P), channel 1)`, "hit" = it returned true; perf = `BP_TresIsGameEnablePerformanceCamera`
//! (form_bp.rs [`crate::form_bp::PerfCamera`], with the pawn's state at the event); a blend `{t, f, e, lock}` =
//! `SetTresCameraTransitionParams(t, f, e, lock, ..)`, "cut" = t 0; `StartTresCameraAnim(anim, bLoop, bTakeover, in, bAutoEnd,
//! bEndTakeover, end)`. `End Tres Camera Normal(cam, takeover, time, func, lock, interp, pp)` (FunctionLibrary_Camera) =
//! `cam.EndTresCamera(takeover, {time, func, 2, lock})`. Every camera here: `BP_SettingTresCamera(Sora)` unless said,
//! `EnableCollision(true)`, `SetPriority(200)`.
//!
//! # BP_TresPlayerStateEventCamera_tate_finish (no weapon test)
//! - `ReceivePlayerBeginCommandAction` (@30198): 137 CHANGE_SH -> @7735: perf (no -> @9158 RemoteEvent('Cnage_Key_Cancel')).
//!   Trace (684.07, -95.41, 38.91) hit -> nothing; trace (700, -700, 2000) hit -> SH_tate_start02, clear -> SH_tate_start.
//!   @8642 "SH_tate_start": attached, `StartTresCameraAnim(anim, false, false, in {0.3, 4, 2, lock}, autoEnd, bEndTakeover,
//!   end {0.3, 3, 2, lock})` = Camera_Anim.
//! - `ReceiveAnimNotifyStartBpEvent` (@40371, Sora only; `Nmae` = AnimSeqName, `ID` = Param):
//!   - B418 (Counter Shield's grown 0x55; COMBO_SH) -> @5690 perf -> Big_Punch_camera_Start (@42349):
//!     - ID 0 -> @41318: ATARI = false; trace (-416, -96, 53) hit -> BIG_PUNCH_NG; clear: trace (-399, -215, 77) hit ->
//!       BIG_PUNCH_NG, clear -> BIG_PUNCH. @7673 "BIG_PUNCH" (local RV5, = Camera_Anim): attached, `(anim, false, bTakeover,
//!       in {0.3, 2, 2, lock}, autoEnd, bEndTakeover, end {0.3, 3, 2, lock})`.
//!     - ID 2 -> @42402: traces (68, -75, 210) and (1221, -248, 73): either hit -> nothing; both clear -> @9215 RV5.End(cut),
//!       @5628 "BIG_PUNCH02" = "Camera Anim": attached, `(BIG_PUNCH02, false, false, a cut, no autoEnd, no takeover, cut)`;
//!       ATARI = true; `SetGlobalGameSpeed(1, 0.1, 0.08)`.
//!     - ID 3 -> @41749: nothing.
//!   - B426 (grown 0x56) -> @6194 perf -> 100attack_camera_start (@42140):
//!     - ID 0 -> @42032: ATARI = false; @3178 "BIG_PUNCH" = "Camera Anim": attached, `(100_PUNCH01, false, bTakeover, in
//!       {0.3, 2, 2, lock}, autoEnd, bEndTakeover, end {0.3, 3, 2, lock})`; `Delay(1)` -> @2493
//!       `ClientPlayCameraShake(shake_bibibi_8sec, 3)`, ATARI = true.
//!     - ID 2 -> @4948: `IsOverlapTresCameraActor(Sora, P, 80)` at (1000, -300, 100), (800, -250, 100), (600, -200, 100),
//!       (400, -150, 100), (200, -100, 100): any overlap -> nothing; else @4476 "Camera Anim".End(cut), @3742 "BIG_PUNCH02" =
//!       "Camera Anim": attached, `(100_PUNCH02, false, false, a cut, no autoEnd, no takeover, cut)`, the shake again x3,
//!       ATARI = true.
//!     - ID 3 -> nothing. ID 4 -> @42048 `SetGlobalGameSpeed(1, 0.05, 0.2)`.
//!   - B417 (the combo finisher, both forms; no perf test) -> Combo_Finish_camera_Start (@42344 -> @6262): the RotTarget
//!     "none" (RV2 = Camera_Rot): not attached, `SetTresCameraTargetInfluenceRotation(false, 0, false, ..)` (no yaw of his),
//!     `SetTresCameraRotTargetParam(600, GetCameraRotation(), 0, local (0, 0, 100))`, `StartTresCamera(bTakeover, {0.2, 4,
//!     2})`, `SetFieldOfView(110, 0.2)`.
//!   - B481 -> Lead_shot (@42135): the shotlock's (SHOT's), not ported here.
//!   - B423 (FINISH_SH's first clip) -> Tate_finish_camera_Normal (@44089 -> @6970): RotTarget "none" (local RV3): not
//!     attached, yaw once, (450, R(-20, 30, 0), 0, local (0, 0, 50)), `StartTresCamera(bTakeover, {0.3, 4, 2})`; @2209 the
//!     cast of `Tres Char Pawn base` succeeds: `SetOutsideWallCheckType(4, .., the pawn)`; `Delay(3)` -> @2434
//!     `End Tres Camera Normal(RV3, takeover, 2, 4, lock)`.
//!   - B424 (FINISH_SH: Pegasus and the Chariot appear) -> Tate_finish_camera (@43331): "SH_tate_End" (RV7 = Camera) ON THE
//!     PAWN (`if (false)` @43388 -> @43783 `BP_SettingTresCamera(self.Tres Char Pawn base)`): not attached,
//!     `(SH_tate_cut02, false, false, a cut, autoEnd, bEndTakeover, end {0.4, 0 linear, 2, lock})`; ON = true;
//!     `TresWaitCycle(1)` -> @2619 `WaitPlayEnd(RV7)` -> @15 `TresWaitCycle(1)` -> @92 `Delay(2)` -> @169 (ON): the RotTarget
//!     "CUt02" (RV4 = Camera_Rot2): attached, yaw once, (50, R(-20, 170, 0), 0, local (50, 0, 0)), a cut; `Delay(0.1)` ->
//!     @810 `SetDistance(100, 4)`, `SetBaseRotation(R(-25, 140, 0), 4)`; `Delay(4)` -> @1401 RV4.End(cut), (ON) the RotTarget
//!     "Cut03" (Camera_Rot3): attached, yaw once, (450, R(-20, 30, 0), 0, local (100, 0, 0)), a cut; `Delay(2)` -> @2085
//!     End(cut).
//! - `ReceiveAnimNotifyEndBpEvent` (@40918, Sora only):
//!   - B418 -> Big_Punch_camera_End (@43987): ID 2 -> @41775 "Camera Anim".End(cut); ID 3 -> @41760 (ATARI)
//!     `SetGlobalGameSpeed(1, 1, 0.2)`.
//!   - B426 -> 100attack_camera_end (@43836): ID 2 -> @41859 "Camera Anim".End(cut), `ClientStopCameraShake(bibibi,
//!     immediately)`, ATARI = false, @3178 100_PUNCH01 again (as ID 0's start, its Delay(1) and shake); ID 4 -> @42094
//!     `ResetGlobalGameSpeed(1, 0.01)`.
//!   - B417 -> Combo_Finish_camera_End (@43831 -> @6911): `End Tres Camera Normal(RV2, takeover, 0.4, 0 linear, no lock)`.
//! - `ReceivePlayerSpecificAction` (@9766): ActionID 0 -> @15738: `GetStateID() == 38` (STYLEFINISH_SH) -> @22847
//!   `Tres Char Pawn base = GetSpecificActionAccompanyPawn(0)` (Pegasus: the first pawn 0x1405D9130 spawns in Tick sub 0;
//!   Frying Pan's slot 167 sends it too: its pan), RemoteEvent('HS_tate_get_pos'). ActionID 64 -> @15228: Camera, Camera_Normal,
//!   Camera_Rot, Camera_Rot2, Camera_Anim, Camera_Rot3 `EndTresCamera(cut)`; ON = false. (kh-core combo_sh.rs sends both:
//!   Tick sub 0's pawn vt+0x1038(0, 0) and the cleanup 0x1413CFB60's vt+0x1038(0x40), once.)
//! - `ReceivePlayerCtorState` (@44384): DAMAGE 19 -> Camera_Rot and Camera_Anim `EndTresCamera(cut)`.
//! - `ReceivePlayerDtorState` (@23017): STYLECHANGE 31, WEAPONCHANGE 32, STYLEFINISH 33, STYLEFINISH_SH 38, SHOOTFLOW 41,
//!   COMBO_SH 84 -> @30053 `EndAllSameBPTresCamera({0, 3, 2})` (a cut) and `EndAllSameBPTresLatent` (its pending latents).
//!
//! # BP_TresPlayerStateEventCamera_Lemylkey
//! - `ReceiveAnimNotifyStartBpEvent` (@211): Sora only and `BP_GetEquipWeaponID() == 11` (WEP_KEYBLADE11 = Grand Chef, item
//!   WEP_KEYBLADE_SO_10) (@369 -> @3285), then:
//!   - J423 (FINISH_SH's pan finish; no perf test) -> LemyKey_Finish_Cam (@5016): trace (133.16, -92.22, 88.06) hit -> none;
//!     trace (-338.03, 226.08, 421.06) hit -> none; trace (717.65, -439.15, 270.37) hit -> LemyKey_Finish_cam_Atari, Blend
//!     0.33; clear -> LemyKey_Finish_Cam, Blend 0.
//!   - J401 (StyleChange19) -> @82 perf (no -> RemoteEvent) -> LemyKey_Change_Cam (@12604): trace (257.28, -49.92, 155.94)
//!     hit -> none; clear -> LemyKey_Change_Cam, Blend 0.
//!   Both (@4286): "LemyKey_Finish_Cam" not attached, `(anim, false, false, in {Blend, 4, 2, lock}, autoEnd, no takeover,
//!   end {0, 4, 2, lock})` = camera_anim0, On = true, PlayZeroFrame, yaw always; `WaitPlayEnd` -> @27 (On)
//!   `ResetTresCameraExceptActive` (0x14222A350 -> 0x140E631E0 -> 0x1405EEE50: every camera of the manager's stack under the
//!   top one Reset(snap), vt+0x7A0(1): the field camera behind him), On = false.
//! - `ReceivePlayerCtorState` (@5432): DAMAGE 19 -> @12448 On = false, camera_anim0.End(cut).
//! - `ReceivePlayerDtorState` (@13067): 31 / 33 / 38 -> @20180 `EndAllSameBPTresCamera(cut)`, On = false,
//!   `ResetTresCameraPrevious`; 32 -> @20095 the same end, On = false.
//!
//! # Port notes
//! - The weapon gate of Lemylkey is implied: J401 / J423 are Grand Chef's form clips (StyleChange19, its M_J423); a form is
//!   held in its own keyblade's slot (P2), so they never play with another keyblade equipped.
//! - `Tres Char Pawn base` (Pegasus) is placed from kh-core's ride links (shield.rs `ride_link`, the plink rows: Sora's actor
//!   in Pegasus's frame per frame of M_B424, the clip Pegasus and the Chariot play with him while SH_tate_cut02 (3.665 s)
//!   runs), at the time since B424's BpEvent began; before B424 (or without a row) at Sora's actor as the event saw it.
//! - [I] The RotTargets' bTakeover at their start is taken as a blend from the view on screen (the transition given), as
//!   their rotation / distance are set by the parameters; SetFieldOfView(110, 0.2) ramps from the native camera's 90.
//! - `SetGlobalGameSpeed` / `ResetGlobalGameSpeed` (B418 / B426's hit-stops: the world AND Sora slowed) are no camera: they
//!   are published in [`ShieldCameras::game_speed`] for a host that owns the world clock (none does yet: reported).
//! - `Delay` counts the step's dt; a `Delay` node re-entered while pending is ignored (UE's FLatentActionManager keeps one
//!   action per node UUID); `TresWaitCycle(n)` n steps; `WaitPlayEnd` fires the step after the anim reached its end (also when
//!   bAutoEnd removed it), not when another end destroyed it first.

use glam::Vec3;
use kh_core::host::{Event, EventKind};
use kh_core::{StateId, World};

use crate::camera_anim::{camera_anim, camera_shake, collide, frame_target, sweep_start, AnimView, CameraAnimDef, Transition};
use crate::form_bp::FormBpFrame;
use crate::forms_guns::KeyCamOutput;
use crate::shake::ShakeSet;
use crate::Rot;

/// CHANGE_SH (ETresCommandKind 137): tate_finish's ReceivePlayerBeginCommandAction case.
pub const CHANGE_SH: i32 = 137;
/// The shake of B426 (@2536 / @4232: scale 3).
pub const BIBIBI: &str = "shake_bibibi_8sec";
pub const BIBIBI_SCALE: f32 = 3.0;

/// The traces start at his location + this.
pub const TRACE_FROM: Vec3 = Vec3::new(0.0, 0.0, 100.0);
/// CHANGE_SH: hit = no camera (@7985); then hit = SH_tate_start02 (@8401).
pub const CHANGE_BLOCK: Vec3 = Vec3::new(684.068_6, -95.409_21, 38.905_754);
pub const CHANGE_TIGHT: Vec3 = Vec3::new(700.0, -700.0, 2000.0);
/// B418 ID 0: either hit = BIG_PUNCH_NG (@41522, @9496).
pub const BIG_PUNCH_TIGHT: [Vec3; 2] = [Vec3::new(-416.0, -96.0, 53.0), Vec3::new(-399.0, -215.0, 77.0)];
/// B418 ID 2: either hit = no BIG_PUNCH02 (@42643, @43059).
pub const BIG_PUNCH02_BLOCK: [Vec3; 2] = [Vec3::new(68.0, -75.0, 210.0), Vec3::new(1221.0, -248.0, 73.0)];
/// B426 ID 2: `IsOverlapTresCameraActor(Sora, P, 80)`, in the order tested (@4991 .. @4391).
pub const PUNCH02_FREE: [Vec3; 5] = [Vec3::new(1000.0, -300.0, 100.0), Vec3::new(800.0, -250.0, 100.0), Vec3::new(600.0, -200.0, 100.0), Vec3::new(400.0, -150.0, 100.0), Vec3::new(200.0, -100.0, 100.0)];
pub const PUNCH02_RADIUS: f32 = 80.0;
/// Lemylkey: J423 (@5209 hit = none, @3606 hit = none, @4022 hit = _Atari) and J401 (@12797 hit = none).
pub const LEMY_FINISH_BLOCK: [Vec3; 2] = [Vec3::new(133.159_35, -92.224_49, 88.055_91), Vec3::new(-338.034_55, 226.082_49, 421.059_7)];
pub const LEMY_FINISH_TIGHT: Vec3 = Vec3::new(717.652_6, -439.151_7, 270.365_45);
pub const LEMY_CHANGE_BLOCK: Vec3 = Vec3::new(257.275_2, -49.924_68, 155.942_5);

/// `SetTresCameraRotTargetParam(Distance, Rotation, TargetOffset, TargetLocalOffset)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotParam {
    pub distance: f32,
    pub rotation: Rot,
    pub local_offset: Vec3,
}

/// B423 @7238 "none"; B417 @6623 "none" (its rotation: the camera's); FINISH_SH's ride: @447 "CUt02" (then SetDistance(100, 4),
/// SetBaseRotation(R(-25, 140, 0), 4) @854 / @902) and @1762 "Cut03".
pub const B423_ROT: RotParam = RotParam { distance: 450.0, rotation: Rot::new(-20.0, 30.0, 0.0), local_offset: Vec3::new(0.0, 0.0, 50.0) };
pub const B417_DISTANCE: f32 = 600.0;
pub const B417_LOCAL: Vec3 = Vec3::new(0.0, 0.0, 100.0);
pub const B417_FOV: (f32, f32) = (110.0, 0.2);
pub const CUT02_ROT: RotParam = RotParam { distance: 50.0, rotation: Rot::new(-20.0, 170.0, 0.0), local_offset: Vec3::new(50.0, 0.0, 0.0) };
pub const CUT02_MOVE: (f32, Rot, f32) = (100.0, Rot::new(-25.0, 140.0, 0.0), 4.0);
pub const CUT03_ROT: RotParam = RotParam { distance: 450.0, rotation: Rot::new(-20.0, 30.0, 0.0), local_offset: Vec3::new(100.0, 0.0, 0.0) };

const CUT: Transition = Transition::new(0.0, 3, 2.0, false);

/// (FINISH-AUDIT2) StylePawn/w_so036 (Pegasus) TresCharMesh0 RelativeLocation x: its mesh, the camera base of a camera on it
/// (`m_bCameraLookPosToMesh` true in Default__w_so036_C), stands 100 cm ahead of its actor.
const PEGASUS_MESH_X: f32 = 100.0;

/// One `StartTresCameraAnim` call's settings.
#[derive(Clone, Copy, Debug)]
struct AnimStart {
    anim: &'static str,
    blend_in: Transition,
    auto_end: bool,
    end: Transition,
    end_takeover: bool,
}

/// {t, f, 2, lock}.
const fn tr(time: f32, function: u8, lock: bool) -> Transition {
    Transition::new(time, function, 2.0, lock)
}

/// The two blueprints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Bp {
    Tate,
    Lemy,
}

/// The blueprint variable / local a camera is held in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    /// tate Camera_Anim (SH_tate_start).
    TateStart,
    /// tate's local RV5 (BIG_PUNCH / _NG; also Camera_Anim).
    BigPunch,
    /// tate "Camera Anim" (BIG_PUNCH02, 100_PUNCH01 / 02).
    CameraAnimSp,
    /// tate Camera_Rot (B417's "none").
    ComboRot,
    /// tate's local RV3 (B423's "none").
    B423Rot,
    /// tate Camera (SH_tate_End).
    TateEnd,
    /// tate Camera_Rot2 ("CUt02") / Camera_Rot3 ("Cut03").
    Rot2,
    Rot3,
    /// Lemylkey camera_anim0.
    Lemy,
}

/// The actor a camera is set on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum On {
    Sora,
    /// `Tres Char Pawn base` (Pegasus).
    Pawn,
}

/// A type-5 ease-out move of a value (`SetDistance` / `SetBaseRotation` / `SetFieldOfView`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Ramp {
    from: f32,
    to: f32,
    time: f32,
    elapsed: f32,
}

impl Ramp {
    fn fixed(v: f32) -> Ramp {
        Ramp { from: v, to: v, time: 0.0, elapsed: 0.0 }
    }
    fn value(&self) -> f32 {
        if self.time <= 0.0 {
            self.to
        } else {
            self.from + (self.to - self.from) * crate::lockon::interp_curve(5, self.elapsed / self.time)
        }
    }
    fn set(&mut self, to: f32, time: f32) {
        *self = Ramp { from: self.value(), to, time, elapsed: 0.0 };
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Anim { def: &'static CameraAnimDef, time: f32, auto_end: bool, end: Transition, end_takeover: bool, zero_frame: bool },
    /// `ATresCameraRotTarget`: distance, `m_Rotation` (pitch, yaw, roll), the local offset, the FOV.
    Orbit { distance: Ramp, rotation: [Ramp; 3], local_offset: Vec3, fov: Ramp },
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Cam {
    id: u32,
    bp: Bp,
    name: &'static str,
    slot: Slot,
    on: On,
    kind: Kind,
    /// The target's yaw: taken every step / once / never (B417: `SetTresCameraTargetInfluenceRotation(false, 0, false ..)`).
    yaw: YawInfluence,
    base_yaw: Option<f32>,
    /// `SetOutsideWallCheckType(4, .., the pawn)`.
    wall_pawn: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum YawInfluence {
    Always,
    Once,
    None,
}

/// A latent continuation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Then {
    /// @2493 (Delay(1) of 100_PUNCH01): the shake, ATARI.
    PunchShake,
    /// @2434 (Delay(3) of B423).
    B423End,
    /// @2619 (TresWaitCycle(1) after SH_tate_End): its WaitPlayEnd.
    TateEndWait,
    /// @15 (WaitPlayEnd of SH_tate_End): TresWaitCycle(1).
    TateEndPlayed,
    /// @92 (TresWaitCycle(1)): Delay(2).
    TateDelay2,
    /// @169 (Delay(2)): CUt02.
    Cut02,
    /// @810 (Delay(0.1)): CUt02's moves.
    Cut02Move,
    /// @1401 (Delay(4)): CUt02 ends, Cut03.
    Cut03,
    /// @2085 (Delay(2)): Cut03 ends.
    Cut03End,
    /// Lemylkey @27 (WaitPlayEnd of camera_anim0).
    LemyPlayed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Wait {
    Cycles(u32),
    Time(f32),
    PlayEnd(u32),
}

/// The shake player (compared by whether one plays: `ShakeSet` has no equality).
#[derive(Clone, Debug, Default)]
struct Shakes(ShakeSet);

impl PartialEq for Shakes {
    fn eq(&self, other: &Shakes) -> bool {
        self.0.is_empty() == other.0.is_empty()
    }
}

/// The two blueprints' state (one per Sora).
#[derive(Clone, Debug, PartialEq)]
pub struct ShieldCameras {
    cams: Vec<Cam>,
    next_id: u32,
    blend: Option<(Option<AnimView>, Transition, f32)>,
    last: Option<AnimView>,
    latent: Vec<(Bp, Wait, Then)>,
    shakes: Shakes,
    /// The pawn's state (tracked through StateChange; the frame's at each step's end).
    state: Option<StateId>,
    /// tate `Tres Char Pawn base` is set (Sora's actor at the specific action), ON, ATARI.
    pawn_at: Option<(Vec3, f32)>,
    on: bool,
    atari: bool,
    /// The time since B424's BpEvent began (Pegasus's place through the ride links).
    b424: Option<f32>,
    /// Lemylkey On.
    lemy_on: bool,
    /// The anims whose own end ran this step (their WaitPlayEnd fires at the next one).
    played: Vec<u32>,
    pending: KeyCamOutput,
    /// `SetGlobalGameSpeed(1, rate, time)` / `ResetGlobalGameSpeed(1, time)` = (rate 1, time) of this step (hosts that own
    /// the world clock take it; see the module notes).
    pub game_speed: Option<(f32, f32)>,
    /// What the blueprints did (tests, a host's log).
    pub log: Vec<String>,
}

impl Default for ShieldCameras {
    fn default() -> ShieldCameras {
        ShieldCameras { cams: Vec::new(), next_id: 0, blend: None, last: None, latent: Vec::new(), shakes: Shakes(ShakeSet::new()), state: None, pawn_at: None, on: false, atari: false, b424: None, lemy_on: false, played: Vec::new(), pending: KeyCamOutput::default(), game_speed: None, log: Vec::new() }
    }
}

fn turn(v: Vec3, yaw: f32) -> Vec3 {
    let (s, c) = yaw.to_radians().sin_cos();
    Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
}

/// The trace from his location + `TRACE_FROM` to his local `to` hit (no level: clear).
fn hit(world: Option<&dyn World>, at: (Vec3, f32), to: Vec3) -> bool {
    world.is_some_and(|w| w.line_trace(at.0 + TRACE_FROM, at.0 + turn(to, at.1)).is_some())
}

impl ShieldCameras {
    /// A camera of the blueprints is on (or blending out).
    pub fn active(&self) -> bool {
        !self.cams.is_empty() || self.blend.is_some()
    }

    /// The running cameras' anims / names, oldest first.
    pub fn cameras(&self) -> Vec<&'static str> {
        self.cams
            .iter()
            .map(|c| match c.kind {
                Kind::Anim { def, .. } => def.name,
                Kind::Orbit { .. } => c.name,
            })
            .collect()
    }

    fn push(&mut self, mut cam: Cam, transition: Transition) -> u32 {
        cam.id = self.next_id;
        self.next_id += 1;
        let label = match cam.kind {
            Kind::Anim { def, .. } => def.name.to_string(),
            Kind::Orbit { distance, rotation, .. } => format!("RotTarget {} R({}, {}, {})", distance.value(), rotation[0].value(), rotation[1].value(), rotation[2].value()),
        };
        self.log.push(format!("start {} {label} (in {} s)", cam.name, transition.time));
        let from = if transition.lock_outgoing || !self.cams.is_empty() { self.last } else { None };
        self.blend = (transition.time > 0.0).then_some((from, transition, 0.0));
        self.cams.push(cam);
        cam.id
    }

    /// `BP_SpawnTresCameraAnim(name)` on `on` + `StartTresCameraAnim(s.anim, false, .., s.blend_in, s.auto_end, s.end_takeover, s.end)`.
    fn anim(&mut self, bp: Bp, name: &'static str, slot: Slot, on: On, s: AnimStart) -> Option<u32> {
        let Some(def) = camera_anim(s.anim) else {
            self.log.push(format!("{} is not in the generated camera tables (node tools/party_team_pack.js --cameras)", s.anim));
            return None;
        };
        let c = Cam { id: 0, bp, name, slot, on, kind: Kind::Anim { def, time: 0.0, auto_end: s.auto_end, end: s.end, end_takeover: s.end_takeover, zero_frame: false }, yaw: YawInfluence::Once, base_yaw: None, wall_pawn: false };
        Some(self.push(c, s.blend_in))
    }

    fn orbit(name: &'static str, slot: Slot, p: RotParam, yaw: YawInfluence) -> Cam {
        let rotation = [Ramp::fixed(p.rotation.pitch), Ramp::fixed(p.rotation.yaw), Ramp::fixed(p.rotation.roll)];
        Cam { id: 0, bp: Bp::Tate, name, slot, on: On::Sora, kind: Kind::Orbit { distance: Ramp::fixed(p.distance), rotation, local_offset: p.local_offset, fov: Ramp::fixed(crate::forms::NATIVE_FOV) }, yaw, base_yaw: None, wall_pawn: false }
    }

    fn end_where(&mut self, transition: Transition, takeover: bool, why: &str, pred: impl Fn(&Cam) -> bool) {
        let top = self.cams.last().map(|c| c.id);
        let before = self.cams.len();
        self.cams.retain(|c| !pred(c));
        if self.cams.len() == before {
            return;
        }
        self.log.push(format!("end {} camera(s) ({why})", before - self.cams.len()));
        if self.cams.last().map(|c| c.id) != top {
            self.blend = (transition.time > 0.0).then_some((self.last, transition, 0.0));
            if takeover && self.cams.is_empty() {
                self.pending.takeover = self.last;
            }
        }
    }

    fn end_slots(&mut self, slots: &[Slot], why: &str) {
        self.end_where(CUT, false, why, |c| slots.contains(&c.slot));
    }

    /// The camera a variable holds (the newest of that slot).
    fn slot_id(&self, slot: Slot) -> Option<u32> {
        self.cams.iter().rev().find(|c| c.slot == slot).map(|c| c.id)
    }

    fn later(&mut self, bp: Bp, wait: Wait, then: Then) {
        // a Delay node re-entered while pending is ignored (one latent action per node)
        if matches!(wait, Wait::Time(_)) && self.latent.iter().any(|(_, w, t)| *t == then && matches!(w, Wait::Time(_))) {
            return;
        }
        self.latent.push((bp, wait, then));
    }

    fn speed(&mut self, rate: f32, time: f32, why: &str) {
        self.game_speed = Some((rate, time));
        self.log.push(format!("{why}: game speed x{rate} in {time} s"));
    }

    fn shake(&mut self, why: &str) {
        if let Some(s) = camera_shake(BIBIBI) {
            self.shakes.0.play(&s.def(), BIBIBI_SCALE, None);
            self.log.push(format!("{why}: shake {BIBIBI} x{BIBIBI_SCALE}"));
        }
    }

    fn perf(&self, frame: &FormBpFrame) -> bool {
        frame.perf.enabled(self.state.unwrap_or(frame.state))
    }

    // ---------------------------------------------------------------- tate_finish

    /// @7735 (CHANGE_SH began).
    fn change_sh(&mut self, frame: &FormBpFrame, world: Option<&dyn World>) {
        // 0x140EAB300 also tests the QUEUED state (form_bp.rs): StartStyle queued STYLECHANGE before 0x1414158F0 sent this
        // event, so "Keyblade Cinematics" decides, as for the other transformations
        if !frame.perf.enabled(StateId::Stylechange) {
            self.log.push("CHANGE_SH: no performance camera: RemoteEvent(Cnage_Key_Cancel) @9158".to_string());
            return;
        }
        if hit(world, frame.sora, CHANGE_BLOCK) {
            self.log.push("CHANGE_SH: the trace @7985 hit: no camera".to_string());
            return;
        }
        let anim = if hit(world, frame.sora, CHANGE_TIGHT) { "SH_tate_start02" } else { "SH_tate_start" };
        self.anim(Bp::Tate, "SH_tate_start", Slot::TateStart, On::Sora, AnimStart { anim, blend_in: tr(0.3, 4, true), auto_end: true, end: tr(0.3, 3, true), end_takeover: true });
    }

    /// @3178: "BIG_PUNCH" 100_PUNCH01 = "Camera Anim", Delay(1) -> the shake.
    fn punch01(&mut self) {
        self.anim(Bp::Tate, "BIG_PUNCH", Slot::CameraAnimSp, On::Sora, AnimStart { anim: "100_PUNCH01", blend_in: tr(0.3, 2, true), auto_end: true, end: tr(0.3, 3, true), end_takeover: true });
        self.later(Bp::Tate, Wait::Time(1.0), Then::PunchShake);
    }

    fn tate_start(&mut self, seq: &str, id: i32, frame: &FormBpFrame, world: Option<&dyn World>) {
        let sora = frame.sora;
        match (seq, id) {
            ("B418", _) | ("B426", _) if !self.perf(frame) => self.log.push(format!("{seq}: no performance camera")),
            ("B418", 0) => {
                self.atari = false;
                let anim = if BIG_PUNCH_TIGHT.iter().any(|p| hit(world, sora, *p)) { "BIG_PUNCH_NG" } else { "BIG_PUNCH" };
                self.anim(Bp::Tate, "BIG_PUNCH", Slot::BigPunch, On::Sora, AnimStart { anim, blend_in: tr(0.3, 2, true), auto_end: true, end: tr(0.3, 3, true), end_takeover: true });
            }
            ("B418", 2) => {
                if BIG_PUNCH02_BLOCK.iter().any(|p| hit(world, sora, *p)) {
                    self.log.push("B418 ID 2: a trace hit (@42643 / @43059): no BIG_PUNCH02".to_string());
                    return;
                }
                self.end_slots(&[Slot::BigPunch], "@9215 RV5.EndTresCamera");
                self.anim(Bp::Tate, "BIG_PUNCH02", Slot::CameraAnimSp, On::Sora, AnimStart { anim: "BIG_PUNCH02", blend_in: tr(0.0, 2, false), auto_end: false, end: tr(0.0, 3, false), end_takeover: false });
                self.atari = true;
                self.speed(0.1, 0.08, "B418 ID 2 SetGlobalGameSpeed @3092");
            }
            ("B426", 0) => {
                self.atari = false;
                self.punch01();
            }
            ("B426", 2) => {
                let free = world.is_none_or(|w| !PUNCH02_FREE.iter().any(|p| w.overlap_sphere(sora.0 + turn(*p, sora.1), PUNCH02_RADIUS)));
                if !free {
                    self.log.push("B426 ID 2: IsOverlapTresCameraActor: no 100_PUNCH02".to_string());
                    return;
                }
                self.end_slots(&[Slot::CameraAnimSp], "@4476 Camera Anim.EndTresCamera");
                self.anim(Bp::Tate, "BIG_PUNCH02", Slot::CameraAnimSp, On::Sora, AnimStart { anim: "100_PUNCH02", blend_in: tr(0.0, 2, false), auto_end: false, end: tr(0.0, 3, false), end_takeover: false });
                self.shake("B426 ID 2 @4232");
                self.atari = true;
            }
            ("B426", 4) => self.speed(0.05, 0.2, "B426 ID 4 SetGlobalGameSpeed @42048"),
            ("B417", _) => {
                // @6262: RotTarget "none", the camera's rotation now, no yaw of his; bTakeover {0.2, 4, 2}; FOV 110 in 0.2 s
                let rotation = self.last.unwrap_or(frame.field).rotation;
                let mut c = Self::orbit("none", Slot::ComboRot, RotParam { distance: B417_DISTANCE, rotation, local_offset: B417_LOCAL }, YawInfluence::None);
                if let Kind::Orbit { fov, .. } = &mut c.kind {
                    fov.set(B417_FOV.0, B417_FOV.1);
                }
                self.push(c, Transition::new(0.2, 4, 2.0, false));
            }
            ("B423", _) => {
                let mut c = Self::orbit("none", Slot::B423Rot, B423_ROT, YawInfluence::Once);
                c.wall_pawn = self.pawn_at.is_some();
                self.push(c, Transition::new(0.3, 4, 2.0, false));
                self.later(Bp::Tate, Wait::Time(3.0), Then::B423End);
            }
            ("B424", _) => {
                self.b424 = Some(0.0);
                if self.pawn_at.is_none() {
                    self.log.push("B424: no Tres Char Pawn base (the specific action 0 of 38 came first in KH3): at Sora".to_string());
                }
                if self.anim(Bp::Tate, "SH_tate_End", Slot::TateEnd, On::Pawn, AnimStart { anim: "SH_tate_cut02", blend_in: tr(0.0, 0, false), auto_end: true, end: tr(0.4, 0, true), end_takeover: true }).is_some() {
                    self.on = true;
                    self.later(Bp::Tate, Wait::Cycles(1), Then::TateEndWait);
                }
            }
            _ => {}
        }
    }

    fn tate_end(&mut self, seq: &str, id: i32) {
        match (seq, id) {
            ("B418", 2) => self.end_slots(&[Slot::CameraAnimSp], "@41775 Camera Anim.EndTresCamera"),
            ("B418", 3) if self.atari => self.speed(1.0, 0.2, "B418 ID 3 end SetGlobalGameSpeed @41272"),
            ("B426", 2) => {
                self.end_slots(&[Slot::CameraAnimSp], "@41859 Camera Anim.EndTresCamera");
                self.shakes.0.clear();
                self.log.push(format!("ClientStopCameraShake({BIBIBI}, immediately)"));
                self.atari = false;
                self.punch01();
            }
            ("B426", 4) => self.speed(1.0, 0.01, "B426 ID 4 end ResetGlobalGameSpeed @42094"),
            ("B417", _) => {
                let id = self.slot_id(Slot::ComboRot);
                self.end_where(Transition::new(0.4, 0, 2.0, false), true, "@6911 End Tres Camera Normal(Camera_Rot, takeover, 0.4)", |c| Some(c.id) == id);
            }
            _ => {}
        }
    }

    fn then(&mut self, then: Then) {
        match then {
            Then::PunchShake => {
                self.shake("@2493 Delay(1)");
                self.atari = true;
            }
            Then::B423End => {
                let id = self.slot_id(Slot::B423Rot);
                self.end_where(Transition::new(2.0, 4, 2.0, true), true, "@2434 End Tres Camera Normal(RV3, takeover, 2)", |c| Some(c.id) == id);
            }
            Then::TateEndWait => {
                if let Some(id) = self.slot_id(Slot::TateEnd) {
                    self.later(Bp::Tate, Wait::PlayEnd(id), Then::TateEndPlayed);
                }
            }
            Then::TateEndPlayed => self.later(Bp::Tate, Wait::Cycles(1), Then::TateDelay2),
            Then::TateDelay2 => self.later(Bp::Tate, Wait::Time(2.0), Then::Cut02),
            Then::Cut02 if self.on => {
                let c = Self::orbit("CUt02", Slot::Rot2, CUT02_ROT, YawInfluence::Once);
                self.push(c, CUT);
                self.later(Bp::Tate, Wait::Time(0.1), Then::Cut02Move);
            }
            Then::Cut02Move => {
                let (d, r, t) = CUT02_MOVE;
                if let Some(Kind::Orbit { distance, rotation, .. }) = self.cams.iter_mut().rev().find(|c| c.slot == Slot::Rot2).map(|c| &mut c.kind) {
                    distance.set(d, t);
                    for (ramp, to) in rotation.iter_mut().zip([r.pitch, r.yaw, r.roll]) {
                        ramp.set(to, t);
                    }
                }
                self.later(Bp::Tate, Wait::Time(t), Then::Cut03);
            }
            Then::Cut03 => {
                self.end_slots(&[Slot::Rot2], "@1401 CUt02 EndTresCamera");
                if self.on {
                    let c = Self::orbit("Cut03", Slot::Rot3, CUT03_ROT, YawInfluence::Once);
                    self.push(c, CUT);
                    self.later(Bp::Tate, Wait::Time(2.0), Then::Cut03End);
                }
            }
            Then::Cut03End => self.end_slots(&[Slot::Rot3], "@2085 Cut03 EndTresCamera"),
            Then::LemyPlayed => {
                if std::mem::take(&mut self.lemy_on) {
                    self.pending.reset_field_camera = true;
                    self.log.push("Lemylkey @27: ResetTresCameraExceptActive".to_string());
                }
            }
            Then::Cut02 => {}
        }
    }

    // ---------------------------------------------------------------- Lemylkey

    fn lemy_start(&mut self, seq: &str, frame: &FormBpFrame, world: Option<&dyn World>) {
        let sora = frame.sora;
        let (anim, blend) = match seq {
            "J423" => {
                if LEMY_FINISH_BLOCK.iter().any(|p| hit(world, sora, *p)) {
                    self.log.push("J423: a trace hit (@5209 / @3606): no camera".to_string());
                    return;
                }
                if hit(world, sora, LEMY_FINISH_TIGHT) {
                    ("LemyKey_Finish_cam_Atari", 0.33)
                } else {
                    ("LemyKey_Finish_Cam", 0.0)
                }
            }
            "J401" if !self.perf(frame) => {
                self.log.push("J401: no performance camera: RemoteEvent(Cnage_Key_Cancel) @154".to_string());
                return;
            }
            "J401" => {
                if hit(world, sora, LEMY_CHANGE_BLOCK) {
                    self.log.push("J401: the trace @12797 hit: no camera".to_string());
                    return;
                }
                ("LemyKey_Change_Cam", 0.0)
            }
            _ => return,
        };
        if let Some(id) = self.anim(Bp::Lemy, "LemyKey_Finish_Cam", Slot::Lemy, On::Sora, AnimStart { anim, blend_in: tr(blend, 4, true), auto_end: true, end: tr(0.0, 4, true), end_takeover: false }) {
            self.lemy_on = true;
            if let Some(c) = self.cams.iter_mut().find(|c| c.id == id) {
                c.yaw = YawInfluence::Always;
                if let Kind::Anim { zero_frame, .. } = &mut c.kind {
                    *zero_frame = true;
                }
            }
            self.later(Bp::Lemy, Wait::PlayEnd(id), Then::LemyPlayed);
        }
    }

    // ---------------------------------------------------------------- events

    fn dtor(&mut self, from: StateId) {
        let s = from as i32;
        if matches!(s, 31 | 32 | 33 | 38 | 41 | 84) {
            self.end_where(CUT, false, "tate_finish dtor @30053: EndAllSameBPTresCamera", |c| c.bp == Bp::Tate);
            self.latent.retain(|(bp, ..)| *bp != Bp::Tate);
        }
        if matches!(s, 31 | 32 | 33 | 38) {
            // @20180 / @20095: EndAllSameBPTresCamera (a cut), On = false; 31 / 33 / 38 also ResetTresCameraPrevious (0x140E63360:
            // the camera under the top one of the manager's stack, none with fewer than two: FUN_1405ECA30)
            let running = self.cams.iter().any(|c| c.bp == Bp::Lemy);
            self.end_where(CUT, false, "Lemylkey dtor @20095 / @20180: EndAllSameBPTresCamera", |c| c.bp == Bp::Lemy);
            self.lemy_on = false;
            if running && s != 32 {
                self.pending.reset_field_camera = true;
                self.log.push("Lemylkey dtor: ResetTresCameraPrevious".to_string());
            }
        }
        if s == 38 {
            self.b424 = None;
        }
    }

    fn ctor(&mut self, to: StateId) {
        if to == StateId::Damage {
            self.end_slots(&[Slot::ComboRot, Slot::TateStart, Slot::BigPunch], "tate_finish ctor 19 @51400");
            self.lemy_on = false;
            self.end_slots(&[Slot::Lemy], "Lemylkey ctor 19 @12448");
        }
    }

    /// One core event of the step (in order).
    fn on_event(&mut self, ev: &EventKind, frame: &FormBpFrame, world: Option<&dyn World>) {
        match *ev {
            EventKind::StateChange { from, to } => {
                self.dtor(from);
                self.ctor(to);
                self.state = Some(to);
            }
            EventKind::PlayerSpecificAction { state: StateId::StylefinishSh, action: 0, .. } => {
                self.pawn_at = Some(frame.sora);
                self.log.push("specific action 0 in 38: Tres Char Pawn base = GetSpecificActionAccompanyPawn(0); RemoteEvent(HS_tate_get_pos)".to_string());
            }
            EventKind::PlayerSpecificAction { action: 0x40, .. } => {
                self.log.push("specific action 64: Camera, Camera_Normal, Camera_Rot, Camera_Rot2, Camera_Anim, Camera_Rot3 end (cut); ON = false".to_string());
                self.end_slots(&[Slot::TateEnd, Slot::ComboRot, Slot::Rot2, Slot::TateStart, Slot::BigPunch, Slot::Rot3], "specific action 64 @15228");
                self.on = false;
            }
            EventKind::AnimBpEvent { seq, param, begin: true } => {
                self.tate_start(seq, param, frame, world);
                self.lemy_start(seq, frame, world);
            }
            EventKind::AnimBpEvent { seq, param, begin: false } => {
                if seq == "B424" {
                    self.b424 = None;
                }
                self.tate_end(seq, param);
            }
            _ => {}
        }
    }

    /// The latent actions whose condition came true before this step.
    fn run_latents(&mut self, dt: f32) {
        let mut due = Vec::new();
        let (cams, played) = (&self.cams, &self.played);
        self.latent.retain_mut(|(_, wait, then)| {
            let fire = match wait {
                Wait::Cycles(n) => {
                    *n = n.saturating_sub(1);
                    *n == 0
                }
                Wait::Time(t) => {
                    *t -= dt;
                    *t <= 1.0e-4
                }
                Wait::PlayEnd(id) => {
                    if played.contains(id) {
                        true
                    } else {
                        match cams.iter().find(|c| c.id == *id) {
                            Some(Cam { kind: Kind::Anim { def, time, .. }, .. }) => *time >= def.length,
                            // a camera another end destroyed: its latent action goes with it
                            _ => return false,
                        }
                    }
                }
            };
            if fire {
                due.push(*then);
            }
            !fire
        });
        self.played.clear();
        for t in due {
            self.then(t);
        }
    }

    /// Pegasus's actor (`Tres Char Pawn base`) now: Sora's actor in its frame is the ride link of M_B424 at B424's time.
    fn pawn(&self, sora: (Vec3, f32)) -> (Vec3, f32) {
        let link = self.b424.and_then(|t| kh_core::combat::forms::shield::ride_link("B424", t));
        match link {
            Some((_, _, s, s_yaw)) => {
                let yaw = sora.1 - s_yaw;
                (sora.0 - turn(s, yaw), yaw)
            }
            None => self.pawn_at.unwrap_or(sora),
        }
    }

    /// (FINISH-AUDIT2) The camera BASE on Pegasus: its MESH's place, not its actor's. The camera base 0x140621E00 takes the
    /// target actor's root location (+0x188 -> +0x2B0), unless the target is a TresCharPawnBase with
    /// `m_bCameraLookPosToMesh` (pawn+0x6E0 bit 0, reflection offset 1760; set only by its property setter 0x141D5B590) and
    /// `MyMesh` (pawn+0x580, the TresCharMesh made in the ctor 0x14059F950): then the mesh component's world location
    /// (+0x2B0 .. +0x2B8). StylePawn/w_so036 (Pegasus, `GetSpecificActionAccompanyPawn(0)`) sets the flag in its CDO, and its
    /// TresCharMesh0 sits at RelativeLocation (100, 0, 0): 100 cm ahead of its actor (kh-core shield.rs `PEGASUS_MESH_X`). The
    /// rotation stays the actor root's (0x140621FF0 reads +0x188 only). Sora (p_ex001_PawnBase) does not set the flag: his
    /// cameras keep his root (his feet).
    fn pawn_base(&self, pawn: (Vec3, f32)) -> (Vec3, f32) {
        if self.b424.and_then(|t| kh_core::combat::forms::shield::ride_link("B424", t)).is_some() {
            (pawn.0 + turn(Vec3::new(PEGASUS_MESH_X, 0.0, 0.0), pawn.1), pawn.1)
        } else {
            pawn
        }
    }

    fn view_of(&mut self, i: usize, sora: (Vec3, f32), world: Option<&dyn World>) -> AnimView {
        let pawn = self.pawn(sora);
        let base = self.pawn_base(pawn); // (FINISH-AUDIT2)
        let has_pawn = self.pawn_at.is_some();
        let c = &mut self.cams[i];
        let (loc, yaw) = match c.on {
            On::Sora => sora,
            On::Pawn => base, // (FINISH-AUDIT2) m_bCameraLookPosToMesh
        };
        match c.yaw {
            YawInfluence::Always => c.base_yaw = Some(yaw),
            YawInfluence::Once if c.base_yaw.is_none() => c.base_yaw = Some(yaw),
            YawInfluence::None => c.base_yaw = Some(0.0),
            YawInfluence::Once => {}
        }
        let by = c.base_yaw.unwrap_or(yaw);
        let (v, pivot, check) = match c.kind {
            Kind::Anim { def, time, .. } => {
                let s = def.sample(time);
                let v = AnimView { location: loc + turn(s.location, by), rotation: Rot { pitch: s.rotation.pitch, yaw: s.rotation.yaw + by, roll: s.rotation.roll }.normalized(), fov: s.fov };
                (v, loc, loc)
            }
            Kind::Orbit { distance, rotation, local_offset, fov } => {
                let look = loc + turn(local_offset, yaw);
                let r = Rot { pitch: rotation[0].value(), yaw: rotation[1].value() + by, roll: rotation[2].value() }.normalized();
                let v = AnimView { location: look - r.forward() * distance.value(), rotation: r, fov: fov.value() };
                let check = if c.wall_pawn && has_pawn { pawn.0 } else { look };
                (v, look, check)
            }
        };
        let Some(world) = world else { return v };
        match frame_target(&v, check, world) {
            Some(f) if f.distance > 0.0 => {
                let start = f.location + f.rotation.forward() * f.distance;
                AnimView { location: collide(start, f.location, world), rotation: f.rotation, ..v }
            }
            _ => AnimView { location: collide(sweep_start(&v, pivot), v.location, world), ..v },
        }
    }

    /// One step: the core's events of the step (in order), the command that began this step
    /// (`ReceivePlayerBeginCommandAction`, form_bp.rs), advance; the view over the cameras below (`frame.field`).
    pub fn step(&mut self, dt: f32, events: &[Event], frame: &FormBpFrame, begun_command: Option<i32>, world: Option<&dyn World>) -> KeyCamOutput {
        let sora = frame.sora;
        let below = frame.field;
        self.game_speed = None;
        self.run_latents(dt);
        for e in events {
            self.on_event(&e.kind, frame, world);
        }
        if begun_command == Some(CHANGE_SH) {
            self.change_sh(frame, world);
        }
        self.state = Some(frame.state);
        if let Some(t) = self.b424.as_mut() {
            *t += dt;
        }
        // AdvanceAnim (PlayZeroFrame: the first tick uses dt 0); bAutoEnd; the RotTargets' moves
        let mut ended = Vec::new();
        for c in &mut self.cams {
            match &mut c.kind {
                Kind::Anim { def, time, auto_end, end, end_takeover, zero_frame } => {
                    let step = if std::mem::take(zero_frame) { 0.0 } else { dt };
                    let before = *time;
                    *time = (*time + step).min(def.length);
                    if *auto_end && *time >= def.length && before < def.length {
                        ended.push((c.id, *end, *end_takeover));
                    }
                }
                Kind::Orbit { distance, rotation, fov, .. } => {
                    distance.elapsed += dt;
                    fov.elapsed += dt;
                    for r in rotation.iter_mut() {
                        r.elapsed += dt;
                    }
                }
            }
        }
        for (id, t, takeover) in ended {
            self.played.push(id);
            self.end_where(t, takeover, "autoEnd", |c| c.id == id);
        }
        let top = match self.cams.len() {
            0 => None,
            n => Some(self.view_of(n - 1, sora, world)),
        };
        let target_view = top.unwrap_or(below);
        let view = match self.blend.as_mut() {
            Some((from, t, elapsed)) => {
                *elapsed += dt;
                let a = t.alpha(*elapsed);
                let v = from.unwrap_or(below).blend(&target_view, a);
                if a >= 1.0 {
                    self.blend = None;
                }
                (top.is_some() || self.blend.is_some()).then_some(v)
            }
            None => top,
        };
        self.shakes.0.step(dt);
        self.last = Some(view.unwrap_or(below));
        let mut out = std::mem::take(&mut self.pending);
        out.view = view;
        out.shake = self.shakes.0.offset();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form_bp::PerfCamera;

    fn frame(state: StateId) -> FormBpFrame {
        FormBpFrame { sora: (Vec3::new(100.0, 0.0, 0.0), 30.0), field: AnimView { location: Vec3::new(-400.0, 0.0, 200.0), rotation: Rot::new(-10.0, 0.0, 0.0), fov: 90.0 }, state, ultima: false, perf: PerfCamera::default() }
    }

    fn bp(seq: &'static str, param: i32, begin: bool) -> Vec<Event> {
        vec![Event { step: 0, kind: EventKind::AnimBpEvent { seq, param, begin } }]
    }

    fn ev(kind: EventKind) -> Vec<Event> {
        vec![Event { step: 0, kind }]
    }

    fn run(c: &mut ShieldCameras, f: &FormBpFrame, steps: usize) -> KeyCamOutput {
        let mut out = KeyCamOutput::default();
        for _ in 0..steps {
            let o = c.step(1.0 / 60.0, &[], f, None, None);
            out.reset_field_camera |= o.reset_field_camera;
            out.takeover = out.takeover.or(o.takeover);
            out.view = o.view;
            out.shake = o.shake;
        }
        out
    }

    /// CHANGE_SH's command: SH_tate_start eases in over 0.3 s on Sora, ends by itself (2.333 s) with the takeover; Keyblade
    /// Cinematics Basic: none; STYLECHANGE's dtor cuts it.
    #[cfg(kh_party_team_cameras)]
    #[test]
    fn counter_shields_transformation_camera() {
        let mut c = ShieldCameras::default();
        let f = frame(StateId::Stylechange);
        let v = c.step(1.0 / 60.0, &[], &f, Some(CHANGE_SH), None).view.unwrap();
        assert_eq!(c.cameras(), ["SH_tate_start"]);
        assert!((v.location - f.field.location).length() < 50.0, "the 0.3 s ease starts at the field view: {v:?}");
        let out = run(&mut c, &f, 150);
        assert!(c.cameras().is_empty() && out.takeover.is_some(), "autoEnd at 2.333 s with bEndTakeover");
        let off = FormBpFrame { perf: PerfCamera { combo_cam: 0, keyblade_cinematics: false, restricted: false }, ..f };
        c.step(1.0 / 60.0, &[], &off, Some(CHANGE_SH), None);
        assert!(c.cameras().is_empty(), "Basic: RemoteEvent only");
        c.step(1.0 / 60.0, &[], &f, Some(CHANGE_SH), None);
        c.step(1.0 / 60.0, &ev(EventKind::StateChange { from: StateId::Stylechange, to: StateId::Idle }), &frame(StateId::Idle), None, None);
        assert!(c.cameras().is_empty(), "dtor 31 cuts");
    }

    /// B418 (0x55): BIG_PUNCH at ID 0, BIG_PUNCH02 cut in at ID 2 with the hit-stop x0.1, back x1 at ID 3's end, BIG_PUNCH02
    /// ended at ID 2's end; COMBO_SH's dtor ends the rest.
    #[cfg(kh_party_team_cameras)]
    #[test]
    fn the_big_punch_cameras_and_hit_stop() {
        let mut c = ShieldCameras::default();
        let f = frame(StateId::ComboSh);
        c.step(1.0 / 60.0, &bp("B418", 0, true), &f, None, None);
        assert_eq!(c.cameras(), ["BIG_PUNCH"]);
        c.step(1.0 / 60.0, &bp("B418", 3, true), &f, None, None);
        let o = c.step(1.0 / 60.0, &bp("B418", 2, true), &f, None, None);
        assert_eq!(c.cameras(), ["BIG_PUNCH02"]);
        assert!(o.view.is_some() && c.game_speed == Some((0.1, 0.08)));
        c.step(1.0 / 60.0, &bp("B418", 3, false), &f, None, None);
        assert_eq!(c.game_speed, Some((1.0, 0.2)));
        c.step(1.0 / 60.0, &bp("B418", 2, false), &f, None, None);
        assert!(c.cameras().is_empty());
        c.step(1.0 / 60.0, &bp("B418", 0, true), &f, None, None);
        c.step(1.0 / 60.0, &ev(EventKind::StateChange { from: StateId::ComboSh, to: StateId::Idle }), &frame(StateId::Idle), None, None);
        assert!(c.cameras().is_empty(), "dtor 84");
        // Combo Cam Off: none
        let off = FormBpFrame { perf: PerfCamera { combo_cam: 2, keyblade_cinematics: true, restricted: false }, ..f };
        c.step(1.0 / 60.0, &ev(EventKind::StateChange { from: StateId::Idle, to: StateId::ComboSh }), &off, None, None);
        c.step(1.0 / 60.0, &bp("B418", 0, true), &off, None, None);
        assert!(c.cameras().is_empty());
    }

    /// B426 (0x56): 100_PUNCH01, the shake one second later, 100_PUNCH02 at ID 2, x0.05 at ID 4, 100_PUNCH01 again at ID 2's end.
    #[cfg(kh_party_team_cameras)]
    #[test]
    fn the_hundred_punch_cameras() {
        let mut c = ShieldCameras::default();
        let f = frame(StateId::ComboSh);
        c.step(1.0 / 60.0, &bp("B426", 0, true), &f, None, None);
        assert_eq!(c.cameras(), ["100_PUNCH01"]);
        run(&mut c, &f, 47);
        c.step(1.0 / 60.0, &bp("B426", 2, true), &f, None, None);
        assert_eq!(c.cameras(), ["100_PUNCH02"]);
        let o = run(&mut c, &f, 14);
        assert!(o.shake != crate::shake::ShakeOffset::default(), "the shake of ID 2 and of the Delay(1)");
        assert!(c.log.iter().filter(|l| l.contains("shake shake_bibibi")).count() == 2, "{:?}", c.log);
        c.step(1.0 / 60.0, &bp("B426", 4, true), &f, None, None);
        assert_eq!(c.game_speed, Some((0.05, 0.2)));
        c.step(1.0 / 60.0, &bp("B426", 4, false), &f, None, None);
        assert_eq!(c.game_speed, Some((1.0, 0.01)));
        c.step(1.0 / 60.0, &bp("B426", 2, false), &f, None, None);
        assert_eq!(c.cameras(), ["100_PUNCH01"]);
    }

    /// B417: a RotTarget 600 cm along the view on screen, FOV to 110 in 0.2 s; its end hands over in 0.4 s.
    #[test]
    fn the_combo_finish_rot_target() {
        let mut c = ShieldCameras::default();
        let f = frame(StateId::ComboSh);
        c.step(1.0 / 60.0, &bp("B417", 0, true), &f, None, None);
        let v = run(&mut c, &f, 30).view.unwrap();
        assert_eq!(c.cameras(), ["none"]);
        assert!((v.rotation.pitch - f.field.rotation.pitch).abs() < 1e-3 && (v.rotation.yaw - f.field.rotation.yaw).abs() < 1e-3, "the camera's rotation, no yaw of his");
        let look = f.sora.0 + B417_LOCAL;
        assert!((v.location.distance(look) - B417_DISTANCE).abs() < 0.5 && (v.fov - 110.0).abs() < 0.5);
        let o = c.step(1.0 / 60.0, &bp("B417", 0, false), &f, None, None);
        assert!(c.cameras().is_empty() && o.takeover.is_some() && o.view.is_some(), "End Tres Camera Normal(takeover, 0.4)");
    }

    /// FINISH_SH: B423's RotTarget, SH_tate_cut02 on Pegasus at B424 (Sora 150 cm behind it), its end with the takeover, then
    /// 2 s later CUt02 (50 cm, moving 4 s), Cut03 (2 s); the specific action 64 ends them.
    #[cfg(kh_party_team_cameras)]
    #[test]
    fn the_ultimate_finish_cameras() {
        let mut c = ShieldCameras::default();
        let f = frame(StateId::StylefinishSh);
        c.step(1.0 / 60.0, &ev(EventKind::PlayerSpecificAction { state: StateId::StylefinishSh, action: 0, sub: 0 }), &f, None, None);
        c.step(1.0 / 60.0, &bp("B423", 0, true), &f, None, None);
        assert_eq!(c.cameras(), ["none"]);
        run(&mut c, &f, 15);
        let v = c.step(1.0 / 60.0, &bp("B424", 0, true), &f, None, None).view.unwrap();
        assert_eq!(c.cameras(), ["none", "SH_tate_cut02"]);
        let s = camera_anim("SH_tate_cut02").unwrap().sample(1.0 / 60.0);
        let (_, _, link, _) = kh_core::combat::forms::shield::ride_link("B424", 1.0 / 60.0).unwrap();
        let pegasus = f.sora.0 - turn(link, f.sora.1);
        // (FINISH-AUDIT2) based on Pegasus's MESH (m_bCameraLookPosToMesh): 100 cm ahead of its actor
        let mesh = pegasus + turn(Vec3::new(PEGASUS_MESH_X, 0.0, 0.0), f.sora.1);
        assert!((v.location - (mesh + turn(s.location, f.sora.1))).length() < 1.0, "on Pegasus's mesh: {v:?}");
        let mut took = false;
        let mut started = None;
        for i in 0..900 {
            let o = c.step(1.0 / 60.0, &[], &f, None, None);
            took |= o.takeover.is_some() && c.cameras().is_empty();
            if started.is_none() && c.cameras() == ["CUt02"] {
                started = Some(i);
            }
            if c.cameras() == ["Cut03"] {
                break;
            }
        }
        assert!(took, "SH_tate_cut02's end hands over (the B423 RotTarget ended at its Delay(3) under it)");
        let at = started.expect("CUt02");
        assert!((330..=345).contains(&at), "3.665 s + 2 cycles + 2 s after B424: {at}");
        assert_eq!(c.cameras(), ["Cut03"]);
        c.step(1.0 / 60.0, &ev(EventKind::PlayerSpecificAction { state: StateId::StylefinishSh, action: 0x40, sub: 0 }), &f, None, None);
        assert!(c.cameras().is_empty() && !c.on);
    }

    /// (FINISH-AUDIT2) CUt02 is on SORA (`BP_SettingTresCamera(GetLocalTresPlayerPawnBase, None, ..)`): his root (feet; p_ex001
    /// sets no m_bCameraLookPosToMesh), never Pegasus's mesh; `SetBaseRotation` (0x14060A640) eases the same +0x828 that
    /// SetTresCameraRotTargetParam (0x14060B9C0) wrote, so after 4 s the rotation is R(-25, 140) + his yaw at 100 cm.
    #[test]
    fn cut02_looks_from_soras_root_and_set_base_rotation_moves_its_rotation() {
        let mut c = ShieldCameras { on: true, ..ShieldCameras::default() };
        c.then(Then::Cut02);
        let f = frame(StateId::StylefinishSh);
        let v = c.step(1.0 / 60.0, &[], &f, None, None).view.unwrap();
        let look = f.sora.0 + turn(CUT02_ROT.local_offset, f.sora.1);
        assert!((v.location.distance(look) - CUT02_ROT.distance).abs() < 1.0, "50 cm from his root + local (50, 0, 0): {v:?}");
        let v = run(&mut c, &f, 4 * 60 + 3).view.unwrap();
        assert_eq!(c.cameras(), ["CUt02"]);
        assert!((v.rotation.pitch - CUT02_MOVE.1.pitch).abs() < 1.0 && (v.rotation.yaw - (CUT02_MOVE.1.yaw + f.sora.1)).abs() < 1.0, "{v:?}");
        assert!((v.location.distance(look) - CUT02_MOVE.0).abs() < 1.0, "{v:?}");
    }

    /// Lemylkey: J401's LemyKey_Change_Cam (perf), J423's LemyKey_Finish_Cam (always), its end resets the field camera;
    /// STYLEFINISH_SH's dtor cuts and resets.
    #[cfg(kh_party_team_cameras)]
    #[test]
    fn grand_chefs_cameras() {
        let mut c = ShieldCameras::default();
        let f = frame(StateId::Stylechange);
        c.step(1.0 / 60.0, &bp("J401", 0, true), &f, None, None);
        assert_eq!(c.cameras(), ["LemyKey_Change_Cam"]);
        let o = run(&mut c, &f, 260);
        assert!(c.cameras().is_empty() && o.reset_field_camera && o.takeover.is_none(), "autoEnd (cut), WaitPlayEnd -> ResetTresCameraExceptActive");
        let g = frame(StateId::StylefinishSh);
        let off = FormBpFrame { perf: PerfCamera { combo_cam: 2, keyblade_cinematics: false, restricted: false }, ..g };
        c.step(1.0 / 60.0, &bp("J423", 0, true), &off, None, None);
        assert_eq!(c.cameras(), ["LemyKey_Finish_Cam"], "no perf test for J423");
        let o = c.step(1.0 / 60.0, &ev(EventKind::StateChange { from: StateId::StylefinishSh, to: StateId::Idle }), &frame(StateId::Idle), None, None);
        assert!(c.cameras().is_empty() && o.reset_field_camera);
    }
}
