// (weapons F-HAMMER) KH3's hammer / drill formchanges as DATA for their form package (kh-core combat/forms/hammer.rs,
// states/combo_hm.rs): Hyper Hammer (form.HM, CHANGE_HM 0x81) -> Drill Punch (form.DR, CHANGE_DR 0x84) of Favorite Deputy and
// Boom Hammer (form.HM2, CHANGE_HM2 0x92) -> Clock Drill (form.DR2, CHANGE_DR2 0x93) of Classic Tone. Both pairs share their
// attack classes (pawn slot 1337 0x141432F00: 0x81 / 0x92 -> COMBO_HM 76, 0x84 / 0x93 -> COMBO_DR 79) and their clips
// (AnimSet3 / AnimSet6; Classic Tone adds AnimSet20 / AnimSet21 and plays the clips with the effect group 0x76C, its toon
// effects). documentation/KH3_WEAPONS_PLAN.md 1.3 / 4 (package F-HAMMER), build/sora_tmp/weapons/F-HAMMER/STATE.md.
//
//   node tools/sora_form_hammer.js   write sheets/port/sora-form-hammer.json and register it (once, in place) in
//                                    tools/lib/sheet_defs.js and tools/lib/port_gen_defs.js; then node tools/gen_port.js
//
// Inputs (read only): sheets/port/sora-keyblade-forms.json (P3), build/sora_pack/keyblades/sora_forms_manifest.json (the form
// anim sets), build/sora_pack/manifest.json + xsi_manifest.json (the base / XS rows), build/sora_pack/keyblades/<model>/
// {model,sockets}.json (the form weapons and the Clock), assets_converted (skeletons, clips, the weapon / style-pawn
// blueprints' TresAtkColl0), sheets/entities/DataTable/Player/Attack/p_ex001_AttackData.json, and THE CODE TABLES BELOW, read
// in the decompiled exe (re/decomp):
//   PLAY     TresPlayerState_Attack::PlayAttack 0x1413378B0 cases 0x58 .. 0x6A (the defaults before the switch, the tail
//            LAB_14133A87A: the root scale clamp(dist / fVar28, 0.1, fVar32), SetAnimTime(fVar33), SetVoiceGroupNo(+0x19C),
//            pawn vt+0x2068(1) invincible; +0x198 = 0x76C in forms 0x92 / 0x93; the head: kinds 0x60 / 0x68 pawn vt+0x1388).
//   DECIDE   ComboHM::Decide 0x141362030, ComboDR::Decide 0x141360F70 (data in the Rust: combat/forms/hammer.rs).
//   RENAME   ComboHM TresAtk begin 0x141357320 (form 0x92: SCHM_* -> HM2_*, SCHM_UPPER of kind 0x5F -> HM2_RISE; form 0x81:
//            SCHM_UPPER of kind 0x5F -> SCHM_RISE), ComboDR TresAtk begin 0x141357100 (form 0x93: SCDR_* -> DR2_*), DodgeRoll
//            0x14137B880 (form 0x93: SCDR_DODGE -> DR2_DODGE).
//   FINISH   FINISH_DR 0x9E: TryStyleFinish 0x141447840 -> STYLEFINISH (33) in 0x84, STYLEFINISH_DR (34) in 0x93. Drill Punch:
//            StyleFinish Begin 0x1413CBF70 case 0x9E (form 0x84) -> delegate 0x1404EFA80 -> slot 148 0x1413DF1F0. Clock Drill:
//            StyleFinishDR Reset 0x1413D0DC0, Begin 0x1413CC300, Tick 0x1413D4990, End 0x1413CF0C0 (0x1413CFA10 the Clock away).
//   START    StartStyle 0x14141A9D0: 0x81 / 0x84: pawn+0xE60 |= 0x40000; 0x93: pawn vt+0xFB0(0x9E).
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
const CODES = ['HM', 'DR', 'HM2', 'DR2'];
const FORMS = Object.fromEntries(sheet('sora-keyblade-forms').filter((r) => CODES.map((c) => 'form.' + c).includes(r.id)).map((r) => [r.id.slice(5), r]));
must(Object.keys(FORMS).length === 4, 'sora-keyblade-forms lacks one of form.HM / DR / HM2 / DR2');
const OV = readJson(P('build', 'sora_pack', 'keyblades', 'sora_forms_manifest.json'));
const MAIN = readJson(P('build', 'sora_pack', 'manifest.json'));
const XSI = (() => { for (const f of [P('build', 'sora_pack', 'xsi_manifest.json'), P('build', 'sora_tmp', 'xsi', 'pack', 'xsi_manifest.json')]) if (fs.existsSync(f)) return readJson(f); return { items: [], logical_animations: [] }; })();
const ATK = readJson(P('sheets', 'entities', 'DataTable', 'Player', 'Attack', 'p_ex001_AttackData.json')).rows;
const setName = (a) => String(a).split('/').pop().split('.')[0];
const ITEMS = new Map([...MAIN.items, ...XSI.items, ...OV.items].filter((i) => i.kind === 'animation').map((i) => [i.animation_name, i]));
const BASE = new Map(MAIN.logical_animations.map((a) => [a.name, a]));
const XSI_ROWS = new Map(XSI.logical_animations.map((a) => [a.name, a]));
// logical name -> clip of a form: its sets' rows, the later set over the earlier (m_ReplaceAnimSets order), else the base.
const ROWS_OF = {};
for (const [code, f] of Object.entries(FORMS)) {
  const out = new Map();
  for (const set of f.anim_sets.map(setName)) for (const a of OV.logical_animations.filter((x) => setName(x.anim_set) === set)) out.set(a.name, { set, clip: a.clip, file: a.file, cls: a.class, xsi: !!a.in_xsi_overlay });
  ROWS_OF[code] = out;
}
// the sequence a logical name plays in a form
function clipOf(form, logical) {
  const r = ROWS_OF[form].get(logical);
  if (r && r.file) return r.clip;
  if (XSI_ROWS.has(logical) && XSI_ROWS.get(logical).file) return path.basename(XSI_ROWS.get(logical).file, '.glb');
  const b = BASE.get(logical);
  return b && b.file ? path.basename(b.file, '.glb') : '-';
}
// the animation-table row a clip plays as: form-set clips are rows named by their sequence (tools/gen_port.js
// FORM_ANIM_SETS), the XS overlay's (M_3416) and the base set's rows by their logical name.
function rowOf(form, logical) {
  const r = ROWS_OF[form].get(logical);
  if (r && r.file) return r.clip;
  if (XSI_ROWS.has(logical) || BASE.has(logical)) return logical;
  return '-';
}
const item = (clip) => ITEMS.get(clip) || null;
const prop = (n, key) => { const p = n.properties || {}; const v = p[key]; return v && typeof v === 'object' && 'value' in v ? v.value : v; };
const atkRow = (id) => ATK.find((r) => r._row_name === id || String(r.id).toLowerCase() === String(id).toLowerCase());
const tOf = (n) => (n.trigger_time !== undefined ? n.trigger_time : n.time);
const eOf = (n) => (n.end_trigger_time !== undefined ? n.end_trigger_time : n.end_time !== undefined ? n.end_time : tOf(n) + (n.duration || 0));

