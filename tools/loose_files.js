// Every file under assets/ that is not part of a cooked package: identify its format from its
// bytes, and decode the engine's own formats to JSON with a byte-exact round trip.
//
//   node tools/loose_files.js
//
// Decoded (written under assets_converted/loose/<path>.json, rebuilt and compared):
//   .locres, .locmeta (tools/lib/locres.js), GlobalShaderCache-*.bin (shader map, tools/lib/pkg_classes/material.js)
// Identified as a standard format (signature and structure checked per file, not converted):
//   ICU data, TrueType, PNG, text files (ini/txt/json/...), demuxed movie streams, asset registry.
// Output: audit/loose_files.json. Nothing under assets/ is written.
const fs = require('fs');
const path = require('path');
const zlib = require('zlib');
const { ROOT, logRun, writeJson } = require('./lib/log');
const P = require('./lib/package');
const L = require('./lib/locres');
const M = require('./lib/pkg_classes/material');
const A = require('./lib/assetregistry');
const UA = require('./lib/uasset');
const TX = require('./lib/texture');

const VERSION = '1.0.0';
const ASSETS = path.join(ROOT, 'assets');
const OUT_ROOT = path.join(ROOT, 'assets_converted', 'loose');
const OUT = path.join(ROOT, 'audit', 'loose_files.json');

function walk(dir, relDir, out) {
  for (const d of fs.readdirSync(dir, { withFileTypes: true })) {
    const rel = relDir ? relDir + '/' + d.name : d.name;
    if (d.isDirectory()) walk(path.join(dir, d.name), rel, out);
    else out.push(rel);
  }
}
const head = (abs, n) => { const fd = fs.openSync(abs, 'r'); const b = Buffer.alloc(n); const got = fs.readSync(fd, b, 0, n, 0); fs.closeSync(fd); return b.subarray(0, got); };
const writeDoc = (rel, text) => { const out = path.join(OUT_ROOT, rel + '.json'); fs.mkdirSync(path.dirname(out), { recursive: true }); fs.writeFileSync(out, text); return path.relative(ROOT, out).replace(/\\/g, '/'); };
// the loose totals count a file as uninterpreted when its row says so


