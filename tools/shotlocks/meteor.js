// (SHOT-A meteor) Family `meteor`: TresPlayerState_ShootFlow's update 0x1413BA090 (vtable 0x14693CA58 + 0x3D0) and its
// stream spawner 0x1413C24D0 (vt+0x438). Kinds: 5 SF_SHOOTINGSTAR "Meteor Shower" (Shooting Star, weapon type 3; Begin
// 0x1413A5CD0's default branch), SHOT-B's 18 BX_SHOOT / 23 RE_SHOOT and SHOT-C's 40 LF_SHOOT (the same update). Rows
// `meteor.*` of sheets/port/sora-shotlocks.json for kh-core magic/shotlocks/meteor.rs.
//
// THE CODE (instructions: SHOT-B's listings build/sora_tmp/weapons/SHOT-B/spec/dis_1413BA090.txt and
// spec/scripts/shootingstar/dis_1413C24D0.txt; decompiled C re/decomp/1413B/1413BA090.c). Every constant below is checked
// against its instruction in those listings (`ins`).
//   sub 0: gravity off (vt+0x1728(0)), Flying stopped (vt+0x17A0(5, 1)), vt+0x1388(1, 0, 0); Play "M_1475" (blend -1,
//          mode 2, group +0xC0, rate 1), voice 0; +0x84 |= 0x10 (face request); sub 1; LiftCheck(100) -> sub 3.
//   sub 3: LiftStep -> sub 4; falls into sub 1's code.
//   sub 1 (and 3): at the clip's end Play "ShootFlowEnd" (group 0 LITERAL), voice 0; sub 2.
//   any other sub (2, and 4 = after the lift): at the clip's end vt+0x1388(0, 0, 0), kind 0x28: pawn vt+0x1E48(1.0),
//          Finish (vt+0x270). (Sub 4 jumps to this branch: lifted, M_1475's end finishes without ShootFlowEnd.)
//   every tick (after Finish too): flagsCC bit 0 (the Shot handler 0x1413B4A70 on M_1475's TresShot) -> spawner while
//          +0x90 < N, else the bit off; then SetFace(false, 0) (vt+0x390).
//   spawner: +0xA0 counts down (spawns on the tick it reaches 0); target = locked[+0x90 mod lockedTargets] (none
//          locked: +0xD0); a dead target: no spawn, no count; origin = the right-hand weapon actor's root (ATK_01 the
//          fallback), direction = normalize(origin - pawn vt+0x6E0 = his lock-on point); one projectile +0xB8 homing on
//          the target, fired; +0x90 += 1; +0xA0 = 0.066666 (N < 8) / 0.033333 (N < 15) / stays 0.
'use strict';
const fs = require('fs');

