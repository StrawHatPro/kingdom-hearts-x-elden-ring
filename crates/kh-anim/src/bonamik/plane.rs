//! (IMPL_ER59 / ER69) THE LIMIT PLANE (a triangle of three particles) AGAINST A SPHERE / A CAPSULE, transcribed from the
//! decompiled C (re/decomp/143B1/143B1BE30.c, 143B1D720.c, 143B1CB50.c) for Sora's outfits' assets (Sora's own asset never
//! reaches them; KH2 Sora's and the Caribbean coat's do). Plane object: f0 / f8 / f10 its particles A, B, C, f20 the
//! normal, f2c a point, f60 the offset along the normal, f5c the extra radius. DAT_149b5c798 (an FVector in .data's
//! zero-filled part, used as the zero vector by its other readers): zero.

use super::coll::{fn_143b15850, fn_143b1b110, fn_143b1c700};
use super::m::{rsqrt, sqrt, V3};
use super::records::{CollObj, Particle};

#[inline]
fn min1(s: f32) -> f32 {
    if s < 1.0 {
        s
    } else {
        1.0
    }
}

/// Barycentric (s, t) of `p` on the triangle a, b, c with e1 = c - a, e2 = b - a (the C's own formula and order).
fn tri_bary(p: V3, a: V3, b: V3, c: V3) -> (f32, f32) {
    let e1 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let e2 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let d = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
    let e1e1 = ((e1[1] * e1[1]) + (e1[0] * e1[0])) + (e1[2] * e1[2]);
    let e2e1 = ((e2[1] * e1[1]) + (e2[0] * e1[0])) + (e2[2] * e1[2]);
    let pe1 = ((d[1] * e1[1]) + (d[0] * e1[0])) + (d[2] * e1[2]);
    let pe2 = ((d[1] * e2[1]) + (d[0] * e2[0])) + (d[2] * e2[2]);
    let e2e2 = ((e2[1] * e2[1]) + (e2[0] * e2[0])) + (e2[2] * e2[2]);
    let inv = 1.0 / ((e2e2 * e1e1) - (e2e1 * e2e1));
    (((e2e2 * pe1) - (pe2 * e2e1)) * inv, ((pe2 * e1e1) - (pe1 * e2e1)) * inv)
}

