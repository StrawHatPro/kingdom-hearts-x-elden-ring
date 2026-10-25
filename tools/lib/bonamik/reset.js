// Bonamik pass 2, part "reset": frame transforms, teleport check, reset, pre-step (without the cone refresh loop),
// SetFromBone, LocalForce (fast-motion mode 1). Transcribed from the disassembly of KH3.exe; see H_reset.md.
// Conventions: SUB_BRIEF.md (fields by hex offset, float32 via m.js, bones[i] = {q,t,s}, owner.vfXX()).
//
// Object shapes used here
//   core.f3848 = owner interface object  (methods vf48() -> {q,t,s} ComponentToWorld, vf60() -> bones array,
//                vf88() -> int (0 for the KBD host), vfD0() -> bool/0/1, vfF0() -> [x,y,z])
//   core.f3778 = scene.  FTransforms of the core: f0/f10/f20, f30/f40/f50, f70/f80/f90, fA0/fB0/fC0, fD0/fE0/fF0,
//                f100/f110/f120, f130/f140/f150, f160/f170/f180, f190/f1A0/f1B0 (quat, vec3, vec3).
//   scene.f0 particles (count f8), scene.f10 anchors (count f18), scene.f20 plane groups (count f28),
//   scene.f30 cones (count f38), scene.f40 links (count f48), scene.f748 int array of chain-root particle indices
//   (count f750), scene.f758 attached-chain entries (count f760; e.f0 = particle), scene.f768 KDI groups (count f770;
//   g.f8 = entry array, g.f10 = count, entry.f0 = particle), scene.f7AC byte.
//   The LocalForce argument struct ("ctx", built on the stack at rbp-80h in PreStep):
//     f0 = chain root particle R, f8 = dq quat, f10 = componentDeltaTrans vec3, f18 = componentDeltaRot quat,
//     f20 = pointer core+0x30 (previous ComponentToWorld)  -> here an object {f0:quat, f10:vec3, f20:vec3},
//     f28 = pointer core+0x00 (current ComponentToWorld)   -> here the core itself (f0, f10, f20),
//     f30 = pointer core+0x658 (indexed [group*4 + 0x6BC] = core+0xD14+4g) -> here the core (core.fD14[g]),
//     f38 = core, f40 = reset byte (copy of core+0x33F4), f44 = r (float).
const { f, add, sub, mul, dot, cross, sqrt, rsqrt, qmul, qconj, qrot } = require('./m.js');

// ---- constants that live in .bss (runtime initialised; identified by their use, see H_reset.md section 0) ----------
const ZERO_VEC = [0, 0, 0];                    // 0x149B5C798 (FVector::ZeroVector)
const IDENT_Q = [0, 0, 0, 1];                  // 0x149B5C560 (FTransform::Identity rotation)
const IDENT_T = [0, 0, 0];                     // 0x149B5C570
const IDENT_S = [1, 1, 1];                     // 0x149B5C580
const SMALL = f(1e-8);                         // 0x149D52920 lanes; scalar copy 0x145C9C488 = 0x322BCC77

const v3 = a => [a[0], a[1], a[2]];
const q4 = a => [a[0], a[1], a[2], a[3]];
const mulv = (a, b) => [f(a[0] * b[0]), f(a[1] * b[1]), f(a[2] * b[2])];       // mulps lane-wise

