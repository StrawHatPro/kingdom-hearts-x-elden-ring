#!/usr/bin/env node
// tools/sora_savepoint_pack.js: KH3's save point (Blueprints/Gimmick/ex/g_ex_SavePoint, native ATresSavePointActor)
// into the Sora pack. docs/sora-savepoint-spec.md is the text.
//
// The save point has no mesh, no light component, no animation and no sound asset of its own: it IS five particle
// systems (base ring, body, their "limited" twins, the heal effect), two capsules and a rule. So the export is:
//   1. the five systems join the effect list of the effect pack (build/sora_tmp/vfx/fxinv/effect_list.json, category
//      "savepoint", for the next full build) and tools/sora_vfx_pack.js exports the five alone into
//      build/sora_tmp/savepoint/vfx_full; the files the pack does not have yet are copied in (nothing it has is touched);
//   2. their rows go into sheets/port/sora-assets.json (and vfx/sheet_rows.json); the shaders of the new materials
//      for the test app are a separate run of tools/sora_vfx_shaders.js;
//   3. build/sora_pack/savepoint/savepoint.json: the actor's data (effects by role, the two capsules, the recover
//      rule's numbers) read from the blueprint and from the spec's constants, + its row in sheets/port/sora-assets.json.
// The Elden Ring shader set (PACK/vfx/er, `cargo run --release -p kh-shaderc -- --vfx`) is NOT rebuilt here.
//
//   node tools/sora_savepoint_pack.js            do it
//   node tools/sora_savepoint_pack.js --data     only step 3
// Then: node tools/extract.js; node tools/preflight.js --summary
'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { spawnSync } = require('child_process');
const ROOT = path.resolve(__dirname, '..');
const PACK = path.join(ROOT, 'build', 'sora_pack');
const VFX = path.join(PACK, 'vfx');
const LIST = path.join(ROOT, 'build', 'sora_tmp', 'vfx', 'fxinv', 'effect_list.json');
const TRIAL = path.join(ROOT, 'build', 'sora_tmp', 'savepoint', 'vfx_full');
const BP_REL = 'KINGDOM HEARTS III/Content/Blueprints/Gimmick/ex/g_ex_SavePoint';
const BP = path.join(ROOT, 'assets_converted', 'packages', BP_REL + '.uasset.json');
const SHEET = path.join(ROOT, 'sheets', 'port', 'sora-assets.json');
const sha = (b) => crypto.createHash('sha256').update(b).digest('hex');
const run = (args) => { const r = spawnSync(process.execPath, args, { cwd: ROOT, stdio: ['ignore', 'pipe', 'inherit'], encoding: 'utf8', maxBuffer: 1 << 28 }); if (r.status !== 0) throw new Error('failed: node ' + args.join(' ')); return r.stdout; };

