#!/usr/bin/env node
// tools/sora_outfit_legguard.js (ER79; the user, 2026-10-15: "I just want his pants to never clip into his leg")
// THE OUTFITS' LEG GUARD: <out>/outfits/<id>/legguard.json (NEW files; build/sora_pack is only READ). For each outfit:
//   - its LEG: the vertices skinned (>= 95 %) to the thigh / shin bones and their helpers, with no weight on a simulated
//     (Bonamik) bone: tapered capsules fitted around them in the bind pose (thigh = <side>_momo -> <side>_sune, shin =
//     <side>_sune -> <side>_ashi1), so that every leg vertex lies inside its capsule (+ MARGIN);
//   - its CLOTH: the vertices with weight on a simulated bone of the lower body (a descendant of koshi): shorts, skirt
//     pieces, coat tails;
//   - whether the cloth goes INTO the leg: offline, the harness (crates/kh-er-render/examples/effects.rs --stretch
//     --legguard) counts cloth vertices inside the capsules frame by frame.
// The renderer (crates/kh-er-render outfit.rs, gfx.rs) reads it when an outfit is put on:
//   (1) bone level: every simulated lower-body bone of `bones` is pushed out of the capsules after the outfit's Bonamik
//       step (the anticlip rule format, component space, cm);
//   (2) vertex level: the cloth joints (`cloth_joints`) and the capsules go to the skin shader (er_char.wgsl
//       KH_CLOTH_GUARD), which puts any vertex with cloth weight that ends inside a capsule on its surface (+ margin).
//       Only cloth vertices move; the leg is never touched.
// `enabled` per outfit: on where the cloth was measured going into the legs (see ENABLE below).
//
//   node tools/sora_outfit_legguard.js --out build/sora_tmp/er/er79/pack [--only kh1]
'use strict';
const fs = require('fs'), path = require('path');
const ROOT = path.join(__dirname, '..');
const PACK = path.join(ROOT, 'build/sora_pack');
const args = process.argv.slice(2);
const opt = (k) => { const i = args.indexOf(k); return i >= 0 ? args[i + 1] : null; };
const OUT = opt('--out');
if (!OUT) { console.error('--out DIR is required (never build/sora_pack)'); process.exit(2); }
if (path.resolve(OUT) === path.resolve(PACK)) { console.error('refusing to write into build/sora_pack'); process.exit(2); }
const only = opt('--only') ? opt('--only').split(',') : null;
const MARGIN_CM = 0.5;
// measured with the harness (IMPL_ER79_STATE.md): the outfits whose cloth goes into the legs (kh1: its shorts, up to 6.2
// cm deep; caribbean: its coat, up to 10.3 cm); the others have no lower-body cloth (kh2, toybox, sanfransokyo,
// verumrex) or it never reached the legs (monstropolis' tail, kept off them by its anticlip.json)
const ENABLE = new Set((opt('--enable') || 'kh1,caribbean').split(',').filter(Boolean));

function readGlb(p) {
  const b = fs.readFileSync(p); const jl = b.readUInt32LE(12); const j = JSON.parse(b.slice(20, 20 + jl).toString()); const bin = b.slice(20 + jl + 8);
  const acc = (i) => {
    const a = j.accessors[i], bv = j.bufferViews[a.bufferView]; const n = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4, MAT4: 16 }[a.type];
    const ct = a.componentType, size = { 5126: 4, 5123: 2, 5121: 1, 5125: 4 }[ct]; const stride = bv.byteStride || n * size; const off = (bv.byteOffset || 0) + (a.byteOffset || 0);
    const out = new Array(a.count);
    for (let k = 0; k < a.count; k++) { const v = []; for (let c = 0; c < n; c++) { const o = off + k * stride + c * size; v.push(ct === 5126 ? bin.readFloatLE(o) : ct === 5123 ? bin.readUInt16LE(o) : ct === 5121 ? bin.readUInt8(o) : bin.readUInt32LE(o)); } out[k] = n === 1 ? v[0] : v; }
    return out;
  };
  const skin = j.skins[0];
  return { joints: skin.joints.map((n) => j.nodes[n].name), ibm: acc(skin.inverseBindMatrices), prims: j.meshes[0].primitives.map((p) => ({ material: j.materials[p.material].name, pos: acc(p.attributes.POSITION), jo: acc(p.attributes.JOINTS_0), we: acc(p.attributes.WEIGHTS_0) })) };
}
// the translation of the inverse of an affine column-major 4x4 (the joint's bind position)
function bindPos(m) {
  const r = [[m[0], m[4], m[8]], [m[1], m[5], m[9]], [m[2], m[6], m[10]]], t = [m[12], m[13], m[14]];
  const det = r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1]) - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0]) + r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0]);
  const inv = [
    [(r[1][1] * r[2][2] - r[1][2] * r[2][1]) / det, (r[0][2] * r[2][1] - r[0][1] * r[2][2]) / det, (r[0][1] * r[1][2] - r[0][2] * r[1][1]) / det],
    [(r[1][2] * r[2][0] - r[1][0] * r[2][2]) / det, (r[0][0] * r[2][2] - r[0][2] * r[2][0]) / det, (r[0][2] * r[1][0] - r[0][0] * r[1][2]) / det],
    [(r[1][0] * r[2][1] - r[1][1] * r[2][0]) / det, (r[0][1] * r[2][0] - r[0][0] * r[2][1]) / det, (r[0][0] * r[1][1] - r[0][1] * r[1][0]) / det]];
  return [0, 1, 2].map((i) => -(inv[i][0] * t[0] + inv[i][1] * t[1] + inv[i][2] * t[2]));
}
const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]], dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2], len = (a) => Math.sqrt(dot(a, a));
// t along a -> b and the distance to the segment's line
function axial(p, a, b) { const d = sub(b, a), l2 = dot(d, d); const t = dot(sub(p, a), d) / l2; const q = [a[0] + d[0] * t, a[1] + d[1] * t, a[2] + d[2] * t]; return { t, d: len(sub(p, q)), segLen: Math.sqrt(l2) }; }

