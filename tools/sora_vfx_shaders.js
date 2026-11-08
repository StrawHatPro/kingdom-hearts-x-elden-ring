#!/usr/bin/env node
// tools/sora_vfx_shaders.js: the materials of the effect pack as shaders the test app can run.
//
// A cooked material has no math nodes left: its graph is only parameters, texture samples and function calls.
// The full truth is the compiled shader map inside the package (docs/formats/material.md): D3D11 bytecode per
// vertex factory and pass. This tool takes, for every material of build/sora_pack/vfx/materials/, the base-pass
// pixel shader (TBasePassPSFNoLightMapPolicy) of the package that owns its shader map (the material itself when it
// has static parameters or a blend-mode override, else the nearest parent that has one), disassembles it with the
// system's own D3D compiler DLL (D3DDisassemble through tools/lib/dxbc_disasm.ps1) and translates the instruction
// list 1:1 into a WGSL function. The shader's inputs are named by their D3D semantics and constant-buffer slots;
// crates/kh-app/src/vfx_material.wgsl (the harness) supplies them. Nothing is tuned by eye.
//
//   node tools/sora_vfx_shaders.js            build build/sora_pack/vfx/shaders/{materials.wgsl,shading.json}
//   node tools/sora_vfx_shaders.js --report   also print every owner that could not be translated, and why
//
// Outputs (local build output, never distributed):
//   shaders/materials.wgsl   one function per (shader owner, class), each inside `#ifdef KH_S<id>_<SPRITE|MESH|DECAL>`
//                            (DECAL = the material's FDeferredDecalPS: decal emitters project it onto the scene)
//   shaders/shading.json     owners (id, which classes translated, number of outputs, why not) and per material:
//                            owner, the material constant buffer (uniform expressions of the owner's shader map
//                            evaluated with the instance's parameter values; time-dependent entries as expression
//                            trees), the 2D textures by slot (file, sRGB, address modes), samplers, blend mode
// Cache: build/cache/vfx_shaders/*.bin / *.asm (the bytecode and its disassembly; safe to delete).
//
// What is translated: straight-line float code, if / else, discard, texture samples (always mip 0: the pack keeps
// the top mip only). What is not: integer / bit operations that reach the colour outputs, loops, dynamic
// constant-buffer indexing, cube textures, scene-colour reads, uniform buffers other than View / Primitive /
// Material / the particle sprite buffer. Such an owner is listed with the reason and the app draws its materials
// with the generic variant of the harness.
'use strict';
const fs = require('fs'), path = require('path'), cp = require('child_process');
const ROOT = path.resolve(__dirname, '..');
// (KBFX) `--root DIR`: AN EFFECT ROOT'S OWN TABLE (PACK/keyblades/vfx, packed by tools/sora_vfx_pack.js --out DIR with every
// material and texture its effects use): ONLY DIR's materials (DIR/materials) and DIR's texture records (DIR/manifest.json),
// owner ids 0.. of DIR's own owners, written to DIR/shaders (or --out). Its ids and records then depend on that root alone:
// the keyblades' table was made with `--party` (the MAIN materials first: a keyblade material whose name the main pack also
// has for another asset took the main record, e.g. Crystal Snow's tmi_loc_00sa = Wheel of Fate's; the party stream's texture
// records instead of the root's: 325 empty texture slots; the main pack's materials in the table, so its owner ids moved with
// the main pack and the built er/ files fell out of step). Not with --party / --xsi. Without `--root`: exactly as before.
const ROOT_ARG = process.argv.indexOf('--root');
const OWN_ROOT = ROOT_ARG > 0 ? path.resolve(process.argv[ROOT_ARG + 1]) : null;
const PACK = OWN_ROOT || path.join(ROOT, 'build/sora_pack/vfx');
const PKG = path.join(ROOT, 'assets_converted/packages/KINGDOM HEARTS III/Content');
const CACHE = path.join(ROOT, 'build/cache/vfx_shaders');
// (party VF) `--party`: THE PARTY'S OWN TABLES (build/sora_pack/party/vfx/shaders, read by the test app for every material
// and by the Elden Ring host for the party root's own materials): the materials of the main pack AND of the party root
// (PARTY_PACK/materials), the texture records of the main manifest plus those of the party root's textures (the party
// effect pack's manifest, PARTY_MANIFEST: without them every party-only texture slot was left unbound), and
// Primitive[10] translated (PRIM_OBJECT_AT_PARTICLE). `--out DIR`: write the two files there instead (a check run).
// Without `--party` the tool does exactly what it did before (the main table stays as it is: sha 2dd1d564).
// (XSI) `--xsi`: THE XS ABILITIES' TABLES (build/sora_tmp/xsi/pack/xsi/vfx/shaders; deployed to PACK/xsi/vfx/shaders): the
// party tables (everything `--party` does) plus the materials and texture records of the XSI effect root
// (XSI_PACK, packed by tools/sora_vfx_pack.js --out). The test app prefers them when present (a superset of the party's).
const XSI = process.argv.includes('--xsi');
const XSI_PACK = [path.join(ROOT, 'build/sora_pack/xsi/vfx'), path.join(ROOT, 'build/sora_tmp/xsi/pack/xsi/vfx')].find((d) => fs.existsSync(path.join(d, 'manifest.json'))) || path.join(ROOT, 'build/sora_tmp/xsi/pack/xsi/vfx'); // deployed, else staged
const PARTY = process.argv.includes('--party') || XSI;
if (OWN_ROOT && PARTY) throw new Error('--root DIR is a root\'s own table: not with --party / --xsi');
// (DON-MAGIC) KH_PARTY_VFX=<dir>: the party root read from a staged copy (a stream's own pack merged in), never written but
// through --out; KH_PARTY_EXTRA_MANIFEST=<file>: one more effect pack's texture records (its textures copied into that dir)
const PARTY_PACK = process.env.KH_PARTY_VFX ? path.resolve(process.env.KH_PARTY_VFX) : path.join(ROOT, 'build/sora_pack/party/vfx');
const PARTY_MANIFEST = path.join(ROOT, 'build/sora_tmp/party/impl_e/vfx_party/manifest.json');
const OUT_ARG = process.argv.indexOf('--out');
const OUT = OUT_ARG > 0 ? path.resolve(process.argv[OUT_ARG + 1]) : XSI ? path.join(XSI_PACK, 'shaders') : PARTY ? path.join(PARTY_PACK, 'shaders') : path.join(PACK, 'shaders');
const REPORT = process.argv.includes('--report');
const MAX_TEXTURES = 10, MAX_CB = 64;

