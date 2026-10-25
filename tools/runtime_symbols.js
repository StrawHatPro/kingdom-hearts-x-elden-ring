// From Ghidra's function map (build/ghidra/functions.tsv) derive what the lowering and the
// harness need to know about the original's runtime:
//   externs      names the compiler may reference on its own -> address of the original's copy
//   noreturn     functions that never return (a call to one ends a test run)
//   transparent  helpers that are not part of a function's behaviour and are skipped by the
//                harness: stack probe and stack-cookie check
// Writes build/ghidra/runtime_symbols.json and a committed copy in audit/.
const fs = require('fs');
const path = require('path');
const { ROOT, logRun, writeJson } = require('./lib/log');

const VERSION = '1.1.0';
const rows = fs.readFileSync(path.join(ROOT, 'build', 'ghidra', 'functions.tsv'), 'utf8').split('\n').slice(1).filter(Boolean).map((l) => l.split('\t'));
const out = { externs: {}, noreturn: [], transparent: [], models: [], evidence: {} };
const WANT = ['memcpy', 'memset', 'memmove', 'memcmp', 'strlen', 'wcslen'];
for (const [entry, name, source, thunk, noreturn, library] of rows) {
  const addr = '0x' + entry;
  if (noreturn === '1') out.noreturn.push(addr);
  const bare = name.replace(/^.*::/, '');
  if (name === '_alloca_probe' || name === '__chkstk') {
    out.externs.__chkstk = addr; out.externs._alloca_probe = addr; out.transparent.push(addr);
    out.evidence[addr] = `${name} (${library === '1' ? 'Ghidra Function ID' : source})`;
  }
  // modelled helpers (kind numbers are the MODEL_ constants in tools/native/harness.c)
  if (name === '_Init_thread_header' && library === '1') { out.models.push({ addr, kind: 1 }); out.evidence[addr] = name + ' (Ghidra Function ID)'; }
  if (name === '__security_check_cookie') { out.transparent.push(addr); out.evidence[addr] = `${name} (${library === '1' ? 'Ghidra Function ID' : source})`; }
  // a thunk to an imported C runtime function: jmp [__imp_memcpy]
  if (thunk.startsWith('ext:')) {
    const ext = thunk.slice(4).replace(/^.*::/, '');
    if (WANT.includes(ext) && out.externs[ext] === undefined) { out.externs[ext] = addr; out.evidence[addr] = `thunk to import ${thunk.slice(4)}`; }
  }
  if (WANT.includes(bare) && library === '1' && out.externs[bare] === undefined) { out.externs[bare] = addr; out.evidence[addr] = `${name} (Ghidra Function ID)`; }
}
// C runtime helpers the harness carries out for real on sandbox memory (kinds as in harness.c)
const MODEL_KIND = { memset: 2, memcpy: 3, memmove: 3, memcmp: 4, strlen: 5, wcslen: 6 };
for (const [name, kind] of Object.entries(MODEL_KIND)) if (out.externs[name]) out.models.push({ addr: out.externs[name], kind });
// Data models (kinds 16+): globals whose run-time invariant the harness's rescue vectors keep.
// Found from the original's code, not from names: see tools/lib/data_models.js.
{ const dm = require('./lib/data_models').findDataModels(); out.models.push(...dm.models); Object.assign(out.evidence, dm.evidence); }
const OUT = path.join(ROOT, 'audit', 'runtime_symbols.json');
writeJson(OUT, out);
fs.copyFileSync(OUT, path.join(ROOT, 'build', 'ghidra', 'runtime_symbols.json'));
logRun({ tool: 'runtime_symbols', version: VERSION, outputs: [OUT] });
console.log('externs', JSON.stringify(out.externs), 'noreturn', out.noreturn.length, 'transparent', JSON.stringify(out.transparent));
