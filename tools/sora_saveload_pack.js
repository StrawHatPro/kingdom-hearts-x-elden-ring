#!/usr/bin/env node
// tools/sora_saveload_pack.js: the external images of KH3's Save / Load slot rows into the Sora pack
// (docs/sora-saveload-spec.md is the text). The rows of the movie 17_saveload/SaveLoad hold two
// `tres.controls.Icon` clips that the game fills at run time from loose textures:
//   - `charaImage.coImage` (CoImgChara, 256 x 256 shown through the strip y 60..169): /Game/UI/Icon/MenuFace/MenuFace<World><Chara>
//   - `worldTitleImage.coImage` (CoImgWorldTitle, 164 x 128): /Game/UI/Icon/MenuWorldTitle/MenuWorldTitle<WORLD>
//     (table /Game/UI/Data/WorldIconName: world code -> icon + the world's name key in kh3_map.wm)
// This tool ONLY ADDS files: build/sora_pack/saveload/textures/*.png (Sora's faces, every world title, and the
// game's own "no world" title /Game/UI/Icon/Hologram/HologramArea/MenuWorldTitleNone) and
// build/sora_pack/saveload/saveload.json, plus their rows in sheets/port/sora-assets.json (ids saveload/...).
// Nothing of build/sora_pack/ui is touched (tools/sora_ui_pack.js may be re-run without losing these).
//
//   node tools/sora_saveload_pack.js
// Then: node tools/extract.js; node tools/preflight.js --summary
'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const bc = require('./lib/bcdec.js');
const ROOT = path.resolve(__dirname, '..');
const GAME = 'KINGDOM HEARTS III/Content/';
const TEX = path.join(ROOT, 'assets_converted', 'textures', GAME);
const PKG = path.join(ROOT, 'assets_converted', 'packages', GAME);
const OUT = path.join(ROOT, 'build', 'sora_pack', 'saveload');
const SHEET = path.join(ROOT, 'sheets', 'port', 'sora-assets.json');
const sha = (b) => crypto.createHash('sha256').update(b).digest('hex');

// ---- the world table (/Game/UI/Data/WorldIconName)
const win = JSON.parse(fs.readFileSync(path.join(PKG, 'UI/Data/WorldIconName.uasset.json'), 'utf8'));
const wprops = win.exports[0].data.props;
const nameSpace = wprops.find((p) => p.name === 'm_WorldNameNameSpace').value;
const worlds = wprops.find((p) => p.name === 'm_IconName').value.items.map((it) => {
  const get = (n) => it.find((p) => p.name === n).value;
  return { code: String(get('WorldCode')).replace('TRES_WCID_', '').toLowerCase(), name_key: nameSpace + '.' + get('WorldNameKey'), asset: String(get('IconAsset')).split('.')[0].replace('/Game/', '') };
});

const files = []; // [rel, buffer, source package, evidence]
function texture(rel, asset, evidence) {
  const dds = path.join(TEX, asset + '.dds');
  if (!fs.existsSync(dds)) { console.log('  MISSING ' + asset); return null; }
  const t = bc.decodeDds(fs.readFileSync(dds), 0);
  const png = bc.encodePng(t.width, t.height, Buffer.from(t.rgba), { alpha: true, srgb: true });
  if (!files.some((f) => f[0] === rel)) files.push([rel, png, GAME + asset + '.uasset', evidence]); // (the table lists a world twice)
  return { file: rel, size: [t.width, t.height] };
}

const data = {
  format: 'kh3-saveload-data/1',
  note: 'External images of the Save / Load slot rows (movie SaveLoad). file = path under build/sora_pack/saveload. chara: shown 256 x 256 through the clip rectangle of CoImgChara (x 0, y 60, 256 x 109); world: shown 164 x 128 (CoImgWorldTitle). worlds[code] = the world title of a world code (table /Game/UI/Data/WorldIconName, name = key of the world name). none = the game\'s own title image without a world (/Game/UI/Icon/Hologram/HologramArea/MenuWorldTitleNone): what a host without a KH world shows.',
  chara_size: [256, 256], chara_clip: [0, 60, 256, 109], world_size: [164, 128],
  faces: {}, worlds: {}, none: null,
};
// Sora's faces of /Game/UI/Icon/MenuFace (one per costume: base, second costume, Caribbean, Monstropolis, Toy Box, San Fransokyo)
// (party P) + Donald's and Goofy's (the slot of a save whose party leader was Donald / Goofy)
for (const [key, name] of [['sora', 'MenuFaceExSora'], ['sora2', 'MenuFaceEx2Sora'], ['sora_ca', 'MenuFaceCaSora'], ['sora_mi', 'MenuFaceMiSora'], ['sora_ts', 'MenuFaceTsSora'], ['sora_bx', 'MenuFaceBxSora'], ['donald', 'MenuFaceExDonald'], ['goofy', 'MenuFaceExGoofy']]) {
  const t = texture('textures/' + name + '.png', 'UI/Icon/MenuFace/' + name, 'face image of a save slot row (table /Game/UI/Data/SaveMenuFaceIconData picks by world and progress); loaded into CoImgChara by tres.controls.Icon');
  if (t) data.faces[key] = t;
}
for (const w of worlds) {
  const name = path.basename(w.asset);
  const t = texture('textures/' + name + '.png', w.asset, 'world title image of a save slot row (/Game/UI/Data/WorldIconName, world ' + w.code.toUpperCase() + '); loaded into CoImgWorldTitle');
  if (t) data.worlds[w.code] = { ...t, name_key: w.name_key };
}
data.none = texture('textures/MenuWorldTitleNone.png', 'UI/Icon/Hologram/HologramArea/MenuWorldTitleNone', 'the game\'s world title image for "no world" (Data Greeting area list); used by the port for a save whose place is no KH world');

fs.mkdirSync(path.join(OUT, 'textures'), { recursive: true });
const text = Buffer.from(JSON.stringify(data, null, 1) + '\n');
files.push(['saveload.json', text, GAME + 'UI/Data/WorldIconName.uasset', 'world code -> world title image and name key read from the data asset; faces by name; written by tools/sora_saveload_pack.js']);
for (const [rel, buf] of files) fs.writeFileSync(path.join(OUT, rel), buf);

// ---- rows of the assets sheet
const raw = fs.readFileSync(SHEET, 'utf8');
const sheet = JSON.parse(raw);
const mine = files.map(([rel, buf, source, evidence]) => ['saveload/' + rel, rel.endsWith('.png') ? 'texture' : 'data', rel.endsWith('.png') ? ['ui-bitmap'] : '-', source, '-', buf.length, sha(buf), '-', '-', '-', '-', rel.endsWith('.png') ? 'png read back ' + (() => { const d = bc.decodePng(buf); return d.width + ' x ' + d.height; })() : 'json read back', evidence]);
for (const row of mine) if (row.length !== sheet.columns.length) throw new Error('sora-assets has ' + sheet.columns.length + ' columns, the row ' + row.length);
const rows = sheet.rows.filter((r) => !String(r[0]).startsWith('saveload/')).concat(mine);
const head = raw.slice(0, raw.indexOf('"rows":[') + 8);
fs.writeFileSync(SHEET, head + '\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
console.log('sora_saveload_pack: ' + files.length + ' files -> build/sora_pack/saveload (' + Object.keys(data.faces).length + ' faces, ' + Object.keys(data.worlds).length + ' world titles, none ' + (data.none ? 'yes' : 'NO') + '); sora-assets: ' + mine.length + ' rows set');
