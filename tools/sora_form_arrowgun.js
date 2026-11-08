// (weapons F-ARROWGUN) KH3's gun formchanges as DATA for their form package (kh-core combat/forms/arrowgun.rs,
// states/combo_dw.rs, states/combo_bz.rs): Double Arrowguns (form.DW, CHANGE_DW 0x80) and Magic Launcher (form.BZ, CHANGE_BZ
// 0x83) of Shooting Star, Hunny Blasters (form.DW2, 0x8E) and Hunny Launcher (form.BZ2, 0x8F) of Hunny Spout. One attack class
// per stage (pawn slot 1337 0x141432F00: 0x80 / 0x8E -> COMBO_DW 75, 0x83 / 0x8F -> COMBO_BZ 78), one finish command
// (FINISH_BZ 0x9D for both second stages), the gun projectiles of the two projectile sets (w_so020_ProjSet / w_so040_ProjSet).
// documentation/KH3_WEAPONS_PLAN.md 1.3 / 4 (package F-ARROWGUN), build/sora_tmp/weapons/F-ARROWGUN/STATE.md.
//
//   node tools/sora_form_arrowgun.js   write sheets/port/sora-form-arrowgun.json and register it (once, in place) in
//                                      tools/lib/sheet_defs.js and tools/lib/port_gen_defs.js; then node tools/gen_port.js
//
// Inputs (read only): sheets/port/sora-keyblade-forms.json (P3: the form rows), build/sora_pack/keyblades/
// sora_forms_manifest.json (the form anim sets: logical rows, clips, notifies, blend spaces), build/sora_pack/manifest.json
// (the base set), the projectile blueprints (assets_converted .../Blueprints/Projectile/{DW,BZ,ShootFlow}/*: the class
// defaults of TresProjectileBase and its components ProjectileComp / TresAtkColl0 / ParticleComp / TresRoot), the form
// weapons w_so024_Wep / w_so044_Wep (TresAtkColl0 CollisionShapesSrc), p_ex001_AttackData, and THE CODE TABLES BELOW, read
// in the decompiled exe (re/decomp):
//   DECIDE   TresPlayerState_ComboDW::Decide 0x141361270, TresPlayerState_ComboBZ::Decide 0x1413602A0 (vtables 0x14690DEB8 /
//            0x14690BF30 slot 122; the only main-vtable override besides the Reset / Begin / Tick / End thunks): the Rust.
//   PLAY     TresPlayerState_Attack::PlayAttack 0x1413378B0 cases 0x44 .. 0x48, 0x4A .. 0x4E: `PLAY` below.
//   SHOT     the anim-interface shot handler (vtable +0x228 = slot 69): ComboDW 0x1413582D0 (anim vtable 0x14690E330),
//            ComboBZ 0x141357FA0 (0x14690C3A8): the projectile name per kind, the hands, the ring, the fan: `PLAY`.
//   MAX      MaxCombo 0x141433570: 5 in 0x80 / 0x8E, 3 in 0x83 / 0x8F; Finishers 0x141433A60: 2 in all four.
//   FINISH   FINISH_BZ (0x9D): StyleFinish Begin 0x1413CBF70 delegate vt+0x478 = slot 143 0x1413DF100 (Play M_5417, sub 5,
//            DoChain at its end); the shot: StyleFinish anim-interface 0x1413DD180 (BZ_BIG_BANG at weapon 0's muzzle,
//            pitch 30, homing to the point 1000 cm ahead of Sora's centre; WorldSlow off).
//   NATIVE   UTresProjectileMovementComponent ctor 0x141470D50: InitialSpeed = MaxSpeed = 2000 when the blueprint sets none.
'use strict';
const fs = require('fs');
const path = require('path');
const ROOT = path.resolve(__dirname, '..');
const VERSION = '1.0.0';
const P = (...a) => path.join(ROOT, ...a);
const readJson = (f) => JSON.parse(fs.readFileSync(f, 'utf8'));
const must = (c, m) => { if (!c) throw new Error(m); };
const r4 = (x) => Math.round(x * 10000) / 10000;
const flagValue = (flags, key) => { const x = flags.find((y) => y.startsWith(key + ':')); return x ? x.slice(key.length + 1) : null; };

// ------------------------------------------------------------------------------------------------ inputs
const sheet = (name) => { const s = readJson(P('sheets', 'port', name + '.json')); return s.rows.map((r) => Object.fromEntries(s.columns.map((c, i) => [c, r[i]]))); };
const FORMS = sheet('sora-keyblade-forms');
const FORM = {};
for (const f of ['DW', 'BZ', 'DW2', 'BZ2']) { FORM[f] = FORMS.find((r) => r.id === 'form.' + f); must(FORM[f], 'sora-keyblade-forms has no form.' + f); }
const OV = readJson(P('build', 'sora_pack', 'keyblades', 'sora_forms_manifest.json'));
const MAIN = readJson(P('build', 'sora_pack', 'manifest.json'));
const ATK = readJson(P('sheets', 'entities', 'DataTable', 'Player', 'Attack', 'p_ex001_AttackData.json')).rows;
const CONTENT = P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content');
const setName = (a) => String(a).split('/').pop().split('.')[0];
const ITEMS = new Map([...MAIN.items, ...OV.items].filter((i) => i.kind === 'animation').map((i) => [i.animation_name, i]));
const BASE = new Map(MAIN.logical_animations.map((a) => [a.name, a]));

// logical name -> clip of a form: its sets' rows, the LAST set over the earlier ones (AnimSet16 over AnimSet2 for Hunny
// Blasters: the m_ReplaceAnimSets order), a blend space of a set by its sample at axis value 0 (the straight-ahead aim: the
// form's MagicFire = MagicFire2 / MagicFire5 ... are BlendSpace1D over pitch -45 .. 45; INFERRED: the port plays no aim
// blend for them), else the base set's row (none: the clip is unchanged).
function blendSample(name) {
  const b = OV.blend_spaces[name];
  if (!b) return null;
  const s = (b.samples || []).find((x) => x.value === 0) || (b.samples || [])[0];
  return s ? setName(s.asset) : null;
}
function rowsOf(form) {
  const out = new Map();
  for (const set of FORM[form].anim_sets.map(setName)) {
    for (const a of OV.logical_animations.filter((x) => setName(x.anim_set) === set)) {
      const clip = a.file ? a.clip : blendSample(a.clip);
      if (clip && ITEMS.has(clip)) out.set(a.name, { set, clip });
    }
  }
  return out;
}
const ROWS_OF = Object.fromEntries(Object.keys(FORM).map((f) => [f, rowsOf(f)]));
const item = (clip) => ITEMS.get(clip) || null;
const prop = (n, key) => { const p = n.properties || {}; const v = p[key]; return v && typeof v === 'object' && 'value' in v ? v.value : v; };
function notes(clip) {
  const it = item(clip);
  const out = { length: it ? r4(it.duration_s) : 0, shots: [], windows: [], rows: [], change: -1, input: -1 };
  if (!it) return out;
  for (const n of it.notifies || []) {
    if (n.class === 'TresAnimNotify_Shot') out.shots.push(r4(n.time) + '@' + (prop(n, 'LocationIndex') === undefined ? 0 : prop(n, 'LocationIndex')));
    if (n.class === 'TresAnimNotifyState_Attack') { out.windows.push(r4(n.time) + '-' + r4(n.time + (n.duration || 0))); out.rows.push(String(prop(n, 'm_AttackDataIDName')) + '@' + String(prop(n, 'CollisionGrpName'))); }
    if (n.class === 'TresAnimNotify_EnableChange' && out.change < 0) out.change = r4(n.time);
    if (n.class === 'TresAnimNotify_EnableInput' && out.input < 0) out.input = r4(n.time);
  }
  return out;
}

