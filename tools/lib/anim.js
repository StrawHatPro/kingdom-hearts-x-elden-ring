// UE 4.17 (Square Enix fork, KINGDOM HEARTS III) cooked UAnimSequence export: parser, writer and
// key decoder. Format notes and the evidence for every field: docs/formats/animsequence.md.
// Proof: tools/anim_convert.js (audit/anim_manifest.json).
//
//   parseExport(pkg, uexp, exp)   -> { doc, start, end }   every byte of the export assigned to a field
//   serialize(pkg, doc)           -> Buffer                the export's exact bytes
//   decodeTrack(doc, track)       -> { translation, rotation, scale }   keys as floats + key frames
//
// The compressed byte stream is not kept as an opaque range: parseStream() splits it into blocks
// (one per track component), each block into header / bounds / keys / frame table / padding, and
// refuses any byte it cannot assign. writeStream() packs the blocks again from those fields.
const { Reader, Writer, nameOf, objectName } = require('./uasset');
const up = require('./uprops');

// AnimationKeyFormat / AnimationCompressionFormat (names: /Script/Engine enums in audit/reflection.json)
const KEY_FORMATS = ['AKF_ConstantKeyLerp', 'AKF_VariableKeyLerp', 'AKF_PerTrackCompression'];
const COMP_FORMATS = ['ACF_None', 'ACF_Float96NoW', 'ACF_Fixed48NoW', 'ACF_IntervalFixed32NoW', 'ACF_Fixed32NoW', 'ACF_Float32NoW', 'ACF_Identity'];
const ACF = { None: 0, Float96NoW: 1, Fixed48NoW: 2, IntervalFixed32NoW: 3, Fixed32NoW: 4, Float32NoW: 5, Identity: 6 };
const AKF = { ConstantKeyLerp: 0, VariableKeyLerp: 1, PerTrackCompression: 2 };

// Tables read from the executable (base/KH3.exe), used by the per-track decoders
// 0x143F15D00 (rotation), 0x143F1BF10 (translation), 0x143F19580 (scale):
//   0x1475E7918  u8[8][8]   components per key, index [format][flags & 7]
//   0x1475E7A38  i32[7]     bytes per component, index [format]
const PER_TRACK_NUM_COMPONENTS = [
  [4, 4, 4, 4, 4, 4, 4, 4], [3, 1, 1, 2, 1, 2, 2, 3], [3, 1, 1, 2, 1, 2, 2, 3], [6, 2, 2, 4, 2, 4, 4, 6],
  [1, 1, 1, 1, 1, 1, 1, 1], [1, 1, 1, 1, 1, 1, 1, 1], [0, 0, 0, 0, 0, 0, 0, 0], [0, 0, 0, 0, 0, 0, 0, 0],
];
const PER_TRACK_COMPONENT_BYTES = [4, 4, 2, 4, 4, 4, 0];
// The same decoders: bytes per key = (format == 3 ? 1 : components) * bytes per component;
// bytes of bounds in front of the keys = (format == 3 ? components : 0) * 4.
function perTrackSizes(format, flags) {
  if (format > 6) throw new Error(`per-track header: format ${format}`);
  const n = PER_TRACK_NUM_COMPONENTS[format][flags & 7];
  return { keyBytes: (format === 3 ? 1 : n) * PER_TRACK_COMPONENT_BYTES[format], fixedBytes: format === 3 ? n * 4 : 0 };
}

const PAD = 0x55;
const align4 = (n) => (n + 3) & ~3;
const KINDS = ['translation', 'rotation', 'scale'];

// ---------------------------------------------------------------- compressed byte stream

// Key-lerp codecs (AKF_ConstantKeyLerp / AKF_VariableKeyLerp): bytes per component and components
// per key, index [format]. Tables in the executable: translation 0x1475E79F8 / 0x1475E7A18,
// rotation 0x1475E7A38 / 0x1475E7A58, scale 0x1475E7A78 / 0x1475E7A98; used by the ByteSwap*In
// functions 0x143F10150 / 0x143F0F520 / 0x143F0FB30.
const LERP_TABLES = {
  translation: { stride: [4, 4, 4, 4, 4, 4, 0], num: [3, 3, 3, 1, 3, 3, 0], oneKey: 0 },
  rotation: { stride: [4, 4, 2, 4, 4, 4, 0], num: [4, 3, 3, 1, 1, 1, 0], oneKey: 1 },
  scale: { stride: [4, 4, 4, 4, 4, 4, 0], num: [3, 3, 3, 1, 3, 3, 0], oneKey: 0 },
};

