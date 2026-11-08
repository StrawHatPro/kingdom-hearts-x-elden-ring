// (weapons F-NANO) KH3's Nano Arms (form.GM, CHANGE_GM 0x86, Nano Gear) as DATA for its form package (kh-core
// combat/forms/nano.rs, states/combo_gm.rs): the attack kinds of COMBO_GM (PlayAttack cases 0xB5 .. 0xBD and the Nano
// Counter 0x25), the replacement anim set AnimSet8 (logical name -> clip), the weapon's hit shapes (w_so172_Wep
// TresAtkColl0) tracked per clip frame, the projectiles (the Rocket Fist BX_ROCKET, the Nano Counter BARRIERCRACKER),
// FINISH_GM (StyleFinishGM: the gate w_so174, its own clips' hit windows and shapes) and the attack rows.
// documentation/KH3_WEAPONS_PLAN.md 1.3 / 4 (package F-NANO), build/sora_tmp/weapons/F-NANO/STATE.md.
//
//   node tools/sora_form_nano.js   write sheets/port/sora-form-nano.json and register it (once, in place) in
//                                  tools/lib/sheet_defs.js and tools/lib/port_gen_defs.js; then node tools/gen_port.js
//
// Inputs (read only): sheets/port/sora-keyblade-forms.json (P3), build/sora_pack/keyblades/sora_forms_manifest.json (the
// asset agent's pack of AnimSet8), build/sora_pack/manifest.json (the base set), build/sora_pack/keyblades/w_so172 / w_so174
// (model.json, sockets.json: the weapon and the gate), the converted animations and blueprints (assets_converted), sheets/
// entities/DataTable/Player/Attack/p_ex001_AttackData.json, and THE CODE TABLES BELOW, read in the decompiled exe (re/decomp):
//   PLAY     TresPlayerState_Attack::PlayAttack 0x1413378B0 cases 0xB5 .. 0xBD and 0x25 (the GM branch): logical name,
//            the case's writes; `break` = the air mode (+0x1DF = 1, the line after the switch), `goto caseD_26` = not.
//   DECIDE   TresPlayerState_ComboGM::Decide 0x141361870 (vtable 0x14690EBF0 slot 122): in the Rust (combat/forms/nano.rs).
//   HOOK     the hookshot (kind 0xBC -> 0xBD): the on-contact handler 0x141355780 of ComboGM's notify object.
//   SHOT     the shot handler 0x141344580: case 0xB9 = the notify's projectile (BX_ROCKET) with the homing target, case
//            0x25 = BARRIERCRACKER (the projectile set w_so170_ProjSet maps both names).
//   FINISH   TresPlayerState_StyleFinishGM (id 36; TryStyleFinish 0x141447840 case 0xA0): Reset 0x1413D0DC0, Begin
//            0x1413CC360, Tick 0x1413D5660, End 0x1413CF170.
//   SPEED    TresProjectileMovementComponent ctor 0x141470A90: InitialSpeed (+0x248) = MaxSpeed (+0x24C) = 2000.
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
const r1 = (x) => Math.round(x * 10) / 10;

// ------------------------------------------------------------------------------------------------ inputs
const sheet = (name) => { const s = readJson(P('sheets', 'port', name + '.json')); return s.rows.map((r) => Object.fromEntries(s.columns.map((c, i) => [c, r[i]]))); };
const GM = sheet('sora-keyblade-forms').find((r) => r.id === 'form.GM');
must(GM && GM.cmd === 0x86, 'sora-keyblade-forms has no form.GM (0x86)');
const SET = 'p_ex001_AnimSet8';
must(GM.anim_sets.some((s) => s.includes(SET)), 'form.GM does not replace with ' + SET);
const OV = readJson(P('build', 'sora_pack', 'keyblades', 'sora_forms_manifest.json'));
const MAIN = readJson(P('build', 'sora_pack', 'manifest.json'));
const ATK = readJson(P('sheets', 'entities', 'DataTable', 'Player', 'Attack', 'p_ex001_AttackData.json')).rows;
const ITEMS = new Map([...MAIN.items, ...OV.items].filter((i) => i.kind === 'animation').map((i) => [i.animation_name, i]));
const item = (clip) => ITEMS.get(clip) || null;
// the logical name a main-pack sequence is played under (1341 -> MagicCast): the row of the animation table to play it
const MAIN_LOGICAL = new Map();
for (const a of MAIN.logical_animations) if (a.file && !a.samples) { const seq = path.basename(a.file, '.glb'); if (!MAIN_LOGICAL.has(seq)) MAIN_LOGICAL.set(seq, a.name); }
const SET8 = OV.logical_animations.filter((a) => String(a.anim_set).endsWith('/' + SET));
must(SET8.length > 20, SET + ': not in the forms manifest');
// logical -> { clip: the animation-table row that plays it, seq: the sequence }
function rowOf(a) {
  if (a.file) return { clip: a.clip, seq: a.clip };                     // packed by the forms overlay: its row is named by the sequence
  if (MAIN_LOGICAL.has(a.clip)) return { clip: MAIN_LOGICAL.get(a.clip), seq: a.clip }; // a sequence of the main pack (1341 = MagicCast)
  return { clip: '-', seq: a.clip };
}
const ANIM = new Map(SET8.map((a) => [a.name, { ...rowOf(a), cls: a.class }]));
const prop = (n, key) => { const p = n.properties || {}; const v = p[key]; return v && typeof v === 'object' && 'value' in v ? v.value : v; };
// the notifies of a clip (sequence) for the rows: hit windows a-b, their attack rows @ group, TresEnableChange / Input, shots
function notes(seq) {
  const it = item(seq);
  const out = { length: it ? r4(it.duration_s) : 0, frames: it ? it.num_frames : 0, windows: [], rows: [], change: -1, input: -1, shots: [], interval: [] };
  if (!it) return out;
  for (const n of it.notifies || []) {
    if (n.class === 'TresAnimNotifyState_Attack') { out.windows.push(r4(n.time) + '-' + r4(n.end_time)); out.rows.push(String(prop(n, 'm_AttackDataIDName')) + '@' + String(prop(n, 'CollisionGrpName'))); out.interval.push(r4(+(prop(n, 'AttackInterval') || 0))); }
    if (n.class === 'TresAnimNotify_EnableChange' && out.change < 0) out.change = r4(n.time);
    if (n.class === 'TresAnimNotify_EnableInput' && out.input < 0) out.input = r4(n.time);
    if (n.class === 'TresAnimNotify_Shot') out.shots.push(r4(n.time) + '@' + (prop(n, 'm_ProjectileName') || 'None'));
  }
  return out;
}
const atkRow = (id) => ATK.find((r) => r._row_name === id);

