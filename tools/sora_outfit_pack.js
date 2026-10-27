#!/usr/bin/env node
// tools/sora_outfit_pack.js (IMPL_ER59): SORA'S OTHER OUTFITS as folders of the Sora pack, for the Elden Ring renderer.
//
// The user: "Boc is able to change Sora's appearance: KH1, KH2, and all of Sora's world outfits in 3".
// KH3's own player-pawn table (FUN_140ea70b0, decompiled C: ETresPlayerUniqueID -> pawn class) and the pawns' class
// defaults (m_PlayerUniqueID, read from the decoded packages) name the models used here:
//   SORA_KH3 0 -> p_ex001 (the main pack), SORA_KH3CA 1 -> p_ca001, SORA_KH3MI 2 -> p_mi001, SORA_KH3TS 3 -> p_ts001,
//   SORA_KH3BX 4 -> p_bx001, SORA_KH2 6 -> p_ex011, SORA_KH1 7 -> p_ex005. SORA_KH3DL 15 -> p_dl001 (the realistic Sora of
//   the Verum Rex fight, Re Mind: 4 of its 16 material slots, the clothes and the eyes, have shaders the translator cannot
//   take (more than 14 textures / a constant buffer over 160 rows): drawn with STAND-INS, see sora_material_pack.js).
//   NOT USED: SORA_KH3EW / SORA_KH3PO (p_ew001 / p_po001: the default look).
// Per outfit this writes build/sora_pack/outfits/<id>/ (NEW files only; the main pack is not touched):
//   model.glb       LOD 0 of Character/pc/<model>/mdl/<model>_0, skinned to the outfit's OWN skeleton (bind pose kept),
//                   in the pack's conventions; a vertex keeps its four largest influences (the cooked mesh has up to
//                   eight for some outfits); a bone of the outfit that Sora's skeleton (p_ex001) does not have is FOLDED
//                   into its nearest ancestor that it has (EXACT for a bone that keeps its reference pose relative to
//                   its parent, which is what the renderer gives such a bone: nothing animates or simulates it)
//   manifest.json   the outfit's skeleton (the main manifest's format), its material slots, the mesh asset, the
//                   wear-form set of its pawn folder (Blueprints/Player/<model>/<model>_WearformSet, when there is one)
//   materials/      tools/sora_material_pack.js with KH_OUTFIT_PACK=<folder>: the game's compiled shaders of the
//                   outfit's material instances translated, their textures, the wear forms
//   materials/er/   crates/kh-shaderc --pack <folder>: DXIL + block-compressed textures for the renderer
// and build/sora_pack/outfits/outfits.json (the list the DLL and the menu read).
//
//   node tools/sora_outfit_pack.js [--only kh2,caribbean] [--no-materials] [--no-shaders]
'use strict';
const fs = require('fs'), path = require('path'), cp = require('child_process'), crypto = require('crypto');
const sp = require('./sora_pack.js');
const ROOT = sp.ROOT;
const PACK = path.join(ROOT, 'build/sora_pack');
const OUTFITS = path.join(PACK, 'outfits');
const VERSION = '1.0.0';
const MAX_PALETTE = 320; // crates/kh-er-render gfx.rs MAX_JOINTS_OUTFIT (er_char.wgsl KH_JOINTS_EXTRA)

// id, model, ETresPlayerUniqueID, the menu's text (a key of the UI pack's strings, or LOCAL text), the world's text key.
const LIST = [
  { id: 'kh3', model: 'p_ex001', unique: 'SORA_KH3', text_key: 'kh3_system.home_0010', main: true },
  { id: 'kh2', model: 'p_ex011', unique: 'SORA_KH2', text: 'KINGDOM HEARTS II' },
  { id: 'kh1', model: 'p_ex005', unique: 'SORA_KH1', text: 'KINGDOM HEARTS' },
  { id: 'toybox', model: 'p_ts001', unique: 'SORA_KH3TS', text_key: 'kh3_map.world_ts' },
  { id: 'monstropolis', model: 'p_mi001', unique: 'SORA_KH3MI', text_key: 'kh3_map.world_mi' },
  { id: 'caribbean', model: 'p_ca001', unique: 'SORA_KH3CA', text_key: 'kh3_map.world_ca' },
  { id: 'sanfransokyo', model: 'p_bx001', unique: 'SORA_KH3BX', text_key: 'kh3_map.world_bx' },
  // the Verum Rex fight (Re Mind, level ss_DLC/ss_01): its game mode TresGameMode_3_SS has DefaultPawnClass p_dl001_Pawn_C
  { id: 'verumrex', model: 'p_dl001', unique: 'SORA_KH3DL', text: 'Quadratum' },
];

