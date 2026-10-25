// UE 4.17 tagged property serialization (FPropertyTag streams), lossless in both directions.
// A value whose type is not interpreted is kept as { raw: hex }, so serialize() is always exact.
const { nameOf } = require('./uasset');

// Structs the engine serializes natively (no property tags). Sizes in bytes.
const NATIVE_STRUCTS = {
  Vector: ['f32', 'f32', 'f32'], Vector2D: ['f32', 'f32'], Vector4: ['f32', 'f32', 'f32', 'f32'], Rotator: ['f32', 'f32', 'f32'],
  Quat: ['f32', 'f32', 'f32', 'f32'], LinearColor: ['f32', 'f32', 'f32', 'f32'], Color: ['u8', 'u8', 'u8', 'u8'],
  IntPoint: ['i32', 'i32'], Guid: ['u32', 'u32', 'u32', 'u32'], IntVector: ['i32', 'i32', 'i32'],
  RichCurveKey: ['u8', 'u8', 'u8', 'f32', 'f32', 'f32', 'f32', 'f32', 'f32'],
};
const KEYS = { Vector: 'xyz', Vector2D: 'xy', Vector4: 'xyzw', Rotator: ['pitch', 'yaw', 'roll'], Quat: 'xyzw', LinearColor: 'rgba', Color: 'bgra', IntPoint: 'xy', Guid: 'abcd', IntVector: 'xyz',
  RichCurveKey: ['interpMode', 'tangentMode', 'tangentWeightMode', 'time', 'value', 'arriveTangent', 'arriveTangentWeight', 'leaveTangent', 'leaveTangentWeight'] };
// FString <-> plain string where that is exact (non-empty ANSI), else { s, enc }.
const strOut = (x) => (x.enc === 'ansi' && x.s.length > 0 ? x.s : x);
const strIn = (x) => (typeof x === 'string' ? { s: x, enc: 'ansi' } : x);

const readName = (r) => ({ index: r.i32(), number: r.i32() });
const writeName = (w, pkg, s) => {
  const f = nameIndex(pkg, s);
  w.i32(f.index).i32(f.number);
};
// String -> FName. Strings were produced by nameOf(), so "Base_N" may mean number N+1 on "Base".
function nameIndex(pkg, s) {
  if (!pkg.nameLookup) { pkg.nameLookup = new Map(); pkg.names.forEach((n, i) => { if (!pkg.nameLookup.has(n.s)) pkg.nameLookup.set(n.s, i); }); }
  if (typeof s === 'object') return s; // already { index, number }: kept when the string form is ambiguous
  const m = /^(.*)_(\d+)$/.exec(s);
  if (pkg.nameLookup.has(s)) return { index: pkg.nameLookup.get(s), number: 0 };
  if (m && pkg.nameLookup.has(m[1]) && String(+m[2]) === m[2]) return { index: pkg.nameLookup.get(m[1]), number: +m[2] + 1 };
  throw new Error('name not in package name table: ' + s);
}
// FName -> string when that round-trips, else the raw pair.
function nameStr(pkg, f) {
  const s = nameOf(pkg, f);
  try { const back = nameIndex(pkg, s); if (back.index === f.index && back.number === f.number) return s; } catch { /* fall through */ }
  return f;
}

