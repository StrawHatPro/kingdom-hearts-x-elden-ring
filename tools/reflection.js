// Recover Unreal reflection data from the game's own registration code.
// UE 4.17 builds every reflected class, struct, enum and script function at start-up with
// generated functions (StaticClass getters, Z_Construct_UClass_*, Z_Construct_UScriptStruct_*,
// Z_Construct_UEnum_*, Z_Construct_UFunction_*, StaticRegisterNatives*). That code is in the
// executable, compiled without optimisation, and passes every fact as a constant or as the result
// of another such function. This tool runs each of those functions through a small emulator
// (tools/lib/emu.js: concrete values, calls recorded but not entered) and reads the arguments of
// the engine calls they make:
//   GetPrivateStaticClassBody(Package, Name, &Class, RegisterNatives, sizeof, ClassFlags, CastFlags,
//       Config, ..., SuperClassFn, WithinClassFn, ...)            class, size, flags, super, within
//   operator new(size, EInternal, Outer, FName, RF_ flags) of each UProperty / UFunction /
//       UScriptStruct / UEnum class, then that class's constructor  every reflected object
//   U<Type>Property(init, EC_CppProperty, Offset, Flags [, target getter result ...])
//   FDetermineBitfieldOffsetAndMask (bool members): the setter the registration code installs is
//       itself emulated on a zeroed buffer, exactly as the engine does, giving byte offset and mask
//   FindExistingStructIfHotReloadOrDynamic(Outer, Name, sizeof, crc), UScriptStruct(init, Super, Ops, flags)
//   FindExistingEnumIfHotReloadOrDynamic(Outer, Name, ...), TPair<FName,int64>(Name, value), SetEnums(names, form)
//   FNativeFunctionRegistrar::RegisterFunctions(Class, table, count)   native exec functions
//   UClass::AddFunctionToFunctionMap(Class, Function, Name)
// Nothing is guessed: a value the code does not show stays null.
//   node tools/reflection.js
// Output: audit/reflection.json  (one record per class / struct / enum / function)
//         audit/reflection_stats.json (counts and the cross-checks made here)
const fs = require('fs');
const path = require('path');
const { ROOT, logRun, writeJson } = require('./lib/log');

const VERSION = '2.0.0';
const OUT = path.join(ROOT, 'audit', 'reflection.json');
const STATS = path.join(ROOT, 'audit', 'reflection_stats.json');
const E = require('./lib/emu').create();
const { BASE } = E;
const X = (n) => (n === null || n === undefined ? null : '0x' + n.toString(16).toUpperCase());

// Engine functions the registration code calls (addresses in this executable; each was identified
// from its callers' argument shapes and its own body, see docs/reflection.md).
const K = {
  GPSCB: 0x142E54B80,          // GetPrivateStaticClassBody
  SAO: 0x142F78270,            // StaticAllocateObject
  FIND_STRUCT: 0x142F6AD10,    // FindExistingStructIfHotReloadOrDynamic
  FIND_ENUM: 0x142F6AC30,      // FindExistingEnumIfHotReloadOrDynamic
  REG_NATIVES: 0x142E58EA0,    // FNativeFunctionRegistrar::RegisterFunctions
  FORCE_REG: 0x142F7C280,      // UObjectForceRegistration
  FNAME_W: 0x142DD02C0,        // FName(const wchar_t*, EFindName)
  FNAME_A: 0x142DD0240,        // FName(const char*, EFindName)
  BITFIELD: 0x14180E000,       // FDetermineBitfieldOffsetAndMask::DoDetermineBitfieldOffsetAndMask(size)
  INIT_THREAD_HEADER: 0x14586FE44,
  CHKSTK: 0x1458701C0,
  COOKIE: 0x145870110,         // __security_check_cookie
  ADD_FN_MAP: 0x14180CE80,     // UClass::AddFunctionToFunctionMap(Function, Name)
  UCLASS_CTOR: 0x142E4BB80,    // UClass(EStaticConstructor, Name, Size, ClassFlags, CastFlags, Config, ObjectFlags, ctor, vtable helper, AddReferencedObjects)
  REGISTER: 0x142E38D80,       // UObjectBase::Register(PackageName, Name): follows UCLASS_CTOR where GetPrivateStaticClassBody is inlined
};
for (const k of Object.keys(K)) K[k] = E.resolveThunk(K[k]);

// ---------------------------------------------------------------- callers of a set of functions
const text = E.secs[0];
function callersOf(targets) {
  const b = E.img.b, lo = text.raw, hi = text.raw + (text.rawHi - text.lo) - 5;
  const out = new Set();
  for (let o = lo; o < hi; o++) {
    if (b[o] !== 0xE8 && b[o] !== 0xE9) continue;
    const va = text.lo + (o - lo);
    const t = va + 5 + b.readInt32LE(o + 1);
    if (t < text.lo || t >= text.rawHi) continue;
    if (!targets.has(t) && !(b[text.raw + (t - text.lo)] === 0xE9 && targets.has(E.resolveThunk(t)))) continue;
    const f = E.funcStart(va);
    if (f !== null) out.add(f);
  }
  return out;
}

// ---------------------------------------------------------------- emulation with hooks
const BUF = 0x200000000000n, HEAP_LO = Number(require('./lib/emu').HEAP);
const bitfieldCache = new Map(); // vtable -> { offset, mask }
function bitfield(vt, size) {
  if (bitfieldCache.has(vt)) return bitfieldCache.get(vt);
  let out = null;
  const setter = Number(E.u64(vt));
  if (E.inText(setter)) {
    // the engine calls the setter on a zeroed buffer of sizeof(owner) bytes and takes the first non-zero byte
    const writes = new Map();
    const r = E.run(E.resolveThunk(setter), { regs: { 1: BUF - 0x1000n, 2: BUF, 8: 1n }, budget: 400, zero: [BUF, BUF + 0x100000n], onWrite: (a, v) => writes.set(a, v) });
    if (r.end === 'ret' && r.calls.length === 0) {
      const inBuf = [...writes].filter(([a]) => a >= Number(BUF) && a < Number(BUF) + 0x100000);
      const nz = inBuf.filter(([, v]) => v > 0).sort((p, q) => p[0] - q[0]);
      if (nz.length && inBuf.every(([, v]) => v >= 0)) out = { offset: nz[0][0] - Number(BUF), mask: nz[0][1], setter, bytes: nz.length };
    }
  }
  bitfieldCache.set(vt, out);
  return out;
}

