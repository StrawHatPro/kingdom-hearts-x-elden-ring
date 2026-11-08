// (HM-MAGIC) KH3's TURN-TYPE magic set `Blueprints/Player/p_ex001/p_ex001_hm_MagicSet` as DATA for the cast state
// (kh-core magic/hm_set.rs, states/fire_magic.rs) and the spell actors (magic/projectile.rs): the set's 30 units, the 20
// `TresProj_Mag*CL` blueprints its units spawn, the formchanges whose weapon form installs it (m_ReplaceMagicSet of
// sora-keyblade-forms: Favorite Deputy's Hyper Hammer, Happy Gear's Agile Claws, Crystal Snow's Blizzard Claws and Blizzard
// Blades, Wheel of Fate's Storm Flag, Classic Tone's Boom Hammer), their MagicTurn / MagicSpin clips (the forms' anim
// sets) with the Shot times, and the form weapon's ATK_01 per frame of those clips (where the turn-type shot spawns).
//
//   node tools/sora_hm_magic.js              write sheets/port/sora-hm-magic.json
//   node tools/sora_hm_magic.js --register   also register it (once, in place) in tools/lib/sheet_defs.js and
//                                            tools/lib/port_gen_defs.js; then node tools/gen_port.js
//
// THE CODE, read in the decompiled exe (re/decomp), docs in build/sora_tmp/weapons/HM-MAGIC/STATE.md:
//   UNIT     UTresEquipmentComponent 0x141780ED0 (pawn vt+0x1440): the unit of m_ReplaceMagicSet (+0x298) when that set has
//            one, else m_MagicSet's (+0x280); class 0x141780F20 (pawn vt+0x1438) the same order. The hm set has no Cure unit:
//            Cure / Cura / Curaga / Curagan keep p_ex001_MagicSet's.
//   INSTALL  StartStyle 0x14141A9D0 -> ChangeWeapon -> UTresEquipmentComponent 0x141791120: the form entry's
//            m_ReplaceMagicSet (+0x40); EndStyle's weapon change back installs the Keyblade's own (null).
//   TICK     TresPlayerState_FireMagic::Tick 0x1413794B0: isTurn = vt+0x3A8 (0x14136F400: unit +4 bit 1); the moving flag is
//            cleared by vt+0x3C0 (0x14136F320: weapon form 3, 5, 6, 7, 12, 14, 15, 17, 18, 20, 21); a standing turn-type
//            unit plays MagicTurn / MagicAirTurn, a finish-end or grand one MagicSpin / MagicAirSpin.
//   SHOT     OnShot 0x14137BB30: a turn-type unit -> vt+0x3F0 = 0x141380470 and return: class vt+0x1438(cmd, 0); P = pawn
//            vt+0x1410(LocationIndex) (weapon[i]'s ATK_01, no height limit, no wall trace); T = root + (0, 0, 100); D = the
//            actor rotation's vector; spawner vt+0x788(class, pawn, P, T, D, team, 0, 0); setters 0x1100(cmd, -1),
//            0x1130(+0xB4), 0x1140(+0xB8, +0xBC, +0xC0), 0x1160(flag 0x20000), 0x1120(flag 0x80, flag 0x400) and NO homing
//            target; fire effects at P (0x11F8) or on the weapon socket (0x1200, m_bAttachFireEffect); StartFire 0x1270;
//            BulletCount += 1; MP (0x14140D560) unless grand; flags |= 0x200000; pawn vt+0x1D80(1), vt+0x1728(1), Falling.
//   HITS     TresAtkColl TimerTick 0x14062CFC0: with m_AttackInterval > 0 the hit list is emptied every interval from the
//            first hit (the CL shapes are the only magic shapes with an interval).
'use strict';
const fs = require('fs');
const path = require('path');
const sp = require('./sora_pack.js');
const ROOT = path.resolve(__dirname, '..');
const VERSION = '1.0.0';
const P = (...a) => path.join(ROOT, ...a);
const readJson = (f) => JSON.parse(fs.readFileSync(f, 'utf8'));
const must = (c, m) => { if (!c) throw new Error(m); };
const r4 = (x) => Math.round(x * 10000) / 10000;
const r1 = (x) => Math.round(x * 10) / 10;
const short = (s) => (typeof s === 'string' ? s.replace(/^E[A-Za-z]+::/, '') : s);
const REGISTER = process.argv.includes('--register');

