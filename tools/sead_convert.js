// Phase 4: SEAD audio exports (SQEXSEADSound / SQEXSEADMusic / SQEXSEADSoundBank) <-> the embedded
// bank file (.sab / .mab) + sidecar JSON, with a round-trip proof per export.
//
//   node tools/sead_convert.js verify [--prefix <mounted path prefix>] [--workers N]
//       In memory: split each export into bank bytes + sidecar, rebuild the export from those two
//       and compare. Writes audit/sead_manifest.json and audit/sead_summary.json.
//   node tools/sead_convert.js export [--prefix ...] [--workers N]
//       Same, but writes <name>.sab|.mab and <name>.sead.json under assets_converted/audio/, reads
//       them back and rebuilds from the files on disk. Writes audit/sead_export.json (+ summary).
//
// The bank is copied out unmodified (a container conversion); its HCA streams are not decoded.
// Nothing is read from or written to assets/. Format: docs/formats/sead_audio.md.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { spawn } = require('child_process');
const { ROOT, logRun, writeJson } = require('./lib/log');
const pak = require('./lib/pak');
const ua = require('./lib/uasset');
const sead = require('./lib/sead');

const VERSION = '1.1.0';
const OUT_ROOT = path.join(ROOT, 'assets_converted', 'audio');
const TMP = path.join(ROOT, 'build', 'sead_tmp');
const sha256 = (...bufs) => { const h = crypto.createHash('sha256'); for (const b of bufs) h.update(b); return h.digest('hex'); };
const bump = (stats, key, value, by = 1) => { const m = (stats[key] = stats[key] || {}); m[value] = (m[value] || 0) + by; };

function parseArgs(argv) {
  const a = { mode: argv[0], prefix: '', workers: 4 };
  for (let i = 1; i < argv.length; i++) {
    if (argv[i] === '--prefix') a.prefix = argv[++i];
    else if (argv[i] === '--workers') a.workers = Math.min(4, Math.max(1, +argv[++i]));
    else if (argv[i] === '--shard') a.shard = argv[++i].split('/').map(Number);
    else if (argv[i] === '--out') a.out = argv[++i];
    else throw new Error('unknown argument ' + argv[i]);
  }
  if (a.mode !== 'verify' && a.mode !== 'export') throw new Error('usage: sead_convert.js verify|export [--prefix P] [--workers N]');
  return a;
}

