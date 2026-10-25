#!/usr/bin/env node
// tools/sora_outfit_anims.js (IMPL_ER59 / ER69): AN OUTFIT'S OWN CLIPS, for the clips of Sora's pack that it has.
//
// The user: "I want every outfit to use whatever animations they have, default to adjusted KH3 sora's animations if there
// is none". KH3's outfit pawns ship clips of their own under Character/pc/<model>/anm/rt/{body,face} with the names of
// p_ex001's (0001 = idle, 0003 = run, ...: the same rows of the anim sets). For every clip of the pack (manifest items of
// kind animation / face-animation) that the outfit has a clip of the same name for, this writes
// build/sora_pack/outfits/<id>/{anims,face}/<name>.glb: the outfit's clip
//   - TIME-WARPED onto Sora's clip of that name (its length and its key times: the simulation's combo timing, hit frames
//     and cancel windows read Sora's clip info, so the outfit's motion is stretched to fit them);
//   - with Sora's ROOT tracks (kh_sk, trans: the root motion the simulation moves him by);
//   - expressed on SORA'S skeleton (the bones both have): the inverse of the renderer's retarget (crates/kh-er-render
//     outfit.rs: rotation = his * his ref^-1 * outfit ref, translation = outfit ref + his change * body scale), so that
//     the renderer, posing the outfit from this, gives back exactly the outfit's own clip.
// The DLL (kh-anim Animator::set_own_clips) samples these instead of Sora's while the outfit is worn; every other clip
// stays Sora's, retargeted. A list: outfits/<id>/anims.json.
//
//   node tools/sora_outfit_anims.js [--only kh1,kh2]
'use strict';
const fs = require('fs'), path = require('path');
const sp = require('./sora_pack.js');
const ROOT = sp.ROOT, PACK = path.join(ROOT, 'build/sora_pack');
const VERSION = '1.0.0';
const ROOT_BONES = ['kh_sk', 'trans'];
const args = process.argv.slice(2);
const only = (() => { const k = args.indexOf('--only'); return k >= 0 ? args[k + 1].split(',') : null; })();

const qm = (a, b) => [a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1], a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0], a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3], a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]];
const qi = (q) => [-q[0], -q[1], -q[2], q[3]];
const qn = (q) => { const l = Math.hypot(...q) || 1; return q.map((x) => x / l); };
const meshOf = (m) => sp.readGlb(path.join(ROOT, 'assets_converted/meshes', sp.CONTENT, 'Character/pc', m, 'mdl', m + '_0.glb'));
const animFile = (gamePath) => path.join(ROOT, 'assets_converted/animations', sp.CONTENT, sp.gamePathToContent(gamePath) + '.glb');

// a channel's value at time t (UE values; linear, quaternions along the shorter arc)
function sampleCh(ch, t) {
  const n = ch.path === 'rotation' ? 4 : 3, T = ch.times, v = ch.ue_values;
  let k = 0; while (k + 1 < T.length && T[k + 1] <= t) k++;
  const a = v.slice(k * n, k * n + n);
  if (k + 1 >= T.length || t <= T[0]) return t <= T[0] ? v.slice(0, n) : a;
  const b = v.slice((k + 1) * n, (k + 1) * n + n), f = (t - T[k]) / (T[k + 1] - T[k] || 1);
  if (n === 4) { const d = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]; const s = d < 0 ? -1 : 1; return qn(a.map((x, i) => x + (s * b[i] - x) * f)); }
  return a.map((x, i) => x + (b[i] - x) * f);
}
function hipHeight(bones) { const k = bones.findIndex((b) => b.name === 'koshi'); return k < 0 ? null : sp.globalBind(bones)[k][13]; }

