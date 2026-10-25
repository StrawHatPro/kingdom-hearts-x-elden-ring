// Cheap sheet lookups, so nobody has to open a sheet file to answer a question.
//   node tools/q.js sheets [regex]                     the sheet kinds: group, file, columns
//   node tools/q.js <kind> [id regex] [column ...]     rows of a sheet kind ("" = every id), e.g.
//       node tools/q.js classes "^TresPlayer" id size system
//       node tools/q.js properties "^TresPlayerPawnBase\." name type offset
//       node tools/q.js row/TresAttackDataTable "" m_AttackKind      (the game's own tables)
//   --where col=regex   keep rows whose column matches (repeatable); col=null finds unchecked cells
//   --count             print only the number of matching rows
//   --all               lift the 40 row cap;  --json  one JSON object per row;  --cold  include provenance columns
//   node tools/q.js row <kind> <id>                    one row, one column per line
//   node tools/q.js todo [kind]                        unfinished rows per sheet from the last preflight
//   node tools/q.js sig <function id regex>            Class.Function(params) -> return
//   node tools/q.js fn <address or name regex>         one function across code, progress, exec, strings
//   node tools/q.js queue [n] [--where col=regex]      what to read first: the n (20) highest triage scores of
//                                                      impl/progress among functions that are not verified
//   node tools/q.js strings <text regex>               strings by content (id regex matches the address)
// Rows are streamed; a query stops reading as soon as the cap is reached.
const fs = require('fs');
const path = require('path');
const { ROOT, SHEETS, eachRow, sheetsByKind, rowsOf, normKey, dataFields, readDataSheet, readSheet } = require('./lib/sheets');

const argv = process.argv.slice(2);
const flag = (f) => { const i = argv.indexOf(f); if (i < 0) return false; argv.splice(i, 1); return true; };
const ALL = flag('--all'), COUNT = flag('--count'), JSONL = flag('--json'), COLD = flag('--cold');
const where = [];
for (let i; (i = argv.indexOf('--where')) >= 0;) { const [c, ...re] = argv[i + 1].split('='); where.push([c, re.join('=')]); argv.splice(i, 2); }
const cap = ALL || COUNT ? Infinity : 40;
const [kind, pattern, ...cols] = argv;
const show = (v) => (typeof v === 'string' ? (v.length > 100 && !ALL ? v.slice(0, 97) + '...' : v) : JSON.stringify(v));
const usage = () => { console.log(fs.readFileSync(__filename, 'utf8').split('\n').slice(1, 20).map((l) => l.replace(/^\/\/ ?/, '')).join('\n')); process.exit(1); };
if (!kind) usage();

const byKind = sheetsByKind();
const coldOf = (k) => new Set(rowsOf('schema', byKind).filter((r) => r.sheet === k && r.cold === 'yes').map((r) => r.column));

// Streams matching rows of a kind to fn(obj-like accessors); returns the number of matches.
function query(k, re, conds, pick, out) {
  let n = 0, printed = 0;
  const test = (get) => conds.every(([c, x]) => { const v = get(c); return x === 'null' ? v === null || v === undefined : v !== null && v !== undefined && new RegExp(x).test(typeof v === 'string' ? v : JSON.stringify(v)); });
  let checkedCols = false;
  const handle = (r, columns, col, file) => {
    if (!checkedCols) { checkedCols = true; for (const c of [...conds.map((x) => x[0]), ...(pick || [])]) if (!(c in col)) { console.log(`no column "${c}" in ${k}; columns: ${columns.join(' ')}`); process.exit(1); } }
    if (re && !re.test(String(r[0]))) return undefined;
    if (conds.length && !test((c) => r[col[c]])) return undefined;
    n++;
    if (printed < cap) { printed++; out(r, columns, col, file); }
    return printed >= cap && !COUNT && !ALL ? (n > cap ? false : undefined) : undefined;
  };
  if (byKind.has(k)) {
    for (const file of byKind.get(k)) { let stop = false; eachRow(file, (r, h) => { if (handle(r, h.columns, h.col, file) === false) { stop = true; return false; } return undefined; }); if (stop) break; }
  } else if (/^(row|object)\//.test(k)) {
    const fields = dataFields();
    const tables = rowsOf('data-tables', byKind).filter((t) => t.classes.includes(k));
    outer: for (const t of tables) {
      for (const v of readDataSheet(path.join(SHEETS, t.id), fields).views) {
        if (v.sheet !== k) continue;
        for (const r of v.rows) if (handle(r, v.columns, v.col, t.id) === false) break outer;
      }
    }
  } else { console.log(`no sheet kind "${k}" (node tools/q.js sheets)`); process.exit(1); }
  return n;
}

