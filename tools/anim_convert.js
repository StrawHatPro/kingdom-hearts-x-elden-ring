// Phase 4: UAnimSequence exports -> glTF 2.0 binary (.glb) animation + sidecar JSON, with a
// round-trip proof per sequence. Format: docs/formats/animsequence.md; code: tools/lib/anim.js.
//
//   node tools/anim_convert.js index
//       Scans every package under assets/ for exports of the animation asset classes and writes
//       build/anim_tmp/anim_index.json (the work list) with the per-class counts.
//   node tools/anim_convert.js export [--workers N] [--budget SECONDS] [--prefix <path prefix>]
//       For every AnimSequence: parses the export (every byte assigned), re-serialises it and
//       compares; decodes the keys; writes <name>.glb and <name>.anim.json under
//       assets_converted/animations/; reads both files back and rebuilds the export from them
//       (plus the package name table) and compares again. The related classes (AnimMontage,
//       BlendSpace, ...) carry no key data: they are parsed and re-serialised only.
//       Resumable: per-export rows accumulate in build/anim_tmp/export_<k>.jsonl, workers stop
//       taking new work after the time budget (default 540 s), and the run that finds every
//       export done writes audit/anim_manifest.json and audit/anim_export.json.
//   node tools/anim_convert.js verify [...]
//       The same checks in memory, nothing written under assets_converted/ (rows: verify_<k>.jsonl,
//       result: audit/anim_verify.json).
//
// Reads assets/ (never writes there). At most 8 worker processes.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { spawn } = require('child_process');
const { ROOT, logRun, writeJson } = require('./lib/log');
const ua = require('./lib/uasset');
const mesh = require('./lib/mesh');
const anim = require('./lib/anim');

const VERSION = '1.0.2';
const ASSETS = path.join(ROOT, 'assets');
const OUT_ROOT = path.join(ROOT, 'assets_converted', 'animations');
const TMP = path.join(ROOT, 'build', 'anim_tmp');
const INDEX = path.join(TMP, 'anim_index.json');
const SEQ = 'AnimSequence';
const PLAIN = ['AnimMontage', 'AnimComposite', 'BlendSpace', 'BlendSpace1D', 'AimOffsetBlendSpace', 'AimOffsetBlendSpace1D', 'PoseAsset'];
const CLASSES = [SEQ, ...PLAIN];
const QUAT_TOLERANCE = 1e-3; // on | |q|^2 - 1 | of a decoded key, before the glTF copy is normalised
const sha256 = (b) => crypto.createHash('sha256').update(b).digest('hex');
const bump = (stats, key, value, by = 1) => { const m = (stats[key] = stats[key] || {}); m[value] = (m[value] || 0) + by; };
const readLines = (file) => (fs.existsSync(file) ? fs.readFileSync(file, 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l)) : []);

function parseArgs(argv) {
  const a = { mode: argv[0], prefix: '', workers: 8, budget: 540 };
  for (let i = 1; i < argv.length; i++) {
    if (argv[i] === '--prefix') a.prefix = argv[++i];
    else if (argv[i] === '--workers') a.workers = Math.min(8, Math.max(1, +argv[++i]));
    else if (argv[i] === '--budget') a.budget = +argv[++i];
    else if (argv[i] === '--shard') a.shard = argv[++i].split('/').map(Number);
    else throw new Error('unknown argument ' + argv[i]);
  }
  if (!['index', 'export', 'verify'].includes(a.mode)) throw new Error('usage: anim_convert.js index|export|verify [--workers N] [--budget S] [--prefix P]');
  return a;
}

// ------------------------------------------------------------------ index

function listPackages() {
  const list = [];
  const walk = (d, rel) => {
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      if (rel === '' && e.name.startsWith('_')) continue; // _case_collision, _superseded
      const r = rel ? rel + '/' + e.name : e.name;
      if (e.isDirectory()) walk(path.join(d, e.name), r); else if (/\.(uasset|umap)$/.test(e.name)) list.push(r);
    }
  };
  walk(ASSETS, '');
  return list.sort();
}

