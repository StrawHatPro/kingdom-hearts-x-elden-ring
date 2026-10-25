// Virtual function tables of the reflected classes, their constructors, and who owns each virtual function.
// UObject classes carry no RTTI in this build, so their vtables are found by execution: the class constructor
// given to GetPrivateStaticClassBody (InternalConstructor<T>, audit/reflection.json class_constructor) is run in
// the emulator (tools/lib/emu.js) on a buffer; calls whose `this` is the object are entered. Every store to
// [this+0] is a constructor installing a vtable, and the last one is the vtable of the class itself.
//   class vtable   the value at [this+0] when InternalConstructor<T> returns
//   constructors   a function that is entered with this == object and whose last store to [this+0] is the
//                  vtable of class C is a constructor of C (it runs the base constructors first, then installs
//                  its own vtable); when that function is InternalConstructor<C> itself the constructor is inlined
//   vtable extent  from the vtable address to the next address that code loads with lea (the start of another
//                  table) or to the first entry that is not a function, whichever comes first
//   ownership      a function is introduced by class C at slot i when C's vtable holds it there and the super
//                  class's vtable holds another function there (override) or has no such slot (new)
// Classes with RTTI (audit/rtti.json) are added with the vtables found there.
//   node tools/vtables.js
// Output: audit/class_vtables.json (per class: vtable, slots, constructors; checks), audit/vfuncs.json (per function)
const fs = require('fs');
const path = require('path');
const { ROOT, logRun } = require('./lib/log');
const { create, ENTER } = require('./lib/emu');

const VERSION = '1.0.0';
const E = create();
const X = (n) => (n === null || n === undefined ? null : '0x' + n.toString(16).toUpperCase());
const recs = JSON.parse(fs.readFileSync(path.join(ROOT, 'audit', 'reflection.json'), 'utf8'));
const classes = recs.filter((r) => r.kind === 'class');
const byName = new Map(classes.map((c) => [c.name, c]));
const rdata = E.secs.find((s) => s.name === '.rdata');
const inRdata = (a) => a >= rdata.lo && a < rdata.rawHi;
const fnStart = (a) => E.inText(a) && (E.funcStart(a) === a || E.funcOf(a) === null); // an entry in .pdata, or a leaf without unwind data

// ---- 1. run every class constructor
const BUF = 0x200000000000n, XBUF = 0x300000000000n;
const K = { CHKSTK: 0x1458701C0, COOKIE: 0x145870110 };
const runs = new Map(); // class -> { vtable, events: [{ fn, value }], end }
const stats = { classes: classes.length, constructor_runs_returned: 0, classes_with_vtable: 0, run_failures: {}, constructors_named: 0, constructors_inlined: 0 };
for (const c of classes) {
  const ctor = c.owner.class_constructor ? parseInt(c.owner.class_constructor, 16) : null;
  if (!ctor || !E.inText(ctor)) { stats.run_failures['no class constructor'] = (stats.run_failures['no class constructor'] || 0) + 1; continue; }
  const size = BigInt(c.owner.size);
  const cur = new Array(8).fill(-1), events = [];
  let live = false;
  const hooks = new Map();
  hooks.set(K.CHKSTK, (k, st) => st.regs[0]); hooks.set(K.COOKIE, (k, st) => st.regs[0]);
  hooks.set('*', (k, st) => (k.target !== null && k.a[0] !== null && k.a[0] >= BUF && k.a[0] < BUF + size && st.depth() < 24 ? ENTER : undefined));
  hooks.set('indirect', hooks.get('*'));
  let r;
  try {
    r = E.run(ctor, { regs: { 1: XBUF }, hooks, budget: 300000, zero: [BUF, BUF + size], dataUnknown: true,
      init: (st) => { st.wr(XBUF, 8, BUF); live = true; },
      onWrite: (a, v, ip) => { if (!live) return; const o = a - Number(BUF); if (o < 0 || o > 7) return; cur[o] = v; if (o === 7) events.push({ ip, value: cur.every((x) => x >= 0) ? cur.reduce((s, x, i) => s + x * 2 ** (8 * i), 0) : null }); } });
  } catch (e) { stats.run_failures['emulator error'] = (stats.run_failures['emulator error'] || 0) + 1; continue; }
  if (r.end !== 'ret' && r.end !== 'tail') { stats.run_failures[r.end] = (stats.run_failures[r.end] || 0) + 1; continue; }
  stats.constructor_runs_returned++;
  const last = events.length ? events[events.length - 1].value : null;
  runs.set(c.name, { ctor, events, vtable: last !== null && inRdata(last) && fnStart(Number(E.u64(last))) ? last : null });
}
// a vtable belongs to the class whose construction ends with it. The linker folds identical tables, so a class that
// overrides nothing ends with its super's table: such a table names a constructor's class only when one class ends with it
const ownerOf = new Map(); const dupVt = new Set();
for (const [n, r] of runs) { if (r.vtable === null) continue; if (ownerOf.has(r.vtable)) dupVt.add(r.vtable); else ownerOf.set(r.vtable, n); }
for (const v of dupVt) ownerOf.delete(v);
stats.vtables_shared_by_several_classes = dupVt.size;

