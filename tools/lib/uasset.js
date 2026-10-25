// UE 4.17 cooked package (.uasset header + .uexp export data) reader/writer.
// Format notes: docs/formats/uasset.md. Every structure parsed here is also written back by
// serialize(), and tools/uasset_roundtrip.js proves parse -> serialize is byte-identical.
const TAG = 0x9e2a83c1;

class Reader {
  constructor(buf, pos = 0) { this.b = buf; this.p = pos; }
  i32() { const v = this.b.readInt32LE(this.p); this.p += 4; return v; }
  u32() { const v = this.b.readUInt32LE(this.p); this.p += 4; return v; }
  u16() { const v = this.b.readUInt16LE(this.p); this.p += 2; return v; }
  u8() { return this.b[this.p++]; }
  i64() { const v = this.b.readBigInt64LE(this.p); this.p += 8; return Number(v); }
  u64() { const v = this.b.readBigUInt64LE(this.p); this.p += 8; return v; }
  f32() { const v = this.b.readFloatLE(this.p); this.p += 4; return v; }
  bytes(n) { const v = this.b.subarray(this.p, this.p + n); this.p += n; return v; }
  hex(n) { return this.bytes(n).toString('hex'); }
  // FString: int32 length including the terminator; negative = UTF-16.
  fstring() {
    const len = this.i32();
    if (len === 0) return { s: '', enc: 'empty' };
    if (len > 0) { const s = this.b.toString('latin1', this.p, this.p + len - 1); this.p += len; return { s, enc: 'ansi' }; }
    const s = this.b.toString('utf16le', this.p, this.p - len * 2 - 2); this.p += -len * 2; return { s, enc: 'utf16' };
  }
}

class Writer {
  constructor() { this.parts = []; this.len = 0; }
  push(b) { this.parts.push(b); this.len += b.length; return this; }
  i32(v) { const b = Buffer.alloc(4); b.writeInt32LE(v); return this.push(b); }
  u32(v) { const b = Buffer.alloc(4); b.writeUInt32LE(v >>> 0); return this.push(b); }
  u16(v) { const b = Buffer.alloc(2); b.writeUInt16LE(v); return this.push(b); }
  u8(v) { return this.push(Buffer.from([v])); }
  i64(v) { const b = Buffer.alloc(8); b.writeBigInt64LE(BigInt(v)); return this.push(b); }
  u64(v) { const b = Buffer.alloc(8); b.writeBigUInt64LE(BigInt(v)); return this.push(b); }
  f32(v) { const b = Buffer.alloc(4); b.writeFloatLE(v); return this.push(b); }
  hex(h) { return this.push(Buffer.from(h, 'hex')); }
  fstring(x) {
    if (x.enc === 'empty') return this.i32(0);
    if (x.enc === 'ansi') { this.i32(x.s.length + 1); return this.push(Buffer.from(x.s + '\0', 'latin1')); }
    this.i32(-(x.s.length + 1));
    return this.push(Buffer.from(x.s + '\0', 'utf16le'));
  }
  concat() { return Buffer.concat(this.parts, this.len); }
}

