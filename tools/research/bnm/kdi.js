// Bonamik pass 2, part "kdi": KDI constraint (ESQEX_Bonamik_LinkType 4), Pin constraint (type 3), attached chains.
// Transcribed at instruction level from KH3.exe; see H_kdi.md for the addresses and the marks.
// Data layout used here (field = 'f' + hex offset):
//   scene.f0   particles (array), scene.f8 count
//   scene.f758 pin records (array, stride 0x28), scene.f760 count, scene.f764 capacity
//       pin record : f0 parent particle, f8 child particle, f10 vec3 (child in the parent frame, setup pose),
//                    f1C m_PinStr, f20 m_PinRadius
//   scene.f768 KDI records (array, stride 0x18), scene.f770 count, scene.f774 capacity
//       KDI record : f0 driven particle (the link CHILD), f8 driver entries (array, stride 0x40), f10 count
//       driver entry: f0 driver particle (the link PARENT), f8 vec3 (child in the driver frame, setup pose),
//                    f20 quat (conj(driver.f20) * child.f20, setup pose), f30 m_KdiTrans / sum, f34 m_KdiRot / sum
//   particle: f0 pos, f20 quat, f120 quat / f130 vec3 / f140 vec3 = current animated bone FTransform, f160 target
//             quat, f190 parent, f198 child, f1A8 number of ParentChild links, f1B0[4] links, f238 pin record,
//             f240 KDI record, f276 kinematic, f310 number of twist neighbours
//   link (stride 0x38): f0 parent particle, f8 child particle, f20 rest length
'use strict';
const { f, add, sub, mul, dot, cross, sqrt, rsqrt, qmul, qconj, qrot } = require('./m.js');

const EPS8 = f(1e-8);            // 0x322BCC77 (0x145C9C488 and immediates)
const EPS4 = f(1e-4);            // 0x38D1B717 (0x145CF35A8)
const M9999 = f(-0.9999);        // 0xBF7FF972 (0x14750BAA8)
const P9999 = f(0.9999);         // 0x3F7FF972 (0x145CF35B8)

// ---- injected functions (transcribed by other parts) -------------------------------------------------------------
const deps = {
  // ChainLink 0x143b192a0 (xmm0 r, rdx link, r8 = core+0x658).  JS: chainLink(r, link, cfg)
  chainLink: () => { throw new Error('kdi.js: chainLink (0x143b192a0) not injected'); },
  // SphereSnap 0x143b1b830 (rcx out vec3*, rdx x = pointer to a vec3 that is moved, r8 centre vec3*, xmm3 radius,
  // [rsp+20] strength).  JS: sphereSnap(out, x, centre, radius, strength)
  sphereSnap: () => { throw new Error('kdi.js: sphereSnap (0x143b1b830) not injected'); },
  // 0x143b03380 (rcx particle): joint-twist correction of particle.f20 from the twist neighbours of particle.f198.
  // Default below = the exe's two early exits; the body is not transcribed here (dead for Sora's KDI driver).
  twistFix: null,
  // CRT float functions used by 0x142d90ca0 (0x14587198E = acosf, 0x1458719A0 = sinf)
  acosf: x => f(Math.acos(x)),
  sinf: x => f(Math.sin(x)),
};
function setDeps(d) { for (const k of Object.keys(d)) if (k in deps) deps[k] = d[k]; return module.exports; }

