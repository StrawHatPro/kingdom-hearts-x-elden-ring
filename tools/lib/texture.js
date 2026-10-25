// UE 4.17 cooked UTexture2D export <-> DDS + sidecar JSON, lossless in both directions.
// Format notes: docs/formats/texture.md. Proof: tools/texture_convert.js (audit/texture_manifest.json).
//
//   parseExport(pkg, uexp, exp)           -> { tex, end }   structured export, pixel data referenced
//   toDds(tex, platformIndex, mipData)    -> Buffer         standard .dds, original blocks untouched
//   sidecarOf(tex)                        -> plain object   every non-pixel field (JSON-safe)
//   rebuild(pkg, sidecar, dds)            -> { exportBytes, bulk: [{offset, data}] }
//
// rebuild() uses only the sidecar, the .dds bytes and the package name table (for FNames).
const { Reader, Writer } = require('./uasset');
const up = require('./uprops');

// Bulk data flags (names from the public UE 4.17 source; behaviour is what this reader verifies).
const BULK_PAYLOAD_AT_END_OF_FILE = 0x0001;
const BULK_FORCE_INLINE_PAYLOAD = 0x0040;
const BULK_PAYLOAD_IN_SEPARATE_FILE = 0x0100;

const fourcc = (s) => Buffer.from(s, 'latin1').readUInt32LE(0);
// EPixelFormat name -> how the same bytes are described in a DDS header.
//   block: bytes per 4x4 block (block-compressed)   bpp: bytes per pixel (uncompressed)
const FORMATS = {
  PF_DXT1: { block: 8, fourcc: 'DXT1' },
  PF_DXT3: { block: 16, fourcc: 'DXT3' },
  PF_DXT5: { block: 16, fourcc: 'DXT5' },
  PF_BC4: { block: 8, fourcc: 'ATI1' },
  PF_BC5: { block: 16, fourcc: 'ATI2' },
  PF_BC6H: { block: 16, dxgi: 95 }, // DXGI_FORMAT_BC6H_UF16
  PF_BC7: { block: 16, dxgi: 98 }, // DXGI_FORMAT_BC7_UNORM
  PF_B8G8R8A8: { bpp: 4, masks: [32, 0x00ff0000, 0x0000ff00, 0x000000ff, 0xff000000], pfFlags: 0x41 }, // DDPF_RGB | DDPF_ALPHAPIXELS
  PF_R8G8B8A8: { bpp: 4, masks: [32, 0x000000ff, 0x0000ff00, 0x00ff0000, 0xff000000], pfFlags: 0x41 },
  PF_G8: { bpp: 1, masks: [8, 0xff, 0, 0, 0], pfFlags: 0x20000 }, // DDPF_LUMINANCE
  PF_A8: { bpp: 1, masks: [8, 0, 0, 0, 0xff], pfFlags: 0x2 }, // DDPF_ALPHA
  PF_G16: { bpp: 2, masks: [16, 0xffff, 0, 0, 0], pfFlags: 0x20000 },
  PF_V8U8: { bpp: 2, masks: [16, 0x00ff, 0xff00, 0, 0], pfFlags: 0x80000 }, // DDPF_BUMPDUDV
  PF_FloatRGBA: { bpp: 8, fourccNum: 113 }, // D3DFMT_A16B16G16R16F
  PF_A32B32G32R32F: { bpp: 16, fourccNum: 116 }, // D3DFMT_A32B32G32R32F
  PF_R32_FLOAT: { bpp: 4, fourccNum: 114 }, // D3DFMT_R32F
  PF_R16F: { bpp: 2, fourccNum: 111 }, // D3DFMT_R16F
  PF_G16R16F: { bpp: 4, fourccNum: 112 }, // D3DFMT_G16R16F
  PF_G32R32F: { bpp: 8, fourccNum: 115 }, // D3DFMT_G32R32F
};

