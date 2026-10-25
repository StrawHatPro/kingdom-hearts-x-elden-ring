// CRI Sofdec2 `.usm` container: chunk reader/writer, @UTF table codec, demux/remux, and the
// bitstream probes used to decide whether payloads are masked. Format notes: docs/formats/usm.md.
// All integers are big-endian.
const fs = require('fs');
const crypto = require('crypto');

const VERSION = '1.0.0';
const TYPE_NAMES = ['stream', 'header', 'section_end', 'metadata'];

// ---------------------------------------------------------------------------------------------
// Chunk header (0x20 bytes in every chunk of these files; `dataOffset` generalises it)
// ---------------------------------------------------------------------------------------------
//  0  char[4] signature
//  4  u32     chunkSize        bytes following this field
//  8  u8      r08
//  9  u8      dataOffset       payload starts at 8 + dataOffset
// 10  u16     paddingSize      bytes after the payload, inside chunkSize
// 12  u8      chno
// 13  u8      r0D
// 14  u8      r0E
// 15  u8      dataType         0 stream, 1 header, 2 section end, 3 metadata
// 16  u32     frameTime
// 20  u32     frameRate
// 24  u32     r18
// 28  u32     r1C
function parseChunkHeader(b, o) {
  return {
    sig: b.toString('latin1', o, o + 4),
    chunkSize: b.readUInt32BE(o + 4),
    r08: b[o + 8],
    dataOffset: b[o + 9],
    paddingSize: b.readUInt16BE(o + 10),
    chno: b[o + 12],
    r0D: b[o + 13],
    r0E: b[o + 14],
    dataType: b[o + 15],
    frameTime: b.readUInt32BE(o + 16),
    frameRate: b.readUInt32BE(o + 20),
    r18: b.readUInt32BE(o + 24),
    r1C: b.readUInt32BE(o + 28),
  };
}

// Rebuilds the 8 + dataOffset header bytes. chunkSize is derived, not copied.
function buildChunkHeader(h, payloadLength, extra) {
  const b = Buffer.alloc(8 + h.dataOffset);
  b.write(h.sig, 0, 4, 'latin1');
  b.writeUInt32BE(h.dataOffset + payloadLength + h.paddingSize, 4);
  b[8] = h.r08; b[9] = h.dataOffset;
  b.writeUInt16BE(h.paddingSize, 10);
  b[12] = h.chno; b[13] = h.r0D; b[14] = h.r0E; b[15] = h.dataType;
  b.writeUInt32BE(h.frameTime, 16);
  b.writeUInt32BE(h.frameRate, 20);
  b.writeUInt32BE(h.r18, 24);
  b.writeUInt32BE(h.r1C, 28);
  if (extra) extra.copy(b, 0x20);
  return b;
}

// Sequential reader. Calls onChunk(header, payload, padding, extra, offset) for every chunk; the
// Buffers are views that are only valid during the call. onRaw(buf) sees every byte of the file
// exactly once, in order (used to hash the original independently of the parse).
// Returns { size, parsedBytes, trailing } where trailing is the number of bytes that did not
// form a chunk (0 in all files seen).
function readChunks(file, onChunk, onRaw) {
  const fd = fs.openSync(file, 'r');
  try {
    const size = fs.fstatSync(fd).size;
    let buf = Buffer.allocUnsafe(1 << 24);
    let start = 0, end = 0, filePos = 0, base = 0; // base = file offset of buf[start]
    const fill = (need) => {
      if (end - start >= need) return true;
      if (start + need > buf.length) {
        const nb = need > buf.length ? Buffer.allocUnsafe(Math.max(need, buf.length * 2)) : buf;
        buf.copy(nb, 0, start, end);
        end -= start; start = 0; buf = nb;
      }
      while (end - start < need) {
        const n = fs.readSync(fd, buf, end, buf.length - end, filePos);
        if (n <= 0) return false;
        if (onRaw) onRaw(buf.subarray(end, end + n));
        end += n; filePos += n;
      }
      return true;
    };
    while (base < size) {
      if (!fill(0x20)) break;
      const h = parseChunkHeader(buf, start);
      const total = 8 + h.chunkSize;
      if (h.dataOffset < 0x18 || h.dataOffset + h.paddingSize > h.chunkSize || base + total > size) {
        throw new Error(`bad chunk header at 0x${base.toString(16)} in ${file}`);
      }
      if (!fill(total)) break;
      const p0 = start + 8 + h.dataOffset, p1 = start + total - h.paddingSize;
      onChunk(h, buf.subarray(p0, p1), buf.subarray(p1, start + total), buf.subarray(start + 0x20, p0), base);
      start += total; base += total;
    }
    // drain anything left so onRaw still covers the whole file
    const trailing = size - base;
    if (onRaw) {
      const tmp = Buffer.allocUnsafe(1 << 20);
      for (let n; (n = fs.readSync(fd, tmp, 0, tmp.length, filePos)) > 0; filePos += n) onRaw(tmp.subarray(0, n));
    }
    return { size, parsedBytes: base, trailing };
  } finally {
    fs.closeSync(fd);
  }
}