// ---- small shared shapes -----------------------------------------------------------------------------------------
// componentwise scale of the bone FTransform, inverted only when min(|sx|,|sy|,|sz|) >= 1e-4 (143aecdc5..143aece2e,
// identical code at 143aed6a5..143aed6ec).  minss a,b = (a < b) ? a : b.
function invScale(s) {
  const ax = Math.abs(s[0]), ay = Math.abs(s[1]), az = Math.abs(s[2]);
  let m = (ay < ax) ? ay : ax;                 // minss xmm2(|sy|), xmm0(|sx|)
  m = (m < az) ? m : az;                       // minss xmm2, xmm0(|sz|)
  if (m >= EPS4) return [f(1 / s[0]), f(1 / s[1]), f(1 / s[2])];   // comiss m,1e-4 ; jb skip
  return [s[0], s[1], s[2]];                   // NOT inverted (and not replaced by 1): as coded
}
// stock FVector::GetSafeNormal(1e-8) shape: == 1 -> unchanged, < 1e-8 -> FVector::ZeroVector (0x149B5C798), else
// * InvSqrt (rsqrtss + 2 Newton steps).  L2 = (y*y + x*x) + z*z.
function safeNormal(d) {
  const L2 = f(f(f(d[1] * d[1]) + f(d[0] * d[0])) + f(d[2] * d[2]));
  if (L2 === 1 || L2 !== L2) return [d[0], d[1], d[2]];            // ucomiss ; je (also taken when unordered)
  if (!(L2 >= EPS8)) return [0, 0, 0];                             // comiss L2,1e-8 ; jae normalise
  const y = rsqrt(L2);
  return [f(d[0] * y), f(d[1] * y), f(d[2] * y)];
}
// shortest arc from +X to the unit vector v, as coded (143af60f6..143af6163)
function arcFromX(v) {
  if (!(v[0] >= M9999)) return [0, 1, 0, 0];                       // comiss v.x,-0.9999 ; jae normal ; 0x145D4A2E0
  const s = sqrt(f(f(v[0] + v[0]) + 2));
  const inv = f(1 / s);
  const w = f(s * 0.5);
  return [-0, f(-f(inv * v[2])), f(inv * v[1]), w];
}
// stock VectorNormalizeQuaternion (VectorNormalizeSafe with fallback (0,0,0,1) = 0x149D52880, 0.5 = 0x149D52890)
function qnormSafe(v) {
  const s0 = f(v[0] * v[0]), s1 = f(v[1] * v[1]), s2 = f(v[2] * v[2]), s3 = f(v[3] * v[3]);
  const N = f(f(s1 + s3) + f(s0 + s2));
  if (!(EPS8 <= N)) return [0, 0, 0, 1];                           // cmpleps 1e-8 <= N
  const y = rsqrt(N);                                              // rsqrtps + 2 Newton steps
  return [f(y * v[0]), f(y * v[1]), f(y * v[2]), f(y * v[3])];
}

// ---- setup -------------------------------------------------------------------------------------------------------
// 0x143aed5a0 (rcx scene, edx parent particle index, r8d child particle index, xmm3 m_PinRadius, [rsp+28h] m_PinStr)
function fn_143aed5a0(scene, parentIdx, childIdx, pinRadius, pinStr) {
  const P = scene.f0[parentIdx], C = scene.f0[childIdx];
  const rec = {};
  scene.f758.push(rec); scene.f760 = f760(scene) + 1;
  rec.f0 = P; rec.f8 = C;
  const s = invScale(P.f140);
  const r = qrot(qconj(P.f20), sub(C.f0, P.f0));                   // 143aed6f0..143aed7ed
  rec.f10 = [f(r[0] * s[0]), f(r[1] * s[1]), f(r[2] * s[2])];
  rec.f20 = pinRadius;                                             // 143aed82a
  rec.f1C = pinStr;                                                // 143aed838
  C.f238 = scene.f758[scene.f760 - 1];
}
const f760 = scene => scene.f760 | 0;

