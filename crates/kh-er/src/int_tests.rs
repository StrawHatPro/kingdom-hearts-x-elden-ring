//! (INT) THE WHOLE KH3 SIDE OF THE FRAME BETWEEN THE STEPS, MEASURED BY RENDERED FRAMES (SM's harness extended to Sora,
//! the camera and the effects). Sora without a game (`kh_host::rig`: the core on a floor, a training dummy, scripted pad
//! input, his pose from kh-anim), the effect side every host shares (`kh_host::vfx::EffectHost`, the real pack) stepped
//! as kh-er fx.rs `effect_step` steps it, KH3's camera rig (cam.rs's order), all on sora.rs's accumulator at 1/60 s,
//! driven by Elden Ring-like frame times (JITTER: 16 - 33 ms, a 50 ms hitch 1 frame in 60; STEADY: 60 fps; SM's clock).
//! Every frame the DRAWN state is read as kh-er draws it (interp.rs `sora_drawn`, cam.rs `post`'s lerp + `camera_cut`,
//! fx.rs `draw_at` -> kh-vfx, sora.rs `int_snap_check`), under three rules:
//! - `live`  = `r.interp = 0` (the old default): everything at the latest step;
//! - `mixed` = `r.interp = 1` BEFORE this stream: Sora and the camera between the steps, the effects at the latest step;
//! - `interp`= the new default: everything at the frame's alpha.
//!
//! Measured: drawn jerk and frame-to-frame error (crates/kh-partyhost/src/sm_metrics.rs) of Sora's root, the blade tip,
//! the camera, the Firaga in flight; THE DRAWING'S OWN ERROR (`judder`: the drawn place against the object's own step path,
//! linear between the steps, at the best latency: what the drawing adds to the motion, whatever the motion); the EFFECTS
//! AGAINST SORA: the Keyblade trail's drawn head against the drawn blade tip, every effect on one of his sockets against
//! where his drawn socket puts it (cm); frames drawn between the places before and after a teleport.
//! `INT_OUT=<dir>`: one CSV per run and the frame-strip geometry (JSON) for `build/sora_tmp/er/int/strips.js`.
//! `cargo test -p kh-er --lib --release int_ -- --nocapture --test-threads=1`.

use std::fmt::Write as _;
use std::path::PathBuf;

use glam::{Vec2, Vec3};
use kh_camera::{CameraInput, CameraRig, CameraView, CAMERA_PARAMS};
use kh_core::{EffectHandle, EventKind, Pose};
use kh_host::audio::PoseTap;
use kh_host::rig::{Rig, Script, STEP};
use kh_host::seam::{AudioQueue, EffectQueue};
use kh_host::vfx::{EffectHost, Scene};

use crate::interp;

#[path = "../../kh-partyhost/src/sm_metrics.rs"]
#[allow(dead_code)]
mod metrics;
use metrics::{DrawnSample, SimSample};

