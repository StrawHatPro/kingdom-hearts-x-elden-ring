// Bonamik core pieces transcribed by the pass-2 lead (see native_H_bonamik2.md): group weights 0x143b10010,
// bone weights 0x143b11700, time step of CoreUpdate 0x143b0efc0, setup blocks of 0x143b07550 (orientation init,
// joint-twist rest angle, chain roots, hard-length marking). Field naming: f<HEX OFFSET> (see SUB_BRIEF.md).
const { f, add, sub, mul, dot, cross, sqrt, rsqrt, qmul, qconj } = require('./m.js');
const ZERO = () => [0, 0, 0];
// normalize as the module does: if |v|^2 == 1 keep; if < 1e-8 -> zero vector; else v * rsqrt(two Newton steps)
function normSafe(v) { const l = f(f(f(v[0] * v[0]) + f(v[1] * v[1])) + f(v[2] * v[2])); if (l === 1) return v.slice(); if (!(l >= f(1e-8))) return ZERO(); const k = rsqrt(l); return [f(v[0] * k), f(v[1] * k), f(v[2] * k)]; }
// shortest arc from +X to unit vector d, as coded: (0, -d.z/s, d.y/s, s/2), s = sqrt(2 d.x + 2); (0,1,0,0) if d.x < -0.9999
function arcFromX(d) { if (f(-0.9999) <= d[0]) { const s = sqrt(f(f(d[0] + d[0]) + 2)); const inv = f(1 / s); return [-0, f(-f(inv * d[2])), f(inv * d[1]), f(s * 0.5)]; } return [0, 1, 0, 0]; }
// rotate v by q in the order used for particle+0x1DC: (c2 + v) + w*t, t = 2 (q x v)
function rotB(q, v) { const t = mul(cross(q, v), 2); const c2 = cross(q, t); return [f(f(c2[0] + v[0]) + f(q[3] * t[0])), f(f(c2[1] + v[1]) + f(q[3] * t[1])), f(f(c2[2] + v[2]) + f(q[3] * t[2]))]; }
// "swing with the bone twist": q = arcFromX(dir); P = q*conj(B); T = normalize((P.xyz . a) a, P.w), a = rot(q,+X); result = conj(T)*q
function swingKeepTwist(dir, B) {
  const q = arcFromX(dir);
  const a = [f(f(f(f(-2 * q[1]) * q[1]) - f(f(2 * q[2]) * q[2])) + 1), f(f(f(2 * q[2]) * q[3]) - f(f(-2 * q[1]) * q[0])), f(f(f(2 * q[2]) * q[0]) + f(f(-2 * q[1]) * q[3]))];
  const P = qmul(q, qconj(B));
  const pr = f(f(f(a[0] * P[0]) + f(a[1] * P[1])) + f(a[2] * P[2]));
  let T = [f(a[0] * pr), f(a[1] * pr), f(a[2] * pr), P[3]];
  const l = f(f(f(T[0] * T[0]) + f(T[2] * T[2])) + f(f(T[1] * T[1]) + f(T[3] * T[3])));
  if (l >= f(1e-8)) { let y = f(1 / Math.sqrt(l)); y = f(f(f(0.5 - f(f(f(y * y) * l) * 0.5)) * y) + y); T = T.map(x => f(x * y)); } else T = [0, 0, 0, 1];   // rsqrtps + ONE Newton step
  return qmul([f(-T[0]), f(-T[1]), f(-T[2]), T[3]], q);
}
// polynomial acos used all over the module (constants 1.5707288, 0.2121144, 0.074261, 0.0187293; pi 3.1415927)
function acosPoly(x) { if (!(x >= -1)) x = -1; else if (x >= 1) x = 1; const a = Math.abs(x); const r = f(sqrt(f(1 - a)) * f(f(f(f(1.5707288) - f(a * f(0.2121144))) + f(f(a * a) * f(0.074261))) - f(f(f(a * a) * a) * f(0.0187293)))); return x < 0 ? f(f(3.1415927) - r) : r; }

