// (XSI, 2026-10-16) THE FORM WEAPONS OF THE XS MOVES (build/sora_tmp/party/XS_SORA_ABILITIES.md 2.6, 2.7, 2.14): the
// skeletal meshes KH3 puts in Sora's hands for the three form-weapon moves, as glTF in the conventions of the Kingdom Key
// (tools/sora_weapon_pack.js: metres, +Y up, right-handed, LOD 0, one skin, PNG base colour / normal textures).
//
//   node tools/xsi_weapons.js        write build/sora_tmp/xsi/pack/xsi/weapons/<model>/{weapon.glb,textures/*.png} and
//                                    build/sora_tmp/xsi/pack/xsi/weapons/weapons.json (deployed to PACK/xsi/weapons)
//
// Which mesh, on which socket of Sora's skeleton: the equip data of the keyblade whose form the clip belongs to
// (TresEquipKeybladeData m_EquipMap[form].m_List[0].m_Equips[]: RsrcData = the weapon blueprint, AttachBoneName = the
// socket), the blueprint's TresWeaponMesh0.SkeletalMesh. Read from the game data, not authored:
//   Giant Stamp (clip 3416, AnimSet3 Hyper Hammer):    w_so050_Equip form 1: w_so052 on R_wep4_skt
//   Chandelier  (clip 7419, AnimSet7 Blizzard Blades): w_so080_Equip form 1: w_so082 on R_wep8_skt, w_so083 on L_wep8_skt
//   Myriad      (clip 9419, AnimSet9 Ultimate Form):   w_so190_Equip form 1: w_so192 on R_wep_skt
// Sockets: p_ex001_0_Skeleton's SkeletalMeshSocket exports (bone, relative rotation). Materials: base colour and normal
// texture of each slot by parameter name (our approximation, as the Kingdom Key's MATERIAL_RULES; the test app may
// replace them with its translated game shaders). Local build output from the user's own copy; never distributed.
'use strict';
const fs = require('fs');
const path = require('path');
const sp = require('./sora_pack.js');
const { ROOT, CONTENT, sha256, readGlb, loadPkg, importPath, gamePathToContent, buildSkeleton, materialInfo, texturePng, GlbWriter, addSkeletonNodes, addMesh } = sp;

const VERSION = '1.0.0';
const P = (...a) => path.join(...a);
const OUT = P(ROOT, 'build', 'sora_tmp', 'xsi', 'pack', 'xsi', 'weapons');
const MOVES = [
  { move: 'giant_stamp', clip: '3416', equip: 'Blueprints/Player/p_ex001/w_so050_Equip', form: 1 },
  { move: 'chandelier', clip: '7419', equip: 'Blueprints/Player/p_ex001/w_so080_Equip', form: 1 },
  { move: 'myriad', clip: '9419', equip: 'Blueprints/Player/p_ex001/w_so190_Equip', form: 1 },
];

