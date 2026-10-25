// Gate for sheets (spec 5.2, 5.6, section 7): every sheet validates against its schema, and
// compiles back to the exact bytes of the original .uasset and .uexp.
// Writes audit/sheets_roundtrip.json. Exit code 1 on any failure.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const Ajv = require('ajv/dist/2020');
const { ROOT, logRun, writeJson } = require('./lib/log');
const pak = require('./lib/pak');
const sh = require('./lib/sheet');

const VERSION = '1.1.0';
const sha256 = (b) => crypto.createHash('sha256').update(b).digest('hex');
const ajv = new Ajv({ strict: false, allErrors: false });
const validators = new Map();
const index = JSON.parse(fs.readFileSync(path.join(ROOT, 'audit', 'sheets.json'), 'utf8'));
const files = pak.mountAll();
const fds = new Map();
const original = (p) => {
  const f = files.get(p);
  if (!fds.has(f.pak)) fds.set(f.pak, fs.openSync(f.pak, 'r'));
  return pak.extract(fds.get(f.pak), f.entry);
};

// Values kept as raw bytes because their type is not interpreted yet, at any nesting depth.
const countRaw = (v) => (!v || typeof v !== 'object' ? 0 : v.raw !== undefined ? 1 : Object.values(v).reduce((n, x) => n + countRaw(x), 0));
const res = { generated_by: 'tools/sheets_check.js', sheets: 0, rows: 0, schema_valid: 0, schema_invalid: 0, roundtrip_ok: 0, roundtrip_fail: 0, stale: 0,
  unk_fields: 0, uninterpreted_values: 0, complete_sheets: 0, by_kind: {}, failures: [] };
const loadSchema = (rel) => {
  if (!validators.has(rel)) { const schema = JSON.parse(fs.readFileSync(path.join(ROOT, rel), 'utf8')); validators.set(rel, { schema, validate: ajv.compile(schema) }); }
  return validators.get(rel);
};
const NO_SCHEMA = Object.assign(() => false, { errors: [{ instancePath: '/_class', message: 'no schema for this class' }] });
for (const g of index.generated) {
  res.sheets++;
  const text = fs.readFileSync(path.join(ROOT, g.sheet));
  if (sha256(text) !== g.sha256) { res.stale++; res.failures.push({ sheet: g.sheet, error: 'sheet differs from the generated one recorded in audit/sheets.json' }); }
  const sheet = JSON.parse(text);
  const objects = g.kind === 'objects';
  const kindName = objects ? 'objects' : 'datatable';
  const kind = res.by_kind[kindName] || (res.by_kind[kindName] = { sheets: 0, rows: 0, schema_valid: 0, roundtrip_ok: 0, complete_sheets: 0, uninterpreted_values: 0 });
  kind.sheets++;
  // a DataTable sheet has one schema for its rows; an object sheet one per class of object
  const schemas = objects ? Object.fromEntries(Object.entries(sheet.schemas).map(([c, rel]) => [c, loadSchema(rel)])) : null;
  const schema = objects ? null : loadSchema(g.schema).schema;
  let valid = true, raw = 0, unk = 0;
  for (const row of objects ? sheet.objects : sheet.rows) {
    res.rows++; kind.rows++;
    const validate = objects ? (schemas[row._class] ? schemas[row._class].validate : NO_SCHEMA) : loadSchema(g.schema).validate;
    if (!validate(row)) { valid = false; if (res.failures.length < 50) res.failures.push({ sheet: g.sheet, row: row.id, error: ajv.errorsText(validate.errors) }); break; }
    for (const [k, v] of Object.entries(row)) { if (k.startsWith('unk_')) unk++; if (!k.startsWith('_')) raw += countRaw(v); }
  }
  valid ? res.schema_valid++ : res.schema_invalid++;
  res.unk_fields += unk; res.uninterpreted_values += raw;
  let same = false;
  try {
    const built = objects ? sh.compileObjects(sheet, Object.fromEntries(Object.entries(schemas).map(([c, v]) => [c, v.schema]))) : sh.compile(sheet, schema);
    same = built.uasset.equals(original(sheet._source.uasset)) && built.uexp.equals(original(sheet._source.uexp));
    if (!same && res.failures.length < 50) res.failures.push({ sheet: g.sheet, error: 'compiled bytes differ from the original' });
  } catch (e) { if (res.failures.length < 50) res.failures.push({ sheet: g.sheet, error: 'compile: ' + e.message }); }
  same ? res.roundtrip_ok++ : res.roundtrip_fail++;
  if (valid && same && !unk && !raw) { res.complete_sheets++; kind.complete_sheets++; }
  if (valid) kind.schema_valid++;
  if (same) kind.roundtrip_ok++;
  kind.uninterpreted_values += raw;
}
for (const fd of fds.values()) fs.closeSync(fd);
const OUT = path.join(ROOT, 'audit', 'sheets_roundtrip.json');
writeJson(OUT, res);
const failed = res.schema_invalid || res.roundtrip_fail || res.stale;
logRun({ tool: 'sheets_check', version: VERSION, outputs: [OUT], note: failed ? 'FAIL' : 'PASS' });
const { failures, ...show } = res;
console.log(JSON.stringify(show, null, 1));
for (const f of failures.slice(0, 10)) console.log('  ', JSON.stringify(f));
if (failed) process.exit(1);