// rcpps + two Newton steps  r = (r + r) - (r*r)*s   (rcpps itself emulated by a correctly rounded 1/s)
function recipAccurate(s) {
  let r = f(1 / s);
  r = f(f(r + r) - f(f(r * r) * s));
  r = f(f(r + r) - f(f(r * r) * s));
  return r;
}
// stock FTransform::GetSafeScaleReciprocal(Scale, SmallNumber): lane = |s| <= 1e-8 ? 0 : 1/s
function safeScaleRecip(s) {
  return [0, 1, 2].map(i => (Math.abs(s[i]) <= SMALL ? 0 : recipAccurate(s[i])));
}
const anyAbsGreaterSmall = s => Math.abs(s[0]) > SMALL || Math.abs(s[1]) > SMALL || Math.abs(s[2]) > SMALL;
// stock FTransform::Inverse (SSE shape): returns {q,t,s}
function xfInverse(q, t, s) {
  if (!anyAbsGreaterSmall(s)) return { q: q4(IDENT_Q), t: v3(IDENT_T), s: v3(IDENT_S) };
  const iq = [f(q[0] * -1), f(q[1] * -1), f(q[2] * -1), f(q[3] * 1)];        // mulps QINV_SIGN_MASK (0x149D52910)
  const is = safeScaleRecip(s);
  const r = qrot(iq, mulv(t, is));
  return { q: iq, t: [f(0 - r[0]), f(0 - r[1]), f(0 - r[2])], s: is };
}
// stock FTransform::Multiply(A, B) = "A then B", quaternion path only
function xfMul(aq, at, as, bq, bt, bs, where) {
  if (Math.min(as[0], bs[0]) < 0 || Math.min(as[1], bs[1]) < 0 || Math.min(as[2], bs[2]) < 0)
    throw new Error('not transcribed: negative scale -> FTransform::MultiplyUsingMatrixWithScale path (' + where + ')');
  return { q: qmul(bq, aq), t: add(qrot(bq, mulv(at, bs)), bt), s: mulv(as, bs) };
}

// ---------------------------------------------------------------------------------------------------------------------
// 0x143b1da00  FrameTransforms(core /*rcx*/)
function fn_143b1da00(core) {
  const owner = core.f3848;
  const bones = owner.vf60();
  core.f30 = q4(core.f0); core.f40 = v3(core.f10); core.f50 = v3(core.f20);          // previous ComponentToWorld
  const W = owner.vf48();
  core.f0 = q4(W.q); core.f10 = v3(W.t); core.f20 = v3(W.s);                         // current ComponentToWorld
  core.f190 = q4(core.f160); core.f1A0 = v3(core.f170); core.f1B0 = v3(core.f180);   // previous root-bone delta
  const inv = xfInverse(core.f130, core.f140, core.f150);                            // inverse of the reference root bone
  const B = bones[owner.vf88()];
  const d = xfMul(B.q, B.t, B.s, inv.q, inv.t, inv.s, '143b1dc1e..143b1dfc9');
  core.f160 = d.q; core.f170 = d.t; core.f180 = d.s;                                 // root-bone delta (component space)
  core.f100 = q4(core.f0); core.f110 = v3(core.f10); core.f120 = v3(core.f20);       // component -> world (copy)
  const wi = xfInverse(core.f0, core.f10, core.f20);
  core.fD0 = wi.q; core.fE0 = wi.t; core.fF0 = wi.s;                                 // world -> component
  core.fA0 = q4(core.f70); core.fB0 = v3(core.f80); core.fC0 = v3(core.f90);         // previous "root world" transform
  let sc = d.s;
  if (core.f60 === -1 || core.f660 !== 2) {
    core.f70 = q4(core.f160); core.f80 = v3(core.f170);
  } else {                                                                           // LSS mode only (dead for Sora)
    const R = bones[core.f60];
    const w = xfMul(R.q, R.t, R.s, core.f0, core.f10, core.f20, '143b1e21d..143b5cc');
    core.f70 = w.q; core.f80 = w.t; sc = w.s;
  }
  core.f90 = v3(sc);
}

// 0x143b00530  ResetFrameTransforms(core /*rcx*/, bones /*rdx: pointer to the bone array pointer*/)
function fn_143b00530(core, bones) {
  const owner = core.f3848;
  const W = owner.vf48();
  core.f100 = q4(W.q); core.f110 = v3(W.t); core.f120 = v3(W.s);
  const wi = xfInverse(core.f100, core.f110, core.f120);
  core.fD0 = wi.q; core.fE0 = wi.t; core.fF0 = wi.s;
  const inv = xfInverse(core.f130, core.f140, core.f150);
  const B = bones[owner.vf88()];
  const d = xfMul(B.q, B.t, B.s, inv.q, inv.t, inv.s, '143b007f7..143b00bb0');
  core.f160 = d.q; core.f170 = d.t; core.f180 = d.s;
  core.f190 = q4(d.q); core.f1A0 = v3(d.t); core.f1B0 = v3(d.s);
  let sc = d.s;
  if (core.f60 === -1 || core.f660 !== 2) {
    core.f70 = q4(d.q); core.f80 = v3(d.t);
  } else {                                                                           // LSS mode only (dead for Sora)
    const R = bones[core.f60];
    const w = xfMul(R.q, R.t, R.s, core.f0, core.f10, core.f20, '143b00cc0..143b010ac');
    core.f70 = w.q; core.f80 = w.t; sc = w.s;
  }
  core.f90 = v3(sc);
  core.fA0 = q4(core.f70); core.fB0 = v3(core.f80); core.fC0 = v3(core.f90);
}

