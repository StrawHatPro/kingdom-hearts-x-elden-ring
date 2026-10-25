// (party stream D6b) The shots of Sora's team finishers -> sheets/port/sora-combat-shots.json (one row per attack
// shape of the shot's actor), generated into build/gen/rust/sora_combat_shots.rs (crates/kh-core combat/hit.rs reads it).
//
// Source [A]: Blueprints/Player/p_ex001/p_ex001_ProjectileSet (ProjectileName -> class), each class's blueprint and its
// parents up to the native class (the CDO's template chain): CDO (m_MainLife, m_bNoDestroyOnHit, MyFireEffects), the
// components TresAtkColl0 (CollisionShapesSrc, m_HitEffects, m_fHitEffectRandomOffsetRadius, m_CollAutoActivate) and
// ParticleComp (Template, RelativeLocation); a child's property replaces the parent's of the same name.
// The spawn rule is code, not data [C]: attack-state slot 137 0x14134CCE0 (UNISON_*: Sora's root + 100 z, his root
// rotation) and slot 138 0x14134C710 (FUSION_SPIN: Sora's root, Goofy's root rotation, attached to Goofy's mesh keeping
// the world transform, FUN_143E8FC60 with rules DAT_148750ED0).
//
//   node tools/sora_combat_shots_sheet.js        writes the sheet
const fs = require('fs');
const path = require('path');
const { ROOT, writeSheet } = require('./lib/sheets');

const CONTENT = path.join(ROOT, 'assets_converted/packages/KINGDOM HEARTS III/Content');
const SHOTS = {
  UNISON_FIRE: { spawn: 'ROOT_UP_100', ref: '0x14134CCE0' },
  UNISON_BLIZZARD: { spawn: 'ROOT_UP_100', ref: '0x14134CCE0' },
  UNISON_THUNDER: { spawn: 'ROOT_UP_100', ref: '0x14134CCE0' },
  FUSION_SPIN: { spawn: 'ROOT_ATTACHED_GOOFY', ref: '0x14134C710' },
};

