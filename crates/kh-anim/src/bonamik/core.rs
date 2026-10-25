//! `tools/lib/bonamik/core.js`: group weights 0x143b10010, bone weights 0x143b11700, the time
//! step of CoreUpdate 0x143b0efc0, setup blocks of 0x143b07550 (orientation init, chain roots,
//! hard-length marking).
//!
//! Not ported: `initTwistRest` and `acosPoly` of core.js (the driver uses
//! `chain::blk_143b0a327` for the joint-twist rest angles; nothing calls them).

use super::m::{cross, mul, qconj, qmul, rsqrt, sqrt, sub, Q4, V3};
use super::records::{Cfg, Core};

const ZERO: V3 = [0.0, 0.0, 0.0];
const C_1E8: f32 = 1e-8_f64 as f32;
const C_0001: f32 = 0.0001_f64 as f32;
const C_9999: f32 = 0.9999_f64 as f32;

/// normalize as the module does: if |v|^2 == 1 keep; if < 1e-8 -> zero vector; else v * rsqrt
pub fn norm_safe(v: V3) -> V3 {
    let l = ((v[0] * v[0]) + (v[1] * v[1])) + (v[2] * v[2]);
    if l == 1.0 {
        return v;
    }
    if !(l >= C_1E8) {
        return ZERO;
    }
    let k = rsqrt(l);
    [v[0] * k, v[1] * k, v[2] * k]
}

/// shortest arc from +X to unit vector d, as coded
pub fn arc_from_x(d: V3) -> Q4 {
    if (-0.9999_f64) as f32 <= d[0] {
        let s = sqrt((d[0] + d[0]) + 2.0);
        let inv = 1.0 / s;
        return [-0.0, -(inv * d[2]), inv * d[1], s * 0.5];
    }
    [0.0, 1.0, 0.0, 0.0]
}

/// rotate v by q in the order used for particle+0x1DC: (c2 + v) + w*t, t = 2 (q x v)
pub fn rot_b(q: Q4, v: V3) -> V3 {
    let t = mul(cross(&q, &v), 2.0);
    let c2 = cross(&q, &t);
    [(c2[0] + v[0]) + (q[3] * t[0]), (c2[1] + v[1]) + (q[3] * t[1]), (c2[2] + v[2]) + (q[3] * t[2])]
}

/// "swing with the bone twist": q = arcFromX(dir); P = q*conj(B); T = normalize((P.xyz . a) a, P.w),
/// a = rot(q,+X); result = conj(T)*q
pub fn swing_keep_twist(dir: V3, b: Q4) -> Q4 {
    let q = arc_from_x(dir);
    let a = [
        (((-2.0 * q[1]) * q[1]) - ((2.0 * q[2]) * q[2])) + 1.0,
        ((2.0 * q[2]) * q[3]) - ((-2.0 * q[1]) * q[0]),
        ((2.0 * q[2]) * q[0]) + ((-2.0 * q[1]) * q[3]),
    ];
    let p = qmul(q, qconj(b));
    let pr = ((a[0] * p[0]) + (a[1] * p[1])) + (a[2] * p[2]);
    let mut t = [a[0] * pr, a[1] * pr, a[2] * pr, p[3]];
    let l = ((t[0] * t[0]) + (t[2] * t[2])) + ((t[1] * t[1]) + (t[3] * t[3]));
    if l >= C_1E8 {
        // rsqrtps + ONE Newton step
        let mut y = (1.0 / (l as f64).sqrt()) as f32;
        y = ((0.5 - (((y * y) * l) * 0.5)) * y) + y;
        t = [t[0] * y, t[1] * y, t[2] * y, t[3] * y];
    } else {
        t = [0.0, 0.0, 0.0, 1.0];
    }
    qmul([-t[0], -t[1], -t[2], t[3]], q)
}