// 0x143af9090  (core /*rcx*/, bones /*rdx*/): world transform cur = prev, reference root bone = current, then 0x143b00530
function fn_143af9090(core, bones) {
  const owner = core.f3848;
  const W = owner.vf48();
  core.f0 = q4(W.q); core.f10 = v3(W.t); core.f20 = v3(W.s);
  core.f30 = q4(core.f0); core.f40 = v3(core.f10); core.f50 = v3(core.f20);
  const B = bones[owner.vf88()];
  core.f130 = q4(B.q); core.f140 = v3(B.t); core.f150 = v3(B.s);
  fn_143b00530(core, bones);
}

// 0x143afaf80  TeleportCheck(core /*rcx*/)
function fn_143afaf80(core) {
  const forced = core.f33E9;
  core.f33E9 = 0;
  const neg = (a, b) => Math.min(a[0], b[0]) < 0 || Math.min(a[1], b[1]) < 0 || Math.min(a[2], b[2]) < 0;
  if (neg(core.f1B0, core.f50)) throw new Error('not transcribed: 143afb022..143afb3df (negative scale, matrix path)');
  const prev = add(qrot(core.f30, mulv(core.f1A0, core.f50)), core.f40);
  if (neg(core.f180, core.f20)) throw new Error('not transcribed: 143afb46a..143afb79a (negative scale, matrix path)');
  const cur = add(qrot(core.f0, mulv(core.f170, core.f20)), core.f10);
  if (!forced) {
    const dx = f(cur[0] - prev[0]), dy = f(cur[1] - prev[1]), dz = f(cur[2] - prev[2]);
    const d2 = f(f(f(dy * dy) + f(dx * dx)) + f(dz * dz));
    if (!(d2 >= core.f371C)) return;                       // comiss + setae: NaN -> no teleport
  }
  core.f33F4 = 1;
  const n = core.f33F0;
  if (n > 0 && n > core.f33EC) { core.f33EA = 1; core.f33EC = n; }
}

// 0x143b01210  Reset(core /*rcx*/, hard /*dl*/)
function fn_143b01210(core, hard) {
  const bones = core.f3848.vf60();
  const scene = core.f3778;
  scene.f7AC = 1;
  fn_143af9090(core, bones);
  if (hard) {
    for (let i = 0; i < scene.f8; i++) {
      const p = scene.f0[i];
      const B = bones[p.f248];
      p.f0 = add(qrot(B.q, mulv(p.f170, B.s)), B.t);
      p.f40 = v3(ZERO_VEC);
      p.f120 = q4(B.q); p.f130 = v3(B.t); p.f140 = v3(B.s);
      p.f20 = qmul(B.q, p.f180);
    }
  }
  for (let i = 0; i < scene.f8; i++) {
    const p = scene.f0[i];
    p.fC = v3(p.f0);
    p.f4C = v3(p.f40);
    p.fF0 = q4(p.f120); p.f100 = v3(p.f130); p.f110 = v3(p.f140);
    p.f30 = q4(p.f20);
  }
}

// 0x143b0e790  (core /*rcx*/): shift the animated bone transform history of every particle (every CoreUpdate)
function fn_143b0e790(core) {
  const bones = core.f3848.vf60();
  const scene = core.f3778;
  for (let i = 0; i < scene.f8; i++) {
    const p = scene.f0[i];
    p.fF0 = q4(p.f120); p.f100 = v3(p.f130); p.f110 = v3(p.f140);
    const B = bones[p.f248];
    p.f120 = q4(B.q); p.f130 = v3(B.t); p.f140 = v3(B.s);
  }
}

