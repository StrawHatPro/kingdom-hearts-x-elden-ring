// (XSI, 2026-10-16) THE XS ABILITIES' NEW PACK FILES (build/sora_tmp/party/XS_SORA_ABILITIES.md, approved). Nothing here writes
// build/sora_pack: the files are staged under build/sora_tmp/xsi/pack/ with the pack's own layout and are COPIED INTO
// build/sora_pack AT DEPLOY (new files only; no existing pack file is replaced):
//   anims/<clip>.glb        the clips of other anim sets the abilities play (Light Form, Hyper Hammer, Blizzard Blades,
//                           Ultimate Form, Rage Form, the Re Mind light wave), packed with tools/sora_pack.js's own helpers
//   xsi_manifest.json       an OVERLAY of manifest.json: items (format and notifies as the main manifest's), logical_animations,
//                           root_motion.tracks, attack_sockets.clips of those clips. tools/gen_port.js adds them to the
//                           generated animation table; kh-anim's Pack::load reads it next to manifest.json
//                           (or from KH_PACK_OVERLAY before the deploy)
//
//   node tools/xsi_pack.js            write the staged files
//   node tools/xsi_pack.js --check    exit 1 when a staged file differs from what the tool would write
'use strict';
const fs = require('fs'), path = require('path');
const sp = require('./sora_pack.js');
const { ROOT, CONTENT, readGlb, buildSkeleton, gamePathToContent, loadAnimation, addSkeletonNodes, addAnimation, GlbWriter, sha256, buildRootMotion, attackSockets, rel, assetId, sourceFiles } = sp;

const VERSION = '1.0.0';
const OUT = path.join(ROOT, 'build', 'sora_tmp', 'xsi', 'pack');
const MAIN = path.join(ROOT, 'build', 'sora_pack', 'manifest.json');
const BODY = '/Game/Character/pc/p_ex001/anm/rt/body/';
// [logical name (the row of its anim set), clip, anim set, the ability]
const CLIPS = [
  ['M_M416', 'M416', 'p_ex001_AnimSet22', 'Spiral Rush (Light Form, Re Mind)'],
  ['M_M411', 'M411', 'p_ex001_AnimSet22', 'Zantetsuken (Light Form)'],
  ['M_M413', 'M413', 'p_ex001_AnimSet22', 'Event Horizon (Light Form)'],
  ['M_3416', '3416', 'p_ex001_AnimSet3', 'Giant Stamp (Hyper Hammer)'],
  ['M_7419', '7419', 'p_ex001_AnimSet7', 'Chandelier (Blizzard Blades)'],
  ['M_9419', '9419', 'p_ex001_AnimSet9', 'Myriad (Ultimate Form)'],
  ['M_D417', 'D417', 'p_ex001_AnimSet13', 'Crimson Blast (Rage Form; Riskcharge)'],
  ['M_D419', 'D419', 'p_ex001_AnimSet13', 'Riskcharge (Rage Form)'],
  ['M_D420', 'D420', 'p_ex001_AnimSet13', 'Rage Raid (Rage Form)'],
  ['SoKcShoot', '1813', 'AnimSet/remind/p_ex001_AnimSet_SoKc', 'Light Wave, tap (Re Mind light wave)'],
  ['SoKcIrradiate', '1814', 'AnimSet/remind/p_ex001_AnimSet_SoKc', 'Light Wave, hold (Re Mind light wave)'],
];

