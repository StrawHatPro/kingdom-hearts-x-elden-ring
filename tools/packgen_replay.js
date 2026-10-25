#!/usr/bin/env node
// (PACKGEN) THE STREAMS' PACK FILES, REPLAYED: the pack files the development streams made with their own tools (the
// party's and the XS abilities' effects, Donald's composed spells, the merged stream packs), made again on the player's PC
// from the recipes in tools/packgen/replay (written by tools/packgen_recipe.js in the development tree).
//
//   node tools/packgen_replay.js [prepare] [apply] [--check]
//     prepare  the scratch packs (build/packgen/scratch/<root>): every KH3 effect the streams packed, by tools/sora_vfx_pack.js
//              from the player's files (the lists: tools/packgen/replay/scratch_<root>.json)
//     apply    every file recipe (tools/packgen/replay/files_<root>.json) -> build/packgen/replay_out, each checked against
//              its SHA-256, then copied into build/sora_pack
//     verify   after the replay steps (the bake): every recipe's file in build/sora_pack is the recorded one
//     --check  apply without writing the pack: report what would differ
//     --overlays  apply the overlay recipes (overlays_<root>.json) instead of the file recipes
//   default: prepare, then apply.
//
// A recipe holds no game data: `copy` names a file the build made before (the pack, the scratch packs), `json` is our
// structure and values with every value of the game's referenced in those files (file + JSON pointer; v2: the file as its
// differences from those files, every part that is the same a reference: tools/packgen_diff.js), `text` is a list of
// references to lines of those files and our own lines. See tools/packgen_recipe.js.
'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const cp = require('child_process');
const SER = require('./packgen_serializers.js');
const DIFF = require('./packgen_diff.js');

const ROOT = path.resolve(__dirname, '..');
const DIR = path.join(ROOT, 'tools', 'packgen', 'replay');
const PACK = path.join(ROOT, 'build', 'sora_pack');
const SCRATCH = path.join(ROOT, 'build', 'packgen', 'scratch');
// (--overlays: the effect tables' overlay recipes, overlays_<root>.json, made after the overlay translator runs:
// tools/packgen_overlays.js)
const OVERLAYS = process.argv.includes('--overlays');
const RECIPES = OVERLAYS ? /^overlays_.*\.json$/ : /^files_.*\.json$/;
const OUT = path.join(ROOT, 'build', 'packgen', OVERLAYS ? 'replay_out_overlays' : 'replay_out');
const args = process.argv.slice(2);
const check = args.includes('--check');
const doVerify = args.includes('verify');
const doPrepare = args.includes('prepare') || (!args.includes('apply') && !doVerify);
const doApply = args.includes('apply') || (!args.includes('prepare') && !doVerify);
const sha = (b) => crypto.createHash('sha256').update(b).digest('hex');
const corpusPath = (id) => { const i = id.indexOf(':'); const tag = id.slice(0, i), f = id.slice(i + 1); if (tag === 'pack') return path.join(PACK, f); if (tag === 'scratch') return path.join(SCRATCH, f); if (tag === 'root') return path.join(ROOT, f); if (tag === 'out') return path.join(OUT, f); /* (v2) a file this replay made before */ throw new Error('corpus: ' + id); };

if (!fs.existsSync(DIR)) { console.log('packgen_replay: no recipes (tools/packgen/replay): nothing to do'); process.exit(0); }

if (doPrepare) {
  for (const f of fs.readdirSync(DIR).filter((x) => /^scratch_.*\.json$/.test(x)).sort()) {
    const name = f.replace(/^scratch_|\.json$/g, '');
    const out = path.join(SCRATCH, name); // (a list named *_fragments: packed with the fragment meshes, --fragments)
    fs.rmSync(out, { recursive: true, force: true });
    const r = cp.spawnSync(process.execPath, [path.join(ROOT, 'tools', 'sora_vfx_pack.js'), '--list', path.join(DIR, f), '--no-seed', ...(/_fragments$/.test(name) ? ['--fragments'] : []), '--out', out], { cwd: ROOT, encoding: 'utf8', maxBuffer: 1 << 28, env: { ...process.env, MSYS_NO_PATHCONV: '1' } });
    if (r.status !== 0) { console.error(`scratch ${name}: tools/sora_vfx_pack.js failed:\n${(r.stdout + r.stderr).split('\n').slice(-15).join('\n')}`); process.exit(1); }
    console.log(`scratch ${name}: ${fs.readdirSync(path.join(out, 'effects')).length} effects packed -> ${path.relative(ROOT, out)}`);
  }
}

