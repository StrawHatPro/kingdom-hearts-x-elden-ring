//! (AUDIT-A STAFF) MIRAGE STAFF'S CAMERA BLUEPRINT `Blueprints/Camera/p_ex001_StateEvent/change_key/
//! BP_TresPlayerStateEventCamera_Rapunzel_key` as data: the transformation's camera (A401, StyleChange10) and FINISH_WA's
//! (A417, Ever After's tower). Bytecode read 2026-11-03 with build/sora_tmp/party/tools/bpdump.js
//! (build/sora_tmp/weapons/AUDIT-A/STAFF/traces/bp_rapunzel_key_cam.txt); `@n` = the ubergraph's bytecode offset. The
//! CameraAnims and the shake are generated tables (tools/party_team_pack.js `--cameras`).
//!
//! # The events
//! - `ReceiveAnimNotifyStartBpEvent` (@43853, `IsLocalTresPlayerPawnBase(SendActor)`): "A401" -> @10414; "A481" ->
//!   `Rapunzel_ShootLock_camera` (@44261 -> @6226: the shotlock's own stick-driven CameraAnim, `BP_SetTresCameraAnimCtrl`:
//!   NOT PORTED here, a shotlock camera); "A417" -> `rap_key_finish_cam` (@44338).
//! - `ReceiveAnimNotifyEndBpEvent` (@44113): "A417" -> `Rap_key_Tower_Finish_End` (@44241).
//! - `ReceivePlayerSpecificAction` (@31001): ActionID 0 with `GetStateID() == 37` (TSID_STATE_STYLEFINISH_WA, @38069) ->
//!   @43568 `Tres Char Pawn base = GetSpecificActionAccompanyPawn(0)`: the tower (w_so073, TresStylePawnWATower; FINISH_WA's
//!   Tick 0x1413D7670 sub 0 calls pawn vt+0x1038(0, 0)). Other actions / states: nothing.
//! - `ReceivePlayerDtorState` (@22916): STYLECHANGE 31 / WEAPONCHANGE 32 -> @30723 `EndAllSameBPTresCamera({0, 3, 2})` (a
//!   cut); STYLEFINISH_WA 37 -> @29940 `TSUE_END = false`, `EndAllSameBPTresCamera` (a cut), `EndAllSameBPTresLatent`, then
//!   the tower's cleanup @30064 (below).
//! - `ReceivePlayerBeginCommandAction` (@12731): 162 (FINISH_WA 0xA2) -> @22904 `Atari = false`. `ReceivePlayerEndCommandAction`
//!   (@44365): no case is wired.
//!
//! # A401, the transformation (@10414)
//! `BP_TresIsGameEnablePerformanceCamera` (in STYLECHANGE the config "Keyblade Cinematics": form_bp.rs [`PerfCamera`]); off ->
//! @12674 `RemoteEvent("Cnage_Key_Cancel")` (the level script's: no listener in the port). Line traces (channel 1) from his
//! location + (0, 0, 100) to `TransformLocation(his transform, P)`: (200, 300, 120) @10714 or (-260, -150, 200) @11130 hit ->
//! nothing; else (400, 300, 180) @11546, (560, 190, 70) @12019, (630, -160, 140) @12439: any hit -> @11718 `Rapunzel_Start02`,
//! Atari = true, Blend = 0.3; all clear -> @12616 `Rapunzel_Start`, Atari = false, Blend = 0. @6882 "camera_anim" on Sora:
//! `EnableCollision(true)`, `SetPriority(200)`, `BP_AttachTresCameraTarget(false)`, `StartTresCameraAnim(anim, bLoop false,
//! bTakeover true, in {0.3, 4 ease in-out, 2, lock}, bAutoEnd true, bEndTakeover = Atari, end {Blend, 4, 2, lock})`,
//! `BP_SetTresCameraAnimPlayZeroFrame(true)`, `SetTresCameraTargetInfluenceRotation(bAlways true, 0, yaw)`: the yaw follows.
//! So the open-ground version cuts back at its end (4.167 s, or the dtor of STYLECHANGE), the cramped one blends back in 0.3 s
//! and hands its view to the field camera (bEndTakeover, `StaffCameras::takeover`).
//!
//! # A417, FINISH_WA (@44338)
//! `TSUE_END = true`, `Atari = false`, then a sequence (@43682):
//! 0. @43692 `GreaterEqual_FlagLabel(gameflow_TT, TT_TRM_EVENT01) && Less_FlagLabel(gameflow_TT, TT_TRM_MISSION02)` -> @8555
//!    Atari = true. PORT: the port has no story flags and its worlds are never that Twilight Town scene: false.
//! 1. @9955 the room test: his location -> `TransformLocation(his, (0, 0, 2500))` @10237, then from his location + (0, 0, 1000)
//!    to (1000, 1000, 2500) @8383, (1000, -1000, 2500) @8864, (-1000, 1000, 2500) @9323, (-1000, -1000, 2500) @9782: any hit ->
//!    Atari = true (no room above him: the cramped version, below).
//! 2. @2427 the Note: `BeginDeferredActorSpawnFromClass(Note, Tres Char Pawn base->GetTransform())`: an empty actor at the
//!    tower's root with its yaw (= his root and yaw when FINISH_WA began). Traces from his location + (0, 0, 100):
//!    (182.69, -120.15, 56.89) @2913 clear -> @5511 `Rap_Tower_Cut01`; hit: (-187.45, -121.07, 78.40) @3328 clear -> @5487
//!    `Rap_Tower_Cut00`; hit -> @3500 `Rap_Tower_Cut000`. @3519 "camera anim" ON THE NOTE (@3576 `if (false)` -> @5439
//!    `BP_SettingTresCamera(self.Actor)`): `EnableCollision(true)`, `StartTresCameraAnim(anim, false, bTakeover true, in {1.0, 4,
//!    2, no lock}, bAutoEnd false, false, end {0, 0, 2})` = canera_anim_0, `SetPriority(200)`, `WaitPlayEnd` -> @4075.
//!
//! @4075 (`TSUE_END`): Atari -> Collision = false and THE VOID (@4115 .. @4750: `SetActorVisibleOff` / `SetActorStopOn`
//! (channel 2), `vw_so073_fin_fsg0` at his location (`SpawnEmitterAtLocation`, R0), `DisableLight(true, 1)`,
//! "r.ReflectionEnvironment 0", `postprocess_Rapunzel_tower_C`): `StaffCameras::void`; else Collision = true. @4811 "camera
//! anim" on the Note: `EnableCollision(Collision)`, `Rap_Tower_Cut02`, in a cut {0, 4, 2}, no takeover, no autoEnd, end a cut
//! = canera_anim, priority 200, `WaitPlayEnd` -> @5535 (`TSUE_END`): `Rap_Finish_cam_End` (@30804:
//! `EndValidBPTresCameraArray([camera_rot, camera_rot2, canera_anim, canera_anim_0], {0, 3, 2})`, a cut) and @30939 -> @7628 the
//! `TresCameraRotTarget` "beam" ON SORA: `BP_AttachTresCameraTarget(false)`, `EnableCollision(Collision)`,
//! `SetTresCameraTargetInfluenceRotation(bAlways false)` (his yaw once), `SetTresCameraRotTargetParam(1000, R(80, 0, 0),
//! target offset 0, local offset (0, 0, -400))`, `StartTresCamera({0, 4, 2})` (a cut) = camera_rot, @7472 `SetPriority(200)`,
//! `SetFieldOfView(80, 0)`, `TresWaitCycle(1)` -> @5560 (`TSUE_END`): `SetDistance(2000, 0.3)` (priority 100, then 200 @6047),
//! `Delay(0.2)` -> @6161 (`TSUE_END`): `StopAllCameraShake`, the DoN(1) @15 -> @201 `ClientPlayCameraShake(
//! Shake_Rap_key_finish01, 1.5)`, @539 `Delay(0.5)` -> @616 (`TSUE_END`): `ClientStopCameraShake(Shake_Rap_key_finish01,
//! immediately)` (+ three of None), @958 the RotTarget "chakudan" on Sora: `EnableCollision(Collision)`, yaw once, (4000,
//! R(-25, 0, 0), target offset (0, 0, -1000), local 0), `StartTresCamera({0, 0, 2})` (a cut) = camera_rot2,
//! `SetPriority(200)`, `SetOutsideWallCheckType(4, V0, None, the tower)` (the cast to TresAccompanyPawnBase succeeds),
//! `StopAllCameraShake`, `TresWaitCycle(1)` -> @1943 (`TSUE_END`): `SetDistance(4500, 6)`, `SetBaseRotation(R(-30, -60, 0),
//! 6)`, `SetPriority(100)`.
//!
//! The end of A417's BpEvent (9.5 s) @44241 (`TSUE_END`) -> @30064: camera_rot and camera_rot2 `EndTresCamera({0, 3, 2})` (a
//! cut to the field camera); Atari: the void off (@30342 .. @30646: `SetActorVisibleOn` / `SetActorStopOff`, the particle
//! `FadeOut(0)`, `DisableLight(false)`, the Note and the post-process destroyed, "r.ReflectionEnvironment 1"), Atari = false.
//! (`SetForcedLOD(1)` @30107 and `(0)` two cycles later @6779: his mesh's LOD, nothing of the camera's.)
//!
//! # The camera stack
//! The manager (0x1405FB910) inserts a started camera by its priority (+0x3E8, 0x1405CEB40) in the frame it starts
//! (0x1405F59C0: before the first one of a higher priority: the newest of equal priority on top); `SetPriority` later
//! (0x1405D7840 writes +0x3E8 only) does not re-sort. All of these are inserted at 200 (over the field camera's 100), so the
//! chakudan's `SetPriority(100)` one cycle after its start leaves it on top of the beam.
//! The RotTarget is camera_anim.rs's orbit (look point = his location + his yaw x local offset + target offset; rotation =
//! `m_Rotation` + his yaw taken once; camera = look - forward x distance); `SetDistance` / `SetBaseRotation` start the type 5
//! ease-out (0x1405EFB60 / 0x14060A640, docs/sora-bossfinish-spec.md 3.3). The chakudan sets no FOV: forms.rs `NATIVE_FOV`
//! 90 [I: the native camera component's default, as forms.rs's ride camera]. [I] The outside-wall check type 4 (0x1406223B0
//! reads the actor's +0xB10 point, not traced on the tower): the tower's root here. `Delay` counts world seconds, a
//! `TresWaitCycle(n)` n steps; a latent action runs at the start of the step after its condition (a `WaitPlayEnd` sees the anim
//! that ended in the step before).
//!
//! THE EVENTS (AUDIT-C's path: kh-core `states/bp_event.rs` sends every `TresAnimNotifyState_BpEvent` window of the held
//! form's clips; [`crate::form_bp::FormBpCameras`] steps this blueprint with the same events and its frame, as it does
//! mi_key / fz_key / ca_key / UnionDiscode): `AnimBpEvent` A401 / A417 begin = ReceiveAnimNotifyStartBpEvent, A417's end =
//! ReceiveAnimNotifyEndBpEvent; `StateChange` = ReceivePlayerDtorState (the port's STYLEFINISH while FINISH_WA runs = KH3's
//! STYLEFINISH_WA 37); `PlayerSpecificAction { StylefinishWa, 0 }` (StyleFinishWA Tick sub 0 pawn vt+0x1038(0, 0), kh-core
//! `states/combo_wa.rs`) = ReceivePlayerSpecificAction: the tower, spawned that tick at Sora's root with his yaw, is
//! `Tres Char Pawn base` (its root and yaw = the frame's Sora at that event). The compose and the collision pass are
//! camera_anim.rs's.

