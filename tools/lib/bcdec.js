// Block-compressed texture decoding (BC1/DXT1, BC3/DXT5, BC4, BC5, BC7) and uncompressed BGRA/G8 to RGBA8, plus a PNG writer.
// Used by tools/sora_pack.js to turn the .dds files of assets_converted/textures into PNG.
//
//   decodeDds(buf, mip = 0)        -> { width, height, format, rgba: Buffer, stats }
//   encodePng(width, height, rgba, { alpha = true }) -> Buffer   (8-bit RGB or RGBA, zlib level 9)
//   decodePng(buf)                 -> { width, height, channels, data }   (only what encodePng writes)
//   selfCheck()                    -> throws when the BC7 tables are inconsistent
//
// BC7 follows the format as specified for DXGI_FORMAT_BC7_UNORM (8 modes, partition tables of 64 shapes
// for 2 and 3 subsets, anchor index tables, 2/3/4-bit interpolation weights). The tables were typed in,
// not copied from a file: selfCheck() verifies that every anchor index lies in its own subset and that
// every shape uses all of its subsets, and tools/sora_pack.js compares each decoded top mip with the
// decoded next mip of the same texture (blocks of different shapes), so a wrong table entry shows.
const zlib = require('zlib');
const { readDds } = require('./texture');

const VERSION = '1.0.0';

// ---------------------------------------------------------------- BC1 / BC3 / BC4 / BC5
function color565(c) { const r = (c >> 11) & 31, g = (c >> 5) & 63, b = c & 31; return [(r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2)]; }
function bc1Block(src, o, out, forceFour) {
  const c0 = src.readUInt16LE(o), c1 = src.readUInt16LE(o + 2);
  const a = color565(c0), b = color565(c1);
  const pal = [[...a, 255], [...b, 255], null, null];
  if (c0 > c1 || forceFour) {
    pal[2] = [((2 * a[0] + b[0]) / 3 + 0.5) | 0, ((2 * a[1] + b[1]) / 3 + 0.5) | 0, ((2 * a[2] + b[2]) / 3 + 0.5) | 0, 255];
    pal[3] = [((a[0] + 2 * b[0]) / 3 + 0.5) | 0, ((a[1] + 2 * b[1]) / 3 + 0.5) | 0, ((a[2] + 2 * b[2]) / 3 + 0.5) | 0, 255];
  } else {
    pal[2] = [(a[0] + b[0]) >> 1, (a[1] + b[1]) >> 1, (a[2] + b[2]) >> 1, 255];
    pal[3] = [0, 0, 0, 0];
  }
  const bits = src.readUInt32LE(o + 4);
  for (let i = 0; i < 16; i++) { const p = pal[(bits >>> (2 * i)) & 3]; out[i * 4] = p[0]; out[i * 4 + 1] = p[1]; out[i * 4 + 2] = p[2]; out[i * 4 + 3] = p[3]; }
}
// One 8-byte interpolated single-channel block (the alpha block of BC3, each half of BC5, BC4).
function alphaBlock(src, o, out, channel) {
  const a0 = src[o], a1 = src[o + 1];
  const pal = [a0, a1, 0, 0, 0, 0, 0, 0];
  if (a0 > a1) for (let i = 1; i < 7; i++) pal[i + 1] = (((7 - i) * a0 + i * a1) / 7 + 0.5) | 0;
  else { for (let i = 1; i < 5; i++) pal[i + 1] = (((5 - i) * a0 + i * a1) / 5 + 0.5) | 0; pal[6] = 0; pal[7] = 255; }
  let bits = 0n;
  for (let i = 0; i < 6; i++) bits |= BigInt(src[o + 2 + i]) << BigInt(8 * i);
  for (let i = 0; i < 16; i++) out[i * 4 + channel] = pal[Number((bits >> BigInt(3 * i)) & 7n)];
}

