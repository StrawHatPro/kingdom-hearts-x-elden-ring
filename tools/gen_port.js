// Sheets -> Rust for the Sora port: every generated file under build/gen/rust/ is plain data, so
// that the engine-free crates stay deterministic and read no files. The crates include them by
// path (crates/kh-core/build.rs exports the folder as KH_GEN_DIR; a module does
// `include!(concat!(env!("KH_GEN_DIR"), "/<file>.rs"));`). Plan and contract: docs/sora-port-plan.md
// section 5. THE REGISTRY an implementer edits is tools/lib/port_gen_defs.js (params, tables, enums).
//
//   sheets/port/sora-movement.json  ->  sora_params.rs          SoraParams: one field per row, doc = source/evidence
//   sheets/port/sora-camera.json    ->  sora_camera_params.rs   CameraParams, the same way (crates/kh-camera)
//   sheets/port/<any other parameter sheet>.json  ->  <name with _>.rs   the same way (PARAMETER SHEETS below)
//   sheets/port/<a table sheet registered with a schema>.json  ->  its file: a row struct and a static table
//   sheets/reflect/enum-values.json ->  tres_enums.rs           the reflected enums the registry lists
//   build/sora_pack/manifest.json   ->  sora_anim.rs            EVERY row of Sora's main anim set: the enum AnimId (the
//                                       14 stage-1 rows first, pinned; then the set's order), per row its
//                                       stage-1 play mode (if any), blend-in, blend-space samples (1D) or
//                                       grid (2D); per sequence its length, frames, the per-frame root track
//                                       where bEnableRootMotion, and EVERY notify as typed data: the enum
//                                       NotifyClass (the classes present), NotifyData (decoded properties of
//                                       the classes a spec defines) and the raw remainder as (name, JSON text)
//
//   node tools/gen_port.js [--check] [--sheet <file>] [--out-dir <dir>] [--defs <file>] [--sheets-dir <dir>]
//     --check       write nothing; exit 1 when a file on disk differs from what the rows give
//     --sheet       read this file instead of sheets/port/sora-movement.json (for trying a draft)
//     --out-dir     write there instead of build/gen/rust (build the crates against it with the
//                   environment variable KH_GEN_RUST_DIR=<dir>)
//     --defs        another registry instead of tools/lib/port_gen_defs.js (for fixtures)
//     --sheets-dir  another folder instead of sheets/port (for fixtures)
//
// PARAMETER SHEETS: columns id, value, unit, source, evidence, ref_addr, ref_conf, note. One struct
// per sheet, one field per row (id with dots as underscores). Value types: number -> f32 (i32 when
// the unit is int, count, frames or index and the value is whole), true/false -> bool, string ->
// &'static str, array of numbers -> [f32; N]. Generated: the struct, <PREFIX>_PARAMS (the values),
// <PREFIX>_PARAMS_ORIGIN, <PREFIX>_PARAM_ROWS, <PREFIX>_PARAM_INFORMATIONAL.
// - REGISTERED sheets (registry `params`): a row whose value is unchecked (null) cannot become a
//   field: the ids are listed, exit code 1, nothing is written. INFORMATIONAL rows are the
//   exception: they record a question, not a code input, and are left out of the struct whether
//   checked or not; the registry says per sheet which ids they are and what the code uses instead.
// - UNREGISTERED sheets (any other sheets/port/*.json with id and value columns) get default
//   names: sora-combat-params.json -> sora_combat_params.rs, struct SoraCombatParams, constants
//   SORA_COMBAT_PARAMS ... (a trailing "-params" of the sheet name is not doubled). If such a sheet
//   has unchecked or untypable rows it is NOT GENERATED (a stale file of it is deleted) and the run
//   goes on with exit code 0: research sheets appear there unfinished. Register the sheet with its
//   informational list once code depends on it.
//
// TABLE SHEETS (one row = one attack, spell, ability, sound ...): generated only from a schema in
// the registry's `tables` (keys documented there): `pub struct <Row>` (Clone, Copy, Debug,
// PartialEq), `pub static <TABLE>: &[<Row>]`, with a key column `pub const <KEY_PREFIX>_<VALUE>:
// usize` per row and, for a string key, `pub fn <table>_by_<key>(&str) -> Option<&'static <Row>>`.
// Column types: str, f32, i32, u32, bool, yesno (the cells "yes" / "no" -> bool), f32[], i32[], str[],
// enum (the cell may also be the game name without the enum's common prefix: "SMALL"). A schema may
// set hex_key: true to name the key constants of an integer key by its value in hex (KIND_0A).
// (For enum columns the generated file expects
// the enum type in scope: `use` it in the including module). A null cell is None in an `optional`
// column (so is the cell "-" when the optional column is not of type str) and an error (exit 1) in
// any other.
//
// ENUMS: for each name in the registry's `enums`, a `#[repr(i32)] pub enum <GameName>` with the
// game's values, `ALL`, `name()` (the Rust variant: "AirSlide"), `game_name()` (the sheet's:
// "TSID_STATE_AIR_SLIDE"), `value()`, `from_value()`, `from_game_name()`. Variant names: the
// `Enum::` qualifier and the prefix all values share up to an underscore are dropped
// (TSID_STATE_IDLE -> Idle), then UPPER_SNAKE -> CamelCase. A second name with the same value
// becomes an associated constant. An unchecked value in a listed enum is an error.
//
// Exit code: 0 written (or --check clean), 1 unchecked values / bad rows / missing inputs / --check differs.
const fs = require('fs');
const path = require('path');
const { ROOT, readSheet, checked } = require('./lib/sheets');
const { BANNER_MARK, GEN_ROOT } = require('./lib/genout');

const VERSION = '3.1.0';
const CHECK = process.argv.includes('--check');
const arg = (name) => { const i = process.argv.indexOf(name); return i > 0 && process.argv[i + 1] ? path.resolve(process.argv[i + 1]) : null; };
const SHEETS_DIR = arg('--sheets-dir') || path.join(ROOT, 'sheets', 'port');
const DEFS = require(arg('--defs') || './lib/port_gen_defs');
const MOVEMENT_DRAFT = arg('--sheet');
const ENUM_VALUES = path.join(ROOT, 'sheets', 'reflect', 'enum-values.json');
const MANIFEST = path.join(ROOT, 'build', 'sora_pack', 'manifest.json');
const OUT_DIR = arg('--out-dir') || path.join(ROOT, GEN_ROOT, 'rust');
const COLUMNS = ['id', 'value', 'unit', 'source', 'evidence', 'ref_addr', 'ref_conf', 'note'];
const INT_UNITS = new Set(['int', 'count', 'frames', 'index']);
const RESERVED = new Set(['as', 'break', 'const', 'continue', 'crate', 'else', 'enum', 'extern', 'false', 'fn', 'for', 'if', 'impl', 'in', 'let', 'loop', 'match', 'mod', 'move', 'mut', 'pub', 'ref', 'return', 'self', 'static', 'struct', 'super', 'trait', 'true', 'type', 'unsafe', 'use', 'where', 'while', 'async', 'await', 'dyn', 'abstract', 'become', 'box', 'do', 'final', 'macro', 'override', 'priv', 'typeof', 'unsized', 'virtual', 'yield', 'try']);
const rel = (p) => path.relative(ROOT, p).replace(/\\/g, '/');
const fail = (msg) => { console.log(`gen_port: FAILED  ${msg}`); process.exit(1); };
const banner = (from) => `${BANNER_MARK}${from} by tools/gen_port.js (emitter ${VERSION}) - DO NOT EDIT: change the input and run node tools/gen_port.js\n`;

// "MaxWalkSpeed", "jump.height", "Jump Height (max)" -> max_walk_speed, jump_height, jump_height_max
function field(id) {
  let f = String(id).replace(/([a-z0-9])([A-Z])/g, '$1_$2').replace(/([A-Z]+)([A-Z][a-z])/g, '$1_$2').toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_+|_+$/g, '');
  if (!f) f = 'param';
  if (/^[0-9]/.test(f)) f = '_' + f;
  if (RESERVED.has(f)) f += '_';
  return f;
}
// "sora-combat-params", "AIR_SLIDE", "ETresAbilityKind_MAX" -> SoraCombatParams, AirSlide, ETresAbilityKindMax
function camel(name) {
  let c = String(name).split(/[^A-Za-z0-9]+/).filter(Boolean).map((p) => (/[a-z]/.test(p) ? p[0].toUpperCase() + p.slice(1) : p[0] + p.slice(1).toLowerCase())).join('');
  if (!c) c = 'Value';
  if (/^[0-9]/.test(c)) c = '_' + c;
  if (c === 'Self' || RESERVED.has(c)) c += '_';
  return c;
}
const f32 = (n) => { const s = String(Math.fround(n) === n ? n : Number(n.toPrecision(9))); return /[.eE]/.test(s) ? s : s + '.0'; };
function typed(row) {
  const v = row.value;
  if (typeof v === 'number' && Number.isFinite(v)) return INT_UNITS.has(row.unit) && Number.isInteger(v) ? ['i32', String(v)] : ['f32', f32(v)];
  if (typeof v === 'boolean') return ['bool', String(v)];
  if (typeof v === 'string') return ["&'static str", JSON.stringify(v)];
  if (Array.isArray(v) && v.length && v.every((x) => typeof x === 'number' && Number.isFinite(x))) return [`[f32; ${v.length}]`, `[${v.map(f32).join(', ')}]`];
  return null;
}
const text = (v) => (checked(v) && v !== '-' && v !== '' ? String(v).replace(/\s+/g, ' ').trim() : '');
const lit = (s) => JSON.stringify(String(s));
const docEscape = (d) => d.replace(/[<>\[\]]/g, (c) => '\\' + c);