// ------------------------------------------------------------------------------------------------ blueprints
const load = (pkg) => readJson(path.join(CONTENT, pkg.replace(/^\/Game\//, '').split('.')[0] + '.uasset.json'));
const objRef = (j, v) => (typeof v === 'number' && v < 0 ? j.imports[-v - 1] : typeof v === 'number' && v > 0 ? j.exports[v - 1] : null);
const importPackage = (j, imp) => { let o = imp; while (o && o.outerIndex) o = j.imports[-o.outerIndex - 1]; return o && o.objectName; };
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
// The class defaults and components of one blueprint (all four projectile families derive TresProjectileBase directly).
function blueprint(pkg) {
  const j = load(pkg);
  const cdo = j.exports.find((e) => e.objectName.startsWith('Default__'));
  must(cdo, pkg + ': no class default object');
  const comps = {};
  for (const e of j.exports) if (e.outerIndex === j.exports.indexOf(cdo) + 1 && e.data) comps[e.objectName] = plain(j, e.data.props);
  const tmpl = cdo.templateIndex < 0 ? j.imports[-cdo.templateIndex - 1] : null;
  return { cdo: plain(j, cdo.data.props), comps, parent: tmpl ? importPackage(j, tmpl) : null };
}
const tail = (v) => String(v || '').split('::').pop();
const MAP_HIT = { IGNORE_GROUND: 0, HITMAP: 1, NOHIT: 2 };

// One projectile class -> row (values order: see the note of the rows).
const PROJ = new Map();
function addProj(cls, setPkg) {
  const key = cls.replace(/_C$/, '');
  if (PROJ.has(key)) return key;
  PROJ.set(key, setPkg);
  const dir = /SS_|PO_/.test(key) ? (/(Normal|Curve|Spin|Spiral|Gatling)$/.test(key) ? 'DW' : /(Shoot|Barrage|Cannon2?|CannonEx)$/.test(key) ? 'ShootFlow' : 'BZ') : '';
  const pkg = '/Game/Blueprints/Projectile/' + (dir ? dir + '/' : '') + key;
  const bp = blueprint(pkg);
  must(bp.parent === '/Script/TresGame', key + ': parent ' + bp.parent + ' (expected the native TresProjectileBase)');
  const c = bp.cdo, mv = bp.comps.ProjectileComp || {}, coll = (bp.comps.TresAtkColl0 || {}), pc = bp.comps.ParticleComp || {}, root = bp.comps.TresRoot || {};
  const shape = (coll.CollisionShapesSrc || [])[0] || {};
  const he = (coll.m_HitEffects || [])[0] || {};
  const rootShape = (root.CollisionShapesSrc || [])[0] || {};
  const fire = (c.MyFireEffects || [])[0] || {};
  const respawn = c.m_ReSpawnProjectileClass ? String(c.m_ReSpawnProjectileClass).replace(/_C$/, '') : '-';
  // native defaults (the TresProjectileBase / movement constructors): speeds 2000 (0x141470D50), no homing, no spin.
  const speed = mv.InitialSpeed !== undefined ? mv.InitialSpeed : 2000;
  const max = mv.MaxSpeed !== undefined ? mv.MaxSpeed : 2000;
  const size = shape.m_Size || { x: 0, z: 0 }, inc = shape.m_IncSize || { x: 0 }, incMax = shape.m_IncMaxSize || { x: 0 }, loc = shape.m_RelativeLocation1 || { x: 0, y: 0, z: 0 };
  const spin = mv.m_Spin || { pitch: 0, yaw: 0, roll: 0 };
  const values = [c.m_MainLife || 0, c.m_MaxLifeDistance || 0, speed, max, mv.m_bEnableHoming ? 1 : 0, mv.m_HomingStartDelay || 0, mv.m_HomingMinDist || 0, mv.m_HomingTurnMax || 0, mv.m_HomingTurnAccel || 0, mv.m_HomingMaxTurnMax || 0,
    size.x || 0, inc.x || 0, incMax.x || 0, loc.x || 0, c.m_bExplodeOnTimeOut ? 1 : 0, c.m_bReSpawnProjectileOnTimeOut ? 1 : 0, c.m_bReSpawnTakeOverAtkCollHitList ? 1 : 0, c.m_bReSpawnTakeOverAtkCollFinishFlag ? 1 : 0, c.m_bNoDestroyOnHit ? 1 : 0,
    mv.m_bEnableSpin ? 1 : 0, spin.yaw || 0, mv.m_SpinMaxTime || 0, (rootShape.Size && rootShape.Size.x) || 0, mv.m_Accel || 0, mv.m_fMoveStartDelay || 0, MAP_HIT[tail(shape.m_MapHitType)] === undefined ? 0 : MAP_HIT[tail(shape.m_MapHitType)]].map(r4);
  const name = (x) => (x ? String(x) : '-');
  add({ id: 'proj.' + key, what: 'proj', form: /PO_/.test(key) ? 'PO' : 'SS', logical: key, clip: name(pc.Template), flags: [String(shape.m_DefaultAttackDataIDName || 'None'), name(fire.MyParticleSystem), name(he.m_PawnHitEffect), name(he.m_PawnHitSEAsset), respawn, name(he.m_MapHitEffect), tail(shape.m_ShapeType) || 'SPHERE'], values,
    ref_addr: '0x141470D50', ref_conf: 'confirmed', note: pkg + ' (in ' + setPkg + '): flags = [attack row, fire effect, pawn hit effect, pawn hit SE, respawn class, map hit effect, shape]; values = [life s, max distance, initial speed, max speed, homing, homing delay, homing min dist, turn max, turn accel, max turn max, size, inc size /s, inc max size, offset x, explode on time out, respawn on time out, take over hit list, take over finish, no destroy on hit, spin, spin yaw /tick, spin max time, root radius, accel, move start delay, map hit (0 IGNORE_GROUND, 1 HITMAP, 2 NOHIT)]' });
  if (respawn !== '-') addProj(respawn, setPkg);
  return key;
}

// ------------------------------------------------------------------------------------------------ the code tables
// PlayAttack 0x1413378B0 cases (+0x14C z gain, +0x1D8 root length, +0x1D4 DirZ with +0x1DD, +0x188, fVar28 = the root-scale
// divisor; +0x1F4 |= 0x10 keep_face; +0x1DF = 1 air mode; +0x234 & ~1 no XY channel; |2 the root channel; +0x23C |= 0x10 keep
// DirZ; +0x16C / +0x16D: no reader in the class) and the shot handler's kind table (0x1413582D0 / 0x141357FA0).
const PLAY = [
  // DW: [kind, logical, flags, values = [z gain, root length, DirZ, root-scale divisor, +0x188], DW2 group (+0x198)]
  ['DW', 0x44, 'M_2411', ['keep_face', 'air_mode', 'f16c', 'shot:DW_CURVE', 'hands:1', 'fan:45'], [1, 0, 0, 0, 0], 0x71E, 'LAB_14133887C / LAB_141338892; shot: case 0x44 DW_CURVE, one hand, yaw +45 (hand 0) / -45 (hand 1)'],
  ['DW', 0x45, 'M_2412', ['keep_face', 'air_mode', 'f16c', 'shot:DW_CURVE', 'hands:2', 'fan:45'], [1, 0, 0, 0, 0], 0x71E, 'shot: case 0x45 DW_CURVE, both hands (index, index + 1), yaw +-45'],
  ['DW', 0x46, 'M_2413', ['keep_face', 'air_mode', 'f16c', 'shot:DW_SPIN', 'hands:2', 'outward'], [1, 0, 0, 0, 0], 0x71E, 'shot: case 0x46 DW_SPIN, both hands, direction = muzzle - Sora\'s centre (3D)'],
  ['DW', 0x47, 'M_2414', ['keep_face', 'no_xy', 'keep_dir_z', 'fixed_dir_z', 'finish', 'shot:DW_SPIRAL', 'hands:2', 'ring:8', 'ring_pitch:45'], [1, 0, 250, 0, 0], 0, 'case 0x47 (+0x234 &= ~1, +0x23C |= 0x10, +0x1D4 = 250, +0x1DD = 1; no air mode); Decide: the finisher (+0x140 / +0x141, finish count + 1); shot: DW_SPIRAL, both hands, a ring of 8: yaw of (muzzle - centre).xy + 45 i, then pitched 45'],
  ['DW', 0x48, 'M_2415', ['keep_face', 'air_mode', 'f16d', 'shot:DW_CURVE', 'hands:1', 'outward'], [1, 0, 0, 0, 500], 0x71E, 'case 0x48 (+0x16D = 1, +0x188 = 500); Decide: the target is more below than away (dz < -100 and dz < -dist); shot: DW_CURVE, one hand, direction = muzzle - centre'],
  // BZ
  ['BZ', 0x4A, 'M_5411', ['keep_face', 'air_mode', 'chan_root', 'no_xy_near:450', 'zgain2_dz:150'], [3, 300, 0, 300, 0], 0x721, 'case 0x4A: dist < 450: root-scale divisor 300 and no XY channel; +0x234 |= 2, +0x1D8 = 300; |dz| >= 150: z gain doubled; BZ2: +0x198 = 0x721'],
  ['BZ', 0x4B, 'M_5412', ['keep_face', 'air_mode', 'chan_root', 'no_xy_near:450'], [3, 300, 0, 300, 0], 0x722, 'case 0x4B: as 0x4A (divisor 300 always, no z rule); BZ2: 0x722'],
  ['BZ', 0x4C, 'M_5413', ['keep_face', 'air_mode', 'chan_root', 'no_xy_near:560', 'finish'], [3, 400, 0, 400, 0], 0x723, 'case 0x4C: dist < 560: no XY channel; divisor 400; +0x1D8 = 400; BZ2: 0x723; Decide: the finisher (near)'],
  ['BZ', 0x4D, 'MagicFire', ['no_xy', 'air_clip:MagicAirFire', 'shot:BZ_BOMBSHOT1'], [3, 0, 0, 0, 0], 0, 'case 0x4D: MagicFire (bAir: MagicAirFire with air mode), +0x234 &= ~1, +0x19C = 0x36; shot (0x141357FA0): BZ_BOMBSHOT1 at the notify\'s muzzle, along his forward, homing on the attack target; Decide: the far first steps (dist > 600)'],
  ['BZ', 0x4E, 'MagicShoot', ['no_xy', 'air_clip:MagicAirShoot', 'finish', 'shot:BZ_BOMBSHOT2'], [3, 0, 0, 0, 0], 0, 'case 0x4E: MagicShoot (MagicAirShoot), +0x19C = 0x37; shot: BZ_BOMBSHOT2; Decide: the far finisher'],
];

// ------------------------------------------------------------------------------------------------ FK (as tools/sora_pack.js attackSockets
// and tools/sora_form_claws.js): the form weapons' sockets per frame of Sora's clip, ACTOR space, UE cm (x forward, y right,
// z up): Sora's skeleton + the weapon's socket on it (R_wep1_skt / L_wep1_skt, the row's `units`) + the weapon's skeleton
// posed by ITS clip of the same logical name (else its reference pose) + the weapon socket (sockets.json).
const sp = require('./sora_pack.js');
const qmulH = (a, b) => [a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1], a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0], a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3], a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]];
const qrot = (q, v) => { const tx = 2 * (q[1] * v[2] - q[2] * v[1]), ty = 2 * (q[2] * v[0] - q[0] * v[2]), tz = 2 * (q[0] * v[1] - q[1] * v[0]); return [v[0] + q[3] * tx + (q[1] * tz - q[2] * ty), v[1] + q[3] * ty + (q[2] * tx - q[0] * tz), v[2] + q[3] * tz + (q[0] * ty - q[1] * tx)]; };
const under = (c, p) => { const w = qrot(p.q, [p.s[0] * c.t[0], p.s[1] * c.t[1], p.s[2] * c.t[2]]); return { q: qmulH(p.q, c.q), t: [w[0] + p.t[0], w[1] + p.t[1], w[2] + p.t[2]], s: [p.s[0] * c.s[0], p.s[1] * c.s[1], p.s[2] * c.s[2]] }; };
const point = (T, w) => { const r = qrot(T.q, [T.s[0] * w[0], T.s[1] * w[1], T.s[2] * w[2]]); return [r[0] + T.t[0], r[1] + T.t[1], r[2] + T.t[2]]; };
function rotQ(r) {
  const h = Math.PI / 360, SP = Math.sin(r.pitch * h), CP = Math.cos(r.pitch * h), SY = Math.sin(r.yaw * h), CY = Math.cos(r.yaw * h), SR = Math.sin(r.roll * h), CR = Math.cos(r.roll * h);
  return [CR * SP * SY - SR * CP * CY, -CR * SP * CY - SR * CP * SY, CR * CP * SY - SR * SP * CY, CR * CP * CY + SR * SP * SY];
}
const ID = MAIN.identification;
const SORA_BONES = sp.buildSkeleton(sp.readGlb(P('assets_converted', 'meshes', sp.CONTENT, sp.gamePathToContent(ID.skeletal_mesh) + '.glb')).json);
const SORA_SOCKETS = (() => {
  const d = sp.loadPkg(sp.gamePathToContent(ID.skeleton));
  const out = {};
  for (const e of d.exports.filter((x) => x.class === 'SkeletalMeshSocket')) {
    const p = sp.propsOf(e);
    const loc = p.RelativeLocation ? p.RelativeLocation.value : { x: 0, y: 0, z: 0 }, rot = p.RelativeRotation ? p.RelativeRotation.value : { pitch: 0, yaw: 0, roll: 0 }, sc = p.RelativeScale ? p.RelativeScale.value : { x: 1, y: 1, z: 1 };
    out[p.SocketName.value] = { bone: p.BoneName.value, T: { q: rotQ(rot), t: [loc.x, loc.y, loc.z], s: [sc.x, sc.y, sc.z] } };
  }
  return out;
})();
const meshYaw = ID.mesh_component.relative_rotation_ue_deg.yaw * Math.PI / 180, cyaw = Math.cos(meshYaw), syaw = Math.sin(meshYaw);
const actorSpace = (w) => [w[0] * cyaw - w[1] * syaw, w[0] * syaw + w[1] * cyaw, w[2]];
const localT = (b, tr, time) => ({ q: tr.rotation ? sp.sampleChan(tr.rotation, time)(tr.rotation.ue_values) : b.ue.rotation, t: tr.translation ? sp.sampleChan(tr.translation, time)(tr.translation.ue_values) : b.ue.translation, s: tr.scale ? sp.sampleChan(tr.scale, time)(tr.scale.ue_values) : b.ue.scale });
const byBone = (a) => { const m = new Map(); if (a) for (const c of a.chans) { if (!m.has(c.bone)) m.set(c.bone, {}); m.get(c.bone)[c.path] = c; } return m; };
function boneT(bones, chans, k, time, rootMotion) {
  const chain = []; for (let i = k; i >= 0; i = bones[i].parent) chain.unshift(i);
  let T = null;
  for (const i of chain) { const l = localT(bones[i], (i === 0 && rootMotion) ? {} : (chans.get(i) || {}), time); T = T ? under(l, T) : l; }
  return T;
}
const soraAnims = new Map();
function soraAnim(clip) {
  if (soraAnims.has(clip)) return soraAnims.get(clip);
  const it = item(clip);
  let a = null;
  if (it && it.source_asset) { try { a = sp.loadAnimation(it.source_asset + '.' + path.basename(it.source_asset), SORA_BONES); } catch (e) { a = null; } }
  soraAnims.set(clip, a);
  return a;
}
const weapons = new Map();
function weaponOf(model) {
  if (weapons.has(model)) return weapons.get(model);
  const dir = P('build', 'sora_pack', 'keyblades', model);
  if (!fs.existsSync(path.join(dir, 'model.json'))) { weapons.set(model, null); return null; }
  const mj = readJson(path.join(dir, 'model.json')), sj = readJson(path.join(dir, 'sockets.json'));
  const bones = sp.buildSkeleton(sp.readGlb(P('assets_converted', 'meshes', sp.CONTENT, sp.gamePathToContent(mj.source_mesh) + '.glb')).json);
  const w = { model, mj, sockets: sj.weapon_sockets || {}, bones, anims: new Map() };
  weapons.set(model, w);
  return w;
}
function weaponAnim(w, logical, clip) {
  const key = logical + '|' + clip;
  if (w.anims.has(key)) return w.anims.get(key);
  const c = w.mj.clips.find((x) => (x.logical_names || []).includes(logical)) || w.mj.clips.find((x) => x.sequence === clip);
  let a = null;
  if (c) { try { a = sp.loadAnimation(c.asset + '.' + c.sequence, w.bones); } catch (e) { a = null; } }
  w.anims.set(key, a);
  return a;
}
// the actor-space location of `socketName` of the weapon part at Sora's clip time
function weaponSocket(part, socketName, logical, clip, time, offset) {
  const w = weaponOf(part.model);
  const sa = soraAnim(clip);
  const sock = SORA_SOCKETS[part.socket];
  if (!w || !sa || !sock) return null;
  const hand = SORA_BONES.findIndex((b) => b.name === sock.bone);
  const H = boneT(SORA_BONES, byBone(sa), hand, Math.min(time, sa.length), sa.props.bEnableRootMotion === true);
  const base = under(sock.T, H);
  const a = weaponAnim(w, logical, clip);
  const ws = w.sockets[socketName];
  const k = w.bones.findIndex((b) => b.name === (ws ? ws.bone : socketName));
  if (k < 0) return null;
  let T = under(boneT(w.bones, byBone(a), k, a ? Math.min(time, a.length) : 0, false), base);
  if (ws) T = under({ q: rotQ({ pitch: ws.local_rotation[0], yaw: ws.local_rotation[1], roll: ws.local_rotation[2] }), t: ws.local_location, s: [1, 1, 1] }, T);
  return actorSpace(point(T, offset || [0, 0, 0]));
}
// the form's weapon parts (unit@part:socket=blueprint)
const partsOf = (form) => FORM[form].units.map((u) => { const m = u.match(/^(\w+)@(\w+):(\w+)=(.*)$/); must(m, 'unit ' + u); return { unit: m[1], socket: m[3] === 'None' ? 'R_wep_skt' : m[3], model: m[4].split('/').pop().replace(/_[Ww]ep$/, '') }; });
// per frame of the clip: [weapon 0 muzzle xyz, weapon 1 muzzle xyz] (MuzzleAttachPoint: the class default ATK_01, constructor
// 0x141777AE0; the form weapons' blueprints do not change it); a one-weapon form repeats weapon 0 (index out of range -> 0).
function muzzleTrack(form, logical, clip) {
  const it = item(clip);
  if (!it) return null;
  const parts = partsOf(form);
  const n = Math.max(2, it.num_frames || 2), out = [];
  for (let f = 0; f < n; f++) {
    const t = f * it.duration_s / (n - 1);
    const m0 = weaponSocket(parts[0], 'ATK_01', logical, clip, t);
    if (!m0) return null;
    const m1 = parts[1] ? weaponSocket(parts[1], 'ATK_01', logical, clip, t) : m0;
    out.push(...m0, ...(m1 || m0));
  }
  return out.map((x) => Math.round(x * 10) / 10);
}
// per frame: the launcher's melee shape of a group [a xyz, b xyz] (CAPSULE2 ATK_00 + loc .. ATK_01; SPHERE at socket 1 + loc)
function shapeTrack(form, shape, logical, clip) {
  const it = item(clip);
  const part = partsOf(form)[0];
  const n = Math.max(2, it.num_frames || 2), out = [];
  for (let f = 0; f < n; f++) {
    const t = f * it.duration_s / (n - 1);
    const a = weaponSocket(part, shape.s1, logical, clip, t, shape.loc);
    if (!a) return null;
    const b = shape.s2 === 'None' ? a : weaponSocket(part, shape.s2, logical, clip, t, [0, 0, 0]) || a;
    out.push(...a, ...b);
  }
  return out.map((x) => Math.round(x * 10) / 10);
}

