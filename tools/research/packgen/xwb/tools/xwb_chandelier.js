// (XW-b, 2026-10-16) CHANDELIER'S CHANDELIER: KH3's StylePawn w_so088 (/Game/Blueprints/StylePawn/w_so088, native
// ATresStylePawnSKChandelier; the accompany pawn of TRES_CMD_KIND_FINISH_SK in w_so080_Equip form 07) as NEW files under
// build/sora_tmp/er/xwb/pack/xsi/weapons/ (deployed to PACK/xsi/weapons/):
//   pawns_xwb.json              the pawn: model, its glb, skeleton, clip (its anim BP's one row M_7423), its clip's facts
//                               (length, the attack notify SCSK_CHANDELIER_END), the BP's m_Param_MaxTime, its material library
//   w_so088/weapon.glb, textures/, skeleton_xw.json, anims_xw/7423.glb   (as tools/xw_form_weapons.js writes a weapon; its
//                               one material slot is named w_so088_mat0: the mesh names it w_so087_mat0, a slot name XW's
//                               library already has)
//   materials_xwb/              its translated material (tools/sora_material_pack.js outfit mode, owner ids 410+, mesh
//                               xsi:w_so088); kh-shaderc builds materials_xwb/er from it
// Local build output from the user's own copy of the game; never distributed.
'use strict';
const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const ROOT = path.resolve(__dirname, '../../../../..');
const sp = require(path.join(ROOT, 'tools', 'sora_pack.js'));
const { CONTENT, sha256, readGlb, loadPkg, importPath, gamePathToContent, buildSkeleton, materialInfo, texturePng, GlbWriter, addSkeletonNodes, addMesh, loadAnimation, addAnimation, f32 } = sp;

const VERSION = '1.0.0';
const P = (...a) => path.join(...a);
const OUT = P(__dirname, '..', 'pack', 'xsi', 'weapons');
const MODEL = 'w_so088';
const BP = 'Blueprints/StylePawn/w_so088';
const ID_BASE = 410;
const written = [];
const write = (rel, buf) => { const p = P(OUT, rel); fs.mkdirSync(path.dirname(p), { recursive: true }); fs.writeFileSync(p, buf); written.push(rel); };
const pick = (tex, re) => Object.keys(tex || {}).find((k) => re.test(k));

