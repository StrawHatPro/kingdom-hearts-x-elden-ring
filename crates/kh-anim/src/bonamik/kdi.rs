//! `tools/lib/bonamik/kdi.js`: KDI constraint (ESQEX_Bonamik_LinkType 4), Pin constraint
//! (type 3), attached chains. Transcribed at instruction level from KH3.exe.
//!
//!   pin record : f0 parent particle, f8 child particle, f10 vec3 (child in the parent frame,
//!                setup pose), f1C m_PinStr, f20 m_PinRadius
//!   KDI record : f0 driven particle (the link CHILD), f8 driver entries
//!   driver entry: f0 driver particle (the link PARENT), f8 vec3 (child in the driver frame, setup
//!                pose), f20 quat (conj(driver.f20) * child.f20, setup pose), f30 m_KdiTrans / sum,
//!                f34 m_KdiRot / sum
//!
//! The CRT functions of 0x142d90ca0 (`acosf`, `sinf`) are computed in double and rounded, as the
//! JS does (`Math.acos`, `Math.sin`).

use super::bnm_eval::LinkDesc;
use super::chain;
use super::m::{qconj, qmul, qrot, rsqrt, sqrt, sub, Q4, V3};
use super::records::{Core, Kdi, KdiEntry, Pin, Scene};
use super::wind;

const EPS8: f32 = 1e-8_f64 as f32; // 0x322BCC77
const EPS4: f32 = 1e-4_f64 as f32; // 0x38D1B717
const M9999: f32 = -0.9999_f64 as f32; // 0xBF7FF972
const P9999: f32 = 0.9999_f64 as f32; // 0x3F7FF972

/// componentwise scale of the bone FTransform, inverted only when min(|sx|,|sy|,|sz|) >= 1e-4
fn inv_scale(s: V3) -> V3 {
    let (ax, ay, az) = (s[0].abs(), s[1].abs(), s[2].abs());
    let mut m = if ay < ax { ay } else { ax }; // minss xmm2(|sy|), xmm0(|sx|)
    m = if m < az { m } else { az }; // minss xmm2, xmm0(|sz|)
    if m >= EPS4 {
        return [1.0 / s[0], 1.0 / s[1], 1.0 / s[2]];
    }
    [s[0], s[1], s[2]] // NOT inverted (and not replaced by 1): as coded
}

/// stock FVector::GetSafeNormal(1e-8) shape
fn safe_normal(d: V3) -> V3 {
    let l2 = ((d[1] * d[1]) + (d[0] * d[0])) + (d[2] * d[2]);
    if l2 == 1.0 || l2.is_nan() {
        return [d[0], d[1], d[2]];
    }
    if !(l2 >= EPS8) {
        return [0.0, 0.0, 0.0];
    }
    let y = rsqrt(l2);
    [d[0] * y, d[1] * y, d[2] * y]
}

/// shortest arc from +X to the unit vector v, as coded (143af60f6..143af6163)
fn arc_from_x(v: V3) -> Q4 {
    if !(v[0] >= M9999) {
        return [0.0, 1.0, 0.0, 0.0];
    }
    let s = sqrt((v[0] + v[0]) + 2.0);
    let inv = 1.0 / s;
    let w = s * 0.5;
    [-0.0, -(inv * v[2]), inv * v[1], w]
}

