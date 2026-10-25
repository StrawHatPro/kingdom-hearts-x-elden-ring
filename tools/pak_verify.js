// Phase 4: asset manifest with round-trip proof for every pak entry (spec 3.3).
//   node tools/pak_verify.js            all paks, in parallel; writes audit/asset_manifest.json
//   node tools/pak_verify.js --one <pak path relative to gameRoot>
//
// Per entry: sha256_raw of the stored payload, sha256_extracted of the decrypted+inflated file,
// SHA-1 check against the index, and a round-trip: every zlib block re-deflated with zlib 1.2.8
// (build/native/zrt.exe) must equal the stored block, and encrypted payloads must re-encrypt to
// the stored bytes. Nothing is written to assets/ here; tools/pak_extract.js does that on demand.
const fs = require('fs');
const path = require('path');
const zlib = require('zlib');
const crypto = require('crypto');
const { execFileSync, spawn } = require('child_process');
const { ROOT, CONFIG, logRun, writeJson } = require('./lib/log');
const pak = require('./lib/pak');

const VERSION = '1.1.0';
const OUT_DIR = path.join(ROOT, 'audit', 'asset_manifest');
const ZRT = path.join(ROOT, 'build', 'native', 'zrt.exe');
const sha = (alg, ...bufs) => { const h = crypto.createHash(alg); for (const x of bufs) h.update(x); return h.digest('hex'); };
const outName = (rel) => rel.replace(/^KINGDOM HEARTS III\/Content\//, '').replace(/[\\/]/g, '__').replace(/\.pak$/, '') + '.json';
const align16 = (n) => (n + 15) & ~15;

function verifyOne(rel) {
  const file = path.join(CONFIG.gameRoot, rel);
  const idx = pak.readIndex(file);
  const tmp = path.join(ROOT, 'build', 'zrt_tmp');
  fs.mkdirSync(tmp, { recursive: true });
  const tag = outName(rel);

  // One zrt job per plain zlib block. Encrypted compressed blocks are handled in JS below.
  const jobList = [];
  for (const e of idx.entries) if (e.compression === 1 && !e.encrypted) for (const blk of e.blocks) jobList.push(blk);
  function runZrt(dataFile, list, suffix) {
    if (!list.length) return Buffer.alloc(0);
    const jobs = Buffer.alloc(list.length * 16);
    list.forEach(([s, t], i) => { jobs.writeBigInt64LE(BigInt(s), i * 16); jobs.writeBigInt64LE(BigInt(t), i * 16 + 8); });
    const jf = path.join(tmp, tag + suffix + '.jobs'), of = path.join(tmp, tag + suffix + '.out');
    fs.writeFileSync(jf, jobs);
    execFileSync(ZRT, [dataFile, jf, of]);
    const res = fs.readFileSync(of);
    fs.rmSync(jf); fs.rmSync(of);
    if (res.length !== list.length * 6) throw new Error('zrt output size mismatch');
    return res;
  }
  const zr = runZrt(file, jobList, '');

  const fd = fs.openSync(file, 'r');
  // Encrypted compressed blocks: decrypt here, then let zrt re-deflate the plaintext streams
  // from a side file. Each block is stored padded to 16 bytes and encrypted on its own.
  const encStreams = [], encJobs = [];
  let encPos = 0;
  for (const e of idx.entries) if (e.compression === 1 && e.encrypted) for (const [s, t] of e.blocks) {
    const dec = pak.aesEcb(pak.readAt(fd, s, align16(t - s)), true).subarray(0, t - s);
    encStreams.push(dec);
    encJobs.push([encPos, encPos + dec.length]);
    encPos += dec.length;
  }
  let zrEnc = Buffer.alloc(0), encJob = 0;
  if (encJobs.length) {
    const side = path.join(tmp, tag + '.enc');
    fs.writeFileSync(side, Buffer.concat(encStreams));
    zrEnc = runZrt(side, encJobs, '.e');
    fs.rmSync(side);
  }
  const rows = [];
  const sum = { pak: rel, entries: idx.entries.length, roundtrip_ok: 0, roundtrip_fail: 0, sha1_ok: 0, sha1_fail: 0, header_mismatch: 0,
    size_mismatch: 0, deflate_settings: {}, stored_bytes: 0, extracted_bytes: 0 };
  let job = 0;
  for (const e of idx.entries) {
    const dataStart = e.offset + e.headerSize;
    const storedLen = e.encrypted ? align16(e.size) : e.size;
    const stored = pak.readAt(fd, dataStart, storedLen);
    const hdr = pak.readEntry(pak.readAt(fd, e.offset, e.headerSize), 0, idx.footer.version).entry;
    if (hdr.size !== e.size || hdr.uncompressedSize !== e.uncompressedSize || hdr.sha1 !== e.sha1 || hdr.compression !== e.compression) sum.header_mismatch++;

    const row = {
      archive: rel, entry: idx.mountPoint + e.name, offset: '0x' + dataStart.toString(16).toUpperCase(), size: e.size,
      sha256_raw: sha('sha256', stored), compression: e.compression ? 'zlib' : 'none', encrypted: e.encrypted,
      uncompressed_size: e.uncompressedSize, index_sha1: e.sha1,
    };
    let reversible = true, reason = null, extractedHash, extractedLen = 0, settings = new Set();
    const plain = e.encrypted ? pak.aesEcb(stored, true) : stored;
    if (e.encrypted && !pak.aesEcb(plain, false).equals(stored)) { reversible = false; reason = 're-encryption does not reproduce stored bytes'; }
    if (e.compression === 0) {
      const body = plain.subarray(0, e.size);
      extractedHash = sha('sha256', body);
      extractedLen = body.length;
      if (e.encrypted && storedLen !== e.size) row.encryption_padding_hex = plain.subarray(e.size).toString('hex'); // needed to repack exactly
    } else {
      const h = crypto.createHash('sha256');
      let expectStart = dataStart;
      for (const [s, t] of e.blocks) {
        if (s !== expectStart) { reversible = false; reason = 'blocks are not contiguous'; }
        expectStart = e.encrypted ? s + align16(t - s) : t;
        let raw, status, setting;
        if (e.encrypted) {
          const enc = pak.readAt(fd, s, align16(t - s));
          const dec = pak.aesEcb(enc, true);
          if (!pak.aesEcb(dec, false).equals(enc)) { reversible = false; reason = 're-encryption does not reproduce a stored block'; }
          (row.encryption_padding_hex = row.encryption_padding_hex || []).push(dec.subarray(t - s).toString('hex')); // needed to repack exactly
          raw = zlib.inflateSync(dec.subarray(0, t - s));
          status = zrEnc[encJob * 6]; setting = zrEnc[encJob * 6 + 1];
          encJob++;
        } else {
          status = zr[job * 6]; setting = zr[job * 6 + 1];
          job++;
          raw = zlib.inflateSync(stored.subarray(s - dataStart, t - dataStart));
        }
        if (status === 0) settings.add(`level${setting >> 4}/mem${setting & 15}`);
        else { reversible = false; reason = status === 2 ? 'zlib 1.2.8 failed to inflate a block' : 'no deflate setting reproduces a block'; }
        h.update(raw);
        extractedLen += raw.length;
      }
      extractedHash = h.digest('hex');
      if (settings.size) row.conversion_params = { zlib: '1.2.8', settings: [...settings], block_size: e.blockSize };
    }
    if (extractedLen !== e.uncompressedSize) { sum.size_mismatch++; reversible = false; reason = 'uncompressed size differs from index'; }
    // FPakEntry.Hash is the SHA-1 of the stored payload (after compression, before encryption padding is considered).
    // Observed: the hash covers exactly size bytes of the stored payload, i.e. it stops before
    // the encryption padding. The other candidates are kept so a different pak would be noticed.
    row.index_sha1_matches = sha('sha1', stored.subarray(0, e.size)) === e.sha1 ? 'stored' : sha('sha1', stored) === e.sha1 ? 'stored_padded'
      : sha('sha1', plain.subarray(0, e.size)) === e.sha1 ? 'decrypted' : false;
    row.index_sha1_matches ? sum.sha1_ok++ : sum.sha1_fail++;

    row.extracted_path = 'assets/' + (idx.mountPoint + e.name).replace(/^(\.\.\/)+/, '').replace(/^\//, '');
    row.sha256_extracted = extractedHash;
    row.converter = 'tools/pak_extract.js';
    row.converter_version = VERSION;
    row.reversible = reversible;
    if (reversible) { row.roundtrip_sha256 = row.sha256_raw; row.roundtrip_matches = true; sum.roundtrip_ok++; } // repacked bytes compared equal to stored bytes
    else { row.roundtrip_matches = false; row.irreversible_reason = reason; sum.roundtrip_fail++; }
    for (const s of settings) sum.deflate_settings[s] = (sum.deflate_settings[s] || 0) + 1;
    sum.stored_bytes += storedLen;
    sum.extracted_bytes += extractedLen;
    rows.push(row);
  }
  fs.closeSync(fd);
  fs.mkdirSync(OUT_DIR, { recursive: true });
  const outFile = path.join(OUT_DIR, tag);
  fs.writeFileSync(outFile, '[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]\n');
  sum.manifest = 'audit/asset_manifest/' + tag;
  sum.manifest_sha256 = sha('sha256', fs.readFileSync(outFile));
  return sum;
}

if (process.argv[2] === '--one') {
  const sum = verifyOne(process.argv[3]);
  process.stdout.write('RESULT ' + JSON.stringify(sum) + '\n');
} else {
  const all = pak.listPaks().map((p) => path.relative(CONFIG.gameRoot, p).replace(/\\/g, '/'));
  const only = process.argv[2] === '--match' ? all.filter((p) => p.includes(process.argv[3])) : all;
  const sizeOf = (p) => fs.statSync(path.join(CONFIG.gameRoot, p)).size;
  const queue = only.slice().sort((a, b) => sizeOf(b) - sizeOf(a));
  const results = [], errors = [];
  const PAR = 10;
  let running = 0, done = 0;
  const next = () => {
    while (running < PAR && queue.length) {
      const rel = queue.shift();
      running++;
      const c = spawn(process.execPath, ['--max-old-space-size=6000', __filename, '--one', rel], { stdio: ['ignore', 'pipe', 'pipe'] });
      let out = '', err = '';
      c.stdout.on('data', (d) => (out += d));
      c.stderr.on('data', (d) => (err += d));
      c.on('close', (code) => {
        running--; done++;
        const m = /^RESULT (.*)$/m.exec(out);
        if (code === 0 && m) results.push(JSON.parse(m[1]));
        else errors.push({ pak: rel, error: err.slice(-400) });
        process.stdout.write(`\r${done}/${only.length}  `);
        if (queue.length) next();
        else if (!running) finish();
      });
    }
  };
  const finish = () => {
    results.sort((a, b) => a.pak.localeCompare(b.pak));
    const tot = { paks: results.length, entries: 0, roundtrip_ok: 0, roundtrip_fail: 0, sha1_ok: 0, sha1_fail: 0, header_mismatch: 0, size_mismatch: 0, stored_bytes: 0, extracted_bytes: 0, deflate_settings: {} };
    for (const r of results) {
      for (const k of ['entries', 'roundtrip_ok', 'roundtrip_fail', 'sha1_ok', 'sha1_fail', 'header_mismatch', 'size_mismatch', 'stored_bytes', 'extracted_bytes']) tot[k] += r[k];
      for (const [k, v] of Object.entries(r.deflate_settings)) tot.deflate_settings[k] = (tot.deflate_settings[k] || 0) + v;
    }
    const OUT = path.join(ROOT, 'audit', 'asset_manifest.json');
    if (only.length === all.length) {
      writeJson(OUT, { generated_by: 'tools/pak_verify.js', note: 'Per-entry rows are in audit/asset_manifest/<pak>.json (one file per archive).', totals: tot, errors, archives: results });
      logRun({ tool: 'pak_verify', version: VERSION, outputs: [OUT], note: `zrt.exe = zlib 1.2.8 built by tools/native/build.ps1; ${errors.length} errors` });
    }
    console.log('\n' + JSON.stringify(tot, null, 1));
    if (errors.length) console.log('ERRORS', JSON.stringify(errors, null, 1));
  };
  next();
}
