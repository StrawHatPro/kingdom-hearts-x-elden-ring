// (weapons F-CLAWS) KH3's claw / yo-yo / blade formchanges as DATA for their form package (kh-core combat/forms/claws.rs,
// states/combo_cl.rs, combo_yo.rs, combo_sk.rs): Agile Claws (form.CL 0x8A) -> Twin Yo-Yos (form.YO 0x82) on Happy Gear,
// Blizzard Claws (form.CL2 0x90) -> Blizzard Blades (form.SK 0x85) on Crystal Snow. Rows: the attack kinds of COMBO_CL /
// COMBO_YO / COMBO_SK (PlayAttack cases), the replacement anim sets (AnimSet12 for both claw forms, AnimSet4, AnimSet7:
// logical name -> clip), the finish commands FINISH_YO (StyleFinish slot 149) and FINISH_SK (StyleFinishSK), the attack-row
// rename of Blizzard Claws, the form weapons' collision shapes, the SHAPE TRACKS (where each hit shape of a form weapon is,
// per frame of each attack clip: FK of Sora's skeleton through the weapon's socket and the weapon's own skeleton and clip)
// and the attack rows the forms hit with. documentation/KH3_WEAPONS_PLAN.md 1.3 / 4, build/sora_tmp/weapons/F-CLAWS/STATE.md.
//
//   node tools/sora_form_claws.js   write sheets/port/sora-form-claws.json and register it (once, in place) in
//                                   tools/lib/sheet_defs.js and tools/lib/port_gen_defs.js; then node tools/gen_port.js
//
// Inputs (read only): sheets/port/sora-keyblade-forms.json (P3), build/sora_pack/keyblades/sora_forms_manifest.json (the
// form anim sets), build/sora_pack/manifest.json (Sora's identification, the base set, vfx_cure), build/sora_pack/
// keyblades/<model>/{model,sockets}.json (the form weapons: clips by logical name, sockets), assets_converted (Sora's and the
// weapons' skeletons and clips, Sora's skeleton sockets, the weapon blueprints' TresAtkColl0), sheets/entities/DataTable/
// Player/Attack/p_ex001_AttackData.json and THE CODE TABLES BELOW, read in the decompiled exe (re/decomp):
//   CLASS    pawn slot 1337 0x141432F00: forms 0x8A / 0x90 -> COMBO_CL (85), 0x82 -> COMBO_YO (77), 0x85 -> COMBO_SK (80).
//   VTABLES  ComboCL 0x14690CB10 (iface 0x14690CF88), ComboYO 0x146914228 (0x1469146A0), ComboSK 0x146911BD8 (0x146912050):
//            against ComboLimit 0x146910B80 (which overrides only Decide) they override slot 122 Decide (CL 0x141360440, YO
//            0x1413646B0, SK 0x141363570), YO slot 98 End 0x141350900 and slot 136 0x141337890 (the warp with 100), iface
//            slot 13 (attack-window begin) CL 0x141356F10 (the CL2 rename), iface slot 15 (on contact) YO 0x1413561B0 (the
//            Anchor Shot pull, 0x97's hit), SK 0x141355DD0 (0xAF into 0xB0 on a tall victim).
//   MAX      MaxCombo 0x141433570: CL / CL2 / SK 5, YO 6; Finishers 0x141433A60: CL / CL2 1, YO / SK 2.
//   PLAY     TresPlayerState_Attack::PlayAttack 0x1413378B0 cases 0x88 .. 0x91, 0x92 .. 0x9B, 0xAA .. 0xB4 (decompiled C).
//   FINISH   TryStyleFinish 0x141447840: FINISH_YO (0x9C) -> STYLEFINISH (33), its body slot 149 0x1413E0CD0 (Begin
//            0x1413CBF70 binds 0x1404EFA8C = vt+0x4A8); FINISH_SK (0x9F) -> STYLEFINISH_SK (35, vtable 0x14694B1E8): Reset
//            0x1413D0DC0, Begin 0x1413CC360, Tick 0x1413D6F30, End 0x1413CF2D0 (0x1413CFE10 the chandelier cleaned up).
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
const FORMS = Object.fromEntries(sheet('sora-keyblade-forms').filter((r) => ['form.CL', 'form.YO', 'form.CL2', 'form.SK'].includes(r.id)).map((r) => [r.id.slice(5), r]));
must(Object.keys(FORMS).length === 4, 'sora-keyblade-forms lacks one of form.CL / YO / CL2 / SK');
const OV = readJson(P('build', 'sora_pack', 'keyblades', 'sora_forms_manifest.json'));
const MAIN = readJson(P('build', 'sora_pack', 'manifest.json'));
const XSI = (() => { for (const f of [P('build', 'sora_pack', 'xsi_manifest.json'), P('build', 'sora_tmp', 'xsi', 'pack', 'xsi_manifest.json')]) if (fs.existsSync(f)) return readJson(f); return { items: [], logical_animations: [] }; })();
const ATK = readJson(P('sheets', 'entities', 'DataTable', 'Player', 'Attack', 'p_ex001_AttackData.json')).rows;
const setName = (a) => String(a).split('/').pop().split('.')[0];
const ITEMS = new Map([...MAIN.items, ...XSI.items, ...OV.items].filter((i) => i.kind === 'animation').map((i) => [i.animation_name, i]));
const BASE = new Map(MAIN.logical_animations.map((a) => [a.name, a]));
const XSI_ROWS = new Map(XSI.logical_animations.map((a) => [a.name, a]));
// logical name -> clip of a form: its set's rows, else the base set's row (the replacement order of m_ReplaceAnimSets).
const ROWS_OF = {};
for (const [code, f] of Object.entries(FORMS)) {
  const out = new Map();
  for (const set of f.anim_sets.map(setName)) for (const a of OV.logical_animations.filter((x) => setName(x.anim_set) === set)) out.set(a.name, { set, clip: a.clip, file: a.file, cls: a.class });
  ROWS_OF[code] = out;
}
function clipOf(form, logical) {
  const r = ROWS_OF[form].get(logical);
  if (r) return r.file ? r.clip : '-';
  // a row the asset agent left to the XS overlay (M_7419 Chandelier) or the base set (M_1501)
  if (XSI_ROWS.has(logical) && XSI_ROWS.get(logical).file) return path.basename(XSI_ROWS.get(logical).file, '.glb');
  const b = BASE.get(logical);
  return b && b.file ? path.basename(b.file, '.glb') : '-';
}
// the animation-table row a clip plays as: form-set clips are rows named by their sequence (tools/gen_port.js
// FORM_ANIM_SETS), the XS overlay's and the base set's rows by their logical name.
function rowOf(form, logical) {
  if (ROWS_OF[form].has(logical) && ROWS_OF[form].get(logical).file) return ROWS_OF[form].get(logical).clip;
  if (XSI_ROWS.has(logical) || BASE.has(logical)) return logical;
  return '-';
}
const item = (clip) => ITEMS.get(clip) || null;
const prop = (n, key) => { const p = n.properties || {}; const v = p[key]; return v && typeof v === 'object' && 'value' in v ? v.value : v; };
const atkRow = (id) => ATK.find((r) => r._row_name === id || String(r.id).toLowerCase() === String(id).toLowerCase());