// 0x143afd380  ReinitializeSimulationPose(core /*rcx*/), run by CoreUpdate when core+0x33F6.
// Loops 1, 3 and 4 are transcribed from the decompiled C only ([C]); loop 2 (cone rest data, 143afd4b0..143afe0a0)
// is NOT transcribed. Not reachable for the player unless the native / notify is used (H_reset.md section 5).
let coneReinit = null;
function fn_143afd380(core) {
  const bones = core.f3848.vf60();
  const scene = core.f3778;
  for (let i = 0; i < scene.f8; i++) {                       // loop 1
    const p = scene.f0[i];
    p.fC = v3(p.f0);
    p.f30 = q4(p.f20);
    const B = bones[p.f248];
    p.f20 = q4(B.q);
    p.f0 = add(qrot(B.q, mulv(p.f170, B.s)), B.t);
    p.f120 = q4(B.q); p.f130 = v3(B.t); p.f140 = v3(B.s);
    p.fF0 = q4(B.q); p.f100 = v3(p.f130); p.f110 = v3(p.f140);
  }
  if (scene.f38 > 0) {                                       // loop 2
    if (!coneReinit) throw new Error('not transcribed: 143afd4b0..143afe0a0 (cone rest data from the current pose)');
    coneReinit(core, bones);
  }
  for (let i = 0; i < scene.f48; i++) {                      // loop 3: link rest lengths
    const l = scene.f40[i];
    const a = l.f0.f0, b = l.f8.f0;
    const dx = f(b[0] - a[0]), dy = f(b[1] - a[1]), dz = f(b[2] - a[2]);
    l.f20 = sqrt(f(f(f(dy * dy) + f(dx * dx)) + f(dz * dz)));
  }
  for (let i = 0; i < scene.f8; i++) {                       // loop 4: positions back to what they were
    const p = scene.f0[i];
    p.f0 = v3(p.fC);
  }
}

// 0x143aee110  SetFromBone(core /*rcx*/, bones /*rdx*/, p /*r8*/)
function fn_143aee110(core, bones, p) {
  const B = bones[p.f248];
  const pos = add(qrot(B.q, mulv(p.f170, B.s)), B.t);
  p.f0 = pos;
  const q = qmul(B.q, p.f180);
  p.f20 = q;
  p.f160 = q4(q);
  if (core.f33F4) {
    p.fC = v3(pos);
    p.f4C = v3(p.f40);
    p.f30 = q4(q);
  }
}

