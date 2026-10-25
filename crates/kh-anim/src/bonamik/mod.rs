//! Bonamik: Square Enix's hair / cloth secondary-motion solver (BonamikRt / KBDRt) as it runs for
//! Sora (docs/sora-animation-spec.md section 14; recipe: docs/sora-animation-spec-changes.md B4).
//!
//! A port, module for module, of the reference evaluator `tools/lib/bonamik/*.js`, which is a
//! transcription of the exe in float32 with the exe's operation order
//! (`build/sora_tmp/native_H_bonamik2.md`):
//!
//! | JS | here |
//! |---|---|
//! | `m.js` | [`m`] float32 helpers |
//! | `core.js` | [`core`] group / bone weights, time step, setup blocks |
//! | `cone.js` | [`cone`] cone constraint |
//! | `chain.js` | [`chain`] chain links, write-back, joint twist |
//! | `coll.js` | [`coll`] collision |
//! | `kdi.js` | [`kdi`] KDI and pin constraints |
//! | `reset.js` | [`reset`] frame transforms, teleport, reset, pre-step, local force |
//! | `wind.js` | [`wind`] integrate, aero, random stream, anchors, lateral links, velocity update |
//! | `hfps.js` | [`hfps`] the 30 Hz evaluation timer and the interpolation of the component |
//! | `bnm_eval.js` | [`bnm_eval`] setup, Step, CoreUpdate; its `create` is [`Bonamik::new`] |
//! | `notify.js` | [`notify`] the notify states' effects |
//! | (implicit JS objects) | [`records`] particle, link, cone, ... with the exe's offsets as field names |
//!
//! RULES OF THE PORT. Every `Math.fround` boundary of the JS is one `f32` operation here, in the
//! same order; nothing is simplified, reordered or fused. Where the JS computes in double before
//! rounding (`1 / Math.sqrt`, `Math.sin`, `Math.acos`, `Math.pow`, `Math.log`, `Math.exp`,
//! `Math.hypot`), so does this. Functions keep the exe addresses as names (`fn_143b166d0`).
//! Branches the JS does not transcribe (dead for Sora's data) are `unreachable!` with the JS's
//! message; the ones that depend only on the asset are refused at setup with an `Err`.
//!
//! UNITS: UE component space of the mesh, centimetres (the crate's own space); the solver's
//! constants are in those units. [`crate::Trs`] values are copied to the JS's `{q, t, s}` arrays
//! without changing a bit.
//!
//! THE FRAME DRIVER ([`Bonamik::frame`], hfps.js `createRunner`): a timer of 1/30 s with the
//! overshoot carried; the solver is evaluated on the frame where the timer expires (the first two
//! ticks both evaluate) with solver step 0.05 s at >= 30 fps; two buffers per driven bone hold
//! the delta of the simulated local transform against the animated one (prev, cur); a frame
//! shows animated local * delta interpolated with a(a+1)/2, an evaluation frame shows prev. So
//! the output is always ONE EVALUATION BEHIND by construction. It must be driven with the
//! SIMULATION step dt (the app steps at 60 Hz), never at a render rate.
//!
//! DISPLAY BETWEEN STEPS (decision, recipe item 3). The app draws between two simulation steps
//! by interpolating the two latest [`crate::Pose`]s ([`crate::Pose::lerp`]). The bones Bonamik
//! writes are treated like every other bone there: their final local transforms of the two
//! latest steps are interpolated. This adds up to one simulation step (1/60 s) of display lag on
//! top of the built-in one-evaluation lag; it is the same lag every other bone has, so the cloth
//! stays in phase with the body it hangs from.
//!
//! START STATE: the nodes at the mesh reference pose with a soft reset pending, no pre-roll.
//! JITTER: zeroed by default ([`Jitter::Zero`]); a logged sequence of draws or the JS's seeded
//! xorshift stream can be supplied. HOST INPUTS ([`BonamikHost`]): world wind (default 0), ground
//! (default none), time dilation (default 1). Gravity comes from the asset.
//!
//! NOT COVERED: comparison with the real game, level-script resets at spawn, per-map wind, the
//! ground clamp in practice (recipe item 9).

/// (ER69) The branches of the port that run a FALLBACK instead of the game's code (bit n = branch n of `unported`): a
/// host may report them. Never a stop: an outfit's asset may reach branches Sora's never does.
pub static UNPORTED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub(crate) fn unported(branch: u32) {
    UNPORTED.fetch_or(1 << branch.min(31), std::sync::atomic::Ordering::Relaxed);
}