// ---------------------------------------------------------------------------------------------
// @UTF tables
// ---------------------------------------------------------------------------------------------
//  0  char[4] "@UTF"
//  4  u32     tableSize         bytes following this field
//  8  u16     r08
// 10  u16     rowsOffset        } all offsets relative to byte 8
// 12  u32     stringsOffset     }
// 16  u32     dataOffset        }
// 20  u32     nameOffset        into the string pool
// 24  u16     columnCount
// 26  u16     rowLength
// 28  u32     rowCount
// 32  columns: u8 flags, u32 nameOffset, [constant value if storage == 3]
//     rows, string pool, data pool, zero padding
// flags: high nibble = storage (1 zero / no value, 3 constant in schema, 5 per row), low = type.
const UTF_TYPES = ['u8', 's8', 'u16', 's16', 'u32', 's32', 'u64', 's64', 'f32', 'f64', 'string', 'data'];
const UTF_SIZE = [1, 1, 2, 2, 4, 4, 8, 8, 4, 8, 4, 8];
const STORAGE = { 1: 'zero', 3: 'constant', 5: 'row' };
const STORAGE_CODE = { zero: 1, constant: 3, row: 5 };

function cstr(b, o) {
  let e = o;
  while (e < b.length && b[e] !== 0) e++;
  return b.toString('utf8', o, e);
}

function parseUtf(b) {
  if (b.length < 32 || b.toString('latin1', 0, 4) !== '@UTF') throw new Error('not an @UTF table');
  const tableSize = b.readUInt32BE(4);
  if (8 + tableSize > b.length) throw new Error('@UTF table larger than its chunk payload');
  const t = b.subarray(8, 8 + tableSize);
  const r08 = b.readUInt16BE(8);
  const rowsOffset = t.readUInt16BE(2), stringsOffset = t.readUInt32BE(4), dataOffset = t.readUInt32BE(8);
  const nameOffset = t.readUInt32BE(12), columnCount = t.readUInt16BE(16), rowLength = t.readUInt16BE(18), rowCount = t.readUInt32BE(20);
  const str = (o) => cstr(t, stringsOffset + o);
  const readValue = (type, o) => {
    switch (type) {
      case 0: return t.readUInt8(o);
      case 1: return t.readInt8(o);
      case 2: return t.readUInt16BE(o);
      case 3: return t.readInt16BE(o);
      case 4: return t.readUInt32BE(o);
      case 5: return t.readInt32BE(o);
      case 6: return t.readBigUInt64BE(o).toString();
      case 7: return t.readBigInt64BE(o).toString();
      case 8: { // keep the bits if the float does not survive a JS round trip (NaN payloads)
        const v = t.readFloatBE(o), chk = Buffer.alloc(4); chk.writeFloatBE(v);
        return chk.equals(t.subarray(o, o + 4)) && Number.isFinite(v) ? v : { f32_hex: t.toString('hex', o, o + 4) };
      }
      case 9: {
        const v = t.readDoubleBE(o);
        return Number.isFinite(v) ? v : { f64_hex: t.toString('hex', o, o + 8) };
      }
      case 10: return str(t.readUInt32BE(o));
      case 11: {
        const off = t.readUInt32BE(o), len = t.readUInt32BE(o + 4);
        return { data_hex: t.toString('hex', dataOffset + off, dataOffset + off + len) };
      }
      default: throw new Error('unknown @UTF column type ' + type);
    }
  };
  const columns = [];
  let p = 24;
  for (let i = 0; i < columnCount; i++) {
    const flags = t[p], type = flags & 15, storage = STORAGE[flags >> 4];
    if (!storage || type >= UTF_TYPES.length) throw new Error('unknown @UTF column flags 0x' + flags.toString(16));
    const col = { name: str(t.readUInt32BE(p + 1)), type: UTF_TYPES[type], storage };
    p += 5;
    if (storage === 'constant') { col.value = readValue(type, p); p += UTF_SIZE[type]; }
    columns.push(col);
  }
  if (p !== rowsOffset) throw new Error('@UTF schema does not end at rowsOffset');
  const rows = [];
  for (let r = 0; r < rowCount; r++) {
    let o = rowsOffset + r * rowLength;
    const row = {};
    for (const c of columns) {
      if (c.storage !== 'row') continue;
      const ti = UTF_TYPES.indexOf(c.type);
      row[c.name] = readValue(ti, o);
      o += UTF_SIZE[ti];
    }
    if (o !== rowsOffset + (r + 1) * rowLength) throw new Error('@UTF row length mismatch');
    rows.push(row);
  }
  const table = { name: str(nameOffset), columns, rows };
  if (r08 !== 0) table.r08 = r08;
  return table;
}

