#!/usr/bin/env node
// tools/vfx_cube_pack.js (VFX-2): THE EFFECT MATERIALS' CUBE MAPS, packed so the effect pipeline can read them.
//
// tools/sora_vfx_pack.js leaves a TextureCube parameter without a file (`"file": null`: tools/lib/bcdec.js decoded 2D only),
// so a material that samples a cube map (Dead of Night's matcap shards MI_mcp_01/02_ma: Cubemap ztx_hdr_01.test_hdr_01, the
// water's ReflectionTexture T_ca_Sky) could not be drawn as KH3 draws it. This tool decodes each cube (BC6H HDR, bcdec.js
// decodeBc6hCube) and writes its six faces side by side as RGBM PNG (`textures/<name>.cube.png`, 6N x N, +X -X +Y -Y +Z -Z,
// rgb x a x 8 = linear colour), and records it in the root's `textures/cubes.json` (texture asset -> file, face size, range,
// the decoder's mip check). The material records are NOT rewritten: the translator (tools/sora_vfx_shaders.js --root / --vxl)
// reads cubes.json for the cube slots of the tables it writes. New files only.
//
//   node tools/vfx_cube_pack.js --pack ROOT [--out DIR]     ROOT: an effect root (build/sora_pack/keyblades/vfx, build/sora_pack/vfx)
//                                                            DIR:  where to write (default ROOT): textures/<name>.cube.png, cubes.json
'use strict';
const fs = require('fs');
const path = require('path');
const bc = require('./lib/bcdec');

const ROOT = path.resolve(__dirname, '..');
const arg = (k, d) => { const i = process.argv.indexOf(k); return i < 0 ? d : process.argv[i + 1]; };
const PACK = path.resolve(arg('--pack', path.join(ROOT, 'build/sora_pack/keyblades/vfx')));
const OUT = path.resolve(arg('--out', PACK));
const TEX = path.join(ROOT, 'assets_converted', 'textures', 'KINGDOM HEARTS III', 'Content');

// "/Game/A/b.c" -> the converted DDS (named <package>.<export>.dds when the names differ)
function ddsOf(ref) {
  const m = /^\/Game\/(.*)\.([^.]+)$/.exec(ref); if (!m) return null;
  const dir = path.join(TEX, path.dirname(m[1])), base = path.basename(m[1]);
  for (const f of [base + '.dds', base + '.' + m[2] + '.dds']) if (fs.existsSync(path.join(dir, f))) return path.join(dir, f);
  let all = []; try { all = fs.readdirSync(dir); } catch (e) { /* */ }
  const hit = all.find((f) => f.toLowerCase().startsWith(base.toLowerCase() + '.') && f.toLowerCase().endsWith('.dds'));
  return hit ? path.join(dir, hit) : null;
}

const cubes = new Map(); // asset -> [material.parameter]
for (const f of fs.readdirSync(path.join(PACK, 'materials')).filter((n) => n.endsWith('.json'))) {
  const m = JSON.parse(fs.readFileSync(path.join(PACK, 'materials', f), 'utf8'));
  const entries = [...Object.entries(m.textures || {}), ...(m.unnamed_textures || []).map((t, k) => ['#' + k, t])];
  for (const [k, t] of entries) if (/Cube/.test(t.expression || '') && t.texture) (cubes.get(t.texture) || cubes.set(t.texture, []).get(t.texture)).push(f.replace(/\.json$/, '') + '.' + k);
}
const table = { format: 'kh3-sora-vfx-cubes/1', generator: 'tools/vfx_cube_pack.js (VFX-2)', layout: 'six faces side by side, +X -X +Y -Y +Z -Z (D3D face order), each N x N, rows from the top', encoding: `RGBM: linear colour = rgb x a x ${bc.CUBE_RGBM_RANGE}`, range: bc.CUBE_RGBM_RANGE, cubes: {} };
fs.mkdirSync(path.join(OUT, 'textures'), { recursive: true });
for (const [asset, users] of cubes) {
  const dds = ddsOf(asset);
  if (!dds) { table.cubes[asset] = { users, error: 'no converted texture' }; console.log('NOT PACKED', asset, '(no converted texture)', users.join(', ')); continue; }
  try {
    const buf = fs.readFileSync(dds);
    const c0 = bc.decodeBc6hCube(buf, 0);
    let check = null;
    if (c0.mipCount > 1 && c0.size >= 8) { // the top mip box-filtered against the next one (a wrong mode table shows here)
      const c1 = bc.decodeBc6hCube(buf, 1), N = c1.size; let s = 0, n = 0, mean = 0;
      for (let f = 0; f < 6; f++) for (let y = 0; y < N; y++) for (let x = 0; x < N; x++) for (let c = 0; c < 3; c++) {
        const a = c0.faces[f], at = (yy, xx) => a[(yy * 2 * N + xx) * 3 + c];
        const v = (at(2 * y, 2 * x) + at(2 * y, 2 * x + 1) + at(2 * y + 1, 2 * x) + at(2 * y + 1, 2 * x + 1)) / 4;
        s += Math.abs(v - c1.faces[f][(y * N + x) * 3 + c]); mean += v; n++;
      }
      check = { mean_abs_difference: +(s / n).toFixed(5), mean_value: +(mean / n).toFixed(5) };
    }
    const at = bc.cubeAtlasRgbm(c0);
    const name = path.basename(dds).replace(/\.dds$/i, '').split('.').pop() + '.cube.png';
    fs.writeFileSync(path.join(OUT, 'textures', name), bc.encodePng(at.width, at.height, at.rgba, { alpha: true, srgb: false }));
    table.cubes[asset] = { file: 'textures/' + name, face: c0.size, users, mip_check: check };
    console.log('packed', asset, '->', name, c0.size + 'x6', 'mip check', JSON.stringify(check), users.join(', '));
  } catch (e) { table.cubes[asset] = { users, error: e.message }; console.log('NOT PACKED', asset, e.message); }
}
fs.writeFileSync(path.join(OUT, 'textures', 'cubes.json'), JSON.stringify(table, null, 1));
console.log(`${Object.keys(table.cubes).length} cube map(s) named by ${PACK}; table ${path.join(OUT, 'textures', 'cubes.json')}`);
