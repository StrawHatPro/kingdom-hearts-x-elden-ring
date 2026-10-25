//! Distributions: the baked lookup table of `FRawDistribution`, which is what the shipped game
//! samples (docs/sora-vfx-spec.md 8.1; stock UE 4.17 `FRawDistribution::GetValue*` [U]).
//!
//! `x = max(t - TimeBias, 0) * TimeScale`; the value is linear between entries `floor(x)` and
//! `floor(x) + 1` (clamped to the last). Op 1 holds one set of values; op 2 (random) holds the
//! minimum set then the maximum set and the result is `min + (max - min) * r`, one `r` per
//! axis unless the lock flag ties axes together; op 3 (extremes) picks minimum or maximum.

use glam::Vec3;
use serde_json::Value;

use crate::rng::Rng;

#[derive(Clone, Debug, PartialEq)]
pub struct Dist {
    /// 1 none, 2 random, 3 extremes.
    op: u8,
    entries: usize,
    /// Floats per entry (both sets for op 2 / 3).
    stride: usize,
    /// Offset of the maximum set inside an entry.
    sub: usize,
    time_scale: f32,
    time_bias: f32,
    /// `EDistributionVectorLockFlags`: 0 none, 1 XY, 2 XZ, 3 YZ, 4 XYZ.
    lock: u8,
    values: Vec<f32>,
}

impl Dist {
    pub fn constant(v: f32) -> Dist {
        Dist { op: 1, entries: 1, stride: 1, sub: 0, time_scale: 0.0, time_bias: 0.0, lock: 0, values: vec![v] }
    }

    pub fn constant_vec(v: Vec3) -> Dist {
        Dist { op: 1, entries: 1, stride: 3, sub: 0, time_scale: 0.0, time_bias: 0.0, lock: 0, values: vec![v.x, v.y, v.z] }
    }

    pub fn uniform(min: f32, max: f32) -> Dist {
        Dist { op: 2, entries: 1, stride: 2, sub: 1, time_scale: 0.0, time_bias: 0.0, lock: 0, values: vec![min, max] }
    }

    pub fn uniform_vec(min: Vec3, max: Vec3) -> Dist {
        Dist { op: 2, entries: 1, stride: 6, sub: 3, time_scale: 0.0, time_bias: 0.0, lock: 0, values: vec![min.x, min.y, min.z, max.x, max.y, max.z] }
    }

    /// A linear ramp over `t` in [0, 1] (tests).
    pub fn ramp(from: f32, to: f32) -> Dist {
        Dist { op: 1, entries: 2, stride: 1, sub: 0, time_scale: 1.0, time_bias: 0.0, lock: 0, values: vec![from, to] }
    }

