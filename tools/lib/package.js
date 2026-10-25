// Generic UE 4.17 cooked package decoder: .uasset + .uexp  <->  one JSON document.
//
//   decode(uasset, uexp, opts) -> doc      encode(doc, opts) -> { uasset, uexp }
//
// Every export is: tagged properties, the object-guid flag (non-CDO objects), then the class's
// native data. Native data is described per class by a codec (reader + writer from one
// description, see `T`) registered in tools/lib/pkg_classes/. An export whose class has no codec,
// or whose codec does not consume the export exactly, keeps its native bytes as
// { undecoded: hex } - encode() is byte-exact either way and stats() reports those bytes.
// Docs: docs/formats/package_json.md.
const ua = require('./uasset');

const RF_ClassDefaultObject = 0x10;

// ------------------------------------------------------------------ reader / writer

class R extends ua.Reader {
  i8() { const v = this.b.readInt8(this.p); this.p += 1; return v; }
  i16() { const v = this.b.readInt16LE(this.p); this.p += 2; return v; }
  f64() { const v = this.b.readDoubleLE(this.p); this.p += 8; return v; }
  need(n) { if (n < 0 || this.p + n > this.b.length) throw new Error(`need ${n} bytes at ${this.p}, have ${this.b.length - this.p}`); }
  get left() { return this.b.length - this.p; }
}
for (const m of ['i32', 'u32', 'u16', 'u8', 'f32']) {
  const size = { i32: 4, u32: 4, u16: 2, u8: 1, f32: 4 }[m];
  const base = ua.Reader.prototype[m];
  R.prototype[m] = function () { if (this.p + size > this.b.length) throw new Error(`read past end at ${this.p}`); return base.call(this); };
}
R.prototype.bytes = function (n) { this.need(n); const v = this.b.subarray(this.p, this.p + n); this.p += n; return v; };
R.prototype.fstring = function () {
  const len = this.i32();
  if (len === 0) return { s: '', enc: 'empty' };
  if (len > 0) { this.need(len); if (this.b[this.p + len - 1] !== 0) throw new Error('FString not terminated'); const s = this.b.toString('latin1', this.p, this.p + len - 1); this.p += len; return { s, enc: 'ansi' }; }
  if (len < -0x4000000) throw new Error('bad FString length');
  this.need(-len * 2);
  if (this.b.readUInt16LE(this.p - len * 2 - 2) !== 0) throw new Error('FString not terminated');
  const raw = this.b.subarray(this.p, this.p - len * 2 - 2);
  const s = raw.toString('utf16le');
  this.p += -len * 2;
  // lone surrogates do not survive JSON/UTF-8: keep those strings as hex
  if (!Buffer.from(s, 'utf16le').equals(raw)) return { hex16: raw.toString('hex'), enc: 'utf16' };
  return { s, enc: 'utf16' };
};

// Growable writer (same method names as uasset.Writer).
class W {
  constructor(n = 1 << 12) { this.b = Buffer.allocUnsafe(n); this.len = 0; }
  grow(n) { if (this.len + n > this.b.length) { const nb = Buffer.allocUnsafe(Math.max(this.b.length * 2, this.len + n)); this.b.copy(nb, 0, 0, this.len); this.b = nb; } }
  push(b) { this.grow(b.length); b.copy(this.b, this.len); this.len += b.length; return this; }
  i32(v) { this.grow(4); this.b.writeInt32LE(v, this.len); this.len += 4; return this; }
  u32(v) { this.grow(4); this.b.writeUInt32LE(v >>> 0, this.len); this.len += 4; return this; }
  u16(v) { this.grow(2); this.b.writeUInt16LE(v, this.len); this.len += 2; return this; }
  i16(v) { this.grow(2); this.b.writeInt16LE(v, this.len); this.len += 2; return this; }
  u8(v) { this.grow(1); this.b[this.len++] = v; return this; }
  i8(v) { this.grow(1); this.b.writeInt8(v, this.len); this.len += 1; return this; }
  i64(v) { this.grow(8); this.b.writeBigInt64LE(BigInt(v), this.len); this.len += 8; return this; }
  u64(v) { this.grow(8); this.b.writeBigUInt64LE(BigInt(v), this.len); this.len += 8; return this; }
  f32(v) { this.grow(4); this.b.writeFloatLE(v, this.len); this.len += 4; return this; }
  f64(v) { this.grow(8); this.b.writeDoubleLE(v, this.len); this.len += 8; return this; }
  hex(h) { const n = h.length / 2; this.grow(n); this.b.write(h, this.len, n, 'hex'); this.len += n; return this; }
  fstring(x) {
    if (x.enc === 'empty') return this.i32(0);
    if (x.enc === 'ansi') { this.i32(x.s.length + 1); return this.push(Buffer.from(x.s + '\0', 'latin1')); }
    if (x.hex16 !== undefined) { this.i32(-(x.hex16.length / 4 + 1)); return this.hex(x.hex16).u16(0); }
    this.i32(-(x.s.length + 1));
    return this.push(Buffer.from(x.s + '\0', 'utf16le'));
  }
  concat() { return Buffer.from(this.b.subarray(0, this.len)); }
}

// ------------------------------------------------------------------ names

function nameIndex(pkg, s) {
  if (!pkg.nameLookup) { pkg.nameLookup = new Map(); pkg.names.forEach((n, i) => { if (!pkg.nameLookup.has(n.s)) pkg.nameLookup.set(n.s, i); }); }
  if (typeof s === 'object') return s;
  const hit = pkg.nameLookup.get(s);
  if (hit !== undefined) return { index: hit, number: 0 };
  const m = /^(.*)_(\d+)$/.exec(s);
  if (m && pkg.nameLookup.has(m[1]) && String(+m[2]) === m[2]) return { index: pkg.nameLookup.get(m[1]), number: +m[2] + 1 };
  throw new Error('name not in package name table: ' + s);
}
// FName -> string when the string maps back to the same (index, number); else the raw pair.
function nameStr(pkg, index, number) {
  const n = pkg.names[index];
  if (!n || index < 0) throw new Error(`bad name index ${index}`);
  if (number < 0) throw new Error('bad name number ' + number);
  if (!pkg.nameCanon) {
    // a name string is canonical for index i when no earlier entry has the same text and the
    // text itself cannot be read as "Base_N" of another entry
    if (!pkg.nameLookup) nameIndex(pkg, 'None');
    pkg.nameCanon = pkg.names.map((x, i) => {
      if (x.s === undefined || pkg.nameLookup.get(x.s) !== i) return false;
      return true;
    });
  }
  if (!pkg.nameCanon[index]) return { index, number };
  if (number === 0) return n.s;
  const s = `${n.s}_${number - 1}`;
  if (pkg.nameLookup.has(s)) return { index, number };
  return s;
}

// ------------------------------------------------------------------ codecs

