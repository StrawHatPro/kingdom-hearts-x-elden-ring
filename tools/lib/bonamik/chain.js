// Bonamik pass 2, part "chain": instruction-level transcription (KH3.exe) of
//   fn_143b0c600  WriteBack (particle -> bone transform)
//   fn_143b13b70  ChainWriteBack (recursive, twist removal against the target quat, wind direction p+0x1DC)
//   fn_143b192a0  ChainLink (recursive ParentChild constraint; cone solve injected through setCone)
//   fn_143af3f70  JointTwist
//   fn_143aec770 / fn_143ae7de0  link record setup
//   blk_143b0a327 joint-twist sign / rest angle pass of the core setup (block inside 0x143b07550)
//   fn_143b15520 / fn_143b155b0  hard-length flag pass of the core setup (called from 0x143b07550 at 143b0a2fb/143b0a30a)
//   fn_143b1a4d0  recursive length snap for attached chains
// See H_chain.md for the register-level notes. Field names = hex offsets (SUB_BRIEF conventions).
const { f, add, sub, mul, dot, cross, sqrt, rsqrt, qmul, qconj, qrot } = require('./m.js');

// ---- constants (address in the exe) ----------------------------------------------------------------------------
const C_1E8 = f(9.99999993922529e-9);        // 0x145C9C488 / immediate 0x322BCC77
const C_1E4 = f(0.00009999999747378752);     // 0x145CF35A8
const C_001 = f(0.009999999776482582);       // 0x145CADFC4
const C_M9999 = f(-0.9998999834060669);      // 0x14750BAA8
const C_SPD = f(0.004999999888241291);       // 0x145E2AB68
const C_008 = f(0.07999998331069946);        // 0x14750BA94 (0x3da3d708, NOT float(0.08))
const C_092 = f(0.9200000166893005);         // 0x14698F900
const C_084 = f(0.8400000333786011);         // 0x14750BA9C
const C_PI = f(3.1415927410125732);          // 0x145C9C4F8
const C_HPI = f(1.5707963705062866);         // 0x145C9C4F0
const C_2PI = f(6.2831854820251465);         // 0x145C9C508
const C_INV2PI = f(0.15915493667125702);     // 0x145C9C4C4
const AC0 = f(1.5707287788391113), AC1 = f(0.2121143937110901), AC2 = f(0.07426100224256516), AC3 = f(0.018729299306869507); // 0x14750BAA4, BA98, BA90, BA8C
const S1 = f(2.388985897994189e-8), S2 = f(0.0000027525561563379597), S3 = f(0.00019840874301735312), S4 = f(0.008333330973982811), S5 = f(0.1666666716337204); // 0x145C9C490, 49C, 4A4, 4B0, 4C8
const K1 = f(2.605161455448979e-7), K2 = f(0.0000247604948526714), K3 = f(0.0013888378161936998), K4 = f(0.04166663810610771); // 0x145C9C494, 4A0, 4AC, 4B8

let cone = () => { throw new Error('cone solve 0x143b166d0 not injected (setCone)'); };

