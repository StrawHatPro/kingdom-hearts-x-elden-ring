// (weapons F-SHIELD) KH3's two shield formchanges as DATA for their form package (kh-core combat/forms/shield.rs,
// states/combo_sh.rs): Counter Shield (form.SH, Hero's Origin) and Frying Pan (form.SH2, Grand Chef). The attack kinds of
// COMBO_SH (0x50 .. 0x57), the replacement anim sets (AnimSet11, AnimSet19: logical name -> clip), the grow levels (the
// guard that raises them, the attack that spends them, the weapon parts), the finish command FINISH_SH (sub-steps, clips,
// its style pawns), the attack-row rename of Frying Pan, the weapon / pawn collision shapes and the Attack command labels.
// documentation/KH3_WEAPONS_PLAN.md 1.3 / 4 (package F-SHIELD), build/sora_tmp/weapons/F-SHIELD/STATE.md.
//
//   node tools/sora_form_shield.js   write sheets/port/sora-form-shield.json and register it (once, in place) in
//                                    tools/lib/sheet_defs.js and tools/lib/port_gen_defs.js; then node tools/gen_port.js
//
// Inputs (read only): sheets/port/sora-keyblade-forms.json (P3: the form rows), build/sora_pack/keyblades/
// sora_forms_manifest.json (the asset agent's pack of the form anim sets: logical rows, clips, notifies), build/sora_pack/
// manifest.json (the base set's clips), the weapon / style-pawn blueprints (TresAtkColl0 CollisionShapesSrc), sheets/
// entities/DataTable/Player/Attack/p_ex001_AttackData.json, and THE CODE TABLES BELOW, read in the decompiled exe (re/decomp):
//   PLAY     TresPlayerState_Attack::PlayAttack 0x1413378B0 cases 0x50 .. 0x57: clip, traction channels, stop distance,
//            z gain, root length, air mode, the effect group of form 0x91, the finish flags of 0x55 / 0x56.
//   DECIDE   TresPlayerState_ComboSH::Decide 0x1413632A0 (vtable 0x146910FF8 slot 122): the combo (data in the Rust).
//   GROW     TresPlayerState_Guard slot 118 0x14139D2D0 (the shield guard tick, weapon forms 0x0B / 0x13: actions spec
//            table 0x141385898): a SUCCESSFUL guard hit plays GuardChange (grow LV1 -> LV2) / GuardChange2 (LV2 -> LV3) and
//            calls GrowUp (pawn slot 638 0x1405BD810: ChangeWeapon(grow + 1)); ComboSH Decide: grow LV2 -> kind 0x55, LV3 ->
//            0x56; ComboSH Tick 0x141353A80 / End 0x141350750: at TresEnableChange of 0x55 / 0x56 the weapon goes back to
//            LV1 (pawn slot 637 0x1405BD770 (0, 0, 0)).
//   FINISH   TresPlayerState_StyleFinishSH (id 38; TryStyleFinish 0x141447840 case 0xA3): Begin 0x1413CC390, Tick
//            0x1413D5E50 (Pegasus + Chariot), slot 167 0x1413E1210 (Frying Pan, form 0x91), End 0x1413CF260.
//   RENAME   0x141357590: in form 0x91 the SCSH_* attack rows of the shared clips are SH2_*.
//   VOICE    StyleChange Begin 0x1413CBA70 case 0x89: +0xA4 = 2 = the voice group number (anim interface vt+0x1F0
//            SetVoiceGroupNo) of the transformation clip; every other form 0.
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
const SH = FORMS.find((r) => r.id === 'form.SH');
const SH2 = FORMS.find((r) => r.id === 'form.SH2');
must(SH && SH2, 'sora-keyblade-forms has no form.SH / form.SH2');
const OV = readJson(P('build', 'sora_pack', 'keyblades', 'sora_forms_manifest.json'));
const MAIN = readJson(P('build', 'sora_pack', 'manifest.json'));
const ATK = readJson(P('sheets', 'entities', 'DataTable', 'Player', 'Attack', 'p_ex001_AttackData.json')).rows;
const setName = (a) => String(a).split('/').pop().split('.')[0];
const SETS = { SH: SH.anim_sets.map(setName), SH2: SH2.anim_sets.map(setName) };
const ITEMS = new Map([...MAIN.items, ...OV.items].filter((i) => i.kind === 'animation').map((i) => [i.animation_name, i]));
const BASE = new Map(MAIN.logical_animations.map((a) => [a.name, a]));
// logical name -> clip of a form: its sets' rows, the LAST set first (AnimSet19 over AnimSet11 for Frying Pan: [I] the
// replacement order is the m_ReplaceAnimSets order, the form's own set last), else the base set's row.
function rowsOf(form) {
  const out = new Map();
  for (const set of SETS[form]) for (const a of OV.logical_animations.filter((x) => setName(x.anim_set) === set)) out.set(a.name, { set, clip: a.clip, file: a.file, cls: a.class });
  return out;
}
const ROWS_OF = { SH: rowsOf('SH'), SH2: rowsOf('SH2') };
function clipOf(form, logical) {
  const r = ROWS_OF[form].get(logical);
  if (r) return r.file ? r.clip : '-';
  const b = BASE.get(logical);
  return b && b.file ? path.basename(b.file, '.glb') : '-';
}
const item = (clip) => ITEMS.get(clip) || null;
const prop = (n, key) => { const p = n.properties || {}; const v = p[key]; return v && typeof v === 'object' && 'value' in v ? v.value : v; };
// the notifies of a clip, for the rows: hit windows, their attack rows, TresEnableChange / TresEnableInput, the effects of
// one effect group (the group a play passes: 0 for Counter Shield, the case's 0x867 .. for Frying Pan), the sounds.
function notes(clip, group) {
  const it = item(clip);
  const out = { length: it ? r4(it.duration_s) : 0, windows: [], rows: [], change: -1, input: -1, effects: [], sounds: [] };
  if (!it) return out;
  for (const n of it.notifies || []) {
    if (n.class === 'TresAnimNotifyState_Attack') { out.windows.push(r4(n.time) + '-' + r4(n.end_time)); out.rows.push(String(prop(n, 'm_AttackDataIDName'))); }
    if (n.class === 'TresAnimNotify_EnableChange' && out.change < 0) out.change = r4(n.time);
    if (n.class === 'TresAnimNotify_EnableInput' && out.input < 0) out.input = r4(n.time);
    if (/AttachEffect/.test(n.class)) {
      const s = JSON.stringify(n.properties || {});
      const re = /"GroupID","type":"IntProperty","value":(-?\d+)[\s\S]*?"AttachEffect","type":"ObjectProperty","value":"([^"]*)"/g;
      let m; while ((m = re.exec(s))) { const g = +m[1]; if ((g === group || g === -1) && m[2] !== 'null') out.effects.push(r4(n.time) + '@' + m[2].split('.').pop()); }
    }
    if (/PlaySound|AttachSound|PlayVoice/.test(n.class)) { const s = JSON.stringify(n.properties || {}); const m = s.match(/\/Game\/Sound[^"]*/); out.sounds.push(r4(n.time) + '@' + (m ? m[0].split('.').pop() : n.class.replace(/^.*Notify(State)?_/, ''))); }
  }
  out.effects = [...new Set(out.effects)];
  return out;
}
const atkRow = (id) => ATK.find((r) => r._row_name === id || String(r.id).toLowerCase() === String(id).toLowerCase());

// ------------------------------------------------------------------------------------------------ the code tables
// PLAY: PlayAttack 0x1413378B0, cases 0x50 .. 0x57 (decompiled C): logical name; the case's writes (z gain +0x14C, stop
// distance +0x24C, root length +0x1D8, +0x234 bit 0 the XY channel / bit 1 the root-scale channel, m_bEnableTraction,
// +0x1DF the air mode); +0x198 = the play's effect group in form 0x91 (Counter Shield: 0); the tail LAB_141339926 of 0x55 /
// 0x56: 0x1414145C0(ctrl, 0) the counter window closed, +0x234 |= 0x100 (cancel only at TresEnableChange), finish +
// finish end (+0x140, +0x141, pawn vt+0x2740(1)).
const PLAY = [
  { kind: 0x50, logical: 'M_B411', z_gain: 4.5, stop: 120, group2: 0x867, label: 'ground 1 (Shield Spin)', at: '0x141339B5A' },
  { kind: 0x51, logical: 'M_B412', z_gain: 4.5, root_len: 800, chan_root: true, label: 'ground 2 (the stab)', at: '0x1413397E5' },
  { kind: 0x52, logical: 'M_B413', chan_xy: false, label: 'ground 3: the shield raid M_B413 -> M_B414 -> M_B415 (Shield Charge; ComboSH slot 125 0x141366990)', at: '0x141339845' },
  { kind: 0x53, logical: 'M_B416', air: true, stop: 200, group2: 0x868, flags: ['finish'], label: 'finisher 1 (Lightning / Flame Circle)', at: '0x14133984F' },
  { kind: 0x54, logical: 'M_B417', chan_xy: false, air: true, group2: 0x869, flags: ['finish', 'finish_end'], label: 'finisher end (the second spin)', at: '0x1413398A3' },
  { kind: 0x55, logical: 'M_B418', chan_xy: false, air: true, group2: 0x86A, flags: ['finish', 'finish_end', 'counter_close', 'cancel_at_change', 'invincible', 'world_slow', 'weapon_reset'], label: 'grow LV2: Wrathful Fist / Flame Torrent', at: '0x1413398F7' },
  { kind: 0x56, logical: 'M_B426', chan_xy: false, air: true, group2: 0x86B, flags: ['finish', 'finish_end', 'counter_close', 'cancel_at_change', 'invincible', 'world_slow', 'weapon_reset'], label: 'grow LV3: Wrathful Flurry / Flame Barrage', at: '0x14133994B' },
  { kind: 0x57, logical: 'M_B419', z_gain: 4.5, stop: 120, air: true, traction: true, group2: 0x867, label: 'air (or the target high): the air spin', at: '0x1413397A0' },
];
// GROW: guard slot 118 0x14139D2D0 sub 0 (decompiled C): (grow before the hit) -> the guard clip and the grow after it.
const GROW = [
  { level: 1, guard: 'GuardChange', air: 'AirGuardChange', kind: 0, cmd: 0, cmd2: 0 },
  { level: 2, guard: 'GuardChange2', air: 'AirGuardChange2', kind: 0x55, cmd: 0x10, cmd2: 0x12 },
  { level: 3, guard: 'GuardHit', air: 'AirGuardHit', kind: 0x56, cmd: 0x11, cmd2: 0x13 },
];
// FINISH: StyleFinishSH Tick 0x1413D5E50 (Counter Shield) and slot 167 0x1413E1210 (Frying Pan), by sub-step.
const FINISH = {
  SH: [
    { sub: 0, logical: 'M_B423', what: 'the call: Sora M_B423, Flying (pawn vt+0x17A0(5)); Pegasus (m_ReplaceAccompanyPawnAsset of FINISH_SH) and its Chariot (m_SHChariotAssets) spawned beside him, hidden; the fixed camera (100, 0) on Sora', flags: ['spawn_pawns', 'flying', 'camera_fix'] },
    { sub: 1, logical: 'M_B424', what: 'at M_B423\'s end Sora, Pegasus and the Chariot play M_B424 (the pawns run in)', flags: ['pawns_play'] },
    { sub: 2, logical: '-', what: 'both pawns have their mesh: the Chariot is linked to Pegasus at X_Link_skt and Sora to the Chariot; Sora\'s weapon put away (vt+0x1340(0, 0, 1)); the pawns shown; the camera on Pegasus (offset (0, 0, 100), pitch -25, distance 700)', flags: ['ride', 'camera_ride'] },
    { sub: 3, logical: 'M_B425', what: 'at M_B424\'s end the three play M_B425 (the ride; Sora\'s row is a blend space on the pawn skeleton, not packed); Pegasus starts (0x1413ECE80); the ride timer = Pegasus vt+0x2620 m_Param_MaxTime; the world slow off; the extra FINISH extra: shield thunder (0xAA) shown', flags: ['timer', 'extra_0xAA', 'world_slow_off'] },
    { sub: 4, logical: '-', what: 'the ride: the timer runs (the HUD gauge controller vt+0x1B38); Pegasus flies by the stick (0x1413E34B0); the extra used (pending 0xAA) and Pegasus able (0x141419A50): a thunder (m_ProjThunder TresProjSHThunder) and the pending cleared; ends when the timer is out, slot 87 says so, or Pegasus +0xC60 >= 3', flags: ['rush', 'thunder'] },
    { sub: 5, logical: 'M_1501', what: 'the pawns and cameras cleaned up (0x1413CFB60); Sora M_1501 (the landing)', flags: ['cleanup'] },
    { sub: 6, logical: '-', what: 'one tick', flags: [] },
    { sub: 7, logical: '-', what: 'turn at 18 x dt; at M_1501\'s end DoChain', flags: ['end'] },
  ],
  SH2: [
    { sub: 0, logical: 'M_J423', what: 'the Frying Pan pawn (m_ReplaceAccompanyPawnAsset of FINISH_SH) spawned at Sora with his rotation, shown; both play M_J423; weapon away; m_bFaceAt; homing target = the controller\'s target, else the soft target', flags: ['spawn_pawns', 'face_at'] },
    { sub: 2, logical: '-', what: 'the pan starts (pan +0xB48 = 1), then the world slow off', flags: ['world_slow_off'] },
    { sub: 4, logical: '-', what: 'until TresEnableChange of M_J423 (or its end): the pan\'s time (m_Param_MaxTime) is never counted down here', flags: [] },
    { sub: 5, logical: '-', what: 'the pan cleaned up (0x1413CFB60)', flags: ['cleanup'] },
    { sub: 6, logical: '-', what: 'at M_J423\'s end DoChain', flags: ['end'] },
  ],
};
// RENAME: 0x141357590 (decompiled C), form 0x91 only.
const RENAME = ['SCSH_TURN', 'SCSH_STAB', 'SCSH_RAID', 'SCSH_SPIN', 'SCSH_FINISH', 'SCSH_FINISH2', 'SCSH_COUNTER', 'SCSH_COUNTER2', 'SCSH_COUNTER2_END'];
// SHAPES: TresAtkColl0 CollisionShapesSrc of the weapon parts and the style pawns ([A] the blueprints).
const SHAPE_OWNERS = ['Player/p_ex001/w_so032_wep', 'Player/p_ex001/w_so033_wep', 'Player/p_ex001/w_so034_wep', 'Player/p_ex001/w_so037_wep', 'Player/p_ex001/w_so142_Wep', 'Player/p_ex001/w_so143_Wep', 'Player/p_ex001/w_so144_Wep', 'StylePawn/w_so036', 'StylePawn/w_so145'];
const PKG = P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content', 'Blueprints');
function shapes(rel) {
  const d = readJson(path.join(PKG, rel + '.uasset.json'));
  const flat = (props) => { const o = {}; for (const p of props || []) o[p.name] = p.type === 'StructProperty' ? (Array.isArray(p.value) ? flat(p.value) : p.value) : p.type === 'ArrayProperty' ? (p.value && p.value.items ? p.value.items.map((it) => (Array.isArray(it) ? flat(it) : it)) : p.value) : p.value; return o; };
  const out = [];
  for (const e of d.exports) {
    if (e.class !== 'TresAtkCollComponent') continue;
    const p = flat((e.data && e.data.props) || e.props);
    for (const s of p.CollisionShapesSrc || []) {
      const v = (o) => (o ? [o.x || 0, o.y || 0, o.z || 0] : [0, 0, 0]);
      out.push({ owner: rel.split('/').pop(), group: s.m_GrpName, shape: String(s.m_ShapeType).split('::').pop(), size: v(s.m_Size), socket1: s.m_SocketName1 || 'None', socket2: s.m_SocketName2 || 'None', loc: v(s.m_RelativeLocation1), row: s.m_DefaultAttackDataIDName || 'None', attach: String(s.m_AttachType1 || '').split('::').pop(),
        // (F-SHAPES) what the CONVEX transform reads (Update 0x140647950 case 6 / 8; combat/hull.rs)
        mesh: s.m_Mesh < 0 && d.imports[-s.m_Mesh - 1] ? d.imports[-s.m_Mesh - 1].objectName : '-', scale: v(s.m_Scale), inc_scale: v(s.m_IncScale), max_scale: v(s.m_IncMaxScale),
        rot: s.m_RelativeRocation ? { pitch: s.m_RelativeRocation.pitch || 0, yaw: s.m_RelativeRocation.yaw || 0, roll: s.m_RelativeRocation.roll || 0 } : { pitch: 0, yaw: 0, roll: 0 },
        rot_attach: String(s.m_RotAttachType || 'TRES_ACRA_NORMAL').split('::').pop(), abs1: s.m_bAbsoluteOffset1 === true, ignore_scale: s.m_bIgnoreParentScale === true, frozen: s.m_bDisableLocation1Attach === true });
    }
  }
  return out;
}

// ------------------------------------------------------------------------------------------------ rows
const ROWS = [];
// (AUDIT-A SHIELD) a thrown row's [trigger, end] on the THROWN weapon's own clip clock (time 0 = the throw)
const THROWN_CLOCK = new Map();
// (AUDIT-B W-VFX2) a form weapon's THUNDER row (TypeDataSQEX_Thunder: a beam whose ends KH3 reads on the mesh it hangs on,
// 0x144250690): the host-resolved anchor `kb:<model>:<mount>:<socket>[:to<EndSocketName>]` (tools/lib/wnotify_rows.js
// `thunderAnchor`), which its wnotify row spawns on instead of `pawn:<row>` so the host finds both ends on the weapon
const THUNDER_KB = new Map();
const WNOTIFY_LIB = require('./lib/wnotify_rows.js'); // (AUDIT-B W-VFX2)
const SRC_FORMS = 'sheets/port/sora-keyblade-forms.json (P3)';
const SRC_PACK = 'build/sora_pack/keyblades/sora_forms_manifest.json (the asset agent: AnimSet11 / AnimSet19 rows, clips, notifies)';
const blank = { form: '-', code: 0, logical: '-', clip: '-', length: 0, group: 0, root_len: 0, stop_dist: 0, z_gain: 0, air_mode: false, traction: false, chan_xy: true, chan_root: false, flags: [], windows: [], rows: [], effects: [], sounds: [], values: [], ref_addr: '-', ref_conf: 'confirmed', note: '-' };
const add = (r) => ROWS.push({ ...blank, ...r });
// attacks, per form (the group and the rows differ)
for (const form of ['SH', 'SH2']) {
  for (const k of PLAY) {
    const clip = clipOf(form, k.logical);
    must(clip !== '-', form + ' ' + k.logical + ': no clip');
    const group = form === 'SH2' ? (k.group2 || 0) : 0;
    const n = notes(clip, group);
    const rows = form === 'SH2' ? n.rows.map((r) => (RENAME.includes(r) ? r.replace(/^SCSH_/, 'SH2_') : r)) : n.rows;
    for (const r of rows) must(atkRow(r), form + ' ' + k.logical + ': p_ex001_AttackData has no row ' + r);
    add({ id: 'attack.' + form + '.' + hex(k.kind), what: 'attack', form, code: k.kind, logical: k.logical, clip, length: n.length, group, root_len: k.root_len || 0, stop_dist: k.stop || 160, z_gain: k.z_gain || 3, air_mode: !!k.air, traction: !!k.traction, chan_xy: k.chan_xy !== false, chan_root: !!k.chan_root, flags: k.flags || [],
      windows: n.windows, rows, effects: n.effects, sounds: n.sounds, values: [n.change, n.input], ref_addr: k.at, ref_conf: 'confirmed', note: k.label + '; values = [TresEnableChange, TresEnableInput] of the clip (-1 none); defaults stop 160, z gain 3 (P.traction_*)' });
  }
}
// the M_B413 raid chain (ComboSH slot 125 0x141366990: M_B413 -> M_B414 at its end -> M_B415 at its end or a queued attack)
for (const form of ['SH', 'SH2']) for (const [i, logical] of ['M_B414', 'M_B415'].entries()) {
  const clip = clipOf(form, logical);
  const n = notes(clip, 0);
  add({ id: 'chain.' + form + '.' + logical, what: 'chain', form, code: 0x52, logical, clip: clip === '-' ? 'B' + logical.slice(3) : clip, length: n.length, flags: i === 1 ? ['weapon_fx_end'] : ['enable_input_on_hit'], windows: n.windows, rows: form === 'SH2' ? n.rows.map((r) => (RENAME.includes(r) ? r.replace(/^SCSH_/, 'SH2_') : r)) : n.rows, effects: n.effects, sounds: n.sounds, values: [n.change, n.input],
    ref_addr: '0x141366990', note: i === 0 ? 'at M_B413\'s end (anim vt+200): played (mode 2, blend 0); TresEnableInput once a hit landed (pawn+0xE24 > 0); a queued attack sets m_bEnableChange' : 'at M_B414\'s end or with an attack queued (+0x143): equipment vt+0x560(7) (every weapon part vt+0x10D0), played (blend -1)' });
}
// the replacement anim sets (logical name -> clip), per form
for (const form of ['SH', 'SH2']) {
  for (const [logical, r] of [...ROWS_OF[form]].sort((a, b) => (a[0] < b[0] ? -1 : 1))) {
    add({ id: 'anim.' + form + '.' + logical, what: 'anim', form, logical, clip: r.file ? r.clip : '-', length: r.file && item(r.clip) ? r4(item(r.clip).duration_s) : 0, flags: [r.set, r.cls], ref_addr: '-', note: r.file ? '-' : 'a ' + r.cls + ' of the set: not a row of the animation table (its samples are)' });
  }
}
// the grow levels
for (const form of ['SH', 'SH2']) {
  const row = form === 'SH' ? SH : SH2;
  for (const g of GROW) {
    const units = g.level === 1 ? row.units : row.grow_units.filter((u) => u.startsWith('G' + g.level + ':')).map((u) => u.slice(3));
    add({ id: 'grow.' + form + '.' + g.level, what: 'grow', form, code: g.level, logical: g.guard, clip: clipOf(form, g.guard), length: item(clipOf(form, g.guard)) ? r4(item(clipOf(form, g.guard)).duration_s) : 0, flags: [g.air, clipOf(form, g.air)], rows: units, values: [g.kind, form === 'SH' ? g.cmd : g.cmd2],
      ref_addr: '0x14139D2D0', ref_conf: g.cmd ? 'inferred' : 'confirmed', note: 'a successful guard hit at this grow level plays logical (flags: the air variant) and grows the weapon (level 3: no further grow); rows = the weapon parts of this level; values = [the attack kind Decide gives at this level (0 = the combo), the Attack command label (TresCmdData TRES_CMD_KIND_SHIELDCOUNTER* by name: inferred, the HUD call was not traced)]' });
  }
}
// the finish command
for (const form of ['SH', 'SH2']) for (const f of FINISH[form]) {
  const clip = f.logical === '-' ? '-' : clipOf(form, f.logical);
  const n = clip !== '-' ? notes(clip, 0) : null;
  add({ id: 'finish.' + form + '.' + f.sub, what: 'finish', form, code: f.sub, logical: f.logical, clip, length: n ? n.length : 0, flags: f.flags, windows: n ? n.windows : [], effects: n ? n.effects : [], sounds: n ? n.sounds : [], values: n ? [n.change, n.input] : [],
    ref_addr: form === 'SH' ? '0x1413D5E50' : '0x1413E1210', note: f.what });
}
// the style pawns of FINISH_SH (m_ReplaceAccompanyPawnAsset of the equip rows; keyblades/manifest accompany_pawns)
const KBM = readJson(P('build', 'sora_pack', 'keyblades', 'manifest.json'));
for (const [form, kb] of [['SH', 'w_so030'], ['SH2', 'w_so140']]) {
  const pawns = new Map();
  for (const f of KBM.keyblades[kb].forms) for (const p of f.accompany_pawns || []) pawns.set(p.model, p);
  for (const p of pawns.values()) {
    const m = KBM.models[p.model];
    // the hit windows of its own packed clips (keyblades/<model>/model.json: TresAnimNotifyState_Attack)
    const mj = readJson(P('build', 'sora_pack', 'keyblades', p.model, 'model.json'));
    const wins = [], wrows = [];
    for (const c of mj.clips || []) for (const n of c.notifies || []) if (n.class === 'TresAnimNotifyState_Attack') { wins.push((c.logical_names || [c.sequence]).join('/') + ':' + r4(n.trigger_time) + '-' + r4(n.end_trigger_time)); wrows.push(n.properties.m_AttackDataIDName + '@' + n.properties.CollisionGrpName); }
    // (F-SHIELD) the pawns' numbers, traced: Pegasus's ctor 0x1413E8300 (m_Param_MaxTime 10; the blueprint w_so036 sets
    // none of them), its Move state 0x141409CE0 (speed (sin(phase x pi) x 700 + 900) x accel, phase += dt x 0.3, accel 0.1
    // -> 1 at dt x 0.75, the Hermite segments, the next point 700 .. 800 away (+300 up in 30 %), the enemy search radius
    // 500, the random turns), its thunder 0x141419A50 (cool-down 1.0 s; TresProjSHThunder: SPHERE 700, m_MainLife 0.5, row
    // FINISH_SH_THUNDER), its TresAtkColl0 (FINISH_SH_RUSH CAPSULE1 r 270, half height 350 at (0, -100, 150), auto-activated
    // with m_AttackInterval 1.0). The pan: TresAccompanyPawnBase's ctor 0x1413E6850 (m_Param_MaxTime 30), its FinishSH2.
    const PEGASUS = [10, 1.0, 700, 0.5, 270, 350, -100, 150, 1.0, 900, 700, 0.3, 0.75, 0.1, 700, 300, 500, 1.2, 100, 1000, 1.5, 0.2, 300, 200, 100, -400, 500, 200, 100, 400, 200, 0.3, 0.2, 0.2, 0.5, 0.3, 0.4];
    const PAN = [30, 100, 200, 0, 0];
    add({ id: 'pawn.' + form + '.' + p.model, what: 'pawn', form, logical: p.blueprint, clip: p.model, flags: Object.keys((m && m.clips) || {}), windows: wins, rows: [...wrows, ...((m && m.rows_not_sequences) || []).map((x) => x.name + ' (' + x.class + ', not packed)')], values: form === 'SH' ? PEGASUS : PAN,
      ref_addr: form === 'SH' ? '0x1413E8300 0x141409CE0 0x141419A50' : '0x1413E6850 0x1413E1210', ref_conf: 'confirmed', note: (form === 'SH' ? 'Pegasus (TresStylePawnSHPegasus; its Chariot TresStylePawnSHChariot is m_SHChariotAssets, the same model): values = [m_Param_MaxTime s, the thunder cool-down s, the thunder radius cm, its life s, the rush radius, half height, location y, z cm, the rush interval s, the speed base, amplitude cm/s, the phase rate, the accel rate, the accel start, the next point distance cm (+ 0 .. 100), the climb cm (30 %), the enemy search radius cm, the speed cap factor, the tangent length min, max cm, the tangent length factor, the start delay s, the ctor points P1 (x, y, z), P2, P3 and the tangents T1, T2 (normalised)]' : 'the Frying Pan (TresStylePawnSHFryingpan): values = [m_Param_MaxTime s (not counted down by the finish), its FinishSH2 sphere radius cm, relative location x, y, z cm]') + '; flags = its packed clips' });
  }
}
// the Frying Pan's row rename
for (const r of RENAME) add({ id: 'rename.SH2.' + r, what: 'rename', form: 'SH2', logical: r, clip: r.replace(/^SCSH_/, 'SH2_'), ref_addr: '0x141357590', note: 'in form 0x91 an attack window of this row hits with the other' });
// the collision shapes
for (const rel of SHAPE_OWNERS) for (const s of shapes(rel)) {
  add({ id: 'shape.' + s.owner + '.' + s.group, what: 'shape', form: /w_so14/.test(s.owner) ? 'SH2' : 'SH', logical: s.group, clip: s.owner, flags: [s.shape, s.socket1, s.socket2, s.attach], values: [...s.size, ...s.loc], rows: [s.row], ref_addr: '-', note: 'TresAtkColl0 CollisionShapesSrc of ' + rel + ': flags = [shape, socket 1, socket 2, attach type], values = [size x, y, z, location 1 x, y, z], rows = [the default attack row]' });
}
// THE SHAPE TRACKS: for every attack clip and every group its hit windows name, that group's shapes on the weapon parts of
// the grow level the clip is played at (the combo LV1, 0x55 LV2, 0x56 LV3) as one capsule per animation frame (time =
// frame x length / (frames - 1)): [ax, ay, az, bx, by, bz, radius], Sora's actor space (UE cm, his yaw 0). The weapon's
// sockets (ATK_01 ...) from its mesh (build/sora_pack/keyblades/<model>/sockets.json) on Sora's socket (R_Wep3_skt ...)
// under the clip, the weapon at its own clip of the same logical name (else its reference pose): the forward kinematics
// of tools/sora_pack.js attackSockets, as tools/sora_form_claws.js does it (copied, not shared: another package's file).
{
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
  const soraAnims = new Map();
  const soraAnim = (clip) => { if (!soraAnims.has(clip)) { const it = item(clip); let a = null; if (it && it.source_asset) { try { a = sp.loadAnimation(it.source_asset + '.' + path.basename(it.source_asset), SORA_BONES); } catch (e) { a = null; } } soraAnims.set(clip, a); } return soraAnims.get(clip); };
  const weapons = new Map();
  const weapon = (model) => {
    if (!weapons.has(model)) {
      const dir = P('build', 'sora_pack', 'keyblades', model);
      if (!fs.existsSync(path.join(dir, 'model.json'))) weapons.set(model, null);
      else { const mj = readJson(path.join(dir, 'model.json')), sj = readJson(path.join(dir, 'sockets.json')); weapons.set(model, { mj, sockets: sj.weapon_sockets || {}, bones: sp.buildSkeleton(sp.readGlb(P('assets_converted', 'meshes', sp.CONTENT, sp.gamePathToContent(mj.source_mesh) + '.glb')).json), anims: new Map() }); }
    }
    return weapons.get(model);
  };
  const weaponAnim = (w, logical, clip) => { const key = logical + '|' + clip; if (!w.anims.has(key)) { const c = (w.mj.clips || []).find((x) => (x.logical_names || []).includes(logical)) || (w.mj.clips || []).find((x) => x.sequence === clip); let a = null; if (c) { try { a = sp.loadAnimation(c.asset + '.' + c.sequence, w.bones); } catch (e) { a = null; } } w.anims.set(key, a); } return w.anims.get(key); };
  const socketAt = (part, socketName, logical, clip, time) => {
    const w = weapon(part.model), sa = soraAnim(clip), sock = SORA_SOCKETS[Object.keys(SORA_SOCKETS).find((n) => n.toLowerCase() === part.socket.toLowerCase())];
    if (!w || !sa || !sock) return null;
    const hand = SORA_BONES.findIndex((b) => b.name === sock.bone);
    const base = under(sock.T, boneT(SORA_BONES, byBone(sa), hand, Math.min(time, sa.length), sa.props.bEnableRootMotion === true));
    const a = weaponAnim(w, logical, clip);
    const ws = w.sockets[socketName];
    const k = w.bones.findIndex((b) => b.name === (ws ? ws.bone : socketName));
    if (k < 0) return null;
    let T = under(boneT(w.bones, byBone(a), k, a ? Math.min(time, a.length) : 0, false), base);
    if (ws) T = under({ q: rotQ({ pitch: ws.local_rotation[0], yaw: ws.local_rotation[1], roll: ws.local_rotation[2] }), t: ws.local_location, s: [1, 1, 1] }, T);
    return T;
  };
  const shapeAt = (s, logical, clip, time) => {
    if (s.attach === 'OWNER_ATTACHED_MESH') { const c = [s.loc[0], s.loc[1], 91 + s.loc[2]]; return s.shape === 'CAPSULE2' ? [0, 0, 91, ...c, s.size[0]] : [...c, ...c, s.size[0]]; }
    const T1 = socketAt(s.part, s.socket1, logical, clip, time);
    if (!T1) return null;
    const a = actor(point(T1, s.loc));
    if (s.shape !== 'CAPSULE2' || s.socket2 === 'None') return s.shape === 'CAPSULE1' ? [a[0], a[1], a[2] - s.size[2], a[0], a[1], a[2] + s.size[2], s.size[0]] : [...a, ...a, s.size[0]];
    const T2 = socketAt(s.part, s.socket2, logical, clip, time);
    return T2 ? [...a, ...actor(point(T2, [0, 0, 0])), s.size[0]] : [...a, ...a, s.size[0]];
  };
  // (F-SHAPES) a CONVEX's real transform (combat/hull.rs `VolTrack` HULL; Update 0x140647950 case 6 / 8): T1 the socket; location =
  // T1.t + (m_bAbsoluteOffset1 ? relLoc x T1.s : T1.q (relLoc x T1.s)); rotation = LOCAL ? relRot (world) : T1.q x relRot; scale =
  // FUN_140649BF0(t) x T1.s. `at` = the clip time of the place (the window's start for m_bDisableLocation1Attach), `t` = the time
  // since the window opened. Any other shape of such a group: its capsule + [0, 0, 0].
  const MESH_YAW_Q = [0, 0, Math.sin(yaw / 2), Math.cos(yaw / 2)];
  const growScale = (s, t) => { if (s.inc_scale.every((x) => x === 0)) return s.scale; return s.scale.map((b, i) => { const v = Math.max(b + s.inc_scale[i] * t, 0.01); return s.max_scale[i] > 0 ? Math.min(v, s.max_scale[i]) : v; }); };
  const volAt = (s, logical, clip, at, t) => {
    if (s.shape !== 'CONVEX') { const v = shapeAt(s, logical, clip, at); return v ? [...v.map((x) => Math.round(x * 10) / 10), 0, 0, 0] : null; }
    const T1 = socketAt(s.part, s.socket1, logical, clip, at);
    if (!T1) return null;
    const sl = [s.loc[0] * T1.s[0], s.loc[1] * T1.s[1], s.loc[2] * T1.s[2]];
    const off = s.abs1 ? sl : qrot(T1.q, sl);
    const c = actor([T1.t[0] + off[0], T1.t[1] + off[1], T1.t[2] + off[2]]);
    const q = s.rot_attach === 'TRES_ACRA_LOCAL' ? rotQ(s.rot) : qmulH(MESH_YAW_Q, qmulH(T1.q, rotQ(s.rot)));
    const n = Math.hypot(...q);
    const g = growScale(s, t);
    const sc = s.ignore_scale ? g : g.map((x, i) => x * T1.s[i]);
    return [...c.map((x) => Math.round(x * 10) / 10), ...q.map((x) => Math.round(x / n * 1e6) / 1e6), ...sc.map(r4)];
  };
  const volFlags = (s) => [s.shape === 'CONVEX' ? 'HULL' : 'CAPSULE', s.shape === 'CONVEX' ? s.mesh : '-', s.shape === 'CONVEX' && s.rot_attach === 'TRES_ACRA_LOCAL' ? 'world' : 'actor', s.owner + ':' + s.shape + ':' + s.socket1 + '-' + s.socket2 + '@' + s.part.socket + (s.frozen ? ':frozen' : '') + ':' + s.rot_attach];
  const merge = (list) => { const pts = list.flatMap((v) => [v.slice(0, 3), v.slice(3, 6)]); let best = [pts[0], pts[0]], d = -1; for (let i = 0; i < pts.length; i++) for (let j = i; j < pts.length; j++) { const e = Math.hypot(pts[i][0] - pts[j][0], pts[i][1] - pts[j][1], pts[i][2] - pts[j][2]); if (e > d) { d = e; best = [pts[i], pts[j]]; } } return [...best[0], ...best[1], Math.max(...list.map((v) => v[6]))]; };
  const partsAt = (form, level) => (ROWS.find((r) => r.id === 'grow.' + form + '.' + level) || { rows: [] }).rows.map((u) => { const m = u.match(/^(\w+)@(\w+):(\w+)=(.*)$/); return { socket: m[3] === 'None' ? 'R_wep_skt' : m[3], model: m[4].split('/').pop().replace(/_[Ww]ep$/, '') }; });
  const r1 = (x) => Math.round(x * 10) / 10;
  let tracks = 0;
  for (const form of ['SH', 'SH2']) {
    for (const r of ROWS.filter((x) => x.form === form && (x.what === 'attack' || x.what === 'chain'))) {
      const level = r.code === 0x55 ? 2 : r.code === 0x56 ? 3 : 1;
      const it = item(r.clip);
      if (!it || ROWS.some((x) => x.id.startsWith('track.' + form + '.' + r.clip + '.'))) continue;
      const shapesOfLevel = partsAt(form, level).flatMap((part) => SHAPE_OWNERS.filter((o) => o.split('/').pop().replace(/_[Ww]ep$/, '') === part.model).flatMap((o) => shapes(o)).map((s) => ({ ...s, part })));
      const groups = [...new Set((it.notifies || []).filter((n) => n.class === 'TresAnimNotifyState_Attack').map((n) => String(prop(n, 'CollisionGrpName') ?? 'None')))];
      for (const g of groups) {
        const list = shapesOfLevel.filter((s) => s.group === g);
        if (!list.length) continue;
        // (F-SHAPES) a group with a CONVEX: one `vol` row per shape (combat/hull.rs) instead of one merged sphere / capsule of m_Size
        if (list.some((s) => s.shape === 'CONVEX')) {
          const mine = (it.notifies || []).filter((n) => n.class === 'TresAnimNotifyState_Attack' && String(prop(n, 'CollisionGrpName') ?? 'None') === g).map((n) => ({ a: +n.time, b: +n.end_time }));
          for (const [k, s] of list.entries()) {
            const vals = [];
            for (let f = 0; f < it.num_frames; f++) {
              const t = it.num_frames > 1 ? f * it.duration_s / (it.num_frames - 1) : 0;
              const w = mine.filter((x) => x.a <= t + 1e-6).pop() || mine[0];
              const v = volAt(s, r.logical, r.clip, s.frozen ? w.a : t, Math.max(t - w.a, 0));
              must(v, form + ' ' + r.clip + ' ' + g + ': a socket could not be placed');
              vals.push(...v);
            }
            add({ id: 'vol.' + form + '.' + r.clip + '.' + g + '.' + k, what: 'vol', form, code: it.num_frames, logical: g, clip: r.clip, length: r4(it.duration_s), flags: volFlags(s), windows: mine.map((w) => r4(w.a) + '-' + r4(w.b)), values: vals, ref_addr: '0x140647950',
              note: 'grow LV' + level + ': shape ' + k + ' of group ' + g + ' per frame (combat/hull.rs VolTrack): flags = [HULL | CAPSULE, the CONVEX mesh, world | actor (the frame of the rotation: m_RotAttachType LOCAL = relRot in the world), owner:shape:sockets@Sora\'s socket]; a HULL [x, y, z, qx, qy, qz, qw, sx, sy, sz] = Update 0x140647950 case 6\'s location / rotation / scale (FUN_140649BF0 growth from the window\'s start) in Sora\'s actor space (his yaw 0), a CAPSULE [ax, ay, az, bx, by, bz, r, 0, 0, 0]' });
          }
          tracks += 1;
          continue;
        }
        const values = [];
        let ok = true;
        for (let f = 0; f < it.num_frames && ok; f++) {
          const t = it.num_frames > 1 ? f * it.duration_s / (it.num_frames - 1) : 0;
          const each = list.map((s) => shapeAt(s, r.logical, r.clip, t));
          if (each.some((v) => !v)) ok = false;
          else values.push(...merge(each).map(r1));
        }
        must(ok, form + ' ' + r.clip + ' ' + g + ': a socket could not be placed');
        add({ id: 'track.' + form + '.' + r.clip + '.' + g, what: 'track', form, code: it.num_frames, logical: g, clip: r.clip, length: r4(it.duration_s), flags: list.map((s) => s.owner + ':' + s.shape + ':' + s.socket1 + '-' + s.socket2 + '@' + s.part.socket), values, ref_addr: '-',
          note: 'grow LV' + level + ': per frame [ax, ay, az, bx, by, bz, radius] (cm, actor space, his yaw 0) of group ' + g + ' on the weapon parts of that level (flags: owner:shape:sockets@Sora\'s socket); merged into one capsule when several' });
        tracks += 1;
      }
    }
  }
  must(tracks >= 16, 'expected the shield clips\' tracks, got ' + tracks);
  // the Frying Pan pawn's FinishSH2 (SPHERE 100 at its ATK_01 + (200, 0, 0)) under its own M_J423 (the pawn stands at Sora
  // with his rotation: its actor space is his)
  {
    const w = weapon('w_so145');
    const c = w && (w.mj.clips || []).find((x) => (x.logical_names || []).includes('M_J423'));
    must(c, 'w_so145 has no M_J423');
    const a = weaponAnim(w, 'M_J423', c.sequence);
    const ws = w.sockets.ATK_01;
    const k = w.bones.findIndex((b) => b.name === (ws ? ws.bone : 'ATK_01'));
    must(k >= 0, 'w_so145: no ATK_01');
    const values = [];
    for (let f = 0; f < c.frames; f++) {
      const t = c.frames > 1 ? f * c.duration_s / (c.frames - 1) : 0;
      let T = boneT(w.bones, byBone(a), k, Math.min(t, a ? a.length : 0), false);
      if (ws) T = under({ q: rotQ({ pitch: ws.local_rotation[0], yaw: ws.local_rotation[1], roll: ws.local_rotation[2] }), t: ws.local_location, s: [1, 1, 1] }, T);
      const pt = actor(point(T, [200, 0, 0]));
      values.push(...pt.map(r1), ...pt.map(r1), 100);
    }
    add({ id: 'track.SH2.pan.FinishSH2', what: 'track', form: 'SH2', code: c.frames, logical: 'FinishSH2', clip: 'pan', length: r4(c.duration_s), flags: ['w_so145:SPHERE:ATK_01'], values, ref_addr: '-', note: 'the Frying Pan pawn (StylePawn/w_so145 TresAtkColl0 FinishSH2) under its own M_J423 (' + c.sequence + '): per frame [x, y, z, x, y, z, radius] in Sora\'s actor space at the spawn' });
  }
  // (AUDIT-A SHIELD) THE PAWNS' OWN CLIP NOTIFIES. KH3's Pegasus (StylePawn/w_so036), its Chariot (StylePawn/w_so035, Pegasus's
  // m_SHChariotAssets: its own actor and model) and the Frying Pan (StylePawn/w_so145) play M_B424 / M_B425 / M_J423 on their own
  // anim instances in the same tick as Sora (StyleFinishSH Tick 0x1413D5E50 sub 1 / 3, slot 167 0x1413E1210 sub 0): their
  // AttachEffect / PlaySound / AttachSound notifies fire on THEIR meshes. One `pnotify` row per notify entry: flags = [effect |
  // sound, the socket, abs | rel (bAbsoluteRotation), the anchor name `pawn:<row id>` the hosts resolve (kh-core
  // shield::pawn_socket)], effects / sounds = the asset, values = [trigger time, end time (-1 = instant), fade out s, then per
  // frame (time = f x length / (frames - 1)) the socket + LocationOffset in Sora's actor space (the pawn stands at his root, his
  // yaw: kh-er-render keyblade::form_pawn) and the rotator (pitch, yaw, roll) of socket x RotationOffset (abs: RotationOffset
  // alone, world)]. A sound's track is the pawn's root bone. M_B425 is a 2D blend space (the samples B425 .. B425UR): every
  // sample's own notifies (the core fires the heaviest sample's, as Sora's).
  {
    const quatRot = (q) => { const [X, Y, Z, W] = q, st = Z * X - W * Y, d = 180 / Math.PI; return [Math.asin(Math.max(-1, Math.min(1, 2 * st))) * d, Math.atan2(2 * (W * Z + X * Y), 1 - 2 * (Y * Y + Z * Z)) * d, Math.atan2(-2 * (W * X + Y * Z), 1 - 2 * (X * X + Y * Y)) * d]; };
    const walkAll = (v, f) => { if (Array.isArray(v)) v.forEach((x) => walkAll(x, f)); else if (v && typeof v === 'object') { f(v); Object.values(v).forEach((x) => walkAll(x, f)); } };
    const B425 = ['B425', 'B425DL', 'B425DR', 'B425UL', 'B425UR'];
    let pn = 0;
    // (AUDIT-A SHIELD) THE RIDE'S LINKS (StyleFinishSH Tick 0x1413D5E50 sub 2): the Chariot attached to Pegasus at Pegasus's
    // X_Link_skt (Chariot vt+0x23A8(Pegasus, "X_Link_skt", 1)), Sora attached to the Chariot at ITS X_link_skt (vt+0x23A8,
    // the FName "X_Link_skt" built before it; UE names are case-insensitive). Per frame of each clip the three play together
    // (M_B424, M_B425's samples): where the Chariot's actor and Sora's actor stand in PEGASUS's actor frame (Pegasus's
    // X_Link_skt under its clip x the Chariot's X_link_skt under ITS clip; the meshes stand at their actors with the
    // characters' mesh yaw). values = per frame [chariot x, y, z, chariot yaw, sora x, y, z, sora yaw].
    const MESH_T = { q: MESH_YAW_Q, t: [0, 0, 0], s: [1, 1, 1] };
    // Pegasus's mesh stands 100 cm ahead of its actor (StylePawn/w_so036 TresCharMesh0 RelativeLocation (100, 0, 0), yaw 270);
    // the Chariot's and Sora's meshes at their actors (w_so035 / p_ex001: no RelativeLocation)
    const PEG_MESH_T = { q: MESH_YAW_Q, t: [100, 0, 0], s: [1, 1, 1] };
    const links = new Map();
    {
      const peg = weapon('w_so036'), car = weapon('w_so035');
      const sockT = (w, name) => { const ws = w.sockets[Object.keys(w.sockets).find((s) => s.toLowerCase() === name.toLowerCase())]; return { ws, k: w.bones.findIndex((b) => b.name === ws.bone) }; };
      const ps = sockT(peg, 'X_Link_skt'), cs = sockT(car, 'X_Link_skt');
      for (const seq of ['B424', ...B425]) {
        const pa = sp.loadAnimation('/Game/Character/wep/w_so036/anm/rt/' + seq + '.' + seq, peg.bones), ca = sp.loadAnimation('/Game/Character/wep/w_so035/anm/rt/' + seq + '.' + seq, car.bones);
        const frames = pa.numFrames, len = pa.length, vals = [];
        for (let f = 0; f < frames; f++) {
          const t = frames > 1 ? f * len / (frames - 1) : 0;
          const sock = (w, a, s, tt) => under({ q: rotQ({ pitch: s.ws.local_rotation[0], yaw: s.ws.local_rotation[1], roll: s.ws.local_rotation[2] }), t: s.ws.local_location, s: [1, 1, 1] }, boneT(w.bones, byBone(a), s.k, Math.min(tt, a.length), false));
          const C = under(sock(peg, pa, ps, t), PEG_MESH_T); // the Chariot's actor in Pegasus's actor frame
          const S = under(sock(car, ca, cs, t), under(MESH_T, C)); // Sora's actor
          const yawOf = (q) => quatRot(q)[1];
          vals.push(...C.t.map(r1), r1(yawOf(C.q)), ...S.t.map(r1), r1(yawOf(S.q)));
        }
        links.set(seq, { frames, len, vals });
        add({ id: 'plink.SH.' + seq, what: 'plink', form: 'SH', code: frames, logical: 'X_Link_skt', clip: seq, length: r4(len), values: vals, ref_addr: '0x1413D5E50', ref_conf: 'confirmed', note: 'the ride\'s links per frame of ' + seq + ': [chariot x, y, z, yaw, sora x, y, z, yaw] in Pegasus\'s actor frame (cm, degrees)' });
        console.log('plink ' + seq + ' frame 0: chariot (' + vals.slice(0, 4).join(', ') + ') sora (' + vals.slice(4, 8).join(', ') + ')');
      }
    }
    for (const [form, model, seqs] of [['SH', 'w_so036', ['B424', ...B425]], ['SH', 'w_so035', ['B424', ...B425]], ['SH2', 'w_so145', ['J423']]]) {
      const w = weapon(model);
      must(w, model + ': not packed');
      for (const seq of seqs) {
        const gp = '/Game/Character/wep/' + model + '/anm/rt/' + seq;
        const a = sp.loadAnimation(gp + '.' + seq, w.bones);
        const chans = byBone(a);
        const frames = a.numFrames, len = a.length;
        const at = (f) => (frames > 1 ? f * len / (frames - 1) : 0);
        a.notifies.forEach((n, k) => {
          const id = 'pnotify.' + form + '.' + model + '.' + seq + '.' + k;
          const base = { id, what: 'pnotify', form, code: frames, logical: model, clip: seq, length: r4(len), ref_addr: '0x1413D5E50', ref_conf: 'confirmed' };
          if (/AttachSound|PlaySound/.test(n.class)) {
            const sound = n.properties.Sound;
            if (!sound) return;
            const vals = [r4(n.trigger_time), n.kind === 'state' ? r4(n.end_trigger_time) : -1, r4(+(n.properties.FadeOutDuration ?? 0))];
            for (let f = 0; f < frames; f++) { const T = boneT(w.bones, chans, 0, Math.min(at(f), len), false); vals.push(...actor(T.t).map(r1), 0, 0, 0); }
            add({ ...base, flags: ['sound', 'None', 'rel', 'pawn:' + id], sounds: [sound], values: vals, note: model + ' ' + seq + ' ' + n.class + ' (' + n.object + '): played at the pawn (its root bone)' });
            pn += 1;
            return;
          }
          if (!/AttachEffect/.test(n.class)) return;
          let effect = null, socket = 'None', absRot = false, absLoc = false, off = { x: 0, y: 0, z: 0 }, rot = { pitch: 0, yaw: 0, roll: 0 }, group = 0;
          walkAll(n.properties, (o) => {
            if (o.name === 'AttachEffect' && o.value) effect = o.value;
            if (o.name === 'SocketName' && o.value && o.value !== 'None') socket = o.value;
            if (o.name === 'bAbsoluteRotation') absRot = o.value === true;
            if (o.name === 'bAbsoluteLocation') absLoc = o.value === true;
            if (o.name === 'LocationOffset' && o.value) off = o.value;
            if (o.name === 'RotationOffset' && o.value) rot = o.value;
            if (o.name === 'GroupID') group = o.value;
          });
          if (!effect) return; // an entry without an asset (no common effect: bUseCommonEffect false) spawns nothing
          const ws = w.sockets[socket];
          const bone = socket === 'None' ? 0 : w.bones.findIndex((b) => b.name === (ws ? ws.bone : socket));
          must(bone >= 0, id + ': socket ' + socket + ' is on no bone of ' + model);
          const vals = [r4(n.trigger_time), n.kind === 'state' ? r4(n.end_trigger_time) : -1, 0];
          for (let f = 0; f < frames; f++) {
            // socket None: the owner itself (its actor: his root and yaw), the offsets in the actor's frame
            if (socket === 'None') { vals.push(off.x, off.y, off.z, rot.pitch, rot.yaw, rot.roll); continue; }
            let T = boneT(w.bones, chans, bone, Math.min(at(f), len), false);
            if (ws) T = under({ q: rotQ({ pitch: ws.local_rotation[0], yaw: ws.local_rotation[1], roll: ws.local_rotation[2] }), t: ws.local_location, s: [1, 1, 1] }, T);
            const o = [off.x, off.y, off.z];
            const p = absLoc ? [T.t[0] + o[0], T.t[1] + o[1], T.t[2] + o[2]] : point(T, o);
            const r = absRot ? [rot.pitch, rot.yaw, rot.roll] : quatRot(qmulH(MESH_YAW_Q, qmulH(T.q, rotQ(rot))));
            vals.push(...actor(p).map(r1), ...r.map(r1));
          }
          add({ ...base, group, flags: ['effect', socket, absRot ? 'abs' : 'rel', 'pawn:' + id], effects: [effect], values: vals, note: model + ' ' + seq + ' ' + n.class + ' (' + n.object + ') group ' + group + ' at ' + socket + ' offset (' + [off.x, off.y, off.z].join(', ') + ') rotation (' + [rot.pitch, rot.yaw, rot.roll].join(', ') + ')' + (absRot ? ' absolute' : '') });
          pn += 1;
        });
      }
    }
    must(pn >= 25, 'expected the pawns\' clip notifies, got ' + pn);
    // (AUDIT-A SHIELD) THE FORM WEAPONS' OWN CLIP NOTIFIES, the same way: each weapon actor of the form (the transform unit
    // w_so031 / w_so141 of StyleChange11 / 19 and of the grow's GuardChange / GuardChange2, the units of the grow level a clip
    // is played at) plays its row of Sora's logical name in step with him; its notifies fire on ITS bones. Rows keyed on
    // Sora's sequence (`clip`); the track is the weapon's socket under both clips (socketAt: Sora's socket under his clip x
    // the weapon's bone under its own), per frame of HIS clip. The weapons' plays take Sora's effect group (Frying Pan's
    // J419 / J416 rows carry 2151 / 2152). The THROWN shield (M_B413's TresAnimNotify_ThrowWeapon at 0.3005, m_WeaponAnimName
    // M_B414): its M_B414 notifies fire at the throw + their time, from where the shield left his hand (one frame: the
    // socket at the throw), flag `thrown`: its states last while M_B413 / M_B414 play (the recall is M_B415's equipment
    // vt+0x560(7) -> each part's vt+0x10D0: weapon+0x6F0 == 5 (thrown) -> its state machine back to 0).
    const UNIT = { SH: 'w_so031', SH2: 'w_so141' };
    const unitNames = { SH: ['StyleChange11', 'GuardChange', 'GuardChange2', 'AirGuardChange', 'AirGuardChange2'], SH2: ['StyleChange19', 'GuardChange', 'GuardChange2', 'AirGuardChange', 'AirGuardChange2'] };
    const levelNames = { 1: ['M_B411', 'M_B419', 'M_B412', 'M_B413', 'M_B415', 'M_B416', 'M_B417'], 2: ['M_B418'], 3: ['M_B426'] };
    let wn = 0;
    const emit = (form, part, logical, soraClip, wclip, n, k, trackAt, frames, len, thrown) => {
      const w = weapon(part.model);
      const id = 'pnotify.' + form + '.' + part.model + '.' + wclip.sequence + '.' + k + (thrown ? '.thrown' : '') + '@' + soraClip;
      if (ROWS.some((r) => r.id === id)) return; // (M_B411 and M_B419 are the same rows)
      const base = { id, what: 'pnotify', form, code: frames, logical: part.model, clip: soraClip, length: r4(len), ref_addr: thrown ? '0x141366990' : '0x1405BD620', ref_conf: 'confirmed' };
      const extra = thrown ? ['thrown'] : [];
      const t0 = thrown ? thrown.t0 : 0;
      if (thrown) THROWN_CLOCK.set(id, [r4(n.trigger_time), n.kind === 'state' ? r4(n.end_trigger_time) : -1]); // (AUDIT-A SHIELD) the weapon clip's own times (wnotify rows)
      if (/AttachSound|PlaySound/.test(n.class)) {
        const sound = n.properties.Sound;
        if (!sound) return;
        const vals = [r4(t0 + n.trigger_time), n.kind === 'state' ? r4(t0 + n.end_trigger_time) : -1, r4(+(n.properties.FadeOutDuration ?? 0))];
        for (let f = 0; f < frames; f++) { const T = trackAt(w.bones[0].name, f); must(T, id + ': no place'); vals.push(...actor(T.t).map(r1), 0, 0, 0); }
        add({ ...base, flags: ['sound', 'None', 'rel', 'pawn:' + id, ...extra], sounds: [sound], values: vals, note: part.model + ' ' + wclip.sequence + ' (' + logical + ') ' + n.class + ': at the weapon (its root), Sora on ' + soraClip });
        wn += 1;
        return;
      }
      if (!/AttachEffect/.test(n.class)) return;
      let effect = null, socket = 'None', absRot = false, absLoc = false, off = { x: 0, y: 0, z: 0 }, rot = { pitch: 0, yaw: 0, roll: 0 }, group = 0;
      let endSocket = 'None', toAttach = true, endType = '', fadeOut = 0; // (AUDIT-B W-VFX2) for a thunder row's kb: anchor
      walkAll(n.properties, (o) => {
        if (o.name === 'AttachEffect' && o.value) effect = o.value;
        if (o.name === 'SocketName' && o.value && o.value !== 'None') socket = o.value;
        if (o.name === 'bAbsoluteRotation') absRot = o.value === true;
        if (o.name === 'bAbsoluteLocation') absLoc = o.value === true;
        if (o.name === 'LocationOffset' && o.value) off = o.value;
        if (o.name === 'RotationOffset' && o.value) rot = o.value;
        if (o.name === 'GroupID') group = o.value;
        if (o.name === 'EndSocketName' && o.value && o.value !== 'None') endSocket = o.value; // (AUDIT-B W-VFX2)
        if (o.name === 'ToAttach') toAttach = o.value !== false; // (AUDIT-B W-VFX2)
        if (o.name === 'AttachEffectEndType' && o.value) endType = String(o.value).replace('ESQEX_ATTACH_EFFECT_END_TYPE_', ''); // (AUDIT-B W-VFX2)
        if (o.name === 'FadeOut' && typeof o.value === 'number') fadeOut = o.value; // (AUDIT-B W-VFX2)
      });
      if (!effect) return;
      // UE names are case-insensitive ("Center" names the bone `center`)
      const lc = socket.toLowerCase();
      const real = Object.keys(w.sockets).find((s) => s.toLowerCase() === lc) || (w.bones.find((b) => b.name.toLowerCase() === lc) || {}).name || socket;
      // (AUDIT-B W-VFX2) a thunder effect on the weapon (not thrown: the thrown weapon flies on its own clip): its beam's two
      // ends are the weapon's sockets, so the row spawns on the kb: anchor (the weapon's socket under its clip on his mount)
      if (!thrown) {
        const realOf = (name) => { const l = name.toLowerCase(); return Object.keys(w.sockets).find((x) => x.toLowerCase() === l) || (w.bones.find((b) => b.name.toLowerCase() === l) || {}).name || name; };
        // (the offsets go to the wnotify row's values [2..7], the shared pass's placement on the anchor: they move the component,
        // never the beam's two ends, which are the sockets)
        const d = { EndSocketName: endSocket === 'None' ? 'None' : realOf(endSocket), bAbsoluteRotation: absRot, ToAttach: toAttach, FadeOut: fadeOut };
        const kb = WNOTIFY_LIB.thunderAnchor(P('build', 'sora_pack'), String(effect), part.model, part.socket, socket === 'None' ? 'None' : real, d, endType);
        if (kb) {
          must(!absLoc, id + ': a thunder row with bAbsoluteLocation (the kb: anchor has none)');
          THUNDER_KB.set(id, { kb, place: [off.x || 0, off.y || 0, off.z || 0, rot.pitch || 0, rot.yaw || 0, rot.roll || 0].map(r4) });
        }
      }
      const vals = [r4(t0 + n.trigger_time), n.kind === 'state' ? r4(t0 + n.end_trigger_time) : -1, 0];
      for (let f = 0; f < frames; f++) {
        const T = trackAt(socket === 'None' ? w.bones[0].name : real, f);
        must(T, id + ': socket ' + socket + ' could not be placed');
        const o = [off.x, off.y, off.z];
        const p = absLoc ? [T.t[0] + o[0], T.t[1] + o[1], T.t[2] + o[2]] : point(T, o);
        const r = absRot ? [rot.pitch, rot.yaw, rot.roll] : quatRot(qmulH(MESH_YAW_Q, qmulH(T.q, rotQ(rot))));
        vals.push(...actor(p).map(r1), ...r.map(r1));
      }
      add({ ...base, group, flags: ['effect', socket, absRot ? 'abs' : 'rel', 'pawn:' + id, ...extra], effects: [effect], values: vals, note: part.model + ' ' + wclip.sequence + ' (' + logical + ') ' + n.class + ' group ' + group + ' at ' + socket + ' offset (' + [off.x, off.y, off.z].join(', ') + ') rotation (' + [rot.pitch, rot.yaw, rot.roll].join(', ') + ')' + (absRot ? ' absolute' : '') + ', Sora on ' + soraClip + (thrown ? ', thrown at ' + r4(t0) : '') });
      wn += 1;
    };
    for (const form of ['SH', 'SH2']) {
      const jobs = [
        ...unitNames[form].map((l) => ({ part: { socket: 'R_wep_skt', model: UNIT[form] }, logical: l })),
        ...[1, 2, 3].flatMap((lv) => partsAt(form, lv).flatMap((part) => levelNames[lv].map((l) => ({ part, logical: l })))),
      ];
      for (const { part, logical } of jobs) {
        const soraClip = clipOf(form, logical);
        const it = soraClip && soraClip !== '-' ? item(soraClip) : null;
        const w = weapon(part.model);
        if (!it || !w) continue;
        const wclip = (w.mj.clips || []).find((x) => (x.logical_names || []).includes(logical));
        if (!wclip || !(wclip.notifies || []).some((n) => /Effect|Sound/.test(n.class))) continue;
        const frames = it.num_frames, len = it.duration_s, at = (f) => (frames > 1 ? f * len / (frames - 1) : 0);
        wclip.notifies.forEach((n, k) => emit(form, part, logical, soraClip, wclip, n, k, (bone, f) => socketAt(part, bone, logical, soraClip, at(f)), frames, len, null));
      }
      // the thrown shield / pan (M_B413's ThrowWeapon)
      const raid = clipOf(form, 'M_B413');
      const it = item(raid);
      for (const tn of (it ? it.notifies || [] : []).filter((n) => n.class === 'TresAnimNotify_ThrowWeapon')) {
        const name = String(prop(tn, 'm_WeaponAnimName') ?? '').replace(/"/g, '');
        const t0 = +tn.time;
        for (const part of partsAt(form, 1)) {
          const w = weapon(part.model);
          const wclip = w && (w.mj.clips || []).find((x) => (x.logical_names || []).includes(name));
          if (!wclip) continue;
          const sa = soraAnim(raid), sock = SORA_SOCKETS[Object.keys(SORA_SOCKETS).find((s) => s.toLowerCase() === part.socket.toLowerCase())];
          must(sa && sock, form + ': the throw\'s socket');
          const hand = under(sock.T, boneT(SORA_BONES, byBone(sa), SORA_BONES.findIndex((b) => b.name === sock.bone), Math.min(t0, sa.length), sa.props.bEnableRootMotion === true));
          const wa = weaponAnim(w, name, raid);
          const trackAt = (bone) => { const ws = w.sockets[bone]; const k = w.bones.findIndex((b) => b.name === (ws ? ws.bone : bone)); if (k < 0) return null; let T = under(boneT(w.bones, byBone(wa), k, 0, false), hand); if (ws) T = under({ q: rotQ({ pitch: ws.local_rotation[0], yaw: ws.local_rotation[1], roll: ws.local_rotation[2] }), t: ws.local_location, s: [1, 1, 1] }, T); return T; };
          wclip.notifies.forEach((n, k) => emit(form, part, name, raid, wclip, n, k, (bone) => trackAt(bone), 1, it.duration_s, { t0 }));
        }
      }
    }
    must(wn >= 40, 'expected the form weapons\' clip notifies, got ' + wn);
  }
}
// the attack rows the form hits with (p_ex001_AttackData), for the combat sheet's owner
const usedRows = new Set(ROWS.flatMap((r) => (['attack', 'chain'].includes(r.what) ? r.rows : r.what === 'rename' ? [r.clip] : r.what === 'shape' ? r.rows : [])).filter((x) => x !== 'None'));
for (const extra of ['SCSH_CHANGE', 'SH2_CHANGE', 'FINISH_SH_RUSH', 'FINISH_SH_THUNDER', 'SH2_FRYINGPAN']) usedRows.add(extra);
for (const id of [...usedRows].sort()) {
  const a = atkRow(id);
  must(a, 'p_ex001_AttackData has no row ' + id);
  add({ id: 'row.' + id, what: 'row', form: /^SH2_/.test(id) ? 'SH2' : 'SH', logical: id, values: [a.m_Power, a.m_FormPoint, a.m_ReactionPowerValue], flags: [String(a.m_DamageKind).split('::').pop(), String(a.m_DamageAttribute).split('::').pop()], ref_addr: '-', note: 'p_ex001_AttackData row ' + a.id + ': values = [m_Power, m_FormPoint, m_ReactionPowerValue]; the combat sheet sora-combat-attack-data must carry it for the hit to deal it (STATE.md INT line)' });
}
// the shield guard (Guard slot 118 0x14139D2D0): its numbers
add({ id: 'guard.params', what: 'guard', form: 'SH', logical: 'TresPlayerState_Guard slot 118', values: [200, 0.2, 0.2, 6.0, 50, 1.0, 2000], ref_addr: '0x14139D2D0', note: 'values = [MyGuardMoveSpeed cm/s (ATresPlayerPawnBase ctor 0x141423210: 200; the pawn blueprint does not set it), the walk stop delay s (+0x108 = 0.2), the turn rate toward the locked target per 1/60 s (+0xF4 = 0.2), the turn rate toward the stick (dt x 6), the height above the ground that starts the air guard descent cm (50), the height that lands it (1), the ground probe range cm (2000)]; the hold time is MyGuardHoldTime (sora-combat-params guard.hold_time 3), the give-up height MyDiveFallStartHeight (sora-actions divefall.start_height)' });
// the transformation clip's voice group (StyleChange Begin 0x1413CBA70 case 0x89)
add({ id: 'voice.SH.StyleChange11', what: 'voice', form: 'SH', logical: 'StyleChange11', clip: 'B401', values: [2], ref_addr: '0x1413CBC5A', note: 'StyleChange +0xA4 = 2 for 0x89 only: the voice group number set after the transformation clip starts (anim interface vt+0x1F0 = SetVoiceGroupNo: the "start section" P3 left open)' });

// (AUDIT-A SHIELD) THE ONE SHARED WEAPON-CLIP PASS (AUDIT-B's seam, kh-core forms/ship.rs `wnotify_pass_dt`): one `wnotify`
// row per `pnotify` row above, in F-SHIP's columns, appended so that no older row moves. code 0 = effect, 1 = sound; flags =
// [the anchor `pawn:<pnotify row>` (its socket's track stays in the pnotify row: kh-core shield::pawn_socket), the model,
// state | instant, g<the effect group>, then `all` (a sample of the pawns' M_B425: no NotifyTriggerMode in w_so036 /
// w_so035's assets = the engine default AllAnimations, every sample in the blend above 1e-5 fires: UBlendSpaceBase::
// TickAssetPlayer 0x143F79E00) or `thrown` (the thrown weapon's own M_B414 / J414 notifies on ITS clip clock from the throw:
// placement::ThrownWeapon)]; values = [trigger, end (-1 instant), 0, 0, 0, 0, 0, 0 (the anchor carries offset and rotation), fade out].
{
  const B425S = new Set(['B425', 'B425DL', 'B425DR', 'B425UL', 'B425UR']);
  for (const p of ROWS.filter((r) => r.what === 'pnotify')) {
    const effect = p.flags[0] === 'effect';
    const thrown = p.flags.includes('thrown');
    const all = !thrown && ['w_so036', 'w_so035'].includes(p.logical) && B425S.has(p.clip);
    const [t, end] = thrown ? THROWN_CLOCK.get(p.id) : [p.values[0], p.values[1]];
    must(t !== undefined, p.id + ': no clock');
    add({ id: 'wnotify.' + p.id.slice('pnotify.'.length), what: 'wnotify', form: p.form, code: effect ? 0 : 1, logical: p.logical, clip: p.clip, length: p.length, group: p.group, flags: [THUNDER_KB.has(p.id) ? THUNDER_KB.get(p.id).kb : p.flags[3] /* (AUDIT-B W-VFX2) a thunder row: the kb: anchor */, p.logical, end >= 0 ? 'state' : 'instant', 'g' + p.group, ...(all ? ['all'] : []), ...(thrown ? ['thrown'] : [])], effects: p.effects, sounds: p.sounds, values: [t, end, ...(THUNDER_KB.has(p.id) ? THUNDER_KB.get(p.id).place : [0, 0, 0, 0, 0, 0]) /* (AUDIT-B W-VFX2) */, p.values[2]], ref_addr: p.ref_addr, ref_conf: 'confirmed', note: 'the shared pass\'s row of ' + p.id + (all ? ' (AllAnimations: fires while its sample is in the blend)' : '') + (thrown ? ' (on the thrown weapon\'s own clip clock)' : '') });
  }
  must(ROWS.filter((r) => r.what === 'wnotify').length === ROWS.filter((r) => r.what === 'pnotify').length, 'one wnotify row per pnotify row');
}

// ------------------------------------------------------------------------------------------------ checks
must(ROWS.filter((r) => r.what === 'attack').length === 16, 'expected 8 attack kinds per form');
for (const r of ROWS.filter((x) => x.what === 'attack')) must(r.clip !== '-' && r.length > 0, r.id + ': no clip');
must(ROWS.some((r) => r.id === 'anim.SH2.M_J423' && r.clip === 'J423'), 'Frying Pan has M_J423');

// ------------------------------------------------------------------------------------------------ sheet
const S = (n, t, g, doc) => [n, t, g, doc];
const COLS = [
  S('id', 'str', 'str', '<what>.<form>.<key>'), S('what', 'enum:attack|chain|anim|grow|finish|pawn|rename|shape|track|vol|row|voice|guard|pnotify|plink|wnotify' /* (AUDIT-A SHIELD) pnotify, plink, wnotify */, 'str', 'the row kind'), S('form', 'str', 'str', 'SH (Counter Shield, 0x89) or SH2 (Frying Pan, 0x91)'),
  S('code', 'int', 'i32', 'attack kind (attack, chain), grow level (grow), finish sub-step (finish), else 0'), S('logical', 'str', 'str', 'the logical anim name KH3 plays (attack / chain / anim / finish), the guard clip (grow), the blueprint (pawn), the old row (rename), the group (shape), the row (row)'),
  S('clip', 'str', 'str', 'the clip = the animation table row (the sequence); the new row (rename); the owner (shape); the model (pawn)'), S('length', 'num', 'f32', 'the clip length, s'),
  S('group', 'int', 'i32', 'the effect group the play passes (PlayAttack +0x198)'), S('root_len', 'num', 'f32', 'root length (+0x1D8), 0 = none'), S('stop_dist', 'num', 'f32', 'stop distance (+0x24C)'), S('z_gain', 'num', 'f32', 'z gain (+0x14C)'),
  S('air_mode', 'bool', 'bool', '+0x1DF'), S('traction', 'bool', 'bool', 'm_bEnableTraction at the play'), S('chan_xy', 'bool', 'bool', '+0x234 bit 0'), S('chan_root', 'bool', 'bool', '+0x234 bit 1'),
  S('flags', 'list', 'str[]', 'the case\'s other writes / the kind\'s strings (see the note)'), S('windows', 'list', 'str[]', 'the clip\'s hit windows a-b, s'), S('rows', 'list', 'str[]', 'the attack rows of those windows (Frying Pan: renamed); grow: the weapon parts'),
  S('effects', 'list', 'str[]', 'the clip\'s effects of that group (time@asset)'), S('sounds', 'list', 'str[]', 'the clip\'s sounds / voices (time@asset)'), S('values', 'list', 'f32[]', 'numbers by kind (see the note)'),
  S('ref_addr', 'str', 'str', 'decompiled function'), S('ref_conf', 'enum:confirmed|inferred|guess', 'str', 'confirmed = read in the code or the data; inferred; guess'), S('note', 'str', 'str', '-'),
];
const NAME = 'sora-form-shield';
const rows = ROWS.map((r) => COLS.map((c) => r[c[0]]));
for (const [i, r] of rows.entries()) for (const [k, v] of r.entries()) must(v !== undefined && v !== null && !(typeof v === 'number' && !Number.isFinite(v)), 'row ' + ROWS[i].id + ': cell ' + COLS[k][0] + ' is ' + v);
fs.writeFileSync(P('sheets', 'port', NAME + '.json'), '{"sheet":"' + NAME + '","group":"port","columns":' + JSON.stringify(COLS.map((c) => c[0])) + ',"rows":[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
const by = {}; for (const r of ROWS) by[r.what] = (by[r.what] || 0) + 1;
console.log('sheets/port/' + NAME + '.json: ' + rows.length + ' rows ' + JSON.stringify(by));

// ------------------------------------------------------------------------------------------------ register (this tool's entry only; replaced in place)
const q = (s) => "'" + String(s).replace(/\\/g, '\\\\').replace(/'/g, "\\'") + "'";
const NL = '\n';
const DOC = 'KH3\'s Counter Shield and Frying Pan as data for their form package (kh-core combat/forms/shield.rs, states/combo_sh.rs): COMBO_SH\'s attack kinds, the replacement anim sets, the grow levels, FINISH_SH, the row rename, the collision shapes.';
const MARK = '(weapons F-SHIELD)';
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
  const text = '  // ' + MARK + ' ' + DOC + ' Written by tools/sora_form_shield.js (documentation/KH3_WEAPONS_PLAN.md 1.3)' + NL +
    '  ' + q(NAME) + ": { group: 'port', file: " + q(NAME + '.json') + ", by: 'tools/sora_form_shield.js', from: " + q('sora-keyblade-forms, the form anim sets of the asset pack, the weapon / style-pawn blueprints, p_ex001_AttackData and the decompiled PlayAttack / ComboSH / Guard / StyleFinishSH') + ', external: true, cols: [' + NL +
    '    ' + COLS.map((c, i) => '[' + q(c[0]) + ', ' + (i === 0 ? 'K' : 'V') + ', ' + q(c[1]) + ", '-', " + q(c[3]) + (['note', 'flags', 'windows', 'rows', 'effects', 'sounds', 'values', 'logical', 'clip'].includes(c[0]) ? ', true' : '') + ']').join(', ') + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/sheet_defs.js');
}
{
  const f = P('tools', 'lib', 'port_gen_defs.js'); const before = fs.readFileSync(f, 'utf8');
  const END = NL + '    },' + NL;
  const pos = (t) => { const anchor = '  tables: {' + NL; const at = t.indexOf(anchor); must(at >= 0, 'port_gen_defs.js: anchor tables'); return at + anchor.length; };
  const typeOf = { str: "'str'", int: "'i32'", num: "'f32'", bool: "'bool'", list: null };
  const cols = COLS.filter((c) => c[2]).map((c) => c[0] + ': { type: ' + q(c[2]) + (c[3] && c[3] !== '-' ? ', doc: ' + q(c[3]) : '') + ' }').join(', ');
  void typeOf;
  const text = '    // ' + MARK + ' ' + NAME + ': written by tools/sora_form_shield.js (documentation/KH3_WEAPONS_PLAN.md 1.3)' + NL +
    '    ' + q(NAME) + ': {' + NL +
    "      file: 'sora_form_shield.rs', row: 'FormShieldRow', table: 'FORM_SHIELD', key: 'id', key_prefix: 'FSH'," + NL +
    '      doc: ' + q(DOC) + ',' + NL +
    '      columns: { ' + cols + ' },' + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/port_gen_defs.js');
}
console.log('sora_form_shield ' + VERSION);
