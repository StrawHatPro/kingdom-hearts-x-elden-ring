// Bonamik reference evaluator for Sora (KH3 PC) - see build/sora_tmp/native_H_bonamik2.md.
// Links the instruction-level transcriptions (cone.js, chain.js, coll.js, kdi.js, reset.js, wind.js, hfps.js, core.js)
// into: core setup (0x143b07550), CoreUpdate (0x143b0efc0), Step (0x143b0cb60) and the component's 30 Hz evaluation +
// interpolation path (0x143b5b990 / 0x143b590d0).
//
//   const E = require('./bnm_eval.js');
//   const sim = E.create({ refPose, parents, toWorld });          // refPose: component-space {q,t,s}[] (UE, cm)
//   const out = sim.frame(dt, animatedPose);                      // -> { pose, didEval, evalDt, alpha, ... } (hfps path)
//   sim.evaluate(pose, evalDt)                                    // one core evaluation in place (no interpolation)
//
// Field naming of every record: f<HEX OFFSET> of the exe's struct (particle 0x330, link 0x38, cone 0xA0, ...).
'use strict';
const path = require('path');
const M = require('./m.js'); const { f, sqrt, qmul, qrot } = M;
const CORE = require('./core.js'), CONE = require('./cone.js'), CHAIN = require('./chain.js'), COLL = require('./coll.js'),
  KDI = require('./kdi.js'), RESET = require('./reset.js'), WIND = require('./wind.js'), HFPS = require('./hfps.js');

CHAIN.setCone(CONE.fn_143b166d0);
RESET.setConeRefresh((core, bones, r) => CONE.fn_143aee280_cones(core.f3778, bones));
KDI.setDeps({ chainLink: (r, link, cfg) => CHAIN.fn_143b192a0(r, link, cfg), sphereSnap: WIND.fn_143b1b830 });

const LINKTYPE = { Lateral: 0, ParentChild: 1, Bending: 2, PinConstraint: 3, KdiConstraint: 4, ReactionTransfer: 5, InverseFreeConstraint: 6 };
const SHAPE = { None: 0, Sphere: 1, Capsule: 2, Inverse_Sphere: 3, Inverse_Capsule: 4, Plane: 6, Box: 8, Self_Collision: 9 };
const ADDI = { None: 0, Sphere: 1, Spring: 2, GroundAttach: 3, SecondOrder_Sphere: 4 };
const enumOf = (tbl, v) => { if (typeof v === 'number') return v; if (v in tbl) return tbl[v]; const k = Object.keys(tbl).find(k => String(v).endsWith(k)); if (k === undefined) throw new Error('enum ' + v); return tbl[k]; };
const v3 = a => [f(a[0]), f(a[1]), f(a[2])], q4 = a => [f(a[0]), f(a[1]), f(a[2]), f(a[3])];
const G = n => new Array(48).fill(n);

// SQEX_BonamikGlobalConfig CDO (constructor 0x143AE8A30): +0x53 bEnableUpdate, +0x55 bEnableSimulation,
// +0x5C ExpectedFrameRate, +0x60 MayaAnimationFrameRate, +0x64 MayaSimulationFrameRate
const defaultCfg = () => ({ f53: 1, f55: 1, f56: 1, f58: f(1 / 30), f5C: f(30), f60: f(30), f64: f(20) });

// LinkDesc in the exe's layout from a plain sora_bonamik.json row
const linkDesc = l => ({
  f20: l.parent, f24: l.child, f28: enumOf(LINKTYPE, l.type), f29: l.selfCollision ? 1 : 0, f2A: l.jointTwist ? 1 : 0, f2B: l.cone ? 1 : 0,
  f2C: l.coneAnimation ? 1 : 0, f30: f(l.linkStr), f34: f(l.linkParentStr), f38: f(l.coneInnerStr), f3C: f(l.coneOuterStr), f40: f(l.coneParentStr),
  f44: f(l.limitYdeg), f48: f(l.limitZdeg), f4C: f(l.coneOffsetYdeg), f50: f(l.coneOffsetZdeg), f54: f(l.limitOffsetYdeg), f58: f(l.limitOffsetZdeg),
  f5C: l.collisionReceiver ? 1 : 0, f5D: l.collisionRepulsor ? 1 : 0, f5E: l.lra ? 1 : 0, f60: f(l.lraStr), f64: f(l.lraDamping), f68: l.linkBatchNo,
  f6C: l.coneBatchNo, f70: l.collisionBatchNo, f74: f(l.pinRadius), f78: f(l.pinStr), f7C: f(l.kdiTrans), f80: f(l.kdiRot), f84: f(l.concaveRatio),
});