// ---- shared inline blocks ----------------------------------------------------------------------------------------
// SSE quaternion normalise with identity fallback (same block at every site, e.g. 143b0c7a3..143b0c80c):
// ss = (x*x + z*z) + (y*y + w*w); y = rsqrtps + 2 Newton steps; ss >= 1e-8 ? q*y : (0,0,0,1)
function qnorm(q) {
  const ss = f(f(f(q[0] * q[0]) + f(q[2] * q[2])) + f(f(q[1] * q[1]) + f(q[3] * q[3])));
  if (!(C_1E8 <= ss)) return [0, 0, 0, 1];
  const y = rsqrt(ss);
  return [f(y * q[0]), f(y * q[1]), f(y * q[2]), f(y * q[3])];
}
// scalar vector normalise block (e.g. 143b0c8e1..143b0c974): l2 == 1 (or unordered) -> unchanged; l2 < 1e-8 -> FVector::ZeroVector
function vnorm(d) {
  const l2 = dot(d, d);
  if (l2 === 1 || l2 !== l2) return [d[0], d[1], d[2]];
  if (!(l2 >= C_1E8)) return [0, 0, 0];
  const y = rsqrt(l2);
  return [f(d[0] * y), f(d[1] * y), f(d[2] * y)];
}
// shortest arc from +X to the unit vector d (e.g. 143b0c978..143b0c9f6). x component is -0.0 (0x80000000).
function arcX(d) {
  if (!(d[0] >= C_M9999)) return [0, 1, 0, 0];                       // 0x145D4A2E0
  const s = sqrt(f(f(d[0] + d[0]) + 2));
  const inv = f(1 / s);
  return [-0, f(-f(d[2] * inv)), f(d[1] * inv), f(s * 0.5)];
}
// rot(q, +X) as coded (e.g. 143b13c84..143b13d03)
function axisX(q) {
  const m2y = f(f(-q[1]) + f(-q[1])), z2 = f(q[2] + q[2]);
  return [f(f(f(m2y * q[1]) - f(z2 * q[2])) + 1), f(f(z2 * q[3]) - f(m2y * q[0])), f(f(m2y * q[3]) + f(z2 * q[0]))];
}
// twist of (q * conj(ref)) about q's X axis, normalised (identity fallback)
function twistOf(q, ref) {
  const D = qmul(q, qconj(ref));
  const A = axisX(q);
  const k = f(f(f(A[1] * D[1]) + f(A[0] * D[0])) + f(A[2] * D[2]));
  return qnorm([f(k * A[0]), f(k * A[1]), f(k * A[2]), D[3]]);
}
// q with that twist removed: conj(Tw) * q   (NOT re-normalised)
function untwist(q, ref) { return qmul(qconj(twistOf(q, ref)), q); }
// polynomial acos (e.g. 143af49a0..143af4a22); c already clamped to [-1,1]
function acosP(c) {
  const a = Math.abs(c);
  const a2 = f(a * a);
  const poly = f(f(f(AC0 - f(a * AC1)) + f(a2 * AC2)) - f(f(a2 * a) * AC3));
  const r = f(sqrt(f(1 - a)) * poly);
  return (c >= 0) ? r : f(C_PI - r);
}
// clamp block: comiss c,-1 ; jae -> minss c,1 ; else c = -1
function clamp11(c) { return (c >= -1) ? (c < 1 ? c : 1) : -1; }
// FMath::SinCos shape (143af4ad0..143af4bfd) -> [sin, cos]
function sincos(x) {
  let q = f(x * C_INV2PI);
  q = (x >= 0) ? f(q + 0.5) : f(q - 0.5);
  let y = f(x - f(f(Math.trunc(q)) * C_2PI));
  let sign;
  if (y > C_HPI) { y = f(C_PI - y); sign = -1; }
  else if (!(y >= f(-C_HPI))) { y = f(f(-C_PI) - y); sign = -1; }
  else sign = 1;
  const y2 = f(y * y);
  let s = f(S2 - f(y2 * S1)); s = f(f(s * y2) - S3); s = f(f(s * y2) + S4); s = f(f(s * y2) - S5); s = f(f(s * y2) + 1); s = f(s * y);
  let c = f(K2 - f(y2 * K1)); c = f(f(c * y2) - K3); c = f(f(c * y2) + K4); c = f(f(c * y2) - 0.5); c = f(f(c * y2) + 1); c = f(c * sign);
  return [s, c];
}

// ---- 0x143b0c600 WriteBack(rcx core) ---------------------------------------------------------------------------
function fn_143b0c600(core) {
  const bones = core.f3848.vf60();
  const scene = core.f3778;
  for (let i = 0; i < scene.f8; i++) {
    const p = scene.f0[i];
    if (core.f804[p.f2B8] < 2) continue;
    if (p.f277 !== 0) continue;
    let ch = p.f198;
    if (p.f276 === 0) {
      if (ch == null) {                                              // dynamic tip (143b0c70b)
        const B = bones[p.f248];
        p.f20 = B.q.slice();
        B.t = [p.f0[0], p.f0[1], p.f0[2]];                           // 4th float = 0
        const par = p.f190;
        if (par == null) continue;
        if (p.f274 !== 0) continue;
        B.q = qnorm(bones[par.f248].q);                              // tip bone rotation := normalised PARENT bone rotation
        continue;
      }
    } else {
      if (ch == null) continue;
      if (ch.f276 !== 0) continue;
      if (ch.fBC === 0) continue;
    }
    // 143b0c842
    const P = p.f0;
    let d = [f(ch.f0[0] - P[0]), f(ch.f0[1] - P[1]), f(ch.f0[2] - P[2])];
    const l2 = f(f(f(d[1] * d[1]) + f(d[0] * d[0])) + f(d[2] * d[2]));
    if (!(l2 >= C_001)) {
      const gc = ch.f198;
      if (gc != null) d = [f(gc.f0[0] - P[0]), f(gc.f0[1] - P[1]), f(gc.f0[2] - P[2])];
    }
    d = vnorm(d);
    ch.f1D0 = [d[0], d[1], d[2]];
    p.f20 = arcX(d);                                                 // store at 143b0c9fa, then normalised:
    p.f20 = qnorm(p.f20);
    const B = bones[p.f248];
    B.t = [p.f0[0], p.f0[1], p.f0[2]];
    B.q = p.f20.slice();
  }
}

