// (weapons P3) KH3's formchanges as DATA for the form framework (kh-core combat/form.rs, style.rs): one row per formchange
// command (`form.<code>`: form.L1 Second Form ... form.AN Rage Form), with its Keyblade, command, level chain (the next
// stage a full gauge offers), time, finish commands, wear form, form abilities, transformation clip, replacement anim set,
// projectiles and weapon parts with their attach points. documentation/KH3_WEAPONS_PLAN.md 1.1 / 1.3 (package P3).
//
//   node tools/sora_keyblade_forms.js   write sheets/port/sora-keyblade-forms.json and register it (once, in place) in
//                                       tools/lib/sheet_defs.js and tools/lib/port_gen_defs.js; then node tools/extract.js,
//                                       node tools/preflight.js --summary, node tools/gen_port.js
//
// Inputs (read only): sheets/port/kh3-keyblade-forms.json and kh3-keyblades.json (the KH3 tables agent's sheets,
// documentation/KH3_WEAPON_DATA.md), sheets/entities/Load/Common/TresCmdData.json (command kinds, Param0 / 1 / 3), the
// decoded p_ex001_FormAbility (Rage Form's wear form 07, which no Keyblade row carries) and THE CODE TABLES BELOW, read in
// the decompiled exe (re/decomp, decompiled C = ref_conf "inferred" where the data alone does not show it):
//   OFFER   AwardAtMax 0x1413F0ED0: the formchange a full gauge offers, by weapon type and form level (switch twice).
//   FINISH  SetStyleFinishCommand 0x1413EE030 (the same switch heads AwardAtMax): the finish command of a form.
//   WEAPON  StartStyle 0x14141A9D0: the weapon type a command needs (the Keyblade it switches to when another is held).
//   CLIP    StyleChange Tick 0x1413D3AC0 sub 0: the transformation clip; Begin 0x1413CBA70: the wear form it applies.
//   ENDDEL  EndStyle 0x14141CA20: the command the end of a form deletes (its next stage's offer or its finish command).
'use strict';
const fs = require('fs');
const path = require('path');
const ROOT = path.resolve(__dirname, '..');
const VERSION = '1.0.0';
const P = (...a) => path.join(ROOT, ...a);
const readJson = (f) => JSON.parse(fs.readFileSync(f, 'utf8'));
const must = (c, m) => { if (!c) throw new Error(m); };
const hex = (n) => '0x' + n.toString(16).toUpperCase();

