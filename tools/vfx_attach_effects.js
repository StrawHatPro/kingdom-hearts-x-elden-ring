#!/usr/bin/env node
// tools/vfx_attach_effects.js (VFX-XLATE): THE KEYBLADES' ATTACH DATA AS EFFECTS OF THE KEYBLADES' ROOT.
//
// An attach data asset (SQEX_ParticleAttachDataAsset, the ead/ folders) is no particle system: it lists rows (GroupID, the
// particle system, where it attaches). The game spawns its rows by the group of the request; the core spawns the asset's
// name (SHOT-A: `ead_so021_sht0` Diving Barrage's muzzle flashes, `ead_so053_stf0` Drill Dive's warp, `ead_shf_tame0`
// Snakebite's charge), and the effect host found no effect of that name: nothing was drawn. tools/sora_keyblades_pack.js
// packed their particle systems (manifest vfx.where `attach_data`) but no effect named after the asset.
// This tool writes, into the root's effects/ folder, one effect per attach data asset whose rows all share one group (or that
// has rows of group 0, the always-group: those rows), named as the asset, and one per (asset, group) `<asset>__g<group>`,
// each a set of control emitters that spawn the rows' systems at time 0 (SQEX_SpawnParticleSystem: the host registers and
// places them as it does any nested system) [approximation: every row sits where the request puts the asset; a row's own
// attach target (weapon socket / owner) is not applied]. A request whose group picks the rows needs the host to try
// `<asset>__g<group>` first (kh-host vfx.rs resolve_spawn: spawn.id.0 is the group).
//
//   node tools/vfx_attach_effects.js [--root PACK/keyblades/vfx] [--out DIR] [--also /Game/Effects/...,...]
//   --out: write there instead of ROOT/effects (staging); --also: more attach data assets (the common ones the core names:
//   default /Game/Effects/wep/w_com/ead/ead_shf_tame0)
'use strict';
const fs = require('fs');
const path = require('path');
const ROOT = path.resolve(__dirname, '..');
const PKG = path.join(ROOT, 'assets_converted/packages/KINGDOM HEARTS III/Content');
const opt = (k, d) => { const i = process.argv.indexOf(k); return i > 0 ? process.argv[i + 1] : d; };
const KB = path.resolve(opt('--root', path.join(ROOT, 'build/sora_pack/keyblades/vfx')));
const OUT = path.resolve(opt('--out', path.join(KB, 'effects')));
const ALSO = opt('--also', '/Game/Effects/wep/w_com/ead/ead_shf_tame0').split(',').filter(Boolean);
const PACK = path.dirname(path.dirname(KB)); // build/sora_pack
const roots = ['vfx', 'party/vfx', 'xsi/vfx', 'keyblades/vfx'].map((r) => path.join(PACK, r, 'effects')).filter((d) => fs.existsSync(d));
const have = new Map(); for (const d of roots) for (const f of fs.readdirSync(d)) if (f.endsWith('.json')) { const k = f.slice(0, -5).toLowerCase(); if (!have.has(k)) have.set(k, f.slice(0, -5)); }
const props = (arr) => Object.fromEntries((arr || []).map((p) => [p.name, p.value]));
function rows(asset) { // -> [{ group, system, attach, socket }]
  const f = path.join(PKG, asset.replace(/^\/Game\//, '').split('.')[0] + '.uasset.json'); if (!fs.existsSync(f)) return null;
  const d = JSON.parse(fs.readFileSync(f, 'utf8'));
  const imp = (i) => { let im = d.imports[-i - 1]; const n = [im.objectName]; while (im.outerIndex) { im = d.imports[-im.outerIndex - 1]; n.unshift(im.objectName); } return n[0]; };
  const e = d.exports.find((x) => x.class === 'SQEX_ParticleAttachDataAsset'); if (!e) return null;
  const list = (e.data.props.find((p) => p.name === 'EffectDataList') || {}).value; const out = [];
  for (const outer of (list && list.items) || []) {
    const a = props(outer); const inner = a.EffectDataList; for (const it of (inner && (inner.items || (inner.value || {}).items)) || []) {
      const r = props(it); if (!(r.AttachEffect < 0)) continue; const ao = props(r.AttachObjectData || []);
      out.push({ group: r.GroupID, system: imp(r.AttachEffect), attach: ao.AttachType || null, socket: ao.ObjectSearchSocketName && ao.ObjectSearchSocketName !== 'None' ? ao.ObjectSearchSocketName : null });
    }
  }
  return out;
}
// the template: a control emitter that spawns a nested system (VW_SO035_THD0's Control_lp, keyblades root)
const tplEffect = JSON.parse(fs.readFileSync(path.join(KB, 'effects', 'VW_SO035_THD0.json'), 'utf8'));
const tplEmitter = tplEffect.emitters.find((x) => x.modules.some((m) => m.class === 'ParticleModuleSQEX_SpawnParticleSystem'));
function effectOf(name, asset, list) {
  const e = JSON.parse(JSON.stringify(tplEffect));
  Object.assign(e, { name, asset, object: name, source: { attach_data: asset, generator: 'tools/vfx_attach_effects.js (VFX-XLATE)' }, references: [], unsupported: {}, used_by: [], files: {} });
  e.emitters = list.map((r, k) => {
    const em = JSON.parse(JSON.stringify(tplEmitter));
    em.index = k; em.name = `row${k}_${r.system}`; em.object = `${name}_row${k}`;
    em.modules = em.modules.filter((m) => m.class === 'ParticleModuleSQEX_SpawnParticleSystem');
    em.enabled = true; em.required.params = { ...(em.required.params || {}), bAutoAddToComponent: true }; // (the template is a child emitter)
    em.modules[0].params = { ...em.modules[0].params, SpawnTime: 0, LoopEndTime: 0, ParticleSystemAsset: `${r.system}.${r.system.split('/').pop()}` };
    em.attach_row = r; // (for reading: the row's group and attach target)
    return em;
  });
  return e;
}
// ONLY the assets a request is known to name and to draw nothing for (SHOT-A); the others are left alone: the host handles
// some itself (ead_wep_cal0, ead_com_abn_w0: the cast / heal rows) and a new effect of that name could double what it draws
// (ead_key_trail0: the trail). `--assets all`: every attach data asset of the manifest.
const manifest = JSON.parse(fs.readFileSync(path.join(PACK, 'keyblades', 'manifest.json'), 'utf8'));
const KNOWN = ['/Game/Effects/wep/w_so020/ead/ead_so021_sht0', '/Game/Effects/wep/w_so050/ead/ead_so053_stf0'];
const all = Object.entries(manifest.vfx.where).filter(([, v]) => v && v.attach_data).map(([k]) => k);
const assets = [...new Set([...(opt('--assets', '') === 'all' ? all : KNOWN.filter((k) => all.includes(k))), ...ALSO])].sort();
fs.mkdirSync(OUT, { recursive: true });
let wrote = 0; const lines = [];
for (const asset of assets) {
  const name = asset.split('/').pop().split('.')[0]; const list = rows(asset);
  if (!list || !list.length) { lines.push(`${name}: no rows read`); continue; }
  const missing = list.filter((r) => !have.has(r.system.split('/').pop().toLowerCase()));
  if (missing.length) lines.push(`${name}: systems not in any root: ${missing.map((r) => r.system).join(', ')} (left out)`);
  const ok = list.filter((r) => !missing.includes(r));
  const groups = [...new Set(ok.map((r) => r.group))];
  const write = (n, l) => { if (!l.length) return; fs.writeFileSync(path.join(OUT, n + '.json'), JSON.stringify(effectOf(n, asset, l))); wrote++; };
  const base = groups.length === 1 ? ok : ok.filter((r) => r.group === 0);
  write(name, base);
  if (groups.length > 1) for (const g of groups) write(`${name}__g${g}`, ok.filter((r) => r.group === g));
  lines.push(`${name}: ${ok.length} row(s), group(s) ${groups.join(' ')}${base.length ? ` -> ${name} (${base.length})` : ' -> per group only'}`);
}
console.log(lines.join('\n'));
console.log(`vfx_attach_effects: ${wrote} effect file(s) -> ${OUT}`);