function indexWorker(a) {
  const [k, n] = a.shard;
  const list = fs.readFileSync(path.join(TMP, 'packages.txt'), 'utf8').split('\n').filter(Boolean);
  const needles = CLASSES.map((c) => Buffer.from(c + '\0', 'latin1'));
  const out = [];
  for (let i = k; i < list.length; i += n) {
    const buf = fs.readFileSync(path.join(ASSETS, list[i]));
    if (!needles.some((nd) => buf.includes(nd))) continue;
    let pkg;
    try { pkg = ua.parseHeader(buf); } catch (e) { out.push({ p: list[i], err: e.message }); continue; }
    pkg.exports.forEach((e, idx) => { const c = ua.objectName(pkg, e.classIndex); if (CLASSES.includes(c)) out.push({ p: list[i], i: idx, c, n: ua.nameOf(pkg, e.objectName), s: e.serialSize }); });
  }
  fs.writeFileSync(path.join(TMP, `index_${k}.json`), JSON.stringify(out));
}

function indexParent(a) {
  fs.mkdirSync(TMP, { recursive: true });
  const list = listPackages();
  fs.writeFileSync(path.join(TMP, 'packages.txt'), list.join('\n'));
  const n = a.workers;
  let running = n;
  for (let k = 0; k < n; k++) {
    spawn(process.execPath, [__filename, 'index', '--shard', `${k}/${n}`], { stdio: 'inherit' }).on('close', (code) => {
      if (code !== 0) { console.error(`index worker ${k} failed`); process.exitCode = 1; }
      if (--running) return;
      if (process.exitCode) return;
      let all = [];
      for (let j = 0; j < n; j++) { const f = path.join(TMP, `index_${j}.json`); all = all.concat(JSON.parse(fs.readFileSync(f, 'utf8'))); fs.rmSync(f); }
      all.sort((x, y) => (x.p < y.p ? -1 : x.p > y.p ? 1 : x.i - y.i));
      const classes = {};
      for (const c of CLASSES) classes[c] = { exports: 0, bytes: 0 };
      const errors = all.filter((r) => r.err);
      for (const r of all) if (!r.err) { classes[r.c].exports++; classes[r.c].bytes += r.s; }
      writeJson(INDEX, { generated_by: 'tools/anim_convert.js', version: VERSION, packages_scanned: list.length, classes, header_errors: errors, exports: all.filter((r) => !r.err) });
      logRun({ tool: 'anim_convert', version: VERSION, outputs: [INDEX], note: `index: ${list.length} packages; ` + CLASSES.map((c) => `${c} ${classes[c].exports}`).join(', ') });
      console.log(JSON.stringify({ packages_scanned: list.length, classes, header_errors: errors.length }, null, 1));
    });
  }
}

// ------------------------------------------------------------------ worker

const skeletons = new Map();
function loadSkeleton(pkg, doc) {
  const ref = anim.propValue(doc.props, 'Skeleton');
  if (!ref || typeof ref.object !== 'number') throw new Error('no Skeleton property');
  const imp = anim.importPath(pkg, ref.object);
  if (!imp) throw new Error('Skeleton is not an import');
  const file = anim.packageFile(imp.packageName);
  if (!file) throw new Error('skeleton package outside /Game and /Engine: ' + imp.packageName);
  const key = file + '#' + imp.objectName;
  if (!skeletons.has(key)) {
    let v;
    try {
      const head = path.join(ASSETS, file + '.uasset');
      if (!fs.existsSync(head)) throw new Error('skeleton package not found: ' + file + '.uasset');
      const sp = ua.parseHeader(fs.readFileSync(head));
      const e = sp.exports.find((x) => ua.objectName(sp, x.classIndex) === 'Skeleton' && ua.nameOf(sp, x.objectName) === imp.objectName);
      if (!e) throw new Error(`no Skeleton export ${imp.objectName} in ${file}.uasset`);
      v = { skel: anim.parseSkeleton(sp, fs.readFileSync(path.join(ASSETS, file + '.uexp')), e), path: file + '.uasset', object: imp.objectName };
    } catch (err) { v = { error: err.message }; }
    skeletons.set(key, v);
  }
  const v = skeletons.get(key);
  if (v.error) throw new Error(v.error);
  return v;
}

