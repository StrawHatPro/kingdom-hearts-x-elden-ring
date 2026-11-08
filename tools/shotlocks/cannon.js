// (SHOT-A cannon) Family `cannon`: KH3's kind 7 BZ_CANNON "Cluster Cannonade" (Shooting Star's MAX kind when 0x1413ECC80 is
// false) and SHOT-B's kind 22 PO_CANNON "Sweet Surprise" (Hunny Spout's), both run by TresPlayerState_ShootFlow update
// 0x1413B6960 (vtable 0x14693CA58 + 0x3E0) and the cluster call-back 0x1413B3BF0 (OnProjectileShutdown, the state's
// notify interface +0x48 vt+0x98). Rows `cannon.*` of sheets/port/sora-shotlocks.json for kh-core magic/shotlocks/cannon.rs.
//
// THE CODE (re/decomp/1413B/1413B6960.c, constants re-read in build/sora_tmp/weapons/SHOT-B/spec/scripts/dis_1413B6960.txt):
//   sub 0: gravity off (vt+0x1728(0)); Flying, stopped (vt+0x17A0(5, 1)); the weapon change vt+0x13C8(weapon type, 5 (kind
//          0x16: 0x11), 0, ...) -> Magic Launcher (Hunny Launcher) and its anim sets; Play "ShootFlowStart" (slot 0, blend
//          0.2, mode 2, group 0, rate 1), voice group 0 (+0x1F0) when its length > 0; m_bFaceAt; sub 4; LiftCheck
//          vt+0x398(200) -> sub 3.
//   sub 3: LiftStep vt+0x3A0 done -> sub 4; falls into the sub 4 code (0x1413B6B0B -> 0x1413B6B12).
//   sub 4: at the clip's end: vt+0x17A0(5, 1); Play "M_5482" (blend -1, group 0), voice group 0; m_bFaceAt; sub 5.
//   sub 5: at the clip's end: Play "ShootFlowEnd" (blend -1, group 0), voice group 0; m_bEnableChange = 0; sub 2.
//   other: once m_bEnableChange (ShootFlowEnd's TresEnableChange) and +0xB0 != 0: vt+0x1408, the controller ->
//          vt+0x13C8(weapon type, +0xB0, +0xB1) (Begin's weapon form back), +0xB0 = 0; at the clip's end Finish (vt+0x270,
//          tail jump 0x1413B6A45).
//   The shell: M_5482's TresShot -> the Shot handler 0x1413B4A70's default branch (one +0xB8 projectile at weapon 0's
//   ATK_01 along his facing, homing target locked[0]; kinds 7 / 0x16 keep it at +0x118, 0x1413B514D).
//   The cluster 0x1413B3BF0 (re/decomp/1413B/1413B3BF0.c, build/sora_tmp/weapons/SHOT-B/spec/scripts/dis_1413B3BF0.txt):
//   when the kept shell shuts down (+0x118 == it, kind 7 / 0x16, +0x120 == 0): N (+0x94) "BZ_CANNON2" at the shell's
//   location, direction = Rotator(Quat(yaw of its horizontal forward, roll 360 / N x i) * Quat(pitch 45)).Vector(), target
//   locked[+0x90 mod locked] (none locked: +0xD0), +0x90 += 1 per spawn; +0x120 = 1.
// THE DATA: the clips (ShootFlowStart / ShootFlowEnd of the base set: AnimSet5 / AnimSet17 do not replace them; M_5482 of
// p_ex001_AnimSet5), sheets/port/sora-keyblade-forms form.BZ / form.BZ2 (weapon form values, units, wear forms), the
// launcher's FK muzzle over M_5482 (tools/shotlocks/shotb/fk.js: Sora's clip -> R_wep1_skt -> the launcher -> ATK_01).
'use strict';
const path = require('path');

