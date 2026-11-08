// (KEYBLADES, the user 2026-11-03: "KH2 second form needs to look like KH3's default outfit! Use the same colour accents, use the
// same base colour") THE SECOND FORM MIRROR SWAP, FROM DATA. KH3's Second Form costume repaints KH3 Sora's outfit with KH2-style
// colours: the texel pairs (KH3 SECOND FORM colour, KH3 DEFAULT colour) of p_ex001_5d0N / p_ex001_0d0N say which KH3 default
// colour each KH2-style colour stands for. KH2 Sora's Second Form inverts that: each cloth texel of the KH2 outfit takes the
// KH3 default colour paired with its own (KH2-style) colour.
//
//   1. the TABLE: the Second Form colours of KH3's cloth textures in 8 x 8 x 8 bins; a bin covering >= 0.3 % of the cloth is an
//      entry: its mean Second Form colour -> the dominant KH3 default colour of the same texels (12 x 12 x 12 bins);
//   2. the PLACE: a colour that KH3 paints differently in different places (Second Form's black is the default's black top AND its
//      grey-green bottoms) is resolved by where the KH2 texel is on the body: every KH2 cloth texel gets its 3D position (its
//      triangle rasterised in UV space, bind pose), KH3 Sora's cloth vertices within RADIUS carry (Second Form colour, default
//      colour) sampled from the textures at their UVs; among those whose Second Form colour is close to the texel's (Oklab
//      distance < MATCH), the nearest one gives the target; without one, the table's nearest entry does;
//   3. the RECOLOUR keeps the KH2 texture's shading and detail: target (linear) x the texel's luminance / the matched Second Form
//      colour's luminance (0.3 .. 2), highlights compressed towards white instead of clipped.
// The decision is made on a grid of at most 1024 x 1024 texels of the texture (each texel uses its cell's decision with its own
// shading). Skin texels are kept.
'use strict';
const fs = require('fs');
const path = require('path');
const bc = require('./bcdec');
const sp = require('../sora_pack.js');

const RADIUS = 0.07, CELL = 0.035, MATCH = 0.11, MIN_SHARE = 0.003, DECIDE_MAX = 1024;
const lin = (c) => { const x = c / 255; return x <= 0.04045 ? x / 12.92 : Math.pow((x + 0.055) / 1.055, 2.4); };
const enc = (x) => { const c = Math.max(0, Math.min(1, x)); return Math.round(255 * (c <= 0.0031308 ? c * 12.92 : 1.055 * Math.pow(c, 1 / 2.4) - 0.055)); };
function oklab(r8, g8, b8) {
  const r = lin(r8), g = lin(g8), b = lin(b8);
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b), m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b), s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s, 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s];
}
const dist = (a, b) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
const luminance = (r8, g8, b8) => 0.2126 * lin(r8) + 0.7152 * lin(g8) + 0.0722 * lin(b8);
function isSkin(r, g, b) { const mx = Math.max(r, g, b), mn = Math.min(r, g, b), d = mx - mn; if (!d || mx !== r) return false; const h = (((g - b) / d) % 6) * 60; const s = d / mx, v = mx / 255; return h >= 8 && h <= 40 && s >= 0.2 && s <= 0.7 && v > 0.45; }

// the KH3 texture pair N (0, 1, 2): default p_ex001_0d0N, Second Form p_ex001_5d0N
function pair(texRoot, k) {
  const dir = path.join(texRoot, 'Character', 'pc', 'p_ex001', 'mdl', 'mat0');
  return { def: bc.decodeDds(fs.readFileSync(path.join(dir, `p_ex001_0d0${k}.dds`)), 0), sf: bc.decodeDds(fs.readFileSync(path.join(dir, 'FormChangeTex', `p_ex001_5d0${k}.dds`)), 0) };
}

// 1. the table
function buildTable(pairs) {
  const B = 8, D = 12; const bins = new Map(); let total = 0;
  for (const p of pairs) {
    const { def, sf } = p;
    for (let i = 0; i < sf.width * sf.height; i++) {
      const s = i * 4; const r = sf.rgba[s], g = sf.rgba[s + 1], b = sf.rgba[s + 2];
      if (isSkin(r, g, b)) continue;
      const k = (((r * B) >> 8) * B + ((g * B) >> 8)) * B + ((b * B) >> 8);
      if (!bins.has(k)) bins.set(k, { n: 0, sf: [0, 0, 0], def: new Map() });
      const e = bins.get(k); e.n++; e.sf[0] += r; e.sf[1] += g; e.sf[2] += b; total++;
      const dr = def.rgba[s], dg = def.rgba[s + 1], db = def.rgba[s + 2]; const dk = (((dr * D) >> 8) * D + ((dg * D) >> 8)) * D + ((db * D) >> 8);
      if (!e.def.has(dk)) e.def.set(dk, [0, 0, 0, 0]); const q = e.def.get(dk); q[0] += dr; q[1] += dg; q[2] += db; q[3]++;
    }
  }
  const table = [];
  for (const e of bins.values()) {
    if (e.n < total * MIN_SHARE) continue;
    let best = null; for (const q of e.def.values()) if (!best || q[3] > best[3]) best = q;
    const sfRgb = e.sf.map((x) => Math.round(x / e.n)), defRgb = best.slice(0, 3).map((x) => Math.round(x / best[3]));
    table.push({ second_form: sfRgb, kh3_default: defRgb, share: +(e.n / total).toFixed(4), agreement: +(best[3] / e.n).toFixed(3), lab: oklab(...sfRgb) });
  }
  table.sort((a, b) => b.share - a.share);
  return table;
}

