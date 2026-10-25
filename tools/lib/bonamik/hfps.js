// Bonamik pass 2, part "hfps": component-level frame path of USQEX_KBD_Component (KH3.exe, vtable 0x147518F70):
// 30 Hz evaluation timer, per-bone prev/cur delta buffers, interpolation, recomposition, weight blend, the core Blend,
// instance Update / PreUpdate, tick gate. See H_hfps.md for the instruction-level notes and the marks.
//
// Object model used here (fields named by hex offset, see SUB_BRIEF.md):
//   comp  = USQEX_KBD_Component: f440 mesh, f450 all sub-tasks, f468 Bonamik instances, f478 "before" tasks,
//           f488 "after" tasks, f498 high-fps enable (byte), f49C interval0, f4A0 evalDt, f4A4 remaining, f4A8 interval,
//           f4AC doEval (byte), f4AD force (byte), f4B0 array of 0x60 records {f0 quat, f10 vec3, f20 vec3 = prev;
//           f30 quat, f40 vec3, f50 vec3 = cur} (count +0x4B8 = f4B0.length), f428 bMultiThread, f429 finalized, f42C dt.
//   mesh  = TresSkeletalMeshComponent: f980 skeletal mesh {f158: [{f8: parentIndex}], f160: boneCount, f60: resource},
//           f990 = [buffer0, buffer1] (ComponentSpaceTransformsArray, each an array of bones {q,t,s}),
//           f9B4 = index of the buffer the KBD code reads AND writes (= stock CurrentReadComponentTransforms),
//           f1B0 flags, fB20 lock counter, fA40, fA80.
//   inst  = FSQEX_KBDBonamikInstance: f3868 -> core, f3870 -> owner interface, f3878, f3879 rendered, f387A turned on,
//           f387B auto disable, f3884 previous LOD size, f3888 out-of-camera mode, f3890 LOD entries; methods vf10
//           (PreUpdate), vf18 (Update), vf20 (type id).
//   core  = inst + 0x10. ALIASES: inst+0x3850 == core+0x3840 (byte "blend deferred to the component"),
//           inst+0x3800 == core+0x37F0 (inline words of the driven-bone TBitArray), inst+0x3810 == core+0x3800 (its heap
//           pointer, null when the inline words are used), inst+0x3820 == core+0x3810 (per-bone weight array),
//           inst+0x3828 == core+0x3818 (its count). The JS always goes through the core object.
// Functions of other blocks are reached through `ext` (see link()); nothing here guesses their behaviour.
'use strict';
const { f, add, sub, mul, dot, cross, sqrt, rsqrt, qmul, qconj, qrot } = require('./m.js');

const ONE = f(1), HALF = f(0.5);
const K_SMALL = f(1e-8);        // 0x322BCC77 @145C9C488 / 149D535D0 (runtime copy of 145C826E0)
const K_1E5 = f(1e-5);          // 0x3727C5AC @145D20588
const K_1E4 = f(1e-4);          // 0x38D1B717 @145CF35A8
const K_9999 = f(0.9999);       // 0x3F7FF972 @145CF35B8
const K_DT30 = f(1 / 30);       // 0x3D088889 @145D3F94C
const K_QNORM = f(0.01);        // 0x3C23D70A @149B5C640 (runtime copy of 145C82720) THRESH_QUAT_NORMALIZED

const maxss = (a, b) => (a > b ? a : b);    // maxss a,b : b when equal / unordered
const minss = (a, b) => (a < b ? a : b);
const ident = () => ({ q: [0, 0, 0, 1], t: [0, 0, 0], s: [1, 1, 1] });   // 149B5C560 / 570 / 580
const cloneXf = x => ({ q: x.q.slice(), t: x.t.slice(), s: x.s.slice() });

// ---- external functions (other blocks); replace with link({...}) ------------------------------------------------
const ext = {
  g_HighFpsEval: 1,                                                    // int @14874E21C (1 in the image)
  globalConfig: { f53: 1, f55: 1, f56: 1 },                            // SQEX_BonamikGlobalConfig CDO (+0x53 bEnableUpdate, +0x55 bEnableSimulation, +0x56 bEnableUpdateOverlaps)
  fn_143b10010: () => { throw new Error('not linked: 0x143b10010 GroupWeights'); },
  fn_143b11700: () => { throw new Error('not linked: 0x143b11700 BoneWeights'); },
  fn_143afe170: () => { throw new Error('not linked: 0x143afe170 LOD pose blend'); },
  fn_143b0efc0: () => { throw new Error('not linked: 0x143b0efc0 CoreUpdate'); },
  fn_143b10a40: () => { throw new Error('not linked: 0x143b10a40 Release'); },
  fn_143b0ebd0: () => { throw new Error('not linked: 0x143b0ebd0'); },
  fn_143b0ea70: () => { throw new Error('not linked: 0x143b0ea70'); },
  fn_143b10ca0: () => { throw new Error('not linked: 0x143b10ca0'); },
};
function link(o) { Object.assign(ext, o); return module.exports; }

