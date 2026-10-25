// VFX pack for the player character Sora: every Cascade particle system his weapon / attacks / states use,
// exported from the decoded cooked packages into a neutral, documented JSON description, with the materials,
// textures (PNG) and static meshes (glb) the systems reference.
//
//   node tools/sora_vfx_pack.js                         build build/sora_pack/vfx/ from (effect list ∪ seed set)
//   node tools/sora_vfx_pack.js --list <file>           effect list (default build/sora_tmp/vfx/fxinv/effect_list.json):
//                                                       [{ effect: "/Game/...", package, exists, used_by, class }]
//   node tools/sora_vfx_pack.js --effect /Game/A/B ...  only the named systems (repeatable; with --out <dir> for trials)
//   node tools/sora_vfx_pack.js --dir Effects/wep/w_so010/par     every ParticleSystem package of a Content folder
//   node tools/sora_vfx_pack.js --no-seed               do not add the seed set to the list
//   node tools/sora_vfx_pack.js --out <dir>             write somewhere else than build/sora_pack/vfx
//   node tools/sora_vfx_pack.js --check                 validate the pack on disk (hashes, PNG read-back, gltf-validator)
//   node tools/sora_vfx_pack.js --defaults              regenerate audit/sora_port/vfx_class_defaults.json (class
//                                                       default values from the class constructors in the executable)
//   node tools/sora_vfx_pack.js --census [--game]       module / distribution / feature census (runs
//                                                       build/sora_tmp/vfx/cascade/census.js with the same effect set)
//
// Inputs (nothing under assets/ or assets_converted/ is modified):
//   assets_converted/packages/...   decoded packages: ParticleSystem, Material, MaterialInstanceConstant, MaterialFunction
//   assets_converted/textures/...   .dds + .texture.json per texture (decoded by tools/lib/bcdec.js)
//   assets_converted/meshes/...     .glb per StaticMesh (tools/mesh_convert.js); LOD 0 is re-exported in pack conventions
//   assets/...                      only hashed (sha256 of the cooked files each item comes from)
//   audit/sora_port/vfx_class_defaults.json   class defaults (written by --defaults; generated on first use)
//   build/sora_tmp/vfx/fxsys/sqex_modules.json   optional: semantics of the Square Enix modules (labels only)
// Outputs (build/sora_pack/vfx/, local build output, never distributed):
//   manifest.json            format kh3-sora-vfx/1: conventions, schema of every JSON field, effect list, census
//   effects/<name>.json      one per ParticleSystem (format kh3-sora-vfx-effect/1)
//   materials/<name>.json    one per referenced Material / MaterialInstanceConstant (format kh3-sora-vfx-material/1)
//   textures/<name>.png      every texture the materials reference (top mip, RGBA 8 bit)
//   meshes/<name>.glb        every static mesh a mesh emitter uses (LOD 0, pack conventions)
//   triggers.json            trigger -> effect table (rows of sheets/port/sora-effects.json + effect_file), when that sheet exists
//   sheet_rows.json          rows for sheets/port/sora-assets.json (merged by the coordinator's hook, not by this tool)
//   validation.json          written by the build and by --check
// The output is deterministic: the same inputs give byte-identical files. Semantics and schema:
// build/sora_tmp/vfx/cascade/spec.md and manifest.json "schema".
'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const bc = require('./lib/bcdec');

const VERSION = '1.2.0';
// (XW-d) pack the pieces of TypeDataSQEX_FragmentMesh emitters too (see exportEffect)
const FRAGMENTS = process.argv.includes('--fragments');
const ROOT = path.resolve(__dirname, '..');
const CONTENT = 'KINGDOM HEARTS III/Content';
const PKG = path.join(ROOT, 'assets_converted', 'packages', CONTENT);
const TEX = path.join(ROOT, 'assets_converted', 'textures', CONTENT);
const MESH = path.join(ROOT, 'assets_converted', 'meshes', CONTENT);
const COOKED = path.join(ROOT, 'assets', CONTENT);
const DEFAULTS_FILE = path.join(ROOT, 'audit', 'sora_port', 'vfx_class_defaults.json');
const SQEX_FILE = path.join(ROOT, 'build', 'sora_tmp', 'vfx', 'fxsys', 'sqex_modules.json');
const ACTOR_MESH_BLUEPRINTS = ['Blueprints/Projectile/MagicBlizzard/TresSplineMeshBlizzard'];
const DEFAULT_LIST = path.join(ROOT, 'build', 'sora_tmp', 'vfx', 'fxinv', 'effect_list.json');
const CENSUS_JS = path.join(ROOT, 'build', 'sora_tmp', 'vfx', 'cascade', 'census.js');
const SEED_DIR = 'Effects/wep/w_so010/par';
// 1.2.0: the Water family's burn actors (the ParticleComp template of TresProj_Wat*Burn: vm_wat_han010 / 040 / 060 / 080,
// docs/sora-vfx-spec.md 7.5), which no row of the effect list names.
const SEED_BLUEPRINTS = ['Blueprints/Player/p_ex001/w_so010_Wep', 'Blueprints/Player/p_ex001/w_so_KeybladeBase',
  ...['WaterBurn', 'WataraBurn', 'WatagaBurn', 'WataganBurn', 'WaterBZBurn', 'WataraBZBurn', 'WatagaBZBurn', 'WataganBZBurn'].map((n) => 'Blueprints/Projectile/MagicWater/TresProj_' + n)];
const CURVE_SAMPLES = 33; // samples of an evaluated curve table (uniform over its domain, both ends included)

const sha256 = (b) => crypto.createHash('sha256').update(b).digest('hex');
const readJson = (f) => JSON.parse(fs.readFileSync(f, 'utf8'));
const rel = (f) => path.relative(ROOT, f).replace(/\\/g, '/');
// JSON text: objects one member per line, arrays of numbers / small flat objects on one line (deterministic)
function stringify(v, ind = '') {
  if (v === null || typeof v !== 'object') return JSON.stringify(v === undefined ? null : v);
  const flat = (x) => x === null || typeof x !== 'object' || (Array.isArray(x) && x.every((y) => y === null || typeof y !== 'object'));
  if (Array.isArray(v)) {
    if (!v.length) return '[]';
    if (v.every(flat)) return JSON.stringify(v);
    return '[\n' + v.map((x) => ind + ' ' + stringify(x, ind + ' ')).join(',\n') + '\n' + ind + ']';
  }
  const keys = Object.keys(v).filter((k) => v[k] !== undefined);
  if (!keys.length) return '{}';
  if (keys.every((k) => flat(v[k]))) { const t = JSON.stringify(v); if (t.length <= 240) return t; }
  return '{\n' + keys.map((k) => ind + ' ' + JSON.stringify(k) + ': ' + stringify(v[k], ind + ' ')).join(',\n') + '\n' + ind + '}';
}
const sig = (x) => (Number.isFinite(x) ? Number(x.toPrecision(9)) : x);

// ------------------------------------------------------------------------------------------------ packages
const dirCache = new Map();
function realName(dir, base) { // the file name with the case it has on disk (object paths are case-insensitive)
  let m = dirCache.get(dir);
  if (!m) { m = new Map(); try { for (const f of fs.readdirSync(dir)) m.set(f.toLowerCase(), f); } catch (e) { /* missing folder */ } dirCache.set(dir, m); }
  return m.get(base.toLowerCase()) || null;
}
function gamePathToContent(p) { // "/Game/A/B.B" or "/Game/A/B" -> "A/B" with the case of the files on disk; null when there is no such package
  const m = /^\/Game\/(.+?)(?:\.[^./]+)?$/.exec(p);
  if (!m) return null;
  const parts = m[1].split('/');
  let dir = PKG; const out = [];
  for (let i = 0; i < parts.length; i++) {
    const last = i === parts.length - 1;
    const r = realName(dir, last ? parts[i] + '.uasset.json' : parts[i]);
    if (!r) return null;
    out.push(last ? r.slice(0, -'.uasset.json'.length) : r); dir = path.join(dir, r);
  }
  return out.join('/');
}
// the decoder writes a float JSON cannot hold (-0, NaN, +-Inf) as { f32bits }: turned back into the number here (-0 prints as 0, non-finite as null)
const F32 = (k, v) => { if (v && typeof v === 'object' && typeof v.f32bits === 'number' && Object.keys(v).length === 1) { const b = Buffer.alloc(4); b.writeUInt32LE(v.f32bits >>> 0); return b.readFloatLE(0); } return v; };
const pkgCache = new Map();
let pkgCacheBytes = 0;
function loadPkg(content) {
  let d = pkgCache.get(content);
  if (d) return d;
  const f = path.join(PKG, content + '.uasset.json');
  const text = fs.readFileSync(f, 'utf8');
  d = JSON.parse(text, F32); d._content = content; d._bytes = text.length;
  pkgCache.set(content, d); pkgCacheBytes += text.length;
  while (pkgCacheBytes > 600e6 && pkgCache.size > 1) { const k = pkgCache.keys().next().value; pkgCacheBytes -= pkgCache.get(k)._bytes; pkgCache.delete(k); }
  return d;
}
function importRef(d, i) { // -> { path: "/Game/A/B" | "/Script/Engine", object: "B" (outer chain joined by '.'), class }
  let im = d.imports[-i - 1]; const names = [im.objectName]; const cls = im.className;
  while (im.outerIndex) { im = d.imports[-im.outerIndex - 1]; names.unshift(im.objectName); }
  return { path: names[0], object: names.slice(1).join('.'), class: cls };
}
function refString(d, i) { // object reference as text: null, "/Game/A/B.B" (import) or "export:<name>" (same package)
  if (!i) return null;
  if (i < 0) { const r = importRef(d, i); return r.object ? r.path + '.' + r.object : r.path; }
  return 'export:' + d.exports[i - 1].objectName;
}
const propMap = (props) => { const o = {}; for (const p of props || []) o[p.name + (p.arrayIndex ? `[${p.arrayIndex}]` : '')] = p; return o; };
const propsOf = (e) => propMap(e.data && e.data.props);
const assetId = (content) => `${CONTENT}/${content}.uasset`;
const hashCache = new Map();
function sourceFiles(content) { // sha256 of the cooked files of a package (assets/...); falls back to the decoded JSON
  if (hashCache.has(content)) return hashCache.get(content);
  const out = {};
  for (const ext of ['uasset', 'uexp', 'ubulk']) {
    const f = path.join(COOKED, content + '.' + ext);
    if (fs.existsSync(f)) out[ext] = sha256(fs.readFileSync(f));
  }
  if (!out.uexp && !out.uasset) out.decoded_json = sha256(fs.readFileSync(path.join(PKG, content + '.uasset.json')));
  hashCache.set(content, out);
  return out;
}
const sourceSha = (content) => { const s = sourceFiles(content); return s.uexp || s.uasset || s.decoded_json; };

// ------------------------------------------------------------------------------------------------ class defaults
let DEFAULTS = null;
function loadDefaults(regen) {
  if (DEFAULTS && !regen) return DEFAULTS;
  if (regen || !fs.existsSync(DEFAULTS_FILE)) {
    const cd = require('./lib/vfx_class_defaults');
    const r = cd.build(cd.allParticleClasses());
    fs.mkdirSync(path.dirname(DEFAULTS_FILE), { recursive: true });
    fs.writeFileSync(DEFAULTS_FILE, JSON.stringify(r, null, 1) + '\n');
  }
  DEFAULTS = readJson(DEFAULTS_FILE);
  return DEFAULTS;
}
function defValue(v) { // record of the defaults file -> plain value, or undefined when it is not determined
  if (!v) return undefined;
  if (v.members && /^StructProperty:Box(2D)?$/.test(v.type || '')) { const mn = defValue(v.members.Min), mx = defValue(v.members.Max), iv = defValue(v.members.IsValid); return mn === undefined || mx === undefined || iv === undefined ? undefined : { min: mn, max: mx, isValid: iv }; }
  if (v.members) { const o = {}; for (const [k, m] of Object.entries(v.members)) { const x = defValue(m); if (x !== undefined) o[k] = x; } return o; }
  if (v.elements) { const a = v.elements.map(defValue); return a.some((x) => x === undefined) ? undefined : a; }
  if (v.value === null && v.reason) return undefined;
  return typeof v.value === 'number' ? sig(v.value) : v.value;
}
function classDefaults(cls) { // -> { known: {prop: value}, unknown: [prop], flags } of the class default object
  const D = loadDefaults().classes[cls];
  const known = {}, unknown = [];
  if (!D || D.fail) return { known, unknown, missing: true };
  for (const [k, v] of Object.entries(D.props)) { const x = defValue(v); if (x === undefined) unknown.push(k); else known[k] = x; }
  return { known, unknown };
}
let SQEX = undefined;
function sqexInfo(cls) { // optional semantics of a Square Enix class, from the fxsys strand
  if (SQEX === undefined) {
    SQEX = null;
    if (fs.existsSync(SQEX_FILE)) { try { const j = readJson(SQEX_FILE); SQEX = new Map(); const add = (k, v) => SQEX.set(String(k).replace(/^U(?=Particle)/, ''), v);
      if (Array.isArray(j)) for (const r of j) add(r.class || r.id || r.name, r); else for (const [k, v] of Object.entries(j.modules || j.classes || j)) { if (Array.isArray(j.modules || j.classes)) add(v.class || v.id || v.name, v); else add(k, v); } } catch (e) { SQEX = null; } }
  }
  const r = SQEX && SQEX.get(cls);
  if (!r) return null;
  const s = typeof r === 'string' ? r : r.semantics || r.summary || r.note || r.description || null;
  return { semantics: typeof s === 'string' ? s : s ? JSON.stringify(s) : null, record: r };
}

