//! `tools/lib/bonamik/chain.js`: instruction-level transcription (KH3.exe) of
//!   fn_143b0c600  WriteBack (particle -> bone transform)
//!   fn_143b13b70  ChainWriteBack (recursive, twist removal against the target quat, wind direction p+0x1DC)
//!   fn_143b192a0  ChainLink (recursive ParentChild constraint; calls the cone solve)
//!   fn_143af3f70  JointTwist
//!   fn_143aec770 / fn_143ae7de0  link record setup
//!   blk_143b0a327 joint-twist sign / rest angle pass of the core setup (block inside 0x143b07550)
//!
//! Not ported: `fn_143b15520` / `fn_143b155b0` / `blk_143b0a2af` (the driver uses the same pass
//! in `core::build_roots`) and `fn_143b1a4d0` (the driver uses the copy in kdi.js). The capacity
//! test `scene.f790 === idx` compares against a field the driver never sets and is not reproduced.

use super::bnm_eval::LinkDesc;
use super::cone;
use super::m::{cross, dot, qconj, qmul, qrot, rsqrt, sqrt, Q4, V3};
use super::records::{Core, Link, Scene};

const C_1E8: f32 = 9.99999993922529e-9_f64 as f32; // 0x145C9C488 / immediate 0x322BCC77
const C_1E4: f32 = 0.00009999999747378752_f64 as f32; // 0x145CF35A8
const C_001: f32 = 0.009999999776482582_f64 as f32; // 0x145CADFC4
const C_M9999: f32 = -0.9998999834060669_f64 as f32; // 0x14750BAA8
const C_SPD: f32 = 0.004999999888241291_f64 as f32; // 0x145E2AB68
const C_008: f32 = 0.07999998331069946_f64 as f32; // 0x14750BA94 (0x3da3d708, NOT float(0.08))
const C_092: f32 = 0.9200000166893005_f64 as f32; // 0x14698F900
const C_084: f32 = 0.8400000333786011_f64 as f32; // 0x14750BA9C
const C_PI: f32 = 3.1415927410125732_f64 as f32; // 0x145C9C4F8
const C_HPI: f32 = 1.5707963705062866_f64 as f32; // 0x145C9C4F0
const C_2PI: f32 = 6.2831854820251465_f64 as f32; // 0x145C9C508
const C_INV2PI: f32 = 0.15915493667125702_f64 as f32; // 0x145C9C4C4
const AC0: f32 = 1.5707287788391113_f64 as f32;
const AC1: f32 = 0.2121143937110901_f64 as f32;
const AC2: f32 = 0.07426100224256516_f64 as f32;
const AC3: f32 = 0.018729299306869507_f64 as f32;
const S1: f32 = 2.388985897994189e-8_f64 as f32;
const S2: f32 = 0.0000027525561563379597_f64 as f32;
const S3: f32 = 0.00019840874301735312_f64 as f32;
const S4: f32 = 0.008333330973982811_f64 as f32;
const S5: f32 = 0.1666666716337204_f64 as f32;
const K1: f32 = 2.605161455448979e-7_f64 as f32;
const K2: f32 = 0.0000247604948526714_f64 as f32;
const K3: f32 = 0.0013888378161936998_f64 as f32;
const K4: f32 = 0.04166663810610771_f64 as f32;