// Splits the serialized stream into blocks, in the order the loader reads them: for each track
// its translation, rotation and (when the sequence has a scale table) scale block; a component
// whose offset is -1 has no block (per-track encoding only). The offset tables are NOT file
// positions: they say where the loader puts each block in memory (`memoryOffset`), and the two
// orders differ whenever scale blocks exist. Returns { blocks, padOverrides }; a block's
// bounds / keys / frames are views into `s`.
function parseStream(s, d, numFrames) {
  const keyFormat = d.keyEncodingFormat, per = keyFormat === AKF.PerTrackCompression;
  const nTracks = d.compressedTrackToSkeletonMapTable.length;
  const to = d.compressedTrackOffsets, so = d.compressedScaleOffsets;
  const hasScale = so.offsetData.length > 0;
  if (to.length !== nTracks * (per ? 2 : 4)) throw new Error(`${to.length} track offsets for ${nTracks} tracks`);
  if (hasScale && so.offsetData.length !== nTracks * so.stripSize) throw new Error('scale offsets are not StripSize per track');
  if (hasScale && so.stripSize !== (per ? 1 : 2)) throw new Error(`scale offset strip size ${so.stripSize}`);
  const frameBytes = numFrames > 0xff ? 2 : 1;
  const blocks = [];
  const padOverrides = [];
  let p = 0;
  // every pad byte is PAD; anything else is recorded so that the writer can reproduce it
  const pad = () => { const e = align4(p); if (e > s.length) throw new Error('block padding runs past the stream'); for (; p < e; p++) if (s[p] !== PAD) padOverrides.push([p, s[p]]); };
  let what = '';
  const take = (n, part) => { if (n < 0 || p + n > s.length) throw new Error(`${what}: ${part} runs past the stream`); const v = s.subarray(p, p + n); p += n; return v; };
  const block = (track, kind, memoryOffset, numKeys) => {
    what = `track ${track} ${kind}`;
    if (memoryOffset === -1 && per) return;
    if (memoryOffset < 0 || memoryOffset >= s.length || (memoryOffset & 3)) throw new Error(`${what}: offset ${memoryOffset} is not an aligned position inside the stream of ${s.length} bytes`);
    const b = { track, kind, memoryOffset, fileOffset: p };
    if (per) {
      const h = take(4, 'header').readUInt32LE(0);
      b.format = h >>> 28; b.flags = (h >>> 24) & 0xf; b.numKeys = h & 0xffffff;
      const { keyBytes, fixedBytes } = perTrackSizes(b.format, b.flags);
      b.keyBytes = keyBytes;
      b.bounds = take(fixedBytes, 'bounds');
      b.keys = take(keyBytes * b.numKeys, 'keys');
      pad();
      if (b.flags & 8) { b.frameBytes = frameBytes; b.frames = take(frameBytes * b.numKeys, 'frame table'); pad(); }
    } else {
      // the format is per sequence (one key: always the uncompressed format), the key count is in the offset table
      const T = LERP_TABLES[kind];
      if (!(numKeys >= 1)) throw new Error(`${what}: key count ${numKeys}`);
      b.numKeys = numKeys;
      b.format = numKeys === 1 ? T.oneKey : kind === 'rotation' ? d.rotationCompressionFormat : kind === 'translation' ? d.translationCompressionFormat : d.scaleCompressionFormat;
      b.flags = 7;
      b.keyBytes = T.stride[b.format] * T.num[b.format];
      b.bounds = take(b.format === ACF.IntervalFixed32NoW ? 24 : 0, 'bounds');
      b.keys = take(b.keyBytes * numKeys, 'keys');
      pad();
      if (keyFormat === AKF.VariableKeyLerp && numKeys > 1) { b.frameBytes = frameBytes; b.frames = take(frameBytes * numKeys, 'frame table'); pad(); }
    }
    b.size = p - b.fileOffset;
    blocks.push(b);
  };
  for (let t = 0; t < nTracks; t++) {
    if (per) {
      block(t, 'translation', to[t * 2]);
      block(t, 'rotation', to[t * 2 + 1]);
      if (hasScale) block(t, 'scale', so.offsetData[t]);
    } else {
      block(t, 'translation', to[t * 4], to[t * 4 + 1]);
      block(t, 'rotation', to[t * 4 + 2], to[t * 4 + 3]);
      if (hasScale) block(t, 'scale', so.offsetData[t * 2], so.offsetData[t * 2 + 1]);
    }
  }
  if (p !== s.length) throw new Error(`stream: blocks end at ${p} of ${s.length} bytes`);
  // in memory the blocks must not overlap and must fill the stream (the loader sizes its buffer to the file's byte count)
  const mem = blocks.slice().sort((x, y) => x.memoryOffset - y.memoryOffset);
  let m = 0;
  for (const b of mem) { if (b.memoryOffset !== m) throw new Error(`memory layout: track ${b.track} ${b.kind} at ${b.memoryOffset}, previous block ends at ${m}`); m += b.size; }
  return { blocks, padOverrides };
}

