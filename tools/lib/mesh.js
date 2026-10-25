// UE 4.17 cooked UStaticMesh / USkeletalMesh export <-> glTF 2.0 binary (.glb) + sidecar JSON,
// lossless in both directions. Format notes: docs/formats/staticmesh.md, skeletalmesh.md.
// Proof: tools/mesh_convert.js (audit/mesh_manifest.json).
//
//   parseExport(pkg, uexp, exp, cls)   -> { doc, blobs }      structured export + raw byte ranges
//   toGlb(pkg, cls, doc, blobs, info)  -> { glb, sidecar }    .glb bytes and the sidecar object
//   rebuild(pkg, sidecar, glb)         -> Buffer              the export's exact bytes
//
// The layout of each class is written once, as a function over an archive (`Ar`) that either reads
// or writes, the way the engine's own Serialize() functions work; the reader and the writer can
// therefore not drift apart. Large arrays are "blobs": byte ranges that the archive hands over by
// name. toGlb() puts every blob into the .glb (decoded where glTF has a place for it), and
// rebuild() recovers every blob from the .glb alone, guided by `sidecar.gltf.blobs`.
//
// What rebuild() takes from where:
//   positions            POSITION accessor (float32, bytes unchanged)
//   packed tangent basis _UE_TANGENT_X / _UE_TANGENT_Z accessors: the original packed integers.
//                        NORMAL and TANGENT (float, normalised) are derived for viewers and are
//                        NOT used by rebuild(): normalising does not re-encode bit-exactly.
//   UVs                  TEXCOORD_n accessors (float32). Half floats widen to float32 exactly and
//                        narrow back exactly; both directions are bit operations (NaN payloads kept).
//   vertex colours       COLOR_0 (RGBA bytes; the export stores BGRA)
//   indices              the index bufferView (triangle winding restored when it was swapped)
//   skin weights         JOINTS_n / WEIGHTS_n, mapped back through each section's bone map
//                        (or a raw bufferView when that mapping is not invertible for a LOD)
//   reference pose       the joint nodes' translation/rotation/scale (JSON numbers), plus per-bone
//                        overrides in the sidecar where JSON cannot carry the bits (-0, non-unit)
//   everything else      sidecar JSON, or a named raw bufferView for large opaque arrays
const { Reader, Writer, nameOf } = require('./uasset');
const up = require('./uprops');

const SIDECAR_VERSION = 1;

// ---------------------------------------------------------------- archive

const f32buf = Buffer.alloc(4);
const bitsOf = (f) => { f32buf.writeFloatLE(f); return f32buf.readUInt32LE(0); };
const fromBits = (u) => { f32buf.writeUInt32LE(u >>> 0); return f32buf.readFloatLE(0); };
// JSON cannot hold NaN, infinities or -0: those are kept as { f32bits }.
const floatJson = (bits) => { const f = fromBits(bits); return Number.isFinite(f) && !Object.is(f, -0) ? f : { f32bits: bits >>> 0 }; };
const floatBits = (v) => (typeof v === 'object' ? v.f32bits >>> 0 : bitsOf(v));
const floatVal = (v) => (typeof v === 'object' ? fromBits(v.f32bits) : v);

class Ar {
  // read:  new Ar({ pkg, r: Reader, end, blobs: {} })      write: new Ar({ pkg, w: Writer, blobs })
  constructor(o) { Object.assign(this, o); this.reading = !!o.r; }
  need(n, what) { if (n < 0 || this.r.p + n > this.end) throw new Error(`${what}: runs past the export`); }
  i32(o, k) { if (this.reading) { this.need(4, k); o[k] = this.r.i32(); } else this.w.i32(o[k]); return o[k]; }
  u32(o, k) { if (this.reading) { this.need(4, k); o[k] = this.r.u32(); } else this.w.u32(o[k]); return o[k]; }
  u16(o, k) { if (this.reading) { this.need(2, k); o[k] = this.r.u16(); } else this.w.u16(o[k]); return o[k]; }
  i16(o, k) { if (this.reading) { this.need(2, k); o[k] = (this.r.u16() << 16) >> 16; } else this.w.u16(o[k] & 0xffff); return o[k]; }
  u8(o, k) { if (this.reading) { this.need(1, k); o[k] = this.r.u8(); } else this.w.u8(o[k]); return o[k]; }
  bool(o, k) { const v = this.i32(o, k); if (v !== 0 && v !== 1) throw new Error(`${k}: not a bool (${v})`); return v; }
  f32(o, k) { if (this.reading) { this.need(4, k); o[k] = floatJson(this.r.u32()); } else this.w.u32(floatBits(o[k])); return o[k]; }
  floats(o, k, n) {
    if (this.reading) { this.need(4 * n, k); o[k] = []; for (let i = 0; i < n; i++) o[k].push(floatJson(this.r.u32())); } else for (const v of o[k]) this.w.u32(floatBits(v));
    return o[k];
  }
  hex(o, k, n) { if (this.reading) { this.need(n, k); o[k] = this.r.hex(n); } else this.w.hex(o[k]); return o[k]; }
  bytes2(o, k) { if (this.reading) { this.need(2, k); o[k] = [this.r.u8(), this.r.u8()]; } else this.w.push(Buffer.from(o[k])); return o[k]; }
  name(o, k) {
    if (this.reading) { this.need(8, k); o[k] = up.nameStr(this.pkg, { index: this.r.i32(), number: this.r.i32() }); } else { const f = up.nameIndex(this.pkg, o[k]); this.w.i32(f.index).i32(f.number); }
    return o[k];
  }
  props(o, k) { if (this.reading) o[k] = up.readProps(this.r, this.pkg); else up.writeProps(this.w, this.pkg, o[k]); return o[k]; }
  // TArray of structures: i32 count, then fn(item, index) per element.
  array(o, k, fn, max = 1 << 24) {
    if (this.reading) {
      this.need(4, k);
      const n = this.r.i32();
      if (n < 0 || n > max) throw new Error(`${k}: count ${n} out of range`);
      o[k] = [];
      for (let i = 0; i < n; i++) { const it = {}; o[k].push(it); fn(it, i); }
    } else { this.w.i32(o[k].length); o[k].forEach(fn); }
    return o[k];
  }
  // TArray of plain numbers, kept as a JSON array (for short arrays).
  nums(o, k, type) {
    const size = { i32: 4, u32: 4, u16: 2 }[type];
    if (this.reading) {
      this.need(4, k);
      const n = this.r.i32();
      this.need(n * size, k);
      o[k] = [];
      for (let i = 0; i < n; i++) o[k].push(this.r[type]());
    } else { this.w.i32(o[k].length); const b = Buffer.alloc(o[k].length * size); o[k].forEach((v, i) => (size === 2 ? b.writeUInt16LE(v, i * 2) : type === 'i32' ? b.writeInt32LE(v, i * 4) : b.writeUInt32LE(v, i * 4))); this.w.push(b); }
    return o[k];
  }
  // A byte range handed over by name. `n` is only used when reading.
  blob(o, k, id, n) {
    if (this.reading) { this.need(n, id); o[k] = n ? id : null; if (n) this.blobs[id] = this.r.bytes(n); o[k + 'Bytes'] = n; } else if (o[k + 'Bytes']) {
      const b = this.blobs[o[k]];
      if (!b || b.length !== o[k + 'Bytes']) throw new Error(`blob ${o[k]}: ${b ? b.length : 'no'} bytes, expected ${o[k + 'Bytes']}`);
      this.w.push(b);
    }
  }
  // TArray with bulk serialization (i32 element size, i32 count, data) as a blob.
  bulk(o, k, id, expectSize) {
    const x = this.reading ? (o[k] = {}) : o[k];
    this.i32(x, 'elementSize'); this.i32(x, 'count');
    if (this.reading) {
      if (x.elementSize < 0 || x.count < 0) throw new Error(`${id}: bad bulk array header`);
      if (expectSize !== undefined && x.count && x.elementSize !== expectSize) throw new Error(`${id}: element size ${x.elementSize}, expected ${expectSize}`);
    }
    this.blob(x, 'blob', id, this.reading ? x.elementSize * x.count : 0);
    return x;
  }
  // TArray of fixed-size elements (i32 count, data) as a blob.
  arrayBlob(o, k, id, elementSize) {
    const x = this.reading ? (o[k] = {}) : o[k];
    this.i32(x, 'count');
    if (this.reading && x.count < 0) throw new Error(`${id}: bad array count`);
    this.blob(x, 'blob', id, this.reading ? elementSize * x.count : 0);
    return x;
  }
}

