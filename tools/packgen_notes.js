#!/usr/bin/env node
// (PACKGEN) DEVELOPMENT TOOL: writes tools/packgen/notes/, the research notes the asset-pack build on a player's PC needs
// besides the player's own KINGDOM HEARTS III files.
//
//   node tools/packgen_notes.js [--capture SANDBOX]
//
// A note is OUR research, reduced to what the pack tools read: function addresses, names and verdicts from the reading of
// the executable, call sites with their constant arguments, the sound assets other research passes asked for (paths),
// the Square Enix effect modules' semantics we wrote. No game asset data and no game text: tools/packgen.js `scan-notes`
// (and release-tools/scan.js) refuse Japanese text in them. Each note's file is the path it is copied to, relative to the
// repository root (tools/packgen.js stages tools/packgen/notes/<path> to <path>).
//
// Notes made here (from this development tree):
//   build/sora_tmp/audio/code/fx_cache.json   the decompilation's function index, ONLY the entries the audio research
//                                             script build_rows.js reads (captured: --capture)
//   build/sora_tmp/audio/code/ui_sites.json   UI sound call sites in the executable (address, caller, constant id)
//   build/sora_tmp/audio/code/vcall_260.json  virtual calls of UTresUISubParts vf#76 with their constant ids
//   build/sora_tmp/ui/as_invoke_map.json      ActionScript invoke names per native function, ONLY the functions
//                                             build_rows.js asks about (captured)
//   build/sora_tmp/vfx/fxsys/sqex_modules.json the Square Enix particle modules: semantics and property meanings
//                                             (the fields tools/sora_vfx_pack.js reads)
//   audit/sora_port/audio/requests_<strand>.json the sound asset paths each research strand asked for, in the order
//                                             tools/sora_audio_pack.js reads them
// --capture SANDBOX: SANDBOX is a tree where build/sora_tmp/audio/code holds the research scripts and their regenerated
// inputs (tools/packgen.js research run there); build_rows.js is run there once, instrumented, to record which function
// index entries and which ActionScript lookups it makes. Without --capture the last capture (tools/packgen/notes/
// .capture.json) is used.
'use strict';
const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const { ROOT } = require('./lib/log');

const NOTES = path.join(ROOT, 'tools', 'packgen', 'notes');
const CAPTURE = path.join(NOTES, '.capture.json');
const args = process.argv.slice(2);
const opt = (k) => { const i = args.indexOf(k); return i >= 0 ? args[i + 1] : null; };
const readJson = (f) => JSON.parse(fs.readFileSync(f, 'utf8'));
const write = (rel, text) => { const f = path.join(NOTES, rel); fs.mkdirSync(path.dirname(f), { recursive: true }); fs.writeFileSync(f, text); console.log(`  ${rel} (${text.length} bytes)`); };

// ---- capture: which fx_cache entries and which ActionScript lookups build_rows.js makes
if (opt('--capture')) {
  const dir = path.join(path.resolve(opt('--capture')), 'build', 'sora_tmp', 'audio', 'code');
  const fx = path.join(dir, 'fx.js'), br = path.join(dir, 'build_rows.js');
  const fxText = fs.readFileSync(fx, 'utf8'), brText = fs.readFileSync(br, 'utf8');
  const LOAD = "let D;function load(){if(D)return D;D=fs.existsSync(CF)?JSON.parse(fs.readFileSync(CF,'utf8')):build();return D;}";
  const HINT = 'const asHint = (fn) => { const x = AS_REV[String(fn).toLowerCase()];';
  if (!fxText.includes(LOAD) || !brText.includes(HINT)) throw new Error('capture: fx.js / build_rows.js are not the versions this tool knows');
  const out = path.join(dir, '.packgen_capture.json');
  try {
    fs.writeFileSync(fx, 'globalThis.__K=globalThis.__K||{F:new Set(),AS:new Set()};\n' + fxText.replace(LOAD, "let D;function load(){if(D)return D;const raw=fs.existsSync(CF)?JSON.parse(fs.readFileSync(CF,'utf8')):build();D={F:new Proxy(raw.F,{get(o,k){if(typeof k==='string')globalThis.__K.F.add(k);return o[k];}}),C:raw.C};return D;}")
      + `\nprocess.on('exit',()=>require('fs').writeFileSync(${JSON.stringify(out)},JSON.stringify({F:[...globalThis.__K.F],AS:[...globalThis.__K.AS]})));\n`);
    fs.writeFileSync(br, brText.replace(HINT, 'globalThis.__K=globalThis.__K||{F:new Set(),AS:new Set()};const asHint = (fn) => { globalThis.__K.AS.add(String(fn).toLowerCase()); const x = AS_REV[String(fn).toLowerCase()];'));
    const r = cp.spawnSync(process.execPath, ['build_rows.js'], { cwd: dir, encoding: 'utf8' });
    if (r.status !== 0) throw new Error('capture: build_rows.js failed: ' + r.stderr);
  } finally { fs.writeFileSync(fx, fxText); fs.writeFileSync(br, brText); }
  const k = readJson(out); fs.unlinkSync(out);
  fs.mkdirSync(NOTES, { recursive: true });
  fs.writeFileSync(CAPTURE, JSON.stringify({ about: 'tools/packgen_notes.js --capture: the fx_cache keys and ActionScript lookups build_rows.js made', F: k.F.sort(), AS: k.AS.sort() }, null, 1) + '\n');
  console.log(`capture: ${k.F.length} function index entries, ${k.AS.length} ActionScript lookups`);
}
if (!fs.existsSync(CAPTURE)) { console.error('no capture yet: run with --capture SANDBOX first'); process.exit(1); }
const cap = readJson(CAPTURE);
const T = (p) => path.join(ROOT, p);
console.log('packgen_notes -> tools/packgen/notes/');