// ------------------------------------------------------------------------------------------------ rows
const ROWS = [];
const add = (r) => ROWS.push({ form: '-', code: 0, logical: '-', clip: '-', length: 0, group: 0, flags: [], values: [], ref_addr: '-', ref_conf: 'confirmed', note: '-', ...r });

// the replacement anim sets
for (const f of Object.keys(FORM)) {
  for (const [logical, r] of [...ROWS_OF[f].entries()].sort((a, b) => a[0].localeCompare(b[0]))) {
    const n = notes(r.clip);
    add({ id: 'anim.' + f + '.' + logical, what: 'anim', form: f, logical, clip: r.clip, length: n.length, flags: [r.set, ...n.shots.map((s) => 'shot:' + s)], values: [n.change, n.input], note: 'm_ReplaceAnimSets ' + r.set + ': ' + logical + ' plays clip ' + r.clip + ' (flags = [set, shot notifies time@LocationIndex]; values = [TresEnableChange, TresEnableInput], -1 = none)' });
  }
}
// the attack kinds (PlayAttack + the shot handler)
for (const [fam, kind, logical, flags, values, group, note] of PLAY) {
  const clip = ROWS_OF[fam].get(logical);
  must(clip, fam + ' ' + logical + ': no clip in the form\'s anim sets');
  const n = notes(clip.clip);
  add({ id: 'attack.' + fam + '.' + kind, what: 'attack', form: fam, code: kind, logical, clip: clip.clip, length: n.length, group, flags: [...flags, ...n.shots.map((s) => 'notify:' + s), ...n.rows.map((s) => 'window:' + s)], values, ref_addr: '0x1413378B0', note: note + '; values = [z gain, root length, DirZ, root-scale divisor, +0x188]; group = the DW2 / BZ2 effect group (+0x198)' });
}
// the projectiles of the four forms (Form::projectile name -> blueprint; these rows by class)
for (const f of Object.keys(FORM)) for (const p of FORM[f].projectiles) { const [, bp] = p.split('='); addProj(setName(bp), FORM[f].projectile_set); }
// the form weapons' melee shapes (StickM / StickL / EnergyRelease of the launchers)
for (const w of ['w_so024_Wep', 'w_so044_Wep']) {
  const bp = blueprint('/Game/Blueprints/Player/p_ex001/' + w);
  for (const s of (bp.comps.TresAtkColl0 || {}).CollisionShapesSrc || []) {
    if (!s.m_GrpName || s.m_GrpName === 'None') continue;
    const v = (o) => (o ? [o.x, o.y, o.z] : [0, 0, 0]);
    add({ id: 'shape.' + w + '.' + s.m_GrpName, what: 'shape', form: w === 'w_so024_Wep' ? 'BZ' : 'BZ2', logical: s.m_GrpName, clip: w, flags: [tail(s.m_ShapeType), String(s.m_SocketName1), String(s.m_SocketName2)], values: [...v(s.m_Size), ...v(s.m_RelativeLocation1)].map(r4), note: 'TresAtkColl0 CollisionShapesSrc of ' + w + ': flags = [shape, socket 1, socket 2], values = [size x, y, z, location 1 x, y, z]' });
  }
}
// FINISH_BZ (0x9D), both second stages
{
  const clip = ROWS_OF.BZ.get('M_5417');
  must(clip, 'M_5417 is not in AnimSet5');
  const n = notes(clip.clip);
  must(n.shots.length === 1, 'M_5417 should have one TresShot');
  add({ id: 'finish.BZ.157', what: 'finish', form: 'BZ', code: 157, logical: 'M_5417', clip: clip.clip, length: n.length, flags: ['BZ_BIG_BANG'], values: [parseFloat(n.shots[0]), 30, 1000, n.change], ref_addr: '0x1413DF100', note: 'FINISH_BZ: StyleFinish slot 143 0x1413DF100 (sub 0: Play M_5417, m_bFaceAt, sub 5; sub 5: DoChain at the clip\'s end; no gravity / traction writes); its TresShot: 0x1413DD180 (BZ_BIG_BANG, weapon 0\'s muzzle, direction = (pitch 30, yaw of muzzle - centre), homing to the point 1000 ahead of Sora\'s centre, WorldSlow off). values = [shot time, pitch, aim distance, TresEnableChange]' });
}
// the muzzles (per frame of the clips that shoot) and the launchers' melee shapes (per frame of the clips with windows)
const SHAPES = {};
for (const r of ROWS.filter((x) => x.what === 'shape')) SHAPES[r.form + '.' + r.logical] = { s1: r.flags[1], s2: r.flags[2], loc: r.values.slice(3, 6), size: r.values[0] };
for (const f of Object.keys(FORM)) {
  const fam = f.startsWith('DW') ? 'DW' : 'BZ';
  const shooting = ROWS.filter((r) => (r.what === 'attack' && r.form === fam) || (r.what === 'finish' && fam === 'BZ'));
  for (const r of shooting) {
    const logical = r.logical;
    const clip = ROWS_OF[f].get(logical);
    if (!clip) continue;
    const air = flagValue(r.flags, 'air_clip');
    for (const [lg, cl] of [[logical, clip.clip], ...(air && ROWS_OF[f].get(air) ? [[air, ROWS_OF[f].get(air).clip]] : [])]) {
      if (r.what === 'attack' && !r.flags.some((x) => x.startsWith('notify:')) && !r.flags.some((x) => x.startsWith('window:'))) continue;
      const m = muzzleTrack(f, lg, cl);
      must(m, f + ' ' + lg + ': no muzzle track (the weapon pack or the clip is missing)');
      add({ id: 'muzzle.' + f + '.' + cl, what: 'muzzle', form: f, logical: lg, clip: cl, length: item(cl).duration_s, values: m, ref_addr: '0x1405AA790', note: 'pawn vt+0x1410(i) (0x1405AA790 -> equipment vt+0x580 0x1417812E0): weapon[i]\'s ATK_01 per frame (frame k at k x length / (frames - 1)), actor space cm: values = [w0 x, y, z, w1 x, y, z] per frame (FK: Sora\'s ' + cl + ' + the weapon socket + the weapon\'s own clip of the same name)' });
      if (fam === 'BZ') {
        for (const w of r.flags.filter((x) => x.startsWith('window:'))) {
          const group = w.split('@')[1];
          const sh = SHAPES[(f === 'BZ2' ? 'BZ2' : 'BZ') + '.' + group];
          must(sh, f + ' ' + group + ': no shape');
          const t = shapeTrack(f, sh, lg, cl);
          must(t, f + ' ' + lg + ' ' + group + ': no shape track');
          add({ id: 'track.' + f + '.' + cl + '.' + group, what: 'track', form: f, logical: group, clip: cl, length: item(cl).duration_s, values: [sh.size, ...t], ref_addr: '0x140647950', note: 'the form weapon\'s TresAtkColl0 group ' + group + ' per frame of ' + cl + ', actor space cm: values = [radius, then per frame a x, y, z, b x, y, z] (CAPSULE2 socket 1 + m_RelativeLocation1 .. socket 2; SPHERE at socket 1 + the location)' });
        }
      }
    }
  }
}

