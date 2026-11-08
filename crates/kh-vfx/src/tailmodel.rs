//! (VFX-RING) `ParticleModuleTypeDataSQEX_TailModelMarch`: meshes marching along a tail, a W x H grid of them at every point
//! of the node's track (the keyblades' vw_so170_shf_blt00 TailPoly_lp10110, vw_so172_blc00 til, vw_so220_brs_blk00 chain).
//!
//! KH3, traced:
//! - CreateInstance 0x144383480: a 0xB50-byte SQEX mesh emitter instance (base ctor 0x144275620, vtable 0x14775A470) with a
//!   tail helper at +0x8D0 (ctor 0x144275F90, vtable 0x14775A380; helper +0x58 = the instance). The helper's grid:
//!   +0x234 = (int)ModelWidthNumbetr, +0x238 = (int)ModelHeightNumbetr, +0x23C = (float)(int)ModelWidthInterval,
//!   +0x240 = (float)(int)ModelHeightInterval (the intervals are TRUNCATED to whole cm). Property offsets from
//!   Z_Construct_UClass 0x144C059D0: ShapeType +0x1C0, WidthNumbetr +0x1C4, HeightNumbetr +0x1C8, WidthInterval +0x1CC,
//!   HeightInterval +0x1D0, bOnIntervalScale +0x1D4, ModelIntervalScale +0x1D8, ModelWidthIntervalScale +0x210,
//!   ModelHeightIntervalScale +0x248, bOnInterValScroll +0x280, ModelIntervalScrollScale +0x288, ModelWidthIntervalScrollScale
//!   +0x2C0, ModelHeightIntervalScrollScale +0x2F8, ImpactModelIntervalScrollScale +0x330, bOnNoiseMove +0x380, Intensity
//!   +0x388, PositionScale +0x3C0, TimeScale +0x3F8, CurlNoiseScale +0x430, CurlNoiseTimeScale +0x480. PostInitProperties
//!   0x144389B20: ShapeType QUAD_AIR (0; QUAD_GROUND 1, enum 0x144C25860), 3 x 2 models 50 cm apart, Intensity 300,
//!   PositionScale 0.01, TimeScale 1, CurlNoiseScale 1, CurlNoiseTimeScale 0, the three interval scales 1, the three scroll
//!   scales 0.
//! - Tick 0x14439D860: the base tick, the instance parameters `SQEX_TailModelMarch_Model{Width,Height}{Numbetr,Interval}`
//!   (component vt+0xAA8; a host sets none), the ParticleAround hook-up, then the helper's tick (helper vt+0x08).
//! - SPAWN (instance slot 0x260 = 0x14429F310 -> helper vt+0x10 = 0x14439DC10) replaces the rate and bursts: while the
//!   particles are fewer than W x H x TailNumber(t), and the track is longer than (particles / (W x H)) x the interval
//!   (FUN_144286050: the track's length), particles are force-spawned one by one (vt+0xD8, the spawn modules run) with
//!   their velocity, base velocity, RelativeTime and rotation zeroed (+0x20 / +0x30 / +0x0C / +0x1C / +0x38).
//! - THE MARCH (helper tick 0x1443A1530): along the recorded track (entries of 0x3C0 bytes, a matrix at +0x130 with the
//!   place at +0x160), from its first entry, a point every TailInterval(t) x |component scale| (0x144287CC0), at most
//!   TailNumber(t) (0x144287E40, an int) of them; at each the place is the linear blend between the two entries and the
//!   rotation the normalised blend of their quaternions (the second negated when the dot product is negative; FUN_142D91D70
//!   matrix -> quat, FUN_142D8DB60 quat -> rotator, then the rotator's matrix). Point k (fraction f = k / TailNumber) places
//!   its W x H particles (0x14438B2D0), then every particle past the placed points gets RelativeTime 1.1 (killed).
//! - ONE POINT (0x14438B2D0), its particles in order: width outer, height inner, the next particle index each:
//!   - IS = bOnIntervalScale ? ModelIntervalScale(f) : 1, WIS = ... ModelWidthIntervalScale(f), HIS = ... ModelHeightIntervalScale(f);
//!   - g = f + the helper's scroll position (+0x138), wrapped into [0, 1) unless 0 or 1; with bUseTailScroll (helper +0x134
//!     bit 0) the scroll's scale (vector, +0xB8), colour (+0xC0) and alpha (+0xC8) at g, else 1;
//!   - ISS = bOnInterValScroll ? ModelIntervalScrollScale(f) : 1, WISS = ... ModelWidthIntervalScrollScale(g),
//!     HISS = ... ModelHeightIntervalScrollScale(g); the impact scroll (instance parameter SqExIsImpactScroll) is a host's;
//!   - column w: (-(W - 1) / 2 x WI + w x WI) x ISS x IS x WIS x WISS along the point's Y axis; row h: (start + h x HI) x
//!     ISS x IS x HIS x HISS along its Z axis, start = -((H - 1) / 2) x HI for QUAD_AIR when HI > 1, else 0 (integer halves);
//!   - the particle (helper vt+0x78 = 0x144399A80): its rotation payload = the point's rotation (helper vt+0xA0 =
//!     0x1442939B0: the mesh's own rotation is turned by it), its place = the point + the offsets, its size x
//!     TailScale(f) (+0x90) x the scroll scale, its colour x TailColor(f) (+0x98) x the scroll colour, its alpha x
//!     TailAlpha(f) (+0xA0) x the scroll alpha. (Its size is 0 when its index is among the last +0xB34 particles: +0xB34 is
//!     set only through instance slot 0x3C8 = 0x1443993D0, which nothing of the effects calls: always 0.)
//!   - bOnNoiseMove (+0x380 bit 0) adds a curl-noise offset (FUN_1443B8F30 x3 scaled by CurlNoiseTimeScale x IS): no effect
//!     of the pack sets it (all three users: false), and the decompiler lost FUN_1443B8F30's arguments; it is parsed and
//!     reported, not evaluated.

