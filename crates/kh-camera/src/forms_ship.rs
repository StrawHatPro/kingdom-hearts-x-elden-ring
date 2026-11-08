//! (AUDIT-B F-SHIP) WHEEL OF FATE'S CAMERA BLUEPRINT `BP_TresPlayerStateEventCamera_ca_key`
//! (Blueprints/Camera/p_ex001_StateEvent/change_key; its ubergraph read 2026-11-03 with build/sora_tmp/party/tools/bpdump.js:
//! build/sora_tmp/weapons/AUDIT-B/F-SHIP/traces/bp_ca_key.txt, offsets `@n` below). KH3 holds every state-event camera
//! blueprint of the pawn's load list and feeds each the state changes and Sora's BpEvent windows; this one listens to
//! Highwind's and Storm Flag's clips. Its camera anims (Camera/sora/Skill/Ca_Key) are in kh-camera's generated tables
//! (tools/party_team_pack.js `--cameras`).
//!
//! - `ReceiveAnimNotifyStartBpEvent` (@5188, SendActor = Sora): by `AnimSeqName`
//!   - E401 (StyleChange14) -> @4571: with the performance camera on (`BP_TresIsGameEnablePerformanceCamera`), @5681:
//!     On_Skill; three line traces (channel 1) from his actor location + (0, 0, 100) to his transform x (-592.2, -338.5,
//!     199.2), (143.2, -673.8, 269.5), (662.2, -390.4, 227.4) (@5935 / @6351 / @4943): any one blocked -> nothing more;
//!     else @3759 / @2422 "YoYo_Start_Cmaera": Cakey_Start on Sora (EnableCollision, priority 200, not attached), in
//!     {0.4 s, fn 4, exp 2, lock}, autoEnd with a cut, yaw always (`SetTresCameraTargetInfluenceRotation(true, ..)`),
//!     PlayZeroFrame; WaitPlayEnd -> @321: if On_Skill: the normal camera `ResetCamera(true)`, a RotTarget "back_camera"
//!     on Sora (500, R(-10, 0, 0), a cut in, priority 200), `TresWaitCycle(1)` -> @49: "back_camera" ends with {0.5 s,
//!     fn 3, exp 2, lock}, On_Skill off. Without the performance camera: only `RemoteEvent("Cnage_Key_Cancel")`.
//!   - F401 (StyleChange15) -> @5116 / @6524: the same with the traces (680.4, -83.8, 71.8), (748.3, 336.9, 99.8),
//!     (217.5, 585.7, 859.5), "Tsume_Start_Cmaera" = Flag_Start (@3148: in {0.4, 4, 2}, out a cut), WaitPlayEnd -> @1099
//!     (as @321) -> @185 (as @49).
//!   - E416 (Highwind's last finisher) -> `halberd_combo_finish` @7845 / @3033: performance camera -> @1882
//!     "halberd_combo_finish": Halberd_combo_finish, a cut in, autoEnd with a cut, yaw always.
//!   - F418 (Storm Flag's last finisher) -> @14989: Camera_Anim = Ca_Flag_combo_finish, Blend_Time 0.33 -> @4513 / @3986
//!     "ca_flag_combo_finish": a cut in, autoEnd with {Blend_Time, fn 4, exp 2, lock}, yaw always; if On_Skill:
//!     WaitPlayEnd -> @1877 / @15 `ResetTresCameraPrevious` (0x140E63360: the camera under the top, vt+0x7A0(1) =
//!     ResetCamera(true)).
//!   - F420 (FINISH_FL) -> @15036: Ca_Flag_finish, Blend_Time 0, On_Skill on -> @3986 (a cut in, a cut out, then the
//!     reset at its end).
//!   - F481 (the Storm Flag shotlock's clip) -> @15094: Ca_Flag_ranbu, Blend_Time 0.33 -> @3986.
//! - `ReceivePlayerCtorState` (@7850): state 19 (DAMAGE) -> @14866: On_Skill off, `EndAllSameBPTresCamera` {0.3 s, fn 3,
//!   exp 2, lock}.
//! - `ReceivePlayerDtorState` (@15141): states 31 STYLECHANGE, 32 WEAPONCHANGE, 33 STYLEFINISH, 39 STYLEFINISH_FL, 87
//!   COMBO_HL, 88 COMBO_FL -> @22177: `EndAllSameBPTresCamera` with a cut.
//! - WaitPlayEnd (0x1405DE630, its update 0x1405DDC20): resumes when the anim instance has finished OR the camera is gone
//!   (a camera ended by the dtor resumes its WaitPlayEnd on the next update).
//! - The performance camera (FUN_140EAB300): the config byte +0xB5B (2 = off, 1 = on, 0 = on unless pawn vt+0x2798);
//!   always on inside STYLEFINISH-class states (0x21 .. 0x27). The port's hosts have no such option: [`ShipCameras::performance`]
//!   (on by default).
//! - BlendFunction numbers: UE `EViewTargetBlendFunction` (camera_anim.rs Transition).

