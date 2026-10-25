//! (CV1) THE CONVERSATION CAMERA, engine-free (both hosts): the shots of a talk (over the shoulder on the speaker, a
//! two-shot, a reaction close-up, a slow push-in), which shot a line gets (variety, a cut when the speaker changes,
//! the 180-degree line kept), the framing of each shot from the subjects' heads (KH3 space, cm, Z up), the collision
//! pass against the host's world (the team cameras' sweep, `camera_anim::collide`: never inside a wall or the ground),
//! and the blend in from / back to the field camera.
//!
//! [L] All of it is composed for the port: KH3 plays its talks as cutscenes (Sequencer cameras), with no field
//! conversation camera to read. The shot sizes follow KH3's event cameras by eye: narrow lenses (45 - 60 degrees
//! horizontal against the field camera's 100), heads at the upper third.

use glam::{Vec2, Vec3};

use crate::{normalize_axis, CameraView, Rot};

/// The kinds of shot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShotKind {
    /// On the subject's face, over the other's shoulder.
    OverShoulder,
    /// Both in frame, side on.
    TwoShot,
    /// A close-up on the subject's face (a reaction when the subject is the listener).
    CloseUp,
    /// Over the shoulder, pushing in slowly for the length of the line.
    PushIn,
}

impl ShotKind {
    pub fn name(self) -> &'static str {
        match self {
            ShotKind::OverShoulder => "over-the-shoulder",
            ShotKind::TwoShot => "two-shot",
            ShotKind::CloseUp => "close-up",
            ShotKind::PushIn => "push-in",
        }
    }

    /// The lens (horizontal degrees on 16:9).
    pub fn fov(self) -> f32 {
        match self {
            ShotKind::OverShoulder => 55.0,
            ShotKind::TwoShot => 60.0,
            ShotKind::CloseUp => 42.0,
            ShotKind::PushIn => 52.0,
        }
    }
}

/// A shot asked for: its kind, its subject (the face it frames) and the other (whose shoulder / the second of a
/// two-shot), whether it is a cut (else a glide from the shot before).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotCue {
    pub kind: ShotKind,
    pub subject: u32,
    pub other: Option<u32>,
    pub cut: bool,
}

/// THE SHOT DIRECTOR: one shot per line, with variety. Rules: the first line is a two-shot (who is here); a new
/// speaker is a CUT to him (over the listener's shoulder, every third change a close-up); the same speaker again gets a
/// reaction close-up on the listener (a cut) or a slow push-in on him (no cut), alternately; a long line (> 110
/// characters) is a push-in; a choice is over the chooser's shoulder on the one who asked.
#[derive(Clone, Debug, Default)]
pub struct ShotDirector {
    lines: u32,
    changes: u32,
    same: u32,
    last_speaker: Option<u32>,
    last: Option<ShotCue>,
}

impl ShotDirector {
    pub fn line(&mut self, speaker: u32, listener: Option<u32>, chars: usize) -> ShotCue {
        let first = self.lines == 0;
        self.lines += 1;
        let changed = self.last_speaker != Some(speaker);
        self.last_speaker = Some(speaker);
        let cue = if first {
            ShotCue { kind: ShotKind::TwoShot, subject: speaker, other: listener, cut: true }
        } else if changed {
            self.changes += 1;
            self.same = 0;
            let kind = if self.changes % 3 == 0 { ShotKind::CloseUp } else if chars > 110 { ShotKind::PushIn } else { ShotKind::OverShoulder };
            ShotCue { kind, subject: speaker, other: listener, cut: true }
        } else {
            self.same += 1;
            match (self.same % 2, listener) {
                (1, Some(l)) if chars <= 110 => ShotCue { kind: ShotKind::CloseUp, subject: l, other: Some(speaker), cut: true },
                _ => ShotCue { kind: ShotKind::PushIn, subject: speaker, other: listener, cut: self.last.is_some_and(|c| c.kind != ShotKind::PushIn || c.subject != speaker) },
            }
        };
        self.last = Some(cue);
        cue
    }

    /// A choice: on the one who asked, over the chooser's shoulder.
    pub fn choice(&mut self, asker: u32, chooser: Option<u32>) -> ShotCue {
        let cue = ShotCue { kind: ShotKind::OverShoulder, subject: asker, other: chooser, cut: self.last.is_none_or(|c| c.kind != ShotKind::OverShoulder || c.subject != asker) };
        self.last_speaker = Some(asker);
        self.lines += 1;
        self.last = Some(cue);
        cue
    }
}

/// A subject of the shots: feet, head (cm) and body yaw (degrees).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Subject {
    pub id: u32,
    pub feet: Vec3,
    pub head: Vec3,
    pub yaw: f32,
}

fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.y, 0.0)
}

fn yaw_dir(yaw: f32) -> Vec3 {
    let r = yaw.to_radians();
    Vec3::new(r.cos(), r.sin(), 0.0)
}

/// THE FRAMING of a shot (before collision): camera position and look point. `side` = +1 / -1: which side of the line
/// between the two the cameras stay on (the 180-degree rule); `push` = 0..1 of a push-in.
pub fn frame(kind: ShotKind, subject: &Subject, other: Option<&Subject>, side: f32, push: f32) -> (Vec3, Vec3) {
    // the line from the other to the subject (or the subject's facing when alone)
    let axis = match other {
        Some(o) if flat(subject.head - o.head).length() > 20.0 => flat(subject.head - o.head).normalize(),
        _ => -yaw_dir(subject.yaw),
    };
    // the perpendicular on the chosen side (Z up: right of the axis = (axis.y, -axis.x))
    let n = Vec3::new(axis.y, -axis.x, 0.0) * side;
    let up = Vec3::Z;
    let gap = other.map_or(160.0, |o| flat(subject.head - o.head).length());
    match kind {
        ShotKind::OverShoulder | ShotKind::PushIn => {
            // behind the other's shoulder on the camera side, a little above it, on the subject's face
            let o = other.copied().unwrap_or(Subject { id: u32::MAX, feet: subject.feet - axis * 160.0, head: subject.head - axis * 160.0, yaw: subject.yaw });
            // behind the other's shoulder (his head less a neck), at a height between his shoulder and the subject's face
            // (a short subject is looked at from lower down), the subject's face a little above the middle
            let back = if kind == ShotKind::PushIn { 140.0 - 55.0 * push.clamp(0.0, 1.0) } else { 140.0 };
            let shoulder = o.head - up * 24.0;
            let height = shoulder.z + (subject.head.z - shoulder.z) * 0.35 + 6.0;
            let mut eye = shoulder - axis * back + n * (62.0 + gap * 0.1);
            eye.z = height;
            let look = subject.head - up * 14.0;
            (eye, look)
        }
        ShotKind::TwoShot => {
            let o = other.copied().unwrap_or(*subject);
            let mid = (subject.head + o.head) * 0.5;
            let dist = (gap * 1.15 + 140.0).clamp(220.0, 900.0);
            let eye = mid + n * dist - axis * gap * 0.12 + up * 18.0;
            (eye, mid - up * 10.0)
        }
        ShotKind::CloseUp => {
            // in front of the face, to the camera side of the other
            let toward = other.map_or(yaw_dir(subject.yaw), |o| flat(o.head - subject.head).normalize_or_zero());
            let dir = (toward + n * 0.55).normalize_or_zero();
            // head and shoulders on the narrow lens (KH3's event close-ups by eye)
            let eye = subject.head + dir * 190.0 - up * 4.0;
            (eye, subject.head - up * 24.0)
        }
    }
}

/// The view of a camera at `eye` looking at `look`.
pub fn view_at(eye: Vec3, look: Vec3, collided: bool) -> CameraView {
    let d = look - eye;
    let rot = Rot::looking_along(d);
    CameraView { position: eye, rotation: rot, rotation_target: rot, look_point: look, distance: d.length(), auto_power: 0.0, stick_wait: 0.0, still_wait: 0.0, reset_wait: 0.0, slope_pitch: 0.0, stick_active: false, followed: false, collided }
}

/// The 180-degree side the field camera is on now: +1 when it is to the right of the line from `a` to `b`.
pub fn side_of(camera: Vec3, a: Vec3, b: Vec3) -> f32 {
    let axis = flat(b - a);
    if axis.length() < 1.0 {
        return 1.0;
    }
    let n = Vec3::new(axis.y, -axis.x, 0.0);
    if flat(camera - a).dot(n) >= 0.0 {
        1.0
    } else {
        -1.0
    }
}

/// Seconds of the blend from the field camera into the first shot, and back out at the end; of a glide between shots.
pub const BLEND_IN: f32 = 0.6;
pub const BLEND_OUT: f32 = 0.8;
pub const GLIDE: f32 = 0.45;
/// Seconds a push-in takes (its whole length).
pub const PUSH_SECONDS: f32 = 6.0;

fn ease(u: f32) -> f32 {
    let u = u.clamp(0.0, 1.0);
    u * u * (3.0 - 2.0 * u)
}