// ---- small SIMD idioms as coded ---------------------------------------------------------------------------------
// sum of squares / dot of 4 lanes: mulps; shufps 4E; addps; shufps 39; addps  ->  (x+z) + (y+w)
const dot4 = (a, b) => f(f(f(a[0] * b[0]) + f(a[2] * b[2])) + f(f(a[1] * b[1]) + f(a[3] * b[3])));
// VectorNormalizeSafe(q, default (0,0,0,1)) with threshold thr: rsqrtps + two Newton steps (m.js rsqrt), select.
function qnormSafe(q, thr) {
  const sq = dot4(q, q);
  if (!(thr <= sq)) return [0, 0, 0, 1];                               // cmpleps thr,sq
  const r = rsqrt(sq);
  return [f(r * q[0]), f(r * q[1]), f(r * q[2]), f(r * q[3])];
}
// rcpps + two Newton steps x = (x+x) - (x*x)*s ; |s| <= 1e-8 -> 0  (GetSafeScaleReciprocal). rcpps emulated by 1/s.
function safeRecip(s) {
  if (Math.abs(s) <= K_SMALL) return 0;
  let x = f(1 / s);
  x = f(f(x + x) - f(f(x * x) * s));
  x = f(f(x + x) - f(f(x * x) * s));
  return x;
}

// ---- 0x142d801f0 : FTransform::GetRelativeTransform (stock 4.17, vectorised). rcx = A (this), rdx = out, r8 = B ----
// out = A * B^-1. Returns out (rax = rdx).
function fn_142d801f0(A, out, B) {
  const sq = dot4(B.q, B.q);
  if (K_QNORM < Math.abs(f(ONE - sq))) {                               // 142d8025b..142d80275: !B.IsRotationNormalized()
    out.q = [0, 0, 0, 1]; out.t = [0, 0, 0]; out.s = [1, 1, 1]; return out;
  }
  for (let i = 0; i < 3; i++) if (minss(A.s[i], B.s[i]) < 0) throw new Error('not transcribed: 0x142d805f0 (GetRelativeTransformUsingMatrixWithScale, negative scale)');
  const rs = [safeRecip(B.s[0]), safeRecip(B.s[1]), safeRecip(B.s[2])];
  const d = sub(A.t, B.t);                                             // subps A.T, B.T ; W masked
  const qi = [f(-B.q[0]), f(-B.q[1]), f(-B.q[2]), B.q[3]];             // mulps (-1,-1,-1,1) @149B5C5C0
  const r = qrot(qi, d);                                               // c2 + ((w*t) + d), same value as m.js qrot
  out.s = [f(rs[0] * A.s[0]), f(rs[1] * A.s[1]), f(rs[2] * A.s[2])];
  out.t = [f(r[0] * rs[0]), f(r[1] * rs[1]), f(r[2] * rs[2])];
  out.q = qmul(qi, A.q);                                               // VectorQuaternionMultiply2(inv(B.q), A.q); NOT normalised
  return out;
}

// ---- 0x142d90ca0 : FQuat::Slerp_NotNormalized (stock). rcx = out, rdx = A, r8 = B, xmm3 = t ------------------------
function fn_142d90ca0(out, A, B, t) {
  t = f(t);
  const raw = f(f(f(f(A[1] * B[1]) + f(A[0] * B[0])) + f(A[2] * B[2])) + f(A[3] * B[3]));   // y, x, z, w
  const abs = Math.abs(raw);
  let s0, s1;
  if (abs >= K_9999) { s0 = f(ONE - t); s1 = t; }                      // 142d90d1a jae -> linear
  else {
    const x = (abs >= -1) ? minss(abs, ONE) : f(-1);                   // jb (unordered) keeps -1
    const omega = f(Math.acos(x));                                     // CRT acosf
    const inv = f(ONE / f(Math.sin(omega)));                           // CRT sinf ; divss 1, sin
    s0 = f(f(Math.sin(f(f(ONE - t) * omega))) * inv);
    s1 = f(f(Math.sin(f(omega * t))) * inv);
  }
  if (!(raw >= 0)) s1 = f(-s1);                                        // 142d90da8 comiss raw,0 ; jae skips the xorps
  for (let i = 0; i < 4; i++) out[i] = f(f(s1 * B[i]) + f(s0 * A[i]));
  return out;
}