// 0x143aeaf80  LocalForce(ctx /*rcx*/, p /*rdx*/)
function fn_143aeaf80(ctx, p) {
  if (p.f276) return;
  const s = ctx.f30.fD14[p.f2B8];                                      // [ (core+0x658) + group*4 + 0x6BC ]
  const R = ctx.f0, dq = ctx.f8;
  const ox = p.f0[0], oy = p.f0[1], oz = p.f0[2];
  const rel = [f(ox - R.f100[0]), f(oy - R.f100[1]), f(oz - R.f100[2])];
  let park = f(s * p.f28C);
  park = park < 1 ? park : 1;                                          // minss xmm15, 1.0
  const T = add(qrot(dq, rel), R.f0);
  let F;
  if (ctx.f40) {
    p.f0 = T;
    F = v3(ZERO_VEC);
  } else {
    const k = f(1 - park);
    const x = f(f(f(T[0] - ox) * k) + ox);
    const y = f(f(f(T[1] - oy) * k) + oy);
    const z = f(f(f(T[2] - oz) * k) + oz);
    p.f20 = qmul(dq, p.f20);
    const rr = qrot(ctx.f18, [x, y, z]);
    const cdt = ctx.f10;
    p.f0 = [
      f(x + f(f(f(rr[0] - cdt[0]) - x) * park)),
      f(f(f(f(rr[1] - cdt[1]) - y) * park) + y),
      f(f(f(f(rr[2] - cdt[2]) - z) * park) + z)];
    const P = ctx.f20, C = ctx.f28;                                    // previous / current ComponentToWorld
    const dT = [f(C.f10[0] - P.f10[0]), f(C.f10[1] - P.f10[1]), f(C.f10[2] - P.f10[2])];
    const pr = qrot(P.f0, mulv(P.f20, p.f100));                        // lanes: P.scale * prevAnimTranslation
    const cr = qrot(C.f0, mulv(C.f20, p.f130));
    const D = [f(f(cr[0] - pr[0]) + dT[0]), f(f(cr[1] - pr[1]) + dT[1]), f(f(cr[2] - pr[2]) + dT[2])];
    const nq = [-C.f0[0], -C.f0[1], -C.f0[2], C.f0[3]];                // xorps sign mask 0x145C82880
    const rd = qrot(nq, D);
    const m = -f(s * p.f288);
    F = [f(rd[0] * m), f(rd[1] * m), f(rd[2] * m)];
  }
  const owner = ctx.f38.f3848;
  if (owner.vfD0()) {
    const e = owner.vfF0();
    F = [f(F[0] + f(s * e[0])), f(F[1] + f(s * e[1])), f(F[2] + f(s * e[2]))];
  }
  const maxLen = f(p.f290 * ctx.f44);
  const l2 = f(f(f(F[1] * F[1]) + f(F[0] * F[0])) + f(F[2] * F[2]));
  if (l2 > f(maxLen * maxLen)) {
    const inv = f(1 / sqrt(l2));
    F = [f(f(F[0] * maxLen) * inv), f(f(F[1] * maxLen) * inv), f(f(F[2] * maxLen) * inv)];
  }
  p.f7C = F;
}

// 0x143ae2cb0  (p /*rcx*/, ctx /*rdx*/): LocalForce on p, then on every ParentChild child, depth first
function fn_143ae2cb0(p, ctx) {
  fn_143aeaf80(ctx, p);
  for (let k = 0; k < p.f1A8; k++) fn_143ae2cb0(p.f1B0[k].f8, ctx);
}