function worker(args) {
  const files = pak.mountAll();
  const fds = new Map();
  const get = (p) => {
    const f = files.get(p);
    if (!f) return null;
    if (!fds.has(f.pak)) fds.set(f.pak, fs.openSync(f.pak, 'r'));
    return pak.extract(fds.get(f.pak), f.entry);
  };
  const classSet = new Set(sead.CLASSES);
  const needle = Buffer.from('SQEXSEAD', 'latin1');
  const pkgs = [...files.keys()].filter((p) => /\.(uasset|umap)$/.test(p) && p.startsWith(args.prefix)).sort();
  const [k, n] = args.shard;
  const out = fs.openSync(args.out, 'w');
  const stats = {};
  const usedOut = new Set();
  let seen = 0;

  for (let i = 0; i < pkgs.length; i++) {
    if ((i >> 6) % n !== k) continue;
    const p = pkgs[i];
    const head = get(p);
    if (!head.includes(needle)) continue;
    const pkg = ua.parseHeader(head);
    const hits = [];
    pkg.exports.forEach((e, idx) => { const c = ua.objectName(pkg, e.classIndex); if (classSet.has(c)) hits.push({ e, idx, cls: c }); });
    if (!hits.length) continue;
    const base = p.replace(/\.(uasset|umap)$/, '');
    const uexp = get(base + '.uexp');
    const rebuilt = new Map();
    const rows = [];
    for (const { e, idx, cls } of hits) {
      const name = ua.nameOf(pkg, e.objectName);
      const row = { package: p, export: name, export_index: idx + 1, class: cls };
      seen++;
      bump(stats, 'class', cls);
      try {
        if (!uexp) throw new Error('no .uexp');
        const { asset, data, start, end } = sead.parseExport(pkg, uexp, e, cls);
        const src = uexp.subarray(start, end);
        row.sha256_source_export = sha256(src);
        row.export_size = src.length;
        bump(stats, 'object_guid_present', String(!!asset.objectGuid));
        bump(stats, `trailing_bytes:${cls}`, asset.trailing.length / 2);
        bump(stats, `bank_embedded:${cls}`, String(!!asset.bank));
        if (asset.preamble) { bump(stats, 'preamble_cooked', asset.preamble.cooked); bump(stats, 'preamble_unknown', asset.preamble.unknown); }
        for (const { tag } of asset.props) bump(stats, `property:${cls}`, `${tag.name}:${tag.type}`);
        let ext = null;
        if (asset.bank) {
          const b = asset.bank;
          const s = sead.surveyBank(data);
          ext = sead.MAGIC_EXT[s.magic];
          if (!ext) throw new Error('unknown bank magic ' + JSON.stringify(s.magic));
          Object.assign(row, { bank_magic: s.magic, bank_record_version: b.version,bank_bytes: b.size, bank_name: s.name, materials: s.materials.length });
          row.sha256_bank = sha256(data);
          bump(stats, 'bank_record_version_vs_magic', `${b.version}:${s.magic}`);
          bump(stats, 'bank_record_reserved', `version ${b.version}: ${b.reserved}`);
          bump(stats, 'bank_bulk_flags', '0x' + b.bulkFlags.toString(16));
          bump(stats, 'bank_offset_rule', String(b.offsetRuleOk));
          bump(stats, 'bank_size_field_matches', String(s.sizeFieldMatches));
          bump(stats, `bank_header:${s.magic}`, `version ${s.version} byte5 ${s.byte5} headerSize ${s.headerSize} sections ${s.sectionCount}`);
          bump(stats, 'bank_name_length', s.nameLength);
          bump(stats, 'bank_name_terminated', String(s.nameTerminated));
          bump(stats, 'bank_first_section_offset_is_end_of_table', String(s.sections.length > 0 && s.sections[0].offset === s.tableOffset + 16 * s.sectionCount));
          bump(stats, `bank_sections:${s.magic}`, s.sections.map((x) => x.tag).join(','));
          for (const x of s.sections) { bump(stats, 'bank_section_word', '0x' + x.word.toString(16)); bump(stats, 'bank_section_pad', x.pad); }
          bump(stats, 'bank_material_chain', String(s.materialChainOk));
          bump(stats, 'bank_bytes', s.magic, b.size);
          for (const m of s.materials) {
            if (m.bad) { bump(stats, 'material', 'entry outside bank'); continue; }
            bump(stats, 'material_codec', m.codec);
            bump(stats, 'material_version_headersize', `${m.version}:${m.headerSize}`);
            if (m.codec !== 0) {
              bump(stats, 'material_channels', m.channels);
              bump(stats, 'material_sample_rate', m.sampleRate);
              bump(stats, 'material_looped', String(m.loopEnd !== 0));
              bump(stats, 'material_stream_bytes', 'codec ' + m.codec, m.streamSize);
            }
            if (m.codec === 7) {
              bump(stats, 'hca_magic', JSON.stringify(m.hcaMagic));
              bump(stats, 'hca_version', '0x' + m.hcaVersion.toString(16));
              bump(stats, 'hca_ciph_type', String(m.hcaCiphType));
              bump(stats, 'hca_header_is_extra_minus_16', String(m.hcaHeaderSize === m.extraSize - 16));
            }
          }
        }
        // ---- forward
        let bankFile = data;
        let sidecarText = JSON.stringify(sead.sidecarOf(asset, { package: p, export: name, class: cls, converter: 'tools/sead_convert.js', converter_version: VERSION }), null, 1) + '\n';
        if (args.mode === 'export') {
          let relOut = base + (path.posix.basename(base) === name ? '' : '.' + name);
          if (usedOut.has(relOut.toLowerCase())) relOut += '.' + (idx + 1);
          usedOut.add(relOut.toLowerCase());
          const jsonPath = path.join(OUT_ROOT, relOut + '.sead.json');
          fs.mkdirSync(path.dirname(jsonPath), { recursive: true });
          fs.writeFileSync(jsonPath, sidecarText);
          sidecarText = fs.readFileSync(jsonPath, 'utf8');
          row.sidecar_path = 'assets_converted/audio/' + relOut + '.sead.json';
          row.sha256_sidecar = sha256(Buffer.from(sidecarText));
          if (data) {
            const bankPath = path.join(OUT_ROOT, relOut + '.' + ext);
            fs.writeFileSync(bankPath, data);
            bankFile = fs.readFileSync(bankPath);
            row.converted_path = 'assets_converted/audio/' + relOut + '.' + ext;
            row.sha256_bank_on_disk = sha256(bankFile);
          }
        }
        // ---- reverse
        const back = sead.rebuild(pkg, JSON.parse(sidecarText), bankFile);
        row.sha256_rebuilt_export = sha256(back);
        row.roundtrip_matches = back.equals(src);
        row.reversible = row.roundtrip_matches;
        if (!row.roundtrip_matches) row.error = 'rebuilt bytes differ from the original';
        else rebuilt.set(idx, back);
      } catch (err) {
        row.roundtrip_matches = false;
        row.reversible = false;
        row.error = String(err.message);
      }
      bump(stats, 'roundtrip', row.roundtrip_matches ? 'matches' : 'fails');
      bump(stats, `roundtrip:${cls}`, row.roundtrip_matches ? 'matches' : 'fails');
      if (row.error) bump(stats, 'errors', row.error.replace(/\d+/g, 'N'));
      rows.push(row);
    }
    // whole-.uexp proof when every export of the package was rebuilt
    const pf = {};
    if (uexp) {
      pf.sha256_uexp = sha256(uexp);
      if (rebuilt.size === pkg.exports.length) {
        const order = pkg.exports.map((e, idx) => ({ e, idx })).sort((a, b) => a.e.serialOffset - b.e.serialOffset);
        const tag = Buffer.alloc(4); tag.writeUInt32LE(ua.TAG);
        const again = Buffer.concat([...order.map(({ idx }) => rebuilt.get(idx)), tag]);
        pf.sha256_uexp_rebuilt = sha256(again);
        pf.uexp = again.equals(uexp) ? 'identical' : 'differs';
      } else pf.uexp = rebuilt.size ? 'other_exports_present' : 'not_rebuilt';
    } else pf.uexp = 'absent';
    pf.ubulk = files.has(base + '.ubulk') ? 'present_not_used' : 'absent';
    bump(stats, 'package_files', `uexp ${pf.uexp} / ubulk ${pf.ubulk}`);
    for (const row of rows) { row.package_files = pf; row.converter = 'tools/sead_convert.js'; row.converter_version = VERSION; fs.writeSync(out, JSON.stringify(row) + '\n'); }
  }
  fs.closeSync(out);
  for (const fd of fds.values()) fs.closeSync(fd);
  process.stdout.write('RESULT ' + JSON.stringify({ shard: k, exports: seen, stats }) + '\n');
}

