#!/usr/bin/env node
// (PACKGEN) audit/blueprint_class_bases.json from EVERY package of the player's paks.
//
//   node tools/packgen_bases.js
//
// tools/package_convert.js decodes a blueprint's exports with the serializer of the blueprint class's native base, which it
// reads from audit/blueprint_class_bases.json. tools/asset_census.js makes that file from the packages under assets/; the
// asset-pack build extracts only the folders it reads (tools/packgen_extract.js), so a blueprint whose parent blueprint
// lives elsewhere would lose its native base. This tool reads the header of every .uasset / .umap in the paks (through
// tools/lib/pak.js, nothing is written under assets/) and resolves every exported class to its native base with
// tools/asset_census.js's own rule, so the decoded packages are the same as from a full extraction.
'use strict';
const fs = require('fs');
const path = require('path');
const { ROOT, writeJson } = require('./lib/log');
const pak = require('./lib/pak');
const ua = require('./lib/uasset');

function classOf(pkg, idx) { // as tools/asset_census.js
  if (idx === 0) return { name: '(UClass)', from: '' };
  if (idx > 0) return { name: ua.nameOf(pkg, pkg.exports[idx - 1].objectName), from: '(this package)' };
  const imp = pkg.imports[-idx - 1];
  let outer = imp;
  while (outer.outerIndex < 0) outer = pkg.imports[-outer.outerIndex - 1];
  return { name: ua.nameOf(pkg, imp.objectName), from: outer === imp ? '' : ua.nameOf(pkg, outer.objectName) };
}

const t0 = Date.now();
const files = pak.mountAll();
const fds = new Map();
const supers = new Map();
let n = 0, errors = 0;
for (const [p, { pak: pakFile, entry }] of [...files].sort((a, b) => (a[0] < b[0] ? -1 : 1))) {
  if (!/\.(uasset|umap)$/i.test(p)) continue;
  if (!fds.has(pakFile)) fds.set(pakFile, fs.openSync(pakFile, 'r'));
  let pkg;
  try { pkg = ua.parseHeader(pak.extract(fds.get(pakFile), entry)); } catch { errors++; continue; }
  n++;
  pkg.exports.forEach((e) => {
    const c = classOf(pkg, e.classIndex);
    if (e.classIndex !== 0 && e.superIndex !== 0 && /Class$/.test(c.name)) {
      const s = classOf(pkg, e.superIndex);
      const cls = ua.nameOf(pkg, e.objectName);
      let m = supers.get(cls); if (!m) supers.set(cls, (m = new Map()));
      const key = s.name + '\0' + s.from + '\0' + c.name; m.set(key, (m.get(key) || 0) + 1);
    }
  });
}
for (const fd of fds.values()) fs.closeSync(fd);
const nativeBase = (name, depth = 0) => { // as tools/asset_census.js
  const m = supers.get(name);
  if (!m || depth > 32) return null;
  const bases = new Set();
  for (const key of m.keys()) {
    const [sup, from] = key.split('\0');
    if (from.startsWith('/Script/')) bases.add(sup); else { const b = nativeBase(sup, depth + 1); bases.add(b || '?' + sup); }
  }
  return bases.size === 1 ? [...bases][0] : [...bases].sort().join('|');
};
const bases = {};
for (const name of [...supers.keys()].sort()) { const b = nativeBase(name); if (b) bases[name] = b; }
const OUT = path.join(ROOT, 'audit', 'blueprint_class_bases.json');
writeJson(OUT, bases);
console.log(`packgen_bases: ${n} package headers (${errors} unreadable), ${Object.keys(bases).length} classes -> audit/blueprint_class_bases.json (${((Date.now() - t0) / 1000).toFixed(0)} s)`);