module.exports = (lib) => {
  const { row, clip, blueprint, projectileSet, ATTACK, decomp, must, P, MAIN } = lib;
  const rows = [];
  const add = (...a) => rows.push(row(...a));
  const UPD = '0x1413BA090', SPAWN = '0x1413C24D0', BEGIN = '0x1413A5CD0';
  const LIST = {
    [UPD]: P('build', 'sora_tmp', 'weapons', 'SHOT-B', 'spec', 'dis_1413BA090.txt'),
    [SPAWN]: P('build', 'sora_tmp', 'weapons', 'SHOT-B', 'spec', 'scripts', 'shootingstar', 'dis_1413C24D0.txt'),
  };
  const text = {};
  for (const [a, f] of Object.entries(LIST)) { must(fs.existsSync(f), 'no listing ' + f); text[a] = fs.readFileSync(f, 'utf8'); }
  // The instruction at `at` of function `fn` contains `needle`; returns the instruction's address for ref_addr.
  const ins = (fn, at, needle) => {
    const line = text[fn].split(/\r?\n/).find((l) => l.startsWith(at.toLowerCase() + ' '));
    must(line && line.toLowerCase().includes(needle.toLowerCase()), `${fn} ${at}: expected "${needle}", got "${line}"`);
    return fn; // ref_addr: the function (preflight knows functions); the instruction is in the source text
  };
  const f32 = (hex) => { const b = Buffer.alloc(4); b.writeUInt32BE(parseInt(hex, 16)); return b.readFloatBE(0); };

  // ---- the kinds that run this update (Begin's switch: the projectile name of each; the kind table has the rest)
  const begin = decomp(BEGIN);
  must(begin, 'no decompiled Begin ' + BEGIN);
  const KINDS = [
    [5, 'SF_SHOOTINGSTAR', 'w_so020', 'WEP_KEYBLADE_SO_02 Shooting Star'],
    [18, 'BX_SHOOT', 'w_so170', 'WEP_KEYBLADE_SO_08 Nano Gear (SHOT-B)'],
    [23, 'RE_SHOOT', 'w_so140', 'WEP_KEYBLADE_SO_10 Grand Chef (SHOT-B)'],
    [40, 'LF_SHOOT', 'w_so200', 'WEP_KEYBLADE_SO_12 Oathkeeper (SHOT-C)'],
  ];
  add('meteor.update', UPD, '-', 'ShootFlow vtable 0x14693CA58 + 0x3D0 (Begin ' + BEGIN + ' stores it as the delegate +0x170)', 'the update of kinds ' + KINDS.map((k) => k[0]).join(', '), BEGIN);
  add('meteor.kinds', KINDS.map((k) => k[0]), 'kind', 'Begin ' + BEGIN + ': the cases whose delegate is vt+0x3D0 (5 = the default branch)', 'the module runs every one from its kind data (+0xB8 name, +0xC0 group)', BEGIN);
  for (const [k, name, w, key] of KINDS) {
    must(begin.includes('"' + name + '"'), 'Begin has no FName ' + name);
    const set = projectileSet(w);
    const cls = set.get(name);
    must(cls, w + '_ProjSet has no ' + name);
    const bp = blueprint('/Game/Blueprints/Projectile/ShootFlow/' + cls.replace(/_C$/, ''));
    must(bp.parent === 'TresProj_Ragnarok', cls + ': parent ' + bp.parent);
    const shapes = (bp.comps.TresAtkColl0 || {}).CollisionShapesSrc || [];
    must(shapes.length === 1, cls + ': expected one attack shape');
    const atk = String(shapes[0].m_DefaultAttackDataIDName);
    const a = ATTACK.get(atk);
    must(a, 'p_ex001_AttackData has no ' + atk);
    const g = 'meteor.k' + k;
    add(g + '.proj_class', cls, '-', 'Blueprints/Player/p_ex001/' + w + '_ProjSet : ' + name, key + '; native parent TresProj_Ragnarok (the helix, magic/projectile.rs)', BEGIN);
    add(g + '.attack_row', atk, '-', 'Blueprints/Projectile/ShootFlow/' + cls.replace(/_C$/, '') + ' : TresAtkColl0.CollisionShapesSrc[0].m_DefaultAttackDataIDName', 'p_ex001_AttackData ' + atk + ': power ' + a.m_Power + ', ' + String(a.m_DamageKind).split('::').pop(), '-');
    add(g + '.damage_kind', String(a.m_DamageKind).split('::').pop(), '-', 'p_ex001_AttackData ' + atk + ' m_DamageKind', 'the KH3 reaction of every bolt hit (SIDESPIN / TOSS launches)', '-');
    add(g + '.particle', String((bp.comps.ParticleComp || {}).Template || ''), '-', 'Blueprints/Projectile/ShootFlow/' + cls.replace(/_C$/, '') + ' : ParticleComp.Template', 'the bolt\'s body effect', '-');
  }

  // ---- the clips
  const main = clip('M_1475');
  must(main, 'no clip M_1475');
  add('meteor.clip', 'M_1475', '-', 'base set M_1475 -> sequence ' + main.sequence, 'sub 0: Play (slot 0, blend -1, mode 2, group +0xC0, rate 1)', ins(UPD, '1413ba228', 'M_1475'));
  add('meteor.clip_length', main.length, 's', 'sequence ' + main.sequence + ' duration', 'sub 1 / 4 wait for its end', UPD);
  const shot = main.notifies.filter((n) => n.class === 'TresAnimNotify_Shot');
  must(shot.length === 1, 'M_1475: expected one TresShot');
  add('meteor.shot_time', shot[0].time, 's', 'M_1475 TresAnimNotify_Shot', 'the Shot handler 0x1413B4A70 sets flagsCC bit 0 for kinds 5 / 0x12 / 0x17 / 0x28 (no spawn of its own): the stream starts', '0x1413B4A70');
  add('meteor.shot_location_index', shot[0].properties.LocationIndex, 'int', 'M_1475 TresAnimNotify_Shot LocationIndex', 'ignored by the handler for these kinds', '0x1413B4A70');
  const end = clip('ShootFlowEnd');
  must(end, 'no clip ShootFlowEnd');
  add('meteor.end_clip_length', end.length, 's', 'sequence ' + end.sequence + ' duration', 'played at M_1475\'s end from sub 1 / 3 with group 0 (a literal: xor r8d / [rsp+28h] = esi = 0)', ins(UPD, '1413ba1ab', 'mov [rsp+28h],esi'));
  const change = end.notifies.find((n) => n.class === 'TresAnimNotify_EnableChange');
  must(change, 'ShootFlowEnd has no TresEnableChange');
  add('meteor.end_change', change.time, 's', 'ShootFlowEnd TresAnimNotify_EnableChange', 'm_bEnableChange from here (the update never clears or reads it)', UPD);

  // ---- the update's constants
  add('meteor.flying_mode', 5, 'int', 'sub 0: pawn vt+0x17A0(5, 1)', 'movement mode 5 = Flying, the movement stopped', ins(UPD, '1413ba1ff', 'mov edx,5'));
  ins(UPD, '1413ba2a7', '145C87D40h');
  add('meteor.lift.height', 100, 'cm', 'sub 0: LiftCheck vt+0x398(xmm1 = [145C87D40h] = f32 100)', 'the lift target is 100 cm above the floor point (King of Hearts: 500); instruction 0x1413BA2A7', UPD);
  add('meteor.lift.sub_after', 4, 'int', 'sub 3: LiftStep true -> mov dword [rdi+78h],4', 'the switch (sub - 1 == 0 -> 0x1413BA141, == 2 -> 0x1413BA12A, else the Finish branch 0x1413BA0C8) sends sub 4 to the Finish branch: lifted, M_1475\'s end finishes the state without ShootFlowEnd', ins(UPD, '1413ba13a', 'mov dword ptr [rdi+78h],4'));
  add('meteor.finish_e44_kind', 0x28, 'kind', 'Finish branch: cmp dword [rdi+8Ch],28h -> pawn vt+0x1E48(1.0)', 'kind 40 only: pawn+0xE44 = 1.0 (0x141112510); no reader found in re/decomp (grep field_0xe44 / 0xe44): not ported', ins(UPD, '1413ba0fb', 'cmp dword ptr [rdi+8Ch],28h'), 'confirmed', 'SHOT-C kind');
  add('meteor.voice_group', 0, 'int', 'after each Play: anim interface vt+0x1F0(edx = 0)', 'the voice group number 0 (Ctx::set_voice_group_no)', ins(UPD, '1413ba299', 'xor edx,edx'));

  // ---- the spawner's constants
  const small = ins(SPAWN, '1413c2a0e', '3D88882Fh'), mid = ins(SPAWN, '1413c2a1f', '3D08882Fh');
  add('meteor.stream.small_below', 8, 'int', 'spawner: cmp eax ([rbx+94h] = N),8; jge', 'N < 8: the long interval', ins(SPAWN, '1413c2a09', 'cmp eax,8'));
  add('meteor.stream.small_interval', f32('3D88882F'), 's', 'spawner: mov dword [rbx+0A0h],3D88882Fh', '0.066666 = 4 ticks at 60 Hz (f32 0.066666 - 4 dt < 0)', small);
  add('meteor.stream.mid_below', 15, 'int', 'spawner: cmp eax,0Fh; jge', '8 <= N < 15: the short interval; N >= 15: +0xA0 stays 0 (one a tick)', ins(SPAWN, '1413c2a1a', 'cmp eax,0Fh'));
  add('meteor.stream.mid_interval', f32('3D08882F'), 's', 'spawner: mov dword [rbx+0A0h],3D08882Fh', '0.033333 = 2 ticks at 60 Hz', mid);
  ins(SPAWN, '1413c2822', '145C9C488h');
  add('meteor.stream.min_len_sq', '1e-8', 'cm2', 'spawner: comiss |d|^2,[145C9C488h] (f32 1e-8)', 'the direction is normalised only above this (text: the sheet rounds numbers to 1e-6); instruction 0x1413C2822', SPAWN);
  ins(SPAWN, '1413c26e3', 'mov dl,1');
  add('meteor.stream.hand', 1, 'int', 'spawner: pawn vt+0x1428(dl = 1, &array) (equipment vt+0x530: the weapon actors of ETresCharEquipPart 1)', 'the right hand\'s weapon actor; its root (RootComponent +0x188 ComponentToWorld +0x2B0) is the origin, ATK_01 (vt+0x1410(0)) the fallback; instruction 0x1413C26E3', SPAWN);
  const s = MAIN.attack_sockets || {};
  must(Array.isArray(s.atk_00_on_wep_cm) && s.atk_00_on_wep_cm.every((v) => v === 0), 'ATK_00 is not the weapon\'s origin');
  must((s.clips || {})['1475'], 'no socket track of 1475');
  add('meteor.stream.origin_socket', 'ATK_00', '-', 'build/sora_pack/manifest.json attack_sockets.atk_00_on_wep_cm = (0, 0, 0): ATK_00 is the weapon\'s origin on ' + s.attach_bone, 'the weapon actor\'s root = ATK_00 of the clip\'s socket track (the port\'s per-frame weapon data); the root read at 0x1413C273B', SPAWN, 'inferred');
  ins(SPAWN, '1413c27cc', 'call qword ptr [r9+6E0h]');
  add('meteor.stream.from', 'lock_point', '-', 'spawner: direction = origin - pawn vt+0x6E0 (0x1405BAC00: the MyLockOnSetBoneName point, else the root)', 'his lock-on point (magic/shotlock.rs lock_point); instruction 0x1413C27CC', SPAWN);
  ins(SPAWN, '1413c28e8', 'mov rdx,[rsi+14E0h]');
  add('meteor.stream.fire_effect', '', '-', 'spawner: FUN_14411B3D0(pawn, pawn+0x14E0 m_pFireEffect, ...)', 'Sora\'s blueprints p_ex001_Pawn / PawnBase set no m_pFireEffect (SHOT-B shootingstar.md 7.1): no muzzle particle; instruction 0x1413C28E8', SPAWN, 'inferred');
  return rows;
};
