// (SHOT-A pellet) Family `pellet`: KH3's update 0x1413B9910 (TresPlayerState_ShootFlow vtable 0x14693CA58 +0x488) of kind 4
// "HE散弾" (sf_pellet_0) "Atomic Deluge" (Hero's Origin's MAX kind) and kind 24 "Fruit Crusher" (Grand Chef's MAX kind, SHOT-B).
// Rows `pellet.*` of sheets/port/sora-shotlocks.json for kh-core magic/shotlocks/pellet.rs.
//
// THE CODE (decompiled C re/decomp/1413B/1413B9910.c, its disassembly build/sora_tmp/weapons/SHOT-B/spec/scripts/
// dis_1413B9910.txt; the spawner re/decomp/1413C/1413C1E40.c + dis_1413C1E40.txt):
//   sub 0: gravity off (pawn vt+0x1728(0)), Flying stopped (vt+0x17A0(5, 1)); ChangeWeapon (vt+0x13C8 0x14144A9F0)
//          (weapon type, form 0x13 for kind 0x18 else 0x0B, grow 0 = LVL01, 0, 0.0, 0, 0); pawn vt+0x1400 (0x1405BDC20 ->
//          equipment vt+0x518 0x14178EE30: the form set's m_TransformSetData is equipped instead of m_Equips, equipment
//          +0x2C8 |= 4); Play "M_B481" (blend -1, mode 2, group +0xC0, rate 1), voice +0xC4 when the length > 0;
//          +0x84 |= 0x10 (m_bFaceAt); sub 4.
//   sub 4 / 5: at the clip's end: pawn vt+0x13E8(2, 0, 0.0) (0x1405BD770: ChangeWeapon(type, the form on, grow 2 =
//          LVL03, 0, 0.0, 0, 1): the LV3 m_Equips, the transform set off); Play "ShootFlowEnd" (group 0); +0x84 &= ~1
//          (m_bEnableChange off); sub 2.
//   other (sub 2): m_bEnableChange (ShootFlowEnd's TresEnableChange) and +0xB0 != 0: pawn vt+0x1408 (0x1405BD8F0 ->
//          equipment vt+0x520 0x14177ED40: the transform set off), ChangeWeapon(type, +0xB0, +0xB1, 0, 0.0, 0, 0) when the
//          controller exists, +0xB0 = 0; at the clip's end Finish (vt+0x270).
//   every tick: flagsCC bit 0 (the TresAtk begin handler slot 64 0x1413B5430 sets it and +0xA0 = 0.3 for kind 4): while
//          +0x90 < +0x94 the spawner vt+0x490 = 0x1413C1E40(dt), else the bit off; then SetFace(false, 0) (vt+0x390).
//   spawner 0x1413C1E40: +0xA0 > 0: -= dt, still > 0: return (else 0). The locked entry +0x110 (ctrl+0xB08, 0x34 bytes,
//          count ctrl+0xB10): gone / invalid: +0x110 += 1, +0x114 = 0. Weapon out (pawn vt+0x1330) else return. p =
//          pawn vt+0x1450("SF_SHIELDSHOT", T, T, dir, team, fire 1) with T = the entry's component location (vt+0xB88)
//          and dir = T - pawn vt+0x6E0 (normalised when |.|^2 > 1e-8); none: return. p.vt+0x1100(+0x88, +0x8C);
//          +0x114 += 1, +0x90 += 1; +0x114 >= entry lock count (+0xC): +0x114 = 0, +0x110 += 1; +0xA0 = (6 - +0x90) / 60,
//          < 0: 0.
// THE DATA: M_B481 of p_ex001_AnimSet11 (build/sora_pack/keyblades/sora_forms_manifest.json), the equip sets of the weapon
// forms (sheets/port/sora-keyblade-forms.json, from TresEquipKeybladeData m_EquipMap), the transform units' TresAtkColl0
// (Blueprints/Player/p_ex001/w_so031_wep, w_so141_Wep), p_ex001_AttackData SF_SHIELDSHOT / SF_SHIELDSHOT_BLOW.
'use strict';