use glam::Vec3;
use kh_core::host::{Event, EventKind};
use kh_core::{StateId, World};

use crate::camera_anim::{camera_anim, camera_shake, collide, frame_target, sweep_start, AnimView, CameraAnimDef, Transition};
use crate::form_bp::FormBpFrame;
use crate::shake::{ShakeOffset, ShakeSet};
use crate::Rot;

/// The CameraAnims (build/gen/rust/party_team_cameras.rs, `tools/party_team_pack.js --cameras`).
pub const START_ANIM: &str = "Rapunzel_Start";
pub const START02_ANIM: &str = "Rapunzel_Start02";
pub const TOWER_CUT000: &str = "Rap_Tower_Cut000";
pub const TOWER_CUT00: &str = "Rap_Tower_Cut00";
pub const TOWER_CUT01: &str = "Rap_Tower_Cut01";
pub const TOWER_CUT02: &str = "Rap_Tower_Cut02";
/// @201 `ClientPlayCameraShake(Shake_Rap_key_finish01_C, 1.5, ..)`.
pub const FINISH_SHAKE: &str = "Shake_Rap_key_finish01";
pub const FINISH_SHAKE_SCALE: f32 = 1.5;
/// The cramped version's void (@4296 / @4619).
pub const VOID_PARTICLE: &str = "vw_so073_fin_fsg0";
pub const VOID_POSTPROCESS: &str = "postprocess_Rapunzel_tower";

