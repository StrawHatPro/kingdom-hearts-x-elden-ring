//! (AUDIT-B F-CLAWS) THE CAMERA BLUEPRINTS OF HAPPY GEAR AND CRYSTAL SNOW, as data:
//! `Blueprints/Camera/p_ex001_StateEvent/change_key/BP_TresPlayerStateEventCamera_mi_key` (Agile Claws 0x8A / Twin Yo-Yos
//! 0x82) and `..._fz_key` (Blizzard Claws 0x90 / Blizzard Blades 0x85). Bytecode read with
//! build/sora_tmp/party/tools/bpdump.js (listings build/sora_tmp/weapons/AUDIT-B/F-CLAWS/traces/bp_mi_key_cam.txt and
//! bp_fz_key_cam.txt); `@n` = the ubergraph's bytecode offset. The CameraAnims are generated tables
//! (tools/party_team_pack.js `--cameras`). Driven like kh-camera `form_bp.rs` (AUDIT-C) by the core's events: the
//! `AnimBpEvent`s of Sora's `TresAnimNotifyState_BpEvent` windows (kh-core `states/bp_event.rs`, sent while a form is held)
//! and `StateChange` (the blueprints' ReceivePlayerDtorState); `form_bp::FormBpCameras::step` steps this and lays its view
//! over its own (the hosts kh-er cam.rs and the effects harness need no line of their own).
//!
//! Every camera below: `BP_SettingTresCamera(GetLocalTresPlayerPawnBase)` (on Sora), `EnableCollision(true)`,
//! `SetPriority(200)`, `BP_AttachTresCameraTarget(false)`; `SetTresCameraTransitionParams(time, func, exp, lock, ..)` =
//! [`Transition`] (func 4 ease in-out, 3 ease out; time 0 = a cut); `StartTresCameraAnim(anim, bLoop, bTakeover, in,
//! bAutoEnd, bEndTakeover, end)` (the reflected parameter order); "perf" = `BP_TresIsGameEnablePerformanceCamera`
//! (form_bp `PerfCamera`); a trace = `BP_TresLineTraceByChannel` (channel 1) from his location + (0, 0, 100) to a point of
//! his transform (x forward, y right, z up): a hit = no camera unless said otherwise.
//!
//! # mi_key (ReceiveAnimNotifyStartBpEvent @6532, SendActor = Sora)
//! | seq | entry | test | camera | anim | in | end | other |
//! |---|---|---|---|---|---|---|---|
//! | 4401 StyleChange4 | @4976 -> ChangeKey_Camera_YoYo_Start @7096 -> @6116 | perf (else @5048 RemoteEvent "Cnage_Key_Cancel"); traces (352.63, 646.07, 470.42) @6359 and (1030, 3.53, 175.31) @5348 clear | "YoYo_Start_Cmaera" @5521 | Yo-Yo_Start | 0.4 ease in-out | 0.33, bEndTakeover | takeover, yaw always @3456, zero frame @3351, WaitPlayEnd -> @1070 ResetTresCameraPrevious |
//! | C401 StyleChange12 | @6044 -> ChangeKey_Camera_Tsume_Start @7101 -> @4498 | perf (else Cancel); trace (1058.20, 38.73, 214.83) clear | "Tsume_Start_Cmaera" @4914 -> @2811 | Tsume_Start | 0.4 | 0.33, bEndTakeover | takeover, yaw always, zero frame |
//! | C415 (COMBO_CL's finisher 0x8C) | Tornado crusher @7106 -> @1644 | perf; trace (-470, 330, 100) clear | "Tornado_crusher" @2113 -> @1104 | mi_Wscissors | 0.3 | 0.3, bEndTakeover | takeover, yaw always, zero frame |
//! | 4418 (COMBO_YO) | mi_camera_walk_the_dog @7292 -> @3506 | perf; trace (-437.86, 202.54, 320.65) clear | "mi_camera_walk_the_dog" @2175 | walk_the_dog | 0.4 | 0.33, bEndTakeover | takeover, yaw ONCE (no influence call), zero frame |
//! | 4420 FINISH_YO | YoYO_spiral_shredder @7297: TresWaitCycle(1) -> @15 | no perf test; trace (826.38, 881.08, 157.12): hit -> _ver2 (@430), clear -> YoYO_spiral_shredder (@1006) | "mi_camera_walk_the_dog" @449 | YoYO_spiral_shredder(_ver2) | 0.4 | 0.33, bEndTakeover | takeover, yaw always, zero frame; started ONE FRAME after the event |
//! | 4481 | Yoyo_Gate_Start @7374 -> @3975 | none | "mi_camera_walk_the_dog" | YoYO_Gate | cut | 0.33, bEndTakeover | yaw always @2761, zero frame @2727 |
//! | 4483 | Yoyo_Gate_End @7379 | | | | | EndAllSameBPTresCamera, a cut | |
//!
//! ReceivePlayerDtorState @7460: STYLECHANGE 31, WEAPONCHANGE 32, STYLEFINISH 33, COMBO_YO 77, COMBO_CL 85 -> @7226: the last
//! started camera (`cam_anim`) `EndTresCamera(false, {0, 3, 2})` = a cut. No ReceivePlayerCtorState, no end event.
//!
//! # fz_key (ReceiveAnimNotifyStartBpEvent @7803, SendActor = Sora)
//! | seq | entry | test | camera | anim | in | end | other |
//! |---|---|---|---|---|---|---|---|
//! | 7401 StyleChange7 | @6576 -> fz-key_change @10598 -> @9730 | perf (else @6648 Cancel); traces (127.58, -603.82, 340.46) @9973, (-372.69, -36.26, 212.14) @9278, (570.54, -41.12, 71.96) @8757 clear | "Po_2Shot__Start_Cmaera" @8930 -> @4719 (On_Skill) | fz-key_change | 0.33 | a cut | takeover, yaw always, zero frame; WaitPlayEnd -> @1065: the back camera |
//! | I401 StyleChange18 | @6705 -> fz_Crow_change @10781 | perf (else Cancel); traces (-152.88, -515.37, 142.12) @11024, (-708.92, 154.65, 32.80) @11440, (-467.96, 584.93, -62.59) @10409 clear | "ca_flag_combo_finish" @10582 -> @5387 (On_Skill) | fz-key_crow-chage | cut | a cut | yaw ONCE (`InfluenceRotation(false, ..)` @5846), zero frame; WaitPlayEnd -> @1843: ResetTresCameraNormal(true) |
//! | 7416 (SK ground finisher) | skert_ground_finish @8456 | perf | "Po_2Shot__Start_Cmaera" @7672 -> @6036 | skert_ground_finish | 0.33 | 0.33, bEndTakeover | takeover, yaw always, zero frame |
//! | 7418 (SK air finisher) | skert_sky_finish @8451 -> @7734 | perf | "PoBazooka_Start_Cmaera" @3424 (On_Skill) | skert_sky_finish | 0.33 | a cut | takeover, SetTargetBoneName("trans"), yaw always, zero frame; WaitPlayEnd -> @287: the back camera |
//! | 7419 FINISH_SK (M_7419) | fz_Finish01 @8946 -> @2399 (FZ_FINISH = true @8092) | none; trace (-137.77, -72.92, 956.998) @3139: hit -> ATARI (a ceiling) | "fz_finish02" @2399, then "fz_finish01" @3322 ON TOP | Fz_FInish_Cam02 / Fz_FInish_Cam01 (9 s each) | cut | a cut | both `InfluenceLocationXYZ(false, false)`: his place taken once; yaw once; no zero frame |
//! | 7423 Param 1 (window 0 .. 5.633) | Fz_FInish01_End @10761 | FZ_FINISH, ATARI | | | | | the ceiling look (below) |
//! | 7423 Param 0 (window 3.673 .. 5.633) | fz_Finish02 @8951 | FZ_FINISH | | | | Cam01 `EndTresCamera` cut: Cam02 shows | |
//! | 7481 | fz_Rambu @10146 -> @4141 | none | "ca_flag_combo_finish" | Ca_Flag_ranbu | cut | 0.33 | yaw always, zero frame |
//!
//! THE BACK CAMERA (@1065 / @287, only while On_Skill): `GetTresCameraNormal().ResetCamera(true)` (the field camera behind
//! him), `BP_SpawnTresCameraRotTarget("back_camera")` on Sora with `SetTresCameraRotTargetParam(500, R(-10, 0, 0), 0, 0)`,
//! `StartTresCamera(false, {0, 3, 2})` (a cut), priority 200, `TresWaitCycle(1)` -> @15 / @151
//! `EndAllBPTresCameraOfName("back_camera", {0.5, 3 ease out, 2, lock})`, On_Skill = false: shown one frame, then 0.5 s back
//! to the reset field camera.
//! ReceiveAnimNotifyEndBpEvent @10603: 7423 -> Fz_FInish02_End @10776 -> @10151 (ATARI: the ceiling look undone).
//! ReceivePlayerDtorState @11613: STYLECHANGE 31, WEAPONCHANGE 32, STYLEFINISH 33, STYLEFINISH_SK 35 -> @18641: FZ_FINISH and
//! On_Skill false, `EndAllSameBPTresCamera` (a cut), the actors visible and moving again, the particle faded.
//!
//! THE CEILING LOOK (ATARI, Fz_FInish01_End @9451 .. @7035; undone @10151 .. @7243): `SetActorVisibleOff(2, ..)` /
//! `SetActorStopOn(2, ..)` (the level's actors), both finish cameras' collision off (@7244 / @7281), the particle
//! `vw_so073_fin_fsg0` at his location (@7411), `DisableLight(true, 1)`, `r.ReflectionEnvironment 0`, the post-process actor
//! `postprocess_Rapunzel_tower`. The cameras' part (collision off) is ported; the rest is published as [`ClawsCameras::ceiling`]
//! and its edges as `FormBpOutput::tower` ((AUDIT-B W-FIN) the level hide / lights / post-process: kh-er-render bluehour.rs
//! `set_tower`, from kh-er cam.rs and the harness; the particle: kh-core states/bp_event.rs `ceiling_look`).
//!
//! [I] / limits: `SetTargetBoneName("trans")` (7418): the base is his actor location (the frame carries no bones); the back
//! camera's FOV is the RotTarget's component default 90 and it takes the collision pass (it makes no EnableCollision call);
//! bTakeover at a start only seeds what the anim overwrites (form_bp.rs's note). `RemoteEvent("Cnage_Key_Cancel")` goes to the
//! level script: no listener in the port (logged).