// Canonical serialisation, as observed in every table of these files:
//   string pool = "<NULL>", table name, then column names in schema order (a constant string
//   value follows its column name), then per-row strings in row/column order; a string that is
//   already in the pool is not stored again (its first offset is reused);
//   data pool   = per-row blobs in order, starting at the next multiple of 8 after the strings
//   (only one kind of table here has a data pool); table zero-padded to a multiple of 8 bytes.
function buildUtf(table) {
  const pool = [Buffer.from('<NULL>\0')];
  let poolLen = 7;
  const seen = new Map([['<NULL>', 0]]);
  const addStr = (s) => {
    if (seen.has(s)) return seen.get(s);
    const b = Buffer.from(s + '\0', 'utf8'), o = poolLen;
    pool.push(b); poolLen += b.length;
    seen.set(s, o);
    return o;
  };
  const data = [];
  let dataLen = 0;
  const writeValue = (type, v) => {
    const b = Buffer.alloc(UTF_SIZE[type]);
    switch (type) {
      case 0: b.writeUInt8(v); break;
      case 1: b.writeInt8(v); break;
      case 2: b.writeUInt16BE(v); break;
      case 3: b.writeInt16BE(v); break;
      case 4: b.writeUInt32BE(v); break;
      case 5: b.writeInt32BE(v); break;
      case 6: b.writeBigUInt64BE(BigInt(v)); break;
      case 7: b.writeBigInt64BE(BigInt(v)); break;
      case 8: if (typeof v === 'object') Buffer.from(v.f32_hex, 'hex').copy(b); else b.writeFloatBE(v); break;
      case 9: if (typeof v === 'object') Buffer.from(v.f64_hex, 'hex').copy(b); else b.writeDoubleBE(v); break;
      case 10: b.writeUInt32BE(addStr(v)); break;
      case 11: {
        const d = Buffer.from(v.data_hex, 'hex');
        b.writeUInt32BE(d.length ? dataLen : 0, 0); b.writeUInt32BE(d.length, 4);
        data.push(d); dataLen += d.length;
        break;
      }
    }
    return b;
  };
  const nameOffset = addStr(table.name);
  const schema = [];
  for (const c of table.columns) {
    const type = UTF_TYPES.indexOf(c.type);
    const head = Buffer.alloc(5);
    head[0] = (STORAGE_CODE[c.storage] << 4) | type;
    head.writeUInt32BE(addStr(c.name), 1);
    schema.push(head);
    if (c.storage === 'constant') schema.push(writeValue(type, c.value));
  }
  const rowBufs = [];
  for (const row of table.rows) {
    for (const c of table.columns) if (c.storage === 'row') rowBufs.push(writeValue(UTF_TYPES.indexOf(c.type), row[c.name]));
  }
  const schemaBuf = Buffer.concat(schema), rowsBuf = Buffer.concat(rowBufs);
  const rowsOffset = 24 + schemaBuf.length;
  const stringsOffset = rowsOffset + rowsBuf.length;
  const dataOffset = dataLen ? (stringsOffset + poolLen + 7) & ~7 : stringsOffset + poolLen;
  const unpadded = dataOffset + dataLen;
  const tableSize = (unpadded + 7) & ~7;
  const out = Buffer.alloc(8 + tableSize);
  out.write('@UTF', 0, 'latin1');
  out.writeUInt32BE(tableSize, 4);
  out.writeUInt16BE(table.r08 || 0, 8);
  out.writeUInt16BE(rowsOffset, 10);
  out.writeUInt32BE(stringsOffset, 12);
  out.writeUInt32BE(dataLen ? dataOffset : tableSize, 16);
  out.writeUInt32BE(nameOffset, 20);
  out.writeUInt16BE(table.columns.length, 24);
  out.writeUInt16BE(table.rows.length ? rowsBuf.length / table.rows.length : 0, 26);
  out.writeUInt32BE(table.rows.length, 28);
  Buffer.concat([schemaBuf, rowsBuf, ...pool]).copy(out, 32);
  if (dataLen) Buffer.concat(data).copy(out, 8 + dataOffset);
  return out;
}

// Flattens constant columns into each row, for readers that just want values.
function utfRows(table) {
  const rows = table.rows.length ? table.rows : [];
  return rows.map((r) => {
    const o = {};
    for (const c of table.columns) o[c.name] = c.storage === 'row' ? r[c.name] : c.storage === 'constant' ? c.value : null;
    return o;
  });
}

// ---------------------------------------------------------------------------------------------
// Parsed representation of one chunk, and its inverse
// ---------------------------------------------------------------------------------------------
// describeChunk() turns a chunk into JSON-able metadata plus (for stream chunks) a payload that
// belongs to an elementary stream. rebuildChunk() is the inverse and is given only that
// representation. Anything that cannot be expressed structurally is kept as an explicit
// `raw_*_hex` exception, and the callers count those.
function describeChunk(h, payload, padding, extra) {
  const d = {
    sig: h.sig, chno: h.chno, type: h.dataType, paddingSize: h.paddingSize,
    frameTime: h.frameTime, frameRate: h.frameRate,
  };
  // fields that are constant in every chunk seen are only recorded when they differ
  if (h.dataOffset !== 0x18) d.dataOffset = h.dataOffset;
  for (const k of ['r08', 'r0D', 'r0E', 'r18', 'r1C']) if (h[k] !== 0) d[k] = h[k];
  if (extra.length) d.raw_header_extra_hex = extra.toString('hex');
  for (let i = 0; i < padding.length; i++) if (padding[i] !== 0) { d.raw_padding_hex = padding.toString('hex'); break; }

  if (h.dataType === 1 || h.dataType === 3) {
    let ok = false;
    try {
      const table = parseUtf(payload);
      if (buildUtf(table).equals(payload)) { d.utf = table; ok = true; }
    } catch (e) { d.utf_error = e.message; }
    if (!ok) d.raw_payload_hex = payload.toString('hex');
  } else if (h.dataType === 2) {
    // "#HEADER END     ===============\0" style marker: latin1 text + one NUL
    const text = payload.toString('latin1', 0, Math.max(0, payload.length - 1));
    if (payload.length && payload[payload.length - 1] === 0 && !text.includes('\0')) d.marker = text;
    else d.raw_payload_hex = payload.toString('hex');
  } else if (h.dataType === 0) {
    d.payloadSize = payload.length;
  } else {
    d.raw_payload_hex = payload.toString('hex');
  }
  return d;
}

