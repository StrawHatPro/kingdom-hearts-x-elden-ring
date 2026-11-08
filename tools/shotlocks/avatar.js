// (SHOT-C avatar) Family `avatar`: KH3's kinds 8 "Ghost Horde" (Favorite Deputy, WEP_KEYBLADE_SO_03, weapon type 4) and 25
// "Phantom Rush" (Classic Tone, WEP_KEYBLADE_SO_11, weapon type 12), both the base kind of their Keyblade. Rows `avatar.*`
// of sheets/port/sora-shotlocks.json for kh-core magic/shotlocks/avatar.rs. (Moved from SHOT-A to SHOT-C, 2026-11-03.)
//
// THE CODE (re/decomp C; constants checked in the disassembly, build/sora_tmp/weapons/SHOT-C/avatar/dis_*.txt):
//   Begin 0x1413A5CD0: case 0x19 sets +0xC0 = 0x76C, +0xC4 = 1 and falls into case 8: delegate FUN_1404EFA74 -> vt+0x498.
//     Neither sets the projectile name +0xB8 (stays None): the Shot handler's default branch (kind 25) spawns nothing,
//     kind 8 is the handler's empty case.
//   Update 0x1413B5E30 (ShootFlow vtable 0x14693CA58 +0x498):
//     sub 0: gravity off (vt+0x1728(0)), Flying stopped (vt+0x17A0(5, 1)), vt+0x1388(1, 0, 0) (weapon presentation); Play
//            "M_1454" (blend -1, mode 2, group +0xC0, rate 1); length > 0: voice group +0xC4 (anim interface +0x1F0);
//            m_bFaceAt; sub 4; LiftCheck vt+0x398(100.0) true -> sub 3.
//     sub 3: LiftStep vt+0x3A0 done -> sub 4; falls into sub 4.
//     sub 4: the clip ended: pawn vt+0x1010(0, 0) (body collision off), sub 1, WorldSlow(ctrl, 1) (FUN_14140E500: types
//            0x419C at 0.1), +0xA4 = 0.133333, +0xC8 = 10.0, +0x110 = +0x114 = 0, +0xE4 = his location.
//     sub 1: +0xC8 -= dt; vt+0x4A8 (0x1413BDB80: move the avatars; true when every avatar has arrived and there are as many
//            as locked targets) or +0xC8 <= 0: sub 5, +0xA4 = 0.5; else +0xA4 -= dt, <= 0: vt+0x4A0(1) (0x1413BD290: the
//            next avatar), +0xA4 = 0.3.
//     sub 5: +0xA4 -= dt; <= 0: vt+0x4A0(0) (the avatars off), sub 6, ctrl 0x1414122A0(0, 0, 0, 0) (the shotlock event
//            camera off).
//     sub 6: vt+0x1010(1, 0) (body collision on), StopMovement, vt+0x1628 (MaxFlySpeed default); Play "M_1456" (group +0xC0),
//            voice group 0; vt+0x4B0 (0x1413BF420: the rush); m_bEnableChange = 0; sub 7.
//     sub 7: the clip ended: Play "ShootFlowEnd" (group +0xC0), voice group 0; m_bEnableChange = 0; sub 2.
//     other: the clip ended: vt+0x1388(0, 0, 0), Finish (vt+0x270).
//   0x1413BD290(1): for locked entry +0x114 (once per locked TARGET; +0x114 += 1 even when the target is gone): d = unit
//     (target's lock point - his lock point (pawn vt+0x6E0)); SpawnEmitterAtLocation (0x14411B3D0) of pawn vt+0x1490(0) =
//     equipment slot 188 0x141781B60 = weapon 0's m_pShootFlowEffect (TresWeaponBase +0x638) at his lock point, rotation of
//     d; the entry (+0x148 list, 0x24 bytes): the particle, the move = (target - him) + 100 d, the timer 0.166666; then
//     the event camera on that target (ctrl 0x1414122A0(1, 2, target, +0xE4)).
//   0x1413BD290(0): every listed particle off (the first FUN_1442633A0(comp, 0.1), the others vt+0xAE8), the list emptied.
//   0x1413BDB80: per entry with timer > 0: timer <= 0.033333 -> the move's z = 0; the particle += move x min(6.000024 dt,
//     2); timer -= dt; an entry with timer <= 0 counts (+0x110); true when +0x110 >= locked targets.
//   0x1413BF420: only while +0x90 < 1: WorldSlow(ctrl, 0); per locked entry whose target lives, per lock: projectile
//     "SF_AVATARRUSH" (the held Keyblade's set) at the target's location (vt+0xB88), no direction, SetCommand(cmd, kind),
//     attached to the target (0x143E8FC60), SetDelayFire(1, 0.033333 x lock index, 0) (0x141482540; StartFire 0x141474970
//     then waits that long), fire effects, StartFire; +0x90 += 1.
'use strict';