use crate::camera_anim::{camera_anim, collide, frame_target, sweep_start, AnimView, CameraAnimDef, Transition};
use crate::form_bp::{FormBpFrame, FormBpOutput};
use crate::Rot;
use glam::Vec3;
use kh_core::host::EventKind;
use kh_core::{StateId, World};

/// The two blueprints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// BP_TresPlayerStateEventCamera_mi_key (Happy Gear).
    Mi,
    /// BP_TresPlayerStateEventCamera_fz_key (Crystal Snow).
    Fz,
}

/// {time, ease in-out (4), exp 2, bLockOutgoing true}.
pub const fn eased(time: f32) -> Transition {
    Transition::new(time, 4, 2.0, true)
}

/// `EndTresCamera` / `EndAllSameBPTresCamera`'s {0, ease out (3), 2}: a cut.
pub const CUT_OUT: Transition = Transition::new(0.0, 3, 2.0, false);
/// The back camera's end {0.5, ease out, 2, lock} (@15 / @151).
pub const BACK_OUT: Transition = Transition::new(0.5, 3, 2.0, true);
/// `SetTresCameraRotTargetParam(500, R(-10, 0, 0), ..)`.
pub const BACK_DISTANCE: f32 = 500.0;
pub const BACK_PITCH: f32 = -10.0;
/// [I] the RotTarget's component default.
pub const BACK_FOV: f32 = 90.0;