const SET = '/Game/Blueprints/Player/p_ex001/p_ex001_hm_MagicSet.p_ex001_hm_MagicSet';
const PKG = P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content');
const ROWS = [];
const add = (r) => ROWS.push({ form: '-', code: 0, logical: '-', clip: '-', length: 0, flags: [], rows: [], effects: [], sounds: [], values: [], ref_addr: '-', ref_conf: 'confirmed', note: '-', ...r });

// ------------------------------------------------------------------------------------------------ the set (m_Units)
const setJson = readJson(path.join(PKG, 'Blueprints', 'Player', 'p_ex001', 'p_ex001_hm_MagicSet.uasset.json'));
const imp = (i) => (i < 0 && setJson.imports[-i - 1] ? setJson.imports[-i - 1].objectName : null);
const units = setJson.exports[0].data.props.find((p) => p.name === 'm_Units').value.items.map((it) => Object.fromEntries(it.map((q) => [q.name, q])));
must(units.length === 30, 'p_ex001_hm_MagicSet: 30 units expected, found ' + units.length);
const CMD = readJson(P('sheets', 'port', 'sora-magic-spells.json')).rows.filter((r) => /^cast\.magicset\.main\.[a-z_]+\.cmd$/.test(r[0])).map((r) => [r[0].split('.')[3], r[1]]);
const cmdOf = (key) => { const f = CMD.find((c) => c[0] === key); must(f, 'no command id for ' + key + ' in sora-magic-spells (cast.magicset.main.<key>.cmd)'); return f[1]; };
const classes = new Set();
for (const u of units) {
  const key = u.m_Command.value.replace('TRES_CMD_KIND_', '').toLowerCase();
  const cls = imp(u.m_AssetData00.value);
  must(cls && /^TresProj_Mag[A-Za-z]+CL_C$/.test(cls), key + ': class ' + cls);
  must(u.m_AssetData01.value === 0, key + ': m_AssetData01 set');
  classes.add(cls);
  add({ id: 'set.' + key, what: 'set', code: cmdOf(key), logical: key, clip: cls,
    values: [+u.m_bIsCastType.value, +u.m_bIsTurnType.value, +u.m_bIsDirectPos.value, u.m_EffectGroup.value, u.m_VoiceGroup.value, u.m_VoiceGroupFinish.value],
    ref_addr: '0x141780ED0', note: 'TresPlayerMagicAssetUnit of p_ex001_hm_MagicSet: code = m_Command, logical = the unit key of cast.magicset.main.<key>, clip = m_AssetData00; values = [m_bIsCastType, m_bIsTurnType, m_bIsDirectPos, m_EffectGroup, m_VoiceGroup, m_VoiceGroupFinish]' });
}
must(classes.size === 20, '20 CL classes expected');