use glam::{Mat3, Quat, Vec3};
use serde_json::Value;

use crate::def::TailDef;
use crate::dist::Dist;
use crate::rng::Rng;

/// The type data's grid and its scales.
#[derive(Clone, Debug, PartialEq)]
pub struct TailModelDef {
    pub width: u32,
    pub height: u32,
    /// Whole cm (CreateInstance truncates).
    pub width_interval: f32,
    pub height_interval: f32,
    /// `ESQEX_SHAPE_TYPE_QUAD_GROUND`: the rows start at the point instead of being centred on it.
    pub ground: bool,
    pub on_interval_scale: bool,
    pub interval_scale: Dist,
    pub width_interval_scale: Dist,
    pub height_interval_scale: Dist,
    pub on_interval_scroll: bool,
    pub interval_scroll: Dist,
    pub width_interval_scroll: Dist,
    pub height_interval_scroll: Dist,
    /// `bOnNoiseMove` (not evaluated: see the module doc).
    pub noise_move: bool,
}

impl TailModelDef {
    /// The type data's `params` when the emitter's type data is `sqex_tailmodelmarch`.
    pub fn parse(kind_name: &str, params: Option<&Value>) -> Option<TailModelDef> {
        if kind_name != "sqex_tailmodelmarch" {
            return None;
        }
        let null = Value::Null;
        let p = params.unwrap_or(&null);
        let f = |k: &str, d: f32| p.get(k).and_then(Value::as_f64).map_or(d, |v| v as f32);
        let b = |k: &str| p.get(k).and_then(Value::as_bool).unwrap_or(false);
        let dist = |k: &str, d: f32| p.get(k).and_then(Dist::parse).unwrap_or(Dist::constant(d));
        Some(TailModelDef {
            width: f("ModelWidthNumbetr", 3.0).max(0.0) as u32,
            height: f("ModelHeightNumbetr", 2.0).max(0.0) as u32,
            width_interval: (f("ModelWidthInterval", 50.0) as i32) as f32,
            height_interval: (f("ModelHeightInterval", 50.0) as i32) as f32,
            ground: p.get("ShapeType").and_then(Value::as_str) == Some("ESQEX_SHAPE_TYPE_QUAD_GROUND"),
            on_interval_scale: b("bOnIntervalScale"),
            interval_scale: dist("ModelIntervalScale", 1.0),
            width_interval_scale: dist("ModelWidthIntervalScale", 1.0),
            height_interval_scale: dist("ModelHeightIntervalScale", 1.0),
            on_interval_scroll: b("bOnInterValScroll"),
            interval_scroll: dist("ModelIntervalScrollScale", 0.0),
            width_interval_scroll: dist("ModelWidthIntervalScrollScale", 0.0),
            height_interval_scroll: dist("ModelHeightIntervalScrollScale", 0.0),
            noise_move: b("bOnNoiseMove"),
        })
    }

    /// Particles per point.
    pub fn per_point(&self) -> usize {
        (self.width * self.height) as usize
    }
}

