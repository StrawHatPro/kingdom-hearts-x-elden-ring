// (weapons F-SHIP) KH3's two Wheel of Fate formchanges as DATA for their form package (kh-core combat/forms/ship.rs,
// states/combo_hl.rs, states/combo_fl.rs): Highwind (form.HL, CHANGE_HL 0x8C, stage 1) and Storm Flag (form.FL,
// CHANGE_FL 0x8D, stage 2, FINISH_FL 0xA6). The attack kinds of COMBO_HL (0x9C .. 0xA2) and COMBO_FL (0xA3 .. 0xA9), the
// decision thresholds, the flag throw chain (M_F412 -> M_F413 -> M_F414), the spear rain of HL_SPEAR and its projectile,
// the warp of Highwind's last finisher, the replacement anim sets (AnimSet14, AnimSet15: logical name -> clip),
// FINISH_FL (the Kraken: sub-steps, its style pawn w_so094 with its own hit windows), the weapon / transform-unit / pawn
// collision shapes and the attack rows the forms hit with. documentation/KH3_WEAPONS_PLAN.md 1.3 / 4 (package F-SHIP),
// build/sora_tmp/weapons/F-SHIP/STATE.md.
//
//   node tools/sora_form_ship.js   write sheets/port/sora-form-ship.json and register it (once, in place) in
//                                  tools/lib/sheet_defs.js and tools/lib/port_gen_defs.js; then node tools/gen_port.js
//
// Inputs (read only): sheets/port/sora-keyblade-forms.json (P3: the form rows), build/sora_pack/keyblades/
// sora_forms_manifest.json (the asset agent's pack of the form anim sets: logical rows, clips, notifies), build/sora_pack/
// manifest.json (the base set's clips), build/sora_pack/keyblades/w_so094/model.json (the Kraken's clip), the weapon /
// style-pawn / projectile blueprints, sheets/entities/DataTable/Player/Attack/p_ex001_AttackData.json, and THE CODE TABLES
// BELOW, read in the decompiled exe (re/decomp):
//   CLASS    pawn slot 1337 0x141432F00: form 0x8C -> class 0x57 TresPlayerState_ComboHL (id 87), 0x8D -> 0x58 ComboFL (id
//            88). MaxCombo (slot 928 0x141433570) 5 and Finishers (slot 1253 0x141433A60) 2 in both.
//   PLAY     TresPlayerState_Attack::PlayAttack 0x1413378B0 cases 0x9C .. 0xA9 (and the 0xA2 pre-switch 0x141337xxx): clip,
//            root length +0x1D8, stop distance +0x24C, +0x234 bits (0 XY, 1 root scale, 3 no Z, 16 root scale kept),
//            +0x23C bit 4 (DirZ kept), +0x1DE dive, +0x1DF air mode (a "break" of the switch sets it), DirZ +0x1D4.
//   DECIDE   ComboHL::Decide 0x141361CC0 / ComboFL::Decide 0x141361530 (vtables 0x14690F388 / 0x14690E778 slot 122).
//   CHAIN    ComboFL slot 125 0x141366630: M_F412 -> M_F413 at its end -> M_F414 at its end or with an attack queued.
//   RAIN     ComboHL slot 143 0x141364D20 (called by its Tick 0x141353A50) and its anim interface's shot handler (secondary
//            vtable 0x14690F808 slot 69 0x141358E20): a Shot notify (not in kind 0x37) starts the spear rain.
//   WARP     ComboHL slot 136 0x1413524A0: the teleport of kind 0xA0 (above the target), PlayAttack's head (+0x1E1).
//   FINISH   TresPlayerState_StyleFinishFL (id 39): Begin 0x1413CC350 (StyleFinish's), Tick 0x1413D5060, End 0x1413CF130,
//            cleanup 0x1413CFAF0.
//   START    StartStyle 0x14141A9D0 tails 0x8C / 0x8D (and the resume 0x14141E2B0 / 0x14141DD00): the row's start_extra.
'use strict';
const fs = require('fs');
const path = require('path');
const ROOT = path.resolve(__dirname, '..');
const VERSION = '1.0.0';
const P = (...a) => path.join(ROOT, ...a);
const readJson = (f) => JSON.parse(fs.readFileSync(f, 'utf8'));
const must = (c, m) => { if (!c) throw new Error(m); };
const hex = (n) => '0x' + n.toString(16).toUpperCase();
const r4 = (x) => Math.round(x * 10000) / 10000;

