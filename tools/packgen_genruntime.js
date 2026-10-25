#!/usr/bin/env node
// (PACKGEN) THE GENERATED TABLES MADE FROM KH3 DATA, LOADED AT RUN TIME instead of compiled into the DLL (the prebuilt
// release DLL carries no KH3 table).
//
//   node tools/packgen_genruntime.js [--in DIR] [--out DIR] [--data DIR] [--check]
//     --in    the generated Rust tables (default build/gen/rust: tools/gen_port.js, gen_party.js, party_sheet.js,
//             party_team_pack.js)
//     --out   the run-time version of those sources (default build/gen/rust_runtime): the files below rewritten, every
//             other file copied as it is; plus khg_schemas.rs (what the DLL expects)
//     --data  where the data files go (default build/sora_pack/gen: the pack, which the DLL finds through kh-er.cfg)
//
// For each file of RUNTIME_FILES every `pub static NAME: TYPE = <literal>;` becomes
//   pub static NAME: kh_gendata::Gen<TYPE> = kh_gendata::Gen::new("<file>", "<schema>", <index>, |r| <decoder>);
// The decoder is made from the literal's own SHAPE (struct and enum names, field names, nesting: our code's types) with
// every leaf (number, string, bool) and every unit-enum value read from the data file <file>.khg; Rust infers each
// leaf's type from the field it fills, so this tool needs no type information. Types, enums, consts and impl blocks stay
// as they are (names and code: ours). The schema (a hash of the decoders) is in both the DLL and the data file, so a DLL
// and a pack made from different data refuse each other with a clear message (kh-gendata).
// Deterministic: the same inputs give the same bytes.
'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const ROOT = path.resolve(__dirname, '..');
const args = process.argv.slice(2);
const opt = (k, d) => { const i = args.indexOf(k); return i >= 0 ? path.resolve(args[i + 1]) : d; };
const IN = opt('--in', path.join(ROOT, 'build', 'gen', 'rust'));
const OUT = opt('--out', path.join(ROOT, 'build', 'gen', 'rust_runtime'));
const DATA = opt('--data', path.join(ROOT, 'build', 'sora_pack', 'gen'));
const CHECK = args.includes('--check');
// the tables made from the pack (KH3 animation, camera and clip data); every other generated file comes from our sheets
const RUNTIME_FILES = ['sora_anim.rs', 'party_anim_n_ex001.rs', 'party_anim_n_ex002.rs', 'party_clips_n_ex001.rs', 'party_clips_n_ex002.rs', 'party_team_cameras.rs', 'party_team_gshoot.rs', 'party_data.rs'];

// ---------------------------------------------------------------- tokens
function tokenize(src, start, end) {
  const toks = []; let i = start;
  const isId = (c) => /[A-Za-z0-9_]/.test(c);
  while (i < end) {
    const c = src[i];
    if (c === ' ' || c === '\n' || c === '\r' || c === '\t') { i++; continue; }
    if (c === '/' && src[i + 1] === '/') { while (i < end && src[i] !== '\n') i++; continue; }
    if (c === '/' && src[i + 1] === '*') { const j = src.indexOf('*/', i + 2); i = j < 0 ? end : j + 2; continue; }
    if (c === '"') {
      let j = i + 1, s = '';
      while (j < end && src[j] !== '"') {
        if (src[j] === '\\') {
          const e = src[j + 1];
          if (e === 'n') { s += '\n'; j += 2; } else if (e === 't') { s += '\t'; j += 2; } else if (e === 'r') { s += '\r'; j += 2; } else if (e === '0') { s += '\0'; j += 2; }
          else if (e === '\\' || e === '"' || e === '\'') { s += e; j += 2; }
          else if (e === 'x') { s += String.fromCharCode(parseInt(src.slice(j + 2, j + 4), 16)); j += 4; }
          else if (e === 'u') { const k = src.indexOf('}', j); s += String.fromCodePoint(parseInt(src.slice(j + 3, k), 16)); j = k + 1; }
          else if (e === '\n' || e === '\r') { j += 2; while (/\s/.test(src[j])) j++; } // line continuation
          else throw new Error('string escape \\' + e);
        } else { s += src[j]; j++; }
      }
      toks.push({ t: 'str', v: s }); i = j + 1; continue;
    }
    if (/[0-9]/.test(c)) {
      let j = i; while (j < end && /[0-9A-Za-z_.]/.test(src[j]) && !(src[j] === '.' && src[j + 1] === '.')) { if ((src[j] === 'e' || src[j] === 'E') && (src[j + 1] === '-' || src[j + 1] === '+')) j += 2; else j++; }
      toks.push({ t: 'num', v: src.slice(i, j) }); i = j; continue;
    }
    if (isId(c)) { let j = i; while (j < end && isId(src[j])) j++; toks.push({ t: 'id', v: src.slice(i, j) }); i = j; continue; }
    if (c === ':' && src[i + 1] === ':') { toks.push({ t: 'p', v: '::' }); i += 2; continue; }
    toks.push({ t: 'p', v: c }); i++;
  }
  return toks;
}

