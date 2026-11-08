// (KH3TEXT 2026-11-08) NO KH3 GAME TEXT IN THE BINARIES: the release checker's text scan (release-tools/make_release.js
// 3b, "game text lines") run on the dev tree's built DLL, plus a stricter pass. KH3's text is copyrighted: the mod reads it
// from the PLAYER's own UI pack at run time (build/sora_pack/ui/strings/en, made by the setup from the player's KH3 install),
// it is never compiled in.
//
//   node tools/kh3_text_scan.js [binary ...] [--pack DIR] [--loose]
//
// Default binary: target/release/kh_er.dll. Exit 1 on any hit.
//   RELEASE pass (make_release's exact rule): every pack line that is a sentence (>= 24 characters after <br> -> space and
//     the markup removed, and ending in . ! ? or six words or more; `*_name*` keys skipped) is a reference; its first 40
//     characters must not be in the binary (latin1).
//   STRICT pass (on by default; --loose drops it): the same references, but the binary's text is normalised the same way
//     (<br> -> space, <...> markup removed, whitespace collapsed) before the probe is looked for, and every sentence of a
//     reference line (split at . ! ? followed by a space) of 24 characters or more is a probe of its own. This catches a
//     KH3 line compiled with its markup, or a KH3 sentence inside a composed line.
'use strict';
const fs = require('fs');
const path = require('path');
const ROOT = path.resolve(__dirname, '..');

const args = process.argv.slice(2);
let pack = path.join(ROOT, 'build', 'sora_pack');
let strict = true;
const bins = [];
for (let k = 0; k < args.length; k++) {
  if (args[k] === '--pack') pack = path.resolve(args[++k]);
  else if (args[k] === '--loose') strict = false;
  else bins.push(path.resolve(args[k]));
}
if (!bins.length) bins.push(path.join(ROOT, 'target', 'release', 'kh_er.dll'));

const norm = (s) => s.replace(/<br>/g, ' ').replace(/<[^>]*>/g, '').replace(/\s+/g, ' ').trim();
// make_release.js 3b: the reference lines
const gameText = new Map(); // stripped line -> its id
const sdir = path.join(pack, 'ui', 'strings', 'en');
if (!fs.existsSync(sdir)) {
  console.error(`kh3_text_scan: no UI pack strings at ${sdir} (the scan needs the pack's English texts as its reference)`);
  process.exit(2);
}
for (const f of fs.readdirSync(sdir).filter((x) => x.endsWith('.json'))) {
  const j = JSON.parse(fs.readFileSync(path.join(sdir, f), 'utf8'));
  for (const [nsName, ns] of Object.entries(j.namespaces || {})) for (const [k, v] of Object.entries(ns)) {
    if (typeof v !== 'string' || /(^|_)name(_|$)/.test(k)) continue;
    const t = norm(v);
    if (t.length >= 24 && (/[.!?]$/.test(t) || t.split(' ').length >= 6) && !gameText.has(t)) gameText.set(t, `${path.basename(f, '.json')}.${nsName}.${k}`);
  }
}

let total = 0;
for (const bin of bins) {
  const b = fs.readFileSync(bin);
  const latin = b.toString('latin1');
  const hits = [];
  for (const [t, id] of gameText) {
    const probe = t.slice(0, 40);
    if (probe.length >= 24 && latin.includes(probe)) hits.push({ pass: 'release', id, probe });
  }
  if (strict) {
    const flat = norm(latin);
    const seen = new Set(hits.map((h) => h.id));
    for (const [t, id] of gameText) {
      if (seen.has(id)) continue;
      const probes = [t.slice(0, 40), ...t.split(/(?<=[.!?])\s+/).filter((s) => s.length >= 24 && s !== t)];
      const hit = probes.find((p) => p.length >= 24 && flat.includes(p));
      if (hit) hits.push({ pass: 'strict', id, probe: hit });
    }
  }
  const rel = path.relative(ROOT, bin);
  console.log(`${rel}: ${(b.length / 1048576).toFixed(1)} MB; reference lines ${gameText.size}; KH3 text hits ${hits.length} (release rule ${hits.filter((h) => h.pass === 'release').length}${strict ? `, strict ${hits.filter((h) => h.pass === 'strict').length}` : ''})`);
  for (const h of hits) console.log(`  [${h.pass}] ${h.id}: "${h.probe}"`);
  total += hits.length;
}
process.exit(total ? 1 : 0);
