// (ZETTA) Donald's Zettaflare clips from the KH3 event kg702b (cutscene_kg01_kg702, matinee kg702b) on his gameplay
// skeleton, in the party pack's format (tools/party_pack.js packMember: loadClip + glbOfClip + the manifest item row).
//   node build/sora_tmp/party/ZETTA/tools/zetta_clips.js
// -> build/sora_tmp/party/ZETTA/pack/party/n_ex001/{anims,face}/*.glb + manifest_items.json (rows to append to
//    PACK/party/n_ex001/manifest.json "items" at deploy).
'use strict';
const fs = require('fs'), path = require('path');
const ROOTDIR = path.resolve(__dirname, '../../../../..');
const sp = require(path.join(ROOTDIR, 'tools/sora_pack.js'));
const { CONTENT, sha256, readGlb, buildSkeleton, loadPkg, loadAnimation, addSkeletonNodes, addAnimation, GlbWriter, gamePathToContent } = sp;
const OUT = path.join(ROOTDIR, 'build/sora_tmp/party/ZETTA/pack/party/n_ex001');
const CLIPS = [
  // the channel (matinee 57.333 / 58.767 / 63.5, 3 s each, PlayRate 1, no blend): the pose of the user's reference frame
  { asset: '/Game/Character/npc/n_ex001/anm/localization/ev_kg/body/ja/n_ex001_70286wr.n_ex001_70286wr', kind: 'animation', name: 'n_ex001_70286wr' },
  // the cast (matinee 69.733, 3.1 s): the staff raised, "Zettaflare!", the release
  { asset: '/Game/Character/npc/n_ex001/anm/localization/ev_kg/body/ja/n_ex001_70291w.n_ex001_70291w', kind: 'animation', name: 'n_ex001_70291w' },
  // its face track (Face / Mouth slots, 69.733, 3.1 s); the English lip version (the game's en voice)
  { asset: '/Game/Character/npc/n_ex001/anm/localization/ev_kg/face/en/n_ex001_70291w.n_ex001_70291w', kind: 'face-animation', name: 'n_ex001_70291w_face' },
  // the Japanese lip version (the game's ja voice, `snd.lang = ja` / --voice-language ja): round 2, the beak follows the voice
  { asset: '/Game/Character/npc/n_ex001/anm/localization/ev_kg/face/ja/n_ex001_70291w.n_ex001_70291w', kind: 'face-animation', name: 'n_ex001_70291w_face_ja' },
  // (round 5, the ending) the event kg702c's pose of him under the shrinking beam (kg702c 0 .. 17.267, 4.7 s, played again every 4.7 s):
  // the staff held up, the light on him (the wide shot 960, the shot behind him 970)
  { asset: '/Game/Character/npc/n_ex001/anm/localization/ev_kg/body/ja/n_ex001_70297wr.n_ex001_70297wr', kind: 'animation', name: 'n_ex001_70297wr' },
  // (2026-10-23, the user: "Can you use the cutscene animation where he falls down and drops his staff?") kg702c's collapse after
  // the beam: 702101w (19.267, 1.9 s), 702102w (21.167, 2.667 s; the weapon track keys 21.633), then his exhausted loop
  // 702104er (27.7 .., 1.667 s, again and again: the staff back in his lap) and its last clips 702151e / 702153e (him elsewhere
  // in the set) are not used: the collapse is held on its last frame, his get-up is his own M_0213
  // the collapse
  { asset: '/Game/Character/npc/n_ex001/anm/localization/ev_kg/body/ja/n_ex001_702101w.n_ex001_702101w', kind: 'animation', name: 'n_ex001_702101w' },
  // the collapse, its end
  { asset: '/Game/Character/npc/n_ex001/anm/localization/ev_kg/body/ja/n_ex001_702102w.n_ex001_702102w', kind: 'animation', name: 'n_ex001_702102w' },
];
const mesh = readGlb(path.join(ROOTDIR, 'assets_converted', 'meshes', CONTENT, 'Character/npc/n_ex001/mdl/n_ex001_0.glb'));
const bones = buildSkeleton(mesh.json);
const sk = loadPkg('Character/npc/n_ex001/mdl/n_ex001_0_Skeleton').exports.find((e) => e.class === 'Skeleton');
const skelBones = sk.data.native.referenceSkeleton.refBoneInfo.map((b) => ({ name: b.name, parent: b.parentIndex }));
const extra = skelBones.filter((b) => !bones.some((x) => x.name === b.name)).map((b) => ({ name: b.name, parent: -1 }));
const items = [];
for (const c of CLIPS) {
  const a = loadAnimation(c.asset, extra.length ? [...bones, ...extra] : bones);
  if (extra.length) { a.chans = a.chans.filter((ch) => ch.bone < bones.length); a.tracked_bones = [...new Set(a.chans.map((ch) => ch.bone))].sort((x, y) => x - y); a.tracks = a.tracked_bones.length; }
  const w = new GlbWriter(`(ZETTA) Zettaflare clip of the KH3 event kg702b: ${gamePathToContent(c.asset)}`);
  addSkeletonNodes(w, bones); addAnimation(w, c.name, a); w.json.scenes = [{ name: c.name, nodes: [0] }];
  const glb = w.finish();
  const file = (c.kind === 'face-animation' ? 'face/' : 'anims/') + c.name + '.glb';
  fs.writeFileSync(path.join(OUT, file), glb);
  items.push({ file, kind: c.kind, bytes: glb.length, sha256: sha256(glb), source_asset: `/Game/${a.content}`, animation_name: c.name, logical_names: ['(ZETTA) Zettaflare'], duration_s: a.length, num_frames: a.numFrames, frame_rate: a.fps, loop: false,
    root_motion_enabled: a.props.bEnableRootMotion === true, tracks: a.tracks, interpolation: a.chans.length ? a.chans[0].interpolation : 'LINEAR', sequence_properties: a.props, key_counts: a.keyCounts, root_bone: a.root.kh_sk, root_track_moves: a.root_track_moves, notifies: a.notifies });
  console.log(`${file}: ${a.length.toFixed(3)} s, ${a.numFrames} frames @ ${a.fps}, ${a.tracks} tracks, root moves ${JSON.stringify(a.root_track_moves)}, notifies ${a.notifies.length}`);
}
// (round 5, the eyes) THE CHANNEL'S FACE (the user: "Donald's eyes need to be closed when he's charging up the
// zettaflare"): the event's own face track through the charging shot 860 (kg702b TresInterpGroup_13: [Face]
// n_ex001_fe202 (0.333 s, played again every 0.333 s from 57.333 to 61.667: his eyes shut, no blink) and [Mouth]
// n_ex001_fm200 (1 s, again every 1 s)), merged into one face clip: the eye row's tracks for the Face slot's bones, the lip
// row's for the Mouth slot's (the bones only the lip row moves), 1 s long, played as a loop through the whole channel
{
  const EYE = '/Game/Character/npc/n_ex001/anm/localization/ev_ex/eye/ja/n_ex001_fe202.n_ex001_fe202';
  const LIP = '/Game/Character/npc/n_ex001/anm/localization/ev_ex/lip/ja/n_ex001_fm200.n_ex001_fm200';
  const all = extra.length ? [...bones, ...extra] : bones;
  const prep = (asset) => { const a = loadAnimation(asset, all); if (extra.length) a.chans = a.chans.filter((ch) => ch.bone < bones.length); return a; };
  const e = prep(EYE), l = prep(LIP);
  const eyeBones = new Set(e.chans.map((ch) => ch.bone));
  const a = l;
  a.chans = [...e.chans, ...l.chans.filter((ch) => !eyeBones.has(ch.bone))];
  a.tracked_bones = [...new Set(a.chans.map((ch) => ch.bone))].sort((x, y) => x - y);
  a.tracks = a.tracked_bones.length;
  const name = 'n_ex001_70286_face';
  const w = new GlbWriter('(ZETTA) Zettaflare channel face of the KH3 event kg702b: the eye row n_ex001_fe202 (Face) + the lip row n_ex001_fm200 (Mouth)');
  addSkeletonNodes(w, bones); addAnimation(w, name, a); w.json.scenes = [{ name, nodes: [0] }];
  const glb = w.finish();
  const file = 'face/' + name + '.glb';
  fs.writeFileSync(path.join(OUT, file), glb);
  items.push({ file, kind: 'face-animation', bytes: glb.length, sha256: sha256(glb), source_asset: '/Game/' + e.content + ' + /Game/' + l.content, animation_name: name, logical_names: ['(ZETTA) Zettaflare'], duration_s: a.length, num_frames: a.numFrames, frame_rate: a.fps, loop: true,
    root_motion_enabled: false, tracks: a.tracks, interpolation: a.chans.length ? a.chans[0].interpolation : 'LINEAR', sequence_properties: l.props, key_counts: l.keyCounts, root_bone: l.root.kh_sk, root_track_moves: false, notifies: [] });
  console.log(`${file}: ${a.length.toFixed(3)} s, ${a.tracks} tracks (eye row ${e.chans.length} channels, lip row ${a.chans.length - e.chans.length})`);
}
fs.writeFileSync(path.join(OUT, 'manifest_items.json'), JSON.stringify({ note: '(ZETTA) rows for PACK/party/n_ex001/manifest.json "items" (append; kinds animation / face-animation)', items }, null, 1));
