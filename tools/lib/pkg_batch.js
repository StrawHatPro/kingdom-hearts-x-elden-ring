// Shared driver for tools/package_convert.js and tools/package_check.js: lists the packages under
// assets/, cuts the list into chunks, runs them on up to 8 worker processes (each finished chunk
// is a file, so a rerun resumes), and aggregates per-class statistics.
const fs = require('fs');
const path = require('path');
const { spawn } = require('child_process');
const { ROOT } = require('./log');

const ASSETS = path.join(ROOT, 'assets');
const CHUNK = 400;

function walk(dir, relDir, out) {
  for (const d of fs.readdirSync(dir, { withFileTypes: true })) {
    const rel = relDir ? relDir + '/' + d.name : d.name;
    if (d.isDirectory()) walk(path.join(dir, d.name), rel, out);
    else if (/\.(uasset|umap)$/i.test(d.name)) out.push(rel);
  }
}
function walkExt(dir, relDir, out, re) {
  for (const d of fs.readdirSync(dir, { withFileTypes: true })) {
    const rel = relDir ? relDir + '/' + d.name : d.name;
    if (d.isDirectory()) walkExt(path.join(dir, d.name), rel, out, re);
    else if (re.test(d.name)) out.push(rel);
  }
}
// .uexp files with no header beside them, paired with a header found at the same path in the
// live tree or in another superseded pak when that header's export table tiles the file exactly
// (exports end 4 bytes before its end). Entries are "orphan.uexp|header.uasset".
function orphanPairs(packages) {
  const ua = require('./uasset');
  const have = new Set(packages.map((p) => p.replace(/\.(uasset|umap)$/i, '').toLowerCase()));
  const byPath = new Map(); // logical path (without _superseded/<pak>/) -> header paths
  const logical = (p) => p.replace(/^_(superseded|case_collision)\/[^/]+\//, '');
  const byName = new Map(); // file name without extension -> header paths
  for (const p of packages) { const k = logical(p).replace(/\.(uasset|umap)$/i, '').toLowerCase(); if (!byPath.has(k)) byPath.set(k, []); byPath.get(k).push(p); const n = k.slice(k.lastIndexOf('/') + 1); if (!byName.has(n)) byName.set(n, []); byName.get(n).push(p); }
  const uexps = [];
  walkExt(ASSETS, '', uexps, /\.uexp$/i);
  uexps.sort();
  const out = [];
  const ends = new Map();
  for (const u of uexps) {
    const base = u.replace(/\.uexp$/i, '');
    if (have.has(base.toLowerCase())) continue;
    const size = fs.statSync(path.join(ASSETS, u)).size;
    let cands = (byPath.get(logical(base).toLowerCase()) || []).slice().sort((a, b) => (a.startsWith('_') ? 1 : 0) - (b.startsWith('_') ? 1 : 0));
    // no header at that path in any pak: the paks ship some .uexp files at <dir>/<name> whose headers
    // exist only in the per-language directories <dir>/<lang>/<name>; try those (same tiling test)
    if (!cands.length) {
      const lb = logical(base).toLowerCase(); const cut = lb.lastIndexOf('/');
      const re = new RegExp('^' + lb.slice(0, cut).replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '/[^/]+/' + lb.slice(cut + 1).replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '$');
      cands = (byName.get(lb.slice(cut + 1)) || []).filter((h) => re.test(logical(h).replace(/\.(uasset|umap)$/i, '').toLowerCase())).sort((a, b) => (a.startsWith('_') ? 1 : 0) - (b.startsWith('_') ? 1 : 0) || (a < b ? -1 : 1));
    }
    for (const h of cands) {
      if (!ends.has(h)) {
        let end = -1;
        try { const pkg = ua.parseHeader(fs.readFileSync(path.join(ASSETS, h))); end = 0; for (const e of pkg.exports) end = Math.max(end, e.serialOffset - pkg.summary.totalHeaderSize + e.serialSize); } catch { /* unreadable header */ }
        ends.set(h, end);
      }
      if (ends.get(h) + 4 === size) { out.push(u + '|' + h); break; }
    }
  }
  return out;
}
// Package paths (relative to assets/), sorted, followed by the orphan pairs; cached in the scratch directory.
function packageList(tmp) {
  const file = path.join(tmp, 'packages.json');
  if (fs.existsSync(file)) return JSON.parse(fs.readFileSync(file, 'utf8'));
  const out = [];
  walk(ASSETS, '', out);
  out.sort();
  const all = out.concat(orphanPairs(out));
  fs.mkdirSync(tmp, { recursive: true });
  fs.writeFileSync(file, JSON.stringify(all));
  return all;
}
// Other headers of the same package path (the live tree and the superseded copies), for a
// package whose own header does not describe its .uexp (lib/package.js decode opts.altHeaders).
let ALT_INDEX = null;
function altHeaders(tmp, entry) {
  const logical = (p) => p.replace(/^_(superseded|case_collision)\/[^/]+\//, '').replace(/\.(uasset|umap)$/i, '').toLowerCase();
  if (!ALT_INDEX) {
    ALT_INDEX = new Map();
    for (const p of packageList(tmp)) { if (p.includes('|')) continue; const k = logical(p); if (!ALT_INDEX.has(k)) ALT_INDEX.set(k, []); ALT_INDEX.get(k).push(p); }
  }
  const own = entry.includes('|') ? entry.slice(entry.indexOf('|') + 1) : entry;
  return (ALT_INDEX.get(logical(own)) || []).filter((p) => p !== own).map((rel) => ({ rel, bytes: fs.readFileSync(path.join(ASSETS, rel)) }));
}
// A list entry -> { rel (the name rows and JSON files use), header path, .uexp path }
function entryPaths(entry) {
  const i = entry.indexOf('|');
  if (i < 0) return { rel: entry, header: path.join(ASSETS, entry), uexp: path.join(ASSETS, entry.replace(/\.(uasset|umap)$/i, '.uexp')), headerRel: null };
  return { rel: entry.slice(0, i), header: path.join(ASSETS, entry.slice(i + 1)), uexp: path.join(ASSETS, entry.slice(0, i)), headerRel: entry.slice(i + 1) };
}

function parseArgs(argv, extra = {}) {
  const a = { workers: 8, fresh: false, prefix: '', ...extra };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--workers') a.workers = Math.min(8, Math.max(1, +argv[++i]));
    else if (argv[i] === '--fresh') a.fresh = true;
    else if (argv[i] === '--prefix') a.prefix = argv[++i];
    else if (argv[i] === '--worker') a.worker = +argv[++i];
    else if (argv[i].startsWith('--') && argv[i].slice(2) in extra) a[argv[i].slice(2)] = typeof extra[argv[i].slice(2)] === 'boolean' ? true : argv[++i];
    else throw new Error('unknown argument ' + argv[i]);
  }
  return a;
}

const chunkFile = (tmp, c) => path.join(tmp, `chunk_${String(c).padStart(5, '0')}.jsonl`);

// Worker side: claims chunks through lock files so that fast workers take more of them.
function runWorker(tmp, perPackage, redo) {
  const list = packageList(tmp);
  const nChunks = Math.ceil(list.length / CHUNK);
  if (redo) {
    // --redo A,B: recompute only the packages that contain an export of one of these classes
    // (or failed), keeping every other row of the finished chunks
    const want = new Set(redo.split(','));
    let n = 0;
    for (let c = 0; c < nChunks; c++) {
      const out = chunkFile(tmp, c);
      if (!fs.existsSync(out)) continue;
      const mark = out + '.redo';
      try { fs.closeSync(fs.openSync(mark, 'wx')); } catch { continue; }
      const rows = fs.readFileSync(out, 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l));
      let changed = false;
      rows.forEach((row, i) => {
        if (!row.fatal && !Object.entries(row.cls || {}).some(([k, v]) => want.has(k) || want.has(v.nc))) return;
        const entry = row.entry || row.p;
        try { rows[i] = perPackage(entry); } catch (e) { rows[i] = { p: row.p, entry: row.entry, fatal: String(e && e.stack || e).slice(0, 600) }; }
        changed = true; n++;
      });
      if (changed) { fs.writeFileSync(out + '.tmp', rows.map((r) => JSON.stringify(r)).join('\n') + '\n'); fs.renameSync(out + '.tmp', out); }
    }
    process.stdout.write('RESULT ' + JSON.stringify({ redone: n }) + '\n');
    return;
  }
  // large chunks first is not known in advance; walk in a scattered order so the big map
  // directories are spread over time
  const order = [...Array(nChunks).keys()].sort((a, b) => ((a * 7919) % nChunks) - ((b * 7919) % nChunks));
  let done = 0;
  for (const c of order) {
    const out = chunkFile(tmp, c);
    if (fs.existsSync(out)) continue;
    const lock = out + '.lock';
    try { fs.closeSync(fs.openSync(lock, 'wx')); } catch { continue; }
    const lines = [];
    for (let i = c * CHUNK; i < Math.min(list.length, (c + 1) * CHUNK); i++) {
      let row;
      try { row = perPackage(list[i]); } catch (e) { const bar = list[i].indexOf('|'); row = { p: bar < 0 ? list[i] : list[i].slice(0, bar), entry: bar < 0 ? undefined : list[i], fatal: String(e && e.stack || e).slice(0, 600) }; }
      lines.push(JSON.stringify(row));
    }
    fs.writeFileSync(out + '.tmp', lines.join('\n') + '\n');
    fs.renameSync(out + '.tmp', out);
    fs.rmSync(lock, { force: true });
    done++;
  }
  process.stdout.write('RESULT ' + JSON.stringify({ chunks: done }) + '\n');
}

