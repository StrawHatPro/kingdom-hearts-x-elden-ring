#!/usr/bin/env node
// The small driver tables of the Sora audio pack that tools/sora_audio_pack.js does not write:
//   build/sora_pack/audio/tables.json
//     events      the SEAD configuration's category events (STATE_SLOW_START ...), as decoded by the
//                 plugin research pass (audit/sora_port/audio/config_events.json; docs/sora-audio-spec.md 3.4)
//     static_se   ETresStaticSE id -> sound asset (43 rows; audit/sora_port/audio/code_tables.json
//                 static_se; spec 5.6), `packed` = the asset is in the pack's sound_assets
//     ui_kinds    PlaySystemSE kind -> static id (code_tables.json ui_system_se_kind)
//     projectiles (1.1.0) spell / shot actor class -> its sounds: the effects of the class that carry sounds
//                 (roles fire, body, hit, map_hit of build/sora_pack/vfx/triggers.json; the audio side plays the
//                 SEs attached to them, spec 5.5), the fire SE (MyFireEffects.MySoundCue, rows code.projectile)
//                 and the pawn / map hit SE (m_HitEffects[0], rows code.hit_magic) of the class or the nearest
//                 class of its chain that has one; `command` = the spell command that spawns the class
//     sockets     (1.1.0) Sora's skeleton sockets: name -> parent bone + relative location (cm, UE axes) and
//                 rotation (degrees), from build/sora_pack/vfx/host.json (tools/sora_vfx_host.js)
// Read by crates/kh-audio/src/pack.rs. Inputs are research outputs; nothing is invented here.
//   node tools/sora_audio_tables.js [--check]
'use strict';
const fs = require('fs');
const path = require('path');
const VERSION = '1.1.0';
const R = path.join(__dirname, '..');
const read = (f) => JSON.parse(fs.readFileSync(path.join(R, f), 'utf8'));
const cfg = read('audit/sora_port/audio/config_events.json');
const tables = read('audit/sora_port/audio/code_tables.json');
const manifest = read('build/sora_pack/audio/manifest.json');
const events = cfg.events.map((e) => ({
  number: e.number, name: e.name,
  commands: e.cmds.filter((c) => c.op !== 'End').map((c) => {
    if (typeof c.op !== 'string') return { op: 'Unknown', cmd: c.cmd, raw: c.raw };
    const o = { op: c.op.replace(/#.*/, '') };
    for (const k of ['category', 'volume', 'pitch', 'fade', 'layer', 'preset']) if (c[k] !== undefined) o[k] = typeof c[k] === 'number' ? +c[k].toPrecision(7) : c[k];
    return o;
  }),
}));
const static_se = tables.static_se.rows.map((r) => ({ id: r.id, asset: r.asset, packed: !!manifest.sound_assets[r.asset] }));
const ui_kinds = tables.ui_system_se_kind.rows.map((r) => ({ kind: r.kind, name: r.name, static_se: r.default_static_se }));
// ---- projectiles and sockets (inputs of the VFX pack; absent -> empty tables, said on the console)
const optional = (f) => (fs.existsSync(path.join(R, f)) ? read(f) : null);
const vfxTriggers = optional('build/sora_pack/vfx/triggers.json');
const vfxHost = optional('build/sora_pack/vfx/host.json');
const base = (p) => String(p).split('/').pop().split('.').pop();
const classSe = { fire: new Map(), pawn: new Map(), map: new Map() };
for (const t of manifest.triggers) {
  const cls = String(t.trigger || '').split(':')[0].trim();
  const asset = t.sounds[0] && t.sounds[0].asset;
  if (!asset || !/^TresProj/.test(cls)) continue;
  if (t.route === 'code.projectile') classSe.fire.set(cls, asset);
  else if (t.route === 'code.hit_magic') classSe[/map/.test(t.trigger) ? 'map' : 'pawn'].set(cls, asset);
}
const projectiles = {};
for (const r of (vfxTriggers ? vfxTriggers.rows : [])) {
  const p = r.params || {};
  if (r.trigger_kind !== 'projectile' || !p.blueprint) continue;
  const cls = base(p.blueprint) + '_C';
  const e = (projectiles[cls] = projectiles[cls] || { class: cls, chain: (p.class_chain || [p.blueprint]).map(base), command: null, effects: {} });
  if (p.command && !e.command && (p.depth === 0 || p.depth === undefined)) e.command = p.command;
  if (typeof r.effect === 'string' && r.effect !== '-') {
    const role = String(p.role || r.id.split('.').slice(2).join('.')).split('.').pop();
    const name = base(r.effect);
    (e.effects[role] = e.effects[role] || []).includes(name) || e.effects[role].push(name);
  }
}
for (const e of Object.values(projectiles)) {
  // the class's own value, else the nearest ancestor's (the chain is base first)
  const nearest = (m) => { for (const c of [...e.chain].reverse()) if (m.has(c)) return m.get(c); return null; };
  e.fire_se = nearest(classSe.fire); e.pawn_hit_se = nearest(classSe.pawn); e.map_hit_se = nearest(classSe.map);
}
const sockets = {};
for (const [name, s] of Object.entries(vfxHost ? vfxHost.sockets : {})) sockets[name] = { bone: s.bone, location: s.location, rotation: s.rotation };
if (!vfxTriggers || !vfxHost) console.log('sora_audio_tables: build/sora_pack/vfx/{triggers,host}.json absent: projectiles / sockets left empty');
const out = { format: 'kh3-sora-audio-tables/1', generator: `tools/sora_audio_tables.js ${VERSION}`, spec: 'docs/sora-audio-spec.md', events, static_se, ui_kinds, projectiles: Object.values(projectiles), sockets };
const text = JSON.stringify(out, null, 1) + '\n';
const file = path.join(R, 'build', 'sora_pack', 'audio', 'tables.json');
if (process.argv.includes('--check')) {
  const same = fs.existsSync(file) && fs.readFileSync(file, 'utf8') === text;
  console.log(same ? 'sora_audio_tables: file on disk matches' : 'sora_audio_tables: file on disk DIFFERS');
  process.exitCode = same ? 0 : 1;
} else {
  fs.writeFileSync(file, text);
  console.log(`sora_audio_tables ${VERSION}: ${events.length} events, ${static_se.length} static SE (${static_se.filter((r) => r.packed).length} packed), ${ui_kinds.length} UI kinds, ${Object.keys(projectiles).length} projectile classes (${Object.values(projectiles).filter((e) => e.fire_se).length} fire SE, ${Object.values(projectiles).filter((e) => e.pawn_hit_se).length} pawn hit SE, ${Object.values(projectiles).filter((e) => e.map_hit_se).length} map hit SE), ${Object.keys(sockets).length} sockets -> build/sora_pack/audio/tables.json`);
}