use glam::Vec3;
use kh_core::host::EventKind;
use kh_core::StateId;

use crate::camera_anim::{camera_anim, collide, frame_target, sweep_start, AnimView, CameraAnimDef, Transition};
use crate::Rot;

/// The trace points of the two transformation cameras, in his actor frame (@5885 / @6301 / @4893; @6728 / @7144 / @7560).
pub const HALBERD_START_TRACES: [Vec3; 3] = [Vec3::new(-592.166_14, -338.456_36, 199.242_98), Vec3::new(143.182_11, -673.797_1, 269.537_78), Vec3::new(662.200_9, -390.445_47, 227.443_13)];
pub const FLAG_START_TRACES: [Vec3; 3] = [Vec3::new(680.411_25, -83.807_65, 71.758_01), Vec3::new(748.261_3, 336.888, 99.796_66), Vec3::new(217.527_16, 585.699_04, 859.453_1)];
/// The traces start 100 above his actor location (@5785 `Add_VectorVector(location, (0, 0, 100))`).
pub const TRACE_RAISE: f32 = 100.0;
/// "back_camera" (@826 / @1604): `SetTresCameraRotTargetParam(500, R(-10, 0, 0), 0, 0)`.
pub const BACK_DISTANCE: f32 = 500.0;
pub const BACK_PITCH: f32 = -10.0;

const IN_START: Transition = Transition::new(0.4, 4, 2.0, true);
const CUT_4: Transition = Transition::new(0.0, 4, 2.0, true);
const BACK_OUT: Transition = Transition::new(0.5, 3, 2.0, true);
const DAMAGE_OUT: Transition = Transition::new(0.3, 3, 2.0, true);
const DTOR_CUT: Transition = Transition::new(0.0, 3, 2.0, false);

#[derive(Clone, Copy, Debug, PartialEq)]
enum Continuation {
    /// @321 / @1099 (WaitPlayEnd of a start camera): the back camera if On_Skill.
    StartEnd,
    /// @1877 -> @15: ResetTresCameraPrevious.
    ResetPrevious,
    /// @49 / @185 (TresWaitCycle(1) after the back camera): it ends, On_Skill off.
    BackEnd,
}

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Anim { def: &'static CameraAnimDef, time: f32, end: Transition, on_end: Option<Continuation> },
    /// The RotTarget "back_camera": its yaw is Sora's at its start (`+0x704` default: once).
    Back { yaw: f32 },
}

#[derive(Clone, Debug, PartialEq)]
struct Cam {
    name: &'static str,
    kind: Kind,
}

/// What one update gives the host.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ShipOutput {
    /// The view to draw over the field / form cameras (None: nothing of this blueprint's).
    pub view: Option<AnimView>,
    /// `ResetCamera(true)` / `ResetTresCameraPrevious`: the field camera goes behind Sora.
    pub reset_field_camera: bool,
    /// The view does not continue the last one (a cut in or out): no interpolation between the two.
    pub cut: bool,
}

