// (SHOT-A) Family `barrage`: KH3's kind 6 DW_BARRAGE "Diving Barrage" (Shooting Star's alternative MAX kind) and SHOT-B's
// kind 21 PO_BARRAGE "Hunny Drizzle" (Hunny Spout's), both run by TresPlayerState_ShootFlow update 0x1413B6320 (vtable
// 0x14693CA58 + 0x3D8). Rows `barrage.*` of sheets/port/sora-shotlocks.json for kh-core magic/shotlocks/barrage.rs.
//
// THE CODE (re/decomp/1413B/1413B6320.c, constants re-read in build/sora_tmp/weapons/SHOT-B/spec/scripts/dis_1413B6320.txt):
//   sub 0: gravity off (vt+0x1728(0)); Flying, stopped (vt+0x17A0(5, 1)); the weapon change vt+0x13C8(weapon type, 2 (kind
//          0x15: 0x10), 0, ...) -> Double Arrowguns' (Hunny Blasters') weapons and anim set; Play "M_2481" (slot 0, blend
//          -1, mode 2, group +0xC0, rate 1), voice when its length > 0; m_bFaceAt; sub 4; +0xE4 = (root location x, y,
//          centre z) (vt+0x6E0 = his centre); +0xD8 = the first component's point (vt+0xB88; none or pending kill: his
//          centre), +0xE0 += 1000 + his half height (vt+0x548); +0xF0 = max(0, |(+0xD8 - +0xE4).xy| - 50).
//   sub 4: at M_2481's end: vt+0x1010(1, 0) (body collision on), StopMovement, Play "M_2482", sub 5, +0xE0 -= 800; else
//          the velocity (vt+0x16B0): d = +0xD8 - centre; xy = 5 d.xy (|d.xy|^2 <= 2500: half his velocity's xy);
//          z = 5 d.z when d.z > 0 or d.z < -100, else 0.
//   sub 5: at M_2482's end: StopMovement, Play "M_2483", m_bEnableChange = 0, sub 2; else the velocity = his velocity
//          with z = 0.5 (+0xE0 - centre z) when that is < 0, else 0.
//   other: once m_bEnableChange (M_2483's TresEnableChange) and +0xB0 != 0: vt+0x1408(), the controller (FUN_14150DAF0)
//          -> vt+0x13C8(weapon type, +0xB0, +0xB1) (Begin's weapon form back), +0xB0 = 0; at the clip's end Finish
//          (vt+0x270). No FUN_14140E500 here: the aim mode is released by the common End.
//   The projectiles: M_2482's TresShot notifies -> the Shot handler 0x1413B4A70's default branch (kinds 6 / 0x15: a pair,
//   weapon[LocationIndex] then weapon[index ^ 1], each along (muzzle - centre), homing on locked[spawn mod locked]).
// THE DATA: the clips of p_ex001_AnimSet2 (Hunny Blasters' AnimSet16 has no M_248x: AnimSet2's play), the form rows of
// sheets/port/sora-keyblade-forms (form.DW / form.DW2: weapon form values, units), the weapons' FK muzzles (as
// tools/sora_form_arrowgun.js muzzleTrack: Sora's clip + the weapon socket + the weapon's own clip of the same name).
'use strict';
const path = require('path');

