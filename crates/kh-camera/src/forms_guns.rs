//! (AUDIT-A CAMS) THE GUN FORMS' CAMERA BLUEPRINTS, as data: Shooting Star's
//! `Blueprints/Camera/p_ex001_StateEvent/change_key/BP_TresPlayerStateEventCamera_2shotguns` (Double Arrowguns) and
//! `..._Bazooka` (Magic Launcher, and FINISH_BZ of both launchers), Hunny Spout's `..._po_key` (Hunny Blasters / Hunny
//! Launcher). Bytecode read in full 2026-11-05 with build/sora_tmp/weapons/AUDIT-C/CAM/bpdis.js (listings
//! build/sora_tmp/weapons/AUDIT-A/CAMS/bp/*.txt); `@n` = the ubergraph's bytecode offset. All three are in Sora's load list
//! (TresPlayerPawnLoad_ex001), so they run with every keyblade; the sequences and commands they compare are the gun forms'
//! only. A trace = `BP_TresLineTraceByChannel(his actor location + (0, 0, 100) -> TransformLocation(his transform, P),
//! channel 1)`; "hit" = it returned true. perf = `BP_TresIsGameEnablePerformanceCamera` (form_bp.rs `PerfCamera`).
//!
//! # BP_TresPlayerStateEventCamera_2shotguns (StyleChange2 = M_2401, its BpEvent 0 .. 2.233 s)
//! - Start '2401' (@1633 -> @1044): perf off -> @10 `RemoteEvent('Cnage_Key_Cancel')` (the level script's: no listener).
//!   T1 (900, 200, 150) clear -> 2shotguns_start1; hit -> @543 T2 (106, -56, 134): hit -> none, clear -> 2ShotGuns_start_02.
//!   @982 `BP_SpawnTresCameraAnim('2SHOT')`, `BP_SettingTresCamera(Sora)`, `EnableCollision(true)`,
//!   `BP_AttachTresCameraTarget(true)`, `StartTresCameraAnim(waza, bLoop false, bTakeover false, in {0, 0, 2}: a cut,
//!   bAutoEnd FALSE, bEndTakeover false, end {0, 0, 2})`, `SetPriority(200)` -> `2shotguns_cam_anim` (no influence call:
//!   the yaw is taken once).
//! - End '2401' (@1789): `ResetTresCameraPrevious` (the field camera behind him) -> @1545
//!   `2shotguns_cam_anim.EndTresCamera(false, {0, ease out 3, 2})` (a cut).
//! - Dtor (@1978): STYLECHANGE 31, WEAPONCHANGE 32, STYLEFINISH 33, COMBO_DW 75 -> @9678 `EndAllSameBPTresCamera` (a cut).
//!
//! # BP_TresPlayerStateEventCamera_Bazooka
//! - `ReceivePlayerBeginCommandAction` (@6117): CHANGE_BZ 131 -> @3286, FINISH_BZ 157 -> @16294 (the others: nothing).
//! - CHANGE_BZ (Magic Launcher; StyleChange5 = M_5401 has no BpEvent: this is its transformation camera): perf (the queued
//!   STYLECHANGE: Keyblade Cinematics) off -> RemoteEvent. T (62.212006, 170.13983, 91.76937) hit, or @4229 T
//!   (-257.3405, 188.58539, 132.2165) hit, or @4649 T (119, 97, 113) hit -> cam_bazooka_02; all clear -> bazooka_start.
//!   @3777 'start bazooka': Sora, collision, attached, `StartTresCameraAnim(waza, false, false, in {0, 0, 2}: a cut,
//!   bAutoEnd, bEndTakeover TRUE, end {0, 3, 2, lock}: a cut)` -> `bazooka_cam_anim`, priority 200.
//! - '5413' start (@28139, Rising Blast 0x4C, COMBO_BZ): perf (Combo Cam) -> `RandomIntegerInRange(1, 10) == 1` and T
//!   (-210, 70, 119) clear and T (-23, 117, 49) clear -> @1027 'bazooka_rush': Bazooka_Rush, bTakeover true, in {0.6, ease
//!   in-out 4, 2, lock}, bAutoEnd, bEndTakeover, end {0.3, 3, 2, lock}; otherwise @1552 the RotTarget 'Bazooka_Rush'
//!   (@392): attached, collision, `SetTresCameraTargetInfluenceRotation(false, 0, true ..)` (the yaw once),
//!   `SetTresCameraRotTargetParam(300, R(10, -20, 0), (0, 0, 0), local (0, 100, 70))`, `StartTresCamera(bTakeover true,
//!   {0.6, 4, 2})` -> `cam_Rot`, `SetOutsideWallCheckType(1, ..)`, priority 200; `Delay(1.5)` -> @272: if cam_Rot is valid,
//!   `End Tres Camera Normal(cam_Rot, bTakeover true, 0.3, 4, lock)` (FunctionLibrary_Camera: `EndTresCamera(true, {0.3,
//!   ease in-out, 2})`).
//! - FINISH_BZ (@16294, NO perf test): `BP_GetEquipWeaponID()` 3 (WEP_KEYBLADE01 Shooting Star) -> @19727: T (488, 139,
//!   253) clear -> `RandomIntegerInRange(1, 10) <= 5` ? Bazooka_end01 : Bazooka_end03; hit -> T (-398, 267, 311) clear ->
//!   Bazooka_end02, hit -> none. 8 (WEP_KEYBLADE07 Hunny Spout) -> @19311: T (282.05063, -24.388718, 241.68532) clear and
//!   T (-277.9551, 216.43114, 108.7398) clear -> Bazooka_cannon_po02 (Po_key folder), else none. Other weapons: none.
//!   @5625 'finish bazooka': Sora, collision, attached, `StartTresCameraAnim(waza, false, false, in {0.7, 3, 2, lock},
//!   bAutoEnd, bEndTakeover TRUE, end {0.5, 3, 2, lock})` -> `bazooka_cam_anim`, priority 200.
//! - Dtor (@20716): STYLECHANGE 31 -> @27942: `waza_name == bazooka_start` and `bazooka_cam_anim` valid ->
//!   `EndTresCamera(bTakeover true, a cut)`, then @2169 the RotTarget 'none' (300, R(0, 0, 0), 0, 0): attached, collision,
//!   yaw once, `StartTresCamera(false, a cut)`, priority 200, `Delay(0.1)` -> @96 `EndTresCamera(true, {0.3, 3, 2, lock})`.
//!   WEAPONCHANGE 32 -> @15 `EndAllSameBPTresCamera` (a cut). STYLEFINISH 33 -> @27825 `bazooka_cam_anim.EndTresCamera(cut)`,
//!   `ResetTresCameraPrevious`. COMBO_BZ 78 -> @27744 `EndAllSameBPTresCamera` (a cut).
//! - Ctor (@28257): DAMAGE 19 -> @35273 `bazooka_cam_anim` and `cam_Rot` `EndTresCamera(cut)`.
//!
//! # BP_TresPlayerStateEventCamera_po_key (Hunny Spout)
//! - Start (@5465, Sora only): 'G401' (StyleChange16, Hunny Blasters) -> @4432: perf off -> RemoteEvent; on ->
//!   `ChangeKey_Camera_Po_2ShotGuns_Start` (@5674): ON_Skill = true; T (-592.16614, -338.45636, 199.24298), T (143.18211,
//!   -673.7971, 269.53778), T (662.20087, -390.44547, 227.44313): any hit -> none; @3091 'Po_2Shot__Start_Cmaera':
//!   Po_2ShitGuns. 'H401' (StyleChange17, Hunny Launcher) -> @5393: perf off -> RemoteEvent; on ->
//!   `ChangeKey_Camera_Po_Bazooka_Start` (@5690): ON_Skill = true; T (680.41125, -83.80765, 71.75801), T (748.2613,
//!   336.888, 99.79666), T (217.52716, 585.69904, 859.4531): any hit -> none; @4370 'PoBazooka_Start_Cmaera': Po_bazooka.
//!   Both: Sora, collision, priority 200, `BP_AttachTresCameraTarget(false)`, `StartTresCameraAnim(anim, false, false, in
//!   {0, 4, 2, lock}: a cut, bAutoEnd, bEndTakeover false, end {0, 4, 2, lock}: a cut)` -> `cam_anim`,
//!   `SetTresCameraTargetInfluenceRotation(true, 0, true ..)` (the yaw always), `PlayZeroFrame(true)`; `WaitPlayEnd` ->
//!   @265 / @1054: if ON_Skill: ON_Skill = false, `GetTresCameraNormal().ResetCamera(true)`, the RotTarget 'back_camera'
//!   on Sora (500, R(-10, 0, 0)), `StartTresCamera(false, a cut)`, priority 200; `TresWaitCycle(1)` -> @15 / @140
//!   `EndAllBPTresCameraOfName('back_camera', {0.5, ease out 3, 2, lock})`. (No `ReceiveAnimNotifyEndBpEvent`.)
//! - Dtor (@6298): 31, 32, 33, COMBO_DW 75, COMBO_BZ 78 -> @13330 ON_Skill = false; `cam_anim` valid -> `EndTresCamera(
//!   false, {0, 3, 2, lock})` (a cut).
//!
//! # Natively (re/decomp): StyleChange Begin 0x1413CBA70 / Tick 0x1413D3AC0 for 0x80 / 0x83 / 0x8E / 0x8F (CHANGE_DW / _BZ /
//! _DW2 / _BZ2) call no camera (no vt+0x788 spawn, no attack-camera request); their cameras are these blueprints'.
//!
//! # Port notes
//! - The composition, collision pass and blends are form_bp.rs's / forms_shield.rs's: World = base + yaw-turned anim key,
//!   base = his actor location every step (attached or not: camera_anim.rs [I]); the RotTargets look at his actor location +
//!   the yaw-turned local offset. `SetOutsideWallCheckType(1, ..)` is the RotTarget's own wall test: the collision pass
//!   (EnableCollision) stands for it.
//! - bTakeover at the start of a CameraAnim only seeds what the anim overwrites (0x1405D9AC0); a RotTarget's start bTakeover
//!   is a blend from the view on screen. `EndTresCamera(bTakeover true)` / bEndTakeover = [`KeyCamOutput::takeover`].
//! - `RandomIntegerInRange` = FMath::RandRange: here a deterministic LCG ([`GunCameras::rng`], hosts may seed it).
//! - "valid" (`IsValid(cam)`) = the camera is still running: an ended camera (its own end or an EndAll) is destroyed.
//! - ReceivePlayerBeginCommandAction runs after the step's events (StartStyle's state change, then 0x1414158F0).

