//! `tools/lib/bonamik/reset.js`: frame transforms, teleport check, reset, pre-step, SetFromBone,
//! LocalForce (fast-motion mode 1). Transcribed from the disassembly of KH3.exe.
//!
//! The plane-group loop of PreStep (143aee4d7) runs over `scene.f20`, which the driver never
//! fills (count 0): it is not reproduced.

use super::cone;
use super::m::{add, js_min, qmul, qrot, sqrt, Q4, V3, Xf};
use super::records::Core;

const ZERO_VEC: V3 = [0.0, 0.0, 0.0]; // 0x149B5C798 (FVector::ZeroVector)
const IDENT_Q: Q4 = [0.0, 0.0, 0.0, 1.0]; // 0x149B5C560
const IDENT_T: V3 = [0.0, 0.0, 0.0]; // 0x149B5C570
const IDENT_S: V3 = [1.0, 1.0, 1.0]; // 0x149B5C580
const SMALL: f32 = 1e-8_f64 as f32; // 0x149D52920 lanes; scalar copy 0x145C9C488 = 0x322BCC77

#[inline]
fn mulv(a: V3, b: V3) -> V3 {
    [a[0] * b[0], a[1] * b[1], a[2] * b[2]]
}

/// rcpps + two Newton steps r = (r + r) - (r*r)*s (rcpps itself emulated by a correctly rounded 1/s)
pub fn recip_accurate(s: f32) -> f32 {
    let mut r = 1.0 / s;
    r = (r + r) - ((r * r) * s);
    r = (r + r) - ((r * r) * s);
    r
}

/// stock FTransform::GetSafeScaleReciprocal(Scale, SmallNumber): lane = |s| <= 1e-8 ? 0 : 1/s
pub fn safe_scale_recip(s: V3) -> V3 {
    let one = |x: f32| if x.abs() <= SMALL { 0.0 } else { recip_accurate(x) };
    [one(s[0]), one(s[1]), one(s[2])]
}

fn any_abs_greater_small(s: V3) -> bool {
    s[0].abs() > SMALL || s[1].abs() > SMALL || s[2].abs() > SMALL
}

/// stock FTransform::Inverse (SSE shape)
pub fn xf_inverse(q: Q4, t: V3, s: V3) -> Xf {
    if !any_abs_greater_small(s) {
        return Xf { q: IDENT_Q, t: IDENT_T, s: IDENT_S };
    }
    let iq = [q[0] * -1.0, q[1] * -1.0, q[2] * -1.0, q[3] * 1.0]; // mulps QINV_SIGN_MASK (0x149D52910)
    let is = safe_scale_recip(s);
    let r = qrot(iq, mulv(t, is));
    Xf { q: iq, t: [0.0 - r[0], 0.0 - r[1], 0.0 - r[2]], s: is }
}

/// stock FTransform::Multiply(A, B) = "A then B", quaternion path only
pub fn xf_mul(a: &Xf, bq: Q4, bt: V3, bs: V3, at: &str) -> Xf {
    if js_min(a.s[0], bs[0]) < 0.0 || js_min(a.s[1], bs[1]) < 0.0 || js_min(a.s[2], bs[2]) < 0.0 {
        // (ER69) NOT TRANSCRIBED, a FALLBACK instead of a stop (an outfit's asset reaches it; Sora's never): negative scale -> the matrix path: the quaternion path
        let _ = at;
        super::unported(16);
    }
    Xf { q: qmul(bq, a.q), t: add(qrot(bq, mulv(a.t, bs)), bt), s: mulv(a.s, bs) }
}