// ------------------------------------------------------------------------------------------------ the code tables
// PLAY: PlayAttack 0x1413378B0 (decompiled C). Before the switch: m_bFaceAt; +0x234 = (+0x234 & ~6) | 1 (XY channel on, root
// channel off); stop 160 (0x43200000); z gain 3 (0x40400000); +0x1DF (air mode) 0; +0x198 0; fVar28 = 0, fVar32 = 2.5, fVar33 = 0.
// Per case (the writes of the case): root = +0x1D8, stop = +0x24C, `| 2` = chan_root, `& ~1` = no chan_xy, `| 8` = no_vz
// (Z channel writes 0 in the window), +0x23C |= 0x10 = keep_dir_z, +0x1DD fixed DirZ, +0x1DE dive, +0x1D4 DirZ, +0x1DF air,
// +0x19C the voice group, bit 16 (0x10000) / bit 11 (0x800) no reader in the port (STATE.md). `near` = the case's
// `if (dist < D) { fVar28 = div; fVar32 = max; chan_xy off }` (the root scale clamp(dist / div, 0.1, max) of the play).
const PLAY = [
  // COMBO_HM (76): Hyper Hammer / Boom Hammer
  { kind: 0x58, logical: 'M_3411', root: 250, at: '0x141338951', label: 'ground step (after 0x59 near, 0x5B with reach 2, the first step with reach 0 / 2 near)' },
  { kind: 0x59, logical: 'M_3412', root: 800, stop: 400, near: [950, 800, 1.0, true], air_if_air: true, at: '0x14133895F', label: 'ground step, the dash (reach 1 at the first step; after 0x5A); in the air: air mode (+0x234 bit 16)' },
  { kind: 0x5A, logical: 'M_3413', root: 150, at: '0x1413389A3', label: 'ground step, the double slam (after 0x58 with reach 2, after 0x5B)' },
  { kind: 0x5B, logical: 'M_3414', root: 600, chan_root: true, near: [750, 600, 2.5, false], air_is_air: true, at: '0x1413389B1', label: 'ground / air step, the swing (after 0x58, 0x59 reach 2; air: combo < 3 after a step); air mode = bAir' },
  { kind: 0x5C, logical: 'M_3415', root: 800, chan_root: true, air: true, near: [950, 800, 2.5, false], at: '0x1413389F3', label: 'air step, the rolling (the first air step, after 0x5B)' },
  { kind: 0x5D, logical: 'M_3416', voice: 1, at: '0x141338A33', label: 'ground finisher (Giant Stamp: the hammer grows, M_3416); SetVoiceGroupNo(1)' },
  { kind: 0x5E, logical: 'M_3417', no_vz: true, air: true, at: '0x141338A4C', label: 'air finisher (the giant beat, M_3417)' },
  { kind: 0x5F, logical: 'M_3412', root: 800, stop: 300, air: true, chan_root: true, no_vz: true, keep_dir_z: true, fixed_dir_z: true, dir_z: 1000, near: [950, 800, 1.0, false], start: 0.2, at: '0x141338A67', label: 'the rise (target high, dist <= 600, not right after 0x5F): M_3412 from 0.2 s, DirZ fixed 1000; its SCHM_UPPER hits as SCHM_RISE / HM2_RISE' },
  { kind: 0x60, logical: 'M_3418', air: true, dive: true, no_vz: true, keep_dir_z: true, dir_z: -4000, weapon_display: true, at: '0x141338AE1', label: 'the falling stamp (air, target 100 below, not right after 0x60; a flow attack refined to 0x37): dive, DirZ -4000' },
  // COMBO_DR (79): Drill Punch / Clock Drill
  { kind: 0x61, logical: 'M_6411', root: 300, chan_root: true, at: '0x141338B12', label: 'step, the slide upper (after 0x63 on the ground; air: after 0x62)' },
  { kind: 0x62, logical: 'M_6412', root: 800, chan_root: true, at: '0x141338B2B', label: 'step, the stab dash (reach 1; the first step; air: after 0x61, the first air step, target below and airborne)' },
  { kind: 0x63, logical: 'M_6413', root: 250, at: '0x141338B4D', label: 'step, the break (after 0x62; any later step)' },
  { kind: 0x64, logical: 'M_6414', root: 250, at: '0x141338B54', label: 'step, the crack (after 0x61; reach 2: alternates with 0x65)' },
  { kind: 0x65, logical: 'M_6415', at: '0x141338B75', label: 'step, crack 2 (reach 2 after 0x64 / 0x65)' },
  { kind: 0x66, logical: 'M_6424', at: '0x141338B8E', label: 'ground finisher with 3+ enemies near (reach 2): the crack end (M_6424)' },
  { kind: 0x67, logical: 'M_6417', root: 1100, air: true, voice: 1, at: '0x141338BA7', label: 'air finisher, the giga stab (M_6417); SetVoiceGroupNo(1)' },
  { kind: 0x68, logical: 'M_6416', air: true, dive: true, no_vz: true, dir_z: -4000, weapon_display: true, at: '0x141338BCC', label: 'the falling crack (air, target 100 below and on the ground; a flow attack refined to 0x37): dive, DirZ -4000' },
  { kind: 0x69, logical: 'M_6418', air: true, at: '0x141338BF1', label: 'the rise (ground, target high, dist <= 400, not right after 0x69): DirZ fixed pawn+0xBAC (3.5) x 200 (Decide)' },
  { kind: 0x6A, logical: 'M_6419', root: 800, chan_root: true, keep_dir_z: true, dir_z: 0, invincible: true, at: '0x141338C0A', label: 'ground finisher (fewer than 3 enemies near): the giga upper (M_6419); invincible (pawn vt+0x2068(1)), +0x234 |= 0x800, FUN_1405B8B30(pawn+0x460)' },
  // Classic Tone's Barrier Cracker: the guard counter (TryAttack 0x141437330: forced kind 0x25 in forms 0x86 0x87 0x8E 0x8F 0x92 0x93
  // 0xDD .. 0xE0) with PlayAttack case 0x25 (not 0xDF / 0xE0): ground BarrierCracker / air AirBarrierCracker (+0x142), +0x198 =
  // 0xD48, +0x19C = 0x34 (the voice group), +0x164 = 1 (the swing counts as started), pawn+0x?? = 1, +0x234 |= 0x2800,
  // invincible, FUN_1405B8B30; its TresAnimNotify_Shot -> the shot handler 0x141344580 case 0x25: BARRIERCRACKER (the projectile
  // set's TresProj_BarrierCrackerSO) at the shot location (vt+0x1410: the weapon's ATK_01).
  { kind: 0x25, logical: 'BarrierCracker', forms: ['HM2', 'DR2'], group: 0xD48, voice: 0x34, invincible: true, swing: true, shot: true, at: '0x141338519', label: 'Barrier Cracker on the ground (the guard counter of the Classic Tone forms)' },
  { kind: 0x25, logical: 'AirBarrierCracker', variant: 'air', forms: ['HM2', 'DR2'], air: true, group: 0xD48, voice: 0x34, invincible: true, swing: true, shot: true, at: '0x141338519', label: 'Barrier Cracker in the air (+0x142: air mode)' },
];
const FAMILY = { HM: [0x58, 0x60], HM2: [0x58, 0x60], DR: [0x61, 0x6A], DR2: [0x61, 0x6A] };
const TOON_GROUP = 0x76C;
const isToon = (form) => form === 'HM2' || form === 'DR2';
// RENAME (the TresAtk begin handlers): form -> [old, new, kind (0 = any)]
const RENAME = {
  HM: [['SCHM_UPPER', 'SCHM_RISE', 0x5F]],
  HM2: [['SCHM_BREAK', 'HM2_BREAK', 0], ['SCHM_UPPER', 'HM2_RISE', 0x5F], ['SCHM_UPPER', 'HM2_UPPER', 0], ['SCHM_DOUBLE', 'HM2_DOUBLE', 0], ['SCHM_SWING', 'HM2_SWING', 0], ['SCHM_SWING_END', 'HM2_SWING_END', 0], ['SCHM_ROLLING', 'HM2_ROLLING', 0], ['SCHM_GIANTSTAMP', 'HM2_GIANTSTAMP', 0], ['SCHM_GIANTBEAT', 'HM2_GIANTBEAT', 0], ['SCHM_FALLBEAT', 'HM2_FALLBEAT', 0], ['SCHM_FALLSTAMP', 'HM2_FALLSTAMP', 0]],
  DR2: ['SLIDEUPPER', 'SLIDEUPPER_END', 'STAB', 'BREAK', 'CRACK', 'CRACK2', 'FALLCRACK', 'GIGASTAB', 'UPPER', 'GIGAUPPER'].map((s) => ['SCDR_' + s, 'DR2_' + s, 0]).concat([['SCDR_DODGE', 'DR2_DODGE', -1]]),
};
const RENAME_AT = { HM: '0x141357320', HM2: '0x141357320', DR2: '0x141357100' };
function renamed(form, row, kind) {
  for (const [a, b, k] of RENAME[form] || []) if (a === row && (k === 0 || k === kind)) return b;
  return row;
}
// FINISH: FINISH_DR by form, by sub-step.
const FINISH = {
  DR: [
    { sub: 0, logical: 'M_6421', what: 'Flying (pawn vt+0x17A0(5, 1)); Play("M_6421", blend -1, mode 2) + SetVoiceGroupNo(0); +0x188 = -150, +0x260 = 1.0, +0x1F4 |= 0x20 (the facing pitches toward the target), +0x194 (a timer) = 0; sub 2', flags: ['flying', 'pitch'] },
    { sub: 2, logical: '-', what: 'facing on (slot 127(1, 0.2)); a caught pawn (m_pCatchPawn, set by M_6421\'s SCDR_FINISH hit: DRILL_BIND) in state 62: taken onto the drill\'s X_link_skt (stopped, gravity off, no movement); without one: until M_6421 ends (the timer gives up after 10 s: DoChain); then facing off, the pitch facing off, the pitch levelled (vt+0xDB0), Play("M_6423") + voice 0, +0x164 = 0, sub 5', flags: ['catch', 'timer_10'] },
    { sub: 5, logical: 'M_6423', what: 'the caught pawn kept on X_link_skt (vt+0x16E8, FUN_143EAC880); at M_6423\'s end the finish is over (vt+0x270: DoChain)', flags: ['hold'] },
  ],
  DR2: [
    { sub: 0, logical: 'M_L421', what: 'the Clock (FINISH_DR\'s accompany pawn StylePawn/w_so114, ATresStylePawnDRClock: pawn vt+0x1460) spawned at Sora\'s location with his yaw, started (0x1413F5600), Sora put where it stands (0x143EAC3D0), shown (vt+0xD98(1)); the Clock and Sora play M_L421 (blend -1) + voice 0; +0x164 = 0; +0x2E8 (the clock) = 0; vt+0x1038(0, 0); sub 2', flags: ['spawn_clock'] },
    { sub: 2, logical: '-', what: 'every tick +0x2E8 += dt; while M_L421 is before 3.8 s Sora is kept at the Clock; while the Clock lives, +0x2E8 < its m_Param_MaxTime (vt+0x2620; w_so114: 10 s), no TresEnableChange and his clip has not ended: wait; then sub 6', flags: ['hold_at_clock_3.8'] },
    { sub: 6, logical: '-', what: '0x1413CFA10: the Clock cleaned up; sub 8', flags: ['cleanup'] },
    { sub: 8, logical: '-', what: 'at M_L421\'s end: DoChain (vt+0x270)', flags: ['end'] },
  ],
};
const FINISH_AT = { DR: '0x1413DF1F0', DR2: '0x1413D4990' };