// ------------------------------------------------------------------------------------------------ the code tables
// PLAY (0x1413378B0, decompiled C). Defaults before the switch: m_bFaceAt, +0x234 = (x & ~6) | 1 (the XY channel on, the
// root-scale channel off), stop distance +0x24C = 160, z gain +0x14C = 3, +0x1DF = 0, +0x198 (effect group) = 0. The cases:
const PLAY = [
  { kind: 0xB5, logical: 'M_8411', air: true, chan_root: true, root_len: 1000, stop: 350, hold: 0.3, flags: ['x190'], label: 'finisher 1, ground (Nano Crash: the fist slam)', at: '0x141338D1D' },
  { kind: 0xB6, logical: 'M_8412', air: true, chan_root: true, root_len: 1100, stop: 350, voice: 2, label: 'finisher 1, air (the giga drill stab)', at: '0x141338D76' },
  { kind: 0xB7, logical: 'M_8413', air: true, voice: 2, flags: ['no_vz'], label: 'finisher 2, ground (the giant stamp)', at: '0x141338DC6' },
  { kind: 0xB8, logical: 'M_8414', air: true, root_len: 500, stop: 350, label: 'finisher 2, air (the yo-yos)', at: '0x141338E04' },
  { kind: 0xB9, logical: 'M_8415', air: true, chan_xy: false, flags: ['shot'], label: 'finisher 3 (the punch and the Rocket Fist: Shot BX_ROCKET)', at: '0x141338E3A' },
  { kind: 0xBA, logical: 'M_8419', air: true, stop: 300, flags: ['bit16'], label: 'the rise (target above: the flail swing up)', at: '0x141338E61' },
  { kind: 0xBB, logical: 'M_8420', air: true, dive: true, dir_z: -4000, flags: ['no_vz', 'keep_dir_z'], label: 'the fall (target below: the beat and the stamp down)', at: '0x141338E8A' },
  { kind: 0xBC, logical: 'M_8423', air: true, root_len: 100, flags: ['hookshot', 'mark_start'], label: 'far (> 400 cm): the hookshot (the yo-yo shot at the target; a hit on it pulls him in: 0xBD)', at: '0x141338CA4' },
  { kind: 0xBD, logical: 'M_8424', air: true, flags: ['hook_end'], label: 'the hookshot\'s kick (played by the on-contact handler 0x141355780; PlayAttack case 0xBD plays the same clip)', at: '0x14133A1F0' },
  // the GM branch of case 0x25 (forms 0x86, 0x87, 0x8E, 0x8F, 0x92, 0x93; 0xDD / 0xDE other groups; 0xDF / 0xE0 M_N426):
  // the ground / air clip (air: +0x1DF), +0x198 = 0xD48, +0x19C (voice) = 0x34, +0x164 = 1, +0x234 |= 0x2800, the
  // invincibility of the tail (pawn vt+0x2068(1)); the shot of its TresShot: BARRIERCRACKER (shot handler case 0x25).
  { kind: 0x25, logical: 'BarrierCracker', air_logical: 'AirBarrierCracker', chan_xy: true, group: 0xD48, voice: 0x34, flags: ['shot', 'invincible', 'swing_started'], label: 'Nano Counter: the guard counter (TryAttack 0x141437E28: forced kind 0x25 in 0x86)', at: '0x1413396A2' },
];
// FINISH: StyleFinishGM Tick 0x1413D5660 by sub-step (Sora's clip / the gate's clip).
const FINISH = [
  { sub: 0, sora: 'M_8416', gate: 'M_8416', flags: ['spawn_gate', 'flying', 'effect_group_0x66D'], what: 'the gate (TresStylePawnGMGate w_so174, m_ReplaceAccompanyPawnAsset of FINISH_GM) spawned at Sora\'s location and rotation, linked (pawn vt+0x23A8(gate, "X_link01_skt")), shown; both play M_8416; Sora Flying (vt+0x17A0(5, 1)); +0x198 = 0x66D, the anim\'s effect group 0x66D; +0x2E8 = 0' },
  { sub: 2, sora: 'M_8417', gate: 'M_8417', flags: ['weapon_away', 'sora_hidden', 'world_slow_off'], what: 'at Sora\'s clip end: the weapon put away (vt+0x1340(0, 0, 1.0)), Sora HIDDEN (vt+0xD98(0)); both play M_8417 (the gate\'s hit windows FinishGM / FinishGM2), gate +0xB48 = 0; the world slow off (0x14140E500(ctrl, 0))' },
  { sub: 5, sora: 'M_8418', gate: 'M_8418', flags: ['sora_shown'], what: 'at Sora\'s clip end: Sora shown (vt+0xD98(1)); both play M_8418' },
  { sub: 6, sora: '-', gate: '-', flags: ['end'], what: 'at Sora\'s clip end: slot 0x270 (DoChain); End 0x1413CF170: +0x2EC once -> vt+0x1038(0x42); vt+0x23B0(0) (detach); the gate\'s end (vt+0x2E8, gate vt+0x2590); the effect group 0x198 ended on the weapons and on Sora' },
];
// HOOK (0x141355780): the on-contact handler of kinds 0xBC / 0xBD. 0xBD: +0x84 |= 1. 0xBC with the victim = the attack's
// homing target: the turn back (+0x1F4 bit 5), the warp point by slot 136 (ComboGM 0x141337890 = Attack 0x141337320 with 150,
// 100), FUN_1414466F0 (the pawn there), kind 0xBD, start time (1000 - |warp - him|xy) x 0.000333 >= 0, stop 160, z gain 3,
// +0x234 = (x & ~2) | 5 | 0x10 | 0x800, phases 0, air mode, M_8424 (blend 0, mode 2), GravityScale 0.1, Flying, +0x18C 0.6,
// m_bEnableInput, m_bFaceAt, +0x164 = 1, pawn+0xE28 = 1 (the hit confirm), invincible (vt+0x2068(1)), vt+0x16D8(1).
const HOOK = { distance: 150, drop: 100, start_from: 1000, start_rate: 0.000333, stop: 160, z_gain: 3, gravity_scale: 0.1, hold: 0.6 };

// ------------------------------------------------------------------------------------------------ rows
const ROWS = [];
const blank = { code: 0, logical: '-', clip: '-', length: 0, root_len: 0, stop_dist: 160, z_gain: 3, air_mode: false, chan_xy: true, chan_root: false, flags: [], windows: [], rows: [], values: [], ref_addr: '-', ref_conf: 'confirmed', note: '-' };
const add = (r) => ROWS.push({ ...blank, ...r });
const clipOf = (logical) => { const a = ANIM.get(logical); if (a) return a; const b = MAIN.logical_animations.find((x) => x.name === logical); return b && b.file ? { clip: logical, seq: path.basename(b.file, '.glb') } : { clip: '-', seq: '-' }; };
for (const k of PLAY) {
  for (const air of k.air_logical ? [false, true] : [k.air || false]) {
    const logical = air && k.air_logical ? k.air_logical : k.logical;
    const c = clipOf(logical);
    must(c.clip !== '-', hex(k.kind) + ' ' + logical + ': no clip in the pack');
    const n = notes(c.seq);
    for (const r of n.rows) must(atkRow(r.split('@')[0]), logical + ': p_ex001_AttackData has no row ' + r);
    add({ id: 'attack.' + hex(k.kind) + (k.air_logical ? (air ? '.air' : '.ground') : ''), what: 'attack', code: k.kind, logical, clip: c.clip, length: n.length, root_len: k.root_len || 0, stop_dist: k.stop || 160, z_gain: 3, air_mode: air || !!k.air && !k.air_logical, chan_xy: k.chan_xy !== false, chan_root: !!k.chan_root,
      flags: [...(k.flags || []), ...(k.dive ? ['dive'] : [])], windows: n.windows, rows: n.rows, values: [k.dir_z || 0, k.hold || 0, k.voice || 0, k.group || 0, n.change, n.input, ...n.interval], ref_addr: k.at,
      note: k.label + '; sequence ' + c.seq + (n.shots.length ? '; shots ' + n.shots.join(' ') : '') + '; values = [DirZ (+0x1D4, 0 = none), +0x18C (0 = the air-mode 0.6), voice group (+0x19C), effect group (+0x198), TresEnableChange, TresEnableInput, then each window\'s AttackInterval]' });
  }
}
// the replacement anim set (logical name -> the animation-table row); '-' = not packed
for (const [logical, a] of [...ANIM].sort((x, y) => (x[0] < y[0] ? -1 : 1))) {
  const it = a.seq !== '-' ? item(a.seq) : null;
  add({ id: 'anim.' + logical, what: 'anim', logical, clip: a.clip, length: it ? r4(it.duration_s) : 0, flags: [a.seq, a.cls], note: a.clip === '-' ? 'not packed: the base clip plays' : 'AnimSet8 ' + logical + ' = sequence ' + a.seq + (a.clip !== a.seq ? ' (the main pack\'s row ' + a.clip + ')' : '') });
}
// the finish command
for (const f of FINISH) {
  const c = f.sora === '-' ? { clip: '-', seq: '-' } : clipOf(f.sora);
  const n = c.seq !== '-' ? notes(c.seq) : null;
  add({ id: 'finish.' + f.sub, what: 'finish', code: f.sub, logical: f.sora, clip: c.clip, length: n ? n.length : 0, flags: f.flags, values: [], ref_addr: '0x1413D5660', note: f.what });
}
// code constants
add({ id: 'code.combo', what: 'code', logical: 'MaxCombo / Finishers', values: [6, 3], ref_addr: '0x141433570 / 0x141433A60', note: 'case 0x86: MaxCombo 6, Finishers 3' });
add({ id: 'code.hook', what: 'code', logical: 'hookshot', values: [HOOK.distance, HOOK.drop, HOOK.start_from, HOOK.start_rate, HOOK.stop, HOOK.z_gain, HOOK.gravity_scale, HOOK.hold], ref_addr: '0x141355780', note: 'values = [warp distance short of the target, below it (slot 136 args), start-time formula (from - dxy) x rate, stop distance, z gain, gravity scale, +0x18C]' });
add({ id: 'code.finish', what: 'code', logical: 'FINISH_GM', values: [0x66D, 0x42], ref_addr: '0x1413D5660 / 0x1413CF170', note: 'values = [the effect group of the finish (+0x198), End\'s vt+0x1038 argument]' });

