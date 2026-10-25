// Phase 1 archive triage: locate and decode the footer of every .pak, and test whether the
// index is readable as plaintext (first field is the mount-point FString).
const fs = require('fs');
const path = require('path');
const { ROOT, CONFIG, logRun, writeJson, hex } = require('./lib/log');

const VERSION = '1.0.0';
const OUT = path.join(ROOT, 'audit', 'triage', 'pak_footers.json');
const MAGIC = 0x5a6f12e1;

function walk(dir, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walk(p, out);
    else if (e.name.toLowerCase().endsWith('.pak')) out.push(p);
  }
  return out;
}

function entropy(buf) {
  if (!buf.length) return 0;
  const c = new Float64Array(256);
  for (const x of buf) c[x]++;
  let e = 0;
  for (const n of c) if (n) e -= (n / buf.length) * Math.log2(n / buf.length);
  return Math.round(e * 1000) / 1000;
}

const rows = [];
for (const p of walk(CONFIG.gameRoot).sort()) {
  const size = fs.statSync(p).size;
  const fd = fs.openSync(p, 'r');
  const tailLen = Math.min(size, 512);
  const tail = Buffer.alloc(tailLen);
  fs.readSync(fd, tail, 0, tailLen, size - tailLen);
  const row = { path: path.relative(CONFIG.gameRoot, p).replace(/\\/g, '/'), size };
  let m = -1;
  for (let o = tailLen - 4; o >= 0; o--) if (tail.readUInt32LE(o) === MAGIC) { m = o; break; }
  if (m < 0) {
    row.magic_found = false;
    row.tail_hex = tail.subarray(Math.max(0, tailLen - 64)).toString('hex');
  } else {
    row.magic_found = true;
    row.footer_size = tailLen - m; // from magic to EOF
    row.version = tail.readUInt32LE(m + 4);
    row.index_offset = Number(tail.readBigUInt64LE(m + 8));
    row.index_size = Number(tail.readBigUInt64LE(m + 16));
    row.index_sha1 = tail.subarray(m + 24, m + 44).toString('hex');
    row.bytes_before_magic = m > 0 ? hex(tail[m - 1], 2) : null; // bEncryptedIndex in v4+ footers
    row.trailing_after_hash = tail.subarray(m + 44).toString('hex');
    row.index_end_matches_footer = row.index_offset + row.index_size <= size;
    const n = Math.min(row.index_size, 4096);
    const head = Buffer.alloc(n);
    if (n > 0 && row.index_offset + n <= size) fs.readSync(fd, head, 0, n, row.index_offset);
    row.index_head_entropy = entropy(head);
    row.index_head_hex = head.subarray(0, 48).toString('hex');
    // Plaintext index starts with FString mount point: int32 length, then "../../../" text.
    if (n >= 8) {
      const len = head.readInt32LE(0);
      row.index_plaintext = len > 0 && len < 512 && len + 4 <= n && head.toString('latin1', 4, 4 + Math.min(len, 9)).startsWith('../');
      if (row.index_plaintext) {
        row.mount_point = head.toString('latin1', 4, 4 + len - 1);
        row.entry_count = head.readInt32LE(4 + len);
      }
    }
  }
  fs.closeSync(fd);
  rows.push(row);
}
writeJson(OUT, rows);
logRun({ tool: 'pak_footers', version: VERSION, outputs: [OUT] });

const by = (f) => rows.reduce((a, r) => ((a[f(r)] = (a[f(r)] || 0) + 1), a), {});
console.log('paks:', rows.length);
console.log('magic found:', JSON.stringify(by((r) => r.magic_found)));
console.log('version:', JSON.stringify(by((r) => r.version)));
console.log('footer size:', JSON.stringify(by((r) => r.footer_size)));
console.log('byte before magic:', JSON.stringify(by((r) => r.bytes_before_magic)));
console.log('index plaintext:', JSON.stringify(by((r) => r.index_plaintext)));
console.log('entries (plaintext):', rows.filter((r) => r.index_plaintext).reduce((a, r) => a + r.entry_count, 0));
for (const r of rows.slice(0, 3).concat(rows.slice(-2))) console.log(JSON.stringify(r));
