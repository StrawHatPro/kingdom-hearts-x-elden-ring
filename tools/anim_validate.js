// Phase 4: validation of the exported animations.
//
//   node tools/anim_validate.js [--sample N] [--seed S] [--workers N] [--budget SECONDS]
//
// 1. Khronos glTF-Validator (npm package gltf-validator, shares no code with tools/lib/anim.js) on
//    a deterministic sample of the .glb files listed in audit/anim_export.json: N files (default
//    2000) drawn with xorshift32 from the seed, plus the largest and the smallest file of every
//    key encoding. --sample 0 validates every file. Resumable: per-file results accumulate in
//    build/anim_tmp/validate_<k>.jsonl; a run stops taking new files after the budget (default
//    520 s); the run that finds the whole sample done writes audit/anim_validation.json.
// 2. For the same files, an independent read of the .glb (not through tools/lib/anim.js): every
//    rotation key is a unit quaternion (| |q| - 1 | <= 1e-5), every value is finite, every
//    sampler's key times start at 0, increase strictly and end at or before SequenceLength, the
//    key count of every sampler is <= NumFrames, and every channel targets a bone node.
// 3. The byte-exact round-trip and decode-sanity totals of audit/anim_manifest.json are quoted
//    in the result so that one file answers "is the animation export valid".
const fs = require('fs');
const path = require('path');
const { spawn } = require('child_process');
const validator = require('gltf-validator');
const { ROOT, logRun, writeJson } = require('./lib/log');

const VERSION = '1.0.0';
const TMP = path.join(ROOT, 'build', 'anim_tmp');
const SEVERITY = ['error', 'warning', 'info', 'hint'];
const readLines = (file) => (fs.existsSync(file) ? fs.readFileSync(file, 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l)) : []);

function parseArgs(argv) {
  const a = { sample: 2000, seed: 1, workers: 8, budget: 520 };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--sample') a.sample = +argv[++i];
    else if (argv[i] === '--seed') a.seed = +argv[++i];
    else if (argv[i] === '--workers') a.workers = Math.min(8, Math.max(1, +argv[++i]));
    else if (argv[i] === '--budget') a.budget = +argv[++i];
    else if (argv[i] === '--shard') a.shard = argv[++i].split('/').map(Number);
    else throw new Error('unknown argument ' + argv[i]);
  }
  return a;
}

function pick(a) {
  const rows = JSON.parse(fs.readFileSync(path.join(ROOT, 'audit', 'anim_export.json'), 'utf8')).filter((r) => r.converted_path)
    .sort((x, y) => (x.converted_path < y.converted_path ? -1 : 1));
  if (!a.sample || a.sample >= rows.length) return rows;
  const chosen = new Set();
  for (const kf of [...new Set(rows.map((r) => r.key_format))]) {
    let lo = -1, hi = -1;
    rows.forEach((r, i) => { if (r.key_format !== kf) return; if (lo < 0 || r.glb_bytes < rows[lo].glb_bytes) lo = i; if (hi < 0 || r.glb_bytes > rows[hi].glb_bytes) hi = i; });
    chosen.add(lo); chosen.add(hi);
  }
  let state = a.seed >>> 0 || 1; // xorshift32: the sample is reproducible from the seed
  const rnd = () => { state ^= state << 13; state >>>= 0; state ^= state >>> 17; state ^= state << 5; state >>>= 0; return state / 4294967296; };
  while (chosen.size < Math.min(a.sample, rows.length)) chosen.add(Math.floor(rnd() * rows.length));
  return [...chosen].sort((x, y) => x - y).map((i) => rows[i]);
}

