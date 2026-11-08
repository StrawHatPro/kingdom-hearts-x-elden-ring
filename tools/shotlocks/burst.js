// (SHOT-B) Family `burst`: KH3's kinds 14 FZ_BURST "Diamond Dust" (Crystal Snow, weapon type 7's base kind) and 16 CA_BURST
// "Blade Storm" (Wheel of Fate, weapon type 10's base kind); also the update of 0x1B SL_SHOT / 0x29 DF_BURST (other
// packages). Rows `burst.*` of sheets/port/sora-shotlocks.json for kh-core magic/shotlocks/burst.rs.
//
// THE CODE (instruction level: build/sora_tmp/weapons/SHOT-B/spec/burst.md, listings spec/dis_1413BADB0.txt,
// spec/scripts/burst/dis_1413C2A70.txt; decompiled C re/decomp/1413B/1413BADB0.c, re/decomp/1413C/1413C2A70.c):
//   Begin 0x1413A5CD0 case 0xE / 0x10: update = thunk 0x140430C7C -> ShootFlow vt+0x3A8 = 0x1413BADB0; name FZ_BURST /
//   CA_BURST; group 0x7D5 / 0.
//   Update 0x1413BADB0: sub 0 Flying (vt+0x17A0(5, 1)), gravity off, vt+0x1388(1, 0, weapon kind == 0x18), Play M_1466
//   (group +0xC0), face request, sub 4 / LiftCheck(500) -> 3; sub 3 LiftStep -> 4 (falls through); sub 4 at the clip's
//   end: Flying, the volley set-up 0x1413C3330 (N, +0xA0 = 0.5, SetFace off), Play M_1467, flagsCC |= 1, sub 5; sub 5 / 6
//   at the clip's end: M_1468 / M_1469 while spawned < N, else ShootFlowEnd (group 0) + PlayerSpecificAction(2, 0), sub 2;
//   sub 7 at the clip's end: ShootFlowEnd + the action, sub 2; sub 2 at the clip's end: vt+0x1388(0, 0, 0), Finish (no
//   return). Tail: flagsCC bit 0 -> the spawner 0x1413C2A70(dt) while spawned < N, else the bit off; SetFace(false, 0).
//   Spawner 0x1413C2A70: +0xA0 counts down first; locked[+0x110] lock by lock (+0x114 against its lockCount); a gone
//   target: +0x110 += 1, +0x114 = 0, no shot; from the blade tip (weapon 0's ATK_01, read for every shot) along
//   normalize(tip - lock point), homing on the entry, SetMoveStartDelay(0.033333), StartFire; +0xA0 = 0.066666 in subs
//   5..7 only.
'use strict';