/// @7142 the transformation's in {0.3, ease in-out, 2, lock}.
pub const START_IN: Transition = Transition::new(0.3, 4, 2.0, true);
/// @3748 the tower cut's in {1.0, ease in-out, 2, no lock}.
pub const TOWER_IN: Transition = Transition::new(1.0, 4, 2.0, false);
/// @5048 / @7923 / @1310: a cut in.
pub const CUT_IN: Transition = Transition::new(0.0, 4, 2.0, false);
/// `EndAllSameBPTresCamera` / `EndTresCamera` / `EndValidBPTresCameraArray` {0, 3, 2}: a cut.
pub const CUT: Transition = Transition::new(0.0, 3, 2.0, false);
/// @3816 / @5116 the tower anims' end (never reached: no autoEnd; `Rap_Finish_cam_End` ends them).
pub const ANIM_END: Transition = Transition::new(0.0, 0, 2.0, false);

/// The traces start at his location + this (@10564 ...).
pub const TRACE_FROM: Vec3 = Vec3::new(0.0, 0.0, 100.0);
/// A401: either hit = no camera (@10714, @11130).
pub const CHANGE_BLOCK: [Vec3; 2] = [Vec3::new(200.0, 300.0, 120.0), Vec3::new(-260.0, -150.0, 200.0)];
/// A401: any hit = Rapunzel_Start02 (@11546, @12019, @12439).
pub const CHANGE_TIGHT: [Vec3; 3] = [Vec3::new(400.0, 300.0, 180.0), Vec3::new(560.0, 190.0, 70.0), Vec3::new(630.0, -160.0, 140.0)];
/// A417's room test: his location -> this (@10237) ...
pub const ROOM_UP: Vec3 = Vec3::new(0.0, 0.0, 2500.0);
/// ... then from his location + this to those (@8383, @8864, @9323, @9782).
pub const ROOM_FROM: Vec3 = Vec3::new(0.0, 0.0, 1000.0);
pub const ROOM_TO: [Vec3; 4] = [Vec3::new(1000.0, 1000.0, 2500.0), Vec3::new(1000.0, -1000.0, 2500.0), Vec3::new(-1000.0, 1000.0, 2500.0), Vec3::new(-1000.0, -1000.0, 2500.0)];
/// A417's cut choice (@2913: clear = Cut01; @3328: clear = Cut00, hit = Cut000).
pub const TOWER_TRACE: [Vec3; 2] = [Vec3::new(182.687_77, -120.150_36, 56.892_7), Vec3::new(-187.449_8, -121.073, 78.401)];

/// `SetTresCameraRotTargetParam(Distance, Rotation, TargetOffset, TargetLocalOffset)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotParam {
    pub distance: f32,
    pub rotation: Rot,
    pub target_offset: Vec3,
    pub local_offset: Vec3,
}

/// @7847 "beam", `SetFieldOfView(80, 0)` @7509; one cycle later `SetDistance(2000, 0.3)` @5614.
pub const BEAM: RotParam = RotParam { distance: 1000.0, rotation: Rot::new(80.0, 0.0, 0.0), target_offset: Vec3::ZERO, local_offset: Vec3::new(0.0, 0.0, -400.0) };
pub const BEAM_FOV: f32 = 80.0;
pub const BEAM_DISTANCE: (f32, f32) = (2000.0, 0.3);
/// @6084 `Delay(0.2)` to the shake, @539 `Delay(0.5)` to the chakudan.
pub const BEAM_SHAKE_DELAY: f32 = 0.2;
pub const CHAKUDAN_DELAY: f32 = 0.5;
/// @1234 "chakudan"; one cycle later `SetDistance(4500, 6)` @1997, `SetBaseRotation(R(-30, -60, 0), 6)` @2045.
pub const CHAKUDAN: RotParam = RotParam { distance: 4000.0, rotation: Rot::new(-25.0, 0.0, 0.0), target_offset: Vec3::new(0.0, 0.0, -1000.0), local_offset: Vec3::ZERO };
pub const CHAKUDAN_DISTANCE: (f32, f32) = (4500.0, 6.0);
pub const CHAKUDAN_ROTATION: (Rot, f32) = (Rot::new(-30.0, -60.0, 0.0), 6.0);
/// The RotTarget's FOV without `SetFieldOfView` [I].
pub const ROT_TARGET_FOV: f32 = crate::forms::NATIVE_FOV;

/// The sequences whose BpEvent windows this blueprint answers (A481 is the shotlock's, not this port's camera).
pub const SEQUENCES_ANSWERED: [&str; 2] = ["A401", "A417"];

/// The blueprint variable a camera is held in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    /// @6882's local (the transformation).
    Change,
    CanAnim0,
    CanAnim,
    Rot,
    Rot2,
}

/// `BP_SettingTresCamera(actor)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum On {
    Sora,
    /// The Note at the tower's transform.
    Note,
}

/// A type-5 ease-out move of a value (`SetDistance` / `SetBaseRotation`).
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
    /// `ATresCameraRotTarget`: distance, `m_Rotation` (pitch, yaw, roll), the offsets, the FOV.
    Orbit { distance: Ramp, rotation: [Ramp; 3], target_offset: Vec3, local_offset: Vec3, fov: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Cam {
    id: u32,
    name: &'static str,
    slot: Slot,
    on: On,
    kind: Kind,
    collision: bool,
    yaw_always: bool,
    base_yaw: Option<f32>,
    /// `SetOutsideWallCheckType(4, .., the tower)`.
    wall_tower: bool,
}

/// A latent continuation of the blueprint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Then {
    /// @4075 (WaitPlayEnd of canera_anim_0).
    TowerCutEnd,
    /// @5535 (WaitPlayEnd of canera_anim).
    Cut02End,
    /// @5560 (TresWaitCycle(1) after the beam).
    BeamCycle,
    /// @6161 (Delay(0.2)).
    BeamShake,
    /// @616 (Delay(0.5)).
    Chakudan,
    /// @1943 (TresWaitCycle(1) after the chakudan).
    ChakudanCycle,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Wait {
    Cycles(u32),
    Time(f32),
    PlayEnd(Slot),
}

/// The shake player (compared by whether one plays: `ShakeSet` has no equality).
#[derive(Clone, Debug, Default)]
struct Shakes(ShakeSet);

impl PartialEq for Shakes {
    fn eq(&self, other: &Shakes) -> bool {
        self.0.is_empty() == other.0.is_empty()
    }
}

/// The blueprint's state (one per Sora).
#[derive(Clone, Debug, PartialEq)]
pub struct StaffCameras {
    cams: Vec<Cam>,
    next_id: u32,
    /// The blend in progress: from (None = the live view below), the transition, the time.
    blend: Option<(Option<AnimView>, Transition, f32)>,
    last: Option<AnimView>,
    latent: Vec<(Wait, Then)>,
    shakes: Shakes,
    /// The pawn's state as KH3 reports it (`GetStateID`: STYLEFINISH while FINISH_WA runs = STYLEFINISH_WA 37).
    state: Option<StateId>,
    finish_wa: bool,
    /// `Tres Char Pawn base` (the tower) and `self.Actor` (the Note): root and yaw.
    pawn: Option<(Vec3, f32)>,
    note: Option<(Vec3, f32)>,
    tsue_end: bool,
    atari: bool,
    collision: bool,
    /// This step's camera shake (form_bp takes it into its output).
    pub shake: ShakeOffset,
    /// bEndTakeover: the field camera takes over this view (hosts take it).
    pub takeover: Option<AnimView>,
    /// The cramped version's void began (true) / ended (false) (hosts take it).
    pub void: Option<bool>,
    /// What the blueprint did (tests, a host's log).
    pub log: Vec<String>,
}

impl Default for StaffCameras {
    fn default() -> StaffCameras {
        StaffCameras { cams: Vec::new(), next_id: 0, blend: None, last: None, latent: Vec::new(), shakes: Shakes(ShakeSet::new()), state: None, finish_wa: false, pawn: None, note: None, tsue_end: false, atari: false, collision: false, shake: ShakeOffset::default(), takeover: None, void: None, log: Vec::new() }
    }
}

/// The view the blueprint shows this step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaffView {
    pub view: AnimView,
    /// The top camera's name ("camera_anim", "beam" ...), None while it blends back to the field camera.
    pub camera: Option<&'static str>,
    /// This step's view does not continue the last one.
    pub cut: bool,
}

