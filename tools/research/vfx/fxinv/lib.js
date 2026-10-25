// shared helpers for the fxinv strand
const fs = require('fs'), path = require('path');
const ROOT = path.resolve(__dirname, '../../../..');
const PK = path.join(ROOT, 'assets_converted/packages');
const AN = path.join(ROOT, 'assets_converted/animations');
const KH = 'KINGDOM HEARTS III/Content';
const OUT = __dirname;
function readJson(f) { return JSON.parse(fs.readFileSync(f, 'utf8')); }
function walk(dir, re, out = []) {
  if (!fs.existsSync(dir)) return out;
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walk(p, re, out); else if (!re || re.test(e.name)) out.push(p);
  }
  return out;
}
// "/Game/X/Y.Y" or "/Game/X/Y" -> decoded package json path
function gameToPkgFile(g) { const p = g.replace(/^\/Game\//, '').split('.')[0]; return path.join(PK, KH, p + '.uasset.json'); }
function pkgFileToGame(f) { const r = path.relative(path.join(PK, KH), f).replace(/\\/g, '/').replace(/\.(uasset|umap)\.json$/, ''); return '/Game/' + r; }
function pkgFileToCooked(f) { return path.relative(PK, f).replace(/\\/g, '/').replace(/\.json$/, ''); }
function loadPkg(f) { const d = readJson(f); d.__file = f; return d; }
// full object path of an import/export index
function ref(d, i) {
  if (!i) return null;
  if (i < 0) { let im = d.imports[-i - 1]; const parts = [im.objectName]; while (im.outerIndex) { im = d.imports[-im.outerIndex - 1]; parts.unshift(im.objectName); } return parts.length > 1 ? parts[0] + '.' + parts.slice(1).join('.') : parts[0]; }
  const e = d.exports[i - 1]; return 'E' + i + ':' + e.objectName;
}
// simplify tagged property trees of package json ({name,type,value}) to plain objects
function simp(d, p) {
  if (p === null || typeof p !== 'object') return p;
  if (Array.isArray(p)) {
    if (p.length && p.every((e) => e && typeof e === 'object' && 'name' in e && 'type' in e)) { const o = {}; for (const e of p) o[e.name + (e.arrayIndex ? '[' + e.arrayIndex + ']' : '')] = sv(d, e); return o; }
    return p.map((x) => simp(d, x));
  }
  if ('name' in p && 'type' in p && 'value' in p) return { [p.name]: sv(d, p) };
  const o = {}; for (const k in p) o[k] = simp(d, p[k]); return o;
}
function sv(d, e) {
  if (e.type === 'ObjectProperty') return ref(d, e.value);
  if (e.type === 'ArrayProperty' && e.innerType === 'ObjectProperty' && Array.isArray(e.value)) return e.value.map((x) => (typeof x === 'number' ? ref(d, x) : simp(d, x)));
  return simp(d, e.value);
}
function exportProps(d, e) { return e.data && e.data.props ? simp(d, e.data.props) : {}; }
function exportClass(d, e) { return e.class || ref(d, e.classIndex); }
// all strings in a simplified value, with property paths
function strings(v, pre, out) { out = out || []; pre = pre || ''; if (typeof v === 'string') out.push([pre, v]); else if (Array.isArray(v)) v.forEach((x, i) => strings(x, pre + '[' + i + ']', out)); else if (v && typeof v === 'object') for (const k in v) strings(v[k], pre ? pre + '.' + k : k, out); return out; }
module.exports = { fs, path, ROOT, PK, AN, KH, OUT, readJson, walk, gameToPkgFile, pkgFileToGame, pkgFileToCooked, loadPkg, ref, simp, sv, exportProps, exportClass, strings };
