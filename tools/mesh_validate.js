// Phase 4: independent check of exported .glb files with the Khronos glTF-Validator
// (npm package gltf-validator, a devDependency). The validator shares no code with
// tools/lib/mesh.js: it parses each file from scratch and checks it against the glTF 2.0
// specification, including accessor data (index ranges, declared min/max, unit normals and
// tangents, weight sums, inverse bind matrices).
//
//   node tools/mesh_validate.js [--sample N] [--seed S] [--class StaticMesh|SkeletalMesh] [files or directories ...]
//
// Without paths it reads audit/mesh_export.json and validates a deterministic sample of N files
// per class (default 20), always including the largest and the smallest file of each class.
// Writes audit/mesh_validation.json.
//
//   node tools/mesh_validate.js --all [--budget SECONDS] [--workers N]
//
// Validates every file listed in audit/mesh_export.json with up to 3 worker processes. The run is
// resumable: per-file results accumulate in build/mesh_tmp/validate_all_<k>.jsonl, a run stops
// taking new files after the time budget (default 520 s), and the run that finds every file done
// writes audit/mesh_validation_all.json.
const fs = require('fs');
const path = require('path');
const validator = require('gltf-validator');
const { ROOT, logRun, writeJson } = require('./lib/log');

const VERSION = '1.1.0';
const { spawn } = require('child_process');
const TMP = path.join(ROOT, 'build', 'mesh_tmp');
const SEVERITY = ['error', 'warning', 'info', 'hint'];

async function validateFile(file) {
  const data = new Uint8Array(fs.readFileSync(file));
  const rep = await validator.validateBytes(data, { uri: path.basename(file), maxIssues: 0, writeTimestamp: false });
  const issues = {}, first = {};
  for (const m of rep.issues.messages) { const k = SEVERITY[m.severity] + ':' + m.code; issues[k] = (issues[k] || 0) + 1; if (m.severity < 2 && !first[m.code]) first[m.code] = ((m.pointer || '') + ' ' + m.message).trim(); }
  const r = { bytes: data.length, errors: rep.issues.numErrors, warnings: rep.issues.numWarnings, infos: rep.issues.numInfos, hints: rep.issues.numHints, issues };
  if (Object.keys(first).length) r.first_message_per_code = first;
  return r;
}

function allRows() {
  return JSON.parse(fs.readFileSync(path.join(ROOT, 'audit', 'mesh_export.json'), 'utf8')).filter((r) => r.converted_path)
    .sort((x, y) => y.glb_bytes - x.glb_bytes || (x.converted_path < y.converted_path ? -1 : 1));
}
const readLines = (file) => (fs.existsSync(file) ? fs.readFileSync(file, 'utf8').split('\n').filter(Boolean).map((l) => JSON.parse(l)) : []);

async function allWorker(a) {
  const [k, n] = a.shard;
  const out = path.join(TMP, `validate_all_${k}.jsonl`);
  const done = new Set(readLines(out).map((r) => r.file));
  const stopAt = Date.now() + a.budget * 1000;
  const rows = allRows();
  for (let i = k; i < rows.length; i += n) {
    const row = rows[i];
    if (done.has(row.converted_path)) continue;
    if (Date.now() > stopAt) break;
    let r;
    try { r = await validateFile(path.join(ROOT, row.converted_path)); } catch (e) { r = { validator_failed: String(e) }; }
    fs.appendFileSync(out, JSON.stringify({ file: row.converted_path, class: row.class, sha256_glb: row.sha256_glb_on_disk, ...r }) + '\n');
  }
}