function mipBytes(fmt, w, h) {
  const f = FORMATS[fmt];
  if (!f) return null;
  if (f.block) return Math.max(1, (w + 3) >> 2) * Math.max(1, (h + 3) >> 2) * f.block;
  return w * h * f.bpp;
}

// ---------------------------------------------------------------- export parsing

// One FByteBulkData header + FTexture2DMipMap fields. `abs` converts a .uexp position to the
// offset in the virtual file "<.uasset><.uexp>" that inline bulk offsets are expressed in.
function readMip(r, ctx) {
  const m = { cooked: r.i32(), flags: r.u32(), elementCount: r.i32(), sizeOnDisk: r.i32(), offset: r.i64() };
  if (m.flags & BULK_PAYLOAD_IN_SEPARATE_FILE) {
    m.storage = 'ubulk';
    m.dataOffset = m.offset + ctx.bulkDataStartOffset; // position in the .ubulk
  } else if (m.flags & BULK_FORCE_INLINE_PAYLOAD || !(m.flags & BULK_PAYLOAD_AT_END_OF_FILE)) {
    m.storage = 'inline';
    m.dataOffset = r.p; // position in the .uexp
    m.inlineOffsetRuleOk = m.offset === r.p + ctx.totalHeaderSize;
    r.p += m.sizeOnDisk;
  } else {
    throw new Error('unhandled bulk data flags 0x' + m.flags.toString(16));
  }
  if (m.sizeOnDisk !== m.elementCount) throw new Error(`bulk size on disk ${m.sizeOnDisk} != element count ${m.elementCount} (flags 0x${m.flags.toString(16)})`);
  m.sizeX = r.i32();
  m.sizeY = r.i32();
  return m;
}

// exp: export table row. ctx: { totalHeaderSize, bulkDataStartOffset }.
// The export must be a UTexture2D (or a subclass that only appends data; that is kept in `trailing`).
function parseExport(pkg, uexp, exp) {
  const ctx = { totalHeaderSize: pkg.summary.totalHeaderSize, bulkDataStartOffset: pkg.summary.bulkDataStartOffset };
  const start = exp.serialOffset - ctx.totalHeaderSize, end = start + exp.serialSize;
  if (start < 0 || end > uexp.length) throw new Error('export outside .uexp');
  const r = new Reader(uexp, start);
  const tex = { props: up.readProps(r, pkg) };
  const hasGuid = r.i32();
  if (hasGuid !== 0 && hasGuid !== 1) throw new Error('bad object guid flag ' + hasGuid);
  tex.objectGuid = hasGuid ? r.hex(16) : null;
  tex.stripFlagsTexture = [r.u8(), r.u8()];
  tex.stripFlagsTexture2D = [r.u8(), r.u8()];
  tex.cooked = r.i32();
  if (tex.cooked !== 1) throw new Error('bCooked = ' + tex.cooked);
  tex.platforms = [];
  for (;;) {
    const name = up.nameStr(pkg, { index: r.i32(), number: r.i32() });
    if (name === 'None') break;
    const p = { pixelFormatName: name, skipOffset: r.i32() };
    p.sizeX = r.i32(); p.sizeY = r.i32(); p.numSlices = r.i32();
    p.pixelFormat = r.fstring();
    p.firstMip = r.i32();
    const n = r.i32();
    if (n < 0 || n > 32) throw new Error('bad mip count ' + n);
    p.mips = [];
    for (let i = 0; i < n; i++) p.mips.push(readMip(r, ctx));
    p.skipOffsetRuleOk = p.skipOffset === r.p + ctx.totalHeaderSize;
    tex.platforms.push(p);
    if (r.p > end) throw new Error('platform data runs past the export');
  }
  if (r.p > end) throw new Error('parse ran past the export');
  tex.trailing = uexp.subarray(r.p, end).toString('hex'); // subclass data (e.g. ULightMapTexture2D flags)
  return { tex, start, end };
}

