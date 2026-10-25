// Exports the recovered function names for Ghidra: sheets/exe/names -> build/names/apply_names.tsv
// (address <TAB> name <TAB> source), the input of tools/ghidra/ApplyNames.java.
//   node tools/names_tsv.js [--vtable-labels] [out.tsv]
// Every name in the file belongs to one address: a name that two rows share gets the suffix
// @0x<address> on each. White space, which Ghidra does not accept in a symbol, becomes "_".
//   --vtable-labels   also write a positional label for functions that have no name but one owning
//                     class in sheets/exe/vfuncs: <Class>::vfunc_<slot>, source "vtable-slot".
//                     These are positions, not recovered names; they are never written to
//                     sheets/exe/names and are off by default.
const fs = require('fs');
const path = require('path');
const { ROOT, logRun } = require('./lib/log');
const S = require('./lib/sheets');

const VERSION = '1.0.0';
const LABELS = process.argv.includes('--vtable-labels');
const OUT = path.resolve(process.argv.slice(2).find((a) => !a.startsWith('--')) || path.join(ROOT, 'build', 'names', 'apply_names.tsv'));
const byKind = S.sheetsByKind();
const rows = [];
const named = new Set();
for (const r of S.rowsOf('names', byKind)) { rows.push({ addr: r.ref_addr, name: r.name, source: r.basis }); named.add(S.normKey(r.ref_addr)); }
const stats = { names: rows.length, vtable_labels: 0, suffixed: 0, sanitised: 0, by_source: {} };
if (LABELS) {
  const cpp = new Map(S.rowsOf('classes', byKind).map((c) => [c.id, c.cpp_name || c.id]));
  for (const v of S.rowsOf('vfuncs', byKind)) {
    if (named.has(S.normKey(v.ref_addr)) || v.class === '-' || typeof v.slot !== 'number') continue;
    rows.push({ addr: v.ref_addr, name: `${v.owner_kind === 'class' ? cpp.get(v.class) || v.class : v.class}::vfunc_${v.slot}`, source: 'vtable-slot' });
    stats.vtable_labels++;
  }
}
for (const r of rows) if (/\s/.test(r.name)) { r.name = r.name.replace(/\s+/g, '_'); stats.sanitised++; }
const count = new Map();
for (const r of rows) count.set(r.name, (count.get(r.name) || 0) + 1);
for (const r of rows) { if (count.get(r.name) > 1) { r.name += '@0x' + r.addr.replace(/^0x/i, '').toUpperCase(); stats.suffixed++; } stats.by_source[r.source] = (stats.by_source[r.source] || 0) + 1; }
rows.sort((a, b) => parseInt(a.addr, 16) - parseInt(b.addr, 16));
fs.mkdirSync(path.dirname(OUT), { recursive: true });
fs.writeFileSync(OUT, '# address\tname\tsource   (tools/names_tsv.js from sheets/exe/names; apply with tools/ghidra/ApplyNames.java)\n' + rows.map((r) => `${r.addr.replace(/^0x/i, '').toUpperCase()}\t${r.name}\t${r.source}`).join('\n') + '\n');
logRun({ tool: 'names_tsv', version: VERSION, outputs: [OUT], note: JSON.stringify(stats) });
console.log(`names_tsv: ${rows.length} lines -> ${path.relative(ROOT, OUT)}  ${JSON.stringify(stats)}`);
