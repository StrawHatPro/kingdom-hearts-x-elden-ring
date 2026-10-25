// Preflight checklist of the Spreadsheet Method. Runs before every generation and build.
// It overlaps every sheet with the schema and with the other sheets and reports, per sheet,
// what is unfinished. Layers (after section 6 of THE-SPREADSHEET-METHOD):
//   L0 structure   every sheet file parses, its columns are exactly the schema's, every declared
//                  sheet exists, keys are present and unique
//   L1 checkmarks  every row x column intersection holds a value of the column's type; a row
//                  with an unchecked (null) cell is unfinished
//   L2 refs        every ref cell resolves to a key of its target sheet (dangling otherwise)
//   L3 coverage    overlaps between sheets that should have the same rows; generated files exist
//   L4 assets      with --assets: every assets/files row exists on disk (556,711 stats; slow)
//   L5 evidence    sheets/schema.json and index.json are what tools/lib/sheet_defs.js declares
//   L6 budget      the token ledger (sheets/tokens.json)
// Also from the method's check card (Appendix C): no BOM, LF only; every hand-written tool is a
// row of sheets/kernel.json; nothing generated outside build/gen; the emitter version equals the
// doctrine sheet's; bulk exports have at least min_rows rows; no cycles in a sheet's own
// hierarchy (super, bases); which evidence sheets lack an address or confidence column; which
// sheets changed their row count since the last run. Not applicable here and not checked: units,
// id charset and immutability against a commit (no git), RNG and parity traces (no port), dead
// columns (the sheets are evidence, not only emitter input). Determinism of the emitters is the
// job of the --check gates in tools/check.js.
// The game's data sheets (sheets/entities, sheets/systems) are checked the same way, with the
// columns of their row struct (sheets/data/fields.json).
//   node tools/preflight.js [--summary] [--json] [--no-cache] [--assets]
// Exit code: 0 everything checked; 1 errors (L0, wrong types, duplicate keys); 2 no errors but
// unchecked cells, dangling refs or coverage gaps remain. Full detail: build/preflight.json.
const fs = require('fs');
const path = require('path');
const { ROOT, SHEETS, checked, normKey, eachRow, readHeader, listSheets, listDataSheets, dataFields, readDataSheet, dataChecked, DATA_KEY_COLUMNS } = require('./lib/sheets');

const SUMMARY = process.argv.includes('--summary');
const JSON_OUT = process.argv.includes('--json');
const CAP = 200; // ids kept per list in build/preflight.json
const t0 = Date.now();
const errors = [];   // { code, sheet, row, col, msg }
const errorCount = {};
const warnings = []; // reported, never change the exit code
function err(code, sheet, msg, row, col) {
  errorCount[code] = (errorCount[code] || 0) + 1;
  if (errorCount[code] <= CAP) errors.push({ code, sheet, row: row ?? null, col: col ?? null, msg });
}

// ---- L0: schema and files
const SCHEMA = path.join(SHEETS, 'schema.json');
if (!fs.existsSync(SCHEMA)) { console.log('PREFLIGHT FAILED  sheets/schema.json is missing (run node tools/extract.js)'); process.exit(1); }
const spec = new Map(); // kind -> [{ column, role, type, ref }]
eachRow(SCHEMA, (r, h) => {
  const o = Object.fromEntries(h.columns.map((c, i) => [c, r[i]]));
  (spec.get(o.sheet) || spec.set(o.sheet, []).get(o.sheet)).push(o);
});
const files = new Map(); // kind -> [{ file, head }]
for (const f of listSheets()) {
  const file = path.join(SHEETS, f);
  {
    const fd = fs.openSync(file, 'r'), b = Buffer.alloc(4096), n = fs.readSync(fd, b, 0, 4096, 0);
    fs.closeSync(fd);
    if (n >= 3 && b[0] === 0xEF && b[1] === 0xBB && b[2] === 0xBF) err('E-L0-BOM', f, 'file starts with a byte order mark');
    const nl = b.subarray(0, n).indexOf(10);
    if (nl > 0 && b[nl - 1] === 13) err('E-L0-EOL', f, 'CRLF line ending (sheets are LF only)');
  }
  let head;
  try { head = readHeader(file); } catch (e) { err('E-L0-PARSE', f, `not a sheet: ${e.message.slice(0, 120)}`); continue; }
  if (!head.sheet || !Array.isArray(head.columns)) { err('E-L0-PARSE', f, 'no "sheet" or "columns" in the header line'); continue; }
  (files.get(head.sheet) || files.set(head.sheet, []).get(head.sheet)).push({ file, rel: f, head });
}
// A declared sheet with no file yet is a coverage gap (planned, not produced), not a structural error.
const absentSheets = [...spec.keys()].filter((kind) => !files.has(kind));
for (const [kind, list] of files) {
  const cols = spec.get(kind);
  if (!cols) { err('E-L0-NOSCHEMA', kind, `sheet has no rows in sheets/schema.json (${list[0].rel})`); continue; }
  for (const { rel, head } of list) {
    for (const c of head.columns) if (!cols.some((x) => x.column === c)) err('E-L0-COLUMN', kind, `column not in the schema (${rel})`, null, c);
    for (const x of cols) if (!(x.column in head.col)) err('E-L0-COLUMN', kind, `schema column missing from the sheet (${rel})`, null, x.column);
  }
}

