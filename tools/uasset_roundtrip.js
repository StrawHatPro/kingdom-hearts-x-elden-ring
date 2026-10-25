// Prove tools/lib/uasset.js reads the package header exactly: parse then serialize every
// .uasset/.umap under a mounted prefix and compare with the original bytes.
//   node tools/uasset_roundtrip.js [mounted path prefix]     default: all packages in the paks
const fs = require('fs');
const path = require('path');
const { ROOT, logRun, writeJson } = require('./lib/log');
const pak = require('./lib/pak');
const ua = require('./lib/uasset');

const VERSION = '1.0.0';
const prefix = process.argv[2] || '';
const OUT = path.join(ROOT, 'audit', 'uasset_roundtrip.json');
const files = pak.mountAll();
const fds = new Map();
const res = { prefix, packages: 0, identical: 0, different: 0, parse_errors: 0, export_classes: {}, engine_versions: {}, errors: [] };
for (const [p, { pak: pakFile, entry }] of files) {
  if (!p.startsWith(prefix) || !/\.(uasset|umap)$/.test(p)) continue;
  if (!fds.has(pakFile)) fds.set(pakFile, fs.openSync(pakFile, 'r'));
  const data = pak.extract(fds.get(pakFile), entry);
  res.packages++;
  try {
    const pkg = ua.parseHeader(data);
    const again = ua.serializeHeader(pkg);
    if (again.equals(data)) res.identical++;
    else { res.different++; if (res.errors.length < 20) res.errors.push({ path: p, error: `serialized ${again.length} bytes, original ${data.length}` }); }
    for (const e of pkg.exports) { const c = ua.objectName(pkg, e.classIndex) || '(class)'; res.export_classes[c] = (res.export_classes[c] || 0) + 1; }
    const v = pkg.summary.savedByEngineVersion;
    const key = `${v.major}.${v.minor}.${v.patch}-${v.changelist} ue4=${pkg.summary.fileVersionUE4}`;
    res.engine_versions[key] = (res.engine_versions[key] || 0) + 1;
  } catch (e) {
    res.parse_errors++;
    if (res.errors.length < 20) res.errors.push({ path: p, error: String(e.message) });
  }
  if (res.packages % 20000 === 0) process.stdout.write(`\r${res.packages}`);
}
for (const fd of fds.values()) fs.closeSync(fd);
res.export_classes = Object.fromEntries(Object.entries(res.export_classes).sort((a, b) => b[1] - a[1]));
if (!prefix) { writeJson(OUT, res); logRun({ tool: 'uasset_roundtrip', version: VERSION, outputs: [OUT] }); }
const { export_classes, ...show } = res;
console.log('\n' + JSON.stringify(show, null, 1));
console.log('classes:', Object.entries(export_classes).slice(0, 40).map(([k, v]) => `${k}:${v}`).join(' '));
