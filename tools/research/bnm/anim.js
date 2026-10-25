// Animated component-space poses for the Bonamik reference evaluator.
// Skeleton: build/sora_pack/manifest.json -> skeleton.bones (*_ue fields = UE local reference pose, cm).
// Clips: build/sora_pack/anims/<clip>.glb (pack convention; converted back to UE here: pos (x,z,y)*100, quat (-x,-z,-y,w)).
const path = require('path'); const fs = require('fs');
const man = require('../../sora_pack/manifest.json'); const bones = man.skeleton.bones;
const f = Math.fround;
function readGlb(file) { const b = fs.readFileSync(file); const jl = b.readUInt32LE(12); const json = JSON.parse(b.toString('utf8', 20, 20 + jl)); const bo = 20 + jl; const bl = bo < b.length ? b.readUInt32LE(bo) : 0; return { json, bin: b.subarray(bo + 8, bo + 8 + bl) }; }
const qmul = (a, b) => [a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1], a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0], a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3], a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]];
const qrot = (q, v) => { const tx = 2 * (q[1] * v[2] - q[2] * v[1]), ty = 2 * (q[2] * v[0] - q[0] * v[2]), tz = 2 * (q[0] * v[1] - q[1] * v[0]); return [v[0] + q[3] * tx + (q[1] * tz - q[2] * ty), v[1] + q[3] * ty + (q[2] * tx - q[0] * tz), v[2] + q[3] * tz + (q[0] * ty - q[1] * tx)]; };
const qnorm = q => { const l = Math.hypot(...q); return q.map(x => x / l); };
// local {q,t,s}[] -> component space {q,t,s}[] (float32-rounded, FTransform multiply semantics child * parent)
function componentSpace(local) {
  const cs = [];
  for (let i = 0; i < bones.length; i++) {
    const l = local[i], pi = bones[i].parent; let o;
    if (pi < 0) o = { q: l.q.slice(), t: l.t.slice(), s: l.s.slice() };
    else { const P = cs[pi]; const rt = qrot(P.q, [P.s[0] * l.t[0], P.s[1] * l.t[1], P.s[2] * l.t[2]]); o = { q: qnorm(qmul(P.q, l.q)), t: [rt[0] + P.t[0], rt[1] + P.t[1], rt[2] + P.t[2]], s: [l.s[0] * P.s[0], l.s[1] * P.s[1], l.s[2] * P.s[2]] }; }
    cs.push(o);
  }
  return cs.map(x => ({ q: x.q.map(f), t: x.t.map(f), s: x.s.map(f) }));
}
const refLocal = () => bones.map(b => ({ q: b.rotation_ue.slice(), t: b.translation_ue.slice(), s: b.scale_ue.slice() }));
const refPose = () => componentSpace(refLocal());
function loadClip(name) {
  const g = readGlb(path.join(__dirname, '../../sora_pack/anims', name));
  const acc = i => { const a = g.json.accessors[i], bv = g.json.bufferViews[a.bufferView]; const n = { SCALAR: 1, VEC3: 3, VEC4: 4 }[a.type]; const off = (bv.byteOffset || 0) + (a.byteOffset || 0); const out = []; for (let k = 0; k < a.count; k++) { const r = []; for (let c = 0; c < n; c++) r.push(g.bin.readFloatLE(off + (k * n + c) * 4)); out.push(r); } return out; };
  const an = g.json.animations[0]; const tracks = {}; let tmax = 0;
  for (const c of an.channels) { const s = an.samplers[c.sampler]; const t = acc(s.input).map(x => x[0]); const v = acc(s.output); tmax = Math.max(tmax, t[t.length - 1]); (tracks[c.target.node] = tracks[c.target.node] || {})[c.target.path] = { t, v }; }
  const lerp = (a, b, u) => a.map((x, i) => x + (b[i] - x) * u);
  const sample = (tr, time, isQuat) => { const { t, v } = tr; if (time <= t[0]) return v[0]; if (time >= t[t.length - 1]) return v[v.length - 1]; let i = 0; while (t[i + 1] < time) i++; const u = (time - t[i]) / (t[i + 1] - t[i]); let b = v[i + 1]; if (isQuat) { const d = v[i][0] * b[0] + v[i][1] * b[1] + v[i][2] * b[2] + v[i][3] * b[3]; if (d < 0) b = b.map(x => -x); return qnorm(lerp(v[i], b, u)); } return lerp(v[i], b, u); };
  const posUE = p => [p[0] * 100, p[2] * 100, p[1] * 100], quatUE = q => [-q[0], -q[2], -q[1], q[3]], sclUE = s => [s[0], s[2], s[1]];
  const localAt = time => bones.map((b, i) => { const tr = tracks[i] || {}; return { q: tr.rotation ? quatUE(sample(tr.rotation, time, true)) : b.rotation_ue.slice(), t: tr.translation ? posUE(sample(tr.translation, time)) : b.translation_ue.slice(), s: tr.scale ? sclUE(sample(tr.scale, time)) : b.scale_ue.slice() }; });
  return { duration: tmax, poseAt: time => componentSpace(localAt(time)), localAt, trackedBones: Object.keys(tracks).length };
}
module.exports = { bones, componentSpace, refLocal, refPose, loadClip };
if (require.main === module) { const c = loadClip(process.argv[2] || '0001.glb'); const p = c.poseAt(0); const r = refPose(); const i = bones.findIndex(b => b.name === 'atama'); console.log('duration', c.duration, 'tracked', c.trackedBones, 'atama ref', r[i].t, 'clip t0', p[i].t); }