module.exports = (lib) => {
  const { row, clip, prop, blueprint, projectileSet, ATTACK, decomp, must, readJson, P } = lib;
  const rows = [];
  const add = (...a) => rows.push(row(...a));
  const UPD = '0x1413B9910';
  const SPAWN = '0x1413C1E40';
  const upd = decomp(UPD);
  const spawn = decomp(SPAWN);
  must(upd && spawn, 'no decompiled 0x1413B9910 / 0x1413C1E40 in re/decomp');

  // the weapon forms of sub 0
  must(/== 0x18\)/.test(upd) && /uVar6 = 0x13;/.test(upd) && /uVar6 = 0xb;/.test(upd), UPD + ': the weapon form switch changed');
  add('pellet.weapon_form.kind', 24, 'kind', UPD + ' 0x1413B9AD7 cmp [rdi+8Ch],18h', 'the kind that takes the other weapon form', UPD);
  add('pellet.weapon_form.that_kind', 19, 'int', UPD + ' 0x1413B9AF5 mov r8b,13h -> ChangeWeapon form', 'TRES_WEAPON_FORM_13 = Frying Pan (form.SH2, Grand Chef)', UPD);
  add('pellet.weapon_form.other', 11, 'int', UPD + ' 0x1413B9AFC mov r8b,0Bh -> ChangeWeapon form', 'TRES_WEAPON_FORM_0B = Counter Shield (form.SH, Hero\'s Origin): kind 4', UPD);
  add('pellet.end_grow', 2, 'int', UPD + ' 0x1413B9972 mov dl,2 -> pawn vt+0x13E8 = 0x1405BD770(2, 0, 0.0)', 'ChangeWeapon(type, the form on, grow 2 = ETresWeaponGrowLvl TRES_WEAPON_GROW_LVL03): the LV3 parts during ShootFlowEnd', '0x1405BD770');

  // the parts of those sets (sora-keyblade-forms: units = LVL01, grow_units "G2:" / "G3:", transform_units)
  const forms = readJson(P('sheets', 'port', 'sora-keyblade-forms.json'));
  const fr = forms.rows.map((r) => Object.fromEntries(forms.columns.map((c, i) => [c, r[i]])));
  const shapeOf = new Map();
  for (const f of [11, 19]) {
    const r = fr.find((x) => x.weapon_form_value === f && x.weapon_type > 0);
    must(r && r.transform_units && r.transform_units.length, 'sora-keyblade-forms: no transform units of weapon form ' + f);
    add(`pellet.units.f${f}.transform`, r.transform_units, '-', 'sheets/port/sora-keyblade-forms ' + r.id + ' transform_units (m_TransformSetData of grow LVL01)', 'what the hosts draw from sub 0 (ChangeWeapon + pawn vt+0x1400 -> equipment vt+0x518 0x14178EE30) to M_B481\'s end', UPD);
    const g3 = (r.grow_units || []).filter((u) => u.startsWith('G3:')).map((u) => u.slice(3));
    must(g3.length, r.id + ': no G3 units');
    add(`pellet.units.f${f}.g2`, g3, '-', 'sheets/port/sora-keyblade-forms ' + r.id + ' grow_units G3 (m_Equips of grow LVL03)', 'what the hosts draw from M_B481\'s end (ChangeWeapon grow 2) until the weapon form of Begin comes back', '0x1405BD770');
    for (const u of r.transform_units) shapeOf.set(u.split('/').pop(), r.id);
  }

  // the clips
  const main = clip('M_B481', 'p_ex001_AnimSet11');
  must(main, 'no clip M_B481 of p_ex001_AnimSet11');
  add('pellet.clip', 'M_B481', '-', 'FName literal 0x146933838 (' + UPD + ' 0x1413B9B36); p_ex001_AnimSet11 M_B481 -> sequence ' + main.sequence, 'the attack clip (slot 0, blend -1, mode 2, group +0xC0, rate 1): AnimSet11 is the anim set of both weapon forms 0x0B / 0x13', UPD);
  add('pellet.clip_sequence', main.sequence, '-', 'p_ex001_AnimSet11 M_B481 -> its sequence', 'the anim table row (AnimId::from_name)', UPD);
  add('pellet.clip_length', main.length, 's', 'sequence ' + main.sequence + ' duration', 'sub 4 waits for its end', UPD);
  const end = clip('ShootFlowEnd');
  must(end, 'no clip ShootFlowEnd');
  add('pellet.end_clip_length', end.length, 's', 'FName literal 0x146933848; sequence ' + end.sequence + ' duration', 'sub 2 finishes at its end (Play with group 0)', UPD);
  const change = (end.notifies || []).find((n) => n.class === 'TresAnimNotify_EnableChange');
  must(change, 'ShootFlowEnd has no TresEnableChange');
  add('pellet.end_change', change.time, 's', 'ShootFlowEnd TresAnimNotify_EnableChange', 'from here the weapon form of Begin comes back (sub 2)', UPD);

  // M_B481's attack window (the generic hit path) and the spawn start it gives
  const w = (main.notifies || []).filter((n) => n.class === 'TresAnimNotifyState_Attack');
  must(w.length === 1, 'M_B481: expected one attack window');
  const n = w[0];
  const group = String(prop(n, 'CollisionGrpName')), rowName = String(prop(n, 'm_AttackDataIDName'));
  add('pellet.window.from', n.time, 's', 'M_B481 TresAnimNotifyState_Attack', 'the window opens; its begin (slot 64 0x1413B5430) also sets flagsCC bit 0 and +0xA0 = 0.3 for kind 4: the spawner starts', '0x1413B5430');
  add('pellet.window.to', n.time + (n.duration || 0), 's', 'M_B481 TresAnimNotifyState_Attack (time + duration)', 'the window closes', '0x1405620F0');
  add('pellet.window.group', group, '-', 'M_B481 TresAnimNotifyState_Attack CollisionGrpName', 'the group of the equipped transform unit\'s TresAtkColl0', '-');
  add('pellet.window.row', rowName, '-', 'M_B481 TresAnimNotifyState_Attack m_AttackDataIDName', 'the attack row (sora-combat-attack-data)', '-');
  add('pellet.window.interval', prop(n, 'AttackInterval') || 0, 's', 'M_B481 TresAnimNotifyState_Attack AttackInterval (absent: 0)', 'no interval: one hit per target in the window', '-');

  // the group's shapes on the transform units (w_so031_wep / w_so141_Wep TresAtkColl0)
  for (const [bp, formId] of shapeOf) {
    const b = blueprint('/Game/Blueprints/Player/p_ex001/' + bp);
    const coll = Object.values(b.comps).find((c) => Array.isArray(c.CollisionShapesSrc));
    const s = coll ? coll.CollisionShapesSrc.filter((x) => x.m_GrpName === group) : [];
    must(s.length === 1, bp + ': expected one ' + group + ' shape');
    const sh = s[0];
    const k = `pellet.shape.${bp.toLowerCase()}.${group.toLowerCase()}`;
    const src = bp + ' TresAtkColl0 ' + group;
    add(k + '.type', String(sh.m_ShapeType).split('::').pop(), '-', src + ' m_ShapeType', 'the transform unit of ' + formId, '-');
    add(k + '.radius', sh.m_Size.x, 'cm', src + ' m_Size.x', 'm_IncSize ' + sh.m_IncSize.x + ', no curve', '-');
    add(k + '.attach', String(sh.m_AttachType1).split('::').pop(), '-', src + ' m_AttachType1', 'OWNER_ATTACHED_MESH = Sora\'s mesh socket; NORMAL = the unit\'s own mesh socket', '-');
    add(k + '.socket', sh.m_SocketName1, '-', src + ' m_SocketName1', 'kh_root on bone kh_sk: B481 is a root-motion clip (its root track lifts the actor up to 271 cm), so kh_sk keeps its reference pose: the port\'s actor location (as SHOT-C\'s ultima)', '-');
    add(k + '.offset', [sh.m_RelativeLocation1.x, sh.m_RelativeLocation1.y, sh.m_RelativeLocation1.z], 'cm', src + ' m_RelativeLocation1', 'added to the socket', '-');
  }

  // the spawner's constants
  must(/L"SF_SHIELDSHOT"/.test(spawn) && /\(6 - iVar3\) \* 0\.016666668/.test(spawn), SPAWN + ': the spawner changed');
  add('pellet.proj_name', 'SF_SHIELDSHOT', '-', SPAWN + ' 0x1413C1FCF FName literal 0x146933868', 'the projectile pawn vt+0x1450 spawns (the weapon\'s projectile set resolves it: w_so030 -> ' + projectileSet('w_so030').get('SF_SHIELDSHOT') + ', w_so140 -> ' + projectileSet('w_so140').get('SF_SHIELDSHOT') + ')', SPAWN);
  add('pellet.spawn.fire', true, '-', SPAWN + ' 0x1413C20EE mov byte ptr [rsp+30h],1', 'vt+0x1450 bFireNow = 1 (no homing target is set: only vt+0x1100(cmd, kind))', SPAWN);
  add('pellet.spawn.base', 6, 'count', SPAWN + ' 0x1413C21CF mov eax,6', '+0xA0 = (6 - spawnIndex) * step after each spawn', SPAWN);
  add('pellet.spawn.rate', 60, '1/s', SPAWN + ' 0x1413C21DE mulss [145D314C4h] = f32 0.016666668 = 1 / 60 as f32', 'the step of +0xA0 per count is 1 / rate (one 60 Hz tick); the direction test |d|^2 > 1e-8 (0x1413C2054, [145C9C488h]) is Projectile::new\'s own', SPAWN);
  add('pellet.spawn.delay', 0.3, 's', '0x1413B5430 (anim interface slot 64, TresAtk begin): kinds 4 / 0x2A +0xA0 = 0.3', 'the first spawn waits this long after M_B481\'s window opens (shotlocks/mod.rs on_notifies)', '0x1413B5430');

  // the rows
  for (const name of ['SF_SHIELDSHOT', 'SF_RESHIELDSHOT', rowName]) {
    const a = ATTACK.get(name);
    must(a, 'p_ex001_AttackData has no ' + name);
    add(`pellet.row.${name.toLowerCase()}.power`, a.m_Power, '-', 'p_ex001_AttackData ' + name + ' m_Power', String(a.m_DamageKind).split('::').pop() + ', ' + String(a.m_DamageAttribute).split('::').pop() + (a.m_bIsMagicAttack ? ', magic' : ''), '-');
  }
  return rows;
};