// ------------------------------------------------------------------------------------------------ the weapons' shapes
const PKG = P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content', 'Blueprints');
function shapesOf(rel) {
  let f = path.join(PKG, rel + '.uasset.json');
  if (!fs.existsSync(f)) f = path.join(PKG, rel.replace(/_Wep$/, '_wep') + '.uasset.json');
  if (!fs.existsSync(f)) return [];
  const d = readJson(f);
  const flat = (props) => { const o = {}; for (const p of props || []) o[p.name] = p.type === 'StructProperty' ? (Array.isArray(p.value) ? flat(p.value) : p.value) : p.type === 'ArrayProperty' ? (p.value && p.value.items ? p.value.items.map((it) => (Array.isArray(it) ? flat(it) : it)) : p.value) : p.value; return o; };
  const out = [];
  const imp = (i) => (i < 0 && d.imports[-i - 1] ? d.imports[-i - 1].objectName : '-'); // (F-SHAPES)
  for (const e of d.exports) {
    if (e.class !== 'TresAtkCollComponent') continue;
    const p = flat((e.data && e.data.props) || e.props);
    for (const s of p.CollisionShapesSrc || []) {
      const v = (o) => (o ? [o.x || 0, o.y || 0, o.z || 0] : [0, 0, 0]);
      out.push({ owner: rel.split('/').pop().replace(/_[Ww]ep$/, ''), group: s.m_GrpName, shape: String(s.m_ShapeType).split('::').pop(), size: v(s.m_Size), inc: v(s.m_IncSize), incmax: v(s.m_IncMaxSize), scale: v(s.m_Scale), iscale: v(s.m_IncScale), mscale: v(s.m_IncMaxScale), socket1: s.m_SocketName1 || 'None', socket2: s.m_SocketName2 || 'None', loc: v(s.m_RelativeLocation1), row: s.m_DefaultAttackDataIDName || 'None', attach: String(s.m_AttachType1 || '').split('::').pop(), frozen: s.m_bDisableLocation1Attach === true,
        // (F-SHAPES) what the CONVEX transform reads (Update 0x140647950 case 6 / 8): m_Mesh, m_RelativeRocation, m_RotAttachType, m_bAbsoluteOffset1, m_bIgnoreParentScale
        mesh: imp(s.m_Mesh), rot: s.m_RelativeRocation ? { pitch: s.m_RelativeRocation.pitch || 0, yaw: s.m_RelativeRocation.yaw || 0, roll: s.m_RelativeRocation.roll || 0 } : { pitch: 0, yaw: 0, roll: 0 },
        rot_attach: String(s.m_RotAttachType || 'TRES_ACRA_NORMAL').split('::').pop(), abs1: s.m_bAbsoluteOffset1 === true, ignore_scale: s.m_bIgnoreParentScale === true });
    }
  }
  return out;
}
const partsOf = (form) => FORMS[form].units.map((u) => { const m = u.match(/^(\w+)@(\w+):(\w+)=(.*)$/); must(m, 'unit ' + u); return { unit: m[1], hand: m[2], socket: m[3] === 'None' ? 'R_wep_skt' : m[3], blueprint: m[4], model: m[4].split('/').pop().replace(/_[Ww]ep$/, '') }; });
const SHAPES = {};
for (const form of CODES) SHAPES[form] = partsOf(form).flatMap((p) => shapesOf('Player/p_ex001/' + p.model + '_Wep').map((s) => ({ ...s, part: p })));
const CLOCK_SHAPES = shapesOf('StylePawn/w_so114');