/// 0x143b1da00 FrameTransforms(core)
pub fn fn_143b1da00(core: &mut Core) {
    core.f30 = core.f0;
    core.f40 = core.f10;
    core.f50 = core.f20; // previous ComponentToWorld
    let w = core.f3848.world;
    core.f0 = w.q;
    core.f10 = w.t;
    core.f20 = w.s; // current ComponentToWorld
    core.f190 = core.f160;
    core.f1a0 = core.f170;
    core.f1b0 = core.f180; // previous root-bone delta
    let inv = xf_inverse(core.f130, core.f140, core.f150); // inverse of the reference root bone
    let b = core.f3848.bones[0]; // owner.vf88() = 0
    let d = xf_mul(&b, inv.q, inv.t, inv.s, "143b1dc1e..143b1dfc9");
    core.f160 = d.q;
    core.f170 = d.t;
    core.f180 = d.s; // root-bone delta (component space)
    core.f100 = core.f0;
    core.f110 = core.f10;
    core.f120 = core.f20; // component -> world (copy)
    let wi = xf_inverse(core.f0, core.f10, core.f20);
    core.fd0 = wi.q;
    core.fe0 = wi.t;
    core.ff0 = wi.s; // world -> component
    core.fa0 = core.f70;
    core.fb0 = core.f80;
    core.fc0 = core.f90; // previous "root world" transform
    let mut sc = d.s;
    if core.f60 == -1 || core.f660 != 2 {
        core.f70 = core.f160;
        core.f80 = core.f170;
    } else {
        // LSS mode only (dead for Sora)
        let r = core.f3848.bones[core.f60 as usize];
        let w = xf_mul(&r, core.f0, core.f10, core.f20, "143b1e21d..143b5cc");
        core.f70 = w.q;
        core.f80 = w.t;
        sc = w.s;
    }
    core.f90 = sc;
}

/// 0x143b00530 ResetFrameTransforms(core, bones)
pub fn fn_143b00530(core: &mut Core) {
    let w = core.f3848.world;
    core.f100 = w.q;
    core.f110 = w.t;
    core.f120 = w.s;
    let wi = xf_inverse(core.f100, core.f110, core.f120);
    core.fd0 = wi.q;
    core.fe0 = wi.t;
    core.ff0 = wi.s;
    let inv = xf_inverse(core.f130, core.f140, core.f150);
    let b = core.f3848.bones[0]; // owner.vf88() = 0
    let d = xf_mul(&b, inv.q, inv.t, inv.s, "143b007f7..143b00bb0");
    core.f160 = d.q;
    core.f170 = d.t;
    core.f180 = d.s;
    core.f190 = d.q;
    core.f1a0 = d.t;
    core.f1b0 = d.s;
    let mut sc = d.s;
    if core.f60 == -1 || core.f660 != 2 {
        core.f70 = d.q;
        core.f80 = d.t;
    } else {
        // LSS mode only (dead for Sora)
        let r = core.f3848.bones[core.f60 as usize];
        let w = xf_mul(&r, core.f0, core.f10, core.f20, "143b00cc0..143b010ac");
        core.f70 = w.q;
        core.f80 = w.t;
        sc = w.s;
    }
    core.f90 = sc;
    core.fa0 = core.f70;
    core.fb0 = core.f80;
    core.fc0 = core.f90;
}

/// 0x143af9090 (core, bones): world transform cur = prev, reference root bone = current, then 0x143b00530
pub fn fn_143af9090(core: &mut Core) {
    let w = core.f3848.world;
    core.f0 = w.q;
    core.f10 = w.t;
    core.f20 = w.s;
    core.f30 = core.f0;
    core.f40 = core.f10;
    core.f50 = core.f20;
    let b = core.f3848.bones[0]; // owner.vf88() = 0
    core.f130 = b.q;
    core.f140 = b.t;
    core.f150 = b.s;
    fn_143b00530(core);
}

/// 0x143afaf80 TeleportCheck(core)
pub fn fn_143afaf80(core: &mut Core) {
    let forced = core.f33e9;
    core.f33e9 = 0;
    let neg = |a: V3, b: V3| js_min(a[0], b[0]) < 0.0 || js_min(a[1], b[1]) < 0.0 || js_min(a[2], b[2]) < 0.0;
    if neg(core.f1b0, core.f50) {
        // (ER69) NOT TRANSCRIBED, a FALLBACK instead of a stop (an outfit's asset reaches it; Sora's never): 143afb022..143afb3df (negative scale): the quaternion path
        super::unported(17);
    }
    let prev = add(qrot(core.f30, mulv(core.f1a0, core.f50)), core.f40);
    if neg(core.f180, core.f20) {
        // (ER69) NOT TRANSCRIBED, a FALLBACK instead of a stop (an outfit's asset reaches it; Sora's never): 143afb46a..143afb79a (negative scale): the quaternion path
        super::unported(18);
    }
    let cur = add(qrot(core.f0, mulv(core.f170, core.f20)), core.f10);
    if forced == 0 {
        let (dx, dy, dz) = (cur[0] - prev[0], cur[1] - prev[1], cur[2] - prev[2]);
        let d2 = ((dy * dy) + (dx * dx)) + (dz * dz);
        if !(d2 >= core.f371c) {
            return; // comiss + setae: NaN -> no teleport
        }
    }
    core.f33f4 = 1;
    let n = core.f33f0;
    if n > 0 && n > core.f33ec {
        core.f33ea = 1;
        core.f33ec = n;
    }
}

