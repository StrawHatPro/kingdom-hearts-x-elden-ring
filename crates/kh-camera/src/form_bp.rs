//! (AUDIT-C) SORA'S FORMCHANGE AND FINISHER CAMERAS OF THE STATE-EVENT CAMERA BLUEPRINTS, as data: the three
//! blueprints that film Light / Dark / Double Form, Ultimate Form and Rage Form (and Second Form's finishers, which share
//! the first). Read 2026-11-03 with build/sora_tmp/weapons/AUDIT-C/CAM/bpdis.js (listings in AUDIT-C/CAM/bp/); `@n` =
//! the ubergraph's bytecode offset. The CameraAnims are generated tables (tools/party_team_pack.js `--cameras`).
//!
//! # BP_TresPlayerStateEvent_form_change (Blueprints/Camera/p_ex001_StateEvent)
//! ReceiveAnimNotifyStartBpEvent (@4536, SendActor = Sora):
//! - 1401 (@837, StyleChange of Second Form / Light / Dark Form): perf -> change_limit_form (@6564 -> @4005): line trace
//!   from Sora + (0, 0, 100) to his local (300.33, -4.69, 72.64) (Camera channel); blocked: no camera. Clear: "none" on
//!   Sora (@973): EnableCollision, priority 200, StartTresCameraAnim(change_limit_form, bLoop false, bTakeover TRUE,
//!   in {0.6, ease in-out, 2, lock}, bAutoEnd, bEndTakeover false, end {0: a cut}); WaitPlayEnd -> @96: stop the shake.
//! - 1403 (@905, StyleChangeEnd): perf -> @6569: while that camera is the active one, ClientPlayCameraShake(
//!   shake_super_kokizami_1sec, scale 1). End of 1403 (@5532): stop the shake.
//! - 1421 / 1422 (stan_impact_land @5627 / _sky @6124): trace to (269.61, 61.52, 51.97); clear: stan_impact01 / 02,
//!   in 0.66, end 0.3, takeover both (@6062 -> @1627: "none", priority 100, Reset: WaitPlayEnd with no continuation).
//! - 1426 Sonic_rave (@6766): sonic_rave_01, in 0.33, end 0.3, takeover. 1416 magne_splash (@1487, perf): in 0.66,
//!   end 0.3, takeover. 1433 LastAlcanam_End (@6818): cut in / out, no takeover. 1413 ripple_drive (@2232, perf): the
//!   active camera's SetFieldOfView(120, 0.2), Delay(0.6) -> @139: SetFieldOfView(default, 1).
//! - D401 anti_begin (@2489, perf, StyleChange13): three traces to (424.41, -116.41, 241.57), (167.82, -520.02, 41.54),
//!   (143.78, 235.40, 71.46): any blocked = no camera; else "anti" (@321): priority 100, anti_change, cut in, autoEnd,
//!   no takeover, end cut.
//! - D417 anti_brust (@3871, perf, Crimson Blast): anti_brust, cut in, autoEnd, end cut.
//! - D418 anti_bomb (@6914, NO perf test, FINISH_AN): anti_bomb, cut in, autoEnd, takeover both, end 0.3.
//! ReceivePlayerDtorState (@6995): STYLECHANGE 31 -> @14027 stop the shake + @15 EndAllSameBPTresCamera(cut);
//! WEAPONCHANGE 32, STYLEFINISH 33, COMBO_AN 86, COMBO_LIMIT 90 -> @14074 EndAllSameBPTresCamera(cut).
//!
//! # BP_TresPlayerStateEventCamera_AltemaWep (change_key)
//! Start (@1582): only with BP_GetEquipWeaponID() == 15 (WEP_KEYBLADE14, Ultima Weapon) (@2386 -> @4656):
//! - 9401 CustoAaltema_change (@12971 -> @49): perf: trace to (533.51, 31.94, 25.18); clear: "LemyKey_Finish_Cam"
//!   (@666): EnableCollision, priority 200, Altema_Change_Cut01, cut in, no takeover, autoEnd, end cut; PlayZeroFrame
//!   (@1160); SetTresCameraTargetInfluenceRotation(bAlways, 0, yaw) (@1198); WaitPlayEnd -> @15
//!   ResetTresCameraPrevious (the camera under it, the field camera: vt+0x7A0(1) = Reset(snap), 0x140E63360). Perf
//!   off: RemoteEvent('Cnage_Key_Cancel') (@522, to the level script: no listener in the port's level or in ER).
//! - 9418 altema_yoko_finish (@4765 -> @1471): perf: altema_yoko_finish, cut in, no takeover (same body).
//! - 9419 altema_buki_finish (@1403 -> @12555): perf: trace to (-125, 300, -105); blocked = none; clear: trace to
//!   (515, -235, 535) (@12081); blocked = none; clear: altema_buki_finish, in {0.33, ease in-out, 2}, end cut.
//! ReceivePlayerCtorState (@4984): DAMAGE 19 -> @12000 EndAllSameBPTresCamera(cut). Dtor (@13068): STYLECHANGE 31,
//! WEAPONCHANGE 32, STYLEFINISH 33, SHOOTFLOW 41 -> @20096 EndAllSameBPTresCamera(cut).
//!
//! # BP_TresPlayerStateEventCamera_Oath_keeper (change_key; Double Form's Ultimate Finish)
//! - N424 (@4759 -> @3702): trace Sora + 100 -> Sora + (0, 0, 300). Blocked (@3140): "BlueHour_Camera01" BlueHour_Cut01,
//!   EnableCollision, priority 200, cut in, no autoEnd, end cut, BP_SetTresCameraAnimBaseOffset((0, 0, -300), R0).
//!   Clear (@4122): BlueHour_Cut01 the same without the offset, SetTresCameraTargetInfluenceLocationXYZ(bAlwaysXY true,
//!   bAlwaysZ false): the base height taken once (his N424 rise does not lift it).
//! - N425 (@2377 -> @10): "BlueHour_Camera02" BlueHour_Cut02B, EnableCollision(FALSE), priority 200, cut in, no autoEnd;
//!   EndTresCamera(the clear-case Camera01, cut) (@412); THE BLUE HOUR (@495 .. @1750): SetActorVisibleOff / StopOn
//!   (channel 2: map, background, gimmicks, projectiles, NPCs, prizes; NOT the player, friends, enemies),
//!   vw_so220_fin_fsg0 at his location (kh-core states/bp_event.rs), DisableLight(true, 1), r.ReflectionEnvironment 0,
//!   postprocess_BlueHour (IndirectLightingIntensity 0, priority 500; SkyLight: its cubemap, intensity 3, angle = 160 -
//!   camera yaw, RecaptureSky). -> `FormBpOutput::blue_hour`.
//! - End of N425 (@4956 -> @2439): EndTresCamera(Camera02, cut), the particle FadeOut(0), lights / reflections back, the
//!   post-process gone, actors visible / moving, GetTresCameraNormal().ResetCamera(true) (the field camera behind him).
//! - Dtor: STYLECHANGE 31, WEAPONCHANGE 32, STYLEFINISH 33 -> @12812 EndAllSameBPTresCamera(cut). (N481 BigBang_Cam is
//!   the Inception shotlock's: SHOT's.)
//!
//! # (AUDIT-C) BP_TresPlayerStateEvent_motion_notify (p_ex001_StateEvent; no ctor / dtor handler, no perf test)
//! Start (@1079, SendActor = Sora): 1050 (DiveFallAttack) -> "DiveFallAttack_camera" (@65): DiveFallAttack_camera,
//! EnableCollision, priority 200, cut in, autoEnd 1.5 s, end cut; WaitPlayEnd -> @30 ResetTresCameraNormal(true). 1176
//! (RailSlideStartS) -> "specialRail" (@579): SpecialRail_Camera, EnableCollision(false), priority 200, cut in, autoEnd
//! 0.8 s, bEndTakeover, end {0.33, ease in-out, 2}. 1582 (@1241): compared, nothing done. 0002 / 0107: no blueprint
//! compares them.
//!
//! # perf = BP_TresIsGameEnablePerformanceCamera (0x1422227D0 -> 0x140EAB300 on the user config block, +0xB58)
//! The state now (the pawn's +0x450): STYLECHANGE 0x1F -> the anim interface's vt+0x2C8 value in 0x99 / 0x9A, else
//! config byte 1 bit 2 = "Keyblade Cinematics" Normal (kh-core config.rs: byte 1 bit 2, inverted; default Normal);
//! STYLEFINISH 0x21 ..= 0x27 -> always; (the queued state is tested the same way once); else config byte 3 "Combo Cam":
//! Off (2) -> no; On (1) -> yes; Auto (0) -> unless pawn+0xE60 bit 19 (pawn vt+0x2798 0x14110E730, a scripted-fight
//! restriction). The config block layout (kh-core config.rs, GW+0x2E8, spec 10.8) matches bit for bit.
//!
//! # The CameraAnim (camera_anim.rs's notes): World = Anim x Offset x Base; base = Sora's actor location (X/Y always, Z
//! always unless `SetTresCameraTargetInfluenceLocationXYZ(_, false)`), base yaw once unless influenced always.
//! bTakeover at the start (0x1405D9AC0 -> 0x1405D9CC0): the anim composes at once (0x1405E0270(this, 0)), so the start
//! takeover only seeds what the anim overwrites. bEndTakeover (0x1405CA980, +0x468 bit 1, 0x140625CA0): the camera
//! under it takes over the view (location = the manager's view, rotation = the control rotation, current distance, FOV)
//! -> `FormBpOutput::takeover`. PlayZeroFrame (0x1405C64B0 / 0x1405DA000): the first tick after the start uses dt 0.
//! SetPriority: the field camera is 100 (ctor 0x1405C02C0 +0x3E8); these are 100 / 200: they lie over the field camera.