    /// From a module property of the effect JSON: `{ "distribution": { ... } }`, the
    /// distribution object itself, or a plain number / `{x, y, z}`. `None` when the property
    /// holds no distribution (an unset raw distribution, null).
    pub fn parse(v: &Value) -> Option<Dist> {
        if let Some(n) = v.as_f64() {
            return Some(Dist::constant(n as f32));
        }
        let d = match v.get("distribution") {
            Some(d) => d,
            None if v.get("type").is_some() || v.get("lookup_table").is_some() => v,
            None => {
                if let (Some(x), Some(y), Some(z)) = (v.get("x"), v.get("y"), v.get("z")) {
                    return Some(Dist::constant_vec(Vec3::new(x.as_f64()? as f32, y.as_f64()? as f32, z.as_f64()? as f32)));
                }
                return None;
            }
        };
        if d.is_null() {
            return None;
        }
        if let Some(table) = d.get("lookup_table") {
            let values: Vec<f32> = table.get("values").and_then(Value::as_array).map(|a| a.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect()).unwrap_or_default();
            let entries = table.get("entry_count").and_then(Value::as_u64).unwrap_or(0) as usize;
            let stride = table.get("entry_stride").and_then(Value::as_u64).unwrap_or(0) as usize;
            let op = table.get("op").and_then(Value::as_u64).unwrap_or(0) as u8;
            if op != 0 && entries > 0 && stride > 0 && values.len() >= entries * stride {
                let mut sub = table.get("sub_entry_stride").and_then(Value::as_u64).unwrap_or(0) as usize;
                if op != 1 && (sub == 0 || sub >= stride) {
                    sub = stride / 2;
                }
                return Some(Dist {
                    op: if op == 1 || stride < 2 { 1 } else { op },
                    entries,
                    stride,
                    sub,
                    time_scale: table.get("time_scale").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                    time_bias: table.get("time_bias").and_then(Value::as_f64).unwrap_or(0.0) as f32,
                    lock: table.get("lock_flag").and_then(Value::as_u64).unwrap_or(0) as u8,
                    values,
                });
            }
        }
        // (ZETTA) A Square Enix UNIFORM distribution whose table was never baked (`bCanBeBaked` false: op RDO_Uninitialized,
        // the pack exports its properties verbatim): uniform between its `Min` and `Max` (floats, or {x, y, z}). Before,
        // such a property read as absent and its module took the default (a StartAlpha of 0.1 .. 0.5 drew at 1: the Zettaflare
        // circle's dust 2-10x too dense). The Gauss classes are left as they were (their spread is not read here).
        // (VFXR) the Square Enix GAUSS classes the same way (op 4: a normal draw about the middle of Min .. Max, sigma =
        // (Max - Min) / 6, clamped to the range [A]); `LockedAxes` ties the axes of a vector (both classes).
        let se_class = d.get("class").and_then(Value::as_str).unwrap_or("");
        if se_class.starts_with("SQEXDistribution") && (se_class.ends_with("Uniform") || se_class.ends_with("Gauss")) {
            let gauss = se_class.ends_with("Gauss");
            let locked = |dist: Dist| -> Dist {
                let lock = match d.get("properties").and_then(|p| p.get("LockedAxes")).and_then(Value::as_str).unwrap_or("") {
                    "EDVLF_XY" => 1,
                    "EDVLF_XZ" => 2,
                    "EDVLF_YZ" => 3,
                    "EDVLF_XYZ" => 4,
                    _ => 0,
                };
                Dist { lock, op: if gauss { 4 } else { dist.op }, ..dist }
            };
            if let Some(p) = d.get("properties") {
                let vec = |v: &Value| -> Option<Vec3> { Some(Vec3::new(v.get("x")?.as_f64()? as f32, v.get("y")?.as_f64()? as f32, v.get("z")?.as_f64()? as f32)) };
                match (p.get("Min"), p.get("Max")) {
                    (Some(a), Some(b)) if a.is_number() && b.is_number() => return Some(locked(Dist::uniform(a.as_f64()? as f32, b.as_f64()? as f32))),
                    (Some(a), Some(b)) => {
                        if let (Some(a), Some(b)) = (vec(a), vec(b)) {
                            return Some(locked(Dist::uniform_vec(a, b)));
                        }
                    }
                    _ => {}
                }
            }
        }
        if d.get("class").and_then(Value::as_str).is_some_and(|c| c.starts_with("SQEXDistribution") && c.ends_with("Uniform")) {
            if let Some(p) = d.get("properties") {
                let vec = |v: &Value| -> Option<Vec3> { Some(Vec3::new(v.get("x")?.as_f64()? as f32, v.get("y")?.as_f64()? as f32, v.get("z")?.as_f64()? as f32)) };
                match (p.get("Min"), p.get("Max")) {
                    (Some(a), Some(b)) if a.is_number() && b.is_number() => return Some(Dist::uniform(a.as_f64()? as f32, b.as_f64()? as f32)),
                    (Some(a), Some(b)) => {
                        if let (Some(a), Some(b)) = (vec(a), vec(b)) {
                            return Some(Dist::uniform_vec(a, b));
                        }
                    }
                    _ => {}
                }
            }
        }
        // No usable table (the Square Enix distribution classes): constant or uniform bounds.
        let nums = |v: &Value| -> Option<Vec<f32>> {
            match v {
                Value::Number(n) => Some(vec![n.as_f64()? as f32]),
                Value::Array(a) => Some(a.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect()),
                _ => None,
            }
        };
        if let Some(c) = d.get("constant").and_then(nums) {
            return Some(if c.len() >= 3 { Dist::constant_vec(Vec3::new(c[0], c[1], c[2])) } else { Dist::constant(*c.first()?) });
        }
        if let (Some(min), Some(max)) = (d.get("min").and_then(nums), d.get("max").and_then(nums)) {
            return Some(if min.len() >= 3 && max.len() >= 3 { Dist::uniform_vec(Vec3::new(min[0], min[1], min[2]), Vec3::new(max[0], max[1], max[2])) } else { Dist::uniform(*min.first()?, *max.first()?) });
        }
        None
    }

