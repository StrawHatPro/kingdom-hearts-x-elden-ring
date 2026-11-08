//! (AUDIT-A SECOND) ANCIENT LIGHT'S CAMERA: `BP_TresPlayerStateEventCamera_UnionDiscode`
//! (Blueprints/Camera/p_ex001_StateEvent/change_key), as data. Read 2026-11-03 from its ubergraph (Kismet bytecode;
//! `@n` = the offset; listing in build/sora_tmp/weapons/AUDIT-A/SECOND.md):
//! - ReceiveAnimNotifyStartBpEvent (@2417 -> @1018): two line traces from his actor location + (0, 0, 100) to his local
//!   (500, -300, 0) and (500, 300, 0) (@1211 / @1627, channel 1); either blocked -> `pop`: no camera. Then (@1850) SendActor
//!   = the local player and AnimSeqName '1435' (M_1435, LM_UNIONDISCODE's clip) -> "Limit_UnionDiscode_Camera"
//!   (@1993 -> @556): BP_SettingTresCamera(Sora), EnableCollision(true), SetPriority(200), BP_AttachTresCameraTarget(false),
//!   StartTresCameraAnim(Limit_UnionDiscode, bLoop false, bTakeover true, in {0.3, ease in-out (4), 2, lock}, bAutoEnd true,
//!   bEndTakeover true, end {0.3, 4, 2}); kept as `Uniqulo_Cam`.
//! - ReceiveAnimNotifyEndBpEvent (@2422): '1435' -> Uniqulo_Cam.EndTresCamera({0, ease out (3), 2}: a cut) (@2565) ->
//!   @2224: GetTresCameraNormal valid -> @2055 ResetCamera(true) (the field camera behind him) -> @140 the RotTarget
//!   "back_camera" on Sora: SetTresCameraRotTargetParam(500, R(-10, 0, 0), 0, 0), StartTresCamera(cut), SetPriority(200);
//!   TresWaitCycle(1) (@479) -> @15: EndAllBPTresCameraOfName('back_camera', {0.5, ease out (3), 2, lock}).
//! - ReceivePlayerDtorState (@2653): STYLECHANGE 31, WEAPONCHANGE 32, STYLEFINISH 33, COMBO_LIMIT 90 -> @9681
//!   EndAllSameBPTresCamera({0, 3, 2}: a cut).
//!
//! The same shape as Wheel of Fate's ca_key back camera (forms_ship.rs, whose constants are used). No performance-camera
//! test in this blueprint. The camera anim is `Camera/sora/Skill/Limit_UnionDiscode` (tools/party_team_pack.js
//! `--cameras`). Stepped by kh-camera form_bp.rs with the other form blueprints.

use glam::Vec3;
use kh_core::host::EventKind;

use crate::camera_anim::{camera_anim, collide, frame_target, sweep_start, AnimView, CameraAnimDef, Transition};
use crate::forms_ship::{Actor, BACK_DISTANCE, BACK_PITCH, TRACE_RAISE};
use crate::Rot;

/// The two trace points of the start, in his actor frame (@1211 / @1627).
pub const UNION_TRACES: [Vec3; 2] = [Vec3::new(500.0, -300.0, 0.0), Vec3::new(500.0, 300.0, 0.0)];
/// The camera anim's asset and the spawn name.
pub const UNION_ANIM: &str = "Limit_UnionDiscode";
pub const UNION_CAMERA: &str = "Limit_UnionDiscode_Camera";

const IN: Transition = Transition::new(0.3, 4, 2.0, true);
const END: Transition = Transition::new(0.3, 4, 2.0, true);
const CUT_OUT: Transition = Transition::new(0.0, 3, 2.0, false);
const BACK_OUT: Transition = Transition::new(0.5, 3, 2.0, true);

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Anim { def: &'static CameraAnimDef, time: f32 },
    /// The RotTarget "back_camera": Sora's yaw at its start.
    Back { yaw: f32 },
}

#[derive(Clone, Debug, PartialEq)]
struct Cam {
    name: &'static str,
    kind: Kind,
}