/// The ca_key blueprint as data. Feed every core event of a step ([`ShipCameras::on_event`]), then [`ShipCameras::update`].
#[derive(Clone, Debug, PartialEq)]
pub struct ShipCameras {
    cams: Vec<Cam>,
    on_skill: bool,
    /// Continuations due at this update / registered during this step (latent actions resume on the following tick).
    due: Vec<Continuation>,
    later: Vec<Continuation>,
    blend: Option<(AnimView, Transition, f32)>,
    last: Option<AnimView>,
    pending: ShipOutput,
    /// `BP_TresIsGameEnablePerformanceCamera` (KH3's config; the port has no option: on).
    pub performance: bool,
    /// A readable log of what the blueprint did (tests, traces).
    pub log: Vec<String>,
}

impl Default for ShipCameras {
    fn default() -> ShipCameras {
        ShipCameras::new()
    }
}

/// His actor location and yaw this step: the camera base (his actor origin at his feet, as camera_anim.rs / form_bp.rs
/// feed it: party D4); the traces start 100 above it.
pub type Actor = (Vec3, f32);

fn yawed(v: Vec3, yaw: f32) -> Vec3 {
    let (s, c) = yaw.to_radians().sin_cos();
    Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
}

impl ShipCameras {
    pub fn new() -> ShipCameras {
        ShipCameras { cams: Vec::new(), on_skill: false, due: Vec::new(), later: Vec::new(), blend: None, last: None, pending: ShipOutput::default(), performance: true, log: Vec::new() }
    }

    /// A camera of the blueprint is up (or blending out).
    pub fn active(&self) -> bool {
        !self.cams.is_empty() || self.blend.is_some()
    }