const fbits = Buffer.alloc(8);
function f32out(r) {
  const bits = r.b.readUInt32LE(r.p);
  const f = r.f32();
  if (f === 0 ? bits === 0 : Number.isFinite(f)) return f;
  return { f32bits: bits };
}
function f32in(w, v) { if (typeof v === 'object') w.u32(v.f32bits); else w.f32(v); }
function f64out(r) {
  r.need(8);
  const lo = r.b.readUInt32LE(r.p), hi = r.b.readUInt32LE(r.p + 4);
  const f = r.f64();
  if (f === 0 ? (lo === 0 && hi === 0) : Number.isFinite(f)) return f;
  return { f64bits: [lo, hi] };
}
function f64in(w, v) { if (typeof v === 'object') { w.u32(v.f64bits[0]); w.u32(v.f64bits[1]); } else w.f64(v); }
void fbits;

const strOut = (x) => (x.enc === 'ansi' && x.s.length > 0 ? x.s : x);
const strIn = (x) => (typeof x === 'string' ? { s: x, enc: 'ansi' } : x);

const T = {};
T.i32 = { r: (r) => r.i32(), w: (w, v) => w.i32(v) };
T.u32 = { r: (r) => r.u32(), w: (w, v) => w.u32(v) };
T.u16 = { r: (r) => r.u16(), w: (w, v) => w.u16(v) };
T.i16 = { r: (r) => r.i16(), w: (w, v) => w.i16(v) };
T.u8 = { r: (r) => r.u8(), w: (w, v) => w.u8(v) };
T.i8 = { r: (r) => r.i8(), w: (w, v) => w.i8(v) };
T.i64 = { r: (r) => { r.need(8); const v = r.b.readBigInt64LE(r.p); r.p += 8; return v >= -9007199254740991n && v <= 9007199254740991n ? Number(v) : String(v); }, w: (w, v) => w.i64(BigInt(v)) };
T.u64 = { r: (r) => { r.need(8); const v = r.b.readBigUInt64LE(r.p); r.p += 8; return v <= 9007199254740991n ? Number(v) : String(v); }, w: (w, v) => w.u64(BigInt(v)) };
T.f32 = { r: f32out, w: f32in };
T.f64 = { r: f64out, w: f64in };
// UE bool serialized as a 32-bit value that must be 0 or 1
T.b32 = { r: (r) => { const v = r.u32(); if (v > 1) throw new Error('bool32 = ' + v + ' at ' + (r.p - 4)); return v === 1; }, w: (w, v) => w.u32(v ? 1 : 0) };
T.b8 = { r: (r) => { const v = r.u8(); if (v > 1) throw new Error('bool8 = ' + v + ' at ' + (r.p - 1)); return v === 1; }, w: (w, v) => w.u8(v ? 1 : 0) };
T.name = { r: (r, c) => { if (c.namesAsStrings) return T.str.r(r, c); const i = r.i32(); if (i < 0 || i >= c.pkg.names.length) throw new Error(`bad name index ${i} at ${r.p - 4}`); return nameStr(c.pkg, i, r.i32()); }, w: (w, v, c) => { if (c.namesAsStrings) { T.str.w(w, v, c); return; } const f = nameIndex(c.pkg, v); w.i32(f.index).i32(f.number); } };
// package index: >0 export, <0 import, 0 null. Range-checked so a misparse fails early.
T.obj = {
  r: (r, c) => { const v = r.i32(); if (v > c.pkg.exports.length || -v > c.pkg.imports.length) throw new Error(`bad object index ${v} at ${r.p - 4}`); return v; },
  w: (w, v) => w.i32(v),
};
T.str = { r: (r) => strOut(r.fstring()), w: (w, v) => w.fstring(strIn(v)) };
T.guid = { r: (r) => r.bytes(16).toString('hex'), w: (w, v) => w.hex(v) };
T.hex = (n) => ({ r: (r) => r.bytes(n).toString('hex'), w: (w, v) => { if (v.length !== n * 2) throw new Error('hex length'); w.hex(v); } });
T.rest = { r: (r) => r.bytes(r.left).toString('hex'), w: (w, v) => w.hex(v) };
// constant: a field that always holds one value (asserted on read, not stored)
T.constant = (codec, value) => ({ r: (r, c) => { const at = r.p; const v = codec.r(r, c); if (JSON.stringify(v) !== JSON.stringify(value)) throw new Error(`expected constant ${JSON.stringify(value)}, got ${JSON.stringify(v)} at ${at}`); return undefined; }, w: (w, v, c) => codec.w(w, value, c), constant: true });
const MAX_COUNT = 0x8000000;
function count(r, min = 1) { const n = r.i32(); if (n < 0 || n > MAX_COUNT || n * min > r.left) throw new Error(`bad array count ${n} at ${r.p - 4}`); return n; }
// TArray<T>: int32 count + elements
T.arr = (codec, min = 1) => ({
  r: (r, c) => { const n = count(r, min); const a = new Array(n); for (let i = 0; i < n; i++) a[i] = codec.r(r, c); return a; },
  w: (w, v, c) => { w.i32(v.length); for (const x of v) codec.w(w, x, c); },
});
// fixed-count sequence
T.fixed = (codec, n) => ({
  r: (r, c) => { const a = new Array(n); for (let i = 0; i < n; i++) a[i] = codec.r(r, c); return a; },
  w: (w, v, c) => { if (v.length !== n) throw new Error('fixed length'); for (const x of v) codec.w(w, x, c); },
});
// TArray<uint8> as hex
T.bytes = { r: (r) => { const n = count(r); return r.bytes(n).toString('hex'); }, w: (w, v) => { w.i32(v.length / 2); w.hex(v); } };
// TArray::BulkSerialize: int32 element size, int32 count, elements
T.bulkArr = (codec, elemSize) => ({
  r: (r, c) => { const s = r.i32(); if (s !== elemSize) throw new Error(`bulk array element size ${s}, expected ${elemSize} at ${r.p - 4}`); const n = count(r, elemSize); const a = new Array(n); for (let i = 0; i < n; i++) a[i] = codec.r(r, c); return a; },
  w: (w, v, c) => { w.i32(elemSize); w.i32(v.length); for (const x of v) codec.w(w, x, c); },
});
// TArray::BulkSerialize of fixed-size elements kept as one hex string (large homogeneous arrays
// whose element layout is documented with the class): int32 element size, int32 count, data
T.packedBulk = (elemSize) => ({
  r: (r) => { const s = r.i32(); if (s !== elemSize) throw new Error(`bulk array element size ${s}, expected ${elemSize} at ${r.p - 4}`); const n = count(r, elemSize); return r.bytes(n * elemSize).toString('hex'); },
  w: (w, v) => { w.i32(elemSize); w.i32(v.length / 2 / elemSize); w.hex(v); },
});
// TArray<T> of fixed-size elements kept as one hex string: int32 count, data
T.packed = (elemSize) => ({
  r: (r) => { const n = count(r, elemSize); return r.bytes(n * elemSize).toString('hex'); },
  w: (w, v) => { w.i32(v.length / 2 / elemSize); w.hex(v); },
});
// A payload in a named third-party format, kept verbatim. Counted separately from
// uninterpreted bytes (stats.thirdParty[format]).
T.foreign = (format, lenCodec) => ({
  r: (r, c) => { const n = lenCodec ? lenCodec.r(r, c) : r.left; if (n < 0) throw new Error('foreign length'); const b = r.bytes(n); if (c.stats) { c.stats.thirdParty = c.stats.thirdParty || {}; c.stats.thirdParty[format] = (c.stats.thirdParty[format] || 0) + n; } return b.toString('hex'); },
  w: (w, v, c) => { if (lenCodec) lenCodec.w(w, v.length / 2, c); w.hex(v); },
});
// Run a codec over a length-delimited sub-range that it must consume exactly
T.sized = (lenCodec, codec) => ({
  r: (r, c) => { const n = lenCodec.r(r, c); const sub = new R(r.bytes(n)); const v = codec.r(sub, c); if (sub.left) throw new Error(`sized block: ${sub.left} of ${n} bytes left`); return v; },
  w: (w, v, c) => { const sub = new W(256); codec.w(sub, v, c); lenCodec.w(w, sub.len, c); w.push(sub.b.subarray(0, sub.len)); },
});
// struct: [[name, codec | (obj, ctx) => codec|null], ...]; a null codec skips the field
T.struct = (fields) => ({
  r: (r, c) => {
    const o = {};
    for (const [k, f] of fields) {
      const codec = typeof f === 'function' ? f(o, c) : f;
      if (!codec) continue;
      const v = codec.r(r, c, o);
      if (!codec.constant) o[k] = v;
    }
    return o;
  },
  w: (w, o, c) => {
    for (const [k, f] of fields) {
      const codec = typeof f === 'function' ? f(o, c) : f;
      if (!codec) continue;
      if (!codec.constant && o[k] === undefined) throw new Error('missing field ' + k);
      codec.w(w, o[k], c, o);
    }
  },
});
T.tuple = (keys, codec) => T.struct([...keys].map((k) => [k, codec]));
T.vec2 = T.tuple('xy', T.f32);
T.vec3 = T.tuple('xyz', T.f32);
T.vec4 = T.tuple('xyzw', T.f32);
T.quat = T.vec4;
T.rot = T.struct([['pitch', T.f32], ['yaw', T.f32], ['roll', T.f32]]);
T.color = T.tuple('bgra', T.u8);
T.linearColor = T.tuple('rgba', T.f32);
T.box = T.struct([['min', T.vec3], ['max', T.vec3], ['isValid', T.u8]]);
T.sphere = T.struct([['center', T.vec3], ['w', T.f32]]);
T.boxSphereBounds = T.struct([['origin', T.vec3], ['boxExtent', T.vec3], ['sphereRadius', T.f32]]);
T.intPoint = T.tuple('xy', T.i32);
T.intVector = T.tuple('xyz', T.i32);
T.transform = T.struct([['rotation', T.quat], ['translation', T.vec3], ['scale3D', T.vec3]]);
// FStripDataFlags: two bytes
T.strip = T.struct([['global', T.u8], ['cls', T.u8]]);
// TMap<K,V>: int32 count + pairs
T.map = (k, v, min = 2) => T.arr(T.struct([['key', k], ['value', v]]), min);
// optional by predicate
T.when = (pred, codec) => (o, c) => (pred(o, c) ? codec : null);
// lazily resolved codec (recursive structures)
T.lazy = (fn) => ({ r: (r, c, o) => fn().r(r, c, o), w: (w, v, c, o) => fn().w(w, v, c, o) });