// ------------------------------------------------------------------------------------------------ FK (as tools/sora_pack.js attackSockets)
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
  const d = sp.loadPkg(sp.gamePathToContent(ID.skeleton));
  const out = {};
  for (const e of d.exports.filter((x) => x.class === 'SkeletalMeshSocket')) {
    const p = sp.propsOf(e);
    const loc = p.RelativeLocation ? p.RelativeLocation.value : { x: 0, y: 0, z: 0 }, rot = p.RelativeRotation ? p.RelativeRotation.value : { pitch: 0, yaw: 0, roll: 0 }, sc = p.RelativeScale ? p.RelativeScale.value : { x: 1, y: 1, z: 1 };
    out[p.SocketName.value] = { bone: p.BoneName.value, T: { q: rotQ(rot), t: [loc.x, loc.y, loc.z], s: [sc.x, sc.y, sc.z] } };
  }
  // (F-SHAPES) the skeletal mesh's own sockets too (`center`: the OWNER_ATTACHED_MESH CONVEX shapes of the drills), never over the skeleton's
  let m = null; try { m = sp.loadPkg(sp.gamePathToContent(ID.skeletal_mesh)); } catch (e) { m = null; }
  for (const e of (m ? m.exports : []).filter((x) => x.class === 'SkeletalMeshSocket')) {
    const p = sp.propsOf(e);
    const loc = p.RelativeLocation ? p.RelativeLocation.value : { x: 0, y: 0, z: 0 }, rot = p.RelativeRotation ? p.RelativeRotation.value : { pitch: 0, yaw: 0, roll: 0 }, sc = p.RelativeScale ? p.RelativeScale.value : { x: 1, y: 1, z: 1 };
    if (!out[p.SocketName.value]) out[p.SocketName.value] = { bone: p.BoneName.value, T: { q: rotQ(rot), t: [loc.x, loc.y, loc.z], s: [sc.x, sc.y, sc.z] } };
  }
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
// a weapon socket's actor-space transform at Sora's clip time (the weapon on Sora's socket, posed by its own clip)
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
  const chans = byBone(a);
  const wt = a ? Math.min(time, a.length) : 0;
  const ws = w.sockets[socketName];
  const boneName = ws ? ws.bone : socketName;
  const k = w.bones.findIndex((b) => b.name === boneName);
  if (k < 0) return null;
  let T = boneT(w.bones, chans, k, wt, false);
  T = under(T, base);
  if (ws) T = under({ q: rotQ({ pitch: ws.local_rotation[0], yaw: ws.local_rotation[1], roll: ws.local_rotation[2] }), t: ws.local_location, s: [1, 1, 1] }, T);
  return T;
}
// the shape's conservative sphere [x, y, z, radius] at a time (as the claws tool: SPHERE at socket 1 + its relative location
// in the socket frame; CAPSULE2 between socket 1 and 2: the middle, radius + half the length; CAPSULE1 / CONVEX at socket 1
// with m_Size.X; owner-attached shapes at Sora's `center` (0, 0, 91) as combat/hit.rs takes it). The growing sizes
// (m_IncSize / m_IncMaxSize) are in the shape rows, applied at run time.
function shapeAt(s, logical, clip, time) {
  // CONVEX = tres_colltest01 (radius 100 under m_Scale): its horizontal radius
  const radius = s.shape === 'CONVEX' ? 100 * s.scale[0] : s.size[0];
  if (s.attach === 'OWNER_ATTACHED_MESH') return [s.loc[0], s.loc[1], 91 + s.loc[2], radius];
  const T1 = weaponSocketT(s.part, s.socket1, logical, clip, time);
  if (!T1) return null;
  const a = actor(point(T1, s.loc));
  if (s.shape !== 'CAPSULE2' || s.socket2 === 'None') return [...a, radius];
  const T2 = weaponSocketT(s.part, s.socket2, logical, clip, time);
  if (!T2) return [...a, s.size[0]];
  const b = actor(point(T2, [0, 0, 0]));
  const half = Math.hypot(b[0] - a[0], b[1] - a[1], b[2] - a[2]) / 2;
  return [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2, s.size[0] + half];
}
// (F-SHAPES) THE CONVEX SHAPES' REAL TRANSFORM (combat/hull.rs; Update 0x140647950 case 6 / 8): T1 = the socket transform (NORMAL
// the weapon's socket / bone, OWNER_ATTACHED_MESH Sora's socket); location = T1.translation + (m_bAbsoluteOffset1 ? relLoc x T1.scale :
// T1.rotation (relLoc x T1.scale)); rotation = m_RotAttachType LOCAL ? relRot (in the world) : T1.rotation x relRot; scale =
// FUN_140649BF0(t) x T1.scale (not with m_bIgnoreParentScale). -> [x, y, z, qx, qy, qz, qw, sx, sy, sz], the location and the
// rotation in actor space (his yaw 0; a LOCAL rotation in the world), as combat/hull.rs `VolTrack` reads it.
const MESH_YAW_Q = [0, 0, Math.sin(yaw / 2), Math.cos(yaw / 2)];
const growScale = (s, t) => { if (s.iscale.every((x) => x === 0)) return s.scale; return s.scale.map((b, i) => { const v = Math.max(b + s.iscale[i] * t, 0.01); return s.mscale[i] > 0 ? Math.min(v, s.mscale[i]) : v; }); };
function soraSocketT(name, clip, time) {
  const sa = soraAnim(clip);
  if (!sa) return null;
  // a socket of the skeleton / mesh, else a bone of that name (GetSocketTransform falls back to the bone)
  const sock = SORA_SOCKETS[name];
  const bone = SORA_BONES.findIndex((b) => b.name === (sock ? sock.bone : name));
  if (bone < 0) return null;
  const B = boneT(SORA_BONES, byBone(sa), bone, Math.min(time, sa.length), sa.props.bEnableRootMotion === true);
  return sock ? under(sock.T, B) : B;
}
function hullOf(s, T1, t) {
  const sl = [s.loc[0] * T1.s[0], s.loc[1] * T1.s[1], s.loc[2] * T1.s[2]];
  const off = s.abs1 ? sl : qrot(T1.q, sl);
  const c = actor([T1.t[0] + off[0], T1.t[1] + off[1], T1.t[2] + off[2]]);
  const q = s.rot_attach === 'TRES_ACRA_LOCAL' ? rotQ(s.rot) : qmulH(MESH_YAW_Q, qmulH(T1.q, rotQ(s.rot)));
  const n = Math.hypot(...q);
  const g = growScale(s, t);
  const sc = s.ignore_scale ? g : g.map((x, i) => x * T1.s[i]);
  return [...c.map(r1), ...q.map((x) => Math.round(x / n * 1e6) / 1e6), ...sc.map(r4)];
}
function hullAt(s, logical, clip, at, t) {
  const T1 = s.attach === 'OWNER_ATTACHED_MESH' ? soraSocketT(s.socket1, clip, at) : weaponSocketT(s.part, s.socket1, logical, clip, at);
  return T1 ? hullOf(s, T1, t) : null;
}
const volFlags = (s) => ['HULL', s.mesh, s.rot_attach === 'TRES_ACRA_LOCAL' ? 'world' : 'actor', s.owner + ':' + s.shape + ':' + s.socket1 + (s.frozen ? ':frozen' : '') + ':' + s.rot_attach];
// the Clock's shape at its clip time (the Clock's own skeleton, its root at the spawn point with Sora's yaw). (F-SHAPES) Its hull's
// whole transform ([`hullOf`]) instead of the centre alone; `t` = the time since the group's window opened (the growth).
function clockHullAt(s, time, t) {
  const w = weapon('w_so114');
  if (!w) return null;
  const { a } = weaponAnim(w, 'M_L421', 'L421');
  const k = w.bones.findIndex((b) => b.name === s.socket1);
  if (k < 0) return null;
  const T = boneT(w.bones, byBone(a), k, a ? Math.min(time, a.length) : 0, false);
  return hullOf(s, T, t);
}

