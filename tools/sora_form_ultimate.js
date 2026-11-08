// (weapons F-ULTIMATE) KH3's Ultimate Form (form.SW, CHANGE_SW 0x87, Ultima Weapon) as DATA for its form package (kh-core
// combat/forms/ultimate.rs, states/combo_sw.rs): the attack kinds of COMBO_SW (PlayAttack cases 0xC7 .. 0xCC), the guard
// counter of the form (kind 0x25, BarrierCracker, with the BARRIERCRACKER projectile TresProjSW_BladeCounter), the
// replacement anim set (AnimSet9: logical name -> clip), the form's Reflec guard (Guard slot 119), the finish command
// FINISH_SW (StyleFinish slot 151, M_9419), the form weapons' collision shapes (w_so192 in his hand, the transform unit
// w_so191), their SHAPE TRACKS (where each hit shape is per frame of each clip: FK of Sora's skeleton through the weapon's
// socket and the weapon's own skeleton, with the shapes' growth over the window), the projectiles of w_so190_ProjSet, the
// StartStyle tail (the soft-target range) and the attack rows the form hits with.
// documentation/KH3_WEAPONS_PLAN.md 1.3 / 4, build/sora_tmp/weapons/F-ULTIMATE/STATE.md.
//
//   node tools/sora_form_ultimate.js   write sheets/port/sora-form-ultimate.json and register it (once, in place) in
//                                      tools/lib/sheet_defs.js and tools/lib/port_gen_defs.js; then node tools/gen_port.js
//
// Inputs (read only): sheets/port/sora-keyblade-forms.json (P3), build/sora_pack/keyblades/sora_forms_manifest.json (AnimSet9),
// build/sora_pack/manifest.json + xsi_manifest.json (Sora's identification, the base set, the XS overlay's M_9419),
// build/sora_pack/keyblades/w_so192/{model,sockets}.json (the weapon in his hand), assets_converted (Sora's and the weapon's
// skeletons and clips, Sora's skeleton sockets, the blueprints' TresAtkColl0 / projectile set), sheets/entities/DataTable/
// Player/Attack/p_ex001_AttackData.json and THE CODE TABLES BELOW, read in the decompiled exe (re/decomp):
//   CLASS    pawn slot 1337 0x141432F00: form 0x87 -> COMBO_SW (82, vtable 0x146912370); it overrides only slot 122 Decide
//            0x141363810 (sheets/exe/vfuncs.json); Begin / Tick / End are the Attack class's (0x141333480 / 0x1413417E0 /
//            0x141335DF0 as sheets/port/sora-states.json lists them).
//   MAX      MaxCombo 0x141433570 case 0x87: 6; Finishers 0x141433A60 case 0x87: 3.
//   PLAY     TresPlayerState_Attack::PlayAttack 0x1413378B0 cases 199 .. 0xCC (lines 1628 .. 1663 of the decompiled C) and
//            case 0x25 (BarrierCracker), its head (+0x1E1: the teleport) and its tail LAB_14133A87A.
//   WARP     slot 117 0x14134AA80: +0x258 counts down, then slot 136 (150, 100) and PlayAttack again; Attack Tick
//            0x141341340: +0x25C (0.5 s after the teleport) then the invincibility off.
//   COUNTER  TryAttack 0x141437330 (0x141437E1A): out of GUARD with the counter window open, forms 0x86 / 0x87 / 0x8E / 0x8F
//            / 0x92 / 0x93 / 0xDD .. 0xE0 force kind 0x25 (the others 0x21 / 0x23 / 0x24); the shot handler 0x141344580 case
//            0x25: the held weapon's projectile BARRIERCRACKER at vt+0x1410 (the weapon's ATK_01).
//   GUARD    TresPlayerState_Guard Begin 0x1413851D0 (table 0x141385898: weapon forms 8, 9, 16, 17, 20 .. 23 -> slot 119) and
//            slot 119 0x14139F770 (Reflec / AirReflec, ReflecHit / AirReflecHit; effect group 0xD48); CanGuard 0x1414294C0
//            (only weapon form 1 needs REFLECT_GUARD; forms 2, 4, 5, 7, 10, 12, 18 cannot guard).
//   FINISH   StyleFinish Begin 0x1413CBF70 case 0xA1 binds 0x1404EFAA4 = slot 151 0x1413E06B0 (M_9419).
//   START    StartStyle 0x14141A9D0 case 0x87 (and the slot resume 0x14141E2B0): ctrl+0x970 (the soft-target range,
//            pawn MyAutoLockonLimit 900) x 2.0.
'use strict';
const fs = require('fs');
const path = require('path');
const sp = require('./sora_pack.js');
const ROOT = path.resolve(__dirname, '..');
const VERSION = '1.0.0';
const P = (...a) => path.join(ROOT, ...a);
const readJson = (f) => JSON.parse(fs.readFileSync(f, 'utf8'));
const must = (c, m) => { if (!c) throw new Error(m); };
const hex = (n) => '0x' + n.toString(16).toUpperCase();
const r4 = (x) => Math.round(x * 10000) / 10000;
const r1 = (x) => Math.round(x * 10) / 10;

// ------------------------------------------------------------------------------------------------ inputs
const sheet = (name) => { const s = readJson(P('sheets', 'port', name + '.json')); return s.rows.map((r) => Object.fromEntries(s.columns.map((c, i) => [c, r[i]]))); };
const SW = sheet('sora-keyblade-forms').find((r) => r.id === 'form.SW');
must(SW, 'sora-keyblade-forms has no form.SW');
const OV = readJson(P('build', 'sora_pack', 'keyblades', 'sora_forms_manifest.json'));
const MAIN = readJson(P('build', 'sora_pack', 'manifest.json'));
const XSI = (() => { for (const f of [P('build', 'sora_pack', 'xsi_manifest.json'), P('build', 'sora_tmp', 'xsi', 'pack', 'xsi_manifest.json')]) if (fs.existsSync(f)) return readJson(f); return { items: [], logical_animations: [] }; })();
const ATK = readJson(P('sheets', 'entities', 'DataTable', 'Player', 'Attack', 'p_ex001_AttackData.json')).rows;
const setName = (a) => String(a).split('/').pop().split('.')[0];
const ITEMS = new Map([...MAIN.items, ...XSI.items, ...OV.items].filter((i) => i.kind === 'animation').map((i) => [i.animation_name, i]));
const BASE = new Map(MAIN.logical_animations.map((a) => [a.name, a]));
const XSI_ROWS = new Map(XSI.logical_animations.map((a) => [a.name, a]));
const SET = SW.anim_sets.map(setName);
must(SET.length === 1 && SET[0] === 'p_ex001_AnimSet9', 'form.SW: expected the anim set p_ex001_AnimSet9, got ' + SET.join(','));
const ROWS_OF = new Map();
for (const a of OV.logical_animations.filter((x) => setName(x.anim_set) === SET[0])) ROWS_OF.set(a.name, { clip: a.clip, file: a.file, cls: a.class });
// the clip (sequence) a logical name plays as in the form: the set's row, else the XS overlay's (M_9419), else the base set's
function clipOf(logical) {
  const r = ROWS_OF.get(logical);
  if (r && r.file) return r.clip;
  if (XSI_ROWS.has(logical) && XSI_ROWS.get(logical).file) return path.basename(XSI_ROWS.get(logical).file, '.glb');
  const b = BASE.get(logical);
  return b && b.file ? path.basename(b.file, '.glb') : '-';
}
// the animation-table row a logical name plays as: form-set clips are rows named by their sequence (tools/gen_port.js
// FORM_ANIM_SETS), the XS overlay's and the base set's rows by their logical name.
function rowOf(logical) {
  const r = ROWS_OF.get(logical);
  if (r && r.file) return r.clip;
  if (XSI_ROWS.has(logical) || BASE.has(logical)) return logical;
  return '-';
}
const item = (clip) => ITEMS.get(clip) || null;
const prop = (n, key) => { const p = n.properties || {}; const v = p[key]; return v && typeof v === 'object' && 'value' in v ? v.value : v; };
const atkRow = (id) => ATK.find((r) => r._row_name === id || String(r.id).toLowerCase() === String(id).toLowerCase());