fn mix_view(a: &CameraView, b: &CameraView, t: f32) -> CameraView {
    let lerp = |x: f32, y: f32| normalize_axis(x + normalize_axis(y - x) * t);
    let rot = Rot { pitch: lerp(a.rotation.pitch, b.rotation.pitch), yaw: lerp(a.rotation.yaw, b.rotation.yaw), roll: lerp(a.rotation.roll, b.rotation.roll) };
    CameraView { position: a.position.lerp(b.position, t), rotation: rot, rotation_target: rot, look_point: a.look_point.lerp(b.look_point, t), distance: a.distance + (b.distance - a.distance) * t, collided: a.collided || b.collided, ..*b }
}

/// THE CONVERSATION CAMERA: shots in, cuts and glides, the push, collision, the blend in and out.
#[derive(Clone, Debug, Default)]
pub struct ConvCamera {
    cue: Option<ShotCue>,
    /// The cue's age (s) and, for a glide, the view it glides from.
    age: f32,
    glide_from: Option<(CameraView, f32)>,
    side: f32,
    /// 0..1 how much of the view is ours (eased); the conversation runs / ended.
    weight: f32,
    running: bool,
    last: Option<(CameraView, f32)>,
    /// Shots taken (testing / logs).
    pub shots: u32,
    pub collided: u32,
}

impl ConvCamera {
    pub fn active(&self) -> bool {
        self.running || self.weight > 0.0
    }

    pub fn running(&self) -> bool {
        self.running
    }

    pub fn cue(&self) -> Option<ShotCue> {
        self.cue
    }

    /// A conversation begins: its two first subjects fix the side (the field camera's).
    pub fn begin(&mut self, field_eye: Vec3, a: Vec3, b: Vec3) {
        if !self.running {
            self.side = side_of(field_eye, a, b);
            self.running = true;
            self.cue = None;
            self.glide_from = None;
        }
    }

    /// The conversation ended: the view blends back to the field camera.
    pub fn end(&mut self) {
        self.running = false;
    }

    /// A new shot.
    pub fn shot(&mut self, cue: ShotCue) {
        if !cue.cut {
            if let Some(last) = self.last {
                self.glide_from = Some(last);
            }
        } else {
            self.glide_from = None;
        }
        self.cue = Some(cue);
        self.age = 0.0;
        self.shots += 1;
    }

    /// One step: `under` = the field camera's view of this step (blended with), `field_fov` its lens; `subjects` = the
    /// heads of this step. Returns the view and the lens to draw, or None when the conversation camera has no part in
    /// it (then the field camera is drawn as it is).
    pub fn step(&mut self, dt: f32, subjects: &[Subject], world: Option<&dyn kh_core::World>, under: &CameraView, field_fov: f32) -> Option<(CameraView, f32)> {
        let target = if self.running { 1.0 } else { 0.0 };
        let rate = if self.running { dt / BLEND_IN } else { dt / BLEND_OUT };
        self.weight = if self.weight < target { (self.weight + rate).min(1.0) } else { (self.weight - rate).max(0.0) };
        if self.weight <= 0.0 && !self.running {
            self.last = None;
            self.cue = None;
            return None;
        }
        self.age += dt;
        let shot = self.cue.and_then(|cue| {
            let s = subjects.iter().find(|s| s.id == cue.subject)?;
            let o = cue.other.and_then(|id| subjects.iter().find(|s| s.id == id));
            let push = if cue.kind == ShotKind::PushIn { self.age / PUSH_SECONDS } else { 0.0 };
            let (eye, look) = frame(cue.kind, s, o, if self.side == 0.0 { 1.0 } else { self.side }, push);
            // THE COLLISION PASS (the team cameras' sweep from the look point): never inside the level
            let (eye, hit) = match world {
                Some(w) => {
                    let e = crate::camera_anim::collide(look, eye, w);
                    (e, (e - eye).length() > 0.5)
                }
                None => (eye, false),
            };
            self.collided += u32::from(hit);
            Some((view_at(eye, look, hit), cue.kind.fov()))
        });
        let (mut view, mut fov) = match (shot, self.last) {
            (Some(v), _) => v,
            (None, Some(last)) => last,
            (None, None) => (*under, field_fov),
        };
        if let Some((from, from_fov)) = self.glide_from {
            let t = ease(self.age / GLIDE);
            view = mix_view(&from, &view, t);
            fov = from_fov + (fov - from_fov) * t;
            if t >= 1.0 {
                self.glide_from = None;
            }
        }
        self.last = Some((view, fov));
        let w = ease(self.weight);
        Some((mix_view(under, &view, w), field_fov + (fov - field_fov) * w))
    }
}

