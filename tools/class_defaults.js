// Class default values: what a property holds in the class default object (CDO).
// A cooked object does not serialize a property whose value equals its archetype's, and every
// object in the data sheets has its class's CDO as archetype (checked here against the template
// index of each export). The CDO is built by the class's C++ constructor, which is in the
// executable: GetPrivateStaticClassBody is given __DefaultConstructor (audit/reflection.json,
// class_constructor), which calls  new (X.GetObj()) TClass(X).
// This tool runs that constructor in the emulator (tools/lib/emu.js) on a zeroed buffer of
// sizeof(class) bytes and reads each property at its reflected offset. Calls whose `this` (RCX)
// lies inside the object are executed (super-class constructors, member constructors, virtual
// calls through the vtable the constructor installs); every other call is stubbed and recorded.
// A value is accepted only when it is determined:
//   zero   no instruction of the constructor chain stores to the member, and the engine zero-fills
//          the object before construction (StaticAllocateObject); the member is all zero bytes
//   store  every byte the chain stores to the member is a constant. Where a conditional jump depends
//          on a value not known here (run-time globals, the FObjectInitializer), both ways are
//          executed; the member must come out the same on every path that returns. A path that
//          runs into a trap instruction (the int3 after a fatal-error call) is not a return.
//   delta  a struct member of tagged properties: a cooked struct lists only the members that differ
//          from the archetype's struct, so the default is the empty list (no constructor needed)
// Nothing is accepted for a member a stubbed callee was handed a pointer into, for a property the
// engine loads from config files (CPF_Config), or when the run did not finish cleanly.
//   node tools/class_defaults.js
// Output: audit/class_defaults.json (per class: constructor, run, per data field: value, rule,
//         evidence or the reason nothing was recovered; and the cross-check against serialized values)
const fs = require('fs');
const path = require('path');
const { ROOT, logRun } = require('./lib/log');
const S = require('./lib/sheets');
const { create, ENTER } = require('./lib/emu');
const { TYPES } = require('./lib/layout');

const VERSION = '1.0.0';
const OUT = path.join(ROOT, 'audit', 'class_defaults.json');
const E = create();
const X = (n) => (n === null || n === undefined ? null : '0x' + n.toString(16).toUpperCase());
const K = { CHKSTK: 0x1458701C0, COOKIE: 0x145870110, FNAME_W: 0x142DD02C0, FNAME_A: 0x142DD0240, MEMSET: [0x14587190A, 0x145872B74], MEMCPY: [0x1458718FE, 0x145871904] };

// ---- evidence
const recs = JSON.parse(fs.readFileSync(path.join(ROOT, 'audit', 'reflection.json'), 'utf8'));
const classes = new Map(recs.filter((r) => r.kind === 'class').map((r) => [r.name, r]));
const structs = new Map(recs.filter((r) => r.kind === 'struct').map((r) => [r.name, r]));
const enums = new Map(recs.filter((r) => r.kind === 'enum').map((r) => [r.name, r]));
const isProp = (o) => /Property$/.test(o.object_class || '');
function findProp(cls, name) { // the property and the class that declares it
  for (let c = classes.get(cls), n = 0; c && n < 64; c = c.owner.super && c.owner.super !== '-' ? classes.get(c.owner.super) : null, n++) {
    const p = c.objects.find((o) => isProp(o) && o.outer === undefined && o.name === name);
    if (p) return { p, owner: c, inner: c.objects.filter((o) => o.outer === p.id) };
  }
  return null;
}
const elemSize = (p, inner) => (p.object_class === 'StructProperty' ? (p.target && p.target !== '-' && structs.get(p.target.name) ? structs.get(p.target.name).struct.size : null)
  : p.object_class === 'EnumProperty' ? (inner[0] && TYPES[inner[0].object_class] ? TYPES[inner[0].object_class][0] : null) : TYPES[p.object_class] ? TYPES[p.object_class][0] : null);

