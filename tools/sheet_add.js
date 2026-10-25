// Adds or replaces one row of an authored sheet (sheets/doctrine.json, decisions.json,
// tokens.json): the sheets whose rows are decisions and records, not extracted evidence.
//   node tools/sheet_add.js <kind> id=<id> col=value [col=value ...]
// A column that is not given stays unchecked (null); give col=- for "deliberately none".
// A row with the same id is replaced. Columns and types come from tools/lib/sheet_defs.js.
// Every call is logged in audit/tool_log.jsonl, so who wrote which row when stays traceable.
const fs = require('fs');
const path = require('path');
const { logRun } = require('./lib/log');
const { SHEETS, readSheet, writeSheet } = require('./lib/sheets');
const { DEFS, columnsOf, fileOf } = require('./lib/sheet_defs');

const VERSION = '1.0.0';
const [kind, ...pairs] = process.argv.slice(2);
const def = DEFS[kind];
if (!def || !/authored/.test(def.by)) { console.log(`usage: node tools/sheet_add.js <${Object.keys(DEFS).filter((k) => /authored/.test(DEFS[k].by)).join('|')}> id=<id> col=value ...`); process.exit(1); }
const cols = columnsOf(kind);
const row = new Array(cols.length).fill(null);
for (const p of pairs) {
  const i = p.indexOf('=');
  const c = p.slice(0, i), v = p.slice(i + 1);
  const at = cols.indexOf(c);
  if (i < 0 || at < 0) { console.log(`no column "${c}" in ${kind}; columns: ${cols.join(' ')}`); process.exit(1); }
  const type = def.cols[at][2];
  row[at] = v === 'null' ? null : v === '-' ? '-' : type === 'int' || type === 'num' ? Number(v) : v;
  if ((type === 'int' || type === 'num') && typeof row[at] === 'number' && !Number.isFinite(row[at])) { console.log(`${c}=${v} is not a number`); process.exit(1); }
}
if (row[0] === null) { console.log('id=<id> is required'); process.exit(1); }
const file = path.join(SHEETS, fileOf(kind));
const rows = fs.existsSync(file) ? readSheet(file).rows : [];
const at = rows.findIndex((r) => r[0] === row[0]);
if (at >= 0) rows[at] = row; else rows.push(row);
writeSheet(file, kind, def.group, cols, rows);
logRun({ tool: 'sheet_add', version: VERSION, outputs: [file], note: `${kind} ${at >= 0 ? 'replaced' : 'added'} ${row[0]}` });
console.log(`${kind}: ${at >= 0 ? 'replaced' : 'added'} ${row[0]} (${rows.length} rows)`);
