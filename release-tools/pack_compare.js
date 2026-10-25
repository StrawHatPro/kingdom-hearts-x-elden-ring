#!/usr/bin/env node
// Compares an asset pack (and generated Rust tables) built by the setup with a reference pack, file by file (SHA-256), and
// sorts every difference by its explanation. Used for the clean-room proof (release-tools/cleanroom.js); reads both trees,
// writes only the report.
//
//   node release-tools/pack_compare.js --pack A --ref B [--gen A_GEN --ref-gen B_GEN] [--out report.json]
//
// Classes (first rule that matches):
//   identical          same bytes
//   kh1-voice          the KH1 outfit's own voice (audio/outfits/kh1): not part of the release (KH3's battle voice instead)
//   scrubbed-text      vfx/triggers.json: its notify descriptions come from the release sheets, where Japanese quoted from the
//                      game is replaced with '…' (a description field; the mod does not read it)
//   runtime-data       gen/*.khg, gen/schemas.json: the tables made from KH3 data that a prebuilt DLL reads at start (a development
//                      pack has none: the development DLL has them compiled in)
//   build-record       ui/sheet_rows.json: the UI pack tool's record of its files (rows for the asset sheet; not read by the mod)
//   dev-aid            development-only files (dxil dumps of the test harness)
//   obsolete-variant   shader variants the current DLL no longer loads (*_hy, *_gb, *_hy2, *_gb2 and their hair files) and the
//                      character folders' kh-shaderc index files (the DLL finds the variants by file name)
//   shader-rebuilt     a DXIL built again from the current harness / tables (dxc is deterministic: same inputs, same bytes)
//   effect-table       the effect shading tables and their built shaders, made by the current translator
//   translator-newer   a translated material file the reference pack has from an older translator run
//   other              anything else (listed one by one)
'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const args = process.argv.slice(2);
const opt = (k) => { const i = args.indexOf(k); return i >= 0 ? path.resolve(args[i + 1]) : null; };
const A = opt('--pack'), B = opt('--ref');
if (!A || !B) { console.error('usage: pack_compare.js --pack A --ref B [--gen A --ref-gen B] [--out FILE]'); process.exit(2); }
function walk(root, sub = '', out = new Map()) {
  const d = path.join(root, sub);
  if (!fs.existsSync(d)) return out;
  for (const e of fs.readdirSync(d, { withFileTypes: true })) {
    const r = sub ? sub + '/' + e.name : e.name;
    if (e.isDirectory()) walk(root, r, out); else out.set(r, fs.statSync(path.join(root, r)).size);
  }
  return out;
}
const sha = (f) => crypto.createHash('sha256').update(fs.readFileSync(f)).digest('hex');
const RULES = [
  ['kh1-voice', (r) => /^audio\/outfits\/kh1\//.test(r)],
  ['scrubbed-text', (r) => r === 'vfx/triggers.json'],
  ['runtime-data', (r) => r.startsWith('gen/') && /\.(khg|json)$/.test(r)],
  ['build-record', (r) => r === 'ui/sheet_rows.json'],
  ['dev-aid', (r) => /\/er\/dump\d*\.ps\.dxil$/.test(r)],
  ['obsolete-variant', (r) => /(^|\/)materials[^/]*\/er\/[^/]*_(hy|gb|hy2|gb2)(_[a-z0-9]+)?\.ps\.dxil$/.test(r) || /(^|\/)materials[^/]*\/er\/shaders[^/]*\.json$/.test(r)],
  ['effect-table', (r) => /(^|\/)vfx\/(er\/|shaders\/)/.test(r)],
  ['shader-rebuilt', (r) => /\.dxil$/.test(r)],
  ['translator-newer', (r) => /\/materials\/materials(_[a-z0-9]+)?\.(wgsl|json)$/.test(r) || /^materials\/materials\.(wgsl|json)$/.test(r)],
];
function compare(a, b) {
  const fa = walk(a), fb = walk(b);
  const res = { counts: { identical: 0, differing: 0, missing: 0, extra: 0 }, by_class: {}, other: { differing: [], missing: [], extra: [] } };
  const note = (kind, r) => {
    const cls = (RULES.find(([, t]) => t(r)) || ['other'])[0];
    const k = res.by_class[cls] = res.by_class[cls] || { differing: 0, missing: 0, extra: 0 };
    k[kind]++;
    if (cls === 'other') res.other[kind].push(r);
  };
  for (const [r, s] of fb) {
    if (!fa.has(r)) { res.counts.missing++; note('missing', r); continue; }
    if (fa.get(r) === s && sha(path.join(a, r)) === sha(path.join(b, r))) { res.counts.identical++; continue; }
    res.counts.differing++; note('differing', r);
  }
  for (const r of fa.keys()) if (!fb.has(r)) { res.counts.extra++; note('extra', r); }
  return res;
}
const report = { generated_by: 'release-tools/pack_compare.js', pack: A, ref: B, pack_result: compare(A, B) };
if (opt('--gen') && opt('--ref-gen')) report.gen_result = compare(opt('--gen'), opt('--ref-gen'));
const show = (name, r) => {
  console.log(`${name}: identical ${r.counts.identical}, differing ${r.counts.differing}, missing ${r.counts.missing}, extra ${r.counts.extra}`);
  for (const [cls, k] of Object.entries(r.by_class).sort()) console.log(`   ${cls.padEnd(18)} differing ${k.differing}, missing ${k.missing}, extra ${k.extra}`);
};
show('pack', report.pack_result);
if (report.gen_result) show('gen', report.gen_result);
if (opt('--out')) { fs.mkdirSync(path.dirname(opt('--out')), { recursive: true }); fs.writeFileSync(opt('--out'), JSON.stringify(report, null, 1) + '\n'); console.log('report: ' + opt('--out')); }