use glam::Vec3;
use kh_core::host::EventKind;
use kh_core::{Event, StateId, World};

use crate::camera_anim::{camera_anim, collide, frame_target, sweep_start, AnimView, CameraAnimDef, Transition};
use crate::form_bp::FormBpFrame;
use crate::shake::ShakeOffset;
use crate::Rot;

/// One step's result of a keyblade's camera blueprint layer (shared by the AUDIT-A CAMS layers).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct KeyCamOutput {
    /// The view to draw (None = the cameras under it).
    pub view: Option<AnimView>,
    /// ResetTresCameraPrevious / ResetCamera(true): the field camera behind Sora at once.
    pub reset_field_camera: bool,
    /// bEndTakeover / EndTresCamera(bTakeover true): the field camera takes over this view.
    pub takeover: Option<AnimView>,
    pub shake: ShakeOffset,
}

/// `ETresCommandKind` CHANGE_BZ / FINISH_BZ (tres_enums.rs).
pub const CHANGE_BZ: i32 = 131;
pub const FINISH_BZ: i32 = 157;
/// `BP_GetEquipWeaponID()` (ETresItemDefWeapon) of Shooting Star (WEP_KEYBLADE01) and Hunny Spout (WEP_KEYBLADE07): the
/// same numbers as kh-core `combat::form::weapon_type`.
pub const SHOOTING_STAR: u32 = 3;
pub const HUNNY_SPOUT: u32 = 8;