function writeStream(d, stream, length) {
  const out = Buffer.alloc(length, PAD);
  const per = d.keyEncodingFormat === AKF.PerTrackCompression;
  let p = 0;
  for (const b of stream.blocks) {
    if (per) { out.writeUInt32LE(((b.format << 28) | (b.flags << 24) | b.numKeys) >>> 0, p); p += 4; }
    p += b.bounds.copy(out, p);
    if (b.keys.length !== b.keyBytes * b.numKeys) throw new Error('key data has the wrong length');
    p += b.keys.copy(out, p);
    p = align4(p);
    if (b.frames) { if (b.frames.length !== b.frameBytes * b.numKeys) throw new Error('frame table has the wrong length'); p += b.frames.copy(out, p); p = align4(p); }
  }
  if (p !== length) throw new Error(`stream: wrote ${p} of ${length} bytes`);
  for (const [i, v] of stream.padOverrides) out[i] = v;
  return out;
}

// ---------------------------------------------------------------- export layout

const rdInts = (r, end, what) => {
  const n = r.i32();
  if (n < 0 || r.p + n * 4 > end) throw new Error(`${what}: bad array count ${n}`);
  const a = new Array(n);
  for (let i = 0; i < n; i++) a[i] = r.i32();
  return a;
};
const wrInts = (w, a) => { const b = Buffer.alloc(4 + a.length * 4); b.writeInt32LE(a.length, 0); a.forEach((v, i) => b.writeInt32LE(v, 4 + i * 4)); w.push(b); };

// CompressedCurveData is a tagged property list (FRawCurveTracks). tools/lib/uprops.js keeps a
// struct it has no layout for as { raw }; the one such struct in curve data is FSmartName, whose
// 8 bytes are an FName (the curve's DisplayName). curveValues() turns those into { smartName }
// (and back when writing) and counts whatever is still raw.
function curveValues(pkg, props, toFile) {
  let raw = 0;
  const walk = (list) => list.map(({ tag, value }) => {
    let v = value;
    if (tag.type === 'StructProperty' && tag.structName === 'SmartName' && tag.size === 8) {
      if (!toFile && v && v.raw !== undefined) {
        const b = Buffer.from(v.raw, 'hex');
        const f = { index: b.readInt32LE(0), number: b.readInt32LE(4) };
        if (f.index < 0 || f.index >= pkg.names.length || f.number < 0) throw new Error('curve data: SmartName is not a name table reference');
        v = { smartName: up.nameStr(pkg, f) };
      } else if (toFile && v && v.smartName !== undefined) {
        const f = up.nameIndex(pkg, v.smartName), b = Buffer.alloc(8);
        b.writeInt32LE(f.index, 0); b.writeInt32LE(f.number, 4);
        v = { raw: b.toString('hex') };
      }
    } else if (v && typeof v === 'object') {
      if (v.raw !== undefined) raw++;
      else if (v.props) v = { ...v, props: walk(v.props) };
      else if (v.innerTag && v.items) v = { ...v, items: v.items.map((it) => (it && it.props ? { ...it, props: walk(it.props) } : (it && it.raw !== undefined ? (raw++, it) : it))) };
    }
    return { tag, value: v };
  });
  const out = walk(props);
  if (raw) throw new Error(`curve data: ${raw} values are not interpreted`);
  return out;
}
// Number of values in a property list that uprops kept as raw bytes.
function countRaw(o) {
  if (Array.isArray(o)) return o.reduce((n, x) => n + countRaw(x), 0);
  if (!o || typeof o !== 'object') return 0;
  let n = o.raw !== undefined ? 1 : 0;
  for (const k of Object.keys(o)) if (k !== 'tag' && k !== 'innerTag') n += countRaw(o[k]);
  return n;
}

function propValue(props, name) { const p = props.find((x) => x.tag.name === name); return p ? p.value : undefined; }