// ------------------------------------------------------------------------------------------------ the code tables
// PLAY: PlayAttack 0x1413378B0 (decompiled C). Before the switch: m_bFaceAt; +0x234 = (+0x234 & ~6) | 1 (the XY channel on, the
// root-scale channel off); stop distance 160; z gain 3; +0x1DF (air mode) 0; +0x198 (the play's effect group) 0. A case that
// BREAKS reaches `+0x1DF = 1`; `goto switchD_141337e0d_caseD_26` jumps past it. LAB_1413393CF: +0x234 |= 8 (no Z traction in the
// window), +0x23C |= 0x10 (TargetMetrics keeps DirZ), +0x1DE = +0x1DF = 1 (a dive in air mode), +0x1D4 (DirZ) = -4000.
// +0x1F4 |= 0x10: the facing is kept (attack.rs `keep_face`). +0x234 bit 16 (0x10000) with the root channel: no reader of it in
// the Attack class's traction was read (attack.rs head: "bits 15 / 16 of the root-scale channel ... never set"): recorded only.
const PLAY = [
  { kind: 0xC7, logical: 'M_9411', root: 1250, chan_root: true, air: true, flags: ['keep_face', 'root_bit16'], at: '0x141339560', label: 'combo 1 (Decide: combo 0 / an unknown previous kind) - the blade flurry, its SW_Rengeki window hits every 0.1 s' },
  { kind: 0xC8, logical: 'M_9412', root: 950, chan_root: true, air: true, flags: ['keep_face', 'root_bit16'], at: '0x141339594', label: 'combo 2 (after 0xC7) - the stabs and the crossing blades (SW_StabCross)' },
  { kind: 0xC9, logical: 'M_9413', chan_xy: false, air: true, flags: ['keep_face'], at: '0x1413395C8', label: 'combo 3 (after 0xC8) - the blade raid (SW_Raid / SW_LRaid, then SW_RaidEnd); the XY channel off (+0x234 &= ~1)' },
  { kind: 0xCA, logical: 'M_9416', air: true, flags: ['keep_face', 'dive', 'no_vz', 'keep_dir_z'], dir_z: -4000, at: '0x1413395E5', label: 'combo 4 (after 0xC9; Decide clears the teleport) - the dive (SW_Dive): LAB_1413393CF' },
  { kind: 0xCB, logical: 'M_9417', root: 950, chan_root: true, air: true, flags: ['keep_face'], at: '0x1413395EC', label: 'combo 5 (after 0xCA; Decide clears the teleport) - the finisher (SW_Finish)' },
  { kind: 0xCC, logical: 'M_9418', air: true, flags: ['keep_face', 'dive', 'no_vz', 'keep_dir_z'], dir_z: -4000, at: '0x14133961B', label: 'combo 6 (after 0xCB) - the last finisher (SW_Finish2): Decide gives it the finish end (pawn vt+0x2740(1), +0x141)' },
  // case 0x25 (0x141338B7D): not forms 0xDF / 0xE0: "BarrierCracker", in the air (+0x142) "AirBarrierCracker" with air mode;
  // +0x198 = 0xD48 (0x834 in 0xDD, 0x898 in 0xDE); +0x19C = 0x34 (the voice group number of the play: anim vt+0x1F0 after Play);
  // +0x164 = 1 (the swing counts as started); +0x234 |= 0x2800; pUStackX_10 = 1 -> the tail's pawn vt+0x2068(1) (invincible).
  { kind: 0x25, logical: 'BarrierCracker', air: false, group: 0xD48, voice: 0x34, flags: ['swing_started', 'invincible', 'counter'], at: '0x141338B7D', label: 'the guard counter of the form (TryAttack 0x141437E1A: forced kind 0x25 out of GUARD): its Shot notify fires BARRIERCRACKER (TresProjSW_BladeCounter)' },
  { kind: 0x25, logical: 'AirBarrierCracker', air: true, group: 0xD48, voice: 0x34, flags: ['swing_started', 'invincible', 'counter', 'air_variant'], at: '0x141338B9B', label: 'the guard counter in the air (+0x142): air mode' },
];
// DECIDE: ComboSW Decide 0x141363810: the kind chain, the reach and the teleport (+0x1E1, +0x25C = 0.5, +0x22C = 15).
const DECIDE = { chain: [0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC], far: 400, below: -100, invincible: 0.5, vanish_frames: 15, max_combo: 6, finishers: 3 };
// FINISH: StyleFinish slot 151 0x1413E06B0 (FINISH_SW 0xA1), by sub-step.
const FINISH = [
  { sub: 0, logical: 'M_9419', what: 'SetMovementMode(5 Flying, 1); Play("M_9419", blend -1, mode 2) + voice group 0; m_bFaceAt; sub 4' },
  { sub: 4, logical: '-', what: 'once a hit window of M_9419 opened (+0x164): the world slow off (0x14140E500(ctrl, 0)), m_bFaceAt, sub 5' },
  { sub: 5, logical: '-', what: 'at M_9419\'s end: SetMovementMode(3 Falling, 1), gravity on (vt+0x1728(1)), vt+0x1658() (the class gravity scale), DoChain' },
];
// GUARD: slot 119 0x14139F770 (decompiled C).
const GUARD = { group: 0xD48, hit_seek: 0.1666666, player_turn: 1.0, miss_turn: 0.05, success_turn: 1.0 };

// ------------------------------------------------------------------------------------------------ the weapons' shapes
const PKG = P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content', 'Blueprints');
function loadBp(rel) {
  const d = readJson(path.join(PKG, rel + '.uasset.json'));
  const imp = (i) => (i < 0 && d.imports[-i - 1] ? d.imports[-i - 1].objectName : null);
  const flat = (props) => { const o = {}; for (const p of props || []) o[p.name] = p.type === 'StructProperty' ? (Array.isArray(p.value) ? flat(p.value) : p.value) : p.type === 'ArrayProperty' ? (p.value && p.value.items ? p.value.items.map((it) => (Array.isArray(it) ? flat(it) : it)) : p.value) : p.value; return o; };
  return { d, imp, flat };
}
function shapesOf(rel, part) {
  const { d, imp, flat } = loadBp(rel);
  const out = [];
  const v = (o) => (o ? [o.x || 0, o.y || 0, o.z || 0] : [0, 0, 0]);
  const rot = (o) => (o ? { pitch: o.pitch || 0, yaw: o.yaw || 0, roll: o.roll || 0 } : { pitch: 0, yaw: 0, roll: 0 });
  for (const e of d.exports) {
    if (e.class !== 'TresAtkCollComponent') continue;
    const p = flat((e.data && e.data.props) || e.props);
    const hits = (p.m_HitEffects || []).map((h) => ({ group: h.m_GrpName, effect: imp(h.m_PawnHitEffect) }));
    for (const s of p.CollisionShapesSrc || []) {
      out.push({ owner: rel.split('/').pop().replace(/_[Ww]ep$/, ''), part, group: s.m_GrpName, shape: String(s.m_ShapeType).split('::').pop(), size: v(s.m_Size), inc: v(s.m_IncSize), max: v(s.m_IncMaxSize),
        scale: v(s.m_Scale), inc_scale: v(s.m_IncScale), max_scale: v(s.m_IncMaxScale), socket1: s.m_SocketName1 || 'None', socket2: s.m_SocketName2 || 'None', loc: v(s.m_RelativeLocation1), loc2: v(s.m_RelativeLocation2),
        rot: rot(s.m_RelativeRocation), row: s.m_DefaultAttackDataIDName || 'None', attach: String(s.m_AttachType1 || '').split('::').pop(), frozen: !!s.m_bDisableLocation1Attach, phys: !!s.m_bIsPhysAttackCollision, hits,
        // (F-SHAPES) what the CONVEX transform reads (Update 0x140647950 case 6 / 8): m_Mesh, m_RotAttachType, m_bAbsoluteOffset1, m_bIgnoreParentScale
        mesh: imp(s.m_Mesh) || '-', rot_attach: String(s.m_RotAttachType || 'TRES_ACRA_NORMAL').split('::').pop(), abs1: !!s.m_bAbsoluteOffset1, ignore_scale: !!s.m_bIgnoreParentScale });
    }
  }
  return out;
}
const unitOf = (u) => { const m = u.match(/^(\w+)@(\w+):(\w+)=(.*)$/); must(m, 'unit ' + u); return { unit: m[1], hand: m[2], socket: m[3] === 'None' ? 'R_wep_skt' : m[3], blueprint: m[4], model: m[4].split('/').pop().replace(/_[Ww]ep$/, '') }; };
const PARTS = SW.units.map(unitOf);
const TRANSFORM = SW.transform_units.map(unitOf);
must(PARTS.length === 1 && PARTS[0].model === 'w_so192', 'form.SW: expected one weapon part w_so192, got ' + SW.units.join(','));
const SHAPES = [...PARTS, ...TRANSFORM].flatMap((p) => shapesOf('Player/p_ex001/' + p.model + '_Wep', p));

