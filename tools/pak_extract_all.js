// Extract every pak entry to assets/ and prove each file equals the manifest's sha256_extracted.
//   assets/<mounted path>                       the version the game loads (last pak wins)
//   assets/_superseded/<pak>/<mounted path>     older versions of files a patch pak replaces
// The voice overlay paks (Content/_DLC_/VOICE/<lang>/Paks) are outside the mount order
// (tools/lib/pak.js) and are not extracted: audit/pak_overlays.json maps each of their entries to
// the regular file with the same bytes. Files under _superseded/ and _case_collision/ that no
// manifest row names any more are removed at the end of a run.
// Updates audit/asset_manifest/<pak>.json (extracted_path, on_disk) and audit/asset_manifest.json.
//   node tools/pak_extract_all.js [--jobs N]
//   node tools/pak_extract_all.js --one <pak path relative to gameRoot>     (worker)
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { spawn } = require('child_process');
const { ROOT, CONFIG, logRun, writeJson } = require('./lib/log');
const pak = require('./lib/pak');

const VERSION = '1.0.0';
const WINNERS = path.join(ROOT, 'build', 'pak_winners.json');
const MAN_DIR = path.join(ROOT, 'audit', 'asset_manifest');
const manName = (rel) => rel.replace(/^KINGDOM HEARTS III\/Content\//, '').replace(/[\\/]/g, '__').replace(/\.pak$/, '') + '.json';
const clean = (p) => p.replace(/^(\.\.\/)+/, '').replace(/^\//, '');

function extractOne(rel) {
  const { winners, caseClash } = JSON.parse(fs.readFileSync(WINNERS, 'utf8'));
  const file = path.join(CONFIG.gameRoot, rel);
  const idx = pak.readIndex(file);
  const manFile = path.join(MAN_DIR, manName(rel));
  const rows = JSON.parse(fs.readFileSync(manFile, 'utf8'));
  if (rows.length !== idx.entries.length) throw new Error('manifest and index disagree on entry count');
  const fd = fs.openSync(file, 'r');
  const pakTag = path.basename(rel, '.pak');
  const sum = { pak: rel, files: 0, bytes: 0, effective: 0, superseded: 0, hash_mismatch: 0, skipped_existing: 0, written_missing_or_wrong_size: 0, rewritten_wrong_hash: 0 };
  const made = new Set();
  idx.entries.forEach((e, i) => {
    const row = rows[i];
    const mounted = clean(idx.mountPoint + e.name);
    const wins = winners[mounted] === rel;
    // Windows file names are case-insensitive: a second path that differs only in case would
    // overwrite the first, so it is kept under its own directory instead.
    const clash = wins && caseClash[mounted];
    const relOut = clash ? `assets/_case_collision/${pakTag}/${mounted}` : wins ? `assets/${mounted}` : `assets/_superseded/${pakTag}/${mounted}`;
    if (clash) row.case_collision_with = caseClash[mounted];
    const out = path.join(ROOT, relOut);
    row.extracted_path = relOut;
    row.superseded = !wins;
    // resumable: a file of the right size that is already there is re-hashed instead of rewritten
    let data = null;
    if (fs.existsSync(out) && fs.statSync(out).size === e.uncompressedSize) { data = fs.readFileSync(out); sum.skipped_existing++; }
    else {
      sum.written_missing_or_wrong_size++;
      data = pak.extract(fd, e);
      const dir = path.dirname(out);
      if (!made.has(dir)) { fs.mkdirSync(dir, { recursive: true }); made.add(dir); }
      fs.writeFileSync(out, data);
    }
    let ok = crypto.createHash('sha256').update(data).digest('hex') === row.sha256_extracted;
    if (!ok) { // a stale or foreign file was sitting at this path: write the real contents and check again
      sum.rewritten_wrong_hash++;
      data = pak.extract(fd, e);
      fs.mkdirSync(path.dirname(out), { recursive: true });
      fs.writeFileSync(out, data);
      ok = crypto.createHash('sha256').update(data).digest('hex') === row.sha256_extracted;
    }
    row.on_disk = ok;
    if (!ok) sum.hash_mismatch++;
    sum.files++; sum.bytes += data.length;
    wins ? sum.effective++ : sum.superseded++;
  });
  fs.closeSync(fd);
  fs.writeFileSync(manFile, '[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]\n');
  sum.manifest_sha256 = crypto.createHash('sha256').update(fs.readFileSync(manFile)).digest('hex');
  return sum;
}

// Overlay paks (Content/_DLC_/VOICE/<lang>/Paks): not extracted. Every entry must be SHA-1
// identical (pak index hashes) to the regular file at the other language's path; the mapping is
// written to audit/pak_overlays.json and the paks' manifest rows point at the regular file.
function describeOverlays(winners) {
  const regular = new Map(); // mounted path -> { pak, sha1, size } of the version the game loads
  const byPak = new Map();
  for (const [p, rel] of Object.entries(winners)) { if (!byPak.has(rel)) byPak.set(rel, new Set()); byPak.get(rel).add(p); }
  for (const [rel, paths] of byPak) {
    const idx = pak.readIndex(path.join(CONFIG.gameRoot, rel));
    for (const e of idx.entries) { const m = clean(idx.mountPoint + e.name); if (paths.has(m)) regular.set(m, { pak: rel, sha1: e.sha1, size: e.uncompressedSize }); }
  }
  const out = { generated_by: 'tools/pak_extract_all.js', version: VERSION,
    note: 'Voice overlay paks are alternatives the game mounts one at a time (func_141555B10 in the executable; docs/formats/pak_mount_finding.md). They are not extracted: each entry duplicates the regular file named in `duplicates`.',
    row_format: ['entry path as mounted', 'index SHA-1', 'uncompressed size', 'regular mounted path with the same bytes', 'pak that supplies that regular file'],
    overlays: [] };
  for (const file of pak.listOverlayPaks()) {
    const rel = path.relative(CONFIG.gameRoot, file).replace(/\\/g, '/');
    const { lang, pathLang } = pak.overlayLanguages(file);
    const idx = pak.readIndex(file);
    const manFile = path.join(MAN_DIR, manName(rel));
    const rows = JSON.parse(fs.readFileSync(manFile, 'utf8'));
    if (rows.length !== idx.entries.length) throw new Error('manifest and index disagree on entry count for ' + rel);
    const o = { pak: rel, voice_language: lang, entries_use_paths_of_language: pathLang, mount_point: idx.mountPoint, entries: idx.entries.length, identical_to_regular_file: 0, different: 0, no_regular_file: 0, bytes: 0, mismatches: [], files: [] };
    idx.entries.forEach((e, i) => {
      const mounted = clean(idx.mountPoint + e.name);
      const dup = mounted.split('/').map((c) => (c === pathLang ? lang : c)).join('/');
      const reg = dup !== mounted ? regular.get(dup) : undefined;
      const row = rows[i];
      row.extracted_path = null; row.on_disk = false; row.superseded = false; row.overlay = true;
      o.bytes += e.uncompressedSize;
      if (!reg) { o.no_regular_file++; o.mismatches.push({ entry: mounted, reason: 'no regular file at ' + dup }); row.duplicates = null; }
      else if (reg.sha1 !== e.sha1 || reg.size !== e.uncompressedSize) { o.different++; o.mismatches.push({ entry: mounted, reason: 'differs from ' + dup }); row.duplicates = null; }
      else { o.identical_to_regular_file++; row.duplicates = 'assets/' + dup; }
      o.files.push([mounted, e.sha1, e.uncompressedSize, reg ? dup : null, reg ? reg.pak : null]);
    });
    fs.writeFileSync(manFile, '[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]\n');
    o.manifest_sha256 = crypto.createHash('sha256').update(fs.readFileSync(manFile)).digest('hex');
    out.overlays.push(o);
  }
  return out;
}

// Files under assets/_superseded and assets/_case_collision that no manifest row names any more
// (left over from an earlier mount order) are removed; files elsewhere under assets/ that no row
// names are only counted (the *.usm.d directories written by tools/usm_extract.js are skipped).
function pruneStale(pakList) {
  const expected = new Set();
  for (const rel of pakList) for (const r of JSON.parse(fs.readFileSync(path.join(MAN_DIR, manName(rel)), 'utf8'))) if (r.extracted_path) expected.add(r.extracted_path.toLowerCase());
  const res = { removed_files: 0, removed_bytes: 0, removed_directories: 0, unlisted_live_files: 0, unlisted_live_examples: [], expected_files: expected.size };
  const stale = [];
  const walk = (dir, rel, prunable) => {
    let left = 0;
    for (const d of fs.readdirSync(dir, { withFileTypes: true })) {
      const r = rel + '/' + d.name, abs = path.join(dir, d.name);
      if (d.isDirectory()) {
        if (/\.usm\.d$/i.test(d.name)) { left++; continue; }
        walk(abs, r, prunable || (rel === 'assets' && (d.name === '_superseded' || d.name === '_case_collision')));
        left++;
      } else if (expected.has(r.toLowerCase())) left++;
      else if (prunable) { stale.push(abs); left++; }
      else { left++; res.unlisted_live_files++; if (res.unlisted_live_examples.length < 10) res.unlisted_live_examples.push(r); }
    }
    return left;
  };
  walk(path.join(ROOT, 'assets'), 'assets', false);
  // safety: a count far above what a mount-order change can explain means the manifests are wrong
  const LIMIT = +(process.env.PAK_PRUNE_LIMIT || 80000);
  if (stale.length > LIMIT) { res.not_removed = `${stale.length} stale files exceed the limit of ${LIMIT}; nothing was deleted`; return res; }
  const dirs = new Set();
  for (const abs of stale) { res.removed_bytes += fs.statSync(abs).size; fs.rmSync(abs); res.removed_files++; dirs.add(path.dirname(abs)); }
  // remove directories emptied by that, bottom-up, never above _superseded / _case_collision
  const stop = path.join(ROOT, 'assets');
  for (let d of [...dirs].sort((a, b) => b.length - a.length)) {
    while (d.length > stop.length + 1 && path.dirname(d) !== stop) { try { fs.rmdirSync(d); res.removed_directories++; } catch (e) { break; } d = path.dirname(d); }
  }
  return res;
}

if (process.argv[2] === '--one') {
  process.stdout.write('RESULT ' + JSON.stringify(extractOne(process.argv[3])) + '\n');
} else {
  const par = process.argv.includes('--jobs') ? +process.argv[process.argv.indexOf('--jobs') + 1] : 8;
  // which pak provides the version of each path the game loads
  const winners = {};
  for (const [p, { pak: file }] of pak.mountAll()) winners[p] = path.relative(CONFIG.gameRoot, file).replace(/\\/g, '/');
  fs.mkdirSync(path.dirname(WINNERS), { recursive: true });
  const firstByLower = new Map(), caseClash = {};
  for (const p of Object.keys(winners).sort()) { const l = p.toLowerCase(); if (firstByLower.has(l)) caseClash[p] = firstByLower.get(l); else firstByLower.set(l, p); }
  fs.writeFileSync(WINNERS, JSON.stringify({ winners, caseClash }));
  const all = pak.listPaks().filter((p) => !pak.isOverlayPak(p)).map((p) => path.relative(CONFIG.gameRoot, p).replace(/\\/g, '/'));
  const OVERLAYS = path.join(ROOT, 'audit', 'pak_overlays.json');
  const size = (p) => fs.statSync(path.join(CONFIG.gameRoot, p)).size;
  const queue = all.slice().sort((a, b) => size(b) - size(a));
  const results = [], errors = [];
  let running = 0, done = 0;
  const finish = () => {
    const tot = { files: 0, bytes: 0, effective: 0, superseded: 0, hash_mismatch: 0, skipped_existing: 0, written_missing_or_wrong_size: 0, rewritten_wrong_hash: 0 };
    for (const r of results) for (const k of Object.keys(tot)) tot[k] += r[k];
    const top = path.join(ROOT, 'audit', 'asset_manifest.json');
    const m = JSON.parse(fs.readFileSync(top, 'utf8'));
    // overlay paks: mapping manifest instead of files
    const ov = describeOverlays(winners);
    writeJson(OVERLAYS, ov);
    const ovSum = ov.overlays.map((o) => ({ pak: o.pak, entries: o.entries, bytes: o.bytes, identical_to_regular_file: o.identical_to_regular_file, different: o.different, no_regular_file: o.no_regular_file }));
    // files left behind by an earlier mount order
    const pruned = errors.length ? { skipped: 'extraction errors' } : pruneStale(all);
    m.extraction = { generated_by: 'tools/pak_extract_all.js', ...tot, unique_paths: Object.keys(winners).length, case_collisions: Object.keys(caseClash).length, errors,
      overlay_paks_not_extracted: ovSum, overlay_manifest: 'audit/pak_overlays.json', stale_files: pruned };
    const by = new Map(results.map((r) => [r.pak, r]));
    for (const a of m.archives) if (by.has(a.pak)) { a.manifest_sha256 = by.get(a.pak).manifest_sha256; a.extracted_files = by.get(a.pak).files; }
    for (const o of ov.overlays) { const a = m.archives.find((x) => x.pak === o.pak); if (a) { a.manifest_sha256 = o.manifest_sha256; a.extracted_files = 0; a.overlay = true; } }
    writeJson(top, m);
    logRun({ tool: 'pak_extract_all', version: VERSION, outputs: [top, OVERLAYS], note: JSON.stringify({ ...tot, overlays: ovSum, stale: pruned }) });
    console.log('\n' + JSON.stringify(tot), errors.length ? 'ERRORS ' + JSON.stringify(errors).slice(0, 600) : '');
    console.log('overlays', JSON.stringify(ovSum));
    console.log('stale files', JSON.stringify(pruned));
    if (errors.length || tot.hash_mismatch || ov.overlays.some((o) => o.different || o.no_regular_file)) process.exit(1);
  };
  const next = () => {
    while (running < par && queue.length) {
      const rel = queue.shift();
      running++;
      const c = spawn(process.execPath, ['--max-old-space-size=6000', __filename, '--one', rel], { stdio: ['ignore', 'pipe', 'pipe'] });
      let out = '', err = '';
      c.stdout.on('data', (d) => (out += d));
      c.stderr.on('data', (d) => (err += d));
      c.on('close', (code) => {
        running--; done++;
        const m = /^RESULT (.*)$/m.exec(out);
        if (code === 0 && m) results.push(JSON.parse(m[1])); else errors.push({ pak: rel, error: err.slice(-300) });
        process.stdout.write(`\r${done}/${all.length}`);
        if (queue.length) next(); else if (!running) finish();
      });
    }
  };
  next();
}
