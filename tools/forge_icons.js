// (FORGE-PREVIEW, the user 2026-11-06: "for the keyblade forge, the icon up top needs to be the keychain like in the example
// picture, not the whole keyblade itself. The keyblade itself is only shown where it's shown in the example")
// KH3'S KEYBLADE FORGE PICTURES, a pack step of its own.
//
// KH3's Forge does not use the item picture (TresItemData.m_IconAsset). Its own table
// Load/Tres/ItemSynthesis/TresItemWeaponIconData (row struct TresItemWeaponEnhanceIconDataTable) names two pictures per
// Keyblade (m_WeaponID):
//   PickerIconAsset    /Game/UI/Icon/Keyblade/chainTopNN    104 x 104: the KEYCHAIN in the selector's round buttons
//                      (CoImgPickerKeyb, a tres.controls.Icon of 104 px)
//   KeybladeIconAsset  /Game/UI/Icon/Keyblade/keybladeNN    512 x 512: the whole Keyblade in the big preview
//                      (CoImgKeyb, 104 x 4.92 = 512 px) and in the power-up effect (anmKeyb.img1 / img2)
// The PC release's Dead of Night has them under /Game/UI/Icon/Steam/Keyblade/. The Keyblade's port key
// (WEP_KEYBLADE_SO_nn) comes from sheets/port/kh3-keyblades.json (its weapon_def column = m_WeaponID).
//
// Output (a staged pack part; install copies NEW files only into build/sora_pack/ui, never overwriting another file):
//   data/forge_icons.json                       picker[key] / keyblade[key] = texture file; missing = keys without one
//   textures/Icon/Keyblade/*.png, textures/Icon/Steam/Keyblade/*.png
// Usage: node tools/forge_icons.js [--out=DIR] [--check] [--install]
'use strict';
const fs = require('fs'), path = require('path');
const bc = require('./lib/bcdec.js');
const ROOT = path.resolve(__dirname, '..');
const GAME = 'KINGDOM HEARTS III/Content/';
const TEXDIR = path.join(ROOT, 'assets_converted/textures');
const TABLE = path.join(ROOT, 'sheets/entities/Load/Tres/ItemSynthesis/TresItemWeaponIconData.json');
const KEYBLADES = path.join(ROOT, 'sheets/port/kh3-keyblades.json');
const LIVE = path.join(ROOT, 'build/sora_pack/ui');

/** The pack files: Map(relative path -> Buffer), and the problems found. */
function build() {
  const files = new Map(); const problems = [];
  const sheet = JSON.parse(fs.readFileSync(KEYBLADES, 'utf8')); const col = (n) => sheet.columns.indexOf(n);
  const keyOf = new Map(sheet.rows.map((r) => [String(r[col('weapon_def')]), String(r[col('id')])]));
  const out = { format: 'kh3-ui-data/1', source: GAME + 'Load/Tres/ItemSynthesis/TresItemWeaponIconData.uasset (PickerIconAsset, KeybladeIconAsset) + UI/Icon/(Steam/)Keyblade/*.uasset', note: 'picker[key] = the keychain picture of the Forge selector (CoImgPickerKeyb); keyblade[key] = the whole-Keyblade picture of the Forge preview (CoImgKeyb) and its power-up effect; key = the port key (sheets/port/kh3-keyblades.json weapon_def = m_WeaponID)', picker: {}, keyblade: {}, missing: [] };
  const picture = (asset) => {
    const m = /^\/Game\/(UI\/Icon\/(?:Steam\/)?Keyblade\/[A-Za-z0-9_]+)\./.exec(String(asset || ''));
    if (!m) return null;
    const file = 'textures/' + m[1].replace(/^UI\//, '') + '.png';
    if (!files.has(file)) {
      const dds = path.join(TEXDIR, GAME, m[1] + '.dds');
      if (!fs.existsSync(dds)) { problems.push('missing texture ' + m[1]); return null; }
      const t = bc.decodeDds(fs.readFileSync(dds), 0);
      files.set(file, bc.encodePng(t.width, t.height, Buffer.from(t.rgba), { alpha: true, srgb: true }));
    }
    return file;
  };
  for (const r of JSON.parse(fs.readFileSync(TABLE, 'utf8')).rows) {
    const def = String(r.m_WeaponID).split('::')[1];
    const key = keyOf.get(def); if (!key) continue; // WEP_KEYBLADE15 / 16: no Sora Keyblade of the port
    const p = picture(r.PickerIconAsset), k = picture(r.KeybladeIconAsset);
    if (p) out.picker[key] = p;
    if (k) out.keyblade[key] = k;
  }
  for (const key of keyOf.values()) if (!out.picker[key] || !out.keyblade[key]) out.missing.push(key);
  files.set('data/forge_icons.json', Buffer.from(JSON.stringify(out)));
  return { files, problems, out };
}

function main() {
  const outArg = process.argv.find((a) => a.startsWith('--out='));
  const OUT = outArg ? path.resolve(outArg.slice(6)) : path.join(ROOT, 'build/sora_tmp/weapons/FORGE-PREVIEW/stage/ui');
  const { files, problems, out } = build();
  console.log(`forge_icons: ${Object.keys(out.picker).length} keychains, ${Object.keys(out.keyblade).length} Keyblades, ${files.size} files; missing: ${out.missing.join(', ') || 'none'}`);
  if (problems.length) console.log('  problems:\n   ' + problems.join('\n   '));
  if (process.argv.includes('--check')) { process.exitCode = problems.length || out.missing.length ? 1 : 0; return; }
  for (const [rel, data] of files) { const f = path.join(OUT, rel); fs.mkdirSync(path.dirname(f), { recursive: true }); fs.writeFileSync(f, data); }
  console.log('  staged -> ' + path.relative(ROOT, OUT));
  if (process.argv.includes('--install')) {
    let added = 0, same = 0;
    for (const [rel, data] of files) {
      const f = path.join(LIVE, rel);
      if (fs.existsSync(f)) { if (Buffer.compare(fs.readFileSync(f), data) === 0) { same++; continue; } console.log('  REFUSED (another file is there): ' + rel); process.exitCode = 1; continue; }
      fs.mkdirSync(path.dirname(f), { recursive: true }); fs.writeFileSync(f, data); added++;
    }
    console.log(`  installed into build/sora_pack/ui: ${added} new, ${same} already there`);
  }
  if (problems.length || out.missing.length) process.exitCode = 1;
}

if (require.main === module) main();
module.exports = { build };