fn turn(v: Vec3, yaw: f32) -> Vec3 {
    let (s, c) = yaw.to_radians().sin_cos();
    Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
}

/// `BP_TresLineTraceByChannel(location + from, TransformLocation(transform, to), channel 1)` returned true (no level: clear).
fn hit(world: Option<&dyn World>, at: (Vec3, f32), from: Vec3, to: Vec3) -> bool {
    world.is_some_and(|w| w.line_trace(at.0 + from, at.0 + turn(to, at.1)).is_some())
}

impl StaffCameras {
    /// A camera of the blueprint is on (or blending out).
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

    /// The top camera's anim (or name) and its time / distance.
    pub fn running(&self) -> Option<(&'static str, f32)> {
        self.cams.last().map(|c| match c.kind {
            Kind::Anim { def, time, .. } => (def.name, time),
            Kind::Orbit { distance, .. } => (c.name, distance.value()),
        })
    }

    fn push(&mut self, mut cam: Cam, transition: Transition) {
        cam.id = self.next_id;
        self.next_id += 1;
        self.log.push(format!("start {} {}", cam.name, cam_label(&cam)));
        // the outgoing view: frozen when locked or when it is one of ours, else the live camera below
        let from = if transition.lock_outgoing || !self.cams.is_empty() { self.last } else { None };
        self.blend = (transition.time > 0.0).then_some((from, transition, 0.0));
        // all of them at priority 200: the newest on top (the manager's insertion, 0x1405F59C0)
        self.cams.push(cam);
    }

    fn anim(&mut self, name: &'static str, slot: Slot, on: On, anim: &'static str) -> Option<Cam> {
        let Some(def) = camera_anim(anim) else {
            self.log.push(format!("{anim} is not in the generated camera tables (node tools/party_team_pack.js --cameras)"));
            return None;
        };
        Some(Cam { id: 0, name, slot, on, kind: Kind::Anim { def, time: 0.0, auto_end: false, end: ANIM_END, end_takeover: false, zero_frame: false }, collision: true, yaw_always: false, base_yaw: None, wall_tower: false })
    }

