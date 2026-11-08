// (weapons F-RAGE) KH3's Rage Form (form.AN, CHANGE_AN 0x8B) as DATA for its form package (kh-core combat/forms/rage.rs,
// states/combo_anti.rs): the HP-loss offer, the cooldown and the use counts, the attack kinds of COMBO_ANTI (0xBF .. 0xC6),
// the replacement anim set (p_ex001_AnimSet13: logical name -> clip), the finish commands RISKCHARGE (0xB3) / FINISH_AN
// (0xA5) / FINISH_AN2 (0x6D), the aura (ead_p_ant_com: effect groups -> particle @ socket), the attack rows and the states
// the offer is not made in. documentation/KH3_WEAPONS_PLAN.md 6e (package F-RAGE), build/sora_tmp/weapons/F-RAGE/STATE.md.
//
//   node tools/sora_form_rage.js   write sheets/port/sora-form-rage.json and register it (once, in place) in
//                                  tools/lib/sheet_defs.js and tools/lib/port_gen_defs.js; then node tools/gen_port.js
//
// Inputs (read only): sheets/port/sora-keyblade-forms.json (P3: the form row), build/sora_pack/keyblades/
// sora_forms_manifest.json (AnimSet13's rows, clips, notifies), build/sora_pack/manifest.json and the XS overlay
// (build/sora_pack/xsi_manifest.json: D417 / D419 / D420), sheets/entities/DataTable/Player/Attack/p_ex001_AttackData.json,
// Content/Effects/wep/anti/ead/ead_p_ant_com (the aura), and THE CODE TABLES BELOW, read in the decompiled exe (re/decomp):
//   OFFER    0x14140CE00 (the pawn's HP setter tail, +0x448 sub-object vtable 0x14696E4B8 slot +0x220): the HP-loss roll.
//   GATE     pawn vt+0x1FE0 0x141436580 (Rage blocked): the style-change save bit (gi+0x870)+0x95B0 & 8
//            (BP_SetAbilityStyleChange), pawn+0xE60 bit 14 (BP_SetRestrictAntiForm), restrict action STYLE (6).
//   LEARN    pawn+0xCBC = ATresPlayerPawnBase.MyAntiFormGenerate: false in the base ctor 0x141423210, TRUE in the
//            ATresPlayerPawnSora ctor 0x1413E7580 (line 242): no learn event; no asset sets it.
//   COUNT    pawn+0xE5C (setter vt+0x27D8 0x141111240, getter vt+0x27E0): +1 in StyleChange 0x1413CE760 case 0x8B,
//            0 in SetBattleMode(false) 0x141446CE0 (pawn vt+0x1738). Riskcharges pawn+0xE58 (vt+0x27D0 0x1411127F0,
//            getter +0x460 vt+0xA8 0x141109C30): 0 in 0x1413CE760 case 0x8B and in EndStyle, +1 in StartStyle 0x46.
//   COOL     EndStyle 0x14141CA20 case 0x8B (bytes 0x14141CD37 .. 0x14141CD6B): vt+0x1E38(pawn+0xC74
//            MyAntiFormCoolDownTime = 90; 9.0 with RageFever) -> pawn+0xE40, ticked by the pawn tick 0x141449CD0;
//            vt+0x1E40 (0 < pawn+0xE40) blocks the offer.
//   ENTER    StyleChange 0x1413CE760 case 0x8B: anim vt+0x348(0x6B); Riskcharges = 0; count + 1; HP = MaxHP (vt+0x1878).
//   DECIDE   TresPlayerState_ComboAnti::Decide 0x141360000 (data in the Rust); MaxCombo 0x141433570 case 0x8B = 5,
//            Finishers 0x141433A60 case 0x8B = 1.
//   PLAY     TresPlayerState_Attack::PlayAttack 0x1413378B0 cases 0xBE .. 0xC6 (lines 1517 .. 1627).
//   WARP     ComboAnti slot 136 0x141352140 = Attack's 0x141337320 with 200.0 for the drop (the warp of 0xC2 M_D414);
//            ComboAnti 0x141366300: at M_D414's end back to the start point, m_bFaceAt, M_D421.
//   FINISH   StyleFinish 0x1413DEBC0 (FINISH_AN, M_D418), 0x1413E2A10 (RISKCHARGE, M_D419, effect group = Riskcharges
//            + 9), 0x1413DE5F0 (FINISH_AN2, M_D422 / M_D423 / M_D424: only while ctrl+0x888 == 0x8B, the scripted
//            BP_NotifyStartSpecialAntiForm of bt_04_camera_assension). StartStyle 0x14141A9D0 case 0x46 (RISKCHARGE):
//            CanStyleFinish, TryStyleFinish, HP = max(HP / 2, 1), Riskcharges + 1 (no form time / gauge reset); the
//            end event 0x14150C440 skips slot 859 (the form's end) for 0xB3.
//   END      EndStyle 0x14141CA20 case 0x8B: deletes 0xA5 / 0xB3 / 0x6D, EndEffect, pawn vt+0x23C8 (the aura), Riskcharges
//            = 0, the cooldown, the face clip ANTI_01 (ANTI_02 when pawn vt+0x1740) on pawn+0x440 (not in the pack).
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
const AN = sheet('sora-keyblade-forms').find((r) => r.id === 'form.AN');
must(AN, 'sora-keyblade-forms has no form.AN');
const OV = readJson(P('build', 'sora_pack', 'keyblades', 'sora_forms_manifest.json'));
const MAIN = readJson(P('build', 'sora_pack', 'manifest.json'));
const XSI_FILE = [P('build', 'sora_pack', 'xsi_manifest.json'), P('build', 'sora_tmp', 'xsi', 'pack', 'xsi_manifest.json')].find((f) => fs.existsSync(f));
const XSI = XSI_FILE ? readJson(XSI_FILE) : { items: [], logical_animations: [] };
const ATK = readJson(P('sheets', 'entities', 'DataTable', 'Player', 'Attack', 'p_ex001_AttackData.json')).rows;
const SET = 'p_ex001_AnimSet13';
const setName = (a) => String(a).split('/').pop().split('.')[0];
must(AN.anim_sets.map(setName).includes(SET), 'form.AN does not name ' + SET);
const ITEMS = new Map([...MAIN.items, ...XSI.items, ...OV.items].filter((i) => i.kind === 'animation').map((i) => [i.animation_name, i]));
const XS_ROWS = new Set(XSI.logical_animations.map((a) => a.name));
const ROWS13 = new Map(OV.logical_animations.filter((a) => setName(a.anim_set) === SET).map((a) => [a.name, a]));
must(ROWS13.size > 50, SET + ' has ' + ROWS13.size + ' rows in the form overlay');
// the animation-table row a clip plays as: the overlay's sequence (tools/gen_port.js FORM_ANIM_SETS), else the XS row
// M_<seq> (D417 / D419 / D420 came with the XS abilities), else the logical name itself.
const tableRow = (logical) => {
  const r = ROWS13.get(logical);
  if (r && r.file && OV.items.some((i) => i.animation_name === r.clip)) return r.clip;
  const seq = r ? r.clip : logical.replace(/^M_/, '');
  if (XS_ROWS.has('M_' + seq)) return 'M_' + seq;
  return r ? r.clip : '-';
};
const seqOf = (logical) => (ROWS13.get(logical) || {}).clip || logical.replace(/^M_/, '');
const prop = (n, key) => { const p = n.properties || {}; const v = p[key]; return v && typeof v === 'object' && 'value' in v ? v.value : v; };
// the notifies of a clip: hit windows, their attack rows, TresEnableChange / TresEnableInput, shots, effects, sounds.
function notes(seq) {
  const it = ITEMS.get(seq);
  const out = { length: it ? r4(it.duration_s) : 0, windows: [], rows: [], change: -1, input: -1, shots: [], effects: [], sounds: [] };
  if (!it) return out;
  for (const n of it.notifies || []) {
    if (n.class === 'TresAnimNotifyState_Attack') { out.windows.push(r4(n.time) + '-' + r4(n.end_time)); out.rows.push(String(prop(n, 'm_AttackDataIDName'))); }
    if (n.class === 'TresAnimNotify_EnableChange' && out.change < 0) out.change = r4(n.time);
    if (n.class === 'TresAnimNotify_EnableInput' && out.input < 0) out.input = r4(n.time);
    if (n.class === 'TresAnimNotify_Shot') out.shots.push(r4(n.time) + '@' + (prop(n, 'm_ProjectileName') || ('location ' + (prop(n, 'LocationIndex') || 0))));
    if (/AttachEffect/.test(n.class)) {
      const s = JSON.stringify(n.properties || {});
      const re = /"GroupID","type":"IntProperty","value":(-?\d+)[\s\S]*?"AttachEffect","type":"ObjectProperty","value":"([^"]*)"/g;
      let m; while ((m = re.exec(s))) if (m[2] !== 'null') out.effects.push(r4(n.time) + '@' + m[1] + ':' + m[2].split('.').pop());
    }
    if (/PlaySound|AttachSound|PlayVoice/.test(n.class)) { const s = JSON.stringify(n.properties || {}); const m = s.match(/\/Game\/Sound[^"]*/); out.sounds.push(r4(n.time) + '@' + (m ? m[0].split('.').pop() : n.class.replace(/^.*Notify(State)?_/, ''))); }
  }
  out.effects = [...new Set(out.effects)];
  return out;
}
const atkRow = (id) => ATK.find((r) => r._row_name === id || String(r.id).toLowerCase() === String(id).toLowerCase());

// ------------------------------------------------------------------------------------------------ the code tables
// RULES: the numbers of the offer, the counts, the cooldown, the combo and the warp (addresses above).
const RULES = [
  { key: 'offer_hp_share', v: [0.25], at: '0x1405AD600', note: 'pawn vt+0x2538 (thunk 0x141435F10): HP <= MaxHP x 25 / 100 (integer: (MaxHP * 0x19) / 100): the offer is rolled only at or below it' },
  { key: 'offer_base', v: [35], at: '0x14140CE00', note: 'the chance in % = RoundToInt(f x RoundToInt((MaxHP x 35 - HP x 100) / MaxHP) x 0.01) (RoundToInt = floor(x + 0.5): the -((int)ROUND(-0.5 - 2x) >> 1) idiom)' },
  { key: 'offer_factor', v: [100, 30, 0], at: '0x14140CE00', note: 'f by the Rage count pawn+0xE5C (param_1 + 0xA14): 100 at 0, 30 at 1, 0 from 2' },
  { key: 'offer_critical_rate', v: [0.5], at: '0x1405D0250', note: 'pawn vt+0x8D0 (difficulty Critical: gi+0x8660 > 2): f = (int)(f x pawn+0xCD0 CriticalModeAntiFormGenerateRate = 0.5)' },
  { key: 'offer_fever', v: [1000], at: '0x140E8E970', note: 'FUN_1405AD350(pawn, 10) = ETresDlcCheatCode 10 RageFever (Re Mind Premium Menu): f = 1000 (a certain offer); no seam in the port: off' },
  { key: 'cooldown', v: [90, 9], at: '0x14141CD37', note: 'EndStyle case 0x8B: pawn+0xE40 = MyAntiFormCoolDownTime (pawn+0xC74, class default 90 s; 9 s with RageFever); the pawn tick 0x141449CD0 counts it down; vt+0x1E40 (0 < pawn+0xE40) blocks the offer' },
  { key: 'riskcharge_max', v: [3], at: '0x1413EE030', note: 'SetStyleFinishCommand case 0x8B: RISKCHARGE (0xB3) while the Riskcharges (pawn+0xE58) < 3, else FINISH_AN (0xA5)' },
  { key: 'riskcharge_hp', v: [0.5, 1], at: '0x14141A9D0', note: 'StartStyle case 0x46: HP = max(HP / 2 (integer), 1); no form-time or gauge reset (the other finish kinds zero both)' },
  { key: 'riskcharge_group', v: [9], at: '0x1413E2A10', note: 'RISKCHARGE plays M_D419 with the effect group Riskcharges + 9 (10 / 11 / 12: ead_p_ant_com vp_ant_rsk_crg_00 / 01 / 00 @ vfx_center) when the count is > 0' },
  { key: 'enter_hp', v: [1], at: '0x1413CE760', note: 'StyleChange case 0x8B: HP = MaxHP (pawn vt+0x1878(MaxHP, 0, 0, 0, 0)); anim vt+0x348(0x6B)' },
  { key: 'combo', v: [5, 1], at: '0x141433570', note: '[MaxCombo 0x141433570 case 0x8B, Finishers 0x141433A60 case 0x8B]' },
  { key: 'decide_near', v: [400, 1000, -100], at: '0x141360000', note: 'Decide: reach at combo 0 (dist > 400 -> 1, else 2 with more than one enemy near); the warp approach (forced kind 0, dist <= 1000 and (dist > 400 or the target high or (air and dz < -100))): +0x1E1 = 1, +0x25C = 0.5, +0x22C = 0x11' },
  { key: 'warp', v: [150, 200, 0.5], at: '0x141352140', note: 'the teleport of the approach and of 0xC2 (Attack 0x141337320 through ComboAnti slot 136): 150 cm short of the target on the line from him, 200 cm below it (the base class: 100); +0x25C = 0.5: the invincibility after it ends at 0.5 s or at TresEnableChange (Attack Tick 0x141341340)' },
];
// PLAY: PlayAttack 0x1413378B0 cases 0xBE .. 0xC6 (decompiled C): logical name; root length +0x1D8; +0x234 bit 0 the XY
// channel / bit 1 the root-scale channel; +0x1F4 bit 4 (keep face); +0x1DF the air mode; +0x24C the stop distance.
const PLAY = [
  { kind: 0xBE, logical: 'M_D419', flags: ['not_decided'], label: 'Riskcharge as an attack kind (no Decide branch gives it; RISKCHARGE is the finish command)', at: '0x141339A8C' },
  { kind: 0xBF, logical: 'M_D411', root_len: 490, chan_root: true, keep: true, air: true, label: 'ground 1 / first: Crimson Upper', at: '0x141339A9E' },
  { kind: 0xC0, logical: 'M_D412', root_len: 600, chan_root: true, keep: true, air: true, label: 'ground 2: Vertical Drop', at: '0x141339AE6' },
  { kind: 0xC1, logical: 'M_D413', root_len: 500, chan_root: true, keep: true, air: true, label: 'ground 3 / reach 2: Blade Turn', at: '0x141339B2E' },
  { kind: 0xC2, logical: 'M_D414', keep: true, air: true, flags: ['warp', 'return'], label: 'from combo 3: Warp Thrust (teleport to the target, at its end back to the start point and M_D421)', at: '0x141339B76' },
  { kind: 0xC3, logical: 'M_D415', root_len: 570, chan_root: true, keep: true, air: true, label: 'air 1: Speed Slash', at: '0x141339CF1' },
  { kind: 0xC4, logical: 'M_D416', root_len: 600, chan_root: true, keep: true, air: true, label: 'air 2: Vertical Slash', at: '0x141339D2F' },
  { kind: 0xC5, logical: 'M_D417', root_len: 440, chan_root: true, keep: true, air: true, flags: ['finish'], label: 'the finisher: Crimson Blast', at: '0x141339DC9' },
  { kind: 0xC6, logical: 'M_D420', root_len: 500, keep: true, air: true, stop: 500, label: 'the counter: Rage Raid (its forcer was not found: the XS Rage Raid offers it, kind 0xEB)', at: '0x141339E11' },
];
// FINISH: the StyleFinish bodies, by sub-step.
const FINISH = [
  { cmd: 0xA5, name: 'FINISH_AN', logical: 'M_D418', at: '0x1413DEBC0', note: 'sub 0: m_bFaceAt, M_D418 (mode 2, blend -1); sub 1: the first hit window -> ForceFeedback(1), sub 5; sub 1 / 5: more than 7 shots (+0x170) -> WorldSlow off; at TresEnableChange gravity on (vt+0x1728(1)), Falling (vt+0x1658), sub 12; at its end DoChain. The Shot notify spawns AN_FINISH (p_ex001_ProjectileSet, row AN_FINISH_BOMB). Ends the form.' },
  { cmd: 0xB3, name: 'RISKCHARGE', logical: 'M_D419', at: '0x1413E2A10', note: 'sub 0: M_D419 with the effect group Riskcharges + 9 (> 0), m_bFaceAt; sub 1: the first hit window -> WorldSlow off, ForceFeedback(1), sub 2; at TresEnableChange gravity on, Falling, sub 12; at its end DoChain. The form goes on (0x14150C440 skips slot 859 for 0xB3).' },
  { cmd: 0x6D, name: 'FINISH_AN2', logical: 'M_D422', at: '0x1413DE5F0', flags: ['not_offered'], note: 'Discension (M_D422 -> M_D423 -> M_D424, rows AN_DISCENSION / _END): SetStyleFinishCommand gives it only while ctrl+0x888 == 0x8B (BP_NotifyStartSpecialAntiForm, the scripted bt_04 Rage of the story): never in Elden Ring.' },
];
// the aura: ead_p_ant_com (SQEX_ParticleAttachDataAsset, m_AntiFormEffect of the pawn): group -> particle @ socket.
function aura() {
  const d = readJson(P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content', 'Effects', 'wep', 'anti', 'ead', 'ead_p_ant_com.uasset.json'));
  // (F-RAGE fix) every SQEX_StructParticleAttachData row in document order: its SocketName, LocationOffset, ScaleOffset,
  // ParticleColor, ParticleAlpha and each EffectDataList item (GroupID, AttachEffect) [A: the asset].
  const flat = (props) => Object.fromEntries((props || []).map((p) => [p.name, p.value]));
  const lists = [];
  const walk = (v) => {
    if (Array.isArray(v)) {
      if (v.length && v.every((x) => x && typeof x === 'object' && 'name' in x && 'type' in x)) lists.push(v);
      v.forEach(walk);
    } else if (v && typeof v === 'object') Object.values(v).forEach(walk);
  };
  walk(d.exports);
  const out = [];
  for (const l of lists) {
    const o = flat(l);
    if (!('SocketName' in o) || !o.EffectDataList) continue;
    const v3 = (x, dflt) => (x ? [x.x, x.y, x.z] : dflt);
    for (const it of o.EffectDataList.items.map(flat)) {
      const imp = d.imports[-Number(it.AttachEffect) - 1];
      out.push({ group: Number(it.GroupID), effect: imp ? imp.objectName : '?', socket: o.SocketName || 'None', loc: v3(o.LocationOffset, [0, 0, 0]), scale: v3(o.ScaleOffset, [1, 1, 1]), color: v3(o.ParticleColor, [1, 1, 1]), alpha: o.ParticleAlpha ?? 1 });
    }
  }
  return out;
}
// THE STATES the offer is not made in (0x14140CE00: the current ETresStateID FUN_1405ABE20 - 0x1A in the switch's cases).
const EXCLUDED = [26, 27, 28, 30, 61, 62, 73, 92, 95, 96, 97, 101, 103, 110, 112, 114, 115, 117, 119, 120, 121, 122, 123, 124, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 163, 164, 165, 166, 167];

// ------------------------------------------------------------------------------------------------ rows
const ROWS = [];
const blank = { code: 0, logical: '-', clip: '-', length: 0, root_len: 0, stop_dist: 160, z_gain: 3, air_mode: false, keep_face: false, chan_xy: true, chan_root: false, flags: [], windows: [], rows: [], effects: [], sounds: [], values: [], ref_addr: '-', ref_conf: 'confirmed', note: '-' };
const add = (r) => ROWS.push({ ...blank, ...r });
for (const r of RULES) add({ id: 'rule.' + r.key, what: 'rule', logical: r.key, values: r.v, ref_addr: r.at, ref_conf: r.key === 'enter_hp' || r.key === 'warp' ? 'inferred' : 'confirmed', note: r.note });
add({ id: 'rule.excluded_states', what: 'rule', logical: 'excluded_states', values: EXCLUDED, ref_addr: '0x14140CF6A', note: 'ETresStateID values: the rides, CATCH / BIND, the team attacks, the bad statuses STUN / FREEZE / BURN ... (the switch of 0x14140CE00)' });
for (const k of PLAY) {
  const seq = seqOf(k.logical);
  const clip = tableRow(k.logical);
  must(clip !== '-', k.logical + ': no clip');
  const n = notes(seq);
  for (const r of n.rows) must(atkRow(r), k.logical + ': p_ex001_AttackData has no row ' + r);
  add({ id: 'attack.' + hex(k.kind), what: 'attack', code: k.kind, logical: k.logical, clip, length: n.length, root_len: k.root_len || 0, stop_dist: k.stop || 160, air_mode: !!k.air, keep_face: !!k.keep, chan_xy: true, chan_root: !!k.chan_root, flags: k.flags || [],
    windows: n.windows, rows: n.rows, effects: [...n.effects, ...n.shots.map((s) => 'shot ' + s)], sounds: n.sounds, values: [n.change, n.input], ref_addr: k.at, note: k.label + '; values = [TresEnableChange, TresEnableInput] of the clip (-1 none)' });
}
// M_D414's return (ComboAnti 0x141366300)
{
  const n = notes(seqOf('M_D421'));
  add({ id: 'chain.M_D421', what: 'chain', code: 0xC2, logical: 'M_D421', clip: tableRow('M_D421'), length: n.length, windows: n.windows, rows: n.rows, effects: n.effects, sounds: n.sounds, values: [n.change, n.input], ref_addr: '0x141366300', note: 'at M_D414\'s end (anim vt+200) with the pawn able (vt+0x968): the move back to the start point (+0x240, swept), m_bFaceAt, M_D421 (blend -1), voice group 0' });
}
for (const f of FINISH) {
  const seq = seqOf(f.logical);
  const n = notes(seq);
  add({ id: 'finish.' + f.name, what: 'finish', code: f.cmd, logical: f.logical, clip: tableRow(f.logical), length: n.length, flags: f.flags || [], windows: n.windows, rows: n.rows, effects: [...n.effects, ...n.shots.map((s) => 'shot ' + s)], sounds: n.sounds, values: [n.change, n.input], ref_addr: f.at, note: f.note });
}
// the replacement anim set (logical name -> clip)
for (const [logical, r] of [...ROWS13].sort((a, b) => (a[0] < b[0] ? -1 : 1))) {
  const clip = tableRow(logical);
  add({ id: 'anim.' + logical, what: 'anim', logical, clip, length: ITEMS.get(r.clip) ? r4(ITEMS.get(r.clip).duration_s) : 0, flags: [SET, r.class], note: r.file ? '-' : 'a ' + r.class + ' of the set: not a row of the animation table (its samples are)' });
}
// the aura
for (const [i, a] of aura().entries()) add({ id: 'aura.' + a.group + '.' + i + '.' + a.effect + '.' + a.socket, what: 'aura', code: a.group, logical: a.effect, clip: a.socket,
  // (F-RAGE fix) the row's own placement: [LocationOffset x, y, z, ScaleOffset x, ParticleColor r, g, b, ParticleAlpha]
  values: [...a.loc.map(r4), r4(a.scale[0]), ...a.color.map(r4), r4(a.alpha)], ref_addr: a.group === 100 ? '0x1413CBA70' : a.group >= 10 && a.group <= 13 ? '0x14140E7F0' : '-', ref_conf: 'confirmed',
  // (F-POLISH) traced: FUN_1405B9370(pawn+0x440, m_AntiFormEffect, group, 0) calls of 0x1405 .. 0x1415.
  note: 'ead_p_ant_com (m_AntiFormEffect): group ' + a.group + (a.group === 100 ? ': the aura of the form (StyleChange Begin 0x1413CBA70 at 0x1413CBD38, tick 0x1413D3AC0 at 0x1413D3DFB, 0x1413CE760 at 0x1413CE9B9; restored by ctrl vt+0x1B00 0x14140E7F0)' : a.group === 101 ? ': passed by no code (not started)' : a.group >= 10 && a.group <= 12 ? ': RISKCHARGE\'s group (count + 9, 0x1413E2A10); lasting: restored for count >= group - 9 by 0x14140E7F0' : a.group >= 21 && a.group <= 33 ? ': passed by no code (not started)' : ': the eyes, from the first Riskcharge (StyleFinish End 0x1413CEBD0 at 0x1413CF044; restored by 0x14140E7F0)') });
// (AUDIT-C) THE WARP'S AURA: COMBO_ANTI's Decide 0x141360000 sets +0x22C = 0x11 with the warp (+0x1E1); PlayAttack's head
// 0x1413378B0 (line 110) starts group +0x22C of m_CmnAuraEffect (pawn vt+0x2458 = ead_pc_aura_com0) at the vanish, slot 117
// 0x14134AA80 group +0x22C + 1 at the reappearance (End 0x141335120 the same while the warp is pending). Groups 17 / 18:
// vs_ant_wrp_in01 / in03 (out01 / out03) on his vfx_center (ToAttach false, absolute rotation) and vs_ant_wrp_in01 / in04
// (out01 / out04) on the RIGHT-HAND WEAPON's vfx_bottom (the held Keyblade: socket `kbhand:vfx_bottom`, the model put in by
// rage.rs). values = the LocationOffset; flags = [end type].
{
  const ead = readJson(P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content', 'Effects', 'system', 'aura', 'ead', 'ead_pc_aura_com0.uasset.json'));
  const ref = (i) => (typeof i === 'number' && i < 0 && ead.imports[-i - 1] ? ead.imports[-i - 1].objectName : String(i));
  const pv = (list) => { const o = {}; for (const q of list || []) o[q.name] = q.type === 'StructProperty' ? (Array.isArray(q.value) ? pv(q.value) : q.value) : q.type === 'ArrayProperty' ? (q.value && q.value.items ? q.value.items.map((it) => (Array.isArray(it) ? pv(it) : it)) : q.value) : q.value; return o; };
  let k = 0;
  for (const e of ead.exports) {
    const props = pv((e.data && e.data.props) || e.props);
    for (const meta of props.EffectDataList || []) {
      const m = Array.isArray(meta) ? pv(meta) : meta;
      for (const d of (m.EffectDataList || []).map((x) => (Array.isArray(x) ? pv(x) : x))) {
        if (d.GroupID !== 17 && d.GroupID !== 18) continue;
        const ao = Array.isArray(d.AttachObjectData) ? pv(d.AttachObjectData) : d.AttachObjectData || {};
        const onWeapon = d.UseAttachObjectData && /RIGHT_HAND_WEAPON/.test(ao.AttachType || '');
        const sock = m.SocketName && m.SocketName !== 'None' ? m.SocketName : '';
        const anchor = onWeapon ? 'kbhand:' + sock : sock + (m.ToAttach === false ? ':free' : '') + (m.bAbsoluteRotation ? ':abs' : '');
        const lo = m.LocationOffset || {};
        add({ id: 'aura.' + d.GroupID + '.wrp.' + k++, what: 'aura', code: d.GroupID, logical: ref(d.AttachEffect), clip: anchor, values: [r4(lo.x || 0), r4(lo.y || 0), r4(lo.z || 0), 1],
          flags: [String(m.AttachEffectEndType || '').replace('ESQEX_ATTACH_EFFECT_END_TYPE_', '')], ref_addr: '0x141360000 / 0x1413378B0 / 0x14134AA80 / 0x141335120', ref_conf: 'confirmed',
          note: '(AUDIT-C) ead_pc_aura_com0 group ' + d.GroupID + ': Rage Form warp ' + (d.GroupID === 17 ? 'vanish' : 'reappearance') + ' (+0x22C = 0x11 / + 1)' });
      }
    }
  }
  must(ROWS.filter((r) => r.what === 'aura' && (r.code === 17 || r.code === 18)).length === 8, 'ead_pc_aura_com0: 4 rows each of groups 17 / 18');
}
// the attack rows the form hits with
const used = new Set(ROWS.filter((r) => ['attack', 'chain', 'finish'].includes(r.what)).flatMap((r) => r.rows));
for (const extra of ['AN_FINISH_BOMB', 'AN_CRIMSONUPPER_RC', 'AN_CRIMSONBLAST_RC1', 'AN_CRIMSONBLAST_RC2', 'AN_CRIMSONBLAST_RC3', 'AN_CRIMSONBLAST_IMPACT', 'AN_CHANGE', 'AN_DISCENSION', 'AN_DISCENSION_END']) used.add(extra);
for (const id of [...used].sort()) {
  const a = atkRow(id);
  must(a, 'p_ex001_AttackData has no row ' + id);
  add({ id: 'row.' + id, what: 'row', logical: id, values: [a.m_Power, a.m_FormPoint, a.m_ReactionPowerValue], flags: [String(a.m_DamageKind).split('::').pop(), String(a.m_DamageAttribute).split('::').pop()], note: 'p_ex001_AttackData row ' + a.id + ': values = [m_Power, m_FormPoint, m_ReactionPowerValue]; carried by sora-combat-attack-data (tools/sora_combat_sheet.js attackData, F-RAGE rows after all others)' });
}

// ------------------------------------------------------------------------------------------------ sock: the hit sockets per frame
// (COMBAT-FIX, THE USER 2026-11-06: "rage form's normal attacks do no damage, fix this") AnimSet13's own clips (D411 .. D416,
// D418, D423, D424) have no socket track in the pack (build/sora_pack/manifest.json / the XS overlay carry D417 / D419 / D420
// only), so their KB_Stick* / KB_Sphere* windows (the held Keyblade's TresAtkColl0, w_so_KeybladeBase) had no shape and never
// hit; the pawn's AN_LHAND (p_ex001_Pawn TresAtkColl0: SPHERE 100 on the body socket L_wep_skt) had no place on ANY clip.
// From the clip's glb (tools/lib/form_sockets.js, the rule of tools/sora_pack.js `attackSockets`, checked below against
// D417's XS track):
//   sock.R.<seq>: per frame [ATK_00 xyz, ATK_01 xyz] of the held Keyblade (R_buki -> R_wep_skt -> the weapon's root -> wep;
//                 ATK_01 the Kingdom Key's, as the pack's own tracks), actor space, cm: only clips with a KB_* window and no
//                 pack track;
//   sock.L.<seq>: per frame [L_wep_skt xyz, L_wep_skt xyz] (L_buki -> the body socket L_wep_skt), actor space, cm: clips with
//                 an AN_LHAND window.
// code = frames, length = the clip's length (frame i at i x length / (frames - 1)).
{
  const { socketTrack } = require('./lib/form_sockets');
  const CONTENT = P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content');
  const pkg = (game) => readJson(path.join(CONTENT, game.replace(/^\/Game\//, '').split('.')[0] + '.uasset.json'));
  const pv = (props, name) => { const p = (props || []).find((x) => x.name === name); return p ? p.value : undefined; };
  const GLB = new Map();
  for (const [m, dir] of [[OV, P('build', 'sora_pack', 'keyblades')], [MAIN, P('build', 'sora_pack')], [XSI, P('build', 'sora_pack')]]) for (const i of m.items || []) if (i.kind === 'animation' && i.file && !GLB.has(i.animation_name)) GLB.set(i.animation_name, { ...i, path: path.join(dir, i.file) });
  const TRACKS = { ...((MAIN.attack_sockets || {}).clips || {}), ...((XSI.attack_sockets || {}).clips || {}) };
  const SK = readJson(P('build', 'sora_pack', 'weapon', 'manifest.json'));
  const wbone = (name) => { const x = SK.skeleton.bones.find((y) => y.name === name); must(x, 'weapon skeleton bone ' + name); return { q: x.rotation_ue, t: x.translation_ue, s: x.scale_ue }; };
  const WROOT = [wbone('kh_sk'), wbone('wep')];
  const ID = { q: [0, 0, 0, 1], t: [0, 0, 0], s: [1, 1, 1] };
  const KK = SK.sockets.sockets.find((x) => x.name === 'ATK_01').relative_location_ue_cm;
  const MESH_YAW = MAIN.identification.mesh_component.relative_rotation_ue_deg.yaw;
  const SOCK = { R: { bone: 'R_buki', socket: 'R_wep_skt' }, L: { bone: 'L_buki', socket: 'L_wep_skt' } };
  const skel = pkg('/Game/Character/pc/p_ex001/mdl/p_ex001_0_Skeleton');
  for (const side of Object.keys(SOCK)) {
    const e = skel.exports.find((x) => x.class === 'SkeletalMeshSocket' && pv(x.data.props, 'SocketName') === SOCK[side].socket);
    must(e && pv(e.data.props, 'BoneName') === SOCK[side].bone, SOCK[side].socket + ' on ' + SOCK[side].bone);
    SOCK[side].rot = pv(e.data.props, 'RelativeRotation') || { pitch: 0, yaw: 0, roll: 0 };
    SOCK[side].loc = pv(e.data.props, 'RelativeLocation') || { x: 0, y: 0, z: 0 };
  }
  const track = (seq, side) => {
    const it = GLB.get(seq); const s = SOCK[side];
    must(it && fs.existsSync(it.path), seq + ': no glb');
    return socketTrack(it.path, { bone: s.bone, socketRotDeg: s.rot, socketLoc: [s.loc.x, s.loc.y, s.loc.z], atk00: [0, 0, 0], atk01: side === 'R' ? KK : [0, 0, 0], weaponBones: side === 'R' ? WROOT : [ID, ID], rootMotion: it.root_motion_enabled === true, meshYawDeg: MESH_YAW, frames: it.num_frames, length: it.duration_s });
  };
  // the reader against the pack's own track of an AnimSet13 clip (D417 Crimson Blast, the XS overlay)
  {
    const ref = TRACKS.D417; must(ref, 'the XS overlay has D417\'s track');
    const r = track('D417', 'R');
    let mx = 0; r.sockets.forEach((f, i) => f.forEach((v, k) => { mx = Math.max(mx, Math.abs(v - ref.sockets[i][k])); }));
    must(mx < 0.05, 'D417: the socket reader is off the pack\'s track by ' + mx + ' cm');
  }
  const groups = (seq) => ((GLB.get(seq) || {}).notifies || []).filter((n) => n.class === 'TresAnimNotifyState_Attack').map((n) => { const g = prop(n, 'CollisionGrpName'); return String(g && g.value !== undefined ? g.value : g); });
  const seqs = [...new Set([...ROWS13.values()].map((a) => a.clip).filter((c) => GLB.has(c)))].sort();
  for (const seq of seqs) {
    const g = groups(seq);
    const it = GLB.get(seq);
    for (const side of ['R', 'L']) {
      if (side === 'R' && !(g.some((x) => /^KB_/.test(x)) && !TRACKS[seq])) continue;
      if (side === 'L' && !g.includes('AN_LHAND')) continue;
      const t = track(seq, side);
      add({ id: 'sock.' + side + '.' + seq, what: 'sock', code: t.frames, logical: side, clip: seq, length: r4(it.duration_s), values: t.sockets.flat().map((x) => Math.round(x * 1000) / 1000), ref_addr: '-', ref_conf: 'confirmed',
        note: '(COMBAT-FIX, the user 2026-11-06: "rage form\'s normal attacks do no damage, fix this") per frame ' + (side === 'R' ? '[ATK_00 xyz, ATK_01 xyz] of the held Keyblade (R_buki -> R_wep_skt -> weapon root -> wep; ATK_01 the Kingdom Key\'s, as the pack\'s tracks): the KB_* windows (' + g.filter((x) => /^KB_/.test(x)).join(', ') + ') had no pack track' : '[L_wep_skt xyz] x 2 (L_buki -> the body socket L_wep_skt): the place of p_ex001_Pawn\'s AN_LHAND (SPHERE 100)') + ', actor space, cm; from ' + path.relative(ROOT, it.path).replace(/\\/g, '/') });
    }
  }
  // (COMBAT-FIX) p_ex001_Pawn's TresAtkColl0 groups of Rage's clips that hit.rs cannot place (its owner sockets are `center` /
  // `vfx_cure`): AN_LHAND (L_wep_skt: the sock.L tracks above), FINISH_AN's AN_Finish (a CONVEX hull) and AN_FinishEnd (kh_sk).
  // flags = [m_ShapeType, m_SocketName1, the CONVEX mesh, m_RotAttachType, m_bDisableLocation1Attach yes | no];
  // values = [m_Size.x, m_RelativeLocation1 xyz, m_Scale xyz].
  const pawn = pkg('/Game/Blueprints/Player/p_ex001/p_ex001_Pawn');
  const imp = (i) => (typeof i === 'number' && i < 0 && pawn.imports[-i - 1] ? pawn.imports[-i - 1].objectName : 'None');
  const findShape = (o, g) => { if (Array.isArray(o) && o.some((p) => p && p.name === 'm_GrpName' && p.value === g)) return o; if (o && typeof o === 'object') for (const k of Object.keys(o)) { const r = findShape(o[k], g); if (r) return r; } return null; };
  const tail = (v) => String(v).split('::').pop();
  for (const g of ['AN_LHAND', 'AN_Finish', 'AN_FinishEnd']) {
    const s = findShape(pawn, g); must(s, 'p_ex001_Pawn has no shape ' + g);
    const v = (n) => pv(s, n);
    const size = v('m_Size') || {}, loc = v('m_RelativeLocation1') || {}, sc = v('m_Scale') || { x: 1, y: 1, z: 1 }, rot = v('m_RelativeRocation') || {};
    must(!rot.pitch && !rot.yaw && !rot.roll, g + ': a rotated shape (not ported)');
    must(tail(v('m_AttachType1')) === (g === 'AN_LHAND' ? 'NORMAL' : 'OWNER_ATTACHED_MESH'), g + ': attach type');
    const mesh = tail(v('m_ShapeType')) === 'CONVEX' ? imp(v('m_Mesh')) : 'None';
    must(mesh === 'None' || mesh === 'tres_colltest01', g + ': the mesh ' + mesh + ' is not one kh-core reads (combat/hull.rs `mesh`)');
    add({ id: 'shape.' + g, what: 'shape', logical: g, clip: v('m_SocketName1'), flags: [tail(v('m_ShapeType')), v('m_SocketName1'), mesh, tail(v('m_RotAttachType')), v('m_bDisableLocation1Attach') ? 'yes' : 'no'],
      values: [size.x || 0, loc.x || 0, loc.y || 0, loc.z || 0, sc.x, sc.y, sc.z], ref_addr: '0x140647950', ref_conf: 'confirmed',
      note: '(COMBAT-FIX, the user 2026-11-06: "rage form\'s normal attacks do no damage, fix this") p_ex001_Pawn TresAtkColl0 CollisionShapesSrc ' + g + ': values = [m_Size.x, m_RelativeLocation1 xyz, m_Scale xyz]; flags = [shape, socket, mesh, rotation attach, m_bDisableLocation1Attach]' });
  }
}

// ------------------------------------------------------------------------------------------------ checks
must(ROWS.filter((r) => r.what === 'attack').length === PLAY.length, 'attack rows');
for (const r of ROWS.filter((x) => x.what === 'attack' && !x.flags.includes('not_decided'))) must(r.clip !== '-' && r.length > 0, r.id + ': no clip / length');
must(ROWS.some((r) => r.id === 'anim.IdleB' && r.clip === 'D015'), 'AnimSet13 has IdleB = D015');
must(ROWS.filter((r) => r.what === 'aura').length >= 10, 'the aura list');

// ------------------------------------------------------------------------------------------------ sheet
const S = (n, t, g, doc) => [n, t, g, doc];
const COLS = [
  S('id', 'str', 'str', '<what>.<key>'), S('what', 'enum:rule|attack|chain|finish|anim|aura|row|sock|shape', 'str', 'the row kind (sock / shape: COMBAT-FIX, the hit sockets per frame, the pawn groups)'),
  S('code', 'int', 'i32', 'attack kind (attack, chain), finish command (finish), effect group (aura), else 0'), S('logical', 'str', 'str', 'the logical anim name KH3 plays (attack / chain / finish / anim), the rule (rule), the particle (aura), the row (row)'),
  S('clip', 'str', 'str', 'the clip = the animation-table row; the socket (aura)'), S('length', 'num', 'f32', 'the clip length, s'),
  S('root_len', 'num', 'f32', 'root length (+0x1D8), 0 = none'), S('stop_dist', 'num', 'f32', 'stop distance (+0x24C)'), S('z_gain', 'num', 'f32', 'z gain (+0x14C)'),
  S('air_mode', 'bool', 'bool', '+0x1DF'), S('keep_face', 'bool', 'bool', '+0x1F4 bit 4'), S('chan_xy', 'bool', 'bool', '+0x234 bit 0'), S('chan_root', 'bool', 'bool', '+0x234 bit 1'),
  S('flags', 'list', 'str[]', 'the case\'s other writes (see the note)'), S('windows', 'list', 'str[]', 'the clip\'s hit windows a-b, s'), S('rows', 'list', 'str[]', 'the attack rows of those windows'),
  S('effects', 'list', 'str[]', 'the clip\'s effects (time@group:asset) and shots (shot time@name)'), S('sounds', 'list', 'str[]', 'the clip\'s sounds / voices (time@asset)'), S('values', 'list', 'f32[]', 'numbers by kind (see the note)'),
  S('ref_addr', 'str', 'str', 'decompiled function'), S('ref_conf', 'enum:confirmed|inferred|guess', 'str', 'confirmed = read in the code or the data; inferred; guess'), S('note', 'str', 'str', '-'),
];
const NAME = 'sora-form-rage';
const rows = ROWS.map((r) => COLS.map((c) => r[c[0]]));
for (const [i, r] of rows.entries()) for (const [k, v] of r.entries()) must(v !== undefined && v !== null && !(typeof v === 'number' && !Number.isFinite(v)), 'row ' + ROWS[i].id + ': cell ' + COLS[k][0] + ' is ' + v);
must(new Set(ROWS.map((r) => r.id)).size === ROWS.length, 'duplicate ids');
fs.writeFileSync(P('sheets', 'port', NAME + '.json'), '{"sheet":"' + NAME + '","group":"port","columns":' + JSON.stringify(COLS.map((c) => c[0])) + ',"rows":[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
const by = {}; for (const r of ROWS) by[r.what] = (by[r.what] || 0) + 1;
console.log('sheets/port/' + NAME + '.json: ' + rows.length + ' rows ' + JSON.stringify(by));

// ------------------------------------------------------------------------------------------------ register (this tool's entry only; replaced in place)
const q = (s) => "'" + String(s).replace(/\\/g, '\\\\').replace(/'/g, "\\'") + "'";
const NL = '\n';
const DOC = 'KH3\'s Rage Form as data for its form package (kh-core combat/forms/rage.rs, states/combo_anti.rs): the HP-loss offer, the counts and the cooldown, COMBO_ANTI\'s attack kinds, AnimSet13, RISKCHARGE / FINISH_AN, the aura, the attack rows.';
const MARK = '(weapons F-RAGE)';
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
  const text = '  // ' + MARK + ' ' + DOC + ' Written by tools/sora_form_rage.js (documentation/KH3_WEAPONS_PLAN.md 6e)' + NL +
    '  ' + q(NAME) + ": { group: 'port', file: " + q(NAME + '.json') + ", by: 'tools/sora_form_rage.js', from: " + q('sora-keyblade-forms, AnimSet13 of the asset pack, ead_p_ant_com, p_ex001_AttackData and the decompiled HP-setter tail / ComboAnti / PlayAttack / StyleFinish / StartStyle / EndStyle') + ', external: true, cols: [' + NL +
    '    ' + COLS.map((c, i) => '[' + q(c[0]) + ', ' + (i === 0 ? 'K' : 'V') + ', ' + q(c[1]) + ", '-', " + q(c[3]) + (['note', 'flags', 'windows', 'rows', 'effects', 'sounds', 'values', 'logical', 'clip'].includes(c[0]) ? ', true' : '') + ']').join(', ') + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/sheet_defs.js');
}
{
  const f = P('tools', 'lib', 'port_gen_defs.js'); const before = fs.readFileSync(f, 'utf8');
  const END = NL + '    },' + NL;
  const pos = (t) => { const anchor = '  tables: {' + NL; const at = t.indexOf(anchor); must(at >= 0, 'port_gen_defs.js: anchor tables'); return at + anchor.length; };
  const cols = COLS.filter((c) => c[2]).map((c) => c[0] + ': { type: ' + q(c[2]) + (c[3] && c[3] !== '-' ? ', doc: ' + q(c[3]) : '') + ' }').join(', ');
  const text = '    // ' + MARK + ' ' + NAME + ': written by tools/sora_form_rage.js (documentation/KH3_WEAPONS_PLAN.md 6e)' + NL +
    '    ' + q(NAME) + ': {' + NL +
    "      file: 'sora_form_rage.rs', row: 'FormRageRow', table: 'FORM_RAGE', key: 'id', key_prefix: 'FRG'," + NL +
    '      doc: ' + q(DOC) + ',' + NL +
    '      columns: { ' + cols + ' },' + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/port_gen_defs.js');
}
console.log('sora_form_rage ' + VERSION);