/// The yaw (degrees) that faces from `from` to `to`.
pub fn yaw_to(from: Vec3, to: Vec3) -> f32 {
    let d = Vec2::new(to.x - from.x, to.y - from.y);
    d.y.atan2(d.x).to_degrees()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject(id: u32, x: f32, y: f32, h: f32) -> Subject {
        Subject { id, feet: Vec3::new(x, y, 0.0), head: Vec3::new(x, y, h), yaw: 0.0 }
    }

    #[test]
    fn the_director_varies_and_cuts_on_a_new_speaker() {
        let mut d = ShotDirector::default();
        let a = d.line(1, Some(2), 40);
        assert_eq!(a.kind, ShotKind::TwoShot);
        let b = d.line(2, Some(1), 40);
        assert!(b.cut && b.subject == 2 && b.kind == ShotKind::OverShoulder);
        let c = d.line(2, Some(1), 40);
        assert!(c.cut && c.subject == 1 && c.kind == ShotKind::CloseUp, "a reaction on the listener: {c:?}");
        let e = d.line(2, Some(1), 40);
        assert_eq!(e.kind, ShotKind::PushIn);
        let f = d.line(1, Some(2), 300);
        assert!(f.cut && f.kind == ShotKind::PushIn);
        let kinds: std::collections::HashSet<_> = [a, b, c, e, f].iter().map(|c| c.kind).collect();
        assert!(kinds.len() >= 3);
    }

    #[test]
    fn every_shot_looks_at_its_subject_from_the_one_side() {
        let a = subject(1, 0.0, 0.0, 150.0);
        let b = subject(2, 180.0, 40.0, 90.0);
        for kind in [ShotKind::OverShoulder, ShotKind::TwoShot, ShotKind::CloseUp, ShotKind::PushIn] {
            for side in [1.0, -1.0] {
                let (eye, look) = frame(kind, &b, Some(&a), side, 0.5);
                assert!(eye.is_finite() && look.is_finite());
                // on the chosen side of the line a -> b
                assert_eq!(side_of(eye, a.head, b.head), side_of(a.head + Vec3::new(40.0, -180.0, 0.0) * side, a.head, b.head), "{kind:?} {side}");
                assert!((eye - look).length() > 60.0, "{kind:?}");
            }
        }
    }

    #[test]
    fn the_collision_pass_keeps_the_camera_out_of_a_wall() {
        let a = subject(1, 0.0, 0.0, 150.0);
        let b = subject(2, 180.0, 0.0, 90.0);
        // the over-the-shoulder camera on b sits behind a (x < 0): a wall at x = -40
        let world = kh_core::test_world::TestWorld::floor().with_plane_through(Vec3::new(-40.0, 0.0, 0.0), Vec3::X);
        let mut cam = ConvCamera::default();
        cam.begin(Vec3::new(0.0, -300.0, 150.0), a.head, b.head);
        cam.shot(ShotCue { kind: ShotKind::OverShoulder, subject: 2, other: Some(1), cut: true });
        let under = view_at(Vec3::new(-300.0, 0.0, 200.0), a.head, false);
        let mut out = None;
        for _ in 0..60 {
            out = cam.step(1.0 / 30.0, &[a, b], Some(&world), &under, 100.0);
        }
        let (view, fov) = out.unwrap();
        assert!(view.position.x > -40.0, "{:?}", view.position);
        assert!(cam.collided > 0);
        assert!((fov - ShotKind::OverShoulder.fov()).abs() < 0.01);
        // the end: back to the field camera within BLEND_OUT
        cam.end();
        let mut last = None;
        for _ in 0..30 {
            last = cam.step(1.0 / 30.0, &[a, b], Some(&world), &under, 100.0);
        }
        assert!(last.is_none(), "handed back");
        assert!(!cam.active());
    }

    #[test]
    fn the_blend_in_is_smooth() {
        let a = subject(1, 0.0, 0.0, 150.0);
        let b = subject(2, 200.0, 0.0, 150.0);
        let mut cam = ConvCamera::default();
        let under = view_at(Vec3::new(-400.0, -200.0, 250.0), a.head, false);
        cam.begin(under.position, a.head, b.head);
        cam.shot(ShotCue { kind: ShotKind::TwoShot, subject: 1, other: Some(2), cut: true });
        let mut prev = under.position;
        let mut steps = Vec::new();
        for _ in 0..30 {
            let (v, _) = cam.step(1.0 / 30.0, &[a, b], None, &under, 100.0).unwrap();
            steps.push((v.position - prev).length());
            prev = v.position;
        }
        let max = steps.iter().cloned().fold(0.0, f32::max);
        assert!(max < 120.0, "no jump: {steps:?}");
    }
}
