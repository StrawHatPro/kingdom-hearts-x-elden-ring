// Probe the pak data area: does the in-data FPakEntry header equal the index entry, and which
// deflate settings (if any) reproduce the stored compressed blocks byte-for-byte?
const fs = require('fs');
const path = require('path');
const zlib = require('zlib');
const { ROOT, CONFIG, logRun, writeJson } = require('./lib/log');
const pak = require('./lib/pak');

const VERSION = '1.0.0';
const OUT = path.join(ROOT, 'audit', 'triage', 'pak_probe.json');
const file = pak.listPaks()[0];
const idx = pak.readIndex(file);
const fd = fs.openSync(file, 'r');
const res = { pak: path.relative(CONFIG.gameRoot, file).replace(/\\/g, '/'), sampled: 0, header_equals_index: 0, header_differs: 0,
  zlib_headers: {}, block_offsets_absolute: 0, block_offsets_relative: 0, inflate_ok: 0, inflate_fail: 0,
  uncompressed_size_ok: 0, redeflate_match_by_level: {}, blocks: 0, first_mismatch_examples: [] };

const sample = idx.entries.filter((e) => e.compression === 1 && !e.encrypted).filter((_, i) => i % 200 === 0).slice(0, 300);
for (const e of sample) {
  res.sampled++;
  const hdr = pak.readAt(fd, e.offset, e.headerSize);
  const parsed = pak.readEntry(hdr, 0, idx.footer.version).entry;
  // In-data headers store offset 0; everything else should agree with the index.
  const same = parsed.size === e.size && parsed.uncompressedSize === e.uncompressedSize && parsed.sha1 === e.sha1 && parsed.blocks.length === e.blocks.length;
  same ? res.header_equals_index++ : res.header_differs++;
  const dataStart = e.offset + e.headerSize;
  const absolute = e.blocks[0][0] === dataStart;
  absolute ? res.block_offsets_absolute++ : res.block_offsets_relative++;
  let total = 0;
  for (const [s, t] of e.blocks) {
    const start = absolute ? s : e.offset + s, end = absolute ? t : e.offset + t;
    const comp = pak.readAt(fd, start, end - start);
    const h = comp.subarray(0, 2).toString('hex');
    res.zlib_headers[h] = (res.zlib_headers[h] || 0) + 1;
    res.blocks++;
    let raw;
    try { raw = zlib.inflateSync(comp); res.inflate_ok++; } catch { res.inflate_fail++; continue; }
    total += raw.length;
    let matched = false;
    for (const level of [-1, 1, 2, 3, 4, 5, 6, 7, 8, 9]) {
      const again = zlib.deflateSync(raw, { level });
      if (again.equals(comp)) { res.redeflate_match_by_level[level] = (res.redeflate_match_by_level[level] || 0) + 1; matched = true; }
    }
    if (!matched && res.first_mismatch_examples.length < 5) {
      const again = zlib.deflateSync(raw, { level: 6 });
      res.first_mismatch_examples.push({ entry: e.name, stored: comp.length, node_level6: again.length, raw: raw.length });
    }
  }
  if (total === e.uncompressedSize) res.uncompressed_size_ok++;
}
fs.closeSync(fd);
res.node_zlib = process.versions.zlib;
writeJson(OUT, res);
logRun({ tool: 'pak_probe', version: VERSION, outputs: [OUT] });
console.log(JSON.stringify(res, null, 1));