    fn orbit(name: &'static str, slot: Slot, p: RotParam, fov: f32, collision: bool) -> Cam {
        let rotation = [Ramp::fixed(p.rotation.pitch), Ramp::fixed(p.rotation.yaw), Ramp::fixed(p.rotation.roll)];
        Cam { id: 0, name, slot, on: On::Sora, kind: Kind::Orbit { distance: Ramp::fixed(p.distance), rotation, target_offset: p.target_offset, local_offset: p.local_offset, fov }, collision, yaw_always: false, base_yaw: None, wall_tower: false }
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
                self.takeover = self.last;
            }
        }
    }

    fn end_slots(&mut self, slots: &[Slot], why: &str) {
        self.end_where(CUT, false, why, |c| slots.contains(&c.slot));
    }

    /// @30064: the tower's cleanup (the rot cameras end; the void off).
    fn cleanup(&mut self, why: &str) {
        self.end_slots(&[Slot::Rot, Slot::Rot2], why);
        if self.atari {
            self.atari = false;
            self.void = Some(false);
            self.log.push(format!("the void ends ({VOID_PARTICLE} FadeOut(0), lights and reflections back, {VOID_POSTPROCESS} destroyed)"));
        }
    }

    fn dtor(&mut self, from: StateId) {
        match from {
            StateId::Stylechange | StateId::Weaponchange => self.end_where(CUT, false, "dtor 31 / 32: EndAllSameBPTresCamera @30723", |_| true),
            StateId::StylefinishWa => {
                self.tsue_end = false;
                self.end_where(CUT, false, "dtor 37: EndAllSameBPTresCamera @29951", |_| true);
                self.latent.clear();
                self.cleanup("dtor 37 @30064");
            }
            _ => {}
        }
    }

    /// A401 @10414 (the perf test with the pawn's state at the event).
    fn change_start(&mut self, frame: &FormBpFrame, world: Option<&dyn World>) {
        let sora = frame.sora;
        if !frame.perf.enabled(self.state.unwrap_or(frame.state)) {
            self.log.push("A401: no performance camera: RemoteEvent(Cnage_Key_Cancel) @12674".to_string());
            return;
        }
        if CHANGE_BLOCK.iter().any(|p| hit(world, sora, TRACE_FROM, *p)) {
            self.log.push("A401: a trace hit (@10714 / @11130): no camera".to_string());
            return;
        }
        let tight = CHANGE_TIGHT.iter().any(|p| hit(world, sora, TRACE_FROM, *p));
        let (anim, blend) = if tight { (START02_ANIM, 0.3) } else { (START_ANIM, 0.0) };
        self.atari = tight;
        let Some(mut c) = self.anim("camera_anim", Slot::Change, On::Sora, anim) else { return };
        if let Kind::Anim { auto_end, end, end_takeover, zero_frame, .. } = &mut c.kind {
            *auto_end = true;
            *end = Transition::new(blend, 4, 2.0, true);
            *end_takeover = tight;
            *zero_frame = true;
        }
        c.yaw_always = true;
        self.push(c, START_IN);
    }

    /// A417 @44338.
    fn finish_start(&mut self, sora: (Vec3, f32), world: Option<&dyn World>) {
        self.tsue_end = true;
        self.atari = false;
        // 0. the gameflow_TT window: never in the port. 1. the room test.
        if hit(world, sora, Vec3::ZERO, ROOM_UP) || ROOM_TO.iter().any(|p| hit(world, sora, ROOM_FROM, *p)) {
            self.atari = true;
            self.log.push("A417: no room above him: Atari (the cramped version)".to_string());
        }
        // 2. the Note at the tower's transform, the first cut
        if self.pawn.is_none() {
            self.log.push("A417: no tower (the specific action 0 of 37 came first in KH3): the Note at his transform".to_string());
        }
        self.note = Some(self.pawn.unwrap_or(sora));
        let anim = if !hit(world, sora, TRACE_FROM, TOWER_TRACE[0]) {
            TOWER_CUT01
        } else if !hit(world, sora, TRACE_FROM, TOWER_TRACE[1]) {
            TOWER_CUT00
        } else {
            TOWER_CUT000
        };
        if let Some(c) = self.anim("camera anim", Slot::CanAnim0, On::Note, anim) {
            self.push(c, TOWER_IN);
            self.latent.push((Wait::PlayEnd(Slot::CanAnim0), Then::TowerCutEnd));
        }
    }

    fn then(&mut self, then: Then) {
        if !self.tsue_end {
            return;
        }
        match then {
            Then::TowerCutEnd => {
                self.collision = !self.atari;
                if self.atari {
                    self.void = Some(true);
                    self.log.push(format!("the void begins (channel 2 hidden and stopped, {VOID_PARTICLE} at his location, lights off, reflections 0, {VOID_POSTPROCESS})"));
                }
                if let Some(mut c) = self.anim("camera anim", Slot::CanAnim, On::Note, TOWER_CUT02) {
                    c.collision = self.collision;
                    self.push(c, CUT_IN);
                    self.latent.push((Wait::PlayEnd(Slot::CanAnim), Then::Cut02End));
                }
            }
            Then::Cut02End => {
                self.end_slots(&[Slot::Rot, Slot::Rot2, Slot::CanAnim, Slot::CanAnim0], "Rap_Finish_cam_End @30804");
                let c = Self::orbit("beam", Slot::Rot, BEAM, BEAM_FOV, self.collision);
                self.push(c, CUT_IN);
                self.latent.push((Wait::Cycles(1), Then::BeamCycle));
            }
            Then::BeamCycle => {
                if let Some(Kind::Orbit { distance, .. }) = self.cams.iter_mut().find(|c| c.slot == Slot::Rot).map(|c| &mut c.kind) {
                    distance.set(BEAM_DISTANCE.0, BEAM_DISTANCE.1);
                }
                self.latent.push((Wait::Time(BEAM_SHAKE_DELAY), Then::BeamShake));
            }
            Then::BeamShake => {
                self.shakes.0.clear();
                if let Some(s) = camera_shake(FINISH_SHAKE) {
                    self.shakes.0.play(&s.def(), FINISH_SHAKE_SCALE, None);
                    self.log.push(format!("shake {FINISH_SHAKE} x{FINISH_SHAKE_SCALE}"));
                }
                self.latent.push((Wait::Time(CHAKUDAN_DELAY), Then::Chakudan));
            }
            Then::Chakudan => {
                self.shakes.0.clear();
                let mut c = Self::orbit("chakudan", Slot::Rot2, CHAKUDAN, ROT_TARGET_FOV, self.collision);
                c.wall_tower = self.pawn.is_some();
                self.push(c, CUT_IN);
                self.latent.push((Wait::Cycles(1), Then::ChakudanCycle));
            }
            Then::ChakudanCycle => {
                if let Some(Kind::Orbit { distance, rotation, .. }) = self.cams.iter_mut().find(|c| c.slot == Slot::Rot2).map(|c| &mut c.kind) {
                    distance.set(CHAKUDAN_DISTANCE.0, CHAKUDAN_DISTANCE.1);
                    let (r, t) = CHAKUDAN_ROTATION;
                    for (ramp, to) in rotation.iter_mut().zip([r.pitch, r.yaw, r.roll]) {
                        ramp.set(to, t);
                    }
                }
            }
        }
    }

    /// The latent actions whose condition came true before this step.
    fn run_latents(&mut self, dt: f32) {
        let mut due = Vec::new();
        let cams = &self.cams;
        self.latent.retain_mut(|(wait, then)| {
            let fire = match wait {
                Wait::Cycles(n) => {
                    *n = n.saturating_sub(1);
                    *n == 0
                }
                Wait::Time(t) => {
                    *t -= dt;
                    *t <= 1.0e-4
                }
                Wait::PlayEnd(slot) => match cams.iter().find(|c| c.slot == *slot) {
                    Some(Cam { kind: Kind::Anim { def, time, .. }, .. }) => *time >= def.length,
                    // a destroyed camera's latent action goes with it
                    _ => return false,
                },
            };
            if fire {
                due.push(*then);
            }
            !fire
        });
        for t in due {
            self.then(t);
        }
    }

    /// One core event of the step (in order), with the step's frame.
    pub fn on_event(&mut self, ev: &EventKind, frame: &FormBpFrame, world: Option<&dyn World>) {
        match *ev {
            // ReceivePlayerDtorState (the state left, as KH3 numbers it), then the new one
            EventKind::StateChange { from, to } => {
                let old = if from == StateId::Stylefinish && self.finish_wa { StateId::StylefinishWa } else { from };
                if old == StateId::StylefinishWa {
                    self.finish_wa = false;
                }
                self.dtor(old);
                self.state = Some(to);
            }
            // ReceivePlayerSpecificAction(0) in 37: GetSpecificActionAccompanyPawn(0) = the tower (at Sora's root this tick).
            // The FINISH_WA command began before it (ReceivePlayerBeginCommandAction(162) @22904: Atari = false).
            EventKind::PlayerSpecificAction { state: StateId::StylefinishWa, action: 0, .. } => {
                if !self.finish_wa {
                    self.atari = false;
                }
                self.finish_wa = true;
                self.state = Some(StateId::StylefinishWa);
                self.pawn = Some(frame.sora);
                self.log.push("specific action 0 in 37: Tres Char Pawn base = the tower".to_string());
            }
            EventKind::AnimBpEvent { seq: "A401", begin: true, .. } => self.change_start(frame, world),
            EventKind::AnimBpEvent { seq: "A417", begin: true, .. } => self.finish_start(frame.sora, world),
            // ReceiveAnimNotifyEndBpEvent "A417" @44113 -> Rap_key_Tower_Finish_End @44241 (TSUE_END)
            EventKind::AnimBpEvent { seq: "A417", begin: false, .. } if self.tsue_end => self.cleanup("end of A417: Rap_key_Tower_Finish_End @44241"),
            _ => {}
        }
    }

    /// The view of a camera now, with its collision pass (no level: none).
    fn view_of(&mut self, i: usize, sora: (Vec3, f32), world: Option<&dyn World>) -> AnimView {
        let note = self.note.unwrap_or(sora);
        let pawn = self.pawn;
        let c = &mut self.cams[i];
        let (loc, yaw) = match c.on {
            On::Sora => sora,
            On::Note => note,
        };
        if c.base_yaw.is_none() || c.yaw_always {
            c.base_yaw = Some(yaw);
        }
        let by = c.base_yaw.unwrap_or(yaw);
        let (v, pivot, check) = match c.kind {
            Kind::Anim { def, time, .. } => {
                let s = def.sample(time);
                let v = AnimView { location: loc + turn(s.location, by), rotation: Rot { pitch: s.rotation.pitch, yaw: s.rotation.yaw + by, roll: s.rotation.roll }.normalized(), fov: s.fov };
                (v, loc, loc)
            }
            Kind::Orbit { distance, rotation, target_offset, local_offset, fov } => {
                let look = loc + turn(local_offset, yaw) + target_offset;
                let r = Rot { pitch: rotation[0].value(), yaw: rotation[1].value() + by, roll: rotation[2].value() }.normalized();
                let v = AnimView { location: look - r.forward() * distance.value(), rotation: r, fov };
                let check = match (c.wall_tower, pawn) {
                    (true, Some(p)) => p.0,
                    _ => look,
                };
                (v, look, check)
            }
        };
        let Some(world) = world.filter(|_| c.collision) else { return v };
        match frame_target(&v, check, world) {
            Some(f) if f.distance > 0.0 => {
                let start = f.location + f.rotation.forward() * f.distance;
                AnimView { location: collide(start, f.location, world), rotation: f.rotation, ..v }
            }
            _ => AnimView { location: collide(sweep_start(&v, pivot), v.location, world), ..v },
        }
    }

    /// One step: `events` = the core's events of the step (in order); `frame` = Sora (actor location, yaw), the view below
    /// (the field / team cameras: blends from / to it), his state, the perf config; `world` for the blueprint's traces and
    /// the collision pass. None = nothing of the blueprint is shown. Its shake is in `shake`, bEndTakeover in `takeover`, the
    /// cramped finish's void in `void`.
    pub fn step(&mut self, dt: f32, events: &[Event], frame: &FormBpFrame, world: Option<&dyn World>) -> Option<StaffView> {
        let sora = frame.sora;
        let below = frame.field;
        let top_before = self.cams.last().map(|c| c.id);
        self.run_latents(dt);
        for e in events {
            self.on_event(&e.kind, frame, world);
        }
        self.state = Some(match frame.state {
            StateId::Stylefinish if self.finish_wa => StateId::StylefinishWa,
            s => s,
        });
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
                Kind::Orbit { distance, rotation, .. } => {
                    distance.elapsed += dt;
                    for r in rotation.iter_mut() {
                        r.elapsed += dt;
                    }
                }
            }
        }
        for (id, t, takeover) in ended {
            self.end_where(t, takeover, "autoEnd", |c| c.id == id);
        }
        let top = match self.cams.len() {
            0 => None,
            n => Some((self.view_of(n - 1, sora, world), self.cams[n - 1].name)),
        };
        let top_now = self.cams.last().map(|c| c.id);
        let target_view = top.map_or(below, |t| t.0);
        let view = match self.blend.as_mut() {
            Some((from, t, elapsed)) => {
                *elapsed += dt;
                let a = t.alpha(*elapsed);
                let v = from.unwrap_or(below).blend(&target_view, a);
                if a >= 1.0 {
                    self.blend = None;
                }
                // a finished blend back to the camera below: nothing of ours any more
                (top.is_some() || self.blend.is_some()).then_some(v)
            }
            None => top.map(|t| t.0),
        };
        self.shakes.0.step(dt);
        self.shake = self.shakes.0.offset();
        let cut = top_now != top_before && top_now.is_some() && self.blend.is_none();
        let result = view.map(|v| StaffView { view: v, camera: top.map(|t| t.1), cut });
        self.last = Some(result.map_or(below, |r| r.view));
        result
    }
}

