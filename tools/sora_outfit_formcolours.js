#!/usr/bin/env node
// (KEYBLADES, the user 2026-11-03: "for all of the keyblade forms, make sure KH1 and KH2 Sora get unique colours for each form
// change that suits the colours of the form change from KH3 Sora") EVERY KH3 WEAR FORM ON THE KH1 AND KH2 OUTFITS.
//
// How KH3 does it [A]: Blueprints/Player/p_ex001/p_ex001_WearformSet m_Forms[N] (N = ETresCharWearForm: 1 Brave, 2 Wisdom,
// 3 Solid, 4 Grace, 5 Limit, 6 Final, 7 Anti, 8 Light, 9 Dark, 10 TWO-KB) swaps the base colour texture of Sora's three cloth
// material instances (p_ex001_mat0/1/2_Inst: FormChangeTex <- p_ex001_<N>d00 / d01 / d02, FormChangePointTex <- p_ex001_<N>f00)
// while the equipment component slides FormChangeSlide 0 -> 1 (tools/sora_material_pack.js wearForms). Each keyblade form
// names the wear form Sora wears (TresEquipKeybladeData m_OwnerWearForm; keyblades/manifest.json forms[].wear_form).
//
// THE KH1 / KH2 VARIANTS (generated from the outfits' own textures, nothing hand painted): KH3's form colours are read from the
// texel pairs default p_ex001_0d0K -> form p_ex001_<N>d0K, per colour region of KH3 Sora's default costume (red accent, yellow
// trim, black / dark grey cloth, light grey, white / cream); every cloth texel of the KH1 / KH2 base colour textures is put in
// a region by its hue / saturation / value and takes the KH3 form's colour of the region it maps to (MAPPING below), its own
// shading kept (value relative to its region's mean value). Skin and anything unclassified keep their colour.
// Wear form 5 (Second Form) keeps the look tools/sora_outfit_forms.js gives it (the user's earlier choice: ER69 / ER72).
// Wear form 7 (Anti: Rage Form) is not a costume in KH3 but the ANTI MODE of Sora's materials (AntiVolume): tools/lib/rage_bake.js
// bakes that look onto every body slot of the outfit (KH3's darkness formula + KH3's glowing pattern carried onto the outfit's
// UVs) and writes the pattern as a glow mask for the renderer (the user 2026-11-03: "make sure KH1 and KH2 Sora can look like
// rage form as well").
//
//   node tools/sora_outfit_formcolours.js     writes outfits/<kh1|kh2>/materials/textures/<name>_kbf<N>.png, merges
//                                             direct[N] rows into outfits/<id>/materials/wear_extra.json (kh-look library.rs:
//                                             the slot's base colour while wear form N is on), keyblades/outfit_forms.json
// Runs after tools/sora_outfit_forms.js (which writes wear_extra.json); idempotent (its own rows are replaced, others kept).
// Local build output from the user's own copy of the game; never distributed.
'use strict';
const fs = require('fs');
const path = require('path');
const bc = require('./lib/bcdec');