module.exports = (lib) => {
  const { row, clip, blueprint, must, decomp } = lib;
  const rows = [];
  const add = (...a) => rows.push(row(...a));
  const UPD = '0x1413B5E30', SPAWN = '0x1413BD290', MOVE = '0x1413BDB80', RUSH = '0x1413BF420';

  // the decompiled code must still say what the rows claim
  const u = decomp(UPD);
  must(u && u.includes('L"M_1454"') && u.includes('L"M_1456"') && u.includes('L"ShootFlowEnd"') && u.includes('0x3e088872') && u.includes('0x41200000') && u.includes('0x3e99999a') && u.includes('0x3f000000') && u.includes('0x42c80000'), UPD + ': not the avatar update');
  const sp = decomp(SPAWN);
  must(sp && sp.includes('0x3e2aaa7e') && sp.includes('* 100.0') && sp.includes('0x1490'), SPAWN + ': not the avatar spawner');
  const mv = decomp(MOVE);
  must(mv && mv.includes('6.000024') && mv.includes('0.033333') && mv.includes('2.0'), MOVE + ': not the avatar motion');
  const r = decomp(RUSH);
  must(r && r.includes('L"SF_AVATARRUSH"') && r.includes('0x1248') && r.includes('0.033333'), RUSH + ': not the rush');

  add('avatar.update', UPD, 'addr', 'ShootFlow vtable 0x14693CA58 +0x498 (Begin 0x1413A5CD0 cases 8 / 0x19)', 'the update delegate of kinds 8 and 25', UPD);

  // ------------------------------------------------------------------------------------------ clips
  for (const [key, name, where] of [['start', 'M_1454', 'sub 0'], ['rush', 'M_1456', 'sub 6'], ['end', 'ShootFlowEnd', 'sub 7']]) {
    const c = clip(name);
    must(c, 'no clip ' + name);
    add('avatar.clip.' + key, name, '-', 'p_ex001 base set ' + name + ' -> sequence ' + c.sequence, 'Play(slot 0, blend -1, mode 2, group +0xC0, rate 1) in ' + where, UPD);
    add('avatar.clip.' + key + '_length', c.length, 's', 'sequence ' + c.sequence + ' duration', 'the update waits for its end', UPD);
  }
  const rush = clip('M_1456');
  const shot = rush.notifies.find((n) => n.class === 'TresAnimNotify_Shot');
  must(shot, 'M_1456: no TresShot');
  add('avatar.clip.rush_shot', shot.time, 's', 'M_1456 TresAnimNotify_Shot', 'the Shot handler 0x1413B4A70: kind 8 = its empty case, kind 25 = the default branch with +0xB8 None (pawn vt+0x1450 finds no class): nothing is spawned', '0x1413B4A70');
  add('avatar.voice_group.start', 'kind', '-', UPD + ' sub 0: anim interface +0x1F0(+0xC4) after the Play (length > 0)', 'SetVoiceGroupNo with the kind\'s voice (kind.k<N>.voice: 8 -> 0, 25 -> 1); M_1456 / ShootFlowEnd pass 0', UPD);

  // ------------------------------------------------------------------------------------------ constants of the update
  add('avatar.lift.height', 100, 'cm', UPD + ' sub 0: vt+0x398 with 0x42C80000 (0x1413B5F63)', 'LiftCheck 0x1413AD550 height', UPD);
  add('avatar.first_delay', 0.133333, 's', UPD + ' sub 4: mov dword ptr [rdi+0A4h],3E088872h (0x1413B5FE5)', '+0xA4: the first avatar comes when it runs out', UPD);
  add('avatar.max_time', 10, 's', UPD + ' sub 4: mov dword ptr [rdi+0C8h],41200000h (0x1413B5FEF)', '+0xC8: sub 1 ends at the latest after it', UPD);
  add('avatar.interval', 0.3, 's', UPD + ' sub 1: mov dword ptr [rdi+0A4h],3E99999Ah (0x1413B60C1)', '+0xA4 after an avatar', UPD);
  add('avatar.finish_wait', 0.5, 's', UPD + ' sub 1 -> 5: mov dword ptr [rdi+0A4h],3F000000h (0x1413B60D7)', 'the avatars stay this long before they go off and the rush comes', UPD);

  // ------------------------------------------------------------------------------------------ the avatars
  add('avatar.move.beyond', 100, 'cm', SPAWN + ': mulss by [0x145C87D40] = 100.0 (0x1413BD61F..0x1413BD62D)', 'the move = (target - his lock point) + 100 x unit direction: the avatar ends 100 cm past the target', SPAWN);
  add('avatar.move.time', 0.166666, 's', SPAWN + ': [0x146932698] = 0.166666 (0x1413BD68F)', 'the entry timer', SPAWN);
  add('avatar.move.rate', 6.000024, '1/s', MOVE + ': [0x14693D720] (0x1413BDCA2)', 'per tick the particle moves move x min(rate x dt, cap): rate x time = 1, the whole move', MOVE);
  add('avatar.move.cap', 2, '-', MOVE + ': [0x145C9C4F4] (0x1413BDCB1)', 'the per-tick factor cap', MOVE);
  add('avatar.move.flat_below', 0.033333, 's', MOVE + ': [0x14693D710] (0x1413BDC8A)', 'with the timer at or below it the move\'s z is 0 (the last ticks stay level)', MOVE);
  // (SHOT-FIX) the attack camera after each avatar: thunk 0x14140DF20 = 0x1414122A0(ctrl, 0, ...) then (ctrl, 1, 2, target,
  // &ShootFlow+0xE4); off again in sub 5 and in End 0x1413ABA70
  must(sp.includes("thunk_FUN_1414122a0(plVar5,'\\x01',2,"), SPAWN + ': no attack camera mode 2 after the avatar');
  add('avatar.camera.mode', 2, 'int', SPAWN + ': thunk_FUN_1414122a0(ctrl, 1, 2, the entry\'s component, &this->field_0xe4)', 'the attack camera mode after each avatar (combat/camera.rs request_at: a camera of its own on that target, anchored at +0xE4); off in sub 5 and End', SPAWN);
  // The particle: weapon 0's m_pShootFlowEffect (TresWeaponBase +0x638, reflection offset 1592) of the held Keyblade.
  for (const [w, key] of [['w_so050', 'WEP_KEYBLADE_SO_03'], ['w_so110', 'WEP_KEYBLADE_SO_11']]) {
    const bp = blueprint('/Game/Blueprints/Player/p_ex001/' + w + '_Wep');
    const fx = bp.cdo.m_pShootFlowEffect;
    must(fx, w + '_Wep: no m_pShootFlowEffect');
    add('avatar.effect.' + key.toLowerCase(), String(fx), '-', '/Game/Blueprints/Player/p_ex001/' + w + '_Wep m_pShootFlowEffect', 'the avatar particle (pawn vt+0x1490(0) -> equipment 0x141781B60 -> weapon +0x638), SpawnEmitterAtLocation 0x14411B3D0', SPAWN);
  }

  // ------------------------------------------------------------------------------------------ the rush
  add('avatar.rush.proj', 'SF_AVATARRUSH', '-', RUSH + ': FName literal L"SF_AVATARRUSH"', 'resolved by the held Keyblade\'s projectile set: w_so050 TresProjTS_AvatarRush (SF_AVATARRUSH), w_so110 TresProjTR_AvatarRush (SF_TRAVATARRUSH)', RUSH);
  add('avatar.rush.delay_step', 0.033333, 's', RUSH + ': [0x14693D710] added per lock (0x1413BF4E3)', 'SetDelayFire(1, step x lock index of that target, 0): the projectiles of one target fire one tick apart', RUSH);
  return rows;
};