// ---------------------------------------------------------------- shared pieces

function serHead(ar, m) {
  ar.props(m, 'props');
  ar.i32(m, 'objectGuidFlag');
  if (m.objectGuidFlag !== 0) throw new Error('object guid present');
}
// FStaticMaterial / FSkeletalMaterial, 36 bytes.
function serMaterial(ar, x) {
  ar.i32(x, 'materialInterface'); ar.name(x, 'slotName');
  ar.bool(x, 'uvInitialized'); ar.bool(x, 'uvOverrideDensities'); ar.floats(x, 'localUVDensities', 4);
}
// FBoxSphereBounds, 28 bytes.
function serBounds(ar, o, k) { ar.floats(o, k, 7); }

// ---------------------------------------------------------------- UStaticMesh

const STATIC_INDEX_BUFFERS = ['indices', 'reversedIndices', 'depthOnlyIndices', 'reversedDepthOnlyIndices', 'adjacencyIndices'];

function serStaticLod(ar, lod, i) {
  const id = `lod${i}.`;
  ar.bytes2(lod, 'stripFlags');
  ar.array(lod, 'sections', (s) => {
    ar.i32(s, 'materialIndex'); ar.i32(s, 'firstIndex'); ar.i32(s, 'numTriangles'); ar.i32(s, 'minVertexIndex'); ar.i32(s, 'maxVertexIndex');
    ar.bool(s, 'enableCollision'); ar.bool(s, 'castShadow');
  }, 4096);
  ar.f32(lod, 'maxDeviation');
  ar.i32(lod, 'positionStride'); ar.i32(lod, 'positionCount');
  ar.bulk(lod, 'positions', id + 'positions', 12);
  ar.bytes2(lod, 'vertexStripFlags');
  ar.i32(lod, 'numTexCoords'); ar.i32(lod, 'vertexStride'); ar.i32(lod, 'vertexCount');
  ar.bool(lod, 'fullPrecisionUVs'); ar.bool(lod, 'highPrecisionTangents');
  ar.bulk(lod, 'vertices', id + 'vertices');
  ar.bytes2(lod, 'colorStripFlags');
  ar.i32(lod, 'colorStride'); ar.i32(lod, 'colorCount');
  if (lod.colorCount > 0) ar.bulk(lod, 'colors', id + 'colors', 4);
  for (const nm of STATIC_INDEX_BUFFERS) {
    const b = ar.reading ? (lod[nm] = {}) : lod[nm];
    ar.bool(b, 'is32Bit');
    ar.bulk(b, 'data', id + nm, 1);
    if (ar.reading && b.data.count % (b.is32Bit ? 4 : 2)) throw new Error(nm + ': byte count not a multiple of the index size');
  }
  if (ar.reading) lod.samplers = [];
  for (let s = 0; s <= lod.sections.length; s++) { // one per section, then one for the LOD
    const x = ar.reading ? (lod.samplers[s] = {}) : lod.samplers[s];
    ar.array(x, 'prob', (e) => ar.f32(e, 'v')); ar.nums(x, 'alias', 'i32'); ar.f32(x, 'totalWeight');
  }
  if (ar.reading) {
    const want = (lod.highPrecisionTangents ? 16 : 8) + lod.numTexCoords * (lod.fullPrecisionUVs ? 8 : 4);
    if (lod.positionStride !== 12 || lod.positions.count !== lod.positionCount) throw new Error('position buffer header disagrees');
    if (lod.vertices.count !== lod.vertexCount || lod.vertexCount !== lod.positionCount || (lod.vertexCount && (lod.vertices.elementSize !== lod.vertexStride || lod.vertexStride !== want))) throw new Error('vertex buffer header disagrees');
    if (lod.colorCount > 0 && (lod.colors.count !== lod.colorCount || lod.colorStride !== 4)) throw new Error('color buffer header disagrees');
  }
}

function serStaticMesh(ar, m) {
  serHead(ar, m);
  ar.bytes2(m, 'stripFlags');
  ar.bool(m, 'cooked');
  ar.i32(m, 'bodySetup'); ar.i32(m, 'navCollision');
  ar.hex(m, 'lightingGuid', 16);
  ar.nums(m, 'sockets', 'i32');
  ar.array(m, 'lods', (lod, i) => serStaticLod(ar, lod, i), 8);
  ar.bytes2(m, 'distanceFieldStripFlags');
  if (ar.reading) m.distanceFields = m.lods.map(() => ({}));
  m.distanceFields.forEach((d, i) => {
    ar.bool(d, 'valid');
    if (!d.valid) return;
    ar.arrayBlob(d, 'volume', `lod${i}.distanceFieldVolume`, 1);
    ar.nums2(d);
  });
  serBounds(ar, m, 'bounds');
  ar.bool(m, 'lodsShareStaticLighting');
  ar.floats(m, 'screenSize', 8);
  ar.bool(m, 'hasSpeedTreeWind');
  if (m.hasSpeedTreeWind) {
    // The wind block is not interpreted. Reading: the material array is the last thing in the
    // export, so its start is found from the end and everything in between is the wind block.
    if (ar.reading) {
      const cands = [];
      for (let n = 0; ar.end - 4 - 36 * n >= ar.r.p; n++) if (ar.r.b.readInt32LE(ar.end - 4 - 36 * n) === n) cands.push(ar.end - 4 - 36 * n);
      // More than one position can look like the array's count; the tagged property
      // StaticMaterials holds the same array, so its length picks the right one.
      const prop = m.props.find((x) => x.tag.name === 'StaticMaterials');
      const want = prop && prop.value && prop.value.items ? prop.value.items.length : null;
      const pick = cands.length === 1 ? cands : cands.filter((p) => want !== null && (ar.end - 4 - p) / 36 === want);
      if (pick.length !== 1) throw new Error(`SpeedTree wind block: ${cands.length} possible ends`);
      m.speedTreeWind = ar.r.hex(pick[0] - ar.r.p);
    } else ar.w.hex(m.speedTreeWind);
  }
  ar.array(m, 'materials', (x) => serMaterial(ar, x), 4096);
}
// Distance field tail: Size (3 x i32), LocalBoundingBox (6 x f32 + u8 valid), DistanceMinMax (2 x f32), three bools.
Ar.prototype.nums2 = function (d) {
  if (this.reading) { d.size = [this.r.i32(), this.r.i32(), this.r.i32()]; } else for (const v of d.size) this.w.i32(v);
  this.floats(d, 'localBoundingBox', 6); this.u8(d, 'localBoundingBoxValid'); this.floats(d, 'distanceMinMax', 2);
  this.bool(d, 'meshWasClosed'); this.bool(d, 'builtAsIfTwoSided'); this.bool(d, 'meshWasPlane');
};

