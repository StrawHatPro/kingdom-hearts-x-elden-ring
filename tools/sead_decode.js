// SEAD audio decoder CLI: material streams of .sab/.mab banks -> WAV (or .ogg for Vorbis).
//
//   node tools/sead_decode.js info   <bank>
//       Materials of one bank: codec, channels, rate, samples, loop.
//   node tools/sead_decode.js decode <bank> [--material N] [--out DIR] [--float]
//       Writes <out>/<bank name>_m<NNN>.wav (PCM s16, or float32 with --float; loop in a smpl
//       chunk). Vorbis materials are written as the Ogg stream they are (.ogg). Default out:
//       build/sora_tmp/audio/decoded.
//   node tools/sead_decode.js verify [--prefix <path under assets_converted/audio>] [--workers N]
//       Decodes every material of every bank (no files written) and records, per material, what
//       was checked: audit/sead_decode.json and audit/sead_decode_summary.json.
//   node tools/sead_decode.js tables
//       Re-reads the codec tables from the exe and compares with the cached copy.
//
// Decoders: tools/lib/hca.js (CRI HCA 2.0), tools/lib/msadpcm.js, tools/lib/ogg.js (container
// check only), tools/lib/sead_material.js. Format: docs/formats/sead_audio.md.
const fs = require('fs');
const path = require('path');
const { spawn } = require('child_process');
const { ROOT, logRun, writeJson } = require('./lib/log');
const sead = require('./lib/sead');
const mat = require('./lib/sead_material');
const wav = require('./lib/wav');
const hca = require('./lib/hca');
const vorbis = require('./lib/vorbis');

const VERSION = '1.1.0';
const AUDIO_ROOT = path.join(ROOT, 'assets_converted', 'audio');
const TMP = path.join(ROOT, 'build', 'sead_tmp');
const bump = (stats, key, value, by = 1) => { const m = (stats[key] = stats[key] || {}); m[value] = (m[value] || 0) + by; };

function parseArgs(argv) {
  const a = { mode: argv[0], pos: [], workers: 6, prefix: '' };
  for (let i = 1; i < argv.length; i++) {
    const k = argv[i];
    if (k === '--material') a.material = +argv[++i];
    else if (k === '--out') a.out = argv[++i];
    else if (k === '--float') a.float = true;
    else if (k === '--prefix') a.prefix = argv[++i];
    else if (k === '--workers') a.workers = Math.max(1, Math.min(12, +argv[++i]));
    else if (k === '--shard') a.shard = argv[++i].split('/').map(Number);
    else if (k === '--list') a.list = argv[++i];
    else if (k.startsWith('--')) throw new Error('unknown argument ' + k);
    else a.pos.push(k);
  }
  return a;
}

function listBanks(prefix) {
  const out = [];
  const walk = (d) => { for (const e of fs.readdirSync(d, { withFileTypes: true })) { const p = path.join(d, e.name); if (e.isDirectory()) walk(p); else if (/\.(sab|mab)$/.test(e.name)) out.push(path.relative(AUDIO_ROOT, p).split(path.sep).join('/')); } };
  walk(AUDIO_ROOT);
  return out.filter((p) => p.startsWith(prefix)).sort();
}

function signalStats(channels) {
  let peak = 0, sum = 0, n = 0, bad = 0;
  for (const c of channels) for (let i = 0; i < c.length; i++) { const v = c[i]; if (!Number.isFinite(v)) { bad++; continue; } const a = v < 0 ? -v : v; if (a > peak) peak = a; sum += v * v; n++; }
  return { peak: +peak.toFixed(5), rms: +Math.sqrt(sum / Math.max(1, n)).toFixed(5), nonFinite: bad };
}

function info(file) {
  const d = fs.readFileSync(file);
  const s = sead.surveyBank(d);
  console.log(`${s.magic} "${s.name}" ${d.length} bytes, sections ${s.sections.map((x) => x.tag).join(',')}, ${s.materials.length} materials`);
  s.materials.forEach((m, i) => {
    const x = mat.describe(d, m);
    let extra = '';
    if (m.codec === 7) { const h = hca.parseHeader(d, m.entryOffset + m.headerSize + x.extraHeaderSize); extra = ` frames ${h.frameCount}x${h.frameSize} samples ${h.sampleCount} (${(h.sampleCount / h.sampleRate).toFixed(3)} s) delay ${h.encoderDelay}${x.obfuscated ? ' obfuscated seed ' + x.keySeed : ''}`; }
    else if (m.codec === 2) extra = ` block ${x.blockAlign}`;
    else if (m.codec === 3) extra = ` samples ${x.sampleCount}`;
    console.log(`  ${String(i).padStart(3)} ${x.codecName.padEnd(7)} ${m.channels}ch ${m.sampleRate} Hz stream ${m.streamSize}${x.looped ? ` loop ${x.loopStart}-${x.loopEnd}` : ''}${extra}`);
  });
}

