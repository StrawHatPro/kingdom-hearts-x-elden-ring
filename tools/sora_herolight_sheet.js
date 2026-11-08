#!/usr/bin/env node
// (HEROLIGHT-KB, 2026-11-05) THE HERO LIGHT ON EVERY WEAPON: one row per weapon model a hero-light carrier can hold
// (Sora's 17 Keyblades and every form / grow-level weapon of theirs, Donald's 14 staffs, Goofy's 14 shields), each traced
// from KH3's own data. The user: "the light effect for the hero light in dark places needs to adapt to all the keyblades"
// (and "for the hero light, it should just be whoever is party leader, not dependent on Sora": the leader's weapon, any).
//
//   node tools/sora_herolight_sheet.js           write sheets/port/sora-herolight.json
//   node tools/sora_herolight_sheet.js --check   exit 1 when the sheet differs from what this tool makes
//
// WHAT EACH ROW SAYS AND WHERE IT COMES FROM
// - effect: the weapon blueprint's `m_AppearEffect` (its class default object, along the blueprint chain: Sora's
//   w_soNNN_Wep, Donald's w_do_WepBase, Goofy's w_gu_WepBase), the effect KH3 plays when that weapon is drawn; the pack's
//   file of that asset (its `asset` field), in the root that has it (vfx, keyblades/vfx, party/vfx).
// - glow: the emitters of that effect held STEADY while the light is on (IMPL_ER53's rule, the user: "the glow on the
//   keyblade pulses instead of being constant while in the dark, it must be constant"): spawned once (one burst, no rate)
//   and either (a) a MESH COPY OF THE WEAPON (a mesh particle whose mesh reaches the weapon's tip socket: its +X extent
//   within 15 % of vfx_top, at least half of it long; not opaque / masked), or a skeletal-mesh copy of a weapon (the form
//   weapons' TypeDataSQEX_SkeletalMesh emitters of /Game/Character/wep/<model>/mdl/<model>_0), or (b) the HALO at the hand (an additive sprite
//   in the socket's local space with a glow material, `*glw*`).
// - spark: the emitters that keep spawning while the effect runs (a spawn rate above 0: KH3's sparkles, glitter, petals,
//   snow, cubes ... of that weapon), with the control parents that generate them; restarted at `hero.sparkle_s`.
//   Neither list ever takes the effect's particle light (ERM_LightsOnly / ParticleModuleLight): the hero light is the light.
// - hold: the time the glow copy's clock is held at: the latest glow emitter's delay + the age at which its alpha over life
//   first reaches 95 % of its peak (its first full brightness; a particle without a Lifetime module runs on its emitter's
//   time: the emitter's duration is its timeline), at least one step after its delay.
// - socket: the character socket the weapon hangs on (the equip unit's socket; the party weapons' attach socket); the effect
//   is started there, as KH3's Draw starts the appear effect.
// - line: where the light sits on the weapon: from vfx_bottom to vfx_top (else ATK_00 to ATK_01) of the weapon's own
//   skeleton sockets (bone, local cm); the host puts it at the Kingdom Key's fraction of that line (`hero.blade_cm` on the
//   Kingdom Key's 14.2 .. 79 cm), so every weapon gets the same place relative to its own blade.
// - glow_color: the glow emitters' StartColor x ColorScaleOverLife at the hold (KH3 linear colour; the mesh copy first,
//   else the halo; only emitters with a colour module: a skeletal copy's colour is its material's and keeps tint 1); tint: that colour relative to the Kingdom Key's glow of the same kind, each at luma 1 (the Kingdom
//   Key is (1, 1, 1): the hero light's own colour stays as it is for it and every other weapon turns by its own glow).
// - kh3_light: the appear effect's own particle light (BrightnessOverLife, RadiusScale, LightExponent), for the record: KH3's
//   appear lights are all white (StartColor 1 x ColorScale 1), so the colour can only follow the glow.
'use strict';
const fs = require('fs');
const path = require('path');

const VERSION = '1.0.0';
const R = path.resolve(__dirname, '..');
const PACK = path.join(R, 'build/sora_pack');
const PK = path.join(R, 'assets_converted/packages/KINGDOM HEARTS III/Content') + '/';
const ROOTS = ['vfx', 'keyblades/vfx', 'party/vfx'];

