// Phase 4: UTexture2D exports <-> .dds + sidecar JSON, with a round-trip proof per texture.
//
//   node tools/texture_convert.js verify [--prefix <mounted path prefix>] [--classes A,B] [--workers N]
//       Streams every texture from the paks, converts it to DDS + sidecar in memory, rebuilds the
//       export (and its .ubulk pieces) from those two alone and compares with the original bytes.
//       Writes audit/texture_manifest.json (one row per texture) and audit/texture_summary.json.
//   node tools/texture_convert.js export [--prefix ...] [--classes A,B] [--workers N]
//       Same, but writes <name>.dds and <name>.texture.json under assets_converted/textures/, reads
//       both files back from disk and rebuilds from what was read. Writes audit/texture_export.json
//       (+ audit/texture_export_summary.json).
//
// Nothing is read from or written to assets/. Format: docs/formats/texture.md.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { spawn } = require('child_process');
const { ROOT, logRun, writeJson } = require('./lib/log');
const pak = require('./lib/pak');
const ua = require('./lib/uasset');
const tx = require('./lib/texture');

const VERSION = '1.1.0';
// UTexture2D and the classes that serialize the same cooked platform data.
const DEFAULT_CLASSES = ['Texture2D', 'LightMapTexture2D', 'ShadowMapTexture2D', 'TextureLightProfile', 'TextureCube'];
const OUT_ROOT = path.join(ROOT, 'assets_converted', 'textures');
const TMP = path.join(ROOT, 'build', 'texture_tmp');
const sha256 = (...bufs) => { const h = crypto.createHash('sha256'); for (const b of bufs) h.update(b); return h.digest('hex'); };

function parseArgs(argv) {
  const a = { mode: argv[0], prefix: '', classes: DEFAULT_CLASSES, workers: 4 };
  for (let i = 1; i < argv.length; i++) {
    if (argv[i] === '--prefix') a.prefix = argv[++i];
    else if (argv[i] === '--classes') a.classes = argv[++i].split(',');
    else if (argv[i] === '--workers') a.workers = Math.min(4, Math.max(1, +argv[++i]));
    else if (argv[i] === '--shard') a.shard = argv[++i].split('/').map(Number);
    else if (argv[i] === '--out') a.out = argv[++i];
    else throw new Error('unknown argument ' + argv[i]);
  }
  if (a.mode !== 'verify' && a.mode !== 'export') throw new Error('usage: texture_convert.js verify|export [--prefix P] [--classes A,B] [--workers N]');
  return a;
}

const bump = (stats, key, value, by = 1) => { const m = (stats[key] = stats[key] || {}); m[value] = (m[value] || 0) + by; };

// ------------------------------------------------------------------ worker