// ---- 0x143b609b0 : scale interpolation. xmm0 = a, xmm1 = b, xmm2 = t -----------------------------------------------
function fn_143b609b0(a, b, t) {
  a = f(a); b = f(b); t = f(t);
  if (f(a * b) > 0) return f(f(Math.pow(f(b / a), t)) * a);           // CRT powf
  return f(f(f(ONE - t) * a) + f(b * t));
}

// ---- inlined stock FTransform::Multiply(out, A, B) = "A then B", non-negative-scale path -------------------------
// q = B.q * A.q (VectorQuaternionMultiply2, NOT normalised here), t = rot(B.q, B.s * A.t) + B.t, s = B.s * A.s
function xfMulInline(A, B, where) {
  for (let i = 0; i < 3; i++) if (minss(A.s[i], B.s[i]) < 0) throw new Error('not transcribed: matrix multiply path (negative scale) ' + where);
  const v = [f(A.t[0] * B.s[0]), f(A.t[1] * B.s[1]), f(A.t[2] * B.s[2])];
  const r = qrot(B.q, v);
  return {
    q: qmul(B.q, A.q),
    t: [f(r[0] + B.t[0]), f(r[1] + B.t[1]), f(r[2] + B.t[2])],
    s: [f(A.s[0] * B.s[0]), f(A.s[1] * B.s[1]), f(A.s[2] * B.s[2])],
  };
}

// ---- inlined weight blend of a new transform N into the buffer entry `old` (143b5a0ec..143b5a2fc, 143b5aef8..143b5b11f)
function blendIntoInline(old, N, w) {
  w = f(w);
  if (!(Math.abs(w) > K_1E5)) return;                                  // buffer entry copied onto itself
  if (!(Math.abs(f(w - ONE)) > K_1E5)) { old.q = N.q.slice(); old.t = N.t.slice(); old.s = N.s.slice(); return; }
  const t = [0, 0, 0], s = [0, 0, 0];
  for (let i = 0; i < 3; i++) t[i] = f(f(w * f(N.t[i] - old.t[i])) + old.t[i]);
  for (let i = 0; i < 3; i++) {                                        // CRT logf / expf
    const lo = f(Math.log(old.s[i])), ln = f(Math.log(N.s[i]));
    s[i] = f(Math.exp(f(f(f(ln - lo) * w) + lo)));
  }
  const d = dot4(old.q, N.q);
  const sel = (0 <= d) ? old.q : [f(0 - old.q[0]), f(0 - old.q[1]), f(0 - old.q[2]), f(0 - old.q[3])];
  const om = f(ONE - w);
  const q = [0, 0, 0, 0];
  for (let i = 0; i < 4; i++) q[i] = f(f(sel[i] * om) + f(w * N.q[i]));
  old.t = t; old.s = s; old.q = qnormSafe(q, K_SMALL);
}

// ---- 0x143b5b990 : HighFpsTimer(rcx = comp, xmm1 = dt) -------------------------------------------------------------
function fn_143b5b990(comp, dt) {
  dt = f(dt);
  if (comp.f4B0.length === 0) {                                        // cmp dword [comp+0x4B8],0
    const n = comp.f440.f980.f160;
    for (let i = 0; i < n; i++) comp.f4B0.push({ f0: [0, 0, 0, 1], f10: [0, 0, 0], f20: [1, 1, 1], f30: [0, 0, 0, 1], f40: [0, 0, 0], f50: [1, 1, 1] });
    let iv = comp.f49C;
    if (!(iv > K_SMALL)) { iv = K_DT30; comp.f49C = K_DT30; }
    comp.f4A0 = dt; comp.f4AC = 1; comp.f4A4 = 0; comp.f4A8 = maxss(iv, dt);
    return;
  }
  let r = comp.f4A4;
  if (r > 0) { r = f(r - dt); comp.f4A4 = r; }
  if (r > K_SMALL && !comp.f4AD) return;
  const iv = maxss(comp.f49C, dt);
  comp.f4AC = 1; comp.f4A0 = iv; comp.f4A8 = iv;
  if (comp.f4AD) { comp.f4AD = 0; comp.f4A4 = 0; } else comp.f4A4 = f(iv + r);
  for (const e of comp.f4B0) { e.f0 = e.f30.slice(); e.f10 = e.f40.slice(); e.f20 = e.f50.slice(); }
}