function allParent(a) {
  fs.mkdirSync(TMP, { recursive: true });
  const n = a.workers;
  // the shard count is part of the resume state: results are keyed by file, so it only has to be stable
  let running = 0;
  const finish = () => {
    const rows = allRows();
    const got = new Map();
    for (let k = 0; k < 3; k++) for (const r of readLines(path.join(TMP, `validate_all_${k}.jsonl`))) got.set(r.file, r);
    const have = rows.filter((r) => got.has(r.converted_path) && got.get(r.converted_path).sha256_glb === r.sha256_glb_on_disk);
    if (have.length < rows.length) { console.log(`\n${have.length} of ${rows.length} files validated so far; run the same command again to continue`); return; }
    const files = rows.map((r) => got.get(r.converted_path)).sort((x, y) => (x.file < y.file ? -1 : 1));
    const per = {}, codes = {};
    const totals = { files: 0, bytes: 0, files_without_errors: 0, files_without_errors_or_warnings: 0, validator_failed: 0, errors: 0, warnings: 0, infos: 0, hints: 0 };
    for (const r of files) {
      const c = (per[r.class] = per[r.class] || { files: 0, files_without_errors: 0, files_without_errors_or_warnings: 0, validator_failed: 0, errors: 0, warnings: 0 });
      totals.files++; c.files++;
      if (r.validator_failed) { totals.validator_failed++; c.validator_failed++; continue; }
      totals.bytes += r.bytes; totals.errors += r.errors; totals.warnings += r.warnings; totals.infos += r.infos; totals.hints += r.hints; c.errors += r.errors; c.warnings += r.warnings;
      if (!r.errors) { totals.files_without_errors++; c.files_without_errors++; }
      if (!r.errors && !r.warnings) { totals.files_without_errors_or_warnings++; c.files_without_errors_or_warnings++; }
      for (const [code, cnt] of Object.entries(r.issues)) { const e = (codes[code] = codes[code] || { messages: 0, files: 0 }); e.messages += cnt; e.files++; }
    }
    const outFile = path.join(ROOT, 'audit', 'mesh_validation_all.json');
    writeJson(outFile, { generated_by: 'tools/mesh_validate.js', version: VERSION, validator: 'gltf-validator ' + validator.version(), scope: 'every file in audit/mesh_export.json', totals, per_class: per, issue_codes: codes, files });
    logRun({ tool: 'mesh_validate', version: VERSION, outputs: [outFile], note: `${validator.version()}: all ${totals.files} files, ${totals.files_without_errors} without errors, ${totals.files_without_errors_or_warnings} without errors or warnings, ${totals.validator_failed} validator failures` });
    console.log('\n' + JSON.stringify({ validator: validator.version(), totals, per_class: per, issue_codes: codes }, null, 1));
  };
  for (let k = 0; k < n; k++) {
    running++;
    const c = spawn(process.execPath, ['--max-old-space-size=8000', __filename, '--all', '--shard', `${k}/${n}`, '--budget', String(a.budget)], { stdio: ['ignore', 'inherit', 'inherit'] });
    c.on('close', () => { if (!--running) finish(); });
  }
}


function parseArgs(argv) {
  const a = { sample: 20, seed: 1, paths: [], cls: null, budget: 520, workers: 3 };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--sample') a.sample = +argv[++i];
    else if (argv[i] === '--seed') a.seed = +argv[++i];
    else if (argv[i] === '--class') a.cls = argv[++i];
    else if (argv[i] === '--all') a.all = true;
    else if (argv[i] === '--budget') a.budget = +argv[++i];
    else if (argv[i] === '--workers') a.workers = Math.min(3, Math.max(1, +argv[++i]));
    else if (argv[i] === '--shard') a.shard = argv[++i].split('/').map(Number);
    else a.paths.push(argv[i]);
  }
  return a;
}

function pickFiles(a) {
  if (a.paths.length) {
    const out = [];
    const walk = (p) => { if (fs.statSync(p).isDirectory()) for (const e of fs.readdirSync(p)) walk(path.join(p, e)); else if (p.toLowerCase().endsWith('.glb')) out.push({ file: p, cls: null }); };
    a.paths.forEach((p) => walk(path.resolve(p)));
    return out;
  }
  const rows = JSON.parse(fs.readFileSync(path.join(ROOT, 'audit', 'mesh_export.json'), 'utf8')).filter((r) => r.converted_path);
  const out = [];
  let state = a.seed >>> 0; // xorshift32: the sample is reproducible from the seed
  const rnd = () => { state ^= state << 13; state >>>= 0; state ^= state >>> 17; state ^= state << 5; state >>>= 0; return state / 4294967296; };
  for (const cls of [...new Set(rows.map((r) => r.class))].sort()) {
    if (a.cls && a.cls !== cls) continue;
    const list = rows.filter((r) => r.class === cls).sort((x, y) => x.glb_bytes - y.glb_bytes || (x.converted_path < y.converted_path ? -1 : 1));
    const chosen = new Set([0, list.length - 1]);
    while (chosen.size < Math.min(a.sample, list.length)) chosen.add(Math.floor(rnd() * list.length));
    for (const i of [...chosen].sort((x, y) => x - y)) out.push({ file: path.join(ROOT, list[i].converted_path), cls, row: list[i] });
  }
  return out;
}