let ctx = null; // the function being emulated
const allocators = new Map(); // allocator function -> UClass getter of the class it allocates
const hooks = new Map();
hooks.set(K.CHKSTK, (c, st) => st.regs[0]);
hooks.set(K.COOKIE, (c, st) => st.regs[0]);
hooks.set(K.INIT_THREAD_HEADER, (c, st) => { if (c.a[0] !== null) st.wr(c.a[0], 4, 0xFFFFFFFFn); return 0n; });
const fname = (c, st) => {
  const s = c.a[1] !== null ? E.str(Number(c.a[1])) : null;
  const tok = st.fresh({ fname: s, at: c.a[1] === null ? null : Number(c.a[1]) });
  if (c.a[0] !== null) st.wr(c.a[0], 8, tok);
  if (ctx && s && s.startsWith('/Script/') && !ctx.script) ctx.script = s;
  return c.a[0] === null ? undefined : c.a[0];
};
hooks.set(K.FNAME_W, fname); hooks.set(K.FNAME_A, fname);
const bitfieldHook = (c, st, vt0, size0) => {
  const obj = c.a[0];
  const vt = vt0 !== undefined ? vt0 : obj === null ? null : st.rd(obj, 8);
  if (vt0 !== undefined) { c = { a: [obj, size0] }; if (obj !== null) st.wr(obj, 8, vt0); }
  const bf = vt !== null && E.inImage(Number(vt)) ? bitfield(Number(vt), c.a[1]) : null;
  ctx.bitfields++;
  if (bf && obj !== null) { st.wr(obj + 8n, 4, BigInt(bf.offset)); st.wr(obj + 12n, 4, BigInt(bf.mask)); if (c.a[1] !== null && BigInt(bf.offset) >= c.a[1]) ctx.notes.push(`bitfield offset ${bf.offset} outside size ${c.a[1]}`); }
  else { ctx.bitfield_fail++; if (obj !== null) { st.wr(obj + 8n, 4, null); st.wr(obj + 12n, 4, null); } }
  return vt0 !== undefined ? obj : 0n;
};
hooks.set(K.BITFIELD, (c, st) => bitfieldHook(c, st));
hooks.set(K.GPSCB, (c, st) => {
  const a = [...c.a, ...c.sq];
  const pre = a[1] !== null && E.u32(Number(a[1]) - 4) >>> 16 !== 0 && (E.u32(Number(a[1]) - 4) & 0xFFFF) === 0 ? String.fromCharCode(E.u32(Number(a[1]) - 4) >>> 16) : null; // the character before the name, when the string starts there
  const info = { prefix: pre && /^[A-Z]$/.test(pre) ? pre : null, package: a[0], name: a[1] !== null ? E.wstr(Number(a[1])) : null, register_natives: a[3], size: c.sd[0], class_flags: c.sd[1], cast_flags: a[6], config: a[7],
    class_constructor: a[8], vtable_helper: a[9], add_referenced_objects: a[10], super_fn: a[11], within_fn: a[12] };
  const tok = st.fresh({ cls: info });
  if (c.a[2] !== null) st.wr(c.a[2], 8, tok);
  ctx.gpscb = info; info.ptr = c.a[2];
  return 0n;
});
// Core classes built by hand-written code (IMPLEMENT_CORE_INTRINSIC_CLASS: Class, Struct, Field, Function ...) have
// GetPrivateStaticClassBody inlined into their StaticClass getter: UClass ctor, WithinClass getter, SuperClass getter,
// Register(Package, Name), SetSuperStruct(Super) through the vtable, ClassWithin stored at +0xD8.
hooks.set(K.UCLASS_CTOR, (c, st) => {
  const nr = E.recOf(c.a[2]);
  if (ctx.gpscb || !nr || typeof nr.fname !== 'string' || ctx.fn === K.GPSCB) return undefined;
  const info = { inline: true, package: null, name: nr.fname, register_natives: null, size: c.a[3] === null ? null : c.a[3] & 0xFFFFFFFFn, class_flags: c.sd[0], cast_flags: c.sq[1], config: c.sq[2],
    class_constructor: c.sq[4], vtable_helper: c.sq[5], add_referenced_objects: c.sq[6], super_fn: null, within_fn: null, ptr: null };
  ctx.gpscb = info; info.tok = st.fresh({ cls: info });
  return info.tok;
});
hooks.set(K.REGISTER, (c) => { if (ctx.gpscb && ctx.gpscb.inline && ctx.gpscb.package === null) { ctx.gpscb.package = c.a[0]; ctx.gpscb.register_name = c.a[1] !== null ? E.wstr(Number(c.a[1])) : null; } return 0n; });
hooks.set(K.FIND_STRUCT, (c) => { ctx.findStruct = { outer: c.a[0], name: c.a[1] !== null ? E.wstr(Number(c.a[1])) : null, size: c.a[2] }; return 0n; }); // "not found": the function builds the struct
hooks.set(K.FIND_ENUM, (c) => { ctx.findEnum = { outer: c.a[0], name: c.a[1] !== null ? E.wstr(Number(c.a[1])) : null }; return 0n; });
hooks.set(K.REG_NATIVES, (c) => { ctx.regNatives.push({ cls: c.a[0], table: c.a[1], count: c.a[2] }); return 0n; });
hooks.set(K.FORCE_REG, (c) => { ctx.forceReg.push(c.a[0]); return 0n; });
hooks.set(K.ADD_FN_MAP, (c) => { ctx.fnMap.push({ cls: c.a[0], fn: c.a[1], name: c.a[2] }); return 0n; });
const allocHook = (c, st) => {
  const o = { id: ctx.objs.length, getter: allocators.get(c.target), alloc: c.target, alloc_size: c.a[0], outer: c.a[2], name: c.a[3], flags: c.sd[0], ctor: null, ctx };
  o.ptr = st.fresh({ obj: o });
  ctx.objs.push(o);
  return o.ptr;
};
hooks.set('*', (c, st) => {
  const r = E.recOf(c.a[0]);
  if (c.indirect && ctx.gpscb && ctx.gpscb.inline && c.a[0] === ctx.gpscb.tok && ctx.gpscb.super_tok === undefined && ctx.gpscb.package !== null) ctx.gpscb.super_tok = c.a[1];
  if (r && r.obj && r.obj.ctx === ctx && !r.obj.ctor) { r.obj.ctor = { fn: c.target, a: c.a, sq: c.sq, sd: c.sd, sb: c.sb, site: c.site }; return c.a[0]; }
  if (ctx.findEnum && c.target !== null && c.a[1] !== null && E.inImage(Number(c.a[1])) && E.isStack(c.a[2]) && E.isStack(c.a[0]) === false) {
    const s = E.wstr(Number(c.a[1]));
    if (s && /^[\x20-\x7e]+$/.test(s)) ctx.pairs.push({ name: s, value: c.dd[2], value8: c.dq[2], fn: c.target });
  }
  if (c.indirect && ctx.findEnum && c.disp === 0x260 && !ctx.setEnums) ctx.setEnums = { obj: c.a[0], form: c.a[2], add_max: c.a[3] };
  ctx.order.push(c.target);
  return undefined;
});
hooks.set('indirect', hooks.get('*'));

