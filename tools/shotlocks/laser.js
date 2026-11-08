// (SHOT-A laser) Family `laser`: KH3's update 0x1413B9600 (TresPlayerState_ShootFlow vtable 0x14693CA58 +0x4D0) of kind 11
// "ラプ反射" (kh3_system.command.sf_laser_0) "Spectral Rays", Ever After's MAX kind (WEP_KEYBLADE_SO_04: base kind 10
// Shimmering Drops, Param0 28). Rows `laser.*` of sheets/port/sora-shotlocks.json for kh-core magic/shotlocks/laser.rs.
//
// THE CODE (decompiled C re/decomp/1413B/1413B9600.c, its disassembly build/sora_tmp/weapons/SHOT-A/laser/dis_1413B9600.txt;
// the spawner re/decomp/1413C/1413C1A90.c + dis_1413C1A90.txt; both listings made with SHOT-B's spec/scripts/dis.js):
//   sub 0: gravity off (pawn vt+0x1728(0)), Flying stopped (vt+0x17A0(5, 1)); ChangeWeapon (vt+0x13C8 0x14144A9F0)
//          (weapon type, form 0x0A = Mirage Staff, 0, ...) at 0x1413B97F2; Play "M_A481" (slot 0, blend -1, mode 2, group
//          0, rate 1: [rsp+28h] = 0 at 0x1413B9852), voice 0 (vt+0x1F0(0)) when the length > 0; +0x84 |= 0x10 (m_bFaceAt);
//          sub 4; LiftCheck vt+0x398(200.0) -> sub 3.
//   sub 3: LiftStep vt+0x3A0 -> sub 4 (no fall-through into sub 4's code that tick).
//   sub 4 / 5: at the clip's end: Play "ShootFlowEnd" (group 0, voice 0), +0x84 &= ~1 (m_bEnableChange off), sub 2.
//   other (sub 2): m_bEnableChange (ShootFlowEnd's TresEnableChange) and +0xB0 != 0: pawn vt+0x1408, ChangeWeapon(type,
//          +0xB0, +0xB1, ...) (Begin's weapon back), FUN_14140E500(ctrl, 0) (WorldSlow off: ctrl+0x878 bit 3, not the
//          release handshake, which is the common End's slot 809), +0xB0 = 0; at the
//          clip's end Finish (vt+0x270); the tail below still runs that tick.
//   every tick: flagsCC bit 0 (the TresAtk begin handler slot 64 0x1413B5430 sets it and +0xA0 = 0.5 for kind 0xB): while
//          +0x90 < +0x94 the spawner vt+0x4D8 = 0x1413C1A90(dt), else the bit off; then SetFace(false, 0) (vt+0x390).
//   spawner 0x1413C1A90: +0xA0 > 0: -= dt, still > 0: return (else 0). The locked entry +0x110 (ctrl+0xB08, 0x34 bytes,
//          count ctrl+0xB10): gone / invalid / pending kill: +0x110 += 1, +0x114 = 0. Weapon out (pawn vt+0x1330
//          0x1405ADE80) else return. p = pawn vt+0x1450("SF_REFLECTIVELASER", T, T, dir, team, fire 1) with T = the entry's
//          component location (vt+0xB88) and dir = T - pawn vt+0x6E0 (normalised when |.|^2 > 1e-8); none / pending kill:
//          return. p.vt+0x1100(+0x88, +0x8C); +0x114 += 1, +0x90 += 1; +0x114 >= entry lock count (+0xC): +0x114 = 0,
//          +0x110 += 1; +0xA0 = 0.1 (0x1413C1E00 mov [rbx+0A0h],3DCCCCCDh).
// THE DATA: M_A481 of p_ex001_AnimSet10 (build/sora_pack/keyblades/sora_forms_manifest.json), the Mirage Staff form row of
// sheets/port/sora-keyblade-forms.json (form.WA: weapon form 10, its unit w_so072_Wep, its projectile set w_so070_ProjSet),
// w_so072_Wep TresAtkColl0 (SF_Range, the ReflecLaser hit effects), p_ex001_AttackData SF_REFLECTIVELASER(_RANGE).
'use strict';

