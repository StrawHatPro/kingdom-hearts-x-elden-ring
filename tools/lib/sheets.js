// Sheet I/O for the Spreadsheet Method (see sheets/README.md).
//
// A method sheet is { sheet, group, columns:[...], rows:[[...], ...] } with the header on the
// first line and one row per line after it, so a sheet of any size can be read and written as
// a stream. A cell is "checked" when it holds a value; null is the empty, unchecked cell.
// "" is a real value; "-" means "deliberately none".
//
// The game's own data (DataTables and data-asset objects) stays in the per-asset files under
// sheets/entities and sheets/systems, because those compile back to the cooked bytes
// (tools/sheets_check.js). readDataSheet() presents each of them as method sheets: kind
// "row/<RowStruct>" or "object/<Class>", columns = the fields of that struct (sheet data-fields).
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..', '..');
const SHEETS = path.join(ROOT, 'sheets');
const DATA_DIRS = ['entities', 'systems']; // per-asset data sheets
const SKIP_DIRS = new Set([...DATA_DIRS, 'schemas']);

const checked = (v) => v !== null && v !== undefined;
// Addresses are written 0x1400010A0 by some tools and 0x1400010a0 by others; keys and refs
// are compared in one spelling.
const normKey = (v) => (typeof v === 'string' && v.length > 2 && v.charCodeAt(0) === 48 && (v[1] === 'x' || v[1] === 'X') && /^0[xX][0-9a-fA-F]+$/.test(v) ? '0x' + v.slice(2).replace(/^0+(?=.)/, '').toLowerCase() : v);

// Calls fn(line) for every line of the file; fn returning false stops the read.
function eachLine(file, fn) {
  const fd = fs.openSync(file, 'r');
  const buf = Buffer.allocUnsafe(1 << 22);
  let rest = null;
  try {
    for (let n; (n = fs.readSync(fd, buf, 0, buf.length, null)) > 0;) {
      let from = 0;
      for (let nl; (nl = buf.indexOf(10, from)) >= 0 && nl < n; from = nl + 1) {
        let line = buf.toString('utf8', from, nl);
        if (rest) { line = Buffer.concat([rest, buf.subarray(from, nl)]).toString('utf8'); rest = null; }
        if (fn(line) === false) return;
      }
      if (from < n) rest = rest ? Buffer.concat([rest, buf.subarray(from, n)]) : Buffer.from(buf.subarray(from, n));
    }
    if (rest && rest.length) fn(rest.toString('utf8'));
  } finally { fs.closeSync(fd); }
}

const withCol = (s, file) => { s.file = file; s.col = Object.fromEntries(s.columns.map((c, i) => [c, i])); return s; };

function readSheet(file) {
  return withCol(JSON.parse(fs.readFileSync(file, 'utf8').replace(/^﻿/, '')), file);
}

// Streams the rows of a method sheet: fn(row, header). Returns the header ({sheet, group,
// columns, col, file}). A file that is not in one-row-per-line form is parsed whole instead.
function eachRow(file, fn) {
  let head = null, whole = false;
  eachLine(file, (raw) => {
    const line = raw.replace(/\r$/, '');
    if (!head) {
      const first = line.replace(/^﻿/, '');
      if (!/"rows":\[\s*$/.test(first)) { whole = true; return false; }
      head = withCol(JSON.parse(first + ']}'), file);
      delete head.rows;
      return fn ? undefined : false;
    }
    if (line.charCodeAt(0) !== 91) return undefined; // "]}" and blank lines
    return fn(JSON.parse(line.charCodeAt(line.length - 1) === 44 ? line.slice(0, -1) : line), head);
  });
  if (whole) {
    const s = readSheet(file);
    const rows = s.rows;
    delete s.rows;
    if (fn) for (const r of rows) if (fn(r, s) === false) break;
    return s;
  }
  if (!head) throw new Error(`${file}: empty sheet file`);
  return head;
}
const readHeader = (file) => eachRow(file, null);

// rows: an array or any iterable of row arrays.
function writeSheet(file, sheet, group, columns, rows) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const tmp = file + '.tmp';
  const fd = fs.openSync(tmp, 'w');
  let chunk = `{"sheet":${JSON.stringify(sheet)},"group":${JSON.stringify(group)},"columns":${JSON.stringify(columns)},"rows":[\n`;
  let first = true, count = 0;
  for (const r of rows) {
    if (r.length !== columns.length) throw new Error(`${sheet}: row ${JSON.stringify(r[0])} has ${r.length} cells for ${columns.length} columns`);
    chunk += (first ? '' : ',\n') + JSON.stringify(r);
    first = false; count++;
    if (chunk.length > 1 << 22) { fs.writeSync(fd, chunk); chunk = ''; }
  }
  fs.writeSync(fd, chunk + (first ? '' : '\n') + ']}\n');
  fs.closeSync(fd);
  fs.renameSync(tmp, file);
  return count;
}

function walk(dir, rel, skip, out) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    if (e.isDirectory()) { if (!(skip && skip.has(e.name))) walk(path.join(dir, e.name), rel + e.name + '/', null, out); }
    else if (e.name.endsWith('.json')) out.push(rel + e.name);
  }
  return out;
}
// Every method sheet under sheets/, as paths relative to it ("reflect/classes.json").
const listSheets = () => walk(SHEETS, '', SKIP_DIRS, []).sort();
// Every per-asset data sheet ("entities/AI/.../X.json").
const listDataSheets = () => DATA_DIRS.flatMap((d) => (fs.existsSync(path.join(SHEETS, d)) ? walk(path.join(SHEETS, d), d + '/', null, []) : [])).sort();