// ---- the blueprint
const bp = JSON.parse(fs.readFileSync(BP, 'utf8'));
const name = (e) => e.objectName || e.name;
const props = (e) => (e.data && e.data.props) || e.props || [];
const ref = (i) => (i < 0 ? name(bp.imports[-i - 1]) : i > 0 ? name(bp.exports[i - 1]) : null);
const cdo = bp.exports.find((e) => /^Default__/.test(name(e)));
const roles = {};
for (const p of props(cdo)) if (/Effect$/.test(p.name) && p.type === 'ObjectProperty') roles[p.name] = ref(p.value);
const capsule = (exportName) => {
  const e = bp.exports.find((x) => name(x) === exportName); const P = Object.fromEntries(props(e).map((p) => [p.name, p]));
  const unit = Object.fromEntries(P.m_CollisionShapesSrc.value.items[0].map((x) => [x.name, x.value]));
  return { shape: unit.ShapeType, radius: unit.Size.x, height: unit.Size.z, offset: [P.RelativeLocation.value.x, P.RelativeLocation.value.y, P.RelativeLocation.value.z] };
};
const data = {
  format: 'kh3-sora-savepoint/1',
  source: '/Game/Blueprints/Gimmick/ex/g_ex_SavePoint (g_ex_SavePoint_C : ATresSavePointActor : ATresPlayerStart)',
  units: 'centimetres, seconds; offsets relative to the actor origin (the capsule root), Z up',
  effects: {
    base: roles.m_BaseEffect, body: roles.m_BodyEffect, limit_base: roles.m_LimitBaseEffect, limit_body: roles.m_LimitBodyEffect, heal: roles.m_HealEffect,
    note: 'base + body are shown while the save point is active and in view; the limit_ twins replace them for a limited save point (m_bLimitedSavePoint); heal is spawned at the actor root while a pawn that needs recovery stands in the recover capsule, and for first_access_time after the first entry (0x140F9CA00, 0x140F8E910)',
  },
  reactor: Object.assign(capsule('TresReactor0'), { note: 'TresReactorComponent: inside it the Save command (ETresCommandKind TRES_CMD_KIND_SAVE 76, reactor command TRES_RCID_SAVE) is offered' }),
  recover: Object.assign(capsule('TresRecover0'), { note: 'TresSavePointRecoverComponent: inside it HP, MP and focus recover' }),
  recover_rate: { value: 180, unit: 'points per second each of HP, MP and focus', source: '0x140F93590: accumulator += dt x 180; every whole point is given through Recover(n, n, n, kind 1) to the pawn and its party members; kind 1 = while the MP gauge recharges the recharge timer loses n seconds instead' },
  recover_condition: 'only while the pawn needs it (player virtual +0x1880 = 0x141436260: not dying; HP below max, or MP below max when the MP gauge is enabled, or focus below max when enabled, or a party member below max HP / MP)',
  first_access_time: { value: 3, unit: 's', source: 'ATresSavePointActor ctor 0x140F8B810 (+0x4E0 = 3.0); counted down in the tick 0x140F9CA00 while a pawn is checked' },
  pouch_refill: 'none of its own. The item pouch is refilled when the game leaves a pause (magic spec J 7.5, game-instance tick 0x140EAE5A0 -> RefillAll 0x141446500): opening the save menu pauses, closing it refills',
  menu: { movie: 'SaveLoad', clip: 'menuSavePoint', state: 'TresPlayerState_SaveMenu (state id 105)', restriction_table: '/Game/UI/Data/SaveMenuRestrictionData (per save point: area select, world map, restaurant sections by game progress)' },
  mesh: null, light: null, animation: null, sound: null,
};