// ---- the data: which fields exist, which objects omit them, what the others hold
const fieldRows = S.rowsOf('data-fields').filter((f) => f.struct.startsWith('object/'));
const fieldsOf = new Map();
for (const f of fieldRows) (fieldsOf.get(f.struct) || fieldsOf.set(f.struct, []).get(f.struct)).push(f);
const use = new Map(); // "Class.field" -> { omitted, present, values: Map(json -> count) }
const templates = { cdo: 0, other: 0, examples: [] };
for (const f of S.listDataSheets()) {
  const text = fs.readFileSync(path.join(S.SHEETS, f), 'utf8');
  if (!text.includes('"kind": "objects"') && !text.includes('"kind":"objects"')) continue;
  const s = JSON.parse(text);
  if (s.kind !== 'objects') continue;
  const pk = s._package || {}, names = pk.names || [];
  const nm = (x) => (x && names[x.index] ? names[x.index].s : null);
  for (const row of s.objects) {
    // archetype of the export: the import named Default__<Class>
    const ex = (pk.exports || [])[row._export - 1];
    const imp = ex && ex.templateIndex < 0 ? (pk.imports || [])[-ex.templateIndex - 1] : null;
    if (imp && nm(imp.objectName) === 'Default__' + row._class) templates.cdo++; else { templates.other++; if (templates.examples.length < 10) templates.examples.push(`${f}#${row.id}`); row._notCdo = true; }
    for (const fl of fieldsOf.get('object/' + row._class) || []) {
      const k = row._class + '.' + fl.name;
      const u = use.get(k) || use.set(k, { omitted: 0, present: 0, raw: 0, values: new Map() }).get(k);
      const v = row[fl.name];
      if (v === undefined || v === null) u.omitted++;
      else if (S.hasRaw(v)) u.raw++;
      else { u.present++; const j = JSON.stringify(v); u.values.set(j, (u.values.get(j) || 0) + 1); }
    }
  }
}

// structs the data stores as tagged property lists (seen serialized that way)
const taggedStructs = new Set();
for (const fl of fieldRows) { if (fl.ue_type !== 'StructProperty') continue; const uq = use.get(fl.struct.slice(7) + '.' + fl.name); if (uq && [...uq.values.keys()].some((j) => j.startsWith('{"props":['))) taggedStructs.add(fl.struct_name); }

// StaticAllocateObject (0x142F78270) zero-fills the object before the constructor runs: its body calls memset
const SAO = 0x142F78270;
const zeroFill = (() => { const f = E.funcOf(SAO); if (!f) return null; const lo = E.BASE + f.begin, hi = E.BASE + f.end; for (let a = lo; a < hi - 5; a++) if (E.imgByte(a) === 0xE8) { const t = E.resolveThunk(a + 5 + (E.u32(a + 1) | 0)); if (K.MEMSET.includes(t)) return X(a); } return null; })();

