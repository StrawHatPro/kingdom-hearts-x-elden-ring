// (SHOT-C) Family `ultima`: KH3's kind 29 UW_ULTIMA "Infinity Circle", Ultima Weapon's shotlock (weapon type 15; no MAX
// entry in SelectKind 0x141411B80). Rows `ultima.*` of sheets/port/sora-shotlocks.json for kh-core magic/shotlocks/ultima.rs.
//
// THE CODE (re/decomp, read in decompiled C; the delegate from the ShootFlow vtable 0x14693CA58 read in base/KH3.exe):
//   Begin 0x1413A5CD0 case 0x1D: update = thunk FUN_1404EFB10 -> vt+0x500 = 0x1413BB3C0, name UW_ULTIMA, group 0, voice 0.
//   Update 0x1413BB3C0:
//     sub 0: gravity off (vt+0x1728(0)); Flying, stopped (vt+0x17A0(5, 1)); the weapon change vt+0x13C8(weapon type,
//            9, 0, ...) = ChangeWeapon 0x14144A9F0 to weapon form 9 (Ultimate Form's weapon w_so192 and its anim set
//            AnimSet9, F-LOCO / F-ULTIMATE); Play "M_9481" (slot 0, blend -1, mode 2, group 0, rate 1), voice 0 when the
//            length > 0; m_bFaceAt (+0x84 bit 4) = 1; sub 4. No lift (unlike KC).
//     sub 3..5: at the clip's end: Play "ShootFlowEnd" (blend -1, mode 2, group 0), voice 0; m_bEnableChange = 0; sub 2.
//     sub 2:    once m_bEnableChange is set again (ShootFlowEnd's TresEnableChange, 0.3 s) and +0xB0 != 0: vt+0x1408(),
//            vt+0x13C8(weapon type, +0xB0, +0xB1, ...) (the weapon form of Begin back), WorldSlow off
//            (FUN_14140E500(ctrl, 0): ctrl+0x878 bit 3; (SHOT-FIX) not the release, which is End's slot 809), +0xB0 = 0;
//            at the clip's end Finish (vt+0x270).
//   End 0x1413ABA70 (Sora): +0xB0 != 0 -> the same restore (a dropped state still gives the weapon back).
//   Shot handler 0x1413B4A70: kind 0x1D only sets flagsCC bit 0 (no TresShot in M_9481: nothing reads it).
// THE DATA: M_9481's notifies (build/sora_pack/keyblades/sora_forms_manifest.json: clip 9481 of p_ex001_AnimSet9), the
// window's group SF_Range of Player/p_ex001/w_so192_Wep TresAtkColl0, p_ex001_AttackData SF_ULTIMA.
'use strict';

