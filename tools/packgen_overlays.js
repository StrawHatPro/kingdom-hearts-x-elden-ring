#!/usr/bin/env node
// (PACKGEN) THE EFFECT TABLES' OVERLAYS, their translator runs: the streams (XW, XW-b, XW-c, party VF-b, XW-d, RSF rounds
// 20 / 21) laid records and shader blocks over the three effect tables (PACK/<root>/vfx/shaders/shading_<s>.json,
// materials_<s>.wgsl, er/<name>_<s>.*.dxil): each from a run of tools/sora_vfx_shaders.js with its own options, written
// to --out only. This tool makes those runs on the player's PC (the "stages", build/packgen/vfx_stage/<name>), from the
// effect roots as the pack and the replay left them; the overlay files are then recipe files over the stages
// (tools/packgen/replay/overlays_*.json, tools/packgen_replay.js apply --overlays) and their shaders are built by
// tools/packgen_shaders.js (step vfx-overlays).
//
//   node tools/packgen_overlays.js [--only NAME,...]
//
// The runs (tools/packgen/recipes/vfx_stages.json: our options): every run keeps the pack's owner ids
// (KH_KEEP_OWNER_IDS: tools/packgen/recipes/vfx_owners_<table>.json) and the streams' texture records
// (KH_PARTY_EXTRA_MANIFEST, as tools/packgen.js vfx-tables), then ZA's decal pass (decal_dyn.js) on the run's table.
// The XS chandelier runs read the chandelier's own effects (--xsi-extra): the scratch pack xsi_chandelier
// (tools/packgen_replay.js prepare), XW-d's with the three suffixed XS material files of the pack added.
'use strict';
const fs = require('fs');
const path = require('path');
const cp = require('child_process');

const ROOT = path.resolve(__dirname, '..');
const RECIPE = path.join(ROOT, 'tools', 'packgen', 'recipes', 'vfx_stages.json');
const STAGE = path.join(ROOT, 'build', 'packgen', 'vfx_stage');
const PACK = path.join(ROOT, 'build', 'sora_pack');
const args = process.argv.slice(2);
const opt = (k) => { const i = args.indexOf(k); return i >= 0 ? args[i + 1] : null; };
const only = opt('--only') ? new Set(opt('--only').split(',')) : null;
const recipe = JSON.parse(fs.readFileSync(RECIPE, 'utf8'));
const T = 'build/sora_tmp';

const extraManifests = ['DON-KIT', 'DON-MAGIC', 'ZETTA', 'DON-SPELLB', 'AUD', 'HYB', 'YFX'].map((s) => path.join(ROOT, T, 'party', s, 'pack', 'vfx', 'manifest.json')).filter((f) => fs.existsSync(f)).map((f) => f.split(path.sep).join('/')).join(';');
function node(argv, env = {}) {
  const r = cp.spawnSync(process.execPath, argv, { cwd: ROOT, encoding: 'utf8', maxBuffer: 1 << 28, env: { ...process.env, MSYS_NO_PATHCONV: '1', ...env } });
  if (r.status !== 0) { console.error(`node ${argv.join(' ')} failed:\n${(r.stdout + r.stderr).split('\n').slice(-15).join('\n')}`); process.exit(1); }
  return r.stdout;
}
// the --xsi-extra packs
const extras = {};
for (const [name, x] of Object.entries(recipe.extras || {})) {
  const dir = path.join(STAGE, '_extra_' + name);
  fs.rmSync(dir, { recursive: true, force: true });
  const src = path.join(ROOT, x.from);
  if (!fs.existsSync(src)) { console.error(`packgen_overlays: ${x.from} is missing (tools/packgen_replay.js prepare)`); process.exit(1); }
  fs.cpSync(src, dir, { recursive: true });
  for (const f of x.add_from_pack || []) { const d = path.join(dir, f.slice(f.indexOf('/vfx/') + 5)); fs.mkdirSync(path.dirname(d), { recursive: true }); fs.copyFileSync(path.join(PACK, f), d); }
  extras[name] = dir;
}
let n = 0;
for (const [name, s] of Object.entries(recipe.stages)) {
  if (only && !only.has(name)) continue;
  const out = path.join(STAGE, name);
  fs.rmSync(out, { recursive: true, force: true });
  const a = s.args.map((x) => (x.startsWith('@extra:') ? extras[x.slice(7)] : x));
  node(['tools/sora_vfx_shaders.js', ...a, '--out', out], { KH_KEEP_OWNER_IDS: path.join(ROOT, 'tools', 'packgen', 'recipes', `vfx_owners_${s.table}.json`), ...(s.table === 'main' ? {} : { KH_PARTY_EXTRA_MANIFEST: extraManifests }) });
  if (s.decal_dyn !== false) node([`${T}/party/ZA/tools/decal_dyn.js`, out, out]);
  n++;
  console.log(`packgen_overlays: ${name}: sora_vfx_shaders.js ${s.args.join(' ')} -> ${path.relative(ROOT, out)}`);
}
console.log(`packgen_overlays: ${n} translator run(s)`);
