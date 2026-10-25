#!/usr/bin/env node
// tools/sora_vfx_host.js: the small host table the effect renderer of the test app needs on top
// of the effect pack (tools/sora_vfx_pack.js): build/sora_pack/vfx/host.json
//   sockets            SkeletalMeshSocket objects of Sora's skeleton (name -> bone + relative transform, UE cm / degrees)
//   weapon_sockets     the Kingdom Key's sockets relative to the weapon root (= socket R_wep_skt), from the weapon pack
//   camera_shakes      class defaults of the UCameraShake blueprints the pack's SQEX_CameraShake modules play
//   projectiles        projectile blueprint class -> { role -> [effect names] } from the trigger table
//                      (roles: body = Template of the actor's ParticleComp, fire = MyFireEffects, hit, map_hit, PutEffect)
// Asset data only [A]; deterministic. Usage: node tools/sora_vfx_host.js [--check]
'use strict';
const fs = require('fs'), path = require('path');
const root = path.resolve(__dirname, '..');
const out = path.join(root, 'build/sora_pack/vfx/host.json');
const skel = JSON.parse(fs.readFileSync(path.join(root, 'assets_converted/packages/KINGDOM HEARTS III/Content/Character/pc/p_ex001/mdl/p_ex001_0_Skeleton.uasset.json'), 'utf8'));
const prop = (e, n) => { const p = (e.data.props || []).find(p => p.name === n); return p ? p.value : undefined; };
const sockets = {};
for (const e of skel.exports) {
  if (e.class !== 'SkeletalMeshSocket') continue;
  const l = prop(e, 'RelativeLocation') || { x: 0, y: 0, z: 0 }, r = prop(e, 'RelativeRotation') || { pitch: 0, yaw: 0, roll: 0 }, s = prop(e, 'RelativeScale') || { x: 1, y: 1, z: 1 };
  sockets[prop(e, 'SocketName')] = { bone: prop(e, 'BoneName'), location: [l.x, l.y, l.z], rotation: [r.pitch, r.yaw, r.roll], scale: [s.x, s.y, s.z] };
}
const weapon = JSON.parse(fs.readFileSync(path.join(root, 'build/sora_pack/weapon/manifest.json'), 'utf8'));
const weapon_sockets = {};
for (const s of weapon.sockets.sockets) {
  // Component space of the weapon mesh at its reference pose = relative to the weapon root, which sits on R_wep_skt.
  const c = s.component_space_ue || s.component_ue || null;
  weapon_sockets[s.name] = { bone: s.bone, local_location: s.relative_location_ue_cm, local_rotation: [s.relative_rotation_ue_deg.pitch, s.relative_rotation_ue_deg.yaw, s.relative_rotation_ue_deg.roll], component: c };
}
const trig = JSON.parse(fs.readFileSync(path.join(root, 'build/sora_pack/vfx/triggers.json'), 'utf8')).rows;
const projectiles = {};
for (const r of trig) {
  if (r.trigger_kind !== 'projectile' || typeof r.effect !== 'string' || r.effect === '-') continue;
  const p = r.params || {};
  const role = p.role || r.id.split('.').slice(2).join('.');
  // A row of a class the spell re-spawns (ReSpawnProjectileClass, ResidueList) names that class in `blueprint`.
  const bp = p.blueprint; if (!bp) continue;
  const cls = bp.split('/').pop() + '_C';
  const entry = projectiles[cls] = projectiles[cls] || {};
  const name = r.effect.split('.').pop();
  const key = role.split('.').pop();
  (entry[key] = entry[key] || []).includes(name) || entry[key].push(name);
}
// The Water family's burn actors are not rows of the trigger table: the template of their ParticleComp is read from
// the blueprint itself (the body effect that follows the actor).
const BP = path.join(root, 'assets_converted/packages/KINGDOM HEARTS III/Content/Blueprints/Projectile/MagicWater');
if (fs.existsSync(BP)) for (const file of fs.readdirSync(BP).sort()) {
  const m = /^(TresProj_Wat\w*Burn)\.uasset\.json$/.exec(file); if (!m) continue;
  const j = JSON.parse(fs.readFileSync(path.join(BP, file), 'utf8'));
  for (const e of j.exports) {
    if (e.class !== 'ParticleSystemComponent') continue;
    const t = ((e.data && e.data.props) || []).find((p) => p.name === 'Template' && p.type === 'ObjectProperty' && p.value < 0); if (!t) continue;
    const entry = projectiles[m[1] + '_C'] = projectiles[m[1] + '_C'] || {};
    const name = j.imports[-t.value - 1].objectName;
    (entry.body = entry.body || []).includes(name) || entry.body.push(name);
  }
}
// The Blizzard ice trail (docs/sora-magic-spec.md 4): class defaults of TresSplineMeshBlizzard_C (a
// TresDecalMeshSplineActor) and the pack files of its meshes with their slot materials. The core announces every
// rail node as the effect "TresRailSlideActor"; the app lays one trail piece between consecutive nodes.
const actors = {};
{
  const file = path.join(root, 'assets_converted/packages/KINGDOM HEARTS III/Content/Blueprints/Projectile/MagicBlizzard/TresSplineMeshBlizzard.uasset.json');
  const man = JSON.parse(fs.readFileSync(path.join(root, 'build/sora_pack/vfx/manifest.json'), 'utf8'));
  if (fs.existsSync(file)) {
    const j = JSON.parse(fs.readFileSync(file, 'utf8'));
    const cdo = j.exports.find((e) => /^Default__/.test(e.objectName));
    const P = Object.fromEntries(((cdo && cdo.data.props) || []).map((p) => [p.name, p.value]));
    const meshOf = (ref) => {
      if (!(ref < 0)) return null;
      const name = j.imports[-ref - 1].objectName;
      const rec = (man.meshes || []).find((m) => m.file && m.asset && m.asset.split('.').pop() === name);
      if (!rec) return null;
      const stem = (x) => x.split('/').pop().replace(/\.[^.]+$/, '');
      const mat = rec.slots && rec.slots[0] && rec.slots[0].material ? (man.materials || []).find((m) => m.asset === rec.slots[0].material.split('.')[0]) : null;
      return { mesh: stem(rec.file), material: mat && mat.file ? stem(mat.file) : null, bounds_m: rec.bounds_m || null };
    };
    const list = (v) => (Array.isArray(v) ? v : v && v.items ? v.items : []);
    actors.TresRailSlideActor = {
      note: 'drawn as the ice trail of the Blizzard bolt (TresSplineMeshBlizzard_C); the rail node itself has no mesh',
      segments: list(P.m_SplineMeshArray).map(meshOf).filter(Boolean),
      start: meshOf(P.m_SplineMeshPointStart), end: meshOf(P.m_SplineMeshPointEnd), start_end: meshOf(P.m_SplineMeshPointStartEnd),
      scale: [P.m_SplineMeshScale ? P.m_SplineMeshScale.x : 1, P.m_SplineMeshScale ? P.m_SplineMeshScale.y : 1],
      scale_random_min: [P.m_SplineMeshScaleRandOffsetMin ? P.m_SplineMeshScaleRandOffsetMin.x : 0, P.m_SplineMeshScaleRandOffsetMin ? P.m_SplineMeshScaleRandOffsetMin.y : 0],
      scale_random_max: [P.m_SplineMeshScaleRandOffsetMax ? P.m_SplineMeshScaleRandOffsetMax.x : 0, P.m_SplineMeshScaleRandOffsetMax ? P.m_SplineMeshScaleRandOffsetMax.y : 0],
      offset: P.m_Offset ?? 0, mesh_roll: !!P.m_EnableMeshRoll, mesh_fade_out_wait: P.m_MeshFadeOutWait ?? 0,
    };
  }
}
// Camera shakes: the blueprint classes named by the SQEX_CameraShake modules of the pack's effects, with the class
// defaults of stock UCameraShake (oscillators: Amplitude, Frequency, InitialOffset; cm / degrees / seconds).
const camera_shakes = {};
{
  const dir = path.join(root, 'build/sora_pack/vfx/effects');
  const classes = new Set();
  for (const file of fs.readdirSync(dir).sort()) {
    const text = fs.readFileSync(path.join(dir, file), 'utf8'); if (!text.includes('SQEX_CameraShake')) continue;
    for (const em of JSON.parse(text).emitters) for (const m of em.modules) if (/SQEX_CameraShake$/.test(m.class) && m.params && m.params.Shake) classes.add(m.params.Shake);
  }
  const osc = (list, name) => { const o = (list || []).find((x) => x.name === name); const g = (n, d) => { const p = o && (o.value || []).find((x) => x.name === n); return p ? p.value : d; }; return { amplitude: g('Amplitude', 0), frequency: g('Frequency', 0), start_at_zero: g('InitialOffset', 'EOO_OffsetRandom') === 'EOO_OffsetZero' }; };
  for (const cls of [...classes].sort()) {
    const file = path.join(root, 'assets_converted/packages/KINGDOM HEARTS III/Content', cls.replace('/Game/', '').split('.')[0] + '.uasset.json');
    if (!fs.existsSync(file)) continue;
    const j = JSON.parse(fs.readFileSync(file, 'utf8'));
    const cdo = j.exports.find((e) => /^Default__/.test(e.objectName)); if (!cdo) continue;
    const P = Object.fromEntries(((cdo.data && cdo.data.props) || []).map((p) => [p.name, p.value]));
    const fov = { name: 'F', value: P.FOVOscillation || [] };
    camera_shakes[cls.split('.').pop()] = {
      asset: cls, duration: P.OscillationDuration ?? 0, blend_in: P.OscillationBlendInTime ?? 0.1, blend_out: P.OscillationBlendOutTime ?? 0.2, single_instance: !!P.bSingleInstance,
      rot: ['Pitch', 'Yaw', 'Roll'].map((n) => osc(P.RotOscillation, n)), loc: ['X', 'Y', 'Z'].map((n) => osc(P.LocOscillation, n)), fov: osc([fov], 'F'),
    };
  }
}
const doc = { format: 'kh3-sora-vfx-host/1', generator: 'tools/sora_vfx_host.js 1.3.0', actors, camera_shakes, note: 'host table of the effect renderer (crates/kh-app/src/vfx.rs): sockets, weapon sockets, projectile class -> effects', sockets, weapon_sockets, projectiles };
const text = JSON.stringify(doc, null, 1) + '\n';
if (process.argv.includes('--check')) { const same = fs.existsSync(out) && fs.readFileSync(out, 'utf8') === text; console.log(same ? 'host.json: up to date' : 'host.json: DIFFERS'); process.exit(same ? 0 : 1); }
fs.writeFileSync(out, text);
console.log(`host.json: ${Object.keys(sockets).length} sockets, ${Object.keys(weapon_sockets).length} weapon sockets, ${Object.keys(projectiles).length} projectile classes, ${Object.keys(camera_shakes).length} camera shakes`);