// streamPayload: for type 0 chunks, the next d.payloadSize bytes of that chunk's elementary
// stream. Returns [header, payload, padding] buffers.
function rebuildChunk(d, streamPayload) {
  let payload;
  if (d.raw_payload_hex !== undefined) payload = Buffer.from(d.raw_payload_hex, 'hex');
  else if (d.utf) payload = buildUtf(d.utf);
  else if (d.marker !== undefined) payload = Buffer.from(d.marker + '\0', 'latin1');
  else payload = streamPayload;
  const h = {
    sig: d.sig, dataOffset: d.dataOffset === undefined ? 0x18 : d.dataOffset, paddingSize: d.paddingSize,
    chno: d.chno, dataType: d.type, frameTime: d.frameTime, frameRate: d.frameRate,
    r08: d.r08 || 0, r0D: d.r0D || 0, r0E: d.r0E || 0, r18: d.r18 || 0, r1C: d.r1C || 0,
  };
  const header = buildChunkHeader(h, payload.length, d.raw_header_extra_hex ? Buffer.from(d.raw_header_extra_hex, 'hex') : null);
  const padding = d.raw_padding_hex ? Buffer.from(d.raw_padding_hex, 'hex') : Buffer.alloc(d.paddingSize);
  return [header, payload, padding];
}

// ---------------------------------------------------------------------------------------------
// CRI video mask (applies to @SFV and @ALP stream payloads in these files)
// ---------------------------------------------------------------------------------------------
// A payload shorter than 0x240 bytes is stored as is. Otherwise bytes 0..0x3F are plain and, with
// d = payload + 0x40 and two 32-byte tables m1, m2 = m1 ^ FF:
//   d[0x100..end): c[i] = p[i] ^ k[i & 31], k starts as m2 and after each byte k[i & 31] = p[i] ^ m2[i & 31]
//   d[0..0x100):   k starts as m1; for each i: k[i & 31] ^= p[0x100 + i]; c[i] = p[i] ^ k[i & 31]
// unmaskVideo and maskVideo are exact inverses for any table, so the round trip never depends on
// the table being right; whether the table is right is judged by the bitstream probes.
const MASK_MIN = 0x240;
function unmaskVideo(b, m2) {
  if (b.length < MASK_MIN) return false;
  const k = Buffer.from(m2), n = b.length;
  for (let i = 0x140; i < n; i++) { const r = i & 31, p = b[i] ^ k[r]; b[i] = p; k[r] = p ^ m2[r]; }
  for (let r = 0; r < 32; r++) k[r] = m2[r] ^ 0xff;
  for (let i = 0; i < 0x100; i++) { const r = i & 31; k[r] ^= b[0x140 + i]; b[0x40 + i] ^= k[r]; }
  return true;
}
function maskVideo(b, m2) {
  if (b.length < MASK_MIN) return false;
  const k = Buffer.alloc(32), n = b.length;
  for (let r = 0; r < 32; r++) k[r] = m2[r] ^ 0xff;
  for (let i = 0; i < 0x100; i++) { const r = i & 31; k[r] ^= b[0x140 + i]; b[0x40 + i] ^= k[r]; }
  m2.copy(k);
  for (let i = 0x140; i < n; i++) { const r = i & 31, p = b[i]; b[i] = p ^ k[r]; k[r] = p ^ m2[r]; }
  return true;
}

// The table is not stored anywhere in this project; it is recovered from the movies themselves.
// For payload offsets >= 0x160 the stored byte is p[i] ^ p[i - 32] ^ m2[i & 31]. In MPEG video
// p[i] ^ p[i - 32] is most often 0, so the most frequent stored byte at each offset mod 32 is
// m2 itself. Internal relations between the table bytes (24 of the 32 follow from the other 8)
// are then used as a check that every position was recovered correctly.
class MaskHistogram {
  constructor() { this.hist = Array.from({ length: 32 }, () => new Uint32Array(256)); this.samples = 0; }
  addFile(file) {
    readChunks(file, (h, b) => {
      if ((h.sig !== '@SFV' && h.sig !== '@ALP') || h.dataType !== 0 || b.length < MASK_MIN) return;
      for (let i = 0x160; i < b.length; i++) this.hist[i & 31][b[i]]++;
      this.samples += b.length - 0x160;
    });
  }
  solve() {
    const m2 = Buffer.alloc(32);
    let minRatio = Infinity;
    for (let r = 0; r < 32; r++) {
      const h = this.hist[r];
      let best = 0, second = -1;
      for (let v = 1; v < 256; v++) { if (h[v] > h[best]) { second = best; best = v; } else if (second < 0 || h[v] > h[second]) second = v; }
      m2[r] = best;
      minRatio = Math.min(minRatio, h[second] ? h[best] / h[second] : Infinity);
    }
    return { m2, min_peak_ratio: +minRatio.toFixed(3), relations_ok: maskRelationsHold(m2), samples: this.samples };
  }
}
function maskRelationsHold(m2) {
  const t = Uint8Array.from(m2, (x) => x ^ 0xff), e = new Uint8Array(32);
  for (const i of [0, 1, 2, 3, 4, 5, 6, 0x13]) e[i] = t[i]; // the 8 independent bytes
  e[7] = e[0] ^ 0xff; e[8] = e[2] + e[1]; e[9] = e[1] - e[7]; e[10] = e[2] ^ 0xff; e[11] = e[1] ^ 0xff;
  e[12] = e[11] + e[9]; e[13] = e[8] - e[3]; e[14] = e[13] ^ 0xff; e[15] = e[10] - e[11]; e[16] = e[8] - e[15];
  e[17] = e[16] ^ e[7]; e[18] = e[15] ^ 0xff; e[20] = e[4] - 0x32; e[21] = e[5] + 0xed; e[22] = e[6] ^ 0xf3;
  e[23] = e[19] - e[15]; e[24] = e[21] + e[7]; e[25] = 0x21 - e[19]; e[26] = e[20] ^ e[23]; e[27] = e[22] + e[22];
  e[28] = e[23] + 0x44; e[29] = e[3] + e[4]; e[30] = e[5] - e[22]; e[31] = e[29] ^ e[19];
  return Buffer.from(e).equals(Buffer.from(t));
}
// Feeds the smallest files first until the table is unambiguous and self-consistent.
function findVideoMask(files) {
  const sorted = files.map((f) => [f, fs.statSync(f).size]).sort((a, b) => a[1] - b[1]);
  const mh = new MaskHistogram();
  let used = 0, next = 4 << 20, res = null;
  for (const [f] of sorted) {
    mh.addFile(f); used++;
    if (mh.samples < next) continue;
    res = mh.solve();
    if (res.relations_ok && res.min_peak_ratio >= 1.05) break;
    next = mh.samples * 2;
    if (mh.samples > (1 << 30)) break;
  }
  if (!res) res = mh.solve();
  const ok = res.relations_ok && res.min_peak_ratio >= 1.05;
  return { ok, m2: ok ? res.m2 : null, files_used: used, samples: res.samples, min_peak_ratio: res.min_peak_ratio, relations_ok: res.relations_ok,
    mask_sha256: ok ? crypto.createHash('sha256').update(res.m2).digest('hex') : null };
}
const isVideoSig = (sig) => sig === '@SFV' || sig === '@ALP';

