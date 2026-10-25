#!/usr/bin/env node
// (PACKGEN) IS THIS THE KH3 EXECUTABLE THE TOOLS KNOW? Several research tools read the game's executable at fixed
// addresses (tools/reflection.js: the engine's registration functions; tools/lib/hca.js: the audio codec's tables; the
// effect class defaults): addresses of the build the tools were made with (KINGDOM HEARTS III + Re Mind, the Steam
// build). This step compares the player's executable's code and constant sections with that build's, by SHA-256
// (tools/packgen/recipes/exe_sections.json: hashes only), so another build stops here with a clear message instead of
// giving wrong research data. The Steam DRM wrapper section (.bind) and the headers are not compared: a build with the
// same code and data at the same addresses (e.g. a store's copy without the wrapper) passes.
//
//   node tools/packgen_exe_check.js [--exe FILE] [--force]     check (default: the executable tools/lib/pe.js reads)
//   node tools/packgen_exe_check.js --record                   (development) write the recipe from base/KH3.exe
'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const pe = require('./lib/pe');

const ROOT = path.resolve(__dirname, '..');
const RECIPE = path.join(ROOT, 'tools', 'packgen', 'recipes', 'exe_sections.json');
const args = process.argv.slice(2);
const opt = (k) => { const i = args.indexOf(k); return i >= 0 ? args[i + 1] : null; };
const SECTIONS = ['.text', '.rdata', '.data', '.pdata'];
const exe = opt('--exe') || pe.BASE_EXE;
if (!fs.existsSync(exe)) { console.error(`packgen_exe_check: ${exe} not found`); process.exit(1); }
const img = pe.load(exe);
const hashes = {};
for (const s of img.sections) if (SECTIONS.includes(s.name)) hashes[s.name] = { rva: '0x' + s.rva.toString(16), raw_size: s.rawSize, sha256: crypto.createHash('sha256').update(img.b.subarray(s.rawPtr, s.rawPtr + s.rawSize)).digest('hex') };
if (args.includes('--record')) {
  fs.mkdirSync(path.dirname(RECIPE), { recursive: true });
  fs.writeFileSync(RECIPE, JSON.stringify({ about: 'tools/packgen_exe_check.js: the SHA-256 of the KH3 executable sections the research tools read at fixed addresses (the build they were made with)', image_base: '0x' + img.imageBase.toString(16), sections: hashes }, null, 1) + '\n');
  console.log(`packgen_exe_check: recorded ${Object.keys(hashes).join(', ')} -> ${path.relative(ROOT, RECIPE)}`);
  process.exit(0);
}
const want = JSON.parse(fs.readFileSync(RECIPE, 'utf8'));
const bad = [];
if ('0x' + img.imageBase.toString(16) !== want.image_base) bad.push(`image base ${'0x' + img.imageBase.toString(16)} (expected ${want.image_base})`);
for (const [name, w] of Object.entries(want.sections)) {
  const h = hashes[name];
  if (!h) bad.push(`no ${name} section`);
  else if (h.rva !== w.rva || h.sha256 !== w.sha256) bad.push(`${name} differs (${h.rva}, ${h.raw_size} bytes; expected ${w.rva}, ${w.raw_size} bytes)`);
}
if (!bad.length) { console.log(`packgen_exe_check: ${path.basename(exe)}: the build the tools know (${Object.keys(want.sections).join(', ')} identical)`); process.exit(0); }
console.error(`packgen_exe_check: ${exe} is not the KH3 build the research tools were made with:`);
for (const b of bad) console.error('   ' + b);
console.error('The tools read this executable at fixed addresses; with another build they would make wrong data. Let Steam / Epic');
console.error('update or verify the game, or report this output to the mod (the addresses must be found again for this build).');
if (args.includes('--force')) { console.error('--force: going on anyway'); process.exit(0); }
process.exit(1);