// FByteBulkData header (+ inline payload). Offsets are positions in the virtual file
// "<.uasset><.uexp>" for inline payloads, or relative to BulkDataStartOffset for .ubulk payloads.
const BULK = { PayloadAtEndOfFile: 0x1, ForceInlinePayload: 0x40, PayloadInSeperateFile: 0x100, Unused: 0x20 };
// payload: codec for an inline payload (must consume it exactly); default keeps it as hex `data`.
// For an inline payload, offset == TotalHeaderSize + position of the payload in the .uexp; a
// header that breaks this rule is marked offsetRule: false (the stored offset is always kept).
T.bulkOf = (payload) => ({
  r: (r, c) => {
    const b = { flags: r.u32(), elementCount: r.i32(), sizeOnDisk: r.i32(), offset: T.i64.r(r) };
    if (b.flags & ~0x5ff) throw new Error('bulk flags 0x' + b.flags.toString(16));
    if (b.flags & 0x202) throw new Error('compressed bulk data');
    const inline = !(b.flags & BULK.PayloadInSeperateFile) && !(b.flags & BULK.PayloadAtEndOfFile);
    if (inline) {
      if (b.sizeOnDisk < 0 || b.sizeOnDisk !== b.elementCount) throw new Error('bulk size');
      if (b.offset !== c.fileOffset(r.p)) b.offsetRule = false;
      if (payload) { const sub = new R(r.bytes(b.sizeOnDisk)); b.payload = payload.r(sub, c); if (sub.left) throw new Error(`bulk payload: ${sub.left} bytes left`); }
      else b.data = r.bytes(b.sizeOnDisk).toString('hex');
    }
    return b;
  },
  w: (w, b, c) => {
    w.u32(b.flags).i32(b.elementCount).i32(b.sizeOnDisk); T.i64.w(w, b.offset);
    if (b.payload !== undefined) payload.w(w, b.payload, c); else if (b.data !== undefined) w.hex(b.data);
  },
});
T.bulk = T.bulkOf(null);
// FFormatContainer: int32 count + { FName format, FByteBulkData }. payloads: format name -> codec
T.formatContainer = (payloads) => T.arr({
  r: (r, c) => { const format = T.name.r(r, c); const p = payloads[format]; if (!p) throw new Error('format container: unknown format ' + JSON.stringify(format)); return { format, bulk: T.bulkOf(p).r(r, c) }; },
  w: (w, v, c) => { T.name.w(w, v.format, c); T.bulkOf(payloads[v.format]).w(w, v.bulk, c); },
}, 28);

// ------------------------------------------------------------------ FText

const TEXT_HISTORY = {
  '-1': () => T.struct([]),
  0: () => T.struct([['namespace', T.str], ['key', T.str], ['source', T.str]]),
};
T.text = {
  r: (r, c) => {
    const t = { flags: r.u32(), history: r.i8() };
    const h = TEXT_HISTORY[t.history];
    if (!h) throw new Error('FText history type ' + t.history);
    Object.assign(t, h().r(r, c));
    return t;
  },
  w: (w, t, c) => { w.u32(t.flags).i8(t.history); TEXT_HISTORY[t.history]().w(w, t, c); },
};