function convertSequence(a, it, pkg, uexp, exp, row) {
  const { doc, start, end } = anim.parseExport(pkg, uexp, exp);
  const src = uexp.subarray(start, end);
  row.sha256_source_export = sha256(src);
  row.parse_roundtrip_matches = anim.serialize(pkg, doc).equals(src);
  if (!doc.bSerializeCompressedData) throw new Error('no compressed data in the export');
  row.key_format = anim.KEY_FORMATS[doc.keyEncodingFormat];
  row.sequence_formats = [doc.translationCompressionFormat, doc.rotationCompressionFormat, doc.scaleCompressionFormat].map((f) => anim.COMP_FORMATS[f]).join('/');
  row.num_frames = anim.propValue(doc.props, 'NumFrames');
  const sl = anim.propValue(doc.props, 'SequenceLength');
  row.sequence_length = sl === undefined ? null : sl;
  row.tracks = doc.compressedTrackToSkeletonMapTable.length;
  row.stream_bytes = doc.streamLength;
  row.raw_size_declared = doc.compressedRawDataSize;
  row.scale_table = doc.compressedScaleOffsets.offsetData.length > 0;
  row.use_raw_data_only = doc.bUseRawDataOnly;
  row.strip_flags = doc.stripFlags.join(',');
  row.scale_strip_size = doc.compressedScaleOffsets.stripSize;
  const add = anim.propValue(doc.props, 'AdditiveAnimType');
  if (add !== undefined) row.additive = add;
  const curves = anim.propValue(doc.compressedCurveData, 'FloatCurves');
  row.curve_properties = doc.compressedCurveData.map((p) => p.tag.name).join(',');
  row.float_curves = curves && curves.items ? curves.items.length : null;
  row.raw_values_in_properties = anim.countRaw(doc.props);
  const tmap = anim.propValue(doc.props, 'TrackToSkeletonMapTable');
  row.track_map_property = !tmap || !tmap.items ? 'absent' : tmap.items.length === row.tracks ? 'same length as the compressed table' : 'other length';
  // per block statistics
  const fm = {};
  let memSame = true, pos = 0, noBlock = { translation: 0, rotation: 0, scale: 0 };
  const frameStats = { ok: 0, bad: 0 };
  for (const b of doc.stream.blocks) {
    if (b.memoryOffset !== pos) memSame = false;
    pos += b.size;
    const k = `${b.kind} ${anim.COMP_FORMATS[b.format]} flags ${b.flags}`;
    const e = (fm[k] = fm[k] || [0, 0]);
    e[0]++; e[1] += b.numKeys;
    if (b.frames) {
      let ok = true, prev = -1;
      for (let j = 0; j < b.numKeys; j++) { const v = b.frameBytes === 2 ? b.frames.readUInt16LE(j * 2) : b.frames[j]; if (v <= prev || v >= row.num_frames) ok = false; if (j === 0 && v !== 0) ok = false; prev = v; }
      if (prev !== row.num_frames - 1) ok = false;
      frameStats[ok ? 'ok' : 'bad']++;
    }
  }
  const nComp = row.tracks * (row.scale_table ? 3 : 2);
  row.blocks = doc.stream.blocks.length;
  row.components_without_block = nComp - row.blocks;
  row.block_formats = fm;
  row.memory_order = memSame ? 'same as the file order' : 'differs from the file order';
  row.frame_tables = frameStats;
  row.pad_overrides = doc.stream.padOverrides.length;
  // skeleton + glTF
  const sk = loadSkeleton(pkg, doc);
  row.skeleton = sk.path;
  row.bones = sk.skel.bones.length;
  row.skeleton_guid = sk.skel.guidIfNoRetargetSources === null ? 'skeleton has retarget sources (not compared)' : sk.skel.guidIfNoRetargetSources === doc.skeletonGuid ? 'equals the skeleton asset GUID' : 'differs from the skeleton asset GUID';
  const base = it.p.replace(/\.(uasset|umap)$/, '');
  const relOut = base + (path.posix.basename(base) === it.n ? '' : '.' + it.n);
  const conv = anim.toGlb(mesh, pkg, doc, sk.skel, { package: it.p, export: it.n, skeleton: sk.path, converter: 'tools/anim_convert.js', converter_version: VERSION });
  let glb = conv.glb;
  let sidecarText = JSON.stringify(conv.sidecar) + '\n';
  row.decode = conv.stats;
  row.interpolation = conv.sidecar.gltf.interpolation;
  if (a.mode === 'export') {
    const glbPath = path.join(OUT_ROOT, relOut + '.glb'), jsonPath = path.join(OUT_ROOT, relOut + '.anim.json');
    fs.mkdirSync(path.dirname(glbPath), { recursive: true });
    fs.writeFileSync(glbPath, glb);
    fs.writeFileSync(jsonPath, sidecarText);
    glb = fs.readFileSync(glbPath); // the proof below is made from the bytes that are on disk
    sidecarText = fs.readFileSync(jsonPath, 'utf8');
    row.converted_path = 'assets_converted/animations/' + relOut + '.glb';
    row.sidecar_path = 'assets_converted/animations/' + relOut + '.anim.json';
  }
  row.glb_bytes = glb.length;
  row.sidecar_bytes = Buffer.byteLength(sidecarText);
  row.sha256_glb = sha256(glb);
  const back = anim.rebuild(mesh, pkg, JSON.parse(sidecarText), glb);
  row.rebuild_roundtrip_matches = back.equals(src);
  row.roundtrip_matches = row.parse_roundtrip_matches && row.rebuild_roundtrip_matches;
  // decoded key sanity
  const s = conv.stats, problems = [];
  if (!row.roundtrip_matches) problems.push('rebuilt bytes differ from the original');
  if (row.pad_overrides) problems.push(`${row.pad_overrides} pad bytes are not 0x55`);
  if (s.nonFiniteValues) problems.push(`${s.nonFiniteValues} decoded values are not finite`);
  if (s.nonIncreasingTimes) problems.push('key times are not strictly increasing');
  if (s.badBone) problems.push(`${s.badBone} tracks name a bone outside the skeleton`);
  if (s.duplicateBone) problems.push(`${s.duplicateBone} tracks name a bone that an earlier track already drives`);
  if (s.keysOverNumFrames) problems.push(`${s.keysOverNumFrames} blocks have more keys than NumFrames`);
  if (frameStats.bad) problems.push(`${frameStats.bad} frame tables are not 0 .. NumFrames-1 strictly increasing`);
  if (s.maxQuatNormDeviation > QUAT_TOLERANCE) problems.push(`a decoded quaternion is ${s.maxQuatNormDeviation.toExponential(2)} away from unit length (|q|^2)`);
  if (problems.length) row.error = problems.join('; ');
}