// [file name, text, what it holds] of everything to write; files to delete when present.
const outputs = [];
const stale = [];
const claim = (() => { const seen = new Map(); return (kind, name, by) => { const k = `${kind} ${name}`; if (seen.has(k)) fail(`${by}: the ${kind} ${name} is already generated for ${seen.get(k)} (name the sheet's output in tools/lib/port_gen_defs.js)`); seen.set(k, by); }; })();

// ================================================================ enums (tres_enums.rs)
// name -> { variants: [{ variant, game, value }], aliases: [{ constant, game, variant }], byGame, byValue }
const ENUMS = new Map();
if ((DEFS.enums || []).length) {
  if (!fs.existsSync(ENUM_VALUES)) fail(`${rel(ENUM_VALUES)} does not exist`);
  const sheet = readSheet(ENUM_VALUES);
  const wanted = new Set(DEFS.enums);
  const rows = new Map(DEFS.enums.map((e) => [e, []]));
  for (const r of sheet.rows) if (wanted.has(r[sheet.col.enum])) rows.get(r[sheet.col.enum]).push({ game: r[sheet.col.name], value: r[sheet.col.value] });
  // (XSI, 2026-10-16) the port's own values (sheets/port/sora-enum-ext.json, tools/sora_xs_sheet.js: the XS abilities,
  // ETresAbilityKind 256..): appended after the game's; a value or name the game has is an error.
  const EXT = path.join(ROOT, 'sheets', 'port', 'sora-enum-ext.json');
  if (fs.existsSync(EXT)) {
    const ext = readSheet(EXT);
    for (const r of ext.rows) {
      const e = r[ext.col.enum], list = rows.get(e);
      if (!list) continue;
      if (list.some((v) => v.value === r[ext.col.enum_value] || v.game === r[ext.col.name])) fail(`sora-enum-ext ${r[ext.col.id]}: the value or name is the game's already`);
      list.push({ game: r[ext.col.name], value: r[ext.col.enum_value] });
    }
  }
  let out = banner('reflect/enum-values');
  out += `\n/// Where the enums come from.\npub const TRES_ENUMS_SOURCE: &str = "sheets/reflect/enum-values";\n`;
  for (const name of DEFS.enums) {
    const values = rows.get(name);
    if (!values.length) fail(`enum ${name} (tools/lib/port_gen_defs.js enums): no rows in ${rel(ENUM_VALUES)}`);
    const bad = values.filter((v) => !checked(v.game) || !Number.isInteger(v.value) || Math.abs(v.value) > 0x7fffffff);
    if (bad.length) fail(`enum ${name}: value(s) unchecked or not a 32-bit integer: ${bad.map((v) => `${v.game}=${JSON.stringify(v.value)}`).join(', ')}`);
    // The short name: without the `Enum::` qualifier and without the prefix every value shares up
    // to an underscore (UE's own `<Enum>_MAX` sentinel does not take part in finding the prefix).
    const short = values.map((v) => String(v.game).replace(/^.*::/, ''));
    const forPrefix = short.filter((s) => s !== `${name}_MAX`);
    let prefix = forPrefix.length > 1 ? forPrefix[0] : '';
    for (const s of forPrefix) while (prefix && !s.startsWith(prefix)) prefix = prefix.slice(0, -1);
    prefix = prefix.slice(0, prefix.lastIndexOf('_') + 1);
    const e = { variants: [], aliases: [], byGame: new Map(), byValue: new Map() };
    const names = new Map();
    values.forEach((v, i) => {
      const rest = short[i].startsWith(prefix) && short[i].length > prefix.length ? short[i].slice(prefix.length) : short[i];
      if (e.byValue.has(v.value)) {
        const first = e.byValue.get(v.value);
        e.aliases.push({ constant: field(rest).toUpperCase(), game: v.game, variant: first });
        e.byGame.set(v.game, first);
        return;
      }
      const variant = camel(rest);
      if (names.has(variant)) fail(`enum ${name}: ${v.game} and ${names.get(variant)} both become the variant ${variant}`);
      names.set(variant, v.game);
      e.variants.push({ variant, game: v.game, value: v.value });
      e.byValue.set(v.value, variant);
      e.byGame.set(v.game, variant);
    });
    for (const [game, variant] of [...e.byGame]) e.byGame.set(String(game).replace(/^.*::/, ''), variant);
    e.variants.sort((a, b) => a.value - b.value);
    ENUMS.set(name, e);
    out += `\n/// \`${name}\`: ${e.variants.length} values${prefix ? `; the game's names without their common prefix \`${prefix}\`` : ''}.\n`;
    out += `#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]\n#[repr(i32)]\npub enum ${name} {\n`;
    for (const v of e.variants) out += `    /// \`${v.game}\` = ${v.value}\n    ${v.variant} = ${v.value},\n`;
    out += `}\n\nimpl ${name} {\n`;
    for (const a of e.aliases) out += `    /// \`${a.game}\`: the same value as \`${a.variant}\`.\n    pub const ${a.constant}: ${name} = ${name}::${a.variant};\n`;
    out += `    /// Every value, ascending.\n    pub const ALL: &'static [${name}] = &[${e.variants.map((v) => `${name}::${v.variant}`).join(', ')}];\n\n`;
    out += `    /// The Rust variant's name ("${e.variants[Math.min(1, e.variants.length - 1)].variant}").\n    pub const fn name(self) -> &'static str {\n        match self {\n`;
    for (const v of e.variants) out += `            ${name}::${v.variant} => ${lit(v.variant)},\n`;
    out += `        }\n    }\n\n    /// The game's name of the value, as reflection has it.\n    pub const fn game_name(self) -> &'static str {\n        match self {\n`;
    for (const v of e.variants) out += `            ${name}::${v.variant} => ${lit(v.game)},\n`;
    out += `        }\n    }\n\n    pub const fn value(self) -> i32 {\n        self as i32\n    }\n\n    pub const fn from_value(value: i32) -> Option<${name}> {\n        match value {\n`;
    for (const v of e.variants) out += `            ${v.value} => Some(${name}::${v.variant}),\n`;
    out += `            _ => None,\n        }\n    }\n\n    /// By the game's name, with or without its \`Enum::\` qualifier.\n    pub fn from_game_name(name: &str) -> Option<${name}> {\n        let name = name.rsplit("::").next().unwrap_or(name);\n        match name {\n`;
    const done = new Set();
    for (const [game, variant] of e.byGame) { const g = String(game).replace(/^.*::/, ''); if (done.has(g)) continue; done.add(g); out += `            ${lit(g)} => Some(${name}::${variant}),\n`; }
    out += `            _ => None,\n        }\n    }\n}\n`;
  }
  claim('file', 'tres_enums.rs', 'the enums');
  outputs.push(['tres_enums.rs', out, `${DEFS.enums.length} enums (${DEFS.enums.map((n) => `${n} ${ENUMS.get(n).variants.length}`).join(', ')})`]);
}

