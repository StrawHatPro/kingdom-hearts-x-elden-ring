// (SHOT-A rush) Family `rush`: KH3's kind 9 "トイ高速" (sf_rush_0) "Drill Dive" (Favorite Deputy's MAX kind) and SHOT-C's
// kind 26 "タイム高速" (sf_rush_0_1) "Noise Flux" (Classic Tone's MAX kind), both run by TresPlayerState_ShootFlow update
// 0x1413BC550 (vtable 0x14693CA58 + 0x4B8). Rows `rush.*` of sheets/port/sora-shotlocks.json for kh-core
// magic/shotlocks/rush.rs.
//
// THE CODE (decompiled C re/decomp/1413B/1413BC550.c, 1413B/1413BE650.c, 1413C/1413C4030.c; constants re-read in the
// disassembly of build/sora_tmp/weapons/SHOT-B/spec/scripts/dis.js, f32 values read from base/KH3.exe):
//   sub 0: gravity off (vt+0x1728(0)), Flying stopped (vt+0x17A0(5, 1)); ChangeWeapon vt+0x13C8(weapon type, 6 (kind 0x1A:
//          0x15), 0, ...) = Drill Punch's (Clock Drill's) weapon and anim set AnimSet6; Play "M_6481" (blend -1, mode 2,
//          group +0xC0), voice +0xC4 when the length > 0; m_bFaceAt; sub 4; LiftCheck vt+0x398(200) -> sub 3.
//   sub 3: LiftStep vt+0x3A0 -> sub 4; then (sub 3 and 4) at M_6481's end: vt+0x1010(0) (body collision off), sub 5,
//          FUN_14140E500(ctrl, 1) (the world slow on), FUN_1405D4D90(pawn, 1, 0) (no core part), +0x110 = 0, +0xD0 = 0,
//          +0x104 = his rotation, +0xAC = its yaw, +0xE4 = +0xD8 = his location (the capsule centre); the first warp
//          vt+0x4C0: none -> the end below; else Play "M_6482" (mode 3: loop + root motion, group +0xC0), voice 0; kind
//          0x1A: his mesh hidden (vt+0xB58 -> SetVisibility(0, 1)); +0xA4 = 0.3.
//   sub 5: the dash vt+0x4C8(dt) 0x1413BE650: SetActorLocation(+0xF8 - +0xA4 x 3.3333333 x (+0xF8 - +0xD8)) (no sweep),
//          +0xA4 -= dt; at <= 0 (+0xA4 = 0) the next warp vt+0x4C0: Play "M_6482" (mode 3), voice 0, +0xA4 = 0.3; none
//          -> the attack camera off, his mesh shown, Play "M_6483" (mode 2, group +0xC0), voice +0xC4, SetActorLocation
//          (+0xE4), SetActorRotation(+0x104), sub 8.
//   sub 8: at M_6483's end Play "ShootFlowEnd" (mode 2, group +0xC0), voice 99, m_bEnableChange = 0, sub 2.
//   other (sub 2): m_bEnableChange (ShootFlowEnd's TresEnableChange) and +0xB0 != 0: vt+0x1408, ChangeWeapon(type, +0xB0,
//          +0xB1) (Begin's weapon back), +0xB0 = 0, vt+0x1010(1, 0) (body collision on), FUN_14140E500(ctrl, 0) (the world
//          slow off); at the clip's end vt+0x16E8 (StopMovement), vt+0x1628, Finish (vt+0x270).
//   the warp 0x1413C4030: the locked entry +0x110 (ctrl+0xB08, 0x34 bytes, count ctrl+0xB10): out of range, or its
//          component gone / pending kill -> false. P = the component's point (vt+0xB88) - 100 z; +0xAC -= 120; D =
//          FRotator(0, +0xAC, 0).Vector(); +0xD8 = P - 800 D, +0xF8 = P + 800 D; (state vt+0x140 = GetOwner, unused); a new
//          target actor: the attack camera off, then on in mode 2 with the component and +0xE4; SetActorLocation(+0xD8);
//          Turn(0, +0xAC, 0) at alpha 1 (vt+0xDB0); the right-hand weapon's effect attach "WarpRush" (its designated
//          attach data) with group 0x686 (kind 0x1A: 0x76C) (FUN_1405B9370); +0x114 += 1; >= the entry's lock count
//          (+0xC): +0x110 += 1, +0x114 = 0, +0xAC -= 120. True.
//   the TresAtk begin rename 0x1413B3590: kind 0x1A turns SF_WARPRUSH into SF_TRWARPRUSH.
// THE DATA: the clips of p_ex001_AnimSet6 (Clock Drill's AnimSet21 has no M_648x), the form rows of
// sheets/port/sora-keyblade-forms (form.DR / form.DR2), the weapons' TresAtkColl0 / TresEffectAttach0
// (Blueprints/Player/p_ex001/w_so053_Wep, w_so113_Wep), their designated attach data (Effects/wep/*/ead/ead_so*_stf0),
// p_ex001_AttackData SF_WARPRUSH / SF_TRWARPRUSH, the FK of the weapon's ATK_00 / ATK_01 over M_6482 (as barrage.js).
'use strict';
const path = require('path');
const fs = require('fs');