// Parent side: spawn workers, then call merge(rows iterator).
function runParent(script, tmp, args, passArgs, merge) {
  if (args.fresh) fs.rmSync(tmp, { recursive: true, force: true });
  fs.mkdirSync(tmp, { recursive: true });
  for (const f of fs.readdirSync(tmp)) if (/\.(lock|tmp|redo)$/.test(f)) fs.rmSync(path.join(tmp, f), { force: true });
  const list = packageList(tmp);
  const nChunks = Math.ceil(list.length / CHUNK);
  const started = Date.now();
  let running = 0, failed = 0;
  const tick = setInterval(() => {
    const n = fs.readdirSync(tmp).filter((f) => /^chunk_\d+\.jsonl$/.test(f)).length;
    process.stdout.write(`chunks ${n}/${nChunks}  ${Math.round((Date.now() - started) / 1000)} s\n`);
  }, 60000);
  for (let k = 0; k < args.workers; k++) {
    running++;
    const c = spawn(process.execPath, ['--max-old-space-size=12000', script, '--worker', String(k), ...passArgs], { stdio: ['ignore', 'pipe', 'pipe'] });
    let out = '', err = '';
    c.stdout.on('data', (d) => (out += d));
    c.stderr.on('data', (d) => (err += d));
    c.on('close', (code) => {
      if (code !== 0 || !/^RESULT /m.test(out)) { failed++; console.error(`worker ${k} failed (${code}): ${err.slice(-800)}`); }
      if (--running) return;
      clearInterval(tick);
      for (const f of fs.readdirSync(tmp)) if (/\.(lock|tmp|redo)$/.test(f)) fs.rmSync(path.join(tmp, f), { force: true });
      const missing = [];
      for (let c2 = 0; c2 < nChunks; c2++) if (!fs.existsSync(chunkFile(tmp, c2))) missing.push(c2);
      if (missing.length) { console.error(`${missing.length} chunks not finished (${failed} workers failed); rerun to resume`); process.exit(1); }
      const rows = function* () { for (let c2 = 0; c2 < nChunks; c2++) for (const line of fs.readFileSync(chunkFile(tmp, c2), 'utf8').split('\n')) if (line) yield JSON.parse(line); };
      merge(rows, { packages: list.length, seconds: Math.round((Date.now() - started) / 1000) });
    });
  }
}

