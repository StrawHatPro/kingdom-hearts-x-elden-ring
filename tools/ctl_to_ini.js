#!/usr/bin/env node
// (INI) Converts the mod's old settings file kh-er.ctl into kh-er.ini (2026-10-24: the DLL reads kh-er.ini; the old
// kh-er.ctl only while no kh-er.ini exists, crates/kh-er/src/control.rs).
//
//   node tools/ctl_to_ini.js [--dry-run] PATH...
//
//   PATH       a .ctl file (er/mods/kh-er.ctl, er/profiles/kh-sora-zetta.ctl, ...) or a folder holding kh-er.ctl
//   --dry-run  writes nothing: prints the .ini it would write and the check, exit 0 when the check passes
//
// What it does, for each PATH (close Elden Ring first: the DLL reads the file while the game runs):
//   1. reads the .ctl with the rules the DLLs before 2026-10-24 read it with (what the settings ARE today);
//   2. writes <name>.ini beside it: the SAME keys and values, grouped into [Sections] with a comment per known key, the
//      file's own comment lines kept above the key they stood above. Only whole-line "#" comments are written, so a
//      DLL that does not know the .ini rules reads the same settings from it too ([Section] lines have no "=");
//   3. checks the written .ini with the new rules (the same as control.rs `parse`): the same keys, the same values,
//      else nothing is renamed and the exit code is 1;
//   4. renames <name>.ctl to <name>.ctl.bak (.bak2, .bak3 ... when that exists), so the DLL surely reads the .ini.
// IDEMPOTENT: a .ini with no .ctl beside it = already converted (nothing done). A .ini AND a .ctl: when the .ini holds
// the same settings, only step 4 is done; when they differ, nothing is done (exit 1: your .ini is never overwritten).
// Exit codes: 0 done / already converted / dry run passed, 1 a check failed or refused, 2 bad arguments.
'use strict';
const fs = require('fs');
const path = require('path');

// ---- the rules (Rust's str::lines and str::trim, so that both languages cut a line the same way)
const WS = '[\t\n\v\f\r \u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]';
const TRIM = new RegExp('^' + WS + '+|' + WS + '+$', 'g');
const trim = (s) => s.replace(TRIM, '');
const lines = (text) => { const l = text.split('\n').map((s) => (s.endsWith('\r') ? s.slice(0, -1) : s)); if (l.length && l[l.length - 1] === '') l.pop(); return l; };

/** The old rules (kh-er before 2026-10-24): trimmed lines, "#" at the start = comment, key / value split at the first "=". */
function parseOld(text) {
  const map = new Map();
  for (const raw of lines(text)) {
    const line = trim(raw);
    if (!line || line.startsWith('#')) continue;
    const i = line.indexOf('=');
    if (i < 0) continue;
    const k = trim(line.slice(0, i));
    if (!k) continue; // an empty key was stored, but never asked for
    map.set(k, trim(line.slice(i + 1)));
  }
  return map;
}

const isSpace = (c) => c === ' ' || c === '\t';
/** The text before an inline comment (a "#" or ";" after a space or a tab), trimmed. */
function stripInline(s) {
  let prev = false;
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if ((c === '#' || c === ';') && prev) return trim(s.slice(0, i));
    prev = isSpace(c);
  }
  return trim(s);
}
function value(raw) {
  const v = trim(raw);
  if (v.startsWith('"')) {
    const end = v.indexOf('"', 1);
    if (end > 0) {
      const after = trim(v.slice(end + 1));
      if (!after || after.startsWith('#') || after.startsWith(';')) return v.slice(1, end);
    }
  }
  return stripInline(raw);
}
/** The new rules: control.rs `parse` (the same answers: crates/kh-er/src/control.rs ini_tests). */
function parseIni(text) {
  if (text.startsWith('﻿')) text = text.slice(1);
  const map = new Map();
  for (const raw of lines(text)) {
    const line = trim(raw);
    if (!line || line.startsWith('#') || line.startsWith(';')) continue;
    if (line.startsWith('[') && stripInline(line).endsWith(']')) continue;
    const i = line.indexOf('=');
    if (i < 0) continue;
    const k = trim(line.slice(0, i));
    if (!k) continue;
    map.set(k, value(line.slice(i + 1)));
  }
  return map;
}

/** A file's text: UTF-8 (a BOM dropped) or UTF-16 with its BOM. */
function readText(file) {
  const b = fs.readFileSync(file);
  if (b[0] === 0xef && b[1] === 0xbb && b[2] === 0xbf) return { text: b.subarray(3).toString('utf8'), bom: 'UTF-8 BOM' };
  if (b[0] === 0xff && b[1] === 0xfe) return { text: b.subarray(2).toString('utf16le'), bom: 'UTF-16 LE' };
  if (b[0] === 0xfe && b[1] === 0xff) { const s = Buffer.from(b.subarray(2)); s.swap16(); return { text: s.toString('utf16le'), bom: 'UTF-16 BE' }; }
  return { text: b.toString('utf8'), bom: null };
}