pub mod bnm_eval;
pub mod chain;
pub mod coll;
pub mod cone;
pub mod core;
pub mod hfps;
pub mod kdi;
pub mod m;
pub mod notify;
// (ER69) limit planes against spheres / capsules (the outfits' assets)
pub mod plane;
pub mod records;
pub mod reset;
pub mod wind;

use glam::{Quat, Vec3};

use crate::json::Json;
use crate::pack::Skeleton;
use crate::Trs;

use self::m::Xf;
use self::notify::{CharaWind, GroupParam, NotifyState};
use self::records::{Cfg, Core, Owner, Stats};
use self::wind::Rng;

pub use self::wind::DEFAULT_SEED;

/// The random stream of the aero force (three draws per dynamic particle per sub-step, in
/// particle order).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Jitter {
    /// The generator returns the constant that makes the jitter 0 (`zeroJitter` of the JS).
    #[default]
    Zero,
    /// The JS's `WIND.makeRng(seed)`: xorshift128 from this state ([`DEFAULT_SEED`] = the JS's
    /// default, the per-thread initial state).
    Seeded([u32; 4]),
    /// A logged sequence of draws; zero jitter once it is exhausted.
    Logged(Vec<u32>),
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct BonamikOptions {
    /// `hardResetAtStart`: the first evaluation does a hard reset (as Reset(core, 1) would).
    pub hard_reset_at_start: bool,
    pub jitter: Jitter,
    /// `teleportDistance` (cm); default: the asset user data's (800).
    pub teleport_distance: Option<f32>,
}

/// What the host supplies per frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BonamikHost {
    /// World wind vector.
    pub wind: Vec3,
    /// Ground query result: (point, normal), world space.
    pub ground: Option<(Vec3, Vec3)>,
    pub time_dilation: f32,
}

impl Default for BonamikHost {
    fn default() -> BonamikHost {
        BonamikHost { wind: Vec3::ZERO, ground: None, time_dilation: 1.0 }
    }
}

/// What the JS runner's `step` returns besides the pose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FrameInfo {
    /// The solver was evaluated on this frame.
    pub did_eval: bool,
    /// The dt of that evaluation (the timer's).
    pub eval_dt: f32,
    pub remaining: f32,
    pub interval: f32,
    pub alpha_linear: f32,
    /// The eased interpolation alpha, a(a+1)/2.
    pub alpha: f32,
}

/// The character wind as it stands in a core (`core+0x3720..0x3744`, group scale `+0x954`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CharaWindState {
    pub direction_space: i32,
    pub direction: [f32; 3],
    pub base_strength: f32,
    pub wave_amplitude: f32,
    pub frequency: f32,
    pub scale: f32,
}

pub struct Bonamik {
    core: Core,
    cfg: Cfg,
    comp: hfps::Comp,
    rng: Rng,
    stats: Stats,
    notify: NotifyState,
    /// Clip 0033's notify rows (`data/bonamik.json` -> `notify_states`).
    rows_0033: Vec<GroupParam>,
    wind_0033: Option<CharaWind>,
    wind_ticks: u32,
}

fn to_xf(t: &Trs) -> Xf {
    Xf { q: [t.rotation.x, t.rotation.y, t.rotation.z, t.rotation.w], t: [t.translation.x, t.translation.y, t.translation.z], s: [t.scale.x, t.scale.y, t.scale.z] }
}

fn to_trs(x: &Xf) -> Trs {
    Trs { translation: Vec3::new(x.t[0], x.t[1], x.t[2]), rotation: Quat::from_xyzw(x.q[0], x.q[1], x.q[2], x.q[3]), scale: Vec3::new(x.s[0], x.s[1], x.s[2]) }
}