module.exports = (lib) => {
  const { row, clip, prop, blueprint, ATTACK, must, readJson, P } = lib;
  const rows = [];
  const add = (...a) => rows.push(row(...a));
  const UPD = '0x1413BB3C0';

  add('ultima.kind', 29, 'kind', 'TresShootFlowData row 29 (アルテマ, kh3_system.command.sf_ultima_0)', 'the kind SelectKind 0x141411B80 gives weapon type 15 (Ultima Weapon); no MAX entry for 15: the kind stays at MAX', '0x141411B80');
  add('ultima.ability', 'TEXT_SHOOTFLOW_KB14_00', '-', 'TresAbilityData A00285 (m_NameTextID kh3_system.command.sf_ultima_0, help kh3_system.ability.detail_txt_0290)', 'the display ability Ultima Weapon lists (m_ShootflowAbilitys); "Infinity Circle"', '-');
  add('ultima.weapon_form', 9, 'int', UPD + ' sub 0: pawn vt+0x13C8(weapon type, 9, 0, ...)', 'ChangeWeapon 0x14144A9F0 to weapon form 9 = Ultimate Form (form.SW, w_so192 on R_wep_skt, AnimSet9); restored at sub 2 / End with Begin\'s +0xB0 / +0xB1', UPD);

  // the weapon form's parts: the Ultimate Form row of sora-keyblade-forms (P3; the same parts F-ULTIMATE draws)
  const forms = readJson(P('sheets', 'port', 'sora-keyblade-forms.json'));
  const fr = forms.rows.map((r) => Object.fromEntries(forms.columns.map((c, i) => [c, r[i]]))).find((r) => r.id === 'form.SW');
  must(fr && fr.units && fr.units.length, 'sora-keyblade-forms has no form.SW units');
  add('ultima.weapon_units', fr.units, '-', 'sheets/port/sora-keyblade-forms form.SW units (equip data of weapon form 9)', 'the parts drawn instead of the Keyblade while weapon form 9 is on (kh-er-render keyblade.rs draw_form_weapon_parts, form "09")', UPD);

  // the clips
  const main = clip('M_9481', 'p_ex001_AnimSet9');
  must(main, 'no clip M_9481 of p_ex001_AnimSet9');
  add('ultima.clip', 'M_9481', '-', 'p_ex001_AnimSet9 M_9481 -> sequence ' + main.sequence, 'the attack clip (slot 0, blend -1, mode 2, group 0)', UPD);
  add('ultima.clip_length', main.length, 's', 'sequence ' + main.sequence + ' duration', 'sub 4 waits for its end', UPD);
  const end = clip('ShootFlowEnd');
  must(end, 'no clip ShootFlowEnd');
  add('ultima.end_clip_length', end.length, 's', 'sequence ' + end.sequence + ' duration', 'sub 2 finishes at its end', UPD);
  const change = (end.notifies || []).find((n) => n.class === 'TresAnimNotify_EnableChange');
  must(change, 'ShootFlowEnd has no TresEnableChange');
  add('ultima.end_change', change.time, 's', 'ShootFlowEnd TresAnimNotify_EnableChange', 'the weapon form comes back from here (and WorldSlow off); (SHOT-FIX) the aim mode closes with End\'s slot 809', UPD);

  // the hit window of M_9481
  const w = (main.notifies || []).filter((n) => n.class === 'TresAnimNotifyState_Attack');
  must(w.length === 1, 'M_9481: expected one attack window');
  const n = w[0];
  const group = String(prop(n, 'CollisionGrpName')), rowName = String(prop(n, 'm_AttackDataIDName'));
  add('ultima.window.from', n.time, 's', 'M_9481 TresAnimNotifyState_Attack', 'the window opens (the generic hit path: combat/hit.rs on_notifies)', '0x1405620F0');
  add('ultima.window.to', n.time + (n.duration || 0), 's', 'M_9481 TresAnimNotifyState_Attack (time + duration)', 'the window closes', '0x1405620F0');
  add('ultima.window.group', group, '-', 'M_9481 TresAnimNotifyState_Attack CollisionGrpName', 'the shape group of weapon form 9\'s weapon w_so192', '-');
  add('ultima.window.row', rowName, '-', 'M_9481 TresAnimNotifyState_Attack m_AttackDataIDName', 'the attack row (sora-combat-attack-data carries it)', '-');
  add('ultima.window.interval', prop(n, 'AttackInterval') || 0, 's', 'M_9481 TresAnimNotifyState_Attack AttackInterval', 'the hit list clears every interval: one hit per target per interval', '-');
  add('ultima.window.hit_effect_group', String(prop(n, 'm_HitEffectGrpName')), '-', 'M_9481 m_bOverrideHitEffect true, m_HitEffectGrpName', 'NO such m_HitEffects group on w_so192_Wep (None, genei) or p_ex001_PawnBase; of all p_ex001 blueprints only w_so072_Wep has ReflecLaser: the chooser 0x140628960 keeps the attribute default', '0x140628960', 'inferred');
  add('ultima.window.hit_se', String(prop(n, 'm_WeaponHitSEAliasID')).split('::').pop(), '-', 'M_9481 m_WeaponHitSEAliasID', 'the weapon hit SE alias of the window', '-');

  // the SF_Range shape of w_so192_Wep TresAtkColl0
  const bp = blueprint('/Game/Blueprints/Player/p_ex001/w_so192_Wep');
  const coll = Object.values(bp.comps).find((c) => Array.isArray(c.CollisionShapesSrc) && c.CollisionShapesSrc.some((s) => s.m_GrpName === group));
  must(coll, 'w_so192_Wep: no shape group ' + group);
  const s = coll.CollisionShapesSrc.filter((x) => x.m_GrpName === group);
  must(s.length === 1, group + ': expected one shape');
  const sh = s[0];
  must(String(sh.m_ShapeType).endsWith('SPHERE'), group + ': not a sphere');
  add('ultima.range.radius', sh.m_Size.x, 'cm', 'w_so192_Wep TresAtkColl0 ' + group + ' m_Size.x (SPHERE)', 'the sphere\'s radius; m_IncSize ' + sh.m_IncSize.x + ', no curve', '-');
  add('ultima.range.socket', sh.m_SocketName1, '-', 'w_so192_Wep TresAtkColl0 ' + group + ' m_SocketName1 / m_AttachType1 ' + String(sh.m_AttachType1).split('::').pop(), 'OWNER_ATTACHED_MESH = Sora\'s mesh: socket kh_root on bone kh_sk (p_ex001_0_Skeleton, no offset); M_9481 has root motion, so kh_sk keeps its reference pose (0, 0, 0): the centre is his root = the port\'s actor location', '-');
  add('ultima.range.offset', [sh.m_RelativeLocation1.x, sh.m_RelativeLocation1.y, sh.m_RelativeLocation1.z], 'cm', 'w_so192_Wep TresAtkColl0 ' + group + ' m_RelativeLocation1', 'added to the socket', '-');
  add('ultima.range.map_hit', String(sh.m_MapHitType).split('::').pop(), '-', 'w_so192_Wep TresAtkColl0 ' + group + ' m_MapHitType', 'no map hits', '-');

  // the row
  const a = ATTACK.get(rowName);
  must(a, 'p_ex001_AttackData has no ' + rowName);
  add('ultima.row.power', a.m_Power, '-', 'p_ex001_AttackData ' + rowName + ' m_Power', String(a.m_DamageKind).split('::').pop() + ', ' + String(a.m_DamageAttribute).split('::').pop() + (a.m_bIsMagicAttack ? ', magic' : ''), '-');
  return rows;
};
