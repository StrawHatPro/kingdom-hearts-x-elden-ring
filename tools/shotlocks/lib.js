// (SHOT-A) Helpers shared by tools/sora_shotlocks.js and the per-family row modules tools/shotlocks/<family>.js.
// A family module exports `(lib) => rows` (an array of rows made with lib.row); the generator loads every .js file of
// this folder except lib.js, sorted by name, and fails on a duplicate id. SHOT-B / SHOT-C add their own family files.
'use strict';
const fs = require('fs');
const path = require('path');
const ROOT = path.resolve(__dirname, '..', '..');
const P = (...a) => path.join(ROOT, ...a);
const readJson = (f) => JSON.parse(fs.readFileSync(f, 'utf8'));
const must = (c, m) => { if (!c) throw new Error(m); };
const r6 = (x) => (typeof x === 'number' ? Math.round(x * 1e6) / 1e6 : x);

// ---------------------------------------------------------------------------------------------- rows
// One row of the id / value sheet sora-shotlocks (the sora-magic-* format): [id, value, unit, source, evidence, ref_addr,
// ref_conf, note]. `value` is a number, boolean, string or a list of numbers / strings.
function row(id, value, unit, source, evidence, refAddr, refConf, note) {
  must(/^[a-z0-9_.]+$/.test(id), 'bad row id ' + id);
  const v = Array.isArray(value) ? value.map(r6) : r6(value);
  return [id, v, unit || '-', source || '-', evidence || '-', refAddr || '-', refConf || 'confirmed', note || '-'];
}

// ---------------------------------------------------------------------------------------------- the asset pack
const MAIN = readJson(P('build', 'sora_pack', 'manifest.json'));
const OVERLAY = P('build', 'sora_pack', 'keyblades', 'sora_forms_manifest.json');
const OV = fs.existsSync(OVERLAY) ? readJson(OVERLAY) : { items: [], logical_animations: [] };
const ITEMS = new Map([...MAIN.items, ...OV.items].filter((i) => i.kind === 'animation').map((i) => [i.animation_name, i]));
const setName = (a) => String(a || '').split('/').pop().split('.')[0];
// A clip by the name KH3 plays: a base-set logical name ("M_1475" -> sequence "1475"), a form set's logical name with its
// set ("M_2481", "p_ex001_AnimSet2" -> "2481"), or a sequence name. Returns { sequence, length, notifies } or null.
function clip(name, animSet) {
  let seq = null;
  if (animSet) {
    const a = OV.logical_animations.find((x) => x.name === name && setName(x.anim_set) === animSet && x.file);
    if (a) seq = a.clip;
  }
  if (!seq) {
    const a = MAIN.logical_animations.find((x) => x.name === name);
    if (a && a.file) seq = path.basename(a.file, '.glb');
    else if (a && a.samples && a.samples.length) seq = a.samples[0].animation_name;
  }
  if (!seq && ITEMS.has(name)) seq = name;
  const it = seq ? ITEMS.get(seq) : null;
  if (!it) return null;
  return { sequence: seq, length: it.duration_s, notifies: it.notifies || [] };
}
const prop = (n, key) => { const p = n.properties || {}; const v = p[key]; return v && typeof v === 'object' && 'value' in v ? v.value : v; };

// ---------------------------------------------------------------------------------------------- blueprints
const CONTENT = P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content');
const load = (pkg) => readJson(path.join(CONTENT, pkg.replace(/^\/Game\//, '').split('.')[0] + '.uasset.json'));
const objRef = (j, v) => (typeof v === 'number' && v < 0 ? j.imports[-v - 1] : typeof v === 'number' && v > 0 ? j.exports[v - 1] : null);
function plain(j, props) {
  const o = {};
  for (const p of props || []) {
    let v = p.value;
    if (v && v.items) v = v.items.map((it) => (Array.isArray(it) ? plain(j, it) : it));
    else if (Array.isArray(v) && v.length && v[0] && v[0].name) v = plain(j, v);
    else if (p.type === 'ObjectProperty') { const r = objRef(j, v); v = r ? r.objectName : null; }
    o[p.name + (p.arrayIndex ? `[${p.arrayIndex}]` : '')] = v;
  }
  return o;
}
// The class defaults (`cdo`), the components of the class default object (`comps`, by name) and the native parent class.
function blueprint(pkg) {
  const j = load(pkg);
  const cdo = j.exports.find((e) => e.objectName.startsWith('Default__'));
  must(cdo, pkg + ': no class default object');
  const comps = {};
  for (const e of j.exports) if (e.outerIndex === j.exports.indexOf(cdo) + 1 && e.data) comps[e.objectName] = plain(j, e.data.props);
  const cls = j.exports.find((e) => e.objectName === path.basename(pkg.split('.')[0]) + '_C');
  const sup = cls && cls.superIndex ? objRef(j, cls.superIndex) : null;
  return { cdo: plain(j, cdo.data.props), comps, parent: sup ? sup.objectName : null };
}
// A weapon's projectile set: name (FName) -> class ("HE_DRAIN" -> "TresProjHE_DrainEx_C").
function projectileSet(weapon) {
  const j = load('/Game/Blueprints/Player/p_ex001/' + weapon + '_ProjSet');
  const imp = j.imports.map((i) => i.objectName);
  const out = [];
  (function walk(v) {
    if (Array.isArray(v)) v.forEach(walk);
    else if (v && typeof v === 'object') {
      if (v.type === 'ObjectProperty' || v.type === 'ClassProperty') out.push('@' + (typeof v.value === 'number' && v.value < 0 ? imp[-v.value - 1] : v.value));
      else if (v.type === 'NameProperty' || v.type === 'StrProperty') out.push(String(v.value));
      for (const k in v) if (k !== 'value' || typeof v.value === 'object') walk(v[k]);
    }
  })(j.exports[0].data);
  const map = new Map();
  for (let i = 0; i + 1 < out.length; i++) if (!out[i].startsWith('@') && out[i + 1].startsWith('@')) map.set(out[i], out[i + 1].slice(1));
  return map;
}

// ---------------------------------------------------------------------------------------------- data tables
const ATTACK = new Map(readJson(P('sheets', 'entities', 'DataTable', 'Player', 'Attack', 'p_ex001_AttackData.json')).rows.map((r) => [r._row_name, r]));
// The decompiled function at an address ("0x1413B73D0"), as text (re/decomp); null when missing.
function decomp(addr) {
  const a = String(addr).replace(/^0x/i, '').toUpperCase();
  const f = P('re', 'decomp', a.slice(0, 5), a + '.c');
  return fs.existsSync(f) ? fs.readFileSync(f, 'utf8') : null;
}

module.exports = { ROOT, P, readJson, must, row, clip, prop, blueprint, projectileSet, ATTACK, decomp, setName, MAIN, OV };
