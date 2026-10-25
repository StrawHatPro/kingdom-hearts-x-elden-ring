// writes build/sora_tmp/sora_bonamik2.json: sora_bonamik.json + the RUNTIME records as core setup 0x143b07550 builds them
// from the mesh reference pose (computed by the linked transcription, bnm_eval.js setupCore). node dump2.js
const fs = require('fs'), path = require('path');
const E = require('./bnm_eval.js'); const A = require('./anim.js');
const data = require('../sora_bonamik.json');
const parents = A.bones.map(b => b.parent), names = A.bones.map(b => b.name);
const sim = E.create({ refPose: A.refPose(), parents, boneNames: names, zeroJitter: true });
const core = sim.core, sc = core.f3778, P = sc.f0;
const idx = p => (p ? p.f24C : -1);
const out = Object.assign({}, data);
out._schema2 = {
  about: 'Extension of sora_bonamik.json (pass 2). Everything under "runtime2" is the state right after core setup (0x143b07550) on the mesh reference pose, produced by build/sora_tmp/bnm/bnm_eval.js (instruction-level transcriptions). Indices are particle (= body) indices; quaternions [x,y,z,w]; positions cm in mesh component space; angles rad. Field names in comments are byte offsets of the exe records.',
  gravityWorld: 'owner gravity (interface +0x20 -> 0x143b57440): asset m_Gravity with Y and Z swapped, * 100 (0x143b61cf0).',
  groups: 'per group g: state core+0x804 (2 simulated, 0 off), animWeight core+0x3578, refUp core+0x1C0, tag +0x684, collisionTag +0x744, enableGround +0x924, windPresetSlot +0xAD4 (11 = none), scales: charaWind +0x954, presetWind +0xA14, innerCone +0xB94, outerCone +0xC54, localForce +0xD14, worldWind +0xDD4, mass +0xE94, aeroEnable +0x8F4.',
  particles: 'per particle: pos +0x00, quat +0x20 (frame: +X along the segment to the child, twist of the animated bone), parent +0x190 / child +0x198 (after link setup), pcLinks +0x1B0 (link indices), isChainChild +0xBC, hasAnchor +0xBD, kinematic +0x276 (after the KDI override), twistNeighbours +0x2E0, twistSigns +0x300, twistRest +0x318, pcTwist +0x314, dir1DC +0x1DC, driverOfMultiCollision: for m_IsMultipleCollisionObject particles the particle they are attached to by 0x143b10230 (core+0x400 map) or -1.',
  links: 'link records scene+0x40 (stride 0x38) in creation order: parent +0x00, child +0x08, batch +0x10, str +0x14, parentStr +0x18, rest +0x20, isParentChild +0x24, hardLength +0x26, cone +0x28 (cone index), assetLink = row of sora_bonamik.json links.',
  cones: 'cone records scene+0x30 (stride 0xA0): parent +0x00, child +0x08, batch +0x10, outerStr +0x14 and +0x2C, parentShare +0x18, animation +0x20, innerStr +0x28, limitY +0x30, limitZ +0x34, restLength +0x40, length +0x44 (= rest * parent bone scale x), q50 +0x50 (limit frame rel. to the grandparent segment frame * limit offset), q60 +0x60 (cone axis frame), q70 +0x70 (limit offset quat), q80 +0x80 (cone offset quat).',
  anchors: 'anchor constraints scene+0x10 (stride 0x40): particle +0x00, type +0x04 (1 Sphere, 2 Spring, 4 SecondOrder_Sphere), strength +0x08, anchor +0x0C, radius +0x24.',
  chainRoots: 'scene+0x748 (count +0x750) in list order.',
  dynObjects: 'scene+0x50 after the stable sort by batch: dynamic-side collision objects {type +0x18 (1 sphere, 2 capsule, 6 plane), a +0x00, b +0x08, batch +0x1C, group +0x38, concave +0x64, boundRadius +0x80}; each is tested against the collider lists named by particle a colId[8].',
  colliderLists: 'scene+0x70[c]: per group c the collider objects (same fields).',
  selfPairs: 'scene+0x60: self-collision pairs (type 9) a/b.',
  kdi: 'scene+0x768 records: driven particle, drivers [{driver, local +0x08, rel +0x20, wTrans +0x30, wRot +0x34}].',
};
const obj = o => ({ type: o.f18, a: idx(o.f0), b: idx(o.f8), c: idx(o.f10), batch: o.f1C, group: o.f38, concave: o.f64, boundRadius: o.f80, thickness: o.f5C });
const g0 = sim.owner.vf20();
out.runtime2 = {
  gravityWorld: g0,
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
  kdi: sc.f768.map(r => ({ driven: idx(r.f0), drivers: r.f8.map(e => ({ driver: idx(e.f0), local: e.f8, rel: e.f20, wTrans: e.f30, wRot: e.f34 })) })),
  counts: { particles: sc.f8, links: sc.f48, cones: sc.f38, anchors: sc.f18, chainRoots: sc.f750, dynObjects: sc.f58, selfPairs: sc.f68, kdi: sc.f770, pin: sc.f760, hardLengthLinks: sc.f40.slice(0, sc.f48).filter(l => l.f26).length, assetTotals: data.runtime && data.runtime.totals },
};
// asset link row for each runtime link (creation order = asset order of the rows that reach 0x143aec770)
{ let k = 0; data.links.forEach((l, i) => { if (l.selfCollision || l.lra) return; if (['PinConstraint', 'KdiConstraint', 'ReactionTransfer'].includes(l.type)) return; out.runtime2.links[k++].assetLink = i; }); }
fs.writeFileSync(path.join(__dirname, '..', 'sora_bonamik2.json'), JSON.stringify(out));
console.log('written', JSON.stringify(out.runtime2.counts), 'bytes', fs.statSync(path.join(__dirname, '..', 'sora_bonamik2.json')).size);