function parseHeader(buf) {
  const r = new Reader(buf);
  const s = {};
  if (r.u32() !== TAG) throw new Error('not a UE4 package');
  s.legacyFileVersion = r.i32();
  if (s.legacyFileVersion !== -7) throw new Error('unsupported legacy version ' + s.legacyFileVersion);
  s.legacyUE3Version = r.i32();
  s.fileVersionUE4 = r.i32();
  s.fileVersionLicenseeUE4 = r.i32();
  const nCustom = r.i32();
  s.customVersions = [];
  for (let i = 0; i < nCustom; i++) s.customVersions.push({ key: r.hex(16), version: r.i32() });
  s.totalHeaderSize = r.i32();
  s.folderName = r.fstring();
  s.packageFlags = r.u32();
  s.nameCount = r.i32(); s.nameOffset = r.i32();
  s.gatherableTextDataCount = r.i32(); s.gatherableTextDataOffset = r.i32();
  s.exportCount = r.i32(); s.exportOffset = r.i32();
  s.importCount = r.i32(); s.importOffset = r.i32();
  s.dependsOffset = r.i32();
  s.stringAssetReferencesCount = r.i32(); s.stringAssetReferencesOffset = r.i32();
  s.searchableNamesOffset = r.i32();
  s.thumbnailTableOffset = r.i32();
  s.guid = r.hex(16);
  const nGen = r.i32();
  s.generations = [];
  for (let i = 0; i < nGen; i++) s.generations.push({ exportCount: r.i32(), nameCount: r.i32() });
  const engineVersion = () => ({ major: r.u16(), minor: r.u16(), patch: r.u16(), changelist: r.u32(), branch: r.fstring() });
  s.savedByEngineVersion = engineVersion();
  s.compatibleWithEngineVersion = engineVersion();
  s.compressionFlags = r.u32();
  s.compressedChunkCount = r.i32();
  if (s.compressedChunkCount !== 0) throw new Error('compressed chunks not supported');
  s.packageSource = r.u32();
  const nCook = r.i32();
  s.additionalPackagesToCook = [];
  for (let i = 0; i < nCook; i++) s.additionalPackagesToCook.push(r.fstring());
  s.assetRegistryDataOffset = r.i32();
  s.bulkDataStartOffset = r.i64();
  s.worldTileInfoDataOffset = r.i32();
  const nChunk = r.i32();
  s.chunkIds = [];
  for (let i = 0; i < nChunk; i++) s.chunkIds.push(r.i32());
  s.preloadDependencyCount = r.i32(); s.preloadDependencyOffset = r.i32();
  const summaryEnd = r.p;

  // Everything between the fixed tables is kept verbatim in `gaps` so serialize() is exact
  // even where a table is not interpreted yet.
  const pkg = { summary: s, summaryEnd };
  r.p = s.nameOffset;
  pkg.names = [];
  for (let i = 0; i < s.nameCount; i++) { const n = r.fstring(); pkg.names.push({ ...n, hashNoCase: r.u16(), hashCase: r.u16() }); }
  pkg.namesEnd = r.p;
  r.p = s.importOffset;
  const fname = () => ({ index: r.i32(), number: r.i32() });
  pkg.imports = [];
  for (let i = 0; i < s.importCount; i++) pkg.imports.push({ classPackage: fname(), className: fname(), outerIndex: r.i32(), objectName: fname() });
  pkg.importsEnd = r.p;
  r.p = s.exportOffset;
  pkg.exports = [];
  for (let i = 0; i < s.exportCount; i++) {
    pkg.exports.push({
      classIndex: r.i32(), superIndex: r.i32(), templateIndex: r.i32(), outerIndex: r.i32(), objectName: fname(), objectFlags: r.u32(),
      serialSize: r.i64(), serialOffset: r.i64(), forcedExport: r.i32(), notForClient: r.i32(), notForServer: r.i32(),
      packageGuid: r.hex(16), packageFlags: r.u32(), notAlwaysLoadedForEditorGame: r.i32(), isAsset: r.i32(),
      firstExportDependency: r.i32(), serializationBeforeSerialization: r.i32(), createBeforeSerialization: r.i32(),
      serializationBeforeCreate: r.i32(), createBeforeCreate: r.i32(),
    });
  }
  pkg.exportsEnd = r.p;
  // Tail: depends map, asset registry data, preload dependencies. Kept raw for now.
  pkg.tail = buf.subarray(pkg.exportsEnd).toString('hex');
  pkg.gapAfterSummary = buf.subarray(summaryEnd, s.nameOffset).toString('hex');
  pkg.gapAfterNames = buf.subarray(pkg.namesEnd, s.importOffset).toString('hex');
  pkg.gapAfterImports = buf.subarray(pkg.importsEnd, s.exportOffset).toString('hex');
  pkg.size = buf.length;
  return pkg;
}