const load = (pkg) => JSON.parse(fs.readFileSync(path.join(CONTENT, pkg.replace(/^\/Game\//, '') + '.uasset.json'), 'utf8'));
const objRef = (j, v) => (typeof v === 'number' && v < 0 ? j.imports[-v - 1] : typeof v === 'number' && v > 0 ? j.exports[v - 1] : null);
const importPackage = (j, imp) => { let o = imp; while (o && o.outerIndex) o = j.imports[-o.outerIndex - 1]; return o && o.objectName; };

// Property list -> plain object (object references by name; nested structs and arrays decoded).
function plain(j, props) {
  const o = {};
  for (const p of props || []) {
    let v = p.value;
    if (v && v.items) v = v.items.map((it) => (Array.isArray(it) ? plain(j, it) : it));
    else if (Array.isArray(v) && v.length && v[0] && v[0].name) v = plain(j, v);
    else if (p.type === 'ObjectProperty') { const r = objRef(j, v); v = r ? { name: r.objectName, pkg: r.className === 'Package' ? r.objectName : importPackage(j, r) } : null; }
    o[p.name + (p.arrayIndex ? `[${p.arrayIndex}]` : '')] = v;
  }
  return o;
}

// The class chain, child first: [{ pkg, cdo, comps: { name: props } }].
function chain(pkg) {
  const out = [];
  for (let guard = 0; pkg && pkg.startsWith('/Game/') && guard < 8; guard++) {
    const j = load(pkg);
    const cdo = j.exports.find((e) => e.objectName.startsWith('Default__'));
    const comps = {};
    for (const e of j.exports) if (e.outerIndex === j.exports.indexOf(cdo) + 1 && e.data) comps[e.objectName] = plain(j, e.data.props);
    out.push({ pkg, cdo: plain(j, cdo.data.props), comps });
    const tmpl = cdo.templateIndex < 0 ? j.imports[-cdo.templateIndex - 1] : null;
    pkg = tmpl ? importPackage(j, tmpl) : null;
  }
  return out;
}
const merged = (links, pick) => Object.assign({}, ...links.map(pick).filter(Boolean).reverse());

const vec = (v) => (v ? [v.x, v.y, v.z] : [0, 0, 0]);
const enumTail = (v) => String(v || '').split('::').pop();

function curveKeys(ref) {
  if (!ref || !ref.pkg) return [];
  const j = load(ref.pkg);
  const e = j.exports.find((x) => x.objectName === ref.name);
  const fc = e.data.props.find((p) => p.name === 'FloatCurves' && !p.arrayIndex);
  const keys = fc.value.find((p) => p.name === 'Keys').value.items;
  if (keys.some((k) => k.interpMode !== 0)) throw new Error(`${ref.name}: a key is not linear`);
  return keys.flatMap((k) => [k.time, k.value]);
}

const set = load('/Game/Blueprints/Player/p_ex001/p_ex001_ProjectileSet');
const setRows = set.exports[0].data.props.find((p) => p.name === 'Projectiles').value.items;
const rows = [];
for (const [shot, rule] of Object.entries(SHOTS)) {
  const unit = setRows.find((it) => it.find((p) => p.name === 'ProjectileName').value === shot);
  const cls = objRef(set, unit.find((p) => p.name === 'ProjectileData').value);
  const pkg = importPackage(set, cls);
  const links = chain(pkg);
  const cdo = merged(links, (l) => l.cdo);
  const atk = merged(links, (l) => l.comps.TresAtkColl0);
  const part = merged(links, (l) => l.comps.ParticleComp);
  const fire = (cdo.MyFireEffects || [])[0] || {};
  const auto = atk.m_CollAutoActivate || [];
  (atk.CollisionShapesSrc || []).forEach((s, k) => {
    const effGroup = s.m_EffectGrpName || 'None';
    const hit = (atk.m_HitEffects || []).find((h) => (h.m_GrpName || 'None') === effGroup) || {};
    const act = auto.find((a) => (a.m_GrpName || 'None') === (s.m_GrpName || 'None')) || {};
    const source = `[A] ${links.map((l) => l.pkg.split('/').pop()).join(' < ')}: TresAtkColl0.CollisionShapesSrc[${k}], m_HitEffects (group ${effGroup}), m_CollAutoActivate, ParticleComp, CDO`;
    rows.push([
      `${shot}#${k}`, shot, cls.objectName, s.m_GrpName || 'None', s.m_DefaultAttackDataIDName, enumTail(s.m_ShapeType), s.m_Mesh ? s.m_Mesh.name : '',
      cdo.m_MainLife, cdo.m_bNoDestroyOnHit ? 'yes' : 'no',
      vec(s.m_Size), vec(s.m_IncSize), vec(s.m_IncMaxSize), curveKeys(s.m_SizeVectorCurve), vec(s.m_RelativeLocation1),
      vec(s.m_Scale), vec(s.m_IncScale), vec(s.m_IncMaxScale), act.m_AttackInterval || 0, enumTail(s.m_MapHitType),
      part.Template ? part.Template.name : '', vec(part.RelativeLocation),
      hit.m_PawnHitEffect ? hit.m_PawnHitEffect.name : '', hit.m_PawnHitSEAsset ? hit.m_PawnHitSEAsset.name : '', atk.m_fHitEffectRandomOffsetRadius || 0,
      fire.MyParticleSystem ? fire.MyParticleSystem.name : '', fire.MySoundCue ? fire.MySoundCue.name : '',
      rule.spawn, rule.ref, source,
    ]);
  });
}
const columns = ['id', 'shot', 'class', 'group', 'row', 'shape', 'mesh', 'life', 'no_destroy', 'size', 'inc_size', 'max_size', 'size_curve', 'loc', 'scale', 'inc_scale', 'max_scale', 'interval', 'map_hit',
  'particle', 'particle_loc', 'hit_effect', 'hit_se', 'hit_offset', 'fire_effect', 'fire_se', 'spawn', 'ref_addr', 'source'];
const n = writeSheet(path.join(ROOT, 'sheets/port/sora-combat-shots.json'), 'sora-combat-shots', 'port', columns, rows);
console.log(`sora-combat-shots: ${n} rows`);
