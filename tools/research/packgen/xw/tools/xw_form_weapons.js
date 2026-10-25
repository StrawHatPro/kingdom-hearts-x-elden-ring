// (XW, 2026-10-16) THE XS MOVES' FORM WEAPONS AS KH3 HAS THEM: their own skeletons, their own clips, their own materials.
// ER90 left them approximate (the Kingdom Key's material with their maps, bind pose). This tool writes NEW files only, under
// build/sora_tmp/er/xw/pack/xsi/weapons/ (deployed to PACK/xsi/weapons/; nothing of XSI's or ER90's staged files changes):
//
//   weapons_xw.json              per move: the form KH3 equips for the clip's anim set and its weapons (model, socket, bone,
//                                socket transform), each weapon's skeleton and clip, the materials library
//   <model>/weapon.glb, textures/ the weapons XSI's table did not have (Chandelier's four: see below), as tools/xsi_weapons.js
//                                writes them (the Kingdom Key's conventions)
//   <model>/skeleton_xw.json     the weapon skeleton in the manifest format kh_anim::Skeleton::from_manifest reads
//   <model>/anims_xw/<clip>.glb  the weapon's own clip for the move, in the pack's conventions (kh_anim::Clip::load)
//   materials/                   the weapons' translated materials (tools/sora_material_pack.js outfit mode, as
//                                tools/party_pack.js runs it), owner ids 400+, meshes `xsi:<model>`: materials.json,
//                                materials.wgsl, textures/, uv2_*.bin; kh-shaderc builds materials/er/ from it
//
// THE FORM (read from the game data, not authored): the equip data's m_EquipMap entry whose m_ReplaceAnimSets holds the anim
// set that has the move's logical name (Sora plays M_3416 / M_7419 / M_9419 from p_ex001_AnimSet3 / 7 / 9):
//   Giant Stamp  w_so050_Equip TRES_WEAPON_FORM_03 (AnimSet3): w_so052 on R_wep4_skt
//   Chandelier   w_so080_Equip TRES_WEAPON_FORM_07 (AnimSet7): w_so084 R_ude2_wep2_skt, w_so085 L_ude2_wep2_skt,
//                w_so086 R_ashi2_wep_skt, w_so087 L_ashi2_wep_skt (XSI's table took entry 1 = TRES_WEAPON_FORM_12 =
//                AnimSet12, the CL set: w_so082 / w_so083, which Chandelier's clip does not belong to)
//   Myriad       w_so190_Equip TRES_WEAPON_FORM_09 (AnimSet9): w_so192 on R_wep_skt
// THE CLIP: the weapon's anim instance (its AnimBlueprint's MyAnimSets) plays the row of the same logical name; a weapon
// whose set has no such row shows its m_DefaultAnimName row (ResetPose) [inferred from the data: the anim BPs' default].
// Local build output from the user's own copy of the game; never distributed.
//
//   node build/sora_tmp/er/xw/tools/xw_form_weapons.js [--no-materials]
'use strict';
const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const ROOT = path.resolve(__dirname, '../../../../..');
const sp = require(path.join(ROOT, 'tools', 'sora_pack.js'));
const { CONTENT, sha256, readGlb, readAccessor, loadPkg, importPath, gamePathToContent, buildSkeleton, materialInfo, texturePng, GlbWriter, addSkeletonNodes, addMesh, loadAnimation, addAnimation, f32 } = sp;

const VERSION = '1.0.0';
const P = (...a) => path.join(...a);
const OUT = P(__dirname, '..', 'pack', 'xsi', 'weapons');
const XSI = P(ROOT, 'build', 'sora_tmp', 'xsi', 'pack', 'xsi', 'weapons');
const ID_BASE = 400; // (300+ is the party team props' library: PACK/party/team)
const NO_MATERIALS = process.argv.includes('--no-materials');
const MOVES = [
  { move: 'giant_stamp', clip: '3416', logical: 'M_3416', equip: 'Blueprints/Player/p_ex001/w_so050_Equip' },
  { move: 'chandelier', clip: '7419', logical: 'M_7419', equip: 'Blueprints/Player/p_ex001/w_so080_Equip' },
  { move: 'myriad', clip: '9419', logical: 'M_9419', equip: 'Blueprints/Player/p_ex001/w_so190_Equip' },
];

