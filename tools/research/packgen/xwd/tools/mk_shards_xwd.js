// (XW-d) THE CHANDELIER'S OTHER FRAGMENTS (the pieces kh_vfx draws since XW-d that VF-b's shards_vfb.json lacks: hahen02's
// kmd_so088_c05, hahen03's kmd_so088_d00..04) through the character pipeline, as party/vfb/tools/mk_shards.js does:
//   node mk_shards_xwd.js XSI_ROOT XWD_XSI OUT_PACK
// XSI_ROOT: the XS root as deployed (PACK/xsi, with weapons/materials_xwb and weapons/shards_vfb.json); XWD_XSI: the XW-d
// pack's xsi folder (vfx/effects_xwd, vfx/meshes); OUT_PACK gets NEW files only:
//   xsi/weapons/shards_xwd.json, shards_xwd/xwd_<mesh>.glb, materials_xwd/{materials.json, er/*}
'use strict';
const fs = require('fs'), path = require('path');
const [root, xwd, out] = process.argv.slice(2);
const rd = (f) => JSON.parse(fs.readFileSync(f, 'utf8'));
const W = path.join(out, 'xsi', 'weapons');
const MATERIAL = 'w_so088_mat0_Inst';
// the meshes of every emitter whose material is the chandelier's own
const meshes = new Set();
const done = new Set(rd(path.join(root, 'weapons', 'shards_vfb.json')).models.map((m) => m.mesh.toLowerCase()));
for (const f of fs.readdirSync(path.join(root, 'vfx', 'effects'))) {
  const own = path.join(xwd, 'vfx', 'effects_xwd', f);
  const j = rd(fs.existsSync(own) ? own : path.join(root, 'vfx', 'effects', f));
  for (const e of j.emitters || []) {
    const mat = (e.required && e.required.material_file) || '';
    if (path.basename(mat, '.json') !== MATERIAL) continue;
    const td = e.typedata || {};
    for (const m of [td.mesh_file, ...((td.sub_mesh_files || []).flat())]) if (m && /kmd_so088_/.test(m) && !done.has(path.basename(m, '.glb').toLowerCase())) meshes.add(path.basename(m, '.glb'));
  }
}
const list = [...meshes].sort();
// glb in / out
function readGlb(file) {
  const b = fs.readFileSync(file); if (b.toString('ascii', 0, 4) !== 'glTF') throw new Error(file + ': not a glb');
  const jl = b.readUInt32LE(12); const json = JSON.parse(b.slice(20, 20 + jl).toString('utf8'));
  const at = 20 + jl; const bl = b.readUInt32LE(at); return { json, bin: b.slice(at + 8, at + 8 + bl) };
}
function writeGlb(file, json, bin) {
  let j = Buffer.from(JSON.stringify(json), 'utf8'); const jp = (4 - (j.length % 4)) % 4; j = Buffer.concat([j, Buffer.alloc(jp, 0x20)]);
  const bp = (4 - (bin.length % 4)) % 4; const b = Buffer.concat([bin, Buffer.alloc(bp, 0)]);
  const head = Buffer.alloc(12); head.write('glTF', 0, 'ascii'); head.writeUInt32LE(2, 4); head.writeUInt32LE(12 + 8 + j.length + 8 + b.length, 8);
  const ch = (len, type) => { const c = Buffer.alloc(8); c.writeUInt32LE(len, 0); c.write(type, 4, 'ascii'); return c; };
  fs.writeFileSync(file, Buffer.concat([head, ch(j.length, 'JSON'), j, ch(b.length, 'BIN\0'), b]));
}
const xwb = rd(path.join(root, 'weapons', 'materials_xwb', 'materials.json'));
const owner = xwb.owners.w_so088_mat0; if (!owner) throw new Error('materials_xwb has no owner w_so088_mat0');
const record = Object.values(xwb.materials).find((m) => m.owner === 'w_so088_mat0'); if (!record) throw new Error('materials_xwb has no record of owner w_so088_mat0');
fs.mkdirSync(path.join(W, 'shards_xwd'), { recursive: true }); fs.mkdirSync(path.join(W, 'materials_xwd', 'er'), { recursive: true });
const materials = {}, facts = {}, models = [];
for (const mesh of list) {
  const src = [path.join(xwd, 'vfx', 'meshes', mesh + '.glb'), path.join(root, 'vfx', 'meshes', mesh + '.glb')].find((p) => fs.existsSync(p)); if (!src) { console.log('no mesh ' + mesh); continue; }
  const model = 'xwd_' + mesh; const g = readGlb(src);
  g.json.materials = [{ name: model, extras: { source: (g.json.materials || [])[0] || null, note: '(XW-d) the slot of materials_xwd/materials.json' } }];
  for (const m of g.json.meshes) for (const p of m.primitives) p.material = 0;
  if (g.json.skins && g.json.skins.length) throw new Error(mesh + ': skinned');
  writeGlb(path.join(W, 'shards_xwd', model + '.glb'), g.json, g.bin);
  // the cooked bounds in component space (GAME cm): the pack bakes (x, y, z) cm -> (x, z, y) * 0.01 m
  let lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
  for (const m of g.json.meshes) for (const p of m.primitives) { const a = g.json.accessors[p.attributes.POSITION]; for (let k = 0; k < 3; k++) { lo[k] = Math.min(lo[k], a.min[k]); hi[k] = Math.max(hi[k], a.max[k]); } }
  const game = (v) => [v[0] * 100, v[2] * 100, v[1] * 100];
  const [a, b] = [game(lo), game(hi)]; const mn = a.map((x, k) => Math.min(x, b[k])), mx = a.map((x, k) => Math.max(x, b[k]));
  const origin = mn.map((x, k) => (x + mx[k]) / 2), extent = mn.map((x, k) => (mx[k] - x) / 2);
  facts['xsi:' + model] = { bone_params: [], bounds: { origin, extent, radius: Math.hypot(...extent) } };
  materials[model] = { ...record, mesh: 'xsi:' + model, slot: 0, textures: record.textures.map((t) => ({ ...t, file: '../materials_xwb/' + t.file })) };
  models.push({ mesh, model, file: 'shards_xwd/' + model + '.glb' });
}
const lib = { format: xwb.format, generator: 'build/sora_tmp/er/xwd/tools/mk_shards_xwd.js (XW-d)', max_textures: xwb.max_textures, max_cb: xwb.max_cb, max_collection: xwb.max_collection,
  note: '(XW-d) as materials_vfb: the chandelier pawn w_so088\'s translated material (owner 410) on each fragment model of shards_xwd.json (mesh xsi:xwd_<mesh>); its shaders: er/ (copies of materials_xwb/er)',
  owners: { w_so088_mat0: owner }, materials, uv2: {}, mesh_facts: facts };