const analyses = new Map();
function analyze(fn, budget = 4000000) {
  fn = E.resolveThunk(fn);
  if (analyses.has(fn)) return analyses.get(fn);
  const A = { fn, objs: [], gpscb: null, findStruct: null, findEnum: null, pairs: [], setEnums: null, regNatives: [], forceReg: [], fnMap: [], order: [], script: null, bitfields: 0, bitfield_fail: 0, notes: [] };
  analyses.set(fn, A);
  const saved = ctx; ctx = A;
  let r;
  const hw = new Set();
  try { r = E.run(fn, { hooks, budget, onWrite: (a) => { if (a >= HEAP_LO) hw.add(a); } }); } catch (e) { r = { end: 'error:' + e.message, ret: null, unknown_branches: 0, steps: 0, mem: null }; }
  ctx = saved;
  A.end = r.end; A.ret = r.ret; A.unknown_branches = r.unknown_branches; A.steps = r.steps;
  // fields the registration code writes into an object after constructing it (ArrayDim, RepNotifyFunc ...)
  // fields the registration code writes into an object after constructing it (ArrayDim, RepNotifyFunc ...)
  if (r.mem) for (const a of hw) { const rec = E.recAt(BigInt(a)); if (rec && rec.obj && rec.obj.ctx === A) { const off = (a - Number(rec.obj.ptr)) & ~3; (rec.obj.written = rec.obj.written || {})[off] = r.mem.rd(rec.obj.ptr + BigInt(off), 4); if (off === 0x60) rec.obj.rep_notify = r.mem.rd(rec.obj.ptr + 0x60n, 8); } }
  if (r.mem && A.gpscb && A.gpscb.inline) A.gpscb.within_tok = r.mem.rd(A.gpscb.tok + 0xD8n, 8);
  A.order = A.order.slice(0, 6);
  return A;
}

// ---------------------------------------------------------------- 1. allocators
const t0 = Date.now();
const log = (s) => console.log(`[${((Date.now() - t0) / 1000).toFixed(1)}s] ${s}`);
{
  const M = [0, 1, 2, 3, 4].map((i) => E.fresh({ marker: i }));
  for (const f of callersOf(new Set([K.SAO]))) {
    const r = E.run(f, { regs: { 1: M[0], 2: M[1], 8: M[2], 9: M[3] }, budget: 200, init: (st) => st.wr(st.regs[4] + 0x28n, 8, 0x77n), hooks: new Map([[K.SAO, (c) => 0n]]) });
    const sao = r.calls.find((c) => c.target === K.SAO);
    if (!sao || r.calls.length !== 2 || r.end !== 'ret') continue;
    const g = E.recOf(sao.a[0]);
    // operator new(size, EInternal, Outer, Name, Flags): StaticAllocateObject(T::StaticClass(), Outer, Name, Flags, ...)
    if (g && g.call && sao.a[1] === M[2] && sao.a[2] === M[3] && sao.a[3] === 0x77n) { allocators.set(f, g.call); hooks.set(f, allocHook); }
  }
}
// Some bitfield probes have an out-of-line constructor: ctor(this) { base ctor; vtable = &X; DoDetermineBitfieldOffsetAndMask(sizeof) }
let bitfieldCtors = 0;
for (const f of callersOf(new Set([K.BITFIELD]))) {
  let cap = null;
  const r = E.run(f, { regs: { 1: BUF }, budget: 60, hooks: new Map([[K.BITFIELD, (c, st) => { cap = { obj: c.a[0], vt: c.a[0] === null ? null : st.rd(c.a[0], 8), size: c.a[1] }; return 0n; }]]) });
  if (r.end === 'ret' && r.calls.length <= 3 && cap && cap.obj === BUF && cap.vt !== null && E.inImage(Number(cap.vt)) && r.ret === BUF) { hooks.set(f, (c, st) => bitfieldHook(c, st, cap.vt, cap.size)); bitfieldCtors++; }
}
log(`allocators: ${allocators.size}, out-of-line bitfield probe constructors: ${bitfieldCtors}`);

// ---------------------------------------------------------------- 2. emulate every registration function
const targets = new Set([K.GPSCB, K.UCLASS_CTOR, K.FIND_STRUCT, K.FIND_ENUM, K.REG_NATIVES, K.FORCE_REG, ...allocators.keys()]);
const cands = [...callersOf(targets)].sort((a, b) => a - b);
log(`candidate functions: ${cands.length}`);
for (const f of cands) if (!allocators.has(f)) analyze(f);
log(`emulated: ${analyses.size}`);

// ---------------------------------------------------------------- 3. entities
// Every getter in the generated code returns one object: a class (through its StaticClass getter),
// or the struct / enum / function / package the function itself constructs. Wrappers
// (Z_Construct_UClass_X_NoRegister, thunks) return the result of another getter.
const entities = new Map();
const classByGetter = new Map(); // getter fn -> class record (built below)
const KIND = { ScriptStruct: 'struct', Function: 'function', DelegateFunction: 'delegatefunction', Enum: 'enum', UserDefinedEnum: 'enum', Package: 'package' };
const constStr = (v) => { // a string constant, or the result of a function that returns one
  if (v === null || v === undefined) return null;
  const r = E.recOf(v);
  if (r && r.call) { const A = analyze(r.call, 2000); return A.ret !== null && E.inImage(Number(A.ret)) ? E.str(Number(A.ret)) : null; }
  return E.inImage(Number(v)) ? E.str(Number(v)) : null;
};
const constNum = (v) => { // a constant, or the result of a function that returns one
  const r = E.recOf(v);
  if (!r) return v ?? null;
  if (r.call === undefined || r.call === null) return null;
  const A = analyze(r.call, 2000);
  return A.ret !== null && !E.recOf(A.ret) && A.end === 'ret' && A.steps < 20 ? A.ret : null;
};
const className = (getter) => { const A = analyze(getter, 20000); return A.gpscb ? A.gpscb.name : null; };
function entityOf(fn, depth = 0) {
  if (fn === null || fn === undefined) return null;
  fn = E.resolveThunk(fn);
  if (entities.has(fn)) return entities.get(fn);
  entities.set(fn, null);
  let e = null;
  if (E.inText(fn)) {
    const A = analyze(fn, 60000);
    const r = E.recOf(A.ret);
    if (A.gpscb && r && r.cls === A.gpscb) e = { kind: 'class', name: A.gpscb.name, fn };
    else if (r && r.obj && r.obj.ctx === A) { const oc = className(r.obj.getter); e = { kind: KIND[oc] || 'object', object_class: oc, name: nameOf(r.obj.name), fn }; }
    else if (r && r.cls) e = { kind: 'class', name: r.cls.name, fn };
    else if (r && r.call !== undefined && r.call !== null && depth < 6) {
      const inner = entityOf(r.call, depth + 1);
      if (inner) e = inner;
      else if (A.script && !A.objs.length) e = { kind: 'package', name: A.script, fn };
    }
  }
  entities.set(fn, e);
  return e;
}
function nameOf(v) { const r = E.recOf(v); return r && r.fname !== undefined ? r.fname : null; }
const useOf = new Map(); // function -> how its result is used by the registration code
const noteUse = (fn, use) => { if (fn === null || fn === undefined || !use) return; const k = E.resolveThunk(Number(fn)); (useOf.get(k) || useOf.set(k, new Set()).get(k)).add(use); };
const entOfVal = (v, use) => { const r = E.recOf(v); if (r && r.call !== undefined && r.call !== null) noteUse(r.call, use); return r && r.call !== undefined && r.call !== null ? entityOf(r.call) : r && r.cls ? { kind: 'class', name: r.cls.name } : null; };
const ref = (e) => (e ? { kind: e.kind, name: e.name, fn: X(e.fn) } : null);

