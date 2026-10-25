#!/usr/bin/env node
// (PACKGEN) sheets/reflect/enum-values.json for the asset-pack / generated-table build on a player's PC.
//
//   node tools/packgen_enums.js [--out FILE]
//
// The full sheet is written by tools/extract.js from the whole evidence chain of the decompilation (every DataTable of the
// paks, the symbol tables, ...), which a player's PC does not have. Everything the pack tools and tools/gen_port.js read from
// it is the enums the executable registers: their names and values (audit/reflection.json, written by tools/reflection.js
// from the player's own KINGDOM HEARTS III executable). This tool writes exactly those rows, with the same ids, order and
// file layout as tools/extract.js; the `uses` column (data cells holding the value, counted over every DataTable of the
// game) is 0 and the value names only cooked data uses (no registered value) are left out. tools/gen_port.js gives the same
// tres_enums.rs from it, and tools/party_ai_pack.js the same tables.
'use strict';
const fs = require('fs');
const path = require('path');
const { ROOT } = require('./lib/log');
const { writeSheet } = require('./lib/sheets');
const { DEFS, columnsOf } = require('./lib/sheet_defs');

const args = process.argv.slice(2);
const OUT = args.includes('--out') ? path.resolve(args[args.indexOf('--out') + 1]) : path.join(ROOT, 'sheets', 'reflect', 'enum-values.json');
const REFL = path.join(ROOT, 'audit', 'reflection.json');
if (!fs.existsSync(REFL)) { console.error(`${path.relative(ROOT, REFL)} is missing: run node tools/reflection.js first`); process.exit(1); }
const recs = JSON.parse(fs.readFileSync(REFL, 'utf8'));
const addr = (a) => (a === null || a === undefined ? null : '0x' + String(a).replace(/^0x/i, '').toUpperCase());
// as tools/extract.js
const unique = (seen, id, alt) => { let k = id; if (seen.has(k)) k = `${id}${alt}`; for (let n = 2; seen.has(k); n++) k = `${id}${alt}~${n}`; seen.add(k); return k; };

const enumIds = new Set();
const enums = [];
for (const r of recs) {
  if (r.kind !== 'enum') continue;
  const regfn = addr(r.registration_function);
  const id = unique(enumIds, r.name || '@' + regfn, '@' + regfn);
  enums.push({ id, values: r.enum.values });
}
const rows = [], seenIds = new Set();
for (const e of enums) for (const [name, value] of e.values) rows.push([unique(seenIds, `${e.id}/${name}`, '#'), e.id, name, value, 0]);
const n = writeSheet(OUT, 'enum-values', DEFS['enum-values'].group, columnsOf('enum-values'), rows);
console.log(`packgen_enums: ${enums.length} enums, ${n} values -> ${path.relative(ROOT, OUT).replace(/\\/g, '/')}`);
