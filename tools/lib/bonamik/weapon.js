// Kingdom Key keychain: the weapon's own Bonamik asset (w_so010_Bonamik) on the reference evaluator bnm_eval.js.
// Same runtime as Sora's hair / cloth; no branch the evaluator leaves untranscribed is reached by this asset
// (build/sora_tmp/IMPL_KEYCHAIN_STATE.md). Used by tools/sora_weapon_pack.js (runtime records, --check) and stand-alone:
//   node tools/lib/bonamik/weapon.js            prints the checks
//   const W = require('./weapon.js'); const k = W.load(); const sim = k.create({ toWorld }); sim.frame(dt, k.refPose())
'use strict';
const path = require('path'); const fs = require('fs');
const E = require('./bnm_eval.js');
const PACK = path.join(__dirname, '../../../build/sora_pack/weapon');
const f = Math.fround;
// TeleportDistance is serialized neither in the mesh's SQEX_BonamikAssetUserData (+0x98) nor in the asset
// (m_TeleportDistance +0x560). Neither constructor writes the field (0x143AE89D0, 0x143AE88A0: read at instruction
// level) and UObject memory is zero-filled, so instance Setup 0x143b61cf0 passes 0 and core setup stores
// teleportDistSq = 40000 (0x143b07a2b: dist > 0 ? dist^2 : 40000) = 200 cm.
const TELEPORT_DISTANCE_DEFAULT = 0;
const qmul = (a, b) => [a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1], a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0], a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3], a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]];
const qrot = (q, v) => { const tx = 2 * (q[1] * v[2] - q[2] * v[1]), ty = 2 * (q[2] * v[0] - q[0] * v[2]), tz = 2 * (q[0] * v[1] - q[1] * v[0]); return [v[0] + q[3] * tx + (q[1] * tz - q[2] * ty), v[1] + q[3] * ty + (q[2] * tx - q[0] * tz), v[2] + q[3] * tz + (q[0] * ty - q[1] * tx)]; };
const qnorm = q => { const l = Math.hypot(...q); return q.map(x => x / l); };
const qaxis = (ax, deg) => { const h = deg * Math.PI / 360, s = Math.sin(h), l = Math.hypot(...ax); return [ax[0] / l * s, ax[1] / l * s, ax[2] / l * s, Math.cos(h)]; };
const dist = (a, b) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
// ComponentToWorld {q,t,s} of the weapon mesh, float32
const world = (q, t) => ({ q: qnorm(q).map(f), t: (t || [0, 0, 0]).map(f), s: [1, 1, 1] });
// The blade is component +X (grip -> tip); the keychain leaves the pommel along component -X.
const ORIENT = {
  horizontal: [0, 0, 0, 1],                 // blade along world +X (the reference pose as authored)
  blade_up: qaxis([0, 1, 0], -90),          // component +X -> world +Z: the chain's rest direction is straight down
  blade_down: qaxis([0, 1, 0], 90),         // component +X -> world -Z: the chain's rest direction is straight up
};

// bones: manifest.skeleton.bones (with *_ue fields); data: the pack's data/bonamik.json object
function load(src = {}) {
  const bones = src.bones || JSON.parse(fs.readFileSync(path.join(PACK, 'manifest.json'), 'utf8')).skeleton.bones;
  const data = () => JSON.parse(JSON.stringify(src.data || JSON.parse(fs.readFileSync(path.join(PACK, 'data/bonamik.json'), 'utf8'))));
  const parents = bones.map(b => b.parent), names = bones.map(b => b.name);
  function componentSpace(local) {
    const cs = [];
    for (let i = 0; i < bones.length; i++) {
      const l = local[i], pi = parents[i]; let o;
      if (pi < 0) o = { q: l.q.slice(), t: l.t.slice(), s: l.s.slice() };
      else { const P = cs[pi]; const rt = qrot(P.q, [P.s[0] * l.t[0], P.s[1] * l.t[1], P.s[2] * l.t[2]]); o = { q: qnorm(qmul(P.q, l.q)), t: [rt[0] + P.t[0], rt[1] + P.t[1], rt[2] + P.t[2]], s: [l.s[0] * P.s[0], l.s[1] * P.s[1], l.s[2] * P.s[2]] }; }
      cs.push(o);
    }
    return cs.map(x => ({ q: x.q.map(f), t: x.t.map(f), s: x.s.map(f) }));
  }
  const refLocal = () => bones.map(b => ({ q: b.rotation_ue.slice(), t: b.translation_ue.slice(), s: b.scale_ue.slice() }));
  const refPose = () => componentSpace(refLocal());
  const create = (o = {}) => { const d = data(); if (d.user_data.TeleportDistance == null) d.user_data.TeleportDistance = TELEPORT_DISTANCE_DEFAULT; return E.create(Object.assign({ refPose: refPose(), parents, boneNames: names, data: d, zeroJitter: true }, o)); };
  return { bones, parents, names, data, componentSpace, refLocal, refPose, create };
}

