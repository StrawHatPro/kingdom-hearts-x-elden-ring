// Validation: for every package under assets/, read assets_converted/packages/<path>.json,
// rebuild the .uasset + .uexp from it and compare with the originals; independently decode the
// originals again for the per-class byte accounting.
//
//   node tools/package_check.js [--workers N] [--fresh]
//
// Output: audit/package_manifest.json - per class: files, exports, decoded, round-trip identical,
// uninterpreted bytes (target 0), third-party and delegated bytes. A package without a JSON file
// counts as not converted. Resumable (build/pkg_tmp/check/). Nothing under assets/ is written.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { ROOT, logRun, writeJson } = require('./lib/log');
const B = require('./lib/pkg_batch');
const P = require('./lib/package');

const VERSION = '1.0.0';
const TMP = path.join(ROOT, 'build', 'pkg_tmp', 'check');
const OUT_ROOT = path.join(ROOT, 'assets_converted', 'packages');
const BASES = path.join(ROOT, 'audit', 'blueprint_class_bases.json');
const sha256 = (b) => crypto.createHash('sha256').update(b).digest('hex');

function checkOne(entry) {
  // entry: a package path, or "orphan.uexp|header" for a .uexp paired with a header elsewhere
  const { rel, header, uexp: uexpPath, headerRel } = B.entryPaths(entry);
  const uasset = fs.readFileSync(header);
  const uexp = fs.existsSync(uexpPath) ? fs.readFileSync(uexpPath) : null;
  const { stats } = P.decode(uasset, uexp, { altHeaders: () => B.altHeaders(TMP, entry) });
  const jsonPath = path.join(OUT_ROOT, rel + '.json');
  let rt = false, why;
  if (!fs.existsSync(jsonPath)) why = 'not converted (no JSON file)';
  else {
    const doc = P.parseDoc(fs.readFileSync(jsonPath));
    const back = P.encode(doc, {
      delegatedBytes: (i, d, inner) => {
        const D = inner || doc;
        const e = D.exports[i - 1];
        const s = e.serialOffset - D.summary.totalHeaderSize;
        const b = uexp.subarray(s, s + e.serialSize);
        if (d && (b.length !== d.size || sha256(b) !== d.sha256)) throw new Error('delegated export does not match its recorded hash');
        return b;
      },
    });
    const okA = back.uasset.equals(uasset);
    const okE = uexp === null ? back.uexp === null : back.uexp !== null && back.uexp.equals(uexp);
    rt = okA && okE;
    if (!rt) why = `uasset ${okA ? 'identical' : 'differs'}, uexp ${okE ? 'identical' : 'differs'}`;
  }
  return B.packageRow(rel, stats, { rt, why, sha: sha256(uasset), entry: headerRel ? entry : undefined });
}

function merge(rows, info) {
  const agg = B.aggregate(rows);
  const OUT = path.join(ROOT, 'audit', 'package_manifest.json');
  const notDecoded = Object.entries(agg.classes).filter(([, c]) => c.uninterpreted_bytes > 0).map(([k, c]) => ({ class: k, exports: c.exports, uninterpreted_bytes: c.uninterpreted_bytes, reasons: c.errors || null, undecoded_kinds: c.undecoded_kinds || null }));
  writeJson(OUT, {
    generated_by: 'tools/package_check.js', version: VERSION, ...info, json_root: 'assets_converted/packages/',
    columns: {
      files: 'packages containing an export of the class', files_roundtrip_identical: 'of those, packages whose JSON rebuilds .uasset and .uexp byte for byte',
      exports: 'exports of the class', bytes: 'serialized bytes of those exports', decoded: 'exports with zero uninterpreted bytes',
      roundtrip_identical: 'exports in packages that rebuild identically', uninterpreted_bytes: 'bytes carried as raw hex without a known layout (target 0)',
      third_party_bytes: 'payloads in a named third-party format kept verbatim', delegated_bytes: 'export bytes decoded by another converter of this project',
    },
    totals: agg.totals, third_party_formats: agg.third_party_formats,
    not_fully_decoded: notDecoded, roundtrip_different: agg.different, fatal: agg.fatal, export_table_mismatch: agg.export_table_mismatch, decoded_with_another_header: agg.decoded_with_another_header,
    classes: agg.classes,
  });
  logRun({ tool: 'package_check', version: VERSION, outputs: [OUT], note: `${agg.totals.packages} packages, ${agg.totals.roundtrip_identical} identical, ${agg.totals.roundtrip_different} different, ${agg.totals.fatal} fatal, ${agg.totals.uninterpreted_bytes} uninterpreted bytes` });
  console.log(JSON.stringify({ ...info, totals: agg.totals, third_party_formats: agg.third_party_formats, classes_not_fully_decoded: notDecoded.length }, null, 1));
  if (agg.totals.roundtrip_different || agg.totals.fatal) process.exitCode = 1;
}

const args = B.parseArgs(process.argv.slice(2), { redo: '' });
if (fs.existsSync(BASES)) P.setBases(JSON.parse(fs.readFileSync(BASES, 'utf8')));
if (args.worker !== undefined) B.runWorker(TMP, checkOne, args.redo);
else B.runParent(__filename, TMP, args, args.redo ? ['--redo', args.redo] : [], merge);
