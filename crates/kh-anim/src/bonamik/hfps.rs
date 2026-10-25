//! `tools/lib/bonamik/hfps.js`: the component-level frame path of USQEX_KBD_Component (KH3.exe,
//! vtable 0x147518F70): the 30 Hz evaluation timer, the per-bone prev / cur delta buffers, the
//! interpolation, the recomposition, the weight blend, the core Blend.
//!
//! Ported: what the JS's reference runner (`createRunner`) runs for one Bonamik instance without
//! "before" / "after" sub-tasks: `fn_143b5b990` (timer), `fn_143b590d0` (UpdateAll, high-fps
//! path), and what they call. The mesh's bone buffer is the owner's (`core.f3848.bones`), as in
//! the runner, where the instance's Update evaluates on the buffer itself.
//!
//! Not ported (not reached by the runner): the instance PreUpdate / Update wrappers
//! (`fn_143b5b7e0`, `fn_143b633e0`), the tick gate (`fn_144067e10`, `fn_143b62bc0`), the
//! non-high-fps UpdateAll (`fn_143b58f40`), `fn_143b0e840`.
//!
//! CRT functions (`acosf`, `sinf`, `powf`, `logf`, `expf`) are computed in double and rounded,
//! as the JS does.

use super::bnm_eval;
use super::core as bcore;
use super::m::{qmul, qrot, rsqrt, sub, Q4, V3, Xf};
use super::records::{Cfg, Core, Stats};
use super::wind::Rng;

const ONE: f32 = 1.0;
const HALF: f32 = 0.5;
const K_SMALL: f32 = 1e-8_f64 as f32; // 0x322BCC77 @145C9C488
const K_1E5: f32 = 1e-5_f64 as f32; // 0x3727C5AC @145D20588
const K_1E4: f32 = 1e-4_f64 as f32; // 0x38D1B717 @145CF35A8
const K_9999: f32 = 0.9999_f64 as f32; // 0x3F7FF972 @145CF35B8
pub const K_DT30: f32 = (1.0_f64 / 30.0) as f32; // 0x3D088889 @145D3F94C
const K_QNORM: f32 = 0.01_f64 as f32; // 0x3C23D70A THRESH_QUAT_NORMALIZED

/// maxss a,b : b when equal / unordered
#[inline]
fn maxss(a: f32, b: f32) -> f32 {
    if a > b {
        a
    } else {
        b
    }
}

#[inline]
fn minss(a: f32, b: f32) -> f32 {
    if a < b {
        a
    } else {
        b
    }
}

/// One record of the delta buffers (0x60 bytes): prev = f0 / f10 / f20, cur = f30 / f40 / f50.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeltaRec {
    pub f0: Q4,
    pub f10: V3,
    pub f20: V3,
    pub f30: Q4,
    pub f40: V3,
    pub f50: V3,
}

impl DeltaRec {
    const IDENT: DeltaRec = DeltaRec { f0: [0.0, 0.0, 0.0, 1.0], f10: [0.0; 3], f20: [1.0; 3], f30: [0.0, 0.0, 0.0, 1.0], f40: [0.0; 3], f50: [1.0; 3] };
}

/// The fields of USQEX_KBD_Component the frame path uses.
#[derive(Clone, Debug)]
pub struct Comp {
    /// high-fps enable
    pub f498: u8,
    /// interval0
    pub f49c: f32,
    /// evalDt
    pub f4a0: f32,
    /// remaining
    pub f4a4: f32,
    /// interval
    pub f4a8: f32,
    /// doEval
    pub f4ac: u8,
    /// force
    pub f4ad: u8,
    pub f4b0: Vec<DeltaRec>,
    pub f42c: f32,
}

impl Default for Comp {
    fn default() -> Comp {
        Comp { f498: 1, f49c: K_DT30, f4a0: 0.0, f4a4: 0.0, f4a8: 0.0, f4ac: 0, f4ad: 0, f4b0: Vec::new(), f42c: 0.0 }
    }
}

/// sum of squares / dot of 4 lanes: (x+z) + (y+w)
#[inline]
pub fn dot4(a: Q4, b: Q4) -> f32 {
    ((a[0] * b[0]) + (a[2] * b[2])) + ((a[1] * b[1]) + (a[3] * b[3]))
}

