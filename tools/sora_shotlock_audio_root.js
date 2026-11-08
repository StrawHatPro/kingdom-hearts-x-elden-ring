#!/usr/bin/env node
// (SETUP-TOOLS 2026-11-07) THE SHOTLOCKS' AUDIO ROOT (PACK/shotlocks/audio) IN ONE PACK STEP, as the development deploys made it:
//   1. tools/sora_shotlock_audio.js (FINAL-FIX): the classes of sora-shotlocks / sora-magic-projectiles the main audio table
//      lacks, with their SEs (tables.json, sounds.json, effects.json in WORK);
//   2. (HM-MAGIC, build/sora_tmp/weapons/HM-MAGIC/tools/audio_root.js) + the 20 TresProj_Mag*CL classes of p_ex001_hm_MagicSet
//      (sheets/port/sora-hm-magic.json `actor` rows: fire / pawn hit / map hit SE and their effect roles) and the SEs the main
//      pack lacks (se00002_406, AeroCL's fire SE);
//   3. tools/sora_audio_pack.js --party-extra over those lists -> WORK/audio, tables.json copied in, the root copied to PACK.
// The same files as the development tools (the logic of step 2 is audio_root.js's, reading WORK instead of the staged and live
// copies). Reads the main audio root (PACK/audio: run after the `audio` / `audio-tables` steps).
//
//   node tools/sora_shotlock_audio_root.js [--work DIR] [--out DIR]
//     --work  scratch folder (default build/packgen/shotlocks)
//     --out   the root (default build/sora_pack/shotlocks/audio; replaced as a whole)
'use strict';
const fs = require('fs');
const path = require('path');
const cp = require('child_process');

const ROOT = path.resolve(__dirname, '..');
const P = (...a) => path.join(ROOT, ...a);
const opt = (k, d) => { const i = process.argv.indexOf(k); return i > 0 ? path.resolve(process.argv[i + 1]) : d; };
const WORK = opt('--work', P('build', 'packgen', 'shotlocks'));
const OUT = opt('--out', P('build', 'sora_pack', 'shotlocks', 'audio'));
const node = (argv, env = {}) => {
  const r = cp.spawnSync(process.execPath, argv, { cwd: ROOT, env: { ...process.env, ...env }, encoding: 'utf8', maxBuffer: 1 << 28 });
  process.stdout.write(r.stdout || ''); process.stderr.write(r.stderr || '');
  if (r.status !== 0) throw new Error(`node ${argv.join(' ')} failed (exit ${r.status})`);
};

fs.rmSync(WORK, { recursive: true, force: true });
// ---- 1. FINAL-FIX
node([P('tools', 'sora_shotlock_audio.js'), '--out', WORK]);
// ---- 2. HM-MAGIC (audio_root.js)
const tables = JSON.parse(fs.readFileSync(path.join(WORK, 'tables.json'), 'utf8'));
const sounds = JSON.parse(fs.readFileSync(path.join(WORK, 'sounds.json'), 'utf8'));
const sheet = JSON.parse(fs.readFileSync(P('sheets/port/sora-hm-magic.json'), 'utf8'));
const col = (r, n) => r[sheet.columns.indexOf(n)];
const main = JSON.parse(fs.readFileSync(P('build/sora_pack/audio/tables.json'), 'utf8'));
const mainManifest = fs.readFileSync(P('build/sora_pack/audio/manifest.json'), 'utf8');
const known = new Set([...main.projectiles, ...tables.projectiles].map((r) => r.class));
const nn = (s) => (s && s !== '-' ? s : null);
let added = 0;
for (const r of sheet.rows.filter((r) => col(r, 'what') === 'actor')) {
  const cls = col(r, 'clip');
  if (known.has(cls)) continue;
  const [body, fire, hit, mapHit] = col(r, 'effects');
  const [fireSe, hitSe, mapSe] = col(r, 'sounds');
  const effects = {};
  if (nn(body)) effects.body = [body];
  if (nn(fire)) effects.fire = [fire];
  if (nn(hit)) effects.hit = [hit];
  if (nn(mapHit)) effects.map_hit = [mapHit];
  tables.projectiles.push({ class: cls, chain: [cls.replace(/_C$/, '')], parent: col(r, 'flags')[0], blueprint: '/Game/Blueprints/Projectile/Magic' + col(r, 'logical').replace(/^./, (c) => c.toUpperCase()) + '/' + cls.replace(/_C$/, ''), command: null, effects, fire_se: nn(fireSe), pawn_hit_se: nn(hitSe), map_hit_se: nn(mapSe) });
  added += 1;
  for (const [asset, what] of [[fireSe, 'fire (MyFireEffects[0].MySoundCue)'], [hitSe, 'pawn hit'], [mapSe, 'map hit']]) {
    if (!nn(asset)) continue;
    const base = asset.split('/').pop();
    if (mainManifest.includes(base + '"') || sounds.some((s) => s.asset === asset)) continue;
    sounds.push({ asset, trigger: cls + ' ' + what, spatial: '3D', ref: '/Game/Blueprints/Projectile/' + cls.replace(/_C$/, ''), note: '(HM-MAGIC) a CL class SE the main pack lacks (build/sora_tmp/weapons/HM-MAGIC/tools/audio_root.js)' });
  }
}
tables.note = (tables.note || '') + ' (HM-MAGIC) + the 20 TresProj_Mag*CL classes of p_ex001_hm_MagicSet (sheets/port/sora-hm-magic.json), with their effect roles.';
fs.writeFileSync(path.join(WORK, 'tables.json'), JSON.stringify(tables, null, 1) + '\n');
fs.writeFileSync(path.join(WORK, 'sounds.json'), JSON.stringify(sounds, null, 1) + '\n');
fs.writeFileSync(path.join(WORK, 'effects.json'), '[]\n');
console.log(`sora_shotlock_audio_root: CL classes added ${added}; sounds ${sounds.length}`);
// ---- 3. the root
node([P('tools', 'sora_audio_pack.js'), '--party-extra', '--out', path.join(WORK, 'audio')], { KH_AUDIO_EXTRA_EFFECTS: path.join(WORK, 'effects.json'), KH_AUDIO_EXTRA_SOUNDS: path.join(WORK, 'sounds.json') });
fs.copyFileSync(path.join(WORK, 'tables.json'), path.join(WORK, 'audio', 'tables.json'));
fs.rmSync(OUT, { recursive: true, force: true });
fs.cpSync(path.join(WORK, 'audio'), OUT, { recursive: true });
console.log(`sora_shotlock_audio_root: ${path.relative(ROOT, OUT)}`);
