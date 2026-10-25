// Static validations of the Bonamik reference evaluator (no game needed). usage: node validate.js [evals=240]
// (a) rest stability in the reference pose and in IdleN (0001) frame 0, (b) length invariants, (c) determinism,
// (d) dt handling: 60 fps and 30 fps frame sequences give the same evaluation states.
const E = require('./bnm_eval.js'); const A = require('./anim.js');
const parents = A.bones.map(b => b.parent), names = A.bones.map(b => b.name);
const NE = +(process.argv[2] || 240);
const clone = pose => pose.map(b => ({ q: b.q.slice(), t: b.t.slice(), s: b.s.slice() }));
const dist = (a, b) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
const qang = (a, b) => { const d = Math.abs(a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]); return 2 * Math.acos(Math.min(1, d)) * 180 / Math.PI; };
const qrot = (q, v) => { const tx = 2 * (q[1] * v[2] - q[2] * v[1]), ty = 2 * (q[2] * v[0] - q[0] * v[2]), tz = 2 * (q[0] * v[1] - q[1] * v[0]); return [v[0] + q[3] * tx + (q[1] * tz - q[2] * ty), v[1] + q[3] * ty + (q[2] * tx - q[0] * tz), v[2] + q[3] * tz + (q[0] * ty - q[1] * tx)]; };
const mk = (o) => E.create(Object.assign({ refPose: A.refPose(), parents, boneNames: names }, o));
const stateHash = sim => { let h = 0x811c9dc5; const buf = new Float32Array(1), u = new Uint32Array(buf.buffer); for (const p of sim.particles()) for (const v of [...p.f0, ...p.f40, ...p.f20]) { buf[0] = v; h = Math.imul(h ^ u[0], 0x01000193) >>> 0; } return h.toString(16); };
const chainOf = (sim) => { const sc = sim.core.f3778, P = sc.f0, m = new Map(); const walk = (p, root) => { m.set(p, root); for (let k = 0; k < p.f1A8; k++) walk(p.f1B0[k].f8, root); }; for (let i = 0; i < sc.f750; i++) walk(P[sc.f748[i]], P[sc.f748[i]]); return m; };

