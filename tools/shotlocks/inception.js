// (SHOT-C) Family `inception`: KH3's kind 42 (0x2A) SF_LIGHTINCEPTION "Stellar Inception", the MAX shotlock of Oathkeeper
// (weapon type 13) and Oblivion (14) (SelectKind 0x141411B80: 13, 14 -> 42). Rows `inception.*` of
// sheets/port/sora-shotlocks.json for kh-core magic/shotlocks/inception.rs.
//
// THE CODE (re/decomp, decompiled C; constants re-read in the disassembly build/sora_tmp/weapons/SHOT-C/inception/
// dis_1413C0850.txt; the delegate from the ShootFlow vtable 0x14693CA58 read in base/KH3.exe):
//   Begin 0x1413A5CD0 case 0x2A: update = thunk FUN_1404EFB1C -> vt+0x508 = 0x1413B7F10, name SF_LIGHTINCEPTION, group 0.
//   Update 0x1413B7F10:
//     sub 0:   +0xE4 = the actor location; gravity off (vt+0x1728(0)); Flying, stopped (vt+0x17A0(5, 1)); the weapon change
//              vt+0x13C8(weapon type, 0x18, 0, ...) = ChangeWeapon 0x14144A9F0 to weapon form 0x18 = TRES_WEAPON_FORM_18
//              (Double Form: Oathkeeper w_so200 right, Oblivion w_so210 left, AnimSet23); Play "M_N481" (blend -1, mode 2,
//              group 0), voice 0; face request (+0x84 |= 0x10); sub 4.
//     sub 3/4: flagsCC bit 2 (OnTiming 0x1413B57D0, kind 0x2A) -> sub 5; sub 3..5: at the clip's end Play "ShootFlowEnd",
//              m_bEnableChange = 0, sub 2, WorldSlow off (FUN_14140E500(ctrl, 0); (SHOT-FIX) not the release: End's slot
//              809 releases the aim mode).
//     sub 1/2: m_bEnableChange and +0xB0 != 0: vt+0x13C8(weapon type, +0xB0, +0xB1) (Begin's weapon form back), +0xB0 = 0;
//              at the clip's end pawn vt+0x1E48(1.0) (the pawn+0xE44 invincibility timer) and Finish (vt+0x270).
//     every tick: flagsCC bit 2: spawn counter +0x90 < N (+0x94 = total locks) -> the spawner vt+0x510 = 0x1413C0850(dt),
//              else bit 2 off; then vt+0x390(0, 0) (SetFace off).
//   TresAtk begin (anim interface slot 64, 0x1413B5430): kind 0x2A: flagsCC |= 1, +0xA0 = 0.3 (0x3E99999A).
//   Spawner 0x1413C0850(dt): +0xA0 > 0: -= dt, return while > 0 (the first bolt waits 0.3 s from the window's begin, once:
//              +0xA0 is not set again); the entry locked[+0x110] (its component alive: else +0x110 += 1, +0x114 = 0, no bolt);
//              rotator = the actor's, yaw += 360 / N x +0x90; Quat(that) x Quat(pitch 30) -> direction; the spawn point =
//              pawn vt+0x6E0 (his lock-on point) + 300 x Vector(0, yaw' - 90, 0) (300 cm to the left of the bolt's own
//              heading); vt+0x1450(+0xB8, point, point, direction, team, bFireNow 1); SetCommand(cmd, kind); homing on that
//              entry's component; +0x114 += 1, +0x90 += 1; +0x114 >= the entry's lock count (+0xC) -> +0x110 += 1, +0x114 = 0.
//              ONE bolt per call = one per tick; each locked target gets as many bolts as its locks.
// THE DATA: M_N481's notifies (build/sora_pack/keyblades/sora_forms_manifest.json: clip N481 of p_ex001_AnimSet23), the
// window's group R_Finish of Player/p_ex001/w_so200_Wep TresAtkColl0, p_ex001_AttackData, sora-keyblade-forms form.TF.
'use strict';