// ---------------------------------------------------------------- USkeletalMesh

function serSkelSection(ar, s, id) {
  ar.bytes2(s, 'stripFlags');
  ar.u16(s, 'materialIndex'); ar.u32(s, 'baseIndex'); ar.u32(s, 'numTriangles');
  ar.u8(s, 'triangleSorting'); ar.bool(s, 'disabled');
  ar.i16(s, 'correspondClothSectionIndex'); ar.u8(s, 'enableClothLOD');
  ar.bool(s, 'recomputeTangent'); ar.bool(s, 'castShadow');
  ar.u32(s, 'baseVertexIndex');
  ar.nums(s, 'boneMap', 'u16');
  ar.i32(s, 'numVertices'); ar.i32(s, 'maxBoneInfluences');
  ar.arrayBlob(s, 'clothMappingData', id + 'clothMappingData', 64);
  ar.arrayBlob(s, 'physicalMeshVertices', id + 'physicalMeshVertices', 12);
  ar.arrayBlob(s, 'physicalMeshNormals', id + 'physicalMeshNormals', 12);
  ar.i16(s, 'correspondClothAssetIndex');
  ar.hex(s, 'clothingAssetGuid', 16); ar.i32(s, 'clothingAssetLodIndex');
  // Not in the stock 4.17 layout; meaning not identified. An i32 that is 0 or 1, then an array of
  // 24-byte elements (six floats each) that is either empty or has one element per section vertex.
  ar.i32(s, 'unk_tailFlag');
  ar.arrayBlob(s, 'unk_tailArray', id + 'unk_tailArray', 24);
}

function serSkelLod(ar, lod, i, hasColors) {
  const id = `lod${i}.`;
  ar.bytes2(lod, 'stripFlags');
  ar.array(lod, 'sections', (s, j) => serSkelSection(ar, s, `${id}section${j}.`), 1000);
  ar.u8(lod, 'indexSize');
  if (lod.indexSize !== 2 && lod.indexSize !== 4) throw new Error('index size is ' + lod.indexSize);
  ar.bulk(lod, 'indices', id + 'indices', lod.indexSize);
  ar.nums(lod, 'activeBoneIndices', 'u16');
  ar.u32(lod, 'size');
  ar.u32(lod, 'numVertices');
  ar.nums(lod, 'requiredBones', 'u16');
  ar.arrayBlob(lod, 'meshToImportVertexMap', id + 'meshToImportVertexMap', 4);
  ar.i32(lod, 'maxImportVertex');
  ar.u32(lod, 'numTexCoords');
  ar.bytes2(lod, 'vertexStripFlags');
  ar.u32(lod, 'vertexNumTexCoords'); ar.bool(lod, 'fullPrecisionUVs');
  ar.floats(lod, 'meshExtension', 3); ar.floats(lod, 'meshOrigin', 3);
  ar.bulk(lod, 'vertices', id + 'vertices');
  ar.bytes2(lod, 'weightStripFlags');
  ar.bool(lod, 'extraBoneInfluences'); ar.u32(lod, 'weightVertexCount');
  ar.bulk(lod, 'weights', id + 'weights', lod.extraBoneInfluences ? 16 : 8);
  if (hasColors) {
    ar.bytes2(lod, 'colorStripFlags');
    ar.i32(lod, 'colorStride'); ar.i32(lod, 'colorCount');
    if (lod.colorCount > 0) ar.bulk(lod, 'colors', id + 'colors', 4);
  }
  if (!(lod.stripFlags[1] & 1)) { // class strip flag bit 0: adjacency data stripped
    ar.u8(lod, 'adjacencyIndexSize');
    ar.bulk(lod, 'adjacencyIndices', id + 'adjacencyIndices', lod.adjacencyIndexSize);
  }
  if (lod.sections.some((s) => s.clothMappingData.count > 0)) {
    ar.bytes2(lod, 'clothStripFlags');
    ar.bulk(lod, 'clothVertices', id + 'clothVertices', 64);
    ar.arrayBlob(lod, 'clothIndexMapping', id + 'clothIndexMapping', 8);
  }
  // Not in the stock 4.17 layout: when a section carries the unidentified 24-byte array, the LOD
  // ends with a buffer of the same element size, one element per LOD vertex.
  if (lod.sections.some((s) => s.unk_tailArray.count > 0)) {
    ar.bytes2(lod, 'unk_buffer24StripFlags');
    ar.bulk(lod, 'unk_buffer24', id + 'unk_buffer24', 24);
  }
  if (ar.reading) {
    if (lod.indexSize !== 2 && lod.indexSize !== 4) throw new Error('index size is ' + lod.indexSize);
    const tris = lod.sections.reduce((n, s) => n + s.numTriangles, 0);
    if (lod.indices.count !== tris * 3) throw new Error('index count is not 3 x triangles');
    const want = 20 + lod.numTexCoords * (lod.fullPrecisionUVs ? 8 : 4);
    if (lod.numTexCoords !== lod.vertexNumTexCoords || lod.vertices.count !== lod.numVertices || (lod.numVertices && lod.vertices.elementSize !== want)) throw new Error('vertex buffer header disagrees');
    if (lod.weightVertexCount !== lod.numVertices || lod.weights.count !== lod.numVertices) throw new Error('skin weight count differs from vertex count');
    if (hasColors && lod.colorCount > 0 && (lod.colors.count !== lod.colorCount || lod.colorStride !== 4)) throw new Error('color buffer header disagrees');
  }
}

function serSkeletalMesh(ar, m) {
  serHead(ar, m);
  const vc = m.props.find((x) => x.tag.name === 'bHasVertexColors');
  const hasColors = !!(vc && vc.tag.boolVal);
  ar.bytes2(m, 'stripFlags');
  serBounds(ar, m, 'importedBounds');
  ar.array(m, 'materials', (x) => serMaterial(ar, x), 4096);
  ar.array(m, 'boneInfo', (b) => { ar.name(b, 'name'); ar.i32(b, 'parent'); }, 65536);
  ar.arrayBlob(m, 'refPose', 'refPose', 40);
  ar.array(m, 'nameToIndex', (b) => { ar.name(b, 'name'); ar.i32(b, 'index'); }, 65536);
  if (ar.reading && (m.refPose.count !== m.boneInfo.length || m.nameToIndex.length !== m.boneInfo.length)) throw new Error('reference skeleton arrays differ in length');
  ar.array(m, 'lods', (lod, i) => serSkelLod(ar, lod, i, hasColors), 8);
  ar.i32(m, 'unk_afterLods');
  if (ar.reading) m.hasTrailingArrays = ar.r.p < ar.end;
  // A few meshes carry one more block: i32 count, then per entry a u16 array. Meaning not identified.
  if (m.hasTrailingArrays) ar.array(m, 'unk_trailingArrays', (x) => ar.nums(x, 'values', 'u16'), 64);
}

const SERIALIZERS = { StaticMesh: serStaticMesh, SkeletalMesh: serSkeletalMesh };