async function main() {
  const a = parseArgs(process.argv.slice(2));
  if (a.all) return a.shard ? allWorker(a) : allParent({ ...a, workers: 3 });
  const files = pickFiles(a);
  const results = [];
  const totals = { files: 0, files_without_errors: 0, files_without_errors_or_warnings: 0, errors: 0, warnings: 0, infos: 0, hints: 0 };
  const codes = {};
  for (const f of files) {
    const data = new Uint8Array(fs.readFileSync(f.file));
    const rel = path.relative(ROOT, f.file).replace(/\\/g, '/');
    let rep;
    try { rep = await validator.validateBytes(data, { uri: path.basename(f.file), maxIssues: 0, writeTimestamp: false }); } catch (e) { results.push({ file: rel, class: f.cls, validator_failed: String(e) }); totals.files++; continue; }
    const byCode = {};
    for (const m of rep.issues.messages) { const k = `${['error', 'warning', 'info', 'hint'][m.severity]}:${m.code}`; byCode[k] = (byCode[k] || 0) + 1; codes[k] = (codes[k] || 0) + 1; }
    const examples = {};
    for (const m of rep.issues.messages) { const k = m.code; if (!examples[k]) examples[k] = `${m.pointer || ''} ${m.message}`.trim(); }
    results.push({
      file: rel, class: f.cls, bytes: data.length, errors: rep.issues.numErrors, warnings: rep.issues.numWarnings, infos: rep.issues.numInfos, hints: rep.issues.numHints,
      issues: byCode, first_message_per_code: examples, info: rep.info && { version: rep.info.version, generator: rep.info.generator, drawCallCount: rep.info.drawCallCount, totalVertexCount: rep.info.totalVertexCount, totalTriangleCount: rep.info.totalTriangleCount, maxInfluences: rep.info.maxInfluences, maxUVs: rep.info.maxUVs, hasSkins: rep.info.hasSkins, materialCount: rep.info.materialCount },
    });
    totals.files++; totals.errors += rep.issues.numErrors; totals.warnings += rep.issues.numWarnings; totals.infos += rep.issues.numInfos; totals.hints += rep.issues.numHints;
    if (!rep.issues.numErrors) totals.files_without_errors++;
    if (!rep.issues.numErrors && !rep.issues.numWarnings) totals.files_without_errors_or_warnings++;
  }
  const perClass = {};
  for (const r of results) { const c = (perClass[r.class || 'unknown'] = perClass[r.class || 'unknown'] || { files: 0, files_without_errors: 0, files_without_errors_or_warnings: 0 }); c.files++; if (r.errors === 0) c.files_without_errors++; if (r.errors === 0 && r.warnings === 0) c.files_without_errors_or_warnings++; }
  const outFile = path.join(ROOT, 'audit', a.paths.length ? 'mesh_validation.partial.json' : 'mesh_validation.json');
  writeJson(outFile, { generated_by: 'tools/mesh_validate.js', version: VERSION, validator: 'gltf-validator ' + validator.version(), sample_per_class: a.sample, seed: a.seed, totals, per_class: perClass, issue_codes: codes, files: results });
  logRun({ tool: 'mesh_validate', version: VERSION, outputs: [outFile], note: `${validator.version()}: ${totals.files} files, ${totals.files_without_errors} without errors, ${totals.errors} errors, ${totals.warnings} warnings` });
  console.log(JSON.stringify({ validator: validator.version(), totals, per_class: perClass, issue_codes: codes }, null, 1));
  for (const r of results) if (r.errors || r.warnings || r.validator_failed) console.log(r.file, JSON.stringify(r.issues || r.validator_failed), JSON.stringify(r.first_message_per_code || {}));
}
main();