fs.writeFileSync(path.join(W, 'materials_xwd', 'materials.json'), JSON.stringify(lib, null, 1) + '\n');
for (const f of fs.readdirSync(path.join(root, 'weapons', 'materials_xwb', 'er'))) fs.copyFileSync(path.join(root, 'weapons', 'materials_xwb', 'er', f), path.join(W, 'materials_xwd', 'er', f));
fs.writeFileSync(path.join(W, 'shards_xwd.json'), JSON.stringify({ format: 'kh3-xsi-shards-vfb/1', generator: 'build/sora_tmp/er/xwd/tools/mk_shards_xwd.js (XW-d)', legal: 'local build output from the user\'s own copy of the game; never distributed',
  note: '(XW-d) more of the Chandelier\'s shatter fragments for the character pipeline, read by the hosts AFTER shards_vfb.json (same format, same material w_so088_mat0_Inst): the pieces VF-b\'s file lacks (hahen02 kmd_so088_c05, hahen03 kmd_so088_d00..04; kh_vfx draws TypeDataSQEX_FragmentMesh since XW-d). Keys as shards_vfb.json: kh-er fx.shards_own, kh-app KH_FX_SHARDS_OWN.',
  materials: 'materials_xwd/materials.json', routes: [{ effect_material: MATERIAL }], models }, null, 1) + '\n');
console.log(`${models.length} fragment models (${list.length} meshes named by the effects), library slots ${Object.keys(materials).length}`);