// ------------------------------------------------------------------ tagged properties

// Structs serialized natively (no property tags) when they are the value of a StructProperty.
const NATIVE_STRUCTS = {
  Vector: T.vec3, Vector2D: T.vec2, Vector4: T.vec4, Rotator: T.rot, Quat: T.quat, Plane: T.vec4,
  LinearColor: T.linearColor, Color: T.color, IntPoint: T.intPoint, IntVector: T.intVector, Guid: T.guid,
  Box: T.box, Box2D: T.struct([['min', T.vec2], ['max', T.vec2], ['isValid', T.u8]]),
  TwoVectors: T.struct([['v1', T.vec3], ['v2', T.vec3]]),
  DateTime: T.i64, Timespan: T.i64,
  StringAssetReference: T.str, StringClassReference: T.str,
  RichCurveKey: T.struct([['interpMode', T.u8], ['tangentMode', T.u8], ['tangentWeightMode', T.u8], ['time', T.f32], ['value', T.f32], ['arriveTangent', T.f32], ['arriveTangentWeight', T.f32], ['leaveTangent', T.f32], ['leaveTangentWeight', T.f32]]),
};

const tagMeta = (r, c) => {
  const pkg = c.pkg;
  const name = nameStr(pkg, r.i32(), r.i32());
  if (name === 'None') return null;
  const type = nameStr(pkg, r.i32(), r.i32());
  if (typeof type !== 'string') throw new Error('property type is not a plain name');
  const t = { name, type, size: r.i32(), arrayIndex: r.i32() };
  if (type === 'StructProperty') { t.structName = nameStr(pkg, r.i32(), r.i32()); t.structGuid = r.bytes(16).toString('hex'); }
  else if (type === 'BoolProperty') t.boolVal = r.u8();
  else if (type === 'ByteProperty' || type === 'EnumProperty') t.enumName = nameStr(pkg, r.i32(), r.i32());
  else if (type === 'ArrayProperty' || type === 'SetProperty') t.innerType = nameStr(pkg, r.i32(), r.i32());
  else if (type === 'MapProperty') { t.innerType = nameStr(pkg, r.i32(), r.i32()); t.valueType = nameStr(pkg, r.i32(), r.i32()); }
  const hasGuid = r.u8();
  if (hasGuid > 1) throw new Error('property guid flag ' + hasGuid);
  if (hasGuid) t.guid = r.bytes(16).toString('hex');
  return t;
};
function writeTag(w, c, t, size) {
  T.name.w(w, t.name, c); T.name.w(w, t.type, c); w.i32(size).i32(t.arrayIndex || 0);
  if (t.type === 'StructProperty') { T.name.w(w, t.structName, c); w.hex(t.structGuid || '00000000000000000000000000000000'); }
  else if (t.type === 'BoolProperty') w.u8(t.value ? 1 : 0);
  else if (t.type === 'ByteProperty' || t.type === 'EnumProperty') T.name.w(w, t.enumName, c);
  else if (t.type === 'ArrayProperty' || t.type === 'SetProperty') T.name.w(w, t.innerType, c);
  else if (t.type === 'MapProperty') { T.name.w(w, t.innerType, c); T.name.w(w, t.valueType, c); }
  if (t.guid) { w.u8(1); w.hex(t.guid); } else w.u8(0);
}

const SIMPLE = {
  IntProperty: T.i32, UInt32Property: T.u32, FloatProperty: T.f32, DoubleProperty: T.f64, NameProperty: T.name, EnumProperty: T.name,
  ObjectProperty: T.obj, ClassProperty: T.obj, WeakObjectProperty: T.obj, InterfaceProperty: T.obj,
  StrProperty: T.str, AssetObjectProperty: T.str, AssetClassProperty: T.str,
  Int8Property: T.i8, Int16Property: T.i16, UInt16Property: T.u16, Int64Property: T.i64, UInt64Property: T.u64,
  TextProperty: T.text, LazyObjectProperty: T.guid,
  DelegateProperty: T.struct([['object', T.obj], ['function', T.name]]),
  MulticastDelegateProperty: T.arr(T.struct([['object', T.obj], ['function', T.name]]), 12),
};

// How a struct value is serialized: native codec by name, else a tagged property list.
function structCodec(structName, c) {
  const n = NATIVE_STRUCTS[structName];
  if (n) return n;
  const x = c.structCodecs && c.structCodecs[structName];
  if (x) return x;
  return T.propsStruct;
}
// Element (no tag) of an array/set/map. `variant` picks between layouts the tag cannot tell apart.
function elemCodec(type, c, variant, structName) {
  if (type === 'StructProperty') return structName ? structCodec(structName, c) : (variant.struct ? NATIVE_STRUCTS[variant.struct] : T.propsStruct);
  if (type === 'ByteProperty') return variant.byteAsName ? T.name : T.u8;
  if (type === 'BoolProperty') return T.u8;
  const s = SIMPLE[type];
  if (!s) throw new Error('no element codec for ' + type);
  return s;
}