function main() {
  const index = JSON.parse(fs.readFileSync(path.join(PACK, 'outfits/outfits.json'), 'utf8'));
  const manifest = JSON.parse(fs.readFileSync(path.join(PACK, 'manifest.json'), 'utf8'));
  const host = sp.buildSkeleton(meshOf('p_ex001').json);
  const hostIndex = new Map(host.map((b, i) => [b.name, i]));
  const items = manifest.items.filter((i) => i.kind === 'animation' || i.kind === 'face-animation');
  for (const o of index.outfits) {
    if (!o.dir || (only && !only.includes(o.id))) continue;
    const dir = path.join(PACK, o.dir);
    const bones = sp.buildSkeleton(meshOf(o.model).json);
    const outIndex = new Map(bones.map((b, i) => [b.name, i]));
    const scale = (hipHeight(bones) || 1) / (hipHeight(host) || 1);
    for (const sub of ['anims', 'face']) fs.rmSync(path.join(dir, sub), { recursive: true, force: true });
    const list = [];
    for (const item of items) {
      const name = item.animation_name;
      const folder = item.kind === 'face-animation' ? 'face' : 'body';
      const ownPath = `/Game/Character/pc/${o.model}/anm/rt/${folder}/${name}.${name}`;
      if (!fs.existsSync(animFile(ownPath))) continue;
      let mine, own;
      try { mine = sp.loadAnimation(item.source_asset.replace(/\.uasset$/, '') + '.' + name, host); } catch (e) { try { mine = sp.loadAnimation(`/Game/Character/pc/p_ex001/anm/rt/${folder}/${name}.${name}`, host); } catch (e2) { list.push({ name, skipped: 'Sora\'s clip: ' + e2.message.slice(0, 80) }); continue; } }
      try { own = sp.loadAnimation(ownPath, bones); } catch (e) { list.push({ name, skipped: 'own clip: ' + e.message.slice(0, 100) }); continue; }
      const Lm = mine.length || 0, Lo = own.length || 0;
      // the output's key times: Sora's clip's frames
      const frames = Math.max(2, mine.numFrames || 2);
      const times = []; for (let f = 0; f < frames; f++) times.push(frames > 1 ? (Lm * f) / (frames - 1) : 0);
      const warp = (t) => (Lm > 0 ? (t * Lo) / Lm : 0);
      const chans = [];
      const byBone = (anim) => { const m = new Map(); for (const c of anim.chans) { if (!m.has(c.bone)) m.set(c.bone, {}); m.get(c.bone)[c.path] = c; } return m; };
      const mineBy = byBone(mine), ownBy = byBone(own);
      const push = (bone, pth, values) => chans.push({ bone, path: pth, times: times.slice(), values, interpolation: 'LINEAR' });
      // the root bones: Sora's tracks
      for (const rb of ROOT_BONES) { const h = hostIndex.get(rb); const tr = mineBy.get(h); if (!tr) continue;
        for (const pth of ['translation', 'rotation', 'scale']) { const c = tr[pth]; if (!c) continue; const vals = []; for (const t of times) { const u = sampleCh(c, t); vals.push(...(pth === 'translation' ? sp.cvPos(...u) : pth === 'rotation' ? sp.cvQuat(...u) : sp.cvScale(...u))); } push(h, pth, vals); } }
      // the outfit's tracks on the bones both have, back on Sora's skeleton
      let shared = 0;
      for (const [ob, tr] of ownBy) {
        const name2 = bones[ob].name; const h = hostIndex.get(name2);
        if (h === undefined || ROOT_BONES.includes(name2)) continue;
        shared++;
        const oref = bones[ob].ue, href = host[h].ue;
        if (tr.rotation) { const vals = []; for (const t of times) { const r = sampleCh(tr.rotation, warp(t)); vals.push(...sp.cvQuat(...qn(qm(qm(r, qi(oref.rotation)), href.rotation)))); } push(h, 'rotation', vals); }
        if (tr.translation) { const vals = []; for (const t of times) { const v = sampleCh(tr.translation, warp(t)); vals.push(...sp.cvPos(...href.translation.map((x, k) => x + (v[k] - oref.translation[k]) / scale))); } push(h, 'translation', vals); }
        if (tr.scale) { const vals = []; for (const t of times) { const v = sampleCh(tr.scale, warp(t)); vals.push(...sp.cvScale(...href.scale.map((x, k) => x * (Math.abs(oref.scale[k]) > 1e-6 ? v[k] / oref.scale[k] : 1)))); } push(h, 'scale', vals); }
      }
      const w = new sp.GlbWriter(`(outfit ${o.id}: its own clip ${name}, tools/sora_outfit_anims.js ${VERSION})`);
      sp.addSkeletonNodes(w, host);
      sp.addAnimation(w, name, { chans });
      w.json.scenes = [{ name, nodes: [0] }];
      const sub = item.kind === 'face-animation' ? 'face' : 'anims';
      fs.mkdirSync(path.join(dir, sub), { recursive: true });
      fs.writeFileSync(path.join(dir, sub, name + '.glb'), w.finish());
      list.push({ name, kind: item.kind, logical: item.logical_names, own_length_s: +Lo.toFixed(4), sora_length_s: +Lm.toFixed(4), warp: Lm > 0 ? +(Lo / Lm).toFixed(4) : null, bones: shared, own_bones_not_in_sora: [...ownBy.keys()].filter((b) => !hostIndex.has(bones[b].name)).length });
    }
    const used = list.filter((l) => !l.skipped);
    fs.writeFileSync(path.join(dir, 'anims.json'), JSON.stringify({ format: 'kh3-sora-outfit-anims/1', generator: 'tools/sora_outfit_anims.js ' + VERSION, outfit: o.id, model: o.model, body_scale: +scale.toFixed(4),
      note: 'the outfit\'s own clips of the names of Sora\'s pack, time-warped onto Sora\'s clips, root tracks Sora\'s, on Sora\'s skeleton (see the tool)', clips: list }, null, 1) + '\n');
    const logical = new Set(used.flatMap((l) => l.logical || []));
    console.log(`${o.id} (${o.model}): ${used.filter((l) => l.kind === 'animation').length} body + ${used.filter((l) => l.kind === 'face-animation').length} face clips of its own${list.length - used.length ? ', ' + (list.length - used.length) + ' skipped' : ''}; actions: ${[...logical].slice(0, 30).join(' ')}${logical.size > 30 ? ' ... (' + logical.size + ')' : ''}`);
    for (const l of list.filter((x) => x.skipped)) console.log('   skipped ' + l.name + ': ' + l.skipped);
  }
}
main();