// 0x143aecd20 (rcx KDI record, rdx driver particle, xmm2 m_KdiTrans, xmm3 m_KdiRot)
function fn_143aecd20(rec, D, kdiTrans, kdiRot) {
  const C = rec.f0;
  const c = qconj(D.f20);                                          // [rsp+30h] = (-x,-y,-z,w)
  const s = invScale(D.f140);
  const r = qrot(c, sub(C.f0, D.f0));                              // 143aece49..143aecf2a
  const local = [f(r[0] * s[0]), f(r[1] * s[1]), f(r[2] * s[2])];
  const rel = qmul(c, C.f20);                                      // 143aecf17..143aecf88
  const e = { f0: D, f8: local, f20: rel, f30: kdiTrans, f34: kdiRot };
  rec.f8.push(e); rec.f10 = (rec.f10 | 0) + 1;
}

// 0x143afb8e0 (rcx KDI record): divide f30 by the sum of f30 and f34 by the sum of f34 (each only if its sum > 0)
function fn_143afb8e0(rec) {
  const n = rec.f10 | 0;
  if (n >= 8) throw new Error('not transcribed: 143afb945..143afba5c (8 or more drivers: 4-lane packed sums)');
  let sT = 0, sR = 0;                                              // xmm2, xmm1
  for (let i = 0; i < n; i++) { sT = f(sT + rec.f8[i].f30); sR = f(sR + rec.f8[i].f34); }
  for (let i = 0; i < n; i++) {
    const e = rec.f8[i];
    if (sT > 0) e.f30 = f(e.f30 / sT);                             // comiss sT,0 ; jbe skip
    if (sR > 0) e.f34 = f(e.f34 / sR);
  }
}

// The Pin/KDI block of core setup 0x143b07550 (143b09d04..143b0a03d).
// links: the m_Links rows of ONE asset; a row is either the plain row of sora_bonamik.json ({parent, child, type,
// pinRadius, pinStr, kdiTrans, kdiRot}) or a raw descriptor ({f20, f24, f28, f74, f78, f7C, f80}).
// base = particle index of the asset's first body (0 for a single asset); total = number of particles.
// Call it once per asset in asset order, AFTER every particle and every link/cone record has been built (the exe
// runs the whole block after the link-record loop), then call fn_143b07550_kdiNormalize(scene) once.
const TYPE = { Lateral: 0, ParentChild: 1, Bending: 2, PinConstraint: 3, KdiConstraint: 4, ReactionTransfer: 5, InverseFreeConstraint: 6 };
function row(l) {
  const t = ('f28' in l) ? l.f28 : (typeof l.type === 'string' ? TYPE[l.type] : l.type);
  return {
    parent: ('f20' in l) ? l.f20 : l.parent, child: ('f24' in l) ? l.f24 : l.child, type: t,
    pinRadius: f(('f74' in l) ? l.f74 : l.pinRadius), pinStr: f(('f78' in l) ? l.f78 : l.pinStr),
    kdiTrans: f(('f7C' in l) ? l.f7C : l.kdiTrans), kdiRot: f(('f80' in l) ? l.f80 : l.kdiRot),
  };
}
function fn_143b07550_kdiInit(scene) {                             // 143b09ddb..143b09e0a (arrays emptied, reserved)
  scene.f758 = []; scene.f760 = 0; scene.f768 = []; scene.f770 = 0;
}
function fn_143b07550_kdiLinks(scene, links, base) {               // 143b09e90..143b09fca
  const total = scene.f8 !== undefined ? scene.f8 : scene.f0.length;
  base = base | 0;
  for (const l0 of links) {
    const l = row(l0);
    const pi = (l.parent + base) >>> 0, ci = (l.child + base) >>> 0;
    if (!(pi < total) || !(ci < total)) continue;                  // unsigned compares (jae skip)
    if (l.type === 3) fn_143aed5a0(scene, pi, ci, l.pinRadius, l.pinStr);
    else if (l.type === 4) {
      const D = scene.f0[pi], C = scene.f0[ci];
      let rec = C.f240;
      if (!rec) {                                                  // first KDI link of this child
        rec = { f0: C, f8: [], f10: 0 };
        scene.f768.push(rec); scene.f770 = (scene.f770 | 0) + 1;
        C.f240 = rec;
        C.f276 = 1;                                                // 143b09f92: the driven particle becomes kinematic
      }
      fn_143aecd20(rec, D, l.kdiTrans, l.kdiRot);
    }
  }
}
function fn_143b07550_kdiNormalize(scene) {                        // 143b09ff9..143b0a03d
  for (let i = 0; i < (scene.f770 | 0); i++) fn_143afb8e0(scene.f768[i]);
}
// convenience: one asset
function setupPinKdi(scene, links, base) {
  fn_143b07550_kdiInit(scene); fn_143b07550_kdiLinks(scene, links, base | 0); fn_143b07550_kdiNormalize(scene);
  return scene;
}