// ---- 0x143b13b70 ChainWriteBack(rcx core, rdx particle, r8 bones, r9 &parentQuat) --------------------------------
function fn_143b13b70(core, p, bones, parentQuat) {
  if (p.f275 !== 0) return;                                          // no recursion into the children either
  const B = bones[p.f248].q.slice();
  const v = p.f40;
  const sp = f(sqrt(f(f(f(v[0] * v[0]) + f(v[1] * v[1])) + f(v[2] * v[2]))) * C_SPD);
  const oneMinusSpeed = f(1 - (sp < 1 ? sp : 1));                    // [rbp+48h]; used only at 143b146bf and 143b150a5 (dead for Sora)
  let Q;
  if (p.f31C === 0 && p.f31D === 0) {
    Q = untwist(B, p.f160);                                          // 143b13c5f..143b13f5e
  } else {
    void oneMinusSpeed; void parentQuat;
    throw new Error('not transcribed: 143b13f63..143b14b2e (ground-attach / tag-11 smoothing variant)');
  }
  if (p.f31C !== 0) {
    const n = p.f88;
    if (f(f(f(n[0] * n[0]) + f(n[1] * n[1])) + f(n[2] * n[2])) > C_1E4)
      throw new Error('not transcribed: 143b14b79..143b1532e (ground-attach tilt)');
  }
  p.f1DC = qrot(Q, core.f1C0[p.f2B8]);                               // 143b1535a..143b154ad
  bones[p.f248].q = Q.slice();
  for (let i = 0; i < p.f1A8; i++) fn_143b13b70(core, p.f1B0[i].f8, bones, Q);
}

// ---- 0x143b192a0 ChainLink(xmm0 r, rdx link, r8 cfg = core+0x658) -----------------------------------------------
// The JS takes the core object for the third argument: [r8+g*4+0x53C] = core.fB94[g], [r8+g*4+0x5FC] = core.fC54[g].
function fn_143b192a0(r, L, core) {
  const p1 = L.f0;
  if (L.f28 != null) {
    const g = p1.f2B8;
    cone(r, L.f28, core.fB94[g], core.fC54[g], 1);                  // xmm0, rdx, xmm2, xmm3, byte [rsp+20h]
  } else {
    const p2 = L.f8;
    const pp = p1.f190;
    let Rq;
    if (pp != null && p1.fBC !== 0) Rq = (pp.f190 == null ? pp.f160 : pp.f20).slice();
    else Rq = p1.f160.slice();
    if (L.f26 !== 0) {                                               // hard length (143b19366)
      let dx = f(p2.f0[0] - p1.f0[0]), dy = f(p2.f0[1] - p1.f0[1]), dz = f(p2.f0[2] - p1.f0[2]);
      let l2 = f(f(f(dy * dy) + f(dx * dx)) + f(dz * dz));
      if (!(l2 >= 0)) { p1.f0 = [0, 0, 0]; p2.f0 = [0, 0, 0]; dx = dy = dz = 0; l2 = 0; }
      const len = sqrt(l2);
      if (len > 0) { const inv = f(1 / len); dx = f(dx * inv); dy = f(dy * inv); dz = f(dz * inv); }
      const k = f(L.f20 - len);
      dx = f(dx * k); dy = f(dy * k); dz = f(dz * k);
      if (p1.f276 !== 0) {
        if (p2.f276 === 0) p2.f0 = [f(dx + p2.f0[0]), f(dy + p2.f0[1]), f(dz + p2.f0[2])];
      } else if (p2.f276 !== 0) {
        p1.f0 = [f(p1.f0[0] - dx), f(p1.f0[1] - dy), f(p1.f0[2] - dz)];
      } else {
        p1.f0 = [f(p1.f0[0] - f(dx * C_008)), f(p1.f0[1] - f(dy * C_008)), f(p1.f0[2] - f(dz * C_008))];
        p2.f0 = [f(f(dx * C_092) + p2.f0[0]), f(f(dy * C_092) + p2.f0[1]), f(f(dz * C_092) + p2.f0[2])];
        p1.f58 = [f(f(dx * C_084) + p1.f58[0]), f(f(dy * C_084) + p1.f58[1]), f(f(dz * C_084) + p1.f58[2])];
      }
    }
    const d = vnorm([f(p2.f0[0] - p1.f0[0]), f(p2.f0[1] - p1.f0[1]), f(p2.f0[2] - p1.f0[2])]);
    const q = arcX(d);
    const Qn = untwist(q, Rq);
    if (p1.f190 != null && p1.fBC !== 0) {                           // 143b19548
      p2.f20 = Qn;
      if (p1.f1A8 === 1) p1.f20 = Qn.slice();
    } else {                                                         // 143b1992f
      p1.f20 = Qn;
      p2.f20 = Qn.slice();
    }
  }
  for (let i = 0; i < L.f8.f1A8; i++) fn_143b192a0(r, L.f8.f1B0[i], core);
}