// ---- force-flag setters (bodies up to the "apply to children" recursion) ------------------------------------------
function fn_143b5c470(comp, toChildren) { comp.f4AD = 1; }                         // recursion over child meshes' KBD components not modelled
function fn_143b618b0(comp, enable, toChildren) { if (enable && !comp.f498) comp.f4AD = 1; comp.f498 = enable ? 1 : 0; }
function fn_143b61b10(comp, interval, toChildren) {
  interval = f(interval);
  const old = comp.f49C, v = (interval > K_SMALL) ? interval : K_DT30;
  comp.f49C = v; if (v !== old) comp.f4AD = 1;                                     // ucomiss ; jne/jp
}

// ---- 0x143aefb30 : Blend(rcx = core): blend every particle bone against its animated transform (p+0x120) --------
function fn_143aefb30(core) {
  const bones = core.f3848.vf60();
  const scene = core.f3778;
  for (let i = 0; i < scene.f8; i++) {
    const p = scene.f0[i];
    if (p.f278) continue;                                              // multiple-collision object
    const w = f(f(ONE - core.f3578[p.f2B8]) * core.f36F8);
    if (w > K_9999) continue;
    const B = bones[p.f248];
    if (!(w >= K_1E4) || !(Math.abs(w) > K_1E5)) { B.q = p.f120.slice(); B.t = p.f130.slice(); B.s = p.f140.slice(); continue; }
    if (!(Math.abs(f(w - ONE)) > K_1E5)) continue;                     // stores B onto itself
    const t = [0, 0, 0], s = [0, 0, 0];
    for (let k = 0; k < 3; k++) t[k] = f(f(f(B.t[k] - p.f130[k]) * w) + p.f130[k]);
    for (let k = 0; k < 3; k++) {
      const la = f(Math.log(p.f140[k])), lb = f(Math.log(B.s[k]));
      s[k] = f(Math.exp(f(f(f(lb - la) * w) + la)));
    }
    const A = p.f120;
    const d = dot4(A, B.q);
    const sel = (0 <= d) ? A : [f(0 - A[0]), f(0 - A[1]), f(0 - A[2]), f(0 - A[3])];
    const om = f(ONE - w);
    const q = [0, 0, 0, 0];
    for (let k = 0; k < 4; k++) q[k] = f(f(sel[k] * om) + f(B.q[k] * w));
    B.t = t; B.s = s; B.q = qnormSafe(q, K_SMALL);                     // threshold @149D52940, default @149D52880
  }
}

// ---- 0x143af28e0 : deferred Blend(rcx = core) -------------------------------------------------------------------
function fn_143af28e0(core) { if (core.f3840) { core.f3840 = 0; fn_143aefb30(core); } }

// ---- 0x143b11a20 (rcx = core, xmm1 = dt) and 0x143b02170 (rcx = core) ---------------------------------------------
function fn_143b11a20(core, dt, e = ext) { e.fn_143b10010(core, f(dt)); e.fn_143b11700(core); }
function fn_143b02170(core, e = ext) {
  for (let g = 0; g < core.f410; g++) if (core.f8C4[g]) { e.fn_143afe170(core); return; }
}

// ---- 0x143afc920 : FollowBones(rcx = core) - structure only (see md); live only when a follower list is non-empty --
function fn_143afc920(core) {
  const scene = core.f3778;
  for (let i = 0; i < core.f3770; i++) {
    const L = core.f3768[i];                                           // {f0 particle index, f8 bone list, f10 count}
    const p = scene.f0[L.f0];
    const w = f(f(ONE - core.f3578[p.f2B8]) * core.f36F8);
    if (!(w >= K_1E4)) continue;
    if (L.f10 > 0) throw new Error('not transcribed: 143afc9e1..143afd2e4 (follower bones, matrix path)');
  }
}

const bitSet = (core, i) => { const words = core.f3800 || core.f37F0; return (words[i >> 5] & ((1 << (i & 31)) >>> 0)) !== 0; };