function build() {
  const main = JSON.parse(fs.readFileSync(MAIN, 'utf8'));
  const id = main.identification;
  const meshGlb = readGlb(path.join(ROOT, 'assets_converted', 'meshes', CONTENT, gamePathToContent(id.skeletal_mesh) + '.glb'));
  const bones = buildSkeleton(meshGlb.json);
  const have = new Set(main.items.filter((i) => i.kind === 'animation').map((i) => i.animation_name));
  const names = new Set(main.logical_animations.map((l) => l.name));
  const files = new Map();
  const anims = new Map();
  const logical = [];
  const items = [];
  for (const [name, clip, set, use] of CLIPS) {
    if (have.has(clip)) throw new Error(`${clip} is in the main pack already: no overlay needed`);
    if (names.has(name)) throw new Error(`the logical name ${name} is the main anim set's`);
    const gp = `${BODY}${clip}.${clip}`;
    const a = loadAnimation(gp, bones);
    anims.set(gp, a);
    const w = new GlbWriter(`(XSI: tools/xsi_pack.js ${VERSION})`);
    addSkeletonNodes(w, bones);
    addAnimation(w, clip, a);
    w.json.scenes = [{ name: clip, nodes: [0] }];
    const file = `anims/${clip}.glb`;
    files.set(file, w.finish());
    const setPath = `/Game/Character/pc/p_ex001/anm/${set}`;
    logical.push({ name, loop: null, play: { mode: null, loop_bit: null, root_motion_bit: null, blend_in_argument: null, play_rate: 1, root_translation_scale: [1, 1], played_by: 'not a stage-1 animation: mode and blend-in are the arguments of the state that plays it (XSI: crates/kh-core/src/abilities/xs)', blend_in_s: null, asset_m_InterpolationTime: a.props.m_InterpolationTime || 0, sequences_with_bEnableRootMotion: a.props.bEnableRootMotion === true ? [clip] : [], root_motion_extracted: null },
      use: `(XSI) ${use}`, optional: true, anim_set: `${setPath}.${path.basename(set)}`, asset: gp, class: 'AnimSequence', same_asset_as: [], stage1: false, file });
    const content = gamePathToContent(gp);
    items.push({ file, kind: 'animation', bytes: files.get(file).length, sha256: sha256(files.get(file)), source_asset: `/Game/${content}`, source_package: assetId(content), source_sha256: sourceFiles(content), animation_name: clip, logical_names: [name], duration_s: a.length, num_frames: a.numFrames, frame_rate: a.fps, loop: false,
      root_motion_enabled: a.props.bEnableRootMotion === true, tracks: a.tracks, interpolation: a.chans.length ? a.chans[0].interpolation : 'LINEAR', sequence_properties: a.props, key_counts: a.keyCounts, root_bone: a.root.kh_sk, root_track_moves: a.root_track_moves, notifies: a.notifies, converted_from: rel(path.join(ROOT, 'assets_converted', 'animations', CONTENT, content + '.glb')) });
  }
  const rootMotion = buildRootMotion(anims, bones, logical, id, new Set());
  const sockets = attackSockets(anims, bones, id);
  const manifest = {
    format: 'kh3-sora-pack-overlay/1', generator: `tools/xsi_pack.js ${VERSION}`,
    note: 'XSI overlay of manifest.json (the XS abilities\' clips of other anim sets): items, logical_animations, root_motion.tracks and attack_sockets.clips in the main manifest\'s formats; files relative to this file. Local build output, never distributed.',
    legal: main.legal, items, logical_animations: logical, root_motion: { tracks: rootMotion.tracks }, attack_sockets: { clips: sockets.clips },
  };
  files.set('xsi_manifest.json', Buffer.from(JSON.stringify(manifest, null, 1)));
  return files;
}

function main() {
  const check = process.argv.includes('--check');
  const files = build();
  let bad = 0;
  for (const [file, bytes] of files) {
    const to = path.join(OUT, file);
    if (check) {
      if (!fs.existsSync(to) || !fs.readFileSync(to).equals(bytes)) { console.log('DIFFERS ' + file); bad++; }
      continue;
    }
    fs.mkdirSync(path.dirname(to), { recursive: true });
    fs.writeFileSync(to, bytes);
    console.log(`xsi_pack: ${rel(to)} (${bytes.length} bytes)`);
  }
  if (bad) process.exit(1);
}
main();