// ------------------------------------------------------------------------------------------------ the code tables
// PLAY: PlayAttack 0x1413378B0 (decompiled C). Before the switch: m_bFaceAt; +0x234 = (+0x234 & ~6) | 1 (the XY channel on, the
// root-scale channel off); stop distance 160; z gain 3; the phases 0; +0x1DF (air mode) 0; +0x198 (the play's effect group)
// 0. A case that BREAKS out of the switch reaches `+0x1DF = 1` (air mode) at its end; the others jump past it. The tail
// (LAB_14133A87A) with air mode: +0x234 |= 4 | 0x10, GravityScale 0.1, gravity off, Flying, +0x18C = 0.6 (the air hold).
// Cases 0x88 .. 0x8E and 0x91 in form 0x90 (Blizzard Claws): +0x198 = 0x7DA (the effect group 2010: the ice variants of the
// claws' effects). Kinds 0x91 / 0x96 / 0xAF: LAB_141338CE1: +0x1F4 |= 0x20 (the facing pitches toward the target: Facing
// 0x14134D0E0) and +0x218 = the pitch at the play (End 0x141335120 levels him again).
const PLAY = [
  // Agile Claws / Blizzard Claws (COMBO_CL 85)
  { kind: 0x88, logical: 'M_C411', root: 400, chan_root: true, air: true, ice: true, at: '0x1413399E2', label: 'combo step (reach 0 / after 0x89; Decide 0x141360440)' },
  { kind: 0x89, logical: 'M_C412', root: 800, z: 4.5, chan_root: true, air: true, ice: true, at: '0x141339993', label: 'combo step, the long dash (reach 1, after 0x8A)' },
  { kind: 0x8A, logical: 'M_C413', root: 400, chan_root: true, air: true, ice: true, at: '0x1413399DA', label: 'combo step (reach 2, after 0x88)' },
  { kind: 0x8B, logical: 'M_C414', root: 500, chan_root: true, air: true, ice: true, at: '0x1413399EA', label: 'every fourth step (combo % 4 == 3)' },
  { kind: 0x8C, logical: 'M_C415', air: true, ice: true, flags: ['finish', 'finish_end'], at: '0x141339A2D', label: 'the finisher (combo >= MaxCombo - Finishers)' },
  { kind: 0x8D, logical: 'M_C416', air: true, ice: true, flags: ['rise'], at: '0x141339A4B', label: 'the rise (target high, dist <= 400, not right after 0x8D): DirZ fixed (pawn+0xBAC x 200)' },
  { kind: 0x8E, logical: 'M_C417', root: 200, chan_root: true, air: true, ice: true, dive: true, no_vz: true, at: '0x141339A69', label: 'the falling attack (target 150 below, dist <= 400): +0x1DE dive, +0x234 |= 8 (no Z traction)' },
  { kind: 0x91, logical: 'M_C418', root: 1000, z: 4.5, air: true, ice: true, air_hold: 0.3, pitch: true, flags: ['flow', 'draw'], at: '0x141339AA6', label: 'the flow attack (forced 0x36: out of a rail / super slide, a wall kick): +0x190 |= 1; head of PlayAttack: pawn vt+0x1388(1, 0), vt+0x1388(0)' },
  // Twin Yo-Yos (COMBO_YO 77)
  { kind: 0x92, logical: 'M_4411', root: 350, stop: 350, air: true, at: '0x141338C20', label: 'combo step (combo % 4 == 0; reach 2: 1)' },
  { kind: 0x93, logical: 'M_4412', root: 400, stop: 350, air: true, at: '0x141338BF6', label: 'combo step (combo % 4 == 1; reach 2: 3)' },
  { kind: 0x94, logical: 'M_4413', root: 350, stop: 350, air: true, at: '0x141338C1F', label: 'combo step (combo % 4 == 2)' },
  { kind: 0x95, logical: 'M_4414', root: 400, stop: 350, air: true, at: '0x141338C55', label: 'combo step (combo % 4 == 3; reach 2: 0)' },
  { kind: 0x96, logical: 'M_4415', root: 100, air: true, pitch: true, flags: ['hookshot'], at: '0x141338CB6', label: 'Anchor Shot (dist > 400, not right after 0x96 / 0x97): its hit pulls him to the target (iface slot 15 0x1413561B0)' },
  { kind: 0x97, logical: 'M_4416', air: true, flags: ['pull'], at: '0x141338CA0', label: 'the pull after Anchor Shot\'s hit (0x1413561B0): a hit of it enables the next attack (state+0x84 |= 1)' },
  { kind: 0x98, logical: 'M_4417', air: true, flags: ['rise'], at: '0x141338C95', label: 'the rise (target high, dz > 150, dist <= 400): DirZ fixed (pawn+0xBAC x 200)' },
  { kind: 0x99, logical: 'M_4417', z: 4.5, air: true, at: '0x141338C76', label: 'target below (dz < -100, dist <= 400, not right after 0x99)' },
  { kind: 0x9A, logical: 'M_4418', root: 200, air: true, flags: ['finish', 'finish_end'], at: '0x141338C55', label: 'the last finisher' },
  { kind: 0x9B, logical: 'M_4419', root: 500, stop: 350, air: true, flags: ['finish'], at: '0x141338E0B', label: 'the first finisher (finish count < Finishers - 1, combo < MaxCombo - 1)' },
  // Blizzard Blades (COMBO_SK 80)
  { kind: 0xAA, logical: 'M_7411', root: 340, chan_root: true, air: true, at: '0x14133935E', label: 'combo step (ground)' },
  { kind: 0xAB, logical: 'M_7412', root: 800, chan_root: true, air: true, at: '0x141339388', label: 'combo step (ground), the long slide' },
  { kind: 0xAC, logical: 'M_7413', root: 360, chan_root: true, air: true, at: '0x1413393A3', label: 'combo step (ground)' },
  { kind: 0xAD, logical: 'M_7414', root: 300, chan_root: true, air: true, at: '0x1413393E7', label: 'combo step (ground)' },
  { kind: 0xAE, logical: 'M_7415', root: 400, chan_root: true, air: true, flags: ['finish'], at: '0x141339411', label: 'the first finisher (ground)' },
  { kind: 0xAF, logical: 'M_7417', root: 1000, stop: 0, z: 4.5, air: true, pitch: true, flags: ['tall_hit'], at: '0x141339437', label: 'the air step (dz >= -250 or dist > 500); a hit on a tall victim turns it into 0xB0 (iface slot 15 0x141355DD0)' },
  { kind: 0xB0, logical: 'M_7422', air: true, at: '0x14133946D', label: 'after 0xAF\'s hit on a victim at least 220 cm tall (0x141355DD0)' },
  { kind: 0xB1, logical: 'M_7416', root: 190, chan_root: true, air: true, flags: ['finish', 'finish_end'], at: '0x14133A80E', label: 'the last finisher (ground)' },
  { kind: 0xB2, logical: 'M_7418', root: 1200, chan_root: true, air: true, flags: ['finish', 'finish_end'], at: '0x141339495', label: 'the last finisher (air)' },
  { kind: 0xB3, logical: 'M_7420', air: true, flags: ['rise'], at: '0x1413394B4', label: 'the rise (ground, target more above than away and 100 above)' },
  { kind: 0xB4, logical: 'M_7421', air: true, dive: true, no_vz: true, keep_dir_z: true, dir_z: -4000, at: '0x1413393C0', label: 'the dive (air, target 250 below, dist <= 500): +0x234 |= 8, +0x23C |= 0x10, +0x1DE, DirZ -4000' },
];
const FAMILY = { CL: [0x88, 0x91], CL2: [0x88, 0x91], YO: [0x92, 0x9B], SK: [0xAA, 0xB4] };
const ICE_GROUP = 0x7DA;
// RENAME: ComboCL iface slot 13 0x141356F10 (decompiled C): in form 0x90 the SCCL_* rows of the shared clips are CL2_*.
const RENAME = ['SCCL_CROSS', 'SCCL_STAB', 'SCCL_SPIN', 'SCCL_QUAD', 'SCCL_QUAD4', 'SCCL_FINISH', 'SCCL_RISE', 'SCCL_DOWN', 'SCCL_CRASH'];
// FINISH: FINISH_YO = StyleFinish slot 149 0x1413E0CD0; FINISH_SK = StyleFinishSK Tick 0x1413D6F30, by sub-step.
const FINISH = {
  YO: [
    { sub: 0, logical: 'M_4420', what: 'gravity off (pawn vt+0x1728(0)), Flying (vt+0x17A0(5, 1)); Play("M_4420", blend -1, mode 2) + voice group 0; +0x188 = -150, +0x260 = 1.0, +0x194 (a timer) = 0' },
    { sub: 1, logical: '-', what: 'until a hit window of M_4420 opens (+0x164)' },
    { sub: 2, logical: '-', what: 'facing on (slot 127(1, 0.2)); with a caught pawn (m_pCatchPawn): wait until it is in state 62, then it is stopped, its gravity off, MOVE_None and attached to the yo-yo\'s X_link01; without one: until M_4420 ends or a window opens again (the timer gives up after 10 s: DoChain); then WorldSlow(off), facing off, his pitch levelled, sub 5' },
    { sub: 5, logical: '-', what: 'the caught pawn kept on X_link01 (relative location 0); at M_4420\'s end: Falling (vt+0x17A0(3, 1)), gravity on, the default gravity scale, DoChain' },
  ],
  SK: [
    { sub: 0, logical: 'M_7419', what: 'the Chandelier style pawn (TresStylePawnSKChandelier, w_so088; vt+0x1460 = the accompany pawn class of the finish command) spawned at his location with his yaw, hidden (vt+0xD98(0)); he is put where it stands; not on the ground: Walking? (vt+0x17A0(1)); Play("M_7419") + voice 0; +0x2E8 (its clock) = 0; vt+0x1038(0, 0)' },
    { sub: 1, logical: 'M_7423', what: 'at M_7419\'s end: weapon off (vt+0xFF0(0), vt+0x1010(0, 0)), Flying; he and the chandelier play M_7423; chandelier +0xB48 (its done flag) = 0' },
    { sub: 2, logical: '-', what: 'the chandelier shown (vt+0xD98()); falls into 3' },
    { sub: 3, logical: '-', what: 'until the chandelier\'s done flag (or no chandelier): WorldSlow(off), sub 5' },
    { sub: 5, logical: '-', what: 'while the clock (+0x2E8, counted every tick from sub 1) is below the chandelier\'s m_Param_MaxTime (vt+0x2620), no TresEnableChange and his clip has not ended: wait; then sub 6' },
    { sub: 6, logical: 'M_1501', what: '0x1413CFE10: the chandelier cleaned up (vt+0x1038(0x42) once, vt+0x23B0(0), its effects ended); Play("M_1501")' },
    { sub: 7, logical: '-', what: 'one tick' },
    { sub: 8, logical: '-', what: 'at M_1501\'s end: DoChain' },
  ],
};

