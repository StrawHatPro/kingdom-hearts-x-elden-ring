// UE4 .pak reader (footer version 4, AES-256-ECB encrypted index). Format: docs/formats/pak.md
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { ROOT, CONFIG } = require('./log');

const MAGIC = 0x5a6f12e1;
const FOOTER_SIZE = 45; // bEncryptedIndex + magic + version + index offset + index size + SHA-1

let cachedKey = null;
// The key is never stored in the repo: it is read from the user's exe at the audited offset.
function aesKey() {
  if (cachedKey) return cachedKey;
  const info = JSON.parse(fs.readFileSync(path.join(ROOT, 'audit', 'triage', 'pak_key.json'), 'utf8'));
  const off = parseInt(info.key_file_offset);
  const fd = fs.openSync(path.join(CONFIG.gameRoot, CONFIG.mainExe), 'r');
  const key = Buffer.alloc(32);
  fs.readSync(fd, key, 0, 32, off);
  fs.closeSync(fd);
  if (crypto.createHash('sha256').update(key).digest('hex') !== info.key_sha256) throw new Error('pak key mismatch: different exe?');
  return (cachedKey = key);
}

function aesEcb(data, decrypt) {
  const c = (decrypt ? crypto.createDecipheriv : crypto.createCipheriv)('aes-256-ecb', aesKey(), null);
  c.setAutoPadding(false);
  return Buffer.concat([c.update(data), c.final()]);
}

function readAt(fd, offset, length) {
  const buf = Buffer.alloc(length);
  let got = 0;
  while (got < length) {
    const n = fs.readSync(fd, buf, got, length - got, offset + got);
    if (!n) throw new Error(`short read at ${offset + got}`);
    got += n;
  }
  return buf;
}

function readFooter(fd, size) {
  const f = readAt(fd, size - FOOTER_SIZE, FOOTER_SIZE);
  if (f.readUInt32LE(1) !== MAGIC) throw new Error('bad pak magic');
  return {
    encryptedIndex: f[0] !== 0,
    version: f.readUInt32LE(5),
    indexOffset: Number(f.readBigUInt64LE(9)),
    indexSize: Number(f.readBigUInt64LE(17)),
    indexSha1: f.subarray(25, 45).toString('hex'),
  };
}

// Serialized FPakEntry, as it appears both in the index and in front of each payload.
function readEntry(b, o, version) {
  const e = {
    offset: Number(b.readBigInt64LE(o)),
    size: Number(b.readBigInt64LE(o + 8)),
    uncompressedSize: Number(b.readBigInt64LE(o + 16)),
    compression: b.readInt32LE(o + 24),
  };
  o += 28;
  if (version <= 1) o += 8; // timestamp
  e.sha1 = b.subarray(o, o + 20).toString('hex');
  o += 20;
  e.blocks = [];
  if (version >= 3) {
    if (e.compression !== 0) {
      const n = b.readInt32LE(o);
      o += 4;
      for (let i = 0; i < n; i++, o += 16) e.blocks.push([Number(b.readBigInt64LE(o)), Number(b.readBigInt64LE(o + 8))]);
    }
    e.encrypted = b[o] !== 0;
    e.blockSize = b.readUInt32LE(o + 1);
    o += 5;
  }
  return { entry: e, next: o };
}

function readFString(b, o) {
  const len = b.readInt32LE(o);
  o += 4;
  if (len === 0) return { s: '', next: o };
  if (len > 0) return { s: b.toString('latin1', o, o + len - 1), next: o + len };
  return { s: b.toString('utf16le', o, o - len * 2 - 2), next: o - len * 2 };
}