// fx_cache: F entries build_rows.js reads; no call graph (build_rows.js does not use it)
{
  const fx = readJson(T('build/sora_tmp/audio/code/fx_cache.json'));
  const F = {}; for (const key of cap.F) if (Object.prototype.hasOwnProperty.call(fx.F, key)) F[key] = fx.F[key];
  write('build/sora_tmp/audio/code/fx_cache.json', JSON.stringify({ F, C: {} }));
}
for (const f of ['ui_sites.json', 'vcall_260.json']) write('build/sora_tmp/audio/code/' + f, fs.readFileSync(T('build/sora_tmp/audio/code/' + f), 'utf8'));
// as_invoke_map: per invoke name only the functions build_rows.js asks about (key order kept: the Set order of each lookup)
{
  const j = readJson(T('build/sora_tmp/ui/as_invoke_map.json'));
  const want = new Set(cap.AS); const out = {};
  for (const name of Object.keys(j)) {
    if (!Array.isArray(j[name]) || name.includes('.') || name.includes('/')) continue;
    const groups = j[name].map((g) => ({ fns: (g.fns || []).filter((fn) => want.has(String(fn[0]).toLowerCase())).map((fn) => [fn[0]]) })).filter((g) => g.fns.length);
    if (groups.length) out[name] = groups;
  }
  write('build/sora_tmp/ui/as_invoke_map.json', JSON.stringify(out));
}
// sqex_modules: the fields tools/sora_vfx_pack.js sqexInfo() reads
{
  const j = readJson(T('build/sora_tmp/vfx/fxsys/sqex_modules.json'));
  if (!Array.isArray(j)) throw new Error('sqex_modules.json: expected an array');
  const keep = ['class', 'id', 'name', 'semantics', 'summary', 'note', 'description', 'confidence'];
  const out = j.map((r) => {
    const o = {}; for (const k of keep) if (r[k] !== undefined) o[k] = r[k];
    if (Array.isArray(r.properties)) o.properties = r.properties.map((q) => (q.meaning ? { name: q.name, meaning: q.meaning } : { name: q.name }));
    return o;
  });
  write('build/sora_tmp/vfx/fxsys/sqex_modules.json', JSON.stringify(out, null, 1));
}
// requests: the sound asset paths, in the order requestedAssets() walks them
{
  const re = /^\/Game\/Sound\/|Content\/Sound\//;
  for (const strand of ['actions', 'combat', 'magic', 'ui']) {
    const j = readJson(T(`audit/sora_port/audio/requests_${strand}.json`)); const list = [];
    const walk = (v) => { if (typeof v === 'string') { if (re.test(v)) list.push(v); } else if (Array.isArray(v)) v.forEach(walk); else if (v && typeof v === 'object') for (const k of Object.keys(v)) walk(v[k]); };
    walk(j);
    write(`audit/sora_port/audio/requests_${strand}.json`, JSON.stringify({ about: `the sound assets the ${strand} research asked the audio pack for (tools/packgen_notes.js)`, requests: list }, null, 1) + '\n');
  }
}
// no game text
const JP = /[　-ヿ㐀-鿿豈-﫿＀-￯]/;
const bad = [];
(function walk(d) { for (const e of fs.readdirSync(d, { withFileTypes: true })) { const p = path.join(d, e.name); if (e.isDirectory()) walk(p); else if (JP.test(fs.readFileSync(p, 'utf8'))) bad.push(path.relative(NOTES, p)); } })(NOTES);
if (bad.length) { console.error('Japanese text in notes: ' + bad.join(', ')); process.exit(1); }
console.log('packgen_notes: done (no Japanese text)');