// ---------------------------------------------------------------- 4. records
const stats = { emulated: 0, allocators: allocators.size, classes: 0, classes_with_construct: 0, structs: 0, enums: 0, enum_values: 0, functions: 0, delegates: 0, properties: 0, bool_properties: 0, bool_without_offset: 0,
  unnamed_objects: 0, objects_without_ctor: 0, properties_without_target: 0, natives: 0, native_tables: 0, unknown_branches: 0, not_returned: 0, by_property_class: {}, written_fields: {}, pair_functions: {}, checks: {} };
const records = [];
// values in evidence: numbers as hex; the result of a call as "call:0x<function>"; an object built by the same function as "object:<id>"
const hexv = (v) => { if (v === null || v === undefined) return null; const r = E.recOf(v); return r ? (r.obj ? 'object:' + r.obj.id : r.call !== undefined && r.call !== null ? 'call:' + X(E.resolveThunk(r.call)) : r.cls ? 'class:' + r.cls.name : null) : '0x' + v.toString(16).toUpperCase(); };
const TARGET = { // which constructor argument (after init, EC_CppProperty, Offset, Flags) names another object
  ObjectProperty: 'class', WeakObjectProperty: 'class', LazyObjectProperty: 'class', AssetObjectProperty: 'class', InterfaceProperty: 'class', ClassProperty: 'class', AssetClassProperty: 'class',
  StructProperty: 'struct', ByteProperty: 'enum', EnumProperty: 'enum', DelegateProperty: 'function', MulticastDelegateProperty: 'function',
};
function objects(A) {
  const out = [];
  for (const o of A.objs) {
    const oc = className(o.getter);
    const x = { id: o.id, name: nameOf(o.name), alloc: X(o.alloc), alloc_size: o.alloc_size === null ? null : Number(o.alloc_size), object_flags: hexv(o.flags), ctor: o.ctor ? X(o.ctor.fn) : null, object_class: oc };
    const or = E.recOf(o.outer);
    if (or && or.obj && or.obj.ctx === A) x.outer = or.obj.id;
    else { const e = entOfVal(o.outer); x.outer_ref = e ? ref(e) : null; x.outer_getter = or && or.call ? X(or.call) : null; }
    if (x.name === null) stats.unnamed_objects++;
    if (o.written) {
      for (const k of Object.keys(o.written)) { const key = oc + '+0x' + Number(k).toString(16); stats.written_fields[key] = (stats.written_fields[key] || 0) + 1; }
      // UProperty::ArrayDim is at +0x48 and RepNotifyFunc at +0x60 in this build (UObject is 0x40 bytes); the generated code assigns them after construction
      if (/Property$/.test(oc || '')) { if (o.written[0x48] !== undefined) x.array_dim = o.written[0x48] === null ? null : Number(o.written[0x48]); if (o.rep_notify !== undefined) x.rep_notify = nameOf(o.rep_notify); }
    }
    if (!o.ctor) { stats.objects_without_ctor++; out.push(x); continue; }
    const c = o.ctor;
    // constructor arguments after (memory, FObjectInitializer): 3rd, 4th, then the stack arguments that the object class is known to take
    x.args = [hexv(c.a[2]), hexv(c.a[3]), hexv(oc === 'Function' || oc === 'DelegateFunction' || oc === 'ScriptStruct' ? c.sd[0] : c.sq[0])];
    if (oc === 'BoolProperty') x.args.push(hexv(c.sd[1]), hexv(c.sd[2]), hexv(c.sb[3]));
    else if (TARGET[oc]) { x.args.push(hexv(c.sq[1])); if (oc === 'ClassProperty') x.args.push(hexv(c.sq[2])); }
    else if (oc === 'Function' || oc === 'DelegateFunction') x.args.push(hexv(c.sd[1] === null ? null : c.sd[1] & 0xFFFFn));
    if (/Property$/.test(oc || '')) {
      stats.properties++; stats.by_property_class[oc] = (stats.by_property_class[oc] || 0) + 1;
      x.offset = c.a[3] === null ? null : Number(c.a[3] & 0xFFFFFFFFn);
      x.property_flags = hexv(c.sq[0]);

      if (oc === 'BoolProperty') {
        stats.bool_properties++;
        x.mask = c.sd[1] === null ? null : Number(c.sd[1]);
        x.element_size = c.sd[2] === null ? null : Number(c.sd[2]);
        x.native_bool = c.sb[3] === null ? null : Number(c.sb[3]);
        if (x.offset === null || x.mask === null) stats.bool_without_offset++;
      } else if (TARGET[oc]) {
        const v = c.sq[1];
        if (v === 0n) x.target = '-';
        else { const e = entOfVal(v, 'target'); x.target = e ? ref(e) : null; if (!e) { stats.properties_without_target++; const r = E.recOf(v); x.target_getter = r && r.call ? X(r.call) : hexv(v); } }
        if (oc === 'ClassProperty') { const e2 = entOfVal(c.sq[2]); x.property_class = e2 ? ref(e2) : null; }
      }
    }
    out.push(x);
  }
  return out;
}
const moduleOfOuter = (v) => { const e = entOfVal(v); return e && e.kind === 'package' ? e.name : null; };

