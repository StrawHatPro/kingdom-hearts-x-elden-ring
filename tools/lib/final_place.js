// (weapons P5b, the user 2026-11-03: "Match KH3 Ultimate") KH1 / KH2 SORA IN FINAL FORM (wear form 6, Ultima Weapon's form) WITH
// KH3'S OWN PLACEMENT OF ITS DARK PANELS. KH3's Final costume (p_ex001_6d00..02) is white with black panels and dark trims; where
// they sit is read from KH3 Sora's mesh and carried onto the outfit BY PLACE, and decided per PIECE of the outfit itself:
//   1. KH3 Sora's cloth triangle centres (bind pose) carry KH3's Final colour at their UV (tools/lib/second_form_swap.js kh3Points
//      with the pair (default 0d0N, Final 6d0N));
//   2. every outfit texel of the given slots gets its 3D position (its triangle rasterised in UV space, bind pose, on a decision
//      grid of at most 1024 x 1024);
//   3. the outfit's PIECES: 4-connected grid cells of one colour region (the outfit's own panels: a UV chart of one cloth colour,
//      cut at the outfit's own seams and colour edges);
//   4. each piece votes with its cells: the nearest KH3 cloth point within RADIUS says dark (KH3's Final colour there has value
//      < DARK) or light; a piece with at least SHARE (2 / 3) of its votes dark is DARK. No vote: not dark (the region rule stands).
// Returns a texel mask (the REGION of the texel's piece where that piece is dark, else 0: the caller applies it to texels of that
// region only, so a stitch of another colour inside a dark piece keeps its own rule), used by tools/sora_outfit_formcolours.js for wear form 6 only.
'use strict';
const fs = require('fs');
const path = require('path');
const bc = require('./bcdec');
const sp = require('../sora_pack.js');
const SFS = require('./second_form_swap.js');

// SHARE: KH3's Final is a white costume with dark panels, so a piece is dark only where KH3 is dark at two thirds of its place
const RADIUS = SFS.RADIUS, DARK = 0.3, SHARE = 2 / 3, DECIDE_MAX = 1024;

// KH3 Sora's cloth points with KH3's Final Form colour (`sf`) and default colour (`def`)
function kh3FinalPoints(texRoot, soraGlb) {
  const dir = path.join(texRoot, 'Character', 'pc', 'p_ex001', 'mdl', 'mat0');
  const pairs = [0, 1, 2].map((k) => ({ def: bc.decodeDds(fs.readFileSync(path.join(dir, `p_ex001_0d0${k}.dds`)), 0), sf: bc.decodeDds(fs.readFileSync(path.join(dir, 'FormChangeTex', `p_ex001_6d0${k}.dds`)), 0) }));
  return SFS.kh3Points(soraGlb, pairs);
}

// texture `src` of the outfit (slots `slotNames` of `modelGlb`), `regionOf(r, g, b)` (0 = keep), `points` from kh3FinalPoints
function darkPieces(src, modelGlb, slotNames, regionOf, points) {
  const t = bc.decodePng(fs.readFileSync(src)); const W = t.width, H = t.height, C = t.channels;
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
  // the pieces
  const cellReg = new Int8Array(GW * GH).fill(-1);
  for (let y = 0; y < GH; y++) for (let x = 0; x < GW; x++) { const gi = y * GW + x; if (Number.isNaN(posGrid[gi * 3])) continue; const s = (Math.min(H - 1, y * sc + (sc >> 1)) * W + Math.min(W - 1, x * sc + (sc >> 1))) * C; const r = regionOf(t.data[s], t.data[s + 1], t.data[s + 2]); if (r) cellReg[gi] = r; }
  const piece = new Int32Array(GW * GH).fill(-1); const dark = []; const pieceReg = []; const list = [];
  let pieces = 0, darkCells = 0, voted = 0;
  for (let start = 0; start < GW * GH; start++) {
    if (cellReg[start] < 0 || piece[start] >= 0) continue;
    const id = dark.length; const reg = cellReg[start]; const stack = [start]; piece[start] = id; const cells = [];
    while (stack.length) { const c = stack.pop(); cells.push(c); const x = c % GW, y = (c / GW) | 0; for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) { const nx = x + dx, ny = y + dy; if (nx < 0 || ny < 0 || nx >= GW || ny >= GH) continue; const nc = ny * GW + nx; if (piece[nc] < 0 && cellReg[nc] === reg) { piece[nc] = id; stack.push(nc); } } }
    let d = 0, n = 0;
    for (const c of cells) {
      const x = c % GW, y = (c / GW) | 0; if (cells.length > 16 && ((x | y) & 1)) continue;
      let best = null; for (const [q, dd] of points.near(posGrid[c * 3], posGrid[c * 3 + 1], posGrid[c * 3 + 2])) if (!best || dd < best[1]) best = [q, dd];
      if (!best) continue; n++; if (Math.max(...best[0].sf) / 255 < DARK) d++;
    }
    const isDark = n > 0 && d / n >= SHARE; dark.push(isDark); pieceReg.push(reg); list.push({ region: reg, cells: cells.length, votes: n, dark_share: n ? +(d / n).toFixed(3) : null, dark: isDark, at: n ? posGrid.subarray(cells[0] * 3, cells[0] * 3 + 3).map((v) => Math.round(v * 100) / 100) : null }); pieces++; if (n) voted++; if (isDark) darkCells += cells.length;
  }
  const mask = new Uint8Array(W * H);
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) { const p = piece[Math.floor(y / sc) * GW + Math.floor(x / sc)]; if (p >= 0 && dark[p]) mask[y * W + x] = pieceReg[p]; }
  return { mask, list: list.filter((p) => p.cells >= 400).sort((a, b) => b.cells - a.cells), pieces, voted, dark_pieces: dark.filter(Boolean).length, dark_share: +(darkCells / Math.max(1, cellReg.filter((r) => r > 0).length)).toFixed(3) };
}

module.exports = { kh3FinalPoints, darkPieces, RADIUS, DARK, SHARE };
