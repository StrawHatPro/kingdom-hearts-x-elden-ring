// (KEYBLADES, the user 2026-11-03: "make sure KH1 and KH2 Sora can look like rage form as well") KH3's RAGE FORM LOOK, BAKED.
//
// How KH3 draws Rage Form [A, translated shader]: Rage Form is wear form 7 "Anti" (p_ex001_WearformSet m_Forms[7]); its costume
// textures are the default's. The look is the ANTI MODE of every material of Sora (p_ex001_Basemat and the skin / face / hair /
// eye materials: static switch UseAntiMode), driven at run time by ONE scalar, AntiVolume (0 -> 1; native code fades it,
// docs/sora-vfx-spec.md): in the translated base pass (build/sora_pack/materials/materials.wgsl KH_C8, lines ~446..455)
//   lum  = dot(albedo, (0.3, 0.59, 0.11))                      (linear)
//   dark = lum * AntiDarknessColorMulti (0.04, 0.5, 1.0)        (a dark blue monochrome of the texel)
//   albedo = lerp(albedo, dark, grade)                           grade = the darkness front: distance from the head point
//            (atama + AntiVolumePositionOffset*) / AntiVolumeGradeLength 170 cm, contrast AntiVolumeGradeContrast, reversed;
//            it covers the whole body at AntiVolume 1
// + the face (UseAntiFaceDark: lum -> 0.008 over AntiFaceDarkVolume 15 cm from AntiFaceDarkOffset), + the eye lens
// (AntiEyeMaterial 1: red emissive AntiEyeEmissiv_L (10, 0.2, 0) / _H (5, 1, -0.5), AntiEyeWPO 0.4), + the PATTERN: the alpha of
// AntiPointMaskTex = p_ex001_7f00, sampled with the mesh's SECOND UV set where that set lies in UV tile (1, 0), times
// AntiMaskEmissiv (p_ex001_mat0 / mat1: (10, 15, 30): a blue-white glow), scrolled by AntiMaskEmissivSpeed (0.2). The aura
// particles: ead_p_ant_com (vp_ant_bdy_ora00, vp_ant_eye_ora00, ...), PACK/vfx.
//
// Here, for an outfit with other shaders (KH1, KH2): the darkness on every base colour texel of its body slots (the formula
// above at AntiVolume 1), and KH3's pattern carried onto the outfit's own UVs: every vertex of the outfit's mesh takes the
// second-UV coordinate of the nearest KH3 Sora cloth vertex (p_ex001_mat0 / mat1, bind poses, within MAX_DIST); a triangle
// whose three corners map into tile (1, 0) close together is rasterised in the outfit's first UV space and samples 7f00's
// alpha there. The glow is baked into the base colour (so the look reads without an emissive input) and written as a mask
// (<name>_kbf7_glow.png: red = the pattern) for a renderer that draws it as emission (documentation/KH3_WEAPON_ASSETS.md).
'use strict';
const fs = require('fs');
const path = require('path');
const bc = require('./bcdec');
const sp = require('../sora_pack.js');

const MAX_DIST = 0.06; // metres
const CELL = 0.03;
const lin = (c) => { const x = c / 255; return x <= 0.04045 ? x / 12.92 : Math.pow((x + 0.055) / 1.055, 2.4); };
const enc = (x) => { const c = Math.max(0, Math.min(1, x)); return Math.round(255 * (c <= 0.0031308 ? c * 12.92 : 1.055 * Math.pow(c, 1 / 2.4) - 0.055)); };

// KH3's own values (the main pack's translated library: the owner's default vector and the cloth instance's override)
function antiParams(mainMaterials) {
  const owner = mainMaterials.owners.p_ex001_mat0_Inst;
  const def = (name) => { const v = (owner.vec || []).find((e) => e && e.v === name); return v ? v.d : null; };
  const inst = mainMaterials.materials.p_ex001_mat0 || { vectors: {}, scalars: {} };
  return { darkness: (def('AntiDarknessColorMulti') || [0.04, 0.5, 1, 0]).slice(0, 3), emissive: (inst.vectors.AntiMaskEmissiv || def('AntiMaskEmissiv') || [10, 15, 30, 1]).slice(0, 3),
    eye_low: (def('AntiEyeEmissiv_L') || [10, 0.2, 0, 1]).slice(0, 3), eye_high: (def('AntiEyeEmissiv_H') || [5, 1, -0.5, 1]).slice(0, 3), speed: inst.scalars.AntiMaskEmissivSpeed ?? 0.2 };
}

