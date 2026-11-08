#!/usr/bin/env node
// (HEROLIGHT-KB, 2026-11-08) HOW KH3 KEEPS A GLOW COPY FROM Z-FIGHTING THE WEAPON IT COPIES. The user (DEPLOY4): "the keyblade hero
// lights are a little buggy now, the light overlay on the keyblade flickers with the actual keyblade model". The copies are drawn
// exactly on the weapon (sheets/port/sora-herolight-copies.json), so they share its depth; KH3 never draws them there unbiased:
// - every MESH copy carries `ParticleModuleSQEX_DepthOffset` (Kingdom Key 2.5 cm, Shooting Star / Hero's Origin 5, Oblivion /
//   Dead of Night's aura 10): its vertex stage slides it that far TOWARD THE EYE along the eye ray, its screen picture unchanged
//   (kh-vfx sim.rs `depth_pull`, KH3's mesh factory fork 0x1442F2650);
// - every SKELETAL copy's material (smt_wep_add_lgt_00a) has the CameraOffset material function, set per emitter by
//   `SQEX_Material_Scalar "Camera_Offset"` (2.5 on Counter Shield, the Arrowguns ...): the same pull toward the eye. A skeletal
//   copy without that scalar (Hyper Hammer, Drill Punch ...) inflates instead (`4.WPO_power` along its normals), which this
//   renderer's translated vertex stages do not do: it takes the 2.5 cm KH3 gives every other copy of its kind (inferred).
//
//   node tools/sora_herolight_depth.js           write sheets/port/sora-herolight-depth.json
//   node tools/sora_herolight_depth.js --check   exit 1 when the sheet differs
'use strict';
const fs = require('fs');
const path = require('path');

const VERSION = '1.0.0';
const R = path.resolve(__dirname, '..');
const PACK = path.join(R, 'build/sora_pack');
const FALLBACK_CM = 2.5;

const copies = JSON.parse(fs.readFileSync(path.join(R, 'sheets/port/sora-herolight-copies.json'), 'utf8'));
const col = (n) => copies.columns.indexOf(n);
const columns = ['id', 'model', 'emitter', 'kind', 'offset_cm', 'source', 'ref_conf', 'note'];
const rows = [];
const constant = (d) => { const x = d && (d.distribution || d); if (!x) return null; if (typeof x.constant === 'number') return x.constant; const v = x.evaluated && x.evaluated.value; if (Array.isArray(v) && v.length === 1) return Number([].concat(v[0])[0]); return null; };
for (const r of copies.rows) {
  const file = r[col('source')].split(' ')[0];
  const fx = JSON.parse(fs.readFileSync(path.join(PACK, file), 'utf8'));
  const e = fx.emitters.find((x) => x.name === r[col('emitter')]);
  const mods = (e && e.modules) || [];
  let offset = null, source = '', conf = 'confirmed', note = '-';
  if (r[col('kind')] === 'mesh') {
    const m = mods.find((x) => x.class === 'ParticleModuleSQEX_DepthOffset');
    if (m) { offset = Number(m.params.DepthOffset); source = file + ' emitter ' + e.name + ' SQEX_DepthOffset.DepthOffset'; }
  } else {
    const m = mods.filter((x) => /Material_Scalar/.test(x.class)).find((x) => JSON.stringify(x.params).includes('"Camera_Offset"'));
    const v = m && constant(Object.values(m.params).find((p) => p && typeof p === 'object' && (p.distribution || p.evaluated)));
    if (v !== null && v !== undefined) { offset = v; source = file + ' emitter ' + e.name + ' SQEX_Material_Scalar Camera_Offset (material smt_wep_add_lgt_00a: CameraOffset)'; }
  }
  if (offset === null || !(offset > 0)) {
    offset = FALLBACK_CM; conf = 'inferred'; source = file + ' emitter ' + (e ? e.name : r[col('emitter')]);
    note = 'no depth offset / Camera_Offset of its own (KH3 inflates it by 4.WPO_power instead, not done by this renderer): the ' + FALLBACK_CM + ' cm of every other copy of its kind';
  }
  rows.push(['hld.' + r[col('model')] + '.' + r[col('emitter')], r[col('model')], r[col('emitter')], r[col('kind')], offset, source, conf, note]);
}
const out = JSON.stringify({ sheet: 'sora-herolight-depth', group: 'port', columns }).replace(/\}$/, ',"rows":[\n') + rows.map((x) => JSON.stringify(x)).join(',\n') + '\n]}\n';
const file = path.join(R, 'sheets/port/sora-herolight-depth.json');
if (process.argv.includes('--check')) {
  const same = fs.existsSync(file) && fs.readFileSync(file, 'utf8') === out;
  console.log(same ? 'sora-herolight-depth: ' + rows.length + ' rows, current' : 'sora-herolight-depth: sheet differs from tools/sora_herolight_depth.js ' + VERSION);
  process.exit(same ? 0 : 1);
}
fs.writeFileSync(file, out);
console.log('sora-herolight-depth: ' + rows.length + ' rows written (tools/sora_herolight_depth.js ' + VERSION + ')');
for (const x of rows) console.log('  ' + x[1].padEnd(8) + ' ' + String(x[2]).padEnd(10) + ' ' + x[3] + ' ' + x[4] + ' cm ' + x[6]);
