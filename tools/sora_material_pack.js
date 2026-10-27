#!/usr/bin/env node
// tools/sora_material_pack.js: Sora's and the Kingdom Key's materials as the game shades them.
//
// The character materials of the game are deferred-lit Unreal materials whose graphs are compiled into the cooked
// packages (docs/formats/material.md): a base-pass pixel shader per vertex factory writes the G-buffer (base colour,
// normal, metallic / specular / roughness, the subsurface colour and opacity of the Subsurface shading model) and the
// emissive + indirect term; the lights are then applied by the engine's deferred light shader (global shader cache).
// This tool takes, for every material slot of the Sora pack's model and of the Kingdom Key:
//   - the material instance chain: every scalar / vector / texture parameter (most derived wins);
//   - the shader map of the nearest package of the chain that owns one (static permutation);
//   - its base-pass pixel shader for the GPU-skin vertex factory (TBasePassPSFCachedPointIndirectLightingPolicy; a
//     translucent material: the same class, forward shaded) and, for a masked material, FDepthOnlyPS as the mask;
//   - disassembles them with the system's D3D compiler (tools/lib/dxbc_disasm.ps1) and translates the instruction
//     list 1:1 to WGSL (the translator is the one of tools/sora_vfx_shaders.js, adapted: seven outputs, implicit-LOD
//     samples, derivatives, two shared samplers, material parameter collections);
//   - the uniform expressions of the shader map as trees with the parameter NAMES kept (the app evaluates the material
//     constant buffer itself, so that parameters can be driven at run time);
//   - every texture the shaders read, as PNG with all four channels untouched (normal maps stay two-channel: the
//     shader rebuilds Z as the game's does).
//
//   node tools/sora_material_pack.js            build build/sora_pack/materials/ and sheets/port/sora-materials.json
//   node tools/sora_material_pack.js --report   also print per shader what it reads
//   node tools/sora_material_pack.js --no-textures   skip the PNG conversion (shaders and JSON only)
//
// Output (local build output, never distributed): build/sora_pack/materials/
//   materials.json   format kh3-sora-materials/1: owners (shader id, expression trees, texture slots, collections,
//                    what was translated), materials by mesh slot name (owner, parameters, blend, two-sided)
//   materials.wgsl   fn kh_material(i: KhIn) -> KhOut and fn kh_mask(i: KhIn) per owner inside #ifdef KH_C<id>
//   textures/*.png   uv2_<mesh>_<primitive>.bin (third UV set: f32 pairs in vertex order; Bevy meshes carry two)
// Cache: build/cache/char_shaders (bytecode + disassembly; safe to delete).
'use strict';
const fs = require('fs'), path = require('path'), cp = require('child_process');
const sp = require('./sora_pack.js');
const bc = require('./lib/bcdec');
const ROOT = path.resolve(__dirname, '..');
// (IMPL_ER59, outfits) KH_OUTFIT_PACK=<dir>: the same build for an OUTFIT folder written by tools/sora_outfit_pack.js
// (its manifest.json names the slots, the mesh asset and the wear-form set); the Kingdom Key comes from the main pack;
// no camp-menu model, no sheet rewrite (the outfit tool adds its own rows). Unset: exactly the build as before.
const MAIN_PACK = path.join(ROOT, 'build/sora_pack');
const OUTFIT = !!process.env.KH_OUTFIT_PACK;
const PACK = OUTFIT ? path.resolve(process.env.KH_OUTFIT_PACK) : MAIN_PACK;
const OUT = path.join(PACK, 'materials');
const CACHE = OUTFIT ? path.join(ROOT, 'build/cache/char_shaders_outfit', path.basename(PACK)) : path.join(ROOT, 'build/cache/char_shaders');
const CONTENT = 'KINGDOM HEARTS III/Content';
const PKG = path.join(ROOT, 'assets_converted/packages', CONTENT);
const REPORT = process.argv.includes('--report');
const NO_TEX = process.argv.includes('--no-textures');
const VERSION = '1.2.0';
const MAX_TEXTURES = 14, MAX_CB = 160, MAX_COLLECTION = 48;
// (IMPL_ER59) an outfit's shader may bind 4 textures more (crates/kh-er-render er_char.wgsl KH_TEX_EXTRA: kh_t14..kh_t17)
const MAX_TEXTURES_OUTFIT = 18;
const VF = 'TGPUSkinVertexFactoryfalse';
const BASE_PS = 'TBasePassPSFCachedPointIndirectLightingPolicy';

// ------------------------------------------------------------------------------------------------ packages
const pkgCache = new Map();
function pkg(asset) { // "/Game/..." or "/Engine/..." -> decoded package or null
  const key = asset.split('.')[0];
  if (pkgCache.has(key)) return pkgCache.get(key);
  const f = /^\/Engine\//.test(key) ? path.join(ROOT, 'assets_converted/packages/Engine/Content', key.replace(/^\/Engine\//, '') + '.uasset.json') : path.join(PKG, key.replace(/^\/Game\//, '') + '.uasset.json');
  const d = fs.existsSync(f) ? JSON.parse(fs.readFileSync(f, 'utf8')) : null;
  pkgCache.set(key, d);
  return d;
}
function importPath(d, i) { let im = d.imports[-i - 1]; const names = [im.objectName]; while (im.outerIndex) { im = d.imports[-im.outerIndex - 1]; names.unshift(im.objectName); } return names[0] + (names.length > 1 ? '.' + names.slice(1).join('.') : ''); }
const propOf = (e, n) => { const p = ((e.data && e.data.props) || []).find((p) => p.name === n); return p ? p.value : undefined; };
const structOf = (items) => Object.fromEntries(items.map((p) => [p.name, p.value]));

// The texture list of a root material, as UMaterial::RebuildExpressionTextureReferences builds it at load (stock 4.17;
// see tools/sora_vfx_shaders.js, which checks this reconstruction against 1,441 texture parameters of the effect set).
const DEFAULT_TEXTURE = '/Engine/EngineResources/DefaultTexture.DefaultTexture';
const DEFAULT_TEXTURE_CLASSES = /^MaterialExpressionTexture(Object|ObjectParameter|SampleParameter2D)$/;
function appendFunctionTextures(asset, out) {
  const d = pkg(asset); if (!d) return; const fn = d.exports.find((e) => e.class === 'MaterialFunction'); if (!fn) return;
  for (const i of propOf(fn, 'FunctionExpressions') || []) { const e = d.exports[i - 1]; if (!e || /FunctionCall$/.test(e.class)) continue; const t = propOf(e, 'Texture'); if (t && t < 0) { const p = importPath(d, t); if (!out.includes(p)) out.push(p); } else if (!t && DEFAULT_TEXTURE_CLASSES.test(e.class) && !out.includes(DEFAULT_TEXTURE)) out.push(DEFAULT_TEXTURE); }
}
function dependentFunctions(asset, list) {
  const d = pkg(asset); if (!d) return; const fn = d.exports.find((e) => e.class === 'MaterialFunction'); if (!fn) return;
  for (const i of propOf(fn, 'FunctionExpressions') || []) { const e = d.exports[i - 1]; if (!e || !/FunctionCall$/.test(e.class)) continue; const r = propOf(e, 'MaterialFunction'); if (!r || r >= 0) continue; const p = importPath(d, r).split('.')[0]; dependentFunctions(p, list); if (!list.includes(p)) list.push(p); }
}
function expressionTextures(d) {
  const mat = d.exports.find((e) => e.class === 'Material'); if (!mat) return [];
  const out = [];
  for (const i of propOf(mat, 'Expressions') || []) {
    const e = d.exports[i - 1]; if (!e) continue;
    if (/FunctionCall$/.test(e.class)) { const r = propOf(e, 'MaterialFunction'); if (!r || r >= 0) continue; const p = importPath(d, r).split('.')[0]; const list = [p]; dependentFunctions(p, list); for (const f of list) appendFunctionTextures(f, out); continue; }
    const t = propOf(e, 'Texture'); if (t && t < 0) { const p = importPath(d, t); if (!out.includes(p)) out.push(p); } else if (!t && DEFAULT_TEXTURE_CLASSES.test(e.class) && !out.includes(DEFAULT_TEXTURE)) out.push(DEFAULT_TEXTURE);
  }
  return out;
}

// A material instance: its chain, every parameter (most derived value wins), the owner of its shader map.
function gather(asset) {
  const chain = []; const scalars = {}, vectors = {}, textures = {}, from = {};
  let p = asset, root = null, owner = null, rootProps = {}; const overrides = {};
  while (p) {
    const d = pkg(p); if (!d) throw new Error('package not decoded: ' + p);
    const e = d.exports.find((x) => x.outerIndex === 0 && /^Material(InstanceConstant)?$/.test(x.class));
    if (!e) throw new Error('no material in ' + p);
    chain.push(p);
    const hasMap = e.data && e.data.native && e.data.native.shaderMaps && e.data.native.shaderMaps.length;
    if (hasMap && !owner) owner = p;
    if (e.class === 'Material') {
      root = p;
      for (const pr of e.data.props || []) if (typeof pr.value !== 'object' || pr.value === null) rootProps[pr.name] = pr.value;
      break;
    }
    const short = p.split('.').pop();
    const grab = (prop, into, val) => { const v = propOf(e, prop); for (const it of (v ? v.items : [])) { const s = structOf(it); const n = s.ParameterName; if (!(n in into)) { into[n] = val(s.ParameterValue); from[n] = short; } } };
    grab('ScalarParameterValues', scalars, (v) => v);
    grab('VectorParameterValues', vectors, (v) => [v.r, v.g, v.b, v.a]);
    grab('TextureParameterValues', textures, (v) => (v ? importPath(d, v) : null));
    const bpo = propOf(e, 'BasePropertyOverrides');
    if (bpo) { const s = structOf(bpo); for (const k of ['BlendMode', 'TwoSided', 'OpacityMaskClipValue', 'ShadingModel']) if (s['bOverride_' + k] && !(k in overrides)) overrides[k] = s[k]; }
    const par = propOf(e, 'Parent');
    p = par ? importPath(d, par) : null;
  }
  return { asset, chain, root, owner, rootProps, overrides, scalars, vectors, textures, from };
}

// ------------------------------------------------------------------------------------------------ uniform expressions
const T = (e) => (e.type || '').replace('FMaterialUniformExpression', '');
const col = (v) => [v.r, v.g, v.b, v.a];
const VT = { 1: 1, 2: 2, 4: 3, 8: 4 };
// -> a tree with the parameter names kept: {c:[4]} | {s:name,d:v} | {v:name,d:[4]} | {t:1} | {f,a:[...],...}
function tree(e) {
  const t = T(e);
  switch (t) {
    case 'Constant': return { c: col(e.value) };
    case 'ScalarParameter': return { s: e.parameterName, d: e.defaultValue };
    case 'VectorParameter': return { v: e.parameterName, d: col(e.defaultValue) };
    case 'Time': case 'RealTime': return { t: 1 };
    case 'FoldedMath': return { f: ['add', 'sub', 'mul', 'div', 'dot'][e.op] || 'bad', n: VT[e.valueType] || 4, a: [tree(e.a), tree(e.b)] };
    case 'AppendVector': return { f: 'append', n: e.numComponentsA, a: [tree(e.a), tree(e.b)] };
    case 'ComponentSwizzle': return { f: 'swizzle', i: [e.indexR, e.indexG, e.indexB, e.indexA], n: e.numElements, a: [tree(e.x)] };
    case 'Clamp': return { f: 'clamp', a: [tree(e.input), tree(e.min), tree(e.max)] };
    case 'Min': case 'Max': case 'Fmod': return { f: t.toLowerCase(), a: [tree(e.a), tree(e.b)] };
    case 'Sine': return { f: e.bIsCosine ? 'cos' : 'sin', a: [tree(e.x)] };
    case 'SquareRoot': return { f: 'sqrt', a: [tree(e.x)] };
    case 'Periodic': case 'Frac': return { f: 'frac', a: [tree(e.x)] };
    case 'Floor': case 'Ceil': case 'Abs': case 'Saturate': case 'Round': case 'Sign': case 'Truncate': return { f: t.toLowerCase(), a: [tree(e.x)] };
    default: return { f: 'bad', what: t, a: [] };
  }
}
const badIn = (n) => (n.f === 'bad' ? [n.what || 'op'] : (n.a || []).flatMap(badIn));
const namesIn = (n, out) => { if (n.s) out.scalars.add(n.s); if (n.v) out.vectors.add(n.v); if (n.t) out.time = true; for (const x of n.a || []) namesIn(x, out); return out; };