// ---- 0x143af3f70 JointTwist(rcx core) ---------------------------------------------------------------------------
function fn_143af3f70(core) {
  const bones = core.f3848.vf60();
  for (let i = 0; i < core.f3778.f8; i++) {
    const scene = core.f3778;
    const p = scene.f0[i];
    const g = p.f2B8;
    if (core.f804[g] < 2) continue;
    const n = p.f310;
    if (n === 0) continue;
    const par = p.f190;
    let d, U;
    if (par != null && par.f31C === 0) {
      d = [f(p.f0[0] - par.f0[0]), f(p.f0[1] - par.f0[1]), f(p.f0[2] - par.f0[2])];
      U = qrot(bones[par.f248].q, core.f1C0[g]);
    } else {
      const ch = p.f198;
      if (ch == null) continue;
      if (ch.f31C !== 0) continue;
      d = [f(ch.f0[0] - p.f0[0]), f(ch.f0[1] - p.f0[1]), f(ch.f0[2] - p.f0[2])];
      U = qrot(bones[p.f248].q, core.f1C0[g]);
    }
    d = vnorm(d);
    U = vnorm(U);
    let S = [0, 0, 0];
    for (let k = 0; k < n; k++) {
      const nb = scene.f0[p.f2E0[k].f24C];                           // through the neighbour's particle index
      const e = vnorm([f(nb.f0[0] - p.f0[0]), f(nb.f0[1] - p.f0[1]), f(nb.f0[2] - p.f0[2])]);
      const sg = f(p.f300[k]);                                       // cvtsi2ss of the int sign
      const c = cross(d, e);
      S = [f(S[0] + f(c[0] * sg)), f(S[1] + f(c[1] * sg)), f(S[2] + f(c[2] * sg))];
    }
    S = vnorm(S);
    if (par != null) par.f1D0 = S.slice(); else p.f1D0 = S.slice();
    if (p.f198 == null) p.f1D0 = S.slice();
    const kk = dot(S, d);
    S = vnorm([f(S[0] - f(d[0] * kk)), f(S[1] - f(d[1] * kk)), f(S[2] - f(d[2] * kk))]);
    let c = dot(S, U);
    const X = cross(U, S);
    const t = f(f(f(X[0] * d[0]) + f(X[1] * d[1])) + f(X[2] * d[2]));
    c = clamp11(c);
    const ang = (t >= 0) ? acosP(c) : f(-acosP(c));
    const delta = f(ang - p.f318);
    if (!(Math.abs(delta) > C_1E4)) continue;
    const sc = sincos(f(delta * 0.5));
    const rq = [f(d[0] * sc[0]), f(d[1] * sc[0]), f(d[2] * sc[0]), sc[1]];
    let bi;
    if (par != null) { if (par.f275 !== 0) continue; bi = par.f248; }
    else { if (p.f275 !== 0) continue; bi = p.f248; }
    bones[bi].q = qnorm(qmul(rq, bones[bi].q));
  }
}