module.exports = (lib) => {
  const { row, clip, prop, blueprint, ATTACK, must, readJson, P } = lib;
  const rows = [];
  const add = (...a) => rows.push(row(...a));
  const UPD = '0x1413B7F10', SPAWN = '0x1413C0850';

  add('inception.kind', 42, 'kind', 'TresShootFlowData row 42 (二刀流ショット, kh3_system.command.sf_dual)', 'the MAX kind SelectKind 0x141411B80 gives weapon types 13 (Oathkeeper) and 14 (Oblivion)', '0x141411B80');
  add('inception.ability', 'TEXT_SHOOTFLOW_KB12_01', '-', 'TresAbilityData A00363 (m_NameTextID kh3_system.command.sf_dual, help kh3_system.ability.detail_txt_1070)', 'the display ability both Keyblades list (m_ShootflowAbilitys); "Stellar Inception"', '-');

  // the weapon form: Double Form's (P3's form.TF row: TRES_WEAPON_FORM_18 = 24, units w_so200 + w_so210)
  const forms = readJson(P('sheets', 'port', 'sora-keyblade-forms.json'));
  const fr = forms.rows.map((r) => Object.fromEntries(forms.columns.map((c, i) => [c, r[i]]))).find((r) => r.id === 'form.TF');
  must(fr && fr.weapon_form_value === 0x18 && fr.units && fr.units.length === 2, 'sora-keyblade-forms form.TF: weapon form 0x18 with two units');
  add('inception.weapon_form', fr.weapon_form_value, 'int', UPD + ' sub 0: pawn vt+0x13C8(weapon type, 0x18, 0, ...)', 'ChangeWeapon 0x14144A9F0 to weapon form 0x18 = ' + fr.weapon_form + ' (Double Form: AnimSet23); restored at sub 2 / End with Begin\'s +0xB0 / +0xB1', UPD);
  add('inception.weapon_units', fr.units, '-', 'sheets/port/sora-keyblade-forms form.TF units (equip data of weapon form 0x18)', 'the parts drawn instead of the Keyblade while weapon form 0x18 is on (kh-er-render keyblade.rs draw_form_weapon_parts, form "18")', UPD);

  // the clips
  const main = clip('M_N481', 'p_ex001_AnimSet23');
  must(main, 'no clip M_N481 of p_ex001_AnimSet23');
  add('inception.clip', 'M_N481', '-', 'p_ex001_AnimSet23 M_N481 -> sequence ' + main.sequence, 'the attack clip (slot 0, blend -1, mode 2, group 0)', UPD);
  add('inception.clip_sequence', main.sequence, '-', 'p_ex001_AnimSet23 M_N481', 'the animation-table row the logical name plays (AnimSet23 is the weapon form\x27s replacement set)', UPD);
  add('inception.clip_length', main.length, 's', 'sequence ' + main.sequence + ' duration', 'sub 3..5 wait for its end', UPD);
  const timing = (main.notifies || []).filter((n) => n.class === 'TresAnimNotify_Timing');
  must(timing.length === 1, 'M_N481: expected one TresAnimNotify_Timing');
  add('inception.timing', timing[0].time, 's', 'M_N481 TresAnimNotify_Timing', 'OnTiming 0x1413B57D0 (kind 0x2A): flagsCC bit 2 = the spawner runs, sub 5', '0x1413B57D0');
  const end = clip('ShootFlowEnd');
  must(end, 'no clip ShootFlowEnd');
  add('inception.end_clip_length', end.length, 's', 'sequence ' + end.sequence + ' duration', 'sub 2 finishes at its end', UPD);
  const change = (end.notifies || []).find((n) => n.class === 'TresAnimNotify_EnableChange');
  must(change, 'ShootFlowEnd has no TresEnableChange');
  add('inception.end_change', change.time, 's', 'ShootFlowEnd TresAnimNotify_EnableChange', 'the weapon form comes back from here (sub 2)', UPD);
  add('inception.end_invincible', 1.0, 's', UPD + ' sub 2: pawn vt+0x1E48(1.0) (0x3F800000)', 'the pawn+0xE44 invincibility timer after the state (combat spec IsInvincible): not ported (the port has no such timer; see inception.md)', UPD, 'inferred');

  // the hit window of M_N481
  const w = (main.notifies || []).filter((n) => n.class === 'TresAnimNotifyState_Attack');
  must(w.length === 1, 'M_N481: expected one attack window');
  const n = w[0];
  const group = String(prop(n, 'CollisionGrpName')), rowName = String(prop(n, 'm_AttackDataIDName'));
  add('inception.window.from', n.time, 's', 'M_N481 TresAnimNotifyState_Attack', 'the window opens (the generic hit path: combat/hit.rs on_notifies); TresAtk begin 0x1413B5430: the spawner delay starts', '0x1405620F0');
  add('inception.window.to', n.time + (n.duration || 0), 's', 'M_N481 TresAnimNotifyState_Attack (time + duration)', 'the window closes', '0x1405620F0');
  add('inception.window.group', group, '-', 'M_N481 TresAnimNotifyState_Attack CollisionGrpName', 'the shape group of weapon form 0x18\'s right weapon w_so200 (R_* groups are on one weapon only)', '-');
  add('inception.window.row', rowName, '-', 'M_N481 TresAnimNotifyState_Attack m_AttackDataIDName', 'the attack row (sora-combat-attack-data carries it)', '-');
  add('inception.window.interval', prop(n, 'AttackInterval') || 0, 's', 'M_N481 TresAnimNotifyState_Attack AttackInterval (absent: 0)', '0: one hit per target per window', '-');
  add('inception.window.hit_se', String(prop(n, 'm_WeaponHitSEAliasID')).split('::').pop(), '-', 'M_N481 m_WeaponHitSEAliasID', 'the weapon hit SE alias of the window', '-');
  add('inception.delay', 0.3, 's', '0x1413B5430 (TresAtk begin, anim interface slot 64): kind 0x2A: +0xA0 = 0x3E99999A', 'the spawner waits this long (counted only while it runs, i.e. from the Timing notify)', '0x1413B5430');

  // the R_Finish shape of w_so200_Wep TresAtkColl0
  const bp = blueprint('/Game/Blueprints/Player/p_ex001/w_so200_Wep');
  const coll = Object.values(bp.comps).find((c) => Array.isArray(c.CollisionShapesSrc) && c.CollisionShapesSrc.some((s) => s.m_GrpName === group));
  must(coll, 'w_so200_Wep: no shape group ' + group);
  const s = coll.CollisionShapesSrc.filter((x) => x.m_GrpName === group);
  must(s.length === 1, group + ': expected one shape');
  const sh = s[0];
  must(String(sh.m_ShapeType).endsWith('SPHERE'), group + ': not a sphere');
  add('inception.finish.radius', sh.m_Size.x, 'cm', 'w_so200_Wep TresAtkColl0 ' + group + ' m_Size.x (SPHERE)', 'the sphere\'s radius at the window\'s begin', '0x140649E40');
  add('inception.finish.inc', (sh.m_IncSize || {}).x || 0, 'cm/s', 'w_so200_Wep TresAtkColl0 ' + group + ' m_IncSize.x', 'growth per second of window time (0x140649E40)', '0x140649E40');
  add('inception.finish.inc_max', (sh.m_IncMaxSize || {}).x || 0, 'cm', 'w_so200_Wep TresAtkColl0 ' + group + ' m_IncMaxSize.x', 'the growth stops here', '0x140649E40');
  add('inception.finish.socket', String(sh.m_SocketName1), '-', 'w_so200_Wep TresAtkColl0 ' + group + ' m_SocketName1 / m_AttachType1 ' + String(sh.m_AttachType1).split('::').pop(), 'OWNER_ATTACHED_MESH = Sora\'s mesh: socket kh_root (his root = the port\'s actor location, as F-LIGHTDARK places it)', '-');
  add('inception.finish.offset', [sh.m_RelativeLocation1.x, sh.m_RelativeLocation1.y, sh.m_RelativeLocation1.z], 'cm', 'w_so200_Wep TresAtkColl0 ' + group + ' m_RelativeLocation1', 'added to the socket (rotation ' + String(sh.m_RotAttachType).split('::').pop() + ')', '-');
  add('inception.finish.map_hit', String(sh.m_MapHitType).split('::').pop(), '-', 'w_so200_Wep TresAtkColl0 ' + group + ' m_MapHitType', 'map hits of the shape', '-');

  // the spawner's constants (disassembly of 0x1413C0850)
  add('inception.spawn.yaw_turn', 360, 'deg', SPAWN + ': movss xmm2,[145C9C550h] (360.0) / N x +0x90 added to the actor yaw', 'bolt i heads at yaw + 360 / N x i', SPAWN);
  add('inception.spawn.pitch', 30, 'deg', SPAWN + ': movss xmm0,[145C87D34h] (30.0) = the pitch of the second quaternion', 'every bolt leaves 30 degrees up', SPAWN);
  add('inception.spawn.side_yaw', -90, 'deg', SPAWN + ': addss xmm0,[145C9C5A4h] (-90.0) to the bolt\'s yaw, pitch 0', 'the spawn point lies to the left of the bolt\'s heading', SPAWN);
  add('inception.spawn.side', 300, 'cm', SPAWN + ': movss xmm0,[145C9C54Ch] (300.0) x that direction, added to pawn vt+0x6E0', 'distance of the spawn point from his lock-on point', SPAWN);
  add('inception.projectile', 'SF_LIGHTINCEPTION', '-', 'Begin 0x1413A5CD0 case 0x2A (+0xB8); w_so200_ProjSet / w_so210_ProjSet SF_LIGHTINCEPTION -> TresProjTF_LightInception_C', 'the bolt (proj.tf_lightinception.*: row SF_LIGHTINCEPTION_BOMB)', SPAWN);

  // the rows
  for (const id of ['SF_LIGHTINCEPTION', 'SF_LIGHTINCEPTION_BOMB', 'SF_LIGHTINCEPTION_END']) {
    const a = ATTACK.get(id);
    must(a, 'p_ex001_AttackData has no ' + id);
    const used = id === 'SF_LIGHTINCEPTION' ? 'the R_Finish window' : id === 'SF_LIGHTINCEPTION_BOMB' ? 'the bolts (TresProjTF_LightInception shape 0)' : 'NO user found: not in M_N481, the projectile sets, the ShootFlow / player blueprints or the p_ex001 clips of the pack';
    add('inception.row.' + id.toLowerCase() + '.power', a.m_Power, '-', 'p_ex001_AttackData ' + id + ' m_Power', String(a.m_DamageKind).split('::').pop() + ', ' + String(a.m_DamageAttribute).split('::').pop() + (a.m_bIsMagicAttack ? ', magic' : '') + ': ' + used, '-');
  }

  // what only a renderer / host plays (the generic notify path sends the effects, sounds and the voice)
  const cls = new Set();
  for (const x of main.notifies || []) if (/Bonamik/.test(x.class)) cls.add(x.class);
  add('inception.renderer_notifies', [...cls].sort(), '-', 'M_N481 notifies', 'Bonamik wind / group parameters over the whole clip (hood, sleeves: the renderer\'s cloth), not the core\'s', '-');
  return rows;
};