// ------------------------------------------------------------------------------------------------ the weapons' shapes
const PKG = P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content', 'Blueprints');
function shapesOf(rel) {
  let f = path.join(PKG, rel + '.uasset.json');
  if (!fs.existsSync(f)) f = path.join(PKG, rel.replace(/_Wep$/, '_wep') + '.uasset.json');
  if (!fs.existsSync(f)) return [];
  const d = readJson(f);
  const flat = (props) => { const o = {}; for (const p of props || []) o[p.name] = p.type === 'StructProperty' ? (Array.isArray(p.value) ? flat(p.value) : p.value) : p.type === 'ArrayProperty' ? (p.value && p.value.items ? p.value.items.map((it) => (Array.isArray(it) ? flat(it) : it)) : p.value) : p.value; return o; };
  const out = [];
  for (const e of d.exports) {
    if (e.class !== 'TresAtkCollComponent') continue;
    const p = flat((e.data && e.data.props) || e.props);
    for (const s of p.CollisionShapesSrc || []) {
      const v = (o) => (o ? [o.x || 0, o.y || 0, o.z || 0] : [0, 0, 0]);
      out.push({ owner: rel.split('/').pop().replace(/_[Ww]ep$/, ''), group: s.m_GrpName, shape: String(s.m_ShapeType).split('::').pop(), size: v(s.m_Size), socket1: s.m_SocketName1 || 'None', socket2: s.m_SocketName2 || 'None', loc: v(s.m_RelativeLocation1), row: s.m_DefaultAttackDataIDName || 'None', attach: String(s.m_AttachType1 || '').split('::').pop(),
        // (F-SHAPES) what the CONVEX transform reads (Update 0x140647950 case 6 / 8; combat/hull.rs)
        mesh: s.m_Mesh < 0 && d.imports[-s.m_Mesh - 1] ? d.imports[-s.m_Mesh - 1].objectName : '-', scale: v(s.m_Scale), inc_scale: v(s.m_IncScale), max_scale: v(s.m_IncMaxScale),
        rot: s.m_RelativeRocation ? { pitch: s.m_RelativeRocation.pitch || 0, yaw: s.m_RelativeRocation.yaw || 0, roll: s.m_RelativeRocation.roll || 0 } : { pitch: 0, yaw: 0, roll: 0 },
        rot_attach: String(s.m_RotAttachType || 'TRES_ACRA_NORMAL').split('::').pop(), abs1: s.m_bAbsoluteOffset1 === true, ignore_scale: s.m_bIgnoreParentScale === true, frozen: s.m_bDisableLocation1Attach === true });
    }
  }
  return out;
}
// the form's weapon parts: unit@part:socket=blueprint (the row's units)
const partsOf = (form) => FORMS[form].units.map((u) => { const m = u.match(/^(\w+)@(\w+):(\w+)=(.*)$/); must(m, 'unit ' + u); return { unit: m[1], hand: m[2], socket: m[3] === 'None' ? 'R_wep_skt' : m[3], blueprint: m[4], model: m[4].split('/').pop().replace(/_[Ww]ep$/, '') }; });
const SHAPES = {};
for (const form of Object.keys(FORMS)) SHAPES[form] = partsOf(form).flatMap((p) => shapesOf('Player/p_ex001/' + p.model + '_Wep').map((s) => ({ ...s, part: p })));
const PAWN_SHAPES = shapesOf('StylePawn/w_so088');
// (AUDIT-B W-FIN) THE CHANDELIER'S OWN FRAME, from ITS blueprint (StylePawn/w_so088), not Sora's: its TresCharMesh0 sets no
// RelativeLocation / RelativeRotation (the Kraken w_so094, w_so073, w_so035 and p_ex001 all set yaw 270) and the native
// ATresCharPawnBase ctor FUN_14059F950 writes none (it attaches the mesh to the root, 0x144043A80, and sets flags only), so the
// mesh component IS the root: rotation 0, location 0. Its TresRoot overrides CollisionShapesSrc: a SPHERE r 250 at z 250 (the
// "capsule": ATresPawnBase vt+0x9F0 0x1405D4BA0 = -(MyRoot vt+0xA58 0x1405CD120: the shapes' lowest point - the root) >= 0 = 0,
// so the land spawn 0x1413F5840 stands the root ON the floor; the shape's centre is 250 cm up).
const PAWN_BP = (() => {
  const d = readJson(path.join(PKG, 'StylePawn', 'w_so088.uasset.json'));
  const props = (e) => (e && ((e.data && e.data.props) || e.props)) || [];
  const get = (e, n) => props(e).find((p) => p.name === n);
  const mesh = d.exports.find((e) => e.class === 'TresSkeletalMeshComponent' && e.objectName === 'TresCharMesh0');
  const root = d.exports.find((e) => e.class === 'TresRootComponent' && e.objectName === 'TresRoot');
  must(mesh && root, 'w_so088: no TresCharMesh0 / TresRoot');
  const rr = (get(mesh, 'RelativeRotation') || {}).value || { pitch: 0, yaw: 0, roll: 0 }, rl = (get(mesh, 'RelativeLocation') || {}).value || { x: 0, y: 0, z: 0 };
  must(!rr.pitch && !rr.roll, 'w_so088: a pitched / rolled mesh is not handled');
  const my = (rr.yaw || 0) * Math.PI / 180, c = Math.cos(my), s = Math.sin(my);
  const shapes = ((get(root, 'CollisionShapesSrc') || {}).value || {}).items || [];
  must(shapes.length === 1, 'w_so088: expected one root shape');
  const sv = (n) => (shapes[0].find((p) => p.name === n) || {}).value;
  return {
    yaw_deg: rr.yaw || 0,
    meshQ: [0, 0, Math.sin(my / 2), Math.cos(my / 2)],
    actor: (v) => [v[0] * c - v[1] * s + (rl.x || 0), v[0] * s + v[1] * c + (rl.y || 0), v[2] + (rl.z || 0)],
    shape: String(sv('ShapeType')).split('::').pop(), size: [(sv('Size') || {}).x || 0, (sv('Size') || {}).y || 0, (sv('Size') || {}).z || 0], loc: [(sv('RelativeLocation') || {}).x || 0, (sv('RelativeLocation') || {}).y || 0, (sv('RelativeLocation') || {}).z || 0],
  };
})();

// ------------------------------------------------------------------------------------------------ FK (as tools/sora_pack.js attackSockets)
const qmulH = (a, b) => [a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1], a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0], a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3], a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]];
const qrot = (q, v) => { const tx = 2 * (q[1] * v[2] - q[2] * v[1]), ty = 2 * (q[2] * v[0] - q[0] * v[2]), tz = 2 * (q[0] * v[1] - q[1] * v[0]); return [v[0] + q[3] * tx + (q[1] * tz - q[2] * ty), v[1] + q[3] * ty + (q[2] * tx - q[0] * tz), v[2] + q[3] * tz + (q[0] * ty - q[1] * tx)]; };
const under = (c, p) => { const v = qrot(p.q, [p.s[0] * c.t[0], p.s[1] * c.t[1], p.s[2] * c.t[2]]); return { q: qmulH(p.q, c.q), t: [v[0] + p.t[0], v[1] + p.t[1], v[2] + p.t[2]], s: [p.s[0] * c.s[0], p.s[1] * c.s[1], p.s[2] * c.s[2]] }; };
const point = (T, v) => { const r = qrot(T.q, [T.s[0] * v[0], T.s[1] * v[1], T.s[2] * v[2]]); return [r[0] + T.t[0], r[1] + T.t[1], r[2] + T.t[2]]; };
// FRotator (degrees) -> quaternion, the stock UE formula (as sora_pack.js's vfx_cure)
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
const yaw = ID.mesh_component.relative_rotation_ue_deg.yaw * Math.PI / 180, cy = Math.cos(yaw), sy = Math.sin(yaw);
const actor = (v) => [v[0] * cy - v[1] * sy, v[0] * sy + v[1] * cy, v[2]];
const local = (b, tr, time) => ({ q: tr.rotation ? sp.sampleChan(tr.rotation, time)(tr.rotation.ue_values) : b.ue.rotation, t: tr.translation ? sp.sampleChan(tr.translation, time)(tr.translation.ue_values) : b.ue.translation, s: tr.scale ? sp.sampleChan(tr.scale, time)(tr.scale.ue_values) : b.ue.scale });
const byBone = (a) => { const m = new Map(); if (a) for (const c of a.chans) { if (!m.has(c.bone)) m.set(c.bone, {}); m.get(c.bone)[c.path] = c; } return m; };
// the component-space transform of bone k of a skeleton under a clip (root motion: the root's own track is the actor's)
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
// the weapon's own clip for Sora's logical name (its rows of the same names), else none (reference pose)
function weaponAnim(w, logical, clip) {
  const key = logical + '|' + clip;
  if (w.anims.has(key)) return w.anims.get(key);
  const c = w.mj.clips.find((x) => (x.logical_names || []).includes(logical)) || w.mj.clips.find((x) => x.sequence === clip);
  let a = null;
  if (c) { try { a = sp.loadAnimation(c.asset + '.' + c.sequence, w.bones); } catch (e) { a = null; } }
  w.anims.set(key, { a, seq: c ? c.sequence : '-' });
  return w.anims.get(key);
}
// the world (actor-space) transform of a weapon socket (or bone) at Sora's clip time
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
// the shape's segment and radius at a time [ax, ay, az, bx, by, bz, r] (actor space): SPHERE / CONVEX at socket 1 (+ the
// relative location in its frame; a CONVEX as a sphere of m_Size.X), CAPSULE1 / CAPSULE2 between socket 1 and socket 2 (the
// relative location in each socket's frame). Owner-attached shapes at Sora's own sockets (center: the capsule centre
// (0, 0, 91) as combat/hit.rs takes it; vfx_cure: the pack's).
function shapeAt(s, logical, clip, time) {
  if (s.attach === 'OWNER_ATTACHED_MESH') {
    const base = s.socket1 === 'vfx_cure' && MAIN.attack_sockets.vfx_cure ? MAIN.attack_sockets.vfx_cure.position_actor_cm : [0, 0, 91];
    const l = actor(s.loc); // the relative location in the mesh frame (its yaw); the socket's own rotation is not read: [I]
    const c = [base[0] + l[0], base[1] + l[1], base[2] + l[2]];
    return [...c, ...c, s.size[0]];
  }
  const T1 = weaponSocketT(s.part, s.socket1, logical, clip, time);
  if (!T1) return null;
  const a = actor(point(T1, s.loc));
  if (s.shape === 'SPHERE' || s.shape === 'CONVEX' || s.socket2 === 'None') return [...a, ...a, s.size[0]];
  const T2 = weaponSocketT(s.part, s.socket2, logical, clip, time);
  if (!T2) return [...a, ...a, s.size[0]];
  return [...a, ...actor(point(T2, s.loc)), s.size[0]];
}
// (F-SHAPES) a CONVEX's real transform (combat/hull.rs `VolTrack` HULL; Update 0x140647950 case 6 / 8): T1 the socket; location =
// T1.t + (m_bAbsoluteOffset1 ? relLoc x T1.s : T1.q (relLoc x T1.s)); rotation = LOCAL ? relRot (world) : T1.q x relRot; scale =
// FUN_140649BF0(t) x T1.s. `at` = the clip time of the place (the window's start for m_bDisableLocation1Attach), `t` = the time
// since the window opened. Any other shape of such a group: its capsule + [0, 0, 0].
const MESH_YAW_Q = [0, 0, Math.sin(yaw / 2), Math.cos(yaw / 2)];
const growScale = (s, t) => { if (s.inc_scale.every((x) => x === 0)) return s.scale; return s.scale.map((b, i) => { const v = Math.max(b + s.inc_scale[i] * t, 0.01); return s.max_scale[i] > 0 ? Math.min(v, s.max_scale[i]) : v; }); };
function volAt(s, logical, clip, at, t) {
  if (s.shape !== 'CONVEX' || s.attach === 'OWNER_ATTACHED_MESH') { const v = s.shape === 'CONVEX' ? null : shapeAt(s, logical, clip, at); return v ? [...v.map(r1), 0, 0, 0] : null; }
  const T1 = weaponSocketT(s.part, s.socket1, logical, clip, at);
  if (!T1) return null;
  const sl = [s.loc[0] * T1.s[0], s.loc[1] * T1.s[1], s.loc[2] * T1.s[2]];
  const off = s.abs1 ? sl : qrot(T1.q, sl);
  const c = actor([T1.t[0] + off[0], T1.t[1] + off[1], T1.t[2] + off[2]]);
  const q = s.rot_attach === 'TRES_ACRA_LOCAL' ? rotQ(s.rot) : qmulH(MESH_YAW_Q, qmulH(T1.q, rotQ(s.rot)));
  const n = Math.hypot(...q);
  const g = growScale(s, t);
  const sc = s.ignore_scale ? g : g.map((x, i) => x * T1.s[i]);
  return [...c.map(r1), ...q.map((x) => Math.round(x / n * 1e6) / 1e6), ...sc.map(r4)];
}
const volFlags = (s) => [s.shape === 'CONVEX' ? 'HULL' : 'CAPSULE', s.shape === 'CONVEX' ? s.mesh : '-', s.shape === 'CONVEX' && s.rot_attach === 'TRES_ACRA_LOCAL' ? 'world' : 'actor', s.owner + ':' + s.shape + ':' + s.socket1 + '-' + s.socket2 + (s.frozen ? ':frozen' : '') + ':' + s.rot_attach];
// several shapes of one group (KH3: one record, one hit list): their union as one capsule, the two farthest-apart end
// points, the largest radius ([I]: the port's generic window has one shape per group).
function mergeShapes(list) {
  const pts = list.flatMap((v) => [v.slice(0, 3), v.slice(3, 6)]);
  let best = [pts[0], pts[0]], d = -1;
  for (let i = 0; i < pts.length; i++) for (let j = i; j < pts.length; j++) { const e = Math.hypot(pts[i][0] - pts[j][0], pts[i][1] - pts[j][1], pts[i][2] - pts[j][2]); if (e > d) { d = e; best = [pts[i], pts[j]]; } }
  return [...best[0], ...best[1], Math.max(...list.map((v) => v[6]))];
}

