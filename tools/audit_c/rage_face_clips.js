// (AUDIT-C) Rage Form's end faces: EndStyle 0x14141CA20 case 0x8B plays face row "ANTI_01" (out of battle) / "ANTI_02" (in
// battle, pawn vt+0x1740) by FUN_1405A8980(pawn+0x440, name, -1, 0, 1, 0); p_ex001_FaceAnimSet maps ANTI_00 / 01 / 02 to
// 9999EM / 9998EM / 9997EM. tools/sora_pack.js packs only the face rows the body clips' notifies name, so these three are
// written here with ITS OWN writer (the same skeleton, GlbWriter / addAnimation, item fields) as NEW files beside the pack's:
// face/9997EM.glb, 9998EM.glb, 9999EM.glb and the side file face/face_extra.json (rows + items; kh-anim Pack reads it; an
// older build never opens it). Nothing the live build reads is rewritten.
//   node tools/audit_c/rage_face_clips.js OUT_DIR      (stage), then copy OUT_DIR/face/* into build/sora_pack/face
const fs = require('fs');
const path = require('path');
const sp = require('../sora_pack');
const out = path.resolve(process.argv[2] || path.join(sp.ROOT, 'build', 'sora_tmp', 'weapons', 'AUDIT-C', 'AN', 'stage'));
const manifest = sp.readJson(path.join(sp.ROOT, 'build', 'sora_pack', 'manifest.json'));
const meshContent = sp.gamePathToContent(manifest.identification.skeletal_mesh);
const meshGlb = sp.readGlb(path.join(sp.ROOT, 'assets_converted', 'meshes', sp.CONTENT, meshContent + '.glb'));
const bones = sp.buildSkeleton(meshGlb.json);
const SET = '/Game/Character/pc/p_ex001/anm/p_ex001_FaceAnimSet';
const ROWS = [['ANTI_00', '9999EM'], ['ANTI_01', '9998EM'], ['ANTI_02', '9997EM']];
fs.mkdirSync(path.join(out, 'face'), { recursive: true });
const rows = [], items = [];
for (const [name, seq] of ROWS) {
  const asset = `/Game/Character/pc/p_ex001/anm/rt/face/${seq}.${seq}`;
  const a = sp.loadAnimation(asset, bones);
  const w = new sp.GlbWriter('(face clip)');
  sp.addSkeletonNodes(w, bones);
  sp.addAnimation(w, path.basename(a.content), a);
  w.json.scenes = [{ name: path.basename(a.content), nodes: [0] }];
  const buf = w.finish();
  const file = `face/${seq}.glb`;
  fs.writeFileSync(path.join(out, file), buf);
  rows.push({ name, anim_set: SET, asset, file });
  items.push({ file, kind: 'face-animation', bytes: buf.length, sha256: sp.sha256(buf), source_asset: asset.split('.')[0], animation_name: path.basename(a.content), logical_names: [name], duration_s: a.length, num_frames: a.numFrames, frame_rate: a.fps, loop: true, tracks: a.tracks, sequence_properties: a.props, notifies: a.notifies });
}
const side = { format: 'kh3-sora-face-extra 1', generator: 'tools/audit_c/rage_face_clips.js', note: 'Face rows the body clips do not name but code plays (EndStyle 0x14141CA20 case 0x8B: ANTI_01 / ANTI_02). Read by kh-anim Pack::load next to manifest.json; rows extend runtime.face.rows.', rows, items };
fs.writeFileSync(path.join(out, 'face', 'face_extra.json'), JSON.stringify(side, null, 1));
console.log('staged', rows.map((r) => r.name + '=' + r.file).join(' '), '->', path.relative(sp.ROOT, out));