function readValue(r, c, t, end) {
  const type = t.type;
  if (type === 'BoolProperty') return t.boolVal === 1 ? true : t.boolVal === 0 ? false : (() => { throw new Error('bool tag value'); })();
  if (type === 'ByteProperty') {
    if (t.size === 1) return r.u8();
    if (t.size === 8) return T.name.r(r, c);
    throw new Error('ByteProperty size ' + t.size);
  }
  if (type === 'StructProperty') return structCodec(t.structName, c).r(r, c);
  if (type === 'ArrayProperty') {
    const n = count(r, 0);
    if (t.innerType === 'StructProperty') {
      const inner = tagMeta(r, c);
      if (!inner || inner.type !== 'StructProperty') throw new Error('array inner tag');
      const codec = structCodec(inner.structName, c);
      const items = new Array(n);
      const s0 = r.p;
      for (let i = 0; i < n; i++) items[i] = codec.r(r, c);
      if (r.p - s0 !== inner.size) throw new Error('array inner size');
      const v = { structName: inner.structName, items };
      if (inner.name !== t.name) v.innerName = inner.name;
      if (inner.structGuid !== '00000000000000000000000000000000') v.structGuid = inner.structGuid;
      if (inner.arrayIndex) v.innerArrayIndex = inner.arrayIndex;
      if (inner.guid) v.innerGuid = inner.guid;
      return v;
    }
    let variant = {};
    if (t.innerType === 'ByteProperty' && n > 0 && end - r.p === n * 8 ) variant = { byteAsName: true };
    if (t.innerType === 'ByteProperty' && !variant.byteAsName) return { bytes: r.bytes(n).toString('hex') };
    const codec = elemCodec(t.innerType, c, variant);
    const items = new Array(n);
    for (let i = 0; i < n; i++) items[i] = codec.r(r, c);
    return variant.byteAsName ? { items, byteAsName: true } : items;
  }
  if (type === 'SetProperty' || type === 'MapProperty') {
    // the tag does not say how struct or byte elements are laid out: try the candidates until
    // one consumes the value exactly
    const start = r.p;
    const kv = type === 'MapProperty' ? [t.innerType, t.valueType] : [t.innerType];
    const options = (ty) => (ty === 'StructProperty' ? [{}, { struct: 'Guid' }, { struct: 'Vector' }, { struct: 'IntPoint' }, { struct: 'Vector2D' }, { struct: 'LinearColor' }, { struct: 'StringAssetReference' }, { struct: 'Rotator' }, { struct: 'Color' }, { struct: 'IntVector' }] : ty === 'ByteProperty' ? [{}, { byteAsName: true }] : [{}]);
    let lastErr;
    for (const kOpt of options(kv[0])) {
      for (const vOpt of (kv[1] ? options(kv[1]) : [null])) {
        r.p = start;
        try {
          const kc = elemCodec(kv[0], c, kOpt), vc = kv[1] ? elemCodec(kv[1], c, vOpt) : null;
          const nRemove = count(r, 1);
          const remove = [];
          for (let i = 0; i < nRemove; i++) remove.push(kc.r(r, c));
          const n = count(r, 1);
          const entries = [];
          for (let i = 0; i < n; i++) entries.push(vc ? { key: kc.r(r, c), value: vc.r(r, c) } : kc.r(r, c));
          if (r.p !== end) throw new Error('map/set did not consume its value');
          const v = { entries };
          if (remove.length) v.remove = remove;
          if (Object.keys(kOpt).length) v.keyLayout = kOpt;
          if (vOpt && Object.keys(vOpt).length) v.valueLayout = vOpt;
          return v;
        } catch (e) { lastErr = e; }
      }
    }
    throw lastErr;
  }
  const s = SIMPLE[type];
  if (!s) throw new Error('no codec for property type ' + type);
  return s.r(r, c);
}

function writeValue(w, c, t, v) {
  const type = t.type;
  if (type === 'BoolProperty') return;
  if (type === 'ByteProperty') { if (typeof v === 'number') w.u8(v); else T.name.w(w, v, c); return; }
  if (type === 'StructProperty') { structCodec(t.structName, c).w(w, v, c); return; }
  if (type === 'ArrayProperty') {
    if (t.innerType === 'StructProperty') {
      w.i32(v.items.length);
      const codec = structCodec(v.structName, c);
      const sub = new W();
      for (const it of v.items) codec.w(sub, it, c);
      writeTag(w, c, { name: v.innerName !== undefined ? v.innerName : t.name, type: 'StructProperty', structName: v.structName, structGuid: v.structGuid, arrayIndex: v.innerArrayIndex, guid: v.innerGuid }, sub.len);
      w.push(sub.b.subarray(0, sub.len));
      return;
    }
    if (v.bytes !== undefined) { w.i32(v.bytes.length / 2); w.hex(v.bytes); return; }
    const items = Array.isArray(v) ? v : v.items;
    const codec = elemCodec(t.innerType, c, Array.isArray(v) ? {} : v);
    w.i32(items.length);
    for (const it of items) codec.w(w, it, c);
    return;
  }
  if (type === 'SetProperty' || type === 'MapProperty') {
    const kc = elemCodec(t.innerType, c, v.keyLayout || {});
    const vc = type === 'MapProperty' ? elemCodec(t.valueType, c, v.valueLayout || {}) : null;
    const remove = v.remove || [];
    w.i32(remove.length);
    for (const k of remove) kc.w(w, k, c);
    w.i32(v.entries.length);
    for (const e of v.entries) { if (vc) { kc.w(w, e.key, c); vc.w(w, e.value, c); } else kc.w(w, e, c); }
    return;
  }
  SIMPLE[type].w(w, v, c);
}

// Tagged property list up to None. A value that does not decode to exactly its tagged size is
// kept as { raw } and counted in c.stats.rawPropertyBytes.
T.props = {
  r: (r, c) => {
    const out = [];
    for (;;) {
      const t = tagMeta(r, c);
      if (!t) return out;
      if (t.size < 0 || r.p + t.size > r.b.length) throw new Error(`property ${JSON.stringify(t.name)} size ${t.size} at ${r.p} runs past the data`);
      const start = r.p, end = start + t.size;
      const p = { name: t.name, type: t.type };
      if (t.arrayIndex) p.arrayIndex = t.arrayIndex;
      if (t.structName !== undefined) { p.structName = t.structName; if (t.structGuid !== '00000000000000000000000000000000') p.structGuid = t.structGuid; }
      if (t.enumName !== undefined) p.enumName = t.enumName;
      if (t.innerType !== undefined) p.innerType = t.innerType;
      if (t.valueType !== undefined) p.valueType = t.valueType;
      if (t.guid) p.guid = t.guid;
      let v;
      try {
        // limit nested reads to this value
        const sub = new R(r.b.subarray(0, end), start);
        v = readValue(sub, c, t, end);
        if (sub.p !== end) throw new Error(`value decoded ${sub.p - start} of ${t.size} bytes`);
      } catch (e) {
        v = undefined;
        if (c.stats) { c.stats.rawPropertyBytes += t.size; const k = `${t.type}${t.structName ? ':' + t.structName : ''}${t.innerType ? ':' + t.innerType : ''}${t.valueType ? ':' + t.valueType : ''}`; c.stats.rawKinds[k] = (c.stats.rawKinds[k] || 0) + t.size; }
      }
      r.p = end;
      if (v === undefined) p.raw = r.b.toString('hex', start, end); else p.value = v;
      out.push(p);
    }
  },
  w: (w, props, c) => {
    for (const p of props) {
      if (p.raw !== undefined) { writeTag(w, c, p, p.raw.length / 2); w.hex(p.raw); continue; }
      const sub = new W(64);
      writeValue(sub, c, p, p.value);
      writeTag(w, c, p, sub.len);
      w.push(sub.b.subarray(0, sub.len));
    }
    T.name.w(w, 'None', c);
  },
};
// A struct value that is a tagged list. Inside a value a failure must propagate (no size to fall back on).
T.propsStruct = {
  r: (r, c) => {
    // quick reject: the first name must be a valid index, else this is a native struct we do not know
    r.need(8);
    const idx = r.b.readInt32LE(r.p);
    if (idx < 0 || idx >= c.pkg.names.length) throw new Error('not a tagged struct');
    const saved = c.stats;
    const local = { rawPropertyBytes: 0, rawKinds: {} };
    c.stats = local;
    let v;
    try { v = T.props.r(r, c); } finally { c.stats = saved; }
    if (saved) { saved.rawPropertyBytes += local.rawPropertyBytes; for (const k in local.rawKinds) saved.rawKinds[k] = (saved.rawKinds[k] || 0) + local.rawKinds[k]; }
    return v;
  },
  w: (w, v, c) => T.props.w(w, v, c),
};