// ================================================================ parameter sheets
// One struct per sheet: a field per row, the doc comment is the row's source, evidence and address.
// Returns { out, what } or, when rows cannot become fields, { problems: [lines] }.
function paramsFile(file, informationalIds, names) {
  const sheet = readSheet(file);
  const all = sheet.rows.map((r) => Object.fromEntries(COLUMNS.map((c) => [c, c in sheet.col ? r[sheet.col[c]] : null])));
  const informational = all.filter((r) => r.id in informationalIds);
  if (names.values) return valuesFile(all, informational, informationalIds, names);
  const rows = all.filter((r) => !(r.id in informationalIds));
  const problems = [];
  const missing = rows.filter((r) => !checked(r.id) || !checked(r.value)).map((r) => (checked(r.id) ? r.id : '(row without id)'));
  const seen = new Map();
  for (const r of rows) {
    if (!checked(r.id) || !checked(r.value)) continue;
    r.field = field(r.id);
    r.typed = typed(r);
    if (!r.typed) problems.push(`${r.id}: value ${JSON.stringify(r.value).slice(0, 60)} is not a number, boolean, string or array of numbers`);
    if (seen.has(r.field)) problems.push(`${r.id}: field name ${r.field} is also the name of ${seen.get(r.field)}`);
    seen.set(r.field, r.id);
  }
  if (missing.length) problems.unshift(`${missing.length} row(s) with an unchecked value (null) that are not listed as informational in tools/lib/port_gen_defs.js: ${missing.join(', ')}`);
  if (problems.length) return { problems };
  let out = banner(names.from);
  out += `\n/// ${names.doc}\n`;
  out += `#[derive(Clone, Copy, Debug, PartialEq)]\npub struct ${names.struct} {\n`;
  for (const r of rows) {
    const doc = [`\`${r.id}\` = ${JSON.stringify(r.value)}${text(r.unit) ? ' ' + text(r.unit) : ''}.`, `Source: ${text(r.source) || '(none given)'}.`];
    if (text(r.evidence)) doc.push(`Evidence: ${text(r.evidence)}.`);
    const ref = [text(r.ref_addr), text(r.ref_conf) ? `confidence ${text(r.ref_conf)}` : ''].filter(Boolean).join(', ');
    if (ref) doc.push(`Ref: ${ref}.`);
    if (text(r.note)) doc.push(text(r.note));
    out += doc.map((d) => `    /// ${docEscape(d)}\n`).join('    ///\n');
    out += `    pub ${r.field}: ${r.typed[0]},\n`;
  }
  out += `}\n\n/// The values of the rows.\npub const ${names.prefix}_PARAMS: ${names.struct} = ${names.struct} {\n`;
  for (const r of rows) out += `    ${r.field}: ${r.typed[1]},\n`;
  out += `};\n\n/// Where the values come from.\npub const ${names.prefix}_PARAMS_ORIGIN: &str = "sheet";\n\n`;
  out += `/// Every row that became a field: (field, sheet id, unit, source).\npub const ${names.prefix}_PARAM_ROWS: &[(&str, &str, &str, &str)] = &[\n`;
  for (const r of rows) out += `    (${lit(r.field)}, ${lit(r.id)}, ${lit(text(r.unit))}, ${lit(text(r.source))}),\n`;
  out += `];\n\n/// Rows of the sheet that are questions, not code inputs: (sheet id, what the code uses instead).\npub const ${names.prefix}_PARAM_INFORMATIONAL: &[(&str, &str)] = &[\n`;
  for (const r of informational) out += `    (${lit(r.id)}, ${lit(informationalIds[r.id])}),\n`;
  out += '];\n';
  return { out, what: `${rows.length} fields (${informational.length} informational row(s) left out: ${informational.map((r) => r.id).join(', ') || 'none'})` };
}

// A registered parameter sheet with `values: true` (magic implementer, 2026-10-11): instead of a
// struct with one field per row, ONE sorted lookup table (sheet id -> value) is generated:
// `pub const <PREFIX>_PARAM_VALUES: &[(&str, PV)]`, ascending by id in byte order (binary search).
// The including module defines `PV` (N(f32) number, B(bool), S(&str) name, A(&[f32]) list of
// numbers, L(&[&str]) list of names). For sheets of thousands of uniform rows that the code reads
// by a composed id ("<spell>.<stage>_<property>"). A null outside the informational list fails.
function valuesFile(all, informational, informationalIds, names) {
  const rows = all.filter((r) => !(r.id in informationalIds));
  const problems = [];
  const missing = rows.filter((r) => !checked(r.id) || !checked(r.value)).map((r) => (checked(r.id) ? r.id : '(row without id)'));
  if (missing.length) problems.push(`${missing.length} row(s) with an unchecked value (null) that are not listed as informational in tools/lib/port_gen_defs.js: ${missing.join(', ')}`);
  const seen = new Set();
  const value = (v) => {
    if (typeof v === 'number' && Number.isFinite(v)) return `PV::N(${f32(v)})`;
    if (typeof v === 'boolean') return `PV::B(${v})`;
    if (typeof v === 'string') return `PV::S(${JSON.stringify(v)})`;
    if (Array.isArray(v) && v.every((x) => typeof x === 'number' && Number.isFinite(x))) return `PV::A(&[${v.map(f32).join(', ')}])`;
    if (Array.isArray(v) && v.every((x) => typeof x === 'string')) return `PV::L(&[${v.map((x) => JSON.stringify(x)).join(', ')}])`;
    return null;
  };
  for (const r of rows) {
    if (!checked(r.id) || !checked(r.value)) continue;
    r.lit = value(r.value);
    if (!r.lit) problems.push(`${r.id}: value ${JSON.stringify(r.value).slice(0, 60)} is not a number, boolean, string or list of numbers / strings`);
    if (seen.has(r.id)) problems.push(`${r.id}: the id occurs twice`);
    seen.add(r.id);
  }
  if (problems.length) return { problems };
  const sorted = [...rows].sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
  let out = banner(names.from);
  out += `\n/// ${names.doc}\n///\n/// (sheet id, value), ascending by id in byte order.\npub const ${names.prefix}_PARAM_VALUES: &[(&str, PV)] = &[\n`;
  for (const r of sorted) out += `    (${lit(r.id)}, ${r.lit}),\n`;
  out += `];\n\n/// Where the values come from.\npub const ${names.prefix}_PARAMS_ORIGIN: &str = "sheet";\n\n`;
  out += `/// Rows of the sheet that are questions, not code inputs: (sheet id, what the code uses instead).\npub const ${names.prefix}_PARAM_INFORMATIONAL: &[(&str, &str)] = &[\n`;
  for (const r of informational) out += `    (${lit(r.id)}, ${lit(informationalIds[r.id])}),\n`;
  out += '];\n';
  return { out, what: `${rows.length} values in a lookup table (${informational.length} informational row(s) left out)` };
}