if (doApply) {
  const jsonCache = new Map();
  const corpusJson = (id) => { if (!jsonCache.has(id)) { if (jsonCache.size > 300) jsonCache.clear(); jsonCache.set(id, JSON.parse(fs.readFileSync(corpusPath(id), 'utf8'))); } return jsonCache.get(id); };
  const textCache = new Map();
  const corpusLines = (id) => { if (!textCache.has(id)) { if (textCache.size > 50) textCache.clear(); textCache.set(id, fs.readFileSync(corpusPath(id), 'utf8').split('\n')); } return textCache.get(id); };
  // (a key the recipe holds as its hash, `{h}`: the game's text, found among the object's own keys)
  const hkey = (k) => crypto.createHash('sha256').update(String(k), 'utf8').digest('hex').slice(0, 16);
  const byHash = (obj, h) => { const k = Object.keys(obj).find((x) => hkey(x) === h); if (k === undefined) throw new Error('no key with hash ' + h); return k; };
  const at = (obj, ptr) => {
    let v = obj;
    const segs = Array.isArray(ptr) ? ptr : ptr.split('/').slice(1).map((k) => k.replace(/~1/g, '/').replace(/~0/g, '~'));
    for (const k of segs) { v = v[typeof k === 'object' ? byHash(v, k.h) : k]; if (v === undefined) throw new Error('pointer ' + JSON.stringify(ptr)); }
    return v;
  };
  // the Japanese keys of a recipe's corpus files, by hash
  const keysOf = (rec) => { if (!rec._keys) { rec._keys = new Map(); for (const id of rec.corpus) (function kw(x) { if (x && typeof x === 'object') for (const k of Object.keys(x)) { rec._keys.set(hkey(k), k); kw(x[k]); } })(corpusJson(id)); } return rec._keys; };
  fs.rmSync(OUT, { recursive: true, force: true });
  let made = 0, bad = 0; const badList = [];
  const files = fs.readdirSync(DIR).filter((x) => RECIPES.test(x)).sort();
  for (const f of files) {
    const { root: recRoot, files: recs } = JSON.parse(fs.readFileSync(path.join(DIR, f), 'utf8'));
    for (const [rel, rec] of Object.entries(recs)) {
      let buf;
      if (rec.kind === 'bake') continue; // made by the bake step (the pack tools' file + its baked points)
      try {
        if (rec.kind === 'copy') buf = fs.readFileSync(corpusPath(rec.from));
        else if (rec.kind === 'json') {
          const resolve = (v) => {
            if (Array.isArray(v)) return v.map(resolve);
            if (v !== null && typeof v === 'object') {
              const r = v['$ref@packgen']; if (r && Object.keys(v).length === 1) return at(corpusJson(rec.corpus[r[0]]), r[1]);
              const e = v['$obj@packgen']; if (e && Object.keys(v).length === 1) { const o = {}; for (const [k, x] of e) { const kk = typeof k === 'object' ? keysOf(rec).get(k.h) : k; if (kk === undefined) throw new Error('no corpus key with hash ' + k.h); o[kk] = resolve(x); } return o; }
              const o = {}; for (const k of Object.keys(v)) o[k] = resolve(v[k]); return o;
            }
            return v;
          };
          // (recipes v2: the file as its differences from the corpus, tools/packgen_diff.js)
          const value = rec.v === 2 ? DIFF.decoder((c, p) => at(corpusJson(rec.corpus[c]), p), (hh) => keysOf(rec).get(hh))(rec.template) : resolve(rec.template);
          buf = Buffer.from(SER.serialize(value, rec.format), 'utf8');
        } else if (rec.kind === 'jsontext') buf = Buffer.from(rec.parts.map((p) => (Array.isArray(p) ? JSON.stringify(at(corpusJson(rec.corpus[p[0]]), p[1])) : p)).join(''), 'utf8'); // the file's own text, its values referenced
        else if (rec.kind === 'raw') buf = Buffer.from(rec.text, 'utf8'); // our own design file
        else if (rec.kind === 'text') buf = Buffer.from(rec.lines.map((l) => (Array.isArray(l) ? corpusLines(rec.corpus[l[0]])[l[1]] : l)).join('\n'), 'utf8');
        else throw new Error('recipe kind ' + rec.kind);
      } catch (e) { bad++; badList.push(`${rel}: ${e.message}`); continue; } finally { delete rec._keys; }
      if (sha(buf) !== rec.sha256) { bad++; badList.push(`${rel}: made, but not the recorded file (SHA-256 differs)`); continue; }
      // a pack file goes to the staging folder (copied into the pack below); an AUX file (a stream's manifest outside the
      // pack) straight to its own path
      const dst = recRoot === 'aux' ? path.join(ROOT, rel) : path.join(OUT, rel); fs.mkdirSync(path.dirname(dst), { recursive: true }); fs.rmSync(dst, { force: true }); fs.writeFileSync(dst, buf); made++;
    }
  }
  console.log(`packgen_replay: ${made} file(s) made and checked, ${bad} failed`);
  for (const l of badList.slice(0, 30)) console.log('   ' + l);
  if (!check) {
    (function walk(d, base) { if (!fs.existsSync(d)) return; for (const e of fs.readdirSync(d, { withFileTypes: true })) { const p = path.join(d, e.name); if (e.isDirectory()) walk(p, base); else { const t = path.join(PACK, path.relative(base, p)); fs.mkdirSync(path.dirname(t), { recursive: true }); fs.rmSync(t, { force: true }); fs.copyFileSync(p, t); } } })(OUT, OUT);
    console.log(`packgen_replay: copied into ${path.relative(ROOT, PACK)}`);
  }
  if (bad) process.exitCode = 1;
}

if (doVerify) {
  let ok = 0, bad = 0; const list = [];
  for (const f of fs.readdirSync(DIR).filter((x) => /^(files|overlays)_.*\.json$/.test(x)).sort()) {
    const { root: recRoot, files: recs } = JSON.parse(fs.readFileSync(path.join(DIR, f), 'utf8'));
    for (const [rel, rec] of Object.entries(recs)) {
      const want = rec.final_sha256 || rec.sha256; const p = recRoot === 'aux' ? path.join(ROOT, rel) : path.join(PACK, rel);
      if (fs.existsSync(p) && sha(fs.readFileSync(p)) === want) ok++; else { bad++; list.push(rel); }
    }
  }
  console.log(`packgen_replay verify: ${ok} file(s) as recorded, ${bad} not`);
  for (const l of list.slice(0, 30)) console.log('   ' + l);
  fs.mkdirSync(path.join(ROOT, 'build', 'packgen'), { recursive: true });
  fs.writeFileSync(path.join(ROOT, 'build', 'packgen', 'replay_verify.json'), JSON.stringify({ ok, not_as_recorded: list }, null, 1) + '\n');
  if (bad) process.exitCode = 1;
}
