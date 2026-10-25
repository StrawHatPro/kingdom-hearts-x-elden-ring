// Stage 1 of the Spreadsheet Method: evidence -> method sheets.
// Rebuilds every sheet declared in tools/lib/sheet_defs.js (except the sheets/native ones, which
// tools/native_sheets.js owns) from the evidence files under audit/ and from sheets/schemas.
// Nothing is typed by hand: a cell with no evidence behind it is written as null (unchecked),
// and tools/preflight.js lists it as unfinished. Deterministic: same evidence, same bytes.
//   node tools/extract.js
const fs = require('fs');
const path = require('path');
const { ROOT, logRun } = require('./lib/log');
const { SHEETS, writeSheet, eachLine } = require('./lib/sheets');
const { DEFS, schemaRows, indexRows, columnsOf, fileOf } = require('./lib/sheet_defs');
const pe = require('./lib/pe');

const VERSION = '1.0.0';
const A = (f) => JSON.parse(fs.readFileSync(path.join(ROOT, 'audit', f), 'utf8'));
const outputs = [];
const counts = {};
function emit(kind, rows, file = fileOf(kind)) {
  const out = path.join(SHEETS, file);
  counts[kind] = (counts[kind] || 0) + writeSheet(out, kind, DEFS[kind].group, columnsOf(kind), rows);
  outputs.push(out);
}
const addr = (a) => (a === null || a === undefined ? null : '0x' + String(a).replace(/^0x/i, '').toUpperCase());
const hexLit = (s) => (/^0x[0-9a-fA-F]+$/.test(s || '') ? '0x' + s.slice(2).toUpperCase() : /^\d+$/.test(s || '') ? '0x' + Number(s).toString(16).toUpperCase() : null);
const ident = (s) => { const x = String(s).replace(/[^A-Za-z0-9_]/g, '_'); return /^[0-9]/.test(x) ? '_' + x : x || '_'; }; // as tools/reflection_gen.js
const isProp = (o) => /Property$/.test(o.object_class || '');
const unique = (seen, id, alt) => { let k = id; if (seen.has(k)) k = `${id}${alt}`; for (let n = 2; seen.has(k); n++) k = `${id}${alt}~${n}`; seen.add(k); return k; };
const bump = (m, k, n = 1) => m.set(k, (m.get(k) || 0) + n);

// ================================================================ reflect + exe/exec
const GAME = '/Script/TresGame';
// Storage size and C type per property class (UE 4.17, x64): the table of tools/lib/layout.js.
const { TYPES } = require('./lib/layout');

const recs = A('reflection.json');
const symbols = A('symbols.json');
const bpBases = A('blueprint_class_bases.json');

// The naming rules of tools/docs_gen.js (ordered regular expressions) are the grouping evidence.
const RULES = (() => {
  const src = fs.readFileSync(path.join(ROOT, 'tools', 'docs_gen.js'), 'utf8');
  const m = /const RULES = (\[[\s\S]*?\n\]);/.exec(src);
  if (!m) throw new Error('RULES not found in tools/docs_gen.js');
  return new Function(`return ${m[1]}`)();
})();
const ruleOf = (name) => RULES.find((r) => r.re.test(name));