async function decodeOne(file, a) {
  const d = fs.readFileSync(file);
  const s = sead.surveyBank(d);
  const outDir = a.out || path.join(ROOT, 'build', 'sora_tmp', 'audio', 'decoded');
  fs.mkdirSync(outDir, { recursive: true });
  const base = path.basename(file).replace(/\.(sab|mab)$/, '');
  for (let i = 0; i < s.materials.length; i++) {
    const m = s.materials[i];
    if (m.codec === 0 || (a.material !== undefined && a.material !== i)) continue;
    const r = mat.decodeMaterial(d, m);
    const name = `${base}_m${String(i).padStart(3, '0')}`;
    if (r.kind === 'ogg') {
      // the stream as stored, plus its PCM through the vendored libvorbis build
      fs.writeFileSync(path.join(outDir, name + '.ogg'), r.ogg);
      console.log(`${name}.ogg  vorbis ${m.channels}ch ${r.sampleCount} samples (stream as stored)`);
      const v = await vorbis.decode(r.ogg);
      if (v.errors.length || v.sampleCount !== r.sampleCount) r.issues.push(`vorbis decode: ${v.sampleCount} samples, errors ${JSON.stringify(v.errors)}`);
      r.channels = v.channels; r.sampleRate = v.sampleRate; r.sampleCount = v.sampleCount;
    }
    const w = wav.encode({ channels: r.channels, sampleRate: r.sampleRate, float: !!a.float, loop: r.loop });
    fs.writeFileSync(path.join(outDir, name + '.wav'), w.buffer);
    const st = signalStats(r.channels);
    console.log(`${name}.wav  ${r.info.codecName} ${m.channels}ch ${r.sampleRate} Hz ${r.sampleCount} samples (${(r.sampleCount / r.sampleRate).toFixed(3)} s) peak ${st.peak}${r.loop ? ` loop ${r.loop.start}-${r.loop.end}` : ''}${w.clipped ? ` clipped ${w.clipped}` : ''}${r.issues.length ? '  ISSUES: ' + r.issues.join('; ') : ''}`);
  }
}

// ------------------------------------------------------------------ verify

const peakBin = (p) => (p === 0 ? 'silent' : p <= 0.5 ? '<=0.5' : p <= 0.9 ? '<=0.9' : p <= 1.0 ? '<=1.0' : p <= 1.1 ? '<=1.1' : p <= 2 ? '<=2' : '>2');