const prop = (list, name) => (list || []).find((p) => p && p.name === name);
// The first m_Equips array (prop) found inside a value tree.
function findProp(v, name) {
  if (Array.isArray(v)) { for (const x of v) { const r = findProp(x, name); if (r) return r; } return null; }
  if (v && typeof v === 'object') { if (v.name === name && 'value' in v) return v; for (const k of Object.keys(v)) { const r = findProp(v[k], name); if (r) return r; } }
  return null;
}
function equipsOf(equipContent, form) {
  const d = loadPkg(equipContent);
  const data = d.exports.find((e) => /TresEquip.*Data/.test(e.class)).data.props;
  const map = prop(data, 'm_EquipMap').value.entries[form].value;
  const equips = findProp(map, 'm_Equips').value.items;
  return equips.map((unit) => {
    const bp = findProp(unit, 'RsrcData'); const socket = findProp(unit, 'AttachBoneName'); const part = findProp(unit, 'm_AttachPart');
    const bpPath = importPath(d, bp.value).replace(/_C$/, '');
    return { blueprint: bpPath, socket: socket.value, part: part ? part.value : null };
  });
}
function meshOf(blueprintPath) {
  const d = loadPkg(gamePathToContent(blueprintPath));
  const s = JSON.stringify(d.imports.map((im, i) => importPath(d, -(i + 1))));
  const m = s.match(/\/Game\/Character\/wep\/[A-Za-z0-9_]+\/mdl\/[A-Za-z0-9_]+_0\b/);
  if (!m) throw new Error('no skeletal mesh in ' + blueprintPath);
  return m[0];
}
function socketsOfSora() {
  const d = loadPkg('Character/pc/p_ex001/mdl/p_ex001_0_Skeleton');
  const out = {};
  for (const e of d.exports) {
    if (e.class !== 'SkeletalMeshSocket') continue;
    const p = Object.fromEntries((e.data.props || []).map((x) => [x.name, x.value]));
    out[p.SocketName] = { bone: p.BoneName, location_ue: p.RelativeLocation || { x: 0, y: 0, z: 0 }, rotation_ue: p.RelativeRotation || { pitch: 0, yaw: 0, roll: 0 } };
  }
  return out;
}
const pick = (tex, re) => Object.keys(tex || {}).find((k) => re.test(k));

function main() {
  fs.mkdirSync(OUT, { recursive: true });
  const sockets = socketsOfSora();
  const index = { format: 'kh3-sora-xsi-weapons/1', generator: `tools/xsi_weapons.js ${VERSION}`, legal: 'local build output from the user\'s own copy of the game; never distributed', note: 'per XS move: the form weapons KH3 equips for its clip (equip data form 1), their files and Sora\'s socket (bone + UE relative rotation)', moves: [] };
  for (const mv of MOVES) {
    const units = equipsOf(mv.equip, mv.form);
    const weapons = [];
    for (const u of units) {
      const meshPath = meshOf(u.blueprint); const meshContent = gamePathToContent(meshPath); const model = path.basename(meshContent).replace(/_0$/, '');
      const meshGlb = readGlb(P(ROOT, 'assets_converted', 'meshes', CONTENT, meshContent + '.glb'));
      const bones = buildSkeleton(meshGlb.json);
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
      const w = new GlbWriter(`(XSI) via tools/xsi_weapons.js ${VERSION}`); addSkeletonNodes(w, bones);
      const r = addMesh(w, meshGlb, bones, mats.map((m) => ({ gltf: JSON.parse(JSON.stringify(m.gltf)), textures: m.textures, rec: m.rec })));
      w.json.skins[0].name = model; w.json.meshes[0].name = model + '_LOD0'; w.json.nodes[r.node].name = model + '_mesh'; w.json.scenes = [{ name: model, nodes: [0, r.node] }];
      const glb = w.finish();
      const dir = P(OUT, model); fs.mkdirSync(P(dir, 'textures'), { recursive: true });
      fs.writeFileSync(P(dir, 'weapon.glb'), glb);
      for (const t of textures.values()) fs.writeFileSync(P(dir, t.file), t.png);
      const sock = sockets[u.socket];
      if (!sock) throw new Error('no socket ' + u.socket + ' on Sora');
      weapons.push({ model, file: `${model}/weapon.glb`, sha256: sha256(glb), bytes: glb.length, source_mesh: meshPath, blueprint: u.blueprint, attach_part: u.part, socket: u.socket, bone: sock.bone, socket_location_ue: sock.location_ue, socket_rotation_ue: sock.rotation_ue, textures: [...textures.keys()], materials: mats.map((m) => m.rec), stats: r.stats });
      console.log(`${mv.move}: ${model} on ${u.socket} (bone ${sock.bone}), ${glb.length} bytes, ${textures.size} textures`);
    }
    index.moves.push({ move: mv.move, clip: mv.clip, equip: `/Game/${mv.equip}`, form: mv.form, weapons });
  }
  fs.writeFileSync(P(OUT, 'weapons.json'), JSON.stringify(index, null, 1) + '\n');
  console.log(`wrote ${path.relative(ROOT, OUT)}`);
}
main();