const streamKey = (sig, chno) => `${sig}:${chno}`;

// ---------------------------------------------------------------------------------------------
// Bitstream probes. They never alter data; they only gather evidence.
// ---------------------------------------------------------------------------------------------
const ZZ = Buffer.from([0, 0]);

// H.264 Annex B. Inside a NAL unit the byte sequences 00 00 00, 00 00 01 and 00 00 02 cannot
// occur, and 00 00 03 must be followed by a byte <= 03 (emulation prevention). A masked or
// encrypted payload would violate these at a rate of about 1 per 2^24 bytes. That test has no
// power on a file of a few MB, so the probe also requires every chunk (one access unit) to
// contain at least one slice NAL: in masked data only start codes in the first 0x40 bytes survive.
class H264Probe {
  constructor() {
    this.codec = 'h264';
    this.chunks = 0; this.bytes = 0; this.chunksStartingWithStartCode = 0;
    this.nal = {}; this.forbiddenBit = 0; this.badEscape = 0; this.seq0002 = 0; this.escapes = 0; this.zeroRuns = 0;
    this.sps = null; this.chunksWithoutSlice = 0; this.slicesMin = Infinity; this.slicesMax = 0;
  }
  push(b) {
    this.chunks++; this.bytes += b.length;
    let slices = 0;
    if (b.length >= 4 && b[0] === 0 && b[1] === 0 && (b[2] === 1 || (b[2] === 0 && b[3] === 1))) this.chunksStartingWithStartCode++;
    let i = 0;
    while ((i = b.indexOf(ZZ, i)) !== -1) {
      let j = i + 2;
      while (j < b.length && b[j] === 0) j++; // run of zeros i..j-1
      if (j >= b.length) { this.zeroRuns++; break; } // trailing zeros at the end of the chunk
      const v = b[j];
      if (v === 1) {
        const hdr = b[j + 1];
        if (hdr === undefined) break;
        if (hdr & 0x80) this.forbiddenBit++;
        const t = hdr & 31;
        this.nal[t] = (this.nal[t] || 0) + 1;
        if (t === 1 || t === 5) slices++;
        if (t === 7 && !this.sps) this.sps = { profile_idc: b[j + 2], constraint_flags: b[j + 3], level_idc: b[j + 4] };
      } else if (j - i > 2) {
        this.zeroRuns++; // 00 00 00 not leading into a start code
      } else if (v === 3) {
        this.escapes++;
        if (j + 1 < b.length && b[j + 1] > 3) this.badEscape++;
      } else if (v === 2) {
        this.seq0002++;
      }
      i = j + 1;
    }
    if (!slices) this.chunksWithoutSlice++;
    if (slices < this.slicesMin) this.slicesMin = slices;
    if (slices > this.slicesMax) this.slicesMax = slices;
  }
  result() {
    const violations = this.forbiddenBit + this.badEscape + this.seq0002 + this.zeroRuns;
    return {
      codec: this.codec, access_chunks: this.chunks, chunks_starting_with_start_code: this.chunksStartingWithStartCode,
      chunks_without_slice_nal: this.chunksWithoutSlice, slice_nals_per_chunk_min: this.chunks ? this.slicesMin : 0, slice_nals_per_chunk_max: this.slicesMax,
      nal_units_by_type: this.nal, sps: this.sps, emulation_prevention_bytes: this.escapes,
      annexb_violations: { forbidden_zero_bit: this.forbiddenBit, bad_escape: this.badEscape, seq_000002: this.seq0002, stray_zero_run: this.zeroRuns },
      expected_violations_if_random: +(this.bytes / 16777216 * (3 + 252 / 256)).toFixed(2),
      plaintext_evidence: violations === 0 && this.chunksStartingWithStartCode === this.chunks && this.chunksWithoutSlice === 0,
    };
  }
}