// ------------------------------------------------------------------------------------------------ FK (as tools/sora_form_claws.js / sora_pack.js attackSockets)
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
  for (const pkg of [sp.gamePathToContent(ID.skeleton), sp.gamePathToContent(ID.skeletal_mesh)]) {
    let d; try { d = sp.loadPkg(pkg); } catch (e) { continue; }
    for (const e of d.exports.filter((x) => x.class === 'SkeletalMeshSocket')) {
      const p = sp.propsOf(e);
      const loc = p.RelativeLocation ? p.RelativeLocation.value : { x: 0, y: 0, z: 0 }, rot = p.RelativeRotation ? p.RelativeRotation.value : { pitch: 0, yaw: 0, roll: 0 }, sc = p.RelativeScale ? p.RelativeScale.value : { x: 1, y: 1, z: 1 };
      if (!out[p.SocketName.value]) out[p.SocketName.value] = { bone: p.BoneName.value, T: { q: rotQ(rot), t: [loc.x, loc.y, loc.z], s: [sc.x, sc.y, sc.z] } };
    }
  }
  return out;
})();
const yaw = ID.mesh_component.relative_rotation_ue_deg.yaw * Math.PI / 180, cy = Math.cos(yaw), sy = Math.sin(yaw);
// the mesh component's yaw only: its origin is Sora's feet, which is the port's actor location (cmc.location; combat/hit.rs)
const actorDir = (v) => [v[0] * cy - v[1] * sy, v[0] * sy + v[1] * cy, v[2]];
const actor = actorDir;
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
  if (it && it.source_asset) { try { a = sp.loadAnimation(it.source_asset + '.' + path.basename(it.source_asset), SORA_BONES); } catch (e) { a = null; } }
  soraAnims.set(clip, a);
  return a;
}
// the component-space transform of one of Sora's sockets or bones under a clip
function soraT(name, clip, time) {
  const sa = soraAnim(clip);
  if (!sa) return null;
  const sock = SORA_SOCKETS[name];
  const bone = SORA_BONES.findIndex((b) => b.name === (sock ? sock.bone : name));
  if (bone < 0) return null;
  const B = boneT(SORA_BONES, byBone(sa), bone, Math.min(time, sa.length), sa.props.bEnableRootMotion === true);
  return sock ? under(sock.T, B) : B;
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
// the weapon's own clip for Sora's logical name (its anim set's rows of the same names: w_so192 uses w_so010_AnimSet, whose
// rows are ResetPose / BackRaid / SoKcIrradiate), else none (its reference pose)
function weaponAnim(w, logical, clip) {
  const key = logical + '|' + clip;
  if (w.anims.has(key)) return w.anims.get(key);
  const c = w.mj.clips.find((x) => (x.logical_names || []).includes(logical)) || w.mj.clips.find((x) => x.sequence === clip);
  let a = null;
  if (c) { try { a = sp.loadAnimation(c.asset + '.' + c.sequence, w.bones); } catch (e) { a = null; } }
  w.anims.set(key, { a, seq: c ? c.sequence : '-' });
  return w.anims.get(key);
}
// the component-space transform of a weapon socket at Sora's clip time
function weaponSocketT(part, socketName, logical, clip, time) {
  const w = weapon(part.model);
  if (!w) return null;
  const base = soraT(part.socket, clip, time);
  if (!base) return null;
  const { a } = weaponAnim(w, logical, clip);
  const ws = w.sockets[socketName];
  const k = w.bones.findIndex((b) => b.name === (ws ? ws.bone : socketName));
  if (k < 0) return null;
  let T = under(boneT(w.bones, byBone(a), k, a ? Math.min(time, a.length) : 0, false), base);
  if (ws) T = under({ q: rotQ({ pitch: ws.local_rotation[0], yaw: ws.local_rotation[1], roll: ws.local_rotation[2] }), t: ws.local_location, s: [1, 1, 1] }, T);
  return T;
}
// the frame of a shape's socket: the weapon's own (NORMAL), Sora's (OWNER_ATTACHED_MESH)
function socketT(s, name, logical, clip, time) {
  if (s.attach === 'OWNER_ATTACHED_MESH') return soraT(name, clip, time);
  return weaponSocketT(s.part, name, logical, clip, time);
}
// FUN_140649E40 (size over time, as combat/hit.rs `Shot::radius`) and FUN_140649BF0 (scale over time, hit.rs `Convex::scale_at`)
const grow = (base, inc, max, t) => (inc !== 0 ? Math.min(base + inc * t, Math.max(max, base)) : base);
const growScale = (s, t) => { if (s.inc_scale.every((x) => x === 0)) return s.scale; return s.scale.map((b, i) => { const v = Math.max(b + s.inc_scale[i] * t, 0.01); return s.max_scale[i] > 0 ? Math.min(v, s.max_scale[i]) : v; }); };
// the shape's segment and radius at a clip time [ax, ay, az, bx, by, bz, r] (actor space, his yaw 0). `t` = the time since
// its window opened (the growth), `at` = the clip time its location is taken at (the window's start for m_bDisableLocation1Attach).
// SPHERE: socket 1 + relative location; CAPSULE2: socket 1 -> socket 2 (each with its relative location); CAPSULE1: an axis of
// half-length m_Size.Z (+ growth) along the shape's local Z turned by m_RelativeRocation in the socket's frame. (F-SHAPES) A
// CONVEX is no capsule any more: `volAt` gives its hull's transform (the group becomes `vol` rows, combat/hull.rs).
function shapeAt(s, logical, clip, at, t) {
  const T1 = socketT(s, s.socket1, logical, clip, at);
  if (!T1) return null;
  const c = actor(point(T1, s.loc));
  if (s.shape === 'SPHERE') return [...c, ...c, grow(s.size[0], s.inc[0], s.max[0], t)];
  if (s.shape === 'CAPSULE2') {
    const T2 = socketT(s, s.socket2, logical, clip, at);
    const b = T2 ? actor(point(T2, s.loc2)) : c;
    return [...c, ...b, grow(s.size[0], s.inc[0], s.max[0], t)];
  }
  if (s.shape === 'CAPSULE1') {
    const h = grow(s.size[2], s.inc[2], s.max[2], t);
    const axis = actorDir(qrot(qmulH(T1.q, rotQ(s.rot)), [0, 0, 1]));
    return [c[0] - axis[0] * h, c[1] - axis[1] * h, c[2] - axis[2] * h, c[0] + axis[0] * h, c[1] + axis[1] * h, c[2] + axis[2] * h, grow(s.size[0], s.inc[0], s.max[0], t)];
  }
  return null;
}
// (F-SHAPES) One shape of a `vol` row at a clip time: 10 numbers (combat/hull.rs `VolTrack`). A CONVEX (Update 0x140647950 case
// 6 / 8): T1 = the socket transform; location = T1.translation + (m_bAbsoluteOffset1 ? relLoc x T1.scale : T1.rotation (relLoc x
// T1.scale)); rotation = m_RotAttachType LOCAL ? relRot (a world rotation) : T1.rotation x relRot; scale = FUN_140649BF0(t) x
// T1.scale (not with m_bIgnoreParentScale) -> [x, y, z, qx, qy, qz, qw, sx, sy, sz] (actor space, his yaw 0; the rotation in the
// world for LOCAL). Location and rotation at `at` (the window's start for m_bDisableLocation1Attach), the scale at `t`. Any other
// shape: its capsule [ax, ay, az, bx, by, bz, r, 0, 0, 0].
const MESH_YAW_Q = [0, 0, Math.sin(yaw / 2), Math.cos(yaw / 2)];
function volAt(s, logical, clip, at, t) {
  if (s.shape !== 'CONVEX') { const v = shapeAt(s, logical, clip, at, t); return v ? [...v, 0, 0, 0] : null; }
  const T1 = socketT(s, s.socket1, logical, clip, at);
  if (!T1) return null;
  const sl = [s.loc[0] * T1.s[0], s.loc[1] * T1.s[1], s.loc[2] * T1.s[2]];
  const off = s.abs1 ? sl : qrot(T1.q, sl);
  const c = actor([T1.t[0] + off[0], T1.t[1] + off[1], T1.t[2] + off[2]]);
  const q = s.rot_attach === 'TRES_ACRA_LOCAL' ? rotQ(s.rot) : qmulH(MESH_YAW_Q, qmulH(T1.q, rotQ(s.rot)));
  const n = Math.hypot(...q);
  const g = growScale(s, t);
  const sc = s.ignore_scale ? g : g.map((x, i) => x * T1.s[i]);
  return [...c, ...q.map((x) => x / n), ...sc];
}
// (F-SHAPES) the flags of a shape's `vol` row: [HULL | CAPSULE, mesh, world | actor (where the rotation is), what it is]
const volFlags = (s) => [s.shape === 'CONVEX' ? 'HULL' : 'CAPSULE', s.shape === 'CONVEX' ? s.mesh : '-', s.shape === 'CONVEX' && s.rot_attach === 'TRES_ACRA_LOCAL' ? 'world' : 'actor', s.owner + ':' + s.shape + ':' + s.socket1 + '-' + s.socket2 + (s.frozen ? ':frozen' : '') + ':' + s.rot_attach];
// several shapes of one group (KH3: one record, one hit list; the port's generic window has one capsule per group [I]): one
// capsule along their mean axis through their mean centre, long enough for every end point, its radius the largest distance of
// an end point from that axis plus the largest radius (SW_StabCross: the two blades crossing at +-30 degrees).
function mergeShapes(list) {
  if (list.length === 1) return list[0];
  const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]], dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  const centre = [0, 1, 2].map((k) => list.reduce((s, v) => s + (v[k] + v[k + 3]) / 2, 0) / list.length);
  let axis = [0, 0, 0];
  for (const v of list) { let d = sub(v.slice(3, 6), v.slice(0, 3)); const n = Math.hypot(...d); if (n < 1e-6) continue; d = d.map((x) => x / n); if (dot(d, axis) < 0) d = d.map((x) => -x); axis = axis.map((x, k) => x + d[k]); }
  const n = Math.hypot(...axis);
  axis = n < 1e-6 ? [0, 0, 1] : axis.map((x) => x / n);
  let h = 0, side = 0;
  for (const p of list.flatMap((v) => [v.slice(0, 3), v.slice(3, 6)])) { const d = sub(p, centre); const along = dot(d, axis); h = Math.max(h, Math.abs(along)); side = Math.max(side, Math.hypot(...d.map((x, k) => x - axis[k] * along))); }
  const r = Math.max(...list.map((v) => v[6])) + side;
  return [centre[0] - axis[0] * h, centre[1] - axis[1] * h, centre[2] - axis[2] * h, centre[0] + axis[0] * h, centre[1] + axis[1] * h, centre[2] + axis[2] * h, r];
}