// ------------------------------------------------------------------------------------------------ the CL actors
const { resolve } = require(P('build', 'sora_tmp', 'magic', 'proj', 'resolve.js'));
const ELEMENT = { Fire: ['Fire', 'Fira', 'Figa', 'Figan'], Blizzard: ['Blizzard', 'Blizzara', 'Blizzaga', 'Blizzagan'], Thunder: ['Thunder', 'Thundara', 'Thundaga', 'Thundagan'], Water: ['Water', 'Watara', 'Wataga', 'Watagan'], Aero: ['Aero', 'Aerora', 'Aeroga', 'Aerogan'] };
const nameOf = (x) => (x && x !== 'null' && x !== 'None' ? String(x) : '-');
const sePath = (n) => (n === '-' ? '-' : '/Game/Sound/SE/common/00002_magic/' + n);
for (const [element, tiers] of Object.entries(ELEMENT)) tiers.forEach((t, tier) => {
  const rel = 'Blueprints/Projectile/Magic' + element + '/TresProj_Mag' + t + 'CL';
  const r = resolve(rel);
  const cls = 'TresProj_Mag' + t + 'CL_C';
  must(classes.has(cls), cls + ' is not a unit class of the set');
  const v = (k, d) => (k in r.cdo ? r.cdo[k].v : d);
  const mv = (k, d) => (r.comps.ProjectileComp && r.comps.ProjectileComp.props[k] ? r.comps.ProjectileComp.props[k].v : d);
  const ac = r.comps.TresAtkColl0.props;
  const shapes = ac.CollisionShapesSrc.v;
  must(shapes.length === 1 && short(shapes[0].m_ShapeType) === 'SPHERE', cls + ': one SPHERE');
  const s = shapes[0];
  const auto = ac.m_CollAutoActivate ? ac.m_CollAutoActivate.v : [];
  must(auto.length === 1 && auto[0].m_AttackDataIDName === s.m_DefaultAttackDataIDName, cls + ': one auto activation of the shape row');
  const he = ac.m_HitEffects.v[0];
  const fire = v('MyFireEffects', [])[0] || {};
  must(mv('InitialSpeed', 0) === 0 && mv('MaxSpeed', 0) === 0 && !mv('m_bEnableHoming', false), cls + ': a static actor');
  must(v('m_bNoDestroyOnHit', false) === true && v('m_bReSpawnProjectileOnTimeOut', false) === false, cls + ': not destroyed on hit, no respawn');
  must(!v('m_bCallPutEffectEventCall', false) && !v('m_bExplodeOnTimeOut', false), cls + ': no put effect, no explode on time-out');
  const residues = v('m_ResidueList', []);
  must(!residues || residues.length === 0, cls + ': no residue');
  const native = r.chain.filter((c) => /^TresProj_Magic[A-Z][a-z]+$/.test(c)).pop();
  add({ id: 'actor.' + t + 'CL', what: 'actor', logical: element.toLowerCase(), code: tier + 1, clip: cls,
    flags: [native, nameOf(v('m_ReSpawnProjectileClass', 'null')), short(v('m_RespawnCheckType', '-'))],
    rows: [s.m_DefaultAttackDataIDName, nameOf(v('m_OverrideFinishAttackDataID', 'None')), nameOf(v('m_OverrideSituationAttackDataID', 'None'))],
    effects: [nameOf(r.comps.ParticleComp.props.Template && r.comps.ParticleComp.props.Template.v), nameOf(fire.MyParticleSystem), nameOf(he.m_PawnHitEffect), nameOf(he.m_MapHitEffect)],
    sounds: [sePath(nameOf(fire.MySoundCue)), sePath(nameOf(he.m_PawnHitSEAsset)), sePath(nameOf(he.m_MapHitSEAsset))],
    values: [v('m_MainLife', 10), s.m_Size.x, s.m_IncSize.x, s.m_IncMaxSize.x, auto[0].m_AttackInterval, v('m_SwirlLimit', -1), s.m_RelativeLocation1.x, s.m_RelativeLocation1.y, s.m_RelativeLocation1.z, r4(v('m_AutoStuckShutdownTime', 0))],
    ref_addr: '0x141486890',
    note: 'Blueprints/Projectile/Magic' + element + '/TresProj_Mag' + t + 'CL (chain ' + r.chain.join(' > ') + '): code = tier (4 = -gan), flags = [native class, m_ReSpawnProjectileClass (inert: m_bReSpawnProjectileOnTimeOut false gates every respawn, 0x141477100), m_RespawnCheckType]; rows = [shape row, m_OverrideFinishAttackDataID, m_OverrideSituationAttackDataID]; effects = [ParticleComp.Template (the ring), MyFireEffects[0].MyParticleSystem, m_HitEffects[0].m_PawnHitEffect, m_MapHitEffect]; sounds = [MyFireEffects[0].MySoundCue, m_PawnHitSEAsset, m_MapHitSEAsset]; values = [m_MainLife s, sphere radius cm, m_IncSize.x cm/s, m_IncMaxSize.x cm, m_CollAutoActivate interval s, m_SwirlLimit, m_RelativeLocation1 x y z, m_AutoStuckShutdownTime]; InitialSpeed = MaxSpeed = 0, no homing, m_bNoDestroyOnHit' });
});