async function verifyWorker(a) {
  const banks = JSON.parse(fs.readFileSync(a.list, 'utf8'));
  const [k, n] = a.shard;
  const out = fs.openSync(a.out, 'w');
  const stats = {};
  let materials = 0;
  for (let i = 0; i < banks.length; i++) {
    if (i % n !== k) continue;
    const rel = banks[i];
    let d, s;
    try { d = fs.readFileSync(path.join(AUDIO_ROOT, rel)); s = sead.surveyBank(d); } catch (e) { fs.writeSync(out, JSON.stringify({ bank: rel, error: e.message }) + '\n'); bump(stats, 'bank', 'unreadable'); continue; }
    bump(stats, 'bank', s.magic);
    for (let mi = 0; mi < s.materials.length; mi++) {
      const m = s.materials[mi];
      materials++;
      const row = { bank: rel, material: mi, codec: m.codec };
      if (m.bad) { row.ok = false; row.issues = ['entry outside bank']; fs.writeSync(out, JSON.stringify(row) + '\n'); bump(stats, 'result', 'fail'); continue; }
      try {
        const r = mat.decodeMaterial(d, m);
        Object.assign(row, { channels: m.channels, sample_rate: m.sampleRate, samples: r.sampleCount, loop: r.loop ? [r.loop.start, r.loop.end] : null });
        bump(stats, 'codec', r.info.codecName);
        if (m.codec === 0) { row.ok = true; }
        else {
          bump(stats, 'channels', m.channels); bump(stats, 'sample_rate', m.sampleRate);
          bump(stats, 'seconds:' + r.info.codecName, 'total', r.sampleCount / r.sampleRate);
          if (r.kind === 'pcm') {
            const st = signalStats(r.channels);
            Object.assign(row, st);
            if (st.nonFinite) r.issues.push('non-finite samples');
            if (st.peak > 2) r.issues.push('peak above 2.0');
            bump(stats, 'peak:' + r.info.codecName, st.peak === 0 ? 'silent' : st.peak <= 0.5 ? '<=0.5' : st.peak <= 0.9 ? '<=0.9' : st.peak <= 1.0 ? '<=1.0' : st.peak <= 1.1 ? '<=1.1' : st.peak <= 2 ? '<=2' : '>2');
          }
          if (m.codec === 7) {
            row.frames = r.info.hca.frameCount; row.frame_size = r.info.hca.frameSize; row.obfuscated = r.info.obfuscated;
            row.max_bits_used = r.stats.maxBitsUsed;
            bump(stats, 'hca_obfuscated', String(r.info.obfuscated));
            bump(stats, 'hca_frames', 'total', r.info.hca.frameCount);
            bump(stats, 'hca_frame_crc', r.stats.crcBad ? 'some bad' : 'all ok');
            bump(stats, 'hca_bit_budget', r.stats.maxBitsUsed === r.info.hca.frameSize * 8 - 16 ? 'a frame ends exactly at its CRC' : r.stats.maxBitsUsed < r.info.hca.frameSize * 8 - 16 ? 'all frames end before the CRC' : 'overrun');
            bump(stats, 'hca_loop', r.loop ? 'looped' : 'one-shot');
            if (r.loop) { bump(stats, 'hca_loop_end_is_sample_count', String(r.loop.end === r.sampleCount)); bump(stats, 'hca_loop_minus_mtrl_fields', r.info.loopDelta ? r.info.loopDelta.join('/') : 'no hca loop chunk'); }
          } else if (m.codec === 2) { row.block_align = r.info.blockAlign; row.boundary_vs_inner_step = r.stats.boundaryVsInnerStep; bump(stats, 'msadpcm_block', `${m.channels}ch block ${r.info.blockAlign}`); if (r.loop) { bump(stats, 'msadpcm_samples_after_loop_end', String(r.sampleCount - r.loop.end)); bump(stats, 'msadpcm_loop_minus_mtrl_fields', r.info.loopDelta.join('/')); } }
          else if (m.codec === 3) {
            const v = await vorbis.decode(r.ogg);
            const st = signalStats(v.channels);
            Object.assign(row, st, { decoded_samples: v.sampleCount });
            if (v.errors.length) r.issues.push('vorbis decode errors: ' + v.errors.join('; '));
            if (v.sampleCount !== r.sampleCount) r.issues.push(`vorbis decoder returned ${v.sampleCount} samples, last granule is ${r.sampleCount}`);
            if (v.channels.length !== m.channels || v.sampleRate !== m.sampleRate) r.issues.push('vorbis decoder channel/rate differ');
            if (st.nonFinite) r.issues.push('non-finite samples');
            bump(stats, 'vorbis_decoded_samples_equal_granule', String(v.sampleCount === r.sampleCount));
            bump(stats, 'peak:vorbis', peakBin(st.peak));
            bump(stats, 'vorbis_container', r.info.ogg.ok ? 'ok' : 'bad'); bump(stats, 'vorbis_length_rule', r.info.lengthRule); bump(stats, 'vorbis_extra_version', r.info.extraVersion); bump(stats, 'vorbis_blocksizes', r.info.ogg.vorbis ? r.info.ogg.vorbis.blockSize0 + '/' + r.info.ogg.vorbis.blockSize1 : '?');
          }
          row.ok = r.issues.length === 0;
          if (r.issues.length) row.issues = r.issues;
          if (r.notes && r.notes.length) { row.notes = r.notes; for (const x of r.notes) bump(stats, 'notes', x.replace(/[0-9]+/g, 'N').slice(0, 100)); }
        }
      } catch (e) { row.ok = false; row.issues = ['exception: ' + e.message]; }
      bump(stats, 'result', row.ok ? 'ok' : 'fail');
      if (!row.ok) for (const x of row.issues) bump(stats, 'issues', x.replace(/\d+/g, 'N').slice(0, 100));
      fs.writeSync(out, JSON.stringify(row) + '\n');
    }
  }
  fs.closeSync(out);
  process.stdout.write('RESULT ' + JSON.stringify({ shard: k, materials, stats }) + '\n');
}