/// The track's length (FUN_144286050).
pub fn track_length(path: &[Vec3]) -> f32 {
    path.windows(2).map(|w| w[0].distance(w[1])).sum()
}

/// 0x14439DC10: how many particles the helper force-spawns this tick, given `active` of them and the track's `length`;
/// `interval` is TailInterval(t) x |component scale|, `number` TailNumber(t).
pub fn to_spawn(tm: &TailModelDef, number: i32, interval: f32, active: usize, length: f32) -> usize {
    let per = tm.per_point();
    let target = per * number.max(0) as usize;
    if per == 0 || active >= target {
        return 0;
    }
    let mut n = active;
    loop {
        if n != 0 && length <= (n / per) as f32 * interval {
            return n - active;
        }
        // one point's worth, one by one up to the target
        n = (n + per).min(target);
        if n >= target {
            return n - active;
        }
    }
}

/// One placed particle: its place, the point's rotation, and what its size and colour are multiplied by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub position: Vec3,
    pub rotation: Mat3,
    pub scale: Vec3,
    pub color: [f32; 4],
}

/// The recorded track: places, newest last, and the node rotations aligned with them from the newest end.
#[derive(Clone, Copy, Debug)]
pub struct Track<'a> {
    pub path: &'a [Vec3],
    pub rots: &'a [Quat],
}

/// A point of the march: place, rotation.
fn points(path: &[Vec3], rots: &[Quat], interval: f32, number: usize) -> Vec<(Vec3, Quat)> {
    let mut out = Vec::new();
    if number == 0 || interval <= 0.0 || path.len() < 2 {
        return out;
    }
    // the track from its newest entry (the helper's first) backwards
    let entry = |i: usize| (path[path.len() - 1 - i], rots.get(rots.len().wrapping_sub(1 + i)).copied().unwrap_or(Quat::IDENTITY));
    let mut cum = 0.0f32;
    for i in 1..path.len() {
        let (a, qa) = entry(i - 1);
        let (b, qb) = entry(i);
        let seg = a.distance(b);
        cum += seg;
        while out.len() < number {
            let at = out.len() as f32 * interval;
            if at >= cum || seg <= 0.0 {
                break;
            }
            let f = 1.0 - (cum - at) / seg;
            let qb = if qa.dot(qb) < 0.0 { -qb } else { qb };
            let q = (qa * (1.0 - f) + qb * f).normalize();
            out.push((a.lerp(b, f), q));
        }
        if out.len() >= number {
            break;
        }
    }
    out
}

