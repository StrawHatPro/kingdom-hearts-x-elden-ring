#!/usr/bin/env node
// The vfx/ rows of sheets/port/sora-assets.json.
//
// tools/sora_vfx_pack.js writes build/sora_pack/vfx/sheet_rows.json (effects, materials, textures, meshes, triggers,
// manifest). Two later tools add files to the same folder: tools/sora_vfx_host.js (host.json) and
// tools/sora_vfx_shaders.js (shaders/materials.wgsl, shaders/shading.json). This tool adds their rows to
// sheet_rows.json and then replaces the vfx/ rows of the sheet with that list, in the row format tools/sora_pack.js
// writes (it merges the same file when it runs; this is that merge alone, without rebuilding the Sora pack).
//
//   node tools/sora_vfx_assets_rows.js           write
//   node tools/sora_vfx_assets_rows.js --check   report only; exit 1 when the sheet differs
const fs = require('fs'), path = require('path'), crypto = require('crypto');
const ROOT = path.resolve(__dirname, '..');
const VFX = path.join(ROOT, 'build/sora_pack/vfx');
const SHEET = path.join(ROOT, 'sheets/port/sora-assets.json');
const check = process.argv.includes('--check');
const sha = (b) => crypto.createHash('sha256').update(b).digest('hex');

const EXTRA = [
  ['host.json', 'data', 'json read back', 'host table of the effect renderer (sockets, weapon sockets, projectile class -> effects, actor meshes); written by tools/sora_vfx_host.js from the blueprints and sheets named in it'],
  ['shaders/materials.wgsl', 'data', 'compiled by the app at start (naga); every translated function used by a looked-at effect was drawn', 'pixel shaders of the effect materials, translated from the DXBC of the cooked shader maps by tools/sora_vfx_shaders.js'],
  ['shaders/shading.json', 'data', 'json read back', 'per material: shader owner, blend mode, constant buffer and texture slots, read from the cooked shader maps by tools/sora_vfx_shaders.js'],
];

const rowsFile = path.join(VFX, 'sheet_rows.json');
let rows = JSON.parse(fs.readFileSync(rowsFile, 'utf8'));
const extraIds = new Set(EXTRA.map((e) => 'vfx/' + e[0]));
rows = rows.filter((r) => !extraIds.has(r[0]));
const at = rows.findIndex((r) => r[0] === 'vfx/manifest.json');
const add = [];
for (const [file, kind, validation, evidence] of EXTRA) {
  const f = path.join(VFX, file);
  if (!fs.existsSync(f)) { console.error(`missing ${file}: run its tool first`); process.exit(1); }
  const b = fs.readFileSync(f);
  add.push(['vfx/' + file, kind, '-', '-', '-', b.length, sha(b), '-', '-', '-', '-', validation, evidence]);
}
rows.splice(at < 0 ? rows.length : at, 0, ...add);

const text = fs.readFileSync(SHEET, 'utf8');
const sheet = JSON.parse(text);
for (const r of rows) if (!Array.isArray(r) || r.length !== sheet.columns.length || !String(r[0]).startsWith('vfx/')) throw new Error('bad row ' + JSON.stringify(r).slice(0, 120));
const first = sheet.rows.findIndex((r) => String(r[0]).startsWith('vfx/'));
const others = sheet.rows.filter((r) => !String(r[0]).startsWith('vfx/'));
const pos = first < 0 ? others.length : first;
const merged = [...others.slice(0, pos), ...rows, ...others.slice(pos)];
const out = `{"sheet":"sora-assets","group":"port","columns":${JSON.stringify(sheet.columns)},"rows":[\n${merged.map((r) => JSON.stringify(r)).join(',\n')}\n]}\n`;
if (check) { const same = out === text; console.log(same ? 'sora-assets vfx rows: up to date' : 'sora-assets vfx rows: DIFFER'); process.exit(same ? 0 : 1); }
fs.writeFileSync(rowsFile, JSON.stringify(rows));
fs.writeFileSync(SHEET, out);
console.log(`sora-assets: ${rows.length} vfx rows (${add.length} from the host and shader tools), ${merged.length} rows in all`);