// ---- the layout: sections by key, and a comment per common key
const SECTIONS = [
  ['Mod', 'The mod itself.', (k) => k === 'mode' || k === 'ride' || /^(sora|w|draw|hero|cam|pad|in)\./.test(k)],
  ['Hud', 'Sora\'s HUD and menus; Elden Ring\'s own HUD, prompts and banners.', (k) => /^(hud|prompt|menu|config|erui|title|tutorial)\./.test(k)],
  ['Save', 'Saving, save points, a new game, the game-over menu, cutscenes.', (k) => /^(save|savepoint|saveload|newgame|gameover|cutscene|story|slots)\./.test(k)],
  ['Items', 'Loot, munny, shops, rewards, outfits.', (k) => /^(loot|prize|munny|shop|items|boss|boc|chest)\./.test(k)],
  ['Combat', 'Fighting: finishers, breakables, damage, status, blood.', (k) => /^(finish|break|dmg|damage|cure|blood|react|juggle|lock|magic|shotlock)\./.test(k)],
  ['Party', 'Donald, Goofy and the party.', (k) => /^(party|don|goofy|unlock|conv|talk)\./.test(k)],
  ['Render', 'Drawing Sora (the r.* switches of the renderer), light and effects.', (k) => /^(r|fx|light|vfx)\./.test(k)],
  ['Test', 'DEVELOPMENT AIDS: only for a test profile (the released mod ignores them).', (k) => /^(test|dbg)\./.test(k) || /^p\.dbg_/.test(k)],
];
const OTHER = ['Other', 'Keys of no section above (every key works in any section).'];
const COMMENTS = {
  mode: 'sora = Sora plays in place of the Tarnished',
  ride: '1 = Sora rides lifts and moving floors (0 = off)',
  'w.clamp': 'collision: 1 = a push out of an overlap is cut short where it would go into something else (0 = off, the shipped default)',
  'w.hits': '1 = the walls Sora\'s collision meets are written to the log',
  'draw.axes': 'debug drawing of the axes (0 = off)',
  'draw.sora': 'debug drawing of Sora\'s shapes (0 = off)',
  'hud.kh': '1 = Sora\'s HUD and menus (the shipped default)',
  'hud.er': '0 = Elden Ring\'s HUD hidden part by part while Sora\'s is up; 1 = left whole; off = all off',
  'prompt.er_hide': '1 = Elden Ring\'s own action prompt box is hidden under KH3\'s reaction command',
  'save.dir': 'the KH save folder, a folder of er\\ (default saves)',
  'save.file': 'the Elden Ring save file this profile uses',
  'save.kh': '1 = Sora\'s own save data per Elden Ring character',
  'save.points_only': '1 = saving only at save points',
  'savepoint.rest_events': '0 = a save point never runs Elden Ring\'s rest; sit (or 1) = when Sora sits; always',
  'loot.kh': '1 = KH3 items drop from killed enemies',
  'loot.gate': '1 = the gate: every other way an item reaches the player is turned into KH3\'s',
  'loot.log': '1 = the gate writes what it sees to the log',
  'boc.gate': '0 = Boc\'s outfits without having talked to Boc (testing); 1 = only after talking to him',
  'r.world_lights': '0 = no light of the world\'s on Sora',
  'r.hybrid': '1 = the hybrid: KH3\'s toon shading lit by Elden Ring\'s world light',
  'test.zetta_aid': 'instant Zettaflare: F7, or L3 + R3 on the pad',
  'test.party_level': 'the party brought to this level by the game\'s own level-up path',
  'test.don_all_magic': '1 = Donald with every spell and ability',
};
const sectionOf = (k) => (SECTIONS.find(([, , t]) => t(k)) || OTHER);

/** The .ini text for the settings `map` (from parseOld), with `notes` (comment lines above each key) and `tail`. */
function layout(name, map, notes, tail, from) {
  const NL = '\r\n';
  const out = [
    `# Kingdom Hearts x Elden Ring: the mod's settings (${name}). Read again while the game runs.`,
    '# One "key = value" per line. Lines starting with # or ; are comments, and so is the rest of a line after " #" or',
    '# " ;". [Sections] are only for layout: every key works in any section. A key that is not here has its shipped',
    '# default; "key = 0" switches a default off.',
  ];
  if (from) out.push(`# Converted from ${from} by tools/ctl_to_ini.js (the old file is kept as ${from}.bak).`);
  const order = [...SECTIONS, OTHER];
  for (const sec of order) {
    const keys = [...map.keys()].filter((k) => sectionOf(k) === sec);
    if (!keys.length) continue;
    out.push('', `[${sec[0]}]`, `# ${sec[1]}`);
    for (const k of keys) {
      for (const n of notes.get(k) || []) out.push(n);
      if (COMMENTS[k] && !(notes.get(k) || []).length) out.push(`# ${k}: ${COMMENTS[k]}`);
      out.push(`${k} = ${quote(map.get(k), k)}`);
    }
  }
  if (tail.length) out.push('', ...tail);
  return out.join(NL) + NL;
}
/** A value written so that the new rules give it back exactly. */
function quote(v, k) {
  const plain = parseIni(`x = ${v}`).get('x');
  if (plain === v) return v;
  if (!v.includes('"')) return `"${v}"`;
  throw new Error(`the value of ${k} (${JSON.stringify(v)}) cannot be written so that it reads back the same`);
}