// ------------------------------------------------------------------------------------------------ the shapes
const BP = P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content', 'Blueprints');
const flat = (props) => { const o = {}; for (const p of props || []) o[p.name] = p.type === 'StructProperty' ? (Array.isArray(p.value) ? flat(p.value) : p.value) : p.type === 'ArrayProperty' ? (p.value && p.value.items ? p.value.items.map((it) => (Array.isArray(it) ? flat(it) : it)) : p.value) : p.value; return o; };
const v3 = (o) => (o ? [o.x || 0, o.y || 0, o.z || 0] : [0, 0, 0]);
function shapes(rel) {
  const d = readJson(path.join(BP, rel + '.uasset.json'));
  const out = [];
  for (const e of d.exports) {
    if (e.class !== 'TresAtkCollComponent') continue;
    const p = flat((e.data && e.data.props) || e.props);
    for (const s of p.CollisionShapesSrc || []) out.push({ owner: rel.split('/').pop(), group: s.m_GrpName, shape: String(s.m_ShapeType).split('::').pop(), size: v3(s.m_Size), inc: v3(s.m_IncSize), max: v3(s.m_IncMaxSize), socket1: s.m_SocketName1 || 'None', socket2: s.m_SocketName2 || 'None', loc1: v3(s.m_RelativeLocation1), loc2: v3(s.m_RelativeLocation2), fixed: !!s.m_bDisableLocation1Attach, map: String(s.m_MapHitType || '').split('::').pop(), row: s.m_DefaultAttackDataIDName || 'None',
      // (F-SHAPES) what the CONVEX transform reads (Update 0x140647950 case 6 / 8; combat/hull.rs)
      attach: String(s.m_AttachType1 || 'NORMAL').split('::').pop(), mesh: s.m_Mesh < 0 && d.imports[-s.m_Mesh - 1] ? d.imports[-s.m_Mesh - 1].objectName : '-', scale: v3(s.m_Scale), inc_scale: v3(s.m_IncScale), max_scale: v3(s.m_IncMaxScale),
      rot: s.m_RelativeRocation ? { pitch: s.m_RelativeRocation.pitch || 0, yaw: s.m_RelativeRocation.yaw || 0, roll: s.m_RelativeRocation.roll || 0 } : { pitch: 0, yaw: 0, roll: 0 },
      rot_attach: String(s.m_RotAttachType || 'TRES_ACRA_NORMAL').split('::').pop(), abs1: s.m_bAbsoluteOffset1 === true, ignore_scale: s.m_bIgnoreParentScale === true });
  }
  return out;
}
const WEP_SHAPES = shapes('Player/p_ex001/w_so172_Wep');
const GATE_SHAPES = shapes('StylePawn/w_so174');
for (const s of [...WEP_SHAPES, ...GATE_SHAPES]) add({ id: 'shape.' + s.owner + '.' + s.group, what: 'shape', logical: s.group, clip: s.owner, flags: [s.shape, s.socket1, s.socket2, s.map, s.fixed ? 'fixed' : 'follow'], values: [s.size[0], s.inc[0], s.max[0], ...s.loc1, ...s.loc2], rows: [s.row], note: 'TresAtkColl0 CollisionShapesSrc: flags = [shape, socket 1, socket 2, map hit type, m_bDisableLocation1Attach], values = [size x, inc x, inc max x, location 1 xyz, location 2 xyz]' });