/// The component-space reference pose as `tools/lib/bonamik/anim.js` computes it
/// (`componentSpace(refLocal())`): composed in double precision from the local reference pose,
/// then rounded to float32. (The crate's `component_space` composes in `f32`; the last bits
/// differ, and the setup records are compared bit for bit against the JS's.)
pub fn reference_pose(skeleton: &Skeleton) -> Vec<Trs> {
    let qmul = |a: [f64; 4], b: [f64; 4]| [a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1], a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0], a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3], a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]];
    let qrot = |q: [f64; 4], v: [f64; 3]| {
        let (tx, ty, tz) = (2.0 * (q[1] * v[2] - q[2] * v[1]), 2.0 * (q[2] * v[0] - q[0] * v[2]), 2.0 * (q[0] * v[1] - q[1] * v[0]));
        [v[0] + q[3] * tx + (q[1] * tz - q[2] * ty), v[1] + q[3] * ty + (q[2] * tx - q[0] * tz), v[2] + q[3] * tz + (q[0] * ty - q[1] * tx)]
    };
    let mut cs: Vec<([f64; 4], [f64; 3], [f64; 3])> = Vec::with_capacity(skeleton.bones.len());
    for bone in &skeleton.bones {
        let r = &bone.reference;
        let (lq, lt, ls) = ([r.rotation.x as f64, r.rotation.y as f64, r.rotation.z as f64, r.rotation.w as f64], [r.translation.x as f64, r.translation.y as f64, r.translation.z as f64], [r.scale.x as f64, r.scale.y as f64, r.scale.z as f64]);
        let value = match bone.parent {
            None => (lq, lt, ls),
            Some(parent) => {
                let (pq, pt, ps) = cs[parent];
                let rt = qrot(pq, [ps[0] * lt[0], ps[1] * lt[1], ps[2] * lt[2]]);
                let q = qmul(pq, lq);
                let l = notify::js_hypot(&q);
                ([q[0] / l, q[1] / l, q[2] / l, q[3] / l], [rt[0] + pt[0], rt[1] + pt[1], rt[2] + pt[2]], [ls[0] * ps[0], ls[1] * ps[1], ls[2] * ps[2]])
            }
        };
        cs.push(value);
    }
    cs.iter().map(|(q, t, s)| Trs { translation: Vec3::new(t[0] as f32, t[1] as f32, t[2] as f32), rotation: Quat::from_xyzw(q[0] as f32, q[1] as f32, q[2] as f32, q[3] as f32), scale: Vec3::new(s[0] as f32, s[1] as f32, s[2] as f32) }).collect()
}

/// World gravity from the asset (0x143b61cf0): the asset's m_Gravity with Y and Z swapped, * 100.
fn gravity_of(data: &Json) -> [f32; 3] {
    let gv = data.get("scene").get("gravity").floats();
    let gv: [f32; 3] = if gv.len() >= 3 { [gv[0], gv[1], gv[2]] } else { [0.0, -9.81_f64 as f32, 0.0] };
    // f(Math.sqrt(f(f(f(x*x) + f(z*z) + f(y*y)) * 10000))): the three squares are summed in double
    let sum = ((gv[0] * gv[0]) as f64 + (gv[2] * gv[2]) as f64 + (gv[1] * gv[1]) as f64) as f32;
    let gmag = (((sum as f64) * 10000.0) as f32 as f64).sqrt() as f32;
    let ginv = 1.0 / gmag;
    let gdir = [(ginv * gv[0]) * 100.0, (gv[2] * 100.0) * ginv, (gv[1] * 100.0) * ginv]; // (x, z, y)
    [gmag * gdir[0], gmag * gdir[1], gmag * gdir[2]]
}

impl Bonamik {
    /// From `data/bonamik.json` of the pack and the skeleton. Setup runs on the mesh reference
    /// pose ([`reference_pose`]).
    pub fn new(data: &Json, skeleton: &Skeleton, options: BonamikOptions) -> Result<Bonamik, String> {
        let reference = reference_pose(skeleton);
        Bonamik::with_reference_pose(data, skeleton, &reference, options)
    }