// ---------------------------------------------------------------------------------------------------------------------
// Core setup: 0x143af0550 (defaults) + 0x143b07550 (from the asset). bones = component-space reference pose of the mesh
// (owner vf50, 0x143b585a0, builds it from the mesh RefSkeleton) [C].
function setupCore(data, owner, cfg, opts = {}) {
  const core = {
    // frame transforms (filled below / by reset.js)
    f0: [0, 0, 0, 1], f10: [0, 0, 0], f20: [1, 1, 1], f30: [0, 0, 0, 1], f40: [0, 0, 0], f50: [1, 1, 1], f60: -1,
    f70: [0, 0, 0, 1], f80: [0, 0, 0], f90: [1, 1, 1], fA0: [0, 0, 0, 1], fB0: [0, 0, 0], fC0: [1, 1, 1], fD0: [0, 0, 0, 1], fE0: [0, 0, 0], fF0: [1, 1, 1],
    f100: [0, 0, 0, 1], f110: [0, 0, 0], f120: [1, 1, 1], f130: [0, 0, 0, 1], f140: [0, 0, 0], f150: [1, 1, 1], f160: [0, 0, 0, 1], f170: [0, 0, 0], f180: [1, 1, 1],
    f190: [0, 0, 0, 1], f1A0: [0, 0, 0], f1B0: [1, 1, 1],
    // 0x143af0550 defaults
    f1C0: G(0).map(() => [0, -1, 0]), f400: [], f410: 0, f414: G(0), f4D8: G(null), f658: 0, f65C: 1, f65D: 1, f65E: 0, f65F: 0, f660: 1, f664: 0,
    f668: f(0.2), f66C: f(0.2), f670: f(1), f674: 0, f678: 0, f67C: 0, f680: 0, f684: G(0), f744: G(0), f804: G(2), f8C4: G(0), f8F4: G(1), f924: G(0),
    f954: G(0), fA14: G(f(1)), fAD4: G(11), fB94: G(f(1)), fC54: G(f(1)), fD14: G(f(1)), fDD4: G(f(1)), fE94: G(f(1)),
    f33E8: 0, f33E9: 0, f33EA: 0, f33EC: 0, f33F0: 0, f33F4: 1, f33F5: 0, f33F6: 0, f33F8: G(0), f34B8: G(0), f3578: G(0), f3638: G(0),
    f36F8: f(1), f36FC: f(1), f3700: 0, f3704: 0, f3708: f(1), f370C: f(1), f3710: 0, f3714: 0, f3718: 0, f371C: 0,
    f3720: 0, f3724: [0, 0, 0], f3730: null, f3738: 0, f373C: 0, f3740: 0, f3744: 0, f3748: [], f3750: 0, f3768: [], f3770: 0,
    f3778: null, f37E0: [], f37E8: 0, f37F0: new Uint32Array(4), f3800: null, f3808: 0, f3810: [], f3818: 0, f3840: 0, f3848: owner,
  };
  // time step state (head of 0x143b07550) [C]
  core.f3828 = 0;
  const fps = opts.engineFps > 0 ? f(opts.engineFps) : cfg.f5C;            // (*0x149DE3B98)->vf[0x280]() if > 0 [?], else ExpectedFrameRate
  const b = CORE.nearestBucket(f(1 / f(1 / fps)));
  core.f3820 = b; core.f3824 = b;
  const st = f(1 / CORE.FPS_TABLE[b]); core.f382C = st; core.f3830 = st; core.f3834 = st;
  core.f3838 = f(cfg.f60 / cfg.f64); core.f383C = f(f(0.6) / cfg.f60);
  const scene = core.f3778 = {
    f0: [], f8: 0, f10: [], f18: 0, f20: [], f28: 0, f30: [], f38: 0, f40: [], f48: 0, f4C: 0, f50: [], f58: 0, f60: [], f68: 0, f70: [],
    f738: [], f748: [], f750: 0, f758: [], f760: 0, f768: [], f770: 0, f778: [], f780: 0, f7AC: 0,
  };
  const S = data.scene, ver = data.source.versionNo;
  core.f658 = ver;
  if (ver < 0x960) throw new Error('version < 2400 not modelled');
  core.f65D = S.enableGroundCollision ? 1 : 0;
  core.f660 = enumOf({ NONE: 0, LOCAL_FORCE: 1, LSS: 2 }, S.fastMotionHandlingMode);
  core.f664 = /BATCH/.test(String(S.jointLinkSimulationMode)) ? 1 : 0;
  core.f668 = f(S.lssCoeffTran); core.f66C = f(S.lssCoeffRot); core.f674 = S.subStep; core.f670 = f(1);
  const up0 = (S.refUpVec === 1) ? [0, 0, 1] : [0, -1, 0];                 // scene m_RefUpVec == 1 and version >= 2400
  const td = opts.teleportDistance !== undefined ? opts.teleportDistance : data.user_data.TeleportDistance;
  core.f371C = td > 0 ? f(f(td) * f(td)) : f(40000);
  core.f410 = data.groups.length; core.f678 = 0; core.f67C = 0; core.f680 = 1;
  data.groups.forEach((g, i) => {
    if (g.id !== i) throw new Error('group id != index');
    core.f1C0[i] = g.refUpVec === 0 ? up0.slice() : g.refUpVec === 1 ? [0, -1, 0] : [0, 0, 1];
    core.f414[i] = g.id; core.f4D8[g.id] = g.name; core.f684[i] = g.tagId; core.f744[i] = g.collisionTagId;
    core.f804[i] = g.enable ? 2 : 0; core.f3578[i] = g.enable ? 0 : f(1); core.f3638[i] = core.f3578[i];
    core.f924[i] = g.enableGround ? 1 : 0; core.f954[i] = 0; core.fA14[i] = f(1);
    if (core.f678 < g.solverIter) core.f678 = g.solverIter;
    if (core.f67C < g.linkIter) core.f67C = g.linkIter;
  });
  const bones = owner.vf50();
  core.f3808 = bones.length; core.f3818 = bones.length; core.f3810 = new Array(bones.length).fill(0);
  core.f60 = bones.findIndex((_, i) => owner.boneName(i) === S.rootName);                  // m_RootName ("Trans"); only used by LSS mode
  const W = owner.vf48();
  core.f0 = q4(W.q); core.f10 = v3(W.t); core.f20 = v3(W.s); core.f30 = q4(W.q); core.f40 = v3(W.t); core.f50 = v3(W.s);
  const B0 = bones[owner.vf88()]; core.f130 = q4(B0.q); core.f140 = v3(B0.t); core.f150 = v3(B0.s);
  RESET.fn_143b00530(core, bones);
  // particles (0x143aed230 + constructor 0x143ae5d70) and anchor constraints
  for (const b of data.bodies) {
    const B = bones[b.bone_index];
    const lt = v3(b.localTranslation);
    const sv = [f(B.s[0] * lt[0]), f(B.s[1] * lt[1]), f(B.s[2] * lt[2])];
    const t = M.mul(M.cross(B.q, sv), 2), c2 = M.cross(B.q, t);                            // T + c2 + w*t + v (order of 143b07f6f..)
    const pos = [f(f(f(B.t[0] + c2[0]) + f(B.q[3] * t[0])) + sv[0]), f(f(f(B.t[1] + c2[1]) + f(B.q[3] * t[1])) + sv[1]), f(f(f(B.t[2] + c2[2]) + f(B.q[3] * t[2])) + sv[2])];
    const mass = f(b.mass); let m2 = mass; if (mass < 0) { if (m2 >= f(-0.0001)) m2 = f(-0.0001); } else if (m2 <= f(0.0001)) m2 = f(0.0001);
    const p = {
      f0: pos.slice(), fC: pos.slice(), f20: q4(B.q), f30: q4(B.q), f40: [0, 0, 0], f4C: [0, 0, 0], f58: [0, 0, 0], f64: [0, 0, 0], f70: [0, 0, 0],
      f7C: [0, 0, 0], f88: [0, 0, 0], f94: [0, 0, 0], fA0: [0, 0, 0], fAC: [0, 0, 0], fB8: 0, fBC: 0, fBD: 0,
      fC0: q4(B.q), fD0: v3(B.t), fE0: v3(B.s), fF0: q4(B.q), f100: v3(B.t), f110: v3(B.s), f120: q4(B.q), f130: v3(B.t), f140: v3(B.s),
      f150: v3(b.localScale), f160: [0, 0, 0, 1], f170: lt, f180: q4(b.localRotation), f190: null, f198: null,
      f1A0: b.parentBody, f1A4: b.childBody, f1A8: 0, f1B0: [null, null, null, null], f1D0: [0, 0, 0], f1DC: [0, 0, 0],
      f220: null, f228: 0, f22C: 0, f230: 0, f238: null, f240: null, f248: b.bone_index, f24C: b.index,
      f250: mass, f254: f(1 / m2), f258: f(b.inertia), f25C: f(b.radius), f260: f(b.damping), f264: f(b.bounciness), f268: f(b.friction),
      f26C: f(b.windDrag), f270: f(b.windLift), f274: 0, f275: 0, f276: b.kinematic ? 1 : 0, f277: b.collision ? 1 : 0, f278: b.multipleCollisionObject ? 1 : 0, f279: 0,
      f27C: enumOf(SHAPE, b.colShape), f280: b.collisionBatchNo, f284: f(b.concaveRatio), f288: f(b.lfCoeff), f28C: f(b.parkRate), f290: f(b.maxLF),
      f294: f(b.lfDrag), f298: f(b.lfLift), f2A0: null, f2A8: null, f2B8: b.group, f2BC: b.colId.slice(0, 8),
      f2E0: [null, null, null, null], f300: [0, 0, 0, 0], f310: 0, f314: 0, f318: 0, f31C: 0, f31D: 0,
      f320: f(b.lfCoeff), f324: f(b.parkRate), f328: f(b.maxLF), name: b.joint,
    };
    scene.f0[b.index] = p;
    const at = enumOf(ADDI, b.addiConstType);
    if (at === 3) { core.f37E0.push(p); core.f37E8++; }
    else if (at !== 0) { scene.f10.push({ f0: b.index, f4: at, f8: f(b.addiConstStr), fC: pos.slice(), f18: [0, 0, 0], f24: f(b.addiConstRad), f28: pos.slice(), f34: [0, 0, 0] }); p.fBD = 1; }
  }
  scene.f8 = scene.f0.length; scene.f18 = scene.f10.length;
  for (const p of scene.f0) if (core.f684[p.f2B8] === 11) p.f31D = 1;
  // planes, initial orientations, per-body collision objects
  COLL.initCollisionLists(scene);
  COLL.setupPlanes(core, data.planes.map(pl => ({ m_GroupId: pl.group, m_ParticleA: pl.particleA, m_ParticleB: pl.particleB, m_ParticleC: pl.particleC, m_Thickness: f(pl.thickness), m_Offset: f(pl.offset), m_IsCollision: pl.collision, m_CollisionBatchNo: pl.collisionBatchNo, m_ColId: pl.colId })), 0);
  CORE.initOrientations(core);
  COLL.setupBodyObjects(core);
  // links
  const descs = data.links.map(linkDesc); const n = scene.f8;
  for (const d of descs) {
    const col = { m_ParentId: d.f20, m_ChildId: d.f24, m_IsSelfCollision: d.f29, m_IsCollisionReceiver: d.f5C, m_IsCollisionRepulsor: d.f5D, m_CollisionBatchNo: d.f70, m_ConcaveRatio: d.f84 };
    if (d.f29) { COLL.setupLinkCollision(core, col, null, 0); continue; }
    if (d.f5E) { const P1 = scene.f0[d.f20], P2 = scene.f0[d.f24]; P2.f220 = P1; P2.f22C = d.f60; P2.f230 = d.f64; const dx = f(P2.f0[0] - P1.f0[0]), dy = f(P2.f0[1] - P1.f0[1]), dz = f(P2.f0[2] - P1.f0[2]); P2.f228 = sqrt(f(f(f(dx * dx) + f(dy * dy)) + f(dz * dz))); continue; }
    if (d.f28 === 5) throw new Error('ReactionTransfer link not modelled (none in Sora)');
    if (d.f28 === 3 || d.f28 === 4) continue;
    if (!((d.f20 >>> 0) < n && (d.f24 >>> 0) < n)) continue;
    const L = CHAIN.fn_143aec770(scene, d, 0);
    if (d.f2B) { const c = CONE.fn_143aec370(scene, d, 0); if (L) L.f28 = c; }
    COLL.setupLinkCollision(core, col, L, 0);
  }
  KDI.setupPinKdi(scene, descs, 0);
  CORE.buildRoots(core);                                                   // roots + hard-length flags (143b0a03d..143b0a324)
  CHAIN.blk_143b0a327(core, n);                                            // joint-twist signs and rest angles
  COLL.setupBoneMap(core, bones.length);
  COLL.setupFinishCollision(core);
  owner.vf60 = () => owner._bones;                                          // editable component-space transforms from now on
  RESET.fn_143b01210(core, 0);
  return core;
}

