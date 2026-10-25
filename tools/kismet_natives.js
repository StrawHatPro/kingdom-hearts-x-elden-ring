// The Blueprint VM's native table (GNatives) as the executable fills it:
// every "lea rax,[handler]; mov [GNatives + 8*opcode],rax" store in base/KH3.exe.
//   node tools/kismet_natives.js      -> audit/kismet_natives.json
// GNatives is the table at 0x149C6E260: the interpreter's dispatch reads it as
// "(&g_149C6E260)[opcode]" (e.g. func_142F4AEB0, func_142F5CDD0 in build/decomp).
const path = require('path');
const { ROOT, logRun, writeJson } = require('./lib/log');
const pe = require('./lib/pe');
require('./lib/package'); // loads the class modules, which load lib/kismet
const { BY_NAME } = require('./lib/kismet');

const VERSION = '1.0.0';
const BASE = 0x149C6E260;
const img = pe.load();
const base = Number(img.imageBase);
const b = img.b;
const byCode = {};
for (const [name, d] of Object.entries(BY_NAME)) byCode[d.code] = name;
const table = {};
for (const s of img.sections) {
  if (!(s.flags & 0x20000000)) continue;
  for (let o = s.rawPtr + 10; o < s.rawPtr + s.rawSize - 4; o++) {
    const va = base + s.rva + (o - s.rawPtr);
    const t = va + 4 + b.readInt32LE(o);
    if (t < BASE || t >= BASE + 0x800 || (t - BASE) % 8) continue;
    if (!(b[o - 3] === 0x48 && b[o - 2] === 0x89 && b[o - 1] === 0x05 && b[o - 10] === 0x48 && b[o - 9] === 0x8d && b[o - 8] === 0x05)) continue;
    const idx = (t - BASE) / 8;
    const handler = va - 10 + 7 + b.readInt32LE(o - 7);
    const key = '0x' + idx.toString(16).toUpperCase().padStart(2, '0');
    (table[key] = table[key] || []).push({ store_instruction: '0x' + (va - 3).toString(16).toUpperCase(), handler: '0x' + handler.toString(16).toUpperCase(), name_in_tools_lib_kismet: byCode[idx] || null });
  }
}
const sorted = Object.fromEntries(Object.entries(table).sort((a, b2) => parseInt(a[0], 16) - parseInt(b2[0], 16)));
const OUT = path.join(ROOT, 'audit', 'kismet_natives.json');
const registered = Object.keys(sorted).map((k) => parseInt(k, 16));
writeJson(OUT, {
  generated_by: 'tools/kismet_natives.js', version: VERSION, gnatives: '0x' + BASE.toString(16).toUpperCase(),
  registered_opcodes: registered.length,
  registered_without_a_decoder_entry: Object.entries(sorted).filter(([, v]) => !v[0].name_in_tools_lib_kismet).map(([k]) => k),
  decoder_entries_without_a_registration: Object.entries(BY_NAME).filter(([, d]) => !registered.includes(d.code)).map(([n, d]) => `0x${d.code.toString(16).toUpperCase().padStart(2, '0')} ${n}`),
  table: sorted,
});
logRun({ tool: 'kismet_natives', version: VERSION, outputs: [OUT] });
console.log('registered opcodes', registered.length, 'without decoder entry', Object.entries(sorted).filter(([, v]) => !v[0].name_in_tools_lib_kismet).map(([k]) => k).join(' '));
console.log('decoder entries without registration', Object.entries(BY_NAME).filter(([, d]) => !registered.includes(d.code)).map(([n]) => n).join(' '));
