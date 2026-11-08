// (FINAL-FIX) THE AUDIO ROOT OF THE PROJECTILE CLASSES THE MAIN AUDIO PACK DOES NOT KNOW (FINAL 9: `sound: no route for
// "TresProjTS_AvatarRush_C"` x6, TresProjTR_AvatarRush_C, TresProjFZ_Burst_C, se02006_032, se02008_036).
//
// kh-core's projectiles (magic/projectile.rs) play a hit by the class (`ACTOR_PAWN_HIT` / `ACTOR_MAP_HIT`, the class's
// m_HitEffects[0].m_PawnHitSEAsset / m_MapHitSEAsset) unless their row names a hit sound (`hit_sound`, played by name).
// The main pack's actor table (build/sora_pack/audio/tables.json, tools/sora_audio_tables.js) only has the classes of the
// VFX pack's triggers: the shotlocks' classes (sheets/port/sora-shotlocks.json `proj.*.class`) were never in it, and some of
// their SEs were never packed. This writes, for every class of sora-shotlocks / sora-magic-projectiles that the main table
// lacks, its row from ITS OWN BLUEPRINT (the same parser as tools/sora_shotlocks.js):
//   fire_se      MyFireEffects[0].MySoundCue
//   pawn_hit_se  TresAtkColl0.m_HitEffects[0].m_PawnHitSEAsset
//   map_hit_se   TresAtkColl0.m_HitEffects[0].m_MapHitSEAsset
// (null where KH3's blueprint has none: the hit is silent by KH3's data, "the class has no SE for this"). No effect roles:
// the hit / body effects are spawned by kh-core on the effect seam, which plays their own sounds.
//
//   node tools/sora_shotlock_audio.js [--out DIR]     DIR (default build/sora_tmp/weapons/FINAL-FIX/stage/shotlocks):
//     tables.json   the rows (kh-audio Pack::merge_party_root reads DIR/audio/tables.json)
//     sounds.json   the SE assets the main pack and its roots lack (KH_AUDIO_EXTRA_SOUNDS of sora_audio_pack.js)
//     effects.json  [] (KH_AUDIO_EXTRA_EFFECTS: no effect rows in this root)
// then:
//   KH_AUDIO_EXTRA_EFFECTS=DIR/effects.json KH_AUDIO_EXTRA_SOUNDS=DIR/sounds.json node tools/sora_audio_pack.js --party-extra --out DIR/audio
//   cp DIR/tables.json DIR/audio/   and copy DIR/audio -> build/sora_pack/shotlocks/audio (new files; kh-er fx.rs merges it)
'use strict';
const fs = require('fs');
const path = require('path');
const lib = require('./shotlocks/lib.js');
const { P, readJson } = lib;

const OUT_AT = process.argv.indexOf('--out');
const OUT = OUT_AT > 0 ? path.resolve(process.argv[OUT_AT + 1]) : P('build', 'sora_tmp', 'weapons', 'FINAL-FIX', 'stage', 'shotlocks');
const CONTENT = P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content');

const sheetRows = (name) => { const s = readJson(P('sheets', 'port', name + '.json')); const id = s.columns.indexOf('id'), v = s.columns.indexOf('value'); return s.rows.map((r) => [r[id], r[v]]); };
const classes = new Set();
for (const sheet of ['sora-shotlocks', 'sora-magic-projectiles']) for (const [id, v] of sheetRows(sheet)) if (/class$/.test(id) && typeof v === 'string' && /^TresProj.*_C$/.test(v)) classes.add(v);

// what the pack already knows: its actor classes and its sound assets (the main pack and the roots kh-er merges)
const tables = readJson(P('build', 'sora_pack', 'audio', 'tables.json'));
const party = fs.existsSync(P('build', 'sora_pack', 'audio', 'party_tables.json')) ? readJson(P('build', 'sora_pack', 'audio', 'party_tables.json')) : { projectiles: [] };
const known = new Set([...tables.projectiles, ...party.projectiles].map((p) => p.class.toLowerCase()));
const packed = new Set();
for (const root of [['audio'], ['xsi', 'audio'], ['keyblades', 'audio'], ['party', 'audio']]) {
  const f = P('build', 'sora_pack', ...root, 'manifest.json');
  if (fs.existsSync(f)) for (const k of Object.keys(readJson(f).sound_assets || {})) packed.add(k.toLowerCase());
}

