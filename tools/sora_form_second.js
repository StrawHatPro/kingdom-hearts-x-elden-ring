// (weapons F-SECOND) Second Form's two variants on the other Keyblades as DATA for kh-core (combat/forms/second.rs,
// states/style_finish.rs slot 155): Second Form S (form.L2, CHANGE_L2 0x95, Starlight) and Second Form D (form.L5,
// CHANGE_L5 0x98, Dead of Night). KH3 runs both with Second Form's own code (AwardAtMax / StartStyle / StyleChange /
// SetStyleFinishCommand / EndStyle: one case 0x94 .. 0x98 each); what differs is the data of their rows and Second Form S's
// finish level 3, LM_UNIONDISCODE (Ancient Light, 0xAE), whose STYLEFINISH body (slot 155) this sheet describes, with the
// Starlight weapon's own hit shapes that its clip M_1435 opens. documentation/KH3_WEAPONS_PLAN.md 1.3 / 4,
// build/sora_tmp/weapons/F-SECOND/STATE.md.
//
//   node tools/sora_form_second.js   write sheets/port/sora-form-second.json and register it (once, in place) in
//                                    tools/lib/sheet_defs.js and tools/lib/port_gen_defs.js; then node tools/gen_port.js
//
// Inputs (read only): sheets/port/sora-keyblade-forms.json (P3: the form rows), build/sora_pack/manifest.json (clip M_1435
// = sequence 1435 and its notifies; the wielder's socket vfx_cure), the Starlight weapon blueprint w_so150_Wep (TresAtkColl0
// CollisionShapesSrc), and THE CODE TABLES BELOW, read in the decompiled exe (re/decomp):
//   SETFINISHLEVEL  0x141414C70: level 3 = 0xAE in form 0x95, else 0xAD (the rows' finish_levels).
//   STYLEFINISH     Begin 0x1413CBF70 binds the per-command tick by command: 0xAB slot 152, 0xAC 153, 0xAD 154, 0xAE 155
//                   (TresPlayerState_StyleFinish, audit/vfuncs.json: slot 155 = 0x1413E2810).
//   SLOT 155        0x1413E2810 (Ancient Light): the sub-steps below.
//   SLOT 164        0x1413D12A0 (h): the ground probe 0x14063A650(pawn, 0, h + 300); a floor nearer than h (or no hit but
//                   the floor test 0x1405ADAA0 = movement vt+0x9D8): the rise target = actor location + (h - distance) up
//                   (state+0x290), true.
//   SLOT 165        0x1413E50D0 (dt): dz = target z - actor z; dz > 0 and dt x 500 x dz < dz: StopMovement, Velocity.Z =
//                   dt x 500 x dz, false; else StopMovement, SetActorLocation(target, sweep) (0x143EAC3D0), true.
//   SLOT 115        0x141332790 (the attack state's hit-window begin, through StyleFinish's TresAtk handler 0x1413DD7D0 ->
//                   0x1413459E0): +0x12C (and +0x138) += 1 per window, the count slot 155 reads.
'use strict';
const fs = require('fs');
const path = require('path');
const ROOT = path.resolve(__dirname, '..');
const VERSION = '1.0.0';
const P = (...a) => path.join(ROOT, ...a);
const readJson = (f) => JSON.parse(fs.readFileSync(f, 'utf8'));
const must = (c, m) => { if (!c) throw new Error(m); };
const r4 = (x) => Math.round(x * 10000) / 10000;

// ------------------------------------------------------------------------------------------------ inputs
const sheet = (name) => { const s = readJson(P('sheets', 'port', name + '.json')); return s.rows.map((r) => Object.fromEntries(s.columns.map((c, i) => [c, r[i]]))); };
const FORMS = sheet('sora-keyblade-forms');
const L1 = FORMS.find((r) => r.id === 'form.L1');
const L2 = FORMS.find((r) => r.id === 'form.L2');
const L5 = FORMS.find((r) => r.id === 'form.L5');
must(L1 && L2 && L5, 'sora-keyblade-forms has no form.L1 / form.L2 / form.L5');
must(L2.finish_levels[2] === 0xAE && L1.finish_levels[2] === 0xAD && L5.finish_levels[2] === 0xAD, 'finish level 3: 0xAE only in form.L2 (SetFinishLevel 0x141414C70)');
const MAIN = readJson(P('build', 'sora_pack', 'manifest.json'));
const CLIP = MAIN.items.find((i) => i.kind === 'animation' && i.animation_name === '1435');
must(CLIP, 'manifest: no sequence 1435 (M_1435)');
must((MAIN.logical_animations.find((a) => a.name === 'M_1435') || {}).asset === '/Game/Character/pc/p_ex001/anm/rt/body/1435.1435', 'M_1435 is not sequence 1435');
const VFX_CURE = MAIN.attack_sockets && MAIN.attack_sockets.vfx_cure;
must(VFX_CURE && VFX_CURE.position_actor_cm, 'manifest: no vfx_cure socket');
// The socket frame in actor space: the pack gives the socket point and the point (200, 0, 0) of the socket frame. The
// socket's rotation (pitch 90, roll -90) on the root bone kh_sk (UE FRotationMatrix: socket x -> bone +Z, y -> bone -X, z ->
// bone -Y) with the root bone's reference transform (bone +Z -> actor +X, bone -Y -> actor +Z, both from the two pack
// points; bone +X -> actor -Y by the cross product) is the identity: a relative location of the socket frame is the same
// vector in actor space. Checked here on the one axis the pack carries.
{
  const [a, b] = [VFX_CURE.position_actor_cm, VFX_CURE.plus_200_x_actor_cm];
  must(Math.abs(b[0] - a[0] - 200) < 1e-3 && Math.abs(b[1] - a[1]) < 1e-3 && Math.abs(b[2] - a[2]) < 1e-3, 'vfx_cure: the socket x axis is not actor x');
}

