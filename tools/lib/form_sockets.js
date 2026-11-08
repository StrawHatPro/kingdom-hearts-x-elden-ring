// (weapons F-LIGHTDARK) THE HIT SOCKETS OF A FORM CLIP: where a weapon's ATK_00 / ATK_01 are, per animation frame, in
// ACTOR space (UE cm; x forward, y right, z up), for clips of the form anim sets (build/sora_pack/keyblades/sora_forms/
// anims/*.glb) that build/sora_pack/manifest.json `attack_sockets` does not carry. The same rule as tools/sora_pack.js
// `attackSockets` (bone -> socket on Sora's skeleton -> the weapon's root bone and bone `wep` at the weapon's reference
// pose -> the socket; Sora's root bone at its reference transform when the clip has root motion), read from the clip's
// glb instead of the uasset: the glb nodes carry the skeleton's reference pose and the clip's channels in the pack
// conventions, converted back to UE here ((x, y, z)_pack m -> (x, z, y) * 100 cm; quaternion (x, y, z, w)_pack ->
// (-x, -z, -y, w)). Checked against the main pack's own tracks by tools/sora_form_lightdark.js (`--check`).
//
//   const { socketTrack } = require('./lib/form_sockets');
//   socketTrack(glbPath, { bone: 'R_buki', socketRotDeg: { pitch, yaw, roll }, atk00, atk01, weaponBones, rootMotion, meshYawDeg })
'use strict';
const fs = require('fs');

function readGlb(file) {
  const b = fs.readFileSync(file);
  if (b.readUInt32LE(0) !== 0x46546c67) throw new Error(file + ': not a glb');
  const jl = b.readUInt32LE(12);
  const json = JSON.parse(b.slice(20, 20 + jl).toString('utf8'));
  const binStart = 20 + jl + 8;
  const binLen = b.readUInt32LE(20 + jl);
  return { json, bin: b.slice(binStart, binStart + binLen) };
}

function accessor(g, i) {
  const a = g.json.accessors[i];
  const v = g.json.bufferViews[a.bufferView];
  const n = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4 }[a.type];
  if (a.componentType !== 5126) throw new Error('accessor ' + i + ': not float');
  const off = (v.byteOffset || 0) + (a.byteOffset || 0);
  const stride = v.byteStride || n * 4;
  const out = new Float64Array(a.count * n);
  for (let k = 0; k < a.count; k++) for (let c = 0; c < n; c++) out[k * n + c] = g.bin.readFloatLE(off + k * stride + c * 4);
  return out;
}

// pack -> UE
const ueT = (t) => [t[0] * 100, t[2] * 100, t[1] * 100];
const ueQ = (q) => [-q[0], -q[2], -q[1], q[3]];
const ueS = (s) => [s[0], s[2], s[1]];

const qmulH = (a, b) => [a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1], a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0], a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3], a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]];
const qrot = (q, v) => { const tx = 2 * (q[1] * v[2] - q[2] * v[1]), ty = 2 * (q[2] * v[0] - q[0] * v[2]), tz = 2 * (q[0] * v[1] - q[1] * v[0]); return [v[0] + q[3] * tx + (q[1] * tz - q[2] * ty), v[1] + q[3] * ty + (q[2] * tx - q[0] * tz), v[2] + q[3] * tz + (q[0] * ty - q[1] * tx)]; };
// child (q, t, s) under parent (q, t, s): UE FTransform product child * parent
const under = (c, p) => { const v = qrot(p.q, [p.s[0] * c.t[0], p.s[1] * c.t[1], p.s[2] * c.t[2]]); return { q: qmulH(p.q, c.q), t: [v[0] + p.t[0], v[1] + p.t[1], v[2] + p.t[2]], s: [p.s[0] * c.s[0], p.s[1] * c.s[1], p.s[2] * c.s[2]] }; };
const point = (T, v) => { const r = qrot(T.q, [T.s[0] * v[0], T.s[1] * v[1], T.s[2] * v[2]]); return [r[0] + T.t[0], r[1] + T.t[1], r[2] + T.t[2]]; };
// FRotator -> quaternion (the stock UE formula)
function rotQuat(r) {
  const h = Math.PI / 360, SP = Math.sin(r.pitch * h), CP = Math.cos(r.pitch * h), SY = Math.sin(r.yaw * h), CY = Math.cos(r.yaw * h), SR = Math.sin(r.roll * h), CR = Math.cos(r.roll * h);
  return [CR * SP * SY - SR * CP * CY, -CR * SP * CY - SR * CP * SY, CR * CP * SY - SR * SP * CY, CR * CP * CY + SR * SP * SY];
}

