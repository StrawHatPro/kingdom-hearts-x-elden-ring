// Sheet <-> DataTable / tagged-property object conversion shared by tools/sheets_build.js (extract) and
// tools/sheets_check.js (compile back and compare). See sheets/README.md for the row format.
const ua = require('./uasset');
const up = require('./uprops');
const dt = require('./datatable');

const TAG_FIELDS = ['type', 'arrayIndex', 'structName', 'structGuid', 'enumName', 'innerType', 'valueType', 'hasGuid', 'guid'];
const template = (tag) => Object.fromEntries(TAG_FIELDS.filter((k) => tag[k] !== undefined).map((k) => [k, tag[k]]));
const fieldKey = (tag) => (tag.arrayIndex ? `${tag.name}[${tag.arrayIndex}]` : tag.name);
const isStr = (t) => t === 'StrProperty' || t === 'AssetObjectProperty';

// Property value -> the plainest JSON that still writes back exactly.
function toCell(tag, value) {
  if (tag.type === 'BoolProperty') return tag.boolVal !== 0 && tag.boolVal !== 1 ? { boolVal: tag.boolVal } : tag.boolVal === 1;
  if (isStr(tag.type) && value && value.raw === undefined) {
    if (value.enc === 'empty') return '';
    if (value.enc === 'ansi' && value.s.length > 0) return value.s;
    return value; // empty-but-terminated or UTF-16: keep the encoding
  }
  return value;
}
function fromCell(t, cell) {
  if (t.type === 'BoolProperty') return { boolVal: typeof cell === 'object' ? cell.boolVal : cell ? 1 : 0, value: null };
  if (isStr(t.type) && typeof cell === 'string') return { value: cell === '' ? { s: '', enc: 'empty' } : { s: cell, enc: 'ansi' } };
  return { value: cell };
}

// snake_case id from the original row name, unique within the sheet.
function makeIds(names) {
  const seen = new Map();
  return names.map((n) => {
    const s = typeof n === 'string' ? n : `name_${n.index}_${n.number}`;
    let id = s.replace(/([a-z0-9])([A-Z])/g, '$1_$2').toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_+|_+$/g, '') || 'row';
    const k = seen.get(id) || 0;
    seen.set(id, k + 1);
    return k ? `${id}_${k + 1}` : id;
  });
}

// Adds the properties to the row `out` as fields. `fields` (Map key -> template) accumulates the schema.
function addFields(out, props, fields, order) {
  const keys = [];
  for (const { tag, value } of props) {
    let key = fieldKey(tag);
    if (keys.includes(key) || key.startsWith('_') || key === 'id') key = `${key}#${keys.length}`; // repeated or reserved name: keep both, by position
    const t = template(tag);
    if (!fields.has(key)) { fields.set(key, { template: t, name: tag.name, count: 0 }); order.push(key); }
    const f = fields.get(key);
    f.count++;
    // A row whose tag differs from the schema's template carries its own.
    if (JSON.stringify(f.template) !== JSON.stringify(t)) (out._tags = out._tags || {})[key] = t;
    out[key] = toCell(tag, value);
    keys.push(key);
  }
  out._keys = keys; // serialized property order; dropped by the caller when it equals schema order
  return out;
}

// Package whose exports are all plain tagged-property objects -> { pkg, objects } or { error }.
// Export data layout: tagged properties terminated by None, then int32 0. The exports tile the
// .uexp in SerialOffset order and the 4-byte package tag follows the last one.
function parseObjects(pkg, uexpBuf) {
  const exps = pkg.exports.map((e, i) => ({ e, i })).sort((a, b) => a.e.serialOffset - b.e.serialOffset);
  const objects = [];
  let pos = 0;
  for (const { e, i } of exps) {
    const cls = ua.objectName(pkg, e.classIndex) || '(class)';
    const start = e.serialOffset - pkg.summary.totalHeaderSize, end = start + e.serialSize;
    if (start !== pos) return { error: `export ${i + 1} (${cls}) starts at ${start}, previous export ends at ${pos}` };
    if (end > uexpBuf.length) return { error: `export ${i + 1} (${cls}) ends beyond the .uexp` };
    const r = new ua.Reader(uexpBuf.subarray(0, end), start);
    let props;
    try { props = up.readProps(r, pkg); } catch (err) { return { error: `export ${i + 1} (${cls}): properties do not parse` }; }
    if (r.p + 4 !== end || r.i32() !== 0) return { error: `export ${i + 1} (${cls}): ${end - r.p} bytes follow the properties (natively serialized data)` };
    objects.push({ exportIndex: i + 1, cls, name: up.nameStr(pkg, e.objectName), offset: start, size: e.serialSize, props });
    pos = end;
  }
  if (pos + 4 !== uexpBuf.length || uexpBuf.readUInt32LE(pos) !== ua.TAG) return { error: 'the .uexp does not end with the package tag right after the last export' };
  return { pkg, objects };
}