function main() {
  // the blueprint: its mesh, its anim BP, its max time
  const bp = loadPkg(BP);
  const imports = bp.imports.map((im, i) => importPath(bp, -(i + 1)));
  const meshPath = imports.find((s) => /\/Character\/wep\/w_so088\/mdl\/w_so088_0$/.test(s));
  const animClass = imports.find((s) => /\/anm\/w_so088_Anim$/.test(s));
  const cdo = bp.exports.find((e) => /^Default__/.test(e.objectName));
  const props = Object.fromEntries((cdo.data.props || []).map((p) => [p.name, p.value]));
  const nativeClass = imports.find((s) => /TresStylePawnSKChandelier$/.test(s));
  if (!meshPath || !animClass) throw new Error('w_so088: no mesh / anim BP');
  // the anim BP's set and row
  const ab = loadPkg(gamePathToContent(animClass));
  const abCdo = ab.exports.find((e) => /^Default__/.test(e.objectName));
  const abProps = Object.fromEntries((abCdo.data.props || []).map((p) => [p.name, p.value]));
  const setPath = importPath(ab, abProps.MyAnimSets[0]).split('.')[0];
  const set = loadPkg(gamePathToContent(setPath));
  const rows = set.exports[0].data.props.find((p) => p.name === 'Anims').value.items.map((it) => ({ name: it.find((x) => x.name === 'AnimName').value, asset: importPath(set, it.find((x) => x.name === 'AnimData').value) }));
  if (rows.length !== 1) throw new Error('w_so088: expected one row, got ' + rows.length);
  // mesh, skeleton, glb (material slot renamed)
  const content = gamePathToContent(meshPath);
  const meshGlb = readGlb(P(ROOT, 'assets_converted', 'meshes', CONTENT, content + '.glb'));
  const bones = buildSkeleton(meshGlb.json);
  const textures = new Map();
  const mats = meshGlb.json.materials.map((m, slot) => {
    const gp = m.extras && m.extras.ue_material; const info = gp ? materialInfo(gp) : { tex: {} };
    const name = `${MODEL}_mat${slot}`;
    const gl = { name, pbrMetallicRoughness: { metallicFactor: 0, roughnessFactor: 0.8, baseColorFactor: [0.6, 0.6, 0.6, 1] }, extras: { ue_material: gp || null, ue_root_material: info.root_material || null, source_slot_name: m.name } };
    const own = [];
    const base = pick(info.tex, /base.?colou?r|diffuse|albedo|^color/i);
    if (base) { const t = texturePng(info.tex[base], 'color'); own.push(t); textures.set(t.file, t); gl.pbrMetallicRoughness.baseColorTexture = { index: t.file }; delete gl.pbrMetallicRoughness.baseColorFactor; }
    const normal = pick(info.tex, /normal/i);
    if (normal) { const t = texturePng(info.tex[normal], 'normal'); own.push(t); textures.set(t.file, t); gl.normalTexture = { index: t.file }; }
    if (info.two_sided) gl.doubleSided = true;
    return { gltf: gl, textures: own, rec: { slot, slot_name: name, source_slot_name: m.name, material_instance: gp || null } };
  });
  const w = new GlbWriter(`(XW-b) via build/sora_tmp/er/xwb/tools/xwb_chandelier.js ${VERSION}`); addSkeletonNodes(w, bones);
  const r = addMesh(w, meshGlb, bones, mats.map((m) => ({ gltf: JSON.parse(JSON.stringify(m.gltf)), textures: m.textures, rec: m.rec })));
  w.json.skins[0].name = MODEL; w.json.meshes[0].name = MODEL + '_LOD0'; w.json.nodes[r.node].name = MODEL + '_mesh'; w.json.scenes = [{ name: MODEL, nodes: [0, r.node] }];
  const glb = w.finish();
  write(`${MODEL}/weapon.glb`, glb);
  for (const t of textures.values()) write(`${MODEL}/${t.file}`, t.png);
  write(`${MODEL}/skeleton_xw.json`, Buffer.from(JSON.stringify({ format: 'kh3-sora-weapon-skeleton-xw/1', source_mesh: meshPath, note: 'kh_anim::Skeleton::from_manifest reads skeleton.bones',
    skeleton: { bone_count: bones.length, root: bones[0].name, bones: bones.map((b, i) => ({ index: i, name: b.name, parent: b.parent, translation: b.translation.map(f32), rotation: b.rotation.map(f32), scale: b.scale.map(f32), translation_ue: b.ue.translation, rotation_ue: b.ue.rotation, scale_ue: b.ue.scale })) } }, null, 1) + '\n'));
  // the clip
  const a = loadAnimation(rows[0].asset, bones);
  const clip = path.basename(gamePathToContent(rows[0].asset));
  const cw = new GlbWriter(`(XW-b) via build/sora_tmp/er/xwb/tools/xwb_chandelier.js ${VERSION}`); addSkeletonNodes(cw, bones); addAnimation(cw, clip, a); cw.json.scenes = [{ name: clip, nodes: [0] }];
  write(`${MODEL}/anims_xw/${clip}.glb`, cw.finish());
  const attack = a.notifies.find((n) => n.class === 'TresAnimNotifyState_Attack');
  const effects = a.notifies.filter((n) => /AttachEffect/.test(n.class)).map((n) => ({ time: +n.trigger_time.toFixed(4), end: n.end_trigger_time != null ? +n.end_trigger_time.toFixed(4) : null, effect: (JSON.stringify(n.properties).match(/vw_so088_[a-z0-9_]+/) || [null])[0] }));
  const m = /"importedBounds":\[([^\]]+)\]/.exec(fs.readFileSync(P(ROOT, 'assets_converted/meshes', CONTENT, content + '.mesh.json'), 'utf8'));
  const b = m ? m[1].split(',').map(Number) : null;
  const facts = b && b.length === 7 ? { bone_params: [], bounds: { origin: b.slice(0, 3), extent: b.slice(3, 6), radius: b[6] } } : null;
  // the material (outfit mode, as tools/xw_form_weapons.js)
  const stage = P(__dirname, '..', 'stage', 'pawnmat');
  fs.rmSync(stage, { recursive: true, force: true }); fs.mkdirSync(stage, { recursive: true });
  fs.writeFileSync(P(stage, 'model_stage.glb'), glb);
  fs.writeFileSync(P(stage, 'manifest.json'), JSON.stringify({ format: 'xwb-stage', materials: mats.map((x) => ({ slot: x.rec.slot, slot_name: x.rec.slot_name, material_instance: x.rec.material_instance })), items: [{ kind: 'mesh', file: 'model_stage.glb' }], mesh_content: content, wearform_set: null }));
  const res = cp.spawnSync(process.execPath, [P(ROOT, 'tools', 'sora_material_pack.js')], { env: { ...process.env, KH_OUTFIT_PACK: stage, KH_XSI_WEAPONS: '1' }, encoding: 'utf8', maxBuffer: 1 << 28 });
  if (res.status !== 0) throw new Error('sora_material_pack.js failed: ' + res.stderr + (res.stdout || '').slice(-3000));
  for (const l of (res.stdout || '').split('\n').filter((l) => l.trim())) console.log('  [material pack] ' + l);
  const json = JSON.parse(fs.readFileSync(P(stage, 'materials', 'materials.json'), 'utf8')); const wgsl = fs.readFileSync(P(stage, 'materials', 'materials.wgsl'), 'utf8');
  for (const [k, v] of Object.entries(json.materials)) { if (v.mesh === 'kingdom_key') delete json.materials[k]; else v.mesh = `xsi:${MODEL}`; }
  const used = new Set(Object.values(json.materials).map((v) => v.owner)); const idMap = new Map();
  for (const [name, o] of Object.entries(json.owners)) { if (!used.has(name)) { delete json.owners[name]; continue; } idMap.set(o.id, ID_BASE + o.id); o.id = ID_BASE + o.id; }
  for (const v of Object.values(json.materials)) v.id = json.owners[v.owner].id;
  const kept = [...wgsl.matchAll(/#ifdef KH_C(\d+)\n([\s\S]*?)\n#endif\n/g)].filter((x) => idMap.has(+x[1])).map((x) => `#ifdef KH_C${idMap.get(+x[1])}\n${x[2]}\n#endif\n`);
  json.uv2 = {}; json.mesh_facts = facts ? { [`xsi:${MODEL}`]: facts } : {}; json.khs = null; json.wear_forms = null;
  json.xsi = { generator: `build/sora_tmp/er/xwb/tools/xwb_chandelier.js ${VERSION}`, id_base: ID_BASE, note: 'XW-b: the chandelier pawn w_so088\'s translated material, merged into Sora\'s library by the hosts (mesh xsi:w_so088)' };
  const texFiles = new Set(); for (const v of Object.values(json.materials)) for (const t of v.textures || []) if (t && t.file) texFiles.add(t.file);
  write('materials_xwb/materials.json', Buffer.from(JSON.stringify(json)));
  write('materials_xwb/materials.wgsl', Buffer.from(`// GENERATED by build/sora_tmp/er/xwb/tools/xwb_chandelier.js from tools/sora_material_pack.js (outfit mode) for the chandelier pawn w_so088: owner ids ${ID_BASE}+. Do not edit.\n\n` + kept.join('\n')));
  for (const f of texFiles) write(`materials_xwb/${f}`, fs.readFileSync(P(stage, 'materials', f)));
  for (const [k, v] of Object.entries(json.materials)) console.log(`material ${k}: owner ${v.owner} (KH_C${v.id}) ${v.blend_mode}${v.stand_in_for ? ' STAND-IN' : ''}; textures ${v.textures.filter(Boolean).length}`);
  const table = { format: 'kh3-sora-xsi-pawns-xwb/1', generator: `build/sora_tmp/er/xwb/tools/xwb_chandelier.js ${VERSION}`, legal: 'local build output from the user\'s own copy of the game; never distributed',
    note: 'XW-b: the accompany pawns of the XS moves the hosts draw (kh-core Sora::xs_chandelier_pawn gives where and its clip time)', materials: 'materials_xwb/materials.json',
    pawns: [{ move: 'chandelier', model: MODEL, blueprint: `/Game/${BP}`, native_class: nativeClass || null, file: `${MODEL}/weapon.glb`, sha256: sha256(glb), skeleton: `${MODEL}/skeleton_xw.json`, material_slots: mats.map((x) => x.rec),
      clip: { file: `${MODEL}/anims_xw/${clip}.glb`, row: rows[0].name, asset: rows[0].asset, anim_set: setPath, length_s: a.length, frames: a.numFrames, attack: attack ? { row: (JSON.stringify(attack.properties).match(/SC[A-Z]{2}_[A-Z_]+/) || [null])[0], start: +attack.trigger_time.toFixed(4), end: +attack.end_trigger_time.toFixed(4) } : null, effects },
      max_time_s: props.m_Param_MaxTime ?? 30, invincible: !!props.m_bIsInvincible }] };
  write('pawns_xwb.json', Buffer.from(JSON.stringify(table, null, 1) + '\n'));
  console.log(`${written.length} new file(s) under ${path.relative(ROOT, OUT)}; clip ${clip} ${a.length.toFixed(4)} s, attack ${JSON.stringify(table.pawns[0].clip.attack)}, max time ${table.pawns[0].max_time_s}`);
}
main();