// ---- L1: checkmarks, types, keys
const TYPE = {
  str: (v) => typeof v === 'string', int: Number.isInteger, num: (v) => typeof v === 'number' && Number.isFinite(v),
  addr: (v) => typeof v === 'string' && /^0x[0-9a-fA-F]+$/.test(v), hex: (v) => typeof v === 'string' && /^0x[0-9a-fA-F]+$/.test(v),
  yesno: (v) => v === 'yes' || v === 'no', list: Array.isArray, json: () => true,
};
const typeTest = (t) => {
  if (t && t.startsWith('enum:')) { const set = new Set(t.slice(5).split('|')); return (v) => set.has(v); }
  return TYPE[t] || null;
};
const stats = new Map();   // kind -> { group, files, rows, finished, cells, unchecked, gaps:{col:n}, dangling:{col:n}, unfinished:[ids], total_dangling }
const keys = new Map();    // kind -> Set of normalised keys
const idSets = new Map();  // kind -> Set of ids, for kinds whose id column is a ref (overlaps)
const newStat = (group) => ({ group, files: 0, rows: 0, finished: 0, cells: 0, unchecked: 0, gaps: {}, dangling: {}, dangling_total: 0, unfinished: [] });
const missing = new Map(); // "target kind(s) <- value" -> inbound refs (blast radius)
let refCells = 0;
// One read per sheet: a sheet is read after the sheets its refs point at, so keys are known and
// the refs are followed in the same pass. Refs into the sheet itself, or into a sheet that could
// not be ordered first (a cycle), are followed in a second read of that sheet only.
const deferred = new Map(); // kind -> ref columns to follow in a second read
const HIERARCHY = new Set(['super', 'bases', 'owner']); // columns that must form a tree or DAG within their own sheet
const selfEdges = new Map(); // kind -> Map(key -> [keys of the same sheet it points at])
function scan(kind, list, done) {
  const cols = spec.get(kind);
  const st = stats.get(kind) || newStat(list[0].head.group);
  const first = !stats.has(kind);
  stats.set(kind, st);
  const keyDef = cols.find((c) => c.role === 'key');
  const ks = first ? new Set() : keys.get(kind);
  const ids = first && !keyDef && cols[0] && cols[0].column === 'id' ? new Set() : null;
  const refs = cols.filter((c) => c.role === 'ref' && c.ref && c.ref !== '-');
  const later = [];
  for (const { file, rel, head } of list) {
    if (first) st.files++;
    const tests = head.columns.map((c) => { const d = cols.find((x) => x.column === c); return d ? typeTest(d.type) : null; });
    const names = head.columns, n = names.length, ki = keyDef ? head.col[keyDef.column] : -1, ii = ids ? head.col.id : -1;
    const selfCols = first && keyDef ? refs.filter((c) => HIERARCHY.has(c.column) && c.ref.split('|').includes(kind) && c.column in head.col).map((c) => head.col[c.column]) : [];
    const edges = selfCols.length ? selfEdges.get(kind) || selfEdges.set(kind, new Map()).get(kind) : null;
    const plan = [];
    for (const c of refs) {
      if (!(c.column in head.col)) continue;
      const targets = c.ref.split('|');
      const ready = targets.every((t) => done.has(t) || !files.has(t) || !spec.has(t));
      if (first ? ready : deferred.get(kind).includes(c.column)) plan.push({ i: head.col[c.column], name: c.column, target: c.ref, sets: targets.map((t) => keys.get(t)).filter(Boolean) });
      if (first && !ready && !later.includes(c.column)) later.push(c.column);
    }
    if (!first && !plan.length) continue;
    try {
      eachRow(file, (r) => {
        if (first) {
          st.rows++;
          let ok = r.length === n;
          if (!ok) err('E-L0-ROW', kind, `row has ${r.length} cells for ${n} columns (${rel})`, r[0]);
          for (let i = 0; i < n; i++) {
            const v = r[i];
            st.cells++;
            if (v === null || v === undefined) { ok = false; st.unchecked++; st.gaps[names[i]] = (st.gaps[names[i]] || 0) + 1; }
            else if (v !== '-' && tests[i] && !tests[i](v)) err('E-L1-TYPE', kind, `${JSON.stringify(v).slice(0, 60)} is not ${cols.find((x) => x.column === names[i]).type}`, r[0], names[i]);
          }
          if (ok) st.finished++; else if (st.unfinished.length < CAP) st.unfinished.push(r[0]);
          if (ki >= 0) {
            const k = normKey(r[ki]);
            if (!checked(k)) err('E-L0-KEY', kind, 'row without a key', r[0], keyDef.column);
            else if (ks.has(k)) err('E-L0-KEY', kind, 'duplicate key', r[ki], keyDef.column);
            else ks.add(k);
          }
          if (ii >= 0 && checked(r[ii])) ids.add(normKey(r[ii]));
          if (edges) { const to = []; for (const i of selfCols) { const v = r[i]; if (Array.isArray(v)) to.push(...v); else if (typeof v === 'string' && v !== '-') to.push(v); } if (to.length) edges.set(r[ki], to); }
        }
        for (const p of plan) {
          const cell = r[p.i];
          if (cell === null || cell === undefined || cell === '-') continue;
          const many = Array.isArray(cell);
          for (let j = 0, m = many ? cell.length : 1; j < m; j++) {
            const raw = many ? cell[j] : cell;
            if (raw === null || raw === '-') continue;
            refCells++;
            const v = normKey(raw);
            let hit = false;
            for (const t of p.sets) if (t.has(v)) { hit = true; break; }
            if (hit) continue;
            st.dangling[p.name] = (st.dangling[p.name] || 0) + 1; st.dangling_total++;
            const mk = `${p.target} <- ${raw}`;
            missing.set(mk, (missing.get(mk) || 0) + 1);
            if (st.dangling_total <= 20) (st.dangling_examples = st.dangling_examples || []).push(`${r[0]}.${p.name} -> ${raw}`);
          }
        }
      });
    } catch (e) { err("E-L0-PARSE", kind, `${rel}: ${e.message.slice(0, 160)}`); }
  }
  if (first) { keys.set(kind, ks); if (ids) idSets.set(kind, ids); deferred.set(kind, later); }
  return later.length > 0;
}
{
  const todo = new Map([...files].filter(([k]) => spec.has(k)));
  const done = new Set(), again = [];
  const targetsOf = (k) => new Set(spec.get(k).filter((c) => c.role === 'ref' && c.ref && c.ref !== '-').flatMap((c) => c.ref.split('|')).filter((t) => t !== k && todo.has(t)));
  while (todo.size) {
    let pick = [...todo.keys()].find((k) => [...targetsOf(k)].every((t) => done.has(t)));
    if (pick === undefined) pick = [...todo.keys()].sort((a, b) => targetsOf(a).size - targetsOf(b).size)[0]; // cycle: defer its refs
    // refs into the sheet itself are never ready in the first read: it is not in `done` yet
    if (scan(pick, todo.get(pick), done)) again.push(pick);
    done.add(pick); todo.delete(pick);
  }
  for (const k of again) scan(k, files.get(k), done);
}