// ---- 2. constructors
const cpp = (n) => { const c = byName.get(n); if (c.owner.cpp_name) return c.owner.cpp_name; let a = false; for (let x = c, i = 0; x && i < 64; x = x.owner.super !== '-' ? byName.get(x.owner.super) : null, i++) if (x.name === 'Actor') a = true; return (a ? 'A' : 'U') + n; };
const ctorFacts = new Map(); // function -> { class, vtable, runs }
const conflicts = [];
for (const [n, r] of runs) {
  const lastIn = new Map(); // function -> last vtable value it stores, in this run
  for (const e of r.events) { const f = E.funcStart(e.ip) ?? null; if (f !== null) lastIn.set(f, e.value); }
  for (const [f, v] of lastIn) {
    const owner = v !== null ? ownerOf.get(v) : undefined;
    if (!owner) continue;
    const old = ctorFacts.get(f);
    if (old && old.class !== owner) { conflicts.push([X(f), old.class, owner]); old.bad = true; continue; }
    if (old) old.runs++; else ctorFacts.set(f, { class: owner, vtable: v, runs: 1, inlined: f === E.resolveThunk(runs.get(owner).ctor) });
  }
}
const ctors = [];
for (const [f, c] of [...ctorFacts].sort((a, b) => a[0] - b[0])) {
  if (c.bad) continue;
  if (c.inlined) { stats.constructors_inlined++; continue; }
  if ([...runs.values()].some((r) => E.resolveThunk(r.ctor) === f)) continue; // an InternalConstructor of another class: not a constructor by itself
  stats.constructors_named++;
  ctors.push({ address: X(f), class: c.class, name: `${cpp(c.class)}::${cpp(c.class)}`, vtable: X(c.vtable),
    evidence: `entered with this = the object during InternalConstructor<${cpp(c.class)}> (${byName.get(c.class).owner.class_constructor})${c.runs > 1 ? ` and the constructors of ${c.runs - 1} derived classes` : ''}; the last value it stores to [this] is ${X(c.vtable)}, the vtable the construction of ${c.class} ends with` });
}
stats.constructor_conflicts = conflicts.length;

// ---- 3. vtable extents: starts of tables that code addresses
const tableStarts = new Set();
{
  const b = E.img.b, text = E.secs[0], lo = text.raw, hi = text.raw + (text.rawHi - text.lo) - 7;
  for (let o = lo; o < hi; o++) { // lea r64, [rip+disp32]: REX.W 8D /r with mod=00 rm=101
    if (b[o + 1] !== 0x8D || (b[o] !== 0x48 && b[o] !== 0x4C) || (b[o + 2] & 0xC7) !== 0x05) continue;
    const t = text.lo + (o - lo) + 7 + b.readInt32LE(o + 3);
    if (inRdata(t)) tableStarts.add(t);
  }
}
const rtti = JSON.parse(fs.readFileSync(path.join(ROOT, 'audit', 'rtti.json'), 'utf8'));
for (const t of rtti) for (const v of t.vtables) tableStarts.add(parseInt(v.address, 16));
for (const v of ownerOf.keys()) tableStarts.add(v);
function slotsOf(vt, max = 4096) {
  const out = [];
  for (let a = vt; out.length < max; a += 8) {
    if (a !== vt && tableStarts.has(a)) break;
    const f = Number(E.u64(a));
    if (!E.inText(f)) break;
    out.push(f);
  }
  return out;
}

