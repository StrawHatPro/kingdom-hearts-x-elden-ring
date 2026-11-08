// (weapons F-STAFF) KH3's Mirage Staff (form.WA, CHANGE_WA 0x88, Ever After) as DATA for its form package (kh-core
// combat/forms/staff.rs, states/combo_wa.rs): the attack kinds of COMBO_WA (0x80 .. 0x87), the replacement anim set
// (AnimSet10: logical name -> clip), the mirror images (TresProj_WandAvator, the controller's m_AvaterProjList), the
// projectiles of w_so070_ProjSet (WA_BEAM / WA_FINISH / WA_BRAID / WA_AVATER), the stick steering of the combo, MaxCombo /
// Finishers, FINISH_WA (the tower TresStylePawnWATower, w_so073) and StartStyle's tail of 0x88.
// documentation/KH3_WEAPONS_PLAN.md 1.3 / 4 (package F-STAFF), build/sora_tmp/weapons/F-STAFF/STATE.md.
//
//   node tools/sora_form_staff.js   write sheets/port/sora-form-staff.json and register it (once, in place) in
//                                   tools/lib/sheet_defs.js and tools/lib/port_gen_defs.js; then node tools/gen_port.js
//
// Inputs (read only): sheets/port/sora-keyblade-forms.json (P3: the form row), build/sora_pack/keyblades/
// sora_forms_manifest.json (the asset agent's pack of AnimSet10: logical rows, clips, notifies), build/sora_pack/keyblades/
// w_so073/model.json (the tower's own clip), the blueprints (Projectile/WA/*, StylePawn/w_so073: movement, life, hit shapes,
// effects), sheets/entities/DataTable/Player/Attack/p_ex001_AttackData.json, and THE CODE TABLES BELOW, read in the
// decompiled exe (re/decomp; vtable 0x146913968 of TresPlayerState_ComboWA, id 83, read from the exe image):
//   CLASS    pawn slot 1337 0x141432F00: form 0x88 -> state 0x53 (COMBO_WA). Overrides: slot 95 Reset 0x141352EC0 (Attack's,
//            then +0x234 |= 0x400000: the homing target is taken again from the controller when it is lost, slot 130
//            0x14134D020), 96 Begin 0x14134F4D0, 122 Decide 0x1413642B0, 129 0x14041F340 (return 0: no weapon recoil),
//            131 0x141355520 (the stick steering), 132 0x14134E420, 133 0x1413527E0, anim-interface slot 212 (vt+0x228, the
//            shot handler) 0x141359700. PlayAttack (slot 120 0x1413378B0) is the Attack class's: cases 0x80 .. 0x87 below.
//   MAX      MaxCombo (pawn slot 928 0x141433570) / Finishers (slot 1253 0x141433A60), case 0x88: controller+0xB90 (the
//            mirror images are out: ctrl vt+0x1B28 0x14128B950) ? 10 / 1 : 5 / 2.
//   AVATAR   controller slot 868 0x14140B920 (mode 0 spawn WA_AVATER, newest first, at most 4; 1 every image fires a beam,
//            each 0.033333 s after the one before; 2 every image fires the finish; 3 all gone = slot 845 0x1413FA210),
//            slot 843 0x141419BC0 (+0xB90 = 1, +0xB94 = 0), the controller tick 0x1413F7030 (+0xB94 += dt; past 10 s
//            they go), EndStyle / StartStyle call slot 845; TresProj_WandAvator tick 0x141484720, command 0x141481CE0,
//            target 0x141481DB0, the image's shot 0x14147E8E0 (from its root + (125, 0, 168) in its frame).
//            Spawned by TryAirSlide 0x141436D20 (weapon form 10 in battle: the dodge, flag 2, and slot 868(0)); the gate
//            0x141425780 lets weapon form 10 through without the stick and on the ground in battle.
//   SHOT     0x141359700: kind 0x80 WA_BEAM, 0x81 WA_FINISH, 0x86 WA_BRAID (0x37 the Attack class's); the images fire
//            with it (mode 1 for 0x80, 2 for the others); from the weapon's muzzle (0x86: Sora's root + 20 up, flat), at
//            the homing target (else 1000 ahead); pawn+0xE28 = 1; 0x86 also counts the combo (slot 115 "SCWA_BRAID").
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
const WA = sheet('sora-keyblade-forms').find((r) => r.id === 'form.WA');
must(WA, 'sora-keyblade-forms has no form.WA');
const OV = readJson(P('build', 'sora_pack', 'keyblades', 'sora_forms_manifest.json'));
const MAIN = readJson(P('build', 'sora_pack', 'manifest.json'));
const ATK = readJson(P('sheets', 'entities', 'DataTable', 'Player', 'Attack', 'p_ex001_AttackData.json')).rows;
const setName = (a) => String(a).split('/').pop().split('.')[0];
const SETS = WA.anim_sets.map(setName);
must(SETS.length === 1 && SETS[0] === 'p_ex001_AnimSet10', 'form.WA: expected the one anim set p_ex001_AnimSet10, not ' + SETS);
const ITEMS = new Map([...MAIN.items, ...OV.items].filter((i) => i.kind === 'animation').map((i) => [i.animation_name, i]));
const BASE = new Map(MAIN.logical_animations.map((a) => [a.name, a]));
const SET_ROWS = new Map();
for (const a of OV.logical_animations.filter((x) => setName(x.anim_set) === SETS[0])) SET_ROWS.set(a.name, { clip: a.clip, file: a.file, cls: a.class });
function clipOf(logical) {
  const r = SET_ROWS.get(logical);
  if (r) return r.file ? r.clip : '-';
  const b = BASE.get(logical);
  return b && b.file ? path.basename(b.file, '.glb') : '-';
}
const item = (clip) => ITEMS.get(clip) || null;
const prop = (n, key) => { const p = n.properties || {}; const v = p[key]; return v && typeof v === 'object' && 'value' in v ? v.value : v; };
function notes(clip) {
  const it = item(clip);
  const out = { length: it ? r4(it.duration_s) : 0, windows: [], rows: [], change: -1, input: -1, shot: -1, effects: [], sounds: [] };
  if (!it) return out;
  for (const n of it.notifies || []) {
    if (n.class === 'TresAnimNotifyState_Attack') { out.windows.push(r4(n.time) + '-' + r4(n.end_time)); out.rows.push(String(prop(n, 'm_AttackDataIDName'))); }
    if (n.class === 'TresAnimNotify_EnableChange' && out.change < 0) out.change = r4(n.time);
    if (n.class === 'TresAnimNotify_EnableInput' && out.input < 0) out.input = r4(n.time);
    if (n.class === 'TresAnimNotify_Shot' && out.shot < 0) out.shot = r4(n.time);
    if (/AttachEffect/.test(n.class)) {
      const m = JSON.stringify(n.properties || {}).match(/"AttachEffect","type":"ObjectProperty","value":"([^"]*)"/);
      if (m && m[1] !== 'null') out.effects.push(r4(n.time) + '@' + m[1].split('.').pop());
    }
    if (/PlaySound|AttachSound|PlayVoice/.test(n.class)) { const s = JSON.stringify(n.properties || {}); const m = s.match(/\/Game\/Sound[^"]*/); out.sounds.push(r4(n.time) + '@' + (m ? m[0].split('.').pop() : n.class.replace(/^.*Notify(State)?_/, ''))); }
  }
  out.effects = [...new Set(out.effects)];
  return out;
}
const atkRow = (id) => ATK.find((r) => r._row_name === id || String(r.id).toLowerCase() === String(id).toLowerCase());
const PKG = P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content', 'Blueprints');
const flat = (props) => { const o = {}; for (const p of props || []) o[p.name] = p.type === 'StructProperty' ? (Array.isArray(p.value) ? flat(p.value) : p.value) : p.type === 'ArrayProperty' ? (p.value && p.value.items ? p.value.items.map((it) => (Array.isArray(it) ? flat(it) : it)) : p.value) : p.value; return o; };
function blueprint(rel) {
  const d = readJson(path.join(PKG, rel + '.uasset.json'));
  const imp = (i) => (i < 0 && d.imports[-i - 1] ? d.imports[-i - 1].objectName : null);
  const byClass = (c) => d.exports.filter((e) => e.class === c).map((e) => flat((e.data && e.data.props) || []));
  const cdo = d.exports.find((e) => /^Default__/.test(e.objectName));
  return { d, imp, byClass, cdo: flat((cdo && cdo.data && cdo.data.props) || []), particle: (byClass('ParticleSystemComponent')[0] || {}).Template };
}
const v3 = (o) => (o ? [o.x || 0, o.y || 0, o.z || 0] : [0, 0, 0]);

// ------------------------------------------------------------------------------------------------ the code tables
// PLAY: PlayAttack 0x1413378B0, cases 0x80 .. 0x87 (decompiled C). The defaults before the case: m_bFaceAt, +0x234 = (x &
// ~6) | 1 (the XY channel only), stop distance 160, z gain 3.0, the channel phases 0, +0x1DF (air mode) 0, the play's group
// +0x198 = 0. LAB_14133A868 = +0x1F4 |= 0x10 (keep facing) and +0x1DF = 1 (air mode: +0x234 |= 4 | 0x10, GravityScale 0.1,
// gravity off, Flying, +0x18C = 0.6); +0x16C = 1: the stick steering (slot 131) with MaxWalkSpeed = 0.9 x the class
// default and MaxAcceleration 3672 (the tail at 0x14133A8F7).
const PLAY = [
  { kind: 0x80, logical: 'M_A411', z_gain: 1, flags: ['keep_face', 'air', 'beam_repeat'], label: 'the beam (the images are out): combo 0 or another clip playing -> M_A411; M_A411 playing past 0.5 s -> M_A411 again from 0.333333 with m_bEnableInput (anim vt+0xF0); before 0.5 s -> nothing is played (the clip runs on); Begin 0x14134F4D0 then fires at once past 0.3 s', at: '0x1413378B0' },
  { kind: 0x81, logical: 'M_A412', z_gain: 1, flags: ['keep_face', 'air'], label: 'the beam finisher (combo 9 with the images out)', at: '0x1413378B0' },
  { kind: 0x82, logical: 'M_A414', z_gain: 1, flags: ['keep_face', 'air', 'steer'], label: 'combo: the front swing (SCWA_FRONT)', at: '0x1413378B0' },
  { kind: 0x83, logical: 'M_A415', flags: ['keep_face', 'air', 'steer'], label: 'combo: the break (SCWA_BREAK)', at: '0x1413378B0' },
  { kind: 0x84, logical: 'M_A418', flags: ['keep_face', 'steer', 'no_vz', 'keep_dir_z', 'dive', 'air', 'dir_z'], dir_z: -4000, label: 'combo, the target well below: the dive (SCWA_DOWN); LAB_1413393CF: +0x234 |= 8, +0x23C |= 0x10, +0x1DE / +0x1DF = 1, DirZ = -4000', at: '0x1413378B0' },
  { kind: 0x85, logical: 'M_A419', flags: ['keep_face', 'air'], label: 'combo, the target well above (or the step after 0x82 / 0x83): the rising spin (SCWA_BRITZ)', at: '0x1413378B0' },
  { kind: 0x86, logical: 'M_A413', z_gain: 1, flags: ['keep_face', 'air', 'steer', 'finish'], label: 'finisher 1: the braid (the WA_BRAID shot)', at: '0x1413378B0' },
  { kind: 0x87, logical: 'M_A416', flags: ['keep_face', 'air', 'finish', 'finish_end'], label: 'finisher 2, the combo\'s end (SCWA_BEHIND / SCWA_BEHIND2)', at: '0x1413378B0' },
];
// STEER: slot 131 0x141355520 (decompiled C; the Attack class's 0x141341870 decays by 0.5, ComboWA's by 0.98).
const STEER = { decay: 0.98, ramp: 7.5, turn: 6.0, stick_min: 0.5, walk_scale: 0.9, accel: 3672.0 };
// MAX: MaxCombo / Finishers, form 0x88 (images out / not).
const MAX = { combo_images: 10, finishers_images: 1, combo: 5, finishers: 2 };
// AVATAR: the controller (0x14140B920, 0x1413F7030) and TresProj_WandAvator (0x141484720 ...).
const AVATAR = { max: 4, life: 10.0, beam_step: 0.033333, finish_wait: 0.5, fire_lock: 0.5, turn_idle: 12.0, turn_busy: 30.0, muzzle: [125, 0, 168] };
// FINISH: StyleFinishWA Tick 0x1413D7670 (Begin 0x1413CC360: StyleFinish's + pawn 0x1405D4D90(1, 0); End 0x1413CF310:
// 0x1413CFE90 the tower gone, then the class's end event).
const FINISH = [
  { sub: 0, logical: 'M_A417', what: 'the tower: the accompany pawn of FINISH_WA (0xA2: w_so073, TresStylePawnWATower) spawned at Sora\'s root with his yaw; shown (vt+0xD98(1)); the tower and Sora play M_A417 (mode 2); +0x164 = 0; the clock +0x2E8 = 0; pawn vt+0x1038(0, 0)', flags: ['spawn_tower'] },
  { sub: 1, logical: '-', what: 'Sora attached to the tower\'s socket X_link01_skt (pawn vt+0x23A8)', flags: ['attach'] },
  { sub: 2, logical: '-', what: 'once the tower runs (tower +0xB48): the world slow off (0x14140E500(ctrl, 0))', flags: ['world_slow_off'] },
  { sub: 5, logical: '-', what: 'while the tower\'s time (vt+0x2620 = m_Param_MaxTime) is not out, no TresEnableChange and M_A417 has not ended', flags: [] },
  { sub: 6, logical: 'M_1501', what: 'the tower gone, Sora detached (0x1413CFE90); Sora M_1501 (the landing)', flags: ['cleanup'] },
  { sub: 7, logical: '-', what: 'one tick', flags: [] },
  { sub: 8, logical: '-', what: 'at M_1501\'s end: DoFinish (vt+0x270)', flags: ['end'] },
];

// ------------------------------------------------------------------------------------------------ rows
const ROWS = [];
const blank = { code: 0, logical: '-', clip: '-', length: 0, z_gain: 0, stop_dist: 0, air_mode: false, flags: [], windows: [], rows: [], effects: [], sounds: [], values: [], ref_addr: '-', ref_conf: 'confirmed', note: '-' };
const add = (r) => ROWS.push({ ...blank, ...r });
for (const k of PLAY) {
  const clip = clipOf(k.logical);
  must(clip !== '-', k.logical + ': no clip in AnimSet10');
  const n = notes(clip);
  for (const r of n.rows) must(atkRow(r), k.logical + ': p_ex001_AttackData has no row ' + r);
  add({ id: 'attack.' + hex(k.kind), what: 'attack', code: k.kind, logical: k.logical, clip, length: n.length, z_gain: k.z_gain || 3, stop_dist: 160, air_mode: k.flags.includes('air'), flags: k.flags,
    windows: n.windows, rows: n.rows, effects: n.effects, sounds: n.sounds, values: [n.change, n.input, n.shot, k.dir_z || 0], ref_addr: k.at, note: k.label + '; values = [TresEnableChange, TresEnableInput, TresAnimNotify_Shot of the clip (-1 none), DirZ]' });
}
for (const [logical, r] of [...SET_ROWS].sort((a, b) => (a[0] < b[0] ? -1 : 1))) {
  add({ id: 'anim.' + logical, what: 'anim', logical, clip: r.file ? r.clip : '-', length: r.file && item(r.clip) ? r4(item(r.clip).duration_s) : 0, flags: [SETS[0], r.cls], note: r.file ? '-' : 'a ' + r.cls + ' of the set: not a row of the animation table (its samples are)' });
}
add({ id: 'steer', what: 'steer', values: [STEER.decay, STEER.ramp, STEER.turn, STEER.stick_min, STEER.walk_scale, STEER.accel], ref_addr: '0x141355520', note: 'the stick steering of the kinds with +0x16C (slot 131): values = [the speed scale +0x174 x this per tick without the stick (or below the magnitude), the scale += dt x this with it (max 1), the yaw +0x17C turns by min(dt x this, 1) of the way to the stick yaw, the stick magnitude below which it decays, MaxWalkSpeed = this x the class default at the play, MaxAcceleration at the play]; the speed is MaxWalkSpeed (decay) / MySprintSpeed (with the stick) x the scale' });
add({ id: 'max', what: 'max', code: 0x88, values: [MAX.combo_images, MAX.finishers_images, MAX.combo, MAX.finishers], ref_addr: '0x141433570', note: 'values = [MaxCombo with the images out (controller+0xB90), Finishers with them, MaxCombo without, Finishers without] (pawn slots 928 / 1253, case 0x88)' });
add({ id: 'avatar', what: 'avatar', values: [AVATAR.max, AVATAR.life, AVATAR.beam_step, AVATAR.finish_wait, AVATAR.fire_lock, AVATAR.turn_idle, AVATAR.turn_busy, ...AVATAR.muzzle], ref_addr: '0x14140B920', note: 'the mirror images: values = [at most this many (the 5th oldest goes), their time from the latest spawn s (controller tick 0x1413F7030), the delay of each further image\'s beam s, the wait after a finish before all go s (state 4), +0x80C at a command, the turn rate idle / busy (vt+0xDB0 x dt), the muzzle in the image\'s frame cm (0x14147E8E0: (125, 0, 168))]' });
// the projectiles of w_so070_ProjSet
const PROJ = { WA_BEAM: 'Projectile/WA/TresProjWA_Beam', WA_FINISH: 'Projectile/WA/TresProjWA_Beam2', WA_BRAID: 'Projectile/WA/TresProjWA_Braid', WA_AVATER: 'Projectile/WA/TresProjWA_Avater' };
for (const p of WA.projectiles) { const [name, bp] = p.split('='); if (PROJ[name]) must(bp.includes(PROJ[name].split('/').pop()), name + ': ' + bp); }
// TresProjectileMovementComponent ctor 0x141470D50: InitialSpeed = MaxSpeed = 2000, homing off (tools/party_sheet.js MOVE_NATIVE)
const MOVE_NATIVE = { InitialSpeed: 2000, MaxSpeed: 2000, m_Accel: 0, m_HomingStartDelay: 0, m_HomingMinDist: 0, m_HomingTurnMax: 0, m_HomingTurnAccel: 0, m_HomingMaxTurnMax: 0 };
for (const [name, rel] of Object.entries(PROJ)) {
  const b = blueprint(rel);
  const mv = { ...MOVE_NATIVE, ...(b.byClass('TresProjectileMovementComponent')[0] || {}) };
  const shapes = (b.byClass('TresAtkCollComponent')[0] || {}).CollisionShapesSrc || [];
  const s = shapes[0] || {};
  const fire = (b.cdo.MyFireEffects || []).map((u) => b.imp(u.MyParticleSystem)).filter(Boolean);
  const effects = [b.imp(b.particle), ...fire.map((f) => 'fire@' + f)].filter(Boolean);
  const hits = b.d.imports.map((x) => x.objectName).filter((x) => /_hit\d|_wht\d/.test(x) && !/^\//.test(x));
  add({ id: 'proj.' + name, what: 'proj', logical: name, clip: rel.split('/').pop() + '_C', flags: [String(s.m_ShapeType || 'NONE').split('::').pop(), b.cdo.m_bNoDestroyOnHit ? 'no_destroy_on_hit' : 'destroy_on_hit', b.cdo.m_bAttachToHitActor ? 'attach_to_hit' : '-', mv.m_bDisableMovementProc ? 'no_move' : 'move'],
    rows: [s.m_DefaultAttackDataIDName || 'None'], effects: [...effects, ...hits.map((h) => 'hit@' + h)],
    values: [b.cdo.m_MainLife || 0, b.cdo.m_MaxLifeDistance || 0, mv.InitialSpeed, mv.MaxSpeed, mv.m_HomingStartDelay, mv.m_HomingMinDist, mv.m_HomingTurnMax, mv.m_HomingTurnAccel, mv.m_HomingMaxTurnMax, ...v3(s.m_Size).filter((_, i) => i !== 1), ...v3(s.m_RelativeLocation1)].map((x) => r4(x)),
    ref_addr: name === 'WA_AVATER' ? '0x141484720' : '0x141359700', note: 'the blueprint ' + rel + ': values = [m_MainLife s (0 = none), m_MaxLifeDistance cm, InitialSpeed, MaxSpeed cm/s, m_HomingStartDelay s, m_HomingMinDist cm, m_HomingTurnMax, m_HomingTurnAccel, m_HomingMaxTurnMax deg/s, the shape size x (radius), z (half length), its relative location x, y, z]; homing on with the shot\'s target (vt+0x1220) or toward a point (vt+0x1210); effects = [its particle, fire@ its fire effects, hit@ its hit effects]; rows = the shape\'s attack row' });
}
// the tower (FINISH_WA's accompany pawn) and its own clip
const TB = blueprint('StylePawn/w_so073');
const TM = readJson(P('build', 'sora_pack', 'keyblades', 'w_so073', 'model.json'));
const TC = Object.values(TM.clips).find((c) => c.sequence === 'A417');
must(TC, 'w_so073 has no A417');
const twin = [], trow = [];
for (const n of TC.notifies || []) if (n.class === 'TresAnimNotifyState_Attack') { twin.push(r4(n.trigger_time) + '-' + r4(n.end_trigger_time) + '@' + (n.properties.AttackInterval || 0)); trow.push(n.properties.m_AttackDataIDName + '@' + n.properties.CollisionGrpName); }
add({ id: 'pawn.w_so073', what: 'pawn', logical: '/Game/Blueprints/StylePawn/w_so073', clip: 'A417', length: r4(TC.duration_s), windows: twin, rows: trow, values: [TB.cdo.m_Param_MaxTime || 0], ref_addr: '0x1413D7670', note: 'the tower (TresStylePawnWATower): its M_A417 (clip A417), invincible; windows = its hit windows a-b@interval; rows = row@group; values = [m_Param_MaxTime s (vt+0x2620)]' });
for (const s of (TB.byClass('TresAtkCollComponent')[0] || {}).CollisionShapesSrc || []) {
  add({ id: 'shape.w_so073.' + s.m_GrpName, what: 'shape', logical: s.m_GrpName, clip: 'w_so073', flags: [String(s.m_ShapeType).split('::').pop(), s.m_SocketName1 || 'None'], values: [...v3(s.m_Size), ...v3(s.m_RelativeLocation1), ...v3(s.m_IncSize), ...v3(s.m_IncMaxSize), s.m_RelativeRocation ? s.m_RelativeRocation.roll : 0].map(r4), rows: [s.m_DefaultAttackDataIDName || 'None'], ref_addr: '-', note: 'TresAtkColl0 CollisionShapesSrc of StylePawn/w_so073: flags = [shape, socket 1], values = [size x, y, z, location 1 x, y, z, m_IncSize x, y, z, m_IncMaxSize x, y, z, roll]' });
}
for (const f of FINISH) {
  const clip = f.logical === '-' ? '-' : clipOf(f.logical);
  const n = clip !== '-' ? notes(clip) : null;
  add({ id: 'finish.' + f.sub, what: 'finish', code: f.sub, logical: f.logical, clip, length: n ? n.length : 0, flags: f.flags, windows: n ? n.windows : [], effects: n ? n.effects : [], sounds: n ? n.sounds : [], values: n ? [n.change, n.input] : [], ref_addr: '0x1413D7670', note: f.what });
}
// StartStyle 0x14141A9D0 case 0x88 (the row's start_extra) and EndStyle
add({ id: 'start', what: 'start', code: 0x88, values: [1.35, 2.0], ref_addr: '0x14141A9D0', note: 'StartStyle case 0x88: pawn vt+0xFB0(0x88) / (0xA2) (the commands\' assets: nothing to do here); the camera distance scale (camera +0x804, controller+0xAE0 = 1) = values[0]; the soft-target range controller+0x970 = MyAutoLockonLimit x values[1]. Before the switch every form drops the images (ctrl slot 845), and EndStyle too.' });
// the rows the form hits with
const usedRows = new Set(ROWS.flatMap((r) => (['attack', 'proj'].includes(r.what) ? r.rows : r.what === 'pawn' ? r.rows.map((x) => x.split('@')[0]) : [])).filter((x) => x !== 'None'));
usedRows.add('SCWA_CHANGE');
for (const id of [...usedRows].sort()) {
  const a = atkRow(id);
  must(a, 'p_ex001_AttackData has no row ' + id);
  add({ id: 'row.' + id, what: 'row', logical: id, values: [a.m_Power, a.m_FormPoint, a.m_ReactionPowerValue], flags: [String(a.m_DamageKind).split('::').pop(), String(a.m_DamageAttribute).split('::').pop()], note: 'p_ex001_AttackData row ' + a.id + ': values = [m_Power, m_FormPoint, m_ReactionPowerValue]; carried by the combat sheet sora-combat-attack-data (tools/sora_combat_sheet.js: the form rows group) for the hit to deal it' });
}

// THE SHAPE TRACKS: the hit groups of the attack clips (WA_Front, WA_Break, WA_Britz, WA_Behind, WA_Behind2) are shapes of
// the staff's TresAtkColl0 (Player/p_ex001/w_so072_Wep) with m_AttachType1 OWNER_ATTACHED_MESH: on SORA's mesh, at his bone
// or socket (kh_root = the socket on kh_sk, trans, center) + m_RelativeLocation1 in its frame, under the clip. Per animation
// frame [ax, ay, az, bx, by, bz, radius] in his actor space (UE cm, his yaw 0): SPHERE a = b; CAPSULE2 a = socket 1 +
// location 1, b = socket 2 + location 2. The forward kinematics of tools/sora_pack.js attackSockets, as
// tools/sora_form_shield.js does it (copied, not shared: another package's file).
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
  const myaw = ID.mesh_component.relative_rotation_ue_deg.yaw * Math.PI / 180, cy = Math.cos(myaw), sy = Math.sin(myaw);
  const actor = (v) => [v[0] * cy - v[1] * sy, v[0] * sy + v[1] * cy, v[2]];
  const localT = (b, tr, time) => ({ q: tr.rotation ? sp.sampleChan(tr.rotation, time)(tr.rotation.ue_values) : b.ue.rotation, t: tr.translation ? sp.sampleChan(tr.translation, time)(tr.translation.ue_values) : b.ue.translation, s: tr.scale ? sp.sampleChan(tr.scale, time)(tr.scale.ue_values) : b.ue.scale });
  const byBone = (a) => { const m = new Map(); if (a) for (const c of a.chans) { if (!m.has(c.bone)) m.set(c.bone, {}); m.get(c.bone)[c.path] = c; } return m; };
  const boneT = (bones, chans, k, time, rootMotion) => { const chain = []; for (let i = k; i >= 0; i = bones[i].parent) chain.unshift(i); let T = null; for (const i of chain) { const l = localT(bones[i], (i === 0 && rootMotion) ? {} : (chans.get(i) || {}), time); T = T ? under(l, T) : l; } return T; };
  const at = (anim, name, time) => {
    const sock = SORA_SOCKETS[name];
    const k = SORA_BONES.findIndex((b) => b.name === (sock ? sock.bone : name));
    if (k < 0) return null;
    const T = boneT(SORA_BONES, byBone(anim), k, Math.min(time, anim.length), anim.props && anim.props.bEnableRootMotion === true);
    return sock ? under(sock.T, T) : T;
  };
  const WEP = blueprint('Player/p_ex001/w_so072_Wep');
  const SHAPES = ((WEP.byClass('TresAtkCollComponent')[0] || {}).CollisionShapesSrc || []).map((s) => ({ group: s.m_GrpName, shape: String(s.m_ShapeType).split('::').pop(), size: v3(s.m_Size), s1: s.m_SocketName1 || 'None', s2: s.m_SocketName2 || 'None', l1: v3(s.m_RelativeLocation1), l2: v3(s.m_RelativeLocation2), attach: String(s.m_AttachType1 || '').split('::').pop() }));
  const r1 = (x) => Math.round(x * 10) / 10;
  for (const r of ROWS.filter((x) => x.what === 'attack')) {
    const it = item(r.clip);
    const groups = [...new Set((it.notifies || []).filter((n) => n.class === 'TresAnimNotifyState_Attack').map((n) => String(prop(n, 'CollisionGrpName') ?? 'None')))];
    if (!groups.length) continue;
    const anim = sp.loadAnimation(it.source_asset + '.' + path.basename(it.source_asset), SORA_BONES);
    for (const g of groups) {
      const s = SHAPES.find((x) => x.group === g);
      must(s && s.attach === 'OWNER_ATTACHED_MESH', r.clip + ' ' + g + ': not an owner-attached shape of w_so072');
      const values = [];
      for (let f = 0; f < it.num_frames; f++) {
        const t = it.num_frames > 1 ? f * it.duration_s / (it.num_frames - 1) : 0;
        const T1 = at(anim, s.s1, t);
        must(T1, r.clip + ' ' + g + ': no bone / socket ' + s.s1);
        const a = actor(point(T1, s.l1));
        const T2 = s.shape === 'CAPSULE2' && s.s2 !== 'None' ? at(anim, s.s2, t) : null;
        const b = T2 ? actor(point(T2, s.l2)) : a;
        values.push(...a.map(r1), ...b.map(r1), s.size[0]);
      }
      add({ id: 'track.' + r.clip + '.' + g, what: 'track', code: it.num_frames, logical: g, clip: r.clip, length: r4(it.duration_s), flags: [s.shape, s.s1 + '-' + s.s2], values, note: 'w_so072_Wep TresAtkColl0 ' + g + ' (OWNER_ATTACHED_MESH: on Sora\'s ' + s.s1 + ' / ' + s.s2 + ' + location ' + s.l1 + ' / ' + s.l2 + '): per frame [ax, ay, az, bx, by, bz, radius] (cm, his actor space, his yaw 0)' });
    }
  }
  // THE TOWER (FINISH_WA's w_so073) under its own A417 (no root motion: its actor stands where it spawned): the forward
  // kinematics of its skeleton (the mesh build/sora_pack/keyblades/w_so073 model.json source_mesh; TresCharMesh0
  // RelativeRotation yaw 270 in StylePawn/w_so073), per frame, in the tower's actor space:
  //   X_link01_skt (bone X_link01 + its socket rotation): Sora rides it (StyleFinishWA sub 1: pawn vt+0x23A8, attach rules
  //   DAT_148750ED4, the actor snapped to the socket): [x, y, z, forward x, y, z];
  //   the TresAtkColl0 groups FinishWA / FinishWA2 (CAPSULE1 at socket ATK_00 + m_RelativeLocation1, turned by
  //   m_RelativeRocation, TRES_ACRA_NORMAL): [centre x, y, z, unit axis x, y, z] (the half length is m_Size.Z).
  {
    const TMJ = readJson(P('build', 'sora_pack', 'keyblades', 'w_so073', 'model.json'));
    const TSK = readJson(P('build', 'sora_pack', 'keyblades', 'w_so073', 'sockets.json')).weapon_sockets;
    const TBONES = sp.buildSkeleton(sp.readGlb(P('assets_converted', 'meshes', sp.CONTENT, sp.gamePathToContent(TMJ.source_mesh) + '.glb')).json);
    const tc = Object.values(TMJ.clips).find((c) => c.sequence === 'A417');
    const tanim = sp.loadAnimation(tc.asset + '.' + tc.sequence, TBONES);
    const mesh = (TB.byClass('TresSkeletalMeshComponent').find((m) => m.RelativeRotation) || {}).RelativeRotation || { yaw: 0 };
    const ty = mesh.yaw * Math.PI / 180, tcy = Math.cos(ty), tsy = Math.sin(ty);
    const tactor = (v) => [v[0] * tcy - v[1] * tsy, v[0] * tsy + v[1] * tcy, v[2]];
    const sockT = (name, time) => {
      const ws = TSK[name];
      const k = TBONES.findIndex((b) => b.name === (ws ? ws.bone : name));
      must(k >= 0, 'w_so073: no bone for ' + name);
      let T = boneT(TBONES, byBone(tanim), k, Math.min(time, tanim.length), false);
      if (ws) T = under({ q: rotQ({ pitch: ws.local_rotation[0], yaw: ws.local_rotation[1], roll: ws.local_rotation[2] }), t: ws.local_location, s: [1, 1, 1] }, T);
      return T;
    };
    const norm = (v) => { const l = Math.hypot(v[0], v[1], v[2]) || 1; return v.map((x) => x / l); };
    const frames = tc.frames, len = tc.duration_s;
    const tf = (f) => (frames > 1 ? f * len / (frames - 1) : 0);
    const link = [];
    for (let f = 0; f < frames; f++) {
      const T = sockT('X_link01_skt', tf(f));
      const p = tactor(T.t), fw = norm(tactor(qrot(T.q, [1, 0, 0])));
      link.push(...p.map(r1), ...fw.map((x) => r4(x)));
    }
    add({ id: 'track.tower.X_link01_skt', what: 'track', code: frames, logical: 'X_link01_skt', clip: 'A417', length: r4(len), flags: ['w_so073', 'yaw ' + mesh.yaw], values: link, note: 'the tower\'s socket X_link01_skt under its A417, per frame [x, y, z, forward x, y, z] (cm, the tower\'s actor space): where Sora rides (snapped to it)' });
    for (const s of (TB.byClass('TresAtkCollComponent')[0] || {}).CollisionShapesSrc || []) {
      const vals = [];
      const rr = s.m_RelativeRocation || { pitch: 0, yaw: 0, roll: 0 };
      for (let f = 0; f < frames; f++) {
        const T = sockT(s.m_SocketName1, tf(f));
        const c = tactor(point(T, v3(s.m_RelativeLocation1)));
        const ax = norm(tactor(qrot(qmulH(T.q, rotQ(rr)), [0, 0, 1])));
        vals.push(...c.map(r1), ...ax.map((x) => r4(x)));
      }
      add({ id: 'track.tower.' + s.m_GrpName, what: 'track', code: frames, logical: s.m_GrpName, clip: 'A417', length: r4(len), flags: [String(s.m_ShapeType).split('::').pop(), s.m_SocketName1], values: vals, note: 'the tower\'s TresAtkColl0 ' + s.m_GrpName + ' at ' + s.m_SocketName1 + ' + ' + JSON.stringify(v3(s.m_RelativeLocation1)) + ' turned ' + JSON.stringify(rr) + ', per frame [centre x, y, z, unit axis x, y, z] (the tower\'s actor space); half length = m_Size.Z, radius = m_Size.X + m_IncSize.X t, to m_IncMaxSize.X' });
    }
  }
}

// (AUDIT-A STAFF) THE FORM WEAPONS' AND THE TOWER'S OWN CLIP NOTIFIES (AUDIT-B's seam, AUDIT-B/AUDIT.md: tools/lib/wnotify_rows.js
// `wnotify` rows in F-SHIP's columns, played by kh-core forms/ship.rs `wnotify_pass` from staff.rs `on_notifies`; the host
// resolves the `kb:` anchors on the weapon's own skeleton). KH3 gives the staff w_so072 (RWeapon on R_wep7_skt), the transform
// unit w_so071 (Deform, no socket: R_wep_skt) and the tower w_so073 (TresStylePawnWATower, its own actor: mount `tower`, its
// place and yaw = kh-core staff::tower) each an anim instance playing the row of Sora's logical name; their notifies fire on
// THEIR meshes: the unit's StyleChange10 (A401) se02007_025 and vw_so072_chg0 .. chg5, the staff's swing SEs se02007_021 ..
// 024 / 038 / 039, the tower's A417 effects (fin_end0 / add_lgt0 / obi_one_set0 / lbl0 / mrg1 x2 / fin_key0 x2 / beam0 /
// beam3 / exp0), its sounds se02007_026 (bigD_01) / se00001_034 and Sora's voice lines (the tower's PlayVoice: weighted
// bt1010740 / bt1010750, then bt1010770: rows `code` 1 with the voice's path, the hosts' routes resolve the list by it).
// The staff's A416 TresAnimNotifyState_ChangeAttachParent (kh_root, 0 .. 3.1 s): (AUDIT-A PLACEMENT) tools/sora_form_placement.js.
{
  const { wnotifyRows } = require('./lib/wnotify_rows.js');
  const models = [
    { model: 'w_so072', mount: 'R_wep7_skt', form: () => 'WA' },
    { model: 'w_so071', mount: 'R_wep_skt', form: (c) => (c.sequence === clipOf(WA.transform_clip) ? 'WA' : null) },
    { model: 'w_so073', mount: 'tower', form: (c) => (c.sequence === 'A417' ? 'WA' : null) },
  ];
  for (const r of wnotifyRows(P('build', 'sora_pack'), models)) add({ ...r, ref_addr: r.ref_addr + ' (AUDIT-A STAFF)' });
  const pvv = (list) => Object.fromEntries((list || []).map((p) => [p.name, p.value]));
  for (const m of ['w_so072', 'w_so073']) {
    const mj = readJson(P('build', 'sora_pack', 'keyblades', m, 'model.json'));
    for (const c of mj.clips || []) {
      const clip = ((c.logical_names || []).find((n) => /^M_/.test(n)) || 'M_' + c.sequence).slice(2);
      for (const [k, n] of (c.notifies || []).entries()) {
        const np = n.properties || {};
        if (n.class === 'TresAnimNotify_PlayVoice') {
          const list = np.m_RandomVoiceAssets ? np.m_RandomVoiceAssets.items.map((i) => pvv(i)) : [{ Voice: np.m_VoiceAsset, RandomWeights: 100 }];
          const sock = np.m_SocketName && np.m_SocketName !== 'None' ? np.m_SocketName : '';
          add({ id: 'wnotify.' + m + '.' + c.sequence + '.voice' + k, what: 'wnotify', code: 1, logical: 'voice', clip, length: r4(c.duration_s || 0), flags: ['kb:' + m + ':tower:' + sock, '-', 'instant', 'g-'], sounds: [list[0].Voice], values: [r4(n.trigger_time), -1, 0, 0, 0, 0, 0, 0, 0],
            ref_addr: 'build/sora_pack/keyblades/' + m + '/model.json (AUDIT-A STAFF)', note: 'the ' + m + ' clip ' + c.sequence + ' with Sora\'s ' + clip + ': TresAnimNotify_PlayVoice ' + list.map((v) => String(v.Voice).split('.').pop() + '@' + v.RandomWeights).join(' / ') + ' (the hosts\' routes draw the weighted list by its first entry); values = [time, -1, 0 x6, 0]' });
        } // (AUDIT-A PLACEMENT) the A416 TresAnimNotifyState_ChangeAttachParent: tools/sora_form_placement.js (every form weapon's window)
      }
    }
  }
}

// ------------------------------------------------------------------------------------------------ checks
must(ROWS.filter((r) => r.what === 'attack').length === 8, 'expected 8 attack kinds');
// (AUDIT-A STAFF) every own-clip notify of the three models: unit A401 1 sound + 9 effects, staff 6 sounds, tower 2 sounds + 11 effects + 2 voices
must(ROWS.filter((r) => r.what === 'wnotify').length === 31, 'expected 31 wnotify rows, got ' + ROWS.filter((r) => r.what === 'wnotify').length);
// (AUDIT-A PLACEMENT) the staff's A416 ChangeAttachParent row moved to sheets/port/sora-form-placement.json
must(ROWS.filter((r) => r.what === 'track').length === 9, 'expected the six tracked groups of the attack clips and the three of the tower, got ' + ROWS.filter((r) => r.what === 'track').length);
for (const r of ROWS.filter((x) => x.what === 'attack')) must(r.clip !== '-' && r.length > 0, r.id + ': no clip');
must(ROWS.find((r) => r.id === 'attack.0x80').values[2] > 0, 'M_A411 has a Shot notify');
must(ROWS.some((r) => r.id === 'anim.M_A417' && r.clip === 'A417'), 'AnimSet10 has M_A417');
for (const n of ['WA_BEAM', 'WA_FINISH', 'WA_BRAID', 'WA_AVATER']) must(ROWS.some((r) => r.id === 'proj.' + n), n);

// ------------------------------------------------------------------------------------------------ sheet
const S = (n, t, g, doc) => [n, t, g, doc];
const COLS = [
  S('id', 'str', 'str', '<what>.<key>'), S('what', 'enum:attack|anim|steer|max|avatar|proj|pawn|shape|finish|start|row|track|wnotify|parent', 'str', 'the row kind'),
  S('code', 'int', 'i32', 'attack kind (attack), finish sub-step (finish), the form command (max, start), else 0'), S('logical', 'str', 'str', 'the logical anim name KH3 plays (attack / anim / finish), the projectile name (proj), the blueprint (pawn), the group (shape), the row (row)'),
  S('clip', 'str', 'str', 'the clip = the animation table row (the sequence); the blueprint class (proj); the pawn\'s clip (pawn); the owner (shape)'), S('length', 'num', 'f32', 'the clip length, s'),
  S('z_gain', 'num', 'f32', 'z gain (+0x14C)'), S('stop_dist', 'num', 'f32', 'stop distance (+0x24C)'), S('air_mode', 'bool', 'bool', '+0x1DF'),
  S('flags', 'list', 'str[]', 'the case\'s writes / the kind\'s strings (see the note)'), S('windows', 'list', 'str[]', 'the clip\'s hit windows a-b, s'), S('rows', 'list', 'str[]', 'the attack rows of those windows; the projectile\'s row'),
  S('effects', 'list', 'str[]', 'the clip\'s effects (time@asset); the projectile\'s particles'), S('sounds', 'list', 'str[]', 'the clip\'s sounds / voices (time@asset)'), S('values', 'list', 'f32[]', 'numbers by kind (see the note)'),
  S('ref_addr', 'str', 'str', 'decompiled function'), S('ref_conf', 'enum:confirmed|inferred|guess', 'str', 'confirmed = read in the code or the data; inferred; guess'), S('note', 'str', 'str', '-'),
];
const NAME = 'sora-form-staff';
const rows = ROWS.map((r) => COLS.map((c) => r[c[0]]));
for (const [i, r] of rows.entries()) for (const [k, v] of r.entries()) must(v !== undefined && v !== null && !(typeof v === 'number' && !Number.isFinite(v)), 'row ' + ROWS[i].id + ': cell ' + COLS[k][0] + ' is ' + v);
fs.writeFileSync(P('sheets', 'port', NAME + '.json'), '{"sheet":"' + NAME + '","group":"port","columns":' + JSON.stringify(COLS.map((c) => c[0])) + ',"rows":[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
const by = {}; for (const r of ROWS) by[r.what] = (by[r.what] || 0) + 1;
console.log('sheets/port/' + NAME + '.json: ' + rows.length + ' rows ' + JSON.stringify(by));

// ------------------------------------------------------------------------------------------------ register (this tool's entry only; replaced in place)
const q = (s) => "'" + String(s).replace(/\\/g, '\\\\').replace(/'/g, "\\'") + "'";
const NL = '\n';
const DOC = 'KH3\'s Mirage Staff as data for its form package (kh-core combat/forms/staff.rs, states/combo_wa.rs): COMBO_WA\'s attack kinds, the replacement anim set, the stick steering, MaxCombo / Finishers, the mirror images and the projectiles, FINISH_WA\'s tower.';
const MARK = '(weapons F-STAFF)';
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
  const text = '  // ' + MARK + ' ' + DOC + ' Written by tools/sora_form_staff.js (documentation/KH3_WEAPONS_PLAN.md 1.3)' + NL +
    '  ' + q(NAME) + ": { group: 'port', file: " + q(NAME + '.json') + ", by: 'tools/sora_form_staff.js', from: " + q('sora-keyblade-forms, AnimSet10 of the asset pack, the WA projectile / style-pawn blueprints, p_ex001_AttackData and the decompiled ComboWA / PlayAttack / WandAvator / StyleFinishWA') + ', external: true, cols: [' + NL +
    '    ' + COLS.map((c, i) => '[' + q(c[0]) + ', ' + (i === 0 ? 'K' : 'V') + ', ' + q(c[1]) + ", '-', " + q(c[3]) + (['note', 'flags', 'windows', 'rows', 'effects', 'sounds', 'values', 'logical', 'clip'].includes(c[0]) ? ', true' : '') + ']').join(', ') + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/sheet_defs.js');
}
{
  const f = P('tools', 'lib', 'port_gen_defs.js'); const before = fs.readFileSync(f, 'utf8');
  const END = NL + '    },' + NL;
  const pos = (t) => { const anchor = '  tables: {' + NL; const at = t.indexOf(anchor); must(at >= 0, 'port_gen_defs.js: anchor tables'); return at + anchor.length; };
  const cols = COLS.filter((c) => c[2]).map((c) => c[0] + ': { type: ' + q(c[2]) + (c[3] && c[3] !== '-' ? ', doc: ' + q(c[3]) : '') + ' }').join(', ');
  const text = '    // ' + MARK + ' ' + NAME + ': written by tools/sora_form_staff.js (documentation/KH3_WEAPONS_PLAN.md 1.3)' + NL +
    '    ' + q(NAME) + ': {' + NL +
    "      file: 'sora_form_staff.rs', row: 'FormStaffRow', table: 'FORM_STAFF', key: 'id', key_prefix: 'FST'," + NL +
    '      doc: ' + q(DOC) + ',' + NL +
    '      columns: { ' + cols + ' },' + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/port_gen_defs.js');
}
console.log('sora_form_staff ' + VERSION);