// ------------------------------------------------------------------------------------------------ packages
const pkgCache = new Map();
function loadPkg(asset) { // "/Game/Effects/x/y" -> decoded package or null
  if (pkgCache.has(asset)) return pkgCache.get(asset);
  const f = path.join(PKG, asset.replace(/^\/Game\//, '').split('.')[0] + '.uasset.json');
  let d = null;
  if (fs.existsSync(f)) { d = JSON.parse(fs.readFileSync(f, 'utf8')); }
  // only the shader map and the texture references are needed afterwards: drop the rest to keep memory flat
  if (d) { const e = d.exports.find((e) => e.data && e.data.native && e.data.native.shaderMaps && e.data.native.shaderMaps.length); d = { shaderMap: e ? e.data.native.shaderMaps[0].shaderMap : null, textureRefs: expressionTextures(d) }; }
  pkgCache.set(asset, d);
  return d;
}

// The textures a material's expressions reference, in the order UMaterial::RebuildExpressionTextureReferences builds
// the list at load [U] (stock 4.17: every expression of `Expressions` in order; a function call contributes the
// function, then its dependent functions (nested first); AddUnique). `TextureIndex` of a uniform texture
// expression indexes this list. The tool checks the reconstruction against every texture PARAMETER (whose name
// gives the texture independently) and prints the mismatches.
function importPath(d, i) { let im = d.imports[-i - 1]; const names = [im.objectName]; while (im.outerIndex) { im = d.imports[-im.outerIndex - 1]; names.unshift(im.objectName); } return names[0] + (names.length > 1 ? '.' + names.slice(1).join('.') : ''); }
// Stock 4.17 [U]: the constructors of these expression classes set Texture to the engine's default texture, and a
// default value is not serialized: such an expression still takes a place in the texture list (TwoSidedTexturing's
// TextureObject shifts every later index of the 44 materials that call it).
const DEFAULT_TEXTURE = '/Engine/EngineResources/DefaultTexture.DefaultTexture';
const DEFAULT_TEXTURE_CLASSES = /^MaterialExpressionTexture(Object|ObjectParameter|SampleParameter2D)$/;
const rawCache = new Map();
// /Engine/ packages (stock material functions that hold textures of their own, e.g. TwoSidedTexturing) sit beside the
// game's: without them the texture list of a material that calls one is one entry short and every later index is off.
function rawPkg(asset) { if (rawCache.has(asset)) return rawCache.get(asset); const f = /^\/Engine\//.test(asset) ? path.join(ROOT, 'assets_converted/packages/Engine/Content', asset.replace(/^\/Engine\//, '').split('.')[0] + '.uasset.json') : path.join(PKG, asset.replace(/^\/Game\//, '').split('.')[0] + '.uasset.json'); const d = fs.existsSync(f) ? JSON.parse(fs.readFileSync(f, 'utf8')) : null; rawCache.set(asset, d); if (rawCache.size > 400) rawCache.delete(rawCache.keys().next().value); return d; }
const propOf = (e, n) => { const p = ((e.data && e.data.props) || []).find((p) => p.name === n); return p ? p.value : undefined; };
function appendFunctionTextures(asset, out, seen) {
  const d = rawPkg(asset); if (!d) return; const fn = d.exports.find((e) => e.class === 'MaterialFunction'); if (!fn) return;
  for (const i of propOf(fn, 'FunctionExpressions') || []) { const e = d.exports[i - 1]; if (!e || /FunctionCall$/.test(e.class)) continue; const t = propOf(e, 'Texture'); if (t && t < 0) { const p = importPath(d, t); if (!out.includes(p)) out.push(p); } else if (!t && DEFAULT_TEXTURE_CLASSES.test(e.class) && !out.includes(DEFAULT_TEXTURE)) out.push(DEFAULT_TEXTURE); }
}
function dependentFunctions(asset, list) {
  const d = rawPkg(asset); if (!d) return; const fn = d.exports.find((e) => e.class === 'MaterialFunction'); if (!fn) return;
  for (const i of propOf(fn, 'FunctionExpressions') || []) { const e = d.exports[i - 1]; if (!e || !/FunctionCall$/.test(e.class)) continue; const r = propOf(e, 'MaterialFunction'); if (!r || r >= 0) continue; const p = importPath(d, r).split('.')[0]; dependentFunctions(p, list); if (!list.includes(p)) list.push(p); }
}
function expressionTextures(d) {
  const mat = d.exports.find((e) => e.class === 'Material'); if (!mat) return null;
  const out = [];
  for (const i of propOf(mat, 'Expressions') || []) {
    const e = d.exports[i - 1]; if (!e) continue;
    if (/FunctionCall$/.test(e.class)) { const r = propOf(e, 'MaterialFunction'); if (!r || r >= 0) continue; const p = importPath(d, r).split('.')[0]; const list = [p]; dependentFunctions(p, list); for (const f of list) appendFunctionTextures(f, out); continue; }
    const t = propOf(e, 'Texture'); if (t && t < 0) { const p = importPath(d, t); if (!out.includes(p)) out.push(p); } else if (!t && DEFAULT_TEXTURE_CLASSES.test(e.class) && !out.includes(DEFAULT_TEXTURE)) out.push(DEFAULT_TEXTURE);
  }
  return out;
}

// ------------------------------------------------------------------------------------------------ uniform expressions
const T = (e) => (e.type || '').replace('FMaterialUniformExpression', '');
const col = (v) => [v.r, v.g, v.b, v.a];
// -> a tree with the parameters substituted: {c:[4]} | {t:1} | {f:name,a:[trees],...}
function bindExpr(e, mat) {
  const t = T(e);
  // (VFX-2) with --root / --vxl a parameter's leaf keeps its name (p) and kind (k: s scalar, v vector): what a dynamic
  // material instance (SQEX_Material_* on a skeletal mesh emitter's component) sets at run time; c stays its value
  const sc = (n, d) => { const s = mat.scalars && mat.scalars[n]; const v = s && typeof s.value === 'number' ? s.value : d; return XL ? { c: [v, v, v, v], p: n, k: 's' } : { c: [v, v, v, v] }; };
  const ve = (n, d) => { const s = mat.vectors && mat.vectors[n]; const v = s && s.value ? s.value : d; return XL ? { c: col(v), p: n, k: 'v' } : { c: col(v) }; };
  switch (t) {
    case 'Constant': return { c: col(e.value) };
    case 'ScalarParameter': return sc(e.parameterName, e.defaultValue);
    case 'VectorParameter': return ve(e.parameterName, e.defaultValue);
    case 'Time': case 'RealTime': return { t: 1 };
    case 'FoldedMath': return { f: ['add', 'sub', 'mul', 'div', 'dot'][e.op] || 'bad', a: [bindExpr(e.a, mat), bindExpr(e.b, mat)] };
    case 'AppendVector': return { f: 'append', n: e.numComponentsA, a: [bindExpr(e.a, mat), bindExpr(e.b, mat)] };
    case 'ComponentSwizzle': return { f: 'swizzle', i: [e.indexR, e.indexG, e.indexB, e.indexA], n: e.numElements, a: [bindExpr(e.x, mat)] };
    case 'Clamp': return { f: 'clamp', a: [bindExpr(e.input, mat), bindExpr(e.min, mat), bindExpr(e.max, mat)] };
    case 'Min': case 'Max': case 'Fmod': return { f: t.toLowerCase(), a: [bindExpr(e.a, mat), bindExpr(e.b, mat)] };
    case 'Sine': return { f: e.bIsCosine ? 'cos' : 'sin', a: [bindExpr(e.x, mat)] };
    case 'SquareRoot': return { f: 'sqrt', a: [bindExpr(e.x, mat)] };
    case 'Periodic': case 'Frac': return { f: 'frac', a: [bindExpr(e.x, mat)] };
    case 'Floor': case 'Ceil': case 'Abs': case 'Saturate': case 'Round': case 'Sign': case 'Truncate': return { f: t.toLowerCase(), a: [bindExpr(e.x, mat)] };
    case 'TrigMath': return { f: 'trig' + e.op, a: [bindExpr(e.x, mat), bindExpr(e.y, mat)] };
    default: return { f: 'bad', a: [] };
  }
}
function evalTree(n, time) { // -> [4] ; mirrors crates/kh-app/src/vfx_material.rs `eval_tree`
  if (n.c) return n.c;
  if (n.t) return [time, time, time, time];
  const a = n.a.map((x) => evalTree(x, time));
  const m1 = (fn) => a[0].map(fn), m2 = (fn) => a[0].map((x, i) => fn(x, a[1][i]));
  switch (n.f) {
    case 'add': return m2((x, y) => x + y); case 'sub': return m2((x, y) => x - y); case 'mul': return m2((x, y) => x * y);
    case 'div': return m2((x, y) => (Math.abs(y) < 1e-5 ? 0 : x / y));
    case 'dot': { const d = a[0][0] * a[1][0] + a[0][1] * a[1][1] + a[0][2] * a[1][2] + a[0][3] * a[1][3]; return [d, d, d, d]; }
    case 'append': { const r = [0, 0, 0, 0]; for (let i = 0; i < 4; i++) r[i] = i < n.n ? a[0][i] : a[1][i - n.n]; return r; }
    case 'swizzle': { const r = [0, 0, 0, 0]; for (let i = 0; i < 4; i++) r[i] = i < n.n && n.i[i] >= 0 ? a[0][n.i[i]] : 0; return r; }
    case 'clamp': return a[0].map((x, i) => Math.min(Math.max(x, a[1][i]), a[2][i]));
    case 'min': return m2(Math.min); case 'max': return m2(Math.max); case 'fmod': return m2((x, y) => (y === 0 ? 0 : x % y));
    case 'sin': return m1(Math.sin); case 'cos': return m1(Math.cos); case 'sqrt': return m1((x) => Math.sqrt(Math.max(x, 0)));
    case 'frac': return m1((x) => x - Math.floor(x)); case 'floor': return m1(Math.floor); case 'ceil': return m1(Math.ceil);
    case 'abs': return m1(Math.abs); case 'saturate': return m1((x) => Math.min(Math.max(x, 0), 1)); case 'round': return m1(Math.round);
    case 'sign': return m1(Math.sign); case 'truncate': return m1(Math.trunc);
    // (XSI) FMaterialUniformExpressionTrigMath (UE 4.17 ETrigMathOperation: 0 Sin, 1 Cos, 2 Tan, 3 Asin, 4 Acos, 5 Atan,
    // 6 Atan2) on X (Atan2: X, Y), per component; only with --xsi, so the main and party tables stay as they are
    case 'trig0': case 'trig1': case 'trig2': case 'trig3': case 'trig4': case 'trig5': case 'trig6': {
      // (RSF round 20) ALWAYS evaluated (was: only with --xsi / --trig, so every table made again since party VF-b lost it for
      // the materials its overlay did not list: omi_crs_3, Stitch's star, a flat white square in Elden Ring)
      const fn = [Math.sin, Math.cos, Math.tan, Math.asin, Math.acos, Math.atan][+n.f.slice(4)];
      return n.f === 'trig6' ? m2((x, y) => Math.atan2(x, y)) : m1((x) => { const v = fn(x); return Number.isFinite(v) ? v : 0; });
    }
    default: return [0, 0, 0, 0];
  }
}
const usesTime = (n) => !!n.t || (n.a || []).some(usesTime);
const isBad = (n) => n.f === 'bad' || (/^trig/.test(n.f || '') && !/^trig[0-6]$/.test(n.f)) || (n.a || []).some(isBad); // (RSF round 20) TrigMath always evaluated

// ------------------------------------------------------------------------------------------------ the translator
// Constant-buffer slots of the fork's View / Primitive / sprite buffers that the pixel shaders read, identified by
// how the compiled code uses them (the fork's layouts differ from stock 4.17); the harness gives each a value.
const VIEW_SLOTS = [8, 9, 10, 53, 54, 55, 58, 117, 118, 119, 122, 123, 124, 125, 126, 127, 128, 130, 137, 161];
const PRIM_SLOTS = [8, 9, 11, 17, 18];
// (party VF) Primitive[10] = ObjectWorldPositionAndRadius (the fork's Primitive buffer is stock 4.17's shifted by 5 rows:
// InvNonUniformScaleAndDeterminantSign at [9], whose .w the harness gives as the determinant sign). The imposter smoke master
// jmb_smk_imp_00sa (`Position Type` 1: the 32 x 32 view atlas frame is picked from the direction OBJECT -> camera) reads
// it; untranslated, its materials fell back to the generic variant, which draws the WHOLE atlas on every particle: the
// "fields of blue tiles" of Goofy's Fusion Rocket (vn_ex002_frt_tsn00 c_cloud0, ami_imp_flw_smc00a). The harnesses have
// no per-draw object position, so with `--party` its .xyz is the particle's own position in the space the shader builds
// from PARTICLE_POSITION - View[58] (i.ppos - i.v58): the frame is picked per particle (= Position Type 0) [approximation:
// all puffs of one emitter no longer share the object's view angle]. Only where the shader has PARTICLE_POSITION and
// reads no .w (the radius).
// (RSF round 21) and the main table's too (its jmb_smk_imp_00sa, the gap the party table closed), unless --no-twp-all
const PRIM_OBJECT_AT_PARTICLE = PARTY || !process.argv.includes('--no-twp-all');
const SPRITE_SLOTS = [10];
const PRECOMP_SLOTS = [5];
// (fifth pass) The vertex stage (base-pass vertex shader of the mesh-particle factory). View: [8..11] translated world -> view,
// [12..15] view -> translated world, [32..35] clip -> translated world (only inside the fork's depth-offset block, which the
// harness switches off: the evaluator applies SQEX_DepthOffset itself), [48] view forward, [55] camera origin, [56] the
// translated camera origin (zero), [58] PreViewTranslation, [128] / [161] the clock as in the pixel stage.
// MeshParticleVF: [2].xy UV scale, [3] UV flip / turn selectors, [4].xyz pivot offset, [5].x depth offset, [5].y emitter time.
const VIEW_SLOTS_CAMERA = [16, 17, 18, 19]; // (VFX-2 cube) --root / --vxl only
const VS_VIEW_SLOTS = [8, 9, 10, 11, 12, 13, 14, 15, 32, 33, 34, 35, 48, 55, 56, 58, 128, 161];
const VS_PRIM_SLOTS = [0, 1, 2, 3, 8, 9];
const VS_MESH_SLOTS = [0, 1, 2, 3, 4, 5];
// The interpolants the harness carries from a translated vertex shader to the pixel shader.
const VS_OUT = { TEXCOORD10: 'tc10', TEXCOORD11: 'tc11', TEXCOORD0: 'tc0', TEXCOORD1: 'tc1', COLOR0: 'col0', COLOR1: 'col1', COLOR2: 'col2', COLOR3: 'col3' };
// FDeferredDecalPS: its own loose constants (the unnamed buffer): [0..3] SvPositionToDecal (replaced by the decal-space
// position the harness reconstructs from the scene depth), [4..6] DecalToWorld axes (normal output only), [12].x the fade,
// [23].x the emitter time.
const DECAL_SLOTS = [12, 23];
// (XW) `--xw-decals NAME,..` (with --xsi): the decal owners whose emitter-driven arithmetic is evaluated per pixel
// (XW-b) `--xw-decals all` (with --xsi / --party): every decal owner's emitter-driven arithmetic per pixel.
// `--xw-break` (with --xsi / --party only): a plain `break` inside a loop is translated (before: it was not a control
// statement of the liveness walk and was left out without a word; the loop then ran past its exit). Every shader in which
// such a break stands is listed at the end (`XW-b break:` lines), with or without the flag: the main table's list.
// (XW-c) also for the MAIN table when written elsewhere (`--out DIR`): the main pack's own files are never written with it
const XW_BREAK = (XSI || PARTY || process.argv.indexOf('--out') > 0 || OWN_ROOT) && process.argv.includes('--xw-break'); // (KBFX) + --root
// (XW-c) `--xsi-extra DIR` (with --xsi): more materials (DIR/materials) and texture records (DIR/manifest.json) of a pack
// made by tools/sora_vfx_pack.js --out DIR (the chandelier's own effects); the owners only they use get ids AFTER every
// other owner, so the table's existing ids do not move.
// (XW-c) the extra-only owners' shader keys (`<id>_<name>`): for them only, TEXCOORD9 (the base pass's
// PixelPositionExcludingWPO = the translated world position, which the shader turns back with View[58]) is the harness's i.twp
const XWC_KEYS = new Set();
// (XW-d) `--twp-owners A,B` (with --xsi, or the main / party table with --out only): the same TEXCOORD9 -> i.twp (and Primitive[10]
// -> i.p8) rule for these owners of the table (tmt_cry_00a: the Chandelier's han03 c_dst0 tmi_cry_00a and Myriad's nmi_cry_00a
// had no translated shader: input TEXCOORD9). Without the flag every output is as before.
// (RSF round 21) THE SAME RULE FOR EVERY OWNER of every table (default; `--no-twp-all`: only XWC_KEYS / --twp-owners, as before):
// a shader that reads TEXCOORD9 (the base pass's world position) or Primitive[10] (ActorWorldPosition) had no translation at all
// (omt_star_00o: vm_fns_star's mesh, tmt_cyl_pil_00sa, Metal_Nuno_Leather_EnemyColorSkin_Basemat, o_ex162_mat ...); a shader that
// reads neither is translated exactly as before. The reasons that remain are listed at the end of every run, as before.
const TWP_ALL = !process.argv.includes('--no-twp-all');
const XWD_TWP = new Set((process.argv.indexOf('--twp-owners') > 0 ? process.argv[process.argv.indexOf('--twp-owners') + 1] : '').split(',').filter(Boolean));
if (XWD_TWP.size && !XSI && process.argv.indexOf('--out') < 0) throw new Error('--twp-owners: give --out DIR (the main / party table is not rewritten)');
// (party VF-b) `--trig` (with --party, written with --out only): the TrigMath uniform expressions (sin / cos of a rotation
// parameter: jmb_2layer_msk's UV turn) evaluated as --xsi does; without it they are 0 and the turned UV collapses to the
// texture's centre (vw_do090_hit0's crs: flat white cards). The party table itself is never rewritten with it: its records
// go to an overlay (party/vfx/shaders/shading_vfb.json).
// Also for the MAIN table (without --party): its records go to the main overlay (vfx/shaders/shading_vfb.json).
const VFB_TRIG = !XSI && process.argv.includes('--trig');
if (VFB_TRIG && process.argv.indexOf('--out') < 0) throw new Error('--trig: give --out DIR (the party / main table is not rewritten)');
// (party VF-b) `--vfb` (with --xsi --xsi-extra, --out only): the extra-only owners (XWC_KEYS: the chandelier's
// w_so088_mat0, a CHARACTER material its shatter fragments use as mesh particles) get what their shaders read beyond the
// harness's rows: MaterialCollection0 (KHS_Control: the collection's DEFAULT values, no world controller) as material rows
// from row R (R = the owner's own rows), Primitive[8] / [10] (object / actor position, [C] kh-look ObjectRows) = the
// particle's origin (i.ppos: the mesh instance's origin, GAME cm), [4..7] (WorldToLocal) = a translation by minus that
// origin, [15] / [16] (local bounds min / max) = material rows 62 / 63 (`--vfb-bounds x0,y0,z0,x1,y1,z1`: the
// chandelier pawn's cooked bounds). [approximation: KH3 gives the particle system component's rows, one per system]
const VFB = XSI && process.argv.includes('--vfb');
if (VFB && process.argv.indexOf('--out') < 0) throw new Error('--vfb: give --out DIR');
const VFB_BOUNDS = (process.argv.indexOf('--vfb-bounds') > 0 ? process.argv[process.argv.indexOf('--vfb-bounds') + 1] : '0,0,0,0,0,0').split(',').map(Number);
const VFB_ROW_MIN = MAX_CB - 2, VFB_ROW_MAX = MAX_CB - 1;
// (party VF-b) a material parameter collection by its StateId (as tools/sora_material_pack.js findCollection): stock 4.17
// layout, scalars four to a row, then one row per vector; the defaults
let vfbCollections = null;
function vfbCollection(guid) {
  if (!vfbCollections) {
    vfbCollections = new Map();
    const walk = (dir) => { for (const e of fs.readdirSync(dir, { withFileTypes: true })) { const p = path.join(dir, e.name); if (e.isDirectory()) walk(p); else if (/\.uasset\.json$/.test(e.name) && fs.statSync(p).size < 200000) { const text = fs.readFileSync(p, 'utf8'); if (!text.includes('"MaterialParameterCollection"')) continue; const d = JSON.parse(text); const x = d.exports.find((y) => y.class === 'MaterialParameterCollection'); if (x) vfbCollections.set(propOf(x, 'StateId'), { asset: path.relative(PKG, p).split(path.sep).join('/'), e: x }); } } };
    walk(path.join(PKG, 'Shader'));
  }
  const c = vfbCollections.get(guid); if (!c) return null;
  const st = (items) => Object.fromEntries((items || []).map((p) => [p.name, p.value]));
  const sc = ((propOf(c.e, 'ScalarParameters') || { items: [] }).items || []).map(st), ve = ((propOf(c.e, 'VectorParameters') || { items: [] }).items || []).map(st);
  const rows = [];
  sc.forEach((x, i) => { if (i % 4 === 0) rows.push([0, 0, 0, 0]); rows[i >> 2][i & 3] = x.DefaultValue; });
  ve.forEach((v) => { const d = v.DefaultValue || {}; rows.push([d.r || 0, d.g || 0, d.b || 0, d.a || 0]); });
  return { asset: c.asset, rows };
}
const XSI_EXTRA = XSI && process.argv.indexOf('--xsi-extra') > 0 ? path.resolve(process.argv[process.argv.indexOf('--xsi-extra') + 1]) : null;
const BREAKS = new Map();
const XW_DECALS = new Set((process.argv.indexOf('--xw-decals') > 0 ? process.argv[process.argv.indexOf('--xw-decals') + 1] : '').split(',').filter(Boolean));
// (XW, --xsi only) THE DECAL SHADER'S OWN PARAMETER TABLE (FDeferredDecalPS's serialized FShaderParameters, after the
// material's name: [byte offset, bytes, buffer] in the order SvPositionToDecal, DecalToWorld, (unbound), the fade (8 bytes),
// the emitter time (4), (unbound), a 4x4 = the decal's axes in the world, unit length (rows X, Y, Z: what the material's
// world -> tangent transforms read; e.g. Giant Stamp's sdc_so050_jwr_00a, bump-offset relief)): row -> harness input. The
// fade and time rows sit where a shader's table puts them (most at rows 12 / 23; Giant Stamp's and Myriad's two at 12 / 24),
// the axes as i.drot0..3. Without --xsi the fixed rows above, as before.
function decalRowsOf(parameters) {
  const at = (parameters || []).indexOf('Material'); if (at < 0) return null;
  let k = at + 1; while (k < parameters.length && !(Array.isArray(parameters[k]) && parameters[k].length === 3)) k++;
  const p = parameters.slice(k, k + 7); if (p.length < 7 || !p.every((x) => Array.isArray(x) && x.length === 3)) return null;
  const rows = {}; const row = (q) => (q[1] > 0 ? q[0] / 16 : null);
  if (row(p[3]) !== null) rows[row(p[3])] = 'i.d12';
  if (row(p[4]) !== null) rows[row(p[4])] = 'i.d23';
  if (row(p[6]) !== null && p[6][1] === 64) for (let r = 0; r < 4; r++) rows[row(p[6]) + r] = 'i.drot' + r;
  return rows;
}
// Material parameters a decal emitter writes every frame (TypeDataSQEX_Decal: DecalColor, DecalOpacity, CustomParam0..2,
// ZFadePow, ZFadeSegment): read from the harness inputs instead of the constant buffer.
const DECAL_PARAMS = { Decal_Color: 'i.dcol', Decal_Opacity: 'i.dcol.w', Decal_CustomParam0: 'i.dprm.x', Decal_CustomParam1: 'i.dprm.y', Decal_CustomParam2: 'i.dprm.z', Decal_ZFadePow: 'i.dz.x', Decal_ZFadeSegment: 'i.dz.y' };
const VIEW_CONST = { '126.x': 0, '161.x': 1 }; // selection highlight off; eye adaptation fixed (pre-exposed emissive)
// (VFX-XLATE, --root only) THE TRANSLATOR'S GAPS CLOSED FOR AN EFFECT ROOT'S OWN TABLE (the keyblades' root; the main / party /
// XS tables are never made with --root, so they stay byte for byte as they were):
// 1. SCENE COLOUR READS (the blur / warp materials: smt_blur_sho_00a, jmb_blur_*, smt_e_ex305_wrp_00a ...). Which texture of a
//    base-pass pixel shader is which scene texture is told by its place in the shader's serialized parameter list (the fork's
//    FBasePassPS layout, the same in every shader: census over the root's 383 owners): entry 7 = SceneDepthTexture (as before:
//    i.inv_scene_depth), entry 54 = the scene colour copy, entry 66 = the fork's translucency layer composited over it
//    (`scene.rgb * layer.a + layer.rgb`). The colour is read from the renderer's copy of the frame behind the effects
//    (er_vfx.wgsl kh_scene_color: t13, the copy kh-er-render vfx.rs makes before the translucent passes; in KH3's units), the
//    layer as empty (0, 0, 0, 1): our translucents are not drawn into a layer of their own. Such an owner's record says
//    `scene: true`; the renderer makes the copy for it (and skips it in a frame without one).
// 2. THE SCENE'S PARTICLE COLOURS (lit smoke: PARTICLE_BASECOLOR / BRIGHTCOLOR / DARKCOLOR / COLORSYNTESIS of the mesh-particle
//    factory, SpriteVF rows 11..14 of the sprite factory: [traced] the VF uniform structs' members SceneBaseColor /
//    SceneBrightColor / SceneDarkColor / SceneColorSynthesis (FUN_1442733c0, FUN_14037f0f0: FMeshParticleUniformParameters),
//    filled from the level's TresVFXSceneColorTable rows (TresSceneParamItem m_SceneColor*): harness inputs i.pbase / i.pbright /
//    i.pdark / i.psynth (er_vfx.wgsl: base 1 and synthesis 0 = no level tint, bright / dark = the host's key / shadow share of a
//    white surface [A: no KH3 level is loaded in Elden Ring]).
// 3. MATERIAL PARAMETER COLLECTIONS (MaterialCollectionN: KHS_Control, read by the character materials the effects use as mesh
//    particles, Nano Gear's w_so174_mat0_Inst on w_so172_mat1): the collection's rows after the material's own, with its
//    DEFAULT values overlaid by its run-time writer's (the KHS controller blueprint's `updateKHSParameter`, the values
//    tools/sora_material_pack.js reads into PACK/materials/materials.json khs.presets.blueprint.control); an owner whose rows
//    then pass the harness's 64 is COMPACTED: only the rows its shaders read, renumbered (`row_map` in its record).
// 4. Primitive[0..7] (LocalToWorld / WorldToLocal: the Ghost Horde afterimage smt_so050_stf_tsn_00k): the component's axes are
//    the world's, its origin the particle's (as --vfb does for its owners) [approximation: one transform per particle system].
// 5. View[160] [traced: the fork's View struct, FUN_144497e20: x MobilePreviewMode 0, y HMDEyePaddingOffset 1, z
//    ReflectionCubemapMaxMip, w SqexMasterLightCoefficient; the latter is APlayerCameraManager::SqexMasterLightCoefficient,
//    1.0 in its constructor (0x144421D70), changed only by bosses' SetMasterLightCoefficient (XIII, e_ex306 / 355 / 773) and
//    TresSceneManager]: (0, 1, 0, 1). KBFX's guess (1, 1, 1, 1) had .w right.
// `--vxl` (with --out only, the main / party / XS tables): the same five for a table written elsewhere (kh-er-render reads the
// result as the overlay shading_vxl.json + er/shaders_vxl.json of that root: tools/vfx_xlate_overlay.js); without it nothing changes
// (PARTY-NATIVE) `--fwl` (with --out only, the main / party / XS tables): everything --vxl does, plus THE LIT TRANSLUCENTS' LIGHT
// INPUT. A lit (not MSM_Unlit) translucent / additive / modulate owner whose base-pass pixel shader does not bind
// ForwardGlobalLightData (its TranslucencyLightingMode is volumetric: hmt_wat_lit_00a, kmt_sublit_ma, fmt_wtr_ygm_lit_00a ...)
// has NO light in TBasePassPSFNoLightMapPolicy: the fork compiles no translucency lighting volume into it (no 3D texture; its
// diffuse term is x View[130].x only). What lights it in KH3 is the permutation a particle with an indirect-lighting-cache
// allocation draws with (UE 4.17 ProcessBasePassMesh: a lit movable primitive with a point ILC sample, the particle default):
// TBasePassPSFCachedPointIndirectLightingPolicy, which adds `diffuse x PrecomputedLightingBuffer SH x View[142] x View[160].w`
// (rows 8..10 = IndirectLightingSHCoefficients0[R, G, B] (V0..V3), 11..13 = Coefficients1, 14 = Coefficients2; the
// non-directional modes read only the .x of 8..10 x 0.886228 = CalcDiffuseTransferSH1, the directional ones the 3-band dot,
// e.g. kmt_sublit_ma). With --fwl such an owner is translated from that permutation (cache key `.ilc`, the NoLightMap
// translation's cache stays as it is) and PrecomputedLightingBuffer[8..14] -> kh_fwd_ilc(k) (er_vfx.wgsl: in a lit variant
// Elden Ring's SH at the pixel in UE's SH layout, else 0 = the light the NoLightMap translation had), View[142] -> 1. The
// Skylight permutations are not taken (their sky term is the SH of a UE sky light; Elden Ring's SH already holds the sky).
// Its inputs are named by their semantics as every translated shader's (the harness hands each semantic over), so the
// permutation's own interpolant use needs nothing else; an owner whose ILC translation fails is not taken by the overlay.
function fwlIlcShader(o, msm, s0) { // -> the ILC pixel shader to translate instead of s0, or null
  const ubs = (x) => (((x.shader.resource.code.optionalData || []).find((y) => y.key === 'u') || {}).uniformBufferNames || []).map((n) => (typeof n === 'string' ? n : n.s));
  const d = o.sm.debugDescription || '';
  if (/LightingModel=MSM_Unlit/.test(d) || !/BlendMode=BLEND_(Translucent|Additive|Modulate|AlphaComposite)\b/.test(d)) return null;
  if (ubs(s0).includes('ForwardGlobalLightData')) return null; // a forward-lit owner: the VFX-2 lit rules
  const x = msm.shaders.find((y) => y.type === FWL_ILC_PS);
  return x && ubs(x).includes('PrecomputedLightingBuffer') ? x : null;
}
const FWL = !OWN_ROOT && process.argv.includes('--fwl');
const VXL = !OWN_ROOT && (process.argv.includes('--vxl') || FWL);
if (VXL && process.argv.indexOf('--out') < 0) throw new Error('--vxl / --fwl: give --out DIR (the table is not rewritten in place)');
const FWL_ILC_PS = 'TBasePassPSFCachedPointIndirectLightingPolicy';
const XL = !!OWN_ROOT || VXL;
// (VFX-2 cube) the packed cube maps of the root (tools/vfx_cube_pack.js): <root>/textures/cubes.json, or the files named by
// KH_VFX_CUBES (a ';' list: a staged table); asset -> { file }
const CUBES = {};
for (const file of (process.env.KH_VFX_CUBES ? process.env.KH_VFX_CUBES.split(';') : OWN_ROOT ? [path.join(OWN_ROOT, 'textures', 'cubes.json')] : [])) { try { Object.assign(CUBES, JSON.parse(fs.readFileSync(file, 'utf8')).cubes || {}); } catch (e) { /* none */ } }
const XL_SCENE_SLOTS = { 7: 'depth', 54: 'color', 66: 'over' };
function sceneTexOf(parameters) { // t-index -> 'depth' | 'color' | 'over' (entries [baseIndex, numResources] at the fixed places)
  const out = {}; if (!Array.isArray(parameters)) return out;
  for (const [k, kind] of Object.entries(XL_SCENE_SLOTS)) { const p = parameters[+k]; if (Array.isArray(p) && p.length === 2 && p[1] === 1) out[p[0]] = kind; }
  return out;
}
// (AUDIT-A IMAGES, with --root / --vxl) 7. THE MATERIAL'S PER-FRAME UNIFORM EXPRESSIONS (UE 4.17: an expression that
//    IsChangingPerFrame, i.e. reads Time, is not put in the Material buffer; the translator makes it a loose `$Globals` constant
//    UE_Material_PerFrameScalarExpression<k> / UE_Material_PerFrameVectorExpression<k>, and FMaterialShader::SetParameters
//    evaluates UniformExpressionSet.PerFrameUniformScalar/VectorExpressions[k] every draw and sets it). Which `$Globals` bytes
//    hold expression k is the shader's own FMaterialShader parameter table: the serialized arrays PerFrameScalarExpressions,
//    PerFrameVectorExpressions, PerFramePrevScalarExpressions, PerFramePrevVectorExpressions (FShaderParameter
//    [byte offset, bytes, buffer]) at 'Material' + 6 .. + 9 of `parameters` (checked: their lengths are those of the four lists
//    of the owner's uniform expression set). Mirage Staff's mirror image smt_so070_bnsn_00a (vw_so072_mrg_sora1): its pixel
//    shader reads cb0[10].xy / cb0[11].xy = vectors 0 / 1 (UV pans (fmod(t, 3600) x 0.3, x 0.9) and (1, fmod(t, 3600) x 0.45)),
//    its vertex shader cb0[10] / cb0[12] = vectors 0 / 2; untranslated ("uniform buffer"), the image drew with the generic
//    variant and showed nothing. Each expression becomes WGSL over the harness's clock i.v128.x (View[128], the clock every
//    translated shader reads), node by node as UE 4.17's GetNumberValue: Constant = its value; Time = (t, 0, 0, 1) (sets R
//    into its parent's FLinearColor::Black); FoldedMath add / sub / mul / div per component; Fmod = FMath::Fmod per component
//    (0 where |b| <= 1e-8); AppendVector = A's first NumComponentsA, then B's. Any other node, a parameter leaf or a bound
//    Prev expression: the shader stays untranslated with the reason (no owner of the keyblades' root has one). Without
//    --root / --vxl, or for a shader that binds no per-frame expression: nothing changes.
const fv4 = (c) => `vec4<f32>(${[c.r, c.g, c.b, c.a].map((x) => fl(String(+x || 0))).join(', ')})`;
function perFrameWgsl(e) { // -> { w: WGSL vec4 } | { why }
  const t = T(e); const sub = (x) => perFrameWgsl(x);
  switch (t) {
    case 'Constant': return { w: fv4(e.value || {}) };
    case 'Time': return { w: 'vec4<f32>(i.v128.x, 0.0, 0.0, 1.0)' };
    case 'FoldedMath': { const op = ['+', '-', '*', '/'][e.op]; if (!op) return { why: 'FoldedMath op ' + e.op }; const a = sub(e.a), b = sub(e.b); if (a.why || b.why) return a.why ? a : b; return { w: `(${a.w} ${op} ${b.w})` }; }
    case 'Fmod': { const a = sub(e.a), b = sub(e.b); if (a.why || b.why) return a.why ? a : b; return { w: `select(${a.w} % ${b.w}, vec4<f32>(0.0), abs(${b.w}) <= vec4<f32>(1e-8))` }; }
    case 'AppendVector': { const a = sub(e.a), b = sub(e.b); if (a.why || b.why) return a.why ? a : b; const n = e.numComponentsA | 0; if (n < 0 || n > 4) return { why: 'AppendVector of ' + n }; return { w: `vec4<f32>(${[0, 1, 2, 3].map((k) => (k < n ? `(${a.w}).${'xyzw'[k]}` : `(${b.w}).${'xyzw'[k - n]}`)).join(', ')})` }; }
    default: return { why: 'per-frame expression node ' + t };
  }
}
function perFrameOf(parameters, ue) { // -> null (none bound) | { rows: { row: WGSL vec4 } } | { why }
  const L = [ue.perFrameUniformScalarExpressions || [], ue.perFrameUniformVectorExpressions || [], ue.perFramePrevUniformScalarExpressions || [], ue.perFramePrevUniformVectorExpressions || []];
  if (!L.some((l) => l.length) || !Array.isArray(parameters)) return null;
  const at = parameters.indexOf('Material'); if (at < 0) return null;
  const P = [0, 1, 2, 3].map((j) => parameters[at + 6 + j]);
  if (!P.every((p, j) => Array.isArray(p) && p.length === L[j].length && p.every((q) => Array.isArray(q) && q.length === 3))) return { why: 'per-frame parameter table not where expected' };
  const bound = (p) => p.map((q, k) => ({ k, off: q[0], bytes: q[1], buf: q[2] })).filter((b) => b.bytes > 0);
  if (bound(P[2]).length || bound(P[3]).length) return { why: 'previous-frame expressions bound' };
  if ([...bound(P[0]), ...bound(P[1])].some((b) => b.buf !== 0)) return { why: 'per-frame expression outside $Globals' };
  const comp = {}; // byte offset -> WGSL scalar
  for (const b of bound(P[0])) { if (b.bytes !== 4 || b.off % 4) return { why: 'per-frame scalar of ' + b.bytes + ' bytes' }; const x = perFrameWgsl(L[0][b.k]); if (x.why) return x; comp[b.off] = `(${x.w}).x`; }
  for (const b of bound(P[1])) { if (b.off % 16 || b.bytes > 16 || b.bytes % 4) return { why: 'per-frame vector at byte ' + b.off }; const x = perFrameWgsl(L[1][b.k]); if (x.why) return x; for (let c = 0; c < b.bytes / 4; c++) comp[b.off + 4 * c] = `(${x.w}).${'xyzw'[c]}`; }
  const rows = {};
  for (const off of Object.keys(comp).map(Number)) { const r = off >> 4; if (rows[r]) continue; rows[r] = `(vec4<f32>(${[0, 1, 2, 3].map((c) => comp[r * 16 + 4 * c] || '0.0').join(', ')}))`; }
  return { rows };
}
const XL_SEMANTICS = { PARTICLE_BASECOLOR0: 'pbase', PARTICLE_BRIGHTCOLOR0: 'pbright', PARTICLE_DARKCOLOR0: 'pdark', PARTICLE_COLORSYNTESIS0: 'psynth' };
const XL_SPRITE_ROWS = { 11: 'i.pbase', 12: 'i.pbright', 13: 'i.pdark', 14: 'i.psynth' };
// (VFX-2 lit, with --root / --vxl) 6. THE FORWARD-LIT OWNERS (the fork's surface forward shading of MSM_DefaultLit translucents:
//    jmb_smk_implit_ac_00sa, jmb_sublit_msa, smt_so050_smk_sublit_ac_msa, smt_so053_mdl_smk_lit_00k; their shaders bind
//    ForwardGlobalLightData): UE 4.17's forward lighting translated 1:1 over the harness's stand-ins of its objects
//    (kh-er-render fwdlight.rs / er_vfx.wgsl `fwd_*`: Elden Ring's sun as the directional light, the lights our renderer puts
//    on the KH characters as ONE light-grid cell, the reflection environment as one infinite sphere capture):
//    - ForwardGlobalLightData[k] -> er.fwd_g[k]; PrecomputedLightingBuffer[5] (the precomputed directional shadowing) ->
//      er.fwd_g[7].x (the game's sun visibility at his feet); Primitive[11] = (0, 1, 0, 0) (UseSingleSampleShadowFromStationary
//      Lights on: that shadowing applies), Primitive[16].w = 1 (LightingChannelMask: channel 0, as every light's mask);
//    - the grid's buffers (by their declarations: the first uint buffer NumCulledLightsGrid, the second CulledLightDataGrid,
//      the float buffer ForwardLocalLightBuffer) -> kh_fwd_grid / kh_fwd_data / kh_fwd_light; ReflectionCapture[rN.c + K]
//      (dynamic indexing) -> kh_fwd_capture(rN.c + K); the capture cube array -> kh_fwd_env(direction, mip);
//    - the loose constants (the planar reflection's: its plane is 0, so its block is skipped) -> 0;
//    - View[11] (dead: planar block) (0, 0, 0, 1), [142] IndirectLightingColorScale-like scale of the reflections 1 [A], [148].w
//      0 [A], [163] (0, 0, 1, 0) = no roughness mixing with a sky light (the fork has none) [A], [160].z = the cube's max mip 7;
//    - integer operations on the integer VALUES the translator holds in floats (and without a comparison mask operand, ushr,
//      ubfe, bfi, umin / umax, imad, swapc, f16tof32 of a packed half): er_vfx.wgsl kh_iand ...;
//    - the scene colours SceneBrightColor / SceneDarkColor -> i.fbright / i.fdark (Elden Ring's light on a white surface, lit /
//      in the shade: fwdlight.rs).
//    Without --root / --vxl, or for any owner that does not bind ForwardGlobalLightData: nothing changes.
const FWD_SEMANTICS = { PARTICLE_BRIGHTCOLOR0: 'fbright', PARTICLE_DARKCOLOR0: 'fdark' };
const FWD_SPRITE_ROWS = { 12: 'i.fbright', 13: 'i.fdark' };
const FWD_VIEW = { 11: '(vec4<f32>(0.0, 0.0, 0.0, 1.0))', 142: '(vec4<f32>(1.0, 1.0, 1.0, 0.0))', 148: '(vec4<f32>(0.0))', 163: '(vec4<f32>(0.0, 0.0, 1.0, 0.0))', 160: '(vec4<f32>(0.0, 1.0, 7.0, 1.0))' };
let xlKhsControl = null; // KHS_Control's run-time values (name -> number), from the main pack's character table
function xlCollectionValues(asset) {
  if (!/KHS_Control$/.test(asset || '')) return {};
  if (xlKhsControl === null) { try { const m = JSON.parse(fs.readFileSync(path.join(ROOT, 'build/sora_pack/materials/materials.json'), 'utf8')); xlKhsControl = (((m.khs || {}).presets || {}).blueprint || {}).control || {}; } catch (e) { xlKhsControl = {}; } }
  return xlKhsControl;
}
// a collection by StateId: rows (scalars four to a row, then one row per vector), defaults overlaid by the run-time writer
function xlCollection(guid) {
  const c = vfbCollection(guid); if (!c) return null;
  const vals = xlCollectionValues(c.asset.replace(/\.uasset\.json$/, ''));
  const x = vfbCollections.get(guid).e; const st = (items) => Object.fromEntries((items || []).map((p) => [p.name, p.value]));
  const sc = ((propOf(x, 'ScalarParameters') || { items: [] }).items || []).map(st);
  const rows = c.rows.map((r) => r.slice()); const set = [];
  sc.forEach((s, i) => { if (typeof vals[s.ParameterName] === 'number') { rows[i >> 2][i & 3] = vals[s.ParameterName]; set.push(s.ParameterName); } });
  return { asset: c.asset, rows, set };
}
const SEMANTICS = { // D3D semantic -> field of the harness's KhIn
  TEXCOORD10: 'tc10', TEXCOORD11: 'tc11', TEXCOORD0: 'tc0', TEXCOORD1: 'tc1', TEXCOORD2: 'tc2', TEXCOORD3: 'tc3', TEXCOORD7: 'tc7',
  COLOR0: 'col0', COLOR1: 'col1', COLOR2: 'col2', COLOR3: 'col3', PARTICLE_TIME0: 'ptime', PARTICLE_POSITION0: 'ppos', PARTICLE_SUBUVS0: 'psubuv',
  SV_Position0: 'svpos', SV_IsFrontFace0: 'front', PSIZE0: 'psize',
  // (fourth pass) the mesh-particle instance transform (three rows of local -> world): the harness bakes the instances into
  // one mesh, so the rows it hands over are the identity's (a vector the shader turns with them stays in the mesh's own axes)
  PARTICLE_TRANSFORM0: 'ptr0', PARTICLE_TRANSFORM1: 'ptr1', PARTICLE_TRANSFORM2: 'ptr2',
};
function splitOps(s) { const out = []; let depth = 0, cur = ''; for (const ch of s) { if (ch === '(' || ch === '[') depth++; if (ch === ')' || ch === ']') depth--; if (ch === ',' && depth === 0) { out.push(cur.trim()); cur = ''; } else cur += ch; } if (cur.trim()) out.push(cur.trim()); return out; }
function parseOperand(s) {
  const o = { neg: false, abs: false, raw: s };
  if (s[0] === '-') { o.neg = true; s = s.slice(1); }
  if (s[0] === '|') { o.abs = true; s = s.slice(1, -1); }
  let m;
  if ((m = /^l\((.*)\)$/.exec(s))) { o.kind = 'l'; o.imm = m[1].split(',').map((x) => x.trim()); return o; }
  if (s === 'null') { o.kind = 'null'; return o; }
  if ((m = /^cb(\d+)\[(\d+)\](?:\.(\w+))?$/.exec(s))) { o.kind = 'cb'; o.cb = +m[1]; o.idx = +m[2]; o.swz = m[3] || 'xyzw'; return o; }
  if ((m = /^cb(\d+)\[r(\d+)\.([xyzw]) \+ (\d+)\](?:\.(\w+))?$/.exec(s))) { o.kind = 'cbdyn'; o.cb = +m[1]; o.reg = +m[2]; o.comp = m[3]; o.off = +m[4]; o.swz = m[5] || 'xyzw'; return o; } // (VFX-2 lit) cb[rN.c + K]
  if (/^cb\d+\[/.test(s)) { o.kind = 'cbdyn'; return o; }
  if ((m = /^([rvots])(\d+)(?:\.(\w+))?$/.exec(s))) { o.kind = m[1]; o.idx = +m[2]; o.swz = m[3] || 'xyzw'; return o; }
  o.kind = 'unknown'; return o;
}
const fl = (s) => { let v; if (/^-?1.#INF/.test(s)) return s[0] === '-' ? '(-1.0e30)' : '1.0e30'; /* an infinite constant */ if (/^0x/i.test(s)) { const b = Buffer.alloc(4); b.writeUInt32LE(parseInt(s, 16) >>> 0); v = b.readFloatLE(0); if (!Number.isFinite(v) || (v !== 0 && Math.abs(v) < 1e-30)) return null; /* a float constant printed as bits (and mask, -pi) */ } else v = Number(s); if (!Number.isFinite(v)) return null; let t = String(v); if (!/[.e]/.test(t)) t += '.0'; return v < 0 ? `(${t})` : t; };
const CI = { x: 0, y: 1, z: 2, w: 3 };

function transpile(asm, meta) {
  // meta: { ubNames: [..] by cb slot, texMap: {bind: texture2D index}, sampMap: {bind: {tex}|{shared:'wrap'|'clamp'}} }
  const fail = (why) => ({ ok: false, why });
  const lines = asm.split(/\r?\n/);
  const sem = {}; let nOut = 0; const osem = {}; let section = 'in'; let packed = null;
  for (const l of lines) {
    if (/^\/\/ Output signature/.test(l)) section = 'out';
    const m = /^\/\/ (\w+)\s+(\d+)\s+(\w+)\s+(\d+)\s+(\w+)\s+(\w+)\s*(.*?)\s*$/.exec(l);
    if (!m) continue;
    if (m[1] === 'SV_Target') { nOut++; continue; }
    // (fifth pass) a vertex shader: its outputs by register (the interpolants it hands the pixel shader)
    if (section === 'out') { if (!meta.vertex) continue; if (osem[+m[4]]) packed = m[1] + m[2]; osem[+m[4]] = { name: m[1].toUpperCase() + m[2] }; continue; }
    sem[+m[4]] = { name: m[1] + m[2], used: m[7] || '', format: m[6] };
  }
  if (packed) return fail('two outputs share a register (' + packed + ')');
  // (VFX-2 lit) a forward-lit owner's light-grid buffers and capture cube array, by their declarations
  const fwdRes = {};
  if (meta.fwd) {
    const uints = [];
    for (const l of lines) {
      let m;
      if ((m = /^dcl_resource_buffer \(uint,uint,uint,uint\) t(\d+)/.exec(l))) uints.push(+m[1]);
      else if ((m = /^dcl_resource_buffer \(float,float,float,float\) t(\d+)/.exec(l))) fwdRes[+m[1]] = 'kh_fwd_light';
      else if ((m = /^dcl_resource_texturecubearray \(float,float,float,float\) t(\d+)/.exec(l))) fwdRes[+m[1]] = 'cube';
    }
    if (uints.length !== 2 || Object.keys(fwdRes).length !== 2) return fail('forward lighting resources of an unknown shape');
    fwdRes[uints[0]] = 'kh_fwd_grid'; fwdRes[uints[1]] = 'kh_fwd_data';
  }
  const body = lines.filter((l) => l.trim() && !/^\s*(\/\/|dcl_|ps_5_0|vs_5_0)/.test(l)).map((l) => l.trim());
  let nTemps = 0; for (const l of lines) { const m = /^dcl_temps (\d+)/.exec(l); if (m) nTemps = +m[1]; }
  const ubOf = (cb) => (meta.ubNames[cb] !== undefined ? meta.ubNames[cb] : '?');
  // ---- parse
  const ins = [];
  for (const l of body) {
    const sp = l.indexOf(' ');
    let op = sp < 0 ? l : l.slice(0, sp), rest = sp < 0 ? '' : l.slice(sp + 1);
    let texType = null;
    const mm = /^(\w+)\((\w+)\)\(.*?\)$/.exec(op); if (mm) { op = mm[1]; texType = mm[2]; }
    let sat = false; if (/_sat$/.test(op)) { sat = true; op = op.replace(/_sat$/, ''); }
    ins.push({ op, sat, texType, a: splitOps(rest).map(parseOperand), line: l });
  }
  if (meta.key && ins.some((n) => n.op === 'break')) BREAKS.set(meta.key, { breaks: ins.filter((n) => n.op === 'break').length, switches: ins.filter((n) => n.op === 'switch').length }); // (XW-b)
  // ---- static folding of the two view constants that gate whole blocks, and the SvPosition -> translated-world idiom
  const svReg = Object.keys(sem).find((r) => sem[r].name === 'SV_Position0');
  const out = []; // {kind:'ins', i} | {kind:'raw', text, dest:{r,mask}, src:[]} | control
  const constVal = (o) => { if (o.kind === 'l' && o.imm.length >= 1) { const v = Number(o.imm[0]); return Number.isFinite(v) && o.imm.every((x) => Number(x) === v) ? v : null; } if (o.kind === 'cb' && ubOf(o.cb) === 'View') { const k = o.idx + '.' + o.swz[0]; if (o.swz.split('').every((c) => c === o.swz[0]) && k in VIEW_CONST) return VIEW_CONST[k]; } return null; };
  let known = null; // {reg, comp, value} set by the previous instruction
  let sawClip = false;
  if (meta.decal) for (let i = 0; i + 1 < ins.length; i++) { // a sample unpacked with `mad r, r, l(2), l(-1)` right after it
    const n = ins[i], m = ins[i + 1]; if (!/^sample/.test(n.op) || m.op !== 'mad' || m.a.length < 4) continue;
    const lit = (o, v) => o.kind === 'l' && Number(o.imm[0]) === v;
    if (n.a[0].kind === 'r' && m.a[1].kind === 'r' && m.a[1].idx === n.a[0].idx && lit(m.a[2], 2) && lit(m.a[3], -1)) n.gbufferNormal = true;
  }
  if (meta.vertex) {
    // (fifth pass) The fork's depth-offset block of the mesh-particle vertex shader is gated by MeshParticleVF[5].x != 0
    // through conditional moves. The evaluator applies SQEX_DepthOffset itself, so the constant is 0 here: comparisons
    // of constants are folded and a `movc` on a folded condition becomes a `mov` (the block is then dead code).
    const kc = new Map();
    const cv = (o) => { if (o.kind === 'l' && o.imm.length >= 1) { const v = Number(o.imm[0]); return Number.isFinite(v) && o.imm.every((x) => Number(x) === v) ? v : null; } if (o.kind === 'cb' && /^(MeshParticleVF|FMeshParticleUniformParameters)$/.test(ubOf(o.cb)) && o.idx === 5 && /^x+$/.test(o.swz)) return 0; return null; };
    for (const n of ins) {
      if (/^(if_|else|endif|loop|endloop|breakc)/.test(n.op)) { kc.clear(); continue; }
      const d = n.a[0]; if (!d || d.kind !== 'r') continue;
      if (n.op === 'movc' && n.a[1].kind === 'r') {
        const c = n.a[1]; const vals = d.swz.split('').map((ch) => kc.get('r' + c.idx + '.' + (c.swz.length === 1 ? c.swz[0] : c.swz[CI[ch]] || c.swz[c.swz.length - 1])));
        if (vals.every((v) => v !== undefined) && vals.every((v) => v === vals[0])) { n.op = 'mov'; n.a = [d, vals[0] ? n.a[2] : n.a[3]]; }
      }
      for (const ch of d.swz) kc.delete('r' + d.idx + '.' + ch);
      if (/^(lt|ne|ge|eq)$/.test(n.op) && d.swz.length === 1) { const x = cv(n.a[1]), y = cv(n.a[2]); if (x !== null && y !== null) kc.set('r' + d.idx + '.' + d.swz, ({ lt: x < y, ne: x !== y, ge: x >= y, eq: x === y })[n.op] ? 1 : 0); }
    }
  }
  for (let i = 0; i < ins.length; i++) {
    const n = ins[i];
    if ((n.op === 'if_nz' || n.op === 'if_z') && known && n.a[0].kind === 'r' && n.a[0].idx === known.reg && n.a[0].swz === known.comp) {
      const taken = (n.op === 'if_nz') === (known.value !== 0);
      // find the matching else / endif
      let depth = 0, j = i + 1, elseAt = -1, endAt = -1;
      for (; j < ins.length; j++) { const o = ins[j].op; if (o === 'if_nz' || o === 'if_z') depth++; else if (o === 'endif') { if (depth === 0) { endAt = j; break; } depth--; } else if (o === 'else' && depth === 0) elseAt = j; }
      if (endAt < 0) return fail('unbalanced if');
      const from = taken ? i + 1 : (elseAt < 0 ? endAt : elseAt + 1), to = taken ? (elseAt < 0 ? endAt : elseAt) : endAt;
      const keep = ins.slice(from, to);
      ins.splice(i, endAt - i + 1, ...keep);
      // the comparison itself is dead now
      if (out.length && out[out.length - 1].i === known.at) out.pop();
      known = null; i--; continue;
    }
    known = null;
    if ((n.op === 'lt' || n.op === 'ne' || n.op === 'ge' || n.op === 'eq') && n.a[0].kind === 'r' && n.a[0].swz.length === 1) {
      const x = constVal(n.a[1]), y = constVal(n.a[2]);
      if (x !== null && y !== null) known = { reg: n.a[0].idx, comp: n.a[0].swz, value: ({ lt: x < y, ne: x !== y, ge: x >= y, eq: x === y })[n.op] ? 1 : 0, at: i };
    }
    if (meta.decal && n.op === 'mul' && n.a.length === 3 && n.a[2].kind === 'cb' && ubOf(n.a[2].cb) === '' && n.a[2].idx === 1 && n.a[1].kind === 'v' && n.a[0].swz === 'xyzw') {
      const nx = ins.slice(i + 1, i + 4);
      const okIdiom = nx.length === 3 && nx[0].op === 'mad' && nx[0].a[2].idx === 0 && nx[1].op === 'mad' && nx[1].a[2].idx === 2 && nx[2].op === 'add' && nx[2].a[2].idx === 3 && nx.every((x) => x.a[0].kind === 'r' && x.a[0].swz === 'xyzw') && nx[0].a[0].idx === n.a[0].idx && nx[1].a[0].idx === nx[2].a[0].idx;
      if (!okIdiom) return fail('SvPositionToDecal used outside the usual idiom');
      out.push({ kind: 'raw', text: `r${nx[2].a[0].idx} = vec4<f32>(i.decal_pos, 1.0);`, dest: { r: 'r' + nx[2].a[0].idx, mask: 'xyzw' }, src: [] });
      i += 3; continue;
    }
    if (n.op === 'mul' && n.a.length === 3 && n.a[2].kind === 'cb' && ubOf(n.a[2].cb) === 'View' && n.a[2].idx === 37 && n.a[1].kind === 'v' && String(n.a[1].idx) === String(svReg) && n.a[0].swz === 'xyzw') {
      const nx = ins.slice(i + 1, i + 4);
      const okIdiom = nx.length === 3 && nx[0].op === 'mad' && nx[0].a[2].idx === 36 && nx[1].op === 'mad' && nx[1].a[2].idx === 38 && nx[2].op === 'add' && nx[2].a[2].idx === 39 && nx.every((x) => x.a[0].kind === 'r' && x.a[0].idx === n.a[0].idx);
      if (!okIdiom) return fail('SVPositionToTranslatedWorld used outside the usual idiom');
      out.push({ kind: 'raw', text: `r${n.a[0].idx} = vec4<f32>(i.twp, 1.0);`, dest: { r: 'r' + n.a[0].idx, mask: 'xyzw' }, src: [] });
      i += 3; continue;
    }
    // (fifth pass) a vertex shader's last step, translated world -> clip: `mul rA, rW.yyyy, View[1]; mad rA, rW.xxxx, View[0], rA;
    // mad rB, rW.zzzz, View[2], rA; mad oPos, w, View[3], rB`. The harness projects with its own camera, so the four
    // instructions become `kh_wpos = rW` (the translated world position with the material's world position offset).
    if (meta.vertex && n.op === 'mul' && i + 3 < ins.length) {
      const viewRow = (x, k) => x.a.some((o) => o.kind === 'cb' && ubOf(o.cb) === 'View' && o.idx === k);
      const nx = ins.slice(i, i + 4); const last = nx[3];
      if (viewRow(nx[0], 1) && viewRow(nx[1], 0) && viewRow(nx[2], 2) && viewRow(last, 3) && last.a[0].kind === 'o' && osem[last.a[0].idx] && osem[last.a[0].idx].name === 'SV_POSITION0') {
        const pick = (x, c) => x.a.slice(1).find((o) => (o.kind === 'r' || o.kind === 'v') && o.swz === c + c + c + c);
        const sy = pick(nx[0], 'y'), sx = pick(nx[1], 'x'), sz = pick(nx[2], 'z');
        if (!sy || !sx || !sz || sx.idx !== sy.idx || sz.idx !== sy.idx || sx.kind !== sy.kind || sz.kind !== sy.kind || sy.kind !== 'r') return fail('clip transform outside the usual idiom');
        out.push({ kind: 'raw', vertex: true, text: `kh_wpos = vec4<f32>(r${sy.idx}.xyz, 1.0);`, dest: { r: 'kh_wpos', mask: 'xyzw' }, src: ['x', 'y', 'z'].map((c) => 'r' + sy.idx + '.' + c) });
        sawClip = true; i += 3; continue;
      }
    }
    out.push({ kind: 'ins', i, n });
  }
  if (meta.vertex && !sawClip) return fail('no clip transform found');
  // ---- operands -> WGSL
  let distIdx = null;
  let why = null; const lenientNotes = []; const bad = (w) => { if (meta.lenient) { lenientNotes.push(w); return 'vec4<f32>(0.0)'; } if (!why) why = w; return '0.0'; };
  let usesScene = false; // (VFX-XLATE)
  let usesClock = false; const rowsUsed = new Set(); const texUsed = new Set(), sampUsed = new Set(), viewUsed = new Set(), semUsed = new Set(); let usesDepth = false, usesTwp = false, maxCb = -1;
  const base = (o) => {
    switch (o.kind) {
      case 'r': return 'r' + o.idx;
      case 'o': return 'o' + o.idx;
      case 'v': { const s = sem[o.idx];
        if (meta.vertex) { const m = s && /^ATTRIBUTE(\d+)$/.exec(s.name); if (!m) return bad('vertex input ' + (s ? s.name : 'v' + o.idx)); if (s.format !== 'float') return bad('integer vertex input ' + s.name); semUsed.add(s.name); return 'i.a' + m[1]; }
        if (s && s.name === 'TEXCOORD9' && meta.key && (TWP_ALL || XWC_KEYS.has(meta.key.split('.')[0]))) { semUsed.add(s.name); usesTwp = true; return '(vec4<f32>(i.twp, 1.0))'; } // (XW-c)
        const f = s && ((meta.semantics || {})[s.name] || SEMANTICS[s.name] || (meta.fwd ? FWD_SEMANTICS[s.name] : undefined) /* (VFX-2 lit) */ || (XL ? XL_SEMANTICS[s.name] : undefined)); if (!f) return bad('input ' + (s ? s.name : 'v' + o.idx)); semUsed.add(s.name); return 'i.' + f; } // (VFX-XLATE) + the scene particle colours
      case 'cbdyn': { // (VFX-2 lit) the forward-lit owners' dynamically indexed arrays
        const ub = ubOf(o.cb);
        if (meta.fwd && o.reg !== undefined && ub === 'ReflectionCapture') return `kh_fwd_capture(r${o.reg}.${o.comp} + ${o.off}.0)`;
        if (meta.fwd && o.reg !== undefined && ub === '') return '(vec4<f32>(0.0))';
        return bad('operand ' + o.raw); }
      case 'cb': { const ub = ubOf(o.cb);
        // (AUDIT-A IMAGES, --root / --vxl) a `$Globals` row that holds per-frame uniform expressions: their WGSL (perFrameOf)
        if (XL && ub === '' && meta.perFrame && meta.perFrame.rows && meta.perFrame.rows[o.idx]) { viewUsed.add(128); return meta.perFrame.rows[o.idx]; }
        if (meta.ilc && !meta.vertex) { // (PARTY-NATIVE) --fwl: the indirect lighting cache's SH and its colour scale
          if (ub === 'PrecomputedLightingBuffer' && o.idx >= 8 && o.idx <= 14) return `kh_fwd_ilc(${o.idx})`;
          if (ub === 'View' && o.idx === 142) return FWD_VIEW[142];
        }
        if (meta.fwd && !meta.vertex) { // (VFX-2 lit) the forward lighting's buffers and the rows it reads beyond the others'
          if (ub === 'ForwardGlobalLightData') { if (o.idx > 6) return bad('ForwardGlobalLightData[' + o.idx + ']'); return `er.fwd_g[${o.idx}]`; }
          if (ub === 'PrecomputedLightingBuffer' && o.idx === 5) return '(vec4<f32>(er.fwd_g[7].x))';
          if (ub === 'ReflectionCapture') return `kh_fwd_capture(${o.idx}.0)`;
          if (ub === '') return '(vec4<f32>(0.0))';
          if (ub === 'Primitive' && o.idx === 11) return '(vec4<f32>(0.0, 1.0, 0.0, 0.0))';
          if (ub === 'Primitive' && o.idx === 16) return '(vec4<f32>(1.0, 1.0, 1.0, 1.0))';
          if (ub === 'View' && FWD_VIEW[o.idx]) return FWD_VIEW[o.idx];
          if ((ub === 'SpriteVF' || ub === 'FParticleSpriteUniformParameters') && FWD_SPRITE_ROWS[o.idx]) return FWD_SPRITE_ROWS[o.idx];
        }
        if (meta.vertex && ub !== 'Material') { // (fifth pass) the vertex stage's own view of the buffers (harness: KhVsIn)
          if (ub === 'View') { if (!VS_VIEW_SLOTS.includes(o.idx)) return bad('vertex View[' + o.idx + ']'); viewUsed.add(o.idx); return 'i.v' + o.idx; }
          if (ub === 'Primitive' && meta.vfb && o.idx === 10) return '(vec4<f32>(i.a8.w, i.a9.w, i.a10.w, 1.0))'; // (party VF-b) the instance's origin
          // (AUDIT-A IMAGES, --root / --vxl) WorldToLocal [4..7] in the vertex stage: the pixel stage's rule 4 (VFX-XLATE: the
          // component's axes are the world's, its origin the particle's = the instance's origin i.a8..a10.w), so both stages
          // of an owner use one frame (smt_so070_bnsn_00a: the polar UV of its world position offset, as its pixel shader's).
          // Only in a shader with per-frame expressions (this job's owner), so that no other owner's output moves (the one
          // other owner it would reach, gmt_ant_kag_00a "vertex Primitive[5]", stays as it was: a follow-up)
          if (XL && meta.perFrame && meta.perFrame.rows && ub === 'Primitive' && o.idx >= 4 && o.idx <= 7) return `(${{ 4: 'vec4<f32>(1.0, 0.0, 0.0, 0.0)', 5: 'vec4<f32>(0.0, 1.0, 0.0, 0.0)', 6: 'vec4<f32>(0.0, 0.0, 1.0, 0.0)', 7: 'vec4<f32>(-i.a8.w, -i.a9.w, -i.a10.w, 1.0)' }[o.idx]})`;
          if (ub === 'Primitive') { if (!VS_PRIM_SLOTS.includes(o.idx)) return bad('vertex Primitive[' + o.idx + ']'); return 'i.p' + o.idx; }
          if (ub === 'MeshParticleVF' || ub === 'FMeshParticleUniformParameters') { if (!VS_MESH_SLOTS.includes(o.idx)) return bad('mesh particle buffer[' + o.idx + ']'); return 'i.mp' + o.idx; }
          return bad('vertex uniform buffer "' + ub + '"[' + o.idx + ']');
        }
        // the distortion pixel shader's only loose constant is DistortionParams (its slot varies from shader to shader)
        if (ub === '' && meta.distort) { if (distIdx === null) distIdx = o.idx; if (o.idx !== distIdx) return bad('a second loose constant in the distortion shader [' + o.idx + ']'); return 'i.dist12'; }
        // (VFX-XLATE, --root) a collection's row, after the material's own; an owner's rows compacted through its row map
        if (XL && meta.xl && /^MaterialCollection[0-9]$/.test(ub)) { const k = +ub.slice(-1); const c = meta.xl.colls[k]; if (!c || !c.rows) return bad('collection ' + ub); if (o.idx >= c.rows.length) return bad('collection row ' + o.idx + ' past ' + c.asset); const L = c.base + o.idx; const P = meta.xl.map ? meta.xl.map.get(L) : L; if (P === undefined || P >= MAX_CB) return bad('material constant buffer too large'); maxCb = Math.max(maxCb, P); return `kh.m[${P}]`; }
        if (XL && meta.xl && meta.xl.map && ub === 'Material') { const P = meta.xl.map.get(o.idx); if (P === undefined) return bad('material row ' + o.idx + ' not in the row map'); maxCb = Math.max(maxCb, P); return `kh.m[${P}]`; }
        if (ub === 'Material') { if (o.idx >= MAX_CB) return bad('material constant buffer too large'); maxCb = Math.max(maxCb, o.idx); if (meta.rowOverride && meta.rowOverride[o.idx]) { rowsUsed.add(o.idx); return `m${o.idx}`; } return `kh.m[${o.idx}]`; }
        if (ub === '' && meta.decal && meta.decalRows) { if (!meta.decalRows[o.idx]) return bad('decal buffer[' + o.idx + ']'); return meta.decalRows[o.idx]; } // (XW, --xsi)
        if (ub === '' && meta.decal) { if (!DECAL_SLOTS.includes(o.idx)) return bad('decal buffer[' + o.idx + ']'); return 'i.d' + o.idx; }
        // (KBFX, --root only) View[160].w scales the emissive term just before the base pass writes it (jmb_opqlit_env:
        // `mul r3.xyz, r3, cb0[160].w` -> o0 = cb0[130].x * diffuse + emissive): the exposure scale, 1 here as the eye
        // adaptation View[161].x is (the harness's emissive is pre-exposed) [inferred from the shader, not traced]. Untranslated,
        // those materials fell to the generic variant and drew flat (Classic Tone's finisher gears zmi_ger_slv02 / gld00)
        // (VFX-XLATE) traced: (MobilePreviewMode 0, HMDEyePaddingOffset 1, ReflectionCubemapMaxMip 0, SqexMasterLightCoefficient 1)
        if (ub === 'View' && XL && o.idx === 160) return '(vec4<f32>(0.0, 1.0, 0.0, 1.0))';
        // (VFX-2 cube) View[16..19] = TranslatedWorldToCameraView (UE 4.17 FViewUniformShaderParameters rows 16..19, after
        // TranslatedWorldToView 8..11): the matcap's view-space normal; without stereo / view offsets the same rows as 8..10
        if (ub === 'View') { if (!VIEW_SLOTS.includes(o.idx) && !(XL && VIEW_SLOTS_CAMERA.includes(o.idx))) return bad('View[' + o.idx + ']'); viewUsed.add(o.idx); return 'i.v' + o.idx; }
        if (meta.vfb) { // (party VF-b) --vfb, the extra-only owners
          const mc = /^MaterialCollection([0-9])$/.exec(ub);
          if (mc) { if (+mc[1] !== 0 || !meta.vfb.coll) return bad('collection ' + ub); const row = meta.vfb.R + o.idx; if (row >= VFB_ROW_MIN || o.idx >= meta.vfb.coll.rows.length) return bad('collection row ' + o.idx + ' past the material rows'); maxCb = Math.max(maxCb, row); return `kh.m[${row}]`; }
          if (ub === 'Primitive') {
            const p = { 4: 'vec4<f32>(1.0, 0.0, 0.0, 0.0)', 5: 'vec4<f32>(0.0, 1.0, 0.0, 0.0)', 6: 'vec4<f32>(0.0, 0.0, 1.0, 0.0)', 7: 'vec4<f32>(-i.ppos.xyz, 1.0)', 8: 'vec4<f32>(i.ppos.xyz, 0.0)', 10: 'vec4<f32>(i.ppos.xyz, 1.0)' }[o.idx];
            if (p) return `(${p})`;
            if (o.idx === 15 || o.idx === 16) { maxCb = Math.max(maxCb, VFB_ROW_MAX); return `kh.m[${o.idx === 15 ? VFB_ROW_MIN : VFB_ROW_MAX}]`; }
          }
        }
        // (VFX-XLATE, --root) LocalToWorld [0..3] / WorldToLocal [4..7]: the world's axes, the particle's origin
        if (XL && ub === 'Primitive' && o.idx <= 7 && !meta.vertex) {
          const p = { 0: 'vec4<f32>(1.0, 0.0, 0.0, 0.0)', 1: 'vec4<f32>(0.0, 1.0, 0.0, 0.0)', 2: 'vec4<f32>(0.0, 0.0, 1.0, 0.0)', 3: 'vec4<f32>(i.ppos.xyz, 1.0)', 4: 'vec4<f32>(1.0, 0.0, 0.0, 0.0)', 5: 'vec4<f32>(0.0, 1.0, 0.0, 0.0)', 6: 'vec4<f32>(0.0, 0.0, 1.0, 0.0)', 7: 'vec4<f32>(-i.ppos.xyz, 1.0)' }[o.idx];
          return `(${p})`;
        }
        // (VFX-XLATE, --root) LocalObjectBoundsMin / Max: a unit box (the character materials only normalise a direction by
        // them: w_so172_mat1's cube transform) [approximation: the mesh's own extents are not handed to the shader]
        if (XL && ub === 'Primitive' && (o.idx === 15 || o.idx === 16) && !meta.vertex) return o.idx === 15 ? '(vec4<f32>(-1.0, -1.0, -1.0, 0.0))' : '(vec4<f32>(1.0, 1.0, 1.0, 0.0))';
        if (ub === 'Primitive' && o.idx === 10 && PRIM_OBJECT_AT_PARTICLE && Object.values(sem).some((s) => s.name === 'PARTICLE_POSITION0')) { if (/w/.test(o.swz || '')) return bad('Primitive[10].w'); semUsed.add('PARTICLE_POSITION0'); return '(i.ppos - i.v58)'; } // (party VF)
        if (ub === 'Primitive' && o.idx === 10 && meta.key && (TWP_ALL || XWC_KEYS.has(meta.key.split('.')[0]))) return 'i.p8'; // (XW-c) ActorWorldPosition as the object's position (the harness's i.p8)
        if (ub === 'Primitive') { if (!PRIM_SLOTS.includes(o.idx)) return bad('Primitive[' + o.idx + ']'); return 'i.p' + o.idx; }
        if (XL && (ub === 'SpriteVF' || ub === 'FParticleSpriteUniformParameters') && XL_SPRITE_ROWS[o.idx]) return XL_SPRITE_ROWS[o.idx]; // (VFX-XLATE) the scene particle colours
        if (ub === 'SpriteVF' || ub === 'FParticleSpriteUniformParameters') { if (!SPRITE_SLOTS.includes(o.idx)) return bad('sprite buffer[' + o.idx + ']'); return 'i.s' + o.idx; }
        if (ub === 'PrecomputedLightingBuffer' || ub === 'FPrecomputedLightingParameters') { if (!PRECOMP_SLOTS.includes(o.idx)) return bad('precomputed lighting[' + o.idx + ']'); return 'i.l' + o.idx; }
        return bad('uniform buffer ' + ub); }
      default: return bad('operand ' + o.raw);
    }
  };
  const wrap = (o, e) => { if (o.abs) e = `abs(${e})`; if (o.neg) e = `(-${e})`; return e; };
  // the expression of operand `o` for the destination components `comps` (indices 0..3)
  const vec = (o, comps) => {
    if (o.kind !== 'l' && !o.swz) o = Object.assign({}, o, { swz: 'xyzw' }); // (party E) an operand written without a swizzle is .xyzw (a party effect's shader has one)
    if (o.kind === 'l') { const v = comps.map((k) => fl(o.imm.length === 1 ? o.imm[0] : o.imm[k])); if (v.includes(null)) return bad('integer immediate'); return wrap(o, v.length === 1 ? v[0] : `vec${v.length}<f32>(${v.join(', ')})`); }
    if (o.kind === 'cb' && ubOf(o.cb) === 'View' && o.idx === 161 && /z/.test(comps.map((k) => (o.swz.length === 1 ? o.swz[0] : o.swz[k])).join(''))) usesClock = true;
    const b = base(o); const s = comps.map((k) => (o.swz.length === 1 ? o.swz[0] : o.swz[k] || o.swz[o.swz.length - 1])).join('');
    return wrap(o, `${b}.${s}`);
  };
  const first = (o, n) => vec(o, [0, 1, 2, 3].slice(0, n)); // the first n components of the operand's own swizzle
  const destComps = (o) => o.swz.split('').map((c) => CI[c]);
  const assign = (o, expr, n, sat) => { // expr has n components (n = 1: scalar, replicated)
    const d = o.kind === 'o' ? 'o' + o.idx : 'r' + o.idx; const comps = o.swz.split('');
    if (o.kind !== 'r' && o.kind !== 'o') return bad('destination ' + o.raw);
    if (sat) expr = `saturate(${expr})`;
    if (comps.length === 1) return `${d}.${comps[0]} = ${expr}${n > 1 ? '.x' : ''};`;
    if (n === 1) return `{ let t = ${expr}; ${comps.map((c) => `${d}.${c} = t;`).join(' ')} }`;
    if (comps.length === 4 && o.swz === 'xyzw') return `${d} = ${expr};`;
    return `{ let t = ${expr}; ${comps.map((c, k) => `${d}.${c} = t.${'xyzw'[k]};`).join(' ')} }`;
  };
  const zero = (n) => (n === 1 ? '0.0' : `vec${n}<f32>(0.0)`), one = (n) => (n === 1 ? '1.0' : `vec${n}<f32>(1.0)`);
  const UN = { mov: (a) => a, sqrt: (a) => `sqrt(${a})`, rsq: (a) => `inverseSqrt(${a})`, rcp: (a) => `(1.0 / ${a})`, log: (a) => `log2(${a})`, exp: (a) => `exp2(${a})`, frc: (a) => `fract(${a})`, round_ni: (a) => `floor(${a})`, round_pi: (a) => `ceil(${a})`, round_z: (a) => `trunc(${a})`, ftoi: (a) => `trunc(${a})`, ftou: (a) => `trunc(${a})`, utof: (a) => a, round_ne: (a) => `round(${a})`,
    // (XW, --xsi only) the screen derivatives (Giant Stamp's relief decal: its parallax steps by the UV's footprint) // (VFX-XLATE) + --root
    ...(XSI || XL ? { deriv_rtx: (a) => `dpdx(${a})`, deriv_rty: (a) => `dpdy(${a})`, deriv_rtx_coarse: (a) => `dpdxCoarse(${a})`, deriv_rty_coarse: (a) => `dpdyCoarse(${a})`, deriv_rtx_fine: (a) => `dpdxFine(${a})`, deriv_rty_fine: (a) => `dpdyFine(${a})` } : {}) };
  const BIN = { iadd: (a, b) => `(${a} + ${b})`, imul: (a, b) => `(${a} * ${b})`, add: (a, b) => `(${a} + ${b})`, mul: (a, b) => `(${a} * ${b})`, div: (a, b) => `(${a} / ${b})`, min: (a, b) => `min(${a}, ${b})`, max: (a, b) => `max(${a}, ${b})`, and: (a, b) => `(${a} * ${b})`, or: (a, b) => `max(${a}, ${b})` };
  const CMP = { lt: '<', ge: '>=', eq: '==', ne: '!=', ilt: '<', ige: '>=', ieq: '==', ine: '!=', ult: '<', uge: '>=' };
  // components that hold a comparison result (all bits set in the game = 1.0 here): `itof` of one is -1.0
  const masks = new Set();
  const texel = (n, tOp, sOp, uv, lod, grad) => {
    const t = meta.texMap[tOp.idx];
    if (t === undefined) return null;
    if (t >= MAX_TEXTURES) { bad('more than ' + MAX_TEXTURES + ' textures'); return 'vec4<f32>(0.0)'; }
    const s = sOp ? meta.sampMap[sOp.idx] : null; let sn;
    if (!s) sn = `kh_s${t}`; else if (s.shared) sn = s.shared === 'wrap' ? 'kh_swrap' : 'kh_sclamp'; else if (s.tex < MAX_TEXTURES) sn = `kh_s${s.tex}`; else { bad('sampler of a texture beyond the slots'); sn = 'kh_s0'; }
    texUsed.add(t); sampUsed.add(sn);
    if (grad) return `textureSampleGrad(kh_t${t}, ${sn}, ${uv}, ${grad[0]}, ${grad[1]})`; // (XW, --xsi: sample_d)
    return `textureSampleLevel(kh_t${t}, ${sn}, ${uv}, ${lod})`;
  };
  let loopDepth = 0;
  // (XW-b, --xw-break) what each break ends: 'L' a loop, 'S' a switch case (gen runs in order over the kept statements)
  const ctl = [];
  const gen = (n) => {
    const a = n.a, d = a[0];
    switch (n.op) {
      case 'ret': return '';
      case 'else': return '} else {';
      case 'endif': return '}';
      case 'if_nz': return `if (${vec(a[0], [0])} != 0.0) {`;
      case 'if_z': return `if (${vec(a[0], [0])} == 0.0) {`;
      case 'discard_nz': return `if (${vec(a[0], [0])} != 0.0) { discard; }`;
      case 'discard_z': return `if (${vec(a[0], [0])} == 0.0) { discard; }`;
      case 'loop': loopDepth++; if (XW_BREAK) ctl.push('L'); return 'loop {';
      case 'endloop': loopDepth--; if (XW_BREAK) ctl.pop(); return '}';
      // (XW-b, --xw-break) a switch on an integer held as a float: WGSL switch, every case its own block (no fall-through)
      case 'switch': if (!XW_BREAK) break; ctl.push({ sw: true, def: false }); return `switch (i32(round(${vec(a[0], [0])}))) {`;
      case 'case': { if (!XW_BREAK) break; const g = caseGroup.get(n); if (!g) return ''; return `case ${g.join(', ')}: {`; }
      case 'default': { if (!XW_BREAK) break; const top = ctl[ctl.length - 1]; if (top && top.sw) top.def = true; const g = caseGroup.get(n); return g && g.length ? `case ${g.join(', ')}, default: {` : 'default: {'; }
      case 'endswitch': { if (!XW_BREAK) break; const top = ctl.pop(); return top && top.def ? '}' : 'default: {}\n}'; }
      case 'breakc_nz': return `if (${vec(a[0], [0])} != 0.0) { break; }`;
      case 'breakc_z': return `if (${vec(a[0], [0])} == 0.0) { break; }`;
      case 'break': { if (!((XSI && meta.decal) || XW_BREAK)) break; const top = ctl[ctl.length - 1]; return top && top.sw ? '}' : 'break;'; } // (XW, --xsi, decals only: elsewhere a plain break is still left out, as before)
    }
    if (!d || (d.kind !== 'r' && d.kind !== 'o')) { if (n.op === 'sincos') { /* below */ } else return bad('instruction ' + n.op); }
    const comps = d && d.swz ? destComps(d) : [], k = comps.length;
    const srcMask = (o) => (o.kind === 'r' ? comps.map((c) => masks.has('r' + o.idx + '.' + (o.swz.length === 1 ? o.swz[0] : o.swz[c] || o.swz[o.swz.length - 1]))) : comps.map(() => false));
    const dkeys = d.kind === 'r' ? d.swz.split('').map((c) => 'r' + d.idx + '.' + c) : [];
    const isMaskOp = !!CMP[n.op] || ((n.op === 'and' || n.op === 'or') && srcMask(a[1]).every(Boolean) && srcMask(a[2]).every(Boolean));
    const intAnd = meta.fwd && n.op === 'and' && !srcMask(a[1]).some(Boolean) && !srcMask(a[2]).some(Boolean); // (VFX-2 lit) before the masks change
    if (n.op === 'itof') { const m = srcMask(a[1]); for (const key of dkeys) masks.delete(key); if (m.every(Boolean)) return assign(d, `(-${vec(a[1], comps)})`, k, false); if (m.some(Boolean)) return bad('itof of a mixed operand'); return assign(d, vec(a[1], comps), k, false); }
    if (n.op === 'utof' && srcMask(a[1]).some(Boolean)) return bad('utof of a comparison');
    if (n.op === 'ishl') { const sh = a[2].kind === 'l' ? Number(a[2].imm[0]) : NaN; for (const key of dkeys) masks.delete(key); if (!Number.isInteger(sh) || sh < 0 || sh > 16) return bad('ishl by a register'); return assign(d, `(${vec(a[1], comps)} * ${1 << sh}.0)`, k, false); }
    for (const key of dkeys) { if (isMaskOp) masks.add(key); else masks.delete(key); }
    if (meta.fwd) { // (VFX-2 lit) the integer operations of the forward lighting, on integer values held as floats
      const per = (fn, srcs) => { const parts = comps.map((c) => `${fn}(${srcs.map((o) => vec(o, [c])).join(', ')})`); return assign(d, k === 1 ? parts[0] : `vec${k}<f32>(${parts.join(', ')})`, k, n.sat); };
      if (intAnd) return per('kh_iand', [a[1], a[2]]);
      if (n.op === 'ushr') return per('kh_ushr', [a[1], a[2]]);
      if (n.op === 'ubfe') return per('kh_ubfe', [a[1], a[2], a[3]]);
      if (n.op === 'bfi') return per('kh_bfi', [a[1], a[2], a[3], a[4]]);
      if (n.op === 'f16tof32') return per('kh_f16lo', [a[1]]);
      if (n.op === 'umin' || n.op === 'umax') return assign(d, `${n.op === 'umin' ? 'min' : 'max'}(${vec(a[1], comps)}, ${vec(a[2], comps)})`, k, false);
      if (n.op === 'imad') return assign(d, `(${vec(a[1], comps)} * ${vec(a[2], comps)} + ${vec(a[3], comps)})`, k, false);
      if (n.op === 'swapc') { // swapc d0, d1, c, s0, s1: d0 = c ? s1 : s0, d1 = c ? s0 : s1 (both from the values before it)
        const d1 = a[1]; if (d1.kind !== 'r' && d1.kind !== 'o') return bad('swapc destination ' + d1.raw);
        for (const c of d1.swz) masks.delete('r' + d1.idx + '.' + c);
        const c1 = destComps(d1), k1 = c1.length;
        const sel = (dc, kk, x, y) => `select(${vec(x, dc)}, ${vec(y, dc)}, ${vec(a[2], dc)} != ${zero(kk)})`;
        const t0 = sel(comps, k, a[3], a[4]), t1 = sel(c1, k1, a[4], a[3]);
        return `{ let kh_sw0 = ${t0}; let kh_sw1 = ${t1}; ${assign(d, 'kh_sw0', k, false)} ${assign(d1, 'kh_sw1', k1, false)} }`;
      }
    }
    if (UN[n.op]) return assign(d, UN[n.op](vec(a[1], comps)), k, n.sat);
    if (BIN[n.op]) return assign(d, BIN[n.op](vec(a[1], comps), vec(a[2], comps)), k, n.sat);
    if (n.op === 'mad') return assign(d, `(${vec(a[1], comps)} * ${vec(a[2], comps)} + ${vec(a[3], comps)})`, k, n.sat);
    if (CMP[n.op]) return assign(d, `select(${zero(k)}, ${one(k)}, ${vec(a[1], comps)} ${CMP[n.op]} ${vec(a[2], comps)})`, k, false);
    if (n.op === 'movc') return assign(d, `select(${vec(a[3], comps)}, ${vec(a[2], comps)}, ${vec(a[1], comps)} != ${zero(k)})`, k, n.sat);
    if (/^dp[234]$/.test(n.op)) { const m = +n.op[2]; return assign(d, `dot(${first(a[1], m)}, ${first(a[2], m)})`, 1, n.sat); }
    if (n.op === 'sincos') { const parts = []; if (a[0].kind !== 'null') parts.push(assign(a[0], `sin(${vec(a[2], destComps(a[0]))})`, destComps(a[0]).length, n.sat)); if (a[1].kind !== 'null') parts.push(assign(a[1], `cos(${vec(a[2], destComps(a[1]))})`, destComps(a[1]).length, n.sat)); return parts.join(' '); }
    if (n.op === 'sample' || n.op === 'sample_indexable' || n.op === 'sample_l' || n.op === 'sample_l_indexable' || n.op === 'sample_b_indexable' || n.op === 'sample_b') {
      // (VFX-2 lit) the reflection captures' cube array: the harness's environment at that direction and mip
      if (meta.fwd && n.texType === 'texturecubearray' && fwdRes[a[2].idx] === 'cube' && /sample_l/.test(n.op)) return assign(d, `kh_fwd_env(${first(a[1], 3)}, ${vec(a[4], [0])}).${comps.map((c) => a[2].swz[c]).join('')}`, k, n.sat);
      // (VFX-2 cube) a material cube map (--root / --vxl): its packed atlas through kh_cube (er_vfx.wgsl), at the direction
      if (XL && n.texType === 'texturecube' && (meta.cubeSlots || []).includes(meta.texMap[a[2].idx])) {
        const t = meta.texMap[a[2].idx]; if (t >= MAX_TEXTURES) return bad('more than ' + MAX_TEXTURES + ' textures');
        const sm = a[3] ? meta.sampMap[a[3].idx] : null; const sn = sm && sm.shared ? (sm.shared === 'wrap' ? 'kh_swrap' : 'kh_sclamp') : `kh_s${t}`;
        texUsed.add(t); sampUsed.add(sn);
        return assign(d, `kh_cube(kh_t${t}, ${sn}, ${first(a[1], 3)}).${comps.map((c) => a[2].swz[c]).join('')}`, k, n.sat);
      }
      if (n.texType && n.texType !== 'texture2d') return bad('texture type ' + n.texType);
      const isL = /sample_l/.test(n.op); const lod = isL ? vec(a[4], [0]) : '0.0';
      let s = texel(n, a[2], a[3], first(a[1], 2), lod);
      const xlScene = s === null && XL && meta.sceneTex ? meta.sceneTex[a[2].idx] : undefined; // (VFX-XLATE) which scene texture
      if (xlScene === 'color') { usesScene = true; s = `kh_scene_color(${first(a[1], 2)})`; }
      else if (xlScene === 'over') s = 'vec4<f32>(0.0, 0.0, 0.0, 1.0)';
      if (s === null) { // not a material texture: the scene depth (always read with sample_l)
        if (!isL && meta.decal) return assign(d, `vec4<f32>(0.5, 0.5, 1.0, 1.0).${comps.map((c) => a[2].swz[c]).join('')}`, k, n.sat); // GBufferA: the receiving surface's normal, taken as a floor
        if (!isL) return bad('scene texture read');
        // a loop that samples a texture which is not the material's own walks the scene colour (the blur materials):
        // there is no scene colour copy to read, and the depth stand-in would draw nonsense
        if (loopDepth > 0) return bad('scene colour read in a loop (blur)');
        // (fifth pass) a decal shader reads two scene textures with sample_l: the receiver mask (0 = receives) and the
        // receiving surface's normal (GBufferA, packed; recognised by the `* 2 - 1` that follows). The normal was given
        // as 0 too, which unpacks to (-1, -1, -1): the water ground decals fade by normal . up and so drew nothing.
        if (meta.decal) s = n.gbufferNormal ? 'vec4<f32>(0.5, 0.5, 1.0, 1.0)' : 'vec4<f32>(0.0)'; else { usesDepth = true; s = 'vec4<f32>(i.inv_scene_depth)'; }
      }
      const sw = comps.map((c) => a[2].swz[c]).join('');
      return assign(d, `${s}.${sw}`, k, n.sat);
    }
    // (XW, --xsi only) sample with explicit gradients, and a material texture's size (resinfo: width, height, 0, mip levels)
    if ((XSI || XL) && (n.op === 'sample_d' || n.op === 'sample_d_indexable')) {
      if (n.texType && n.texType !== 'texture2d') return bad('texture type ' + n.texType);
      const s = texel(n, a[2], a[3], first(a[1], 2), '0.0', [first(a[4], 2), first(a[5], 2)]);
      if (s === null) return bad('sample_d of a scene texture');
      return assign(d, `${s}.${comps.map((c) => a[2].swz[c]).join('')}`, k, n.sat);
    }
    if ((XSI || XL) && n.op === 'resinfo_indexable') {
      if (n.texType && n.texType !== 'texture2d') return bad('texture type ' + n.texType);
      const t = meta.texMap[a[2].idx]; if (t === undefined || t >= MAX_TEXTURES) return bad('resinfo of a scene texture');
      texUsed.add(t);
      const lod = a[1].kind === 'l' ? Number(a[1].imm[0]) : NaN; if (!Number.isInteger(lod)) return bad('resinfo of a register mip');
      const all = `vec4<f32>(vec2<f32>(textureDimensions(kh_t${t}, ${lod})), 0.0, f32(textureNumLevels(kh_t${t})))`;
      return assign(d, `${all}.${comps.map((c) => a[2].swz[c]).join('')}`, k, false);
    }
    // (VFX-2 lit) a forward-lit owner's light-grid buffers
    if (meta.fwd && n.op === 'ld_indexable' && n.texType === 'buffer' && fwdRes[a[2].idx] && fwdRes[a[2].idx] !== 'cube') return assign(d, `${fwdRes[a[2].idx]}(${vec(a[1], [0])}).${comps.map((c) => a[2].swz[c]).join('')}`, k, false);
    if (n.op === 'ld_indexable' || n.op === 'ld') { if (meta.texMap[a[2].idx] !== undefined) return bad('texel load of a material texture'); return assign(d, one(k), k, false); }
    return bad('instruction ' + n.op);
  };
  // ---- liveness per register component, backwards through the structured code: only what reaches the colour
  //      outputs is emitted (the deferred shaders also compute shading-model flags with integer code for other targets)
  const vsOuts = meta.vertex ? Object.entries(osem).filter(([, s]) => VS_OUT[s.name] && (!meta.only || meta.only === s.name)).map(([r, s]) => ({ reg: 'o' + r, field: VS_OUT[s.name], name: s.name })) : [];
  const wanted = meta.vertex ? [...(meta.only && meta.only !== 'SV_POSITION0' ? [] : ['kh_wpos']), ...vsOuts.map((x) => x.reg)] : meta.maskOnly ? [] : meta.decal || meta.distort ? ['o0'] : nOut >= 4 ? ['o0', 'o1', 'o2', 'o3'] : ['o0'];
  const key = (o, c) => o.kind + o.idx + '.' + c;
  const isControl = (e) => e.kind === 'ins' && (/^(if_|else|endif|ret|discard|loop|endloop|breakc_)/.test(e.n.op) || (XW_BREAK && /^(switch|case|default|endswitch)$/.test(e.n.op)));
  const writes = (e) => { // [reg.comp]
    if (e.kind === 'raw') return ['x', 'y', 'z', 'w'].map((c) => e.dest.r + '.' + c);
    const n = e.n; if (isControl(e)) return [];
    const ds = n.op === 'sincos' || (meta.fwd && n.op === 'swapc') /* (VFX-2 lit) */ ? [n.a[0], n.a[1]] : [n.a[0]];
    return ds.filter((o) => o && (o.kind === 'r' || o.kind === 'o')).flatMap((o) => o.swz.split('').map((c) => key(o, c)));
  };
  const reads = (e) => {
    if (e.kind === 'raw') return e.src || [];
    const n = e.n; const res = [];
    const add = (o, comps) => { if (o && (o.kind === 'r' || o.kind === 'o')) for (const k of comps) res.push(key(o, o.swz.length === 1 ? o.swz[0] : o.swz[k] || o.swz[o.swz.length - 1])); };
    if (/^(if_|discard|breakc_)/.test(n.op) || (XW_BREAK && n.op === 'switch')) { add(n.a[0], [0]); return res; }
    if (isControl(e)) return res;
    if (/^dp[234]$/.test(n.op)) { const m = [0, 1, 2, 3].slice(0, +n.op[2]); add(n.a[1], m); add(n.a[2], m); return res; }
    if (/^sample_d/.test(n.op)) { add(n.a[1], [0, 1]); add(n.a[4], [0, 1]); add(n.a[5], [0, 1]); return res; } // (XW, --xsi)
    // (VFX-2 lit) the cube array's direction and layer; a buffer load's index; swapc's two destinations
    if (meta.fwd && /^sample/.test(n.op) && n.texType === 'texturecubearray') { add(n.a[1], [0, 1, 2, 3]); if (n.a[4]) add(n.a[4], [0]); return res; }
    if (meta.fwd && /^ld/.test(n.op) && n.texType === 'buffer') { add(n.a[1], [0]); return res; }
    if (meta.fwd && n.op === 'swapc') { for (const d of [n.a[0], n.a[1]]) { const cs = destComps(d); for (const o of n.a.slice(2)) add(o, cs); } return res; }
    if (/^sample/.test(n.op)) { add(n.a[1], [0, 1]); if (n.a[4]) add(n.a[4], [0]); return res; }
    if (/^resinfo/.test(n.op)) return res; // (XW, --xsi)
    if (/^ld/.test(n.op)) return res;
    if (n.op === 'sincos') { for (const d of [n.a[0], n.a[1]]) if (d.kind !== 'null') add(n.a[2], destComps(d)); return res; }
    const d = n.a[0]; const comps = d && d.swz ? destComps(d) : [0];
    for (const o of n.a.slice(1)) add(o, comps);
    return res;
  };
  const needed = new Array(out.length).fill(false);
  // walk(from, to, liveOut): the instructions out[from..to) form a block; returns the live set before it
  const matchEnd = (at) => { let depth = 0, elseAt = -1; for (let j = at + 1; j < out.length; j++) { const o = out[j].kind === 'ins' ? out[j].n.op : ''; if (o === 'if_nz' || o === 'if_z') depth++; else if (o === 'endif') { if (depth === 0) return { elseAt, endAt: j }; depth--; } else if (o === 'else' && depth === 0) elseAt = j; } return null; };
  const blocks = new Map(); for (let x = 0; x < out.length; x++) { const o = out[x].kind === 'ins' ? out[x].n.op : ''; if (o === 'if_nz' || o === 'if_z') { const m = matchEnd(x); if (!m) return fail('unbalanced if'); blocks.set(x, m); } }
  const ifOfEnd = new Map(); for (const [x, m] of blocks) ifOfEnd.set(m.endAt, x);
  // loops: the body is walked until its live set stops growing; a break leaves with what is live after the loop
  const loopOfEnd = new Map(); { const stack = []; for (let x = 0; x < out.length; x++) { const o = out[x].kind === 'ins' ? out[x].n.op : ''; if (o === 'loop') stack.push(x); else if (o === 'endloop') { if (!stack.length) return fail('unbalanced loop'); loopOfEnd.set(x, stack.pop()); } } if (stack.length) return fail('unbalanced loop'); }
  // (XW-b, --xw-break) switches: per switch its case markers at its own depth; a case body runs from its marker (after any
  // markers that share it: `case 0, 1:`) to its break (or the next marker / endswitch)
  const switchOfEnd = new Map(); const caseGroup = new Map(); const switchCases = new Map();
  if (XW_BREAK) {
    const stack = [];
    for (let x = 0; x < out.length; x++) {
      const o = out[x].kind === 'ins' ? out[x].n.op : '';
      if (o === 'switch') stack.push({ at: x, marks: [] });
      else if (o === 'case' || o === 'default') { if (stack.length) stack[stack.length - 1].marks.push(x); }
      else if (o === 'endswitch') { const sw = stack.pop(); if (!sw) return fail('unbalanced switch'); switchOfEnd.set(x, sw.at); switchCases.set(sw.at, { marks: sw.marks, end: x }); }
    }
    if (stack.length) return fail('unbalanced switch');
    for (const [, sw] of switchCases) {
      let group = [];
      for (let k = 0; k < sw.marks.length; k++) {
        const m = out[sw.marks[k]].n; if (m.op === 'case') { const v = m.a[0] && m.a[0].kind === 'l' ? Number(m.a[0].imm[0]) : NaN; if (!Number.isInteger(v)) return fail('switch case of a register'); group.push(v); }
        const next = sw.marks[k + 1];
        if (next === sw.marks[k] + 1) { caseGroup.set(m, null); continue; } // the next marker shares this body
        caseGroup.set(out[sw.marks[k]].n, group); group = [];
      }
    }
  }
  const exits = [];
  const walk = (from, to, liveOut) => {
    let live = new Set(liveOut);
    for (let x = to - 1; x >= from; x--) {
      const e = out[x]; const op = e.kind === 'ins' ? e.n.op : '';
      if (op === 'endif') {
        const ifAt = ifOfEnd.get(x), m = blocks.get(ifAt);
        const thenLive = walk(ifAt + 1, m.elseAt < 0 ? m.endAt : m.elseAt, live);
        const elseLive = m.elseAt < 0 ? live : walk(m.elseAt + 1, m.endAt, live);
        // (fifth pass) a block none of whose instructions is needed has no effect: neither it nor its condition is kept
        // (a vertex shader's fog branches would otherwise keep the fog constants alive)
        let any = false; for (let y = ifAt + 1; y < x && !any; y++) if (needed[y] && !(out[y].kind === 'ins' && out[y].n.op === 'else')) any = true;
        if (!any) { x = ifAt; continue; }
        live = new Set([...thenLive, ...elseLive]); for (const r of reads(out[ifAt])) live.add(r);
        needed[x] = needed[ifAt] = true; if (m.elseAt >= 0) needed[m.elseAt] = true;
        x = ifAt; continue;
      }
      if (XW_BREAK && op === 'endswitch') {
        const swAt = switchOfEnd.get(x); const sw = switchCases.get(swAt); const union = new Set(); let def = false;
        for (let k = 0; k < sw.marks.length; k++) {
          const at = sw.marks[k]; needed[at] = true; if (out[at].n.op === 'default') def = true;
          const next = k + 1 < sw.marks.length ? sw.marks[k + 1] : x; if (next === at + 1) continue;
          let end = next; for (let y = at + 1; y < next; y++) { const oy = out[y].kind === 'ins' ? out[y].n.op : ''; if (oy === 'break' && !sw.marks.includes(y)) { // the case's own break: the first one at the case's depth
            let depth = 0; for (let z = at + 1; z < y; z++) { const oz = out[z].kind === 'ins' ? out[z].n.op : ''; if (/^(loop|switch)$/.test(oz)) depth++; else if (/^(endloop|endswitch)$/.test(oz)) depth--; } if (depth === 0) { end = y; break; } } }
          if (end < next) needed[end] = true; else return fail('a switch case falls through');
          for (const r of walk(at + 1, end, live)) union.add(r);
        }
        if (!def) for (const r of live) union.add(r);
        for (const r of reads(out[swAt])) union.add(r);
        needed[x] = needed[swAt] = true; live = union; x = swAt; continue;
      }
      if (op === 'endloop') {
        const loopAt = loopOfEnd.get(x);
        exits.push(live);
        let entry = new Set();
        for (let pass = 0; pass < 8; pass++) { const next = walk(loopAt + 1, x, entry); let grew = false; for (const r of next) if (!entry.has(r)) { entry.add(r); grew = true; } if (!grew) break; }
        exits.pop();
        live = new Set([...live, ...entry]);
        needed[x] = needed[loopAt] = true;
        x = loopAt; continue;
      }
      if (/^breakc_/.test(op)) { needed[x] = true; for (const r of reads(e)) live.add(r); for (const r of exits[exits.length - 1] || []) live.add(r); continue; }
      if (((XSI && meta.decal) || XW_BREAK) && op === 'break') { needed[x] = true; live = new Set(exits[exits.length - 1] || []); continue; } // (XW, --xsi) leaves the loop: what is live after it
      if (isControl(e)) { needed[x] = true; for (const r of reads(e)) live.add(r); continue; }
      const w = writes(e);
      if (w.some((r) => live.has(r))) { needed[x] = true; for (const r of w) live.delete(r); for (const r of reads(e)) live.add(r); }
    }
    return live;
  };
  walk(0, out.length, new Set(wanted.flatMap((o) => ['x', 'y', 'z', 'w'].map((c) => o + '.' + c))));
  const liveRegs = new Set(); for (let x = 0; x < out.length; x++) if (needed[x]) for (const r of [...writes(out[x]), ...reads(out[x])]) liveRegs.add(r.split('.')[0]);
  const live = liveRegs;
  // empty if / else blocks left by the pruning are harmless
  const code = [];
  for (let x = 0; x < out.length; x++) { if (!needed[x]) continue; const e = out[x]; const text = e.kind === 'raw' ? ((usesTwp = usesTwp || !e.vertex), e.text) : gen(e.n); if (text) code.push(text); if (why) return fail(why + "  (" + (e.n ? e.n.line : e.text) + ")"); }
  if (meta.vertex) { // (fifth pass)
    const decl = []; for (let r = 0; r < nTemps; r++) if (live.has('r' + r)) decl.push(`var r${r}: vec4<f32>;`);
    const outs = ['var kh_wpos: vec4<f32>;', ...vsOuts.map((x) => `var ${x.reg}: vec4<f32>;`)];
    const tail = ['var kh_out: KhVsOut;', ...vsOuts.map((x) => `kh_out.${x.field} = ${x.reg};`), 'kh_out.wpos = kh_wpos.xyz;', 'return kh_out;'];
    return { ok: true, notes: lenientNotes, uses_clock: usesClock, wgsl: [...decl, ...outs, ...code, ...tail].map((l) => '    ' + l).join('\n'), outputs: vsOuts.map((x) => x.name), textures: [...texUsed].sort((x, y) => x - y), cb_len: maxCb + 1, inputs: [...semUsed].sort(), view: [...viewUsed].sort((x, y) => x - y) };
  }
  const lastRet = ins.length && ins[ins.length - 1].op === 'ret';
  if (ins.filter((n) => n.op === 'ret').length !== (lastRet ? 1 : 0)) return fail('early return');
  const decl = []; for (let r = 0; r < nTemps; r++) if (live.has('r' + r)) decl.push(`var r${r}: vec4<f32>;`);
  const outs = wanted.map((o) => `var ${o}: vec4<f32>;`);
  const rows = [...rowsUsed].sort((x, y) => x - y).map((r) => `let m${r} = ${meta.rowOverride[r]};`);
  if (meta.maskOnly) return { ok: true, uses_clock: usesClock, wgsl: [...decl, ...code].map((l) => '    ' + l).join('\n'), outputs: 0, textures: [...texUsed].sort((x, y) => x - y), cb_len: maxCb + 1, uses_depth: usesDepth, uses_position: usesTwp, inputs: [...semUsed].sort(), view: [...viewUsed].sort((x, y) => x - y) };
  const wgsl = [...rows, ...decl, ...outs, ...code, `return KhOut(o0, ${!meta.decal && nOut >= 4 ? 'o1, o2, o3' : 'vec4<f32>(0.0), vec4<f32>(0.0), vec4<f32>(0.0)'});`].map((l) => '    ' + l).join('\n');
  return { ok: true, uses_clock: usesClock, wgsl, outputs: meta.decal ? 1 : nOut, textures: [...texUsed].sort((x, y) => x - y), cb_len: maxCb + 1, uses_depth: usesDepth, uses_position: usesTwp, inputs: [...semUsed].sort(), view: [...viewUsed].sort((x, y) => x - y), ...(usesScene ? { uses_scene: true } : {}) };
}

// ------------------------------------------------------------------------------------------------ main
function resourceTables(code, n2d, nCube, names) {
  // entries: uniform buffer index << 24 | resource index << 8 | bind slot; only the Material buffer's resources are the
  // material's textures (the fork's GBuffers / View buffers hold scene textures)
  const mat = names.indexOf('Material');
  // the array starts with one offset per uniform buffer (0 = no resources), each pointing at that buffer's tokens,
  // which end with 0xFFFFFFFF
  const dec = (arr) => { const o = {}; if (!arr || mat < 0 || mat >= arr.length) return o; const off = arr[mat]; if (!off || off >= arr.length) return o; for (let k = off; k < arr.length && arr[k] !== 0xFFFFFFFF; k++) { const e = arr[k]; if ((e >>> 24) !== mat) break; o[e & 0xFF] = (e >>> 8) & 0xFFFF; } return o; };
  const rt = code.resourceTable || {}; const tex = dec(rt.textureMap), samp = dec(rt.samplerMap);
  const texMap = {}, sampMap = {};
  // (VFX-2 cube) with --root / --vxl the cube textures follow the 2D ones in the material's slots (n2d + k: the record's
  // `textures` carries them as packed cube atlases, tools/vfx_cube_pack.js); without, beyond the slots as before
  const cubeOf = (res) => { const k = (res - 2 * n2d) >> 1; return XL && res >= 2 * n2d && k < nCube ? n2d + k : null; };
  const cubeSlots = [];
  for (const [bind, res] of Object.entries(tex)) if (res < 2 * n2d && res % 2 === 0) texMap[bind] = res >> 1; else if (cubeOf(res) !== null && res % 2 === 0) { texMap[bind] = cubeOf(res); cubeSlots.push(cubeOf(res)); } else texMap[bind] = 1000 + res; // beyond the 2D textures: cube (never translated)
  for (const [bind, res] of Object.entries(samp)) { if (res < 2 * n2d) sampMap[bind] = { tex: res >> 1 }; else if (cubeOf(res) !== null) sampMap[bind] = { tex: cubeOf(res) }; else { const k = res - 2 * (n2d + nCube); sampMap[bind] = k === 1 ? { shared: 'clamp' } : k === 0 ? { shared: 'wrap' } : { tex: 999 }; } }
  return { texMap, sampMap, cubeSlots };
}
// The mesh class falls back to the local vertex factory (a static-mesh material that was never meant for particles:
// the Blizzard ice trail): the same interpolants without the particle colour and dynamic parameter.
// The sprite class also serves tails and trails (materials made only for them have no sprite shaders): the Square
// Enix polygon factories have the sprite factory's interpolants; the beam / trail factory names them differently.
const VF_CLASS = { sprite: ['FParticleSpriteVertexFactory', 'FSQEXSquarePolygonVertexFactory', 'FSQEXTrianglePolygonVertexFactory', 'FParticleBeamTrailVertexFactory'], mesh: ['FMeshParticleVertexFactory', 'FLocalVertexFactory'] };
const VF_SEMANTICS = { FParticleBeamTrailVertexFactory: { TEXCOORD2: 'tc3', TEXCOORD3: 'tc2', TEXCOORD4: 'tc1' } };

function main() {
  fs.mkdirSync(CACHE, { recursive: true }); fs.mkdirSync(OUT, { recursive: true });
  const manifest = JSON.parse(fs.readFileSync(path.join(PACK, 'manifest.json'), 'utf8'));
  const texInfo = {}; for (const t of manifest.textures || []) texInfo[t.asset] = t;
  // (party VF) the party root's textures: their records from the party effect pack's manifest (main records win an asset both have)
  if (PARTY) { let n = 0; for (const t of JSON.parse(fs.readFileSync(PARTY_MANIFEST, 'utf8')).textures || []) if (!texInfo[t.asset] && fs.existsSync(path.join(PARTY_PACK, t.file))) { texInfo[t.asset] = t; n++; } console.log(`(party) ${n} texture records of the party root`); }
  // (ZETTA) several manifests separated by ';' (every stream's pack merged into the party root: DON-KIT, DON-MAGIC, ZETTA ...)
  if (PARTY && process.env.KH_PARTY_EXTRA_MANIFEST) for (const mf of process.env.KH_PARTY_EXTRA_MANIFEST.split(';').filter(Boolean)) { let n = 0; for (const t of JSON.parse(fs.readFileSync(mf, 'utf8')).textures || []) if (!texInfo[t.asset] && fs.existsSync(path.join(PARTY_PACK, t.file))) { texInfo[t.asset] = t; n++; } console.log(`(DON-MAGIC) ${n} texture records of ${mf}`); }
  if (XSI) { let n = 0; for (const t of JSON.parse(fs.readFileSync(path.join(XSI_PACK, 'manifest.json'), 'utf8')).textures || []) if (!texInfo[t.asset] && fs.existsSync(path.join(XSI_PACK, t.file))) { texInfo[t.asset] = t; n++; } console.log(`(xsi) ${n} texture records of the XSI root`); }
  if (XSI_EXTRA) { let n = 0; for (const t of JSON.parse(fs.readFileSync(path.join(XSI_EXTRA, 'manifest.json'), 'utf8')).textures || []) if (!texInfo[t.asset] && fs.existsSync(path.join(XSI_EXTRA, t.file))) { texInfo[t.asset] = t; n++; } console.log(`(XW-c) ${n} texture records of ${XSI_EXTRA}`); }
  const matDir0 = (f) => (fs.existsSync(path.join(PACK, 'materials', f)) || !PARTY ? path.join(PACK, 'materials') : fs.existsSync(path.join(PARTY_PACK, 'materials', f)) || !XSI ? path.join(PARTY_PACK, 'materials') : path.join(XSI_PACK, 'materials')); // (XSI) main, party, XSI
  const baseFiles = new Set([...fs.readdirSync(path.join(PACK, 'materials')), ...(PARTY ? fs.readdirSync(path.join(PARTY_PACK, 'materials')) : []), ...(XSI ? fs.readdirSync(path.join(XSI_PACK, 'materials')) : [])]);
  // (XW-c) the extra pack's materials that no root has
  const extraFiles = XSI_EXTRA ? new Set(fs.readdirSync(path.join(XSI_EXTRA, 'materials')).filter((f) => !baseFiles.has(f))) : new Set();
  const matDir = (f) => (extraFiles.has(f) ? path.join(XSI_EXTRA, 'materials') : matDir0(f));
  const matFiles = [...new Set([...baseFiles, ...extraFiles])].filter((f) => f.endsWith('.json')).sort();
  // keyed by the pack's file name: two assets may share a base name (17 do; the pack suffixes the later one's file)
  const mats = matFiles.map((f) => ({ ...JSON.parse(fs.readFileSync(path.join(matDir(f), f), 'utf8')), key: f.replace(/.json$/, '') }));
  // ---- owners
  const owners = new Map(); // asset -> { name, sm }
  const matOwner = {};
  for (const m of mats) {
    let owner = null;
    for (const p of m.parent_chain || []) { const d = loadPkg(p); if (d && d.shaderMap) { owner = p; break; } }
    matOwner[m.key] = owner;
    if (owner && !owners.has(owner)) owners.set(owner, { asset: owner, name: owner.split('/').pop(), sm: loadPkg(owner).shaderMap });
  }
  const sorted = [...owners.values()].sort((a, b) => (a.asset < b.asset ? -1 : 1));
  // (XW-c) owners that only the extra materials use come last (the other ids stay)
  const baseOwners = new Set(mats.filter((m) => !extraFiles.has(m.key + '.json')).map((m) => matOwner[m.key]));
  let list = [...sorted.filter((o) => baseOwners.has(o.asset)), ...sorted.filter((o) => !baseOwners.has(o.asset))];
  // (ZETTA) KH_KEEP_OWNER_IDS=<an earlier shading.json of this root>: every owner of that table keeps its id (the overlays
  // shading_xwb.json / shading_vfb.json and the built er/<s|m|d|v><id> files are keyed by owner id), and owners new to this
  // run (the materials streams added to the root since: DON-KIT's, DON-MAGIC's, ZETTA's event / DLC effects) are appended
  // after the last old id in asset order. An old owner no materials use any more keeps its slot (its shader is still built).
  // Without the variable nothing changes.
  if (process.env.KH_KEEP_OWNER_IDS) {
    const old = JSON.parse(fs.readFileSync(path.resolve(process.env.KH_KEEP_OWNER_IDS), 'utf8'));
    const oldList = Object.values(old.owners || {}).sort((a, b) => a.id - b.id);
    if (oldList.some((o, i) => o.id !== i)) throw new Error('KH_KEEP_OWNER_IDS: the old table\'s owner ids are not 0..n-1');
    const byAsset = new Map(list.map((o) => [o.asset, o]));
    const kept = oldList.map((o) => byAsset.get(o.asset) || (loadPkg(o.asset) && loadPkg(o.asset).shaderMap ? { asset: o.asset, name: o.asset.split('/').pop(), sm: loadPkg(o.asset).shaderMap } : null));
    if (kept.some((o) => !o)) throw new Error('KH_KEEP_OWNER_IDS: an old owner has no shader map now: ' + oldList.filter((o, i) => !kept[i]).map((o) => o.asset).join(', '));
    const keptSet = new Set(oldList.map((o) => o.asset));
    const added = list.filter((o) => !keptSet.has(o.asset));
    list = [...kept, ...added];
    console.log(`(ZETTA) KH_KEEP_OWNER_IDS: ${kept.length} owner ids kept from ${process.env.KH_KEEP_OWNER_IDS}, ${added.length} new owner(s) appended from id ${kept.length}`);
  }
  if (XSI_EXTRA) list.forEach((o, i) => { if (!baseOwners.has(o.asset)) XWC_KEYS.add(`${String(i).padStart(3, '0')}_${o.name}`); });
  list.forEach((o, i) => { if (XWD_TWP.has(o.name)) XWC_KEYS.add(`${String(i).padStart(3, '0')}_${o.name}`); }); // (XW-d)
  { const seen = new Set(); list.forEach((o, i) => { o.id = i; o.key = seen.has(o.name) ? o.name + '@' + i : o.name; seen.add(o.key); }); }
  // ---- bytecode
  let wrote = 0;
  for (const o of list) {
    o.cls = {};
    const ue = o.sm.uniformExpressionSet || {};
    o.n2d = (ue.uniform2DTextureExpressions || []).length; o.nCube = (ue.uniformCubeTextureExpressions || []).length;
    { // the deferred-decal shader of a decal material
      const s = (o.sm.shaders || []).find((x) => x.type === 'FDeferredDecalPS');
      if (!s) o.cls.decal = { ok: false, why: 'no FDeferredDecalPS' };
      else {
        const code = s.shader.resource.code; const key = `${String(o.id).padStart(3, '0')}_${o.name}.decal`;
        const bin = path.join(CACHE, key + '.bin'); const buf = Buffer.from(code.dxbc, 'hex');
        if (!fs.existsSync(bin) || !fs.readFileSync(bin).equals(buf)) { fs.writeFileSync(bin, buf); const asm = bin.replace(/\.bin$/, '.asm'); if (fs.existsSync(asm)) fs.unlinkSync(asm); wrote++; }
        const names = ((code.optionalData || []).find((x) => x.key === 'u') || {}).uniformBufferNames || [];
        // rows of the material buffer that hold a parameter the decal emitter drives
        const vecs = ue.uniformVectorExpressions || [], scas = ue.uniformScalarExpressions || []; const rowOverride = {};
        const dyn = (e) => { const t = T(e); if ((t === 'VectorParameter' || t === 'ScalarParameter') && DECAL_PARAMS[e.parameterName]) return DECAL_PARAMS[e.parameterName]; return null; };
        vecs.forEach((e, i) => { const d = dyn(e); const sw = T(e) === 'ComponentSwizzle' && T(e.x) === 'VectorParameter' && DECAL_PARAMS[e.x.parameterName] && [e.indexR, e.indexG, e.indexB].join() === '0,1,2' ? DECAL_PARAMS[e.x.parameterName] : null; if (d || sw) rowOverride[i] = `vec4<f32>(${d || sw}.xyz, kh.m[${i}].w)`; });
        // (XW, --xsi only) a scalar that is arithmetic over a parameter the emitter drives (Giant Stamp's relief decal reveals
        // itself by Decal_CustomParam0 x 2 - 1): the expression in WGSL over the harness input, instead of its value at rest
        const folded = (e) => { if (!e) return null; const t = T(e); if ((t === 'ScalarParameter') && DECAL_PARAMS[e.parameterName]) return DECAL_PARAMS[e.parameterName]; if (t === 'Constant') return fl(String((e.value || {}).r ?? 0)); if (t === 'FoldedMath') { const op = ['+', '-', '*', '/'][e.op]; const x = folded(e.a), y = folded(e.b); return op && x !== null && y !== null ? `(${x} ${op} ${y})` : null; } return null; };
        // (only for the decals named by `--xw-decals A,B`: the XSI table's other decals keep their rows at rest, as ER90 built them)
        const foldedDyn = (e) => ((((XSI || PARTY) && (XW_DECALS.has(o.name) || XW_DECALS.has('all'))) || XL) && e && T(e) === 'FoldedMath' && JSON.stringify(e).includes('Decal_') ? folded(e) : null);
        for (let r = 0; r * 4 < scas.length; r++) { const parts = [0, 1, 2, 3].map((c) => { const e = scas[r * 4 + c]; const d = e && (dyn(e) || foldedDyn(e)); return d || `kh.m[${vecs.length + r}].${'xyzw'[c]}`; }); if (parts.some((p) => /^i\./.test(p))) rowOverride[vecs.length + r] = `vec4<f32>(${parts.join(', ')})`; }
        o.cls.decal = { key, vf: 'FDeferredDecalPS', decal: true, rowOverride, ...(XSI || XL ? { decalRows: decalRowsOf(s.shader.parameters) } : {}) /* (VFX-XLATE) + --root / --vxl */, semantics: {}, ubNames: names.map((n) => (typeof n === 'string' ? n : n.s)), ...resourceTables(code, o.n2d, o.nCube, names.map((n) => (typeof n === 'string' ? n : n.s))) };
      }
    }
    for (const [cls, vfs] of Object.entries(VF_CLASS)) {
      const msm = vfs.map((v) => (o.sm.meshShaderMaps || []).find((m) => m.vertexFactoryType === v && m.shaders.some((s) => s.type === 'TBasePassPSFNoLightMapPolicy'))).find(Boolean); if (!msm) { o.cls[cls] = { ok: false, why: 'no ' + vfs[0] + ' shaders' }; continue; }
      const s0 = msm.shaders.find((s) => s.type === 'TBasePassPSFNoLightMapPolicy'); if (!s0) { o.cls[cls] = { ok: false, why: 'no base-pass pixel shader' }; continue; }
      const ilcS = FWL ? fwlIlcShader(o, msm, s0) : null; // (PARTY-NATIVE) --fwl: the lit translucent's ILC permutation
      const s = ilcS || s0;
      const code = s.shader.resource.code; const key = `${String(o.id).padStart(3, '0')}_${o.name}.${cls}`;
      const pkey = key + (ilcS ? '.ilc' : ''); // (PARTY-NATIVE) the pixel shader's own cache name (mask / vertex / distortion keep `key`)
      const bin = path.join(CACHE, pkey + '.bin'); const buf = Buffer.from(code.dxbc, 'hex');
      if (!fs.existsSync(bin) || !fs.readFileSync(bin).equals(buf)) { fs.writeFileSync(bin, buf); const asm = bin.replace(/\.bin$/, '.asm'); if (fs.existsSync(asm)) fs.unlinkSync(asm); wrote++; }
      const names = ((code.optionalData || []).find((x) => x.key === 'u') || {}).uniformBufferNames || [];
      // A masked material clips in the depth-only pass (the base pass then draws with an equal depth test and has no
      // clip of its own): its FDepthOnlyPS is translated as the mask.
      let mask = null;
      const dps = /BlendMode=BLEND_Masked/.test(o.sm.debugDescription || '') ? msm.shaders.find((x) => x.type === 'FDepthOnlyPS') : null;
      if (dps) {
        const mcode = dps.shader.resource.code; const mkey = key + '.mask'; const mbin = path.join(CACHE, mkey + '.bin'); const mbuf = Buffer.from(mcode.dxbc, 'hex');
        if (!fs.existsSync(mbin) || !fs.readFileSync(mbin).equals(mbuf)) { fs.writeFileSync(mbin, mbuf); const asm = mbin.replace(/.bin$/, '.asm'); if (fs.existsSync(asm)) fs.unlinkSync(asm); wrote++; }
        const mnames = (((mcode.optionalData || []).find((x) => x.key === 'u') || {}).uniformBufferNames || []).map((n) => (typeof n === 'string' ? n : n.s));
        mask = { key: mkey, maskOnly: true, semantics: VF_SEMANTICS[msm.vertexFactoryType] || {}, ubNames: mnames, ...resourceTables(mcode, o.n2d, o.nCube, mnames), ...(XL ? { perFrame: perFrameOf(dps.shader.parameters, ue) } : {}) /* (AUDIT-A IMAGES) */ };
      }
      // (fifth pass) the same factory's base-pass vertex shader (world position offset, the interpolants) and the pixel
      // shader of its distortion pass (refraction: offsets accumulated for the scene-colour lookup)
      const extra = (type, suffix, more) => {
        const x = msm.shaders.find((y) => (type instanceof RegExp ? type.test(y.type) : y.type === type)); if (!x) return null;
        const xc = x.shader.resource.code; const xkey = key + suffix; const xbin = path.join(CACHE, xkey + '.bin'); const xbuf = Buffer.from(xc.dxbc, 'hex');
        if (!fs.existsSync(xbin) || !fs.readFileSync(xbin).equals(xbuf)) { fs.writeFileSync(xbin, xbuf); const asm = xbin.replace(/\.bin$/, '.asm'); if (fs.existsSync(asm)) fs.unlinkSync(asm); wrote++; }
        const xn = (((xc.optionalData || []).find((y) => y.key === 'u') || {}).uniformBufferNames || []).map((n) => (typeof n === 'string' ? n : n.s));
        return { key: xkey, ...more, ubNames: xn, ...resourceTables(xc, o.n2d, o.nCube, xn), ...(XL ? { perFrame: perFrameOf(x.shader.parameters, ue) } : {}) /* (AUDIT-A IMAGES) */ };
      };
      const vs = extra('TBasePassVSFNoLightMapPolicy', '.vs', { vertex: true });
      const dist = /Distorted=1/.test(o.sm.debugDescription || '') ? extra(/^TDistortionMeshPS/, '.dist', { distort: true, semantics: VF_SEMANTICS[msm.vertexFactoryType] || {} }) : null;
      if (VFB && XWC_KEYS.has(`${String(o.id).padStart(3, '0')}_${o.name}`)) { // (party VF-b)
        const R = (ue.uniformVectorExpressions || []).length + Math.ceil((ue.uniformScalarExpressions || []).length / 4);
        const coll = (ue.parameterCollections || []).length ? vfbCollection(ue.parameterCollections[0]) : null;
        o.vfb = { R, coll };
        if (mask) mask.vfb = o.vfb;
        if (vs) vs.vfb = o.vfb;
      }
      o.cls[cls] = { key: pkey /* (PARTY-NATIVE) */, mask, vs, dist, vfb: o.vfb, vf: msm.vertexFactoryType, ...(ilcS ? { ilc: true } : {}) /* (PARTY-NATIVE) --fwl */, ...(XL ? { sceneTex: sceneTexOf(s.shader.parameters), perFrame: perFrameOf(s.shader.parameters, ue) } : {}) /* (VFX-XLATE) (AUDIT-A IMAGES: perFrame) */, ...(XL && names.some((n) => (typeof n === 'string' ? n : n.s) === 'ForwardGlobalLightData') ? { fwd: true } : {}) /* (VFX-2 lit) */, semantics: VF_SEMANTICS[msm.vertexFactoryType] || {}, ubNames: names.map((n) => (typeof n === 'string' ? n : n.s)), ...resourceTables(code, o.n2d, o.nCube, names.map((n) => (typeof n === 'string' ? n : n.s))) };
    }
  }
  const missing = fs.readdirSync(CACHE).filter((f) => f.endsWith('.bin') && !fs.existsSync(path.join(CACHE, f.replace(/\.bin$/, '.asm'))));
  if (missing.length) {
    console.log(`disassembling ${missing.length} shaders (d3dcompiler_47.dll) ...`);
    const r = require('./lib/dxbc_disasm').systemOrOwn(CACHE);
    if (r.status !== 0) throw new Error('dxbc_disasm.ps1 failed: ' + (r.stderr || r.stdout));
  }
  // ---- (VFX-XLATE, --root) per owner: its collections' rows after its own, and a row map when they pass the harness's 64
  if (XL) {
    let compacted = 0;
    for (const o of list) {
      const ue = o.sm.uniformExpressionSet || {};
      const R = (ue.uniformVectorExpressions || []).length + Math.ceil((ue.uniformScalarExpressions || []).length / 4);
      const colls = []; let base = R;
      for (const g of ue.parameterCollections || []) { const c = xlCollection(g); colls.push(c ? { ...c, base } : { asset: 'missing ' + g, rows: null, base }); if (c) base += c.rows.length; }
      let map = null;
      if (base > MAX_CB) { // the rows the owner's shaders read (every class, mask, vertex and distortion stage): renumbered
        const used = new Set(); let decal = false;
        for (const c of Object.values(o.cls)) for (const m of [c, c.mask, c.vs, c.dist]) {
          if (!m || !m.key) continue; if (m === c && c.decal) decal = true;
          const f = path.join(CACHE, m.key + '.asm'); if (!fs.existsSync(f)) continue;
          for (const x of fs.readFileSync(f, 'utf8').matchAll(/cb(\d+)\[(\d+)\]/g)) { const ub = m.ubNames[+x[1]]; if (ub === 'Material') used.add(+x[2]); else if (/^MaterialCollection[0-9]$/.test(ub || '') && colls[+ub.slice(-1)]) used.add(colls[+ub.slice(-1)].base + +x[2]); }
        }
        if (!decal && used.size <= MAX_CB) { map = new Map([...used].sort((x, y) => x - y).map((L, P) => [L, P])); compacted++; }
      }
      o.xl = { R, colls, map, total: base };
      for (const c of Object.values(o.cls)) for (const m of [c, c.mask, c.vs, c.dist]) if (m && m.key) m.xl = o.xl;
    }
    console.log(`(VFX-XLATE) ${list.filter((o) => o.xl.colls.length).length} owners read a parameter collection; ${compacted} owners compacted into ${MAX_CB} rows`);
  }
  // ---- translate
  const wgsl = ['// GENERATED by tools/sora_vfx_shaders.js from the compiled base-pass pixel shaders of the effect materials. Do not edit.', ''];
  const outOwners = {}; const reasons = {}; let okCount = 0, total = 0;
  const vsStats = { ok: 0, fail: 0, wpoOk: 0, wpoFail: 0, distOk: 0, distFail: 0, spriteDep: {} };
  for (const o of list) {
    const rec = { id: o.id, asset: o.asset, description: (o.sm.debugDescription || '').replace(/^Compiling [^:]*: /, '') };
    for (const cls of [...Object.keys(VF_CLASS), 'decal']) {
      const c = o.cls[cls]; if (cls === 'decal' && !c.key) { rec[cls] = { ok: false, why: c.why }; continue; } total++;
      if (!c.key) { rec[cls] = { ok: false, why: c.why }; continue; }
      const asmFile = path.join(CACHE, c.key + '.asm');
      const asm = fs.existsSync(asmFile) ? fs.readFileSync(asmFile, 'utf8') : '';
      const t = /^HRESULT|^$/.test(asm) ? { ok: false, why: 'disassembly failed' } : transpile(asm, c);
      if (t.ok && t.cb_len > MAX_CB) { t.ok = false; t.why = 'material constant buffer too large'; }
      if (!t.ok) { rec[cls] = { ok: false, why: t.why }; const k = t.why.replace(/\s+\(.*$/, ''); (reasons[k] = reasons[k] || []).push(o.name + '.' + cls); continue; }
      okCount++;
      rec[cls] = { ok: true, vf: c.vf, outputs: t.outputs, textures: t.textures, cb_len: t.cb_len, uses_depth: t.uses_depth, uses_time: t.view.includes(128) || !!t.uses_clock, inputs: t.inputs, ...(t.uses_scene ? { scene: true } : {}), ...(c.ilc ? { ilc: true } : {}) /* (PARTY-NATIVE) --fwl: from the ILC permutation */ }; // (VFX-XLATE) scene: reads the frame behind it
      let maskFn = null;
      if (c.mask) {
        const mf = path.join(CACHE, c.mask.key + '.asm'); const masm = fs.existsSync(mf) ? fs.readFileSync(mf, 'utf8') : '';
        const mt = /^HRESULT|^$/.test(masm) ? { ok: false, why: 'disassembly failed' } : transpile(masm, c.mask);
        if (mt.ok && /discard/.test(mt.wgsl)) { maskFn = mt.wgsl; rec[cls].mask = true; if (mt.uses_clock) rec[cls].uses_time = true; rec[cls].textures = [...new Set([...rec[cls].textures, ...mt.textures])].sort((x, y) => x - y); }
        else rec[cls].mask_why = mt.ok ? 'no clip in the depth-only shader' : mt.why;
      }
      // (fifth pass) vertex stage: translated for the mesh-particle factory (the harness hands it the mesh's own vertices
      // and the instance transform); for the sprite-like factories only analysed: which of the vertex shader's outputs
      // depend on the material (constants, textures), i.e. what the harness's own quad set-up does not reproduce
      let vsFn = null, distFn = null;
      const readAsm = (k) => { const f = path.join(CACHE, k + '.asm'); const a = fs.existsSync(f) ? fs.readFileSync(f, 'utf8') : ''; return /^HRESULT|^$/.test(a) ? null : a; };
      const wpo = /ModifiesMeshPosition=1/.test(o.sm.debugDescription || '');
      if (c.vs) {
        const vasm = readAsm(c.vs.key);
        if (cls === 'mesh' && c.vf === 'FMeshParticleVertexFactory') {
          const vt = vasm ? transpile(vasm, c.vs) : { ok: false, why: 'disassembly failed' };
          if (vt.ok && vt.cb_len > MAX_CB) { vt.ok = false; vt.why = 'material constant buffer too large'; }
          if (vt.ok) { vsFn = vt.wgsl; rec[cls].vs = true; rec[cls].vs_inputs = vt.inputs; rec[cls].vs_outputs = vt.outputs; rec[cls].vs_textures = vt.textures; rec[cls].textures = [...new Set([...rec[cls].textures, ...vt.textures])].sort((x, y) => x - y); if (vt.uses_clock || vt.view.includes(128)) rec[cls].uses_time = true; vsStats.ok++; if (wpo) vsStats.wpoOk++; }
          else { rec[cls].vs_why = vt.why; vsStats.fail++; if (wpo) vsStats.wpoFail++; const k = 'vertex: ' + vt.why.replace(/\s+\(.*$/, ''); (reasons[k] = reasons[k] || []).push(o.name + '.' + cls); }
        } else if (vasm) {
          const dep = [];
          for (const only of ['SV_POSITION0', 'TEXCOORD0', 'TEXCOORD1', 'TEXCOORD2', 'TEXCOORD3', 'TEXCOORD4', 'COLOR0', 'COLOR1', 'COLOR2']) { const a = transpile(vasm, { ...c.vs, lenient: true, only }); if (a.ok && (a.cb_len > 0 || a.textures.length)) dep.push(only); }
          if (dep.length) { rec[cls].vs_material_outputs = dep; (vsStats.spriteDep[dep.join('+')] = vsStats.spriteDep[dep.join('+')] || []).push(o.name + '.' + cls); }
        }
      }
      if (c.dist) {
        const dasm = readAsm(c.dist.key); const dt = dasm ? transpile(dasm, c.dist) : { ok: false, why: 'disassembly failed' };
        if (dt.ok && dt.cb_len > MAX_CB) { dt.ok = false; dt.why = 'material constant buffer too large'; }
        if (dt.ok) { distFn = dt.wgsl; rec[cls].distort = true; rec[cls].textures = [...new Set([...rec[cls].textures, ...dt.textures])].sort((x, y) => x - y); if (dt.uses_clock || dt.view.includes(128)) rec[cls].uses_time = true; vsStats.distOk++; }
        else { rec[cls].distort_why = dt.why; vsStats.distFail++; const k = 'distortion: ' + dt.why.replace(/\s+\(.*$/, ''); (reasons[k] = reasons[k] || []).push(o.name + '.' + cls); }
      }
      wgsl.push(`#ifdef KH_S${o.id}_${cls.toUpperCase()}`, `// ${o.asset} (${cls})`, 'fn kh_material(i: KhIn) -> KhOut {', t.wgsl, '}', ...(maskFn ? ['fn kh_mask(i: KhIn) {', maskFn, '}'] : []), ...(distFn ? ['#ifdef KH_DISTORT', 'fn kh_distort(i: KhIn) -> KhOut {', distFn, '}', '#endif'] : []), ...(vsFn ? ['#ifdef KH_VS', 'fn kh_vertex(i: KhVsIn) -> KhVsOut {', vsFn, '}', '#endif'] : []), '#endif', '');
    }
    if (o.xl && (o.xl.colls.length || o.xl.map)) rec.xl = { rows: o.xl.total, collections: o.xl.colls.map((c) => ({ asset: c.asset, base: c.base, rows: c.rows ? c.rows.length : 0, runtime_values: c.set || [] })), ...(o.xl.map ? { row_map: [...o.xl.map.keys()] } : {}) }; // (VFX-XLATE)
    outOwners[o.key] = rec;
  }
  // ---- per material: constant buffer and textures
  const outMats = {}; const checks = { total: 0, bad: 0, examples: [] }; const missingTex = new Set();
  for (const m of mats) {
    const ownerAsset = matOwner[m.key]; if (!ownerAsset) { outMats[m.key] = { owner: null }; continue; }
    const o = owners.get(ownerAsset); const ue = o.sm.uniformExpressionSet || {};
    const vec = (ue.uniformVectorExpressions || []).map((e) => bindExpr(e, m)), sca = (ue.uniformScalarExpressions || []).map((e) => bindExpr(e, m));
    const cb = []; const timed = []; let badExpr = false;
    const params = []; // (VFX-2) the constants a parameter feeds: {at, n, tree} (the tree's leaves name their parameter)
    const usesParam = (n) => !!n.p || (n.a || []).some(usesParam);
    vec.forEach((t, i) => { if (isBad(t)) badExpr = true; const v = evalTree(t, 0); cb.push(...v); if (usesTime(t)) timed.push({ at: i * 4, n: 4, tree: t }); if (XL && usesParam(t)) params.push({ at: i * 4, n: 4, tree: t }); });
    sca.forEach((t, i) => { if (isBad(t)) badExpr = true; cb.push(evalTree(t, 0)[0]); if (usesTime(t)) timed.push({ at: vec.length * 4 + i, n: 1, tree: t }); if (XL && usesParam(t)) params.push({ at: vec.length * 4 + i, n: 1, tree: t }); });
    while (cb.length % 4) cb.push(0);
    if (o.xl && (o.xl.colls.length || o.xl.map)) { // (VFX-XLATE) the collections' rows, then the owner's row map
      for (const c of o.xl.colls) { while (cb.length < c.base * 4) cb.push(0); if (c.rows) for (const r of c.rows) cb.push(...r); }
      if (o.xl.map) {
        const full = cb.slice(); cb.length = 0; for (let k = 0; k < MAX_CB * 4; k++) cb.push(0);
        for (const [L, P] of o.xl.map) for (let c = 0; c < 4; c++) cb[P * 4 + c] = full[L * 4 + c] || 0;
        for (let k = timed.length - 1; k >= 0; k--) { const L = timed[k].at >> 2, P = o.xl.map.get(L); if (P === undefined) timed.splice(k, 1); else timed[k] = { ...timed[k], at: P * 4 + (timed[k].at & 3) }; }
        for (let k = params.length - 1; k >= 0; k--) { const L = params[k].at >> 2, P = o.xl.map.get(L); if (P === undefined) params.splice(k, 1); else params[k] = { ...params[k], at: P * 4 + (params[k].at & 3) }; } // (VFX-2)
        while (cb.length > 4 && cb.slice(-4).every((x) => x === 0) && cb.length / 4 > Math.max(...o.xl.map.values()) + 1) cb.length -= 4;
      }
    }
    if (o.vfb) { // (party VF-b) the collection's rows from R, the bounds rows 62 / 63
      while (cb.length < MAX_CB * 4) cb.push(0);
      (o.vfb.coll ? o.vfb.coll.rows : []).forEach((r, k) => { if (o.vfb.R + k < VFB_ROW_MIN) for (let c = 0; c < 4; c++) cb[(o.vfb.R + k) * 4 + c] = r[c]; });
      for (let c = 0; c < 3; c++) { cb[VFB_ROW_MIN * 4 + c] = VFB_BOUNDS[c]; cb[VFB_ROW_MAX * 4 + c] = VFB_BOUNDS[3 + c]; }
    }
    const rootRefs = (loadPkg(m.root_material) || {}).textureRefs || [];
    if (process.env.KH_SHADER_DEBUG && m.name === process.env.KH_SHADER_DEBUG) console.log('debug', m.name, 'root', m.root_material, 'refs', JSON.stringify(rootRefs), 'expressions', JSON.stringify(ue.uniform2DTextureExpressions));
    const textures = (ue.uniform2DTextureExpressions || []).map((e) => {
      // a texture parameter: the instance's value (the pack resolved it by name); a plain Texture expression: the root
      // material's referenced texture of that index
      const byParam = e.parameterName && m.textures && m.textures[e.parameterName];
      const asset = byParam ? byParam.texture : rootRefs[e.textureIndex];
      if (e.parameterName) { checks.total++; const dflt = byParam && (byParam.overrides || (/^instance:/.test(byParam.from || '') ? null : byParam.texture)); if (dflt && rootRefs[e.textureIndex] !== dflt) { checks.bad++; if (checks.examples.length < 5) checks.examples.push(m.name + ':' + e.parameterName + ' index ' + e.textureIndex + ' -> ' + rootRefs[e.textureIndex] + ' but default is ' + dflt); } }
      const info = asset && texInfo[asset];
      if (!info) { if (asset) missingTex.add(asset); return null; }
      return { file: info.file, srgb: !!info.srgb, address_x: info.address_x || 'TA_Wrap', address_y: info.address_y || 'TA_Wrap' };
    });
    // (VFX-2 cube) --root / --vxl: the cube textures after the 2D ones (the slots resourceTables gives them): the packed atlas
    // of the parameter's cube (tools/vfx_cube_pack.js: textures/cubes.json), clamped
    if (XL) for (const e of ue.uniformCubeTextureExpressions || []) {
      const byParam = e.parameterName && m.textures && m.textures[e.parameterName];
      const c = byParam && CUBES[byParam.texture];
      textures.push(c && c.file ? { file: c.file, srgb: false, address_x: 'TA_Clamp', address_y: 'TA_Clamp', cube: true } : null);
      if (!(c && c.file)) missingTex.add((byParam && byParam.texture) || 'cube of ' + m.key);
    }
    outMats[m.key] = { owner: o.key, asset: m.asset, ...(m.root_properties && m.root_properties.MaterialDomain === 'MD_DeferredDecal' ? { decal_blend: m.root_properties.DecalBlendMode || 'DBM_Translucent' } : {}), id: o.id, blend_mode: m.blend_mode, two_sided: !!m.two_sided, shading_model: m.shading_model, clip: m.opacity_mask_clip_value, cb: cb.map((x) => +(+x).toPrecision(9)), ...(timed.length ? { timed } : {}), ...(params.length ? { params } : {}) /* (VFX-2) */, ...(badExpr ? { bad_uniform_expression: true } : {}), textures };
  }
  // (party VF) a party / check run writes NEW files: the old ones may be hard links shared with harness copies of the pack
  if (PARTY || XSI || OUT_ARG > 0 || OWN_ROOT) for (const f of ['materials.wgsl', 'shading.json']) if (fs.existsSync(path.join(OUT, f))) fs.unlinkSync(path.join(OUT, f));
  fs.writeFileSync(path.join(OUT, 'materials.wgsl'), wgsl.join('\n'));
  fs.writeFileSync(path.join(OUT, 'shading.json'), JSON.stringify({ format: 'kh3-sora-vfx-shading/1', generator: 'tools/sora_vfx_shaders.js', max_textures: MAX_TEXTURES, max_cb: MAX_CB, owners: outOwners, materials: outMats }, null, 0));
  console.log(`${mats.length} materials, ${list.length} shader owners, ${okCount} of ${total} pixel shaders translated (${wrote} new bytecode files)`);
  console.log(`texture-index reconstruction: ${checks.total - checks.bad} of ${checks.total} texture parameters agree${checks.bad ? ' (' + checks.examples.join('; ') + ')' : ''}; ${missingTex.size} referenced textures are not in the pack${missingTex.size ? ': ' + [...missingTex].slice(0, 6).join(', ') : ''}`);
  console.log(`vertex stage (mesh particles): ${vsStats.ok} vertex shaders translated, ${vsStats.fail} not (with a world position offset: ${vsStats.wpoOk} translated, ${vsStats.wpoFail} not); distortion pass: ${vsStats.distOk} pixel shaders translated, ${vsStats.distFail} not`);
  for (const [k, v] of Object.entries(vsStats.spriteDep).sort((a, b) => b[1].length - a[1].length)) console.log(`  sprite-like vertex shader computes from the material: ${k}: ${v.length}${REPORT ? ' (' + v.join(', ') + ')' : ' e.g. ' + v.slice(0, 4).join(', ')}`);
  const noOwner = mats.filter((m) => !matOwner[m.key]).length; if (noOwner) console.log(`${noOwner} materials have no shader map in their parent chain`);
  for (const [k, v] of Object.entries(reasons).sort((a, b) => b[1].length - a[1].length)) console.log(`  not translated: ${k}: ${v.length}${REPORT ? ' (' + v.join(', ') + ')' : ' e.g. ' + v.slice(0, 4).join(', ')}`);
  // (XW-b) every shader with a plain break (a loop's exit or a switch case's end; translated with --xw-break under --xsi / --party; left out otherwise)
  for (const [k, n] of [...BREAKS].sort()) console.log(`XW-b break: ${k} (${n.breaks} break(s)${n.switches ? ', ' + n.switches + ' switch(es)' : ''})${XW_BREAK ? ' translated' : n.switches ? ' LEFT OUT with its switch: every case runs, the last one wins' : ' LEFT OUT: the loop runs past its exit'}`);
}
main();
