#!/usr/bin/env node
// (PACKGEN) DEVELOPMENT TOOL: writes the replay recipes (tools/packgen/replay/) that let tools/packgen_replay.js remake, on a
// player's PC, the pack files the development streams made with their own tools (the party's and the XS abilities'
// effects, Donald's composed spells, the merged stream packs).
//
//   node tools/packgen_recipe.js lists                  the scratch packing lists (tools/packgen/replay/scratch_*.json)
//   node tools/packgen_recipe.js compile --sandbox DIR [--dev DIR] [--overlays]  the file recipes, against DIR (a release-like tree where
//                                                       tools/packgen.js research + pack + tools/packgen_replay.js prepare
//                                                       have run): tools/packgen/replay/files_<root>.json + report.json
//
// A file recipe says how to make one pack file of this development pack (build/sora_pack) from what the build on the
// player's PC has made before it (its "corpus": the pack the pack tools made, and the scratch packs of the effects and
// sounds the streams used). It never holds game data:
//   copy      the file is byte for byte a corpus file (its path is recorded)
//   json      a JSON file: its structure and our own values, with every value the corpus has replaced by a reference
//             (corpus file + JSON pointer); the file's layout (serializer) is recorded
//   text      a text file (WGSL tables): each line a reference (corpus file + line) or our own line
// Each recipe carries the file's SHA-256: tools/packgen_replay.js checks what it writes against it. A file none of these
// can make is listed in report.json (unresolved) with the reason. Japanese text is never written: a value must be referenced, a key or pointer segment is its hash; else the
// file stays unresolved.
'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { ROOT } = require('./lib/log');
const SER = require('./packgen_serializers.js');
const DIFF = require('./packgen_diff.js');

const OUTDIR = path.join(ROOT, 'tools', 'packgen', 'replay');
// --dev DIR: the development tree whose pack the recipes remake (default: this tree); its stream manifests too
const DEVROOT = path.resolve((() => { const a = process.argv.slice(2); const i = a.indexOf('--dev'); return i >= 0 ? a[i + 1] : ROOT; })());
const DEVPACK = path.join(DEVROOT, 'build', 'sora_pack');
const args = process.argv.slice(2);
const opt = (k) => { const i = args.indexOf(k); return i >= 0 ? args[i + 1] : null; };
const mode = args[0];
const sha = (b) => crypto.createHash('sha256').update(b).digest('hex');
const readJson = (f) => JSON.parse(fs.readFileSync(f, 'utf8'));
// (a key or a pointer segment with Japanese text in it, which is the game's text: written as the first 16 hex digits of its
// SHA-256, `{"h": ...}`, which the replay finds among the corpus file's own keys)
const hkey = (k) => crypto.createHash('sha256').update(String(k), 'utf8').digest('hex').slice(0, 16);
const encPtr = (ptr) => { if (!JP.test(ptr)) return ptr; return ptr.split('/').slice(1).map((x) => x.replace(/~1/g, '/').replace(/~0/g, '~')).map((x) => (JP.test(x) ? { h: hkey(x) } : x)); };
const JP = /[　-ヿ㐀-鿿豈-﫿＀-￯]/;
function walkFiles(dir, base = dir, out = []) {
  if (!fs.existsSync(dir)) return out;
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) { const p = path.join(dir, e.name); if (e.isDirectory()) walkFiles(p, base, out); else out.push(path.relative(base, p).split(path.sep).join('/')); }
  return out;
}
// the vfx roots the streams added to: their KH3 effects are packed into scratch on the player's PC
const VFX_ROOTS = ['party/vfx', 'xsi/vfx'];