use crate::camera_anim::{camera_anim, camera_shake, collide, frame_target, sweep_start, AnimView, CameraAnimDef, Transition};
use crate::shake::{ShakeOffset, ShakeSet};
use crate::Rot;
use glam::Vec3;
use kh_core::host::EventKind;
use kh_core::StateId;

/// The blueprints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bp {
    FormChange,
    AltemaWep,
    OathKeeper,
    /// (AUDIT-C) BP_TresPlayerStateEvent_motion_notify (1050, 1176).
    MotionNotify,
}

/// `BP_TresIsGameEnablePerformanceCamera`'s inputs (see the module note).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PerfCamera {
    /// Config "Combo Cam": 0 Auto, 1 On, 2 Off (byte 3; default Auto).
    pub combo_cam: u8,
    /// Config "Keyblade Cinematics" = Normal (byte 1 bit 2 set; the default).
    pub keyblade_cinematics: bool,
    /// pawn+0xE60 bit 19 (scripted fights; never set in the port's worlds).
    pub restricted: bool,
}

impl Default for PerfCamera {
    fn default() -> PerfCamera {
        PerfCamera { combo_cam: 0, keyblade_cinematics: true, restricted: false }
    }
}

impl PerfCamera {
    /// 0x140EAB300 for the state the pawn is in.
    pub fn enabled(&self, state: StateId) -> bool {
        let s = state as i32;
        if s == 0x1F {
            return self.keyblade_cinematics;
        }
        if (0x21..0x28).contains(&s) {
            return true;
        }
        self.combo_cam != 2 && (self.combo_cam != 0 || !self.restricted)
    }
}

/// What the cameras follow, every step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FormBpFrame {
    /// Sora's actor location (feet) and yaw.
    pub sora: (Vec3, f32),
    /// The field camera's view now (blends from / to it).
    pub field: AnimView,
    /// The pawn's state at the end of the step.
    pub state: StateId,
    /// The equipped weapon is WEP_KEYBLADE14 (Ultima Weapon, `BP_GetEquipWeaponID() == 15`).
    pub ultima: bool,
    pub perf: PerfCamera,
}

