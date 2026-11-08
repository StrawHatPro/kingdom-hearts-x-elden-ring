// (VFX-RING M) Stage the TailModelMarch meshes the keyblades root lacks, converted exactly as the pack does
// (tools/sora_vfx_pack.js exportMesh: metres, +Y up, CCW): node stage_mesh.js [out dir]
const fs = require('fs'), path = require('path');
const vp = require(path.resolve(__dirname, '../../../../../../tools/sora_vfx_pack.js'));
const out = process.argv[2] || path.resolve(__dirname, '../../stage/M/keyblades/vfx/meshes');
const live = path.resolve(__dirname, '../../../../../sora_pack/keyblades/vfx/meshes');
fs.mkdirSync(out, { recursive: true });
for (const ref of ['/Game/Effects/dlc01/wep/w_so220/mdl/kmd_cin00.kmd_cin00', '/Game/Effects/trescom/mdl/tmd_box_4x4x4_00.tmd_box_4x4x4_00']) {
  const name = ref.split('/').pop().split('.')[0];
  if (fs.existsSync(path.join(live, name + '.glb'))) { console.log(`${name}: already in the live pack`); continue; }
  const r = vp.exportMesh(ref);
  if (r.error) { console.log(`${name}: ${r.error}`); process.exitCode = 1; continue; }
  fs.writeFileSync(path.join(out, name + '.glb'), r.glb);
  console.log(`${name}: staged ${r.glb.length} bytes, ${r.vertices} vertices, ${r.triangles} triangles, from ${r.converted_from}`);
}