// ---- constructor runs
const BUF = 0x200000000000n, XBUF = 0x300000000000n, UOBJECT = 0x40;
const runs = new Map();
const MAX_PATHS = 96;
function construct(cls) {
  if (runs.has(cls)) return runs.get(cls);
  const c = classes.get(cls);
  const size = c.owner.size, ctor = c.owner.class_constructor ? parseInt(c.owner.class_constructor, 16) : null;
  const R = { class: cls, class_constructor: X(ctor), size, paths: [], dead_paths: [], taints: [], passed: [], stubs: new Map(), entered: new Set(), fnames: 0, branch_sites: new Set() };
  runs.set(cls, R);
  if (ctor === null || !E.inText(ctor)) { R.fail = 'no class constructor recorded'; return R; }
  const lo = Number(BUF), hi = lo + size;
  const inBuf = (v) => typeof v === 'bigint' && v >= BUF && v < BUF + BigInt(size);
  const work = [[]]; // decision prefixes still to run
  let runsDone = 0;
  while (work.length) {
    if (runsDone++ >= MAX_PATHS) { R.fail = `more than ${MAX_PATHS} paths through the constructor depend on values not known here`; break; }
    const prefix = work.pop();
    const P = { prov: new Map(), taints: [], decisions: [] };
    let live = false;
    const hooks = new Map();
    hooks.set(K.CHKSTK, (k, st) => st.regs[0]); hooks.set(K.COOKIE, (k, st) => st.regs[0]);
    const fname = (k, st) => { const str = k.a[1] !== null ? E.str(Number(k.a[1])) : null; const tok = st.fresh({ fname: str }); if (k.a[0] !== null) st.wr(k.a[0], 8, str === null ? null : tok); R.fnames++; return k.a[0] === null ? undefined : k.a[0]; };
    hooks.set(K.FNAME_W, fname); hooks.set(K.FNAME_A, fname);
    for (const m of K.MEMSET) hooks.set(m, (k, st) => {
      const [d, v, n] = k.a;
      if (d === null) return undefined;
      if (n === null || n > 0x100000n) { if (inBuf(d)) P.taints.push({ off: Number(d - BUF), to: size, target: X(k.target), site: X(k.site), why: 'memset of unknown length' }); return d; }
      for (let i = 0n; i < n; i++) st.wr(d + i, 1, v === null ? null : v & 0xFFn);
      return d;
    });
    for (const m of K.MEMCPY) hooks.set(m, (k, st) => {
      const [d, src, n] = k.a;
      if (d === null) return undefined;
      if (n === null || n > 0x100000n) { if (inBuf(d)) P.taints.push({ off: Number(d - BUF), to: size, target: X(k.target), site: X(k.site), why: 'memcpy of unknown length' }); return d; }
      for (let i = 0n; i < n; i++) st.wr(d + i, 1, src === null ? null : st.rd(src + i, 1));
      return d;
    });
    hooks.set('*', (k, st) => {
      if (k.target !== null && inBuf(k.a[0]) && st.depth() < 24) { R.entered.add(k.target); return ENTER; }
      R.stubs.set(k.target !== null ? k.target : 'indirect', (R.stubs.get(k.target !== null ? k.target : 'indirect') || 0) + 1);
      // a callee that is not executed but receives a pointer into the object may write there
      // (a pointer to the object itself, not to a member, is listed separately: object_passed_to)
      k.a.forEach((v, i) => { if (!inBuf(v)) return; if (v === BUF) { if (R.passed.length < 200) R.passed.push({ arg: i, target: X(k.target), site: X(k.site) }); } else P.taints.push({ off: Number(v - BUF), arg: i, target: X(k.target), site: X(k.site) }); });
      return undefined;
    });
    hooks.set('indirect', hooks.get('*'));
    let r;
    try {
      r = E.run(ctor, { regs: { 1: XBUF }, hooks, budget: 400000, zero: [BUF, BUF + BigInt(size)], dataUnknown: true,
        decide: (ip, i) => { const d = i < prefix.length ? prefix[i] : false; P.decisions.push(d); R.branch_sites.add(ip); return d; },
        init: (st) => { st.wr(XBUF, 8, BUF); st.wr(BUF, UOBJECT, null); live = true; }, // UObjectBase (flags, class, name, outer) is filled in before construction: not known here
        onWrite: (a, v, ip) => { if (live && a >= lo && a < hi) P.prov.set(a - lo, { v, ip }); } });
    } catch (e) { R.fail = 'emulator error: ' + e.message; break; }
    // the other way at every undecided jump this run met beyond its prefix
    for (let i = prefix.length; i < P.decisions.length; i++) work.push([...P.decisions.slice(0, i), true]);
    P.end = r.end; P.steps = r.steps; P.entered = r.entered; P.end_ip = r.end_ip;
    if (r.end === 'ret' || r.end === 'tail') { R.paths.push(P); for (const t of P.taints) if (R.taints.length < 200) R.taints.push(t); }
    else if (r.end === 'trap') R.dead_paths.push({ end: r.end, at: X(r.end_ip), decisions: P.decisions.length });
    else { R.fail = `a path through the constructor ended with "${r.end}" after ${r.steps} instructions${r.end_ip ? ' at ' + X(r.end_ip) : ''}`; break; }
  }
  if (!R.fail && !R.paths.length) R.fail = 'no path through the constructor returns';
  return R;
}