// ------------------------------------------------------------------------------------------------ distributions
// Stock UE 4.17 semantics implemented here (Distributions.cpp, InterpCurve.h); see spec.md "Distributions".
const CUBIC = (p0, t0, p1, t1, a) => { const a2 = a * a, a3 = a2 * a; return (2 * a3 - 3 * a2 + 1) * p0 + (a3 - 2 * a2 + a) * t0 + (a3 - a2) * t1 + (-2 * a3 + 3 * a2) * p1; };
function evalCurve(points, t) { // FInterpCurve<T>::Eval; points: [{ t, v[], arrive[], leave[], mode }] -> number[]
  const n = points.length;
  if (!n) return null;
  if (n === 1 || t < points[0].t) return points[0].v.slice();
  if (t >= points[n - 1].t) return points[n - 1].v.slice();
  let i = 0; while (i + 1 < n && points[i + 1].t <= t) i++;
  const a = points[i], b = points[i + 1], diff = b.t - a.t;
  if (!(diff > 0) || a.mode === 'CIM_Constant') return a.v.slice();
  const al = (t - a.t) / diff;
  if (a.mode === 'CIM_Linear') return a.v.map((x, k) => x + (b.v[k] - x) * al);
  return a.v.map((x, k) => CUBIC(x, a.leave[k] * diff, b.v[k], b.arrive[k] * diff, al));
}
const vec3 = (v) => (v ? [v.x, v.y, v.z] : [0, 0, 0]);
function lockAxes(v, lock) { // EDistributionVectorLockFlags applied to a value: the locked axes copy the first one
  const o = v.slice();
  if (lock === 'EDVLF_XY') o[1] = o[0]; else if (lock === 'EDVLF_XZ') o[2] = o[0]; else if (lock === 'EDVLF_YZ') o[2] = o[1]; else if (lock === 'EDVLF_XYZ') { o[1] = o[0]; o[2] = o[0]; }
  return o;
}
function mirrorMin(max, min, flags) { // EDistributionVectorMirrorFlags: the minimum actually used per axis
  return max.map((m, k) => (flags[k] === 'EDVMF_Same' ? m : flags[k] === 'EDVMF_Mirror' ? -m : min[k]));
}
function curvePoints(curveProp, kind) { // InterpCurve{Float,Vector,Vector2D,TwoVectors} -> points
  const cp = propMap(curveProp ? curveProp.value : []);
  const items = cp.Points ? cp.Points.value.items : [];
  const conv = (x) => (kind === 'float' ? [x || 0] : kind === 'vector' ? vec3(x) : kind === 'vector2d' ? [x.x, x.y] : [...vec3(x.v1), ...vec3(x.v2)]);
  const pts = items.map((it) => { const p = propMap(it); return { t: p.InVal ? p.InVal.value : 0, v: conv(p.OutVal && p.OutVal.value), arrive: conv(p.ArriveTangent && p.ArriveTangent.value), leave: conv(p.LeaveTangent && p.LeaveTangent.value), mode: p.InterpMode ? p.InterpMode.value : 'CIM_Linear' }; });
  return { points: pts, looped: cp.bIsLooped ? !!cp.bIsLooped.value : false, loop_key_offset: cp.LoopKeyOffset ? cp.LoopKeyOffset.value : 0 };
}
function exportDistribution(d, idx, rawStruct) {
  // rawStruct: property list of the FRawDistributionFloat / FRawDistributionVector that holds the reference (may be null)
  const out = {};
  const rs = propMap(rawStruct || []);
  if (!idx && rs.Distribution) idx = rs.Distribution.value;
  const e = idx > 0 ? d.exports[idx - 1] : null;
  if (!e) { out.class = null; out.type = 'none'; out.note = 'no distribution object'; return finishRaw(out, rs); }
  const cls = e.class, p = propsOf(e), def = classDefaults(cls).known;
  const get = (k) => (p[k] ? p[k].value : def[k]);
  out.class = cls; out.object = e.objectName;
  const vector = /Vector/.test(cls);
  out.value_kind = vector ? 'vector' : 'float';
  const serialized = Object.keys(p).filter((k) => !/^b(IsDirty|BakedDataSuccesfully|CanBeBaked)$/.test(k));
  const lock = vector ? get('LockedAxes') || 'EDVLF_None' : null;
  const mirrorOf = () => [0, 1, 2].map((k) => (p[`MirrorFlags${k ? `[${k}]` : ''}`] ? p[`MirrorFlags${k ? `[${k}]` : ''}`].value : (def.MirrorFlags || [])[k] || 'EDVMF_Different'));
  let ev = null;
  const domainOf = (pts) => { let a = 0, b = 1; for (const q of pts) { if (q.t < a) a = q.t; if (q.t > b) b = q.t; } return [a, b]; };
  const sample = (pts, fn) => { const [a, b] = domainOf(pts); const rows = []; for (let i = 0; i < CURVE_SAMPLES; i++) rows.push(fn(a + (b - a) * i / (CURVE_SAMPLES - 1))); return { domain: [a, b], samples: CURVE_SAMPLES, rows }; };
  const keyOut = (c, split) => c.points.map((q) => ({ t: q.t, interp: q.mode, ...(split ? { max: q.v.slice(0, 3), min: q.v.slice(3), arrive_max: q.arrive.slice(0, 3), arrive_min: q.arrive.slice(3), leave_max: q.leave.slice(0, 3), leave_min: q.leave.slice(3) } : { value: q.v, arrive_tangent: q.arrive, leave_tangent: q.leave }) }));
  switch (cls) {
    case 'DistributionFloatConstant': out.type = 'constant'; out.constant = get('Constant') || 0; ev = { time_invariant: true, value: [out.constant] }; break;
    case 'DistributionVectorConstant': out.type = 'constant'; out.constant = vec3(get('Constant')); out.locked_axes = lock; ev = { time_invariant: true, value: lockAxes(out.constant, lock) }; break;
    case 'DistributionFloatUniform': out.type = 'uniform'; out.min = get('Min') || 0; out.max = get('Max') || 0; ev = { time_invariant: true, min: [out.min], max: [out.max] }; break;
    case 'DistributionVectorUniform': {
      out.type = 'uniform'; out.min = vec3(get('Min')); out.max = vec3(get('Max')); out.locked_axes = lock; out.mirror_flags = mirrorOf(); out.use_extremes = !!get('bUseExtremes');
      ev = { time_invariant: true, min: mirrorMin(out.max, out.min, out.mirror_flags), max: out.max.slice() }; break;
    }
    case 'DistributionFloatConstantCurve': {
      out.type = 'constant_curve'; const c = curvePoints(p.ConstantCurve, 'float'); out.curve = { looped: c.looped, loop_key_offset: c.loop_key_offset, keys: keyOut(c) };
      const s = sample(c.points, (t) => evalCurve(c.points, t) || [0]); ev = { domain: s.domain, samples: s.samples, value: s.rows }; out._pts = c.points; break;
    }
    case 'DistributionVectorConstantCurve': {
      out.type = 'constant_curve'; const c = curvePoints(p.ConstantCurve, 'vector'); out.curve = { looped: c.looped, loop_key_offset: c.loop_key_offset, keys: keyOut(c) }; out.locked_axes = lock;
      const s = sample(c.points, (t) => lockAxes(evalCurve(c.points, t) || [0, 0, 0], lock)); ev = { domain: s.domain, samples: s.samples, value: s.rows }; out._pts = c.points; break;
    }
    case 'DistributionFloatUniformCurve': {
      out.type = 'uniform_curve'; const c = curvePoints(p.ConstantCurve, 'vector2d'); out.curve = { looped: c.looped, loop_key_offset: c.loop_key_offset, keys: c.points.map((q) => ({ t: q.t, interp: q.mode, max: [q.v[0]], min: [q.v[1]], arrive_max: [q.arrive[0]], arrive_min: [q.arrive[1]], leave_max: [q.leave[0]], leave_min: [q.leave[1]] })) };
      const s = sample(c.points, (t) => evalCurve(c.points, t) || [0, 0]); ev = { domain: s.domain, samples: s.samples, max: s.rows.map((r) => [r[0]]), min: s.rows.map((r) => [r[1]]) }; out._pts = c.points; break;
    }
    case 'DistributionVectorUniformCurve': {
      out.type = 'uniform_curve'; const c = curvePoints(p.ConstantCurve, 'twovectors'); out.curve = { looped: c.looped, loop_key_offset: c.loop_key_offset, keys: keyOut(c, true) };
      out.locked_axes = [0, 1].map((k) => (p[`LockedAxes${k ? '[1]' : ''}`] ? p[`LockedAxes${k ? '[1]' : ''}`].value : (Array.isArray(def.LockedAxes) ? def.LockedAxes[k] : def.LockedAxes) || 'EDVLF_None'));
      out.lock_axes = [!!get('bLockAxes1'), !!get('bLockAxes2')]; out.mirror_flags = mirrorOf(); out.use_extremes = !!get('bUseExtremes');
      const s = sample(c.points, (t) => evalCurve(c.points, t) || [0, 0, 0, 0, 0, 0]);
      ev = { domain: s.domain, samples: s.samples, max: s.rows.map((r) => r.slice(0, 3)), min: s.rows.map((r) => mirrorMin(r.slice(0, 3), r.slice(3), out.mirror_flags)) }; out._pts = c.points; break;
    }
    default: {
      if (/Parameter/.test(cls)) {
        out.type = 'parameter';
        out.parameter = { name: get('ParameterName') === undefined ? null : get('ParameterName'), param_mode: vector ? [0, 1, 2].map((k) => (p[`ParamModes${k ? `[${k}]` : ''}`] ? p[`ParamModes${k ? `[${k}]` : ''}`].value : (def.ParamModes || [])[k] || null)) : get('ParamMode') || null,
          min_input: vector ? vec3(get('MinInput')) : get('MinInput'), max_input: vector ? vec3(get('MaxInput')) : get('MaxInput'), min_output: vector ? vec3(get('MinOutput')) : get('MinOutput'), max_output: vector ? vec3(get('MaxOutput')) : get('MaxOutput'),
          constant: vector ? vec3(get('Constant')) : get('Constant') || 0 };
        out.note = 'instance parameter: the value comes from the component at run time; constant is used when the parameter is not set';
      } else { out.type = 'other'; out.note = cls.startsWith('SQEX') ? 'Square Enix distribution class: properties exported verbatim, not evaluated' : 'distribution class not interpreted: properties exported verbatim'; }
      out.properties = plainProps(d, e.data.props, {});
    }
  }
  if (ev) out.evaluated = roundEv(ev);
  out.serialized = serialized;
  return finishRaw(out, rs);
}
function roundEv(ev) { const r = (x) => (Array.isArray(x) ? x.map(r) : typeof x === 'number' ? sig(x) : x); const o = {}; for (const [k, v] of Object.entries(ev)) o[k] = k === 'domain' ? v : r(v); return o; }
function finishRaw(out, rs) {
  const raw = {};
  if (rs.MinValue) raw.min_value = rs.MinValue.value; if (rs.MaxValue) raw.max_value = rs.MaxValue.value;
  if (rs.MinValueVec) raw.min_value_vec = vec3(rs.MinValueVec.value); if (rs.MaxValueVec) raw.max_value_vec = vec3(rs.MaxValueVec.value);
  if (Object.keys(raw).length) out.raw_range = raw;
  if (rs.Table) {
    const t = propMap(rs.Table.value), g = (k) => (t[k] ? t[k].value : 0);
    out.lookup_table = { op: g('Op'), op_name: ['RDO_Uninitialized', 'RDO_None', 'RDO_Random', 'RDO_Extreme'][g('Op')] || null, entry_count: g('EntryCount'), entry_stride: g('EntryStride'), sub_entry_stride: g('SubEntryStride'), time_scale: g('TimeScale'), time_bias: g('TimeBias'), lock_flag: g('LockFlag'), values: t.Values ? t.Values.value : [] };
  }
  // the cooker's table against this tool's evaluation of the curve: 128 float32 samples over the key range, resampled
  // linearly to entry_count entries. A table that disagrees (stale bake) is still what the game samples.
  const T = out.lookup_table;
  if (out._pts && out._pts.length && out.type === 'constant_curve' && T && T.op === 1 && T.entry_count >= 2 && T.values.length === T.entry_count * T.entry_stride) {
    const fr = Math.fround, pts = out._pts, a = pts[0].t, b = pts[pts.length - 1].t, step = fr((b - a) / 127), n = T.entry_count, lockF = out.value_kind === 'vector' ? out.locked_axes : null;
    const full = []; for (let i = 0; i < 128; i++) { const v = evalCurve(pts, fr(a + fr(i * step))); full.push(lockF ? lockAxes(v, lockF) : v); }
    const scale = fr(127 / (n - 1)); let worst = 0;
    for (let i = 0; i < n; i++) { const pos = fr(i * scale), i0 = Math.min(Math.trunc(pos), 127), i1 = Math.min(i0 + 1, 127), al = pos - Math.trunc(pos);
      for (let c = 0; c < T.entry_stride; c++) { const v = n === 128 ? full[i][c] : full[i0][c] + al * (full[i1][c] - full[i0][c]); const w = T.values[i * T.entry_stride + c]; const e = Math.abs(v - w) / Math.max(1, Math.abs(w)); if (e > worst) worst = e; } }
    T.curve_check = { agrees: worst <= 1e-3, max_rel_diff: sig(worst) };
  }
  delete out._pts;
  return out;
}

// ------------------------------------------------------------------------------------------------ generic property conversion
function plainValue(d, p, ctx) {
  const v = p.value;
  switch (p.type) {
    case 'ObjectProperty': case 'ClassProperty': case 'WeakObjectProperty': case 'LazyObjectProperty': case 'InterfaceProperty': return objValue(d, v, ctx);
    case 'StructProperty':
      if (p.structName === 'RawDistributionFloat' || p.structName === 'RawDistributionVector') { if (ctx.dists) ctx.dists.push(p.name); return { distribution: exportDistribution(d, 0, v) }; }
      return Array.isArray(v) ? plainProps(d, v, ctx) : v;
    case 'ArrayProperty':
      if (p.innerType === 'StructProperty') return (v.items || []).map((it) => { if (!Array.isArray(it)) return it; if (v.structName === 'RawDistributionFloat' || v.structName === 'RawDistributionVector') return { distribution: exportDistribution(d, 0, it) }; return plainProps(d, it, ctx); });
      if (p.innerType === 'ObjectProperty') return v.map((x) => objValue(d, x, ctx));
      return v;
    case 'StrProperty': case 'TextProperty': return v && typeof v === 'object' && 'enc' in v ? v.s : v;
    default: return v;
  }
}
function objValue(d, i, ctx) {
  if (!i) return null;
  if (typeof i !== 'number') return i;
  if (i > 0) { const e = d.exports[i - 1]; if (/Distribution/.test(e.class) && !/^Particle/.test(e.class)) return { distribution: exportDistribution(d, i, null) }; return { export: e.objectName, class: e.class }; }
  const r = importRef(d, i); const s = r.object ? r.path + '.' + r.object : r.path;
  if (ctx.refs) ctx.refs.push({ ref: s, class: r.class });
  return s;
}
function plainProps(d, props, ctx) { const o = {}; for (const p of props || []) o[p.name + (p.arrayIndex ? `[${p.arrayIndex}]` : '')] = plainValue(d, p, ctx); return o; }
const BASE_FLAGS = ['bSpawnModule', 'bUpdateModule', 'bFinalUpdateModule', 'bFinalTickComponentModule', 'bParticleUpdateModule', 'bUpdateForGPUEmitter', 'bCurvesAsColor', 'b3DDrawMode', 'bSupported3DDrawMode', 'bEnabled', 'bEditable', 'LODDuplicate', 'bSupportsRandomSeed', 'bRequiresLoopingNotification', 'LODValidity', 'DebugSphareSizeScale', 'ModuleEditorColor'];
function objectParams(d, e, ctx) { // params of an export = class defaults overlaid with what the asset serializes
  const cd = classDefaults(e.class);
  const ser = plainProps(d, e.data.props, ctx);
  const params = {};
  for (const [k, v] of Object.entries(cd.known)) params[k] = v;
  for (const [k, v] of Object.entries(ser)) {
    const m = /^(.+)\[(\d+)\]$/.exec(k);
    if (m && Array.isArray(params[m[1]])) { params[m[1]] = params[m[1]].slice(); params[m[1]][+m[2]] = v; continue; }
    if (Array.isArray(params[k]) && params[k].length && !Array.isArray(v)) { params[k] = params[k].slice(); params[k][0] = v; continue; } // element 0 of a fixed array
    if (v && typeof v === 'object' && !Array.isArray(v) && !v.distribution && params[k] && typeof params[k] === 'object' && !Array.isArray(params[k])) params[k] = { ...params[k], ...v }; // struct: only the members that differ are serialized
    else params[k] = v;
  }
  return { params, serialized: Object.keys(ser), undetermined_defaults: cd.unknown.filter((k) => !(k in ser)), defaults_missing: !!cd.missing };
}
function moduleRecord(d, idx, ctx) {
  const e = d.exports[idx - 1];
  const o = objectParams(d, e, ctx);
  const P = o.params;
  const rec = { class: e.class, object: e.objectName, origin: /SQEX/i.test(e.class) ? 'sqex' : 'ue4', enabled: P.bEnabled !== false,
    flags: { spawn: !!P.bSpawnModule, update: !!P.bUpdateModule, final_update: !!P.bFinalUpdateModule, final_tick_component: !!P.bFinalTickComponentModule, particle_update: !!P.bParticleUpdateModule, update_for_gpu_emitter: !!P.bUpdateForGPUEmitter, draw_3d: !!P.b3DDrawMode, curves_as_color: !!P.bCurvesAsColor, supports_random_seed: !!P.bSupportsRandomSeed, requires_looping_notification: !!P.bRequiresLoopingNotification, lod_validity: P.LODValidity === undefined ? null : P.LODValidity } };
  if (rec.origin === 'sqex') { const s = sqexInfo(e.class); rec.semantics = s ? s.semantics : null; if (s && s.record && Array.isArray(s.record.properties)) { const pm = {}; for (const q of s.record.properties) if (q.meaning) pm[q.name] = q.meaning; if (Object.keys(pm).length) rec.property_meanings = pm; if (s.record.confidence) rec.semantics_confidence = s.record.confidence; } }
  const params = {}; for (const [k, v] of Object.entries(P)) if (!BASE_FLAGS.includes(k)) params[k] = v;
  rec.params = params;
  rec.serialized = o.serialized.filter((k) => !BASE_FLAGS.includes(k));
  if (o.undetermined_defaults.length) rec.undetermined_defaults = o.undetermined_defaults;
  if (o.defaults_missing) rec.defaults_missing = true;
  return rec;
}