// the Hunny forms' guard counter Barrier Cracker (kind 0x25: PlayAttack's BarrierCracker / AirBarrierCracker, AnimSet16 / 17)
// the arrowguns' Sliding Shot / Back Slide (AirSlide's clips, shot handler 0x141343E10)
for (const f of ['DW', 'DW2']) for (const lg of ['SlideShotF', 'SlideShotB', 'SlideShotR', 'SlideShotL']) {
  const clip = ROWS_OF[f].get(lg);
  must(clip, f + ' ' + lg + ': no clip');
  if (ROWS.some((r) => r.id === 'muzzle.' + f + '.' + clip.clip)) continue;
  const m = muzzleTrack(f, lg, clip.clip);
  must(m, f + ' ' + lg + ': no muzzle track');
  add({ id: 'muzzle.' + f + '.' + clip.clip, what: 'muzzle', form: f, logical: lg, clip: clip.clip, length: item(clip.clip).duration_s, values: m, ref_addr: '0x1405AA790', note: 'as the other muzzle rows (AirSlide: Sliding Shot / Back Slide)' });
}
for (const f of ['DW2', 'BZ2']) for (const lg of ['BarrierCracker', 'AirBarrierCracker']) {
  const clip = ROWS_OF[f].get(lg);
  must(clip, f + ' ' + lg + ': no clip');
  const m = muzzleTrack(f, lg, clip.clip);
  must(m, f + ' ' + lg + ': no muzzle track');
  add({ id: 'muzzle.' + f + '.' + clip.clip, what: 'muzzle', form: f, logical: lg, clip: clip.clip, length: item(clip.clip).duration_s, values: m, ref_addr: '0x1405AA790', note: 'as the other muzzle rows (Barrier Cracker, kind 0x25)' });
}
// (F-ARROWGUN) ShooterMode's shooting clips (state 40, TresShot handler 0x1413B51B0: weapon[LocationIndex]'s muzzle): the
// arrowguns' gatling loop M_2416, the launchers' M_5415 (BZ_BOMBSHOT2) / M_5416 (BZ_CRACKER)
for (const [f, lgs] of [['DW', ['M_2416']], ['DW2', ['M_2416']], ['BZ', ['M_5415', 'M_5416']], ['BZ2', ['M_5415', 'M_5416']]]) for (const lg of lgs) {
  const clip = ROWS_OF[f].get(lg);
  must(clip, f + ' ' + lg + ': no clip');
  if (ROWS.some((r) => r.id === 'muzzle.' + f + '.' + clip.clip)) continue;
  const m = muzzleTrack(f, lg, clip.clip);
  must(m, f + ' ' + lg + ': no muzzle track');
  add({ id: 'muzzle.' + f + '.' + clip.clip, what: 'muzzle', form: f, logical: lg, clip: clip.clip, length: item(clip.clip).duration_s, values: m, ref_addr: '0x1405AA790', note: 'as the other muzzle rows (ShooterMode 0x1413B51B0)' });
}

