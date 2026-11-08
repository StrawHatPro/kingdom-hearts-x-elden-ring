// (SHOT-A drain) Family `drain`: KH3's kinds 3 HE_DRAIN "Drain Shock" (Hero's Origin, WEP_KEYBLADE_SO_01, weapon type 2) and
// 10 RA_DRAIN "Shimmering Drops" (Ever After, WEP_KEYBLADE_SO_04, weapon type 5), both the base kind of their Keyblade.
// Rows `drain.*` of sheets/port/sora-shotlocks.json for kh-core magic/shotlocks/drain.rs.
//
// THE CODE (re/decomp C, constants checked in the disassembly of base/KH3.exe):
//   Update 0x1413B73D0 (ShootFlow vtable 0x14693CA58 +0x468):
//     sub 0: pawn vt+0x1728(0) gravity off, vt+0x17A0(5, 1) Flying (stopped), vt+0x1388(1, 0, 0) (weapon presentation);
//            Play "StyleChange11" (slot 0, blend -1, mode 2, group 0xC78, rate 1); length > 0: voice group 1 (anim
//            interface +0x1F0); m_bFaceAt (+0x84 |= 0x10); sub 4; LiftCheck vt+0x398(100.0) true -> sub 3.
//     sub 3: LiftStep vt+0x3A0 done -> sub 4; falls into sub 4.
//     sub 4: flagsCC bit 0 (OnTiming / TresAtk begin) or the clip ended: StopMovement (vt+0x16E8), bit 0 off, vt+0x470
//            (0x1413BFBF0, not sub 5: one HE_DRAIN_BLOW (kind 3) / RA_DRAIN_BLOW at his lock point); sub 5.
//     sub 5: bit 0: vt+0x470 (sub 5: +0x114 = 0; per locked target, per lock, one +0xB8 projectile (HE_DRAIN / RA_DRAIN)
//            at the target's lock-on point, vt+0x1100(cmd, kind)); +0xA0 = +0xA4 = 0.8; sub 6.
//     sub 6: vt+0x478 (0x1413BDE70: the drain orbs spiral to him); +0xA4 -= dt; <= 0: +0x114 > 0 -> Recover(6 x total
//            locks, 0, 0, 0xD) (pawn vt+0x1878); FUN_14140E500(ctrl, 0); vt+0x480 (0x1413BD870: the orbs off); sub 7;
//            falls into sub 7.
//     sub 7: the clip ended: Play "M_1472" (blend -1, mode 2, group 0), voice group 0; m_bEnableChange = 0; sub 2.
//     other: the clip ended: vt+0x1388(0, 0, 0), Finish (vt+0x270).
//   The hit call-back 0x1413B3780 (state+0x48 interface slot 18, from the pawn's attack-hit dispatch 0x1405B3A80):
//     kinds 3 / 10, the victim a character with tag "Enemy", the result not 5 (Guard), the attacker's vt+0x12E0
//     (ATresProjectileBase 0x14066E9F0 = +0x5F0 bit 3 = m_bAttachToHitActor): an orb (the weapon 0's +0x630 =
//     m_pDrainEffect, SpawnEmitterAtLocation at the victim's location) per victim (+0x148 list, 0x24-byte entries:
//     component, ..., count +0x18, victim +0x1C); a victim already listed: count += 1; +0x114 += 1.
//   Slot 13 0x1413B3590: kinds 3 / 10 rename the TresAtk group "ChangeSH" -> "Drain".
'use strict';