module.exports = (lib) => {
  const { row, clip, prop, blueprint, ATTACK, decomp, must, readJson, P } = lib;
  const rows = [];
  const add = (...a) => rows.push(row(...a));
  const UPD = '0x1413BC550', MOVE = '0x1413BE650', WARP = '0x1413C4030', RENAME = '0x1413B3590';
  const upd = decomp(UPD), move = decomp(MOVE), warp = decomp(WARP), rename = decomp(RENAME);
  must(upd && move && warp && rename, 'no decompiled 0x1413BC550 / 0x1413BE650 / 0x1413C4030 / 0x1413B3590 in re/decomp');

  add('rush.update', UPD, '-', 'vtable 0x14693CA58 + 0x4B8 (Begin 0x1413A5CD0 cases 9 / 0x1A)', 'the update of kinds 9 (Drill Dive) and 0x1A (Noise Flux)', UPD);
  add('rush.kinds', [9, 26], 'kind', 'sora-shotlocks kind.k9 / kind.k26 update 0x1413BC550', 'the kinds this update runs (9 SHOT-A Drill Dive, 26 SHOT-C Noise Flux)', UPD);
  must(/== 0x1a\)/.test(upd), UPD + ': the kind test changed');
  add('rush.toon_kind', 26, 'kind', UPD + ' 0x1413BCBC6 / 0x1413BCA73 cmp [rdi+8Ch],1Ah; ' + WARP + ' 0x1413C433D', 'the kind that takes Clock Drill (0x15), hides his mesh after the first warp and spawns WarpRush with group 0x76C', UPD);

  // ---------------------------------------------------------------------------------------------- the weapon forms
  const forms = readJson(P('sheets', 'port', 'sora-keyblade-forms.json'));
  const formRow = (id) => { const r = forms.rows.map((x) => Object.fromEntries(forms.columns.map((c, i) => [c, x[i]]))).find((x) => x.id === id); must(r, 'sora-keyblade-forms has no ' + id); return r; };
  const KIND_FORM = [[9, 6, 'form.DR', '0x1413BCBEB mov r8b,6'], [26, 21, 'form.DR2', '0x1413BCBE4 mov r8b,15h']];
  const FORM_OF = {};
  for (const [kind, value, id, at] of KIND_FORM) {
    const f = formRow(id);
    must(f.weapon_form_value === value, id + ': weapon form ' + f.weapon_form_value + ', the code puts on ' + value);
    FORM_OF[kind] = f;
    add('rush.k' + kind + '.weapon_form', value, 'int', UPD + ' sub 0: pawn vt+0x13C8(weapon type, ' + value + ', 0, ...) at ' + at, f.weapon_form + ' = ' + f.name + ' (' + id + '): its weapon and anim sets ' + f.anim_sets.map(lib.setName).join(' + ') + '; Begin\'s +0xB0 / +0xB1 come back in sub 2 / End', UPD);
    add('rush.k' + kind + '.weapon_units', f.units, '-', 'sheets/port/sora-keyblade-forms ' + id + ' units', 'the parts the hosts draw instead of the Keyblade while PawnMagic::shot_weapon_form is ' + value, UPD);
  }

  // ---------------------------------------------------------------------------------------------- the clips
  const CLIPS = [['start', 'M_6481', '0x1413BCC1C (@146933CB8), sub 0', 2], ['dash', 'M_6482', '0x1413BCA01 (@146933CC8) / 0x1413BC796 (@146933CE8), each warp', 3], ['end', 'M_6483', '0x1413BCAD6 (@146933CD8) / 0x1413BC850 (@146933CF8), no warp left', 2]];
  const C = {};
  for (const [key, name, at, mode] of CLIPS) {
    const c = clip(name, 'p_ex001_AnimSet6');
    must(c, 'no clip ' + name + ' of p_ex001_AnimSet6');
    must(!clip(name, 'p_ex001_AnimSet21'), name + ': AnimSet21 replaces it');
    C[key] = c;
    add('rush.clip.' + key, name, '-', UPD + ' Play FName at ' + at, 'slot 0, blend -1, mode ' + mode + ', group +0xC0, rate 1; p_ex001_AnimSet6 ' + name + ' (both weapon forms: AnimSet21 has none)', UPD);
    add('rush.clip.' + key + '.sequence', c.sequence, '-', 'p_ex001_AnimSet6 ' + name + ' -> sequence', 'the animation-table row (AnimId::A' + c.sequence + ')', UPD);
    add('rush.clip.' + key + '.length', c.length, 's', 'sequence ' + c.sequence + ' duration', key === 'dash' ? 'looped (mode 3) while the dash runs' : 'the sub-step waits for its end', UPD);
    add('rush.clip.' + key + '.mode', mode, 'bits', UPD + ' Play [rsp+20h] = ' + mode, 'bit 0 loop, bit 1 root motion', UPD);
  }
  const end = clip('ShootFlowEnd');
  must(end, 'no clip ShootFlowEnd');
  add('rush.end_clip_length', end.length, 's', '0x1413BC6DD (@146933D08); sequence ' + end.sequence + ' duration', 'sub 2 finishes at its end (Play mode 2, group +0xC0, voice 99)', UPD);
  const change = end.notifies.find((n) => n.class === 'TresAnimNotify_EnableChange');
  must(change, 'ShootFlowEnd has no TresEnableChange');
  add('rush.end_change', change.time, 's', 'ShootFlowEnd TresAnimNotify_EnableChange', 'from here Begin\'s weapon comes back, the body collision is on and the world slow off (sub 2)', UPD);
  add('rush.voice.dash', 0, 'int', UPD + ' 0x1413BC806 / 0x1413BCA6B xor edx,edx -> anim interface vt+0x1F0', 'the voice group after each M_6482', UPD);
  add('rush.voice.end', 99, 'int', UPD + ' 0x1413BC747 mov edx,63h -> vt+0x1F0', 'the voice group after ShootFlowEnd', UPD);

  // M_6482's window (the generic hit path)
  const w = C.dash.notifies.filter((n) => n.class === 'TresAnimNotifyState_Attack');
  must(w.length === 1, 'M_6482: expected one attack window');
  const n = w[0];
  const group = String(prop(n, 'CollisionGrpName')), rowName = String(prop(n, 'm_AttackDataIDName'));
  add('rush.window.from', n.time, 's', 'M_6482 TresAnimNotifyState_Attack', 'the window opens with each M_6482 (a new play: the hit list is new)', '0x1405620F0');
  add('rush.window.to', n.time + (n.duration || 0), 's', 'M_6482 TresAnimNotifyState_Attack (time + duration)', 'the whole clip (looped: the window stays while the dash runs)', '0x1405620F0');
  add('rush.window.group', group, '-', 'M_6482 TresAnimNotifyState_Attack CollisionGrpName', 'the group of the drill weapon\'s TresAtkColl0', '-');
  add('rush.window.row', rowName, '-', 'M_6482 TresAnimNotifyState_Attack m_AttackDataIDName', 'the attack row (sora-combat-attack-data)', '-');
  add('rush.window.interval', prop(n, 'AttackInterval') || 0, 's', 'M_6482 TresAnimNotifyState_Attack AttackInterval', 'the hit list clears every interval (one dash = 0.3 s)', '-');
  must(/L"SF_WARPRUSH"/.test(rename) && /L"SF_TRWARPRUSH"/.test(rename) && /iVar1 != 0x1a/.test(rename), RENAME + ': the rename changed');
  add('rush.k26.row', 'SF_TRWARPRUSH', '-', RENAME + ' (the ShootFlow TresAtk begin pre-processor): kind 0x1A, SF_WARPRUSH -> SF_TRWARPRUSH', 'the row Noise Flux hits with', RENAME);
  add('rush.k9.row', rowName, '-', 'M_6482 m_AttackDataIDName (no rename for kind 9)', 'the row Drill Dive hits with', RENAME);
  for (const name of [rowName, 'SF_TRWARPRUSH']) {
    const a = ATTACK.get(name);
    must(a, 'p_ex001_AttackData has no ' + name);
    add(`rush.row.${name.toLowerCase()}.power`, a.m_Power, '-', 'p_ex001_AttackData ' + name + ' m_Power', String(a.m_DamageKind).split('::').pop() + ', ' + String(a.m_DamageAttribute).split('::').pop() + (a.m_bIsMagicAttack ? ', magic' : ''), '-');
  }

  // ---------------------------------------------------------------------------------------------- the constants
  const K = [
    ['rush.lift.height', 200, 'cm', UPD + ' 0x1413BCC9A movss xmm1,[145C9C544h] = f32 200 -> LiftCheck vt+0x398', 'sub 0: the lift (sub 3) when the ground is closer than this'],
    ['rush.dash.time', 0.3, 's', UPD + ' 0x1413BC80E / 0x1413BCA96 mov [rdi+0A4h],3E99999Ah = f32 0.3', '+0xA4: the dash timer of each warp'],
    ['rush.dash.rate', 3.3333333, '1/s', MOVE + ' 0x1413BE682 / 0x1413BE6DE / 0x1413BE6E6 mulss [145E0D9A0h] = f32 3.3333333', 'the dash: +0xF8 - +0xA4 x this x (+0xF8 - +0xD8) (1 / 0.3: from +0xD8 at the warp to +0xF8)'],
    ['rush.warp.down', -100, 'cm', WARP + ' 0x1413C4183 addss [145CADFE0h] = f32 -100', 'the target point: its component point (vt+0xB88) + this in z'],
    ['rush.warp.turn', 120, 'deg', WARP + ' 0x1413C4198 / 0x1413C446E subss [145C9C52Ch] = f32 120', '+0xAC -= this per warp, and again after the last lock of an entry'],
    ['rush.warp.reach', 800, 'cm', WARP + ' 0x1413C41C8 movss [145C9C560h] = f32 800', '+0xD8 = P - this x D, +0xF8 = P + this x D'],
    ['rush.warp.camera_mode', 2, 'int', WARP + ' 0x1413C42DE mov r8d,2 -> 0x14140DF20(ctrl, 1, 2, +0xD0, +0xE4)', 'the attack camera of a new target actor (mode 2: a camera actor of its own; combat/camera.rs keeps the mode only)'],
  ];
  for (const [id, v, unit, src, ev] of K) add(id, v, unit, src, ev, id.startsWith('rush.dash.rate') ? MOVE : id.startsWith('rush.warp') ? WARP : UPD);
  must(/3\.3333333/.test(move), MOVE + ': the dash rate changed');
  must(/0x686/.test(warp) && /0x76c/.test(warp) && /L"WarpRush"/.test(warp), WARP + ': the effect changed');

  // ---------------------------------------------------------------------------------------------- the effect
  // pawn vt+0x1428(1) -> the right-hand weapon actor; its +0x580 = TresEffectAttach0 (FUN_141767630 walks its
  // DesignatedAttachDataList by Key) -> "WarpRush" -> FUN_1405B9370(pawn+0x440, data, group, 0).
  const CONTENT = P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content');
  for (const [kind, , id] of KIND_FORM) {
    const unit = FORM_OF[kind].units[0];
    const bpName = unit.split('/').pop();
    const j = readJson(path.join(CONTENT, 'Blueprints', 'Player', 'p_ex001', bpName + '.uasset.json'));
    const ea = j.exports.find((e) => e.objectName === 'TresEffectAttach0');
    must(ea, bpName + ': no TresEffectAttach0');
    const list = (ea.data.props.find((p) => p.name === 'DesignatedAttachDataList') || {}).value;
    const it = list && list.items.find((x) => x.find((p) => p.name === 'Key' && p.value === 'WarpRush'));
    must(it, bpName + ': no WarpRush');
    const ref = it.find((p) => p.name === 'DesignatedAttachData').value;
    const imp = j.imports[-ref - 1], pkg = j.imports[-imp.outerIndex - 1].objectName;
    const ead = readJson(path.join(CONTENT, pkg.replace(/^\/Game\//, '') + '.uasset.json'));
    const groupId = kind === 26 ? 0x76C : 0x686;
    const parts = [];
    for (const L of ead.exports[0].data.props.find((p) => p.name === 'EffectDataList').value.items) {
      for (const a of L.find((p) => p.name === 'EffectDataList').value.items) {
        const g = a.find((p) => p.name === 'GroupID').value;
        const eff = ead.imports[-a.find((p) => p.name === 'AttachEffect').value - 1].objectName;
        const ao = a.find((p) => p.name === 'AttachObjectData').value;
        const type = String(ao.find((p) => p.name === 'AttachType').value).replace('SQEX_ATTACH_OBJECT_', '');
        const sock = ao.find((p) => p.name === 'ObjectSearchSocketName').value;
        must(g === groupId, imp.objectName + ': group ' + g);
        parts.push(eff + '@' + type + ':' + sock);
      }
    }
    add('rush.k' + kind + '.effect', imp.objectName, '-', bpName + ' TresEffectAttach0 DesignatedAttachDataList Key WarpRush -> ' + pkg, 'the effect of each warp (FUN_1405B9370 with group ' + groupId + ')', WARP);
    add('rush.k' + kind + '.effect_group', groupId, 'int', WARP + ' 0x1413C4332 / 0x1413C4337 (cmove on kind 0x1A)', 'the group of the WarpRush data; every entry of ' + imp.objectName + ' has it', WARP);
    add('rush.k' + kind + '.effect_parts', parts, '-', pkg + ' EffectDataList (AttachEffect @ AttachType : socket)', 'what the host draws for the effect (OWNER = the weapon actor)', WARP);
    add('rush.k' + kind + '.effect_socket', FORM_OF[kind].units[0].match(/:(\w+)=/)[1], '-', 'sheets/port/sora-keyblade-forms ' + id + ' RWeapon socket', 'where the weapon actor hangs on Sora (the effect\'s owner)', WARP);
  }

  // ---------------------------------------------------------------------------------------------- the hit shape (FK)
  // The StickL capsule of the drill weapon (its own mesh: ATK_00 -> ATK_01, m_Size.x) posed over M_6482: Sora's clip (root
  // motion left out) + the weapon's socket on him + the weapon's skeleton posed by its own clip of the same name.
  const sp = require(path.join(lib.ROOT, 'tools', 'sora_pack.js'));
  const qmulH = (a, b) => [a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1], a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0], a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3], a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]];
  const qrot = (q, v) => { const tx = 2 * (q[1] * v[2] - q[2] * v[1]), ty = 2 * (q[2] * v[0] - q[0] * v[2]), tz = 2 * (q[0] * v[1] - q[1] * v[0]); return [v[0] + q[3] * tx + (q[1] * tz - q[2] * ty), v[1] + q[3] * ty + (q[2] * tx - q[0] * tz), v[2] + q[3] * tz + (q[0] * ty - q[1] * tx)]; };
  const under = (c, pp) => { const wv = qrot(pp.q, [pp.s[0] * c.t[0], pp.s[1] * c.t[1], pp.s[2] * c.t[2]]); return { q: qmulH(pp.q, c.q), t: [wv[0] + pp.t[0], wv[1] + pp.t[1], wv[2] + pp.t[2]], s: [pp.s[0] * c.s[0], pp.s[1] * c.s[1], pp.s[2] * c.s[2]] }; };
  const point = (T, wv) => { const r = qrot(T.q, [T.s[0] * wv[0], T.s[1] * wv[1], T.s[2] * wv[2]]); return [r[0] + T.t[0], r[1] + T.t[1], r[2] + T.t[2]]; };
  const rotQ = (r) => { const h = Math.PI / 360, SP = Math.sin(r.pitch * h), CP = Math.cos(r.pitch * h), SY = Math.sin(r.yaw * h), CY = Math.cos(r.yaw * h), SR = Math.sin(r.roll * h), CR = Math.cos(r.roll * h); return [CR * SP * SY - SR * CP * CY, -CR * SP * CY - SR * CP * SY, CR * CP * SY - SR * SP * CY, CR * CP * CY + SR * SP * SY]; };
  const ID = lib.MAIN.identification;
  const SORA_BONES = sp.buildSkeleton(sp.readGlb(P('assets_converted', 'meshes', sp.CONTENT, sp.gamePathToContent(ID.skeletal_mesh) + '.glb')).json);
  const SORA_SOCKETS = {};
  for (const e of sp.loadPkg(sp.gamePathToContent(ID.skeleton)).exports.filter((x) => x.class === 'SkeletalMeshSocket')) {
    const pr = sp.propsOf(e);
    const loc = pr.RelativeLocation ? pr.RelativeLocation.value : { x: 0, y: 0, z: 0 }, rot = pr.RelativeRotation ? pr.RelativeRotation.value : { pitch: 0, yaw: 0, roll: 0 }, sc = pr.RelativeScale ? pr.RelativeScale.value : { x: 1, y: 1, z: 1 };
    SORA_SOCKETS[pr.SocketName.value] = { bone: pr.BoneName.value, T: { q: rotQ(rot), t: [loc.x, loc.y, loc.z], s: [sc.x, sc.y, sc.z] } };
  }
  const meshYaw = ID.mesh_component.relative_rotation_ue_deg.yaw * Math.PI / 180, cyaw = Math.cos(meshYaw), syaw = Math.sin(meshYaw);
  const actorSpace = (wv) => [wv[0] * cyaw - wv[1] * syaw, wv[0] * syaw + wv[1] * cyaw, wv[2]];
  const localT = (b, tr, time) => ({ q: tr.rotation ? sp.sampleChan(tr.rotation, time)(tr.rotation.ue_values) : b.ue.rotation, t: tr.translation ? sp.sampleChan(tr.translation, time)(tr.translation.ue_values) : b.ue.translation, s: tr.scale ? sp.sampleChan(tr.scale, time)(tr.scale.ue_values) : b.ue.scale });
  const byBone = (a) => { const m = new Map(); if (a) for (const c of a.chans) { if (!m.has(c.bone)) m.set(c.bone, {}); m.get(c.bone)[c.path] = c; } return m; };
  const boneT = (bones, chans, k, time, rootMotion) => { const chain = []; for (let i = k; i >= 0; i = bones[i].parent) chain.unshift(i); let T = null; for (const i of chain) { const l = localT(bones[i], (i === 0 && rootMotion) ? {} : (chans.get(i) || {}), time); T = T ? under(l, T) : l; } return T; };
  const ITEMS = new Map([...lib.MAIN.items, ...lib.OV.items].filter((i) => i.kind === 'animation').map((i) => [i.animation_name, i]));
  const soraAnim = (seq) => { const it = ITEMS.get(seq); return it && it.source_asset ? sp.loadAnimation(it.source_asset + '.' + path.basename(it.source_asset), SORA_BONES) : null; };
  const weaponOf = (model) => {
    const dir = P('build', 'sora_pack', 'keyblades', model);
    must(fs.existsSync(path.join(dir, 'model.json')), 'no weapon pack ' + model);
    const mj = readJson(path.join(dir, 'model.json')), sj = readJson(path.join(dir, 'sockets.json'));
    return { mj, sockets: sj.weapon_sockets || {}, bones: sp.buildSkeleton(sp.readGlb(P('assets_converted', 'meshes', sp.CONTENT, sp.gamePathToContent(mj.source_mesh) + '.glb')).json) };
  };
  const weaponAnim = (wp, logical, seq) => { const c = wp.mj.clips.find((x) => (x.logical_names || []).includes(logical)) || wp.mj.clips.find((x) => x.sequence === seq); return c ? sp.loadAnimation(c.asset + '.' + c.sequence, wp.bones) : null; };
  for (const [kind, , id] of KIND_FORM) {
    const m = FORM_OF[kind].units[0].match(/^(\w+)@(\w+):(\w+)=(.*)$/);
    must(m && FORM_OF[kind].units.length === 1, id + ': expected one weapon');
    const bpName = m[4].split('/').pop(), model = bpName.replace(/_[Ww]ep$/, ''), socket = m[3];
    // the shape group of the weapon
    const b = blueprint('/Game/Blueprints/Player/p_ex001/' + bpName);
    const coll = Object.values(b.comps).find((c) => Array.isArray(c.CollisionShapesSrc));
    const s = coll ? coll.CollisionShapesSrc.filter((x) => x.m_GrpName === group) : [];
    must(s.length === 1, bpName + ': expected one ' + group + ' shape');
    const sh = s[0];
    must(String(sh.m_ShapeType).endsWith('CAPSULE2') && String(sh.m_AttachType1).endsWith('NORMAL') && String(sh.m_AttachType2).endsWith('NORMAL'), bpName + ' ' + group + ': not a CAPSULE2 on its own mesh');
    must(!sh.m_IncSize.x && !sh.m_RelativeLocation1.x && !sh.m_RelativeLocation1.y && !sh.m_RelativeLocation1.z && !sh.m_RelativeLocation2.x && !sh.m_RelativeLocation2.y && !sh.m_RelativeLocation2.z, bpName + ' ' + group + ': offsets / growth');
    const k = 'rush.k' + kind + '.shape';
    const src = bpName + ' TresAtkColl0 ' + group;
    add(k + '.blueprint', bpName, '-', src, 'the weapon whose group M_6482\'s window names', '-');
    add(k + '.type', 'CAPSULE2', '-', src + ' m_ShapeType', 'between m_SocketName1 ' + sh.m_SocketName1 + ' and m_SocketName2 ' + sh.m_SocketName2 + ' of the weapon\'s own mesh (NORMAL), no offsets, no growth', '-');
    add(k + '.radius', sh.m_Size.x, 'cm', src + ' m_Size.x', 'the capsule radius', '-');
    add(k + '.map_hit', String(sh.m_MapHitType).split('::').pop(), '-', src + ' m_MapHitType', 'no map hits', '-');
    // the track
    const wp = weaponOf(model);
    const seq = C.dash.sequence;
    const it = ITEMS.get(seq);
    const sa = soraAnim(seq);
    must(it && sa, seq + ': no Sora animation');
    const sock = SORA_SOCKETS[socket];
    must(sock, 'no Sora socket ' + socket);
    const hand = SORA_BONES.findIndex((x) => x.name === sock.bone);
    const anim = weaponAnim(wp, 'M_6482', seq);
    const frames = Math.max(2, it.num_frames || 2), out = [];
    for (let fr = 0; fr < frames; fr++) {
      const t = fr * it.duration_s / (frames - 1);
      const base = under(sock.T, boneT(SORA_BONES, byBone(sa), hand, Math.min(t, sa.length), sa.props.bEnableRootMotion === true));
      for (const name of [sh.m_SocketName1, sh.m_SocketName2]) {
        const ws = wp.sockets[name];
        const kb = wp.bones.findIndex((x) => x.name === (ws ? ws.bone : name));
        must(kb >= 0, model + ': no ' + name);
        let T = under(boneT(wp.bones, byBone(anim), kb, anim ? Math.min(t, anim.length) : 0, false), base);
        if (ws) T = under({ q: rotQ({ pitch: ws.local_rotation[0], yaw: ws.local_rotation[1], roll: ws.local_rotation[2] }), t: ws.local_location, s: [1, 1, 1] }, T);
        out.push(...actorSpace(point(T, [0, 0, 0])).map((v) => Math.round(v * 10) / 10));
      }
    }
    add(k + '.track', out, 'cm', 'FK: sequence ' + seq + ' (M_6482, root motion left out) + ' + model + ' on ' + socket + ' posed by its ' + (anim ? 'M_6482' : 'reference pose') + ' + ' + sh.m_SocketName1 + ' / ' + sh.m_SocketName2 + ' (sockets.json)', 'the capsule ends in ACTOR space per frame (frame k at k x ' + it.duration_s.toFixed(4) + ' / ' + (frames - 1) + ' s): [end1 x, y, z, end2 x, y, z] x ' + frames, '-');
  }
  return rows;
};