function serializeHeader(pkg) {
  const s = pkg.summary;
  const w = new Writer();
  w.u32(TAG).i32(s.legacyFileVersion).i32(s.legacyUE3Version).i32(s.fileVersionUE4).i32(s.fileVersionLicenseeUE4);
  w.i32(s.customVersions.length);
  for (const c of s.customVersions) w.hex(c.key).i32(c.version);
  w.i32(s.totalHeaderSize).fstring(s.folderName).u32(s.packageFlags);
  w.i32(s.nameCount).i32(s.nameOffset).i32(s.gatherableTextDataCount).i32(s.gatherableTextDataOffset);
  w.i32(s.exportCount).i32(s.exportOffset).i32(s.importCount).i32(s.importOffset).i32(s.dependsOffset);
  w.i32(s.stringAssetReferencesCount).i32(s.stringAssetReferencesOffset).i32(s.searchableNamesOffset).i32(s.thumbnailTableOffset);
  w.hex(s.guid).i32(s.generations.length);
  for (const g of s.generations) w.i32(g.exportCount).i32(g.nameCount);
  for (const v of [s.savedByEngineVersion, s.compatibleWithEngineVersion]) w.u16(v.major).u16(v.minor).u16(v.patch).u32(v.changelist).fstring(v.branch);
  w.u32(s.compressionFlags).i32(s.compressedChunkCount).u32(s.packageSource).i32(s.additionalPackagesToCook.length);
  for (const a of s.additionalPackagesToCook) w.fstring(a);
  w.i32(s.assetRegistryDataOffset).i64(s.bulkDataStartOffset).i32(s.worldTileInfoDataOffset).i32(s.chunkIds.length);
  for (const c of s.chunkIds) w.i32(c);
  w.i32(s.preloadDependencyCount).i32(s.preloadDependencyOffset);
  w.hex(pkg.gapAfterSummary);
  for (const n of pkg.names) w.fstring(n).u16(n.hashNoCase).u16(n.hashCase);
  w.hex(pkg.gapAfterNames);
  const fname = (f) => w.i32(f.index).i32(f.number);
  for (const i of pkg.imports) { fname(i.classPackage); fname(i.className); w.i32(i.outerIndex); fname(i.objectName); }
  w.hex(pkg.gapAfterImports);
  for (const e of pkg.exports) {
    w.i32(e.classIndex).i32(e.superIndex).i32(e.templateIndex).i32(e.outerIndex); fname(e.objectName); w.u32(e.objectFlags);
    w.i64(e.serialSize).i64(e.serialOffset).i32(e.forcedExport).i32(e.notForClient).i32(e.notForServer);
    w.hex(e.packageGuid).u32(e.packageFlags).i32(e.notAlwaysLoadedForEditorGame).i32(e.isAsset);
    w.i32(e.firstExportDependency).i32(e.serializationBeforeSerialization).i32(e.createBeforeSerialization).i32(e.serializationBeforeCreate).i32(e.createBeforeCreate);
  }
  w.hex(pkg.tail);
  return w.concat();
}

// FName -> string ("Name" or "Name_N" for number > 0, matching the engine's display form).
function nameOf(pkg, f) {
  const n = pkg.names[f.index];
  if (!n) return `<bad name ${f.index}>`;
  return f.number ? `${n.s}_${f.number - 1}` : n.s;
}
// Package index: >0 export, <0 import, 0 none.
function objectName(pkg, idx) {
  if (idx === 0) return null;
  if (idx > 0) return nameOf(pkg, pkg.exports[idx - 1].objectName);
  return nameOf(pkg, pkg.imports[-idx - 1].objectName);
}

module.exports = { TAG, Reader, Writer, parseHeader, serializeHeader, nameOf, objectName };