// ------------------------------------------------------------------ package helpers (as tools/party_audio_pack.js)
const cache = new Map();
const gamePathToRel = (p) => { let s = String(p).replace(/^\/Game\//, ''); const i = s.lastIndexOf('.'); if (i > s.lastIndexOf('/')) s = s.slice(0, i); return s; };
function loadPkg(rel) { if (cache.has(rel)) return cache.get(rel); const f = PK + rel + '.uasset.json'; let d = null; if (fs.existsSync(f)) d = JSON.parse(fs.readFileSync(f, 'utf8')); if (cache.size > 60) cache.clear(); cache.set(rel, d); return d; }
function full(d, i) { if (!i) return null; if (i < 0) { let im = d.imports[-i - 1], s = im.objectName; while (im.outerIndex) { im = d.imports[-im.outerIndex - 1]; s = im.objectName + '.' + s; } return s; } return 'E' + i + ':' + d.exports[i - 1].objectName; }
function simp(d, p) {
  if (p === null || typeof p !== 'object') return p;
  if (Array.isArray(p)) { if (p.length && p.every((e) => e && typeof e === 'object' && 'name' in e && 'type' in e)) { const o = {}; for (const e of p) o[e.name + (e.arrayIndex ? '[' + e.arrayIndex + ']' : '')] = sv(d, e); return o; } return p.map((x) => simp(d, x)); }
  if ('name' in p && 'type' in p) return sv(d, p);
  if ('items' in p && Object.keys(p).length <= 3) return p.items.map((x) => simp(d, x));
  const o = {}; for (const k in p) o[k] = simp(d, p[k]); return o;
}
function sv(d, e) { if (e.type === 'ObjectProperty') return full(d, e.value); if (e.type === 'ArrayProperty' && e.innerType === 'ObjectProperty' && Array.isArray(e.value)) return e.value.map((x) => full(d, x)); return simp(d, e.value); }
const cdo = (rel) => { const d = loadPkg(rel); if (!d) return null; const e = d.exports.find((x) => /^Default__/.test(x.objectName)); return e ? { props: simp(d, (e.data && e.data.props) || []), super: superOf(d) } : null; };
function superOf(d) {
  const cls = d.exports.find((e) => e.class === 'BlueprintGeneratedClass'); if (!cls || !cls.superIndex || cls.superIndex > 0) return null;
  let im = d.imports[-cls.superIndex - 1]; while (im.outerIndex) im = d.imports[-im.outerIndex - 1];
  return /^\/Game\//.test(im.objectName) ? gamePathToRel(im.objectName + '.x') : null;
}
function chainProp(rel, name) { for (let r = rel, n = 0; r && n < 8; n++) { const c = cdo(r); if (!c) return null; if (c.props[name] !== undefined && c.props[name] !== null) return { value: c.props[name], from: r }; r = c.super; } return null; }
// the blueprint path's package, whatever its case on disk (w_so032_wep / w_so032_Wep)
function pkgRel(blueprint) {
  const rel = gamePathToRel(blueprint + '.x');
  if (fs.existsSync(PK + rel + '.uasset.json')) return rel;
  const dir = path.dirname(PK + rel); const base = path.basename(rel).toLowerCase() + '.uasset.json';
  const hit = fs.existsSync(dir) && fs.readdirSync(dir).find((f) => f.toLowerCase() === base);
  return hit ? path.posix.join(path.posix.dirname(rel), hit.replace(/\.uasset\.json$/, '')) : rel;
}
const objPath = (s) => String(s).replace(/^</, '').replace(/>$/, '').replace(/\.[^./]+$/, '');

// ------------------------------------------------------------------ the pack's effects
// The pack's file of an effect asset: its base name (or a clash-suffixed one, `name__a_b_c`) whose `asset` is that path.
function effectFile(asset) {
  const base = asset.split('/').pop().toLowerCase();
  for (const root of ROOTS) {
    const dir = path.join(PACK, root, 'effects');
    if (!fs.existsSync(dir)) continue;
    for (const f of fs.readdirSync(dir)) {
      const stem = f.replace(/\.json$/, '');
      const low = stem.toLowerCase();
      if (low !== base && !low.startsWith(base + '__')) continue;
      const json = JSON.parse(fs.readFileSync(path.join(dir, f), 'utf8'));
      if (String(json.asset).toLowerCase() === asset.toLowerCase()) return { root, stem, json };
    }
  }
  return null;
}
function glbMaxX(file) {
  if (!fs.existsSync(file)) return null;
  const b = fs.readFileSync(file); const len = b.readUInt32LE(12); const j = JSON.parse(b.slice(20, 20 + len).toString());
  let mn = Infinity, mx = -Infinity;
  for (const m of j.meshes || []) for (const p of m.primitives) { const a = j.accessors[p.attributes.POSITION]; if (a && a.min) { mn = Math.min(mn, a.min[0]); mx = Math.max(mx, a.max[0]); } }
  return Number.isFinite(mx) ? { min: mn * 100, max: mx * 100 } : null; // the pack's meshes are in metres
}
const mod = (e, cls) => (e.modules || []).find((m) => m.class === cls);
// samples over life (each a number list); a constant is one sample (its evaluated value is the flat value itself)
function evaluated(dist) { const d = dist && (dist.distribution || dist); if (!d) return null; let v = null; if (d.evaluated && Array.isArray(d.evaluated.value)) v = d.evaluated.value; else if (d.constant !== undefined) v = [].concat(d.constant); if (!v || !v.length) return null; return Array.isArray(v[0]) ? v : [v]; }
const maxOf = (vals) => (vals ? Math.max(...vals.map((v) => Math.max(...[].concat(v)))) : 0);
function sampleAt(vals, t) { if (!vals || !vals.length) return null; const i = Math.min(vals.length - 1, Math.max(0, Math.round(t * (vals.length - 1)))); return [].concat(vals[i]); }
function rateOf(e) { return maxOf(evaluated(e.spawn && e.spawn.rate)); }
function burstsOf(e) { const b = (e.spawn && (e.spawn.bursts || e.spawn.burst_list)) || []; return b.reduce((a, x) => a + Math.max(0, Number(x.count) || 0), 0); }
const isLight = (e) => e.render_mode === 'ERM_LightsOnly' || Boolean(mod(e, 'ParticleModuleLight'));
const lifetimeOf = (e) => { const l = mod(e, 'ParticleModuleLifetime'); return l ? maxOf(evaluated(l.params.Lifetime)) : 0; };
const req = (e) => e.required || {};
const kindOf = (e) => (e.typedata && e.typedata.kind) || 'sprite';

function classify(fx, root, top) {
  const glow = [], halo = [], spark = [];
  const parents = new Map(); // child name -> control parents
  for (const e of fx.emitters) {
    const p = mod(e, 'ParticleModuleSQEX_Parent');
    for (const c of (p && p.params && p.params.ChildrenData) || []) { if (!parents.has(c.GenerateEmitterName)) parents.set(c.GenerateEmitterName, []); parents.get(c.GenerateEmitterName).push(e.name); }
  }
  for (const e of fx.emitters) {
    if (e.enabled === false || isLight(e)) continue;
    const r = req(e); const blend = String(r.emitter_blend_type || ''); const once = rateOf(e) === 0 && burstsOf(e) === 1;
    const mat = String(r.material || '').split('/').pop().toLowerCase();
    if (once && kindOf(e) === 'mesh' && !/Opaque|Masked/.test(blend) && e.typedata.mesh_file && top > 0) {
      const b = glbMaxX(path.join(PACK, root, e.typedata.mesh_file));
      if (b && Math.abs(b.max - top) <= 0.15 * top && b.max - Math.max(b.min, 0) >= 0.5 * top) { glow.push(e); continue; }
    }
    // (c) a skeletal-mesh copy of a weapon (TypeDataSQEX_SkeletalMesh: one component per emitter instance, of the weapon's own
    // mesh /Game/Character/wep/<model>/mdl/<model>_0: the form weapons' appear effects)
    if (kindOf(e) === 'sqex_skeletalmesh' && !/Opaque|Masked/.test(blend) && /\/Character\/wep\/w_[a-z]{2}\d{3}\//i.test(String(e.typedata.params && e.typedata.params.SkeletalMesh))) { glow.push(e); continue; }
    if (once && kindOf(e) === 'sprite' && r.use_local_space === true && /Additive/.test(blend) && mat.includes('glw')) { halo.push(e); continue; }
    if (rateOf(e) > 0 && ['sprite', 'gpu_sprite', 'mesh'].includes(kindOf(e))) spark.push(e);
  }
  // a spawning emitter generated BY a glow emitter (the form weapons' skeletal copies generate their glitter over themselves)
  // is left out: restarting it would restart the copy over the held one (a pulse); the other parents come along
  const glowNames = new Set([...glow, ...halo].map((e) => e.name));
  const kept = spark.filter((e) => !(parents.get(e.name) || []).some((p) => glowNames.has(p)));
  const sparkNames = new Set(kept.map((e) => e.name));
  for (const e of kept) for (const p of parents.get(e.name) || []) sparkNames.add(p);
  return { glow, halo, spark: fx.emitters.filter((e) => sparkNames.has(e.name)).map((e) => e.name) };
}
// the emitter's first full brightness: (delay + age, colour there)
function peak(e) {
  const c = mod(e, 'ParticleModuleColor'); const s = mod(e, 'ParticleModuleColorScaleOverLife');
  const alpha = evaluated(s && s.params.AlphaScaleOverLife) || [[1]];
  const top = maxOf(alpha); let i = alpha.findIndex((v) => [].concat(v)[0] >= 0.95 * top); if (i < 0) i = 0;
  const t = alpha.length > 1 ? i / (alpha.length - 1) : 0;
  // a particle without a Lifetime module is the emitter's own body: its colour runs on the EMITTER's time (kh-vfx sim.rs,
  // bKillOnCompleted with no lifetime), so its timeline is the emitter's duration
  const delay = Number(req(e).delay) || 0; const life = lifetimeOf(e) || Number(req(e).duration) || 0;
  const start = sampleAt(evaluated(c && c.params.StartColor), 0) || [1, 1, 1];
  const scale = sampleAt(evaluated(s && s.params.ColorScaleOverLife), t) || [1, 1, 1];
  const color = [0, 1, 2].map((k) => (Number(start[k] ?? start[0]) || 0) * (Number(scale[k] ?? scale[0]) || 0));
  return { hold: delay + Math.max(life * t, 1 / 60), color, duration: (Number(req(e).duration) || 0) + delay };
}
const luma = (c) => 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
const unit = (c) => { const l = luma(c); return l > 1e-6 ? c.map((x) => x / l) : [1, 1, 1]; };
const r4 = (x) => Math.round(x * 1e4) / 1e4;

// ------------------------------------------------------------------ the weapons
const manifest = JSON.parse(fs.readFileSync(path.join(PACK, 'keyblades/manifest.json'), 'utf8'));
const kbSheet = JSON.parse(fs.readFileSync(path.join(R, 'sheets/port/sora-keyblades.json'), 'utf8'));
const KEY = kbSheet.columns.indexOf('keyblade'), KIND = kbSheet.columns.indexOf('kind'), NAME = kbSheet.columns.indexOf('name'), BP = kbSheet.columns.indexOf('blueprint');
const equippable = kbSheet.rows.filter((r) => r[KIND] === 'keyblade').map((r) => ({ key: r[KEY], name: r[NAME], model: String(r[BP]).split('/').pop().replace(/_Wep$/i, '').toLowerCase() }));
const weapons = new Map(); // model -> { owner, keys, forms, blueprint, socket, sockets }
function addWeapon(model, owner, w) {
  if (!weapons.has(model)) weapons.set(model, { model, owner, keys: [], forms: [], blueprint: w.blueprint, socket: w.socket, sockets: w.sockets });
  const x = weapons.get(model);
  for (const k of w.keys || []) if (!x.keys.includes(k)) x.keys.push(k);
  if (w.form && !x.forms.includes(w.form)) x.forms.push(w.form);
}
const kbSockets = (model) => { const f = path.join(PACK, 'keyblades', model, 'sockets.json'); return fs.existsSync(f) ? JSON.parse(fs.readFileSync(f, 'utf8')).weapon_sockets || {} : {}; };
for (const k of equippable) {
  const entry = manifest.keyblades[k.model];
  if (!entry) throw new Error('sora-herolight: ' + k.key + ' (' + k.model + ') is not in the keyblades manifest');
  for (const f of entry.forms) for (const w of f.weapons) {
    if (!w.blueprint || !w.socket) continue;
    const base = f.form === 'TRES_WEAPON_FORM_01';
    addWeapon(w.model, 'sora', { keys: [k.key], blueprint: w.blueprint, socket: w.socket, sockets: kbSockets(w.model), form: base ? null : f.form.replace('TRES_WEAPON_FORM_', '') + ':' + f.grow_level.replace('TRES_WEAPON_GROW_LVL0', '') + ':' + w.part });
  }
}
for (const [owner, prefix] of [['donald', 'w_do'], ['goofy', 'w_gu']]) {
  for (let n = 10; n <= 140; n += 10) {
    const model = prefix + String(n).padStart(3, '0');
    const f = path.join(PACK, 'party/weapons', model, 'manifest.json');
    if (!fs.existsSync(f)) throw new Error('sora-herolight: no party weapon ' + model);
    const m = JSON.parse(fs.readFileSync(f, 'utf8'));
    const sockets = {}; for (const s of m.sockets || []) sockets[s.name] = { bone: s.bone, local_location: s.relative_location_ue_cm };
    addWeapon(model, owner, { keys: [m.item_key], blueprint: m.blueprint, socket: m.attach.socket, sockets });
  }
}

// ------------------------------------------------------------------ rows
const columns = ['id', 'owner', 'model', 'keyblades', 'forms', 'blueprint', 'appear_asset', 'effect', 'root', 'glow', 'spark', 'hold', 'socket', 'line_bone', 'line_from', 'line_to', 'line_sockets', 'glow_color', 'tint', 'kh3_light', 'source', 'ref_conf', 'note'];
const rows = [];
const made = [];
for (const w of weapons.values()) {
  const rel = pkgRel(w.blueprint);
  const p = chainProp(rel, 'm_AppearEffect');
  if (!p) throw new Error('sora-herolight: ' + w.model + ': no m_AppearEffect along ' + rel);
  const asset = objPath(p.value);
  const found = effectFile(asset);
  if (!found) throw new Error('sora-herolight: ' + w.model + ': ' + asset + ' is in no root of the pack');
  const pair = w.sockets.vfx_bottom && w.sockets.vfx_top ? ['vfx_bottom', 'vfx_top'] : w.sockets.ATK_00 && w.sockets.ATK_01 ? ['ATK_00', 'ATK_01'] : null;
  const top = w.sockets.vfx_top ? Number(w.sockets.vfx_top.local_location[0]) : 0;
  const c = classify(found.json, found.root, top);
  const glowE = [...c.glow, ...c.halo];
  const peaks = glowE.map(peak);
  const hold = peaks.length ? Math.min(Math.max(...peaks.map((x) => x.hold)), Math.min(...peaks.map((x) => x.duration)) - 1 / 60) : 0;
  // (only emitters that carry a colour of their own: a skeletal copy without a colour module is the material's, untraced)
  const coloured = (list) => list.filter((e) => mod(e, 'ParticleModuleColor') || mod(e, 'ParticleModuleColorScaleOverLife'));
  const colorFrom = coloured(c.glow).length ? coloured(c.glow) : coloured(c.halo);
  const glowColor = colorFrom.length ? colorFrom.map(peak).reduce((a, x) => a.map((v, k) => v + x.color[k]), [0, 0, 0]) : [1, 1, 1];
  const light = found.json.emitters.find(isLight);
  const lm = light && mod(light, 'ParticleModuleLight');
  made.push({ w, rel, p, asset, found, pair, c, glowE, hold, glowColor, coloured: colorFrom.length > 0, kind: coloured(c.glow).length ? 'mesh' : 'halo', light: lm ? [r4(maxOf(evaluated(lm.params.BrightnessOverLife))), r4(maxOf(evaluated(lm.params.RadiusScale))), r4(maxOf(evaluated(lm.params.LightExponent)))] : [] });
}
const kk = made.find((m) => m.w.model === 'w_so010');
if (!kk) throw new Error('sora-herolight: no Kingdom Key row');
const kkHalo = kk.c.halo.length ? kk.c.halo.map(peak).reduce((a, x) => a.map((v, k) => v + x.color[k]), [0, 0, 0]) : [1, 1, 1];
const reference = { mesh: unit(kk.glowColor), halo: unit(kkHalo) };
for (const m of made) {
  const ref = reference[m.kind];
  // (no steady glow at all: the hero light's own colour, as the Kingdom Key's)
  const tint = m.coloured ? unit(unit(m.glowColor).map((v, k) => v / Math.max(ref[k], 1e-4))) : [1, 1, 1];
  const loc = (s) => (m.w.sockets[s].local_location || [0, 0, 0]).map((x) => r4(Number(x)));
  const notes = [];
  if (!m.coloured) notes.push('the glow carries no colour of its own (a skeletal copy: the colour is its material\'s): the hero light keeps its colour');
  if (!m.c.glow.length) notes.push('no mesh copy of the weapon in its appear effect: the halo is the steady glow');
  if (!m.glowE.length) notes.push('no steady glow emitter: only the sparkles');
  if (!m.c.spark.length) notes.push('no spawning emitter: no sparkles');
  if (!m.pair) notes.push('no vfx / ATK socket pair: the light sits at the weapon\'s socket');
  rows.push(['hl.' + m.w.model, m.w.owner, m.w.model, m.w.keys, m.w.forms, '/Game/' + m.p.from, m.asset, m.found.stem, m.found.root, m.glowE.map((e) => e.name), m.c.spark, r4(m.hold), m.w.socket,
    m.pair ? m.w.sockets[m.pair[0]].bone || 'wep' : '-', m.pair ? loc(m.pair[0]) : [], m.pair ? loc(m.pair[1]) : [], m.pair ? m.pair.join('>') : '-',
    m.glowColor.map(r4), tint.map(r4), m.light,
    m.p.from + '.uasset.json m_AppearEffect; ' + m.found.root + '/effects/' + m.found.stem + '.json emitters; ' + (m.w.owner === 'sora' ? 'keyblades/' + m.w.model + '/sockets.json' : 'party/weapons/' + m.w.model + '/manifest.json sockets'),
    'confirmed', notes.join('; ') || '-']);
}
rows.sort((a, b) => (a[1] === b[1] ? a[2].localeCompare(b[2]) : ['sora', 'donald', 'goofy'].indexOf(a[1]) - ['sora', 'donald', 'goofy'].indexOf(b[1])));
const missing = equippable.filter((k) => !rows.some((r) => r[2] === k.model));
if (missing.length) throw new Error('sora-herolight: equippable keyblades without a row: ' + missing.map((k) => k.key).join(', '));
const out = JSON.stringify({ sheet: 'sora-herolight', group: 'port', columns }).replace(/\}$/, ',"rows":[\n') + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n';
const file = path.join(R, 'sheets/port/sora-herolight.json');
if (process.argv.includes('--check')) {
  const same = fs.existsSync(file) && fs.readFileSync(file, 'utf8') === out;
  console.log(same ? 'sora-herolight: ' + rows.length + ' rows, current' : 'sora-herolight: sheet differs from tools/sora_herolight_sheet.js ' + VERSION);
  process.exit(same ? 0 : 1);
}
fs.writeFileSync(file, out);
console.log('sora-herolight: ' + rows.length + ' rows written (tools/sora_herolight_sheet.js ' + VERSION + ')');
for (const r of rows) console.log('  ' + r[2].padEnd(8) + ' ' + String(r[7]).padEnd(30) + ' glow ' + JSON.stringify(r[9]) + ' spark ' + r[10].length + ' hold ' + r[11] + ' tint ' + JSON.stringify(r[18]) + ' ' + r[16] + (r[22] !== '-' ? '  [' + r[22] + ']' : ''));