function verifyMain(a) {
  fs.mkdirSync(TMP, { recursive: true });
  hca.tables(); // make sure the table cache exists before the workers start
  const banks = listBanks(a.prefix);
  const listFile = path.join(TMP, 'decode_banks.json');
  fs.writeFileSync(listFile, JSON.stringify(banks));
  const n = a.workers, results = [], errors = [];
  let running = 0;
  const started = Date.now();
  const finish = () => {
    const stats = {};
    for (const r of results) for (const [key, m] of Object.entries(r.stats)) for (const [v, c] of Object.entries(m)) bump(stats, key, v, c);
    for (const key of Object.keys(stats)) stats[key] = Object.fromEntries(Object.entries(stats[key]).sort((x, y) => y[1] - x[1]));
    const lines = [];
    for (let k = 0; k < n; k++) { const f = path.join(TMP, `decode_${k}.jsonl`); if (!fs.existsSync(f)) continue; for (const l of fs.readFileSync(f, 'utf8').split('\n')) if (l) lines.push(l); fs.rmSync(f); }
    const keyed = lines.map((l) => { const r = JSON.parse(l); return [r.bank + '\0' + String(r.material || 0).padStart(6, '0'), l]; }).sort((x, y) => (x[0] < y[0] ? -1 : x[0] > y[0] ? 1 : 0));
    const suffix = !a.prefix && !errors.length ? '' : '.partial';
    const rowsFile = path.join(ROOT, 'audit', `sead_decode${suffix}.json`), sumFile = path.join(ROOT, 'audit', `sead_decode_summary${suffix}.json`);
    const fd = fs.openSync(rowsFile, 'w');
    fs.writeSync(fd, '[\n'); keyed.forEach(([, l], i) => fs.writeSync(fd, l + (i + 1 < keyed.length ? ',\n' : '\n'))); fs.writeSync(fd, ']\n'); fs.closeSync(fd);
    const summary = { generated_by: 'tools/sead_decode.js', version: VERSION, prefix: a.prefix, banks: banks.length, materials: keyed.length, ok: (stats.result || {}).ok || 0, fail: (stats.result || {}).fail || 0, rows: path.relative(ROOT, rowsFile).replace(/\\/g, '/'), seconds: Math.round((Date.now() - started) / 1000), worker_errors: errors, stats };
    writeJson(sumFile, summary);
    logRun({ tool: 'sead_decode', version: VERSION, outputs: [rowsFile, sumFile], note: `verify: ${banks.length} banks, ${keyed.length} materials, ${summary.ok} ok, ${summary.fail} fail` });
    console.log('\n' + JSON.stringify(summary, null, 1));
  };
  for (let k = 0; k < n; k++) {
    running++;
    const c = spawn(process.execPath, ['--max-old-space-size=4000', __filename, 'verify', '--shard', `${k}/${n}`, '--list', listFile, '--out', path.join(TMP, `decode_${k}.jsonl`)], { stdio: ['ignore', 'pipe', 'pipe'] });
    let out = '', err = '';
    c.stdout.on('data', (x) => (out += x)); c.stderr.on('data', (x) => (err += x));
    c.on('close', (code) => {
      const m = /^RESULT (.*)$/m.exec(out);
      if (code === 0 && m) results.push(JSON.parse(m[1])); else errors.push({ shard: k, code, error: err.slice(-600) });
      process.stdout.write(`\rworkers done: ${results.length + errors.length}/${n}  `);
      if (!--running) finish();
    });
  }
}

function tablesCheck() {
  const fresh = hca.extractTables();
  const cachedText = fs.existsSync(hca.TABLE_FILE) ? fs.readFileSync(hca.TABLE_FILE, 'utf8') : null;
  if (!cachedText) { hca.tables(); console.log('table cache written: ' + path.relative(ROOT, hca.TABLE_FILE)); return; }
  const same = JSON.stringify(JSON.parse(cachedText)) === JSON.stringify(fresh);
  for (const [name, t] of Object.entries(fresh.tables)) console.log(`${name.padEnd(10)} ${t.va} ${t.type} x${t.values.length}`);
  console.log(same ? 'cached tables equal the exe' : 'CACHED TABLES DIFFER FROM THE EXE');
  if (!same) process.exitCode = 1;
}

const a = parseArgs(process.argv.slice(2));
const resolveBank = (p) => (fs.existsSync(p) ? p : path.join(AUDIO_ROOT, p));
if (a.mode === 'info' && a.pos[0]) info(resolveBank(a.pos[0]));
else if (a.mode === 'decode' && a.pos[0]) decodeOne(resolveBank(a.pos[0]), a).catch((e) => { console.error(e); process.exitCode = 1; });
else if (a.mode === 'verify') { if (a.shard) verifyWorker(a).catch((e) => { console.error(e); process.exit(1); }); else verifyMain(a); }
else if (a.mode === 'tables') tablesCheck();
else { console.error('usage: sead_decode.js info <bank> | decode <bank> [--material N] [--out DIR] [--float] | verify [--prefix P] [--workers N] | tables'); process.exitCode = 2; }