// (AUDIT-A ARROWGUN) THE WEAPON GLOW of ShooterMode's charge: Tick sub 4 / 5 and End call pawn vt+0x1468(level) =
// 0x1405BDB70 -> MyEquipment+0x200 (its interface, ctor 0x141777FC0: vtable 0x146B8CCF8) slot 60 0x14178A5B0: each weapon
// of m_WeaponEquips -> weapon+0x440 (ATresWeaponBase ctor 0x140968100: vtable 0x146110E10) slot 60 0x14178A480(0, level):
// with m_CmnChargeEffect (weapon+0x620) set: level < 0 stops every group it started; else groups 0x802 + i for i <= level
// (started once, kept), the ones above the level stopped. Only the launchers' weapons set it (w_so024_Wep ead_wep_chg0,
// w_so044_Wep ead_wep_chg1; the arrowguns' w_so022 / 23 / 42 / 43 none). One row per group whose entry names an effect:
// values = [LocationOffset x, y, z, RotationOffset pitch, yaw, roll], flags = [socket on the weapon, the unit's socket on
// Sora, AttachEffectEndType, the asset, the kh-host anchor kb:<model>:<unit socket>:<socket>].
for (const f of Object.keys(FORM)) {
  for (const u of FORM[f].units) {
    const m = /^(\w+)@\w+:(\w+)=(\S+)$/.exec(u);
    if (!m) continue;
    const [, , mount, pkg] = m;
    const bp = blueprint(pkg);
    const asset = bp.cdo.m_CmnChargeEffect;
    if (!asset) continue;
    const j = load(pkg);
    const imp = j.imports.find((i) => i.objectName === asset && i.className === 'SQEX_ParticleAttachDataAsset');
    must(imp, f + ': ' + asset + ' is no attach-data import of ' + pkg);
    const apkg = importPackage(j, imp);
    const a = load(apkg);
    const data = plain(a, (a.exports.find((e) => e.objectName === asset) || {}).data.props);
    const model = setName(pkg).replace(/_Wep$/, '');
    for (const entry of data.EffectDataList || []) {
      for (const g of entry.EffectDataList || []) {
        if (!g.AttachEffect) continue;
        const lo = entry.LocationOffset || {}, ro = entry.RotationOffset || {};
        add({ id: 'charge.' + f + '.' + g.GroupID, what: 'charge', form: f, code: g.GroupID, logical: model, clip: g.AttachEffect, flags: [String(entry.SocketName), mount, tail(entry.AttachEffectEndType), setName(apkg), 'kb:' + model + ':' + mount + ':' + entry.SocketName], values: [lo.x || 0, lo.y || 0, lo.z || 0, ro.pitch || 0, ro.yaw || 0, ro.roll || 0].map(r4), ref_addr: '0x14178A480', note: 'm_CmnChargeEffect of ' + setName(pkg) + ' (' + setName(apkg) + '): group ' + g.GroupID + ' (0x' + g.GroupID.toString(16) + ' = 0x802 + level) plays ' + g.AttachEffect + ' on the weapon\'s ' + entry.SocketName + '; values = [LocationOffset x, y, z, RotationOffset pitch, yaw, roll], flags = [weapon socket, unit socket on Sora, end type, asset, the effect side\'s anchor (kh-host vfx kbweapon)]' });
      }
    }
  }
}
must(ROWS.some((r) => r.id === 'charge.BZ.2050') && ROWS.some((r) => r.id === 'charge.BZ2.2050'), 'the launchers charge (m_CmnChargeEffect group 2050)');

