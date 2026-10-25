// Phase 5: derive JSON sheets from every UDataTable in the paks (spec section 5), and from the
// game's data-asset packages whose exports are all plain tagged-property objects.
//   sheets/entities|systems/<path>.json   one sheet per DataTable asset or data-asset package
//   sheets/schemas/<RowStruct>.schema.json  JSON Schema (draft 2020-12), one per row struct
//   sheets/schemas/<Class>.object.schema.json  one per class of object in the object sheets
//   audit/sheets.json                      what was generated, with hashes
// Deterministic: the same paks always give the same files. tools/sheets_check.js validates the
// sheets and compiles them back to the original bytes.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { ROOT, logRun, writeJson } = require('./lib/log');
const pak = require('./lib/pak');
const ua = require('./lib/uasset');
const dt = require('./lib/datatable');
const sh = require('./lib/sheet');

const VERSION = '1.1.0';
// Object sheets: packages whose main asset's class comes from one of these script packages, or
// is one of these engine classes. A package is only turned into a sheet when every export in it
// is a plain tagged-property object; the others are listed in audit/sheets.json with the reason.
const OBJECT_CLASS_PACKAGES = new Set(['/Script/TresGame']);
const OBJECT_CLASSES = new Set(['CurveFloat', 'CurveVector', 'CurveLinearColor']);
// Left out although it parses: lists of texture paths for streaming, about 160 MB as sheets.
const OBJECT_CLASSES_EXCLUDED = new Set(['TresTexturePump']);
const SHEETS = path.join(ROOT, 'sheets');
const sha256 = (b) => crypto.createHash('sha256').update(b).digest('hex');
const CONTENT = 'KINGDOM HEARTS III/Content/';
// Gameplay entities vs. menus/progress/system tables.
const groupOf = (rel) => (/^(UI|System)\//.test(rel) ? 'systems' : 'entities');

for (const d of ['entities', 'systems', 'schemas']) fs.rmSync(path.join(SHEETS, d), { recursive: true, force: true });

const files = pak.mountAll();
const fds = new Map();
const read = (p) => {
  const f = files.get(p);
  if (!f) return null;
  if (!fds.has(f.pak)) fds.set(f.pak, fs.openSync(f.pak, 'r'));
  return { data: pak.extract(fds.get(f.pak), f.entry), archive: path.basename(f.pak) };
};

// pass 1: parse every DataTable; group by row struct so each struct gets one schema
const byStruct = new Map();
const stats = { packages_scanned: 0, datatables: 0, parse_errors: [], rows: 0 };
const objectPackages = [];   // { parsed, source }
const objectsSkipped = [];   // { path, asset_class, reason }
// Class of the package's main asset and the script package that defines it.
function assetClass(pkg) {
  const main = pkg.exports.find((e) => e.isAsset) || pkg.exports.find((e) => e.outerIndex === 0);
  if (!main || main.classIndex >= 0) return null;
  const imp = pkg.imports[-main.classIndex - 1];
  return { name: ua.nameOf(pkg, imp.objectName), from: imp.outerIndex < 0 ? ua.nameOf(pkg, pkg.imports[-imp.outerIndex - 1].objectName) : '' };
}
for (const p of [...files.keys()].sort()) {
  if (!p.endsWith('.uasset')) continue;
  stats.packages_scanned++;
  const a = read(p);
  let hasTable = false, pkg, cls;
  try {
    pkg = ua.parseHeader(a.data);
    hasTable = pkg.exports.some((e) => ua.objectName(pkg, e.classIndex) === 'DataTable');
    if (!hasTable) cls = assetClass(pkg);
  } catch (e) { continue; } // not a package this reader understands; counted by uasset_roundtrip
  const uexpPath = p.replace(/\.uasset$/, '.uexp');
  if (!hasTable) {
    if (!cls || !(OBJECT_CLASS_PACKAGES.has(cls.from) || OBJECT_CLASSES.has(cls.name))) continue;
    if (OBJECT_CLASSES_EXCLUDED.has(cls.name)) { stats.object_packages_excluded = (stats.object_packages_excluded || 0) + 1; continue; }
    const x = read(uexpPath);
    const parsed = x ? sh.parseObjects(pkg, x.data) : { error: 'no .uexp' };
    if (parsed.error) { objectsSkipped.push({ path: p, asset_class: cls.name, reason: parsed.error }); continue; }
    const rel = p.startsWith(CONTENT) ? p.slice(CONTENT.length).replace(/\.uasset$/, '') : p.replace(/\.uasset$/, '');
    objectPackages.push({ parsed, source: { sheet: rel, assetClass: cls.name, uasset: p, uexp: uexpPath, uassetSha: sha256(a.data), uexpSha: sha256(x.data), archive: a.archive } });
    continue;
  }
  const x = read(uexpPath);
  if (!x) { stats.parse_errors.push({ path: p, error: 'no .uexp' }); continue; }
  try {
    const parsed = dt.parse(a.data, x.data);
    const rel = p.startsWith(CONTENT) ? p.slice(CONTENT.length).replace(/\.uasset$/, '') : p.replace(/\.uasset$/, '');
    const key = parsed.table.rowStruct || '(none)';
    if (!byStruct.has(key)) byStruct.set(key, []);
    byStruct.get(key).push({ parsed, source: { sheet: rel, uasset: p, uexp: uexpPath, uassetSha: sha256(a.data), uexpSha: sha256(x.data), archive: a.archive } });
    stats.datatables++;
    stats.rows += parsed.table.rows.length;
  } catch (e) { stats.parse_errors.push({ path: p, error: String(e.message) }); }
  if (stats.packages_scanned % 20000 === 0) process.stdout.write(`\r${stats.packages_scanned} packages`);
}
for (const fd of fds.values()) fs.closeSync(fd);

// pass 2: one schema per struct, then the sheets
const JSON_TYPE = {
  IntProperty: { type: 'integer' }, UInt32Property: { type: 'integer', minimum: 0 }, Int8Property: { type: 'integer' }, Int16Property: { type: 'integer' },
  UInt16Property: { type: 'integer', minimum: 0 }, BoolProperty: { type: ['boolean', 'object'] }, FloatProperty: { type: ['number', 'object'] },
  NameProperty: { type: ['string', 'object'] }, EnumProperty: { type: ['string', 'object'] }, ByteProperty: { type: ['integer', 'string', 'object'] },
  StrProperty: { type: ['string', 'object'] }, AssetObjectProperty: { type: ['string', 'object'] }, Int64Property: { type: 'string' }, UInt64Property: { type: 'string' },
};
const countRaw = (v) => (!v || typeof v !== 'object' ? 0 : v.raw !== undefined ? 1 : Object.values(v).reduce((n, x) => n + countRaw(x), 0));
const generated = [];
let uninterpreted = 0;
// Schema properties for the fields collected in `fields` (in `order`); appends to `required`.
function fieldProperties(fields, order, totalRows, properties, required, what) {
  for (const key of order) {
    const f = fields.get(key);
    const base = JSON_TYPE[f.template.type] || { type: 'object' };
    const nullable = f.count < totalRows;
    const type = [].concat(base.type);
    properties[key] = {
      ...base, type: nullable ? [...type, 'null'] : type.length === 1 ? type[0] : type,
      description: `${f.template.type}${f.template.structName ? ' ' + f.template.structName : ''}${f.template.enumName && f.template.enumName !== 'None' ? ' ' + JSON.stringify(f.template.enumName) : ''}` +
        (nullable ? `. null = the property is not serialized in this ${what} (the cooked data omits it).` : ''),
      'x-ue-name': f.name, 'x-ue': f.template,
      'x-confidence': 'confirmed', 'x-evidence': 'property name and type are stored in the cooked package\'s property tag',
    };
    if (!nullable) required.push(key);
  }
}
const SOURCE_PROP = { type: 'object', required: ['file', 'offset', 'size'], properties: { file: { type: 'string' }, offset: { type: 'string', pattern: '^0x[0-9A-F]+$' }, size: { type: 'integer' } } };
const KEYS_PROP = { type: 'array', items: { type: 'string' }, description: 'serialized property order when it differs from x-field-order' };
const TAGS_PROP = { type: 'object', description: 'property tags that differ from the schema template for this row' };
// Makes absent fields explicit, drops _keys when the schema order reproduces it, counts raw values.
function finishRow(row, order) {
  const present = order.filter((k) => row[k] !== undefined);
  if (JSON.stringify(present) === JSON.stringify(row._keys)) delete row._keys;
  for (const k of order) if (row[k] === undefined) row[k] = null;
  let raw = 0;
  for (const k of order) raw += countRaw(row[k]);
  return raw;
}
const sheetFile = (s) => `sheets/${groupOf(s.sheet)}/${s.sheet}.json`;
function writeSheet(rel, head, listKey, rows) {
  const out = path.join(ROOT, rel);
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, JSON.stringify(head).slice(0, -1) + `,"${listKey}":[\n` + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
  return sha256(fs.readFileSync(out));
}

for (const [struct, tables] of [...byStruct.entries()].sort()) {
  const fields = new Map(), order = [];
  const sheets = tables.map((t) => sh.toSheet(t.parsed, t.source, fields, order));
  const totalRows = sheets.reduce((n, s) => n + s.rows.length, 0);
  const properties = {
    id: { type: 'string', pattern: '^[a-z0-9_]+$', description: 'snake_case form of the row name, unique within the sheet' },
    _source: SOURCE_PROP,
    _row_name: { type: ['string', 'object'], description: 'row name exactly as stored (FName)' },
    _keys: KEYS_PROP, _tags: TAGS_PROP,
  };
  const required = ['id', '_source', '_row_name'];
  fieldProperties(fields, order, totalRows, properties, required, 'row');
  const schema = {
    $schema: 'https://json-schema.org/draft/2020-12/schema', $id: `kh3/${struct}`, title: struct,
    description: `Row of a UDataTable whose RowStruct is ${struct}. Generated by tools/sheets_build.js.`,
    type: 'object', required, properties, additionalProperties: false, 'x-field-order': order,
  };
  const schemaRel = `sheets/schemas/${struct.replace(/[^A-Za-z0-9_]/g, '_')}.schema.json`;
  writeJson(path.join(ROOT, schemaRel), schema);
  for (const s of sheets) {
    for (const row of s.rows) uninterpreted += finishRow(row, order);
    s.schema = schemaRel;
    const rel = sheetFile(s);
    const { rows, ...head } = s;
    generated.push({ sheet: rel, schema: schemaRel, row_struct: struct, rows: rows.length, sha256: writeSheet(rel, head, 'rows', rows), source_uexp_sha256: s._source.uexp_sha256 });
  }
}
const tableSheets = generated.length, tableUninterpreted = uninterpreted;

// pass 3: object sheets. One schema per class of object, shared by every sheet.
const classes = new Map();
objectPackages.sort((a, b) => (a.source.sheet < b.source.sheet ? -1 : a.source.sheet > b.source.sheet ? 1 : 0));
const objectSheets = objectPackages.map((o) => sh.toObjectSheet(o.parsed, o.source, classes));
const classSchema = (c) => `sheets/schemas/${c.replace(/[^A-Za-z0-9_]/g, '_')}.object.schema.json`;
for (const [cls, c] of [...classes.entries()].sort()) {
  const properties = {
    id: { type: 'string', pattern: '^[a-z0-9_]+$', description: 'snake_case form of the object name, unique within the sheet' },
    _source: SOURCE_PROP,
    _object_name: { type: ['string', 'object'], description: 'object name exactly as stored in the export table (FName)' },
    _class: { const: cls },
    _export: { type: 'integer', minimum: 1, description: '1-based index of the object in the package export table' },
    _keys: KEYS_PROP, _tags: TAGS_PROP,
  };
  const required = ['id', '_source', '_object_name', '_class', '_export'];
  fieldProperties(c.fields, c.order, c.count, properties, required, 'object');
  writeJson(path.join(ROOT, classSchema(cls)), {
    $schema: 'https://json-schema.org/draft/2020-12/schema', $id: `kh3/object/${cls}`, title: cls,
    description: `Object of class ${cls}, serialized as tagged properties. Generated by tools/sheets_build.js.`,
    type: 'object', required, properties, additionalProperties: false, 'x-kind': 'object', 'x-field-order': c.order,
  });
}
const assetClasses = {};
let objectCount = 0;
for (const s of objectSheets) {
  const used = {};
  for (const o of s.objects) { uninterpreted += finishRow(o, classes.get(o._class).order); used[o._class] = classSchema(o._class); }
  s.schemas = used;
  const rel = sheetFile(s);
  const { objects, ...head } = s;
  generated.push({ sheet: rel, kind: 'objects', asset_class: s.asset_class, schemas: used, rows: objects.length, sha256: writeSheet(rel, head, 'objects', objects), source_uexp_sha256: s._source.uexp_sha256 });
  const a = assetClasses[s.asset_class] || (assetClasses[s.asset_class] = { sheets: 0, objects: 0, skipped: 0 });
  a.sheets++; a.objects += objects.length;
  objectCount += objects.length;
}
for (const k of objectsSkipped) (assetClasses[k.asset_class] || (assetClasses[k.asset_class] = { sheets: 0, objects: 0, skipped: 0 })).skipped++;
const OUT = path.join(ROOT, 'audit', 'sheets.json');
writeJson(OUT, {
  generated_by: 'tools/sheets_build.js', ...stats, row_structs: byStruct.size, datatable_sheets: tableSheets, datatable_uninterpreted_values: tableUninterpreted,
  object_sheets: objectSheets.length, objects: objectCount, object_classes: classes.size, object_uninterpreted_values: uninterpreted - tableUninterpreted,
  object_packages_skipped: objectsSkipped.length, object_asset_classes: Object.fromEntries(Object.entries(assetClasses).sort()),
  sheets: generated.length, uninterpreted_values: uninterpreted, objects_skipped: objectsSkipped, generated,
});
logRun({ tool: 'sheets_build', version: VERSION, outputs: [OUT], note: `${generated.length} sheets, ${stats.rows} rows, ${objectCount} objects` });
console.log(`\n${tableSheets} DataTable sheets, ${byStruct.size} row schemas, ${stats.rows} rows, ${tableUninterpreted} uninterpreted values, ${stats.parse_errors.length} parse errors`);
console.log(`${objectSheets.length} object sheets, ${classes.size} object schemas, ${objectCount} objects, ${uninterpreted - tableUninterpreted} uninterpreted values, ${objectsSkipped.length} packages skipped`);
for (const e of stats.parse_errors.slice(0, 10)) console.log('  ', JSON.stringify(e));