// ---- the shader recipe (tools/packgen/recipes/shaders.json): which kh-shaderc runs make the pack's Elden Ring shaders.
// Names, defines and arguments only (our build's), from this pack's folders and the NF stream's rebuild plan
// (build/sora_tmp/party/NF/plan: every character pixel shader the DLL can load, with its defines and materials source).
if (mode === 'shaders') {
  const MODES = [['skincg', 'skin_cg.vs.dxil'], ['velo88', 'skin_v88.vs.dxil'], ['finale', 'finale.ps.dxil'], ['hycap', 'hycap.ps.dxil'], ['h91', 'h91.vs.dxil'], ['probe3', 'probe3.ps.dxil'], ['probe4', 'probe4.ps.dxil'], ['zdraw', 'zdraw.vs.dxil']];
  const dirs = [];
  const add = (pack, args) => {
    const er = path.join(DEVPACK, pack, 'materials', 'er'); if (!fs.existsSync(er)) return;
    dirs.push({ pack: 'build/sora_pack' + (pack ? '/' + pack : ''), er: (pack ? pack + '/' : '') + 'materials/er', args, modes: MODES.filter(([, f]) => fs.existsSync(path.join(er, f))).map(([m]) => m) });
  };
  add('', []);
  for (const o of fs.readdirSync(path.join(DEVPACK, 'outfits')).sort()) add('outfits/' + o, []);
  for (const m of fs.readdirSync(path.join(DEVPACK, 'party')).sort()) if (fs.existsSync(path.join(DEVPACK, 'party', m, 'materials', 'er'))) add('party/' + m, ['--party', '--main-pack', 'build/sora_pack']);
  const NF = path.join(ROOT, 'build', 'sora_tmp', 'party', 'NF', 'plan');
  const plan = readJson(path.join(NF, 'groups.json'));
  const groups = plan.groups.map((g) => ({ id: g.id, er: g.er, pack: g.pack, materials: g.materials, args: g.args, copies: g.copies, jobs: readJson(path.join(NF, g.id + '.json')).jobs.map((j) => ({ name: j.name, stage: j.stage, entry: j.entry, defines: j.defines })) }));
  // the hair translations the plan's groups read (er87 / er91 genh), from the folder's materials: the scripts of the hair
  // streams (tools/research er/er87/tools, er/er91/tools)
  const hairSrc = (name) => { const m = /^(main|[a-z0-9]+?)(?:_(cg|cg2|real))?$/.exec(name); const base = m[1] === 'main' ? 'build/sora_pack/materials' : `build/sora_pack/outfits/${m[1]}/materials`; return { wgsl: `${base}/materials${m[2] ? '_' + m[2] : ''}.wgsl`, json: `${base}/materials.json` }; };
  const hair = { er87: [], er91: [] };
  for (const k of ['er87', 'er91']) for (const f of fs.readdirSync(path.join(ROOT, 'build', 'sora_tmp', 'er', k, 'genh')).sort()) { const n = f.replace(/\.wgsl$/, ''); hair[k].push({ name: n, ...hairSrc(n) }); }
  const vfx = ['build/sora_pack', 'build/sora_pack/party', 'build/sora_pack/xsi'].filter((p) => fs.existsSync(path.join(ROOT, p, 'vfx', 'er')));
  // the outfits' own shader sets (ER81 / ER85: tools/sora_material_pack.js --own-khs): outfit, post-process material,
  // suffix, the world whose controller is the preset (from each set's own_shaders.json: its world preset's source names the
  // map), the KHS_Control values chosen against a reference (--control)
  const own = [];
  for (const o of fs.readdirSync(path.join(DEVPACK, 'outfits')).sort()) {
    const f = path.join(DEVPACK, 'outfits', o, 'materials', 'own_shaders.json'); if (!fs.existsSync(f)) continue;
    const j = readJson(f); const w = j.world_preset; const src = (j.khs.presets[w] || {}).source || '';
    const m = /^(.+?): the KHS_Blueprint_C placed in (Maps\/\S+?\.umap\.json)/.exec(src);
    if (!m) throw new Error(o + ': own set without its world');
    own.push({ outfit: o, suffix: j.suffix, asset: j.khs.asset, world: `${w}=${m[2]}=${m[1]}`, control: Object.entries(j.control_overrides || {}).map(([k, v]) => `${k}=${v}`).join(',') || null });
  }
  fs.writeFileSync(path.join(ROOT, 'tools', 'packgen', 'recipes', 'own_sets.json'), JSON.stringify({ about: 'the outfits\' own shader sets (tools/packgen_recipe.js shaders): tools/sora_material_pack.js --own-khs, one run each', sets: own }, null, 1) + '\n');
  // the effect tables' owner ids (the built er/<s|m|d|v><id> files and the overlays are keyed by them): owner -> id, asset
  // (tools/sora_vfx_shaders.js KH_KEEP_OWNER_IDS keeps them)
  for (const [name, p] of [['main', 'vfx'], ['party', 'party/vfx'], ['xsi', 'xsi/vfx']]) {
    const f = path.join(DEVPACK, p, 'shaders', 'shading.json'); if (!fs.existsSync(f)) continue;
    const owners = readJson(f).owners;
    const keep = { about: `the owner ids of the ${name} effect table (tools/packgen_recipe.js shaders): KH_KEEP_OWNER_IDS of tools/sora_vfx_shaders.js`, owners: Object.fromEntries(Object.entries(owners).map(([k, o]) => [k, { id: o.id, asset: o.asset }])) };
    fs.writeFileSync(path.join(ROOT, 'tools', 'packgen', 'recipes', `vfx_owners_${name}.json`), JSON.stringify(keep, null, 1) + '\n');
  }
  // the effect tables' overlay shaders (er/shaders_<s>.json): kh-shaderc --vfx --only <name> over the overlay translator run
  // of that table and family (tools/packgen_overlays.js), the file renamed
  const vfxOverlays = [];
  for (const [root, table] of [['vfx', 'main'], ['party/vfx', 'party'], ['xsi/vfx', 'xsi']]) {
    const er = path.join(DEVPACK, root, 'er'); if (!fs.existsSync(er)) continue;
    for (const f of fs.readdirSync(er).filter((x) => /^shaders_[a-z0-9]+\.json$/.test(x)).sort()) {
      const fam = f.slice(8, -5);
      for (const s of readJson(path.join(er, f)).shaders) vfxOverlays.push({ root: 'build/sora_pack/' + root, stage: `${table}_${fam}`, table: 'materials.wgsl', only: s.name.replace(new RegExp('_' + fam + '$'), ''), file: s.file });
    }
  }
  const out = { about: 'tools/packgen_shaders.js: the kh-shaderc runs of the pack (tools/packgen_recipe.js shaders). Our build\'s names, defines and arguments only.', char_dirs: dirs, hair, nf_groups: groups, vfx_roots: vfx, vfx_overlays: vfxOverlays };
  const text = JSON.stringify(out, null, 1) + '\n';
  if (JP.test(text)) throw new Error('Japanese text in the shader recipe');
  fs.mkdirSync(path.join(ROOT, 'tools', 'packgen', 'recipes'), { recursive: true });
  fs.writeFileSync(path.join(ROOT, 'tools', 'packgen', 'recipes', 'shaders.json'), text);
  console.log(`shaders.json: ${dirs.length} character folders, ${groups.length} rebuild groups (${groups.reduce((s, g) => s + g.jobs.length, 0)} jobs), hair ${hair.er87.length} + ${hair.er91.length}, vfx roots ${vfx.length}`);
  process.exit(0);
}

