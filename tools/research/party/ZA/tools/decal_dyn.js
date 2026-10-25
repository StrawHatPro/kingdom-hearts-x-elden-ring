// (ZA) decal_dyn.js IN_SHADERS_DIR OUT_SHADERS_DIR [--report]
// THE DECALS' EMITTER-DRIVEN CONSTANTS. A deferred decal's material constants that are ARITHMETIC over a parameter the
// decal emitter drives every frame (TypeDataSQEX_Decal: Decal_Color, Decal_Opacity, Decal_CustomParam0..2, ZFadePow /
// Segment) are UE preshaders: the engine re-evaluates them on the CPU when the parameter changes. The translator
// (tools/sora_vfx_shaders.js) hands the shader the plain parameters (i.dcol / i.dprm / i.dz) but evaluated the derived
// rows ONCE, at the parameter's default (only the XS table's named decals got them per pixel, XW `--xw-decals`).
// Example: Watera / Waterga / Waterza's wet ground kdc_wat_zmn_00a: mask uv = pos * P0 + (0.5 - 0.5 * P0) with P0 the
// emitter's CustomParam0 (10 -> 1: the wet spot grows); frozen at P0 = 0 the offset stayed 0.5 and the whole decal box
// was drawn: a hard-edged dark rectangle on the floor. Its colour (Decal_Color x Emi Mix) was frozen at white too.
// This pass rewrites, in every DECAL block of a table, each material row that depends on a Decal_ parameter as WGSL over
// the harness inputs; a material parameter that is not driven is read from its own row of the same buffer (kh.m[r].c),
// or is a constant when every material of the owner has the same value. A row it cannot express stays as it was.
'use strict';
const fs = require('fs'), path = require('path');
const ROOT = path.resolve(__dirname, '..', '..', '..', '..', '..');
const PKG = path.join(ROOT, 'assets_converted/packages/KINGDOM HEARTS III/Content');
const [inDir, outDir, fileArg] = process.argv.slice(2);
const FILE = fileArg && !fileArg.startsWith('--') ? fileArg : 'materials.wgsl'; // (a table's overlay, e.g. materials_xwb.wgsl, with the same shading.json)
const REPORT = process.argv.includes('--report');
const T = (e) => (e && e.type ? e.type.replace('FMaterialUniformExpression', '') : '');
const DECAL = { Decal_Opacity: 'vec4<f32>(i.dcol.w)', Decal_CustomParam0: 'vec4<f32>(i.dprm.x)', Decal_CustomParam1: 'vec4<f32>(i.dprm.y)', Decal_CustomParam2: 'vec4<f32>(i.dprm.z)', Decal_ZFadePow: 'vec4<f32>(i.dz.x)', Decal_ZFadeSegment: 'vec4<f32>(i.dz.y)' };
const DECAL_VEC = { Decal_Color: 'vec4<f32>(i.dcol.xyz, 1.0)' };
const f = (x) => { const v = Number(x); const t = Number.isFinite(v) ? String(+v.toPrecision(9)) : '0'; return /[.e]/.test(t) ? t : t + '.0'; };
const v4 = (a) => `vec4<f32>(${a.map(f).join(', ')})`;
const C = 'xyzw';
// A LIT translucent decal (DBM_Translucent, MSM_DefaultLit: Watera / Waterga / Waterza's wet ground kdc_wat_zmn_00a) is a GBuffer
// decal in UE 4.17: it is blended into the scene BEFORE the direct light is added, so its near-black emissive only takes
// away the indirect share of the ground's light (the ground looks wet, not painted black). The hosts composite every decal
// over the LIT picture: its opacity is scaled by that share [A] (an approximation: the share of the test level's and Elden
// Ring's ground light that is not direct sun).
const LIT_SHARE = 0.3;

function ueOf(asset) {
  const rel = asset.replace(/^\/Game\//, '') + '.uasset.json';
  const file = path.join(PKG, rel);
  if (!fs.existsSync(file)) return null;
  const j = JSON.parse(fs.readFileSync(file, 'utf8'));
  const find = (o, d) => { if (!o || typeof o !== 'object' || d > 14) return null; if (o.uniformExpressionSet) return o.uniformExpressionSet; for (const v of Object.values(o)) { const r = find(v, d + 1); if (r) return r; } return null; };
  return find(j, 0);
}
const usesDecal = (e) => { if (!e || typeof e !== 'object') return false; if (/Parameter$/.test(T(e)) && (DECAL[e.parameterName] || DECAL_VEC[e.parameterName])) return true; return Object.values(e).some((v) => typeof v === 'object' && usesDecal(v)); };
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);