// ---- 0x143ae7de0 link record init (rcx link, rdx p1, r8 p2, xmm3 linkStr, [rsp+28h] float parentStr,
//                                     [rsp+30h] byte isParentChild, [rsp+38h] byte enableJointTwist) -----------------
function fn_143ae7de0(L, p1, p2, linkStr, parentStr, isPC, jointTwist) {
  L.f0 = p1; L.f8 = p2;
  const dz = f(p1.f0[2] - p2.f0[2]), dy = f(p1.f0[1] - p2.f0[1]), dx = f(p1.f0[0] - p2.f0[0]);
  L.f14 = linkStr; L.f10 = -1; L.f26 = 0; L.f24 = isPC;
  L.f20 = sqrt(f(f(f(dy * dy) + f(dx * dx)) + f(dz * dz)));
  L.f18 = parentStr;
  if (isPC === 0 && jointTwist !== 0) {
    if (p1.f310 < 4) { p1.f2E0[p1.f310] = p2; p1.f300[p1.f310] = 0; p1.f310++; }
    if (p2.f310 < 4) { p2.f2E0[p2.f310] = p1; p2.f300[p2.f310] = 0; p2.f310++; }
  }
  if (L.f24 !== 0) {
    p2.fBC = 1;
    const qc = p2.f20, qp = p1.f20;
    const D = qmul(qc, qconj(qp));
    const A = axisX(qc);
    const k = f(f(f(A[0] * D[0]) + f(A[1] * D[1])) + f(A[2] * D[2]));
    const tw = [f(A[0] * k), f(A[1] * k), f(A[2] * k), D[3]];
    const ss = f(f(f(f(tw[1] * tw[1]) + f(tw[0] * tw[0])) + f(tw[2] * tw[2])) + f(tw[3] * tw[3]));
    const N = qnorm(ss > C_1E4 ? tw : [0, 0, 0, 1]);                 // identity = 0x14871E410
    const w = N[3];
    const a = Math.abs(w), a2 = f(a * a);
    let ang = f(sqrt(f(1 - a)) * f(f(f(AC0 - f(a * AC1)) + f(a2 * AC2)) - f(f(a2 * a) * AC3)));
    if (!(w >= 0)) ang = f(C_PI - ang);
    p2.f314 = f(ang + ang);
  }
  L.f28 = null; L.f30 = null;
  return L;
}

// ---- 0x143aec770 add link (rcx scene, rdx LinkDesc, r8d particle index base) -> link or null ---------------------
// desc fields: f20 m_ParentId, f24 m_ChildId, f28 m_LinkType (byte), f2A m_EnableJointTwist, f30 m_LinkStr,
//              f34 m_LinkParentStr, f68 m_LinkBatchNo
function fn_143aec770(scene, desc, base) {
  const idx = scene.f48;
  if (scene.f790 === idx) return null;
  const p1 = scene.f0[desc.f20 + base];
  const p2 = scene.f0[desc.f24 + base];
  const type = desc.f28;
  const isPC = (type === 1) ? 1 : 0;
  scene.f48 = idx + 1;
  if (scene.f48 > scene.f4C) scene.f4C = scene.f48;                  // TArray grow 0x140A0F710 [U]; exact new max not modelled
  const L = scene.f40[idx] = (scene.f40[idx] || {});
  fn_143ae7de0(L, p1, p2, desc.f30, desc.f34, isPC, desc.f2A);
  if (type === 1) {
    if (p1.f1A8 < 4) { p1.f1B0[p1.f1A8] = L; p1.f1A8++; }
    if (p1.f198 !== p2) {
      p1.f198 = p2;
      const d = vnorm([f(p2.f0[0] - p1.f0[0]), f(p2.f0[1] - p1.f0[1]), f(p2.f0[2] - p1.f0[2])]);
      const q = arcX(d);
      const Aq = p1.f120.slice();                                    // read before the store to +0x20
      p1.f20 = q;
      p1.f20 = untwist(q, Aq);
    }
  }
  L.f10 = desc.f68;
  return L;
}

