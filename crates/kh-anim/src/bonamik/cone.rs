//! `tools/lib/bonamik/cone.js`: instruction-level transcription (KH3.exe) of
//!   fn_143b166d0  cone constraint solve (only call site 0x143b192f9 in ChainLink 0x143b192a0)
//!   fn_143af6960  helper called by it (attachment point of a particle that has +0x238)
//!   fn_143aec370  cone record allocation from a SQEX_BonamikLinkDesc -> fn_143ae6800 record setup
//!   fn_143aee280_cones  the cone refresh loop inside PreStep 0x143aee280
//!
//! The capacity test of the JS (`scene.f78C === idx`) compares against a field the driver never
//! sets (undefined): it is always false there and is not reproduced.

use super::bnm_eval::LinkDesc;
use super::m::{qconj, qmul, rsqrt, sqrt, Q4, V3, Xf};
use super::records::{Cone, Scene};

const SMALL: f32 = f32::from_bits(0x322bcc77); // 1e-8        0x145C9C488
const TENTH: f32 = f32::from_bits(0x3dcccccd); // 0.1         0x145C87D00
const ARC_MIN: f32 = f32::from_bits(0xbf7ff972); // -0.9999     0x14750BAA8
const P01: f32 = f32::from_bits(0x3c23d70a); // 0.01        0x145CADFC4
const P001: f32 = f32::from_bits(0x3a83126f); // 0.001       0x145C9C4A8
const P0001: f32 = f32::from_bits(0x38d1b717); // 0.0001      0x145CF35A8
const PI: f32 = f32::from_bits(0x40490fdb); // 0x145C9C4F8
const NEG_PI: f32 = f32::from_bits(0xc0490fdb); // 0x145C9C5A0
const HALF_PI: f32 = f32::from_bits(0x3fc90fdb); // 0x145C9C4F0
const NEG_HALF_PI: f32 = f32::from_bits(0xbfc90fdb); // 0x145C9C598
const TWO_PI: f32 = f32::from_bits(0x40c90fdb); // 0x145C9C508
const INV_TWO_PI: f32 = f32::from_bits(0x3e22f983); // 0.15915494  0x145C9C4C4
const DEG2RAD: f32 = f32::from_bits(0x3c8efa35); // 0.017453292 0x145C9C4B4
// acos polynomial 0x14750BAA4 / 0x14750BA98 / 0x14750BA90 / 0x14750BA8C
const AC0: f32 = f32::from_bits(0x3fc90da4);
const AC1: f32 = f32::from_bits(0x3e593484);
const AC2: f32 = f32::from_bits(0x3d981627);
const AC3: f32 = f32::from_bits(0x3c996e30);
// stock FMath::SinCos coefficients 0x145C9C490.. (sin) / 0x145C9C494.. (cos)
const S0: f32 = f32::from_bits(0x32cd365b);
const S1: f32 = f32::from_bits(0x3638b88e);
const S2: f32 = f32::from_bits(0x39500bf1);
const S3: f32 = f32::from_bits(0x3c088886);
const S4: f32 = f32::from_bits(0x3e2aaaab);
const C0: f32 = f32::from_bits(0x348bdd11);
const C1: f32 = f32::from_bits(0x37cfb4c2);
const C2: f32 = f32::from_bits(0x3ab609aa);
const C3: f32 = f32::from_bits(0x3d2aaaa3);

/// FVector::GetSafeNormal shape as inlined everywhere in the module:
/// sq = (y*y + x*x) + z*z ; sq == 1 (or unordered) -> unchanged ; sq < 1e-8 -> ZeroVector ; else v * rsqrt2(sq)
pub fn safe_normal(v: V3) -> V3 {
    let sq = ((v[1] * v[1]) + (v[0] * v[0])) + (v[2] * v[2]);
    if sq == 1.0 || sq.is_nan() {
        return [v[0], v[1], v[2]];
    }
    if !(sq >= SMALL) {
        return [0.0, 0.0, 0.0];
    }
    let y = rsqrt(sq);
    [y * v[0], y * v[1], y * v[2]]
}

