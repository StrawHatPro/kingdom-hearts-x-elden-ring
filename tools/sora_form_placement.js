// (AUDIT-A PLACEMENT) WHERE KH3 PUTS THE FORM WEAPONS while their own clips move them off their sockets, as DATA for the
// hosts' form-part placement (kh-core combat/forms/placement.rs; drawn by kh-er sora.rs `submit` and the effects harness
// examples/effects_forms, the effect side kh-host vfx.rs):
//
//   parent / clip  TresAnimNotifyState_ChangeAttachParent of a form weapon's / transform unit's OWN clip (every
//                  build/sora_pack/keyblades/<model>/model.json `clips[].notifies`, every model of every form): for the
//                  window [begin, end) of that clip KH3 re-parents the weapon actor from its socket to Sora's
//                  `m_SocketName` (kh_root in every instance) and its own clip poses it from there. One row per (model,
//                  logical name of the pack's manifest `clips` map), in the renderer's `rig_of` order (the logical names
//                  sorted, as serde_json's map gives them): `parent` rows carry the window, `clip` rows are the model's other
//                  clips (so that the core picks the weapon's clip for Sora's sequence exactly as kh-er-render `rig_of`:
//                  a logical-name match first, then the file stem).
//   throw          TresAnimNotify_ThrowWeapon of Sora's clips (build/sora_pack/manifest.json + keyblades/
//                  sora_forms_manifest.json items): ::Notify 0x140561C20 -> the owner's anim interface vt+0x3C0 (pawn thunk
//                  0x1405B4680) -> the CURRENT STATE's interface (state+0x40) slot 120:
//                    TresPlayerState_Attack / ComboSH / ComboFL (+0x40 vtables 0x1469009F0 / 0x146911470 / 0x1469009F0):
//                    0x141345A60: velocity = pawn vt+0xE60 (its actor forward, 0x1405CEB90) x state+0x250 (Attack Reset
//                    0x14133D230: 6000; ComboSH Reset 0x141352E90 / ComboFL (ComboGM's) Reset 0x141352E20: 2000), unless
//                    the attack kind (+0x1B0) is 0xC6; pawn vt+0x13A8 (0x1405BDC80) -> equipment vt+0x550 (0x14178F2E0):
//                    every equipped weapon (m_ThrowWeaponName None = all) -> weapon vt+0x10C0 (0x14178F080): MyMovement
//                    InitialSpeed = |v|, Velocity = v, homing on (vt+0x590 0x1414826C0: the attack's target, state+0x118),
//                    the weapon plays m_WeaponAnimName (m_bWeaponAnimLoop, default true), weapon state 5 = FreeMove
//                    (0x141783270 -> begin 0x14177DB30: its root absolute at its world place, turned along its velocity
//                    when its movement has bRotationFollowsVelocity, the movement active). Recall: pawn vt+0x13B8(7) ->
//                    equipment vt+0x560(7) -> weapon vt+0x10D0 0x14177F050 (state 5 -> 0; FreeMove end 0x14177E920: back
//                    on its socket, relative 0), called by Attack End 0x141335120, ComboSH 0x141350750 (while M_B414
//                    plays) and slot 125 0x141366990 (before M_B415), ComboFL slot 125 0x141366630 (before M_F414).
//                    TresPlayerState_StyleFinish / StyleFinishDR (+0x40 vtables 0x146947F60 / 0x146948C00): slot 120 is
//                    0x14041F120 (an empty function): the notify does nothing (Drill Punch's 6421).
//                  One row per (form, grow level, part): Sora's socket of the part at the notify time in his ACTOR frame
//                  (his clip's forward kinematics, the root bone held when the clip has root motion: the actor moves), the
//                  part's own throw clip, its movement component (its blueprint's TresWeaponMovementComponent).
//
//   node tools/sora_form_placement.js   write sheets/port/sora-form-placement.json and register it (once, in place) in
//                                        tools/lib/sheet_defs.js and tools/lib/port_gen_defs.js; then node tools/gen_port.js
'use strict';
const fs = require('fs');
const path = require('path');
const ROOT = path.resolve(__dirname, '..');
const VERSION = '1.0.0';
const P = (...a) => path.join(ROOT, ...a);
const readJson = (f) => JSON.parse(fs.readFileSync(f, 'utf8').replace(/^﻿/, ''));
const must = (c, m) => { if (!c) throw new Error(m); };
const r4 = (x) => Math.round(x * 10000) / 10000;
const hex = (n) => '0x' + n.toString(16).toUpperCase();