module.exports = (lib) => {
  const { row, clip, prop, blueprint, projectileSet, ATTACK, must, decomp, P } = lib;
  const fs = require('fs');
  const rows = [];
  const add = (...a) => rows.push(row(...a));
  const UPD = '0x1413B73D0', SPAWN = '0x1413BFBF0', ORBS = '0x1413BDE70', OFF = '0x1413BD870', HIT = '0x1413B3780';

  // the decompiled code must still say what the rows claim
  const c = decomp(UPD);
  must(c && c.includes('L"StyleChange11"') && c.includes('L"M_1472"') && c.includes('0xc78') && c.includes('0x3f4ccccd') && c.includes('uVar8 * 6') && c.includes(',0xd,'), UPD + ': not the drain update');
  const s = decomp(SPAWN);
  must(s && s.includes('L"RA_DRAIN_BLOW"') && s.includes('L"HE_DRAIN_BLOW"') && s.includes('== 3'), SPAWN + ': not the drain spawner');
  const o = decomp(ORBS);
  must(o && o.includes('0x42340000') && o.includes('500.0') && o.includes('-1000.0') && o.includes('20000.0') && o.includes('360.0'), ORBS + ': not the orb motion');
  const h = decomp(HIT);
  must(h && h.includes('"Enemy"') && h.includes("== '\\x05'") && h.includes('0x12e0') && h.includes('0x1488'), HIT + ': not the drain hit call-back');

  add('drain.update', UPD, 'addr', 'ShootFlow vtable 0x14693CA58 +0x468 (Begin 0x1413A5CD0 cases 3 / 10)', 'the update delegate of kinds 3 and 10', UPD);

  // ------------------------------------------------------------------------------------------ sub 0
  add('drain.lift.height', 100, 'cm', UPD + ' sub 0: vt+0x398 with xmm1 = [0x145C87D40] = 100.0 (0x1413B74CD)', 'LiftCheck 0x1413AD550 height (King of Hearts: 500)', UPD);
  const start = clip('StyleChange11');
  must(start, 'no clip StyleChange11');
  add('drain.clip', 'StyleChange11', '-', 'p_ex001 base set StyleChange11 -> sequence ' + start.sequence, 'Play(slot 0, blend -1, mode 2, group 0xC78, rate 1) in sub 0', UPD);
  add('drain.clip_length', start.length, 's', 'sequence ' + start.sequence + ' duration', 'sub 4 also moves on at its end; sub 7 waits for it', UPD);
  add('drain.clip_group', 0xC78, 'int', UPD + ' sub 0: mov dword ptr [rsp+28h],0C78h (0x1413B7490)', 'the effect group of the Play (StyleChange11\'s TresAttachEffect GroupID 3192)', UPD);
  add('drain.voice_group.start', 1, 'int', UPD + ' sub 0: anim interface +0x1F0(1) after the Play (length > 0)', 'SetVoiceGroupNo', UPD);
  add('drain.voice_group.end', 0, 'int', UPD + ' sub 7: anim interface +0x1F0(0) after the Play of M_1472', 'SetVoiceGroupNo', UPD);
  const timing = start.notifies.find((n) => n.class === 'TresAnimNotify_Timing');
  must(timing, 'StyleChange11: no TresTiming');
  add('drain.timing', timing.time, 's', 'StyleChange11 TresAnimNotify_Timing', 'OnTiming 0x1413B57D0 sets flagsCC bit 0 for kinds 3 / 10: sub 4 spawns the blow', '0x1413B57D0');
  const atk = start.notifies.filter((n) => n.class === 'TresAnimNotifyState_Attack');
  must(atk.length === 1, 'StyleChange11: expected one TresAtk');
  add('drain.atk.from', atk[0].time, 's', 'StyleChange11 TresAnimNotifyState_Attack', 'TresAtk begin 0x1413B5430 sets flagsCC bit 0: sub 5 spawns the drains', '0x1413B5430');
  add('drain.atk.to', atk[0].time + atk[0].duration, 's', 'StyleChange11 TresAnimNotifyState_Attack (time + duration)', 'the window closes', '0x1413B5430');
  add('drain.atk.group', String(prop(atk[0], 'CollisionGrpName')), '-', 'StyleChange11 TresAnimNotifyState_Attack CollisionGrpName', 'renamed by 0x1413B3590 (kinds 3 / 10) to drain.atk.group_renamed', '0x1413B3590');
  add('drain.atk.group_renamed', 'Drain', '-', '0x1413B3590: FName L"Drain" for kinds 3 / 10 when the group is L"ChangeSH"', 'no p_ex001 blueprint has a shape group "Drain" (searched Blueprints/Player/**): the window hits nothing', '0x1413B3590');
  add('drain.atk.row', String(prop(atk[0], 'm_AttackDataIDName')), '-', 'StyleChange11 TresAnimNotifyState_Attack m_AttackDataIDName', 'unused (no shape)', '-');
  const fx = start.notifies.filter((n) => n.class === 'TresAnimNotifyState_AttachEffect' || n.class === 'TresAnimNotify_AttachEffect');
  const fxName = (n) => { const m = JSON.stringify(n.properties).match(/Effects\/PC\/drain\/par\/(\w+)\./); return m ? m[1] : '-'; };
  const fxSocket = (n) => { const m = [...JSON.stringify(n.properties).matchAll(/"SocketName","type":"NameProperty","value":"(\w+)"/g)].map((x) => x[1]); return m.join('/'); };
  fx.forEach((n, i) => add(`drain.clip_effect${i}`, [fxName(n), fxSocket(n), String(n.time), String(n.end_time || n.time)], '-', 'StyleChange11 ' + n.class, 'effect, attach socket(s), from, to (group 3192); played by the generic notify path', '-'));
  const end = clip('M_1472');
  must(end, 'no clip M_1472');
  add('drain.end_clip', 'M_1472', '-', 'p_ex001 base set M_1472 -> sequence ' + end.sequence, 'Play(slot 0, blend -1, mode 2, group 0, rate 1) in sub 7', UPD);
  add('drain.end_clip_length', end.length, 's', 'sequence ' + end.sequence + ' duration', 'the state finishes at its end', UPD);

  // ------------------------------------------------------------------------------------------ the spawns (vt+0x470)
  for (const [k, w, pre] of [[3, 'w_so030', 'HE'], [10, 'w_so070', 'RA']]) {
    const set = projectileSet(w);
    const blow = pre + '_DRAIN_BLOW', drain = pre + '_DRAIN';
    must(set.get(blow) && set.get(drain), w + ': no ' + blow + ' / ' + drain);
    add(`drain.k${k}.blow`, blow, '-', SPAWN + ' (not sub 5): L"' + blow + '" (kind == 3 ? HE : RA)', 'spawned once at his lock point along his facing; ' + w + '_ProjSet -> ' + set.get(blow), SPAWN);
    const bp = (cls) => blueprint('/Game/Blueprints/Projectile/ShootFlow/' + cls.replace(/_C$/, ''));
    const b = bp(set.get(blow)), d = bp(set.get(drain));
    add(`drain.k${k}.blow_attach`, !!b.cdo.m_bAttachToHitActor, 'bool', 'Blueprints/Projectile/ShootFlow/' + set.get(blow).replace(/_C$/, '') + ' m_bAttachToHitActor (absent = false)', 'the hit call-back\'s attacker test (vt+0x12E0 = +0x5F0 bit 3): its hits make no orb', HIT);
    add(`drain.k${k}.drain_attach`, !!d.cdo.m_bAttachToHitActor, 'bool', 'Blueprints/Projectile/ShootFlow/' + set.get(drain).replace(/_C$/, '') + ' m_bAttachToHitActor', 'its hits make the orbs and count (+0x114)', HIT);
    const dRow = d.comps.TresAtkColl0 && JSON.stringify(d.comps.TresAtkColl0).match(/"m_DefaultAttackDataIDName":"(\w+)"/);
    must(dRow && ATTACK.get(dRow[1]), set.get(drain) + ': no attack row');
    add(`drain.k${k}.drain_row`, dRow[1], '-', set.get(drain) + ' TresAtkColl0.CollisionShapesSrc[0].m_DefaultAttackDataIDName', 'the hits of the drain projectile (the call-back\'s attacker)', HIT);
  }

  // ------------------------------------------------------------------------------------------ sub 5 / 6
  add('drain.timer', 0.8, 's', UPD + ' sub 5: +0xA0 = +0xA4 = 0x3F4CCCCD (0x1413B7583 / 0x1413B758D)', 'the orb time; sub 6 counts +0xA4 down', UPD);
  add('drain.heal.per_lock', 6, 'HP', UPD + ' sub 6: lea edx,[r11+r11*2]; add edx,edx (0x1413B7744)', 'HP = 6 x the locks of every lock entry (ctrl+0xB08 entries +0xC), when the drains hit (+0x114 > 0)', UPD);
  add('drain.heal.kind', 0xD, 'int', UPD + ' sub 6: mov byte ptr [rsp+20h],0Dh (0x1413B7752)', 'Recover(hp, 0, 0, 0xD) pawn vt+0x1878 (0x141445F40)', UPD);
  add('drain.orb.half', 0.5, '-', ORBS + ': xmm9 = +0xA0 x [0x145C9C4D0] = 0.5', 'h = time x 0.5: the ring shrinks and the rise stops after it', ORBS);
  add('drain.orb.spin', 360, 'deg', ORBS + ': [0x145C9C550] = 360.0', 'roll = i x 360 / n + his roll + elapsed x 360', ORBS);
  add('drain.orb.pitch', 45, 'deg', ORBS + ': FRotator(0x42340000 = 45.0, 0, 0)', 'the ring direction: (pitch 45) then the roll about his facing', ORBS);
  add('drain.orb.radius_rate', 500, 'cm/s', ORBS + ': +0xA0 x [0x145C9C554] = 500.0', 'ring radius = time x 500', ORBS);
  add('drain.orb.shrink_rate', -1000, 'cm/s', ORBS + ': [0x145E0D9DC] = -1000.0', 'after h: radius += (elapsed - h) x -1000', ORBS);
  add('drain.orb.rise_rate', 20000, 'cm/s2', ORBS + ': [0x145E2AB6C] = 20000.0', 'while +0xA4 > h: z += (+0xA4 - h) x dt x 20000 before the pull', ORBS);
  add('drain.orb.stop', 'vt+0xAE8', '-', OFF + ': component vt+0xAE8 per orb; End 0x1413ABA70: vt+0x368', 'sub 6 stops the orbs (fade), a dropped state removes them', OFF, 'inferred');

  // ------------------------------------------------------------------------------------------ the orb effect of each weapon
  const dir = P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content', 'Blueprints', 'Player', 'p_ex001');
  for (const f of fs.readdirSync(dir).filter((x) => /^w_so\d+_Wep\.uasset\.json$/i.test(x)).sort()) {
    const name = f.replace('.uasset.json', '');
    const b = blueprint('/Game/Blueprints/Player/p_ex001/' + name);
    if (!b.cdo.m_pDrainEffect) continue;
    const key = name.replace(/_Wep$/i, '').toLowerCase();
    add(`drain.orb.effect.${key}`, b.cdo.m_pDrainEffect, '-', 'Blueprints/Player/p_ex001/' + name + ' m_pDrainEffect', 'pawn vt+0x1488(0) -> equipment 0x14177FBE0: weapon 0 +0x630; the orb particle of the hit call-back', HIT);
  }
  must(rows.some((r) => r[0] === 'drain.orb.effect.w_so030') && rows.some((r) => r[0] === 'drain.orb.effect.w_so070'), 'no m_pDrainEffect on w_so030 / w_so070');
  add('drain.hit.guard_result', 5, 'int', HIT + ': cmp byte ptr [rsi+10h],5 (0x1413B395B)', 'a guarded hit (ReactionResult 5) makes no orb', HIT);
  return rows;
};
