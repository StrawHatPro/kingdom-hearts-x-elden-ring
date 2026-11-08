// (weapons F-LIGHTDARK) KH3 Re Mind's Light Form, Dark Form and Double Form as DATA for their form package (kh-core
// combat/forms/lightdark.rs, states/combo_lf.rs, combo_df.rs, combo_tf.rs): Light Form (form.LF, CHANGE_LF 0xDD,
// Oathkeeper) -> Double Form (form.TF, CHANGE_TF 0xDF); Dark Form (form.DF, CHANGE_DF 0xDE, Oblivion) -> Double Form
// (form.TF2, CHANGE_TF2 0xE0). documentation/KH3_WEAPONS_PLAN.md 1.3 / 4, build/sora_tmp/weapons/F-LIGHTDARK/STATE.md.
//
//   node tools/sora_form_lightdark.js            write sheets/port/sora-form-lightdark.json (then node tools/gen_port.js)
//   node tools/sora_form_lightdark.js --check    also prove the socket reader on the main pack's own tracks
//
// Inputs (read only): sheets/port/sora-keyblade-forms.json (P3's rows), build/sora_pack/keyblades/sora_forms_manifest.json
// (the asset agent's pack of AnimSet22 / AnimSet23), build/sora_pack/manifest.json + xsi_manifest.json (the clips those sets
// share with the base set and the XS overlay), the weapon blueprints w_so200_Wep / w_so210_Wep (TresAtkColl0), the
// projectile blueprints of the forms' projectile sets, build/sora_pack/keyblades/w_so200 / w_so210 sockets.json (ATK_00 /
// ATK_01), p_ex001_0_Skeleton (R_wep_skt / L_wep_skt), p_ex001_AttackData, and THE CODE TABLES BELOW, read in the
// decompiled exe (re/decomp; vtables read from base/KH3.exe):
//   CLASS    ATresPlayerPawnBase slot 1337 0x141432F00: form 0xDD -> COMBO_LF (0x9F), 0xDE -> COMBO_DF (0xA0), 0xDF / 0xE0 ->
//            COMBO_TF (0xA1). The three classes (vtables 0x146910708 / 0x14690D2A8 / 0x1469130A8) override only Decide (slot
//            122: 0x141362300 / 0x1413606F0 / 0x141363CE0); ComboDF also slot 136 0x141352160 (Dark Break's warp); the anim
//            interface of ComboTF slot 69 (0x228) 0x1413590B0 (its shot handler). Reset / Begin / Tick / End / PlayAttack are
//            the Attack class's (the overrides are thunks to 0x14133D230 / 0x141333210 / 0x141341340 / 0x141335120 / 0x1413378B0).
//   PLAY     TresPlayerState_Attack::PlayAttack 0x1413378B0: cases 0x1F, 0x25, 0xCD .. 0xE0 (PLAY below).
//   COUNT    MaxCombo 0x141433570: 6; Finishers 0x141433A60: 3 (forms 0xDD .. 0xE0).
//   COUNTER  TryAttack 0x141437330 (0x141437E28 ...): the guard counter's forced kind in 0xDD .. 0xE0 is 0x25 (not 0x21 ..);
//            FUN_1414145C0: its reaction command controller+0x8A4 = 0xD7 (LF) / 0xD8 (DF) / 0xD9 (TF, TF2).
//   SHOT     the shot handlers 0x141344580 (Attack: 0x25 -> BARRIERCRACKER, 0xD0 -> SCDF_DARKBREAK after the landing) and
//            0x1413590B0 (ComboTF: 0xDA -> the notify's projectile LocationIndex x 200 ahead of his root; 0xDC -> the notify's
//            projectile 300 cm from the target, random pitch -60..0 / yaw -180..180 for the first 10, then +-90).
//   GUARD    TresPlayerState_Guard Begin 0x1413851D0 (weapon forms 0x16 / 0x17 / 0x18 -> the form guard, slot 119 0x14139F770:
//            Reflec / AirReflec / ReflecHit / AirReflecHit with the effect group 0x834 / 0x898 / 0xD48 (0xD52 with +0xE8 bit 13)).
//   DODGE    TresPlayerState_DodgeRoll 0x141368880: the play's effect group +0x90 = 0x834 in 0xDD, 0x898 in 0xDE.
//   FINISH   FINISH_TF / FINISH_TF2 (0xE1 / 0xE2): STYLEFINISH Begin 0x1413CBF70 binds slot 166 0x1413E0860 (Ultimate Finish:
//            M_N424 then M_N425); CanStyleFinish 0x14142E030: 0xE1 only in 0xDF, 0xE2 only in 0xE0.
//   CHANGE   StyleChange Begin 0x1413CBA70 / 0x1413D08A0: the three Second Form clips with the wear forms 8 / 9 / 10 and the
//            weapon forms 0x16 / 0x17 / 0x18 (P3's framework already runs them).
'use strict';
const fs = require('fs');
const path = require('path');
const { socketTrack } = require('./lib/form_sockets');
const ROOT = path.resolve(__dirname, '..');
const VERSION = '1.0.0';
const NAME = 'sora-form-lightdark';
const P = (...a) => path.join(ROOT, ...a);
const readJson = (f) => JSON.parse(fs.readFileSync(f, 'utf8'));
const must = (c, m) => { if (!c) throw new Error(m); };
const hex = (n) => '0x' + n.toString(16).toUpperCase();
const r4 = (x) => Math.round(x * 10000) / 10000;
const CHECK = process.argv.includes('--check');