const written = [];
function write(rel, buf) {
  const p = P(OUT, rel);
  if (fs.existsSync(P(XSI, rel))) throw new Error('XSI has this file already (new files only): ' + rel);
  fs.mkdirSync(path.dirname(p), { recursive: true });
  fs.writeFileSync(p, buf);
  written.push(rel);
}
const findAll = (v, name, out = []) => { if (Array.isArray(v)) v.forEach((x) => findAll(x, name, out)); else if (v && typeof v === 'object') { if (v.name === name && 'value' in v) out.push(v); for (const k of Object.keys(v)) findAll(v[k], name, out); } return out; };
const first = (v, name) => findAll(v, name)[0];

function animSetRows(gamePath) { // TresAnimSet -> [{ name, asset }]
  const d = loadPkg(gamePathToContent(gamePath));
  const items = d.exports[0].data.props.find((p) => p.name === 'Anims').value.items;
  return items.map((it) => ({ name: it.find((x) => x.name === 'AnimName').value, asset: importPath(d, it.find((x) => x.name === 'AnimData').value) }));
}
function formOf(mv) {
  const d = loadPkg(mv.equip);
  const data = d.exports.find((e) => /TresEquip.*Data/.test(e.class)).data.props;
  const entries = data.find((p) => p.name === 'm_EquipMap').value.entries;
  for (const [index, en] of entries.entries()) {
    const sets = findAll(en, 'm_ReplaceAnimSets').flatMap((p) => p.value).map((v) => importPath(d, v));
    for (const set of sets) {
      const row = animSetRows(set).find((r) => r.name === mv.logical);
      if (!row) continue;
      if (path.basename(gamePathToContent(row.asset)) !== mv.clip) throw new Error(`${mv.logical} of ${set} is ${row.asset}, not ${mv.clip}`);
      const units = first(en, 'm_Equips').value.items.map((u) => ({ unit: first(u, 'UnitName').value, part: first(u, 'm_AttachPart').value, socket: first(u, 'AttachBoneName').value, blueprint: importPath(d, first(u, 'RsrcData').value).replace(/_C$/, '') }));
      return { index, form: en.key, anim_set: set, units };
    }
  }
  throw new Error('no form of ' + mv.equip + ' has ' + mv.logical);
}
function importsOf(gamePath) { const d = loadPkg(gamePathToContent(gamePath)); return d.imports.map((im, i) => importPath(d, -(i + 1))); }
function meshOf(blueprint) { const m = JSON.stringify(importsOf(blueprint)).match(/\/Game\/Character\/wep\/[A-Za-z0-9_]+\/mdl\/[A-Za-z0-9_]+_0\b/); if (!m) throw new Error('no mesh in ' + blueprint); return m[0]; }
function animClassOf(blueprint) { const m = JSON.stringify(importsOf(blueprint)).match(/\/Game\/Character\/wep\/[A-Za-z0-9_]+\/anm\/[A-Za-z0-9_]+_Anim\b/); if (!m) throw new Error('no anim BP in ' + blueprint); return m[0]; }
function animSetsOf(animClass) {
  const d = loadPkg(gamePathToContent(animClass));
  const cdo = d.exports.find((e) => /^Default__/.test(e.objectName));
  const p = Object.fromEntries((cdo.data.props || []).map((x) => [x.name, x.value]));
  return { sets: (p.MyAnimSets || []).map((v) => importPath(d, v).split('.')[0]), default_anim: p.m_DefaultAnimName || null };
}
function socketsOfSora() {
  const d = loadPkg('Character/pc/p_ex001/mdl/p_ex001_0_Skeleton');
  const out = {};
  for (const e of d.exports) {
    if (e.class !== 'SkeletalMeshSocket') continue;
    const p = Object.fromEntries((e.data.props || []).map((x) => [x.name, x.value]));
    out[p.SocketName] = { bone: p.BoneName, location_ue: p.RelativeLocation || { x: 0, y: 0, z: 0 }, rotation_ue: p.RelativeRotation || { pitch: 0, yaw: 0, roll: 0 }, scale_ue: p.RelativeScale || { x: 1, y: 1, z: 1 } };
  }
  return out;
}
const pick = (tex, re) => Object.keys(tex || {}).find((k) => re.test(k));
// the glTF of a weapon XSI's table lacks: as tools/xsi_weapons.js writes it
function weaponGlb(model, meshGlb, bones) {
  const textures = new Map();
  const mats = meshGlb.json.materials.map((m, slot) => {
    const gp = m.extras && m.extras.ue_material; const info = gp ? materialInfo(gp) : { tex: {} };
    const gl = { name: m.name, pbrMetallicRoughness: { metallicFactor: 0, roughnessFactor: 0.8, baseColorFactor: [0.6, 0.6, 0.6, 1] }, extras: { ue_material: gp || null, ue_root_material: info.root_material || null } };
    const own = [];
    const base = pick(info.tex, /base.?colou?r|diffuse|albedo|^color/i);
    if (base) { const t = texturePng(info.tex[base], 'color'); own.push(t); textures.set(t.file, t); gl.pbrMetallicRoughness.baseColorTexture = { index: t.file }; delete gl.pbrMetallicRoughness.baseColorFactor; }
    const normal = pick(info.tex, /normal/i);
    if (normal) { const t = texturePng(info.tex[normal], 'normal'); own.push(t); textures.set(t.file, t); gl.normalTexture = { index: t.file }; }
    if (info.two_sided) gl.doubleSided = true;
    return { gltf: gl, textures: own, rec: { slot, slot_name: m.name, material_instance: gp || null, base_color_parameter: base || null, normal_parameter: normal || null } };
  });
  const w = new GlbWriter(`(XW) via build/sora_tmp/er/xw/tools/xw_form_weapons.js ${VERSION}`); addSkeletonNodes(w, bones);
  const r = addMesh(w, meshGlb, bones, mats.map((m) => ({ gltf: JSON.parse(JSON.stringify(m.gltf)), textures: m.textures, rec: m.rec })));
  w.json.skins[0].name = model; w.json.meshes[0].name = model + '_LOD0'; w.json.nodes[r.node].name = model + '_mesh'; w.json.scenes = [{ name: model, nodes: [0, r.node] }];
  return { glb: w.finish(), textures, stats: r.stats, materials: mats.map((m) => m.rec) };
}
function meshFactsOf(content) {
  const p = P(ROOT, 'assets_converted/meshes', CONTENT, content + '.mesh.json'); if (!fs.existsSync(p)) return null;
  const m = /"importedBounds":\[([^\]]+)\]/.exec(fs.readFileSync(p, 'utf8')); const b = m ? m[1].split(',').map(Number) : null;
  return b && b.length === 7 ? { source: 'assets_converted/meshes/' + CONTENT + '/' + content + '.mesh.json', bone_params: [], bounds: { origin: b.slice(0, 3), extent: b.slice(3, 6), radius: b[6] } } : null;
}