const args = process.argv.slice(2);
const only = (() => { const k = args.indexOf('--only'); return k >= 0 ? args[k + 1].split(',') : null; })();
const meshFile = (model) => path.join(ROOT, 'assets_converted/meshes', sp.CONTENT, 'Character/pc', model, 'mdl', model + '_0.glb');
const sha = (b) => crypto.createHash('sha256').update(b).digest('hex');

function pawnUnique(model) { // the pawn's m_PlayerUniqueID (class default object)
  const f = path.join(ROOT, 'assets_converted/packages', sp.CONTENT, 'Blueprints/Player', model, model + '_Pawn.uasset.json');
  if (!fs.existsSync(f)) return null;
  const m = /"m_PlayerUniqueID","type":"EnumProperty","enumName":"ETresPlayerUniqueID","value":"ETresPlayerUniqueID::(\w+)"/.exec(fs.readFileSync(f, 'utf8'));
  return m ? m[1] : null;
}
function wearformSet(model) {
  const f = path.join(ROOT, 'assets_converted/packages', sp.CONTENT, 'Blueprints/Player', model, model + '_WearformSet.uasset.json');
  return fs.existsSync(f) ? `/Game/Blueprints/Player/${model}/${model}_WearformSet` : null;
}

// The mesh: LOD 0, the outfit's own skeleton and bind pose; influences folded and cut to four.
function buildModel(o, bones, hostNames, keep = new Set()) {
  const g = sp.readGlb(meshFile(o.model)); const j = g.json; const lod0 = j.meshes[0];
  // fold: a bone Sora's skeleton lacks -> its nearest ancestor that Sora has
  // (ER69) a bone the outfit's own Bonamik drives (its cloth, tail, coat) is kept: the renderer simulates it
  const fold = bones.map((b, i) => { let k = i; while (k >= 0 && !hostNames.has(bones[k].name) && !keep.has(k)) k = bones[k].parent; return k < 0 ? 0 : k; });
  const folded = bones.filter((b, i) => fold[i] !== i).length;
  const cache = new Map(); const rd = (i) => { if (!cache.has(i)) cache.set(i, sp.readAccessor(g, i)); return cache.get(i); };
  // per primitive: vertices' influences
  const prims = [];
  const used = new Set();
  let cut = 0, maxIn = 0;
  for (const pr of lod0.primitives) {
    const idx = rd(pr.indices).data;
    let lo = Infinity, hi = -1; for (const v of idx) { if (v < lo) lo = v; if (v > hi) hi = v; }
    const n = hi - lo + 1;
    const A = pr.attributes;
    const sets = [];
    for (let s = 0; A['JOINTS_' + s] !== undefined; s++) { const ja = rd(A['JOINTS_' + s]), wa = rd(A['WEIGHTS_' + s]); const div = wa.componentType === 5121 ? 255 : wa.componentType === 5123 ? 65535 : 1; sets.push({ j: ja.data, w: wa.data, div }); }
    const J = new Array(n * 4).fill(0), W = new Array(n * 4).fill(0);
    for (let v = 0; v < n; v++) {
      const acc = new Map();
      for (const s of sets) for (let c = 0; c < 4; c++) { const w = s.w[(lo + v) * 4 + c] / s.div; if (w <= 0) continue; const b = fold[s.j[(lo + v) * 4 + c]]; acc.set(b, (acc.get(b) || 0) + w); }
      const list = [...acc].sort((a, b) => b[1] - a[1]);
      if (list.length > maxIn) maxIn = list.length;
      if (list.length > 4) cut++;
      const top = list.slice(0, 4); let sum = top.reduce((s, x) => s + x[1], 0);
      if (!(sum > 0)) { top.length = 0; top.push([0, 1]); sum = 1; }
      top.forEach(([b, w], c) => { J[v * 4 + c] = b; W[v * 4 + c] = w / sum; used.add(b); });
    }
    prims.push({ pr, idx, lo, n, J, W });
  }
  const joints = [...used].sort((a, b) => a - b);
  if (joints.length > MAX_PALETTE) throw new Error(`${o.id}: ${joints.length} joints after folding, the renderer has room for ${MAX_PALETTE}`);
  const jointOf = new Map(joints.map((b, k) => [b, k]));
  // material slot names: unique (the material library keys slots by name)
  const names = []; j.materials.forEach((m, k) => { let n = m.name && m.name !== 'None' ? m.name : `slot${k}`; if (names.includes(n)) n = `${n}_${k}`; names.push(n); });
  const w = new sp.GlbWriter(`(outfit ${o.id}: tools/sora_outfit_pack.js ${VERSION})`);
  sp.addSkeletonNodes(w, bones);
  const ibm = sp.globalBind(bones).map(sp.invertAffine);
  const skinAcc = w.accessor(joints.map((b) => ibm[b]).flat(), 'MAT4', 5126);
  w.json.skins = [{ name: o.model + '_0', joints, skeleton: 0, inverseBindMatrices: skinAcc }];
  const out = []; const stats = { vertices: 0, triangles: 0 };
  for (const p of prims) {
    const A = p.pr.attributes; const slice = (acc, comps) => rd(acc).data.slice(p.lo * comps, (p.lo + p.n) * comps);
    const pos = slice(A.POSITION, 3), nor = slice(A.NORMAL, 3), tan = A.TANGENT !== undefined ? slice(A.TANGENT, 4) : null;
    const P2 = new Array(p.n * 3), N2 = new Array(p.n * 3), T2 = new Array(p.n * 4);
    for (let i = 0; i < p.n; i++) {
      const q = sp.cvPos(pos[i * 3], pos[i * 3 + 1], pos[i * 3 + 2]); P2[i * 3] = q[0]; P2[i * 3 + 1] = q[1]; P2[i * 3 + 2] = q[2];
      const nn = sp.cvDir(nor[i * 3], nor[i * 3 + 1], nor[i * 3 + 2]); N2[i * 3] = nn[0]; N2[i * 3 + 1] = nn[1]; N2[i * 3 + 2] = nn[2];
      if (tan) { const tt = sp.cvDir(tan[i * 4], tan[i * 4 + 1], tan[i * 4 + 2]); T2[i * 4] = tt[0]; T2[i * 4 + 1] = tt[1]; T2[i * 4 + 2] = tt[2]; T2[i * 4 + 3] = -tan[i * 4 + 3]; } else { T2[i * 4] = 1; T2[i * 4 + 1] = 0; T2[i * 4 + 2] = 0; T2[i * 4 + 3] = 1; }
    }
    const JJ = p.J.map((b, k) => (p.W[k] === 0 ? 0 : jointOf.get(b)));
    const attributes = {
      POSITION: w.accessor(P2, 'VEC3', 5126, { minmax: true, target: 34962 }), NORMAL: w.accessor(N2, 'VEC3', 5126, { target: 34962 }), TANGENT: w.accessor(T2, 'VEC4', 5126, { target: 34962 }),
      JOINTS_0: w.accessor(JJ, 'VEC4', 5123, { target: 34962 }), WEIGHTS_0: w.accessor(p.W, 'VEC4', 5126, { target: 34962 }),
    };
    for (let u = 0; A['TEXCOORD_' + u] !== undefined; u++) attributes['TEXCOORD_' + u] = w.accessor(slice(A['TEXCOORD_' + u], 2), 'VEC2', 5126, { target: 34962 });
    const ix = new Array(p.idx.length); // the reflection is baked into the vertices: the winding is reversed (as tools/sora_pack.js)
    for (let i = 0; i < p.idx.length; i += 3) { ix[i] = p.idx[i] - p.lo; ix[i + 1] = p.idx[i + 2] - p.lo; ix[i + 2] = p.idx[i + 1] - p.lo; }
    out.push({ attributes, indices: w.accessor(ix, 'SCALAR', p.n > 65535 ? 5125 : 5123, { target: 34963 }), mode: 4, material: p.pr.material });
    stats.vertices += p.n; stats.triangles += ix.length / 3;
  }
  w.json.meshes = [{ name: o.model + '_0_LOD0', primitives: out }];
  w.json.materials = j.materials.map((m, k) => ({ name: names[k], pbrMetallicRoughness: { metallicFactor: 0, roughnessFactor: 0.8 }, extras: { ue_material: m.extras && m.extras.ue_material } }));
  const node = w.json.nodes.push({ name: 'sora_mesh', mesh: 0, skin: 0 }) - 1;
  w.json.scenes = [{ name: o.id, nodes: [0, node] }];
  const materials = j.materials.map((m, k) => ({ slot: k, slot_name: names[k], material_instance: m.extras && m.extras.ue_material }));
  return { glb: w.finish(), materials, stats: { ...stats, palette: joints.length, folded_bones: folded, vertices_cut_to_four: cut, most_influences: maxIn } };
}