// ---- PreStep part (0x143aee280, 143aeebcf..143aeecb6) ------------------------------------------------------------
function fn_143aee280_kdiRefresh(scene) {
  for (let i = 0; i < (scene.f760 | 0); i++) {
    const P = scene.f758[i].f0;                                    // pin PARENT
    const par = P.f190;
    P.f160 = P.f120.slice();
    if (par) par.f160 = par.f120.slice();
  }
  for (let i = 0; i < (scene.f770 | 0); i++) {
    const rec = scene.f768[i];
    for (let k = 0; k < rec.f10; k++) { const D = rec.f8[k].f0; D.f160 = D.f120.slice(); }
  }
}

// ---- per step ----------------------------------------------------------------------------------------------------
// 0x142d90ca0 (rcx out, rdx a, r8 b, xmm3 alpha): stock FQuat::Slerp_NotNormalized
function fn_142d90ca0(out, a, b, alpha) {
  let raw = f(f(f(a[1] * b[1]) + f(a[0] * b[0])) + f(a[2] * b[2]));
  raw = f(raw + f(a[3] * b[3]));
  const ad = Math.abs(raw);
  let s0, s1;
  if (ad >= P9999) { s0 = f(1 - alpha); s1 = alpha; }              // comiss |raw|,0.9999 ; jae linear
  else {
    let x = -1;
    if (!(ad < -1)) x = (ad < 1) ? ad : 1;                         // comiss ad,-1 ; jb keep -1 ; minss ad,1
    if (ad !== ad) x = -1;                                         // unordered takes the jb
    const om = deps.acosf(x);
    const inv = f(1 / deps.sinf(om));
    s0 = f(deps.sinf(f(f(1 - alpha) * om)) * inv);
    s1 = f(deps.sinf(f(om * alpha)) * inv);
  }
  if (!(raw >= 0)) s1 = f(-s1);                                    // comiss raw,0 ; jae keep
  const r = [f(f(s1 * b[0]) + f(s0 * a[0])), f(f(s1 * b[1]) + f(s0 * a[1])), f(f(s1 * b[2]) + f(s0 * a[2])), f(f(s1 * b[3]) + f(s0 * a[3]))];
  out[0] = r[0]; out[1] = r[1]; out[2] = r[2]; out[3] = r[3];
  return out;
}

// 0x143b03380 (rcx particle): only the two early exits (143b03380 head) are transcribed here.
function fn_143b03380(p) {
  if (deps.twistFix) return deps.twistFix(p);
  const c = p.f198;
  if (!c) return;
  if ((c.f310 | 0) === 0) return;
  throw new Error('not transcribed: 143b03380 body (driver whose child has twist neighbours); inject twistFix');
}