// ---- 0x143b15520(rcx core, rdx particle) -> bool ; 0x143b155b0(rcx core, rdx particle) --------------------------
function fn_143b15520(core, p) {
  if (p.fBD !== 0) return 1;
  for (let i = 0; i < p.f1A8; i++) {
    const c = p.f1B0[i].f8;
    if (c.f276 !== 0) return 1;
    if (c.fBD !== 0) return 1;
    if (fn_143b15520(core, c)) return 1;
  }
  return 0;
}
function fn_143b155b0(core, p) {
  for (let i = 0; i < p.f1A8; i++) { p.f1B0[i].f26 = 1; fn_143b155b0(core, p.f1B0[i].f8); }
}
// block 143b0a2af..143b0a324 of the core setup 0x143b07550
function blk_143b0a2af(core) {
  if (core.f658 < 0x960) return;
  if (core.f664 !== 0) return;
  const scene = core.f3778;
  for (let i = 0; i < scene.f750; i++) {
    const R = scene.f0[scene.f748[i]];
    if (!fn_143b15520(core, R)) fn_143b155b0(core, R);
  }
}

// ---- block 143b0a327..143b0ac04 of the core setup 0x143b07550: joint-twist signs (+0x300) and rest angle (+0x318) --
// count = r12d (number of particles of this asset)
function blk_143b0a327(core, count) {
  for (let i = 0; i < count; i++) {
    const p = core.f3778.f0[i];
    if (p.f310 === 0) continue;
    const g = p.f2B8;
    const par = p.f190;
    let d, U;
    if (par != null) {
      U = qrot(par.f120, core.f1C0[g]);
      d = [f(p.f0[0] - par.f0[0]), f(p.f0[1] - par.f0[1]), f(p.f0[2] - par.f0[2])];
    } else {
      const ch = p.f198;
      if (ch == null) continue;
      U = qrot(p.f120, core.f1C0[g]);
      d = [f(ch.f0[0] - p.f0[0]), f(ch.f0[1] - p.f0[1]), f(ch.f0[2] - p.f0[2])];
    }
    d = vnorm(d);                                                    // U is NOT normalised here
    let S = [0, 0, 0];
    for (let k = 0; k < p.f310; k++) {
      const nb = p.f2E0[k];                                          // direct pointer here
      const e = vnorm([f(nb.f0[0] - p.f0[0]), f(nb.f0[1] - p.f0[1]), f(nb.f0[2] - p.f0[2])]);
      const c = cross(d, e);
      const t = f(f(f(c[0] * U[0]) + f(c[1] * U[1])) + f(c[2] * U[2]));
      let sg;
      if (t >= 0) { p.f300[k] = 1; sg = 1; } else { p.f300[k] = -1; sg = -1; }
      S = [f(S[0] + f(c[0] * sg)), f(S[1] + f(c[1] * sg)), f(S[2] + f(c[2] * sg))];
    }
    S = vnorm(S);
    const kk = dot(S, d);
    S = vnorm([f(S[0] - f(d[0] * kk)), f(S[1] - f(d[1] * kk)), f(S[2] - f(d[2] * kk))]);
    let c = dot(S, U);
    const X = cross(U, S);
    const t = f(f(f(X[0] * d[0]) + f(d[1] * X[1])) + f(d[2] * X[2]));
    c = clamp11(c);
    p.f318 = (t >= 0) ? acosP(c) : f(-acosP(c));
  }
}

// ---- 0x143b1a4d0(xmm0 r (unused, passed on), rdx link) ----------------------------------------------------------
function fn_143b1a4d0(r, L) {
  const p1 = L.f0, p2 = L.f8;
  const dx = f(p1.f0[0] - p2.f0[0]), dy = f(p1.f0[1] - p2.f0[1]), dz = f(p1.f0[2] - p2.f0[2]);
  const l2 = f(f(f(dy * dy) + f(dx * dx)) + f(dz * dz));
  if (l2 > C_1E4) {
    const len = sqrt(l2);
    const inv = f(1 / len);
    const e = f(len - L.f20);
    if (p2.f276 === 0)
      p2.f0 = [f(f(f(dx * inv) * e) + p2.f0[0]), f(f(f(inv * dy) * e) + p2.f0[1]), f(f(f(inv * dz) * e) + p2.f0[2])];
  }
  for (let i = 0; i < p2.f1A8; i++) fn_143b1a4d0(r, p2.f1B0[i]);
}