const classRecById = new Map();
const classRows = [], structRows = [], fnRows = [], propRows = [], enumRows = [], enumValueRows = [];
const typeUses = new Map();
const classIds = new Set(), structIds = new Set(), fnIds = new Set(), propIds = new Set(), enumIds = new Set();
const classInfo = new Map(); // id -> { module, system, functions, super }
const moduleStats = new Map(); // module -> { classes, structs, enums, functions, natives, asset_classes, asset_exports }
const mod = (m) => moduleStats.get(m) || moduleStats.set(m, { classes: 0, structs: 0, enums: 0, functions: 0, natives: 0, asset_classes: 0, asset_exports: 0 }).get(m);
const sysStats = new Map(); // system id -> { classes, structs, functions, natives, blueprints }
const sys = (s) => sysStats.get(s) || sysStats.set(s, { classes: 0, structs: 0, functions: 0, natives: 0, blueprints: 0 }).get(s);
const moduleSystem = new Map(); // module -> system id (modules other than the game's)
const ruleIds = new Set(RULES.map((r) => r.id));
const systemOfModule = (m) => {
  if (!moduleSystem.has(m)) { let id = m.replace(/^\/Script\//, '').toLowerCase(); if (ruleIds.has(id)) id = 'module_' + id; moduleSystem.set(m, id); }
  return moduleSystem.get(m);
};
let strayProps = 0;
const violations = {}; // cross-checks on the recovered facts; reported, never hidden
const violate = (k, v) => { (violations[k] = violations[k] || []).push(v); };

// pass 1: ids of every record (properties and functions refer to them)
const idOfRec = new Map(); // record -> row id
const idByFn = new Map();  // registration function -> row id (struct, enum, function)
recs.forEach((r) => {
  const regfn = addr(r.registration_function);
  let id;
  if (r.kind === 'class') {
    id = unique(classIds, r.name || '@' + addr(r.owner.getter), '@' + addr(r.owner.getter));
    const module = r.owner.package || null;
    const system = module === null ? null : module === GAME ? ruleOf(r.name).id : systemOfModule(module);
    classRecById.set(id, r);
    classInfo.set(id, { module, system, functions: 0, super: r.owner.super, size: r.owner.size, row: null });
    if (module) mod(module).classes++;
    if (system) sys(system).classes++;
  } else if (r.kind === 'struct') id = unique(structIds, r.name || '@' + regfn, '@' + regfn);
  else if (r.kind === 'enum') id = unique(enumIds, r.name || '@' + regfn, '@' + regfn);
  else if (r.kind === 'function' || r.kind === 'delegatefunction') {
    const o = r.owner || {};
    const scope = o.name && classIds.has(o.name) ? o.name : o.package || null;
    id = unique(fnIds, scope ? `${scope}.${r.name || '@' + regfn}` : `@${regfn}${r.name ? '.' + r.name : ''}`, '@' + regfn);
  } else return;
  idOfRec.set(r, id);
  if (regfn) idByFn.set(regfn, id);
});
const structSize = new Map(recs.filter((r) => r.kind === 'struct').map((r) => [idOfRec.get(r), r.struct.size]));

// exec functions (native script functions): audit/symbols.json pairs name and address; the class comes from the
// native function table its StaticRegisterNatives function registers (audit/reflection.json, natives)
const execRows = [], execByAddr = new Map();
{
  const tableOwner = new Map(); // table address -> { cls, reg }
  for (const r of recs) if (r.kind === 'class') for (const t of r.natives || []) if (!tableOwner.has(addr(t.table))) tableOwner.set(addr(t.table), { cls: idOfRec.get(r), reg: addr(r.owner.register_natives) });
  const seen = new Set();
  for (const s of symbols) {
    if (!s.script_name) continue;
    const table = /native function table at (0x[0-9A-Fa-f]+)/.exec(s.evidence);
    const own = table ? tableOwner.get(addr(table[1])) : null;
    const cls = own ? own.cls : s.class || null;
    const reg = /registered by func_([0-9A-Fa-f]+)/.exec(s.evidence);
    const id = unique(seen, cls ? `${cls}::exec${s.script_name}` : s.name, '@' + addr(s.address));
    execRows.push([id, addr(s.address), cls, s.script_name, table ? addr(table[1]) : null, own ? own.reg : reg ? addr(reg[1]) : null, s.confidence]);
    execByAddr.set(addr(s.address), id);
  }
}

// pass 2: rows, in record order
const TARGET_KIND = { class: classIds, struct: structIds, enum: enumIds };
const headerUsed = new Set();
const CPP_FORM = ['regular', 'namespaced', 'enumclass'];
const exeEnumValues = new Map(); // enum id -> Map(value name -> number)
recs.forEach((r, ord) => {
  if (!idOfRec.has(r)) return;
  const o = r.owner || {};
  const regfn = addr(r.registration_function);
  const objs = r.objects;
  const top = r.kind === 'class' ? null : objs[r.top];
  const entity = idOfRec.get(r);
  const entityKind = r.kind === 'delegatefunction' ? 'function' : r.kind;
  // properties
  const propId = new Map(); // object id -> properties row id
  const ownerOf = (x) => {
    if (x.outer === undefined) return top ? [null, null] : [entity, entityKind];
    if (top && x.outer === top.id) return [entity, entityKind];
    const parent = objs.find((y) => y.id === x.outer);
    return parent && isProp(parent) ? [idOf(parent), 'property'] : [null, null];
  };
  const idOf = (x) => {
    if (!propId.has(x.id)) { const [own] = ownerOf(x); propId.set(x.id, unique(propIds, `${own ?? entity}.${x.name ?? '@' + x.id}`, '#' + x.id)); }
    return propId.get(x.id);
  };
  let direct = 0, withOffset = 0;
  for (const x of objs) {
    if (!isProp(x)) continue;
    const [own, ownKind] = ownerOf(x);
    if (own === null) strayProps++;
    if (ownKind !== 'property' && own !== null) { direct++; if (typeof x.offset === 'number') withOffset++; }
    bump(typeUses, x.object_class);
    let target = '-';
    if (x.target !== undefined) {
      const t = x.target;
      if (t === null) target = null;
      else if (t !== '-') target = t.kind === 'function' || t.kind === 'delegatefunction' ? idByFn.get(addr(t.fn)) ?? null : TARGET_KIND[t.kind] && TARGET_KIND[t.kind].has(t.name) ? t.name : null;
    }
    const isBool = x.object_class === 'BoolProperty';
    propRows.push([idOf(x), own, ownKind, x.id, x.name ?? null, x.object_class, ownKind === 'property' ? '-' : typeof x.offset === 'number' ? x.offset : null,
      x.property_flags || null, isBool ? (typeof x.mask === 'number' ? hexLit(String(x.mask)) : null) : '-', x.array_dim ?? 1, target]);
    // cross-checks: a member lies inside its owner
    if (ownKind === 'class' || ownKind === 'struct') {
      const size = ownKind === 'class' ? o.size : r.struct.size;
      if (typeof size === 'number' && typeof x.offset === 'number' && x.offset >= size) violate('member_beyond_owner_size', `${idOf(x)} at ${x.offset} >= ${size}`);
    } else if (ownKind === 'function' && typeof r.function.params_size === 'number' && typeof x.offset === 'number' && x.offset >= Math.max(1, r.function.params_size)) violate('parameter_beyond_params_size', `${idOf(x)} at ${x.offset} >= ${r.function.params_size}`);
  }
  let header = '-';
  if (withOffset && (r.kind === 'class' || r.kind === 'struct')) {
    let cname = ident(r.name);
    if (headerUsed.has(cname)) cname += '_' + String(r.registration_function).slice(2);
    headerUsed.add(cname);
    header = `build/gen/include/generated/reflection/${cname}.h`;
  }
  if (r.kind === 'class') {
    const info = classInfo.get(entity);
    const within = o.within && classIds.has(o.within) ? o.within : null;
    info.row = [entity, r.name || null, o.cpp_name || null, ord, info.module, o.size ?? null, o.class_flags || null, o.cast_flags || null, o.config ?? null, o.super === '-' ? '-' : o.super && classIds.has(o.super) ? o.super : null, within,
      info.system, direct, 0, header, addr(o.getter), o.register_natives ? addr(o.register_natives) : o.intrinsic ? '-' : null, regfn];
    classRows.push(info.row);
  } else if (r.kind === 'struct') {
    const s = r.struct;
    const module = o.package || null;
    if (module) mod(module).structs++;
    structRows.push([entity, r.name || null, ord, module, s.size ?? null, s.ops === '-' ? '-' : s.ops ? s.ops.align : null, s.super === '-' ? '-' : s.super && structIds.has(s.super) ? s.super : null, s.struct_flags || null,
      direct, header, addr(o.getter), regfn]);
    if (module === GAME) sys(ruleOf(r.name).id).structs++;
  } else if (r.kind === 'enum') {
    const module = o.package || null;
    if (module) mod(module).enums++;
    exeEnumValues.set(entity, new Map(r.enum.values));
    enumRows.push({ id: entity, module, form: CPP_FORM[r.enum.cpp_form] ?? null, values: r.enum.values, regfn });
  } else {
    const cls = o.name && classIds.has(o.name) ? o.name : null;
    const f = r.function;
    const flags = f.flags ? hexLit(f.flags) : null;
    let native = flags === null ? null : '-';
    if (flags !== null && (parseInt(flags, 16) & 0x400)) { // FUNC_Native: the exec function its class registers under the same name
      const crec = cls ? classRecById.get(cls) : null;
      const e = crec ? (crec.natives || []).flatMap((t) => t.entries).find((x) => x.name === r.name) : null;
      native = e ? execByAddr.get(addr(e.fn)) ?? null : null;
    }
    fnRows.push([entity, cls ?? (o.package ? '-' : null), r.name || null, r.kind, flags, f.params_size ?? null, direct, native, addr(o.getter), regfn]);
    if (cls) { const info = classInfo.get(cls); info.functions++; if (info.module) mod(info.module).functions++; if (info.system) sys(info.system).functions++; }
    else if (o.package) mod(o.package).functions++;
  }
});
for (const info of classInfo.values()) info.row[13] = info.functions;
for (const e of execRows) { const info = e[2] && classInfo.get(e[2]); if (info) { if (info.module) mod(info.module).natives++; if (info.system) sys(info.system).natives++; } }
// cross-checks on the class tree: size >= super's size, chains end at Object
for (const [id, info] of classInfo) {
  const s = info.super && info.super !== '-' ? classInfo.get(info.super) : null;
  if (s && typeof s.size === 'number' && typeof info.size === 'number' && info.size < s.size) violate('class_smaller_than_super', `${id} ${info.size} < ${info.super} ${s.size}`);
  let cur = id, n = 0;
  while (classInfo.get(cur) && classInfo.get(cur).super && classInfo.get(cur).super !== '-' && n++ < 64) cur = classInfo.get(cur).super;
  if (cur !== 'Object') violate('super_chain_does_not_end_at_Object', id);
}

emit('classes', classRows);
emit('structs', structRows);
emit('functions', fnRows);
emit('properties', propRows);
emit('exec', execRows);

// ================================================================ exe
emit('names', symbols.map((s) => {
  const e = s.evidence || '';
  const basis = s.basis ? s.basis : /^native function table/.test(e) ? 'native function table' : /^thunk to import/.test(e) ? 'import thunk' : /Function ID/.test(e) ? 'Ghidra Function ID' : /PE export|delay-import/.test(e) ? 'PE export or import data' : /RTTI|vtable/i.test(e) ? 'RTTI' : 'other';
  return [s.symbol, addr(s.address), s.name, basis, s.confidence, e];
}));
if (fs.existsSync(path.join(ROOT, 'audit', 'vfuncs.json'))) {
  // ownership of virtual functions (not names): which class introduces each function, at which slot
  emit('vfuncs', A('vfuncs.json').map((v) => [addr(v.address), addr(v.address), v.class ?? '-', v.owner_kind ?? '-', v.slot ?? '-', v.how ?? '-', v.introduced_by, v.tables, v.introduced_by === 1 ? 'confirmed' : 'inferred']));
}
{
  const rtti = A('rtti.json');
  emit('rtti', rtti.map((t) => [t.name, t.mangled, t.bases, t.vtables.length]));
  emit('vtables', rtti.flatMap((t) => t.vtables.map((v) => [addr(v.address), t.name, v.this_offset, v.slots])));
}
{
  // sections: the PE section table; coverage: ranges per section and kind
  const img = pe.load();
  const BASE = Number(img.imageBase);
  const dm = A('data_map_summary.json');
  const cov = new Map(); // "section/kind" -> { section, kind, ranges, bytes, mr, mb }
  const add = (section, kind, ranges, bytes, mr, mb) => {
    const k = `${section}/${kind}`;
    const c = cov.get(k) || cov.set(k, { section, kind, ranges: 0, bytes: 0, mr: 0, mb: 0 }).get(k);
    c.ranges += ranges; c.bytes += bytes; c.mr += mr; c.mb += mb;
  };
  const typed = new Set(Object.keys(dm.per_section));
  eachLine(path.join(ROOT, 'audit', 'symbol_coverage.json'), (line) => {
    if (line.charCodeAt(0) !== 123) return;
    const r = JSON.parse(line.endsWith(',') ? line.slice(0, -1) : line);
    if (typed.has(r.section)) return; // audit/data_map.json supersedes these rows (tools/check.js)
    const n = parseInt(r.end) - parseInt(r.start), m = r.level === 'L2';
    add(r.section, r.kind, 1, n, m ? 1 : 0, m ? n : 0);
  });
  const dataCov = new Set();
  for (const [s, v] of Object.entries(dm.per_section)) for (const [k, x] of Object.entries(v.by_kind)) { add(s, k, x.count, x.bytes, 0, 0); dataCov.add(`${s}/${k}`); }
  const secRows = img.sections.map((s) => {
    const mine = [...cov.values()].filter((c) => c.section === s.name);
    const unknown = mine.find((c) => c.kind === 'unknown');
    return [s.name, addr((BASE + s.rva).toString(16)), addr((BASE + s.rva + Math.max(s.vsize, s.rawSize)).toString(16)), Math.max(s.vsize, s.rawSize),
      mine.length ? mine.reduce((n, c) => n + c.ranges, 0) : null, mine.length ? (unknown ? unknown.bytes : 0) : null];
  });
  emit('sections', secRows);
  emit('coverage', [...cov.entries()].sort((a, b) => (a[0] < b[0] ? -1 : 1)).map(([k, c]) => [k, c.section, c.kind, c.ranges, c.bytes, dataCov.has(k) ? '-' : c.mr, dataCov.has(k) ? '-' : c.mb]));
  emit('data-types', Object.entries(dm.per_type).map(([k, v]) => [k, v.count, v.bytes]));
}

// ================================================================ assets
const census = A('asset_census.json');
// Structs and enums made in the editor (UserDefinedStruct, UserDefinedEnum) are defined by a cooked package, not
// registered by the exe: name -> the package files whose exports include one (the asset is named after it).
const userDefined = { UserDefinedStruct: new Map(), UserDefinedEnum: new Map() };
for (const f of census.files) {
  if (f[1] !== 'package') continue;
  for (const i of f[3] || []) { const m = userDefined[census.classes[i]]; if (m) { const n = path.basename(f[0]).replace(/\.[^.]+$/, ''); (m.get(n) || m.set(n, []).get(n)).push(f[0]); } }
}
// The census marks classes defined by cooked packages with a suffix; the row id is the class name.
const BP = ' [blueprint class]';
const assetClassId = new Map();
{
  const taken = new Set(census.classes.filter((c) => !c.endsWith(BP)));
  for (const c of census.classes) assetClassId.set(c, c.endsWith(BP) && !taken.has(c.slice(0, -BP.length)) ? c.slice(0, -BP.length) : c);
}
{
  const bc = A('asset_census_summary.json').by_class;
  const pm = A('package_manifest.json').classes;
  const rows = [];
  for (const [censusName, c] of Object.entries(bc)) {
    const id = assetClassId.get(censusName) || censusName;
    const from = Object.keys(c.from || {});
    const script = !censusName.endsWith(BP) && from.length > 0 && from.every((f) => f.startsWith('/Script/'));
    const module = script ? from[0] : '-';
    const p = pm[censusName] || {};
    // a class name that several cooked packages define with different native bases ("A|B"): their nearest common native ancestor
    let native = script ? id : (bpBases[id] || null);
    if (native && native.includes('|')) {
      const chain = (c) => { const out = []; for (let n = 0; c && c !== '-' && classInfo.has(c) && n < 64; n++) { out.push(c); c = classInfo.get(c).super; } return out; };
      const chains = native.split('|').map(chain);
      native = chains[0].find((c) => chains.every((x) => x.includes(c))) || null;
    }
    if (script) { mod(module).asset_classes++; mod(module).asset_exports += c.exports; }
    else if (native && classInfo.has(native) && classInfo.get(native).system) sys(classInfo.get(native).system).blueprints++;
    const status = p.uninterpreted_bytes === undefined ? null : p.uninterpreted_bytes === 0 ? 'decoded' : p.decoded === 0 ? 'undecoded' : 'partial';
    rows.push([id, script ? 'script' : 'blueprint', module, native, c.exports, c.bytes, p.files ?? null, c.decoder || null, c.layout || null,
      p.decoded ?? null, p.roundtrip_identical ?? null, p.uninterpreted_bytes ?? null, p.third_party_bytes ?? null, p.delegated_bytes ?? null, status]);
  }
  emit('asset-classes', rows);
}
{
  const areaOf = (p) => {
    const s = p.split('/');
    if (s[0] === '_superseded' || s[0] === '_case_collision') return s[0];
    if (s[0] === 'KINGDOM HEARTS III' && s[1] === 'Content' && s.length > 3) return 'Content/' + s[2];
    return s.length > 2 ? `${s[0]}/${s[1]}` : s[0];
  };
  const areas = new Map();
  for (const f of census.files) {
    const a = areaOf(f[0]);
    const x = areas.get(a) || areas.set(a, { rows: [], bytes: 0, packages: 0 }).get(a);
    const pkg = f[1] === 'package';
    x.rows.push([f[0], a, f[1], f[2], pkg ? (f[3] || []).map((i) => assetClassId.get(census.classes[i])) : '-']);
    x.bytes += f[2]; if (pkg) x.packages++;
  }
  const dir = path.join(SHEETS, 'assets', 'files');
  fs.rmSync(dir, { recursive: true, force: true });
  const areaRows = [];
  for (const [a, x] of [...areas].sort((p, q) => (p[0] < q[0] ? -1 : 1))) {
    const file = `assets/files/${a.replace(/[^A-Za-z0-9_]+/g, '_')}.json`;
    emit('asset-files', x.rows, file);
    areaRows.push([a, 'sheets/' + file, x.rows.length, x.bytes, x.packages]);
  }
  emit('asset-areas', areaRows);
}
emit('asset-formats', Object.entries(A('loose_files.json').formats).map(([k, v]) => [k, Object.keys(v.extensions || {}), v.files, v.bytes, v.identified_by ?? null, v.decoded ?? null, v.roundtrip_identical ?? null]));
emit('paks', A('asset_manifest.json').archives.map((a) => [a.pak, a.entries, a.roundtrip_ok, a.sha1_ok, a.stored_bytes, a.extracted_bytes, a.manifest]));
emit('movies', A('usm_manifest.json').files.map((f) => [f.path, f.size, f.chunks, (f.streams || []).map((s) => s.id), f.structural === undefined ? null : f.structural ? 'yes' : 'no', f.roundtrip_matches === undefined ? null : f.roundtrip_matches ? 'yes' : 'no']));

// ================================================================ data (DataTables and data-asset objects)
const enumUse = new Map(); // enum -> { fields: [], values: Map }
{
  const idx = A('sheets.json');
  const C_TYPE = { // as tools/gen_structs.js
    IntProperty: 'int32_t', UInt32Property: 'uint32_t', Int8Property: 'int8_t', Int16Property: 'int16_t', UInt16Property: 'uint16_t',
    Int64Property: 'int64_t', UInt64Property: 'uint64_t', FloatProperty: 'float', BoolProperty: 'bool', NameProperty: 'kh3_name',
    EnumProperty: 'kh3_name', StrProperty: 'const char *', AssetObjectProperty: 'const char *', ObjectProperty: 'int32_t',
  };
  const NATIVE = { Vector: 'kh3_vector', Vector2D: 'kh3_vector2d', Rotator: 'kh3_rotator', Quat: 'kh3_quat', LinearColor: 'kh3_linear_color', Color: 'kh3_color', IntPoint: 'kh3_int_point', Guid: 'kh3_guid' };
  const dir = path.join(SHEETS, 'schemas');
  const structs = new Map(); // kind id -> { row, enumFields: Map field -> enum }
  const fieldRows = [];
  const str = (v) => (v === undefined || v === null ? '-' : typeof v === 'string' ? v : JSON.stringify(v));
  for (const file of fs.readdirSync(dir).filter((f) => f.endsWith('.schema.json')).sort()) {
    const schema = JSON.parse(fs.readFileSync(path.join(dir, file), 'utf8'));
    const object = schema['x-kind'] === 'object';
    const id = `${object ? 'object' : 'row'}/${schema.title}`;
    const order = schema['x-field-order'];
    const enumFields = new Map();
    order.forEach((key, i) => {
      const p = schema.properties[key], t = p['x-ue'];
      let c = C_TYPE[t.type], note = t.type;
      const hasEnum = t.enumName && t.enumName !== 'None';
      if (t.type === 'ByteProperty') { c = hasEnum ? 'kh3_name' : 'uint8_t'; note += hasEnum ? ` enum ${typeof t.enumName === 'string' ? t.enumName : '?'}` : ''; }
      else if (t.type === 'StructProperty') { c = NATIVE[t.structName] || 'kh3_opaque'; note += ' ' + t.structName; }
      else if (t.type === 'ArrayProperty') { c = 'kh3_array'; note += `<${t.innerType}>`; }
      else if (t.type === 'MapProperty') { c = 'kh3_opaque'; note += `<${t.innerType}, ${t.valueType}>`; }
      else if (!c) c = 'kh3_opaque';
      const en = hasEnum && typeof t.enumName === 'string' ? t.enumName : '-';
      if (en !== '-') { enumFields.set(key, en); (enumUse.get(en) || enumUse.set(en, { fields: [], values: new Map() }).get(en)).fields.push(`${id}.${key}`); }
      bump(typeUses, t.type, 0);
      fieldRows.push([`${id}.${key}`, id, i, key, p['x-ue-name'], t.type, en, str(t.structName), str(t.innerType), str(t.valueType), [].concat(p.type).includes('null') ? 'yes' : 'no', c, note]);
    });
    const name = ident2(schema.title) + (object ? '_Object' : '_Row');
    // the same struct or class in the exe; for a user-defined struct, the cooked package that defines it
    const ud = userDefined.UserDefinedStruct.get(schema.title) || [];
    const reflect = (object ? classIds : structIds).has(schema.title) || classIds.has(schema.title) || structIds.has(schema.title) ? schema.title : ud.length === 1 ? ud[0] : schema.title;
    structs.set(id, { enumFields, row: [id, object ? 'object' : 'row', schema.title, reflect, order.length, 0, 0, `sheets/schemas/${file}`, `build/gen/include/generated/sheets/${name}.h`] });
  }
  const tableRows = [];
  for (const g of idx.generated) {
    const s = JSON.parse(fs.readFileSync(path.join(ROOT, g.sheet), 'utf8'));
    const objects = g.kind === 'objects';
    const kinds = new Set();
    for (const row of objects ? s.objects : s.rows) {
      const k = objects ? `object/${row._class}` : `row/${s.row_struct}`;
      const st = structs.get(k);
      if (!kinds.has(k)) { kinds.add(k); if (st) st.row[5]++; }
      if (!st) continue;
      st.row[6]++;
      for (const [f, en] of st.enumFields) if (typeof row[f] === 'string') bump(enumUse.get(en).values, row[f]);
    }
    tableRows.push([g.sheet.replace(/^sheets\//, ''), objects ? 'objects' : 'datatable', objects ? `object/${s.asset_class}` : `row/${s.row_struct}`, [...kinds].sort(), (objects ? s.objects : s.rows).length,
      s._source ? s._source.uasset : null, s._source ? s._source.archive : null]);
  }
  emit('data-structs', [...structs.values()].map((s) => s.row));
  emit('data-fields', fieldRows);
  emit('data-tables', tableRows);
}
function ident2(s) { return s.replace(/\[(\d+)\]/g, '_$1').replace(/[^A-Za-z0-9_]/g, '_').replace(/^(\d)/, '_$1'); } // as tools/gen_structs.js

// enums: every enum the exe registers (name -> value table); the cooked data adds where each is used
{
  // enums the data uses that the exe does not register: user-defined enums, read from the decoded package that defines them
  for (const id of [...enumUse.keys()].filter((k) => !enumIds.has(k)).sort()) {
    const files = userDefined.UserDefinedEnum.get(id) || [];
    const jf = files.length === 1 ? path.join(ROOT, 'assets_converted', 'packages', files[0] + '.json') : null;
    if (!jf || !fs.existsSync(jf)) continue;
    const ex = JSON.parse(fs.readFileSync(jf, 'utf8')).exports.find((e) => e.class === 'UserDefinedEnum' && e.objectName === id);
    if (!ex || !ex.data || !ex.data.native || !Array.isArray(ex.data.native.names)) continue;
    enumIds.add(id);
    enumRows.push({ id, module: '-', form: CPP_FORM[ex.data.native.cppForm] ?? null, values: ex.data.native.names.map((n) => [n.name, Number(n.value)]), regfn: '-', asset: files[0] });
  }
  const rows = enumRows.map((e) => { const u = enumUse.get(e.id); return [e.id, e.module, e.form, e.values.length, u ? u.values.size : 0, u ? u.fields : [], e.asset || '-', e.regfn]; });
  for (const id of [...enumUse.keys()].filter((k) => !enumIds.has(k)).sort()) { const u = enumUse.get(id); rows.push([id, null, null, null, u.values.size, u.fields, null, null]); violate('data_enum_not_registered_in_exe', id); }
  emit('enums', rows);
  const vrows = [], seenIds = new Set();
  for (const e of enumRows) {
    const u = enumUse.get(e.id), named = new Set();
    for (const [name, value] of e.values) { named.add(name); vrows.push([unique(seenIds, `${e.id}/${name}`, '#'), e.id, name, value, u ? u.values.get(name) || 0 : 0]); }
    if (u) for (const [v, n] of [...u.values].sort((a, b) => (a[0] < b[0] ? -1 : 1))) if (!named.has(v)) { // a value name the data uses but the registration does not list
      vrows.push([unique(seenIds, `${e.id}/${v}`, '#'), e.id, v, null, n]); violate('data_enum_value_not_in_exe', `${e.id}/${v} (${n} cells)`);
    }
  }
  for (const [id, u] of [...enumUse].sort((a, b) => (a[0] < b[0] ? -1 : 1))) if (!enumIds.has(id)) for (const [v, n] of [...u.values].sort((a, b) => (a[0] < b[0] ? -1 : 1))) vrows.push([unique(seenIds, `${id}/${v}`, '#'), id, v, null, n]);
  emit('enum-values', vrows);
}
// property types: StructProperty and EnumProperty have no size of their own (the struct's size, the underlying type's)
const DEPENDS = new Set(['StructProperty', 'EnumProperty']);
emit('property-types', [...new Set([...Object.keys(TYPES), ...typeUses.keys()])].sort().map((t) => [t, TYPES[t] ? TYPES[t][0] : DEPENDS.has(t) ? '-' : null, TYPES[t] ? TYPES[t][1] : DEPENDS.has(t) ? '-' : null, typeUses.get(t) || 0]));

// class defaults: one row per (class, data field) that some object omits
{
  const file = path.join(ROOT, 'audit', 'class_defaults.json');
  const rows = [];
  if (fs.existsSync(file)) {
    const cd = JSON.parse(fs.readFileSync(file, 'utf8'));
    for (const r of cd.rows) {
      if (!r.omitted) continue;
      const ok = r.value !== undefined && !r.violation;
      const crec = classRecById.get(r.class);
      rows.push([r.id, r.class, r.field, r.property ? (propIds.has(r.property) ? r.property : null) : null, r.type || null, r.omitted, ok ? r.value : null, ok ? r.rule : '-', ok ? (r.evidence === '-' ? '-' : addr(r.evidence)) : '-',
        crec && crec.owner.class_constructor ? addr(crec.owner.class_constructor) : null, ok ? '-' : r.violation ? 'contradicted by the data: ' + r.violation : r.reason]);
    }
  }
  emit('class-defaults', rows);
}

// ================================================================ design
// Whose code a module is. Decided only where there is evidence; the evidence is written next to the value.
const STOCK_RUNTIME = 'a module of stock Unreal Engine 4.17 under Engine/Source/Runtime (public source tree)';
const STOCK_PLUGIN = 'a module of a plugin shipped with stock Unreal Engine 4.17 under Engine/Plugins (public source tree)';
const ORIGIN = {};
for (const m of ['CoreUObject', 'Engine', 'AIModule', 'AnimGraphRuntime', 'AnimationCore', 'AssetRegistry', 'AudioMixer', 'CinematicCamera', 'ClothingSystemRuntime', 'ClothingSystemRuntimeInterface', 'EngineMessages', 'EngineSettings',
  'Foliage', 'GameplayTags', 'GameplayTasks', 'HeadMountedDisplay', 'InputCore', 'JsonUtilities', 'Landscape', 'LevelSequence', 'LightPropagationVolumeRuntime', 'MRMesh', 'MaterialShaderQualitySettings', 'MediaAssets', 'MoviePlayer',
  'MovieScene', 'MovieSceneCapture', 'MovieSceneTracks', 'Overlay', 'PacketHandler', 'Renderer', 'Serialization', 'SessionMessages', 'Slate', 'SlateCore', 'UMG', 'VectorVM']) ORIGIN[m] = ['engine', STOCK_RUNTIME];
for (const m of ['ActorSequence', 'AnalyticsBlueprintLibrary', 'ArchVisCharacter', 'AvfMediaFactory', 'WmfMediaFactory', 'CableComponent', 'CustomMeshComponent', 'ProceduralMeshComponent', 'FacialAnimation', 'GeometryCache', 'Paper2D',
  'OnlineSubsystem', 'OnlineSubsystemSteam', 'OnlineSubsystemUtils', 'TcpMessaging']) ORIGIN[m] = ['plugin', STOCK_PLUGIN];
ORIGIN.TresGame = ['game', 'the module of the Tres-prefixed classes (the project name in every cooked path, /Game content classes derive from it); its classes use config "Game"'];
ORIGIN.CriWareRuntime = ['middleware', 'not in stock UE 4.17; CRIWARE runtime of CRI Middleware (the movies are CRI .usm files, sheet assets/movies)'];
ORIGIN.Mercuna = ['middleware', 'not in stock UE 4.17; Mercuna 3D navigation middleware (Mercuna Developments)'];
ORIGIN.Mercuna3DMovement = ['middleware', 'not in stock UE 4.17; part of the Mercuna 3D navigation middleware'];
ORIGIN.HoudiniEngineRuntime = ['middleware', 'not in stock UE 4.17; runtime module of the Houdini Engine plugin (SideFX)'];
ORIGIN.ScaleformUI = ['middleware', 'not in stock UE 4.17; Scaleform GFx UI integration'];
ORIGIN.SQEXSEAD = ['middleware', 'not in stock UE 4.17; carries the vendor prefix SQEX: Square Enix sound driver SEAD (its .sab/.mab files are in sheet assets/asset-formats)'];
const UNDECIDED = {
  Niagara: 'Epic code, but whether 4.17 ships it as a runtime module or as a plugin is not established here',
  ImmediatePhysics: 'Epic code, but whether 4.17 ships it as a runtime module or as a plugin is not established here',
  BonamikRt: 'not a stock UE 4.17 module; nothing in the exe names its vendor', KBDRt: 'not a stock UE 4.17 module; nothing in the exe names its vendor',
  KineDriverRt: 'not a stock UE 4.17 module; nothing in the exe names its vendor', DirectInput: 'not a stock UE 4.17 module; nothing in the exe names its vendor',
};
emit('modules', [...moduleStats].sort((a, b) => (a[0] < b[0] ? -1 : 1)).map(([id, m]) => {
  const short = id.replace(/^\/Script\//, '');
  const og = ORIGIN[short];
  return [id, m.classes, m.structs, m.enums, m.functions, m.natives, m.asset_classes, m.asset_exports, og ? og[0] : null,
    [m.classes ? `${m.classes} class registration calls name it` : null, m.structs || m.enums ? `${m.structs} structs and ${m.enums} enums are outered to its package` : null, m.asset_classes ? `${m.asset_classes} classes imported from it by cooked packages` : null,
      og ? `origin: ${og[1]}` : `origin undecided: ${UNDECIDED[short] || 'no evidence'}`].filter(Boolean).join('; ')];
}));
{
  const z = { classes: 0, structs: 0, functions: 0, natives: 0, blueprints: 0 };
  // roots: classes of the system whose parent class is not in the same system (the tops of its subtrees)
  const rootsOf = new Map();
  for (const [id, info] of classInfo) { if (!info.system) continue; const sup = info.super && info.super !== '-' ? classInfo.get(info.super) : null; if (!sup || sup.system !== info.system) (rootsOf.get(info.system) || rootsOf.set(info.system, []).get(info.system)).push(id); }
  const roots = (id) => (rootsOf.get(id) || []).sort();
  const rows = RULES.map((r, i) => { const s = sysStats.get(r.id) || z; return [r.id, i + 1, r.title, GAME, r.re.source, s.classes, s.structs, s.functions, s.natives, s.blueprints, roots(r.id),
    `naming rule ${i + 1} of ${RULES.length} in tools/docs_gen.js (a heuristic on names, first match wins): ${r.words}`]; });
  let ord = RULES.length;
  for (const [m, id] of [...moduleSystem].sort((a, b) => (a[0] < b[0] ? -1 : 1))) { const s = sysStats.get(id) || z; rows.push([id, ++ord, `Classes of ${m}`, m, '-', s.classes, '-', s.functions, s.natives, s.blueprints, roots(id), `every class whose registration call names ${m}`]); }
  emit('systems', rows);
}

// ================================================================ gen targets, kernel, schema, index
{
  const refl = classRows.filter((r) => r[10] && r[10] !== '-').length + structRows.filter((r) => r[7] && r[7] !== '-').length;
  const data = counts['data-structs'];
  emit('targets', [
    ['reflection-layout', ['classes', 'structs', 'properties', 'property-types'], 'classes and structs rows whose header cell names a file', ['name', 'header', 'size'],
      'one packed C struct: a member and a static_assert(offsetof) per properties row that has an offset (bools at the byte that holds the bit), explicit padding between and up to size, a static_assert(sizeof); a properties row without an offset becomes a comment',
      'build/gen/include/generated/reflection/<Name>.h', refl, 'tools/check_asserts.js (gate static_asserts_target_abi); tools/reflection_gen.js --check regenerates the same files from audit/reflection.json'],
    ['reflection-index', ['classes', 'structs'], 'the same rows', ['header'], 'one #include line', 'build/gen/include/generated/reflection.h', 1, 'tools/check_asserts.js'],
    ['reflection-types', ['property-types'], 'rows with a c_type', ['c_type'], 'the value types named by c_type (fixed prelude text in tools/gen.js)', 'build/gen/include/generated/reflection_types.h', 1, 'tools/check_asserts.js'],
    ['data-struct', ['data-structs', 'data-fields'], 'every data-structs row', ['name', 'kind', 'header'], 'one C struct in serialized field order: a member per data-fields row',
      'build/gen/include/generated/sheets/<Name>_Row.h, <Name>_Object.h', data, 'tools/gen_structs.js --check (gate generated_code_current) regenerates the same files from sheets/schemas'],
    ['data-index', ['data-structs'], 'every data-structs row', ['header'], 'one #include line', 'build/gen/include/generated/sheets.h', 1, 'tools/gen_structs.js --check'],
    ['data-types', ['data-fields'], 'rows with a c_type', ['c_type'], 'the value types named by c_type (fixed prelude text in tools/gen.js)', 'build/gen/include/generated/sheet_types.h', 1, 'tools/gen_structs.js --check'],
  ]);
}
{
  const rows = [];
  const scan = (dir, kind, ext) => {
    const d = path.join(ROOT, dir);
    if (!fs.existsSync(d)) return;
    for (const f of fs.readdirSync(d).filter((x) => ext.test(x)).sort()) {
      const text = fs.readFileSync(path.join(d, f), 'utf8');
      const first = /^\s*(?:\/\/+|#|\/\*+|\*|<#)\s*(\S.*?)\s*(?:\*\/)?\s*$/m.exec(text.split('\n').slice(0, 6).join('\n'));
      const v = /\bVERSION\s*=\s*['"]([^'"]+)['"]/.exec(text);
      rows.push([`${dir}/${f}`, kind, first ? first[1] : null, v ? v[1] : '-']);
    }
  };
  scan('tools', 'tool', /\.(js|ps1)$/); scan('tools/lib', 'lib', /\.js$/); scan('tools/ghidra', 'ghidra', /\.(java|py)$/);
  emit('kernel', rows);
}
emit('schema', schemaRows());
emit('sheets', indexRows());

{
  const V = path.join(ROOT, 'audit', 'reflection_checks.json');
  fs.writeFileSync(V, JSON.stringify({ generated_by: 'tools/extract.js', note: 'cross-checks of the reflection sheets; an empty object means none failed', violations: Object.fromEntries(Object.entries(violations).map(([k, v]) => [k, { count: v.length, rows: v.slice(0, 200) }])) }, null, 2) + '\n');
  outputs.push(V);
  for (const [k, v] of Object.entries(violations)) console.log(`check: ${k}: ${v.length}  e.g. ${v.slice(0, 3).join('; ')}`);
}
logRun({ tool: 'extract', version: VERSION, outputs, note: Object.entries(counts).map(([k, n]) => `${k} ${n}`).join(', ') });
const w = Math.max(...Object.keys(counts).map((k) => k.length));
for (const [k, n] of Object.entries(counts)) if (!process.argv.includes('--quiet')) console.log(`  ${k.padEnd(w)} ${String(n).padStart(7)} rows  sheets/${fileOf(k)}`);
if (strayProps) console.log(`note: ${strayProps} properties have no recorded outer inside a struct or function record; their owner cell is unchecked`);
console.log(`extract: ${outputs.length} files written`);