/// VectorNormalizeSafe(q, default (0,0,0,1)) with threshold thr
pub fn qnorm_safe(q: Q4, thr: f32) -> Q4 {
    let sq = dot4(q, q);
    if !(thr <= sq) {
        return [0.0, 0.0, 0.0, 1.0]; // cmpleps thr,sq
    }
    let r = rsqrt(sq);
    [r * q[0], r * q[1], r * q[2], r * q[3]]
}

/// rcpps + two Newton steps x = (x+x) - (x*x)*s ; |s| <= 1e-8 -> 0 (GetSafeScaleReciprocal)
pub fn safe_recip(s: f32) -> f32 {
    if s.abs() <= K_SMALL {
        return 0.0;
    }
    let mut x = 1.0 / s;
    x = (x + x) - ((x * x) * s);
    x = (x + x) - ((x * x) * s);
    x
}

/// 0x142d801f0 : FTransform::GetRelativeTransform (stock 4.17, vectorised): A * B^-1
pub fn fn_142d801f0(a: &Xf, b: &Xf) -> Xf {
    let sq = dot4(b.q, b.q);
    if K_QNORM < (ONE - sq).abs() {
        // 142d8025b..142d80275: !B.IsRotationNormalized()
        return Xf::IDENT;
    }
    for i in 0..3 {
        if minss(a.s[i], b.s[i]) < 0.0 {
            // (ER69) NOT TRANSCRIBED, a FALLBACK instead of a stop (an outfit's asset reaches it; Sora's never): 0x142d805f0 (negative scale): the quaternion path
            super::unported(10);
        }
    }
    let rs = [safe_recip(b.s[0]), safe_recip(b.s[1]), safe_recip(b.s[2])];
    let d = sub(a.t, b.t); // subps A.T, B.T ; W masked
    let qi = [-b.q[0], -b.q[1], -b.q[2], b.q[3]]; // mulps (-1,-1,-1,1) @149B5C5C0
    let r = qrot(qi, d);
    Xf {
        s: [rs[0] * a.s[0], rs[1] * a.s[1], rs[2] * a.s[2]],
        t: [r[0] * rs[0], r[1] * rs[1], r[2] * rs[2]],
        q: qmul(qi, a.q), // VectorQuaternionMultiply2(inv(B.q), A.q); NOT normalised
    }
}

/// 0x142d90ca0 : FQuat::Slerp_NotNormalized (stock)
pub fn fn_142d90ca0(a: Q4, b: Q4, t: f32) -> Q4 {
    let raw = (((a[1] * b[1]) + (a[0] * b[0])) + (a[2] * b[2])) + (a[3] * b[3]); // y, x, z, w
    let abs = raw.abs();
    let (s0, mut s1);
    if abs >= K_9999 {
        // 142d90d1a jae -> linear
        s0 = ONE - t;
        s1 = t;
    } else {
        let x = if abs >= -1.0 { minss(abs, ONE) } else { -1.0 }; // jb (unordered) keeps -1
        let omega = (x as f64).acos() as f32; // CRT acosf
        let inv = ONE / ((omega as f64).sin() as f32); // CRT sinf ; divss 1, sin
        s0 = ((((ONE - t) * omega) as f64).sin() as f32) * inv;
        s1 = (((omega * t) as f64).sin() as f32) * inv;
    }
    if !(raw >= 0.0) {
        s1 = -s1; // 142d90da8 comiss raw,0 ; jae skips the xorps
    }
    [(s1 * b[0]) + (s0 * a[0]), (s1 * b[1]) + (s0 * a[1]), (s1 * b[2]) + (s0 * a[2]), (s1 * b[3]) + (s0 * a[3])]
}

/// 0x143b609b0 : scale interpolation
pub fn fn_143b609b0(a: f32, b: f32, t: f32) -> f32 {
    if a * b > 0.0 {
        return (((b / a) as f64).powf(t as f64) as f32) * a; // CRT powf
    }
    ((ONE - t) * a) + (b * t)
}