/// One step's result.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct FormBpOutput {
    /// The view to draw (None = the field camera's own).
    pub view: Option<AnimView>,
    pub shake: ShakeOffset,
    /// ResetTresCameraPrevious / ResetCamera(true): the field camera behind Sora at once.
    pub reset_field_camera: bool,
    /// bEndTakeover: the field camera takes over this view (its rotation and distance from his look point).
    pub takeover: Option<AnimView>,
    /// The Blue Hour of N425 began (true) / ended (false) this step.
    pub blue_hour: Option<bool>,
    /// (AUDIT-B W-FIN) FINISH_SK's ceiling look (fz_key ATARI, kh-camera forms_claws `ceiling`) began (true) / ended (false)
    /// this step: the level hidden and stopped (SetActorVisibleOff / StopOn, CH_EVENT: map, background, gimmicks,
    /// projectiles, NPCs, prizes), the lights off (DisableLight(true, CH_PLAYER)), r.ReflectionEnvironment 0 and
    /// postprocess_Rapunzel_tower (IndirectLightingIntensity 0, its sky light htx_dw03_hdr): the host's (kh-er-render
    /// bluehour.rs `set_tower`). Its particle vw_so073_fin_fsg0 is the core's (kh-core states/bp_event.rs).
    pub tower: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum OnEnd {
    StopShake,
    ResetField,
}

#[derive(Clone, Debug, PartialEq)]
struct Cam {
    name: &'static str,
    bp: Bp,
    def: &'static CameraAnimDef,
    time: f32,
    auto_end: bool,
    end: Transition,
    end_takeover: bool,
    zero_frame: bool,
    yaw_always: bool,
    z_always: bool,
    offset: Vec3,
    collision: bool,
    on_end: Option<OnEnd>,
    base_yaw: Option<f32>,
    base_z: Option<f32>,
}

impl Cam {
    fn view(&mut self, sora: (Vec3, f32)) -> AnimView {
        let (loc, yaw) = sora;
        if self.base_yaw.is_none() || self.yaw_always {
            self.base_yaw = Some(yaw);
        }
        if self.base_z.is_none() || self.z_always {
            self.base_z = Some(loc.z);
        }
        let base = Vec3::new(loc.x, loc.y, self.base_z.unwrap_or(loc.z));
        let by = self.base_yaw.unwrap_or(yaw);
        let s = self.def.sample(self.time);
        let (sn, cs) = by.to_radians().sin_cos();
        let p = self.offset + s.location;
        let world = base + Vec3::new(p.x * cs - p.y * sn, p.x * sn + p.y * cs, p.z);
        AnimView { location: world, rotation: Rot { pitch: s.rotation.pitch, yaw: s.rotation.yaw + by, roll: s.rotation.roll }.normalized(), fov: s.fov }
    }
}

/// A started camera's settings (the StartTresCameraAnim / setter calls of one blueprint branch).
#[derive(Clone, Copy, Debug)]
struct Start {
    name: &'static str,
    anim: &'static str,
    blend_in: Transition,
    end: Transition,
    auto_end: bool,
    end_takeover: bool,
    collision: bool,
}

/// A blend of the common shape {time, ease in-out (4), exp 2, bLockOutgoing true}.
const fn eased(time: f32) -> Transition {
    Transition::new(time, 4, 2.0, true)
}

/// EndAllSameBPTresCamera / EndTresCamera's {0, ease out (3), 2}: a cut.
const CUT_OUT: Transition = Transition::new(0.0, 3, 2.0, false);

/// The cameras of the three blueprints.
#[derive(Clone, Debug, Default)]
pub struct FormBpCameras {
    cams: Vec<Cam>,
    blend: Option<(AnimView, Transition, f32)>,
    last: Option<AnimView>,
    shakes: ShakeSet,
    /// The state the pawn is in (tracked through StateChange; reset from the frame at each step's end).
    state: Option<StateId>,
    /// ripple_drive's FOV on the active camera: (from, to, time, elapsed) and the pending Delay(0.6) back.
    fov_ramp: Option<(f32, f32, f32, f32)>,
    fov_back: Option<f32>,
    pending: FormBpOutput,
    /// A readable log (tests, traces).
    pub log: Vec<String>,
    /// (AUDIT-B F-CLAWS) Happy Gear's / Crystal Snow's camera blueprints (mi_key / fz_key, kh-camera forms_claws.rs), stepped
    /// with the same events; their view lies over this file's (the two sets never run together: other sequences).
    pub claws: crate::forms_claws::ClawsCameras,
    /// (AUDIT-B F-SHIP) Wheel of Fate's camera blueprint (ca_key, kh-camera forms_ship.rs), stepped with the same events;
    /// its view lies over this file's (its sequences E401 F401 E416 F418 F420 F481 are no other blueprint's).
    pub ship: crate::forms_ship::ShipCameras,
    /// (AUDIT-A SECOND) Ancient Light's camera blueprint (UnionDiscode, kh-camera forms_second.rs), stepped with the same
    /// events; its view lies over this file's (its sequence 1435 is no other blueprint's).
    pub union: crate::forms_second::UnionDiscodeCamera,
    /// (AUDIT-A STAFF) Mirage Staff's camera blueprint (Rapunzel_key, kh-camera forms_staff.rs), stepped with the same events;
    /// its view lies over this file's (its sequences A401 / A417 are no other blueprint's). Hosts take its `void`.
    pub staff: crate::forms_staff::StaffCameras,
    /// (AUDIT-A CAMS) pawn+0xE2C, the command in action (kh-core `combat().form.command_in_action`, set by the executor's
    /// 0x1414158F0 = `OnPlayerBeginCommandAction`): hosts write it before every `step`; a new value is the blueprints'
    /// `ReceivePlayerBeginCommandAction(kind)` (Bazooka: CHANGE_BZ / FINISH_BZ; tate_finish: CHANGE_SH).
    pub command: i32,
    command_seen: i32,
    /// (AUDIT-A CAMS) the gun forms' blueprints (2shotguns, Bazooka, po_key: kh-camera forms_guns.rs), stepped with the same
    /// events; their view lies over this file's (their sequences 2401 / 5413 / G401 / H401 are no other blueprint's).
    pub guns: crate::forms_guns::GunCameras,
    /// (AUDIT-A CAMS) Hero's Origin's / Grand Chef's blueprints (tate_finish, Lemylkey: kh-camera forms_shield.rs), stepped with
    /// the same events and the begun command; their view lies over these (B417 B418 B423 B424 B426 J401 J423 are no other
    /// blueprint's). Hosts may take its `game_speed` (B418 / B426's SetGlobalGameSpeed).
    pub shield: crate::forms_shield::ShieldCameras,
}