function writeData() {
  const dir = path.join(PACK, 'savepoint'); fs.mkdirSync(dir, { recursive: true });
  const text = JSON.stringify(data, null, 1) + '\n';
  fs.writeFileSync(path.join(dir, 'savepoint.json'), text);
  // its row in the assets sheet (same columns as the rows tools/sora_pack.js writes)
  const raw = fs.readFileSync(SHEET, 'utf8');
  const sheet = JSON.parse(raw);
  const cooked = path.join(ROOT, 'assets', BP_REL + '.uasset');
  const srcSha = fs.existsSync(cooked) ? sha(fs.readFileSync(cooked)) : null;
  const row = ['savepoint/savepoint.json', 'data', '-', BP_REL + '.uasset', srcSha, Buffer.byteLength(text), sha(Buffer.from(text)), '-', '-', '-', '-', 'json read back', 'the save point actor: its five effects by role, the reactor and recover capsules, the recover rule (docs/sora-savepoint-spec.md); written by tools/sora_savepoint_pack.js'];
  if (row.length !== sheet.columns.length) throw new Error('sora-assets has ' + sheet.columns.length + ' columns, the row ' + row.length);
  const rows = sheet.rows.filter((r) => r[0] !== row[0]); rows.push(row);
  const head = raw.slice(0, raw.indexOf('"rows":[') + 8);
  fs.writeFileSync(SHEET, head + '\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
  console.log('savepoint/savepoint.json written (' + Buffer.byteLength(text) + ' bytes); sora-assets row set');
}

if (process.argv.includes('--data')) { writeData(); process.exit(0); }

// ---- 1. the effect list
const list = JSON.parse(fs.readFileSync(LIST, 'utf8'));
let added = 0;
for (const [role, effect] of Object.entries(data.effects)) {
  if (role === 'note') continue;
  const game = '/Game/Effects/level/ex/gimmick/par/' + effect + '.' + effect;
  if (list.some((r) => r.effect && r.effect.toLowerCase() === game.toLowerCase())) continue;
  const pkg = 'assets_converted/packages/KINGDOM HEARTS III/Content/Effects/level/ex/gimmick/par/' + effect + '.uasset.json';
  if (!fs.existsSync(path.join(ROOT, pkg))) throw new Error('no package ' + pkg);
  list.push({ effect: game, referenced_as: game, package: pkg, exists: true, class: 'ParticleSystem', priority: 'core', categories: ['savepoint'], used_by: ['savepoint.' + role] });
  added++;
}
if (added) fs.writeFileSync(LIST, JSON.stringify(list, null, 1) + '\n');
console.log('effect list: ' + added + ' save point effect(s) added (' + list.length + ' entries)');

// ---- the five systems alone, into a trial folder; ONLY FILES THE PACK DOES NOT HAVE are copied in.
// (A full rebuild of the pack is not done here: on 2026-10-12 the pack tool no longer reproduced a number of the
// material files the pack holds byte for byte (the trial of a full build listed them), and the pack is in use by the
// hosts. The effects are in the effect list, so the next full build carries them.)
const walk = (dir, rel = '') => fs.readdirSync(path.join(dir, rel), { withFileTypes: true }).flatMap((d) => (d.isDirectory() ? walk(dir, rel + d.name + '/') : [rel + d.name]));
fs.rmSync(TRIAL, { recursive: true, force: true });
const wanted = Object.entries(data.effects).filter(([role]) => role !== 'note').map(([, effect]) => '/Game/Effects/level/ex/gimmick/par/' + effect);
run(['tools/sora_vfx_pack.js', '--no-seed', '--out', TRIAL, ...wanted.flatMap((e) => ['--effect', e])]);
const TABLES = new Set(['manifest.json', 'validation.json', 'sheet_rows.json', 'triggers.json']);
const fresh = [], differing = [];
for (const f of walk(TRIAL)) {
  if (TABLES.has(f)) continue;
  const live = path.join(VFX, f);
  if (!fs.existsSync(live)) fresh.push(f);
  else if (!fs.readFileSync(live).equals(fs.readFileSync(path.join(TRIAL, f)))) differing.push(f);
}
for (const f of fresh) { fs.mkdirSync(path.dirname(path.join(VFX, f)), { recursive: true }); fs.copyFileSync(path.join(TRIAL, f), path.join(VFX, f)); }
console.log('pack: ' + fresh.length + ' new file(s) copied: ' + fresh.join(' '));
if (differing.length) console.log('NOTE: ' + differing.length + ' file(s) the save point shares with the pack differ from what the tool writes today (the files of the pack were kept): ' + differing.join(' '));
// their rows join vfx/sheet_rows.json (what tools/sora_vfx_assets_rows.js puts into the sheet)
const trialRows = JSON.parse(fs.readFileSync(path.join(TRIAL, 'sheet_rows.json'), 'utf8'));
const rowsFile = path.join(VFX, 'sheet_rows.json');
const liveRows = JSON.parse(fs.readFileSync(rowsFile, 'utf8'));
const rowList = (x) => (Array.isArray(x) ? x : x.rows);
const have = new Set(rowList(liveRows).map((r) => r[0]));
let rowsAdded = 0;
for (const r of rowList(trialRows)) { const rel = String(r[0]).slice(4); if (!have.has(r[0]) && !TABLES.has(rel) && fs.existsSync(path.join(VFX, rel))) { rowList(liveRows).push(r); rowsAdded++; } }
if (rowsAdded) fs.writeFileSync(rowsFile, JSON.stringify(liveRows));
console.log('vfx/sheet_rows.json: ' + rowsAdded + ' row(s) added');

// ---- 2. the rows of the new files in the assets sheet (tools/sora_vfx_assets_rows.js is NOT run: it would replace
// every vfx/ row by vfx/sheet_rows.json and drop the rows other tools added since). The shaders of the new materials
// for the test app: run  node tools/sora_vfx_shaders.js  (not run from here: it rewrites one file for all materials).
{
  const raw = fs.readFileSync(SHEET, 'utf8'); const sheet = JSON.parse(raw);
  const mine = rowList(trialRows).filter((r) => fresh.includes(String(r[0]).slice(4)));
  const names = new Set(mine.map((r) => r[0]));
  const rows = sheet.rows.filter((r) => !names.has(r[0])).concat(mine);
  const head = raw.slice(0, raw.indexOf('"rows":[') + 8);
  if (mine.length) fs.writeFileSync(SHEET, head + '\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
  console.log('sora-assets: ' + mine.length + ' vfx row(s) set');
}
// ---- 3.
writeData();
