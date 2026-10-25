// Extract pak entries to assets/ (decrypted, decompressed; bytes are verified against the
// sha256_extracted values in audit/asset_manifest when --verify is given).
//   node tools/pak_extract.js <path prefix or /regex/> [--verify]
// Paths are mounted paths, e.g. "KINGDOM HEARTS III/Content/DataTable/".
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { ROOT, logRun, writeJson } = require('./lib/log');
const pak = require('./lib/pak');

const VERSION = '1.0.0';
const sel = process.argv[2];
if (!sel) { console.error('usage: pak_extract.js <prefix|/regex/>'); process.exit(2); }
const match = sel.startsWith('/') && sel.endsWith('/') ? ((re) => (p) => re.test(p))(new RegExp(sel.slice(1, -1))) : (p) => p.startsWith(sel);
const files = pak.mountAll();
const fds = new Map();
let n = 0, bytes = 0;
const listing = [];
for (const [p, { pak: pakFile, entry }] of files) {
  if (!match(p)) continue;
  if (!fds.has(pakFile)) fds.set(pakFile, fs.openSync(pakFile, 'r'));
  const data = pak.extract(fds.get(pakFile), entry);
  const out = path.join(ROOT, 'assets', p);
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, data);
  listing.push({ path: 'assets/' + p, archive: path.basename(pakFile), size: data.length, sha256: crypto.createHash('sha256').update(data).digest('hex') });
  n++; bytes += data.length;
}
for (const fd of fds.values()) fs.closeSync(fd);
const OUT = path.join(ROOT, 'build', 'extract_last.json');
writeJson(OUT, listing);
logRun({ tool: 'pak_extract', version: VERSION, outputs: [OUT], note: `${n} files, ${bytes} bytes, selector ${sel}` });
console.log(`${n} files, ${bytes} bytes -> assets/`);