function parseExport(pkg, uexp, exp) {
  const start = exp.serialOffset - pkg.summary.totalHeaderSize, end = start + exp.serialSize;
  if (start < 0 || end > uexp.length) throw new Error('export outside .uexp');
  const r = new Reader(uexp, start);
  const d = {};
  const need = (n, what) => { if (r.p + n > end) throw new Error(`${what}: runs past the export`); };
  const bool = (what) => { need(4, what); const v = r.i32(); if (v !== 0 && v !== 1) throw new Error(`${what}: not a bool (${v})`); return v; };
  d.props = up.readProps(r, pkg);
  need(4 + 16 + 2 + 4, 'header');
  d.objectGuidFlag = r.i32();
  if (d.objectGuidFlag !== 0) throw new Error('object guid present');
  d.skeletonGuid = r.hex(16);
  d.stripFlags = [r.u8(), r.u8()];
  if (!(d.stripFlags[0] & 1)) throw new Error('editor data is not stripped: raw animation data is not implemented');
  d.bSerializeCompressedData = bool('bSerializeCompressedData');
  if (d.bSerializeCompressedData) {
    need(4, 'formats');
    d.keyEncodingFormat = r.u8(); d.translationCompressionFormat = r.u8(); d.rotationCompressionFormat = r.u8(); d.scaleCompressionFormat = r.u8();
    if (d.keyEncodingFormat > 2) throw new Error(`key encoding format ${d.keyEncodingFormat}`);
    for (const k of ['translationCompressionFormat', 'rotationCompressionFormat', 'scaleCompressionFormat']) if (d[k] > 6) throw new Error(`${k} ${d[k]}`);
    d.compressedTrackOffsets = rdInts(r, end, 'CompressedTrackOffsets');
    d.compressedScaleOffsets = { offsetData: rdInts(r, end, 'CompressedScaleOffsets.OffsetData') };
    need(4, 'StripSize'); d.compressedScaleOffsets.stripSize = r.i32();
    d.compressedTrackToSkeletonMapTable = rdInts(r, end, 'CompressedTrackToSkeletonMapTable');
    d.compressedCurveData = curveValues(pkg, up.readProps(r, pkg), false);
    need(8, 'stream header');
    d.compressedRawDataSize = r.i32();
    const nBytes = r.i32();
    if (nBytes < 0 || r.p + nBytes > end) throw new Error('compressed byte stream runs past the export');
    const numFrames = propValue(d.props, 'NumFrames');
    if (!Number.isInteger(numFrames)) throw new Error('no NumFrames property');
    d.streamLength = nBytes;
    d.stream = parseStream(r.bytes(nBytes), d, numFrames);
    d.bUseRawDataOnly = bool('bUseRawDataOnly');
  }
  if (r.p !== end) throw new Error(r.p < end ? `stopped ${end - r.p} bytes before the end` : `ran ${r.p - end} bytes past the end`);
  return { doc: d, start, end };
}

function serialize(pkg, d) {
  const w = new Writer();
  up.writeProps(w, pkg, d.props);
  w.i32(d.objectGuidFlag).hex(d.skeletonGuid).push(Buffer.from(d.stripFlags));
  w.i32(d.bSerializeCompressedData);
  if (d.bSerializeCompressedData) {
    w.u8(d.keyEncodingFormat).u8(d.translationCompressionFormat).u8(d.rotationCompressionFormat).u8(d.scaleCompressionFormat);
    wrInts(w, d.compressedTrackOffsets);
    wrInts(w, d.compressedScaleOffsets.offsetData);
    w.i32(d.compressedScaleOffsets.stripSize);
    wrInts(w, d.compressedTrackToSkeletonMapTable);
    up.writeProps(w, pkg, curveValues(pkg, d.compressedCurveData, true));
    w.i32(d.compressedRawDataSize).i32(d.streamLength);
    w.push(writeStream(d, d.stream, d.streamLength));
    w.i32(d.bUseRawDataOnly);
  }
  return w.concat();
}

// ---------------------------------------------------------------- key decoding

const fr = Math.fround;
// Constants as the executable has them (float32): 1/32767, 1/1023, 1/511.
const Q16 = fr(3.051851e-05), Q16SQ = fr(9.313794e-10), Q11 = fr(0.0009775171056389809), Q10 = fr(0.001956947147846222);
const SCALE_MISSING = fr(0.00009999999747378752); // 0x38D1B717, scale decoder 0x143F19580