function parseExport(pkg, uexp, exp, cls) {
  const start = exp.serialOffset - pkg.summary.totalHeaderSize, end = start + exp.serialSize;
  if (start < 0 || end > uexp.length) throw new Error('export outside .uexp');
  const ar = new Ar({ pkg, r: new Reader(uexp, start), end, blobs: {} });
  const doc = {};
  try { SERIALIZERS[cls](ar, doc); } catch (e) { if (e instanceof RangeError) throw new Error('read past the end of the .uexp'); throw e; }
  if (ar.r.p !== end) throw new Error(ar.r.p < end ? `stopped ${end - ar.r.p} bytes before the end` : `ran ${ar.r.p - end} bytes past the end`);
  return { doc, blobs: ar.blobs, start, end };
}

function serialize(pkg, cls, doc, blobs) {
  const ar = new Ar({ pkg, w: new Writer(), blobs });
  SERIALIZERS[cls](ar, doc);
  return ar.w.concat();
}

// ---------------------------------------------------------------- half floats

// Half -> float32 bit patterns. Exact for every half, including subnormals, infinities and NaNs.
const H2F = new Uint32Array(65536);
for (let h = 0; h < 65536; h++) {
  const s = (h >>> 15) << 31, e = (h >>> 10) & 31, m = h & 1023;
  let bits;
  if (e === 0) {
    if (m === 0) bits = s;
    else { let ee = -1, mm = m; do { ee++; mm <<= 1; } while (!(mm & 0x400)); bits = s | ((112 - ee) << 23) | ((mm & 0x3ff) << 13); }
  } else if (e === 31) bits = s | 0x7f800000 | (m << 13);
  else bits = s | ((e + 112) << 23) | (m << 13);
  H2F[h] = bits >>> 0;
}
// float32 bits -> half, or -1 when the float is not exactly a half.
function floatBitsToHalf(bits) {
  const s = (bits >>> 31) << 15, e = (bits >>> 23) & 255, m = bits & 0x7fffff;
  let h;
  if (e === 255) h = s | 0x7c00 | (m >>> 13);
  else if (e === 0) h = s;
  else {
    const E = e - 127;
    if (E > 15 || E < -24) return -1;
    if (E >= -14) h = s | ((E + 15) << 10) | (m >>> 13);
    else h = s | ((0x800000 | m) >>> (-1 - E));
  }
  return H2F[h] === bits >>> 0 ? h : -1;
}

// ---------------------------------------------------------------- GLB container

const GL = { BYTE: 5120, UBYTE: 5121, SHORT: 5122, USHORT: 5123, UINT: 5125, FLOAT: 5126, ARRAY_BUFFER: 34962, ELEMENT_ARRAY_BUFFER: 34963 };
const COMPONENT_BYTES = { 5120: 1, 5121: 1, 5122: 2, 5123: 2, 5125: 4, 5126: 4 };
const TYPE_COMPONENTS = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4, MAT4: 16 };

class GlbBuilder {
  constructor(generator) {
    this.json = { asset: { version: '2.0', generator }, scene: 0, scenes: [{ nodes: [] }], nodes: [], buffers: [{ byteLength: 0 }], bufferViews: [], accessors: [] };
    this.parts = []; this.len = 0;
  }
  view(buf, opts = {}) {
    const pad = (4 - (this.len & 3)) & 3;
    if (pad) { this.parts.push(Buffer.alloc(pad)); this.len += pad; }
    const v = { buffer: 0, byteOffset: this.len, byteLength: buf.length, ...opts };
    this.parts.push(buf); this.len += buf.length;
    return this.json.bufferViews.push(v) - 1;
  }
  accessor(view, componentType, type, count, opts = {}) { return this.json.accessors.push({ bufferView: view, componentType, type, count, ...opts }) - 1; }
  // one tightly packed vertex attribute
  attribute(buf, componentType, type, count, opts = {}) { return this.accessor(this.view(buf, { target: GL.ARRAY_BUFFER }), componentType, type, count, opts); }
  finish() {
    const pad = (4 - (this.len & 3)) & 3;
    if (pad) { this.parts.push(Buffer.alloc(pad)); this.len += pad; }
    const j = this.json;
    j.buffers[0].byteLength = this.len;
    if (!this.len) delete j.buffers;
    for (const k of ['bufferViews', 'accessors', 'meshes', 'materials', 'skins']) if (j[k] && !j[k].length) delete j[k];
    let text = Buffer.from(JSON.stringify(j), 'utf8');
    if (text.length & 3) text = Buffer.concat([text, Buffer.alloc(4 - (text.length & 3), 0x20)]);
    const head = Buffer.alloc(20);
    const total = 12 + 8 + text.length + (this.len ? 8 + this.len : 0);
    if (total > 0xffffffff) throw new Error('GLB larger than 4 GiB');
    head.writeUInt32LE(0x46546c67, 0); head.writeUInt32LE(2, 4); head.writeUInt32LE(total, 8);
    head.writeUInt32LE(text.length, 12); head.writeUInt32LE(0x4e4f534a, 16);
    if (!this.len) return Buffer.concat([head, text]);
    const bh = Buffer.alloc(8);
    bh.writeUInt32LE(this.len, 0); bh.writeUInt32LE(0x004e4942, 4);
    return Buffer.concat([head, text, bh, ...this.parts]);
  }
}

function readGlb(buf) {
  if (buf.length < 20 || buf.readUInt32LE(0) !== 0x46546c67 || buf.readUInt32LE(4) !== 2) throw new Error('not a GLB (glTF 2.0 binary) file');
  if (buf.readUInt32LE(8) !== buf.length) throw new Error('GLB length field differs from the file size');
  const jl = buf.readUInt32LE(12);
  if (buf.readUInt32LE(16) !== 0x4e4f534a) throw new Error('GLB: first chunk is not JSON');
  const json = JSON.parse(buf.toString('utf8', 20, 20 + jl));
  let bin = Buffer.alloc(0);
  if (20 + jl < buf.length) {
    const bl = buf.readUInt32LE(20 + jl);
    if (buf.readUInt32LE(24 + jl) !== 0x004e4942) throw new Error('GLB: second chunk is not BIN');
    bin = buf.subarray(28 + jl, 28 + jl + bl);
  }
  const viewBytes = (i) => { const v = json.bufferViews[i]; return bin.subarray(v.byteOffset || 0, (v.byteOffset || 0) + v.byteLength); };
  const accessorBytes = (i) => {
    const a = json.accessors[i], v = json.bufferViews[a.bufferView];
    if (v.byteStride) throw new Error('interleaved accessors are not produced by this converter');
    const o = (v.byteOffset || 0) + (a.byteOffset || 0);
    return bin.subarray(o, o + a.count * COMPONENT_BYTES[a.componentType] * TYPE_COMPONENTS[a.type]);
  };
  return { json, bin, viewBytes, accessorBytes };
}

// ---------------------------------------------------------------- vertex decoding (export -> glTF)

// UE space (left-handed, Z up, centimetres) -> glTF space (right-handed, Y up, metres):
// (x, y, z) -> (x, z, y) / 100. The matrix is a reflection, so glTF viewers reverse the winding of
// everything below it; index order is therefore handled per LOD (see `windingSwapped`).
const UE_TO_GLTF = [0.01, 0, 0, 0, 0, 0, 0.01, 0, 0, 0.01, 0, 0, 0, 0, 0, 1];

