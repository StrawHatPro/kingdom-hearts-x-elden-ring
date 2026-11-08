// (SHOT-A gate) Family `gate`: KH3's update 0x1413B7B40 (TresPlayerState_ShootFlow vtable 0x14693CA58 +0x4E0) and its
// spawner 0x1413C02D0 (+0x4E8) of kind 13 MI_GATE "Warp Trick" (Happy Gear's MAX kind) and SHOT-B's kind 19 BX_GATE
// "Zone Connector" (Nano Gear's MAX kind). Rows `gate.*` of sheets/port/sora-shotlocks.json for kh-core
// magic/shotlocks/gate.rs.
//
// THE CODE (decompiled C re/decomp/1413B/1413B7B40.c, re/decomp/1413C/1413C02D0.c; constants re-read in SHOT-B's listings
// build/sora_tmp/weapons/SHOT-B/spec/scripts/gate/dis_1413B7B40.txt / dis_1413C02D0.txt; SHOT-B's research
// build/sora_tmp/weapons/SHOT-B/spec/gate.md):
//   sub 0:   gravity off (pawn vt+0x1728(0)), Flying stopped (vt+0x17A0(5, 1)); ChangeWeapon pawn vt+0x13C8(weapon type
//            vt+0x12C8(-1), form 8 for kind 0x13 else 4, 0, 0, 0.0, 0, 0); Play "M_4481" (slot 0, blend -1, mode 2,
//            group 0 = a literal, rate 1), voice index 0 (anim vt+0x1F0(0)) when its length > 0; +0x84 |= 0x10 (face
//            request); sub 4 (no lift).
//   sub 3/4: at the clip's end (anim vt+0xC8(0)): Play "M_4482" (mode 3: loop), m_bEnableChange off (+0x84 &= ~1),
//            +0xCC |= 1 (the spawner on), sub 5, +0xA4 = 3.0, +0xA0 = 0.5.
//   sub 5:   +0xA4 -= dt; at <= 0: pawn vt+0x23C8() (0x1405A8610: EndEffect of the pawn's aura group 0xC62 / 0xC48),
//            Play "M_4483" (mode 2), m_bEnableChange off, sub 2, FUN_14140E500(ctrl, 0) (WorldSlow off).
//   other:   m_bEnableChange and +0xB0 != 0: Begin's weapon back (vt+0x1408, vt+0x13C8(type, +0xB0, +0xB1)), +0xB0 = 0;
//            at the clip's end Finish (vt+0x270).
//   tail:    every tick (after Finish too): +0xCC bit 0: +0x90 < +0x94 -> the spawner (tail jump vt+0x4E8), else the bit off.
//   spawner: +0xA0 gate; the locked entry +0x110 (ctrl+0xB08, stride 0x34; none / gone: +0x110 += 1, +0x114 = 0); weapon
//            out (vt+0x1330); up to 6 per call: R = (pitch u24 x 60/2^24 - 60, yaw u24 x 360/2^24 - 180, roll 0), off =
//            R.RotateVector(300, 0, 0), p = vt+0x1450(+0xB8, aim - off, aim - off, off / |off|, team, fire 1) (aim = the
//            entry component's vt+0xB88), p.vt+0x1100(+0x88, +0x8C), +0x90 += 1 (>= N: stop), +0x114 += 1 (>= the entry's
//            lock count +0xC: the next entry, stop); then +0xA0 = (+0x114 == 0 ? 0.1 : 0), +0xA4 = 0.5.
// THE DATA: the clips of the forms' anim sets (p_ex001_AnimSet4 / AnimSet8, build/sora_pack/keyblades/
// sora_forms_manifest.json), the weapon forms (sheets/port/sora-keyblade-forms.json), the ProjSets (w_so060 / w_so170), the
// gate blueprints' TresAtkColl0 rows and p_ex001_AttackData.
'use strict';