// ---------------------------------------------------------------------------------------------------------------------
// Step 0x143b0cb60 (rcx core, rdx cfg, xmm2 dtSim, xmm3 r)
function step(core, cfg, dtSim, r, deps) {
  if (!(dtSim >= f(1e-8))) return;
  const scene = core.f3778, owner = core.f3848, P = scene.f0;
  const N = core.f674; const invN = f(1 / N); const h = f(dtSim * invN); const invH = f(1 / h);
  const g0 = owner.vf20(); const gs = [f(g0[0] * core.fF0[0]), f(g0[1] * core.fF0[1]), f(g0[2] * core.fF0[2])];
  const q = core.fD0; const t = M.mul(M.cross(q, gs), 2), c2 = M.cross(q, t);
  const g = [f(f(c2[0] + f(q[3] * t[0])) + gs[0]), f(f(c2[1] + f(q[3] * t[1])) + gs[1]), f(f(c2[2] + f(q[3] * t[2])) + gs[2])];
  const ground = COLL.stepGroundQuery(core);
  const W = WIND.stepWind_143b0ce63(core, owner);
  for (let sub = 0; sub < N; sub++) {
    if (scene.f8 > 0) WIND.fn_143b123b0(r, core, scene, core, h, invH, invN, g, W.wind, W.presetWind, deps);
    if (ground.valid && core.f65D) COLL.fn_143b19d80(core, scene, ground.ok, ground.pos, ground.normal);
    if (scene.f18 > 0) WIND.fn_143ae5160(r, { f0: scene, f8: -1 }, h, invH);
    if (scene.f28 > 0) throw new Error('shape matching (0x143b0c170) not transcribed (none in Sora)');
    for (let it = 0; it < core.f678; it++) {
      for (let li = 0; li < core.f67C; li++) {
        for (const p of P) { p.fB8 = 0; p.fAC = [0, 0, 0]; p.fA0 = [0, 0, 0]; }
        WIND.fn_143ae53f0(r, { f0: scene, f8: -1 });
        for (const p of P) {
          if (!p.f276 && core.f804[p.f2B8] > 1 && p.fB8 > 0) {
            let k = f(1 / p.fB8);
            p.f0 = [f(f(k * p.fAC[0]) + p.f0[0]), f(f(k * p.fAC[1]) + p.f0[1]), f(f(k * p.fAC[2]) + p.f0[2])];
            k = f(1 / p.fB8);
            p.f58 = [f(f(k * p.fA0[0]) + p.f58[0]), f(f(k * p.fA0[1]) + p.f58[1]), f(f(k * p.fA0[2]) + p.f58[2])];
          }
        }
      }
      for (let i = 0; i < scene.f750; i++) { const R = P[scene.f748[i]]; if (!R.f240) for (let k = 0; k < R.f1A8; k++) CHAIN.fn_143b192a0(r, R.f1B0[k], core); }
      if (scene.f18 > 0) WIND.fn_143ae5160(r, { f0: scene, f8: -1 }, h, invH);
    }
    for (const p of P) {                                                   // LRA pass (none in Sora) [C]
      const a = p.f220; if (!a || p.f276) continue;
      const dx = f(p.f0[0] - a.f0[0]), dy = f(p.f0[1] - a.f0[1]), dz = f(p.f0[2] - a.f0[2]);
      const l2 = f(f(f(dy * dy) + f(dx * dx)) + f(dz * dz));
      if (f(p.f228 * p.f228) < l2) {
        const len = sqrt(l2), inv = f(1 / len), m = f(f(r * p.f22C) * f(len - p.f228));
        const c = [f(f(inv * dx) * m), f(f(inv * dy) * m), f(f(inv * dz) * m)];
        p.f0 = [f(p.f0[0] - c[0]), f(p.f0[1] - c[1]), f(p.f0[2] - c[2])];
        p.fC = [f(p.fC[0] - f(p.f230 * c[0])), f(p.fC[1] - f(p.f230 * c[1])), f(p.fC[2] - f(p.f230 * c[2]))];
      }
    }
    if (scene.f780 > 0) throw new Error('reaction transfer pass not transcribed (none in Sora)');
    COLL.stepCollision(core, scene, ground);
    KDI.fn_143b0cb60_kdiPass(scene);
    KDI.fn_143b15610(r, core, null, core);
    WIND.fn_143b1e710(core, scene, core, h, invH);
  }
  scene.f7AC = 0;
}

