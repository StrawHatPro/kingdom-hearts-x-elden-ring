// MSVC x64 RTTI: recover class names, inheritance and vtables straight from .rdata.
//   vtable[-1] -> RTTICompleteObjectLocator { sig=1, offset, cdOffset, typeDesc RVA, classDesc RVA, self RVA }
//   TypeDescriptor { vfptr, spare, char name[] }      name like ".?AVUObject@@"
//   ClassHierarchyDescriptor { sig, attrs, numBases, baseArray RVA } -> BaseClassDescriptor { typeDesc RVA, ... }
// Writes audit/rtti.json (classes) and build/rtti_vfuncs.json (function address -> class slot).
const fs = require('fs');
const path = require('path');
const { ROOT, logRun, hex } = require('./lib/log');
const pe = require('./lib/pe');

const VERSION = '1.0.0';
const img = pe.load();
const { b, sections } = img;
const BASE = Number(img.imageBase);
const text = sections[0];
const dataSecs = sections.filter((s) => s.name === '.rdata' || s.name === '.data');
const off = (rva) => img.rvaToOff(rva);
const inText = (va) => va >= BASE + text.rva && va < BASE + text.rva + text.vsize;

function typeName(rva) {
  const o = off(rva);
  if (o < 0) return null;
  let e = o + 16;
  while (e < b.length && b[e] && e - o < 2048) e++;
  const s = b.toString('latin1', o + 16, e);
  return s.startsWith('.?A') ? s : null;
}
// ".?AVBar@Foo@@" -> "Foo::Bar". Templates and other decorated forms are kept mangled.
function demangle(m) {
  const x = /^\.\?A[VUW](?:4)?([A-Za-z_][\w]*(?:@[A-Za-z_][\w]*)*)@@$/.exec(m);
  return x ? x[1].split('@').reverse().join('::') : m;
}

// 1. complete object locators
const cols = new Map(); // COL VA -> info
const baseTypes = new Map(); // mangled name -> { name, mangled, bases }: every type a class hierarchy descriptor names
for (const s of dataSecs) {
  const end = s.rawPtr + Math.min(s.rawSize, s.vsize) - 24;
  for (let o = s.rawPtr; o <= end; o += 4) {
    if (b.readUInt32LE(o) !== 1) continue;
    const rva = s.rva + (o - s.rawPtr);
    if (b.readUInt32LE(o + 20) !== rva) continue;
    const name = typeName(b.readUInt32LE(o + 12));
    if (!name) continue;
    const chd = off(b.readUInt32LE(o + 16));
    const bases = [];
    if (chd >= 0) {
      const n = b.readUInt32LE(chd + 8), arr = off(b.readUInt32LE(chd + 12));
      const entries = [];
      for (let i = 0; i < n && i < 64 && arr >= 0; i++) { // entry 0 is the class itself
        const bcd = off(b.readUInt32LE(arr + i * 4));
        const bn = bcd >= 0 ? typeName(b.readUInt32LE(bcd)) : null;
        entries.push({ bn, contained: bcd >= 0 ? b.readUInt32LE(bcd + 4) : 0 });
        if (bn && i > 0) bases.push(demangle(bn));
      }
      // the array is the hierarchy in pre-order: the numContainedBases entries after a base are that base's own bases
      entries.forEach((e, i) => { if (i > 0 && e.bn && !baseTypes.has(e.bn)) baseTypes.set(e.bn, { name: demangle(e.bn), mangled: e.bn, bases: entries.slice(i + 1, i + 1 + e.contained).filter((x) => x.bn).map((x) => demangle(x.bn)), vtables: [] }); });
    }
    cols.set(BASE + rva, { mangled: name, name: demangle(name), offset: b.readUInt32LE(o + 4), bases });
  }
}

// 2. vtables: a pointer to a COL followed by pointers into .text
const classes = new Map();
const vfuncs = {};
let nVtables = 0, nSlots = 0;
for (const s of dataSecs) {
  const end = s.rawPtr + Math.min(s.rawSize, s.vsize) - 16;
  for (let o = s.rawPtr; o <= end; o += 8) {
    const col = cols.get(Number(b.readBigUInt64LE(o)));
    if (!col) continue;
    const vt = BASE + s.rva + (o - s.rawPtr) + 8;
    const slots = [];
    for (let p = o + 8; p <= end + 8; p += 8) {
      const v = Number(b.readBigUInt64LE(p));
      if (!inText(v) || (p !== o + 8 && cols.has(Number(b.readBigUInt64LE(p))))) break;
      slots.push(v);
    }
    if (!slots.length) continue;
    nVtables++; nSlots += slots.length;
    if (!classes.has(col.mangled)) classes.set(col.mangled, { name: col.name, mangled: col.mangled, bases: col.bases, vtables: [] });
    classes.get(col.mangled).vtables.push({ address: hex(vt), this_offset: col.offset, slots: slots.length });
    slots.forEach((fn, i) => { (vfuncs[fn.toString(16).toUpperCase()] = vfuncs[fn.toString(16).toUpperCase()] || []).push([col.name, col.offset, i, vt.toString(16).toUpperCase()]); });
  }
}
// types that have RTTI but no vtable of their own in the image: classes whose locator no vtable points at, and
// base classes that occur only inside other classes' hierarchy descriptors (non-polymorphic or never instantiated)
let withoutVtable = 0;
for (const c of cols.values()) if (!classes.has(c.mangled)) { classes.set(c.mangled, { name: c.name, mangled: c.mangled, bases: c.bases, vtables: [] }); withoutVtable++; }
for (const [m, t] of baseTypes) if (!classes.has(m)) { classes.set(m, t); withoutVtable++; }
const list = [...classes.values()].sort((a, c) => a.name.localeCompare(c.name, 'en'));
const OUT = path.join(ROOT, 'audit', 'rtti.json');
fs.writeFileSync(OUT, '[\n' + list.map((c) => JSON.stringify(c)).join(',\n') + '\n]\n');
fs.mkdirSync(path.join(ROOT, 'build'), { recursive: true });
fs.writeFileSync(path.join(ROOT, 'build', 'rtti_vfuncs.json'), JSON.stringify(vfuncs));
const stats = { locators: cols.size, classes_with_vtables: list.length - withoutVtable, types_without_vtable: withoutVtable, vtables: nVtables, slots: nSlots, distinct_virtual_functions: Object.keys(vfuncs).length };
logRun({ tool: 'rtti', version: VERSION, outputs: [OUT], note: JSON.stringify(stats) });
console.log(JSON.stringify(stats));
const tres = list.filter((c) => /^[AUF]Tres/.test(c.name));
console.log('Tres* classes:', tres.length, tres.filter((_, i) => i % 400 === 0).map((c) => `${c.name} : ${c.bases.slice(0, 3).join(' > ')}`).join(' | '));