// ------------------------------------------------------------------ class registry

const CLASSES = new Map(); // class name -> { codec, family }
const STRUCTS = {}; // extra native structs registered by class modules
function register(names, codec, family) { for (const n of [].concat(names)) CLASSES.set(n, { codec, family }); }
function registerStruct(name, codec) { STRUCTS[name] = codec; }
// Classes whose native data is decoded by another tool of this project; their export bytes are
// not embedded in the JSON (see `delegated`).
const DELEGATED = new Map();
function delegate(names, tool) { for (const n of [].concat(names)) DELEGATED.set(n, tool); }

let BASES = null; // blueprint class name -> native base (audit/blueprint_class_bases.json)
function setBases(b) { BASES = b; }

function classOfExport(pkg, e) {
  const idx = e.classIndex;
  if (idx === 0) return { name: '(UClass)', native: true };
  if (idx > 0) return { name: ua.nameOf(pkg, pkg.exports[idx - 1].objectName), native: false };
  const imp = pkg.imports[-idx - 1];
  let outer = imp;
  while (outer.outerIndex < 0) outer = pkg.imports[-outer.outerIndex - 1];
  const from = outer === imp ? '' : ua.nameOf(pkg, outer.objectName);
  return { name: ua.nameOf(pkg, imp.objectName), native: from.startsWith('/Script/'), from };
}
// Native class whose serializer applies to an export.
function nativeClass(pkg, e) {
  const c = classOfExport(pkg, e);
  if (c.native) return c.name;
  if (e.classIndex > 0) {
    // class exported by this package: follow its super chain
    let x = pkg.exports[e.classIndex - 1];
    for (let depth = 0; depth < 32; depth++) {
      const s = x.superIndex;
      if (s === 0) return null;
      if (s > 0) { x = pkg.exports[s - 1]; continue; }
      const imp = pkg.imports[-s - 1];
      let outer = imp;
      while (outer.outerIndex < 0) outer = pkg.imports[-outer.outerIndex - 1];
      const nm = ua.nameOf(pkg, imp.objectName);
      if (ua.nameOf(pkg, outer.objectName).startsWith('/Script/')) return nm;
      return BASES && BASES[nm] || null;
    }
    return null;
  }
  return BASES && BASES[c.name] || null;
}

// ------------------------------------------------------------------ package

function decodeHeaderTail(pkg, buf) {
  // depends map: per export a TArray<FPackageIndex>; then the asset registry data; then the
  // preload dependency indexes. Anything not covered stays in gaps (counted as uninterpreted).
  const s = pkg.summary;
  const r = new R(buf, pkg.exportsEnd);
  const out = { uninterpreted: 0 };
  const gap = (to, key) => { if (to < r.p) throw new Error('header tables overlap at ' + key); if (to > r.p) { out[key] = buf.toString('hex', r.p, to); out.uninterpreted += to - r.p; r.p = to; } };
  gap(s.dependsOffset, 'gapBeforeDepends');
  out.depends = [];
  for (let i = 0; i < s.exportCount; i++) out.depends.push(T.arr(T.i32, 4).r(r, {}));
  if (s.assetRegistryDataOffset > 0) {
    gap(s.assetRegistryDataOffset, 'gapBeforeAssetRegistry');
    out.assetRegistryCount = r.i32();
    if (out.assetRegistryCount !== 0) throw new Error('asset registry data present');
  }
  if (s.preloadDependencyCount > 0 || s.preloadDependencyOffset > 0) {
    gap(s.preloadDependencyOffset, 'gapBeforePreload');
    out.preloadDependencies = [];
    for (let i = 0; i < s.preloadDependencyCount; i++) out.preloadDependencies.push(r.i32());
  }
  gap(buf.length, 'gapAtEnd');
  return out;
}
function encodeHeaderTail(t) {
  const w = new W();
  if (t.gapBeforeDepends) w.hex(t.gapBeforeDepends);
  for (const d of t.depends) { w.i32(d.length); for (const x of d) w.i32(x); }
  if (t.gapBeforeAssetRegistry) w.hex(t.gapBeforeAssetRegistry);
  if (t.assetRegistryCount !== undefined) w.i32(t.assetRegistryCount);
  if (t.gapBeforePreload) w.hex(t.gapBeforePreload);
  if (t.preloadDependencies) for (const x of t.preloadDependencies) w.i32(x);
  if (t.gapAtEnd) w.hex(t.gapAtEnd);
  return w.concat().toString('hex');
}

const sha256 = (b) => require('crypto').createHash('sha256').update(b).digest('hex');