/// 2shotguns @1294 / @736.
pub const TWO_SHOT_TRACES: [Vec3; 2] = [Vec3::new(900.0, 200.0, 150.0), Vec3::new(106.0, -56.0, 134.0)];
/// Bazooka CHANGE_BZ @3536 / @4422 / @4842.
pub const CHANGE_BZ_TRACES: [Vec3; 3] = [Vec3::new(62.212_006, 170.139_83, 91.769_37), Vec3::new(-257.340_5, 188.585_39, 132.216_5), Vec3::new(119.0, 97.0, 113.0)];
/// Bazooka 5413 @2563 / @2983.
pub const RUSH_TRACES: [Vec3; 2] = [Vec3::new(-210.0, 70.0, 119.0), Vec3::new(-23.0, 117.0, 49.0)];
/// Bazooka FINISH_BZ, Shooting Star @19920 / @20335; Hunny Spout @19504 / @5383.
pub const FINISH_SS_TRACES: [Vec3; 2] = [Vec3::new(488.0, 139.0, 253.0), Vec3::new(-398.0, 267.0, 311.0)];
pub const FINISH_HS_TRACES: [Vec3; 2] = [Vec3::new(282.050_63, -24.388_718, 241.685_32), Vec3::new(-277.955_1, 216.431_14, 108.739_8)];
/// po_key G401 @2036 / @2452 / @2868, H401 @5894 / @5170 / @4754.
pub const PO_2SHOT_TRACES: [Vec3; 3] = [Vec3::new(-592.166_1, -338.456_36, 199.242_98), Vec3::new(143.182_11, -673.797_1, 269.537_78), Vec3::new(662.200_87, -390.445_47, 227.443_13)];
pub const PO_BAZOOKA_TRACES: [Vec3; 3] = [Vec3::new(680.411_25, -83.807_65, 71.758_01), Vec3::new(748.261_3, 336.888, 99.796_66), Vec3::new(217.527_16, 585.699_04, 859.453_1)];
/// Bazooka's RotTarget 'Bazooka_Rush' (@603): distance, rotation, local offset; its `Delay(1.5)`.
pub const RUSH_ROT: (f32, Rot, Vec3) = (300.0, Rot { pitch: 10.0, yaw: -20.0, roll: 0.0 }, Vec3::new(0.0, 100.0, 70.0));
pub const RUSH_ROT_TIME: f32 = 1.5;
/// Bazooka's RotTarget 'none' after the transformation (@1825) and its `Delay(0.1)`.
pub const AFTER_CHANGE_ROT: (f32, Rot, Vec3) = (300.0, Rot { pitch: 0.0, yaw: 0.0, roll: 0.0 }, Vec3::ZERO);
pub const AFTER_CHANGE_TIME: f32 = 0.1;
/// po_key's RotTarget 'back_camera' (@781 / @1570).
pub const BACK_ROT: (f32, Rot, Vec3) = (500.0, Rot { pitch: -10.0, yaw: 0.0, roll: 0.0 }, Vec3::ZERO);

/// A cut (`{0, ease out 3, 2}` / `{0, 0, 2}` / `{0, 4, 2, lock}`).
const CUT: Transition = Transition::new(0.0, 3, 2.0, false);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Bp {
    TwoShotguns,
    Bazooka,
    PoKey,
}