// ------------------------------------------------------------------------------------------------ the forms and their clips
const sheet = (name) => { const s = readJson(P('sheets', 'port', name + '.json')); return s.rows.map((r) => Object.fromEntries(s.columns.map((c, i) => [c, r[i]]))); };
const FORMS = sheet('sora-keyblade-forms').filter((r) => r.magic_set === SET);
must(FORMS.length === 6, 'six formchanges name p_ex001_hm_MagicSet (HM, CL, CL2, SK, FL, HM2): ' + FORMS.map((f) => f.id));
const MOVE_CLEAR = [3, 5, 6, 7, 12, 14, 15, 17, 18, 20, 21];
for (const f of FORMS) {
  must(MOVE_CLEAR.includes(f.weapon_form_value), f.id + ': weapon form ' + f.weapon_form_value + ' not in the moving-clear list');
  add({ id: 'form.' + f.id.slice(5), what: 'form', form: f.id.slice(5), code: f.cmd, logical: f.command, clip: f.magic_set, values: [f.weapon_form_value], flags: f.anim_sets.map((a) => String(a).split('/').pop().split('.')[0]), ref_addr: '0x141791120',
    note: f.name + ' (' + f.keyblade_name + ', ' + f.weapon_form + '): StartStyle installs m_ReplaceMagicSet = p_ex001_hm_MagicSet with the weapon form; EndStyle\'s weapon change back removes it. code = the form command, values = [weapon form value], flags = the form\'s anim sets' });
}
add({ id: 'code.move_clear_forms', what: 'code', values: MOVE_CLEAR, ref_addr: '0x14136F320', note: 'FireMagic vt+0x3C0 (0x14136F320: switch(form - 3) cases 0, 2, 3, 4, 9, 11, 12, 14, 15, 17, 18): a cast in these weapon forms is never a moving cast (Tick 0x1413794B0 clears flag 2)' });
add({ id: 'code.turn_target_lift', what: 'code', values: [100], ref_addr: '0x141380470', note: 'the turn-type shot\'s target vector T = RootComponent location + (0, 0, 100) (fStack_80 + 100.0)' });

const OV = readJson(P('build', 'sora_pack', 'keyblades', 'sora_forms_manifest.json'));
const MAIN = readJson(P('build', 'sora_pack', 'manifest.json'));
const ITEMS = new Map([...MAIN.items, ...OV.items].filter((i) => i.kind === 'animation').map((i) => [i.animation_name, i]));
const item = (clip) => ITEMS.get(clip) || null;
const setName = (a) => String(a).split('/').pop().split('.')[0];
const clipOf = (f, logical) => { let out = null; for (const set of f.anim_sets.map(setName)) for (const a of OV.logical_animations.filter((x) => setName(x.anim_set) === set && x.name === logical)) out = a.clip; return out; };
const TURN = ['MagicTurn', 'MagicAirTurn', 'MagicSpin', 'MagicAirSpin'];