// Decodes one block to floats. Returns { values: Float32Array (3 per key; 4 for rotation), frames }.
// frames: Uint16Array of key frame numbers, or null when the keys are spread evenly over the sequence.
// Arithmetic follows the per-track decoders in the executable, in float32.
function decodeBlock(b, perTrack) {
  const n = b.numKeys, rot = b.kind === 'rotation';
  const k = b.keys, w = rot ? 4 : 3;
  const out = new Float32Array(n * w);
  const f = b.flags, fmt = b.format;
  const missing = b.kind === 'scale' && perTrack ? SCALE_MISSING : 0;
  const wOf = (x, y, z) => { const ww = fr(fr(fr(1 - fr(x * x)) - fr(y * y)) - fr(z * z)); return ww > 0 ? fr(Math.sqrt(ww)) : 0; };
  if (fmt === ACF.Identity) {
    if (rot) for (let i = 0; i < n; i++) out[i * 4 + 3] = 1; else out.fill(missing);
  } else if (fmt === ACF.None || fmt === ACF.Float96NoW) {
    // rotation and the key-lerp vectors: always three floats; per-track vectors: the flagged components (flags 0 = all three)
    const all = rot || !perTrack || (f & 7) === 0;
    for (let i = 0, p = 0; i < n; i++) {
      let x, y, z;
      if (all) { x = k.readFloatLE(p); y = k.readFloatLE(p + 4); z = k.readFloatLE(p + 8); p += b.keyBytes; }
      else {
        x = f & 1 ? k.readFloatLE(p) : missing; if (f & 1) p += 4;
        y = f & 2 ? k.readFloatLE(p) : missing; if (f & 2) p += 4;
        z = f & 4 ? k.readFloatLE(p) : missing; if (f & 4) p += 4;
      }
      out[i * w] = x; out[i * w + 1] = y; out[i * w + 2] = z;
      if (rot) out[i * 4 + 3] = wOf(x, y, z);
    }
  } else if (fmt === ACF.Fixed48NoW) {
    for (let i = 0, p = 0; i < n; i++) {
      const p0 = p;
      const u = [0, 0, 0], has = [f & 1, f & 2, f & 4];
      for (let c = 0; c < 3; c++) if (has[c]) { u[c] = k.readUInt16LE(p); p += 2; }
      p = p0 + b.keyBytes;
      if (rot) {
        const x = (has[0] ? u[0] : 32767) - 32767, y = (has[1] ? u[1] : 32767) - 32767, z = (has[2] ? u[2] : 32767) - 32767;
        const ww = fr(fr(fr(1 - fr(fr(x * x) * Q16SQ)) - fr(fr(y * y) * Q16SQ)) - fr(fr(z * z) * Q16SQ));
        out[i * 4] = fr(x * Q16); out[i * 4 + 1] = fr(y * Q16); out[i * 4 + 2] = fr(z * Q16); out[i * 4 + 3] = ww < 0 ? 0 : fr(Math.sqrt(ww));
      } else for (let c = 0; c < 3; c++) out[i * 3 + c] = has[c] ? u[c] - 255 : missing;
    }
  } else if (fmt === ACF.IntervalFixed32NoW) {
    const mins = [0, 0, 0], ranges = [0, 0, 0];
    for (let c = 0, p = 0; c < 3; c++) if (f & (1 << c)) { mins[c] = b.bounds.readFloatLE(p); ranges[c] = b.bounds.readFloatLE(p + 4); p += 8; }
    for (let i = 0; i < n; i++) {
      const v = k.readUInt32LE(i * 4);
      if (rot) {
        const x = fr(fr(fr(((v >>> 21) - 1023) * Q11) * ranges[0]) + mins[0]);
        const y = fr(fr(fr((((v >>> 10) & 0x7ff) - 1023) * Q11) * ranges[1]) + mins[1]);
        const z = fr(fr(fr(((v & 0x3ff) - 511) * Q10) * ranges[2]) + mins[2]);
        out[i * 4] = x; out[i * 4 + 1] = y; out[i * 4 + 2] = z; out[i * 4 + 3] = wOf(x, y, z);
      } else {
        out[i * 3] = fr(fr(fr(((v & 0x3ff) - 511) * Q10) * ranges[0]) + mins[0]);
        out[i * 3 + 1] = fr(fr(fr((((v >>> 10) & 0x7ff) - 1023) * Q11) * ranges[1]) + mins[1]);
        out[i * 3 + 2] = fr(fr(fr(((v >>> 21) - 1023) * Q11) * ranges[2]) + mins[2]);
      }
    }
  } else throw new Error(`decoding of ${COMP_FORMATS[fmt]} ${b.kind} keys is not implemented`);
  let frames = null;
  if (b.frames) { frames = new Uint16Array(n); for (let i = 0; i < n; i++) frames[i] = b.frameBytes === 2 ? b.frames.readUInt16LE(i * 2) : b.frames[i]; }
  return { values: out, frames, width: w };
}
// ---------------------------------------------------------------- related asset classes