// Independent structural read of one .glb.
function checkGlb(buf, row) {
  const r = { quat_keys: 0, max_quat_length_error: 0, non_unit_quats: 0, non_finite: 0, samplers: 0, bad_times: 0, samplers_over_num_frames: 0, channels: 0, bad_targets: 0, keys: 0 };
  const jl = buf.readUInt32LE(12);
  const json = JSON.parse(buf.toString('utf8', 20, 20 + jl));
  const bin = buf.subarray(28 + jl, 28 + jl + buf.readUInt32LE(20 + jl));
  const acc = (i) => { const a = json.accessors[i], v = json.bufferViews[a.bufferView]; const n = { SCALAR: 1, VEC3: 3, VEC4: 4 }[a.type]; const o = (v.byteOffset || 0) + (a.byteOffset || 0); return { a, n, f: (k) => bin.readFloatLE(o + k * 4) }; };
  const nNodes = json.nodes.length;
  const len = json.extras.ue_sequence_length, nf = json.extras.ue_num_frames;
  for (const an of json.animations || []) {
    const timesChecked = new Set();
    for (const ch of an.channels) {
      r.channels++;
      if (!(ch.target.node >= 1 && ch.target.node < nNodes)) r.bad_targets++;
      const s = an.samplers[ch.sampler];
      r.samplers++;
      const out = acc(s.output), inp = acc(s.input);
      if (inp.a.count !== out.a.count) r.bad_times++;
      if (out.a.count > nf) r.samplers_over_num_frames++;
      r.keys += out.a.count;
      if (!timesChecked.has(s.input)) {
        timesChecked.add(s.input);
        let ok = inp.f(0) === 0;
        for (let k = 1; k < inp.a.count; k++) if (!(inp.f(k) > inp.f(k - 1))) ok = false;
        if (!(inp.f(inp.a.count - 1) <= Math.fround(len) * (1 + 1e-6))) ok = false;
        if (!ok) r.bad_times++;
      }
      for (let k = 0; k < out.a.count; k++) {
        let l2 = 0;
        for (let c = 0; c < out.n; c++) { const v = out.f(k * out.n + c); if (!Number.isFinite(v)) r.non_finite++; l2 += v * v; }
        if (ch.target.path === 'rotation') { r.quat_keys++; const e = Math.abs(Math.sqrt(l2) - 1); if (e > r.max_quat_length_error) r.max_quat_length_error = e; if (!(e <= 1e-5)) r.non_unit_quats++; }
      }
    }
  }
  r.ok = !r.non_unit_quats && !r.non_finite && !r.bad_times && !r.samplers_over_num_frames && !r.bad_targets && r.channels === row.decode.channels && r.keys === row.decode.keys + row.decode.constantChannels;
  return r;
}

async function worker(a) {
  const [k, n] = a.shard;
  const out = path.join(TMP, `validate_${k}.jsonl`);
  const done = new Set();
  for (let j = 0; j < 8; j++) for (const r of readLines(path.join(TMP, `validate_${j}.jsonl`))) done.add(r.file + ' ' + r.sha256_glb);
  const rows = pick(a);
  const stopAt = Date.now() + a.budget * 1000;
  for (let i = k; i < rows.length; i += n) {
    const row = rows[i];
    if (done.has(row.converted_path + ' ' + row.sha256_glb)) continue;
    if (Date.now() > stopAt) break;
    const res = { file: row.converted_path, sha256_glb: row.sha256_glb, key_format: row.key_format };
    try {
      const buf = fs.readFileSync(path.join(ROOT, row.converted_path));
      res.bytes = buf.length;
      const rep = await validator.validateBytes(new Uint8Array(buf), { uri: path.basename(row.converted_path), maxIssues: 0, writeTimestamp: false });
      const issues = {}, first = {};
      for (const m of rep.issues.messages) { const key = SEVERITY[m.severity] + ':' + m.code; issues[key] = (issues[key] || 0) + 1; if (m.severity < 2 && !first[m.code]) first[m.code] = ((m.pointer || '') + ' ' + m.message).trim(); }
      Object.assign(res, { errors: rep.issues.numErrors, warnings: rep.issues.numWarnings, infos: rep.issues.numInfos, hints: rep.issues.numHints, issues });
      if (Object.keys(first).length) res.first_message_per_code = first;
      res.animation_channels = rep.info && rep.info.animationCount !== undefined ? rep.info.animationCount : undefined;
      res.check = checkGlb(buf, row);
    } catch (e) { res.validator_failed = String(e && e.message || e); }
    fs.appendFileSync(out, JSON.stringify(res) + '\n');
  }
}

