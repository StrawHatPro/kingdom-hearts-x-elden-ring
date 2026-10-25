// Evidence sheets about the executable's data that code refers to (method section 8.4):
//   sheets/exe/strings/ascii.json, utf16.json   one row per string in the data sections
//   sheets/exe/constants.json                   one row per typed scalar constant
// with, for each row, the functions that refer to it (used_by).
// Inputs: audit/data_map.json (tools/datamap.js: which ranges are strings and scalars, and how
// sure that is), base/KH3.exe (the bytes), sheets/native/code.json (function extents).
// used_by: every function of native/code is decoded with iced-x86 from its entry for `size`
// bytes (not past the next function's entry) and every RIP-relative memory operand that names
// the first byte of a string or constant counts as a reference. Not seen: references from data
// (pointer tables), references by absolute address, and code of the few functions whose body is
// not one contiguous run (their count is printed).
//   node tools/xref_sheets.js
const fs = require('fs');
const path = require('path');
const iced = require('iced-x86');
const { ROOT, logRun } = require('./lib/log');
const { SHEETS, eachLine, eachRow, writeSheet } = require('./lib/sheets');
const { DEFS, columnsOf } = require('./lib/sheet_defs');
const pe = require('./lib/pe');

const VERSION = '1.0.0';
const img = pe.load();
const BASE = Number(img.imageBase);
const off = (va) => img.rvaToOff(va - BASE);
const id = (va) => '0x' + va.toString(16);

const STRING = { string_ascii: 'ascii', string_ascii_unreferenced: 'ascii', string_utf16: 'utf16', string_utf16_unreferenced: 'utf16' };
const SCALAR = new Set(['float32', 'float64', 'scalar8', 'scalar16', 'scalar32', 'scalar64', 'scalar128', 'simd128_const']);

// ---- targets from the data map
const items = [];             // { va, end, type, conf, users: [] }
const byVa = new Map();
eachLine(path.join(ROOT, 'audit', 'data_map.json'), (line) => {
  if (line.charCodeAt(0) !== 123) return;
  const t = /"type":"([^"]+)"/.exec(line);
  if (!t || !(STRING[t[1]] || SCALAR.has(t[1]))) return;
  const r = JSON.parse(line.endsWith(',') ? line.slice(0, -1) : line);
  const it = { va: parseInt(r.start, 16), end: parseInt(r.end, 16), type: r.type, conf: r.confidence, users: [] };
  items.push(it); byVa.set(it.va, it);
});

// ---- references from code
const starts = [], sizes = [];
eachRow(path.join(SHEETS, 'native', 'code.json'), (r, h) => { starts.push(parseInt(r[h.col.id], 16)); sizes.push(r[h.col.size]); });
const ins = new iced.Instruction();
let refs = 0, clipped = 0, instructions = 0;
for (let i = 0; i < starts.length; i++) {
  const a = starts[i];
  let e = a + sizes[i];
  if (i + 1 < starts.length && e > starts[i + 1]) { e = starts[i + 1]; clipped++; }
  const o = off(a);
  if (o < 0 || e <= a) continue;
  const dec = new iced.Decoder(64, img.b.subarray(o, o + (e - a)), iced.DecoderOptions.None);
  dec.ip = BigInt(a);
  let last = null;
  while (dec.canDecode) {
    dec.decodeOut(ins);
    instructions++;
    if (!ins.isIpRelMemoryOperand) continue;
    const it = byVa.get(Number(ins.ipRelMemoryAddress));
    if (!it || it === last) continue;
    if (it.users[it.users.length - 1] !== a) { it.users.push(a); refs++; }
    last = it;
  }
  dec.free();
}
ins.free();

// ---- sheets
const text = (it, wide) => {
  const o = off(it.va);
  let n = it.end - it.va;
  if (wide) { let k = 0; while (k + 1 < n && (img.b[o + k] || img.b[o + k + 1])) k += 2; n = k; } else { let k = 0; while (k < n && img.b[o + k]) k++; n = k; }
  return img.b.toString(wide ? 'utf16le' : 'latin1', o, o + n);
};
const outputs = [];
const count = {};
for (const enc of ['ascii', 'utf16']) {
  const file = path.join(SHEETS, 'exe', 'strings', `${enc}.json`);
  const rows = items.filter((it) => STRING[it.type] === enc).map((it) => [id(it.va), enc, it.end - it.va, text(it, enc === 'utf16'), it.conf, it.users.map(id)]);
  count[`strings/${enc}`] = writeSheet(file, 'strings', DEFS.strings.group, columnsOf('strings'), rows);
  outputs.push(file);
}
{
  const file = path.join(SHEETS, 'exe', 'constants.json');
  const rows = items.filter((it) => SCALAR.has(it.type)).map((it) => {
    const o = off(it.va), n = it.end - it.va;
    let value = '-';
    if (it.type === 'float32') value = img.b.readFloatLE(o);
    else if (it.type === 'float64') value = img.b.readDoubleLE(o);
    else if (it.type === 'scalar8') value = img.b[o];
    else if (it.type === 'scalar16') value = img.b.readUInt16LE(o);
    else if (it.type === 'scalar32') value = img.b.readUInt32LE(o);
    if (typeof value === 'number' && !Number.isFinite(value)) value = '-'; // NaN and infinities: see `raw`
    return [id(it.va), it.type, n, img.b.toString('hex', o, o + n), value, it.conf, it.users.map(id)];
  });
  count.constants = writeSheet(file, 'constants', DEFS.constants.group, columnsOf('constants'), rows);
  outputs.push(file);
}
logRun({ tool: 'xref_sheets', version: VERSION, outputs, note: `${JSON.stringify(count)}, ${refs} references from ${starts.length} functions` });
console.log(`xref_sheets: ${JSON.stringify(count)}; ${instructions} instructions decoded, ${refs} references; ${clipped} functions decoded only up to the next entry (non-contiguous body)`);