/// The traces (his transform: x forward, y right, z up).
pub const TRACE_4401: [Vec3; 2] = [Vec3::new(352.628_27, 646.074_7, 470.415_65), Vec3::new(1030.0, 3.532_631, 175.305_63)];
pub const TRACE_C401: Vec3 = Vec3::new(1058.204, 38.727_95, 214.831_09);
pub const TRACE_C415: Vec3 = Vec3::new(-470.0, 330.0, 100.0);
pub const TRACE_4418: Vec3 = Vec3::new(-437.856_75, 202.535_7, 320.649_05);
pub const TRACE_4420: Vec3 = Vec3::new(826.381_6, 881.077_45, 157.123_02);
pub const TRACE_7401: [Vec3; 3] = [Vec3::new(127.580_93, -603.815_7, 340.463_32), Vec3::new(-372.690_12, -36.264_587, 212.143_3), Vec3::new(570.540_8, -41.121_094, 71.959_67)];
pub const TRACE_I401: [Vec3; 3] = [Vec3::new(-152.88, -515.371_34, 142.116_47), Vec3::new(-708.922_36, 154.646_85, 32.801_52), Vec3::new(-467.960_5, 584.931_15, -62.593_85)];
pub const TRACE_7419: Vec3 = Vec3::new(-137.773_83, -72.918_39, 956.998_05);

#[derive(Clone, Copy, Debug, PartialEq)]
enum Shot {
    Anim { def: &'static CameraAnimDef, time: f32, zero_frame: bool },
    /// The RotTarget "back_camera".
    Back,
}

/// What a camera's WaitPlayEnd continues with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OnEnd {
    /// mi @1070 ResetTresCameraPrevious.
    ResetPrevious,
    /// fz @1065 / @287 (On_Skill): the back camera.
    BackCamera,
    /// fz @1843 (On_Skill): ResetTresCameraNormal(true).
    ResetNormal,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Cam {
    id: u32,
    name: &'static str,
    key: Key,
    shot: Shot,
    auto_end: bool,
    end: Transition,
    end_takeover: bool,
    xy_always: bool,
    z_always: bool,
    yaw_always: bool,
    collision: bool,
    on_end: Option<OnEnd>,
    base: Option<(Vec3, f32)>,
}

impl Cam {
    fn view(&mut self, sora: (Vec3, f32)) -> AnimView {
        let (loc, yaw) = sora;
        let base = match self.base {
            Some((p, y)) => {
                let xy = if self.xy_always { (loc.x, loc.y) } else { (p.x, p.y) };
                (Vec3::new(xy.0, xy.1, if self.z_always { loc.z } else { p.z }), if self.yaw_always { yaw } else { y })
            }
            None => (loc, yaw),
        };
        self.base = Some(base);
        match self.shot {
            Shot::Anim { def, time, .. } => {
                let s = def.sample(time);
                let (sn, cs) = base.1.to_radians().sin_cos();
                let p = s.location;
                AnimView { location: base.0 + Vec3::new(p.x * cs - p.y * sn, p.x * sn + p.y * cs, p.z), rotation: Rot { pitch: s.rotation.pitch, yaw: s.rotation.yaw + base.1, roll: s.rotation.roll }.normalized(), fov: s.fov }
            }
            Shot::Back => {
                // look point = his location (no offsets); rotation = his yaw (once) + m_Rotation (-10, 0, 0)
                let rotation = Rot::new(BACK_PITCH, base.1, 0.0).normalized();
                AnimView { location: base.0 - rotation.forward() * BACK_DISTANCE, rotation, fov: BACK_FOV }
            }
        }
    }
}

/// A started camera's settings.
#[derive(Clone, Copy, Debug)]
struct Start {
    name: &'static str,
    anim: &'static str,
    blend_in: Transition,
    end: Transition,
    end_takeover: bool,
    yaw_always: bool,
    zero_frame: bool,
    on_end: Option<OnEnd>,
}

/// A `TresWaitCycle(1)` continuation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Latent {
    /// mi @7297 -> @15: FINISH_YO's camera.
    Spiral,
    /// fz @15 / @151: the back camera's end.
    BackEnd,
}

/// The cameras of the two blueprints (one per Sora).
#[derive(Clone, Debug, Default)]
pub struct ClawsCameras {
    cams: Vec<Cam>,
    next_id: u32,
    blend: Option<(AnimView, Transition, f32)>,
    last: Option<AnimView>,
    state: Option<StateId>,
    latent: Vec<(u32, Latent)>,
    /// mi's / fz's `cam_anim` (the camera the dtor / fz_Finish02 end).
    mi_cam: Option<u32>,
    fz_cam: Option<u32>,
    /// fz's Fz_FInish_Cam02 (its collision is switched off by the ceiling look with Cam01's).
    fz_cam2: Option<u32>,
    on_skill: bool,
    fz_finish: bool,
    atari: bool,
    /// THE CEILING LOOK of FINISH_SK is on (fz ATARI: the level hidden and stopped, the lights off, vw_so073_fin_fsg0,
    /// postprocess_Rapunzel_tower): for the host; the cameras' part is done here.
    pub ceiling: bool,
    pending: FormBpOutput,
    /// A readable log (tests, traces).
    pub log: Vec<String>,
}

impl ClawsCameras {
    pub fn new() -> ClawsCameras {
        ClawsCameras::default()
    }

    pub fn active(&self) -> bool {
        !self.cams.is_empty() || self.blend.is_some()
    }

