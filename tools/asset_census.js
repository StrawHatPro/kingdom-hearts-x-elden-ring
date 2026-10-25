// Census of every file under assets/: container type, and for cooked packages each export's
// class, serial size, bytes consumed by the tagged-property parser and bytes left after it.
//
//   node tools/asset_census.js [--workers N] [--fresh]
//
// Resumable: the sorted file list is cut into fixed chunks; each finished chunk is a file under
// build/pkg_tmp/census/ and is skipped on the next run (--fresh discards them).
// Outputs: audit/asset_census.json (every file), audit/asset_census_summary.json (totals).
// Nothing under assets/ is written.
const fs = require('fs');
const path = require('path');
const { spawn } = require('child_process');
const { ROOT, logRun, writeJson } = require('./lib/log');
const ua = require('./lib/uasset');
const up = require('./lib/uprops');

const VERSION = '1.0.0';
const ASSETS = path.join(ROOT, 'assets');
const TMP = path.join(ROOT, 'build', 'pkg_tmp', 'census');
const LIST = path.join(TMP, 'filelist.json');
const CHUNK = 3000;
const RF_ClassDefaultObject = 0x10;

function parseArgs(argv) {
  const a = { workers: 8, fresh: false };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--workers') a.workers = Math.min(8, Math.max(1, +argv[++i]));
    else if (argv[i] === '--fresh') a.fresh = true;
    else if (argv[i] === '--shard') a.shard = argv[++i].split('/').map(Number);
    else throw new Error('unknown argument ' + argv[i]);
  }
  return a;
}

function walk(dir, relDir, out) {
  for (const d of fs.readdirSync(dir, { withFileTypes: true })) {
    const rel = relDir ? relDir + '/' + d.name : d.name;
    if (d.isDirectory()) walk(path.join(dir, d.name), rel, out);
    else out.push(rel);
  }
}

// Sum of bytes kept as { raw } by the property parser (values it does not interpret).
function rawBytes(v) {
  if (v === null || typeof v !== 'object') return 0;
  if (typeof v.raw === 'string') return v.raw.length / 2;
  let n = 0;
  if (Array.isArray(v)) { for (const x of v) n += rawBytes(x); return n; }
  for (const k in v) { if (k === 'tag' || k === 'innerTag') continue; n += rawBytes(v[k]); }
  return n;
}

// Class of an export as { name, from } where from is the outer package of an imported class,
// or '(this package)' for a class exported by the same package.
function classOf(pkg, idx) {
  if (idx === 0) return { name: '(UClass)', from: '' };
  if (idx > 0) return { name: ua.nameOf(pkg, pkg.exports[idx - 1].objectName), from: '(this package)' };
  const imp = pkg.imports[-idx - 1];
  let outer = imp;
  while (outer.outerIndex < 0) outer = pkg.imports[-outer.outerIndex - 1];
  return { name: ua.nameOf(pkg, imp.objectName), from: outer === imp ? '' : ua.nameOf(pkg, outer.objectName) };
}