// ---- 0x143b590d0 : high-fps UpdateAll(rcx = comp, xmm1 = dt) ------------------------------------------------------
function fn_143b590d0(comp, dt, e = ext) {
  dt = f(dt);
  const mesh = comp.f440;
  const bones = mesh.f990[mesh.f9B4];                                  // mesh + (0x99 + [mesh+0x9B4]) * 0x10
  const skel = mesh.f980;
  const n = skel.f160;
  for (const task of comp.f478) task.vf18(dt);                         // "before" sub-tasks, every frame, real dt
  // local pose of the pose now in the buffer (animated + before-tasks): 0x50 records {parent, FTransform, flag}
  const L = new Array(n);
  if (n > 0) L[0] = { parent: -1, xf: cloneXf(bones[0]), flag: 0 };    // bone 0: raw copy of the component transform
  for (let i = 1; i < n; i++) {
    const parent = skel.f158[i].f8;
    const xf = fn_142d801f0(bones[i], ident(), bones[parent]);
    xf.q = qnormSafe(xf.q, K_SMALL);
    L[i] = { parent, xf, flag: 0 };
  }
  if (comp.f4AC) {                                                     // evaluation frame
    comp.f4AC = 0;
    for (let i = 0; i < n; i++) L[i].flag = 0;
    for (const inst of comp.f468) {
      const core = inst.f3868;
      core.f3840 = 1;                                                  // byte [inst+0x3850]
      inst.vf18(comp.f4A0);                                            // Update(evalDt)
      for (let i = 0; i < n; i++) if (bitSet(core, i)) L[i].flag = 1;
    }
    const idq = [0, 0, 0, 1];
    const setBoth = (rec) => { rec.f0 = idq.slice(); rec.f10 = [0, 0, 0]; rec.f20 = [1, 1, 1]; rec.f30 = idq.slice(); rec.f40 = [0, 0, 0]; rec.f50 = [1, 1, 1]; };
    const setCur = (rec, x) => { rec.f30 = qnormSafe(x.q, K_SMALL); rec.f40 = x.t; rec.f50 = x.s; };
    if (n > 0) {
      if (!L[0].flag) setBoth(comp.f4B0[0]);
      else setCur(comp.f4B0[0], fn_142d801f0(bones[0], ident(), L[0].xf));
    }
    for (let i = 1; i < n; i++) {
      if (!L[i].flag) { setBoth(comp.f4B0[i]); continue; }
      const tmp = fn_142d801f0(bones[i], ident(), bones[L[i].parent]); // new local transform (post-Update buffer, both)
      tmp.q = qnormSafe(tmp.q, K_SMALL);
      setCur(comp.f4B0[i], fn_142d801f0(tmp, ident(), L[i].xf));       // delta against the animated local transform
    }
    for (const inst of comp.f468) fn_143af28e0(inst.f3868);            // the core's own Blend, deferred to here
  }
  // alpha (143b59810..143b5985b)
  let a;
  const rem = comp.f4A4;
  if (rem >= 0) { a = f(ONE - f(rem / comp.f4A8)); a = (a >= 0) ? minss(a, ONE) : 0; }
  else a = ONE;                                                        // jb (also unordered): minss(1,1)
  const alpha = f(f(f(a + ONE) * a) * HALF);                           // ease-in: a*(a+1)/2
  // per-bone maximum weight over the Bonamik instances
  const maxW = new Array(n).fill(0);
  for (const inst of comp.f468) {
    const core = inst.f3868;
    fn_143b11a20(core, dt, e);
    fn_143b02170(core, e);
    for (let i = 0; i < core.f3818; i++) { const w = core.f3810[i]; if (w > maxW[i]) maxW[i] = w; }
  }
  const interp = (rec) => {
    const T = [f(f(f(rec.f40[0] - rec.f10[0]) * alpha) + rec.f10[0]),
               f(rec.f10[1] + f(f(rec.f40[1] - rec.f10[1]) * alpha)),
               f(rec.f10[2] + f(f(rec.f40[2] - rec.f10[2]) * alpha))];
    const S = [fn_143b609b0(rec.f20[0], rec.f50[0], alpha), fn_143b609b0(rec.f20[1], rec.f50[1], alpha), fn_143b609b0(rec.f20[2], rec.f50[2], alpha)];
    const q = qnormSafe(fn_142d90ca0([0, 0, 0, 0], rec.f0, rec.f30, alpha), K_SMALL);
    return { q, t: T, s: S };
  };
  if (n > 0 && !(K_SMALL >= maxW[0])) {                                // bone 0
    const A = interp(comp.f4B0[0]);
    const N = xfMulInline(A, L[0].xf, '143b59bac');
    N.q = qnormSafe(N.q, K_SMALL);
    blendIntoInline(bones[0], N, maxW[0]);
  }
  for (let i = 1; i < n; i++) {                                        // ascending bone index: parents are final already
    if (K_SMALL >= maxW[i]) continue;
    const A = interp(comp.f4B0[i]);
    const M1 = xfMulInline(A, L[i].xf, '143b5a524');                   // delta * animated local (q not normalised)
    const N = xfMulInline(M1, bones[L[i].parent], '143b5a9f9');        // * parent component transform now in the buffer
    N.q = qnormSafe(N.q, K_SMALL);
    blendIntoInline(bones[i], N, maxW[i]);
  }
  for (const task of comp.f488) task.vf18(dt);                         // "after" sub-tasks
  return { alphaLinear: a, alpha, maxW };                              // (the exe returns nothing; for diagnostics)
}