    /// As [`Bonamik::new`], with the component-space reference pose given (`refPose` of the JS).
    pub fn with_reference_pose(data: &Json, skeleton: &Skeleton, reference: &[Trs], options: BonamikOptions) -> Result<Bonamik, String> {
        if reference.len() != skeleton.bones.len() || reference.is_empty() {
            return Err("Bonamik: the reference pose does not match the skeleton".to_string());
        }
        let ref_bones: Vec<Xf> = reference.iter().map(to_xf).collect();
        let owner = Owner {
            bones: ref_bones.clone(),
            ref_bones,
            parents: skeleton.bones.iter().map(|b| b.parent.map_or(-1, |p| p as i32)).collect(),
            names: skeleton.bones.iter().map(|b| b.name.clone()).collect(),
            world: Xf::IDENT,
            gravity: gravity_of(data),
            wind: [0.0; 3],
            ground: None,
            time_dilation: 1.0,
        };
        let cfg = Cfg::default();
        let mut core = bnm_eval::setup_core(data, owner, &cfg, options.teleport_distance)?;
        if options.hard_reset_at_start {
            core.f33f4 = 1;
            core.f33f5 = 1;
        }
        let rng = match options.jitter {
            Jitter::Zero => Rng::Zero,
            Jitter::Seeded(seed) => Rng::Xorshift(seed),
            Jitter::Logged(draws) => Rng::Logged { draws, at: 0 },
        };
        let mut rows_0033 = Vec::new();
        let mut wind_0033 = None;
        for n in data.get("notify_states").get("0033").get("notifies").items() {
            let class = n.get("class").as_str().unwrap_or("");
            if class.contains("ChangeGroupParams") {
                rows_0033 = notify::group_params_from_json(n.get("props").get("m_GroupParams"))?;
            } else if class.contains("CharaWind") {
                wind_0033 = Some(notify::chara_wind_from_json(n.get("props"))?);
            }
        }
        let notify = NotifyState::new(&core);
        Ok(Bonamik { core, cfg, comp: hfps::Comp::default(), rng, stats: Stats::default(), notify, rows_0033, wind_0033, wind_ticks: 0 })
    }

    fn set_host(&mut self, component_to_world: &Trs, host: &BonamikHost) {
        let owner = &mut self.core.f3848;
        owner.world = to_xf(component_to_world);
        owner.wind = [host.wind.x, host.wind.y, host.wind.z];
        owner.ground = host.ground.map(|(p, n)| ([p.x, p.y, p.z], [n.x, n.y, n.z]));
        owner.time_dilation = host.time_dilation;
    }

    fn load(&mut self, pose: &[Trs]) {
        let bones = &mut self.core.f3848.bones;
        assert_eq!(pose.len(), bones.len(), "Bonamik: the pose has another bone count than the skeleton");
        for (b, t) in bones.iter_mut().zip(pose) {
            *b = to_xf(t);
        }
    }

    fn store(&self, pose: &mut [Trs]) {
        for (t, b) in pose.iter_mut().zip(&self.core.f3848.bones) {
            *t = to_trs(b);
        }
    }

    /// One simulation step (the JS runner's `step`): `pose` = the component-space transforms
    /// after leg IK and KineDriver (UE, cm), rewritten in place for the driven bones.
    pub fn frame(&mut self, dt: f32, pose: &mut [Trs], component_to_world: &Trs, host: &BonamikHost) -> FrameInfo {
        self.set_host(component_to_world, host);
        self.load(pose);
        let n = pose.len();
        hfps::fn_143b5b990(&mut self.comp, dt, n);
        let (did_eval, eval_dt) = (self.comp.f4ac != 0, self.comp.f4a0);
        self.comp.f42c = dt;
        let (alpha_linear, alpha) = hfps::fn_143b590d0(&mut self.comp, &mut self.core, &self.cfg, &mut self.rng, &mut self.stats, dt);
        self.store(pose);
        FrameInfo { did_eval, eval_dt, remaining: self.comp.f4a4, interval: self.comp.f4a8, alpha_linear, alpha }
    }

    /// One core evaluation in place, without the timer and the interpolation path (the JS's
    /// `sim.evaluate`: the core blends by itself).
    pub fn evaluate(&mut self, pose: &mut [Trs], eval_dt: f32, component_to_world: &Trs, host: &BonamikHost) {
        self.set_host(component_to_world, host);
        self.load(pose);
        bnm_eval::core_update(&mut self.core, &self.cfg, eval_dt, &mut self.rng, &mut self.stats);
        self.store(pose);
    }

    /// The bones with a Bonamik weight (the simulation may write them), ascending. Every other
    /// bone leaves [`Bonamik::frame`] bit-identical to the incoming pose.
    pub fn driven_bones(&self) -> Vec<usize> {
        // BoneWeights (0x143b11700) on a copy: the weights of the current group states.
        let mut copy = self.core.clone();
        core::fn_143b11700(&mut copy);
        copy.f3810.iter().enumerate().filter(|(_, w)| **w > 0.0).map(|(i, _)| i).collect()
    }