// ------------------------------------------------------------------------------------------------ the shape tracks
// For every clip of the form with hit windows, every group its windows name: one capsule per animation frame (time = frame
// x length / (frames - 1)), [ax, ay, az, bx, by, bz] in Sora's actor space (UE cm, his yaw 0). A socket of the weapon
// (w_so172: build/sora_pack/keyblades/w_so172/sockets.json, else a bone of its skeleton) is placed by Sora's bone R_buki ->
// R_wep_skt (the form's unit socket) -> the weapon skeleton posed by its own clip of the same sequence (else its reference
// pose: the combo clips 830x have none); a socket the weapon lacks (center, vfx_cure) is Sora's own (as combat/hit.rs does
// for the Kingdom Key's component). The forward kinematics of tools/sora_pack.js attackSockets (copied, as the sibling
// packages do). The gate's groups (FinishGM / FinishGM2 on wepA_skt) are tracked in the gate's actor space by its M_8417.
const sp = require('./sora_pack.js');
const qmulH = (a, b) => [a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1], a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0], a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3], a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]];
const qrot = (q, v) => { const tx = 2 * (q[1] * v[2] - q[2] * v[1]), ty = 2 * (q[2] * v[0] - q[0] * v[2]), tz = 2 * (q[0] * v[1] - q[1] * v[0]); return [v[0] + q[3] * tx + (q[1] * tz - q[2] * ty), v[1] + q[3] * ty + (q[2] * tx - q[0] * tz), v[2] + q[3] * tz + (q[0] * ty - q[1] * tx)]; };
const under = (c, p) => { const v = qrot(p.q, [p.s[0] * c.t[0], p.s[1] * c.t[1], p.s[2] * c.t[2]]); return { q: qmulH(p.q, c.q), t: [v[0] + p.t[0], v[1] + p.t[1], v[2] + p.t[2]], s: [p.s[0] * c.s[0], p.s[1] * c.s[1], p.s[2] * c.s[2]] }; };
const point = (T, v) => { const r = qrot(T.q, [T.s[0] * v[0], T.s[1] * v[1], T.s[2] * v[2]]); return [r[0] + T.t[0], r[1] + T.t[1], r[2] + T.t[2]]; };
const rotQ = (r) => { const h = Math.PI / 360, SP = Math.sin(r.pitch * h), CP = Math.cos(r.pitch * h), SY = Math.sin(r.yaw * h), CY = Math.cos(r.yaw * h), SR = Math.sin(r.roll * h), CR = Math.cos(r.roll * h); return [CR * SP * SY - SR * CP * CY, -CR * SP * CY - SR * CP * SY, CR * CP * SY - SR * SP * CY, CR * CP * CY + SR * SP * SY]; };
const ID = MAIN.identification;
const SORA_BONES = sp.buildSkeleton(sp.readGlb(P('assets_converted', 'meshes', sp.CONTENT, sp.gamePathToContent(ID.skeletal_mesh) + '.glb')).json);
const SORA_SOCKETS = {};
for (const e of sp.loadPkg(sp.gamePathToContent(ID.skeleton)).exports.filter((x) => x.class === 'SkeletalMeshSocket')) {
  const p = sp.propsOf(e);
  const loc = p.RelativeLocation ? p.RelativeLocation.value : { x: 0, y: 0, z: 0 }, rot = p.RelativeRotation ? p.RelativeRotation.value : { pitch: 0, yaw: 0, roll: 0 }, sc = p.RelativeScale ? p.RelativeScale.value : { x: 1, y: 1, z: 1 };
  SORA_SOCKETS[p.SocketName.value] = { bone: p.BoneName.value, T: { q: rotQ(rot), t: [loc.x, loc.y, loc.z], s: [sc.x, sc.y, sc.z] } };
}
const yaw = ID.mesh_component.relative_rotation_ue_deg.yaw * Math.PI / 180, cy = Math.cos(yaw), sy = Math.sin(yaw);
const actor = (v) => [v[0] * cy - v[1] * sy, v[0] * sy + v[1] * cy, v[2]];
const localT = (b, tr, time) => ({ q: tr.rotation ? sp.sampleChan(tr.rotation, time)(tr.rotation.ue_values) : b.ue.rotation, t: tr.translation ? sp.sampleChan(tr.translation, time)(tr.translation.ue_values) : b.ue.translation, s: tr.scale ? sp.sampleChan(tr.scale, time)(tr.scale.ue_values) : b.ue.scale });
const byBone = (a) => { const m = new Map(); if (a) for (const c of a.chans) { if (!m.has(c.bone)) m.set(c.bone, {}); m.get(c.bone)[c.path] = c; } return m; };
const boneT = (bones, chans, k, time, rootMotion) => { const chain = []; for (let i = k; i >= 0; i = bones[i].parent) chain.unshift(i); let T = null; for (const i of chain) { const l = localT(bones[i], (i === 0 && rootMotion) ? {} : (chans.get(i) || {}), time); T = T ? under(l, T) : l; } return T; };
const loadA = (asset, bones) => { try { return sp.loadAnimation(asset, bones); } catch (e) { return null; } };
const soraAnims = new Map();
const soraAnim = (seq) => { if (!soraAnims.has(seq)) { const it = item(seq); soraAnims.set(seq, it && it.source_asset ? loadA(it.source_asset + '.' + path.basename(it.source_asset), SORA_BONES) : null); } return soraAnims.get(seq); };
function model(name) {
  const dir = P('build', 'sora_pack', 'keyblades', name);
  const mj = readJson(path.join(dir, 'model.json')), sj = readJson(path.join(dir, 'sockets.json'));
  return { mj, sockets: sj.weapon_sockets || {}, bones: sp.buildSkeleton(sp.readGlb(P('assets_converted', 'meshes', sp.CONTENT, sp.gamePathToContent(mj.source_mesh) + '.glb')).json), anims: new Map() };
}
const WEP = model('w_so172');
const GATE = model('w_so174');
const modelAnim = (w, seq) => { if (!w.anims.has(seq)) { const c = (w.mj.clips || []).find((x) => x.sequence === seq); w.anims.set(seq, c ? loadA(c.asset + '.' + c.sequence, w.bones) : null); } return w.anims.get(seq); };
// the unit's socket on Sora: the row's `units` (RWeapon@RIGHT_HAND:R_wep_skt=...)
const UNIT_SOCKET = GM.units[0].match(/:(\w+)=/)[1];
const soraSocketT = (name, seq, time) => {
  const sa = soraAnim(seq); if (!sa) return null;
  const sock = SORA_SOCKETS[name];
  const k = SORA_BONES.findIndex((b) => b.name === (sock ? sock.bone : name));
  if (k < 0) return null;
  const T = boneT(SORA_BONES, byBone(sa), k, Math.min(time, sa.length), sa.props.bEnableRootMotion === true);
  return sock ? under(sock.T, T) : T;
};
// a socket of a model (its sockets.json, else its bone) under `base`, the model posed by its clip `seq` at `time`
const modelSocketT = (w, name, seq, time, base) => {
  const ws = w.sockets[name];
  const k = w.bones.findIndex((b) => b.name === (ws ? ws.bone : name));
  if (k < 0) return null;
  const a = modelAnim(w, seq);
  let T = under(boneT(w.bones, byBone(a), k, a ? Math.min(time, a.length) : 0, false), base);
  if (ws) T = under({ q: rotQ({ pitch: ws.local_rotation[0], yaw: ws.local_rotation[1], roll: ws.local_rotation[2] }), t: ws.local_location, s: [1, 1, 1] }, T);
  return T;
};
// one end of a weapon shape in Sora's actor space: the weapon's socket, else Sora's own socket / bone
// THE WEAPON'S OWN CLIPS ARE AUTHORED IN SORA'S MESH SPACE: their R_wep follows Sora's R_buki in mesh space (checked frame by
// frame: M_8423 R_wep - R_buki < 5 cm), the transform unit is "None@RIGHT_HAND:None" (no socket = the mesh origin). So a clip
// the weapon has its own sequence for poses it from Sora's mesh origin; the others (the combo 830x) at its reference pose on
// the unit's socket (R_wep_skt), as the Kingdom Key.
const IDENTITY = { q: [0, 0, 0, 1], t: [0, 0, 0], s: [1, 1, 1] };
// (AUDIT-B W-HANDOVER) ... BUT ONLY INSIDE ITS CLIP'S TresAnimNotifyState_ChangeAttachParent WINDOW. KH3: the notify's begin
// 0x1405633F0 -> the weapon's anim interface vt+0x370 = 0x1417873F0: AttachToComponent(m_EquipAttachParent, rules,
// m_SocketName kh_root = his mesh origin); its end 0x140565500 -> vt+0x378 = 0x141785E90: AttachToComponent(m_EquipAttachParent,
// rules, weapon+0x688 = the socket the equipment gave it, 0x14177E4E0: the unit's R_wep_skt). A window that closes before its
// clip ends (8415: 1.1667 of 1.7333 s; 8483: 0.6666 of 0.7 s) leaves a TAIL in which the clip poses the weapon from that
// socket again, and every effect attached to the weapon (ToAttach) moves with it. kh-core combat/forms/placement.rs
// part_parent / clip_parent is the same window for the hosts. A window opening at 0.0001 s is open from the clip's first
// tick (UE fires a notify state's begin on the tick whose range covers its trigger time), so frame 0 is inside it.
const WEP_WINDOWS = new Map();
for (const c of WEP_CLIPS_OF()) for (const n of (c.notifies || []).filter((x) => x.class === 'TresAnimNotifyState_ChangeAttachParent')) WEP_WINDOWS.set(c.sequence, [+n.trigger_time, +n.end_trigger_time]);
function WEP_CLIPS_OF() { return readJson(P('build', 'sora_pack', 'keyblades', 'w_so172', 'model.json')).clips || []; }
const inWindow = (seq, t) => { const w = WEP_WINDOWS.get(seq); return !w || (t >= (w[0] <= 1e-3 ? 0 : w[0]) && t < w[1]); };
// the weapon's frame for its own clip of `seq` at `t`: his mesh origin inside the window, the unit's socket after it
const wepBase = (seq, t) => (inWindow(seq, t) ? IDENTITY : soraSocketT(UNIT_SOCKET, seq, t) || IDENTITY);
let TAIL_FRAMES = 0;
const weaponEnd = (socket, loc, seq, time) => {
  const hand = modelAnim(WEP, seq) ? wepBase(seq, time) : soraSocketT(UNIT_SOCKET, seq, time); if (!hand) return null; // (AUDIT-B W-HANDOVER) wepBase
  const T = modelSocketT(WEP, socket, seq, time, hand) || soraSocketT(socket, seq, time);
  return T ? actor(point(T, loc)) : null;
};
const gateEnd = (socket, loc, seq, time) => { const T = modelSocketT(GATE, socket, seq, time, IDENTITY); return T ? actor(point(T, loc)) : null; };
// (F-SHAPES) a CONVEX's real transform (combat/hull.rs `VolTrack` HULL; Update 0x140647950 case 6 / 8): T1 = the socket (NORMAL the
// weapon's, as `weaponEnd`; OWNER_ATTACHED_MESH Sora's); location = T1.t + (m_bAbsoluteOffset1 ? relLoc x T1.s : T1.q (relLoc x
// T1.s)); rotation = LOCAL ? relRot (world) : T1.q x relRot; scale = FUN_140649BF0(t) x T1.s. `at` = the clip time of the place
// (the window's start for m_bDisableLocation1Attach), `t` = the time since the window opened.
const MESH_YAW_Q = [0, 0, Math.sin(yaw / 2), Math.cos(yaw / 2)];
const growScale = (s, t) => { if (s.inc_scale.every((x) => x === 0)) return s.scale; return s.scale.map((b, i) => { const v = Math.max(b + s.inc_scale[i] * t, 0.01); return s.max_scale[i] > 0 ? Math.min(v, s.max_scale[i]) : v; }); };
const hullAt = (s, seq, at, t) => {
  let T1 = null;
  if (s.attach === 'OWNER_ATTACHED_MESH') T1 = soraSocketT(s.socket1, seq, at);
  else { const hand = modelAnim(WEP, seq) ? wepBase(seq, at) /* (AUDIT-B W-HANDOVER) */ : soraSocketT(UNIT_SOCKET, seq, at); T1 = hand ? modelSocketT(WEP, s.socket1, seq, at, hand) || soraSocketT(s.socket1, seq, at) : null; }
  if (!T1) return null;
  const sl = [s.loc1[0] * T1.s[0], s.loc1[1] * T1.s[1], s.loc1[2] * T1.s[2]];
  const off = s.abs1 ? sl : qrot(T1.q, sl);
  const c = actor([T1.t[0] + off[0], T1.t[1] + off[1], T1.t[2] + off[2]]);
  const q = s.rot_attach === 'TRES_ACRA_LOCAL' ? rotQ(s.rot) : qmulH(MESH_YAW_Q, qmulH(T1.q, rotQ(s.rot)));
  const n = Math.hypot(...q);
  const g = growScale(s, t);
  const sc = s.ignore_scale ? g : g.map((x, i) => x * T1.s[i]);
  return [...c.map(r1), ...q.map((x) => Math.round(x / n * 1e6) / 1e6), ...sc.map(r4)];
};
let tracks = 0;
function track(owner, seq, list, end) {
  const it = owner === 'gate' ? null : item(seq);
  const a = owner === 'gate' ? modelAnim(GATE, seq) : null;
  const frames = it ? it.num_frames : a.numFrames, length = it ? it.duration_s : a.length;
  const groups = owner === 'gate' ? [...new Set(a.notifies.filter((n) => n.class === 'TresAnimNotifyState_Attack').map((n) => String(prop(n, 'CollisionGrpName'))))] : [...new Set((it.notifies || []).filter((n) => n.class === 'TresAnimNotifyState_Attack').map((n) => String(prop(n, 'CollisionGrpName') ?? 'None')))];
  for (const g of groups) {
    const s = list.find((x) => x.group === g);
    must(s, seq + ': its window group ' + g + ' is not a shape of ' + (owner === 'gate' ? 'w_so174' : 'w_so172_Wep'));
    // (F-SHAPES) a CONVEX of the weapon: its hull per frame (`vol` row, combat/hull.rs) instead of a sphere of its m_Size
    if (owner !== 'gate' && s.shape === 'CONVEX') {
      const mine = (it.notifies || []).filter((n) => n.class === 'TresAnimNotifyState_Attack' && String(prop(n, 'CollisionGrpName') ?? 'None') === g).map((n) => ({ a: +n.time, b: +n.end_time }));
      const vals = [];
      for (let f = 0; f < frames; f++) {
        const t = frames > 1 ? f * length / (frames - 1) : 0;
        const w = mine.filter((x) => x.a <= t + 1e-6).pop() || mine[0];
        const h = hullAt(s, seq, s.fixed ? w.a : t, Math.max(t - w.a, 0));
        must(h, seq + ' ' + g + ': socket ' + s.socket1 + ' could not be placed');
        vals.push(...h);
      }
      add({ id: 'vol.sora.' + seq + '.' + g + '.0', what: 'vol', code: frames, logical: g, clip: seq, length: r4(length), flags: ['HULL', s.mesh, s.rot_attach === 'TRES_ACRA_LOCAL' ? 'world' : 'actor', 'sora', s.shape + ':' + s.socket1 + ':' + s.attach + (s.fixed ? ':fixed' : '') + ':' + s.rot_attach], windows: mine.map((w) => r4(w.a) + '-' + r4(w.b)), values: vals, ref_addr: '0x140647950',
        note: 'group ' + g + ' per frame (combat/hull.rs VolTrack): flags = [HULL, the CONVEX mesh, world | actor (the frame of the rotation: m_RotAttachType LOCAL = relRot in the world), owner, what it is]; [x, y, z, qx, qy, qz, qw, sx, sy, sz] = Update 0x140647950 case 6\'s location / rotation / scale (FUN_140649BF0 growth from the window\'s start) in Sora\'s actor space (his yaw 0)' });
      tracks += 1;
      continue;
    }
    const values = [];
    for (let f = 0; f < frames; f++) {
      const t = frames > 1 ? f * length / (frames - 1) : 0;
      const e1 = end(s.socket1, s.loc1, seq, t);
      const e2 = s.shape === 'CAPSULE2' && s.socket2 !== 'None' ? end(s.socket2, s.loc2, seq, t) : e1;
      must(e1 && e2, seq + ' ' + g + ': socket ' + s.socket1 + ' / ' + s.socket2 + ' could not be placed');
      values.push(...e1.map(r1), ...e2.map(r1));
    }
    add({ id: 'track.' + (owner === 'gate' ? 'gate' : 'sora') + '.' + seq + '.' + g, what: 'track', code: frames, logical: g, clip: seq, length: r4(length), flags: [owner, s.shape, s.socket1, s.socket2], values, note: 'per frame [ax, ay, az, bx, by, bz] (cm, ' + (owner === 'gate' ? 'the gate\'s actor space' : 'Sora\'s actor space, his yaw 0') + ') of group ' + g });
    tracks += 1;
  }
}
const played = new Set(ROWS.filter((r) => r.what === 'anim' && r.clip !== '-').map((r) => r.flags[0]));
for (const seq of [...played].sort()) { const it = item(seq); if (it && (it.notifies || []).some((n) => n.class === 'TresAnimNotifyState_Attack')) track('sora', seq, WEP_SHAPES, weaponEnd); }
// the transformation (the framework plays the row's transform_clip StyleChange8 = 8401: its ChangeGM / BD_SphereSS windows)
{ const b = MAIN.logical_animations.find((x) => x.name === GM.transform_clip); must(b && b.file, GM.transform_clip + ': not in the main pack'); track('sora', path.basename(b.file, '.glb'), WEP_SHAPES, weaponEnd); }
track('gate', '8417', GATE_SHAPES, gateEnd);
// THE MUZZLES: where a shot of the clip starts (the shot handler 0x141344580 -> pawn vt+0x1410(0) 0x1405AA790 -> equipment
// vt+0x580: weapon 0's MuzzleAttachPoint, the class default ATK_01 (ctor 0x141777AE0)): w_so172's ATK_01 per frame, actor space.
for (const r of ROWS.filter((x) => x.what === 'attack' && x.flags.includes('shot'))) {
  const seq = clipOf(r.logical).seq, it = item(seq);
  const values = [];
  for (let fr = 0; fr < it.num_frames; fr++) { const t = it.num_frames > 1 ? fr * it.duration_s / (it.num_frames - 1) : 0; const e = weaponEnd('ATK_01', [0, 0, 0], seq, t); must(e, seq + ': ATK_01'); values.push(...e.map(r1)); }
  add({ id: 'muzzle.' + seq, what: 'muzzle', code: it.num_frames, logical: r.logical, clip: seq, length: r4(it.duration_s), values, note: 'per frame [x, y, z] (cm, actor space, his yaw 0): w_so172 ATK_01, the muzzle of weapon 0' });
}
must(tracks >= 18, 'expected the Nano Arms clips\' tracks, got ' + tracks);
// the gate's own clips (its hit windows; the clip it plays with Sora's)
for (const seq of ['8416', '8417', '8418']) {
  const a = modelAnim(GATE, seq);
  must(a, 'w_so174 ' + seq + ': no clip');
  const wins = a.notifies.filter((n) => n.class === 'TresAnimNotifyState_Attack');
  add({ id: 'gate.' + seq, what: 'gate', code: a.numFrames, logical: 'M_' + seq, clip: seq, length: r4(a.length), windows: wins.map((n) => r4(+n.time) + '-' + r4(+n.end_time)), rows: wins.map((n) => String(prop(n, 'm_AttackDataIDName')) + '@' + String(prop(n, 'CollisionGrpName'))), values: wins.map((n) => r4(+(prop(n, 'AttackInterval') || 0))), ref_addr: '0x1413D5660', note: 'the gate w_so174 (TresStylePawnGMGate) plays its M_' + seq + ' with Sora\'s (StyleFinishGM Tick); values = each window\'s AttackInterval' });
}