module.exports = (lib) => {
  const { row, clip, blueprint, projectileSet, ATTACK, must } = lib;
  const fk = require('./shotb/fk.js')(lib);
  const rows = [];
  const add = (...a) => rows.push(row(...a));
  const UPD = '0x1413BADB0', SPAWN = '0x1413C2A70';
  const SPEC = 'build/sora_tmp/weapons/SHOT-B/spec/burst.md';

  // the clips the update plays
  const clips = [['charge', 'M_1466', 'sub 0'], ['fire1', 'M_1467', 'sub 4, with the set-up'], ['fire2', 'M_1468', 'sub 5 while spawned < N'], ['fire3', 'M_1469', 'sub 6 while spawned < N'], ['end', 'ShootFlowEnd', 'subs 5..7, group 0 (a literal)']];
  for (const [key, name, when] of clips) {
    const c = clip(name);
    must(c, 'no clip ' + name);
    add('burst.clip.' + key, name, '-', UPD + ' Play("' + name + '") -> sequence ' + c.sequence, when + ' (slot 0, blend -1, mode 2, rate 1)', UPD);
    add('burst.clip.' + key + '_length', c.length, 's', 'sequence ' + c.sequence + ' duration', 'the update waits for its end (anim interface vt+0xC8)', UPD);
    must(!c.notifies.some((n) => n.class === 'TresAnimNotify_Shot' || n.class === 'TresAnimNotifyState_Attack'), name + ': a TresShot / TresAtk notify (the shared handlers would spawn / flag)');
  }
  const change = clip('ShootFlowEnd').notifies.find((n) => n.class === 'TresAnimNotify_EnableChange');
  must(change, 'ShootFlowEnd: no TresEnableChange');
  add('burst.clip.end_enable_change', change.trigger_time, 's', 'ShootFlowEnd TresAnimNotify_EnableChange', 'm_bEnableChange back (the state does not clear it: the base reset 0x14150DF20 did)', UPD);

  // the update's constants
  add('burst.lift.height', 500, 'cm', UPD + ' sub 0: vt+0x398(500.0)', 'LiftCheck 0x1413AD550 / LiftStep 0x1413C5400 (as KC)', UPD);
  add('burst.end_action', 2, 'int', UPD + ' subs 5..7: pawn vt+0x1038(2, 0) = PlayerSpecificAction 0x141432AF0', 'TRES_PSA_SHOOTFLOW_ACTION_END: an event for listeners (the state-event camera / emotion manager); with the ShootFlowEnd play' + ' [I 0x1413BB0ED]', UPD);
  add('burst.release_in_update', false, 'bool', UPD + ': no call of 0x14140E500 and no m_bEnableChange write', 'the release pending of the controller goes off in End 0x1413ABA70 only (slot 809; (SHOT-FIX) 0x14140E500 is WorldSlow, not a release, in every family)', UPD);
  add('burst.spawn.first_delay', 0.5, 's', 'volley set-up 0x1413C3330: +0xA0 = 0.5 (0x3F000000)', 'the first shot waits for it (= kc2.volley.ring_time, the same set-up)' + ' [I 0x1413C34B4]', '0x1413C3330');
  add('burst.spawn.interval', 0.066666, 's', SPAWN + ': +0xA0 = 0x3D88882F after a shot in subs 5 / 6 / 7', 'not re-armed in sub 2: the last two of 31 / 32 locks come one tick apart' + ' [I 0x1413C2E29]', SPAWN);
  add('burst.spawn.rearm_subs', [5, 6, 7], '-', SPAWN + ': m_nSubStep test before the re-arm', 'the subs that re-arm the timer', SPAWN);
  add('burst.spawn.move_start_delay', 0.033333, 's', SPAWN + ': projectile vt+0x1240(0x3D088889)', 'SetMoveStartDelay of every shot' + ' [I 0x1413C2DAE]', SPAWN);
  add('burst.spawn.min_len_sq', '1e-8', '-', SPAWN + ': normalize when |d|^2 > 1e-8 (a text: the sheet rounds numbers to 6 digits)', 'd = blade tip - pawn vt+0x6E0 (his lock-on point), 3-D' + ' [I 0x1413C2C65]', SPAWN);

  // per kind: the projectile its Keyblade's set names, the row, the effect group
  for (const [k, kind, w, name, group] of [['k14', 14, 'w_so080', 'FZ_BURST', 2005], ['k16', 16, 'w_so090', 'CA_BURST', 0], ['k27', 27, 'w_so150', 'SL_SHOT', 2360], ['k41', 41, 'w_so210', 'DF_BURST', 2200]]) {
    const cls = projectileSet(w).get(name);
    must(cls, w + '_ProjSet: no ' + name);
    const bp = blueprint('/Game/Blueprints/Projectile/ShootFlow/' + cls.replace(/_C$/, ''));
    must(bp.parent === 'TresProjectileBase', cls + ': parent ' + bp.parent);
    const shapes = (bp.comps.TresAtkColl0 || {}).CollisionShapesSrc || [];
    must(shapes.length === 1, cls + ': one attack shape');
    const atk = shapes[0].m_DefaultAttackDataIDName;
    must(ATTACK.has(atk), 'p_ex001_AttackData has no ' + atk);
    add('burst.' + k + '.proj_class', cls, '-', w + '_ProjSet ' + name, 'Begin 0x1413A5CD0 names ' + name + ' (+0xB8); the spawner spawns it (pawn vt+0x1450)', SPAWN);
    add('burst.' + k + '.attack_row', atk, '-', cls + ' TresAtkColl0', 'SIDESPIN 0.8 magic rapid-fire (= SF_KCSHOT)', '-');
    add('burst.' + k + '.effect_group', group, 'int', 'Begin 0x1413A5CD0 case ' + kind + ': +0xC0', 'M_1466\'s two charge effects are the rows of this GroupID (' + SPEC + ' 6)' + ' [I 0x1413A5FDE]', '0x1413A5CD0');
    // The muzzle (pawn vt+0x1410(0) -> equipment vt+0x580: weapon 0's ATK_01) per frame of the clips the spawner fires in:
    // the held Keyblade on R_wep_skt (the pack carries tracks for the Kingdom Key's shotlock clips only).
    const seqs = [];
    for (const key of ['fire1', 'fire2', 'fire3', 'end']) {
      const name = clips.find((c) => c[0] === key)[1];
      const seq = clip(name).sequence;
      const tr = fk.track(seq, 'R_wep_skt', w, [['ATK_01', [0, 0, 0]]]);
      add('burst.' + k + '.muzzle.' + seq.toLowerCase(), tr.values, 'cm', 'FK: sequence ' + seq + ' (' + tr.source + ') -> R_wep_skt -> ' + w + ' (' + (tr.weaponClip === '-' ? 'reference pose' : 'its clip ' + tr.weaponClip) + ') -> ATK_01', 'per frame [x, y, z] of the blade tip, actor space, his yaw 0 (frame i at i x length / (frames - 1))', SPAWN);
      seqs.push(seq);
    }
    add('burst.' + k + '.muzzle_sequences', seqs, '-', 'the sequences of M_1467 / M_1468 / M_1469 / ShootFlowEnd', 'the clips a bolt can leave in (31 / 32 locks: ShootFlowEnd)', SPAWN);
  }
  return rows;
};