module.exports = {
  setCone: fn => { cone = fn; },
  fn_143b0c600, fn_143b13b70, fn_143b192a0, fn_143af3f70, fn_143ae7de0, fn_143aec770, fn_143b1a4d0,
  fn_143b15520, fn_143b155b0, blk_143b0a2af, blk_143b0a327,
  _helpers: { qnorm, vnorm, arcX, axisX, twistOf, untwist, acosP, clamp11, sincos },
};

// ---- self-test ---------------------------------------------------------------------------------------------------
if (require.main === module) {
  const mkP = (i, pos, bone, kin) => ({
    f0: pos.slice(), fC: pos.slice(), f20: [0, 0, 0, 1], f30: [0, 0, 0, 1], f40: [1, 2, 3], f58: [0, 0, 0], f88: [0, 0, 0],
    fBC: 0, fBD: 0, f120: [0, 0, 0, 1], f160: [0, 0, 0, 1], f190: null, f198: null, f1A8: 0, f1B0: [null, null, null, null],
    f1D0: [0, 0, 0], f1DC: [0, 0, 0], f248: bone, f24C: i, f274: 0, f275: 0, f276: kin, f277: 0, f2B8: 0,
    f2E0: [null, null, null, null], f300: [0, 0, 0, 0], f310: 0, f314: 0, f318: 0, f31C: 0, f31D: 0,
  });
  const rq = [f(0.1), f(0.2), f(-0.1), f(0.9695360)];
  const bones = []; for (let i = 0; i < 6; i++) bones.push({ q: qnorm(rq), t: [0, 0, 0], s: [1, 1, 1] });
  const P = [mkP(0, [0, 0, 0], 0, 1), mkP(1, [9, 2, -1], 1, 0), mkP(2, [17, 5, -4], 2, 0),
             mkP(3, [0, 10, 0], 3, 1), mkP(4, [8, 12, -2], 4, 0), mkP(5, [15, 14, -5], 5, 0)];
  for (const p of P) { p.f120 = qnorm(rq); p.f160 = qnorm(rq); p.f20 = qnorm(rq); }
  P[1].f190 = P[0]; P[2].f190 = P[1]; P[4].f190 = P[3]; P[5].f190 = P[4];
  const scene = { f0: P, f8: P.length, f40: [], f48: 0, f4C: 0, f790: 64, f748: [0, 3], f750: 2 };
  const core = { f3778: scene, f3848: { vf60: () => bones }, f804: [2], f1C0: [[0, 0, 1]], fB94: [1], fC54: [1], f658: 2401, f664: 0 };
  const desc = (a, b, type, jt) => ({ f20: a, f24: b, f28: type, f2A: jt, f30: f(0.8), f34: f(0.5), f68: 0 });
  const links = [desc(0, 1, 1, 0), desc(1, 2, 1, 0), desc(3, 4, 1, 0), desc(4, 5, 1, 0), desc(1, 4, 0, 1), desc(2, 5, 0, 1)]
    .map(dsc => fn_143aec770(scene, dsc, 0));
  console.log('links', links.map(l => [l.f20, l.f24, l.f26]), 'twist angles +0x314', P.map(p => p.f314));
  blk_143b0a2af(core);
  blk_143b0a327(core, P.length);
  console.log('hard flags', links.map(l => l.f26), 'signs', P.map(p => p.f300.slice(0, p.f310)), 'rest +0x318', P.map(p => p.f318));
  P[2].f0 = [16, 7, -3]; P[5].f0 = [14, 15, -3]; P[1].f0 = [9, 3, -1];
  module.exports.setCone((r, c, a, b, e) => { throw new Error('unexpected'); });
  for (const R of [P[0], P[3]]) for (let i = 0; i < R.f1A8; i++) fn_143b192a0(1, R.f1B0[i], core);
  console.log('after ChainLink', P.map(p => p.f0.concat(p.f20)));
  fn_143b1a4d0(1, links[0]);
  fn_143b0c600(core);
  console.log('after WriteBack', bones.map(b => b.q.concat(b.t)));
  for (const R of [P[0], P[3]]) fn_143b13b70(core, R, bones, R.f160);
  console.log('after ChainWriteBack', bones.map(b => b.q), P.map(p => p.f1DC));
  fn_143af3f70(core);
  console.log('after JointTwist', bones.map(b => b.q), P.map(p => p.f1D0));
}