/// SSE quaternion normalise with identity fallback
pub fn qnorm(q: Q4) -> Q4 {
    let ss = ((q[0] * q[0]) + (q[2] * q[2])) + ((q[1] * q[1]) + (q[3] * q[3]));
    if !(C_1E8 <= ss) {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let y = rsqrt(ss);
    [y * q[0], y * q[1], y * q[2], y * q[3]]
}

/// scalar vector normalise block: l2 == 1 (or unordered) -> unchanged; l2 < 1e-8 -> ZeroVector
pub fn vnorm(d: V3) -> V3 {
    let l2 = dot(d, d);
    if l2 == 1.0 || l2.is_nan() {
        return [d[0], d[1], d[2]];
    }
    if !(l2 >= C_1E8) {
        return [0.0, 0.0, 0.0];
    }
    let y = rsqrt(l2);
    [d[0] * y, d[1] * y, d[2] * y]
}

/// shortest arc from +X to the unit vector d. x component is -0.0.
pub fn arc_x(d: V3) -> Q4 {
    if !(d[0] >= C_M9999) {
        return [0.0, 1.0, 0.0, 0.0];
    }
    let s = sqrt((d[0] + d[0]) + 2.0);
    let inv = 1.0 / s;
    [-0.0, -(d[2] * inv), d[1] * inv, s * 0.5]
}

/// rot(q, +X) as coded
pub fn axis_x(q: Q4) -> V3 {
    let m2y = (-q[1]) + (-q[1]);
    let z2 = q[2] + q[2];
    [((m2y * q[1]) - (z2 * q[2])) + 1.0, (z2 * q[3]) - (m2y * q[0]), (m2y * q[3]) + (z2 * q[0])]
}

/// twist of (q * conj(ref)) about q's X axis, normalised (identity fallback)
pub fn twist_of(q: Q4, r: Q4) -> Q4 {
    let d = qmul(q, qconj(r));
    let a = axis_x(q);
    let k = ((a[1] * d[1]) + (a[0] * d[0])) + (a[2] * d[2]);
    qnorm([k * a[0], k * a[1], k * a[2], d[3]])
}

/// q with that twist removed: conj(Tw) * q (NOT re-normalised)
pub fn untwist(q: Q4, r: Q4) -> Q4 {
    qmul(qconj(twist_of(q, r)), q)
}

/// polynomial acos; c already clamped to [-1,1]
pub fn acos_p(c: f32) -> f32 {
    let a = c.abs();
    let a2 = a * a;
    let poly = ((AC0 - (a * AC1)) + (a2 * AC2)) - ((a2 * a) * AC3);
    let r = sqrt(1.0 - a) * poly;
    if c >= 0.0 {
        r
    } else {
        C_PI - r
    }
}

/// clamp block: comiss c,-1 ; jae -> minss c,1 ; else c = -1
pub fn clamp11(c: f32) -> f32 {
    if c >= -1.0 {
        if c < 1.0 {
            c
        } else {
            1.0
        }
    } else {
        -1.0
    }
}

/// FMath::SinCos shape (143af4ad0..143af4bfd) -> [sin, cos]
pub fn sincos(x: f32) -> [f32; 2] {
    let mut q = x * C_INV2PI;
    q = if x >= 0.0 { q + 0.5 } else { q - 0.5 };
    let mut y = x - (((q as f64).trunc() as f32) * C_2PI);
    let sign: f32;
    if y > C_HPI {
        y = C_PI - y;
        sign = -1.0;
    } else if !(y >= -C_HPI) {
        y = (-C_PI) - y;
        sign = -1.0;
    } else {
        sign = 1.0;
    }
    let y2 = y * y;
    let mut s = S2 - (y2 * S1);
    s = (s * y2) - S3;
    s = (s * y2) + S4;
    s = (s * y2) - S5;
    s = (s * y2) + 1.0;
    s = s * y;
    let mut c = K2 - (y2 * K1);
    c = (c * y2) - K3;
    c = (c * y2) + K4;
    c = (c * y2) - 0.5;
    c = (c * y2) + 1.0;
    c = c * sign;
    [s, c]
}

/// 0x143b0c600 WriteBack(rcx core)
pub fn fn_143b0c600(core: &mut Core) {
    let bones = &mut core.f3848.bones;
    let scene = &mut core.f3778;
    for i in 0..scene.f0.len() {
        let p = &scene.f0[i];
        if core.f804[p.f2b8] < 2 {
            continue;
        }
        if p.f277 != 0 {
            continue;
        }
        let ch = p.f198;
        if p.f276 == 0 {
            if ch.is_none() {
                // dynamic tip (143b0c70b)
                let (bi, pos, par, f274) = (p.f248, p.f0, p.f190, p.f274);
                scene.f0[i].f20 = bones[bi].q;
                bones[bi].t = [pos[0], pos[1], pos[2]];
                let Some(par) = par else { continue };
                if f274 != 0 {
                    continue;
                }
                // tip bone rotation := normalised PARENT bone rotation
                bones[bi].q = qnorm(bones[scene.f0[par].f248].q);
                continue;
            }
        } else {
            let Some(c) = ch else { continue };
            if scene.f0[c].f276 != 0 {
                continue;
            }
            if scene.f0[c].fbc == 0 {
                continue;
            }
        }
        // 143b0c842
        let ch = ch.expect("a child particle");
        let pp = p.f0;
        let cp = scene.f0[ch].f0;
        let mut d = [cp[0] - pp[0], cp[1] - pp[1], cp[2] - pp[2]];
        let l2 = ((d[1] * d[1]) + (d[0] * d[0])) + (d[2] * d[2]);
        if !(l2 >= C_001) {
            if let Some(gc) = scene.f0[ch].f198 {
                let gp = scene.f0[gc].f0;
                d = [gp[0] - pp[0], gp[1] - pp[1], gp[2] - pp[2]];
            }
        }
        d = vnorm(d);
        scene.f0[ch].f1d0 = [d[0], d[1], d[2]];
        let q = qnorm(arc_x(d)); // store at 143b0c9fa, then normalised
        let p = &mut scene.f0[i];
        p.f20 = q;
        let b = &mut bones[p.f248];
        b.t = [p.f0[0], p.f0[1], p.f0[2]];
        b.q = p.f20;
    }
}

/// 0x143b13b70 ChainWriteBack(rcx core, rdx particle, r8 bones, r9 &parentQuat)
pub fn fn_143b13b70(core: &mut Core, pi: usize, _parent_quat: Q4) {
    let p = &core.f3778.f0[pi];
    if p.f275 != 0 {
        return; // no recursion into the children either
    }
    let b = core.f3848.bones[p.f248].q;
    let v = p.f40;
    let sp = sqrt(((v[0] * v[0]) + (v[1] * v[1])) + (v[2] * v[2])) * C_SPD;
    let _one_minus_speed = 1.0 - (if sp < 1.0 { sp } else { 1.0 }); // used only in the branches that are dead for Sora
    let q: Q4;
    if p.f31c == 0 && p.f31d == 0 {
        q = untwist(b, p.f160); // 143b13c5f..143b13f5e
    } else {
        // (ER69) NOT TRANSCRIBED, a FALLBACK instead of a stop (an outfit's asset reaches it; Sora's never): 143b13f63..143b14b2e (ground-attach / tag-11 smoothing): the plain untwist
        super::unported(3);
        q = untwist(b, p.f160);
    }
    if p.f31c != 0 {
        let n = p.f88;
        if ((n[0] * n[0]) + (n[1] * n[1])) + (n[2] * n[2]) > C_1E4 {
            // (ER69) NOT TRANSCRIBED, a FALLBACK instead of a stop (an outfit's asset reaches it; Sora's never): 143b14b79..143b1532e (ground-attach tilt): none
            super::unported(4);
        }
    }
    let up = core.f1c0[p.f2b8];
    let bi = p.f248;
    core.f3778.f0[pi].f1dc = qrot(q, up); // 143b1535a..143b154ad
    core.f3848.bones[bi].q = q;
    for i in 0..core.f3778.f0[pi].f1a8 {
        let child = core.f3778.f40[core.f3778.f0[pi].f1b0[i]].f8;
        fn_143b13b70(core, child, q);
    }
}

/// 0x143b192a0 ChainLink(xmm0 r, rdx link, r8 cfg = core+0x658)
pub fn fn_143b192a0(r: f32, li: usize, core: &mut Core) {
    let link: Link = core.f3778.f40[li];
    let p1 = link.f0;
    if let Some(c) = link.f28 {
        let g = core.f3778.f0[p1].f2b8;
        let (inner, outer) = (core.fb94[g], core.fc54[g]);
        cone::fn_143b166d0(&mut core.f3778, r, c, inner, outer, true);
    } else {
        let sc: &mut Scene = &mut core.f3778;
        let p2 = link.f8;
        let pp = sc.f0[p1].f190;
        let rq: Q4 = match pp {
            Some(pp) if sc.f0[p1].fbc != 0 => {
                if sc.f0[pp].f190.is_none() {
                    sc.f0[pp].f160
                } else {
                    sc.f0[pp].f20
                }
            }
            _ => sc.f0[p1].f160,
        };
        if link.f26 != 0 {
            // hard length (143b19366)
            let (a, b) = (sc.f0[p1].f0, sc.f0[p2].f0);
            let (mut dx, mut dy, mut dz) = (b[0] - a[0], b[1] - a[1], b[2] - a[2]);
            let mut l2 = ((dy * dy) + (dx * dx)) + (dz * dz);
            if !(l2 >= 0.0) {
                sc.f0[p1].f0 = [0.0, 0.0, 0.0];
                sc.f0[p2].f0 = [0.0, 0.0, 0.0];
                dx = 0.0;
                dy = 0.0;
                dz = 0.0;
                l2 = 0.0;
            }
            let len = sqrt(l2);
            if len > 0.0 {
                let inv = 1.0 / len;
                dx = dx * inv;
                dy = dy * inv;
                dz = dz * inv;
            }
            let k = link.f20 - len;
            dx = dx * k;
            dy = dy * k;
            dz = dz * k;
            let (a, b) = (sc.f0[p1].f0, sc.f0[p2].f0);
            if sc.f0[p1].f276 != 0 {
                if sc.f0[p2].f276 == 0 {
                    sc.f0[p2].f0 = [dx + b[0], dy + b[1], dz + b[2]];
                }
            } else if sc.f0[p2].f276 != 0 {
                sc.f0[p1].f0 = [a[0] - dx, a[1] - dy, a[2] - dz];
            } else {
                sc.f0[p1].f0 = [a[0] - (dx * C_008), a[1] - (dy * C_008), a[2] - (dz * C_008)];
                sc.f0[p2].f0 = [(dx * C_092) + b[0], (dy * C_092) + b[1], (dz * C_092) + b[2]];
                let o = sc.f0[p1].f58;
                sc.f0[p1].f58 = [(dx * C_084) + o[0], (dy * C_084) + o[1], (dz * C_084) + o[2]];
            }
        }
        let (a, b) = (sc.f0[p1].f0, sc.f0[p2].f0);
        let d = vnorm([b[0] - a[0], b[1] - a[1], b[2] - a[2]]);
        let q = arc_x(d);
        let qn = untwist(q, rq);
        if sc.f0[p1].f190.is_some() && sc.f0[p1].fbc != 0 {
            // 143b19548
            sc.f0[p2].f20 = qn;
            if sc.f0[p1].f1a8 == 1 {
                sc.f0[p1].f20 = qn;
            }
        } else {
            // 143b1992f
            sc.f0[p1].f20 = qn;
            sc.f0[p2].f20 = qn;
        }
    }
    let child = link.f8;
    for i in 0..core.f3778.f0[child].f1a8 {
        let l = core.f3778.f0[child].f1b0[i];
        fn_143b192a0(r, l, core);
    }
}

/// 0x143af3f70 JointTwist(rcx core)
pub fn fn_143af3f70(core: &mut Core) {
    for i in 0..core.f3778.f0.len() {
        let scene = &core.f3778;
        let bones = &core.f3848.bones;
        let p = &scene.f0[i];
        let g = p.f2b8;
        if core.f804[g] < 2 {
            continue;
        }
        let n = p.f310;
        if n == 0 {
            continue;
        }
        let par = p.f190;
        let mut d: V3;
        let mut u: V3;
        match par {
            Some(pa) if scene.f0[pa].f31c == 0 => {
                let pap = &scene.f0[pa];
                d = [p.f0[0] - pap.f0[0], p.f0[1] - pap.f0[1], p.f0[2] - pap.f0[2]];
                u = qrot(bones[pap.f248].q, core.f1c0[g]);
            }
            _ => {
                let Some(ch) = p.f198 else { continue };
                let chp = &scene.f0[ch];
                if chp.f31c != 0 {
                    continue;
                }
                d = [chp.f0[0] - p.f0[0], chp.f0[1] - p.f0[1], chp.f0[2] - p.f0[2]];
                u = qrot(bones[p.f248].q, core.f1c0[g]);
            }
        }
        d = vnorm(d);
        u = vnorm(u);
        let mut s: V3 = [0.0, 0.0, 0.0];
        for k in 0..n {
            let nb = &scene.f0[scene.f0[p.f2e0[k]].f24c]; // through the neighbour's particle index
            let e = vnorm([nb.f0[0] - p.f0[0], nb.f0[1] - p.f0[1], nb.f0[2] - p.f0[2]]);
            let sg = p.f300[k] as f32; // cvtsi2ss of the int sign
            let c = cross(&d, &e);
            s = [s[0] + (c[0] * sg), s[1] + (c[1] * sg), s[2] + (c[2] * sg)];
        }
        s = vnorm(s);
        let has_child = p.f198.is_some();
        let (own_bone, own_f275) = (p.f248, p.f275);
        let rest = p.f318;
        match par {
            Some(pa) => core.f3778.f0[pa].f1d0 = s,
            None => core.f3778.f0[i].f1d0 = s,
        }
        if !has_child {
            core.f3778.f0[i].f1d0 = s;
        }
        let kk = dot(s, d);
        s = vnorm([s[0] - (d[0] * kk), s[1] - (d[1] * kk), s[2] - (d[2] * kk)]);
        let mut c = dot(s, u);
        let x = cross(&u, &s);
        let t = ((x[0] * d[0]) + (x[1] * d[1])) + (x[2] * d[2]);
        c = clamp11(c);
        let ang = if t >= 0.0 { acos_p(c) } else { -acos_p(c) };
        let delta = ang - rest;
        if !(delta.abs() > C_1E4) {
            continue;
        }
        let sc = sincos(delta * 0.5);
        let rq = [d[0] * sc[0], d[1] * sc[0], d[2] * sc[0], sc[1]];
        let bi;
        match par {
            Some(pa) => {
                if core.f3778.f0[pa].f275 != 0 {
                    continue;
                }
                bi = core.f3778.f0[pa].f248;
            }
            None => {
                if own_f275 != 0 {
                    continue;
                }
                bi = own_bone;
            }
        }
        let bones = &mut core.f3848.bones;
        bones[bi].q = qnorm(qmul(rq, bones[bi].q));
    }
}

/// 0x143ae7de0 link record init
#[allow(clippy::too_many_arguments)]
pub fn fn_143ae7de0(sc: &mut Scene, p1: usize, p2: usize, link_str: f32, parent_str: f32, is_pc: u8, joint_twist: u8) -> Link {
    let (a, b) = (sc.f0[p1].f0, sc.f0[p2].f0);
    let (dz, dy, dx) = (a[2] - b[2], a[1] - b[1], a[0] - b[0]);
    let l = Link { f0: p1, f8: p2, f10: -1, f14: link_str, f18: parent_str, f20: sqrt(((dy * dy) + (dx * dx)) + (dz * dz)), f24: is_pc, f26: 0, f28: None, f30: None };
    if is_pc == 0 && joint_twist != 0 {
        if sc.f0[p1].f310 < 4 {
            let p = &mut sc.f0[p1];
            p.f2e0[p.f310] = p2;
            p.f300[p.f310] = 0;
            p.f310 += 1;
        }
        if sc.f0[p2].f310 < 4 {
            let p = &mut sc.f0[p2];
            p.f2e0[p.f310] = p1;
            p.f300[p.f310] = 0;
            p.f310 += 1;
        }
    }
    if l.f24 != 0 {
        sc.f0[p2].fbc = 1;
        let (qc, qp) = (sc.f0[p2].f20, sc.f0[p1].f20);
        let d = qmul(qc, qconj(qp));
        let a = axis_x(qc);
        let k = ((a[0] * d[0]) + (a[1] * d[1])) + (a[2] * d[2]);
        let tw = [a[0] * k, a[1] * k, a[2] * k, d[3]];
        let ss = (((tw[1] * tw[1]) + (tw[0] * tw[0])) + (tw[2] * tw[2])) + (tw[3] * tw[3]);
        let n = qnorm(if ss > C_1E4 { tw } else { [0.0, 0.0, 0.0, 1.0] }); // identity = 0x14871E410
        let w = n[3];
        let a = w.abs();
        let a2 = a * a;
        let mut ang = sqrt(1.0 - a) * (((AC0 - (a * AC1)) + (a2 * AC2)) - ((a2 * a) * AC3));
        if !(w >= 0.0) {
            ang = C_PI - ang;
        }
        sc.f0[p2].f314 = ang + ang;
    }
    l
}

/// 0x143aec770 add link (rcx scene, rdx LinkDesc, r8d particle index base) -> link index
pub fn fn_143aec770(sc: &mut Scene, desc: &LinkDesc, base: i32) -> usize {
    let idx = sc.f40.len();
    let p1 = (desc.f20 + base) as usize;
    let p2 = (desc.f24 + base) as usize;
    let ty = desc.f28;
    let is_pc: u8 = if ty == 1 { 1 } else { 0 };
    let mut l = fn_143ae7de0(sc, p1, p2, desc.f30, desc.f34, is_pc, desc.f2a);
    if ty == 1 {
        if sc.f0[p1].f1a8 < 4 {
            let p = &mut sc.f0[p1];
            p.f1b0[p.f1a8] = idx;
            p.f1a8 += 1;
        }
        if sc.f0[p1].f198 != Some(p2) {
            sc.f0[p1].f198 = Some(p2);
            let (a, b) = (sc.f0[p1].f0, sc.f0[p2].f0);
            let d = vnorm([b[0] - a[0], b[1] - a[1], b[2] - a[2]]);
            let q = arc_x(d);
            let aq = sc.f0[p1].f120; // read before the store to +0x20
            sc.f0[p1].f20 = untwist(q, aq);
        }
    }
    l.f10 = desc.f68;
    sc.f40.push(l);
    idx
}

/// block 143b0a327..143b0ac04 of the core setup 0x143b07550: joint-twist signs (+0x300) and rest angle (+0x318)
pub fn blk_143b0a327(core: &mut Core, count: usize) {
    for i in 0..count {
        let sc = &core.f3778;
        let p = &sc.f0[i];
        if p.f310 == 0 {
            continue;
        }
        let g = p.f2b8;
        let mut d: V3;
        let u: V3;
        if let Some(par) = p.f190 {
            let pa = &sc.f0[par];
            u = qrot(pa.f120, core.f1c0[g]);
            d = [p.f0[0] - pa.f0[0], p.f0[1] - pa.f0[1], p.f0[2] - pa.f0[2]];
        } else {
            let Some(ch) = p.f198 else { continue };
            let chp = &sc.f0[ch];
            u = qrot(p.f120, core.f1c0[g]);
            d = [chp.f0[0] - p.f0[0], chp.f0[1] - p.f0[1], chp.f0[2] - p.f0[2]];
        }
        d = vnorm(d); // U is NOT normalised here
        let mut s: V3 = [0.0, 0.0, 0.0];
        let mut signs = p.f300;
        for k in 0..p.f310 {
            let nb = &sc.f0[p.f2e0[k]]; // direct pointer here
            let e = vnorm([nb.f0[0] - p.f0[0], nb.f0[1] - p.f0[1], nb.f0[2] - p.f0[2]]);
            let c = cross(&d, &e);
            let t = ((c[0] * u[0]) + (c[1] * u[1])) + (c[2] * u[2]);
            let sg: f32;
            if t >= 0.0 {
                signs[k] = 1;
                sg = 1.0;
            } else {
                signs[k] = -1;
                sg = -1.0;
            }
            s = [s[0] + (c[0] * sg), s[1] + (c[1] * sg), s[2] + (c[2] * sg)];
        }
        s = vnorm(s);
        let kk = dot(s, d);
        s = vnorm([s[0] - (d[0] * kk), s[1] - (d[1] * kk), s[2] - (d[2] * kk)]);
        let mut c = dot(s, u);
        let x = cross(&u, &s);
        let t = ((x[0] * d[0]) + (d[1] * x[1])) + (d[2] * x[2]);
        c = clamp11(c);
        let rest = if t >= 0.0 { acos_p(c) } else { -acos_p(c) };
        let p = &mut core.f3778.f0[i];
        p.f300 = signs;
        p.f318 = rest;
    }
}
