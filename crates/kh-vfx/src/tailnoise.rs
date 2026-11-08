//! (VFX-RING) `ParticleModuleSQEX_TailNoise` as KH3 runs it: random bumps along a tail that thin its points out (size) and /
//! or fade them (alpha), harder as `NoiseIntensity` rises over the emitter time.
//!
//! The module has no Spawn / Update of its own (vtable 0x14775ED78: slots 0x240 / 0x248 = the base's 0x14041F120). The tail
//! helper (vtable 0x147710E90, embedded in the emitter instance at +0x868 TailShape / +0x8D0 TailShapeTypeMesh / +0xA10
//! TailPolygon) does the work:
//! - **The segments** (helper slot 4 = 0x14428A380, through the emitter's vt+0x2B0 = 0x14428A880 / 0x14433CD50): made at the
//!   emitter's Init (0x14428A330 / 0x14433CD00: Init then vt+0x2B0) and again at every loop when `bLoopResetNoiseData`
//!   (0x144383E50, the module's looping notification). FUN_144274630 finds the LOD's first ENABLED TailNoise module.
//!   - `ShareNoiseDataEmitterName` set and an emitter of that name in the component: its segments are copied and ITS
//!     module's NoiseIntensity is used (helper +0x168 = that module + 0x68).
//!   - Otherwise, from cursor c = 0 while (c of the next) < max(TailNumber) (GetOutRange 0x1440C69B0: the distribution's
//!     MaxValue): peak = c + NoiseMin + floor((NoiseMax - NoiseMin + 1) x u), end = 2 x peak - c; when max(TailNumber) <= peak,
//!     peak = (int) max and end = (int)(max + 1); intensity = uniform[NoiseIntensityRandomMin, NoiseIntensityRandomMax];
//!     flags = bUseNoiseTypeSize (bit 0) | bUseNoiseTypeAlpha (bit 1), MinSize; the next segment starts at end + 1.
//!     Entry (0x18 bytes at helper +0x158): peak, start, end, intensity, flags, MinSize.
//! - **The consumer** 0x14428D380, called by 0x144296090 (helper slot 17, the tail point's place, size x TailScale, colour x
//!   TailColor, alpha x TailAlpha) for every tail point k the helper update 0x1442A7650 visits, after it reset the segment
//!   cursor (+0x30 = 0, 0x1442A7650 line 191). For the cursor's segment (peak P, start S, end E, intensity w), with
//!   I = NoiseIntensity(emitter time):
//!   - k == P: d = 1, span = P - S;  S <= k < P: d = P - k, span = P - S;  P < k <= E: d = k - P, span = E - P;
//!     otherwise (or d == 0) d = 1e-5; span 0 -> 1e-5;
//!   - v = clamp(1 - d / span x w x I, 0, 1); size bit: the point's Size x max(MinSize, v); alpha bit: its alpha x v;
//!   - k >= E: the cursor moves to the next segment; past the last segment nothing changes.

use serde_json::Value;

use crate::dist::Dist;
use crate::rng::Rng;

/// The module's properties (constructor 0x14437DE40 defaults: NoiseMin 2, NoiseMax 10, NoiseIntensityRandomMin 0.1,
/// NoiseIntensityRandomMax 5, bUseNoiseTypeSize off, bUseNoiseTypeAlpha on; PostInit 0x14438A790 gives NoiseIntensity a
/// constant 0 when it has none).
#[derive(Clone, Debug, PartialEq)]
pub struct TailNoiseDef {
    pub noise_min: i32,
    pub noise_max: i32,
    pub random_min: f32,
    pub random_max: f32,
    pub size: bool,
    pub alpha: bool,
    pub min_size: f32,
    pub intensity: Dist,
    pub loop_reset: bool,
    /// `ShareNoiseDataEmitterName` (None for "None" / empty).
    pub share: Option<String>,
}

/// One bump (the helper's 0x18-byte entry).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoiseSeg {
    pub peak: i32,
    pub start: i32,
    pub end: i32,
    pub intensity: f32,
    pub size: bool,
    pub alpha: bool,
    pub min_size: f32,
}