function readValue(r, pkg, type, size, tag) {
  const start = r.p;
  let v;
  switch (type) {
    case 'IntProperty': v = r.i32(); break;
    case 'UInt32Property': v = r.u32(); break;
    case 'FloatProperty': v = floatOrBits(r); break;
    case 'BoolProperty': v = null; break; // value lives in the tag
    case 'NameProperty': v = nameStr(pkg, readName(r)); break;
    case 'ObjectProperty': v = { object: r.i32() }; break;
    case 'StrProperty': case 'AssetObjectProperty': v = r.fstring(); break;
    case 'EnumProperty': v = nameStr(pkg, readName(r)); break;
    case 'ByteProperty': v = size === 1 ? r.u8() : size === 8 ? nameStr(pkg, readName(r)) : undefined; break;
    case 'Int8Property': v = r.b.readInt8(r.p); r.p += 1; break;
    case 'Int16Property': v = r.b.readInt16LE(r.p); r.p += 2; break;
    case 'UInt16Property': v = r.u16(); break;
    case 'Int64Property': v = String(r.b.readBigInt64LE(r.p)); r.p += 8; break;
    case 'UInt64Property': v = String(r.u64()); break;
    case 'StructProperty': {
      const layout = NATIVE_STRUCTS[tag.structName];
      if (layout && r.p + layoutSize(layout) > r.b.length) break;
      if (layout) { v = {}; layout.forEach((t, i) => { v[KEYS[tag.structName][i]] = t === 'f32' ? floatOrBits(r) : r[t](); }); }
      else if (tag.structName === 'StringAssetReference') { if (r.p + 4 <= r.b.length && fstringFits(r, size < 0 ? r.b.length : start + size)) v = { path: strOut(r.fstring()) }; }
      else if (looksTagged(r, pkg, start + size)) {
        // a struct that is not a property list after all reads as garbage: keep it raw instead
        try { v = { props: readProps(r, pkg) }; } catch (e) { v = undefined; }
      }
      break;
    }
    case 'ArrayProperty': {
      const count = r.i32();
      if (tag.innerType === 'StructProperty') {
        const inner = readTag(r, pkg);
        const items = [];
        for (let i = 0; i < count; i++) {
          const item = readValue(r, pkg, 'StructProperty', -1, inner);
          if (item === undefined) { v = undefined; r.p = start; break; }
          items.push(item);
        }
        if (r.p !== start) v = { innerTag: inner, items };
      } else if (['IntProperty', 'FloatProperty', 'NameProperty', 'ObjectProperty', 'StrProperty', 'UInt32Property', 'EnumProperty', 'AssetObjectProperty'].includes(tag.innerType)) {
        v = { items: [] };
        for (let i = 0; i < count; i++) v.items.push(readValue(r, pkg, tag.innerType, -1, tag));
      } else if (tag.innerType === 'ByteProperty' || tag.innerType === 'BoolProperty') {
        // one byte per element unless the bytes are enum names (8 per element)
        if (size - 4 === count) { v = { items: [...r.bytes(count)] }; }
        else if (size - 4 === count * 8) { v = { items: [] }; for (let i = 0; i < count; i++) v.items.push(nameStr(pkg, readName(r))); }
      }
      break;
    }
    case 'MapProperty': {
      // int32 number of keys to remove + those keys, int32 entry count + (key, value) pairs.
      // Keys and values are serialized like array elements (no tags).
      // A struct value is a tagged property list (the tag does not name the struct).
      if (!MAP_ELEMENT.has(tag.innerType) || !(MAP_ELEMENT.has(tag.valueType) || tag.valueType === 'StructProperty')) break;
      const end = start + size;
      const el = (t) => (r.p + (MAP_MIN[t] || 4) > end ? undefined : readValue(r, pkg, t, -1, t === 'StructProperty' ? MAP_STRUCT : tag));
      const m = { remove: [], entries: [] };
      let ok = true;
      const nRemove = r.i32();
      if (nRemove < 0 || nRemove > size) break;
      for (let i = 0; i < nRemove && ok; i++) { const k = el(tag.innerType); if (k === undefined) ok = false; else m.remove.push(k); }
      if (!ok || r.p + 4 > end) break;
      const n = r.i32();
      if (n < 0 || n > size) break;
      for (let i = 0; i < n && ok; i++) {
        const key = el(tag.innerType);
        const value = key === undefined ? undefined : el(tag.valueType);
        if (value === undefined) ok = false; else m.entries.push({ key, value });
      }
      if (ok) v = m;
      break;
    }
    default: break;
  }
  if (size < 0) return v; // array element: undefined makes the caller keep the whole array raw
  if (v === undefined || r.p !== start + size) {
    r.p = start;
    return { raw: r.hex(size) };
  }
  return v;
}
// Map key/value types whose element form (no tag) has a single possible layout.
const MAP_ELEMENT = new Set(['IntProperty', 'UInt32Property', 'FloatProperty', 'NameProperty', 'EnumProperty', 'ObjectProperty', 'StrProperty',
  'AssetObjectProperty', 'Int8Property', 'Int16Property', 'UInt16Property', 'Int64Property', 'UInt64Property']);
const MAP_STRUCT = { type: 'StructProperty' };
const MAP_MIN = { StructProperty: 8, Int8Property: 1, Int16Property: 2, UInt16Property: 2, NameProperty: 8, EnumProperty: 8, Int64Property: 8, UInt64Property: 8 };
const layoutSize = (l) => l.reduce((n, t) => n + (t === 'u8' ? 1 : 4), 0);
function bitsOf(f) { const b = Buffer.alloc(4); b.writeFloatLE(f); return b.readUInt32LE(0); }
function floatOrBits(r) { const bits = r.b.readUInt32LE(r.p); const f = r.f32(); return Number.isFinite(f) && bitsOf(f) === bits && !Object.is(f, -0) ? f : { f32bits: bits }; } // JSON cannot hold NaN, infinities or -0
// True when an FString at r.p ends at or before `end`.
function fstringFits(r, end) {
  const len = r.b.readInt32LE(r.p);
  return r.p + 4 + (len < 0 ? -len * 2 : len) <= end;
}
// A tagged struct starts with a valid FName (property name or None).
function looksTagged(r, pkg, end) {
  if (r.p + 8 > r.b.length) return false;
  const idx = r.b.readInt32LE(r.p), num = r.b.readInt32LE(r.p + 4);
  return idx >= 0 && idx < pkg.names.length && num >= 0 && num < 0x10000;
}