if (mode === 'lists') {
  fs.mkdirSync(OUTDIR, { recursive: true });
  for (const root of VFX_ROOTS) {
    const rows = [];
    for (const d of ['effects', 'effects_xwd']) {
      const p = path.join(DEVPACK, root, d); if (!fs.existsSync(p)) continue;
      for (const f of fs.readdirSync(p).sort()) {
        const j = readJson(path.join(p, f));
        if (!j.asset || !j.source || !j.source.package) continue; // composed by a stream script: made by its recipe
        rows.push({ effect: j.asset, used_by: j.used_by || [], priority: j.priority || null, categories: j.categories || [] });
      }
    }
    // also every KH3 effect a composed effect took emitters from (its emitters' `from`)
    const name = root.replace(/\//g, '_');
    const seen = new Set(rows.map((r) => r.effect.toLowerCase()));
    for (const d of ['effects']) {
      const p = path.join(DEVPACK, root, d); if (!fs.existsSync(p)) continue;
      for (const f of fs.readdirSync(p).sort()) {
        const j = readJson(path.join(p, f)); if (j.source && j.source.package) continue;
        for (const e of j.emitters || []) { const m = typeof e.from === 'string' && /\/Game\/[^\s:#]+/.exec(e.from); if (m && !seen.has(m[0].toLowerCase())) { seen.add(m[0].toLowerCase()); rows.push({ effect: m[0].replace(/\.[^/]*$/, ''), used_by: ['packgen: emitter source of composed effects'], priority: null, categories: [] }); } }
      }
    }
    const text = JSON.stringify(rows, null, 1) + '\n';
    if (JP.test(text)) throw new Error(name + ': Japanese text in a scratch list');
    fs.writeFileSync(path.join(OUTDIR, `scratch_${name}.json`), text);
    console.log(`scratch_${name}.json: ${rows.length} effects`);
  }
  // the KH3 effects the streams packed on their own to take parts from (HYB: the Re Mind feather and n_ex005's effects;
  // XW-d: Chandelier's fragment effects, packed with their fragment meshes; XW-c: the chandelier's own effects, its
  // --xsi-extra pack): from those packs' manifests (their command lines)
  const EXTRA = [['streams_extra', ['build/sora_tmp/party/HYB/imp/vfx', 'build/sora_tmp/party/HYB/imp2/vfx']], ['xsi_fragments', null], ['xsi_chandelier', ['build/sora_tmp/er/xwc/stage/fxpack']]];
  for (const [name, packs] of EXTRA) {
    const effects = packs ? packs.flatMap((p) => readJson(path.join(ROOT, p, 'manifest.json')).inputs.command_line.effects)
      // (XW-d's fragment pack is gone from its stage; its command line, from IMPL_XWD_STATE.md / stage/fxpack.log)
      : ['hahen02', 'hahen03', 'zmn0'].map((e) => '/Game/Effects/wep/w_so080/par/vw_so088_fin_' + e);
    const rows = effects.map((e) => ({ effect: e, used_by: ['command line'], priority: null, categories: [] }));
    const text = JSON.stringify(rows, null, 1) + '\n';
    if (JP.test(text)) throw new Error(name + ': Japanese text in a scratch list');
    fs.writeFileSync(path.join(OUTDIR, `scratch_${name}.json`), text);
    console.log(`scratch_${name}.json: ${rows.length} effects`);
  }
  process.exit(0);
}

if (mode !== 'compile') { console.error('usage: packgen_recipe.js lists | compile --sandbox DIR'); process.exit(2); }
const SB = path.resolve(opt('--sandbox') || '');
const SBPACK = path.join(SB, 'build', 'sora_pack');
const SCRATCH = path.join(SB, 'build', 'packgen', 'scratch');
if (!fs.existsSync(SBPACK)) { console.error('--sandbox DIR: no build/sora_pack there'); process.exit(2); }

// ---------------------------------------------------------------- what is to be made
const EXCLUDE = [
  /\.dxil$/i,                                   // shaders: compiled by tools/packgen_shaders.js
  /\/er\/shaders[^/]*\.json$/, /\/er\/sets\.json$/, // their indexes: written by kh-shaderc
  /\/er\/tex\/[^/]+\.dds$/,                     // block-compressed textures: copied by kh-shaderc
  /^audio\/outfits\//,                          // the KH1 outfit's own voice (the user's clips): not in a release
  /(^|\/)vfx\/shaders\/(shading|materials)[^/]*\.(json|wgsl)$/, // the effect tables: the replay's vfx-tables step (and the streams' overlays)
  /^xsi\/weapons\/shards_(vfb|xwd)\/[^/]+\.glb$/, // the fragment models: the replay's xsi-shards step (after the recipes)
];
// the streams' own outputs the replay makes before the recipes (tools/packgen.js replay: zetta-audio, zetta-clips), by their
// path from the repository root
const STREAM_OUT = ['build/sora_tmp/party/ZETTA/pack'];
// --overlays: the second pass, the effect tables' overlays (the streams' shading_<s>.json, materials_<s>.wgsl, er/shaders_<s>.json)
// against the overlay translator runs of the sandbox (tools/packgen_overlays.js: build/packgen/vfx_stage/<run>) and the
// root tables: written as overlays_<root>.json (tools/packgen_replay.js apply --overlays)
const OVERLAYS = args.includes('--overlays');
const OVERLAY = [/(^|\/)vfx\/shaders\/(shading|materials)_[a-z0-9]+\.(json|wgsl)$/, /(^|\/)vfx\/er\/shaders_[a-z0-9]+\.json$/];
const STAGE_REL = 'build/packgen/vfx_stage';
const corpusRoots = OVERLAYS ? [['root', path.join(SB, STAGE_REL), STAGE_REL]] : [['pack', SBPACK], ['scratch', SCRATCH], ...STREAM_OUT.map((p) => ['root', path.join(SB, p), p])];
const devFiles = walkFiles(DEVPACK);
const targets = [];
for (const r of devFiles) {
  if (OVERLAYS ? !OVERLAY.some((re) => re.test(r)) : EXCLUDE.some((re) => re.test(r))) continue;
  const sb = path.join(SBPACK, r);
  const want = fs.readFileSync(path.join(DEVPACK, r));
  if (fs.existsSync(sb) && fs.readFileSync(sb).equals(want)) continue; // the pack tools make it already
  targets.push(r);
}
// AUX: files outside the pack the later steps read (the streams' effect pack manifests: tools/sora_vfx_shaders.js reads
// their texture records for the party and XS tables). Written to their own path (relative to the repository root).
const AUX = ['build/sora_tmp/party/impl_e/vfx_party/manifest.json', ...['DON-KIT', 'DON-MAGIC', 'ZETTA', 'DON-SPELLB', 'AUD', 'HYB', 'YFX'].map((s) => `build/sora_tmp/party/${s}/pack/vfx/manifest.json`)];
if (!OVERLAYS) for (const a of AUX) if (fs.existsSync(path.join(DEVROOT, a))) targets.push('aux:' + a);
console.log(`targets: ${targets.length} files the pack tools do not make as they are`);

// ---------------------------------------------------------------- corpus index (by hash; JSON values on demand)
const byHash = new Map(); const corpusFiles = [];
for (const [tag, dir, rel0] of corpusRoots) for (const f of walkFiles(dir)) {
  if (OVERLAYS && !/^[a-z0-9_]+\/(shading\.json|materials\.wgsl)$/.test(f)) continue; // (the runs' tables only)
  const p = path.join(dir, f); const st = fs.statSync(p); if (st.size > 64 << 20) continue;
  const id = tag === 'root' ? `root:${rel0}/${f}` : `${tag}:${f}`; corpusFiles.push(id);
  const h = sha(fs.readFileSync(p)); if (!byHash.has(h)) byHash.set(h, id);
}
// (--overlays: and the root tables of the pack)
if (OVERLAYS) for (const r of ['vfx', 'party/vfx', 'xsi/vfx']) for (const f of ['shading.json', 'materials.wgsl']) if (fs.existsSync(path.join(SBPACK, r, 'shaders', f))) corpusFiles.push(`pack:${r}/shaders/${f}`);
const corpusPath = (id) => { const [tag, f] = [id.slice(0, id.indexOf(':')), id.slice(id.indexOf(':') + 1)]; return tag === 'root' ? path.join(SB, f) : path.join(tag === 'pack' ? SBPACK : SCRATCH, f); };
console.log(`corpus: ${corpusFiles.length} files`);
// JSON corpus files by basename (a target's candidates: corpus files with its name, plus the scratch effects it names)
const byBase = new Map();
for (const id of corpusFiles) if (id.endsWith('.json')) { const b = path.basename(id).toLowerCase(); if (!byBase.has(b)) byBase.set(b, []); byBase.get(b).push(id); }
const jsonCache = new Map();
// (v2) `out:<path>`: a file this replay made before (an earlier recipe of the same root: the replay writes them in order)
const OUT_MADE = new Map(); const madeByRoot = new Map();
const loadCorpusJson = (id) => { if (id.startsWith('out:')) return OUT_MADE.has(id) ? OUT_MADE.get(id) : null; if (!jsonCache.has(id)) { if (jsonCache.size > 400) jsonCache.clear(); try { jsonCache.set(id, readJson(corpusPath(id))); } catch { jsonCache.set(id, null); } } return jsonCache.get(id); };
const eligible = (v) => (typeof v === 'string' ? v.length >= 3 : typeof v === 'number' ? !Number.isInteger(v) || Math.abs(v) >= 1000 : false);
function indexValues(ids) {
  const m = new Map();
  ids.forEach((id, fi) => {
    const j = loadCorpusJson(id); if (j === null) return;
    (function walk(v, ptr) {
      if (v !== null && typeof v === 'object') { for (const k of Object.keys(v)) walk(v[k], ptr + '/' + String(k).replace(/~/g, '~0').replace(/\//g, '~1')); return; }
      if (!eligible(v)) return;
      const key = typeof v + ':' + String(v);
      if (!m.has(key)) m.set(key, [fi, ptr]);
    })(j, '');
  });
  return m;
}

// the global index: every value of every effect / material file of the corpus (composed effects take their emitters from
// KH3 effects they do not name)
let GLOBAL = null;
// (v2) every emitter of the corpus's effect files by its make (class, materials, module classes): a composed effect's
// emitters are KH3 emitters it changed, found here when the effect does not name them
let EMITTERS = null;
// (v2) the parts of every effect of the corpus: each emitter, module, type data, required block and LOD by its exact text
// (a composed effect keeps KH3 modules as they are) and the classed objects (emitters, modules) by their class and members
// (a composed effect's emitters and modules are KH3 ones it changed)
const classKey = (o) => o.class + '\u0001' + Object.keys(o).join('\u0001');
const sha1 = (s) => crypto.createHash('sha1').update(s).digest('base64');
function emitterIndex() {
  if (EMITTERS) return EMITTERS;
  EMITTERS = { byText: new Map(), byClass: new Map() };
  const addText = (v, id, ptr) => { const s = JSON.stringify(v); if (s.length < 48) return; const k = sha1(s); if (!EMITTERS.byText.has(k)) EMITTERS.byText.set(k, [id, ptr]); };
  const addClass = (v, id, ptr) => { if (!v || typeof v.class !== 'string') return; const k = classKey(v); let l = EMITTERS.byClass.get(k); if (!l) EMITTERS.byClass.set(k, (l = [])); if (l.length < 60) l.push([id, ptr]); };
  for (const id of corpusFiles) {
    if (!/\/(effects|effects_xwd)\/[^/]+\.json$/.test(id)) continue;
    let j; try { j = readJson(corpusPath(id)); } catch { continue; }
    (j.emitters || []).forEach((e, i) => {
      if (!e || typeof e !== 'object') return;
      const p = '/emitters/' + i; addText(e, id, p); addClass(e, id, p);
      for (const k of ['typedata', 'required', 'spawn', 'materials', 'params']) if (e[k]) addText(e[k], id, p + '/' + k);
      (e.modules || []).forEach((m, mi) => { addText(m, id, p + '/modules/' + mi); addClass(m, id, p + '/modules/' + mi); if (m && m.params) addText(m.params, id, p + '/modules/' + mi + '/params'); });
      (e.lods || []).forEach((l, li) => addText(l, id, p + '/lods/' + li));
    });
  }
  console.log(`effect part index: ${EMITTERS.byText.size} parts, ${EMITTERS.byClass.size} classed kinds`);
  return EMITTERS;
}
const ptrGet = (j, ptr) => { let v = j; for (const s of ptr.split('/').slice(1)) { if (v == null) return undefined; v = v[s]; } return v; };
const findTree = (text) => { const hit = emitterIndex().byText.get(sha1(text)); if (!hit) return null; const file = loadCorpusJson(hit[0]); return file ? { id: hit[0], ptr: hit[1], file } : null; };
const findEmitter = (t) => (emitterIndex().byClass.get(classKey(t)) || []).map(([id, ptr]) => { const file = loadCorpusJson(id); const v = file && ptrGet(file, ptr); return v ? { id, ptr, v, file } : null; }).filter(Boolean);
function globalIndex() {
  if (GLOBAL) return GLOBAL;
  GLOBAL = new Map();
  for (const id of corpusFiles) {
    // (and the scratch packs' manifests: their texture / material records)
    if (!/\/(effects|effects_xwd|materials)\/[^/]+\.json$/.test(id) && !/^scratch:[^/]+\/manifest\.json$/.test(id)) continue;
    let j; try { j = readJson(corpusPath(id)); } catch { continue; }
    (function walk(v, ptr) {
      if (v !== null && typeof v === 'object') { for (const k of Object.keys(v)) walk(v[k], ptr + '/' + String(k).replace(/~/g, '~0').replace(/\//g, '~1')); return; }
      if (!eligible(v)) return;
      const key = typeof v + ':' + String(v);
      if (!GLOBAL.has(key)) GLOBAL.set(key, [id, ptr]);
    })(j, '');
  }
  console.log(`global index: ${GLOBAL.size} values`);
  return GLOBAL;
}
// the files our streams wrote by hand (design data: no game values in them), kept as they are
const AUTHORED = [/^outfits\/[^/]+\/twosided\.json$/, /^outfits\/[^/]+\/materials\/own_look\.json$/];
// a JSON recipe is taken only when what is left as our own values is small (else the file is our tools' to make)
const MAX_OWN = Number(opt('--max-own') || 400);

// ---------------------------------------------------------------- recipes
const recipes = {}; const unresolved = []; const stats = { copy: 0, json: 0, unresolved: 0, json_refs: 0, json_own_values: 0 };
const rootOf = (r) => { if (r.startsWith('build/')) return 'aux'; const p = r.split('/'); return p[0] === 'party' || p[0] === 'xsi' || p[0] === 'outfits' ? p.slice(0, 2).join('/') : p[0]; };
for (let r of targets) {
  const isAux = r.startsWith('aux:'); const src = isAux ? path.join(DEVROOT, r.slice(4)) : path.join(DEVPACK, r); if (isAux) r = r.slice(4);
  const want = fs.readFileSync(src); const h = sha(want);
  const root = rootOf(r);
  const inCore = !isAux && fs.existsSync(path.join(SBPACK, r));
  const put = (rec) => { (recipes[root] = recipes[root] || {})[r] = { sha256: h, ...rec }; };
  const hit = byHash.get(h);
  if (hit) { put({ kind: 'copy', from: hit }); stats.copy++; continue; }
  // files our own design wrote, with no game data in them (a slot list, our light values): the file itself
  if (AUTHORED.some((re) => re.test(r)) && want.length < 65536 && !JP.test(want.toString('utf8'))) { put({ kind: 'raw', text: want.toString('utf8') }); stats.raw = (stats.raw || 0) + 1; continue; }
  if (r.endsWith('.json')) {
    let target; try { target = JSON.parse(want.toString('utf8')); } catch { unresolved.push([r, 'not JSON', inCore]); continue; }
    const fmt = SER.detect(target, want.toString('utf8'));
    if (!fmt) {
      // a layout no serializer gives: the file's own text, every value token the corpus has replaced by a reference
      // (taken only where the value's JSON spelling is the token's: the bytes come back as they are)
      const text = want.toString('utf8');
      const cands = new Set(byBase.get(path.basename(r).toLowerCase()) || []);
      const ids = [...cands].slice(0, 64); const idx = indexValues(ids); const G = globalIndex();
      const corpus = ids.slice(); const ci = new Map(corpus.map((id, i) => [id, i]));
      const parts = []; let at = 0, own = 0, jp = false;
      const tokRe = /"(?:[^"\\]|\\.)*"|-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/g; let m;
      while ((m = tokRe.exec(text))) {
        const tok = m[0]; let v; try { v = JSON.parse(tok); } catch { continue; }
        if (typeof v === 'string' && JP.test(v)) jp = true;
        if (!eligible(v)) continue;
        const key = typeof v + ':' + String(v);
        let hitv = idx.get(key); if (!hitv) { const hg = G.get(key); if (hg) { if (!ci.has(hg[0])) { ci.set(hg[0], corpus.length); corpus.push(hg[0]); } hitv = [ci.get(hg[0]), hg[1]]; } }
        if (!hitv || JSON.stringify(v) !== tok) { own++; continue; }
        parts.push(text.slice(at, m.index), [hitv[0], encPtr(hitv[1])]); at = m.index + tok.length;
      }
      parts.push(text.slice(at));
      if (jp) { unresolved.push([r, 'Japanese text not found in the corpus', inCore]); continue; }
      if (own > MAX_OWN && !isAux) { unresolved.push([r, `${own} values not in the corpus (more than ${MAX_OWN}; own layout)`, inCore]); continue; }
      put({ kind: 'jsontext', corpus, parts, own_values: own }); stats.jsontext = (stats.jsontext || 0) + 1; continue;
    }
    // (VFXR) an effect with baked spawn points (`kh_points`): the recipe makes the file before the bake, the replay's bake
    // step (tools/research vfxr/tools/vfxr_bake.js, from the player's textures / vertex data) adds the points
    let bake = null;
    if (/\/effects[^/]*\//.test(r) && want.includes('"kh_points"')) {
      const strip = (v) => (Array.isArray(v) ? v.map(strip) : v && typeof v === 'object' ? Object.fromEntries(Object.entries(v).filter(([k]) => k !== 'kh_points').map(([k, x]) => [k, strip(x)])) : v);
      target = strip(target);
      const pre = Buffer.from(SER.serialize(target, fmt), 'utf8'); const ph = sha(pre);
      if (inCore && sha(fs.readFileSync(path.join(SBPACK, r))) === ph) { put({ kind: 'bake' }); stats.bake = (stats.bake || 0) + 1; continue; }
      bake = { sha256: ph, final_sha256: h };
      const hit2 = byHash.get(ph); if (hit2) { put({ kind: 'copy', from: hit2, ...bake }); stats.copy++; continue; }
    }
    const cands = new Set(OVERLAYS ? corpusFiles.filter((x) => x.endsWith(".json")) : byBase.get(path.basename(r).toLowerCase()) || []);
    const names = new Set(); JSON.stringify(target).replace(/\/Game\/[A-Za-z0-9_./-]+/g, (m) => { names.add(m.split('/').pop().split('.')[0].toLowerCase() + '.json'); return m; });
    // (v2) and the KH3 effects the file says it was made from (its source package, the effect names its source notes give)
    if (target && target.source && typeof target.source === 'object') JSON.stringify(target.source).replace(/Content\/[A-Za-z0-9_./-]+\.uasset|\b[a-z]{1,3}_[a-z0-9_]{3,}\b/g, (m) => { names.add(m.split('/').pop().replace(/\.uasset$/, '').toLowerCase() + '.json'); return m; });
    for (const n of names) for (const id of byBase.get(n) || []) cands.add(id);
    const ids = [...cands].slice(0, 64);
    const idx = indexValues(ids);
    const G = /\/(effects|effects_xwd|materials)\//.test(r) || isAux ? globalIndex() : null;
    let refs = 0, own = 0, jp = false; const corpus = ids.slice(); const ci = new Map(corpus.map((id, i) => [id, i]));
    const jpKeys = new Set();
    const tpl = (function walk(v) {
      if (Array.isArray(v)) return v.map(walk);
      if (v !== null && typeof v === 'object') {
        const ks = Object.keys(v);
        // an object with a Japanese key: its entries in order, that key by its hash (`$obj@packgen`)
        if (ks.some((k) => JP.test(k))) { for (const k of ks) if (JP.test(k)) jpKeys.add(k); return { '$obj@packgen': ks.map((k) => [JP.test(k) ? { h: hkey(k) } : k, walk(v[k])]) }; }
        const o = {}; for (const k of ks) o[k] = walk(v[k]); return o;
      }
      if (eligible(v)) {
        const key = typeof v + ':' + String(v);
        const hl = idx.get(key); if (hl) { refs++; return { '$ref@packgen': [hl[0], encPtr(hl[1])] }; }
        const hg = G && G.get(key); if (hg) { if (!ci.has(hg[0])) { ci.set(hg[0], corpus.length); corpus.push(hg[0]); } refs++; return { '$ref@packgen': [ci.get(hg[0]), encPtr(hg[1])] }; }
        own++;
      }
      if (typeof v === 'string' && JP.test(v)) jp = true;
      return v;
    })(target);
    // (every Japanese key must be one of the corpus files' keys: the replay finds it there by its hash)
    if (jpKeys.size) { const have = new Set(); for (const id of corpus) { const j = loadCorpusJson(id); (function kw(x) { if (x && typeof x === 'object') for (const k of Object.keys(x)) { if (JP.test(k)) have.add(k); kw(x[k]); } })(j); } if ([...jpKeys].some((k) => !have.has(k))) jp = true; }
    if (jp) { unresolved.push([r, 'Japanese text not found in the corpus', inCore]); continue; }
    // an effect our own scripts composed (no KH3 asset: `source.authored`) is our design: its values the corpus does not
    // have are our parameters, whatever their number (every KH3 value in it is a reference)
    const authored = target && target.source && typeof target.source.authored === 'string' && !target.source.package;
    // (a stream's effect pack manifest outside the pack is a build record: its own values are its records' sizes and hashes)
    if (own > MAX_OWN && !authored && !isAux) { unresolved.push([r, `${own} values not in the corpus (more than ${MAX_OWN})`, inCore]); continue; }
    // (recipes v2, tools/packgen_diff.js) the file as its differences from the corpus: every part that is the same as a
    // corpus file (small integers and short strings included) is a reference; what is left is ours
    const v1Literals = (function count(v) { if (Array.isArray(v)) return v.reduce((s, x) => s + count(x), 0); if (v !== null && typeof v === 'object') { if (v['$ref@packgen']) return 0; if (v['$obj@packgen']) return v['$obj@packgen'].reduce((s, [, x]) => s + count(x), 0); return Object.values(v).reduce((s, x) => s + count(x), 0); } return 1; })(tpl);
    const dstats = { refs: 0, tree_refs: 0, literals: 0, diffs: 0, jp: false };
    const corpusIndex = (id) => { if (!ci.has(id)) { ci.set(id, corpus.length); corpus.push(id); } return ci.get(id); };
    // (v2) and the files of this root the replay made before, of the same family (name without its numbers / last part):
    // the streams' composed effects repeat each other (grv1 / grv2 / grv3, the HYB parts)
    const stem = (p) => path.posix.basename(p).replace(/.json$/, '').replace(/[0-9]+$/, '').replace(/_[a-z0-9]*$/, '');
    const earlier = isAux ? [] : (madeByRoot.get(root) || []).filter((m) => stem(m.rel) === stem(r) || stem(stem(m.rel) + '_x') === stem(stem(r) + '_x')).slice(-6);
    const files2 = ids.map((id) => ({ id, json: loadCorpusJson(id) })).filter((f) => f.json !== null).concat(earlier.map((m) => ({ id: 'out:' + m.rel, json: m.json })));
    const scalarRef = (v) => {
      if (!eligible(v)) return null;
      const key = typeof v + ':' + String(v);
      const hl = idx.get(key); if (hl) return [corpusIndex(ids[hl[0]]), encPtr(hl[1])];
      const hg = G && G.get(key); if (hg) return [corpusIndex(hg[0]), encPtr(hg[1])];
      return null;
    };
    const tpl2 = DIFF.encoder({ files: files2, corpusIndex, scalarRef, encPtr, jp: JP, hkey, jpKeys, stats: dstats, findEmitter: /\/effects[^/]*\//.test(r) ? findEmitter : null, findTree: /\/effects[^/]*\//.test(r) ? findTree : null })(target);
    if (dstats.jp) { unresolved.push([r, 'Japanese text not found in the corpus (v2)', inCore]); continue; }
    if (!isAux) { const id = 'out:' + r; OUT_MADE.set(id, target); if (!madeByRoot.has(root)) madeByRoot.set(root, []); madeByRoot.get(root).push({ rel: r, json: target }); }
    put({ kind: 'json', v: 2, format: fmt, corpus, template: tpl2, own_values: own, literals: dstats.literals, literals_v1: v1Literals, ...(bake || {}) });
    stats.json++; stats.json_refs += refs; stats.json_own_values += own; stats.v1_literals = (stats.v1_literals || 0) + v1Literals; stats.v2_literals = (stats.v2_literals || 0) + dstats.literals; stats.v2_tree_refs = (stats.v2_tree_refs || 0) + dstats.tree_refs; continue;
  }
  // a WGSL table (--overlays: an overlay; else a translated material table the pack tools make in another version): each
  // line a line of a translator run (or of the folder's tables); our own lines may only be comments, preprocessor lines or blank
  if (/\.wgsl$/i.test(r)) {
    const lines = want.toString('utf8').split('\n');
    const ids = OVERLAYS ? corpusFiles.filter((x) => x.endsWith('.wgsl')) : corpusFiles.filter((x) => x.startsWith('pack:' + path.posix.dirname(r) + '/') && x.endsWith('.wgsl'));
    const at = new Map(); ids.forEach((id, ci) => fs.readFileSync(corpusPath(id), 'utf8').split('\n').forEach((l, li) => { if (!at.has(l)) at.set(l, [ci, li]); }));
    let code = 0, refs = 0;
    // (and our own fixes over the translated code: ER85's kh_div1 divisions, a few lines at most)
    let fixes = 0;
    const out = lines.map((l) => { const h = at.get(l); if (h) { refs++; return h; } if (/^\s*(\/\/.*)?$/.test(l) || /^#(ifdef|ifndef|endif|else|define)\b/.test(l)) return l; if (/^\s+[a-z0-9.]+ = kh_div1\([a-z0-9.]+, [a-z0-9.]+\);$/.test(l) && ++fixes <= 8) return l; code++; return l; });
    if (code || JP.test(want.toString('utf8'))) { unresolved.push([r, `${code} code line(s) not in the translator runs`, inCore]); continue; }
    put({ kind: 'text', corpus: ids, lines: out }); stats.text = (stats.text || 0) + 1; continue;
  }
  if (/\.(wgsl|hlsl)$/i.test(r)) { unresolved.push([r, 'shader table: made by the shader steps', inCore]); continue; }
  unresolved.push([r, 'binary file not in the corpus']);
}
stats.unresolved = unresolved.length;
const PREFIX = OVERLAYS ? 'overlays' : 'files';
fs.mkdirSync(OUTDIR, { recursive: true });
for (const f of fs.readdirSync(OUTDIR)) if (f.startsWith(PREFIX + '_') && f.endsWith('.json')) fs.unlinkSync(path.join(OUTDIR, f));
for (const [root, recs] of Object.entries(recipes)) { const text = JSON.stringify({ root, files: recs }) + '\n'; if (JP.test(text)) throw new Error(`${PREFIX}_${root}: Japanese text in a recipe`); fs.writeFileSync(path.join(OUTDIR, `${PREFIX}_${root.replace(/\//g, '_')}.json`), text); }
const byReason = {}; for (const [r, why, inCore] of unresolved) { const k = rootOf(r) + (inCore ? ' (the pack tools make another version)' : ' (missing)') + ': ' + why.replace(/^\d+ values/, 'N values'); byReason[k] = (byReason[k] || 0) + 1; }
fs.writeFileSync(path.join(OUTDIR, OVERLAYS ? 'report_overlays.json' : 'report.json'), JSON.stringify({ about: 'tools/packgen_recipe.js compile: what the replay recipes make and what they cannot', stats, unresolved_by_reason: byReason, unresolved: unresolved.map(([r, why, inCore]) => ({ file: r, why, pack_tools_make_another_version: !!inCore })) }, null, 1) + '\n');
console.log(JSON.stringify(stats));
for (const [k, v] of Object.entries(byReason).sort()) console.log(String(v).padStart(6), k);