// Packed tangent basis -> unit float vectors for viewers. src: Buffer of count x stride bytes;
// xOff/zOff: byte offsets of TangentX / TangentZ; wide: 4 x u16 (FPackedRGBA16N) instead of 4 x u8
// (FPackedNormal). Returns the original packed values untouched plus the derived floats.
function decodeTangentBasis(src, count, stride, xOff, zOff, wide) {
  const w = wide ? 8 : 4;
  const px = Buffer.alloc(count * w), pz = Buffer.alloc(count * w);
  const nrm = new Float32Array(count * 3), tan = new Float32Array(count * 4);
  const k = wide ? 1 / 32767.5 : 1 / 127.5;
  const stats = { zeroNormals: 0, zeroTangents: 0, normalLenMin: Infinity, normalLenMax: 0, tangentLenMin: Infinity, tangentLenMax: 0, dotMax: 0, wValues: {} };
  const comp = wide ? (o) => src.readUInt16LE(o) * k - 1 : (o) => src[o] * k - 1;
  const cs = wide ? 2 : 1;
  for (let i = 0; i < count; i++) {
    const o = i * stride;
    src.copy(px, i * w, o + xOff, o + xOff + w);
    src.copy(pz, i * w, o + zOff, o + zOff + w);
    let nx = comp(o + zOff), ny = comp(o + zOff + cs), nz = comp(o + zOff + 2 * cs);
    let tx = comp(o + xOff), ty = comp(o + xOff + cs), tz = comp(o + xOff + 2 * cs);
    const wRaw = wide ? src.readUInt16LE(o + zOff + 6) : src[o + zOff + 3];
    stats.wValues[wRaw] = (stats.wValues[wRaw] || 0) + 1;
    const nl = Math.hypot(nx, ny, nz), tl = Math.hypot(tx, ty, tz);
    if (nl < stats.normalLenMin) stats.normalLenMin = nl;
    if (nl > stats.normalLenMax) stats.normalLenMax = nl;
    if (tl < stats.tangentLenMin) stats.tangentLenMin = tl;
    if (tl > stats.tangentLenMax) stats.tangentLenMax = tl;
    if (nl > 1e-4 && tl > 1e-4) { const d = Math.abs(nx * tx + ny * ty + nz * tz) / (nl * tl); if (d > stats.dotMax) stats.dotMax = d; }
    if (!(nl > 1e-4)) { stats.zeroNormals++; nx = 0; ny = 0; nz = 1; } else { nx /= nl; ny /= nl; nz /= nl; }
    if (!(tl > 1e-4)) { stats.zeroTangents++; tx = 1; ty = 0; tz = 0; } else { tx /= tl; ty /= tl; tz /= tl; }
    nrm[i * 3] = nx; nrm[i * 3 + 1] = ny; nrm[i * 3 + 2] = nz;
    tan[i * 4] = tx; tan[i * 4 + 1] = ty; tan[i * 4 + 2] = tz;
    tan[i * 4 + 3] = (wide ? wRaw >= 32768 : wRaw >= 128) ? 1 : -1;
  }
  return { px, pz, nrm: Buffer.from(nrm.buffer), tan: Buffer.from(tan.buffer), stats };
}

// UV set `set` of an interleaved buffer as float32 pairs.
function decodeUVs(src, count, stride, off, full) {
  const out = Buffer.alloc(count * 8);
  if (full) for (let i = 0; i < count; i++) src.copy(out, i * 8, i * stride + off, i * stride + off + 8);
  else for (let i = 0; i < count; i++) { const o = i * stride + off; out.writeUInt32LE(H2F[src.readUInt16LE(o)], i * 8); out.writeUInt32LE(H2F[src.readUInt16LE(o + 2)], i * 8 + 4); }
  return out;
}
function encodeUVs(dst, src, count, stride, off, full) {
  if (src.length !== count * 8) throw new Error('TEXCOORD accessor has the wrong length');
  if (full) { for (let i = 0; i < count; i++) src.copy(dst, i * stride + off, i * 8, i * 8 + 8); return; }
  for (let i = 0; i < count; i++) {
    const a = floatBitsToHalf(src.readUInt32LE(i * 8)), b = floatBitsToHalf(src.readUInt32LE(i * 8 + 4));
    if (a < 0 || b < 0) throw new Error(`TEXCOORD value of vertex ${i} is not representable as a half float`);
    dst.writeUInt16LE(a, i * stride + off); dst.writeUInt16LE(b, i * stride + off + 2);
  }
}
// column `off`, `width` bytes, of an interleaved buffer <-> tight buffer
function column(src, count, stride, off, width) { const out = Buffer.alloc(count * width); for (let i = 0; i < count; i++) src.copy(out, i * width, i * stride + off, i * stride + off + width); return out; }
function setColumn(dst, src, count, stride, off, width) { if (src.length !== count * width) throw new Error('accessor has the wrong length'); for (let i = 0; i < count; i++) src.copy(dst, i * stride + off, i * width, i * width + width); }
// FColor is stored B, G, R, A; glTF COLOR_0 is R, G, B, A. The same function converts both ways.
function swapRB(src) { const out = Buffer.from(src); for (let i = 0; i + 3 < out.length; i += 4) { out[i] = src[i + 2]; out[i + 2] = src[i]; } return out; }
// (a, b, c) -> (a, c, b) for every triangle; its own inverse.
function swapWinding(src, size) {
  const out = Buffer.from(src);
  for (let o = 0; o + 3 * size <= out.length; o += 3 * size) { src.copy(out, o + size, o + 2 * size, o + 3 * size); src.copy(out, o + 2 * size, o + size, o + 2 * size); }
  return out;
}
function positionMinMax(buf, count) {
  const min = [Infinity, Infinity, Infinity], max = [-Infinity, -Infinity, -Infinity];
  let bad = 0;
  for (let i = 0; i < count; i++) for (let c = 0; c < 3; c++) {
    const v = buf.readFloatLE(i * 12 + c * 4);
    if (!Number.isFinite(v)) { bad++; continue; }
    if (v < min[c]) min[c] = v;
    if (v > max[c]) max[c] = v;
  }
  return { min, max, bad };
}
const texAttr = (g, uv, count) => g.attribute(uv, GL.FLOAT, 'VEC2', count);

// Full object path of a package index ("/Game/.../M_Foo.M_Foo" style, built from the import/export outers).
function objectPath(pkg, idx) {
  const parts = [];
  for (let i = idx, guard = 0; i !== 0 && guard < 32; guard++) {
    const o = i > 0 ? pkg.exports[i - 1] : pkg.imports[-i - 1];
    if (!o) return null;
    parts.unshift(nameOf(pkg, o.objectName));
    i = o.outerIndex;
  }
  return parts.length ? parts.join('.') : null;
}

function addMaterials(g, pkg, materials) {
  g.json.materials = materials.map((m) => ({ name: typeof m.slotName === 'string' ? m.slotName : nameOf(pkg, m.slotName), extras: { ue_material: objectPath(pkg, m.materialInterface) } }));
}

// Index buffer -> one primitive per section. Returns the bufferView and whether winding was swapped.
function addIndexedPrimitives(g, idxBlob, indexSize, sections, attributes, numVertices, nMaterials) {
  // Under the reflecting root matrix a triangle is front-facing when its stored order is the
  // reverse of what the export holds, so every triangle is stored as (a, c, b). That is only a
  // faithful per-triangle operation when every section starts on a triangle boundary.
  const canSwap = idxBlob.length % (3 * indexSize) === 0 && sections.every((s) => s.first % 3 === 0);
  const view = g.view(canSwap ? swapWinding(idxBlob, indexSize) : idxBlob, { target: GL.ELEMENT_ARRAY_BUFFER });
  const primitives = [];
  const total = idxBlob.length / indexSize;
  for (const s of sections) {
    if (s.tris <= 0 || s.first < 0 || s.first + s.tris * 3 > total) continue;
    const p = { attributes, indices: g.accessor(view, indexSize === 4 ? GL.UINT : GL.USHORT, 'SCALAR', s.tris * 3, { byteOffset: s.first * indexSize }), mode: 4 };
    if (s.material >= 0 && s.material < nMaterials) p.material = s.material;
    primitives.push(p);
  }
  return { view, swapped: canSwap, primitives };
}