// MPEG-1/2 video. Every picture must carry slice start codes 01..mb_rows in order; the slices
// are spread over the whole picture, so a mask applied past the first bytes would destroy them.
class MpegProbe {
  constructor() {
    this.codec = 'mpeg'; this.chunks = 0; this.bytes = 0; this.chunksStartingWithStartCode = 0;
    this.codes = {}; this.pictures = 0; this.picturesComplete = 0; this.sliceOrderErrors = 0;
    this.seq = null; this.mbRows = 0; this.lastSlice = 0; this.inPicture = false; this.mpeg2 = false;
  }
  endPicture() {
    if (!this.inPicture) return;
    if (this.lastSlice === this.mbRows) this.picturesComplete++;
    this.inPicture = false;
  }
  push(b) {
    this.chunks++; this.bytes += b.length;
    if (b.length >= 4 && b[0] === 0 && b[1] === 0 && b[2] === 1) this.chunksStartingWithStartCode++;
    let i = 0;
    while ((i = b.indexOf(ZZ, i)) !== -1) {
      let j = i + 2;
      while (j < b.length && b[j] === 0) j++;
      if (j + 1 >= b.length) break;
      if (b[j] !== 1) { i = j + 1; continue; }
      const code = b[j + 1];
      const cat = code === 0 ? 'picture' : code <= 0xaf ? 'slice' : code === 0xb3 ? 'sequence_header' : code === 0xb5 ? 'extension'
        : code === 0xb8 ? 'gop' : code === 0xb7 ? 'sequence_end' : code === 0xb2 ? 'user_data' : 'other_' + code.toString(16);
      this.codes[cat] = (this.codes[cat] || 0) + 1;
      if (code === 0xb3) {
        this.endPicture();
        if (j + 8 < b.length) {
          const w = (b[j + 2] << 4) | (b[j + 3] >> 4), h = ((b[j + 3] & 15) << 8) | b[j + 4];
          if (!this.seq) this.seq = { width: w, height: h, aspect_code: b[j + 5] >> 4, frame_rate_code: b[j + 5] & 15 };
          this.mbRows = (h + 15) >> 4;
        }
      } else if (code === 0xb5) {
        this.mpeg2 = true;
      } else if (code === 0) {
        this.endPicture();
        this.pictures++; this.inPicture = true; this.lastSlice = 0;
      } else if (code <= 0xaf) {
        if (code !== this.lastSlice + 1 && code !== this.lastSlice) this.sliceOrderErrors++;
        this.lastSlice = code;
      } else if (code === 0xb8 || code === 0xb7) {
        this.endPicture();
      }
      i = j + 2;
    }
  }
  result() {
    this.endPicture();
    return {
      codec: this.mpeg2 ? 'mpeg2' : 'mpeg1', access_chunks: this.chunks, chunks_starting_with_start_code: this.chunksStartingWithStartCode,
      start_codes: this.codes, sequence_header: this.seq, macroblock_rows: this.mbRows,
      pictures: this.pictures, pictures_with_all_slice_rows: this.picturesComplete, slice_order_errors: this.sliceOrderErrors,
      plaintext_evidence: this.pictures > 0 && this.picturesComplete === this.pictures && this.sliceOrderErrors === 0,
    };
  }
}

const CRC16 = (() => {
  const t = new Uint16Array(256);
  for (let i = 0; i < 256; i++) { let c = i << 8; for (let k = 0; k < 8; k++) c = c & 0x8000 ? ((c << 1) ^ 0x8005) & 0xffff : (c << 1) & 0xffff; t[i] = c; }
  return t;
})();
function crc16(b, s, e) {
  let c = 0;
  for (let i = s; i < e; i++) c = ((c << 8) & 0xffff) ^ CRC16[(c >> 8) ^ b[i]];
  return c;
}

// HCA: header (plain or with the high bit set on its tags), then fixed-size blocks, each starting
// with FF FF and ending in a CRC-16 that makes the CRC of the whole block zero. A container-level
// mask over the audio payload would break those CRCs; the codec's own cipher is the `ciph` field.
function parseHcaHeader(b) {
  const tag = (o) => String.fromCharCode(b[o] & 0x7f, b[o + 1] & 0x7f, b[o + 2] & 0x7f, b[o + 3] & 0x7f).replace(/\0+$/, '');
  if (b.length < 8 || tag(0) !== 'HCA') return null;
  const h = { tags_masked: (b[0] & 0x80) !== 0, version: b.readUInt16BE(4), header_size: b.readUInt16BE(6), chunks: [] };
  if (b.length < h.header_size) return h;
  let o = 8;
  while (o + 4 <= h.header_size - 2) {
    const t = tag(o);
    h.chunks.push(t);
    if (t === 'fmt') {
      h.channels = b[o + 4]; h.sample_rate = b.readUIntBE(o + 5, 3); h.block_count = b.readUInt32BE(o + 8);
      h.encoder_delay = b.readUInt16BE(o + 12); h.encoder_padding = b.readUInt16BE(o + 14); o += 16;
    } else if (t === 'comp') {
      h.block_size = b.readUInt16BE(o + 4); h.comp = b.toString('hex', o + 6, o + 16); o += 16;
    } else if (t === 'dec') {
      h.block_size = b.readUInt16BE(o + 4); h.dec = b.toString('hex', o + 6, o + 12); o += 12;
    } else if (t === 'vbr') { o += 8;
    } else if (t === 'ath') { h.ath_type = b.readUInt16BE(o + 4); o += 6;
    } else if (t === 'loop') { h.loop = [b.readUInt32BE(o + 4), b.readUInt32BE(o + 8)]; o += 16;
    } else if (t === 'ciph') { h.cipher_type = b.readUInt16BE(o + 4); o += 6;
    } else if (t === 'rva') { h.rva_volume = b.readFloatBE(o + 4); o += 8;
    } else if (t === 'comm') { o += 5 + b[o + 4];
    } else if (t === 'pad') { break;
    } else { h.chunks.pop(); h.unparsed_from = o; break; }
  }
  h.header_crc_ok = crc16(b, 0, h.header_size) === 0;
  return h;
}