// the KH3 source: positions and second UVs of the glowing cloth slots (p_ex001_mat0 / mat1) of sora.glb
function kh3Source(soraGlb) {
  const g = sp.readGlb(soraGlb); const pts = []; const uvs = [];
  for (const p of g.json.meshes[0].primitives) {
    const name = g.json.materials[p.material].name;
    if (!/^p_ex001_mat[01]$/.test(name) || p.attributes.TEXCOORD_1 === undefined) continue;
    const pos = sp.readAccessor(g, p.attributes.POSITION).data, uv = sp.readAccessor(g, p.attributes.TEXCOORD_1).data;
    for (let i = 0; i < pos.length / 3; i++) { pts.push(pos[i * 3], pos[i * 3 + 1], pos[i * 3 + 2]); uvs.push(uv[i * 2], uv[i * 2 + 1]); }
  }
  const grid = new Map(); const key = (x, y, z) => `${Math.floor(x / CELL)},${Math.floor(y / CELL)},${Math.floor(z / CELL)}`;
  for (let i = 0; i < pts.length / 3; i++) { const k = key(pts[i * 3], pts[i * 3 + 1], pts[i * 3 + 2]); if (!grid.has(k)) grid.set(k, []); grid.get(k).push(i); }
  const nearest = (x, y, z) => {
    let best = -1, bd = MAX_DIST * MAX_DIST; const r = Math.ceil(MAX_DIST / CELL);
    const cx = Math.floor(x / CELL), cy = Math.floor(y / CELL), cz = Math.floor(z / CELL);
    for (let a = -r; a <= r; a++) for (let b = -r; b <= r; b++) for (let c = -r; c <= r; c++) for (const i of grid.get(`${cx + a},${cy + b},${cz + c}`) || []) { const d = (pts[i * 3] - x) ** 2 + (pts[i * 3 + 1] - y) ** 2 + (pts[i * 3 + 2] - z) ** 2; if (d < bd) { bd = d; best = i; } }
    return best < 0 ? null : [uvs[best * 2], uvs[best * 2 + 1]];
  };
  return { nearest, vertices: pts.length / 3 };
}