// ------------------------------------------------------------------------------------------------ the translator
// (tools/sora_vfx_shaders.js `transpile`, adapted; see the header.) Constant-buffer rows of View / Primitive /
// PrecomputedLightingBuffer are named by slot; crates/kh-app/src/char_material.wgsl gives each a value.
const VIEW_SLOTS = [0, 1, 2, 3, 8, 9, 10, 26, 28, 36, 37, 38, 39, 53, 54, 55, 58, 117, 118, 119, 122, 123, 124, 125, 126, 127, 128, 129, 130, 137, 141, 142, 151, 160, 161, 172];
// (XW, the XS form weapons) KH_XSI_WEAPONS=1 (build/sora_tmp/er/xw/tools/xw_form_weapons.js): also View[12..14] =
// ViewToTranslatedWorld rows (the view axes in the world: right, up, forward; the harnesses give them as i.v12..v14). Only
// then: every other build translates exactly as before.
if (process.env.KH_XSI_WEAPONS === "1") VIEW_SLOTS.push(12, 13, 14);
const PRIM_SLOTS = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 15, 16, 17, 18];
const PRECOMP_SLOTS = [4, 5, 8, 9, 10, 11, 12, 13, 14, 15];
const VIEW_CONST = { '126.x': 0, '161.x': 1 }; // selection highlight off; eye adaptation fixed (pre-exposure 1)
const SEMANTICS = { TEXCOORD10: 'tc10', TEXCOORD11: 'tc11', TEXCOORD0: 'tc0', TEXCOORD1: 'tc1', TEXCOORD2: 'tc2', TEXCOORD3: 'tc3', TEXCOORD7: 'tc7', TEXCOORD9: 'tc9', COLOR0: 'col0', SV_Position0: 'svpos', SV_IsFrontFace0: 'front' };
function splitOps(s) { const out = []; let depth = 0, cur = ''; for (const ch of s) { if (ch === '(' || ch === '[') depth++; if (ch === ')' || ch === ']') depth--; if (ch === ',' && depth === 0) { out.push(cur.trim()); cur = ''; } else cur += ch; } if (cur.trim()) out.push(cur.trim()); return out; }
function parseOperand(s) {
  const o = { neg: false, abs: false, raw: s };
  if (s[0] === '-') { o.neg = true; s = s.slice(1); }
  if (s[0] === '|') { o.abs = true; s = s.slice(1, -1); }
  let m;
  if ((m = /^l\((.*)\)$/.exec(s))) { o.kind = 'l'; o.imm = m[1].split(',').map((x) => x.trim()); return o; }
  if (s === 'null') { o.kind = 'null'; return o; }
  if ((m = /^cb(\d+)\[(\d+)\](?:\.(\w+))?$/.exec(s))) { o.kind = 'cb'; o.cb = +m[1]; o.idx = +m[2]; o.swz = m[3] || 'xyzw'; return o; }
  if (/^cb\d+\[/.test(s)) { o.kind = 'cbdyn'; return o; }
  if ((m = /^([rvots])(\d+)(?:\.(\w+))?$/.exec(s))) { o.kind = m[1]; o.idx = +m[2]; o.swz = m[3] || 'xyzw'; return o; }
  // (party I1) an indexable temp `x0[3]` / `x0[r0.x + 0]` (dcl_indexableTemp x0[16], 4: e.g. a dither matrix)
  if ((m = /^x(\d+)\[(?:(\d+)|r(\d+)\.([xyzw])\s*\+\s*(\d+))\](?:\.(\w+))?$/.exec(s))) { o.kind = 'x'; o.idx = +m[1]; o.at = m[2] !== undefined ? { imm: +m[2] } : { reg: +m[3], comp: m[4], add: +m[5] }; o.swz = m[6] || 'xyzw'; return o; }
  o.kind = 'unknown'; return o;
}
const fl = (s) => { let v; if (/#IND|#QNAN/i.test(s)) return OUTFIT ? '0.0' : null; /* (IMPL_ER59) a NaN immediate (an unused LOD argument) */ if (/^-?1.#INF/.test(s)) return s[0] === '-' ? '(-1.0e30)' : '1.0e30'; if (/^0x/i.test(s)) { const b = Buffer.alloc(4); b.writeUInt32LE(parseInt(s, 16) >>> 0); v = b.readFloatLE(0); if (!Number.isFinite(v) || (v !== 0 && Math.abs(v) < 1e-30)) return null; } else v = Number(s); if (!Number.isFinite(v)) return null; let t = String(v); if (!/[.e]/.test(t)) t += '.0'; return v < 0 ? `(${t})` : t; };
const CI = { x: 0, y: 1, z: 2, w: 3 };

function transpile(asm, meta) {
  // meta: { ubNames, texMap {bind: slot}, sampMap {bind: 'wrap'|'clamp'}, wanted: ['o0.x', ...] | [] (mask), nOutDecl }
  const fail = (why) => ({ ok: false, why });
  const lines = asm.split(/\r?\n/);
  const sem = {};
  for (const l of lines) {
    const m = /^\/\/ (\w+)\s+(\d+)\s+(\w+)\s+(\d+)\s+(\w+)\s+(\w+)\s*(.*?)\s*$/.exec(l);
    if (!m || m[1] === 'SV_Target') continue;
    sem[+m[4]] = { name: m[1] + m[2], used: m[7] || '' };
  }
  const body = lines.filter((l) => l.trim() && !/^\s*(\/\/|dcl_|ps_5_0)/.test(l)).map((l) => l.trim());
  let nTemps = 0; for (const l of lines) { const m = /^dcl_temps (\d+)/.exec(l); if (m) nTemps = +m[1]; }
  const xTemps = {}; for (const l of lines) { const m = /^dcl_indexableTemp x(\d+)\[(\d+)\], (\d+)/.exec(l); if (m) xTemps[+m[1]] = +m[2]; } // (party I1)
  const ubOf = (cb) => (meta.ubNames[cb] !== undefined ? meta.ubNames[cb] : '?');
  const ins = [];
  for (const l of body) {
    const spc = l.indexOf(' ');
    let op = spc < 0 ? l : l.slice(0, spc), rest = spc < 0 ? '' : l.slice(spc + 1);
    let texType = null;
    const mm = /^(\w+)\((\w+)\)\(.*?\)$/.exec(op); if (mm) { op = mm[1]; texType = mm[2]; }
    let sat = false; if (/_sat$/.test(op)) { sat = true; op = op.replace(/_sat$/, ''); }
    ins.push({ op, sat, texType, a: splitOps(rest).map(parseOperand), line: l });
  }
  // ---- static folding of the view constants that gate whole blocks, and the SvPosition -> translated-world idiom
  const svReg = Object.keys(sem).find((r) => sem[r].name === 'SV_Position0');
  const out = [];
  const constVal = (o) => { if (o.kind === 'l' && o.imm.length >= 1) { const v = Number(o.imm[0]); return Number.isFinite(v) && o.imm.every((x) => Number(x) === v) ? v : null; } if (o.kind === 'cb' && ubOf(o.cb) === 'View') { const k = o.idx + '.' + o.swz[0]; if (o.swz.split('').every((c) => c === o.swz[0]) && k in VIEW_CONST) return VIEW_CONST[k]; } return null; };
  let known = null;
  for (let i = 0; i < ins.length; i++) {
    const n = ins[i];
    if ((n.op === 'if_nz' || n.op === 'if_z') && known && n.a[0].kind === 'r' && n.a[0].idx === known.reg && n.a[0].swz === known.comp) {
      const taken = (n.op === 'if_nz') === (known.value !== 0);
      let depth = 0, j = i + 1, elseAt = -1, endAt = -1;
      for (; j < ins.length; j++) { const o = ins[j].op; if (o === 'if_nz' || o === 'if_z') depth++; else if (o === 'endif') { if (depth === 0) { endAt = j; break; } depth--; } else if (o === 'else' && depth === 0) elseAt = j; }
      if (endAt < 0) return fail('unbalanced if');
      const from = taken ? i + 1 : (elseAt < 0 ? endAt : elseAt + 1), to = taken ? (elseAt < 0 ? endAt : elseAt) : endAt;
      const keep = ins.slice(from, to);
      ins.splice(i, endAt - i + 1, ...keep);
      // (the same register may gate several blocks: the comparison stays known until it is overwritten)
      i--; continue;
    }
    if (known && n.a[0] && n.a[0].kind === 'r' && n.a[0].idx === known.reg && n.a[0].swz.includes(known.comp)) known = null;
    if ((n.op === 'lt' || n.op === 'ne' || n.op === 'ge' || n.op === 'eq') && n.a[0].kind === 'r' && n.a[0].swz.length === 1) {
      const x = constVal(n.a[1]), y = constVal(n.a[2]);
      if (x !== null && y !== null) known = { reg: n.a[0].idx, comp: n.a[0].swz, value: ({ lt: x < y, ne: x !== y, ge: x >= y, eq: x === y })[n.op] ? 1 : 0, at: i };
    }
    if (n.op === 'mul' && n.a.length === 3 && n.a[2].kind === 'cb' && ubOf(n.a[2].cb) === 'View' && n.a[2].idx === 37 && n.a[1].kind === 'v' && String(n.a[1].idx) === String(svReg) && n.a[0].swz === 'xyzw') {
      const nx = ins.slice(i + 1, i + 4);
      const okIdiom = nx.length === 3 && nx[0].op === 'mad' && nx[0].a[2].idx === 36 && nx[1].op === 'mad' && nx[1].a[2].idx === 38 && nx[2].op === 'add' && nx[2].a[2].idx === 39 && nx.every((x) => x.a[0].kind === 'r' && x.a[0].idx === n.a[0].idx);
      if (!okIdiom) return fail('SVPositionToTranslatedWorld used outside the usual idiom');
      out.push({ kind: 'raw', text: `r${n.a[0].idx} = vec4<f32>(i.twp, 1.0);`, dest: { r: 'r' + n.a[0].idx, mask: 'xyzw' }, src: [] });
      i += 3; continue;
    }
    out.push({ kind: 'ins', i, n });
  }
  // ---- operands -> WGSL
  let why = null; const bad = (w) => { if (!why) why = w; return '0.0'; };
  let usesClock = false; const rowsUsed = new Set(); const texUsed = new Set(), viewUsed = new Set(), primUsed = new Set(), preUsed = new Set(), semUsed = new Set(), collUsed = {}; let maxCb = -1;
  const base = (o) => {
    switch (o.kind) {
      case 'r': return 'r' + o.idx;
      case 'o': return 'o' + o.idx;
      case 'x': { if (xTemps[o.idx] === undefined) return bad('undeclared indexable temp x' + o.idx); // (party I1) D3D reads 0 out of range; clamped here
        const n = xTemps[o.idx]; return o.at.imm !== undefined ? `x${o.idx}[${o.at.imm}]` : `x${o.idx}[clamp(i32(r${o.at.reg}.${o.at.comp}) + ${o.at.add}, 0, ${n - 1})]`; }
      case 'v': {
        const s = sem[o.idx];
        // (party A2) an owner whose vertex shader writes the PRE-SKINNED LOCAL POSITION (GPU-skin MeshOrigin /
        // MeshExtension decompression, cb0[6] / cb0[7]) into TEXCOORD3.xy and TEXCOORD4.x (customized UVs; w_do070,
        // w_do130, w_gu130): both read from i.tc3.xyz, which the harness fills from the vertex colour under KH_PSP
        if (meta.psp && s && s.name === 'TEXCOORD3') { semUsed.add(s.name); return 'i.tc3'; }
        if (meta.psp && s && s.name === 'TEXCOORD4') { semUsed.add(s.name); return 'vec4<f32>(i.tc3.z, 0.0, 0.0, 0.0)'; }
        const f = s && SEMANTICS[s.name]; if (!f) return bad('input ' + (s ? s.name : 'v' + o.idx)); semUsed.add(s.name); return 'i.' + f; }
      case 'cb': { const ub = ubOf(o.cb); let m;
        if (ub === 'Material') { if (o.idx >= (meta.remap ? 4096 : MAX_CB)) return bad('material constant buffer too large (' + o.idx + ')'); maxCb = Math.max(maxCb, o.idx); if (meta.remap) { rowsUsed.add(o.idx); return `kh.m[@@R${o.idx}@@]`; } return `kh.m[${o.idx}]`; }
        if ((m = /^MaterialCollection(\d)$/.exec(ub))) { // (a character material reads one collection, KHS_Control = kh.c0; a post-process material names its own arrays)
          const arr = meta.collMap ? meta.collMap[m[1]] : (+m[1] === 0 ? 'c0' : null);
          if (!arr || o.idx >= MAX_COLLECTION) return bad('collection ' + ub + '[' + o.idx + ']'); collUsed[arr] = Math.max(collUsed[arr] || 0, o.idx + 1); return `kh.${arr}[${o.idx}]`; }
        if (ub === 'View') { if (!VIEW_SLOTS.includes(o.idx)) return bad('View[' + o.idx + ']'); viewUsed.add(o.idx); return 'i.v' + o.idx; }
        if (ub === 'Primitive') { if (!PRIM_SLOTS.includes(o.idx)) return bad('Primitive[' + o.idx + ']'); primUsed.add(o.idx); return 'i.p' + o.idx; }
        if (ub === 'PrecomputedLightingBuffer') { if (!PRECOMP_SLOTS.includes(o.idx)) return bad('precomputed lighting[' + o.idx + ']'); preUsed.add(o.idx); return 'i.l' + o.idx; }
        return bad('uniform buffer ' + ub + '[' + o.idx + ']'); }
      default: return bad('operand ' + o.raw);
    }
  };
  const wrap = (o, e) => { if (o.abs) e = `abs(${e})`; if (o.neg) e = `(-${e})`; return e; };
  const vec = (o, comps) => {
    if (o.kind === 'l') { const v = comps.map((k) => fl(o.imm.length === 1 ? o.imm[0] : o.imm[k])); if (v.includes(null)) return bad('integer immediate'); return wrap(o, v.length === 1 ? v[0] : `vec${v.length}<f32>(${v.join(', ')})`); }
    if (o.kind === 'cb' && ubOf(o.cb) === 'View' && (o.idx === 128 || (o.idx === 161 && /z/.test(comps.map((k) => (o.swz.length === 1 ? o.swz[0] : o.swz[k])).join(''))))) usesClock = true;
    if (!o.swz) return bad('operand ' + o.raw); // (party D4) e.g. an indexable temp x0[r0.x + 0] (a dither matrix): fail cleanly, not crash
    const b = base(o); const s = comps.map((k) => (o.swz.length === 1 ? o.swz[0] : o.swz[k] || o.swz[o.swz.length - 1])).join('');
    return wrap(o, `${b}.${s}`);
  };
  const first = (o, n) => vec(o, [0, 1, 2, 3].slice(0, n));
  const destComps = (o) => o.swz.split('').map((c) => CI[c]);
  const assign = (o, expr, n, sat) => {
    const d = o.kind === 'o' ? 'o' + o.idx : o.kind === 'x' ? base(o) : 'r' + o.idx; const comps = o.swz.split(''); // (party I1) x: an indexable temp
    if (o.kind !== 'r' && o.kind !== 'o' && o.kind !== 'x') return bad('destination ' + o.raw);
    if (sat) expr = `saturate(${expr})`;
    if (comps.length === 1) return `${d}.${comps[0]} = ${expr}${n > 1 ? '.x' : ''};`;
    if (n === 1) return `{ let t = ${expr}; ${comps.map((c) => `${d}.${c} = t;`).join(' ')} }`;
    if (comps.length === 4 && o.swz === 'xyzw') return `${d} = ${expr};`;
    return `{ let t = ${expr}; ${comps.map((c, k) => `${d}.${c} = t.${'xyzw'[k]};`).join(' ')} }`;
  };
  const zero = (n) => (n === 1 ? '0.0' : `vec${n}<f32>(0.0)`), one = (n) => (n === 1 ? '1.0' : `vec${n}<f32>(1.0)`);
  const UN = { mov: (a) => a, sqrt: (a) => `sqrt(${a})`, rsq: (a) => `inverseSqrt(${a})`, rcp: (a) => `(1.0 / ${a})`, log: (a) => `log2(${a})`, exp: (a) => `exp2(${a})`, frc: (a) => `fract(${a})`, round_ni: (a) => `floor(${a})`, round_pi: (a) => `ceil(${a})`, round_z: (a) => `trunc(${a})`, ftoi: (a) => `trunc(${a})`, ftou: (a) => `trunc(${a})`, utof: (a) => a, round_ne: (a) => `round(${a})`,
    deriv_rtx: (a) => `dpdx(${a})`, deriv_rty: (a) => `dpdy(${a})`, deriv_rtx_coarse: (a) => `dpdxCoarse(${a})`, deriv_rty_coarse: (a) => `dpdyCoarse(${a})`, deriv_rtx_fine: (a) => `dpdxFine(${a})`, deriv_rty_fine: (a) => `dpdyFine(${a})` };
  const BIN = { iadd: (a, b) => `(${a} + ${b})`, imul: (a, b) => `(${a} * ${b})`, add: (a, b) => `(${a} + ${b})`, mul: (a, b) => `(${a} * ${b})`, div: (a, b) => `(${a} / ${b})`, min: (a, b) => `min(${a}, ${b})`, max: (a, b) => `max(${a}, ${b})`, and: (a, b) => `(${a} * ${b})`, or: (a, b) => `max(${a}, ${b})` };
  const CMP = { lt: '<', ge: '>=', eq: '==', ne: '!=', ilt: '<', ige: '>=', ieq: '==', ine: '!=', ult: '<', uge: '>=' };
  const masks = new Set();
  const texel = (tOp, sOp, uv, how) => {
    const t = meta.texMap[tOp.idx];
    if (t === undefined) return null;
    if (t >= (meta.remap ? 64 : MAX_TEXTURES)) { bad('more than ' + MAX_TEXTURES + ' textures'); return 'vec4<f32>(0.0)'; }
    const s = sOp && meta.sampMap[sOp.idx] === 'clamp' ? 'kh_sclamp' : 'kh_swrap';
    texUsed.add(t);
    return how(meta.texNames ? meta.texNames[t] : meta.remap ? `kh_t@@T${t}@@` : `kh_t${t}`, s, uv);
  };
  let loopDepth = 0;
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
      case 'loop': loopDepth++; return 'loop {';
      case 'endloop': loopDepth--; return '}';
      case 'breakc_nz': return `if (${vec(a[0], [0])} != 0.0) { break; }`;
      case 'breakc_z': return `if (${vec(a[0], [0])} == 0.0) { break; }`;
    }
    // (ER87) `udiv q, r, a, b`: the unsigned quotient AND REMAINDER (integers held as floats); either destination may be null.
    // KH3's hair masks take the remainder of a screen-position sum by 5 (their DitherTemporalAA phase): it used to be dropped
    // (the liveness walk saw no write through a null first destination), which turned the dither into a hard cut.
    if (n.op === 'udiv') {
      const dsts = [[a[0], false], [a[1], true]].filter(([o]) => o && o.kind !== 'null');
      if (!dsts.length) return '';
      if (dsts.some(([o]) => o.kind !== 'r' && o.kind !== 'o')) return bad('udiv destination');
      const cs = destComps(dsts[0][0]); if (dsts.some(([o]) => destComps(o).join() !== cs.join())) return bad('udiv with differently masked destinations');
      for (const [o] of dsts) if (o.kind === 'r') for (const c of o.swz.split('')) masks.delete('r' + o.idx + '.' + c);
      const k1 = cs.length, x = vec(a[2], cs), y = vec(a[3], cs);
      return `{ let ua = ${x}; let ub = ${y}; ${dsts.map(([o, rem]) => assign(o, rem ? `(ua - ub * floor(ua / ub))` : `floor(ua / ub)`, k1, false)).join(' ')} }`;
    }
    if (!d || (d.kind !== 'r' && d.kind !== 'o' && d.kind !== 'x')) { if (n.op !== 'sincos') return bad('instruction ' + n.op); }
    const comps = d && d.swz ? destComps(d) : [], k = comps.length;
    const srcMask = (o) => (o.kind === 'r' ? comps.map((c) => masks.has('r' + o.idx + '.' + (o.swz.length === 1 ? o.swz[0] : o.swz[c] || o.swz[o.swz.length - 1]))) : comps.map(() => false));
    const dkeys = d.kind === 'r' ? d.swz.split('').map((c) => 'r' + d.idx + '.' + c) : [];
    const isMaskOp = !!CMP[n.op] || ((n.op === 'and' || n.op === 'or') && srcMask(a[1]).every(Boolean) && srcMask(a[2]).every(Boolean));
    // `and` of a comparison with a float constant (0x3f800000 = 1.0): select the constant
    if (n.op === 'and' && (a[1].kind === 'l' || a[2].kind === 'l')) { const [m, c] = a[1].kind === 'l' ? [a[2], a[1]] : [a[1], a[2]]; if (srcMask(m).every(Boolean)) { for (const key of dkeys) masks.delete(key); return assign(d, `(${vec(m, comps)} * ${vec(c, comps)})`, k, false); }
      // a bit test of an integer held as a float (a ftou / ftoi / iadd result): the immediate is printed without a decimal point
      const ints = comps.map((q) => (c.imm.length === 1 ? c.imm[0] : c.imm[q]));
      if (ints.every((x) => /^\d+$/.test(x))) { for (const key of dkeys) masks.delete(key); const u = k === 1 ? `u32(${vec(m, comps)})` : `vec${k}<u32>(${vec(m, comps)})`; const bits = k === 1 ? `${ints[0]}u` : `vec${k}<u32>(${ints.map((x) => x + 'u').join(', ')})`; return assign(d, k === 1 ? `f32(${u} & ${bits})` : `vec${k}<f32>(${u} & ${bits})`, k, false); }
      return bad('and with an immediate'); }
    if (n.op === 'itof') { const m = srcMask(a[1]); for (const key of dkeys) masks.delete(key); if (m.every(Boolean)) return assign(d, `(-${vec(a[1], comps)})`, k, false); if (m.some(Boolean)) return bad('itof of a mixed operand'); return assign(d, vec(a[1], comps), k, false); }
    if (n.op === 'utof' && srcMask(a[1]).some(Boolean)) return bad('utof of a comparison');
    if (n.op === 'ishl') { const sh = a[2].kind === 'l' ? Number(a[2].imm[0]) : NaN; for (const key of dkeys) masks.delete(key); if (!Number.isInteger(sh) || sh < 0 || sh > 16) return bad('ishl by a register'); return assign(d, `(${vec(a[1], comps)} * ${1 << sh}.0)`, k, false); }
    for (const key of dkeys) { if (isMaskOp) masks.add(key); else masks.delete(key); }
    // (party I1) bfi width, offset, insert, base = insertBits(base, insert, offset, width) per component (integers held as floats)
    if (n.op === 'bfi') { const imm = (o, q) => (o.kind === 'l' ? Number(o.imm.length === 1 ? o.imm[0] : o.imm[q]) : NaN); const parts = comps.map((q) => { const w = imm(a[1], q), off = imm(a[2], q); if (!Number.isInteger(w) || !Number.isInteger(off)) return null; return `f32(insertBits(u32(${vec(a[4], [q])}), u32(${vec(a[3], [q])}), ${off}u, ${w}u))`; }); if (parts.includes(null)) return bad('bfi with a register width / offset'); return assign(d, k === 1 ? parts[0] : `vec${k}<f32>(${parts.join(', ')})`, k, false); }
    if (UN[n.op]) return assign(d, UN[n.op](vec(a[1], comps)), k, n.sat);
    // (D3D drops a NaN in min / max / saturate; where a shader divides 0 by 0 on purpose the quotient is made 0 instead)
    if (n.op === 'div' && meta.safeDiv) return assign(d, `kh_div${k}(${vec(a[1], comps)}, ${vec(a[2], comps)})`, k, n.sat);
    if (BIN[n.op]) return assign(d, BIN[n.op](vec(a[1], comps), vec(a[2], comps)), k, n.sat);
    if (n.op === 'mad') return assign(d, `(${vec(a[1], comps)} * ${vec(a[2], comps)} + ${vec(a[3], comps)})`, k, n.sat);
    if (CMP[n.op]) return assign(d, `select(${zero(k)}, ${one(k)}, ${vec(a[1], comps)} ${CMP[n.op]} ${vec(a[2], comps)})`, k, false);
    if (n.op === 'movc') return assign(d, `select(${vec(a[3], comps)}, ${vec(a[2], comps)}, ${vec(a[1], comps)} != ${zero(k)})`, k, n.sat);
    if (/^dp[234]$/.test(n.op)) { const m = +n.op[2]; return assign(d, `dot(${first(a[1], m)}, ${first(a[2], m)})`, 1, n.sat); }
    if (n.op === 'sincos') { const parts = []; if (a[0].kind !== 'null') parts.push(assign(a[0], `sin(${vec(a[2], destComps(a[0]))})`, destComps(a[0]).length, n.sat)); if (a[1].kind !== 'null') parts.push(assign(a[1], `cos(${vec(a[2], destComps(a[1]))})`, destComps(a[1]).length, n.sat)); return parts.join(' '); }
    if (/^sample/.test(n.op)) {
      if (n.texType && n.texType !== 'texture2d') return bad('texture type ' + n.texType);
      const kind = n.op.replace(/_indexable$/, '');
      const how = kind === 'sample_l' ? (t, s, uv) => `textureSampleLevel(${t}, ${s}, ${uv}, ${vec(a[4], [0])})`
        : kind === 'sample_b' ? (t, s, uv) => `textureSampleBias(${t}, ${s}, ${uv}, ${vec(a[4], [0])})`
        : kind === 'sample_d' ? (t, s, uv) => `textureSampleGrad(${t}, ${s}, ${uv}, ${first(a[4], 2)}, ${first(a[5], 2)})`
        : kind === 'sample' ? (t, s, uv) => `textureSample(${t}, ${s}, ${uv})` : null;
      if (!how) return bad('instruction ' + n.op);
      // a scene texture of a post-process material: the value of this very pixel, handed in by the harness
      const scene = meta.sceneTex && meta.sceneTex[a[2].idx];
      const s = scene || texel(a[2], a[3], first(a[1], 2), meta.lod0 ? (t, sm, uv) => `textureSampleLevel(${t}, ${sm}, ${uv}, 0.0)` : how);
      if (s === null) return bad('scene texture read (t' + a[2].idx + ')');
      const sw = comps.map((c) => a[2].swz[c]).join('');
      return assign(d, `${s}.${sw}`, k, n.sat);
    }
    if (n.op === 'ld_indexable' || n.op === 'ld') { if (meta.texMap[a[2].idx] !== undefined) return bad('texel load of a material texture'); return assign(d, one(k), k, false); }
    return bad('instruction ' + n.op);
  };
  // ---- liveness per register component, backwards through the structured code
  const wanted = meta.wanted;
  const key = (o, c) => o.kind + o.idx + '.' + c;
  const isControl = (e) => e.kind === 'ins' && /^(if_|else|endif|ret|discard|loop|endloop|breakc_)/.test(e.n.op);
  const writes = (e) => {
    if (e.kind === 'raw') return ['x', 'y', 'z', 'w'].map((c) => e.dest.r + '.' + c);
    const n = e.n; if (isControl(e)) return [];
    const ds = n.op === 'sincos' || n.op === 'udiv' ? [n.a[0], n.a[1]] : [n.a[0]];
    return ds.filter((o) => o && (o.kind === 'r' || o.kind === 'o' || o.kind === 'x')).flatMap((o) => o.swz.split('').map((c) => key(o, c)));
  };
  const reads = (e) => {
    if (e.kind === 'raw') return [];
    const n = e.n; const res = [];
    // (party I1) an indexable temp (key x<n>.<c> for every element) also reads the register of a dynamic index
    const add = (o, comps) => { if (o && o.kind === 'x' && o.at && o.at.reg !== undefined) res.push('r' + o.at.reg + '.' + o.at.comp); if (o && (o.kind === 'r' || o.kind === 'o' || o.kind === 'x')) for (const k of comps) res.push(key(o, o.swz.length === 1 ? o.swz[0] : o.swz[k] || o.swz[o.swz.length - 1])); };
    if (n.a[0] && n.a[0].kind === 'x' && n.a[0].at && n.a[0].at.reg !== undefined) res.push('r' + n.a[0].at.reg + '.' + n.a[0].at.comp);
    if (/^(if_|discard|breakc_)/.test(n.op)) { add(n.a[0], [0]); return res; }
    if (isControl(e)) return res;
    if (/^dp[234]$/.test(n.op)) { const m = [0, 1, 2, 3].slice(0, +n.op[2]); add(n.a[1], m); add(n.a[2], m); return res; }
    if (/^sample/.test(n.op)) { add(n.a[1], [0, 1]); if (n.a[4]) add(n.a[4], [0, 1]); if (n.a[5]) add(n.a[5], [0, 1]); return res; }
    if (/^ld/.test(n.op)) return res;
    if (n.op === 'sincos') { for (const d of [n.a[0], n.a[1]]) if (d.kind !== 'null') add(n.a[2], destComps(d)); return res; }
    if (n.op === 'udiv') { for (const d of [n.a[0], n.a[1]]) if (d && d.kind !== 'null') { add(n.a[2], destComps(d)); add(n.a[3], destComps(d)); } return res; } // (ER87)
    const d = n.a[0]; const comps = d && d.swz ? destComps(d) : [0];
    for (const o of n.a.slice(1)) add(o, comps);
    return res;
  };
  const needed = new Array(out.length).fill(false);
  const matchEnd = (at) => { let depth = 0, elseAt = -1; for (let j = at + 1; j < out.length; j++) { const o = out[j].kind === 'ins' ? out[j].n.op : ''; if (o === 'if_nz' || o === 'if_z') depth++; else if (o === 'endif') { if (depth === 0) return { elseAt, endAt: j }; depth--; } else if (o === 'else' && depth === 0) elseAt = j; } return null; };
  const blocks = new Map(); for (let x = 0; x < out.length; x++) { const o = out[x].kind === 'ins' ? out[x].n.op : ''; if (o === 'if_nz' || o === 'if_z') { const m = matchEnd(x); if (!m) return fail('unbalanced if'); blocks.set(x, m); } }
  const ifOfEnd = new Map(); for (const [x, m] of blocks) ifOfEnd.set(m.endAt, x);
  const loopOfEnd = new Map(); { const stack = []; for (let x = 0; x < out.length; x++) { const o = out[x].kind === 'ins' ? out[x].n.op : ''; if (o === 'loop') stack.push(x); else if (o === 'endloop') { if (!stack.length) return fail('unbalanced loop'); loopOfEnd.set(x, stack.pop()); } } if (stack.length) return fail('unbalanced loop'); }
  const exits = [];
  // An `if` whose blocks hold nothing that is needed (and no discard) is dropped with its condition.
  const hasDiscard = (from, to) => { for (let x = from; x < to; x++) if (out[x].kind === 'ins' && /^discard/.test(out[x].n.op)) return true; return false; };
  const walk = (from, to, liveOut) => {
    let live = new Set(liveOut);
    for (let x = to - 1; x >= from; x--) {
      const e = out[x]; const op = e.kind === 'ins' ? e.n.op : '';
      if (op === 'endif') {
        const ifAt = ifOfEnd.get(x), m = blocks.get(ifAt);
        const before = needed.slice(ifAt, x + 1);
        const thenLive = walk(ifAt + 1, m.elseAt < 0 ? m.endAt : m.elseAt, live);
        const elseLive = m.elseAt < 0 ? live : walk(m.elseAt + 1, m.endAt, live);
        let any = false; for (let y = ifAt + 1; y < x; y++) if (needed[y] && y !== m.elseAt) any = true;
        if (!any && !hasDiscard(ifAt, x)) { for (let y = ifAt; y <= x; y++) needed[y] = before[y - ifAt]; x = ifAt; continue; }
        live = new Set([...thenLive, ...elseLive]); for (const r of reads(out[ifAt])) live.add(r);
        needed[x] = needed[ifAt] = true; if (m.elseAt >= 0) needed[m.elseAt] = true;
        x = ifAt; continue;
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
      if (isControl(e)) { needed[x] = true; for (const r of reads(e)) live.add(r); continue; }
      const w = writes(e);
      if (w.some((r) => live.has(r))) { needed[x] = true; for (const r of w) if (!/^x/.test(r)) live.delete(r); for (const r of reads(e)) live.add(r); } // (party I1) one element's store leaves the others live
    }
    return live;
  };
  walk(0, out.length, new Set(wanted));
  const liveRegs = new Set(); for (let x = 0; x < out.length; x++) if (needed[x]) for (const r of [...writes(out[x]), ...reads(out[x])]) liveRegs.add(r.split('.')[0]);
  let usesTwp = false;
  const code = [];
  for (let x = 0; x < out.length; x++) { if (!needed[x]) continue; const e = out[x]; const text = e.kind === 'raw' ? ((usesTwp = true), e.text) : gen(e.n); if (text) code.push(text); if (why) return fail(why + '  (' + (e.n ? e.n.line : e.text) + ')'); }
  const lastRet = ins.length && ins[ins.length - 1].op === 'ret';
  if (ins.filter((n) => n.op === 'ret').length !== (lastRet ? 1 : 0)) return fail('early return');
  const decl = []; for (let r = 0; r < nTemps; r++) if (liveRegs.has('r' + r)) decl.push(`var r${r}: vec4<f32>;`);
  for (const [xi, len] of Object.entries(xTemps)) if (liveRegs.has('x' + xi)) decl.push(`var x${xi}: array<vec4<f32>, ${len}>;`); // (party I1)
  const info = { rows: [...rowsUsed].sort((x, y) => x - y), uses_clock: usesClock, textures: [...texUsed].sort((x, y) => x - y), cb_len: maxCb + 1, collections: collUsed, inputs: [...semUsed].sort(), view: [...viewUsed].sort((x, y) => x - y), primitive: [...primUsed].sort((x, y) => x - y), precomputed: [...preUsed].sort((x, y) => x - y), instructions: code.length };
  if (!wanted.length) return { ok: true, ...info, wgsl: [...decl, ...code].map((l) => '    ' + l).join('\n') };
  const outs = []; for (let k = 0; k < 7; k++) outs.push(`var o${k}: vec4<f32> = vec4<f32>(0.0);`);
  const wgsl = [...decl, ...outs, ...code, meta.ret || 'return KhOut(o0, o1, o2, o3, o4, o5, o6);'].map((l) => '    ' + l).join('\n');
  return { ok: true, ...info, wgsl };
}

// Resource tables of a shader: bind slot -> the Material buffer's resource index (docs/formats/material.md).
function resourceTables(code, n2d, nCube, names, addressOf) {
  const mat = names.indexOf('Material');
  const dec = (arr) => { const o = {}; if (!arr || mat < 0 || mat >= arr.length) return o; const off = arr[mat]; if (!off || off >= arr.length) return o; for (let k = off; k < arr.length && arr[k] !== 0xFFFFFFFF; k++) { const e = arr[k]; if ((e >>> 24) !== mat) break; o[e & 0xFF] = (e >>> 8) & 0xFFFF; } return o; };
  const rt = code.resourceTable || {}; const tex = dec(rt.textureMap), samp = dec(rt.samplerMap);
  const texMap = {}, sampMap = {};
  for (const [bind, res] of Object.entries(tex)) texMap[bind] = res < 2 * n2d && res % 2 === 0 ? res >> 1 : 1000 + res;
  // a texture's own sampler: its address mode; the two shared samplers: wrap, clamp
  for (const [bind, res] of Object.entries(samp)) { if (res < 2 * n2d) sampMap[bind] = addressOf(res >> 1); else { const k = res - 2 * (n2d + nCube); sampMap[bind] = k === 1 ? 'clamp' : 'wrap'; } }
  return { texMap, sampMap };
}

// ------------------------------------------------------------------------------------------------ textures
const texCache = new Map(); // asset -> record
function textureRecord(asset) {
  if (texCache.has(asset)) return texCache.get(asset);
  const isEngine = /^\/Engine\//.test(asset);
  const content = asset.replace(/^\/(Game|Engine)\//, '').split('.')[0];
  const dir = path.join(ROOT, 'assets_converted/textures', isEngine ? 'Engine/Content' : CONTENT);
  const dds = path.join(dir, content + '.dds'), side = path.join(dir, content + '.texture.json');
  let rec;
  if (!fs.existsSync(dds) || !fs.existsSync(side)) rec = { asset, missing: true };
  else {
    const s = JSON.parse(fs.readFileSync(side, 'utf8')); const pr = Object.fromEntries(s.props.map((x) => [x.tag.name, x.value]));
    const srgb = pr.SRGB === undefined ? true : !!pr.SRGB;
    rec = { asset, file: 'textures/' + path.basename(content) + '.png', dds, srgb, compression: pr.CompressionSettings || 'TC_Default', address_x: pr.AddressX || 'TA_Wrap', address_y: pr.AddressY || 'TA_Wrap', lod_group: pr.LODGroup || null };
  }
  texCache.set(asset, rec);
  return rec;
}
function writeTextures() {
  let wrote = 0, kept = 0;
  for (const rec of texCache.values()) {
    if (rec.missing) continue;
    const out = path.join(OUT, rec.file);
    const t = bc.decodeDds(fs.readFileSync(rec.dds), 0);
    rec.width = t.width; rec.height = t.height; rec.pixel_format = t.format; rec.mips = t.mipCount;
    if (NO_TEX || (fs.existsSync(out) && fs.statSync(out).mtimeMs > fs.statSync(rec.dds).mtimeMs)) { kept++; continue; }
    fs.writeFileSync(out, bc.encodePng(t.width, t.height, Buffer.from(t.rgba), { alpha: true, srgb: rec.srgb }));
    wrote++;
  }
  return { wrote, kept };
}

// ------------------------------------------------------------------------------------------------ collections
let collectionIndex = null;
function findCollection(guid) {
  if (!collectionIndex) {
    collectionIndex = new Map();
    const walk = (dir) => { for (const f of fs.readdirSync(dir, { withFileTypes: true })) { const p = path.join(dir, f.name); if (f.isDirectory()) walk(p); else if (/\.uasset\.json$/.test(f.name) && fs.statSync(p).size < 200000) { const text = fs.readFileSync(p, 'utf8'); if (!text.includes('"MaterialParameterCollection"')) continue; const d = JSON.parse(text); const e = d.exports.find((x) => x.class === 'MaterialParameterCollection'); if (e) collectionIndex.set(propOf(e, 'StateId'), { asset: '/Game/' + path.relative(PKG, p).replace(/\\/g, '/').replace(/\.uasset\.json$/, ''), e }); } } };
    walk(path.join(PKG, 'Shader'));
  }
  const c = collectionIndex.get(guid); if (!c) return null;
  // stock 4.17 layout: scalars packed four to a row, then one row per vector
  const sc = (propOf(c.e, 'ScalarParameters') || { items: [] }).items.map(structOf), ve = (propOf(c.e, 'VectorParameters') || { items: [] }).items.map(structOf);
  const rows = []; const params = {};
  sc.forEach((s, i) => { if (i % 4 === 0) rows.push([0, 0, 0, 0]); rows[i >> 2][i & 3] = s.DefaultValue; params[s.ParameterName] = { row: i >> 2, comp: i & 3 }; });
  ve.forEach((v) => { params[v.ParameterName] = { row: rows.length, comp: -1 }; const d = v.DefaultValue; rows.push([d.r, d.g, d.b, d.a]); });
  return { asset: c.asset, rows, params };
}

// ------------------------------------------------------------------------------------------------ the camp menu's model
// The game renders each party member for the camp menu with a "UI actor" blueprint: a skeletal mesh component with a
// camp anim blueprint, a scene capture into a 512 x 800 render target and two spot lights (docs/sora-ui-spec.md 10.9).
// Everything below is read from /Game/UI/Blueprints/CampTop/p_ex001_UIActor and what it references.
const UI_ACTOR = '/Game/UI/Blueprints/CampTop/p_ex001_UIActor';
function menuPack() {
  const d = pkg(UI_ACTOR); if (!d) return null;
  const props = (e) => Object.fromEntries((e.data.props || []).map((x) => [x.name, x.value]));
  const cap = props(d.exports.find((e) => e.class === 'SceneCaptureComponent2D'));
  const mesh = props(d.exports.find((e) => e.class === 'TresSkeletalMeshComponent'));
  const pp = Object.fromEntries((cap.PostProcessSettings || []).map((x) => [x.name, x.value]));
  const target = pkg(importPath(d, cap.TextureTarget)); const tp = props(target.exports.find((e) => e.class === 'TextureRenderTarget2D'));
  // stock 4.17 class defaults for what a template does not serialize
  const lights = d.exports.filter((e) => e.class === 'SpotLightComponent').map((e) => { const l = props(e); return { name: e.objectName.replace(/_GEN_VARIABLE$/, ''), location_cm: l.RelativeLocation, rotation_deg: l.RelativeRotation, intensity: l.Intensity ?? 5000, inner_cone_deg: l.InnerConeAngle ?? 0, outer_cone_deg: l.OuterConeAngle ?? 44, attenuation_radius_cm: l.AttenuationRadius ?? 1000, min_roughness: l.MinRoughness ?? 0.08, color: l.LightColor ? [l.LightColor.r / 255, l.LightColor.g / 255, l.LightColor.b / 255] : [1, 1, 1], defaults_note: 'Intensity 5000, OuterConeAngle 44, InnerConeAngle 0, AttenuationRadius 1000, MinRoughness 0.08, white, inverse-squared falloff: stock 4.17 class defaults of members the template does not serialize' }; });
  // the ambient cube map: mean colour of each face's top mip (A16B16G16R16F, six faces, each with its mip chain)
  const cube = importPath(d, pp.AmbientCubemap); let faces = null;
  { const f = path.join(ROOT, 'assets_converted/textures', CONTENT, cube.replace(/^\/Game\//, '').split('.')[0] + '.dds');
    if (fs.existsSync(f)) { const b = fs.readFileSync(f); const size = b.readUInt32LE(12); const fourcc = b.readUInt32LE(84);
      if (fourcc === 113) { const half = (h) => { const sg = (h & 0x8000) ? -1 : 1, e = (h >> 10) & 31, m = h & 1023; return e === 0 ? sg * Math.pow(2, -14) * (m / 1024) : sg * Math.pow(2, e - 15) * (1 + m / 1024); };
        let texels = 0; for (let w = size; w >= 1; w >>= 1) texels += w * w; faces = [];
        for (let k = 0; k < 6; k++) { const o = 128 + k * texels * 8; const sum = [0, 0, 0]; for (let i = 0; i < size * size; i++) for (let c = 0; c < 3; c++) sum[c] += half(b.readUInt16LE(o + i * 8 + c * 2)); faces.push(sum.map((x) => +(x / (size * size)).toFixed(4))); } } } }
  // the camp anim set of the anim blueprint
  const animClass = importPath(d, mesh.AnimClass).split('.')[0];
  const setPkg = pkg('/Game/Character/pc/p_ex001/anm/p_ex001_CM_Body_AnimSet'); const clips = [];
  const meshGlb = sp.readGlb(path.join(ROOT, 'assets_converted/meshes', CONTENT, 'Character/pc/p_ex001/mdl/p_ex001_0.glb')); const bones = sp.buildSkeleton(meshGlb.json);
  fs.mkdirSync(path.join(PACK, 'menu'), { recursive: true });
  if (setPkg) for (const it of propOf(setPkg.exports.find((e) => e.class === 'TresAnimSet'), 'Anims').items) {
    const row = structOf(it); const asset = importPath(setPkg, row.AnimData); const seq = asset.split('.').pop();
    const an = sp.loadAnimation(asset, bones);
    const w = new sp.GlbWriter(); sp.addSkeletonNodes(w, bones); sp.addAnimation(w, seq, an); w.json.scenes = [{ name: seq, nodes: [0] }];
    fs.writeFileSync(path.join(PACK, 'menu', seq + '.glb'), w.finish());
    clips.push({ name: row.AnimName, sequence: seq, asset, file: 'menu/' + seq + '.glb', duration_s: an.length, frames: an.numFrames, interpolation_time: an.props.m_InterpolationTime, notifies: an.notifies.length });
  }
  const animToPlay = (mesh.AnimationData || []).find((x) => x.name === 'AnimToPlay');
  const out = { format: 'kh3-sora-menu-model/1', generator: 'tools/sora_material_pack.js ' + VERSION, source: UI_ACTOR,
    note: 'UE component space of the UI actor: cm, X forward, Y right, Z up; the mesh component is turned by mesh.rotation_deg (yaw -90: the model faces +X, toward the capture). FOVAngle is the horizontal field of view (over the width of the target).',
    target: { width: tp.SizeX, height: tp.SizeY, asset: importPath(d, cap.TextureTarget) },
    capture: { fov_deg: cap.FOVAngle, location_cm: cap.RelativeLocation, rotation_deg: cap.RelativeRotation, show_only: 'the mesh component' },
    mesh: { rotation_deg: mesh.RelativeRotation, anim_class: animClass, anim_to_play: animToPlay ? importPath(d, animToPlay.value) : null, skeletal_mesh: importPath(d, mesh.SkeletalMesh) },
    lights, post_process: { FilmContrast: pp.FilmContrast, FilmHealAmount: pp.FilmHealAmount, FilmDynamicRange: pp.FilmDynamicRange, AmbientCubemapIntensity: pp.AmbientCubemapIntensity, AmbientCubemap: cube, ColorGradingLUT: importPath(d, pp.ColorGradingLUT), blendable: 'UIActorPostProcessMaterial (not reproduced)' },
    ambient_cube_face_means: faces, ambient_cube_face_order: '+X -X +Y -Y +Z -Z (DDS order)', clips };
  fs.writeFileSync(path.join(PACK, 'menu', 'menu.json'), JSON.stringify(out, null, 1));
  return out;
}

// ------------------------------------------------------------------------------------------------ the KHS post-process material
// /Game/Shader/KHS_set/postprocess/KHS (MD_PostProcess, blendable location BL_SQEXAfterLight: drawn by the renderer
// between the lights and the reflection environment, 0x14341C610 in FDeferredShadingSceneRenderer::Render 0x1432FE4A0)
// composes the picture of every pixel whose discernment mask has bit 512 from the G-buffer and the fork's separate
// light buffers. Its pixel shader is translated like a base pass; the textures it reads become per-pixel inputs of the
// harness (crates/kh-app/src/char_material.wgsl `kh_khs`). Which texture is which:
//   uniform buffer GBuffers (resource table of the shader): resources 0..3 = GBufferA..D, 6 = the fork's 7th target
//   FDeferredPixelShaderParameters as the fork serializes it (binder 0x1435108E0, order checked against the two
//   parameters whose identity the bytecode proves: SceneDepthTexture = the texture sent through
//   InvDeviceZToWorldZTransform, GBuffers = the buffer index of the resource table): SceneDepthTexture,
//   ScreenSpaceAOTexture, SQEXLightMapTexture, SQEXBackupSceneColorTexture, SQEXAmbientColorTexture,
//   SQEXIndirectLightWithEmissiveTexture.
const KHS = '/Game/Shader/KHS_set/postprocess/KHS';
const KHS_BP = '/Game/Shader/KHS_set/postprocess/KHS_Blueprint';
// The serialized parameter list (after the material buffer and the collections): index -> what it is.
const KHS_PARAM = { 7: ['SceneDepthTexture', 'i.sdepth'], 47: ['ScreenSpaceAOTexture', 'i.sao'], 52: ['SQEXLightMapTexture', 'i.slight'], 54: ['SQEXBackupSceneColorTexture', 'i.sbackup'], 56: ['SQEXAmbientColorTexture', 'i.samb'], 58: ['SQEXIndirectLightWithEmissiveTexture', 'i.sind'] };
const KHS_GBUFFER = { 0: ['GBufferATexture', 'i.sa'], 1: ['GBufferBTexture', 'i.sb'], 2: ['GBufferCTexture', 'i.sc'], 3: ['GBufferDTexture', 'i.sd'], 6: ['the fork\'s seventh base-pass target', 'i.sf'] };
// (ER81) `asset` / `key`: another post-process material of the same family (KHS_CG / KHS_Real / KHS_Poo: the same
// serialized parameter layout, checked by the throws below) under its own cache key; the defaults are the build as before.
function khsPrepare(dump, asset = KHS, key = 'khs.pp') {
  const d = pkg(asset); if (!d) return null;
  const e = d.exports.find((x) => x.data && x.data.native && x.data.native.shaderMaps && x.data.native.shaderMaps.length); if (!e) return null;
  const sm = e.data.native.shaderMaps[0].shaderMap; const ue = sm.uniformExpressionSet || {};
  const ps = (sm.shaders || []).find((s) => /PostProcessMaterialPS/.test(s.type) && /HighEnd/.test(s.type)); if (!ps) return null;
  const code = ps.shader.resource.code; dump(key, code);
  const names = (((code.optionalData || []).find((x) => x.key === 'u') || {}).uniformBufferNames || []).map((n) => (typeof n === 'string' ? n : n.s));
  const tex2d = ue.uniform2DTextureExpressions || []; const refs = expressionTextures(d);
  const slots = tex2d.map((x) => ({ parameter: x.parameterName || null, root_default: refs[x.textureIndex] || null }));
  const addressOf = (k) => { const a = slots[k] && slots[k].root_default; const r = a ? textureRecord(a) : null; return r && r.address_x === 'TA_Clamp' ? 'clamp' : 'wrap'; };
  const tables = resourceTables(code, tex2d.length, 0, names, addressOf);
  // scene textures: the GBuffers buffer's resources, then the loose parameters
  const sceneTex = {}, inputs = {};
  const gb = names.indexOf('GBuffers'); const tm = (code.resourceTable || {}).textureMap || [];
  if (gb >= 0 && tm[gb]) for (let k = tm[gb]; k < tm.length && tm[k] !== 0xFFFFFFFF && (tm[k] >>> 24) === gb; k++) { const res = (tm[k] >>> 8) & 0xFFFF, bind = tm[k] & 0xFF; const g = KHS_GBUFFER[res]; if (!g) throw new Error('KHS: GBuffers resource ' + res + ' is not known'); sceneTex[bind] = g[1]; inputs['t' + bind] = g[0]; }
  const params = ps.shader.parameters || [];
  for (const [idx, [name, field]] of Object.entries(KHS_PARAM)) { const p = params[+idx]; if (!Array.isArray(p) || !p[1]) throw new Error('KHS: parameter ' + idx + ' (' + name + ') is not bound: the parameter layout is not the one read'); sceneTex[p[0]] = field; inputs['t' + p[0]] = name; }
  const colls = (ue.parameterCollections || []).map((g) => findCollection(g) || { missing: g });
  const collMap = {}; colls.forEach((c, k) => { collMap[k] = /KHS_Control$/.test(c.asset || '') ? 'c0' : /KHS_RefKit_MatParamCol$/.test(c.asset || '') ? 'k0' : null; });
  return { code, names, slots, colls, collMap, meta: { ubNames: names, ...tables, sceneTex, collMap, texNames: { 0: 'kh_tclamp' }, lod0: true, safeDiv: true, wanted: ['o0.x', 'o0.y', 'o0.z'], ret: 'return o0.xyz;' }, inputs, instructions: null };
}
// The values the worlds give the two collections: the controller blueprint (KHS_Blueprint, a child of the native
// ATresKHShaderController) writes its variables into them in `updateKHSParameter` (read in the disassembled graph:
// build/sora_tmp/impl_charmat2/khs_bp.js) and, while the camp menu renders, the CMP_ variables in `SetCmpBoardReflector`.
// (ER81) `worlds`: the levels whose controller is read ([name, umap file, label]); `campOver`: the camp preset over that
// world's controller instead of the class defaults. The defaults are the build as before.
const KHS_WORLDS = [['olympus', 'Maps/he/umap/he_01/he_01_ENV.umap.json', 'Olympus, Thebes (he_01)'], ['twilight', 'Maps/tt/umap/tt_01/tt_01_ENV.umap.json', 'Twilight Town (tt_01)']];
function khsPresets(colls, worlds = KHS_WORLDS, campOver = null) {
  const bp = pkg(KHS_BP); if (!bp) return null;
  const simple = (e) => { const o = {}; for (const p of e.data.props || []) { if (typeof p.value !== 'object' || p.value === null || ['LinearColor', 'Vector', 'Rotator', 'Color'].includes(p.structName)) o[p.name] = p.value; } return o; };
  const cdo = simple(bp.exports.find((e) => /^Default__/.test(e.objectName)));
  const worldType = (v) => (typeof v === 'string' ? +(/(\d+)$/.exec(v) || [0, 0])[1] : v || 0);
  const num = (v) => (typeof v === 'number' ? v : typeof v === 'boolean' ? +v : 0);
  const rgba = (v) => (v ? ('r' in v ? [v.r, v.g, v.b, v.a] : [v.x, v.y, v.z, 1]) : [0, 0, 0, 0]); // Conv_VectorToLinearColor: alpha 1
  const world = (v) => { const g = (n) => num(v[n]); const c = (n) => rgba(v[n]);
    // KHS_WorldType: 0 KH World, 1 CG World, 2 Real World, 5 Poo World -> WorldTypeInt 0..3 (the blueprint's switch)
    const wt = worldType(v.WorldType);
    return { control: { WorldTypeInt: { 0: 0, 1: 1, 2: 2, 5: 3 }[wt] ?? 0, Chr_KHColVol: g('Chr_KHColVol'), ChangeShader: g('ChangeShader'), ChrShaderWindPower: g('ChrShaderWindPower'), KHS_ChrEmissivMultiCounter: g('KHS_ChrEmissivMultiCounter') },
      refkit: { AO_Patturn: Math.trunc(g('AOPatturn')), IndirectLight_LA: g('IndirectLightLA'), CameraLightingSwith: g('CameraLightingSwitch'), ShadowColorMultiply: c('ShadowColorMultiply'),
        RefboradA: [g('Board A rotate'), g('Board A gradient'), g('Board A Power'), g('Board A PowerBG')], RefboradAColor: c('Board A Color'),
        RefboradB: [g('Board B rotate'), g('Board B gradient'), g('Board B Power'), g('Board B PowerBG')], RefboradBColor: c('Board B Color'),
        RefboradC: [g('Board C rotate'), g('Board WLV2 gradient'), g('Board WLV2 Power'), g('Board C PowerBG')], RefboradCColor: c('Board WLV2 Color'),
        RefboradD: [g('Board D rotate'), g('Board WLV gradient'), g('Board WLV Power'), g('Board D PowerBG')], RefboradDColor: c('Board WLV Color'),
        SSAOShadowPower: g('SSAOShadowPower'), RefLighingMode: g('RefLightMode'), DarkAreaBoostMode: g('DarkAreaBoostMode'),
        UseWorldLightVecterMode: g('UseWorldLightVecterMode'), WorldLightVecter: c('WorldLightVecter'), UseWorldLightVecter2Mode: g('UseWorldLightVecter2Mode'), WorldLightVecter2: c('WorldLightVecter2'),
        UseWLVAreaDepth_A: g('UseWLVAreaDepth_A'), WLVecter_depthDarkLength0: g('WLV_Aresdepth_A_Length_0'), WLVecter_depthShineLength0: g('WLV_Aresdepth_A_Length_1'),
        UseWLVAreaDepth_B: g('UseWLVAreaDepth_B'), WLVecter_depthShineLength1: g('WLV_Aresdepth_B_Length_1'), WLVecter_depthDarkLength1: g('WLV_Aresdepth_B_Length_0'),
        UseWLVAreaDepth_Fov: g('UseWLVAreaDepth_Fov'), WLVAreaDepth_Invert: g('WLVAreaDepth_Invert'), WLVAreaDepth_DarknessVolume: g('WLVAreaDepthDarknessVolume') } }; };
  const camp = (v) => { const w = world(v); const g = (n) => num(v[n]); const c = (n) => rgba(v[n]);
    Object.assign(w.refkit, { CameraLightingSwith: g('CMP_CameraLightingSwitch'), ShadowPower: g('CMP_ShadowPower'), SSAOShadowPower: g('CMP_SSAOShadowPower'), DarkAreaBoostMode: g('CMP_DarkAreaBoostMode'), RefLighingMode: g('CMP_RefLightMode'),
      RefboradAColor: c('CMP_Board A Color'), RefboradA: [g('CMP_Board A rotate'), g('CMP_Board A gradient'), g('CMP_Board A Power'), g('Board A PowerBG')],
      RefboradBColor: c('CMP_Board B Color'), RefboradB: [g('Board B rotate'), g('Board B gradient'), g('CMP_Board B Power'), g('Board B PowerBG')],
      UseWorldLightVecterMode: g('CMP_UseWorldLightVecterMode'), WorldLightVecter: c('CMP_WorldLightVecter'), RefboradDColor: c('CMP_Board WLV Color'), RefboradD: [g('Board D rotate'), g('CMP_Board WLV gradient'), g('CMP_Board WLV Power'), g('Board D PowerBG')],
      UseWorldLightVecter2Mode: g('CMP_UseWorldLightVecter2Mode'), WorldLightVecter2: c('CMP_WorldLightVecter2'), RefboradCColor: c('CMP_Board WLV2 Color'), RefboradC: [g('Board C rotate'), g('CMP_Board WLV2 gradient'), g('CMP_Board WLV2 Power'), g('Board C PowerBG')] });
    return w; };
  const presets = { collection: { source: 'the defaults of the two parameter collections (no controller)', control: {}, refkit: {} },
    blueprint: { source: KHS_BP + ' class defaults through updateKHSParameter (a world whose controller overrides nothing)', ...world(cdo) },
    camp: { source: KHS_BP + ' class defaults through SetCmpBoardReflector (camp render mode: the menu\'s character pictures)', ...camp(cdo) } };
  // worlds of type "KH World" (the type whose post-process material is KHS; CG / Real / Poo worlds run KHS_CG / KHS_Real / KHS_Poo)
  for (const [name, file, label] of worlds) {
    const f = path.join(PKG, file); if (!fs.existsSync(f)) continue;
    const d = JSON.parse(fs.readFileSync(f, 'utf8')); const k = d.exports.find((e) => e.class === 'KHS_Blueprint_C'); if (!k) continue;
    const inst = simple(k); const over = Object.keys(inst).filter((n) => !['KHSPostVolume', 'KHS', 'RootComponent', 'm_CurrentPostProcessMaterial'].includes(n));
    const sun = d.exports.filter((e) => e.class === 'DirectionalLightComponent').map(simple)[0] || null, sky = d.exports.filter((e) => e.class === 'SkyLightComponent').map(simple)[0] || null;
    presets[name] = { source: `${label}: the KHS_Blueprint_C placed in ${file} over the class defaults (overrides: ${over.join(', ')})`, ...world({ ...cdo, ...inst }),
      level: { directional_light: sun ? { intensity: sun.Intensity ?? 3.1415927, exposure_controlled: !!sun.m_IntensityExposureControl, exposure_base: sun.m_IntensityExposureBase ?? null, color: sun.LightColor ? [sun.LightColor.r, sun.LightColor.g, sun.LightColor.b] : [255, 255, 255], rotation: sun.RelativeRotation || null } : null, sky_light: sky ? { intensity: sky.Intensity ?? 1 } : null } };
    if (campOver === name) presets.camp = { source: `${KHS_BP} through SetCmpBoardReflector over the KHS_Blueprint_C of ${label} (camp render mode in that world)`, ...camp({ ...cdo, ...inst }) };
  }
  // check every name against the collections
  for (const [pn, p] of Object.entries(presets)) for (const [cn, arr] of [['control', 'c0'], ['refkit', 'k0']]) { const coll = colls[arr]; for (const n of Object.keys(p[cn])) if (!coll || !(n in coll.params)) throw new Error(`KHS preset ${pn}: ${cn} has no parameter ${n}`); }
  return presets;
}

// ------------------------------------------------------------------------------------------------ what the mesh asset says
// The skeletal mesh's own tagged properties (kept by the mesh converter in <mesh>.mesh.json):
//   m_BonePoseToMaterialParam  the fork's table "bone pose -> material parameter" (USkeletalMesh +0x338,
//                              struct SqEX_BonePoseToMaterialParam): which bone, what of its pose
//                              (ESqEX_BonePoseToMaterialCheckType), the parameter's name, the material instances
//                              it is written on (none listed = all), and an optional remap of the value
//   importedBounds             FBoxSphereBounds of the cooked mesh: origin, box extent, sphere radius (cm, component space)
function meshFacts(content) {
  const p = path.join(ROOT, 'assets_converted/meshes', CONTENT, content + '.mesh.json'); if (!fs.existsSync(p)) return null;
  const text = fs.readFileSync(p, 'utf8'); const d = JSON.parse(text);
  let arr = null; const find = (x) => { if (arr || !x || typeof x !== 'object') return; if (x.tag && x.tag.name === 'm_BonePoseToMaterialParam') { arr = x.value.items; return; } for (const v of Object.values(x)) find(v); }; find(d);
  const P = (it) => Object.fromEntries(it.props.map((q) => [q.tag.name, q.tag.type === 'BoolProperty' ? !!q.tag.boolVal : q.value]));
  const v3 = (x) => (x ? [x.x, x.y, x.z] : [0, 0, 0]);
  const bone_params = (arr || []).map((it) => { const q = P(it); return { bone: P(q.m_Bone).BoneName, check: (q.m_CheckType || 'ESqEX_BonePoseToMaterialCheckType::SQEX_BPMCT_LOCAL_ROT_DIFF').replace(/.*SQEX_BPMCT_/, ''), parameter: q.m_MaterialParamName, set: (q.m_MaterialParamSetType || 'SQEX_BPMPT_SCALAR').replace(/.*SQEX_BPMPT_/, ''), lod_filter: q.m_LodFilter ?? -1,
    materials: q.m_MaterialInstanceList ? q.m_MaterialInstanceList.items : [], normalize: !!q.m_bNeedNormalize, normalize_min: v3(q.m_NormalizeParamRangeMin), normalize_max: v3(q.m_NormalizeParamRangeMax), set_min: v3(q.m_SetParamRangeMin), set_max: v3(q.m_SetParamRangeMax) }; });
  const m = /"importedBounds":\[([^\]]+)\]/.exec(text); const b = m ? m[1].split(',').map(Number) : null;
  return { source: 'assets_converted/meshes/' + CONTENT + '/' + content + '.mesh.json', bone_params, bounds: b && b.length === 7 ? { origin: b.slice(0, 3), extent: b.slice(3, 6), radius: b[6] } : null };
}
// The wear forms (UTresWearformSet of the pawn's equipment component): per form, the textures the equipment component
// puts into FormChangeTex1 / FormChangePointTex1 of the named material instances while it slides FormChangeSlide from
// 0 to 1, and into FormChangeTex0 / FormChangePointTex0 when the slide is over (0x1417915A0 start, 0x141790C80 tick).
const WEARFORMS = '/Game/Blueprints/Player/p_ex001/p_ex001_WearformSet';
function wearForms(want, set = WEARFORMS) {
  if (!set) return null;
  const d = pkg(set); if (!d) return null; const e = d.exports.find((x) => /WearformSet/.test(x.class)); if (!e) return null;
  const out = {};
  for (const pr of e.data.props || []) { if (pr.name !== 'm_Forms') continue; const idx = pr.arrayIndex || 0; if (!want.includes(idx)) continue; const u = structOf(pr.value);
    out[idx] = { name: u.m_FormName || 'Normal', replace: !!u.m_bTextureReplace, materials: ((u.m_TextureReplaceData || {}).items || []).map((it) => { const r = structOf(it); const tex = (i) => (i && i < 0 ? importPath(d, i) : null);
      return { index: r.m_ReplaceIndex, material: r.m_ReplaceMaterialName, image: tex(r.m_ImageTexture), normal: tex(r.m_NormalTexture), mask: tex(r.m_MaskTexture), point: tex(r.m_PointTexture) }; }) }; }
  return out;
}

// ------------------------------------------------------------------------------------------------ main
function main() {
  fs.mkdirSync(CACHE, { recursive: true }); fs.mkdirSync(path.join(OUT, 'textures'), { recursive: true });
  const manifest = JSON.parse(fs.readFileSync(path.join(PACK, 'manifest.json'), 'utf8'));
  const wman = fs.existsSync(path.join(MAIN_PACK, 'weapon/manifest.json')) ? JSON.parse(fs.readFileSync(path.join(MAIN_PACK, 'weapon/manifest.json'), 'utf8')) : { materials: [] };
  const slots = [...manifest.materials.map((m) => ({ mesh: 'sora', slot: m.slot, name: m.slot_name, asset: m.material_instance })), ...wman.materials.map((m) => ({ mesh: 'kingdom_key', slot: m.slot, name: m.slot_name, asset: m.material_instance }))];
  // ---- instances and owners
  const owners = new Map();
  for (const s of slots) {
    s.info = gather(s.asset);
    if (!s.info.owner) throw new Error('no shader map in the chain of ' + s.asset);
    if (!owners.has(s.info.owner)) owners.set(s.info.owner, { asset: s.info.owner, name: s.info.owner.split('.').pop(), users: [] });
    owners.get(s.info.owner).users.push(s);
  }
  const list = [...owners.values()]; list.forEach((o, i) => { o.id = i; });
  // ---- per owner: shader map, textures, bytecode
  let wrote = 0;
  const dump = (key, code) => { const bin = path.join(CACHE, key + '.bin'); const buf = Buffer.from(code.dxbc, 'hex'); if (!fs.existsSync(bin) || !fs.readFileSync(bin).equals(buf)) { fs.writeFileSync(bin, buf); const asm = bin.replace(/\.bin$/, '.asm'); if (fs.existsSync(asm)) fs.unlinkSync(asm); wrote++; } };
  for (const o of list) {
    const d = pkg(o.asset); const e = d.exports.find((x) => x.data && x.data.native && x.data.native.shaderMaps && x.data.native.shaderMaps.length);
    const sm = e.data.native.shaderMaps[0].shaderMap; o.sm = sm;
    const ue = sm.uniformExpressionSet || {};
    o.description = (sm.debugDescription || '').replace(/^Compiling [^:]*: /, '');
    o.blend = (/BlendMode=(\w+)/.exec(o.description) || [])[1]; o.lighting = (/LightingModel=(\w+)/.exec(o.description) || [])[1]; o.two_sided = /TwoSided=1/.test(o.description);
    o.switches = [...o.description.matchAll(/StaticSwitch'([^']+)'=True/g)].map((m) => m[1]);
    o.vec = (ue.uniformVectorExpressions || []).map(tree); o.sca = (ue.uniformScalarExpressions || []).map(tree);
    o.tex2d = ue.uniform2DTextureExpressions || []; o.nCube = (ue.uniformCubeTextureExpressions || []).length;
    o.collections = (ue.parameterCollections || []).map((g) => findCollection(g) || { missing: g });
    const info = o.users[0].info; const rootRefs = expressionTextures(pkg(info.root));
    // texture slots: a parameter takes the instance's value (per material, below); a plain sample the root's reference
    o.slots = o.tex2d.map((x) => ({ parameter: x.parameterName || null, root_default: rootRefs[x.textureIndex] || null }));
    const addressOf = (k) => { const s = o.slots[k]; const a = s && (s.parameter ? info.textures[s.parameter] : null) || (s && s.root_default); const r = a ? textureRecord(a) : null; return r && r.address_x === 'TA_Clamp' ? 'clamp' : 'wrap'; };
    const msm = (sm.meshShaderMaps || []).find((m) => m.vertexFactoryType === VF);
    if (!msm) throw new Error('no ' + VF + ' shaders in ' + o.asset);
    const ps = msm.shaders.find((s) => s.type === BASE_PS); if (!ps) throw new Error('no ' + BASE_PS + ' in ' + o.asset);
    const namesOf = (code) => (((code.optionalData || []).find((x) => x.key === 'u') || {}).uniformBufferNames || []).map((n) => (typeof n === 'string' ? n : n.s));
    const key = `${String(o.id).padStart(2, '0')}_${o.name}`;
    const code = ps.shader.resource.code; dump(key + '.base', code);
    // (party A2) its base-pass vertex shader, read for the interpolant sources (pre-skinned position: psp)
    const vs = msm.shaders.find((s) => s.type === 'TBasePassVSFCachedPointIndirectLightingPolicy'); o.vsKey = vs ? key + '.vs' : null; if (vs) dump(key + '.vs', vs.shader.resource.code);
    const translucent = o.blend === 'BLEND_Translucent';
    // (IMPL_ER59) an outfit's owner: the rows / textures it reads are renumbered after translation (it may read rows past
    // MAX_CB or more texture slots than the harness has, while USING few of them), its collections by asset
    const collMap = OUTFIT ? Object.fromEntries(o.collections.map((c, k) => [k, /KHS_Control$/.test(c.asset || '') ? 'c0' : /KHS_RefKit/.test(c.asset || '') ? 'k0' : null])) : undefined;
    o.base = { remap: OUTFIT, collMap, key: key + '.base', ubNames: namesOf(code), ...resourceTables(code, o.tex2d.length, o.nCube, namesOf(code), addressOf),
      wanted: translucent ? ['o0.x', 'o0.y', 'o0.z', 'o0.w'] : ['o0.x', 'o0.y', 'o0.z', 'o1.x', 'o1.y', 'o1.z', 'o2.x', 'o2.y', 'o2.z', 'o3.x', 'o3.y', 'o3.z', 'o2.w', 'o4.x', 'o4.y', 'o4.z', 'o4.w', 'o6.x', 'o6.y', 'o6.z'] };
    o.mask = null;
    if (o.blend === 'BLEND_Masked') {
      const dps = msm.shaders.find((s) => s.type === 'FDepthOnlyPS');
      if (dps) { const mcode = dps.shader.resource.code; dump(key + '.mask', mcode); o.mask = { remap: OUTFIT, collMap, key: key + '.mask', ubNames: namesOf(mcode), ...resourceTables(mcode, o.tex2d.length, o.nCube, namesOf(mcode), addressOf), wanted: [] }; }
    }
  }
  const khs = khsPrepare(dump);
  // ---- the mesh assets' own facts, the wear forms (NORMAL and TRES_WEAR_FORM_05 = Second Form)
  const facts = { sora: meshFacts(OUTFIT ? manifest.mesh_content : 'Character/pc/p_ex001/mdl/p_ex001_0'), kingdom_key: meshFacts('Character/wep/w_so010/mdl/w_so010_0') };
  const forms = OUTFIT ? wearForms([0, 5], manifest.wearform_set || null) : wearForms([0, 5]);
  if (forms) for (const fm of Object.values(forms)) for (const m of fm.materials) for (const k of ['image', 'normal', 'mask', 'point']) if (m[k]) textureRecord(m[k]);
  if (khs) for (const sl of khs.slots) if (sl.root_default) textureRecord(sl.root_default);
  const missing = fs.readdirSync(CACHE).filter((f) => f.endsWith('.bin') && !fs.existsSync(path.join(CACHE, f.replace(/\.bin$/, '.asm'))));
  if (missing.length) {
    console.log(`disassembling ${missing.length} shaders (d3dcompiler_47.dll) ...`);
    const r = require('./lib/dxbc_disasm').systemOrOwn(CACHE);
    if (r.status !== 0) throw new Error('dxbc_disasm.ps1 failed: ' + (r.stderr || r.stdout));
  }
  // ---- translate
  const wgsl = ['// GENERATED by tools/sora_material_pack.js from the compiled base-pass pixel shaders of the character materials. Do not edit.', ''];
  const outOwners = {}; let ok = 0, total = 0;
  for (const o of list) {
    const read = (k) => { const f = path.join(CACHE, k + '.asm'); const t = fs.existsSync(f) ? fs.readFileSync(f, 'utf8') : ''; return /^HRESULT|^$/.test(t) ? null : t; };
    const asm = read(o.base.key); total++;
    // (party A2) the vertex shader writes TEXCOORD3.xy / TEXCOORD4.x from `mad rN.xyz, v0.xyzx, cb0[7].xyzx, cb0[6].xyzx`
    // (the decompressed local position, before skinning): only then are those interpolants read as the psp
    const vasm = o.vsKey ? read(o.vsKey) : null;
    if (vasm) {
      const sig = {}; for (const l of vasm.split(/\r?\n/)) { const q = /^\/\/ (TEXCOORD)\s+(\d+)\s+\w+\s+(\d+)\s/.exec(l); if (q && !(('TEXCOORD' + q[2]) in sig) && /^\/\/ TEXCOORD/.test(l) && vasm.indexOf(l) > vasm.indexOf('Output signature')) sig['TEXCOORD' + q[2]] = +q[3]; }
      const p = /mad (r\d+)\.xyz, v0\.xyzx, cb0\[7\]\.xyzx, cb0\[6\]\.xyzx/.exec(vasm);
      const r = p && p[1];
      const psp = r && sig.TEXCOORD3 !== undefined && sig.TEXCOORD4 !== undefined && vasm.includes(`mov o${sig.TEXCOORD3}.xy, ${r}.xyxx`) && vasm.includes(`mov o${sig.TEXCOORD4}.x, ${r}.z`);
      if (psp) { o.psp = true; o.base.psp = true; if (o.mask) o.mask.psp = true; }
    }
    const t = asm ? transpile(asm, o.base) : { ok: false, why: 'disassembly failed' };
    let m = null;
    if (o.mask) { const masm = read(o.mask.key); m = masm ? transpile(masm, o.mask) : { ok: false, why: 'disassembly failed' }; if (m.ok && !/discard/.test(m.wgsl)) m = { ok: false, why: 'no clip in the depth-only shader' }; }
    if (OUTFIT && t.ok) { // (IMPL_ER59) the renumbering: rows past MAX_CB / texture slots past MAX_TEXTURES onto the ones used
      const R = [...new Set([...t.rows, ...(m && m.ok ? m.rows : [])])].sort((a, b) => a - b), T = [...new Set([...t.textures, ...(m && m.ok ? m.textures : [])])].sort((a, b) => a - b);
      const needR = R.length > 0 && R[R.length - 1] >= MAX_CB, needT = T.length > 0 && T[T.length - 1] >= Math.max(MAX_TEXTURES, T.length);
      if (R.length > MAX_CB) Object.assign(t, { ok: false, why: 'material constant buffer: ' + R.length + ' rows used' });
      else if (T.length > MAX_TEXTURES_OUTFIT) Object.assign(t, { ok: false, why: 'more than ' + MAX_TEXTURES_OUTFIT + ' textures used (' + T.length + ')' });
      else {
        const rmap = new Map(R.map((r, k) => [r, needR ? k : r])), tmap = new Map(T.map((x, k) => [x, needT ? k : x]));
        const fill = (w) => w.replace(/@@R([0-9]+)@@/g, (_, r) => rmap.get(+r)).replace(/@@T([0-9]+)@@/g, (_, x) => tmap.get(+x));
        t.wgsl = fill(t.wgsl); if (m && m.ok) m.wgsl = fill(m.wgsl);
        if (needT) { o.slots = T.map((x) => o.slots[x]); t.textures = t.textures.map((x) => tmap.get(x)); if (m && m.ok) m.textures = m.textures.map((x) => tmap.get(x)); }
        if (needR) { o.cb_rows = R; t.cb_len = R.length; }
        if (needR || needT) console.log(`  renumbered: ${o.name}: ${R.length} material rows (highest ${R[R.length - 1]}), ${T.length} texture slots (highest ${T[T.length - 1]})`);
      }
    }
    const bad = [...new Set([...o.vec, ...o.sca].flatMap(badIn))];
    const names = { scalars: new Set(), vectors: new Set(), time: false }; for (const x of [...o.vec, ...o.sca]) namesIn(x, names);
    const rec = { id: o.id, asset: o.asset, description: o.description, blend_mode: o.blend, lighting_model: o.lighting, two_sided: o.two_sided, static_switches_true: o.switches,
      ...(o.psp ? { pre_skinned_position: { interpolants: 'TEXCOORD3.xy, TEXCOORD4.x = local position before skinning (base-pass VS: v0 * cb0[7] + cb0[6])', harness: 'KH_PSP: i.tc3.xyz = vertex colour = the mesh\'s bind-pose position in game space (cm)' } } : {}),
      base: t.ok ? { ok: true, shader: BASE_PS, vertex_factory: VF, textures: t.textures, cb_len: t.cb_len, uses_time: t.uses_clock, inputs: t.inputs, view: t.view, primitive: t.primitive, precomputed: t.precomputed, collections: t.collections, instructions: t.instructions } : { ok: false, why: t.why },
      mask: o.mask ? (m.ok ? { ok: true, shader: 'FDepthOnlyPS', textures: m.textures, view: m.view, instructions: m.instructions } : { ok: false, why: m.why }) : null,
      slots: o.slots, ...(o.cb_rows ? { cb_rows: o.cb_rows } : {}), vec: o.vec, sca: o.sca, parameters: { scalars: [...names.scalars].sort(), vectors: [...names.vectors].sort(), time: names.time },
      collections: o.collections.map((c) => (c.missing ? { missing: c.missing } : { asset: c.asset, rows: c.rows, params: c.params })), ...(bad.length ? { bad_uniform_expressions: bad } : {}) };
    if (t.ok) { ok++; wgsl.push(`#ifdef KH_C${o.id}`, `// ${o.asset}`, 'fn kh_material(i: KhIn) -> KhOut {', t.wgsl, '}', // (the mask as a value, 0 = clipped: the harness evaluates it at several dither offsets and writes the coverage)
      ...(m && m.ok ? ['fn kh_mask(i: KhIn) -> f32 {', m.wgsl.replace(/\{ discard; \}/g, '{ return 0.0; }'), '    return 1.0;', '}'] : []), '#endif', ''); }
    outOwners[o.name] = rec;
    console.log(`  ${String(o.id).padStart(2)} ${o.name.padEnd(24)} ${o.lighting} ${o.blend}${o.two_sided ? ' two-sided' : ''}: base ${t.ok ? 'ok (' + t.instructions + ' statements, ' + t.textures.length + ' textures, cb ' + t.cb_len + ')' : 'NOT TRANSLATED: ' + t.why}${o.mask ? '; mask ' + (m.ok ? 'ok (' + m.instructions + ')' : 'no: ' + m.why) : ''}${bad.length ? '; BAD EXPRESSIONS ' + bad.join(',') : ''}`);
    if (REPORT && t.ok) console.log(`       inputs ${t.inputs.join(' ')} | View ${t.view.join(' ')} | Primitive ${t.primitive.join(' ')} | precomputed ${t.precomputed.join(' ')} | collections ${JSON.stringify(t.collections)}${m && m.ok ? ' | mask View ' + m.view.join(' ') : ''}`);
  }
  // ---- the KHS post-process material
  let khsOut = null;
  if (khs) {
    const f = path.join(CACHE, 'khs.pp.asm'); const asm = fs.existsSync(f) ? fs.readFileSync(f, 'utf8') : '';
    const t = asm && !/^HRESULT/.test(asm) ? transpile(asm, khs.meta) : { ok: false, why: 'disassembly failed' };
    const colls = {}; khs.colls.forEach((c, k) => { if (khs.collMap[k]) colls[khs.collMap[k]] = c; });
    wgsl.push('// ' + KHS + ' (FPostProcessMaterialPS, blendable location BL_SQEXAfterLight)', 'fn kh_khs(i: KhIn) -> vec3<f32> {', t.ok ? t.wgsl : '    return i.sbackup.xyz;', '}', '');
    khsOut = { ok: t.ok, ...(t.ok ? { instructions: t.instructions, view: t.view, collections_read: t.collections } : { why: t.why }), asset: KHS, inputs: khs.inputs, slots: khs.slots,
      collections: Object.fromEntries(Object.entries(colls).map(([k, c]) => [k, c.missing ? { missing: c.missing } : { asset: c.asset, rows: c.rows, params: c.params }])), presets: t.ok ? khsPresets(colls) : null };
    console.log('  KHS post-process: ' + (t.ok ? 'ok (' + t.instructions + ' statements; View ' + t.view.join(' ') + '; collections ' + JSON.stringify(t.collections) + '; presets ' + Object.keys(khsOut.presets || {}).join(', ') + ')' : 'NOT TRANSLATED: ' + t.why));
  } else wgsl.push('fn kh_khs(i: KhIn) -> vec3<f32> {', '    return i.sbackup.xyz;', '}', '');
  // (IMPL_ER59) AN OUTFIT'S SLOT WHOSE SHADER WAS NOT TRANSLATED (e.g. a constant buffer over MAX_CB) is drawn with the
  // translated shader of another of the outfit's slots on the SAME root material (most texture parameters in common),
  // fed with its own parameters and textures: a STAND-IN (its static switches may differ), said in `stand_in_for`.
  const standIn = new Map();
  if (OUTFIT) for (const o of list) {
    if (outOwners[o.name].base.ok) continue;
    const root = o.users[0].info.root; const mine = new Set(o.slots.map((x) => x.parameter).filter(Boolean));
    const best = list.filter((c) => c !== o && outOwners[c.name].base.ok && c.users[0].info.root === root).map((c) => [c, c.slots.filter((x) => mine.has(x.parameter)).length]).sort((a, b) => b[1] - a[1])[0];
    if (best) { standIn.set(o.asset, best[0]); console.log(`  stand-in: ${o.name} is drawn with the shader of ${best[0].name} (same root ${root.split('.').pop()}, ${best[1]} texture parameters in common)`); continue; }
    // else an opaque / masked slot: the Kingdom Key's shader (w_allBaseMat: base colour, normal, mask, AO), its textures
    // found by the usual parameter names; a translucent one is left out (not drawn)
    const key = list.find((c) => c.name === 'w_so010_mat0_Inst' && outOwners[c.name].base.ok);
    if (key && o.blend !== 'BLEND_Translucent') { standIn.set(o.asset, key); console.log(`  stand-in: ${o.name} is drawn with the Kingdom Key's shader (w_allBaseMat; base colour / normal / AO by parameter name)`); }
    else console.log(`  NOT DRAWN: ${o.name} (no translated shader to stand in)`);
  }
  // the parameter names a stand-in's texture slot is looked up by (first found wins)
  const ALIASES = { BaseColorTex: ['BaseColorTex', 'BaseColorTexture', 'FormChangeTex0', 'BaseTex', 'BaseTexture', 'FaceBaseColorTexture', 'EyelsisTexture'], NormalTex: ['NormalTex', 'NormalTexture', 'FaceNormalTexture'], AOTexture: ['AOTexture'], MaskTexture: ['MaskTexture'] };
  const standInTexture = (sl, info) => { if (!sl.parameter) return sl.root_default; for (const n of ALIASES[sl.parameter] || [sl.parameter]) if (info.textures[n]) return info.textures[n]; return null; };
  // ---- per material
  const outMats = {}; const rows = [];
  for (const s of slots) {
    const o = standIn.get(s.info.owner) || owners.get(s.info.owner); const info = s.info;
    const keyStandIn = standIn.has(s.info.owner) && o.name === 'w_so010_mat0_Inst' && s.mesh === 'sora';
    const textures = o.slots.map((sl) => { const asset = keyStandIn ? standInTexture(sl, info) : (sl.parameter && info.textures[sl.parameter]) || sl.root_default; if (!asset) return null; const r = textureRecord(asset); return r.missing ? { asset, missing: true } : r; });
    const blend = info.overrides.BlendMode || info.rootProps.BlendMode || 'BLEND_Opaque';
    const twoSided = 'TwoSided' in info.overrides ? !!info.overrides.TwoSided : !!info.rootProps.TwoSided;
    outMats[s.name] = { mesh: s.mesh, slot: s.slot, asset: s.asset, chain: info.chain, root: info.root, owner: o.name, id: o.id, blend_mode: blend, two_sided: twoSided,
      shading_model: info.overrides.ShadingModel || info.rootProps.ShadingModel || 'MSM_DefaultLit', clip: info.overrides.OpacityMaskClipValue ?? info.rootProps.OpacityMaskClipValue ?? 0.3333,
      scalars: info.scalars, vectors: info.vectors, texture_parameters: info.textures, textures, ...(standIn.has(info.owner) ? { stand_in_for: info.owner } : {}) };
    s.textures = textures; s.o = o; s.blend = blend; s.twoSided = twoSided;
  }
  const tex = writeTextures();
  for (const m of Object.values(outMats)) m.textures = m.textures.map((r) => (r ? (r.missing ? { asset: r.asset, missing: true } : { file: r.file, asset: r.asset, srgb: r.srgb, address_x: r.address_x, address_y: r.address_y, width: r.width, height: r.height, mips: r.mips, compression: r.compression }) : null));
  // ---- the third UV set (Bevy's vertex layout carries two)
  const uv2 = {};
  for (const [mesh, file, base] of [['sora', manifest.items.find((i) => i.kind === 'mesh').file, PACK], ['kingdom_key', 'weapon/kingdom_key.glb', MAIN_PACK]]) {
    if (!fs.existsSync(path.join(base, file))) continue;
    const g = sp.readGlb(path.join(base, file));
    g.json.meshes[0].primitives.forEach((p, k) => {
      if (p.attributes.TEXCOORD_2 === undefined) return;
      const a = sp.readAccessor(g, p.attributes.TEXCOORD_2); const buf = Buffer.alloc(a.data.length * 4); a.data.forEach((v, i) => buf.writeFloatLE(v, i * 4));
      const name = `uv2_${mesh}_${k}.bin`; fs.writeFileSync(path.join(OUT, name), buf);
      uv2[`${mesh}:${g.json.materials[p.material].name}`] = { file: name, vertices: a.data.length / 2, primitive: k };
    });
  }
  fs.writeFileSync(path.join(OUT, 'materials.wgsl'), wgsl.join('\n'));
  fs.writeFileSync(path.join(OUT, 'materials.json'), JSON.stringify({ format: 'kh3-sora-materials/1', generator: 'tools/sora_material_pack.js ' + VERSION, max_textures: MAX_TEXTURES, max_cb: MAX_CB, max_collection: MAX_COLLECTION,
    note: 'owners: the packages whose shader map a material runs (base = the translated base-pass pixel shader in materials.wgsl under KH_C<id>; vec / sca = the uniform expressions of the material constant buffer, as trees: c constant, s scalar parameter, v vector parameter, t game time, f function); materials: by mesh slot name',
    owners: outOwners, materials: outMats, uv2, mesh_facts: facts,
    wear_forms: forms ? Object.fromEntries(Object.entries(forms).map(([k, fm]) => [k, { ...fm, materials: fm.materials.map((m) => { const o = { index: m.index, material: m.material }; for (const key of ['image', 'normal', 'mask', 'point']) { const r = m[key] ? textureRecord(m[key]) : null; if (r && !r.missing) o[key] = { file: r.file, asset: r.asset, srgb: r.srgb, mips: r.mips }; } return o; }) }])) : null,
    khs: khsOut ? { ...khsOut, textures: khs.slots.map((sl) => { const r = sl.root_default ? textureRecord(sl.root_default) : null; return r && !r.missing ? { file: r.file, asset: r.asset, srgb: r.srgb, address_x: r.address_x, address_y: r.address_y, width: r.width, height: r.height, mips: r.mips, compression: r.compression } : null; }) } : null }));
  if (OUTFIT) { // (IMPL_ER59) an outfit folder: its materials only (sheet rows: tools/sora_outfit_pack.js)
    console.log(`outfit ${path.basename(PACK)}: ${slots.length} material slots, ${list.length} shader owners, ${ok} of ${total} base-pass shaders translated; textures ${tex.wrote} written, ${tex.kept} kept, ${[...texCache.values()].filter((r) => r.missing).length} missing`);
    for (const r of texCache.values()) if (r.missing) console.log('  missing texture: ' + r.asset);
    return;
  }
  // ---- the sheet
  for (const s of slots) {
    const o = s.o; const info = s.info; const id = `${s.mesh}.${s.name}`;
    const src = `assets_converted/packages/${CONTENT}/${s.asset.replace(/^\/Game\//, '').split('.')[0]}.uasset.json`;
    const push = (suffix, value, unit, note, conf = 'confirmed') => rows.push([`${id}${suffix}`, value, unit, src, 'asset', '-', conf, note]);
    push('', s.asset, 'asset', `material slot ${s.slot} of ${s.mesh}; chain ${info.chain.map((c) => c.split('.').pop()).join(' > ')}`);
    push('#root', info.root, 'asset', 'root material of the chain');
    push('#owner', o.asset, 'asset', `package whose compiled shader map the slot runs (static switches set: ${o.switches.join(', ') || 'none'})`);
    push('#shading_model', o.lighting, 'enum', 'LightingModel of the shader map description; the deferred light shader of the global shader cache applies it (stock 4.17 code for this model, read in the bytecode)');
    push('#blend_mode', s.blend, 'enum', 'BlendMode (root material or the instance override)');
    push('#two_sided', s.twoSided ? 1 : 0, 'bool', '-');
    push('#base_pass', outOwners[o.name].base.ok ? 'translated' : 'not translated: ' + outOwners[o.name].base.why, 'state', `${BASE_PS} of ${VF} -> build/sora_pack/materials/materials.wgsl KH_C${o.id}`);
    if (outOwners[o.name].mask) push('#opacity_mask', outOwners[o.name].mask.ok ? 'translated' : 'none: ' + outOwners[o.name].mask.why, 'state', 'FDepthOnlyPS (a masked material clips in the depth-only pass)');
    for (const [n, v] of Object.entries(info.scalars).sort()) push(`#scalar:${n}`, v, 'scalar', `set by ${info.from[n]}${outOwners[o.name].parameters.scalars.includes(n) ? '' : '; not read by the compiled shader map (folded by a static switch or unused)'}`);
    for (const [n, v] of Object.entries(info.vectors).sort()) push(`#vector:${n}`, v.map((x) => +(+x).toPrecision(7)).join(' '), 'rgba', `set by ${info.from[n]}${outOwners[o.name].parameters.vectors.includes(n) ? '' : '; not read by the compiled shader map'}`);
    o.slots.forEach((sl, k) => { const r = s.textures[k]; push(`#texture${k}:${sl.parameter || '(fixed)'}`, r ? r.asset : null, 'asset', r ? (r.missing ? 'NOT CONVERTED (no .dds)' : `${r.file}; ${r.srgb ? 'sRGB' : 'linear'}; ${r.compression}; ${r.width}x${r.height}; ${sl.parameter ? (info.textures[sl.parameter] ? 'instance / chain value' : 'root default') : 'fixed sample of the root material or of a material function'}`) : 'slot not resolved', r && !r.missing ? 'confirmed' : 'guess'); });
    for (const n of outOwners[o.name].parameters.scalars) if (!(n in info.scalars)) { const dflt = findDefault(o, 's', n); push(`#default_scalar:${n}`, dflt, 'scalar', 'read by the shader map, not set by the chain: the default compiled into the uniform expression'); }
    for (const n of outOwners[o.name].parameters.vectors) if (!(n in info.vectors)) { const dflt = findDefault(o, 'v', n); push(`#default_vector:${n}`, dflt ? dflt.map((x) => +(+x).toPrecision(7)).join(' ') : null, 'rgba', 'read by the shader map, not set by the chain: the default compiled into the uniform expression'); }
  }
  // ---- the mesh assets' tables, the wear forms
  for (const [mesh, fa] of Object.entries(facts)) { if (!fa) continue;
    const push = (id, value, unit, note) => rows.push([`${mesh}.mesh#${id}`, value, unit, fa.source, 'asset', '-', 'confirmed', note]);
    if (fa.bounds) push('bounds', [...fa.bounds.origin, ...fa.bounds.extent, fa.bounds.radius].map((x) => +x.toFixed(3)).join(' '), 'cm', 'importedBounds of the cooked mesh: origin xyz, box extent xyz, sphere radius (component space) -> Primitive rows ObjectWorldPositionAndRadius / ObjectBounds / LocalObjectBoundsMin / Max');
    for (const b of fa.bone_params) push('bone_param:' + b.parameter, b.bone, 'bone', `m_BonePoseToMaterialParam: ${b.check} of the bone -> ${b.set.toLowerCase()} parameter on ${b.materials.length ? b.materials.join(', ') : 'every material'}${b.normalize ? '; remapped from ' + b.normalize_min.join(' ') + ' .. ' + b.normalize_max.join(' ') + ' to ' + b.set_min.join(' ') + ' .. ' + b.set_max.join(' ') : ''}`); }
  if (forms) for (const [k, fm] of Object.entries(forms)) for (const m of fm.materials) rows.push([`sora.wear_form${k}.${m.material}`, m.image, 'asset', `assets_converted/packages/${CONTENT}/Blueprints/Player/p_ex001/p_ex001_WearformSet.uasset.json`, 'asset', '0x1417915A0', 'confirmed', `wear form ${k} (${fm.name}): FormChangeTex of material index ${m.index}; point texture ${m.point}`]);
  if (forms) { const spec = 'docs/sora-combat-spec.md';
    rows.push(['sora.wear_form.slide_on', 1.0, 's', spec, 'decompiled C', '0x1413CBA70', 'inferred', 'FormChangeSlide 0 -> 1 when a form is put on: STYLECHANGE sub-step 3 applies the wear form through pawn.vt+0x1498(form, 1.0); the last argument reaches UTresEquipmentComponent 0x1417915A0 as the slide\'s length; the ramp\'s shape (the interpolator of 0x141790C80) was not read: linear in the port']);
    rows.push(['sora.wear_form.slide_off', 0.2, 's', spec, 'decompiled C', '0x1417915A0', 'inferred', 'the slide back to wear form NORMAL when the form ends: pawn.vt+0x13E0(1, 0, 0, 0.2)']); }
  // ---- the KHS post-process material
  if (khsOut) {
    const src = `assets_converted/packages/${CONTENT}/Shader/KHS_set/postprocess/KHS.uasset.json`;
    const push = (id, value, unit, note, source = src) => rows.push([`khs.${id}`, value, unit, source, 'asset', '-', 'confirmed', note]);
    push('material', KHS, 'asset', 'post-process material of the worlds of type "KH World" (MD_PostProcess, BL_SQEXAfterLight); pixel shader ' + (khsOut.ok ? 'translated -> materials.wgsl fn kh_khs (' + khsOut.instructions + ' statements)' : 'NOT translated: ' + khsOut.why));
    for (const [t, name] of Object.entries(khsOut.inputs)) push('input.' + t, name, 'texture', 'texture register of the pixel shader: identified from the shader resource table (GBuffers buffer) or the serialized parameter list (binder 0x1435108E0)');
    khs.slots.forEach((sl, k) => { const r = sl.root_default ? textureRecord(sl.root_default) : null; push('texture' + k, sl.root_default, 'asset', r && !r.missing ? `${r.file}; ${r.srgb ? 'sRGB' : 'linear'}; ${r.width}x${r.height}; the colour tables (16 slices a row)` : 'not converted'); });
    for (const [arr, c] of Object.entries(khsOut.collections)) if (!c.missing) push('collection.' + arr, c.asset, 'asset', Object.keys(c.params).length + ' parameters, ' + c.rows.length + ' rows');
    for (const [pn, p] of Object.entries(khsOut.presets || {})) { push('preset.' + pn, p.source, 'text', '--khs-preset ' + pn);
      for (const [cn, vals] of [['control', p.control], ['refkit', p.refkit]]) for (const [k, v] of Object.entries(vals)) push(`preset.${pn}.${cn}.${k}`, Array.isArray(v) ? v.map((x) => +(+x).toPrecision(7)).join(' ') : v, Array.isArray(v) ? 'rgba' : 'scalar', 'value the controller blueprint writes into the collection parameter');
      if (p.level && p.level.directional_light) push(`preset.${pn}.sun`, p.level.directional_light.intensity, 'intensity', 'the directional light of the level (colour ' + p.level.directional_light.color.join(' ') + ', exposure base ' + p.level.directional_light.exposure_base + '); NOT applied to the test level'); }
  }
  // ---- the camp menu's model
  const menu = menuPack();
  if (menu) {
    const src = `assets_converted/packages/${CONTENT}/UI/Blueprints/CampTop/p_ex001_UIActor.uasset.json`;
    const push = (id, value, unit, note) => rows.push([`menu.${id}`, value, unit, src, 'asset', '-', 'confirmed', note]);
    const v3 = (v) => [v.x, v.y, v.z].map((x) => +x.toFixed(2)).join(' '), rot = (r) => [r.pitch, r.yaw, r.roll].map((x) => +x.toFixed(2)).join(' ');
    push('target', `${menu.target.width}x${menu.target.height}`, 'px', 'TextureRenderTarget2D of the scene capture (' + menu.target.asset + ')');
    push('capture.fov', menu.capture.fov_deg, 'deg', 'SceneCaptureComponent2D.FOVAngle: the horizontal field of view (over the 512 px width); checked against the scale of the model in a capture of the real menu');
    push('capture.location', v3(menu.capture.location_cm), 'cm', 'RelativeLocation of the capture in the UI actor');
    push('capture.rotation', rot(menu.capture.rotation_deg), 'deg', 'pitch yaw roll');
    push('mesh.rotation', rot(menu.mesh.rotation_deg), 'deg', 'the mesh component: yaw -90 = the model faces the capture');
    push('mesh.anim_class', menu.mesh.anim_class, 'asset', 'camp anim blueprint (native parent TresCampPawnAnimInstance)');
    for (const c of menu.clips) push('clip.' + c.name, c.asset, 'asset', `${c.file}; ${c.duration_s} s, ${c.frames} frames, ${c.notifies} notifies`);
    for (const l of menu.lights) push('light.' + l.name, v3(l.location_cm), 'cm', `spot light: intensity ${l.intensity}, cone ${l.inner_cone_deg} / ${l.outer_cone_deg} deg, radius ${l.attenuation_radius_cm} cm, MinRoughness ${+l.min_roughness.toFixed(3)}, rotation (pitch yaw roll) ${rot(l.rotation_deg)}`);
    push('post.AmbientCubemapIntensity', menu.post_process.AmbientCubemapIntensity, 'scale', 'ambient cube map ' + menu.post_process.AmbientCubemap + '; face means ' + JSON.stringify(menu.ambient_cube_face_means));
    push('post.film', [menu.post_process.FilmContrast, menu.post_process.FilmHealAmount, menu.post_process.FilmDynamicRange].join(' '), 'scalar', 'FilmContrast FilmHealAmount FilmDynamicRange overrides of the capture; colour grading LUT ' + menu.post_process.ColorGradingLUT + ' (not reproduced)');
    console.log(`menu model: ${menu.clips.map((c) => c.name + ' = ' + c.sequence + ' (' + c.duration_s + ' s)').join(', ')}; target ${menu.target.width}x${menu.target.height}, FOV ${menu.capture.fov_deg}, ${menu.lights.length} spot lights`);
  }
  const sheet = { sheet: 'sora-materials', group: 'port', columns: ['id', 'value', 'unit', 'source', 'evidence', 'ref_addr', 'ref_conf', 'note'], rows };
  fs.writeFileSync(path.join(ROOT, 'sheets/port/sora-materials.json'), '{"sheet":"sora-materials","group":"port","columns":' + JSON.stringify(sheet.columns) + ',"rows":[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
  console.log(`${slots.length} material slots, ${list.length} shader owners, ${ok} of ${total} base-pass shaders translated (${wrote} new bytecode files); textures: ${tex.wrote} written, ${tex.kept} kept, ${[...texCache.values()].filter((r) => r.missing).length} missing; ${rows.length} sheet rows`);
  for (const r of texCache.values()) if (r.missing) console.log('  missing texture: ' + r.asset);
  assetRows();
}

// The files of PACK/materials and PACK/menu as rows of sheets/port/sora-assets.json (the sheet is written by
// tools/sora_pack.js, which does not know these two folders: RERUN this tool after it).
function assetRows() {
  const crypto = require('crypto'); const sha = (b) => crypto.createHash('sha256').update(b).digest('hex');
  const sheetFile = path.join(ROOT, 'sheets/port/sora-assets.json'); if (!fs.existsSync(sheetFile)) return;
  const sheet = JSON.parse(fs.readFileSync(sheetFile, 'utf8'));
  const byFile = new Map(); for (const r of texCache.values()) if (!r.missing) byFile.set('materials/' + r.file, r);
  const rows = [];
  const walk = (dir, prefix) => { for (const e of fs.readdirSync(dir, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name, 'en'))) { const p = path.join(dir, e.name); if (e.isDirectory()) { walk(p, prefix + e.name + '/'); continue; }
    const id = prefix + e.name; const b = fs.readFileSync(p); const t = byFile.get(id);
    // (the source column names an asset file: the model's mesh asset for what the tool derives from several assets)
    let kind = 'data', logical = '-', source = CONTENT + '/Character/pc/p_ex001/mdl/p_ex001_0.uasset', validation = 'read back', evidence = '';
    if (t) { kind = 'texture'; logical = [t.srgb ? 'srgb' : 'linear']; source = (/^\/Engine\//.test(t.asset) ? 'Engine/Content/' : CONTENT + '/') + t.asset.replace(/^\/(Game|Engine)\//, '').split('.')[0] + '.uasset'; validation = 'png read back by the app (all four channels kept)'; evidence = 'texture slot of a character material\'s shader map, of the KHS post-process material, or a wear form\'s texture (sheet sora-materials)'; }
    else if (/^materials\/materials\.json$/.test(id)) { validation = 'json read back; the app\'s unit tests read its collections, tables and presets'; evidence = 'shader owners, uniform expressions, parameters, texture slots, the KHS post-process material with its collections and presets, the mesh assets\' bone table and bounds, the wear forms (sheet sora-materials)'; }
    else if (/^materials\/materials\.wgsl$/.test(id)) { validation = 'compiled by the app at start (naga); every function was drawn in a looked-at run'; evidence = 'the compiled base-pass / depth-only pixel shaders of the character materials and the pixel shader of /Game/Shader/KHS_set/postprocess/KHS, translated instruction by instruction'; }
    else if (/^materials\/er\//.test(id)) { validation = 'Direct3D 12 pipeline states are created from them in Elden Ring 2.7.1.0 and in the standalone harness of crates/kh-er-render (looked-at runs)'; evidence = 'built by crates/kh-shaderc (cargo run --release -p kh-shaderc; RERUN it after this tool) from materials.wgsl and the shared harness of crates/kh-look: naga WGSL -> HLSL, dxc -> DXIL'; }
    else if (/^materials\/uv2_/.test(id)) { validation = 'vertex count equal to the mesh primitive\'s (checked by the app)'; evidence = 'third UV set of a mesh primitive (TEXCOORD_2 of the pack\'s glTF)'; }
    else if (/^menu\/menu\.json$/.test(id)) { source = CONTENT + '/UI/Blueprints/CampTop/p_ex001_UIActor.uasset'; validation = 'json read back; unit test the_menu_rig_is_the_ui_actors'; evidence = 'the camp menu\'s UI actor: capture, lights, post-process settings, clips'; }
    else if (/^menu\/.*\.glb$/.test(id)) { kind = 'animation'; logical = [e.name === '0001CM.glb' ? 'Idle_CM' : 'IdleBreak_CM']; source = CONTENT + '/Character/pc/p_ex001/anm/UI/body/' + e.name.replace(/\.glb$/, '.uasset'); validation = 'read by the pose evaluator (kh-anim) at start'; evidence = 'rows of /Game/Character/pc/p_ex001/anm/p_ex001_CM_Body_AnimSet (the UI actor\'s anim blueprint p_ex001_CM_Anim)'; }
    rows.push([id, kind, logical, source, '-', b.length, sha(b), '-', '-', '-', '-', validation, evidence]); } };
  for (const dir of ['materials', 'menu']) if (fs.existsSync(path.join(PACK, dir))) walk(path.join(PACK, dir), dir + '/');
  const kept = sheet.rows.filter((r) => !/^(materials|menu)\//.test(r[0]));
  const all = [...kept, ...rows];
  fs.writeFileSync(sheetFile, '{"sheet":"sora-assets","group":"port","columns":' + JSON.stringify(sheet.columns) + ',"rows":[\n' + all.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
  console.log(`sheets/port/sora-assets.json: ${rows.length} rows of materials/ and menu/ (${all.length} rows in all)`);
}
function findDefault(o, kind, name) { let found = null; const walk = (n) => { if (found !== null) return; if (n[kind] === name) { found = n.d; return; } for (const x of n.a || []) walk(x); }; for (const x of [...o.vec, ...o.sca]) walk(x); return found; }

// ------------------------------------------------------------------------------------------------ (ER81) an outfit's own shader set
// An outfit whose game world runs ANOTHER post-process material than the "KH World" one (Verum Rex, p_dl001: Quadratum,
// ss_01, a "CG World": KHS_CG) gets its own set beside its pack's, built from the same translation:
//   KH_OUTFIT_PACK=build/sora_pack/outfits/<id> node tools/sora_material_pack.js --own-khs --asset /Game/Shader/KHS_set/postprocess/KHS_CG
//     --suffix cg --world "quadratum=Maps/ss_DLC/umap/ss_01/ss_01_Env.umap.json=Quadratum (ss_01)" [--control PixarCoreControl=0] --out DIR
// READS the outfit's materials/materials.wgsl (never written) and writes, in DIR only:
//   materials_<suffix>.wgsl   that file with fn kh_khs replaced by the translation of ASSET (every other line identical)
//   own_shaders.json          the post-process block the renderer reads for the set (kh-look Khs::parse: inputs, collections
//                             c0 = KHS_Control, which the base passes read, + the material's own, presets: the world's
//                             controller and the camp mode over it), the suffix and the world preset's name
// It also translates the KH World's KHS again with the same code and checks the result against the block in the file
// (the translator unchanged for every other set). `kh-shaderc --materials materials_<suffix>.wgsl --suffix <suffix>` builds the DXIL.
function ownKhs() {
  const arg = (n) => { const k = process.argv.indexOf(n); return k >= 0 ? process.argv[k + 1] : null; };
  const asset = arg('--asset'), suffix = arg('--suffix'), outArg = arg('--out'), world = arg('--world');
  if (!OUTFIT || !asset || !suffix || !outArg || !world || !/^[a-z0-9]+$/.test(suffix)) throw new Error('usage: KH_OUTFIT_PACK=DIR node tools/sora_material_pack.js --own-khs --asset A --suffix s --world NAME=Maps/...umap.json[=label] --out DIR');
  const out = path.resolve(outArg);
  if (out.toLowerCase().startsWith(path.resolve(MAIN_PACK).toLowerCase())) throw new Error('--out must not be inside build/sora_pack (the live pack)');
  const [wname, wfile, wlabel] = world.split('=');
  const cache = path.join(ROOT, 'build/cache/char_shaders_own', path.basename(PACK) + '_' + suffix);
  fs.mkdirSync(cache, { recursive: true }); fs.mkdirSync(out, { recursive: true });
  const dump = (k, code) => { const bin = path.join(cache, k + '.bin'); const buf = Buffer.from(code.dxbc, 'hex'); if (!fs.existsSync(bin) || !fs.readFileSync(bin).equals(buf)) { fs.writeFileSync(bin, buf); const a = bin.replace(/\.bin$/, '.asm'); if (fs.existsSync(a)) fs.unlinkSync(a); } };
  const ownKey = 'khs.' + asset.split('/').pop().split('.')[0] + '.pp';
  const own = khsPrepare(dump, asset, ownKey); if (!own) throw new Error(asset + ': no post-process pixel shader in its shader map');
  const std = khsPrepare(dump); if (!std) throw new Error(KHS + ': not found');
  const r = require('./lib/dxbc_disasm').systemOrOwn(cache);
  if (r.status !== 0) throw new Error('dxbc_disasm.ps1 failed: ' + (r.stderr || r.stdout));
  const asmOf = (k) => { const x = fs.readFileSync(path.join(cache, k + '.asm'), 'utf8'); if (/^HRESULT/.test(x)) throw new Error(k + ': disassembly failed'); return x; };
  const t = transpile(asmOf(ownKey), own.meta); if (!t.ok) throw new Error(asset + ': NOT TRANSLATED: ' + t.why);
  const ts = transpile(asmOf('khs.pp'), std.meta); if (!ts.ok) throw new Error(KHS + ': NOT TRANSLATED: ' + ts.why);
  // the material's own collections, then the KH World's for the rest (KHS_Control: the base passes read it)
  const colls = {}; own.colls.forEach((c, k) => { if (own.collMap[k]) colls[own.collMap[k]] = c; });
  std.colls.forEach((c, k) => { const n = std.collMap[k]; if (n && !colls[n]) colls[n] = c; });
  const presets = khsPresets(colls, [[wname, wfile, wlabel || wname]], wname); if (!presets || !presets[wname]) throw new Error('no KHS_Blueprint_C in ' + wfile);
  // `--control NAME=V,..`: KHS_Control values written over the world preset and the camp preset (a look chosen against a
  // reference, NOT read from the game); the world preset as the game data gives it stays as `<world>_game`
  const overrides = {};
  for (const kv of (arg('--control') || '').split(',').filter(Boolean)) { const [n, v] = kv.split('='); if (!colls.c0 || !(n in colls.c0.params) || !isFinite(+v)) throw new Error('--control: ' + kv); overrides[n] = +v; }
  if (Object.keys(overrides).length) {
    presets[wname + '_game'] = JSON.parse(JSON.stringify(presets[wname])); presets[wname + '_game'].source += ' (as the game data gives it)';
    for (const p of [presets[wname], presets.camp]) { Object.assign(p.control, overrides); p.source += ' + ' + JSON.stringify(overrides) + ' (--control: chosen against a reference, not game data)'; }
  }
  // the outfit's generated file with its fn kh_khs replaced
  const srcFile = path.join(OUT, 'materials.wgsl'); const src = fs.readFileSync(srcFile, 'utf8');
  const head = '// ' + KHS + ' (FPostProcessMaterialPS, blendable location BL_SQEXAfterLight)\nfn kh_khs(i: KhIn) -> vec3<f32> {\n';
  const at = src.indexOf(head); if (at < 0 || src.indexOf(head, at + 1) >= 0) throw new Error(srcFile + ': the KHS block was not found exactly once');
  const end = src.indexOf('\n}\n', at + head.length); if (end < 0) throw new Error(srcFile + ': the KHS block does not end');
  const same = src.slice(at + head.length, end) === ts.wgsl;
  console.log('  ' + KHS + ' translated again with this code: ' + (same ? 'IDENTICAL to the block in ' : 'DIFFERS from the block in ') + srcFile);
  if (!same) throw new Error('the translator no longer reproduces the KH World set: nothing written');
  const fresh = '// ' + asset + ' (FPostProcessMaterialPS, blendable location BL_SQEXAfterLight; ER81: the outfit\'s own set "' + suffix + '")\nfn kh_khs(i: KhIn) -> vec3<f32> {\n' + t.wgsl + '\n}\n';
  fs.writeFileSync(path.join(out, 'materials_' + suffix + '.wgsl'), src.slice(0, at) + fresh + src.slice(end + 3));
  const khsOut = { ok: true, instructions: t.instructions, view: t.view, collections_read: t.collections, asset, inputs: own.inputs, slots: own.slots,
    collections: Object.fromEntries(Object.entries(colls).map(([k, c]) => [k, c.missing ? { missing: c.missing } : { asset: c.asset, rows: c.rows, params: c.params }])), presets,
    textures: own.slots.map((sl) => { const x = sl.root_default ? textureRecord(sl.root_default) : null; return x && !x.missing ? { file: x.file, asset: x.asset, srgb: x.srgb, address_x: x.address_x, address_y: x.address_y, width: x.width, height: x.height, mips: x.mips, compression: x.compression } : null; }) };
  const json = { format: 'kh3-sora-own-shaders/1', generator: 'tools/sora_material_pack.js ' + VERSION + ' --own-khs', outfit: path.basename(PACK), suffix, materials: 'materials_' + suffix + '.wgsl',
    world_preset: wname, camp_preset: 'camp', ...(Object.keys(overrides).length ? { control_overrides: overrides, game_preset: wname + '_game' } : {}),
    note: 'the outfit\'s own shader set: materials/er/<variant>_' + suffix + '[_pl|_bl|_bli|_hy2|_gb2].ps.dxil (kh-shaderc --suffix), the post-process material below in place of the KH World\'s (KHS), and its world\'s controller as the preset (KHS_Control WorldTypeInt selects the base passes\' world branch)',
    khs: khsOut };
  fs.writeFileSync(path.join(out, 'own_shaders.json'), JSON.stringify(json, null, 1));
  console.log('  ' + asset + ': ok (' + t.instructions + ' statements; View ' + t.view.join(' ') + '; collections ' + JSON.stringify(t.collections) + '; inputs ' + JSON.stringify(own.inputs) + '; presets ' + Object.keys(presets).join(', ') + ')');
  console.log('  wrote ' + path.join(out, 'materials_' + suffix + '.wgsl') + ', ' + path.join(out, 'own_shaders.json'));
}
if (process.argv.includes('--own-khs')) ownKhs(); else main();