function readTag(r, pkg) {
  const name = nameStr(pkg, readName(r));
  if (name === 'None') return null;
  const t = { name, type: nameOf(pkg, readName(r)), size: r.i32(), arrayIndex: r.i32() };
  if (t.type === 'StructProperty') { t.structName = nameOf(pkg, readName(r)); t.structGuid = r.hex(16); }
  else if (t.type === 'BoolProperty') t.boolVal = r.u8();
  else if (t.type === 'ByteProperty' || t.type === 'EnumProperty') t.enumName = nameStr(pkg, readName(r));
  else if (t.type === 'ArrayProperty' || t.type === 'SetProperty') t.innerType = nameOf(pkg, readName(r));
  else if (t.type === 'MapProperty') { t.innerType = nameOf(pkg, readName(r)); t.valueType = nameOf(pkg, readName(r)); }
  t.hasGuid = r.u8();
  if (t.hasGuid) t.guid = r.hex(16);
  return t;
}

// Reads tags until None. Returns [{ tag, value }].
function readProps(r, pkg) {
  const out = [];
  for (;;) {
    const tag = readTag(r, pkg);
    if (!tag) return out;
    if (tag.size < 0 || r.p + tag.size > r.b.length) throw new Error(`property ${JSON.stringify(tag.name)} has size ${tag.size} at ${r.p}, beyond the data`);
    const value = readValue(r, pkg, tag.type, tag.size, tag);
    out.push({ tag, value });
  }
}

function writeTag(w, pkg, t) {
  writeName(w, pkg, t.name); writeName(w, pkg, t.type); w.i32(t.size).i32(t.arrayIndex);
  if (t.type === 'StructProperty') { writeName(w, pkg, t.structName); w.hex(t.structGuid); }
  else if (t.type === 'BoolProperty') w.u8(t.boolVal);
  else if (t.type === 'ByteProperty' || t.type === 'EnumProperty') writeName(w, pkg, t.enumName);
  else if (t.type === 'ArrayProperty' || t.type === 'SetProperty') writeName(w, pkg, t.innerType);
  else if (t.type === 'MapProperty') { writeName(w, pkg, t.innerType); writeName(w, pkg, t.valueType); }
  w.u8(t.hasGuid);
  if (t.hasGuid) w.hex(t.guid);
}
const writeFloat = (w, v) => (typeof v === 'object' ? w.u32(v.f32bits) : w.f32(v));

function writeValue(w, pkg, type, v, tag) {
  if (v && typeof v === 'object' && v.raw !== undefined) return w.hex(v.raw);
  switch (type) {
    case 'IntProperty': return w.i32(v);
    case 'UInt32Property': return w.u32(v);
    case 'FloatProperty': return writeFloat(w, v);
    case 'BoolProperty': return w;
    case 'NameProperty': case 'EnumProperty': return writeName(w, pkg, v);
    case 'ObjectProperty': return w.i32(v.object);
    case 'StrProperty': case 'AssetObjectProperty': return w.fstring(v);
    case 'ByteProperty': return typeof v === 'number' ? w.u8(v) : writeName(w, pkg, v);
    case 'Int8Property': { const b = Buffer.alloc(1); b.writeInt8(v); return w.push(b); }
    case 'Int16Property': { const b = Buffer.alloc(2); b.writeInt16LE(v); return w.push(b); }
    case 'UInt16Property': return w.u16(v);
    case 'Int64Property': return w.i64(BigInt(v));
    case 'UInt64Property': return w.u64(BigInt(v));
    case 'StructProperty': {
      const layout = NATIVE_STRUCTS[tag.structName];
      if (layout) { layout.forEach((t, i) => { const x = v[KEYS[tag.structName][i]]; t === 'f32' ? writeFloat(w, x) : w[t](x); }); return w; }
      if (v.path !== undefined) return w.fstring(strIn(v.path));
      return writeProps(w, pkg, v.props);
    }
    case 'ArrayProperty': {
      w.i32(v.items.length);
      if (v.innerTag) { writeTag(w, pkg, v.innerTag); for (const it of v.items) writeValue(w, pkg, 'StructProperty', it, v.innerTag); return w; }
      if (tag.innerType === 'ByteProperty' || tag.innerType === 'BoolProperty') {
        if (v.items.every((x) => typeof x === 'number')) return w.push(Buffer.from(v.items));
        for (const it of v.items) writeName(w, pkg, it);
        return w;
      }
      for (const it of v.items) writeValue(w, pkg, tag.innerType, it, tag);
      return w;
    }
    case 'MapProperty': {
      w.i32(v.remove.length);
      for (const k of v.remove) writeValue(w, pkg, tag.innerType, k, tag);
      w.i32(v.entries.length);
      for (const e of v.entries) { writeValue(w, pkg, tag.innerType, e.key, tag); writeValue(w, pkg, tag.valueType, e.value, tag.valueType === 'StructProperty' ? MAP_STRUCT : tag); }
      return w;
    }
    default: throw new Error('cannot write ' + type);
  }
}
function writeProps(w, pkg, props) {
  for (const { tag, value } of props) { writeTag(w, pkg, tag); writeValue(w, pkg, tag.type, value, tag); }
  writeName(w, pkg, 'None');
  return w;
}

module.exports = { readProps, writeProps, readTag, writeTag, readValue, writeValue, nameStr, nameIndex, NATIVE_STRUCTS };