// Method sheets by kind: Map kind -> [absolute file]. Reads only the first line of each file.
function sheetsByKind() {
  const m = new Map();
  for (const f of listSheets()) {
    const file = path.join(SHEETS, f);
    const h = readHeader(file);
    (m.get(h.sheet) || m.set(h.sheet, []).get(h.sheet)).push(file);
  }
  return m;
}

// All rows of one kind as objects keyed by column name (for tools that need a whole sheet).
function rowsOf(kind, byKind = sheetsByKind()) {
  const out = [];
  for (const file of byKind.get(kind) || []) eachRow(file, (r, h) => { out.push(Object.fromEntries(h.columns.map((c, i) => [c, r[i]]))); });
  return out;
}

// ---- data sheets (DataTables and data-asset objects)
// A value the codec could not interpret is kept as { raw: "<hex>" }; such a cell is unchecked.
const hasRaw = (v) => !!v && typeof v === 'object' && (v.raw !== undefined || Object.values(v).some(hasRaw));
const dataChecked = (v) => checked(v) && !hasRaw(v);
const DATA_KEY_COLUMNS = ['id', 'name']; // id = unique within its file; name = the stored row/object name

// Map "row/<Struct>" | "object/<Class>" -> [field, ...] in serialized order, from sheets/data/fields.json.
function dataFields() {
  const m = new Map();
  const file = path.join(SHEETS, 'data', 'fields.json');
  if (!fs.existsSync(file)) return m;
  eachRow(file, (r, h) => { (m.get(r[h.col.struct]) || m.set(r[h.col.struct], []).get(r[h.col.struct])).push(r[h.col.name]); });
  return m;
}

// Class defaults (sheets/reflect/class-defaults.json): "Class.field" -> value. A cooked object leaves out a property
// whose value equals its archetype's; where the archetype is the class default object and the default is recovered
// from the exe, the cell holds that value. The per-asset files are not changed (they compile back to the cooked bytes).
let classDefaultsCache = null;
function classDefaults() {
  if (classDefaultsCache) return classDefaultsCache;
  classDefaultsCache = new Map();
  const file = path.join(SHEETS, 'reflect', 'class-defaults.json');
  if (fs.existsSync(file)) eachRow(file, (r, h) => { if (checked(r[h.col.value])) classDefaultsCache.set(r[h.col.id], r[h.col.value]); });
  return classDefaultsCache;
}
// true when the export's archetype is the import named Default__<Class>
function templateIsCdo(pk, row) {
  if (!pk || !pk.exports || !pk.imports || !pk.names) return false;
  const ex = pk.exports[row._export - 1];
  const imp = ex && ex.templateIndex < 0 ? pk.imports[-ex.templateIndex - 1] : null;
  const n = imp && imp.objectName && pk.names[imp.objectName.index];
  return !!n && n.s === 'Default__' + row._class;
}

// One per-asset file -> [{ sheet: kind, group: 'data', file, columns, rows, extra }]. A DataTable
// gives one view; an object sheet gives one per class of object. `extra` lists row keys that are
// not fields of the struct (there should be none).
function readDataSheet(file, fields) {
  const text = fs.readFileSync(file, 'utf8');
  const s = JSON.parse(text);
  const objects = s.kind === 'objects';
  const views = new Map();
  const defaults = objects ? classDefaults() : null;
  let defaulted = 0;
  for (const row of objects ? s.objects : s.rows) {
    const kind = objects ? `object/${row._class}` : `row/${s.row_struct}`;
    let v = views.get(kind);
    if (!v) {
      const f = fields.get(kind) || [];
      v = withCol({ sheet: kind, group: 'data', columns: [...DATA_KEY_COLUMNS, ...f], rows: [], extra: [], known: new Set(f), declared: fields.has(kind) }, file);
      views.set(kind, v);
    }
    const f = v.columns;
    const out = new Array(f.length);
    out[0] = row.id;
    out[1] = objects ? row._object_name : row._row_name;
    let cdo = null; // decided once per row, only when a cell is omitted
    for (let i = 2; i < f.length; i++) {
      let v = row[f[i]] === undefined ? null : row[f[i]];
      if (v === null && defaults && defaults.size) {
        const d = defaults.get(row._class + '.' + f[i]);
        if (d !== undefined) { if (cdo === null) cdo = templateIsCdo(s._package, row); if (cdo) { v = d; defaulted++; } }
      }
      out[i] = v;
    }
    for (const k in row) if (k !== 'id' && k.charCodeAt(0) !== 95 && !v.known.has(k)) v.extra.push(k);
    v.rows.push(out);
  }
  return { name: s.sheet, raw: text.includes('"raw"'), objects, row_struct: s.row_struct, asset_class: s.asset_class, source: s._source, defaulted, views: [...views.values()] };
}

module.exports = { ROOT, SHEETS, DATA_DIRS, checked, normKey, eachLine, readSheet, readHeader, eachRow, writeSheet, listSheets, listDataSheets, sheetsByKind, rowsOf, dataFields, readDataSheet, classDefaults, dataChecked, hasRaw, DATA_KEY_COLUMNS };