// ------------------------------------------------------------------------------------------------ FK (as tools/sora_form_hammer.js)
const qmulH = (a, b) => [a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1], a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0], a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3], a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]];
const qrot = (q, v) => { const tx = 2 * (q[1] * v[2] - q[2] * v[1]), ty = 2 * (q[2] * v[0] - q[0] * v[2]), tz = 2 * (q[0] * v[1] - q[1] * v[0]); return [v[0] + q[3] * tx + (q[1] * tz - q[2] * ty), v[1] + q[3] * ty + (q[2] * tx - q[0] * tz), v[2] + q[3] * tz + (q[0] * ty - q[1] * tx)]; };
const under = (c, p) => { const v = qrot(p.q, [p.s[0] * c.t[0], p.s[1] * c.t[1], p.s[2] * c.t[2]]); return { q: qmulH(p.q, c.q), t: [v[0] + p.t[0], v[1] + p.t[1], v[2] + p.t[2]], s: [p.s[0] * c.s[0], p.s[1] * c.s[1], p.s[2] * c.s[2]] }; };
const point = (T, v) => { const r = qrot(T.q, [T.s[0] * v[0], T.s[1] * v[1], T.s[2] * v[2]]); return [r[0] + T.t[0], r[1] + T.t[1], r[2] + T.t[2]]; };
function rotQ(r) {
  const h = Math.PI / 360, SP = Math.sin(r.pitch * h), CP = Math.cos(r.pitch * h), SY = Math.sin(r.yaw * h), CY = Math.cos(r.yaw * h), SR = Math.sin(r.roll * h), CR = Math.cos(r.roll * h);
  return [CR * SP * SY - SR * CP * CY, -CR * SP * CY - SR * CP * SY, CR * CP * SY - SR * SP * CY, CR * CP * CY + SR * SP * SY];
}
const ID = MAIN.identification;
const SORA_BONES = sp.buildSkeleton(sp.readGlb(P('assets_converted', 'meshes', sp.CONTENT, sp.gamePathToContent(ID.skeletal_mesh) + '.glb')).json);
const SORA_SOCKETS = (() => {
  const out = {};
  const read = (d) => { for (const e of d.exports.filter((x) => x.class === 'SkeletalMeshSocket')) { const p = sp.propsOf(e); const loc = p.RelativeLocation ? p.RelativeLocation.value : { x: 0, y: 0, z: 0 }, rot = p.RelativeRotation ? p.RelativeRotation.value : { pitch: 0, yaw: 0, roll: 0 }, sc = p.RelativeScale ? p.RelativeScale.value : { x: 1, y: 1, z: 1 }; if (!out[p.SocketName.value]) out[p.SocketName.value] = { bone: p.BoneName.value, T: { q: rotQ(rot), t: [loc.x, loc.y, loc.z], s: [sc.x, sc.y, sc.z] } }; } };
  read(sp.loadPkg(sp.gamePathToContent(ID.skeleton)));
  try { read(sp.loadPkg(sp.gamePathToContent(ID.skeletal_mesh))); } catch (e) { /* the skeleton's sockets suffice */ }
  return out;
})();
const yaw = ID.mesh_component.relative_rotation_ue_deg.yaw * Math.PI / 180, cy = Math.cos(yaw), sy = Math.sin(yaw);
const actor = (v) => [v[0] * cy - v[1] * sy, v[0] * sy + v[1] * cy, v[2]];
const local = (b, tr, time) => ({ q: tr.rotation ? sp.sampleChan(tr.rotation, time)(tr.rotation.ue_values) : b.ue.rotation, t: tr.translation ? sp.sampleChan(tr.translation, time)(tr.translation.ue_values) : b.ue.translation, s: tr.scale ? sp.sampleChan(tr.scale, time)(tr.scale.ue_values) : b.ue.scale });
const byBone = (a) => { const m = new Map(); if (a) for (const c of a.chans) { if (!m.has(c.bone)) m.set(c.bone, {}); m.get(c.bone)[c.path] = c; } return m; };
function boneT(bones, chans, k, time, rootMotion) {
  const chain = []; for (let i = k; i >= 0; i = bones[i].parent) chain.unshift(i);
  let T = null;
  for (const i of chain) { const l = local(bones[i], (i === 0 && rootMotion) ? {} : (chans.get(i) || {}), time); T = T ? under(l, T) : l; }
  return T;
}
const soraAnims = new Map();
function soraAnim(clip) {
  if (soraAnims.has(clip)) return soraAnims.get(clip);
  const it = item(clip);
  let a = null;
  if (it && it.source_asset) { try { a = sp.loadAnimation(it.source_asset.includes('.') ? it.source_asset : it.source_asset + '.' + path.basename(it.source_asset), SORA_BONES); } catch (e) { a = null; } }
  soraAnims.set(clip, a);
  return a;
}
const weapons = new Map();
function weapon(model) {
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
  w.anims.set(key, { a, seq: c ? c.sequence : '-' });
  return w.anims.get(key);
}
function weaponSocketT(part, socketName, logical, clip, time) {
  const w = weapon(part.model);
  if (!w) return null;
  const sa = soraAnim(clip);
  if (!sa) return null;
  const sock = SORA_SOCKETS[part.socket];
  if (!sock) return null;
  const hand = SORA_BONES.findIndex((b) => b.name === sock.bone);
  const H = boneT(SORA_BONES, byBone(sa), hand, Math.min(time, sa.length), sa.props.bEnableRootMotion === true);
  const base = under(sock.T, H);
  const { a } = weaponAnim(w, logical, clip);
  const wt = a ? Math.min(time, a.length) : 0;
  const ws = w.sockets[socketName];
  const k = w.bones.findIndex((b) => b.name === (ws ? ws.bone : socketName));
  if (k < 0) return null;
  let T = under(boneT(w.bones, byBone(a), k, wt, false), base);
  if (ws) T = under({ q: rotQ({ pitch: ws.local_rotation[0], yaw: ws.local_rotation[1], roll: ws.local_rotation[2] }), t: ws.local_location, s: [1, 1, 1] }, T);
  return T;
}
const partsOf = (f) => f.units.map((u) => { const m = u.match(/^(\w+)@(\w+):(\w+)=(.*)$/); must(m, 'unit ' + u); return { unit: m[1], hand: m[2], socket: m[3] === 'None' ? 'R_wep_skt' : m[3], blueprint: m[4], model: m[4].split('/').pop().replace(/_[Ww]ep$/, '') }; });