    /// (VFXR) The random fraction between the two sets: uniform, or (op 4, gauss) a normal draw about the middle with
    /// sigma = 1/6 of the range, clamped (Box-Muller on two uniforms).
    fn draw(&self, rng: &mut Rng) -> f32 {
        match self.op {
            1 => 0.0,
            4 => {
                let (u1, u2) = (rng.f32().max(1.0e-7), rng.f32());
                let n = (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos();
                (0.5 + n / 6.0).clamp(0.0, 1.0)
            }
            _ => rng.f32(),
        }
    }

    /// True when sampling draws random numbers (uniform distributions).
    pub fn is_random(&self) -> bool {
        self.op != 1
    }

    /// How many components a value has (1 or 3).
    pub fn dims(&self) -> usize {
        if self.op == 1 {
            self.stride
        } else {
            self.sub
        }
    }

    fn entry(&self, t: f32) -> (usize, usize, f32) {
        let x = ((t - self.time_bias).max(0.0) * self.time_scale).max(0.0);
        let last = self.entries - 1;
        let i = (x as usize).min(last);
        let j = (i + 1).min(last);
        (i * self.stride, j * self.stride, if i == last { 0.0 } else { x - i as f32 })
    }

    fn component(&self, a: usize, b: usize, alpha: f32, k: usize, r: f32) -> f32 {
        let at = |base: usize, k: usize| self.values.get(base + k).copied().unwrap_or(0.0);
        let lo = at(a, k) + (at(b, k) - at(a, k)) * alpha;
        match self.op {
            1 => lo,
            _ => {
                let hi = at(a, self.sub + k) + (at(b, self.sub + k) - at(a, self.sub + k)) * alpha;
                if self.op == 3 {
                    if r > 0.5 {
                        hi
                    } else {
                        lo
                    }
                } else {
                    lo + (hi - lo) * r
                }
            }
        }
    }

    /// A float at time `t` (seconds of emitter time, or the particle's relative time).
    pub fn float(&self, t: f32, rng: &mut Rng) -> f32 {
        let (a, b, alpha) = self.entry(t);
        let r = self.draw(rng);
        self.component(a, b, alpha, 0, r)
    }

    /// A vector at time `t`. A float distribution gives the same value on the three axes.
    pub fn vec3(&self, t: f32, rng: &mut Rng) -> Vec3 {
        let (a, b, alpha) = self.entry(t);
        if self.dims() < 3 {
            let r = self.draw(rng);
            return Vec3::splat(self.component(a, b, alpha, 0, r));
        }
        let mut r = [0.0f32; 3];
        if self.op != 1 {
            r = [self.draw(rng), self.draw(rng), self.draw(rng)];
            match self.lock {
                1 => r[1] = r[0],
                2 => r[2] = r[0],
                3 => r[2] = r[1],
                4 => {
                    r[1] = r[0];
                    r[2] = r[0];
                }
                _ => {}
            }
        }
        Vec3::new(self.component(a, b, alpha, 0, r[0]), self.component(a, b, alpha, 1, r[1]), self.component(a, b, alpha, 2, r[2]))
    }

    /// The largest value of component 0 over the table (bounds for hosts and tests).
    pub fn max_abs(&self) -> f32 {
        self.values.iter().fold(0.0f32, |m, v| m.max(v.abs()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn table_is_sampled_linearly_and_clamped() {
        let d = Dist::parse(&json!({"distribution": {"type": "constant_curve", "lookup_table": {"op": 1, "entry_count": 3, "entry_stride": 1, "sub_entry_stride": 0, "time_scale": 2.0, "time_bias": 0.0, "lock_flag": 0, "values": [0.0, 10.0, 4.0]}}})).unwrap();
        let mut rng = Rng::new(1);
        assert_eq!(d.float(0.0, &mut rng), 0.0);
        assert!((d.float(0.25, &mut rng) - 5.0).abs() < 1e-5);
        assert!((d.float(0.75, &mut rng) - 7.0).abs() < 1e-5);
        assert_eq!(d.float(5.0, &mut rng), 4.0);
        assert_eq!(d.float(-1.0, &mut rng), 0.0);
        assert!(!d.is_random());
    }

    #[test]
    fn uniform_vector_respects_bounds_and_locks() {
        let d = Dist::parse(&json!({"distribution": {"type": "uniform", "lookup_table": {"op": 2, "entry_count": 1, "entry_stride": 6, "sub_entry_stride": 3, "time_scale": 0.0, "time_bias": 0.0, "lock_flag": 4, "values": [-1.0, 0.0, 10.0, 1.0, 2.0, 20.0]}}})).unwrap();
        let mut rng = Rng::new(7);
        for _ in 0..200 {
            let v = d.vec3(0.3, &mut rng);
            assert!((-1.0..=1.0).contains(&v.x) && (0.0..=2.0).contains(&v.y) && (10.0..=20.0).contains(&v.z));
            // XYZ lock: one random number for the three axes.
            let r = (v.x + 1.0) / 2.0;
            assert!((v.y / 2.0 - r).abs() < 1e-4 && ((v.z - 10.0) / 10.0 - r).abs() < 1e-4);
        }
        assert!(d.is_random());
    }

    #[test]
    fn fallbacks_without_a_table() {
        let mut rng = Rng::new(1);
        assert_eq!(Dist::parse(&json!({"distribution": {"type": "constant", "constant": [1.0, 2.0, 3.0]}})).unwrap().vec3(0.0, &mut rng), Vec3::new(1.0, 2.0, 3.0));
        let u = Dist::parse(&json!({"distribution": {"type": "uniform", "min": 2.0, "max": 4.0}})).unwrap();
        let v = u.float(0.0, &mut rng);
        assert!((2.0..=4.0).contains(&v));
        // An unset raw distribution of a Square Enix module.
        assert!(Dist::parse(&json!({"Distribution": null, "MaxValue": 0, "Table": {"Op": 0}})).is_none());
        assert!(Dist::parse(&json!({"distribution": null})).is_none());
        assert_eq!(Dist::parse(&json!(2.5)).unwrap().float(0.0, &mut rng), 2.5);
    }

    /// (ZETTA) an unbaked Square Enix uniform (the Zettaflare circle's dust StartAlpha 0.1 .. 0.5) is read from its properties
    #[test]
    fn unbaked_sqex_uniform_reads_its_min_and_max() {
        let mut rng = Rng::new(7);
        let f = json!({"distribution": {"class": "SQEXDistributionFloatUniform", "properties": {"Min": 0.1, "Max": 0.5, "bCanBeBaked": false}, "lookup_table": {"op": 0, "entry_count": 0, "entry_stride": 0, "values": []}}});
        let d = Dist::parse(&f).expect("parsed");
        for _ in 0..50 {
            let v = d.float(0.0, &mut rng);
            assert!((0.1..=0.5).contains(&v), "{v}");
        }
        let v = json!({"distribution": {"class": "SQEXDistributionVectorUniform", "properties": {"Min": {"x": 1.0, "y": 2.0, "z": 3.0}, "Max": {"x": 1.0, "y": 2.0, "z": 3.0}}, "lookup_table": {"op": 0}}});
        assert_eq!(Dist::parse(&v).expect("parsed").vec3(0.0, &mut rng), glam::Vec3::new(1.0, 2.0, 3.0));
        // (VFXR) a Gauss class is read now (op 4: a normal draw inside Min .. Max; vfxr_gauss_tests)
        assert!(Dist::parse(&json!({"distribution": {"class": "SQEXDistributionFloatGauss", "properties": {"Min": 0.1, "Max": 0.5}, "lookup_table": {"op": 0}}})).is_some());
    }
}

#[cfg(test)]
mod vfxr_gauss_tests {
    use super::*;

    /// (VFXR) a Square Enix gauss distribution samples inside Min .. Max, centred, and a locked vector moves as one
    #[test]
    fn square_enix_gauss_classes_are_read() {
        let v: Value = serde_json::from_str(r#"{"distribution":{"class":"SQEXDistributionFloatGauss","lookup_table":{"op":0,"entry_count":0,"values":[]},"properties":{"Min":0.5,"Max":1.3,"bCanBeBaked":false}}}"#).unwrap();
        let d = Dist::parse(&v).expect("a gauss distribution");
        let mut rng = Rng::new(3);
        let samples: Vec<f32> = (0..4000).map(|_| d.float(0.0, &mut rng)).collect();
        assert!(samples.iter().all(|x| (0.5..=1.3).contains(x)));
        let mean = samples.iter().sum::<f32>() / samples.len() as f32;
        assert!((mean - 0.9).abs() < 0.02, "mean {mean}");
        // sigma about (1.3 - 0.5) / 6
        let var = samples.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / samples.len() as f32;
        assert!((var.sqrt() - 0.8 / 6.0).abs() < 0.02, "sigma {}", var.sqrt());
        let v: Value = serde_json::from_str(r#"{"distribution":{"class":"SQEXDistributionVectorGauss","lookup_table":{"op":0,"entry_count":0,"values":[]},"properties":{"Max":{"x":0.4,"y":0.4,"z":0.4},"Min":{"x":0.15,"y":0.15,"z":0.15},"LockedAxes":"EDVLF_XYZ"}}}"#).unwrap();
        let d = Dist::parse(&v).expect("a vector gauss");
        for _ in 0..50 {
            let s = d.vec3(0.0, &mut rng);
            assert!(s.x == s.y && s.y == s.z && (0.15..=0.4).contains(&s.x));
        }
    }
}