// 0x143af5f00 (rcx KDI record, rdx out quat) -> rax = out.
// Side effects: for every driver D of the record D.f20 is rebuilt from the CURRENT particle positions.
function fn_143af5f00(rec, out) {
  const C = rec.f0;
  out[0] = C.f20[0]; out[1] = C.f20[1]; out[2] = C.f20[2]; out[3] = C.f20[3];
  for (let i = 0; i < (rec.f10 | 0); i++) {
    const e = rec.f8[i];
    const D = e.f0;
    if (D.f198) {                                                  // 143af601f: driver has a child particle
      const v = safeNormal(sub(D.f198.f0, D.f0));
      D.f20 = arcFromX(v);                                         // 143af6557
    } else if (D.f190) {                                           // 143af617d: no child, but a parent particle
      throw new Error('not transcribed: 143af617d..143af6552 (KDI driver without child particle)');
    }                                                              // neither: D.f20 is used as it is
    // 143af655b: remove from D.f20 the twist about its own X axis relative to D.f160
    const q = D.f20, tq = D.f160;
    const c = [f(-tq[0]), f(-tq[1]), f(-tq[2]), tq[3]];
    const z2 = f(q[2] + q[2]);
    const my2 = f(f(-q[1]) + f(-q[1]));
    const ay = f(f(q[3] * z2) - f(q[0] * my2));
    const ax = f(f(f(q[1] * my2) - f(q[2] * z2)) + 1);
    const az = f(f(q[3] * my2) + f(q[0] * z2));
    const p = qmul(q, c);
    const dp = f(f(f(p[1] * ay) + f(p[0] * ax)) + f(p[2] * az));
    const n = qnormSafe([f(dp * ax), f(dp * ay), f(dp * az), p[3]]);
    D.f20 = qmul([f(-n[0]), f(-n[1]), f(-n[2]), n[3]], q);         // 143af67c0
    fn_143b03380(D);                                               // 143af67c4
    const T = qmul(D.f20, e.f20);                                  // 143af67c9..143af6837
    const tmp = fn_142d90ca0([0, 0, 0, 0], out, T, e.f34);         // 143af6842
    const r = qnormSafe(tmp);                                      // 143af6860..143af68ce
    out[0] = r[0]; out[1] = r[1]; out[2] = r[2]; out[3] = r[3];
  }
  return out;
}

// The KDI pass inlined in Step 0x143b0cb60 (143b0d7fb..143b0df90).  Runs once per sub-step, after the collision
// iterations and before 0x143b15610.
function fn_143b0cb60_kdiPass(scene) {
  for (let i = 0; i < (scene.f770 | 0); i++) {
    const rec = scene.f768[i];
    const C = rec.f0;
    const q = fn_143af5f00(rec, [0, 0, 0, 0]);
    C.f20 = [q[0], q[1], q[2], q[3]];                              // 143b0d843
    let ax = 0, ay = 0, az = 0;
    for (let k = 0; k < (rec.f10 | 0); k++) {                      // 143b0d8a0 (x4 unrolled) / 143b0ddfc (tail)
      const e = rec.f8[k], D = e.f0, w = e.f30;
      const v = [f(D.f140[0] * e.f8[0]), f(D.f140[1] * e.f8[1]), f(D.f140[2] * e.f8[2])];
      const r = qrot(D.f20, v);
      ax = f(ax + f(f(r[0] + D.f0[0]) * w));
      ay = f(ay + f(f(r[1] + D.f0[1]) * w));
      az = f(az + f(f(r[2] + D.f0[2]) * w));
    }
    C.f0 = [ax, ay, az];                                           // 143b0df68
    C.f160 = C.f20.slice();                                        // 143b0df7e
  }
}

// 0x143af6960 (rcx pin record, rdx out vec3) -> rax = out: parent.pos + rot(parent.f20, parent.f140 * rec.f10)
function fn_143af6960(rec, out) {
  const P = rec.f0;
  const v = [f(P.f140[0] * rec.f10[0]), f(P.f140[1] * rec.f10[1]), f(P.f140[2] * rec.f10[2])];
  const r = qrot(P.f20, v);
  out[0] = f(r[0] + P.f0[0]); out[1] = f(r[1] + P.f0[1]); out[2] = f(r[2] + P.f0[2]);
  return out;
}