// ------------------------------------------------------------------------------------------------ inputs
const sheet = (name) => { const s = readJson(P('sheets', 'port', name + '.json')); return s.rows.map((r) => Object.fromEntries(s.columns.map((c, i) => [c, r[i]]))); };
const FORMS = sheet('sora-keyblade-forms');
const HL = FORMS.find((r) => r.id === 'form.HL');
const FL = FORMS.find((r) => r.id === 'form.FL');
must(HL && FL, 'sora-keyblade-forms has no form.HL / form.FL');
must(HL.cmd === 0x8C && FL.cmd === 0x8D && HL.next === 'form.FL' && FL.finish === 0xA6, 'form.HL / form.FL are not CHANGE_HL -> CHANGE_FL with FINISH_FL');
const OV = readJson(P('build', 'sora_pack', 'keyblades', 'sora_forms_manifest.json'));
const MAIN = readJson(P('build', 'sora_pack', 'manifest.json'));
const ATK = readJson(P('sheets', 'entities', 'DataTable', 'Player', 'Attack', 'p_ex001_AttackData.json')).rows;
const setName = (a) => String(a).split('/').pop().split('.')[0];
const SETS = { HL: HL.anim_sets.map(setName), FL: FL.anim_sets.map(setName) };
const ITEMS = new Map([...MAIN.items, ...OV.items].filter((i) => i.kind === 'animation').map((i) => [i.animation_name, i]));
const BASE = new Map(MAIN.logical_animations.map((a) => [a.name, a]));
function rowsOf(form) {
  const out = new Map();
  for (const set of SETS[form]) for (const a of OV.logical_animations.filter((x) => setName(x.anim_set) === set)) out.set(a.name, { set, clip: a.clip, file: a.file, cls: a.class });
  return out;
}
const ROWS_OF = { HL: rowsOf('HL'), FL: rowsOf('FL') };
function clipOf(form, logical) {
  const r = ROWS_OF[form].get(logical);
  if (r) return r.file ? r.clip : '-';
  const b = BASE.get(logical);
  return b && b.file ? path.basename(b.file, '.glb') : '-';
}
const item = (clip) => ITEMS.get(clip) || null;
// the notifies of a clip: hit windows (a-b) with their rows and groups, TresEnableChange / TresEnableInput, effects, sounds,
// the Shot notifies (time@projectile), the ThrowWeapon notifies (time@weapon clip).
function notes(clip) {
  const it = item(clip);
  const out = { length: it ? r4(it.duration_s) : 0, windows: [], rows: [], groups: [], change: -1, input: -1, effects: [], sounds: [], shots: [], throws: [] };
  if (!it) return out;
  for (const n of it.notifies || []) {
    const p = n.properties || {};
    if (n.class === 'TresAnimNotifyState_Attack') { out.windows.push(r4(n.time) + '-' + r4(n.end_time)); out.rows.push(String(p.m_AttackDataIDName)); out.groups.push(String(p.CollisionGrpName || 'None')); }
    if (n.class === 'TresAnimNotify_EnableChange' && out.change < 0) out.change = r4(n.time);
    if (n.class === 'TresAnimNotify_EnableInput' && out.input < 0) out.input = r4(n.time);
    if (n.class === 'TresAnimNotify_Shot') out.shots.push(r4(n.time) + '@' + (p.m_ProjectileName || '-'));
    if (n.class === 'TresAnimNotify_ThrowWeapon') out.throws.push(r4(n.time) + '@' + (p.m_WeaponAnimName || '-'));
    if (/AttachEffect/.test(n.class)) {
      const s = JSON.stringify(p);
      for (const m of s.matchAll(/"AttachEffect","type":"ObjectProperty","value":"([^"]*)"/g)) if (m[1] !== 'null') out.effects.push(r4(n.time) + '@' + m[1].split('.').pop());
    }
    if (/PlaySound|AttachSound|PlayVoice/.test(n.class)) { const s = JSON.stringify(p); const m = s.match(/\/Game\/Sound[^"]*/); out.sounds.push(r4(n.time) + '@' + (m ? m[0].split('.').pop() : n.class.replace(/^.*Notify(State)?_/, ''))); }
  }
  out.effects = [...new Set(out.effects)];
  return out;
}
const atkRow = (id) => ATK.find((r) => r._row_name === id || String(r.id).toLowerCase() === String(id).toLowerCase());

// ------------------------------------------------------------------------------------------------ the code tables
// PLAY: PlayAttack 0x1413378B0 (decompiled C). Before the switch: +0x234 = (bits & ~6) | 1 (the XY channel on, root scale
// off), stop 160 (0x43200000), z gain 3.0 (0x40400000), +0x1DF = 0. A case that ends with `break` falls on
// "+0x1DF = 1" (air mode) before the tail; `goto caseD_26` skips it. Tail LAB_14133A87A: air mode -> +0x234 |= 4 | 0x10,
// GravityScale 0.1, Flying, +0x18C = 0.6; else gravity on, DirZ 0. fVar28 (the root scale's distance) is 0 for every
// kind here: the root scale of the Play call is 1.
const PLAY = [
  { form: 'HL', kind: 0x9C, logical: 'M_E411', root_len: 835, stop: 300, chan_root: true, air: true, flags: ['root_keep'], label: 'ground 1 (the stab; +0x1DF = 1, goto caseD_26)', at: 'PlayAttack 0x1413378B0 case 0x9C' },
  { form: 'HL', kind: 0x9D, logical: 'M_E412', root_len: 362, stop: 300, chan_root: true, air: true, flags: ['root_keep'], label: 'ground 2 (the spin)', at: 'PlayAttack 0x1413378B0 case 0x9D' },
  { form: 'HL', kind: 0x9E, logical: 'M_E413', root_len: 528, stop: 300, chan_root: true, air: true, flags: ['root_keep'], label: 'ground 3 (the screw)', at: 'PlayAttack 0x1413378B0 case 0x9E' },
  { form: 'HL', kind: 0x9F, logical: 'M_E414', root_len: 895, stop: 300, chan_root: true, air: true, flags: ['root_keep'], label: 'finisher 1 (the thrust: Spear Rush)', at: 'PlayAttack 0x1413378B0 case 0x9F' },
  { form: 'HL', kind: 0xA0, logical: 'M_E416', stop: 300, air: true, dive: true, dir_z: -4000, flags: ['root_keep', 'no_vz', 'keep_dir_z', 'warp', 'finish_end'], label: 'finisher end (the warp above the target, the spear rain: Spear Rain / Spear Dive; LAB_141339FC7)', at: 'PlayAttack 0x1413378B0 case 0xA0' },
  { form: 'HL', kind: 0xA1, logical: 'M_E417', stop: 300, air: true, flags: ['root_keep', 'rise'], label: 'rise (the target above; LAB_1413394DD, break: air mode); Decide: DirZ = pawn+0xBAC x 200 (decide.rise_speed)', at: 'PlayAttack 0x1413378B0 case 0xA1' },
  { form: 'HL', kind: 0xA2, logical: 'M_E418', stop: 300, air: true, dive: true, dir_z: -4000, flags: ['root_keep', 'no_vz', 'keep_dir_z', 'flow_display'], label: 'dive (the target below; also the Sonic Cleave flow kind 0x37 in Highwind); the pre-switch case 0xA2: pawn vt+0x1388(1, 0) then (0) (the flow-weapon display mode, no core effect)', at: 'PlayAttack 0x1413378B0 case 0xA2' },
  { form: 'FL', kind: 0xA3, logical: 'M_F411', root_len: 450, stop: 160, chan_root: true, air: true, label: 'ground 1 (Storm Spinner)', at: 'PlayAttack 0x1413378B0 case 0xA3' },
  { form: 'FL', kind: 0xA4, logical: 'M_F412', stop: 160, chan_xy: false, air: true, flags: ['throw'], label: 'far: the flag throw M_F412 -> M_F413 -> M_F414 (ComboFL slot 125; LAB_1413394BB: the XY channel off)', at: 'PlayAttack 0x1413378B0 case 0xA4' },
  { form: 'FL', kind: 0xA5, logical: 'M_F415', root_len: 540, stop: 160, chan_root: true, air: true, label: 'ground 2 (the four swings)', at: 'PlayAttack 0x1413378B0 case 0xA5' },
  { form: 'FL', kind: 0xA6, logical: 'M_F416', stop: 160, air: true, label: 'ground 3 (the rotating flag)', at: 'PlayAttack 0x1413378B0 case 0xA6' },
  { form: 'FL', kind: 0xA7, logical: 'M_F417', root_len: 600, stop: 160, chan_root: true, air: true, flags: ['finish'], label: 'finisher 1 (the launch; LAB_14133A0C3)', at: 'PlayAttack 0x1413378B0 case 0xA7' },
  { form: 'FL', kind: 0xA8, logical: 'M_F418', stop: 160, air: true, flags: ['finish', 'finish_end'], label: 'finisher end (the whirlwind: Cyclone)', at: 'PlayAttack 0x1413378B0 case 0xA8' },
  { form: 'FL', kind: 0xA9, logical: 'M_F419', stop: 160, air: true, flags: ['rise'], label: 'rise (the target above); Decide: DirZ = decide.rise_speed', at: 'PlayAttack 0x1413378B0 case 0xA9' },
];
// DECIDE: ComboHL::Decide 0x141361CC0 / ComboFL::Decide 0x141361530: [the reach distance, the rise dz, the dive dz].
const DECIDE = [
  { form: 'HL', values: [400, 150, -150], kinds: [0x9C, 0x9D, 0x9E, 0x9F, 0xA0, 0xA1, 0xA2], at: '0x141361CC0',
    note: 'combo 0: dist <= v0 -> crowd > 1: reach 2 (else kept); dist > v0 -> reach 0; last kind 0. combo < MaxCombo 5 - Finishers 2: target high && dz > v1 && last != 0xA1 && dist <= v0 -> 0xA1 (DirZ fixed 700); dz < v2 && last != 0xA2 && dist <= v0 -> 0xA2; else by the last kind: 0x9C -> 0x9D + (reach 2), 0x9D -> reach 2 ? 0x9C : 0x9E, 0x9E -> 0x9C + (reach 2), none: combo 0 -> 0x9C + (reach 2), else 0x9D + (reach 2). Else the finish: n = finishers played; n < 1 && combo < 4 -> 0x9F, else finish end, 0xA0 (the warp +0x1E1), the homing target = the controller\'s; n + 1. +0x1F4 bit 1 -> 0xA0 at once (no writer found). First of all slot 124(0x37): Sonic Cleave (forced 0x37, kept without SONIC_DOWN) becomes 0xA2 with bAir' },
  { form: 'FL', values: [600, 150, -150], kinds: [0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9], at: '0x141361530',
    note: 'combo 0: dist <= v0 -> crowd > 1: reach 2 (else kept); dist > v0 -> reach 1; last kind 0. combo < 3: target high && dz > v1 && last != 0xA9 && dist <= v0 -> 0xA9 (DirZ fixed 700); dz < v2 && last != 0xA3 && dist <= v0 -> 0xA3; else by the last kind: 0xA3 -> 0xA5 + (reach 2), 0xA4 -> 0xA5, 0xA5 -> reach 2 ? 0xA3 : 0xA6, 0xA6 -> reach 2 ? 0xA5 : 0xA3, none: combo 0 -> reach 2 ? 0xA6 : 0xA3 + (reach 1), else 0xA5. Else the finish: n < 1 && combo < 4 -> 0xA7, else finish end, 0xA8, the homing target; n + 1. +0x1F4 bit 1 -> 0xA8 at once (no writer found)' },
];
// RAIN: ComboHL slot 143 0x141364D20: [interval s (0x3D88882F), the last count (stops when +0x170 > 14), shots per wave,
// the angle step, the radius base, the radius per shot, the height]; the first yaw is (xorshift >> 8) x 8.940697e-6 - 180 (random in [-180, -30), world degrees).
const RAIN = [0.0667, 14, 3, 120, 150, 50, 200];
// WARP: PlayAttack's head with +0x1E1 (JumpNormalStart, the countdown +0x258 = 0.2, invincible pawn vt+0x2068(1)), slot 117
// (the countdown), ComboHL slot 136 0x1413524A0 (kind 0xA0 only, the Attack class's for the rest): from his point to the
// target's (its collision top + 500 for a pawn), swept with radius 60 (pawn vt+0x2750), then 100 down (the second argument
// of the slot-117 call: 0x42C80000). Values: [delay, height, drop, radius].
const WARP = [0.2, 500, 100, 60];

// FINISH: StyleFinishFL Tick 0x1413D5060 by sub-step (+0x2E8 counts every tick).
const FINISH = [
  { sub: 0, logical: 'M_F420', what: 'the Kraken (m_ReplaceAccompanyPawnAsset of FINISH_FL: TresStylePawnFLKraken, w_so094) spawned at Sora\'s root with his yaw (0x1405D9130), put on the ground (0x1413F5600), Sora moved to it (0x143EAC3D0), the Kraken shown (vt+0xD98(1)) and playing M_F420; Sora Flying (vt+0x17A0(5, 1)) and playing M_F420; +0x164 = 0; +0x2E8 = 0; pawn vt+0x1038(0, 0)', flags: ['spawn_pawn', 'flying', 'move_to_pawn'] },
  { sub: 2, logical: '-', what: 'the Kraken starts (+0xB48 = 1) and the tick returns; the next tick 0x14140E500(ctrl, 0): the world slow off', flags: ['world_slow_off'] },
  { sub: 5, logical: '-', what: 'until the Kraken\'s m_Param_MaxTime (vt+0x2620) <= +0x2E8, m_bEnableChange, or the end of Sora\'s M_F420', flags: [] },
  { sub: 6, logical: 'M_1501', what: 'the Kraken removed (0x1413CFAF0: its vt+0x23B0(0), the effects, vt+0x2590 destroy); Sora M_1501 (the landing)', flags: ['cleanup'] },
  { sub: 7, logical: '-', what: 'one tick', flags: [] },
  { sub: 8, logical: '-', what: 'at M_1501\'s end the finish is over (vt+0x270 = DoChain)', flags: ['end'] },
];
// SHAPES: TresAtkColl0 CollisionShapesSrc of the weapon parts, the transform unit and the style pawn ([A] the blueprints).
const SHAPE_OWNERS = [['HL', 'Player/p_ex001/w_so092_Wep'], ['FL', 'Player/p_ex001/w_so093_Wep'], ['HL', 'Player/p_ex001/w_so091_Wep'], ['FL', 'StylePawn/w_so094']];
const PKG = P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content', 'Blueprints');
const flat = (props) => { const o = {}; for (const p of props || []) o[p.name] = p.type === 'StructProperty' ? (Array.isArray(p.value) ? flat(p.value) : p.value) : p.type === 'ArrayProperty' ? (p.value && p.value.items ? p.value.items.map((it) => (Array.isArray(it) ? flat(it) : it)) : p.value) : p.value; return o; };
const pkg = (rel) => readJson(path.join(PKG, rel + '.uasset.json'));
function shapes(rel) {
  const d = pkg(rel);
  const out = [];
  for (const e of d.exports) {
    if (e.class !== 'TresAtkCollComponent') continue;
    const p = flat((e.data && e.data.props) || e.props);
    for (const s of p.CollisionShapesSrc || []) {
      const v = (o) => (o ? [o.x || 0, o.y || 0, o.z || 0] : [0, 0, 0]);
      out.push({ owner: rel.split('/').pop().replace(/_Wep$/, ''), group: s.m_GrpName, shape: String(s.m_ShapeType).split('::').pop(), size: v(s.m_Size), socket1: s.m_SocketName1 || 'None', socket2: s.m_SocketName2 || 'None', loc: v(s.m_RelativeLocation1), loc2: v(s.m_RelativeLocation2), scale: s.m_Scale ? v(s.m_Scale) : [1, 1, 1], row: s.m_DefaultAttackDataIDName || 'None', attach: String(s.m_AttachType1 || '').split('::').pop(), frozen: !!s.m_bDisableLocation1Attach });
    }
  }
  return out;
}
function classProps(rel, cls) {
  const d = pkg(rel);
  const e = d.exports.find((x) => x.class === cls);
  return e ? flat((e.data && e.data.props) || e.props) : {};
}

// ------------------------------------------------------------------------------------------------ the shape tracks (FK)
// Where each hit shape of a form weapon is, per frame of Sora's clip, in his ACTOR space (UE cm, x forward, his yaw 0): FK of
// Sora's skeleton under the clip to the weapon's socket on him (the part's socket; (FORMWEP-FIX) a transform unit has none:
// UE's AttachToComponent(NAME_None) = his MESH component, `MESH`: its clips are authored in his mesh space), the weapon's own skeleton under ITS clip of the same logical name (else its reference pose), the weapon's
// socket (sockets.json), the shape's relative location. The same rule as tools/sora_pack.js attackSockets and
// tools/sora_form_claws.js (F-CLAWS), whose FK this repeats. Owner-attached shapes (ChangeHL2 at `center`, HL_Thrust at
// `kh_root`) are placed on Sora's own socket / bone under the clip.
const sp = require('./sora_pack.js');
const qmulH = (a, b) => [a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1], a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0], a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3], a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]];
const qrot = (q, v) => { const tx = 2 * (q[1] * v[2] - q[2] * v[1]), ty = 2 * (q[2] * v[0] - q[0] * v[2]), tz = 2 * (q[0] * v[1] - q[1] * v[0]); return [v[0] + q[3] * tx + (q[1] * tz - q[2] * ty), v[1] + q[3] * ty + (q[2] * tx - q[0] * tz), v[2] + q[3] * tz + (q[0] * ty - q[1] * tx)]; };
const under = (c, p) => { const v = qrot(p.q, [p.s[0] * c.t[0], p.s[1] * c.t[1], p.s[2] * c.t[2]]); return { q: qmulH(p.q, c.q), t: [v[0] + p.t[0], v[1] + p.t[1], v[2] + p.t[2]], s: [p.s[0] * c.s[0], p.s[1] * c.s[1], p.s[2] * c.s[2]] }; };
const point = (T, v) => { const r = qrot(T.q, [T.s[0] * v[0], T.s[1] * v[1], T.s[2] * v[2]]); return [r[0] + T.t[0], r[1] + T.t[1], r[2] + T.t[2]]; };
function rotQ(r) {
  const h = Math.PI / 360, SP = Math.sin(r.pitch * h), CP = Math.cos(r.pitch * h), SY = Math.sin(r.yaw * h), CY = Math.cos(r.yaw * h), SR = Math.sin(r.roll * h), CR = Math.cos(r.roll * h);
  return [CR * SP * SY - SR * CP * CY, -CR * SP * CY - SR * CP * SY, CR * CP * SY - SR * SP * CY, CR * CP * CY + SR * SP * SY];
}
const r1 = (x) => Math.round(x * 10) / 10;
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
const meshYaw = ID.mesh_component.relative_rotation_ue_deg.yaw * Math.PI / 180, mcy = Math.cos(meshYaw), msy = Math.sin(meshYaw);
const actor = (v) => [v[0] * mcy - v[1] * msy, v[0] * msy + v[1] * mcy, v[2]];
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
// Sora's own socket or bone under his clip (component space)
function soraT(name, clip, time) {
  const sa = soraAnim(clip);
  if (!sa) return null;
  const sock = SORA_SOCKETS[name];
  const bone = SORA_BONES.findIndex((b) => b.name === (sock ? sock.bone : name));
  if (bone < 0) return null;
  const H = boneT(SORA_BONES, byBone(sa), bone, Math.min(time, sa.length), sa.props.bEnableRootMotion === true);
  return sock ? under(sock.T, H) : H;
}
function weaponSocketT(part, socketName, logical, clip, time) {
  const w = weapon(part.model);
  if (!w) return null;
  const base = part.socket === MESH ? { q: [0, 0, 0, 1], t: [0, 0, 0], s: [1, 1, 1] } : soraT(part.socket, clip, time); // (FORMWEP-FIX)
  if (!base) return null;
  const { a } = weaponAnim(w, logical, clip);
  const wt = a ? Math.min(time, a.length) : 0;
  const ws = w.sockets[socketName];
  const k = w.bones.findIndex((b) => b.name === (ws ? ws.bone : socketName));
  if (k < 0) return null;
  let T = under(boneT(w.bones, byBone(a), k, wt, false), base);
  if (ws) T = under({ q: rotQ({ pitch: ws.local_rotation[0], yaw: ws.local_rotation[1], roll: ws.local_rotation[2] }), t: ws.local_location, s: [1, 1, 1] }, T);
  return T;
}
// [ax, ay, az, bx, by, bz, radius] of one shape at a time; a CONVEX (the 20-gon prism of tres_colltest01, combat/hit.rs
// `Convex`) as [centre, centre, -1] with its scale in the row (the prism is built in the core).
function shapeAt(s, logical, clip, time) {
  const at = (sock, loc) => {
    const T = s.attach === 'OWNER_ATTACHED_MESH' ? soraT(sock, clip, time) : weaponSocketT(s.part, sock, logical, clip, time);
    return T ? actor(point(T, loc)) : null;
  };
  const a = at(s.socket1, s.loc);
  if (!a) return null;
  if (s.shape === 'CONVEX') return [...a, ...a, -1];
  if (s.shape === 'SPHERE' || s.socket2 === 'None') return [...a, ...a, s.size[0]];
  if (s.shape === 'CAPSULE1') return [a[0], a[1], a[2] - s.size[2], a[0], a[1], a[2] + s.size[2], s.size[0]];
  const b = at(s.socket2, s.loc2);
  return b ? [...a, ...b, s.size[0]] : [...a, ...a, s.size[0]];
}
// the form's parts: unit@part:socket=blueprint (units; transform units: socket None -> (FORMWEP-FIX) his mesh component, MESH)
const MESH = 'mesh';
const partsOf = (list, none) => list.map((u) => { const m = u.match(/^(\w+)@(\w+):(\w+)=(.*)$/); must(m, 'unit ' + u); return { socket: m[3] === 'None' ? none : m[3], model: m[4].split('/').pop().replace(/_[Ww]ep$/, '') }; });
const PARTS = { HL: [...partsOf(HL.units, 'R_wep_skt'), ...partsOf(HL.transform_units, MESH)], FL: [...partsOf(FL.units, 'R_wep_skt'), ...partsOf(FL.transform_units, MESH)] };
const SHAPES_OF = (form) => PARTS[form].flatMap((part) => shapes('Player/p_ex001/' + part.model + '_Wep').map((s) => ({ ...s, part })));

