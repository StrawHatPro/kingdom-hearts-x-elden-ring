// UE 4.17 localization resources: .locres (legacy and "compact" v1) and .locmeta, lossless both ways.
// Docs: docs/formats/locres.md.
const { R, W, T } = require('./package');

const LOCRES_MAGIC = '0e147475674a03fc4a15909dc3377f1b';
const LOCMETA_MAGIC = '4fee4ca1684855836c4c46bd70da507c';
const ctx = {};

// Legacy: namespaces { FString name, keys { FString key, uint32 source string hash, FString text } }
const legacy = T.arr(T.struct([['namespace', T.str], ['keys', T.arr(T.struct([['key', T.str], ['sourceStringHash', T.u32], ['text', T.str]]), 12)]]), 8);
// v1: int64 offset of the string array, namespaces with keys that index the string array
const compactKeys = T.arr(T.struct([['namespace', T.str], ['keys', T.arr(T.struct([['key', T.str], ['sourceStringHash', T.u32], ['stringIndex', T.i32]]), 12)]]), 8);

function decodeLocres(buf) {
  const r = new R(buf);
  const doc = { format: 'kh3-locres-json/1' };
  if (buf.length >= 17 && buf.toString('hex', 0, 16) === LOCRES_MAGIC) {
    r.p = 16;
    doc.version = r.u8();
    if (doc.version !== 1) throw new Error('locres version ' + doc.version);
    const offset = T.i64.r(r);
    doc.namespaces = compactKeys.r(r, ctx);
    if (r.p !== offset) throw new Error(`string array offset ${offset}, keys end at ${r.p}`);
    doc.strings = T.arr(T.str, 4).r(r, ctx);
  } else {
    doc.version = 'legacy';
    doc.namespaces = legacy.r(r, ctx);
  }
  if (r.left) throw new Error(r.left + ' bytes after the last string');
  return doc;
}
function encodeLocres(doc) {
  const w = new W(1 << 16);
  if (doc.version === 'legacy') { legacy.w(w, doc.namespaces, ctx); return w.concat(); }
  const keys = new W(1 << 16);
  compactKeys.w(keys, doc.namespaces, ctx);
  w.hex(LOCRES_MAGIC).u8(doc.version); T.i64.w(w, 16 + 1 + 8 + keys.len);
  w.push(keys.b.subarray(0, keys.len));
  T.arr(T.str, 4).w(w, doc.strings, ctx);
  return w.concat();
}

const locmeta = T.struct([['version', T.u8], ['nativeCulture', T.str], ['nativeLocRes', T.str]]);
function decodeLocmeta(buf) {
  if (buf.toString('hex', 0, 16) !== LOCMETA_MAGIC) throw new Error('not a locmeta file');
  const r = new R(buf, 16);
  const doc = { format: 'kh3-locmeta-json/1', ...locmeta.r(r, ctx) };
  if (r.left) throw new Error(r.left + ' trailing bytes');
  return doc;
}
function encodeLocmeta(doc) { const w = new W(256); w.hex(LOCMETA_MAGIC); locmeta.w(w, doc, ctx); return w.concat(); }

module.exports = { decodeLocres, encodeLocres, decodeLocmeta, encodeLocmeta, LOCRES_MAGIC, LOCMETA_MAGIC };