/// The blueprint variable a camera is held in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    /// 2shotguns `2shotguns_cam_anim`.
    TwoShot,
    /// Bazooka `bazooka_cam_anim` (start bazooka / bazooka_rush / finish bazooka).
    BazookaAnim,
    /// Bazooka `cam_Rot` (the RotTarget 'Bazooka_Rush').
    BazookaRot,
    /// Bazooka's local RV2 (the RotTarget 'none').
    AfterChange,
    /// po_key `cam_anim`.
    PoAnim,
    /// po_key's 'back_camera'.
    Back,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Anim { def: &'static CameraAnimDef, time: f32, auto_end: bool, end: Transition, end_takeover: bool, zero_frame: bool },
    /// `ATresCameraRotTarget`: distance, rotation, local offset (in his yaw frame).
    Orbit { distance: f32, rotation: Rot, local: Vec3 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Cam {
    id: u32,
    bp: Bp,
    name: &'static str,
    slot: Slot,
    kind: Kind,
    yaw_always: bool,
    base_yaw: Option<f32>,
}

/// One StartTresCameraAnim call's settings.
#[derive(Clone, Copy, Debug)]
struct Start {
    anim: &'static str,
    blend_in: Transition,
    auto_end: bool,
    end: Transition,
    end_takeover: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Then {
    /// Bazooka @272: `Delay(1.5)` of the RotTarget 'Bazooka_Rush'.
    RushEnd,
    /// Bazooka @96: `Delay(0.1)` of the RotTarget 'none'.
    AfterChangeEnd,
    /// po_key @265 / @1054: `WaitPlayEnd` of cam_anim.
    PoPlayed,
    /// po_key @15 / @140: `TresWaitCycle(1)` after the back camera.
    BackEnd,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Wait {
    Time(f32),
    Cycles(u32),
    PlayEnd(u32),
}

/// The gun forms' blueprints (2shotguns, Bazooka, po_key).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct GunCameras {
    cams: Vec<Cam>,
    next_id: u32,
    blend: Option<(AnimView, Transition, f32)>,
    last: Option<AnimView>,
    latent: Vec<(Bp, Wait, Then, Option<u32>)>,
    /// The ids whose anim reached its end this step (their WaitPlayEnd runs at the next one).
    played: Vec<u32>,
    state: Option<StateId>,
    /// Bazooka `waza_name` (the last anim it chose).
    waza: Option<&'static str>,
    /// po_key `ON_Skill`.
    on_skill: bool,
    pending: KeyCamOutput,
    /// `BP_GetEquipWeaponID()`: hosts write kh-core `combat::form::weapon_type(status)` before every step.
    pub weapon: u32,
    /// `RandomIntegerInRange`'s generator (an LCG; any seed).
    pub rng: u32,
    /// A readable log (tests, traces).
    pub log: Vec<String>,
}

fn turn(v: Vec3, yaw: f32) -> Vec3 {
    let (s, c) = yaw.to_radians().sin_cos();
    Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
}

/// The trace from his location + 100 to his local `to` hit (no level: clear).
fn hit(world: Option<&dyn World>, at: (Vec3, f32), to: Vec3) -> bool {
    world.is_some_and(|w| w.line_trace(at.0 + Vec3::Z * 100.0, at.0 + turn(to, at.1)).is_some())
}

impl GunCameras {
    pub fn new() -> GunCameras {
        GunCameras::default()
    }

    pub fn active(&self) -> bool {
        !self.cams.is_empty() || self.blend.is_some()
    }

    /// The running cameras (spawn name / anim), oldest first.
    pub fn cameras(&self) -> Vec<&'static str> {
        self.cams
            .iter()
            .map(|c| match c.kind {
                Kind::Anim { def, .. } => def.name,
                Kind::Orbit { .. } => c.name,
            })
            .collect()
    }

    /// `RandomIntegerInRange(lo, hi)`.
    fn random(&mut self, lo: i32, hi: i32) -> i32 {
        self.rng = self.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        lo + ((self.rng >> 8) % (hi - lo + 1).max(1) as u32) as i32
    }

    fn push(&mut self, cam: Cam, blend_in: Transition) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.log.push(format!("start {} {} (in {} s)", cam.name, match cam.kind {
            Kind::Anim { def, .. } => def.name.to_string(),
            Kind::Orbit { distance, rotation, .. } => format!("RotTarget {distance} R({}, {}, {})", rotation.pitch, rotation.yaw, rotation.roll),
        }, blend_in.time));
        self.blend = match (self.last, blend_in.time > 0.0) {
            (Some(from), true) => Some((from, blend_in, 0.0)),
            _ => None,
        };
        self.cams.push(Cam { id, ..cam });
        id
    }

    /// `BP_SpawnTresCameraAnim(name)` + `StartTresCameraAnim(s.anim, false, .., s.blend_in, s.auto_end, s.end_takeover, s.end)`.
    fn anim(&mut self, bp: Bp, name: &'static str, slot: Slot, s: Start) -> Option<u32> {
        let Some(def) = camera_anim(s.anim) else {
            self.log.push(format!("{} is not in the generated camera tables (node tools/party_team_pack.js --cameras)", s.anim));
            return None;
        };
        let kind = Kind::Anim { def, time: 0.0, auto_end: s.auto_end, end: s.end, end_takeover: s.end_takeover, zero_frame: false };
        Some(self.push(Cam { id: 0, bp, name, slot, kind, yaw_always: false, base_yaw: None }, s.blend_in))
    }

    fn orbit(&mut self, bp: Bp, name: &'static str, slot: Slot, p: (f32, Rot, Vec3), blend_in: Transition) -> u32 {
        let kind = Kind::Orbit { distance: p.0, rotation: p.1, local: p.2 };
        self.push(Cam { id: 0, bp, name, slot, kind, yaw_always: false, base_yaw: None }, blend_in)
    }

    fn valid(&self, slot: Slot) -> bool {
        self.cams.iter().any(|c| c.slot == slot)
    }

    fn end_where(&mut self, t: Transition, takeover: bool, why: &str, pred: impl Fn(&Cam) -> bool) {
        let top = self.cams.last().map(|c| c.id);
        let before = self.cams.len();
        self.cams.retain(|c| !pred(c));
        if self.cams.len() == before {
            return;
        }
        self.log.push(format!("end {} camera(s) ({why})", before - self.cams.len()));
        if self.cams.last().map(|c| c.id) != top {
            self.blend = match (self.last, t.time > 0.0) {
                (Some(from), true) => Some((from, t, 0.0)),
                _ => None,
            };
            if takeover && self.cams.is_empty() {
                self.pending.takeover = self.last;
            }
        }
    }

    fn perf_now(&self, frame: &FormBpFrame) -> bool {
        frame.perf.enabled(self.state.unwrap_or(frame.state))
    }

    // ---------------------------------------------------------------- 2shotguns

    fn two_shot_start(&mut self, frame: &FormBpFrame, world: Option<&dyn World>) {
        if !self.perf_now(frame) {
            self.log.push("2401: no performance camera: RemoteEvent(Cnage_Key_Cancel) @10".to_string());
            return;
        }
        let anim = if !hit(world, frame.sora, TWO_SHOT_TRACES[0]) {
            "2shotguns_start1"
        } else if !hit(world, frame.sora, TWO_SHOT_TRACES[1]) {
            "2ShotGuns_start_02"
        } else {
            self.log.push("2401: both traces hit: no camera".to_string());
            return;
        };
        self.anim(Bp::TwoShotguns, "2SHOT", Slot::TwoShot, Start { anim, blend_in: Transition::CUT, auto_end: false, end: CUT, end_takeover: false });
    }

    fn two_shot_end(&mut self) {
        if self.valid(Slot::TwoShot) {
            // @1940 ResetTresCameraPrevious (the camera under it: the field camera), @1545 the cut
            self.pending.reset_field_camera = true;
            self.end_where(CUT, false, "end of 2401 @1545", |c| c.slot == Slot::TwoShot);
        }
    }

    // ---------------------------------------------------------------- Bazooka

    fn change_bz(&mut self, frame: &FormBpFrame, world: Option<&dyn World>) {
        // the queued STYLECHANGE decides (0x140EAB300 tests the queued state; StartStyle queued it before 0x1414158F0)
        if !frame.perf.enabled(StateId::Stylechange) {
            self.log.push("CHANGE_BZ: no performance camera: RemoteEvent(Cnage_Key_Cancel) @5093".to_string());
            return;
        }
        let tight = CHANGE_BZ_TRACES.iter().any(|p| hit(world, frame.sora, *p));
        let anim = if tight { "cam_bazooka_02" } else { "bazooka_start" };
        self.waza = Some(anim);
        self.anim(Bp::Bazooka, "start bazooka", Slot::BazookaAnim, Start { anim, blend_in: Transition::CUT, auto_end: true, end: Transition::new(0.0, 3, 2.0, true), end_takeover: true });
    }

    fn rush(&mut self, frame: &FormBpFrame, world: Option<&dyn World>) {
        if !self.perf_now(frame) {
            return;
        }
        let roll = self.random(1, 10);
        if roll == 1 && RUSH_TRACES.iter().all(|p| !hit(world, frame.sora, *p)) {
            self.anim(Bp::Bazooka, "bazooka_rush", Slot::BazookaAnim, Start { anim: "Bazooka_Rush", blend_in: Transition::new(0.6, 4, 2.0, true), auto_end: true, end: Transition::new(0.3, 3, 2.0, true), end_takeover: true });
            return;
        }
        let id = self.orbit(Bp::Bazooka, "Bazooka_Rush", Slot::BazookaRot, RUSH_ROT, Transition::new(0.6, 4, 2.0, false));
        self.latent.push((Bp::Bazooka, Wait::Time(RUSH_ROT_TIME), Then::RushEnd, Some(id)));
    }

    fn finish_bz(&mut self, frame: &FormBpFrame, world: Option<&dyn World>) {
        let anim = match self.weapon {
            SHOOTING_STAR if !hit(world, frame.sora, FINISH_SS_TRACES[0]) => {
                if self.random(1, 10) <= 5 {
                    "Bazooka_end01"
                } else {
                    "Bazooka_end03"
                }
            }
            SHOOTING_STAR if !hit(world, frame.sora, FINISH_SS_TRACES[1]) => "Bazooka_end02",
            HUNNY_SPOUT if FINISH_HS_TRACES.iter().all(|p| !hit(world, frame.sora, *p)) => "Bazooka_cannon_po02",
            w => {
                self.log.push(format!("FINISH_BZ: weapon {w}: no camera (traces / weapon)"));
                return;
            }
        };
        self.waza = Some(anim);
        self.anim(Bp::Bazooka, "finish bazooka", Slot::BazookaAnim, Start { anim, blend_in: Transition::new(0.7, 3, 2.0, true), auto_end: true, end: Transition::new(0.5, 3, 2.0, true), end_takeover: true });
    }

    // ---------------------------------------------------------------- po_key

    fn po_start(&mut self, seq: &str, frame: &FormBpFrame, world: Option<&dyn World>) {
        let (traces, name, anim) = match seq {
            "G401" => (PO_2SHOT_TRACES, "Po_2Shot__Start_Cmaera", "Po_2ShitGuns"),
            _ => (PO_BAZOOKA_TRACES, "PoBazooka_Start_Cmaera", "Po_bazooka"),
        };
        if !self.perf_now(frame) {
            self.log.push(format!("{seq}: no performance camera: RemoteEvent(Cnage_Key_Cancel) @4504"));
            return;
        }
        self.on_skill = true;
        if traces.iter().any(|p| hit(world, frame.sora, *p)) {
            self.log.push(format!("{seq}: a trace hit: no camera"));
            return;
        }
        let cut = Transition::new(0.0, 4, 2.0, true);
        if let Some(id) = self.anim(Bp::PoKey, name, Slot::PoAnim, Start { anim, blend_in: cut, auto_end: true, end: cut, end_takeover: false }) {
            if let Some(c) = self.cams.iter_mut().find(|c| c.id == id) {
                c.yaw_always = true;
                if let Kind::Anim { zero_frame, .. } = &mut c.kind {
                    *zero_frame = true;
                }
            }
            self.latent.push((Bp::PoKey, Wait::PlayEnd(id), Then::PoPlayed, Some(id)));
        }
    }

    // ---------------------------------------------------------------- events

    fn then(&mut self, then: Then, id: Option<u32>) {
        match then {
            Then::RushEnd => {
                if id.is_some_and(|i| self.cams.iter().any(|c| c.id == i)) {
                    self.end_where(Transition::new(0.3, 4, 2.0, true), true, "Bazooka @272: End Tres Camera Normal(cam_Rot, takeover, 0.3)", |c| Some(c.id) == id);
                }
            }
            Then::AfterChangeEnd => {
                if id.is_some_and(|i| self.cams.iter().any(|c| c.id == i)) {
                    self.end_where(Transition::new(0.3, 3, 2.0, true), true, "Bazooka @96: EndTresCamera(takeover, 0.3)", |c| Some(c.id) == id);
                }
            }
            Then::PoPlayed => {
                if self.on_skill {
                    self.on_skill = false;
                    self.pending.reset_field_camera = true;
                    self.log.push("po_key WaitPlayEnd: ResetCamera(true), back_camera".to_string());
                    self.orbit(Bp::PoKey, "back_camera", Slot::Back, BACK_ROT, Transition::CUT);
                    self.latent.push((Bp::PoKey, Wait::Cycles(1), Then::BackEnd, None));
                }
            }
            Then::BackEnd => self.end_where(Transition::new(0.5, 3, 2.0, true), false, "po_key @15: EndAllBPTresCameraOfName(back_camera, 0.5)", |c| c.slot == Slot::Back),
        }
    }

    fn dtor(&mut self, from: StateId) {
        let s = from as i32;
        if matches!(s, 31 | 32 | 33 | 75) {
            self.end_where(CUT, false, "2shotguns dtor @9678", |c| c.bp == Bp::TwoShotguns);
        }
        match s {
            31 if self.waza == Some("bazooka_start") && self.valid(Slot::BazookaAnim) => {
                self.end_where(CUT, true, "Bazooka dtor 31 @28051: EndTresCamera(takeover, cut)", |c| c.slot == Slot::BazookaAnim);
                let id = self.orbit(Bp::Bazooka, "none", Slot::AfterChange, AFTER_CHANGE_ROT, Transition::CUT);
                self.latent.push((Bp::Bazooka, Wait::Time(AFTER_CHANGE_TIME), Then::AfterChangeEnd, Some(id)));
            }
            32 | 78 => self.end_where(CUT, false, "Bazooka dtor @15 / @27744: EndAllSameBPTresCamera", |c| c.bp == Bp::Bazooka),
            33 if self.valid(Slot::BazookaAnim) => {
                self.end_where(CUT, false, "Bazooka dtor 33 @27825", |c| c.slot == Slot::BazookaAnim);
                self.pending.reset_field_camera = true;
                self.log.push("Bazooka dtor 33: ResetTresCameraPrevious".to_string());
            }
            _ => {}
        }
        if matches!(s, 31 | 32 | 33 | 75 | 78) {
            self.on_skill = false;
            self.end_where(CUT, false, "po_key dtor @13330", |c| c.slot == Slot::PoAnim);
        }
        // the latent actions of ended cameras go with them
        let alive: Vec<u32> = self.cams.iter().map(|c| c.id).collect();
        self.latent.retain(|(_, _, _, id)| id.is_none_or(|i| alive.contains(&i)));
    }

    fn on_event(&mut self, ev: &EventKind, frame: &FormBpFrame, world: Option<&dyn World>) {
        match *ev {
            EventKind::StateChange { from, to } => {
                self.dtor(from);
                if to == StateId::Damage {
                    self.end_where(CUT, false, "Bazooka ctor 19 @35273", |c| matches!(c.slot, Slot::BazookaAnim | Slot::BazookaRot));
                }
                self.state = Some(to);
            }
            EventKind::AnimBpEvent { seq: "2401", begin: true, .. } => self.two_shot_start(frame, world),
            EventKind::AnimBpEvent { seq: "2401", begin: false, .. } => self.two_shot_end(),
            EventKind::AnimBpEvent { seq: "5413", begin: true, .. } => self.rush(frame, world),
            EventKind::AnimBpEvent { seq: seq @ ("G401" | "H401"), begin: true, .. } => self.po_start(seq, frame, world),
            _ => {}
        }
    }

    fn run_latents(&mut self, dt: f32) {
        let played = std::mem::take(&mut self.played);
        let mut due = Vec::new();
        self.latent.retain_mut(|(_, wait, then, id)| {
            let fire = match wait {
                Wait::Time(t) => {
                    *t -= dt;
                    *t <= 1.0e-4
                }
                Wait::Cycles(n) => {
                    *n = n.saturating_sub(1);
                    *n == 0
                }
                Wait::PlayEnd(i) => played.contains(i),
            };
            if fire {
                due.push((*then, *id));
            }
            !fire
        });
        for (t, id) in due {
            self.then(t, id);
        }
    }

    fn view_of(c: &mut Cam, sora: (Vec3, f32), world: Option<&dyn World>) -> AnimView {
        let (loc, yaw) = sora;
        if c.base_yaw.is_none() || c.yaw_always {
            c.base_yaw = Some(yaw);
        }
        let by = c.base_yaw.unwrap_or(yaw);
        let (v, pivot) = match c.kind {
            Kind::Anim { def, time, .. } => {
                let s = def.sample(time);
                (AnimView { location: loc + turn(s.location, by), rotation: Rot { pitch: s.rotation.pitch, yaw: s.rotation.yaw + by, roll: s.rotation.roll }.normalized(), fov: s.fov }, loc)
            }
            Kind::Orbit { distance, rotation, local } => {
                let look = loc + turn(local, by);
                let r = Rot { pitch: rotation.pitch, yaw: rotation.yaw + by, roll: rotation.roll }.normalized();
                (AnimView { location: look - r.forward() * distance, rotation: r, fov: crate::forms::NATIVE_FOV }, look)
            }
        };
        // EnableCollision(true): every camera of these blueprints
        let Some(w) = world else { return v };
        match frame_target(&v, pivot, w) {
            Some(f) if f.distance > 0.0 => {
                let start = f.location + f.rotation.forward() * f.distance;
                AnimView { location: collide(start, f.location, w), rotation: f.rotation, ..v }
            }
            _ => AnimView { location: collide(sweep_start(&v, pivot), v.location, w), ..v },
        }
    }

    /// One step: the step's core events (in order), the command that began this step (ReceivePlayerBeginCommandAction,
    /// form_bp.rs), advance; the view over the cameras below (`frame.field`).
    pub fn step(&mut self, dt: f32, events: &[Event], frame: &FormBpFrame, begun_command: Option<i32>, world: Option<&dyn World>) -> KeyCamOutput {
        self.run_latents(dt);
        for e in events {
            self.on_event(&e.kind, frame, world);
        }
        match begun_command {
            Some(CHANGE_BZ) => self.change_bz(frame, world),
            Some(FINISH_BZ) => self.finish_bz(frame, world),
            _ => {}
        }
        self.state = Some(frame.state);
        // advance the anims (PlayZeroFrame: the first tick uses dt 0); bAutoEnd
        let mut ended = Vec::new();
        for c in &mut self.cams {
            if let Kind::Anim { def, time, auto_end, end, end_takeover, zero_frame } = &mut c.kind {
                let step = if std::mem::take(zero_frame) { 0.0 } else { dt };
                let before = *time;
                *time = (*time + step).min(def.length);
                if *time >= def.length && before < def.length {
                    self.played.push(c.id);
                    if *auto_end {
                        ended.push((c.id, *end, *end_takeover));
                    }
                }
            }
        }
        for (id, t, takeover) in ended {
            let name = self.cams.iter().find(|c| c.id == id).map_or("", |c| c.name);
            self.end_where(t, takeover, &format!("auto end {name}"), |c| c.id == id);
        }
        let below = frame.field;
        let top = self.cams.last_mut().map(|c| Self::view_of(c, frame.sora, world));
        let target = top.unwrap_or(below);
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
        let view = if self.cams.is_empty() && self.blend.is_none() { None } else { view };
        self.last = Some(view.unwrap_or(below));
        let mut out = std::mem::take(&mut self.pending);
        out.view = view;
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form_bp::PerfCamera;

    fn frame(state: StateId) -> FormBpFrame {
        FormBpFrame { sora: (Vec3::new(100.0, 0.0, 0.0), 30.0), field: AnimView { location: Vec3::new(-400.0, 0.0, 200.0), rotation: Rot::default(), fov: 100.0 }, state, ultima: false, perf: PerfCamera::default() }
    }

    fn bp(seq: &'static str, begin: bool) -> Vec<Event> {
        vec![Event { step: 0, kind: EventKind::AnimBpEvent { seq, param: 0, begin } }]
    }

    fn change(from: StateId, to: StateId) -> Vec<Event> {
        vec![Event { step: 0, kind: EventKind::StateChange { from, to } }]
    }

    const DT: f32 = 1.0 / 60.0;

    /// StyleChange2's 2401: 2shotguns_start1 cuts in on him, holds (no autoEnd) until the window's end: the field camera
    /// reset and a cut.
    #[test]
    fn double_arrowguns_transformation_camera() {
        let def = camera_anim("2shotguns_start1").expect("packed (party_team_pack.js --cameras)");
        let mut c = GunCameras::new();
        let f = frame(StateId::Stylechange);
        let v = c.step(DT, &bp("2401", true), &f, None, None).view.unwrap();
        assert_eq!(c.cameras(), ["2shotguns_start1"]);
        let s = def.sample(DT);
        assert!((v.location - (f.sora.0 + turn(s.location, 30.0))).length() < 1e-2, "a cut onto the anim in his frame");
        for _ in 0..200 {
            c.step(DT, &[], &f, None, None);
        }
        assert_eq!(c.cameras(), ["2shotguns_start1"], "no autoEnd: it holds its last key");
        let o = c.step(DT, &bp("2401", false), &f, None, None);
        assert!(o.reset_field_camera && o.view.is_none() && !c.active(), "ResetTresCameraPrevious and a cut");
        // Keyblade Cinematics Basic: none
        let off = FormBpFrame { perf: PerfCamera { keyblade_cinematics: false, ..PerfCamera::default() }, ..f };
        c.step(DT, &bp("2401", true), &off, None, None);
        assert!(c.cameras().is_empty());
        // the dtor of STYLECHANGE cuts it
        c.step(DT, &bp("2401", true), &f, None, None);
        c.step(DT, &change(StateId::Stylechange, StateId::Idle), &frame(StateId::Idle), None, None);
        assert!(c.cameras().is_empty());
    }

    /// CHANGE_BZ: bazooka_start (cut, autoEnd, takeover at its end); STYLECHANGE's dtor while it runs: the takeover cut and
    /// the 0.1 s RotTarget 'none', ended with a 0.3 s takeover.
    #[test]
    fn magic_launchers_transformation_camera() {
        let mut c = GunCameras::new();
        let f = frame(StateId::Stylechange);
        c.step(DT, &[], &f, Some(CHANGE_BZ), None);
        assert_eq!(c.cameras(), ["bazooka_start"]);
        let mut took = None;
        for _ in 0..200 {
            took = took.or(c.step(DT, &[], &f, None, None).takeover);
        }
        assert!(c.cameras().is_empty() && took.is_some(), "autoEnd with bEndTakeover");
        // the state ends first
        c.step(DT, &[], &f, Some(CHANGE_BZ), None);
        c.step(DT, &[], &f, None, None);
        let o = c.step(DT, &change(StateId::Stylechange, StateId::Idle), &frame(StateId::Idle), None, None);
        assert!(o.takeover.is_some() && c.cameras() == ["none"]);
        let mut n = 0;
        let mut took = false;
        while c.active() {
            took |= c.step(DT, &[], &frame(StateId::Idle), None, None).takeover.is_some();
            n += 1;
            assert!(n < 60);
        }
        assert!(took && (22..=26).contains(&n), "0.1 s, then 0.3 s back: {n}");
    }

    /// FINISH_BZ: the weapon decides (Shooting Star: end01 / end03 by the roll; Hunny Spout: Bazooka_cannon_po02), a 0.7 s
    /// blend in, autoEnd with a 0.5 s takeover; no camera for another weapon. STYLEFINISH's dtor cuts and resets the field.
    #[test]
    fn the_launchers_finish_cameras() {
        let f = frame(StateId::Stylefinish);
        let mut c = GunCameras { weapon: SHOOTING_STAR, ..GunCameras::new() };
        c.step(DT, &[], &f, Some(FINISH_BZ), None);
        assert!(matches!(c.cameras().as_slice(), ["Bazooka_end01"] | ["Bazooka_end03"]), "{:?}", c.cameras());
        let o = c.step(DT, &change(StateId::Stylefinish, StateId::Idle), &frame(StateId::Idle), None, None);
        assert!(o.reset_field_camera && c.cameras().is_empty());
        let mut c = GunCameras { weapon: HUNNY_SPOUT, ..GunCameras::new() };
        c.step(DT, &[], &f, Some(FINISH_BZ), None);
        assert_eq!(c.cameras(), ["Bazooka_cannon_po02"]);
        let v = c.step(0.35, &[], &f, None, None).view.unwrap();
        assert!((v.location - f.field.location).length() > 1.0, "blending from the view on screen");
        let mut took = false;
        for _ in 0..240 {
            took |= c.step(DT, &[], &f, None, None).takeover.is_some();
        }
        assert!(took && !c.active(), "3 s, then the 0.5 s takeover");
        let mut c = GunCameras { weapon: 2, ..GunCameras::new() };
        c.step(DT, &[], &f, Some(FINISH_BZ), None);
        assert!(c.cameras().is_empty());
    }

    /// 5413: the RotTarget 'Bazooka_Rush' (9 rolls in 10) blends in over 0.6 s and ends after 1.5 s with a 0.3 s takeover;
    /// COMBO_BZ's dtor cuts the Bazooka cameras.
    #[test]
    fn rising_blasts_camera() {
        let f = frame(StateId::ComboBz);
        let mut seen_anim = false;
        let mut seen_rot = false;
        for seed in 0..40 {
            let mut c = GunCameras { rng: seed, ..GunCameras::new() };
            c.step(DT, &bp("5413", true), &f, None, None);
            match c.cameras().as_slice() {
                ["Bazooka_Rush"] => {
                    // either the anim or the RotTarget of the same name: tell by the slot
                    if c.cams[0].slot == Slot::BazookaRot {
                        seen_rot = true;
                        let mut n = 0;
                        let mut took = false;
                        while c.active() {
                            took |= c.step(DT, &[], &f, None, None).takeover.is_some();
                            n += 1;
                            assert!(n < 200);
                        }
                        assert!(took && (106..=110).contains(&n), "1.5 s + 0.3 s: {n}");
                    } else {
                        seen_anim = true;
                        c.step(DT, &change(StateId::ComboBz, StateId::Idle), &frame(StateId::Idle), None, None);
                        assert!(c.cameras().is_empty());
                    }
                }
                other => panic!("{other:?}"),
            }
        }
        assert!(seen_anim && seen_rot);
    }

    /// po_key: G401 / H401 cut in with the yaw always and PlayZeroFrame; at the anim's end the field reset and the back
    /// camera, gone 0.5 s after one tick; STYLECHANGE's dtor first: a cut and no back camera.
    #[test]
    fn hunny_spouts_transformation_cameras() {
        for (seq, anim) in [("G401", "Po_2ShitGuns"), ("H401", "Po_bazooka")] {
            let def = camera_anim(anim).expect("packed");
            let mut c = GunCameras::new();
            let f = frame(StateId::Stylechange);
            let v = c.step(DT, &bp(seq, true), &f, None, None).view.unwrap();
            let s = def.sample(0.0);
            assert!((v.location - (f.sora.0 + turn(s.location, 30.0))).length() < 1e-2, "{seq}: zero frame");
            let turned = FormBpFrame { sora: (f.sora.0, 90.0), ..f };
            let v2 = c.step(0.0, &[], &turned, None, None).view.unwrap();
            assert!((v2.location - (f.sora.0 + turn(s.location, 90.0))).length() < 1e-2, "{seq}: the yaw always");
            let mut reset = false;
            let mut n = 0;
            while !reset {
                reset = c.step(DT, &[], &f, None, None).reset_field_camera;
                n += 1;
                assert!(n < 400);
            }
            assert_eq!(c.cameras(), ["back_camera"]);
            let mut m = 0;
            while c.active() {
                c.step(DT, &[], &f, None, None);
                m += 1;
                assert!(m < 60);
            }
            assert!((29..=32).contains(&m), "{seq}: one tick, 0.5 s: {m}");
            // the dtor first
            c.step(DT, &bp(seq, true), &f, None, None);
            c.step(DT, &change(StateId::Stylechange, StateId::Idle), &frame(StateId::Idle), None, None);
            for _ in 0..300 {
                assert!(!c.step(DT, &[], &frame(StateId::Idle), None, None).reset_field_camera);
            }
            assert!(!c.active());
        }
    }
}