// 0x143b1a4d0 (xmm0 r [only passed on], rdx link): hard rest length for the child, recursive
function fn_143b1a4d0(r, link) {
  const p1 = link.f0, p2 = link.f8;
  const dx = f(p1.f0[0] - p2.f0[0]), dy = f(p1.f0[1] - p2.f0[1]), dz = f(p1.f0[2] - p2.f0[2]);
  const L2 = f(f(f(dy * dy) + f(dx * dx)) + f(dz * dz));
  if (L2 > EPS4) {                                                 // comiss L2,1e-4 ; jbe skip
    const L = sqrt(L2);
    const inv = f(1 / L);
    const e = f(L - link.f20);
    if (!p2.f276) {
      p2.f0 = [f(f(f(dx * inv) * e) + p2.f0[0]), f(f(f(inv * dy) * e) + p2.f0[1]), f(f(f(inv * dz) * e) + p2.f0[2])];
    }
  }
  for (let k = 0; k < (p2.f1A8 >>> 0); k++) fn_143b1a4d0(r, p2.f1B0[k]);
}

// 0x143b15610 (xmm0 r, rdx core, r8 = Step's 2nd argument [unused], r9 = core+0x658)
// JS: cfg is whatever the injected chainLink expects for its 3rd argument (the exe passes core+0x658).
function fn_143b15610(r, core, arg3, cfg) {
  const scene = core.f3778;
  for (let i = 0; i < (scene.f760 | 0); i++) {                     // 143b15660: pinned particles
    const p = scene.f758[i].f8;
    const c = fn_143af6960(p.f238, [0, 0, 0]);
    const centre = [c[0], c[1], c[2]];
    deps.sphereSnap([0, 0, 0], p.f0, centre, 0, 1);                // rdx = particle (+0 = pos), xmm3 = 0, [rsp+20] = 1
    for (let k = 0; k < (p.f1A8 >>> 0); k++) deps.chainLink(r, p.f1B0[k], cfg);
  }
  for (let i = 0; i < (scene.f770 | 0); i++) {                     // 143b15710: KDI driven particles
    const p = scene.f768[i].f0;
    for (let k = 0; k < (p.f1A8 >>> 0); k++) deps.chainLink(r, p.f1B0[k], cfg);
  }
  for (let i = 0; i < (scene.f760 | 0); i++) {                     // 143b15780
    const p = scene.f758[i].f8;
    for (let k = 0; k < (p.f1A8 >>> 0); k++) fn_143b1a4d0(r, p.f1B0[k]);
  }
  for (let i = 0; i < (scene.f770 | 0); i++) {                     // 143b157e0
    const p = scene.f768[i].f0;
    for (let k = 0; k < (p.f1A8 >>> 0); k++) fn_143b1a4d0(r, p.f1B0[k]);
  }
}

module.exports = {
  setDeps,
  fn_143aed5a0, fn_143aecd20, fn_143afb8e0,
  fn_143b07550_kdiInit, fn_143b07550_kdiLinks, fn_143b07550_kdiNormalize, setupPinKdi,
  fn_143aee280_kdiRefresh,
  fn_142d90ca0, fn_143b03380, fn_143af5f00, fn_143b0cb60_kdiPass,
  fn_143af6960, fn_143b1a4d0, fn_143b15610,
  _internal: { invScale, safeNormal, arcFromX, qnormSafe },
};