fn cam_label(c: &Cam) -> String {
    match c.kind {
        Kind::Anim { def, .. } => def.name.to_string(),
        Kind::Orbit { distance, rotation, .. } => format!("RotTarget {} R({}, {}, {})", distance.value(), rotation[0].value(), rotation[1].value(), rotation[2].value()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form_bp::PerfCamera;
    use kh_core::anim::{NotifyClass, SEQUENCES};
    use kh_core::world::Hit;

    /// Open ground: no trace hits, no wall.
    struct Open;
    impl World for Open {
        fn sweep_capsule(&self, _: Vec3, _: Vec3, _: f32, _: f32) -> Option<Hit> {
            None
        }
        fn line_trace(&self, _: Vec3, _: Vec3) -> Option<Hit> {
            None
        }
    }

    /// The line traces ending at these points (world, 1 cm) hit; nothing else.
    struct Blocked(Vec<Vec3>);
    impl World for Blocked {
        fn sweep_capsule(&self, _: Vec3, _: Vec3, _: f32, _: f32) -> Option<Hit> {
            None
        }
        fn line_trace(&self, a: Vec3, b: Vec3) -> Option<Hit> {
            self.0.iter().any(|p| p.distance(b) < 1.0).then_some(Hit { time: 0.5, position: a, normal: Vec3::X, impact_normal: Vec3::X, start_penetrating: false, penetration_depth: 0.0, component: 0 })
        }
    }

    const DT: f32 = 1.0 / 60.0;

    fn below() -> AnimView {
        AnimView { location: Vec3::new(-400.0, 0.0, 200.0), rotation: Rot::new(-10.0, 0.0, 0.0), fov: 100.0 }
    }

    fn frame(state: StateId, at: Vec3, yaw: f32) -> FormBpFrame {
        FormBpFrame { sora: (at, yaw), field: below(), state, ultima: false, perf: PerfCamera::default() }
    }

    fn ev(kinds: &[EventKind]) -> Vec<Event> {
        kinds.iter().map(|k| Event { step: 0, kind: *k }).collect()
    }

    fn bp(seq: &'static str, begin: bool) -> EventKind {
        EventKind::AnimBpEvent { seq, param: 0, begin }
    }

    fn change(from: StateId, to: StateId) -> EventKind {
        EventKind::StateChange { from, to }
    }

    /// StyleFinishWA Tick sub 0's pawn vt+0x1038(0, 0).
    const TOWER: EventKind = EventKind::PlayerSpecificAction { state: StateId::StylefinishWa, action: 0, sub: 0 };

    fn tables() -> bool {
        let ok = [START_ANIM, START02_ANIM, TOWER_CUT000, TOWER_CUT00, TOWER_CUT01, TOWER_CUT02].iter().all(|a| camera_anim(a).is_some()) && camera_shake(FINISH_SHAKE).is_some();
        if !ok {
            eprintln!("SKIPPED: no generated camera tables");
        }
        ok
    }

    /// The BpEvent windows kh-core sends for these clips (A401 0 .. 3.70 s, A417 0 .. 9.5 s).
    #[test]
    fn the_blueprint_answers_a401_and_a417_only() {
        let window = |seq: &str| SEQUENCES.iter().find(|s| s.name == seq).and_then(|s| s.notifies.iter().find(|n| n.class == NotifyClass::TresAnimNotifyStateBpEvent).map(|n| (n.time, n.end)));
        let (b, e) = window("A401").unwrap();
        assert!(b == 0.0 && (e - 3.699_508_7).abs() < 1.0e-4);
        let (b, e) = window("A417").unwrap();
        assert!(b == 0.0 && (e - 9.5).abs() < 1.0e-4, "A417's BpEvent 0 .. 9.5 s");
        assert_eq!(SEQUENCES_ANSWERED, ["A401", "A417"]);
        // another sequence's event does nothing
        let mut c = StaffCameras::default();
        assert!(c.step(DT, &ev(&[bp("A481", true)]), &frame(StateId::Shootflow, Vec3::ZERO, 0.0), Some(&Open)).is_none());
        assert!(!c.active());
    }

    #[test]
    fn the_transformation_camera_in_the_open_cuts_back_at_its_end() {
        if !tables() {
            return;
        }
        let def = camera_anim(START_ANIM).unwrap();
        assert!((def.length - 4.166_667).abs() < 1.0e-3);
        let mut c = StaffCameras::default();
        let at = Vec3::new(100.0, 50.0, 0.0);
        assert!(c.step(DT, &[], &frame(StateId::Idle, at, 30.0), Some(&Open)).is_none());
        let first = c.step(DT, &ev(&[change(StateId::Idle, StateId::Stylechange), bp("A401", true)]), &frame(StateId::Stylechange, at, 30.0), Some(&Open)).expect("camera_anim");
        assert_eq!(c.cameras(), [START_ANIM]);
        assert!(!first.cut, "0.3 s ease in-out in, not a cut");
        // after 0.3 s: frame t (PlayZeroFrame: the first tick held frame 0) on his feet, turned by his yaw
        let mut v = first;
        for _ in 1..=18 {
            v = c.step(DT, &[], &frame(StateId::Stylechange, at, 30.0), Some(&Open)).unwrap();
        }
        let want = at + turn(def.sample(18.0 * DT).location, 30.0);
        assert!((v.view.location - want).length() < 1.0e-2, "{:?} vs {want:?}", v.view.location);
        // the yaw follows (SetTresCameraTargetInfluenceRotation(bAlways true))
        let turned = c.step(DT, &[], &frame(StateId::Stylechange, at, 120.0), Some(&Open)).unwrap();
        let want = at + turn(def.sample(19.0 * DT).location, 120.0);
        assert!((turned.view.location - want).length() < 1.0e-2);
        // autoEnd at 4.167 s with a cut, no takeover (the window's end at 3.70 s is not answered)
        let mut steps = 20;
        let mut end_sent = false;
        loop {
            let events = if !end_sent && steps as f32 * DT >= 3.7 {
                end_sent = true;
                ev(&[bp("A401", false)])
            } else {
                Vec::new()
            };
            if c.step(DT, &events, &frame(StateId::Stylechange, at, 120.0), Some(&Open)).is_none() {
                break;
            }
            steps += 1;
            assert!(steps < 400);
        }
        assert!((steps as f32 * DT - def.length).abs() < 2.0 * DT, "{steps} steps");
        assert!(c.takeover.is_none() && !c.active());
    }

    #[test]
    fn the_transformation_camera_needs_room_and_the_cramped_one_hands_over() {
        if !tables() {
            return;
        }
        let at = Vec3::ZERO;
        let start = ev(&[change(StateId::Idle, StateId::Stylechange), bp("A401", true)]);
        let f = frame(StateId::Stylechange, at, 0.0);
        // (200, 300, 120) blocked: no camera
        let mut c = StaffCameras::default();
        assert!(c.step(DT, &start, &f, Some(&Blocked(vec![CHANGE_BLOCK[0]]))).is_none());
        assert!(c.log.iter().any(|l| l.contains("no camera")));
        // the config off: Cnage_Key_Cancel
        let mut c = StaffCameras::default();
        let off = FormBpFrame { perf: PerfCamera { keyblade_cinematics: false, ..PerfCamera::default() }, ..f };
        assert!(c.step(DT, &start, &off, Some(&Open)).is_none());
        assert!(c.log.iter().any(|l| l.contains("Cnage_Key_Cancel")));
        // (560, 190, 70) blocked: Rapunzel_Start02, autoEnd with 0.3 s back and the field camera taking over
        let mut c = StaffCameras::default();
        let w = Blocked(vec![CHANGE_TIGHT[1]]);
        c.step(DT, &start, &f, Some(&w)).expect("camera_anim");
        assert_eq!(c.cameras(), [START02_ANIM]);
        let len = camera_anim(START02_ANIM).unwrap().length;
        let (mut blended, mut took) = (0, None);
        for _ in 1..((len + 0.6) / DT) as usize {
            if let Some(v) = c.step(DT, &[], &f, Some(&w)) {
                blended += usize::from(v.camera.is_none());
            }
            took = took.or(c.takeover.take());
        }
        assert!((16..=19).contains(&blended), "0.3 s of blend: {blended}");
        assert!(took.is_some() && !c.active());
        // leaving STYLECHANGE cuts it (ReceivePlayerDtorState 31)
        let mut c = StaffCameras::default();
        c.step(DT, &start, &f, Some(&Open)).expect("on");
        assert!(c.step(DT, &ev(&[change(StateId::Stylechange, StateId::Jump)]), &frame(StateId::Jump, at, 0.0), Some(&Open)).is_none());
        assert!(!c.active());
    }

    /// FINISH_WA's whole path on open ground: the tower cut on the Note (1 s in), Cut02 (a cut), the beam (a cut, FOV 80,
    /// 1000 -> 2000), the shake, the chakudan (a cut, 4000 -> 4500, R(-25, 0) -> R(-30, -60)), the end of A417 (a cut).
    #[test]
    fn the_finish_camera_follows_the_tower_blueprint() {
        if !tables() {
            return;
        }
        let tower = (Vec3::new(300.0, -200.0, 0.0), 45.0);
        let mut c = StaffCameras::default();
        assert!(c.step(DT, &[], &frame(StateId::Idle, tower.0, 45.0), Some(&Open)).is_none());
        let mut events: Vec<(usize, Option<&'static str>, Vec<&'static str>, bool)> = Vec::new();
        let mut cams_before: Vec<&'static str> = Vec::new();
        let mut shaken = None;
        let (cut01, cut02) = (camera_anim(TOWER_CUT01).unwrap(), camera_anim(TOWER_CUT02).unwrap());
        for k in 0..((9.5 / DT) as usize + 30) {
            let t = k as f32 * DT;
            // he rises with the tower (its socket), facing its way; the core's events of the step
            let at = tower.0 + Vec3::new(0.0, 0.0, (t * 300.0).min(2400.0));
            let sent = match k {
                0 => ev(&[change(StateId::Idle, StateId::Stylefinish), TOWER, bp("A417", true)]),
                _ if (t - 9.5).abs() < DT * 0.5 => ev(&[bp("A417", false)]),
                _ => Vec::new(),
            };
            let f = frame(StateId::Stylefinish, if k == 0 { tower.0 } else { at }, 45.0);
            let v = c.step(DT, &sent, &f, Some(&Open));
            if c.shake != ShakeOffset::default() && shaken.is_none() {
                shaken = Some(k);
            }
            let cams = c.cameras();
            if cams != cams_before {
                events.push((k, v.and_then(|v| v.camera), cams.clone(), v.is_some_and(|v| v.cut)));
                cams_before = cams;
            }
            if k == 0 {
                let v = v.expect("the tower cut");
                assert!(!v.cut, "1 s ease in-out from the field camera");
                assert!((v.view.location - below().location).length() < 10.0, "it starts at the view below");
            }
            if k == 60 {
                // 1 s in: the anim's frame on the Note (the tower's root and yaw, not Sora's rising location)
                let want = tower.0 + turn(cut01.sample(61.0 * DT).location, tower.1);
                assert!((v.unwrap().view.location - want).length() < 1.0, "{:?} vs {want:?}", v.unwrap().view.location);
            }
        }
        let names: Vec<&[&str]> = events.iter().map(|e| e.2.as_slice()).collect();
        assert_eq!(names, [&[TOWER_CUT01][..], &[TOWER_CUT01, TOWER_CUT02], &["beam"], &["beam", "chakudan"], &[]], "{events:?}");
        let at = |i: usize| events[i].0 as f32 * DT;
        assert!((at(1) - cut01.length).abs() < 3.0 * DT, "Cut02 after Cut01's {} s: {}", cut01.length, at(1));
        assert!(events[1].3 && events[2].3 && events[3].3, "Cut02, the beam and the chakudan cut in");
        assert!((at(2) - at(1) - cut02.length).abs() < 3.0 * DT, "the beam after Cut02's {} s", cut02.length);
        let shake_at = shaken.expect("the beam's shake") as f32 * DT;
        assert!((shake_at - at(2) - BEAM_SHAKE_DELAY).abs() < 3.0 * DT, "one cycle + 0.2 s: {}", shake_at - at(2));
        assert!((at(3) - shake_at - CHAKUDAN_DELAY).abs() < 3.0 * DT, "0.5 s later the chakudan");
        assert!((at(4) - 9.5).abs() < 2.0 * DT, "the end of A417 ends it: {}", at(4));
        assert!(c.log.iter().any(|l| l.contains("Tres Char Pawn base")));
        assert!(c.void.is_none(), "room above him: no void");
    }

    #[test]
    fn the_rot_target_cameras_place_like_kh3() {
        if !tables() {
            return;
        }
        let mut c = StaffCameras::default();
        let sora = Vec3::new(0.0, 0.0, 2000.0);
        let mut beam = None;
        let mut chakudan_end = None;
        for k in 0..((9.4 / DT) as usize) {
            let sent = if k == 0 { ev(&[change(StateId::Idle, StateId::Stylefinish), TOWER, bp("A417", true)]) } else { Vec::new() };
            let f = frame(StateId::Stylefinish, if k == 0 { Vec3::ZERO } else { sora }, 0.0);
            let v = c.step(DT, &sent, &f, Some(&Open));
            match v.and_then(|v| v.camera.map(|n| (n, v))) {
                Some(("beam", v)) if beam.is_none() => beam = Some(v),
                Some(("chakudan", v)) => chakudan_end = Some(v),
                _ => {}
            }
        }
        // the beam: 1000 below-behind looking up 80 degrees at a point 400 under him, FOV 80
        let b = beam.expect("beam").view;
        let look = sora + BEAM.local_offset;
        assert!((b.rotation.pitch - 80.0).abs() < 1.0e-3 && (b.fov - BEAM_FOV).abs() < 1.0e-6);
        assert!(((look - b.location).length() - 1000.0).abs() < 1.0e-2);
        // the chakudan near its end: 4000 -> 4500 and R(-25, 0) -> R(-30, -60) over 6 s (3.8 s of it shown)
        let e = chakudan_end.expect("chakudan").view;
        let look = sora + CHAKUDAN.target_offset;
        let d = (look - e.location).length();
        assert!(d > 4300.0 && d < 4500.0, "{d}");
        assert!(e.rotation.pitch < -28.0 && e.rotation.yaw < -45.0 && e.rotation.yaw > -60.0, "{:?}", e.rotation);
        assert!((e.fov - ROT_TARGET_FOV).abs() < 1.0e-6);
    }

    #[test]
    fn a_cramped_finish_takes_the_other_cuts_and_the_void() {
        if !tables() {
            return;
        }
        let sora = Vec3::ZERO;
        let start = ev(&[change(StateId::Idle, StateId::Stylefinish), TOWER, bp("A417", true)]);
        let f = frame(StateId::Stylefinish, sora, 0.0);
        // a ceiling (the trace straight up hits) and both cut-choice points blocked
        let w = Blocked(vec![sora + ROOM_UP, sora + TOWER_TRACE[0], sora + TOWER_TRACE[1]]);
        let mut c = StaffCameras::default();
        let mut voids = Vec::new();
        for k in 0..((9.6 / DT) as usize) {
            let t = k as f32 * DT;
            let sent = if k == 0 {
                start.clone()
            } else if (t - 9.5).abs() < DT * 0.5 {
                ev(&[bp("A417", false)])
            } else {
                Vec::new()
            };
            c.step(DT, &sent, &f, Some(&w));
            if k == 1 {
                assert_eq!(c.cameras(), [TOWER_CUT000]);
            }
            if let Some(v) = c.void.take() {
                voids.push(v);
            }
        }
        assert_eq!(voids, [true, false], "the void with Cut02, gone at the end of A417");
        // only the second point blocked: Cut01; only the first: Cut00
        let mut c = StaffCameras::default();
        c.step(DT, &start, &f, Some(&Blocked(vec![sora + TOWER_TRACE[0]])));
        assert_eq!(c.cameras(), [TOWER_CUT00]);
        // the dtor of 37 (the port's STYLEFINISH left while FINISH_WA ran) ends everything and its latent actions
        let mut c = StaffCameras::default();
        c.step(DT, &start, &f, Some(&Open));
        assert!(c.step(DT, &ev(&[change(StateId::Stylefinish, StateId::Idle)]), &frame(StateId::Idle, sora, 0.0), Some(&Open)).is_none());
        assert!(!c.active() && c.latent.is_empty());
    }
}