// Parsed objects -> sheet. `classes` maps class name -> { fields, order, count } and accumulates the schemas.
function toObjectSheet(parsed, source, classes) {
  const { pkg, objects } = parsed;
  const ids = makeIds(objects.map((o) => o.name));
  const rows = objects.map((o, i) => {
    if (!classes.has(o.cls)) classes.set(o.cls, { fields: new Map(), order: [], count: 0 });
    const c = classes.get(o.cls);
    c.count++;
    const out = { id: ids[i], _source: { file: source.uexp, offset: '0x' + o.offset.toString(16).toUpperCase(), size: o.size }, _object_name: o.name, _class: o.cls, _export: o.exportIndex };
    return addFields(out, o.props, c.fields, c.order);
  });
  const { nameLookup, ...pkgPlain } = pkg;
  return {
    sheet: source.sheet, kind: 'objects', asset_class: source.assetClass,
    _source: { uasset: source.uasset, uexp: source.uexp, uasset_sha256: source.uassetSha, uexp_sha256: source.uexpSha, archive: source.archive },
    _package: pkgPlain,
    objects: rows,
  };
}

// Object sheet + schemas by class name -> { uasset, uexp } bytes.
function compileObjects(sheet, schemas) {
  const pkg = sheet._package;
  delete pkg.nameLookup;
  const w = new ua.Writer();
  const offset = (o) => pkg.exports[o._export - 1].serialOffset;
  for (const o of [...sheet.objects].sort((a, b) => offset(a) - offset(b))) {
    up.writeProps(w, pkg, rowProps(o, schemas[o._class], pkg));
    w.i32(0);
  }
  w.u32(ua.TAG);
  return { uasset: ua.serializeHeader(pkg), uexp: w.concat() };
}

// Sheet row -> [{ tag, value }] with the tag sizes recomputed from the values.
function rowProps(row, schema, pkg) {
  const order = schema['x-field-order'];
  const sizeOf = (t, value) => {
    const w = new ua.Writer();
    up.writeValue(w, pkg, t.type, value, t);
    return w.len;
  };
  const keys = row._keys || order.filter((k) => row[k] !== null && row[k] !== undefined);
  return keys.map((key) => {
    const t = (row._tags && row._tags[key]) || schema.properties[key]['x-ue'];
    const { value, boolVal } = fromCell(t, row[key]);
    const tag = { name: schema.properties[key]['x-ue-name'], ...t };
    if (boolVal !== undefined) tag.boolVal = boolVal;
    tag.size = t.type === 'BoolProperty' ? 0 : sizeOf(tag, value);
    return { tag, value };
  });
}

// Parsed DataTable -> sheet object. `fields` (Map key -> template) accumulates the schema.
function toSheet(parsed, source, fields, order) {
  const { pkg, table } = parsed;
  const ids = makeIds(table.rows.map((r) => r.name));
  const rows = table.rows.map((row, i) => {
    const out = { id: ids[i], _source: { file: source.uexp, offset: '0x' + row.offset.toString(16).toUpperCase(), size: row.size }, _row_name: row.name };
    return addFields(out, row.props, fields, order);
  });
  const { nameLookup, ...pkgPlain } = pkg;
  return {
    sheet: source.sheet, row_struct: table.rowStruct,
    _source: { uasset: source.uasset, uexp: source.uexp, uasset_sha256: source.uassetSha, uexp_sha256: source.uexpSha, archive: source.archive },
    _package: pkgPlain,
    _table: { object_props: table.objectProps, guid_flag: table.guidFlag, before_hex: table.before, after_hex: table.after },
    rows,
  };
}

// Sheet + schema -> { uasset, uexp } bytes.
function compile(sheet, schema) {
  const pkg = sheet._package;
  delete pkg.nameLookup;
  const rows = sheet.rows.map((row) => ({ name: row._row_name, props: rowProps(row, schema, pkg) }));
  const table = { objectProps: sheet._table.object_props, guidFlag: sheet._table.guid_flag, before: sheet._table.before_hex, after: sheet._table.after_hex, rows };
  return { uasset: ua.serializeHeader(pkg), uexp: dt.serialize(pkg, table) };
}

module.exports = { toSheet, compile, parseObjects, toObjectSheet, compileObjects, template, fieldKey, makeIds };