impl TailNoiseDef {
    pub fn parse(p: &Value) -> TailNoiseDef {
        let num = |k: &str, d: f64| p.get(k).and_then(Value::as_f64).unwrap_or(d);
        let flag = |k: &str, d: bool| p.get(k).and_then(Value::as_bool).unwrap_or(d);
        let share = p.get("ShareNoiseDataEmitterName").and_then(Value::as_str).filter(|s| !s.is_empty() && *s != "None").map(str::to_string);
        TailNoiseDef {
            noise_min: num("NoiseMin", 2.0) as i32,
            noise_max: num("NoiseMax", 10.0) as i32,
            random_min: num("NoiseIntensityRandomMin", 0.1) as f32,
            random_max: num("NoiseIntensityRandomMax", 5.0) as f32,
            size: flag("bUseNoiseTypeSize", false),
            alpha: flag("bUseNoiseTypeAlpha", true),
            min_size: num("MinSize", 0.0) as f32,
            intensity: p.get("NoiseIntensity").and_then(Dist::parse).unwrap_or_else(|| Dist::constant(0.0)),
            loop_reset: flag("bLoopResetNoiseData", false),
            share,
        }
    }

    /// 0x14428A380, own data: the bumps over a tail of at most `tail_max` points (max of TailNumber).
    pub fn generate(&self, tail_max: f32, rng: &mut Rng) -> Vec<NoiseSeg> {
        let mut out = Vec::new();
        // 0x14428A380 makes none without a tail or with NoiseMax 0 (module +0xA4)
        if tail_max <= 0.0 || self.noise_max == 0 {
            return out;
        }
        let mut cursor = 0i32;
        loop {
            let u = rng.f32();
            let mut peak = self.noise_min + (((self.noise_max - self.noise_min) as f32 + 1.0) * u) as i32 + cursor;
            let mut end = peak * 2 - cursor;
            if tail_max <= peak as f32 {
                peak = tail_max as i32;
                end = (tail_max + 1.0) as i32;
            }
            let intensity = (self.random_max - self.random_min) * rng.f32() + self.random_min;
            out.push(NoiseSeg { peak, start: cursor, end, intensity, size: self.size, alpha: self.alpha, min_size: self.min_size });
            cursor = end + 1;
            // (a guard of ours: KH3's loop ends as the cursor passes the tail; a NoiseMin below 0 could stall it)
            if (cursor as f32) >= tail_max || out.len() >= 4096 {
                break;
            }
        }
        out
    }
}

/// 0x14428D380 over the points 0..n the helper visits (in order, the cursor reset first): each point's (size, alpha) factor.
pub fn factors(segs: &[NoiseSeg], intensity: f32, n: usize) -> Vec<(f32, f32)> {
    let mut out = vec![(1.0, 1.0); n];
    let mut cursor = 0usize;
    for (k, f) in out.iter_mut().enumerate() {
        let Some(s) = segs.get(cursor) else { break };
        let k = k as i32;
        let (mut d, mut span) = (0.0f32, 0.0f32);
        let inside = if k == s.peak {
            d = 1.0;
            span = (s.peak - s.start) as f32;
            true
        } else if s.start <= k && k < s.peak {
            d = (s.peak - k) as f32;
            span = (s.peak - s.start) as f32;
            true
        } else if s.peak < k && k <= s.end {
            d = (k - s.peak) as f32;
            span = (s.end - s.peak) as f32;
            true
        } else {
            false
        };
        if !inside || d == 0.0 {
            d = 1.0e-5;
        }
        if span == 0.0 {
            span = 1.0e-5;
        }
        let v = (1.0 - d / span * s.intensity * intensity).clamp(0.0, 1.0);
        if s.size {
            f.0 = s.min_size.max(v);
        }
        if s.alpha {
            f.1 = v;
        }
        if s.end <= k {
            cursor += 1;
        }
    }
    out
}