// ---------------------------------------------------------------- the literal grammar of the generators
// node: { k: 'num'|'str'|'bool'|'path'|'struct'|'call'|'ref'|'arr'|'tup', ... }
function parseExpr(toks) {
  let p = 0;
  const peek = () => toks[p], next = () => toks[p++];
  const expect = (v) => { const t = next(); if (!t || t.v !== v) throw new Error(`expected ${v}, got ${t ? t.v : 'end'}`); };
  function expr() {
    const t = peek();
    if (!t) throw new Error('unexpected end');
    if (t.v === '-' ) { next(); const n = next(); if (n.t !== 'num') throw new Error('- without a number'); return { k: 'num', v: '-' + n.v }; }
    if (t.t === 'num') { next(); return { k: 'num', v: t.v }; }
    if (t.t === 'str') { next(); return { k: 'str', v: t.v }; }
    if (t.v === '&') { next(); return { k: 'ref', v: expr() }; }
    if (t.v === '[') { next(); const items = []; while (peek().v !== ']') { items.push(expr()); if (peek().v === ',') next(); } next(); return { k: 'arr', items }; }
    if (t.v === '(') { next(); const items = []; while (peek().v !== ')') { items.push(expr()); if (peek().v === ',') next(); } next(); return items.length === 1 && !toks[p - 2].v.endsWith(',') && toks[p - 2].v !== ',' ? items[0] : { k: 'tup', items }; }
    if (t.t === 'id') {
      if (t.v === 'true' || t.v === 'false') { next(); return { k: 'bool', v: t.v === 'true' }; }
      const segs = [next().v];
      while (peek() && peek().v === '::') { next(); segs.push(next().v); }
      const name = segs.join('::');
      if (peek() && peek().v === '{') {
        next(); const fields = [];
        while (peek().v !== '}') { const f = next().v; let val; if (peek().v === ':') { next(); val = expr(); } else val = { k: 'path', v: f }; fields.push([f, val]); if (peek().v === ',') next(); }
        next(); return { k: 'struct', name, fields };
      }
      if (peek() && peek().v === '(') { next(); const items = []; while (peek().v !== ')') { items.push(expr()); if (peek().v === ',') next(); } next(); return { k: 'call', name, items }; }
      return { k: 'path', v: name };
    }
    throw new Error('unexpected token ' + t.v);
  }
  const e = expr();
  if (p !== toks.length) throw new Error('trailing tokens after the literal');
  return e;
}