/// 0x1443A1530 + 0x14438B2D0: every particle's placement this tick, in particle order (`placements.len()` particles are
/// placed; the rest are killed). `t` the emitter time, `scroll` the tail scroll's position, `comp_scale` |component scale|.
pub fn placements(tm: &TailModelDef, tail: &TailDef, track: Track, t: f32, scroll: f32, comp_scale: f32, rng: &mut Rng) -> Vec<Placement> {
    let number = tail.number.float(t, rng) as i32;
    let interval = tail.interval.float(t, rng) * comp_scale.abs();
    let mut out = Vec::new();
    if number <= 0 {
        return out;
    }
    let n = number as f32;
    let one = |d: &Dist, on: bool, x: f32, rng: &mut Rng| if on { d.float(x, rng) } else { 1.0 };
    for (k, (position, q)) in points(track.path, track.rots, interval, number as usize).into_iter().enumerate() {
        let f = k as f32 / n;
        let mut g = f + scroll;
        if g != 0.0 && g != 1.0 {
            g = g.fract();
        }
        let is = one(&tm.interval_scale, tm.on_interval_scale, f, rng);
        let wis = one(&tm.width_interval_scale, tm.on_interval_scale, f, rng);
        let his = one(&tm.height_interval_scale, tm.on_interval_scale, f, rng);
        let (s_scale, s_color, s_alpha) = match &tail.scroll {
            Some(sc) => (sc.scale.as_ref().map_or(Vec3::ONE, |d| d.vec3(g, rng)), sc.color.as_ref().map_or(Vec3::ONE, |d| d.vec3(g, rng)), sc.alpha.as_ref().map_or(1.0, |d| d.float(g, rng))),
            None => (Vec3::ONE, Vec3::ONE, 1.0),
        };
        let iss = one(&tm.interval_scroll, tm.on_interval_scroll, f, rng);
        let wiss = one(&tm.width_interval_scroll, tm.on_interval_scroll, g, rng);
        let hiss = one(&tm.height_interval_scroll, tm.on_interval_scroll, g, rng);
        let spacing = iss * is;
        let scale = s_scale * tail.scale.float(f, rng);
        let c = s_color * tail.color.as_ref().map_or(Vec3::ONE, |d| d.vec3(f, rng));
        let a = s_alpha * tail.alpha.as_ref().map_or(1.0, |d| d.float(f, rng));
        let rotation = Mat3::from_quat(q);
        let half = |count: u32| if count > 1 { ((count as i32 - 1) / 2) as f32 } else { 0.0 };
        let width_start = -(half(tm.width) * tm.width_interval);
        let height_start = if !tm.ground && tm.height_interval > 1.0 { -(half(tm.height) * tm.height_interval) } else { 0.0 };
        for w in 0..tm.width {
            let across = (width_start + w as f32 * tm.width_interval) * spacing * wis * wiss;
            for h in 0..tm.height {
                let up = (height_start + h as f32 * tm.height_interval) * spacing * his * hiss;
                out.push(Placement { position: position + rotation.y_axis * across + rotation.z_axis * up, rotation, scale, color: [c.x, c.y, c.z, a] });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A TailShape as the effect JSON gives it (parsed by def.rs, so the test follows TailDef whatever its fields).
    fn tail_with(extra: Value) -> TailDef {
        let mut params = serde_json::json!({ "TailScale": 1.0 });
        if let (Some(p), Some(x)) = (params.as_object_mut(), extra.as_object()) {
            p.extend(x.clone());
        }
        let emitter = serde_json::json!({
            "name": "til", "enabled": true, "typedata": { "kind": "sqex_tailmodelmarch", "params": {} },
            "required": { "material": "/Game/m.m", "sqex_emitter_time": { "start_time": 0.0, "loop_time": 0.0, "loop_duration": 1.0, "duration_time": 1.0, "use_legacy_emitter_time": false } },
            "spawn": { "enabled": true, "rate": 0.0, "bursts": [] },
            "modules": [{ "class": "ParticleModuleSQEX_TailShape", "enabled": true, "params": params }]
        });
        let def = crate::EffectDef::from_json(&serde_json::json!({ "name": "t", "system": { "params": {} }, "emitters": [emitter] })).expect("the effect");
        def.emitters[0].tail.clone().expect("its tail")
    }

    fn tail(number: f32, interval: f32) -> TailDef {
        tail_with(serde_json::json!({ "TailNumber": number, "TailInterval": interval }))
    }

    fn grid(w: f32, h: f32, wi: f32, hi: f32) -> TailModelDef {
        let params = serde_json::json!({ "ModelWidthNumbetr": w, "ModelHeightNumbetr": h, "ModelWidthInterval": wi, "ModelHeightInterval": hi });
        TailModelDef::parse("sqex_tailmodelmarch", Some(&params)).expect("a march")
    }

    #[test]
    fn parse_takes_the_post_init_defaults_and_truncates_the_intervals() {
        let d = TailModelDef::parse("sqex_tailmodelmarch", None).expect("a march");
        assert_eq!((d.width, d.height, d.width_interval, d.height_interval, d.ground, d.on_interval_scale), (3, 2, 50.0, 50.0, false, false));
        assert_eq!(grid(10.0, 1.0, 6.5, 7.0).width_interval, 6.0, "vw_so172_blc00's 6.5 cm is 6 (CreateInstance's (int))");
        assert!(TailModelDef::parse("mesh", None).is_none());
    }

    #[test]
    fn spawn_fills_one_point_at_a_time_while_the_track_is_long_enough() {
        let tm = grid(2.0, 2.0, 10.0, 10.0);
        assert_eq!(to_spawn(&tm, 30, 24.0, 0, 0.0), 4, "the first point always");
        assert_eq!(to_spawn(&tm, 30, 24.0, 4, 10.0), 0, "10 cm of track: one point (0 x 24) only");
        assert_eq!(to_spawn(&tm, 30, 24.0, 4, 50.0), 8, "50 cm: points at 0, 24, 48");
        assert_eq!(to_spawn(&tm, 3, 24.0, 0, 1.0e4), 12, "never past W x H x TailNumber");
        assert_eq!(to_spawn(&tm, 3, 24.0, 12, 1.0e4), 0);
    }

    #[test]
    fn the_grid_sits_on_the_points_along_the_track_centred_on_its_y_and_z() {
        // a straight track along +X, newest at x = 0, turned so that Y is the world Y
        let path: Vec<Vec3> = (0..20).rev().map(|i| Vec3::new(i as f32 * 5.0, 0.0, 0.0)).collect();
        let rots = vec![Quat::IDENTITY; path.len()];
        let tm = grid(3.0, 2.0, 10.0, 20.0);
        let p = placements(&tm, &tail(4.0, 30.0), Track { path: &path, rots: &rots }, 0.0, 0.0, 1.0, &mut Rng::new(1));
        assert_eq!(p.len(), 4 * 6);
        // point 0 at the newest entry, the next one 30 cm back; width (outer) -10, 0, +10 on Y ((3 - 1) / 2 = 1 interval to
        // the left); height (inner) 0, +20 on Z ((2 - 1) / 2 = 0 in integers: an even count is not centred)
        let want: Vec<Vec3> = [-10.0, 0.0, 10.0].iter().flat_map(|y| [0.0, 20.0].map(|z| Vec3::new(0.0, *y, z))).collect();
        for (got, want) in p[..6].iter().zip(&want) {
            assert!(got.position.distance(*want) < 1.0e-3, "{:?} vs {want:?}", got.position);
        }
        assert!((p[6].position - Vec3::new(30.0, -10.0, 0.0)).length() < 1.0e-3, "{:?}", p[6].position);
        // three rows: centred on the point in the air, starting at it on the ground
        let air = placements(&grid(1.0, 3.0, 0.0, 20.0), &tail(1.0, 30.0), Track { path: &path, rots: &rots }, 0.0, 0.0, 1.0, &mut Rng::new(1));
        assert_eq!(air.iter().map(|x| x.position.z).collect::<Vec<_>>(), vec![-20.0, 0.0, 20.0]);
        let mut ground = grid(1.0, 3.0, 0.0, 20.0);
        ground.ground = true;
        let g = placements(&ground, &tail(1.0, 30.0), Track { path: &path, rots: &rots }, 0.0, 0.0, 1.0, &mut Rng::new(1));
        assert_eq!(g.iter().map(|x| x.position.z).collect::<Vec<_>>(), vec![0.0, 20.0, 40.0]);
    }

    #[test]
    fn interval_scales_shape_the_grid_along_the_tail_and_the_tail_curves_scale_and_tint() {
        let path: Vec<Vec3> = (0..40).rev().map(|i| Vec3::new(i as f32 * 5.0, 0.0, 0.0)).collect();
        let rots = vec![Quat::IDENTITY; path.len()];
        let mut tm = grid(2.0, 1.0, 10.0, 10.0);
        tm.on_interval_scale = true;
        tm.width_interval_scale = Dist::constant(0.5);
        let tl = tail_with(serde_json::json!({ "TailNumber": 2.0, "TailInterval": 50.0, "TailScale": 3.0, "TailColor": { "x": 1.0, "y": 0.5, "z": 0.25 }, "TailAlpha": 0.5 }));
        let p = placements(&tm, &tl, Track { path: &path, rots: &rots }, 0.0, 0.0, 1.0, &mut Rng::new(1));
        // W 2: (W - 1) / 2 = 0 in integers, so the columns are at 0 and 10 x 0.5
        assert_eq!(p[0].position, Vec3::ZERO);
        assert!((p[1].position.y - 5.0).abs() < 1.0e-4);
        assert_eq!((p[0].scale, p[0].color), (Vec3::splat(3.0), [1.0, 0.5, 0.25, 0.5]));
        // the component scale stretches the spacing of the points
        let q = placements(&tm, &tl, Track { path: &path, rots: &rots }, 0.0, 0.0, 2.0, &mut Rng::new(1));
        assert!((q[2].position.x - 100.0).abs() < 1.0e-3, "{:?}", q[2].position);
    }

    #[test]
    fn the_rotation_follows_the_track_between_its_entries() {
        let path = vec![Vec3::new(100.0, 0.0, 0.0), Vec3::ZERO];
        let rots = vec![Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), Quat::IDENTITY];
        let tm = grid(1.0, 1.0, 0.0, 0.0);
        let p = placements(&tm, &tail(2.0, 50.0), Track { path: &path, rots: &rots }, 0.0, 0.0, 1.0, &mut Rng::new(1));
        assert_eq!(p.len(), 2);
        assert!((p[0].rotation.x_axis - Vec3::X).length() < 1.0e-4, "the newest entry's rotation");
        let half = Mat3::from_quat(Quat::from_rotation_z(std::f32::consts::FRAC_PI_4));
        assert!((p[1].rotation.x_axis - half.x_axis).length() < 1.0e-3, "halfway: {:?}", p[1].rotation);
        assert!((p[1].position - Vec3::new(50.0, 0.0, 0.0)).length() < 1.0e-3);
    }
}
