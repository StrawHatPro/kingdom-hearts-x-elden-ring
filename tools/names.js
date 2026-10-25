// Semantic names with recorded evidence (spec 1.3). Sources, strongest first:
//   1. UE4 native-function registration tables in .rdata: arrays of
//      { const char *Name; void *Function } passed to FNativeFunctionRegistrar. The engine itself
//      pairs the script-visible name with the function address        -> confirmed
//   2. functions the reflection miner identifies by construction (audit/reflection_functions.json):
//      StaticClass getters, Z_Construct_* functions, InternalConstructor<T>, StaticRegisterNatives,
//      the engine functions they call                                    -> confirmed (inferred where
//      the C++ class prefix comes from the UnrealHeaderTool rule instead of a literal)
//   3. C++ constructors of reflected classes (audit/class_vtables.json): the functions that install
//      a class's vtable during InternalConstructor<T>                    -> confirmed
//   4. a string literal of the form Class::Method addressed by exactly one function, which addresses
//      no other such literal (PURE_VIRTUAL stubs, __FUNCTION__ texts)     -> inferred
//   5. import thunks (jmp [__imp_X]) and delay-load stubs                -> confirmed
//   6. Ghidra Function ID library matches                                -> inferred
//   7. jump thunks (jmp rel32 at a function start) to a named function   -> as the target
// Every record carries "basis" (the kind of evidence) and "evidence" (the text). A name the same
// basis gives to several addresses gets the suffix @0x<address> (sources 2 to 4 and 7).
// Writes audit/symbols.json. Sources and code keep address names; this table is the mapping.
const fs = require('fs');
const path = require('path');
const { ROOT, logRun, hex } = require('./lib/log');
const pe = require('./lib/pe');

const VERSION = '1.0.0';
const OUT = path.join(ROOT, 'audit', 'symbols.json');
const img = pe.load();
const { b, sections } = img;
const BASE = Number(img.imageBase);
const text = sections[0], rdata = sections.find((s) => s.name === '.rdata');
const inText = (va) => va >= BASE + text.rva && va < BASE + text.rva + text.vsize;
const inRdata = (va) => va >= BASE + rdata.rva && va < BASE + rdata.rva + rdata.vsize;

// function starts known from the coverage map
const starts = new Set();
for (const r of JSON.parse(fs.readFileSync(path.join(ROOT, 'audit', 'symbol_coverage.json'), 'utf8')))
  if (r.section === '.text' && ['function', 'thunk', 'library'].includes(r.kind) && !r.parent) starts.add(parseInt(r.start));

function identAt(va) {
  let o = img.rvaToOff(va - BASE);
  if (o < 0) return null;
  const s = o;
  while (o < b.length && o - s < 96) {
    const c = b[o];
    if (c === 0) break;
    if (!((c >= 0x30 && c <= 0x39) || (c >= 0x41 && c <= 0x5a) || (c >= 0x61 && c <= 0x7a) || c === 0x5f)) return null;
    o++;
  }
  if (b[o] !== 0 || o - s < 3 || (b[s] >= 0x30 && b[s] <= 0x39)) return null;
  return b.toString('latin1', s, o);
}

const symbols = new Map(); // address -> record
const add = (va, rec) => { if (!symbols.has(va)) symbols.set(va, { address: hex(va), symbol: 'func_' + va.toString(16).toUpperCase(), ...rec }); };