const VERSION = '1.0.0';
const ROOT = path.resolve(__dirname, '..');
const PACK = path.join(ROOT, 'build', 'sora_pack');
const TEX = path.join(ROOT, 'assets_converted', 'textures', 'KINGDOM HEARTS III', 'Content', 'Character', 'pc', 'p_ex001', 'mdl', 'mat0');
const REF = path.join(ROOT, 'build', 'sora_tmp', 'keyblades', 'kh3_forms'); // KH3's own form textures as PNG (the picture test)
const FORMS = [
  { n: 1, key: '1', name: 'Brave', id: '1' }, { n: 2, key: '2', name: 'Wisdom', id: '2' }, { n: 3, key: '3', name: 'Solid', id: '3' }, { n: 4, key: '4', name: 'Grace', id: '4' },
  { n: 6, key: '6', name: 'Final', id: '6' }, { n: 7, key: '7', name: 'Anti (Rage Form)', id: '7' }, { n: 8, key: '8', name: 'Light', id: '8' }, { n: 9, key: '9', name: 'Dark', id: '9' }, { n: 10, key: '10', name: 'TWO-KB (Double Form)', id: 'a' },
];
// outfits whose Second Form (wear form 5) this tool makes (the user 2026-11-03: "KH2 second form colours should use KH3 default
// outfit colour palette, like how KH3 second form uses KH2's"): a MIRROR SWAP: KH3 Sora's Second Form costume wears KH2's colours,
// so KH2 Sora's Second Form wears KH3 Sora's DEFAULT (no form) palette. tools/sora_outfit_forms.js's colour-table rows for this
// outfit are dropped (no other form uses them).
const OWN_SECOND_FORM = new Set(['kh2']);
// (Rage Form) the face-dark gradient of KH3's face material taken as one value for a face texture (most of the face lies past
// AntiFaceDarkOffset 0.3 over the 15 cm AntiFaceDarkVolume): a rule of ours for a texture (the shader grades it per pixel)
const FACE_DARK = 0.85;
// (the swap itself: tools/lib/second_form_swap.js: KH3's own (Second Form -> default) texel pairs, by colour and by place)
const CLOTH_ROOTS = /DisneyOrg_Basemat|SQEX_NPC_Basemat_MNLS|SQEX_PC_Basemat_MNLS|SQEX_RealChr_BaseMat|p_ca001_BaseMat/;
// the regions of a texel (hue degrees, saturation 0..1, value 0..1)
// (weapons P5b) 'navy' (index 8): an OUTFIT texel of a dark blue (KH2's navy cloth) when `navy` is asked (recolour); KH3's own palette
// is read without it (its dark cloth stays one region), so only the outfits' MAPPING tells navy from black
const REGIONS = ['keep', 'red', 'yellow', 'blue', 'green', 'white', 'grey', 'dark', 'navy'];
function hsv(r, g, b) { const mx = Math.max(r, g, b), mn = Math.min(r, g, b), d = mx - mn; let h = 0; if (d > 0) { if (mx === r) h = ((g - b) / d) % 6; else if (mx === g) h = (b - r) / d + 2; else h = (r - g) / d + 4; h *= 60; if (h < 0) h += 360; } return [h, mx > 0 ? d / mx : 0, mx / 255]; }
function regionOf(r, g, b, navy = false) {
  const [h, s, v] = hsv(r, g, b);
  if (h >= 10 && h <= 40 && s >= 0.2 && s <= 0.7 && v > 0.45) return 0; // skin
  if (navy && v >= 0.2 && v < 0.42 && h >= 190 && h <= 250 && s > 0.5) return 8; // (P5b) navy cloth of an outfit
  if (v < 0.24) return 7; // black / very dark (any hue)
  if (s < 0.2) return v > 0.68 ? 5 : v > 0.36 ? 6 : 7; // white / cream, light-mid grey, dark grey
  if ((h < 18 || h > 330) && s > 0.4) return 1; // red / maroon
  if (h >= 38 && h <= 70 && s > 0.4) return 2; // yellow / gold
  if (h >= 180 && h <= 270 && s > 0.3) return v < 0.42 ? 7 : 3; // blue (a dark navy is cloth black: KH2's jacket)
  if (h >= 75 && h < 170 && s > 0.25) return 4; // green
  if (s < 0.32) return v > 0.68 ? 5 : v > 0.36 ? 6 : 7; // cream / greyish
  return 0;
}
// outfit region -> KH3 region whose form colour it takes (chosen so the outfit reads like KH3's form: the main cloth colour
// takes KH3's main cloth colour, the accent the accent, the trims the trims)
const MAPPING = {
  // (weapons P5b, the user 2026-11-03: "Match KH3 Ultimate") Final Form (6): `final_place` (tools/lib/final_place.js): the outfit's
  // own pieces where KH3's Final costume is dark at the same place on the body take the form's dark contrast; the rest as below
  kh1: { final_place: true, red: 'accent', yellow: ['yellow', 'grey'], blue: ['dark', 'contrast'], green: 'grey', white: 'white', grey: 'grey', dark: 'dark', navy: 'dark',
    why: 'KH1 Sora: the red jumpsuit is his main colour = KH3\'s red accent panels (the colour each KH3 form changes most); the yellow shoes = KH3\'s yellow trims; the blue pockets / belts and the black jacket parts = KH3\'s black cloth; the white / cream hood and gloves = KH3\'s white; grey = KH3\'s grey. [a, b]: b (the form\'s dark contrast colour) when the form paints both its black cloth and a\'s region light (Final Form: white with black details), so the outfit keeps two tones' },
  // (weapons P5b) Final Form: `final_place` as KH1's. `navy` (KH2's dark blue cloth) is its own region only to shade it as the
  // dark cloth it is; its role is the dark cloth's
  kh2: { final_place: true, red: ['accent', 'contrast'], yellow: ['yellow', 'grey'], blue: ['accent', 'contrast'], green: 'grey', white: 'white', grey: 'grey', dark: 'dark', navy: 'dark',
    why: 'KH2 Sora: the black / navy jacket and shorts = KH3\'s black cloth; the red pocket flaps and the blue details = KH3\'s red accent; the yellow straps and trims = KH3\'s yellow trims; the grey hood and pockets = KH3\'s grey, white = white. [a, b]: as for KH1' },
};