// ---------------------------------------------------------------- BC7
const P2 = [
  '0011001100110011', '0001000100010001', '0111011101110111', '0001001100110111', '0000000100010011', '0011011101111111', '0001001101111111', '0000000100110111',
  '0000000000010011', '0011011111111111', '0000000101111111', '0000000000010111', '0001011111111111', '0000000011111111', '0000111111111111', '0000000000001111',
  '0000100011101111', '0111000100000000', '0000000010001110', '0111001100010000', '0011000100000000', '0000100011001110', '0000000010001100', '0111001100110001',
  '0011000100010000', '0000100010001100', '0110011001100110', '0011011001101100', '0001011111101000', '0000111111110000', '0111000110001110', '0011100110011100',
  '0101010101010101', '0000111100001111', '0101101001011010', '0011001111001100', '0011110000111100', '0101010110101010', '0110100101101001', '0101101010100101',
  '0111001111001110', '0001001111001000', '0011001001001100', '0011101111011100', '0110100110010110', '0011110011000011', '0110011010011001', '0000011001100000',
  '0100111001000000', '0010011100100000', '0000001001110010', '0000010011100100', '0110110010010011', '0011011011001001', '0110001110011100', '0011100111000110',
  '0110110011001001', '0110001100111001', '0111111010000001', '0001100011100111', '0000111100110011', '0011001111110000', '0010001011101110', '0100010001110111',
].map((s) => [...s].map(Number));
const P3 = [
  '0011001102212222', '0001001122112221', '0000200122112211', '0222002200110111', '0000000011221122', '0011001100220022', '0022002211111111', '0011001122112211',
  '0000000011112222', '0000111111112222', '0000111122222222', '0012001200120012', '0112011201120112', '0122012201220122', '0011011211221222', '0011200122002220',
  '0001001101121122', '0111001120012200', '0000112211221122', '0022002200221111', '0111011102220222', '0001000122212221', '0000001101220122', '0000110022102210',
  '0122012200110000', '0012001211222222', '0110122112210110', '0000011012211221', '0022110211020022', '0110011020022222', '0011012201220011', '0000200022112221',
  '0000000211221222', '0222002200120011', '0011001200220222', '0120012001200120', '0000111122220000', '0120120120120120', '0120201212010120', '0011220011220011',
  '0011112222000011', '0101010122222222', '0000000021212121', '0022112200221122', '0022001100220011', '0220122102201221', '0101222222220101', '0000212121212121',
  '0101010101012222', '0222011102220111', '0002111200021112', '0000211221122112', '0222011101110222', '0002111211120002', '0110011001102222', '0000000021122112',
  '0110011022222222', '0022001100110022', '0022112211220022', '0000000000002112', '0002000100020001', '0222122202221222', '0101222222222222', '0111201122012220',
].map((s) => [...s].map(Number));
const A2 = [15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 2, 8, 2, 2, 8, 8, 15, 2, 8, 2, 2, 8, 8, 2, 2,
  15, 15, 6, 8, 2, 8, 15, 15, 2, 8, 2, 2, 2, 15, 15, 6, 6, 2, 6, 8, 15, 15, 2, 2, 15, 15, 15, 15, 15, 2, 2, 15];
const A3a = [3, 3, 15, 15, 8, 3, 15, 15, 8, 8, 6, 6, 6, 5, 3, 3, 3, 3, 8, 15, 3, 3, 6, 10, 5, 8, 8, 6, 8, 5, 15, 15,
  8, 15, 3, 5, 6, 10, 8, 15, 15, 3, 15, 5, 15, 15, 15, 15, 3, 15, 5, 5, 5, 8, 5, 10, 5, 10, 8, 13, 15, 12, 3, 3];
const A3b = [15, 8, 8, 3, 15, 15, 3, 8, 15, 15, 15, 15, 15, 15, 15, 8, 15, 8, 15, 3, 15, 8, 15, 8, 3, 15, 6, 10, 15, 15, 10, 8,
  15, 3, 15, 10, 10, 8, 9, 10, 6, 15, 8, 15, 3, 6, 6, 8, 15, 3, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 3, 15, 15, 8];
const W = { 2: [0, 21, 43, 64], 3: [0, 9, 18, 27, 37, 46, 55, 64], 4: [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64] };
// [subsets, partition bits, rotation bits, index selection bits, colour bits, alpha bits, per-endpoint p bits, per-subset p bits, index bits, second index bits]
const MODES = [[3, 4, 0, 0, 4, 0, 1, 0, 3, 0], [2, 6, 0, 0, 6, 0, 0, 1, 3, 0], [3, 6, 0, 0, 5, 0, 0, 0, 2, 0], [2, 6, 0, 0, 7, 0, 1, 0, 2, 0],
  [1, 0, 2, 1, 5, 6, 0, 0, 2, 3], [1, 0, 2, 0, 7, 8, 0, 0, 2, 2], [1, 0, 0, 0, 7, 7, 1, 0, 4, 0], [2, 6, 0, 0, 5, 5, 1, 0, 2, 0]];