// ---------------------------------------------------------------- StaticMesh -> GLB

function staticToGlb(g, pkg, doc, blobs, map) {
  addMaterials(g, pkg, doc.materials);
  g.json.meshes = [];
  const root = { name: 'UE_to_glTF', matrix: UE_TO_GLTF, children: [] };
  g.json.nodes.push(root);
  const lodNodes = [];
  const info = [];
  doc.lods.forEach((lod, i) => {
    const id = `lod${i}.`, n = lod.vertexCount;
    const li = { vertices: n };
    info.push(li);
    const idx = blobs[id + 'indices'];
    if (!n || !idx) return;
    const pos = blobs[id + 'positions'], vtx = blobs[id + 'vertices'];
    const stride = lod.vertexStride, wide = !!lod.highPrecisionTangents, tb = wide ? 16 : 8, full = !!lod.fullPrecisionUVs;
    const mm = positionMinMax(pos, n);
    const attributes = {};
    attributes.POSITION = g.attribute(pos, GL.FLOAT, 'VEC3', n, mm.bad === n ? {} : { min: mm.min, max: mm.max });
    const t = decodeTangentBasis(vtx, n, stride, 0, tb / 2, wide);
    li.tangentStats = t.stats;
    attributes.NORMAL = g.attribute(t.nrm, GL.FLOAT, 'VEC3', n);
    attributes.TANGENT = g.attribute(t.tan, GL.FLOAT, 'VEC4', n);
    const texcoords = [];
    for (let k = 0; k < lod.numTexCoords; k++) texcoords.push((attributes[`TEXCOORD_${k}`] = texAttr(g, decodeUVs(vtx, n, stride, tb + k * (full ? 8 : 4), full), n)));
    attributes._UE_TANGENT_X = g.attribute(t.px, wide ? GL.USHORT : GL.UBYTE, 'VEC4', n);
    attributes._UE_TANGENT_Z = g.attribute(t.pz, wide ? GL.USHORT : GL.UBYTE, 'VEC4', n);
    map[id + 'positions'] = { kind: 'accessor', accessor: attributes.POSITION };
    map[id + 'vertices'] = { kind: 'static_vertices', count: n, stride, wideTangents: wide, fullPrecisionUVs: full, tangentX: attributes._UE_TANGENT_X, tangentZ: attributes._UE_TANGENT_Z, texcoords };
    const col = blobs[id + 'colors'];
    if (col && lod.colorCount === n) { attributes.COLOR_0 = g.attribute(swapRB(col), GL.UBYTE, 'VEC4', n, { normalized: true }); map[id + 'colors'] = { kind: 'colors_bgra', accessor: attributes.COLOR_0 }; }
    const indexSize = lod.indices.is32Bit ? 4 : 2;
    const ip = addIndexedPrimitives(g, idx, indexSize, lod.sections.map((s) => ({ first: s.firstIndex, tris: s.numTriangles, material: s.materialIndex })), attributes, n, doc.materials.length);
    map[id + 'indices'] = { kind: 'indices', view: ip.view, indexSize, windingSwapped: ip.swapped };
    li.windingSwapped = ip.swapped;
    if (!ip.primitives.length) return;
    const mesh = g.json.meshes.push({ name: `LOD${i}`, primitives: ip.primitives }) - 1;
    lodNodes[i] = g.json.nodes.push({ name: `LOD${i}`, mesh }) - 1;
  });
  const shown = lodNodes.find((x) => x !== undefined);
  if (shown !== undefined) root.children.push(shown); else delete root.children;
  g.json.scenes[0].nodes.push(0);
  g.json.scenes[0].extras = { ue_lod_nodes: doc.lods.map((_, i) => (lodNodes[i] === undefined ? null : lodNodes[i])) };
  // LODs other than the first one shown hang under their own copy of the conversion root, outside the scene.
  lodNodes.forEach((nodeId, i) => { if (nodeId !== undefined && nodeId !== shown) g.json.nodes.push({ name: `UE_to_glTF_LOD${i}`, matrix: UE_TO_GLTF, children: [nodeId] }); });
  return info;
}

// ---------------------------------------------------------------- SkeletalMesh -> GLB

function mat4Mul(a, b) { // column-major
  const o = new Array(16);
  for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) o[c * 4 + r] = a[r] * b[c * 4] + a[4 + r] * b[c * 4 + 1] + a[8 + r] * b[c * 4 + 2] + a[12 + r] * b[c * 4 + 3];
  return o;
}
function mat4FromTRS(q, t, s) {
  const [x, y, z, w] = q;
  return [
    (1 - 2 * (y * y + z * z)) * s[0], 2 * (x * y + z * w) * s[0], 2 * (x * z - y * w) * s[0], 0,
    2 * (x * y - z * w) * s[1], (1 - 2 * (x * x + z * z)) * s[1], 2 * (y * z + x * w) * s[1], 0,
    2 * (x * z + y * w) * s[2], 2 * (y * z - x * w) * s[2], (1 - 2 * (x * x + y * y)) * s[2], 0,
    t[0], t[1], t[2], 1,
  ];
}
// Inverse of an affine matrix (column-major); null when singular.
function mat4InvAffine(m) {
  const a = m[0], b = m[4], c = m[8], d = m[1], e = m[5], f = m[9], g = m[2], h = m[6], i = m[10];
  const A = e * i - f * h, B = f * g - d * i, C = d * h - e * g;
  const det = a * A + b * B + c * C;
  if (!Number.isFinite(det) || Math.abs(det) < 1e-30) return null;
  const r = [A / det, B / det, C / det, 0, (c * h - b * i) / det, (a * i - c * g) / det, (b * g - a * h) / det, 0, (b * f - c * e) / det, (c * d - a * f) / det, (a * e - b * d) / det, 0, 0, 0, 0, 1];
  r[12] = -(r[0] * m[12] + r[4] * m[13] + r[8] * m[14]);
  r[13] = -(r[1] * m[12] + r[5] * m[13] + r[9] * m[14]);
  r[14] = -(r[2] * m[12] + r[6] * m[13] + r[10] * m[14]);
  return r;
}