/// inlined stock FTransform::Multiply(out, A, B) = "A then B", non-negative-scale path:
/// q = B.q * A.q (NOT normalised here), t = rot(B.q, B.s * A.t) + B.t, s = B.s * A.s
pub fn xf_mul_inline(a: &Xf, b: &Xf, at: &str) -> Xf {
    for i in 0..3 {
        if minss(a.s[i], b.s[i]) < 0.0 {
            // (ER69) NOT TRANSCRIBED, a FALLBACK instead of a stop (an outfit's asset reaches it; Sora's never): matrix multiply path (negative scale): the quaternion path
            let _ = at;
            super::unported(11);
        }
    }
    let v = [a.t[0] * b.s[0], a.t[1] * b.s[1], a.t[2] * b.s[2]];
    let r = qrot(b.q, v);
    Xf { q: qmul(b.q, a.q), t: [r[0] + b.t[0], r[1] + b.t[1], r[2] + b.t[2]], s: [a.s[0] * b.s[0], a.s[1] * b.s[1], a.s[2] * b.s[2]] }
}

/// inlined weight blend of a new transform N into the buffer entry `old` (143b5a0ec..143b5a2fc, 143b5aef8..143b5b11f)
pub fn blend_into_inline(old: &mut Xf, n: &Xf, w: f32) {
    if !(w.abs() > K_1E5) {
        return; // buffer entry copied onto itself
    }
    if !((w - ONE).abs() > K_1E5) {
        *old = *n;
        return;
    }
    let mut t = [0.0f32; 3];
    let mut s = [0.0f32; 3];
    for i in 0..3 {
        t[i] = (w * (n.t[i] - old.t[i])) + old.t[i];
    }
    for i in 0..3 {
        // CRT logf / expf
        let lo = (old.s[i] as f64).ln() as f32;
        let ln = (n.s[i] as f64).ln() as f32;
        s[i] = ((((ln - lo) * w) + lo) as f64).exp() as f32;
    }
    let d = dot4(old.q, n.q);
    let sel = if 0.0 <= d { old.q } else { [0.0 - old.q[0], 0.0 - old.q[1], 0.0 - old.q[2], 0.0 - old.q[3]] };
    let om = ONE - w;
    let mut q = [0.0f32; 4];
    for i in 0..4 {
        q[i] = (sel[i] * om) + (w * n.q[i]);
    }
    old.t = t;
    old.s = s;
    old.q = qnorm_safe(q, K_SMALL);
}

/// 0x143b5b990 : HighFpsTimer(rcx = comp, xmm1 = dt). `n` = the mesh's bone count.
pub fn fn_143b5b990(comp: &mut Comp, dt: f32, n: usize) {
    if comp.f4b0.is_empty() {
        // cmp dword [comp+0x4B8],0
        comp.f4b0 = vec![DeltaRec::IDENT; n];
        let mut iv = comp.f49c;
        if !(iv > K_SMALL) {
            iv = K_DT30;
            comp.f49c = K_DT30;
        }
        comp.f4a0 = dt;
        comp.f4ac = 1;
        comp.f4a4 = 0.0;
        comp.f4a8 = maxss(iv, dt);
        return;
    }
    let mut r = comp.f4a4;
    if r > 0.0 {
        r = r - dt;
        comp.f4a4 = r;
    }
    if r > K_SMALL && comp.f4ad == 0 {
        return;
    }
    let iv = maxss(comp.f49c, dt);
    comp.f4ac = 1;
    comp.f4a0 = iv;
    comp.f4a8 = iv;
    if comp.f4ad != 0 {
        comp.f4ad = 0;
        comp.f4a4 = 0.0;
    } else {
        comp.f4a4 = iv + r;
    }
    for e in comp.f4b0.iter_mut() {
        e.f0 = e.f30;
        e.f10 = e.f40;
        e.f20 = e.f50;
    }
}

// ---- force-flag setters (bodies up to the "apply to children" recursion)
pub fn fn_143b5c470(comp: &mut Comp) {
    comp.f4ad = 1;
}

pub fn fn_143b618b0(comp: &mut Comp, enable: bool) {
    if enable && comp.f498 == 0 {
        comp.f4ad = 1;
    }
    comp.f498 = if enable { 1 } else { 0 };
}