// UAnimMontage, UBlendSpace, UBlendSpace1D, UAimOffsetBlendSpace, UAimOffsetBlendSpace1D: tagged
// properties, then the UAnimationAsset tail (object GUID flag, SkeletonGuid). Nothing else.
function parseAssetExport(pkg, uexp, exp) {
  const start = exp.serialOffset - pkg.summary.totalHeaderSize, end = start + exp.serialSize;
  if (start < 0 || end > uexp.length) throw new Error('export outside .uexp');
  const r = new Reader(uexp, start);
  const d = { props: up.readProps(r, pkg) };
  if (r.p + 20 > end) throw new Error('header runs past the export');
  d.objectGuidFlag = r.i32();
  if (d.objectGuidFlag !== 0) throw new Error('object guid present');
  d.skeletonGuid = r.hex(16);
  if (r.p !== end) throw new Error(r.p < end ? `stopped ${end - r.p} bytes before the end` : `ran ${r.p - end} bytes past the end`);
  return { doc: d, start, end };
}
function serializeAsset(pkg, d) {
  const w = new Writer();
  up.writeProps(w, pkg, d.props);
  return w.i32(d.objectGuidFlag).hex(d.skeletonGuid).concat();
}

// ---------------------------------------------------------------- skeleton

// USkeleton export: tagged properties, object GUID flag, then FReferenceSkeleton (bone info,
// reference pose, name -> index map; the same three arrays as in a USkeletalMesh, see
// docs/formats/skeletalmesh.md). Only those are read here; `guidIfNoRetargetSources` is the
// 16 bytes that follow an empty AnimRetargetSources map (the skeleton's Guid).
function parseSkeleton(pkg, uexp, exp) {
  const start = exp.serialOffset - pkg.summary.totalHeaderSize, end = start + exp.serialSize;
  const r = new Reader(uexp, start);
  up.readProps(r, pkg);
  if (r.i32() !== 0) throw new Error('skeleton: object guid present');
  const n = r.i32();
  if (n < 0 || r.p + n * 12 > end) throw new Error('skeleton: bad bone count');
  const bones = [];
  for (let i = 0; i < n; i++) bones.push({ name: nameOf(pkg, { index: r.i32(), number: r.i32() }), parent: r.i32() });
  if (r.i32() !== n || r.p + n * 40 > end) throw new Error('skeleton: reference pose count differs from the bone count');
  const pose = Buffer.from(r.bytes(n * 40));
  if (r.i32() !== n || r.p + n * 12 > end) throw new Error('skeleton: name map count differs from the bone count');
  for (let i = 0; i < n; i++) { const nm = nameOf(pkg, { index: r.i32(), number: r.i32() }); const ix = r.i32(); if (!bones[ix] || bones[ix].name !== nm) throw new Error('skeleton: name map entry does not name the bone at its index'); }
  let guid = null;
  if (r.p + 20 <= end && uexp.readInt32LE(r.p) === 0) guid = uexp.toString('hex', r.p + 4, r.p + 20);
  return { bones, pose, guidIfNoRetargetSources: guid };
}

// Object path of an import ("/Game/.../Pkg" + object name) or null.
function importPath(pkg, idx) {
  if (idx >= 0) return null;
  const imp = pkg.imports[-idx - 1];
  if (!imp) return null;
  let top = imp;
  while (top.outerIndex < 0) top = pkg.imports[-top.outerIndex - 1];
  return { packageName: nameOf(pkg, top.objectName), objectName: nameOf(pkg, imp.objectName), className: nameOf(pkg, imp.className) };
}
// "/Game/A/B" -> path of the package file under assets/ (without extension), or null.
function packageFile(packageName) {
  if (packageName.startsWith('/Game/')) return 'KINGDOM HEARTS III/Content/' + packageName.slice(6);
  if (packageName.startsWith('/Engine/')) return 'Engine/Content/' + packageName.slice(8);
  return null;
}

// ---------------------------------------------------------------- glTF

const SIDECAR_VERSION = 1;
const STREAM_VIEW_NAME = 'ue:compressedByteStream';

// Values the decoders in the executable produce when a component has no block (per-track encoding).
const NO_BLOCK = { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scaleNoTable: [1, 1, 1], scaleNoOffset: [0, 0, 0] };