// SHAPES: TresAtkColl0 CollisionShapesSrc of the Starlight weapon (Second Form S's unit RWeapon = w_so150_Wep), the groups
// the Kingdom Key's component lacks (its M_1435 windows).
const GROUPS = ['L2_Pillar1', 'L2_Pillar2', 'L2_Pillar3', 'L2_Pillar4', 'L2_Pillar5', 'L2_Dark'];
const WEP = 'Player/p_ex001/w_so150_Wep';
must(L2.units.some((u) => u.endsWith('/' + WEP.split('/').pop())), 'form.L2: its weapon is not ' + WEP);
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
      out.push({ group: s.m_GrpName, shape: String(s.m_ShapeType).split('::').pop(), size: v(s.m_Size), socket1: s.m_SocketName1 || 'None', socket2: s.m_SocketName2 || 'None', loc: v(s.m_RelativeLocation1), row: s.m_DefaultAttackDataIDName || 'None', attach: String(s.m_AttachType1 || '').split('::').pop(),
        inc: v(s.m_IncSize), inc_max: v(s.m_IncMaxSize), rot_attach: String(s.m_RotAttachType || 'TRES_ACRA_NORMAL').split('::').pop(), frozen: s.m_bDisableLocation1Attach === true, abs1: s.m_bAbsoluteOffset1 === true });
    }
  }
  return out;
}

// ------------------------------------------------------------------------------------------------ rows
const ROWS = [];
const blank = { what: '-', code: 0, logical: '-', clip: '-', flags: [], windows: [], values: [], ref_addr: '-', ref_conf: 'confirmed', note: '-' };
const add = (r) => ROWS.push({ ...blank, ...r });
const SRC_WEP = 'assets_converted/packages/KINGDOM HEARTS III/Content/Blueprints/' + WEP + '.uasset.json (TresAtkColl0 CollisionShapesSrc)';
const all = shapes(WEP);
for (const g of GROUPS) {
  const s = all.find((x) => x.group === g);
  must(s, WEP + ': no group ' + g);
  must(s.socket1 === 'vfx_cure' && s.attach === 'OWNER_ATTACHED_MESH' && !s.abs1, g + ': not on the wielder\'s vfx_cure');
  must(['CAPSULE1', 'SPHERE'].includes(s.shape), g + ': shape ' + s.shape);
  add({ id: 'shape.L2.' + g, what: 'shape', logical: g, clip: 'w_so150_Wep', flags: [s.shape, s.socket1, s.socket2, s.attach, s.rot_attach, s.frozen ? 'frozen' : 'attached', s.row],
    values: [...s.size, ...s.loc, s.inc[0], s.inc_max[0]], ref_addr: '0x140649E40 (size over time)', note: 'TresAtkColl0 of ' + SRC_WEP + ': flags = [shape, socket 1, socket 2, attach type, rotation attach, frozen (m_bDisableLocation1Attach) | attached, default row]; values = [m_Size x (radius), y, z (CAPSULE1: half height), m_RelativeLocation1 x, y, z (the socket frame = actor space), m_IncSize x (per s of the window), m_IncMaxSize x]' });
}
add({ id: 'socket.vfx_cure', what: 'socket', logical: 'vfx_cure', clip: VFX_CURE.bone || 'kh_sk', values: VFX_CURE.position_actor_cm.map(r4), note: 'the wielder\'s socket vfx_cure in actor space, cm (build/sora_pack/manifest.json attack_sockets.vfx_cure: root bone, reference pose); its frame is actor space (see the generator)' });