// opts.only: Set of class names to decode natively (others stay undecoded) - used by probes.
// opts.altHeaders: () => [{ rel, bytes }] other headers of the same package path (other paks).
// When this header does not describe the .uexp (the exports do not tile it, or tagged properties
// do not parse), the first alternative header with which the .uexp tiles and every property list
// parses is used for the export data; the document then holds this header's tables plus
// uexpDecodedWith = { headerFrom, package } and both files still rebuild exactly.
function decode(uasset, uexp, opts = {}) {
  const first = decodeOne(uasset, uexp, opts);
  const propErr = (st) => st.exports.some((s) => s.error && /^properties:/.test(s.error));
  if (!opts.altHeaders || !uexp || !(first.stats.mismatch || propErr(first.stats))) return first;
  for (const alt of opts.altHeaders()) {
    let second;
    try { second = decodeOne(alt.bytes, uexp, { ...opts, altHeaders: null }); } catch (e) { continue; }
    if (second.stats.mismatch || propErr(second.stats)) continue;
    const live = decodeOne(uasset, null, opts);
    delete live.doc.uexpRaw;
    live.doc.uexpMismatch = first.stats.mismatch || 'the export table tiles the .uexp but the tagged properties of some exports do not parse with this header';
    live.doc.uexpDecodedWith = { headerFrom: alt.rel, package: second.doc };
    second.stats.pairedHeader = alt.rel;
    second.stats.headerUninterpreted += live.stats.headerUninterpreted;
    return { doc: live.doc, stats: second.stats, pkg: first.pkg };
  }
  return first;
}
function decodeOne(uasset, uexp, opts = {}) {
  const pkg = ua.parseHeader(uasset);
  const s = pkg.summary;
  const hdr = s.totalHeaderSize;
  const stats = { exports: [], headerUninterpreted: 0, gapBytes: 0 };
  const doc = { format: 'kh3-package-json/1', summary: s };
  doc.names = pkg.names.map((n) => (n.enc === 'ansi' ? [n.s, n.hashNoCase, n.hashCase] : [{ s: n.s, enc: n.enc }, n.hashNoCase, n.hashCase]));
  const nm = (f) => nameStr(pkg, f.index, f.number);
  doc.imports = pkg.imports.map((i) => ({ classPackage: nm(i.classPackage), className: nm(i.className), outerIndex: i.outerIndex, objectName: nm(i.objectName) }));
  for (const k of ['gapAfterSummary', 'gapAfterNames', 'gapAfterImports']) if (pkg[k]) { doc[k] = pkg[k]; stats.headerUninterpreted += pkg[k].length / 2; }
  const tail = decodeHeaderTail(pkg, uasset);
  stats.headerUninterpreted += tail.uninterpreted; delete tail.uninterpreted;
  doc.headerTables = tail;
  doc.exports = [];
  // export data regions in file order; gaps between them are kept verbatim
  const order = pkg.exports.map((e, i) => i).sort((a, b) => pkg.exports[a].serialOffset - pkg.exports[b].serialOffset || a - b);
  const regions = [];
  let pos = 0;
  const uexpLen = uexp ? uexp.length : 0;
  let mismatch = null;
  for (const i of order) {
    const e = pkg.exports[i];
    const start = e.serialOffset - hdr;
    if (!uexp || start < pos || start + e.serialSize > uexpLen) { mismatch = `export ${i + 1} data (offset ${start}, size ${e.serialSize}) lies outside the ${uexpLen}-byte .uexp or overlaps another export`; break; }
    if (start > pos) { regions.push({ gap: uexp.toString('hex', pos, start) }); stats.gapBytes += start - pos; }
    regions.push({ export: i + 1 });
    pos = start + e.serialSize;
  }
  // every package in the corpus whose export table matches its .uexp tiles it exactly and ends with
  // the 4-byte package tag; anything else is a header paired with another build's .uexp
  if (!mismatch && uexp && (pos + 4 !== uexpLen || uexp.readUInt32LE(pos) !== ua.TAG)) mismatch = `exports end at ${pos} but the .uexp has ${uexpLen} bytes (expected ${pos + 4} with the package tag)`;
  if (mismatch) {
    // The export table does not describe this .uexp (the two files are from different builds of
    // the package). The header is decoded; the .uexp is carried whole and counted uninterpreted.
    doc.uexpMismatch = mismatch;
    doc.uexpRaw = uexp ? uexp.toString('hex') : null;
    doc.exports = pkg.exports.map((e, i) => ({ index: i + 1, objectName: nm(e.objectName), class: classOfExport(pkg, e).name, ...e, data: null }));
    stats.mismatch = mismatch;
    stats.gapBytes = uexpLen;
    return { doc, stats, pkg };
  }
  if (uexp) {
    const trailer = uexp.subarray(pos);
    if (trailer.length === 4 && trailer.readUInt32LE(0) === ua.TAG) doc.uexpTrailer = 'tag';
    else { doc.uexpTrailer = { hex: trailer.toString('hex') }; stats.gapBytes += trailer.length; }
  } else doc.uexpTrailer = null;
  if (regions.some((x) => x.gap) ) doc.uexpLayout = regions;

  pkg.exports.forEach((e, i) => {
    const cls = classOfExport(pkg, e);
    const x = {
      index: i + 1, objectName: nm(e.objectName), class: cls.name, classIndex: e.classIndex, superIndex: e.superIndex, templateIndex: e.templateIndex, outerIndex: e.outerIndex,
      objectFlags: e.objectFlags, serialSize: e.serialSize, serialOffset: e.serialOffset, forcedExport: e.forcedExport, notForClient: e.notForClient, notForServer: e.notForServer,
      packageGuid: e.packageGuid, packageFlags: e.packageFlags, notAlwaysLoadedForEditorGame: e.notAlwaysLoadedForEditorGame, isAsset: e.isAsset,
      firstExportDependency: e.firstExportDependency, serializationBeforeSerialization: e.serializationBeforeSerialization, createBeforeSerialization: e.createBeforeSerialization,
      serializationBeforeCreate: e.serializationBeforeCreate, createBeforeCreate: e.createBeforeCreate,
    };
    const st = { class: cls.native ? cls.name : cls.name + ' [blueprint class]', size: e.serialSize, uninterpreted: 0, rawPropertyBytes: 0, delegated: 0, decoded: false, rawKinds: {} };
    stats.exports.push(st);
    const start = e.serialOffset - hdr;
    const bytes = uexp.subarray(start, start + e.serialSize);
    const native = nativeClass(pkg, e);
    if (native && native !== cls.name) { x.nativeClass = native; st.nativeClass = native; }
    const tool = DELEGATED.get(native);
    if (tool && !opts.embedDelegated) {
      x.data = { delegated: tool, size: bytes.length, sha256: sha256(bytes) };
      st.delegated = bytes.length; st.decoded = true; st.family = 'delegated';
      doc.exports.push(x);
      return;
    }
    const c = { pkg, stats: st, exp: e, exportIndex: i + 1, className: cls.name, nativeClass: native, structCodecs: STRUCTS, fileOffset: (p) => hdr + start + p, summary: s, isCDO: (e.objectFlags & RF_ClassDefaultObject) !== 0, opts };
    const r = new R(bytes);
    const data = {};
    const whole = native && (!opts.only || opts.only.has(native)) ? CLASSES.get(native) : null;
    if (whole && whole.codec.wholeExport) {
      // a class whose codec handles the export from its first byte (it embeds another converter's part)
      st.family = whole.family;
      try { data.whole = whole.codec.r(r, c); if (r.left) throw new Error(r.left + ' bytes left'); st.decoded = true; st.uninterpreted += st.rawPropertyBytes; x.data = data; doc.exports.push(x); return; } catch (err) {
        x.data = { undecoded: bytes.toString('hex'), error: String(err.message).slice(0, 300) };
        st.uninterpreted = bytes.length; st.delegated = 0; st.thirdParty = undefined; st.error = x.data.error;
        doc.exports.push(x); return;
      }
    }
    try {
      data.props = T.props.r(r, c);
      if (!c.isCDO) { const g = r.i32(); if (g !== 0 && g !== 1) throw new Error('object guid flag ' + g); data.objectGuid = g ? r.bytes(16).toString('hex') : null; }
    } catch (err) {
      x.data = { undecoded: bytes.toString('hex'), error: 'properties: ' + err.message };
      st.uninterpreted = bytes.length; st.rawPropertyBytes = 0; st.error = x.data.error;
      doc.exports.push(x);
      return;
    }
    const afterProps = r.p;
    c.props = data.props; // native serializers may depend on property values
    const reg = native && (!opts.only || opts.only.has(native)) ? CLASSES.get(native) : null;
    if (reg) st.family = reg.family;
    if (r.left === 0) st.decoded = true; // nothing after the properties
    else if (reg) {
      try {
        const v = reg.codec.r(r, c);
        if (r.left !== 0) throw new Error(`native data: ${r.left} bytes left at ${r.p} of ${bytes.length}`);
        data.native = v; st.decoded = true;
      } catch (err) {
        r.p = afterProps;
        data.undecoded = r.bytes(r.left).toString('hex');
        data.error = String(err.message).slice(0, 300);
        st.uninterpreted = bytes.length - afterProps; st.error = data.error;
      }
    } else if (r.left) {
      data.undecoded = r.bytes(r.left).toString('hex');
      st.uninterpreted = bytes.length - afterProps;
      st.error = native ? 'no native serializer for class ' + native : 'native base class not resolved';
    }
    st.uninterpreted += st.rawPropertyBytes;
    x.data = data;
    doc.exports.push(x);
  });
  return { doc, stats, pkg };
}