/** The WGSL vec4 of an expression; null when it cannot be expressed. `slot(e)` finds a plain parameter's own row. */
function emit(e, slot, constOf, lit) {
  const t = T(e);
  const r = (x) => emit(x, slot, constOf, lit);
  switch (t) {
    case 'Constant': return v4([e.value.r, e.value.g, e.value.b, e.value.a]);
    case 'ScalarParameter': case 'VectorParameter': {
      if (lit && e.parameterName === 'Decal_Opacity') return `vec4<f32>(i.dcol.w * ${f(LIT_SHARE)})`;
      if (DECAL[e.parameterName]) return DECAL[e.parameterName];
      if (DECAL_VEC[e.parameterName]) return DECAL_VEC[e.parameterName];
      const s = slot(e); if (s) return s;
      const c = constOf(e); return c ? v4(c) : null;
    }
    case 'FoldedMath': {
      const a = r(e.a), b = r(e.b); if (a === null || b === null) return null;
      switch (e.op) {
        case 0: return `(${a} + ${b})`; case 1: return `(${a} - ${b})`; case 2: return `(${a} * ${b})`;
        case 3: return `kh_zdiv(${a}, ${b})`; case 4: return `vec4<f32>(dot(${a}, ${b}))`; default: return null;
      }
    }
    case 'AppendVector': {
      const a = r(e.a), b = r(e.b); if (a === null || b === null) return null; const n = e.numComponentsA;
      return `vec4<f32>(${[0, 1, 2, 3].map((k) => (k < n ? `(${a}).${C[k]}` : `(${b}).${C[k - n]}`)).join(', ')})`;
    }
    case 'ComponentSwizzle': {
      const a = r(e.x); if (a === null) return null; const idx = [e.indexR, e.indexG, e.indexB, e.indexA];
      return `vec4<f32>(${[0, 1, 2, 3].map((k) => (k < e.numElements && idx[k] >= 0 ? `(${a}).${C[idx[k]]}` : '0.0')).join(', ')})`;
    }
    case 'Clamp': { const a = r(e.input), lo = r(e.min), hi = r(e.max); return a && lo && hi ? `clamp(${a}, ${lo}, ${hi})` : null; }
    case 'Min': case 'Max': { const a = r(e.a), b = r(e.b); return a && b ? `${t.toLowerCase()}(${a}, ${b})` : null; }
    case 'Abs': case 'Floor': case 'Ceil': case 'Round': case 'Sign': case 'Saturate': case 'Truncate': case 'SquareRoot': case 'Frac': case 'Periodic': {
      const a = r(e.x); if (a === null) return null;
      const fn = { Abs: 'abs', Floor: 'floor', Ceil: 'ceil', Round: 'round', Sign: 'sign', Saturate: 'saturate', Truncate: 'trunc', SquareRoot: 'sqrt', Frac: 'fract', Periodic: 'fract' }[t];
      return t === 'SquareRoot' ? `sqrt(max(${a}, vec4<f32>(0.0)))` : `${fn}(${a})`;
    }
    case 'Sine': { const a = r(e.x); return a ? `${e.bIsCosine ? 'cos' : 'sin'}(${a})` : null; }
    default: return null;
  }
}