pub fn fn_143b61b10(comp: &mut Comp, interval: f32) {
    let old = comp.f49c;
    let v = if interval > K_SMALL { interval } else { K_DT30 };
    comp.f49c = v;
    if v != old {
        comp.f4ad = 1; // ucomiss ; jne/jp
    }
}

/// 0x143aefb30 : Blend(rcx = core): blend every particle bone against its animated transform (p+0x120)
pub fn fn_143aefb30(core: &mut Core) {
    let bones = &mut core.f3848.bones;
    for p in &core.f3778.f0 {
        if p.f278 != 0 {
            continue; // multiple-collision object
        }
        let w = (ONE - core.f3578[p.f2b8]) * core.f36f8;
        if w > K_9999 {
            continue;
        }
        let b = &mut bones[p.f248];
        if !(w >= K_1E4) || !(w.abs() > K_1E5) {
            b.q = p.f120;
            b.t = p.f130;
            b.s = p.f140;
            continue;
        }
        if !((w - ONE).abs() > K_1E5) {
            continue; // stores B onto itself
        }
        let mut t = [0.0f32; 3];
        let mut s = [0.0f32; 3];
        for k in 0..3 {
            t[k] = ((b.t[k] - p.f130[k]) * w) + p.f130[k];
        }
        for k in 0..3 {
            let la = (p.f140[k] as f64).ln() as f32;
            let lb = (b.s[k] as f64).ln() as f32;
            s[k] = ((((lb - la) * w) + la) as f64).exp() as f32;
        }
        let a = p.f120;
        let d = dot4(a, b.q);
        let sel = if 0.0 <= d { a } else { [0.0 - a[0], 0.0 - a[1], 0.0 - a[2], 0.0 - a[3]] };
        let om = ONE - w;
        let mut q = [0.0f32; 4];
        for k in 0..4 {
            q[k] = (sel[k] * om) + (b.q[k] * w);
        }
        b.t = t;
        b.s = s;
        b.q = qnorm_safe(q, K_SMALL); // threshold @149D52940, default @149D52880
    }
}

/// 0x143af28e0 : deferred Blend(rcx = core)
pub fn fn_143af28e0(core: &mut Core) {
    if core.f3840 != 0 {
        core.f3840 = 0;
        fn_143aefb30(core);
    }
}

/// 0x143b11a20 (rcx = core, xmm1 = dt)
pub fn fn_143b11a20(core: &mut Core, dt: f32) {
    bcore::fn_143b10010(core, dt);
    bcore::fn_143b11700(core);
}

/// 0x143b02170 (rcx = core)
pub fn fn_143b02170(core: &Core) {
    for g in 0..core.f410 {
        if core.f8c4[g] != 0 {
            // (ER69) NOT TRANSCRIBED, a FALLBACK instead of a stop (an outfit's asset reaches it; Sora's never): 0x143afe170: none
            super::unported(12);
        }
    }
}

/// 0x143afc920 : FollowBones(rcx = core) - structure only; live only when a follower list is non-empty
pub fn fn_143afc920(core: &Core) {
    if core.f3770 > 0 {
        // (ER69) NOT TRANSCRIBED, a FALLBACK instead of a stop (an outfit's asset reaches it; Sora's never): 143afc9e1..143afd2e4 (follower bones): they keep the animated pose
        super::unported(13);
    }
}

fn bit_set(core: &Core, i: usize) -> bool {
    core.f3800.get(i >> 5).is_some_and(|w| w & (1 << (i & 31)) != 0)
}