function worker(a) {
  const [k, n] = a.shard;
  const items = JSON.parse(fs.readFileSync(INDEX, 'utf8')).exports.filter((r) => r.p.startsWith(a.prefix));
  const done = new Set();
  for (const f of fs.readdirSync(TMP)) if (new RegExp(`^${a.mode}_\\d+\\.jsonl$`).test(f)) for (const r of readLines(path.join(TMP, f))) if (r.converter_version === VERSION) done.add(r.package + '#' + r.export_index);
  const out = fs.openSync(path.join(TMP, `${a.mode}_${k}.jsonl`), 'a');
  const stopAt = Date.now() + a.budget * 1000;
  let cur = null, curPath = null, curUexp = null;
  for (let i = 0; i < items.length; i++) {
    if ((i >> 3) % n !== k) continue; // runs of 8 keep the exports of one folder (one skeleton) in one worker
    const it = items[i];
    if (done.has(it.p + '#' + (it.i + 1))) continue;
    if (Date.now() > stopAt) break;
    const row = { package: it.p, export: it.n, export_index: it.i + 1, class: it.c, export_size: it.s };
    try {
      if (curPath !== it.p) { cur = ua.parseHeader(fs.readFileSync(path.join(ASSETS, it.p))); curUexp = fs.readFileSync(path.join(ASSETS, it.p.replace(/\.(uasset|umap)$/, '.uexp'))); curPath = it.p; }
      const exp = cur.exports[it.i];
      if (it.c === SEQ) convertSequence(a, it, cur, curUexp, exp, row);
      else {
        const { doc, start, end } = anim.parseAssetExport(cur, curUexp, exp);
        const src = curUexp.subarray(start, end);
        row.sha256_source_export = sha256(src);
        row.roundtrip_matches = anim.serializeAsset(cur, doc).equals(src);
        row.native_bytes_after_properties = 20;
        row.raw_values_in_properties = anim.countRaw(doc.props);
        if (!row.roundtrip_matches) row.error = 're-serialised bytes differ from the original';
      }
    } catch (err) {
      row.roundtrip_matches = false;
      row.error = String(err && err.message || err);
    }
    row.converter = 'tools/anim_convert.js'; row.converter_version = VERSION;
    fs.writeSync(out, JSON.stringify(row) + '\n');
  }
  fs.closeSync(out);
}