function readTex(file) { const t = bc.decodeDds(fs.readFileSync(file), 0); return { w: t.width, h: t.height, rgba: t.rgba }; }
function formFile(id, k) { const n = parseInt(id, 16); const sub = n === 0 ? '' : n >= 8 ? path.join('FormChangeTex', 'dlc01') : 'FormChangeTex'; return path.join(TEX, sub, `p_ex001_${id}d0${k}.dds`); }
// The DOMINANT colour of a texel set (12 levels per channel; the fullest bin's mean): a mean would mix the parts a form paints
// differently (KH3's forms swap which panels take the accent) into a dull grey.
const BINS = 12;
function dominant() { const n = new Float64Array(BINS ** 3), s = new Float64Array(BINS ** 3 * 3); let total = 0;
  return { add(r, g, b, w = 1) { const k = (((r * BINS) >> 8) * BINS + ((g * BINS) >> 8)) * BINS + ((b * BINS) >> 8); n[k] += w; s[k * 3] += r * w; s[k * 3 + 1] += g * w; s[k * 3 + 2] += b * w; total += w; },
    get() { if (!total) return null; let best = 0; for (let k = 1; k < n.length; k++) if (n[k] > n[best]) best = k; const rgb = [s[best * 3] / n[best], s[best * 3 + 1] / n[best], s[best * 3 + 2] / n[best]].map((x) => Math.round(x)); return { rgb, value: +(Math.max(...rgb) / 255).toFixed(4), texels: total, share: +(n[best] / total).toFixed(3) }; } }; }
// KH3's palette of a form: per region of KH3 Sora's DEFAULT costume, the dominant colour the form puts there; plus `accent` = the
// form's dominant coloured cloth texel (saturation > 0.2, value > 0.25, weighted by saturation x value, at least 1 % of the cloth texels; the form's identity colour: Brave's dark red, Wisdom's
// blue, Solid's gold, Grace's green, Dark's violet), falling back to the dominant colour of the default's red panels.
function palette(id) {
  const reg = Object.fromEntries(REGIONS.slice(1).map((r) => [r, { form: dominant(), def: dominant() }]));
  const accent = dominant(), contrast = dominant();
  const hues = { hue_red: dominant(), hue_yellow: dominant(), hue_blue: dominant(), hue_green: dominant() }; const hueN = { hue_red: 0, hue_yellow: 0, hue_blue: 0, hue_green: 0 };
  let cloth = 0, accentN = 0, contrastN = 0;
  for (const k of [0, 1]) { // d00 / d01: the cloth (d02, the shoes' and belts' light-blue tile sheet, is the same in every form but for its dots)
    const a = readTex(formFile('0', k)), f = id === '7' ? a : readTex(formFile(id, k));
    if (a.w !== f.w || a.h !== f.h) throw new Error(`form ${id} texture ${k}: size differs from the default`);
    for (let i = 0; i < a.w * a.h; i++) {
      const s = i * 4; const r0 = regionOf(a.rgba[s], a.rgba[s + 1], a.rgba[s + 2]); if (!r0) continue;
      const fr = f.rgba[s], fg = f.rgba[s + 1], fb = f.rgba[s + 2];
      reg[REGIONS[r0]].form.add(fr, fg, fb); reg[REGIONS[r0]].def.add(a.rgba[s], a.rgba[s + 1], a.rgba[s + 2]);
      cloth++; const [, sat, v] = hsv(fr, fg, fb); if (sat > 0.2 && v > 0.25 && regionOf(fr, fg, fb)) { accent.add(fr, fg, fb, sat * v); accentN++; }
      if (v < 0.3) { contrast.add(fr, fg, fb); contrastN++; }
      if (sat > 0.35 && v > 0.2) { const [h] = hsv(fr, fg, fb); const fam = h < 18 || h > 330 ? 'hue_red' : h >= 38 && h <= 70 ? 'hue_yellow' : h >= 190 && h <= 270 ? 'hue_blue' : h >= 75 && h < 170 ? 'hue_green' : null; if (fam) { hues[fam].add(fr, fg, fb); hueN[fam]++; } }
    }
  }
  const out = Object.fromEntries(Object.entries(reg).map(([r, x]) => [r, { form: x.form.get(), default: x.def.get() }]));
  const acc = accent.get();
  out.accent = { form: acc && accentN > cloth * 0.01 ? acc : out.red.form, default: out.red.default, rule: 'dominant coloured texel of the form' };
  for (const [fam, d] of Object.entries(hues)) { const g = d.get(); out[fam] = { form: g && hueN[fam] > cloth * 0.005 ? g : null, rule: 'dominant texel of this hue family (saturation > 0.35) anywhere in the form\'s cloth, at least 0.5 %' }; }
  const con = contrast.get();
  out.contrast = { form: con && contrastN > cloth * 0.03 ? con : null, rule: 'dominant dark (value < 0.3) cloth texel of the form, when at least 3 % of the cloth is dark' };
  if (id === '7') out.accent.form = out.red.form; // Rage: the default's red, darkened below
  if (id === '7') for (const [r, p] of Object.entries(out)) if (p.form) { const k = r === 'red' || r === 'accent' ? 0.6 : 0.35; p.form = { ...p.form, rgb: p.form.rgb.map((x) => Math.round(x * k)), value: +(p.form.value * k).toFixed(4) }; }
  return out;
}
// an outfit region's role: a name, or [a, b] = b when the form paints both its black cloth and a light (Final Form)
function roleOf(m, pal) {
  if (!Array.isArray(m)) return m;
  const [a, b] = m; const light = (r) => pal[r] && pal[r].form && pal[r].form.value > 0.7;
  if (/^hue_/.test(a)) return pal[a] && pal[a].form ? a : b; // a hue family the form lacks: the fallback
  return light('dark') && light(a) && pal[b] && pal[b].form ? b : a;
}
// (weapons P5b) `place`: tools/lib/final_place.js's mask (a texel's region where its piece is dark at KH3's Final place) -> contrast
function recolour(src, out, pal, map, place) {
  const t = bc.decodePng(fs.readFileSync(src)); const W = t.width, H = t.height, C = t.channels;
  const reg = new Uint8Array(W * H); const vsum = REGIONS.map(() => 0), vcnt = REGIONS.map(() => 0);
  for (let i = 0; i < W * H; i++) { const s = i * C; reg[i] = regionOf(t.data[s], t.data[s + 1], t.data[s + 2], true); const rm = reg[i] === 8 ? 7 : reg[i]; vsum[rm] += Math.max(t.data[s], t.data[s + 1], t.data[s + 2]) / 255; vcnt[rm]++; } // (P5b) navy shades as the dark cloth it was
  const px = Buffer.alloc(W * H * 4); const used = {};
  for (let i = 0; i < W * H; i++) {
    const s = i * C; const r = t.data[s], g = t.data[s + 1], b = t.data[s + 2], a = C > 3 ? t.data[s + 3] : 255;
    let o = [r, g, b];
    const R = REGIONS[reg[i]];
    const role = place && reg[i] && place[i] === reg[i] && pal.contrast && pal.contrast.form ? 'contrast' : roleOf(map[R], pal);
    if (reg[i] && role && pal[role] && pal[role].form) {
      const target = pal[role].form; const rm = reg[i] === 8 ? 7 : reg[i]; const mean = vsum[rm] / vcnt[rm];
      const k = mean > 0 ? (Math.max(r, g, b) / 255) / mean : 1; // the texel's shading relative to its region
      // the texel's shading on the target colour: darker texels scale it down; lighter ones go towards white only as far as
      // the target leaves room (1 - (1 - t) / k), so a light target keeps the texture's detail instead of clipping to white
      const kk = Math.min(1.8, k);
      o = target.rgb.map((x) => { const t0 = x / 255; return 255 * (kk <= 1 ? t0 * kk : Math.min(t0 * kk, 1 - (1 - t0) / kk)); });
      used[R] = (used[R] || 0) + 1;
    }
    for (let c = 0; c < 3; c++) px[i * 4 + c] = Math.max(0, Math.min(255, Math.round(o[c])));
    px[i * 4 + 3] = a;
  }
  fs.writeFileSync(out, bc.encodePng(W, H, px, { alpha: true, srgb: true }));
  return { width: W, height: H, texels_by_region: used };
}