// ------------------------------------------------------------------------------------------------ (AUDIT-B F-NANO) the weapon's and the gate's own clips
// KH3: the form weapon w_so172 plays its OWN clip of Sora's sequence (its TresAnimNotifyState_ChangeAttachParent puts it on
// Sora's kh_root: its clips are authored in his mesh space) and the gate w_so174 plays M_8416 / M_8417 / M_8418 with his;
// the notifies of those clips fire on THOSE actors (SQEX_ATTACH_OBJECT_OWNER = the weapon / the gate): effects on their
// sockets, sounds. One row per effect / sound notify: `wfx.<owner>.<seq>.<k>` (what 'wfx'): flags = [owner (weapon |
// gate), effect | sound, state | instant, attach (ToAttach) | place, abs | rel (bAbsoluteRotation), the end type
// (AttachEffectEndType: LOOPEND | NONE | ...), WhenChangeAnimationType (END | NONE), the socket]; logical = the effect / sound
// name (the sound: its /Game path as the clips' sound notifies carry it); windows = [begin-end]; values = [GroupID,
// ScaleOffset xyz, FadeIn, FadeOut, StartDelayTime, then per frame x y z pitch yaw roll]: the effect's transform (the socket
// x the notify's LocationOffset / RotationOffset; cm, degrees: UE's FRotator) in Sora's actor space (the weapon; his yaw 0)
// or the gate's actor space (the gate). A sound row: values = [FadeOutDuration, then per frame x y z 0 0 0] of its
// AttachPointName. The fist w_so173 (TresProjGM_RocketPunch): m_AttachData ead_so170_com0 group m_AttachEffectGroupID (1620)
// = rows `wfx.fist.<k>` (owner fist, the actor's root: no track).
const rotatorOf = (q) => { // FQuat::Rotator (UE 4.17)
  const [X, Y, Z, W] = q, st = Z * X - W * Y, yawY = 2 * (W * Z + X * Y), yawX = 1 - 2 * (Y * Y + Z * Z), D = 180 / Math.PI;
  if (st < -0.4999995) { const yw = Math.atan2(yawY, yawX) * D; return [-90, yw, -yw - 2 * Math.atan2(X, W) * D]; }
  if (st > 0.4999995) { const yw = Math.atan2(yawY, yawX) * D; return [90, yw, yw - 2 * Math.atan2(X, W) * D]; }
  return [Math.asin(2 * st) * D, Math.atan2(yawY, yawX) * D, Math.atan2(-2 * (W * X + Y * Z), 1 - 2 * (X * X + Y * Y)) * D];
};
const shortName = (p) => String(p).split('.').pop();
const fxRows = { weapon: 0, gate: 0, fist: 0 };
const wnotify = require('./lib/wnotify_rows.js'); // (AUDIT-B W-VFX2) thunderAnchor / thunderEffect
function clipFx(owner, w, seq) {
  const a = modelAnim(w, seq); if (!a) return;
  const frames = a.numFrames, length = a.length;
  const place = (socket, loc, rot, absRot, t) => {
    // a socket the model lacks: UE's attach to an unknown socket is the component itself (its origin)
    // (AUDIT-B W-HANDOVER) the weapon's effects ride its re-parent: its frame by wepBase (the gate's actor is its own frame)
    const base = owner === 'weapon' ? wepBase(seq, t) : IDENTITY;
    if (owner === 'weapon' && !inWindow(seq, t)) TAIL_FRAMES += 1;
    const T = (socket === 'None' ? null : modelSocketT(w, socket, seq, t, base)) || base;
    const off = qrot(T.q, [loc[0] * T.s[0], loc[1] * T.s[1], loc[2] * T.s[2]]);
    const p = actor([T.t[0] + off[0], T.t[1] + off[1], T.t[2] + off[2]]);
    const q = absRot ? rotQ(rot) : qmulH(MESH_YAW_Q, qmulH(T.q, rotQ(rot)));
    return [...p.map(r1), ...rotatorOf(q).map(r1)];
  };
  const track = (socket, loc, rot, absRot) => { const out = []; for (let f = 0; f < frames; f++) out.push(...place(socket, loc, rot, absRot, frames > 1 ? f * length / (frames - 1) : 0)); return out; };
  let k = 0;
  for (const n of a.notifies) {
    const p = n.properties || {};
    const end = Number.isFinite(+n.end_time) ? +n.end_time : +n.time;
    const state = n.kind === 'state';
    if (n.class === 'SQEXSEADAnimNotifyState_AttachSound' || n.class === 'SQEXSEADAnimNotify_PlaySound') {
      const sock = String(p.AttachPointName || 'None');
      add({ id: 'wfx.' + owner + '.' + seq + '.' + k++, what: 'wfx', code: frames, logical: String(p.Sound), clip: seq, length: r4(length), flags: [owner, 'sound', state ? 'state' : 'instant', 'attach', 'rel', '-', '-', sock], windows: [r4(+n.time) + '-' + r4(end)], values: [+(p.FadeOutDuration || 0), ...track(sock, [0, 0, 0], { pitch: 0, yaw: 0, roll: 0 }, false)], ref_addr: '-', note: owner + ' ' + w.mj.model + ' clip ' + seq + ': ' + n.class + ' (values = [FadeOutDuration, per frame x y z 0 0 0 of its AttachPointName])' });
      fxRows[owner] += 1;
      continue;
    }
    if (!/AttachEffect$/.test(n.class)) continue;
    for (const m of (p.MetaAttachDataList || {}).items || []) {
      const mo = flat(m);
      for (const d of mo.MetaAttachData || []) {
        for (const e of d.EffectDataList || []) {
          if (!e.AttachEffect || e.AttachEffect === 0) continue;
          const sock = String(d.SocketName && d.SocketName !== 'None' ? d.SocketName : mo.SocketName || 'None');
          const loc = v3(d.LocationOffset), rot = d.RotationOffset || { pitch: 0, yaw: 0, roll: 0 }, sc = v3(d.ScaleOffset || { x: 1, y: 1, z: 1 });
          const endType = String(d.AttachEffectEndType || '-').replace('ESQEX_ATTACH_EFFECT_END_TYPE_', ''), change = String(d.WhenChangeAnimationType || '-').replace('ESQEX_CHANGE_ANIMATION_TYPE_', '');
          // (AUDIT-B W-VFX2) a TypeDataSQEX_Thunder (beam) row of the weapon: its host-resolved anchor `kb:w_so172:<unit socket>:<socket>
          // [:to<EndSocketName>]` as flags[8] (tools/lib/wnotify_rows.js `thunderAnchor`): the host finds the beam's bone list / end
          // socket on w_so172's own mesh (with its ChangeAttachParent windows, kh-core placement::part_parent); nano.rs spawns it there
          const beam = owner === 'weapon' ? wnotify.thunderAnchor(P('build', 'sora_pack'), String(e.AttachEffect), w.mj.model, UNIT_SOCKET, sock, d, endType) : '';
          must(owner === 'weapon' || !wnotify.thunderEffect(P('build', 'sora_pack'), shortName(e.AttachEffect)), '(AUDIT-B W-VFX2) a thunder effect on the ' + owner + ': ' + e.AttachEffect);
          add({ id: 'wfx.' + owner + '.' + seq + '.' + k++, what: 'wfx', code: frames, logical: shortName(e.AttachEffect), clip: seq, length: r4(length), flags: [owner, 'effect', state ? 'state' : 'instant', d.ToAttach === false ? 'place' : 'attach', d.bAbsoluteRotation ? 'abs' : 'rel', endType, change, sock, ...(beam ? [beam] : [])], windows: [r4(+n.time) + '-' + r4(end)], values: [e.GroupID || 0, ...sc, d.FadeIn || 0, d.FadeOut || 0, d.StartDelayTime || 0, ...track(sock, loc, rot, !!d.bAbsoluteRotation)], ref_addr: '-',
            note: owner + ' ' + w.mj.model + ' clip ' + seq + ': ' + n.class + ' "' + ((p.Description && p.Description.s) || '') + '" (values = [GroupID, ScaleOffset xyz, FadeIn, FadeOut, StartDelayTime, per frame x y z pitch yaw roll in ' + (owner === 'gate' ? 'the gate\'s' : 'Sora\'s') + ' actor space])' });
          fxRows[owner] += 1;
        }
      }
    }
  }
}
for (const c of (WEP.mj.clips || []).map((x) => x.sequence).sort()) clipFx('weapon', WEP, c);
for (const c of ['8416', '8417', '8418']) clipFx('gate', GATE, c);
{
  const bp = readJson(path.join(BP, 'Projectile/GM/TresProjGM_RocketPunch.uasset.json'));
  const cdo = flat(bp.exports.find((e) => e.objectName === 'Default__TresProjGM_RocketPunch_C').data.props);
  const ead = readJson(P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content', 'Effects', 'wep', 'w_so170', 'ead', 'ead_so170_com0.uasset.json'));
  const nm = (i) => (i < 0 ? ead.imports[-i - 1].objectName : '-');
  let k = 0;
  for (const it of flat(ead.exports[0].data.props).EffectDataList || []) {
    for (const e of it.EffectDataList || []) {
      if (e.GroupID !== cdo.m_AttachEffectGroupID) continue;
      add({ id: 'wfx.fist.' + k++, what: 'wfx', code: 0, logical: nm(e.AttachEffect), clip: 'TresProjGM_RocketPunch', flags: ['fist', 'effect', 'state', it.ToAttach === false ? 'place' : 'attach', it.bAbsoluteRotation ? 'abs' : 'rel', String(it.AttachEffectEndType).replace('ESQEX_ATTACH_EFFECT_END_TYPE_', ''), String(it.WhenChangeAnimationType).replace('ESQEX_CHANGE_ANIMATION_TYPE_', ''), String(it.SocketName)], values: [e.GroupID, ...v3(it.ScaleOffset || { x: 1, y: 1, z: 1 }), it.FadeIn || 0, it.FadeOut || 0, it.StartDelayTime || 0, ...v3(it.LocationOffset), 0, 0, 0], ref_addr: '-',
        note: 'TresProjGM_RocketPunch m_AttachData ead_so170_com0, m_AttachEffectGroupID ' + cdo.m_AttachEffectGroupID + ' (TresEffectAttach0 on the fist\'s root; values = [GroupID, ScaleOffset xyz, FadeIn, FadeOut, StartDelayTime, LocationOffset xyz, 0 0 0])' });
      fxRows.fist += 1;
    }
  }
}
// (AUDIT-B W-HANDOVER) the census of the weapon's windows and the tails the effect tracks were placed in
must(WEP_WINDOWS.size === 15, '(AUDIT-B W-HANDOVER) w_so172 ChangeAttachParent windows: ' + WEP_WINDOWS.size);
console.log('(AUDIT-B W-HANDOVER) w_so172 windows ' + WEP_WINDOWS.size + ', tails (window end < clip end): ' + (WEP.mj.clips || []).filter((c) => WEP_WINDOWS.has(c.sequence) && WEP_WINDOWS.get(c.sequence)[1] < c.duration_s - 1e-4).map((c) => c.sequence + ' ' + r4(WEP_WINDOWS.get(c.sequence)[1]) + '/' + r4(c.duration_s)).join(', ') + '; effect-track frames on the socket: ' + TAIL_FRAMES);
must(fxRows.weapon >= 40 && fxRows.gate >= 30 && fxRows.fist === 2, '(AUDIT-B F-NANO) the clips\' effect rows: ' + JSON.stringify(fxRows));
// the gate's hit effect (w_so174 TresAtkColl0 m_HitEffects[0].m_PawnHitEffect, every group): on the gate rows' flags
{
  const d = readJson(path.join(BP, 'StylePawn/w_so174.uasset.json'));
  const ac = flat(d.exports.find((e) => e.objectName === 'TresAtkColl0').data.props);
  const he = ((ac.m_HitEffects || [])[0] || {}).m_PawnHitEffect;
  const hit = typeof he === 'number' && he < 0 ? d.imports[-he - 1].objectName : '-';
  must(hit !== '-', 'w_so174: no hit effect');
  for (const r of ROWS.filter((x) => x.what === 'gate')) r.flags = ['hit:' + hit];
}

// ------------------------------------------------------------------------------------------------ the projectiles
// [life, speed, max speed, homing delay, homing max angle, homing turn max, root radius, root offset x, shape radius, shape offset x, no destroy on hit, map normal check]
function proj(rel) {
  const d = readJson(path.join(BP, rel + '.uasset.json'));
  const cdo = d.exports.find((e) => /^Default__/.test(e.objectName || '') && e.class.endsWith('_C'));
  const pc = d.exports.find((e) => e.class === 'TresProjectileMovementComponent');
  const root = d.exports.find((e) => e.class === 'TresRootComponent');
  const p = flat(cdo.data ? cdo.data.props : cdo.props), m = pc ? flat(pc.data ? pc.data.props : pc.props) : {}, r = root ? flat(root.data ? root.data.props : root.props) : {};
  const s = shapes(rel)[0];
  const rs = (r.CollisionShapesSrc || [])[0];
  const ac = d.exports.find((e) => e.class === 'TresAtkCollComponent'), ap = flat(ac.data ? ac.data.props : ac.props);
  const he = ((ap.m_HitEffects || [])[0] || {}).m_PawnHitEffect;
  const fx = typeof he === 'number' && he < 0 ? d.imports[-he - 1].objectName : '-';
  return { p, m, s, fx, root: rs ? { radius: rs.Size.x, x: (rs.RelativeLocation || {}).x || 0 } : { radius: 0, x: 0 } };
}
const SPEED = 2000; // TresProjectileMovementComponent ctor 0x141470A90: +0x248 = +0x24C = 2000.0 (0x44FA0000)
for (const [name, rel, next] of [['BX_ROCKET', 'Projectile/GM/TresProjGM_RocketPunch0', 'Projectile/GM/TresProjGM_RocketPunch'], ['BARRIERCRACKER', 'Projectile/GM/TresProj_BarrierCrackerBX', null]]) {
  must(GM.projectiles.some((x) => x.startsWith(name + '=')), 'w_so170_ProjSet has no ' + name);
  for (const [stage, r] of [[0, rel], ...(next ? [[1, next]] : [])]) {
    const x = proj(r);
    const speed = x.m.InitialSpeed !== undefined ? x.m.InitialSpeed : SPEED, max = x.m.MaxSpeed !== undefined ? x.m.MaxSpeed : SPEED;
    add({ id: 'proj.' + name + '.' + stage, what: 'proj', code: stage, logical: name, clip: r.split('/').pop(), rows: [x.s.row], flags: [x.p.m_bReSpawnProjectileOnTimeOut ? 'respawn' : 'last', x.m.m_bEnableHoming ? 'homing' : 'straight', x.p.m_bReSpawnTakeOverAtkTarget ? 'take_target' : '-', x.p.m_bReSpawnTakeOverAtkCollHitList ? 'take_hit_list' : '-', x.p.m_bReSpawnTakeOverAtkCollFinishFlag ? 'take_finish' : '-', x.fx],
      values: [x.p.m_MainLife, speed, max, x.m.m_HomingStartDelay || 0, x.m.m_HomingMaxAngle || 0, x.m.m_HomingTurnMax || 0, x.root.radius, x.root.x, x.s.size[0], x.s.loc1[0], x.p.m_bNoDestroyOnHit ? 1 : 0, x.m.m_bEnableMapHitNormalCheck ? x.m.m_CheckMapHitNormal : 0],
      ref_addr: '0x141344580', note: 'Blueprints/' + r + ': values = [m_MainLife, InitialSpeed, MaxSpeed (the blueprint, else the ctor 0x141470A90: 2000), m_HomingStartDelay, m_HomingMaxAngle, m_HomingTurnMax, the root sphere radius, its x offset, the attack sphere radius, its x offset, m_bNoDestroyOnHit, m_CheckMapHitNormal (0 = no check)]; ' + (stage === 0 && next ? 'respawns at its time-out as stage 1 (m_ReSpawnProjectileClass), taking over the target, the hit list and the finish flag' : 'the last stage') });
  }
}
// the attack rows the form hits with (p_ex001_AttackData): the combat sheet carries them (tools/sora_combat_sheet.js `scgm_`)
const usedRows = new Set(ROWS.flatMap((r) => (['attack', 'gate'].includes(r.what) ? r.rows.map((x) => x.split('@')[0]) : r.what === 'proj' ? r.rows : [])).filter((x) => x !== 'None'));
for (const id of ['SCGM_CHANGE', 'SCGM_CHANGE_END', 'SCGM_DODGE', 'SCGM_FINISH_ROCKET', 'SCGM_FINISH_END']) usedRows.add(id);
for (const id of [...usedRows].sort()) {
  const a = atkRow(id);
  must(a, 'p_ex001_AttackData has no row ' + id);
  add({ id: 'row.' + id, what: 'row', logical: id, values: [a.m_Power, a.m_FormPoint, a.m_ReactionPowerValue], flags: [String(a.m_DamageKind).split('::').pop(), String(a.m_DamageAttribute).split('::').pop()], note: 'p_ex001_AttackData row ' + a.id + ': values = [m_Power, m_FormPoint, m_ReactionPowerValue]' });
}

// ------------------------------------------------------------------------------------------------ checks
must(ROWS.filter((r) => r.what === 'attack').length === 11, 'expected the 9 GM kinds and the two Nano Counter clips');
for (const r of ROWS.filter((x) => x.what === 'attack')) must(r.clip !== '-' && r.length > 0, r.id + ': no clip');
must(ROWS.some((r) => r.id === 'anim.1301' && r.clip === '8301'), 'AnimSet8 replaces 1301 by 8301');
must(ROWS.some((r) => r.id === 'anim.BarrierCracker' && r.clip === 'MagicCast'), 'BarrierCracker plays the main pack\'s 1341 (MagicCast)');

// ------------------------------------------------------------------------------------------------ sheet
const S = (n, t, g, doc) => [n, t, g, doc];
const COLS = [
  S('id', 'str', 'str', '<what>.<key>'), S('what', 'enum:attack|anim|finish|code|shape|track|vol|muzzle|gate|proj|row|wfx', 'str', 'the row kind'),
  S('code', 'int', 'i32', 'attack kind (attack), finish sub-step (finish), frames (track, gate), projectile stage (proj), else 0'),
  S('logical', 'str', 'str', 'the logical anim name KH3 plays (attack / anim / finish), the group (shape, track), the projectile (proj), the row (row)'),
  S('clip', 'str', 'str', 'the animation-table row (attack / anim / finish), the owner (shape), the sequence (track, gate), the blueprint (proj)'), S('length', 'num', 'f32', 'the clip length, s'),
  S('root_len', 'num', 'f32', 'root length (+0x1D8), 0 = none'), S('stop_dist', 'num', 'f32', 'stop distance (+0x24C)'), S('z_gain', 'num', 'f32', 'z gain (+0x14C)'),
  S('air_mode', 'bool', 'bool', '+0x1DF'), S('chan_xy', 'bool', 'bool', '+0x234 bit 0'), S('chan_root', 'bool', 'bool', '+0x234 bit 1'),
  S('flags', 'list', 'str[]', 'the case\'s other writes / strings by kind (see the note)'), S('windows', 'list', 'str[]', 'the clip\'s hit windows a-b, s'), S('rows', 'list', 'str[]', 'the attack rows of those windows (row@group); proj / shape: the default row'),
  S('values', 'list', 'f32[]', 'numbers by kind (see the note)'),
  S('ref_addr', 'str', 'str', 'decompiled function'), S('ref_conf', 'enum:confirmed|inferred|guess', 'str', 'confirmed = read in the code or the data'), S('note', 'str', 'str', '-'),
];
const NAME = 'sora-form-nano';
const rows = ROWS.map((r) => COLS.map((c) => r[c[0]]));
for (const [i, r] of rows.entries()) for (const [k, v] of r.entries()) must(v !== undefined && v !== null && !(typeof v === 'number' && !Number.isFinite(v)), 'row ' + ROWS[i].id + ': cell ' + COLS[k][0] + ' is ' + v);
must(new Set(ROWS.map((r) => r.id)).size === ROWS.length, 'duplicate ids');
fs.writeFileSync(P('sheets', 'port', NAME + '.json'), '{"sheet":"' + NAME + '","group":"port","columns":' + JSON.stringify(COLS.map((c) => c[0])) + ',"rows":[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
const by = {}; for (const r of ROWS) by[r.what] = (by[r.what] || 0) + 1;
console.log('sheets/port/' + NAME + '.json: ' + rows.length + ' rows ' + JSON.stringify(by));

// ------------------------------------------------------------------------------------------------ register (this tool's entry only; replaced in place)
const q = (s) => "'" + String(s).replace(/\\/g, '\\\\').replace(/'/g, "\\'") + "'";
const NL = '\n';
const DOC = 'KH3\'s Nano Arms as data for its form package (kh-core combat/forms/nano.rs, states/combo_gm.rs): COMBO_GM\'s attack kinds, AnimSet8, the weapon\'s and the gate\'s hit shapes and their tracks, FINISH_GM, the projectiles, the attack rows.';
const MARK = '(weapons F-NANO)';
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
  const text = '  // ' + MARK + ' ' + DOC + ' Written by tools/sora_form_nano.js (documentation/KH3_WEAPONS_PLAN.md 1.3)' + NL +
    '  ' + q(NAME) + ": { group: 'port', file: " + q(NAME + '.json') + ", by: 'tools/sora_form_nano.js', from: " + q('sora-keyblade-forms, AnimSet8 of the asset pack, the weapon / gate blueprints and clips, the projectile blueprints, p_ex001_AttackData and the decompiled PlayAttack / ComboGM / StyleFinishGM') + ', external: true, cols: [' + NL +
    '    ' + COLS.map((c, i) => '[' + q(c[0]) + ', ' + (i === 0 ? 'K' : 'V') + ', ' + q(c[1]) + ", '-', " + q(c[3]) + (['note', 'flags', 'windows', 'rows', 'values', 'logical', 'clip'].includes(c[0]) ? ', true' : '') + ']').join(', ') + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/sheet_defs.js');
}
{
  const f = P('tools', 'lib', 'port_gen_defs.js'); const before = fs.readFileSync(f, 'utf8');
  const END = NL + '    },' + NL;
  const pos = (t) => { const anchor = '  tables: {' + NL; const at = t.indexOf(anchor); must(at >= 0, 'port_gen_defs.js: anchor tables'); return at + anchor.length; };
  const cols = COLS.filter((c) => c[2]).map((c) => c[0] + ': { type: ' + q(c[2]) + (c[3] && c[3] !== '-' ? ', doc: ' + q(c[3]) : '') + ' }').join(', ');
  const text = '    // ' + MARK + ' ' + NAME + ': written by tools/sora_form_nano.js (documentation/KH3_WEAPONS_PLAN.md 1.3)' + NL +
    '    ' + q(NAME) + ': {' + NL +
    "      file: 'sora_form_nano.rs', row: 'FormNanoRow', table: 'FORM_NANO', key: 'id', key_prefix: 'FNA'," + NL +
    '      doc: ' + q(DOC) + ',' + NL +
    '      columns: { ' + cols + ' },' + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/port_gen_defs.js');
}
console.log('sora_form_nano ' + VERSION);