impl FormBpCameras {
    pub fn new() -> FormBpCameras {
        FormBpCameras { shakes: ShakeSet::new(), ..Default::default() }
    }

    pub fn active(&self) -> bool {
        !self.cams.is_empty() || self.blend.is_some() || self.fov_ramp.is_some() || self.claws.active() /* (AUDIT-B F-CLAWS) */ || self.ship.active() /* (AUDIT-B F-SHIP) */ || self.union.active() /* (AUDIT-A SECOND) */ || self.staff.active() /* (AUDIT-A STAFF) */ || self.guns.active() || self.shield.active() // (AUDIT-A CAMS)
    }

    /// The running cameras' names, oldest first.
    pub fn cameras(&self) -> Vec<&'static str> {
        self.cams.iter().map(|c| c.name).collect()
    }

    fn switch(&mut self, t: Transition) {
        self.blend = match (self.last, t.time > 0.0) {
            (Some(from), true) => Some((from, t, 0.0)),
            _ => None,
        };
    }

    fn start(&mut self, bp: Bp, s: Start) -> Option<usize> {
        let Some(def) = camera_anim(s.anim) else {
            self.log.push(format!("{} is not in the generated camera tables (node tools/party_team_pack.js --cameras)", s.anim));
            return None;
        };
        self.log.push(format!("start {} {}", s.name, s.anim));
        self.switch(s.blend_in);
        self.cams.push(Cam { name: s.name, bp, def, time: 0.0, auto_end: s.auto_end, end: s.end, end_takeover: s.end_takeover, zero_frame: false, yaw_always: false, z_always: true, offset: Vec3::ZERO, collision: s.collision, on_end: None, base_yaw: None, base_z: None });
        Some(self.cams.len() - 1)
    }

    fn end_where(&mut self, t: Transition, takeover: bool, pred: impl Fn(&Cam) -> bool) {
        let top = self.cams.last().map(|c| c.name);
        let before = self.cams.len();
        self.cams.retain(|c| !pred(c));
        if self.cams.len() != before {
            self.log.push(format!("end {} camera(s), top was {top:?}", before - self.cams.len()));
            if self.cams.last().map(|c| c.name) != top {
                self.switch(t);
                if takeover && self.cams.is_empty() {
                    self.pending.takeover = self.last;
                }
            }
        }
    }

    fn end_bp(&mut self, bp: Bp) {
        self.end_where(CUT_OUT, false, |c| c.bp == bp);
    }

    fn trace_clear(world: Option<&dyn kh_core::World>, sora: (Vec3, f32), local: Vec3) -> bool {
        let Some(w) = world else { return true };
        let (loc, yaw) = sora;
        let (s, c) = yaw.to_radians().sin_cos();
        let to = loc + Vec3::new(local.x * c - local.y * s, local.x * s + local.y * c, local.z);
        w.line_trace(loc + Vec3::Z * 100.0, to).is_none()
    }

    fn perf(&self, frame: &FormBpFrame) -> bool {
        frame.perf.enabled(self.state.unwrap_or(frame.state))
    }

    /// One event of the step (in order), with the step's frame and the level for the blueprints' line traces.
    fn on_event(&mut self, ev: &EventKind, frame: &FormBpFrame, world: Option<&dyn kh_core::World>) {
        match *ev {
            EventKind::StateChange { from, to } => {
                self.dtor(from);
                // AltemaWep ReceivePlayerCtorState (@4984): DAMAGE ends its cameras.
                if to == StateId::Damage {
                    self.end_bp(Bp::AltemaWep);
                }
                self.state = Some(to);
            }
            EventKind::AnimBpEvent { seq, begin: true, .. } => self.notify_start(seq, frame, world),
            EventKind::AnimBpEvent { seq, begin: false, .. } => self.notify_end(seq),
            _ => {}
        }
    }

    fn dtor(&mut self, from: StateId) {
        let s = from as i32;
        // form_change @6995: 31 -> @14027 (shake off + @15), 32 33 86 90 -> @14074.
        if s == 31 {
            self.shakes.clear();
        }
        if matches!(s, 31 | 32 | 33 | 86 | 90) {
            self.end_bp(Bp::FormChange);
        }
        // AltemaWep @13068: 31 32 33 41.
        if matches!(s, 31 | 32 | 33 | 41) {
            self.end_bp(Bp::AltemaWep);
        }
        // Oath_keeper @5112: 31 32 33.
        if matches!(s, 31..=33) {
            self.end_bp(Bp::OathKeeper);
        }
    }

    fn notify_start(&mut self, seq: &str, frame: &FormBpFrame, world: Option<&dyn kh_core::World>) {
        let perf = self.perf(frame);
        let sora = frame.sora;
        let none = |anim, blend_in: f32, end: f32, takeover| Start { name: "none", anim, blend_in: eased(blend_in), end: eased(end), auto_end: true, end_takeover: takeover, collision: true };
        let anti = |anim, end: f32, takeover| Start { name: "anti", anim, blend_in: eased(0.0), end: eased(end), auto_end: true, end_takeover: takeover, collision: true };
        match seq {
            // ---- form_change
            "1401" if perf && Self::trace_clear(world, sora, Vec3::new(300.3313, -4.690214, 72.63639)) => {
                let s = Start { name: "none", anim: "change_limit_form", blend_in: eased(0.6), end: eased(0.0), auto_end: true, end_takeover: false, collision: true };
                if let Some(i) = self.start(Bp::FormChange, s) {
                    self.cams[i].on_end = Some(OnEnd::StopShake);
                }
            }
            "1403" if perf => {
                if self.cams.last().is_some_and(|c| c.def.name == "change_limit_form") {
                    if let Some(s) = camera_shake("shake_super_kokizami_1sec") {
                        self.shakes.play(&s.def(), 1.0, None);
                        self.log.push("shake shake_super_kokizami_1sec x1".to_string());
                    }
                }
            }
            "1421" if Self::trace_clear(world, sora, Vec3::new(269.60706, 61.52054, 51.96646)) => {
                self.shakes.clear();
                self.start(Bp::FormChange, none("stan_impact01", 0.66, 0.3, true));
            }
            "1422" if Self::trace_clear(world, sora, Vec3::new(269.60706, 61.52054, 51.96646)) => {
                self.start(Bp::FormChange, none("stan_impact02", 0.66, 0.3, true));
            }
            "1426" => {
                self.start(Bp::FormChange, none("sonic_rave_01", 0.33, 0.3, true));
            }
            "1416" if perf => {
                self.start(Bp::FormChange, none("magne_splash", 0.66, 0.3, true));
            }
            "1433" => {
                self.start(Bp::FormChange, none("LastAlcanam_End", 0.0, 0.0, false));
            }
            "1413" if perf => {
                let from = self.last.map_or(frame.field.fov, |v| v.fov);
                self.fov_ramp = Some((from, 120.0, 0.2, 0.0));
                self.fov_back = Some(0.6);
                self.log.push("fov 120 in 0.2 s".to_string());
            }
            "D401" if perf && [Vec3::new(424.412, -116.4059, 241.57486), Vec3::new(167.82431, -520.022, 41.539597), Vec3::new(143.78271, 235.40381, 71.45606)].iter().all(|p| Self::trace_clear(world, sora, *p)) => {
                self.start(Bp::FormChange, anti("anti_change", 0.0, false));
            }
            "D417" if perf => {
                self.start(Bp::FormChange, anti("anti_brust", 0.0, false));
            }
            "D418" => {
                self.start(Bp::FormChange, anti("anti_bomb", 0.3, true));
            }
            // ---- (AUDIT-C) BP_TresPlayerStateEvent_motion_notify (ReceiveAnimNotifyStartBpEvent @1079, SendActor = Sora;
            // no perf test, no ctor / dtor handler: its cameras end themselves)
            // 1050 (@1170 -> DiveFallAttack_camera() -> @1417 -> @1355 -> @65): "DiveFallAttack_camera", EnableCollision(true),
            // priority 200, StartTresCameraAnim(DiveFallAttack_camera, bLoop false, bTakeover false, in {0, 4, 2, lock}, bAutoEnd,
            // bEndTakeover false, end {0: a cut}); WaitPlayEnd -> @30 ResetTresCameraNormal(self, true).
            "1050" => {
                let s = Start { name: "DiveFallAttack_camera", anim: "DiveFallAttack_camera", blend_in: eased(0.0), end: eased(0.0), auto_end: true, end_takeover: false, collision: true };
                if let Some(i) = self.start(Bp::MotionNotify, s) {
                    self.cams[i].on_end = Some(OnEnd::ResetField);
                }
            }
            // 1176 (@1298 -> @15 special_Railslide_Camera() -> @1422 -> @579): "specialRail", EnableCollision(FALSE), priority
            // 200, StartTresCameraAnim(SpecialRail_Camera, false, false, in {0: a cut}, bAutoEnd, bEndTakeover TRUE, end
            // {0.33, ease in-out, 2, lock}).
            "1176" => {
                let s = Start { name: "specialRail", anim: "SpecialRail_Camera", blend_in: eased(0.0), end: eased(0.33), auto_end: true, end_takeover: true, collision: false };
                self.start(Bp::MotionNotify, s);
            }
            // 1582 (@1241): compared, then nothing (`pop`). 0002 / 0107: compared by no blueprint.
            // ---- AltemaWep (Ultima Weapon equipped)
            "9401" | "9418" | "9419" if frame.ultima => self.altema(seq, perf, frame, world),
            // ---- Oath_keeper
            "N424" => {
                let s = Start { name: "BlueHour_Camera01", anim: "BlueHour_Cut01", blend_in: eased(0.0), end: eased(0.0), auto_end: false, end_takeover: false, collision: true };
                let clear = Self::trace_clear(world, sora, Vec3::new(0.0, 0.0, 300.0));
                if let Some(i) = self.start(Bp::OathKeeper, s) {
                    if clear {
                        // @4581 SetTresCameraTargetInfluenceLocationXYZ(true, false)
                        self.cams[i].z_always = false;
                    } else {
                        // @3599 BP_SetTresCameraAnimBaseOffset((0, 0, -300), R0)
                        self.cams[i].offset = Vec3::new(0.0, 0.0, -300.0);
                        self.cams[i].name = "BlueHour_Camera01_ceiling";
                    }
                }
            }
            "N425" => {
                let s = Start { name: "BlueHour_Camera02", anim: "BlueHour_Cut02B", blend_in: eased(0.0), end: eased(0.0), auto_end: false, end_takeover: false, collision: false };
                self.start(Bp::OathKeeper, s);
                // @412 EndTresCamera(ReturnValue4 = the clear case's Camera01, cut)
                self.end_where(CUT_OUT, false, |c| c.name == "BlueHour_Camera01");
                self.pending.blue_hour = Some(true);
                self.log.push("blue hour on".to_string());
            }
            _ => {}
        }
    }

    fn altema(&mut self, seq: &str, perf: bool, frame: &FormBpFrame, world: Option<&dyn kh_core::World>) {
        let sora = frame.sora;
        let (anim, blend) = match seq {
            "9401" if !perf => {
                // @522 RemoteEvent('Cnage_Key_Cancel'): the level script's (no listener here).
                self.log.push("remote event Cnage_Key_Cancel".to_string());
                return;
            }
            "9401" if Self::trace_clear(world, sora, Vec3::new(533.51465, 31.941803, 25.180176)) => ("Altema_Change_Cut01", 0.0),
            "9418" if perf => ("altema_yoko_finish", 0.0),
            "9419" if perf && Self::trace_clear(world, sora, Vec3::new(-125.0, 300.0, -105.0)) && Self::trace_clear(world, sora, Vec3::new(515.0, -235.0, 535.0)) => ("altema_buki_finish", 0.33),
            _ => return,
        };
        let s = Start { name: "LemyKey_Finish_Cam", anim, blend_in: eased(blend), end: eased(0.0), auto_end: true, end_takeover: false, collision: true };
        if let Some(i) = self.start(Bp::AltemaWep, s) {
            let c = &mut self.cams[i];
            c.zero_frame = true; // @1160
            c.yaw_always = true; // @1198
            c.on_end = Some(OnEnd::ResetField); // @1243 WaitPlayEnd -> @15
        }
    }

    fn notify_end(&mut self, seq: &str) {
        match seq {
            // form_change @5532: the end of 1403 stops the shake (1073: nothing).
            "1403" => self.shakes.clear(),
            // Oath_keeper @2439 .. @3135
            "N425" => {
                self.end_where(CUT_OUT, false, |c| c.name == "BlueHour_Camera02");
                self.pending.blue_hour = Some(false);
                self.pending.reset_field_camera = true;
                self.log.push("blue hour off".to_string());
            }
            _ => {}
        }
    }

    /// One step: the step's core events (in order), advance the cameras, the view to draw.
    pub fn step(&mut self, dt: f32, events: &[kh_core::Event], frame: &FormBpFrame, world: Option<&dyn kh_core::World>) -> FormBpOutput {
        for e in events {
            self.on_event(&e.kind, frame, world);
        }
        self.state = Some(frame.state);
        // advance
        let mut ended: Vec<(usize, Transition, bool, Option<OnEnd>)> = Vec::new();
        for (i, c) in self.cams.iter_mut().enumerate() {
            let step = if std::mem::take(&mut c.zero_frame) { 0.0 } else { dt };
            let before = c.time;
            c.time = (c.time + step).min(c.def.length);
            if c.time >= c.def.length && before < c.def.length {
                if c.auto_end {
                    ended.push((i, c.end, c.end_takeover, c.on_end));
                } else if let Some(o) = c.on_end {
                    ended.push((usize::MAX, c.end, false, Some(o)));
                }
            }
        }
        for (i, t, takeover, on_end) in ended.into_iter().rev() {
            if i != usize::MAX {
                let name = self.cams[i].name;
                let top = self.cams.len() == i + 1;
                self.cams.remove(i);
                self.log.push(format!("auto end {name}"));
                if top {
                    self.switch(t);
                    if takeover && self.cams.is_empty() {
                        self.pending.takeover = self.last;
                    }
                }
            }
            match on_end {
                Some(OnEnd::StopShake) => self.shakes.clear(),
                Some(OnEnd::ResetField) => self.pending.reset_field_camera = true,
                None => {}
            }
        }
        // ripple_drive's FOV
        if let Some(b) = self.fov_back.as_mut() {
            *b -= dt;
            if *b <= 0.0 {
                self.fov_back = None;
                let from = self.fov_ramp.map_or(frame.field.fov, ramp_value); // (AUDIT-B F-CLAWS) Clippy: redundant closure
                self.fov_ramp = Some((from, frame.field.fov, 1.0, 0.0));
            }
        }
        let fov_now = self.fov_ramp.map(ramp_value); // (AUDIT-B F-CLAWS) Clippy: redundant closure
        if let Some(r) = self.fov_ramp.as_mut() {
            r.3 += dt;
            if self.fov_back.is_none() && r.3 >= r.2 {
                self.fov_ramp = None;
            }
        }
        // the view of the top camera, with the collision pass when enabled
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
        self.shakes.step(dt);
        let mut out = std::mem::take(&mut self.pending);
        out.view = view;
        out.shake = self.shakes.offset();
        // (AUDIT-B F-CLAWS) mi_key / fz_key: their view over this one, their field-camera reset / takeover too
        let claws = self.claws.step(dt, events, frame, world);
        if claws.view.is_some() {
            out.view = claws.view;
        }
        out.reset_field_camera |= claws.reset_field_camera;
        out.takeover = out.takeover.or(claws.takeover);
        out.tower = out.tower.or(claws.tower); // (AUDIT-B W-FIN)
        // (AUDIT-B F-SHIP) ca_key: its view over these, its field-camera resets too
        let ship = self.ship.step(dt, events, frame.sora, frame.field, frame.perf.enabled(frame.state), world);
        if ship.view.is_some() {
            out.view = ship.view;
        }
        out.reset_field_camera |= ship.reset_field_camera;
        self.log.extend(self.ship.log.drain(..).map(|l| format!("ca_key: {l}")));
        // (AUDIT-A SECOND) UnionDiscode: Ancient Light's camera over these, its field reset / takeover too
        let union = self.union.step(dt, events, frame.sora, frame.field, world);
        if union.view.is_some() {
            out.view = union.view;
        }
        out.reset_field_camera |= union.reset_field_camera;
        out.takeover = out.takeover.or(union.takeover);
        self.log.extend(self.union.log.drain(..).map(|l| format!("UnionDiscode: {l}")));
        // (AUDIT-A STAFF) Rapunzel_key: Mirage Staff's cameras over these, its shake and bEndTakeover too
        if let Some(v) = self.staff.step(dt, events, frame, world) {
            out.view = Some(v.view);
        }
        let s = std::mem::take(&mut self.staff.shake);
        (out.shake.location, out.shake.rotation, out.shake.fov) = (out.shake.location + s.location, out.shake.rotation + s.rotation, out.shake.fov + s.fov);
        out.takeover = out.takeover.or(self.staff.takeover.take());
        self.log.extend(self.staff.log.drain(..).map(|l| format!("Rapunzel_key: {l}")));
        // (AUDIT-A CAMS) ReceivePlayerBeginCommandAction: the command in action changed to a new kind this step
        let begun = (self.command != 0 && self.command != self.command_seen).then_some(self.command);
        self.command_seen = self.command;
        // (AUDIT-A CAMS) the gun forms' blueprints over these, their resets / takeovers / shakes too
        let guns = self.guns.step(dt, events, frame, begun, world);
        merge_key_cam(&mut out, guns);
        self.log.append(&mut self.guns.log);
        // (AUDIT-A CAMS) tate_finish / Lemylkey over these
        let shield = self.shield.step(dt, events, frame, begun, world);
        merge_key_cam(&mut out, shield);
        self.log.extend(self.shield.log.drain(..).map(|l| format!("tate_finish/Lemylkey: {l}")));
        out
    }
}

