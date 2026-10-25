// (ER90, XSI request 1) The form weapons' maps for the Elden Ring renderer (crates/kh-er-render/src/xsiweapon.rs):
//   node build/sora_tmp/er/er90/tools/xsi_weapon_er_slots.js OUT_PACK
// reads build/sora_tmp/xsi/pack/xsi/weapons/weapons.json (tools/xsi_weapons.js) and each slot's material instance, and writes
// NEW files under OUT_PACK/xsi/weapons/:
//   <model>/er/<texture>.png  the base colour (alpha set to 255: the Kingdom Key's material reads its alpha as AO / mask) and
//                             the normal map, as the game's own texels (the way tools/sora_material_pack.js keeps the character
//                             materials' textures: no glTF green flip; the weapon pack's glTF copies are not touched)
//   er_neutral.png            4x4 RGBA (0, 0, 0, 255): no metal / leather zone, no occlusion
//   er_slots.json             per model, per glTF material: { base, normal } relative to xsi/weapons
// Local build output from the user's own copy of the game; never distributed.
'use strict';
const fs = require('fs');
const path = require('path');
const ROOT = path.resolve(__dirname, '../../../../..');
const sp = require(path.join(ROOT, 'tools', 'sora_pack.js'));
const bc = require(path.join(ROOT, 'tools', 'lib', 'bcdec.js'));
const out = path.resolve(process.argv[2] || path.join(__dirname, '..', 'pack'));
const staged = path.join(ROOT, 'build', 'sora_tmp', 'xsi', 'pack', 'xsi', 'weapons');
const weapons = JSON.parse(fs.readFileSync(path.join(staged, 'weapons.json'), 'utf8'));
const dir = path.join(out, 'xsi', 'weapons');
fs.mkdirSync(dir, { recursive: true });

const pick = (tex, re) => Object.keys(tex || {}).find((k) => !k.startsWith('#') && re.test(k));
function raw(gamePath, opaque) {
  const content = sp.gamePathToContent(gamePath);
  const dds = path.join(ROOT, 'assets_converted', 'textures', sp.CONTENT, content + '.dds');
  const side = JSON.parse(fs.readFileSync(path.join(ROOT, 'assets_converted', 'textures', sp.CONTENT, content + '.texture.json'), 'utf8'));
  const props = Object.fromEntries(side.props.map((x) => [x.tag.name, x.value]));
  const t = bc.decodeDds(fs.readFileSync(dds), 0);
  const px = Buffer.from(t.rgba);
  if (opaque) for (let i = 3; i < px.length; i += 4) px[i] = 255;
  return { name: path.basename(content), png: bc.encodePng(t.width, t.height, px, { alpha: true }), width: t.width, height: t.height, srgb: props.SRGB === undefined ? true : !!props.SRGB, compression: props.CompressionSettings || 'TC_Default' };
}

const record = { format: 'kh3-sora-xsi-weapon-er/1', generator: 'build/sora_tmp/er/er90/tools/xsi_weapon_er_slots.js 1.0.0', legal: 'local build output from the user\'s own copy of the game; never distributed', note: 'ER90: each form weapon slot drawn as a copy of the Kingdom Key\'s translated material (kh-er-render xsiweapon.rs) with these maps; our approximation', neutral: 'er_neutral.png', models: {} };
const written = [];
const write = (rel, buf) => { const p = path.join(dir, rel); fs.mkdirSync(path.dirname(p), { recursive: true }); if (fs.existsSync(p)) throw new Error('exists: ' + p); fs.writeFileSync(p, buf); written.push(rel); };
const neutral = Buffer.alloc(4 * 4 * 4); for (let i = 3; i < neutral.length; i += 4) neutral[i] = 255;
write('er_neutral.png', bc.encodePng(4, 4, neutral, { alpha: true }));
for (const mv of weapons.moves) for (const w of mv.weapons) {
  if (record.models[w.model]) continue;
  const slots = {};
  for (const m of w.materials) {
    const info = sp.materialInfo(m.material_instance);
    const base = pick(info.tex, /^base_?col/i), normal = pick(info.tex, /normal/i);
    const s = { material_instance: m.material_instance, base_parameter: base || null, normal_parameter: normal || null };
    for (const [key, param, opaque] of [['base', base, true], ['normal', normal, false]]) {
      if (!param) continue;
      const t = raw(info.tex[param], opaque);
      const rel = `${w.model}/er/${t.name}.png`;
      if (!written.includes(rel)) write(rel, t.png);
      s[key] = rel;
      s[key + '_source'] = { asset: info.tex[param], width: t.width, height: t.height, srgb: t.srgb, compression: t.compression };
    }
    slots[m.slot_name] = s;
    console.log(`${mv.move} ${w.model} ${m.slot_name}: base ${s.base || '-'} (${base}), normal ${s.normal || '-'} (${normal})`);
  }
  record.models[w.model] = { glb: w.file, slots };
}
write('er_slots.json', Buffer.from(JSON.stringify(record, null, 1) + '\n'));
console.log(`${written.length} new file(s) under ${dir}`);