// ---------------------------------------------------------------- shapes and sites
// A site is a position the decoder reads one value at. Its values are grouped by shape key; a shape's holes are sub-sites.
// Leaves: num / str / bool, and a unit path inside an enum (Type::Variant: read as an index into the site's list);
// a path naming a static of the file is a reference (structure); other paths (None, consts) are structure.
let STATIC_NAMES = new Set();
const isUnitEnum = (v) => /^[A-Za-z_][A-Za-z0-9_]*::[A-Za-z_][A-Za-z0-9_]*$/.test(v);
function shapeKey(n) {
  switch (n.k) {
    case 'num': return '#n'; case 'str': return '#s'; case 'bool': return '#b';
    case 'path': return isUnitEnum(n.v) && !STATIC_NAMES.has(n.v) ? '#e:' + n.v.split('::')[0] : 'P:' + n.v;
    case 'ref': return n.v.k === 'arr' ? '&[]' : '&' + shapeKey(n.v);
    case 'arr': return '[' + n.items.map(shapeKey).join(',') + ']';
    case 'tup': return '(' + n.items.map(shapeKey).join(',') + ')';
    case 'struct': return n.name + '{' + n.fields.map(([f, v]) => f + ':' + shapeKey(v)).join(',') + '}';
    case 'call': return n.name + '(' + n.items.map(shapeKey).join(',') + ')';
  }
  throw new Error('shape of ' + n.k);
}
class Site {
  constructor() { this.shapes = new Map(); /* key -> { node (first), holes: [Site] } */ this.enums = null; }
  add(n) {
    const key = shapeKey(n);
    if (key.startsWith('#e:')) { (this.enums = this.enums || new Set()).add(n.v); }
    if (!this.shapes.has(key)) this.shapes.set(key, { node: n, holes: [] });
    const sh = this.shapes.get(key);
    let h = 0;
    const visit = (x) => {
      switch (x.k) {
        case 'num': case 'str': case 'bool': return;
        case 'path': return;
        case 'ref': if (x.v.k === 'arr') { const s = sh.holes[h] || (sh.holes[h] = new Site()); h++; for (const it of x.v.items) s.add(it); return; } return visit(x.v);
        case 'arr': case 'tup': case 'call': for (const it of x.items) visitChild(it); return;
        case 'struct': for (const [, v] of x.fields) visitChild(v); return;
      }
    };
    // a child that is itself a leaf / enum is read in place; anything else gets its own site (it may vary in shape)
    const visitChild = (c) => {
      if (c.k === 'num' || c.k === 'str' || c.k === 'bool') return;
      if (c.k === 'path') { if (isUnitEnum(c.v) && !STATIC_NAMES.has(c.v)) { const s = sh.holes[h] || (sh.holes[h] = new Site()); h++; s.add(c); } return; }
      const s = sh.holes[h] || (sh.holes[h] = new Site()); h++; s.add(c);
    };
    if (key.startsWith('#')) return; // a leaf site
    if (n.k === 'ref' && n.v.k === 'arr') { visit(n); return; }
    visit(n);
  }
}
// ---- encoding (one byte stream per static)
class W {
  constructor() { this.parts = []; this.len = 0; }
  bytes(b) { this.parts.push(b); this.len += b.length; }
  varint(x) { const a = []; do { let b = x & 0x7f; x = Math.floor(x / 128); if (x) b |= 0x80; a.push(b); } while (x); this.bytes(Buffer.from(a)); }
  text(s) { const b = Buffer.from(s, 'utf8'); this.varint(b.length); this.bytes(b); }
  out() { return Buffer.concat(this.parts); }
}
function keysOf(site) { return [...site.shapes.keys()]; }
function enc(site, n, w) {
  const key = shapeKey(n);
  const keys = keysOf(site);
  if (keys.length > 1) w.varint(keys.indexOf(key));
  if (key === '#n') return w.text(n.v);
  if (key === '#s') return w.text(n.v);
  if (key === '#b') return w.varint(n.v ? 1 : 0);
  if (key.startsWith('#e:')) return w.varint([...site.enums].sort().indexOf(n.v));
  const sh = site.shapes.get(key);
  let h = 0;
  const visitChild = (c) => {
    if (c.k === 'num') return w.text(c.v);
    if (c.k === 'str') return w.text(c.v);
    if (c.k === 'bool') return w.varint(c.v ? 1 : 0);
    if (c.k === 'path') { if (isUnitEnum(c.v) && !STATIC_NAMES.has(c.v)) { enc(sh.holes[h++], c, w); } return; }
    enc(sh.holes[h++], c, w);
  };
  const visit = (x) => {
    switch (x.k) {
      case 'path': return;
      case 'ref': if (x.v.k === 'arr') { const s = sh.holes[h++]; w.varint(x.v.items.length); for (const it of x.v.items) enc(s, it, w); return; } return visit(x.v);
      case 'arr': case 'tup': case 'call': for (const it of x.items) visitChild(it); return;
      case 'struct': for (const [, v] of x.fields) visitChild(v); return;
    }
  };
  visit(n);
}
// ---- the decoder (Rust)
function dec(site) {
  const keys = keysOf(site);
  if (!keys.length) return 'kh_gendata::bad_index()'; // a slice that is empty in every table: no element is ever read
  const arm = (key) => {
    if (key === '#n') return 'r.num()';
    if (key === '#s') return 'r.str()';
    if (key === '#b') return 'r.boolean()';
    if (key.startsWith('#e:')) { const list = [...site.enums].sort(); return `match r.index() { ${list.map((v, i) => `${i} => ${v}`).join(', ')}, _ => kh_gendata::bad_index() }`; }
    const sh = site.shapes.get(key); let h = 0;
    const child = (c) => {
      if (c.k === 'num') return 'r.num()';
      if (c.k === 'str') return 'r.str()';
      if (c.k === 'bool') return 'r.boolean()';
      if (c.k === 'path') { if (isUnitEnum(c.v) && !STATIC_NAMES.has(c.v)) return dec(sh.holes[h++]); return STATIC_NAMES.has(c.v) ? `*${c.v}` : c.v; }
      return dec(sh.holes[h++]);
    };
    const code = (x) => {
      switch (x.k) {
        case 'path': return STATIC_NAMES.has(x.v) ? `*${x.v}` : x.v;
        case 'ref': if (x.v.k === 'arr') { const s = sh.holes[h++]; const body = dec(s); return `r.slice(|${/\br\./.test(body) ? 'r' : '_r'}| ${body})`; } return '&' + code(x.v);
        case 'arr': return '[' + x.items.map(child).join(', ') + ']';
        case 'tup': return '(' + x.items.map(child).join(', ') + (x.items.length === 1 ? ',' : '') + ')';
        case 'call': return x.name + '(' + x.items.map(child).join(', ') + ')';
        case 'struct': return x.name + ' { ' + x.fields.map(([f, v]) => `${f}: ${child(v)}`).join(', ') + ' }';
      }
      return child(x);
    };
    return code(sh.node);
  };
  if (keys.length === 1) return arm(keys[0]);
  return `match r.index() { ${keys.map((k, i) => `${i} => ${arm(k)}`).join(', ')}, _ => kh_gendata::bad_index() }`;
}