// every blueprint package under Blueprints, by its file name
const where = new Map();
(function walk(dir) {
  for (const f of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, f.name);
    if (f.isDirectory()) walk(p);
    else if (f.name.endsWith('.uasset.json')) where.set(f.name.replace(/\.uasset\.json$/, '').toLowerCase(), '/Game/' + path.relative(CONTENT, p).split(path.sep).join('/').replace(/\.uasset\.json$/, ''));
  }
})(path.join(CONTENT, 'Blueprints'));

// "se02008_036" as the blueprint stores the object -> its full asset path (the package under Sound/)
const soundPaths = new Map();
(function walk(dir) {
  for (const f of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, f.name);
    if (f.isDirectory()) walk(p);
    else if (f.name.endsWith('.uasset.json')) soundPaths.set(f.name.replace(/\.uasset\.json$/, '').toLowerCase(), '/Game/' + path.relative(CONTENT, p).split(path.sep).join('/').replace(/\.uasset\.json$/, ''));
  }
})(path.join(CONTENT, 'Sound', 'SE'));
const full = (name) => {
  if (!name) return null;
  const s = String(name);
  if (s.startsWith('/Game/')) return s.split('.')[0];
  const p = soundPaths.get(s.toLowerCase());
  if (!p) throw new Error('no sound package named ' + s);
  return p;
};

const rows = [], sounds = [], missing = [];
for (const cls of [...classes].sort()) {
  if (known.has(cls.toLowerCase())) continue;
  const pkg = where.get(cls.replace(/_C$/, '').toLowerCase());
  if (!pkg) { missing.push(cls); continue; }
  const bp = lib.blueprint(pkg);
  const he = ((bp.comps.TresAtkColl0 || {}).m_HitEffects || [])[0] || {};
  const fire = (bp.cdo.MyFireEffects || [])[0] || {};
  const row = { class: cls, chain: [cls.replace(/_C$/, '')], parent: bp.parent, blueprint: pkg, command: null, effects: {}, fire_se: full(fire.MySoundCue), pawn_hit_se: full(he.m_PawnHitSEAsset), map_hit_se: full(he.m_MapHitSEAsset) };
  rows.push(row);
  for (const [what, a] of [['fire (MyFireEffects[0].MySoundCue)', row.fire_se], ['pawn hit (m_HitEffects[0].m_PawnHitSEAsset)', row.pawn_hit_se], ['map hit (m_HitEffects[0].m_MapHitSEAsset)', row.map_hit_se]]) {
    if (a && !packed.has(a.toLowerCase()) && !sounds.some((s) => s.asset === a)) sounds.push({ asset: a, trigger: `${cls} ${what}`, spatial: '3D', ref: pkg, note: '(FINAL-FIX) a projectile class SE the main pack lacks (tools/sora_shotlock_audio.js)' });
  }
}
if (missing.length) throw new Error('no blueprint for ' + missing.join(', '));
fs.mkdirSync(OUT, { recursive: true });
const write = (f, j) => fs.writeFileSync(path.join(OUT, f), JSON.stringify(j, null, 1) + '\n');
write('tables.json', { format: 'kh3-sora-audio-tables/1', generator: 'tools/sora_shotlock_audio.js 1.0.0', note: '(FINAL-FIX) the projectile classes of sora-shotlocks / sora-magic-projectiles the main tables.json lacks, from their blueprints', projectiles: rows });
write('sounds.json', sounds);
write('effects.json', []);
console.log(`sora_shotlock_audio: ${classes.size} classes in the sheets, ${rows.length} not in the main table (${rows.filter((r) => r.pawn_hit_se).length} pawn hit SE, ${rows.filter((r) => r.map_hit_se).length} map hit SE, ${rows.filter((r) => r.fire_se).length} fire SE), ${sounds.length} SE to pack -> ${path.relative(P(), OUT)}`);