module.exports = (lib) => {
  const { row, clip, prop, blueprint, projectileSet, ATTACK, decomp, must, readJson, P, setName } = lib;
  const rows = [];
  const add = (...a) => rows.push(row(...a));
  const UPD = '0x1413B7B40';
  const SPN = '0x1413C02D0';
  const DIS = 'build/sora_tmp/weapons/SHOT-B/spec/scripts/gate/dis_1413B7B40.txt';
  const DIS_S = 'build/sora_tmp/weapons/SHOT-B/spec/scripts/gate/dis_1413C02D0.txt';
  const upd = decomp(UPD);
  const spn = decomp(SPN);
  must(upd && spn, 'no decompiled 0x1413B7B40 / 0x1413C02D0 in re/decomp');

  add('gate.update', UPD, '-', 'vtable 0x14693CA58 + 0x4E0 (Begin 0x1413A5CD0 cases 0xD / 0x13: delegate FUN_1404EFAE0 -> vt+0x4E0)', 'the update of kinds 13 (MI_GATE) and 19 (BX_GATE)', UPD);
  add('gate.spawner', SPN, '-', 'vtable 0x14693CA58 + 0x4E8 (tail jump at 0x1413B7EE8)', 'the gate spawner, called from the tail of every tick while +0xCC bit 0 and +0x90 < +0x94', SPN);
  add('gate.kinds', [13, 19], 'kind', 'sora-shotlocks kind.k13 / kind.k19 update 0x1413B7B40', 'the kinds this update runs (13 SHOT-A Warp Trick, 19 SHOT-B Zone Connector)', UPD);

  // ---------------------------------------------------------------------------------------------- the weapon forms
  must(/field_0x8c == 0x13\)/.test(upd) && /uVar8 = 8;/.test(upd) && /uVar8 = 4;/.test(upd), UPD + ': the weapon form switch changed');
  add('gate.weapon_form.kind', 19, 'kind', UPD + ' 0x1413B7DD7 cmp dword [rdi+8Ch],13h (' + DIS + ')', 'the kind that takes the other weapon form', UPD);
  add('gate.weapon_form.that_kind', 8, 'int', UPD + ' 0x1413B7DF5 mov r8b,8 -> ChangeWeapon form', 'TRES_WEAPON_FORM_08 = Nano Arms (form.GM, Nano Gear): kind 19', UPD);
  add('gate.weapon_form.other', 4, 'int', UPD + ' 0x1413B7DFC mov r8b,4 -> ChangeWeapon form', 'TRES_WEAPON_FORM_04 = Twin Yo-Yos (form.YO, Happy Gear): kind 13 and every other kind', UPD);
  const forms = readJson(P('sheets', 'port', 'sora-keyblade-forms.json'));
  const fr = forms.rows.map((r) => Object.fromEntries(forms.columns.map((c, i) => [c, r[i]])));
  const KINDS = [[13, 4, 'MI_GATE'], [19, 8, 'BX_GATE']];
  const SET = {};
  for (const [kind, form, name] of KINDS) {
    const f = fr.find((x) => x.weapon_form_value === form && x.weapon_type > 0);
    must(f && f.units.length && f.anim_sets.length === 1, 'sora-keyblade-forms: weapon form ' + form);
    SET[kind] = setName(f.anim_sets[0]);
    add('gate.k' + kind + '.weapon_form', form, 'int', UPD + ' sub 0: pawn vt+0x13C8(weapon type, ' + form + ', 0, 0, 0.0, 0, 0)', f.weapon_form + ' = ' + f.name + ' (' + f.id + ', ' + f.keyblade_name + '): its weapons and anim set ' + SET[kind] + '; Begin\'s +0xB0 / +0xB1 come back in End (0x1413ABA70)', UPD);
    add('gate.k' + kind + '.weapon_units', f.units, '-', 'sheets/port/sora-keyblade-forms ' + f.id + ' units (m_Equips of grow LVL01)', 'the parts the hosts draw instead of the Keyblade while PawnMagic::shot_weapon_form is ' + form, UPD);
    add('gate.k' + kind + '.anim_set', SET[kind], '-', 'sheets/port/sora-keyblade-forms ' + f.id + ' anim_sets', 'the set M_4481 / M_4482 / M_4483 resolve through (the main AnimSet has none)', UPD);
    // the projectile: the Keyblade's ProjSet (the form's projectile_set is the same asset)
    const ps = setName(f.projectile_set).replace(/_ProjSet$/, '');
    const cls = projectileSet(ps).get(name);
    must(cls, ps + '_ProjSet has no ' + name);
    add('gate.k' + kind + '.proj_class', cls, '-', 'Blueprints/Player/p_ex001/' + ps + '_ProjSet ' + name, 'the class the spawner spawns (+0xB8 ' + name + ' -> pawn vt+0x1450 -> the projectile set)', SPN);
    const bp = blueprint('/Game/Blueprints/Projectile/ShootFlow/' + cls.replace(/_C$/, ''));
    must(bp.parent === 'TresProjectileBase', cls + ': native parent ' + bp.parent);
    const shape = ((bp.comps.TresAtkColl0 || {}).CollisionShapesSrc || [])[0] || {};
    const rowName = String(shape.m_DefaultAttackDataIDName || 'None');
    const a = ATTACK.get(rowName);
    must(a, 'p_ex001_AttackData has no ' + rowName);
    add('gate.k' + kind + '.row', rowName, '-', cls + ' TresAtkColl0 CollisionShapesSrc[0].m_DefaultAttackDataIDName', 'p_ex001_AttackData ' + rowName + ': power ' + a.m_Power + ', ' + String(a.m_DamageKind).split('::').pop() + ', ' + String(a.m_DamageAttribute).split('::').pop() + (a.m_bIsMagicAttack ? ', magic' : '') + (a.m_bIsRapidFire ? ', rapid fire' : ''), '-');
  }

  // ---------------------------------------------------------------------------------------------- the clips
  // name, key, the instruction of its Play, the play mode (the [rsp+20h] argument)
  const CLIPS = [['M_4481', 'charge', '0x1413B7E2C (@146933DC8), sub 0', 2, '0x1413B7E76'], ['M_4482', 'loop', '0x1413B7C78 (@146933DD8), sub 3 / 4 at M_4481\'s end', 3, '0x1413B7CC0'], ['M_4483', 'end', '0x1413B7BB7 (@146933DE8), sub 5 when +0xA4 runs out', 2, '0x1413B7C01']];
  const fx = (n) => {
    // TresAnimNotify_AttachEffect: every MetaAttachData entry's effects with its socket and offsets.
    const out = [];
    for (const md of (prop(n, 'MetaAttachDataList') || {}).items || []) {
      const mad = md.find((p) => p.name === 'MetaAttachData');
      for (const d of ((mad && mad.value) || {}).items || []) {
        const get = (k) => (d.find((p) => p.name === k) || {}).value;
        const effs = ((get('EffectDataList') || {}).items || []).map((e) => (e.find((p) => p.name === 'AttachEffect') || {}).value).filter(Boolean).map((v) => String(v).split('.').pop());
        const l = get('LocationOffset') || {}, r = get('RotationOffset') || {};
        for (const e of effs) out.push(e + '@' + get('SocketName') + '(' + [l.x, l.y, l.z].join(',') + ')yaw' + r.yaw + (get('ToAttach') ? ' attached' : ' placed') + '@' + (+n.time).toFixed(4));
      }
    }
    return out;
  };
  for (const [name, key, at, mode, modeAt] of CLIPS) {
    add('gate.clip.' + key, name, '-', UPD + ' Play FName at ' + at, 'slot 0, blend -1, mode ' + mode + ', group 0 (a literal: [rsp+28h] = 0, not +0xC0), rate 1; resolved by the weapon form\'s anim set', UPD);
    add('gate.clip.' + key + '.mode', mode, 'int', UPD + ' ' + modeAt + ' mov dword [rsp+20h],' + mode, mode === 3 ? 'loop (bit 0) + root motion (bit 1): M_4482 repeats until the sub 5 timer runs out' : 'one-shot with root motion', UPD);
    for (const [kind] of KINDS) {
      const c = clip(name, SET[kind]);
      must(c, 'no clip ' + name + ' of ' + SET[kind]);
      for (const cls of ['TresAnimNotify_Shot', 'TresAnimNotifyState_Attack', 'TresAnimNotify_Timing', 'TresAnimNotify_EnableChange']) must(!c.notifies.some((n) => n.class === cls), c.sequence + ': ' + cls + ' (the shared handlers would act)');
      add('gate.k' + kind + '.clip.' + key + '.sequence', c.sequence, '-', SET[kind] + ' ' + name + ' -> sequence', 'the animation-table row (AnimId::A' + c.sequence + ')', UPD);
      add('gate.k' + kind + '.clip.' + key + '.length', c.length, 's', 'sequence ' + c.sequence + ' duration', key === 'loop' ? 'one loop of the pose' : 'the sub-step waits for its end', UPD);
      const effects = c.notifies.filter((n) => n.class === 'TresAnimNotify_AttachEffect').flatMap(fx);
      if (effects.length) add('gate.k' + kind + '.clip.' + key + '.effects', effects, '-', 'sequence ' + c.sequence + ' TresAnimNotify_AttachEffect (effect@socket(offset)yaw@time)', 'the clip\'s own effects (the generic notify path): the two portals beside him', '-');
    }
  }
  add('gate.clip_notifies_checked', ['TresAnimNotify_Shot', 'TresAnimNotifyState_Attack', 'TresAnimNotify_Timing', 'TresAnimNotify_EnableChange'], '-', 'the six sequences 4481..4483 / 8481..8483', 'none of them: the Shot handler 0x1413B4A70 (default branch for 13 / 19) never spawns, slot 64 / 105 never set +0xCC bit 0, m_bEnableChange stays off after sub 4 (Begin\'s weapon comes back in End)', '0x1413B4A70');

  // ---------------------------------------------------------------------------------------------- the constants
  const K = [
    ['gate.loop.timer', 3.0, 's', UPD, '0x1413B7CFE mov dword [rdi+0A4h],40400000h', 'sub 5\'s timer +0xA4 when M_4482 starts (the state ends by it when nothing spawns)'],
    ['gate.loop.first_wait', 0.5, 's', UPD, '0x1413B7D08 mov dword [rdi+0A0h],3F000000h', 'the spawner\'s wait +0xA0 before the first gate'],
    ['gate.spawn.keep', 0.5, 's', SPN, '0x1413C07A4 mov dword [rbx+0A4h],3F000000h', 'every spawning call: sub 5 ends this long after the last one'],
    ['gate.spawn.pause', 0.1, 's', SPN, '0x1413C0792 movss xmm9,[145C87D00h]', '+0xA0 after a call that finished a locked entry (+0x114 back to 0); else 0 (xmm9 = 0)'],
    ['gate.spawn.per_call', 6, 'count', SPN, '0x1413C076B inc edi; cmp edi,6; jl', 'at most this many gates per call (per tick)'],
    ['gate.spawn.distance', 300, 'cm', SPN, '0x1413C0561 mov qword [rsp+70h],43960000h -> FVector(300, 0, 0)', 'the gate starts aim - R.RotateVector((300, 0, 0)) and flies along it through the aim point'],
    ['gate.spawn.pitch_min', -60, 'deg', SPN, '0x1413C054E subss xmm0,xmm12 (xmm12 = [145C87D38h] = 60)', 'pitch = u24 x pitch_span / 2^24 + this (first draw): the gate is above the aim point, elevation 0..60 deg'],
    ['gate.spawn.pitch_span', 60, 'deg', SPN, '0x1413C0549 mulss xmm0,xmm11 (xmm11 = [146530DE8h] = 3.5762787e-06 = 60 / 2^24)', 'the pitch range'],
    ['gate.spawn.yaw_min', -180, 'deg', SPN, '0x1413C05BB subss xmm0,xmm14 (xmm14 = [145C9C540h] = 180)', 'yaw = u24 x yaw_span / 2^24 + this (second draw); roll 0 (FRotator::ZeroRotator 0x149B5C7A8)'],
    ['gate.spawn.yaw_span', 360, 'deg', SPN, '0x1413C05B6 mulss xmm0,xmm13 (xmm13 = [145D73AC4h] = 2.1457672e-05 = 360 / 2^24)', 'the yaw range'],
    ['gate.spawn.random_bits', 24, 'bits', SPN, '0x1413C0541 / 0x1413C05AE shr eax,8 (FUN_142CFF460 xorshift128 word >> 8)', 'each draw is the top 24 bits of a 32-bit word'],
    ['gate.spawn.fire', true, '-', SPN, '0x1413C06B0 mov byte [rsp+30h],1', 'pawn vt+0x1450 bFireNow: SetDirection, the fire effects at the spawn point, StartFire'],
  ];
  for (const [id, v, unit, ref, src, ev] of K) add(id, v, unit, ref + ' ' + src + ' (' + (ref === SPN ? DIS_S : DIS) + ')', ev, ref);
  add('gate.spawn.dir_min_len_sq', '1e-8', '-', SPN + ' 0x1413C062B comiss xmm2,xmm15 (xmm15 = [145C9C488h] = 9.9999999e-9) (' + DIS_S + ')', 'the direction is normalised when |off|^2 is above this (a text row: the sheet rounds numbers to 1e-6)', SPN);
  return rows;
};