// ------------------------------------------------------------------------------------------------ inputs
const sheet = (name) => { const s = readJson(P('sheets', 'port', name + '.json')); return s.rows.map((r) => Object.fromEntries(s.columns.map((c, i) => [c, r[i]]))); };
const FORMS = sheet('sora-keyblade-forms');
const KB = P('build', 'sora_pack', 'keyblades');
const KBM = readJson(path.join(KB, 'manifest.json'));
const MAIN = readJson(P('build', 'sora_pack', 'manifest.json'));
const OV = readJson(path.join(KB, 'sora_forms_manifest.json'));
const BP_DIR = P('assets_converted', 'packages', 'KINGDOM HEARTS III', 'Content');
const setName = (a) => String(a).split('/').pop().split('.')[0];
const modelOf = (bp) => String(bp).split('/').pop().replace(/_[Ww]ep$/, '');
// a form's parts: "Kind@HAND:socket=/Game/.../w_soNNN_Wep", grow units "G2:..." (the grow level's set)
const unitParse = (u) => { const m = String(u).match(/^(?:G(\d):)?(\w+)@(\w+):(\w+)=(.*)$/); return m ? { level: m[1] ? +m[1] : 1, kind: m[2], socket: m[4] === 'None' ? 'R_wep_skt' : m[4], bp: m[5], model: modelOf(m[5]) } : null; };
const formParts = (f) => [...(f.units || []), ...(f.grow_units || []), ...(f.transform_units || [])].map(unitParse).filter(Boolean);
// the forms (their commands) whose parts include a model
const cmdsOf = (model) => FORMS.filter((f) => formParts(f).some((p) => p.model === model)).map((f) => f.cmd).sort((a, b) => a - b);
const ROWS = [];
// (AUDIT-A THROW) the thrown parts' hit shapes (`tshape` rows), appended after every other row (no older row moves)
const TSHAPES = [];
const HAND_CHECKS = [];
const add = (r) => ROWS.push({ id: '-', what: '-', model: '-', cmds: [], logical: '-', clip: '-', names: [], length: 0, flags: [], values: [], ref_addr: '-', ref_conf: 'confirmed', note: '-', ...r });

// ------------------------------------------------------------------------------------------------ parent / clip
// Every keyblade model; the ones whose clips carry a ChangeAttachParent get all their clips as rows (rig_of's choice).
const PARENTED = [];
for (const model of fs.readdirSync(KB).filter((n) => /^w_/.test(n)).sort()) {
  const file = path.join(KB, model, 'model.json');
  if (!fs.existsSync(file)) continue;
  const mj = readJson(file);
  const windows = new Map(); // clip file stem -> [begin, end, socket]
  for (const c of mj.clips || []) {
    for (const n of (c.notifies || []).filter((x) => x.class === 'TresAnimNotifyState_ChangeAttachParent')) {
      const sock = (n.properties || {}).m_SocketName;
      must(sock && sock !== 'None', model + ' ' + c.sequence + ': a ChangeAttachParent without a socket');
      must(!windows.has(c.sequence), model + ' ' + c.sequence + ': two ChangeAttachParent windows (not handled)');
      windows.set(c.sequence, { begin: r4(n.trigger_time), end: r4(n.end_trigger_time), socket: sock, length: r4(c.duration_s || 0) });
    }
  }
  if (!windows.size) continue;
  const m = KBM.models[model];
  must(m && m.clips, model + ': not in the keyblades manifest');
  const cmds = cmdsOf(model);
  PARENTED.push({ model, windows: windows.size, cmds });
  // rig_of: the manifest's `clips` map (logical -> file) in serde_json's (sorted) key order
  for (const logical of Object.keys(m.clips).sort()) {
    const stem = path.basename(m.clips[logical], '.glb');
    const w = windows.get(stem);
    const clip = (mj.clips || []).find((c) => c.sequence === stem);
    if (w) {
      add({ id: 'parent.' + model + '.' + logical, what: 'parent', model, cmds, logical: w.socket, clip: stem, names: [logical], length: w.length, values: [w.begin, w.end],
        ref_addr: 'build/sora_pack/keyblades/' + model + '/model.json', note: 'the ' + model + ' clip ' + stem + ' (logical ' + logical + '): TresAnimNotifyState_ChangeAttachParent {m_SocketName ' + w.socket + '}: the weapon re-parented from its socket to Sora\'s ' + w.socket + ' for [' + w.begin + ', ' + w.end + ') s of its clip; forms ' + cmds.map(hex).join(' ') });
    } else {
      add({ id: 'clip.' + model + '.' + logical, what: 'clip', model, cmds, logical: '-', clip: stem, names: [logical], length: r4(clip ? clip.duration_s || 0 : 0),
        ref_addr: 'build/sora_pack/keyblades/manifest.json', note: 'the ' + model + ' clip ' + stem + ' (logical ' + logical + '): no ChangeAttachParent (rig_of\'s order for the model\'s parented clips)' });
    }
  }
}

// ------------------------------------------------------------------------------------------------ socket
// The new parents are SOCKETS of Sora's skeleton (kh_root: on bone kh_sk, RelativeRotation roll -90), not bones: one row each
// (the skeleton asset's SkeletalMeshSocket), so that the hosts compose the bone's pose with the socket's own transform.
{
  const sp = require('./sora_pack.js');
  const socks = new Map();
  for (const e of sp.loadPkg(sp.gamePathToContent(MAIN.identification.skeleton)).exports.filter((x) => x.class === 'SkeletalMeshSocket')) { const p = sp.propsOf(e); socks.set(p.SocketName.value, p); }
  for (const name of [...new Set(ROWS.filter((r) => r.what === 'parent').map((r) => r.logical))].sort()) {
    const p = socks.get(name);
    must(p, 'Sora\'s skeleton has no socket ' + name);
    const loc = p.RelativeLocation ? p.RelativeLocation.value : { x: 0, y: 0, z: 0 }, rot = p.RelativeRotation ? p.RelativeRotation.value : { pitch: 0, yaw: 0, roll: 0 };
    add({ id: 'socket.' + name, what: 'socket', logical: name, clip: p.BoneName.value, values: [loc.x, loc.y, loc.z, rot.pitch, rot.yaw, rot.roll].map(r4),
      ref_addr: MAIN.identification.skeleton, note: 'Sora\'s socket ' + name + ' (SkeletalMeshSocket of his skeleton asset): on bone ' + p.BoneName.value + '; values = [RelativeLocation x, y, z, RelativeRotation pitch, yaw, roll]' });
  }
}