function main() {
  const index = JSON.parse(fs.readFileSync(path.join(PACK, 'outfits', 'outfits.json'), 'utf8'));
  const kb = (() => { try { return JSON.parse(fs.readFileSync(path.join(PACK, 'keyblades', 'manifest.json'), 'utf8')); } catch { return null; } })();
  const formsUsing = {}; // wear form number -> the keyblade forms that put it on
  for (const [id, k] of Object.entries(kb ? kb.keyblades : {})) for (const f of k.forms) { const m = /TRES_WEAR_FORM_([0-9A-F]+)$/.exec(f.wear_form || ''); if (!m) continue; const n = parseInt(m[1], 16); if (!n) continue; (formsUsing[n] = formsUsing[n] || new Set()).add(`${id} ${k.name || ''} ${f.form}${f.grow_level !== 'TRES_WEAPON_GROW_LVL01' ? ' ' + f.grow_level : ''}`.trim()); }
  fs.mkdirSync(REF, { recursive: true });
  const palettes = {}; let defaultPalette = null;
  for (const f of [{ id: '0', n: 0, name: 'Normal' }, ...FORMS, { id: '5', n: 5, name: 'Limit (Second Form)' }]) {
    palettes[f.n] = f.n === 0 ? null : palette(f.id);
    if (f.n === 0) defaultPalette = palette('0'); // KH3's default costume's own colours (KH2's Second Form, the mirror swap)
    // KH3's own textures of the form as PNG (reference for the picture test; not pack files)
    for (const k of [0, 1, 2]) { const file = f.id === '7' ? formFile('0', k) : formFile(f.id, k); const to = path.join(REF, `p_ex001_${f.id}d0${k}.png`); if (!fs.existsSync(to)) { const t = readTex(file); fs.writeFileSync(to, bc.encodePng(t.w, t.h, Buffer.from(t.rgba), { alpha: false, srgb: true })); } }
  }
  // (a rule of ours) KH3's TWO-KB costume (Double Form: Oathkeeper + Oblivion) has Light Form's palette: on the outfits its
  // trims take Dark Form's identity violet so the two forms differ ("unique colours for each form change"); the rest is KH3's
  const p10 = palettes[10], p9 = palettes[9];
  if (p10 && p9 && p9.accent.form) p10.yellow = { ...p10.yellow, form: p9.accent.form, rule: 'Double Form: the trims take Dark Form\'s identity colour (KH3\'s TWO-KB palette equals Light Form\'s)' };
  const result = { format: 'kh3-sora-outfit-forms/1', generator: `tools/sora_outfit_formcolours.js ${VERSION}`, note: 'per (outfit, wear form): the slot textures that replace the outfit\'s base colours while KH3\'s wear form N is on (outfits/<id>/materials/wear_extra.json direct[N], kh-look library.rs Library::set_wear_form / the wear-form switch); KH3 Sora (main pack) wears KH3\'s own costume textures (p_ex001_WearformSet). Wear form 5 = tools/sora_outfit_forms.js (Second Form, unchanged).',
    wear_forms: {}, palettes, default_palette: defaultPalette, mapping: MAPPING, regions: 'tools/sora_outfit_formcolours.js regionOf (hue / saturation / value)', outfits: {} };
  for (const f of [...FORMS, { n: 5, name: 'Limit (Second Form)' }]) result.wear_forms[f.n] = { name: f.name, enum: `ETresCharWearForm::TRES_WEAR_FORM_${f.n.toString(16).toUpperCase().padStart(2, '0')}`, keyblade_forms: [...(formsUsing[f.n] || [])].sort(), kh3_textures: f.n === 7 ? 'p_ex001_0d00..02 (unchanged) + point p_ex001_7f00' : `p_ex001_${f.n.toString(16)}d00..02, point p_ex001_${f.n.toString(16)}f00` };
  // (Rage Form) KH3's anti values, KH3 Sora's cloth as the pattern's source, the pattern itself
  const RB = require('./lib/rage_bake.js');
  // (Second Form mirror swap) KH3's (Second Form colour -> default colour) table and KH3 Sora's cloth points: tools/lib/second_form_swap.js
  const SFS = require('./lib/second_form_swap.js');
  // (weapons P5b) KH3 Sora's cloth with KH3's Final Form colours (tools/lib/final_place.js), read once when needed
  const FP = require('./lib/final_place.js'); let fpPoints = null;
  const finalPoints = () => fpPoints || (fpPoints = FP.kh3FinalPoints(path.join(ROOT, 'assets_converted', 'textures', 'KINGDOM HEARTS III', 'Content'), path.join(PACK, 'sora.glb')));
  const sfPairs = [0, 1, 2].map((k) => SFS.pair(path.join(ROOT, 'assets_converted', 'textures', 'KINGDOM HEARTS III', 'Content'), k));
  const sfTable = SFS.buildTable(sfPairs); const sfPoints = SFS.kh3Points(path.join(PACK, 'sora.glb'), sfPairs);
  result.second_form_swap = { how: 'tools/lib/second_form_swap.js (header)', radius_m: SFS.RADIUS, oklab_match: SFS.MATCH, kh3_points: sfPoints.count, table: sfTable.map(({ lab, ...e }) => e) };
  const mainMatsR = JSON.parse(fs.readFileSync(path.join(PACK, 'materials', 'materials.json'), 'utf8'));
  const rage = { params: RB.antiParams(mainMatsR), source: RB.kh3Source(path.join(PACK, 'sora.glb')), pattern: RB.readPattern(path.join(ROOT, 'assets_converted', 'textures', 'KINGDOM HEARTS III', 'Content')) };
  const pngSize = (file) => { const b = fs.readFileSync(file); return [b.readUInt32BE(16), b.readUInt32BE(20)]; };
  // every body slot of a library with a base colour texture (cloth, skin, face; not the eyes / hair, which have none)
  const bodySlots = (mats, extraParams) => { const out = []; for (const [slot, m] of Object.entries(mats.materials)) { if (m.mesh !== 'sora') continue; const owner = mats.owners[m.owner]; const k = (owner.slots || []).findIndex((x) => new RegExp(`^(BaseColorTex|BaseColorTexture|BaseTex${extraParams || ''})$`).test(x.parameter || '')); const t = k >= 0 ? m.textures[k] : null; if (t && t.file) out.push({ slot, material: m.asset.split('.').pop(), parameter: owner.slots[k].parameter, tex: t }); } return out; };
  result.rage = { params: rage.params, kh3_source_vertices: rage.source.vertices, max_transfer_distance_m: RB.MAX_DIST, how: 'tools/lib/rage_bake.js (header): KH3 anti mode at AntiVolume 1 baked into the base colour, the 7f00 pattern carried by nearest KH3 cloth vertex (second UV, tile (1, 0)) and baked as the AntiMaskEmissiv hue; <name>_kbf7_glow.png = the pattern mask for an emissive pass' };
  for (const o of index.outfits) {
    if (!MAPPING[o.id] || !o.dir) continue;
    const dir = path.join(PACK, o.dir); const matsFile = path.join(dir, 'materials', 'materials.json');
    const mats = JSON.parse(fs.readFileSync(matsFile, 'utf8'));
    const extraFile = path.join(dir, 'materials', 'wear_extra.json');
    const extra = fs.existsSync(extraFile) ? JSON.parse(fs.readFileSync(extraFile, 'utf8')) : { format: 'kh3-sora-wear-extra/1', generator: `tools/sora_outfit_formcolours.js ${VERSION}`, forms: {}, direct: {} };
    extra.direct = extra.direct || {};
    const slots = [];
    for (const [slot, m] of Object.entries(mats.materials)) {
      if (m.mesh !== 'sora' || !CLOTH_ROOTS.test(m.root)) continue;
      const owner = mats.owners[m.owner]; const k = (owner.slots || []).findIndex((s) => /^(BaseColorTex|BaseColorTexture|BaseTex)$/.test(s.parameter || ''));
      const t = k >= 0 ? m.textures[k] : null; if (!t || !t.file) continue;
      slots.push({ slot, material: m.asset.split('.').pop(), parameter: owner.slots[k].parameter, tex: t });
    }
    result.outfits[o.id] = { model: o.model, mapping: MAPPING[o.id].why, forms: {} };
    const done = new Map(); // (texture, form) -> written file
    // (weapons P5b) Final Form's dark pieces by KH3's placement (per texture: its slots' pieces)
    const places = new Map(); const placeOf = (file) => { if (!places.has(file)) { const r = FP.darkPieces(path.join(dir, 'materials', file), path.join(dir, 'model.glb'), slots.filter((x) => x.tex.file === file).map((x) => x.slot), (r0, g0, b0) => regionOf(r0, g0, b0, true), finalPoints()); places.set(file, r); console.log(`${o.id}: Final Form placement on ${file}: ${r.dark_pieces} of ${r.pieces} pieces dark (${r.voted} with KH3 cloth near), ${(r.dark_share * 100).toFixed(1)} % of the cloth`); } return places.get(file); };
    const own5 = OWN_SECOND_FORM.has(o.id);
    if (own5) delete extra.direct['105']; // (the earlier keep of sora_outfit_forms.js's rows: dropped, no form uses them)
    for (const f of own5 ? [...FORMS, { n: 5, key: '5', name: 'Limit (Second Form)', id: '5' }] : FORMS) {
      if (f.n === 7) continue; // Rage Form: below
      const rows = []; const files = [];
      for (const s of slots) {
        const name = `${path.basename(s.tex.file, '.png')}_kbf${f.n}.png`;
        const key = s.tex.file + '|' + f.n;
        if (!done.has(key)) { const info = f.n === 5 && own5 ? SFS.swapTexture(path.join(dir, 'materials', s.tex.file), path.join(dir, 'materials', 'textures', name), path.join(dir, 'model.glb'), slots.filter((x) => x.tex.file === s.tex.file).map((x) => x.slot), sfTable, sfPoints, defaultPalette.white.default.rgb) : recolour(path.join(dir, 'materials', s.tex.file), path.join(dir, 'materials', 'textures', name), palettes[f.n], MAPPING[o.id], f.n === 6 && MAPPING[o.id].final_place ? placeOf(s.tex.file).mask : null); done.set(key, info); files.push({ file: 'textures/' + name, from: s.tex.file, ...info }); }
        rows.push({ material: s.material, slot: s.slot, parameter: s.parameter, file: 'textures/' + name, srgb: s.tex.srgb, mips: s.tex.mips, from: s.tex.file, by: `tools/sora_outfit_formcolours.js ${VERSION}` });
      }
      extra.direct[f.key] = rows;
      result.outfits[o.id].forms[f.n] = { wear_form: f.n, name: f.name, wear_extra: `outfits/${o.id}/materials/wear_extra.json direct["${f.key}"]`, slots: rows.map((r) => ({ slot: r.slot, material: r.material, file: `outfits/${o.id}/materials/${r.file}` })), textures: files.map((x) => ({ ...x, file: `outfits/${o.id}/materials/${x.file}` })) };
    }
    { // wear form 7: Rage Form on every body slot
      const body = bodySlots(mats); const byTex = new Map(); for (const b of body) { if (!byTex.has(b.tex.file)) byTex.set(b.tex.file, []); byTex.get(b.tex.file).push(b); }
      const rows = []; const files = [];
      for (const [file, list] of byTex) {
        const src = path.join(dir, 'materials', file); const [W, H] = pngSize(src);
        const pm = RB.patternMask(path.join(dir, 'model.glb'), list.map((b) => b.slot), W, H, rage.source, rage.pattern);
        const name = `${path.basename(file, '.png')}_kbf7.png`, glow = `${path.basename(file, '.png')}_kbf7_glow.png`;
        const info = RB.rageTexture(src, path.join(dir, 'materials', 'textures', name), path.join(dir, 'materials', 'textures', glow), pm.mask, rage.params, list.some((b) => /_F_mat/.test(b.slot)) ? FACE_DARK : 0);
        files.push({ file: `outfits/${o.id}/materials/textures/${name}`, glow_mask: `outfits/${o.id}/materials/textures/${glow}`, from: file, ...info, pattern_triangles: pm.triangles, vertices_mapped_to_kh3: `${pm.vertices_mapped} of ${pm.vertices}` });
        for (const b of list) rows.push({ material: b.material, slot: b.slot, parameter: b.parameter, file: 'textures/' + name, srgb: b.tex.srgb, mips: b.tex.mips, from: file, glow_mask: 'textures/' + glow, by: `tools/sora_outfit_formcolours.js ${VERSION} (lib/rage_bake.js)` });
      }
      extra.direct['7'] = rows;
      result.outfits[o.id].forms[7] = { wear_form: 7, name: 'Anti (Rage Form)', wear_extra: `outfits/${o.id}/materials/wear_extra.json direct["7"]`, slots: rows.map((r) => ({ slot: r.slot, material: r.material, file: `outfits/${o.id}/materials/${r.file}`, glow_mask: `outfits/${o.id}/materials/${r.glow_mask}` })), textures: files,
        renderer: 'the base colours carry the look; for the full KH3 look the renderer also draws the glow mask as emission (AntiMaskEmissiv, scrolling), the eyes red (AntiEyeEmissiv) and the aura (ead_p_ant_com): documentation/KH3_WEAPON_ASSETS.md 6b' };
      console.log(`${o.id}: Rage Form on ${body.length} body slot(s): ${files.map((x) => `${path.basename(x.from)} (${x.pattern_texels} pattern texels, ${x.vertices_mapped_to_kh3} vertices on KH3's cloth)`).join(', ')}`);
    }
    if (extra.direct['5'] && !own5) result.outfits[o.id].forms[5] = { wear_form: 5, name: 'Limit (Second Form)', wear_extra: `outfits/${o.id}/materials/wear_extra.json direct["5"] (tools/sora_outfit_forms.js)`, slots: extra.direct['5'].map((r) => ({ slot: r.slot, material: r.material, file: `outfits/${o.id}/materials/${r.file}` })) };
    extra.kb_forms = { generator: `tools/sora_outfit_formcolours.js ${VERSION}`, wear_forms: [...FORMS.map((f) => f.n), ...(own5 ? [5] : [])], note: 'direct rows of these wear forms are written by tools/sora_outfit_formcolours.js (KH3 form colours mapped onto this outfit); 5 / 105 are tools/sora_outfit_forms.js\'s' };
    fs.writeFileSync(extraFile, JSON.stringify(extra, null, 1) + '\n');
    console.log(`${o.id}: ${FORMS.length} wear forms x ${slots.length} cloth slot(s) (${[...new Set(slots.map((s) => path.basename(s.tex.file)))].join(', ')}) -> wear_extra.json direct[${FORMS.map((f) => f.key).join(',')}]`);
  }
  fs.mkdirSync(path.join(PACK, 'keyblades'), { recursive: true });
  fs.writeFileSync(path.join(PACK, 'keyblades', 'outfit_forms.json'), JSON.stringify(result, null, 1) + '\n');
  // the picture test's plan (crates/kh-er-render/examples/kb_pics.rs): rows KH3 / KH1 / KH2 Sora, columns wear forms 0..10
  const relr = (f) => path.relative(ROOT, f).split(path.sep).join('/');
  const baseOf = (mats, dir) => { const out = {}; for (const [slot, m] of Object.entries(mats.materials)) { const o = mats.owners[m.owner]; const k = (o && o.slots || []).findIndex((s) => /^(BaseColorTex|BaseColorTexture|BaseTex|FormChangeTex0|EyeTexture|Diffuse)$/.test(s.parameter || '')); const t = k >= 0 ? m.textures[k] : null; if (t && t.file) out[slot] = relr(path.join(dir, 'materials', t.file)); } return out; };
  const panels = [];
  const order = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
  const mainMats = JSON.parse(fs.readFileSync(path.join(PACK, 'materials', 'materials.json'), 'utf8'));
  const mainBase = baseOf(mainMats, PACK);
  // (Rage Form) KH3 Sora's own look as a mock-up (its anti mode drawn into its textures the same way; the eyes red): picture only
  const rageDir = path.join(REF, 'rage'); fs.mkdirSync(rageDir, { recursive: true });
  const eyeRed = path.join(rageDir, 'eye_red.png'); { const e = rage.params.eye_low, mx = Math.max(...e); const c = e.map((x) => Math.round(255 * Math.max(0, x) / mx)); const b = Buffer.alloc(8 * 8 * 4); for (let i = 0; i < 64; i++) { b[i * 4] = c[0]; b[i * 4 + 1] = c[1]; b[i * 4 + 2] = c[2]; b[i * 4 + 3] = 255; } fs.writeFileSync(eyeRed, bc.encodePng(8, 8, b, { alpha: false, srgb: true })); }
  const kh3Rage = {};
  { const body = bodySlots(mainMatsR, '|FormChangeTex0'); const byTex = new Map(); for (const b of body) { if (!byTex.has(b.tex.file)) byTex.set(b.tex.file, []); byTex.get(b.tex.file).push(b); }
    for (const [file, list] of byTex) { const src = path.join(PACK, 'materials', file); const [W, H] = pngSize(src); const pm = RB.patternMask(path.join(PACK, 'sora.glb'), list.map((b) => b.slot), W, H, rage.source, rage.pattern); const out = path.join(rageDir, `${path.basename(file, '.png')}_rage.png`); RB.rageTexture(src, out, null, pm.mask, rage.params, list.some((b) => /_F_mat/.test(b.slot)) ? FACE_DARK : 0); for (const b of list) kh3Rage[b.slot] = relr(out); } }
  const eyeSlots = (mats) => Object.entries(mats.materials).filter(([, m]) => m.mesh === 'sora' && /IrisTex/.test(m.root || '')).map(([k]) => k);
  for (const n of order) {
    const slots = { ...mainBase }; const id = n.toString(16);
    for (const [slot, m] of Object.entries(mainMats.materials)) { const mm = /^p_ex001_mat([012])_Inst$/.exec(m.asset.split('.').pop()); if (mm && m.mesh === 'sora') slots[slot] = relr(path.join(REF, `p_ex001_${n === 7 ? '0' : id}d0${mm[1]}.png`)); }
    if (n === 7) { Object.assign(slots, kh3Rage); for (const e of eyeSlots(mainMatsR)) slots[e] = relr(eyeRed); }
    panels.push({ label: `KH3 Sora, wear form ${n}${n === 7 ? ' (mock-up of its anti mode)' : ''}`, glb: 'build/sora_pack/sora.glb', slots });
  }
  for (const o of index.outfits.filter((x) => MAPPING[x.id] && x.dir)) {
    const dir = path.join(PACK, o.dir); const mats = JSON.parse(fs.readFileSync(path.join(dir, 'materials', 'materials.json'), 'utf8')); const extra = JSON.parse(fs.readFileSync(path.join(dir, 'materials', 'wear_extra.json'), 'utf8'));
    const base = baseOf(mats, dir);
    for (const n of order) { const slots = { ...base }; for (const r of (extra.direct || {})[String(n)] || []) slots[r.slot] = relr(path.join(dir, 'materials', r.file)); if (n === 7) for (const e of eyeSlots(mats)) slots[e] = relr(eyeRed); panels.push({ label: `${o.id.toUpperCase()} Sora, wear form ${n}`, glb: relr(path.join(dir, 'model.glb')), slots }); }
  }
  fs.writeFileSync(path.join(ROOT, 'build', 'sora_tmp', 'keyblades', 'form_pics_plan.json'), JSON.stringify({ columns: order.length, tile: [200, 330], view: 'front', panels }, null, 1));
  for (const f of FORMS) { const p = palettes[f.n]; console.log(`  form ${String(f.n).padStart(2)} ${f.name.padEnd(22)} accent->${JSON.stringify(p.accent.form && p.accent.form.rgb)} red->${JSON.stringify(p.red.form && p.red.form.rgb)} yellow->${JSON.stringify(p.yellow.form && p.yellow.form.rgb)} dark->${JSON.stringify(p.dark.form && p.dark.form.rgb)} grey->${JSON.stringify(p.grey.form && p.grey.form.rgb)} white->${JSON.stringify(p.white.form && p.white.form.rgb)}`); }
}
main();