const tMethod = Date.now() - t0;
// ---- data sheets: rows of the game's own tables, columns from data/fields
const fields = dataFields();
const dataFiles = listDataSheets();
const declaredTables = keys.get('data-tables') || new Set();
const seenTables = new Set();
const structKeys = keys.get('data-structs') || new Set();
// Reading 7,444 files is the slow part, so each file's result is kept in build/preflight_cache.json
// and reused while the file (size, mtime) and the content of data/fields.json and reflect/class-defaults.json are unchanged. --no-cache rereads.
const CACHE = path.join(ROOT, 'build', 'preflight_cache.json');
const stamp = (f) => { const x = fs.statSync(f); return `${x.size}:${x.mtimeMs}`; };
// (the class defaults fill omitted cells of object sheets, so they are part of the key too)
const defaultsFile = path.join(SHEETS, 'reflect', 'class-defaults.json');
const fieldsStamp = (fs.existsSync(path.join(SHEETS, 'data', 'fields.json')) ? require('crypto').createHash('sha1').update(fs.readFileSync(path.join(SHEETS, 'data', 'fields.json'))).digest('hex') : '') + (fs.existsSync(defaultsFile) ? '|' + require('crypto').createHash('sha1').update(fs.readFileSync(defaultsFile)).digest('hex') : '');
let cache = { fields: fieldsStamp, files: {} };
if (!process.argv.includes('--no-cache') && fs.existsSync(CACHE)) { try { const c = JSON.parse(fs.readFileSync(CACHE, 'utf8')); if (c.fields === fieldsStamp) cache = c; } catch (e) { /* rebuilt below */ } }
const nextCache = { fields: fieldsStamp, files: {} };
let reread = 0;
function checkDataFile(rel) {
  const d = readDataSheet(path.join(SHEETS, rel), fields);
  const ids = new Set();
  const res = { views: [], dup: [] };
  for (const v of d.views) {
    const o = { kind: v.sheet, rows: 0, finished: 0, cells: 0, unchecked: 0, gaps: {}, unfinished: [], extra: [...new Set(v.extra)] };
    const n = v.columns.length;
    for (const r of v.rows) {
      o.rows++;
      let ok = true;
      for (let i = 0; i < n; i++) {
        o.cells++;
        if (r[i] === null || r[i] === undefined || (d.raw && !dataChecked(r[i]))) { ok = false; o.unchecked++; o.gaps[v.columns[i]] = (o.gaps[v.columns[i]] || 0) + 1; }
      }
      if (ok) o.finished++; else if (o.unfinished.length < 20) o.unfinished.push(r[0]);
      if (ids.has(r[0])) res.dup.push(r[0]); else ids.add(r[0]);
    }
    res.views.push(o);
  }
  return res;
}
for (const rel of dataFiles) {
  seenTables.add(rel);
  if (!declaredTables.has(rel)) err('E-L3-ORPHAN', 'data-tables', 'data sheet file without a data-tables row (run node tools/extract.js)', rel);
  const key = stamp(path.join(SHEETS, rel));
  let res = cache.files[rel] && cache.files[rel].stamp === key ? cache.files[rel].res : null;
  if (!res) { reread++; try { res = checkDataFile(rel); } catch (e) { err('E-L0-PARSE', rel, e.message.slice(0, 160)); continue; } }
  nextCache.files[rel] = { stamp: key, res };
  for (const id of res.dup) err('E-L0-KEY', rel, 'duplicate id within the file', id, 'id');
  for (const o of res.views) {
    let st = stats.get(o.kind);
    if (!st) { st = newStat('data'); stats.set(o.kind, st); }
    st.files++; st.rows += o.rows; st.finished += o.finished; st.cells += o.cells; st.unchecked += o.unchecked;
    for (const [c, n] of Object.entries(o.gaps)) st.gaps[c] = (st.gaps[c] || 0) + n;
    for (const id of o.unfinished) if (st.unfinished.length < CAP) st.unfinished.push(`${rel}#${id}`);
    if (!structKeys.has(o.kind)) err('E-L0-NOSCHEMA', o.kind, `no data-structs row for this kind (${rel})`);
    for (const k of o.extra) err('E-L0-COLUMN', o.kind, `field not in data/fields (${rel})`, null, k);
  }
}
fs.mkdirSync(path.dirname(CACHE), { recursive: true });
if (reread || Object.keys(cache.files).length !== dataFiles.length) fs.writeFileSync(CACHE, JSON.stringify(nextCache));
for (const k of declaredTables) if (!seenTables.has(k)) err('E-L3-MISSING', 'data-tables', 'data-tables row without its file', k);

