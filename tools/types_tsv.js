// Exports the layouts and signatures the sheets know, for Ghidra (method section 8.7: a Ghidra edit is a
// projection of sheet rows).  sheets -> build/types/types.tsv, signatures.tsv, summary.json
//   node tools/types_tsv.js
// types.tsv (tab separated, one record per line; applied by tools/ghidra/ApplyTypes.java):
//   E  name  width                                  an enum datatype of `width` bytes
//   V  enum  value-name  value                      one value of it
//   S  kind  name  size                             a structure of exactly `size` bytes; kind = ue4 | struct | class | params
//   M  struct  offset  name  type  count  bit  comment
//        type: a C scalar (float int uint uchar char short ushort longlong ulonglong double bool),
//              ptr:<struct>, ptr:void, struct:<struct>, enum:<enum>;  count > 1 = array of that many
//        bit:  "-" or the bit number (0-7) of a one-bit field in the byte at `offset`
// Design (docs/reflection.md, "Types for Ghidra"):
//   - a class or struct is written flat: its own members and every inherited member at their absolute offsets
//     (`this->m_X` in the decompiler instead of `this->super.super.m_X`; members a derived class places in its
//     base's tail padding need no special case). The comment of an inherited member names the class it is from.
//   - offset 0 of every class is `vtable` (all reflected classes derive from UObject, which has one).
//   - unreflected bytes are left undefined.
//   - value types (FName, TArray, FString, ...) are small structs with the UE 4.17 x64 sizes of tools/lib/layout.js.
// signatures.tsv (applied by tools/ghidra/ApplySignatures.java):
//   THIS   address  class          member function: __thiscall in class namespace `class` (Ghidra then types `this`)
//   EXEC   address  class          native script function: void C::execX(FFrame& Stack, void* Result)   (UE 4.17 Native)
//   RET0   address  struct         T* f(void)
//   VOID0  address                 void f(void)
//   ICTOR  address                 void f(const FObjectInitializer&)
const fs = require('fs');
const path = require('path');
const { ROOT, logRun, writeJson } = require('./lib/log');
const S = require('./lib/sheets');
const { TYPES } = require('./lib/layout');

const VERSION = '1.0.0';
const DIR = path.join(ROOT, 'build', 'types');
const byKind = S.sheetsByKind();
const ident = (s) => { const x = String(s).replace(/[^A-Za-z0-9_]/g, '_'); return /^[0-9]/.test(x) ? '_' + x : x || '_'; };
const issues = {}; // what in the sheets did not fit; reported, the member is left out
const issue = (k, v) => { const e = issues[k] || (issues[k] = { count: 0, examples: [] }); e.count++; if (e.examples.length < 25) e.examples.push(v); };

// ---- sheets
const classes = S.rowsOf('classes', byKind), structs = S.rowsOf('structs', byKind), enums = S.rowsOf('enums', byKind), functions = S.rowsOf('functions', byKind);
const classById = new Map(classes.map((c) => [c.id, c])), structById = new Map(structs.map((s) => [s.id, s])), enumById = new Map(enums.map((e) => [e.id, e]));
const cppOf = (id) => { const c = classById.get(id); return c ? c.cpp_name || 'U' + c.id : null; };
const structName = (id) => 'F' + id;
const members = new Map(); // "kind:owner" -> [property row]
S.eachRow(byKind.get('properties')[0], (r, h) => {
  const o = Object.fromEntries(h.columns.map((c, i) => [c, r[i]]));
  if (o.owner === null) return;
  const k = `${o.owner_kind}:${o.owner}`;
  (members.get(k) || members.set(k, []).get(k)).push(o);
});
const innerOf = (p) => members.get(`property:${p.id}`) || [];

