#!/usr/bin/env node
// (HEROLIGHT-KB final pass, 2026-11-06) THE HERO LIGHT'S WEAPON COPIES, LOCKED ON THE WEAPON. The user: "do a final pass on every
// keyblade's hero light, all of them have graphical errors which need to be fixed" (their shots: the glow copy of the
// Keyblade floating detached beside him, shifted along / away from the blade, not matching its shape).
//
//   node tools/sora_herolight_copies.js           write sheets/port/sora-herolight-copies.json
//   node tools/sora_herolight_copies.js --check   exit 1 when the sheet differs from what this tool makes
//
// One row per glow emitter of sheets/port/sora-herolight.json that is a COPY OF THE WEAPON:
// - kind `mesh`: a mesh particle whose mesh (`typedata.mesh_file`, the pack's glb) is the weapon's silhouette in the weapon's
//   own MODEL space (its bind pose: +X grip -> tip, the same frame as the weapon's `weapon.glb`), turned by the emitter's
//   constant `RollPitchYawRange` (Ever After's and Wheel of Fate's copies are stored a quarter turn about X: Roll -90);
// - kind `skel`: a TypeDataSQEX_SkeletalMesh component of the weapon's own skeletal mesh (`<model>_0`, the form weapons).
// The renderer draws every such copy ON THE WEAPON AS DRAWN that frame (kh-er-render herolight.rs `lock`): the weapon's model
// frame (component x pose(wep) x bind(wep)^-1) x the turn, whatever the particle system made of its place (a socket's frame,
// a spawn-time world place, a mid-animation size scale).
// `parents`: the emitters that generate the copy (SQEX_Parent), kept in the glow copy with it (without them it is never made).
// `hold_min`: the earliest held moment that shows the copy (its timeline start + its first full brightness): the glow's hold is
// raised to it.
// `check_cm`: the copy's extents after the turn against the weapon's own (+X tip, both Y, both Z; the -X end is the keychain,
// which no copy has), in cm: every mesh copy must be within 1 cm, or the tool fails.
'use strict';
const fs = require('fs');
const path = require('path');

const VERSION = '1.0.0';
const R = path.resolve(__dirname, '..');
const PACK = path.join(R, 'build/sora_pack');
const MAX_CM = 1.0;