// Pixel bytes of one mip: a view into the .uexp or the .ubulk (never copied or modified).
function mipData(m, uexp, ubulk) {
  const src = m.storage === 'ubulk' ? ubulk : uexp;
  if (!src) throw new Error('mip refers to a .ubulk that does not exist');
  if (m.dataOffset < 0 || m.dataOffset + m.sizeOnDisk > src.length) throw new Error(`mip data outside the .${m.storage === 'ubulk' ? 'ubulk' : 'uexp'}`);
  return src.subarray(m.dataOffset, m.dataOffset + m.sizeOnDisk);
}

// ---------------------------------------------------------------- DDS

const DDS_MAGIC = 0x20534444; // "DDS "

// mips: [{ sizeX, sizeY, data }] largest first, as stored. Throws when the stored byte counts are
// not what a DDS reader would compute, since such a file would not be a valid DDS.
function writeDds(format, mips, slices = 1) {
  if (slices !== 1 && slices !== 6) throw new Error('numSlices = ' + slices);
  const f = FORMATS[format];
  if (!f) throw new Error('unsupported pixel format ' + format);
  if (!mips.length) throw new Error('no mips');
  // A DDS reader derives each mip's size from the top mip by halving, so that is what is checked.
  // (The sizeX/sizeY fields stored per mip can differ: block-compressed tails are sometimes
  // recorded as 4x4 instead of 2x2 and 1x1. Those fields live in the sidecar.)
  for (let i = 0; i < mips.length; i++) {
    const w = Math.max(1, mips[0].sizeX >> i), h = Math.max(1, mips[0].sizeY >> i);
    const want = mipBytes(format, w, h) * slices;
    if (mips[i].data.length !== want) throw new Error(`mip ${i} (${w}x${h} ${format}): ${mips[i].data.length} bytes stored, DDS expects ${want}`);
  }
  const dx10 = f.dxgi !== undefined;
  const h = Buffer.alloc(4 + 124 + (dx10 ? 20 : 0));
  const w = mips[0].sizeX, ht = mips[0].sizeY;
  h.writeUInt32LE(DDS_MAGIC, 0);
  h.writeUInt32LE(124, 4);
  // CAPS | HEIGHT | WIDTH | PIXELFORMAT, + MIPMAPCOUNT, + LINEARSIZE (compressed) or PITCH
  h.writeUInt32LE(0x1007 | (mips.length > 1 ? 0x20000 : 0) | (f.block ? 0x80000 : 0x8), 8);
  h.writeUInt32LE(ht, 12);
  h.writeUInt32LE(w, 16);
  h.writeUInt32LE(f.block ? mips[0].data.length / slices : w * f.bpp, 20);
  h.writeUInt32LE(0, 24); // depth
  h.writeUInt32LE(mips.length, 28);
  // 11 reserved dwords at 32..75 stay zero
  h.writeUInt32LE(32, 76); // DDS_PIXELFORMAT.dwSize
  if (dx10) { h.writeUInt32LE(0x4, 80); h.writeUInt32LE(fourcc('DX10'), 84); }
  else if (f.fourcc) { h.writeUInt32LE(0x4, 80); h.writeUInt32LE(fourcc(f.fourcc), 84); }
  else if (f.fourccNum) { h.writeUInt32LE(0x4, 80); h.writeUInt32LE(f.fourccNum, 84); }
  else {
    h.writeUInt32LE(f.pfFlags, 80);
    h.writeUInt32LE(f.masks[0], 88);
    for (let i = 1; i < 5; i++) h.writeUInt32LE(f.masks[i] >>> 0, 88 + i * 4);
  }
  h.writeUInt32LE(0x1000 | (mips.length > 1 ? 0x400008 : 0) | (slices === 6 ? 0x8 : 0), 108); // TEXTURE (+ MIPMAP | COMPLEX)
  if (slices === 6) h.writeUInt32LE(0xfe00, 112); // DDSCAPS2_CUBEMAP + all six faces
  if (dx10) {
    h.writeUInt32LE(f.dxgi, 128);
    h.writeUInt32LE(3, 132); // D3D10_RESOURCE_DIMENSION_TEXTURE2D
    h.writeUInt32LE(slices === 6 ? 0x4 : 0, 136); // DDS_RESOURCE_MISC_TEXTURECUBE
    h.writeUInt32LE(1, 140); // array size
    h.writeUInt32LE(0, 144);
  }
  if (slices === 1) return Buffer.concat([h, ...mips.map((m) => m.data)]);
  // Cube: the export stores mip-major data (each mip holds its six faces back to back); a DDS is
  // face-major (each face followed by its own mip chain). Same bytes, reordered.
  const parts = [h];
  for (let face = 0; face < 6; face++) for (const m of mips) { const n = m.data.length / 6; parts.push(m.data.subarray(face * n, (face + 1) * n)); }
  return Buffer.concat(parts);
}