function restRun(label, pose, o) {
  const sim = mk(o); const sc = sim.core.f3778, P = sc.f0; const dyn = P.filter(p => !p.f276);
  let prev = null, hist = [], out;
  for (let i = 0; i < NE; i++) {
    out = sim.frame(1 / 30, clone(pose));
    const cur = dyn.map(p => p.f0.slice());
    if (prev) hist.push(Math.max(...cur.map((c, k) => dist(c, prev[k]))));
    prev = cur;
  }
  const speed = Math.max(...dyn.map(p => Math.hypot(...p.f40)));
  const settle = hist.findIndex(x => x < 1e-3);
  const exact = hist.findIndex((x, i) => hist.slice(i).every(y => y === 0));
  console.log(`\n(a) rest stability [${label}]: ${NE} evaluations at 30 fps; steps run ${sim.stats.steps}, resets ${sim.stats.resets}`);
  console.log(`    max node move per evaluation: first ${hist[0].toExponential(2)} cm, #10 ${hist[9].toExponential(2)}, #60 ${hist[59].toExponential(2)}, last ${hist[hist.length - 1].toExponential(2)}; < 0.001 cm from evaluation ${settle + 1}; bit-exact fixed point from evaluation ${exact < 0 ? 'never' : exact + 1}; final max |vel| ${speed.toExponential(2)} cm/s`);
  // distance from the asset rest configuration = node position given by the animated bone (bone.T + rot(bone.R, S*localT))
  const ch = chainOf(sim); const per = {};
  for (const p of dyn) { const B = pose[p.f248]; const r = qrot(B.q, [B.s[0] * p.f170[0], B.s[1] * p.f170[1], B.s[2] * p.f170[2]]); const rest = [r[0] + B.t[0], r[1] + B.t[1], r[2] + B.t[2]]; const d = dist(p.f0, rest); const root = ch.get(p); const key = root ? root.name : '(no chain)'; (per[key] = per[key] || []).push([p.name, d]); }
  console.log('    distance of the settled nodes from the animated (asset rest) position, per chain [cm]:');
  const rows = Object.keys(per).map(k => `${k}: ` + per[k].map(x => `${x[0].replace(/^.*_/, m => m)} ${x[1].toFixed(3)}`).join(', '));
  for (const r of rows) console.log('      ' + r);
  const all = [].concat(...Object.values(per)).map(x => x[1]);
  console.log(`    all 57 dynamic nodes: max ${Math.max(...all).toFixed(3)} cm, mean ${(all.reduce((a, b) => a + b, 0) / all.length).toFixed(3)} cm`);
  // final displayed pose vs animated pose for the driven bones
  let mt = 0, mr = 0, nd = 0; const w = sim.core.f3810;
  for (let i = 0; i < pose.length; i++) { if (!(w[i] > 0)) { if (out.pose[i].t.some((v, k) => v !== Math.fround(pose[i].t[k])) || out.pose[i].q.some((v, k) => v !== Math.fround(pose[i].q[k]))) throw new Error('non-driven bone changed: ' + names[i]); continue; } nd++; mt = Math.max(mt, dist(out.pose[i].t, pose[i].t)); mr = Math.max(mr, qang(out.pose[i].q, pose[i].q)); }
  console.log(`    displayed pose: ${nd} weighted bones, max translation offset from the animation ${mt.toFixed(3)} cm, max rotation offset ${mr.toFixed(2)} deg; the other ${pose.length - nd} bones are unchanged`);
  return sim;
}
function lengths(label, sim) {
  const sc = sim.core.f3778; let hard = 0, nHard = 0, soft = 0, nSoft = 0, softRel = 0, kk = 0, nkk = 0;
  for (let i = 0; i < sc.f48; i++) { const l = sc.f40[i]; const d = dist(l.f0.f0, l.f8.f0); if (l.f0.f276 && l.f8.f276) { kk = Math.max(kk, Math.abs(d - l.f20)); nkk++; continue; }
    if (l.f28) { hard = Math.max(hard, Math.abs(d - l.f28.f44)); nHard++; } else if (l.f26) { hard = Math.max(hard, Math.abs(d - l.f20)); nHard++; } else { soft = Math.max(soft, Math.abs(d - l.f20)); softRel = Math.max(softRel, Math.abs(d - l.f20) / Math.max(l.f20, 1e-6)); nSoft++; } }
  let anch = 0; for (const a of sc.f10) { const p = sc.f0[a.f0]; if (a.f4 === 1) anch = Math.max(anch, dist(p.f0, a.fC) - a.f24); }
  console.log(`(b) length invariants [${label}]: ${nHard} cone / hard ParentChild segments: max |length - rest| ${hard.toExponential(2)} cm; ${nSoft} soft distance links (Lateral/Bending/PC without cone): max |length - rest| ${soft.toFixed(4)} cm (${(softRel * 100).toFixed(2)} %); ${nkk} kinematic-kinematic links: ${kk.toExponential(2)} cm; sphere anchors: max distance beyond radius ${anch.toExponential(2)} cm`);
}

const ref = A.refPose(); const clip = A.loadClip('0001.glb'); const idle0 = clip.poseAt(0);
const s1 = restRun('reference pose, jitter stream zeroed', ref, { zeroJitter: true }); lengths('reference pose', s1);
const s2 = restRun('IdleN 0001 frame 0, jitter stream zeroed', idle0, { zeroJitter: true }); lengths('IdleN frame 0', s2);
const s2r = restRun('IdleN 0001 frame 0, xorshift stream from the per-thread seed', idle0, {}); lengths('IdleN frame 0 (rng)', s2r);
console.log(`    random stream dependence at rest: state hash zero-jitter ${stateHash(s2)} vs seeded ${stateHash(s2r)} -> ${stateHash(s2) === stateHash(s2r) ? 'IDENTICAL (rest state independent of the stream)' : 'different'}`);
const s2h = restRun('IdleN 0001 frame 0, HARD reset at start (as Reset(core,1) / ResetAndPreRoll would do)', idle0, { zeroJitter: true, hardResetAtStart: true }); lengths('IdleN frame 0 (hard reset)', s2h);
{ const P2 = s2.particles(), Ph = s2h.particles(); let d = 0; P2.forEach((p, i) => { if (!p.f276) d = Math.max(d, dist(p.f0, Ph[i].f0)); }); console.log(`    soft-start vs hard-reset final configuration: max node distance ${d.toExponential(2)} cm`); }