function bounds(file) {
  const b = fs.readFileSync(file); const len = b.readUInt32LE(12); const j = JSON.parse(b.slice(20, 20 + len).toString());
  const mn = [Infinity, Infinity, Infinity], mx = [-Infinity, -Infinity, -Infinity];
  for (const m of j.meshes || []) for (const p of m.primitives) { const a = j.accessors[p.attributes.POSITION]; if (a && a.min) for (let i = 0; i < 3; i++) { mn[i] = Math.min(mn[i], a.min[i] * 100); mx[i] = Math.max(mx[i], a.max[i] * 100); } }
  // in the effects' MESH SPACE (UE cm): the renderer reads a glb's (x, y, z) as (x, z, y) (kh-er-render vfx.rs), and the turn is
  // applied there; the fit must be made in the same space (a quarter turn about X flips its sign across the swap)
  return { mn: [mn[0], mn[2], mn[1]], mx: [mx[0], mx[2], mx[1]] };
}
// UE's FRotator (Pitch about Y, Yaw about Z, Roll about X; applied roll, then pitch, then yaw) on a point
function turn(p, [roll, pitch, yaw]) {
  const r = (d) => (d * Math.PI) / 180;
  let [x, y, z] = p;
  let c = Math.cos(r(roll)), s = Math.sin(r(roll)); [y, z] = [y * c - z * s, y * s + z * c];
  c = Math.cos(r(pitch)); s = Math.sin(r(pitch)); [x, z] = [x * c + z * s, -x * s + z * c];
  c = Math.cos(r(yaw)); s = Math.sin(r(yaw)); [x, y] = [x * c - y * s, x * s + y * c];
  return [x, y, z];
}
function turnedBounds(b, rpy) {
  const mn = [Infinity, Infinity, Infinity], mx = [-Infinity, -Infinity, -Infinity];
  for (const x of [b.mn[0], b.mx[0]]) for (const y of [b.mn[1], b.mx[1]]) for (const z of [b.mn[2], b.mx[2]]) {
    const q = turn([x, y, z], rpy); for (let i = 0; i < 3; i++) { mn[i] = Math.min(mn[i], q[i]); mx[i] = Math.max(mx[i], q[i]); }
  }
  return { mn, mx };
}
// how far a copy's extents (after its turn) are from the weapon's: the tip (+X), and per side axis the nearer of its two ends or
// its centre (a glow shell is a little thicker than the blade: Shooting Star's by 1.4 cm a side; the Storm Flag's copy has no flag)
function placement(c, w) {
  const axis = (i) => Math.min(Math.abs(c.mn[i] - w.mn[i]), Math.abs(c.mx[i] - w.mx[i]), Math.abs((c.mn[i] + c.mx[i]) / 2 - (w.mn[i] + w.mx[i]) / 2));
  return Math.max(Math.abs(c.mx[0] - w.mx[0]), axis(1), axis(2));
}
// THE EARLIEST HOLD THAT SHOWS THE COPY: its emitter starts at its Square Enix timeline's `start_time` (`required.sqex_emitter_time`;
// the stock `delay` only for a legacy timeline: kh-vfx def.rs), then its alpha first reaches 95 % of its peak (on its own lifetime,
// or a particle without a Lifetime module on the emitter's `duration_time`). sheets/port/sora-herolight's `hold` took the stock
// delay (0) for Grand Chef's / Ultima Weapon's copies, which start at 0.2 s: held at 0.15 s they were never made.
function holdMin(e) {
  const r = e.required || {}; const t = r.sqex_emitter_time;
  const legacy = !t || t.use_legacy_emitter_time === true;
  const start = legacy ? Number(r.delay) || 0 : Number(t.start_time) || 0;
  const mod = (c) => (e.modules || []).find((m) => m.class === c);
  const ev = (d) => { const x = d && (d.distribution || d); if (!x) return null; let v = x.evaluated && Array.isArray(x.evaluated.value) ? x.evaluated.value : x.constant !== undefined ? [].concat(x.constant) : null; if (!v || !v.length) return null; return Array.isArray(v[0]) ? v : [v]; };
  const life = (() => { const l = mod('ParticleModuleLifetime'); const v = l && ev(l.params.Lifetime); return v ? Math.max(...v.map((x) => Math.max(...x))) : 0; })() || (legacy ? Number(r.duration) || 0 : Number(t.duration_time) || Number(r.duration) || 0);
  const cs = mod('ParticleModuleColorScaleOverLife'); const alpha = (cs && ev(cs.params.AlphaScaleOverLife)) || [[1]];
  const top = Math.max(...alpha.map((x) => x[0])); let i = alpha.findIndex((x) => x[0] >= 0.95 * top); if (i < 0) i = 0;
  const age = alpha.length > 1 ? (i / (alpha.length - 1)) * life : 0;
  return Math.round((start + Math.max(age, 1 / 30)) * 1e4) / 1e4;
}
function constantVector(dist) {
  const d = dist && (dist.distribution || dist);
  if (!d) return [0, 0, 0];
  if (Array.isArray(d.constant)) return d.constant.map(Number);
  if (Array.isArray(d.min) && Array.isArray(d.max) && JSON.stringify(d.min) === JSON.stringify(d.max)) return d.min.map(Number);
  if (d.evaluated && Array.isArray(d.evaluated.min) && JSON.stringify(d.evaluated.min) === JSON.stringify(d.evaluated.max)) return d.evaluated.min.map(Number);
  if (d.evaluated && Array.isArray(d.evaluated.value) && !Array.isArray(d.evaluated.value[0])) return d.evaluated.value.map(Number);
  throw new Error('not a constant rotation: ' + JSON.stringify(d).slice(0, 200));
}