// (AUDIT-A ARROWGUN) THE FORM WEAPONS' AND TRANSFORM UNITS' OWN CLIP NOTIFIES (AUDIT-B's seam, AUDIT-B/AUDIT.md: rows of
// tools/lib/wnotify_rows.js in F-SHIP's flags / values, played by kh-core forms/ship.rs `wnotify_pass` from arrowgun.rs
// `on_notifies`; the hosts resolve the `kb:` anchors on the weapon's own skeleton under its clip). KH3 gives each weapon
// actor of the four forms (the units w_so022 / w_so023 / w_so024 / w_so042 / w_so043 / w_so044 on R_wep1_skt / L_wep1_skt,
// the transform units w_so021 / w_so041 "Deform" with no socket: R_wep_skt as P5 draws them) an anim instance playing the row
// of the logical name Sora plays, and that clip's AttachEffect / AttachSound / PlaySound notifies fire on THEIR mesh: the
// transformations' star pieces / wings / honey spheres and bees, the gatling barrels (2405), the 0x47 charge (M_2414), the
// muzzle glows (M_2416), Rising Blast's honey (M_5413) and the launchers' sounds, FINISH_BZ's own effects and sounds (M_5417).
// Columns: `logical` = the effect name / the sound's /Game path (WNotify effects / sounds), `group` = the row's GroupID.
// The launchers' 5417 / H417 carry TresAnimNotifyState_ChangeAttachParent {kh_root} over the whole clip (0 - 3.0 s): the
// launcher hangs on Sora's kh_root for the finish, so those clips' anchors mount on `kh_root` (the hosts' part placement
// itself is AUDIT-A PLACEMENT's generic reader, kh-core forms/placement.rs `part_parent`; checked here, no row). w_so024 5417 @0.4968 names its effect through a data asset
// (SQEX_AttachEffectNotifyDataToDataAsset ead_at_so022_fns0, notify GroupID -2): the asset's own rows (vw_so022_fns1 on `wep`,
// LocationOffset (0, -10, 280), group 0) are read here, as the effect side reads an ead.
{
  const { wnotifyRows } = require('./lib/wnotify_rows.js');
  const ATTACH_PARENT = new Set(['5417', 'H417']); // the launchers' clips with ChangeAttachParent kh_root (checked below)
  const unit = (model, mount, f) => [
    { model, mount, form: (c) => (ATTACH_PARENT.has(c.sequence) ? null : f) },
    { model, mount: 'kh_root', form: (c) => (ATTACH_PARENT.has(c.sequence) ? f : null) },
  ];
  // a transform unit's clip plays in the form whose transform clip (StyleChange2 / 5 / 16 / 17) is its logical name
  const transform = (model, forms) => ({ model, mount: 'R_wep_skt', form: (c) => forms.find((f) => (c.logical_names || []).includes(FORM[f].transform_clip)) || null });
  const models = [
    ...unit('w_so022', 'R_wep1_skt', 'DW'), ...unit('w_so023', 'L_wep1_skt', 'DW'), ...unit('w_so024', 'R_wep1_skt', 'BZ'),
    ...unit('w_so042', 'R_wep1_skt', 'DW2'), ...unit('w_so043', 'L_wep1_skt', 'DW2'), ...unit('w_so044', 'R_wep1_skt', 'BZ2'),
    transform('w_so021', ['DW', 'BZ']), transform('w_so041', ['DW2', 'BZ2']),
  ];
  for (const f of Object.keys(FORM)) for (const u of [...FORM[f].units, ...FORM[f].transform_units]) {
    const m = /^(\w+)@\w+:(\w+)=(\S+)$/.exec(u);
    const model = setName(m[3]).replace(/_Wep$/, '').toLowerCase();
    must(models.some((x) => x.model === model && (x.mount === (m[2] === 'None' ? 'R_wep_skt' : m[2]) || x.mount === 'kh_root')), f + ': ' + model + ' is no unit of the rows on ' + m[2]);
  }
  for (const r of wnotifyRows(P('build', 'sora_pack'), models)) {
    const name = r.code === 0 ? r.effects[0] : r.sounds[0];
    const g = Number(String(r.flags[3]).slice(1));
    add({ ...r, logical: name, group: Number.isFinite(g) ? g : 0, ref_addr: r.ref_addr + ' (AUDIT-A ARROWGUN)' });
  }
  let parents = 0;
  for (const { model, mount, form } of models) {
    const mj = readJson(P('build', 'sora_pack', 'keyblades', model, 'model.json'));
    for (const c of mj.clips || []) {
      const f = form(c);
      if (!f) continue;
      const clip = ((c.logical_names || []).find((n) => /^M_/.test(n)) || 'M_' + c.sequence).slice(2);
      for (const [k, n] of (c.notifies || []).entries()) {
        const np = n.properties || {};
        if (n.class === 'TresAnimNotifyState_ChangeAttachParent') {
          const sock = prop(n, 'm_SocketName');
          must(ATTACH_PARENT.has(c.sequence) && sock === 'kh_root', model + ' ' + c.sequence + ': ChangeAttachParent ' + sock + ' (expected only on 5417 / H417, kh_root)');
          parents++;
        } else if (/AttachEffect/.test(n.class) && np.AttachDataList && np.AttachDataList.structName === 'SQEX_AttachEffectNotifyDataToDataAsset') {
          const list = np.AttachDataList.items.map((it) => Object.fromEntries(it.map((p) => [p.name, p.value])));
          for (const [i, d] of list.entries()) {
            must(String(d.AttachType).endsWith('OWNER'), model + ' ' + c.sequence + ': data-asset attach type ' + d.AttachType);
            for (const ref of d.AttachDatas || []) {
              const a = load(String(ref));
              const exp = a.exports.find((e) => e.objectName === setName(ref));
              must(exp, model + ' ' + c.sequence + ': no export in ' + ref);
              const data = plain(a, exp.data.props);
              for (const [j, entry] of (data.EffectDataList || []).entries()) {
                for (const e of entry.EffectDataList || []) {
                  if (!e.AttachEffect) continue;
                  const endType = tail(entry.AttachEffectEndType).replace('ESQEX_ATTACH_EFFECT_END_TYPE_', '');
                  const sock = entry.SocketName && entry.SocketName !== 'None' ? entry.SocketName : d.SocketName && d.SocketName !== 'None' ? d.SocketName : '';
                  const anchor = 'kb:' + model + ':' + mount + ':' + sock + (entry.bAbsoluteRotation ? ':abs' : '') + (entry.ToAttach === false ? ':free' : '') + (endType === 'FADEOUT' ? ':fo' + r4(entry.FadeOut || 0) : '');
                  const lo = entry.LocationOffset || {}, ro = entry.RotationOffset || {};
                  const state = /NotifyState_/.test(n.class);
                  const t = r4(n.trigger_time), end = state ? r4(n.end_trigger_time) : -1;
                  const effect = setName(e.AttachEffect);
                  add({ id: 'wnotify.' + model + '.' + c.sequence + '.ead' + k + '_' + i + '_' + j, what: 'wnotify', form: f, code: 0, logical: effect, clip, length: r4(c.duration_s || 0), group: e.GroupID || 0,
                    flags: [anchor, endType, state ? 'state' : 'instant', 'g' + (e.GroupID || 0)], values: [t, end, r4(lo.x || 0), r4(lo.y || 0), r4(lo.z || 0), r4(ro.pitch || 0), r4(ro.yaw || 0), r4(ro.roll || 0), endType === 'FADEOUT' ? r4(entry.FadeOut || 0) : 0],
                    ref_addr: 'build/sora_pack/keyblades/' + model + '/model.json + ' + setName(ref) + ' (AUDIT-A ARROWGUN)',
                    note: 'the ' + model + ' clip ' + c.sequence + ' with Sora\'s ' + clip + ': ' + n.class + ' through the data asset ' + setName(ref) + ' (notify GroupID ' + d.GroupID + ': -2 = the asset\'s rows decide; its row group ' + (e.GroupID || 0) + '); values = [time, end (-1 instant), location x, y, z, rotation pitch, yaw, roll, fade out s]' });
                }
              }
            }
          }
        }
      }
    }
  }
  const own = ROWS.filter((r) => r.what === 'wnotify');
  // every notify of the eight models' clips: 156 effect / sound rows + the 2 ChangeAttachParent rows = AUDIT-A/ARROWGUN/weapon_clip_notifies.tsv's 158
  must(own.length === 156, 'expected 156 wnotify rows, got ' + own.length);
  must(parents === 2, 'expected the 2 ChangeAttachParent windows (5417 / H417), got ' + parents);
  for (const r of own) must(r.logical && r.logical !== '-' && r.form !== '-', r.id + ': no asset / form');
}