class HcaProbe {
  constructor() { this.codec = 'hca'; this.pending = []; this.pendingLen = 0; this.header = null; this.blocks = 0; this.syncOk = 0; this.crcOk = 0; this.bytes = 0; this.state = 0; }
  push(b) {
    this.bytes += b.length;
    this.pending.push(Buffer.from(b)); this.pendingLen += b.length;
    if (this.state === 2) return;
    const need = this.state === 0 ? (this.header ? this.header.header_size : 8) : this.header.block_size;
    if (this.pendingLen < need) return;
    let buf = this.pending.length === 1 ? this.pending[0] : Buffer.concat(this.pending), o = 0;
    for (;;) {
      if (this.state === 0) {
        if (buf.length - o < 8) break;
        const h = parseHcaHeader(buf.subarray(o));
        if (!h) { this.state = 2; break; }
        if (buf.length - o < h.header_size) { this.header = h; break; }
        this.header = h; o += h.header_size;
        if (!h.block_size) { this.state = 2; break; }
        this.state = 1;
      } else {
        const n = this.header.block_size;
        if (buf.length - o < n) break;
        this.blocks++;
        if (buf[o] === 0xff && buf[o + 1] === 0xff) this.syncOk++;
        if (crc16(buf, o, o + n) === 0) this.crcOk++;
        o += n;
      }
    }
    this.pending = o < buf.length ? [buf.subarray(o)] : []; this.pendingLen = buf.length - o;
  }
  result() {
    const h = this.header;
    return {
      codec: 'hca', header: h, blocks: this.blocks, blocks_with_sync: this.syncOk, blocks_with_valid_crc: this.crcOk, leftover_bytes: this.pendingLen,
      plaintext_evidence: !!h && h.cipher_type === 0 && !h.tags_masked && this.blocks > 0 && this.crcOk === this.blocks && this.syncOk === this.blocks && this.pendingLen === 0,
    };
  }
}

// ADX: 80 00, u16 copyright offset, encoding, block size, bit depth, channels, rate, samples.
class AdxProbe {
  constructor(first) {
    this.codec = 'adx'; this.bytes = 0;
    this.header = first.length >= 20 ? {
      data_offset: first.readUInt16BE(2) + 4, encoding: first[4], block_size: first[5], bit_depth: first[6], channels: first[7],
      sample_rate: first.readUInt32BE(8), sample_count: first.readUInt32BE(12), highpass: first.readUInt16BE(16), version: first[18], flags: first[19],
    } : null;
    if (this.header && first.length >= this.header.data_offset) this.header.cri_marker = first.toString('latin1', this.header.data_offset - 6, this.header.data_offset) === '(c)CRI';
  }
  push(b) { this.bytes += b.length; }
  result() { return { codec: 'adx', header: this.header, plaintext_evidence: !!(this.header && this.header.cri_marker) }; }
}

class UnknownProbe {
  constructor(first) { this.first = first.toString('hex', 0, Math.min(16, first.length)); }
  push() {}
  result() { return { codec: 'unknown', first_bytes_hex: this.first, plaintext_evidence: null }; }
}

// Chooses a probe from the first payload of a stream. The choice is by bitstream signature only.
function makeProbe(sig, first) {
  if (sig === '@SFV' || sig === '@ALP') {
    if (first.length >= 5 && first[0] === 0 && first[1] === 0 && first[2] === 0 && first[3] === 1 && (first[4] & 0x80) === 0) return new H264Probe();
    if (first.length >= 4 && first[0] === 0 && first[1] === 0 && first[2] === 1 && (first[3] === 0xb3 || first[3] === 0)) return new MpegProbe();
    if (first.length >= 4 && first.toString('latin1', 0, 4) === 'DKIF') return new UnknownProbe(first); // VP9 in IVF; not seen
  } else if (sig === '@SFA') {
    if (first.length >= 4 && (first.readUInt32BE(0) & 0x7f7f7f7f) === 0x48434100) return new HcaProbe();
    if (first.length >= 2 && first[0] === 0x80 && first[1] === 0) return new AdxProbe(first);
  }
  return new UnknownProbe(first);
}

const STREAM_EXT = { h264: '.h264', mpeg1: '.m1v', mpeg2: '.m2v', mpeg: '.m2v', hca: '.hca', adx: '.adx', unknown: '.bin' };