// ---- self test ---------------------------------------------------------------------------------------------------
if (require.main === module) {
  const mk = (i, pos, kin) => ({
    f24C: i, f0: pos.map(f), fC: pos.map(f), f20: [0, 0, 0, 1], f120: [0, 0, 0, 1], f130: pos.map(f), f140: [1, 1, 1],
    f160: [0, 0, 0, 1], f190: null, f198: null, f1A8: 0, f1B0: [], f238: null, f240: null, f276: kin ? 1 : 0, f310: 0,
  });
  // 0 = R_susoA1 (kinematic), 1 = R_susoA2 (dynamic, the driver), 2 = R_susoA3, 3 = jip1, 4 = jip2, 5/6 = pin pair
  const P = [mk(0, [0, 0, 100], 1), mk(1, [0, 3, 92], 0), mk(2, [0, 5, 84], 0), mk(3, [1, 6, 90], 1), mk(4, [1, 6.5, 87], 0),
    mk(5, [10, 0, 50], 1), mk(6, [10, 2, 48], 0)];
  const link = (a, b) => { const l = { f0: P[a], f8: P[b], f20: sqrt(dot(sub(P[a].f0, P[b].f0), sub(P[a].f0, P[b].f0))), f24: 1 };
    P[a].f1B0.push(l); P[a].f1A8++; P[a].f198 = P[b]; P[b].f190 = P[a]; return l; };
  link(0, 1); link(1, 2); link(3, 4);
  for (const p of P) if (p.f198) { p.f20 = arcFromX(safeNormal(sub(p.f198.f0, p.f0))); p.f120 = p.f20.slice(); p.f160 = p.f20.slice(); }
  const scene = { f0: P, f8: P.length };
  setupPinKdi(scene, [
    { parent: 1, child: 3, type: 'KdiConstraint', pinRadius: 0, pinStr: 0, kdiTrans: 1, kdiRot: 1 },
    { parent: 5, child: 6, type: 'PinConstraint', pinRadius: 0.5, pinStr: 0.25, kdiTrans: 0, kdiRot: 0 },
  ], 0);
  const rec = scene.f768[0];
  console.log('KDI records', scene.f770, 'drivers', rec.f10, 'entry', JSON.stringify({ f8: rec.f8[0].f8, f20: rec.f8[0].f20, f30: rec.f8[0].f30, f34: rec.f8[0].f34 }));
  console.log('pin record', JSON.stringify({ f10: scene.f758[0].f10, f1C: scene.f758[0].f1C, f20: scene.f758[0].f20 }), 'jip1 kinematic', P[3].f276);
  fn_143aee280_kdiRefresh(scene);
  // setup-pose check: the pass must reproduce jip1's own setup position and orientation
  const before = { pos: P[3].f0.slice(), q: P[3].f20.slice() };
  fn_143b0cb60_kdiPass(scene);
  console.log('setup pose  jip1 pos', before.pos, '->', P[3].f0, ' quat', before.q, '->', P[3].f20);
  // move the driver and its child, run again
  P[1].f0 = [f(0.5), f(3.5), f(92.2)]; P[2].f0 = [f(1.5), f(6), f(84.6)];
  fn_143b0cb60_kdiPass(scene);
  console.log('moved       jip1 pos', P[3].f0, ' quat', P[3].f20, ' target', P[3].f160);
  const calls = [];
  setDeps({
    chainLink: (r, l, cfg) => calls.push(['chainLink', l.f0.f24C, l.f8.f24C]),
    sphereSnap: (out, x, c, rad, str) => { calls.push(['sphereSnap', rad, str]); x[0] = c[0]; x[1] = c[1]; x[2] = c[2]; },
  });
  P[4].f0 = [f(2), f(7), f(80)];
  fn_143b15610(f(1), { f3778: scene }, null, { core: true });
  const d = sub(P[4].f0, P[3].f0);
  console.log('15610 calls', JSON.stringify(calls), ' jip2', P[4].f0, ' |jip2-jip1|', sqrt(dot(d, d)), 'rest', P[3].f1B0[0].f20, ' pinned', P[6].f0);
  console.log('slerp', fn_142d90ca0([0, 0, 0, 0], [0, 0, 0, 1], [0, f(0.38268343), 0, f(0.92387953)], f(0.5)));
  console.log('af6960', fn_143af6960(scene.f758[0], [0, 0, 0]));
  fn_143b03380(P[1]); fn_143afb8e0(rec); fn_143b1a4d0(f(1), P[3].f1B0[0]);
  console.log('ok');
}