    /// The running cameras (name, anim), oldest first.
    pub fn cameras(&self) -> Vec<(&'static str, &'static str)> {
        self.cams
            .iter()
            .map(|c| {
                (
                    c.name,
                    match c.shot {
                        Shot::Anim { def, .. } => def.name,
                        Shot::Back => "back_camera",
                    },
                )
            })
            .collect()
    }

    fn switch(&mut self, t: Transition) {
        self.blend = match (self.last, t.time > 0.0) {
            (Some(from), true) => Some((from, t, 0.0)),
            _ => None,
        };
    }

    /// A camera on top (its id is given here), coming in with `blend_in`.
    fn push(&mut self, cam: Cam, blend_in: Transition) -> u32 {
        self.switch(blend_in);
        self.next_id += 1;
        let id = self.next_id;
        self.cams.push(Cam { id, ..cam });
        id
    }

    /// A camera's defaults: on Sora, autoEnd, his place every frame, the collision pass.
    fn cam(key: Key, name: &'static str, shot: Shot, end: Transition, end_takeover: bool, yaw_always: bool) -> Cam {
        Cam { id: 0, name, key, shot, auto_end: true, end, end_takeover, xy_always: true, z_always: true, yaw_always, collision: true, on_end: None, base: None }
    }

    fn start(&mut self, key: Key, s: Start) -> Option<u32> {
        let Some(def) = camera_anim(s.anim) else {
            self.log.push(format!("{} is not in the generated camera tables (node tools/party_team_pack.js --cameras)", s.anim));
            return None;
        };
        self.log.push(format!("start {} {}", s.name, s.anim));
        let cam = Cam { on_end: s.on_end, ..Self::cam(key, s.name, Shot::Anim { def, time: 0.0, zero_frame: s.zero_frame }, s.end, s.end_takeover, s.yaw_always) };
        let id = self.push(cam, s.blend_in);
        match key {
            Key::Mi => self.mi_cam = Some(id),
            Key::Fz => self.fz_cam = Some(id),
        }
        Some(id)
    }

    fn end_where(&mut self, t: Transition, why: &str, pred: impl Fn(&Cam) -> bool) {
        let top = self.cams.last().map(|c| c.id);
        let before = self.cams.len();
        self.cams.retain(|c| !pred(c));
        if self.cams.len() != before {
            self.log.push(format!("end {} camera(s) ({why})", before - self.cams.len()));
            if self.cams.last().map(|c| c.id) != top {
                self.switch(t);
            }
        }
    }

    fn clear(world: Option<&dyn World>, sora: (Vec3, f32), local: Vec3) -> bool {
        let Some(w) = world else { return true };
        let (loc, yaw) = sora;
        let (s, c) = yaw.to_radians().sin_cos();
        let to = loc + Vec3::new(local.x * c - local.y * s, local.x * s + local.y * c, local.z);
        w.line_trace(loc + Vec3::Z * 100.0, to).is_none()
    }

    fn on_event(&mut self, ev: &EventKind, frame: &FormBpFrame, world: Option<&dyn World>) {
        match *ev {
            EventKind::StateChange { from, to } => {
                self.dtor(from);
                self.state = Some(to);
            }
            EventKind::AnimBpEvent { seq, param, begin: true } => self.notify_start(seq, param, frame, world),
            EventKind::AnimBpEvent { seq, begin: false, .. } => self.notify_end(seq),
            _ => {}
        }
    }

    fn dtor(&mut self, from: StateId) {
        let s = from as i32;
        // mi @7460 -> @7226 / @7111: cam_anim EndTresCamera, a cut
        if matches!(s, 31 | 32 | 33 | 77 | 85) {
            if let Some(id) = self.mi_cam.take() {
                self.end_where(CUT_OUT, "mi dtor: EndTresCamera(cam_anim)", |c| c.id == id);
            }
        }
        // fz @11613 -> @18641
        if matches!(s, 31 | 32 | 33 | 35) {
            (self.fz_finish, self.on_skill) = (false, false);
            self.end_where(CUT_OUT, "fz dtor: EndAllSameBPTresCamera", |c| c.key == Key::Fz);
            if std::mem::take(&mut self.ceiling) {
                self.pending.tower = Some(false); // (AUDIT-B W-FIN) the renderer's look off with it
                self.log.push("ceiling look off (dtor)".to_string());
            }
        }
    }