// ------------------------------------------------------------------ parent

function summarise(a, rows, started) {
  const classes = {};
  const S = {};
  const failures = [];
  const tot = { blocks: 0, keys: 0, channels: 0, constant_channels: 0, quat_keys: 0, quats_with_clamped_w: 0, max_quat_norm_deviation: 0, timed_blocks: 0, glb_bytes: 0, sidecar_bytes: 0, export_bytes: 0, stream_bytes: 0, tracks: 0 };
  for (const r of rows) {
    const c = (classes[r.class] = classes[r.class] || { exports: 0, export_bytes: 0, roundtrip_matches: 0, failures: 0 });
    c.exports++; c.export_bytes += r.export_size;
    if (r.roundtrip_matches) c.roundtrip_matches++;
    if (r.error) { c.failures++; failures.push({ package: r.package, export: r.export, class: r.class, reason: r.error }); }
    if (r.class !== SEQ || !r.decode) continue;
    const kf = r.key_format;
    bump(S, 'key_format', kf);
    bump(S, 'sequence_formats (T/R/S)', `${kf}: ${r.sequence_formats}`);
    bump(S, 'scale_table', `${kf}: ${r.scale_table ? 'present' : 'absent'}, StripSize ${r.scale_strip_size}`);
    bump(S, 'memory_order', `${r.scale_table ? 'with scale table' : 'no scale table'}, ${kf}: ${r.memory_order}`);
    bump(S, 'strip_flags', r.strip_flags);
    bump(S, 'use_raw_data_only', r.use_raw_data_only);
    bump(S, 'interpolation', r.interpolation);
    bump(S, 'curve_properties', r.curve_properties || '(none)');
    bump(S, 'float_curves', r.float_curves ? 'non-empty' : 'empty');
    bump(S, 'float_curves_total', 'curves', r.float_curves || 0);
    bump(S, 'tagged_property_values_kept_raw', r.raw_values_in_properties ? 'sequences with some' : 'sequences with none');
    bump(S, 'tagged_property_values_kept_raw', 'values', r.raw_values_in_properties);
    bump(S, 'track_map_property', r.track_map_property);
    bump(S, 'skeleton_guid', r.skeleton_guid);
    bump(S, 'frame_tables', 'first 0, last NumFrames-1, strictly increasing', r.frame_tables.ok);
    bump(S, 'frame_tables', 'other', r.frame_tables.bad);
    bump(S, 'components_without_block', kf, r.components_without_block);
    bump(S, 'additive_anim_type', r.additive === undefined ? '(property absent)' : String(r.additive));
    for (const [f, [nb, nk]] of Object.entries(r.block_formats)) { bump(S, 'blocks', `${kf} ${f}`, nb); bump(S, 'keys', `${kf} ${f}`, nk); }
    const d = r.decode;
    tot.blocks += d.blocks; tot.keys += d.keys; tot.channels += d.channels; tot.constant_channels += d.constantChannels; tot.quat_keys += d.quatKeys; tot.quats_with_clamped_w += d.quatsClampedW; tot.timed_blocks += d.timedBlocks;
    if (d.maxQuatNormDeviation > tot.max_quat_norm_deviation) tot.max_quat_norm_deviation = d.maxQuatNormDeviation;
    bump(S, 'max_quat_norm_deviation_per_sequence', d.maxQuatNormDeviation <= 1e-6 ? '<= 1e-6' : d.maxQuatNormDeviation <= 1e-5 ? '<= 1e-5' : d.maxQuatNormDeviation <= 1e-4 ? '<= 1e-4' : d.maxQuatNormDeviation <= 1e-3 ? '<= 1e-3' : '> 1e-3');
    tot.glb_bytes += r.glb_bytes; tot.sidecar_bytes += r.sidecar_bytes; tot.export_bytes += r.export_size; tot.stream_bytes += r.stream_bytes; tot.tracks += r.tracks;
  }
  for (const key of Object.keys(S)) S[key] = Object.fromEntries(Object.entries(S[key]).sort((x, y) => (x[0] < y[0] ? -1 : 1)));
  const index = JSON.parse(fs.readFileSync(INDEX, 'utf8'));
  return {
    generated_by: 'tools/anim_convert.js', version: VERSION, mode: a.mode, prefix: a.prefix,
    exports: rows.length, roundtrip_matches: rows.filter((r) => r.roundtrip_matches).length, failures_count: failures.length,
    quat_tolerance: QUAT_TOLERANCE, classes_in_assets: index.classes, classes, sequence_totals: tot, sequence_stats: S, failures,
    seconds_this_run: Math.round((Date.now() - started) / 1000),
  };
}