    /// The particle positions (component space, cm), in particle order.
    pub fn particle_positions(&self) -> Vec<Vec3> {
        self.core.f3778.f0.iter().map(|p| Vec3::new(p.f0[0], p.f0[1], p.f0[2])).collect()
    }

    /// The hash `validate.js` compares states with: FNV-1a (32 bit) over the float32 bit
    /// patterns of every particle's position, velocity and quaternion.
    pub fn state_hash(&self) -> u64 {
        let mut h: u32 = 0x811c9dc5;
        for p in &self.core.f3778.f0 {
            for v in p.f0.iter().chain(&p.f40).chain(&p.f20) {
                h = (h ^ v.to_bits()).wrapping_mul(0x0100_0193);
            }
        }
        h as u64
    }

    /// The core record (for tests and cross-checks).
    pub fn core(&self) -> &Core {
        &self.core
    }

    /// Evaluations and resets so far.
    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// World gravity as derived from the asset (cm/s^2).
    pub fn gravity(&self) -> Vec3 {
        let g = self.core.f3848.gravity;
        Vec3::new(g[0], g[1], g[2])
    }

    /// Forces an evaluation on the next frame (0x143b5c470).
    pub fn force_evaluation(&mut self) {
        hfps::fn_143b5c470(&mut self.comp);
    }

    /// (party A2) Ground collision on / off per group (core 0x143b06200 [C]: `core+0x924[g] = on` for every group named
    /// in `groups` (FName compare: case-insensitive), every group when `groups` is empty). What
    /// `TresAnimNotifyState_Bonamik_EnableGroundCollision` calls (begin 0x140562500: m_Enable; end 0x140564EB0: !m_Enable).
    pub fn set_ground_collision(&mut self, groups: &[String], on: bool) {
        let c = &mut self.core;
        let n = c.f410;
        for g in 0..n {
            if groups.is_empty() || groups.iter().any(|name| c.f4d8[g].eq_ignore_ascii_case(name)) {
                c.f924[g] = on as u8;
            }
        }
    }

    /// (party A2) The group indices a notify's `m_Groups` names (case-insensitive FName compare; empty = every group).
    fn named_groups(&self, groups: &[String]) -> Vec<usize> {
        let c = &self.core;
        (0..c.f410).filter(|g| groups.is_empty() || groups.iter().any(|n| c.f4d8[*g].eq_ignore_ascii_case(n))).collect()
    }

    /// (party A2) `TresAnimNotifyState_Bonamik_ScaleGroupParams` (begin 0x140562650 with the notify's scales, end
    /// 0x140565010 with 1.0), the core setters [C]: inner cone 0x143b06af0 -> core+0xB94[g] clamped to [0, 10000], outer
    /// cone 0x143b06df0 -> +0xC54[g] clamped, local force 0x143b06bf0 -> +0xD14[g], wind 0x143b07480 -> +0xDD4[g], mass
    /// 0x143b06d20 -> +0xE94[g] (raw). `None` = that m_Change* flag is off.
    pub fn set_group_scales(&mut self, groups: &[String], inner: Option<f32>, outer: Option<f32>, local_force: Option<f32>, wind: Option<f32>, mass: Option<f32>) {
        let clamp = |v: f32| if v >= 0.0 { v.min(10000.0) } else { 0.0 };
        for g in self.named_groups(groups) {
            let c = &mut self.core;
            if let Some(v) = inner {
                c.fb94[g] = clamp(v);
            }
            if let Some(v) = outer {
                c.fc54[g] = clamp(v);
            }
            if let Some(v) = local_force {
                c.fd14[g] = v;
            }
            if let Some(v) = wind {
                c.fdd4[g] = v;
            }
            if let Some(v) = mass {
                c.fe94[g] = v;
            }
        }
    }

    /// (party A2) `TresAnimNotifyState_Bonamik_Weight` (begin 0x140562840: m_BeginBonamikWeight / m_BeginTime; end
    /// 0x140565210: m_EndBonamikWeight / m_EndTime) -> KBD 0x143b5f4b0 -> core 0x143b07270 [C]: per named group
    /// +0x34B8 = 0, +0x33F8 = time, +0x3638 = 1 - weight, state +0x804 = 2 (weight > 1e-4) else 4; with no group named,
    /// every group whose state is not 0 (state 3 unless it is 1 or 4). [?] In the named branch the decompiled C writes
    /// the state 4 through a pointer that stays on group 0; read here as the matched group. m_ResetWhenNonZeroWeight
    /// (0x143b5ed70, inst+0x387B) is not applied.
    pub fn set_group_weight(&mut self, groups: &[String], weight: f32, time: f32) {
        let all = groups.is_empty();
        for g in self.named_groups(groups) {
            let c = &mut self.core;
            if all && c.f804[g] == 0 {
                continue;
            }
            c.f34b8[g] = 0.0;
            c.f33f8[g] = time;
            c.f3638[g] = 1.0 - weight;
            c.f804[g] = if weight > 0.0001 {
                2
            } else if all {
                if c.f804[g] != 1 && c.f804[g] != 4 { 3 } else { c.f804[g] }
            } else {
                4
            };
        }
    }

