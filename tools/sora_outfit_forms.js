#!/usr/bin/env node
// tools/sora_outfit_forms.js (ER69): SECOND FORM ON AN OUTFIT THAT HAS NO FORM COSTUME OF ITS OWN.
//
// The user: "Second Form for KH1/KH2/San Fransokyo: do NOT switch to KH3's outfit; keep the chosen outfit and give the
// form its look by the same means KH3's form textures differ". Writes NEW files only, per outfit folder:
//   wear_extra.json   read by crates/kh-look library.rs next to materials.json:
//     forms   wear forms in the schema of materials.json `wear_forms` (slide FormChangeTex0/1), for an outfit whose
//             clothes ARE KH3 Sora's own material instances (San Fransokyo: p_ex001_mat0/1/2_Inst): KH3's own
//             p_ex001_WearformSet rows (its textures copied into the folder under their own names);
//     direct  textures that replace a slot's base colour while the form is on (an outfit whose clothes are other
//             materials, KH1 / KH2): its own base colour textures RECOLOURED BY KH3'S OWN SECOND FORM: a colour table
//             (16 x 16 x 16) learnt from the texel pairs of p_ex001_0d0N (default) -> p_ex001_5d0N (Second Form);
//             a colour the pairs never show stays as it is.
//   materials/textures/<name>_f5.png, materials/er/tex/p_ex001_5*.dds (copies)
// (weapons P5b) then EVERY OTHER WEAR FORM of the WearformSets (`allForms` below): KH3 Sora's (main pack) and the outfits'.
// wear_extra.json is merged, never replaced: tools/sora_outfit_formcolours.js's rows (`kb_forms`) are kept.
//
//   node tools/sora_outfit_forms.js                  both phases
//   node tools/sora_outfit_forms.js --all-forms-only the all-forms phase alone (--list-written: the new files)
'use strict';
const fs = require('fs'), path = require('path');
const bc = require('./lib/bcdec');
const ROOT = path.resolve(__dirname, '..');
const PACK = path.join(ROOT, 'build/sora_pack');
const MAIN_TEX = path.join(PACK, 'materials/textures');
const VERSION = '1.0.0';
const N = 16;
const CLOTH_ROOTS = /DisneyOrg_Basemat|SQEX_NPC_Basemat_MNLS|SQEX_PC_Basemat_MNLS|SQEX_RealChr_BaseMat|p_ca001_BaseMat/;

function learnTable() {
  const sum = new Float64Array(N * N * N * 3), cnt = new Uint32Array(N * N * N);
  for (const k of ['0', '1', '2']) {
    const a = bc.decodePng(fs.readFileSync(path.join(MAIN_TEX, `p_ex001_0d0${k}.png`))), b = bc.decodePng(fs.readFileSync(path.join(MAIN_TEX, `p_ex001_5d0${k}.png`)));
    if (a.width !== b.width || a.height !== b.height) throw new Error('pair of different sizes: ' + k);
    for (let i = 0; i < a.width * a.height; i++) {
      const s = i * a.channels, t = i * b.channels;
      const bin = ((a.data[s] * N) >> 8) * N * N + ((a.data[s + 1] * N) >> 8) * N + ((a.data[s + 2] * N) >> 8);
      // the change of this texel (target - source): applied to whatever colour of the bin
      sum[bin * 3] += b.data[t] - a.data[s]; sum[bin * 3 + 1] += b.data[t + 1] - a.data[s + 1]; sum[bin * 3 + 2] += b.data[t + 2] - a.data[s + 2];
      cnt[bin]++;
    }
  }
  const table = new Float32Array(N * N * N * 3);
  let used = 0, changed = 0;
  for (let i = 0; i < N * N * N; i++) {
    if (cnt[i] < 8) continue;
    used++;
    for (let c = 0; c < 3; c++) table[i * 3 + c] = sum[i * 3 + c] / cnt[i];
    if (Math.abs(table[i * 3]) + Math.abs(table[i * 3 + 1]) + Math.abs(table[i * 3 + 2]) > 6) changed++;
  }
  return { table, used, changed };
}