// ------------------------------------------------------------------------------------------------ particle system -> effect JSON
const TYPEDATA_KIND = { ParticleModuleTypeDataMesh: 'mesh', ParticleModuleTypeDataRibbon: 'ribbon', ParticleModuleTypeDataBeam2: 'beam', ParticleModuleTypeDataAnimTrail: 'anim_trail', ParticleModuleTypeDataGpu: 'gpu_sprite', ParticleModuleTypeDataSQEX_Control: 'sqex_control', ParticleModuleTypeDataSQEX_TailPolygon: 'sqex_tail_polygon', ParticleModuleTypeDataSQEX_TailShape: 'sqex_tail_shape' };
const typedataKind = (cls) => (cls ? TYPEDATA_KIND[cls] || cls.replace(/^ParticleModuleTypeData/, '').toLowerCase() : 'sprite');
function findSystem(d) { return d.exports.find((e) => e.class === 'ParticleSystem' && !e.outerIndex) || d.exports.find((e) => e.class === 'ParticleSystem'); }
function exportEffect(content, name) {
  const d = loadPkg(content);
  const sys = findSystem(d);
  if (!sys) return null;
  const refs = [];
  const so = objectParams(d, sys, { refs: [] });
  const SP = so.params;
  const eff = { format: 'kh3-sora-vfx-effect/1', name, asset: '/Game/' + content, object: sys.objectName,
    source: { package: assetId(content), sha256: sourceFiles(content) }, units: 'UE source units: cm, seconds, degrees unless a module says otherwise; axes X forward, Y right, Z up (left-handed). See manifest.json conventions' };
  const sysDrop = ['Emitters', 'CurveEdSetup', 'ThumbnailImage', 'LODSettings'];
  eff.system = {
    update_mode: SP.SystemUpdateMode === undefined ? null : SP.SystemUpdateMode, update_time_fps: SP.UpdateTime_FPS === undefined ? null : SP.UpdateTime_FPS, update_time_delta: SP.UpdateTime_Delta === undefined ? null : SP.UpdateTime_Delta,
    warmup_time: SP.WarmupTime === undefined ? null : SP.WarmupTime, warmup_tick_rate: SP.WarmupTickRate === undefined ? null : SP.WarmupTickRate, delay: SP.Delay === undefined ? null : SP.Delay, delay_low: SP.DelayLow === undefined ? null : SP.DelayLow, use_delay_range: SP.bUseDelayRange === undefined ? null : SP.bUseDelayRange,
    seconds_before_inactive: SP.SecondsBeforeInactive === undefined ? null : SP.SecondsBeforeInactive, lod_method: SP.LODMethod === undefined ? null : SP.LODMethod, lod_distance_check_time: SP.LODDistanceCheckTime === undefined ? null : SP.LODDistanceCheckTime, lod_distances: SP.LODDistances || [],
    lod_settings: (propsOf(sys).LODSettings ? plainValue(d, propsOf(sys).LODSettings, {}) : []), use_fixed_relative_bounding_box: SP.bUseFixedRelativeBoundingBox === undefined ? null : SP.bUseFixedRelativeBoundingBox, fixed_relative_bounding_box: SP.FixedRelativeBoundingBox || null,
    orient_z_axis_toward_camera: SP.bOrientZAxisTowardCamera === undefined ? null : SP.bOrientZAxisTowardCamera,
    params: Object.fromEntries(Object.entries(SP).filter(([k]) => !sysDrop.includes(k))), serialized: so.serialized.filter((k) => !sysDrop.includes(k)), ...(so.undetermined_defaults.length ? { undetermined_defaults: so.undetermined_defaults } : {}),
  };
  const used = { materials: new Map(), meshes: new Set(), other_refs: new Set() };
  const unsupported = {};
  const uns = (k, what) => { (unsupported[k] || (unsupported[k] = { count: 0, what })).count++; };
  eff.emitters = [];
  const emitters = propsOf(sys).Emitters ? propsOf(sys).Emitters.value : [];
  emitters.forEach((ei, index) => {
    if (!ei || ei < 0) { eff.emitters.push(null); return; }
    const ee = d.exports[ei - 1], eo = objectParams(d, ee, {}), EP = eo.params;
    const lods = (propsOf(ee).LODLevels ? propsOf(ee).LODLevels.value : []).filter((x) => x > 0);
    const em = { index, name: EP.EmitterName === undefined || EP.EmitterName === 'None' ? ee.objectName : EP.EmitterName, object: ee.objectName, class: ee.class };
    em.render_mode = EP.EmitterRenderMode === undefined ? null : EP.EmitterRenderMode;
    em.params = Object.fromEntries(Object.entries(EP).filter(([k]) => !['LODLevels', 'EmitterName', 'EmitterEditorColor', 'bCollapsed', 'bIsSoloing'].includes(k)));
    em.serialized = eo.serialized.filter((k) => k !== 'LODLevels');
    if (!lods.length) { em.lod_count = 0; eff.emitters.push(em); return; }
    const lodInfo = (li) => { const le = d.exports[li - 1], lp = propsOf(le); return { e: le, p: lp, required: lp.RequiredModule ? lp.RequiredModule.value : 0, spawn: lp.SpawnModule ? lp.SpawnModule.value : 0, typedata: lp.TypeDataModule ? lp.TypeDataModule.value : 0, modules: lp.Modules ? lp.Modules.value : [], params: objectParams(d, le, {}).params }; };
    const L0 = lodInfo(lods[0]);
    em.lod_count = lods.length;
    em.enabled = L0.params.bEnabled !== false;
    em.peak_active_particles = L0.params.PeakActiveParticles === undefined ? null : L0.params.PeakActiveParticles;
    // ---- TypeData
    const ctx = { refs: [] };
    if (L0.typedata > 0) {
      const t = moduleRecord(d, L0.typedata, ctx);
      em.typedata = { kind: typedataKind(t.class), ...t };
      // (XW-d) `--fragments`: TypeDataSQEX_FragmentMesh's pieces (SubMeshLists) are packed as well (the Chandelier's
      // shatter hahen03 / ice flowers zmn0: kh_vfx draws them since XW-d); without the flag the output is as before
      if (t.class === 'ParticleModuleTypeDataMesh' || (FRAGMENTS && t.class === 'ParticleModuleTypeDataSQEX_FragmentMesh')) {
        const meshRef = t.params.Mesh; em.typedata.mesh = typeof meshRef === 'string' ? meshRef : null;
        if (em.typedata.mesh) used.meshes.add(em.typedata.mesh);
        // 1.2.0: with bUseSubMeshs the particles draw the meshes of SubMeshLists (`Mesh` is then a placeholder, usually the
        // axis gizmo tmd_xyz_00): every one of them is packed
        for (const sl of t.params.SubMeshLists || []) for (const m of (sl && sl.SubMeshList) || []) if (m && typeof m.SubMesh === 'string' && /^\/Game\//.test(m.SubMesh)) used.meshes.add(m.SubMesh);
      }
    } else em.typedata = { kind: 'sprite', class: null };
    const kind = em.typedata.kind;
    if (kind !== 'sprite' && kind !== 'mesh') uns('typedata:' + (em.typedata.class || kind), kind === 'gpu_sprite' ? 'GPU sprite emitter: EmitterInfo / ResourceData exported verbatim, simulation not described' : kind === 'anim_trail' ? 'anim trail: geometry comes from socket samples of the owning skeletal mesh at run time (notify-driven); parameters exported' : 'emitter type exported with all its properties; rendering rule not described by stock Cascade');
    // ---- Required
    const R = moduleRecord(d, L0.required, ctx), RP = R.params;
    const nz = (x) => (x === undefined ? null : x);
    em.required = {
      material: typeof RP.Material === 'string' ? RP.Material : null, emitter_blend_type: nz(RP.EmitterBlendType), screen_alignment: nz(RP.ScreenAlignment), sort_mode: nz(RP.SortMode),
      use_local_space: nz(RP.bUseLocalSpace), use_emitter_local_space: nz(RP.bUseEmitterLocalSpace), kill_on_deactivate: nz(RP.bKillOnDeactivate), kill_on_completed: nz(RP.bKillOnCompleted),
      duration: nz(RP.EmitterDuration), duration_low: nz(RP.EmitterDurationLow), duration_use_range: nz(RP.bEmitterDurationUseRange), duration_recalc_each_loop: nz(RP.bDurationRecalcEachLoop), loops: nz(RP.EmitterLoops),
      delay: nz(RP.EmitterDelay), delay_low: nz(RP.EmitterDelayLow), delay_use_range: nz(RP.bEmitterDelayUseRange), delay_first_loop_only: nz(RP.bDelayFirstLoopOnly),
      sqex_emitter_time: { start_time: nz(RP.EmitterStartTime), loop_time: nz(RP.EmitterLoopTime), loop_duration: nz(RP.EmitterLoopDuration), duration_time: nz(RP.EmitterDurationTime), legacy_loop_time: nz(RP.LoopTime), legacy_loop_duration: nz(RP.LoopDuration), use_legacy_emitter_time: nz(RP.bUseLegacyEmitterTime), is_loop_start_and_end: nz(RP.bIsLoopStartAndEnd) },
      sub_uv: { horizontal: nz(RP.SubImages_Horizontal), vertical: nz(RP.SubImages_Vertical), interpolation: nz(RP.InterpolationMethod), scale_uv: nz(RP.bScaleUV), random_image_time: nz(RP.RandomImageTime), random_image_changes: nz(RP.RandomImageChanges) },
      use_max_draw_count: nz(RP.bUseMaxDrawCount), max_draw_count: nz(RP.MaxDrawCount), emitter_origin: nz(RP.EmitterOrigin), emitter_rotation: nz(RP.EmitterRotation), tex_u_scale: nz(RP.TexU_Scale), tex_v_scale: nz(RP.TexV_Scale),
      spawn_rate: RP.SpawnRate && RP.SpawnRate.distribution ? RP.SpawnRate.distribution : null,
      enabled: R.enabled, object: R.object, params: Object.fromEntries(Object.entries(RP).filter(([k]) => k !== 'SpawnRate')), serialized: R.serialized, ...(R.undetermined_defaults ? { undetermined_defaults: R.undetermined_defaults } : {}),
    };
    // ---- Spawn
    if (L0.spawn > 0) {
      const S = moduleRecord(d, L0.spawn, ctx), Q = S.params;
      em.spawn = { object: S.object, enabled: S.enabled, process_spawn_rate: nz(Q.bProcessSpawnRate), process_burst_list: nz(Q.bProcessBurstList), apply_global_spawn_rate_scale: nz(Q.bApplyGlobalSpawnRateScale),
        rate: Q.Rate ? Q.Rate.distribution : null, rate_scale: Q.RateScale ? Q.RateScale.distribution : null, burst_method: nz(Q.ParticleBurstMethod),
        bursts: (Q.BurstList || []).map((b) => ({ count: b.Count, count_low: b.CountLow, time: b.Time })), burst_scale: Q.BurstScale ? Q.BurstScale.distribution : null,
        params: Object.fromEntries(Object.entries(Q).filter(([k]) => !['Rate', 'RateScale', 'BurstList', 'BurstScale'].includes(k))), serialized: S.serialized };
    } else em.spawn = null;
    // ---- modules, in the order of ParticleLODLevel.Modules (the order Cascade calls them in)
    em.modules = L0.modules.filter((x) => x > 0).map((mi) => moduleRecord(d, mi, ctx));
    for (const m of em.modules) {
      if (m.origin === 'sqex') uns('module:' + m.class, m.semantics ? 'Square Enix module: properties verbatim, semantics text from the code analysis' : 'Square Enix module: properties verbatim, no semantics available');
      else if (/Collision/.test(m.class)) uns('module:' + m.class, 'collision module: parameters exported; needs the host world for the queries');
      else if (/Event/.test(m.class)) uns('module:' + m.class, 'event module: parameters exported; event routing between emitters is the implementer\'s job');
      else if (/VectorField/.test(m.class)) uns('module:' + m.class, 'vector field: parameters exported, the field asset is not');
      else if (/Light/.test(m.class)) uns('module:' + m.class, 'particle lights: parameters exported; needs dynamic lights in the host');
      if (/_Seeded$/.test(m.class)) uns('seeded:' + m.class, 'seeded module: RandomSeedInfo exported; the engine random stream (FRandomStream) is needed to reproduce the exact sequence');
    }
    const scanDist = (x, where) => { if (!x || typeof x !== 'object') return; if (Array.isArray(x)) { x.forEach((y) => scanDist(y, where)); return; } if (x.distribution) { const t = x.distribution; if (t.type === 'other' || t.type === 'parameter') uns('distribution:' + t.class, t.note); if (t.lookup_table && t.lookup_table.curve_check && !t.lookup_table.curve_check.agrees) uns('distribution:lookup table differs from its curve', 'the baked lookup table (what the game samples) is not the resampled curve stored beside it; use lookup_table'); return; } for (const v of Object.values(x)) scanDist(v, where); };
    scanDist(em.modules); scanDist(em.spawn); scanDist(em.typedata); scanDist(em.required.spawn_rate ? { distribution: em.required.spawn_rate } : null);
    if (RP.CutoutTexture) uns('required:CutoutTexture', 'sub-UV cutout bounding geometry is not exported (it only reduces overdraw)');
    // ---- materials of this emitter
    em.materials = [];
    const addMat = (m, role) => { if (typeof m !== 'string' || !/^\/Game\//.test(m)) return; if (!em.materials.some((x) => x.material === m && x.role === role)) em.materials.push({ material: m, role }); if (!used.materials.has(m)) used.materials.set(m, new Set()); used.materials.get(m).add(role); };
    addMat(em.required.material, 'required');
    for (const m of em.modules) if (m.class === 'ParticleModuleMeshMaterial') for (const x of m.params.MeshMaterials || []) addMat(x, 'mesh_material_module');
    for (const r of ctx.refs) { if (/Material/.test(r.class)) addMat(r.ref, 'referenced'); else if (!/^\/Script\//.test(r.ref)) used.other_refs.add(r.ref + ' (' + r.class + ')'); }
    // ---- higher LODs: only what differs
    em.lods = lods.map((li, level) => {
      const L = level ? lodInfo(li) : L0;
      const row = { level, enabled: L.params.bEnabled !== false, object: L.e.objectName };
      if (level) {
        const diff = [];
        if (L.required !== L0.required) diff.push('RequiredModule'); if (L.spawn !== L0.spawn) diff.push('SpawnModule'); if (L.typedata !== L0.typedata) diff.push('TypeDataModule');
        const n = Math.max(L.modules.length, L0.modules.length);
        for (let k = 0; k < n; k++) if (L.modules[k] !== L0.modules[k]) diff.push(`Modules[${k}]:${L.modules[k] > 0 ? d.exports[L.modules[k] - 1].class : 'null'}`);
        row.same_objects_as_lod0 = !diff.length; if (diff.length) row.differs = diff;
      }
      return row;
    });
    if (em.lods.some((l) => l.level && (!l.same_objects_as_lod0 || l.enabled !== em.enabled))) uns('lod:differs', 'a higher LOD level uses other module objects than LOD 0; only LOD 0 is exported in full');
    eff.emitters.push(em);
  });
  for (const r of so.serialized) if (/ParticleSeAssets/.test(r)) uns('system:ParticleSeAssets', 'sound assets attached to the particle system (Square Enix): references exported in system.params');
  eff.references = { materials: [...used.materials.keys()].sort(), meshes: [...used.meshes].sort(), other: [...used.other_refs].sort() };
  eff.unsupported = Object.fromEntries(Object.keys(unsupported).sort().map((k) => [k, unsupported[k]]));
  eff._used = used;
  return eff;
}

// ------------------------------------------------------------------------------------------------ materials
const MATERIAL_INPUTS = ['EmissiveColor', 'Opacity', 'OpacityMask', 'BaseColor', 'Normal', 'Refraction', 'WorldPositionOffset', 'Metallic', 'Specular', 'Roughness', 'AmbientOcclusion', 'SubsurfaceColor', 'PixelDepthOffset', 'CustomizedUVs', 'CustomizedUVs[1]', 'CustomizedUVs[2]', 'CustomizedUVs[3]'];
const classOfExprName = (n) => (typeof n === 'string' ? n.replace(/_\d+$/, '') : null);
function uexprText(x) { // FMaterialUniformExpression tree -> compact text
  if (!x || typeof x !== 'object') return String(x);
  const t = String(x.type || '').replace(/^FMaterialUniformExpression/, '');
  const c = (v) => `(${[v.r, v.g, v.b, v.a].map(sig).join(',')})`;
  switch (t) {
    case 'Constant': return 'const' + c(x.value);
    case 'VectorParameter': return `vparam("${x.parameterName}"${x.defaultValue ? ',' + c(x.defaultValue) : ''})`;
    case 'ScalarParameter': return `sparam("${x.parameterName}",${sig(x.defaultValue)})`;
    case 'TextureParameter': return `texparam("${x.parameterName}",#${x.textureIndex})`;
    case 'Texture': return `tex(#${x.textureIndex})`;
    case 'Time': return 'time'; case 'RealTime': return 'realtime';
    case 'FoldedMath': return `${['add', 'sub', 'mul', 'div', 'dot'][x.op] || 'op' + x.op}(${uexprText(x.a)},${uexprText(x.b)})`;
    case 'AppendVector': return `append${x.numComponentsA}(${uexprText(x.a)},${uexprText(x.b)})`;
    case 'ComponentSwizzle': return `swizzle[${[x.indexR, x.indexG, x.indexB, x.indexA].slice(0, x.numElements).join('')}](${uexprText(x.x)})`;
    case 'Clamp': return `clamp(${uexprText(x.input)},${uexprText(x.min)},${uexprText(x.max)})`;
    case 'Min': case 'Max': case 'Fmod': return `${t.toLowerCase()}(${uexprText(x.a)},${uexprText(x.b)})`;
    case 'Sine': return `${x.bIsCosine || x.isCosine ? 'cos' : 'sin'}(${uexprText(x.x || x.expression)})`;
    case 'TrigMath': return `trig${x.op}(${uexprText(x.x)},${uexprText(x.y)})`;
    default: { const inner = x.x || x.expression || x.input; return `${t.toLowerCase()}(${inner ? uexprText(inner) : ''})`; }
  }
}
function exprNode(d, e) { // one material expression export -> compact record
  const p = propsOf(e), o = { class: e.class.replace(/^MaterialExpression/, '') };
  const inputs = {};
  const walk = (props, pre) => { for (const q of props) {
    if (q.type === 'StructProperty' && /Input$/.test(q.structName || '') && q.value && typeof q.value === 'object' && 'expressionName' in q.value) { if (q.value.expressionName && q.value.expressionName !== 'None') inputs[pre + q.name] = { from: q.value.expressionName, output: q.value.outputIndex }; }
    else if (q.type === 'ArrayProperty' && q.innerType === 'StructProperty') (q.value.items || []).forEach((it, i) => { if (Array.isArray(it)) walk(it, `${pre}${q.name}[${i}].`); });
  } };
  walk(e.data.props || [], '');
  if (Object.keys(inputs).length) o.inputs = inputs;
  if (p.ParameterName) o.parameter = p.ParameterName.value;
  if (p.DefaultValue) o.default = p.DefaultValue.value;
  if (p.Texture) o.texture = refString(d, p.Texture.value);
  if (p.SamplerType) o.sampler_type = p.SamplerType.value;
  if (p.MaterialFunction) o.function = refString(d, p.MaterialFunction.value);
  if (p.ParamNames) o.param_names = p.ParamNames.value.map((s) => (s && typeof s === 'object' ? s.s : s));
  if (p.Group) o.group = p.Group.value;
  for (const k of ['R', 'G', 'B', 'A', 'Constant', 'ConstA', 'ConstB', 'SpeedX', 'SpeedY', 'ConstCoordinate', 'CoordinateIndex', 'UTiling', 'VTiling', 'Exponent', 'FadeDistanceDefault', 'OpacityDefault', 'DefaultValue', 'InputName', 'OutputName', 'ParameterIndex', 'SliderMin', 'SliderMax', 'Value', 'DefaultR', 'DefaultG', 'DefaultB', 'DefaultA']) if (p[k] && !(k in o) && k !== 'DefaultValue') o[k] = plainValue(d, p[k], {});
  return o;
}
const fnCache = new Map();
function functionSummary(ref) { // a MaterialFunction package: its surviving expressions and the functions it calls
  if (fnCache.has(ref)) return fnCache.get(ref);
  const content = gamePathToContent(ref);
  const out = { function: ref, exists: !!content, expression_classes: {}, textures: [], parameters: [], calls: [], dynamic_parameters: [] };
  fnCache.set(ref, out);
  if (!content) return out;
  const d = loadPkg(content);
  out.source_package = assetId(content);
  for (const e of d.exports) {
    if (!/^MaterialExpression/.test(e.class)) continue;
    out.expression_classes[e.class] = (out.expression_classes[e.class] || 0) + 1;
    const n = exprNode(d, e);
    if (n.texture) out.textures.push({ expression: e.objectName, parameter: n.parameter || null, texture: n.texture, sampler_type: n.sampler_type || null });
    else if (n.parameter) out.parameters.push({ expression: e.objectName, class: n.class, parameter: n.parameter, default: n.default === undefined ? null : n.default });
    if (n.function && !out.calls.includes(n.function)) out.calls.push(n.function);
    if (n.param_names) out.dynamic_parameters.push(n.param_names);
  }
  out.calls.sort();
  return out;
}
function functionClosure(calls) { const seen = new Map(); const go = (f) => { if (seen.has(f)) return; const s = functionSummary(f); seen.set(f, s); s.calls.forEach(go); }; calls.forEach(go); return [...seen.values()]; }
const matCache = new Map();
function materialSummary(ref, light) { // light: census use (no graph), cached separately by the caller
  const key = ref + (light ? '|l' : '');
  if (matCache.has(key)) return matCache.get(key);
  const content = gamePathToContent(ref);
  if (!content) { const r = { asset: ref, exists: false }; matCache.set(key, r); return r; }
  // parent chain
  const chain = []; let cur = content, root = null; const overrides = []; const insts = [];
  for (let guard = 0; cur && guard < 32; guard++) {
    const d = loadPkg(cur);
    const e = d.exports.find((x) => (x.class === 'Material' || /MaterialInstance/.test(x.class)) && !x.outerIndex) || d.exports.find((x) => x.class === 'Material' || /MaterialInstance/.test(x.class));
    if (!e) { chain.push({ asset: '/Game/' + cur, class: null }); break; }
    chain.push({ asset: '/Game/' + cur, class: e.class });
    if (e.class === 'Material') { root = { d, e }; break; }
    insts.push({ d, e });
    const par = propsOf(e).Parent;
    const pr = par && par.value < 0 ? importRef(d, par.value) : null;
    cur = pr ? gamePathToContent(pr.path) : null;
    if (pr && !cur) chain.push({ asset: pr.path, class: null, missing: true });
  }
  const out = { format: 'kh3-sora-vfx-material/1', name: path.basename(content), asset: '/Game/' + content, class: chain[0].class, source: { package: assetId(content), sha256: light ? undefined : sourceFiles(content) },
    parent_chain: chain.map((c) => c.asset), root_material: root ? chain[chain.length - 1].asset : null };
  const RP = root ? objectParams(root.d, root.e, {}).params : {};
  let blend = RP.BlendMode === undefined ? null : RP.BlendMode, shading = RP.ShadingModel === undefined ? null : RP.ShadingModel, twoSided = RP.TwoSided === undefined ? null : RP.TwoSided, clip = RP.OpacityMaskClipValue === undefined ? null : RP.OpacityMaskClipValue;
  const stockDefaulted = [];
  if (root) { // the UMaterial constructor is not emulated (too many paths): stock UE 4.17 defaults for the render-state properties
    if (blend === null) { blend = 'BLEND_Opaque'; stockDefaulted.push('BlendMode'); } if (shading === null) { shading = 'MSM_DefaultLit'; stockDefaulted.push('ShadingModel'); } if (twoSided === null) { twoSided = false; stockDefaulted.push('TwoSided'); } if (clip === null) { clip = 0.3333; stockDefaulted.push('OpacityMaskClipValue'); }
  }
  out.not_serialized_stock_default = stockDefaulted;
  const ovr = [];
  for (let i = insts.length - 1; i >= 0; i--) { // root-most instance first, the asset itself last
    const ip = propsOf(insts[i].e); const b = ip.BasePropertyOverrides ? propMap(ip.BasePropertyOverrides.value) : {};
    const on = (k) => b['bOverride_' + k] && b['bOverride_' + k].value;
    if (on('BlendMode') && b.BlendMode) { blend = b.BlendMode.value; ovr.push('BlendMode@' + insts[i].e.objectName); }
    if (on('ShadingModel') && b.ShadingModel) { shading = b.ShadingModel.value; ovr.push('ShadingModel@' + insts[i].e.objectName); }
    if (on('TwoSided') && b.TwoSided) { twoSided = !!b.TwoSided.value; ovr.push('TwoSided@' + insts[i].e.objectName); }
    if (on('OpacityMaskClipValue') && b.OpacityMaskClipValue) { clip = b.OpacityMaskClipValue.value; ovr.push('OpacityMaskClipValue@' + insts[i].e.objectName); }
  }
  out.blend_mode = blend; out.shading_model = shading; out.two_sided = twoSided; out.opacity_mask_clip_value = clip; out.base_property_overrides = ovr;
  // parameters: root defaults, overridden by each instance from the root downwards
  const scalars = {}, vectors = {}, textures = {};
  const exprClasses = {}; const graph = {}; const calls = []; const dyn = []; const unnamedTex = [];
  if (root) {
    for (const e of root.d.exports) {
      if (!/^MaterialExpression/.test(e.class)) continue;
      exprClasses[e.class] = (exprClasses[e.class] || 0) + 1;
      const n = exprNode(root.d, e);
      if (!light) graph[e.objectName] = n;
      const from = 'root:' + root.e.objectName;
      if (n.texture !== undefined && n.parameter) textures[n.parameter] = { texture: n.texture, from, sampler_type: n.sampler_type || null, expression: e.objectName };
      else if (n.texture !== undefined) unnamedTex.push({ expression: e.objectName, texture: n.texture, sampler_type: n.sampler_type || null });
      else if (n.parameter && e.class === 'MaterialExpressionScalarParameter') scalars[n.parameter] = { value: n.default === undefined ? 0 : n.default, from };
      else if (n.parameter && e.class === 'MaterialExpressionVectorParameter') vectors[n.parameter] = { value: n.default === undefined ? null : n.default, from };
      else if (n.parameter) (out.other_parameters || (out.other_parameters = [])).push({ expression: e.objectName, class: n.class, parameter: n.parameter, default: n.default === undefined ? null : n.default });
      if (n.function && !calls.includes(n.function)) calls.push(n.function);
      if (n.param_names) dyn.push({ expression: e.objectName, names: n.param_names, default: n.default === undefined ? null : n.default });
    }
  }
  const fns = functionClosure(calls.sort());
  for (const f of fns) { // parameters and textures declared inside material functions belong to the material too
    for (const [k, v] of Object.entries(f.expression_classes)) exprClasses[k] = (exprClasses[k] || 0) + v;
    const from = 'function:' + f.function;
    for (const t of f.textures) { if (t.parameter) { if (!textures[t.parameter]) textures[t.parameter] = { texture: t.texture, from, sampler_type: t.sampler_type, expression: t.expression }; } else unnamedTex.push({ expression: t.expression, texture: t.texture, sampler_type: t.sampler_type, in_function: f.function }); }
    for (const q of f.parameters) { if (q.class === 'ScalarParameter') { if (!scalars[q.parameter]) scalars[q.parameter] = { value: q.default === null ? 0 : q.default, from }; } else if (q.class === 'VectorParameter') { if (!vectors[q.parameter]) vectors[q.parameter] = { value: q.default, from }; } }
    for (const n of f.dynamic_parameters) dyn.push({ in_function: f.function, names: n });
  }
  const instParams = { scalar: [], vector: [], texture: [] };
  for (let i = insts.length - 1; i >= 0; i--) {
    const { d, e } = insts[i], ip = propsOf(e), from = 'instance:' + e.objectName;
    const each = (prop, fn) => { for (const it of (ip[prop] ? ip[prop].value.items : [])) { const q = propMap(it); const n = q.ParameterName ? q.ParameterName.value : null; if (n !== null) fn(n, q.ParameterValue); } };
    each('ScalarParameterValues', (n, v) => { scalars[n] = { value: v ? v.value : 0, from, ...(scalars[n] ? { overrides: scalars[n].value } : { not_in_root: true }) }; if (i === 0) instParams.scalar.push(n); });
    each('VectorParameterValues', (n, v) => { vectors[n] = { value: v ? v.value : null, from, ...(vectors[n] ? { overrides: vectors[n].value } : { not_in_root: true }) }; if (i === 0) instParams.vector.push(n); });
    each('TextureParameterValues', (n, v) => { const prev = textures[n]; textures[n] = { texture: v ? refString(d, v.value) : null, from, sampler_type: prev ? prev.sampler_type : null, ...(prev ? { overrides: prev.texture } : { not_in_root: true }) }; if (i === 0) instParams.texture.push(n); });
    if (!light && e.data.native && (e.data.native.staticParameters || e.data.native.staticParameterSet)) { const sp = e.data.native.staticParameters || e.data.native.staticParameterSet; (out.static_parameters || (out.static_parameters = [])).push({ instance: e.objectName, static_switches: sp.staticSwitchParameters || [], component_masks: sp.staticComponentMaskParameters || [] }); }
  }
  out.instance_parameter_names = instParams;
  out.textures = textures; out.unnamed_textures = unnamedTex; out.scalars = scalars; out.vectors = vectors;
  out.dynamic_parameters = dyn;
  out.expression_classes = Object.fromEntries(Object.keys(exprClasses).sort().map((k) => [k, exprClasses[k]]));
  out.material_functions = fns.map((f) => f.function);
  if (light) { out.usage = Object.keys(RP).filter((k) => /^bUsedWith/.test(k) && RP[k] === true); matCache.set(key, out); return out; }
  // ---- what the decoded graph still tells (cooked materials keep only the expressions the client needs: parameters,
  //      texture samples, function calls, dynamic parameters; math nodes are stripped, their names survive in inputs)
  out.root_properties = Object.fromEntries(Object.entries(RP).filter(([k, v]) => !/^(Expressions|MaterialFunctionInfos|EditorX|EditorY|StateId|.*Guid.*|CachedExpressionData|ThumbnailInfo)$/.test(k) && !MATERIAL_INPUTS.includes(k) && (typeof v !== 'object' || v === null)));
  const rootProps = root ? propsOf(root.e) : {};
  const stripped = {};
  const noteStripped = (name) => { if (name && name !== 'None' && !graph[name]) { const c = classOfExprName(name); stripped[c] = (stripped[c] || 0) + 1; } };
  out.inputs = {};
  for (const k of MATERIAL_INPUTS) {
    const q = rootProps[k]; if (!q || !q.value || typeof q.value !== 'object') continue;
    const v = q.value, connected = !!(v.expressionName && v.expressionName !== 'None');
    const rec = { connected, ...(connected ? { expression: v.expressionName, expression_class: classOfExprName(v.expressionName), output_index: v.outputIndex, present_in_cooked_graph: !!graph[v.expressionName] } : {}), ...('useConstant' in v ? { use_constant: !!v.useConstant, constant: v.constant } : {}) };
    if (connected) { // everything reachable through the surviving nodes
      const seen = new Set(), reach = { textures: [], parameters: [], functions: [], dynamic_parameters: false, stripped_feeders: [] };
      const go = (name) => { if (!name || seen.has(name)) return; seen.add(name); const n = graph[name]; if (!n) { noteStripped(name); const c = classOfExprName(name); if (!reach.stripped_feeders.includes(c)) reach.stripped_feeders.push(c); return; }
        if (n.texture !== undefined) reach.textures.push(n.parameter ? 'param:' + n.parameter : n.texture); else if (n.parameter) reach.parameters.push(n.parameter);
        if (n.function) reach.functions.push(n.function.split('.').pop()); if (n.param_names) reach.dynamic_parameters = true;
        for (const i of Object.values(n.inputs || {})) go(i.from); };
      go(v.expressionName);
      rec.reachable = reach;
    }
    out.inputs[k] = rec;
  }
  for (const n of Object.values(graph)) for (const i of Object.values(n.inputs || {})) noteStripped(i.from);
  out.graph = { note: 'surviving expressions of the root material (cooking strips the math nodes; an input whose "from" names no node here pointed at a stripped node, its class is the name without the number)', nodes: graph };
  out.stripped_expression_classes_referenced = Object.fromEntries(Object.keys(stripped).sort().map((k) => [k, stripped[k]]));
  out.function_details = fns.map((f) => ({ function: f.function, exists: f.exists, expression_classes: f.expression_classes, textures: f.textures, parameters: f.parameters, calls: f.calls }));
  // uniform expressions of the compiled shader map: what the engine evaluates on the CPU each frame (panner offsets, parameter math)
  const sm = root && root.e.data.native && root.e.data.native.shaderMaps && root.e.data.native.shaderMaps[0] && root.e.data.native.shaderMaps[0].shaderMap;
  if (sm) {
    const ue = sm.uniformExpressionSet || {};
    out.shader_map = { friendly_name: sm.friendlyName, debug_description: sm.debugDescription, compilation_output_flags_unnamed: sm.compilationOutputFlags,
      vertex_factories: (sm.meshShaderMaps || []).map((m) => m.vertexFactoryType).sort(),
      uniform_vector_expressions: (ue.uniformVectorExpressions || []).map(uexprText), uniform_scalar_expressions: (ue.uniformScalarExpressions || []).map(uexprText),
      uniform_2d_texture_expressions: (ue.uniform2DTextureExpressions || ue.uniformTextureExpressions || []).map(uexprText), uniform_cube_texture_expressions: (ue.uniformCubeTextureExpressions || []).map(uexprText),
      uses_time: JSON.stringify(ue).includes('FMaterialUniformExpressionTime') || JSON.stringify(ue).includes('FMaterialUniformExpressionRealTime'),
      note: 'belongs to the root material; an instance with static parameters has its own shader map (not exported)' };
  }
  // ---- interpretation
  const cls = (re) => Object.keys(exprClasses).some((k) => re.test(k)) || Object.keys(stripped).some((k) => re.test(k));
  const fnNames = fns.map((f) => f.function.split('.').pop());
  out.summary = {
    emissive_from: out.inputs.EmissiveColor && out.inputs.EmissiveColor.reachable ? out.inputs.EmissiveColor.reachable : null,
    opacity_from: (out.inputs.Opacity && out.inputs.Opacity.reachable) || (out.inputs.OpacityMask && out.inputs.OpacityMask.reachable) || null,
    uses_dynamic_parameter: dyn.length > 0, dynamic_parameter_names: dyn.map((x) => x.names),
    uses_particle_color: cls(/ParticleColor|VertexColor/) ? true : null, uses_depth_fade: cls(/DepthFade/) || fnNames.some((n) => /depth.?fade|soft/i.test(n)) ? true : null, uses_fresnel: cls(/Fresnel/) || fnNames.some((n) => /fresnel/i.test(n)) ? true : null,
    uses_panner: cls(/Panner/) || fnNames.some((n) => /pan|scroll/i.test(n)) ? true : null, uses_sub_uv: cls(/ParticleSubUV|SubUV|FlipBook/i) || fnNames.some((n) => /subuv|flipbook/i.test(n)) ? true : null,
    uses_refraction: !!(out.inputs.Refraction && out.inputs.Refraction.connected), uses_world_position_offset: !!(out.inputs.WorldPositionOffset && out.inputs.WorldPositionOffset.connected),
    note: 'true = evidence found (surviving expression, stripped expression name, or material-function name); null = no evidence in the cooked data, which does not prove absence: particle colour, depth fade, panner and fresnel nodes are stripped by cooking unless something that survives points at them. The compiled shader (DXBC inside the package) is the full truth and is not decompiled here',
  };
  const INTERPRETED = /^MaterialExpression(ScalarParameter|VectorParameter|TextureSample|TextureSampleParameter2D|TextureObject|TextureObjectParameter|MaterialFunctionCall|DynamicParameter|StaticSwitchParameter|StaticBoolParameter|TextureSampleParameterCube|TextureSampleParameter)$/;
  out.expression_classes_not_interpreted = Object.keys(exprClasses).filter((k) => !INTERPRETED.test(k)).sort();
  matCache.set(key, out);
  return out;
}

// ------------------------------------------------------------------------------------------------ textures
function exportTexture(ref) { // -> { file, png, ... } or { error }
  const content = gamePathToContent(ref);
  const base = { asset: ref };
  if (!content) return { ...base, error: 'no such package' };
  let dds = path.join(TEX, content + '.dds'), side = path.join(TEX, content + '.texture.json');
  if (!fs.existsSync(dds)) { // named <package>.<export>.dds when the export name differs from the package name in case
    const dir = path.dirname(dds), b = path.basename(content).toLowerCase() + '.'; let all = []; try { all = fs.readdirSync(dir); } catch (e) { /* */ }
    const m = all.find((f) => f.toLowerCase().startsWith(b) && f.toLowerCase().endsWith('.dds'));
    if (m) { dds = path.join(dir, m); side = path.join(dir, m.slice(0, -4) + '.texture.json'); }
  }
  if (!fs.existsSync(dds) || !fs.existsSync(side)) return { ...base, content, error: 'no converted texture (' + rel(dds) + ')' };
  const sc = readJson(side);
  const sp = {}; for (const x of sc.props || []) sp[x.tag.name] = x.tag.type === 'BoolProperty' ? !!x.tag.boolVal : x.value;
  const buf = fs.readFileSync(dds);
  let t;
  const pf0 = sc.platforms && sc.platforms[0] ? sc.platforms[0] : null;
  if (pf0 && pf0.pixelFormatName === 'PF_FloatRGBA' && (pf0.numSlices || 1) === 1) { // half-float data (vertex-animation tables): no 8-bit image is meaningful, the top mip is carried raw
    const hd = require('./lib/texture').readDds(buf); const n = pf0.sizeX * pf0.sizeY * 8;
    return { ...base, content, class: sc.class, raw: Buffer.from(buf.subarray(hd.dataOffset, hd.dataOffset + n)), raw_encoding: 'top mip, row-major from the top-left, 4 x IEEE half float (R, G, B, A) little-endian per texel', width: pf0.sizeX, height: pf0.sizeY, pixel_format: 'PF_FloatRGBA', srgb: false,
      compression_settings: sp.CompressionSettings || 'TC_Default', lod_group: sp.LODGroup || null, address_x: sp.AddressX || 'TA_Wrap', address_y: sp.AddressY || 'TA_Wrap', filter: sp.Filter || 'TF_Default', decoder_mip_check: null, channels: 'RGBA half float', mip_count: pf0.mips ? pf0.mips.length : 1 };
  }
  try { t = bc.decodeDds(buf, 0); } catch (e) { return { ...base, content, class: sc.class, error: 'decode failed: ' + e.message, pixel_format: sc.platforms && sc.platforms[0] ? sc.platforms[0].pixelFormatName : null }; }
  let mipCheck = null;
  if (t.mipCount > 1 && t.width >= 8 && t.height >= 8) {
    try { const m1 = bc.decodeDds(buf, 1); let sum = 0;
      for (let y = 0; y < m1.height; y++) for (let x = 0; x < m1.width; x++) for (let c = 0; c < 4; c++) { const s = t.rgba[((y * 2) * t.width + x * 2) * 4 + c] + t.rgba[((y * 2) * t.width + x * 2 + 1) * 4 + c] + t.rgba[((y * 2 + 1) * t.width + x * 2) * 4 + c] + t.rgba[((y * 2 + 1) * t.width + x * 2 + 1) * 4 + c]; sum += Math.abs(s / 4 - m1.rgba[(y * m1.width + x) * 4 + c]); }
      mipCheck = +(sum / (m1.width * m1.height * 4)).toFixed(3); } catch (e) { mipCheck = null; }
  }
  const srgb = sp.SRGB === undefined ? true : !!sp.SRGB;
  const channels = { PF_BC4: 'one channel (R); copied to G and B, alpha 255', PF_G8: 'one channel (R); copied to G and B, alpha 255', PF_BC5: 'two channels (R, G); B 0, alpha 255', PF_DXT1: 'RGB (+1-bit alpha)', PF_DXT5: 'RGBA', PF_BC7: 'RGBA', PF_B8G8R8A8: 'RGBA' }[t.format] || t.format;
  let amin = 255, amax = 0, lmax = 0; for (let i = 0; i < t.rgba.length; i += 4) { const a = t.rgba[i + 3]; if (a < amin) amin = a; if (a > amax) amax = a; const l = Math.max(t.rgba[i], t.rgba[i + 1], t.rgba[i + 2]); if (l > lmax) lmax = l; }
  const png = bc.encodePng(t.width, t.height, Buffer.from(t.rgba), { alpha: true, srgb });
  return { ...base, content, class: sc.class, png, width: t.width, height: t.height, pixel_format: t.format, mip_count: t.mipCount, srgb, channels, alpha_range: [amin, amax], max_rgb: lmax,
    compression_settings: sp.CompressionSettings || 'TC_Default', lod_group: sp.LODGroup || null, address_x: sp.AddressX || 'TA_Wrap', address_y: sp.AddressY || 'TA_Wrap', filter: sp.Filter || 'TF_Default', decoder_mip_check: mipCheck };
}

// ------------------------------------------------------------------------------------------------ static meshes
function readGlb(file) {
  const b = fs.readFileSync(file);
  if (b.readUInt32LE(0) !== 0x46546c67) throw new Error('not a glb: ' + file);
  const jl = b.readUInt32LE(12); const json = JSON.parse(b.toString('utf8', 20, 20 + jl)); const bo = 20 + jl;
  return { json, bin: bo < b.length ? b.subarray(bo + 8, bo + 8 + b.readUInt32LE(bo)) : Buffer.alloc(0) };
}
const COMPS = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4 };
const CT = { 5120: [1, 'readInt8', 127], 5121: [1, 'readUInt8', 255], 5122: [2, 'readInt16LE', 32767], 5123: [2, 'readUInt16LE', 65535], 5125: [4, 'readUInt32LE', 1], 5126: [4, 'readFloatLE', 1] };
function readAccessor(g, i) {
  const a = g.json.accessors[i], bv = g.json.bufferViews[a.bufferView], n = COMPS[a.type], [sz, fn, norm] = CT[a.componentType];
  const stride = bv.byteStride || n * sz, base = (bv.byteOffset || 0) + (a.byteOffset || 0), out = new Array(a.count * n);
  for (let k = 0; k < a.count; k++) for (let c = 0; c < n; c++) { let v = g.bin[fn](base + k * stride + c * sz); if (a.normalized) v = Math.max(v / norm, -1); out[k * n + c] = v; }
  return { count: a.count, comps: n, data: out, componentType: a.componentType, normalized: !!a.normalized };
}
function meshGlbPath(ref) { // "/Game/A/B.B" -> converted glb (named <package>.glb, or <package>.<export>.glb when the names differ in case)
  const content = gamePathToContent(ref); if (!content) return null;
  const dir = path.join(MESH, path.dirname(content)), base = path.basename(content);
  let m = realName(dir, base + '.glb');
  if (!m) { let all = []; try { all = fs.readdirSync(dir); } catch (e) { /* */ } m = all.find((f) => f.toLowerCase().startsWith(base.toLowerCase() + '.') && f.toLowerCase().endsWith('.glb')) || null; }
  return m ? { content, file: path.join(dir, m) } : { content, file: null };
}
function exportMesh(ref) {
  const loc = meshGlbPath(ref);
  if (!loc || !loc.file) return { asset: ref, error: 'no converted mesh' };
  const g = readGlb(loc.file), J = g.json;
  const lodNode = J.scenes[J.scene || 0].extras && J.scenes[J.scene || 0].extras.ue_lod_nodes ? J.scenes[J.scene || 0].extras.ue_lod_nodes[0] : J.nodes.findIndex((n) => n.mesh !== undefined);
  const mesh = J.meshes[J.nodes[lodNode].mesh];
  const chunks = []; let off = 0;
  const out = { asset: { version: '2.0', generator: `kh3-decomp tools/sora_vfx_pack.js ${VERSION}` }, scene: 0, scenes: [{ nodes: [0] }], nodes: [{ name: path.basename(loc.content), mesh: 0 }], buffers: [{ byteLength: 0 }], bufferViews: [], accessors: [], materials: [], meshes: [{ name: path.basename(loc.content), primitives: [] }],
    extras: { ue_asset: ref, ue_package: assetId(loc.content), lod: 0, source_lods: (J.scenes[J.scene || 0].extras && J.scenes[J.scene || 0].extras.ue_lod_nodes || [0]).length, conventions: 'pack conventions (metres, +Y up, right-handed): position (x,y,z)_UE cm -> (x,z,y) * 0.01, normals/tangents (x,y,z) -> (x,z,y), tangent w negated, triangles wound counter-clockwise' } };
  const push = (buf, target) => { const pad = (4 - (buf.length % 4)) % 4; out.bufferViews.push({ buffer: 0, byteOffset: off, byteLength: buf.length, ...(target ? { target } : {}) }); chunks.push(buf, Buffer.alloc(pad)); off += buf.length + pad; return out.bufferViews.length - 1; };
  const f32buf = (arr) => { const b = Buffer.alloc(arr.length * 4); arr.forEach((v, i) => b.writeFloatLE(v, i * 4)); return b; };
  const cache = new Map();
  const attr = (srcIdx, name) => {
    const key = srcIdx + '|' + name; if (cache.has(key)) return cache.get(key);
    const a = readAccessor(g, srcIdx); let acc;
    if (name === 'POSITION') { const v = new Array(a.data.length); const mn = [Infinity, Infinity, Infinity], mx = [-Infinity, -Infinity, -Infinity];
      for (let k = 0; k < a.count; k++) { const p = [Math.fround(a.data[k * 3] * 0.01), Math.fround(a.data[k * 3 + 2] * 0.01), Math.fround(a.data[k * 3 + 1] * 0.01)]; for (let c = 0; c < 3; c++) { v[k * 3 + c] = p[c]; if (p[c] < mn[c]) mn[c] = p[c]; if (p[c] > mx[c]) mx[c] = p[c]; } }
      acc = { bufferView: push(f32buf(v), 34962), componentType: 5126, type: 'VEC3', count: a.count, min: mn, max: mx };
    } else if (name === 'NORMAL') { const v = new Array(a.data.length);
      for (let k = 0; k < a.count; k++) { let x = a.data[k * 3], y = a.data[k * 3 + 2], z = a.data[k * 3 + 1]; const l = Math.hypot(x, y, z); if (l > 1e-6) { x /= l; y /= l; z /= l; } else { x = 0; y = 1; z = 0; } v[k * 3] = x; v[k * 3 + 1] = y; v[k * 3 + 2] = z; }
      acc = { bufferView: push(f32buf(v), 34962), componentType: 5126, type: 'VEC3', count: a.count };
    } else if (name === 'TANGENT') { const v = new Array(a.count * 4);
      for (let k = 0; k < a.count; k++) { let x = a.data[k * 4], y = a.data[k * 4 + 2], z = a.data[k * 4 + 1]; const l = Math.hypot(x, y, z); if (l > 1e-6) { x /= l; y /= l; z /= l; } else { x = 1; y = 0; z = 0; } v[k * 4] = x; v[k * 4 + 1] = y; v[k * 4 + 2] = z; v[k * 4 + 3] = a.data[k * 4 + 3] < 0 ? 1 : -1; }
      acc = { bufferView: push(f32buf(v), 34962), componentType: 5126, type: 'VEC4', count: a.count };
    } else if (/^TEXCOORD_/.test(name)) acc = { bufferView: push(f32buf(a.data), 34962), componentType: 5126, type: 'VEC2', count: a.count };
    else if (/^COLOR_/.test(name)) { const b = Buffer.alloc(a.count * 4); for (let k = 0; k < a.count; k++) for (let c = 0; c < 4; c++) b[k * 4 + c] = Math.round(Math.min(1, Math.max(0, a.comps > c ? a.data[k * a.comps + c] : 1)) * 255); acc = { bufferView: push(b, 34962), componentType: 5121, normalized: true, type: 'VEC4', count: a.count }; }
    else return null;
    out.accessors.push(acc); cache.set(key, out.accessors.length - 1); return out.accessors.length - 1;
  };
  let verts = 0, tris = 0; const slots = [];
  for (const pr of mesh.primitives) {
    const A = {};
    for (const [name, idx] of Object.entries(pr.attributes)) { if (name.startsWith('_')) continue; const r = attr(idx, name); if (r !== null) A[name] = r; }
    const ia = readAccessor(g, pr.indices); const n = ia.count - (ia.count % 3);
    const maxIdx = ia.data.reduce((m, v) => (v > m ? v : m), 0), wide = maxIdx > 65535;
    const ib = Buffer.alloc(n * (wide ? 4 : 2));
    for (let k = 0; k < n; k += 3) { const tri = [ia.data[k], ia.data[k + 2], ia.data[k + 1]]; tri.forEach((v, c) => (wide ? ib.writeUInt32LE(v, (k + c) * 4) : ib.writeUInt16LE(v, (k + c) * 2))); }
    out.accessors.push({ bufferView: push(ib, 34963), componentType: wide ? 5125 : 5123, type: 'SCALAR', count: n });
    const sm = pr.material !== undefined ? J.materials[pr.material] : null;
    const matRef = sm && sm.extras ? sm.extras.ue_material : null;
    out.materials.push({ name: sm ? sm.name : 'material' + out.materials.length, extras: { ue_material: matRef } });
    out.meshes[0].primitives.push({ attributes: A, indices: out.accessors.length - 1, mode: 4, material: out.materials.length - 1 });
    slots.push({ slot_name: sm ? sm.name : null, material: matRef, triangles: n / 3 }); tris += n / 3;
  }
  verts = out.accessors[out.meshes[0].primitives[0].attributes.POSITION].count;
  const bin = Buffer.concat(chunks); out.buffers[0].byteLength = bin.length;
  let js = Buffer.from(JSON.stringify(out), 'utf8'); js = Buffer.concat([js, Buffer.alloc((4 - (js.length % 4)) % 4, 0x20)]);
  const hdr = Buffer.alloc(12); hdr.writeUInt32LE(0x46546c67, 0); hdr.writeUInt32LE(2, 4); hdr.writeUInt32LE(12 + 8 + js.length + 8 + bin.length, 8);
  const h1 = Buffer.alloc(8); h1.writeUInt32LE(js.length, 0); h1.writeUInt32LE(0x4e4f534a, 4); const h2 = Buffer.alloc(8); h2.writeUInt32LE(bin.length, 0); h2.writeUInt32LE(0x004e4942, 4);
  const pos = out.accessors[out.meshes[0].primitives[0].attributes.POSITION];
  return { asset: ref, content: loc.content, glb: Buffer.concat([hdr, h1, js, h2, bin]), vertices: verts, triangles: tris, slots, bounds_m: { min: pos.min, max: pos.max }, converted_from: rel(loc.file) };
}
async function validateGlb(buf, name) {
  const validator = require('gltf-validator');
  const rep = await validator.validateBytes(new Uint8Array(buf), { uri: name, maxIssues: 0, writeTimestamp: false });
  return { errors: rep.issues.numErrors, warnings: rep.issues.numWarnings, infos: rep.issues.numInfos, first: (rep.issues.messages || []).filter((m) => m.severity <= 1).slice(0, 3).map((m) => `${m.code} ${m.pointer || ''}`) };
}

// ------------------------------------------------------------------------------------------------ effect set
function isParticlePackage(content) { try { const t = fs.readFileSync(path.join(PKG, content + '.uasset.json'), 'utf8'); return /"class":"ParticleSystem"/.test(t); } catch (e) { return false; } }
function particleSystemsReferencedBy(content) { // /Game packages a package imports that hold a ParticleSystem
  const d = loadPkg(content); const out = [];
  for (const im of d.imports) if (im.className === 'ParticleSystem') { let r = im; while (r.outerIndex) r = d.imports[-r.outerIndex - 1]; const c = gamePathToContent(r.objectName); if (c) out.push(c); }
  return [...new Set(out)].sort();
}
function seedSet() {
  const out = new Map();
  for (const f of fs.readdirSync(path.join(PKG, SEED_DIR))) if (f.endsWith('.uasset.json')) { const c = SEED_DIR + '/' + f.slice(0, -12); if (isParticlePackage(c)) out.set(c.toLowerCase(), { content: c, used_by: ['seed: every particle system of ' + SEED_DIR] }); }
  for (const bp of SEED_BLUEPRINTS) { if (!fs.existsSync(path.join(PKG, bp + '.uasset.json'))) continue; for (const c of particleSystemsReferencedBy(bp)) { const k = c.toLowerCase(); if (!out.has(k)) out.set(k, { content: c, used_by: [] }); out.get(k).used_by.push('seed: imported by /Game/' + bp); } }
  return out;
}
function effectSet(opts) { // -> { effects: [{content, used_by}], skipped: [...], list_file, list_entries }
  const set = new Map(), skipped = [];
  const add = (content, why) => { const k = content.toLowerCase(); if (!set.has(k)) set.set(k, { content, used_by: [] }); const u = set.get(k).used_by; for (const w of [].concat(why || [])) if (w && !u.includes(w)) u.push(w); };
  let listEntries = null, listFile = null;
  if (opts.effects.length || opts.dirs.length) {
    for (const e of opts.effects) { const c = gamePathToContent(e); if (c && isParticlePackage(c)) add(c, 'command line'); else skipped.push({ effect: e, reason: c ? 'no ParticleSystem export' : 'no such package' }); }
    for (const dir of opts.dirs) for (const f of fs.readdirSync(path.join(PKG, dir)).sort()) if (f.endsWith('.uasset.json')) { const c = dir.replace(/\\/g, '/').replace(/\/$/, '') + '/' + f.slice(0, -12); if (isParticlePackage(c)) add(c, 'folder ' + dir); }
  } else {
    listFile = opts.list || DEFAULT_LIST;
    if (fs.existsSync(listFile)) {
      const L = readJson(listFile); listEntries = L.length;
      for (const r of L) {
        const c = r.effect ? gamePathToContent(r.effect) : null;
        if (!c) { skipped.push({ effect: r.effect, reason: 'no such package', class: r.class || null }); continue; }
        if (!isParticlePackage(c)) { skipped.push({ effect: r.effect, reason: 'not a ParticleSystem package', class: r.class || null }); continue; }
        add(c, [].concat(r.used_by || []).map((x) => (typeof x === 'string' ? x : JSON.stringify(x))));
        const rec = set.get(c.toLowerCase()); rec.priority = r.priority || null; rec.categories = r.categories || [];
      }
    } else listFile = null;
    if (!opts.noSeed) for (const s of seedSet().values()) add(s.content, s.used_by);
  }
  // 1.2.0: nested systems. A ParticleSystem package imports another ParticleSystem only through
  // ParticleModuleSQEX_SpawnParticleSystem.ParticleSystemAsset (Thunder's bolts vm_pole_thd*, Aero's vm_air_min_* ...):
  // the set is closed over those references so that the pack can play them.
  for (let grew = true; grew;) {
    grew = false;
    for (const e of [...set.values()]) for (const c of particleSystemsReferencedBy(e.content)) if (!set.has(c.toLowerCase())) { add(c, 'nested in ' + e.content.split('/').pop() + ' (SQEX_SpawnParticleSystem)'); grew = true; }
  }
  const effects = [...set.values()].sort((a, b) => (a.content.toLowerCase() < b.content.toLowerCase() ? -1 : 1));
  return { effects, skipped, list_file: listFile ? rel(listFile) : null, list_entries: listEntries };
}
function uniqueNames(contents) { // content path -> output base name (the package name; a clash gets the folder appended)
  const taken = new Map(), out = new Map();
  for (const c of contents) {
    let n = path.basename(c);
    if (taken.has(n.toLowerCase()) && taken.get(n.toLowerCase()) !== c) n = n + '__' + path.dirname(c).split('/').slice(-3).join('_');
    let k = 2; const n0 = n; while (taken.has(n.toLowerCase()) && taken.get(n.toLowerCase()) !== c) n = n0 + '_' + k++;
    taken.set(n.toLowerCase(), c); out.set(c, n);
  }
  return out;
}

// ------------------------------------------------------------------------------------------------ schema (also in spec.md)
const SCHEMA = {
  effect: {
    format: 'kh3-sora-vfx-effect/1', name: 'file base name', asset: '/Game path of the ParticleSystem package', object: 'name of the ParticleSystem export',
    source: '{ package (cooked path), sha256 { uasset, uexp } }',
    system: {
      'update_mode, update_time_fps, update_time_delta': 'UParticleSystem.SystemUpdateMode (EPSUM_RealTime | EPSUM_FixedTime), UpdateTime_FPS, UpdateTime_Delta',
      'warmup_time, warmup_tick_rate': 'seconds simulated on activation, and the tick step used for it (0 = default)', 'delay, delay_low, use_delay_range': 'seconds before the system starts',
      'lod_method, lod_distance_check_time, lod_distances, lod_settings': 'LOD selection (cm from the camera per LOD level)', 'use_fixed_relative_bounding_box, fixed_relative_bounding_box': '{min,max,isValid} in cm, relative to the component',
      params: 'every reflected scalar property of UParticleSystem: class default overlaid with the serialized values (UE property names; Square Enix additions included: SqExParticleFlag, ParticleSeAssets, SQEXBaseScale, ...)',
      serialized: 'names of the properties the asset stores (all others are class defaults)', undetermined_defaults: 'properties whose class default could not be read from the constructor and that the asset does not store',
    },
    'emitters[]': {
      'index, name, object, class': 'position in UParticleSystem.Emitters, EmitterName, export name, emitter class', render_mode: 'EEmitterRenderMode (ERM_Normal, ERM_LightsOnly, ...)', params: 'all scalar properties of the emitter object (defaults + serialized)',
      'lod_count, lods[]': 'number of LOD levels; per level { level, enabled, object, same_objects_as_lod0, differs[] }. Only LOD 0 is exported in full', enabled: 'ParticleLODLevel.bEnabled of LOD 0', peak_active_particles: 'ParticleLODLevel.PeakActiveParticles of LOD 0 (cooked-in particle budget)',
      typedata: '{ kind: sprite | mesh | ribbon | beam | anim_trail | gpu_sprite | sqex_control | sqex_tail_polygon | ..., class, + a module record; mesh: "/Game/..." for mesh emitters }',
      required: {
        material: '/Game path (materials/<name>.json)', emitter_blend_type: 'Square Enix property of the Required module (EBlendMode); how it relates to the material blend mode is not determined here', screen_alignment: 'EParticleScreenAlignment', sort_mode: 'EParticleSortMode',
        'use_local_space, use_emitter_local_space': 'bUseLocalSpace (particles follow the component) and the Square Enix bUseEmitterLocalSpace', 'kill_on_deactivate, kill_on_completed': 'stock flags',
        'duration, duration_low, duration_use_range, duration_recalc_each_loop, loops': 'EmitterDuration (s), its low bound when a range is used, EmitterLoops (0 = forever)', 'delay, delay_low, delay_use_range, delay_first_loop_only': 'EmitterDelay (s)',
        sqex_emitter_time: 'Square Enix emitter timing properties (EmitterStartTime, EmitterLoopTime, EmitterLoopDuration, EmitterDurationTime, LoopTime, LoopDuration, bUseLegacyEmitterTime, bIsLoopStartAndEnd), verbatim',
        sub_uv: '{ horizontal, vertical (SubImages grid), interpolation (EParticleSubUVInterpMethod), scale_uv, random_image_time, random_image_changes }', 'use_max_draw_count, max_draw_count': 'draw limit',
        'emitter_origin, emitter_rotation': 'cm / degrees {pitch,yaw,roll}', 'tex_u_scale, tex_v_scale': 'Square Enix UV scale', spawn_rate: 'distribution (legacy Required.SpawnRate; the Spawn module rate is the one used)', params: 'every scalar property of the Required module', serialized: 'property names stored in the asset',
      },
      spawn: '{ rate, rate_scale, burst_scale: distributions; bursts[] { count, count_low (-1 = no range), time (fraction of the emitter duration) }; burst_method, process_spawn_rate, process_burst_list, apply_global_spawn_rate_scale, params }',
      'modules[]': {
        'class, object, origin': 'module class, export name, ue4 | sqex', enabled: 'bEnabled', flags: '{ spawn, update, final_update (class-default bSpawnModule / bUpdateModule / bFinalUpdateModule: when Cascade calls the module), final_tick_component, particle_update, update_for_gpu_emitter (Square Enix flags), draw_3d, curves_as_color, supports_random_seed, requires_looping_notification, lod_validity }',
        params: 'every property of the module: class default overlaid with the serialized value; a distribution property is { distribution: <distribution> }; struct arrays are arrays of objects; object references are "/Game/..." strings', serialized: 'names stored in the asset', semantics: 'Square Enix modules only: text from build/sora_tmp/vfx/fxsys/sqex_modules.json, or null',
      },
      materials: '[{ material, role: required | mesh_material_module | referenced }]',
    },
    references: '{ materials[], meshes[], other[] } /Game paths used by the system', unsupported: '{ <feature>: { count, what } } features the JSON only carries verbatim or approximates',
  },
  distribution: {
    'class, object, value_kind': 'distribution class, export name, float | vector', type: 'constant | uniform | constant_curve | uniform_curve | parameter | other | none',
    constant: 'number or [x,y,z]', 'min, max': 'uniform: value = max + (min - max) * r, r uniform in [0,1) per axis (per locked group for vectors)', locked_axes: 'EDistributionVectorLockFlags: EDVLF_XY: y = x; EDVLF_XZ: z = x; EDVLF_YZ: z = y; EDVLF_XYZ: y = z = x (the value is copied, not the random number)',
    mirror_flags: 'per axis EDistributionVectorMirrorFlags: EDVMF_Same: min = max; EDVMF_Different: min as given; EDVMF_Mirror: min = -max', use_extremes: 'pick min or max (r > 0.5) instead of interpolating',
    curve: '{ looped, keys[] { t (InVal), interp (EInterpCurveMode), value[], arrive_tangent[], leave_tangent[] } }; uniform curves: max[] / min[] with their tangents. Evaluation FInterpCurve::Eval: before the first key = first value, after the last = last value, CIM_Constant = previous key, CIM_Linear = lerp, all CIM_Curve* = cubic Hermite with the stored tangents scaled by the key interval',
    parameter: '{ name, param_mode, min_input, max_input, min_output, max_output, constant } for instance-parameter distributions',
    raw_range: 'MinValue / MaxValue (/Vec) cached in the FRawDistribution struct', lookup_table: '{ curve_check { agrees, max_rel_diff }: constant curves only, the table against 128 float32 samples of the curve over the key range resampled linearly to entry_count;  op (0 uninitialised, 1 none, 2 random, 3 extreme), entry_count, entry_stride, sub_entry_stride, time_scale, time_bias, lock_flag, values[] }: the table baked by the cooker, which is what the shipped game samples (entry index = (t - time_bias) * time_scale, linear between entries; op 2: each entry holds min then max)',
    evaluated: 'time_invariant: { value | min, max }; curves: { domain [t0,t1], samples, value[] | min[], max[] } sampled uniformly over the domain (t = t0 + (t1 - t0) * i / (samples - 1)); mirror flags and locked axes already applied to min and value',
    serialized: 'property names stored in the distribution object',
  },
  material: {
    'name, asset, class, source': 'identity', 'parent_chain, root_material': 'the asset, its parents, down to the UMaterial', 'blend_mode, shading_model, two_sided, opacity_mask_clip_value': 'root values with the BasePropertyOverrides of the instances applied (base_property_overrides lists which)',
    textures: '{ <parameter>: { texture: /Game path, file: textures/<name>.png, from: root:<name> | function:<path> | instance:<name>, sampler_type, overrides } }', unnamed_textures: 'texture samples without a parameter name [{ expression, texture, file, in_function }]',
    'scalars, vectors': '{ <parameter>: { value, from, overrides (the value it replaces), not_in_root } } resolved from the root material, its material functions and each instance of the chain',
    instance_parameter_names: 'parameters this asset itself overrides', dynamic_parameters: 'DynamicParameter nodes: the four names fed by ParticleModuleParameterDynamic', expression_classes: 'count of surviving expression classes (root + its material functions)', material_functions: 'function assets called (transitively)',
    inputs: '{ <material input>: { connected, expression, expression_class, present_in_cooked_graph, reachable { textures, parameters, functions, dynamic_parameters, stripped_feeders } } }', graph: 'surviving expression nodes of the root material with their inputs',
    stripped_expression_classes_referenced: 'classes of expressions that cooking removed but whose names survive in inputs', shader_map: 'facts from the compiled shader map: vertex factories, uniform expressions as text (CPU-side parameter math, e.g. panner speed * time)', summary: 'interpretation: what feeds emissive / opacity, dynamic parameters, and evidence flags (true or null)',
    expression_classes_not_interpreted: 'surviving expression classes this tool lists but does not interpret',
  },
};
const CONVENTIONS = (packConv) => ({
  effect_json: 'source (Unreal) units and axes, untouched: centimetres, seconds, degrees for rotators; rotation modules (ParticleModuleRotation, RotationRate, MeshRotation) are in turns (1 = 360 degrees) as in stock Cascade; X forward, Y right, Z up, left-handed. Colours are linear RGB, not clamped (values above 1 are HDR intensity)',
  to_pack_space: 'the Sora pack (build/sora_pack/manifest.json conventions) is glTF: metres, +Y up, right-handed. Convert a position / velocity / size from an effect JSON with (x, y, z)_UE cm -> (x, z, y) * 0.01 m; a direction with (x, y, z) -> (x, z, y); a rotation quaternion with (x, y, z, w) -> (-x, -z, -y, w); a scale with (x, y, z) -> (x, z, y)',
  meshes: 'meshes/*.glb are already in pack conventions (the conversion is baked into the vertices, no conversion node; UE triangle order restored so counter-clockwise is front; tangent w negated). A mesh particle of size s (unitless scale in Cascade) scales the glb by s',
  textures: 'PNG, 8 bit RGBA, top mip, origin top-left (same as UE UV origin). sRGB chunk when the texture has SRGB set, gAMA 1.0 otherwise. Single-channel sources (BC4 / G8) are replicated to RGB with alpha 255; BC5 keeps R, G',
  sub_uv: 'frame i of a horizontal x vertical grid is column i % horizontal, row floor(i / horizontal), row 0 at the top of the image',
  time: 'emitter time is in seconds; relative particle time is 0..1 over the particle life; which of the two a distribution is sampled with depends on the module (spec.md)',
  pack_conventions: packConv || null,
});

// ------------------------------------------------------------------------------------------------ build
async function build(opts) {
  loadDefaults();
  bc.selfCheck();
  const OUT = opts.out;
  const set = effectSet(opts);
  if (!set.effects.length) throw new Error('no effects to export');
  const names = uniqueNames(set.effects.map((e) => e.content));
  const files = new Map(); // relative path -> Buffer
  const items = [];
  const put = (file, buf) => files.set(file, Buffer.isBuffer(buf) ? buf : Buffer.from(stringify(buf) + '\n'));
  // ---- effects
  const effects = [], matUse = new Map(), meshUse = new Map();
  for (const s of set.effects) {
    const eff = exportEffect(s.content, names.get(s.content));
    if (!eff) { set.skipped.push({ effect: '/Game/' + s.content, reason: 'no ParticleSystem export' }); continue; }
    eff.used_by = s.used_by; eff.priority = s.priority || null; eff.categories = s.categories || [];
    const used = eff._used; delete eff._used;
    for (const [m, roles] of used.materials) { if (!matUse.has(m)) matUse.set(m, { effects: new Set(), roles: new Set() }); matUse.get(m).effects.add(eff.name); roles.forEach((r) => matUse.get(m).roles.add(r)); }
    for (const m of used.meshes) { if (!meshUse.has(m)) meshUse.set(m, new Set()); meshUse.get(m).add(eff.name); }
    effects.push(eff);
  }
  // ---- 1.2.0: static meshes of spell actors that are no particle systems: the Blizzard ice trail
  //      (TresSplineMeshBlizzard, a TresDecalMeshSplineActor: four segment meshes and three end pieces)
  for (const bp of ACTOR_MESH_BLUEPRINTS) {
    if (!fs.existsSync(path.join(PKG, bp + '.uasset.json'))) continue;
    const d = loadPkg(bp);
    for (const im of d.imports) {
      if (im.className !== 'StaticMesh') continue;
      let r = im; while (r.outerIndex) r = d.imports[-r.outerIndex - 1];
      const m = r.objectName + '.' + im.objectName;
      if (!meshUse.has(m)) meshUse.set(m, new Set());
      meshUse.get(m).add('actor:' + bp.split('/').pop());
    }
  }
  // ---- meshes (their own materials join the material set)
  const meshRecs = [];
  const meshNames = uniqueNames([...meshUse.keys()].sort().map((m) => gamePathToContent(m) || m.replace(/^\/Game\//, '').replace(/\.[^./]+$/, '')));
  const meshFileOf = new Map();
  for (const m of [...meshUse.keys()].sort()) {
    const r = exportMesh(m);
    if (r.error) { meshRecs.push({ asset: m, error: r.error, used_by: [...meshUse.get(m)].sort() }); continue; }
    const file = 'meshes/' + meshNames.get(r.content) + '.glb';
    put(file, r.glb); meshFileOf.set(m, file);
    for (const sl of r.slots) if (sl.material && /^\/Game\//.test(sl.material)) { if (!matUse.has(sl.material)) matUse.set(sl.material, { effects: new Set(), roles: new Set() }); const u = matUse.get(sl.material); meshUse.get(m).forEach((e) => u.effects.add(e)); u.roles.add('mesh_slot'); }
    meshRecs.push({ file, asset: m, source_package: assetId(r.content), source_sha256: sourceFiles(r.content), vertices: r.vertices, triangles: r.triangles, slots: r.slots, bounds_m: r.bounds_m, converted_from: r.converted_from, used_by: [...meshUse.get(m)].sort() });
  }
  // ---- materials
  const matList = [...matUse.keys()].sort();
  const matNames = uniqueNames(matList.map((m) => gamePathToContent(m) || m.replace(/^\/Game\//, '').replace(/\.[^./]+$/, '')));
  const matRecs = [], texUse = new Map(), matFileOf = new Map();
  for (const m of matList) {
    const s = materialSummary(m, false);
    if (!s.exists && s.exists !== undefined) { matRecs.push({ asset: m, error: 'no such package', used_by: [...matUse.get(m).effects].sort() }); continue; }
    const c = gamePathToContent(m); const file = 'materials/' + matNames.get(c) + '.json'; matFileOf.set(m, file);
    const addTex = (t, role) => { if (typeof t !== 'string' || !/^\/Game\//.test(t)) return; if (!texUse.has(t)) texUse.set(t, { materials: new Set(), roles: new Set() }); texUse.get(t).materials.add(s.name); texUse.get(t).roles.add(role); };
    for (const [k, v] of Object.entries(s.textures)) addTex(v.texture, k);
    for (const v of s.unnamed_textures) addTex(v.texture, v.expression);
    matRecs.push({ file, s, used_by: [...matUse.get(m).effects].sort(), roles: [...matUse.get(m).roles].sort() });
  }
  // ---- textures
  const texList = [...texUse.keys()].sort();
  const texNames = uniqueNames(texList.map((t) => gamePathToContent(t) || t.replace(/^\/Game\//, '').replace(/\.[^./]+$/, '')));
  const texRecs = [], texFileOf = new Map();
  for (const t of texList) {
    const r = exportTexture(t), u = texUse.get(t);
    if (r.error) { texRecs.push({ asset: t, error: r.error, class: r.class || null, pixel_format: r.pixel_format || null, used_by: [...u.materials].sort() }); continue; }
    const file = 'textures/' + texNames.get(r.content) + (r.raw ? '.rgba16f.bin' : '.png'); put(file, r.raw || r.png); texFileOf.set(t, file);
    const { png, raw, ...restT } = r;
    texRecs.push({ file, ...restT, source_package: assetId(r.content), source_sha256: sourceFiles(r.content), roles: [...u.roles].sort(), used_by: [...u.materials].sort(), converted_from: rel(path.join(TEX, r.content + '.dds')) });
  }
  // ---- write materials (with file links) and effects (with file links)
  for (const mr of matRecs) {
    if (mr.error) continue; const s = JSON.parse(JSON.stringify(mr.s));
    for (const v of Object.values(s.textures)) v.file = texFileOf.get(v.texture) || null;
    for (const v of s.unnamed_textures) v.file = texFileOf.get(v.texture) || null;
    s.used_by = mr.used_by; put(mr.file, s);
  }
  for (const eff of effects) {
    for (const em of eff.emitters) { if (!em) continue; if (em.required) em.required.material_file = matFileOf.get(em.required.material) || null; for (const m of em.materials || []) m.file = matFileOf.get(m.material) || null; if (em.typedata && em.typedata.params && Array.isArray(em.typedata.params.SubMeshLists)) em.typedata.sub_mesh_files = em.typedata.params.SubMeshLists.map((sl) => ((sl && sl.SubMeshList) || []).map((m) => (m && typeof m.SubMesh === 'string' ? meshFileOf.get(m.SubMesh) || null : null))); if (em.typedata && em.typedata.mesh) { em.typedata.mesh_file = meshFileOf.get(em.typedata.mesh) || null; const mr = meshRecs.find((x) => x.asset === em.typedata.mesh); em.typedata.mesh_slots = mr && mr.slots ? mr.slots.map((sl) => ({ slot_name: sl.slot_name, material: sl.material, material_file: matFileOf.get(sl.material) || null })) : null; } }
    eff.files = { materials: eff.references.materials.map((m) => matFileOf.get(m) || null), meshes: eff.references.meshes.map((m) => meshFileOf.get(m) || null) };
    put('effects/' + eff.name + '.json', eff);
  }
  // ---- census of the pack (per effect and totals)
  const totals = { modules: {}, typedata: {}, distributions: {}, unsupported: {} };
  const bump = (o, k, n = 1) => { o[k] = (o[k] || 0) + n; };
  const effRows = effects.map((eff) => {
    const mods = {}, td = {}, dists = {};
    const scan = (x) => { if (!x || typeof x !== 'object') return; if (Array.isArray(x)) { x.forEach(scan); return; } if (x.distribution) { if (x.distribution.class) bump(dists, x.distribution.class); return; } Object.values(x).forEach(scan); };
    for (const em of eff.emitters) { if (!em || !em.lod_count) continue; bump(td, em.typedata.class || 'sprite (no TypeData)'); bump(mods, 'ParticleModuleRequired'); if (em.spawn) bump(mods, 'ParticleModuleSpawn'); for (const m of em.modules) bump(mods, m.class); scan(em.modules); scan(em.spawn); scan(em.typedata); if (em.required.spawn_rate) scan({ distribution: em.required.spawn_rate }); }
    for (const [k, v] of Object.entries(mods)) bump(totals.modules, k, v); for (const [k, v] of Object.entries(td)) bump(totals.typedata, k, v); for (const [k, v] of Object.entries(dists)) bump(totals.distributions, k, v);
    for (const [k, v] of Object.entries(eff.unsupported)) bump(totals.unsupported, k, v.count);
    const f = 'effects/' + eff.name + '.json';
    return { file: f, name: eff.name, asset: eff.asset, source_package: eff.source.package, source_sha256: eff.source.sha256, bytes: files.get(f).length, sha256: sha256(files.get(f)), emitters: eff.emitters.filter(Boolean).length,
      materials: eff.files.materials, meshes: eff.files.meshes, textures: [...new Set(eff.references.materials.flatMap((m) => { const mr = matRecs.find((x) => x.s && x.s.asset === ('/Game/' + gamePathToContent(m))); return mr ? [...Object.values(mr.s.textures).map((t) => texFileOf.get(t.texture)), ...mr.s.unnamed_textures.map((t) => texFileOf.get(t.texture))].filter(Boolean) : []; }))].sort(),
      typedata: td, module_classes: Object.keys(mods).sort(), unsupported: eff.unsupported, priority: eff.priority, categories: eff.categories, used_by: eff.used_by };
  });
  const sortObj = (o) => Object.fromEntries(Object.keys(o).sort().map((k) => [k, o[k]]));
  // ---- validation of meshes and textures
  const validation = { generator: `tools/sora_vfx_pack.js ${VERSION}`, validator: 'gltf-validator ' + require('gltf-validator/package.json').version, files: {} };
  for (const mr of meshRecs) if (mr.file) { const v = await validateGlb(files.get(mr.file), path.basename(mr.file)); validation.files[mr.file] = { validator: v }; mr.validation = v; }
  for (const tr of texRecs) if (tr.file && /[.]bin$/.test(tr.file)) validation.files[tr.file] = { raw_bytes: files.get(tr.file).length, ok: files.get(tr.file).length === tr.width * tr.height * 8, decoder_mip_check: null }; else if (tr.file) { const dd = bc.decodePng(files.get(tr.file)); validation.files[tr.file] = { png_read_back: `${dd.width}x${dd.height}x${dd.channels}`, ok: dd.width === tr.width && dd.height === tr.height && dd.channels === 4, decoder_mip_check: tr.decoder_mip_check }; }
  // ---- manifest
  let packConv = null; try { packConv = readJson(path.join(ROOT, 'build', 'sora_pack', 'manifest.json')).conventions; } catch (e) { /* the main pack may not be built */ }
  const D = loadDefaults();
  const manifest = {
    format: 'kh3-sora-vfx/1', generator: { tool: 'tools/sora_vfx_pack.js', version: VERSION },
    legal: 'local build output made from the user\'s own copy of the game; never distributed',
    spec: 'docs/sora-vfx-spec.md (effect system, trigger inventory, Cascade subset: module semantics, schema, approximations; detail in build/sora_tmp/vfx/cascade/spec.md)',
    inputs: { effect_list: set.list_file, effect_list_entries: set.list_entries, seed_set: opts.effects.length || opts.dirs.length || opts.noSeed ? null : { folder: '/Game/' + SEED_DIR, blueprints: SEED_BLUEPRINTS.map((b) => '/Game/' + b) }, command_line: { effects: opts.effects, dirs: opts.dirs },
      class_defaults: { file: rel(DEFAULTS_FILE), generated_by: D.generated_by, method: D.method, classes: Object.keys(D.classes).length, failed: Object.entries(D.classes).filter(([, v]) => v.fail).map(([k, v]) => k + ': ' + v.fail) },
      sqex_semantics: fs.existsSync(SQEX_FILE) ? rel(SQEX_FILE) : null },
    conventions: CONVENTIONS(packConv),
    distribution_semantics: { implemented: ['DistributionFloatConstant', 'DistributionFloatUniform', 'DistributionFloatConstantCurve', 'DistributionFloatUniformCurve', 'DistributionVectorConstant', 'DistributionVectorUniform', 'DistributionVectorConstantCurve', 'DistributionVectorUniformCurve'], carried_verbatim: ['Distribution*ParticleParameter / *Parameter* (type parameter)', 'SQEXDistribution* and anything else (type other)'], curve_samples: CURVE_SAMPLES,
      note: 'stock UE 4.17 Distributions.cpp / InterpCurve.h semantics; every distribution is exported raw (keys, tangents, flags, baked lookup table) and evaluated. The shipped game samples the baked lookup table of a distribution (FRawDistribution::GetValue with a table present), so lookup_table is the run-time truth and the curve is its source' },
    schema: SCHEMA,
    counts: { effects: effects.length, emitters: effects.reduce((a, e) => a + e.emitters.filter(Boolean).length, 0), materials: matRecs.filter((m) => m.file).length, textures: texRecs.filter((t) => t.file).length, meshes: meshRecs.filter((m) => m.file).length },
    census: { modules: sortObj(totals.modules), typedata: sortObj(totals.typedata), distributions: sortObj(totals.distributions), unsupported: sortObj(totals.unsupported) },
    effects: effRows,
    materials: matRecs.map((m) => (m.error ? m : { file: m.file, name: m.s.name, asset: m.s.asset, class: m.s.class, source_package: m.s.source.package, source_sha256: m.s.source.sha256, bytes: files.get(m.file).length, sha256: sha256(files.get(m.file)), root_material: m.s.root_material, blend_mode: m.s.blend_mode, shading_model: m.s.shading_model, two_sided: m.s.two_sided,
      textures: [...new Set([...Object.values(m.s.textures).map((t) => texFileOf.get(t.texture)), ...m.s.unnamed_textures.map((t) => texFileOf.get(t.texture))].filter(Boolean))].sort(), expression_classes_not_interpreted: m.s.expression_classes_not_interpreted, roles: m.roles, used_by: m.used_by })),
    textures: texRecs.map((t) => (t.file ? { ...t, bytes: files.get(t.file).length, sha256: sha256(files.get(t.file)) } : t)),
    meshes: meshRecs.map((m) => (m.file ? { ...m, bytes: files.get(m.file).length, sha256: sha256(files.get(m.file)) } : m)),
    skipped: set.skipped,
    failures: { textures: texRecs.filter((t) => t.error).map((t) => `${t.asset}: ${t.error}`), meshes: meshRecs.filter((m) => m.error).map((m) => `${m.asset}: ${m.error}`), materials: matRecs.filter((m) => m.error).map((m) => `${m.asset}: ${m.error}`) },
  };
  // ---- trigger table: the rows of sheets/port/sora-effects.json (written by tools/sora_vfx_sheet.js) as objects, each with the
  // pack file of its effect, so that the pack alone says when and where every effect is spawned (docs/sora-vfx-spec.md 7)
  const TRIG = path.join(ROOT, 'sheets', 'port', 'sora-effects.json');
  if (fs.existsSync(TRIG) && !opts.effects.length && !opts.dirs.length) {
    const sh = readJson(TRIG); const byAsset = new Map(effRows.map((e) => [String(e.asset).split('.')[0].toLowerCase(), e.file]));
    const trig = sh.rows.map((r) => { const o = Object.fromEntries(sh.columns.map((c, i) => [c, r[i]])); o.effect_file = typeof o.effect === 'string' ? byAsset.get(o.effect.split('.')[0].toLowerCase()) || null : null; return o; });
    put('triggers.json', { format: 'kh3-sora-vfx-triggers/1', source: 'sheets/port/sora-effects.json', note: 'one object per trigger -> effect row; columns as documented for the sheet (tools/lib/sheet_defs.js), effect_file = file of this pack or null (no effect, or an effect outside the pack)', rows: trig });
    manifest.triggers = { file: 'triggers.json', rows: trig.length, with_effect_file: trig.filter((t) => t.effect_file).length, bytes: files.get('triggers.json').length, sha256: sha256(files.get('triggers.json')) };
  }
  put('manifest.json', manifest);
  // ---- sheet rows (13 columns of sheets/port/sora-assets.json)
  const rows = [];
  const shaOf = (s) => (s ? s.uexp || s.uasset || s.decoded_json || '-' : '-');
  for (const e of effRows) rows.push(['vfx/' + e.file, 'data', '-', e.source_package, shaOf(e.source_sha256), e.bytes, e.sha256, '-', '-', '-', '-', 'json read back; verify.js value round trip', `ParticleSystem ${e.asset}: ${e.emitters} emitters; ${(e.used_by || []).slice(0, 3).join('; ') || 'effect set'}`]);
  for (const m of manifest.materials) if (m.file) rows.push(['vfx/' + m.file, 'data', '-', m.source_package, shaOf(m.source_sha256), m.bytes, m.sha256, '-', '-', '-', '-', 'json read back', `${m.class} referenced (${m.roles.join(', ')}) by ${m.used_by.slice(0, 4).join(', ')}${m.used_by.length > 4 ? ', ...' : ''}`]);
  for (const t of manifest.textures) if (t.file) rows.push(['vfx/' + t.file, 'texture', t.roles.slice(0, 8), t.source_package, shaOf(t.source_sha256), t.bytes, t.sha256, '-', '-', '-', '-', /[.]bin$/.test(t.file) ? 'raw half-float texels; size checked' : validation.files[t.file].ok ? 'png read back; decoder mip check mean ' + (t.decoder_mip_check === null ? '-' : t.decoder_mip_check) : 'FAILED', `texture of ${t.used_by.slice(0, 4).join(', ')}${t.used_by.length > 4 ? ', ...' : ''} (${t.pixel_format}, sRGB ${t.srgb})`]);
  for (const m of manifest.meshes) if (m.file) rows.push(['vfx/' + m.file, 'mesh', '-', m.source_package, shaOf(m.source_sha256), m.bytes, m.sha256, '-', '-', '-', '-', `gltf-validator ${m.validation.errors} errors ${m.validation.warnings} warnings`, `Mesh of ParticleModuleTypeDataMesh in ${m.used_by.slice(0, 4).join(', ')}${m.used_by.length > 4 ? ', ...' : ''}`]);
  if (manifest.triggers) rows.push(['vfx/triggers.json', 'data', '-', '-', '-', manifest.triggers.bytes, manifest.triggers.sha256, '-', '-', '-', '-', 'json read back', 'trigger table: rows of sheets/port/sora-effects.json with the pack file of each effect']);
  rows.push(['vfx/manifest.json', 'manifest', '-', '-', '-', files.get('manifest.json').length, sha256(files.get('manifest.json')), '-', '-', '-', '-', 'json read back', 'written by tools/sora_vfx_pack.js from the items above']);
  put('sheet_rows.json', rows);
  validation.summary = { meshes: meshRecs.filter((m) => m.file).length, mesh_errors: meshRecs.filter((m) => m.file && m.validation.errors).length, textures: texRecs.filter((t) => t.file).length, texture_failures: manifest.failures.textures.length, png_bad: Object.values(validation.files).filter((v) => v.ok === false).length };
  put('validation.json', validation);
  // ---- write (and drop files of an earlier run that are no longer produced)
  for (const sub of ['', 'effects', 'materials', 'textures', 'meshes']) { const dir = path.join(OUT, sub); fs.mkdirSync(dir, { recursive: true }); for (const f of fs.readdirSync(dir)) { const r = (sub ? sub + '/' : '') + f; if (fs.statSync(path.join(dir, f)).isFile() && !files.has(r)) fs.unlinkSync(path.join(dir, f)); } }
  let bytes = 0; const byKind = {};
  for (const [f, b] of files) { const dst = path.join(OUT, f); if (!fs.existsSync(dst) || !fs.readFileSync(dst).equals(b)) fs.writeFileSync(dst, b); bytes += b.length; const k = f.includes('/') ? f.split('/')[0] : 'root'; byKind[k] = byKind[k] || { files: 0, bytes: 0 }; byKind[k].files++; byKind[k].bytes += b.length; }
  console.log(`sora_vfx_pack ${VERSION}: ${effects.length} effects (${manifest.counts.emitters} emitters), ${manifest.counts.materials} materials, ${manifest.counts.textures} textures, ${manifest.counts.meshes} meshes -> ${rel(OUT)} (${files.size} files, ${bytes} bytes)`);
  for (const [k, v] of Object.entries(byKind)) console.log(`  ${k.padEnd(10)} ${String(v.files).padStart(5)} files ${String(v.bytes).padStart(11)} bytes`);
  console.log(`  effect list: ${set.list_file || '(none)'}${set.list_entries !== null ? ' (' + set.list_entries + ' entries)' : ''}; skipped ${set.skipped.length}`);
  console.log(`  typedata: ${Object.entries(sortObj(totals.typedata)).map(([k, v]) => k.replace('ParticleModuleTypeData', '') + ' ' + v).join(', ')}`);
  console.log(`  glb: ${validation.summary.meshes} validated, ${validation.summary.mesh_errors} with errors; png: ${validation.summary.textures} read back, ${validation.summary.png_bad} bad; failures: textures ${manifest.failures.textures.length}, meshes ${manifest.failures.meshes.length}, materials ${manifest.failures.materials.length}`);
  for (const f of [...manifest.failures.textures, ...manifest.failures.meshes, ...manifest.failures.materials]) console.log('    ' + f);
  return manifest;
}
async function check(opts) {
  const OUT = opts.out; const man = readJson(path.join(OUT, 'manifest.json')); let bad = 0, n = 0;
  const one = (r) => { if (!r.file) return; n++; const f = path.join(OUT, r.file); if (!fs.existsSync(f)) { console.log('missing ' + r.file); bad++; return; } const b = fs.readFileSync(f); if (sha256(b) !== r.sha256) { console.log('hash differs ' + r.file); bad++; } return b; };
  if (man.triggers) { const b = one(man.triggers); if (b) JSON.parse(b.toString('utf8')); }
  for (const e of man.effects) { const b = one(e); if (b) JSON.parse(b.toString('utf8')); }
  for (const m of man.materials) { const b = one(m); if (b) JSON.parse(b.toString('utf8')); }
  for (const t of man.textures) { const b = one(t); if (b && /[.]png$/.test(t.file)) { const d = bc.decodePng(b); if (d.width !== t.width || d.height !== t.height) { console.log('png size ' + t.file); bad++; } } }
  for (const m of man.meshes) { const b = one(m); if (b) { const v = await validateGlb(b, path.basename(m.file)); if (v.errors) { console.log(`glb ${m.file}: ${v.errors} errors ${v.first.join('; ')}`); bad++; } } }
  console.log(`sora_vfx_pack --check: ${n} files, ${bad} problems`);
  return bad;
}

function parseArgs(argv) {
  const o = { effects: [], dirs: [], list: null, out: path.join(ROOT, 'build', 'sora_pack', 'vfx'), noSeed: false, check: false, defaults: false, census: false, rest: [] };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--effect') o.effects.push(argv[++i]); else if (a === '--dir') o.dirs.push(argv[++i]); else if (a === '--list') o.list = path.resolve(argv[++i]); else if (a === '--out') o.out = path.resolve(argv[++i]);
    else if (a === '--no-seed') o.noSeed = true; else if (a === '--check') o.check = true; else if (a === '--defaults') o.defaults = true; else if (a === '--census') o.census = true; else o.rest.push(a);
  }
  return o;
}
module.exports = { VERSION, ROOT, PKG, CONTENT, loadPkg, gamePathToContent, importRef, refString, propMap, propsOf, classDefaults, loadDefaults, exportDistribution, exportEffect, materialSummary, functionSummary, exportTexture, exportMesh, effectSet, seedSet, parseArgs, evalCurve, typedataKind, findSystem, sqexInfo, isParticlePackage, sha256, SCHEMA };
if (require.main === module) {
  const opts = parseArgs(process.argv.slice(2));
  (async () => {
    if (opts.defaults) { const D = loadDefaults(true); console.log(`${rel(DEFAULTS_FILE)}: ${Object.keys(D.classes).length} classes, ${Object.values(D.classes).filter((c) => c.fail).length} failed`); return; }
    if (opts.census) { if (!fs.existsSync(CENSUS_JS)) throw new Error('census script missing: ' + rel(CENSUS_JS)); await require(CENSUS_JS).run(opts); return; }
    if (opts.check) { process.exitCode = (await check(opts)) ? 1 : 0; return; }
    await build(opts);
  })().catch((e) => { console.error(e.stack || e); process.exit(1); });
}