// ================================================================ table sheets
const CELL = {
  str: ["&'static str", (v) => (typeof v === 'string' ? JSON.stringify(v) : null)],
  f32: ['f32', (v) => (typeof v === 'number' && Number.isFinite(v) ? f32(v) : null)],
  i32: ['i32', (v) => (Number.isInteger(v) && Math.abs(v) <= 0x7fffffff ? String(v) : null)],
  u32: ['u32', (v) => (Number.isInteger(v) && v >= 0 && v <= 0xffffffff ? String(v) : null)],
  bool: ['bool', (v) => (typeof v === 'boolean' ? String(v) : null)],
  yesno: ['bool', (v) => (v === 'yes' ? 'true' : v === 'no' ? 'false' : null)],
};
for (const t of ['str', 'f32', 'i32']) CELL[`${t}[]`] = [`&'static [${CELL[t][0]}]`, (v) => { if (!Array.isArray(v)) return null; const cells = v.map(CELL[t][1]); return cells.includes(null) ? null : `&[${cells.join(', ')}]`; }];
function tableFile(file, name, def) {
  const sheet = readSheet(file);
  const problems = [];
  const columns = Object.entries(def.columns || {}).map(([column, c]) => {
    if (!(column in sheet.col)) problems.push(`the sheet has no column "${column}"`);
    let cell = CELL[c.type];
    if (c.type === 'enum') {
      const e = ENUMS.get(c.enum);
      if (!e) problems.push(`column ${column}: enum ${c.enum} is not in the registry's enums list`);
      else cell = [c.enum, (v) => { const variant = typeof v === 'number' ? e.byValue.get(v) : (e.byGame.get(v) || (e.variants.find((x) => x.variant === camel(v)) || {}).variant); return variant ? `${c.enum}::${variant}` : null; }];
    }
    if (!cell) problems.push(`column ${column}: unknown type ${JSON.stringify(c.type)}`);
    return { column, field: c.field || field(column), optional: !!c.optional, doc: c.doc || '', type: c.type, cell: cell || ['()', () => null] };
  });
  if (!columns.length) problems.push('the schema lists no columns');
  if (!def.row || !def.table || !def.file) problems.push('the schema needs file, row and table');
  if (def.key && !columns.some((c) => c.column === def.key)) problems.push(`key column "${def.key}" is not among the schema's columns`);
  if (problems.length) return { problems };
  const key = def.key ? columns.find((c) => c.column === def.key) : null;
  const rowName = (r, i) => (key ? String(r[sheet.col[key.column]]) : `row ${i + 1}`);
  const lines = [];
  const keys = new Map();
  sheet.rows.forEach((r, i) => {
    const cells = columns.map((c) => {
      const v = r[sheet.col[c.column]];
      // '-' is the sheets' "not applicable": in an optional column that is not text it is None too.
      if (!checked(v) || (c.optional && v === '-' && c.type !== 'str')) { if (c.optional) return `${c.field}: None`; problems.push(`${rowName(r, i)}: column ${c.column} is unchecked (null) and not optional`); return ''; }
      const made = c.cell[1](v);
      if (made === null) { problems.push(`${rowName(r, i)}: column ${c.column}: ${JSON.stringify(v).slice(0, 60)} is not a ${c.type}${c.type === 'enum' ? ' value' : ''}`); return ''; }
      return `${c.field}: ${c.optional ? `Some(${made})` : made}`;
    });
    if (key) {
      const kv = r[sheet.col[key.column]];
      const constant = def.hex_key && Number.isInteger(kv) ? kv.toString(16).toUpperCase().padStart(2, '0') : field(kv).toUpperCase();
      if (keys.has(constant)) problems.push(`${rowName(r, i)}: its key constant ${constant} is also row ${keys.get(constant) + 1}'s`);
      keys.set(constant, i);
    }
    const about = ['source', 'evidence', 'ref_addr', 'note'].filter((c) => c in sheet.col && !(c in (def.columns || {})) && text(r[sheet.col[c]])).map((c) => `${c}: ${text(r[sheet.col[c]])}`).join('; ');
    lines.push(`${about ? `    // ${about}\n` : ''}    ${def.row} { ${cells.join(', ')} },\n`);
  });
  if (problems.length) return { problems };
  let out = banner(`port/${name}`);
  out += `\n/// Where the rows come from.\npub const ${def.table}_SOURCE: &str = ${lit(`sheets/port/${name}`)};\n`;
  out += `\n/// ${def.doc || `One row of sheets/port/${name}.`}\n#[derive(Clone, Copy, Debug, PartialEq)]\npub struct ${def.row} {\n`;
  for (const c of columns) out += `    /// Column \`${c.column}\`${c.doc ? `: ${docEscape(c.doc)}` : ''}.\n    pub ${c.field}: ${c.optional ? `Option<${c.cell[0]}>` : c.cell[0]},\n`;
  out += `}\n\n/// The rows, in the sheet's order.\npub static ${def.table}: &[${def.row}] = &[\n${lines.join('')}];\n`;
  if (key) {
    out += `\n// Index of each row in \`${def.table}\`, by its \`${key.column}\`.\n`;
    for (const [constant, i] of keys) out += `pub const ${def.key_prefix || def.table}_${constant}: usize = ${i};\n`;
    if (key.type === 'str' && !key.optional) out += `\n/// The row whose \`${key.column}\` is \`key\`.\npub fn ${def.table.toLowerCase()}_by_${key.field}(key: &str) -> Option<&'static ${def.row}> {\n    ${def.table}.iter().find(|row| row.${key.field} == key)\n}\n`;
  }
  return { out, what: `${sheet.rows.length} rows of ${def.row} (${columns.length} columns)` };
}

// ================================================================ the sheets of sheets/port
const defaultNames = (name) => { const base = name.replace(/-params$/, ''); return { file: `${field(name)}.rs`, struct: camel(base) + 'Params', prefix: field(base).toUpperCase(), from: `port/${name}`, doc: `The rows of sheets/port/${name}.json, one field per row (generated with default names: the sheet is not registered in tools/lib/port_gen_defs.js).` }; };
const sheetNames = fs.existsSync(SHEETS_DIR) ? fs.readdirSync(SHEETS_DIR).filter((f) => f.endsWith('.json')).map((f) => f.slice(0, -5)).sort() : [];
for (const name of new Set([...Object.keys(DEFS.params || {}), ...Object.keys(DEFS.tables || {}), ...sheetNames])) {
  const registered = (DEFS.params || {})[name];
  const table = (DEFS.tables || {})[name];
  const file = name === 'sora-movement' && MOVEMENT_DRAFT ? MOVEMENT_DRAFT : path.join(SHEETS_DIR, `${name}.json`);
  if (!fs.existsSync(file)) fail(`${rel(file)} does not exist (it is registered in tools/lib/port_gen_defs.js)`);
  let sheet;
  try { sheet = readSheet(file); } catch (e) { sheet = null; }
  const isSheet = sheet && Array.isArray(sheet.columns) && Array.isArray(sheet.rows);
  if (!isSheet) { if (registered || table) fail(`${rel(file)} is not a sheet ({sheet, group, columns, rows})`); console.log(`gen_port: ${rel(file)}: not a sheet: skipped`); continue; }
  if (table) {
    const made = tableFile(file, name, table);
    if (made.problems) { console.log(`gen_port: FAILED  ${rel(file)} (table ${table.table}): nothing written`); for (const p of made.problems) console.log(`  ${p}`); process.exit(1); }
    claim('file', table.file, name); claim('type', table.row, name); claim('constant', table.table, name);
    outputs.push([table.file, made.out, made.what]);
    continue;
  }
  const isParams = 'id' in sheet.col && 'value' in sheet.col;
  if (!isParams) { if (registered) fail(`${rel(file)} has no "id" / "value" column (expected ${COLUMNS.join(', ')})`); console.log(`gen_port: ${rel(file)}: not a parameter sheet and no table schema in tools/lib/port_gen_defs.js: skipped`); continue; }
  const names = registered || defaultNames(name);
  const made = paramsFile(file, (registered && registered.informational) || {}, names);
  if (made.problems) {
    if (registered) { console.log(`gen_port: FAILED  ${rel(file)}: nothing written`); for (const p of made.problems) console.log(`  ${p}`); process.exit(1); }
    console.log(`gen_port: ${rel(file)}: NOT GENERATED (the sheet is not registered; register it in tools/lib/port_gen_defs.js with an informational list once its rows are final):`);
    for (const p of made.problems) console.log(`  ${p.slice(0, 400)}`);
    stale.push(names.file);
    continue;
  }
  claim('file', names.file, name); if (!names.values) claim('type', names.struct, name); claim('constant', `${names.prefix}_PARAMS`, name);
  outputs.push([names.file, made.out, made.what + (registered ? '' : ' [unregistered sheet: default names]')]);
}

// ================================================================ sora_anim.rs
// EVERY row of Sora's main anim set that the pack carries (manifest.logical_animations), every
// sequence they use, every notify of every sequence as typed data.
//
// ORDER OF THE ANIMATIONS (= the values of the generated enum AnimId): the 14 animations of the
// stage-1 states first, in the order LEGACY_ORDER gives - crates/kh-core/tests/golden.rs hashes
// `AnimId as u32`, so these 14 values are pinned - then every other row in the manifest's order.
if (!fs.existsSync(MANIFEST)) fail(`${rel(MANIFEST)} does not exist: the animation tables come from the Sora pack (node tools/sora_pack.js)`);
const man = JSON.parse(fs.readFileSync(MANIFEST, 'utf8'));
// (XSI, 2026-10-16) THE OVERLAY of the XS abilities' clips (tools/xsi_pack.js): build/sora_pack/xsi_manifest.json once deployed,
// else the staged build/sora_tmp/xsi/pack/xsi_manifest.json. Its rows come after the main manifest's (no AnimId value moves).
for (const overlay of [path.join(ROOT, 'build', 'sora_pack', 'xsi_manifest.json'), path.join(ROOT, 'build', 'sora_tmp', 'xsi', 'pack', 'xsi_manifest.json')]) {
  if (!fs.existsSync(overlay)) continue;
  const ov = JSON.parse(fs.readFileSync(overlay, 'utf8'));
  man.items.push(...ov.items);
  man.logical_animations.push(...ov.logical_animations);
  Object.assign((man.root_motion = man.root_motion || {}).tracks = man.root_motion.tracks || {}, ov.root_motion.tracks);
  Object.assign((man.attack_sockets = man.attack_sockets || {}).clips = man.attack_sockets.clips || {}, ov.attack_sockets.clips);
  break;
}
const LEGACY_ORDER = ['IdleN', 'IdleBreakN', 'WalkLoopN', 'WalkEndR', 'WalkEndL', 'RunStartN', 'RunLoopN', 'RunEndR', 'RunEndL', 'JumpNormalStart', 'JumpNormalRiseLoop', 'JumpNormalDownLoop', 'Land', 'LightLand'];
const items = new Map(man.items.filter((i) => i.kind === 'animation').map((i) => [i.animation_name, i]));
const tracks = (man.root_motion && man.root_motion.tracks) || {};
const seqNames = [];
const seqIndex = (name) => { if (!items.has(name)) fail(`${rel(MANIFEST)}: no animation item ${name}`); let i = seqNames.indexOf(name); if (i < 0) { i = seqNames.length; seqNames.push(name); } return i; };
const byLogicalName = new Map(man.logical_animations.map((a) => [a.name, a]));
for (const n of LEGACY_ORDER) if (!byLogicalName.has(n)) fail(`${rel(MANIFEST)}: no logical animation ${n} (a stage-1 animation)`);
const ordered = [...LEGACY_ORDER.map((n) => byLogicalName.get(n)), ...man.logical_animations.filter((a) => !LEGACY_ORDER.includes(a.name))];
const anims = [];
const variantNames = new Map();
for (const a of ordered) {
  const p = a.play || {};
  const stage1 = p.mode !== null && p.mode !== undefined;
  const two = a.class === 'BlendSpace';
  const samples = a.samples ? a.samples.map((s) => [s.value, seqIndex(s.animation_name), s.value_y]) : [[0, seqIndex(path.basename(a.file, '.glb'))]];
  if (!two) samples.sort((x, y) => x[0] - y[0]);
  const lengths = samples.map((s) => items.get(seqNames[s[1]]).duration_s);
  const differ = Math.max(...lengths) - Math.min(...lengths) > 1e-4;
  if (differ && stage1 && LEGACY_ORDER.includes(a.name)) fail(`${a.name}: its blend-space samples differ in length (${lengths.join(', ')}); the stage-1 core plays them on one clock`);
  const enabled = samples.some((s) => items.get(seqNames[s[1]]).root_motion_enabled === true);
  // A Rust identifier for the row: the name itself; a leading digit gets the prefix A ("1301" -> A1301).
  let variant = String(a.name).replace(/[^A-Za-z0-9_]/g, '_');
  if (/^[0-9]/.test(variant)) variant = 'A' + variant;
  if (RESERVED.has(variant) || variant === 'Self') variant += '_';
  if (variantNames.has(variant)) fail(`animation rows ${a.name} and ${variantNames.get(variant)} both become the identifier ${variant}`);
  variantNames.set(variant, a.name);
  anims.push({ name: a.name, variant, stage1, mode: stage1 ? p.mode : null, looping: stage1 && !!p.loop_bit, rootBit: stage1 && !!p.root_motion_bit, rootExtracted: stage1 && !!p.root_motion_extracted, rootEnabled: enabled,
    blendIn: stage1 ? (p.blend_in_s || 0) : (p.asset_m_InterpolationTime || 0), assetBlendIn: p.asset_m_InterpolationTime || 0, length: differ ? Math.max(...lengths) : lengths[0], differ,
    blendSpace: !!a.samples, two, axisMin: a.axis ? a.axis.min : 0, axisMax: a.axis ? a.axis.max : 0, axisSmoothing: (a.input_filter && a.input_filter.window_s) || 0, samples, playedBy: p.played_by || '',
    axisY: a.axis_y || null, axisGrid: a.axis ? a.axis.grid_num : 0, ySmoothing: (a.input_filter && a.input_filter.window_y_s) || 0, grid: a.grid_samples || [], allAnimation: /SQEX_AllAnimation/.test(a.notify_trigger_mode || '') });
}

// ---- notifies: plain values out of the serialized property lists
const plain = (v) => {
  if (Array.isArray(v)) {
    if (v.length && v.every((x) => x && typeof x === 'object' && !Array.isArray(x) && 'name' in x && 'type' in x)) return Object.fromEntries(v.map((x) => [x.name + (x.arrayIndex ? `[${x.arrayIndex}]` : ''), plain(x.value)]));
    return v.map(plain);
  }
  if (v && typeof v === 'object') {
    if (Array.isArray(v.items) && 'structName' in v) return v.items.map(plain);
    if ('s' in v && 'enc' in v) return v.s;
    return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, plain(x)]));
  }
  return v;
};
const rstr = (s) => JSON.stringify(String(s));
const opt = (v, make) => (v === undefined || v === null || v === 'None' ? 'None' : `Some(${make(v)})`);
const label = (v) => String(v).replace(/^.*::/, '');
const num = (v, d) => f32(typeof v === 'number' ? v : d);
const bool = (v, d) => String(typeof v === 'boolean' ? v : d);
const classVariant = (c) => camel(String(c).replace(/_/g, ' ')).replace(/^Sqexsead/, 'SqexSead');
// class -> (properties, consumed set) -> Rust expression of NotifyData. A property the decoder
// reads is "consumed"; the rest stays in the raw bag (`props`). The effect-attach lists are decoded
// AND kept raw (the decoder takes only what the seam needs).
const assetNames = new Set();
const asset = (v) => { assetNames.add(String(v)); return rstr(v); };
const DECODE = {
  TresAnimNotifyState_Attack: (p, used) => { ['CollisionGrpName', 'm_AttackDataIDName', 'm_WeaponHitSEAliasID', 'AttackInterval', 'm_HitSEAsset', 'm_bOverrideHitEffect', 'm_HitEffectGrpName'].forEach((k) => used.add(k));
    return `NotifyData::Attack { group: ${rstr(p.CollisionGrpName ?? 'None')}, row: ${rstr(p.m_AttackDataIDName ?? 'None')}, hit_se: ${opt(p.m_WeaponHitSEAliasID, (v) => rstr(label(v)))}, interval: ${opt(p.AttackInterval, f32)}, hit_se_asset: ${opt(p.m_HitSEAsset, asset)}, override_hit_effect: ${bool(p.m_bOverrideHitEffect, false)}, hit_effect_group: ${opt(p.m_HitEffectGrpName, rstr)} }`; },
  TresAnimNotifyState_IK: (p, used) => { ['m_StartInterpolationTime', 'm_EndInterpolationTime', 'm_TargetAlpha'].forEach((k) => used.add(k));
    return `NotifyData::Ik { start_interpolation_time: ${num(p.m_StartInterpolationTime, 0)}, end_interpolation_time: ${num(p.m_EndInterpolationTime, 0)}, target_alpha: ${num(p.m_TargetAlpha, 1)} }`; },
  TresAnimNotify_Timing: (p, used) => { used.add('m_Param'); return `NotifyData::Param { param: ${opt(p.m_Param, String)} }`; },
  TresAnimNotifyState_BpEvent: (p, used) => { used.add('m_Param'); return `NotifyData::Param { param: ${opt(p.m_Param, String)} }`; },
  TresAnimNotify_Shot: (p, used) => { ['LocationIndex', 'm_bAttachFireEffect'].forEach((k) => used.add(k)); return `NotifyData::Shot { location_index: ${opt(p.LocationIndex, String)}, attach_fire_effect: ${bool(p.m_bAttachFireEffect, false)} }`; },
  TresAnimNotify_LandAnimPose: (p, used) => { used.add('m_LandAnimPose'); return `NotifyData::LandAnimPose { pose: ${opt(p.m_LandAnimPose, (v) => rstr(label(v)))} }`; },
  TresAnimNotify_LastAnimPose: (p, used) => { used.add('m_LastAnimPose'); return `NotifyData::LastAnimPose { pose: ${opt(p.m_LastAnimPose, (v) => rstr(label(v)))} }`; },
  TresAnimNotify_PlayVoice: (p, used) => voice(p, used),
  TresAnimNotifyState_PlayVoice: (p, used) => voice(p, used),
  SQEXSEADAnimNotify_PlaySound: (p, used) => sound(p, used),
  SQEXSEADAnimNotifyState_AttachSound: (p, used) => sound(p, used),
  SQEXSEADAnimNotify_PlayAutoSe: (p, used) => autoSe(p, used),
  SQEXSEADAnimNotifyState_PlayAutoSe: (p, used) => autoSe(p, used),
  TresAnimNotify_AttachEffect: (p) => effect(p),
  TresAnimNotifyState_AttachEffect: (p) => effect(p),
  TresAnimNotify_FaceAnim: (p, used) => { ['IsLoop', 'FaceAnimType', 'FaceAnimName', 'SameCheck', 'BlendInTime', 'm_bIsBattleFaceEnd'].forEach((k) => used.add(k));
    return `NotifyData::Face { slot: 0, anim_type: ${opt(p.FaceAnimType, (v) => rstr(label(v)))}, anim_name: ${opt(p.FaceAnimName, rstr)}, blend_in: ${num(p.BlendInTime, -1)}, looped: ${bool(p.IsLoop, false)}, same_check: ${bool(p.SameCheck, true)}, battle_face_end: ${bool(p.m_bIsBattleFaceEnd, false)} }`; },
  TresAnimNotify_FaceAnimEye: (p, used) => face(p, used, 1),
  TresAnimNotify_FaceAnimLip: (p, used) => face(p, used, 2),
};
for (const c of ['TresAnimNotify_PlayWeaponSwingSE', 'TresAnimNotifyState_PlayWeaponSwingSE', 'TresAnimNotify_PlayWeaponHitSE']) DECODE[c] = (p, used) => {
  ['m_SoundAliasID', 'm_SoundAliasUnit', 'm_SocketName', 'm_bFollow', 'm_VolumeMultiplier', 'm_PitchMultiplier', 'm_FadeOutDuration'].forEach((k) => used.add(k));
  return `NotifyData::WeaponSe { hit: ${/Hit/.test(c)}, alias: ${rstr(label(p.m_SoundAliasID ?? 'None'))}, unit: ${rstr(label(p.m_SoundAliasUnit ?? 'None'))}, socket: ${opt(p.m_SocketName, rstr)}, follow: ${bool(p.m_bFollow, false)}, volume: ${num(p.m_VolumeMultiplier, 1)}, pitch: ${num(p.m_PitchMultiplier, 1)}, fade_out: ${num(p.m_FadeOutDuration, 0)} }`;
};
function face(p, used, slot) {
  ['m_bIsLoop', 'm_AnimType', 'm_AnimName', 'm_bSameCheck', 'm_BlendInTime'].forEach((k) => used.add(k));
  return `NotifyData::Face { slot: ${slot}, anim_type: ${opt(p.m_AnimType, (v) => rstr(label(v)))}, anim_name: ${opt(p.m_AnimName, rstr)}, blend_in: ${num(p.m_BlendInTime, -1)}, looped: ${bool(p.m_bIsLoop, false)}, same_check: ${bool(p.m_bSameCheck, true)}, battle_face_end: false }`;
}
function voice(p, used) {
  ['m_RandomVoiceAssets', 'm_VoiceGroupName', 'm_VoiceAsset', 'FadeOutTime'].forEach((k) => used.add(k));
  const random = (plain(p.m_RandomVoiceAssets) || []).filter((r) => r && r.Voice);
  return `NotifyData::Voice { group: ${opt(p.m_VoiceGroupName, rstr)}, asset: ${opt(p.m_VoiceAsset, asset)}, random: &[${random.map((r) => `(${asset(r.Voice)}, ${Number.isInteger(r.RandomWeights) ? r.RandomWeights : 0})`).join(', ')}], fade_out: ${num(p.FadeOutTime, 0)} }`;
}
function sound(p, used) {
  ['Sound', 'AttachPointName', 'VolumeMultiplier', 'PitchMultiplier', 'FadeInDuration', 'FadeOutDuration', 'LoopEndSound', 'LoopEndSoundPitchMultiplier', 'bFollowOwnerVisible'].forEach((k) => used.add(k));
  return `NotifyData::Sound { sound: ${p.Sound ? asset(p.Sound) : '""'}, attach_point: ${opt(p.AttachPointName, rstr)}, volume: ${num(p.VolumeMultiplier, 1)}, pitch: ${num(p.PitchMultiplier, 1)}, fade_in: ${num(p.FadeInDuration, 0)}, fade_out: ${num(p.FadeOutDuration, 0)}, loop_end_sound: ${opt(p.LoopEndSound, asset)}, loop_end_pitch: ${num(p.LoopEndSoundPitchMultiplier, 1)}, follow_owner_visible: ${bool(p.bFollowOwnerVisible, false)} }`;
}
function autoSe(p, used) {
  used.add('FadeOutDuration');
  const s = plain(p.PlaySettings) || {};
  return `NotifyData::AutoSe { motion: ${opt(s.MotionSoundType, (v) => rstr(label(v)))}, source: ${opt(s.SourceType, (v) => rstr(label(v)))}, fade_out: ${num(p.FadeOutDuration, 0)} }`;
}
function effect(p) {
  const entries = [];
  for (const g of plain(p.MetaAttachDataList) || []) {
    for (const m of g.MetaAttachData || []) {
      for (const e of m.EffectDataList || []) entries.push(`EffectAttach { effect: ${opt(e.AttachEffect, asset)}, attach_type: ${rstr(label(g.AttachType ?? 'None'))}, socket: ${rstr(m.SocketName && m.SocketName !== 'None' ? m.SocketName : (g.SocketName ?? 'None'))}, group_id: ${Number.isInteger(e.GroupID) ? e.GroupID : 0}, common: ${bool(m.bUseCommonEffect, false)} }`);
    }
  }
  for (const g of plain(p.AttachDataList) || []) {
    const datas = (g.AttachDatas || []).filter((x) => typeof x === 'string');
    for (const d of datas.length ? datas : [null]) entries.push(`EffectAttach { effect: ${opt(d, asset)}, attach_type: ${rstr(label(g.AttachType ?? 'None'))}, socket: ${rstr(g.SocketName ?? 'None')}, group_id: ${Number.isInteger(g.GroupID) ? g.GroupID : 0}, common: ${bool(g.bUseCommonAttachData, false)} }`);
  }
  return `NotifyData::Effect { entries: &[${entries.join(', ')}] }`;
}
const classes = new Map(); // game class name -> { variant, count, states }
const seqNotifies = new Map();
let notifyCount = 0;
// A notify's trigger time: its link time plus TriggerTimeOffset (pack 1.3.0 gives both).
const trig = (n) => (n.trigger_time !== undefined ? n.trigger_time : n.time);
const endTrig = (n) => (n.end_trigger_time !== undefined ? n.end_trigger_time : n.time + (n.duration || 0));
const notify = (item, name) => (item.notifies || []).find((n) => n.name === name);
for (const name of seqNames) {
  const out = [];
  for (const n of items.get(name).notifies || []) {
    if (!n.class) continue; // an event without a notify object does nothing
    if (!classes.has(n.class)) classes.set(n.class, { variant: classVariant(n.class), count: 0, state: false, instant: false });
    const c = classes.get(n.class);
    c.count++;
    if (n.kind === 'state') c.state = true; else c.instant = true;
    const props = n.properties || {};
    const used = new Set();
    const data = DECODE[n.class] ? DECODE[n.class](props, used) : 'NotifyData::None';
    const bag = Object.entries(props).filter(([k]) => !used.has(k)).map(([k, v]) => `(${rstr(k)}, ${rstr(JSON.stringify(plain(v)))})`);
    const state = n.kind === 'state';
    out.push(`NotifyDef { class: NotifyClass::${c.variant}, state: ${state}, time: ${f32(trig(n))}, end: ${f32(state ? endTrig(n) : trig(n))}, data: ${data}, props: &[${bag.join(', ')}] }`);
    notifyCount++;
  }
  seqNotifies.set(name, out);
}
{ const seen = new Map(); for (const [game, c] of classes) { if (seen.has(c.variant)) fail(`notify classes ${game} and ${seen.get(c.variant)} both become ${c.variant}`); seen.set(c.variant, game); } }
const classList = [...classes].sort((a, b) => (a[0] < b[0] ? -1 : 1));