// ------------------------------------------------------------------------------------------------ inputs
const sheet = (name) => { const s = readJson(P('sheets', 'port', name + '.json')); return s.rows.map((r) => Object.fromEntries(s.columns.map((c, i) => [c, r[i]]))); };
const KBF = sheet('kh3-keyblade-forms');
const KB = sheet('kh3-keyblades');
const CMD = readJson(P('sheets', 'entities', 'Load', 'Common', 'TresCmdData.json')).rows;
const ENUMS = readJson(P('sheets', 'reflect', 'enum-values.json')).rows;
const short = (e) => (e == null ? '-' : String(e).split('::').pop());
const KIND = new Map(); // ETresCommandKind name (without TRES_CMD_KIND_) -> value
for (const r of ENUMS) if (r[1] === 'ETresCommandKind') KIND.set(short(r[2]).replace(/^TRES_CMD_KIND_/, ''), r[3]);
const kindOf = (name) => { const v = KIND.get(name); must(v !== undefined, 'ETresCommandKind has no ' + name); return v; };
const nameOf = (kind) => { for (const [n, v] of KIND) if (v === kind) return n; throw new Error('no ETresCommandKind ' + kind); };
const cmdRow = (kind) => { const r = CMD.find((x) => +x.id === kind); must(r, 'TresCmdData has no row ' + kind); return r; };
// p_ex001_FormAbility (TresFormAbilitySet m_Data): wear form -> abilities (for wear form 07, Rage Form)
const FORMABIL = (() => {
  const d = readJson(P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content', 'Blueprints', 'Player', 'p_ex001', 'p_ex001_FormAbility.uasset.json'));
  const e = d.exports.find((x) => x.class === 'TresFormAbilitySet');
  const m = new Map();
  for (const en of e.data.props.find((p) => p.name === 'm_Data').value.entries) {
    const get = (n) => (en.value.find((p) => p.name === n) || { value: [] }).value.map(short);
    m.set(short(en.key), { abilities: get('m_Ability'), text: get('m_TextAbility') });
  }
  return m;
})();

// ------------------------------------------------------------------------------------------------ the code tables
// OFFER: AwardAtMax 0x1413F0ED0 (both switches on pawn.vt+0x12C8 = the weapon type = ETresItemDefWeapon value of the held
// Keyblade): [form level 0 (no form), form level 1 (in the first stage)] -> command. Types 0x10 / 0x11 (CHANGE_L3 / L4) have
// no Keyblade item on PC and are left out. A form of the Second Form family (0x94..0x98) or Rage Form (0x8B) offers nothing.
const OFFER = { 1: ['CHANGE_L1'], 2: ['CHANGE_SH'], 3: ['CHANGE_DW', 'CHANGE_BZ'], 4: ['CHANGE_HM', 'CHANGE_DR'], 5: ['CHANGE_WA'], 6: ['CHANGE_CL', 'CHANGE_YO'],
  7: ['CHANGE_CL2', 'CHANGE_SK'], 8: ['CHANGE_DW2', 'CHANGE_BZ2'], 9: ['CHANGE_GM'], 10: ['CHANGE_HL', 'CHANGE_FL'], 11: ['CHANGE_SH2'], 12: ['CHANGE_HM2', 'CHANGE_DR2'],
  13: ['CHANGE_LF', 'CHANGE_TF'], 14: ['CHANGE_DF', 'CHANGE_TF2'], 15: ['CHANGE_SW'], 18: ['CHANGE_L2'], 19: ['CHANGE_L5'] };
// FINISH: SetStyleFinishCommand 0x1413EE030 switch on the form command (the head of AwardAtMax has the same one). The Second
// Form family takes its finish LEVELS instead (SetFinishLevel 0x141414C70: 1 0xAB, 2 0xAC, 3 0xAD, 3 = 0xAE in form 0x95).
// Forms not listed have no finish command (the first stages: their full gauge offers the second stage).
const FINISH = { CHANGE_YO: 'FINISH_YO', CHANGE_BZ: 'FINISH_BZ', CHANGE_BZ2: 'FINISH_BZ', CHANGE_DR: 'FINISH_DR', CHANGE_DR2: 'FINISH_DR', CHANGE_SK: 'FINISH_SK', CHANGE_GM: 'FINISH_GM',
  CHANGE_SW: 'FINISH_SW', CHANGE_WA: 'FINISH_WA', CHANGE_SH: 'FINISH_SH', CHANGE_SH2: 'FINISH_SH', CHANGE_AN: 'FINISH_AN', CHANGE_FL: 'FINISH_FL', CHANGE_TF: 'FINISH_TF', CHANGE_TF2: 'FINISH_TF2' };
const LEVELS = { CHANGE_L1: ['LM_STUNIMPACT', 'LM_SONICRAVE', 'LM_LASTARCANUM'], CHANGE_L2: ['LM_STUNIMPACT', 'LM_SONICRAVE', 'LM_UNIONDISCODE'], CHANGE_L5: ['LM_STUNIMPACT', 'LM_SONICRAVE', 'LM_LASTARCANUM'] };
// WEAPON: StartStyle 0x14141A9D0, the switch after ClearQueue: the weapon type a command needs (pawn.vt+0x13C8 switches to it).
const WEAPON = { CHANGE_DW: 3, CHANGE_BZ: 3, CHANGE_HM: 4, CHANGE_DR: 4, CHANGE_YO: 6, CHANGE_CL: 6, CHANGE_SK: 7, CHANGE_CL2: 7, CHANGE_GM: 9, CHANGE_SW: 15, CHANGE_WA: 5, CHANGE_SH: 2,
  CHANGE_HL: 10, CHANGE_FL: 10, CHANGE_DW2: 8, CHANGE_BZ2: 8, CHANGE_SH2: 11, CHANGE_HM2: 12, CHANGE_DR2: 12, CHANGE_L1: 1, CHANGE_L2: 18, CHANGE_L5: 19, CHANGE_LF: 13, CHANGE_TF: 13, CHANGE_DF: 14, CHANGE_TF2: 14 };
// CLIP: StyleChange Tick sub 0. The Second Form family and 0xDD..0xE0 (flags 0x4A) play StyleChange -> StyleChangeLoop ->
// StyleChangeEnd and take their wear form at sub 3 (Begin: L* 5, LF 8, DF 9, TF / TF2 10; LF..TF2 also change the weapon
// variant there, flag 0x40); every other form plays its own clip at sub 0 and changes the weapon variant (the equipment's
// m_OwnerWearForm) at once (sub 1 -> 5). Rage Form: StyleChange13, wear form 7 (Begin, case 0x8B).
const CLIP = { CHANGE_DW: 'StyleChange2', CHANGE_HM: 'StyleChange3', CHANGE_YO: 'StyleChange4', CHANGE_BZ: 'StyleChange5', CHANGE_DR: 'StyleChange6', CHANGE_SK: 'StyleChange7', CHANGE_GM: 'StyleChange8',
  CHANGE_SW: 'StyleChange9', CHANGE_WA: 'StyleChange10', CHANGE_SH: 'StyleChange11', CHANGE_CL: 'StyleChange12', CHANGE_AN: 'StyleChange13', CHANGE_HL: 'StyleChange14', CHANGE_FL: 'StyleChange15',
  CHANGE_DW2: 'StyleChange16', CHANGE_BZ2: 'StyleChange17', CHANGE_CL2: 'StyleChange18', CHANGE_SH2: 'StyleChange19', CHANGE_HM2: 'StyleChange20', CHANGE_DR2: 'StyleChange21' };
const AT_STYLE_CHANGE = new Set(['CHANGE_L1', 'CHANGE_L2', 'CHANGE_L5', 'CHANGE_LF', 'CHANGE_DF', 'CHANGE_TF', 'CHANGE_TF2']);
// ENDDEL: EndStyle 0x14141CA20 switch on the form command: what its end deletes (the Second Form family: 0xAB..0xAE).
const ENDDEL = { CHANGE_DW: ['CHANGE_BZ'], CHANGE_HM: ['CHANGE_DR'], CHANGE_YO: ['FINISH_YO'], CHANGE_BZ: ['FINISH_BZ'], CHANGE_BZ2: ['FINISH_BZ'], CHANGE_DR: ['FINISH_DR'], CHANGE_DR2: ['FINISH_DR'],
  CHANGE_SK: ['FINISH_SK'], CHANGE_GM: ['FINISH_GM'], CHANGE_SW: ['FINISH_SW'], CHANGE_WA: ['FINISH_WA'], CHANGE_SH: ['FINISH_SH'], CHANGE_SH2: ['FINISH_SH'], CHANGE_CL: ['CHANGE_YO'],
  CHANGE_AN: ['FINISH_AN', 'RISKCHARGE', 'FINISH_AN2'], CHANGE_HL: ['CHANGE_FL'], CHANGE_FL: ['FINISH_FL'], CHANGE_DW2: ['CHANGE_BZ2'], CHANGE_CL2: ['CHANGE_SK'], CHANGE_HM2: ['CHANGE_DR2'],
  CHANGE_LF: ['CHANGE_TF'], CHANGE_DF: ['CHANGE_TF2'], CHANGE_TF: ['FINISH_TF'], CHANGE_TF2: ['FINISH_TF2'],
  CHANGE_L1: ['LM_STUNIMPACT', 'LM_SONICRAVE', 'LM_LASTARCANUM', 'LM_UNIONDISCODE'], CHANGE_L2: ['LM_STUNIMPACT', 'LM_SONICRAVE', 'LM_LASTARCANUM', 'LM_UNIONDISCODE'], CHANGE_L5: ['LM_STUNIMPACT', 'LM_SONICRAVE', 'LM_LASTARCANUM', 'LM_UNIONDISCODE'] };
// START: StartStyle's per-form tail (0x14141ACC9 ...), for the F-packages' `on_start` (meaning of the fields [C]).
const START = {
  CHANGE_DW: 'ctrl.vt+0x18B0(); ctrl+0x970 = pawn.vt+0x2980() x 2.0', CHANGE_DW2: 'ctrl.vt+0x18B0(); ctrl+0x970 = pawn.vt+0x2980() x 2.0', CHANGE_SW: 'ctrl+0x970 = pawn.vt+0x2980() x 2.0',
  CHANGE_HM: 'pawn flag 0x40000 (pawn+0xE60 bit 18)', CHANGE_YO: 'pawn flag 0x40000', CHANGE_DR: 'pawn flag 0x40000', CHANGE_CL: 'pawn flag 0x40000',
  CHANGE_BZ: 'ctrl+0x970 = pawn.vt+0x2980() x 3.0', CHANGE_BZ2: 'ctrl+0x970 = pawn.vt+0x2980() x 3.0',
  CHANGE_WA: 'pawn.vt+0xFB0(0x88), pawn.vt+0xFB0(0xA2); camera +0x804 = 1.35 (ctrl+0xAE0 = 1); ctrl+0x970 = pawn.vt+0x2980() x 2.0',
  CHANGE_AN: 'EndStyle of the current form first (slot 859(0, 0)), no weapon switch; ctrl+0x970 = pawn.vt+0x2980() x 2.0; pawn.vt+0x2008(0); HUD 0x1415A85E0(0x2C, 1); game flag 0x140EB4750(7, 0)',
  CHANGE_HL: 'ctrl+0x970 = pawn.vt+0x2980() x 2.0; pawn flag 0x40000', CHANGE_FL: 'pawn.vt+0xFB0(0xA6); pawn flag 0x40000', CHANGE_DR2: 'pawn.vt+0xFB0(0x9E)',
};
// The shipped movesets (kh-core combat/forms/mod.rs MOVESETS must agree; tests/form_framework.rs checks it). The Second
// Form family is the framework's own (ComboLimit + STYLEFINISH 0xAB..0xAD); 0xAE (Starlight's Ancient Light at finish level 3)
// has no body yet: its level is held at 2 by the pending rule.
const MOVESET = { CHANGE_L1: 'second_form', CHANGE_L2: 'second_form', CHANGE_L5: 'second_form',
  // (F-NANO) Nano Arms: kh-core combat/forms/nano.rs
  CHANGE_GM: 'nano',
  // (weapons F-SHIELD) Counter Shield and Frying Pan: kh-core combat/forms/shield.rs
  CHANGE_SH: 'shield', CHANGE_SH2: 'shield',
  // (F-RAGE) Rage Form: kh-core combat/forms/rage.rs
  CHANGE_AN: 'rage',
  // (weapons F-STAFF) Mirage Staff: kh-core combat/forms/staff.rs
  CHANGE_WA: 'staff',
  // (weapons F-CLAWS) Agile Claws / Twin Yo-Yos / Blizzard Claws / Blizzard Blades: kh-core combat/forms/claws.rs
  CHANGE_CL: 'claws', CHANGE_YO: 'claws', CHANGE_CL2: 'claws', CHANGE_SK: 'claws',
  // (F-ARROWGUN) Double Arrowguns / Magic Launcher / Hunny Blasters / Hunny Launcher: kh-core combat/forms/arrowgun.rs
  CHANGE_DW: 'arrowgun', CHANGE_BZ: 'arrowgun', CHANGE_DW2: 'arrowgun', CHANGE_BZ2: 'arrowgun',
  // (F-HAMMER) Hyper Hammer / Drill Punch / Boom Hammer / Clock Drill: kh-core combat/forms/hammer.rs
  CHANGE_HM: 'hammer', CHANGE_DR: 'hammer', CHANGE_HM2: 'hammer', CHANGE_DR2: 'hammer',
  // (F-SHIP) Highwind / Storm Flag: kh-core combat/forms/ship.rs
  CHANGE_HL: 'ship', CHANGE_FL: 'ship',
  // (F-ULTIMATE) Ultimate Form: kh-core combat/forms/ultimate.rs
  CHANGE_SW: 'ultimate',
  // (F-LIGHTDARK) Light Form / Dark Form / Double Form: kh-core combat/forms/lightdark.rs
  CHANGE_LF: 'lightdark', CHANGE_TF: 'lightdark', CHANGE_DF: 'lightdark', CHANGE_TF2: 'lightdark' };

// Rage Form's offer, traced (decompiled C) for F-RAGE: the gauge never offers it.
const RAGE_TRIGGER = 'HP loss: 0x14140CE00 (the pawn HP setter tail, after 0x1405B3600): not in a form 0x8B, ctrl+0x888 != 0x8B; pawn !vt+0x1FE0 !vt+0x1ED8 !vt+0x1EB0 !vt+0xA48 !vt+0x1C30 '
  + '(swimming) !vt+0x2540 (dying), new HP > 0, !vt+0x1E40, !GameFlag(7), the current state not in {26..28, 30, 61, 62, 73, 92, 95..97, 101, 103, 110, 112, 114, 115, 117, 119..124, 126..138, 163..167}; '
  + 'pawn+0xCBC (byte; its writer = how Rage Form is learned: NOT traced) set, new HP < old HP, pawn.vt+0x2538(); chance % = round(f x round(35 - 100 x HP / MaxHP) / 100) with f = 100 '
  + 'for the first time, 30 for the second (pawn+0xE5C count), 0 from the third; x pawn+0xCD0 in Critical Mode; GameFlag(10) forces 1000; rand() % 100 < chance -> NotifySetActionCommand(0x8B, -1). '
  + 'Also 0x1413EA5F0 offers it whenever !pawn.vt+0x1FE0 (a script / Blueprint entry). When the count resets was not traced.';
const RAGE_FINISH = 'SetStyleFinishCommand case 0x8B: FINISH_AN (0xA5), but RISKCHARGE (0xB3) while pawn+0x460 vt+0xA8 (Riskcharges used) < 3; StartStyle 0xB3: TryStyleFinish, then HP = max(HP / 2, 1) '
  + '(pawn.vt+0x17B0 / vt+0x1840) and the count + 1 (vt+0x27D0). With ctrl+0x888 == 0x8B (a mode where Sora is always in it) FINISH_AN2 (0x6D) at a full gauge. EndStyle case 0x8B: '
  + 'deletes 0xA5 / 0xB3 / 0x6D, anim slot vt+0x1D0(0, 0), weapon effects EndEffect(-1, 1), pawn.vt+0x23C8, vt+0x27D0(0) (count reset), vt+0x2010(0), GameFlag(10) test, vt+0x1E38, then the clip ANTI_01 (ANTI_02 when pawn.vt+0x1740) [C].';

// ------------------------------------------------------------------------------------------------ rows
const KEYS = new Map(KB.map((k) => [k.id, k]));
const byType = new Map(KB.map((k) => [k.weapon_def_value, k]));
const ROWS = [];
const SRC = 'sheets/port/kh3-keyblade-forms.json (TresEquipKeybladeData m_EquipMap; KH3 tables agent); sheets/entities/Load/Common/TresCmdData.json; re/decomp 1413F/1413F0ED0.c (AwardAtMax), '
  + '1413E/1413EE030.c (SetStyleFinishCommand), 14141/141414C70.c (SetFinishLevel), 14141/14141A9D0.c (StartStyle), 1413C/1413CBA70.c + 1413D/1413D3AC0.c (StyleChange Begin / Tick), 14141/14141CA20.c (EndStyle)';
const codeOf = (command) => command.replace(/^CHANGE_/, '');
const offerOf = (command) => { for (const [t, list] of Object.entries(OFFER)) { const i = list.indexOf(command); if (i >= 0) return { type: +t, stage: i + 1, list }; } return null; };
const commands = [...new Set(Object.values(OFFER).flat()), 'CHANGE_AN'];
for (const command of commands) {
  const cmd = kindOf(command), cr = cmdRow(cmd), off = offerOf(command);
  const stage = cr.Param1, notes = [];
  let conf = 'confirmed';
  let kb = null, forms = [];
  if (off) {
    kb = byType.get(off.type); must(kb, 'no Keyblade of weapon type ' + off.type);
    must(WEAPON[command] === off.type, command + ': StartStyle wants weapon type ' + WEAPON[command] + ', AwardAtMax offers it for ' + off.type);
    must(stage === off.stage, command + ': Param1 ' + stage + ' but offered at form level ' + (off.stage - 1));
    forms = KBF.filter((f) => f.keyblade === kb.id && f.command === command).sort((a, b) => a.grow - b.grow);
    must(forms.length > 0, command + ': no kh3-keyblade-forms row of ' + kb.id);
    if (forms.some((f) => f.ref_conf !== 'confirmed')) notes.push('kh3-keyblade-forms matched this command by name only (' + forms[0].ref_conf + '); AwardAtMax / StartStyle (weapon type ' + off.type + ') confirm it [C]');
  }
  const f = forms[0] || {};
  const finishName = FINISH[command] || (LEVELS[command] ? LEVELS[command][0] : '-');
  const finish = finishName === '-' ? 0 : kindOf(finishName);
  const levels = (LEVELS[command] || []).map(kindOf);
  if (f.finisher && f.finisher !== (FINISH[command] || '-')) notes.push('kh3-keyblade-forms (by name) says finisher ' + f.finisher + '; the code gives ' + (FINISH[command] || 'none') + ' (SetStyleFinishCommand 0x1413EE030: no case ' + hex(cmd) + ')');
  if (!FINISH[command] && !LEVELS[command] && f.finisher === '-' && stage === 2) notes.push('a second stage without a finish command');
  const next = off && off.stage === 1 && off.list[1] ? off.list[1] : '-';
  const first = off && off.stage === 2 ? off.list[0] : '-';
  const wearName = command === 'CHANGE_AN' ? 'TRES_WEAR_FORM_07' : f.wear_form;
  const wear = parseInt(wearName.replace('TRES_WEAR_FORM_', ''), 16);
  const fa = FORMABIL.get(wearName) || { abilities: [], text: [] };
  if (f.form_abilities) must(JSON.stringify(f.form_abilities) === JSON.stringify(fa.abilities), command + ': form abilities differ from p_ex001_FormAbility');
  const growUnits = forms.slice(1).flatMap((g) => g.units.map((u) => 'G' + g.grow + ':' + u));
  if (forms.length > 1) notes.push(forms.length + ' grow levels with different weapon parts (ETresWeaponGrowLvl); what raises the grow level was NOT traced (KH3_WEAPON_DATA.md 6.3): the framework keeps Form.grow, the F-package raises it');
  if (command === 'CHANGE_L2') notes.push('finish level 3 = LM_UNIONDISCODE (Ancient Light, 0xAE: SetFinishLevel in form 0x95) its STYLEFINISH body is slot 155 0x1413E2810 (states/style_finish.rs union_discode, sheets/port/sora-form-second.json; F-SECOND)');
  const conf2 = (off && forms.some((x) => x.ref_conf !== 'confirmed')) || command === 'CHANGE_AN' ? 'inferred' : conf;
  ROWS.push({
    id: 'form.' + codeOf(command), cmd, command, name: command === 'CHANGE_AN' ? 'Rage Form' : f.command_name, keyblade: kb ? kb.id : '-', keyblade_name: kb ? kb.name : '-',
    weapon_type: off ? off.type : 0, weapon_form: f.weapon_form || '-', weapon_form_value: f.weapon_form_value || 0, stage, first: first === '-' ? '-' : 'form.' + codeOf(first), next: next === '-' ? '-' : 'form.' + codeOf(next),
    next_cmd: next === '-' ? 0 : kindOf(next), grow_levels: Math.max(1, forms.length), time: cr.Param3 > 0 ? cr.Param3 : 30, command_time: cr.Param0, finish, finish_command: finishName, finish_levels: levels,
    end_delete: (ENDDEL[command] || []).map(kindOf), wear_form: wear, wear_form_name: command === 'CHANGE_AN' ? 'Anti' : f.wear_form_name, wear_at: AT_STYLE_CHANGE.has(command) ? 'style_change' : 'weapon_change',
    transform_clip: CLIP[command] || 'StyleChange', anim_sets: command === 'CHANGE_AN' ? ['/Game/Character/pc/p_ex001/anm/p_ex001_AnimSet13.p_ex001_AnimSet13'] : f.anim_sets, magic_set: f.magic_set || '-', projectile_set: f.projectile_set || '-',
    projectiles: f.projectiles || [], units: f.units || [], grow_units: growUnits, transform_units: f.transform_units || [], abilities: fa.abilities, text_abilities: fa.text, moves: f.moves || [], move_names: f.move_names || [],
    wear_textures: f.wear_textures || [], ui_asset: f.ui_asset || '-', act_logo: f.act_logo || [], charge_effect: f.charge_effect || '-', start_extra: START[command] || '-',
    trigger: command === 'CHANGE_AN' ? RAGE_TRIGGER : 'gauge: AwardAtMax 0x1413F0ED0 offers it for weapon type ' + off.type + ' at form level ' + (off.stage - 1),
    moveset: MOVESET[command] || 'pending',
    source: SRC + (command === 'CHANGE_AN' ? '; Rage Form: 14140/14140CE00.c, 1413E/1413EA5F0.c, p_ex001_FormAbility m_Data[TRES_WEAR_FORM_07], p_ex001_AnimSet13 (KH3_WEAPON_ASSETS.md 6b)' : ''),
    ref_conf: conf2, note: [...notes, ...(command === 'CHANGE_AN' ? [RAGE_FINISH, 'no Keyblade: KH3 enters it from any Keyblade and from inside another form; no form-01 equipment row; the look is wear form 7 (point pattern p_ex001_7f00, KH3_WEAPON_ASSETS.md 6b)'] : [])].join('; ') || '-',
  });
}

// ------------------------------------------------------------------------------------------------ checks
must(ROWS.length === 27, 'expected 27 forms (26 Keyblade formchanges + Rage Form), got ' + ROWS.length);
for (const r of ROWS) {
  if (r.next !== '-') must(ROWS.find((x) => x.id === r.next && x.first === r.id), r.id + ': next ' + r.next + ' does not lead back');
  if (r.stage === 2) must(r.finish !== 0, r.id + ': a second stage needs a finish command');
  must(r.end_delete.length > 0, r.id + ': EndStyle deletes nothing');
  must(r.end_delete.includes(r.next_cmd || r.finish), r.id + ': EndStyle does not delete its follow-up');
}
for (const k of KB) for (const c of k.formchanges) must(ROWS.find((r) => r.command === c && r.keyblade === k.id), k.id + ' formchange ' + c + ' has no row');

// ------------------------------------------------------------------------------------------------ sheet
const S = (n, t, g, doc) => [n, t, g, doc];
const COLS = [
  S('id', 'str', 'str', 'form.<code> (the CHANGE_<code> command)'), S('cmd', 'int', 'i32', 'the formchange command (ETresCommandKind value)'), S('command', 'str', 'str', 'its name'), S('name', 'str', 'str', 'English (the command\'s LocKey text)'),
  S('keyblade', 'str', 'str', 'the Keyblade (kh3-keyblades id) whose form it is, - for Rage Form'), S('keyblade_name', 'str', 'str', 'English'),
  S('weapon_type', 'int', 'u32', 'KH3\'s weapon type of that Keyblade (= its ETresItemDefWeapon value; AwardAtMax / StartStyle switch on it); 0 = none'),
  S('weapon_form', 'str', 'str', 'ETresWeaponForm of the equipment variant'), S('weapon_form_value', 'int', 'u32', 'its value'),
  S('stage', 'int', 'i32', 'form level = TresCmdData Param1: 1 first stage, 2 second stage'), S('first', 'str', 'str', 'a second stage: the first-stage form it is offered in, else -'),
  S('next', 'str', 'str', 'a first stage: the form a full gauge offers in it (AwardAtMax at form level 1), else -'), S('next_cmd', 'int', 'i32', 'that form\'s command, 0 = none'),
  S('grow_levels', 'int', 'u32', 'ETresWeaponGrowLvl variants of its weapon (1..3)'), S('time', 'num', 'f32', 'form time, s (TresCmdData Param3; 30 when 0)'), S('command_time', 'num', 'f32', 'how long the command stays offered, s (Param0)'),
  S('finish', 'int', 'i32', 'its finish command (the first finish level for the Second Form family), 0 = none'), S('finish_command', 'str', 'str', 'its name or -'),
  S('finish_levels', 'list', 'i32[]', 'the Second Form family: the finish command of finish level 1, 2, 3 (SetFinishLevel); else empty'),
  S('end_delete', 'list', 'i32[]', 'the commands EndStyle deletes for it (its next stage\'s offer or its finish commands)'),
  S('wear_form', 'int', 'u32', 'ETresCharWearForm value Sora wears (5 Second Form, 7 Rage Form ...)'), S('wear_form_name', 'str', 'str', 'p_ex001_WearformSet m_FormName'),
  S('wear_at', 'enum:style_change|weapon_change', 'str', 'when the transformation applies it: style_change = STYLECHANGE sub 3 (Second Form family, Light / Dark / Double Form), weapon_change = with the weapon variant right after its clip starts'),
  S('transform_clip', 'str', 'str', 'the transformation clip (StyleChange Tick sub 0)'), S('anim_sets', 'list', 'str[]', 'm_ReplaceAnimSets: the replacement anim sets of the form'),
  S('magic_set', 'str', 'str', 'm_ReplaceMagicSet'), S('projectile_set', 'str', 'str', 'm_ReplaceProjectileSet'), S('projectiles', 'list', 'str[]', 'its projectile kinds (name=blueprint)'),
  S('units', 'list', 'str[]', 'weapon parts at grow level 1: unit@part:socket=weapon blueprint'), S('grow_units', 'list', 'str[]', 'weapon parts of grow levels 2 / 3: G<n>:unit@part:socket=blueprint'),
  S('transform_units', 'list', 'str[]', 'm_TransformSetData: the Keyblade\'s transform part'), S('abilities', 'list', 'str[]', 'p_ex001_FormAbility m_Ability of the wear form (ETresAbilityKind; a repeat = another copy)'),
  S('text_abilities', 'list', 'str[]', 'p_ex001_FormAbility m_TextAbility (TEXT_*)'), S('moves', 'list', 'str[]', 'TresKeybladeAbilityData m_FormAbilityMap: the moves the Abilities screen lists'), S('move_names', 'list', 'str[]', 'English'),
  S('wear_textures', 'list', 'str[]', 'Sora\'s costume texture swaps (material=texture)'), S('ui_asset', 'str', 'str', 'm_UIDataAsset'), S('act_logo', 'list', 'str[]', 'the formchange banner movies'),
  S('charge_effect', 'str', 'str', 'm_CmnFormChangeChargeEffect'), S('start_extra', 'str', 'str', 'StartStyle\'s per-form tail (for the F-package\'s on_start)'),
  S('trigger', 'str', 'str', 'what offers the command'), S('moveset', 'str', 'str', 'second_form = the framework\'s own; pending = no moveset yet (never offered, refused); else the F-package'),
  S('source', 'str', null, '-'), S('ref_conf', 'enum:confirmed|inferred|guess', null, '-'), S('note', 'str', null, '-'),
];
const NAME = 'sora-keyblade-forms';
const rows = ROWS.map((r) => COLS.map((c) => r[c[0]]));
for (const r of rows) for (const v of r) must(v !== undefined && v !== null, 'a null cell');
fs.writeFileSync(P('sheets', 'port', NAME + '.json'), '{"sheet":"' + NAME + '","group":"port","columns":' + JSON.stringify(COLS.map((c) => c[0])) + ',"rows":[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
console.log('sheets/port/' + NAME + '.json: ' + rows.length + ' forms (' + ROWS.filter((r) => r.moveset !== 'pending').length + ' with a moveset)');

// ------------------------------------------------------------------------------------------------ register (this tool's entry only; replaced in place)
const q = (s) => "'" + String(s).replace(/\\/g, '\\\\').replace(/'/g, "\\'") + "'";
const NL = '\n';
const DOC = 'One KH3 formchange as data for the form framework (kh-core combat/form.rs): the Keyblade, command, level chain, time, finish commands, wear form, form abilities, transformation clip, replacement anim set, projectiles and weapon parts.';
const MARK = '(weapons P3)';
function upsert(s, name, text, end, pos) {
  const at = s.indexOf("'" + name + "': {");
  if (at < 0) { const p = pos(s); return s.slice(0, p) + text + s.slice(p); }
  const c = s.lastIndexOf(NL, at - 1) + 1, cs = s.lastIndexOf(NL, c - 2) + 1;
  // (P1 declared this sheet up front as an id / value placeholder, "(weapons P1) P3: ..."; the forms are a TABLE (gen_port
  // makes no table of an id / value sheet), so this tool's declaration replaces that placeholder: P1's STATE.md is told.)
  must(s.slice(cs, c).includes(MARK) || s.slice(cs, c).includes('(weapons P1) P3:'), 'the entry of ' + name + ' is not this tool\'s (no "' + MARK + '" comment above it)');
  const e = s.indexOf(end, at); must(e > at, 'end of the entry of ' + name);
  return s.slice(0, cs) + text + s.slice(e + end.length);
}
const save = (f, before, after, what) => { if (after !== before) { fs.writeFileSync(f, after); console.log('registered ' + NAME + ' in ' + what); } };
{
  const f = P('tools', 'lib', 'sheet_defs.js'); const before = fs.readFileSync(f, 'utf8');
  const END = '] },' + NL;
  const pos = (t) => { const at = t.indexOf("  'kh3-keyblade-forms': {"); must(at >= 0, 'sheet_defs.js: anchor kh3-keyblade-forms not found'); const e = t.indexOf(END, at); return e + END.length; };
  const text = '  // ' + MARK + ' ' + DOC + ' Written by tools/sora_keyblade_forms.js (documentation/KH3_WEAPONS_PLAN.md 1.3)' + NL +
    '  ' + q(NAME) + ": { group: 'port', file: " + q(NAME + '.json') + ", by: 'tools/sora_keyblade_forms.js', from: " + q('kh3-keyblade-forms / kh3-keyblades, TresCmdData and the decompiled StartStyle / AwardAtMax / SetStyleFinishCommand / StyleChange / EndStyle') + ', external: true, cols: [' + NL +
    '    ' + COLS.map((c, i) => '[' + q(c[0]) + ', ' + (i === 0 ? 'K' : 'V') + ', ' + q(c[1]) + ", '-', " + q(c[3]) + (['source', 'note', 'trigger', 'start_extra'].includes(c[0]) ? ', true' : '') + ']').join(', ') + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/sheet_defs.js');
}
{
  const f = P('tools', 'lib', 'port_gen_defs.js'); const before = fs.readFileSync(f, 'utf8');
  const END = NL + '    },' + NL;
  const pos = (t) => { const anchor = '  tables: {' + NL; const at = t.indexOf(anchor); must(at >= 0, 'port_gen_defs.js: anchor tables'); return at + anchor.length; };
  const cols = COLS.filter((c) => c[2]).map((c) => c[0] + ': { type: ' + q(c[2]) + (c[3] && c[3] !== '-' ? ', doc: ' + q(c[3]) : '') + ' }').join(', ');
  const text = '    // ' + MARK + ' ' + NAME + ': written by tools/sora_keyblade_forms.js (documentation/KH3_WEAPONS_PLAN.md 1.3)' + NL +
    '    ' + q(NAME) + ': {' + NL +
    "      file: 'sora_keyblade_forms.rs', row: 'KeybladeFormRow', table: 'KEYBLADE_FORMS', key: 'id', key_prefix: 'KBFORM'," + NL +
    '      doc: ' + q(DOC) + ',' + NL +
    '      columns: { ' + cols + ' },' + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/port_gen_defs.js');
}
console.log('sora_keyblade_forms ' + VERSION + ': ' + ROWS.map((r) => r.id + (r.next !== '-' ? '->' + r.next : '')).join(' '));