// ---------------------------------------------------------------------------------------------------------------------
// CoreUpdate 0x143b0efc0 (rcx core, rdx cfg, xmm2 dt)
function coreUpdate(core, cfg, dt, deps, stats) {
  dt = f(dt);
  const owner = core.f3848, scene = core.f3778, P = scene.f0;
  if (core.f33E8) throw new Error('variable-fps correction (0x143b0ed00 / 0x143aedcc0) not transcribed (off for Sora)');
  const ts = CORE.timeStep(core, cfg, dt, f(owner.vf90()));
  const dtSim = ts.dtSim, r = ts.r, dtp = ts.dtp, stepBefore = ts.step;
  if (!cfg.f53) return;
  // global weight
  let time = core.f3700;
  if (dtp <= time) {
    if (dtp > f(1e-8)) {
      if (core.f3704 && core.f36F8 < f(0.0001) && core.f36FC >= f(0.0001)) { RESET.fn_143b01210(core, 1); time = core.f3700; core.f3704 = 0; }
      let v = f(f(f(f(core.f36FC - core.f36F8) * dtp) / time) + core.f36F8); if (v >= 0) { if (v >= 1) v = 1; } else v = 0;
      core.f36F8 = v; core.f3700 = f(time - dtp);
    }
  } else { core.f36F8 = core.f36FC; core.f3700 = 0; core.f3704 = 0; }
  for (let g = 0; g < core.f410; g++) if (core.f8C4[g]) throw new Error('LOD pose blend 0x143afe170 not transcribed (no LOD entries for Sora)');
  RESET.fn_143b0e790(core);
  if (cfg.f55) {
    CORE.fn_143b10010(core, dtp);
    CORE.fn_143b11700(core, owner.vf78);
    RESET.fn_143b1da00(core);
    RESET.fn_143afaf80(core);
    if (core.f33F6) RESET.fn_143afd380(core);
    if (core.f33F4) { if (stats) stats.resets = (stats.resets || 0) + 1; RESET.fn_143b01210(core, core.f33F5); }
    if (core.f65C) {
      RESET.fn_143aee280(core, dtp > f(1e-8) ? r : 0);
      COLL.fn_143b10230(core, dtp > f(1e-8) ? dtSim : 0);
      if (!core.f33EA) { if (dtp > f(1e-8)) { step(core, cfg, dtSim, r, deps); if (stats) stats.steps = (stats.steps || 0) + 1; } }
      else {
        core.f33EC = core.f33EC * ((stepBefore < core.f383C ? 1 : 0) + 1); core.f670 = 0;
        for (let i = 0; i < core.f33EC; i++) { if (i > 0) { core.f670 = f(1); COLL.fn_143b10230(core, dtSim); } step(core, cfg, dtSim, r, deps); if (stats) stats.steps = (stats.steps || 0) + 1; }
        core.f33EA = 0; core.f33EC = 0; core.f670 = f(1);
      }
      CHAIN.fn_143b0c600(core);
      const bones = owner.vf60();
      for (let i = 0; i < scene.f750; i++) {
        const R = P[scene.f748[i]];
        if (core.f804[R.f2B8] > 1 && !R.f277) { const kin = R.f198 ? R.f198.f276 : R.f276; if (!kin) CHAIN.fn_143b13b70(core, R, bones, R.f160); }
      }
      CHAIN.fn_143af3f70(core);
      core.f33F4 = 0; core.f33F5 = 0; core.f33F6 = 0;
    }
    const bones = owner.vf60();
    for (const p of P) p.f30 = bones[p.f248].q.slice();
  }
  if (core.f3768.length) HFPS.fn_143afc920(core);
  const bones = owner.vf60();
  for (const p of P) { const B = bones[p.f248]; p.fC0 = B.q.slice(); p.fD0 = B.t.slice(); p.fE0 = B.s.slice(); }
  if (!core.f3840) HFPS.fn_143aefb30(core);
  if (dtp > f(1e-8)) core.f3714 = f(dtSim + core.f3714);
}