// ------------------------------------------------------------------------------------------------ rows
const ROWS = [];
const CMN_AURA = []; // (AUDIT-B W-CORE) ead_pc_aura_com0 groups 13 / 14 (Final Blow), appended last
const blank = { form: '-', code: 0, logical: '-', clip: '-', length: 0, root_len: 0, stop_dist: 0, z_gain: 0, dir_z: 0, air_mode: false, chan_xy: true, chan_root: false, flags: [], windows: [], rows: [], groups: [], effects: [], sounds: [], values: [], ref_addr: '-', ref_conf: 'confirmed', note: '-' };
const add = (r) => ROWS.push({ ...blank, ...r });
// the attacks
for (const k of PLAY) {
  const clip = clipOf(k.form, k.logical);
  must(clip !== '-', k.form + ' ' + k.logical + ': no clip');
  const n = notes(clip);
  for (const r of n.rows) must(atkRow(r), k.form + ' ' + k.logical + ': p_ex001_AttackData has no row ' + r);
  add({ id: 'attack.' + k.form + '.' + hex(k.kind), what: 'attack', form: k.form, code: k.kind, logical: k.logical, clip, length: n.length, root_len: k.root_len || 0, stop_dist: k.stop, z_gain: 3, dir_z: k.dir_z || 0, air_mode: !!k.air, chan_xy: k.chan_xy !== false, chan_root: !!k.chan_root,
    flags: [...(k.flags || []), ...(k.dive ? ['dive'] : []), ...n.shots.map((s) => 'shot@' + s), ...n.throws.map((s) => 'throw@' + s)], windows: n.windows, rows: n.rows, groups: n.groups, effects: n.effects, sounds: n.sounds, values: [n.change, n.input], ref_addr: k.at, ref_conf: 'confirmed',
    note: k.label + '; values = [TresEnableChange, TresEnableInput] of the clip (-1 none); z gain 3 (the default); dir_z = +0x1D4 written by the case' });
}
// the flag throw chain (ComboFL slot 125 0x141366630)
for (const [i, logical] of ['M_F413', 'M_F414'].entries()) {
  const clip = clipOf('FL', logical);
  must(clip !== '-', 'FL ' + logical + ': no clip');
  const n = notes(clip);
  add({ id: 'chain.FL.' + logical, what: 'chain', form: 'FL', code: 0xA4, logical, clip, length: n.length, flags: i === 1 ? ['weapon_back'] : ['enable_input_on_hit'], windows: n.windows, rows: n.rows, groups: n.groups, effects: n.effects, sounds: n.sounds, values: [n.change, n.input], ref_addr: '0x141366630',
    note: i === 0 ? 'at M_F412\'s end (anim vt+200): played (mode 2, blend 0); TresEnableInput once a hit landed (pawn+0xE24 > 0); SetVoiceGroupNo(0)' : 'at M_F413\'s end or with an attack queued (+0x143): pawn vt+0x13B8(7) (the thrown flag back to the hand), played (blend -1); a queued attack sets m_bEnableChange' });
}
// the decision thresholds
for (const d of DECIDE) add({ id: 'decide.' + d.form, what: 'decide', form: d.form, code: d.form === 'HL' ? 0x8C : 0x8D, values: d.values, flags: d.kinds.map(hex), ref_addr: d.at, note: d.note + '; values = [reach distance, rise dz, dive dz] (cm); MaxCombo 5, Finishers 2 (slots 928 / 1253)' });
// the spear rain and its projectile
{
  const proj = classProps('Projectile/HL/TresProjHL_Spear', 'TresProjHL_Spear_C');
  const mv = classProps('Projectile/HL/TresProjHL_Spear', 'TresProjectileMovementComponent');
  const root = classProps('Projectile/HL/TresProjHL_Spear', 'TresRootComponent');
  const sh = shapes('Projectile/HL/TresProjHL_Spear');
  must(sh.length === 1 && sh[0].row === 'SCHL_FINISH', 'TresProjHL_Spear: one SPHERE of SCHL_FINISH');
  const rootR = ((root.CollisionShapesSrc || [])[0] || {}).Size;
  const imports = (pkg('Projectile/HL/TresProjHL_Spear').imports || []).filter((i) => i.className === 'ParticleSystem').map((i) => i.objectName);
  add({ id: 'proj.HL_SPEAR', what: 'proj', form: 'HL', logical: 'HL_SPEAR', clip: 'TresProjHL_Spear', flags: [proj.m_bNoDestroyOnHit ? 'no_destroy_on_hit' : 'destroy_on_hit', proj.m_bAttachToHitActor ? 'attach_to_hit_actor' : '-'], rows: [sh[0].row], groups: [sh[0].group], effects: imports,
    values: [proj.m_MainLife, mv.InitialSpeed, mv.m_fMoveStartDelay, sh[0].size[0], rootR ? rootR.x : 25], ref_addr: '/Game/Blueprints/Projectile/HL/TresProjHL_Spear',
    note: 'the spear of the rain (w_so090_ProjSet HL_SPEAR): values = [m_MainLife s, InitialSpeed cm/s (spawned with the direction (0, 0, -1)), m_fMoveStartDelay s, the attack sphere radius, the root collision radius]; effects = its particle templates (bullet, hit, map hit)' });
  add({ id: 'rain.HL', what: 'rain', form: 'HL', logical: 'HL_SPEAR', values: RAIN, ref_addr: '0x141364D20', note: 'the spear rain: every values[0] s while +0x284 is set, values[2] spears at the angles yaw0 + k x values[3] (yaw0 = (xorshift >> 8) x 8.940697e-6 - 180: random in [-180, -30), world degrees), each at the shot origin (pawn vt+0x1410: the weapon\'s ATK_01) + direction x (values[4] + values[5] x spears so far) + (0, 0, values[6]), direction down; +0x284 off once more than values[1] were spawned (15). Started by a Shot notify outside kind 0x37 (0x141358E20: +0x284 = 1, +0x280 = 0, +0x288 = the notify\'s m_ProjectileName); only while the attack state ticks' });
  add({ id: 'warp.HL.0xA0', what: 'warp', form: 'HL', code: 0xA0, logical: 'JumpNormalStart', values: WARP, ref_addr: '0x1413524A0', note: 'values = [the countdown s (+0x258), the height above the target\'s collision top, the drop after it, the sweep radius]; invincible from the head (pawn vt+0x2068(1)); then PlayAttack(0xA0)' });
}
// the replacement anim sets (logical name -> clip), per form
for (const form of ['HL', 'FL']) {
  for (const [logical, r] of [...ROWS_OF[form]].sort((a, b) => (a[0] < b[0] ? -1 : 1))) {
    add({ id: 'anim.' + form + '.' + logical, what: 'anim', form, logical, clip: r.file ? r.clip : '-', length: r.file && item(r.clip) ? r4(item(r.clip).duration_s) : 0, flags: [r.set, r.cls], note: r.file ? '-' : 'a ' + r.cls + ' of the set: not a row of the animation table (its samples are)' });
  }
}
// the finish command
for (const f of FINISH) {
  const clip = f.logical === '-' ? '-' : clipOf('FL', f.logical);
  const n = clip !== '-' ? notes(clip) : null;
  add({ id: 'finish.FL.' + f.sub, what: 'finish', form: 'FL', code: f.sub, logical: f.logical, clip, length: n ? n.length : 0, flags: f.flags, windows: n ? n.windows : [], rows: n ? n.rows : [], groups: n ? n.groups : [], effects: n ? n.effects : [], sounds: n ? n.sounds : [], values: n ? [n.change, n.input] : [], ref_addr: '0x1413D5060', note: f.what });
}
// the Kraken (the style pawn of FINISH_FL): its own clip's hit windows, effects, sounds; m_Param_MaxTime from its blueprint
{
  const KBM = readJson(P('build', 'sora_pack', 'keyblades', 'manifest.json'));
  const pawns = new Map();
  for (const f of KBM.keyblades.w_so090.forms) for (const p of f.accompany_pawns || []) pawns.set(p.model, p);
  must(pawns.has('w_so094'), 'w_so090 has no accompany pawn w_so094');
  const p = pawns.get('w_so094');
  const mj = readJson(P('build', 'sora_pack', 'keyblades', 'w_so094', 'model.json'));
  const wins = [], wrows = [], wgroups = [], effects = [], sounds = [];
  for (const c of mj.clips || []) for (const n of c.notifies || []) {
    const np = n.properties || {};
    const name = (c.logical_names || [c.sequence]).join('/');
    if (n.class === 'TresAnimNotifyState_Attack') { wins.push(name + ':' + r4(n.trigger_time) + '-' + r4(n.end_trigger_time) + (np.AttackInterval ? '/' + r4(np.AttackInterval) : '')); wrows.push(np.m_AttackDataIDName); wgroups.push(np.CollisionGrpName); }
    if (/AttachEffect/.test(n.class)) for (const m of JSON.stringify(np).matchAll(/"AttachEffect","type":"ObjectProperty","value":"([^"]*)"/g)) if (m[1] !== 'null') effects.push(r4(n.trigger_time) + '@' + m[1].split('.').pop());
    if (/Sound/.test(n.class)) { const m = JSON.stringify(np).match(/\/Game\/Sound[^"]*/); if (m) sounds.push(r4(n.trigger_time) + (n.end_trigger_time ? '-' + r4(n.end_trigger_time) : '') + '@' + m[0].split('.').pop()); }
    if (n.class === 'TresAnimNotifyState_Attack' && np.m_HitSEAsset) sounds.push('hit@' + String(np.m_HitSEAsset).split('.').pop());
  }
  const bp = classProps('StylePawn/w_so094', 'w_so094_C');
  must(typeof bp.m_Param_MaxTime === 'number', 'w_so094: no m_Param_MaxTime');
  add({ id: 'pawn.FL.w_so094', what: 'pawn', form: 'FL', logical: p.blueprint, clip: p.model, flags: Object.keys((KBM.models.w_so094 && KBM.models.w_so094.clips) || {}), windows: wins, rows: wrows, groups: wgroups, effects: [...new Set(effects)], sounds: [...new Set(sounds)], values: [bp.m_Param_MaxTime], ref_addr: '0x1413D5060',
    note: 'the Kraken (TresStylePawnFLKraken, m_bIsInvincible): flags = its packed clips; windows = clip:a-b[/interval]; values = [m_Param_MaxTime s, from its blueprint]' });
}
// (AUDIT-B F-SHIP) THE WEAPONS' / TRANSFORM UNIT'S / KRAKEN'S OWN CLIP NOTIFIES. KH3 gives each weapon actor and the style
// pawn their own anim instance, which plays the row of the logical name Sora plays (the Kraken: M_F420 with him, from
// StyleFinishFL Tick sub 0; the flag: M_F413 by the ThrowWeapon notify), and that clip's notifies fire on THEIR mesh: the
// AttachEffect rows on the weapon's own bones / sockets (OWNER = the weapon mesh), the AttachSound / PlayWeaponSwingSE rows
// at its sockets. One row per effect entry / sound: the anchor `kb:<model>:<mount>:<bone>` (mount = Sora's socket the part
// stands on: the units' / transform units' socket, R_wep_skt for a unit with none as P5 draws it; `root` = Sora's
// component frame, where the Kraken stands), `:abs` (bAbsoluteRotation), `:free` (ToAttach false); the host resolves it
// on the weapon's own skeleton under its clip at Sora's clip time (kh-host vfx.rs).
{
  const mounts = {};
  for (const f of [HL, FL]) {
    for (const u of [...f.units, ...f.transform_units]) { const m = u.match(/^(\w+)@(\w+):(\w+)=(.*)$/); mounts[m[4].split('/').pop().replace(/_[Ww]ep$/, '')] = m[3] === 'None' ? 'R_wep_skt' : m[3]; }
  }
  mounts.w_so094 = 'root';
  const pv = (list) => { const o = {}; for (const p of list || []) o[p.name] = p.type === 'StructProperty' ? (Array.isArray(p.value) ? pv(p.value) : p.value) : p.type === 'ArrayProperty' ? (p.value && p.value.items ? p.value.items.map((it) => (Array.isArray(it) ? pv(it) : it)) : p.value) : p.value; return o; };
  const tail = (s) => String(s || '').split('/').pop().split('.').pop();
  for (const model of ['w_so091', 'w_so092', 'w_so093', 'w_so094']) {
    const mj = readJson(P('build', 'sora_pack', 'keyblades', model, 'model.json'));
    for (const c of mj.clips || []) {
      let k = 0;
      for (const n of c.notifies || []) {
        const np = n.properties || {};
        const t = r4(n.trigger_time !== undefined ? n.trigger_time : n.time);
        const state = /NotifyState_/.test(n.class);
        const end = state ? r4(n.end_trigger_time !== undefined ? n.end_trigger_time : n.end_time) : -1;
        const form = /^E/.test(c.sequence) ? 'HL' : 'FL';
        const base = { what: 'wnotify', form, clip: c.sequence, length: r4(c.duration_s || 0), ref_addr: 'build/sora_pack/keyblades/' + model + '/model.json', note: 'the ' + model + ' clip ' + c.sequence + ' (' + (c.logical_names || []).join('/') + '): ' + n.class + '; values = [time, end (-1 instant), location x, y, z, rotation pitch, yaw, roll, fade out s]' };
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
                add({ ...base, id: 'wnotify.' + model + '.' + c.sequence + '.' + k++, code: 0, logical: 'effect', flags: [anchor, endType, state ? 'state' : 'instant', 'g' + d.GroupID], effects: [tail(d.AttachEffect)],
                  // (AUDIT-B W-FIN) + [FadeIn, StartDelayTime] when the row has either (`wnotify_pass` -> `Effects::timing`)
                  values: [t, end, r4(lo.x || 0), r4(lo.y || 0), r4(lo.z || 0), r4(ro.pitch || 0), r4(ro.yaw || 0), r4(ro.roll || 0), endType === 'FADEOUT' ? r4(m.FadeOut || 0) : 0, ...((m.FadeIn || 0) || (m.StartDelayTime || 0) ? [r4(m.FadeIn || 0), r4(m.StartDelayTime || 0)] : [])] });
              }
            }
          }
        } else if (/AttachSound|PlaySound/.test(n.class)) {
          const anchor = 'kb:' + model + ':' + mounts[model] + ':' + (np.AttachPointName && np.AttachPointName !== 'None' ? np.AttachPointName : '');
          add({ ...base, id: 'wnotify.' + model + '.' + c.sequence + '.' + k++, code: 1, logical: 'sound', flags: [anchor, '-', state ? 'state' : 'instant'], sounds: [tail(np.Sound)], values: [t, end, 0, 0, 0, 0, 0, 0, r4(np.FadeOutDuration || 0)] });
        } else if (/WeaponSwingSE|WeaponHitSE/.test(n.class)) {
          const anchor = 'kb:' + model + ':' + mounts[model] + ':' + (np.m_SocketName && np.m_SocketName !== 'None' ? np.m_SocketName : '');
          add({ ...base, id: 'wnotify.' + model + '.' + c.sequence + '.' + k++, code: 2, logical: /Hit/.test(n.class) ? 'hit_se' : 'swing_se', flags: [anchor, np.m_bFollow ? 'follow' : 'fixed', state ? 'state' : 'instant'], sounds: [String(np.m_SoundAliasID || '').split('::').pop()], values: [t, end, 0, 0, 0, 0, 0, 0, 0] });
        }
      }
    }
  }
}
// (AUDIT-A THROW) A THROWN part's own throw clip (the flag w_so093's M_F413: TresAnimNotify_ThrowWeapon of Sora's F412 ->
// weapon vt+0x10C0 0x14178F080 plays it from 0 at the throw, looped): its rows run on THAT clip's clock, not Sora's F413
// (kh-core forms/ship.rs `wnotify_pass_dt`, flag `thrown` at flags[4]; [3] padded with '-' on the sound rows, which read no
// group). The thrown clips come from sheets/port/sora-form-placement.json (tools/sora_form_placement.js `throw` rows).
{
  const thrownClips = new Set(sheet('sora-form-placement').filter((r) => r.what === 'throw' && r.flags[4] === 'thrown').map((r) => r.model + '|' + r.names[1]));
  let n = 0;
  for (const r of ROWS.filter((x) => x.what === 'wnotify')) {
    const model = r.id.split('.')[1];
    if (!thrownClips.has(model + '|' + r.clip)) continue;
    while (r.flags.length < 4) r.flags = [...r.flags, '-'];
    r.flags = [...r.flags, 'thrown'];
    n += 1;
  }
  must(n === 3, 'expected the 3 rows of w_so093 F413 as thrown rows, got ' + n);
}
// (AUDIT-B F-SHIP) THE WARP'S AURA GROUPS: PlayAttack's head with +0x1E1 calls the anim interface's vt+0x1A0(pawn vt+0x2458()
// = pawn+0x8E0 m_CmnAuraEffect, +0x22C, 0) (0x1413378B0 line 110), slot 117 at the warp's end vt+0x1A0(.., +0x22C + 1, 0)
// (0x14134AA80) and End the same while the warp is still pending (0x141335120); +0x22C = 11 from the Attack Reset
// 0x14133D230 (ComboHL writes none). Sora's m_CmnAuraEffect (p_ex001_PawnBase CDO) = ead_pc_aura_com0: its rows of groups 11
// (the vanish) and 12 (the reappearance).
{
  const ead = readJson(P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content', 'Effects', 'system', 'aura', 'ead', 'ead_pc_aura_com0.uasset.json'));
  const ref = (i) => { if (typeof i !== 'number') return String(i); if (i < 0) { const im = ead.imports[-i - 1]; return im ? im.objectName : '?'; } const e = ead.exports[i - 1]; return e ? e.objectName : '?'; };
  const pv = (list) => { const o = {}; for (const p of list || []) o[p.name] = p.type === 'StructProperty' ? (Array.isArray(p.value) ? pv(p.value) : p.value) : p.type === 'ArrayProperty' ? (p.value && p.value.items ? p.value.items.map((it) => (Array.isArray(it) ? pv(it) : it)) : p.value) : p.value; return o; };
  let k = 0;
  for (const e of ead.exports) {
    const props = pv((e.data && e.data.props) || e.props);
    // the asset's top list: one SQEX_StructParticleAttachData per row (汎用ワープ(本体)：消失11/出現12: the generic warp of the body,
    // vanish 11 / appear 12), each with its own EffectDataList, socket and offsets
    for (const meta of props.EffectDataList || []) {
      const m = Array.isArray(meta) ? pv(meta) : meta;
      for (const d of (m.EffectDataList || []).map((x) => (Array.isArray(x) ? pv(x) : x))) {
        // (AUDIT-B W-CORE) groups 13 / 14: the chase warp of Final Blow (the base Attack's Begin 0x141333210: +0x22C = 0xD with
        // +0x1E1), every form and no form: rows `aura.ALL.<group>.<n>`, appended after every other row (CMN_AURA)
        if (d.GroupID === 13 || d.GroupID === 14) {
          const lo = m.LocationOffset || {}, ro = m.RotationOffset || {};
          CMN_AURA.push({ id: 'aura.ALL.' + d.GroupID + '.' + CMN_AURA.length, what: 'aura', form: 'ALL', code: d.GroupID, logical: d.GroupID === 13 ? 'warp_in' : 'warp_out', clip: 'ead_pc_aura_com0', flags: [(m.SocketName && m.SocketName !== 'None' ? m.SocketName : '-') + (m.ToAttach === false && m.SocketName && m.SocketName !== 'None' ? ':free' : ''), m.ToAttach === false ? 'free' : 'attached'], effects: [ref(d.AttachEffect)],
            values: [r4(lo.x || 0), r4(lo.y || 0), r4(lo.z || 0), r4(ro.pitch || 0), r4(ro.yaw || 0), r4(ro.roll || 0)], ref_addr: '0x141333210 / 0x1413378B0 / 0x14134AA80 / 0x141335120',
            note: '(AUDIT-B W-CORE) Final Blow\'s chase warp (Begin 0x141333210: +0x234 bit 19 -> kind 0x22, +0x1E1 = 1, +0x22C = 0xD): group 13 at PlayAttack\'s head, 14 at its end (slot 117) or at End while pending; flags = [socket, ToAttach]; values = [location, rotation offset]' });
          continue;
        }
        if (d.GroupID !== 11 && d.GroupID !== 12) continue;
        const lo = m.LocationOffset || {}, ro = m.RotationOffset || {};
        add({ id: 'aura.HL.' + d.GroupID + '.' + k++, what: 'aura', form: 'HL', code: d.GroupID, logical: d.GroupID === 11 ? 'warp_in' : 'warp_out', clip: 'ead_pc_aura_com0', flags: [(m.SocketName && m.SocketName !== 'None' ? m.SocketName : '-') + (m.ToAttach === false && m.SocketName && m.SocketName !== 'None' ? ':free' : ''), m.ToAttach === false ? 'free' : 'attached'], effects: [ref(d.AttachEffect)],
          values: [r4(lo.x || 0), r4(lo.y || 0), r4(lo.z || 0), r4(ro.pitch || 0), r4(ro.yaw || 0), r4(ro.roll || 0)], ref_addr: '0x1413378B0 / 0x14134AA80 / 0x141335120',
          note: 'the warp of 0xA0: group 11 at its head, 12 at its end (the anim interface vt+0x1A0 with m_CmnAuraEffect ead_pc_aura_com0); flags = [socket, ToAttach]; values = [location, rotation offset]' });
      }
    }
  }
  must(ROWS.filter((r) => r.what === 'aura').length >= 4, 'ead_pc_aura_com0: no rows of groups 11 / 12');
  must(CMN_AURA.some((r) => r.code === 13) && CMN_AURA.some((r) => r.code === 14), 'ead_pc_aura_com0: no rows of groups 13 / 14'); // (AUDIT-B W-CORE)
}
// the collision shapes
for (const [form, rel] of SHAPE_OWNERS) for (const s of shapes(rel)) {
  add({ id: 'shape.' + s.owner + '.' + s.group, what: 'shape', form, logical: s.group, clip: s.owner, flags: [s.shape, s.socket1, s.socket2, s.attach, s.frozen ? 'frozen' : '-'], values: [...s.size, ...s.loc, ...s.scale, ...s.loc2], rows: [s.row], ref_addr: '-', note: 'TresAtkColl0 CollisionShapesSrc of ' + rel + ': flags = [shape, socket 1, socket 2, attach type, m_bDisableLocation1Attach], values = [size x, y, z, location 1 x, y, z, scale x, y, z, location 2 x, y, z], rows = [the default attack row]' });
}
// THE SHAPE TRACKS: for every clip of the forms with hit windows and every group its windows name, that group's shapes on the
// form's weapons (several shapes of one group merged into one capsule: the two farthest-apart end points, the largest
// radius, as F-CLAWS), per frame (time = frame x length / (frames - 1)): [ax, ay, az, bx, by, bz, radius] (radius -1 = a
// CONVEX: the prism of the shape row). Keyed by the clip's SEQUENCE and the group.
{
  const wanted = [];
  for (const k of PLAY) wanted.push([k.form, k.logical, clipOf(k.form, k.logical)]);
  for (const l of ['M_F413', 'M_F414']) wanted.push(['FL', l, clipOf('FL', l)]);
  wanted.push(['HL', HL.transform_clip, clipOf('HL', HL.transform_clip)], ['FL', FL.transform_clip, clipOf('FL', FL.transform_clip)]);
  const merge = (list) => {
    if (list.some((v) => v[6] < 0)) return list.find((v) => v[6] < 0);
    const pts = list.flatMap((v) => [v.slice(0, 3), v.slice(3, 6)]);
    let best = [pts[0], pts[0]], d = -1;
    for (let i = 0; i < pts.length; i++) for (let j = i; j < pts.length; j++) { const e = Math.hypot(pts[i][0] - pts[j][0], pts[i][1] - pts[j][1], pts[i][2] - pts[j][2]); if (e > d) { d = e; best = [pts[i], pts[j]]; } }
    return [...best[0], ...best[1], Math.max(...list.map((v) => v[6]))];
  };
  let missing = [];
  for (const [form, logical, clip] of wanted) {
    const it = item(clip);
    must(it, form + ' ' + logical + ': no item ' + clip);
    const groups = [...new Set((it.notifies || []).filter((n) => n.class === 'TresAnimNotifyState_Attack').map((n) => String((n.properties || {}).CollisionGrpName || 'None')))];
    for (const g of groups) {
      const list = SHAPES_OF(form).filter((s) => s.group === g);
      if (!list.length) { missing.push(clip + ':' + g); continue; }
      const frames = it.num_frames, len = it.duration_s;
      const values = [];
      for (let f = 0; f < frames; f++) {
        const t = frames > 1 ? f * len / (frames - 1) : 0;
        const each = list.map((s) => shapeAt(s, logical, clip, t));
        must(each.every((v) => v), form + ' ' + clip + ' ' + g + ': a socket of ' + list.map((s) => s.owner + ':' + s.socket1 + '/' + s.socket2).join(',') + ' was not found');
        values.push(...merge(each).map(r1));
      }
      const seqs = [...new Set(list.map((s) => (s.attach === 'OWNER_ATTACHED_MESH' ? 'sora' : weaponAnim(weapon(s.part.model), logical, clip).seq)))];
      add({ id: 'track.' + form + '.' + clip + '.' + g, what: 'track', form, code: frames, logical: g, clip, length: r4(len), flags: [...new Set(list.map((s) => s.owner + ':' + s.shape + ':' + s.socket1 + '-' + s.socket2 + '@' + s.part.socket))].concat(seqs), values,
        ref_addr: 'tools/sora_pack.js attackSockets', note: 'per frame [ax, ay, az, bx, by, bz, radius] (cm, actor space, his yaw 0) of group ' + g + ' (' + list.length + ' shape(s); radius -1 = a CONVEX at a: its shape row has the scale); flags = owner:shape:sockets@Sora\'s socket, then the weapons\' own clips (- = the reference pose)' });
    }
  }
  must(missing.length === 0, 'groups without a shape on the form\'s weapons: ' + missing.join(', '));
}
// the start tails (StartStyle 0x14141A9D0; the resume 0x14141E2B0 / 0x14141DD00 repeat them)
add({ id: 'start.HL', what: 'start', form: 'HL', code: 0x8C, logical: 'CHANGE_HL', values: [2], flags: ['auto_lockon_range', 'pawn_flag_0x40000'], ref_addr: 'StartStyle 0x14141A9D0 case 0x8C', note: HL.start_extra + ': ctrl+0x970 (the soft target\'s range, pawn+0xC88 MyAutoLockonLimit 900) x values[0]; the pawn flag (bit 18 of a movement word, cleared before the switch; its reader was not found)' });
add({ id: 'start.FL', what: 'start', form: 'FL', code: 0x8D, logical: 'CHANGE_FL', values: [0xA6], flags: ['preload_pawn', 'pawn_flag_0x40000'], ref_addr: 'StartStyle 0x14141A9D0 case 0x8D', note: FL.start_extra + ': pawn slot 502 0x141448D30 (values[0]) = load FINISH_FL\'s accompany pawn asset (the Kraken) ahead of the finish; the pawn flag as Highwind\'s' });
// the attack rows the forms hit with (p_ex001_AttackData)
const usedRows = new Set(ROWS.flatMap((r) => (['attack', 'chain', 'finish', 'pawn', 'proj'].includes(r.what) ? r.rows : r.what === 'shape' ? r.rows : [])).filter((x) => x && x !== 'None'));
for (const extra of ['SCHL_CHANGE', 'SCHL_CHANGE_END', 'SCFL_CHANGE', 'SCFL_CHANGE_END']) usedRows.add(extra);
for (const id of [...usedRows].sort()) {
  const a = atkRow(id);
  must(a, 'p_ex001_AttackData has no row ' + id);
  add({ id: 'row.' + id, what: 'row', form: /^SCFL_/.test(id) ? 'FL' : 'HL', logical: id, values: [a.m_Power, a.m_FormPoint, a.m_ReactionPowerValue], flags: [String(a.m_DamageKind).split('::').pop(), String(a.m_DamageAttribute).split('::').pop()], note: 'p_ex001_AttackData row ' + a.id + ': values = [m_Power, m_FormPoint, m_ReactionPowerValue]; the combat sheet sora-combat-attack-data must carry it for the hit to deal it (STATE.md INT line)' });
}

// ------------------------------------------------------------------------------------------------ checks
must(ROWS.filter((r) => r.what === 'attack').length === 14, 'expected 7 attack kinds per form');
for (const r of ROWS.filter((x) => x.what === 'attack')) must(r.clip !== '-' && r.length > 0, r.id + ': no clip');
must(ROWS.some((r) => r.id === 'attack.HL.0xA0' && r.flags.includes('shot@0.3333@HL_SPEAR')), 'M_E416 has the HL_SPEAR shot');
must(ROWS.some((r) => r.id === 'anim.FL.M_F420' && r.clip === 'F420'), 'Storm Flag has M_F420');
must(ROWS.some((r) => r.what === 'row' && r.logical === 'SCHL_CHANGE_END' && r.flags[1] === 'TRES_DMG_ATTR_WATER'), 'Highwind\'s water');
must(ROWS.some((r) => r.what === 'row' && r.logical === 'SCFL_FINISH' && r.flags[1] === 'TRES_DMG_ATTR_AERO'), 'Storm Flag\'s wind');

// ------------------------------------------------------------------------------------------------ sheet
const S = (n, t, g, doc) => [n, t, g, doc];
const COLS = [
  S('id', 'str', 'str', '<what>.<form>.<key>'), S('what', 'enum:attack|chain|decide|proj|rain|warp|anim|finish|pawn|shape|track|start|row|wnotify|aura', 'str', 'the row kind'), S('form', 'str', 'str', 'HL (Highwind, 0x8C) or FL (Storm Flag, 0x8D); ALL = every form and none (AUDIT-B W-CORE: the Final Blow aura rows)'),
  S('code', 'int', 'i32', 'attack kind (attack, chain, warp), finish sub-step (finish), the form command (decide, start), else 0'), S('logical', 'str', 'str', 'the logical anim name KH3 plays (attack / chain / anim / finish), the projectile (proj / rain), the blueprint (pawn), the group (shape), the row (row), the command (start)'),
  S('clip', 'str', 'str', 'the clip = the animation table row (the sequence); the owner (shape); the model (pawn); the class (proj)'), S('length', 'num', 'f32', 'the clip length, s'),
  S('root_len', 'num', 'f32', 'root length (+0x1D8), 0 = none'), S('stop_dist', 'num', 'f32', 'stop distance (+0x24C)'), S('z_gain', 'num', 'f32', 'z gain (+0x14C)'), S('dir_z', 'num', 'f32', 'DirZ written by the case (+0x1D4), 0 = none'),
  S('air_mode', 'bool', 'bool', '+0x1DF'), S('chan_xy', 'bool', 'bool', '+0x234 bit 0'), S('chan_root', 'bool', 'bool', '+0x234 bit 1'),
  S('flags', 'list', 'str[]', 'the case\'s other writes / the kind\'s strings (see the note)'), S('windows', 'list', 'str[]', 'the clip\'s hit windows a-b, s'), S('rows', 'list', 'str[]', 'the attack rows of those windows'),
  S('groups', 'list', 'str[]', 'the collision groups of those windows'), S('effects', 'list', 'str[]', 'the clip\'s effects (time@asset)'), S('sounds', 'list', 'str[]', 'the clip\'s sounds / voices (time@asset)'), S('values', 'list', 'f32[]', 'numbers by kind (see the note)'),
  S('ref_addr', 'str', 'str', 'decompiled function'), S('ref_conf', 'enum:confirmed|inferred|guess', 'str', 'confirmed = read in the code or the data; inferred; guess'), S('note', 'str', 'str', '-'),
];
for (const r of CMN_AURA) add(r); // (AUDIT-B W-CORE) after every other row: no row moves
const NAME = 'sora-form-ship';
const rows = ROWS.map((r) => COLS.map((c) => r[c[0]]));
for (const [i, r] of rows.entries()) for (const [k, v] of r.entries()) must(v !== undefined && v !== null && !(typeof v === 'number' && !Number.isFinite(v)), 'row ' + ROWS[i].id + ': cell ' + COLS[k][0] + ' is ' + v);
must(new Set(ROWS.map((r) => r.id)).size === ROWS.length, 'duplicate row ids');
fs.writeFileSync(P('sheets', 'port', NAME + '.json'), '{"sheet":"' + NAME + '","group":"port","columns":' + JSON.stringify(COLS.map((c) => c[0])) + ',"rows":[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
const by = {}; for (const r of ROWS) by[r.what] = (by[r.what] || 0) + 1;
console.log('sheets/port/' + NAME + '.json: ' + rows.length + ' rows ' + JSON.stringify(by));

// ------------------------------------------------------------------------------------------------ register (this tool's entry only; replaced in place)
const q = (s) => "'" + String(s).replace(/\\/g, '\\\\').replace(/'/g, "\\'") + "'";
const NL = '\n';
const DOC = 'KH3\'s Highwind and Storm Flag (Wheel of Fate) as data for their form package (kh-core combat/forms/ship.rs, states/combo_hl.rs, states/combo_fl.rs): COMBO_HL / COMBO_FL\'s attack kinds, the decision thresholds, the flag throw, the spear rain, the warp, the replacement anim sets, FINISH_FL (the Kraken), the collision shapes.';
const MARK = '(weapons F-SHIP)';
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
  const text = '  // ' + MARK + ' ' + DOC + ' Written by tools/sora_form_ship.js (documentation/KH3_WEAPONS_PLAN.md 1.3)' + NL +
    '  ' + q(NAME) + ": { group: 'port', file: " + q(NAME + '.json') + ", by: 'tools/sora_form_ship.js', from: " + q('sora-keyblade-forms, the form anim sets of the asset pack, the Kraken\'s clip, the weapon / transform-unit / style-pawn / projectile blueprints, p_ex001_AttackData and the decompiled PlayAttack / ComboHL / ComboFL / StyleFinishFL / StartStyle') + ', external: true, cols: [' + NL +
    '    ' + COLS.map((c, i) => '[' + q(c[0]) + ', ' + (i === 0 ? 'K' : 'V') + ', ' + q(c[1]) + ", '-', " + q(c[3]) + (['note', 'flags', 'windows', 'rows', 'groups', 'effects', 'sounds', 'values', 'logical', 'clip'].includes(c[0]) ? ', true' : '') + ']').join(', ') + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/sheet_defs.js');
}
{
  const f = P('tools', 'lib', 'port_gen_defs.js'); const before = fs.readFileSync(f, 'utf8');
  const END = NL + '    },' + NL;
  const pos = (t) => { const anchor = '  tables: {' + NL; const at = t.indexOf(anchor); must(at >= 0, 'port_gen_defs.js: anchor tables'); return at + anchor.length; };
  const cols = COLS.filter((c) => c[2]).map((c) => c[0] + ': { type: ' + q(c[2]) + (c[3] && c[3] !== '-' ? ', doc: ' + q(c[3]) : '') + ' }').join(', ');
  const text = '    // ' + MARK + ' ' + NAME + ': written by tools/sora_form_ship.js (documentation/KH3_WEAPONS_PLAN.md 1.3)' + NL +
    '    ' + q(NAME) + ': {' + NL +
    "      file: 'sora_form_ship.rs', row: 'FormShipRow', table: 'FORM_SHIP', key: 'id', key_prefix: 'FSHIP'," + NL +
    '      doc: ' + q(DOC) + ',' + NL +
    '      columns: { ' + cols + ' },' + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/port_gen_defs.js');
}
console.log('sora_form_ship ' + VERSION);