// ---- names of the Ghidra types
const taken = new Map(); // datatype name -> what it is
const claim = (name, what) => { if (taken.has(name)) { issue('type name used twice', `${name}: ${taken.get(name)} and ${what}`); return false; } taken.set(name, what); return true; };
const UE4 = { // value types of the engine (UE 4.17, x64): size, members [offset, name, type]
  FName: [8, [[0, 'ComparisonIndex', 'int'], [4, 'Number', 'int']]],
  TArray: [16, [[0, 'Data', 'ptr:void'], [8, 'ArrayNum', 'int'], [12, 'ArrayMax', 'int']]],
  FString: [16, [[0, 'Data', 'ptr:void'], [8, 'ArrayNum', 'int'], [12, 'ArrayMax', 'int']]],
  FWeakObjectPtr: [8, [[0, 'ObjectIndex', 'int'], [4, 'ObjectSerialNumber', 'int']]],
  FScriptInterface: [16, [[0, 'ObjectPointer', 'ptr:UObject'], [8, 'InterfacePointer', 'ptr:void']]],
  FScriptDelegate: [16, [[0, 'Object', 'struct:FWeakObjectPtr'], [8, 'FunctionName', 'struct:FName']]],
  FMulticastScriptDelegate: [16, [[0, 'InvocationList', 'struct:TArray']]],
  FText: [24, [[0, 'TextData', 'ptr:void'], [8, 'TextDataReferenceController', 'ptr:void'], [16, 'Flags', 'uint']]],
  TSet: [80, [[0, 'Elements', 'struct:TArray']]],
  TMap: [80, [[0, 'Elements', 'struct:TArray']]],
  FLazyObjectPtr: [28, [[0, 'WeakPtr', 'struct:FWeakObjectPtr'], [8, 'TagAtLastTest', 'int'], [12, 'Guid', 'uint', 4]]],
  FAssetPtr: [32, [[0, 'WeakPtr', 'struct:FWeakObjectPtr'], [8, 'TagAtLastTest', 'int'], [16, 'AssetLongPathname', 'struct:FString']]],
  FFrame: [0, []], FObjectInitializer: [0, []], // only pointed at: left without a layout
};
const PRIM = { FloatProperty: ['float', 4], IntProperty: ['int', 4], UInt32Property: ['uint', 4], ByteProperty: ['uchar', 1], Int8Property: ['char', 1], Int16Property: ['short', 2], UInt16Property: ['ushort', 2],
  Int64Property: ['longlong', 8], UInt64Property: ['ulonglong', 8], DoubleProperty: ['double', 8] };
const VALUE = { NameProperty: 'FName', StrProperty: 'FString', ArrayProperty: 'TArray', WeakObjectProperty: 'FWeakObjectPtr', InterfaceProperty: 'FScriptInterface', DelegateProperty: 'FScriptDelegate',
  MulticastDelegateProperty: 'FMulticastScriptDelegate', TextProperty: 'FText', MapProperty: 'TMap', SetProperty: 'TSet', LazyObjectProperty: 'FLazyObjectPtr', AssetObjectProperty: 'FAssetPtr', AssetClassProperty: 'FAssetPtr' };
for (const [n, [size]] of Object.entries(UE4)) { claim(n, 'engine value type'); if (TYPES[Object.keys(VALUE).find((k) => VALUE[k] === n)] && size !== TYPES[Object.keys(VALUE).find((k) => VALUE[k] === n)][0]) issue('engine value type size differs from tools/lib/layout.js', n); }
for (const c of classes) claim(cppOf(c.id), `class ${c.id}`);
const structOk = new Set();
for (const s of structs) if (claim(structName(s.id), `struct ${s.id}`)) structOk.add(s.id);

// ---- enums: width = how the properties store them
const enumUse = new Map(); // enum -> Map(width -> uses)
const widthOfEnumProp = (p) => (p.type === 'ByteProperty' ? 1 : (() => { const u = innerOf(p)[0]; return u && TYPES[u.type] ? TYPES[u.type][0] : null; })());
for (const list of members.values()) for (const p of list) {
  if ((p.type !== 'ByteProperty' && p.type !== 'EnumProperty') || !p.target || p.target === '-' || !enumById.has(p.target)) continue;
  const w = widthOfEnumProp(p);
  if (w === null) { issue('enum property without a known underlying type', p.id); continue; }
  const m = enumUse.get(p.target) || enumUse.set(p.target, new Map()).get(p.target);
  m.set(w, (m.get(w) || 0) + 1);
}
const enumMain = new Map();
const enumType = (id, w) => (enumMain.get(id) === w ? ident(id) : `${ident(id)}__${w * 8}`);
const L = [];
const stats = { enums: 0, enum_values: 0, enum_extra_widths: 0, engine_types: 0, structs: 0, classes: 0, params_structs: 0, members: 0, inherited_members: 0, bitfields: 0, pointers_to_classes: 0, embedded_structs: 0, enum_members: 0 };
const valuesOf = new Map();
for (const v of S.rowsOf('enum-values', byKind)) (valuesOf.get(v.enum) || valuesOf.set(v.enum, []).get(v.enum)).push(v);
for (const e of enums) {
  const use = enumUse.get(e.id) || new Map([[1, 0]]); // an enum no property uses: one byte (UENUM enum classes are uint8)
  const widths = [...use].sort((a, b) => b[1] - a[1] || a[0] - b[0]).map((x) => x[0]);
  enumMain.set(e.id, widths[0]);
  if (!claim(ident(e.id), `enum ${e.id}`)) continue;
  for (const w of widths) {
    const name = enumType(e.id, w);
    if (w !== widths[0]) { stats.enum_extra_widths++; claim(name, `enum ${e.id} stored in ${w} bytes`); issue('enum stored with more than one width', `${e.id}: ${[...use].map(([k, n]) => `${k} byte x${n}`).join(', ')}`); }
    L.push(['E', name, w].join('\t'));
    stats.enums++;
    for (const v of valuesOf.get(e.id) || []) { if (typeof v.value !== 'number') { issue('enum value without a number', v.id); continue; } L.push(['V', name, v.name, v.value].join('\t')); stats.enum_values++; }
  }
}