function recolour(file, out, lut) {
  const t = bc.decodePng(fs.readFileSync(file));
  const px = Buffer.alloc(t.width * t.height * 4);
  for (let i = 0; i < t.width * t.height; i++) {
    const s = i * t.channels;
    const r = t.data[s], g = t.data[s + 1], b = t.data[s + 2], a = t.channels > 3 ? t.data[s + 3] : 255;
    const bin = ((r * N) >> 8) * N * N + ((g * N) >> 8) * N + ((b * N) >> 8);
    const clamp = (v) => Math.max(0, Math.min(255, Math.round(v)));
    px[i * 4] = clamp(r + lut[bin * 3]); px[i * 4 + 1] = clamp(g + lut[bin * 3 + 1]); px[i * 4 + 2] = clamp(b + lut[bin * 3 + 2]); px[i * 4 + 3] = a;
  }
  fs.writeFileSync(out, bc.encodePng(t.width, t.height, px, { alpha: true, srgb: true }));
}

// (ER69, the user: "make second form KH1 sora look cooler") A SECOND FORM STYLE BY REGIONS for an outfit of its own
// colours (KH1 Sora): KH3's Second Form costume (p_ex001_5d0N against 0d0N) turns his clothes dark with silver / white
// and gold; here every texel is put in a region by its hue / saturation / value (skin and anything unknown is kept):
// the red jumpsuit -> near black (its shading kept), white / grey -> silver, blue -> gold, yellow -> deep gold; where two
// cloth regions meet, a 2 texel pale-gold seam line (the form's trim). Written as <name>_f5s.png.
function hsv(r, g, b) { const mx = Math.max(r, g, b), mn = Math.min(r, g, b), d = mx - mn; let h = 0; if (d > 0) { if (mx === r) h = ((g - b) / d) % 6; else if (mx === g) h = (b - r) / d + 2; else h = (r - g) / d + 4; h *= 60; if (h < 0) h += 360; } return [h, mx > 0 ? d / mx : 0, mx / 255]; }
function regionOf(r, g, b) {
  const [h, sat, v] = hsv(r, g, b);
  if (h >= 10 && h <= 40 && sat >= 0.2 && sat <= 0.7 && v > 0.45) return 0; // skin: kept
  if ((h < 14 || h > 335) && sat > 0.5 && v > 0.2) return 1; // red
  if (h >= 40 && h <= 65 && sat > 0.45 && v > 0.35) return 2; // yellow
  if (h >= 195 && h <= 265 && sat > 0.35 && v > 0.15) return 3; // blue
  if (sat < 0.18 && v > 0.55) return 4; // white / light grey
  if (v < 0.25) return 5; // black / dark
  return 0;
}
function styled(file, out, glowOut) {
  const t = bc.decodePng(fs.readFileSync(file));
  const W = t.width, H = t.height, px = Buffer.alloc(W * H * 4), reg = new Uint8Array(W * H);
  for (let i = 0; i < W * H; i++) { const s = i * t.channels; reg[i] = regionOf(t.data[s], t.data[s + 1], t.data[s + 2]); }
  const c = (v) => Math.max(0, Math.min(255, Math.round(v)));
  // (ER72) the GLOW MASK: red = the trim (1) and the gold regions (0.25), green 0 (the shader reads red * (1 - green), so a
  // slot without a mask, bound to white, does not glow)
  const glow = Buffer.alloc(W * H * 4);
  for (let i = 0; i < W * H; i++) {
    const s = i * t.channels; const r = t.data[s], g = t.data[s + 1], b = t.data[s + 2], a = t.channels > 3 ? t.data[s + 3] : 255;
    const v = Math.max(r, g, b) / 255; let o;
    switch (reg[i]) {
      case 1: o = [28 + 40 * v, 28 + 40 * v, 34 + 46 * v]; break; // jumpsuit: black with a cool sheen
      case 2: o = [235 * Math.min(1, v + 0.1), 170 * Math.min(1, v + 0.1), 40 * v]; break; // deep gold
      case 3: o = [250 * Math.min(1, v + 0.25), 200 * Math.min(1, v + 0.25), 70 * Math.min(1, v + 0.25)]; break; // gold
      case 4: o = [205 * v + 30, 212 * v + 30, 225 * v + 30]; break; // silver
      case 5: o = [r * 0.6, g * 0.6, b * 0.7]; break; // deeper black
      default: o = [r, g, b];
    }
    // the trim: a cloth region next to another cloth region within 2 texels
    if (reg[i] >= 1 && reg[i] <= 4) {
      const x = i % W, y = (i / W) | 0; let edge = false;
      for (let dy = -2; dy <= 2 && !edge; dy++) for (let dx = -2; dx <= 2 && !edge; dx++) { const xx = x + dx, yy = y + dy; if (xx < 0 || yy < 0 || xx >= W || yy >= H) continue; const q = reg[yy * W + xx]; if (q >= 1 && q <= 4 && q !== reg[i] && (reg[i] === 1 || q === 1)) edge = true; }
      if (edge) { o = [255, 236, 170]; glow[i * 4] = 255; }
      else if (reg[i] === 2 || reg[i] === 3) glow[i * 4] = 64;
    }
    glow[i * 4 + 3] = 255;
    px[i * 4] = c(o[0]); px[i * 4 + 1] = c(o[1]); px[i * 4 + 2] = c(o[2]); px[i * 4 + 3] = a;
  }
  fs.writeFileSync(out, bc.encodePng(W, H, px, { alpha: true, srgb: true }));
  if (glowOut) fs.writeFileSync(glowOut, bc.encodePng(W, H, glow, { alpha: true, srgb: false }));
}
const STYLED = ['kh1'];