// Returns { footer, mountPoint, entries: [{name, headerSize, ...FPakEntry}], indexSha1Ok }
function readIndex(file) {
  const size = fs.statSync(file).size;
  const fd = fs.openSync(file, 'r');
  try {
    const footer = readFooter(fd, size);
    const raw = readAt(fd, footer.indexOffset, footer.indexSize);
    const idx = footer.encryptedIndex ? aesEcb(raw, true) : raw;
    const sha1 = (x) => crypto.createHash('sha1').update(x).digest('hex');
    const indexSha1Of = sha1(idx) === footer.indexSha1 ? 'plaintext' : sha1(raw) === footer.indexSha1 ? 'ciphertext' : null;
    let { s: mountPoint, next: o } = readFString(idx, 0);
    const count = idx.readInt32LE(o);
    o += 4;
    const entries = [];
    for (let i = 0; i < count; i++) {
      const n = readFString(idx, o);
      const start = n.next;
      const r = readEntry(idx, start, footer.version);
      r.entry.name = n.s;
      r.entry.headerSize = r.next - start; // the same struct precedes the payload in the data area
      entries.push(r.entry);
      o = r.next;
    }
    return { size, footer, mountPoint, entries, indexSha1Of, indexBytesUsed: o, indexPadding: idx.subarray(o).toString('hex') };
  } finally {
    fs.closeSync(fd);
  }
}

function listPaks() {
  const out = [];
  (function walk(dir) {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const p = path.join(dir, e.name);
      if (e.isDirectory()) walk(p);
      else if (e.name.toLowerCase().endsWith('.pak')) out.push(p);
    }
  })(CONFIG.gameRoot);
  return out.sort();
}

// Decrypted, decompressed contents of one index entry.
function extract(fd, e) {
  const zlib = require('zlib');
  const align16 = (n) => (n + 15) & ~15;
  const dataStart = e.offset + e.headerSize;
  if (e.compression === 0) {
    const stored = readAt(fd, dataStart, e.encrypted ? align16(e.size) : e.size);
    return (e.encrypted ? aesEcb(stored, true) : stored).subarray(0, e.size);
  }
  const parts = [];
  for (const [s, t] of e.blocks) {
    const raw = e.encrypted ? aesEcb(readAt(fd, s, align16(t - s)), true).subarray(0, t - s) : readAt(fd, s, t - s);
    parts.push(zlib.inflateSync(raw));
  }
  return Buffer.concat(parts);
}

// Voice overlay paks: Content/_DLC_/VOICE/<en|ja>/Paks/*.pak. They are not part of the normal
// mount order. The game registers at most one of the two directories, chosen by language, in
// func_141555B10 (the only code that references the string "_DLC_"); each entry of such a pak
// is byte-identical to the regular file at the other language's path (audit/pak_overlays.json,
// docs/formats/pak_mount_finding.md). mountAll() therefore leaves them out.
const isOverlayPak = (p) => /[\\/]_DLC_[\\/]VOICE[\\/][^\\/]+[\\/]Paks[\\/][^\\/]+\.pak$/i.test(p);
const listOverlayPaks = () => listPaks().filter(isOverlayPak);
// The language directory of an overlay pak and the language whose paths its entries use.
function overlayLanguages(p) {
  const lang = /[\\/]VOICE[\\/]([^\\/]+)[\\/]Paks[\\/]/i.exec(p)[1].toLowerCase();
  return { lang, pathLang: lang === 'en' ? 'ja' : 'en' };
}

// Load order: base paks first, then patch paks (`_P`), so later entries override earlier ones.
// Overlay paks are excluded (see above).
// Returns Map: mounted path (no leading ../) -> { pak (absolute path), entry }.
function mountAll() {
  const paks = listPaks().filter((p) => !isOverlayPak(p)).sort((a, b) => {
    const pa = /_P\.pak$/i.test(a) ? 1 : 0, pb = /_P\.pak$/i.test(b) ? 1 : 0;
    return pa - pb || a.localeCompare(b);
  });
  const files = new Map();
  for (const p of paks) {
    const idx = readIndex(p);
    for (const e of idx.entries) files.set((idx.mountPoint + e.name).replace(/^(\.\.\/)+/, '').replace(/^\//, ''), { pak: p, entry: e });
  }
  return files;
}

module.exports = { MAGIC, FOOTER_SIZE, aesKey, aesEcb, readAt, readFooter, readEntry, readFString, readIndex, listPaks, extract, mountAll, isOverlayPak, listOverlayPaks, overlayLanguages };