// AnimSequence -> .glb + sidecar. `skel`: parseSkeleton() result; `mesh`: require('./mesh').
// Node 0 is the UE-to-glTF root (as in the mesh .glb files); nodes 1.. are the skeleton's bones
// with their reference pose, in bone order. One animation: for every track three channels.
function toGlb(mesh, pkg, d, skel, info = {}) {
  const GL = mesh.GL;
  const g = new mesh.GlbBuilder(`kh3-decomp ${info.converter || 'tools/lib/anim.js'} ${info.converter_version || ''}`.trim());
  const nb = skel.bones.length;
  const root = { name: 'UE_to_glTF', matrix: mesh.UE_TO_GLTF, children: [] };
  g.json.nodes.push(root);
  g.json.scenes[0].nodes.push(0);
  const pose = mesh.poseToNodes(skel.pose, nb);
  skel.bones.forEach((b, i) => {
    const n = pose.nodes[i];
    g.json.nodes.push({ name: b.name, translation: n.translation, rotation: n.rotation, scale: n.scale });
    if (b.parent >= 0 && b.parent < i) { const p = g.json.nodes[1 + b.parent]; (p.children = p.children || []).push(1 + i); } else root.children.push(1 + i);
  });
  if (!root.children.length) delete root.children;

  const numFrames = propValue(d.props, 'NumFrames');
  const seqLenRaw = propValue(d.props, 'SequenceLength');
  const seqLen = typeof seqLenRaw === 'number' ? seqLenRaw : 0; // absent property = class default 0
  const interp = propValue(d.props, 'Interpolation');
  const dis = d.props.find((x) => x.tag.name === 'm_DisableInterpolation');
  const step = interp === 'EAnimInterpolationType::Step' || !!(dis && dis.tag.boolVal);
  const per = d.keyEncodingFormat === AKF.PerTrackCompression;
  const hasScale = d.compressedScaleOffsets.offsetData.length > 0;
  const map = d.compressedTrackToSkeletonMapTable;

  const st = { tracks: map.length, blocks: 0, keys: 0, constantChannels: 0, maxQuatNormDeviation: 0, quatsClampedW: 0, quatKeys: 0, nonFiniteValues: 0, nonIncreasingTimes: 0, badBone: 0, duplicateBone: 0, keysOverNumFrames: 0, timedBlocks: 0 };
  // three growing pools: times (f32), vec3 outputs, vec4 outputs
  const pools = { time: [], v3: [], v4: [] }, sizes = { time: 0, v3: 0, v4: 0 };
  const pending = []; // accessors to create once the views exist
  const addAcc = (pool, f32, count, type, extra) => { const a = { pool, byteOffset: sizes[pool], count, type, ...extra }; pools[pool].push(Buffer.from(f32.buffer, f32.byteOffset, f32.byteLength)); sizes[pool] += f32.byteLength; return pending.push(a) - 1; };
  const timeCache = new Map(), constCache = new Map();
  const timeAcc = (b) => {
    const n = b.numKeys;
    const key = b.frames ? 'f' + b.frameBytes + ':' + b.frames.toString('latin1') : 'u' + n;
    let a = timeCache.get(key);
    if (a !== undefined) return a;
    const t = new Float32Array(n);
    if (b.frames) for (let i = 0; i < n; i++) { const f = b.frameBytes === 2 ? b.frames.readUInt16LE(i * 2) : b.frames[i]; t[i] = numFrames > 1 ? (f / (numFrames - 1)) * seqLen : 0; }
    else for (let i = 0; i < n; i++) t[i] = n > 1 ? (i / (n - 1)) * seqLen : 0;
    for (let i = 1; i < n; i++) if (!(t[i] > t[i - 1])) { st.nonIncreasingTimes++; break; }
    a = addAcc('time', t, n, 'SCALAR', { min: [t[0]], max: [t[n - 1]] });
    timeCache.set(key, a);
    return a;
  };
  const constAcc = (v) => {
    const key = v.join(',');
    let a = constCache.get(key);
    if (a === undefined) { a = addAcc(v.length === 4 ? 'v4' : 'v3', Float32Array.from(v), 1, v.length === 4 ? 'VEC4' : 'VEC3'); constCache.set(key, a); }
    return a;
  };
  const zeroTime = { numKeys: 1 };
  const samplers = [], channels = [];
  const seenBone = new Set();
  const blockOf = new Map();
  for (const b of d.stream.blocks) blockOf.set(b.track * 3 + KINDS.indexOf(b.kind), b);
  for (let t = 0; t < map.length; t++) {
    const bone = map[t];
    if (!(bone >= 0 && bone < nb)) { st.badBone++; continue; }
    if (seenBone.has(bone)) { st.duplicateBone++; continue; }
    seenBone.add(bone);
    for (let k = 0; k < 3; k++) {
      const kind = KINDS[k];
      const b = blockOf.get(t * 3 + k);
      let input, output;
      if (!b) {
        st.constantChannels++;
        input = timeAcc(zeroTime);
        output = constAcc(kind === 'scale' ? (hasScale ? NO_BLOCK.scaleNoOffset : NO_BLOCK.scaleNoTable) : NO_BLOCK[kind]);
      } else {
        const dec = decodeBlock(b, per);
        const v = dec.values;
        st.blocks++; st.keys += b.numKeys;
        if (b.frames) st.timedBlocks++;
        if (b.numKeys > numFrames) st.keysOverNumFrames++;
        for (let i = 0; i < v.length; i++) if (!Number.isFinite(v[i])) st.nonFiniteValues++;
        if (kind === 'rotation') {
          st.quatKeys += b.numKeys;
          for (let i = 0; i < v.length; i += 4) {
            const l2 = v[i] * v[i] + v[i + 1] * v[i + 1] + v[i + 2] * v[i + 2] + v[i + 3] * v[i + 3];
            const dev = Math.abs(l2 - 1);
            if (dev > st.maxQuatNormDeviation) st.maxQuatNormDeviation = dev;
            if (v[i + 3] === 0 && l2 > 1) st.quatsClampedW++;
            const l = Math.sqrt(l2);
            if (l > 0 && Number.isFinite(l)) { v[i] /= l; v[i + 1] /= l; v[i + 2] /= l; v[i + 3] /= l; } else { v[i] = v[i + 1] = v[i + 2] = 0; v[i + 3] = 1; }
          }
        }
        input = timeAcc(b);
        output = addAcc(kind === 'rotation' ? 'v4' : 'v3', v, b.numKeys, kind === 'rotation' ? 'VEC4' : 'VEC3');
      }
      samplers.push({ input, output, interpolation: step ? 'STEP' : 'LINEAR' });
      channels.push({ sampler: samplers.length - 1, target: { node: 1 + bone, path: kind } });
    }
  }
  const views = {};
  for (const pool of ['time', 'v3', 'v4']) if (sizes[pool]) views[pool] = g.view(Buffer.concat(pools[pool], sizes[pool]));
  const accIndex = pending.map((a) => g.accessor(views[a.pool], GL.FLOAT, a.type, a.count, { ...(a.byteOffset ? { byteOffset: a.byteOffset } : {}), ...(a.min ? { min: a.min, max: a.max } : {}) }));
  if (channels.length) {
    for (const s of samplers) { s.input = accIndex[s.input]; s.output = accIndex[s.output]; }
    g.json.animations = [{ name: info.export || 'AnimSequence', samplers, channels }];
  }
  // glTF has no empty bufferView: a sequence whose stream is empty (no component has a block) gets none
  const streamView = d.streamLength ? g.view(writeStream(d, d.stream, d.streamLength), { name: STREAM_VIEW_NAME }) : null;
  g.json.extras = { ue_package: info.package, ue_export: info.export, ue_class: 'AnimSequence', ue_skeleton: info.skeleton, ue_num_frames: numFrames, ue_sequence_length: seqLen };
  st.channels = channels.length;
  st.poseOverrides = Object.keys(pose.overrides).length;
  const data = { ...d };
  delete data.stream;
  const sidecar = { sidecar_version: SIDECAR_VERSION, ...info, class: 'AnimSequence', gltf: { stream_view: streamView, first_bone_node: 1, bones: nb, interpolation: step ? 'STEP' : 'LINEAR' }, data };
  return { glb: g.finish(), sidecar, stats: st };
}