let tracked = 0;
for (const f of FORMS) {
  const code = f.id.slice(5);
  const part = partsOf(f)[0];
  for (const logical of TURN) {
    const clip = clipOf(f, logical);
    must(clip, code + ': no ' + logical + ' in ' + f.anim_sets.map(setName));
    const it = item(clip);
    must(it, code + ' ' + clip + ': no animation item');
    const shots = (it.notifies || []).filter((n) => n.class === 'TresAnimNotify_Shot');
    const change = (it.notifies || []).find((n) => n.class === 'TresAnimNotify_EnableChange');
    must(shots.length === 1, code + ' ' + clip + ': one Shot notify expected, found ' + shots.length);
    const sp0 = shots[0].properties || {};
    const attach = !!(sp0.m_bAttachFireEffect && (sp0.m_bAttachFireEffect.value !== undefined ? sp0.m_bAttachFireEffect.value : sp0.m_bAttachFireEffect));
    const loc = sp0.LocationIndex ? (sp0.LocationIndex.value !== undefined ? sp0.LocationIndex.value : sp0.LocationIndex) : 0;
    add({ id: 'anim.' + code + '.' + logical, what: 'anim', form: code, logical, clip, length: r4(it.duration_s), flags: [attach ? 'attach_fire_effect' : '-', 'location_index=' + loc],
      values: [r4(shots[0].trigger_time), change ? r4(change.trigger_time) : -1], ref_addr: '0x1413794B0',
      note: 'the clip the turn-type cast plays in the form (logical -> the form set\'s row); values = [TresAnimNotify_Shot time, TresAnimNotify_EnableChange time] s; flags = [m_bAttachFireEffect, LocationIndex]' });
    const values = [];
    for (let k = 0; k < it.num_frames; k++) {
      const t = it.num_frames > 1 ? k * it.duration_s / (it.num_frames - 1) : 0;
      const T = weaponSocketT(part, 'ATK_01', logical, clip, t);
      must(T, code + ' ' + clip + ': no ATK_01 on ' + part.model);
      values.push(...actor(point(T, [0, 0, 0])).map(r1));
    }
    add({ id: 'muzzle.' + code + '.' + clip, what: 'muzzle', form: code, code: it.num_frames, logical: 'ATK_01', clip, length: r4(it.duration_s), flags: [part.model, part.socket], values, ref_addr: '0x1405AA790',
      note: 'per frame [x, y, z] (cm, actor space, his yaw 0): weapon[0]\'s ATK_01 (MuzzleAttachPoint) under the clip, where the turn-type shot spawns (pawn vt+0x1410(0)); flags = [the weapon model, Sora\'s socket it hangs on]' });
    tracked += 1;
  }
}

