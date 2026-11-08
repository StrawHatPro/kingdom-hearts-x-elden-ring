#!/usr/bin/env node
// tools/vfx_xlate_overlay.js (VFX-XLATE): THE MAIN EFFECT TABLE'S VXL OVERLAY, new files only (the live table and its built
// shaders stay as they are; the deployed DLLs before VFX-XLATE never read the overlay).
//
// tools/sora_vfx_shaders.js translates scene-colour reads (blur / warp materials), material parameter collections, Primitive
// [0..7] / [15..16], the particle scene colours and View[160] only for an effect root's own table (`--root`) or a table
// written elsewhere (`--vxl --out DIR`). The main table (build/sora_pack/vfx) is read by the deployed DLL, so its owners those
// rules now take are laid over it instead: kh-er-render vfx.rs reads PACK/vfx/shaders/shading_vxl.json (owner and material
// records, `pixel_suffix`) and PACK/vfx/er/shaders_vxl.json (+ the `<letter><id>_vxl` shader files) after the XW-d overlay.
//
//   node tools/vfx_xlate_overlay.js --a DIR_A --b DIR_B --built DIR_B_ER --out OUT
//     DIR_A:      shaders/ of `sora_vfx_shaders.js --out DIR_A` (today's translator, the rules off)
//     DIR_B:      shaders/ of `sora_vfx_shaders.js --vxl --out DIR_B` (the rules on; owner ids must equal the live table's)
//     DIR_B_ER:   er/ of `kh-shaderc --vfx --pack` over DIR_B's pack folder
//     OUT:        a pack-shaped folder: OUT/vfx/shaders/shading_vxl.json, OUT/vfx/er/shaders_vxl.json, OUT/vfx/er/*_vxl.*
// The owners taken: every owner with a class (sprite / mesh / decal) translated in B but not in A (the rules' own gain).
//
// (PARTY-NATIVE) THE LIT TRANSLUCENTS' OVERLAY `_fwl` (any root; without these options the tool does exactly what it did):
//   --take light       the owners taken are those whose sprite / mesh block in B reads a light input (the forward lighting
//                      `er.fwd_*` / `kh_fwd_*`, the ILC `kh_fwd_ilc`, the scene colours i.fbright / i.fdark / i.pbright /
//                      i.pdark: kh-shaderc's `reads_forward_light`), DIR_B from `sora_vfx_shaders.js --fwl --out DIR_B`;
//                      only their sprite / mesh classes are laid over: the owner record is the live one with those two
//                      classes (and B's `xl`) replaced, its decal class and the decal's shader stay the live table's; no --a
//   --live ROOT        the live root (default build/sora_pack/vfx; e.g. build/sora_pack/party/vfx): its shaders/shading.json
//                      gives the ids checked, its shading_*.json overlays are checked for collisions (an overlay owner record
//                      or a pixel_suffix key of a taken owner's sprite / mesh / vertex / distortion shader: an error)
//   --suffix _fwl      the overlay's suffix (default _vxl): shading<s>.json, er/shaders<s>.json, materials<s>.wgsl, files
//                      `<name><s>.*` (with --take light also the fog `f<letter><id>` and lit `l<letter><id>` variants, which the
//                      renderer finds by file name: f / l + the pixel file)
//   --prepare DIR      (with --take light, instead of --built / --out) write DIR/vfx/shaders/{shading.json, materials.wgsl}: B
//                      with only the taken owners and their sprite / mesh classes, for `kh-shaderc --vfx --pack DIR`
'use strict';
const fs = require('fs');
const path = require('path');
const ROOT = path.resolve(__dirname, '..');
const has = (k) => process.argv.includes(k);
const opt = (k) => { const i = process.argv.indexOf(k); return i < 0 ? null : process.argv[i + 1]; };
const arg = (k) => { const v = opt(k); if (v === null) throw new Error('give ' + k); return path.resolve(v); };
const TAKE_LIGHT = opt('--take') === 'light'; // (PARTY-NATIVE)
if (opt('--take') !== null && !TAKE_LIGHT) throw new Error('--take: only `light`');
const PREPARE = opt('--prepare') !== null ? path.resolve(opt('--prepare')) : null; // (PARTY-NATIVE)
if (PREPARE && !TAKE_LIGHT) throw new Error('--prepare needs --take light');
const B = arg('--b');
const A = TAKE_LIGHT ? null : arg('--a');
const BUILT = PREPARE ? null : arg('--built'), OUT = PREPARE ? null : arg('--out');
const LIVE = opt('--live') !== null ? path.resolve(opt('--live')) : path.join(ROOT, 'build/sora_pack/vfx'); // (PARTY-NATIVE)
const SUFFIX = opt('--suffix') || '_vxl'; // (PARTY-NATIVE)
if (!/^_[a-z0-9]+$/.test(SUFFIX)) throw new Error('--suffix: `_` and lower-case letters / digits');
const read = (f) => JSON.parse(fs.readFileSync(f, 'utf8'));
const b = read(path.join(B, 'shading.json')), live = read(path.join(LIVE, 'shaders/shading.json'));
const a = A ? read(path.join(A, 'shading.json')) : null;
for (const [k, o] of Object.entries(b.owners)) { const l = live.owners[k]; if (!l || l.id !== o.id || l.asset !== o.asset) throw new Error(`owner ${k}: id ${o.id} here, ${l ? l.id : 'none'} in the live table: the overlay would name the wrong shaders`); }