// ---- reading a member out of a run
const EMPTY_STR = { s: '', enc: 'empty' };
const GUID0 = '00000000000000000000000000000000';
function memberOn(P, off, n) {
  const bytes = new Array(n); let stored = 0, ip = null;
  for (let i = 0; i < n; i++) {
    const p = P.prov.get(off + i);
    if (!p) { bytes[i] = 0; continue; }
    if (p.v < 0) return { state: 'unknown', why: `the constructor chain stores a value that is not a constant (at ${X(p.ip)})` };
    bytes[i] = p.v; stored++; if (ip === null) ip = p.ip;
  }
  const t = P.taints.find((x) => (x.to !== undefined ? off + n > x.off && off < x.to : x.off >= off && x.off < off + n));
  if (t) return { state: 'unknown', why: `a pointer to the member is passed to ${t.target}, which is not executed (call at ${t.site})` };
  return { state: stored ? 'store' : 'zero', bytes, ip };
}
function member(R, off, n) { // { state: 'zero' | 'store' | 'unknown', bytes, ip, why }: the same on every returning path
  let first = null;
  for (const P of R.paths) {
    const m = memberOn(P, off, n);
    if (m.state === 'unknown') return m;
    if (!first) first = m;
    else if (m.bytes.some((x, i) => x !== first.bytes[i])) return { state: 'unknown', why: 'the member differs between paths through the constructor that depend on values not known here' };
    else if (m.state === 'store' && first.state === 'zero') first = m;
  }
  return first;
}
const u = (b, o, n) => { let v = 0n; for (let i = n - 1; i >= 0; i--) v = (v << 8n) | BigInt(b[o + i]); return v; };
const NATIVE_STRUCT = { // structs the cooked data stores natively: [field, type]...
  Vector: [['x', 'f'], ['y', 'f'], ['z', 'f']], Vector2D: [['x', 'f'], ['y', 'f']], Rotator: [['pitch', 'f'], ['yaw', 'f'], ['roll', 'f']], Quat: [['x', 'f'], ['y', 'f'], ['z', 'f'], ['w', 'f']],
  LinearColor: [['r', 'f'], ['g', 'f'], ['b', 'f'], ['a', 'f']], IntPoint: [['x', 'i'], ['y', 'i']],
};
const f32 = (bits) => { const b = Buffer.alloc(4); b.writeUInt32LE(Number(bits)); const v = b.readFloatLE(0); return Number.isFinite(v) && !Object.is(v, -0) ? v : { f32bits: Number(bits) }; };