function main() {
  const index = JSON.parse(fs.readFileSync(path.join(PACK, 'outfits/outfits.json'), 'utf8'));
  const main = JSON.parse(fs.readFileSync(path.join(PACK, 'materials/materials.json'), 'utf8'));
  const { table, used, changed } = learnTable();
  console.log(`Second Form colour table from p_ex001_0d0N -> 5d0N: ${used} of ${N ** 3} colour bins seen, ${changed} of them changed`);
  for (const o of index.outfits) {
    if (!o.dir) continue;
    const dir = path.join(PACK, o.dir);
    const mats = JSON.parse(fs.readFileSync(path.join(dir, 'materials/materials.json'), 'utf8'));
    if (mats.wear_forms && mats.wear_forms['5']) { console.log(`${o.id}: its own Second Form costume (WearformSet): nothing to add`); continue; }
    const extra = { format: 'kh3-sora-wear-extra/1', generator: 'tools/sora_outfit_forms.js ' + VERSION, forms: {}, direct: {} };
    // (a) clothes that are KH3 Sora's own material instances: KH3's own wear forms
    const instances = new Set(Object.values(mats.materials).map((m) => m.asset.split('.').pop()));
    const own = main.wear_forms || {};
    const shared = (own['5'] ? own['5'].materials : []).filter((m) => instances.has(m.material));
    if (shared.length) {
      for (const form of ['0', '5']) {
        extra.forms[form] = { name: own[form].name, materials: own[form].materials.filter((m) => instances.has(m.material)) };
        for (const m of extra.forms[form].materials) for (const key of ['image', 'point']) {
          if (!m[key]) continue;
          const name = path.basename(m[key].file);
          const to = path.join(dir, 'materials/textures', name);
          if (!fs.existsSync(to)) fs.copyFileSync(path.join(MAIN_TEX, name), to);
          const dds = path.join(PACK, 'materials/er/tex', name.replace(/\.png$/, '.dds')), ddsTo = path.join(dir, 'materials/er/tex', name.replace(/\.png$/, '.dds'));
          if (fs.existsSync(dds) && !fs.existsSync(ddsTo)) fs.copyFileSync(dds, ddsTo);
        }
      }
    }
    // (b) clothes of other materials: their base colour recoloured by the table
    const direct = [];
    for (const [slot, m] of Object.entries(mats.materials)) {
      if (m.mesh !== 'sora' || !CLOTH_ROOTS.test(m.root) || shared.some((s) => s.material === m.asset.split('.').pop())) continue;
      const owner = mats.owners[m.owner];
      const k = (owner.slots || []).findIndex((s) => /^(BaseColorTex|BaseColorTexture|BaseTex)$/.test(s.parameter || ''));
      const t = k >= 0 ? m.textures[k] : null;
      if (!t || !t.file) continue;
      const name = path.basename(t.file, '.png') + '_f5.png';
      recolour(path.join(dir, 'materials', t.file), path.join(dir, 'materials/textures', name), table);
      const row = { material: m.asset.split('.').pop(), slot, parameter: owner.slots[k].parameter, file: 'textures/' + name, srgb: t.srgb, mips: t.mips, from: t.file };
      if (STYLED.includes(o.id)) {
        // the styled look is form 5; the plain recolour is kept as form 105 (kh-er `boc.kh1_form_style = 0`)
        const sname = path.basename(t.file, '.png') + '_f5s.png';
        // (ER72, the user: "no need for a glowing trim, KH3 second form doesnt have that"): no glow mask
        styled(path.join(dir, 'materials', t.file), path.join(dir, 'materials/textures', sname));
        (extra.direct['105'] = extra.direct['105'] || []).push(row);
        direct.push({ ...row, file: 'textures/' + sname, style: 'regions' });
      } else direct.push(row);
    }
    if (direct.length) extra.direct['5'] = direct;
    if (!Object.keys(extra.forms).length && !direct.length) { console.log(`${o.id}: no clothes to give a form look`); continue; }
    // (weapons P5b) merged into the file as it is: the rows of other tools (tools/sora_outfit_formcolours.js's `kb_forms` wear
    // forms: KH1 / KH2 forms 1..10, KH2's own 5) and this tool's all-forms phase (`all_forms`) are kept
    const file = path.join(dir, 'materials/wear_extra.json');
    const before = fs.existsSync(file) ? JSON.parse(fs.readFileSync(file, 'utf8')) : null;
    if (before) {
      const theirs = new Set(((before.kb_forms || {}).wear_forms || []).map(String));
      for (const [k, rows] of Object.entries(before.direct || {})) if (theirs.has(k) || !(k in extra.direct)) extra.direct[k] = rows;
      if (theirs.has('5')) delete extra.direct['105'];
      for (const [k, f] of Object.entries(before.forms || {})) if (!(k in extra.forms)) extra.forms[k] = f;
      for (const key of ['kb_forms', 'all_forms']) if (before[key]) extra[key] = before[key];
    }
    fs.writeFileSync(file, JSON.stringify(extra, null, 1) + '\n');
    console.log(`${o.id}: Second Form = ${shared.length ? 'KH3\'s own costume on its p_ex001 clothes (' + shared.length + ' materials)' : ''}${shared.length && direct.length ? ' + ' : ''}${direct.length ? direct.length + ' clothes recoloured (' + direct.map((d) => d.from.split('/').pop()).join(', ') + ')' : ''}`);
  }
  allForms(index, main);
}