// 40-byte bone pose (quaternion xyzw, translation, scale) <-> glTF node TRS numbers.
// A bone goes into `overrides` (hex of its 40 bytes) when the numbers written to the node would not
// reproduce those bytes: -0, non-finite values, or a quaternion that had to be normalised for glTF.
function poseToNodes(buf, count) {
  const nodes = [], overrides = {};
  const reasons = { negativeZero: 0, nonFinite: 0, nonUnitQuaternion: 0 };
  let maxQuatDeviation = 0;
  for (let b = 0; b < count; b++) {
    const v = [];
    let exact = true, neg0 = false, nonFinite = false;
    for (let c = 0; c < 10; c++) { const f = buf.readFloatLE(b * 40 + c * 4); if (!Number.isFinite(f)) nonFinite = true; else if (Object.is(f, -0)) neg0 = true; v.push(Number.isFinite(f) ? f + 0 : 0); }
    if (neg0) { reasons.negativeZero++; exact = false; }
    if (nonFinite) { reasons.nonFinite++; exact = false; }
    let q = v.slice(0, 4);
    const l2 = q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3];
    if (Math.abs(l2 - 1) > maxQuatDeviation) maxQuatDeviation = Math.abs(l2 - 1);
    if (!(Math.abs(l2 - 1) < 1e-4)) { reasons.nonUnitQuaternion++; exact = false; const l = Math.sqrt(l2); q = l > 1e-12 ? q.map((x) => x / l) : [0, 0, 0, 1]; }
    if (!exact) overrides[b] = buf.toString('hex', b * 40, b * 40 + 40);
    nodes.push({ rotation: q, translation: v.slice(4, 7), scale: v.slice(7, 10) });
  }
  return { nodes, overrides, reasons, maxQuatDeviation };
}
function nodesToPose(json, firstNode, count, overrides) {
  const out = Buffer.alloc(count * 40);
  for (let b = 0; b < count; b++) {
    if (overrides[b] !== undefined) { Buffer.from(overrides[b], 'hex').copy(out, b * 40); continue; }
    const n = json.nodes[firstNode + b];
    [...n.rotation, ...n.translation, ...n.scale].forEach((f, c) => out.writeFloatLE(f, b * 40 + c * 4));
  }
  return out;
}

// Skin weights. Export: per vertex N bone indices (u8, into the owning section's bone map) then N
// weights (u8), N = 4 or 8. glTF: JOINTS_n (u16 x 4, skeleton bone index) / WEIGHTS_n (u8 x 4,
// normalised). A slot with weight 0 is written as joint 0, and read back as local index 0.
function decodeWeights(src, count, n, sections, numBones) {
  const sets = n / 4;
  const joints = [], weights = [];
  for (let s = 0; s < sets; s++) { joints.push(Buffer.alloc(count * 8)); weights.push(Buffer.alloc(count * 4)); }
  const owner = new Int32Array(count).fill(-1);
  sections.forEach((s, si) => { for (let v = s.baseVertexIndex, e = Math.min(count, s.baseVertexIndex + s.numVertices); v < e; v++) owner[v] = si; });
  const stats = { unowned: 0, localOutOfRange: 0, boneOutOfRange: 0, sumNot255: 0, zeroWeightNonzeroIndex: 0, maxInfluences: 0 };
  const stride = 2 * n;
  for (let v = 0; v < count; v++) {
    const map = owner[v] >= 0 ? sections[owner[v]].boneMap : null;
    if (!map) stats.unowned++;
    let sum = 0, used = 0;
    for (let k = 0; k < n; k++) {
      const li = src[v * stride + k], w = src[v * stride + n + k];
      sum += w;
      let j = 0;
      if (w) {
        used++;
        if (map && li < map.length) { j = map[li]; if (j >= numBones) { stats.boneOutOfRange++; j = 0; } } else if (map) stats.localOutOfRange++;
      } else if (li) stats.zeroWeightNonzeroIndex++;
      joints[k >> 2].writeUInt16LE(j, v * 8 + (k & 3) * 2);
      weights[k >> 2][v * 4 + (k & 3)] = w;
    }
    if (sum !== 255) stats.sumNot255++;
    if (used > stats.maxInfluences) stats.maxInfluences = used;
  }
  return { joints, weights, stats };
}
function encodeWeights(joints, weights, count, n, sections) {
  const out = Buffer.alloc(count * 2 * n);
  const stride = 2 * n;
  for (const s of sections) {
    const inv = new Map();
    s.boneMap.forEach((b, i) => { if (!inv.has(b)) inv.set(b, i); });
    for (let v = s.baseVertexIndex, e = Math.min(count, s.baseVertexIndex + s.numVertices); v < e; v++) {
      for (let k = 0; k < n; k++) {
        const w = weights[k >> 2][v * 4 + (k & 3)];
        let li = 0;
        if (w) { li = inv.get(joints[k >> 2].readUInt16LE(v * 8 + (k & 3) * 2)); if (li === undefined) throw new Error(`vertex ${v}: joint is not in its section's bone map`); }
        out[v * stride + k] = li; out[v * stride + n + k] = w;
      }
    }
  }
  return out;
}