// the attack rows they hit with (p_ex001_AttackData), for the combat sheet's owner (STATE.md INT line)
const usedRows = new Set();
for (const r of ROWS) {
  if (r.what === 'proj' && r.flags[0] !== 'None') usedRows.add(r.flags[0].toUpperCase());
  if (r.what === 'attack') for (const w of r.flags.filter((x) => x.startsWith('window:'))) usedRows.add(w.slice(7).split('@')[0].toUpperCase());
}
for (const id of [...usedRows]) if (/^SCBZ_/.test(id)) usedRows.add(id.replace(/^SCBZ_/, 'BZ2_'));
for (const extra of ['SCDW_CHANGE', 'DW2_CHANGE', 'SCBZ_CHANGE', 'BZ2_CHANGE']) usedRows.add(extra);
for (const id of [...usedRows].sort()) {
  const a = ATK.find((r) => r.id.toUpperCase() === id);
  must(a, 'p_ex001_AttackData has no row ' + id);
  add({ id: 'row.' + id, what: 'row', form: /^(DW2|BZ2)_/.test(id) ? 'PO' : 'SS', logical: id, values: [a.m_Power, a.m_FormPoint, a.m_ReactionPowerValue].map(r4), flags: [tail(a.m_DamageKind), tail(a.m_DamageAttribute), tail(a.m_BadStatusKind)], note: 'p_ex001_AttackData row ' + a.id + ': values = [m_Power, m_FormPoint, m_ReactionPowerValue]; the combat sheet sora-combat-attack-data must carry it for the hit to deal it (STATE.md INT line)' });
}