// Per-package statistics row from decode() stats: classes aggregated inside the package.
function packageRow(p, stats, extra) {
  const cls = {};
  for (const s of stats.exports) {
    const c = (cls[s.class] = cls[s.class] || { n: 0, b: 0, d: 0, u: 0, g: 0, t: 0 });
    c.n++; c.b += s.size; c.u += s.uninterpreted; c.g += s.delegated;
    if (s.uninterpreted === 0 && !s.error) c.d++;
    if (s.thirdParty) { for (const k in s.thirdParty) { c.t += s.thirdParty[k]; c.tp = c.tp || {}; c.tp[k] = (c.tp[k] || 0) + s.thirdParty[k]; } }
    if (s.error) { c.e = c.e || {}; const k = s.error.replace(/\d+/g, '#').slice(0, 140); c.e[k] = (c.e[k] || 0) + 1; }
    for (const k in s.rawKinds) { c.r = c.r || {}; c.r[k] = (c.r[k] || 0) + s.rawKinds[k]; }
    if (s.family) c.f = s.family;
    if (s.nativeClass) c.nc = s.nativeClass;
  }
  return { p, hu: stats.headerUninterpreted, gap: stats.gapBytes, mismatch: stats.mismatch, paired: stats.pairedHeader, cls, ...extra };
}