function selfCheck() {
  if (P2.length !== 64 || P3.length !== 64) throw new Error('bc7: partition table size');
  for (let p = 0; p < 64; p++) {
    if (P2[p].length !== 16 || P3[p].length !== 16) throw new Error('bc7: shape length ' + p);
    if (P2[p][0] !== 0 || P3[p][0] !== 0) throw new Error('bc7: pixel 0 not in subset 0, shape ' + p);
    if (!P2[p].includes(1) || !P3[p].includes(1) || !P3[p].includes(2)) throw new Error('bc7: unused subset in shape ' + p);
    if (P2[p][A2[p]] !== 1) throw new Error('bc7: 2-subset anchor outside its subset, shape ' + p);
    if (P3[p][A3a[p]] !== 1) throw new Error('bc7: 3-subset anchor of subset 1 outside its subset, shape ' + p);
    if (P3[p][A3b[p]] !== 2) throw new Error('bc7: 3-subset anchor of subset 2 outside its subset, shape ' + p);
  }
  return true;
}

function bc7Block(src, o, out, stats) {
  const lo = src.readBigUInt64LE(o), hi = src.readBigUInt64LE(o + 8);
  let pos = 0;
  const bits = (n) => {
    if (n === 0) return 0;
    let v;
    if (pos >= 64) v = Number((hi >> BigInt(pos - 64)) & ((1n << BigInt(n)) - 1n));
    else if (pos + n <= 64) v = Number((lo >> BigInt(pos)) & ((1n << BigInt(n)) - 1n));
    else { const a = 64 - pos; v = Number(lo >> BigInt(pos)) | (Number(hi & ((1n << BigInt(n - a)) - 1n)) << a); }
    pos += n;
    return v;
  };
  let mode = 0;
  while (mode < 8 && bits(1) === 0) mode++;
  if (mode === 8) { out.fill(0, 0, 64); if (stats) stats.reserved = (stats.reserved || 0) + 1; return -1; }
  const [ns, pb, rb, isb, cb, ab, epb, spb, ib, ib2] = MODES[mode];
  const part = bits(pb), rot = bits(rb), idxSel = bits(isb);
  if (stats) { stats.modes[mode]++; if (ns > 1) stats.shapes[ns][part]++; }
  const ne = ns * 2;
  const ep = [];
  for (let i = 0; i < ne; i++) ep.push([0, 0, 0, 255]);
  for (let c = 0; c < 3; c++) for (let i = 0; i < ne; i++) ep[i][c] = bits(cb);
  if (ab) for (let i = 0; i < ne; i++) ep[i][3] = bits(ab);
  if (epb) { for (let i = 0; i < ne; i++) { const p = bits(1); for (let c = 0; c < (ab ? 4 : 3); c++) ep[i][c] = (ep[i][c] << 1) | p; } }
  else if (spb) { for (let s = 0; s < ns; s++) { const p = bits(1); for (let i = 0; i < 2; i++) for (let c = 0; c < 3; c++) ep[s * 2 + i][c] = (ep[s * 2 + i][c] << 1) | p; } }
  const cprec = cb + (epb || spb ? 1 : 0), aprec = ab ? ab + (epb ? 1 : 0) : 0;
  for (let i = 0; i < ne; i++) {
    for (let c = 0; c < 3; c++) { const v = ep[i][c] << (8 - cprec); ep[i][c] = v | (v >> cprec); }
    if (ab) { const v = ep[i][3] << (8 - aprec); ep[i][3] = v | (v >> aprec); }
  }
  const shape = ns === 1 ? null : ns === 2 ? P2[part] : P3[part];
  const anchors = ns === 1 ? [0] : ns === 2 ? [0, A2[part]] : [0, A3a[part], A3b[part]];
  const idx1 = new Array(16), idx2 = new Array(16);
  for (let i = 0; i < 16; i++) idx1[i] = bits(anchors.includes(i) ? ib - 1 : ib);
  if (ib2) for (let i = 0; i < 16; i++) idx2[i] = bits(i === 0 ? ib2 - 1 : ib2);
  for (let i = 0; i < 16; i++) {
    const s = shape ? shape[i] : 0;
    const e0 = ep[s * 2], e1 = ep[s * 2 + 1];
    let cw, aw;
    if (!ib2) { cw = aw = W[ib][idx1[i]]; }
    else if (idxSel) { cw = W[ib2][idx2[i]]; aw = W[ib][idx1[i]]; }
    else { cw = W[ib][idx1[i]]; aw = W[ib2][idx2[i]]; }
    let r = ((64 - cw) * e0[0] + cw * e1[0] + 32) >> 6, g = ((64 - cw) * e0[1] + cw * e1[1] + 32) >> 6, b = ((64 - cw) * e0[2] + cw * e1[2] + 32) >> 6;
    let a = ab ? ((64 - aw) * e0[3] + aw * e1[3] + 32) >> 6 : 255;
    if (rot === 1) { const t = a; a = r; r = t; } else if (rot === 2) { const t = a; a = g; g = t; } else if (rot === 3) { const t = a; a = b; b = t; }
    out[i * 4] = r; out[i * 4 + 1] = g; out[i * 4 + 2] = b; out[i * 4 + 3] = a;
  }
  return mode;
}