/// The draw's (f, scale, colour) of one tail point with its noise factor applied (size x, alpha x).
pub fn apply(point: (f32, f32, [f32; 4]), factor: Option<&(f32, f32)>) -> (f32, f32, [f32; 4]) {
    match factor {
        Some(&(s, a)) => (point.0, point.1 * s, [point.2[0], point.2[1], point.2[2], point.2[3] * a]),
        None => point,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn def(size: bool, alpha: bool) -> TailNoiseDef {
        TailNoiseDef { noise_min: 1, noise_max: 2, random_min: 1.0, random_max: 3.0, size, alpha, min_size: 0.25, intensity: Dist::constant(1.0), loop_reset: false, share: None }
    }

    #[test]
    fn segments_tile_the_tail_from_its_head() {
        let d = def(true, true);
        let segs = d.generate(20.0, &mut Rng::new(7));
        assert!(!segs.is_empty());
        assert_eq!(segs[0].start, 0);
        for w in segs.windows(2) {
            assert_eq!(w[1].start, w[0].end + 1, "contiguous: {segs:?}");
        }
        for s in &segs {
            let up = s.peak - s.start;
            assert!((1..=2).contains(&up) || s.peak == 20, "peak NoiseMin..NoiseMax after the start: {s:?}");
            assert!(s.peak == 20 || s.end - s.peak == up, "symmetric unless clamped: {s:?}");
            assert!((1.0..3.0).contains(&s.intensity));
        }
        let last = segs.last().copied().unwrap_or(segs[0]);
        assert!(last.end + 1 >= 20, "covers the tail: {last:?}");
    }

    #[test]
    fn a_segment_past_the_tail_is_clamped_to_it() {
        let d = TailNoiseDef { noise_min: 30, noise_max: 30, ..def(false, true) };
        let segs = d.generate(20.0, &mut Rng::new(1));
        assert_eq!(segs.len(), 1);
        assert_eq!((segs[0].start, segs[0].peak, segs[0].end), (0, 20, 21));
    }

    #[test]
    fn the_peak_keeps_most_and_the_ends_lose_most() {
        // one bump 0..4..8, w 1, I 0.5
        let s = NoiseSeg { peak: 4, start: 0, end: 8, intensity: 1.0, size: true, alpha: true, min_size: 0.6 };
        let f = factors(&[s], 0.5, 12);
        assert!((f[4].1 - (1.0 - 0.25 * 0.5)).abs() < 1.0e-6, "peak: 1 - 1/4 x 0.5: {f:?}");
        assert!((f[0].1 - 0.5).abs() < 1.0e-6, "start: 1 - 4/4 x 0.5");
        assert!((f[2].1 - 0.75).abs() < 1.0e-6);
        assert!((f[8].1 - 0.5).abs() < 1.0e-6, "end");
        assert_eq!(f[0].0, 0.6, "size floored at MinSize");
        assert_eq!(f[9], (1.0, 1.0), "past the last segment: untouched");
        // intensity 0 (the curve before it rises): nothing changes
        assert!(factors(&[s], 0.0, 12).iter().all(|x| *x == (1.0, 1.0)));
        // strong: clamped at 0
        assert!(factors(&[s], 10.0, 12)[0].1 == 0.0);
    }

    #[test]
    fn flags_pick_size_or_alpha() {
        let s = NoiseSeg { peak: 2, start: 0, end: 4, intensity: 1.0, size: false, alpha: true, min_size: 0.0 };
        let f = factors(&[s], 1.0, 5);
        assert!(f.iter().all(|x| x.0 == 1.0) && f[0].1 == 0.0);
        let s = NoiseSeg { size: true, alpha: false, ..s };
        let f = factors(&[s], 1.0, 5);
        assert!(f.iter().all(|x| x.1 == 1.0) && f[0].0 == 0.0);
    }

    #[test]
    fn parse_reads_the_module_with_the_constructor_defaults() {
        let p = json!({ "NoiseMin": 1, "NoiseMax": 2, "NoiseIntensityRandomMin": 1, "NoiseIntensityRandomMax": 3, "bUseNoiseTypeSize": true, "bUseNoiseTypeAlpha": true, "MinSize": 0, "bLoopResetNoiseData": false, "ShareNoiseDataEmitterName": "TailPoly_lp0" });
        let d = TailNoiseDef::parse(&p);
        assert_eq!((d.noise_min, d.noise_max, d.size, d.alpha, d.share.as_deref()), (1, 2, true, true, Some("TailPoly_lp0")));
        let d = TailNoiseDef::parse(&json!({ "ShareNoiseDataEmitterName": "None" }));
        assert_eq!((d.noise_min, d.noise_max, d.random_min, d.random_max, d.size, d.alpha, d.share), (2, 10, 0.1, 5.0, false, true, None));
        assert_eq!(d.intensity.float(0.5, &mut Rng::new(1)), 0.0, "PostInit: NoiseIntensity constant 0");
    }
}