// opts.delegatedBytes(exportIndex, data) -> Buffer for exports stored as { delegated }.
function encode(doc, opts = {}) {
  const pkg = { summary: doc.summary, gapAfterSummary: doc.gapAfterSummary || '', gapAfterNames: doc.gapAfterNames || '', gapAfterImports: doc.gapAfterImports || '' };
  pkg.names = doc.names.map(([n, a, b]) => (typeof n === 'string' ? { s: n, enc: 'ansi', hashNoCase: a, hashCase: b } : { ...n, hashNoCase: a, hashCase: b }));
  const fn = (v) => nameIndex(pkg, v);
  pkg.imports = doc.imports.map((i) => ({ classPackage: fn(i.classPackage), className: fn(i.className), outerIndex: i.outerIndex, objectName: fn(i.objectName) }));
  pkg.exports = doc.exports.map((x) => ({ ...x, objectName: fn(x.objectName) }));
  pkg.tail = encodeHeaderTail(doc.headerTables);
  const uasset = ua.serializeHeader(pkg);
  if (doc.uexpDecodedWith) {
    // delegated exports are located through the export table of the header that describes the .uexp
    const inner = doc.uexpDecodedWith.package;
    const o2 = { ...opts, delegatedBytes: opts.delegatedBytes ? (i, d) => opts.delegatedBytes(i, d, inner) : undefined };
    return { uasset, uexp: encode(inner, o2).uexp };
  }
  if (doc.uexpMismatch) return { uasset, uexp: doc.uexpRaw === null ? null : Buffer.from(doc.uexpRaw, 'hex') };
  if (doc.uexpTrailer === null) return { uasset, uexp: null };
  const hdr = doc.summary.totalHeaderSize;
  const blobs = doc.exports.map((x, i) => {
    const e = pkg.exports[i];
    const d = x.data;
    if (d.delegated) { if (!opts.delegatedBytes) throw new Error('delegated export needs opts.delegatedBytes'); return opts.delegatedBytes(i + 1, d); }
    const native = x.nativeClass || x.class;
    if (d.whole !== undefined) {
      const cw = { pkg, exp: e, exportIndex: i + 1, className: x.class, nativeClass: native, structCodecs: STRUCTS, fileOffset: (p) => e.serialOffset + p, summary: doc.summary, opts };
      const ww = new W(Math.max(4096, Math.min(e.serialSize, 1 << 28)));
      CLASSES.get(native).codec.w(ww, d.whole, cw);
      return ww.b.subarray(0, ww.len);
    }
    if (d.props === undefined) return Buffer.from(d.undecoded, 'hex');
    const c = { pkg, exp: e, exportIndex: i + 1, className: x.class, nativeClass: native, structCodecs: STRUCTS, fileOffset: (p) => hdr + (e.serialOffset - hdr) + p, summary: doc.summary, isCDO: (e.objectFlags & RF_ClassDefaultObject) !== 0, opts };
    const w = new W(Math.max(4096, Math.min(e.serialSize, 1 << 28)));
    T.props.w(w, d.props, c);
    c.props = d.props;
    if (!c.isCDO) { if (d.objectGuid) { w.i32(1); w.hex(d.objectGuid); } else w.i32(0); }
    if (d.native !== undefined) CLASSES.get(native).codec.w(w, d.native, c);
    if (d.undecoded !== undefined) w.hex(d.undecoded);
    return w.b.subarray(0, w.len);
  });
  const parts = [];
  if (doc.uexpLayout) for (const reg of doc.uexpLayout) parts.push(reg.gap !== undefined ? Buffer.from(reg.gap, 'hex') : blobs[reg.export - 1]);
  else {
    const order = doc.exports.map((e, i) => i).sort((a, b) => doc.exports[a].serialOffset - doc.exports[b].serialOffset || a - b);
    for (const i of order) parts.push(blobs[i]);
  }
  if (doc.uexpTrailer === 'tag') { const t = Buffer.alloc(4); t.writeUInt32LE(ua.TAG); parts.push(t); } else parts.push(Buffer.from(doc.uexpTrailer.hex, 'hex'));
  return { uasset, uexp: Buffer.concat(parts) };
}

// ------------------------------------------------------------------ JSON file form
// Valid JSON with one export per line, so a file larger than a JS string can hold is still
// written and read piecewise.
function stringifyDoc(doc) {
  const parts = ['{\n'];
  for (const k of Object.keys(doc)) { if (k === 'exports') continue; parts.push(JSON.stringify(k) + ': ' + JSON.stringify(doc[k]) + ',\n'); }
  parts.push('"exports": [\n');
  doc.exports.forEach((e, i) => { parts.push(JSON.stringify(e)); parts.push(i + 1 < doc.exports.length ? ',\n' : '\n'); });
  parts.push(']\n}\n');
  return parts;
}
function parseDoc(buf) {
  if (buf.length < 0x8000000) return JSON.parse(buf.toString('utf8'));
  const marker = Buffer.from('\n"exports": [\n');
  const at = buf.indexOf(marker);
  if (at < 0) throw new Error('not a package JSON file');
  const head = buf.toString('utf8', 0, at).replace(/,$/, '');
  const doc = JSON.parse(head + '\n}');
  doc.exports = [];
  let p = at + marker.length;
  for (;;) {
    let e = buf.indexOf(10, p);
    if (e < 0) e = buf.length;
    if (buf[p] === 0x5d) break; // ]
    const end = buf[e - 1] === 0x2c ? e - 1 : e;
    doc.exports.push(JSON.parse(buf.toString('utf8', p, end)));
    p = e + 1;
  }
  return doc;
}

module.exports = { R, W, T, decode, encode, stringifyDoc, parseDoc, register, registerStruct, delegate, setBases, CLASSES, DELEGATED, NATIVE_STRUCTS, nameStr, nameIndex, classOfExport, nativeClass, BULK, count, strOut, strIn };

// class modules register themselves
require('./pkg_classes');