// ---- 0x143b58f40 : UpdateAll(rcx = comp, xmm1 = dt) (profiling counters omitted) ---------------------------------
function fn_143b58f40(comp, dt, e = ext) {
  if (e.g_HighFpsEval !== 0 && comp.f498) return fn_143b590d0(comp, dt, e);
  for (const task of comp.f450) task.vf18(f(dt));
  return null;
}

// ---- 0x143b0e840 : request a forced reset -----------------------------------------------------------------------
function fn_143b0e840(core) { core.f33E9 = 1; }

// ---- 0x143b5b7e0 : Bonamik instance PreUpdate(rcx = inst, xmm1 = dt, xmm2 = time dilation; both unused) ----------
function fn_143b5b7e0(inst, dt, td, e = ext) {
  const owner = inst.f3870, core = inst.f3868;
  let mesh = owner.vf8();
  if (mesh && mesh.fA40 > 0) {
    const size = f(sqrt(mesh.fA40) + sqrt(mesh.fA40));                 // sqrtss ; addss x,x
    for (const lod of inst.f3890) {                                    // stride 0x20: +0x10 MinSize, +0x14, +0x18
      let m = lod.f10;
      if (size > m) { if (m >= inst.f3884) { e.fn_143b0ebd0(core, lod, lod.f14, lod.f18); m = lod.f10; } }
      if (!(size > m) && !(m >= inst.f3884)) e.fn_143b0ea70(core, 0, lod, lod.f14);
    }
    inst.f3884 = size;
  }
  if (owner) {
    mesh = owner.vf8();
    inst.f3879 = 0;
    if (mesh && mesh.f980 && mesh.f980.f60) {
      inst.f3879 = 1;
      const mode = inst.f3888 & 0xFF;
      let rendered;
      if (((mode - 1) & 0xFF) <= 1) rendered = (mesh.fA80 >>> 2) & 1;  // modes 1, 2: bRecentlyRendered
      else rendered = mesh.vfAE8(0) ? 1 : 0;
      inst.f3879 = rendered; inst.f3878 = rendered ? 0 : 1;
    }
  }
  const mv = owner.vf10();                                             // CharacterMovementComponent of the owner (or null)
  if (mv && ((mv.f1B0 >>> 27) & 1) && (mv.f4D4 & 0x20)) fn_143b0e840(core);   // bJustTeleported
  return e.fn_143b10ca0(core);
}

// ---- 0x143b633e0 : Bonamik instance Update(rcx = inst, xmm1 = dt) -------------------------------------------------
function fn_143b633e0(inst, dt, e = ext) {
  dt = f(dt);
  const cfg = e.globalConfig;
  if (!cfg.f55) return;
  const core = inst.f3868;
  if (!core) return;
  if (!inst.f387A) { e.fn_143b10a40(core); return; }
  if (!inst.f3879) dt = 0;
  e.fn_143b0efc0(core, cfg, dt);
  if (inst.f387B && K_1E4 > core.f36F8) inst.f387A = 0;
}

// ---- 0x144067e10 : mesh->vf[0xB98]() gate ---------------------------------------------------------------------------
function fn_144067e10(mesh) {
  const fl = mesh.f1B0 >>> 0;
  if ((fl >>> 28) & 1) return 0;
  if (!(fl & 1)) return 0;
  if (mesh.fB20 !== 0) return 0;
  return mesh.f990[mesh.f9B4].length > 0 ? 1 : 0;
}

// ---- 0x143b56cb0 : Finalize(rcx = comp). Engine side effects only (see md); nothing touches the bone buffers. ------
function fn_143b56cb0(comp) { comp.f429 = 1; }

// ---- 0x143b62bc0 : TickComponent(rcx = comp, xmm1 = dt), single-threaded rendition -----------------------------------
// (super tick, the task-graph / batch dispatch and ReadWeightCurves 0x143b63050 are not modelled: opts.readWeightCurves)
function fn_143b62bc0(comp, dt, e = ext, opts = {}) {
  dt = f(dt);
  if ((comp.f1B0 >>> 28) & 1) return null;
  const mesh = comp.f440;
  if (!mesh || !fn_144067e10(mesh)) return null;
  if (e.g_HighFpsEval !== 0) fn_143b5b990(comp, dt);                   // NOT conditioned on comp+0x498
  const td = opts.ownerTimeDilation === undefined ? ONE : f(opts.ownerTimeDilation);
  for (const task of comp.f450) task.vf10(dt, td);
  if (opts.readWeightCurves) opts.readWeightCurves(comp);
  comp.f42C = dt;
  const r = fn_143b58f40(comp, dt, e);
  fn_143b56cb0(comp);
  return r;
}