// The shown name: the UI pack's English string of `text_key` (build/sora_pack/ui/strings/en/<table>.json), else the LOCAL text.
function nameOf(o) {
  if (o.text_key) { const [table, key] = o.text_key.split('.'); const f = path.join(PACK, 'ui/strings/en', table + '.json'); if (fs.existsSync(f)) { const t = fs.readFileSync(f, 'utf8'); const m = new RegExp('"' + key + '":"([^"]*)"').exec(t); if (m) return m[1]; } }
  return o.text || o.id;
}
// The hip height (bone koshi, reference pose, pack space: Y up, metres): an outfit's over Sora's is its BODY SCALE
// (crates/kh-er-render outfit.rs scales his animated translations by it; kh-er boc.rs his camera's target height).
function hipHeight(bones) { const k = bones.findIndex((b) => b.name === 'koshi'); return k < 0 ? null : sp.globalBind(bones)[k][13]; }
// (ER73) --kd-mesh: THE MESH WITH THE OUTFIT'S KINEDRIVER BONES: model_kd.glb (a NEW file; model.glb, the manifest and every
// other file are left as they are), skinned like model.glb but the bones the outfit's own KineDriver writes
// (kinedriver.json, tools/sora_outfit_kdi.js: knee, hip, shoulder, boot-top pieces) keep their influences instead of
// being folded into a parent. crates/kh-er-render outfit.rs draws it when the outfit's kinedriver.json is there.
function kdMeshes() {
  const NL = String.fromCharCode(10);
  const host = sp.buildSkeleton(sp.readGlb(meshFile('p_ex001')).json);
  const hostNames = new Set(host.map((b) => b.name));
  const rows = [];
  for (const o of LIST) {
    if (o.main || (only && !only.includes(o.id))) continue;
    const dir = path.join(OUTFITS, o.id);
    const kdFile = path.join(dir, 'kinedriver.json');
    if (!fs.existsSync(kdFile)) { console.log(`${o.id}: no kinedriver.json, no model_kd.glb`); continue; }
    const bones = sp.buildSkeleton(sp.readGlb(meshFile(o.model)).json);
    const kd = JSON.parse(fs.readFileSync(kdFile, 'utf8'));
    const bnmFile = path.join(dir, 'bonamik.json');
    const bnm = fs.existsSync(bnmFile) ? JSON.parse(fs.readFileSync(bnmFile, 'utf8')) : null;
    const sim = new Set(bnm ? bnm.bones.filter((b) => b.bonamik !== 'none').map((b) => b.index) : []);
    const written = new Set(kd.bones.filter((b) => b.written).map((b) => b.index));
    let m;
    try { m = buildModel(o, bones, hostNames, new Set([...sim, ...written])); } catch (e) {
      console.log('  ' + e.message + ': Bonamik followers folded');
      const strict = new Set([...sim].filter((k) => !/follower/.test(bnm.bones[k].bonamik)));
      m = buildModel(o, bones, hostNames, new Set([...strict, ...written]));
    }
    const out = path.join(dir, 'model_kd.glb');
    fs.writeFileSync(out, m.glb);
    const unfolded = [...written].filter((k) => !hostNames.has(bones[k].name)).length;
    console.log(`${o.id}: model_kd.glb, palette ${m.stats.palette}, ${m.stats.folded_bones} folded, ${unfolded} KineDriver bones of its own kept (${written.size} written)`);
    rows.push(['outfits/' + o.id + '/model_kd.glb', 'mesh', '-', `${sp.CONTENT}/Character/pc/${o.model}/mdl/${o.model}_0.uasset`, '-', m.glb.length, sha(m.glb), '-', '-', '-', '-', 'read back by the renderer (crates/kh-er-render outfit.rs) when the outfit has kinedriver.json', 'tools/sora_outfit_pack.js --kd-mesh (IMPL_ER59, ER73): model.glb with the bones of the outfit KineDriver unfolded']);
  }
  // NEW rows only: rows already in the sheet are kept as they are (a row of the same file is replaced)
  const sheetFile = path.join(ROOT, 'sheets/port/sora-assets.json');
  if (fs.existsSync(sheetFile) && rows.length) {
    const sheet = JSON.parse(fs.readFileSync(sheetFile, 'utf8'));
    const ids = new Set(rows.map((r) => r[0]));
    const all = [...sheet.rows.filter((r) => !ids.has(r[0])), ...rows];
    fs.writeFileSync(sheetFile, '{"sheet":"sora-assets","group":"port","columns":' + JSON.stringify(sheet.columns) + ',"rows":[' + NL + all.map((r) => JSON.stringify(r)).join(',' + NL) + NL + ']}' + NL);
    console.log(`sheets/port/sora-assets.json: ${rows.length} model_kd.glb rows`);
  }
}
function main() {
  if (args.includes('--kd-mesh')) return kdMeshes();
  const host = sp.buildSkeleton(sp.readGlb(meshFile('p_ex001')).json);
  const hostNames = new Set(host.map((b) => b.name));
  fs.mkdirSync(OUTFITS, { recursive: true });
  const index = [];
  const rows = [];
  for (const o of LIST) {
    const unique = pawnUnique(o.model) || 'SORA_KH3'; // (not serialized = the enum's value 0, SORA_KH3)
    if (unique !== o.unique) throw new Error(`${o.id}: the pawn ${o.model} says ${unique}, the list ${o.unique}`);
    const entry = { id: o.id, model: o.model, unique_id: o.unique, name: nameOf(o), text_key: o.text_key || null, text: o.text || null, pawn: `/Game/Blueprints/Player/${o.model}/${o.model}_Pawn`, main: !!o.main };
    if (o.main) { index.push({ ...entry, dir: null, wearform_set: '/Game/Blueprints/Player/p_ex001/p_ex001_WearformSet', wear_forms: [5], scale: 1 }); continue; }
    entry.dir = 'outfits/' + o.id;
    entry.wearform_set = wearformSet(o.model);
    { const mj = path.join(OUTFITS, o.id, 'materials/materials.json'); entry.wear_forms = fs.existsSync(mj) ? Object.keys(JSON.parse(fs.readFileSync(mj, 'utf8')).wear_forms || {}).map(Number).filter((k) => k > 0) : []; }
    const dir = path.join(OUTFITS, o.id);
    if (only && !only.includes(o.id)) { if (fs.existsSync(path.join(dir, 'manifest.json'))) index.push({ ...entry, ...JSON.parse(fs.readFileSync(path.join(dir, 'manifest.json'), 'utf8')).index }); continue; }
    fs.mkdirSync(dir, { recursive: true });
    const bones = sp.buildSkeleton(sp.readGlb(meshFile(o.model)).json);
    // (ER69) THE OUTFIT'S OWN SECONDARY MOTION: its SQEX_BonamikAsset (Character/pc/<model>/mdl/<model>_Bonamik), exported in
    // the schema of build/sora_pack/data/bonamik.json (tools/sora_weapon_pack.js bonamikExport) -> bonamik.json; the bones
    // it moves stay in the mesh's palette (crates/kh-er-render outfit.rs runs it on the outfit's pose each frame).
    let keep = new Set();
    { const content = `Character/pc/${o.model}/mdl/${o.model}_Bonamik`;
      const bnmFile = path.join(dir, 'bonamik.json');
      if (fs.existsSync(path.join(ROOT, 'assets_converted/packages', sp.CONTENT, content + '.uasset.json'))) {
        const sora = JSON.parse(fs.readFileSync(path.join(PACK, 'data/bonamik.json'), 'utf8'));
        const x = require('./sora_weapon_pack.js').bonamikExport(content, `Character/pc/${o.model}/mdl/${o.model}_0`, bones, sora);
        x._schema.about = `Sora's outfit ${o.id} (${o.model}) Bonamik data. Source: Content/${content}. Same schema as build/sora_pack/data/bonamik.json (asset part); written by tools/sora_outfit_pack.js through tools/sora_weapon_pack.js bonamikExport.`;
        fs.writeFileSync(bnmFile, JSON.stringify(x, null, 1) + String.fromCharCode(10));
        keep = new Set(x.bones.filter((b) => b.bonamik !== 'none').map((b) => b.index));
        entry.bonamik = { bodies: x.bodies.length, driven_bones: keep.size };
      } else if (fs.existsSync(bnmFile)) fs.unlinkSync(bnmFile);
    }
    let m;
    try { m = buildModel(o, bones, hostNames, keep); } catch (e) {
      // the palette is full: the followers (carried rigidly) are folded again, then the kinematic roots
      console.log('  ' + e.message + ': followers folded');
      const strict = new Set([...keep].filter((k) => !/follower/.test(JSON.parse(fs.readFileSync(path.join(dir, 'bonamik.json'), 'utf8')).bones[k].bonamik)));
      m = buildModel(o, bones, hostNames, strict);
    }
    { const a = hipHeight(bones), b = hipHeight(host); entry.scale = a && b ? +(a / b).toFixed(4) : 1; }
    fs.writeFileSync(path.join(dir, 'model.glb'), m.glb);
    const shared = bones.filter((b) => hostNames.has(b.name)).length;
    const manifest = {
      format: 'kh3-sora-outfit/1', generator: 'tools/sora_outfit_pack.js ' + VERSION, character: `Sora, outfit ${o.id} (${o.model}, ${o.unique})`,
      mesh_content: `Character/pc/${o.model}/mdl/${o.model}_0`, wearform_set: entry.wearform_set,
      skeleton: { source_asset: `/Game/Character/pc/${o.model}/mdl/${o.model}_0`, bone_count: bones.length, root: bones[0].name, node_index_equals_bone_index: true,
        note: 'the outfit\'s own reference pose (the main manifest\'s format). Bones that Sora\'s skeleton (p_ex001) also has take his animated rotation as a change from his reference, their translation from this skeleton; the others keep this pose relative to their parent (the renderer, crates/kh-er-render outfit.rs).',
        shared_with_sora: shared, only_in_outfit: bones.length - shared,
        bones: bones.map((b, i) => ({ index: i, name: b.name, parent: b.parent, translation: b.translation.map(sp.f32), rotation: b.rotation.map(sp.f32), scale: b.scale.map(sp.f32), translation_ue: b.ue.translation, rotation_ue: b.ue.rotation, scale_ue: b.ue.scale })) },
      materials: m.materials, items: [{ kind: 'mesh', file: 'model.glb' }], model_stats: m.stats,
      index: { model_bytes: m.glb.length },
    };
    fs.writeFileSync(path.join(dir, 'manifest.json'), JSON.stringify(manifest, null, 1) + '\n');
    console.log(`${o.id} (${o.model}): ${bones.length} bones (${shared} shared with Sora), palette ${m.stats.palette}, ${m.stats.folded_bones} folded, ${m.stats.vertices} vertices, ${m.stats.triangles} triangles, ${m.stats.vertices_cut_to_four} vertices cut to four influences (most ${m.stats.most_influences}), wear forms: ${entry.wearform_set || 'none'}`);
    if (!args.includes('--no-materials')) {
      const r = cp.spawnSync(process.execPath, [path.join(__dirname, 'sora_material_pack.js')], { env: { ...process.env, KH_OUTFIT_PACK: dir }, encoding: 'utf8', maxBuffer: 1 << 26 });
      process.stdout.write(r.stdout.split('\n').filter((l) => /outfit|NOT TRANSLATED|NOT DRAWN|stand-in|renumbered|missing|BAD|Error|error/.test(l)).map((l) => '  ' + l).join('\n') + '\n');
      if (r.status !== 0) throw new Error('sora_material_pack.js failed for ' + o.id + ': ' + r.stderr);
    }
    if (!args.includes('--no-shaders')) {
      const r = cp.spawnSync('cargo', ['run', '--release', '-q', '-p', 'kh-shaderc', '--', '--pack', dir, '--hlsl', path.join(ROOT, 'build/sora_tmp/er/er59/hlsl', o.id)], { cwd: ROOT, encoding: 'utf8', maxBuffer: 1 << 26 });
      const lines = (r.stdout + r.stderr).split('\n').filter((l) => /kh-shaderc:|error|failed/i.test(l));
      process.stdout.write(lines.map((l) => '  ' + l).join('\n') + '\n');
      if (r.status !== 0) throw new Error('kh-shaderc failed for ' + o.id);
    }
    index.push(entry);
  }
  // sizes on disk (what the DLL loads on demand: the model, the textures it binds, the shaders)
  for (const e of index) {
    if (!e.dir) continue;
    let bytes = 0, files = 0; const walk = (d) => { for (const f of fs.readdirSync(d, { withFileTypes: true })) { const p = path.join(d, f.name); if (f.isDirectory()) walk(p); else { bytes += fs.statSync(p).size; files++; } } };
    walk(path.join(PACK, e.dir)); e.disk_bytes = bytes; e.files = files;
    const dds = path.join(PACK, e.dir, 'materials/er/tex'); e.dds_bytes = fs.existsSync(dds) ? fs.readdirSync(dds).reduce((s, f) => s + fs.statSync(path.join(dds, f)).size, 0) : 0;
  }
  fs.writeFileSync(path.join(OUTFITS, 'outfits.json'), JSON.stringify({ format: 'kh3-sora-outfits/1', generator: 'tools/sora_outfit_pack.js ' + VERSION,
    note: 'Sora\'s outfits in KH3\'s data (ETresPlayerUniqueID -> pawn, FUN_140ea70b0). "main": the main pack (build/sora_pack). The order is the menu\'s.', outfits: index }, null, 1) + '\n');
  // rows of sheets/port/sora-assets.json (new rows; the rows of other folders are kept as they are)
  const sheetFile = path.join(ROOT, 'sheets/port/sora-assets.json');
  if (fs.existsSync(sheetFile)) {
    const sheet = JSON.parse(fs.readFileSync(sheetFile, 'utf8'));
    const walk = (d, prefix) => { for (const f of fs.readdirSync(d, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name, 'en'))) { const p = path.join(d, f.name); if (f.isDirectory()) { walk(p, prefix + f.name + '/'); continue; }
      if (!/^outfits\/[^/]+\/(model\.glb|manifest\.json|materials\/materials\.json)$|^outfits\/outfits\.json$/.test(prefix + f.name)) continue; // one row per outfit's main files (textures and shaders: listed in materials.json)
      const b = fs.readFileSync(p); const id = prefix + f.name; const o = index.find((e) => e.dir && id.startsWith(e.dir + '/'));
      rows.push([id, /\.glb$/.test(id) ? 'mesh' : 'data', '-', o ? `${sp.CONTENT}/Character/pc/${o.model}/mdl/${o.model}_0.uasset` : `${sp.CONTENT}/Blueprints/Player/p_ex001/p_ex001_Pawn.uasset`, '-', b.length, sha(b), '-', '-', '-', '-', 'read back by the renderer (crates/kh-er-render outfit.rs) and the offline harness', 'tools/sora_outfit_pack.js (IMPL_ER59): Sora\'s outfit models of ETresPlayerUniqueID (player pawn table FUN_140ea70b0)']); } };
    walk(OUTFITS, 'outfits/');
    const kept = sheet.rows.filter((r) => !/^outfits\//.test(r[0]));
    const all = [...kept, ...rows];
    fs.writeFileSync(sheetFile, '{"sheet":"sora-assets","group":"port","columns":' + JSON.stringify(sheet.columns) + ',"rows":[\n' + all.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
    console.log(`sheets/port/sora-assets.json: ${rows.length} rows of outfits/ (${all.length} rows in all)`);
  }
  for (const e of index) console.log(`  ${e.id.padEnd(13)} ${e.model} ${e.unique_id.padEnd(12)} ${e.dir ? ((e.disk_bytes / 1048576).toFixed(1) + ' MB on disk, ' + (e.dds_bytes / 1048576).toFixed(1) + ' MB block-compressed textures') : 'main pack'}`);
}
main();