/// 0x143b01210 Reset(core, hard)
pub fn fn_143b01210(core: &mut Core, hard: u8) {
    core.f3778.f7ac = 1;
    fn_143af9090(core);
    let bones = &core.f3848.bones;
    if hard != 0 {
        for p in core.f3778.f0.iter_mut() {
            let b = &bones[p.f248];
            p.f0 = add(qrot(b.q, mulv(p.f170, b.s)), b.t);
            p.f40 = ZERO_VEC;
            p.f120 = b.q;
            p.f130 = b.t;
            p.f140 = b.s;
            p.f20 = qmul(b.q, p.f180);
        }
    }
    for p in core.f3778.f0.iter_mut() {
        p.fc = p.f0;
        p.f4c = p.f40;
        p.ff0 = p.f120;
        p.f100 = p.f130;
        p.f110 = p.f140;
        p.f30 = p.f20;
    }
}

/// 0x143b0e790 (core): shift the animated bone transform history of every particle (every CoreUpdate)
pub fn fn_143b0e790(core: &mut Core) {
    let bones = &core.f3848.bones;
    for p in core.f3778.f0.iter_mut() {
        p.ff0 = p.f120;
        p.f100 = p.f130;
        p.f110 = p.f140;
        let b = &bones[p.f248];
        p.f120 = b.q;
        p.f130 = b.t;
        p.f140 = b.s;
    }
}

/// 0x143afd380 ReinitializeSimulationPose(core), run by CoreUpdate when core+0x33F6.
/// Loop 2 (cone rest data, 143afd4b0..143afe0a0) is NOT transcribed in the JS. Not reachable for
/// the player unless the native / notify is used.
pub fn fn_143afd380(core: &mut Core) {
    let bones = &core.f3848.bones;
    let scene = &mut core.f3778;
    for p in scene.f0.iter_mut() {
        // loop 1
        p.fc = p.f0;
        p.f30 = p.f20;
        let b = &bones[p.f248];
        p.f20 = b.q;
        p.f0 = add(qrot(b.q, mulv(p.f170, b.s)), b.t);
        p.f120 = b.q;
        p.f130 = b.t;
        p.f140 = b.s;
        p.ff0 = b.q;
        p.f100 = p.f130;
        p.f110 = p.f140;
    }
    if !scene.f30.is_empty() {
        // loop 2
        // (ER69) NOT TRANSCRIBED, a FALLBACK instead of a stop (an outfit's asset reaches it; Sora's never): 143afd4b0..143afe0a0 (cone rest data from the current pose): kept from the setup
        super::unported(19);
    }
    for i in 0..scene.f40.len() {
        // loop 3: link rest lengths
        let (a, b) = (scene.f0[scene.f40[i].f0].f0, scene.f0[scene.f40[i].f8].f0);
        let (dx, dy, dz) = (b[0] - a[0], b[1] - a[1], b[2] - a[2]);
        scene.f40[i].f20 = sqrt(((dy * dy) + (dx * dx)) + (dz * dz));
    }
    for p in scene.f0.iter_mut() {
        // loop 4: positions back to what they were
        p.f0 = p.fc;
    }
}

/// 0x143aee110 SetFromBone(core, bones, p)
pub fn fn_143aee110(core: &mut Core, pi: usize) {
    let p = &mut core.f3778.f0[pi];
    let b = &core.f3848.bones[p.f248];
    let pos = add(qrot(b.q, mulv(p.f170, b.s)), b.t);
    p.f0 = pos;
    let q = qmul(b.q, p.f180);
    p.f20 = q;
    p.f160 = q;
    if core.f33f4 != 0 {
        p.fc = pos;
        p.f4c = p.f40;
        p.f30 = q;
    }
}

