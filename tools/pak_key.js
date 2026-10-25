// Locate the pak index AES-256 key inside the executable by known plaintext: a decrypted
// index begins with the mount-point FString (int32 length, then "../../../").
// Records only where the key lives; tools read it from the user's own exe at run time.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { ROOT, CONFIG, logRun, writeJson, hex } = require('./lib/log');

const VERSION = '1.0.0';
const OUT = path.join(ROOT, 'audit', 'triage', 'pak_key.json');
const exe = fs.readFileSync(path.join(CONFIG.gameRoot, CONFIG.mainExe));
const footers = JSON.parse(fs.readFileSync(path.join(ROOT, 'audit', 'triage', 'pak_footers.json'), 'utf8'));

function indexHead(row, n = 64) {
  const fd = fs.openSync(path.join(CONFIG.gameRoot, row.path), 'r');
  const buf = Buffer.alloc(n);
  fs.readSync(fd, buf, 0, n, row.index_offset);
  fs.closeSync(fd);
  return buf;
}
function decrypt(key, data) {
  const d = crypto.createDecipheriv('aes-256-ecb', key, null);
  d.setAutoPadding(false);
  return Buffer.concat([d.update(data), d.final()]);
}
const looksLikeIndex = (p) => p.readInt32LE(0) > 0 && p.readInt32LE(0) < 512 && p.toString('latin1', 4, 13) === '../../../';

const sample = indexHead(footers[0]).subarray(0, 16);
const printable = (c) => c >= 0x20 && c < 0x7f;
const found = [];
// Pass 1: 32 printable ASCII bytes (UE 4.17 defines the key as a string). Pass 2: any bytes.
for (const asciiOnly of [true, false]) {
  let run = 0;
  for (let o = 0; o + 32 <= exe.length; o++) {
    if (asciiOnly) {
      // run = count of consecutive printable bytes ending at o+31
      run = printable(exe[o + 31]) ? run + 1 : 0;
      if (o === 0) { run = 0; for (let k = 0; k < 32 && printable(exe[k]); k++) run++; }
      if (run < 32) continue;
    }
    const p = decrypt(exe.subarray(o, o + 32), sample);
    if (looksLikeIndex(p)) found.push({ file_offset: hex(o), ascii: asciiOnly });
  }
  if (found.length) break;
  console.log(asciiOnly ? 'no printable-ASCII key; trying every offset' : 'no key found');
}

const result = { cipher: 'AES-256-ECB', candidates: found, verified_paks: 0, total_paks: footers.length, failures: [] };
if (found.length) {
  const off = parseInt(found[0].file_offset);
  const key = exe.subarray(off, off + 32);
  result.key_file_offset = found[0].file_offset;
  result.key_sha256 = crypto.createHash('sha256').update(key).digest('hex');
  // (PACKGEN) A pak whose whole index decrypts to the SHA-1 its footer records opens with this key too: the empty patch
  // paks (`*_0_P.pak`, 61 bytes: a 16-byte index with the mount point "/" and no entry) have no "../../../" to look for.
  const indexSha1Ok = (row) => {
    if (!(row.index_size > 0) || row.index_size % 16) return false;
    const fd = fs.openSync(path.join(CONFIG.gameRoot, row.path), 'r');
    const buf = Buffer.alloc(row.index_size);
    try { fs.readSync(fd, buf, 0, row.index_size, row.index_offset); } finally { fs.closeSync(fd); }
    return crypto.createHash('sha1').update(decrypt(key, buf)).digest('hex') === row.index_sha1;
  };
  result.verified_by_index_sha1 = [];
  for (const row of footers) {
    const p = decrypt(key, indexHead(row));
    if (looksLikeIndex(p)) result.verified_paks++;
    else if (indexSha1Ok(row)) { result.verified_paks++; result.verified_by_index_sha1.push(row.path); }
    else result.failures.push(row.path);
  }
}
writeJson(OUT, result);
logRun({ tool: 'pak_key', version: VERSION, outputs: [OUT] });
console.log(JSON.stringify(result, null, 2));