/// shortest arc from +X to the unit vector m; `x0` is the bit pattern the exe leaves in the x slot
pub fn arc_from_x(m: V3, x0: f32) -> Q4 {
    if m[0] >= ARC_MIN {
        let s = sqrt((m[0] + m[0]) + 2.0);
        let inv = 1.0 / s;
        return [x0, -(m[2] * inv), m[1] * inv, s * 0.5];
    }
    [0.0, 1.0, 0.0, 0.0]
}

/// rot(q, +X) in the exe's order (m2y = (-y)+(-y), p2z = z+z)
pub fn x_axis(q: Q4) -> V3 {
    let m2y = -q[1] + -q[1];
    let p2z = q[2] + q[2];
    [((q[1] * m2y) - (q[2] * p2z)) + 1.0, (q[3] * p2z) - (q[0] * m2y), (q[0] * p2z) + (q[3] * m2y)]
}

/// the same with the x component of q taken as zero
pub fn x_axis0(q: Q4) -> V3 {
    let m2y = -q[1] + -q[1];
    let p2z = q[2] + q[2];
    [((q[1] * m2y) - (q[2] * p2z)) + 1.0, q[3] * p2z, q[3] * m2y]
}

/// rot(q, +Y) as coded at 0x143b17d4d..
pub fn y_axis(q: Q4) -> V3 {
    let p2x = q[0] + q[0];
    let m2z = -q[2] + -q[2];
    [(q[1] * p2x) + (q[3] * m2z), ((q[2] * m2z) - (q[0] * p2x)) + 1.0, (q[3] * p2x) - (q[1] * m2z)]
}

/// rot(q, +Z) as coded ..0x143b17e4e
pub fn z_axis(q: Q4) -> V3 {
    let m2x = -q[0] + -q[0];
    let p2y = q[1] + q[1];
    [(p2y * q[3]) - (m2x * q[2]), (m2x * q[3]) + (p2y * q[2]), ((m2x * q[0]) - (p2y * q[1])) + 1.0]
}