// ---------------------------------------------------------------------------------------------
// Whole-file pass
// ---------------------------------------------------------------------------------------------
// scan(file, sinks): one sequential pass that
//   - hashes the original bytes (sha256_original),
//   - builds the parsed representation chunk by chunk,
//   - rebuilds every chunk from that representation and hashes the result (sha256_rebuilt),
//   - hashes and probes each elementary stream.
// sinks.onStream(key, payload) receives elementary stream bytes (optional);
// sinks.onDescriptor(d) receives every chunk descriptor in file order (optional).
// opts.videoMask (32-byte m2 table): @SFV/@ALP stream payloads are unmasked before they reach the
// elementary stream and masked again by the rebuild.
function scan(file, sinks = {}, opts = {}) {
  const m2 = opts.videoMask || null;
  const hOrig = crypto.createHash('sha256'), hRebuilt = crypto.createHash('sha256');
  const chunkCounts = {}, streams = new Map(), headers = [];
  const exceptions = { raw_padding: 0, raw_payload: 0, raw_header_extra: 0, nonzero_reserved: 0, nondefault_data_offset: 0 };
  const align = { chunk_total_mod_0x20_nonzero: 0, chunk_offset_mod_0x20_nonzero: 0 };
  let chunks = 0, rebuiltBytes = 0, crid = null, firstStreamOffset = null;
  const paddingSizes = { min: Infinity, max: 0 };
  const info = readChunks(file, (h, payload, padding, extra, offset) => {
    chunks++;
    const ck = `${h.sig}/${TYPE_NAMES[h.dataType] || h.dataType}`;
    chunkCounts[ck] = (chunkCounts[ck] || 0) + 1;
    if ((8 + h.chunkSize) & 0x1f) align.chunk_total_mod_0x20_nonzero++;
    if (offset & 0x1f) align.chunk_offset_mod_0x20_nonzero++;
    if (h.paddingSize < paddingSizes.min) paddingSizes.min = h.paddingSize;
    if (h.paddingSize > paddingSizes.max) paddingSizes.max = h.paddingSize;
    const d = describeChunk(h, payload, padding, extra);
    if (d.raw_padding_hex) exceptions.raw_padding++;
    if (d.raw_payload_hex !== undefined) exceptions.raw_payload++;
    if (d.raw_header_extra_hex) exceptions.raw_header_extra++;
    if (d.dataOffset !== undefined) exceptions.nondefault_data_offset++;
    if (d.r08 || d.r0D || d.r0E || d.r18 || d.r1C) exceptions.nonzero_reserved++;

    const key = streamKey(h.sig, h.chno);
    let s = streams.get(key);
    if (!s) {
      s = { sig: h.sig, chno: h.chno, chunks: 0, bytes: 0, hash: crypto.createHash('sha256'), probe: null, rawProbe: null, header: null, metadata: [], markers: [],
        unmasked: false, chunksBelowMaskMin: 0, largestPlainChunk: 0, smallestMaskedChunk: Infinity, minPayload: Infinity, maxPayload: 0,
        frameRates: {}, firstTime: null, lastTime: null };
      streams.set(key, s);
    }
    if (h.dataType === 0) {
      if (firstStreamOffset === null) firstStreamOffset = offset;
      if (payload.length < s.minPayload) s.minPayload = payload.length;
      if (payload.length > s.maxPayload) s.maxPayload = payload.length;
      if (m2 && isVideoSig(h.sig)) {
        // probe the stored bytes first (evidence that they are not a plain bitstream), then unmask in place
        if (!s.rawProbe) s.rawProbe = makeProbe(h.sig, payload);
        s.rawProbe.push(payload);
        s.unmasked = true;
        if (unmaskVideo(payload, m2)) { if (payload.length < s.smallestMaskedChunk) s.smallestMaskedChunk = payload.length; }
        else { s.chunksBelowMaskMin++; if (payload.length > s.largestPlainChunk) s.largestPlainChunk = payload.length; }
      }
      if (!s.probe) s.probe = makeProbe(h.sig, payload);
      s.probe.push(payload);
      s.hash.update(payload);
      s.chunks++; s.bytes += payload.length;
      s.frameRates[h.frameRate] = (s.frameRates[h.frameRate] || 0) + 1;
      if (s.firstTime === null) s.firstTime = h.frameTime;
      s.lastTime = h.frameTime;
      if (sinks.onStream) sinks.onStream(key, payload, s);
    } else if (h.dataType === 1 && d.utf) {
      if (h.sig === 'CRID') crid = d.utf; else s.header = d.utf;
    } else if (h.dataType === 3 && d.utf) {
      s.metadata.push(d.utf);
    } else if (h.dataType === 2 && d.marker !== undefined) {
      s.markers.push(d.marker);
    }
    if (h.dataType !== 0) headers.push({ offset, frameRate: h.frameRate, frameTime: h.frameTime });
    if (sinks.onDescriptor) sinks.onDescriptor(d, offset);

    // The rebuild sees only the descriptor and, for stream chunks, the elementary stream bytes.
    if (h.dataType === 0 && s.unmasked) maskVideo(payload, m2);
    for (const part of rebuildChunk(d, h.dataType === 0 ? payload : null)) { hRebuilt.update(part); rebuiltBytes += part.length; }
  }, (raw) => hOrig.update(raw));
  if (paddingSizes.min === Infinity) paddingSizes.min = 0;
  return {
    size: info.size, trailing_bytes: info.trailing, chunks, chunkCounts, exceptions, align, paddingSizes, firstStreamOffset,
    sha256_original: hOrig.digest('hex'), sha256_rebuilt: hRebuilt.digest('hex'), rebuilt_size: rebuiltBytes,
    crid, streams,
  };
}

// Remux from an extracted directory: descriptors (file order) + elementary stream files.
// write(buf) receives the container bytes in order.
function remux(descriptors, openStream, write, videoMask) {
  const pos = new Map();
  for (const d of descriptors) {
    let payload = null;
    if (d.type === 0 && d.raw_payload_hex === undefined) {
      const key = streamKey(d.sig, d.chno);
      const fd = openStream(key);
      const o = pos.get(key) || 0;
      payload = Buffer.allocUnsafe(d.payloadSize);
      if (fs.readSync(fd, payload, 0, d.payloadSize, o) !== d.payloadSize) throw new Error('elementary stream too short: ' + key);
      pos.set(key, o + d.payloadSize);
      if (videoMask && isVideoSig(d.sig)) maskVideo(payload, videoMask);
    }
    for (const part of rebuildChunk(d, payload)) write(part);
  }
  return pos;
}

module.exports = {
  VERSION, TYPE_NAMES, UTF_TYPES, STREAM_EXT,
  parseChunkHeader, buildChunkHeader, readChunks, parseUtf, buildUtf, utfRows,
  describeChunk, rebuildChunk, streamKey, scan, remux,
  unmaskVideo, maskVideo, findVideoMask, maskRelationsHold, isVideoSig, MASK_MIN,
  parseHcaHeader, makeProbe, crc16, H264Probe, MpegProbe, HcaProbe, AdxProbe,
};