// ================================================================================================================
// Reference runner. Given parent indices and a callback evaluate(pose, evalDt) -> pose (the Bonamik core evaluation:
// it receives a copy of the bone buffer as it is at that moment and returns the buffer as the core leaves it), this
// reproduces the timer, the prev/cur buffers, the interpolation and the final written pose for each frame.
//   opts.driven   : per-bone 0/1 (the core's driven-bone bit set, core+0x37F0 / +0x3800); default: weights > 0
//   opts.weights  : per-bone weight (core+0x3810); default 1 for driven bones
//   opts.core     : a real core object from the linked evaluator (then driven / weights are read from it and
//                   opts.ext should provide fn_143b10010 / fn_143b11700 / fn_143afe170)
//   opts.before / opts.after : arrays of functions (dt, bones) run as the "before" / "after" sub-tasks (KineDriver)
// step(dt, animatedPose) -> { pose, didEval, evalDt, remaining, interval, alphaLinear, alpha }
function createRunner(parents, evaluate, opts = {}) {
  const n = parents.length;
  const bones = [];
  const mesh = { f1B0: 1, fB20: 0, f980: { f60: 1, f158: parents.map(p => ({ f8: p })), f160: n }, f990: [bones, []], f9B4: 0, fA40: 0, fA80: 4 };
  let core = opts.core;
  const e = Object.assign({}, ext, opts.ext || {});
  if (!core) {
    const weights = opts.weights ? opts.weights.map(f) : (opts.driven ? opts.driven.map(d => (d ? ONE : 0)) : new Array(n).fill(ONE));
    const driven = opts.driven || weights.map(w => (w > 0 ? 1 : 0));
    const words = new Uint32Array(Math.max(4, (n + 31) >> 5));
    driven.forEach((d, i) => { if (d) words[i >> 5] |= (1 << (i & 31)); });
    core = { f3840: 0, f37F0: new Uint32Array(4), f3800: words, f3810: weights, f3818: n, f410: 0, f8C4: [], f3578: [], f36F8: ONE,
             f3778: { f0: [], f8: 0 }, f3848: { vf60: () => bones } };
    if (!opts.ext) { e.fn_143b10010 = () => {}; e.fn_143b11700 = () => {}; e.fn_143afe170 = () => {}; }
  }
  const inst = { f3868: core, vf20: () => 0, vf10: () => {},
    vf18(evalDt) { const out = evaluate(bones.map(cloneXf), evalDt); for (let i = 0; i < n; i++) bones[i] = cloneXf(out[i]); } };
  const mk = fn => ({ vf18: dt => fn(dt, bones), vf10: () => {}, vf20: () => 1 });
  const before = (opts.before || []).map(mk), after = (opts.after || []).map(mk);
  const comp = { f1B0: 0, f440: mesh, f450: [...before, inst, ...after], f468: [inst], f478: before, f488: after,
                 f498: 1, f49C: K_DT30, f4A0: 0, f4A4: 0, f4A8: 0, f4AC: 0, f4AD: 0, f4B0: [], f428: 1, f429: 0, f42C: 0 };
  return {
    comp, mesh, inst, core,
    step(dt, pose) {
      for (let i = 0; i < n; i++) bones[i] = { q: pose[i].q.map(f), t: pose[i].t.map(f), s: pose[i].s.map(f) };
      fn_143b5b990(comp, dt);
      const didEval = comp.f4AC ? 1 : 0, evalDt = comp.f4A0;
      comp.f42C = f(dt);
      const r = fn_143b590d0(comp, dt, e);
      fn_143b56cb0(comp);
      return { pose: bones.map(cloneXf), didEval, evalDt, remaining: comp.f4A4, interval: comp.f4A8, alphaLinear: r.alphaLinear, alpha: r.alpha };
    },
  };
}
// convenience: frames = [{dt, pose}] -> array of step results
function simulate(frames, parents, evaluate, opts) { const r = createRunner(parents, evaluate, opts); return frames.map(fr => r.step(fr.dt, fr.pose)); }

module.exports = {
  link, ext, dot4, qnormSafe, safeRecip, xfMulInline, blendIntoInline,
  fn_142d801f0, fn_142d90ca0, fn_143b609b0, fn_143b5b990, fn_143b5c470, fn_143b618b0, fn_143b61b10,
  fn_143aefb30, fn_143af28e0, fn_143b11a20, fn_143b02170, fn_143afc920, fn_143b590d0, fn_143b58f40, fn_143b0e840,
  fn_143b5b7e0, fn_143b633e0, fn_144067e10, fn_143b56cb0, fn_143b62bc0, createRunner, simulate,
};