// Header fields needed to locate the pixel data; format identification is cross-checked by rebuild().
function readDds(buf) {
  if (buf.length < 128 || buf.readUInt32LE(0) !== DDS_MAGIC || buf.readUInt32LE(4) !== 124) throw new Error('not a DDS file');
  const d = { height: buf.readUInt32LE(12), width: buf.readUInt32LE(16), mipCount: buf.readUInt32LE(28) || 1, dataOffset: 128, slices: buf.readUInt32LE(112) & 0x200 ? 6 : 1 };
  const pfFlags = buf.readUInt32LE(80), cc = buf.readUInt32LE(84);
  if (pfFlags & 0x4 && cc === fourcc('DX10')) {
    d.dataOffset = 148;
    d.dxgi = buf.readUInt32LE(128);
    d.format = Object.keys(FORMATS).find((k) => FORMATS[k].dxgi === d.dxgi);
  } else if (pfFlags & 0x4) {
    d.format = Object.keys(FORMATS).find((k) => (FORMATS[k].fourcc && fourcc(FORMATS[k].fourcc) === cc) || FORMATS[k].fourccNum === cc);
  } else {
    const masks = [0, 1, 2, 3, 4].map((i) => buf.readUInt32LE(88 + i * 4));
    d.format = Object.keys(FORMATS).find((k) => FORMATS[k].masks && FORMATS[k].pfFlags === pfFlags && FORMATS[k].masks.every((m, i) => (m >>> 0) === masks[i]));
  }
  if (!d.format) throw new Error('DDS pixel format not recognised');
  return d;
}

// ---------------------------------------------------------------- sidecar / conversion

const SIDECAR_VERSION = 1;

// Everything in the export that is not pixel data. `dataOffset`, `storage` and the *RuleOk flags
// are derived values and are not stored.
function sidecarOf(tex, info = {}) {
  return {
    sidecar_version: SIDECAR_VERSION,
    ...info,
    props: tex.props,
    objectGuid: tex.objectGuid,
    stripFlagsTexture: tex.stripFlagsTexture,
    stripFlagsTexture2D: tex.stripFlagsTexture2D,
    cooked: tex.cooked,
    platforms: tex.platforms.map((p) => ({
      pixelFormatName: p.pixelFormatName, skipOffset: p.skipOffset, sizeX: p.sizeX, sizeY: p.sizeY, numSlices: p.numSlices,
      pixelFormat: p.pixelFormat, firstMip: p.firstMip,
      mips: p.mips.map((m) => ({ cooked: m.cooked, flags: m.flags, elementCount: m.elementCount, sizeOnDisk: m.sizeOnDisk, offset: m.offset, sizeX: m.sizeX, sizeY: m.sizeY })),
    })),
    trailing: tex.trailing,
  };
}