// ------------------------------------------------------------------------------------------------ throw
{
  const sp = require('./sora_pack.js');
  const qmulH = (a, b) => [a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1], a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0], a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3], a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]];
  const qrot = (q, v) => { const tx = 2 * (q[1] * v[2] - q[2] * v[1]), ty = 2 * (q[2] * v[0] - q[0] * v[2]), tz = 2 * (q[0] * v[1] - q[1] * v[0]); return [v[0] + q[3] * tx + (q[1] * tz - q[2] * ty), v[1] + q[3] * ty + (q[2] * tx - q[0] * tz), v[2] + q[3] * tz + (q[0] * ty - q[1] * tx)]; };
  const under = (c, p) => { const v = qrot(p.q, [p.s[0] * c.t[0], p.s[1] * c.t[1], p.s[2] * c.t[2]]); return { q: qmulH(p.q, c.q), t: [v[0] + p.t[0], v[1] + p.t[1], v[2] + p.t[2]], s: [p.s[0] * c.s[0], p.s[1] * c.s[1], p.s[2] * c.s[2]] }; };
  const rotQ = (r) => { const h = Math.PI / 360, SP = Math.sin(r.pitch * h), CP = Math.cos(r.pitch * h), SY = Math.sin(r.yaw * h), CY = Math.cos(r.yaw * h), SR = Math.sin(r.roll * h), CR = Math.cos(r.roll * h); return [CR * SP * SY - SR * CP * CY, -CR * SP * CY - SR * CP * SY, CR * CP * SY - SR * SP * CY, CR * CP * CY + SR * SP * SY]; };
  const ID = MAIN.identification;
  const SORA_BONES = sp.buildSkeleton(sp.readGlb(P('assets_converted', 'meshes', sp.CONTENT, sp.gamePathToContent(ID.skeletal_mesh) + '.glb')).json);
  const SORA_SOCKETS = {};
  for (const e of sp.loadPkg(sp.gamePathToContent(ID.skeleton)).exports.filter((x) => x.class === 'SkeletalMeshSocket')) {
    const p = sp.propsOf(e);
    const loc = p.RelativeLocation ? p.RelativeLocation.value : { x: 0, y: 0, z: 0 }, rot = p.RelativeRotation ? p.RelativeRotation.value : { pitch: 0, yaw: 0, roll: 0 }, sc = p.RelativeScale ? p.RelativeScale.value : { x: 1, y: 1, z: 1 };
    SORA_SOCKETS[p.SocketName.value] = { bone: p.BoneName.value, T: { q: rotQ(rot), t: [loc.x, loc.y, loc.z], s: [sc.x, sc.y, sc.z] } };
  }
  const yaw = ID.mesh_component.relative_rotation_ue_deg.yaw * Math.PI / 180, cy = Math.cos(yaw), sy = Math.sin(yaw);
  const actor = (v) => [v[0] * cy - v[1] * sy, v[0] * sy + v[1] * cy, v[2]];
  const MESH_YAW_Q = [0, 0, Math.sin(yaw / 2), Math.cos(yaw / 2)];
  const localT = (b, tr, time) => ({ q: tr.rotation ? sp.sampleChan(tr.rotation, time)(tr.rotation.ue_values) : b.ue.rotation, t: tr.translation ? sp.sampleChan(tr.translation, time)(tr.translation.ue_values) : b.ue.translation, s: tr.scale ? sp.sampleChan(tr.scale, time)(tr.scale.ue_values) : b.ue.scale });
  const byBone = (a) => { const m = new Map(); if (a) for (const c of a.chans) { if (!m.has(c.bone)) m.set(c.bone, {}); m.get(c.bone)[c.path] = c; } return m; };
  const boneT = (bones, chans, k, time, rootMotion) => { const chain = []; for (let i = k; i >= 0; i = bones[i].parent) chain.unshift(i); let T = null; for (const i of chain) { const l = localT(bones[i], (i === 0 && rootMotion) ? {} : (chans.get(i) || {}), time); T = T ? under(l, T) : l; } return T; };
  // THE STATE THAT PLAYS EACH THROW CLIP and its interface slot 120 (see the header); speed = its +0x250 after Reset
  const STATE = {
    B413: { state: 'ComboSH', handler: '0x141345A60', speed: 2000, reset: '0x141352E90', recall: '0x141366990 (slot 125, before M_B415) / 0x141350750 (while M_B414) / End 0x141335120', play: 'slot 125 0x141366990 (the raid M_B413 -> M_B414 -> M_B415)' },
    F412: { state: 'ComboFL', handler: '0x141345A60', speed: 2000, reset: '0x141352E20', recall: '0x141366630 (slot 125, before M_F414) / End 0x141335120', play: 'slot 125 0x141366630 (the flag throw M_F412 -> M_F413 -> M_F414)' },
    6421: { state: 'StyleFinish', handler: '0x14041F120 (empty)', speed: 0, reset: '-', recall: '-', play: 'StyleFinish slot 148 0x1413DF1F0 (FINISH_DR M_6421)' },
    1297: { state: 'Attack', handler: '0x141345A60', speed: 6000, reset: '0x14133D230', recall: 'End 0x141335120 / interface slot 105 0x141345BD0 (kind 0x29)', play: 'PlayAttack kind 41 (BackRaid, the Kingdom Key: not a form)' },
  };
  const items = [...MAIN.items, ...OV.items].filter((i) => i.kind === 'animation');
  const movementOf = (bp) => {
    const f = path.join(BP_DIR, bp.replace(/^\/Game\//, '') + '.uasset.json');
    if (!fs.existsSync(f)) return null;
    const ex = (readJson(f).exports || []).find((e) => e.class === 'TresWeaponMovementComponent');
    if (!ex) return { none: true };
    const pr = Object.fromEntries(((ex.data || {}).props || []).map((p) => [p.name, p.value]));
    return { homing: pr.m_bEnableHoming === true, delay: pr.m_HomingStartDelay || 0, turn: pr.m_HomingTurnMax || 0, max: pr.MaxSpeed || 0, follow: pr.bRotationFollowsVelocity === true };
  };
  // (AUDIT-A THROW) THE THROWN PART'S HIT SHAPES. TresAnimNotifyState_Attack begin 0x140561F40 -> the pawn's interface slot 64
  // (base 0x1405B3FE0): Activate 0x140625500 on EVERY equipped weapon's MyAtkColl (weapon+0x578, the weapon actor's own
  // TresAtkColl0) and on the pawn's; each primitive's Update 0x140647950 places its shape at SocketTransform 0x14064BA60 of
  // the component's attach parent (NORMAL: the weapon mesh's socket). A thrown weapon is not detached (FreeMove 0x14177DB30:
  // its root absolute at its world place), so its shapes fly with it, posed by its own throw clip. One `tshape` row per
  // (throw row, attack group of Sora's throw / flight clips that the part's blueprint has): per frame of the part's throw
  // clip (its reference pose when it has none) [ax, ay, az, bx, by, bz, radius] in the THROWN PART'S frame (kh-core
  // placement::ThrownWeapon position / rotation: the frame the part hangs in on his socket, as the in-hand `track` rows of
  // sora-form-shield / sora-form-ship compose it).
  const shapesOf = (bp) => {
    const d = readJson(path.join(BP_DIR, bp.replace(/^\/Game\//, '') + '.uasset.json'));
    const flat = (props) => { const o = {}; for (const p of props || []) o[p.name] = p.type === 'StructProperty' ? (Array.isArray(p.value) ? flat(p.value) : p.value) : p.type === 'ArrayProperty' ? (p.value && p.value.items ? p.value.items.map((x) => (Array.isArray(x) ? flat(x) : x)) : p.value) : p.value; return o; };
    const out = [];
    for (const e of d.exports.filter((x) => x.class === 'TresAtkCollComponent')) {
      for (const s of flat((e.data && e.data.props) || e.props).CollisionShapesSrc || []) {
        const v = (o) => (o ? [o.x || 0, o.y || 0, o.z || 0] : [0, 0, 0]);
        out.push({ group: s.m_GrpName, shape: String(s.m_ShapeType).split('::').pop(), size: v(s.m_Size), socket1: s.m_SocketName1 || 'None', socket2: s.m_SocketName2 || 'None', loc: v(s.m_RelativeLocation1), attach: String(s.m_AttachType1 || 'TRES_ATTACH_NORMAL').split('::').pop(), abs1: s.m_bAbsoluteOffset1 === true });
      }
    }
    return out;
  };
  const weapons = new Map();
  const weaponOf = (model) => {
    if (!weapons.has(model)) {
      const dir = path.join(KB, model), mj = readJson(path.join(dir, 'model.json')), sj = readJson(path.join(dir, 'sockets.json'));
      weapons.set(model, { mj, sockets: sj.weapon_sockets || {}, bones: sp.buildSkeleton(sp.readGlb(P('assets_converted', 'meshes', sp.CONTENT, sp.gamePathToContent(mj.source_mesh) + '.glb')).json) });
    }
    return weapons.get(model);
  };
  // the clip's tracks of bones the mesh lacks (w_so032's `Center`: the pack drops them, model.json
  // dropped_tracks_of_bones_not_in_this_mesh) become tracks of unattached stand-ins, so that they pose nothing, as in the pack
  const weaponAnim = (w, c) => {
    if (!c) return null;
    const bones = [...w.bones];
    for (;;) {
      try {
        return sp.loadAnimation(c.asset + '.' + c.sequence, bones);
      } catch (e) {
        const m = String(e.message).match(/^animation bone (\S+) not in the mesh skeleton/);
        must(m && (c.dropped_tracks_of_bones_not_in_this_mesh || []).includes(m[1]), c.asset + ': ' + e.message);
        bones.push({ name: m[1], parent: -1, ue: { rotation: [0, 0, 0, 1], translation: [0, 0, 0], scale: [1, 1, 1] } });
      }
    }
  };
  const point = (T, v) => { const r = qrot(T.q, [T.s[0] * v[0], T.s[1] * v[1], T.s[2] * v[2]]); return [r[0] + T.t[0], r[1] + T.t[1], r[2] + T.t[2]]; };
  // a socket of the weapon mesh in the mesh's frame under its clip `a` at `time` (as sora_form_shield.js socketAt, without his hand)
  const weaponSocket = (w, a, name, time) => {
    const ws = w.sockets[name];
    const k = w.bones.findIndex((b) => b.name === (ws ? ws.bone : name));
    if (k < 0) return null;
    let T = boneT(w.bones, byBone(a), k, a ? Math.min(time, a.length) : 0, false);
    if (ws) T = under({ q: rotQ({ pitch: ws.local_rotation[0], yaw: ws.local_rotation[1], roll: ws.local_rotation[2] }), t: ws.local_location, s: [1, 1, 1] }, T);
    return T;
  };
  // Update 0x140647950: end 1 = T1.t + (abs1 ? loc x T1.s : T1.q (loc x T1.s)); CAPSULE2's end 2 = T2.t; SPHERE: end 1 twice
  const shapeIn = (w, a, s, time, id) => {
    must(s.attach === 'TRES_ATTACH_NORMAL' || s.attach === 'NORMAL', id + ': attach ' + s.attach + ' (only the weapon mesh\'s sockets fly with it)');
    must(s.shape === 'SPHERE' || s.shape === 'CAPSULE2', id + ': shape ' + s.shape + ' (not handled on a thrown part)');
    const T1 = weaponSocket(w, a, s.socket1, time);
    must(T1, id + ': no socket ' + s.socket1);
    const sl = [s.loc[0] * T1.s[0], s.loc[1] * T1.s[1], s.loc[2] * T1.s[2]];
    const off = s.abs1 ? sl : qrot(T1.q, sl);
    const e1 = [T1.t[0] + off[0], T1.t[1] + off[1], T1.t[2] + off[2]];
    if (s.shape === 'SPHERE' || s.socket2 === 'None') return [...e1, ...e1, s.size[0]];
    const T2 = weaponSocket(w, a, s.socket2, time);
    must(T2, id + ': no socket ' + s.socket2);
    return [...e1, ...T2.t, s.size[0]];
  };
  const groupsOf = (seqName) => { const x = items.find((i) => i.animation_name === seqName); return x ? (x.notifies || []).filter((n) => n.class === 'TresAnimNotifyState_Attack').map((n) => String((n.properties || {}).CollisionGrpName || 'None')) : []; };
  // the same composition under his hand (Sora's socket under his clip x the part's clip of his sequence): checked against the
  // in-hand `track` rows the forms' generators wrote, so that the flying shapes are the in-hand ones on the flying part
  // (HAND_CHECKS: top level, next to TSHAPES)
  const seen = new Set();
  for (const it of items) {
    for (const n of (it.notifies || []).filter((x) => x.class === 'TresAnimNotify_ThrowWeapon')) {
      const seq = it.animation_name;
      if (seen.has(seq)) continue;
      seen.add(seq);
      const st = STATE[seq];
      must(st, 'a ThrowWeapon in ' + seq + ': which state plays it? (add it to STATE)');
      const np = n.properties || {};
      const wanim = np.m_WeaponAnimName && np.m_WeaponAnimName !== 'None' ? np.m_WeaponAnimName : '';
      const loop = np.m_bWeaponAnimLoop === undefined ? true : np.m_bWeaponAnimLoop === true;
      must(!np.m_ThrowWeaponName || np.m_ThrowWeaponName === 'None', seq + ': m_ThrowWeaponName ' + np.m_ThrowWeaponName + ' (only None = every weapon is handled)');
      const t0 = +n.trigger_time;
      // the forms whose anim sets carry this clip (the last set of the form that names it: its replacement order)
      const forms = FORMS.filter((f) => (f.anim_sets || []).some((a) => OV.logical_animations.some((x) => setName(x.anim_set) === setName(a) && x.clip === seq)));
      if (!forms.length) {
        add({ id: 'throw.base.' + seq, what: 'throw', model: '-', cmds: [], logical: 'R_wep_skt', clip: seq, names: [wanim || '-'], flags: ['1', 'follow', loop ? 'loop' : 'once', st.state, st.speed > 0 ? 'base' : 'ignored', '-'], values: [r4(t0), st.speed, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
          ref_addr: '0x140561C20 ' + st.handler, note: 'Sora\'s ' + seq + ' (' + (it.logical_names || []).join('/') + ', ' + st.play + '): TresAnimNotify_ThrowWeapon at ' + r4(t0) + ' s, the hand keyblade (no form: the base moveset; not a form part, the hosts\' hand keyblade): state ' + st.state + ', speed ' + st.speed + ' (Reset ' + st.reset + '), recall ' + st.recall });
        continue;
      }
      const sa = sp.loadAnimation(it.source_asset + '.' + path.basename(it.source_asset), SORA_BONES);
      const rootMotion = sa.props.bEnableRootMotion === true;
      for (const f of forms) {
        const setsOf = (f.anim_sets || []).map(setName);
        const soraOf = (logical) => { let c = null; for (const s of setsOf) for (const x of OV.logical_animations.filter((y) => setName(y.anim_set) === s && y.name === logical)) c = x.clip; return c; };
        const flight = wanim ? soraOf(wanim) || '-' : '-';
        // the equipped weapons (m_WeaponEquips: the units of the grow level; the transform units only play the transformation)
        const parts = [...(f.units || []), ...(f.grow_units || [])].map(unitParse).filter(Boolean);
        const levels = [...new Set(parts.map((p) => p.level))].sort();
        for (const level of [1, 2, 3]) {
          const at = levels.includes(level) ? parts.filter((p) => p.level === level) : level === 1 ? [] : parts.filter((p) => p.level === 1);
          for (const part of at) {
            const sock = SORA_SOCKETS[Object.keys(SORA_SOCKETS).find((s) => s.toLowerCase() === part.socket.toLowerCase())];
            must(sock, f.id + ' ' + part.model + ': Sora has no socket ' + part.socket);
            const bi = SORA_BONES.findIndex((b) => b.name === sock.bone);
            const T = under(sock.T, boneT(SORA_BONES, byBone(sa), bi, Math.min(t0, sa.length), rootMotion));
            const pos = actor(T.t), q = qmulH(MESH_YAW_Q, T.q);
            const mv = movementOf(part.bp);
            must(mv, f.id + ' ' + part.model + ': no blueprint ' + part.bp);
            const wj = readJson(path.join(KB, part.model, 'model.json'));
            const wclip = wanim ? (wj.clips || []).find((c) => (c.logical_names || []).includes(wanim)) : null;
            const speed = st.speed > 0 && !mv.none ? st.speed : 0;
            add({ id: 'throw.' + f.id.replace(/^form\./, '') + '.' + level + '.' + seq + '.' + part.model, what: 'throw', model: part.model, cmds: [f.cmd], logical: part.socket, clip: seq, names: [wanim || '-', wclip ? wclip.sequence : '-', flight], length: r4(wclip ? wclip.duration_s || 0 : 0),
              flags: [String(level), mv.follow ? 'follow' : 'keep', loop ? 'loop' : 'once', st.state, st.speed > 0 ? 'thrown' : 'ignored', mv.none ? 'no_movement' : mv.homing ? 'homing' : 'straight'],
              values: [r4(t0), speed, r4(mv.delay || 0), r4(mv.turn || 0), r4(mv.max || 0), ...pos.map((v) => r4(v)), ...q.map((v) => Math.round(v * 1e6) / 1e6)],
              ref_addr: '0x140561C20 ' + st.handler + ' 0x14178F080 0x14177DB30', note: f.name + ' (' + hex(f.cmd) + ') grow ' + level + ': Sora\'s ' + seq + ' TresAnimNotify_ThrowWeapon at ' + r4(t0) + ' s throws ' + part.model + ' (' + part.kind + ' on ' + part.socket + ', ' + part.bp + '): state ' + st.state + ' (' + st.play + '), slot 120 ' + st.handler + ', speed ' + speed + ' cm/s along his forward (+0x250 after Reset ' + st.reset + ')' + (mv.none ? ' (no movement component: it stays where it left his hand)' : '') + '; its movement ' + JSON.stringify(mv) + '; it plays ' + (wanim || 'nothing new') + (wclip ? ' (' + wclip.sequence + ', ' + r4(wclip.duration_s) + ' s, ' + (loop ? 'looped' : 'once') + ')' : '') + ' while Sora plays ' + seq + ' / ' + flight + '; recall ' + st.recall + '. values = [notify time, speed, homing delay s, turn max deg/s, max speed, the socket in his actor frame x, y, z (cm, from his origin), its rotation quaternion x, y, z, w (UE)]' });
            // (AUDIT-A THROW) the part's hit shapes on the flying part (see `shapesOf` above): appended after every other row
            if (st.speed > 0) {
              const throwId = 'throw.' + f.id.replace(/^form\./, '') + '.' + level + '.' + seq + '.' + part.model;
              const w = weaponOf(part.model), a = weaponAnim(w, wclip);
              const frames = wclip ? Math.max(wclip.frames || Math.round((wclip.duration_s || 0) * 30) + 1, 1) : 1;
              const len = wclip ? wclip.duration_s || 0 : 0;
              const bpShapes = shapesOf(part.bp);
              for (const g of [...new Set([...groupsOf(seq), ...groupsOf(flight)])]) {
                const list = bpShapes.filter((s) => s.group === g);
                if (!list.length) continue;
                must(list.length === 1, throwId + ' ' + g + ': ' + list.length + ' shapes in one group (not handled on a thrown part)');
                const s = list[0], id = 'tshape.' + throwId.replace(/^throw\./, '') + '.' + g;
                const vals = [];
                for (let k = 0; k < frames; k++) vals.push(...shapeIn(w, a, s, frames > 1 ? k * len / (frames - 1) : 0, id).map((v) => Math.round(v * 10) / 10));
                TSHAPES.push({ id, what: 'tshape', model: part.model, cmds: [f.cmd], logical: g, clip: seq, names: [throwId, wclip ? wclip.sequence : '-', flight], length: r4(len), flags: [String(level), s.shape, s.socket1 + '-' + s.socket2, String(frames)], values: vals,
                  ref_addr: '0x140561F40 0x1405B3FE0 0x140625500 0x140647950 0x14064BA60', note: f.name + ' grow ' + level + ': ' + part.model + '\'s TresAtkColl0 group ' + g + ' (' + s.shape + ' ' + s.socket1 + '-' + s.socket2 + ', size ' + s.size[0] + ', loc ' + s.loc.join('/') + ') while it is thrown (' + throwId + '): KH3 activates the group on every equipped weapon\'s own component, whose shapes hang on the weapon mesh: per frame of its throw clip ' + (wclip ? wclip.sequence : '(none: its reference pose)') + ' [ax, ay, az, bx, by, bz, radius] cm in the thrown part\'s frame (kh-core placement::ThrownWeapon position / rotation)' });
                // the in-hand check (grow 1: the forms' track rows are of the level-1 parts)
                if (level === 1) {
                  const own = (wj.clips || []).find((c) => (c.logical_names || []).some((n) => (it.logical_names || []).includes(n))) || (wj.clips || []).find((c) => c.sequence === seq);
                  HAND_CHECKS.push({ track: 'track.' + f.id.replace(/^form\./, '') + '.' + seq + '.' + g, frames: it.num_frames, length: it.duration_s, at: (t) => {
                    const hand = under(sock.T, boneT(SORA_BONES, byBone(sa), bi, Math.min(t, sa.length), rootMotion));
                    const v = shapeIn(w, weaponAnim(w, own), s, t, id);
                    return [...actor(point(hand, v.slice(0, 3))), ...actor(point(hand, v.slice(3, 6))), v[6]];
                  } });
                }
              }
            }
          }
        }
      }
    }
  }
}

// ------------------------------------------------------------------------------------------------ (AUDIT-A THROW) tshape
{
  // the in-hand composition equals the forms' own in-hand tracks (sora-form-shield / sora-form-ship `track` rows): the same
  // bones, sockets and shapes, so the flying rows differ from them only by the part's frame
  const tracks = new Map();
  for (const f of fs.readdirSync(P('sheets', 'port')).filter((n) => /^sora-form-.*\.json$/.test(n) && n !== 'sora-form-placement.json')) for (const r of sheet(f.replace(/\.json$/, ''))) if (r.what === 'track') tracks.set(r.id, r);
  let checked = 0;
  for (const c of HAND_CHECKS) {
    const t = tracks.get(c.track);
    if (!t) continue;
    const n = t.code;
    for (let k = 0; k < n; k++) {
      const want = t.values.slice(k * 7, k * 7 + 7), got = c.at(n > 1 ? k * c.length / (n - 1) : 0);
      const d = Math.max(...want.map((v, i) => Math.abs(v - got[i])));
      must(d < 1.5, c.track + ' frame ' + k + ': the in-hand composition is ' + d.toFixed(2) + ' cm off the track row (' + got.map((v) => v.toFixed(1)) + ' vs ' + want + ')');
    }
    checked += 1;
  }
  must(checked >= 3, 'expected the in-hand checks of SH / SH2 B413 and FL F412, got ' + checked);
  for (const r of TSHAPES) add(r);
  console.log('  tshape: ' + TSHAPES.length + ' rows, ' + checked + ' in-hand checks against the forms\' track rows');
}

// ------------------------------------------------------------------------------------------------ checks
must(ROWS.filter((r) => r.what === 'parent').length === 18, 'expected 18 ChangeAttachParent windows (w_so024 5417, w_so044 H417, w_so072 A416, w_so172 x15), got ' + ROWS.filter((r) => r.what === 'parent').length);
for (const [id, cmd] of [['parent.w_so024.M_5417', 0x83], ['parent.w_so044.M_5417', 0x8F], ['parent.w_so072.M_A416', 0x88], ['parent.w_so172.M_8411', 0x86]]) must(ROWS.some((r) => r.id === id && r.cmds.includes(cmd)), id + ' of form ' + hex(cmd));
must(ROWS.some((r) => r.what === 'throw' && r.model === 'w_so032' && r.clip === 'B413' && r.flags[4] === 'thrown'), 'Counter Shield\'s raid throw');
must(ROWS.some((r) => r.what === 'throw' && r.model === 'w_so142' && r.clip === 'B413' && r.flags[4] === 'thrown'), 'Frying Pan\'s raid throw');
must(ROWS.some((r) => r.what === 'throw' && r.model === 'w_so093' && r.clip === 'F412' && r.flags[4] === 'thrown'), 'Storm Flag\'s flag throw');

// ------------------------------------------------------------------------------------------------ sheet
const S = (n, t, g, doc) => [n, t, g, doc];
const COLS = [
  S('id', 'str', 'str', '<what>.<model>.<logical> (parent / clip), throw.<form>.<grow>.<sequence>.<model>'), S('what', 'enum:parent|clip|socket|throw|tshape' /* (AUDIT-A THROW) tshape */, 'str', 'the row kind'),
  S('model', 'str', 'str', 'the weapon model (the pack\'s keyblades/<model>)'), S('cmds', 'list', 'i32[]', 'the form commands whose parts include the model (sora-keyblade-forms units / grow units / transform units)'),
  S('logical', 'str', 'str', 'Sora\'s socket: the new parent (parent), the part\'s own socket (throw), the socket (socket)'), S('clip', 'str', 'str', 'the weapon clip\'s file stem (parent / clip), Sora\'s sequence with the notify (throw), the socket\'s bone (socket)'),
  S('names', 'list', 'str[]', 'the logical name of the weapon clip (parent / clip); [the weapon\'s throw clip logical name, its sequence, Sora\'s sequence of that logical name] (throw)'),
  S('length', 'num', 'f32', 'the weapon clip length, s'), S('flags', 'list', 'str[]', 'throw: [grow level, follow | keep (bRotationFollowsVelocity), loop | once, the state class, thrown | base | ignored, homing | straight | no_movement]'),
  S('values', 'list', 'f32[]', 'parent: [begin, end] s of the weapon clip; socket: [location x, y, z, rotation pitch, yaw, roll]; throw: see the note'),
  S('ref_addr', 'str', 'str', 'the data file / decompiled functions'), S('ref_conf', 'enum:confirmed|inferred|guess', 'str', 'confirmed = read in the code or the data'), S('note', 'str', 'str', '-'),
];
const NAME = 'sora-form-placement';
const rows = ROWS.map((r) => COLS.map((c) => r[c[0]]));
for (const [i, r] of rows.entries()) for (const [k, v] of r.entries()) must(v !== undefined && v !== null && !(typeof v === 'number' && !Number.isFinite(v)), 'row ' + ROWS[i].id + ': cell ' + COLS[k][0] + ' is ' + v);
must(new Set(ROWS.map((r) => r.id)).size === ROWS.length, 'duplicate ids');
fs.writeFileSync(P('sheets', 'port', NAME + '.json'), '{"sheet":"' + NAME + '","group":"port","columns":' + JSON.stringify(COLS.map((c) => c[0])) + ',"rows":[\n' + rows.map((r) => JSON.stringify(r)).join(',\n') + '\n]}\n');
const by = {}; for (const r of ROWS) by[r.what] = (by[r.what] || 0) + 1;
console.log('sheets/port/' + NAME + '.json: ' + rows.length + ' rows ' + JSON.stringify(by));
for (const p of PARENTED) console.log('  parented: ' + p.model + ' (' + p.windows + ' window(s)) forms ' + p.cmds.map(hex).join(' '));
for (const r of ROWS.filter((x) => x.what === 'throw')) console.log('  throw: ' + r.id + ' ' + r.flags.join(' '));

// ------------------------------------------------------------------------------------------------ register (this tool's entry only; replaced in place)
const q = (s) => "'" + String(s).replace(/\\/g, '\\\\').replace(/'/g, "\\'") + "'";
const NL = '\n';
const DOC = 'Where KH3 puts the form weapons while their own clips move them off their sockets (kh-core combat/forms/placement.rs): every TresAnimNotifyState_ChangeAttachParent window of the form weapons\' own clips (the weapon on another socket of Sora) and every TresAnimNotify_ThrowWeapon of Sora\'s form clips (the weapon detached, flying with its movement component).';
const MARK = '(AUDIT-A PLACEMENT)';
function upsert(s, name, text, end, pos) {
  const at = s.indexOf("'" + name + "': {");
  if (at < 0) { const p = pos(s); return s.slice(0, p) + text + s.slice(p); }
  const c = s.lastIndexOf(NL, at - 1) + 1, cs = s.lastIndexOf(NL, c - 2) + 1;
  must(s.slice(cs, c).includes(MARK), 'the entry of ' + name + ' is not this tool\'s (no "' + MARK + '" comment above it)');
  const e = s.indexOf(end, at); must(e > at, 'end of the entry of ' + name);
  return s.slice(0, cs) + text + s.slice(e + end.length);
}
const save = (f, before, after, what) => { if (after !== before) { fs.writeFileSync(f, after); console.log('registered ' + NAME + ' in ' + what); } };
{
  const f = P('tools', 'lib', 'sheet_defs.js'); const before = fs.readFileSync(f, 'utf8');
  const END = '] },' + NL;
  const pos = (t) => { const at = t.indexOf("  'sora-keyblade-forms': {"); must(at >= 0, 'sheet_defs.js: anchor sora-keyblade-forms not found'); const e = t.indexOf(END, at); return e + END.length; };
  const text = '  // ' + MARK + ' ' + DOC + ' Written by tools/sora_form_placement.js (build/sora_tmp/weapons/AUDIT-A/PLACEMENT.md)' + NL +
    '  ' + q(NAME) + ": { group: 'port', file: " + q(NAME + '.json') + ", by: 'tools/sora_form_placement.js', from: " + q('sora-keyblade-forms, the keyblades manifest and every form weapon\'s model.json clips, Sora\'s anim pack manifests, the weapon blueprints\' TresWeaponMovementComponent and the decompiled ThrowWeapon route') + ', external: true, cols: [' + NL +
    '    ' + COLS.map((c, i) => '[' + q(c[0]) + ', ' + (i === 0 ? 'K' : 'V') + ', ' + q(c[1]) + ", '-', " + q(c[3]) + (['note', 'flags', 'names', 'values', 'cmds', 'logical', 'clip', 'model'].includes(c[0]) ? ', true' : '') + ']').join(', ') + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/sheet_defs.js');
}
{
  const f = P('tools', 'lib', 'port_gen_defs.js'); const before = fs.readFileSync(f, 'utf8');
  const END = NL + '    },' + NL;
  const pos = (t) => { const anchor = '  tables: {' + NL; const at = t.indexOf(anchor); must(at >= 0, 'port_gen_defs.js: anchor tables'); return at + anchor.length; };
  const cols = COLS.filter((c) => c[2] && !['ref_addr', 'ref_conf', 'note'].includes(c[0])).map((c) => c[0] + ': { type: ' + q(c[2]) + (c[3] && c[3] !== '-' ? ', doc: ' + q(c[3]) : '') + ' }').join(', ');
  const text = '    // ' + MARK + ' ' + NAME + ': written by tools/sora_form_placement.js (build/sora_tmp/weapons/AUDIT-A/PLACEMENT.md)' + NL +
    '    ' + q(NAME) + ': {' + NL +
    "      file: 'sora_form_placement.rs', row: 'FormPlacementRow', table: 'FORM_PLACEMENT', key: 'id', key_prefix: 'FPL'," + NL +
    '      doc: ' + q(DOC) + ',' + NL +
    '      columns: { ' + cols + ' },' + END;
  save(f, before, upsert(before, NAME, text, END, pos), 'tools/lib/port_gen_defs.js');
}
console.log('sora_form_placement ' + VERSION);