function main() {
  fs.mkdirSync(OUT, { recursive: true });
  const sockets = socketsOfSora();
  const index = { format: 'kh3-sora-xsi-weapons-xw/1', generator: `build/sora_tmp/er/xw/tools/xw_form_weapons.js ${VERSION}`, legal: 'local build output from the user\'s own copy of the game; never distributed',
    note: 'XW: per XS move, the form KH3 equips for the anim set of its clip (m_ReplaceAnimSets), its weapons on Sora\'s sockets, each weapon\'s skeleton (skeleton_xw.json) and its own clip for the move (anims_xw/; the default row ResetPose when its set has no row of that name), the translated materials (materials/, owner ids 400+, meshes xsi:<model>). Supersedes weapons.json (XSI) and er_slots.json (ER90) where read.',
    materials: NO_MATERIALS ? null : 'materials/materials.json', moves: [] };
  const models = new Map(); // model -> { content, bones, glbRel, materials, uv2 }
  for (const mv of MOVES) {
    const form = formOf(mv);
    const weapons = [];
    for (const u of form.units) {
      if (!u.socket || u.socket === 'None') continue;
      const meshPath = meshOf(u.blueprint); const content = gamePathToContent(meshPath); const model = path.basename(content).replace(/_0$/, '');
      const meshGlb = readGlb(P(ROOT, 'assets_converted', 'meshes', CONTENT, content + '.glb'));
      const bones = buildSkeleton(meshGlb.json);
      if (!models.has(model)) {
        const rec = { content, bones, meshGlb };
        // the glb: XSI's where it has one (same tool code: tools/xsi_weapons.js), else a new one
        if (fs.existsSync(P(XSI, model, 'weapon.glb'))) {
          rec.glbRel = `${model}/weapon.glb`; rec.glbFrom = 'xsi'; rec.glbFile = P(XSI, model, 'weapon.glb');
          rec.materials = readGlb(rec.glbFile).json.materials.map((m, slot) => ({ slot, slot_name: m.name, material_instance: m.extras && m.extras.ue_material }));
        } else {
          const g = weaponGlb(model, meshGlb, bones);
          write(`${model}/weapon.glb`, g.glb);
          for (const t of g.textures.values()) write(`${model}/${t.file}`, t.png);
          rec.glbRel = `${model}/weapon.glb`; rec.glbFrom = 'xw'; rec.glbFile = P(OUT, model, 'weapon.glb'); rec.materials = g.materials; rec.stats = g.stats;
        }
        // the skeleton in the manifest format of kh_anim (the *_ue values: cm, the converted glb's own)
        const skel = { format: 'kh3-sora-weapon-skeleton-xw/1', source_mesh: meshPath, note: 'kh_anim::Skeleton::from_manifest reads skeleton.bones (translation_ue cm, rotation_ue xyzw, scale_ue); bone order = the mesh\'s, a parent before its children',
          skeleton: { bone_count: bones.length, root: bones[0].name, bones: bones.map((b, i) => ({ index: i, name: b.name, parent: b.parent, translation: b.translation.map(f32), rotation: b.rotation.map(f32), scale: b.scale.map(f32), translation_ue: b.ue.translation, rotation_ue: b.ue.rotation, scale_ue: b.ue.scale })) } };
        write(`${model}/skeleton_xw.json`, Buffer.from(JSON.stringify(skel, null, 1) + '\n'));
        rec.facts = meshFactsOf(content);
        models.set(model, rec);
      }
      const rec = models.get(model);
      // the weapon's clip for the move
      const animClass = animClassOf(u.blueprint);
      const { sets, default_anim } = animSetsOf(animClass);
      let row = null, set = null, why = null;
      for (const s of sets) { const rows = animSetRows(s); const r = rows.find((x) => x.name === mv.logical); if (r) { row = r; set = s; why = 'the row of the move\'s logical name'; break; } }
      if (!row) for (const s of sets) { const rows = animSetRows(s); const r = rows.find((x) => x.name === default_anim); if (r) { row = r; set = s; why = `no row ${mv.logical}: the anim BP's m_DefaultAnimName ${default_anim}`; break; } }
      if (!row) throw new Error(`${model}: no clip for ${mv.logical} nor ${default_anim}`);
      // (a clip of another skeleton asset the mesh shares: w_so192 plays the Kingdom Key set w_so010_AnimSet; UE binds the
      // tracks by bone name: those of bones this mesh does not have are dropped)
      const extra = []; { const g = readGlb(P(ROOT, "assets_converted", "animations", CONTENT, gamePathToContent(row.asset) + ".glb")); for (const c of g.json.animations[0].channels) { const n = g.json.nodes[c.target.node].name; if (!rec.bones.some((b) => b.name === n) && !extra.some((b) => b.name === n)) extra.push({ name: n }); } }
      const a = loadAnimation(row.asset, [...rec.bones, ...extra]);
      const dropped = [...new Set(a.chans.filter((c) => c.bone >= rec.bones.length).map((c) => extra[c.bone - rec.bones.length].name))];
      a.chans = a.chans.filter((c) => c.bone < rec.bones.length);
      const clipName = path.basename(gamePathToContent(row.asset));
      const clipRel = `${model}/anims_xw/${clipName}.glb`;
      if (!written.includes(clipRel)) {
        const w = new GlbWriter(`(XW) via build/sora_tmp/er/xw/tools/xw_form_weapons.js ${VERSION}`);
        addSkeletonNodes(w, rec.bones); addAnimation(w, clipName, a); w.json.scenes = [{ name: clipName, nodes: [0] }];
        write(clipRel, w.finish());
      }
      const sock = sockets[u.socket];
      if (!sock) throw new Error('no socket ' + u.socket + ' on Sora');
      const moving = a.chans.filter((c) => { const n = c.path === 'rotation' ? 4 : 3; for (let i = n; i < c.values.length; i++) if (Math.abs(c.values[i] - c.values[i % n]) > 1e-4) return true; return false; }).map((c) => `${rec.bones[c.bone].name}.${c.path}`);
      weapons.push({ model, unit: u.unit, file: rec.glbRel, glb_from: rec.glbFrom, sha256: sha256(fs.readFileSync(rec.glbFile)), source_mesh: meshPath, blueprint: u.blueprint, anim_class: animClass, attach_part: u.part, socket: u.socket, bone: sock.bone,
        socket_location_ue: sock.location_ue, socket_rotation_ue: sock.rotation_ue, socket_scale_ue: sock.scale_ue, skeleton: `${model}/skeleton_xw.json`,
        clip: { file: clipRel, sequence: clipName, asset: row.asset, anim_set: set, why, dropped_tracks_of_bones_not_in_this_mesh: dropped, duration_s: a.length, frames: a.numFrames, moving_channels: moving } });
      console.log(`${mv.move}: ${model} on ${u.socket} (bone ${sock.bone}), clip ${clipName} (${why}; ${a.length.toFixed(3)} s, ${moving.length} moving channels${moving.length ? ': ' + moving.join(' ') : ''})`);
    }
    index.moves.push({ move: mv.move, clip: mv.clip, logical: mv.logical, equip: `/Game/${mv.equip}`, form: form.form, form_entry: form.index, anim_set: form.anim_set, weapons });
  }
  if (!NO_MATERIALS) index.materials_report = buildMaterials(models);
  write('weapons_xw.json', Buffer.from(JSON.stringify(index, null, 1) + '\n'));
  console.log(`${written.length} new file(s) under ${path.relative(ROOT, OUT)}`);
}