const sheet = JSON.parse(fs.readFileSync(path.join(R, 'sheets/port/sora-herolight.json'), 'utf8'));
const col = (n) => sheet.columns.indexOf(n);
const columns = ['id', 'model', 'effect', 'emitter', 'kind', 'mesh', 'turn', 'check_cm', 'parents', 'hold_min', 'source', 'ref_conf', 'note'];
const rows = [];
const bad = [];
for (const r of sheet.rows) {
  const model = r[col('model')], root = r[col('root')], effect = r[col('effect')];
  const fx = JSON.parse(fs.readFileSync(path.join(PACK, root, 'effects', effect + '.json'), 'utf8'));
  const weaponGlb = r[col('owner')] === 'sora' ? path.join(PACK, 'keyblades', model, 'weapon.glb') : path.join(PACK, 'party/weapons', model, 'weapon.glb');
  // the emitters that GENERATE another (SQEX_Parent ChildrenData): a copy generated by a parent (Shooting Star's Key0 by "+*",
  // Favorite Deputy's Key by "+*") is never made in a glow copy without its parent: the chain goes into the glow too
  const parentsOf = new Map();
  for (const x of fx.emitters) { const p = (x.modules || []).find((m) => m.class === 'ParticleModuleSQEX_Parent'); for (const c of (p && p.params && p.params.ChildrenData) || []) { if (!parentsOf.has(c.GenerateEmitterName)) parentsOf.set(c.GenerateEmitterName, []); parentsOf.get(c.GenerateEmitterName).push(x.name); } }
  const chain = (name) => { const out = []; const todo = [...(parentsOf.get(name) || [])]; while (todo.length) { const p = todo.shift(); if (!out.includes(p)) { out.push(p); todo.push(...(parentsOf.get(p) || [])); } } return out; };
  for (const name of r[col('glow')]) {
    const e = fx.emitters.find((x) => x.name === name);
    const kind = e && e.typedata && e.typedata.kind;
    if (kind === 'mesh') {
      const authored = constantVector(e.typedata.params && e.typedata.params.RollPitchYawRange);
      const mesh = path.basename(e.typedata.mesh_file, '.glb');
      const cb = bounds(path.join(PACK, root, e.typedata.mesh_file));
      const w = bounds(weaponGlb);
      // the turn that puts the copy on the weapon: the emitter's own RollPitchYawRange, else the axis turn of the 64 that fits
      // the weapon best (Wheel of Fate's copy carries Roll -90 like Ever After's, but its glb is stored already turned: only
      // the fit tells them apart)
      const turns = [authored];
      for (const rr of [0, 90, 180, -90]) for (const pp of [0, 90, -90, 180]) for (const yy of [0, 90, 180, -90]) turns.push([rr, pp, yy]);
      const fit = turns.map((rpy) => ({ rpy, err: placement(turnedBounds(cb, rpy), w) })).reduce((a, b) => (b.err < a.err - 1e-6 ? b : a));
      if (fit.err > MAX_CM) bad.push(model + ' ' + name + ' ' + fit.err.toFixed(2) + ' cm');
      const same = JSON.stringify(fit.rpy) === JSON.stringify(authored);
      rows.push(['hlc.' + model + '.' + name, model, effect, name, 'mesh', mesh.toLowerCase(), fit.rpy, Math.round(fit.err * 100) / 100, chain(name), holdMin(e),
        root + '/effects/' + effect + '.json emitter ' + name + ' (typedata.mesh_file, RollPitchYawRange ' + JSON.stringify(authored) + '); ' + path.relative(R, weaponGlb).replace(/\\/g, '/'), same ? 'confirmed' : 'inferred',
        same ? '-' : 'the authored turn ' + JSON.stringify(authored) + ' puts it ' + placement(turnedBounds(cb, authored), w).toFixed(1) + ' cm off; the fitted ' + JSON.stringify(fit.rpy) + ' puts it on the weapon']);
    } else if (kind === 'sqex_skeletalmesh') {
      const mesh = String(e.typedata.params.SkeletalMesh).split('/').pop().split('.')[0];
      rows.push(['hlc.' + model + '.' + name, model, effect, name, 'skel', mesh.toLowerCase(), [0, 0, 0], 0, chain(name), holdMin(e),
        root + '/effects/' + effect + '.json emitter ' + name + ' (TypeDataSQEX_SkeletalMesh SkeletalMesh)', 'confirmed', mesh.toLowerCase() === model + '_0' ? '-' : 'the copy is the twin weapon ' + mesh + ' (KH3 data)']);
    }
  }
}
if (bad.length) throw new Error('sora-herolight-copies: copies off their weapon by more than ' + MAX_CM + ' cm: ' + bad.join('; '));
const out = JSON.stringify({ sheet: 'sora-herolight-copies', group: 'port', columns }).replace(/\}$/, ',"rows":[\n') + rows.map((x) => JSON.stringify(x)).join(',\n') + '\n]}\n';
const file = path.join(R, 'sheets/port/sora-herolight-copies.json');
if (process.argv.includes('--check')) {
  const same = fs.existsSync(file) && fs.readFileSync(file, 'utf8') === out;
  console.log(same ? 'sora-herolight-copies: ' + rows.length + ' rows, current' : 'sora-herolight-copies: sheet differs from tools/sora_herolight_copies.js ' + VERSION);
  process.exit(same ? 0 : 1);
}
fs.writeFileSync(file, out);
console.log('sora-herolight-copies: ' + rows.length + ' rows written (tools/sora_herolight_copies.js ' + VERSION + ')');
for (const x of rows) console.log('  ' + x[1].padEnd(8) + ' ' + x[3].padEnd(10) + ' ' + x[4] + ' ' + x[5].padEnd(22) + ' turn ' + JSON.stringify(x[6]) + ' ' + x[7] + ' cm');