/// stock VectorNormalizeQuaternion
fn qnorm_safe(v: Q4) -> Q4 {
    let (s0, s1, s2, s3) = (v[0] * v[0], v[1] * v[1], v[2] * v[2], v[3] * v[3]);
    let n = (s1 + s3) + (s0 + s2);
    if !(EPS8 <= n) {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let y = rsqrt(n);
    [y * v[0], y * v[1], y * v[2], y * v[3]]
}

// ---- setup
/// 0x143aed5a0 (rcx scene, edx parent particle index, r8d child particle index, xmm3 m_PinRadius, [rsp+28h] m_PinStr)
pub fn fn_143aed5a0(sc: &mut Scene, parent_idx: usize, child_idx: usize, pin_radius: f32, pin_str: f32) {
    let (p, c) = (&sc.f0[parent_idx], &sc.f0[child_idx]);
    let s = inv_scale(p.f140);
    let r = qrot(qconj(p.f20), sub(c.f0, p.f0)); // 143aed6f0..143aed7ed
    let rec = Pin { f0: parent_idx, f8: child_idx, f10: [r[0] * s[0], r[1] * s[1], r[2] * s[2]], f20: pin_radius, f1c: pin_str };
    sc.f758.push(rec);
    sc.f0[child_idx].f238 = Some(sc.f758.len() - 1);
}

/// 0x143aecd20 (rcx KDI record, rdx driver particle, xmm2 m_KdiTrans, xmm3 m_KdiRot)
pub fn fn_143aecd20(sc: &mut Scene, rec: usize, d: usize, kdi_trans: f32, kdi_rot: f32) {
    let (cp, dp) = (&sc.f0[sc.f768[rec].f0], &sc.f0[d]);
    let c = qconj(dp.f20);
    let s = inv_scale(dp.f140);
    let r = qrot(c, sub(cp.f0, dp.f0)); // 143aece49..143aecf2a
    let local = [r[0] * s[0], r[1] * s[1], r[2] * s[2]];
    let rel = qmul(c, cp.f20); // 143aecf17..143aecf88
    sc.f768[rec].f8.push(KdiEntry { f0: d, f8: local, f20: rel, f30: kdi_trans, f34: kdi_rot });
}

/// 0x143afb8e0 (rcx KDI record): divide f30 by the sum of f30 and f34 by the sum of f34 (each only if its sum > 0)
pub fn fn_143afb8e0(rec: &mut Kdi) {
    let n = rec.f8.len();
    // (ER69) 8 or more drivers (143afb945..143afba5c): the same sums in 4 lanes; summed in order here
    let _ = n;
    let (mut s_t, mut s_r) = (0.0f32, 0.0f32);
    for e in &rec.f8 {
        s_t = s_t + e.f30;
        s_r = s_r + e.f34;
    }
    for e in rec.f8.iter_mut() {
        if s_t > 0.0 {
            e.f30 = e.f30 / s_t;
        }
        if s_r > 0.0 {
            e.f34 = e.f34 / s_r;
        }
    }
}

/// The Pin/KDI block of core setup 0x143b07550 (143b09d04..143b0a03d), one asset.
pub fn setup_pin_kdi(sc: &mut Scene, links: &[LinkDesc], base: i32) {
    // 143b09ddb..143b09e0a (arrays emptied, reserved)
    sc.f758 = Vec::new();
    sc.f768 = Vec::new();
    // 143b09e90..143b09fca
    let total = sc.f0.len() as u32;
    for l in links {
        let (pi, ci) = ((l.f20 + base) as u32, (l.f24 + base) as u32);
        if !(pi < total) || !(ci < total) {
            continue; // unsigned compares (jae skip)
        }
        let (pi, ci) = (pi as usize, ci as usize);
        if l.f28 == 3 {
            fn_143aed5a0(sc, pi, ci, l.f74, l.f78);
        } else if l.f28 == 4 {
            let rec = match sc.f0[ci].f240 {
                Some(rec) => rec,
                None => {
                    // first KDI link of this child
                    sc.f768.push(Kdi { f0: ci, f8: Vec::new() });
                    let rec = sc.f768.len() - 1;
                    sc.f0[ci].f240 = Some(rec);
                    sc.f0[ci].f276 = 1; // 143b09f92: the driven particle becomes kinematic
                    rec
                }
            };
            fn_143aecd20(sc, rec, pi, l.f7c, l.f80);
        }
    }
    // 143b09ff9..143b0a03d
    for rec in sc.f768.iter_mut() {
        fn_143afb8e0(rec);
    }
}

// ---- per step
/// 0x142d90ca0 (rcx out, rdx a, r8 b, xmm3 alpha): stock FQuat::Slerp_NotNormalized
pub fn fn_142d90ca0(a: Q4, b: Q4, alpha: f32) -> Q4 {
    let mut raw = ((a[1] * b[1]) + (a[0] * b[0])) + (a[2] * b[2]);
    raw = raw + (a[3] * b[3]);
    let ad = raw.abs();
    let (s0, mut s1);
    if ad >= P9999 {
        // comiss |raw|,0.9999 ; jae linear
        s0 = 1.0 - alpha;
        s1 = alpha;
    } else {
        let mut x: f32 = -1.0;
        if !(ad < -1.0) {
            x = if ad < 1.0 { ad } else { 1.0 }; // comiss ad,-1 ; jb keep -1 ; minss ad,1
        }
        if ad.is_nan() {
            x = -1.0; // unordered takes the jb
        }
        let om = (x as f64).acos() as f32;
        let inv = 1.0 / ((om as f64).sin() as f32);
        s0 = ((((1.0 - alpha) * om) as f64).sin() as f32) * inv;
        s1 = (((om * alpha) as f64).sin() as f32) * inv;
    }
    if !(raw >= 0.0) {
        s1 = -s1; // comiss raw,0 ; jae keep
    }
    [(s1 * b[0]) + (s0 * a[0]), (s1 * b[1]) + (s0 * a[1]), (s1 * b[2]) + (s0 * a[2]), (s1 * b[3]) + (s0 * a[3])]
}

/// The normalisation of 0x143b03380: a vector of length 1 is kept, one shorter than 1e-4 becomes zero
/// (DAT_149b5c798, the zero vector), any other is scaled by its refined reciprocal square root.
fn twist_normal(v: V3) -> V3 {
    let l = ((v[1] * v[1]) + (v[0] * v[0])) + (v[2] * v[2]);
    if l == 1.0 {
        return v;
    }
    if !(1e-8 <= l) {
        return [0.0; 3];
    }
    let k = rsqrt(l);
    [v[0] * k, v[1] * k, v[2] * k]
}

/// The acos approximation of 0x143b03380 (|x| <= 1).
fn twist_acos(c: f32) -> f32 {
    let a = c.abs();
    let poly = ((1.5707288 - (a * 0.2121144)) + ((a * a) * 0.074261)) - (((a * a) * a) * 0.0187293);
    let r = sqrt(1.0 - a) * poly;
    if c < 0.0 {
        3.1415927 - r
    } else {
        r
    }
}

/// 0x143b03380 (rcx particle P) (IMPL_ER59 / ER69, transcribed for the outfits' assets): THE TWIST FIX of a KDI driver
/// whose child particle C has twist neighbours. Z = P.f20's Z axis, d = (C - P) normalised, s = sum over C's neighbours N
/// of sign_N * (d x (N - C) normalised), normalised, made perpendicular to d; the signed angle between Z and s about d,
/// less C.f318, halved and wrapped, rotates P.f20 about d (q' = r * q; the sign tables 0x149d52850.. set by
/// 0x14035c180 / 190 / 1a0 are the quaternion product's). The sine / cosine are the engine's polynomial (FMath::SinCos).
pub fn fn_143b03380(sc: &mut Scene, p: usize) {
    let Some(c) = sc.f0[p].f198 else { return };
    let count = sc.f0[c].f310.min(4);
    if count == 0 {
        return;
    }
    let q = sc.f0[p].f20;
    let (y2, mx2) = (q[1] + q[1], (-q[0]) + (-q[0]));
    let zx = (y2 * q[3]) - (mx2 * q[2]);
    let zy = (mx2 * q[3]) + (y2 * q[2]);
    let zz = ((mx2 * q[0]) - (y2 * q[1])) + 1.0;
    let d = twist_normal(sub(sc.f0[c].f0, sc.f0[p].f0));
    let (mut sx, mut sy, mut sz) = (0.0f32, 0.0f32, 0.0f32);
    for k in 0..count {
        let n = sc.f0[c].f2e0[k];
        let e = twist_normal(sub(sc.f0[n].f0, sc.f0[c].f0));
        let w = sc.f0[c].f300[k] as f32;
        sx = sx + (((e[2] * d[1]) - (e[1] * d[2])) * w);
        sz = sz + (((e[1] * d[0]) - (e[0] * d[1])) * w);
        sy = sy + (((e[0] * d[2]) - (e[2] * d[0])) * w);
    }
    let s0 = twist_normal([sx, sy, sz]);
    let dot = ((s0[1] * d[1]) + (s0[0] * d[0])) + (s0[2] * d[2]);
    let v = twist_normal([s0[0] - (d[0] * dot), s0[1] - (d[1] * dot), s0[2] - (d[2] * dot)]);
    let cosang = (((zy * v[1]) + (zx * v[0])) + (zz * v[2])).clamp(-1.0, 1.0);
    let triple = ((((zy * v[2]) - (zz * v[1])) * d[0]) + (((zz * v[0]) - (zx * v[2])) * d[1])) + (((zx * v[1]) - (zy * v[0])) * d[2]);
    let ang = if triple < 0.0 { -twist_acos(cosang) } else { twist_acos(cosang) };
    let target = sc.f0[c].f318;
    if !(0.0001 < (ang - target).abs()) {
        return;
    }
    let mut h = (ang - target) * 0.5;
    let k = if h < 0.0 { (h * 0.15915494) - 0.5 } else { (h * 0.15915494) + 0.5 };
    h = h - ((k as i32) as f32 * 6.2831855);
    let mut sign = -1.0f32;
    if h <= 1.5707964 {
        if -1.5707964 <= h {
            sign = 1.0;
        } else {
            h = -3.1415927 - h;
        }
    } else {
        h = 3.1415927 - h;
    }
    let h2 = h * h;
    let sn = (((((((2.7525562e-06 - (h2 * 2.3889859e-08)) * h2) - 0.00019840874) * h2) + 0.008333331) * h2 - 0.16666667) * h2 + 1.0) * h;
    let cs = (((((((2.4760495e-05 - (h2 * 2.6051615e-07)) * h2) - 0.0013888378) * h2) + 0.041666638) * h2 - 0.5) * h2 + 1.0) * sign;
    let (ax, ay, az) = (d[0] * sn, d[1] * sn, d[2] * sn);
    let (x, y, z, w) = (q[0], q[1], q[2], q[3]);
    sc.f0[p].f20 = [(((w * ax) + (cs * x)) + (z * ay)) - (y * az), (((-(z * ax)) + (cs * y)) + (w * ay)) + (x * az), (((y * ax) + (cs * z)) - (x * ay)) + (w * az), (((-(x * ax)) + (cs * w)) - (y * ay)) - (z * az)];
}

/// 0x143af5f00 (rcx KDI record, rdx out quat) -> out.
/// Side effects: for every driver D of the record D.f20 is rebuilt from the CURRENT particle positions.
pub fn fn_143af5f00(sc: &mut Scene, rec: usize) -> Q4 {
    let c = sc.f768[rec].f0;
    let mut out = sc.f0[c].f20;
    for i in 0..sc.f768[rec].f8.len() {
        let e = sc.f768[rec].f8[i];
        let d = e.f0;
        if let Some(ch) = sc.f0[d].f198 {
            // 143af601f: driver has a child particle
            let v = safe_normal(sub(sc.f0[ch].f0, sc.f0[d].f0));
            sc.f0[d].f20 = arc_from_x(v); // 143af6557
        } else if sc.f0[d].f190.is_some() {
            // 143af617d: no child, but a parent particle
            // (ER69) NOT TRANSCRIBED, a FALLBACK instead of a stop (an outfit's asset reaches it; Sora's never): 143af617d..143af6552 (KDI driver without child particle): D.f20 as it is
            super::unported(14);
        } // neither: D.f20 is used as it is
          // 143af655b: remove from D.f20 the twist about its own X axis relative to D.f160
        let (q, tq) = (sc.f0[d].f20, sc.f0[d].f160);
        let cq = [-tq[0], -tq[1], -tq[2], tq[3]];
        let z2 = q[2] + q[2];
        let my2 = (-q[1]) + (-q[1]);
        let ay = (q[3] * z2) - (q[0] * my2);
        let ax = ((q[1] * my2) - (q[2] * z2)) + 1.0;
        let az = (q[3] * my2) + (q[0] * z2);
        let p = qmul(q, cq);
        let dp = ((p[1] * ay) + (p[0] * ax)) + (p[2] * az);
        let n = qnorm_safe([dp * ax, dp * ay, dp * az, p[3]]);
        sc.f0[d].f20 = qmul([-n[0], -n[1], -n[2], n[3]], q); // 143af67c0
        fn_143b03380(sc, d); // 143af67c4
        let t = qmul(sc.f0[d].f20, e.f20); // 143af67c9..143af6837
        let tmp = fn_142d90ca0(out, t, e.f34); // 143af6842
        out = qnorm_safe(tmp); // 143af6860..143af68ce
    }
    out
}

/// The KDI pass inlined in Step 0x143b0cb60 (143b0d7fb..143b0df90). Runs once per sub-step, after the collision
/// iterations and before 0x143b15610.
pub fn fn_143b0cb60_kdi_pass(sc: &mut Scene) {
    for i in 0..sc.f768.len() {
        let c = sc.f768[i].f0;
        let q = fn_143af5f00(sc, i);
        sc.f0[c].f20 = [q[0], q[1], q[2], q[3]]; // 143b0d843
        let (mut ax, mut ay, mut az) = (0.0f32, 0.0f32, 0.0f32);
        for e in &sc.f768[i].f8 {
            // 143b0d8a0 (x4 unrolled) / 143b0ddfc (tail)
            let (d, w) = (&sc.f0[e.f0], e.f30);
            let v = [d.f140[0] * e.f8[0], d.f140[1] * e.f8[1], d.f140[2] * e.f8[2]];
            let r = qrot(d.f20, v);
            ax = ax + ((r[0] + d.f0[0]) * w);
            ay = ay + ((r[1] + d.f0[1]) * w);
            az = az + ((r[2] + d.f0[2]) * w);
        }
        sc.f0[c].f0 = [ax, ay, az]; // 143b0df68
        sc.f0[c].f160 = sc.f0[c].f20; // 143b0df7e
    }
}

/// 0x143af6960 (rcx pin record, rdx out vec3) -> out: parent.pos + rot(parent.f20, parent.f140 * rec.f10)
pub fn fn_143af6960(sc: &Scene, pin: usize) -> V3 {
    let rec = &sc.f758[pin];
    let p = &sc.f0[rec.f0];
    let v = [p.f140[0] * rec.f10[0], p.f140[1] * rec.f10[1], p.f140[2] * rec.f10[2]];
    let r = qrot(p.f20, v);
    [r[0] + p.f0[0], r[1] + p.f0[1], r[2] + p.f0[2]]
}

/// 0x143b1a4d0 (xmm0 r [only passed on], rdx link): hard rest length for the child, recursive
pub fn fn_143b1a4d0(sc: &mut Scene, r: f32, link: usize) {
    let l = sc.f40[link];
    let (p1, p2) = (l.f0, l.f8);
    let (a, b) = (sc.f0[p1].f0, sc.f0[p2].f0);
    let (dx, dy, dz) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    let l2 = ((dy * dy) + (dx * dx)) + (dz * dz);
    if l2 > EPS4 {
        // comiss L2,1e-4 ; jbe skip
        let len = sqrt(l2);
        let inv = 1.0 / len;
        let e = len - l.f20;
        if sc.f0[p2].f276 == 0 {
            sc.f0[p2].f0 = [((dx * inv) * e) + b[0], ((inv * dy) * e) + b[1], ((inv * dz) * e) + b[2]];
        }
    }
    for k in 0..sc.f0[p2].f1a8 {
        let next = sc.f0[p2].f1b0[k];
        fn_143b1a4d0(sc, r, next);
    }
}

/// 0x143b15610 (xmm0 r, rdx core, r8 = Step's 2nd argument [unused], r9 = core+0x658)
pub fn fn_143b15610(r: f32, core: &mut Core) {
    for i in 0..core.f3778.f758.len() {
        // 143b15660: pinned particles
        let p = core.f3778.f758[i].f8;
        let pin = core.f3778.f0[p].f238.expect("a pinned particle has its pin record");
        let centre = fn_143af6960(&core.f3778, pin);
        // rdx = particle (+0 = pos), xmm3 = 0, [rsp+20] = 1
        wind::fn_143b1b830(&mut core.f3778.f0[p].f0, centre, 0.0, 1.0);
        for k in 0..core.f3778.f0[p].f1a8 {
            let l = core.f3778.f0[p].f1b0[k];
            chain::fn_143b192a0(r, l, core);
        }
    }
    for i in 0..core.f3778.f768.len() {
        // 143b15710: KDI driven particles
        let p = core.f3778.f768[i].f0;
        for k in 0..core.f3778.f0[p].f1a8 {
            let l = core.f3778.f0[p].f1b0[k];
            chain::fn_143b192a0(r, l, core);
        }
    }
    for i in 0..core.f3778.f758.len() {
        // 143b15780
        let p = core.f3778.f758[i].f8;
        for k in 0..core.f3778.f0[p].f1a8 {
            let l = core.f3778.f0[p].f1b0[k];
            fn_143b1a4d0(&mut core.f3778, r, l);
        }
    }
    for i in 0..core.f3778.f768.len() {
        // 143b157e0
        let p = core.f3778.f768[i].f0;
        for k in 0..core.f3778.f0[p].f1a8 {
            let l = core.f3778.f0[p].f1b0[k];
            fn_143b1a4d0(&mut core.f3778, r, l);
        }
    }
}