// ---- format handlers: return { format, identified_by, ...details }; decoded formats add roundtrip
function text(abs, rel, ext) {
  const b = fs.readFileSync(abs);
  let encoding, s;
  if (b.length >= 2 && b[0] === 0xff && b[1] === 0xfe) { encoding = 'UTF-16LE with BOM'; s = b.toString('utf16le', 2); if (!Buffer.from(s, 'utf16le').equals(b.subarray(2))) throw new Error('invalid UTF-16'); }
  else {
    const bom = b.length >= 3 && b[0] === 0xef && b[1] === 0xbb && b[2] === 0xbf;
    s = b.toString('utf8', bom ? 3 : 0);
    if (!Buffer.from(s, 'utf8').equals(b.subarray(bom ? 3 : 0))) throw new Error('not valid UTF-8');
    encoding = bom ? 'UTF-8 with BOM' : /[^\x00-\x7f]/.test(s) ? 'UTF-8' : 'ASCII';
  }
  if (/\x00/.test(s)) throw new Error('NUL character in text');
  const kind = { ini: 'Unreal config (INI text)', txt: 'plain text', json: 'JSON text', uplugin: 'JSON text (Unreal plugin descriptor)', uproject: 'JSON text (Unreal project descriptor)', tps: 'XML text (third-party software notice)', vdf: 'Valve KeyValues text (Steam controller configuration)' }[ext];
  const r = { format: kind, identified_by: `decodes as ${encoding} text without NUL characters`, encoding, lines: s.split('\n').length };
  if (ext === 'json' || ext === 'uplugin' || ext === 'uproject') {
    try { JSON.parse(s.replace(/^﻿/, '')); r.identified_by += '; parses as JSON'; } catch (e) {
      // the engine's reader accepts a comma before a closing bracket; strict JSON does not
      JSON.parse(s.replace(/^﻿/, '').replace(/,(\s*[\]}])/g, '$1'));
      r.format += ' with trailing commas'; r.identified_by += '; parses as JSON once commas before closing brackets are dropped';
    }
  }
  if (ext === 'tps' && !/^\s*<\?xml|^\s*</.test(s)) throw new Error('not XML');
  r.decoded = 'text: readable as is';
  return r;
}
function icu(abs) {
  const b = head(abs, 32);
  if (b[2] !== 0xda || b[3] !== 0x27) throw new Error('no ICU data header magic');
  const info = { headerSize: b.readUInt16LE(0), isBigEndian: b[8], charsetFamily: b[9], sizeofUChar: b[10], dataFormat: b.toString('latin1', 12, 16), formatVersion: [...b.subarray(16, 20)].join('.'), dataVersion: [...b.subarray(20, 24)].join('.') };
  return { format: `ICU data file (ICU 53 "icudt53l" tree), data format "${info.dataFormat}"`, identified_by: 'ICU common data header: magic bytes DA 27 at offset 2, UDataInfo block', ...info };
}
function sfnt(abs, rel, ext, size) {
  const b = head(abs, 12 + 16 * 64);
  if (b.readUInt32BE(0) !== 0x00010000) throw new Error('not a TrueType sfnt');
  const n = b.readUInt16BE(4);
  const tables = [];
  for (let i = 0; i < n && 12 + 16 * i + 16 <= b.length; i++) { const o = 12 + 16 * i; const off = b.readUInt32BE(o + 8), len = b.readUInt32BE(o + 12); if (off + len > size) throw new Error('table outside file'); tables.push(b.toString('latin1', o, o + 4)); }
  return { format: 'TrueType font (sfnt)', identified_by: 'sfnt version 00010000 and a table directory whose tables lie inside the file', tables: tables.join(' ') };
}
function png(abs) {
  const b = fs.readFileSync(abs);
  if (b.toString('hex', 0, 8) !== '89504e470d0a1a0a') throw new Error('no PNG signature');
  let p = 8, chunks = 0, w = 0, h = 0, last = '';
  while (p + 12 <= b.length) {
    const len = b.readUInt32BE(p); const type = b.toString('latin1', p + 4, p + 8);
    if (p + 12 + len > b.length) throw new Error('chunk outside file');
    if (zlib.crc32(b.subarray(p + 4, p + 8 + len)) !== b.readUInt32BE(p + 8 + len)) throw new Error('chunk CRC mismatch');
    if (type === 'IHDR') { w = b.readUInt32BE(p + 8); h = b.readUInt32BE(p + 12); }
    chunks++; last = type; p += 12 + len;
  }
  if (last !== 'IEND' || p !== b.length) throw new Error('PNG does not end with IEND');
  return { format: 'PNG image', identified_by: 'PNG signature; every chunk CRC verified; ends with IEND', width: w, height: h, chunks };
}
function stream(abs, rel, ext) {
  const b = head(abs, 16);
  const note = 'elementary stream demultiplexed from a CRI USM movie by tools/usm_extract.js (see docs/formats/usm.md and audit/usm_manifest.json)';
  if (ext === 'h264') { if (b.readUInt32BE(0) !== 1) throw new Error('no Annex B start code'); return { format: 'H.264 / AVC Annex B elementary stream', identified_by: `start code 00 00 00 01, first NAL unit type ${b[4] & 0x1f}`, note }; }
  if (ext === 'm1v') { if (b.readUInt32BE(0) !== 0x1b3) throw new Error('no MPEG sequence header'); return { format: 'MPEG-1 video elementary stream', identified_by: `sequence header start code 00 00 01 B3, ${(b[4] << 4) | (b[5] >> 4)}x${((b[5] & 15) << 8) | b[6]}`, note }; }
  if ((b.readUInt32BE(0) & 0x7f7f7f7f) !== 0x48434100) throw new Error('no HCA signature');
  return { format: 'CRI HCA audio', identified_by: 'signature "HCA\\0", version ' + b[4] + '.' + b[5], note };
}
function locres(abs, rel) {
  const b = fs.readFileSync(abs);
  const doc = L.decodeLocres(b);
  const textOut = JSON.stringify(doc, null, 1) + '\n';
  const back = L.encodeLocres(JSON.parse(textOut));
  const keys = doc.namespaces.reduce((n, ns) => n + ns.keys.length, 0);
  return { format: doc.version === 'legacy' ? 'Unreal localization resource (.locres, legacy layout)' : 'Unreal localization resource (.locres, version 1 with magic GUID)', identified_by: doc.version === 'legacy' ? 'no magic; parses exactly as the legacy namespace/key/string layout' : 'magic GUID 7574140E-FC034A67-9D90154A-1B7F37C3 and version byte 1', namespaces: doc.namespaces.length, keys, strings: doc.strings ? doc.strings.length : keys, decoded: writeDoc(rel, textOut), roundtrip_identical: back.equals(b), uninterpreted_bytes: 0 };
}
function locmeta(abs, rel) {
  const b = fs.readFileSync(abs);
  const doc = L.decodeLocmeta(b);
  const textOut = JSON.stringify(doc, null, 1) + '\n';
  return { format: 'Unreal localization metadata (.locmeta)', identified_by: 'magic GUID A14CEE4F-83554868-BD464C6C-7C50DA70', native_culture: doc.nativeCulture, decoded: writeDoc(rel, textOut), roundtrip_identical: L.encodeLocmeta(JSON.parse(textOut)).equals(b), uninterpreted_bytes: 0 };
}
function bin(abs, rel) {
  const b = fs.readFileSync(abs);
  if (b.readUInt32LE(0) === 0x47534d42) {
    const st = { rawPropertyBytes: 0, rawKinds: {}, thirdParty: {} };
    const c = { namesAsStrings: true, fileOffset: (p) => p, stats: st };
    const r = new P.R(b);
    const doc = { format: 'kh3-global-shader-cache-json/1', ...M.globalShaderCache.r(r, c) };
    if (r.left) throw new Error(r.left + ' bytes after the shader map');
    const parts = ['{"format":' + JSON.stringify(doc.format) + ',\n"shaders":[\n', doc.shaders.map((s) => JSON.stringify(s)).join(',\n'), '\n],\n"shaderPipelines":[\n', doc.shaderPipelines.map((s) => JSON.stringify(s)).join(',\n'), '\n]}\n'];
    const textOut = parts.join('');
    const w = new P.W(b.length);
    M.globalShaderCache.w(w, JSON.parse(textOut), { namesAsStrings: true, fileOffset: (p) => p });
    return { format: 'Unreal global shader cache (TShaderMap<FGlobalShaderType> inline)', identified_by: 'tag "GSMB" (bytes 42 4D 53 47)', shaders: doc.shaders.length, shader_pipelines: doc.shaderPipelines.length, decoded: writeDoc(rel, textOut), roundtrip_identical: w.concat().equals(b), uninterpreted_bytes: st.rawPropertyBytes, uninterpreted_reason: st.rawPropertyBytes ? 'shader parameter bindings without a per-type layout (docs/formats/material.md)' : undefined, third_party_bytes: st.thirdParty };
  }
  if (b.toString('hex', 0, 16) === A.GUID) {
    const doc = A.decode(b);
    const textOut = Buffer.concat(A.stringify(doc).map((s) => Buffer.from(s, 'utf8')));
    const back = A.encode(A.parse(textOut));
    return { format: 'Unreal asset registry state (FAssetRegistryState, version 4)', identified_by: 'FAssetRegistryVersion GUID (bytes E7 9E 7F 71 3A 49 B0 E9 32 91 B3 88 07 81 38 1B) and version 4', assets: doc.assets.length, depends_nodes: doc.dependsNodes.length, package_data: doc.packageData.length, names: doc.names.length, decoded: writeDoc(rel, textOut), roundtrip_identical: back.equals(b), uninterpreted_bytes: 0 };
  }
  throw new Error('unknown .bin signature ' + b.toString('hex', 0, 4));
}
// .uexp / .ubulk whose .uasset is not next to it
function orphan(abs, rel, ext) {
  const live = !rel.startsWith('_superseded/') && !rel.startsWith('_case_collision/');
  // tools/package_convert.js pairs an orphan .uexp with a header found at the same path elsewhere
  // when that header's export table tiles the file exactly; re-check that conversion here
  const converted = path.join(ROOT, 'assets_converted', 'packages', rel + '.json');
  if (ext === 'uexp' && fs.existsSync(converted)) {
    const doc = P.parseDoc(fs.readFileSync(converted));
    const uexp = fs.readFileSync(abs);
    const back = P.encode(doc, { delegatedBytes: (i, d, inner) => { const D = inner || doc; const e = D.exports[i - 1]; const s = e.serialOffset - D.summary.totalHeaderSize; return uexp.subarray(s, s + e.serialSize); } });
    return { format: 'cooked package export data (.uexp) decoded with the header of another copy of the package', identified_by: `the export table of ${doc.headerFrom} tiles this file exactly (its exports end 4 bytes before the end, at the package tag)`, header: doc.headerFrom, decoded: path.relative(ROOT, converted).replace(/\\/g, '/'), roundtrip_identical: back.uexp !== null && back.uexp.equals(uexp), uninterpreted_bytes: 0, note: 'bytes without a layout inside the exports are counted per class in audit/package_manifest.json' };
  }
  // No header tiles the file. Two more checks with tools/lib/texture.js, the converter these
  // texture files belong to:
  //  .uexp  - a single Texture2D export filling the file (size - 4), read with the name table of a
  //           per-language header of the same name (<dir>/<lang>/<name>.uasset);
  //  .ubulk - the mip table of the live copy of the package covers the file exactly.
  const lp = rel.replace(/^_(superseded|case_collision)\/[^/]+\//, '');
  const size = fs.statSync(abs).size;
  if (ext === 'uexp') {
    const dir = path.join(ASSETS, path.dirname(lp)), name = path.basename(lp, '.uexp');
    const uexp = fs.readFileSync(abs);
    for (const d of fs.existsSync(dir) ? fs.readdirSync(dir, { withFileTypes: true }).filter((x) => x.isDirectory()).map((x) => x.name).sort() : []) {
      const h = path.join(dir, d, name + '.uasset');
      if (!fs.existsSync(h)) continue;
      try {
        const pkg = UA.parseHeader(fs.readFileSync(h));
        if (pkg.exports.length !== 1 || UA.objectName(pkg, pkg.exports[0].classIndex) !== 'Texture2D') continue;
        const { tex } = TX.parseExport(pkg, uexp, { ...pkg.exports[0], serialSize: uexp.length - 4 });
        if (tex.trailing.length || uexp.readUInt32LE(uexp.length - 4) !== UA.TAG) continue;
        const pd = tex.platforms[0];
        return { format: 'cooked Texture2D export data (.uexp) with no header in any pak', identified_by: `tools/lib/texture.js reads the whole file as one Texture2D export (${pd.pixelFormat.s} ${pd.sizeX}x${pd.sizeY}, ${pd.mips.length} mip) followed by the package tag, using the name table of ${path.relative(ASSETS, h).replace(/\\/g, '/')}; that header's own export is a different size, so it is not this file's header`, decoded: null, uninterpreted_bytes: 0, note: 'structure read by the texture converter; no header exists to rebuild a package from' };
      } catch (e) { /* next language */ }
    }
  } else {
    const h = path.join(ASSETS, lp.replace(/\.ubulk$/i, '.uasset'));
    if (fs.existsSync(h) && fs.existsSync(h.replace(/\.uasset$/, '.uexp'))) {
      try {
        const pkg = UA.parseHeader(fs.readFileSync(h)); const uexp = fs.readFileSync(h.replace(/\.uasset$/, '.uexp'));
        let end = 0, mips = 0, fmt = '';
        for (const e of pkg.exports) { if (UA.objectName(pkg, e.classIndex) !== 'Texture2D') continue; const { tex } = TX.parseExport(pkg, uexp, e); fmt = tex.platforms[0].pixelFormat.s; for (const m of tex.platforms[0].mips) if (m.storage === 'ubulk') { end = Math.max(end, m.dataOffset + m.sizeOnDisk); mips++; } }
        if (end === size) return { format: 'cooked texture bulk data (.ubulk), superseded copy', identified_by: `the mip table of the live copy of the package (${lp.replace(/\.ubulk$/i, '.uasset')}) places ${mips} ${fmt} mips in the .ubulk ending exactly at this file's size`, decoded: null, uninterpreted_bytes: 0, note: 'pixel data of the mips named by that table (tools/lib/texture.js); only the .ubulk was replaced by the patch pak' };
      } catch (e) { /* fall through */ }
    }
  }
  const why = live
    ? 'no .uasset or .umap with this name exists in any pak, so the name, import and export tables its indexes refer to are not available'
    : 'superseded copy from a lower-priority pak whose .uasset was not superseded (the live .uasset has the same bytes in both paks or belongs to the newer .uexp); its header tables are those of the live package only if that header describes this file';
  return { format: ext === 'uexp' ? 'cooked package export data without its header (.uexp)' : 'cooked package bulk data without its header (.ubulk)', identified_by: 'file extension; ' + (ext === 'uexp' ? 'ends with the package tag C1 83 2A 9E: ' + (head4(abs) === '9e2a83c1') : 'raw bulk payload'), decoded: null, uninterpreted_bytes: fs.statSync(abs).size, uninterpreted_reason: why };
}
const head4 = (abs) => { const fd = fs.openSync(abs, 'r'); const n = fs.fstatSync(fd).size; const b = Buffer.alloc(4); if (n >= 4) fs.readSync(fd, b, 0, 4, n - 4); fs.closeSync(fd); return b.readUInt32LE(0).toString(16); };

const HANDLERS = {
  ini: text, txt: text, json: text, uplugin: text, uproject: text, tps: text, vdf: text,
  res: icu, dict: icu, brk: icu, icu, nrm: icu, cfu: icu, cnv: icu,
  ttf: sfnt, ufont: sfnt, png, h264: stream, hca: stream, m1v: stream, locres, locmeta, bin, uexp: orphan, ubulk: orphan,
};

const all = [];
walk(ASSETS, '', all);
all.sort();
const have = new Set(all.map((p) => p.toLowerCase()));
const rows = [], formats = {};
const totals = { files_under_assets: all.length, package_files: 0, package_data_files: 0, orphan_package_data_files: 0, loose_files: 0, loose_bytes: 0, identified: 0, decoded_with_roundtrip: 0, roundtrip_identical: 0, unidentified: 0, uninterpreted_bytes: 0 };
const started = Date.now();
for (const rel of all) {
  const ext = (/\.([^./]+)$/.exec(rel) || [0, ''])[1].toLowerCase();
  if (ext === 'uasset' || ext === 'umap') { totals.package_files++; continue; }
  if (ext === 'uexp' || ext === 'ubulk') {
    const base = rel.slice(0, -ext.length).toLowerCase();
    if (have.has(base + 'uasset') || have.has(base + 'umap')) { totals.package_data_files++; continue; }
    totals.orphan_package_data_files++;
  }
  const abs = path.join(ASSETS, rel);
  const size = fs.statSync(abs).size;
  const row = { path: rel, size };
  totals.loose_files++; totals.loose_bytes += size;
  try {
    const h = HANDLERS[ext];
    if (!h) throw new Error('no handler for extension .' + ext);
    Object.assign(row, size === 0 ? { format: 'empty file', identified_by: 'size 0' } : h(abs, rel, ext, size) || {});
    totals.identified++;
    if (row.roundtrip_identical !== undefined) { totals.decoded_with_roundtrip++; if (row.roundtrip_identical) totals.roundtrip_identical++; }
    totals.uninterpreted_bytes += row.uninterpreted_bytes || 0;
  } catch (e) { row.format = 'unidentified'; row.error = String(e.message).slice(0, 300); totals.unidentified++; totals.uninterpreted_bytes += size; }
  const f = (formats[row.format] = formats[row.format] || { files: 0, bytes: 0, extensions: {}, identified_by: row.identified_by, decoded: 0, roundtrip_identical: 0, example: rel });
  f.files++; f.bytes += size; f.extensions[ext] = (f.extensions[ext] || 0) + 1;
  if (row.roundtrip_identical !== undefined) { f.decoded++; if (row.roundtrip_identical) f.roundtrip_identical++; }
  rows.push(row);
}
writeJson(OUT, { generated_by: 'tools/loose_files.js', version: VERSION, seconds: Math.round((Date.now() - started) / 1000), totals, formats: Object.fromEntries(Object.entries(formats).sort((a, b) => b[1].bytes - a[1].bytes)), files: rows });
logRun({ tool: 'loose_files', version: VERSION, outputs: [OUT], note: `${totals.loose_files} loose files, ${totals.identified} identified, ${totals.decoded_with_roundtrip} decoded, ${totals.roundtrip_identical} round-trip identical, ${totals.unidentified} unidentified` });
console.log(JSON.stringify(totals, null, 1));
for (const [k, f] of Object.entries(formats)) console.log(String(f.files).padStart(6), String(f.bytes).padStart(13), k, f.decoded ? `decoded ${f.decoded}, identical ${f.roundtrip_identical}` : '');