/// The LocalForce argument struct ("ctx", built on the stack at rbp-80h in PreStep). `f20` / `f28`
/// (previous / current ComponentToWorld), `f30`, `f38` point into the core and are read from it.
#[derive(Clone, Copy)]
pub struct LocalForceCtx {
    /// chain root particle R
    pub f0: usize,
    /// dq
    pub f8: Q4,
    /// componentDeltaTrans
    pub f10: V3,
    /// componentDeltaRot
    pub f18: Q4,
    /// reset byte (copy of core+0x33F4)
    pub f40: u8,
    /// r
    pub f44: f32,
}

/// 0x143aeaf80 LocalForce(ctx, p)
pub fn fn_143aeaf80(core: &mut Core, ctx: &LocalForceCtx, pi: usize) {
    if core.f3778.f0[pi].f276 != 0 {
        return;
    }
    let s = core.fd14[core.f3778.f0[pi].f2b8]; // [ (core+0x658) + group*4 + 0x6BC ]
    let (r_f100, r_f0) = (core.f3778.f0[ctx.f0].f100, core.f3778.f0[ctx.f0].f0);
    let dq = ctx.f8;
    let p = &mut core.f3778.f0[pi];
    let (ox, oy, oz) = (p.f0[0], p.f0[1], p.f0[2]);
    let rel = [ox - r_f100[0], oy - r_f100[1], oz - r_f100[2]];
    let mut park = s * p.f28c;
    park = if park < 1.0 { park } else { 1.0 }; // minss xmm15, 1.0
    let t = add(qrot(dq, rel), r_f0);
    let mut force: V3;
    if ctx.f40 != 0 {
        p.f0 = t;
        force = ZERO_VEC;
    } else {
        let k = 1.0 - park;
        let x = ((t[0] - ox) * k) + ox;
        let y = ((t[1] - oy) * k) + oy;
        let z = ((t[2] - oz) * k) + oz;
        p.f20 = qmul(dq, p.f20);
        let rr = qrot(ctx.f18, [x, y, z]);
        let cdt = ctx.f10;
        p.f0 = [x + (((rr[0] - cdt[0]) - x) * park), (((rr[1] - cdt[1]) - y) * park) + y, (((rr[2] - cdt[2]) - z) * park) + z];
        // previous / current ComponentToWorld
        let d_t = [core.f10[0] - core.f40[0], core.f10[1] - core.f40[1], core.f10[2] - core.f40[2]];
        let pr = qrot(core.f30, mulv(core.f50, p.f100)); // lanes: P.scale * prevAnimTranslation
        let cr = qrot(core.f0, mulv(core.f20, p.f130));
        let d = [(cr[0] - pr[0]) + d_t[0], (cr[1] - pr[1]) + d_t[1], (cr[2] - pr[2]) + d_t[2]];
        let nq = [-core.f0[0], -core.f0[1], -core.f0[2], core.f0[3]]; // xorps sign mask 0x145C82880
        let rd = qrot(nq, d);
        let m = -(s * p.f288);
        force = [rd[0] * m, rd[1] * m, rd[2] * m];
    }
    // owner.vfD0() is 0 for this host: the extra force owner.vfF0() is not added.
    let max_len = p.f290 * ctx.f44;
    let l2 = ((force[1] * force[1]) + (force[0] * force[0])) + (force[2] * force[2]);
    if l2 > max_len * max_len {
        let inv = 1.0 / sqrt(l2);
        force = [(force[0] * max_len) * inv, (force[1] * max_len) * inv, (force[2] * max_len) * inv];
    }
    p.f7c = force;
}

/// 0x143ae2cb0 (p, ctx): LocalForce on p, then on every ParentChild child, depth first
pub fn fn_143ae2cb0(core: &mut Core, pi: usize, ctx: &LocalForceCtx) {
    fn_143aeaf80(core, ctx, pi);
    for k in 0..core.f3778.f0[pi].f1a8 {
        let child = core.f3778.f40[core.f3778.f0[pi].f1b0[k]].f8;
        fn_143ae2cb0(core, child, ctx);
    }
}