/// 0x143b1be30 (rcx = p, rdx = plane object o, xmm2..4 = u, v, w): a particle against the triangle, pushed apart.
pub fn fn_143b1be30(ps: &mut [Particle], p: usize, o: &CollObj, u: f32, v: f32, w: f32) {
    let (Some(b), Some(c)) = (o.f8, o.f10) else { return };
    let a = o.f0;
    if p == a || p == b || p == c {
        return;
    }
    let th = o.f60;
    let (mut nx, mut ny, mut nz) = (o.f20[0], o.f20[1], o.f20[2]);
    let mut d = ((ps[p].f0[0] - ((nx * th) + o.f2c[0])) * nx + (ps[p].f0[1] - ((ny * th) + o.f2c[1])) * ny) + (ps[p].f0[2] - ((nz * th) + o.f2c[2])) * nz;
    if d < 0.0 {
        (nx, ny, nz, d) = (-nx, -ny, -nz, -d);
    }
    let reach = ps[p].f25c + o.f5c;
    if reach <= d {
        return;
    }
    let pen = reach - d;
    let mut push = pen;
    if ps[p].f276 == 0 {
        push = pen * 0.5;
        let q = &mut ps[p];
        q.f0[0] = (nx * push) + q.f0[0];
        q.f0[1] = (ny * push) + q.f0[1];
        q.f0[2] = (nz * push) + q.f0[2];
        q.f279 = 1;
        // (the C adds the plane's own normal here, not the flipped one)
        q.f64[0] = (push * o.f20[0]) + q.f64[0];
        q.f64[2] = (o.f20[2] * push) + q.f64[2];
        q.f64[1] = (o.f20[1] * push) + q.f64[1];
        let fr = q.f268;
        let (vx, vy, vz) = (q.f0[0] - q.fc[0], q.f0[1] - q.fc[1], q.f0[2] - q.fc[2]);
        let vn = ((vx * nx) + (vy * ny)) + (vz * nz);
        let (mut tx, mut ty, mut tz) = (vx - (nx * vn), vy - (ny * vn), vz - (nz * vn));
        if (((pen * pen) * fr) * fr) * 0.25 <= ((ty * ty) + (tx * tx)) + (tz * tz) {
            ty = ty * fr;
            tx = fr * tx;
            tz = tz * fr;
        }
        q.f0[0] = q.f0[0] - tx;
        q.f0[2] = q.f0[2] - tz;
        q.f0[1] = q.f0[1] - ty;
    }
    let (ka, kb, kc) = (ps[a].f276, ps[b].f276, ps[c].f276);
    if ka != 0 || kb != 0 || kc != 0 {
        push = push * 2.5;
    }
    let (sw, sv, su) = (push * w, push * v, push * u);
    let tri_velocity = |ps: &[Particle], r: V3| -> V3 {
        let k = |i: usize| ((ps[a].f0[i] * u) + (v * ps[b].f0[i]) + (w * ps[c].f0[i])) - ((u * ps[a].fc[i]) + (v * ps[b].fc[i]) + (w * ps[c].fc[i]));
        [r[0] - k(0), r[1] - k(1), r[2] - k(2)]
    };
    let mut rel: V3 = [0.0; 3];
    if ka == 0 {
        rel = [ps[p].f0[0] - ps[p].fc[0], ps[p].f0[1] - ps[p].fc[1], ps[p].f0[2] - ps[p].fc[2]];
        rel = tri_velocity(ps, rel);
    } else if kb == 0 || kc == 0 {
        rel = tri_velocity(ps, rel);
    }
    let fr = ps[a].f268;
    let rn = ((rel[1] * ny) + (rel[0] * nx)) + (rel[2] * nz);
    let (rx, ry, rz) = (rel[0] - (nx * rn), rel[1] - (ny * rn), rel[2] - (nz * rn));
    let tt = ((ry * ry) + (rx * rx)) + (rz * rz);
    let mut len = 0.0f32;
    let move_vertex = |ps: &mut [Particle], i: usize, by: f32, weight: f32, len: &mut f32| {
        let q = &mut ps[i];
        q.f0[0] = q.f0[0] - (nx * by);
        q.f0[2] = q.f0[2] - (nz * by);
        q.f0[1] = q.f0[1] - (ny * by);
        q.f279 = 1;
        q.f64[0] = q.f64[0] - (nx * by);
        q.f64[1] = q.f64[1] - (ny * by);
        q.f64[2] = q.f64[2] - (nz * by);
        let lim = fr * by;
        let k = if lim * lim <= tt {
            if tt <= 0.0 {
                return;
            }
            if *len < 1e-8 {
                *len = sqrt(tt);
            }
            min1(lim / *len)
        } else {
            weight
        };
        q.f0[0] = (rx * k) + q.f0[0];
        q.f0[1] = (ry * k) + q.f0[1];
        q.f0[2] = (rz * k) + q.f0[2];
    };
    if ka == 0 {
        move_vertex(ps, a, sw, w, &mut len);
    }
    if ps[b].f276 == 0 {
        move_vertex(ps, b, sv, v, &mut len);
    }
    if ps[c].f276 == 0 {
        move_vertex(ps, c, su, u, &mut len);
    }
}

