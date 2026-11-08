// (SHOT-B) Family `vacuum`: KH3's kinds 15 FZ_VACUUMWAVE "Frozen Crescents" (Crystal Snow at MAX) and 17 CA_VACUUMWAVE
// "Flag Rampage" (Wheel of Fate at MAX). Rows `vacuum.*` of sheets/port/sora-shotlocks.json for kh-core
// magic/shotlocks/vacuum.rs.
//
// THE CODE (instruction level: build/sora_tmp/weapons/SHOT-B/spec/vacuum.md, listings spec/scripts/dis_1413BBEE0.txt,
// dis_1413C2E70.txt; decompiled C re/decomp/1413B/1413BBEE0.c, re/decomp/1413C/1413C2E70.c):
//   Begin 0x1413A5CD0 case 0xF / 0x11: update = thunk 0x1404EFAF8 -> ShootFlow vt+0x4F0 = 0x1413BBEE0; name FZ_VACUUMWAVE /
//   CA_VACUUMWAVE; group 0; +0xB0 / +0xB1 = the weapon form / grow (pawn vt+0x12E0 / vt+0x12E8).
//   Update 0x1413BBEE0: sub 0 gravity off, Flying; weapon type (vt+0x12C8) 7: ChangeWeapon vt+0x13C8(type, 7, 0, ...) and
//   M_7481, else vt+0x13C8(type, 0xF, 0, ...) and M_F481; face request; sub 4 / LiftCheck(200) -> 3; sub 3 LiftStep -> 4
//   (falls through); sub 4 / 5 at the clip's end: ShootFlowEnd, m_bEnableChange off, sub 2, ctrl 0x14140E500(ctrl, 0); sub 2:
//   m_bEnableChange and +0xB0: vt+0x1408, vt+0x13C8(type, +0xB0, +0xB1, ...), +0xB0 = 0; at the clip's end Finish (no
//   return). Tail: flagsCC bit 0 -> the spawner vt+0x4F8 0x1413C2E70(dt) while +0x90 < N, else the bit off.
//   Shot handler 0x1413B4A70 case 0xF / 0x11: flagsCC |= 1, +0x9C += LocationIndex (a hit count); slot 64 (TresAtk begin)
//   0x1413B5430: flagsCC |= 1 for every kind.
//   Spawner 0x1413C2E70: +0xA0 counts down first; +0x9C < 1: nothing; locked[+0x110] lock by lock (+0x114 against its
//   lockCount; a gone target: +0x110 += 1, +0x114 = 0); ONE projectile ON the entry's component (vt+0xB88), direction its
//   velocity (vt+0x418), fired at once (bFireNow 1), no homing target; +0x90 += 1, +0x9C -= 1; +0xA0 = 0.1.
// THE DATA: the clips' notifies (build/sora_pack/keyblades/sora_forms_manifest.json: 7481 of p_ex001_AnimSet7, F481 of
// p_ex001_AnimSet15), sora-keyblade-forms form.SK / form.FL (the weapon forms' parts and wear forms), the Storm Flag's
// TresAtkColl0 (w_so093_Wep KB_StickL) tracked per frame of F481 (FK as tools/sora_form_ship.js: Sora's skeleton under the
// clip -> R_wep6_skt -> w_so093 at its reference pose (it has no F481 clip) -> ATK_00 / ATK_01).
'use strict';