// ---- L3: coverage overlaps (full outer join on the row id) and generated files
const coverage = [];
for (const k of absentSheets) coverage.push({ spec: 'schema', impl: k, what: 'declared sheet exists', covered: null, unimplemented: null, orphan: null, note: 'no sheet file yet' });
function overlap(specKind, implKind, what) {
  const a = keys.get(specKind), b = idSets.get(implKind) || keys.get(implKind);
  if (!a || !b) { coverage.push({ spec: specKind, impl: implKind, what, covered: null, unimplemented: null, orphan: null, note: `${!a ? specKind : implKind} is missing` }); return; }
  let covered = 0, orphan = 0;
  const un = [];
  for (const k of a) { if (b.has(k)) covered++; else if (un.length < CAP) un.push(k); }
  for (const k of b) if (!a.has(k)) orphan++;
  coverage.push({ spec: specKind, impl: implKind, what, covered, unimplemented: a.size - covered, orphan, first_unimplemented: un });
}
overlap('code', 'decomp', 'functions with a decompilation row');
overlap('code', 'verify', 'functions with a verification row');
overlap('code', 'progress', 'functions with a progress row (node tools/link.js)');
// every generated file a row names must exist
{
  const have = new Map();
  const exists = (p) => {
    const dir = path.dirname(p);
    if (!have.has(dir)) have.set(dir, fs.existsSync(path.join(ROOT, dir)) ? new Set(fs.readdirSync(path.join(ROOT, dir))) : new Set());
    return have.get(dir).has(path.basename(p));
  };
  for (const kind of ['classes', 'structs', 'data-structs']) {
    let named = 0, absent = 0;
    for (const { file, head } of files.get(kind) || []) {
      if (!('header' in head.col)) continue;
      eachRow(file, (r) => { const h = r[head.col.header]; if (!h || h === '-') return; named++; if (!exists(h)) absent++; }); // generated output is disposable (D6): a missing file is a gap that node tools/gen.js closes, not an error
    }
    coverage.push({ spec: kind, impl: 'generated files', what: 'rows whose header exists on disk', covered: named - absent, unimplemented: absent, orphan: 0 });
  }
}