    fn notify_start(&mut self, seq: &str, param: i32, frame: &FormBpFrame, world: Option<&dyn World>) {
        let perf = frame.perf.enabled(self.state.unwrap_or(frame.state));
        let sora = frame.sora;
        let all_clear = |pts: &[Vec3]| pts.iter().all(|p| Self::clear(world, sora, *p));
        let s = |name, anim, blend_in: f32, end: Transition, end_takeover, yaw_always, on_end| Start { name, anim, blend_in: eased(blend_in), end, end_takeover, yaw_always, zero_frame: true, on_end };
        let seq = seq.to_ascii_uppercase();
        match seq.as_str() {
            // ---- mi_key
            "4401" | "C401" if !perf => self.log.push(format!("{seq}: RemoteEvent Cnage_Key_Cancel (no listener)")),
            "4401" if all_clear(&TRACE_4401) => {
                self.start(Key::Mi, s("YoYo_Start_Cmaera", "Yo-Yo_Start", 0.4, eased(0.33), true, true, Some(OnEnd::ResetPrevious)));
            }
            "C401" if all_clear(&[TRACE_C401]) => {
                self.start(Key::Mi, s("Tsume_Start_Cmaera", "Tsume_Start", 0.4, eased(0.33), true, true, None));
            }
            "C415" if perf && all_clear(&[TRACE_C415]) => {
                self.start(Key::Mi, s("Tornado_crusher", "mi_Wscissors", 0.3, eased(0.3), true, true, None));
            }
            "4418" if perf && all_clear(&[TRACE_4418]) => {
                self.start(Key::Mi, s("mi_camera_walk_the_dog", "walk_the_dog", 0.4, eased(0.33), true, false, None));
            }
            "4420" => {
                self.log.push("4420: TresWaitCycle(1)".to_string());
                self.latent.push((1, Latent::Spiral));
            }
            "4481" => {
                self.start(Key::Mi, s("mi_camera_walk_the_dog", "YoYO_Gate", 0.0, eased(0.33), true, true, None));
            }
            "4483" => self.end_where(CUT_OUT, "4483: EndAllSameBPTresCamera", |c| c.key == Key::Mi),
            // ---- fz_key
            "7401" | "I401" if !perf => self.log.push(format!("{seq}: RemoteEvent Cnage_Key_Cancel (no listener)")),
            "7401" if all_clear(&TRACE_7401) => {
                self.on_skill = true;
                self.start(Key::Fz, s("Po_2Shot__Start_Cmaera", "fz-key_change", 0.33, CUT_OUT, false, true, Some(OnEnd::BackCamera)));
            }
            "I401" if all_clear(&TRACE_I401) => {
                self.on_skill = true;
                self.start(Key::Fz, s("ca_flag_combo_finish", "fz-key_crow-chage", 0.0, CUT_OUT, false, false, Some(OnEnd::ResetNormal)));
            }
            "7416" if perf => {
                self.start(Key::Fz, s("Po_2Shot__Start_Cmaera", "skert_ground_finish", 0.33, eased(0.33), true, true, None));
            }
            "7418" if perf => {
                self.on_skill = true;
                self.start(Key::Fz, s("PoBazooka_Start_Cmaera", "skert_sky_finish", 0.33, CUT_OUT, false, true, Some(OnEnd::BackCamera)));
            }
            "7419" => {
                self.fz_finish = true;
                let finish = |name, anim| Start { name, anim, blend_in: eased(0.0), end: eased(0.0), end_takeover: false, yaw_always: false, zero_frame: false, on_end: None };
                // @2399 Fz_FInish_Cam02 first, @3322 Fz_FInish_Cam01 over it; both InfluenceLocationXYZ(false, false)
                self.fz_cam2 = self.start(Key::Fz, finish("fz_finish02", "Fz_FInish_Cam02"));
                self.atari = !Self::clear(world, sora, TRACE_7419);
                if self.atari {
                    self.log.push("7419: ATARI (a ceiling over him)".to_string());
                }
                self.start(Key::Fz, finish("fz_finish01", "Fz_FInish_Cam01"));
                for c in self.cams.iter_mut().filter(|c| matches!(c.name, "fz_finish01" | "fz_finish02")) {
                    (c.xy_always, c.z_always) = (false, false);
                }
            }
            "7423" if self.fz_finish => {
                if param == 0 {
                    // fz_Finish02 @8951: cam_anim (Fz_FInish_Cam01) EndTresCamera, a cut
                    if let Some(id) = self.fz_cam.take() {
                        self.end_where(CUT_OUT, "7423 (param 0): EndTresCamera(Fz_FInish_Cam01)", |c| c.id == id);
                    }
                } else if self.atari {
                    // Fz_FInish01_End -> @9451 .. @7281
                    self.ceiling = true;
                    self.pending.tower = Some(true); // (AUDIT-B W-FIN) the level / light / post-process half: the host's
                    self.log.push("ceiling look on".to_string());
                    let (a, b) = (self.fz_cam, self.fz_cam2);
                    for c in self.cams.iter_mut().filter(|c| Some(c.id) == a || Some(c.id) == b) {
                        c.collision = false;
                    }
                }
            }
            "7481" => {
                self.start(Key::Fz, s("ca_flag_combo_finish", "Ca_Flag_ranbu", 0.0, eased(0.33), false, true, None));
            }
            // the branches above that a trace (or the performance camera) held back
            "4401" | "C401" | "C415" | "4418" | "7401" | "I401" | "7416" | "7418" => self.log.push(format!("{seq}: {}: no camera", if perf { "a trace hit" } else { "no performance camera" })),
            _ => {}
        }
    }

    fn notify_end(&mut self, seq: &str) {
        // fz ReceiveAnimNotifyEndBpEvent @10603: 7423 -> Fz_FInish02_End -> @10151
        if seq == "7423" && self.atari {
            if self.ceiling {
                self.pending.tower = Some(false); // (AUDIT-B W-FIN)
            }
            (self.atari, self.fz_finish, self.ceiling) = (false, false, false);
            self.log.push("ceiling look off".to_string());
        }
    }

    fn run_latent(&mut self, l: Latent, frame: &FormBpFrame, world: Option<&dyn World>) {
        match l {
            Latent::Spiral => {
                let anim = if Self::clear(world, frame.sora, TRACE_4420) { "YoYO_spiral_shredder" } else { "YoYO_spiral_shredder_ver2" };
                self.start(Key::Mi, Start { name: "mi_camera_walk_the_dog", anim, blend_in: eased(0.4), end: eased(0.33), end_takeover: true, yaw_always: true, zero_frame: true, on_end: None });
            }
            Latent::BackEnd => {
                self.end_where(BACK_OUT, "EndAllBPTresCameraOfName back_camera", |c| c.name == "back_camera");
                self.on_skill = false;
            }
        }
    }