module.exports = (lib) => {
  const { row, clip, prop, ATTACK, must, readJson, P } = lib;
  const rows = [];
  const add = (...a) => rows.push(row(...a));
  const UPD = '0x1413B6320';
  const DIS = 'build/sora_tmp/weapons/SHOT-B/spec/scripts/dis_1413B6320.txt';

  add('barrage.update', UPD, '-', 'vtable 0x14693CA58 + 0x3D8 (Begin 0x1413A5CD0 cases 6 / 0x15)', 'the update of kinds 6 (DW_BARRAGE) and 0x15 (PO_BARRAGE)', UPD);
  add('barrage.kinds', [6, 21], 'kind', 'sora-shotlocks kind.k6 / kind.k21 update 0x1413B6320', 'the kinds this update runs (6 SHOT-A Diving Barrage, 21 SHOT-B Hunny Drizzle)', UPD);

  // ---------------------------------------------------------------------------------------------- the weapon forms
  const forms = readJson(P('sheets', 'port', 'sora-keyblade-forms.json'));
  const formRow = (id) => { const r = forms.rows.map((x) => Object.fromEntries(forms.columns.map((c, i) => [c, x[i]]))).find((x) => x.id === id); must(r, 'sora-keyblade-forms has no ' + id); return r; };
  const KIND_FORM = [[6, 2, 'form.DW', '0x1413B6710 mov r8b,2'], [21, 16, 'form.DW2', '0x1413B6709 mov r8b,10h (kind == 0x15, cmp at 0x1413B66EB)']];
  const FORM_OF = {};
  for (const [kind, value, id, at] of KIND_FORM) {
    const f = formRow(id);
    must(f.weapon_form_value === value, id + ': weapon form ' + f.weapon_form_value + ', the code puts on ' + value);
    FORM_OF[kind] = f;
    add('barrage.k' + kind + '.weapon_form', value, 'int', UPD + ' sub 0: pawn vt+0x13C8(weapon type, ' + value + ', 0, ...) at ' + at, f.weapon_form + ' = ' + f.name + ' (' + id + '): its weapons and anim sets ' + f.anim_sets.map(lib.setName).join(' + ') + '; Begin\'s +0xB0 / +0xB1 come back in sub 2 / End', UPD);
    add('barrage.k' + kind + '.weapon_units', f.units, '-', 'sheets/port/sora-keyblade-forms ' + id + ' units', 'the parts the hosts draw instead of the Keyblade while PawnMagic::shot_weapon_form is ' + value, UPD);
  }

  // ---------------------------------------------------------------------------------------------- the clips
  const CLIPS = [['rise', 'M_2481', '0x1413B6740 (@146933370), sub 0'], ['dive', 'M_2482', '0x1413B6553 (@146933380), sub 4 at M_2481\'s end'], ['land', 'M_2483', '0x1413B6438 (@146933390), sub 5 at M_2482\'s end']];
  const C = {};
  for (const [key, name, at] of CLIPS) {
    const c = clip(name, 'p_ex001_AnimSet2');
    must(c, 'no clip ' + name + ' of p_ex001_AnimSet2');
    must(!clip(name, 'p_ex001_AnimSet16') || clip(name, 'p_ex001_AnimSet16').sequence === c.sequence, name + ': AnimSet16 replaces it');
    C[key] = c;
    add('barrage.clip.' + key, name, '-', UPD + ' Play FName at ' + at, 'slot 0, blend -1, mode 2, group +0xC0, rate 1; p_ex001_AnimSet2 ' + name + ' (both weapon forms: AnimSet16 has none)', UPD);
    add('barrage.clip.' + key + '.sequence', c.sequence, '-', 'p_ex001_AnimSet2 ' + name + ' -> sequence', 'the animation-table row (AnimId::A' + c.sequence + ')', UPD);
    add('barrage.clip.' + key + '.length', c.length, 's', 'sequence ' + c.sequence + ' duration', 'the sub-step waits for its end', UPD);
  }
  const shots = C.dive.notifies.filter((n) => n.class === 'TresAnimNotify_Shot');
  must(shots.length > 0, 'M_2482 has no TresShot');
  add('barrage.dive.shot_times', shots.map((n) => n.time), 's', 'M_2482 TresAnimNotify_Shot times', 'each one is a Shot (slot 69 0x1413B4A70): a pair of ' + 'DW_BARRAGE / PO_BARRAGE while the spawn index < N', '0x1413B4A70');
  add('barrage.dive.shot_location_index', shots.map((n) => Number(prop(n, 'LocationIndex') || 0)), 'int', 'M_2482 TresAnimNotify_Shot LocationIndex (unset = 0)', 'the first weapon of each pair (the second is index ^ 1)', '0x1413B4A70');
  for (const k of ['rise', 'land']) must(!C[k].notifies.some((n) => n.class === 'TresAnimNotify_Shot'), C[k].sequence + ': unexpected TresShot');
  const change = C.land.notifies.find((n) => n.class === 'TresAnimNotify_EnableChange');
  must(change, 'M_2483 has no TresEnableChange');
  add('barrage.land.change', change.time, 's', 'M_2483 TresAnimNotify_EnableChange', 'm_bEnableChange back: Begin\'s weapon form comes back from here (sub 2)', UPD);

  // ---------------------------------------------------------------------------------------------- the constants
  const K = [
    ['barrage.up', 1000, 'cm', '0x1413B68CB addss f32 1000 (@145C87D44)', 'sub 0: the point above the first target: its component point (vt+0xB88) + this + his half height (vt+0x548)'],
    ['barrage.reach_margin', 50, 'cm', '0x1413B6929 subss f32 50 (@145C9C51C)', 'sub 0: +0xF0 = max(0, horizontal distance origin -> target - this); no reader of +0xF0 in the ShootFlow code (grep field_0xf0 over the 72 decompiled UTresPlayerState_ShootFlow functions of re/decomp: 0x1413AD3E0 zeroes it, 0x1413B6320 writes it; 0x1413BED50 is ShooterMode\'s own +0xF0)'],
    ['barrage.rise.near_sq', 2500, 'cm2', '0x1413B6629 comiss f32 2500 (@145C9C574)', 'sub 4: |d.xy|^2 <= this -> xy = his velocity\'s xy x barrage.rise.near_scale'],
    ['barrage.rise.near_scale', 0.5, '-', '0x1413B6671 / 0x1413B667C mulss f32 0.5 (@145C9C4D0)', 'sub 4: near: half his velocity\'s xy'],
    ['barrage.rise.gain', 5, '1/s', '0x1413B6632 / 0x1413B663A / 0x1413B66A4 mulss f32 5 (@145C87D20)', 'sub 4: velocity = 5 x (point - centre) (xy far; z when above, or below by more than barrage.rise.z_low)'],
    ['barrage.rise.z_low', -100, 'cm', '0x1413B6696 comiss f32 -100 (@145CADFE0)', 'sub 4: -100 <= d.z <= 0 -> z velocity 0'],
    ['barrage.dive.drop', 800, 'cm', '0x1413B65C7 subss f32 800 (@145C9C560)', 'sub 4 -> 5: the point\'s z drops by this (the dive ends 200 cm + his half height above the target\'s point)'],
    ['barrage.dive.gain', 0.5, '1/s', '0x1413B64FA mulss f32 0.5 (@145C9C4D0)', 'sub 5: z velocity = this x (point z - centre z) while below 0, else 0; xy = his velocity\'s (0 after StopMovement)'],
  ];
  for (const [id, v, unit, src, ev] of K) add(id, v, unit, UPD + ' ' + src + ' (' + DIS + ')', ev, UPD);

  // ---------------------------------------------------------------------------------------------- the projectiles and rows
  for (const [kind, , id] of KIND_FORM) {
    const f = FORM_OF[kind];
    const name = kind === 6 ? 'DW_BARRAGE' : 'PO_BARRAGE';
    const p = (f.projectiles || []).find((x) => x.startsWith(name + '='));
    must(p, id + ': no ' + name + ' in its projectile set');
    const cls = lib.setName(p.split('=')[1]) + '_C';
    add('barrage.k' + kind + '.proj_class', cls, '-', lib.setName(f.projectile_set) + ' ' + name, 'the class the Shot handler spawns (pawn vt+0x1450 -> the projectile set)', '0x1413B4A70');
    const bp = lib.blueprint('/Game/Blueprints/Projectile/ShootFlow/' + cls.replace(/_C$/, ''));
    const shape = ((bp.comps.TresAtkColl0 || {}).CollisionShapesSrc || [])[0] || {};
    const rowName = String(shape.m_DefaultAttackDataIDName || 'None');
    const a = ATTACK.get(rowName);
    must(a, 'p_ex001_AttackData has no ' + rowName);
    add('barrage.k' + kind + '.row', rowName, '-', cls + ' TresAtkColl0 m_DefaultAttackDataIDName', 'p_ex001_AttackData ' + rowName + ': power ' + a.m_Power + ', ' + String(a.m_DamageKind).split('::').pop() + ', ' + String(a.m_DamageAttribute).split('::').pop() + (a.m_bIsMagicAttack ? ', magic' : ''), '-');
  }

  // ---------------------------------------------------------------------------------------------- the muzzles (FK)
  // pawn vt+0x1410(i) (0x1405AA790 -> equipment vt+0x580): weapon[i]'s ATK_01 (MuzzleAttachPoint, the class default). As
  // tools/sora_form_arrowgun.js muzzleTrack (its FK, copied: that tool is a writer, not a module): Sora's clip (root motion
  // left out) + the weapon's socket on him + the weapon's skeleton posed by its own clip of the same logical name + ATK_01.
  const sp = require(path.join(lib.ROOT, 'tools', 'sora_pack.js'));
  const fs = require('fs');
  const qmulH = (a, b) => [a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1], a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0], a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3], a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]];
  const qrot = (q, v) => { const tx = 2 * (q[1] * v[2] - q[2] * v[1]), ty = 2 * (q[2] * v[0] - q[0] * v[2]), tz = 2 * (q[0] * v[1] - q[1] * v[0]); return [v[0] + q[3] * tx + (q[1] * tz - q[2] * ty), v[1] + q[3] * ty + (q[2] * tx - q[0] * tz), v[2] + q[3] * tz + (q[0] * ty - q[1] * tx)]; };
  const under = (c, pp) => { const w = qrot(pp.q, [pp.s[0] * c.t[0], pp.s[1] * c.t[1], pp.s[2] * c.t[2]]); return { q: qmulH(pp.q, c.q), t: [w[0] + pp.t[0], w[1] + pp.t[1], w[2] + pp.t[2]], s: [pp.s[0] * c.s[0], pp.s[1] * c.s[1], pp.s[2] * c.s[2]] }; };
  const point = (T, w) => { const r = qrot(T.q, [T.s[0] * w[0], T.s[1] * w[1], T.s[2] * w[2]]); return [r[0] + T.t[0], r[1] + T.t[1], r[2] + T.t[2]]; };
  const rotQ = (r) => { const h = Math.PI / 360, SP = Math.sin(r.pitch * h), CP = Math.cos(r.pitch * h), SY = Math.sin(r.yaw * h), CY = Math.cos(r.yaw * h), SR = Math.sin(r.roll * h), CR = Math.cos(r.roll * h); return [CR * SP * SY - SR * CP * CY, -CR * SP * CY - SR * CP * SY, CR * CP * SY - SR * SP * CY, CR * CP * CY + SR * SP * SY]; };
  const ID = lib.MAIN.identification;
  const SORA_BONES = sp.buildSkeleton(sp.readGlb(P('assets_converted', 'meshes', sp.CONTENT, sp.gamePathToContent(ID.skeletal_mesh) + '.glb')).json);
  const SORA_SOCKETS = {};
  for (const e of sp.loadPkg(sp.gamePathToContent(ID.skeleton)).exports.filter((x) => x.class === 'SkeletalMeshSocket')) {
    const pr = sp.propsOf(e);
    const loc = pr.RelativeLocation ? pr.RelativeLocation.value : { x: 0, y: 0, z: 0 }, rot = pr.RelativeRotation ? pr.RelativeRotation.value : { pitch: 0, yaw: 0, roll: 0 }, sc = pr.RelativeScale ? pr.RelativeScale.value : { x: 1, y: 1, z: 1 };
    SORA_SOCKETS[pr.SocketName.value] = { bone: pr.BoneName.value, T: { q: rotQ(rot), t: [loc.x, loc.y, loc.z], s: [sc.x, sc.y, sc.z] } };
  }
  const meshYaw = ID.mesh_component.relative_rotation_ue_deg.yaw * Math.PI / 180, cyaw = Math.cos(meshYaw), syaw = Math.sin(meshYaw);
  const actorSpace = (w) => [w[0] * cyaw - w[1] * syaw, w[0] * syaw + w[1] * cyaw, w[2]];
  const localT = (b, tr, time) => ({ q: tr.rotation ? sp.sampleChan(tr.rotation, time)(tr.rotation.ue_values) : b.ue.rotation, t: tr.translation ? sp.sampleChan(tr.translation, time)(tr.translation.ue_values) : b.ue.translation, s: tr.scale ? sp.sampleChan(tr.scale, time)(tr.scale.ue_values) : b.ue.scale });
  const byBone = (a) => { const m = new Map(); if (a) for (const c of a.chans) { if (!m.has(c.bone)) m.set(c.bone, {}); m.get(c.bone)[c.path] = c; } return m; };
  const boneT = (bones, chans, k, time, rootMotion) => { const chain = []; for (let i = k; i >= 0; i = bones[i].parent) chain.unshift(i); let T = null; for (const i of chain) { const l = localT(bones[i], (i === 0 && rootMotion) ? {} : (chans.get(i) || {}), time); T = T ? under(l, T) : l; } return T; };
  const ITEMS = new Map([...lib.MAIN.items, ...lib.OV.items].filter((i) => i.kind === 'animation').map((i) => [i.animation_name, i]));
  const soraAnim = (seq) => { const it = ITEMS.get(seq); return it && it.source_asset ? sp.loadAnimation(it.source_asset + '.' + path.basename(it.source_asset), SORA_BONES) : null; };
  const weaponOf = (model) => {
    const dir = P('build', 'sora_pack', 'keyblades', model);
    must(fs.existsSync(path.join(dir, 'model.json')), 'no weapon pack ' + model);
    const mj = readJson(path.join(dir, 'model.json')), sj = readJson(path.join(dir, 'sockets.json'));
    return { mj, sockets: sj.weapon_sockets || {}, bones: sp.buildSkeleton(sp.readGlb(P('assets_converted', 'meshes', sp.CONTENT, sp.gamePathToContent(mj.source_mesh) + '.glb')).json) };
  };
  const weaponAnim = (w, logical, seq) => { const c = w.mj.clips.find((x) => (x.logical_names || []).includes(logical)) || w.mj.clips.find((x) => x.sequence === seq); return c ? sp.loadAnimation(c.asset + '.' + c.sequence, w.bones) : null; };
  const partsOf = (f) => f.units.map((u) => { const m = u.match(/^(\w+)@(\w+):(\w+)=(.*)$/); must(m, 'unit ' + u); return { socket: m[3] === 'None' ? 'R_wep_skt' : m[3], model: m[4].split('/').pop().replace(/_[Ww]ep$/, '') }; });
  for (const [kind, , id] of KIND_FORM) {
    const parts = partsOf(FORM_OF[kind]);
    must(parts.length === 2, id + ': expected two weapons');
    const ws = parts.map((pt) => ({ ...pt, w: weaponOf(pt.model) }));
    for (const [key, name] of CLIPS) {
      const seq = C[key].sequence;
      const it = ITEMS.get(seq);
      const sa = soraAnim(seq);
      must(it && sa, seq + ': no Sora animation');
      const n = Math.max(2, it.num_frames || 2), out = [];
      const anims = ws.map((x) => weaponAnim(x.w, name, seq));
      for (let fr = 0; fr < n; fr++) {
        const t = fr * it.duration_s / (n - 1);
        ws.forEach((x, i) => {
          const sock = SORA_SOCKETS[x.socket];
          must(sock, 'no Sora socket ' + x.socket);
          const hand = SORA_BONES.findIndex((b) => b.name === sock.bone);
          const base = under(sock.T, boneT(SORA_BONES, byBone(sa), hand, Math.min(t, sa.length), sa.props.bEnableRootMotion === true));
          const s = x.w.sockets.ATK_01;
          const k = x.w.bones.findIndex((b) => b.name === (s ? s.bone : 'ATK_01'));
          must(k >= 0, x.model + ': no ATK_01');
          const a = anims[i];
          let T = under(boneT(x.w.bones, byBone(a), k, a ? Math.min(t, a.length) : 0, false), base);
          if (s) T = under({ q: rotQ({ pitch: s.local_rotation[0], yaw: s.local_rotation[1], roll: s.local_rotation[2] }), t: s.local_location, s: [1, 1, 1] }, T);
          out.push(...actorSpace(point(T, [0, 0, 0])).map((v) => Math.round(v * 10) / 10));
        });
      }
      add('barrage.k' + kind + '.muzzle.' + seq, out, 'cm', 'FK: sequence ' + seq + ' (' + name + ', root motion left out) + ' + ws.map((x) => x.model + ' on ' + x.socket + ' posed by its ' + name).join(', ') + ' + ATK_01 (sockets.json)', 'pawn vt+0x1410(i) 0x1405AA790: weapon[i]\'s ATK_01 in ACTOR space per frame (frame k at k x ' + it.duration_s.toFixed(4) + ' / ' + (n - 1) + ' s): [w0 x, y, z, w1 x, y, z] x ' + n, '0x1405AA790', 'confirmed', 'w0 = ' + ws[0].model + ' (' + parts[0].socket + '), w1 = ' + ws[1].model + ' (' + parts[1].socket + ')');
    }
  }
  return rows;
};