/// 0x143aee280 PreStep(core, r)
pub fn fn_143aee280(core: &mut Core, r: f32) {
    // 143aee2e4: particles
    for i in 0..core.f3778.f0.len() {
        let p = &core.f3778.f0[i];
        if p.f278 != 0 {
            continue;
        }
        if p.f276 != 0 {
            fn_143aee110(core, i);
        } else {
            let q = core.f3848.bones[p.f248].q;
            let reset = core.f33f4;
            let p = &mut core.f3778.f0[i];
            p.f160 = q;
            p.f20 = q;
            if reset != 0 {
                p.f30 = q;
            }
        }
    }
    // 143aee363: dynamic chain roots follow the animation
    for i in 0..core.f3778.f748.len() {
        let root = core.f3778.f748[i];
        if core.f3778.f0[root].f276 == 0 {
            fn_143aee110(core, root);
        }
    }
    // 143aee3af: anchor constraints
    {
        let bones = &core.f3848.bones;
        let scene = &mut core.f3778;
        for a in scene.f10.iter_mut() {
            let b = &bones[scene.f0[a.f0].f248];
            let p = add(qrot(b.q, mulv(ZERO_VEC, b.s)), b.t);
            if a.f4 != 4 {
                a.fc = p;
            }
            a.f28 = p;
        }
    }
    // 143aee603..143aeebc6: cone refresh
    if !core.f3778.f30.is_empty() {
        cone::fn_143aee280_cones(&mut core.f3778, &core.f3848.bones);
    }
    // 143aeebcf: attached chains
    for i in 0..core.f3778.f758.len() {
        let pi = core.f3778.f758[i].f0;
        let par = core.f3778.f0[pi].f190;
        core.f3778.f0[pi].f160 = core.f3778.f0[pi].f120;
        if let Some(par) = par {
            core.f3778.f0[par].f160 = core.f3778.f0[par].f120;
        }
    }
    // 143aeec2a: KDI groups
    for i in 0..core.f3778.f768.len() {
        for j in 0..core.f3778.f768[i].f8.len() {
            let pi = core.f3778.f768[i].f8[j].f0;
            core.f3778.f0[pi].f160 = core.f3778.f0[pi].f120;
        }
    }
    // 143aeecb8: inverse of the current component rotation / scale
    let (iq, is);
    if any_abs_greater_small(core.f20) {
        iq = [core.f0[0] * -1.0, core.f0[1] * -1.0, core.f0[2] * -1.0, core.f0[3] * 1.0];
        is = safe_scale_recip(core.f20);
    } else {
        iq = IDENT_Q;
        is = IDENT_S;
    }
    if core.f660 == 1 {
        // 143aeed52: componentDeltaTrans = rot(conj(curRot), (curT - prevT) * invScale); componentDeltaRot = conj(curRot)*prevRot
        let d = [core.f10[0] - core.f40[0], core.f10[1] - core.f40[1], core.f10[2] - core.f40[2]];
        let cdt = qrot(iq, mulv(d, is));
        let cdr = qmul([-core.f0[0], -core.f0[1], -core.f0[2], core.f0[3]], core.f30);
        for i in 0..core.f3778.f748.len() {
            let root = core.f3778.f748[i];
            let rp = &core.f3778.f0[root];
            if rp.f240.is_some() {
                continue;
            }
            let dq = qmul(core.f3848.bones[rp.f248].q, [-rp.ff0[0], -rp.ff0[1], -rp.ff0[2], rp.ff0[3]]);
            for k in 0..core.f3778.f0[root].f1a8 {
                let ctx = LocalForceCtx { f0: root, f8: dq, f10: cdt, f18: cdr, f40: core.f33f4, f44: r };
                let child = core.f3778.f40[core.f3778.f0[root].f1b0[k]].f8;
                fn_143aeaf80(core, &ctx, child);
                for j in 0..core.f3778.f0[child].f1a8 {
                    let grandchild = core.f3778.f40[core.f3778.f0[child].f1b0[j]].f8;
                    fn_143ae2cb0(core, grandchild, &ctx);
                }
            }
        }
    } else if core.f660 == 2 && core.f60 != -1 {
        // (ER69) NOT TRANSCRIBED, a FALLBACK instead of a stop (an outfit's asset reaches it; Sora's never): 143aef0ce..143aef527 (LSS fast-motion mode): none
        super::unported(20);
    }
}