/// (AUDIT-A CAMS) A keyblade layer's output over the result so far: its view on top, its resets / takeover / shake added.
fn merge_key_cam(out: &mut FormBpOutput, k: crate::forms_guns::KeyCamOutput) {
    if k.view.is_some() {
        out.view = k.view;
    }
    out.reset_field_camera |= k.reset_field_camera;
    out.takeover = out.takeover.or(k.takeover);
    (out.shake.location, out.shake.rotation, out.shake.fov) = (out.shake.location + k.shake.location, out.shake.rotation + k.shake.rotation, out.shake.fov + k.shake.fov);
}

fn ramp_value(r: (f32, f32, f32, f32)) -> f32 {
    let (from, to, time, elapsed) = r;
    if time <= 0.0 {
        to
    } else {
        from + (to - from) * crate::lockon::interp_curve(5, elapsed / time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_core::host::{Event, EventKind};

    fn frame(state: StateId, ultima: bool) -> FormBpFrame {
        FormBpFrame { sora: (Vec3::new(100.0, 0.0, 0.0), 0.0), field: AnimView { location: Vec3::new(-400.0, 0.0, 200.0), rotation: Rot::default(), fov: 67.0 }, state, ultima, perf: PerfCamera::default() }
    }

    fn ev(kind: EventKind) -> Vec<Event> {
        vec![Event { step: 0, kind }]
    }

    fn bp(seq: &'static str, begin: bool) -> Vec<Event> {
        ev(EventKind::AnimBpEvent { seq, param: 0, begin })
    }

    #[test]
    fn the_perf_camera_option_is_kh3s_test() {
        let p = PerfCamera::default();
        assert!(p.enabled(StateId::Stylechange) && p.enabled(StateId::Stylefinish) && p.enabled(StateId::ComboAn));
        let off = PerfCamera { combo_cam: 2, keyblade_cinematics: false, restricted: false };
        assert!(!off.enabled(StateId::Stylechange), "Keyblade Cinematics Basic");
        assert!(off.enabled(StateId::Stylefinish), "the finishers always");
        assert!(!off.enabled(StateId::ComboAn), "Combo Cam Off");
        assert!(!PerfCamera { restricted: true, ..p }.enabled(StateId::ComboAn), "Auto, restricted");
        assert!(PerfCamera { combo_cam: 1, restricted: true, ..p }.enabled(StateId::ComboAn), "On");
    }

    #[cfg(kh_party_team_cameras)]
    #[test]
    fn rage_forms_three_cameras_and_their_ends() {
        let mut c = FormBpCameras::new();
        let f = frame(StateId::Stylechange, false);
        assert_eq!(c.step(1.0 / 60.0, &[], &f, None).view, None);
        // D401: anti_change on Sora, a cut in
        let v = c.step(0.0, &bp("D401", true), &f, None).view.unwrap();
        assert_eq!(c.cameras(), ["anti"]);
        let s = camera_anim("anti_change").unwrap().sample(0.0);
        assert!((v.location - (Vec3::new(100.0, 0.0, 0.0) + s.location)).length() < 1e-3, "{v:?}");
        // leaving STYLECHANGE cuts it
        c.step(1.0 / 60.0, &ev(EventKind::StateChange { from: StateId::Stylechange, to: StateId::ComboAn }), &frame(StateId::ComboAn, false), None);
        assert!(c.cameras().is_empty());
        // D417 in COMBO_AN; its dtor ends it
        c.step(0.0, &bp("D417", true), &frame(StateId::ComboAn, false), None);
        assert_eq!(c.cameras(), ["anti"]);
        c.step(1.0 / 60.0, &ev(EventKind::StateChange { from: StateId::ComboAn, to: StateId::Stylefinish }), &frame(StateId::Stylefinish, false), None);
        assert!(c.cameras().is_empty());
        // D418 anti_bomb: no perf test, autoEnd at 3.567 s with the field camera taking over
        let off = FormBpFrame { perf: PerfCamera { combo_cam: 2, keyblade_cinematics: false, restricted: false }, ..frame(StateId::Stylefinish, false) };
        c.step(0.0, &bp("D418", true), &off, None);
        assert_eq!(c.cameras(), ["anti"]);
        let mut took = None;
        for _ in 0..220 {
            let o = c.step(1.0 / 60.0, &[], &off, None);
            took = took.or(o.takeover);
        }
        assert!(c.cameras().is_empty() && took.is_some(), "anti_bomb ends itself and hands its view over");
    }

    #[cfg(kh_party_team_cameras)]
    #[test]
    fn ultima_weapons_cameras_only_with_ultima_and_reset_the_field_camera() {
        let mut c = FormBpCameras::new();
        c.step(0.0, &bp("9401", true), &frame(StateId::Stylechange, false), None);
        assert!(c.cameras().is_empty(), "another Keyblade: AltemaWep does nothing");
        let f = frame(StateId::Stylechange, true);
        // PlayZeroFrame: the camera tick of the frame the notify started it in holds frame 0
        let first = c.step(1.0 / 60.0, &bp("9401", true), &f, None).view.unwrap();
        assert_eq!(c.cameras(), ["LemyKey_Finish_Cam"]);
        let s0 = camera_anim("Altema_Change_Cut01").unwrap().sample(0.0);
        assert!((first.location - (Vec3::new(100.0, 0.0, 0.0) + s0.location)).length() < 1e-3);
        let mut reset = false;
        for _ in 0..300 {
            reset |= c.step(1.0 / 60.0, &[], &f, None).reset_field_camera;
        }
        assert!(c.cameras().is_empty() && reset, "4.7 s, then ResetTresCameraPrevious");
        // 9419 eases in over 0.33 s
        let g = frame(StateId::Stylefinish, true);
        c.step(0.0, &bp("9419", true), &g, None);
        let v = c.step(0.1, &[], &g, None).view.unwrap();
        assert!((v.location - g.field.location).length() > 1.0 && c.cameras() == ["LemyKey_Finish_Cam"]);
        // DAMAGE's ctor ends it with a cut
        c.step(0.0, &ev(EventKind::StateChange { from: StateId::Stylefinish, to: StateId::Damage }), &frame(StateId::Damage, true), None);
        assert!(c.cameras().is_empty());
    }

    #[cfg(kh_party_team_cameras)]
    #[test]
    fn the_ultimate_finish_blue_hour() {
        let mut c = FormBpCameras::new();
        let f = frame(StateId::Stylefinish, false);
        c.step(0.0, &bp("N424", true), &f, None);
        assert_eq!(c.cameras(), ["BlueHour_Camera01"]);
        // the base height is taken once: his rise in N424 does not lift the camera
        let a = c.step(0.0, &[], &f, None).view.unwrap();
        let up = FormBpFrame { sora: (Vec3::new(100.0, 0.0, 300.0), 0.0), ..f };
        let b = c.step(0.0, &[], &up, None).view.unwrap();
        assert!((a.location.z - b.location.z).abs() < 1e-3);
        let o = c.step(0.0, &bp("N425", true), &f, None);
        assert_eq!(o.blue_hour, Some(true));
        assert_eq!(c.cameras(), ["BlueHour_Camera02"]);
        let o = c.step(0.0, &bp("N425", false), &f, None);
        assert_eq!((o.blue_hour, o.reset_field_camera), (Some(false), true));
        assert!(c.cameras().is_empty());
    }

    #[cfg(kh_party_team_cameras)]
    #[test]
    fn motion_notifys_dive_fall_and_rail_cameras() {
        let mut c = FormBpCameras::new();
        let f = frame(StateId::Attack, false);
        // 1050: DiveFallAttack_camera, a cut in, 1.5 s, then the field camera behind Sora (ResetTresCameraNormal(true))
        let v = c.step(0.0, &bp("1050", true), &f, None).view.unwrap();
        assert_eq!(c.cameras(), ["DiveFallAttack_camera"]);
        let s0 = camera_anim("DiveFallAttack_camera").unwrap().sample(0.0);
        assert!((v.location - (Vec3::new(100.0, 0.0, 0.0) + s0.location)).length() < 1e-3, "{v:?}");
        // no dtor handler: a state change does not end it
        c.step(0.0, &ev(EventKind::StateChange { from: StateId::Attack, to: StateId::Idle }), &frame(StateId::Idle, false), None);
        assert_eq!(c.cameras(), ["DiveFallAttack_camera"]);
        let mut reset = false;
        for _ in 0..100 {
            reset |= c.step(1.0 / 60.0, &[], &f, None).reset_field_camera;
        }
        assert!(c.cameras().is_empty() && reset);
        // 1176: specialRail, 0.8 s, handed to the field camera with a 0.33 s blend
        c.step(0.0, &bp("1176", true), &f, None);
        assert_eq!(c.cameras(), ["specialRail"]);
        let mut took = None;
        for _ in 0..60 {
            took = took.or(c.step(1.0 / 60.0, &[], &f, None).takeover);
        }
        assert!(c.cameras().is_empty() && took.is_some());
        // 1582 / 0002 / 0107: nothing
        for s in ["1582", "0002", "0107"] {
            c.step(0.0, &bp(s, true), &f, None);
        }
        assert!(c.cameras().is_empty());
    }

    #[cfg(kh_party_team_cameras)]
    #[test]
    fn the_transformation_camera_and_its_shake() {
        let mut c = FormBpCameras::new();
        let f = frame(StateId::Stylechange, false);
        c.step(0.0, &bp("1401", true), &f, None);
        assert_eq!(c.cameras(), ["none"]);
        // 0.6 s ease in-out from the field camera
        let v = c.step(0.3, &[], &f, None).view.unwrap();
        assert!((v.location - f.field.location).length() > 1.0);
        c.step(0.0, &bp("1403", true), &f, None);
        assert!(c.log.iter().any(|l| l.starts_with("shake")));
        c.step(0.0, &bp("1403", false), &f, None);
        assert_eq!(c.step(1.0 / 60.0, &[], &f, None).shake, ShakeOffset::default());
    }
}