// 2a. KH3 Sora's cloth (triangle centres) with their (Second Form, default) colours, in a grid
function kh3Points(soraGlb, pairs) {
  const g = sp.readGlb(soraGlb); const pts = [];
  for (const p of g.json.meshes[0].primitives) {
    const m = /^p_ex001_mat([012])$/.exec(g.json.materials[p.material].name); if (!m) continue;
    const { def, sf } = pairs[+m[1]];
    // (triangle centres: a vertex sits on its UV island's edge, where the texture has its seams and padding)
    const pos = sp.readAccessor(g, p.attributes.POSITION).data, uv = sp.readAccessor(g, p.attributes.TEXCOORD_0).data, ix = sp.readAccessor(g, p.indices).data;
    for (let t = 0; t < ix.length; t += 3) {
      const V = [ix[t], ix[t + 1], ix[t + 2]]; const cu = (V.reduce((a2, v) => a2 + uv[v * 2], 0)) / 3, cv = (V.reduce((a2, v) => a2 + uv[v * 2 + 1], 0)) / 3;
      const x = Math.min(sf.width - 1, Math.max(0, Math.floor((cu - Math.floor(cu)) * sf.width))), y = Math.min(sf.height - 1, Math.max(0, Math.floor((cv - Math.floor(cv)) * sf.height)));
      const s = (y * sf.width + x) * 4;
      if (isSkin(sf.rgba[s], sf.rgba[s + 1], sf.rgba[s + 2])) continue;
      const cp = [0, 1, 2].map((c) => (pos[V[0] * 3 + c] + pos[V[1] * 3 + c] + pos[V[2] * 3 + c]) / 3);
      pts.push({ p: cp, sf: [sf.rgba[s], sf.rgba[s + 1], sf.rgba[s + 2]], lab: oklab(sf.rgba[s], sf.rgba[s + 1], sf.rgba[s + 2]), def: [def.rgba[s], def.rgba[s + 1], def.rgba[s + 2]] });
    }
  }
  const grid = new Map(); const key = (a, b, c) => `${a},${b},${c}`;
  for (const q of pts) { const k = key(Math.floor(q.p[0] / CELL), Math.floor(q.p[1] / CELL), Math.floor(q.p[2] / CELL)); if (!grid.has(k)) grid.set(k, []); grid.get(k).push(q); }
  const near = (x, y, z) => { const out = []; const r = Math.ceil(RADIUS / CELL); const cx = Math.floor(x / CELL), cy = Math.floor(y / CELL), cz = Math.floor(z / CELL);
    for (let a = -r; a <= r; a++) for (let b = -r; b <= r; b++) for (let c = -r; c <= r; c++) for (const q of grid.get(key(cx + a, cy + b, cz + c)) || []) { const d = Math.hypot(q.p[0] - x, q.p[1] - y, q.p[2] - z); if (d <= RADIUS) out.push([q, d]); } return out; };
  return { near, count: pts.length };
}

// (weapons P5b) KH3's pale warm beige (hue 35..65 degrees, saturation 0.15..0.5, value > 0.6): brass, not cloth
function isBrass(rgb) { const [r, g, b] = rgb; const mx = Math.max(r, g, b), mn = Math.min(r, g, b), d = mx - mn; if (!d || mx === b) return false; let h = mx === r ? ((g - b) / d) % 6 : (b - r) / d + 2; h *= 60; if (h < 0) h += 360; const s = d / mx, v = mx / 255; return h >= 35 && h <= 65 && s >= 0.15 && s <= 0.5 && v > 0.6; }

