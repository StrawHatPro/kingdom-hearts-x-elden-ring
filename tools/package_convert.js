// Every cooked package under assets/ -> assets_converted/packages/<same relative path>.json
// (header, names, imports, exports, tagged properties, native data per class; tools/lib/package.js).
//
//   node tools/package_convert.js [--workers N] [--fresh]
//
// For each package the JSON text is parsed back and re-encoded before it is written; the row in
// the result records whether that reproduces the .uasset and .uexp byte for byte. Resumable:
// finished chunks under build/pkg_tmp/convert/ are skipped (--fresh starts over).
// Output: audit/package_convert.json (per-class totals). Nothing under assets/ is written.
// tools/package_check.js is the independent re-check from the written files.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { ROOT, logRun, writeJson } = require('./lib/log');
const B = require('./lib/pkg_batch');
const P = require('./lib/package');

const VERSION = '1.0.0';
const TMP = path.join(ROOT, 'build', 'pkg_tmp', 'convert');
const OUT_ROOT = path.join(ROOT, 'assets_converted', 'packages');
const BASES = path.join(ROOT, 'audit', 'blueprint_class_bases.json');
const sha256 = (b) => crypto.createHash('sha256').update(b).digest('hex');

function convertOne(entry) {
  // entry: a package path, or "orphan.uexp|header" for a .uexp paired with a header elsewhere
  const { rel, header, uexp: uexpPath, headerRel } = B.entryPaths(entry);
  const uasset = fs.readFileSync(header);
  const uexp = fs.existsSync(uexpPath) ? fs.readFileSync(uexpPath) : null;
  const { doc, stats } = P.decode(uasset, uexp, { altHeaders: () => B.altHeaders(TMP, entry) });
  if (headerRel) doc.headerFrom = headerRel;
  const text = Buffer.concat(P.stringifyDoc(doc).map((s) => Buffer.from(s, 'utf8')));
  // round trip from the text that is written
  const doc2 = P.parseDoc(text);
  const delegatedBytes = (i, d, inner) => {
    const D = inner || doc2;
    const e = D.exports[i - 1];
    const s = e.serialOffset - D.summary.totalHeaderSize;
    const b = uexp.subarray(s, s + e.serialSize);
    if (d && sha256(b) !== d.sha256) throw new Error('delegated export hash mismatch');
    return b;
  };
  const back = P.encode(doc2, { delegatedBytes });
  const okA = back.uasset.equals(uasset);
  const okE = uexp === null ? back.uexp === null : back.uexp !== null && back.uexp.equals(uexp);
  const out = path.join(OUT_ROOT, rel + '.json');
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, text);
  return B.packageRow(rel, stats, { rt: okA && okE, why: okA && okE ? undefined : `uasset ${okA ? 'identical' : 'differs'}, uexp ${okE ? 'identical' : 'differs'}`, json: text.length, src: (headerRel ? 0 : uasset.length) + (uexp ? uexp.length : 0), entry: headerRel ? entry : undefined });
}

// JSON files under assets_converted/packages/ whose package is no longer in the list (the tree
// under assets/ changed) are removed after a complete run.
function pruneOutputs(rows) {
  const expected = new Set();
  for (const r of rows()) expected.add((r.p + '.json').toLowerCase());
  const res = { removed_json_files: 0, removed_directories: 0 };
  const walk = (dir, rel) => {
    let left = 0;
    for (const d of fs.readdirSync(dir, { withFileTypes: true })) {
      const r = rel ? rel + '/' + d.name : d.name, abs = path.join(dir, d.name);
      if (d.isDirectory()) { if (walk(abs, r) === 0) { fs.rmdirSync(abs); res.removed_directories++; } else left++; }
      else if (expected.has(r.toLowerCase())) left++;
      else { fs.rmSync(abs); res.removed_json_files++; }
    }
    return left;
  };
  if (fs.existsSync(OUT_ROOT)) walk(OUT_ROOT, '');
  return res;
}

function merge(rows, info) {
  const agg = B.aggregate(rows);
  let jsonBytes = 0, srcBytes = 0;
  for (const r of rows()) { jsonBytes += r.json || 0; srcBytes += r.src || 0; }
  info = { ...info, stale_outputs: pruneOutputs(rows) };
  const OUT = path.join(ROOT, 'audit', 'package_convert.json');
  writeJson(OUT, {
    generated_by: 'tools/package_convert.js', version: VERSION, ...info, output_root: 'assets_converted/packages/', source_bytes: srcBytes, json_bytes: jsonBytes,
    columns: {
      decoded: 'exports with zero uninterpreted bytes', uninterpreted_bytes: 'native data without a serializer, property values kept raw, shader parameter blobs',
      third_party_bytes: 'payloads in a named third-party format kept verbatim (see third_party)', delegated_bytes: 'exports decoded by another converter of this project (not embedded)',
    },
    ...agg,
  });
  logRun({ tool: 'package_convert', version: VERSION, outputs: [OUT], note: `${agg.totals.packages} packages, ${agg.totals.roundtrip_identical} round-trip identical, ${agg.totals.uninterpreted_bytes} uninterpreted bytes` });
  console.log(JSON.stringify({ ...info, totals: agg.totals, third_party_formats: agg.third_party_formats, json_bytes: jsonBytes }, null, 1));
}

const args = B.parseArgs(process.argv.slice(2), { redo: '' });
if (fs.existsSync(BASES)) P.setBases(JSON.parse(fs.readFileSync(BASES, 'utf8')));
else if (args.worker === undefined) console.error('warning: audit/blueprint_class_bases.json missing (run tools/asset_census.js first); blueprint instances with native data will stay undecoded');
if (args.worker !== undefined) B.runWorker(TMP, convertOne, args.redo);
else B.runParent(__filename, TMP, args, args.redo ? ['--redo', args.redo] : [], merge);
