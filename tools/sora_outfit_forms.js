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
//
//   node tools/sora_outfit_forms.js
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
    fs.writeFileSync(path.join(dir, 'materials/wear_extra.json'), JSON.stringify(extra, null, 1) + '\n');
    console.log(`${o.id}: Second Form = ${shared.length ? 'KH3\'s own costume on its p_ex001 clothes (' + shared.length + ' materials)' : ''}${shared.length && direct.length ? ' + ' : ''}${direct.length ? direct.length + ' clothes recoloured (' + direct.map((d) => d.from.split('/').pop()).join(', ') + ')' : ''}`);
  }
}
main();