module.exports = (lib) => {
  const { row, clip, prop, blueprint, projectileSet, ATTACK, decomp, must, readJson, P } = lib;
  const rows = [];
  const add = (...a) => rows.push(row(...a));
  const UPD = '0x1413B9600';
  const SPAWN = '0x1413C1A90';
  const DIS = 'build/sora_tmp/weapons/SHOT-A/laser/dis_1413B9600.txt';
  const DIS_SPAWN = 'build/sora_tmp/weapons/SHOT-A/laser/dis_1413C1A90.txt';
  const upd = decomp(UPD);
  const spawn = decomp(SPAWN);
  must(upd && spawn, 'no decompiled 0x1413B9600 / 0x1413C1A90 in re/decomp');
  must(/L"M_A481"/.test(upd) && /L"ShootFlowEnd"/.test(upd) && /\+ 0x4d8\)/.test(upd) && /0x43480000/.test(upd), UPD + ': the update changed');
  must(/L"SF_REFLECTIVELASER"/.test(spawn) && /0x3dcccccd/.test(spawn), SPAWN + ': the spawner changed');

  add('laser.update', UPD, '-', 'vtable 0x14693CA58 + 0x4D0 (Begin 0x1413A5CD0 case 0xB, kind.k11.update_slot 1232)', 'the update of kind 11 (Spectral Rays); no other kind of TresShootFlowData uses it', UPD);
  add('laser.kinds', [11], 'kind', 'sora-shotlocks kind.k<N>.update == 0x1413B9600', 'the kinds this update runs (11 SHOT-A Spectral Rays)', UPD);
  add('laser.spawner', SPAWN, '-', 'ShootFlow vtable 0x14693CA58 + 0x4D8 (called by ' + UPD + ' at 0x1413B98D8)', 'one SF_REFLECTIVELASER per call at most', SPAWN);

  // ---------------------------------------------------------------------------------------------- the weapon form
  const forms = readJson(P('sheets', 'port', 'sora-keyblade-forms.json'));
  const fr = forms.rows.map((r) => Object.fromEntries(forms.columns.map((c, i) => [c, r[i]]))).find((r) => r.weapon_form_value === 10 && r.weapon_type > 0);
  must(fr && fr.id === 'form.WA' && fr.units && fr.units.length, 'sora-keyblade-forms: no form.WA (weapon form 10)');
  add('laser.weapon_form', 10, 'int', UPD + ' sub 0: pawn vt+0x13C8(weapon type, 10, 0, ...) at 0x1413B97F2 mov r8b,0Ah', 'ChangeWeapon 0x14144A9F0 to weapon form 10 = ' + fr.name + ' (' + fr.id + ', ' + fr.weapon_form + '): its weapon and anim set ' + fr.anim_sets.map(lib.setName).join(' + ') + ' (M_A481); Begin\'s +0xB0 / +0xB1 come back in sub 2 / End', UPD);
  add('laser.weapon_units', fr.units, '-', 'sheets/port/sora-keyblade-forms ' + fr.id + ' units (grow LVL01: the third argument 0)', 'the parts the hosts draw instead of the Keyblade while PawnMagic::shot_weapon_form is 10', UPD);
  const formSet = lib.setName(fr.projectile_set).replace(/_ProjSet$/, '');
  add('laser.proj_set', formSet, '-', 'sheets/port/sora-keyblade-forms ' + fr.id + ' projectile_set', 'the projectile set of weapon form 10\'s equip entry (Ever After\'s own set too). The port resolves SF_REFLECTIVELASER by the held Keyblade\'s set first (magic/shotlocks projectile_named, KH3\'s pawn vt+0x1450 -> equipment) and by this set when that one has no such name: the Customize screen\'s Spectral Rays (XSI\'s port addition) with another Keyblade', '0x1405BDBF0', 'inferred');

  // ---------------------------------------------------------------------------------------------- the clips
  const main = clip('M_A481', 'p_ex001_AnimSet10');
  must(main, 'no clip M_A481 of p_ex001_AnimSet10');
  add('laser.clip', 'M_A481', '-', 'FName literal 0x146933D70 (' + UPD + ' 0x1413B980C); p_ex001_AnimSet10 M_A481 -> sequence ' + main.sequence, 'the attack clip (slot 0, blend -1, mode 2, group 0, rate 1): AnimSet10 is weapon form 10\'s anim set', UPD);
  add('laser.clip_sequence', main.sequence, '-', 'p_ex001_AnimSet10 M_A481 -> its sequence', 'the anim table row (AnimId::A481)', UPD);
  add('laser.clip_length', main.length, 's', 'sequence ' + main.sequence + ' duration', 'sub 4 waits for its end', UPD);
  const end = clip('ShootFlowEnd', 'p_ex001_AnimSet10');
  must(end && end.sequence === clip('ShootFlowEnd').sequence, 'ShootFlowEnd: AnimSet10 replaces it');
  add('laser.end_clip_length', end.length, 's', 'FName literal 0x146933D80 (0x1413B9728); sequence ' + end.sequence + ' duration (AnimSet10 has the base set\'s own)', 'sub 2 finishes at its end (Play with group 0)', UPD);
  const change = (end.notifies || []).find((n) => n.class === 'TresAnimNotify_EnableChange');
  must(change, 'ShootFlowEnd has no TresEnableChange');
  add('laser.end_change', change.time, 's', 'ShootFlowEnd TresAnimNotify_EnableChange', 'from here the weapon form of Begin comes back (sub 2)', UPD);
  add('laser.lift.height', 200, 'cm', UPD + ' 0x1413B9882 movss xmm1,[145C9C544h] = f32 200 -> LiftCheck vt+0x398 (0x1413AD550) (' + DIS + ')', 'sub 0: lifted by this (less the ground distance) when the probe allows, then sub 3\'s LiftStep (states/shoot_flow.rs lift_check / lift_step)', UPD);

  // ---------------------------------------------------------------------------------------------- M_A481's window
  const w = (main.notifies || []).filter((n) => n.class === 'TresAnimNotifyState_Attack');
  must(w.length === 1, 'M_A481: expected one attack window');
  const n = w[0];
  const group = String(prop(n, 'CollisionGrpName')), rowName = String(prop(n, 'm_AttackDataIDName'));
  add('laser.window.from', n.time, 's', 'M_A481 TresAnimNotifyState_Attack', 'the window opens (combat/hit.rs on_notifies); its begin (slot 64 0x1413B5430) also sets flagsCC bit 0 and +0xA0 = 0.5 for kind 0xB: the spawner starts', '0x1413B5430');
  add('laser.window.to', n.time + (n.duration || 0), 's', 'M_A481 TresAnimNotifyState_Attack (time + duration)', 'the window closes', '0x1405620F0');
  add('laser.window.group', group, '-', 'M_A481 TresAnimNotifyState_Attack CollisionGrpName', 'the shape group of weapon form 10\'s weapon w_so072_Wep', '-');
  add('laser.window.row', rowName, '-', 'M_A481 TresAnimNotifyState_Attack m_AttackDataIDName', 'the attack row (sora-combat-attack-data carries it)', '-');
  add('laser.window.interval', prop(n, 'AttackInterval') || 0, 's', 'M_A481 TresAnimNotifyState_Attack AttackInterval', 'the hit list clears every interval: one hit per target per interval', '-');
  const hitGroup = String(prop(n, 'm_HitEffectGrpName'));
  add('laser.window.hit_effect_group', hitGroup, '-', 'M_A481 TresAnimNotifyState_Attack m_bOverrideHitEffect ' + prop(n, 'm_bOverrideHitEffect') + ', m_HitEffectGrpName', 'the chooser 0x140628960 takes w_so072_Wep\'s m_HitEffects entry of this group (laser.range.hit_effect)', '0x140628960');
  add('laser.window.hit_se', String(prop(n, 'm_WeaponHitSEAliasID')).split('::').pop(), '-', 'M_A481 m_WeaponHitSEAliasID', 'the weapon hit SE alias of the window', '-');

  // the SF_Range shape and the ReflecLaser hit effect of w_so072_Wep (the unit of form.WA)
  const unitBp = fr.units[0].split('=')[1].split('.')[0];
  must(/w_so072_Wep$/.test(unitBp), fr.id + ': the unit is not w_so072_Wep');
  const bp = blueprint(unitBp);
  const coll = Object.values(bp.comps).find((c) => Array.isArray(c.CollisionShapesSrc) && c.CollisionShapesSrc.some((s) => s.m_GrpName === group));
  must(coll, 'w_so072_Wep: no shape group ' + group);
  const s = coll.CollisionShapesSrc.filter((x) => x.m_GrpName === group);
  must(s.length === 1, group + ': expected one shape');
  const sh = s[0];
  const src = 'w_so072_Wep TresAtkColl0 ' + group;
  add('laser.range.type', String(sh.m_ShapeType).split('::').pop(), '-', src + ' m_ShapeType', 'a sphere', '-');
  add('laser.range.radius', sh.m_Size.x, 'cm', src + ' m_Size.x', 'the sphere\'s radius; m_IncSize ' + sh.m_IncSize.x + ', no curve', '-');
  add('laser.range.attach', String(sh.m_AttachType1).split('::').pop(), '-', src + ' m_AttachType1', 'OWNER_ATTACHED_MESH = Sora\'s mesh socket', '-');
  add('laser.range.socket', sh.m_SocketName1, '-', src + ' m_SocketName1', 'kh_root on bone kh_sk (p_ex001_0_Skeleton, no offset); A481 has no root motion and its kh_sk translation track is (0, 0, 0) over the whole clip: the centre is his root = the port\'s actor location (as SHOT-C\'s ultima)', '-');
  add('laser.range.offset', [sh.m_RelativeLocation1.x, sh.m_RelativeLocation1.y, sh.m_RelativeLocation1.z], 'cm', src + ' m_RelativeLocation1 (' + String(sh.m_RotAttachType).split('::').pop() + ')', 'added to the socket, turned by his yaw', '-');
  add('laser.range.map_hit', String(sh.m_MapHitType).split('::').pop(), '-', src + ' m_MapHitType', 'no map hits', '-');
  const he = (coll.m_HitEffects || []).find((e) => e.m_GrpName === hitGroup);
  must(he && he.m_PawnHitEffect, 'w_so072_Wep: no m_HitEffects group ' + hitGroup);
  add('laser.range.hit_effect', he.m_PawnHitEffect, '-', 'w_so072_Wep TresAtkColl0 m_HitEffects[' + hitGroup + '].m_PawnHitEffect', 'the pawn hit effect of the window\'s hits (combat/hit.rs: m_bOverrideHitEffect + m_HitEffectGrpName)', '0x140628960');

  // ---------------------------------------------------------------------------------------------- the spawner
  const set = projectileSet(formSet);
  const cls = lib.setName(set.get('SF_REFLECTIVELASER') || '');
  must(cls === 'TresProjRA_ReflectiveLaser_C', formSet + ': SF_REFLECTIVELASER -> ' + cls);
  add('laser.proj_name', 'SF_REFLECTIVELASER', '-', SPAWN + ' 0x1413C1C13 FName literal 0x146933DA0 (' + DIS_SPAWN + ')', 'the projectile pawn vt+0x1450 spawns (the weapon\'s projectile set resolves it: ' + formSet + ' -> ' + cls + ')', SPAWN);
  add('laser.proj_class', cls, '-', formSet + '_ProjSet SF_REFLECTIVELASER', 'speed 0, life 1 s, a 30 cm sphere of row SF_REFLECTIVELASER (proj.ra_reflectivelaser.*), no homing', SPAWN);
  add('laser.spawn.fire', true, '-', SPAWN + ' 0x1413C1D2C mov byte ptr [rsp+30h],1', 'vt+0x1450 bFireNow = 1 (no homing target is set: only vt+0x1100(cmd, kind))', SPAWN);
  add('laser.spawn.interval', 0.1, 's', SPAWN + ' 0x1413C1E00 mov dword ptr [rbx+0A0h],3DCCCCCDh = f32 0.1', '+0xA0 after each spawn: the next one waits this long', SPAWN);
  add('laser.spawn.delay', 0.5, 's', '0x1413B5430 (anim interface slot 64, TresAtk begin): kind 0xB +0xA0 = 0.5', 'the first spawn waits this long after M_A481\'s window opens (shotlocks/mod.rs on_notifies)', '0x1413B5430');

  // ---------------------------------------------------------------------------------------------- the rows
  for (const name of ['SF_REFLECTIVELASER', rowName]) {
    const a = ATTACK.get(name);
    must(a, 'p_ex001_AttackData has no ' + name);
    add(`laser.row.${name.toLowerCase()}.power`, a.m_Power, '-', 'p_ex001_AttackData ' + name + ' m_Power', String(a.m_DamageKind).split('::').pop() + ', ' + String(a.m_DamageAttribute).split('::').pop() + (a.m_bIsMagicAttack ? ', magic' : ''), '-');
  }
  return rows;
};