let anim = banner('port/sora-assets (build/sora_pack/manifest.json: logical_animations, items[].notifies, root_motion.tracks)');
anim += `\n/// The pack generator the tables were read from.\npub const SORA_ANIM_SOURCE: &str = ${lit(man.generator || man.format || 'unknown')};\n\n`;
anim += '/// One frame of a root-motion track: seconds; translation since frame 0 in actor space at frame 0\n/// (cm: x forward, y right, z up); yaw since frame 0 (degrees, positive = turning right).\n#[derive(Clone, Copy, Debug, PartialEq)]\npub struct RootKey {\n    pub time: f32,\n    pub x: f32,\n    pub y: f32,\n    pub z: f32,\n    pub yaw: f32,\n}\n\n';
anim += `/// The notify classes that occur in the packed sequences (${classList.length}), by the game's class names without\n/// their underscores. \`game_name()\` is the class name as the asset has it.\n#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]\npub enum NotifyClass {\n`;
for (const [game, c] of classList) anim += `    /// \`${game}\`: ${c.count} instance(s)${c.state && c.instant ? ' (as a state and as an instant notify)' : c.state ? ' (a notify state: begin / tick / end)' : ''}.\n    ${c.variant},\n`;
anim += `}\n\nimpl NotifyClass {\n    pub const ALL: &'static [NotifyClass] = &[${classList.map(([, c]) => `NotifyClass::${c.variant}`).join(', ')}];\n\n    /// The class name in the game's assets.\n    pub const fn game_name(self) -> &'static str {\n        match self {\n`;
for (const [game, c] of classList) anim += `            NotifyClass::${c.variant} => ${lit(game)},\n`;
anim += '        }\n    }\n}\n\n';
anim += `/// One effect of a \`TresAnimNotify(State)_AttachEffect\`: the effect asset (None: the entry names common attach\n/// data by group id only), what it is attached to and at which socket. The full serialized lists are in the\n/// notify's \`props\`.\n#[derive(Clone, Copy, Debug, PartialEq)]\npub struct EffectAttach {\n    pub effect: Option<&'static str>,\n    /// \`ESQEX_ATTACH_OBJECT_NAME\` (SQEX_ATTACH_OBJECT_OWNER, SQEX_ATTACH_OBJECT_ATTACH_TO_WEAPON ...).\n    pub attach_type: &'static str,\n    pub socket: &'static str,\n    pub group_id: i32,\n    pub common: bool,\n}\n\n`;
anim += `/// The decoded properties of a notify, for the classes a specification gives a meaning to. Values the asset
/// does not serialize are the class defaults: 0 / false / None unless stated (volume and pitch multipliers 1,
/// face blend-in -1 = the clip's own, face same-check true, leg-IK target alpha 1: docs/sora-animation-spec.md
/// 10, 12; the sound multipliers' default 1 is the stock engine's, not re-read in the exe). Everything the
/// decoder did not read is in \`NotifyDef::props\`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NotifyData {
    /// Nothing decoded: a flag notify (TresEnableChange, TresEnableInput, TresTraction, TresFaceAt,
    /// TresJumpStart, TresReverseLeg ...) or a class whose properties are only in \`props\`.
    None,
    /// \`TresAnimNotifyState_Attack\` ("TresAtk"): a hit window (docs/sora-combat-spec.md 2, 8).
    Attack { group: &'static str, row: &'static str, hit_se: Option<&'static str>, interval: Option<f32>, hit_se_asset: Option<&'static str>, override_hit_effect: bool, hit_effect_group: Option<&'static str> },
    /// \`TresAnimNotifyState_IK\`: a leg-IK window.
    Ik { start_interpolation_time: f32, end_interpolation_time: f32, target_alpha: f32 },
    /// \`TresAnimNotify_Timing\`, \`TresAnimNotifyState_BpEvent\`: \`m_Param\`.
    Param { param: Option<i32> },
    /// \`TresAnimNotify_Shot\`.
    Shot { location_index: Option<i32>, attach_fire_effect: bool },
    /// \`TresAnimNotify_LandAnimPose\` (\`ETresLandAnimPose\` without its prefix path).
    LandAnimPose { pose: Option<&'static str> },
    /// \`TresAnimNotify_LastAnimPose\`.
    LastAnimPose { pose: Option<&'static str> },
    /// \`TresAnimNotify(State)_PlayWeaponSwingSE\` / \`_PlayWeaponHitSE\`: a sound alias of the weapon's set
    /// (SWING1.., HIT1..) on a unit (RH_WEAPON).
    WeaponSe { hit: bool, alias: &'static str, unit: &'static str, socket: Option<&'static str>, follow: bool, volume: f32, pitch: f32, fade_out: f32 },
    /// \`SQEXSEADAnimNotify_PlaySound\` / \`SQEXSEADAnimNotifyState_AttachSound\`: a SEAD sound asset.
    Sound { sound: &'static str, attach_point: Option<&'static str>, volume: f32, pitch: f32, fade_in: f32, fade_out: f32, loop_end_sound: Option<&'static str>, loop_end_pitch: f32, follow_owner_visible: bool },
    /// \`SQEXSEADAnimNotify(State)_PlayAutoSe\`: an automatic (surface-dependent) motion sound.
    AutoSe { motion: Option<&'static str>, source: Option<&'static str>, fade_out: f32 },
    /// \`TresAnimNotify(State)_PlayVoice\`: a voice group, one asset, or a weighted random list.
    Voice { group: Option<&'static str>, asset: Option<&'static str>, random: &'static [(&'static str, i32)], fade_out: f32 },
    /// \`TresAnimNotify_FaceAnim\` (slot 0: both face slots), \`_FaceAnimEye\` (1), \`_FaceAnimLip\` (2).
    Face { slot: u8, anim_type: Option<&'static str>, anim_name: Option<&'static str>, blend_in: f32, looped: bool, same_check: bool, battle_face_end: bool },
    /// \`TresAnimNotify(State)_AttachEffect\`.
    Effect { entries: &'static [EffectAttach] },
}

/// One notify of a sequence. \`time\` is the trigger time (link time + TriggerTimeOffset), \`end\` the end
/// trigger of a notify state (= \`time\` for an instant notify), seconds, at full precision.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NotifyDef {
    pub class: NotifyClass,
    pub state: bool,
    pub time: f32,
    pub end: f32,
    pub data: NotifyData,
    /// The serialized properties the decoder did not read: (name, value as JSON text).
    pub props: &'static [(&'static str, &'static str)],
}

`;
anim += '/// One animation sequence of the pack. The three legacy notify times are seconds; -1.0 = the sequence has no such notify.\n#[derive(Clone, Copy, Debug, PartialEq)]\npub struct SequenceData {\n    /// Asset name, which is also the file name in the pack: `anims/<name>.glb`.\n    pub name: &\'static str,\n    pub length: f32,\n    /// `NumFrames`.\n    pub frames: u32,\n    /// `bEnableRootMotion`.\n    pub root_motion_enabled: bool,\n    /// `m_InterpolationTime`: the default blend-in of a play call.\n    pub interpolation_time: f32,\n    /// `TresJumpStart`: the launch impulse.\n    pub jump_start: f32,\n    /// `TresEnableChange`: from here the state may be left.\n    pub enable_change: f32,\n    /// `TresReverseLeg` notify state: begin and end.\n    pub reverse_leg: (f32, f32),\n    /// Per-frame root track; empty when the sequence has no root motion (`bEnableRootMotion` false).\n    pub root: &\'static [RootKey],\n    /// Every notify, ordered by time, then by the asset\'s order.\n    pub notifies: &\'static [NotifyDef],\n    /// Clips with a hit window only: the sockets of the keyblade per animation frame (frame i is at\n    /// i * length / (frames - 1)), ACTOR space, cm: [ATK_00 x, y, z, ATK_01 x, y, z] (x forward, y right, z up).\n    pub sockets: &\'static [[f32; 6]],\n}\n\n';
anim += '/// A 2D blend space (`UBlendSpace`): the grid the asset stores. Grid point (ix, iy) is entry\n/// `ix * (y_grid + 1) + iy`; each entry lists (index into `samples`, weight).\n#[derive(Clone, Copy, Debug, PartialEq)]\npub struct BlendSpace2d {\n    pub x_min: f32,\n    pub x_max: f32,\n    pub x_grid: usize,\n    pub y_min: f32,\n    pub y_max: f32,\n    pub y_grid: usize,\n    pub y_smoothing: f32,\n    /// (x, y, index into `SEQUENCES`), in the asset\'s order.\n    pub samples: &\'static [(f32, f32, usize)],\n    pub grid: &\'static [&\'static [(usize, f32)]],\n}\n\n';
anim += '/// One animation as the states play it by name (a row of Sora\'s TresAnimSet).\n#[derive(Clone, Copy, Debug, PartialEq)]\npub struct AnimData {\n    pub name: &\'static str,\n    /// The mode of the stage-1 states\' play call (bit 0 loop, bit 1 root motion); None for every other\n    /// row: the state that plays it gives the mode (`AnimPlayer::play_with`).\n    pub stage1_mode: Option<u8>,\n    /// Play-call mode bit 0 of the stage-1 call (false for the other rows).\n    pub looping: bool,\n    /// Play-call mode bit 1 (0x14056E920) of the stage-1 call.\n    pub root_motion_bit: bool,\n    /// True when the stage-1 call really extracts root motion: the mode bit and `bEnableRootMotion` on the sequences.\n    pub root_motion: bool,\n    /// `bEnableRootMotion` on at least one of the row\'s sequences: a play with mode bit 1 extracts root motion.\n    pub root_motion_enabled: bool,\n    /// Blend-in of the stage-1 play call, resolved (argument -1 = the asset\'s m_InterpolationTime), seconds;\n    /// for the other rows the asset\'s m_InterpolationTime (what a call with blend-in -1 gets).\n    pub blend_in: f32,\n    pub length: f32,\n    /// The row\'s blend-space samples differ in length (`length` is the longest): the one-clock playback of\n    /// `AnimPlayer` does not cover it (TurnN, RunStartB, RailSlideLoop, BallRide).\n    pub sample_lengths_differ: bool,\n    pub blend_space: bool,\n    pub axis_min: f32,\n    pub axis_max: f32,\n    /// The blend space\'s own interpolation time for its axis input, seconds.\n    pub axis_smoothing: f32,\n    /// `NotifyTriggerMode` SQEX_AllAnimation: every sample\'s notifies fire, whatever its weight.\n    pub all_animation: bool,\n    /// (axis value, index into `SEQUENCES`): ascending for a 1D blend space, one entry for a plain\n    /// sequence, the asset\'s order (x values) for a 2D blend space.\n    pub samples: &\'static [(f32, usize)],\n    /// The second axis and the grid of a 2D blend space.\n    pub space2d: Option<&\'static BlendSpace2d>,\n}\n\n';
const rootTrack = (name, it) => {
  const tr = tracks[name] && tracks[name].bEnableRootMotion ? tracks[name].frames_data : [];
  return tr.map((k, i) => `RootKey { time: ${f32(i === tr.length - 1 ? it.duration_s : k.time)}, x: ${f32(k.translation_ue_actor_cm[0])}, y: ${f32(k.translation_ue_actor_cm[1])}, z: ${f32(k.translation_ue_actor_cm[2])}, yaw: ${f32(k.yaw_ue_deg)} }`).join(', ');
};
anim += 'pub static SEQUENCES: &[SequenceData] = &[\n';
for (const name of seqNames) {
  const it = items.get(name);
  const js = notify(it, 'TresJumpStart'), ec = notify(it, 'TresEnableChange'), rl = notify(it, 'TresReverseLeg');
  const sp = it.sequence_properties || {};
  anim += `    SequenceData { name: ${lit(name)}, length: ${f32(it.duration_s)}, frames: ${it.num_frames || 0}, root_motion_enabled: ${it.root_motion_enabled === true}, interpolation_time: ${f32(sp.m_InterpolationTime || 0)}, jump_start: ${js ? f32(trig(js)) : '-1.0'}, enable_change: ${ec ? f32(trig(ec)) : '-1.0'}, reverse_leg: ${rl ? `(${f32(trig(rl))}, ${f32(rl.end_trigger_time !== undefined ? rl.end_trigger_time : rl.time + rl.duration)})` : '(-1.0, -1.0)'}, root: &[${rootTrack(name, it)}],\n        notifies: &[\n`;
  for (const n of seqNotifies.get(name)) anim += `            ${n},\n`;
  const sk = ((man.attack_sockets || {}).clips || {})[name];
  anim += `        ], sockets: &[${sk ? sk.sockets.map((r) => `[${r.map(f32).join(', ')}]`).join(', ') : ''}] },\n`;
}
anim += '];\n\n';
const spaces = anims.filter((a) => a.two);
spaces.forEach((a, k) => {
  anim += `static SPACE2D_${k}: BlendSpace2d = BlendSpace2d { x_min: ${f32(a.axisMin)}, x_max: ${f32(a.axisMax)}, x_grid: ${a.axisGrid}, y_min: ${f32(a.axisY.min)}, y_max: ${f32(a.axisY.max)}, y_grid: ${a.axisY.grid_num}, y_smoothing: ${f32(a.ySmoothing)}, samples: &[${a.samples.map((s) => `(${f32(s[0])}, ${f32(s[2])}, ${s[1]})`).join(', ')}], grid: &[${a.grid.map((g) => `&[${g.map((e) => `(${e.sample_index}, ${f32(e.weight)})`).join(', ')}]`).join(', ')}] };\n`;
  a.space = k;
});
anim += '\n/// Every row, in the order of `AnimId` (the row of an id is `ANIMS[id as usize]`).\npub static ANIMS: &[AnimData] = &[\n';
for (const a of anims) {
  if (a.stage1) anim += `    // mode ${a.mode}; ${a.playedBy}\n`;
  anim += `    AnimData { name: ${lit(a.name)}, stage1_mode: ${a.stage1 ? `Some(${a.mode})` : 'None'}, looping: ${a.looping}, root_motion_bit: ${a.rootBit}, root_motion: ${a.rootExtracted}, root_motion_enabled: ${a.rootEnabled}, blend_in: ${f32(a.blendIn)}, length: ${f32(a.length)}, sample_lengths_differ: ${a.differ}, blend_space: ${a.blendSpace}, axis_min: ${f32(a.axisMin)}, axis_max: ${f32(a.axisMax)}, axis_smoothing: ${f32(a.axisSmoothing)}, all_animation: ${a.allAnimation}, samples: &[${a.samples.map((s) => `(${f32(s[0])}, ${s[1]})`).join(', ')}], space2d: ${a.two ? `Some(&SPACE2D_${a.space})` : 'None'} },\n`;
}
anim += '];\n\n';
anim += `/// The animations of Sora's main anim set (\`p_ex001_AnimSet\`), by the game's own row names (a name that starts\n/// with a digit has the prefix \`A\`: row "1301" is \`AnimId::A1301\`). The first ${LEGACY_ORDER.length} values are the stage-1\n/// animations in their historical order (pinned by tests/golden.rs); the rest follow in the anim set's order.\n#[allow(non_camel_case_types)]\n#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]\n#[repr(u16)]\npub enum AnimId {\n`;
anims.forEach((a, i) => { anim += `    ${a.variant} = ${i},\n`; });
anim += `}\n\nimpl AnimId {\n    /// Every animation, in id order.\n    pub const ALL: [AnimId; ${anims.length}] = [${anims.map((a) => `AnimId::${a.variant}`).join(', ')}];\n\n    /// The stage-1 animations (the first ${LEGACY_ORDER.length} ids).\n    pub const STAGE1: [AnimId; ${LEGACY_ORDER.length}] = [${anims.slice(0, LEGACY_ORDER.length).map((a) => `AnimId::${a.variant}`).join(', ')}];\n\n    /// By the game's row name ("IdleN", "1301", "M_1359").\n    pub fn from_name(name: &str) -> Option<AnimId> {\n        ANIMS.iter().position(|a| a.name == name).map(|i| AnimId::ALL[i])\n    }\n}\n\n`;
{ const v = (man.attack_sockets || {}).vfx_cure || { position_actor_cm: [0, 0, 105], plus_200_x_actor_cm: [200, 0, 105] };
  anim += `/// The socket vfx_cure of the wielder (root bone, reference pose) in actor space, cm, and the same moved by the\n/// relative location (200, 0, 0) of the shape groups FrontImpactS and KB_FixPillar, in the socket frame.\npub const SOCKET_VFX_CURE: [f32; 3] = [${v.position_actor_cm.map(f32).join(', ')}];\npub const SOCKET_VFX_CURE_PLUS_200: [f32; 3] = [${v.plus_200_x_actor_cm.map(f32).join(', ')}];\n\n`; }