/// 0x143b10010 GroupWeights(core, dtp)
pub fn fn_143b10010(core: &mut Core, dt: f32) {
    core.f65c = 0;
    for g in 0..core.f410 {
        let a;
        if core.f34b8[g] <= C_0001 {
            let time = core.f33f8[g];
            core.f34b8[g] = 0.0;
            let mut v = core.f3638[g];
            if time <= dt {
                core.f33f8[g] = 0.0;
            } else {
                v = ((v - core.f3578[g]) * (dt / time)) + core.f3578[g];
                if v >= 0.0 {
                    if v >= 1.0 {
                        v = 1.0;
                    }
                } else {
                    v = 0.0;
                }
                core.f33f8[g] = time - dt;
            }
            core.f3578[g] = v;
            a = v;
        } else {
            a = core.f3578[g];
            core.f34b8[g] = core.f34b8[g] - dt;
        }
        if a <= C_9999 || core.f3638[g] <= C_9999 {
            if a < C_0001 && core.f3638[g] < C_0001 {
                core.f8c4[g] = 0;
            }
        } else {
            let s = core.f804[g];
            if s == 3 {
                core.f804[g] = 1;
            } else if s == 4 {
                core.f804[g] = 0;
            } else if s == 2 {
                core.f804[g] = 1;
            }
        }
        if core.f804[g] > 1 {
            core.f65c = 1;
        }
    }
}

/// 0x143b11700 BoneWeights(core): the driven-bone bit set `core.f3800` and the weights
/// `core.f3810`. (The follower lists `core.f3768` are empty: nothing in setup fills them.)
pub fn fn_143b11700(core: &mut Core) {
    let n = core.f3808;
    let mut words = vec![0u32; std::cmp::max(1, (n + 31) >> 5)];
    let get = |words: &[u32], i: usize| (words[i >> 5] >> (i & 31)) & 1 != 0;
    let set = |words: &mut [u32], i: usize| words[i >> 5] |= 1 << (i & 31);
    let sc = &core.f3778;
    for p in &sc.f0 {
        let child_ok = p.f198.is_some_and(|c| sc.f0[c].f276 == 0 && sc.f0[c].fbc != 0);
        if core.f804[p.f2b8] > 1 && p.f277 == 0 && (p.f276 == 0 || child_ok) {
            set(&mut words, p.f248);
        }
    }
    let parents = &core.f3848.parents;
    for b in 1..n {
        if !get(&words, b) {
            let mut a = parents[b];
            while a != -1 {
                if get(&words, a as usize) {
                    set(&mut words, b);
                    break;
                }
                a = parents[a as usize];
            }
        }
    }
    let w = &mut core.f3810;
    w.iter_mut().for_each(|x| *x = 0.0);
    for p in &sc.f0 {
        if get(&words, p.f248) && p.f278 == 0 {
            w[p.f248] = (1.0 - core.f3578[p.f2b8]) * core.f36f8;
        }
    }
    core.f3800 = words;
}

/// 0x147506880
pub const FPS_TABLE: [f32; 5] = [20.0, 30.0, 40.0, 50.0, 60.0];

/// 0x143af5d70
pub fn nearest_bucket(fps: f32) -> usize {
    let mut best = 0;
    let mut bd = f32::INFINITY;
    for (i, table) in FPS_TABLE.iter().enumerate() {
        let d = (fps - table).abs();
        if d < bd {
            bd = d;
            best = i;
        }
    }
    best
}

pub struct TimeStep {
    pub dt_sim: f32,
    pub r: f32,
    pub dtp: f32,
    pub step: f32,
}

/// time step part of CoreUpdate 0x143b0efc0
pub fn time_step(core: &mut Core, cfg: &Cfg, dt: f32, td: f32) -> TimeStep {
    if dt > C_1E8 {
        let mut d = 0.008333334_f64 as f32;
        if dt >= d {
            d = dt;
            if dt >= 0.1_f64 as f32 {
                d = 0.1_f64 as f32;
            }
        }
        core.f382c = ((d - core.f382c) * (0.1_f64 as f32)) + core.f382c;
        let b = nearest_bucket(1.0 / core.f382c);
        if b == core.f3824 {
            if core.f3828 < 10 {
                core.f3828 += 1;
            } else {
                let (tgt, st) = (core.f3834, core.f3830);
                if tgt <= st {
                    let mut v = st - (0.002_f64 as f32);
                    if v <= tgt {
                        v = tgt;
                    }
                    core.f3830 = v;
                } else {
                    let mut v = st + (0.002_f64 as f32);
                    if tgt <= v {
                        v = tgt;
                    }
                    core.f3830 = v;
                }
            }
        } else {
            core.f3824 = b;
            core.f3828 = 0;
            core.f3834 = 1.0 / FPS_TABLE[b];
        }
    }
    let step = core.f3830;
    let base = step * core.f3838;
    let dtp = dt * td;
    if core.f3710 != 0.0 {
        let v = (core.f3710 * dtp) + core.f3708;
        core.f3708 = v;
        if (core.f3710 > 0.0 && core.f370c <= v) || (core.f3710 < 0.0 && v <= core.f370c) {
            core.f3708 = core.f370c;
            core.f3710 = 0.0;
        }
    }
    let dt_sim = (base * td) * core.f3708;
    let r = ((base * cfg.f64) * td) * core.f3708;
    TimeStep { dt_sim, r, dtp, step }
}