// ---------------------------------------------------------------- DDS -> RGBA
const BLOCK_BYTES = { PF_DXT1: 8, PF_DXT5: 16, PF_BC4: 8, PF_BC5: 16, PF_BC7: 16 };
function decodeDds(buf, mip = 0) {
  const d = readDds(buf);
  if (d.slices !== 1) throw new Error('cube maps are not handled');
  if (mip >= d.mipCount) throw new Error(`mip ${mip} of ${d.mipCount}`);
  const sizeOf = (w, h) => (BLOCK_BYTES[d.format] ? Math.ceil(w / 4) * Math.ceil(h / 4) * BLOCK_BYTES[d.format] : w * h * (d.format === 'PF_G8' ? 1 : 4));
  let off = d.dataOffset, w = d.width, h = d.height;
  for (let i = 0; i < mip; i++) { off += sizeOf(w, h); w = Math.max(1, w >> 1); h = Math.max(1, h >> 1); }
  const rgba = Buffer.alloc(w * h * 4);
  const stats = { modes: new Array(8).fill(0), shapes: { 2: new Array(64).fill(0), 3: new Array(64).fill(0) } };
  if (d.format === 'PF_B8G8R8A8') {
    for (let i = 0; i < w * h; i++) { rgba[i * 4] = buf[off + i * 4 + 2]; rgba[i * 4 + 1] = buf[off + i * 4 + 1]; rgba[i * 4 + 2] = buf[off + i * 4]; rgba[i * 4 + 3] = buf[off + i * 4 + 3]; }
  } else if (d.format === 'PF_G8') {
    for (let i = 0; i < w * h; i++) { rgba[i * 4] = rgba[i * 4 + 1] = rgba[i * 4 + 2] = buf[off + i]; rgba[i * 4 + 3] = 255; }
  } else if (BLOCK_BYTES[d.format]) {
    const bs = BLOCK_BYTES[d.format], bw = Math.ceil(w / 4), bh = Math.ceil(h / 4);
    const px = Buffer.alloc(64);
    for (let by = 0; by < bh; by++) for (let bx = 0; bx < bw; bx++) {
      const o = off + (by * bw + bx) * bs;
      if (d.format === 'PF_DXT1') bc1Block(buf, o, px, false);
      else if (d.format === 'PF_DXT5') { bc1Block(buf, o + 8, px, true); alphaBlock(buf, o, px, 3); }
      else if (d.format === 'PF_BC4') { alphaBlock(buf, o, px, 0); for (let i = 0; i < 16; i++) { px[i * 4 + 1] = px[i * 4 + 2] = px[i * 4]; px[i * 4 + 3] = 255; } }
      else if (d.format === 'PF_BC5') { alphaBlock(buf, o, px, 0); alphaBlock(buf, o + 8, px, 1); for (let i = 0; i < 16; i++) { px[i * 4 + 2] = 0; px[i * 4 + 3] = 255; } }
      else bc7Block(buf, o, px, stats);
      for (let y = 0; y < 4; y++) { const Y = by * 4 + y; if (Y >= h) break; for (let x = 0; x < 4; x++) { const X = bx * 4 + x; if (X >= w) break; px.copy(rgba, (Y * w + X) * 4, (y * 4 + x) * 4, (y * 4 + x) * 4 + 4); } }
    }
  } else throw new Error('pixel format not handled: ' + d.format);
  return { width: w, height: h, format: d.format, mipCount: d.mipCount, rgba, stats };
}