// Aggregate rows into the per-class table used by audit/package_convert.json and audit/package_manifest.json.
function aggregate(rows) {
  const classes = {};
  const tot = { packages: 0, orphan_uexp_paired_with_a_header: 0, roundtrip_identical: 0, roundtrip_different: 0, fatal: 0, exports: 0, export_bytes: 0, fully_decoded_exports: 0, uninterpreted_bytes: 0, third_party_bytes: 0, delegated_bytes: 0, header_uninterpreted_bytes: 0, uexp_gap_bytes: 0, export_table_mismatch_packages: 0 };
  const different = [], fatal = [], mismatched = [], rePaired = [], thirdParty = {};
  for (const row of rows()) {
    tot.packages++;
    if (row.entry) tot.orphan_uexp_paired_with_a_header++;
    if (row.fatal) { tot.fatal++; if (fatal.length < 200) fatal.push({ package: row.p, error: row.fatal }); continue; }
    if (row.rt) tot.roundtrip_identical++; else { tot.roundtrip_different++; if (different.length < 500) different.push({ package: row.p, reason: row.why || null }); }
    tot.header_uninterpreted_bytes += row.hu || 0; tot.uexp_gap_bytes += row.gap || 0;
    if (row.paired) rePaired.push({ package: row.p, export_data_decoded_with_header: row.paired });
    if (row.mismatch) { tot.export_table_mismatch_packages++; mismatched.push({ package: row.p, reason: row.mismatch }); }
    for (const [name, c] of Object.entries(row.cls || {})) {
      const a = (classes[name] = classes[name] || { family: null, files: 0, files_roundtrip_identical: 0, exports: 0, bytes: 0, decoded: 0, roundtrip_identical: 0, uninterpreted_bytes: 0, third_party_bytes: 0, delegated_bytes: 0 });
      a.files++; if (row.rt) { a.files_roundtrip_identical++; a.roundtrip_identical += c.n; }
      a.exports += c.n; a.bytes += c.b; a.decoded += c.d; a.uninterpreted_bytes += c.u; a.third_party_bytes += c.t; a.delegated_bytes += c.g;
      if (c.f) a.family = c.f;
      if (c.nc) a.native_class = c.nc;
      if (c.tp) for (const k in c.tp) { a.third_party = a.third_party || {}; a.third_party[k] = (a.third_party[k] || 0) + c.tp[k]; thirdParty[k] = (thirdParty[k] || 0) + c.tp[k]; }
      if (c.e) { a.errors = a.errors || {}; for (const k in c.e) { const e = (a.errors[k] = a.errors[k] || { exports: 0, example: row.p }); e.exports += c.e[k]; } }
      if (c.r) { a.undecoded_kinds = a.undecoded_kinds || {}; for (const k in c.r) { const e = (a.undecoded_kinds[k] = a.undecoded_kinds[k] || { bytes: 0, example: row.p }); e.bytes += c.r[k]; } }
      tot.exports += c.n; tot.export_bytes += c.b; tot.fully_decoded_exports += c.d; tot.uninterpreted_bytes += c.u; tot.third_party_bytes += c.t; tot.delegated_bytes += c.g;
    }
  }
  const sorted = Object.fromEntries(Object.entries(classes).sort((a, b) => b[1].uninterpreted_bytes - a[1].uninterpreted_bytes || b[1].bytes - a[1].bytes));
  tot.packages_decoded_with_another_copy_of_their_header = rePaired.length;
  return { totals: tot, third_party_formats: thirdParty, different, fatal, export_table_mismatch: mismatched, decoded_with_another_header: rePaired, classes: sorted };
}

module.exports = { ASSETS, CHUNK, packageList, entryPaths, altHeaders, parseArgs, runWorker, runParent, packageRow, aggregate };