    /// One step: the step's core events (in order), advance the cameras, the view to draw.
    pub fn step(&mut self, dt: f32, events: &[kh_core::Event], frame: &FormBpFrame, world: Option<&dyn World>) -> FormBpOutput {
        // TresWaitCycle(1): the frame after
        let mut due = Vec::new();
        self.latent.retain_mut(|(n, l)| {
            *n = n.saturating_sub(1);
            if *n == 0 {
                due.push(*l);
            }
            *n > 0
        });
        for l in due {
            self.run_latent(l, frame, world);
        }
        for e in events {
            self.on_event(&e.kind, frame, world);
        }
        self.state = Some(frame.state);
        // advance; bAutoEnd at the anim's end
        let mut ended = Vec::new();
        for c in self.cams.iter_mut() {
            if let Shot::Anim { def, time, zero_frame } = &mut c.shot {
                let step = if std::mem::take(zero_frame) { 0.0 } else { dt };
                let before = *time;
                *time = (*time + step).min(def.length);
                if c.auto_end && *time >= def.length && before < def.length {
                    ended.push(c.id);
                }
            }
        }
        for id in ended {
            let Some(i) = self.cams.iter().position(|c| c.id == id) else { continue };
            let c = self.cams.remove(i);
            self.log.push(format!("auto end {} ({})", c.name, self.cameras_text()));
            if i == self.cams.len() {
                self.switch(c.end);
                if c.end_takeover && self.cams.is_empty() {
                    self.pending.takeover = self.last;
                }
            }
            match c.on_end {
                Some(OnEnd::ResetPrevious) => self.pending.reset_field_camera = true,
                Some(OnEnd::ResetNormal) if self.on_skill => {
                    self.pending.reset_field_camera = true;
                    self.on_skill = false;
                }
                Some(OnEnd::BackCamera) if self.on_skill => {
                    self.pending.reset_field_camera = true;
                    self.log.push("back_camera (500, pitch -10), a cut".to_string());
                    self.push(Self::cam(Key::Fz, "back_camera", Shot::Back, CUT_OUT, false, false), CUT_OUT);
                    self.latent.push((1, Latent::BackEnd));
                }
                _ => {}
            }
        }
        // the top camera's view, with the collision pass
        let sora = frame.sora;
        let top = self.cams.last_mut().map(|c| {
            let v = c.view(sora);
            match (world, c.collision) {
                (Some(w), true) => match frame_target(&v, sora.0, w) {
                    Some(f) if f.distance > 0.0 => {
                        let start = f.location + f.rotation.forward() * f.distance;
                        AnimView { location: collide(start, f.location, w), rotation: f.rotation, ..v }
                    }
                    _ => AnimView { location: collide(sweep_start(&v, sora.0), v.location, w), ..v },
                },
                _ => v,
            }
        });
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
        let view = if self.cams.is_empty() && self.blend.is_none() { None } else { view };
        self.last = Some(view.unwrap_or(frame.field));
        let mut out = std::mem::take(&mut self.pending);
        out.view = view;
        out
    }

    fn cameras_text(&self) -> String {
        self.cameras().iter().map(|(n, a)| format!("{n}:{a}")).collect::<Vec<_>>().join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form_bp::PerfCamera;
    use kh_core::host::Event;
    use kh_core::world::Hit;

    /// A ceiling / walls for every line trace.
    struct Walled;
    impl World for Walled {
        fn sweep_capsule(&self, _: Vec3, _: Vec3, _: f32, _: f32) -> Option<Hit> {
            None
        }
        fn line_trace(&self, a: Vec3, _: Vec3) -> Option<Hit> {
            Some(Hit { time: 0.5, position: a, normal: Vec3::X, impact_normal: Vec3::X, start_penetrating: false, penetration_depth: 0.0, component: 0 })
        }
    }

    const DT: f32 = 1.0 / 60.0;

    fn frame(state: StateId) -> FormBpFrame {
        FormBpFrame { sora: (Vec3::new(100.0, 0.0, 0.0), 30.0), field: AnimView { location: Vec3::new(-400.0, 0.0, 200.0), rotation: Rot::default(), fov: 67.0 }, state, ultima: false, perf: PerfCamera::default() }
    }

    fn bp(seq: &'static str, param: i32, begin: bool) -> Vec<Event> {
        vec![Event { step: 0, kind: EventKind::AnimBpEvent { seq, param, begin } }]
    }

    fn dtor(from: StateId, to: StateId) -> Vec<Event> {
        vec![Event { step: 0, kind: EventKind::StateChange { from, to } }]
    }

    fn anims(c: &ClawsCameras) -> Vec<&'static str> {
        c.cameras().into_iter().map(|(_, a)| a).collect()
    }

    fn tables() -> bool {
        let ok = camera_anim("YoYO_spiral_shredder").is_some() && camera_anim("Fz_FInish_Cam01").is_some();
        if !ok {
            eprintln!("SKIPPED: no generated camera tables (node tools/party_team_pack.js --cameras)");
        }
        ok
    }