// ------------------------------------------------------------------------------------------------ notifies of a clip
function notes(clip, group) {
  const it = item(clip);
  const out = { length: it ? r4(it.duration_s) : 0, frames: it ? it.num_frames : 0, windows: [], groups: [], rows: [], change: -1, input: -1, effects: [], sounds: [] };
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

// ------------------------------------------------------------------------------------------------ rows
const ROWS = [];
const blank = { form: '-', code: 0, logical: '-', clip: '-', length: 0, group: 0, root_len: 0, stop_dist: 0, z_gain: 0, air_mode: false, chan_xy: true, chan_root: false, flags: [], windows: [], rows: [], effects: [], sounds: [], values: [], ref_addr: '-', ref_conf: 'confirmed', note: '-' };
const add = (r) => ROWS.push({ ...blank, ...r });
const renamed = (form, row) => (form === 'CL2' && RENAME.includes(row) ? row.replace(/^SCCL_/, 'CL2_') : row);
const tracksWanted = []; // [form, logical, clip]
for (const form of ['CL', 'CL2', 'YO', 'SK']) {
  const [lo, hi] = FAMILY[form];
  for (const k of PLAY.filter((x) => x.kind >= lo && x.kind <= hi)) {
    const clip = rowOf(form, k.logical);
    must(clip !== '-', form + ' ' + k.logical + ': no clip');
    const group = form === 'CL2' && k.ice ? ICE_GROUP : 0;
    const n = notes(clipOf(form, k.logical), group);
    const rows = n.rows.map((r) => renamed(form, r));
    for (const r of rows) must(atkRow(r), form + ' ' + k.logical + ': p_ex001_AttackData has no row ' + r);
    const flags = [...(k.flags || [])];
    if (k.dive) flags.push('dive');
    if (k.no_vz) flags.push('no_vz');
    if (k.keep_dir_z) flags.push('keep_dir_z');
    if (k.pitch) flags.push('pitch');
    add({ id: 'attack.' + form + '.' + hex(k.kind), what: 'attack', form, code: k.kind, logical: k.logical, clip, length: n.length, group, root_len: k.root || 0, stop_dist: k.stop === undefined ? 160 : k.stop, z_gain: k.z || 3, air_mode: !!k.air, chan_xy: true, chan_root: !!k.chan_root, flags,
      windows: n.windows.map((w) => { const p = w.split('@'); return p[0] + '@' + p[1] + '@' + renamed(form, p[2]) + '@' + p[3]; }), rows, effects: n.effects, sounds: n.sounds, values: [n.change, n.input, k.air_hold || 0, k.dir_z || 0],
      ref_addr: k.at, note: k.label + '; values = [TresEnableChange, TresEnableInput of the clip (-1 none), +0x18C (0 = the air-mode tail\'s 0.6), DirZ (0 = none)]; windows = start-end@group@row@AttackInterval' });
    tracksWanted.push([form, k.logical, clipOf(form, k.logical)]);
  }
}
// the replacement anim sets (logical name -> clip), per form
for (const form of ['CL', 'CL2', 'YO', 'SK']) {
  for (const [logical, r] of [...ROWS_OF[form]].sort((a, b) => (a[0] < b[0] ? -1 : 1))) {
    add({ id: 'anim.' + form + '.' + logical, what: 'anim', form, logical, clip: r.file ? r.clip : '-', length: r.file && item(r.clip) ? r4(item(r.clip).duration_s) : 0, flags: [r.set, r.cls], note: r.file ? '-' : 'a ' + r.cls + ' of the set (the locomotion blend spaces RunLoopB / RunStartB): not a row of the animation table (its samples are)' });
  }
}
// the finish commands
for (const form of ['YO', 'SK']) for (const f of FINISH[form]) {
  const clip = f.logical === '-' ? '-' : rowOf(form, f.logical);
  const n = f.logical !== '-' ? notes(clipOf(form, f.logical), 0) : null;
  add({ id: 'finish.' + form + '.' + f.sub, what: 'finish', form, code: f.sub, logical: f.logical, clip, length: n ? n.length : 0, windows: n ? n.windows : [], rows: n ? n.rows : [], effects: n ? n.effects : [], sounds: n ? n.sounds : [], values: n ? [n.change, n.input] : [],
    ref_addr: form === 'YO' ? '0x1413E0CD0' : '0x1413D6F30', note: f.what });
  if (f.logical !== '-' && f.logical !== 'M_1501') tracksWanted.push([form, f.logical, clipOf(form, f.logical)]);
}
// the chandelier (FINISH_SK's style pawn w_so088): its clip and its hit window (keyblades/w_so088/model.json)
{
  const mj = readJson(P('build', 'sora_pack', 'keyblades', 'w_so088', 'model.json'));
  const c = mj.clips.find((x) => x.sequence === '7423');
  must(c, 'w_so088 has no 7423');
  const wins = (c.notifies || []).filter((n) => n.class === 'TresAnimNotifyState_Attack').map((n) => r4(n.trigger_time) + '-' + r4(n.end_trigger_time) + '@' + n.properties.CollisionGrpName + '@' + n.properties.m_AttackDataIDName + '@0');
  const sh = PAWN_SHAPES.find((s) => s.group === 'FinishSK2') || { size: [600, 0, 0], loc: [0, 0, 0], shape: 'SPHERE' };
  add({ id: 'pawn.SK.w_so088', what: 'pawn', form: 'SK', logical: 'M_7423', clip: 'w_so088', length: r4(c.duration_s), windows: wins, rows: wins.map((w) => w.split('@')[2]), values: [10, sh.size[0], ...sh.loc, PAWN_BP.loc[2], PAWN_BP.size[0]] /* (AUDIT-B W-FIN) + its root shape (TresRoot): centre z, radius */,
    ref_addr: '0x1413D6F30', ref_conf: 'inferred', note: 'the Chandelier (TresStylePawnSKChandelier): its clip 7423 and hit window; values = [m_Param_MaxTime s (ATresAccompanyPawnBase: 10, as abilities/xs/chandelier.rs takes it), the FinishSK2 shape radius cm, its relative location x, y, z, (AUDIT-B W-FIN) the centre height of its root shape above the root and radius cm (TresRoot ' + PAWN_BP.shape + '; the root stands on the floor)] (StylePawn/w_so088 TresAtkColl0; ' + sh.shape + ')' });
  // (F-SHAPES) FinishSK2 is a CONVEX (tres_colltest01 x (3, 3, 8) + (30, 30, 0)/s up to 30 on the pawn's kh_sk, + (0, -300, 0), roll 90,
  // NORMAL): its hull per frame of 7423 (combat/hull.rs VolTrack HULL) in the pawn's root space (as the Clock of F-HAMMER: the
  // pawn's mesh turned as Sora's), its growth from the window's start; m_Size.X above stays the record of the old stand-in.
  if (sh.shape === 'CONVEX') {
    const w = weapon('w_so088');
    must(w, 'w_so088: no model');
    const { a } = weaponAnim(w, 'M_7423', '7423');
    const k = w.bones.findIndex((b) => b.name === sh.socket1);
    must(k >= 0, 'w_so088: no bone ' + sh.socket1);
    const starts = wins.map((x) => +x.split('-')[0]);
    const n = Math.max(1, Math.ceil(c.duration_s * 30));
    const vals = [];
    for (let i = 0; i <= n; i++) {
      const time = i * c.duration_s / n;
      const start = starts.filter((s) => s <= time + 1e-6).pop();
      const T1 = boneT(w.bones, byBone(a), k, a ? Math.min(time, a.length) : 0, false);
      const sl = [sh.loc[0] * T1.s[0], sh.loc[1] * T1.s[1], sh.loc[2] * T1.s[2]];
      const off = sh.abs1 ? sl : qrot(T1.q, sl);
      // (AUDIT-B W-FIN) in the Chandelier's own mesh frame (PAWN_BP: its mesh = its root, yaw 0; was Sora's yaw 270)
      const cc = PAWN_BP.actor([T1.t[0] + off[0], T1.t[1] + off[1], T1.t[2] + off[2]]);
      const q = sh.rot_attach === 'TRES_ACRA_LOCAL' ? rotQ(sh.rot) : qmulH(PAWN_BP.meshQ, qmulH(T1.q, rotQ(sh.rot)));
      const m = Math.hypot(...q);
      const g = growScale(sh, start === undefined ? 0 : time - start);
      const sc = sh.ignore_scale ? g : g.map((x, j) => x * T1.s[j]);
      vals.push(...cc.map(r1), ...q.map((x) => Math.round(x / m * 1e6) / 1e6), ...sc.map(r4));
    }
    add({ id: 'vol.SK.7423.FinishSK2.0', what: 'vol', form: 'SK', code: n + 1, logical: 'FinishSK2', clip: '7423', length: r4(c.duration_s), flags: [...volFlags({ ...sh, owner: 'w_so088' })], windows: wins.map((x) => x.split('@')[0]), values: vals, ref_addr: '0x140647950',
      note: 'the Chandelier\'s FinishSK2 per frame of its 7423 (combat/hull.rs VolTrack HULL [x, y, z, qx, qy, qz, qw, sx, sy, sz]: Update 0x140647950 case 6\'s location / rotation / scale, the growth from the window\'s start) in the pawn\'s root space (its yaw 0), FK of w_so088\'s ' + sh.socket1 });
  }
}
// the Blizzard Claws row rename
for (const r of RENAME) add({ id: 'rename.CL2.' + r, what: 'rename', form: 'CL2', logical: r, clip: r.replace(/^SCCL_/, 'CL2_'), ref_addr: '0x141356F10', note: 'in form 0x90 an attack window of this row hits with the other' });
// the collision shapes
for (const form of ['CL', 'CL2', 'YO', 'SK']) for (const s of SHAPES[form]) {
  add({ id: 'shape.' + form + '.' + s.owner + '.' + s.group + '.' + s.shape, what: 'shape', form, logical: s.group, clip: s.owner, flags: [s.shape, s.socket1, s.socket2, s.attach, s.part.socket], values: [...s.size, ...s.loc], rows: [s.row], note: 'TresAtkColl0 CollisionShapesSrc of Player/p_ex001/' + s.owner + '_Wep: flags = [shape, socket 1, socket 2, attach type, Sora\'s socket the part hangs on], values = [size x, y, z, location 1 x, y, z]' });
}
// THE SHAPE TRACKS: for every clip above and every group its windows name, the group's shapes on the form's weapons as one
// capsule: per frame (time = frame x length / (frames - 1)) [ax, ay, az, bx, by, bz, radius] in Sora's actor space (UE cm,
// his yaw 0), as build/sora_pack/manifest.json attack_sockets for the Kingdom Key. Keyed by the clip's SEQUENCE (the hit
// window's sequence) and the group.
let tracked = 0;
const seen = new Set();
for (const [form, logical, clip] of tracksWanted) {
  const it = item(clip);
  if (!it) continue;
  const groups = [...new Set((it.notifies || []).filter((n) => n.class === 'TresAnimNotifyState_Attack').map((n) => String(prop(n, 'CollisionGrpName'))))];
  for (const g of groups) {
    const shapes = SHAPES[form].filter((s) => s.group === g);
    if (!shapes.length) continue;
    const id = 'track.' + form + '.' + clip + '.' + g;
    if (seen.has(id)) continue;
    seen.add(id);
    const frames = it.num_frames, len = it.duration_s;
    // (F-SHAPES) a group with a CONVEX on a form weapon: one `vol` row per shape (combat/hull.rs) instead of one merged sphere of m_Size
    if (shapes.some((s) => s.shape === 'CONVEX' && s.attach !== 'OWNER_ATTACHED_MESH')) {
      const mine = (it.notifies || []).filter((n) => n.class === 'TresAnimNotifyState_Attack' && String(prop(n, 'CollisionGrpName')) === g).map((n) => ({ a: +n.time, b: +n.end_time }));
      const rows = [];
      for (const [k, s] of shapes.entries()) {
        const vals = [];
        for (let f = 0; f < frames; f++) {
          const t = frames > 1 ? f * len / (frames - 1) : 0;
          const w = mine.filter((x) => x.a <= t + 1e-6).pop() || mine[0];
          const v = volAt(s, logical, clip, s.frozen ? w.a : t, Math.max(t - w.a, 0));
          if (!v) { vals.length = 0; break; }
          vals.push(...v);
        }
        if (!vals.length) break;
        rows.push({ id: 'vol.' + form + '.' + clip + '.' + g + '.' + k, what: 'vol', form, code: frames, logical: g, clip, length: r4(len), flags: volFlags(s), windows: mine.map((w) => r4(w.a) + '-' + r4(w.b)), values: vals, ref_addr: '0x140647950',
          note: 'shape ' + k + ' of group ' + g + ' per frame (combat/hull.rs VolTrack): flags = [HULL | CAPSULE, the CONVEX mesh, world | actor (the frame of the rotation: m_RotAttachType LOCAL = relRot in the world), owner:shape:sockets]; a HULL [x, y, z, qx, qy, qz, qw, sx, sy, sz] = Update 0x140647950 case 6\'s location / rotation / scale (FUN_140649BF0 growth from the window\'s start) in Sora\'s actor space (his yaw 0), a CAPSULE [ax, ay, az, bx, by, bz, r, 0, 0, 0]' });
      }
      if (rows.length === shapes.length) {
        for (const r of rows) add(r);
        tracked += 1;
        continue;
      }
    }
    const values = [];
    let ok = true;
    for (let f = 0; f < frames; f++) {
      const t = frames > 1 ? f * len / (frames - 1) : 0;
      const each = shapes.map((s) => shapeAt(s, logical, clip, t));
      if (each.some((v) => !v)) { ok = false; break; }
      values.push(...mergeShapes(each).map(r1));
    }
    if (!ok) continue;
    const seqs = [...new Set(shapes.map((s) => (weapon(s.part.model) ? weaponAnim(weapon(s.part.model), logical, clip).seq : '-')))];
    add({ id, what: 'track', form, code: frames, logical: g, clip, length: r4(len), flags: [...new Set(shapes.map((s) => s.owner + ':' + s.shape + ':' + s.socket1 + '-' + s.socket2))].concat(seqs), values,
      note: 'per frame [ax, ay, az, bx, by, bz, radius] (cm, actor space, his yaw 0) of group ' + g + ' (' + shapes.length + ' shape(s), merged into one capsule: flags = owner:shape:sockets, then the weapons\' own clips (- = the reference pose))' });
    tracked += 1;
  }
}
// the replacement locomotion (KH3: the Run state keeps the battle stance in forms 0x85 / 0x87 / 0x88 / 0x8B, Run Tick
// 0x1413AED70: `(form - 0x85) < 7 && (0x4D >> (form - 0x85)) & 1`; the B set's rows come from the form's set)
for (const form of ['CL', 'YO', 'SK']) {
  const rs = ROWS_OF[form];
  for (const name of ['RunLoopB', 'RunStartB', 'WalkLoopB', 'IdleB', 'BattleStart', 'BattleEnd']) {
    if (!rs.has(name)) continue;
    const r = rs.get(name);
    const bs = (OV.blend_spaces || []).find ? null : null;
    void bs;
    add({ id: 'loco.' + form + '.' + name, what: 'loco', form, logical: name, clip: r.clip || '-', flags: [r.cls, form === 'SK' ? 'battle_stance' : '-'], ref_addr: '0x1413AED70', note: 'the locomotion row of the form\'s set (a BlendSpace has no table row yet: the Run state and the anim generator must take it, STATE.md); flags[1] battle_stance = KH3 keeps the battle stance in this form (Blizzard Blades: he skates on his blades)' });
  }
}
// the attack rows the forms hit with (p_ex001_AttackData), for the combat sheet's owner
const usedRows = new Set(ROWS.flatMap((r) => (['attack', 'finish', 'pawn'].includes(r.what) ? r.rows : r.what === 'rename' ? [r.clip] : [])).filter((x) => x && x !== 'None'));
for (const extra of ['SCCL_CHANGE', 'CL2_CHANGE', 'SCYO_CHANGE', 'SCSK_CHANGE', 'SCCL_SONIC', 'CL2_SONIC', 'SCYO_DODGE', 'SCCL_DODGE', 'CL2_DODGE', 'SCSK_DODGE']) usedRows.add(extra);
for (const id of [...usedRows].sort()) {
  const a = atkRow(id);
  must(a, 'p_ex001_AttackData has no row ' + id);
  add({ id: 'row.' + id, what: 'row', form: /^CL2_/.test(id) ? 'CL2' : /^SCYO_/.test(id) ? 'YO' : /^SCSK_/.test(id) ? 'SK' : 'CL', logical: id, values: [a.m_Power, a.m_FormPoint, a.m_ReactionPowerValue], flags: [String(a.m_DamageKind).split('::').pop(), String(a.m_DamageAttribute).split('::').pop()], note: 'p_ex001_AttackData row ' + a.id + ': values = [m_Power, m_FormPoint, m_ReactionPowerValue]; the combat sheet sora-combat-attack-data must carry it for the hit to deal it (STATE.md INT line)' });
}

// ------------------------------------------------------------------------------------------------ (AUDIT-B F-CLAWS) (appended last: no row moves) the weapons' and the Chandelier's own clips
// KH3: every form weapon plays its OWN clip of Sora's play (the clip of the same logical name, else of the same sequence:
// kh-er-render keyblade.rs `rig_of`, this tool's `weaponAnim`) and the notifies of THAT clip fire on the weapon
// (SQEX_ATTACH_OBJECT_OWNER = the weapon actor): effects on its sockets / bones, sounds. The Chandelier w_so088 (FINISH_SK's
// style pawn) the same with its 7423. One row per effect / sound notify: `wfx.<form>.<model>.<seq>.<k>` (what 'wfx'); clip =
// SORA's sequence that makes the weapon play it (the pawn: 7423, its own clip); flags = [owner (weapon | pawn), effect |
// sound, state | instant, attach (ToAttach) | place, abs | rel (bAbsoluteRotation), AttachEffectEndType, WhenChangeAnimation
// Type, the socket, the model, the weapon's own sequence]; logical = the effect / sound name (a sound: its /Game path, as Sora's
// sound notifies carry it); windows = [begin-end] (s, Sora's clip time = the weapon's); values = [GroupID, ScaleOffset xyz,
// FadeIn, FadeOut, StartDelayTime, then per frame of Sora's clip x y z pitch yaw roll: the effect's transform (the weapon's
// socket under its clip, on Sora's socket under his clip (the FK of the shape tracks) x LocationOffset / RotationOffset; UE
// cm / FRotator degrees) in Sora's actor space (his yaw 0) / the pawn's actor space (its root at its foot point)]; a sound
// row: [FadeOutDuration, VolumeMultiplier, PitchMultiplier, then per frame x y z 0 0 0 of its AttachPointName]. A socket the model lacks / None = the weapon
// component itself (UE's attach to an unknown socket), on Sora's socket.
const CL_IDENTITY = { q: [0, 0, 0, 1], t: [0, 0, 0], s: [1, 1, 1] };
const clFlat = (props) => { const o = {}; for (const p of props || []) o[p.name] = p.type === 'StructProperty' ? (Array.isArray(p.value) ? clFlat(p.value) : p.value) : p.type === 'ArrayProperty' ? (p.value && p.value.items ? p.value.items.map((it) => (Array.isArray(it) ? clFlat(it) : it)) : p.value) : p.value; return o; };
const clV3 = (o) => (o ? [o.x || 0, o.y || 0, o.z || 0] : [0, 0, 0]);
const clRotator = (q) => { // FQuat::Rotator (UE 4.17)
  const [X, Y, Z, W] = q, st = Z * X - W * Y, yawY = 2 * (W * Z + X * Y), yawX = 1 - 2 * (Y * Y + Z * Z), D = 180 / Math.PI;
  if (st < -0.4999995) { const yw = Math.atan2(yawY, yawX) * D; return [-90, yw, -yw - 2 * Math.atan2(X, W) * D]; }
  if (st > 0.4999995) { const yw = Math.atan2(yawY, yawX) * D; return [90, yw, yw - 2 * Math.atan2(X, W) * D]; }
  return [Math.asin(2 * st) * D, Math.atan2(yawY, yawX) * D, Math.atan2(-2 * (W * X + Y * Z), 1 - 2 * (X * X + Y * Y)) * D];
};
// the component-space transform of a weapon's socket (sockets.json) or bone under its own clip `a`, under `base`; null = unknown
function clModelSocketT(w, a, name, time, base) {
  const ws = w.sockets[name];
  const k = w.bones.findIndex((b) => b.name === (ws ? ws.bone : name));
  if (k < 0) return null;
  let T = under(boneT(w.bones, byBone(a), k, a ? Math.min(time, a.length) : 0, false), base);
  if (ws) T = under({ q: rotQ({ pitch: ws.local_rotation[0], yaw: ws.local_rotation[1], roll: ws.local_rotation[2] }), t: ws.local_location, s: [1, 1, 1] }, T);
  return T;
}
// the weapon part's root on Sora (his socket under his clip), Sora's mesh space
function clPartBase(part, soraSeq, time) {
  const sa = soraAnim(soraSeq); if (!sa) return null;
  const sock = SORA_SOCKETS[part.socket]; if (!sock) return null;
  const hand = SORA_BONES.findIndex((b) => b.name === sock.bone);
  return under(sock.T, boneT(SORA_BONES, byBone(sa), hand, Math.min(time, sa.length), sa.props.bEnableRootMotion === true));
}
const wfxCount = {};
let wfxUnknownSockets = 0;
// (AUDIT-B W-VFX2) `mount` = Sora's socket the weapon stands on (a weapon's / unit's rows): a TypeDataSQEX_Thunder row gets its
// host-resolved anchor `kb:<model>:<mount>:<socket>[:to<EndSocketName>]` as one more flag (flags[10]; tools/lib/wnotify_rows.js
// `thunderAnchor`), so the host finds the beam's ends on the weapon's own mesh; kh-core claws.rs spawns such a row there.
const wnotify = require('./lib/wnotify_rows.js');
const WV2_PACK = P('build', 'sora_pack');
let wv2Anchors = 0;
function clipFxRows(form, ownerKind, w, c, soraSeq, logical, baseAt, frames, length, mount) {
  const { a } = weaponAnim(w, logical, soraSeq);
  const place = (socket, loc, rot, absRot, t) => {
    const base = baseAt(t) || CL_IDENTITY;
    let T = socket === 'None' ? null : clModelSocketT(w, a, socket, t, base);
    if (!T) { if (socket !== 'None' && t === 0) wfxUnknownSockets += 1; T = base; }
    const off = qrot(T.q, [loc[0] * T.s[0], loc[1] * T.s[1], loc[2] * T.s[2]]);
    // (AUDIT-B W-FIN) a pawn's rows in ITS mesh component's frame (PAWN_BP: the Chandelier's mesh = its root, yaw 0), a weapon's
    // in Sora's (his mesh yaw 270); socket None = that component itself (SQEX 0x141765EC0 passes the row's SocketName unchanged to
    // SpawnEmitterAttached 0x14411B7A0 -> 0x144118120 -> SetupAttachment(mesh, NAME_None): the component, not bone 0)
    const p = (ownerKind === 'pawn' ? PAWN_BP.actor : actor)([T.t[0] + off[0], T.t[1] + off[1], T.t[2] + off[2]]);
    const q = absRot ? rotQ(rot) : qmulH(ownerKind === 'pawn' ? PAWN_BP.meshQ : MESH_YAW_Q, qmulH(T.q, rotQ(rot)));
    return [...p.map(r1), ...clRotator(q).map(r1)];
  };
  const track = (socket, loc, rot, absRot) => { const out = []; for (let f = 0; f < frames; f++) out.push(...place(socket, loc, rot, absRot, frames > 1 ? f * length / (frames - 1) : 0)); return out; };
  let k = 0;
  const key = form + '.' + w.model;
  for (const n of c.notifies || []) {
    const p = n.properties || {};
    const begin = +n.trigger_time;
    const end = Number.isFinite(+n.end_trigger_time) && n.end_trigger_time !== null ? +n.end_trigger_time : begin;
    const state = n.kind === 'state';
    const id = 'wfx.' + form + '.' + w.model + '.' + soraSeq + '.' + k;
    if (n.class === 'SQEXSEADAnimNotifyState_AttachSound' || n.class === 'SQEXSEADAnimNotify_PlaySound') {
      if (!p.Sound) continue;
      const sock = String(p.AttachPointName || 'None');
      add({ id, what: 'wfx', form, code: frames, logical: String(p.Sound), clip: soraSeq, length: r4(length), flags: [ownerKind, 'sound', state ? 'state' : 'instant', 'attach', 'rel', '-', '-', sock, w.model, c.sequence], windows: [r4(begin) + '-' + r4(end)], values: [+(p.FadeOutDuration || 0), p.VolumeMultiplier === undefined ? 1 : +p.VolumeMultiplier, p.PitchMultiplier === undefined ? 1 : +p.PitchMultiplier, ...track(sock, [0, 0, 0], { pitch: 0, yaw: 0, roll: 0 }, false)], ref_addr: '-',
        note: '(AUDIT-B F-CLAWS) ' + ownerKind + ' ' + w.model + ' clip ' + c.sequence + ' (Sora\'s ' + soraSeq + '): ' + n.class + ' (values = [FadeOutDuration, VolumeMultiplier, PitchMultiplier, per frame x y z 0 0 0 of its AttachPointName])' });
      k += 1; wfxCount[key] = (wfxCount[key] || 0) + 1;
      continue;
    }
    if (!/AttachEffect$/.test(n.class)) continue;
    for (const m of (p.MetaAttachDataList || {}).items || []) {
      const mo = clFlat(m);
      for (const d of mo.MetaAttachData || []) {
        for (const e of d.EffectDataList || []) {
          if (!e.AttachEffect || e.AttachEffect === 'null' || e.AttachEffect === 0) continue;
          const sock = String(d.SocketName && d.SocketName !== 'None' ? d.SocketName : mo.SocketName || 'None');
          const loc = clV3(d.LocationOffset), rot = d.RotationOffset || { pitch: 0, yaw: 0, roll: 0 }, sc = clV3(d.ScaleOffset || { x: 1, y: 1, z: 1 });
          const endType = String(d.AttachEffectEndType || '-').replace('ESQEX_ATTACH_EFFECT_END_TYPE_', ''), change = String(d.WhenChangeAnimationType || '-').replace('ESQEX_CHANGE_ANIMATION_TYPE_', '');
          // (AUDIT-B W-VFX2) a thunder (beam) row: its kb: anchor (a pawn's has no mount: refused; none is in the data)
          const beam = mount ? wnotify.thunderAnchor(WV2_PACK, String(e.AttachEffect), w.model, mount, sock, d, endType) : '';
          must(mount || !wnotify.thunderEffect(WV2_PACK, String(e.AttachEffect).split('.').pop()), '(AUDIT-B W-VFX2) a thunder effect on a pawn: ' + e.AttachEffect);
          if (beam) wv2Anchors += 1;
          add({ id: 'wfx.' + form + '.' + w.model + '.' + soraSeq + '.' + k, what: 'wfx', form, code: frames, logical: String(e.AttachEffect).split('.').pop(), clip: soraSeq, length: r4(length), flags: [ownerKind, 'effect', state ? 'state' : 'instant', d.ToAttach === false ? 'place' : 'attach', d.bAbsoluteRotation ? 'abs' : 'rel', endType, change, sock, w.model, c.sequence, ...(beam ? [beam] : [])], windows: [r4(begin) + '-' + r4(end)],
            values: [e.GroupID || 0, ...sc, d.FadeIn || 0, d.FadeOut || 0, d.StartDelayTime || 0, ...track(sock, loc, rot, !!d.bAbsoluteRotation)], ref_addr: '-',
            note: '(AUDIT-B F-CLAWS) ' + ownerKind + ' ' + w.model + ' clip ' + c.sequence + ' (Sora\'s ' + soraSeq + '): ' + n.class + ' ' + String(mo.AttachType || '').replace('SQEX_ATTACH_OBJECT_', '') + ' (values = [GroupID, ScaleOffset xyz, FadeIn, FadeOut, StartDelayTime, per frame x y z pitch yaw roll in ' + (ownerKind === 'pawn' ? 'the pawn\'s' : 'Sora\'s') + ' actor space])' });
          k += 1; wfxCount[key] = (wfxCount[key] || 0) + 1;
        }
      }
    }
  }
}
for (const form of ['CL', 'CL2', 'YO', 'SK']) {
  for (const part of partsOf(form)) {
    const w = weapon(part.model);
    if (!w) continue;
    for (const c of w.mj.clips || []) {
      if (!(c.notifies || []).some((n) => /Sound|AttachEffect/.test(n.class))) continue;
      // Sora's sequence that plays it: the form's set row of one of its logical names, else its own sequence
      let soraSeq = null, logical = (c.logical_names || [])[0] || c.sequence;
      for (const l of c.logical_names || []) { const r = ROWS_OF[form].get(l); if (r && r.file && item(r.clip)) { soraSeq = r.clip; logical = l; break; } }
      if (!soraSeq && item(c.sequence)) soraSeq = c.sequence;
      if (!soraSeq) continue;
      const it = item(soraSeq);
      clipFxRows(form, 'weapon', w, c, soraSeq, logical, (t) => clPartBase(part, soraSeq, t), it.num_frames, it.duration_s, part.socket); // (AUDIT-B W-VFX2) + mount
    }
  }
}
{
  const w = weapon('w_so088');
  const c = w.mj.clips.find((x) => x.sequence === '7423');
  const frames = Math.max(2, c.frames || Math.ceil(c.duration_s * 30) + 1);
  clipFxRows('SK', 'pawn', w, c, '7423', 'M_7423', () => CL_IDENTITY, frames, c.duration_s);
}
must((wfxCount['YO.w_so065'] || 0) >= 30 && (wfxCount['SK.w_so088'] || 0) === 19, '(AUDIT-B F-CLAWS) the clips\' effect rows: ' + JSON.stringify(wfxCount));
console.log('(AUDIT-B F-CLAWS) wfx rows ' + JSON.stringify(wfxCount) + ', sockets unknown to their model (the component): ' + wfxUnknownSockets);
// (AUDIT-B F-CLAWS) THE CATCH LINK of FINISH_YO: w_so065's bone X_link01 per frame of M_4420 (StyleFinish slot 149 0x1413E0CD0: the
// caught pawn is attached there, FUN_143E8FC60 with "X_link01", relative location 0 in sub 5 FUN_143EAC880)
{
  const part = partsOf('YO').find((p) => p.model === 'w_so065');
  must(part, 'YO: no w_so065 part');
  const clip = clipOf('YO', 'M_4420'), it = item(clip);
  must(it, 'YO: no M_4420 clip');
  const values = [];
  for (let f = 0; f < it.num_frames; f++) { const t = it.num_frames > 1 ? f * it.duration_s / (it.num_frames - 1) : 0; const T = weaponSocketT(part, 'X_link01', 'M_4420', clip, t); must(T, 'YO 4420: no X_link01'); values.push(...actor(point(T, [0, 0, 0])).map(r1), 0); }
  add({ id: 'track.YO.' + clip + '.X_link01', what: 'track', form: 'YO', code: it.num_frames, logical: 'X_link01', clip, length: r4(it.duration_s), flags: [part.model, 'LINK', 'X_link01', 'None', '-'], values, ref_addr: '0x1413E0CD0', note: '(AUDIT-B F-CLAWS) per frame [x, y, z, 0] (cm, actor space, his yaw 0): the giant yo-yo w_so065\'s X_link01, where FINISH_YO\'s caught pawn (m_pCatchPawn in state 62) is carried' });
}
// (AUDIT-B W-FIN) THE TRANSFORM UNITS' own clips: w_so061 (Happy Gear: C401 StyleChange12, 4401 StyleChange4) and w_so081 (Crystal
// Snow: I401 StyleChange18, 7401 StyleChange7) play their clip of Sora's StyleChange and its AttachEffect / AttachSound notifies
// fire on the unit (OWNER = the unit's mesh on his socket, as the weapons'; F-SHIP's w_so091 the same). Appended after every
// earlier row (no row moves): the same `wfx` rows and player as the weapons'. A unit of a form whose row has none: nothing.
{
  const unitsOf = (form) => (FORMS[form].transform_units || []).map((u) => { const m = u.match(/^(\w+)@(\w+):(\w+)=(.*)$/); must(m, 'transform unit ' + u); return { unit: m[1], hand: m[2], socket: m[3] === 'None' ? 'R_wep_skt' : m[3], blueprint: m[4], model: m[4].split('/').pop().replace(/_[Ww]ep$/, '') }; });
  const before = Object.values(wfxCount).reduce((a, b) => a + b, 0);
  for (const form of ['CL', 'CL2', 'YO', 'SK']) {
    for (const part of unitsOf(form)) {
      const w = weapon(part.model);
      if (!w) continue;
      for (const c of w.mj.clips || []) {
        if (!(c.notifies || []).some((n) => /Sound|AttachEffect/.test(n.class))) continue;
        let soraSeq = null, logical = (c.logical_names || [])[0] || c.sequence;
        for (const l of c.logical_names || []) { const r = ROWS_OF[form].get(l); if (r && r.file && item(r.clip)) { soraSeq = r.clip; logical = l; break; } }
        if (!soraSeq && item(c.sequence)) soraSeq = c.sequence;
        if (!soraSeq) continue;
        // only the unit's clip of THIS form's StyleChange (`transform_clip`; the unit is drawn in that clip only, the other
        // form's clip is that form's row)
        if (!(c.logical_names || []).includes(FORMS[form].transform_clip)) continue;
        const it = item(soraSeq);
        clipFxRows(form, 'weapon', w, c, soraSeq, logical, () => CL_IDENTITY /* (FORMWEP-FIX) a transform unit (socket None) stands on his MESH component (UE NAME_None): its clips are authored in his mesh space */, it.num_frames, it.duration_s, part.socket); // (AUDIT-B W-VFX2) + mount
      }
    }
  }
  const after = Object.values(wfxCount).reduce((a, b) => a + b, 0);
  must(after > before, '(AUDIT-B W-FIN) the transform units gave no rows');
  console.log('(AUDIT-B W-FIN) transform-unit wfx rows: ' + (after - before) + ' ' + JSON.stringify(wfxCount));
}
console.log('(AUDIT-B W-VFX2) thunder rows with their kb: anchor: ' + wv2Anchors);
// (AUDIT-B W-DATA) THE WEAPONS' AttachDataList NOTIFIES: a TresAnimNotify(State)_AttachEffect can name ATTACH DATA ASSETS instead of
// inline rows (SQEX_AttachEffectNotifyDataToDataAsset: AttachDatas, GroupID, AttachType, SocketName, bUseCommonAttachData). Twin
// Yo-Yos' w_so064 / w_so065 magic clips 4321 .. 4340 (MagicFire .. MagicAirFireML) name ead_w_so060_com with GroupID -2. KH3's
// per-list spawn 0x1417719E0: request -2 = every CURRENT group of the attach component (the play's group: FireMagic plays its clip
// with the spell's m_EffectGroup, 0x1413794B0 line 567), any other request group = that group only (FUN_141764560 per entry);
// each asset row (SQEX_StructParticleAttachData) then spawns its entries of that group where ITS row attaches them. ead_w_so060_com
// = "ファイア用_紐" vw_so064_bem_lin00 (TypeDataSQEX_Thunder, BoneList vfx_himo00 .. 16; SocketName vfx_himo00, EndSocketName
// vfx_himo16) and "ファイア用_手元" vw_so064_bem_lin_glw00 on `wep`, each for the groups of Fire .. Aerogan (1301 .. 1403: no Cure
// group, so no string on a Cure). One `wfx` row per asset entry, on the weapon's own mesh (the `kb:` anchor, flags[10]: the host
// poses the weapon under its clip of the logical name and resolves the beam's bone list there); `clip` = Sora's row of the
// clip's logical name (MagicFire4: a blend space; kh-core claws.rs matches the play's own row too), code 0 (no FK track: the
// anchor carries the place). Census of every claws-package weapon / unit clip (and, in AUDIT-B W-DATA's SECTION.md, of every other
// AUDIT-B form's weapons, units and pawns): only these 20 clips carry an AttachDataList.
{
  const unitsOf = (form) => (FORMS[form].transform_units || []).map((u) => { const m = u.match(/^(\w+)@(\w+):(\w+)=(.*)$/); must(m, 'transform unit ' + u); return { socket: m[3] === 'None' ? 'R_wep_skt' : m[3], model: m[4].split('/').pop().replace(/_[Ww]ep$/, '') }; });
  const eadCache = new Map();
  const eadRows = (asset) => {
    if (eadCache.has(asset)) return eadCache.get(asset);
    const file = P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content', asset.replace(/^\/Game\//, '').split('.')[0] + '.uasset.json');
    must(fs.existsSync(file), '(AUDIT-B W-DATA) no attach data asset ' + file);
    const d = readJson(file);
    const ref = (i) => (typeof i === 'number' && i < 0 && d.imports[-i - 1] ? d.imports[-i - 1].objectName : null);
    const pv = (list) => { const o = {}; for (const p of list || []) o[p.name] = p.type === 'StructProperty' ? (Array.isArray(p.value) ? pv(p.value) : p.value) : p.type === 'ArrayProperty' ? (p.value && p.value.items ? p.value.items.map((it) => (Array.isArray(it) ? pv(it) : it)) : p.value) : p.value; return o; };
    const e = d.exports.find((x) => x.class === 'SQEX_ParticleAttachDataAsset');
    must(e, '(AUDIT-B W-DATA) ' + asset + ': no SQEX_ParticleAttachDataAsset export');
    const out = [];
    for (const meta of pv((e.data && e.data.props) || e.props).EffectDataList || []) {
      const m = Array.isArray(meta) ? pv(meta) : meta;
      for (const x of m.EffectDataList || []) {
        const dd = Array.isArray(x) ? pv(x) : x;
        const fx = ref(dd.AttachEffect);
        if (!fx) continue;
        must(!dd.UseAttachObjectData || /OWNER$/.test(String((dd.AttachObjectData || {}).AttachType)), '(AUDIT-B W-DATA) ' + asset + ': an entry attached to another object (' + JSON.stringify(dd.AttachObjectData) + ')');
        must(!m.bUseCommonEffect, '(AUDIT-B W-DATA) ' + asset + ': a common-effect row');
        out.push({ m, group: dd.GroupID, fx });
      }
    }
    eadCache.set(asset, out);
    return out;
  };
  let n = 0;
  const per = {};
  for (const form of ['CL', 'CL2', 'YO', 'SK']) {
    for (const part of [...partsOf(form).map((p) => ({ socket: p.socket, model: p.model })), ...unitsOf(form)]) {
      const w = weapon(part.model);
      if (!w) continue;
      for (const c of w.mj.clips || []) {
        const lists = (c.notifies || []).filter((x) => /AttachEffect$/.test(x.class) && x.properties && x.properties.AttachDataList);
        if (!lists.length) continue;
        // Sora's row of the clip's logical name in this form (a blend space too: MagicFire -> MagicFire4), else none (not played here)
        const logical = (c.logical_names || []).find((l) => ROWS_OF[form].has(l));
        if (!logical) continue;
        const soraClip = ROWS_OF[form].get(logical).clip;
        let k = 0;
        for (const x of lists) {
          const state = x.kind === 'state';
          const begin = +x.trigger_time, end = state ? +x.end_trigger_time : begin;
          for (const it of x.properties.AttachDataList.items || []) {
            const o = Object.fromEntries(it.map((p) => [p.name, p.value]));
            must(!o.bUseCommonAttachData, '(AUDIT-B W-DATA) ' + part.model + ' ' + c.sequence + ': bUseCommonAttachData (the owner\'s DesignatedAttachDataList) is not read here');
            must(/OWNER$/.test(String(o.AttachType)), '(AUDIT-B W-DATA) ' + part.model + ' ' + c.sequence + ': attach type ' + o.AttachType);
            for (const asset of (o.AttachDatas || []).filter((a) => typeof a === 'string')) {
              for (const r of eadRows(asset)) {
                // a request group other than -2 takes only its own group's entries (and -1 ones), whatever the play's group
                if (o.GroupID !== -2 && !(r.group === -1 || o.GroupID === -1 || r.group === o.GroupID)) continue;
                const m = r.m;
                const sock = String(m.SocketName && m.SocketName !== 'None' ? m.SocketName : o.SocketName || 'None');
                const endType = String(m.AttachEffectEndType || '-').replace('ESQEX_ATTACH_EFFECT_END_TYPE_', ''), change = String(m.WhenChangeAnimationType || '-').replace('ESQEX_CHANGE_ANIMATION_TYPE_', '');
                const lo = m.LocationOffset || {}, ro = m.RotationOffset || {}, sc = m.ScaleOffset || { x: 1, y: 1, z: 1 };
                must([lo.x, lo.y, lo.z, ro.pitch, ro.yaw, ro.roll].every((v) => Math.abs(+v || 0) < 1e-6), '(AUDIT-B W-DATA) ' + asset + ' ' + r.fx + ': an offset row (the kb: anchor of a wfx row carries none)');
                const beam = wnotify.thunderAnchor(WV2_PACK, r.fx, part.model, part.socket, sock, m, endType);
                const anchor = beam || 'kb:' + part.model + ':' + part.socket + ':' + (sock === 'None' ? '' : sock) + (m.bAbsoluteRotation ? ':abs' : '') + (m.ToAttach === false ? ':free' : '') + (endType === 'FADEOUT' ? ':fo' + r4(m.FadeOut || 0) : '');
                // the request group decides for -1 / a named group: a -2 request passes the entry's own GroupID to the play's group rule
                const group = o.GroupID === -2 ? r.group : r.group === -1 ? -1 : o.GroupID;
                add({ id: 'wfx.' + form + '.' + part.model + '.' + soraClip + '.ead' + k, what: 'wfx', form, code: 0, logical: r.fx, clip: soraClip, length: r4(c.duration_s || 0),
                  flags: ['weapon', 'effect', state ? 'state' : 'instant', m.ToAttach === false ? 'place' : 'attach', m.bAbsoluteRotation ? 'abs' : 'rel', endType, change, sock, part.model, c.sequence, anchor], windows: [r4(begin) + '-' + r4(end)],
                  values: [group, +(sc.x ?? 1), +(sc.y ?? 1), +(sc.z ?? 1), +(m.FadeIn || 0), +(m.FadeOut || 0), +(m.StartDelayTime || 0)], ref_addr: '0x1417719E0', ref_conf: 'confirmed',
                  note: '(AUDIT-B W-DATA) weapon ' + part.model + ' clip ' + c.sequence + ' (' + logical + '; Sora\'s ' + soraClip + '): its AttachDataList (' + asset.split('.').pop() + ', request group ' + o.GroupID + '), the asset row "' + ((m.Description && m.Description.s) || '') + '" entry of group ' + r.group + ' (values = [GroupID, ScaleOffset xyz, FadeIn, FadeOut, StartDelayTime]; no FK track: flags[10] is the kb: anchor)' });
                k += 1; n += 1; per[part.model] = (per[part.model] || 0) + 1;
              }
            }
          }
        }
      }
    }
  }
  must((per.w_so064 || 0) === 10 * 40 && (per.w_so065 || 0) === 10 * 40, '(AUDIT-B W-DATA) the yo-yos\' magic clips: expected 10 clips x 2 asset rows x 20 groups each, got ' + JSON.stringify(per));
  console.log('(AUDIT-B W-DATA) AttachDataList rows (ead assets of the weapons\' own clips): ' + n + ' ' + JSON.stringify(per));
}
// ------------------------------------------------------------------------------------------------ checks
must(ROWS.filter((r) => r.what === 'attack').length === 8 + 8 + 10 + 11, 'expected 8 claw kinds per claw form, 10 yo-yo kinds, 11 blade kinds');
for (const r of ROWS.filter((x) => x.what === 'attack')) must(r.clip !== '-' && r.length > 0, r.id + ': no clip');
for (const form of ['CL', 'CL2', 'YO', 'SK']) must(ROWS.some((r) => r.what === 'track' && r.form === form), form + ': no shape track (the FK found none)');

// ------------------------------------------------------------------------------------------------ sheet
const S = (n, t, g, doc) => [n, t, g, doc];
const COLS = [
  S('id', 'str', 'str', '<what>.<form>.<key>'), S('what', 'enum:attack|anim|finish|pawn|rename|shape|track|vol|loco|row|wfx', 'str', 'the row kind'), S('form', 'str', 'str', 'CL (Agile Claws 0x8A), CL2 (Blizzard Claws 0x90), YO (Twin Yo-Yos 0x82), SK (Blizzard Blades 0x85)'),
  S('code', 'int', 'i32', 'attack kind (attack), finish sub-step (finish), frames (track), else 0'), S('logical', 'str', 'str', 'the logical anim name KH3 plays (attack / anim / finish / loco), the group (shape / track), the old row (rename), the row (row)'),
  S('clip', 'str', 'str', 'the animation table row (attack / finish / track: the sequence of a form-set clip, a logical name of the base / XS rows); the new row (rename); the owner (shape); the model (pawn)'), S('length', 'num', 'f32', 'the clip length, s'),
  S('group', 'int', 'i32', 'the effect group the play passes (PlayAttack +0x198)'), S('root_len', 'num', 'f32', 'root length (+0x1D8), 0 = none'), S('stop_dist', 'num', 'f32', 'stop distance (+0x24C)'), S('z_gain', 'num', 'f32', 'z gain (+0x14C)'),
  S('air_mode', 'bool', 'bool', '+0x1DF'), S('chan_xy', 'bool', 'bool', '+0x234 bit 0'), S('chan_root', 'bool', 'bool', '+0x234 bit 1'),
  S('flags', 'list', 'str[]', 'the case\'s other writes / the kind\'s strings (see the note)'), S('windows', 'list', 'str[]', 'the clip\'s hit windows start-end@group@row@interval, s (CL2: renamed rows)'), S('rows', 'list', 'str[]', 'the attack rows of those windows'),
  S('effects', 'list', 'str[]', 'the clip\'s effects of that group (time@asset)'), S('sounds', 'list', 'str[]', 'the clip\'s sounds / voices (time@asset)'), S('values', 'list', 'f32[]', 'numbers by kind (see the note)'),
  S('ref_addr', 'str', 'str', 'decompiled function'), S('ref_conf', 'enum:confirmed|inferred|guess', 'str', 'confirmed = read in the code or the data; inferred; guess'), S('note', 'str', 'str', '-'),
];
const NAME = 'sora-form-claws';
const rows = ROWS.map((r) => COLS.map((c) => r[c[0]]));
for (const [i, r] of rows.entries()) for (const [k, v] of r.entries()) must(v !== undefined && v !== null && !(typeof v === 'number' && !Number.isFinite(v)), 'row ' + ROWS[i].id + ': cell ' + COLS[k][0] + ' is ' + v);
const ids = new Set(); for (const r of ROWS) { must(!ids.has(r.id), 'duplicate id ' + r.id); ids.add(r.id); }
fs.writeFileSync(P('sheets', 'port', NAME + '.json'), '{"sheet":"' + NAME + '","group":"port","columns":' + JSON.stringify(COLS.map((c) => c[0])) + ',"rows":[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
const by = {}; for (const r of ROWS) by[r.what] = (by[r.what] || 0) + 1;
console.log('sheets/port/' + NAME + '.json: ' + rows.length + ' rows ' + JSON.stringify(by) + ', ' + tracked + ' shape tracks');

// ------------------------------------------------------------------------------------------------ register (this tool's entry only; replaced in place)
const q = (s) => "'" + String(s).replace(/\\/g, '\\\\').replace(/'/g, "\\'") + "'";
const NL = '\n';
const DOC = 'KH3\'s Agile Claws / Twin Yo-Yos / Blizzard Claws / Blizzard Blades as data for their form package (kh-core combat/forms/claws.rs, states/combo_cl.rs / combo_yo.rs / combo_sk.rs): the attack kinds, the replacement anim sets, FINISH_YO / FINISH_SK, the CL2 row rename, the weapon shapes and their per-frame tracks.';
const MARK = '(weapons F-CLAWS)';
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
  const text = '  // ' + MARK + ' ' + DOC + ' Written by tools/sora_form_claws.js (documentation/KH3_WEAPONS_PLAN.md 1.3)' + NL +
    '  ' + q(NAME) + ": { group: 'port', file: " + q(NAME + '.json') + ", by: 'tools/sora_form_claws.js', from: " + q('sora-keyblade-forms, the form anim sets of the asset pack, the form weapons (blueprints, skeletons, clips), Sora\'s skeleton, p_ex001_AttackData and the decompiled PlayAttack / ComboCL / ComboYO / ComboSK / StyleFinish / StyleFinishSK') + ', external: true, cols: [' + NL +
    '    ' + COLS.map((c, i) => '[' + q(c[0]) + ', ' + (i === 0 ? 'K' : 'V') + ', ' + q(c[1]) + ", '-', " + q(c[3]) + (['note', 'flags', 'windows', 'rows', 'effects', 'sounds', 'values', 'logical', 'clip'].includes(c[0]) ? ', true' : '') + ']').join(', ') + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/sheet_defs.js');
}
{
  const f = P('tools', 'lib', 'port_gen_defs.js'); const before = fs.readFileSync(f, 'utf8');
  const END = NL + '    },' + NL;
  const pos = (t) => { const anchor = '  tables: {' + NL; const at = t.indexOf(anchor); must(at >= 0, 'port_gen_defs.js: anchor tables'); return at + anchor.length; };
  const cols = COLS.filter((c) => c[2]).map((c) => c[0] + ': { type: ' + q(c[2]) + (c[3] && c[3] !== '-' ? ', doc: ' + q(c[3]) : '') + ' }').join(', ');
  const text = '    // ' + MARK + ' ' + NAME + ': written by tools/sora_form_claws.js (documentation/KH3_WEAPONS_PLAN.md 1.3)' + NL +
    '    ' + q(NAME) + ': {' + NL +
    "      file: 'sora_form_claws.rs', row: 'FormClawsRow', table: 'FORM_CLAWS', key: 'id', key_prefix: 'FCL'," + NL +
    '      doc: ' + q(DOC) + ',' + NL +
    '      columns: { ' + cols + ' },' + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/port_gen_defs.js');
}
console.log('sora_form_claws ' + VERSION);