// Reverse direction: the export's exact bytes from the sidecar, the .glb and the package name
// table. The stream comes from the .glb's raw view and goes through parseStream()/writeStream().
function rebuild(mesh, pkg, sidecar, glbBytes) {
  if (sidecar.sidecar_version !== SIDECAR_VERSION || sidecar.class !== 'AnimSequence') throw new Error('unknown sidecar');
  const g = mesh.readGlb(glbBytes);
  const d = { ...sidecar.data };
  let s = Buffer.alloc(0);
  if (sidecar.gltf.stream_view !== null) {
    const v = g.json.bufferViews[sidecar.gltf.stream_view];
    if (!v || v.name !== STREAM_VIEW_NAME) throw new Error('the .glb has no compressed stream view');
    s = g.viewBytes(sidecar.gltf.stream_view);
  }
  if (s.length !== d.streamLength) throw new Error('stream view has the wrong length');
  d.stream = parseStream(s, d, propValue(d.props, 'NumFrames'));
  return serialize(pkg, d);
}

module.exports = {
  KEY_FORMATS, COMP_FORMATS, ACF, AKF, KINDS, PAD, PER_TRACK_NUM_COMPONENTS, PER_TRACK_COMPONENT_BYTES, LERP_TABLES, SCALE_MISSING, NO_BLOCK,
  SIDECAR_VERSION, STREAM_VIEW_NAME,
  perTrackSizes, parseStream, writeStream, parseExport, serialize, decodeBlock, propValue, curveValues, countRaw,
  parseAssetExport, serializeAsset, parseSkeleton, importPath, packageFile, toGlb, rebuild, objectName,
};