// ---------------------------------------------------------------------------------------------------------------------
// opts: refPose (required), parents (required), boneNames, toWorld {q,t,s} | function(frameIndex) (ComponentToWorld of the
// mesh; default identity), gravity (world, default (0,0,-981) = asset gravity with Y/Z swapped * 100, 0x143b61cf0),
// ground: null | function() -> {point:[..], normal:[..]} in world space, wind: function() -> [x,y,z] world (default 0),
// rngSeed, zeroJitter (true: replace the random stream by a constant 0.5 generator => jitter J = 0),
// hardResetAtStart, data (default ../sora_bonamik.json)
function create(opts) {
  const data = opts.data || require(path.join(__dirname, '..', 'sora_bonamik.json'));
  const cfg = opts.cfg || defaultCfg();
  const parents = opts.parents, names = opts.boneNames || [];
  const ident = { q: [0, 0, 0, 1], t: [0, 0, 0], s: [1, 1, 1] };
  const gv = (data.scene.gravity || [0, -9.81, 0]);
  const gmag = f(Math.sqrt(f(f(f(f(gv[0]) * f(gv[0])) + f(f(gv[2]) * f(gv[2])) + f(f(gv[1]) * f(gv[1]))) * 10000)));
  const ginv = f(1 / gmag);
  const gdir = [f(f(ginv * f(gv[0])) * 100), f(f(f(gv[2]) * 100) * ginv), f(f(f(gv[1]) * 100) * ginv)];     // (x, z, y): 0x143b61cf0 [C]
  const gravity = opts.gravity ? v3(opts.gravity) : [f(gmag * gdir[0]), f(gmag * gdir[1]), f(gmag * gdir[2])];
  let world = typeof opts.toWorld === 'function' ? opts.toWorld(0) : (opts.toWorld || ident);
  const owner = {
    _bones: opts.refPose.map(b => ({ q: q4(b.q), t: v3(b.t), s: v3(b.s) })),
    boneName: i => names[i],
    vf10: (pos, normal) => { const gr = opts.ground ? opts.ground() : null; if (!gr) return 0; pos[0] = f(gr.point[0]); pos[1] = f(gr.point[1]); pos[2] = f(gr.point[2]); normal[0] = f(gr.normal[0]); normal[1] = f(gr.normal[1]); normal[2] = f(gr.normal[2]); return 1; },
    vf20: () => gravity,
    vf38: () => (opts.wind ? v3(opts.wind()) : [0, 0, 0]),
    vf40: () => null,
    vf48: () => world,
    vf50: () => opts.refPose.map(b => ({ q: q4(b.q), t: v3(b.t), s: v3(b.s) })),
    vf60: null,
    vf78: i => parents[i],
    vf88: () => 0,
    vf90: () => 1,
    vfD0: () => 0, vfF0: () => [0, 0, 0],
  };
  owner.vf60 = () => owner._bones;
  const core = setupCore(data, owner, cfg, opts);
  if (opts.hardResetAtStart) { core.f33F4 = 1; core.f33F5 = 1; }
  const rng = opts.zeroJitter ? { state: null, next: () => 0x80000000 } : WIND.makeRng(opts.rngSeed);
  const deps = { rng, sinf: x => f(Math.sin(x)) };
  const stats = { steps: 0, resets: 0 };
  const evaluate = (pose, evalDt) => { owner._bones = pose; coreUpdate(core, cfg, evalDt, deps, stats); return owner._bones; };
  const runner = HFPS.createRunner(parents, (pose, evalDt) => evaluate(pose, evalDt), { core, ext: { fn_143b10010: CORE.fn_143b10010, fn_143b11700: c => CORE.fn_143b11700(c, owner.vf78), fn_143afe170: () => { throw new Error('0x143afe170 not transcribed'); } } });
  let frameIndex = 0;
  return {
    core, cfg, owner, stats, rng, data,
    setWorld: w => { world = w; },
    evaluate,                                              // direct core evaluation (non-high-fps path: core blends itself)
    frame(dt, pose) { if (typeof opts.toWorld === 'function') world = opts.toWorld(frameIndex); frameIndex++; return runner.step(dt, pose); },
    particles: () => core.f3778.f0,
  };
}
module.exports = { create, setupCore, coreUpdate, step, defaultCfg, linkDesc };
