// (SHOT-B) Forward kinematics of a weapon socket per frame of one of Sora's clips, in his ACTOR space (UE cm, x forward, his
// yaw 0): Sora's skeleton under the clip (root motion left out when the clip has it) -> his socket the weapon hangs on ->
// the weapon's skeleton under its own clip of the same name (else its reference pose) -> the weapon socket -> a relative
// location. The rule of tools/sora_pack.js attackSockets; the FK is the one tools/sora_form_ship.js / tools/shotlocks/
// barrage.js repeat (those tools are writers, not modules). Used by tools/shotlocks/burst.js and vacuum.js (not a family
// module: tools/sora_shotlocks.js loads only the .js files of tools/shotlocks itself).
'use strict';
const path = require('path');

module.exports = (lib) => {
  const { P, readJson, must, MAIN, OV } = lib;
  const sp = require(path.join(lib.ROOT, 'tools', 'sora_pack.js'));
  const ITEMS = new Map([...MAIN.items, ...OV.items].filter((i) => i.kind === 'animation').map((i) => [i.animation_name, i]));
  const qmulH = (a, b) => [a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1], a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0], a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3], a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]];
  const qrot = (q, v) => { const tx = 2 * (q[1] * v[2] - q[2] * v[1]), ty = 2 * (q[2] * v[0] - q[0] * v[2]), tz = 2 * (q[0] * v[1] - q[1] * v[0]); return [v[0] + q[3] * tx + (q[1] * tz - q[2] * ty), v[1] + q[3] * ty + (q[2] * tx - q[0] * tz), v[2] + q[3] * tz + (q[0] * ty - q[1] * tx)]; };
  const under = (c, p) => { const v = qrot(p.q, [p.s[0] * c.t[0], p.s[1] * c.t[1], p.s[2] * c.t[2]]); return { q: qmulH(p.q, c.q), t: [v[0] + p.t[0], v[1] + p.t[1], v[2] + p.t[2]], s: [p.s[0] * c.s[0], p.s[1] * c.s[1], p.s[2] * c.s[2]] }; };
  const point = (T, v) => { const r = qrot(T.q, [T.s[0] * v[0], T.s[1] * v[1], T.s[2] * v[2]]); return [r[0] + T.t[0], r[1] + T.t[1], r[2] + T.t[2]]; };
  const rotQ = (r) => { const h = Math.PI / 360, SP = Math.sin(r.pitch * h), CP = Math.cos(r.pitch * h), SY = Math.sin(r.yaw * h), CY = Math.cos(r.yaw * h), SR = Math.sin(r.roll * h), CR = Math.cos(r.roll * h); return [CR * SP * SY - SR * CP * CY, -CR * SP * CY - SR * CP * SY, CR * CP * SY - SR * SP * CY, CR * CP * CY + SR * SP * SY]; };
  const ID = MAIN.identification;
  const SORA_BONES = sp.buildSkeleton(sp.readGlb(P('assets_converted', 'meshes', sp.CONTENT, sp.gamePathToContent(ID.skeletal_mesh) + '.glb')).json);
  const SORA_SOCKETS = {};
  for (const e of sp.loadPkg(sp.gamePathToContent(ID.skeleton)).exports.filter((x) => x.class === 'SkeletalMeshSocket')) {
    const p = sp.propsOf(e);
    const loc = p.RelativeLocation ? p.RelativeLocation.value : { x: 0, y: 0, z: 0 }, rot = p.RelativeRotation ? p.RelativeRotation.value : { pitch: 0, yaw: 0, roll: 0 }, sc = p.RelativeScale ? p.RelativeScale.value : { x: 1, y: 1, z: 1 };
    SORA_SOCKETS[p.SocketName.value] = { bone: p.BoneName.value, T: { q: rotQ(rot), t: [loc.x, loc.y, loc.z], s: [sc.x, sc.y, sc.z] } };
  }
  const meshYaw = ID.mesh_component.relative_rotation_ue_deg.yaw * Math.PI / 180, mcy = Math.cos(meshYaw), msy = Math.sin(meshYaw);
  const actor = (v) => [v[0] * mcy - v[1] * msy, v[0] * msy + v[1] * mcy, v[2]];
  const localT = (b, tr, time) => ({ q: tr.rotation ? sp.sampleChan(tr.rotation, time)(tr.rotation.ue_values) : b.ue.rotation, t: tr.translation ? sp.sampleChan(tr.translation, time)(tr.translation.ue_values) : b.ue.translation, s: tr.scale ? sp.sampleChan(tr.scale, time)(tr.scale.ue_values) : b.ue.scale });
  const byBone = (a) => { const m = new Map(); if (a) for (const ch of a.chans) { if (!m.has(ch.bone)) m.set(ch.bone, {}); m.get(ch.bone)[ch.path] = ch; } return m; };
  const boneT = (bones, chans, k, time, rootMotion) => { const chain = []; for (let i = k; i >= 0; i = bones[i].parent) chain.unshift(i); let T = null; for (const i of chain) { const l = localT(bones[i], (i === 0 && rootMotion) ? {} : (chans.get(i) || {}), time); T = T ? under(l, T) : l; } return T; };

  // seq: the clip's sequence name ("1467", "F481"); soraSocket: where the weapon hangs ("R_wep_skt"); model: "w_so080";
  // points: [[weaponSocket, [x, y, z]], ...]. Returns { frames, length, values: per frame the points' xyz, flattened, the
  // weapon clip used ("-" = reference pose) }.
  function track(seq, soraSocket, model, points) {
    const it = ITEMS.get(seq);
    must(it && it.source_asset, 'no clip item ' + seq + ' with a source asset');
    const anim = sp.loadAnimation(it.source_asset + '.' + path.basename(it.source_asset), SORA_BONES);
    const sock = SORA_SOCKETS[soraSocket];
    must(sock, 'Sora has no socket ' + soraSocket);
    const hand = SORA_BONES.findIndex((b) => b.name === sock.bone);
    const dir = P('build', 'sora_pack', 'keyblades', model);
    const mj = readJson(path.join(dir, 'model.json'));
    const ws = readJson(path.join(dir, 'sockets.json')).weapon_sockets;
    const WB = sp.buildSkeleton(sp.readGlb(P('assets_converted', 'meshes', sp.CONTENT, sp.gamePathToContent(mj.source_mesh) + '.glb')).json);
    const logical = (MAIN.logical_animations.find((x) => x.file && path.basename(x.file, '.glb') === seq) || {}).name;
    const wc = (mj.clips || []).find((x) => (x.logical_names || []).includes(logical)) || (mj.clips || []).find((x) => x.sequence === seq);
    const wa = wc ? sp.loadAnimation(wc.asset + '.' + wc.sequence, WB) : null;
    const frames = it.num_frames, length = it.duration_s, values = [];
    for (let f = 0; f < frames; f++) {
      const t = frames > 1 ? f * length / (frames - 1) : 0;
      const H = under(sock.T, boneT(SORA_BONES, byBone(anim), hand, Math.min(t, anim.length), anim.props.bEnableRootMotion === true));
      for (const [name, loc] of points) {
        const s = ws[name];
        must(s, model + ': no socket ' + name);
        const k = WB.findIndex((b) => b.name === s.bone);
        must(k >= 0, model + ': no bone ' + s.bone);
        const S = under({ q: rotQ({ pitch: s.local_rotation[0], yaw: s.local_rotation[1], roll: s.local_rotation[2] }), t: s.local_location, s: [1, 1, 1] }, boneT(WB, byBone(wa), k, wa ? Math.min(t, wa.length) : 0, false));
        values.push(...actor(point(under(S, H), loc)).map((x) => Math.round(x * 10) / 10));
      }
    }
    return { frames, length, values, weaponClip: wc ? wc.sequence : '-', source: it.source_asset };
  }
  return { track };
};