fn project(p: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(p)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Clock {
    Jitter,
    Steady,
}

struct Rng(u64);
impl Rng {
    fn unit(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// SM's frame clock (party/glue/sm_tests.rs), the same numbers.
fn frame_dt(c: Clock, rng: &mut Rng) -> f32 {
    match c {
        Clock::Jitter => {
            if rng.unit() < 1.0 / 60.0 {
                0.050
            } else {
                0.016 + 0.017 * rng.unit()
            }
        }
        Clock::Steady => 1.0 / 60.0 + (rng.unit() - 0.5) * 0.0012,
    }
}

/// The three drawing rules (alpha of Sora + camera, alpha of the effects).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Rule {
    Live,
    Mixed,
    Interp,
}

impl Rule {
    pub const ALL: [Rule; 3] = [Rule::Live, Rule::Mixed, Rule::Interp];
    fn alphas(self, alpha: f32) -> (f32, f32) {
        match self {
            Rule::Live => (1.0, 1.0),
            Rule::Mixed => (alpha, 1.0),
            Rule::Interp => (alpha, alpha),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Rule::Live => "live",
            Rule::Mixed => "mixed",
            Rule::Interp => "interp",
        }
    }
}

pub struct Scenario {
    pub name: &'static str,
    pub script: &'static str,
    pub dummy: Option<f32>,
    /// The right stick on the camera (x) over this span of sim time (s).
    pub camera_turn: Option<(f32, f32, f32)>,
    /// The Keyblade trail (KH3's `ead_key_trail0` = VW_SO010_LOC0 on `vfx_top`, as an attack's notify starts it) held
    /// over this span of sim time (s).
    pub trail: Option<(f32, f32)>,
    /// Teleports of Sora at these sim times (s), 20 m sideways each.
    pub teleports: &'static [f32],
    pub secs: f32,
}

pub fn scenarios() -> Vec<Scenario> {
    vec![
        Scenario { name: "combo with trails", script: "stop 0.5; run 0.45; tap a; stop 0.22; tap a; stop 0.22; tap a; stop 0.22; tap a; stop 0.22; tap a; stop 0.25; tap a; stop 2.0", dummy: Some(520.0), camera_turn: None, trail: Some((0.95, 3.3)), teleports: &[], secs: 4.6 },
        Scenario { name: "firaga in flight", script: "stop 0.6; cast firaga; stop 3.0", dummy: Some(1400.0), camera_turn: None, trail: None, teleports: &[], secs: 3.4 },
        Scenario { name: "sprint + camera turn", script: "stop 0.3; run 1.5; turn 60; run 1.5; turn -90; run 2.0; stop 0.5", dummy: None, camera_turn: Some((1.0, 4.0, 0.8)), trail: None, teleports: &[], secs: 5.6 },
        Scenario { name: "teleports mid-combo", script: "stop 0.3; run 0.6; tap a; stop 0.22; tap a; stop 0.22; tap a; stop 0.22; tap a; stop 0.22; tap a; stop 0.3; run 2.0; stop 0.4", dummy: Some(500.0), camera_turn: None, trail: Some((0.8, 3.0)), teleports: &[1.10, 1.43, 1.77, 2.12, 2.48, 2.80, 3.15, 3.51], secs: 4.4 },
    ]
}

/// A drawn track of one object per rule, and its steps.
#[derive(Default)]
struct Track {
    drawn: [Vec<DrawnSample>; 3],
    sim: Vec<SimSample>,
}

impl Track {
    fn sim(&mut self, t: f64, pos: Vec3) {
        self.sim.push(SimSample { t, pos, yaw: 0.0, leader: Vec3::ZERO, clip_turn: false });
    }
    fn drawn(&mut self, rule: usize, t: f64, pos: Vec3) {
        self.drawn[rule].push(DrawnSample { t, pos, yaw: 0.0, feet: None, leader: Vec3::ZERO });
    }

    /// The object's own step path, linear between the steps.
    fn lin(&self, t: f64) -> Option<Vec3> {
        let s = &self.sim;
        if s.len() < 2 || t < s[0].t || t > s[s.len() - 1].t {
            return None;
        }
        let i = s.partition_point(|x| x.t < t).clamp(1, s.len() - 1);
        let (a, b) = (&s[i - 1], &s[i]);
        Some(a.pos.lerp(b.pos, ((t - a.t) / (b.t - a.t).max(1e-9)) as f32))
    }

    /// THE DRAWING'S OWN ERROR: the best latency (ms, 0..50) of the drawn places against the step path and, at it, the
    /// RMS of the drawn place's distance from it and of the frame displacement's error (cm). A step of more than 3 m
    /// (a teleport) is left out with its neighbours.
    fn judder(&self, rule: usize, discontinuities: &[f64]) -> (f32, f32, f32) {
        let mut jumps: Vec<f64> = self.sim.windows(2).filter(|w| w[0].pos.distance(w[1].pos) > interp::SNAP_CM).map(|w| w[1].t).collect();
        jumps.extend_from_slice(discontinuities);
        let near_jump = |t: f64| jumps.iter().any(|j| (t - j).abs() < 0.1);
        let d = &self.drawn[rule];
        let mut best = (0.0f32, f32::MAX, 0.0f32);
        for ms in 0..=50 {
            let lag = ms as f64 / 1000.0;
            let (mut e, mut f) = (Vec::new(), Vec::new());
            for w in d.windows(2).skip(20) {
                if near_jump(w[1].t) {
                    continue;
                }
                let (Some(a), Some(b)) = (self.lin(w[0].t - lag), self.lin(w[1].t - lag)) else { continue };
                e.push(w[1].pos.distance(b));
                f.push(((w[1].pos - w[0].pos) - (b - a)).length());
            }
            let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt();
            if !e.is_empty() && rms(&e) < best.1 {
                best = (ms as f32, rms(&e), rms(&f));
            }
        }
        best
    }
}

/// Offsets of effects against Sora (cm), per rule.
#[derive(Default, Clone)]
pub struct Offsets {
    pub trail: Vec<f32>,
    pub anchored: Vec<f32>,
    /// the anchored ones in their FIRST step (spawned since the step before)
    pub newborn: Vec<f32>,
}

pub fn stats(v: &[f32]) -> (f32, f32, f32) {
    if v.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    (s.iter().sum::<f32>() / s.len() as f32, s[((s.len() - 1) as f32 * 0.95).round() as usize], *s.last().unwrap())
}

pub struct RunOut {
    pub rows: Vec<(String, String, metrics::Report)>,
    /// (object, rule, best latency ms, RMS off the step path cm, frame displacement error RMS cm)
    pub judder: Vec<(String, String, f32, f32, f32)>,
    pub offsets: [Offsets; 3],
    /// frames that drew Sora or the trail BETWEEN the places before and after a teleport (per rule)
    pub teleport_between: [u32; 3],
    pub snaps: u32,
    pub steps: u32,
    pub notes: Vec<String>,
}

fn camera_basis(position: Vec3, rot: kh_camera::Rot) -> kh_vfx::Camera {
    let f = rot.forward();
    let r = kh_camera::Rot { pitch: 0.0, yaw: rot.yaw + 90.0, roll: 0.0 }.forward();
    kh_vfx::Camera { position, forward: f, right: r, up: f.cross(r).normalize_or(Vec3::Z) }
}

/// One frame's drawn geometry for the strips (KH3 cm).
#[allow(clippy::too_many_arguments)]
fn strip_frame(json: &mut String, t: f64, t_sim: f32, alpha: f32, rule: Rule, cam: (Vec3, kh_camera::Rot), bones: &[(Vec3, Vec3)], tip: Vec3, list: &kh_vfx::DrawList, firaga: Option<Vec3>, sora: Vec3) {
    let v = |p: Vec3| if p.is_finite() { format!("[{:.1},{:.1},{:.1}]", p.x, p.y, p.z) } else { "null".to_string() };
    let _ = write!(json, "{{\"t\":{t:.4},\"t_sim\":{t_sim:.4},\"alpha\":{alpha:.3},\"rule\":\"{}\",\"cam\":{},\"pitch\":{:.3},\"yaw\":{:.3},\"tip\":{},\"sora\":{},", rule.name(), v(cam.0), cam.1.pitch, cam.1.yaw, v(tip), v(sora));
    let _ = write!(json, "\"bones\":[{}],", bones.iter().map(|(a, b)| format!("[{},{}]", v(*a), v(*b))).collect::<Vec<_>>().join(","));
    let _ = write!(json, "\"ribbons\":[{}],", list.ribbons.iter().map(|r| format!("[{}]", r.points.iter().map(|p| format!("[{},{}]", v(p.a), v(p.b))).collect::<Vec<_>>().join(","))).collect::<Vec<_>>().join(","));
    let _ = write!(json, "\"quads\":[{}],", list.quads.iter().take(600).map(|q| format!("[{},{:.1}]", v(q.center), q.size)).collect::<Vec<_>>().join(","));
    let _ = write!(json, "\"meshes\":[{}],", list.meshes.iter().take(300).map(|m| v(Vec3::from(m.transform.translation))).collect::<Vec<_>>().join(","));
    let _ = writeln!(json, "\"firaga\":{}}},", firaga.map_or("null".to_string(), v));
}

/// Per Sora-anchored effect: (step, component, nodes) of the two latest steps.
type History = std::collections::HashMap<u64, [(u64, kh_vfx::Xform, Vec<Option<Vec3>>); 2]>;

/// Runs one scenario by rendered frames. `strip`: the sim-time window (s) whose frames' geometry is written.
pub fn run(s: &Scenario, clock: Clock, out_dir: Option<&std::path::Path>, strip: Option<(f32, f32)>) -> Option<RunOut> {
    let pack = project("build/sora_pack");
    let loadout = project("sheets/port/sora-loadout-99.json");
    let mut rig = Rig::new(&pack, &loadout, s.dummy.map(|d| Vec3::new(d, 0.0, 0.0))).ok()?;
    let mut effects = EffectHost::load(&pack).ok()?;
    effects.slash_follow = true; // sora-er-fx.json vfx.slash_follow = 1
    let script = Script::parse(s.script).ok()?;
    let (mut audio, mut seam) = (AudioQueue::default(), EffectQueue::default());
    let mut tap = PoseTap::default();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ (s.name.len() as u64 * 7919));
    let (mut acc, mut t_disp, mut t_sim, mut k) = (0.0f32, 0.0f64, 0.0f32, 0usize);
    let mut out_before = rig.out.clone();
    let mut pose_before = kh_anim::Pose::default();
    let mut cam: Option<CameraRig> = None;
    let (mut view, mut previous): (Option<CameraView>, Option<CameraView>) = (None, None);
    let mut last_position: Option<Vec3> = None;
    let mut snaps = 0u32;
    let mut disc: Vec<f64> = Vec::new(); // the sim times of his discontinuities (interp.rs `discontinuous`)
    let mut firaga: Option<kh_vfx::EffectId> = None;
    let mut snap_active = false; // a snap until the next step: everything at the latest step
    let (mut sora_t, mut tip_t, mut cam_t, mut fir_t, mut head_t) = (Track::default(), Track::default(), Track::default(), Track::default(), Track::default());
    let mut offsets: [Offsets; 3] = Default::default();
    let mut teleport_between = [0u32; 3];
    let mut unsnapped_between = 0u32; // the control: the same frames blended across the jumps (no snap)
    let mut raw_before = rig.out.position;
    let mut jumps: Vec<(Vec3, Vec3)> = Vec::new();
    let mut history: History = History::new();
    let mut csv = String::from("t,t_sim,alpha,rule,sora_x,sora_y,sora_z,tip_x,tip_y,tip_z,head_x,head_y,head_z,cam_x,cam_y,cam_z,cam_yaw,fir_x,fir_y,fir_z\n");
    let mut json = String::from("[\n");
    let mut notes = Vec::new();
    let mut steps = 0u32;
    let mut trail: Option<EffectHandle> = None;
    let mut trail_asked = false;
    while t_disp < s.secs as f64 {
        let dt = frame_dt(clock, &mut rng);
        t_disp += dt as f64;
        acc = (acc + dt).min(0.25);
        while acc >= STEP {
            acc -= STEP;
            // ---- Sora's step (sora.rs: the camera's yaw of the step before steers him)
            let camera_yaw = view.map_or(0.0, |v| v.rotation.yaw);
            let input = rig.input(&script, k, camera_yaw);
            // the Keyblade trail, as an attack's notify starts and ends it
            match (s.trail, trail) {
                (Some((a, _)), None) if t_sim >= a && !trail_asked => {
                    trail_asked = true;
                    let name = "key_trail0";
                    rig.sora.debug_emit(kh_core::DebugEmit::Effect(kh_core::EffectSpawn { at: kh_core::Attach::Sora { bone: Some("vfx_top"), offset: Vec3::ZERO }, persistent: true, ..kh_core::EffectSpawn::at_sora(kh_core::EffectId(kh_core::notify::notify_asset_id(name)), name) }));
                }
                (Some((_, b)), Some(h)) if t_sim >= b => {
                    rig.sora.debug_emit(kh_core::DebugEmit::StopEffect(h, false));
                    trail = None;
                }
                _ => {}
            }
            if s.teleports.iter().any(|at| (t_sim - at).abs() < STEP * 0.5) {
                let side = if jumps.len() % 2 == 0 { 2000.0 } else { -2000.0 };
                let p = rig.out.position + Vec3::new(0.0, side, 0.0);
                rig.sora.teleport(Pose { position: p, yaw: rig.out.yaw });
                jumps.push((rig.out.position, p));
            }
            snap_active = false;
            raw_before = rig.out.position;
            out_before = rig.out.clone();
            pose_before = std::mem::take(&mut rig.pose);
            rig.step_with(&input, &mut audio, &mut seam);
            audio.0.clear();
            if pose_before.component.is_empty() {
                pose_before = rig.pose.clone();
            }
            k += 1;
            steps += 1;
            t_sim += STEP;
            let out = rig.out.clone();
            if interp::discontinuous(&out_before, &out) {
                disc.push(t_sim as f64);
            }
            firaga = firaga.filter(|id| effects.world.is_alive(*id)).or_else(|| firaga_id(&rig, &effects));
            for e in &out.events {
                if let EventKind::EffectSpawn { handle, spawn } = &e.kind {
                    if spawn.name == "key_trail0" {
                        trail = Some(*handle);
                    }
                }
            }
            // ---- the effects' step (fx.rs `effect_step`)
            tap.record(out.step, &out, rig.animator.skeleton(), &rig.pose, &kh_host::rig::component_to_world(&out));
            tap.samples.clear();
            let jumped = last_position.is_some_and(|p| p.distance(out.position) > 300.0);
            last_position = Some(out.position);
            if jumped {
                effects.cut_trails();
                effects.snap();
            }
            let commands = std::mem::take(&mut seam.0);
            let dummy = rig.dummy.map(|d| d.info.center);
            let target = |_: kh_core::TargetId| dummy;
            effects.step(STEP, &commands, &Scene { out: &out, sora: &rig.sora, tap: &tap, target: &target, ground: Some(0.0), eye: view.map_or(out.position, |v| v.position) });
            if std::env::var_os("INT_DEBUG").is_some() && k % 6 == 0 {
                let names: Vec<String> = effects.world.live().iter().map(|l| format!("{} {:.2}s", l.name, l.seconds)).collect();
                eprintln!("DBG step {k} state {:?} trails {:?} live {names:?} log {:?}", out.state, effects.trail_ids(), effects.log.drain(..).collect::<Vec<_>>());
            }
            // ---- KH3's camera after the pawn (cam.rs `step`)
            let target_c = crate::cam::target_of(&out, &out_before, &rig.sora);
            let c = cam.get_or_insert_with(|| CameraRig::new(CAMERA_PARAMS, &target_c));
            let stick = s.camera_turn.filter(|(a, b, _)| (*a..*b).contains(&t_sim)).map_or(Vec2::ZERO, |(_, _, x)| Vec2::new(x, 0.0));
            let v = c.step(STEP, &CameraInput { right_stick: stick, recentre: false, extra_rotation: Vec2::ZERO }, &target_c, None, &rig.world);
            previous = Some(view.unwrap_or(v));
            view = Some(v);
            // ---- the steps' places
            let scene = Scene { out: &out, sora: &rig.sora, tap: &tap, target: &target, ground: Some(0.0), eye: Vec3::ZERO };
            sora_t.sim(t_sim as f64, out.position);
            tip_t.sim(t_sim as f64, effects.socket_point(&scene, "vfx_top"));
            cam_t.sim(t_sim as f64, v.position);
            if let Some(h) = effects.trail_ids().into_iter().find_map(|id| effects.world.drawn_trail_head(id, 1.0)) {
                head_t.sim(t_sim as f64, h);
            }
            if let Some(p) = firaga.and_then(|id| effects.world.latest_nodes(id)).and_then(|(_, n)| n.first().copied().flatten()) {
                fir_t.sim(t_sim as f64, p);
            }
            for (id, x) in effects.sora_anchored(&scene) {
                let nodes = effects.world.latest_nodes(id).map(|(_, n)| n).unwrap_or_default();
                let now = (out.step, x, nodes);
                let e = history.entry(id.0).or_insert_with(|| [now.clone(), now.clone()]);
                let before = e[1].clone();
                *e = [before, now];
            }
            history.retain(|_, h| h[1].0 == out.step);
        }
        // ---- sora.rs `int_snap_check` (once a frame, after the steps)
        if interp::discontinuous(&out_before, &rig.out) {
            out_before = rig.out.clone();
            pose_before = rig.pose.clone();
            previous = view;
            effects.snap();
            snaps += 1;
            snap_active = true;
        }
        let Some((v, p)) = view.zip(previous) else { continue };
        let true_alpha = (acc / STEP).clamp(0.0, 1.0);
        let out = rig.out.clone();
        for (ri, rule) in Rule::ALL.into_iter().enumerate() {
            let (a_sora, a_fx) = rule.alphas(true_alpha);
            let (a_sora, a_fx) = if snap_active { (1.0, 1.0) } else { (a_sora, a_fx) };
            // Sora as drawn (sora.rs `submit`)
            let (pose_d, c2w_d, _) = interp::sora_drawn(&out_before, &out, &pose_before, &rig.pose, &[], &[], a_sora);
            let yaw_d = interp::yaw_drawn(&out_before, &out, a_sora);
            let mut out_d = out.clone();
            (out_d.position, out_d.yaw) = (c2w_d.translation, yaw_d);
            let mut tap_d = PoseTap::default();
            tap_d.record(out.step, &out_d, rig.animator.skeleton(), &pose_d, &c2w_d);
            let dummy = rig.dummy.map(|d| d.info.center);
            let target = |_: kh_core::TargetId| dummy;
            let scene_d = Scene { out: &out_d, sora: &rig.sora, tap: &tap_d, target: &target, ground: Some(0.0), eye: Vec3::ZERO };
            let tip_d = effects.socket_point(&scene_d, "vfx_top");
            // (fx.rs draw_drawn) the effects on him on his drawn sockets
            if a_fx < 1.0 {
                effects.set_drawn(&scene_d);
            }
            // the camera as drawn (cam.rs `post`)
            let a_cam = if interp::camera_cut(&p, &v) { 1.0 } else { a_sora };
            let (cam_pos, cam_rot) = v.lerp(&p, a_cam);
            // the effects as drawn (fx.rs `draw_at`)
            let head = effects.trail_ids().into_iter().find_map(|id| effects.world.drawn_trail_head(id, a_fx));
            if let Some(h) = head {
                offsets[ri].trail.push(h.distance(tip_d));
                if h.distance(tip_d) > 5.0 && std::env::var_os("INT_DEBUG").is_some() {
                    eprintln!("TRAIL OUTLIER {} t {t_sim:.3} alpha {true_alpha:.2} a_fx {a_fx:.2} snap {snap_active} off {:.1} head {h:?} tip {tip_d:?}", rule.name(), h.distance(tip_d));
                }
                head_t.drawn(ri, t_disp, h);
            }
            for (id, x_d) in effects.sora_anchored(&scene_d) {
                let Some(h) = history.get(&id.0) else { continue };
                let drawn = effects.world.drawn_nodes(id, a_fx);
                let (before, latest) = (&h[0], &h[1]);
                let newborn = before.0 == latest.0;
                let (xb, xl, xd) = (before.1.affine().inverse(), latest.1.affine().inverse(), x_d.affine());
                for (i, d) in drawn.iter().enumerate() {
                    let (Some(d), Some(Some(nl))) = (d, latest.2.get(i)) else { continue };
                    // where his drawn socket puts this node: the drawn component x the node's place in its component (its
                    // place there at the latest step for an emitter in its first step, else between the two steps')
                    let nb = before.2.get(i).copied().flatten();
                    let first = newborn || nb.is_none();
                    let rel = match nb {
                        Some(nb) if !first => xb.transform_point3(nb).lerp(xl.transform_point3(*nl), a_sora),
                        _ => xl.transform_point3(*nl),
                    };
                    let off = d.distance(xd.transform_point3(rel));
                    if first {
                        offsets[ri].newborn.push(off);
                    } else {
                        offsets[ri].anchored.push(off);
                        if off > 5.0 && rule == Rule::Interp && std::env::var_os("INT_DEBUG").is_some() {
                            let name = effects.world.live().into_iter().find(|l| l.id == id).map(|l| (l.name, l.emitters.len()));
                            eprintln!("OUTLIER t {t_sim:.3} alpha {a_fx:.2} {name:?} emitter {i} off {off:.1} drawn {d:?} latest {nl:?} before {nb:?}");
                        }
                    }
                }
            }
            let fir_d = firaga.and_then(|id| effects.world.drawn_nodes(id, a_fx).first().copied().flatten()); // (its first emitter: its body)
            sora_t.drawn(ri, t_disp, c2w_d.translation);
            tip_t.drawn(ri, t_disp, tip_d);
            cam_t.drawn(ri, t_disp, cam_pos);
            if let Some(f) = fir_d {
                fir_t.drawn(ri, t_disp, f);
            }
            // a teleport: a frame between the place before it and the one after is a pop
            let between = |q: Vec3| {
                jumps.iter().any(|(from, to)| {
                    let (d0, d1) = (q.distance(*from), q.distance(*to));
                    d0 > 400.0 && d1 > 400.0 && d0 + d1 < from.distance(*to) * 1.02 // (beyond his reach of either place: his blade reaches 2 m)
                })
            };
            if rule == Rule::Interp && between(raw_before.lerp(out.position, true_alpha)) {
                unsnapped_between += 1;
            }
            if between(c2w_d.translation) || head.is_some_and(between) {
                teleport_between[ri] += 1;
                if std::env::var_os("INT_DEBUG").is_some() {
                    eprintln!("BETWEEN {} t {t_sim:.3} sora {:?} {} head {head:?} {} jumps {jumps:?}", rule.name(), c2w_d.translation, between(c2w_d.translation), head.is_some_and(between));
                }
            }
            let h = head.unwrap_or(Vec3::NAN);
            let f = fir_d.unwrap_or(Vec3::NAN);
            let _ = writeln!(csv, "{t_disp:.4},{t_sim:.4},{true_alpha:.3},{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.3},{:.2},{:.2},{:.2}", rule.name(), c2w_d.translation.x, c2w_d.translation.y, c2w_d.translation.z, tip_d.x, tip_d.y, tip_d.z, h.x, h.y, h.z, cam_pos.x, cam_pos.y, cam_pos.z, cam_rot.yaw, f.x, f.y, f.z);
            if let Some((from, to)) = strip {
                if (from..to).contains(&t_sim) {
                    let sk = rig.animator.skeleton();
                    let world: Vec<Vec3> = pose_d.component.iter().map(|b| c2w_d.transform_point(b.translation)).collect();
                    let bones: Vec<(Vec3, Vec3)> = sk.bones.iter().enumerate().filter_map(|(i, b)| Some((*world.get(i)?, *world.get(b.parent?)?))).filter(|(a, b)| a.distance(*b) < 80.0).collect();
                    let (list, _, _) = effects.draw(a_fx, &camera_basis(cam_pos, cam_rot));
                    strip_frame(&mut json, t_disp, t_sim, true_alpha, rule, (cam_pos, cam_rot), &bones, tip_d, list, fir_d, c2w_d.translation);
                }
            }
            effects.clear_drawn();
        }
    }
    let mut rows = Vec::new();
    let mut judder = Vec::new();
    for (name, t) in [("Sora root", &sora_t), ("blade tip", &tip_t), ("camera", &cam_t), ("trail head", &head_t), ("Firaga", &fir_t)] {
        if t.sim.len() < 10 || t.drawn[0].len() < 10 {
            continue;
        }
        for (ri, rule) in Rule::ALL.into_iter().enumerate() {
            rows.push((name.to_string(), rule.name().to_string(), metrics::report(&t.drawn[ri], &t.sim, STEP)));
            let (ms, off, f2f) = t.judder(ri, &disc);
            judder.push((name.to_string(), rule.name().to_string(), ms, off, f2f));
        }
    }
    if let Some(dir) = out_dir {
        let _ = std::fs::create_dir_all(dir);
        let base = format!("{}_{}", s.name.replace([' ', '/', '(', ')', '.', '+'], "_"), match clock { Clock::Jitter => "jitter", Clock::Steady => "steady" });
        let _ = std::fs::write(dir.join(format!("{base}.csv")), csv);
        if strip.is_some() {
            json.push_str("null]\n");
            let _ = std::fs::write(dir.join(format!("{base}_strip.json")), json);
        }
    }
    notes.push(format!("{} teleport(s), {snaps} frame snap(s) (a frame whose latest step is the jump); blended across the jumps instead (the control), {unsnapped_between} frame(s) would show him between the two places", jumps.len()));
    Some(RunOut { rows, judder, offsets, teleport_between, snaps, steps, notes })
}

/// The Firaga's body effect: the live system whose component is at one of his spell actors.
fn firaga_id(rig: &Rig, effects: &EffectHost) -> Option<kh_vfx::EffectId> {
    let p = rig.sora.magic().projectile_views().next()?.position;
    effects.world.live().into_iter().map(|l| l.id).find(|id| effects.world.latest_nodes(*id).is_some_and(|(x, _)| x.position.distance(p) < 1.0))
}

fn out_dir() -> Option<PathBuf> {
    std::env::var_os("INT_OUT").map(PathBuf::from)
}

/// THE TABLE (`INT | ...` lines) and the CSVs / strips; the assertions are in `int_the_whole_frame_is_between_the_steps`.
#[test]
fn int_measure_all() {
    let out = out_dir();
    eprintln!("INT | scenario | clock | object | rule | {} ", metrics::HEADER);
    for s in scenarios() {
        for clock in [Clock::Jitter, Clock::Steady] {
            let strip = match (s.name, clock) {
                ("combo with trails", Clock::Jitter) => Some((1.20, 1.62)),
                ("firaga in flight", Clock::Jitter) => Some((1.00, 1.40)),
                ("teleports mid-combo", Clock::Jitter) => Some((1.36, 1.56)),
                _ => None,
            };
            let Some(o) = run(&s, clock, out.as_deref(), strip) else {
                eprintln!("SKIPPED: no pack (build/sora_pack) or loadout");
                return;
            };
            let c = format!("{clock:?}");
            for (obj, rule, rep) in &o.rows {
                eprintln!("INT | {:<20} | {:<6} | {:<10} | {:<6} | {}", s.name, c, obj, rule, metrics::line(rep));
            }
            for (obj, rule, ms, off, f2f) in &o.judder {
                eprintln!("INT DRAW | {:<20} | {:<6} | {:<10} | {:<6} | the drawn place = its own step path {ms:.0} ms late; off it RMS {off:.2} cm; frame displacement error RMS {f2f:.2} cm", s.name, c, obj, rule);
            }
            for (ri, rule) in Rule::ALL.into_iter().enumerate() {
                let (t, a, n) = (stats(&o.offsets[ri].trail), stats(&o.offsets[ri].anchored), stats(&o.offsets[ri].newborn));
                eprintln!(
                    "INT FX | {:<20} | {:<6} | {:<6} | trail head - drawn blade tip cm mean/p95/max {:.2}/{:.2}/{:.2} ({} frames) | socket effects - his drawn socket {:.2}/{:.2}/{:.2} ({}) | in their first step {:.2}/{:.2}/{:.2} ({}) | frames between a teleport's two places {}",
                    s.name, c, rule.name(), t.0, t.1, t.2, o.offsets[ri].trail.len(), a.0, a.1, a.2, o.offsets[ri].anchored.len(), n.0, n.1, n.2, o.offsets[ri].newborn.len(), o.teleport_between[ri]
                );
            }
            eprintln!("INT NOTE | {:<20} | {:<6} | {} steps; {}", s.name, c, o.steps, o.notes.join("; "));
        }
    }
}

/// THE ACCEPTANCE: under jitter, interpolated, the drawing adds no judder to anything of ours (the drawn place is its own
/// step path one step late), the effects stay ON him (trail head on the blade, socket effects on his sockets), nothing
/// is drawn between the two places of a teleport; and the camera is its own path one step late (no smoothing on top of
/// KH3's own).
#[test]
fn int_the_whole_frame_is_between_the_steps() {
    for s in &scenarios() {
        let Some(o) = run(s, Clock::Jitter, None, None) else {
            eprintln!("skipped: no pack");
            return;
        };
        let j = |obj: &str, rule: &str| o.judder.iter().find(|r| r.0 == obj && r.1 == rule).map(|r| (r.2, r.3, r.4));
        for obj in ["Sora root", "blade tip", "camera", "trail head", "Firaga"] {
            let (Some(live), Some(int)) = (j(obj, "live"), j(obj, "interp")) else { continue };
            // (the blade tip and the trail head turn on arcs between the steps: their "step path" is a chord; ratio only)
            let arc = matches!(obj, "blade tip" | "trail head");
            assert!((arc || int.2 < 0.6) && int.2 < live.2 * 0.25 + 0.05, "{} / {obj}: frame displacement error interp {:.2} cm vs live {:.2} cm", s.name, int.2, live.2);
            assert!((int.0 - 17.0).abs() <= 1.0, "{} / {obj}: the drawn place is not its step path one step late ({} ms)", s.name, int.0);
        }
        let ri = 2;
        let (t, a, n) = (stats(&o.offsets[ri].trail), stats(&o.offsets[ri].anchored), stats(&o.offsets[ri].newborn));
        assert!(t.1 < 1.0, "{}: the trail's head is {:.2} cm (p95) off the drawn blade tip", s.name, t.1);
        assert!(a.1 < 1.0, "{}: an effect on his socket is {:.2} cm (p95) off his drawn socket", s.name, a.1);
        assert!(n.1 < 1.0, "{}: an effect in its first step is {:.2} cm (p95) off his drawn socket", s.name, n.1);
        assert_eq!(o.teleport_between[ri], 0, "{}: a frame drawn between the two places of a teleport", s.name);
    }
}
