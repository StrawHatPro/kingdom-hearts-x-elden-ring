//! `tools/lib/bonamik/m.js`: the shared float32 helpers. Every scalar operation is one `f32`
//! operation (the JS rounds each with `Math.fround`), in the order of the JS. Where the JS
//! computes in double before rounding (`1 / Math.sqrt(x)`), so does this.

pub type V3 = [f32; 3];
pub type Q4 = [f32; 4];

/// A bone transform as the JS has it: `{q, t, s}`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Xf {
    pub q: Q4,
    pub t: V3,
    pub s: V3,
}

impl Xf {
    pub const IDENT: Xf = Xf { q: [0.0, 0.0, 0.0, 1.0], t: [0.0, 0.0, 0.0], s: [1.0, 1.0, 1.0] };
}

/// `Math.fround` of a double.
#[inline]
pub fn f(x: f64) -> f32 {
    x as f32
}

#[inline]
pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[inline]
pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
pub fn mul(a: V3, s: f32) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

/// `((x*x) + (y*y)) + (z*z)`.
#[inline]
pub fn dot(a: V3, b: V3) -> f32 {
    (a[0] * b[0] + a[1] * b[1]) + a[2] * b[2]
}

/// Cross product of the first three lanes of `a` and `b`.
#[inline]
pub fn cross(a: &[f32], b: &[f32]) -> V3 {
    [(a[1] * b[2]) - (a[2] * b[1]), (a[2] * b[0]) - (a[0] * b[2]), (a[0] * b[1]) - (a[1] * b[0])]
}

/// `sqrtss` (correctly rounded).
#[inline]
pub fn sqrt(x: f32) -> f32 {
    (x as f64).sqrt() as f32
}

/// `rsqrtss` + two Newton steps `y += y*(0.5 - 0.5*x*y*y)`; `rsqrtss` itself is emulated by a
/// correctly rounded `1/sqrt` in double, as the JS does.
#[inline]
pub fn rsqrt(x: f32) -> f32 {
    let mut y = (1.0 / (x as f64).sqrt()) as f32;
    let h = x * 0.5;
    y = y + (y * (0.5 - (h * (y * y))));
    y = y + (y * (0.5 - (h * (y * y))));
    y
}

/// Hamilton product `a * b`.
#[inline]
pub fn qmul(a: Q4, b: Q4) -> Q4 {
    [
        (((a[3] * b[0]) + (a[0] * b[3])) + (a[1] * b[2])) - (a[2] * b[1]),
        (((a[3] * b[1]) - (a[0] * b[2])) + (a[1] * b[3])) + (a[2] * b[0]),
        (((a[3] * b[2]) + (a[0] * b[1])) - (a[1] * b[0])) + (a[2] * b[3]),
        (((a[3] * b[3]) - (a[0] * b[0])) - (a[1] * b[1])) - (a[2] * b[2]),
    ]
}

#[inline]
pub fn qconj(q: Q4) -> Q4 {
    [-q[0], -q[1], -q[2], q[3]]
}

/// `FQuat::RotateVector` shape: `v + 2w(q x v) + 2 q x (q x v)`.
#[inline]
pub fn qrot(q: Q4, v: V3) -> V3 {
    let t = mul(cross(&q, &v), 2.0);
    let c2 = cross(&q, &t);
    [(v[0] + (q[3] * t[0])) + c2[0], (v[1] + (q[3] * t[1])) + c2[1], (v[2] + (q[3] * t[2])) + c2[2]]
}

/// `Math.min` (NaN if either is NaN).
#[inline]
pub fn js_min(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else if a < b {
        a
    } else {
        b
    }
}