/** Converts one .ctl text. Returns { ini, old, check: [problems] }. */
function convert(text, name, from) {
  const old = parseOld(text);
  const notes = new Map();
  let pending = [];
  for (const raw of lines(text)) {
    const line = trim(raw);
    if (!line) continue;
    if (line.startsWith('#')) { pending.push(line); continue; }
    const i = line.indexOf('=');
    const k = i < 0 ? '' : trim(line.slice(0, i));
    if (!k) { pending.push(`# (not a setting, kept as a comment) ${line}`); continue; }
    if (pending.length) { notes.set(k, (notes.get(k) || []).concat(pending)); pending = []; }
  }
  const ini = layout(name, old, notes, pending, from);
  return { ini, old, check: compare(old, parseIni(ini)) };
}
function compare(want, got) {
  const p = [];
  for (const [k, v] of want) if (!got.has(k)) p.push(`${k}: missing`); else if (got.get(k) !== v) p.push(`${k}: ${JSON.stringify(got.get(k))} instead of ${JSON.stringify(v)}`);
  for (const k of got.keys()) if (!want.has(k)) p.push(`${k}: not in the old file`);
  return p;
}
const freeBak = (ctl) => { for (let n = 1; ; n++) { const b = ctl + '.bak' + (n === 1 ? '' : n); if (!fs.existsSync(b)) return b; } };

function run(target, dry) {
  let ctl = target;
  if (fs.existsSync(target) && fs.statSync(target).isDirectory()) ctl = path.join(target, 'kh-er.ctl');
  if (!/\.ctl$/i.test(ctl)) { console.log(`${target}: not a .ctl file or a folder`); return 2; }
  const ini = ctl.replace(/\.ctl$/i, '.ini');
  const name = path.basename(ini), from = path.basename(ctl);
  const hasCtl = fs.existsSync(ctl), hasIni = fs.existsSync(ini);
  console.log(`== ${ctl}`);
  if (!hasCtl) {
    if (hasIni) { console.log(`   already converted: ${name} is there and no ${from}`); return 0; }
    console.log(`   no ${from} and no ${name}: nothing to convert`); return 1;
  }
  const { text, bom } = readText(ctl);
  if (bom) console.log(`   (the file has a ${bom}: read without it)`);
  const { ini: out, old, check } = convert(text, name, from);
  const fallback = compare(old, parseIni(text));
  console.log(`   ${old.size} setting(s)`);
  if (fallback.length) console.log(`   NOTE: the new DLL would read the .ctl itself differently (${fallback.join('; ')}): the .ini keeps the OLD meaning`);
  if (hasIni) {
    const same = compare(old, parseIni(readText(ini).text));
    if (same.length) { console.log(`   REFUSED: ${name} exists already and holds other settings (${same.join('; ')}): nothing done`); return 1; }
    const bak = freeBak(ctl);
    if (dry) { console.log(`   ${name} exists already with the same settings: would rename ${from} -> ${path.basename(bak)}`); return 0; }
    fs.renameSync(ctl, bak);
    console.log(`   ${name} was there with the same settings; ${from} -> ${path.basename(bak)}`);
    return 0;
  }
  if (check.length) { console.log(`   CHECK FAILED (nothing written): ${check.join('; ')}`); return 1; }
  if (dry) {
    console.log(`   check: the .ini reads back the same ${old.size} setting(s) with the new rules`);
    console.log(`   would write ${ini} and rename ${from} -> ${path.basename(freeBak(ctl))}:`);
    console.log('   ---8<---');
    process.stdout.write(out.replace(/\r\n/g, '\n'));
    console.log('   --->8---');
    return 0;
  }
  const tmp = ini + '.tmp';
  fs.writeFileSync(tmp, out);
  const back = compare(old, parseIni(readText(tmp).text));
  if (back.length) { fs.rmSync(tmp, { force: true }); console.log(`   CHECK FAILED after writing (nothing kept): ${back.join('; ')}`); return 1; }
  fs.renameSync(tmp, ini);
  const bak = freeBak(ctl);
  fs.renameSync(ctl, bak);
  console.log(`   wrote ${name} (${old.size} settings, checked); ${from} -> ${path.basename(bak)}`);
  return 0;
}

module.exports = { parseOld, parseIni, convert, compare, layout, readText, run };

if (require.main === module) {
  const args = process.argv.slice(2);
  const dry = args.includes('--dry-run');
  const paths = args.filter((a) => a !== '--dry-run');
  if (!paths.length || args.some((a) => a.startsWith('--') && a !== '--dry-run')) {
    console.log('usage: node tools/ctl_to_ini.js [--dry-run] <kh-er.ctl | folder with kh-er.ctl>...');
    process.exit(2);
  }
  let code = 0;
  for (const p of paths) code = Math.max(code, run(p, dry));
  process.exit(code);
}