// classes: one record per StaticClass getter
const zOfClass = new Map(); // getter fn -> Z_Construct_UClass analysis
const getterOfInfo = new Map([...analyses.values()].filter((A) => A.gpscb).map((A) => [A.gpscb, A.fn]));
for (const A of analyses.values()) {
  if (A.gpscb || !A.forceReg.length) continue;
  // (optimised core code inlines Z_Construct_UClass_<Super>: its own class is the last one it registers)
  const r = E.recOf(A.forceReg[A.forceReg.length - 1]);
  let g = r && r.call ? E.resolveThunk(r.call) : r && r.cls ? getterOfInfo.get(r.cls) ?? null : null;
  if (g === null) { // optimised code re-reads the class pointer from its global after calling the getter: the one StaticClass getter it calls names the class
    const gs = [...new Set(A.order.filter((t) => t !== null).map((t) => E.resolveThunk(t)).filter((t) => analyses.has(t) && analyses.get(t).gpscb))];
    if (gs.length === 1 && !A.objs.length) g = gs[0];
  }
  if (g !== null && analyze(g, 20000).gpscb) { const old = zOfClass.get(g); const w = (z) => z.objs.length + z.fnMap.length;
    if (!old) zOfClass.set(g, A); else if (w(A) > w(old)) { A.dups = [...(old.dups || []), old.fn]; zOfClass.set(g, A); } else (old.dups = old.dups || []).push(A.fn); }
}
if (process.env.DBG_Z) for (const A of analyses.values()) if (!A.gpscb && A.forceReg.length && ![...zOfClass.values()].includes(A)) console.log('unassigned forceReg fn', X(A.fn), A.end, A.objs.length, JSON.stringify(A.order.map(X)), A.forceReg.map((v) => JSON.stringify(E.recOf(v), (k, x) => (typeof x === 'bigint' ? x.toString(16) : k === 'ctx' ? undefined : x))).join(' ').slice(0, 300));
const nativeTable = (rn) => {
  if (rn === null || rn === undefined) return null;
  const A = analyze(Number(rn), 20000);
  if (!A.regNatives.length) return { fn: X(A.fn), tables: [] };
  return { fn: X(A.fn), tables: A.regNatives.map((t) => {
    const table = t.table === null ? null : Number(t.table), count = t.count === null ? null : Number(t.count & 0xFFFFFFFFn);
    const entries = [];
    if (table !== null && count !== null && count < 4096) for (let i = 0; i < count; i++) entries.push({ name: E.astr(Number(E.u64(table + 16 * i))), fn: X(Number(E.u64(table + 16 * i + 8))) });
    const tr = E.recOf(t.cls);
    return { table: X(table), count, class_getter: tr && tr.call ? X(E.resolveThunk(tr.call)) : null, entries };
  }) };
};
const usedFns = new Set();
for (const A of [...analyses.values()].sort((a, b) => a.fn - b.fn)) {
  if (!A.gpscb) continue;
  const g = A.gpscb;
  if (!g.inline) { noteUse(g.super_fn, 'StaticClass'); noteUse(g.within_fn, 'StaticClass'); }
  const sup = g.inline ? (g.super_tok === 0n ? { kind: 'class', name: g.name, fn: A.fn } : g.super_tok ? entOfVal(g.super_tok, 'StaticClass') : null) : g.super_fn === null ? null : entityOf(Number(g.super_fn));
  const within = g.inline ? (g.within_tok ? entOfVal(g.within_tok, 'StaticClass') : null) : g.within_fn === null ? null : entityOf(Number(g.within_fn));
  const owner = { name: g.name, package: constStr(g.package), size: g.size === null ? null : Number(g.size), class_flags: hexv(g.class_flags), cast_flags: hexv(constNum(g.cast_flags)), config: constStr(g.config),
    getter: X(A.fn), cpp_name: g.prefix ? g.prefix + g.name : null, register_natives: hexv(g.register_natives), class_constructor: hexv(g.class_constructor), vtable_helper: hexv(g.vtable_helper), add_referenced_objects: hexv(g.add_referenced_objects), class_ptr: hexv(g.ptr), ...(g.inline ? { intrinsic: true } : {}),
    super: sup && sup.kind === 'class' ? (sup.name === g.name && sup.fn === A.fn ? '-' : sup.name) : null, super_fn: hexv(g.super_fn), within: within && within.kind === 'class' ? within.name : null };
  const Z = zOfClass.get(A.fn);
  const rec = { registration_function: Z ? X(Z.fn) : null, kind: 'class', name: g.name, owner, objects: Z ? objects(Z) : [] };
  if (Z) {
    usedFns.add(Z.fn); stats.classes_with_construct++;
    // Z_Construct_UClass_X calls Z_Construct_UClass_<Super>() and Z_Construct_UPackage first: an independent reading of the super
    const pre = Z.order.map((t) => (t === null ? null : entityOf(t))).filter(Boolean);
    const zs = pre.find((e) => e.kind === 'class' && e.name === owner.super) || pre.find((e) => e.kind === 'class' && e.name !== g.name), zp = pre.find((e) => e.kind === 'package');
    rec.construct_super = zs ? zs.name : null; rec.construct_package = zp ? zp.name : null;
    rec.functions = Z.fnMap.map((f) => { const r = E.recOf(f.fn); return { name: nameOf(f.name), fn: r && r.call ? X(E.resolveThunk(r.call)) : null }; });
    if (Z.dups) rec.other_construct = Z.dups.map(X);
    rec.unknown_branches = Z.unknown_branches; if (Z.end !== 'ret' && Z.end !== 'tail') rec.emulation_end = Z.end;
  }
  const nt = nativeTable(g.register_natives);
  if (nt && nt.tables.length) { rec.natives = nt.tables; stats.native_tables += nt.tables.length; for (const t of nt.tables) stats.natives += t.entries.length; }
  records.push(rec); stats.classes++;
}
const classPackage = new Map(records.map((r) => [r.name, r.owner.package]));
// structs, enums, functions: the object each remaining function builds and returns
for (const A of [...analyses.values()].sort((a, b) => a.fn - b.fn)) {
  if (A.gpscb || usedFns.has(A.fn) || !A.objs.length) continue;
  const r = E.recOf(A.ret);
  const top = r && r.obj && r.obj.ctx === A ? r.obj : null;
  if (!top) { stats.not_returned++; continue; }
  const e = entityOf(A.fn);
  const objs = objects(A);
  const t = objs[top.id];
  const rec = { registration_function: X(A.fn), kind: e.kind, name: t.name, top: top.id };
  if (e.kind === 'struct') {
    const c = top.ctor;
    const sup = c ? (c.a[2] === 0n ? '-' : entOfVal(c.a[2])) : null;
    const ops = c ? E.recOf(c.a[3]) : null;
    let opsInfo = c && c.a[3] === 0n ? '-' : null; // '-': the constructor is given no ICppStructOps
    if (ops && ops.call) { // TCppStructOps<T>::TCppStructOps(): ICppStructOps(sizeof(T), alignof(T))
      const rr = E.run(E.resolveThunk(ops.call), { regs: { 1: BUF }, budget: 300, hooks });
      const sz = rr.mem.rd(BUF + 8n, 4), al = rr.mem.rd(BUF + 12n, 4), vt = rr.mem.rd(BUF, 8);
      if (rr.end === 'ret' && sz !== null && al !== null) opsInfo = { ctor: X(E.resolveThunk(ops.call)), size: Number(sz), align: Number(al), vtable: hexv(vt) };
    }
    rec.owner = { package: A.findStruct ? moduleOfOuter(A.findStruct.outer) : moduleOfOuter(top.outer), outer: objs[top.id].outer_ref || null, getter: objs[top.id].outer_getter || null };
    rec.struct = { size: A.findStruct && A.findStruct.size !== null ? Number(A.findStruct.size) : opsInfo && opsInfo !== '-' ? opsInfo.size : null, find_name: A.findStruct ? A.findStruct.name : null,
      super: sup === '-' ? '-' : sup && sup.kind === 'struct' ? sup.name : null, super_fn: sup && sup !== '-' ? X(sup.fn) : null, struct_flags: c ? hexv(c.sd[0]) : null, ops: opsInfo };
    stats.structs++;
  } else if (e.kind === 'enum') {
    rec.owner = { package: A.findEnum ? moduleOfOuter(A.findEnum.outer) : moduleOfOuter(top.outer), getter: objs[top.id].outer_getter || null };
    rec.enum = { find_name: A.findEnum ? A.findEnum.name : null, cpp_form: A.setEnums && A.setEnums.form !== null ? Number(A.setEnums.form) : null,
      values: A.pairs.map((p) => { stats.pair_functions[X(p.fn)] = (stats.pair_functions[X(p.fn)] || 0) + 1; return [p.name, p.value === null ? null : Number(BigInt.asIntN(32, p.value))]; }) };
    stats.enums++; stats.enum_values += A.pairs.length;
  } else if (e.kind === 'function' || e.kind === 'delegatefunction') {
    const o = objs[top.id];
    const oe = o.outer_ref;
    const c = top.ctor;
    rec.owner = oe && oe.kind === 'class' ? { name: oe.name, package: classPackage.get(oe.name) ?? null, getter: o.outer_getter } : oe && oe.kind === 'package' ? { package: oe.name, getter: o.outer_getter } : { getter: o.outer_getter || null };
    if (oe && oe.kind !== 'class' && oe.kind !== 'package') rec.owner.outer = oe;
    const se = c && c.a[2] !== 0n && c.a[2] !== null ? entOfVal(c.a[2]) : null;
    rec.function = { flags: c ? hexv(c.a[3] === null ? null : c.a[3] & 0xFFFFFFFFn) : null, rep_offset: c && c.sd[0] !== null ? Number(c.sd[0] & 0xFFFFn) : null, params_size: c && c.sd[1] !== null ? Number(c.sd[1] & 0xFFFFn) : null, super: se ? ref(se) : c && c.a[2] === 0n ? '-' : null };
    if (e.kind === 'function') stats.functions++; else stats.delegates++;
  } else { rec.owner = { getter: objs[top.id].outer_getter || null }; rec.object_class = e.object_class; }
  rec.objects = objs;
  rec.unknown_branches = A.unknown_branches; if (A.end !== 'ret' && A.end !== 'tail') rec.emulation_end = A.end;
  if (A.bitfield_fail) rec.bitfield_failures = A.bitfield_fail;
  if (A.notes.length) rec.notes = A.notes;
  records.push(rec);
}
for (const A of analyses.values()) stats.unknown_branches += A.unknown_branches;
stats.emulated = analyses.size;