    /// (party A2) `TresAnimNotifyState_Bonamik_Enable` (bnmsub_notifies_physics.md 1b): TurnOn 0x143B0EBD0 (`on`) /
    /// TurnOff 0x143B0EA70, then the forced evaluation (0x143B5C470). TurnOff's m_PreserveLatestResult (0x143AF1100) is
    /// not applied.
    pub fn turn_groups(&mut self, groups: &[String], on: bool, blend_time: f32) {
        let all = groups.is_empty();
        for g in self.named_groups(groups) {
            let c = &mut self.core;
            if on {
                let allowed = if all { c.f804[g] & !2 != 0 } else { c.f804[g] != 2 };
                if allowed {
                    c.f65c = 1;
                    c.f33f8[g] = blend_time;
                    c.f34b8[g] = 0.0;
                    c.f804[g] = 2;
                    c.f3638[g] = 0.0;
                }
            } else {
                if c.f804[g] > 1 {
                    c.f33f8[g] = blend_time;
                    c.f34b8[g] = 0.0;
                    c.f3638[g] = 1.0;
                    if all {
                        c.f804[g] = 3;
                    }
                }
                if !all {
                    c.f804[g] = 4;
                }
            }
        }
        self.force_evaluation();
    }

    /// (party A2) `SetPreRoll(n)` (core 0x143b06ef0: `if (n > +0x33EC) { +0x33EA = 1 ; +0x33EC = n }`): the next
    /// evaluation with a group active replaces its Step by n Steps (x2 in the 60 fps bucket), the first one with
    /// velocity scale 0 (CoreUpdate's pre-roll loop, bnm_eval::core_update).
    pub fn set_pre_roll(&mut self, n: i32) {
        if n > self.core.f33ec {
            self.core.f33ea = 1;
            self.core.f33ec = n;
        }
    }

    /// Asks for a reset on the next evaluation (`core+0x33F4`, hard: `+0x33F5`), as the
    /// teleport check (soft) or Reset(core, 1) (hard) would.
    pub fn request_reset(&mut self, hard: bool) {
        self.core.f33f4 = 1;
        self.core.f33f5 = hard as u8;
    }

    /// The active character wind when its notify named no group (= every group). Such a wind
    /// also reaches the KBD components attached below this mesh (0x143b5ef60 with "children"
    /// set -> 0x143b53410): see [`crate::keychain`].
    pub fn chara_wind_all_groups(&self) -> Option<CharaWindState> {
        let c = &self.core;
        if c.f3744 == 0 || c.f410 == 0 || self.notify.chara_wind_groups().len() != c.f410 {
            return None;
        }
        Some(CharaWindState { direction_space: c.f3720, direction: c.f3724, base_strength: c.f3738, wave_amplitude: c.f373c, frequency: c.f3740, scale: c.f954[0] })
    }

    /// Sets a character wind on every group of this core (the same writes as `set_chara_wind`
    /// with no group names, from values already converted).
    pub fn set_chara_wind_all_groups(&mut self, wind: &CharaWindState) {
        let c = &mut self.core;
        c.f3720 = wind.direction_space;
        c.f3724 = wind.direction;
        c.f3738 = wind.base_strength;
        c.f373c = wind.wave_amplitude;
        c.f3740 = wind.frequency;
        c.f3744 = 1;
        for g in 0..c.f410 {
            c.f954[g] = wind.scale;
        }
    }

    /// Ends a wind set by [`Bonamik::set_chara_wind_all_groups`].
    pub fn clear_chara_wind_all_groups(&mut self) {
        let c = &mut self.core;
        for g in 0..c.f410 {
            c.f954[g] = 0.0;
        }
        c.f3744 = 0;
    }