function defaultOf(cls, fl) {
  const idx = /\[(\d+)\]$/.exec(fl.name) ? Number(/\[(\d+)\]$/.exec(fl.name)[1]) : 0;
  const found = findProp(cls, fl.ue_name);
  if (!found) return { reason: `no reflected property named ${fl.ue_name} in ${cls} or its super classes` };
  const { p, owner, inner } = found;
  const base = { property: `${owner.name}.${p.name}`, type: p.object_class };
  // (a ClassProperty is tagged ObjectProperty in cooked data, an AssetClassProperty AssetObjectProperty)
  const TAG = { ClassProperty: 'ObjectProperty', AssetClassProperty: 'AssetObjectProperty' };
  if ((TAG[p.object_class] || p.object_class) !== fl.ue_type) return { ...base, reason: `the cooked tag type ${fl.ue_type} is not the reflected type ${p.object_class}` };
  if (BigInt(p.property_flags) & 0x44000n) return { ...base, reason: 'config property (CPF_Config): the engine overwrites the constructor value from .ini files when it builds the class default object' };
  if (idx >= (p.array_dim || 1)) return { ...base, reason: `element ${idx} is outside ArrayDim ${p.array_dim || 1}` };
  // struct of tagged properties: equal to the archetype's struct = nothing listed
  if (p.object_class === 'StructProperty' && !NATIVE_STRUCT[fl.struct_name] && taggedStructs.has(fl.struct_name)) {
    return { ...base, value: { props: [] }, rule: 'delta', evidence: '-', note: 'tagged struct: a cooked struct lists only the members that differ from the class default, so the default is the empty list' };
  }
  const es = elemSize(p, inner);
  if (es === null) return { ...base, reason: `storage size of ${p.object_class} is not known` };
  const R = construct(cls); // the run of the object's own class (it executes the whole super chain)
  if (R.fail) return { ...base, reason: R.fail };
  const off = p.offset + idx * es;
  const isBool = p.object_class === 'BoolProperty';
  const m = member(R, isBool ? p.offset : off, isBool ? 1 : es);
  if (m.state === 'unknown') return { ...base, reason: m.why };
  const b = m.bytes, ok = (value, note) => ({ ...base, value, rule: m.state, evidence: m.ip === null ? '-' : X(m.ip), offset: off, ...(note ? { note } : {}) });
  const allZero = b.every((x) => x === 0);
  switch (p.object_class) {
    case 'BoolProperty': return ok(!!(b[0] & p.mask));
    case 'FloatProperty': return ok(f32(u(b, 0, 4)));
    case 'IntProperty': return ok(Number(BigInt.asIntN(32, u(b, 0, 4))));
    case 'UInt32Property': return ok(Number(u(b, 0, 4)));
    case 'Int8Property': return ok(Number(BigInt.asIntN(8, u(b, 0, 1))));
    case 'Int16Property': return ok(Number(BigInt.asIntN(16, u(b, 0, 2))));
    case 'UInt16Property': return ok(Number(u(b, 0, 2)));
    case 'ByteProperty': case 'EnumProperty': {
      const en = p.target && p.target !== '-' ? enums.get(p.target.name) : null;
      const v = Number(u(b, 0, es));
      if (!en) return p.object_class === 'ByteProperty' && p.target === '-' ? ok(v) : { ...base, reason: 'the enum of the property is not a registered enum' };
      const hit = en.enum.values.find((x) => x[1] === v);
      return hit ? ok(hit[0], `${p.target.name} value ${v}`) : { ...base, reason: `the constructor leaves ${v}, which ${p.target.name} has no name for` };
    }
    case 'NameProperty': {
      if (allZero) return ok('None', 'FName index 0, number 0 is NAME_None');
      const rec = E.recOf(u(b, 0, 8));
      return rec && typeof rec.fname === 'string' ? ok(rec.fname, 'the string the constructor gives to the FName constructor') : { ...base, reason: 'the constructor stores an FName that is not built from a string constant there' };
    }
    case 'StrProperty': return allZero ? ok(EMPTY_STR, 'empty FString (null data, 0 characters)') : { ...base, reason: 'the constructor builds a non-empty FString (heap data)' };
    case 'ObjectProperty': case 'ClassProperty': return allZero ? ok({ object: 0 }, 'null pointer') : { ...base, reason: 'the constructor stores an object pointer that only exists at run time (a default subobject or a loaded object)' };
    case 'AssetObjectProperty': case 'AssetClassProperty': return b.slice(16, 32).every((x) => x === 0) ? ok(EMPTY_STR, 'empty asset path (the FString of the FStringAssetReference is empty)') : { ...base, reason: 'the constructor sets an asset path' };
    case 'ArrayProperty': {
      if (!(u(b, 0, 8) === 0n && u(b, 8, 4) === 0n)) return { ...base, reason: 'the constructor fills the array' };
      const it = inner[0];
      if (it && it.object_class === 'StructProperty') {
        if (!it.target || it.target === '-') return { ...base, reason: 'inner struct of the array not known' };
        return ok({ innerTag: { name: p.name, type: 'StructProperty', size: 0, arrayIndex: 0, structName: it.target.name, structGuid: GUID0, hasGuid: 0 }, items: [] }, 'empty TArray (null data, 0 elements); the inner tag is what an empty struct array serializes');
      }
      return ok({ items: [] }, 'empty TArray (null data, 0 elements)');
    }
    case 'MapProperty': case 'SetProperty':
      return u(b, 0, 8) === 0n && u(b, 8, 4) === 0n ? ok(p.object_class === 'MapProperty' ? { remove: [], entries: [] } : { remove: [], items: [] }, 'empty container (its sparse array has null data and 0 elements)') : { ...base, reason: 'the constructor fills the container' };
    case 'StructProperty': {
      const lay = NATIVE_STRUCT[fl.struct_name];
      if (!lay) return { ...base, reason: `struct ${fl.struct_name} is stored natively and its layout is not modelled here` };
      const v = {}; lay.forEach(([k, t], i) => { v[k] = t === 'f' ? f32(u(b, i * 4, 4)) : Number(BigInt.asIntN(32, u(b, i * 4, 4))); });
      return ok(v);
    }
    default: return { ...base, reason: `default of a ${p.object_class} is not modelled` };
  }
}