// ------------------------------------------------------------------------------------------------ sheet
const S = (n, t, g, doc) => [n, t, g, doc];
const COLS = [
  S('id', 'str', 'str', '<what>.<key>'), S('what', 'enum:set|actor|form|anim|muzzle|code', 'str', 'the row kind'), S('form', 'str', 'str', 'the form code (HM, CL, CL2, SK, FL, HM2) or -'),
  S('code', 'int', 'i32', 'the command (set, form), the tier (actor), frames (muzzle), else 0'), S('logical', 'str', 'str', 'the unit key (set), the element (actor), the form command (form), the logical clip name (anim), ATK_01 (muzzle)'),
  S('clip', 'str', 'str', 'the CL class (set, actor), the magic set (form), the animation-table row (anim, muzzle)'), S('length', 'num', 'f32', 'the clip length, s'),
  S('flags', 'list', 'str[]', 'strings by kind (see the note)'), S('rows', 'list', 'str[]', 'the attack rows (actor)'), S('effects', 'list', 'str[]', 'the effects (actor)'), S('sounds', 'list', 'str[]', 'the sounds (actor)'),
  S('values', 'list', 'f32[]', 'numbers by kind (see the note)'), S('ref_addr', 'str', 'str', 'decompiled function'), S('ref_conf', 'enum:confirmed|inferred|guess', 'str', 'confirmed = read in the code or the data; inferred; guess'), S('note', 'str', 'str', '-'),
];
const NAME = 'sora-hm-magic';
const rows = ROWS.map((r) => COLS.map((c) => r[c[0]]));
for (const [i, r] of rows.entries()) for (const [k, v] of r.entries()) must(v !== undefined && v !== null && !(typeof v === 'number' && !Number.isFinite(v)), 'row ' + ROWS[i].id + ': cell ' + COLS[k][0] + ' is ' + v);
const ids = new Set(); for (const r of ROWS) { must(!ids.has(r.id), 'duplicate id ' + r.id); ids.add(r.id); }
fs.writeFileSync(P('sheets', 'port', NAME + '.json'), '{"sheet":"' + NAME + '","group":"port","columns":' + JSON.stringify(COLS.map((c) => c[0])) + ',"rows":[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
const by = {}; for (const r of ROWS) by[r.what] = (by[r.what] || 0) + 1;
console.log('sheets/port/' + NAME + '.json: ' + rows.length + ' rows ' + JSON.stringify(by) + ', ' + tracked + ' muzzle tracks');

// ------------------------------------------------------------------------------------------------ register (this tool's entry only; replaced in place)
const q = (s) => "'" + String(s).replace(/\\/g, '\\\\').replace(/'/g, "\\'") + "'";
const NL = '\n';
const DOC = 'KH3\'s turn-type magic set p_ex001_hm_MagicSet as data (kh-core magic/hm_set.rs, states/fire_magic.rs, magic/projectile.rs): its units, the TresProj_Mag*CL actors, the formchanges that install it, their MagicTurn / MagicSpin clips and the form weapon\'s ATK_01 over them.';
const MARK = '(HM-MAGIC)';
function upsert(s, name, text, end, pos) {
  const at = s.indexOf("'" + name + "': {");
  if (at < 0) { const p = pos(s); return s.slice(0, p) + text + s.slice(p); }
  const c = s.lastIndexOf(NL, at - 1) + 1, cs = s.lastIndexOf(NL, c - 2) + 1;
  must(s.slice(cs, c).includes(MARK), 'the entry of ' + name + ' is not this tool\'s (no "' + MARK + '" comment above it)');
  const e = s.indexOf(end, at); must(e > at, 'end of the entry of ' + name);
  return s.slice(0, cs) + text + s.slice(e + end.length);
}
const save = (f, before, after, what) => { if (after !== before) { fs.writeFileSync(f, after); console.log('registered ' + NAME + ' in ' + what); } };
function portEntry() {
  const cols = COLS.filter((c) => c[2]).map((c) => c[0] + ': { type: ' + q(c[2]) + (c[3] && c[3] !== '-' ? ', doc: ' + q(c[3]) : '') + ' }').join(', ');
  return '    // ' + MARK + ' ' + NAME + ': written by tools/sora_hm_magic.js' + NL +
    '    ' + q(NAME) + ': {' + NL +
    "      file: 'sora_hm_magic.rs', row: 'HmMagicRow', table: 'HM_MAGIC', key: 'id', key_prefix: 'HMM'," + NL +
    '      doc: ' + q(DOC) + ',' + NL +
    '      columns: { ' + cols + ' },' + NL + '    },' + NL;
}
if (REGISTER) {
  {
    const f = P('tools', 'lib', 'sheet_defs.js'); const before = fs.readFileSync(f, 'utf8');
    const END = '] },' + NL;
    const pos = (t) => { const at = t.indexOf("  'sora-keyblade-forms': {"); must(at >= 0, 'sheet_defs.js: anchor sora-keyblade-forms not found'); const e = t.indexOf(END, at); return e + END.length; };
    const text = '  // ' + MARK + ' ' + DOC + ' Written by tools/sora_hm_magic.js' + NL +
      '  ' + q(NAME) + ": { group: 'port', file: " + q(NAME + '.json') + ", by: 'tools/sora_hm_magic.js', from: " + q('Blueprints/Player/p_ex001/p_ex001_hm_MagicSet, the TresProj_Mag*CL blueprints, sora-keyblade-forms, the form anim sets and weapons of the asset pack, Sora\'s skeleton, sora-magic-spells and the decompiled FireMagic Tick / OnShot / 0x141380470 / 0x14136F320, UTresEquipmentComponent 0x141780ED0') + ', external: true, cols: [' + NL +
      '    ' + COLS.map((c, i) => '[' + q(c[0]) + ', ' + (i === 0 ? 'K' : 'V') + ', ' + q(c[1]) + ", '-', " + q(c[3]) + (['note', 'flags', 'rows', 'effects', 'sounds', 'values', 'logical', 'clip'].includes(c[0]) ? ', true' : '') + ']').join(', ') + END;
    save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/sheet_defs.js');
  }
  {
    const f = P('tools', 'lib', 'port_gen_defs.js'); const before = fs.readFileSync(f, 'utf8');
    const END = NL + '    },' + NL;
    const pos = (t) => { const anchor = '  tables: {' + NL; const at = t.indexOf(anchor); must(at >= 0, 'port_gen_defs.js: anchor tables'); return at + anchor.length; };
    save(f, before, upsert(before, NAME, portEntry(), END, pos), 'tools/lib/port_gen_defs.js');
  }
}
console.log('sora_hm_magic ' + VERSION);