function sample(times, vals, n, t) {
  let k = 0; while (k < times.length - 2 && times[k + 1] <= t) k++;
  const k2 = Math.min(k + 1, times.length - 1);
  const f = times.length > 1 && times[k2] > times[k] ? Math.min(1, Math.max(0, (t - times[k]) / (times[k2] - times[k]))) : 0;
  const v = []; for (let i = 0; i < n; i++) v.push(vals[k * n + i] * (1 - f) + vals[k2 * n + i] * f);
  if (n === 4) { const l = Math.hypot(...v); return v.map((x) => x / l); }
  return v;
}

/// The track of one weapon: frames x [ATK_00 xyz, ATK_01 xyz], actor space, cm.
/// o: { bone, socketRotDeg, socketLoc, atk00, atk01, weaponBones: [root {q,t,s}, wep {q,t,s}], rootMotion, meshYawDeg, frames, length }
function socketTrack(file, o) {
  const g = readGlb(file);
  const nodes = g.json.nodes;
  const parent = new Array(nodes.length).fill(-1);
  nodes.forEach((n, i) => (n.children || []).forEach((c) => { parent[c] = i; }));
  const hand = nodes.findIndex((n) => n.name === o.bone);
  if (hand < 0) throw new Error(file + ': no node ' + o.bone);
  const chain = []; for (let k = hand; k >= 0; k = parent[k]) chain.unshift(k);
  const anim = g.json.animations[0];
  const chans = new Map();
  for (const c of anim.channels) {
    const s = anim.samplers[c.sampler];
    const times = accessor(g, s.input), vals = accessor(g, s.output);
    if (!chans.has(c.target.node)) chans.set(c.target.node, {});
    chans.get(c.target.node)[c.target.path] = { times, vals };
  }
  let length = 0; for (const m of chans.values()) for (const c of Object.values(m)) length = Math.max(length, c.times[c.times.length - 1]);
  if (o.length) length = o.length;
  const frames = o.frames || Math.round(length * 30) + 1;
  const socket = { q: rotQuat(o.socketRotDeg || { pitch: 0, yaw: 0, roll: 0 }), t: o.socketLoc || [0, 0, 0], s: [1, 1, 1] };
  const [wroot, wep] = o.weaponBones;
  const yaw = (o.meshYawDeg || 0) * Math.PI / 180, cy = Math.cos(yaw), sy = Math.sin(yaw);
  const actor = (v) => [v[0] * cy - v[1] * sy, v[0] * sy + v[1] * cy, v[2]];
  const out = [];
  for (let i = 0; i < frames; i++) {
    const time = frames > 1 ? i * length / (frames - 1) : 0;
    let T = null;
    for (const k of chain) {
      const n = nodes[k];
      const tr = (k === chain[0] && o.rootMotion) ? {} : (chans.get(k) || {});
      const t = tr.translation ? sample(tr.translation.times, tr.translation.vals, 3, time) : (n.translation || [0, 0, 0]);
      const q = tr.rotation ? sample(tr.rotation.times, tr.rotation.vals, 4, time) : (n.rotation || [0, 0, 0, 1]);
      const s = tr.scale ? sample(tr.scale.times, tr.scale.vals, 3, time) : (n.scale || [1, 1, 1]);
      const l = { q: ueQ(q), t: ueT(t), s: ueS(s) };
      T = T ? under(l, T) : l;
    }
    const W = under(wep, under(wroot, under(socket, T)));
    out.push([...actor(point(W, o.atk00)), ...actor(point(W, o.atk01))].map((x) => +x.toFixed(3)));
  }
  return { frames, length, sockets: out };
}

module.exports = { socketTrack, readGlb };