// DDS for one platform entry (there is one per texture in this game), all stored mips.
function toDds(tex, uexp, ubulk, platformIndex = 0) {
  const p = tex.platforms[platformIndex];
  if (!p) throw new Error('texture has no platform data');
  return writeDds(p.pixelFormat.s, p.mips.map((m) => ({ sizeX: m.sizeX, sizeY: m.sizeY, data: mipData(m, uexp, ubulk) })), p.numSlices);
}

// Reverse direction. Returns the export's exact bytes and the pieces it owns in the .ubulk.
// `bulkDataStartOffset` comes from the package summary (the .uasset), like the name table.
function rebuild(pkg, sidecar, dds) {
  if (sidecar.sidecar_version !== SIDECAR_VERSION) throw new Error('unknown sidecar version');
  if (sidecar.platforms.length !== 1) throw new Error('expected exactly one platform entry');
  const d = readDds(dds);
  const p0 = sidecar.platforms[0];
  if (d.format !== p0.pixelFormat.s) throw new Error(`DDS is ${d.format}, sidecar says ${p0.pixelFormat.s}`);
  if (d.mipCount !== p0.mips.length || d.width !== p0.mips[0].sizeX || d.height !== p0.mips[0].sizeY) throw new Error('DDS dimensions or mip count differ from the sidecar');
  if (d.slices !== p0.numSlices) throw new Error('DDS cube flag differs from the sidecar');
  // Pixel bytes per mip, undoing the face-major order of a cube DDS.
  const total = p0.mips.reduce((s, m) => s + m.sizeOnDisk, 0);
  if (d.dataOffset + total !== dds.length) throw new Error('DDS pixel data length differs from the mips listed in the sidecar');
  const mipBufs = [];
  if (d.slices === 1) { let o = d.dataOffset; for (const m of p0.mips) { mipBufs.push(dds.subarray(o, o + m.sizeOnDisk)); o += m.sizeOnDisk; } }
  else {
    const faces = p0.mips.map(() => []);
    let o = d.dataOffset;
    for (let face = 0; face < 6; face++) p0.mips.forEach((m, i) => { const n = m.sizeOnDisk / 6; faces[i].push(dds.subarray(o, o + n)); o += n; });
    for (const f of faces) mipBufs.push(Buffer.concat(f));
  }
  const w = new Writer();
  const bulk = [];
  up.writeProps(w, pkg, sidecar.props);
  w.i32(sidecar.objectGuid ? 1 : 0);
  if (sidecar.objectGuid) w.hex(sidecar.objectGuid);
  w.push(Buffer.from(sidecar.stripFlagsTexture)).push(Buffer.from(sidecar.stripFlagsTexture2D));
  w.i32(sidecar.cooked);
  const name = (s) => { const f = up.nameIndex(pkg, s); w.i32(f.index).i32(f.number); };
  for (const p of sidecar.platforms) {
    name(p.pixelFormatName);
    w.i32(p.skipOffset).i32(p.sizeX).i32(p.sizeY).i32(p.numSlices).fstring(p.pixelFormat).i32(p.firstMip).i32(p.mips.length);
    for (let i = 0; i < p.mips.length; i++) {
      const m = p.mips[i], data = mipBufs[i];
      w.i32(m.cooked).u32(m.flags).i32(m.elementCount).i32(m.sizeOnDisk).i64(m.offset);
      if (m.flags & BULK_PAYLOAD_IN_SEPARATE_FILE) bulk.push({ offset: m.offset + pkg.summary.bulkDataStartOffset, data });
      else w.push(data);
      w.i32(m.sizeX).i32(m.sizeY);
    }
  }
  name('None');
  w.hex(sidecar.trailing);
  return { exportBytes: w.concat(), bulk };
}

module.exports = {
  FORMATS, SIDECAR_VERSION, BULK_PAYLOAD_AT_END_OF_FILE, BULK_FORCE_INLINE_PAYLOAD, BULK_PAYLOAD_IN_SEPARATE_FILE,
  mipBytes, parseExport, mipData, writeDds, readDds, sidecarOf, toDds, rebuild,
};