function parent(a) {
  fs.mkdirSync(TMP, { recursive: true });
  const n = a.workers;
  let running = n;
  for (let k = 0; k < n; k++) {
    const c = spawn(process.execPath, ['--max-old-space-size=6000', __filename, '--shard', `${k}/${n}`, '--sample', String(a.sample), '--seed', String(a.seed), '--budget', String(a.budget)], { stdio: ['ignore', 'inherit', 'inherit'] });
    c.on('close', () => {
      if (--running) return;
      const rows = pick(a);
      const got = new Map();
      for (let j = 0; j < 8; j++) for (const r of readLines(path.join(TMP, `validate_${j}.jsonl`))) got.set(r.file + ' ' + r.sha256_glb, r);
      const files = rows.map((r) => got.get(r.converted_path + ' ' + r.sha256_glb)).filter(Boolean);
      if (files.length < rows.length) { console.log(`${files.length} of ${rows.length} files validated so far; run the same command again to continue`); return; }
      const totals = { files: 0, bytes: 0, validator_failed: 0, files_without_errors: 0, files_without_errors_or_warnings: 0, errors: 0, warnings: 0, infos: 0, hints: 0 };
      const check = { files_ok: 0, files_not_ok: 0, channels: 0, keys: 0, quat_keys: 0, non_unit_quats: 0, max_quat_length_error: 0, non_finite: 0, bad_times: 0, samplers_over_num_frames: 0, bad_targets: 0 };
      const codes = {}, per = {}, bad = [];
      for (const r of files) {
        totals.files++;
        const p = (per[r.key_format] = per[r.key_format] || { files: 0, files_without_errors_or_warnings: 0 });
        p.files++;
        if (r.validator_failed) { totals.validator_failed++; bad.push(r); continue; }
        totals.bytes += r.bytes; totals.errors += r.errors; totals.warnings += r.warnings; totals.infos += r.infos; totals.hints += r.hints;
        if (!r.errors) totals.files_without_errors++;
        if (!r.errors && !r.warnings) { totals.files_without_errors_or_warnings++; p.files_without_errors_or_warnings++; } else bad.push(r);
        for (const [code, cnt] of Object.entries(r.issues)) { const e = (codes[code] = codes[code] || { messages: 0, files: 0 }); e.messages += cnt; e.files++; }
        const c = r.check;
        check[c.ok ? 'files_ok' : 'files_not_ok']++;
        if (!c.ok && !bad.includes(r)) bad.push(r);
        for (const key of ['channels', 'keys', 'quat_keys', 'non_unit_quats', 'non_finite', 'bad_times', 'samplers_over_num_frames', 'bad_targets']) check[key] += c[key];
        if (c.max_quat_length_error > check.max_quat_length_error) check.max_quat_length_error = c.max_quat_length_error;
      }
      const man = JSON.parse(fs.readFileSync(path.join(ROOT, 'audit', 'anim_manifest.json'), 'utf8'));
      const outFile = path.join(ROOT, 'audit', 'anim_validation.json');
      writeJson(outFile, {
        generated_by: 'tools/anim_validate.js', version: VERSION, validator: 'gltf-validator ' + validator.version(),
        scope: a.sample && a.sample < JSON.parse(fs.readFileSync(path.join(ROOT, 'audit', 'anim_export.json'), 'utf8')).filter((r) => r.converted_path).length ? `sample of ${rows.length} files (seed ${a.seed}) from audit/anim_export.json` : 'every file in audit/anim_export.json',
        gltf_validator: { totals, per_key_format: per, issue_codes: codes },
        independent_glb_check: check,
        roundtrip_from_manifest: { manifest: 'audit/anim_manifest.json', rows_sha256: man.rows_sha256, exports: man.exports, roundtrip_matches: man.roundtrip_matches, failures: man.failures_count, classes: man.classes, max_quat_norm_deviation_before_normalising: man.sequence_totals.max_quat_norm_deviation, frame_tables: man.sequence_stats.frame_tables },
        files_with_findings: bad,
        files: files.map((r) => ({ file: r.file, bytes: r.bytes, errors: r.errors, warnings: r.warnings, infos: r.infos, hints: r.hints, check_ok: r.check && r.check.ok })),
      });
      logRun({ tool: 'anim_validate', version: VERSION, outputs: [outFile], note: `${validator.version()}: ${totals.files} files, ${totals.files_without_errors_or_warnings} without errors or warnings, ${totals.errors} errors, ${totals.warnings} warnings; independent check ok on ${check.files_ok}` });
      console.log(JSON.stringify({ validator: validator.version(), totals, per_key_format: per, issue_codes: codes, independent_glb_check: check }, null, 1));
      for (const r of bad.slice(0, 10)) console.log(r.file, JSON.stringify(r.issues || r.validator_failed), JSON.stringify(r.first_message_per_code || {}), JSON.stringify(r.check || {}));
    });
  }
}

const args = parseArgs(process.argv.slice(2));
if (args.shard) worker(args); else parent(args);