// (PARTY-NATIVE) --take light: the owners whose sprite / mesh block reads a light input (kh-shaderc vfx.rs reads_forward_light)
const LIGHT_KEYS = ['er.fwd_', 'kh_fwd_', 'i.fbright', 'i.fdark', 'i.pbright', 'i.pdark'];
const generated = TAKE_LIGHT ? fs.readFileSync(path.join(B, 'materials.wgsl'), 'utf8').replace(/\r\n/g, '\n') : '';
const blockOf = (id, tag) => { const m = `#ifdef KH_S${id}_${tag}\n`; const at = generated.indexOf(m); if (at < 0) return null; const rest = generated.slice(at); const end = rest.indexOf('\n#endif'); return end < 0 ? rest : rest.slice(0, end + 7); };
const LIGHT_CLASSES = [['sprite', 'SPRITE', 's'], ['mesh', 'MESH', 'm']];
const litClasses = (o) => LIGHT_CLASSES.filter(([c, tag]) => o[c] && o[c].ok && (blockOf(o.id, tag) || '').split('\n').some((l) => LIGHT_KEYS.some((k) => l.includes(k))));
const taken = TAKE_LIGHT
  ? Object.entries(b.owners).filter(([, o]) => litClasses(o).length)
  : Object.entries(b.owners).filter(([k, o]) => ['sprite', 'mesh', 'decal'].some((c) => o[c] && o[c].ok && !(a.owners[k] && a.owners[k][c] && a.owners[k][c].ok)));

if (TAKE_LIGHT) { // (PARTY-NATIVE) collisions with the root's other overlays
  const mine = new Set(taken.flatMap(([, o]) => ['s', 'm', 'v', 'xs', 'xm'].map((p) => p + o.id)));
  for (const f of fs.readdirSync(path.join(LIVE, 'shaders')).filter((f) => /^shading_[a-z0-9]+\.json$/.test(f))) {
    const t = read(path.join(LIVE, 'shaders', f));
    for (const [k] of taken) if (t.owners && t.owners[k]) throw new Error(`${f} has an owner record of ${k}: lay the overlays in one order first`);
    for (const k of Object.keys(t.pixel_suffix || {})) if (mine.has(k)) throw new Error(`${f} gives ${k} a suffix of its own (${t.pixel_suffix[k]})`);
    for (const [m, r] of Object.entries(b.materials)) if (t.materials && t.materials[m] && taken.some(([k]) => k === r.owner)) console.log(`note: ${f} has a record of material ${m} (owner ${r.owner}); this overlay's record replaces it when laid after it`);
  }
}

if (PREPARE) { // (PARTY-NATIVE) the build pack: B's taken owners, only their sprite / mesh classes
  const owners = {};
  for (const [k, o] of taken) { const r = { ...o, decal: { ok: false, why: '(PARTY-NATIVE) the decal class stays the live table\'s' } }; for (const [c] of LIGHT_CLASSES) if (!litClasses(o).some(([x]) => x === c)) r[c] = { ok: false, why: '(PARTY-NATIVE) reads no light input' }; owners[k] = r; }
  const materials = Object.fromEntries(Object.entries(b.materials).filter(([, r]) => owners[r.owner]));
  fs.mkdirSync(path.join(PREPARE, 'vfx/shaders'), { recursive: true });
  fs.writeFileSync(path.join(PREPARE, 'vfx/shaders/shading.json'), JSON.stringify({ ...b, owners, materials }));
  fs.copyFileSync(path.join(B, 'materials.wgsl'), path.join(PREPARE, 'vfx/shaders/materials.wgsl'));
  console.log(`(PARTY-NATIVE) build pack: ${taken.length} owners (${taken.map(([k, o]) => o.id + ':' + k + '[' + litClasses(o).map(([, , l]) => l).join('') + ']').join(', ')}) -> ${PREPARE}`);
  process.exit(0);
}