// ---- 0x143b10010 GroupWeights(core, dtp)
function fn_143b10010(core, dt) {
  core.f65C = 0;
  for (let g = 0; g < core.f410; g++) {
    let a;
    if (core.f34B8[g] <= f(0.0001)) {
      const time = core.f33F8[g]; core.f34B8[g] = 0; a = core.f3638[g];
      if (time <= dt) core.f33F8[g] = 0;
      else { a = f(f(f(a - core.f3578[g]) * f(dt / time)) + core.f3578[g]); if (a >= 0) { if (a >= 1) a = 1; } else a = 0; core.f33F8[g] = f(time - dt); }
      core.f3578[g] = a;
    } else { a = core.f3578[g]; core.f34B8[g] = f(core.f34B8[g] - dt); }
    if (a <= f(0.9999) || core.f3638[g] <= f(0.9999)) { if (a < f(0.0001) && core.f3638[g] < f(0.0001)) core.f8C4[g] = 0; }
    else { const s = core.f804[g]; if (s === 3) core.f804[g] = 1; else if (s === 4) core.f804[g] = 0; else if (s === 2) core.f804[g] = 1; }
    if (core.f804[g] > 1) core.f65C = 1;
  }
}
// ---- 0x143b11700 BoneWeights(core): driven-bone bit set core.f3800 (array of 0/1 per bone) and weights core.f3810
function fn_143b11700(core, parentOf) {
  const sc = core.f3778, n = core.f3808; const words = core.f3800 = new Uint32Array(Math.max(1, (n + 31) >> 5));
  const get = i => (words[i >> 5] >>> (i & 31)) & 1, set = i => { words[i >> 5] |= (1 << (i & 31)); };
  for (const p of sc.f0) { const c = p.f198; if (core.f804[p.f2B8] > 1 && !p.f277 && (!p.f276 || (c && !c.f276 && c.fBC))) set(p.f248); }
  for (let b = 1; b < n; b++) { if (!get(b)) { let a = parentOf(b); while (a !== -1) { if (get(a)) { set(b); break; } a = parentOf(a); } } }
  const w = core.f3810; w.fill(0);
  for (const p of sc.f0) if (get(p.f248) && !p.f278) w[p.f248] = f(f(1 - core.f3578[p.f2B8]) * core.f36F8);
  for (const fl of core.f3768) { const g = sc.f0[fl.f0].f2B8; for (const b of fl.f8) w[b] = f(f(1 - core.f3578[g]) * core.f36F8); }
}
// ---- time step part of CoreUpdate 0x143b0efc0 [I]: returns {dtSim, r, dtp}
const FPS_TABLE = [20, 30, 40, 50, 60];                       // 0x147506880
function nearestBucket(fps) { let best = 0, bd = Infinity; for (let i = 0; i < 5; i++) { const d = Math.abs(f(fps - FPS_TABLE[i])); if (d < bd) { bd = d; best = i; } } return best; }   // 0x143af5d70 [C]
function timeStep(core, cfg, dt, td) {
  if (dt > f(1e-8)) {
    let d = f(0.008333334); if (dt >= d) { d = dt; if (dt >= f(0.1)) d = f(0.1); }
    core.f382C = f(f(f(d - core.f382C) * f(0.1)) + core.f382C);
    const b = nearestBucket(f(1 / core.f382C));
    if (b === core.f3824) { if (core.f3828 < 10) core.f3828++; else { const tgt = core.f3834, st = core.f3830; if (tgt <= st) { let v = f(st - f(0.002)); if (v <= tgt) v = tgt; core.f3830 = v; } else { let v = f(st + f(0.002)); if (tgt <= v) v = tgt; core.f3830 = v; } } }
    else { core.f3824 = b; core.f3828 = 0; core.f3834 = f(1 / FPS_TABLE[b]); }
  }
  const step = core.f3830; const base = f(step * core.f3838);
  const dtp = f(dt * td);
  if (core.f3710 !== 0) { const v = f(f(core.f3710 * dtp) + core.f3708); core.f3708 = v; if ((core.f3710 > 0 && core.f370C <= v) || (core.f3710 < 0 && v <= core.f370C)) { core.f3708 = core.f370C; core.f3710 = 0; } }
  const dtSim = f(f(base * td) * core.f3708);
  const r = f(f(f(base * cfg.f64) * td) * core.f3708);
  return { dtSim, r, dtp, step };
}
// ---- setup block of 0x143b07550: initial orientation (+0x20, +0x30, +0x1DC) [C]
function initOrientations(core) {
  const P = core.f3778.f0;
  for (const p of P) {
    if (p.f1A4 === -1) p.f20 = p.f120.slice();
    else { const c = p.f198 = P[p.f1A4]; p.f20 = swingKeepTwist(normSafe(sub(c.f0, p.f0)), p.f120); }
    if (p.f1A0 !== -1) p.f190 = P[p.f1A0];
  }
  for (const p of P) {
    const c = p.f198;
    if (c) { const d = sub(c.f0, p.f0); if (f(f(f(d[0] * d[0]) + f(d[1] * d[1])) + f(d[2] * d[2])) < f(0.01) && c.f198) p.f20 = swingKeepTwist(normSafe(sub(c.f198.f0, p.f0)), p.f120); }
    p.f1DC = rotB(p.f20, core.f1C0[p.f2B8]); p.f30 = p.f20.slice();
  }
}
// ---- setup block: chain roots (scene.f748) and hard-length marking [C]
function buildRoots(core) {
  const sc = core.f3778, P = sc.f0; sc.f748 = [];
  P.forEach((p, i) => {
    if (p.f277) return;
    if (!p.f190) { for (let k = 0; k < p.f1A8; k++) if (!p.f1B0[k].f8.f276) { if (p.f276 || p.f238 || p.f240 || p.fBD) sc.f748.push(i); break; } }
    else if ((!p.fBC || (p.f276 && p.f190.f276)) && ((p.f198 && !p.f198.f276 && (p.f276 || p.f238 || p.f240)) || (!p.f276 && p.fBD))) { p.fBC = 0; sc.f748.push(i); }
  });
  sc.f750 = sc.f748.length;
  const anch = p => { if (p.fBD) return true; for (let k = 0; k < p.f1A8; k++) { const c = p.f1B0[k].f8; if (c.f276 || c.fBD || anch(c)) return true; } return false; };   // 0x143b15520
  const mark = p => { for (let k = 0; k < p.f1A8; k++) { p.f1B0[k].f26 = 1; mark(p.f1B0[k].f8); } };                                                                // 0x143b155b0
  if (core.f658 >= 2400 && core.f664 === 0) for (const i of sc.f748) if (!anch(P[i])) mark(P[i]);
}
// ---- setup block: joint-twist rest angle particle+0x318 and neighbour signs +0x300[i] [C]
function initTwistRest(core) {
  for (const p of core.f3778.f0) {
    if (!p.f310) continue;
    let q, dv; const up = core.f1C0[p.f2B8];
    if (p.f190) { q = p.f190.f120; dv = sub(p.f0, p.f190.f0); } else if (p.f198) { q = p.f120; dv = sub(p.f198.f0, p.f0); } else continue;
    const U = rotB(q, up); const d = normSafe(dv); let S = ZERO();
    for (let i = 0; i < p.f310; i++) { const e = normSafe(sub(p.f2E0[i].f0, p.f0)); const c = cross(d, e); let sg; if (f(f(f(c[0] * U[0]) + f(c[1] * U[1])) + f(c[2] * U[2])) < 0) { p.f300[i] = -1; sg = -1; } else { p.f300[i] = 1; sg = 1; } S = [f(S[0] + f(c[0] * sg)), f(S[1] + f(c[1] * sg)), f(S[2] + f(c[2] * sg))]; }
    const n = normSafe(S); const pr = f(f(f(n[1] * d[1]) + f(n[0] * d[0])) + f(n[2] * d[2]));
    const m = normSafe([f(n[0] - f(d[0] * pr)), f(n[1] - f(d[1] * pr)), f(n[2] - f(d[2] * pr))]);
    const cs = f(f(f(m[1] * U[1]) + f(m[0] * U[0])) + f(m[2] * U[2]));
    const tri = f(f(f(f(f(m[2] * U[1]) - f(m[1] * U[2])) * d[0]) + f(d[1] * f(f(m[0] * U[2]) - f(m[2] * U[0])))) + f(d[2] * f(f(m[1] * U[0]) - f(m[0] * U[1]))));
    p.f318 = tri < 0 ? f(-acosPoly(cs)) : acosPoly(cs);
  }
}
module.exports = { normSafe, arcFromX, rotB, swingKeepTwist, acosPoly, fn_143b10010, fn_143b11700, timeStep, nearestBucket, FPS_TABLE, initOrientations, buildRoots, initTwistRest };