// (weapons P5b, the user 2026-11-03: KH3_WEAPONS_PLAN.md 1.3 "OUTFIT x FORM COLOURS") EVERY WEAR FORM OF A WEARFORMSET.
// tools/sora_material_pack.js packs wear forms 0 and 5 only (materials.json `wear_forms`); KH3's WearformSets have all of
// ETresCharWearForm 0..10 (1 Brave, 2 Wisdom, 3 Solid, 4 Grace, 5 Limit, 6 Final, 7 Anti = the default textures with the point
// texture p_ex001_7f00, 8 Light, 9 Dark, 10 TWO-KB). Here the others go into the library's wear_extra.json `forms` (kh-look
// library.rs `add_wear_extra`: a form materials.json has is never replaced), for
//   - KH3 Sora (the main pack, p_ex001_WearformSet): build/sora_pack/materials/wear_extra.json (a NEW file of this tool);
//   - every outfit with a WearformSet of its own (Toy Box, Monstropolis, The Caribbean, Verum Rex: outfits.json `wearform_set`);
//   - an outfit whose clothes ARE KH3 Sora's material instances (San Fransokyo): p_ex001_WearformSet's rows of those instances.
// Textures: the PNG (decoded from the cooked DDS, as tools/sora_material_pack.js writes its own) in `<library>/textures`, and the
// cooked DDS itself in `<library>/er/tex` when it is a block format the renderer reads (kh-shaderc copy_textures' rule). Only new
// files are written. A form one of whose textures is not in assets_converted is left out (the outfit then keeps its clothes).
const CONTENT_DIR = path.join(ROOT, 'assets_converted/textures/KINGDOM HEARTS III/Content');
const PKG_DIR = path.join(ROOT, 'assets_converted/packages/KINGDOM HEARTS III/Content');
const P_EX001_SET = '/Game/Blueprints/Player/p_ex001/p_ex001_WearformSet';
const FORM_NUMBERS = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