// the pattern mask of one base colour texture (W x H) from the outfit primitives that use it
function patternMask(modelGlb, slotsOfTexture, W, H, source, pattern) {
  const g = sp.readGlb(modelGlb); const mask = new Float32Array(W * H); let tris = 0, mapped = 0, total = 0;
  for (const p of g.json.meshes[0].primitives) {
    if (!slotsOfTexture.includes(g.json.materials[p.material].name)) continue;
    const pos = sp.readAccessor(g, p.attributes.POSITION).data, uv = sp.readAccessor(g, p.attributes.TEXCOORD_0).data, ix = sp.readAccessor(g, p.indices).data;
    const n = pos.length / 3; const m = new Array(n);
    for (let i = 0; i < n; i++) { m[i] = source.nearest(pos[i * 3], pos[i * 3 + 1], pos[i * 3 + 2]); total++; if (m[i]) mapped++; }
    const inTile = (q) => q && q[0] >= 1 && q[0] < 2 && q[1] >= 0 && q[1] < 1;
    for (let t = 0; t < ix.length; t += 3) {
      const [a, b, c] = [ix[t], ix[t + 1], ix[t + 2]];
      if (!inTile(m[a]) || !inTile(m[b]) || !inTile(m[c])) continue;
      const d = (q, r) => Math.hypot(q[0] - r[0], q[1] - r[1]);
      if (Math.max(d(m[a], m[b]), d(m[b], m[c]), d(m[c], m[a])) > 0.08) continue; // a seam of KH3's second UVs: skip
      tris++;
      const P = [a, b, c].map((v) => [uv[v * 2] * W, uv[v * 2 + 1] * H]);
      const area = (P[1][0] - P[0][0]) * (P[2][1] - P[0][1]) - (P[1][1] - P[0][1]) * (P[2][0] - P[0][0]); if (Math.abs(area) < 1e-9) continue;
      const x0 = Math.max(0, Math.floor(Math.min(P[0][0], P[1][0], P[2][0]))), x1 = Math.min(W - 1, Math.ceil(Math.max(P[0][0], P[1][0], P[2][0])));
      const y0 = Math.max(0, Math.floor(Math.min(P[0][1], P[1][1], P[2][1]))), y1 = Math.min(H - 1, Math.ceil(Math.max(P[0][1], P[1][1], P[2][1])));
      for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) {
        const px = x + 0.5, py = y + 0.5;
        const w0 = ((P[1][0] - px) * (P[2][1] - py) - (P[1][1] - py) * (P[2][0] - px)) / area, w1 = ((P[2][0] - px) * (P[0][1] - py) - (P[2][1] - py) * (P[0][0] - px)) / area, w2 = 1 - w0 - w1;
        if (w0 < -0.01 || w1 < -0.01 || w2 < -0.01) continue;
        const u1 = m[a][0] * w0 + m[b][0] * w1 + m[c][0] * w2 - 1, v1 = m[a][1] * w0 + m[b][1] * w1 + m[c][1] * w2;
        const sx = Math.min(pattern.width - 1, Math.max(0, Math.floor(u1 * pattern.width))), sy = Math.min(pattern.height - 1, Math.max(0, Math.floor(v1 * pattern.height)));
        const val = pattern.rgba[(sy * pattern.width + sx) * 4 + 3] / 255;
        if (val > mask[y * W + x]) mask[y * W + x] = val;
      }
    }
  }
  return { mask, triangles: tris, vertices_mapped: mapped, vertices: total };
}

// one base colour texture in Rage Form: KH3's darkness on every texel, KH3's pattern glowing on top
function rageTexture(src, out, glowOut, mask, params, faceDark = 0) {
  const t = bc.decodePng(fs.readFileSync(src)); const W = t.width, H = t.height, C = t.channels;
  const px = Buffer.alloc(W * H * 4), glow = Buffer.alloc(W * H * 4);
  const em = params.emissive, emMax = Math.max(...em); const emColour = em.map((x) => x / emMax); // the glow's hue at full value
  let lit = 0;
  for (let i = 0; i < W * H; i++) {
    const s = i * C; const r = lin(t.data[s]), g = lin(t.data[s + 1]), b = lin(t.data[s + 2]); const a = C > 3 ? t.data[s + 3] : 255;
    // the face (UseAntiFaceDark): its luminance pulled to 0.008 by the face-dark gradient (AntiFaceDarkOffset 0.3 + the distance
    // over AntiFaceDarkVolume 15 cm): faceDark = that gradient taken as one value for the whole face texture
    const lum = (0.3 * r + 0.59 * g + 0.11 * b) * (1 - faceDark) + 0.008 * faceDark;
    let o = params.darkness.map((k) => lum * k);
    const k = mask ? mask[i] : 0;
    if (k > 0) { o = o.map((x, c) => x + (emColour[c] - x) * k); lit++; }
    for (let c = 0; c < 3; c++) px[i * 4 + c] = enc(o[c]);
    px[i * 4 + 3] = a;
    glow[i * 4] = Math.round(255 * k); glow[i * 4 + 3] = 255;
  }
  fs.writeFileSync(out, bc.encodePng(W, H, px, { alpha: true, srgb: true }));
  if (glowOut) fs.writeFileSync(glowOut, bc.encodePng(W, H, glow, { alpha: false, srgb: false }));
  return { width: W, height: H, pattern_texels: lit };
}

function readPattern(contentRoot) {
  return bc.decodeDds(fs.readFileSync(path.join(contentRoot, 'Character', 'pc', 'p_ex001', 'mdl', 'mat0', 'FormChangeTex', 'p_ex001_7f00.dds')), 0);
}

module.exports = { antiParams, kh3Source, patternMask, rageTexture, readPattern, MAX_DIST };