const assetList = [...assetNames].sort();
anim += `/// Every sound, voice and effect asset a notify names (sorted): the id the generic notify handlers give a seam\n/// event is \`NOTIFY_ID_BASE\` + the index of its name here, until the audio and effect tables are generated.\npub static NOTIFY_ASSETS: &[&str] = &[\n${assetList.map((n) => `    ${rstr(n)},\n`).join('')}];\n`;
claim('file', 'sora_anim.rs', 'the pack manifest');
outputs.push(['sora_anim.rs', anim, `${anims.length} animations (${anims.filter((a) => a.blendSpace && !a.two).length} BlendSpace1D, ${spaces.length} 2D), ${seqNames.length} sequences, ${seqNames.filter((n) => tracks[n] && tracks[n].bEnableRootMotion).length} root tracks, ${notifyCount} notifies of ${classList.length} classes, ${assetList.length} notify assets`]);

// ================================================================ write
let differs = 0;
for (const [name, body, what] of outputs) {
  const file = path.join(OUT_DIR, name);
  const old = fs.existsSync(file) ? fs.readFileSync(file, 'utf8') : null;
  if (CHECK) {
    if (old !== body) { differs++; console.log(`gen_port: --check: ${rel(file)} differs from what its input gives (run node tools/gen_port.js)`); } else console.log(`gen_port: --check: ${rel(file)} is current: ${what}`);
  } else {
    // Unchanged output keeps its timestamp, so cargo does not rebuild kh-core for nothing.
    if (old !== body) { fs.mkdirSync(OUT_DIR, { recursive: true }); fs.writeFileSync(file, body); }
    console.log(`gen_port: ${rel(file)} ${old === body ? 'unchanged' : 'written'}: ${what}`);
  }
}
// A sheet that is not generated any more must not leave an old file behind for a crate to include.
for (const name of stale) {
  const file = path.join(OUT_DIR, name);
  if (!fs.existsSync(file) || outputs.some((o) => o[0] === name)) continue;
  if (CHECK) { differs++; console.log(`gen_port: --check: ${rel(file)} is stale (its sheet cannot be generated)`); } else { fs.unlinkSync(file); console.log(`gen_port: ${rel(file)} deleted (stale: its sheet cannot be generated)`); }
}
process.exit(differs ? 1 : 0);