function readWearformSet(set) {
  const f = path.join(PKG_DIR, set.replace(/^\/Game\//, '').split('.')[0] + '.uasset.json');
  if (!fs.existsSync(f)) return null;
  const d = JSON.parse(fs.readFileSync(f, 'utf8'));
  const e = d.exports.find((x) => /WearformSet/.test(x.class)); if (!e) return null;
  const importPath = (i) => { let im = d.imports[-i - 1]; const names = [im.objectName]; while (im.outerIndex) { im = d.imports[-im.outerIndex - 1]; names.unshift(im.objectName); } return names[0] + (names.length > 1 ? '.' + names.slice(1).join('.') : ''); };
  const structOf = (items) => Object.fromEntries(items.map((p) => [p.name, p.value]));
  const out = {};
  for (const pr of (e.data && e.data.props) || []) {
    if (pr.name !== 'm_Forms') continue;
    const u = structOf(pr.value); const idx = pr.arrayIndex || 0;
    if (!u.m_bTextureReplace) continue;
    const tex = (i) => (i && i < 0 ? importPath(i) : null);
    out[idx] = { name: u.m_FormName || 'Normal', materials: ((u.m_TextureReplaceData || {}).items || []).map((it) => { const r = structOf(it); return { index: r.m_ReplaceIndex, material: r.m_ReplaceMaterialName, image: tex(r.m_ImageTexture), point: tex(r.m_PointTexture) }; }) };
  }
  return out;
}

// a texture of the set into a library folder: { file, srgb, mips, asset } or null (not converted)
function putTexture(libDir, asset, wrote) {
  const content = asset.replace(/^\/Game\//, '').split('.')[0];
  const dds = path.join(CONTENT_DIR, content + '.dds'), side = path.join(CONTENT_DIR, content + '.texture.json');
  if (!fs.existsSync(dds) || !fs.existsSync(side)) return null;
  const s = JSON.parse(fs.readFileSync(side, 'utf8')); const pr = Object.fromEntries((s.props || []).map((x) => [x.tag.name, x.value]));
  const srgb = pr.SRGB === undefined ? true : !!pr.SRGB;
  const name = path.basename(content);
  const data = fs.readFileSync(dds);
  const png = path.join(libDir, 'textures', name + '.png');
  let mips = 1;
  if (!fs.existsSync(png)) {
    const t = bc.decodeDds(data, 0); mips = t.mipCount;
    fs.mkdirSync(path.dirname(png), { recursive: true });
    fs.writeFileSync(png, bc.encodePng(t.width, t.height, Buffer.from(t.rgba), { alpha: true, srgb }));
    wrote.push(png);
  } else mips = data.length >= 32 ? Math.max(1, data.readUInt32LE(28)) : 1;
  // the cooked file for the renderer (kh-shaderc copy_textures: DXT1 / DXT5 / ATI2 / BC7 family, sizes a multiple of 4)
  const four = data.subarray(84, 88).toString('latin1'); const fmt = data.length >= 132 ? data.readUInt32LE(128) : 0;
  const block = ['DXT1', 'DXT5', 'ATI2'].includes(four) || (four === 'DX10' && [71, 72, 77, 78, 83, 98, 99].includes(fmt));
  const w = data.readUInt32LE(16), h = data.readUInt32LE(12);
  const to = path.join(libDir, 'er', 'tex', name + '.dds');
  // (SETUP-TOOLS) written whether or not <library>/er exists yet: in a build from scratch this step runs before the shader phase makes
  // the folder (the development pack had it, so its DDS were written; a fresh build left every wear form without them)
  if (block && w % 4 === 0 && h % 4 === 0 && !fs.existsSync(to)) {
    fs.mkdirSync(path.dirname(to), { recursive: true }); fs.copyFileSync(dds, to); wrote.push(to);
  }
  return { file: 'textures/' + name + '.png', asset, srgb, mips };
}

function allForms(index, mainMats) {
  const libs = [{ id: 'kh3', dir: path.join(PACK, 'materials'), mats: mainMats, set: P_EX001_SET, only: null }];
  for (const o of index.outfits) {
    if (!o.dir) continue;
    const dir = path.join(PACK, o.dir, 'materials');
    const mats = JSON.parse(fs.readFileSync(path.join(dir, 'materials.json'), 'utf8'));
    const instances = new Set(Object.values(mats.materials).filter((m) => m.mesh === 'sora').map((m) => m.asset.split('.').pop()));
    if (o.wearform_set) libs.push({ id: o.id, dir, mats, set: o.wearform_set, only: null });
    // clothes that are KH3 Sora's own instances (San Fransokyo): KH3's set, those instances' rows
    const own = ((mainMats.wear_forms || {})['0'] || { materials: [] }).materials;
    if (!o.wearform_set && own.some((m) => instances.has(m.material))) libs.push({ id: o.id, dir, mats, set: P_EX001_SET, only: instances });
  }
  const wrote = [];
  for (const lib of libs) {
    const set = readWearformSet(lib.set);
    if (!set) { console.log(`${lib.id}: no WearformSet ${lib.set}: no wear forms added`); continue; }
    const file = path.join(lib.dir, 'wear_extra.json');
    const extra = fs.existsSync(file) ? JSON.parse(fs.readFileSync(file, 'utf8')) : { format: 'kh3-sora-wear-extra/1', generator: 'tools/sora_outfit_forms.js ' + VERSION, forms: {}, direct: {} };
    extra.forms = extra.forms || {};
    // the forms the library has already: materials.json's and wear_extra.json's of other phases (San Fransokyo's 0 / 5 above)
    const have = new Set([...Object.keys(lib.mats.wear_forms || {}), ...Object.entries(extra.forms).filter(([, f]) => !/\(all forms\)/.test(f.by || '')).map(([k]) => k)]);
    const added = [], missing = [];
    for (const n of FORM_NUMBERS) {
      const key = String(n);
      if (have.has(key) || !set[n]) continue;
      const rows = set[n].materials.filter((m) => !lib.only || lib.only.has(m.material));
      if (!rows.length) continue;
      let ok = true;
      const materials = rows.map((m) => {
        const out = { index: m.index, material: m.material };
        for (const k of ['image', 'point']) { if (!m[k]) continue; const t = putTexture(lib.dir, m[k], wrote); if (!t) { ok = false; missing.push(`${n}:${m[k].split('.').pop()}`); } else out[k] = t; }
        return out;
      });
      if (!ok) { delete extra.forms[key]; continue; }
      extra.forms[key] = { name: set[n].name, replace: true, materials, by: 'tools/sora_outfit_forms.js ' + VERSION + ' (all forms)' };
      added.push(n);
    }
    if (!added.length && !extra.all_forms) { console.log(`${lib.id}: no wear form to add from ${lib.set.split('/').pop()}`); continue; }
    extra.all_forms = { generator: 'tools/sora_outfit_forms.js ' + VERSION, set: lib.set, wear_forms: added, note: 'forms rows of these wear forms: the WearformSet\'s own costume textures (forms materials.json lacks); kh-look library.rs add_wear_extra' };
    fs.writeFileSync(file, JSON.stringify(extra, null, 1) + '\n');
    console.log(`${lib.id}: wear forms ${added.join(', ') || '-'} added from ${lib.set.split('/').pop()}${missing.length ? ` (left out, texture not converted: ${missing.join(', ')})` : ''}`);
  }
  console.log(`all forms: ${wrote.length} new texture file(s)`);
  if (process.argv.includes('--list-written')) for (const f of wrote) console.log('  + ' + path.relative(ROOT, f));
}

if (process.argv.includes('--all-forms-only')) {
  allForms(JSON.parse(fs.readFileSync(path.join(PACK, 'outfits/outfits.json'), 'utf8')), JSON.parse(fs.readFileSync(path.join(PACK, 'materials/materials.json'), 'utf8')));
} else main();