// Which class a native table belongs to: each class's StaticRegisterNatives function passes its table and the
// entry count to FNativeFunctionRegistrar::RegisterFunctions; tools/reflection.js reads both arguments
// (audit/reflection.json, "natives" of each class record).
const tableClass = new Map();
const stats = { native_tables: 0, native_names: 0, single_pairs_ignored: 0, tables_from_reflection: 0 };
const reflPath = path.join(ROOT, 'audit', 'reflection.json');
if (fs.existsSync(reflPath)) {
  for (const r of JSON.parse(fs.readFileSync(reflPath, 'utf8'))) {
    if (r.kind !== 'class' || !r.natives) continue;
    for (const t of r.natives) {
      const tableVa = parseInt(t.table, 16);
      if (tableClass.has(tableVa)) continue;
      const tc = { cls: r.name, cpp: r.owner.cpp_name || r.name, fn: r.owner.register_natives.slice(2) };
      tableClass.set(tableVa, tc);
      stats.tables_from_reflection++;
      // the registered entries themselves (also covers tables with a single entry, which the scan below ignores)
      for (const e of t.entries) {
        const fn = parseInt(e.fn, 16);
        if (symbols.has(fn) || !e.name) continue;
        stats.native_names++; stats.native_names_with_class = (stats.native_names_with_class || 0) + 1;
        add(fn, { name: tc.cpp + '::exec' + e.name, script_name: e.name, class: tc.cls, confidence: 'confirmed',
          evidence: `native function table at ${hex(tableVa)} (${t.entries.length} entries) pairs the string "${e.name}" with this address; the table is registered by func_${tc.fn}, the StaticRegisterNatives function of class ${tc.cls}` });
      }
    }
  }
}
// 2. + 3. what the registration code proves (tools/reflection.js, tools/vtables.js)
stats.reflection_functions = 0; stats.constructors = 0; stats.string_literal = 0;
const derived = []; // records added from sources 2-4 and 7: made unique at the end
// A function that several unrelated classes hold in their vtables (audit/vfuncs.json, introduced_by > 1), or whose
// body is a bare return, is code the linker folded from many identical functions: it has no single name.
const sharedCode = new Set();
const vfPath = path.join(ROOT, 'audit', 'vfuncs.json');
if (fs.existsSync(vfPath)) for (const v of JSON.parse(fs.readFileSync(vfPath, 'utf8'))) if (v.introduced_by > 1) sharedCode.add(parseInt(v.address, 16));
const bareReturn = (va) => { const o = img.rvaToOff(va - BASE); return o >= 0 && (b[o] === 0xC3 || b[o] === 0xC2); };
stats.skipped_shared_code = 0;
const addDerived = (va, rec) => {
  if (symbols.has(va) || !inText(va)) return false;
  if (rec.basis !== 'jump thunk' && (sharedCode.has(va) || bareReturn(va))) { stats.skipped_shared_code++; return false; }
  add(va, rec); derived.push(symbols.get(va)); return true;
};
const factsPath = path.join(ROOT, 'audit', 'reflection_functions.json');
if (fs.existsSync(factsPath)) for (const f of JSON.parse(fs.readFileSync(factsPath, 'utf8'))) {
  if (addDerived(parseInt(f.address, 16), { name: f.name, confidence: f.confidence, basis: f.basis, evidence: f.evidence })) stats.reflection_functions++;
}
const vtPath = path.join(ROOT, 'audit', 'class_vtables.json');
if (fs.existsSync(vtPath)) for (const c of JSON.parse(fs.readFileSync(vtPath, 'utf8')).constructors) {
  if (addDerived(parseInt(c.address, 16), { name: c.name, class: c.class, confidence: 'confirmed', basis: 'class constructor chain', evidence: c.evidence })) stats.constructors++;
}
// 4. string literals that name the function addressing them
{
  const S = require('./lib/sheets');
  const byKind = S.sheetsByKind();
  const enums = new Set(fs.existsSync(reflPath) ? JSON.parse(fs.readFileSync(reflPath, 'utf8')).filter((r) => r.kind === 'enum').map((r) => r.name) : []);
  const re = /^[A-Za-z_][A-Za-z0-9_]*(::~?[A-Za-z_][A-Za-z0-9_]*)+$/;
  const byText = new Map(); // text -> { users, ids }
  for (const file of byKind.get('strings') || []) S.eachRow(file, (r, h) => {
    const t = r[h.col.text];
    if (typeof t !== 'string' || t.length > 120 || !re.test(t) || enums.has(t.slice(0, t.indexOf('::')))) return; // (Enum::Value literals are not function names)
    const e = byText.get(t) || byText.set(t, { users: new Set(), ids: [] }).get(t);
    e.ids.push(r[h.col.id]);
    for (const u of r[h.col.used_by]) e.users.add(parseInt(u, 16));
  });
  const perFn = new Map(); // function -> texts it addresses
  for (const [t, e] of byText) for (const u of e.users) (perFn.get(u) || perFn.set(u, []).get(u)).push(t);
  stats.string_literal_skipped_shared = 0; stats.string_literal_skipped_several = 0;
  for (const [t, e] of [...byText].sort((a, b) => (a[0] < b[0] ? -1 : 1))) {
    if (e.users.size !== 1) { if (e.users.size > 1) stats.string_literal_skipped_shared++; continue; }
    const u = [...e.users][0];
    if (perFn.get(u).length !== 1) { stats.string_literal_skipped_several++; continue; }
    if (!starts.has(u)) continue;
    if (addDerived(u, { name: t, confidence: 'inferred', basis: 'string literal', evidence: `the only function that addresses the string "${t}" (at ${e.ids.join(', ')}), and the only Class::Method literal it addresses; such literals are the texts of PURE_VIRTUAL stubs and __FUNCTION__` })) stats.string_literal++;
  }
}