// ---- method check card (Appendix C): the checks that are not per cell
const { DEFS, schemaRows, indexRows } = require('./lib/sheet_defs');
const { EMITTER_VERSION, BANNER_MARK, GEN_ROOT } = require('./lib/genout');
const smallRows = (kind) => { const out = []; for (const { file } of files.get(kind) || []) eachRow(file, (r, h) => { out.push(Object.fromEntries(h.columns.map((c, i) => [c, r[i]]))); }); return out; };
// L5: the schema and the index on disk are the declared ones
{
  const same = (kind, want) => JSON.stringify(smallRows(kind).map((o) => Object.values(o))) === JSON.stringify(want);
  if (!same('schema', schemaRows())) err('E-L5-SCHEMA', 'schema', 'sheets/schema.json differs from tools/lib/sheet_defs.js (run node tools/extract.js)');
  if (!same('sheets', indexRows())) err('E-L5-SCHEMA', 'sheets', 'sheets/index.json differs from tools/lib/sheet_defs.js (run node tools/extract.js)');
}
// L0: bulk exports are not truncated (the method's --expect)
for (const [kind, d] of Object.entries(DEFS)) if (d.minRows && stats.has(kind) && stats.get(kind).rows < d.minRows) err('E-L0-SHORT', kind, `${stats.get(kind).rows} rows, fewer than the ${d.minRows} a complete export has`);
// L0: every hand-written tool is a kernel row, and every kernel row is a file
{
  const listed = keys.get('kernel') || new Set();
  const onDisk = new Set();
  for (const [dir, re] of [['tools', /\.(js|ps1)$/], ['tools/lib', /\.js$/], ['tools/ghidra', /\.(java|py)$/]]) if (fs.existsSync(path.join(ROOT, dir))) for (const x of fs.readdirSync(path.join(ROOT, dir))) if (re.test(x)) onDisk.add(`${dir}/${x}`);
  for (const x of onDisk) if (!listed.has(x)) err('E-L0-KERNEL', 'kernel', 'hand-written file without a kernel row (run node tools/extract.js)', x);
  for (const x of listed) if (!onDisk.has(x)) err('E-L0-KERNEL', 'kernel', 'kernel row without its file (run node tools/extract.js)', x);
}
// L0: generated code lives in build/gen only, and the doctrine sheet names the emitter that wrote it
{
  if (fs.existsSync(path.join(ROOT, 'src', 'include', 'generated'))) err('E-L0-GENERATED', 'targets', `src/include/generated exists; generated output belongs in ${GEN_ROOT}`);
  const inc = path.join(ROOT, 'src', 'include');
  if (fs.existsSync(inc)) for (const x of fs.readdirSync(inc, { withFileTypes: true })) if (x.isFile() && fs.readFileSync(path.join(inc, x.name), 'latin1').startsWith(BANNER_MARK)) err('E-L0-GENERATED', 'targets', `generated file in the source tree: src/include/${x.name}`);
  const d = smallRows('doctrine').find((r) => r.key === 'emitter_version');
  if (files.has('doctrine') && (!d || d.value !== EMITTER_VERSION)) err('E-L0-EMITTER', 'doctrine', `doctrine emitter_version is ${d ? d.value : 'absent'}, tools/lib/genout.js says ${EMITTER_VERSION}`);
}
// L2: no cycles inside a sheet's own hierarchy
for (const [kind, edges] of selfEdges) {
  const ks = keys.get(kind), state = new Map(); // 1 = on the path, 2 = done
  for (const start of edges.keys()) {
    if (state.has(start)) continue;
    const stack = [[start, 0]];
    state.set(start, 1);
    while (stack.length) {
      const top = stack[stack.length - 1], to = edges.get(top[0]) || [];
      if (top[1] >= to.length) { state.set(top[0], 2); stack.pop(); continue; }
      const next = to[top[1]++];
      if (!ks.has(normKey(next))) continue;
      const st = state.get(next);
      if (st === 1) { err('E-L2-CYCLE', kind, `cycle: ${stack.slice(stack.findIndex((x) => x[0] === next)).map((x) => x[0]).concat(next).join(' -> ').slice(0, 200)}`, next); continue; }
      if (st === 2) continue;
      state.set(next, 1); stack.push([next, 0]);
    }
  }
}
// L2: evidence sheets should carry an address and a confidence (Mode B: ref_addr, ref_conf, evidence)
{
  const noAddr = [], noConf = [];
  for (const [kind, cols] of spec) {
    const d = DEFS[kind];
    if (!d || !['reflect', 'exe', 'native'].includes(d.group)) continue;
    if (!cols.some((c) => c.type === 'addr')) noAddr.push(kind);
    if (!cols.some((c) => c.column === 'ref_conf')) noConf.push(kind);
  }
  if (noAddr.length) warnings.push({ code: 'W-L2-PROVENANCE', msg: `evidence sheets without an address column: ${noAddr.join(', ')}` });
  if (noConf.length) warnings.push({ code: 'W-L2-PROVENANCE', msg: `evidence sheets without a ref_conf column: ${noConf.join(', ')}` });
}
// L0: row counts against the previous run
{
  const prev = path.join(ROOT, 'build', 'preflight.json');
  if (fs.existsSync(prev)) {
    try {
      const old = JSON.parse(fs.readFileSync(prev, 'utf8')).sheets || {};
      const changed = [];
      for (const [k, st] of stats) if (spec.has(k) && old[k] && old[k].rows !== st.rows) changed.push(`${k} ${old[k].rows} -> ${st.rows}`);
      for (const k of Object.keys(old)) if (!stats.has(k)) changed.push(`${k} ${old[k].rows} -> gone`);
      if (changed.length) warnings.push({ code: 'W-L0-ROWS', msg: `row counts changed since the last run: ${changed.slice(0, 12).join('; ')}${changed.length > 12 ? `; +${changed.length - 12} more` : ''}` });
    } catch (e) { /* no usable previous report */ }
  }
}
// L4: assets on disk (only with --assets: one stat per file)
let assetLine = 'not checked in this run (--assets); hashes are checked by tools/pak_verify.js';
if (process.argv.includes('--assets')) {
  let n = 0, gone = 0;
  const first = [];
  for (const { file } of files.get('asset-files') || []) eachRow(file, (r) => { n++; if (!fs.existsSync(path.join(ROOT, 'assets', r[0]))) { gone++; if (first.length < CAP) first.push(r[0]); } });
  coverage.push({ spec: 'asset-files', impl: 'files under assets/', what: 'rows whose file exists', covered: n - gone, unimplemented: gone, orphan: 0, first_unimplemented: first });
  assetLine = `${n} rows, ${gone} files missing under assets/`;
}
// L6: token ledger
const ledger = (() => {
  const rows = smallRows('tokens');
  const known = rows.filter((r) => typeof r.total_tokens === 'number');
  return { stages: rows.length, recorded: known.length, total_tokens: known.reduce((n, r) => n + r.total_tokens, 0), unrecorded: rows.filter((r) => typeof r.total_tokens !== 'number').map((r) => r.stage) };
})();