// the clip's hit windows (sequence 1435)
const wins = CLIP.notifies.filter((n) => n.class === 'TresAnimNotifyState_Attack').map((n) => r4(n.trigger_time) + '-' + r4(n.end_trigger_time) + '@' + n.properties.CollisionGrpName + '@' + n.properties.m_AttackDataIDName);
const pillars = wins.filter((w) => /@L2_Pillar\d@/.test(w)).length;
must(pillars === 5, 'M_1435: ' + pillars + ' pillar windows (slot 155 waits for the fifth: +0x12C > 4)');
const change = CLIP.notifies.find((n) => n.class === 'TresAnimNotify_EnableChange');
must(change, 'M_1435: no TresEnableChange');
// FINISH: slot 155 0x1413E2810, sub-step by sub-step (decompiled C, the whole function).
const RISE_HEIGHT = 500; // slot 164's argument (0x43FA0000) at 0x1413E2841
const PROBE_EXTRA = 300; // 0x1413D12A0: the probe range = h + 300
const RISE_RATE = 500; // 0x1413E50D0: dt x 500 x dz
const HITS = 4; // 0x1413E2868: +0x12C > 4
const FIN = [
  { sub: 0, what: 'SetMovementMode(5 Flying, 1) (pawn vt+0x17A0); Play("M_1435", blend -1, mode 2); m_bFaceAt; sub 1; then slot 164 (the rise, h = 500): a floor nearer than h -> sub 3', flags: ['flying', 'play', 'face_at', 'rise_check'], values: [RISE_HEIGHT, PROBE_EXTRA] },
  { sub: 1, what: 'the fifth hit window has begun (+0x12C > 4): WorldSlow(off) (0x14140E500(ctrl, 0)), sub 2; then as sub 2', flags: ['world_slow_off'], values: [HITS] },
  { sub: 2, what: 'm_bEnableChange: SetMovementMode(3 Falling, 1), gravity on (vt+0x1728(1)), GravityScale = the class default (vt+0x1658), sub 0xC; then as sub 0xC', flags: ['falling', 'gravity_on'], values: [r4(change.trigger_time)] },
  { sub: 3, what: 'slot 165 (the rise toward the target of slot 164, dt x 500 x dz, else a swept SetActorLocation): when done sub 1; then as sub 1 (sub 1\'s count test runs every tick of sub 3 too)', flags: ['rise'], values: [RISE_RATE] },
  { sub: 0xC, what: 'AnimEnded (anim interface vt+200): DoChain (vt+0x270), done', flags: ['end'], values: [] },
];
for (const f of FIN) add({ id: 'finish.L2.' + f.sub, what: 'finish', code: f.sub, logical: 'M_1435', clip: 'M_1435', flags: f.flags, windows: f.sub === 0 ? wins : [], values: f.values, ref_addr: f.sub === 3 ? '0x1413E2810 / 0x1413E50D0' : f.sub === 0 ? '0x1413E2810 / 0x1413D12A0' : '0x1413E2810', note: 'STYLEFINISH slot 155 (LM_UNIONDISCODE 0xAE, Ancient Light), sub-step ' + f.sub + ': ' + f.what + (f.sub === 0 ? '; windows = the clip\'s hit windows start-end@group@row (sequence 1435, ' + r4(CLIP.duration_s) + ' s)' : '') });

// THE DIFFERENCES of form.L2 / form.L5 from form.L1 (the rows; KH3's code has one case 0x94 .. 0x98 for all three).
const COLS_DIFF = ['keyblade', 'weapon_type', 'finish_levels', 'units', 'projectile_set', 'projectiles', 'moves', 'ui_asset', 'act_logo'];
for (const r of [L2, L5]) {
  const diff = Object.keys(L1).filter((c) => !['id', 'cmd', 'command', 'name', 'keyblade_name', 'trigger', 'source', 'ref_conf', 'note', 'move_names'].includes(c) && JSON.stringify(L1[c]) !== JSON.stringify(r[c]));
  for (const c of diff) must(COLS_DIFF.includes(c), r.id + ': an unexpected difference from form.L1 in ' + c + ' (port it)');
  add({ id: 'diff.' + r.id.split('.')[1], what: 'diff', code: r.cmd, logical: r.command, clip: r.keyblade, flags: diff, note: 'the columns of ' + r.id + ' that differ from form.L1 (weapon_type: StartStyle 0x14141A9D0 / AwardAtMax 0x1413F0ED0; finish_levels: SetFinishLevel 0x141414C70; units / projectile_set / projectiles: the Keyblade\'s own equipment (its weapon and shotlock set, drawn / shot by the Keyblade packages P5 / SHOT); moves / ui_asset / act_logo: the HUD\'s form data (the act logo movies are not in the pack: kh-ui hud_command.rs))' });
}