// The runtime records right after core setup on the reference pose (schema of build/sora_pack/data/bonamik2.json -> runtime2,
// written for Sora by build/sora_tmp/bnm/dump2.js).
function runtimeRecords(k) {
  const sim = k.create(); const data = sim.data, core = sim.core, sc = core.f3778, P = sc.f0;
  const idx = p => (p ? p.f24C : -1);
  const obj = o => ({ type: o.f18, a: idx(o.f0), b: idx(o.f8), c: idx(o.f10), batch: o.f1C, group: o.f38, concave: o.f64, boundRadius: o.f80, thickness: o.f5C });
  const r = {
    gravityWorld: sim.owner.vf20(),
    iterations: { substeps: core.f674, outer: core.f678, link: core.f67C, collision: core.f680 },
    timeStep: { bucket: core.f3824, step: core.f3830, kAnimSim: core.f3838, kFast: core.f383C, teleportDistSq: core.f371C },
    flags: { fastMotionMode: core.f660, jointLinkMode: core.f664, groundCollision: core.f65D, resetPending: 1, resetHard: 0 },
    groups: data.groups.map((g, i) => ({ id: i, name: g.name, state: core.f804[i], animWeight: core.f3578[i], refUp: core.f1C0[i], tag: core.f684[i], collisionTag: core.f744[i], enableGround: core.f924[i], windPresetSlot: core.fAD4[i], charaWindScale: core.f954[i], presetWindScale: core.fA14[i], innerConeScale: core.fB94[i], outerConeScale: core.fC54[i], localForceScale: core.fD14[i], worldWindScale: core.fDD4[i], massScale: core.fE94[i], aeroEnable: core.f8F4[i] })),
    particles: P.map((p, i) => ({ index: i, joint: p.name, bone: p.f248, pos: p.f0, quat: p.f20, parent: idx(p.f190), child: idx(p.f198), pcLinks: p.f1B0.slice(0, p.f1A8).map(l => sc.f40.indexOf(l)), isChainChild: p.fBC, hasAnchor: p.fBD, kinematic: p.f276, collision: p.f277, multiCollision: p.f278, twistNeighbours: p.f2E0.slice(0, p.f310).map(idx), twistSigns: p.f300.slice(0, p.f310), twistRest: p.f318, pcTwist: p.f314, dir1DC: p.f1DC, kdiDriven: p.f240 ? 1 : 0, driverOfMultiCollision: p.f278 ? ((core.f400[p.f248] === 0 || core.f400[p.f248] === 9999) ? -1 : core.f400[p.f248] - 1) : -1 })),
    links: sc.f40.slice(0, sc.f48).map((l, i) => ({ index: i, parent: idx(l.f0), child: idx(l.f8), batch: l.f10, str: l.f14, parentStr: l.f18, rest: l.f20, isParentChild: l.f24, hardLength: l.f26, cone: l.f28 ? sc.f30.indexOf(l.f28) : -1, collisionObject: l.f30 ? sc.f50.indexOf(l.f30) : -1 })),
    cones: sc.f30.slice(0, sc.f38).map((c, i) => ({ index: i, parent: idx(c.f0), child: idx(c.f8), batch: c.f10, outerStr: c.f14, parentShare: c.f18, animation: c.f20, innerStr: c.f28, limitY: c.f30, limitZ: c.f34, restLength: c.f40, length: c.f44, q50: c.f50, q60: c.f60, q70: c.f70, q80: c.f80 })),
    anchors: sc.f10.map(a => ({ particle: a.f0, joint: P[a.f0].name, type: a.f4, strength: a.f8, anchor: a.fC, radius: a.f24 })),
    chainRoots: sc.f748.slice(), chainRootNames: sc.f748.map(i => P[i].name),
    dynObjects: sc.f50.map(obj),
    colliderLists: Object.fromEntries(sc.f70.map((l, c) => [c, (l || []).map(obj)]).filter(x => x[1].length)),
    selfPairs: sc.f60.map(obj),
    kdi: sc.f768.map(x => ({ driven: idx(x.f0), drivers: x.f8.map(e => ({ driver: idx(e.f0), local: e.f8, rel: e.f20, wTrans: e.f30, wRot: e.f34 })) })),
    counts: { particles: sc.f8, links: sc.f48, cones: sc.f38, anchors: sc.f18, chainRoots: sc.f750, dynObjects: sc.f58, selfPairs: sc.f68, kdi: sc.f770, pin: sc.f760, hardLengthLinks: sc.f40.slice(0, sc.f48).filter(l => l.f26).length, assetTotals: data.runtime && data.runtime.totals },
  };
  let n = 0; data.links.forEach((l, i) => { if (l.selfCollision || l.lra) return; if (['PinConstraint', 'KdiConstraint', 'ReactionTransfer'].includes(l.type)) return; if (r.links[n]) r.links[n++].assetLink = i; });
  return r;
}