// 0x143aee280  PreStep(core /*rcx*/, r /*xmm1*/). The cone refresh loop 143aee603..143aeebc6 is injected.
let coneRefresh = null;
function fn_143aee280(core, r) {
  const owner = core.f3848;
  const bones = owner.vf60();
  const scene = core.f3778;
  // 143aee2e4: particles
  for (let i = 0; i < scene.f8; i++) {
    const p = scene.f0[i];
    if (p.f278) continue;
    if (p.f276) fn_143aee110(core, bones, p);
    else {
      const q = bones[p.f248].q;
      p.f160 = q4(q);
      p.f20 = q4(q);
      if (core.f33F4) p.f30 = q4(q);
    }
  }
  // 143aee363: dynamic chain roots follow the animation
  for (let i = 0; i < scene.f750; i++) {
    const R = scene.f0[scene.f748[i]];
    if (!R.f276) fn_143aee110(core, bones, R);
  }
  // 143aee3af: anchor constraints
  for (let i = 0; i < scene.f18; i++) {
    const a = scene.f10[i];
    const B = bones[scene.f0[a.f0].f248];
    const P = add(qrot(B.q, mulv(ZERO_VEC, B.s)), B.t);
    if (a.f4 !== 4) a.fC = v3(P);
    a.f28 = v3(P);
  }
  // 143aee4d7: plane groups
  for (let i = 0; i < scene.f28; i++) {
    const g = scene.f20[i];
    for (let j = 0; j < g.f8; j++) {
      const e = g.f0[j];
      const B = bones[e.fA8.f248];
      e.f0 = add(qrot(B.q, mulv(ZERO_VEC, B.s)), B.t);
    }
  }
  // 143aee603..143aeebc6: cone refresh (other part)
  if (scene.f38 > 0) {
    if (!coneRefresh) throw new Error('cone refresh loop not injected (setConeRefresh): 143aee603..143aeebc6');
    coneRefresh(core, bones, r);
  }
  // 143aeebcf: attached chains
  for (let i = 0; i < scene.f760; i++) {
    const P = scene.f758[i].f0;
    const par = P.f190;
    P.f160 = q4(P.f120);
    if (par) par.f160 = q4(par.f120);
  }
  // 143aeec2a: KDI groups
  for (let i = 0; i < scene.f770; i++) {
    const g = scene.f768[i];
    for (let j = 0; j < g.f10; j++) { const P = g.f8[j].f0; P.f160 = q4(P.f120); }
  }
  // 143aeecb8: inverse of the current component rotation / scale
  let iq, is;
  if (anyAbsGreaterSmall(core.f20)) {
    iq = [f(core.f0[0] * -1), f(core.f0[1] * -1), f(core.f0[2] * -1), f(core.f0[3] * 1)];
    is = safeScaleRecip(core.f20);
  } else { iq = q4(IDENT_Q); is = v3(IDENT_S); }
  if (core.f660 === 1) {
    // 143aeed52: componentDeltaTrans = rot(conj(curRot), (curT - prevT) * invScale); componentDeltaRot = conj(curRot)*prevRot
    const d = [f(core.f10[0] - core.f40[0]), f(core.f10[1] - core.f40[1]), f(core.f10[2] - core.f40[2])];
    const cdt = qrot(iq, mulv(d, is));
    const cdr = qmul([-core.f0[0], -core.f0[1], -core.f0[2], core.f0[3]], core.f30);
    for (let i = 0; i < scene.f750; i++) {
      const R = scene.f0[scene.f748[i]];
      if (R.f240) continue;
      const dq = qmul(bones[R.f248].q, [-R.fF0[0], -R.fF0[1], -R.fF0[2], R.fF0[3]]);
      for (let k = 0; k < R.f1A8; k++) {
        const ctx = {
          f0: R, f8: dq, f10: cdt, f18: cdr,
          f20: { f0: core.f30, f10: core.f40, f20: core.f50 },
          f28: core, f30: core, f38: core,
          f40: core.f33F4, f44: r,
        };
        const child = R.f1B0[k].f8;
        fn_143aeaf80(ctx, child);
        for (let j = 0; j < child.f1A8; j++) fn_143ae2cb0(child.f1B0[j].f8, ctx);
      }
    }
  } else if (core.f660 === 2) {
    if (core.f60 !== -1) throw new Error('not transcribed: 143aef0ce..143aef527 (LSS fast-motion mode, 0x143aeacf0 / 0x143ae2c50)');
  }
}

module.exports = {
  fn_143b1da00, fn_143b00530, fn_143af9090, fn_143afaf80, fn_143b01210, fn_143b0e790, fn_143afd380,
  fn_143aee110, fn_143aeaf80, fn_143ae2cb0, fn_143aee280,
  FrameTransforms: fn_143b1da00, TeleportCheck: fn_143afaf80, Reset: fn_143b01210, PreStep: fn_143aee280,
  SetFromBone: fn_143aee110, LocalForce: fn_143aeaf80,
  setConeRefresh: fn => { coneRefresh = fn; },
  setConeReinit: fn => { coneReinit = fn; },
  xfInverse, xfMul, safeScaleRecip, recipAccurate,
};