/// One step's result.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct UnionOutput {
    pub view: Option<AnimView>,
    /// `ResetCamera(true)`: the field camera behind Sora at once.
    pub reset_field_camera: bool,
    /// bEndTakeover of the anim camera's own end: the camera under it takes over this view.
    pub takeover: Option<AnimView>,
}

/// The UnionDiscode blueprint as data.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct UnionDiscodeCamera {
    cams: Vec<Cam>,
    /// TresWaitCycle(1) after the back camera: due at the next update.
    back_end_due: bool,
    blend: Option<(AnimView, Transition, f32)>,
    last: Option<AnimView>,
    pending: UnionOutput,
    /// A readable log (tests, traces).
    pub log: Vec<String>,
}

fn yawed(v: Vec3, yaw: f32) -> Vec3 {
    let (s, c) = yaw.to_radians().sin_cos();
    Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
}

impl UnionDiscodeCamera {
    pub fn new() -> UnionDiscodeCamera {
        UnionDiscodeCamera::default()
    }

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
    }

    fn end_where(&mut self, t: Transition, pred: impl Fn(&Cam) -> bool) {
        let top = self.cams.last().map(|c| c.name);
        let before = self.cams.len();
        self.cams.retain(|c| !pred(c));
        if self.cams.len() != before {
            self.log.push(format!("end {} camera(s), top was {top:?}", before - self.cams.len()));
            if self.cams.last().map(|c| c.name) != top {
                self.switch(t);
            }
        }
    }

    /// One core event of the step, in order.
    pub fn on_event(&mut self, ev: &EventKind, sora: Actor, world: Option<&dyn kh_core::World>) {
        match *ev {
            EventKind::AnimBpEvent { seq: "1435", begin: true, .. } => {
                let clear = world.is_none_or(|w| UNION_TRACES.iter().all(|p| w.line_trace(sora.0 + Vec3::Z * TRACE_RAISE, sora.0 + yawed(*p, sora.1)).is_none()));
                if !clear {
                    self.log.push("1435: a trace is blocked: no camera".to_string());
                    return;
                }
                match camera_anim(UNION_ANIM) {
                    Some(def) => {
                        self.log.push(format!("start {UNION_CAMERA}"));
                        self.switch(IN);
                        self.cams.push(Cam { name: UNION_CAMERA, kind: Kind::Anim { def, time: 0.0 } });
                    }
                    None => self.log.push(format!("{UNION_ANIM} is not in the generated camera tables (node tools/party_team_pack.js --cameras)")),
                }
            }
            EventKind::AnimBpEvent { seq: "1435", begin: false, .. } => {
                self.end_where(CUT_OUT, |c| c.name == UNION_CAMERA);
                // @2224 -> @2055 -> @140
                self.pending.reset_field_camera = true;
                self.log.push("start back_camera".to_string());
                self.switch(Transition::CUT);
                self.cams.push(Cam { name: "back_camera", kind: Kind::Back { yaw: sora.1 } });
                self.back_end_due = true;
            }
            EventKind::StateChange { from, .. } if matches!(from as i32, 31 | 32 | 33 | 90) => {
                self.back_end_due = false;
                self.end_where(CUT_OUT, |_| true);
            }
            _ => {}
        }
    }

    fn view_of(cam: &Cam, sora: Actor, field: &AnimView, world: Option<&dyn kh_core::World>) -> AnimView {
        let (loc, yaw) = sora;
        let v = match &cam.kind {
            Kind::Anim { def, time } => {
                let s = def.sample(*time);
                AnimView { location: loc + yawed(s.location, yaw), rotation: Rot { pitch: s.rotation.pitch, yaw: s.rotation.yaw + yaw, roll: s.rotation.roll }.normalized(), fov: s.fov }
            }
            Kind::Back { yaw: y0 } => {
                let r = Rot { pitch: BACK_PITCH, yaw: *y0, roll: 0.0 }.normalized();
                AnimView { location: loc - r.forward() * BACK_DISTANCE, rotation: r, fov: field.fov }
            }
        };
        // EnableCollision(true) (the anim camera; the RotTarget collides as every field-type camera does)
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

    /// The step's core events (in order), then advance; `field` = the view of the cameras under it.
    pub fn step(&mut self, dt: f32, events: &[kh_core::Event], sora: Actor, field: AnimView, world: Option<&dyn kh_core::World>) -> UnionOutput {
        if std::mem::take(&mut self.back_end_due) {
            self.end_where(BACK_OUT, |c| c.name == "back_camera");
        }
        for e in events {
            self.on_event(&e.kind, sora, world);
        }
        // the anim's own end: bAutoEnd with {0.3, ease in-out} and bEndTakeover
        let mut auto_end = false;
        for c in &mut self.cams {
            if let Kind::Anim { def, time } = &mut c.kind {
                *time += dt;
                if *time >= def.length {
                    *time = def.length;
                    auto_end = true;
                }
            }
        }
        if auto_end {
            self.end_where(END, |c| matches!(&c.kind, Kind::Anim { def, time } if *time >= def.length));
            if self.cams.is_empty() {
                self.pending.takeover = self.last;
            }
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
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kh_core::{Event, StateId};

    fn field() -> AnimView {
        AnimView { location: Vec3::new(-400.0, 0.0, 200.0), rotation: Rot::default(), fov: 100.0 }
    }

    const SORA: Actor = (Vec3::new(0.0, 0.0, 90.0), 30.0);

    fn ev(kind: EventKind) -> Vec<Event> {
        vec![Event { step: 0, kind }]
    }

    /// M_1435's window: Limit_UnionDiscode blends in over 0.3 s and follows him (yaw 30), ends by itself (4.93 s, 0.3 s out,
    /// takeover); the window's end: the field reset and the back camera 500 cm behind him at -10, a 0.5 s blend back one
    /// tick later; STYLEFINISH's dtor cuts everything.
    #[test]
    fn ancient_light_camera_as_the_blueprint_runs_it() {
        let def = camera_anim(UNION_ANIM).expect("Limit_UnionDiscode packed (party_team_pack.js --cameras)");
        let mut c = UnionDiscodeCamera::new();
        c.step(1.0 / 60.0, &[], SORA, field(), None);
        let first = c.step(1.0 / 60.0, &ev(EventKind::AnimBpEvent { seq: "1435", param: 0, begin: true }), SORA, field(), None);
        assert_eq!(c.cameras(), [UNION_CAMERA]);
        assert!(first.view.is_some());
        let mut out = UnionOutput::default();
        for _ in 0..60 {
            out = c.step(1.0 / 60.0, &[], SORA, field(), None);
        }
        let s = def.sample(61.0 / 60.0);
        let v = out.view.unwrap();
        assert!((v.location - (SORA.0 + yawed(s.location, SORA.1))).length() < 1e-2, "on the anim in his frame");
        let mut took = false;
        for _ in 0..400 {
            out = c.step(1.0 / 60.0, &[], SORA, field(), None);
            took |= out.takeover.is_some();
        }
        assert!(took && c.cameras().is_empty(), "bAutoEnd with bEndTakeover");
        let out = c.step(1.0 / 60.0, &ev(EventKind::AnimBpEvent { seq: "1435", param: 0, begin: false }), SORA, field(), None);
        assert!(out.reset_field_camera);
        let v = out.view.unwrap();
        assert!((v.location.distance(SORA.0) - BACK_DISTANCE).abs() < 1.0 && (v.rotation.pitch - BACK_PITCH).abs() < 1e-3, "back_camera");
        let mut n = 0;
        while c.active() {
            c.step(1.0 / 60.0, &[], SORA, field(), None);
            n += 1;
            assert!(n < 60);
        }
        assert!((29..=32).contains(&n), "one tick, then 0.5 s back: {n}");
        // the dtor of STYLEFINISH cuts the anim camera
        let mut c = UnionDiscodeCamera::new();
        c.step(1.0 / 60.0, &ev(EventKind::AnimBpEvent { seq: "1435", param: 0, begin: true }), SORA, field(), None);
        let out = c.step(1.0 / 60.0, &ev(EventKind::StateChange { from: StateId::Stylefinish, to: StateId::Jump }), SORA, field(), None);
        assert!(c.cameras().is_empty() && out.view.is_none() && !c.active());
    }
}
