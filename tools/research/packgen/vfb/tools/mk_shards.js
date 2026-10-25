// (party VF-b) THE CHANDELIER'S FRAGMENTS WITH THEIR OWN MATERIAL, through the character pipeline:
//   node mk_shards.js XSI_ROOT OUT_PACK
// XSI_ROOT: the XS root as deployed (PACK/xsi: vfx/effects, vfx/meshes, weapons/materials_xwb); OUT_PACK: the pack folder that
// gets NEW files only:
//   xsi/weapons/shards_vfb.json            routes (the effect material whose mesh particles go to the character pipeline:
//                                          w_so088_mat0_Inst) and the fragment models (effect mesh -> vfb_<mesh>, its glb)
//   xsi/weapons/shards_vfb/vfb_<mesh>.glb  the effect pack's fragment mesh as it is, its glTF material named vfb_<mesh> (the
//                                          library slot it is drawn with: one slot per model, as the hosts key slots)
//   xsi/weapons/materials_vfb/materials.json  the chandelier pawn's translated material (materials_xwb: owner w_so088_mat0,
//                                          id 410) once per fragment model (mesh xsi:vfb_<mesh>), textures from materials_xwb, the
//                                          fragment's own cooked bounds (mesh_facts: the object rows of the character harness)
//   xsi/weapons/materials_vfb/er/*         copies of materials_xwb/er (owner 410's variants: the Elden Ring renderer reads a
//                                          library's shaders from its own folder)
'use strict';
const fs = require('fs'), path = require('path');
const [root, out] = process.argv.slice(2);
const rd = (f) => JSON.parse(fs.readFileSync(f, 'utf8'));
const W = path.join(out, 'xsi', 'weapons');
const MATERIAL = 'w_so088_mat0_Inst';
// the meshes of every emitter whose material is the chandelier's own
const meshes = new Set();
for (const f of fs.readdirSync(path.join(root, 'vfx', 'effects'))) {
  const j = rd(path.join(root, 'vfx', 'effects', f));
  for (const e of j.emitters || []) {
    const mat = (e.required && e.required.material_file) || '';
    if (path.basename(mat, '.json') !== MATERIAL) continue;
    const td = e.typedata || {};
    for (const m of [td.mesh_file, ...((td.sub_mesh_files || []).flat())]) if (m && /kmd_so088_/.test(m)) meshes.add(path.basename(m, '.glb'));
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
fs.mkdirSync(path.join(W, 'shards_vfb'), { recursive: true }); fs.mkdirSync(path.join(W, 'materials_vfb', 'er'), { recursive: true });
const materials = {}, facts = {}, models = [];
for (const mesh of list) {
  const src = path.join(root, 'vfx', 'meshes', mesh + '.glb'); if (!fs.existsSync(src)) { console.log('no mesh ' + mesh); continue; }
  const model = 'vfb_' + mesh; const g = readGlb(src);
  g.json.materials = [{ name: model, extras: { source: (g.json.materials || [])[0] || null, note: '(party VF-b) the slot of materials_vfb/materials.json' } }];
  for (const m of g.json.meshes) for (const p of m.primitives) p.material = 0;
  if (g.json.skins && g.json.skins.length) throw new Error(mesh + ': skinned');
  writeGlb(path.join(W, 'shards_vfb', model + '.glb'), g.json, g.bin);
  // the cooked bounds in component space (GAME cm): the pack bakes (x, y, z) cm -> (x, z, y) * 0.01 m
  let lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
  for (const m of g.json.meshes) for (const p of m.primitives) { const a = g.json.accessors[p.attributes.POSITION]; for (let k = 0; k < 3; k++) { lo[k] = Math.min(lo[k], a.min[k]); hi[k] = Math.max(hi[k], a.max[k]); } }
  const game = (v) => [v[0] * 100, v[2] * 100, v[1] * 100];
  const [a, b] = [game(lo), game(hi)]; const mn = a.map((x, k) => Math.min(x, b[k])), mx = a.map((x, k) => Math.max(x, b[k]));
  const origin = mn.map((x, k) => (x + mx[k]) / 2), extent = mn.map((x, k) => (mx[k] - x) / 2);
  facts['xsi:' + model] = { bone_params: [], bounds: { origin, extent, radius: Math.hypot(...extent) } };
  materials[model] = { ...record, mesh: 'xsi:' + model, slot: 0, textures: record.textures.map((t) => ({ ...t, file: '../materials_xwb/' + t.file })) };
  models.push({ mesh, model, file: 'shards_vfb/' + model + '.glb' });
}
const lib = { format: xwb.format, generator: 'build/sora_tmp/er/party/vfb/tools/mk_shards.js (party VF-b)', max_textures: xwb.max_textures, max_cb: xwb.max_cb, max_collection: xwb.max_collection,
  note: '(party VF-b) the chandelier pawn w_so088\'s translated material (materials_xwb, owner w_so088_mat0, id 410) on each fragment model of shards_vfb.json (mesh xsi:vfb_<mesh>); its shaders: er/ (copies of materials_xwb/er)',
  owners: { w_so088_mat0: owner }, materials, uv2: {}, mesh_facts: facts };
fs.writeFileSync(path.join(W, 'materials_vfb', 'materials.json'), JSON.stringify(lib, null, 1) + '\n');
for (const f of fs.readdirSync(path.join(root, 'weapons', 'materials_xwb', 'er'))) fs.copyFileSync(path.join(root, 'weapons', 'materials_xwb', 'er', f), path.join(W, 'materials_vfb', 'er', f));
fs.writeFileSync(path.join(W, 'shards_vfb.json'), JSON.stringify({ format: 'kh3-xsi-shards-vfb/1', generator: 'build/sora_tmp/er/party/vfb/tools/mk_shards.js (party VF-b)', legal: 'local build output from the user\'s own copy of the game; never distributed',
  note: '(party VF-b) Chandelier\'s shatter fragments (vw_so088_fin_hahen00..02 shanderia_*, han01 c_dst0: mesh particles whose material is the chandelier pawn\'s own character material w_so088_mat0_Inst) are drawn by the hosts\' CHARACTER pipeline with that material (materials_vfb), one draw per particle at its transform, instead of the effect pipeline (XW-c drew a stand-in). Keys: kh-er fx.shards_own, kh-app KH_FX_SHARDS_OWN.',
  materials: 'materials_vfb/materials.json', routes: [{ effect_material: MATERIAL }], models }, null, 1) + '\n');
console.log(`${models.length} fragment models (${list.length} meshes named by the effects), library slots ${Object.keys(materials).length}`);