// ---------------------------------------------------------------- 5. cross-checks made on the evidence itself
{
  const cls = new Map(records.filter((r) => r.kind === 'class').map((r) => [r.name, r]));
  const ck = stats.checks;
  ck.class_names_unique = cls.size === stats.classes;
  ck.super_unresolved = []; ck.super_vs_construct_mismatch = []; ck.size_below_super = []; ck.chain_not_object = []; ck.package_vs_construct_mismatch = [];
  for (const r of cls.values()) {
    const o = r.owner;
    if (o.super === null) ck.super_unresolved.push(r.name);
    if (r.construct_super !== undefined && r.construct_super !== null && o.super !== '-' && r.construct_super !== o.super) ck.super_vs_construct_mismatch.push([r.name, o.super, r.construct_super]);
    if (r.construct_package && o.package && r.construct_package !== o.package) ck.package_vs_construct_mismatch.push([r.name, o.package, r.construct_package]);
    const s = o.super && o.super !== '-' ? cls.get(o.super) : null;
    if (s && s.owner.size !== null && o.size !== null && o.size < s.owner.size) ck.size_below_super.push([r.name, o.size, o.super, s.owner.size]);
    let cur = r, n = 0;
    while (cur && cur.owner.super && cur.owner.super !== '-' && n++ < 64) cur = cls.get(cur.owner.super);
    if (!cur || cur.name !== 'Object' || cur.owner.super !== '-') ck.chain_not_object.push(r.name);
  }
  const st = new Map(records.filter((r) => r.kind === 'struct').map((r) => [r.name, r]));
  ck.struct_size_vs_ops_mismatch = []; ck.struct_name_vs_find_mismatch = []; ck.struct_size_below_super = [];
  for (const r of st.values()) {
    if (r.struct.ops && r.struct.ops !== '-' && r.struct.size !== null && r.struct.ops.size !== r.struct.size) ck.struct_size_vs_ops_mismatch.push([r.name, r.struct.size, r.struct.ops.size]);
    if (r.struct.find_name !== null && r.struct.find_name !== r.name) ck.struct_name_vs_find_mismatch.push([r.name, r.struct.find_name]);
    const s = r.struct.super && r.struct.super !== '-' ? st.get(r.struct.super) : null;
    if (s && s.struct.size !== null && r.struct.size !== null && r.struct.size < s.struct.size) ck.struct_size_below_super.push([r.name, r.struct.size, r.struct.super, s.struct.size]);
  }
  ck.enum_name_vs_find_mismatch = records.filter((r) => r.kind === 'enum' && r.enum.find_name !== null && r.enum.find_name !== r.name).map((r) => [r.name, r.enum.find_name]);
  ck.enum_value_unknown = records.filter((r) => r.kind === 'enum' && r.enum.values.some((v) => v[1] === null)).map((r) => r.name);
  for (const k of Object.keys(ck)) if (Array.isArray(ck[k])) { const n = ck[k].length; ck[k] = { count: n, examples: ck[k].slice(0, 25) }; }
}

// C++ class names: the literal in the executable where there is one (TEXT("ATresGameMode") + 1 is the name argument of
// GetPrivateStaticClassBody); otherwise the prefix UnrealHeaderTool enforces (A for classes derived from AActor, U for the rest)
{
  const cls = new Map(records.filter((r) => r.kind === 'class').map((r) => [r.name, r]));
  const isActor = (n) => { for (let c = cls.get(n), i = 0; c && i < 64; c = c.owner.super !== '-' ? cls.get(c.owner.super) : null, i++) if (c.name === 'Actor') return true; return false; };
  for (const c of cls.values()) { c.owner.cpp_name_literal = !!c.owner.cpp_name; c.owner.cpp_name_by_rule = (isActor(c.name) ? 'A' : 'U') + c.name; if (!c.owner.cpp_name) c.owner.cpp_name = c.owner.cpp_name_by_rule; }
}
fs.writeFileSync(OUT, '[\n' + records.map((x) => JSON.stringify(x)).join(',\n') + '\n]\n');