// 2b + 3: one KH2 cloth texture (`white`: KH3's default white cloth colour, for the brass rule; optional)
function swapTexture(src, out, modelGlb, slotNames, table, points, white) {
  const t = bc.decodePng(fs.readFileSync(src)); const W = t.width, H = t.height, C = t.channels;
  // the decision grid: a 3D position per grid cell from the slots' triangles (bind pose)
  const sc = Math.max(1, Math.ceil(Math.max(W, H) / DECIDE_MAX)); const GW = Math.ceil(W / sc), GH = Math.ceil(H / sc);
  const posGrid = new Float32Array(GW * GH * 3).fill(NaN);
  const g = sp.readGlb(modelGlb);
  for (const p of g.json.meshes[0].primitives) {
    if (!slotNames.includes(g.json.materials[p.material].name)) continue;
    const pos = sp.readAccessor(g, p.attributes.POSITION).data, uv = sp.readAccessor(g, p.attributes.TEXCOORD_0).data, ix = sp.readAccessor(g, p.indices).data;
    for (let k = 0; k < ix.length; k += 3) {
      const V = [ix[k], ix[k + 1], ix[k + 2]]; const P = V.map((v) => [(uv[v * 2] - Math.floor(uv[V[0] * 2])) * GW, (uv[v * 2 + 1] - Math.floor(uv[V[0] * 2 + 1])) * GH]);
      const area = (P[1][0] - P[0][0]) * (P[2][1] - P[0][1]) - (P[1][1] - P[0][1]) * (P[2][0] - P[0][0]); if (Math.abs(area) < 1e-12) continue;
      const x0 = Math.max(0, Math.floor(Math.min(P[0][0], P[1][0], P[2][0]))), x1 = Math.min(GW - 1, Math.ceil(Math.max(P[0][0], P[1][0], P[2][0])));
      const y0 = Math.max(0, Math.floor(Math.min(P[0][1], P[1][1], P[2][1]))), y1 = Math.min(GH - 1, Math.ceil(Math.max(P[0][1], P[1][1], P[2][1])));
      for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) {
        const px = x + 0.5, py = y + 0.5;
        const w0 = ((P[1][0] - px) * (P[2][1] - py) - (P[1][1] - py) * (P[2][0] - px)) / area, w1 = ((P[2][0] - px) * (P[0][1] - py) - (P[2][1] - py) * (P[0][0] - px)) / area, w2 = 1 - w0 - w1;
        if (w0 < -0.02 || w1 < -0.02 || w2 < -0.02) continue;
        for (let c = 0; c < 3; c++) posGrid[(y * GW + x) * 3 + c] = pos[V[0] * 3 + c] * w0 + pos[V[1] * 3 + c] * w1 + pos[V[2] * 3 + c] * w2;
      }
    }
  }
  // the KH2 texture's own palette (its dominant colours, >= 0.5 % of its non-skin texels): every texel's CLASS is the nearest one
  const pal = []; { const B = 8; const bins = new Map(); let n = 0;
    for (let i = 0; i < W * H; i++) { const s = i * C; const r = t.data[s], gg = t.data[s + 1], b = t.data[s + 2]; if (isSkin(r, gg, b)) continue; const k = (((r * B) >> 8) * B + ((gg * B) >> 8)) * B + ((b * B) >> 8); const e = bins.get(k) || [0, 0, 0, 0]; e[0] += r; e[1] += gg; e[2] += b; e[3]++; bins.set(k, e); n++; }
    for (const e of bins.values()) if (e[3] >= n * 0.005) { const rgb = e.slice(0, 3).map((x) => Math.round(x / e[3])); pal.push({ rgb, lab: oklab(...rgb), lum: luminance(...rgb) }); } }
  const classMemo = new Map();
  const classOf = (r, gg, b) => { const k = (r >> 3) * 1024 + (gg >> 3) * 32 + (b >> 3); if (classMemo.has(k)) return classMemo.get(k); const lab = oklab(r, gg, b); let best = 0, bd = Infinity; pal.forEach((p, j) => { const d = dist(p.lab, lab); if (d < bd) { bd = d; best = j; } }); classMemo.set(k, best); return best; };
  // the ISLANDS: 4-connected decision cells of one class on the mesh (a garment piece of one colour)
  const cellClass = new Int16Array(GW * GH).fill(-1);
  for (let y = 0; y < GH; y++) for (let x = 0; x < GW; x++) { const gi = y * GW + x; if (Number.isNaN(posGrid[gi * 3])) continue; const s = (Math.min(H - 1, y * sc + (sc >> 1)) * W + Math.min(W - 1, x * sc + (sc >> 1))) * C; const r = t.data[s], gg = t.data[s + 1], b = t.data[s + 2]; if (!isSkin(r, gg, b)) cellClass[gi] = classOf(r, gg, b); }
  const comp = new Int32Array(GW * GH).fill(-1); const comps = [];
  for (let start = 0; start < GW * GH; start++) {
    if (cellClass[start] < 0 || comp[start] >= 0) continue;
    const id = comps.length; const cls = cellClass[start]; const stack = [start]; comp[start] = id; const cells = [];
    while (stack.length) { const c = stack.pop(); cells.push(c); const x = c % GW, y = (c / GW) | 0; for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) { const nx = x + dx, ny = y + dy; if (nx < 0 || ny < 0 || nx >= GW || ny >= GH) continue; const nc = ny * GW + nx; if (comp[nc] < 0 && cellClass[nc] === cls) { comp[nc] = id; stack.push(nc); } } }
    // the island's votes: the KH3 default colour that the place says (cells sampled every other one each way)
    const votes = new Map(); const lab = pal[cls].lab;
    for (const c of cells) { const x = c % GW, y = (c / GW) | 0; if (cells.length > 16 && ((x | y) & 1)) continue;
      let best = null; for (const [q, d] of points.near(posGrid[c * 3], posGrid[c * 3 + 1], posGrid[c * 3 + 2])) { const e = dist(q.lab, lab); if (e < MATCH && (!best || d + e * 0.2 < best.score)) best = { score: d + e * 0.2, q }; }
      if (!best) continue; const k = best.q.def.map((v) => v >> 5).join(','); const v = votes.get(k) || { n: 0, def: [0, 0, 0], sf: [0, 0, 0] }; v.n++; for (let j = 0; j < 3; j++) { v.def[j] += best.q.def[j]; v.sf[j] += best.q.sf[j]; } votes.set(k, v); }
    let top = null; for (const v of votes.values()) if (!top || v.n > top.n) top = v;
    let pick;
    if (top) pick = { def: top.def.map((x) => Math.round(x / top.n)), how: 'place', votes: top.n };
    else { let bt = null; for (const e of table) { const dd = dist(e.lab, lab); if (!bt || dd < bt.d) bt = { d: dd, e }; } pick = { def: bt.e.kh3_default, how: 'table' }; }
    // (weapons P5b, the user 2026-11-03: KH2 Second Form's chest flaps "came out pale beige and should be KH3 white") a garment
    // piece whose vote is KH3's pale warm beige (the default's brass buttons / zipper pull: small metal parts KH3 draws on its
    // cloth, never a cloth colour of its own) takes KH3's white cloth colour instead
    if (white && isBrass(pick.def)) pick = { ...pick, def: white, how: pick.how + '+white', was: pick.def };
    comps.push({ cls, cells: cells.length, ...pick });
  }
  // the class -> table pick (texels off the mesh: padding, unused space)
  const tablePick = pal.map((p) => { let bt = null; for (const e of table) { const dd = dist(e.lab, p.lab); if (!bt || dd < bt.d) bt = { d: dd, e }; } return bt.e.kh3_default; });
  const px = Buffer.alloc(W * H * 4); const stats = { islands: comps.length, island_cells_by_place: 0, island_cells_by_table: 0, skin_kept: 0, palette: pal.map((p) => p.rgb) };
  for (const c of comps) stats[c.how === 'place' ? 'island_cells_by_place' : 'island_cells_by_table'] += c.cells;
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const i = y * W + x, s = i * C; const r = t.data[s], gg = t.data[s + 1], b = t.data[s + 2], a = C > 3 ? t.data[s + 3] : 255;
    let o = [r, gg, b];
    if (isSkin(r, gg, b)) stats.skin_kept++;
    else {
      const cls = classOf(r, gg, b); const gi = Math.floor(y / sc) * GW + Math.floor(x / sc); const cid = comp[gi];
      // a texel of another class than its cell (an edge, a stitch): the island of its own class nearby is not searched; its class's
      // table pick is used, unless the cell's island has the same class
      const target = cid >= 0 && comps[cid].cls === cls ? comps[cid].def : tablePick[cls];
      // its shading: its luminance against its class colour's (the texture's detail kept), highlights compressed
      const k = Math.max(0.3, Math.min(2, luminance(r, gg, b) / Math.max(1e-4, pal[cls].lum)));
      o = target.map((c) => { const t0 = lin(c); return enc(k <= 1 ? t0 * k : Math.min(t0 * k, 1 - (1 - t0) / k)); });
    }
    px[i * 4] = o[0]; px[i * 4 + 1] = o[1]; px[i * 4 + 2] = o[2]; px[i * 4 + 3] = a;
  }
  fs.writeFileSync(out, bc.encodePng(W, H, px, { alpha: true, srgb: true }));
  return { width: W, height: H, decision_grid: [GW, GH], ...stats, island_list: comps.filter((c) => c.cells >= 200).map((c) => ({ kh2_colour: pal[c.cls].rgb, cells: c.cells, kh3_default: c.def, by: c.how, votes: c.votes || 0, ...(c.was ? { brass_vote: c.was } : {}) })) };
}

module.exports = { pair, buildTable, kh3Points, swapTexture, oklab, isBrass, RADIUS, MATCH };