    /// The running cameras' spawn names, oldest first.
    pub fn cameras(&self) -> Vec<&'static str> {
        self.cams.iter().map(|c| c.name).collect()
    }

    fn switch(&mut self, t: Transition) {
        self.blend = match (self.last, t.time > 0.0) {
            (Some(from), true) => Some((from, t, 0.0)),
            _ => None,
        };
        if t.time <= 0.0 {
            self.pending.cut = true;
        }
    }

    fn push(&mut self, cam: Cam, t: Transition) {
        self.log.push(format!("start {}", cam.name));
        self.switch(t);
        self.cams.push(cam);
    }

    /// Ends the cameras `pred` picks; an anim camera's WaitPlayEnd resumes (the camera is gone).
    fn end_where(&mut self, t: Transition, pred: impl Fn(&Cam) -> bool) {
        let top = self.cams.last().map(|c| c.name);
        let mut ended = Vec::new();
        self.cams.retain(|c| {
            if pred(c) {
                ended.push(c.clone());
                false
            } else {
                true
            }
        });
        if ended.is_empty() {
            return;
        }
        for c in &ended {
            self.log.push(format!("end {}", c.name));
            if let Kind::Anim { on_end: Some(k), .. } = c.kind {
                self.later.push(k);
            }
        }
        if self.cams.last().map(|c| c.name) != top {
            self.switch(t);
        }
    }

    /// `EndAllSameBPTresCamera`.
    fn end_all(&mut self, t: Transition) {
        self.end_where(t, |_| true);
    }

    fn anim(&mut self, name: &'static str, asset: &str, inn: Transition, end: Transition, on_end: Option<Continuation>) {
        match camera_anim(asset) {
            Some(def) => self.push(Cam { name, kind: Kind::Anim { def, time: 0.0, end, on_end } }, inn),
            None => self.log.push(format!("{asset} is not in the generated camera tables (node tools/party_team_pack.js --cameras)")),
        }
    }

    /// The transformation cameras' traces: any one blocked -> no camera (@6093 / @6509 / @5101 `pop`).
    fn traces_clear(points: &[Vec3; 3], sora: Actor, world: Option<&dyn kh_core::World>) -> bool {
        let Some(w) = world else { return true };
        let from = sora.0 + Vec3::Z * TRACE_RAISE;
        points.iter().all(|p| w.line_trace(from, sora.0 + yawed(*p, sora.1)).is_none())
    }

    /// One core event of the step, in order (`sora` = his actor location and yaw now; `world` for the traces).
    pub fn on_event(&mut self, ev: &EventKind, sora: Actor, world: Option<&dyn kh_core::World>) {
        match *ev {
            EventKind::AnimBpEvent { seq, begin: true, .. } => self.notify_start(seq, sora, world),
            EventKind::StateChange { from, to } => {
                // the dtor of the state left, then the ctor of the new one
                if matches!(from as i32, 31 | 32 | 33 | 39 | 87 | 88) {
                    self.end_all(DTOR_CUT);
                }
                if to == StateId::Damage {
                    self.on_skill = false;
                    self.end_all(DAMAGE_OUT);
                }
            }
            _ => {}
        }
    }

    fn notify_start(&mut self, seq: &str, sora: Actor, world: Option<&dyn kh_core::World>) {
        if !self.performance {
            return;
        }
        match seq {
            "E401" | "F401" => {
                self.on_skill = true;
                let (traces, name, asset) = if seq == "E401" { (&HALBERD_START_TRACES, "YoYo_Start_Cmaera", "Cakey_Start") } else { (&FLAG_START_TRACES, "Tsume_Start_Cmaera", "Flag_Start") };
                if Self::traces_clear(traces, sora, world) {
                    self.anim(name, asset, IN_START, CUT_4, Some(Continuation::StartEnd));
                } else {
                    self.log.push(format!("{seq}: a trace to the camera's path is blocked: no camera"));
                }
            }
            "E416" => self.anim("halberd_combo_finish", "Halberd_combo_finish", CUT_4, CUT_4, None),
            "F418" | "F420" | "F481" => {
                let (asset, blend) = match seq {
                    "F418" => ("Ca_Flag_combo_finish", 0.33),
                    "F420" => ("Ca_Flag_finish", 0.0),
                    _ => ("Ca_Flag_ranbu", 0.33),
                };
                if seq == "F420" {
                    self.on_skill = true;
                }
                let on_end = self.on_skill.then_some(Continuation::ResetPrevious);
                self.anim("ca_flag_combo_finish", asset, CUT_4, Transition::new(blend, 4, 2.0, true), on_end);
            }
            _ => {}
        }
    }

    fn run(&mut self, k: Continuation, sora: Actor) {
        match k {
            Continuation::StartEnd => {
                if self.on_skill {
                    self.pending.reset_field_camera = true;
                    self.push(Cam { name: "back_camera", kind: Kind::Back { yaw: sora.1 } }, Transition::CUT);
                    self.later.push(Continuation::BackEnd);
                }
            }
            Continuation::BackEnd => {
                self.end_where(BACK_OUT, |c| c.name == "back_camera");
                self.on_skill = false;
            }
            Continuation::ResetPrevious => self.pending.reset_field_camera = true,
        }
    }

    fn view_of(cam: &Cam, sora: Actor, field: &AnimView, world: Option<&dyn kh_core::World>) -> AnimView {
        let (loc, yaw) = sora;
        let v = match &cam.kind {
            Kind::Anim { def, time, .. } => {
                let s = def.sample(*time);
                AnimView { location: loc + yawed(s.location, yaw), rotation: Rot { pitch: s.rotation.pitch, yaw: s.rotation.yaw + yaw, roll: s.rotation.roll }.normalized(), fov: s.fov }
            }
            Kind::Back { yaw: y0 } => {
                let r = Rot { pitch: BACK_PITCH, yaw: *y0, roll: 0.0 }.normalized();
                AnimView { location: loc - r.forward() * BACK_DISTANCE, rotation: r, fov: field.fov }
            }
        };
        // EnableCollision(true): the collision pass of the event cameras (camera_anim.rs, the same pivot: his location)
        match world {
            Some(w) => match frame_target(&v, loc, w) {
                Some(f) if f.distance > 0.0 => {
                    let start = f.location + f.rotation.forward() * f.distance;
                    AnimView { location: collide(start, f.location, w), rotation: f.rotation, ..v }
                }
                _ => AnimView { location: collide(sweep_start(&v, loc), v.location, w), ..v },
            },
            None => v,
        }
    }

    /// The step's core events (in order), then [`ShipCameras::update`] (`performance` = the performance camera's answer for
    /// his state now: kh-camera form_bp.rs `PerfCamera::enabled`).
    pub fn step(&mut self, dt: f32, events: &[kh_core::Event], sora: Actor, field: AnimView, performance: bool, world: Option<&dyn kh_core::World>) -> ShipOutput {
        self.performance = performance;
        for e in events {
            self.on_event(&e.kind, sora, world);
        }
        self.update(dt, sora, field, world)
    }

    /// One step: advance the anims and the latent actions; the view (`field` = the view of the cameras under it).
    pub fn update(&mut self, dt: f32, sora: Actor, field: AnimView, world: Option<&dyn kh_core::World>) -> ShipOutput {
        for k in std::mem::take(&mut self.due) {
            self.run(k, sora);
        }
        let mut ended: Vec<(&'static str, Transition)> = Vec::new();
        for c in &mut self.cams {
            if let Kind::Anim { def, time, end, .. } = &mut c.kind {
                *time += dt;
                if *time >= def.length {
                    *time = def.length;
                    ended.push((c.name, *end));
                }
            }
        }
        for (name, t) in ended {
            self.end_where(t, |c| c.name == name && matches!(&c.kind, Kind::Anim { def, time, .. } if *time >= def.length));
        }
        let top = self.cams.last().map(|c| Self::view_of(c, sora, &field, world));
        let target = top.unwrap_or(field);
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
        self.last = Some(view.unwrap_or(field));
        let mut out = std::mem::take(&mut self.pending);
        out.view = view;
        self.due.append(&mut self.later);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field() -> AnimView {
        AnimView { location: Vec3::new(-400.0, 0.0, 200.0), rotation: Rot::default(), fov: 100.0 }
    }

    const SORA: Actor = (Vec3::new(0.0, 0.0, 90.0), 30.0);

    fn steps(c: &mut ShipCameras, n: usize) -> ShipOutput {
        let mut out = ShipOutput::default();
        for _ in 0..n {
            out = c.update(1.0 / 60.0, SORA, field(), None);
        }
        out
    }

    /// Highwind's transformation: Cakey_Start blends in over 0.4 s, holds to its end, then the back camera (cut, the field
    /// reset) and the 0.5 s blend back; the STYLECHANGE dtor before its end cuts it and the back camera follows the same way.
    #[test]
    fn the_transformation_camera_and_its_way_back() {
        let mut c = ShipCameras::new();
        c.on_event(&EventKind::AnimBpEvent { seq: "E401", param: 0, begin: true }, SORA, None);
        assert_eq!(c.cameras(), ["YoYo_Start_Cmaera"]);
        let first = c.update(1.0 / 60.0, SORA, field(), None);
        assert!(!first.cut && first.view.is_some(), "a 0.4 s blend in");
        let def = camera_anim("Cakey_Start").unwrap();
        // on the anim after the blend: its key in his frame (yaw 30)
        let v = steps(&mut c, 60).view.unwrap();
        let s = def.sample(61.0 / 60.0);
        assert!((v.location - (SORA.0 + yawed(s.location, SORA.1))).length() < 1e-2 && (v.fov - s.fov).abs() < 1e-4);
        // its end: a cut out, the back camera at once (500 behind him, -10) with the field reset, then 0.5 s back
        let mut out = ShipOutput::default();
        let mut k = 0;
        while c.cameras() != ["back_camera"] && k < 400 {
            out = c.update(1.0 / 60.0, SORA, field(), None);
            k += 1;
        }
        let out2 = c.update(1.0 / 60.0, SORA, field(), None);
        assert!(out.reset_field_camera || out2.reset_field_camera, "ResetCamera(true) with the back camera");
        let mut n = 0;
        while c.active() {
            c.update(1.0 / 60.0, SORA, field(), None);
            n += 1;
            assert!(n < 60);
        }
        assert!((28..=31).contains(&n), "0.5 s back to the field: {n}");
        // the dtor of STYLECHANGE in the middle: a cut, then the same back camera path
        let mut c = ShipCameras::new();
        c.on_event(&EventKind::AnimBpEvent { seq: "F401", param: 0, begin: true }, SORA, None);
        steps(&mut c, 30);
        c.on_event(&EventKind::StateChange { from: StateId::Stylechange, to: StateId::Jump }, SORA, None);
        assert!(c.cameras().is_empty());
        // (the WaitPlayEnd resumes on the next tick)
        c.update(1.0 / 60.0, SORA, field(), None);
        let out = c.update(1.0 / 60.0, SORA, field(), None);
        assert_eq!(c.cameras(), ["back_camera"]);
        assert!(out.reset_field_camera && out.view.is_some());
    }

    /// The finishers: E416's Halberd_combo_finish (cut in, cut out at its end); F418's with its 0.33 s blend out; FINISH_FL's
    /// Ca_Flag_finish (7.2 s) with the field reset at its end; DAMAGE ends them in 0.3 s.
    #[test]
    fn the_finisher_cameras() {
        let mut c = ShipCameras::new();
        c.on_event(&EventKind::AnimBpEvent { seq: "E416", param: 0, begin: true }, SORA, None);
        let out = c.update(1.0 / 60.0, SORA, field(), None);
        assert!(out.cut && out.view.is_some(), "a cut in");
        let len = camera_anim("Halberd_combo_finish").unwrap().length;
        let n = (len * 60.0).ceil() as usize;
        steps(&mut c, n);
        assert!(!c.active(), "autoEnd with a cut after {len} s");
        let mut c = ShipCameras::new();
        c.on_event(&EventKind::AnimBpEvent { seq: "F418", param: 0, begin: true }, SORA, None);
        let len = camera_anim("Ca_Flag_combo_finish").unwrap().length;
        steps(&mut c, (len * 60.0).ceil() as usize + 1);
        assert!(c.cameras().is_empty() && c.active(), "blending out");
        steps(&mut c, 20);
        assert!(!c.active(), "0.33 s");
        let mut c = ShipCameras::new();
        c.on_event(&EventKind::AnimBpEvent { seq: "F420", param: 0, begin: true }, SORA, None);
        let mut reset = false;
        for _ in 0..(7.2f32 * 60.0) as usize + 5 {
            reset |= c.update(1.0 / 60.0, SORA, field(), None).reset_field_camera;
        }
        assert!(!c.active() && reset, "Ca_Flag_finish ends with the reset of the field camera");
        let mut c = ShipCameras::new();
        c.on_event(&EventKind::AnimBpEvent { seq: "F420", param: 0, begin: true }, SORA, None);
        steps(&mut c, 30);
        c.on_event(&EventKind::StateChange { from: StateId::Stylefinish, to: StateId::Damage }, SORA, None);
        assert!(c.cameras().is_empty(), "the dtor's cut");
        assert!(c.update(1.0 / 60.0, SORA, field(), None).view.is_none() || !c.active());
        // without the performance camera nothing
        let mut c = ShipCameras::new();
        c.performance = false;
        c.on_event(&EventKind::AnimBpEvent { seq: "E416", param: 0, begin: true }, SORA, None);
        assert!(!c.active());
    }

    /// The transformation cameras' traces: a wall on one of the three lines -> no camera (On_Skill only).
    #[test]
    fn a_blocked_trace_leaves_the_field_camera() {
        struct Wall;
        impl kh_core::World for Wall {
            fn sweep_capsule(&self, _: Vec3, _: Vec3, _: f32, _: f32) -> Option<kh_core::world::Hit> {
                None
            }
            fn line_trace(&self, a: Vec3, b: Vec3) -> Option<kh_core::world::Hit> {
                // a wall at y = -300 (his right side at yaw 0)
                (a.y > -300.0 && b.y < -300.0).then_some(kh_core::world::Hit { time: 0.5, position: a, normal: Vec3::Y, impact_normal: Vec3::Y, start_penetrating: false, penetration_depth: 0.0, component: 0 })
            }
        }
        let mut c = ShipCameras::new();
        c.on_event(&EventKind::AnimBpEvent { seq: "E401", param: 0, begin: true }, (Vec3::new(0.0, 0.0, 90.0), 0.0), Some(&Wall));
        assert!(!c.active() && c.log.iter().any(|l| l.contains("blocked")));
    }
}