// ---------------------------------------------------------------- 6. what the registration code proves about functions
// Every function the miner had to identify to read the data is itself identified: its name follows from the
// UE 4.17 generated-code naming (Z_Construct_UClass_<Class>, <Class>::GetPrivateStaticClass ...) and the names the
// registration passes to the engine. Written to audit/reflection_functions.json for tools/names.js.
{
  const FN = path.join(ROOT, 'audit', 'reflection_functions.json');
  const cls = new Map(records.filter((r) => r.kind === 'class').map((r) => [r.name, r]));
  const isActor = (n) => { for (let c = cls.get(n), i = 0; c && i < 64; c = c.owner.super !== '-' ? cls.get(c.owner.super) : null, i++) if (c.name === 'Actor') return true; return false; };
  // C++ class name: the literal in the executable where there is one (TEXT("ATresGameMode") + 1); otherwise the prefix
  // UnrealHeaderTool enforces (A for classes derived from AActor, U for the rest)
  const cpp = (n) => { const c = cls.get(n); return c ? c.owner.cpp_name : 'U' + n; };
  const literal = (n) => !!(cls.get(n) && cls.get(n).owner.cpp_name_literal);
  let prefixChecked = 0, prefixMismatch = [];
  for (const c of cls.values()) if (c.owner.cpp_name_literal && /^[AU]/.test(c.owner.cpp_name)) { prefixChecked++; if (c.owner.cpp_name !== c.owner.cpp_name_by_rule) prefixMismatch.push(c.owner.cpp_name); }
  const mod = (p) => String(p).replace(/^\/Script\//, '');
  const cand = new Map(); // address -> [{ name, basis, conf, evidence, share }]
  const add = (a, name, basis, conf, evidence, share) => { if (a === null || a === undefined) return; const k = typeof a === 'number' ? a : parseInt(a, 16); if (!k || !E.inText(k)) return; (cand.get(k) || cand.set(k, []).get(k)).push({ name, basis, conf, evidence, share: share || null }); };
  const REG = 'reflection registration', ENG = 'engine function';
  for (const r of records) {
    if (r.kind === 'class') {
      const o = r.owner, C = cpp(r.name), cf = literal(r.name) ? 'confirmed' : 'inferred';
      const how = literal(r.name) ? `the class name literal L"${C}"` : `the registered name "${r.name}" with the UnrealHeaderTool class prefix`;
      add(o.getter, `${C}::GetPrivateStaticClass`, REG, cf, o.intrinsic ? `builds the UClass named "${r.name}" in place (UClass constructor 0x142E4BB80, Register(${o.package}, ${r.name})) and returns it; name from ${how}` : `calls GetPrivateStaticClassBody (0x142E54B80) with ${how}, package ${o.package}, size ${o.size} and returns the class pointer it fills`);
      add(r.registration_function, `Z_Construct_UClass_${C}`, REG, cf, `calls UObjectForceRegistration (0x142F7C280) on the result of ${C}::GetPrivateStaticClass (${o.getter}) and constructs the ${r.objects.length} reflected objects of the class; generated-code name`);
      add(o.class_constructor, `InternalConstructor<${C}>`, REG, cf, `ClassConstructor argument of the GetPrivateStaticClassBody call in ${C}::GetPrivateStaticClass (${o.getter}): constructs a ${C} in the memory of the FObjectInitializer's object`, 'skip');
      add(o.vtable_helper, `InternalVTableHelperCtorCaller<${C}>`, REG, cf, `ClassVTableHelperCtorCaller argument of the GetPrivateStaticClassBody call in ${o.getter}`, 'skip');
      add(o.register_natives, `${C}::StaticRegisterNatives${C}`, REG, cf, `RegisterNativeFunc argument of the GetPrivateStaticClassBody call in ${o.getter}${r.natives ? '; it registers the native function table ' + r.natives.map((t) => t.table).join(', ') : ''}`, 'skip');
      add(o.add_referenced_objects, 'AddReferencedObjects', REG, cf, `ClassAddReferencedObjects argument of the GetPrivateStaticClassBody call in ${o.getter}`, 'definer:' + r.name);
    } else if (r.kind === 'struct') {
      add(r.registration_function, `Z_Construct_UScriptStruct_F${r.name}`, REG, 'confirmed', `allocates the UScriptStruct named "${r.name}" (size ${r.struct.size}) with its ${r.objects.length - 1} properties and returns it; generated-code name, F is the prefix UnrealHeaderTool enforces for structs`);
      if (r.struct.ops && r.struct.ops !== '-') add(r.struct.ops.ctor, `UScriptStruct::TCppStructOps<F${r.name}>::TCppStructOps`, REG, 'confirmed', `constructs the ICppStructOps passed to the UScriptStruct constructor in ${r.registration_function}: stores vtable ${r.struct.ops.vtable}, size ${r.struct.ops.size}, alignment ${r.struct.ops.align}`, 'skip');
    } else if (r.kind === 'enum') {
      add(r.registration_function, `Z_Construct_UEnum_${mod(r.owner.package)}_${r.name}`, REG, 'confirmed', `allocates the UEnum named "${r.name}" in ${r.owner.package}, adds its ${r.enum.values.length} names and returns it; generated-code name`);
    } else if (r.kind === 'function' || r.kind === 'delegatefunction') {
      const scope = r.owner.name ? cpp(r.owner.name) : r.owner.package ? mod(r.owner.package) : null;
      if (scope) add(r.registration_function, `Z_Construct_${r.kind === 'function' ? 'UFunction' : 'UDelegateFunction'}_${scope}_${r.name}`, REG, r.owner.name && !literal(r.owner.name) ? 'inferred' : 'confirmed',
        `allocates the ${r.kind === 'function' ? 'UFunction' : 'UDelegateFunction'} named "${r.name}" outered to ${r.owner.name ? 'class ' + r.owner.name : r.owner.package} with its ${r.objects.length - 1} parameter properties and returns it; generated-code name`);
    }
  }
  // packages and getter wrappers
  for (const [fn, e] of entities) {
    if (!e) continue;
    if (e.kind === 'package' && e.fn === fn) add(fn, `Z_Construct_UPackage__Script_${mod(e.name)}`, REG, 'confirmed', `creates the package named "${e.name}" and returns it; structs, enums and delegates of the module are outered to its result; generated-code name`);
    if (e.kind === 'class' && e.fn !== fn && cls.has(e.name)) {
      const A = analyses.get(fn), u = useOf.get(fn);
      if (!A || A.gpscb || A.objs.length || A.forceReg.length || A.steps > 40 || !u) continue;
      const C = cpp(e.name), cf = literal(e.name) ? 'confirmed' : 'inferred';
      if (u.has('StaticClass')) add(fn, `${C}::StaticClass`, REG, cf, `returns the result of ${C}::GetPrivateStaticClass (${X(e.fn)}); passed as SuperClassFn or WithinClassFn to GetPrivateStaticClassBody${u.has('target') ? ' (and also used as a property class getter)' : ''}`, 'skip');
      else if (u.has('target')) add(fn, `Z_Construct_UClass_${C}_NoRegister`, REG, cf, `returns the result of ${C}::GetPrivateStaticClass (${X(e.fn)}); its result is the class argument of property constructors; generated-code name`, 'skip');
    }
  }
  // the engine functions the miner hooks, and the allocators and constructors of the reflected object classes
  const ENGINE = {
    GPSCB: ['GetPrivateStaticClassBody', `called once by each of ${stats.classes} StaticClass getters with (package name, class name, &class pointer, register-natives function, size, class flags, cast flags, config name, constructor, vtable helper, AddReferencedObjects, Super::StaticClass, Within::StaticClass)`],
    SAO: ['StaticAllocateObject', `called by all ${allocators.size} placement operator-new functions of the reflected object classes with (T::StaticClass(), Outer, Name, Flags, ...)`],
    FIND_STRUCT: ['FindExistingStructIfHotReloadOrDynamic', `called by every Z_Construct_UScriptStruct function (${stats.structs}) with (Outer, L"Name", sizeof, crc, false) before the struct is built`],
    FIND_ENUM: ['FindExistingEnumIfHotReloadOrDynamic', `called by every Z_Construct_UEnum function (${stats.enums}) with (Outer, L"Name", 0, crc, false) before the enum is built`],
    REG_NATIVES: ['FNativeFunctionRegistrar::RegisterFunctions', `called by ${stats.native_tables} StaticRegisterNatives functions with (class, table of {name, exec function}, count)`],
    FORCE_REG: ['UObjectForceRegistration', 'called by every Z_Construct_UClass function on the class returned by its StaticClass getter'],
    FNAME_W: ['FName::FName', 'constructs an FName from a wide string literal and a find type (out, L"text", FNAME_Add); every reflected object name passes through it'],
    FNAME_A: ['FName::FName', 'constructs an FName from a narrow string literal and a find type (out, "text", FNAME_Add); calls the shared FName initialiser'],
    BITFIELD: ['FDetermineBitfieldOffsetAndMask::DoDetermineBitfieldOffsetAndMask', 'runs the virtual setter of the probe object on a zeroed buffer of the given size and stores the offset and mask of the first non-zero byte into the probe'],
    ADD_FN_MAP: ['UClass::AddFunctionToFunctionMap', 'called by Z_Construct_UClass functions with (class, UFunction, FName): the pairs agree with the function records'],
    UCLASS_CTOR: ['UClass::UClass', 'the EStaticConstructor form (Name, Size, ClassFlags, CastFlags, Config, ObjectFlags, constructor, vtable helper, AddReferencedObjects): called by the StaticClass getters of the core classes that inline GetPrivateStaticClassBody'],
    REGISTER: ['UObjectBase::Register', 'called with (L"/Script/Package", L"Name") right after the UClass constructor in the StaticClass getters of the core classes'],
    INIT_THREAD_HEADER: ['_Init_thread_header', 'CRT guard of function-local statics: called with the address of the guard variable before a static is initialised'],
    CHKSTK: ['__chkstk', 'called with the frame size in RAX at the entry of functions with frames over a page, followed by sub rsp, rax'],
    COOKIE: ['__security_check_cookie', 'called before returning with the frame cookie xor rsp in RCX'],
  };
  for (const [k, [name, ev]] of Object.entries(ENGINE)) add(K[k], name, ENG, 'confirmed', ev + ' (identified from these call shapes; tools/reflection.js, table K)');
  const U = (c) => 'U' + c;
  for (const [a, g] of allocators) { const c = className(g); if (c) add(a, `${U(c)}::operator_new`, ENG, 'confirmed', `the placement operator new(size, EInternal, Outer, Name, Flags) of ${U(c)}: calls ${U(c)}::GetPrivateStaticClass (${X(E.resolveThunk(g))}) and StaticAllocateObject`); }
  const ctorOf = new Map(); // constructor address -> Set(object class)
  for (const r of records) for (const o of r.objects) if (o.ctor && o.object_class) (ctorOf.get(o.ctor) || ctorOf.set(o.ctor, new Set()).get(o.ctor)).add(o.object_class);
  for (const [a, set] of ctorOf) if (set.size === 1) { const c = [...set][0]; add(a, `${U(c)}::${U(c)}`, ENG, 'confirmed', `the constructor the registration code calls on every object it allocates with ${U(c)}::operator_new (first argument the allocated memory, second an FObjectInitializer)`); }
  // one name per address
  const out = [], skipped = { shared_by_several_classes: 0, shared_examples: [] };
  for (const [a, list] of [...cand].sort((x, y) => x[0] - y[0])) {
    const names = [...new Set(list.map((x) => x.name))];
    let f = list[0];
    if (list[0].share && list[0].share.startsWith('definer:')) {
      // &TClass::AddReferencedObjects is the function of the nearest class that declares one: among the classes that
      // pass this address, the one whose super class does not pass it
      const users = new Set(list.filter((x) => x.share && x.share.startsWith('definer:')).map((x) => x.share.slice(8)));
      const roots = [...users].filter((n) => { const sp = cls.get(n).owner.super; return sp === '-' || !users.has(sp); });
      if (roots.length !== 1) { skipped.shared_by_several_classes++; if (skipped.shared_examples.length < 12) skipped.shared_examples.push(X(a) + ' AddReferencedObjects of ' + roots.slice(0, 3).join(', ') + (roots.length > 3 ? ', ...' : '')); continue; }
      f = { ...list[0], name: `${cpp(roots[0])}::AddReferencedObjects`, conf: literal(roots[0]) ? 'confirmed' : 'inferred', evidence: `ClassAddReferencedObjects argument of the GetPrivateStaticClassBody call of ${users.size} class${users.size === 1 ? '' : 'es'}; ${roots[0]} is the one whose super class passes a different function, so the function is declared by ${cpp(roots[0])}` };
    } else if (names.length > 1) { // identical code folded by the linker, or a function that serves several classes: no single name
      skipped.shared_by_several_classes++; if (skipped.shared_examples.length < 12) skipped.shared_examples.push(X(a) + ' ' + names.slice(0, 3).join(' / ') + (names.length > 3 ? ` / ... (${names.length})` : ''));
      continue;
    }
    out.push({ address: X(a), name: f.name, basis: f.basis, confidence: f.conf, evidence: f.evidence });
  }
  fs.writeFileSync(FN, '[\n' + out.map((x) => JSON.stringify(x)).join(',\n') + '\n]\n');
  stats.function_facts = { written: out.length, by_basis: out.reduce((m, x) => { m[x.basis] = (m[x.basis] || 0) + 1; return m; }, {}), ...skipped, class_prefix_literals: prefixChecked, class_prefix_rule_mismatches: prefixMismatch.slice(0, 20), class_prefix_rule_mismatch_count: prefixMismatch.length };
}
stats.allocator_classes = Object.fromEntries([...allocators].map(([a, g]) => [X(a), className(g)]));
stats.bitfield_setters = bitfieldCache.size;
writeJson(STATS, { generated_by: 'tools/reflection.js', version: VERSION, ...stats });
logRun({ tool: 'reflection', version: VERSION, outputs: [OUT, STATS, path.join(ROOT, 'audit', 'reflection_functions.json')], note: JSON.stringify({ ...stats, allocator_classes: undefined, checks: undefined, by_property_class: undefined, pair_functions: undefined, written_fields: undefined }) });
console.log(JSON.stringify({ ...stats, allocator_classes: undefined }, null, 1));
log('done');