function skeletalToGlb(g, pkg, doc, blobs, map) {
  addMaterials(g, pkg, doc.materials);
  g.json.meshes = [];
  const nb = doc.boneInfo.length;
  const root = { name: 'UE_to_glTF', matrix: UE_TO_GLTF };
  g.json.nodes.push(root);
  g.json.scenes[0].nodes.push(0);
  // ---- skeleton: nodes 1..nb, in bone order
  const firstBone = 1;
  let skin;
  const info = { lods: [], bones: nb };
  if (nb) {
    const pose = poseToNodes(blobs.refPose, nb);
    info.poseOverrides = Object.keys(pose.overrides).length; info.poseReasons = pose.reasons; info.maxQuatDeviation = pose.maxQuatDeviation;
    const world = [];
    const ibm = Buffer.alloc(nb * 64);
    const rootBones = [];
    let singular = 0, badParent = 0;
    doc.boneInfo.forEach((b, i) => {
      const n = pose.nodes[i];
      const node = { name: typeof b.name === 'string' ? b.name : nameOf(pkg, b.name), translation: n.translation, rotation: n.rotation, scale: n.scale };
      g.json.nodes.push(node);
      const local = mat4FromTRS(n.rotation, n.translation, n.scale);
      const ok = b.parent >= 0 && b.parent < i;
      if (b.parent >= 0 && !ok) badParent++;
      world.push(ok ? mat4Mul(world[b.parent], local) : local);
      if (ok) { const p = g.json.nodes[firstBone + b.parent]; (p.children = p.children || []).push(firstBone + i); } else rootBones.push(firstBone + i);
      let inv = mat4InvAffine(world[i]);
      if (!inv || inv.some((x) => !Number.isFinite(Math.fround(x)))) { singular++; inv = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]; }
      inv.forEach((x, c) => ibm.writeFloatLE(x, i * 64 + c * 4));
    });
    root.children = rootBones;
    info.singularBindMatrices = singular; info.badParents = badParent;
    const acc = g.accessor(g.view(ibm), GL.FLOAT, 'MAT4', nb);
    const sk = { name: 'Skeleton', joints: doc.boneInfo.map((_, i) => firstBone + i), inverseBindMatrices: acc };
    if (rootBones.length === 1) sk.skeleton = rootBones[0];
    g.json.skins = [sk];
    skin = 0;
    map.refPose = { kind: 'node_trs', firstNode: firstBone, count: nb, overrides: pose.overrides };
  }
  // ---- LODs
  const lodNodes = [];
  doc.lods.forEach((lod, i) => {
    const id = `lod${i}.`, n = lod.numVertices;
    const li = { vertices: n };
    info.lods.push(li);
    const idx = blobs[id + 'indices'];
    if (!n || !idx) return;
    const vtx = blobs[id + 'vertices'], stride = lod.vertices.elementSize, full = !!lod.fullPrecisionUVs;
    const pos = column(vtx, n, stride, 8, 12);
    const mm = positionMinMax(pos, n);
    const attributes = {};
    attributes.POSITION = g.attribute(pos, GL.FLOAT, 'VEC3', n, mm.bad === n ? {} : { min: mm.min, max: mm.max });
    const t = decodeTangentBasis(vtx, n, stride, 0, 4, false);
    li.tangentStats = t.stats;
    attributes.NORMAL = g.attribute(t.nrm, GL.FLOAT, 'VEC3', n);
    attributes.TANGENT = g.attribute(t.tan, GL.FLOAT, 'VEC4', n);
    const texcoords = [];
    for (let k = 0; k < lod.numTexCoords; k++) texcoords.push((attributes[`TEXCOORD_${k}`] = texAttr(g, decodeUVs(vtx, n, stride, 20 + k * (full ? 8 : 4), full), n)));
    attributes._UE_TANGENT_X = g.attribute(t.px, GL.UBYTE, 'VEC4', n);
    attributes._UE_TANGENT_Z = g.attribute(t.pz, GL.UBYTE, 'VEC4', n);
    map[id + 'vertices'] = { kind: 'skeletal_vertices', count: n, stride, fullPrecisionUVs: full, tangentX: attributes._UE_TANGENT_X, tangentZ: attributes._UE_TANGENT_Z, position: attributes.POSITION, texcoords };
    const col = blobs[id + 'colors'];
    if (col && lod.colorCount === n) { attributes.COLOR_0 = g.attribute(swapRB(col), GL.UBYTE, 'VEC4', n, { normalized: true }); map[id + 'colors'] = { kind: 'colors_bgra', accessor: attributes.COLOR_0 }; }
    // skin weights
    const wb = blobs[id + 'weights'], ninf = lod.extraBoneInfluences ? 8 : 4;
    if (wb && skin !== undefined) {
      const w = decodeWeights(wb, n, ninf, lod.sections, nb);
      li.weightStats = w.stats;
      const ja = w.joints.map((b, k) => (attributes[`JOINTS_${k}`] = g.attribute(b, GL.USHORT, 'VEC4', n)));
      const wa = w.weights.map((b, k) => (attributes[`WEIGHTS_${k}`] = g.attribute(b, GL.UBYTE, 'VEC4', n, { normalized: true })));
      let invertible = false;
      try { invertible = encodeWeights(w.joints, w.weights, n, ninf, lod.sections).equals(wb); } catch { /* not invertible */ }
      li.weightsFromJoints = invertible;
      if (invertible) map[id + 'weights'] = { kind: 'skin_weights', lod: i, count: n, influences: ninf, joints: ja, weights: wa };
    }
    const ip = addIndexedPrimitives(g, idx, lod.indexSize, lod.sections.map((s) => ({ first: s.baseIndex, tris: s.numTriangles, material: s.materialIndex })), attributes, n, doc.materials.length);
    map[id + 'indices'] = { kind: 'indices', view: ip.view, indexSize: lod.indexSize, windingSwapped: ip.swapped };
    li.windingSwapped = ip.swapped;
    if (!ip.primitives.length) return;
    const mesh = g.json.meshes.push({ name: `LOD${i}`, primitives: ip.primitives }) - 1;
    const node = { name: `LOD${i}`, mesh };
    if (skin !== undefined && attributes.JOINTS_0 !== undefined) node.skin = skin;
    lodNodes[i] = g.json.nodes.push(node) - 1;
  });
  // A skinned mesh node takes its placement from the joints, so it sits at the scene root. Only
  // the first LOD is in the scene; the others are separate nodes outside it.
  const shown = lodNodes.find((x) => x !== undefined);
  if (shown !== undefined) g.json.scenes[0].nodes.push(shown);
  g.json.scenes[0].extras = { ue_lod_nodes: doc.lods.map((_, i) => (lodNodes[i] === undefined ? null : lodNodes[i])) };
  return info;
}

// ---------------------------------------------------------------- public: forward / reverse

// info: { package, export, class, converter, converter_version }
function toGlb(pkg, cls, doc, blobs, info = {}) {
  const g = new GlbBuilder(`kh3-decomp ${info.converter || 'tools/lib/mesh.js'} ${info.converter_version || ''}`.trim());
  const map = {};
  const details = cls === 'StaticMesh' ? staticToGlb(g, pkg, doc, blobs, map) : skeletalToGlb(g, pkg, doc, blobs, map);
  // every blob that has no decoded home in the glTF is carried as a named raw bufferView
  for (const [id, b] of Object.entries(blobs)) if (!map[id]) map[id] = { kind: 'raw', view: g.view(b, { name: 'ue:' + id }) };
  g.json.extras = { ue_package: info.package, ue_export: info.export, ue_class: cls };
  const glb = g.finish();
  const sidecar = { sidecar_version: SIDECAR_VERSION, ...info, class: cls, gltf: { blobs: map }, export: info.export, data: doc };
  return { glb, sidecar, details };
}

// Recovers every blob from the .glb as directed by sidecar.gltf.blobs.
function blobsFromGlb(sidecar, glbBytes) {
  const g = readGlb(glbBytes);
  const doc = sidecar.data, blobs = {};
  for (const [id, m] of Object.entries(sidecar.gltf.blobs)) {
    if (m.kind === 'raw') blobs[id] = g.viewBytes(m.view);
    else if (m.kind === 'accessor') blobs[id] = g.accessorBytes(m.accessor);
    else if (m.kind === 'colors_bgra') blobs[id] = swapRB(g.accessorBytes(m.accessor));
    else if (m.kind === 'indices') blobs[id] = m.windingSwapped ? swapWinding(g.viewBytes(m.view), m.indexSize) : g.viewBytes(m.view);
    else if (m.kind === 'static_vertices' || m.kind === 'skeletal_vertices') {
      const out = Buffer.alloc(m.count * m.stride);
      const skel = m.kind === 'skeletal_vertices';
      const w = m.wideTangents ? 8 : 4;
      setColumn(out, g.accessorBytes(m.tangentX), m.count, m.stride, 0, w);
      setColumn(out, g.accessorBytes(m.tangentZ), m.count, m.stride, w, w);
      if (skel) setColumn(out, g.accessorBytes(m.position), m.count, m.stride, 8, 12);
      const uv0 = skel ? 20 : 2 * w;
      m.texcoords.forEach((a, k) => encodeUVs(out, g.accessorBytes(a), m.count, m.stride, uv0 + k * (m.fullPrecisionUVs ? 8 : 4), m.fullPrecisionUVs));
      blobs[id] = out;
    } else if (m.kind === 'skin_weights') blobs[id] = encodeWeights(m.joints.map(g.accessorBytes), m.weights.map(g.accessorBytes), m.count, m.influences, doc.lods[m.lod].sections);
    else if (m.kind === 'node_trs') blobs[id] = nodesToPose(g.json, m.firstNode, m.count, m.overrides);
    else throw new Error('unknown blob kind ' + m.kind);
  }
  return blobs;
}

// Reverse direction: the export's exact bytes from the sidecar, the .glb and the package name table.
function rebuild(pkg, sidecar, glbBytes) {
  if (sidecar.sidecar_version !== SIDECAR_VERSION) throw new Error('unknown sidecar version');
  if (!SERIALIZERS[sidecar.class]) throw new Error('unknown class ' + sidecar.class);
  return serialize(pkg, sidecar.class, sidecar.data, blobsFromGlb(sidecar, glbBytes));
}

module.exports = {
  SIDECAR_VERSION, CLASSES: Object.keys(SERIALIZERS), GL, UE_TO_GLTF, H2F, floatBitsToHalf, floatVal,
  parseExport, serialize, toGlb, blobsFromGlb, rebuild, readGlb, GlbBuilder, objectPath, poseToNodes,
};