// ------------------------------------------------------------------------------------------------ notifies of a clip
function notes(clip, group) {
  const it = item(clip);
  const out = { length: it ? r4(it.duration_s) : 0, frames: it ? it.num_frames : 0, windows: [], groups: [], rows: [], change: -1, input: -1, shot: -1, effects: [], sounds: [] };
  if (!it) return out;
  for (const n of it.notifies || []) {
    if (n.class === 'TresAnimNotifyState_Attack') {
      const g = String(prop(n, 'CollisionGrpName')), row = String(prop(n, 'm_AttackDataIDName')), iv = prop(n, 'AttackInterval') || 0;
      out.windows.push(r4(n.time) + '-' + r4(n.end_time) + '@' + g + '@' + row + '@' + r4(iv));
      if (!out.groups.includes(g)) out.groups.push(g);
      if (!out.rows.includes(row)) out.rows.push(row);
    }
    if (n.class === 'TresAnimNotify_EnableChange' && out.change < 0) out.change = r4(n.time);
    if (n.class === 'TresAnimNotify_EnableInput' && out.input < 0) out.input = r4(n.time);
    if (n.class === 'TresAnimNotify_Shot' && out.shot < 0) out.shot = r4(n.time);
    if (/AttachEffect/.test(n.class)) {
      const s = JSON.stringify(n.properties || {});
      const re = /"GroupID","type":"IntProperty","value":(-?\d+)[\s\S]*?"(?:AttachEffect|AttachDatas)","type":"(?:ObjectProperty|ArrayProperty)"[^\]]*?"value":(?:\{"items":\[)?"([^"]*)"/g;
      let m; while ((m = re.exec(s))) { const g = +m[1]; if ((g === group || g < 0) && m[2] !== 'null') out.effects.push(r4(n.time) + '@' + m[2].split('.').pop()); }
    }
    if (/PlaySound|AttachSound|PlayVoice|WeaponSwingSE/.test(n.class)) { const s = JSON.stringify(n.properties || {}); const m = s.match(/\/Game\/Sound[^"]*/); const alias = s.match(/WeaponSwing::(SWING\d+)/); out.sounds.push(r4(n.time) + '@' + (m ? m[0].split('.').pop() : alias ? alias[1] : n.class.replace(/^.*Notify(State)?_/, ''))); }
  }
  out.effects = [...new Set(out.effects)];
  return out;
}

// ------------------------------------------------------------------------------------------------ rows
const ROWS = [];
const blank = { form: 'SW', code: 0, logical: '-', clip: '-', length: 0, group: 0, root_len: 0, stop_dist: 0, z_gain: 0, air_mode: false, chan_xy: true, chan_root: false, flags: [], windows: [], rows: [], effects: [], sounds: [], values: [], ref_addr: '-', ref_conf: 'confirmed', note: '-' };
const add = (r) => ROWS.push({ ...blank, ...r });
const tracksWanted = []; // [logical, clip]
// the attacks
for (const k of PLAY) {
  const clip = rowOf(k.logical), seq = clipOf(k.logical);
  must(clip !== '-' && seq !== '-', k.logical + ': no clip');
  const n = notes(seq, k.group || 0);
  for (const r of n.rows) must(atkRow(r), k.logical + ': p_ex001_AttackData has no row ' + r);
  add({ id: 'attack.SW.' + hex(k.kind) + (k.flags.includes('air_variant') ? '.air' : ''), what: 'attack', code: k.kind, logical: k.logical, clip, length: n.length, group: k.group || 0, root_len: k.root || 0, stop_dist: 160, z_gain: 3, air_mode: !!k.air, chan_xy: k.chan_xy !== false, chan_root: !!k.chan_root, flags: k.flags,
    windows: n.windows, rows: n.rows, effects: n.effects, sounds: n.sounds, values: [n.change, n.input, k.dir_z || 0, k.voice || 0, n.shot], ref_addr: k.at,
    note: k.label + '; values = [TresEnableChange, TresEnableInput of the clip (-1 none), DirZ (0 = none), the voice group number of the play (0 = the kind\'s own), the Shot notify (-1 none)]; windows = start-end@group@row@AttackInterval' });
  tracksWanted.push([k.logical, seq]);
}
// ComboSW's Decide and the class's numbers
add({ id: 'decide.SW', what: 'decide', code: 82, logical: 'COMBO_SW', clip: 'TresPlayerState_ComboSW', values: [...DECIDE.chain, DECIDE.far, DECIDE.below, DECIDE.invincible, DECIDE.vanish_frames, DECIDE.max_combo, DECIDE.finishers], ref_addr: '0x141363810',
  note: 'values = [the kind chain 0xC7 .. 0xCC (by the previous kind pawn+0xE10; combo 0 or an unknown previous kind: 0xC7 / 0xC8 by combo != 0), the teleport when the target is farther than values[6] cm (or high, or he is in the air and it is more than -values[7] cm below) within the soft-target range (ctrl vt+0x1700 = ctrl+0x970), the invincibility after it values[8] s (+0x25C), the vanish frames +0x22C, MaxCombo 0x141433570, Finishers 0x141433A60]; 0xCA / 0xCB clear the teleport' });
// the replacement anim set (logical name -> clip)
for (const [logical, r] of [...ROWS_OF].sort((a, b) => (a[0] < b[0] ? -1 : 1))) {
  add({ id: 'anim.SW.' + logical, what: 'anim', logical, clip: r.file ? r.clip : '-', length: r.file && item(r.clip) ? r4(item(r.clip).duration_s) : 0, flags: [SET[0], r.cls], note: r.file ? '-' : 'a ' + r.cls + ' of the set: not a row of the animation table (' + (r.cls === 'BlendSpace1D' ? 'its samples are' : 'the XS overlay\'s row of the same name plays') + ')' });
}
// the finish command
for (const f of FINISH) {
  const clip = f.logical === '-' ? '-' : rowOf(f.logical);
  const n = f.logical !== '-' ? notes(clipOf(f.logical), 0) : null;
  add({ id: 'finish.SW.' + f.sub, what: 'finish', code: f.sub, logical: f.logical, clip, length: n ? n.length : 0, windows: n ? n.windows : [], rows: n ? n.rows : [], effects: n ? n.effects : [], sounds: n ? n.sounds : [], values: n ? [n.change, n.input] : [], ref_addr: '0x1413E06B0', note: f.what });
  if (f.logical !== '-') tracksWanted.push([f.logical, clipOf(f.logical)]);
}
// the transformation (StyleChange9: the framework plays it; its ChangeSW window is the transform unit w_so191's)
{
  const seq = clipOf(SW.transform_clip);
  const n = notes(seq, 0);
  add({ id: 'change.SW', what: 'change', logical: SW.transform_clip, clip: rowOf(SW.transform_clip), length: n.length, windows: n.windows, rows: n.rows, effects: n.effects, sounds: n.sounds, values: [n.change, n.input], ref_addr: '0x1413D3AC0', note: 'StyleChange Tick case 0x87: the transformation clip; its window ChangeSW (row SCSW_CHANGE) is a shape of the transform unit ' + TRANSFORM.map((t) => t.model).join(', ') });
  tracksWanted.push([SW.transform_clip, seq]);
}
// the guard (slot 119)
for (const [logical, air] of [['Reflec', false], ['AirReflec', true], ['ReflecHit', false], ['AirReflecHit', true]]) {
  const seq = clipOf(logical);
  const n = seq !== '-' ? notes(seq, GUARD.group) : null;
  add({ id: 'guard.SW.' + logical, what: 'guard', code: GUARD.group, logical, clip: rowOf(logical), length: n ? n.length : 0, air_mode: air, windows: n ? n.windows : [], effects: n ? n.effects : [], sounds: n ? n.sounds : [], values: n ? [n.change, n.input, GUARD.hit_seek, GUARD.player_turn, GUARD.miss_turn] : [-1, -1, GUARD.hit_seek, GUARD.player_turn, GUARD.miss_turn], ref_addr: '0x14139F770',
    note: 'Guard slot 119 (weapon forms 8, 9, 16, 17, 20 .. 23: Begin 0x1413851D0 table 0x141385898): gravity off; a player guard plays Reflec / AirReflec (blend -1, mode 2, effect group code) with turn rate values[3]; a hit entry plays it from values[2] s (anim vt+0xF0), turn timer / rate values[4] without success ([ReflecHit / AirReflecHit only with Guard flag 0x400, never set for these forms]); then at the clip\'s end DoChain, a queued JUMP with m_bEnableChange, a queued DODGEROLL / AIR_SLIDE at once' + (seq === '-' ? '; NOT IN THE FORM\'S SET (clip -: the base row of the name, if any)' : '') });
}
// the collision shapes
for (const [i, s] of SHAPES.entries()) {
  add({ id: 'shape.SW.' + s.owner + '.' + s.group + '.' + (SHAPES.slice(0, i).filter((x) => x.owner === s.owner && x.group === s.group).length + 1), what: 'shape', logical: s.group, clip: s.owner, flags: [s.shape, s.socket1, s.socket2, s.attach, s.part.socket, s.frozen ? 'frozen' : '-', s.phys ? 'phys' : '-'], values: [...s.size, ...s.inc, ...s.max, ...s.loc, ...s.scale, ...s.inc_scale, ...s.max_scale, s.rot.pitch, s.rot.yaw, s.rot.roll], rows: [s.row],
    note: 'TresAtkColl0 CollisionShapesSrc of Player/p_ex001/' + s.owner + '_Wep: flags = [shape, socket 1, socket 2, attach type, Sora\'s socket the part hangs on, m_bDisableLocation1Attach, m_bIsPhysAttackCollision], values = [m_Size xyz, m_IncSize xyz, m_IncMaxSize xyz, m_RelativeLocation1 xyz, m_Scale xyz, m_IncScale xyz, m_IncMaxScale xyz, m_RelativeRocation pitch / yaw / roll]' });
}
// THE SHAPE TRACKS: for every clip above and every group its windows name, the group's shapes as one capsule, per frame (time =
// frame x length / (frames - 1)) [ax, ay, az, bx, by, bz, radius] in Sora's actor space (UE cm, his yaw 0), with the growth of
// the window the frame lies in (or the last one opened before it). Keyed by the clip's SEQUENCE and the group.
let tracked = 0;
const seen = new Set();
const missing = [];
for (const [logical, seq] of tracksWanted) {
  const it = item(seq);
  if (!it) continue;
  const wins = (it.notifies || []).filter((n) => n.class === 'TresAnimNotifyState_Attack').map((n) => ({ g: String(prop(n, 'CollisionGrpName')), a: +n.time, b: +n.end_time }));
  for (const g of [...new Set(wins.map((w) => w.g))]) {
    const shapes = SHAPES.filter((s) => s.group === g);
    if (!shapes.length) { missing.push(seq + ':' + g); continue; }
    const id = 'track.SW.' + seq + '.' + g;
    if (seen.has(id)) continue;
    seen.add(id);
    const mine = wins.filter((w) => w.g === g);
    const frames = it.num_frames, len = it.duration_s;
    // (F-SHAPES) a group with a CONVEX or several shapes: one `vol` row per shape (each its own primitive in KH3, one hit list per
    // group; combat/hull.rs) instead of one merged capsule: the hull's real transform, the crossing blades apart.
    if (shapes.length > 1 || shapes.some((s) => s.shape === 'CONVEX')) {
      const rows = [];
      for (const [k, s] of shapes.entries()) {
        const vals = [];
        for (let f = 0; f < frames; f++) {
          const time = frames > 1 ? f * len / (frames - 1) : 0;
          const w = mine.filter((x) => x.a <= time + 1e-6).pop() || mine[0];
          const v = volAt(s, logical, seq, s.frozen ? w.a : time, Math.max(time - w.a, 0));
          if (!v) { vals.length = 0; break; }
          vals.push(...v.slice(0, 3).map(r1), ...v.slice(3, 7).map((x) => Math.round(x * 1e6) / 1e6), ...v.slice(7).map(r4));
        }
        if (!vals.length) break;
        rows.push({ id: 'vol.SW.' + seq + '.' + g + '.' + k, what: 'vol', code: frames, logical: g, clip: seq, length: r4(len), flags: volFlags(s), windows: mine.map((w) => r4(w.a) + '-' + r4(w.b)), values: vals, ref_addr: '0x140647950',
          note: 'shape ' + k + ' of ' + shapes.length + ' of group ' + g + ' per frame (combat/hull.rs VolTrack): flags = [HULL | CAPSULE, the CONVEX mesh, world | actor (the frame of the rotation: m_RotAttachType LOCAL = relRot in the world), owner:shape:sockets]; a HULL [x, y, z, qx, qy, qz, qw, sx, sy, sz] = Update 0x140647950 case 6\'s location / rotation / scale (FUN_140649BF0 growth from the window\'s start) in Sora\'s actor space (his yaw 0), a CAPSULE [ax, ay, az, bx, by, bz, r, 0, 0, 0]; m_bDisableLocation1Attach: location and rotation of the window\'s start' });
      }
      if (rows.length !== shapes.length) { missing.push(seq + ':' + g + ' (FK)'); continue; }
      for (const r of rows) add(r);
      tracked += 1;
      continue;
    }
    const values = [];
    let ok = true;
    for (let f = 0; f < frames; f++) {
      const time = frames > 1 ? f * len / (frames - 1) : 0;
      const w = mine.filter((x) => x.a <= time + 1e-6).pop() || mine[0];
      const t = Math.max(time - w.a, 0);
      const each = shapes.map((s) => shapeAt(s, logical, seq, s.frozen ? w.a : time, t));
      if (each.some((v) => !v)) { ok = false; break; }
      values.push(...mergeShapes(each).map(r1));
    }
    if (!ok) { missing.push(seq + ':' + g + ' (FK)'); continue; }
    add({ id, what: 'track', code: frames, logical: g, clip: seq, length: r4(len), flags: [...new Set(shapes.map((s) => s.owner + ':' + s.shape + ':' + s.socket1 + '-' + s.socket2 + (s.frozen ? ':frozen' : '')))], windows: mine.map((w) => r4(w.a) + '-' + r4(w.b)), values,
      note: 'per frame [ax, ay, az, bx, by, bz, radius] (cm, actor space, his yaw 0) of group ' + g + ' (' + shapes.length + ' shape(s), merged into one capsule; the growth m_IncSize / m_IncScale from the window\'s start; m_bDisableLocation1Attach: the location of the window\'s start)' });
    tracked += 1;
  }
}
// the shot of the guard counter: where the weapon's ATK_01 (vt+0x1410 -> MuzzleAttachPoint) is at the Shot notify
for (const k of PLAY.filter((x) => x.kind === 0x25)) {
  const seq = clipOf(k.logical);
  const n = notes(seq, 0);
  must(n.shot >= 0, k.logical + ': no Shot notify');
  const T = weaponSocketT(PARTS[0], 'ATK_01', k.logical, seq, n.shot);
  must(T, k.logical + ': no FK of ATK_01');
  add({ id: 'shot.SW.' + seq, what: 'shot', code: 0x25, logical: k.logical, clip: seq, length: n.length, values: [n.shot, ...actor(point(T, [0, 0, 0])).map(r1)], ref_addr: '0x141344580',
    note: 'the shot handler case 0x25: BARRIERCRACKER of the held weapon\'s projectile set at pawn vt+0x1410 (the weapon\'s MuzzleAttachPoint ATK_01) with his forward; values = [the Shot notify s, ATK_01 x, y, z (actor space, his yaw 0)]' });
}
// the projectiles of w_so190_ProjSet
{
  const { d, imp, flat } = loadBp('Player/p_ex001/w_so190_ProjSet');
  const set = d.exports.map((e) => flat((e.data && e.data.props) || e.props)).find((p) => p.Projectiles);
  must(set, 'w_so190_ProjSet has no Projectiles');
  for (const pr of set.Projectiles) {
    const cls = imp(pr.ProjectileData);
    const rel = 'Projectile/SW/' + String(cls).replace(/_C$/, '');
    const b = loadBp(rel);
    const actorProps = b.d.exports.map((e) => ({ cls: e.class, p: b.flat((e.data && e.data.props) || e.props) })).find((e) => /_C$/.test(e.cls));
    const shapes = shapesOf(rel, null);
    must(shapes.length === 1, rel + ': expected one attack shape');
    const s = shapes[0];
    const spawner = pr.ProjectileName === 'BARRIERCRACKER' ? 'the shot handler 0x141344580 case 0x25 (kind 0x25 in a form whose set has it)' : 'NONE FOUND: the name is in no string table of the exe (sheets/exe/strings utf16 / ascii), in no decompiled function (re/decomp), in no notify of the form\'s clips (sora_forms_manifest / xsi_manifest) and in no blueprint json (assets_converted Blueprints, Character/pc/p_ex001): not spawned';
    add({ id: 'proj.SW.' + pr.ProjectileName, what: 'proj', logical: pr.ProjectileName, clip: String(cls), flags: [s.shape, actorProps.p.m_bNoDestroyOnHit ? 'no_destroy_on_hit' : '-', actorProps.p.m_bAttachToHitActor ? 'attach_to_hit_actor' : '-', (s.hits[0] && s.hits[0].effect) || '-'], values: [actorProps.p.m_MainLife || 0, s.size[0], s.inc[0], s.max[0]], rows: [s.row], ref_addr: pr.ProjectileName === 'BARRIERCRACKER' ? '0x141344580' : '-', ref_conf: pr.ProjectileName === 'BARRIERCRACKER' ? 'confirmed' : 'inferred',
      note: 'w_so190_ProjSet ' + pr.ProjectileName + ' -> ' + rel + ': flags = [shape, m_bNoDestroyOnHit, m_bAttachToHitActor, m_HitEffects[None].m_PawnHitEffect], values = [m_MainLife s, m_Size.X, m_IncSize.X, m_IncMaxSize.X]; spawned by: ' + spawner });
  }
}
// StartStyle's tail (and the slot resume 0x14141E2B0)
add({ id: 'start.SW', what: 'start', code: 0x87, logical: 'CHANGE_SW', values: [2], flags: ['soft_range'], ref_addr: '0x14141A9D0', note: SW.start_extra + ': the soft-target range controller+0x970 (pawn+0xC88 MyAutoLockonLimit 900) x values[0]; EndStyle\'s reset 0x141410FD0 puts it back' });
// (AUDIT-C SW) THE WEAPONS' OWN CLIPS (`wnotify` rows, AUDIT-B F-SHIP's format and pass): the weapon actors of the form play the
// row of Sora's logical name on their own anim instance and its notifies fire on THEIR mesh. Ultima's transform unit w_so191
// plays its own 9401 (StyleChange9): the AttachSound se02019_011 and the effects vw_so191_chg_ora0 / trf0 / kaz0 / lst0 on its
// bones kh_sk / w_so192. (w_so190 / w_so192's own clips carry Bonamik notifies only.) Anchor `kb:<model>:<mount>:<bone>`
// (kh-host vfx/kbweapon.rs); mount = the unit's socket, R_wep_skt for a unit with none (P5 draws it there).
{
  const mounts = {};
  for (const u of [...SW.units, ...SW.transform_units]) { const m = u.match(/^(\w+)@(\w+):(\w+)=(.*)$/); mounts[m[4].split('/').pop().replace(/_[Ww]ep$/, '')] = m[3] === 'None' ? 'R_wep_skt' : m[3]; }
  mounts.w_so190 = 'R_wep_skt';
  const pv = (list) => { const o = {}; for (const p of list || []) o[p.name] = p.type === 'StructProperty' ? (Array.isArray(p.value) ? pv(p.value) : p.value) : p.type === 'ArrayProperty' ? (p.value && p.value.items ? p.value.items.map((it) => (Array.isArray(it) ? pv(it) : it)) : p.value) : p.value; return o; };
  const tail = (s) => String(s || '').split('/').pop().split('.').pop();
  for (const model of ['w_so190', 'w_so191', 'w_so192']) {
    const mj = readJson(P('build', 'sora_pack', 'keyblades', model, 'model.json'));
    for (const c of mj.clips || []) {
      let k = 0;
      for (const n of c.notifies || []) {
        const np = n.properties || {};
        const t = r4(n.trigger_time !== undefined ? n.trigger_time : n.time);
        const state = /NotifyState_/.test(n.class);
        const end = state ? r4(n.end_trigger_time !== undefined ? n.end_trigger_time : n.end_time) : -1;
        const base = { what: 'wnotify', clip: c.sequence, length: r4(c.duration_s || 0), ref_addr: 'build/sora_pack/keyblades/' + model + '/model.json', ref_conf: 'confirmed', note: '(AUDIT-C SW) the ' + model + ' clip ' + c.sequence + ': ' + n.class + '; values = [time, end (-1 instant), location x, y, z, rotation pitch, yaw, roll, fade out s]' };
        if (/AttachEffect/.test(n.class)) {
          for (const item of (np.MetaAttachDataList && np.MetaAttachDataList.items) || []) {
            const top = pv(item);
            for (const meta of top.MetaAttachData || []) {
              const m = Array.isArray(meta) ? pv(meta) : meta;
              for (const e of m.EffectDataList || []) {
                const d = Array.isArray(e) ? pv(e) : e;
                if (!d.AttachEffect || d.AttachEffect === 'null') continue;
                must(String(top.AttachType).endsWith('OWNER'), model + ' ' + c.sequence + ': attach type ' + top.AttachType + ' (only OWNER is ported)');
                const sock = m.SocketName && m.SocketName !== 'None' ? m.SocketName : top.SocketName && top.SocketName !== 'None' ? top.SocketName : '';
                const endType = String(m.AttachEffectEndType || '').replace('ESQEX_ATTACH_EFFECT_END_TYPE_', '');
                const anchor = 'kb:' + model + ':' + mounts[model] + ':' + sock + (m.bAbsoluteRotation ? ':abs' : '') + (m.ToAttach === false ? ':free' : '') + (endType === 'FADEOUT' ? ':fo' + r4(m.FadeOut || 0) : '');
                const lo = m.LocationOffset || {}, ro = m.RotationOffset || {};
                add({ ...base, id: 'wnotify.SW.' + model + '.' + c.sequence + '.' + k++, code: 0, logical: 'effect', flags: [anchor, endType, state ? 'state' : 'instant', 'g' + d.GroupID], effects: [tail(d.AttachEffect)],
                  values: [t, end, r4(lo.x || 0), r4(lo.y || 0), r4(lo.z || 0), r4(ro.pitch || 0), r4(ro.yaw || 0), r4(ro.roll || 0), endType === 'FADEOUT' ? r4(m.FadeOut || 0) : 0] });
              }
            }
          }
        } else if (/AttachSound|PlaySound/.test(n.class)) {
          const anchor = 'kb:' + model + ':' + mounts[model] + ':' + (np.AttachPointName && np.AttachPointName !== 'None' ? np.AttachPointName : '');
          add({ ...base, id: 'wnotify.SW.' + model + '.' + c.sequence + '.' + k++, code: 1, logical: 'sound', flags: [anchor, '-', state ? 'state' : 'instant'], sounds: [tail(np.Sound)], values: [t, end, 0, 0, 0, 0, 0, 0, r4(np.FadeOutDuration || 0)] });
        }
      }
    }
  }
}
// (AUDIT-C SW) THE ATTACH DATA ASSET OF THE FORM'S CLIPS: every AnimSet9 clip carries a TresAnimNotifyState_AttachEffect naming
// ead_so192_aura0 (GroupID 0, OWNER). KH3 plays the asset's rows of that group, each where ITS row attaches it
// (SQEX_StructParticleAttachData): vw_so192_foot_aura0 on Sora's L_ashi2 / R_ashi2, vw_so192_bld_aur0 on the right-hand
// weapon's vfx_bottom; LOOPEND at the window's end. One `ead` row per asset row: code = GroupID, logical = the asset, clip =
// SocketName, flags = [AttachType, end type, follow | free, abs | -], effects = [the system], values = [LocationOffset xyz,
// RotationOffset pitch yaw roll, ScaleOffset xyz, ParticleColor rgb, ParticleAlpha, FadeIn, FadeOut, StartDelayTime].
{
  const ASSETS = ['/Game/Effects/wep/w_so190/ead/ead_so192_aura0'];
  const unwrap = (v) => (v && v.items ? v.items : v && v.value && v.value.items ? v.value.items : []);
  const props = (arr) => Object.fromEntries((arr || []).map((p) => [p.name, p.value]));
  for (const asset of ASSETS) {
    const d = readJson(P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content', asset.replace(/^\/Game\//, '') + '.uasset.json'));
    const imp = (i) => d.imports[-i - 1].objectName;
    const e = d.exports.find((x) => x.class === 'SQEX_ParticleAttachDataAsset');
    const name = asset.split('/').pop();
    let k = 0;
    for (const outer of unwrap(props(e.data.props).EffectDataList)) {
      const o = props(outer);
      for (const it of unwrap(o.EffectDataList)) {
        const r = props(it);
        if (!(r.AttachEffect < 0)) continue;
        const ao = props(r.AttachObjectData || []);
        const lo = o.LocationOffset || {}, ro = o.RotationOffset || {}, so = o.ScaleOffset || {}, pc = o.ParticleColor || {};
        add({ id: 'ead.SW.' + name + '.' + k++, what: 'ead', code: r.GroupID, logical: name, clip: o.SocketName || 'None',
          flags: [String(r.UseAttachObjectData ? ao.AttachType : 'SQEX_ATTACH_OBJECT_OWNER').replace('SQEX_ATTACH_OBJECT_', ''), String(o.AttachEffectEndType || '').replace('ESQEX_ATTACH_EFFECT_END_TYPE_', ''), o.ToAttach === false ? 'free' : 'follow', o.bAbsoluteRotation ? 'abs' : '-'],
          effects: [imp(r.AttachEffect)], values: [lo.x || 0, lo.y || 0, lo.z || 0, ro.pitch || 0, ro.yaw || 0, ro.roll || 0, so.x ?? 1, so.y ?? 1, so.z ?? 1, pc.x ?? 1, pc.y ?? 1, pc.z ?? 1, o.ParticleAlpha ?? 1, o.FadeIn || 0, o.FadeOut || 0, o.StartDelayTime || 0].map(r4),
          ref_addr: asset, ref_conf: 'confirmed', note: '(AUDIT-C SW) a row of the attach data asset (SQEX_StructParticleAttachData): ' + (o.Description && o.Description.s ? 'Description "' + o.Description.s + '"' : '') });
      }
    }
    must(ROWS.filter((x) => x.what === 'ead' && x.logical === name).length === 3, name + ': expected 3 rows (two feet, the blade)');
  }
}
// (AUDIT-C SW) THE TELEPORT'S AURA GROUPS: ComboSW's Decide 0x141363810 sets +0x22C = 0xF with the teleport (+0x1E1); PlayAttack's
// head calls the anim interface's vt+0x1A0(pawn vt+0x2458() = pawn+0x8E0 m_CmnAuraEffect, +0x22C, 0) (0x1413378B0 line 110),
// slot 117 at the warp's end vt+0x1A0(.., +0x22C + 1, 0) (0x14134AA80) and End the same while the warp is still pending
// (0x141335120). Sora's m_CmnAuraEffect = ead_pc_aura_com0, groups 15 / 16 ("アルテマワープ 消失15/出現16", the Ultima warp:
// vanish / appear): ws_so192_wrp_in01 / in03 (out01 / out03) on vfx_center, ToAttach false, absolute rotation (the body), and
// ws_so192_wrp_in01 / in04 (out01 / out04) on the RIGHT-HAND WEAPON's vfx_bottom (the form blade w_so192 on R_wep_skt).
// One `aura` row per entry: code = group, flags = [the anchor (Sora's socket with :free / :abs, or kb:<model>:<mount>:<socket>)],
// values = [location, rotation offset].
{
  const ead = readJson(P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content', 'Effects', 'system', 'aura', 'ead', 'ead_pc_aura_com0.uasset.json'));
  const ref = (i) => (typeof i === 'number' && i < 0 && ead.imports[-i - 1] ? ead.imports[-i - 1].objectName : String(i));
  const pv = (list) => { const o = {}; for (const p of list || []) o[p.name] = p.type === 'StructProperty' ? (Array.isArray(p.value) ? pv(p.value) : p.value) : p.type === 'ArrayProperty' ? (p.value && p.value.items ? p.value.items.map((it) => (Array.isArray(it) ? pv(it) : it)) : p.value) : p.value; return o; };
  const unit = SW.units.find((u) => u.includes('@RIGHT_HAND:')).match(/^(\w+)@(\w+):(\w+)=(.*)$/);
  const right = 'kb:' + unit[4].split('/').pop().replace(/_[Ww]ep$/, '') + ':' + (unit[3] === 'None' ? 'R_wep_skt' : unit[3]) + ':';
  let k = 0;
  for (const e of ead.exports) {
    const props = pv((e.data && e.data.props) || e.props);
    for (const meta of props.EffectDataList || []) {
      const m = Array.isArray(meta) ? pv(meta) : meta;
      for (const d of (m.EffectDataList || []).map((x) => (Array.isArray(x) ? pv(x) : x))) {
        if (d.GroupID !== 15 && d.GroupID !== 16) continue;
        const ao = Array.isArray(d.AttachObjectData) ? pv(d.AttachObjectData) : d.AttachObjectData || {};
        const sock = m.SocketName && m.SocketName !== 'None' ? m.SocketName : '';
        const onWeapon = d.UseAttachObjectData && /RIGHT_HAND_WEAPON/.test(ao.AttachType || '');
        const anchor = onWeapon ? right + sock + (m.bAbsoluteRotation ? ':abs' : '') + (m.ToAttach === false ? ':free' : '') : sock + (m.ToAttach === false ? ':free' : '') + (m.bAbsoluteRotation ? ':abs' : '');
        const lo = m.LocationOffset || {}, ro = m.RotationOffset || {};
        add({ id: 'aura.SW.' + d.GroupID + '.' + k++, what: 'aura', code: d.GroupID, logical: d.GroupID === 15 ? 'warp_in' : 'warp_out', clip: 'ead_pc_aura_com0', flags: [anchor, String(m.AttachEffectEndType || '').replace('ESQEX_ATTACH_EFFECT_END_TYPE_', '')], effects: [ref(d.AttachEffect)],
          values: [r4(lo.x || 0), r4(lo.y || 0), r4(lo.z || 0), r4(ro.pitch || 0), r4(ro.yaw || 0), r4(ro.roll || 0)], ref_addr: '0x141363810 / 0x1413378B0 / 0x14134AA80 / 0x141335120', ref_conf: 'confirmed',
          note: '(AUDIT-C SW) the teleport: group 15 at the vanish, 16 at the reappearance (m_CmnAuraEffect ead_pc_aura_com0); flags = [anchor, end type]; values = [location, rotation offset]' });
      }
    }
  }
  must(ROWS.filter((r) => r.what === 'aura').length === 8, 'ead_pc_aura_com0: expected 4 rows each of groups 15 / 16');
}
must(ROWS.filter((r) => r.what === 'wnotify').length === 5, 'w_so191 9401: expected 5 own notifies (1 sound, 4 effects)');
// the attack rows the form hits with (p_ex001_AttackData), for the combat sheet (tools/sora_combat_sheet.js (F-ULTIMATE))
const usedRows = new Set(ROWS.flatMap((r) => (['attack', 'finish', 'change', 'proj', 'shape'].includes(r.what) ? r.rows : [])).filter((x) => x && x !== 'None'));
for (const id of [...usedRows].sort()) {
  const a = atkRow(id);
  must(a, 'p_ex001_AttackData has no row ' + id);
  add({ id: 'row.' + id, what: 'row', logical: id, values: [a.m_Power, a.m_FormPoint, a.m_ReactionPowerValue], flags: [String(a.m_DamageKind).split('::').pop(), String(a.m_DamageAttribute).split('::').pop()], note: 'p_ex001_AttackData row ' + a.id + ': values = [m_Power, m_FormPoint, m_ReactionPowerValue]' });
}

// ------------------------------------------------------------------------------------------------ checks
must(ROWS.filter((r) => r.what === 'attack').length === 8, 'expected 6 combo kinds and the two counter clips');
for (const r of ROWS.filter((x) => x.what === 'attack')) must(r.clip !== '-' && r.length > 0, r.id + ': no clip');
must(missing.length === 0, 'groups without a shape track: ' + missing.join(', '));
for (const g of ['SW_StickL', 'SW_Crack', 'SW_Rengeki', 'SW_StabCross', 'SW_Raid', 'SW_LRaid', 'SW_RaidEnd', 'SW_Dive', 'SW_Finish', 'SW_Finish2', 'SW_Myriad', 'SW_Myriad2', 'ChangeSW']) must(ROWS.some((r) => (r.what === 'track' || r.what === 'vol' /* (F-SHAPES) */) && r.logical === g), 'no track of ' + g);
// (F-SHAPES) the hulls and the crossing blades are `vol` rows, one per shape
for (const [g, n] of [['SW_StabCross', 2], ['SW_Finish2', 1], ['SW_Myriad2', 1], ['ChangeSW', 1]]) must(new Set(ROWS.filter((r) => r.what === 'vol' && r.logical === g).map((r) => r.id.split('.').pop())).size === n, g + ': expected ' + n + ' vol shape(s)');

// ------------------------------------------------------------------------------------------------ sheet
const S = (n, t, g, doc) => [n, t, g, doc];
const COLS = [
  S('id', 'str', 'str', '<what>.SW.<key>'), S('what', 'enum:attack|decide|anim|finish|change|guard|shape|track|vol|shot|proj|start|row|wnotify|ead|aura', 'str', 'the row kind'), S('form', 'str', 'str', 'SW (Ultimate Form, 0x87)'),
  S('code', 'int', 'i32', 'attack kind (attack, shot), the class id (decide), finish sub-step (finish), effect group (guard), frames (track), else 0'), S('logical', 'str', 'str', 'the logical anim name KH3 plays (attack / anim / finish / change / guard), the group (shape / track), the projectile (proj), the row (row)'),
  S('clip', 'str', 'str', 'the animation table row (attack / finish / change / guard: a form-set clip by its sequence, a base / XS row by its logical name); the sequence (track / shot); the owner (shape); the class (proj)'), S('length', 'num', 'f32', 'the clip length, s'),
  S('group', 'int', 'i32', 'the effect group the play passes (PlayAttack +0x198)'), S('root_len', 'num', 'f32', 'root length (+0x1D8), 0 = none'), S('stop_dist', 'num', 'f32', 'stop distance (+0x24C)'), S('z_gain', 'num', 'f32', 'z gain (+0x14C)'),
  S('air_mode', 'bool', 'bool', '+0x1DF (attack); the air variant (guard)'), S('chan_xy', 'bool', 'bool', '+0x234 bit 0'), S('chan_root', 'bool', 'bool', '+0x234 bit 1'),
  S('flags', 'list', 'str[]', 'the case\'s other writes / the kind\'s strings (see the note)'), S('windows', 'list', 'str[]', 'the clip\'s hit windows start-end@group@row@interval, s (track: the group\'s windows)'), S('rows', 'list', 'str[]', 'the attack rows of those windows'),
  S('effects', 'list', 'str[]', 'the clip\'s effects of that group (time@asset)'), S('sounds', 'list', 'str[]', 'the clip\'s sounds / voices / swing aliases (time@asset)'), S('values', 'list', 'f32[]', 'numbers by kind (see the note)'),
  S('ref_addr', 'str', 'str', 'decompiled function'), S('ref_conf', 'enum:confirmed|inferred|guess', 'str', 'confirmed = read in the code or the data; inferred; guess'), S('note', 'str', 'str', '-'),
];
const NAME = 'sora-form-ultimate';
const rows = ROWS.map((r) => COLS.map((c) => r[c[0]]));
for (const [i, r] of rows.entries()) for (const [k, v] of r.entries()) must(v !== undefined && v !== null && !(typeof v === 'number' && !Number.isFinite(v)), 'row ' + ROWS[i].id + ': cell ' + COLS[k][0] + ' is ' + v);
for (const [i, r] of ROWS.entries()) must(r.values.every((v) => typeof v === 'number' && Number.isFinite(v)), 'row ' + r.id + ' (' + i + '): a value is not a number');
fs.writeFileSync(P('sheets', 'port', NAME + '.json'), '{"sheet":"' + NAME + '","group":"port","columns":' + JSON.stringify(COLS.map((c) => c[0])) + ',"rows":[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
const by = {}; for (const r of ROWS) by[r.what] = (by[r.what] || 0) + 1;
console.log('sheets/port/' + NAME + '.json: ' + rows.length + ' rows ' + JSON.stringify(by) + ', ' + tracked + ' shape tracks');

// ------------------------------------------------------------------------------------------------ register (this tool's entry only; replaced in place)
const q = (s) => "'" + String(s).replace(/\\/g, '\\\\').replace(/'/g, "\\'") + "'";
const NL = '\n';
const DOC = 'KH3\'s Ultimate Form as data for its form package (kh-core combat/forms/ultimate.rs, states/combo_sw.rs): COMBO_SW\'s attack kinds and the guard counter, the replacement anim set, the Reflec guard, FINISH_SW, the weapon shapes and their per-frame tracks, the projectiles.';
const MARK = '(weapons F-ULTIMATE)';
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
  const text = '  // ' + MARK + ' ' + DOC + ' Written by tools/sora_form_ultimate.js (documentation/KH3_WEAPONS_PLAN.md 1.3)' + NL +
    '  ' + q(NAME) + ": { group: 'port', file: " + q(NAME + '.json') + ", by: 'tools/sora_form_ultimate.js', from: " + q('sora-keyblade-forms, the form anim sets of the asset pack, the weapon / projectile blueprints, p_ex001_AttackData and the decompiled PlayAttack / ComboSW / Guard / StyleFinish') + ', external: true, cols: [' + NL +
    '    ' + COLS.map((c, i) => '[' + q(c[0]) + ', ' + (i === 0 ? 'K' : 'V') + ', ' + q(c[1]) + ", '-', " + q(c[3]) + (['note', 'flags', 'windows', 'rows', 'effects', 'sounds', 'values', 'logical', 'clip'].includes(c[0]) ? ', true' : '') + ']').join(', ') + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/sheet_defs.js');
}
{
  const f = P('tools', 'lib', 'port_gen_defs.js'); const before = fs.readFileSync(f, 'utf8');
  const END = NL + '    },' + NL;
  const pos = (t) => { const anchor = '  tables: {' + NL; const at = t.indexOf(anchor); must(at >= 0, 'port_gen_defs.js: anchor tables'); return at + anchor.length; };
  const cols = COLS.filter((c) => c[2]).map((c) => c[0] + ': { type: ' + q(c[2]) + (c[3] && c[3] !== '-' ? ', doc: ' + q(c[3]) : '') + ' }').join(', ');
  const text = '    // ' + MARK + ' ' + NAME + ': written by tools/sora_form_ultimate.js (documentation/KH3_WEAPONS_PLAN.md 1.3)' + NL +
    '    ' + q(NAME) + ': {' + NL +
    "      file: 'sora_form_ultimate.rs', row: 'FormUltimateRow', table: 'FORM_ULTIMATE', key: 'id', key_prefix: 'FSW'," + NL +
    '      doc: ' + q(DOC) + ',' + NL +
    '      columns: { ' + cols + ' },' + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/port_gen_defs.js');
}
console.log('sora_form_ultimate ' + VERSION);