// ------------------------------------------------------------------------------------------------ checks
must(ROWS.filter((r) => r.what === 'attack').length === 10, 'expected 10 attack kinds');
for (const r of ROWS.filter((x) => x.what === 'attack')) must(r.clip !== '-' && r.length > 0, r.id + ': no clip');
must(ROWS.some((r) => r.id === 'proj.TresProjSS_BombShot1Ex'), 'BombShot1 respawns BombShot1Ex');
must(ROWS.some((r) => r.id === 'anim.DW.M_2411' && r.clip === '2411'), 'DW has M_2411');

// ------------------------------------------------------------------------------------------------ sheet
const S = (n, t, g, doc) => [n, t, g, doc];
const COLS = [
  S('id', 'str', 'str', '<what>.<form>.<key>'), S('what', 'enum:anim|attack|proj|shape|finish|muzzle|track|row|charge|wnotify', 'str', 'the row kind (AUDIT-A ARROWGUN: wnotify = a form weapon or transform unit: its own clip notify, tools/lib/wnotify_rows.js)'), S('form', 'str', 'str', 'DW / BZ / DW2 / BZ2 (anim, attack: DW = both COMBO_DW forms), SS / PO (proj, row: Shooting Star / Hunny Spout)'),
  S('code', 'int', 'i32', 'attack kind (attack), finish command (finish), else 0'), S('logical', 'str', 'str', 'the logical anim name (anim, attack, finish), the class (proj), the group (shape), the row (row)'),
  S('clip', 'str', 'str', 'the clip = the animation table row (the sequence); the particle (proj); the weapon (shape)'), S('length', 'num', 'f32', 'the clip length, s'),
  S('group', 'int', 'i32', 'the effect group the play passes in DW2 / BZ2 (PlayAttack +0x198), 0 = none'),
  S('flags', 'list', 'str[]', 'strings by kind (see the note)'), S('values', 'list', 'f32[]', 'numbers by kind (see the note)'),
  S('ref_addr', 'str', 'str', 'decompiled function'), S('ref_conf', 'enum:confirmed|inferred|guess', 'str', 'confirmed = read in the code or the data; inferred; guess'), S('note', 'str', 'str', '-'),
];
const NAME = 'sora-form-arrowgun';
const rows = ROWS.map((r) => COLS.map((c) => r[c[0]]));
for (const [i, r] of rows.entries()) for (const [k, v] of r.entries()) must(v !== undefined && v !== null && !(typeof v === 'number' && !Number.isFinite(v)), 'row ' + ROWS[i].id + ': cell ' + COLS[k][0] + ' is ' + v);
must(new Set(ROWS.map((r) => r.id)).size === ROWS.length, 'duplicate row ids');
fs.writeFileSync(P('sheets', 'port', NAME + '.json'), '{"sheet":"' + NAME + '","group":"port","columns":' + JSON.stringify(COLS.map((c) => c[0])) + ',"rows":[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
const by = {}; for (const r of ROWS) by[r.what] = (by[r.what] || 0) + 1;
console.log('sheets/port/' + NAME + '.json: ' + rows.length + ' rows ' + JSON.stringify(by));

// ------------------------------------------------------------------------------------------------ register (this tool's entry only; replaced in place)
const q = (s) => "'" + String(s).replace(/\\/g, '\\\\').replace(/'/g, "\\'") + "'";
const NL = '\n';
const DOC = 'KH3\'s gun formchanges (Double Arrowguns, Magic Launcher, Hunny Blasters, Hunny Launcher) as data for their form package (kh-core combat/forms/arrowgun.rs, states/combo_dw.rs, states/combo_bz.rs): COMBO_DW / COMBO_BZ\'s attack kinds and shots, the replacement anim sets, the gun projectiles, the launchers\' melee shapes, FINISH_BZ, the attack rows.';
const MARK = '(weapons F-ARROWGUN)';
function upsert(s, name, text, end, pos) {
  const at = s.indexOf("'" + name + "': {");
  if (at < 0) { const p = pos(s); return s.slice(0, p) + text + s.slice(p); }
  const c = s.lastIndexOf(NL, at - 1) + 1, cs = s.lastIndexOf(NL, c - 2) + 1;
  must(s.slice(cs, c).includes(MARK), 'the entry of ' + name + ' is not this tool\'s (no "' + MARK + '" comment above it)');
  const e = s.indexOf(end, at); must(e > at, 'end of the entry of ' + name);
  return s.slice(0, cs) + text + s.slice(e + end.length);
}
const save = (f, before, after, what) => { if (after !== before) { fs.writeFileSync(f, after); console.log('registered ' + NAME + ' in ' + what); } };
{
  const f = P('tools', 'lib', 'sheet_defs.js'); const before = fs.readFileSync(f, 'utf8');
  const END = '] },' + NL;
  const pos = (t) => { const at = t.indexOf("  'sora-keyblade-forms': {"); must(at >= 0, 'sheet_defs.js: anchor sora-keyblade-forms not found'); const e = t.indexOf(END, at); return e + END.length; };
  const text = '  // ' + MARK + ' ' + DOC + ' Written by tools/sora_form_arrowgun.js (documentation/KH3_WEAPONS_PLAN.md 1.3)' + NL +
    '  ' + q(NAME) + ": { group: 'port', file: " + q(NAME + '.json') + ", by: 'tools/sora_form_arrowgun.js', from: " + q('sora-keyblade-forms, the form anim sets of the asset pack, the projectile / form-weapon blueprints, p_ex001_AttackData and the decompiled PlayAttack / ComboDW / ComboBZ / shot handlers / StyleFinish') + ', external: true, cols: [' + NL +
    '    ' + COLS.map((c, i) => '[' + q(c[0]) + ', ' + (i === 0 ? 'K' : 'V') + ', ' + q(c[1]) + ", '-', " + q(c[3]) + (['note', 'flags', 'values', 'logical', 'clip'].includes(c[0]) ? ', true' : '') + ']').join(', ') + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/sheet_defs.js');
}
{
  const f = P('tools', 'lib', 'port_gen_defs.js'); const before = fs.readFileSync(f, 'utf8');
  const END = NL + '    },' + NL;
  const pos = (t) => { const anchor = '  tables: {' + NL; const at = t.indexOf(anchor); must(at >= 0, 'port_gen_defs.js: anchor tables'); return at + anchor.length; };
  const cols = COLS.filter((c) => c[2]).map((c) => c[0] + ': { type: ' + q(c[2]) + (c[3] && c[3] !== '-' ? ', doc: ' + q(c[3]) : '') + ' }').join(', ');
  const text = '    // ' + MARK + ' ' + NAME + ': written by tools/sora_form_arrowgun.js (documentation/KH3_WEAPONS_PLAN.md 1.3)' + NL +
    '    ' + q(NAME) + ': {' + NL +
    "      file: 'sora_form_arrowgun.rs', row: 'FormArrowgunRow', table: 'FORM_ARROWGUN', key: 'id', key_prefix: 'FAG'," + NL +
    '      doc: ' + q(DOC) + ',' + NL +
    '      columns: { ' + cols + ' },' + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/port_gen_defs.js');
}
console.log('sora_form_arrowgun ' + VERSION);