/// 0x143b1d720 (rcx = plane object o, rdx = p): limit plane vs sphere.
pub fn fn_143b1d720(ps: &mut [Particle], o: &CollObj, p: usize) {
    let (Some(b), Some(c)) = (o.f8, o.f10) else { return };
    let a = o.f0;
    let (s, t) = tri_bary(ps[p].f0, ps[a].f0, ps[b].f0, ps[c].f0);
    if s >= 0.0 && t >= 0.0 && s + t < 1.0 {
        fn_143b1be30(ps, p, o, s, t, (1.0 - s) - t);
        return;
    }
    let off = [o.f60 * o.f20[0], o.f60 * o.f20[1], o.f60 * o.f20[2]];
    if s < 0.0 && t < 0.0 {
        fn_143b1c700(ps, p, a, off, o.f5c);
        return;
    }
    // the nearest edge: B-C (s >= 0, t >= 0), C-A (s >= 0, t < 0), B-A (s < 0, t >= 0)
    let (e0, e1) = if s >= 0.0 {
        if t >= 0.0 {
            (b, c)
        } else {
            (c, a)
        }
    } else {
        (b, a)
    };
    fn_143b1b110(ps, p, e0, e1, 0.0, off, o.f5c);
}

/// 0x143b1cb50 (rcx = plane object o, rdx / r8 = the capsule's particles p2, p3): limit plane vs capsule.
pub fn fn_143b1cb50(ps: &mut [Particle], o: &CollObj, p2: usize, p3: usize) {
    let (Some(b), Some(c)) = (o.f8, o.f10) else { return };
    let a = o.f0;
    let (pa, pb) = (ps[p2].f0, ps[p3].f0);
    let mut dir = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
    let dd = ((dir[1] * dir[1]) + (dir[0] * dir[0])) + (dir[2] * dir[2]);
    if 1e-8 < dd {
        let k = rsqrt(dd);
        dir = [dir[0] * k, dir[1] * k, dir[2] * k];
    }
    let n = o.f20;
    let th = o.f60;
    let off = [n[0] * th, n[1] * th, n[2] * th];
    let r = o.f5c;
    let dn = ((dir[0] * n[0]) + (dir[1] * n[1])) + (dir[2] * n[2]);
    if dn.abs() < 0.2 {
        fn_143b15850(ps, p2, p3, 0.0, a, b, 0.0, off, r);
        fn_143b15850(ps, p2, p3, 0.0, b, c, 0.0, off, r);
        fn_143b15850(ps, p2, p3, 0.0, c, a, 0.0, off, r);
        for p in [p2, p3] {
            let (s, t) = tri_bary(ps[p].f0, ps[a].f0, ps[b].f0, ps[c].f0);
            if s >= 0.0 && t >= 0.0 && s + t < 1.0 {
                fn_143b1be30(ps, p, o, s, t, (1.0 - s) - t);
            }
        }
        return;
    }
    let av = ps[a].f0;
    let rel = [pb[0] - av[0], pb[1] - av[1], pb[2] - av[2]];
    let along = (-1.0 / dn) * (((n[1] * rel[1]) + (n[0] * rel[0])) + (n[2] * rel[2]));
    let (x, end) = if along <= 0.0 {
        (pa, Some(p2))
    } else if along < 1.0 {
        ([(dir[0] * along) + pa[0], (dir[1] * along) + pa[1], (dir[2] * along) + pa[2]], None)
    } else {
        (pb, Some(p3))
    };
    let (s, t) = tri_bary(x, av, ps[b].f0, ps[c].f0);
    if s >= 0.0 && t >= 0.0 && s + t < 1.0 {
        if let Some(p) = end {
            fn_143b1be30(ps, p, o, s, t, (1.0 - s) - t);
        }
        return;
    }
    // the labels of the C: 0x143b1d62d (t >= 0: edge B-C), 0x143b1d66a / 67c (s >= 0, t < 0: edge C-A),
    // 0x143b1d670 (s < 0: t >= 0 edge B-A, t < 0 nothing)
    let edge = if s >= 0.0 {
        if t >= 0.0 {
            Some((b, c))
        } else {
            Some((c, a))
        }
    } else if t >= 0.0 {
        Some((b, a))
    } else {
        None
    };
    if let Some((e0, e1)) = edge {
        fn_143b15850(ps, p2, p3, 0.0, e0, e1, 0.0, off, r);
    }
}