// ---- engine value types
for (const [n, [size, ms]] of Object.entries(UE4)) {
  L.push(['S', 'ue4', n, size].join('\t')); stats.engine_types++;
  for (const [off, name, type, count] of ms) L.push(['M', n, off, name, type, count || 1, '-', ''].join('\t'));
}

// ---- one member line per property
function typeOf(p) { // -> { type, size, comment } or null
  if (PRIM[p.type] && !(p.type === 'ByteProperty' && p.target && p.target !== '-')) return { type: PRIM[p.type][0], size: PRIM[p.type][1], comment: '' };
  if (p.type === 'ByteProperty' || p.type === 'EnumProperty') {
    const w = widthOfEnumProp(p);
    if (w === null) return null;
    if (!enumById.has(p.target) || !taken.get(ident(p.target)) || !taken.get(ident(p.target)).startsWith('enum')) return { type: { 1: 'uchar', 2: 'ushort', 4: 'uint', 8: 'ulonglong' }[w], size: w, comment: `enum ${p.target}` };
    stats.enum_members++;
    return { type: 'enum:' + enumType(p.target, w), size: w, comment: '' };
  }
  if (VALUE[p.type]) {
    const inner = innerOf(p);
    const desc = (x) => `${x.type.replace(/Property$/, '')}${x.target && x.target !== '-' ? ' ' + x.target : ''}`;
    const c = p.type === 'ArrayProperty' && inner[0] ? `TArray<${desc(inner[0])}>` : p.type === 'MapProperty' && inner.length === 2 ? `TMap<${desc(inner[0])}, ${desc(inner[1])}>` : p.type === 'SetProperty' && inner[0] ? `TSet<${desc(inner[0])}>`
      : p.target && p.target !== '-' ? `${p.type.replace(/Property$/, '')} ${p.target}` : '';
    return { type: 'struct:' + VALUE[p.type], size: UE4[VALUE[p.type]][0], comment: c };
  }
  if (p.type === 'ObjectProperty') { const t = p.target && classById.has(p.target) ? cppOf(p.target) : 'UObject'; stats.pointers_to_classes++; return { type: 'ptr:' + t, size: 8, comment: '' }; }
  if (p.type === 'ClassProperty') return { type: 'ptr:UClass', size: 8, comment: p.target && p.target !== '-' ? `TSubclassOf<${cppOf(p.target) || p.target}>` : '' };
  if (p.type === 'StructProperty') {
    const s = structById.get(p.target);
    if (!s || !structOk.has(s.id) || typeof s.size !== 'number') return null;
    stats.embedded_structs++;
    return { type: 'struct:' + structName(s.id), size: s.size, comment: '' };
  }
  return null;
}
function emitStruct(kind, name, size, chain) { // chain: [{ from, props }] base first
  L.push(['S', kind, name, size].join('\t'));
  const used = []; // [start, end)
  const names = new Set();
  const place = (off, len, line, who) => {
    if (off < 0 || off + len > size) { issue('member outside its owner', `${who} at ${off}+${len} in ${name} (${size})`); return false; }
    if (used.some(([a, b]) => off < b && off + len > a)) { issue('member overlaps another member', `${who} at ${off}+${len} in ${name}`); return false; }
    used.push([off, off + len]); L.push(line); stats.members++; return true;
  };
  const uniq = (n, off) => { let x = ident(n); if (names.has(x)) x += '_0x' + off.toString(16); for (let i = 2; names.has(x); i++) x = `${ident(n)}_0x${off.toString(16)}_${i}`; names.add(x); return x; };
  if (kind === 'class') place(0, 8, ['M', name, 0, uniq('vtable', 0), 'ptr:void', 1, '-', 'virtual function table'].join('\t'), 'vtable');
  for (const { from, props } of chain) {
    const inherited = from !== null;
    // bools: one byte may hold several one-bit members
    const boolBytes = new Map();
    for (const p of props) if (p.type === 'BoolProperty' && typeof p.offset === 'number') (boolBytes.get(p.offset) || boolBytes.set(p.offset, []).get(p.offset)).push(p);
    for (const p of [...props].sort((a, b) => (a.offset ?? 0) - (b.offset ?? 0) || a.ord - b.ord)) {
      if (typeof p.offset !== 'number') { issue('member without an offset', p.id); continue; }
      const cmt = (c) => [c, inherited ? `from ${from}` : ''].filter(Boolean).join('; ');
      if (p.type === 'BoolProperty') {
        const group = boolBytes.get(p.offset), mask = typeof p.mask === 'string' ? parseInt(p.mask, 16) : null;
        if (mask === null) { issue('bool without a mask', p.id); continue; }
        const single = (mask & (mask - 1)) === 0;
        if (group.length === 1 && (mask === 1 || mask === 0xFF)) { if (place(p.offset, 1, ['M', name, p.offset, uniq(p.name, p.offset), 'bool', 1, '-', cmt('')].join('\t'), p.id) && inherited) stats.inherited_members++; continue; }
        if (!single) { issue('bool mask is not one bit', `${p.id} ${p.mask}`); continue; }
        // the byte is claimed once; each bit is its own line
        if (group[0] === p) { if (used.some(([a, b]) => p.offset < b && p.offset + 1 > a)) { issue('member overlaps another member', `${p.id} (bit field byte) in ${name}`); group.blocked = true; } else used.push([p.offset, p.offset + 1]); }
        if (group.blocked) continue;
        L.push(['M', name, p.offset, uniq(p.name, p.offset), 'uchar', 1, Math.log2(mask), cmt(`mask ${p.mask}`)].join('\t')); stats.members++; stats.bitfields++; if (inherited) stats.inherited_members++;
        continue;
      }
      const t = typeOf(p);
      if (!t) { issue('member type not expressible', `${p.id} (${p.type}${p.target && p.target !== '-' ? ' ' + p.target : ''})`); continue; }
      const dim = typeof p.array_dim === 'number' && p.array_dim > 1 ? p.array_dim : 1;
      if (place(p.offset, t.size * dim, ['M', name, p.offset, uniq(p.name, p.offset), t.type, dim, '-', cmt(t.comment)].join('\t'), p.id) && inherited) stats.inherited_members++;
    }
  }
}
const chainOf = (kind, id, byId) => { const out = []; for (let x = byId.get(id), i = 0; x && i < 64; x = x.super && x.super !== '-' ? byId.get(x.super) : null, i++) out.unshift({ from: x.id === id ? null : x.id, props: members.get(`${kind}:${x.id}`) || [] }); return out; };
for (const s of structs) { if (!structOk.has(s.id) || typeof s.size !== 'number') { issue('struct without a size or a usable name', s.id); continue; } emitStruct('struct', structName(s.id), s.size, chainOf('struct', s.id, structById)); stats.structs++; }
for (const c of classes) { if (typeof c.size !== 'number') { issue('class without a size', c.id); continue; } emitStruct('class', cppOf(c.id), c.size, chainOf('class', c.id, classById)); stats.classes++; }
// parameter structs of script functions
for (const f of functions) {
  const props = members.get(`function:${f.id}`) || [];
  if (!props.length || typeof f.params_size !== 'number' || f.params_size === 0) continue;
  const scope = f.owner && f.owner !== '-' ? cppOf(f.owner) : ident(String(f.id).split('.')[0].replace(/^\/Script\//, ''));
  const name = `${scope}_${ident(f.name)}_Params`;
  if (!claim(name, `parameters of ${f.id}`)) continue;
  emitStruct('params', name, f.params_size, [{ from: null, props }]);
  stats.params_structs++;
}

// ---- signatures
const G = [];
const sig = { THIS_constructors: 0, THIS_virtual: 0, EXEC: 0, RET0: 0, VOID0: 0, ICTOR: 0, skipped: {} };
const skip = (k) => { sig.skipped[k] = (sig.skipped[k] || 0) + 1; };
const cppSet = new Set(classes.map((c) => cppOf(c.id)));
const done = new Set();
const A = (a) => a.replace(/^0x/i, '').toUpperCase();
const put = (line, a) => { if (done.has(a)) { skip('address already has a signature line'); return false; } done.add(a); G.push(line); return true; };
const execAt = new Set();
for (const e of S.rowsOf('exec', byKind)) { if (!e.class || !classById.has(e.class)) { skip('exec function without a class'); continue; } if (put(['EXEC', A(e.ref_addr), cppOf(e.class)].join('\t'), A(e.ref_addr))) { sig.EXEC++; execAt.add(A(e.ref_addr)); } }
const RET = [[/::GetPrivateStaticClass$/, 'UClass'], [/^Z_Construct_UClass_/, 'UClass'], [/^Z_Construct_UScriptStruct_/, 'UScriptStruct'], [/^Z_Construct_UEnum_/, 'UEnum'], [/^Z_Construct_UFunction_/, 'UFunction'], [/^Z_Construct_UDelegateFunction_/, 'UFunction'], [/^Z_Construct_UPackage_/, 'UPackage']];
for (const n of S.rowsOf('names', byKind)) {
  const name = n.name.replace(/@0x[0-9A-Fa-f]+$/, ''), a = A(n.ref_addr);
  if (n.basis === 'class constructor chain') { const c = name.slice(0, name.indexOf('::')); if (cppSet.has(c)) { if (put(['THIS', a, c].join('\t'), a)) sig.THIS_constructors++; } else skip('constructor of an unknown class'); continue; }
  if (n.basis !== 'reflection registration') continue;
  const r = RET.find(([re]) => re.test(name));
  if (r) { if (put(['RET0', a, r[1]].join('\t'), a)) sig.RET0++; continue; }
  if (/^InternalConstructor</.test(name)) { if (put(['ICTOR', a].join('\t'), a)) sig.ICTOR++; continue; }
  if (/::StaticRegisterNatives/.test(name)) { if (put(['VOID0', a].join('\t'), a)) sig.VOID0++; continue; }
}
if (byKind.has('vfuncs')) for (const v of S.rowsOf('vfuncs', byKind)) {
  if (v.owner_kind !== 'class' || v.class === '-' || !classById.has(v.class)) { skip(v.owner_kind === 'rtti' ? 'virtual function of an RTTI class (no layout in the sheets)' : 'virtual function without one owning class'); continue; }
  const a = A(v.ref_addr);
  if (done.has(a)) { skip('virtual function that already has a signature line'); continue; }
  put(['THIS', a, cppOf(v.class)].join('\t'), a); sig.THIS_virtual++;
}

fs.mkdirSync(DIR, { recursive: true });
const T = path.join(DIR, 'types.tsv'), SG = path.join(DIR, 'signatures.tsv'), SUM = path.join(DIR, 'summary.json');
fs.writeFileSync(T, '# tools/types_tsv.js from the sheets; apply with tools/ghidra/ApplyTypes.java\n' + L.join('\n') + '\n');
fs.writeFileSync(SG, '# tools/types_tsv.js from the sheets; apply with tools/ghidra/ApplySignatures.java (after ApplyNames and ApplyTypes)\n' + G.join('\n') + '\n');
writeJson(SUM, { generated_by: 'tools/types_tsv.js', version: VERSION, types: stats, signatures: sig, issues });
logRun({ tool: 'types_tsv', version: VERSION, outputs: [T, SG, SUM], note: JSON.stringify({ ...stats, ...sig, skipped: undefined }) });
console.log(JSON.stringify({ types: stats, signatures: sig }, null, 1));
for (const [k, v] of Object.entries(issues)) console.log(`issue: ${k}: ${v.count}  e.g. ${v.examples.slice(0, 3).join(' | ')}`);