/// 0x143b590d0 : high-fps UpdateAll(rcx = comp, xmm1 = dt). Returns (alphaLinear, alpha) (the exe
/// returns nothing; for diagnostics).
pub fn fn_143b590d0(comp: &mut Comp, core: &mut Core, cfg: &Cfg, rng: &mut Rng, stats: &mut Stats, dt: f32) -> (f32, f32) {
    let n = core.f3848.bones.len();
    // local pose of the pose now in the buffer (animated + before-tasks): records {parent, FTransform, flag}
    let mut local: Vec<Xf> = Vec::with_capacity(n);
    {
        let bones = &core.f3848.bones;
        let parents = &core.f3848.parents;
        if n > 0 {
            local.push(bones[0]); // bone 0: raw copy of the component transform
        }
        for i in 1..n {
            let mut xf = fn_142d801f0(&bones[i], &bones[parents[i] as usize]);
            xf.q = qnorm_safe(xf.q, K_SMALL);
            local.push(xf);
        }
    }
    if comp.f4ac != 0 {
        // evaluation frame
        comp.f4ac = 0;
        core.f3840 = 1; // byte [inst+0x3850]
        bnm_eval::core_update(core, cfg, comp.f4a0, rng, stats); // inst.vf18(comp.f4A0): Update(evalDt)
        let flags: Vec<bool> = (0..n).map(|i| bit_set(core, i)).collect();
        let bones = &core.f3848.bones;
        let parents = &core.f3848.parents;
        let set_cur = |rec: &mut DeltaRec, x: Xf| {
            rec.f30 = qnorm_safe(x.q, K_SMALL);
            rec.f40 = x.t;
            rec.f50 = x.s;
        };
        if n > 0 {
            if !flags[0] {
                comp.f4b0[0] = DeltaRec::IDENT;
            } else {
                set_cur(&mut comp.f4b0[0], fn_142d801f0(&bones[0], &local[0]));
            }
        }
        for i in 1..n {
            if !flags[i] {
                comp.f4b0[i] = DeltaRec::IDENT;
                continue;
            }
            let mut tmp = fn_142d801f0(&bones[i], &bones[parents[i] as usize]); // new local transform (post-Update buffer, both)
            tmp.q = qnorm_safe(tmp.q, K_SMALL);
            set_cur(&mut comp.f4b0[i], fn_142d801f0(&tmp, &local[i])); // delta against the animated local transform
        }
        fn_143af28e0(core); // the core's own Blend, deferred to here
    }
    // alpha (143b59810..143b5985b)
    let mut a;
    let rem = comp.f4a4;
    if rem >= 0.0 {
        a = ONE - (rem / comp.f4a8);
        a = if a >= 0.0 { minss(a, ONE) } else { 0.0 };
    } else {
        a = ONE; // jb (also unordered): minss(1,1)
    }
    let alpha = ((a + ONE) * a) * HALF; // ease-in: a*(a+1)/2
    // per-bone maximum weight over the Bonamik instances
    let mut max_w = vec![0.0f32; n];
    fn_143b11a20(core, dt);
    fn_143b02170(core);
    for i in 0..core.f3818 {
        let w = core.f3810[i];
        if w > max_w[i] {
            max_w[i] = w;
        }
    }
    let interp = |rec: &DeltaRec| -> Xf {
        let t = [((rec.f40[0] - rec.f10[0]) * alpha) + rec.f10[0], rec.f10[1] + ((rec.f40[1] - rec.f10[1]) * alpha), rec.f10[2] + ((rec.f40[2] - rec.f10[2]) * alpha)];
        let s = [fn_143b609b0(rec.f20[0], rec.f50[0], alpha), fn_143b609b0(rec.f20[1], rec.f50[1], alpha), fn_143b609b0(rec.f20[2], rec.f50[2], alpha)];
        let q = qnorm_safe(fn_142d90ca0(rec.f0, rec.f30, alpha), K_SMALL);
        Xf { q, t, s }
    };
    let bones = &mut core.f3848.bones;
    let parents = &core.f3848.parents;
    if n > 0 && !(K_SMALL >= max_w[0]) {
        // bone 0
        let a0 = interp(&comp.f4b0[0]);
        let mut nn = xf_mul_inline(&a0, &local[0], "143b59bac");
        nn.q = qnorm_safe(nn.q, K_SMALL);
        blend_into_inline(&mut bones[0], &nn, max_w[0]);
    }
    for i in 1..n {
        // ascending bone index: parents are final already
        if K_SMALL >= max_w[i] {
            continue;
        }
        let a1 = interp(&comp.f4b0[i]);
        let m1 = xf_mul_inline(&a1, &local[i], "143b5a524"); // delta * animated local (q not normalised)
        let parent = bones[parents[i] as usize];
        let mut nn = xf_mul_inline(&m1, &parent, "143b5a9f9"); // * parent component transform now in the buffer
        nn.q = qnorm_safe(nn.q, K_SMALL);
        blend_into_inline(&mut bones[i], &nn, max_w[i]);
    }
    (a, alpha)
}