// ------------------------------------------------------------------------------------------------ write
const S = (n, t, g, doc) => [n, t, g, doc];
const COLS = [
  S('id', 'str', 'str', '<what>.<form>.<key>'), S('what', 'enum:shape|socket|finish|diff', 'str', 'the row kind'), S('code', 'int', 'i32', 'finish sub-step (finish), the form command (diff), else 0'),
  S('logical', 'str', 'str', 'the group (shape), the socket (socket), the clip KH3 plays (finish), the command (diff)'), S('clip', 'str', 'str', 'the owner (shape), the bone (socket), the animation table row (finish), the Keyblade (diff)'),
  S('flags', 'list', 'str[]', 'strings by kind (see the note)'), S('windows', 'list', 'str[]', 'the clip\'s hit windows start-end@group@row, s'), S('values', 'list', 'f32[]', 'numbers by kind (see the note)'),
  S('ref_addr', 'str', 'str', 'decompiled function'), S('ref_conf', 'enum:confirmed|inferred|guess', 'str', 'confirmed = read in the code or the data'), S('note', 'str', 'str', '-'),
];
const NAME = 'sora-form-second';
const rows = ROWS.map((r) => COLS.map((c) => r[c[0]]));
for (const [i, r] of rows.entries()) for (const [k, v] of r.entries()) must(v !== undefined && v !== null && !(typeof v === 'number' && !Number.isFinite(v)), 'row ' + ROWS[i].id + ': cell ' + COLS[k][0] + ' is ' + v);
for (const r of ROWS) must(r.values.every((v) => typeof v === 'number' && Number.isFinite(v)), 'row ' + r.id + ': a value is not a number');
fs.writeFileSync(P('sheets', 'port', NAME + '.json'), '{"sheet":"' + NAME + '","group":"port","columns":' + JSON.stringify(COLS.map((c) => c[0])) + ',"rows":[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
const by = {}; for (const r of ROWS) by[r.what] = (by[r.what] || 0) + 1;
console.log('sheets/port/' + NAME + '.json: ' + rows.length + ' rows ' + JSON.stringify(by));

// ------------------------------------------------------------------------------------------------ register (this tool's entry only; replaced in place)
const q = (s) => "'" + String(s).replace(/\\/g, '\\\\').replace(/'/g, "\\'") + "'";
const NL = '\n';
const DOC = 'KH3\'s Second Form S / D (Starlight, Dead of Night) as data for kh-core (combat/forms/second.rs, states/style_finish.rs slot 155): Ancient Light\'s STYLEFINISH body, the Starlight weapon\'s hit shapes of M_1435, the rows\' differences from Second Form.';
const MARK = '(weapons F-SECOND)';
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
  const text = '  // ' + MARK + ' ' + DOC + ' Written by tools/sora_form_second.js (documentation/KH3_WEAPONS_PLAN.md 1.3)' + NL +
    '  ' + q(NAME) + ": { group: 'port', file: " + q(NAME + '.json') + ", by: 'tools/sora_form_second.js', from: " + q('sora-keyblade-forms, clip 1435 of the asset pack, the vfx_cure socket, w_so150_Wep and the decompiled StyleFinish slots 115 / 155 / 164 / 165, SetFinishLevel') + ', external: true, cols: [' + NL +
    '    ' + COLS.map((c, i) => '[' + q(c[0]) + ', ' + (i === 0 ? 'K' : 'V') + ', ' + q(c[1]) + ", '-', " + q(c[3]) + (['note', 'flags', 'windows', 'values', 'logical', 'clip'].includes(c[0]) ? ', true' : '') + ']').join(', ') + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/sheet_defs.js');
}
{
  const f = P('tools', 'lib', 'port_gen_defs.js'); const before = fs.readFileSync(f, 'utf8');
  const END = NL + '    },' + NL;
  const pos = (t) => { const anchor = '  tables: {' + NL; const at = t.indexOf(anchor); must(at >= 0, 'port_gen_defs.js: anchor tables'); return at + anchor.length; };
  const cols = COLS.filter((c) => c[2]).map((c) => c[0] + ': { type: ' + q(c[2]) + (c[3] && c[3] !== '-' ? ', doc: ' + q(c[3]) : '') + ' }').join(', ');
  const text = '    // ' + MARK + ' ' + NAME + ': written by tools/sora_form_second.js (documentation/KH3_WEAPONS_PLAN.md 1.3)' + NL +
    '    ' + q(NAME) + ': {' + NL +
    "      file: 'sora_form_second.rs', row: 'FormSecondRow', table: 'FORM_SECOND', key: 'id', key_prefix: 'FSE'," + NL +
    '      doc: ' + q(DOC) + ',' + NL +
    '      columns: { ' + cols + ' },' + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/port_gen_defs.js');
}
console.log('sora_form_second ' + VERSION);