// THE MATERIALS: tools/sora_material_pack.js in outfit mode over a staged folder whose manifest lists every slot of the
// form weapons (as tools/party_pack.js does for the friends' weapons); then slot -> mesh `xsi:<model>`, the Kingdom Key's
// records dropped, owner ids ID_BASE + n, the post-process function left out (Sora's library has it), the third UV sets.
function buildMaterials(models) {
  const stage = P(__dirname, '..', 'stage', 'xsimat');
  fs.rmSync(stage, { recursive: true, force: true }); fs.mkdirSync(stage, { recursive: true });
  const slots = []; const meshOfSlot = new Map();
  for (const [model, rec] of models) for (const m of rec.materials) {
    if (meshOfSlot.has(m.slot_name)) throw new Error('slot name used by two models: ' + m.slot_name);
    meshOfSlot.set(m.slot_name, `xsi:${model}`); slots.push({ slot: m.slot, slot_name: m.slot_name, material_instance: m.material_instance });
  }
  const firstModel = [...models.values()][0];
  fs.copyFileSync(firstModel.glbFile, P(stage, 'model_stage.glb'));
  fs.writeFileSync(P(stage, 'manifest.json'), JSON.stringify({ format: 'xw-stage', materials: slots, items: [{ kind: 'mesh', file: 'model_stage.glb' }], mesh_content: firstModel.content, wearform_set: null }));
  const r = cp.spawnSync(process.execPath, [P(ROOT, 'tools', 'sora_material_pack.js')], { env: { ...process.env, KH_OUTFIT_PACK: stage, KH_XSI_WEAPONS: "1" }, encoding: 'utf8', maxBuffer: 1 << 28 });
  if (r.status !== 0) throw new Error('sora_material_pack.js failed: ' + r.stderr + (r.stdout || '').slice(-3000));
  const lines = (r.stdout || '').split('\n').filter((l) => l.trim());
  for (const l of lines) console.log('  [material pack] ' + l);
  const jf = P(stage, 'materials', 'materials.json'), wf = P(stage, 'materials', 'materials.wgsl');
  const json = JSON.parse(fs.readFileSync(jf, 'utf8')); const wgsl = fs.readFileSync(wf, 'utf8');
  for (const [k, v] of Object.entries(json.materials)) { if (v.mesh === 'kingdom_key') delete json.materials[k]; else v.mesh = meshOfSlot.get(k) || v.mesh; }
  const used = new Set(Object.values(json.materials).map((v) => v.owner));
  const idMap = new Map();
  for (const [name, o] of Object.entries(json.owners)) { if (!used.has(name)) { delete json.owners[name]; continue; } idMap.set(o.id, ID_BASE + o.id); o.id = ID_BASE + o.id; }
  for (const v of Object.values(json.materials)) v.id = json.owners[v.owner].id;
  const blocks = [...wgsl.matchAll(/#ifdef KH_C(\d+)\n([\s\S]*?)\n#endif\n/g)];
  const kept = blocks.filter((b) => idMap.has(+b[1])).map((b) => `#ifdef KH_C${idMap.get(+b[1])}\n${b[2]}\n#endif\n`);
  const outWgsl = `// GENERATED by build/sora_tmp/er/xw/tools/xw_form_weapons.js from tools/sora_material_pack.js (outfit mode) for the XS form weapons: owner ids ${ID_BASE}+. Do not edit.\n\n` + kept.join('\n');
  // the third UV sets of the weapon primitives (Bevy meshes carry two): f32 pairs in vertex order
  const uv2 = {}; const uvFiles = [];
  for (const [model, rec] of models) {
    const g = readGlb(rec.glbFile);
    g.json.meshes[0].primitives.forEach((p, k) => {
      if (p.attributes.TEXCOORD_2 === undefined) return;
      const a = readAccessor(g, p.attributes.TEXCOORD_2); const buf = Buffer.alloc(a.data.length * 4); a.data.forEach((v, i) => buf.writeFloatLE(v, i * 4));
      const name = `uv2_${model}_${k}.bin`; uvFiles.push([name, buf]);
      uv2[`xsi:${model}:${g.json.materials[p.material].name}`] = { file: name, vertices: a.data.length / 2, primitive: k };
    });
  }
  json.uv2 = uv2;
  json.mesh_facts = Object.fromEntries([...models].filter(([, rec]) => rec.facts).map(([model, rec]) => [`xsi:${model}`, rec.facts]));
  json.khs = null; json.wear_forms = null;
  json.xsi = { generator: `build/sora_tmp/er/xw/tools/xw_form_weapons.js ${VERSION}`, id_base: ID_BASE, note: 'XW: the XS form weapons\' translated materials, merged into Sora\'s library by kh-app char_material.rs / kh-er-render xsiweapon.rs (mesh xsi:<model>); texture and uv2 files relative to this folder' };
  // the textures the kept records name (the Kingdom Key's are left out)
  const texFiles = new Set(); for (const v of Object.values(json.materials)) for (const t of v.textures || []) if (t && t.file) texFiles.add(t.file);
  write('materials/materials.json', Buffer.from(JSON.stringify(json)));
  write('materials/materials.wgsl', Buffer.from(outWgsl));
  for (const f of texFiles) write(`materials/${f}`, fs.readFileSync(P(stage, 'materials', f)));
  for (const [name, buf] of uvFiles) write(`materials/${name}`, buf);
  const owners = Object.values(json.owners); const ok = owners.filter((o) => o.base && o.base.ok).length;
  const standIns = Object.entries(json.materials).filter(([, v]) => v.stand_in_for).map(([k, v]) => `${k} (${v.owner})`);
  const report = `${Object.keys(json.materials).length} material slots, ${owners.length} shader owners, ${ok} translated; stand-ins: ${standIns.join(', ') || 'none'}; ${texFiles.size} textures; ${uvFiles.length} uv2 files`;
  console.log('materials: ' + report);
  for (const [k, v] of Object.entries(json.materials)) console.log(`  ${k.padEnd(22)} ${v.mesh.padEnd(12)} owner ${v.owner} (KH_C${v.id}) ${v.blend_mode}${v.two_sided ? ' two-sided' : ''}${v.stand_in_for ? ' STAND-IN for ' + v.stand_in_for : ''}`);
  return { report, lines: lines.filter((l) => /NOT TRANSLATED|NOT DRAWN|stand-in|renumbered|missing|BAD/.test(l)) };
}
main();