if (require.main === module) {
  const show = x => JSON.stringify(x, (k, v) => (typeof v === 'number' ? +v.toPrecision(7) : v));
  const A = { q: qnormSafe([0.1, 0.2, 0.3, 0.9], K_SMALL), t: [1, 2, 3], s: [1, 1, 1] };
  const B = { q: qnormSafe([0, 0.3826834, 0, 0.9238795], K_SMALL), t: [0, 5, 0], s: [1, 1, 1] };
  const rel = fn_142d801f0(A, ident(), B);
  console.log('fn_142d801f0', show(rel));
  console.log('  recomposed', show(xfMulInline(rel, B, 'test')), 'vs A', show(A));
  console.log('fn_142d90ca0', show(fn_142d90ca0([0, 0, 0, 0], A.q, B.q, 0.25)), show(fn_142d90ca0([0, 0, 0, 0], A.q, A.q, 0.25)));
  console.log('fn_143b609b0', fn_143b609b0(1, 2, 0.5), fn_143b609b0(1, -2, 0.5), fn_143b609b0(1, 1, 0.3));
  // timer + runner: 3-bone chain, bone 2 "simulated" (the fake evaluation pushes it 10 cm along +Z each evaluation)
  const parents = [-1, 0, 1];
  let k = 0;
  const evaluate = (pose, evalDt) => { k++; pose[2].t = [pose[2].t[0], pose[2].t[1], f(pose[2].t[2] + 10 * k)]; return pose; };
  const run = createRunner(parents, evaluate, { driven: [0, 0, 1] });
  const anim = [ident(), { q: [0, 0, 0, 1], t: [0, 0, 50], s: [1, 1, 1] }, { q: [0, 0, 0, 1], t: [0, 0, 80], s: [1, 1, 1] }];
  for (let fr = 0; fr < 8; fr++) {
    const r = run.step(1 / 60, anim);
    console.log('frame', fr, 'eval', r.didEval, 'evalDt', +r.evalDt.toFixed(6), 'rem', +r.remaining.toFixed(6), 'alpha', +r.alphaLinear.toFixed(4), '->', +r.alpha.toFixed(4), 'bone2.z', +r.pose[2].t[2].toFixed(4));
  }
  // setters, Blend, deferred blend, weights wrapper, gate, Update, PreUpdate, UpdateAll (flag off), Tick
  const c = run.comp;
  fn_143b61b10(c, 0, 0); fn_143b618b0(c, 0, 0); fn_143b618b0(c, 1, 0); fn_143b5c470(c, 0);
  console.log('force flag after setters', c.f4AD, 'interval0', c.f49C);
  const bonesT = [ident(), { q: [0, 0, 0, 1], t: [0, 0, 10], s: [1, 1, 1] }];
  const coreT = { f3840: 1, f3848: { vf60: () => bonesT }, f3578: [0.5], f36F8: 1, f33E9: 0, f410: 1, f8C4: [0], f3770: 0, f3768: [],
    f3778: { f8: 1, f0: [{ f278: 0, f2B8: 0, f248: 1, f120: [0, 0, 0, 1], f130: [0, 0, 0], f140: [1, 1, 1] }] } };
  fn_143af28e0(coreT); console.log('fn_143aefb30 (w = 0.5)', show(bonesT[1]), 'deferred flag', coreT.f3840);
  fn_143b02170(coreT); fn_143afc920(coreT);
  fn_143b11a20(coreT, 1 / 60, { fn_143b10010: () => {}, fn_143b11700: () => {} });
  const e2 = Object.assign({}, ext, { fn_143b0efc0: (co, cfg, d) => console.log('  CoreUpdate dt', d), fn_143b10a40: () => {}, fn_143b10ca0: () => 0 });
  const instT = { f3868: coreT, f387A: 1, f3879: 0, f387B: 0, f3884: f(1e20), f3888: 1, f3890: [],
    f3870: { vf8: () => ({ fA40: 0, f980: { f60: 1 }, fA80: 4 }), vf10: () => ({ f1B0: 1 << 27, f4D4: 0x20 }) } };
  fn_143b5b7e0(instT, 1 / 60, 1, e2); console.log('PreUpdate: rendered', instT.f3879, 'forced reset', coreT.f33E9);
  fn_143b633e0(instT, 1 / 60, e2);
  console.log('gate', fn_144067e10(run.mesh));
  c.f498 = 0; fn_143b58f40(c, 1 / 60); c.f498 = 1;
  console.log('tick', show(fn_143b62bc0(c, 1 / 60, Object.assign({}, ext, { fn_143b10010: () => {}, fn_143b11700: () => {}, fn_143afe170: () => {} }))));
}