const built = read(path.join(BUILT, 'shaders.json')).shaders.filter((s) => s.ok);
const owners = {}, materials = {}, suffix = {}, shaders = [], blocks = [];
fs.mkdirSync(path.join(OUT, 'vfx/shaders'), { recursive: true }); fs.mkdirSync(path.join(OUT, 'vfx/er'), { recursive: true });
for (const [k, o] of taken) {
  let names = /^(s|m|d|xs|xm|v)(\d+)$/;
  if (TAKE_LIGHT) { // (PARTY-NATIVE) the live record with B's sprite / mesh classes; their pixel / fog / lit / distortion / vertex files
    const cls = litClasses(o);
    const r = { ...live.owners[k] };
    for (const [c] of cls) r[c] = o[c];
    if (o.xl) r.xl = o.xl; else delete r.xl;
    owners[k] = r;
    const letters = cls.map(([, , l]) => l).join('');
    names = new RegExp(`^((?:[fl]?|x)[${letters}]|${cls.some(([c]) => c === 'mesh') && o.mesh.vs ? 'v' : '(?!)'})(\\d+)$`);
    for (const [, tag] of cls) blocks.push(blockOf(o.id, tag));
  } else owners[k] = o;
  for (const [m, r] of Object.entries(b.materials)) if (r.owner === k) materials[m] = r;
  // every built file of the owner: s / m / d (pixel), xs / xm (distortion), v (vertex); (PARTY-NATIVE) + fs / fm / ls / lm
  for (const s of built.filter((x) => { const n = names.exec(x.name); return n && +n[2] === o.id; })) {
    const file = s.file.replace(/^([a-z]+\d+)\./, `$1${SUFFIX}.`);
    if (TAKE_LIGHT && fs.existsSync(path.join(OUT, 'vfx/er', file))) throw new Error(file + ' is there already: a new name each time');
    fs.copyFileSync(path.join(BUILT, s.file), path.join(OUT, 'vfx/er', file));
    if (TAKE_LIGHT && /^[fl]/.test(s.name)) continue; // (PARTY-NATIVE) found by file name (f / l + the pixel file), not listed
    suffix[s.name] = SUFFIX; shaders.push({ ...s, name: s.name + SUFFIX, file });
  }
}
const S = SUFFIX.slice(1);
const note = TAKE_LIGHT ? `(PARTY-NATIVE) lit translucent owners translated by tools/sora_vfx_shaders.js --fwl (their light input: forward lighting / the indirect lighting cache / the scene colours), their sprite / mesh shaders under <letter><id>${SUFFIX}; fog / lit variants f / l + the pixel file` : '(VFX-XLATE) owners translated by tools/sora_vfx_shaders.js --vxl that today\'s table lacks; their shaders under <letter><id>_vxl';
fs.writeFileSync(path.join(OUT, `vfx/shaders/shading${SUFFIX}.json`), JSON.stringify({ format: `kh3-sora-vfx-shading-overlay-${S}/1`, generator: 'tools/vfx_xlate_overlay.js', note, pixel_suffix: suffix, owners, materials }));
fs.writeFileSync(path.join(OUT, `vfx/er/shaders${SUFFIX}.json`), JSON.stringify({ format: `kh3-er-vfx-shaders-overlay-${S}/1`, note: `kh-shaderc --vfx over the ${TAKE_LIGHT ? '--fwl' : '--vxl'} table, renamed <letter><live id>${SUFFIX}`, shaders }, null, 1));
if (TAKE_LIGHT) fs.writeFileSync(path.join(OUT, `vfx/shaders/materials${SUFFIX}.wgsl`), ['// GENERATED: the taken owners\' blocks of tools/sora_vfx_shaders.js --fwl (tools/vfx_xlate_overlay.js --take light), for reading and rebuilding.', '', ...blocks, ''].join('\n')); // (PARTY-NATIVE)
console.log(`${S} overlay: ${taken.length} owners (${taken.map(([k, o]) => o.id + ':' + k).join(', ')}), ${Object.keys(materials).length} material records, ${shaders.length} shader files${TAKE_LIGHT ? " listed (+ their fog / lit variants)" : ""} -> ${OUT}`);