function main(a) {
  if (!fs.existsSync(INDEX)) throw new Error('run "node tools/anim_convert.js index" first');
  fs.mkdirSync(TMP, { recursive: true });
  const started = Date.now();
  const n = a.workers;
  let running = n;
  const errors = [];
  for (let k = 0; k < n; k++) {
    const argv = [__filename, a.mode, '--shard', `${k}/${n}`, '--budget', String(a.budget)];
    if (a.prefix) argv.push('--prefix', a.prefix);
    const c = spawn(process.execPath, ['--max-old-space-size=6000', ...argv], { stdio: ['ignore', 'inherit', 'pipe'] });
    let err = '';
    c.stderr.on('data', (d) => (err += d));
    c.on('close', (code) => {
      if (code !== 0) errors.push({ shard: k, code, error: err.slice(-600) });
      if (--running) return;
      const items = JSON.parse(fs.readFileSync(INDEX, 'utf8')).exports.filter((r) => r.p.startsWith(a.prefix));
      const got = new Map();
      for (const f of fs.readdirSync(TMP)) if (new RegExp(`^${a.mode}_\\d+\\.jsonl$`).test(f)) for (const r of readLines(path.join(TMP, f))) if (r.converter_version === VERSION) got.set(r.package + '#' + r.export_index, r);
      const rows = items.map((it) => got.get(it.p + '#' + (it.i + 1))).filter(Boolean);
      if (errors.length) console.log('worker errors: ' + JSON.stringify(errors));
      if (rows.length < items.length) { console.log(`${rows.length} of ${items.length} exports done so far; run the same command again to continue`); return; }
      const summary = summarise(a, rows, started);
      summary.worker_errors = errors;
      const suffix = a.prefix ? '.partial' : '';
      const rowsFile = path.join(ROOT, 'audit', `${a.mode === 'export' ? 'anim_export' : 'anim_verify_rows'}${suffix}.json`);
      const sumFile = path.join(ROOT, 'audit', `${a.mode === 'export' ? 'anim_manifest' : 'anim_verify'}${suffix}.json`);
      const fd = fs.openSync(rowsFile, 'w');
      fs.writeSync(fd, '[\n');
      rows.forEach((r, i) => fs.writeSync(fd, JSON.stringify(r) + (i + 1 < rows.length ? ',\n' : '\n')));
      fs.writeSync(fd, ']\n');
      fs.closeSync(fd);
      summary.rows = path.relative(ROOT, rowsFile).replace(/\\/g, '/');
      summary.rows_sha256 = sha256(fs.readFileSync(rowsFile));
      writeJson(sumFile, summary);
      logRun({ tool: 'anim_convert', version: VERSION, outputs: [rowsFile, sumFile], note: `${a.mode}: ${summary.exports} exports, ${summary.roundtrip_matches} round-trip, ${summary.failures_count} failures, ${errors.length} worker errors` });
      const brief = { ...summary, failures: summary.failures.slice(0, 20) };
      console.log(JSON.stringify(brief, null, 1));
    });
  }
}

const args = parseArgs(process.argv.slice(2));
if (args.mode === 'index') (args.shard ? indexWorker : indexParent)(args);
else if (args.shard) worker(args);
else main(args);