function main(args) {
  fs.mkdirSync(TMP, { recursive: true });
  const n = args.workers;
  const results = [], errors = [];
  let running = 0;
  const started = Date.now();
  const finish = () => {
    const stats = {};
    for (const r of results) for (const [key, m] of Object.entries(r.stats)) for (const [v, c] of Object.entries(m)) bump(stats, key, v, c);
    for (const key of Object.keys(stats)) stats[key] = Object.fromEntries(Object.entries(stats[key]).sort((a, b) => b[1] - a[1]));
    const rows = [];
    for (let k = 0; k < n; k++) {
      const f = path.join(TMP, `${args.mode}_${k}.jsonl`);
      if (!fs.existsSync(f)) continue;
      for (const line of fs.readFileSync(f, 'utf8').split('\n')) if (line) rows.push(line);
      fs.rmSync(f);
    }
    const keyOf = (line) => { const r = JSON.parse(line); return r.package + '\0' + String(r.export_index).padStart(8, '0'); };
    const keyed = rows.map((l) => [keyOf(l), l]).sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0));
    const suffix = !args.prefix && errors.length === 0 ? '' : '.partial';
    const rowsFile = path.join(ROOT, 'audit', `${args.mode === 'verify' ? 'sead_manifest' : 'sead_export'}${suffix}.json`);
    const sumFile = path.join(ROOT, 'audit', `${args.mode === 'verify' ? 'sead_summary' : 'sead_export_summary'}${suffix}.json`);
    const fd = fs.openSync(rowsFile, 'w');
    fs.writeSync(fd, '[\n');
    keyed.forEach(([, l], i) => fs.writeSync(fd, l + (i + 1 < keyed.length ? ',\n' : '\n')));
    fs.writeSync(fd, ']\n');
    fs.closeSync(fd);
    const summary = {
      generated_by: 'tools/sead_convert.js', version: VERSION, mode: args.mode, prefix: args.prefix, classes: sead.CLASSES,
      exports: keyed.length, roundtrip_matches: (stats.roundtrip || {}).matches || 0, roundtrip_fails: (stats.roundtrip || {}).fails || 0,
      rows: path.relative(ROOT, rowsFile).replace(/\\/g, '/'), rows_sha256: sha256(fs.readFileSync(rowsFile)),
      seconds: Math.round((Date.now() - started) / 1000), worker_errors: errors, stats,
    };
    writeJson(sumFile, summary);
    logRun({ tool: 'sead_convert', version: VERSION, outputs: [rowsFile, sumFile], note: `${args.mode}: ${keyed.length} exports, ${summary.roundtrip_matches} round-trip, ${summary.roundtrip_fails} fail, ${errors.length} worker errors` });
    console.log('\n' + JSON.stringify(summary, null, 1));
  };
  for (let k = 0; k < n; k++) {
    running++;
    const argv = [__filename, args.mode, '--shard', `${k}/${n}`, '--out', path.join(TMP, `${args.mode}_${k}.jsonl`)];
    if (args.prefix) argv.push('--prefix', args.prefix);
    const c = spawn(process.execPath, ['--max-old-space-size=6000', ...argv], { stdio: ['ignore', 'pipe', 'pipe'] });
    let out = '', err = '';
    c.stdout.on('data', (d) => (out += d));
    c.stderr.on('data', (d) => (err += d));
    c.on('close', (code) => {
      const m = /^RESULT (.*)$/m.exec(out);
      if (code === 0 && m) results.push(JSON.parse(m[1]));
      else errors.push({ shard: k, code, error: err.slice(-600) });
      process.stdout.write(`\rworkers done: ${results.length + errors.length}/${n}  `);
      if (!--running) finish();
    });
  }
}

const args = parseArgs(process.argv.slice(2));
if (args.shard) worker(args);
else main(args);