    // ---- notify.js
    /// `setGroupParam(sim, rows)`
    pub fn set_group_param(&mut self, rows: &[GroupParam]) {
        self.notify.set_group_param(&mut self.core, rows);
    }

    /// `restoreGroupParam(sim, rows)`
    pub fn restore_group_param(&mut self, rows: &[GroupParam]) {
        self.notify.restore_group_param(&mut self.core, rows);
    }

    /// (integration pass) `SetSpeedRate(rate, blendTime)` of a core (0x143B071F0, decompiled C;
    /// the KBD component's 0x143B04E70 is the same on its core): target (+0x370C) = max(rate, 0);
    /// with a blend time below 0.0001 the rate (+0x3708) is set at once and the change (+0x3710)
    /// cleared, else change per second = (target - rate) / blendTime; the time step (core.rs)
    /// then moves the rate to the target and scales the simulated time by it. What
    /// `TresAnimNotifyState_Bonamik_SpeedRate` calls at its begin and end.
    pub fn set_speed_rate(&mut self, rate: f32, blend_time: f32) {
        let target = rate.max(0.0);
        self.core.f370c = target;
        if blend_time < 0.0001 {
            self.core.f3708 = target;
            self.core.f3710 = 0.0;
        } else {
            self.core.f3710 = (target - self.core.f3708) / blend_time;
        }
    }

    /// The speed rate in force (core+0x3708; 1.0 after setup).
    pub fn speed_rate(&self) -> f32 {
        self.core.f3708
    }

    /// `setCharaWind(sim, props)`
    pub fn set_chara_wind(&mut self, props: &CharaWind) {
        self.notify.set_chara_wind(&mut self.core, props);
    }

    /// `setCharaWindScale(sim, scale)`
    pub fn set_chara_wind_scale(&mut self, scale: f32) {
        self.notify.set_chara_wind_scale(&mut self.core, scale);
    }

    /// `clearCharaWind(sim)`
    pub fn clear_chara_wind(&mut self) {
        self.notify.clear_chara_wind(&mut self.core);
    }

    /// Clip 0033, `TresAnimNotifyState_Bonamik_ChangeGroupParams` begin: the group-parameter rows
    /// (WindLift on SLEEVE / JCT / HAIR joints; InnerConeStr 0.12 on the cone records owned by
    /// SLEEVE / HOOD / JCT joints).
    pub fn notify_group_params_begin_0033(&mut self) {
        let rows = std::mem::take(&mut self.rows_0033);
        self.set_group_param(&rows);
        self.rows_0033 = rows;
    }

    /// Clip 0033, `ChangeGroupParams` end: the asset defaults of the same rows.
    pub fn notify_group_params_end_0033(&mut self) {
        let rows = std::mem::take(&mut self.rows_0033);
        self.restore_group_param(&rows);
        self.rows_0033 = rows;
    }

    /// Clip 0033, `TresAnimNotifyState_Bonamik_CharaWind` begin: the character wind, scale 1.
    pub fn notify_chara_wind_begin_0033(&mut self) {
        if let Some(props) = self.wind_0033.clone() {
            self.set_chara_wind(&props);
        }
        self.wind_ticks = 0;
    }

    /// Clip 0033, `CharaWind` tick (every update the state is active, the begin's included):
    /// from the second tick the scale is the clip's `Wind` curve, constant 2.
    pub fn notify_chara_wind_tick_0033(&mut self) {
        self.wind_ticks = self.wind_ticks.saturating_add(1);
        if self.wind_ticks >= 2 {
            self.set_chara_wind_scale(2.0);
        }
    }

    /// Clip 0033, `CharaWind` end.
    pub fn notify_chara_wind_end_0033(&mut self) {
        self.clear_chara_wind();
        self.wind_ticks = 0;
    }

    /// Both of clip 0033's notify states begin (they share their start time).
    pub fn notify_begin_0033(&mut self) {
        self.notify_group_params_begin_0033();
        self.notify_chara_wind_begin_0033();
    }

    /// One update with clip 0033's notify states active (call it on the begin's update too).
    pub fn notify_tick_0033(&mut self) {
        self.notify_chara_wind_tick_0033();
    }

    /// Both of clip 0033's notify states end.
    pub fn notify_end_0033(&mut self) {
        self.notify_group_params_end_0033();
        self.notify_chara_wind_end_0033();
    }
}