// ---- report
const tot = { sheets: stats.size, files: 0, rows: 0, finished: 0, cells: 0, unchecked: 0, dangling: 0 };
for (const st of stats.values()) { tot.files += st.files; tot.rows += st.rows; tot.finished += st.finished; tot.cells += st.cells; tot.unchecked += st.unchecked; tot.dangling += st.dangling_total; }
const nErrors = Object.values(errorCount).reduce((a, b) => a + b, 0);
const covGaps = coverage.reduce((n, c) => n + (c.unimplemented || 0), 0) + coverage.filter((c) => c.covered === null).length;
const blast = [...missing].sort((a, b) => b[1] - a[1]).slice(0, CAP).map(([k, n]) => ({ missing: k, inbound: n }));
const status = nErrors ? 'FAILED' : tot.unchecked || tot.dangling || covGaps ? 'UNFINISHED' : 'PASSED';
const report = {
  generated_by: 'tools/preflight.js', status, totals: { ...tot, errors: nErrors, coverage_gaps: covGaps, refs_followed: refCells },
  error_counts: errorCount, errors,
  sheets: Object.fromEntries([...stats].sort((a, b) => (a[0] < b[0] ? -1 : 1)).map(([k, s]) => [k, { group: s.group, files: s.files, rows: s.rows, finished: s.finished, unfinished_rows: s.rows - s.finished, unchecked_cells: s.unchecked,
    unchecked_by_column: s.gaps, dangling_refs: s.dangling_total, dangling_by_column: s.dangling, dangling_examples: s.dangling_examples || [], first_unfinished: s.unfinished }])),
  missing_ref_targets: blast, coverage, warnings, token_ledger: ledger, assets: assetLine,
};
fs.mkdirSync(path.join(ROOT, 'build'), { recursive: true });
fs.writeFileSync(path.join(ROOT, 'build', 'preflight.json'), JSON.stringify(report, null, 1));