// (c) determinism: animated clip, two runs
function runClip(fps, nFrames, o) { const sim = mk(o); const hs = []; for (let i = 0; i < nFrames; i++) { const out = sim.frame(1 / fps, clip.poseAt((i / fps) % clip.duration)); hs.push(stateHash(sim) + ':' + out.pose.reduce((a, b) => a + b.t[0] + b.t[1] + b.t[2] + b.q[0] + b.q[3], 0)); } return { sim, hs }; }
{ const a = runClip(60, 240, {}), b = runClip(60, 240, {}); let same = a.hs.every((h, i) => h === b.hs[i]); console.log(`\n(c) determinism: IdleN clip, 240 frames at 60 fps, two runs (same seed): ${same ? 'bit-identical particle states and output poses on every frame' : 'DIFFERENT'}`); }

// (d) dt handling: same pose per EVALUATION index, frames at 30 fps vs 60 fps vs 120 fps vs 144 fps
function runEval(fps, nEval, poseOf, o) { const sim = mk(o); const hs = []; let k = 0, frames = 0; const info = []; while (hs.length < nEval) { const out = sim.frame(1 / fps, clone(poseOf(k))); frames++; if (out.didEval) { hs.push(stateHash(sim)); info.push(out.evalDt); k++; } if (frames > 100000) throw new Error('no evaluation'); } return { hs, frames, info, sim }; }
{
  const poseOf = k => clip.poseAt((k / 30) % clip.duration);
  const base = runEval(30, 120, poseOf, { zeroJitter: true });
  for (const fps of [60, 120, 144, 45]) { const r = runEval(fps, 120, poseOf, { zeroJitter: true }); const firstDiff = r.hs.findIndex((h, i) => h !== base.hs[i]); console.log(`(d) ${fps} fps vs 30 fps, same animated pose per evaluation index (IdleN sampled at k/30 s): ${firstDiff < 0 ? 'all 120 evaluation states bit-identical' : 'first difference at evaluation ' + firstDiff}; frames used ${r.frames} (30 fps: ${base.frames}); evalDt of evaluations 0..3: ${r.info.slice(0, 4).map(x => x.toFixed(5)).join(', ')}; solver step state: step ${r.sim.core.f3830.toFixed(6)} bucket ${r.sim.core.f3824}`); }
  const r20 = runEval(20, 120, poseOf, { zeroJitter: true }); const fd = r20.hs.findIndex((h, i) => h !== base.hs[i]);
  console.log(`(d) 20 fps vs 30 fps (below the 30 Hz interval: evaluated every frame with evalDt = 1/20): ${fd < 0 ? 'identical' : 'states differ from evaluation ' + fd + ' (expected: bucket/step change ' + r20.sim.core.f3830.toFixed(6) + ', bucket ' + r20.sim.core.f3824 + ')'}`);
  // evaluation times in a real-time sense: 60 fps evaluates on frames 0,1,3,5,...
  const sim = mk({ zeroJitter: true }); const ev = []; for (let i = 0; i < 12; i++) { const o = sim.frame(1 / 60, clone(idle0)); ev.push(`${i}:${o.didEval ? 'E' : '-'} a=${o.alpha.toFixed(3)}`); } console.log('    60 fps frame pattern (E = evaluation, a = eased interpolation alpha): ' + ev.join('  '));
}