// ---- checks (new feature only): the chain hangs under gravity and settles; segment lengths held; the bones Bonamik does not
// drive are bit-identical; a swing of the weapon stays bounded with no NaN, the chain lags and settles again.
const toW = (W, p) => { const r = qrot(W.q, p); return [r[0] + W.t[0], r[1] + W.t[1], r[2] + W.t[2]]; };
const dynOf = sim => sim.particles().filter(p => !p.f276);
const finite = sim => sim.particles().every(p => p.f0.every(Number.isFinite) && p.f40.every(Number.isFinite));
function segErr(sim) { const sc = sim.core.f3778; let m = 0; for (let i = 0; i < sc.f48; i++) { const l = sc.f40[i]; if (l.f28) m = Math.max(m, Math.abs(dist(l.f0.f0, l.f8.f0) - l.f28.f44)); } return m; }
function hang(k, name, n = 120) {
  const W = world(ORIENT[name], [0, 0, 100]); const sim = k.create({ toWorld: W }); const ref = k.refPose();
  let prev = null, settle = -1, out, same = true;
  for (let i = 0; i < n; i++) {
    out = sim.frame(1 / 30, k.refPose());
    const cur = dynOf(sim).map(p => p.f0.slice());
    if (prev) { const mv = Math.max(...cur.map((c, j) => dist(c, prev[j]))); if (mv >= 1e-3) settle = -1; else if (settle < 0) settle = i + 1; }
    prev = cur;
    for (const b of [0, 1]) for (const key of ['q', 't', 's']) if (out.pose[b][key].some((v, j) => v !== ref[b][key][j])) same = false;
  }
  const P = sim.particles(); const root = toW(W, P[sim.core.f3778.f748[0]].f0); const dyn = dynOf(sim).map(p => toW(W, p.f0));
  const tip = dyn[dyn.length - 1]; const d = dist(tip, root);
  // the cones limit how far the chain can turn from the weapon: 75 deg at the first joint, 70 deg at each later one
  return { orientation: name, evaluations: n, settled_from_evaluation: settle, all_nodes_below_root: dyn.every(p => p[2] < root[2]), tip_below_root_cm: +(root[2] - tip[2]).toFixed(3), tip_down_cos: +((root[2] - tip[2]) / d).toFixed(4), max_segment_length_error_cm: segErr(sim), non_driven_bones_bit_identical: same, finite: finite(sim) };
}
function swing(k) {
  // the weapon turns 180 deg about world Y in 0.4 s around a point 60 cm up the blade, then is held
  const at = i => { const t = Math.min(i / 30, 0.4) / 0.4; const q = qmul(qaxis([0, 1, 0], 180 * t), ORIENT.blade_up); const piv = [0, 0, 160], off = qrot(q, [60, 0, 0]); return world(q, [piv[0] - off[0], piv[1] - off[1], piv[2] - off[2]]); };
  let W = at(0); const sim = k.create({ toWorld: W }); const ref = k.refPose();
  for (let i = 0; i < 60; i++) sim.frame(1 / 30, k.refPose());                    // settle first
  const sc = sim.core.f3778; const total = sc.f30.reduce((a, c) => a + c.f44, 0);
  let maxOff = 0, maxLag = 0, ok = true, maxSeg = 0, settle = -1, prev = null;
  for (let i = 0; i < 150; i++) {
    W = at(i); sim.setWorld(W); sim.frame(1 / 30, k.refPose());
    ok = ok && finite(sim); maxSeg = Math.max(maxSeg, segErr(sim));
    const P = sim.particles(), root = P[sc.f748[0]].f0, tip = P[P.length - 1].f0;
    maxOff = Math.max(maxOff, dist(tip, root));
    if (i <= 14) maxLag = Math.max(maxLag, dist(tip, ref[ref.length - 1].t));       // a rigid chain gives 0: distance from the animated tip while swinging
    const cur = dynOf(sim).map(p => p.f0.slice());
    if (prev) { const mv = Math.max(...cur.map((c, j) => dist(c, prev[j]))); if (mv >= 1e-3) settle = -1; else if (settle < 0) settle = i + 1; }
    prev = cur;
  }
  return { evaluations: 150, swing: '180 deg about world Y in 0.4 s, then held', finite: ok, chain_length_cm: +total.toFixed(3), max_tip_distance_from_root_cm: +maxOff.toFixed(3), bounded: ok && maxOff <= 2 * total, max_tip_lag_during_swing_cm: +maxLag.toFixed(3), max_transient_segment_length_error_cm: +maxSeg.toFixed(3), transient_note: 'the chain whips over when the weapon stops and is pushed by the guard spheres after the length pass (collisions run last in Step 0x143b0cb60): segments stretch for single evaluations', final_segment_length_error_cm: segErr(sim), settled_from_evaluation: settle, teleport_resets: sim.stats.resets - 1 };
}
function validate(k) {
  const r = { evaluator: 'tools/lib/bonamik/bnm_eval.js on data/bonamik.json; jitter stream zeroed; world gravity (0, 0, -981); 30 fps', hang: ['blade_up', 'horizontal'].map(n => hang(k, n)), swing: swing(k) };
  r.ok = r.hang.every(h => h.settled_from_evaluation > 0 && h.all_nodes_below_root && h.max_segment_length_error_cm < 1e-3 && h.non_driven_bones_bit_identical && h.finite)
    && r.swing.bounded && r.swing.max_tip_lag_during_swing_cm > 1 && r.swing.settled_from_evaluation > 0 && r.swing.final_segment_length_error_cm < 1;   // resting on the guard collider keeps a contact stretch (0.34 cm)
  return r;
}
module.exports = { PACK, load, runtimeRecords, validate, hang, swing, world, ORIENT, qmul, qrot, qnorm, qaxis, TELEPORT_DISTANCE_DEFAULT };
if (require.main === module) { const k = load(); const v = validate(k); console.log(JSON.stringify(v, null, 1)); console.log(JSON.stringify(runtimeRecords(k).counts)); process.exit(v.ok ? 0 : 1); }