// ---------------------------------------------------------------------------------------------------------------------
if (require.main === module) {
  const bone = (q, t) => ({ q, t, s: [1, 1, 1] });
  const h = f(Math.SQRT1_2);
  const bones = [bone([0, 0, 0, 1], [0, 0, 0]), bone([0, 0, h, h], [0, 10, 100]), bone([0, 0, h, h], [0, 20, 100]), bone([0, 0, h, h], [0, 30, 100])];
  let c2w = { q: [0, 0, 0, 1], t: [100, 200, 0], s: [1, 1, 1] };
  const owner = { vf48: () => c2w, vf60: () => bones, vf88: () => 0, vfD0: () => 0, vfF0: () => [0, 0, 0] };
  const mkP = (i, kin) => ({
    f0: v3(bones[i].t), fC: v3(bones[i].t), f20: q4(bones[i].q), f30: q4(bones[i].q), f40: [0, 0, 0], f4C: [0, 0, 0], f7C: [0, 0, 0],
    fF0: q4(bones[i].q), f100: v3(bones[i].t), f110: [1, 1, 1], f120: q4(bones[i].q), f130: v3(bones[i].t), f140: [1, 1, 1],
    f160: q4(bones[i].q), f170: [0, 0, 0], f180: [0, 0, 0, 1], f190: null, f1A8: 0, f1B0: [], f240: null, f248: i,
    f276: kin ? 1 : 0, f278: 0, f288: 0.5, f28C: 0, f290: 30, f2B8: 0,
  });
  const P = [mkP(1, true), mkP(2, false), mkP(3, false)];
  const link = (a, b) => { const l = { f0: a, f8: b, f20: 10 }; a.f1B0.push(l); a.f1A8++; b.f190 = a; return l; };
  const links = [link(P[0], P[1]), link(P[1], P[2])];
  const scene = {
    f0: P, f8: 3, f10: [{ f0: 1, f4: 1, fC: [0, 0, 0], f28: [0, 0, 0] }], f18: 1, f20: [], f28: 0, f30: [], f38: 0,
    f40: links, f48: 2, f748: [0], f750: 1, f758: [], f760: 0, f768: [], f770: 0, f7AC: 0,
  };
  const I = () => [0, 0, 0, 1];
  const core = {
    f3848: owner, f3778: scene, f60: -1, f660: 1, f33E9: 0, f33EA: 0, f33EC: 0, f33F0: 0, f33F4: 1, f33F5: 0, f33F6: 0,
    f371C: 640000, fD14: [1],
  };
  for (const o of ['f0', 'f30', 'f70', 'fA0', 'fD0', 'f100', 'f130', 'f160', 'f190']) core[o] = I();
  for (const o of ['f10', 'f40', 'f80', 'fB0', 'fE0', 'f110', 'f140', 'f170', 'f1A0']) core[o] = [0, 0, 0];
  for (const o of ['f20', 'f50', 'f90', 'fC0', 'fF0', 'f120', 'f150', 'f180', 'f1B0']) core[o] = [1, 1, 1];
  const M = module.exports;
  // setup-like state, then the first evaluation (soft reset pending)
  M.fn_143af9090(core, bones);
  M.fn_143b0e790(core);
  M.FrameTransforms(core); M.TeleportCheck(core);
  console.log('first eval: reset', core.f33F4, 'preRoll', core.f33EA);
  M.Reset(core, core.f33F5);
  M.PreStep(core, 1);
  console.log('after reset PreStep: p1', P[1].f0, 'F', P[1].f7C);
  core.f33F4 = 0;
  // second evaluation: the component moved 30 cm in world X, bones unchanged
  c2w = { q: [0, 0, 0, 1], t: [130, 200, 0], s: [1, 1, 1] };
  M.fn_143b0e790(core); M.FrameTransforms(core); M.TeleportCheck(core);
  console.log('moved 30: reset', core.f33F4, 'core.fE0', core.fE0);
  M.PreStep(core, 1);
  console.log('PreStep: p1', P[1].f0, 'F', P[1].f7C, 'p2 F', P[2].f7C, 'anchor', scene.f10[0].fC);
  // teleport
  c2w = { q: [0, 0, h, h], t: [1130, 200, 0], s: [1, 1, 1] };
  core.f33F0 = 3;
  M.fn_143b0e790(core); M.FrameTransforms(core); M.TeleportCheck(core);
  console.log('teleport: reset', core.f33F4, 'preRoll', core.f33EA, core.f33EC);
  M.Reset(core, 1);
  M.PreStep(core, 1);
  console.log('hard reset: p1', P[1].f0, 'vel', P[1].f40, 'F', P[1].f7C);
  M.SetFromBone(core, bones, P[0]);
  M.LocalForce({ f0: P[0], f8: I(), f10: [0, 0, 0], f18: I(), f20: { f0: core.f30, f10: core.f40, f20: core.f50 }, f28: core, f30: core, f38: core, f40: 0, f44: 1 }, P[1]);
  console.log('LocalForce direct: F', P[1].f7C);
  M.setConeReinit(() => {});
  M.fn_143afd380(core);
  console.log('reinit: link rest', links.map(l => l.f20));
}