module.exports = (lib) => {
  const { row, clip, prop, ATTACK, must, readJson, P, decomp, blueprint, projectileSet } = lib;
  const rows = [];
  const add = (...a) => rows.push(row(...a));
  const UPD = '0x1413B6960', CLU = '0x1413B3BF0', SHOT = '0x1413B4A70';
  const DIS = 'build/sora_tmp/weapons/SHOT-B/spec/scripts/dis_1413B6960.txt';
  const DISC = 'build/sora_tmp/weapons/SHOT-B/spec/scripts/dis_1413B3BF0.txt';

  // the decompiled code must still say what the rows claim
  const u = decomp(UPD);
  must(u && u.includes('L"ShootFlowStart"') && u.includes('L"M_5482"') && u.includes('L"ShootFlowEnd"') && u.includes('0x3e4ccccd') && u.includes('0x43480000') && u.includes('== 0x16') && u.includes('uVar8 = 0x11') && u.includes('uVar8 = 5'), UPD + ': not the cannon update');
  const c = decomp(CLU);
  must(c && c.includes('L"BZ_CANNON2"') && c.includes('0x42340000') && c.includes('360.0') && c.includes('== 7') && c.includes('== 0x16') && c.includes('1e-08'), CLU + ': not the cannon cluster');

  add('cannon.update', UPD, 'addr', 'ShootFlow vtable 0x14693CA58 + 0x3E0 (Begin 0x1413A5CD0 cases 7 / 0x16)', 'the update delegate of kinds 7 (BZ_CANNON) and 0x16 (PO_CANNON)', UPD);
  add('cannon.kinds', [7, 22], 'kind', 'sora-shotlocks kind.k7 / kind.k22 update 0x1413B6960', 'the kinds this update runs (7 SHOT-A Cluster Cannonade, 22 SHOT-B Sweet Surprise)', UPD);
  add('cannon.cluster.handler', CLU, 'addr', 'state+0x48 interface (vtable 0x14693D3E0) +0x98', 'OnProjectileShutdown: projectile Shutdown slot 532 0x141475670 -> its owner\'s notify interface +0x98 -> Sora 0x141443FD0 -> the current state\'s +0x48 interface', CLU);

  // ---------------------------------------------------------------------------------------------- the weapon forms
  const forms = readJson(P('sheets', 'port', 'sora-keyblade-forms.json'));
  const formRow = (id) => { const r = forms.rows.map((x) => Object.fromEntries(forms.columns.map((col, i) => [col, x[i]]))).find((x) => x.id === id); must(r, 'sora-keyblade-forms has no ' + id); return r; };
  // [kind, form value, form row, the instruction, the weapon's projectile set, the shell's FName]
  const KIND_FORM = [[7, 5, 'form.BZ', '0x1413B6C22 mov r8b,5', 'w_so020', 'BZ_CANNON'], [22, 17, 'form.BZ2', '0x1413B6C1B mov r8b,11h (kind == 0x16, cmp at 0x1413B6BFD)', 'w_so040', 'PO_CANNON']];
  const FORM_OF = {};
  for (const [kind, value, id, at] of KIND_FORM) {
    const f = formRow(id);
    must(f.weapon_form_value === value, id + ': weapon form ' + f.weapon_form_value + ', the code puts on ' + value);
    FORM_OF[kind] = f;
    add('cannon.k' + kind + '.weapon_form', value, 'int', UPD + ' sub 0: pawn vt+0x13C8(weapon type, ' + value + ', 0, ...) at ' + at + ' (' + DIS + ')', f.weapon_form + ' = ' + f.name + ' (' + id + '): its weapon and anim sets ' + f.anim_sets.map(lib.setName).join(' + ') + '; Begin\'s +0xB0 / +0xB1 come back in sub 2 / End', UPD);
    add('cannon.k' + kind + '.weapon_units', f.units, '-', 'sheets/port/sora-keyblade-forms ' + id + ' units', 'the parts the hosts draw instead of the Keyblade while PawnMagic::shot_weapon_form is ' + value, UPD);
    add('cannon.k' + kind + '.wear_form', f.wear_form, 'int', 'sheets/port/sora-keyblade-forms ' + id + ' wear_form (' + f.wear_form_name + ')', 'equipment vt+0x5F8 0x1417915A0 sets the form\'s m_OwnerWearForm with the swap (as vacuum.k15.wear_form); whether vt+0x13C8 -> equipment vt+0x4D8 does it here was not read (SHOT-B spec/rocket.md 7.3)', '0x1417915A0', 'inferred');
  }

  // ---------------------------------------------------------------------------------------------- the clips
  const CLIPS = [['start', 'ShootFlowStart', null, '0x1413B6C52 (@1469333A0), sub 0, blend 0.2'], ['fire', 'M_5482', 'p_ex001_AnimSet5', '0x1413B6B48 (@1469333C0), sub 4 at ShootFlowStart\'s end, blend -1'], ['end', 'ShootFlowEnd', null, '0x1413B6A6F (@1469333D0), sub 5 at M_5482\'s end, blend -1']];
  const C = {};
  for (const [key, name, set, at] of CLIPS) {
    const cl = clip(name, set);
    must(cl, 'no clip ' + name + (set ? ' of ' + set : ''));
    for (const other of ['p_ex001_AnimSet5', 'p_ex001_AnimSet17']) {
      const o = clip(name, other);
      must(!o || o.sequence === cl.sequence, name + ': ' + other + ' replaces it');
    }
    C[key] = cl;
    add('cannon.clip.' + key, name, '-', UPD + ' Play FName at ' + at, 'slot 0, mode 2, group 0 (the cannon never passes +0xC0), rate 1; ' + (set || 'the base set') + ' ' + name + ' (neither AnimSet5 nor AnimSet17 replaces it otherwise)', UPD);
    add('cannon.clip.' + key + '.sequence', cl.sequence, '-', (set || 'p_ex001 base set') + ' ' + name + ' -> sequence', 'the animation-table row (AnimId)', UPD);
    add('cannon.clip.' + key + '.length', cl.length, 's', 'sequence ' + cl.sequence + ' duration', 'the sub-step waits for its end', UPD);
  }
  add('cannon.clip.start.blend', 0.2, 's', UPD + ' sub 0: movss xmm3,[145C9C4CCh] f32 0.2 (0x3e4ccccd) at 0x1413B6C78 (' + DIS + ')', 'ShootFlowStart\'s blend-in (the other two pass -1: the clip\'s own)', UPD);
  add('cannon.voice_group', 0, 'int', UPD + ' after each Play: anim interface +0x1F0 with edx = 0 (0x1413B6CBC / 0x1413B6BAE / 0x1413B6AD5)', 'SetVoiceGroupNo(0) when the clip\'s length > 0', UPD);
  add('cannon.lift.height', 200, 'cm', UPD + ' sub 0: movss xmm1,[145C9C544h] f32 200 (0x43480000) at 0x1413B6CCA -> vt+0x398', 'LiftCheck 0x1413AD550 / LiftStep 0x1413C5400 (King of Hearts: 500)', UPD);
  const shots = C.fire.notifies.filter((n) => n.class === 'TresAnimNotify_Shot');
  must(shots.length === 1, 'M_5482: expected one TresShot, found ' + shots.length);
  add('cannon.fire.shot_time', shots[0].time, 's', 'M_5482 TresAnimNotify_Shot', 'the Shot (slot 69 0x1413B4A70) default branch: the shell (+0xB8) from weapon 0\'s ATK_01 along his facing', SHOT);
  add('cannon.fire.shot_location_index', Number(prop(shots[0], 'LocationIndex') || 0), 'int', 'M_5482 TresAnimNotify_Shot LocationIndex (unset = 0)', 'the muzzle: pawn vt+0x1410(0)', SHOT);
  for (const k of ['start', 'end']) must(!C[k].notifies.some((n) => n.class === 'TresAnimNotify_Shot'), C[k].sequence + ': unexpected TresShot');
  const change = C.end.notifies.find((n) => n.class === 'TresAnimNotify_EnableChange');
  must(change, 'ShootFlowEnd has no TresEnableChange');
  add('cannon.end.change', change.time, 's', 'ShootFlowEnd TresAnimNotify_EnableChange', 'm_bEnableChange back: Begin\'s weapon form comes back from here (sub 2)', UPD);

  // ---------------------------------------------------------------------------------------------- the cluster constants
  add('cannon.cluster.name', 'BZ_CANNON2', '-', CLU + ': FName L"BZ_CANNON2" (@146932930, 0x1413B40D8) -> pawn vt+0x1450', 'resolved by the held weapon\'s projectile set (w_so020 / w_so040 both list it)', CLU);
  add('cannon.cluster.pitch', 45, 'deg', CLU + ': FRotator(0x42340000 = 45.0, 0, 0) at 0x1413B402B (' + DISC + ')', 'the cone half angle: Quat(R) * Quat(pitch 45)', CLU);
  add('cannon.cluster.spin', 360, 'deg', CLU + ': movss xmm10,[145C9C550h] f32 360 at 0x1413B3E62, divss by +0x94 (N)', 'R.roll += 360 / N x i: the N directions around the shell\'s horizontal forward (i 0 at the top)', CLU);

  // ---------------------------------------------------------------------------------------------- the projectiles and rows
  for (const [kind, , id, , w, name] of KIND_FORM) {
    const set = projectileSet(w);
    const cls = set.get(name), two = set.get('BZ_CANNON2');
    must(cls && two, w + '_ProjSet: no ' + name + ' / BZ_CANNON2');
    const bp = blueprint('/Game/Blueprints/Projectile/ShootFlow/' + cls.replace(/_C$/, ''));
    const ex = String(bp.cdo.m_ReSpawnProjectileClass || 'None');
    must(/CannonEx_C$/.test(ex) && bp.cdo.m_bReSpawnProjectileOnTimeOut === true, cls + ': the respawn is ' + ex);
    must(bp.parent === 'TresProjectileBase', cls + ': parent ' + bp.parent);
    add('cannon.k' + kind + '.proj_class', cls, '-', w + '_ProjSet ' + name + ' (' + id + ')', 'the shell the Shot handler spawns (pawn vt+0x1450 -> the projectile set); kept at +0x118', SHOT);
    add('cannon.k' + kind + '.ex_class', ex, '-', cls + ' m_ReSpawnProjectileClass (m_bReSpawnProjectileOnTimeOut true)', 'the blast its Explode respawns (magic/projectile.rs respawn: the hit list taken over)', '-');
    add('cannon.k' + kind + '.cluster_class', two, '-', w + '_ProjSet BZ_CANNON2', 'the N bomblets of the cluster ' + CLU, CLU);
    const shape = ((bp.comps.TresAtkColl0 || {}).CollisionShapesSrc || [])[0] || {};
    const rowName = String(shape.m_DefaultAttackDataIDName || 'None');
    const a = ATTACK.get(rowName);
    must(a, 'p_ex001_AttackData has no ' + rowName);
    add('cannon.k' + kind + '.row', rowName, '-', cls + ' TresAtkColl0 m_DefaultAttackDataIDName (the Ex and the Cannon2 the same)', 'p_ex001_AttackData ' + rowName + ': power ' + a.m_Power + ', ' + String(a.m_DamageKind).split('::').pop() + ', ' + String(a.m_DamageAttribute).split('::').pop() + (a.m_bIsMagicAttack ? ', magic' : ''), '-');
  }

  // ---------------------------------------------------------------------------------------------- the muzzle (FK)
  // pawn vt+0x1410(0) (0x1405AA790 -> equipment vt+0x580): weapon 0's ATK_01; the launcher hangs on its unit's socket.
  const fk = require(path.join(__dirname, 'shotb', 'fk.js'))(lib);
  for (const [kind, , id] of KIND_FORM) {
    const units = FORM_OF[kind].units;
    must(units.length === 1, id + ': expected one weapon');
    const m = units[0].match(/^(\w+)@(\w+):(\w+)=(.*)$/);
    must(m, 'unit ' + units[0]);
    const socket = m[3] === 'None' ? 'R_wep_skt' : m[3], model = m[4].split('/').pop().replace(/_[Ww]ep$/, '');
    const tr = fk.track(C.fire.sequence, socket, model, [['ATK_01', [0, 0, 0]]]);
    add('cannon.k' + kind + '.muzzle.' + C.fire.sequence, tr.values, 'cm', 'FK: sequence ' + C.fire.sequence + ' (' + tr.source + ', root motion left out) -> ' + socket + ' -> ' + model + ' (' + (tr.weaponClip === '-' ? 'reference pose: it has no ' + C.fire.sequence + ' clip' : 'its clip ' + tr.weaponClip) + ') -> ATK_01 (sockets.json); tools/shotlocks/shotb/fk.js', 'pawn vt+0x1410(0) 0x1405AA790: weapon 0\'s ATK_01 in ACTOR space per frame (frame k at k x ' + tr.length.toFixed(4) + ' / ' + (tr.frames - 1) + ' s): [x, y, z] x ' + tr.frames, '0x1405AA790', 'confirmed', 'w0 = ' + model + ' (' + socket + ')');
  }
  return rows;
};
