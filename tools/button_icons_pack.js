// (BUTTON-ICONS) KH3 PC'S KEYBOARD AND MOUSE GLYPHS for the button icons of the UI pack: a pack step of its own (the
// pad sets xboxone / ps4 / windows are already in data/icon_text.json, written by tools/sora_ui_pack.js iconTextOut).
//
// Source (the user's own KH3 install, converted by tools/texture_convert.js and the package converter):
//   KINGDOM HEARTS III/Content/UI/IconText/IconTextData_km.uasset        the rows (name, X, Y, Width, Height, VSpace, BindKey)
//   KINGDOM HEARTS III/Content/UI/IconText/Localization/<lang>/IconText_km.uasset   the picture, one per language
// KH3 loads exactly these (FUN_14159a9f0: DataTable'/Game/UI/IconText/IconTextData_km'; FUN_1415a5900:
// /Game/UI/IconText/Localization/%s/IconText_km) and finds the glyph of a key by its BindKey (FUN_14157ee80: the row whose
// BindKey is the key's name); a key with no row draws `<icon(Blank)>` (UTresUIP_HudLStickInput 0x141643b50, HudWorkshop
// 0x1416786f0, 0x141681fe0, the alias builder 0x1415a8fc0).
//
// Output: build/sora_tmp/weapons/BUTTON-ICONS/stage/ui/ (a staged pack part; the deploy copies it into build/sora_pack/ui,
// new files only):
//   data/icon_text_km.json                                  rows + the picture of each language
//   textures/IconText/Localization/<lang>/IconText_km.png   the pictures
// Usage: node tools/button_icons_pack.js [--out=DIR] [--check]
'use strict';
const fs = require('fs'), path = require('path');
const bc = require('./lib/bcdec.js');
const ROOT = path.resolve(__dirname, '..');
const GAME = 'KINGDOM HEARTS III/Content/';
const PKG = path.join(ROOT, 'assets_converted/packages'), TEXDIR = path.join(ROOT, 'assets_converted/textures');
const outArg = process.argv.find((a) => a.startsWith('--out='));
const OUT = outArg ? path.resolve(outArg.slice(6)) : path.join(ROOT, 'build/sora_tmp/weapons/BUTTON-ICONS/stage/ui');
const checkOnly = process.argv.includes('--check');
const LANGS = ['en', 'de', 'es', 'fr', 'it', 'ja', 'ko', 'zh-Hant'];

function rowsOf(table) {
  const pj = path.join(PKG, GAME, table + '.uasset.json');
  const j = JSON.parse(fs.readFileSync(pj, 'utf8'));
  const sv = (v) => (v && typeof v === 'object') ? (v.s !== undefined ? v.s : (v.name !== undefined ? v.name : null)) : v;
  // An FName with a number: the name table's entry + "_" + (number - 1), as UE prints it (Kb_1 ... TenKb_0)
  const fname = (n) => (n && typeof n === 'object' && n.index !== undefined) ? (j.names[n.index][0] || j.names[n.index]) + (n.number ? '_' + (n.number - 1) : '') : sv(n);
  const out = {};
  for (const r of j.exports[0].data.native.rows) {
    const name = fname(r.name); if (typeof name !== 'string') continue;
    const g = (k) => { const q = r.props.find((x) => x.name === k); return q ? sv(q.value) : null; };
    const bind = g('BindKey');
    out[name] = { x: g('X'), y: g('Y'), w: g('Width'), h: g('Height'), vspace: g('VSpace'), bind: typeof bind === 'string' && bind !== '' && bind !== 'None' ? bind : null };
  }
  return out;
}

const problems = [];
const icons = rowsOf('UI/IconText/IconTextData_km');
const out = {
  format: 'kh3-ui-data/1',
  source: GAME + 'UI/IconText/IconTextData_km.uasset + Localization/<lang>/IconText_km.uasset',
  note: 'KH3 PC keyboard and mouse glyphs (BUTTON-ICONS). icons[row] = source rectangle in the language\'s picture; bind = the UE key name the row stands for (FUN_14157ee80 finds a key\'s row by it); a key with no row draws Blank. textures[lang] = the picture of that language.',
  textures: {},
  icons,
};
const files = new Map();
for (const lang of LANGS) {
  const image = 'UI/IconText/Localization/' + lang + '/IconText_km';
  const dds = path.join(TEXDIR, GAME, image + '.dds');
  if (!fs.existsSync(dds)) { problems.push('missing ' + image); continue; }
  const t = bc.decodeDds(fs.readFileSync(dds), 0);
  const file = 'textures/' + image.replace(/^UI\//, '') + '.png';
  files.set(file, bc.encodePng(t.width, t.height, Buffer.from(t.rgba), { alpha: true, srgb: true }));
  out.textures[lang] = file;
}
for (const need of ['Kb_A', 'Kb_Sp', 'Kb_Ent', 'ClickLeft', 'ClickRight', 'WheelForward', 'Blank', 'Mouse', 'Kb_X_2']) if (!icons[need]) problems.push('no row ' + need);
const binds = Object.values(icons).filter((r) => r.bind).length;
console.log(`button icons (km): ${Object.keys(icons).length} rows (${binds} bound to a key), ${Object.keys(out.textures).length} languages -> ${OUT}`);
if (problems.length) { console.log('PROBLEMS:\n  ' + problems.join('\n  ')); process.exitCode = 1; }
if (!checkOnly && !problems.length) {
  fs.mkdirSync(path.join(OUT, 'data'), { recursive: true });
  fs.writeFileSync(path.join(OUT, 'data/icon_text_km.json'), JSON.stringify(out));
  for (const [rel, png] of files) { fs.mkdirSync(path.dirname(path.join(OUT, rel)), { recursive: true }); fs.writeFileSync(path.join(OUT, rel), png); }
}