if (JSON_OUT) { const { sheets, errors: e, ...short } = report; console.log(JSON.stringify(short)); process.exit(nErrors ? 1 : status === 'PASSED' ? 0 : 2); }
const N = (x) => Number(x).toLocaleString('en-US');
const pad = (v, n) => String(v).padStart(n);
const lines = [];
lines.push(`PREFLIGHT  sheets/  ${N(tot.sheets)} sheets in ${N(tot.files)} files, ${N(tot.rows)} rows, ${N(tot.cells)} cells`);
const l0 = Object.entries(errorCount).filter(([c]) => c.startsWith('E-L0')), l1 = errorCount['E-L1-TYPE'] || 0, l2 = errorCount['E-L2-CYCLE'] || 0;
lines.push(`  L0 structure .... ${l0.length ? l0.map(([c, n]) => `${n} ${c}`).join(', ') : 'ok'}`);
lines.push(`  L1 checkmarks ... ${N(tot.cells - tot.unchecked)}/${N(tot.cells)} cells checked, ${N(tot.finished)}/${N(tot.rows)} rows finished${l1 ? `, ${l1} E-L1-TYPE` : ''}`);
lines.push(`  L2 refs ......... ${N(refCells)} followed, ${N(tot.dangling)} dangling, ${l2 ? l2 + ' E-L2-CYCLE' : 'no cycles'}`);
lines.push(`  L3 coverage ..... ${coverage.map((c) => (c.covered === null ? `${c.spec === 'schema' ? '' : c.spec + ' x '}${c.impl}: ${c.note}` : `${c.spec} x ${c.impl}: ${N(c.unimplemented)} missing${c.orphan ? `, ${N(c.orphan)} orphan` : ''}`)).join('; ')}`);
lines.push(`  L4 assets ....... ${assetLine}`);
lines.push(`  L5 evidence ..... schema and index ${errorCount['E-L5-SCHEMA'] ? 'STALE' : 'match tools/lib/sheet_defs.js'}`);
lines.push(`  L6 budget ....... ${N(ledger.total_tokens)} tokens recorded in ${ledger.recorded} of ${ledger.stages} stages${ledger.unrecorded.length ? `; not recorded: ${ledger.unrecorded.join(', ')}` : ''}`);
for (const w of warnings) lines.push(`      ${w.code.padEnd(14)} ${SUMMARY && w.msg.length > 230 ? w.msg.slice(0, 227) + '...' : w.msg}`);
for (const e of errors.slice(0, SUMMARY ? 8 : 40)) lines.push(`      ${e.code.padEnd(14)} ${e.sheet}${e.row !== null ? ' row=' + e.row : ''}${e.col ? ' col=' + e.col : ''}  ${e.msg}`);
if (errors.length > (SUMMARY ? 8 : 40)) lines.push(`      ... ${nErrors - (SUMMARY ? 8 : 40)} more in build/preflight.json`);
lines.push('');
const row = (name, s) => `  ${s.rows === s.finished && !s.dangling_total ? 'ok' : ' x'} ${name.padEnd(SUMMARY ? 22 : 44)} ${pad(N(s.files), 6)} ${pad(N(s.rows), 9)} ${pad(N(s.finished), 9)} ${pad(N(s.unchecked), 9)} ${pad(N(s.dangling_total), 9)}`;
const detail = (s) => [Object.entries(s.gaps).map(([c, n]) => `${c}:${N(n)}`).join(' '), Object.entries(s.dangling).map(([c, n]) => `${c}->?:${N(n)}`).join(' ')].filter(Boolean).join('  ');
lines.push(`     ${'sheet'.padEnd(SUMMARY ? 22 : 44)} ${pad('files', 6)} ${pad('rows', 9)} ${pad('finished', 9)} ${pad('unchecked', 9)} ${pad('dangling', 9)}  unchecked cells by column / dangling refs by column`);
const sorted = [...stats].sort((a, b) => (a[1].group + '/' + a[0] < b[1].group + '/' + b[0] ? -1 : 1));
const fold = (name, list) => { const s = newStat('data'); for (const x of list) { s.files += x.files; s.rows += x.rows; s.finished += x.finished; s.unchecked += x.unchecked; s.dangling_total += x.dangling_total; } return row(name, s); };
if (SUMMARY) {
  for (const [k, s] of sorted) if (s.group !== 'data' || spec.has(k)) lines.push(row(`${s.group}/${k}`, s) + '  ' + detail(s));
  const rows = sorted.filter(([k, s]) => s.group === 'data' && k.startsWith('row/')).map((x) => x[1]), objs = sorted.filter(([k, s]) => s.group === 'data' && k.startsWith('object/')).map((x) => x[1]);
  lines.push(fold(`data/row/* (${rows.length} kinds)`, rows));
  lines.push(fold(`data/object/* (${objs.length} kinds)`, objs) + '  ' + sorted.filter(([k, s]) => k.startsWith('object/') && s.unchecked).sort((a, b) => b[1].unchecked - a[1].unchecked).slice(0, 4).map(([k, s]) => `${k.slice(7)}:${N(s.unchecked)}`).join(' ') + ' ...');
} else {
  for (const [k, s] of sorted) lines.push(row(`${s.group === 'data' && !spec.has(k) ? '' : s.group + '/'}${k}`, s) + '  ' + detail(s));
}
lines.push('');
if (blast.length) { lines.push('most-referenced missing rows (target sheet <- key, inbound refs):'); for (const b of blast.slice(0, SUMMARY ? 6 : 15)) lines.push(`  ${pad(N(b.inbound), 8)}  ${b.missing}`); lines.push(''); }
lines.push(`${status}  ${N(nErrors)} errors, ${N(tot.rows - tot.finished)} unfinished rows (${N(tot.unchecked)} unchecked cells), ${N(tot.dangling)} dangling refs, ${N(covGaps)} coverage gaps  [${((Date.now() - t0) / 1000).toFixed(1)} s, ${reread} data files read; detail in build/preflight.json]`);
console.log(lines.join('\n'));
process.exit(nErrors ? 1 : status === 'PASSED' ? 0 : 2);