// 1. registration tables: runs of (string pointer, function pointer) pairs, 16 bytes each
const ro = rdata.rawPtr, roEnd = rdata.rawPtr + Math.min(rdata.rawSize, rdata.vsize);
const pairAt = (o) => {
  if (o + 16 > roEnd) return null;
  const ps = Number(b.readBigUInt64LE(o)), pf = Number(b.readBigUInt64LE(o + 8));
  if (!inRdata(ps) || !inText(pf) || !starts.has(pf)) return null;
  const name = identAt(ps);
  return name ? { name, fn: pf } : null;
};
for (let o = ro; o + 16 <= roEnd; o += 8) {
  const first = pairAt(o);
  if (!first) continue;
  const run = [first];
  let p = o + 16;
  for (let nx; (nx = pairAt(p)); p += 16) run.push(nx);
  // A lone pair is weak evidence (any struct with a name and a callback looks like this).
  if (run.length < 2) { stats.single_pairs_ignored++; continue; }
  stats.native_tables++;
  const tableVa = BASE + rdata.rva + (o - ro);
  for (const { name, fn } of run) {
    if (symbols.has(fn)) continue;
    stats.native_names++;
    const tc = tableClass.get(tableVa);
    if (tc) stats.native_names_with_class = (stats.native_names_with_class || 0) + 1;
    add(fn, { name: (tc ? tc.cls + '::' : '') + 'exec' + name, script_name: name, class: tc ? tc.cls : undefined, confidence: 'confirmed',
      evidence: `native function table at ${hex(tableVa)} (${run.length} entries) pairs the string "${name}" with this address` + (tc ? `; the table is registered by func_${tc.fn}, the StaticRegisterNatives function of class ${tc.cls}` : '') });
  }
  o = p - 8;
}

// 2 + 3. from Ghidra's map
const gh = path.join(ROOT, 'build', 'ghidra', 'functions.tsv');
stats.import_thunks = 0; stats.library = 0; stats.exports_and_delayload = 0;
if (fs.existsSync(gh)) {
  for (const l of fs.readFileSync(gh, 'utf8').split('\n').slice(1)) {
    if (!l) continue;
    const [entry, name, source, thunk, , library] = l.split('\t');
    const va = parseInt(entry, 16);
    if (thunk.startsWith('ext:')) { stats.import_thunks++; add(va, { name: thunk.slice(4).replace(/^.*::/, ''), confidence: 'confirmed', evidence: `thunk to import ${thunk.slice(4)} (import table)` }); }
    else if (library === '1') { stats.library++; add(va, { name, confidence: 'inferred', evidence: 'Ghidra Function ID signature match' }); }
    else if (source === 'IMPORTED' && !/^entry$/.test(name)) { stats.exports_and_delayload++; add(va, { name, confidence: 'confirmed', evidence: 'name from the PE export or delay-import data' }); }
  }
}
// 7. jump thunks: a function that is only jmp rel32 to a named function
stats.jump_thunks = 0;
for (const va of starts) {
  if (symbols.has(va)) continue;
  let o = img.rvaToOff(va - BASE);
  if (o < 0 || b[o] !== 0xE9) continue;
  let t = va, hops = 0;
  for (; hops < 8 && inText(t) && b[img.rvaToOff(t - BASE)] === 0xE9; hops++) t = t + 5 + b.readInt32LE(img.rvaToOff(t - BASE) + 1);
  const target = symbols.get(t);
  if (!target) continue;
  addDerived(va, { name: 'j_' + target.name.replace(/@0x[0-9A-F]+$/, ''), confidence: target.confidence, basis: 'jump thunk', evidence: `jmp rel32 to ${target.symbol} (${target.name})` });
  stats.jump_thunks++;
}
// one address per name among the derived names
{
  const count = new Map();
  for (const r of derived) count.set(r.name, (count.get(r.name) || 0) + 1);
  stats.derived_names_suffixed = 0;
  for (const r of derived) if (count.get(r.name) > 1) { r.name += '@' + r.address; stats.derived_names_suffixed++; }
}
// the kind of evidence of the older sources
for (const r of symbols.values()) if (!r.basis) { const e = r.evidence || ''; r.basis = /^native function table/.test(e) ? 'native function table' : /^thunk to import/.test(e) ? 'import thunk' : /Function ID/.test(e) ? 'Ghidra Function ID' : /PE export|delay-import/.test(e) ? 'PE export or import data' : 'other'; }
stats.by_basis = {};
for (const r of symbols.values()) stats.by_basis[r.basis] = (stats.by_basis[r.basis] || 0) + 1;
const list = [...symbols.entries()].sort((a, c) => a[0] - c[0]).map((x) => x[1]);
fs.writeFileSync(OUT, '[\n' + list.map((r) => JSON.stringify(r)).join(',\n') + '\n]\n');
logRun({ tool: 'names', version: VERSION, outputs: [OUT], note: JSON.stringify(stats) });
console.log(JSON.stringify({ ...stats, total: list.length }, null, 1));
console.log(list.filter((r) => r.script_name).filter((_, i) => i % 1500 === 0).map((r) => `${r.address} ${r.name}`).join('\n'));