// ---------------------------------------------------------------- one file
function convert(file) {
  const src = fs.readFileSync(path.join(IN, file), 'utf8');
  const base = file.replace(/\.rs$/, '');
  // top-level `pub static NAME: TYPE = ...;`
  const statics = [];
  const re = /^pub static ([A-Z0-9_]+): ([^=\n]+?) = /gm;
  let m;
  while ((m = re.exec(src))) {
    const start = m.index, exprStart = m.index + m[0].length;
    // the literal ends at the `;` at depth 0
    let depth = 0, i = exprStart, inStr = false;
    for (; i < src.length; i++) {
      const c = src[i];
      if (inStr) { if (c === '\\') { i++; continue; } if (c === '"') inStr = false; continue; }
      if (c === '"') { inStr = true; continue; }
      if (c === '/' && src[i + 1] === '/') { while (src[i] !== '\n') i++; continue; }
      if ('([{'.includes(c)) depth++; else if (')]}'.includes(c)) depth--; else if (c === ';' && depth === 0) break;
    }
    statics.push({ name: m[1], type: m[2].trim(), start, exprStart, end: i + 1 });
    re.lastIndex = i;
  }
  STATIC_NAMES = new Set(statics.map((s) => s.name));
  const parts = []; let at = 0; const blobs = []; const decs = [];
  statics.forEach((s, index) => {
    const node = parseExpr(tokenize(src, s.exprStart, s.end - 1));
    const site = new Site(); site.add(node);
    const w = new W(); enc(site, node, w); blobs.push(w.out());
    const d = dec(site); decs.push(s.name + ':' + d);
    parts.push(src.slice(at, s.start));
    parts.push(`pub static ${s.name}: kh_gendata::Gen<${s.type}> = kh_gendata::Gen::new(KHG_FILE, KHG_SCHEMA, ${index}, |${/\br\./.test(d) ? 'r' : '_r'}| ${d});`);
    at = s.end;
  });
  parts.push(src.slice(at));
  const schema = crypto.createHash('sha256').update(base + '\n' + decs.join('\n')).digest('hex').slice(0, 24);
  const head = `// (PACKGEN) RUN-TIME VERSION of ${file} (tools/packgen_genruntime.js): the tables are read from <pack>/gen/${base}.khg at\n// run time (kh-gendata); the decoders below are made from the tables' shapes (our types). DO NOT EDIT.\nconst KHG_FILE: &str = ${JSON.stringify(base)};\nconst KHG_SCHEMA: &str = ${JSON.stringify(schema)};\n`;
  const rs = head + parts.join('');
  // the data file: magic, schema, count, offsets, blobs
  const w = new W();
  w.bytes(Buffer.from('KHGD1\n')); w.text(base); w.text(schema); w.varint(blobs.length);
  for (const b of blobs) w.varint(b.length);
  for (const b of blobs) w.bytes(b);
  return { rs, data: w.out(), schema, statics: statics.length };
}