const index = JSON.parse(fs.readFileSync(path.join(PACK, 'outfits/outfits.json'), 'utf8'));
const report = [];
for (const o of index.outfits) {
  if (!o.dir || (only && !only.includes(o.id))) continue;
  const dir = path.join(PACK, o.dir);
  const manifest = JSON.parse(fs.readFileSync(path.join(dir, 'manifest.json'), 'utf8'));
  const bones = manifest.skeleton.bones; const byName = new Map(bones.map((b, i) => [b.name, i]));
  const parentName = (n) => { const b = bones[byName.get(n)]; return b && typeof b.parent === 'number' && b.parent >= 0 ? bones[b.parent].name : null; };
  const under = (n, anc) => { for (let x = n; x; x = parentName(x)) if (x === anc) return true; return false; };
  const bnmFile = path.join(dir, 'bonamik.json');
  const simulated = new Set(fs.existsSync(bnmFile) ? JSON.parse(fs.readFileSync(bnmFile, 'utf8')).bones.filter((b) => b.bonamik === 'simulated_node').map((b) => b.name) : []);
  // lower-body cloth: simulated bones under koshi (not the hips' upper body)
  const cloth = new Set([...simulated].filter((n) => under(n, 'koshi') && !/chain/i.test(n)));
  const modelFile = fs.existsSync(path.join(dir, 'model_kd.glb')) ? path.join(dir, 'model_kd.glb') : path.join(dir, 'model.glb');
  const g = readGlb(modelFile);
  const jpos = g.ibm.map(bindPos);
  const jIndex = new Map(g.joints.map((n, i) => [n, i]));
  const legBone = (n) => /^(L|R)_(momo|sune|hiza|mata)/.test(n) && !cloth.has(n);
  // the materials that carry cloth (a shorts' waist band skinned to the thigh is cloth, not leg)
  const clothMaterials = new Set(g.prims.filter((p) => p.jo.some((jj, k) => jj.some((x, c) => p.we[k][c] > 0 && cloth.has(g.joints[x])))).map((p) => p.material));
  const capsules = [];
  for (const side of ['L', 'R']) {
    for (const [a, b, seg] of [[`${side}_momo`, `${side}_sune`, 'momo'], [`${side}_sune`, `${side}_ashi1`, 'sune']]) {
      if (!jIndex.has(a) || !jIndex.has(b)) continue;
      const pa = jpos[jIndex.get(a)], pb = jpos[jIndex.get(b)];
      const pts = [];
      for (const p of g.prims.filter((q) => !clothMaterials.has(q.material))) for (let k = 0; k < p.pos.length; k++) {
        let leg = 0, clothW = 0, segW = 0;
        for (let c = 0; c < 4; c++) { const n = g.joints[p.jo[k][c]], w = p.we[k][c]; if (!w) continue; if (cloth.has(n)) clothW += w; if (legBone(n)) leg += w; if (n.startsWith(`${side}_${seg}`)) segW += w; }
        if (leg >= 0.95 && clothW === 0 && segW >= 0.5) { const ax = axial(p.pos[k], pa, pb); if (ax.t > -0.05 && ax.t < 1.05) pts.push(ax); }
      }
      if (pts.length < 20) continue;
      // TWO tapered pieces per segment (a calf bulges: one straight taper would be ~3 cm too wide at the knee), each the
      // tightest linear envelope over its span: r(u) = r0 + (r1 - r0) u holds every point of it, r0 + r1 least
      const segLen = pts[0].segLen;
      const t0 = Math.max(0, Math.min(...pts.map((x) => x.t))), t1 = Math.min(1, Math.max(...pts.map((x) => x.t)));
      const tm = (t0 + t1) / 2;
      for (const [lo, hi] of [[t0, tm], [tm, t1]]) {
        const part = pts.filter((x) => x.t >= lo - 0.02 && x.t <= hi + 0.02);
        if (!part.length) continue;
        const us = part.map((x) => ({ u: Math.min(1, Math.max(0, (x.t - lo) / Math.max(1e-6, hi - lo))), d: x.d }));
        const dmax = Math.max(...us.map((x) => x.d));
        let best = null;
        for (let k = 0; k <= 400; k++) {
          const r1 = (dmax * 2 * k) / 400;
          if (us.some((x) => x.u > 0.999 && x.d > r1)) continue;
          const r0 = Math.max(0, ...us.filter((x) => x.u <= 0.999).map((x) => (x.d - r1 * x.u) / (1 - x.u)));
          if (!best || r0 + r1 < best[0] + best[1]) best = [r0, r1];
        }
        const [r0, r1] = best;
        capsules.push({ a, b, t0: +lo.toFixed(3), t1: +hi.toFixed(3), r0: +(r0 * 100 + MARGIN_CM).toFixed(2), r1: +(r1 * 100 + MARGIN_CM).toFixed(2), leg_vertices: part.length, length_cm: +(segLen * 100).toFixed(1) });
      }
    }
  }
  // cloth vertices inside a capsule in the bind pose (the guard puts them on its surface: a few lining vertices)
  let clothVerts = 0;
  for (const c of capsules) {
    const pa = jpos[jIndex.get(c.a)], pb = jpos[jIndex.get(c.b)];
    for (;;) {
      let inside = 0;
      for (const p of g.prims) for (let k = 0; k < p.pos.length; k++) {
        let clothW = 0; for (let q = 0; q < 4; q++) if (p.we[k][q] && cloth.has(g.joints[p.jo[k][q]])) clothW += p.we[k][q];
        if (clothW <= 0) continue;
        const ax = axial(p.pos[k], pa, pb); if (ax.t < c.t0 || ax.t > c.t1) continue;
        const r = (c.r0 + (c.r1 - c.r0) * ((ax.t - c.t0) / Math.max(1e-6, c.t1 - c.t0))) / 100;
        if (ax.d < r) inside++;
      }
      c.cloth_inside_at_bind = inside;
      break;
    }
  }
  for (const p of g.prims) for (let k = 0; k < p.pos.length; k++) { for (let q = 0; q < 4; q++) if (p.we[k][q] && cloth.has(g.joints[p.jo[k][q]])) { clothVerts++; break; } }
  const clothBones = [...cloth].filter((n) => byName.has(n));
  const enabled = ENABLE.has(o.id) && capsules.length > 0 && clothBones.length > 0;
  const out = {
    format: 'kh3-sora-outfit-legguard/1', generator: 'tools/sora_outfit_legguard.js', outfit: o.id, model: o.model, enabled,
    note: 'capsules (component space, cm; fraction t of bone a -> b, radius r0 at t0 to r1 at t1) around the leg mesh in the bind pose (+ ' + MARGIN_CM + ' cm); `bones`: the simulated lower-body bones pushed out of them after the Bonamik step; `cloth_joints`: the skin joints whose weight makes a vertex cloth (pushed out per vertex in the skin shader). Read by crates/kh-er-render outfit.rs / gfx.rs.',
    margin_cm: MARGIN_CM, model_file: path.basename(modelFile), cloth_vertices: clothVerts,
    capsules, bones: clothBones, cloth_joints: g.joints.filter((n) => cloth.has(n)),
  };
  const outDir = path.join(OUT, 'outfits', o.id); fs.mkdirSync(outDir, { recursive: true });
  fs.writeFileSync(path.join(outDir, 'legguard.json'), JSON.stringify(out, null, 1) + '\n');
  report.push(`${o.id}: ${enabled ? 'ON ' : 'off'} ${capsules.length} capsule(s) ${capsules.map((c) => `${c.a}->${c.b} t ${c.t0}-${c.t1} r ${c.r0}-${c.r1} (bind cloth inside ${c.cloth_inside_at_bind})`).join('; ')}; ${clothBones.length} cloth bones, ${clothVerts} cloth vertices`);
}
console.log(report.join('\n'));