if (kind === 'sheets') {
  const re = new RegExp(pattern || '.', 'i');
  const schema = rowsOf('schema', byKind);
  for (const s of rowsOf('sheets', byKind)) if (re.test(s.id)) console.log(`${s.id.padEnd(15)} ${s.file.padEnd(34)} ${schema.filter((r) => r.sheet === s.id).map((r) => r.column + (r.role === 'ref' ? '>' + r.ref : '')).join(' ')}`);
  const ds = rowsOf('data-structs', byKind).filter((s) => re.test(s.id));
  if (pattern) for (const s of ds.slice(0, ALL ? Infinity : 40)) console.log(`${s.id.padEnd(40)} ${s.tables} tables, ${s.rows} rows, ${s.fields} fields`);
  else console.log(`row/<RowStruct>, object/<Class>   ${ds.length} kinds of game data rows; list them with: node tools/q.js data-structs`);
} else if (kind === 'todo') {
  const file = path.join(ROOT, 'build', 'preflight.json');
  if (!fs.existsSync(file)) { console.log('run node tools/preflight.js first'); process.exit(1); }
  const rep = JSON.parse(fs.readFileSync(file, 'utf8'));
  if (pattern && rep.sheets[pattern]) {
    const s = rep.sheets[pattern];
    console.log(`${pattern}: ${s.unfinished_rows} of ${s.rows} rows unfinished`);
    console.log('unchecked by column: ' + JSON.stringify(s.unchecked_by_column));
    if (s.dangling_refs) console.log('dangling by column:  ' + JSON.stringify(s.dangling_by_column) + '\n  ' + s.dangling_examples.join('\n  '));
    console.log('first unfinished rows:\n  ' + s.first_unfinished.slice(0, ALL ? Infinity : 40).join('\n  '));
  } else {
    const re = new RegExp(pattern || '.');
    const list = Object.entries(rep.sheets).filter(([k, s]) => re.test(k) && (s.unfinished_rows || s.dangling_refs)).sort((a, b) => b[1].unchecked_cells + b[1].dangling_refs - a[1].unchecked_cells - a[1].dangling_refs);
    for (const [k, s] of list.slice(0, cap)) console.log(`${k.padEnd(44)} ${String(s.unfinished_rows).padStart(8)} rows  ${Object.entries(s.unchecked_by_column).map(([c, n]) => `${c}:${n}`).join(' ')}${s.dangling_refs ? '  dangling ' + Object.entries(s.dangling_by_column).map(([c, n]) => `${c}:${n}`).join(' ') : ''}`);
    if (list.length > cap) console.log(`... ${list.length - cap} more (--all)`);
    console.log(`${rep.status}: ${JSON.stringify(rep.totals)}`);
  }
} else if (kind === 'row') {
  const [k, id] = [pattern, cols[0]];
  if (!k || id === undefined) usage();
  const want = normKey(id);
  let found = false;
  if (!byKind.has(k) && !/^(row|object)\//.test(k)) { console.log(`no sheet kind "${k}" (node tools/q.js sheets)`); process.exit(1); }
  const print = (r, columns) => { if (found || normKey(String(r[0])) !== want) return; found = true; columns.forEach((c, i) => console.log(`${c.padEnd(18)} ${ALL ? JSON.stringify(r[i]) : show(r[i])}`)); };
  if (byKind.has(k)) for (const file of byKind.get(k)) { eachRow(file, (r, h) => { print(r, h.columns); return found ? false : undefined; }); if (found) break; }
  else { const fields = dataFields(); for (const t of rowsOf('data-tables', byKind).filter((x) => x.classes.includes(k))) for (const v of readDataSheet(path.join(SHEETS, t.id), fields).views) if (v.sheet === k) for (const r of v.rows) print(r, v.columns); }
  if (!found) console.log(`no row ${id} in ${k}`);
} else if (kind === 'sig') {
  const re = new RegExp(pattern || '.');
  const want = new Map();
  query('functions', re, where, null, (r) => want.set(r[0], []));
  eachRow(byKind.get('properties')[0], (r, h) => { const l = want.get(r[h.col.owner]); if (l) l.push({ ord: r[h.col.ord], name: r[h.col.name], type: r[h.col.type], flags: r[h.col.flags], offset: r[h.col.offset] }); });
  for (const [id, ps] of want) {
    ps.sort((a, b) => (a.offset ?? 1e9) - (b.offset ?? 1e9) || b.ord - a.ord);
    const f = (p) => (p.flags ? BigInt(p.flags) : 0n);
    const ret = ps.filter((p) => f(p) & 0x400n);
    console.log(`${id}(${ps.filter((p) => !(f(p) & 0x400n)).map((p) => `${f(p) & 0x100n ? 'out ' : ''}${p.name ?? '?'}: ${p.type}`).join(', ')})${ret.length ? ' -> ' + ret.map((p) => p.type).join(', ') : ''}`);
  }
} else if (kind === 'fn') {
  // one function, joined across the sheets that describe it
  const isAddr = /^0x[0-9a-fA-F]+$/.test(pattern || '');
  const hits = [];
  const re = isAddr ? null : new RegExp(pattern || '.');
  const want = isAddr ? normKey(pattern) : null;
  for (const file of byKind.get('code') || []) eachRow(file, (r, h) => { if (isAddr ? normKey(r[0]) === want : re.test(r[h.col.name])) { hits.push(Object.fromEntries(h.columns.map((c, i) => [c, r[i]]))); if (isAddr || hits.length >= cap) return false; } return undefined; });
  const ids = new Set(hits.map((x) => normKey(x.id)));
  const extra = new Map();
  for (const [k, col] of [['progress', 'id'], ['exec', 'ref_addr'], ['calls', 'id']]) for (const file of byKind.get(k) || []) eachRow(file, (r, h) => { const a = normKey(r[h.col[col]]); if (ids.has(a)) (extra.get(a) || extra.set(a, []).get(a)).push(`${k}: ${h.columns.filter((c) => c !== col && c !== 'evidence').map((c) => `${c}=${show(r[h.col[c]])}`).join(' ')}`); });
  for (const x of hits) { console.log(Object.entries(x).map(([c, v]) => `${c}=${show(v)}`).join('  ')); for (const l of extra.get(normKey(x.id)) || []) console.log('    ' + l); }
  if (!hits.length) console.log('no function matches');
  else if (hits.length === 1) { const lines = []; for (const file of byKind.get('strings') || []) eachRow(file, (r, h) => { if (r[h.col.used_by].some((u) => ids.has(normKey(u)))) lines.push(`    string ${r[0]} ${JSON.stringify(show(r[h.col.text]))}`); }); lines.slice(0, cap).forEach((l) => console.log(l)); if (lines.length > cap) console.log(`    ... ${lines.length - cap} more strings (--all)`); }
} else if (kind === 'queue') {
  // the work queue is a query: highest score first, verified functions left out
  const n = /^\d+$/.test(pattern || '') ? Number(pattern) : 20;
  const top = [];
  let total = 0;
  for (const file of byKind.get('progress') || []) eachRow(file, (r, h) => {
    if (r[h.col.status] === 'verified') return;
    if (!where.every(([c, x]) => { const v = r[h.col[c]]; return x === 'null' ? v === null : v !== null && new RegExp(x).test(String(v)); })) return;
    total++;
    const sc = r[h.col.score];
    if (top.length < n || sc > top[top.length - 1][0]) { top.push([sc, r, h]); top.sort((a, b) => b[0] - a[0]); if (top.length > n) top.pop(); }
  });
  for (const [, r, h] of top) console.log(['score', 'id', 'status', 'name', 'how', 'verdict', 'strings', 'entry_depth'].map((c) => `${c}=${show(r[h.col[c]])}`).join('  '));
  console.log(`${top.length} of ${total} functions that are not verified${where.length ? ' and match' : ''}`);
} else if (kind === 'strings' && pattern && !/^\^?0x/.test(pattern)) {
  const re = new RegExp(pattern);
  let n = 0;
  for (const file of byKind.get('strings') || []) eachRow(file, (r, h) => { if (!re.test(r[h.col.text])) return undefined; n++; if (COUNT) return undefined; if (n > cap) return false; console.log(`id=${r[0]}  encoding=${r[h.col.encoding]}  text=${JSON.stringify(show(r[h.col.text]))}  used_by=${show(r[h.col.used_by])}`); return undefined; });
  console.log(COUNT ? n : n > cap ? '... more rows match (--all to list, --count to count)' : n ? `${n} strings` : 'no rows match');
} else {
  let re = null;
  try { re = pattern ? new RegExp(pattern) : null; } catch (e) { console.log('bad regex: ' + e.message); process.exit(1); }
  const cold = COLD || cols.length ? new Set() : byKind.has(kind) ? coldOf(kind) : new Set();
  const n = query(kind, re, where, cols, (r, columns, col) => {
    if (COUNT) return;
    const pick = cols.length ? cols : columns.filter((c) => !cold.has(c));
    if (JSONL) console.log(JSON.stringify(Object.fromEntries(pick.map((c) => [c, r[col[c]]]))));
    else console.log(pick.map((c) => `${c}=${show(r[col[c]])}`).join('  '));
  });
  if (COUNT) console.log(n);
  else if (n > cap) console.log(`... more rows match (--all to list, --count to count)`);
  else if (!n) console.log('no rows match');
}
