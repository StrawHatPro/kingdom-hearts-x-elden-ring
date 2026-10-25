#!/usr/bin/env node
// (PACKGEN) The bootstrap manifest of a fresh asset-pack build: build/sora_pack/manifest.json with only Sora's skeleton
// and skin joint list, as tools/sora_pack.js writes them.
//
//   node tools/packgen_bootstrap.js
//
// tools/sora_pack.js needs the research data of audit/sora_port (KineDriver, Bonamik, ...), and the research scripts that
// make that data (tools/research) index bones by the pack's skeleton (build/sora_pack/manifest.json skeleton.bones and
// skin.joints). On a fresh machine there is no pack yet: this tool writes those two sections from the converted mesh with
// sora_pack.js's own code (buildSkeleton, and the "bones that carry a vertex weight at LOD 0" rule of its addMesh), so the
// research scripts run before the first pack build. tools/sora_pack.js then overwrites the file with the full manifest
// (the two sections come out the same). It refuses to touch a manifest that is already a full one.
'use strict';
const fs = require('fs');
const path = require('path');
const sp = require('./sora_pack.js');

const MESH = 'Character/pc/p_ex001/mdl/p_ex001_0';
const OUT = path.join(sp.ROOT, 'build', 'sora_pack', 'manifest.json');
if (fs.existsSync(OUT)) {
  let cur = null; try { cur = JSON.parse(fs.readFileSync(OUT, 'utf8')); } catch { /* broken: rewritten */ }
  if (cur && !cur.packgen_bootstrap) { console.log('packgen_bootstrap: build/sora_pack/manifest.json is a full manifest already: kept'); process.exit(0); }
}
const meshGlb = sp.readGlb(path.join(sp.ROOT, 'assets_converted', 'meshes', sp.CONTENT, MESH + '.glb'));
const bones = sp.buildSkeleton(meshGlb.json);
const used = new Set();
for (const pr of meshGlb.json.meshes[0].primitives) {
  const jn0 = sp.readAccessor(meshGlb, pr.attributes.JOINTS_0).data, wt0 = sp.readAccessor(meshGlb, pr.attributes.WEIGHTS_0).data;
  for (let i = 0; i < jn0.length; i++) if (wt0[i] > 0) used.add(jn0[i]);
}
const skinJoints = [...used].sort((a, b) => a - b);
const f32 = sp.f32;
const manifest = {
  packgen_bootstrap: 'written by tools/packgen_bootstrap.js for the research scripts; tools/sora_pack.js replaces this file',
  skeleton: { bone_count: bones.length, root: bones[0].name, node_index_equals_bone_index: true,
    bones: bones.map((b, i) => ({ index: i, name: b.name, parent: b.parent, translation: b.translation.map(f32), rotation: b.rotation.map(f32), scale: b.scale.map(f32), translation_ue: b.ue.translation, rotation_ue: b.ue.rotation, scale_ue: b.ue.scale })) },
  skin: { joint_count: skinJoints.length, joints: skinJoints.map((b, k) => ({ joint: k, bone: b, name: bones[b].name })) },
};
fs.mkdirSync(path.dirname(OUT), { recursive: true });
fs.writeFileSync(OUT, JSON.stringify(manifest, null, 1) + '\n');
console.log(`packgen_bootstrap: ${bones.length} bones, ${skinJoints.length} skin joints -> build/sora_pack/manifest.json`);