const shading = JSON.parse(fs.readFileSync(path.join(inDir, 'shading.json'), 'utf8'));
let wgsl = fs.readFileSync(path.join(inDir, FILE), 'utf8');
const matDir = [path.join(inDir, '..', 'materials'), path.join(ROOT, 'build/play/donpack/vfx/materials'), path.join(ROOT, 'build/sora_pack/vfx/materials'), path.join(ROOT, 'build/sora_pack/party/vfx/materials')];
const matJson = (name) => { for (const d of matDir) { const p = path.join(d, name + '.json'); if (fs.existsSync(p)) return JSON.parse(fs.readFileSync(p, 'utf8')); } return null; };
const byOwner = {};
for (const [name, m] of Object.entries(shading.materials)) (byOwner[m.owner] = byOwner[m.owner] || []).push(name);
let patched = 0, rowsDone = 0, rowsLeft = 0;
const report = [];
for (const [owner, o] of Object.entries(shading.owners)) {
  if (!o.decal || !o.decal.ok) continue;
  const tag = `#ifdef KH_S${o.id}_DECAL\n`;
  const at = wgsl.indexOf(tag); if (at < 0) continue;
  const end = wgsl.indexOf('\n#endif', at); if (end < 0) continue;
  const ue = ueOf(o.asset); if (!ue) continue;
  const vecs = ue.uniformVectorExpressions || [], scas = ue.uniformScalarExpressions || [];
  // a plain (non-driven) parameter's own row in the buffer
  const slot = (e) => {
    if (T(e) === 'VectorParameter') { const i = vecs.findIndex((x) => T(x) === 'VectorParameter' && x.parameterName === e.parameterName); return i >= 0 ? `kh.m[${i}]` : null; }
    const j = scas.findIndex((x) => T(x) === 'ScalarParameter' && x.parameterName === e.parameterName);
    return j >= 0 ? `vec4<f32>(kh.m[${vecs.length + (j >> 2)}].${C[j & 3]})` : null;
  };
  // a parameter with no row of its own: a constant when every material of the owner has the same value
  const constOf = (e) => {
    const vals = (byOwner[owner] || []).map((n) => { const m = matJson(n); const tab = T(e) === 'VectorParameter' ? (m && m.vectors) : (m && m.scalars); const v = tab && tab[e.parameterName] && tab[e.parameterName].value; return v === undefined ? (T(e) === 'VectorParameter' ? e.defaultValue : e.defaultValue) : v; });
    if (!vals.length || !vals.every((v) => same(v, vals[0]))) return null;
    const v = vals[0]; return typeof v === 'number' ? [v, v, v, v] : v && typeof v === 'object' ? [v.r, v.g, v.b, v.a] : null;
  };
  const lit = /MSM_DefaultLit/.test(o.description || '') && (byOwner[owner] || []).some((n) => shading.materials[n].decal_blend === 'DBM_Translucent');
  const rows = {};
  vecs.forEach((e, i) => { if (usesDecal(e)) { const w = emit(e, slot, constOf, lit); if (w) rows[i] = w; else rowsLeft++; } });
  for (let r = 0; r * 4 < scas.length; r++) {
    const comps = [0, 1, 2, 3].map((c) => scas[r * 4 + c]);
    if (!comps.some((e) => e && usesDecal(e))) continue;
    const parts = comps.map((e, c) => (e && usesDecal(e) ? emit(e, slot, constOf, lit) : `kh.m[${vecs.length + r}]`));
    if (parts.some((p) => p === null)) { rowsLeft++; continue; }
    rows[vecs.length + r] = `vec4<f32>(${parts.map((p, c) => `(${p}).${C[c]}`).join(', ')})`;
  }
  if (!Object.keys(rows).length) continue;
  // the block: drop the translator's own row lets, put ours first, and read every overridden row through its let
  let block = wgsl.slice(at, end);
  const head = block.indexOf('fn kh_material(i: KhIn) -> KhOut {\n'); if (head < 0) continue;
  const bodyAt = head + 'fn kh_material(i: KhIn) -> KhOut {\n'.length;
  let body = block.slice(bodyAt);
  body = body.replace(/^    let m(\d+) = .*\/\/ \(ZA decal\).*\n/gm, ''); // idempotent: a block this pass already rewrote
  body = body.replace(/^    let m(\d+) = vec4<f32>\(.*\);\n/gm, (all, n) => (rows[n] ? '' : all));
  const kept = new Set([...body.matchAll(/^    let m(\d+) = /gm)].map((m) => m[1]));
  for (const n of Object.keys(rows)) {
    if (kept.has(n)) continue;
    body = body.replace(new RegExp(`kh\\.m\\[${n}\\]`, 'g'), `m${n}`);
  }
  const lets = Object.entries(rows).map(([n, w]) => `    let m${n} = ${w}; // (ZA decal) the preshader over the emitter's parameters\n`).join('');
  block = block.slice(0, bodyAt) + lets + body;
  wgsl = wgsl.slice(0, at) + block + wgsl.slice(end);
  patched++; rowsDone += Object.keys(rows).length;
  report.push(`${owner} (S${o.id}): rows ${Object.keys(rows).join(', ')}`);
}
// the zero-guarded division of the translator's evaluator (|b| < 1e-5 -> 0)
const helper = '// (ZA decal) division as the preshader evaluator does it: 0 where the divisor is (near) zero\nfn kh_zdiv(a: vec4<f32>, b: vec4<f32>) -> vec4<f32> {\n    return select(a / b, vec4<f32>(0.0), abs(b) < vec4<f32>(1.0e-5));\n}\n';
if (!wgsl.includes('fn kh_zdiv(')) {
  const first = wgsl.indexOf('\n#ifdef ');
  wgsl = first < 0 ? wgsl + '\n' + helper : wgsl.slice(0, first + 1) + helper + wgsl.slice(first + 1);
}
fs.mkdirSync(outDir, { recursive: true });
fs.writeFileSync(path.join(outDir, FILE), wgsl);
console.log(`decal_dyn: ${patched} decal owner(s), ${rowsDone} row(s) per pixel, ${rowsLeft} row(s) left at rest`);
if (REPORT) console.log(report.join('\n'));