function worker(args) {
  const files = pak.mountAll();
  const fds = new Map();
  const get = (p) => {
    const f = files.get(p);
    if (!f) return null;
    if (!fds.has(f.pak)) fds.set(f.pak, fs.openSync(f.pak, 'r'));
    return pak.extract(fds.get(f.pak), f.entry);
  };
  const classSet = new Set(args.classes);
  const needles = args.classes.map((c) => Buffer.from(c + '\0', 'latin1'));
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
    if (!needles.some((nd) => head.includes(nd))) continue;
    const pkg = ua.parseHeader(head);
    const texExports = [];
    pkg.exports.forEach((e, idx) => { const c = ua.objectName(pkg, e.classIndex); if (classSet.has(c)) texExports.push({ e, idx, cls: c }); });
    if (!texExports.length) continue;
    const base = p.replace(/\.(uasset|umap)$/, '');
    const uexp = get(base + '.uexp');
    let ubulk; // loaded on first use
    const getUbulk = () => (ubulk === undefined ? (ubulk = get(base + '.ubulk')) : ubulk);
    const hdr = pkg.summary.totalHeaderSize;
    const rows = [];
    const rebuiltExports = new Map(); // export index -> bytes
    const bulkPieces = [];
    bump(stats, 'packages', 'with_texture_exports');
    bump(stats, 'texture_exports_per_package', texExports.length);

    for (const { e, idx, cls } of texExports) {
      const name = ua.nameOf(pkg, e.objectName);
      const row = { package: p, export: name, export_index: idx + 1, class: cls };
      seen++;
      bump(stats, 'class', cls);
      try {
        if (!uexp) throw new Error('no .uexp');
        const { tex, start, end } = tx.parseExport(pkg, uexp, e);
        const src = uexp.subarray(start, end);
        row.sha256_source_export = sha256(src);
        row.export_offset_in_uexp = start;
        row.export_size = src.length;
        // ---- statistics on the parsed layout (drive docs/formats/texture.md)
        bump(stats, 'object_guid_present', String(!!tex.objectGuid));
        bump(stats, 'strip_flags_texture', tex.stripFlagsTexture.join(','));
        bump(stats, 'strip_flags_texture2d', tex.stripFlagsTexture2D.join(','));
        bump(stats, 'platform_entries', tex.platforms.length);
        bump(stats, `trailing_bytes:${cls}`, tex.trailing.length / 2);
        for (const { tag, value } of tex.props) {
          bump(stats, 'property', `${tag.name}:${tag.type}`);
          if (tag.type === 'ByteProperty' || tag.type === 'EnumProperty') bump(stats, `prop:${tag.name}`, typeof value === 'string' ? value : JSON.stringify(value));
          else if (tag.type === 'BoolProperty') bump(stats, `prop:${tag.name}`, String(tag.boolVal));
          else if (tag.type === 'IntProperty') bump(stats, `prop:${tag.name}`, String(value));
        }
        if (tex.platforms.length !== 1) throw new Error(`${tex.platforms.length} platform entries`);
        const pd = tex.platforms[0];
        const fmt = pd.pixelFormat.s;
        Object.assign(row, { format: fmt, size_x: pd.sizeX, size_y: pd.sizeY, num_slices: pd.numSlices, first_mip: pd.firstMip, num_mips: pd.mips.length });
        row.mips = pd.mips.map((m) => [m.sizeX, m.sizeY, m.sizeOnDisk, m.storage === 'ubulk' ? 'ubulk' : 'uexp', m.dataOffset]);
        bump(stats, 'format', fmt);
        bump(stats, `format:${cls}`, fmt);
        bump(stats, 'pixel_format_fname_equals_string', String(pd.pixelFormatName === fmt));
        bump(stats, 'pixel_format_string_encoding', pd.pixelFormat.enc);
        bump(stats, 'num_slices', pd.numSlices);
        bump(stats, 'first_mip', pd.firstMip);
        bump(stats, 'num_mips', pd.mips.length);
        bump(stats, 'skip_offset_rule', String(pd.skipOffsetRuleOk));
        bump(stats, 'mips_in_ubulk', pd.mips.filter((m) => m.storage === 'ubulk').length);
        bump(stats, 'mips_inline', pd.mips.filter((m) => m.storage === 'inline').length);
        if (pd.mips.length) {
          const m0 = pd.mips[0];
          bump(stats, 'platform_size_vs_mip0', pd.sizeX === m0.sizeX && pd.sizeY === m0.sizeY ? 'equal'
            : pd.sizeX === m0.sizeX << pd.firstMip && pd.sizeY === m0.sizeY << pd.firstMip ? 'mip0 << firstMip' : 'other');
          bump(stats, 'top_mip_size', `${m0.sizeX}x${m0.sizeY}`);
          const pow2 = (v) => (v & (v - 1)) === 0;
          bump(stats, 'top_mip_power_of_two', String(pow2(m0.sizeX) && pow2(m0.sizeY)));
        }
        pd.mips.forEach((m, mi) => {
          bump(stats, 'bulk_flags', `0x${m.flags.toString(16).padStart(4, '0')}:${m.storage}`);
          bump(stats, 'mip_cooked_flag', m.cooked);
          if (m.storage === 'inline') bump(stats, 'inline_offset_rule', String(m.inlineOffsetRuleOk));
          const nx = Math.max(1, pd.mips[0].sizeX >> mi), ny = Math.max(1, pd.mips[0].sizeY >> mi);
          const want = tx.mipBytes(fmt, nx, ny) * pd.numSlices;
          bump(stats, 'mip_size_matches_dds', want === null ? 'unknown format' : String(want === m.sizeOnDisk));
          bump(stats, 'mip_recorded_size', m.sizeX === nx && m.sizeY === ny ? 'nominal (top >> i)' : m.sizeX === Math.max(4, nx) && m.sizeY === Math.max(4, ny) ? 'clamped to 4' : 'other');
          if (mi && m.storage === 'ubulk' && pd.mips[mi - 1].storage !== 'ubulk') bump(stats, 'ubulk_mip_after_inline_mip', 'yes');
        });
        if (pd.mips.some((m) => m.storage === 'ubulk')) getUbulk();

        // ---- forward: DDS + sidecar
        let dds = tx.toDds(tex, uexp, ubulk);
        let sidecarText = JSON.stringify(tx.sidecarOf(tex, { package: p, export: name, class: cls, converter: 'tools/texture_convert.js', converter_version: VERSION }), null, 1) + '\n';
        row.sha256_dds = sha256(dds);
        row.dds_bytes = dds.length;
        row.sidecar_bytes = Buffer.byteLength(sidecarText);
        if (args.mode === 'export') {
          let relOut = base + (path.posix.basename(base) === name ? '' : '.' + name);
          if (usedOut.has(relOut.toLowerCase())) relOut += '.' + (idx + 1);
          usedOut.add(relOut.toLowerCase());
          const ddsPath = path.join(OUT_ROOT, relOut + '.dds'), jsonPath = path.join(OUT_ROOT, relOut + '.texture.json');
          fs.mkdirSync(path.dirname(ddsPath), { recursive: true });
          fs.writeFileSync(ddsPath, dds);
          fs.writeFileSync(jsonPath, sidecarText);
          // the proof below is made from the bytes that are actually on disk
          dds = fs.readFileSync(ddsPath);
          sidecarText = fs.readFileSync(jsonPath, 'utf8');
          row.converted_path = 'assets_converted/textures/' + relOut + '.dds';
          row.sidecar_path = 'assets_converted/textures/' + relOut + '.texture.json';
          row.sha256_dds_on_disk = sha256(dds);
          row.sha256_sidecar = sha256(Buffer.from(sidecarText));
        }
        // ---- reverse: from DDS bytes + sidecar text + package name table only
        const back = tx.rebuild(pkg, JSON.parse(sidecarText), dds);
        row.sha256_rebuilt_export = sha256(back.exportBytes);
        const srcBulk = pd.mips.filter((m) => m.storage === 'ubulk').map((m) => tx.mipData(m, uexp, ubulk));
        row.sha256_source_bulk = srcBulk.length ? sha256(...srcBulk) : null;
        row.sha256_rebuilt_bulk = back.bulk.length ? sha256(...back.bulk.map((b) => b.data)) : null;
        const bulkSame = back.bulk.length === srcBulk.length && back.bulk.every((b, j) => b.data.equals(srcBulk[j]) && b.offset === pd.mips.filter((m) => m.storage === 'ubulk')[j].dataOffset);
        row.roundtrip_matches = back.exportBytes.equals(src) && bulkSame;
        row.reversible = row.roundtrip_matches;
        if (!row.roundtrip_matches) row.error = 'rebuilt bytes differ from the original';
        else { rebuiltExports.set(idx, back.exportBytes); bulkPieces.push(...back.bulk); }
        const px = pd.mips.reduce((s, m) => s + m.sizeOnDisk, 0);
        bump(stats, 'pixel_bytes', 'total', px);
        bump(stats, 'pixel_bytes', 'in_ubulk', pd.mips.reduce((s, m) => s + (m.storage === 'ubulk' ? m.sizeOnDisk : 0), 0));
        bump(stats, 'dds_bytes', 'total', dds.length);
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

    // ---- whole-file proof. .uexp: possible when every export of the package was rebuilt.
    const pf = {};
    if (uexp) {
      pf.sha256_uexp = sha256(uexp);
      if (rebuiltExports.size === pkg.exports.length) {
        const order = pkg.exports.map((e, idx) => ({ e, idx })).sort((a, b) => a.e.serialOffset - b.e.serialOffset);
        let pos = hdr, contiguous = true;
        for (const { e } of order) { if (e.serialOffset !== pos) contiguous = false; pos = e.serialOffset + e.serialSize; }
        const tag = Buffer.alloc(4); tag.writeUInt32LE(ua.TAG);
        const again = Buffer.concat([...order.map(({ idx }) => rebuiltExports.get(idx)), tag]);
        pf.sha256_uexp_rebuilt = sha256(again);
        pf.uexp = contiguous && again.equals(uexp) ? 'identical' : 'differs';
      } else pf.uexp = rebuiltExports.size ? 'other_exports_present' : 'not_rebuilt';
    } else pf.uexp = 'absent';
    // .ubulk: possible when the rebuilt textures' pieces tile the whole file.
    if (files.has(base + '.ubulk')) {
      getUbulk();
      pf.sha256_ubulk = sha256(ubulk);
      const again = Buffer.alloc(ubulk.length);
      bulkPieces.sort((a, b) => a.offset - b.offset);
      let pos = 0, tiled = true;
      for (const b of bulkPieces) {
        if (b.offset !== pos || b.offset + b.data.length > ubulk.length) { tiled = false; break; }
        b.data.copy(again, b.offset);
        pos = b.offset + b.data.length;
      }
      if (tiled && pos === ubulk.length) { pf.sha256_ubulk_rebuilt = sha256(again); pf.ubulk = again.equals(ubulk) ? 'identical' : 'differs'; }
      else pf.ubulk = 'not_fully_covered_by_textures';
    } else pf.ubulk = 'absent';
    bump(stats, 'package_uexp', pf.uexp);
    bump(stats, 'package_ubulk', pf.ubulk);
    bump(stats, 'package_files', `uexp ${pf.uexp} / ubulk ${pf.ubulk}`);
    for (const row of rows) { row.package_files = pf; row.converter = 'tools/texture_convert.js'; row.converter_version = VERSION; fs.writeSync(out, JSON.stringify(row) + '\n'); }
  }
  fs.closeSync(out);
  for (const fd of fds.values()) fs.closeSync(fd);
  process.stdout.write('RESULT ' + JSON.stringify({ shard: k, textures: seen, stats }) + '\n');
}

// ------------------------------------------------------------------ parent

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
    const full = !args.prefix && errors.length === 0;
    const tag = args.mode === 'verify' ? 'texture_manifest' : 'texture_export';
    const suffix = full ? '' : '.partial';
    const rowsFile = path.join(ROOT, 'audit', `${tag}${suffix}.json`);
    const sumFile = path.join(ROOT, 'audit', `${args.mode === 'verify' ? 'texture_summary' : 'texture_export_summary'}${suffix}.json`);
    const fd = fs.openSync(rowsFile, 'w');
    fs.writeSync(fd, '[\n');
    keyed.forEach(([, l], i) => fs.writeSync(fd, l + (i + 1 < keyed.length ? ',\n' : '\n')));
    fs.writeSync(fd, ']\n');
    fs.closeSync(fd);
    const summary = {
      generated_by: 'tools/texture_convert.js', version: VERSION, mode: args.mode, prefix: args.prefix, classes: args.classes,
      textures: keyed.length, roundtrip_matches: (stats.roundtrip || {}).matches || 0, roundtrip_fails: (stats.roundtrip || {}).fails || 0,
      rows: path.relative(ROOT, rowsFile).replace(/\\/g, '/'), rows_sha256: sha256(fs.readFileSync(rowsFile)),
      seconds: Math.round((Date.now() - started) / 1000), worker_errors: errors, stats,
    };
    writeJson(sumFile, summary);
    logRun({ tool: 'texture_convert', version: VERSION, outputs: [rowsFile, sumFile], note: `${args.mode}: ${keyed.length} textures, ${summary.roundtrip_matches} round-trip, ${summary.roundtrip_fails} fail, ${errors.length} worker errors` });
    const { top_mip_size, property, ...show } = stats;
    console.log('\n' + JSON.stringify({ ...summary, stats: show }, null, 1));
  };
  for (let k = 0; k < n; k++) {
    running++;
    const argv = [__filename, args.mode, '--shard', `${k}/${n}`, '--out', path.join(TMP, `${args.mode}_${k}.jsonl`), '--classes', args.classes.join(',')];
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