function censusPackage(rel, size) {
  const row = { p: rel, t: 'package', size };
  const abs = path.join(ASSETS, rel);
  const head = fs.readFileSync(abs);
  let pkg;
  try { pkg = ua.parseHeader(head); } catch (e) { row.err = 'header: ' + e.message; return row; }
  const base = abs.replace(/\.(uasset|umap)$/, '');
  let uexp = null;
  try { uexp = fs.readFileSync(base + '.uexp'); } catch { /* absent */ }
  row.uexp = uexp ? uexp.length : null;
  const hdr = pkg.summary.totalHeaderSize;
  row.ex = [];
  row.sup = [];
  let covered = 0;
  pkg.exports.forEach((e, i) => {
    const c = classOf(pkg, e.classIndex);
    const cdo = (e.objectFlags & RF_ClassDefaultObject) !== 0;
    const x = [c.name, c.from, e.serialSize, 0, e.serialSize, 0, cdo ? 1 : 0];
    // classes exported here: record the super so blueprint classes resolve to a native base
    if (e.classIndex !== 0 && e.superIndex !== 0 && /Class$/.test(c.name)) {
      const s = classOf(pkg, e.superIndex);
      row.sup.push([ua.nameOf(pkg, e.objectName), s.name, s.from, c.name]);
    }
    if (uexp) {
      const start = e.serialOffset - hdr, end = start + e.serialSize;
      if (start < 0 || end > uexp.length) x.push('export outside .uexp');
      else {
        covered += e.serialSize;
        const r = new ua.Reader(uexp.subarray(start, end));
        try {
          const props = up.readProps(r, pkg);
          x[3] = r.p; x[4] = e.serialSize - r.p; x[5] = rawBytes(props);
          // UObject::Serialize: a non-CDO object has an int32 "has guid" flag after its properties
          if (!cdo && x[4] >= 4) { const g = r.i32(); x.push(g === 0 ? 0 : g === 1 ? 1 : 'guidflag=' + g); }
        } catch (err) { x.push('props: ' + String(err.message).slice(0, 80)); }
      }
    } else x.push('no .uexp');
    row.ex.push(x);
  });
  if (uexp) row.gap = uexp.length - covered - 4; // 0 when the exports tile the .uexp before the 4-byte tag
  row.imp = pkg.summary.importCount;
  row.names = pkg.summary.nameCount;
  return row;
}

function worker(args) {
  const list = JSON.parse(fs.readFileSync(LIST, 'utf8'));
  const [k, n] = args.shard;
  const nChunks = Math.ceil(list.length / CHUNK);
  let done = 0;
  for (let c = 0; c < nChunks; c++) {
    if (c % n !== k) continue;
    const outFile = path.join(TMP, `chunk_${String(c).padStart(5, '0')}.jsonl`);
    if (fs.existsSync(outFile)) continue;
    const lines = [];
    for (let i = c * CHUNK; i < Math.min(list.length, (c + 1) * CHUNK); i++) {
      const rel = list[i];
      let row;
      try {
        const st = fs.statSync(path.join(ASSETS, rel));
        const ext = (/\.([^./]+)$/.exec(rel) || [0, '(none)'])[1].toLowerCase();
        if (ext === 'uasset' || ext === 'umap') row = censusPackage(rel, st.size);
        else {
          row = { p: rel, t: ext, size: st.size };
          if (ext !== 'uexp' && ext !== 'ubulk') {
            const fd = fs.openSync(path.join(ASSETS, rel), 'r');
            const b = Buffer.alloc(Math.min(16, st.size));
            fs.readSync(fd, b, 0, b.length, 0);
            fs.closeSync(fd);
            row.magic = b.toString('hex');
          }
        }
      } catch (e) { row = { p: rel, t: 'error', size: 0, err: String(e.message) }; }
      lines.push(JSON.stringify(row));
    }
    fs.writeFileSync(outFile + '.tmp', lines.join('\n') + '\n');
    fs.renameSync(outFile + '.tmp', outFile);
    done++;
  }
  process.stdout.write('RESULT ' + JSON.stringify({ shard: k, chunks: done }) + '\n');
}

// ---------------------------------------------------------------- merge

// Classes an existing converter decodes completely (reader + writer + round-trip audit).
const EXISTING = {
  Texture2D: 'tools/lib/texture.js', LightMapTexture2D: 'tools/lib/texture.js', ShadowMapTexture2D: 'tools/lib/texture.js',
  TextureLightProfile: 'tools/lib/texture.js', TextureCube: 'tools/lib/texture.js',
  StaticMesh: 'tools/lib/mesh.js', SkeletalMesh: 'tools/lib/mesh.js', DataTable: 'tools/lib/datatable.js',
  SQEXSEADSound: 'tools/lib/sead.js', SQEXSEADMusic: 'tools/lib/sead.js', SQEXSEADSoundBank: 'tools/lib/sead.js',
  AnimSequence: 'tools/lib/anim.js',
};
const OTHER_AGENT = {};