// ---------------------------------------------------------------- PNG
const PNG_SIG = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
let crcTable = null;
function crc32(buf) {
  if (zlib.crc32) return zlib.crc32(buf) >>> 0;
  if (!crcTable) { crcTable = new Uint32Array(256); for (let n = 0; n < 256; n++) { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; crcTable[n] = c >>> 0; } }
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = crcTable[(c ^ buf[i]) & 255] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}
function chunk(type, data) {
  const b = Buffer.alloc(12 + data.length);
  b.writeUInt32BE(data.length, 0); b.write(type, 4, 'latin1'); data.copy(b, 8);
  b.writeUInt32BE(crc32(b.subarray(4, 8 + data.length)), 8 + data.length);
  return b;
}
// sRGB chunk is written when opts.srgb is true (rendering intent 0); gAMA of 1.0 when opts.srgb === false.
function encodePng(width, height, rgba, opts = {}) {
  const alpha = opts.alpha !== false, ch = alpha ? 4 : 3;
  const raw = Buffer.alloc((width * ch + 1) * height);
  for (let y = 0; y < height; y++) {
    const ro = y * (width * ch + 1);
    raw[ro] = 0; // filter type None: deterministic and fast; deflate does the work
    if (alpha) rgba.copy(raw, ro + 1, y * width * 4, (y + 1) * width * 4);
    else for (let x = 0; x < width; x++) { const s = (y * width + x) * 4, t = ro + 1 + x * 3; raw[t] = rgba[s]; raw[t + 1] = rgba[s + 1]; raw[t + 2] = rgba[s + 2]; }
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0); ihdr.writeUInt32BE(height, 4); ihdr[8] = 8; ihdr[9] = alpha ? 6 : 2;
  const parts = [PNG_SIG, chunk('IHDR', ihdr)];
  if (opts.srgb === true) parts.push(chunk('sRGB', Buffer.from([0])));
  if (opts.srgb === false) { const g = Buffer.alloc(4); g.writeUInt32BE(100000, 0); parts.push(chunk('gAMA', g)); }
  parts.push(chunk('IDAT', zlib.deflateSync(raw, { level: 9 })), chunk('IEND', Buffer.alloc(0)));
  return Buffer.concat(parts);
}
function decodePng(buf) {
  if (!buf.subarray(0, 8).equals(PNG_SIG)) throw new Error('not a PNG');
  let o = 8, width = 0, height = 0, ch = 0; const idat = [];
  while (o < buf.length) {
    const n = buf.readUInt32BE(o), type = buf.toString('latin1', o + 4, o + 8), data = buf.subarray(o + 8, o + 8 + n);
    if (crc32(buf.subarray(o + 4, o + 8 + n)) !== buf.readUInt32BE(o + 8 + n)) throw new Error('PNG chunk CRC: ' + type);
    if (type === 'IHDR') { width = data.readUInt32BE(0); height = data.readUInt32BE(4); if (data[8] !== 8 || (data[9] !== 2 && data[9] !== 6) || data[12] !== 0) throw new Error('PNG variant not handled'); ch = data[9] === 6 ? 4 : 3; }
    else if (type === 'IDAT') idat.push(data);
    o += 12 + n;
  }
  const raw = zlib.inflateSync(Buffer.concat(idat));
  const stride = width * ch + 1, data = Buffer.alloc(width * height * ch);
  for (let y = 0; y < height; y++) { if (raw[y * stride] !== 0) throw new Error('PNG filter not handled'); raw.copy(data, y * width * ch, y * stride + 1, (y + 1) * stride); }
  return { width, height, channels: ch, data };
}

module.exports = { VERSION, decodeDds, encodePng, decodePng, selfCheck, crc32 };