    /// Every branch picks its anim, camera name and transitions; the trace and perf tests hold them back.
    #[test]
    fn each_branch_starts_its_camera() {
        if !tables() {
            return;
        }
        let cases: [(&'static str, StateId, &str, &str, f32, f32, bool); 9] = [
            ("4401", StateId::Stylechange, "YoYo_Start_Cmaera", "Yo-Yo_Start", 0.4, 0.33, true),
            ("C401", StateId::Stylechange, "Tsume_Start_Cmaera", "Tsume_Start", 0.4, 0.33, true),
            ("C415", StateId::ComboCl, "Tornado_crusher", "mi_Wscissors", 0.3, 0.3, true),
            ("4418", StateId::ComboYo, "mi_camera_walk_the_dog", "walk_the_dog", 0.4, 0.33, true),
            ("4481", StateId::ComboYo, "mi_camera_walk_the_dog", "YoYO_Gate", 0.0, 0.33, true),
            ("7401", StateId::Stylechange, "Po_2Shot__Start_Cmaera", "fz-key_change", 0.33, 0.0, false),
            ("I401", StateId::Stylechange, "ca_flag_combo_finish", "fz-key_crow-chage", 0.0, 0.0, false),
            ("7416", StateId::ComboSk, "Po_2Shot__Start_Cmaera", "skert_ground_finish", 0.33, 0.33, true),
            ("7481", StateId::ComboSk, "ca_flag_combo_finish", "Ca_Flag_ranbu", 0.0, 0.33, false),
        ];
        for (seq, state, name, anim, blend_in, end, takeover) in cases {
            let mut c = ClawsCameras::new();
            let f = frame(state);
            c.step(DT, &[], &f, None);
            let v = c.step(DT, &bp(seq, 0, true), &f, None).view.unwrap_or_else(|| panic!("{seq}"));
            assert_eq!(c.cameras(), [(name, anim)], "{seq}");
            let cam = c.cams[0];
            assert_eq!((cam.end.time, cam.end_takeover), (end, takeover), "{seq} end");
            // the blend in: a cut shows the anim's frame 0 at once, a blend starts at the field camera
            let def = camera_anim(anim).unwrap();
            let s = def.sample(0.0);
            let yaw = 30f32;
            let (sn, cs) = yaw.to_radians().sin_cos();
            let want = Vec3::new(100.0, 0.0, 0.0) + Vec3::new(s.location.x * cs - s.location.y * sn, s.location.x * sn + s.location.y * cs, s.location.z);
            if blend_in == 0.0 {
                assert!((v.location - want).length() < 1e-2, "{seq}: frame 0 at once {v:?} {want:?}");
            } else {
                assert!((v.location - f.field.location).length() < (want - f.field.location).length(), "{seq}: blending in");
                let mut done = None;
                for k in 0..120 {
                    let v = c.step(DT, &[], &f, None).view.unwrap();
                    if c.blend.is_none() {
                        done = Some((k + 2) as f32 * DT);
                        let _ = v;
                        break;
                    }
                }
                let t = done.unwrap_or(9.0);
                assert!((t - blend_in).abs() < 2.5 * DT, "{seq}: {blend_in} s blend in, took {t}");
            }
            // the walls: the traced branches start nothing
            let mut w = ClawsCameras::new();
            let traced = !matches!(seq, "4481" | "7416" | "7481");
            assert_eq!(w.step(DT, &bp(seq, 0, true), &f, Some(&Walled)).view.is_none(), traced, "{seq} with walls");
            // perf off ("Keyblade Cinematics" Basic / Combo Cam Off): the perf branches start nothing
            let off = FormBpFrame { perf: PerfCamera { combo_cam: 2, keyblade_cinematics: false, restricted: false }, ..f };
            let mut p = ClawsCameras::new();
            let gated = !matches!(seq, "4481" | "7481");
            assert_eq!(p.step(DT, &bp(seq, 0, true), &off, None).view.is_none(), gated, "{seq} without the performance camera");
            if matches!(seq, "4401" | "C401" | "7401" | "I401") {
                assert!(p.log.iter().any(|l| l.contains("Cnage_Key_Cancel")), "{seq}: {:?}", p.log);
            }
        }
    }

    /// FINISH_YO (4420): one frame later YoYO_spiral_shredder (a wall in the trace: _ver2), 0.4 s in, the yaw following him,
    /// autoEnd at 5.067 s with 0.33 s back and the field camera taking over; STYLEFINISH's dtor would cut it.
    #[test]
    fn finish_yo_spiral_shredder_runs_its_path() {
        if !tables() {
            return;
        }
        let f = frame(StateId::Stylefinish);
        let mut c = ClawsCameras::new();
        assert!(c.step(DT, &bp("4420", 0, true), &f, None).view.is_none(), "TresWaitCycle(1): nothing the frame of the event");
        c.step(DT, &[], &f, None);
        assert_eq!(anims(&c), ["YoYO_spiral_shredder"]);
        let mut w = ClawsCameras::new();
        w.step(DT, &bp("4420", 0, true), &f, Some(&Walled));
        w.step(DT, &[], &f, Some(&Walled));
        assert_eq!(anims(&w), ["YoYO_spiral_shredder_ver2"]);
        // the yaw follows him (InfluenceRotation(true))
        for _ in 0..40 {
            c.step(DT, &[], &f, None);
        }
        let turned = FormBpFrame { sora: (f.sora.0, 120.0), ..f };
        let a = c.step(DT, &[], &f, None).view.unwrap();
        let b = c.step(0.0, &[], &turned, None).view.unwrap();
        assert!(crate::normalize_axis(b.rotation.yaw - a.rotation.yaw - 90.0).abs() < 1.0, "{a:?} {b:?}");
        let mut took = None;
        let mut steps = 0;
        while c.active() && steps < 600 {
            took = took.or(c.step(DT, &[], &f, None).takeover);
            steps += 1;
        }
        let total = (steps + 42) as f32 * DT;
        assert!(took.is_some() && (total - (5.067 + 0.33)).abs() < 0.1, "{total} s, takeover {took:?}");
        // the dtor of STYLEFINISH cuts a running one
        let mut d = ClawsCameras::new();
        d.step(DT, &bp("4420", 0, true), &f, None);
        d.step(DT, &[], &f, None);
        let o = d.step(DT, &dtor(StateId::Stylefinish, StateId::Idle), &frame(StateId::Idle), None);
        assert!(o.view.is_none() && !d.active(), "a cut");
    }

    /// FINISH_SK: at 7419 Fz_FInish_Cam02 and Fz_FInish_Cam01 over it (cut in, his place taken once); at 7423's param-0 window
    /// (3.673 s) Cam01 is cut away and Cam02 shows from its own clock; Cam02 autoEnds with a cut at 9 s; the ceiling look on
    /// a ceiling; STYLEFINISH's dtor ends everything.
    #[test]
    fn finish_sk_cameras_run_their_path() {
        if !tables() {
            return;
        }
        let f = frame(StateId::Stylefinish);
        let mut c = ClawsCameras::new();
        let v0 = c.step(DT, &bp("7419", 0, true), &f, None).view.unwrap();
        assert_eq!(anims(&c), ["Fz_FInish_Cam02", "Fz_FInish_Cam01"]);
        let s = camera_anim("Fz_FInish_Cam01").unwrap().sample(DT);
        let (sn, cs) = 30f32.to_radians().sin_cos();
        let want = Vec3::new(100.0, 0.0, 0.0) + Vec3::new(s.location.x * cs - s.location.y * sn, s.location.x * sn + s.location.y * cs, s.location.z);
        assert!((v0.location - want).length() < 1e-2, "Cam01 on top, a cut: {v0:?} {want:?}");
        // his place taken once: he moves, the camera stays
        let moved = FormBpFrame { sora: (Vec3::new(400.0, 50.0, 300.0), 30.0), ..f };
        let a = c.step(0.0, &[], &f, None).view.unwrap();
        let b = c.step(0.0, &[], &moved, None).view.unwrap();
        assert!((a.location - b.location).length() < 1e-3);
        // M_7419 3.333 s, then M_7423: its param-1 window (no ceiling: nothing), its param-0 window at 3.673 s
        for _ in 0..199 {
            c.step(DT, &[], &f, None);
        }
        c.step(DT, &bp("7423", 1, true), &f, None);
        assert_eq!(anims(&c).len(), 2);
        for _ in 0..220 {
            c.step(DT, &[], &f, None);
        }
        let v = c.step(DT, &bp("7423", 0, true), &f, None).view.unwrap();
        assert_eq!(anims(&c), ["Fz_FInish_Cam02"], "Cam01 cut away");
        let t2 = match c.cams[0].shot {
            Shot::Anim { time, .. } => time,
            Shot::Back => 0.0,
        };
        assert!((t2 - (3.333 + 3.673)).abs() < 0.1, "Cam02 shows from its own clock: {t2}");
        let s2 = camera_anim("Fz_FInish_Cam02").unwrap().sample(t2);
        assert!((v.location - (Vec3::new(100.0, 0.0, 0.0) + Vec3::new(s2.location.x * cs - s2.location.y * sn, s2.location.x * sn + s2.location.y * cs, s2.location.z))).length() < 1e-2);
        let mut n = 0;
        while c.active() && n < 400 {
            c.step(DT, &[], &f, None);
            n += 1;
        }
        assert!(((n as f32) * DT - (9.0 - t2)).abs() < 0.05, "autoEnd at 9 s, a cut: {n}");
        // a ceiling over him: ATARI; 7423's param-1 window switches the ceiling look on (and both cameras' collision off), its
        // end switches it off
        let mut d = ClawsCameras::new();
        assert_eq!(d.step(DT, &bp("7419", 0, true), &f, Some(&Walled)).tower, None);
        // (AUDIT-B W-FIN) the host's half as the step's edges: on at the param-1 start, off at the end, once each
        assert_eq!(d.step(DT, &bp("7423", 1, true), &f, Some(&Walled)).tower, Some(true));
        assert!(d.ceiling && d.cams.iter().all(|c| !c.collision));
        assert_eq!(d.step(DT, &[], &f, Some(&Walled)).tower, None);
        assert_eq!(d.step(DT, &bp("7423", 1, false), &f, Some(&Walled)).tower, Some(false));
        assert!(!d.ceiling);
        assert_eq!(d.step(DT, &bp("7423", 0, false), &f, Some(&Walled)).tower, None);
        // (AUDIT-B W-FIN) the dtor in the middle of the look: the look off with it
        let mut g = ClawsCameras::new();
        g.step(DT, &bp("7419", 0, true), &f, Some(&Walled));
        assert_eq!(g.step(DT, &bp("7423", 1, true), &f, Some(&Walled)).tower, Some(true));
        assert_eq!(g.step(DT, &dtor(StateId::Stylefinish, StateId::Idle), &frame(StateId::Idle), Some(&Walled)).tower, Some(false));
        // no ceiling: no look
        let mut h = ClawsCameras::new();
        h.step(DT, &bp("7419", 0, true), &f, None);
        assert_eq!(h.step(DT, &bp("7423", 1, true), &f, None).tower, None);
        // the dtor of STYLEFINISH cuts everything
        let mut e = ClawsCameras::new();
        e.step(DT, &bp("7419", 0, true), &f, None);
        assert!(e.step(DT, &dtor(StateId::Stylefinish, StateId::Idle), &frame(StateId::Idle), None).view.is_none() && !e.active());
    }

    /// The SK air finisher's and Blizzard Blades' transformation's WaitPlayEnd: the field camera reset, the back camera one frame
    /// (500 cm, pitch -10, behind him), then 0.5 s back.
    #[test]
    fn the_back_camera_after_skert_sky_finish() {
        if !tables() {
            return;
        }
        let f = frame(StateId::ComboSk);
        let mut c = ClawsCameras::new();
        c.step(DT, &bp("7418", 0, true), &f, None);
        let mut reset = false;
        let mut back = None;
        for _ in 0..200 {
            let o = c.step(DT, &[], &f, None);
            reset |= o.reset_field_camera;
            if anims(&c) == ["back_camera"] {
                back = o.view;
                break;
            }
        }
        let v = back.expect("the back camera");
        assert!(reset);
        let r = Rot::new(BACK_PITCH, 30.0, 0.0);
        assert!((v.location - (Vec3::new(100.0, 0.0, 0.0) - r.forward() * BACK_DISTANCE)).length() < 1e-2, "{v:?}");
        c.step(DT, &[], &f, None);
        assert!(c.cameras().is_empty() && c.blend.is_some_and(|b| b.1 == BACK_OUT), "ended after one frame, 0.5 s ease out");
        // the mi dtor: COMBO_CL's end cuts the claws' finisher camera
        let mut m = ClawsCameras::new();
        m.step(DT, &bp("C415", 0, true), &frame(StateId::ComboCl), None);
        assert!(m.active());
        assert!(m.step(DT, &dtor(StateId::ComboCl, StateId::Idle), &frame(StateId::Idle), None).view.is_none());
    }
}
