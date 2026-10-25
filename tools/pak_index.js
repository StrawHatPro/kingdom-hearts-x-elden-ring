// Decrypt and parse every pak index. Writes the full listing to build/pak_index/ (one JSON per
// pak, regenerable) and a committed summary to audit/triage/pak_index_summary.json.
const fs = require('fs');
const path = require('path');
const { ROOT, CONFIG, logRun, writeJson } = require('./lib/log');
const pak = require('./lib/pak');

const VERSION = '1.0.0';
const OUT = path.join(ROOT, 'audit', 'triage', 'pak_index_summary.json');
const LIST_DIR = path.join(ROOT, 'build', 'pak_index');
fs.mkdirSync(LIST_DIR, { recursive: true });

const bump = (o, k, n = 1) => (o[k] = (o[k] || 0) + n);
const totals = { paks: 0, entries: 0, stored_bytes: 0, uncompressed_bytes: 0, by_compression: {}, encrypted_entries: 0,
  by_extension: {}, bytes_by_extension: {}, block_sizes: {}, index_sha1_of: {}, mount_points: {}, parse_errors: [] };
const perPak = [];
const seen = new Map(); // mounted path -> paks providing it (patch paks override)

for (const p of pak.listPaks()) {
  const relPath = path.relative(CONFIG.gameRoot, p).replace(/\\/g, '/');
  let idx;
  try {
    idx = pak.readIndex(p);
  } catch (e) {
    totals.parse_errors.push({ pak: relPath, error: String(e.message) });
    continue;
  }
  totals.paks++;
  bump(totals.index_sha1_of, String(idx.indexSha1Of));
  bump(totals.mount_points, idx.mountPoint);
  const row = { pak: relPath, size: idx.size, entries: idx.entries.length, index_sha1_of: idx.indexSha1Of,
    index_bytes_used: idx.indexBytesUsed, index_size: idx.footer.indexSize, stored_bytes: 0, uncompressed_bytes: 0 };
  for (const e of idx.entries) {
    totals.entries++;
    row.stored_bytes += e.size;
    row.uncompressed_bytes += e.uncompressedSize;
    bump(totals.by_compression, e.compression);
    if (e.encrypted) totals.encrypted_entries++;
    if (e.compression) bump(totals.block_sizes, e.blockSize);
    const ext = path.extname(e.name).toLowerCase() || '(none)';
    bump(totals.by_extension, ext);
    bump(totals.bytes_by_extension, ext, e.uncompressedSize);
    const full = idx.mountPoint + e.name;
    if (!seen.has(full)) seen.set(full, []);
    seen.get(full).push(relPath);
  }
  totals.stored_bytes += row.stored_bytes;
  totals.uncompressed_bytes += row.uncompressed_bytes;
  perPak.push(row);
  fs.writeFileSync(path.join(LIST_DIR, relPath.replace(/[\\/]/g, '__') + '.json'),
    JSON.stringify({ pak: relPath, mountPoint: idx.mountPoint, footer: idx.footer, entries: idx.entries }));
}
totals.unique_mounted_paths = seen.size;
totals.paths_in_more_than_one_pak = [...seen.values()].filter((v) => v.length > 1).length;
const sortObj = (o) => Object.fromEntries(Object.entries(o).sort((a, b) => b[1] - a[1]));
totals.by_extension = sortObj(totals.by_extension);
totals.bytes_by_extension = sortObj(totals.bytes_by_extension);

writeJson(OUT, { totals, paks: perPak });
logRun({ tool: 'pak_index', version: VERSION, outputs: [OUT] });
console.log(JSON.stringify(totals, null, 2));