// ------------------------------------------------------------------------------------------------ inputs
const sheet = (name) => { const s = readJson(P('sheets', 'port', name + '.json')); return s.rows.map((r) => Object.fromEntries(s.columns.map((c, i) => [c, r[i]]))); };
const FORMS = sheet('sora-keyblade-forms');
const form = (id) => { const f = FORMS.find((r) => r.id === id); must(f, 'sora-keyblade-forms has no ' + id); return f; };
const LF = form('form.LF'), DF = form('form.DF'), TF = form('form.TF'), TF2 = form('form.TF2');
must(LF.cmd === 0xDD && DF.cmd === 0xDE && TF.cmd === 0xDF && TF2.cmd === 0xE0, 'the forms\' commands are 0xDD .. 0xE0');
must(TF.finish === 0xE1 && TF2.finish === 0xE2, 'FINISH_TF / FINISH_TF2');
const MAIN = readJson(P('build', 'sora_pack', 'manifest.json'));
const XSI = readJson(P('build', 'sora_pack', 'xsi_manifest.json'));
const OV = readJson(P('build', 'sora_pack', 'keyblades', 'sora_forms_manifest.json'));
const ATK = readJson(P('sheets', 'entities', 'DataTable', 'Player', 'Attack', 'p_ex001_AttackData.json')).rows;
const CONTENT = P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content');
const pkg = (game) => readJson(path.join(CONTENT, game.replace(/^\/Game\//, '').split('.')[0] + '.uasset.json'));
const setName = (a) => String(a).split('/').pop().split('.')[0];
must(setName(LF.anim_sets[0]) === 'p_ex001_AnimSet22' && setName(DF.anim_sets[0]) === 'p_ex001_AnimSet22', 'Light / Dark Form replace with AnimSet22');
must(setName(TF.anim_sets[0]) === 'p_ex001_AnimSet23' && setName(TF2.anim_sets[0]) === 'p_ex001_AnimSet23', 'Double Form replaces with AnimSet23');

// the clips: item (with its pack file) by sequence name; the animation-table row of a sequence (the gen_port rule: a row of
// the main / XS manifest whose file is that sequence, else the overlay row named by the sequence itself)
const ITEMS = new Map();
for (const [m, dir] of [[MAIN, P('build', 'sora_pack')], [XSI, P('build', 'sora_pack')], [OV, P('build', 'sora_pack', 'keyblades')]]) for (const i of m.items) if (i.kind === 'animation' && !ITEMS.has(i.animation_name)) ITEMS.set(i.animation_name, { ...i, path: path.join(dir, i.file) });
const ROW_OF = new Map();
for (const m of [MAIN, XSI]) for (const a of m.logical_animations) if (a.file && !a.samples) { const seq = path.basename(a.file, '.glb'); if (!ROW_OF.has(seq)) ROW_OF.set(seq, a.name); }
const rowOf = (seq) => ROW_OF.get(seq) || (ITEMS.has(seq) ? seq : null);
const SET = {};
for (const [key, set] of [['LD', 'p_ex001_AnimSet22'], ['TF', 'p_ex001_AnimSet23']]) {
  SET[key] = new Map();
  for (const a of OV.logical_animations.filter((x) => setName(x.anim_set) === set)) SET[key].set(a.name, { seq: a.clip, cls: a.class });
}
const BASE = new Map(MAIN.logical_animations.map((a) => [a.name, a]));
// logical name -> sequence in a form (its set first, else the base set's row)
function seqOf(key, logical) {
  const r = SET[key].get(logical);
  if (r) return r.seq;
  const b = BASE.get(logical) || XSI.logical_animations.find((a) => a.name === logical);
  return b && b.file ? path.basename(b.file, '.glb') : null;
}
const prop = (n, key) => { const p = n.properties || {}; const v = p[key]; return v && typeof v === 'object' && 'value' in v ? v.value : v; };
function notes(seq, group) {
  const it = ITEMS.get(seq);
  const out = { length: it ? r4(it.duration_s) : 0, windows: [], rows: [], groups: [], effects: [], sounds: [], shots: [] };
  if (!it) return out;
  for (const n of it.notifies || []) {
    if (n.class === 'TresAnimNotifyState_Attack') { out.windows.push(r4(n.time) + '-' + r4(n.end_time)); out.rows.push(String(prop(n, 'm_AttackDataIDName'))); out.groups.push(String(prop(n, 'CollisionGrpName'))); }
    if (n.class === 'TresAnimNotify_Shot') out.shots.push(r4(n.time) + '@' + (prop(n, 'm_ProjectileName') || 'None') + '#' + (prop(n, 'LocationIndex') || 0));
    if (/AttachEffect/.test(n.class)) {
      const s = JSON.stringify(n.properties || {});
      const re = /"GroupID","type":"IntProperty","value":(-?\d+)[\s\S]*?"AttachEffect","type":"ObjectProperty","value":"([^"]*)"/g;
      let m; while ((m = re.exec(s))) { const g = +m[1]; if ((g === group || g === -1) && m[2] !== 'null') out.effects.push(r4(n.time) + '@' + g + ':' + m[2].split('.').pop()); }
    }
    if (/PlaySound|AttachSound|PlayVoice/.test(n.class)) { const s = JSON.stringify(n.properties || {}); const m = s.match(/\/Game\/Sound[^"]*/); out.sounds.push(r4(n.time) + '@' + (m ? m[0].split('.').pop() : n.class.replace(/^.*Notify(State)?_/, ''))); }
  }
  out.effects = [...new Set(out.effects)];
  return out;
}
const atkRow = (id) => ATK.find((r) => r._row_name === id);

const ROWS = [];
const add = (r) => ROWS.push({ form: '-', code: 0, logical: '-', clip: '-', length: 0, group: 0, root_len: 0, stop_dist: 160, z_gain: 3, air_mode: false, keep_face: false, chan_xy: true, chan_root: false, flags: [], windows: [], rows: [], effects: [], sounds: [], values: [], ref_addr: '-', ref_conf: 'confirmed', note: '-', ...r });

// ------------------------------------------------------------------------------------------------ anim (m_ReplaceAnimSets)
for (const key of ['LD', 'TF']) for (const [logical, r] of SET[key]) {
  const row = r.cls === 'AnimSequence' ? rowOf(r.seq) : null;
  add({ id: 'anim.' + key + '.' + logical, what: 'anim', form: key, logical, clip: row || '-', length: row ? notes(r.seq, 0).length : 0, ref_addr: '-', ref_conf: row ? 'confirmed' : 'inferred',
    note: (key === 'LD' ? 'p_ex001_AnimSet22 (Light / Dark Form)' : 'p_ex001_AnimSet23 (Double Form)') + ': ' + logical + ' -> sequence ' + r.seq + (row ? ' (animation-table row ' + row + ')' : ' (' + r.cls + ': no row of the animation table; the base set\'s row plays)') });
}

// ------------------------------------------------------------------------------------------------ PLAY: PlayAttack 0x1413378B0
// The defaults of every case (0x141337E0D): m_bFaceAt; +0x234 = (+0x234 & ~6) | 1 (the XY channel); stop distance +0x24C 160;
// z gain +0x14C 3.0; +0x1DF (air) 0; effect group +0x198 0; voice +0x19C 0. "keep_face" = +0x1F4 |= 0x10; "root" = +0x234 |=
// 2 with +0x1D8; the tail LAB_14133A87A: +0x1DF = 0 -> gravity on, DirZ 0; +0x1DF = 1 -> +0x234 |= 4 | 0x10, GravityScale 0.1,
// Flying, +0x18C = 0.6. Cases that `break` out of the switch get +0x1DF = 1 (0x14133A85F).
// LAB_14133A4A1: keep_face, root, group 0x834; LAB_14133A4BD: the group becomes 0x898 in form 0xDE.
const PLAY = [
  // kind, logical, root_len, keep_face, chan_root, air, group LF/DF/TF, flags, ref
  [0x1F, { LF: '1309', DF: '1309', TF: 'M_N420' }, 0, false, false, true, { LF: 0x834, DF: 0x898, TF: 0 }, [], '0x141337E0D case 0x1F: forms 0xDF / 0xE0 M_N420, else 1309 with group 0x106 -> 0x834 in 0xDD, 0x898 in 0xDE (LAB_14133A4BD); +0x1DF = 1'],
  [0x25, { LF: 'BarrierCracker', DF: 'BarrierCracker', TF: 'M_N426' }, 0, false, false, false, { LF: 0x834, DF: 0x898, TF: 0 }, ['invincible', 'voice:0x34', 'swing_started', 'shot:BARRIERCRACKER'], '0x141337E0D case 0x25: TF M_N426 (+0x234 |= 8, +0x23C |= 0x10, air); else BarrierCracker / AirBarrierCracker (air) with group 0xD48 -> 0x834 / 0x898, +0x19C = 0x34 (the voice group of the play), +0x164 = 1, pawn+0x... = 1, +0x234 |= 0x2800, FUN_1405B8B30(pawn+0x460); both: invincible (pawn vt+0x2068(1) at the tail)'],
  [0xCD, { LF: 'M_M411', DF: 'M_M411' }, 580, true, true, false, { LF: 0x834, DF: 0x898 }, [], 'case 0xCD: Zantetsuken; +0x1D8 = 580 (0x44110000), LAB_14133A4A1'],
  [0xCE, { LF: 'M_M412', DF: 'M_M412' }, 400, true, true, false, { LF: 0x834, DF: 0x898 }, [], 'case 0xCE: Finishing Leap; +0x1D8 = 400 (0x43C80000), LAB_14133A4A1'],
  [0xCF, { LF: 'M_M413' }, 380, true, true, false, { LF: 0x834 }, [], 'case 0xCF: Black Hole (Event Horizon); +0x1D8 = 380 (0x43BE0000), group 0x834, no form test'],
  [0xD0, { DF: 'M_M420' }, 0, true, true, true, { DF: 0x898 }, ['chan_xy_off', 'no_vz', 'dive', 'stop:250', 'warp', 'camera:5', 'shot:SCDF_DARKBREAK', 'b23c_20'], 'case 0xD0: Dark Break; +0x1F4 |= 0x10, +0x23C |= 0x20, +0x234 = (+0x234 & ~1) | 10, group 0x898, +0x1DE / +0x1DF = 1 (dive, air), stop distance 250 (0x437A0000), the attack camera thunk_FUN_1414122A0(ctrl, 0) then (ctrl, 1, 5, target) (LAB_14133A37D); ComboDF Decide: +0x1E1 = 1 (the warp), +0x25C = 0.5 (the invincible time)'],
  [0xD1, { LF: 'M_M414', DF: 'M_M414' }, 520, true, true, true, { LF: 0x834, DF: 0x898 }, [], 'case 0xD1: Triple Edge; +0x1DF = 1, +0x1D8 = 520 (0x44020000), LAB_14133A4A1'],
  [0xD2, { LF: 'M_M415', DF: 'M_M415' }, 750, true, true, true, { LF: 0x834, DF: 0x898 }, [], 'case 0xD2: Spinning Rush; +0x1DF = 1, +0x1D8 = 750 (0x443B8000)'],
  [0xD3, { LF: 'M_M416', DF: 'M_M416' }, 900, true, true, true, { LF: 0x834, DF: 0x898 }, [], 'case 0xD3: Spiral Charge; +0x1DF = 1, +0x1D8 = 900 (0x44610000)'],
  [0xD4, { TF: 'M_N411' }, 600, true, true, false, {}, [], 'case 0xD4: +0x1F4 |= 0x10, +0x234 |= 2, +0x1D8 = 600 (0x44160000), ground'],
  [0xD5, { TF: 'M_N412' }, 800, true, true, false, {}, [], 'case 0xD5: +0x1D8 = 800 (0x44480000), ground'],
  [0xD6, { TF: 'M_N427' }, 2040, true, true, true, {}, ['z_gain:4.5'], 'case 0xD6: +0x1D8 = 2040 (0x44FF0000), +0x14C = 4.5, break -> +0x1DF = 1'],
  [0xD7, { TF: 'M_N413' }, 620, true, true, true, {}, [], 'case 0xD7: +0x1D8 = 620 (0x441B0000), break -> air'],
  [0xD8, { TF: 'M_N414' }, 630, true, true, true, {}, [], 'case 0xD8: +0x1D8 = 630 (0x441D8000), break -> air'],
  [0xD9, { TF: 'M_N415' }, 850, true, true, true, {}, [], 'case 0xD9: +0x1D8 = 850 (0x44548000), break -> air'],
  [0xDA, { TF: 'M_N416' }, 240, true, true, false, {}, [], 'case 0xDA: Double Blast; +0x1D8 = 240 (0x43700000), ground'],
  [0xDB, { TF: 'M_N417' }, 1000, true, true, true, {}, [], 'case 0xDB: Flash Charge; +0x1D8 = 1000 (0x447A0000), break -> air'],
  [0xDC, { TF: 'M_N418' }, 100, true, true, true, {}, [], 'case 0xDC: Mirage Thrust; +0x1D8 = 100 (0x42C80000), break -> air'],
  [0xDD, { TF: 'M_N419' }, 190, true, true, true, {}, [], 'case 0xDD: Equilibrium Burst; +0x1D8 = 190 (0x433E0000), break -> air'],
  [0xDE, { TF: 'M_N421' }, 600, true, true, true, {}, ['no_vz'], 'case 0xDE: Spinning Strike; +0x1D8 = 600, LAB_141337FC6: +0x1DF = 1, +0x1F4 |= 0x10, +0x234 |= 10'],
  [0xDF, { TF: 'M_N422' }, 0, true, false, true, {}, ['no_vz', 'keep_dir_z'], 'case 0xDF: Launching Thrust; LAB_14133816D: +0x234 |= 8, +0x1F4 |= 0x10, +0x23C |= 0x10, break -> air'],
  [0xE0, { TF: 'M_N423' }, 0, true, false, true, {}, [], 'case 0xE0: Groundbreaker; LAB_14133A868: +0x1F4 |= 0x10, -> air'],
];
for (const [kind, by, root, keep, chanRoot, air, groups, flags, ref] of PLAY) {
  for (const f of ['LF', 'DF', 'TF']) {
    const logical = by[f];
    if (!logical) continue;
    const key = f === 'TF' ? 'TF' : 'LD';
    const seq = seqOf(key, logical);
    must(seq, f + ' ' + hex(kind) + ': no sequence for ' + logical);
    const row = rowOf(seq);
    must(row, f + ' ' + hex(kind) + ': ' + seq + ' is not in the animation table');
    const group = groups[f] || 0;
    const n = notes(seq, group);
    const fl = [...flags];
    if (kind === 0x25 && f === 'TF') fl.splice(0, fl.length, 'invincible', 'no_vz', 'keep_dir_z');
    const isAir = kind === 0x25 ? f === 'TF' : air;
    add({ id: 'attack.' + f + '.' + hex(kind), what: 'attack', form: f, code: kind, logical, clip: row, length: n.length, group, root_len: root, stop_dist: fl.includes('stop:250') ? 250 : 160,
      z_gain: fl.includes('z_gain:4.5') ? 4.5 : 3, air_mode: isAir, keep_face: keep, chan_xy: !fl.includes('chan_xy_off'), chan_root: chanRoot, flags: fl, windows: n.windows, rows: n.rows, effects: n.effects, sounds: n.sounds,
      values: kind === 0xD0 ? [0.5, 0.2] : kind === 0x25 && f !== 'TF' ? [0x34] : [], ref_addr: '0x1413378B0', note: ref + (kind === 0x25 && f !== 'TF' ? '; in the air AirBarrierCracker (' + rowOf(seqOf('LD', 'AirBarrierCracker')) + ')' : '') + '; shots ' + (n.shots.join(' ') || '-') + '; groups ' + ([...new Set(n.groups)].join(' ') || '-') });
  }
}
// the air variant of the counter (the case's other name)
{
  for (const f of ['LF', 'DF']) {
    const seq = seqOf('LD', 'AirBarrierCracker'); const n = notes(seq, f === 'LF' ? 0x834 : 0x898);
    add({ id: 'attack.' + f + '.0x25.air', what: 'attack', form: f, code: 0x25, logical: 'AirBarrierCracker', clip: rowOf(seq), length: n.length, group: f === 'LF' ? 0x834 : 0x898, air_mode: true, flags: ['invincible', 'voice:0x34', 'swing_started', 'shot:BARRIERCRACKER', 'air_variant'], windows: n.windows, rows: n.rows, effects: n.effects, sounds: n.sounds, values: [0x34], ref_addr: '0x1413378B0', note: 'case 0x25 with +0x142 (air): AirBarrierCracker, +0x1DF = 1' });
  }
}

// ------------------------------------------------------------------------------------------------ the base kinds' clips in the forms
// Light / Dark Form decide the base kinds too (ComboLF / ComboDF Decide: 1, 2, 3, 8, 9 .., the launchers 0x1C / 0x1D, the
// wall-run kinds); their clips are the base set's (AnimSet22 replaces none of them): attack.rs plays them, the hit sockets of
// the form's own weapon are below (sock.*).
const BASE_KINDS = sheet('sora-combat-attacks').filter((r) => r.kind < 0x40);

// ------------------------------------------------------------------------------------------------ the counter, guard, dodge, counts
add({ id: 'rule.max_combo', what: 'rule', logical: 'MaxCombo', values: [6], ref_addr: '0x141433570', note: 'pawn slot 928: forms 0x94 .. 0x98, 0x9A, 0xDD .. 0xE0 -> 6' });
add({ id: 'rule.finishers', what: 'rule', logical: 'Finishers', values: [3], ref_addr: '0x141433A60', note: 'pawn slot 1253: forms 0x94 .. 0x98, 0xDD .. 0xE0 -> 3' });
add({ id: 'rule.counter_kind', what: 'rule', logical: 'GuardCounterKind', values: [0x25], ref_addr: '0x141437330', note: 'TryAttack out of GUARD with the counter window open: the forced kind (+0x1E8) is 0x25 in forms 0x8F, 0x92, 0x93, 0xDD .. 0xE0 (else 0x21 / 0x23 / 0x24)' });
for (const [f, cmd, name] of [['LF', 0xD7, 'lightcounter'], ['DF', 0xD8, 'darkcounter'], ['TF', 0xD9, 'dualcounter']]) add({ id: 'rule.counter_command.' + f, what: 'rule', form: f, logical: 'CounterCommand', clip: name, values: [cmd], ref_addr: '0x1414145C0', note: 'the reaction command of the guard counter window (controller+0x8A4) in this form; kh3_system.command.' + name });
for (const [f, g] of [['LF', 0x834], ['DF', 0x898], ['TF', 0xD48]]) add({ id: 'guard.' + f, what: 'guard', form: f, logical: 'Reflec', clip: rowOf(seqOf(f === 'TF' ? 'TF' : 'LD', 'Reflec')), group: g, flags: ['AirReflec=' + rowOf(seqOf(f === 'TF' ? 'TF' : 'LD', 'AirReflec')), 'ReflecHit=' + (rowOf(seqOf(f === 'TF' ? 'TF' : 'LD', 'ReflecHit') || '') || '-'), 'AirReflecHit=' + (rowOf(seqOf(f === 'TF' ? 'TF' : 'LD', 'AirReflecHit') || '') || '-'), 'Body=3'], values: f === 'TF' ? [0xD52] : [], ref_addr: '0x14139F770', note: 'Guard Begin 0x1413851D0: weapon form ' + hex(f === 'LF' ? 0x16 : f === 'DF' ? 0x17 : 0x18) + ' -> tick slot 119 (the form guard: Light / Dark / Dual Guard), Body reaction type 3 (every direction guarded), ' + (f === 'TF' ? '+0xE8 |= 0xC00 (bits 10 / 11); group 0xD48, 0xD52 with bit 13' : 'no guard parts') });
for (const [f, g] of [['LF', 0x834], ['DF', 0x898]]) add({ id: 'dodge.' + f, what: 'dodge', form: f, logical: 'DodgeRollB', clip: rowOf(seqOf('LD', 'DodgeRollB')), group: g, ref_addr: '0x141368880', note: 'DodgeRoll: +0x90 (the play\'s effect group) = ' + hex(g) + ' in form ' + hex(f === 'LF' ? 0xDD : 0xDE) + '; AnimSet22 DodgeRollB = M051 (Sonic Dodge)' });

// ------------------------------------------------------------------------------------------------ FINISH: FINISH_TF / FINISH_TF2, slot 166 0x1413E0860
for (const [sub, logical, note] of [[0, 'M_N424', 'sub 0: air bit (+0x234 |= 4) when not on the ground; Flying (pawn vt+0x17A0(5, 1)); m_bFaceAt; +0x2B4 = his location, FUN_1413F4EF0(+0x2B4, 300, 1, 1, pawn) and FUN_143EAC3D0 (set his location there); Play(M_N424, blend -1, mode 2), voice 0; -> 1'],
  [1, 'M_N425', 'sub 1 at M_N424\'s end: FUN_1414466F0(pawn, +0x2B4) (pawn+0x1560 = the point, +0x155C = 1); Play(M_N425); on the ground: gravity on (vt+0x1728(1)), vt+0x1658, Walking (vt+0x17A0(1, 1)); -> 2'],
  [2, '-', 'sub 2: when a hit window has begun (+0x164): WorldSlow(ctrl, 0), -> 5'], [5, '-', 'sub 5 at the clip\'s end: +0x2D8 = 0.5, -> 0xC'], [0xC, '-', 'sub 0xC: +0x2D8 -= dt; at 0: pawn vt+0x1E48(1.0), DoChain']]) {
  const seq = logical === '-' ? null : seqOf('TF', logical);
  const n = seq ? notes(seq, 0) : { length: 0, windows: [], rows: [], effects: [], sounds: [] };
  add({ id: 'finish.TF.' + sub, what: 'finish', form: 'TF', code: sub, logical, clip: seq ? rowOf(seq) : '-', length: n.length, windows: n.windows, rows: n.rows, effects: n.effects, sounds: n.sounds, values: sub === 5 ? [0.5] : sub === 0 ? [300] : [], ref_addr: '0x1413E0860', note: 'FINISH_TF / FINISH_TF2 (Ultimate Finish): ' + note });
}

// ------------------------------------------------------------------------------------------------ shape: the weapons' TresAtkColl0
const SHAPE_OWNERS = { R: '/Game/Blueprints/Player/p_ex001/w_so200_Wep', L: '/Game/Blueprints/Player/p_ex001/w_so210_Wep' };
const tail = (v) => String(v || '').split('::').pop();
for (const [side, bp] of Object.entries(SHAPE_OWNERS)) {
  const j = pkg(bp);
  const e = j.exports.find((x) => x.objectName === 'TresAtkColl0');
  const items = e.data.props.find((p) => p.name === 'CollisionShapesSrc').value.items;
  const seen = {};
  for (const it of items) {
    const o = Object.fromEntries(it.map((q) => [q.name, q.value]));
    const v3 = (x) => (x ? [x.x, x.y, x.z] : [0, 0, 0]);
    const g = o.m_GrpName; seen[g] = (seen[g] || 0) + 1;
    add({ id: 'shape.' + side + '.' + g + (seen[g] > 1 ? '#' + seen[g] : ''), what: 'shape', form: side, logical: g, clip: bp.split('/').pop(), flags: [tail(o.m_ShapeType), o.m_SocketName1, o.m_SocketName2, tail(o.m_AttachType1), tail(o.m_RotAttachType), o.m_bDisableLocation1Attach ? 'frozen' : 'follow'],
      values: [...v3(o.m_Size), o.m_IncSize ? o.m_IncSize.x : 0, o.m_IncMaxSize ? o.m_IncMaxSize.x : 0, ...v3(o.m_RelativeLocation1)], rows: [o.m_DefaultAttackDataIDName || 'None'], ref_addr: '-', note: bp + ' TresAtkColl0 CollisionShapesSrc: values = m_Size xyz, m_IncSize.x, m_IncMaxSize.x, m_RelativeLocation1 xyz; flags = shape, socket 1, socket 2, attach 1, rotation attach, location 1 frozen' });
  }
}

// ------------------------------------------------------------------------------------------------ proj: the projectiles the forms fire
const NATIVE_SPEED = 2000; // TresProjectileMovementComponent ctor 0x141470D50: InitialSpeed = MaxSpeed = 2000 (tools/party_sheet.js MOVE_NATIVE)
function chain(game) {
  const out = [];
  for (let guard = 0; game && game.startsWith('/Game/') && guard < 8; guard++) {
    const j = pkg(game);
    const cdo = j.exports.find((e) => e.objectName.startsWith('Default__'));
    const ci = j.exports.indexOf(cdo) + 1;
    const comps = {};
    for (const e of j.exports) if (e.outerIndex === ci && e.data) comps[e.objectName] = e.data.props;
    out.push({ game, j, cdo: cdo.data.props, comps });
    const t = cdo.templateIndex < 0 ? j.imports[-cdo.templateIndex - 1] : null;
    let o = t; while (o && o.outerIndex) o = j.imports[-o.outerIndex - 1];
    game = o ? o.objectName : null;
  }
  return out;
}
const pv = (props, name) => { const p = (props || []).find((x) => x.name === name); return p ? p.value : undefined; };
const objName = (j, v) => { if (typeof v !== 'number' || v === 0) return '-'; const r = v < 0 ? j.imports[-v - 1] : j.exports[v - 1]; return r ? r.objectName : '-'; };
const PROJ = new Map();
for (const f of [LF, DF, TF]) for (const p of f.projectiles) { const [n, bp] = p.split('='); if (!PROJ.has(f.id + n)) PROJ.set(f.id + n, { form: f.id.slice(5), name: n, bp }); }
for (const { form: fk, name: n, bp } of PROJ.values()) {
  if (!['BARRIERCRACKER', 'SCDF_DARKBREAK', 'TF_BRAVESHOOT', 'TF_JOINTSTRUGGLE_L', 'TF_JOINTSTRUGGLE_L_END', 'TF_JOINTSTRUGGLE_D', 'TF_JOINTSTRUGGLE_D_END'].includes(n)) continue;
  if (fk === 'TF' && n === 'BARRIERCRACKER') continue;
  if (fk !== 'TF' && /^TF_/.test(n)) continue;
  const links = chain(bp.replace(/_C$/, ''));
  const first = (name, comp) => { for (const l of links) { const v = comp ? pv(l.comps[comp], name) : pv(l.cdo, name); if (v !== undefined) return { v, j: l.j }; } return { v: undefined }; };
  const life = first('m_MainLife').v || 0, dist = first('m_MaxLifeDistance').v || 0;
  const speed = first('InitialSpeed', 'ProjectileComp').v;
  const coll = first('CollisionShapesSrc', 'TresAtkColl0').v;
  const shapes = coll ? coll.items.map((it) => Object.fromEntries(it.map((q) => [q.name, q.value]))) : [];
  const hitFx = first('m_HitEffects', 'TresAtkColl0');
  const hit = hitFx.v ? hitFx.v.items.map((it) => Object.fromEntries(it.map((q) => [q.name, q.value])))[0] : null;
  const part = first('Template', 'ParticleComp');
  const fire = first('MyFireEffects');
  const fireFx = fire.v && fire.v.items.length ? Object.fromEntries(fire.v.items[0].map((q) => [q.name, q.value])) : null;
  const s0 = shapes[0] || {};
  add({ id: 'proj.' + fk + '.' + n, what: 'proj', form: fk, logical: n, clip: bp.split('/').pop(), flags: [tail(s0.m_ShapeType), part.v !== undefined ? objName(part.j, part.v) : '-', hit ? objName(hitFx.j, hit.m_PawnHitEffect) : '-', hit ? objName(hitFx.j, hit.m_PawnHitSEAsset) : '-', fireFx ? objName(fire.j, fireFx.MyParticleSystem) : '-', first('m_bDisableMovementProc', 'ProjectileComp').v ? 'disable_movement_proc' : 'moves'],
    // (every shape of the actor: Brave Shoot has two upright CAPSULE1 pillars)
    values: [life, speed === undefined ? NATIVE_SPEED : speed, dist, shapes.length, ...shapes.flatMap((s) => {
      must(['SPHERE', 'CAPSULE1'].includes(tail(s.m_ShapeType)), n + ': shape ' + s.m_ShapeType);
      const r = s.m_RelativeRocation || { pitch: 0, yaw: 0, roll: 0 };
      must(r.pitch === 0 && r.yaw === 0 && r.roll === 0 && tail(s.m_RotAttachType) === 'TRES_ACRA_NORMAL', n + ': a rotated shape (not ported)');
      return [tail(s.m_ShapeType) === 'CAPSULE1' ? 1 : 0, s.m_Size ? s.m_Size.x : 0, s.m_IncSize ? s.m_IncSize.x : 0, s.m_IncMaxSize ? s.m_IncMaxSize.x : 0, s.m_Size ? s.m_Size.z : 0, ...(s.m_RelativeLocation1 ? [s.m_RelativeLocation1.x, s.m_RelativeLocation1.y, s.m_RelativeLocation1.z] : [0, 0, 0])];
    })], rows: shapes.map((s) => s.m_DefaultAttackDataIDName || 'None'),
    ref_addr: '-', ref_conf: speed === undefined ? 'confirmed' : 'confirmed', note: links.map((l) => l.game.split('/').pop()).join(' < ') + ': values = m_MainLife (0 = none), InitialSpeed (' + (speed === undefined ? 'not set: the native ' + NATIVE_SPEED + ' of the TresProjectileMovementComponent ctor 0x141470D50 (+0x248 / +0x24C = 2000)' : 'the blueprint\'s') + '), m_MaxLifeDistance (0 = none), the shape count, then per shape (TresAtkColl0 CollisionShapesSrc, rotation 0 / TRES_ACRA_NORMAL: the actor\'s): kind (0 SPHERE, 1 CAPSULE1 upright), m_Size.x (radius), m_IncSize.x, m_IncMaxSize.x, m_Size.z (half length), m_RelativeLocation1 xyz; flags = shape, particle, hit effect, hit SE, fire effect, movement (m_bDisableMovementProc: TickComponent 0x1414854E0 then skips only the Tres procs - vt+0x5F0 / 0x610 / 0x600 / 0x5F8, homing / spin / accel - and the base UProjectileMovementComponent tick 0x144044AC0 still flies it straight at its velocity)' });
}

// ------------------------------------------------------------------------------------------------ sock: the weapons' hit sockets per frame
// Where the form weapon of each hand is, per frame of every clip a form plays with a hit window or a shot (attack kinds,
// finish bodies, the counters), from the clip's glb (tools/lib/form_sockets.js): [x, y, z, dx, dy, dz] = ATK_00 and the unit
// direction of the weapon bone's x axis (ATK_01 = ATK_00 + dir * ATK_01.x of the weapon: w_so200 80, w_so210 103), actor
// space, cm.
const SK = readJson(P('build', 'sora_pack', 'weapon', 'manifest.json'));
const wbone = (name) => { const x = SK.skeleton.bones.find((y) => y.name === name); return { q: x.rotation_ue, t: x.translation_ue, s: x.scale_ue }; };
const WROOT = [wbone('kh_sk'), wbone('wep')];
for (const model of ['w_so200', 'w_so210']) {
  const ws = readJson(P('build', 'sora_pack', 'keyblades', model, 'sockets.json')).weapon_sockets;
  must(ws.ATK_00.bone === 'wep' && ws.ATK_00.local_location.every((x) => x === 0) && ws.ATK_01.local_location[1] === 0 && ws.ATK_01.local_location[2] === 0, model + ': ATK_00 at the wep bone, ATK_01 on its x axis');
  add({ id: 'weapon.' + model, what: 'weapon', form: model === 'w_so200' ? 'R' : 'L', logical: model, values: [ws.ATK_01.local_location[0]], ref_addr: '-', note: 'build/sora_pack/keyblades/' + model + '/sockets.json: ATK_01.x (ATK_00 = 0): the stick between them is the KB_* / R_* / L_* capsule' });
}
const SOCK = { R: { bone: 'R_buki', socket: 'R_wep_skt' }, L: { bone: 'L_buki', socket: 'L_wep_skt' } };
{
  const sk = pkg('/Game/Character/pc/p_ex001/mdl/p_ex001_0_Skeleton');
  for (const side of Object.keys(SOCK)) {
    const e = sk.exports.find((x) => x.class === 'SkeletalMeshSocket' && pv(x.data.props, 'SocketName') === SOCK[side].socket);
    must(e && pv(e.data.props, 'BoneName') === SOCK[side].bone, SOCK[side].socket + ' on ' + SOCK[side].bone);
    SOCK[side].rot = pv(e.data.props, 'RelativeRotation') || { pitch: 0, yaw: 0, roll: 0 };
    SOCK[side].loc = pv(e.data.props, 'RelativeLocation') || { x: 0, y: 0, z: 0 };
  }
}
const MESH_YAW = MAIN.identification.mesh_component.relative_rotation_ue_deg.yaw;
function track(seq, side) {
  const it = ITEMS.get(seq);
  const s = SOCK[side];
  const r = socketTrack(it.path, { bone: s.bone, socketRotDeg: s.rot, socketLoc: [s.loc.x, s.loc.y, s.loc.z], atk00: [0, 0, 0], atk01: [100, 0, 0], weaponBones: WROOT, rootMotion: it.root_motion_enabled === true, meshYawDeg: MESH_YAW, frames: it.num_frames, length: it.duration_s });
  return r.sockets.map((f) => [f[0], f[1], f[2], r4((f[3] - f[0]) / 100), r4((f[4] - f[1]) / 100), r4((f[5] - f[2]) / 100)].map((x) => Math.round(x * 1000) / 1000));
}
if (CHECK) {
  // the reader against the main pack's own tracks (Kingdom Key: ATK_01 at its x)
  const kk = SK.sockets.sockets.find((x) => x.name === 'ATK_01').relative_location_ue_cm;
  for (const seq of ['1301', '1302', '1303', '1318', '1411']) {
    const ref = MAIN.attack_sockets.clips[seq]; const it = ITEMS.get(seq);
    const r = socketTrack(it.path, { bone: 'R_buki', socketRotDeg: SOCK.R.rot, socketLoc: [0, 0, 0], atk00: [0, 0, 0], atk01: kk, weaponBones: WROOT, rootMotion: it.root_motion_enabled === true, meshYawDeg: MESH_YAW, frames: ref.frames, length: ref.duration_s });
    let mx = 0; r.sockets.forEach((f, i) => f.forEach((v, k) => { mx = Math.max(mx, Math.abs(v - ref.sockets[i][k])); }));
    must(mx < 0.01, seq + ': the socket reader is off by ' + mx + ' cm');
    console.log('check ' + seq + ': max ' + mx.toFixed(4) + ' cm');
  }
}
const clipsOf = new Map(); // seq -> sides
const want = (seq, side) => { const it = ITEMS.get(seq); if (!it || !(it.notifies || []).some((n) => n.class === 'TresAnimNotifyState_Attack' || n.class === 'TresAnimNotify_Shot')) return; if (!clipsOf.has(seq)) clipsOf.set(seq, new Set()); clipsOf.get(seq).add(side); };
for (const r of ROWS.filter((x) => x.what === 'attack' || x.what === 'finish')) {
  if (r.clip === '-') continue;
  const seq = seqOf(r.form === 'TF' ? 'TF' : 'LD', r.logical);
  if (!seq) continue;
  want(seq, 'R');
  if (r.form === 'TF') want(seq, 'L');
}
for (const k of BASE_KINDS) { const seq = seqOf('LD', k.anim); if (seq) want(seq, 'R'); }
for (const [seq, sides] of [...clipsOf].sort((a, b) => (a[0] < b[0] ? -1 : 1))) {
  for (const side of [...sides].sort().reverse()) {
    const it = ITEMS.get(seq);
    const t = track(seq, side);
    add({ id: 'sock.' + side + '.' + seq, what: 'sock', form: side, code: t.length, logical: rowOf(seq), clip: seq, length: r4(it.duration_s), values: t.flat(), ref_addr: '-',
      note: 'per frame (frame i at i x length / (frames - 1)) [x, y, z, dx, dy, dz]: the ' + (side === 'R' ? 'right' : 'left') + ' weapon\'s ATK_00 and its x axis (' + SOCK[side].bone + ' -> ' + SOCK[side].socket + ' -> weapon root -> wep), actor space, cm; from ' + path.relative(ROOT, it.path).replace(/\\/g, '/') });
  }
}

// ------------------------------------------------------------------------------------------------ row: the attack rows the forms hit with
const used = new Set();
for (const r of ROWS) { if (['attack', 'finish', 'proj'].includes(r.what)) for (const x of r.rows) if (x && x !== 'None') used.add(x); }
for (const id of [...used].sort()) {
  const a = atkRow(id);
  must(a, 'p_ex001_AttackData has no row ' + id);
  add({ id: 'row.' + id, what: 'row', logical: id, values: [a.m_Power, a.m_FormPoint, a.m_ReactionPowerValue], flags: [String(a.m_DamageKind).split('::').pop(), String(a.m_DamageAttribute).split('::').pop()], ref_addr: '-', note: 'p_ex001_AttackData row ' + a.id + ': values = m_Power, m_FormPoint, m_ReactionPowerValue (combat sheet sora-combat-attack-data carries it)' });
}

// ------------------------------------------------------------------------------------------------ ead: the attach data assets
// (AUDIT-C TF) THE ATTACH DATA ASSET OF DOUBLE FORM'S CLIPS: AnimSet23's AirDodge N054 (Sonic Step) carries a
// TresAnimNotifyState_AttachEffect naming vw_so220_warp (an SQEX_ParticleAttachDataAsset, no particle system). KH3 plays the
// asset's rows of the request's group (GroupID 0), each where ITS row attaches it (SQEX_StructParticleAttachData): the warp's
// head / skin / hood / belly / shin / blur systems on atama, mune, center, R_sune, L_sune, LOOPEND at the window's end. One
// `ead` row per asset row (the same layout as F-ULTIMATE's / AUDIT-C SW's): code = GroupID, logical = the asset, clip =
// SocketName, flags = [AttachType, end type, follow | free, abs | -], effects = [the system], values = [LocationOffset xyz,
// RotationOffset pitch yaw roll, ScaleOffset xyz, ParticleColor rgb, ParticleAlpha, FadeIn, FadeOut, StartDelayTime].
{
  const ASSETS = ['/Game/Effects/dlc01/wep/w_so220/ead/vw_so220_warp'];
  const unwrap = (v) => (v && v.items ? v.items : v && v.value && v.value.items ? v.value.items : []);
  const props = (arr) => Object.fromEntries((arr || []).map((p) => [p.name, p.value]));
  for (const asset of ASSETS) {
    const d = pkg(asset);
    const imp = (i) => d.imports[-i - 1].objectName;
    const e = d.exports.find((x) => x.class === 'SQEX_ParticleAttachDataAsset');
    must(e, asset + ': no SQEX_ParticleAttachDataAsset');
    const name = asset.split('/').pop();
    let k = 0;
    for (const outer of unwrap(props(e.data.props).EffectDataList)) {
      const o = props(outer);
      for (const it of unwrap(o.EffectDataList)) {
        const r = props(it);
        if (!(r.AttachEffect < 0)) continue;
        const ao = props(r.AttachObjectData || []);
        const lo = o.LocationOffset || {}, ro = o.RotationOffset || {}, so = o.ScaleOffset || {}, pc = o.ParticleColor || {};
        add({ id: 'ead.TF.' + name + '.' + String(k).padStart(2, '0'), what: 'ead', form: 'TF', code: r.GroupID, logical: name, clip: o.SocketName || 'None',
          flags: [String(r.UseAttachObjectData ? ao.AttachType : 'SQEX_ATTACH_OBJECT_OWNER').replace('SQEX_ATTACH_OBJECT_', ''), String(o.AttachEffectEndType || '').replace('ESQEX_ATTACH_EFFECT_END_TYPE_', ''), o.ToAttach === false ? 'free' : 'follow', o.bAbsoluteRotation ? 'abs' : '-'],
          effects: [imp(r.AttachEffect)], values: [lo.x || 0, lo.y || 0, lo.z || 0, ro.pitch || 0, ro.yaw || 0, ro.roll || 0, so.x ?? 1, so.y ?? 1, so.z ?? 1, pc.x ?? 1, pc.y ?? 1, pc.z ?? 1, o.ParticleAlpha ?? 1, o.FadeIn || 0, o.FadeOut || 0, o.StartDelayTime || 0].map(r4),
          ref_addr: asset, note: '(AUDIT-C TF) row ' + k + ' of the attach data asset (SQEX_StructParticleAttachData)' + (o.Description && o.Description.s ? ': Description "' + o.Description.s + '"' : '') });
        k++;
      }
    }
    must(ROWS.filter((x) => x.what === 'ead' && x.logical === name).length === 12, name + ': expected 12 rows');
  }
}

// ------------------------------------------------------------------------------------------------ checks
for (const r of ROWS.filter((x) => x.what === 'attack')) must(r.clip !== '-' && r.length > 0, r.id + ': no clip');
must(ROWS.filter((r) => r.what === 'attack').length === 33, 'expected 33 attack rows, got ' + ROWS.filter((r) => r.what === 'attack').length);
must(ROWS.some((r) => r.id === 'anim.LD.DodgeRollB' && r.clip === 'M051'), 'AnimSet22 DodgeRollB is M051 (Sonic Dodge)');
must(ROWS.some((r) => r.id === 'finish.TF.1' && r.clip === 'N425' && r.windows.length === 2), 'Ultimate Finish M_N425 has two hit windows');
must(ROWS.some((r) => r.id === 'sock.L.N411'), 'Double Form clips carry the left weapon');

// ------------------------------------------------------------------------------------------------ sheet
const COLS = ['id', 'what', 'form', 'code', 'logical', 'clip', 'length', 'group', 'root_len', 'stop_dist', 'z_gain', 'air_mode', 'keep_face', 'chan_xy', 'chan_root', 'flags', 'windows', 'rows', 'effects', 'sounds', 'values', 'ref_addr', 'ref_conf', 'note'];
ROWS.sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
const { writeSheet } = require('./lib/sheets');
const n = writeSheet(P('sheets', 'port', NAME + '.json'), NAME, 'port', COLS, ROWS.map((r) => COLS.map((c) => r[c])));
const by = {}; for (const r of ROWS) by[r.what] = (by[r.what] || 0) + 1;
console.log('sheets/port/' + NAME + '.json: ' + n + ' rows ' + JSON.stringify(by));
console.log('sora_form_lightdark ' + VERSION);