// ---- 4. ownership
const tables = []; // { kind, name, vtable, slots, base }
const slotCache = new Map();
for (const [n, r] of runs) { if (r.vtable === null) continue; tableStarts.add(r.vtable); }
for (const [n, r] of runs) { if (r.vtable === null) continue; if (!slotCache.has(r.vtable)) slotCache.set(r.vtable, slotsOf(r.vtable)); const sp = byName.get(n).owner.super; tables.push({ kind: 'class', name: n, vtable: r.vtable, slots: slotCache.get(r.vtable), base: sp !== '-' ? sp : null }); }
const classTable = new Map(tables.map((t) => [t.name, t]));
const rttiPrimary = new Map();
for (const t of rtti) { const v = t.vtables.find((x) => x.this_offset === 0); if (v) rttiPrimary.set(t.name, { kind: 'rtti', name: t.name, vtable: parseInt(v.address, 16), slots: slotsOf(parseInt(v.address, 16), v.slots), base: t.bases[0] || null }); }
for (const t of rttiPrimary.values()) tables.push(t);
const fnInfo = new Map(); // function -> { intros: [{ kind, name, slot, how }], tables }
const check = { shorter_than_super: [], no_super_vtable: 0 };
for (const t of tables) {
  const base = t.base ? (t.kind === 'class' ? classTable.get(t.base) : rttiPrimary.get(t.base)) : null;
  if (t.base && !base) check.no_super_vtable++;
  if (base && base.slots.length > t.slots.length) { check.shorter_than_super.push([t.name, t.slots.length, base.name, base.slots.length]); continue; }
  t.slots.forEach((f, i) => {
    const e = fnInfo.get(f) || fnInfo.set(f, { intros: [], tables: 0 }).get(f);
    e.tables++;
    const how = !base ? 'root' : i >= base.slots.length ? 'new' : base.slots[i] !== f ? 'override' : null;
    if (how) e.intros.push({ kind: t.kind, name: t.name, slot: i, how });
  });
}
const vfuncs = [];
for (const [f, e] of [...fnInfo].sort((a, b) => a[0] - b[0])) {
  // a function reachable only as "root" (its class has no known super vtable) where another class introduces it is the other's
  const real = e.intros.filter((x) => x.how !== 'root');
  const intros = real.length ? real : e.intros;
  const one = intros.length === 1 ? intros[0] : null;
  const sameSlot = intros.length && intros.every((x) => x.slot === intros[0].slot);
  vfuncs.push({ address: X(f), class: one ? one.name : null, owner_kind: one ? one.kind : intros.length && intros.every((x) => x.kind === intros[0].kind) ? intros[0].kind : null,
    slot: sameSlot ? intros[0].slot : null, how: one ? one.how : intros.length && intros.every((x) => x.how === intros[0].how) ? intros[0].how : null, introduced_by: intros.length, tables: e.tables,
    examples: one ? undefined : intros.slice(0, 4).map((x) => `${x.name}[${x.slot}]`) });
}
stats.classes_with_vtable = tables.filter((t) => t.kind === 'class').length; stats.classes_with_a_vtable_of_their_own = ownerOf.size;
stats.rtti_primary_vtables = rttiPrimary.size;
stats.tables = tables.length;
stats.table_starts_addressed_by_code = tableStarts.size;
stats.virtual_functions = vfuncs.length;
stats.introduced_by_one_class = vfuncs.filter((v) => v.introduced_by === 1).length;
stats.vtables_shorter_than_super = check.shorter_than_super.length;
stats.classes_without_super_vtable = check.no_super_vtable;
const OUT1 = path.join(ROOT, 'audit', 'class_vtables.json'), OUT2 = path.join(ROOT, 'audit', 'vfuncs.json');
fs.writeFileSync(OUT1, JSON.stringify({ generated_by: 'tools/vtables.js', version: VERSION, stats, checks: { constructor_conflicts: conflicts.slice(0, 40), vtables_shorter_than_super: check.shorter_than_super.slice(0, 40), vtables_shared: [...dupVt].slice(0, 40).map(X) } }, null, 1).replace(/\n\}$/, '') +
  ',\n "classes": [\n' + tables.filter((t) => t.kind === 'class').sort((a, b) => (a.name < b.name ? -1 : 1)).map((t) => JSON.stringify({ class: t.name, vtable: X(t.vtable), slots: t.slots.length })).join(',\n') +
  '\n ],\n "constructors": [\n' + ctors.map((c) => JSON.stringify(c)).join(',\n') + '\n ]\n}\n');
JSON.parse(fs.readFileSync(OUT1, 'utf8'));
fs.writeFileSync(OUT2, '[\n' + vfuncs.map((v) => JSON.stringify(v)).join(',\n') + '\n]\n');
logRun({ tool: 'vtables', version: VERSION, outputs: [OUT1, OUT2], note: JSON.stringify(stats) });
console.log(JSON.stringify(stats, null, 1));
console.log('conflicts', JSON.stringify(conflicts.slice(0, 5)), 'shorter', JSON.stringify(check.shorter_than_super.slice(0, 5)));