function merge(started) {
  const list = JSON.parse(fs.readFileSync(LIST, 'utf8'));
  const nChunks = Math.ceil(list.length / CHUNK);
  const byExt = {}, byClass = {}, classIdx = new Map(), classes = [];
  const supers = new Map(); // blueprint class name -> Map(super name -> count)
  const totals = { files: 0, bytes: 0, packages: 0, exports: 0, export_bytes: 0, header_errors: 0, packages_without_uexp: 0, uexp_gap_bytes: 0, packages_with_gap: 0 };
  const subtree = {};
  const OUT = path.join(ROOT, 'audit', 'asset_census.json');
  const SUM = path.join(ROOT, 'audit', 'asset_census_summary.json');
  const manifestFile = path.join(ROOT, 'audit', 'package_manifest.json');
  const manifest = fs.existsSync(manifestFile) ? JSON.parse(fs.readFileSync(manifestFile, 'utf8')) : null;
  const fd = fs.openSync(OUT + '.tmp', 'w');
  const rowsTmp = path.join(TMP, 'rows.tmp');
  const rfd = fs.openSync(rowsTmp, 'w');
  let first = true;
  const cid = (name) => { let i = classIdx.get(name); if (i === undefined) { i = classes.length; classes.push(name); classIdx.set(name, i); } return i; };
  for (let c = 0; c < nChunks; c++) {
    const f = path.join(TMP, `chunk_${String(c).padStart(5, '0')}.jsonl`);
    if (!fs.existsSync(f)) throw new Error('missing chunk ' + c + ' (rerun to resume)');
    const out = [];
    for (const line of fs.readFileSync(f, 'utf8').split('\n')) {
      if (!line) continue;
      const row = JSON.parse(line);
      totals.files++; totals.bytes += row.size;
      const top = row.p.startsWith('_superseded/') ? '_superseded' : row.p.startsWith('_case_collision/') ? '_case_collision' : 'live';
      const st = (subtree[top] = subtree[top] || { files: 0, bytes: 0, packages: 0 });
      st.files++; st.bytes += row.size;
      const ext = (/\.([^./]+)$/.exec(row.p) || [0, '(none)'])[1].toLowerCase();
      const be = (byExt[ext] = byExt[ext] || { files: 0, bytes: 0, magic: {} });
      be.files++; be.bytes += row.size;
      if (row.magic !== undefined) { const m = row.magic.slice(0, 8); be.magic[m] = (be.magic[m] || 0) + 1; }
      if (row.t !== 'package') { out.push(JSON.stringify([row.p, row.t, row.size])); continue; }
      totals.packages++; st.packages++;
      if (row.err) { totals.header_errors++; out.push(JSON.stringify([row.p, 'package', row.size, row.err])); continue; }
      if (row.uexp === null) totals.packages_without_uexp++;
      if (row.gap) { totals.uexp_gap_bytes += row.gap; totals.packages_with_gap++; }
      for (const [cls, sup, from, meta] of row.sup) { let m = supers.get(cls); if (!m) supers.set(cls, (m = new Map())); const key = sup + '\0' + from + '\0' + meta; m.set(key, (m.get(key) || 0) + 1); }
      const ids = [];
      for (const x of row.ex) {
        const [name, from, size, propBytes, tail, raw, cdo, extra] = x;
        const key = from === '(this package)' || (from && !from.startsWith('/Script/')) ? name + ' [blueprint class]' : name;
        ids.push(cid(key));
        totals.exports++; totals.export_bytes += size;
        const bc = (byClass[key] = byClass[key] || { from: {}, exports: 0, bytes: 0, property_bytes: 0, tail_bytes: 0, raw_property_bytes: 0, cdo: 0, guid_flag_only: 0, has_guid: 0, tail_zero: 0, native_tail_bytes: 0, exports_with_native_tail: 0, property_errors: 0, max_tail: 0, example: null, example_tail: null });
        bc.from[from] = (bc.from[from] || 0) + 1;
        bc.exports++; bc.bytes += size; bc.property_bytes += propBytes; bc.tail_bytes += tail; bc.raw_property_bytes += raw; bc.cdo += cdo;
        if (!bc.example) bc.example = row.p;
        if (typeof extra === 'string') { bc.property_errors++; bc.native_tail_bytes += tail; bc.exports_with_native_tail++; bc.error_example = bc.error_example || row.p + ': ' + extra; continue; }
        // native tail = bytes after the properties and the object-guid flag (+ guid)
        const guidBytes = cdo ? 0 : extra === 1 ? 20 : extra === 0 ? 4 : 0;
        const native = tail - guidBytes;
        if (extra === 1) bc.has_guid++;
        if (native === 0) { if (cdo) bc.tail_zero++; else bc.guid_flag_only++; } else {
          bc.native_tail_bytes += native; bc.exports_with_native_tail++;
          if (native > bc.max_tail) { bc.max_tail = native; bc.example_tail = row.p; }
        }
      }
      out.push(JSON.stringify([row.p, 'package', row.size, ids]));
    }
    for (const l of out) { fs.writeSync(rfd, (first ? '' : ',\n') + l); first = false; }
  }
  fs.closeSync(rfd);

  // Resolve blueprint classes to a native base through the recorded super chain.
  const nativeBase = (name, depth = 0) => {
    const m = supers.get(name);
    if (!m || depth > 32) return null;
    const bases = new Set();
    for (const key of m.keys()) {
      const [sup, from] = key.split('\0');
      if (from.startsWith('/Script/')) bases.add(sup); else { const b = nativeBase(sup, depth + 1); bases.add(b || '?' + sup); }
    }
    return bases.size === 1 ? [...bases][0] : [...bases].sort().join('|');
  };
  // every class exported by a package (blueprint generated classes, user structs/enums are not classes) -> native base
  const bases = {};
  for (const name of [...supers.keys()].sort()) { const b = nativeBase(name); if (b) bases[name] = b; }
  const BASES = path.join(ROOT, 'audit', 'blueprint_class_bases.json');
  writeJson(BASES, bases);
  for (const [key, bc] of Object.entries(byClass)) {
    const bp = / \[blueprint class\]$/.test(key);
    const name = key.replace(/ \[blueprint class\]$/, '');
    if (bp) bc.native_base = bases[name] || null;
    bc.layout = bc.exports_with_native_tail === 0 && bc.property_errors === 0 ? 'tagged properties only' : 'native data after the properties';
    const man = manifest && manifest.classes && manifest.classes[key];
    if (EXISTING[name] && !bp) { bc.status = 'a'; bc.decoder = EXISTING[name]; }
    else if (man && man.delegated_bytes === 0 && man.exports === man.decoded && man.uninterpreted_bytes === 0 && man.roundtrip_identical === man.exports) { bc.status = 'a'; bc.decoder = 'tools/lib/package.js'; if (man.third_party_bytes) bc.third_party_payload_bytes = man.third_party_bytes; }
    else if (man && man.delegated_bytes > 0 && man.delegated_bytes < man.bytes && man.exports === man.decoded && man.uninterpreted_bytes === 0 && man.roundtrip_identical === man.exports) { bc.status = 'a'; bc.decoder = 'tools/lib/package.js, with part of each export read by another converter of this project (delegated_bytes in audit/package_manifest.json)'; if (man.third_party_bytes) bc.third_party_payload_bytes = man.third_party_bytes; }
    else if (man && man.delegated_bytes > 0) { bc.status = 'c'; bc.note = 'export data left to another converter; tools/lib/package.js records only its size and hash'; }
    else if (bc.exports_with_native_tail === 0 && bc.property_errors === 0) bc.status = bc.raw_property_bytes ? 'b (properties only; some property values kept raw)' : 'b';
    else bc.status = 'c';
    if (OTHER_AGENT[name]) bc.note = OTHER_AGENT[name];
  }
  const sorted = Object.fromEntries(Object.entries(byClass).sort((a, b) => b[1].native_tail_bytes - a[1].native_tail_bytes || b[1].bytes - a[1].bytes));
  for (const be of Object.values(byExt)) be.magic = Object.fromEntries(Object.entries(be.magic).sort((a, b) => b[1] - a[1]).slice(0, 12));
  const statusCount = { a: 0, b: 0, c: 0 }, statusBytes = { a: 0, b: 0, c: 0 };
  for (const bc of Object.values(byClass)) { statusCount[bc.status[0]]++; statusBytes[bc.status[0]] += bc.bytes; }
  const summary = {
    generated_by: 'tools/asset_census.js', version: VERSION, root: 'assets/', seconds: Math.round((Date.now() - started) / 1000),
    status_source: manifest ? 'audit/package_manifest.json (tools/package_check.js) for classes decoded by tools/lib/package.js' : 'no audit/package_manifest.json yet: only the older converters count as decoders',
    status_key: { a: 'fully decoded by an existing tool (reader + writer, round-trip audited); `decoder` names it and `layout` says whether the class has native data', b: 'tagged properties only: no export of the class has bytes after its properties and object-guid flag', c: 'native serialized data after the properties (native_tail_bytes) that the property parser does not consume' },
    totals, subtrees: subtree, class_count: classes.length, classes_by_status: statusCount, export_bytes_by_status: statusBytes,
    by_extension: Object.fromEntries(Object.entries(byExt).sort((a, b) => b[1].bytes - a[1].bytes)),
    by_class: sorted,
  };
  fs.writeSync(fd, '{\n"generated_by": "tools/asset_census.js",\n"version": ' + JSON.stringify(VERSION) + ',\n"row_format": ["path under assets/", "container type", "size", "export class indexes into classes[] (packages)"],\n');
  fs.writeSync(fd, '"totals": ' + JSON.stringify(totals) + ',\n"subtrees": ' + JSON.stringify(subtree) + ',\n"classes": ' + JSON.stringify(classes) + ',\n"files": [\n');
  const rd = fs.openSync(rowsTmp, 'r');
  const buf = Buffer.allocUnsafe(1 << 22);
  for (let n; (n = fs.readSync(rd, buf, 0, buf.length, null)) > 0;) fs.writeSync(fd, buf, 0, n);
  fs.closeSync(rd); fs.rmSync(rowsTmp);
  fs.writeSync(fd, '\n]\n}\n');
  fs.closeSync(fd);
  fs.renameSync(OUT + '.tmp', OUT);
  writeJson(SUM, summary);
  logRun({ tool: 'asset_census', version: VERSION, outputs: [OUT, SUM, BASES], note: `${totals.files} files, ${totals.packages} packages, ${totals.exports} exports, ${classes.length} classes` });
  console.log(JSON.stringify({ totals, subtree, classes: classes.length, statusCount, statusBytes }, null, 1));
}

function main(args) {
  const started = Date.now();
  if (args.fresh) fs.rmSync(TMP, { recursive: true, force: true });
  fs.mkdirSync(TMP, { recursive: true });
  if (!fs.existsSync(LIST)) {
    const out = [];
    walk(ASSETS, '', out);
    out.sort();
    fs.writeFileSync(LIST, JSON.stringify(out));
    console.log('listed', out.length, 'files');
  }
  const n = args.workers;
  let running = 0, failed = 0;
  for (let k = 0; k < n; k++) {
    running++;
    const c = spawn(process.execPath, ['--max-old-space-size=4000', __filename, '--shard', `${k}/${n}`], { stdio: ['ignore', 'pipe', 'pipe'] });
    let out = '', err = '';
    c.stdout.on('data', (d) => (out += d));
    c.stderr.on('data', (d) => (err += d));
    c.on('close', (code) => {
      if (code !== 0 || !/^RESULT /m.test(out)) { failed++; console.error(`shard ${k} failed (${code}): ${err.slice(-500)}`); }
      if (!--running) { if (failed) { console.error('some shards failed; rerun to resume'); process.exit(1); } merge(started); }
    });
  }
}

const args = parseArgs(process.argv.slice(2));
if (args.shard) worker(args);
else main(args);