// ------------------------------------------------------------------------------------------------ notifies of a clip
function notes(clip, group, form, kind) {
  const it = item(clip);
  const out = { length: it ? r4(it.duration_s) : 0, frames: it ? it.num_frames : 0, windows: [], rows: [], change: -1, input: -1, effects: [], sounds: [] };
  if (!it) return out;
  for (const n of it.notifies || []) {
    if (n.class === 'TresAnimNotifyState_Attack') {
      const g = String(prop(n, 'CollisionGrpName')), row = renamed(form, String(prop(n, 'm_AttackDataIDName')), kind), iv = prop(n, 'AttackInterval') || 0;
      out.windows.push(r4(tOf(n)) + '-' + r4(eOf(n)) + '@' + g + '@' + row + '@' + r4(iv));
      if (!out.rows.includes(row)) out.rows.push(row);
    }
    if (n.class === 'TresAnimNotify_EnableChange' && out.change < 0) out.change = r4(tOf(n));
    if (n.class === 'TresAnimNotify_EnableInput' && out.input < 0) out.input = r4(tOf(n));
    if (/AttachEffect/.test(n.class)) {
      const s = JSON.stringify(n.properties || {});
      const re = /"GroupID","type":"IntProperty","value":(-?\d+)[\s\S]*?"AttachEffect","type":"ObjectProperty","value":"([^"]*)"/g;
      let m; while ((m = re.exec(s))) { const g = +m[1]; if ((g === group || g === -1) && m[2] !== 'null') out.effects.push(r4(tOf(n)) + '@' + m[2].split('.').pop()); }
    }
    if (/PlaySound|AttachSound|PlayVoice|WeaponSwingSE|WeaponHitSE/.test(n.class)) { const s = JSON.stringify(n.properties || {}); const m = s.match(/\/Game\/Sound[^"]*/) || s.match(/SWING\d|HIT\d/); out.sounds.push(r4(tOf(n)) + '@' + (m ? m[0].split('.').pop() : n.class.replace(/^.*Notify(State)?_/, ''))); }
  }
  out.effects = [...new Set(out.effects)];
  return out;
}

// ------------------------------------------------------------------------------------------------ rows
const ROWS = [];
const blank = { form: '-', code: 0, logical: '-', clip: '-', length: 0, group: 0, root_len: 0, stop_dist: 0, z_gain: 0, air_mode: false, chan_xy: true, chan_root: false, flags: [], windows: [], rows: [], effects: [], sounds: [], values: [], ref_addr: '-', ref_conf: 'confirmed', note: '-' };
const add = (r) => ROWS.push({ ...blank, ...r });
const tracksWanted = []; // [form, logical, clip]
const muzzles = []; // [form, logical, clip]: the clips whose Shot notify spawns at the weapon's ATK_01
for (const form of CODES) {
  const [lo, hi] = FAMILY[form];
  for (const k of PLAY.filter((x) => (x.forms ? x.forms.includes(form) : x.kind >= lo && x.kind <= hi))) {
    const clip = rowOf(form, k.logical);
    must(clip !== '-', form + ' ' + k.logical + ': no clip');
    const group = k.group !== undefined ? k.group : isToon(form) ? TOON_GROUP : 0;
    const n = notes(clipOf(form, k.logical), group, form, k.kind);
    for (const r of n.rows) must(atkRow(r), form + ' ' + k.logical + ': p_ex001_AttackData has no row ' + r);
    const flags = [];
    for (const f of ['dive', 'no_vz', 'keep_dir_z', 'fixed_dir_z', 'invincible', 'weapon_display', 'air_if_air', 'air_is_air', 'swing', 'shot']) if (k[f]) flags.push(f);
    if (k.variant) flags.push(k.variant);
    const near = k.near || [0, 0, 2.5, false];
    if (k.near && near[3]) flags.push('near_root');
    add({ id: 'attack.' + form + '.' + hex(k.kind) + (k.variant ? '.' + k.variant : ''), what: 'attack', form, code: k.kind, logical: k.logical, clip, length: n.length, group, root_len: k.root || 0, stop_dist: k.stop === undefined ? 160 : k.stop, z_gain: k.z || 3, air_mode: !!k.air, chan_xy: true, chan_root: !!k.chan_root, flags,
      windows: n.windows, rows: n.rows, effects: n.effects, sounds: n.sounds, values: [n.change, n.input, k.dir_z || 0, near[0], near[1], near[2], k.start || 0, k.voice || 0],
      ref_addr: k.at, note: k.label + '; values = [TresEnableChange, TresEnableInput (-1 none), DirZ (0 none), near distance D, its root-scale divisor, the scale max (fVar32), SetAnimTime at the play (s), SetVoiceGroupNo]; near: dist < D -> the XY channel off (near_root: and the root channel on), root scale clamp(dist / divisor, 0.1, max); windows = start-end@group@row@interval' });
    tracksWanted.push([form, k.logical, clipOf(form, k.logical)]);
    if (k.shot) muzzles.push([form, k.logical, clipOf(form, k.logical)]);
  }
}
// the replacement anim sets (logical name -> clip), per form
for (const form of CODES) {
  for (const [logical, r] of [...ROWS_OF[form]].sort((a, b) => (a[0] < b[0] ? -1 : 1))) {
    const clip = r.file ? r.clip : r.xsi && XSI_ROWS.has(logical) ? logical : '-';
    add({ id: 'anim.' + form + '.' + logical, what: 'anim', form, logical, clip, length: item(r.clip) ? r4(item(r.clip).duration_s) : 0, flags: [r.set, r.cls], note: r.file ? '-' : r.xsi ? 'the XS overlay\'s row (the same sequence)' : 'a ' + r.cls + ' of the set (the locomotion blend spaces): not a row of the animation table (its samples are)' });
  }
}
// the finish command FINISH_DR
for (const form of ['DR', 'DR2']) for (const f of FINISH[form]) {
  const clip = f.logical === '-' ? '-' : rowOf(form, f.logical);
  const n = f.logical !== '-' ? notes(clipOf(form, f.logical), isToon(form) ? TOON_GROUP : 0, form, 0) : null;
  if (f.logical !== '-') must(clip !== '-', form + ' ' + f.logical + ': no clip');
  add({ id: 'finish.' + form + '.' + f.sub, what: 'finish', form, code: f.sub, logical: f.logical, clip, length: n ? n.length : 0, flags: f.flags, windows: n ? n.windows : [], rows: n ? n.rows : [], effects: n ? n.effects : [], sounds: n ? n.sounds : [], values: n ? [n.change, n.input] : [],
    ref_addr: FINISH_AT[form], note: f.what });
  if (f.logical !== '-') tracksWanted.push([form, f.logical, clipOf(form, f.logical)]);
}
// the Clock (FINISH_DR of Clock Drill): its clip L421, hit windows, shapes (tres_colltest01 x m_Scale), m_Param_MaxTime
{
  const mj = readJson(P('build', 'sora_pack', 'keyblades', 'w_so114', 'model.json'));
  const c = mj.clips.find((x) => x.sequence === 'L421');
  must(c, 'w_so114 has no L421');
  const wins = (c.notifies || []).filter((n) => n.class === 'TresAnimNotifyState_Attack').map((n) => r4(n.trigger_time) + '-' + r4(n.end_trigger_time) + '@' + n.properties.CollisionGrpName + '@' + n.properties.m_AttackDataIDName + '@' + r4(n.properties.AttackInterval || 0));
  const bp = readJson(path.join(PKG, 'StylePawn', 'w_so114.uasset.json'));
  const cdo = bp.exports.find((e) => /^Default__/.test(e.objectName));
  const maxTime = ((cdo.data && cdo.data.props) || cdo.props || []).find((p) => p.name === 'm_Param_MaxTime');
  must(maxTime, 'w_so114: no m_Param_MaxTime');
  add({ id: 'pawn.DR2.w_so114', what: 'pawn', form: 'DR2', logical: 'M_L421', clip: 'w_so114', length: r4(c.duration_s), windows: wins, rows: [...new Set(wins.map((w) => w.split('@')[2]))], values: [maxTime.value],
    ref_addr: '0x1413D4990', note: 'the Clock (TresStylePawnDRClock, StylePawn/w_so114): its clip L421 and its hit windows (start-end@group@row@interval); values = [m_Param_MaxTime s, read from the blueprint\'s class default]' });
  for (const s of CLOCK_SHAPES) {
    // tres_colltest01: the 20-gon prism of radius 100, z -50 .. 50 (combat/hull.rs) under m_Scale (+ m_IncScale per s, capped by m_IncMaxScale)
    add({ id: 'shape.DR2.w_so114.' + s.group, what: 'shape', form: 'DR2', logical: s.group, clip: 'w_so114', flags: [s.shape, s.socket1, s.socket2, s.attach], values: [...s.scale, ...s.iscale, ...s.mscale, ...s.loc], rows: [s.row],
      note: 'TresAtkColl0 of StylePawn/w_so114: flags = [shape (CONVEX = tres_colltest01), bone, -, attach], values = [m_Scale x, y, z, m_IncScale x, y, z, m_IncMaxScale x, y, z, m_RelativeLocation1 x, y, z]' });
    // (F-SHAPES) the hull's real transform per frame (30 a second) of L421: the `wep` bone's FK under the Clock's clip, its
    // m_RelativeRocation (roll 90: the prism lies on its side), its growth from the start of the group's window (FinishDR2 +10/s).
    const mine = wins.map((w) => w.split('@')).filter((w) => w[1] === s.group).map((w) => +w[0].split('-')[0]);
    const n = Math.max(1, Math.ceil(c.duration_s * 30));
    const vals = [];
    for (let i = 0; i <= n; i++) {
      const time = i * c.duration_s / n;
      const start = mine.filter((a) => a <= time + 1e-6).pop();
      const v = clockHullAt(s, time, start === undefined ? 0 : time - start);
      must(v, 'clock shape ' + s.group);
      vals.push(...v);
    }
    add({ id: 'vol.DR2.L421.' + s.group + '.0', what: 'vol', form: 'DR2', code: n + 1, logical: s.group, clip: 'w_so114', length: r4(c.duration_s), flags: volFlags(s), values: vals, ref_addr: '0x140647950',
      note: 'the Clock\'s hull per frame of L421 (combat/hull.rs VolTrack HULL [x, y, z, qx, qy, qz, qw, sx, sy, sz]: Update 0x140647950 case 6\'s location / rotation / scale in the Clock\'s root space = Sora\'s at the spawn, his yaw 0), FK of the Clock\'s skeleton under its L421' });
  }
}
// the row renames
for (const form of Object.keys(RENAME)) for (const [a, b, k] of RENAME[form]) {
  add({ id: 'rename.' + form + '.' + a + (k > 0 ? '.' + hex(k) : '') + (k < 0 ? '.dodge' : ''), what: 'rename', form, code: k, logical: a, clip: b, ref_addr: k < 0 ? '0x14137B880' : RENAME_AT[form], note: k > 0 ? 'in this form an attack window of row logical hits with row clip when the attack kind is code' : k < 0 ? 'the dodge roll\'s window (MoleDodge) in form 0x93' : 'in this form an attack window of row logical hits with row clip' });
}
// the collision shapes of the form weapons
for (const form of CODES) for (const s of SHAPES[form]) {
  add({ id: 'shape.' + form + '.' + s.owner + '.' + s.group, what: 'shape', form, logical: s.group, clip: s.owner, flags: [s.shape, s.socket1, s.socket2, s.attach, s.part.socket], values: [...s.size, ...s.loc, ...s.inc, ...s.incmax, ...s.scale, s.frozen ? 1 : 0], rows: [s.row],
    note: 'TresAtkColl0 CollisionShapesSrc of Player/p_ex001/' + s.owner + '_Wep: flags = [shape, socket 1, socket 2, attach type, Sora\'s socket the part hangs on], values = [size x, y, z, location 1 x, y, z, m_IncSize x, y, z, m_IncMaxSize x, y, z, m_Scale x, y, z (CONVEX: tres_colltest01, radius 100 / half height 50 under it), m_bDisableLocation1Attach (1: the place is frozen after the first tick)]' });
}
// THE SHAPE TRACKS: per frame the conservative sphere [x, y, z, radius] (actor space, his yaw 0) of each shape of each group
// a clip's windows name, on the form's weapon (FK of Sora's clip, the part's socket, the weapon's own clip).
let tracked = 0;
const seen = new Set();
for (const [form, logical, clip] of tracksWanted) {
  const it = item(clip);
  if (!it) continue;
  const groups = [...new Set((it.notifies || []).filter((n) => n.class === 'TresAnimNotifyState_Attack').map((n) => String(prop(n, 'CollisionGrpName'))))];
  for (const g of groups) {
    const shapes = SHAPES[form].filter((s) => s.group === g);
    for (const [i, s] of shapes.entries()) {
      // (F-SHAPES) a CONVEX: its hull's real transform per frame (`vol` row, combat/hull.rs) instead of a sphere of its radius;
      // the location and rotation at the window's start for m_bDisableLocation1Attach, the scale's growth from the window's start
      if (s.shape === 'CONVEX') {
        const vid = 'vol.' + form + '.' + clip + '.' + g + '.' + i;
        if (seen.has(vid)) continue;
        seen.add(vid);
        const mine = (it.notifies || []).filter((n) => n.class === 'TresAnimNotifyState_Attack' && String(prop(n, 'CollisionGrpName')) === g).map((n) => ({ a: tOf(n), b: eOf(n) }));
        const frames = it.num_frames, len = it.duration_s;
        const vals = [];
        for (let f = 0; f < frames; f++) {
          const time = frames > 1 ? f * len / (frames - 1) : 0;
          const w = mine.filter((x) => x.a <= time + 1e-6).pop() || mine[0];
          const v = hullAt(s, logical, clip, s.frozen ? w.a : time, Math.max(time - w.a, 0));
          if (!v) { vals.length = 0; break; }
          vals.push(...v);
        }
        must(vals.length, form + ' ' + clip + ' ' + g + ': no FK of the hull');
        const wa = weapon(s.part.model) ? weaponAnim(weapon(s.part.model), logical, clip) : { seq: '-' };
        add({ id: vid, what: 'vol', form, code: frames, logical: g, clip, length: r4(len), flags: [...volFlags(s), wa.seq], windows: mine.map((w) => r4(w.a) + '-' + r4(w.b)), values: vals, ref_addr: '0x140647950',
          note: 'shape ' + i + ' of group ' + g + ' on ' + s.owner + ' per frame (combat/hull.rs VolTrack): flags = [HULL, the CONVEX mesh, world | actor (the frame of the rotation: m_RotAttachType LOCAL = relRot in the world), owner:shape:socket, the weapon\'s own clip]; [x, y, z, qx, qy, qz, qw, sx, sy, sz] = Update 0x140647950 case 6\'s location / rotation / scale (FUN_140649BF0 growth from the window\'s start) in Sora\'s actor space (his yaw 0)' });
        tracked += 1;
        continue;
      }
      const id = 'track.' + form + '.' + clip + '.' + g + '.' + i;
      if (seen.has(id)) continue;
      seen.add(id);
      const frames = it.num_frames, len = it.duration_s;
      const values = [];
      let ok = true;
      for (let f = 0; f < frames; f++) {
        const t = frames > 1 ? f * len / (frames - 1) : 0;
        const v = shapeAt(s, logical, clip, t);
        if (!v) { ok = false; break; }
        values.push(...v.map(r1));
      }
      if (!ok) continue;
      const wa = weapon(s.part.model) ? weaponAnim(weapon(s.part.model), logical, clip) : { seq: '-' };
      add({ id, what: 'track', form, code: frames, logical: g, clip, length: r4(len), flags: [s.owner, s.shape, s.socket1, s.socket2, wa.seq], values, note: 'per frame [x, y, z, radius] (cm, actor space, his yaw 0) of shape ' + i + ' of group ' + g + ' on ' + s.owner + ' (flags: owner, shape, sockets, the weapon\'s own clip: - = its reference pose)' });
      tracked += 1;
    }
  }
}
// THE MUZZLE TRACKS: the form weapon's ATK_01 per frame (the shot location vt+0x1410 -> MuzzleAttachPoint "ATK_01")
for (const [form, logical, clip] of muzzles) {
  const it = item(clip);
  if (!it) continue;
  const part = partsOf(form)[0];
  const values = [];
  for (let f = 0; f < it.num_frames; f++) { const t = it.num_frames > 1 ? f * it.duration_s / (it.num_frames - 1) : 0; const T = weaponSocketT(part, 'ATK_01', logical, clip, t); must(T, form + ' ' + clip + ': no ATK_01'); values.push(...actor(point(T, [0, 0, 0])).map(r1), 0); }
  add({ id: 'track.' + form + '.' + clip + '.ATK_01', what: 'track', form, code: it.num_frames, logical: 'ATK_01', clip, length: r4(it.duration_s), flags: [part.model, 'MUZZLE', 'ATK_01', 'None', '-'], values, note: 'per frame [x, y, z, 0] (cm, actor space, his yaw 0): the ATK_01 of the form weapon, where the TresAnimNotify_Shot of the clip spawns its projectile' });
  tracked += 1;
}
// (F-POLISH) THE CATCH LINK: the drill's X_link_skt per frame of M_6421 / M_6423: StyleFinish slot 148's sub 2 / 5 keep the
// caught pawn (m_pCatchPawn, state 62) linked to it (vt+0x16E8, FUN_143EAC880).
for (const logical of ['M_6421', 'M_6423']) {
  const clip = clipOf('DR', logical);
  const it = item(clip);
  must(it, 'DR ' + logical + ': no clip');
  const part = partsOf('DR').find((p) => weapon(p.model) && weapon(p.model).sockets.X_link_skt);
  must(part, 'DR: no part with X_link_skt');
  const values = [];
  for (let f = 0; f < it.num_frames; f++) { const t = it.num_frames > 1 ? f * it.duration_s / (it.num_frames - 1) : 0; const T = weaponSocketT(part, 'X_link_skt', logical, clip, t); must(T, 'DR ' + clip + ': no X_link_skt'); values.push(...actor(point(T, [0, 0, 0])).map(r1), 0); }
  add({ id: 'track.DR.' + clip + '.X_link_skt', what: 'track', form: 'DR', code: it.num_frames, logical: 'X_link_skt', clip, length: r4(it.duration_s), flags: [part.model, 'LINK', 'X_link_skt', 'None', '-'], values, note: '(F-POLISH) per frame [x, y, z, 0] (cm, actor space, his yaw 0): the drill\'s X_link_skt, where the caught pawn of FINISH_DR is carried (m_pCatchPawn in state 62)' });
  tracked += 1;
}
// StartStyle's tail and the transformation (data for the Rust and the record)
for (const form of CODES) {
  const f = FORMS[form];
  add({ id: 'start.' + form, what: 'start', form, code: f.cmd, logical: f.transform_clip, clip: rowOf(form, f.transform_clip) === '-' ? f.transform_clip : rowOf(form, f.transform_clip), flags: [f.start_extra], values: [f.wear_form, f.time],
    ref_addr: '0x14141A9D0', note: 'StartStyle\'s per-form tail (flags) and the transformation clip; values = [wear form, form time s] (from sora-keyblade-forms)' });
}
// the Barrier Cracker projectile (TresProj_BarrierCrackerSO: m_MainLife, TresAtkColl0 SPHERE m_Size, its row, m_HitEffects)
{
  const d = readJson(path.join(PKG, 'Projectile', 'TresProj_BarrierCrackerSO.uasset.json'));
  const flat = (props) => Object.fromEntries((props || []).map((p) => [p.name, p.value]));
  const cdo = flat(((d.exports.find((e) => /^Default__/.test(e.objectName)) || {}).data || {}).props || d.exports.find((e) => /^Default__/.test(e.objectName)).props);
  const coll = d.exports.find((e) => e.class === 'TresAtkCollComponent');
  const cp = (coll.data ? coll.data.props : coll.props);
  const unit = flat(cp.find((p) => p.name === 'CollisionShapesSrc').value.items[0]);
  const he = flat(cp.find((p) => p.name === 'm_HitEffects').value.items[0]);
  const imp = (i) => (i < 0 ? d.imports[-i - 1].objectName : '-');
  add({ id: 'proj.BARRIERCRACKER', what: 'proj', form: 'HM2', logical: 'BARRIERCRACKER', clip: 'TresProj_BarrierCrackerSO', rows: [unit.m_DefaultAttackDataIDName], effects: [imp(he.m_PawnHitEffect)], values: [cdo.m_MainLife, unit.m_Size.x], flags: [cdo.m_bNoDestroyOnHit ? 'no_destroy_on_hit' : '-'],
    ref_addr: '0x141344580', note: 'w_so110_ProjSet BARRIERCRACKER: values = [m_MainLife s, the SPHERE m_Size.X cm (static: InitialSpeed 0)]; rows = its default attack row; effects = m_HitEffects[0].m_PawnHitEffect' });
}
// the attack rows the forms hit with (p_ex001_AttackData), for the combat sheet
const usedRows = new Set(ROWS.flatMap((r) => (['attack', 'finish', 'pawn', 'proj'].includes(r.what) ? r.rows : r.what === 'rename' ? [r.logical, r.clip] : [])).filter((x) => x && x !== 'None'));
for (const extra of ['SCHM_CHANGE', 'SCHM_CHANGE_END', 'HM2_CHANGE', 'HM2_CHANGE_END', 'SCDR_CHANGE', 'DR2_CHANGE', 'SCDR_DODGE', 'DR2_DODGE']) usedRows.add(extra);
for (const id of [...usedRows].sort()) {
  const a = atkRow(id);
  must(a, 'p_ex001_AttackData has no row ' + id);
  add({ id: 'row.' + id, what: 'row', form: /^HM2_/.test(id) ? 'HM2' : /^DR2_/.test(id) ? 'DR2' : /^SCDR_/.test(id) ? 'DR' : 'HM', logical: id, values: [a.m_Power, a.m_FormPoint, a.m_ReactionPowerValue], flags: [String(a.m_DamageKind).split('::').pop(), String(a.m_DamageAttribute).split('::').pop()], note: 'p_ex001_AttackData row ' + a.id + ': values = [m_Power, m_FormPoint, m_ReactionPowerValue] (carried by sora-combat-attack-data, tools/sora_combat_sheet.js)' });
}
// (AUDIT-B F-HAMMER) THE WEAPONS' / TRANSFORM UNITS' / THE CLOCK'S OWN CLIP NOTIFIES (tools/lib/wnotify_rows.js, F-SHIP's
// columns; kh-core forms/hammer.rs `on_notifies` -> forms/ship.rs `wnotify_pass`). Mounts = the parts' sockets of Sora's
// (sora-keyblade-forms units; R_wep_skt for a transform unit with none), `accompany` = the Clock's own root (FinishView::accompany), not his component (the Clock stands where he
// is held, StyleFinishDR Tick). The form a row plays in: the form whose unit / transform clip it is; the drills' shotlock
// clips (M_6481 .. M_6483 of the Rush shotlocks, rush.rs: weapon form 6 / 0x15, no form held) are `SHOT` rows with the
// weapon form `wf<N>` (PawnMagic::shot_weapon_form).
{
  const { wnotifyRows } = require('./lib/wnotify_rows.js');
  const mountOf = (form, model) => { for (const u of [...FORMS[form].units, ...FORMS[form].transform_units]) { const m = u.match(/^(\w+)@(\w+):(\w+)=(.*)$/); if (m[4].split('/').pop().toLowerCase().replace(/_wep$/, '') === model) return m[3] === 'None' ? 'R_wep_skt' : m[3]; } return null; };
  const seqOf = (form) => clipOf(form, FORMS[form].transform_clip);
  const shot = (c) => (c.logical_names || []).some((n) => /^M_648[1-3]$/.test(n));
  const unitOf = (form, model) => ({ model, mount: mountOf(form, model), form: (c) => (shot(c) ? 'SHOT' : form), extra: (c) => (shot(c) ? ['wf' + FORMS[form].weapon_form_value] : []) });
  const transformOf = (forms, model) => ({ model, mount: mountOf(forms[0], model), form: (c) => forms.find((f) => seqOf(f) === c.sequence) || null });
  const models = [unitOf('HM', 'w_so052'), unitOf('DR', 'w_so053'), unitOf('HM2', 'w_so112'), unitOf('DR2', 'w_so113'), transformOf(['HM', 'DR'], 'w_so051'), transformOf(['HM2', 'DR2'], 'w_so111'),
    { model: 'w_so114', mount: 'accompany', form: (c) => (c.sequence === 'L421' ? 'DR2' : null) }]; // the Clock where it stands (FinishView::accompany; kh-host vfx.rs)
  for (const m of models) must(m.mount, m.model + ': no mount');
  for (const r of wnotifyRows(P('build', 'sora_pack'), models)) add(r);
  for (const [form, seq] of [['HM', '3401'], ['DR', '6401'], ['HM2', 'K401'], ['DR2', 'L401']]) must(seqOf(form) === seq && ROWS.some((r) => r.what === 'wnotify' && r.form === form && r.clip === seq), form + ': no transform-unit rows on ' + seq);
}
// (AUDIT-B F-HAMMER) THE PHYS SHAPES (m_bIsPhysAttackCollision -> prim+0x984 bit 7, slot 334 0x1406457E0): the chooser
// 0x140628960 spawns the entry's m_DirectHitEffect too (vw_so050_str0 / vw_so110_str00, the Clock's vw_so110_str01) for a hit
// of such a primitive. flags = the groups of the form's unit shapes that are phys (DR2: + the Clock's FinishDR / FinishDR2).
for (const form of CODES) {
  const physOf = (rel) => {
    let f = path.join(PKG, rel + '.uasset.json');
    if (!fs.existsSync(f)) f = path.join(PKG, rel.replace(/_Wep$/, '_wep') + '.uasset.json');
    const d = readJson(f);
    const out = [];
    for (const e of d.exports.filter((x) => x.class === 'TresAtkCollComponent')) for (const s of (((e.data && e.data.props) || e.props).find((p) => p.name === 'CollisionShapesSrc') || { value: { items: [] } }).value.items) {
      const g = s.find((x) => x.name === 'm_GrpName'), p = s.find((x) => x.name === 'm_bIsPhysAttackCollision');
      if (p && p.value === true && !out.includes(g.value)) out.push(g.value);
    }
    return out;
  };
  const groups = [...new Set([...partsOf(form).flatMap((p) => physOf('Player/p_ex001/' + p.model + '_Wep')), ...(form === 'DR2' ? physOf('StylePawn/w_so114') : [])])];
  must(groups.length > 0, form + ': no phys shape');
  add({ id: 'phys.' + form, what: 'phys', form, flags: groups, ref_addr: '0x1406457E0', ref_conf: 'confirmed',
    note: '(AUDIT-B F-HAMMER) the shape groups of the form\'s weapon units (and Clock Drill\'s Clock) with m_bIsPhysAttackCollision: a hit of them also spawns the entry\'s m_DirectHitEffect (chooser 0x140628960)' });
}

// ------------------------------------------------------------------------------------------------ checks
must(ROWS.filter((r) => r.what === 'attack').length === 9 * 2 + 10 * 2 + 4, 'expected 9 hammer kinds and 10 drill kinds per form');
for (const r of ROWS.filter((x) => x.what === 'attack')) must(r.clip !== '-' && r.length > 0, r.id + ': no clip');
for (const form of CODES) must(ROWS.some((r) => r.what === 'track' && r.form === form), form + ': no shape track');
// (F-SHAPES) every CONVEX the forms' windows name is a hull row, none a sphere track
must(!ROWS.some((r) => r.what === 'track' && (r.flags || []).includes('CONVEX')), 'a CONVEX shape left as a sphere track');
for (const g of ['StampL', 'SwingS', 'SwingM', 'SwingL', 'FinishDR', 'FinishDR2']) must(ROWS.some((r) => r.what === 'vol' && r.logical === g), g + ': no hull row');
must(ROWS.some((r) => r.id === 'anim.DR2.M_L421' && r.clip === 'L421'), 'Clock Drill has M_L421');

// ------------------------------------------------------------------------------------------------ sheet
const S = (n, t, g, doc) => [n, t, g, doc];
const COLS = [
  S('id', 'str', 'str', '<what>.<form>.<key>'), S('what', 'enum:attack|anim|finish|pawn|rename|shape|track|vol|start|proj|row|wnotify|phys', 'str', 'the row kind (AUDIT-B F-HAMMER: wnotify = a weapon or the Clock: its own clip notify, tools/lib/wnotify_rows.js; phys = the phys shape groups of a form)'), S('form', 'str', 'str', 'HM (Hyper Hammer 0x81), DR (Drill Punch 0x84), HM2 (Boom Hammer 0x92), DR2 (Clock Drill 0x93)'),
  S('code', 'int', 'i32', 'attack kind (attack), finish sub-step (finish), frames (track), the attack kind of a rename (0 any, -1 the dodge), the command (start), else 0'), S('logical', 'str', 'str', 'the logical anim name KH3 plays (attack / anim / finish / start), the group (shape / track), the old row (rename), the row (row)'),
  S('clip', 'str', 'str', 'the animation table row (attack / finish / start / track: the sequence of a form-set clip, a logical name of the base / XS rows); the new row (rename); the owner (shape); the model (pawn)'), S('length', 'num', 'f32', 'the clip length, s'),
  S('group', 'int', 'i32', 'the effect group the play passes (PlayAttack +0x198)'), S('root_len', 'num', 'f32', 'root length (+0x1D8), 0 = none'), S('stop_dist', 'num', 'f32', 'stop distance (+0x24C)'), S('z_gain', 'num', 'f32', 'z gain (+0x14C)'),
  S('air_mode', 'bool', 'bool', '+0x1DF'), S('chan_xy', 'bool', 'bool', '+0x234 bit 0'), S('chan_root', 'bool', 'bool', '+0x234 bit 1'),
  S('flags', 'list', 'str[]', 'the case\'s other writes / the kind\'s strings (see the note)'), S('windows', 'list', 'str[]', 'the clip\'s hit windows start-end@group@row@interval, s (renamed rows)'), S('rows', 'list', 'str[]', 'the attack rows of those windows'),
  S('effects', 'list', 'str[]', 'the clip\'s effects of that group (time@asset)'), S('sounds', 'list', 'str[]', 'the clip\'s sounds / voices (time@asset)'), S('values', 'list', 'f32[]', 'numbers by kind (see the note)'),
  S('ref_addr', 'str', 'str', 'decompiled function'), S('ref_conf', 'enum:confirmed|inferred|guess', 'str', 'confirmed = read in the code or the data; inferred; guess'), S('note', 'str', 'str', '-'),
];
const NAME = 'sora-form-hammer';
const rows = ROWS.map((r) => COLS.map((c) => r[c[0]]));
for (const [i, r] of rows.entries()) for (const [k, v] of r.entries()) must(v !== undefined && v !== null && !(typeof v === 'number' && !Number.isFinite(v)), 'row ' + ROWS[i].id + ': cell ' + COLS[k][0] + ' is ' + v);
const ids = new Set(); for (const r of ROWS) { must(!ids.has(r.id), 'duplicate id ' + r.id); ids.add(r.id); }
fs.writeFileSync(P('sheets', 'port', NAME + '.json'), '{"sheet":"' + NAME + '","group":"port","columns":' + JSON.stringify(COLS.map((c) => c[0])) + ',"rows":[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
const by = {}; for (const r of ROWS) by[r.what] = (by[r.what] || 0) + 1;
console.log('sheets/port/' + NAME + '.json: ' + rows.length + ' rows ' + JSON.stringify(by) + ', ' + tracked + ' shape tracks');

// ------------------------------------------------------------------------------------------------ register (this tool's entry only; replaced in place)
const q = (s) => "'" + String(s).replace(/\\/g, '\\\\').replace(/'/g, "\\'") + "'";
const NL = '\n';
const DOC = 'KH3\'s Hyper Hammer / Drill Punch / Boom Hammer / Clock Drill as data for their form package (kh-core combat/forms/hammer.rs, states/combo_hm.rs): the attack kinds of COMBO_HM / COMBO_DR, the replacement anim sets, FINISH_DR (the drill / the Clock), the row renames, the weapon shapes and their per-frame tracks.';
const MARK = '(weapons F-HAMMER)';
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
  const text = '  // ' + MARK + ' ' + DOC + ' Written by tools/sora_form_hammer.js (documentation/KH3_WEAPONS_PLAN.md 1.3)' + NL +
    '  ' + q(NAME) + ": { group: 'port', file: " + q(NAME + '.json') + ", by: 'tools/sora_form_hammer.js', from: " + q('sora-keyblade-forms, the form anim sets of the asset pack, the form weapons and the Clock (blueprints, skeletons, clips), Sora\'s skeleton, p_ex001_AttackData and the decompiled PlayAttack / ComboHM / ComboDR / StyleFinish / StyleFinishDR') + ', external: true, cols: [' + NL +
    '    ' + COLS.map((c, i) => '[' + q(c[0]) + ', ' + (i === 0 ? 'K' : 'V') + ', ' + q(c[1]) + ", '-', " + q(c[3]) + (['note', 'flags', 'windows', 'rows', 'effects', 'sounds', 'values', 'logical', 'clip'].includes(c[0]) ? ', true' : '') + ']').join(', ') + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/sheet_defs.js');
}
{
  const f = P('tools', 'lib', 'port_gen_defs.js'); const before = fs.readFileSync(f, 'utf8');
  const END = NL + '    },' + NL;
  const pos = (t) => { const anchor = '  tables: {' + NL; const at = t.indexOf(anchor); must(at >= 0, 'port_gen_defs.js: anchor tables'); return at + anchor.length; };
  const cols = COLS.filter((c) => c[2]).map((c) => c[0] + ': { type: ' + q(c[2]) + (c[3] && c[3] !== '-' ? ', doc: ' + q(c[3]) : '') + ' }').join(', ');
  const text = '    // ' + MARK + ' ' + NAME + ': written by tools/sora_form_hammer.js (documentation/KH3_WEAPONS_PLAN.md 1.3)' + NL +
    '    ' + q(NAME) + ': {' + NL +
    "      file: 'sora_form_hammer.rs', row: 'FormHammerRow', table: 'FORM_HAMMER', key: 'id', key_prefix: 'FHM'," + NL +
    '      doc: ' + q(DOC) + ',' + NL +
    '      columns: { ' + cols + ' },' + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/port_gen_defs.js');
}
console.log('sora_form_hammer ' + VERSION);