// ---- every field of every object class in the data
const rows = [];
const stats = { classes: 0, fields: 0, recovered: 0, needed_fields: 0, needed_recovered: 0, needed_cells: 0, needed_cells_recovered: 0, by_rule: {}, by_reason: {}, violations: 0, validated_against_serialized_values: 0, inner_tags_compared: 0, inner_tag_mismatches: 0 };
const violations = [];
for (const [kind, fls] of [...fieldsOf].sort((a, b) => (a[0] < b[0] ? -1 : 1))) {
  const cls = kind.slice(7);
  if (!classes.has(cls)) continue;
  stats.classes++;
  for (const fl of fls) {
    const d = defaultOf(cls, fl);
    const uq = use.get(cls + '.' + fl.name) || { omitted: 0, present: 0, values: new Map() };
    stats.fields++;
    const row = { id: `${cls}.${fl.name}`, class: cls, field: fl.id, omitted: uq.omitted, serialized: uq.present, ...d };
    if (d.value !== undefined) {
      // UE does not serialize a value equal to the archetype's: a recovered default must differ from every serialized value
      const j = JSON.stringify(d.value);
      if (uq.present) stats.validated_against_serialized_values++;
      // the inner tag made up here for an empty struct array must be the one the data really uses for the field
      if (d.value && d.value.innerTag && uq.present) { const t = JSON.parse(uq.values.keys().next().value).innerTag; stats.inner_tags_compared++; if (!t || ['name', 'type', 'arrayIndex', 'structName', 'structGuid', 'hasGuid'].some((k) => t[k] !== d.value.innerTag[k])) { stats.inner_tag_mismatches++; row.inner_tag_mismatch = t || null; } }
      if (uq.values.has(j)) { row.violation = `${uq.values.get(j)} objects of class ${cls} serialize exactly this value`; violations.push(row.id + ': ' + row.violation); stats.violations++; }
    }
    if (d.value !== undefined && !row.violation) { stats.recovered++; stats.by_rule[d.rule] = (stats.by_rule[d.rule] || 0) + 1; }
    else { const why = (row.violation ? 'violation' : d.reason).replace(/0x[0-9A-F]+/g, '0x..').replace(/after \d+/, 'after N'); stats.by_reason[why] = (stats.by_reason[why] || 0) + 1; }
    if (uq.omitted) { stats.needed_fields++; stats.needed_cells += uq.omitted; if (d.value !== undefined && !row.violation) { stats.needed_recovered++; stats.needed_cells_recovered += uq.omitted; } }
    rows.push(row);
  }
}
const runOut = [...runs.values()].map((R) => ({ class: R.class, class_constructor: R.class_constructor, size: R.size, returning_paths: R.paths.length, trap_paths: R.dead_paths.length, steps: R.paths.map((P) => P.steps),
  undecided_jump_sites: [...R.branch_sites].map(X), bytes_stored: R.paths.length ? R.paths[0].prov.size : 0, fnames: R.fnames, entered: [...R.entered].map(X), stubbed: [...R.stubs].map(([t, n]) => [typeof t === 'number' ? X(t) : t, n]),
  pointers_given_to_stubs: R.taints.slice(0, 40), object_passed_to: R.passed.slice(0, 40), fail: R.fail || null }));
{
  const head = JSON.stringify({ generated_by: 'tools/class_defaults.js', version: VERSION, zero_fill: { function: X(SAO), memset_call: zeroFill, note: 'StaticAllocateObject clears the object memory with memset before the class constructor runs' }, templates, stats, violations, runs: runOut }, null, 1);
  fs.writeFileSync(OUT, head.slice(0, head.lastIndexOf('}')).trimEnd() + ',\n "rows": [\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n ]\n}\n');
}
logRun({ tool: 'class_defaults', version: VERSION, outputs: [OUT], note: JSON.stringify({ ...stats, by_reason: undefined }) });
console.log(JSON.stringify({ templates: { cdo: templates.cdo, other: templates.other }, ...stats }, null, 1));
if (violations.length) console.log('VIOLATIONS:\n  ' + violations.slice(0, 20).join('\n  '));
