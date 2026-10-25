// UE 4.17 FAssetRegistryState as saved in DevelopmentAssetRegistry.bin, lossless both ways.
// Docs: docs/formats/assetregistry.md.
const { R, W, T } = require('./package');

const GUID = 'e79e7f713a49b0e93291b3880781381b'; // FAssetRegistryVersion GUID bytes as stored

// Names are (index into the file's own name table, number); number 0 is written as the bare index.
const name = { r: (r) => { const i = r.i32(), n = r.i32(); return n ? [i, n] : i; }, w: (w, v) => { if (Array.isArray(v)) w.i32(v[0]).i32(v[1]); else w.i32(v).i32(0); } };
const assetData = T.struct([
  ['objectPath', name], ['packagePath', name], ['assetClass', name], ['packageName', name], ['assetName', name],
  ['tags', T.arr(T.struct([['key', name], ['value', T.str]]), 12)],
  ['chunkIds', T.arr(T.i32, 4)], ['packageFlags', T.u32],
]);
// FAssetIdentifier: a bit per present name (1 PackageName, 2 PrimaryAssetType, 4 ObjectName, 8 ValueName)
const identifier = T.struct([
  ['fieldBits', T.u8],
  ['packageName', T.when((o) => o.fieldBits & 1, name)], ['primaryAssetType', T.when((o) => o.fieldBits & 2, name)],
  ['objectName', T.when((o) => o.fieldBits & 4, name)], ['valueName', T.when((o) => o.fieldBits & 8, name)],
]);
// Dependency node: five counts, then that many node indexes in total. Verified on the data: the
// fifth list is exactly the inverse of the first four (referencers), and every target of the
// third list is a searchable-name identifier (field bits 13). The first, second and fourth are
// kept as numbered dependency lists (stock engine: hard, soft, manage; not verifiable here).
const dependsNode = {
  r: (r, c) => {
    const o = { identifier: identifier.r(r, c) };
    const counts = [r.i32(), r.i32(), r.i32(), r.i32(), r.i32()];
    for (const k of counts) if (k < 0 || k * 4 > r.left) throw new Error('bad dependency count at ' + r.p);
    const list = (k) => { const a = new Array(k); for (let i = 0; i < k; i++) a[i] = r.i32(); return a; };
    o.dependencies0 = list(counts[0]); o.dependencies1 = list(counts[1]); o.searchableNameDependencies = list(counts[2]); o.dependencies3 = list(counts[3]); o.referencers = list(counts[4]);
    return o;
  },
  w: (w, o, c) => {
    identifier.w(w, o.identifier, c);
    const lists = [o.dependencies0, o.dependencies1, o.searchableNameDependencies, o.dependencies3, o.referencers];
    for (const l of lists) w.i32(l.length);
    for (const l of lists) for (const x of l) w.i32(x);
  },
};
const packageData = T.struct([['packageName', name], ['diskSize', T.i64], ['packageGuid', T.guid]]);
const nameEntry = T.struct([['name', T.str], ['hashNoCase', T.u16], ['hashCase', T.u16]]);
const body = T.struct([['assets', T.arr(assetData, 52)], ['dependsNodes', T.arr(dependsNode, 21)], ['packageData', T.arr(packageData, 32)]]);

function decode(buf) {
  const r = new R(buf);
  if (r.hex(16) !== GUID) throw new Error('not an asset registry state');
  const doc = { format: 'kh3-asset-registry-json/1', version: r.i32() };
  if (doc.version !== 4) throw new Error('asset registry version ' + doc.version);
  const nameOffset = T.i64.r(r);
  Object.assign(doc, body.r(r, {}));
  if (r.p !== nameOffset) throw new Error(`name table offset ${nameOffset}, data ends at ${r.p}`);
  doc.names = T.arr(nameEntry, 8).r(r, {});
  if (r.left) throw new Error(r.left + ' bytes after the name table');
  return doc;
}
function encode(doc) {
  const b = new W(1 << 24);
  body.w(b, doc, {});
  const w = new W(b.len + (1 << 24));
  w.hex(GUID).i32(doc.version); T.i64.w(w, 16 + 4 + 8 + b.len);
  w.push(b.b.subarray(0, b.len));
  T.arr(nameEntry, 8).w(w, doc.names, {});
  return w.concat();
}
// JSON text in pieces (the document is too large for one string): one array element per line.
function stringify(doc) {
  const parts = [`{"format":${JSON.stringify(doc.format)},"version":${doc.version},\n`];
  for (const k of ['names', 'assets', 'dependsNodes', 'packageData']) {
    parts.push(`"${k}":[\n`);
    const a = doc[k];
    for (let i = 0; i < a.length; i += 2000) parts.push(a.slice(i, i + 2000).map((x) => JSON.stringify(x)).join(',\n') + (i + 2000 < a.length ? ',\n' : '\n'));
    parts.push(k === 'packageData' ? ']}\n' : '],\n');
  }
  return parts;
}
function parse(buf) {
  const doc = {};
  let p = 0;
  const line = () => { const e = buf.indexOf(10, p); const s = buf.toString('utf8', p, e < 0 ? buf.length : e); p = e < 0 ? buf.length : e + 1; return s; };
  Object.assign(doc, JSON.parse(line().replace(/,$/, '') + '}'));
  for (;;) {
    const head = line();
    const m = /^"(\w+)":\[$/.exec(head);
    if (!m) break;
    const a = (doc[m[1]] = []);
    for (;;) { const l = line(); if (l[0] === ']') break; a.push(JSON.parse(l.replace(/,$/, ''))); }
  }
  return doc;
}
module.exports = { decode, encode, stringify, parse, GUID };
