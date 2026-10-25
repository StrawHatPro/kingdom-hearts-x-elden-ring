#!/usr/bin/env node
// (PACKGEN) Extracts from the player's own KINGDOM HEARTS III paks exactly the files the asset-pack build reads.
//
//   node tools/packgen_extract.js [--list FILE] [--dry-run]
//
// FILE (default setup/pack_inputs.json): { "folders": [...] } or a plain array of mounted folder paths
// ("KINGDOM HEARTS III/Content/Character/pc/p_ex001/mdl", "Engine/Content/EngineResources", ...). Every file whose folder
// is one of them is extracted (not the subfolders: a subfolder the build reads is listed itself) to assets/<mounted path>,
// decrypted and decompressed by tools/lib/pak.js with the key read from the player's executable (tools/pak_key.js).
// The list is the build's, made from traces of the tools' reads (setup/PACK_PIPELINE.md); it names folders, never files,
// so a game build with a few files more or less in a folder still extracts what that folder has. A file already extracted
// with the same size is kept. Writes build/packgen/extract.json (what came from where).
'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { ROOT } = require('./lib/log');
const pak = require('./lib/pak');

const args = process.argv.slice(2);
const opt = (k) => { const i = args.indexOf(k); return i >= 0 ? args[i + 1] : null; };
const LIST = path.resolve(opt('--list') || path.join(ROOT, 'setup', 'pack_inputs.json'));
const dry = args.includes('--dry-run');
const j = JSON.parse(fs.readFileSync(LIST, 'utf8'));
const folders = new Set((Array.isArray(j) ? j : j.folders).map((f) => f.replace(/\/+$/, '').toLowerCase()));
const files = pak.mountAll();
const fds = new Map();
let n = 0, kept = 0, bytes = 0; const seen = new Set(); const listing = [];
for (const [p, { pak: pakFile, entry }] of files) {
  const dir = p.slice(0, p.lastIndexOf('/')).toLowerCase();
  if (!folders.has(dir)) continue;
  seen.add(dir);
  const out = path.join(ROOT, 'assets', p);
  if (fs.existsSync(out) && fs.statSync(out).size === entry.uncompressedSize) { kept++; continue; }
  if (dry) { n++; continue; }
  if (!fds.has(pakFile)) fds.set(pakFile, fs.openSync(pakFile, 'r'));
  const data = pak.extract(fds.get(pakFile), entry);
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, data);
  listing.push({ path: p, archive: path.basename(pakFile), size: data.length, sha256: crypto.createHash('sha256').update(data).digest('hex') });
  n++; bytes += data.length;
}
for (const fd of fds.values()) fs.closeSync(fd);
const missing = [...folders].filter((f) => !seen.has(f));
fs.mkdirSync(path.join(ROOT, 'build', 'packgen'), { recursive: true });
if (!dry) fs.writeFileSync(path.join(ROOT, 'build', 'packgen', 'extract.json'), JSON.stringify({ list: path.relative(ROOT, LIST).split(path.sep).join('/'), folders: folders.size, folders_not_in_paks: missing, extracted: listing }, null, 1) + '\n');
console.log(`packgen_extract: ${folders.size} folders; ${n} file(s) ${dry ? 'to extract' : `extracted (${(bytes / 1048576).toFixed(0)} MB)`}, ${kept} already there; ${missing.length} folder(s) not in the paks`);
for (const m of missing.slice(0, 20)) console.log('   not in the paks: ' + m);
