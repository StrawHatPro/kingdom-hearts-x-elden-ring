// (ER91, the user: "the tips of Sora's hair is also still broken, it needs to look like this image") THE HAIR MASK FIXED
// for the ER91 hair path. Input: a translated materials WGSL (er87/gen: the udiv remainder already restored) and the
// pack's materials.json; output: the same WGSL with
//  1. THE STRAND MASK'S SAMPLER (the cause of the card-wide blunt ends): SQEX_KHHairBaseMat's strand-end mask
//     (hairlineendmask2, "HairLineMaskTex") is CLAMP in U (along the strand) and WRAP in V (across the strands). The
//     translator picks one shared sampler from address_x alone, so it was sampled CLAMP in both; the hair's second UV set
//     spans v -1.3 .. 2.3 (x HairMaskUVRange 5), so every card outside v*5 in [0, 1] read the mask's first or last row:
//     one strand profile for the whole card (a blunt card-wide end, the "card edges") instead of the 16 tapered strands per
//     repeat. Every textureSample whose texture is clamp-U / wrap-V (or wrap-U / clamp-V) now samples through
//     kh91_sample_cw / kh91_sample_wc: the wrap sampler with the clamped axis held inside [h, 1 - h], h = the footprint
//     (anisotropic taps + the bilinear half texel at the coarsest mip used), with the explicit gradients of the coordinate.
//  2. (--phase) ER87's dither phase input: d = i.v137.z when i.v137.y > 0 (the 5-phase DitherTemporalAA remainder).
//  3. (--margin) the mask's final test returns its margin (4 + value - clip) when i.v137.w > 0, so the host can find
//     where the margin crosses 0 over d (a continuous coverage, char_common.wgsl KH_HAIR91).
//  4. (--fade one|keep) the strands' view-angle fade: `one` = ER87's (held at 1), `keep` = KH3's term (default keep).
// Usage: node hair91.js SRC.wgsl materials.json DST.wgsl [--phase] [--margin] [--fade one|keep]
const fs = require('fs');
const [src, json, dst, ...flags] = process.argv.slice(2);
const has = (f) => flags.includes(f);
const fade = flags.includes('--fade') ? flags[flags.indexOf('--fade') + 1] : 'keep';
const mats = JSON.parse(fs.readFileSync(json, 'utf8').replace(/^﻿/, ''));
// owner id -> per texture index: 'cw' (clamp U, wrap V) | 'wc' | null
const modes = new Map();
const conflicts = [];
for (const m of Object.values(mats.materials || {})) {
  if (m.id === undefined || !Array.isArray(m.textures)) continue;
  const row = modes.get(m.id) || [];
  m.textures.forEach((t, k) => {
    const mode = !t || t.missing ? null : t.address_x === 'TA_Clamp' && t.address_y !== 'TA_Clamp' ? 'cw' : t.address_x !== 'TA_Clamp' && t.address_y === 'TA_Clamp' ? 'wc' : null;
    if (row[k] !== undefined && row[k] !== mode) conflicts.push(`owner ${m.id} t${k}: ${row[k]} / ${mode}`);
    if (row[k] === undefined || mode) row[k] = mode;
  });
  modes.set(m.id, row);
}
const lines = fs.readFileSync(src, 'utf8').split('\n');
let owner = null, fn = null;
const report = { cw: 0, wc: 0, phase: 0, margin: 0, fade: 0 };
const touched = [];
for (let k = 0; k < lines.length; k++) {
  const l = lines[k];
  const o = /^#ifdef KH_C(\d+)\b/.exec(l);
  if (o) owner = +o[1];
  const f = /^fn (\w+)\(/.exec(l);
  if (f) fn = f[1];
  if (owner === null) continue;
  const row = modes.get(owner) || [];
  // 1. the sampler of a texture with mixed address modes
  lines[k] = lines[k].replace(/textureSample\(kh_t(\d+), (kh_sclamp|kh_swrap), /g, (all, t, s) => {
    const mode = row[+t];
    if (mode === 'cw' && s === 'kh_sclamp') { report.cw++; touched.push(`KH_C${owner} ${fn} t${t} cw`); return `kh91_sample_cw(kh_t${t}, `; }
    if (mode === 'wc' && s === 'kh_swrap') { report.wc++; touched.push(`KH_C${owner} ${fn} t${t} wc`); return `kh91_sample_wc(kh_t${t}, `; }
    return all;
  });
}
// 4. the view-angle fade (ER87's hair_fade.js rule)
let inMask = false;
for (let k = 0; k < lines.length; k++) {
  if (/^fn kh_mask\(/.test(lines[k])) inMask = true; else if (/^fn /.test(lines[k])) inMask = false;
  const m = /^    (r\d\.[xyzw]) = saturate\(\(\1 \* 3\.0 \+ \(-1\.0\)\)\);$/.exec(lines[k]);
  if (fade === 'one' && inMask && m && /^    r\d\.[xyzw] = \(\(-abs\(r\d\.[xyzw]\)\) \* r\d\.[xyzw] \+ 1\.0\);$/.test(lines[k - 1])) {
    lines[k] = `    ${m[1]} = 1.0; // (ER87) the view-angle fade of the strands kept at 1`;
    report.fade++;
  }
}
// 2. the dither phase input; 3. the margin of the final test of those masks
for (let k = 2; k < lines.length; k++) {
  const m = /^    (r\d\.[xyzw]) = \((r\d\.[xyzw]) \* 0\.166667\);$/.exec(lines[k]);
  if (!(has('--phase') && m && m[1] === m[2] && /^    \{ let ua = r\d\.[xyzw]; let ub = 5\.0;/.test(lines[k - 2]))) continue;
  lines.splice(k + 1, 0, `    if (i.v137.y > 0.0) { ${m[1]} = i.v137.z; } // (ER91) the dither phase handed in`);
  report.phase++;
  if (!has('--margin')) continue;
  // the end of this kh_mask: `rX = (rX + (-CLIP)); rX = select(0.0, 1.0, rX < 0.0); if (rX != 0.0) { return 0.0; } return 1.0;`
  for (let j = k + 2; j < lines.length; j++) {
    if (/^fn /.test(lines[j])) break;
    const t = /^    (r\d\.[xyzw]) = \(\1 \+ \(-([0-9.]+)\)\);$/.exec(lines[j]);
    if (t && lines[j + 1] === `    ${t[1]} = select(0.0, 1.0, ${t[1]} < 0.0);` && lines[j + 2] === `    if (${t[1]} != 0.0) { return 0.0; }` && lines[j + 3] === '    return 1.0;') {
      lines.splice(j + 1, 0, `    if (i.v137.w > 0.0) { return 4.0 + ${t[1]}; } // (ER91) the margin of the final test`);
      report.margin++;
      break;
    }
  }
}
const helpers = `// (ER91) samplers for textures whose two axes have different address modes (tools: er91/tools/hair91.js)
fn kh91_hold(c: f32, g: f32, size: f32, major: f32) -> f32 {
    let lod = ceil(log2(max(major, 1.0)));
    let h = min(0.5 * g + 0.5 * exp2(lod) / size, 0.5);
    return clamp(c, h, 1.0 - h);
}
fn kh91_sample_cw(t: texture_2d<f32>, uv: vec2<f32>) -> vec4<f32> {
    let gx = dpdx(uv);
    let gy = dpdy(uv);
    let size = vec2<f32>(textureDimensions(t, 0));
    let major = max(length(gx * size), length(gy * size));
    let u = kh91_hold(uv.x, max(abs(gx.x), abs(gy.x)), size.x, major);
    return textureSampleGrad(t, kh_swrap, vec2<f32>(u, uv.y), gx, gy);
}
fn kh91_sample_wc(t: texture_2d<f32>, uv: vec2<f32>) -> vec4<f32> {
    let gx = dpdx(uv);
    let gy = dpdy(uv);
    let size = vec2<f32>(textureDimensions(t, 0));
    let major = max(length(gx * size), length(gy * size));
    let v = kh91_hold(uv.y, max(abs(gx.y), abs(gy.y)), size.y, major);
    return textureSampleGrad(t, kh_swrap, vec2<f32>(uv.x, v), gx, gy);
}
`;
fs.writeFileSync(dst, helpers + lines.join('\n'));
console.log(`${src} -> ${dst}: samplers cw ${report.cw} wc ${report.wc}; phase ${report.phase}; margin ${report.margin}; fade=1 ${report.fade}${conflicts.length ? '; CONFLICTS ' + conflicts.join(', ') : ''}`);
for (const t of [...new Set(touched)]) console.log('  ' + t);