/// setup block of 0x143b07550: initial orientation (+0x20, +0x30, +0x1DC)
pub fn init_orientations(core: &mut Core) {
    let n = core.f3778.f0.len();
    for i in 0..n {
        let p = &core.f3778.f0;
        if p[i].f1a4 == -1 {
            let q = p[i].f120;
            core.f3778.f0[i].f20 = q;
        } else {
            let c = p[i].f1a4 as usize;
            let q = swing_keep_twist(norm_safe(sub(p[c].f0, p[i].f0)), p[i].f120);
            let p = &mut core.f3778.f0[i];
            p.f198 = Some(c);
            p.f20 = q;
        }
        let p = &mut core.f3778.f0[i];
        if p.f1a0 != -1 {
            p.f190 = Some(p.f1a0 as usize);
        }
    }
    for i in 0..n {
        let p = &core.f3778.f0;
        if let Some(c) = p[i].f198 {
            let d = sub(p[c].f0, p[i].f0);
            if ((d[0] * d[0]) + (d[1] * d[1])) + (d[2] * d[2]) < 0.01_f64 as f32 {
                if let Some(cc) = p[c].f198 {
                    let q = swing_keep_twist(norm_safe(sub(p[cc].f0, p[i].f0)), p[i].f120);
                    core.f3778.f0[i].f20 = q;
                }
            }
        }
        let up = core.f1c0[core.f3778.f0[i].f2b8];
        let p = &mut core.f3778.f0[i];
        p.f1dc = rot_b(p.f20, up);
        p.f30 = p.f20;
    }
}

/// 0x143b15520
fn anch(core: &Core, i: usize) -> bool {
    let sc = &core.f3778;
    let p = &sc.f0[i];
    if p.fbd != 0 {
        return true;
    }
    for k in 0..p.f1a8 {
        let c = sc.f40[p.f1b0[k]].f8;
        if sc.f0[c].f276 != 0 || sc.f0[c].fbd != 0 || anch(core, c) {
            return true;
        }
    }
    false
}

/// 0x143b155b0
fn mark(core: &mut Core, i: usize) {
    for k in 0..core.f3778.f0[i].f1a8 {
        let l = core.f3778.f0[i].f1b0[k];
        core.f3778.f40[l].f26 = 1;
        let c = core.f3778.f40[l].f8;
        mark(core, c);
    }
}

/// setup block: chain roots (scene.f748) and hard-length marking
pub fn build_roots(core: &mut Core) {
    core.f3778.f748.clear();
    for i in 0..core.f3778.f0.len() {
        let sc = &core.f3778;
        let p = &sc.f0[i];
        if p.f277 != 0 {
            continue;
        }
        match p.f190 {
            None => {
                for k in 0..p.f1a8 {
                    if sc.f0[sc.f40[p.f1b0[k]].f8].f276 == 0 {
                        if p.f276 != 0 || p.f238.is_some() || p.f240.is_some() || p.fbd != 0 {
                            core.f3778.f748.push(i);
                        }
                        break;
                    }
                }
            }
            Some(par) => {
                let child_dynamic = p.f198.is_some_and(|c| sc.f0[c].f276 == 0);
                if (p.fbc == 0 || (p.f276 != 0 && sc.f0[par].f276 != 0)) && ((child_dynamic && (p.f276 != 0 || p.f238.is_some() || p.f240.is_some())) || (p.f276 == 0 && p.fbd != 0)) {
                    core.f3778.f0[i].fbc = 0;
                    core.f3778.f748.push(i);
                }
            }
        }
    }
    if core.f658 >= 2400 && core.f664 == 0 {
        for i in core.f3778.f748.clone() {
            if !anch(core, i) {
                mark(core, i);
            }
        }
    }
}