// ---------------------------------------------------------------- main
if (!fs.existsSync(IN)) { console.error(`${IN}: the generated tables are missing (node tools/packgen.js gen)`); process.exit(1); }
fs.mkdirSync(OUT, { recursive: true }); if (!CHECK) fs.mkdirSync(DATA, { recursive: true });
const schemas = [];
for (const f of fs.readdirSync(IN).filter((x) => x.endsWith('.rs')).sort()) {
  if (!RUNTIME_FILES.includes(f)) { if (!CHECK) fs.copyFileSync(path.join(IN, f), path.join(OUT, f)); continue; }
  const r = convert(f);
  schemas.push([f.replace(/\.rs$/, ''), r.schema]);
  if (!CHECK) { fs.writeFileSync(path.join(OUT, f), r.rs); fs.writeFileSync(path.join(DATA, f.replace(/\.rs$/, '.khg')), r.data); }
  console.log(`packgen_genruntime: ${f}: ${r.statics} table(s) -> gen/${f.replace(/\.rs$/, '.khg')} (${(r.data.length / 1024).toFixed(0)} KB), schema ${r.schema}`);
}
const schemaRs = `// (PACKGEN) the run-time tables this build expects (tools/packgen_genruntime.js): (file, schema).\n&[\n${schemas.map(([f, s]) => `    (${JSON.stringify(f)}, ${JSON.stringify(s)}),`).join('\n')}\n]\n`;
if (!CHECK) { fs.writeFileSync(path.join(OUT, 'khg_schemas.rs'), schemaRs); fs.writeFileSync(path.join(DATA, 'schemas.json'), JSON.stringify({ format: 'kh-gendata-schemas/1', generator: 'tools/packgen_genruntime.js', files: Object.fromEntries(schemas) }, null, 1) + '\n'); }