module.exports = (lib) => {
  const { row, clip, blueprint, projectileSet, ATTACK, must, readJson, P, MAIN, OV } = lib;
  const rows = [];
  const add = (...a) => rows.push(row(...a));
  const UPD = '0x1413BBEE0', SPAWN = '0x1413C2E70';

  add('vacuum.lift.height', 200, 'cm', UPD + ' sub 0: vt+0x398(200.0) (0x43480000)', 'LiftCheck 0x1413AD550 / LiftStep 0x1413C5400 (KC: 500)', UPD);
  add('vacuum.form_weapon_type', 7, 'int', UPD + ' sub 0: cmp eax, 7 on pawn vt+0x12C8(-1)', 'weapon type 7 (Crystal Snow): form 7 + M_7481; any other type: form 0xF + M_F481 (the type decides, not the kind) [I 0x1413BC0DE]', UPD);
  add('vacuum.spawn.interval', 0.1, 's', SPAWN + ': +0xA0 = 0x3DCCCCCD after a spawn', 'one projectile per call; 0.1 s in f32 minus 6 / 60 stays > 0: 7 ticks at 60 Hz', SPAWN);
  add('vacuum.spawn.min_len_sq', '1e-8', '-', SPAWN + ': normalize the component velocity when |v|^2 > 1e-8 (a text: the sheet rounds numbers)', 'the direction only turns the actor (speed 0)', SPAWN);
  const forms = readJson(P('sheets', 'port', 'sora-keyblade-forms.json'));
  const formRow = (id) => forms.rows.map((r) => Object.fromEntries(forms.columns.map((c, i) => [c, r[i]]))).find((r) => r.id === id);
  const END = clip('ShootFlowEnd');
  must(END, 'no clip ShootFlowEnd');
  add('vacuum.clip_end', 'ShootFlowEnd', '-', UPD + ' sub 4 / 5: Play("ShootFlowEnd") group 0 -> sequence ' + END.sequence, 'neither AnimSet7 nor AnimSet15 replaces it', UPD);
  add('vacuum.clip_end_length', END.length, 's', 'sequence ' + END.sequence + ' duration', 'Finish at its end', UPD);

  for (const [k, kind, w, name, form, formId, set] of [['k15', 15, 'w_so080', 'FZ_VACUUMWAVE', 7, 'form.SK', 'p_ex001_AnimSet7'], ['k17', 17, 'w_so090', 'CA_VACUUMWAVE', 15, 'form.FL', 'p_ex001_AnimSet15']]) {
    const logical = form === 7 ? 'M_7481' : 'M_F481';
    const c = clip(logical, set);
    must(c, 'no clip ' + logical + ' of ' + set);
    const cls = projectileSet(w).get(name);
    must(cls, w + '_ProjSet: no ' + name);
    const bp = blueprint('/Game/Blueprints/Projectile/ShootFlow/' + cls.replace(/_C$/, ''));
    must(bp.parent === 'TresProjectileBase', cls + ': parent ' + bp.parent);
    const shapes = (bp.comps.TresAtkColl0 || {}).CollisionShapesSrc || [];
    must(shapes.length === 1 && ATTACK.has(shapes[0].m_DefaultAttackDataIDName), cls + ': one attack shape with a row');
    const fr = formRow(formId);
    must(fr && fr.units && fr.units.length, 'sora-keyblade-forms has no ' + formId + ' units');
    const shots = c.notifies.filter((n) => n.class === 'TresAnimNotify_Shot');
    const sum = shots.reduce((s, n) => s + Number(lib.prop(n, 'LocationIndex') || 0), 0);
    const windows = c.notifies.filter((n) => n.class === 'TresAnimNotifyState_Attack');
    const groups = [...new Set(windows.map((n) => String(lib.prop(n, 'CollisionGrpName') || 'None')))];
    add('vacuum.' + k + '.weapon_form', form, 'int', UPD + ' sub 0: pawn vt+0x13C8(type, ' + form + ', 0, ...) (ChangeWeapon 0x14144A9F0)', formId + ' (' + (form === 7 ? 'Blizzard Blades' : 'Storm Flag') + ') put on the held Keyblade for the shotlock; restored at ShootFlowEnd\'s TresEnableChange / End', UPD);
    add('vacuum.' + k + '.weapon_units', fr.units, '-', 'sheets/port/sora-keyblade-forms ' + formId + ' units', 'the parts drawn instead of the Keyblade while the weapon form is on (kh-er-render keyblade.rs draw_form_weapon_parts)', '0x141791120');
    add('vacuum.' + k + '.wear_form', fr.wear_form, 'int', 'sheets/port/sora-keyblade-forms ' + formId + ' wear_form', 'equipment vt+0x5F8 0x1417915A0 sets the form\'s m_OwnerWearForm with the swap', '0x1417915A0');
    add('vacuum.' + k + '.clip', logical, '-', set + ' ' + logical + ' -> sequence ' + c.sequence, 'sub 0 plays it (slot 0, blend -1, mode 2, group 0)', UPD);
    add('vacuum.' + k + '.sequence', c.sequence, '-', set + ' ' + logical, 'the animation table row (AnimId) of the clip', UPD);
    add('vacuum.' + k + '.clip_length', c.length, 's', 'sequence ' + c.sequence + ' duration', 'ShootFlowEnd at its end', UPD);
    add('vacuum.' + k + '.shot_times', shots.map((n) => n.trigger_time), 's', logical + ' TresAnimNotify_Shot trigger times', 'each adds its LocationIndex to the queue +0x9C (the shared Shot handler)', '0x1413B4A70');
    add('vacuum.' + k + '.shot_counts', shots.map((n) => Number(lib.prop(n, 'LocationIndex') || 0)), 'int', logical + ' TresAnimNotify_Shot LocationIndex', 'the sum is the kind\'s Param0 (' + sum + ')', '0x1413B4A70');
    add('vacuum.' + k + '.proj_class', cls, '-', w + '_ProjSet ' + name, 'the stationary hit sphere the spawner places on a locked target', SPAWN);
    add('vacuum.' + k + '.attack_row', shapes[0].m_DefaultAttackDataIDName, '-', cls + ' TresAtkColl0', 'SMALL, magic, unblockable, not rapid fire', '-');
    add('vacuum.' + k + '.window_groups', groups, '-', logical + ' TresAnimNotifyState_Attack CollisionGrpName', 'the melee windows (row SF_VACUUMWAVE) on the form weapon\'s TresAtkColl0', '0x1413B5430');
  }
  // FZ: the Blizzard Blades (w_so084..087) have no KB_Stick* shape: those windows hit nothing (spec/vacuum.md 6).
  add('vacuum.k15.groups_without_shape', ['KB_StickM', 'KB_StickL'], '-', 'TresAtkColl0 of w_so084_Wep .. w_so087_Wep (groups SK_*)', 'a component without a primitive of the group does nothing (sora-combat-spec 8.2.1): no shape', '-');

  // CA: the Storm Flag's KB_StickL per frame of F481 (FK: tools/shotlocks/shotb/fk.js)
  const fk = require('./shotb/fk.js')(lib);
  const flag = blueprint('/Game/Blueprints/Player/p_ex001/w_so093_Wep');
  const stick = ((flag.comps.TresAtkColl0 || {}).CollisionShapesSrc || []).find((s) => s.m_GrpName === 'KB_StickL');
  must(stick && /CAPSULE2/.test(stick.m_ShapeType) && stick.m_SocketName1 === 'ATK_00' && stick.m_SocketName2 === 'ATK_01', 'w_so093_Wep: KB_StickL is a CAPSULE2 ATK_00 -> ATK_01');
  const l1 = stick.m_RelativeLocation1 || { x: 0, y: 0, z: 0 }, l2 = stick.m_RelativeLocation2 || { x: 0, y: 0, z: 0 };
  const seq = clip('M_F481', 'p_ex001_AnimSet15').sequence;
  const tr = fk.track(seq, 'R_wep6_skt', 'w_so093', [['ATK_00', [l1.x, l1.y, l1.z]], ['ATK_01', [l2.x, l2.y, l2.z]]]);
  const values = [];
  for (let f = 0; f < tr.frames; f++) values.push(...tr.values.slice(f * 6, f * 6 + 6), stick.m_Size.x);
  add('vacuum.k17.track_frames', tr.frames, 'int', 'sequence ' + seq + ' num_frames (' + tr.length + ' s)', 'frame i at i x length / (frames - 1)', '-');
  add('vacuum.k17.track_kb_stickl', values, 'cm', 'FK: sequence ' + seq + ' (' + tr.source + ') -> R_wep6_skt -> w_so093 (' + (tr.weaponClip === '-' ? 'reference pose: it has no ' + seq + ' clip' : 'its clip ' + tr.weaponClip) + ') -> ATK_00 / ATK_01 + m_RelativeLocation1 / 2', 'per frame [ax, ay, az, bx, by, bz, radius] of w_so093_Wep KB_StickL (CAPSULE2, r ' + stick.m_Size.x + '), actor space, his yaw 0', '-');  return rows;
};