/// VectorNormalizeQuaternion shape (stock UE SSE)
pub fn qnormalize(q: Q4) -> Q4 {
    let sq = ((q[1] * q[1]) + (q[3] * q[3])) + ((q[0] * q[0]) + (q[2] * q[2]));
    if !(sq >= SMALL) {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let y = rsqrt(sq);
    [y * q[0], y * q[1], y * q[2], y * q[3]]
}

/// FQuat::RotateVector shape: T = 2*(q x v) ; r = (q x T) + (T*w + v)
pub fn rotate(q: Q4, v: V3) -> V3 {
    let mut tx = (q[1] * v[2]) - (q[2] * v[1]);
    let mut ty = (q[2] * v[0]) - (q[0] * v[2]);
    let mut tz = (q[0] * v[1]) - (q[1] * v[0]);
    tx = tx + tx;
    ty = ty + ty;
    tz = tz + tz;
    [
        ((tz * q[1]) - (ty * q[2])) + ((tx * q[3]) + v[0]),
        ((tx * q[2]) - (tz * q[0])) + ((ty * q[3]) + v[1]),
        ((ty * q[0]) - (tx * q[1])) + ((tz * q[3]) + v[2]),
    ]
}

/// "swing with the reference twist removed"
pub fn twist_fix(qs: Q4, x: V3, ref_conj: Q4) -> Q4 {
    let t = qmul(qs, ref_conj);
    let p = ((t[1] * x[1]) + (t[0] * x[0])) + (t[2] * x[2]);
    let twn = qnormalize([p * x[0], p * x[1], p * x[2], t[3]]);
    qmul(qconj(twn), qs)
}

/// polynomial acos (Abramowitz-Stegun 4.4.45), 0x143b17865..
pub fn acos_poly(c: f32) -> f32 {
    let a = c.abs();
    let root = sqrt(1.0 - a);
    let a2 = a * a;
    let mut t = AC0 - (a * AC1);
    t = t + (a2 * AC2);
    t = t - ((a * a2) * AC3);
    let mut r = root * t;
    if !(c >= 0.0) {
        r = PI - r;
    }
    r
}

/// `cvttss2si` of a float (the JS's `Math.trunc` with the out-of-range value).
pub fn cvttss2si(q: f32) -> i32 {
    let n = (q as f64).trunc();
    if !(n >= -2147483648.0 && n <= 2147483647.0) {
        i32::MIN
    } else {
        n as i32
    }
}

/// stock FMath::SinCos (UE 4.17 UnrealMathUtility.h) as inlined; returns [sin, cos]
pub fn sin_cos(v: f32) -> [f32; 2] {
    let mut q = v * INV_TWO_PI;
    q = if v >= 0.0 { q + 0.5 } else { q - 0.5 };
    let n = cvttss2si(q);
    let mut y = v - ((n as f32) * TWO_PI);
    let sign: f32;
    if y > HALF_PI {
        y = PI - y;
        sign = -1.0;
    } else if y >= NEG_HALF_PI {
        sign = 1.0;
    } else {
        y = NEG_PI - y;
        sign = -1.0;
    }
    let y2 = y * y;
    let mut s = S1 - (y2 * S0);
    s = (s * y2) - S2;
    s = (s * y2) + S3;
    s = (s * y2) - S4;
    s = (s * y2) + 1.0;
    s = s * y;
    let mut c = C1 - (y2 * C0);
    c = (c * y2) - C2;
    c = (c * y2) + C3;
    c = (c * y2) - 0.5;
    c = (c * y2) + 1.0;
    c = c * sign;
    [s, c]
}

/// 0x143af6960 (rcx = a pin record, rdx = out): out = Q.pos + rot(Q.quat(+0x20), Q.scale(+0x140) * a.f10), Q = a.f0
pub fn fn_143af6960(sc: &Scene, pin: usize) -> V3 {
    let a = &sc.f758[pin];
    let q = &sc.f0[a.f0];
    let (s, l) = (q.f140, a.f10);
    let rv = rotate(q.f20, [s[0] * l[0], s[1] * l[1], s[2] * l[2]]);
    [rv[0] + q.f0[0], rv[1] + q.f0[1], rv[2] + q.f0[2]]
}

/// 0x143b166d0 (xmm0 = r, rdx = cone, xmm2 = innerConeScale, xmm3 = outerConeScale, [rsp+0x20] byte = enforceLength)
pub fn fn_143b166d0(sc: &mut Scene, r: f32, ci: usize, inner_scale: f32, outer_scale: f32, enforce_length: bool) {
    let c: Cone = sc.f30[ci];
    let (p1, p2) = (c.f0, c.f8);
    let mut w = c.f18; // xmm12: parent share
    if sc.f0[p1].f31d != 0 {
        w = 0.5;
    }
    if TENTH > c.f44 {
        // 143b1673c: degenerate segment
        sc.f0[p2].f20 = sc.f0[p1].f20;
        sc.f0[p2].f0 = sc.f0[p1].f0;
        return;
    }
    let mut s = p1; // rsi: particle whose chain parent defines the reference frame
    if let Some(pin) = sc.f0[p1].f238 {
        sc.f0[p1].f0 = fn_143af6960(sc, pin);
        if sc.f0[p1].f276 == 0 {
            s = sc.f758[pin].f0;
        }
    }
    let qa: Q4;
    let mut qr: Q4;
    let g = sc.f0[s].f190;
    if g.is_none() || sc.f0[s].fbc == 0 {
        // 143b17066
        qa = sc.f0[s].f160;
        qr = sc.f0[s].f160;
    } else {
        let g = g.expect("checked");
        qa = if sc.f0[g].f190.is_some() { sc.f0[g].f20 } else { sc.f0[g].f160 }; // 143b16817
        qr = if sc.f0[p1].f238.is_some() { sc.f0[g].f20 } else { sc.f0[s].f20 };
        if let Some(pin) = sc.f0[p1].f238 {
            // 143b16856 .. 143b17058 (attached particle)
            let (sp, gp) = (sc.f0[s].f0, sc.f0[g].f0);
            let qs = arc_from_x(safe_normal([sp[0] - gp[0], sp[1] - gp[1], sp[2] - gp[2]]), 0.0);
            qr = twist_fix(qs, x_axis0(qs), qconj(qa));
            let a0 = sc.f758[pin].f0;
            if let Some(ch) = sc.f0[a0].f198 {
                let (cp, ap) = (sc.f0[ch].f0, sc.f0[a0].f0);
                let q2 = arc_from_x(safe_normal([cp[0] - ap[0], cp[1] - ap[1], cp[2] - ap[2]]), -0.0);
                sc.f0[a0].f20 = q2;
                sc.f0[a0].f20 = twist_fix(q2, x_axis(q2), qconj(qa));
            }
            sc.f0[p1].f0 = fn_143af6960(sc, pin);
        }
    }
    // 143b17086: hard length
    {
        let (a, b) = (sc.f0[p1].f0, sc.f0[p2].f0);
        let (dx, dy, dz) = (b[0] - a[0], b[1] - a[1], b[2] - a[2]);
        let len = sqrt(((dy * dy) + (dx * dx)) + (dz * dz));
        if enforce_length {
            let inv = 1.0 / len;
            let e = c.f44 - len;
            let (cx, cy, cz) = ((e * dx) * inv, (e * dy) * inv, (e * dz) * inv);
            if sc.f0[p2].f276 == 0 {
                let o = sc.f0[p2].f0;
                sc.f0[p2].f0 = [cx + o[0], cy + o[1], cz + o[2]];
            }
            if sc.f0[p1].f276 == 0 {
                let n = sc.f0[p1].f1a8;
                let o = sc.f0[p1].f58;
                if n > 1 {
                    let k = 1.0 / (n as f32);
                    sc.f0[p1].f58 = [(k * cx) + o[0], (k * cy) + o[1], (k * cz) + o[2]];
                } else {
                    sc.f0[p1].f58 = [cx + o[0], cy + o[1], cz + o[2]];
                }
            }
        }
    }
    // 143b171ad: inner spring towards rot(Qr * cone.f60, +X) * restLength
    let qc = qmul(qr, c.f50); // [rsp+0x60]: limit frame
    let qi = qmul(qr, c.f60); // xmm7: inner target frame
    let l = c.f44;
    let mut k = (c.f28 * inner_scale) * r; // xmm14
    k = if k >= 0.0 {
        if k < 1.0 {
            k
        } else {
            1.0
        }
    } else {
        0.0
    };
    {
        let xi = x_axis(qi);
        let (a, b) = (sc.f0[p1].f0, sc.f0[p2].f0);
        let cx = ((a[0] + (xi[0] * l)) - b[0]) * k;
        let cy = ((a[1] + (xi[1] * l)) - b[1]) * k;
        let cz = ((a[2] + (xi[2] * l)) - b[2]) * k;
        if sc.f0[p1].f276 != 0 {
            if sc.f0[p2].f276 == 0 {
                sc.f0[p2].f0 = [cx + b[0], cy + b[1], cz + b[2]];
            }
        } else if sc.f0[p2].f276 != 0 {
            sc.f0[p1].f0 = [a[0] - cx, a[1] - cy, a[2] - cz];
        } else {
            let om = 1.0 - w;
            sc.f0[p1].f0 = [a[0] - (cx * om), a[1] - (cy * om), a[2] - (cz * om)];
            sc.f0[p2].f0 = [(cx * w) + b[0], (cy * w) + b[1], (cz * w) + b[2]];
        }
    }
    // 143b173fc: current segment direction n = (P2 - P1) / |P2 - P1| (plain divide, no zero guard)
    let pos1 = sc.f0[p1].f0;
    let pos2 = sc.f0[p2].f0;
    let (mut nx, mut ny, mut nz) = (pos2[0] - pos1[0], pos2[1] - pos1[1], pos2[2] - pos1[2]);
    {
        let inv = 1.0 / sqrt(((ny * ny) + (nx * nx)) + (nz * nz));
        nx = nx * inv;
        ny = ny * inv;
        nz = nz * inv;
    }
    let n = [nx, ny, nz];
    // 143b1746e .. 143b177cd computes a value and discards it (dead store); not reproduced.
    let a = x_axis(qc); // cone axis
    let lim_y = c.f30;
    // distribution of an outer-limit correction e (the same code twice: 143b17bbf.. and 143b186b0..)
    let distribute = |sc: &mut Scene, ex: f32, ey: f32, ez: f32| {
        if sc.f0[p1].f276 != 0 {
            if sc.f0[p2].f276 == 0 {
                sc.f0[p2].f0 = [ex + pos2[0], ey + pos2[1], ez + pos2[2]];
            }
        } else if sc.f0[p2].f276 != 0 {
            sc.f0[p1].f0 = [pos1[0] - ex, pos1[1] - ey, pos1[2] - ez];
        } else {
            let om = 1.0 - w;
            sc.f0[p1].f0 = [pos1[0] - (om * ex), pos1[1] - (om * ey), pos1[2] - (om * ez)];
            sc.f0[p2].f0 = [(ex * w) + pos2[0], (ey * w) + pos2[1], (ez * w) + pos2[2]];
            if w > 0.5 {
                let g = ((w - 0.5) + w) - 0.5; // = 2w - 1
                let o = sc.f0[p1].f58;
                sc.f0[p1].f58 = [(g * ex) + o[0], (g * ey) + o[1], (g * ez) + o[2]];
            } else if !(w >= 0.5) {
                let g = (0.5 - w) + (0.5 - w); // = 1 - 2w
                let o = sc.f0[p2].f58;
                sc.f0[p2].f58 = [o[0] - (g * ex), o[1] - (g * ey), o[2] - (g * ez)];
            }
        }
    };
    if !((lim_y - c.f34).abs() >= P01) {
        // 143b17865: circular limit
        let ang = acos_poly(((a[1] * ny) + (a[0] * nx)) + (a[2] * nz));
        if ang > lim_y {
            // A x n
            let (cx, cy, cz) = ((a[1] * nz) - (a[2] * ny), (a[2] * nx) - (a[0] * nz), (a[0] * ny) - (a[1] * nx));
            let clen = sqrt(((cy * cy) + (cx * cx)) + (cz * cz));
            if clen > P001 {
                let inv = 1.0 / clen;
                let sc2 = sin_cos((lim_y - ang) * 0.5); // half angle (negative)
                let s = sc2[0];
                let rot = rotate([(inv * cx) * s, (cy * inv) * s, s * (cz * inv), sc2[1]], n);
                let mut ko = (outer_scale * c.f2c) * r;
                ko = if ko >= 0.0 {
                    if ko < 1.0 {
                        ko
                    } else {
                        1.0
                    }
                } else {
                    0.0
                };
                distribute(sc, (((rot[0] * l) + pos1[0]) - pos2[0]) * ko, (((rot[1] * l) + pos1[1]) - pos2[1]) * ko, (((rot[2] * l) + pos1[2]) - pos2[2]) * ko);
            }
        }
    } else {
        // 143b17d4d: elliptical limit (limY about the frame's Y axis test, limZ about its Z axis)
        let (y, z) = (y_axis(qc), z_axis(qc));
        let mut d: Q4 = [0.0, 0.0, 0.0, 1.0]; // FQuat::Identity 0x14871E410
        let d_y = ((y[1] * ny) + (y[0] * nx)) + (y[2] * nz);
        let p = safe_normal([nx - (d_y * y[0]), ny - (d_y * y[1]), nz - (d_y * y[2])]);
        let ang1 = acos_poly(((p[1] * a[1]) + (p[0] * a[0])) + (p[2] * a[2]));
        if ang1 > lim_y {
            let dd = ((z[1] * p[1]) + (p[0] * z[0])) + (z[2] * p[2]);
            let sg: f32 = if dd >= 0.0 { 1.0 } else { -1.0 };
            let sc2 = sin_cos((ang1 - lim_y) * 0.5);
            let s = sc2[0];
            d = qmul([(y[0] * sg) * s, s * (y[1] * sg), s * (y[2] * sg), sc2[1]], d);
        }
        let d_z = ((z[1] * ny) + (z[0] * nx)) + (z[2] * nz);
        let p2n = safe_normal([nx - (z[0] * d_z), ny - (z[1] * d_z), nz - (z[2] * d_z)]);
        let ang2 = acos_poly(((p2n[0] * a[0]) + (p2n[1] * a[1])) + (p2n[2] * a[2]));
        let lim_z = c.f34;
        if ang2 > lim_z {
            let dd = ((p2n[1] * y[1]) + (p2n[0] * y[0])) + (p2n[2] * y[2]);
            let sg: f32 = if dd >= 0.0 { 1.0 } else { -1.0 };
            let sc2 = sin_cos((lim_z - ang2) * 0.5);
            let s = sc2[0];
            d = qmul([s * (z[0] * sg), (z[1] * sg) * s, (z[2] * sg) * s, sc2[1]], d);
        }
        let rot = rotate(d, n);
        let ko = r * c.f2c; // 143b18706: no outer scale, no clamp (as coded)
        distribute(sc, (((rot[0] * l) + pos1[0]) - pos2[0]) * ko, (((rot[1] * l) + pos1[1]) - pos2[1]) * ko, (((rot[2] * l) + pos1[2]) - pos2[2]) * ko);
    }
    // 143b1893b: orientation
    let (a1, b1) = (sc.f0[p1].f0, sc.f0[p2].f0);
    let m = safe_normal([b1[0] - a1[0], b1[1] - a1[1], b1[2] - a1[2]]);
    let qs = arc_from_x(m, -0.0);
    let pp = &sc.f0[p1];
    if pp.f190.is_some() && pp.f238.is_none() && pp.fbc != 0 && pp.f240.is_none() {
        let res = twist_fix(qs, x_axis(qs), qconj(if c.f20 != 0 { pp.f160 } else { qa }));
        sc.f0[p2].f20 = res;
        if sc.f0[p1].f1a8 == 1 {
            sc.f0[p1].f20 = res;
        }
    } else {
        let res = twist_fix(qs, x_axis(qs), qconj(pp.f160));
        sc.f0[p1].f20 = res;
        sc.f0[p2].f20 = res;
    }
}

/// 0x143ae6800: cone record setup (angles in radians)
#[allow(clippy::too_many_arguments)]
pub fn fn_143ae6800(sc: &Scene, p1: usize, p2: usize, lim_y: f32, lim_z: f32, lim_off_y: f32, lim_off_z: f32, cone_off_y: f32, cone_off_z: f32, outer_str: f32, inner_str: f32, parent_str: f32, anim: u8) -> Cone {
    let ident: Q4 = [0.0, 0.0, 0.0, 1.0];
    let mut rec = Cone { f0: p1, f8: p2, f10: -1, f14: 0.0, f18: 0.0, f20: 0, f24: 0, f28: 0.0, f2c: 0.0, f30: lim_y, f34: lim_z, f38: 0.0, f3c: 0.0, f40: 0.0, f44: 0.0, f50: ident, f60: ident, f70: ident, f80: ident, f90: [1.0, 1.0, 1.0] };
    let (a, b) = (&sc.f0[p1], &sc.f0[p2]);
    let qs = arc_from_x(safe_normal([b.f0[0] - a.f0[0], b.f0[1] - a.f0[1], b.f0[2] - a.f0[2]]), -0.0);
    let q0 = twist_fix(qs, x_axis0(qs), qconj(a.f120)); // frame of the segment P1->P2
    let mut s = p1;
    if let Some(pin) = a.f238 {
        if a.f276 == 0 {
            s = sc.f758[pin].f0;
        }
    }
    let sp = &sc.f0[s];
    if let Some(g) = sp.f190 {
        let gp = &sc.f0[g];
        if gp.f277 == 0 && sp.f276 == 0 && a.f240.is_none() {
            let mut v = [sp.f0[0] - gp.f0[0], sp.f0[1] - gp.f0[1], sp.f0[2] - gp.f0[2]];
            let sq = ((v[1] * v[1]) + (v[0] * v[0])) + (v[2] * v[2]);
            if !(sq >= P01) {
                if let Some(gg) = gp.f190 {
                    let ggp = &sc.f0[gg];
                    v = [gp.f0[0] - ggp.f0[0], gp.f0[1] - ggp.f0[1], gp.f0[2] - ggp.f0[2]];
                }
            }
            let qg = arc_from_x(safe_normal(v), -0.0);
            let qg2 = twist_fix(qg, x_axis0(qg), qconj(gp.f120)); // frame of the segment G->S
            let rel = qmul(qconj(qg2), q0);
            rec.f50 = rel;
            rec.f60 = rel;
        }
    }
    let mut sc2 = sin_cos(lim_off_y * 0.5);
    let q_y: Q4 = [-0.0, sc2[0], -0.0, sc2[1]];
    sc2 = sin_cos(lim_off_z * 0.5);
    let q_z: Q4 = [-0.0, -0.0, sc2[0], sc2[1]];
    let lq = qmul(q_z, q_y);
    rec.f70 = lq;
    rec.f50 = qmul(rec.f50, lq);
    sc2 = sin_cos(cone_off_y * 0.5);
    let q_y2: Q4 = [-0.0, sc2[0], -0.0, sc2[1]];
    sc2 = sin_cos(cone_off_z * 0.5);
    let q_z2: Q4 = [-0.0, -0.0, sc2[0], sc2[1]];
    let cq = qmul(q_z2, q_y2);
    rec.f80 = cq;
    rec.f60 = qmul(rec.f60, cq);
    rec.f28 = inner_str;
    rec.f90 = [1.0, 1.0, 1.0];
    rec.f14 = outer_str;
    rec.f2c = outer_str;
    rec.f18 = parent_str;
    rec.f20 = if anim != 0 { 1 } else { 0 };
    let (dx, dy, dz) = (a.f0[0] - b.f0[0], a.f0[1] - b.f0[1], a.f0[2] - b.f0[2]);
    rec.f40 = sqrt(((dy * dy) + (dx * dx)) + (dz * dz));
    rec.f44 = rec.f40 * rec.f90[0];
    rec
}

/// 0x143aec370 (rcx = scene, rdx = SQEX_BonamikLinkDesc, r8d = particle index base of this asset)
pub fn fn_143aec370(sc: &mut Scene, d: &LinkDesc, base: i32) -> usize {
    let idx = sc.f30.len();
    let (p1, p2) = ((d.f20 + base) as usize, (d.f24 + base) as usize);
    let mut rec = fn_143ae6800(sc, p1, p2, d.f44 * DEG2RAD, d.f48 * DEG2RAD, d.f54 * DEG2RAD, d.f58 * DEG2RAD, d.f4c * DEG2RAD, d.f50 * DEG2RAD, d.f3c, d.f38, d.f40, d.f2c);
    rec.f10 = d.f6c;
    rec.f24 = idx;
    sc.f30.push(rec);
    sc.f0[p1].f2a8 = Some(idx);
    idx
}

/// PreStep 0x143aee280, loop 0x143aee64d..0x143aeebc0 over the cone array.
/// bones = editable component-space transforms (owner vf60)
pub fn fn_143aee280_cones(sc: &mut Scene, bones: &[Xf]) {
    for i in 0..sc.f30.len() {
        let c = sc.f30[i];
        if c.f20 == 0 {
            continue;
        }
        let (p1, p2) = (c.f0, c.f8);
        let mut s = p1;
        if let Some(pin) = sc.f0[p1].f238 {
            if sc.f0[p1].f276 == 0 {
                s = sc.f758[pin].f0;
            }
        }
        let (q1, q2) = (bones[sc.f0[p1].f248].q, bones[sc.f0[p2].f248].q);
        match sc.f0[s].f190 {
            None => {
                sc.f30[i].f50 = c.f70;
                sc.f30[i].f60 = c.f80;
            }
            Some(g) => {
                if sc.f0[g].f277 == 0 && sc.f0[s].f276 == 0 {
                    let rel = qmul(qconj(bones[sc.f0[g].f248].q), q1);
                    sc.f30[i].f50 = qmul(rel, c.f70);
                    sc.f30[i].f60 = qmul(rel, c.f80);
                }
            }
        }
        // twist angle of the child bone against the parent bone about the child bone's X axis
        let x = x_axis(q2);
        let t = qmul(q2, qconj(q1));
        let p = ((t[0] * x[0]) + (t[1] * x[1])) + (t[2] * x[2]);
        let mut tw = [p * x[0], p * x[1], p * x[2], t[3]];
        let sq = (((tw[1] * tw[1]) + (tw[0] * tw[0])) + (tw[2] * tw[2])) + (tw[3] * tw[3]);
        if !(sq > P0001) {
            tw = [0.0, 0.0, 0.0, 1.0];
        }
        tw = qnormalize(tw);
        let ang = acos_poly(tw[3]);
        sc.f0[p2].f314 = ang + ang;
        let sv = bones[sc.f0[p1].f248].s;
        sc.f30[i].f90 = [sv[0], sv[1], sv[2]];
        sc.f30[i].f44 = c.f40 * sv[0];
    }
}
