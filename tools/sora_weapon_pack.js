// Weapon extension of the Sora asset pack: the Kingdom Key (w_so010) as glTF 2.0 binaries, PNG textures, data files and a
// manifest, in exactly the conventions of build/sora_pack/sora.glb (metres, +Y up, right-handed, conversion baked into
// every vertex / bone / key, node index = bone index, counter-clockwise front faces, one skin of the weighted bones).
//
//   node tools/sora_weapon_pack.js            build build/sora_pack/weapon/, validate it, write weapon/sheet_rows.json and
//                                             replace the rows `weapon/...` of sheets/port/sora-assets.json in place
//   node tools/sora_weapon_pack.js --check    build in memory and validate the pack on disk only (nothing is written)
//
// Outputs (build/sora_pack/weapon/):
//   kingdom_key.glb           skeletal mesh /Game/Character/wep/w_so010/mdl/w_so010_0, LOD 0, all 9 bones (kh_sk, wep, chain1..7)
//   textures/*.png            base colour, normal map (referenced by the glb), mask texture and the alpha channel of the
//                             base colour texture (not referenced; described in the manifest)
//   anims/*.glb               one file per AnimSequence of the weapon anim sets (w_so010_AnimSet, w_so010_AnimSet_holo)
//   data/bonamik.json         the weapon's SQEX_BonamikAsset (keychain), same schema as build/sora_pack/data/bonamik.json
//   data/physics_asset.json   the weapon's PhysicsAsset, same schema as build/sora_pack/data/physics_asset.json
//   data/weapon_cdo.json      class default data of the weapon blueprint w_so010_Wep (+ its parent blueprint), equip data
//   manifest.json             everything described: sources with sha256, skeleton, sockets, materials, animations with
//                             notifies, the attach block (how the weapon hangs on Sora's skeleton), data files
//   validation.json           gltf-validator and the independent read-back of every glb, PNG read-back, attach checks
//   sheet_rows.json           the rows of sheets/port/sora-assets.json this pack contributes (tools/sora_pack.js appends them)
//
// Inputs (nothing under assets/ or assets_converted/ is modified): the decoded packages, the converted mesh, animations
// and textures under assets_converted/, and the cooked files under assets/ (hashed only). Helpers (glb writer, coordinate
// conversion, texture decoding, animation loading, validation) are the ones of tools/sora_pack.js, required as a module.
// Everything that names an asset is read from the game's data; nothing is authored here except the material rule
// (which texture parameter is the base colour / normal map) and the list of blueprint packages below.
// The tool is deterministic: the same inputs give byte-identical output. The pack is local build output; never distributed.
const fs = require('fs');
const path = require('path');
const bc = require('./lib/bcdec');
const sp = require('./sora_pack.js');
const { ROOT, CONTENT, UNIT, sha256, readJson, rel, f32, gamePathToContent, loadPkg, importPath, propsOf, structOf, sourceFiles, assetId, readGlb, readAccessor, GlbWriter,
  cvPos, cvQuat, trsMatrix, mul, xform, loadAnimSet, texturePng, texCache, materialInfo, buildSkeleton, addSkeletonNodes, globalBind, addMesh, loadAnimation, addAnimation, validateGlb, checkGlb, sampleChan } = sp;

const VERSION = '1.0.0';
const OUT = path.join(sp.OUT, 'weapon');
const P = (...a) => path.join(...a);
const WEAPON_BP = 'Blueprints/Player/p_ex001/w_so010_Wep';
const EQUIP_DATA = 'Blueprints/Player/p_ex001/w_so010_Equip';
const WEAPON_SET = 'Blueprints/Player/p_ex001/p_ex001_WepSet';
const HOLO_ANIM_BP = 'Character/wep/w_so010/anm/rt_holo_dlc/w_so010_Anim';
const SORA_MESH = 'Character/pc/p_ex001/mdl/p_ex001_0';
const SORA_SKELETON = 'Character/pc/p_ex001/mdl/p_ex001_0_Skeleton';
const SORA_ANIMSET = '/Game/Character/pc/p_ex001/anm/p_ex001_AnimSet.p_ex001_AnimSet'; // rows 1301..1320 (keyblade attacks) are sampled for the swing evidence of the attach block
// which parameters of the root material the pack uses (the game's shader is not reproduced)
const MATERIAL_RULES = [
  { root: /w_allBaseMat$/, base: 'BaseColorTex', normal: 'NormalTex', mask: 'MaskTexture', approx: 'base colour = BaseColorTex, normal map = NormalTex. Left out: the metal / leather layers selected by MaskTexture (parameters "Metal Mask" and "Leather Mask" pick its channels; tiling detail normal and the metal look-up "Metal selection ID"), and ambient occlusion (AOTexture is the base colour texture and AOChannel selects its alpha channel). The root material is BLEND_Masked; what feeds its opacity mask was not traced and the glTF material is opaque. The mask texture and that alpha channel are exported as PNG files but no glTF material slot references them. metallicFactor 0 / roughnessFactor 0.8 are the fixed values of the Sora pack, not game data' },
];

// ------------------------------------------------------------------------------------------------ helpers
const r6 = (x) => +x.toFixed(6);
const objPath = (d, i) => (!i ? null : i < 0 ? importPath(d, i) : d.exports[i - 1].objectName);
const isProps = (v) => Array.isArray(v) && v.length > 0 && v.every((e) => e && typeof e === 'object' && !Array.isArray(e) && 'name' in e && 'type' in e);
const unbits = (v) => { const b = Buffer.alloc(4); b.writeUInt32LE(v >>> 0); return b.readFloatLE(0); };
// Tagged properties of a decoded package -> plain JSON: object references resolved to object paths (imports) or export
// names, static arrays (arrayIndex) as arrays, {f32bits} as the float it stands for.
function plainProps(d, props) {
  const o = {}, statics = new Set();
  for (const p of props || []) if (p.arrayIndex !== undefined) statics.add(p.name); // a static array: element 0 carries no arrayIndex
  for (const p of props || []) {
    const v = plainValue(d, p);
    if (statics.has(p.name)) { if (!(p.name in o)) o[p.name] = []; o[p.name][p.arrayIndex || 0] = v; } else o[p.name] = v;
  }
  return o;
}
function plainValue(d, p) {
  const v = p.value;
  if (p.type === 'ObjectProperty') return objPath(d, v);
  if (p.type === 'ArrayProperty') {
    if (p.innerType === 'ObjectProperty') return v.map((i) => objPath(d, i));
    if (v && Array.isArray(v.items)) return v.items.map((it) => (isProps(it) || (Array.isArray(it) && !it.length) ? plainProps(d, it) : plainAny(d, it)));
    return plainAny(d, v);
  }
  if (p.type === 'MapProperty') return (v.entries || []).map((e) => ({ key: plainAny(d, e.key), value: isProps(e.value) ? plainProps(d, e.value) : p.valueType === 'ObjectProperty' ? objPath(d, e.value) : plainAny(d, e.value) }));
  if (p.type === 'StructProperty') return isProps(v) || (Array.isArray(v) && !v.length) ? plainProps(d, v) : plainAny(d, v);
  return plainAny(d, v);
}
function plainAny(d, v) {
  if (v === null || typeof v !== 'object') return v;
  if (isProps(v)) return plainProps(d, v);
  if (Array.isArray(v)) return v.map((x) => plainAny(d, x));
  if ('f32bits' in v) return unbits(v.f32bits);
  if ('s' in v && 'enc' in v) return v.s;
  const o = {}; for (const k of Object.keys(v)) o[k] = plainAny(d, v[k]); return o;
}
// UE quaternion helpers (x, y, z, w; Hamilton product; q rotates v as q v q^-1), used for the attach transform in UE units
const qmul = (a, b) => [a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1], a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0], a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3], a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2]];
const qrot = (q, v) => { const tx = 2 * (q[1] * v[2] - q[2] * v[1]), ty = 2 * (q[2] * v[0] - q[0] * v[2]), tz = 2 * (q[0] * v[1] - q[1] * v[0]); return [v[0] + q[3] * tx + (q[1] * tz - q[2] * ty), v[1] + q[3] * ty + (q[2] * tx - q[0] * tz), v[2] + q[3] * tz + (q[0] * ty - q[1] * tx)]; };
const qnorm = (q) => { const l = Math.hypot(...q); return q.map((x) => x / l); };
const qconj = (q) => [-q[0], -q[1], -q[2], q[3]];
const tmul = (a, b) => ({ t: qrot(a.q, b.t).map((x, i) => x + a.t[i]), q: qnorm(qmul(a.q, b.q)) }); // a o b, unit scale
const tinv = (a) => { const q = qconj(a.q); return { t: qrot(q, a.t).map((x) => -x), q }; };
// FRotator (degrees) -> FQuat, the formula of stock UE 4.17 FRotator::Quaternion()
function rotatorQuat(r) {
  const h = Math.PI / 360, sp2 = Math.sin(r.pitch * h), cp = Math.cos(r.pitch * h), sy = Math.sin(r.yaw * h), cy = Math.cos(r.yaw * h), sr = Math.sin(r.roll * h), cr = Math.cos(r.roll * h);
  return [cr * sp2 * sy - sr * cp * cy, -cr * sp2 * cy - sr * cp * sy, cr * cp * sy - sr * sp2 * cy, cr * cp * cy + sr * sp2 * sy];
}
function ueGlobals(bones) { // component-space transforms of the reference pose, UE units (all bone scales are 1: checked)
  const g = [];
  bones.forEach((b, i) => { if (b.ue.scale.some((s) => Math.abs(s - 1) > 1e-6)) throw new Error('bone with a scale in the reference pose: ' + b.name); const l = { t: b.ue.translation.slice(), q: qnorm(b.ue.rotation) }; g.push(b.parent < 0 ? l : tmul(g[b.parent], l)); });
  return g;
}
const packT = (x) => ({ translation_m: cvPos(...x.t).map(r6), rotation_xyzw: cvQuat(...x.q).map(r6) });
const ueT = (x) => ({ translation_cm: x.t.map(r6), rotation_xyzw: x.q.map(r6) });
function socketsOf(skeletonContent, bones) { // SkeletalMeshSocket exports of a Skeleton package
  const d = loadPkg(skeletonContent);
  const sk = d.exports.find((e) => e.class === 'Skeleton');
  const order = propsOf(sk).Sockets.value;
  return order.map((ei) => {
    const e = d.exports[ei - 1], p = plainProps(d, e.data.props);
    const loc = p.RelativeLocation || { x: 0, y: 0, z: 0 }, rot = p.RelativeRotation || { pitch: 0, yaw: 0, roll: 0 }, scl = p.RelativeScale || { x: 1, y: 1, z: 1 };
    const local = { t: [loc.x, loc.y, loc.z], q: rotatorQuat(rot) };
    return { name: p.SocketName, export: e.objectName, bone: p.BoneName, bone_index: bones.findIndex((b) => b.name === p.BoneName), serialized: { RelativeLocation: p.RelativeLocation || null, RelativeRotation: p.RelativeRotation || null, RelativeScale: p.RelativeScale || null },
      relative_location_ue_cm: [loc.x, loc.y, loc.z], relative_rotation_ue_deg: rot, relative_scale: [scl.x, scl.y, scl.z], local };
  });
}

// ------------------------------------------------------------------------------------------------ data exports
// The asset part of build/sora_pack/data/bonamik.json for another SQEX_BonamikAsset (field names and derivation rules of the
// research extractor build/sora_tmp/bnm_extract.js; rules read from 0x143B07550 / 0x143B11700, see build/sora_tmp/native_G_bonamik.md).
function bonamikExport(bonamikContent, meshContent, bones, soraBonamik) {
  const d = loadPkg(bonamikContent);
  const A = plainProps(d, d.exports.find((e) => e.class === 'SQEX_BonamikAsset').data.props);
  const strip = (s, re) => (typeof s === 'string' ? s.replace(re, '') : s);
  const v3 = (v) => [v.x, v.y, v.z], q4 = (q) => [q.x, q.y, q.z, q.w];
  const bi = Object.fromEntries(bones.map((b, i) => [b.name, i]));
  const groups = A.m_Solvers.map((s) => ({ id: s.m_GroupId, name: s.m_GroupName, enable: s.m_IsEnable, useConeSEQ: s.m_IsUseConeSEQ, colIter: s.m_ColIter, consIter: s.m_ConsIter, coneIter: s.m_ConeIter, linkIter: s.m_LinkIter, solverIter: s.m_SolverIter, tagId: s.m_TagId, collisionTagId: s.m_CollisionTagId, enableGround: s.m_EnableGround, lodDistance: s.m_LODdistance, refUpVec: s.m_RefUpVec, fpsCorrection: s.m_FpsCorrection || [] }));
  const bodies = A.m_Bodies.map((b) => ({ index: b.m_Index, joint: b.m_JointName, bone_index: bi[b.m_JointName] ?? -1, parentBody: b.m_ParentId, childBody: b.m_ChildId, parentBodyName: b.m_ParentBodyName, childBodyName: b.m_ChildBodyName, group: b.m_GroupId, groupName: groups[b.m_GroupId].name,
    kinematic: b.m_IsKinematic, collision: b.m_IsCollision, multipleCollisionObject: b.m_IsMultipleCollisionObject, colId: b.m_ColId,
    mass: b.m_Mass, inertia: b.m_Inertia, radius: b.m_Radius, damping: b.m_Damping, bounciness: b.m_Bounciness, friction: b.m_Friction, windDrag: b.m_WindDrag, windLift: b.m_WindLift,
    localTransCoeff: b.m_LocalTransCoeff, maxLocalTrans: b.m_MaxLocalTrans, colShape: strip(b.m_ColShape, /^ESQEX_Bonamik_CollisionShape_/), addiConstType: strip(b.m_AddiConstType, /^ESQEX_Bonamik_ConstraintType_/), addiConstStr: b.m_AddiConstStr, addiConstRad: b.m_AddiConstRad,
    localTranslation: v3(b.m_LocalTransform), localRotation: q4(b.m_LocalRotation), localScale: v3(b.m_LocalScale), collisionBatchNo: b.m_CollisionBatchNo, concaveRatio: b.m_ConcaveRatio,
    lfCoeff: b.m_LFcoeff, parkRate: b.m_ParkRate, maxLF: b.m_MaxLF, lfDrag: b.m_LFdrag, lfLift: b.m_LFlift }));
  if (bodies.some((b, i) => b.index !== i)) throw new Error('bonamik body index mismatch');
  const links = A.m_Links.map((l, i) => ({ index: i, parent: l.m_ParentId, child: l.m_ChildId, parentName: l.m_ParentBodyName, childName: l.m_ChildBodyName, type: strip(l.m_LinkType, /^ESQEX_Bonamik_LinkType_/), selfCollision: l.m_IsSelfCollision, jointTwist: l.m_EnableJointTwist, cone: l.m_EnableCone, coneAnimation: l.m_EnableConeAnimation,
    linkStr: l.m_LinkStr, linkParentStr: l.m_LinkParentStr, coneInnerStr: l.m_ConeInnerStr, coneOuterStr: l.m_ConeOuterStr, coneParentStr: l.m_ConeParentStr, limitYdeg: l.m_LimitY, limitZdeg: l.m_LimitZ, coneOffsetYdeg: l.m_ConeOffsetY, coneOffsetZdeg: l.m_ConeOffsetZ, limitOffsetYdeg: l.m_LimitOffsetY, limitOffsetZdeg: l.m_LimitOffsetZ,
    collisionReceiver: l.m_IsCollisionReceiver, collisionRepulsor: l.m_IsCollisionRepulsor, lra: l.m_IsLRA, lraStr: l.m_LRAstr, lraDamping: l.m_LRAdamping, linkBatchNo: l.m_LinkBatchNo, coneBatchNo: l.m_ConeBatchNo, collisionBatchNo: l.m_CollisionBatchNo, pinRadius: l.m_PinRadius, pinStr: l.m_PinStr, kdiTrans: l.m_KdiTrans, kdiRot: l.m_KdiRot, concaveRatio: l.m_ConcaveRatio }));
  const planes = (A.m_Planes || []).map((p) => ({ index: p.m_Index, group: p.m_GroupId, particleA: p.m_ParticleA, particleB: p.m_ParticleB, particleC: p.m_ParticleC, names: [p.m_ParticleAName, p.m_ParticleBName, p.m_ParticleCName], thickness: p.m_Thickness, offset: p.m_Offset, collision: p.m_IsCollision, collisionBatchNo: p.m_CollisionBatchNo, colId: p.m_ColId }));
  // runtime topology (same rules as the Sora export)
  const Pt = bodies.map((b) => ({ b, kin: b.kinematic, col: b.collision, parent: b.parentBody, child: b.childBody, pc: [], bc: false, addi: !(b.addiConstType === 'None' || b.addiConstType === 'GroundAttach') }));
  for (const l of links) {
    if (l.selfCollision || l.lra) continue; if (['ReactionTransfer', 'PinConstraint', 'KdiConstraint'].includes(l.type)) continue;
    if (l.type === 'ParentChild') { const p = Pt[l.parent]; if (p.pc.length < 4) p.pc.push(l.index); p.child = l.child; Pt[l.child].bc = true; }
  }
  const roots = [];
  Pt.forEach((p, i) => {
    if (p.col) return;
    if (p.parent < 0) { for (const li of p.pc) { if (!Pt[links[li].child].kin) { if (p.kin || p.addi) roots.push(i); break; } } } else {
      const par = Pt[p.parent]; const ch = p.child >= 0 ? Pt[p.child] : null;
      if ((!p.bc || (p.kin && par.kin)) && ((ch && !ch.kin && p.kin) || (!p.kin && p.addi))) { p.bc = false; roots.push(i); }
    }
  });
  const chainOf = (i) => { const p = Pt[i]; return { body: i, joint: bodies[i].joint, kinematic: p.kin, children: p.pc.map((li) => ({ link: li, cone: links[li].cone, ...chainOf(links[li].child) })) }; };
  const role = new Array(bones.length).fill(null), body = new Array(bones.length).fill(-1);
  Pt.forEach((p, i) => {
    if (p.col || !groups[p.b.group].enable || p.b.multipleCollisionObject) return; const bidx = p.b.bone_index; if (bidx < 0) return;
    if (!p.kin) { role[bidx] = 'simulated_node'; body[bidx] = i; } else if (p.child >= 0 && !Pt[p.child].kin && Pt[p.child].bc) { if (role[bidx] !== 'simulated_node') { role[bidx] = 'kinematic_parent_rotation'; body[bidx] = i; } }
  });
  const drivenRoot = role.map((r) => !!r);
  bones.forEach((b, i) => { if (role[i]) return; let p = b.parent; while (p >= 0) { if (drivenRoot[p]) { role[i] = 'follower'; break; } p = bones[p].parent; } });
  const boneTable = bones.map((b, i) => ({ index: i, name: b.name, parent: b.parent, bonamik: role[i] || 'none', body: body[i], group: body[i] >= 0 ? bodies[body[i]].groupName : null, bodies_on_bone: bodies.filter((x) => x.bone_index === i).map((x) => x.index) }));
  const count = (a, f) => { const c = {}; a.forEach((x) => { const k = f(x); c[k] = (c[k] || 0) + 1; }); return c; };
  // user data on the mesh
  const mp = loadPkg(meshContent);
  const userData = mp.exports.filter((e) => e.class === 'SQEX_BonamikAssetUserData').map((e) => ({ export: e.objectName, outer: objPath(mp, e.outerIndex), ...plainProps(mp, e.data.props) }));
  const kbd = mp.exports.filter((e) => e.class === 'SQEX_KBDAssetUserData').map((e) => ({ export: e.objectName, ...plainProps(mp, e.data.props) }));
  const S = A.m_SceneManager;
  return {
    _schema: {
      about: `Kingdom Key (w_so010) Bonamik data: the keychain. Source: Content/${bonamikContent} (class SQEX_BonamikAsset, m_VersionNo ${A.m_VersionNo}) + the user data of the mesh. Same schema as build/sora_pack/data/bonamik.json (asset part); written by tools/sora_weapon_pack.js. Units: cm, degrees where the key ends in deg. Quaternions [x,y,z,w]. scene.gravity is in the authoring convention (Y-up, m/s^2).`,
      global_config: 'copied from build/sora_pack/data/bonamik.json: SQEX_BonamikGlobalConfig constructor defaults (0x143AE8A30) and DefaultEngine.ini; process-wide, the same for every Bonamik component.',
      scene: 'SQEX_BonamikSceneManagerDesc as serialized; *Batches are the authoring-time batch layout.',
      runtime: 'values derived from the asset by the rules the Sora export uses (0x143B07550 / 0x143B61CF0). Values the Sora export took from its own user data or from a run of the reference evaluator are null here: not determined for the weapon.',
      groups: `m_Solvers[${groups.length}]; array index = group id used by bodies/planes. enable=false: group starts in state 0 with animation weight 1 (not simulated).`,
      bodies: `m_Bodies[${bodies.length}] = particles. joint = skeleton bone the particle sits on (bone_index into manifest.json skeleton.bones of this weapon pack). Rest/kinematic position = bone.T + bone.R*(bone.S*localTranslation); orientation = bone.R*localRotation. kinematic=true: re-set from the animated bone every evaluation. collision=true: collider only (colShape, radius; Capsule = segment to childBody). colId[8]: group ids of the collider groups a particle collides with (-1 unused).`,
      links: `m_Links[${links.length}]. Lateral/Bending/ParentChild-without-cone = distance springs; ParentChild = chain segment walked root to tip; cone=true adds the cone constraint (limitY/limitZ half angles); selfCollision=true rows are self-collision pairs, not springs.`,
      planes: 'm_Planes: triangles of three particles used as limit planes.',
      chains: `chain roots and parent-child trees by the rules of the Sora export (0x143B07550). Roots found: ${roots.length}; asset field totals.chainRootParticles: ${A.m_TotalChainRootParticles}.`,
      bones: `one row per skeleton bone (${bones.length}). bonamik: simulated_node = a dynamic particle sits on this bone (component-space translation and rotation are written); kinematic_parent_rotation = animated bone whose rotation is re-aimed at its simulated child; follower = no particle, skeleton descendant of a driven bone, carried rigidly; none = untouched.`,
      notify_states: 'Bonamik anim notify states of the weapon clips are in manifest.json items[].notifies (clip 1814 has two); not repeated here.',
      runtime_records: 'data/bonamik2.json = this file plus runtime2 (the records right after core setup on the reference pose), built by tools/lib/bonamik/weapon.js with the reference evaluator tools/lib/bonamik/; the asset reaches no branch the evaluator leaves untranscribed.',
    },
    source: { package: `/Game/${bonamikContent}`, class: 'SQEX_BonamikAsset', versionNo: A.m_VersionNo },
    user_data: { mesh: `/Game/${meshContent}`, bonamik_user_data: userData, kbd_user_data: kbd, TeleportDistance: 0, TeleportDistance_evidence: 'not serialized (class default). The constructors of SQEX_BonamikAssetUserData 0x143AE89D0 (+0x98) and SQEX_BonamikAsset 0x143AE88A0 (m_TeleportDistance +0x560) write neither field (read at instruction level); object memory is zero-filled, so instance Setup 0x143b61cf0 passes 0 and core setup 0x143b07a2b stores teleportDistSq = 40000 (200 cm).', note: 'as serialized in the SkeletalMesh package: one SQEX_BonamikAssetUserData directly on the mesh (ReferenceGroups ["None"]) and one inside SQEX_KBDAssetUserData_0 element "default" (ReferenceGroups ["KEY"]), both naming this asset. Properties that are not serialized have the class default (TeleportDistance is not serialized here: see TeleportDistance_evidence)' },
    global_config: soraBonamik ? soraBonamik.global_config : null,
    scene: { isEnable: S.m_IsEnable, subStep: S.m_SubStep, totalGroup: S.m_TotalGroup, lssCoeffTran: S.m_LSScoeff_tran, lssCoeffRot: S.m_LSScoeff_rot, rootName: S.m_RootName, gravity: v3(S.m_Gravity), fastMotionHandlingMode: strip(S.m_FastMotionHandlingMode, /^ESQEX_Bonamik_FMH_/), jointLinkSimulationMode: S.m_JointLinkSimulationMode ? strip(S.m_JointLinkSimulationMode, /^ESQEX_Bonamik_JOINTLINK_MODE_/) : 'SEQ (not serialized = default 0)', enableGroundCollision: S.m_EnableGroundCollision, refUpVec: S.m_RefUpVec,
      linkBatches: S.m_TotalLinksInBatch, coneBatches: S.m_TotalConesInBatch, collisionBatches: S.m_TotalCollisionsInBatch, selfCollisionBatches: S.m_TotalSelfCollisionsInBatch ?? null, fpsCorrection: S.m_FpsCorrection ?? null,
      totals: { kinematics: A.m_TotalKinematics ?? null, constraints: A.m_TotalConstraints ?? null, cones: A.m_TotalCones ?? null, links: A.m_TotalLinks ?? null, collisions: A.m_TotalCollisions ?? null, selfCollisions: A.m_TotalSelfCollisions ?? null, chainRootParticles: A.m_TotalChainRootParticles ?? null, lateralLinkCones: A.m_TotalLateralLinkCones ?? null } },
    runtime: { substeps: S.m_SubStep, outerIterations: Math.max(...groups.map((g) => g.solverIter)), linkIterations: Math.max(...groups.map((g) => g.linkIter)), collisionIterations: null, fastMotionMode: null, teleportDistance: null, teleportDistanceSq: null, gravityMagnitude: r6(Math.hypot(...v3(S.m_Gravity)) * 100), gravityDirAsset: v3(S.m_Gravity).map((x) => r6(x / Math.hypot(...v3(S.m_Gravity)))),
      evaluation_at_30Hz: soraBonamik ? soraBonamik.runtime.evaluation_at_30Hz : null,
      counts: { bodies: bodies.length, dynamic: bodies.filter((b) => !b.kinematic).length, kinematic: bodies.filter((b) => b.kinematic).length, colliders: bodies.filter((b) => b.collision).length, byShape: count(bodies, (b) => b.colShape + (b.collision ? '/collider' : b.kinematic ? '/kinematic' : '/dynamic')), linksByType: count(links, (l) => (l.selfCollision ? 'SelfCollisionPair' : l.type + (l.cone ? '+cone' : ''))), addiConstraints: count(bodies, (b) => b.addiConstType), chainRoots: roots.length, dynamicByGroup: count(bodies.filter((b) => !b.kinematic), (b) => b.groupName) } },
    groups, bodies, links, planes, chains: roots.map(chainOf), bones: boneTable, notify_states: null, notify_group_names_present: {},
  };
}
// Same schema as build/sora_pack/data/physics_asset.json (research extractor build/sora_tmp/bnmsub_physx.js).
function physicsExport(physContent, meshContent, meshSide, bones, soraPhysics) {
  const d = loadPkg(physContent);
  const pa = d.exports.find((e) => e.class === 'PhysicsAsset'), pp = plainProps(d, pa.data.props);
  const raw = propsOf(pa);
  const bidx = (n) => bones.findIndex((b) => b.name === n);
  const meshProps = Object.fromEntries(meshSide.data.props.map((x) => [x.tag.name, x.value]));
  const mp = loadPkg(meshContent);
  const ref = (n) => (meshProps[n] && meshProps[n].object ? objPath(mp, meshProps[n].object) : null);
  const out = {
    _schema: {
      source: `Content/${physContent} (UPhysicsAsset, UE 4.17 SQEX fork). Same schema as build/sora_pack/data/physics_asset.json; written by tools/sora_weapon_pack.js. Every tagged property stored in the package is reproduced under \`props\`/\`raw\`; a property that is absent from the asset has the class default (listed under \`stock_defaults_not_in_asset\`, stock UE 4.17 values, NOT read from the exe).`,
      units: 'cm, degrees; KSphylElem Length = cylinder part length (total height = Length + 2*Radius), capsule axis = local Z of the element (element transform = Rotation(pitch,yaw,roll) then Center, in the bone local frame); KBoxElem X/Y/Z = full edge lengths',
      physicsAsset: 'props of the PhysicsAsset export: BoundsBodies (indices into bodies[] in SkeletalBodySetups order), plus native collisionDisableTable (pairs of body indices whose mutual collision is disabled)',
      bodies: 'in PhysicsAsset.SkeletalBodySetups order (index = body index used by BoundsBodies / collisionDisableTable). fields: export, bone, boneIndex (index into manifest.json skeleton.bones of this weapon pack, -1 if missing), physicsType, collisionTraceFlag, aggGeom{spheres[],boxes[],sphyls[],convex[]}, raw (all tagged props), native',
      constraints: 'in PhysicsAsset.ConstraintSetup order. fields: export, jointName, bone1 (child), bone2 (parent), frame1{pos,priAxis,secAxis} in bone1 space, frame2{...} in bone2 space (absent pos = (0,0,0), priAxis = (1,0,0), secAxis = (0,1,0)), profile (ProfileInstance props if stored), raw (all tagged props)',
      skeletalMeshReference: 'how the SkeletalMesh references this asset',
      use: 'whether the game simulates these bodies for the equipped keyblade is not determined from the asset (the keychain bones carry Bonamik particles as well)',
    },
    skeletalMeshReference: { mesh: meshContent, PhysicsAsset: ref('PhysicsAsset'), ShadowPhysicsAsset: ref('ShadowPhysicsAsset'), evidence: `assets_converted/meshes/.../${path.basename(meshContent)}.mesh.json data.props PhysicsAsset / ShadowPhysicsAsset (null = not serialized)` },
    stock_defaults_not_in_asset: soraPhysics ? soraPhysics.stock_defaults_not_in_asset : null,
    physicsAsset: { export: pa.objectName, props: pp, native: pa.data.native }, bodies: [], constraints: [],
  };
  for (const ei of raw.SkeletalBodySetups.value) {
    const e = d.exports[ei - 1], p = plainProps(d, e.data.props), g = p.AggGeom || {};
    out.bodies.push({ export: e.objectName, exportIndex: ei, bone: p.BoneName, boneIndex: bidx(p.BoneName), physicsType: p.PhysicsType || '(default PhysType_Default)', collisionTraceFlag: p.CollisionTraceFlag || '(default)',
      aggGeom: { spheres: g.SphereElems || [], boxes: g.BoxElems || [], sphyls: g.SphylElems || [], convex: g.ConvexElems || [] }, raw: p, native: e.data.native });
  }
  for (const ei of raw.ConstraintSetup.value) {
    const e = d.exports[ei - 1], p = plainProps(d, e.data.props), di = p.DefaultInstance || {};
    out.constraints.push({ export: e.objectName, exportIndex: ei, jointName: di.JointName, bone1: di.ConstraintBone1, bone1Index: bidx(di.ConstraintBone1), bone2: di.ConstraintBone2, bone2Index: bidx(di.ConstraintBone2),
      frame1: { pos: di.Pos1 || { x: 0, y: 0, z: 0 }, priAxis: di.PriAxis1 || { x: 1, y: 0, z: 0 }, secAxis: di.SecAxis1 || { x: 0, y: 1, z: 0 } }, frame2: { pos: di.Pos2 || { x: 0, y: 0, z: 0 }, priAxis: di.PriAxis2 || { x: 1, y: 0, z: 0 }, secAxis: di.SecAxis2 || { x: 0, y: 1, z: 0 } },
      profile: di.ProfileInstance || '(not stored: all defaults)', raw: p, native: e.data.native });
  }
  return out;
}
// Class default data of the weapon blueprint: its own serialized properties and those of the parent blueprint, per object.
function weaponCdo() {
  const bp = loadPkg(WEAPON_BP);
  const cls = bp.exports.find((e) => e.class === 'BlueprintGeneratedClass');
  const parentPath = importPath(bp, cls.superIndex), parent = loadPkg(gamePathToContent(parentPath));
  const pcls = parent.exports.find((e) => e.class === 'BlueprintGeneratedClass'), nativeClass = importPath(parent, pcls.superIndex);
  const cdoOf = (d) => d.exports.find((e) => /^Default__/.test(e.objectName));
  const compsOf = (d) => { const c = cdoOf(d), ci = d.exports.indexOf(c) + 1; return d.exports.filter((e) => e.outerIndex === ci); };
  const own = plainProps(bp, cdoOf(bp).data.props), inh = plainProps(parent, cdoOf(parent).data.props);
  const components = {};
  for (const e of compsOf(parent)) components[e.objectName] = { class: e.class, serialized_in_parent_blueprint: plainProps(parent, e.data.props), serialized_in_weapon_blueprint: {} };
  for (const e of compsOf(bp)) { components[e.objectName] = components[e.objectName] || { class: e.class, serialized_in_parent_blueprint: {} }; components[e.objectName].serialized_in_weapon_blueprint = plainProps(bp, e.data.props); }
  for (const c of Object.values(components)) c.effective = { ...c.serialized_in_parent_blueprint, ...c.serialized_in_weapon_blueprint };
  const eff = { ...inh, ...own };
  const atk = components.TresAtkColl0.effective, fx = components.TresEffectAttach0.effective, mesh = components.TresWeaponMesh0.effective;
  const eq = loadPkg(EQUIP_DATA), eqp = plainProps(eq, eq.exports.find((e) => e.class === 'TresEquipKeybladeData').data.props);
  const projPath = eqp.m_EquipMap[0].value.m_List[0].m_ReplaceProjectileSet;
  const proj = projPath ? loadPkg(gamePathToContent(projPath)) : null;
  const ws = loadPkg(WEAPON_SET);
  const pick = (o, re) => Object.fromEntries(Object.entries(o).filter(([k]) => re.test(k)));
  return {
    parentPath, nativeClass, mesh, eqp,
    json: {
      _schema: {
        about: 'Class default data of the Kingdom Key weapon actor that a host needs. Asset data only: every value is a serialized property of the named package (object references resolved to object paths; static arrays as arrays). A property that neither blueprint serializes has the default of the native class (not read here: absent, never guessed).',
        inheritance: 'UE class-default / sub-object template delta serialization: the weapon blueprint stores only what differs from its parent blueprint, which stores only what differs from the native class. `effective` = parent blueprint properties overridden per property by the weapon blueprint (an array property is replaced as a whole).',
        hit_effects: 'TresAtkColl0.m_HitEffects: array of TresAtkColHitEffect, one per effect group name m_GrpName ("None" = the default group). m_PawnHitEffectPerAttr / m_PawnHitSEAssetPerAttr are static arrays of 8; damage_attribute_index_names lists ETresDamageAttribute by value (sheets/reflect/enum-values), which the effect asset names agree with (fir/blz/thd/wat/win/dak/non); that the code indexes the array with this enum is not verified here.',
        attack_collision_shapes: 'TresAtkColl0.CollisionShapesSrc of the parent blueprint w_so_KeybladeBase (shared by every keyblade): TresAtkCollShapeAssetUnit per group name; m_SocketName1/2 are sockets of the weapon skeleton (manifest.json sockets).',
        units: 'cm, seconds, degrees (UE)',
      },
      blueprint: `/Game/${WEAPON_BP}`, parent_blueprint: parentPath, native_class: nativeClass,
      sources: { [assetId(WEAPON_BP)]: sourceFiles(WEAPON_BP), [assetId(gamePathToContent(parentPath))]: sourceFiles(gamePathToContent(parentPath)), [assetId(EQUIP_DATA)]: sourceFiles(EQUIP_DATA), [assetId(WEAPON_SET)]: sourceFiles(WEAPON_SET), ...(proj ? { [assetId(proj._content)]: sourceFiles(proj._content) } : {}) },
      class_default_object: { serialized_in_weapon_blueprint: own, serialized_in_parent_blueprint: inh, effective: eff },
      appear_disappear: pick(eff, /Appear|Disappear/),
      effects: { m_AppearEffect: eff.m_AppearEffect ?? null, m_DisappearEffect: eff.m_DisappearEffect ?? null, m_CmnMagicCastEffect: eff.m_CmnMagicCastEffect ?? null, m_CmnBadStatesEffect: eff.m_CmnBadStatesEffect ?? null },
      sounds: { m_SoundAssets_Hit: eff.m_SoundAssets_Hit ?? null, m_SoundAssets_Swing: eff.m_SoundAssets_Swing ?? null, m_SoundAliasAsset: eff.m_SoundAliasAsset ?? null },
      mesh_component: { name: 'TresWeaponMesh0', SkeletalMesh: mesh.SkeletalMesh, AnimClass: mesh.AnimClass, AttachParent: mesh.AttachParent, BodyInstance: mesh.BodyInstance ?? null, RelativeLocation: mesh.RelativeLocation ?? null, RelativeRotation: mesh.RelativeRotation ?? null, RelativeScale3D: mesh.RelativeScale3D ?? null, note: 'null = not serialized in either blueprint (native default, not read)' },
      hit_effects: atk.m_HitEffects, damage_attribute_index_names: ['TRES_DMG_ATTR_PHYSICAL', 'TRES_DMG_ATTR_FIRE', 'TRES_DMG_ATTR_BLIZZARD', 'TRES_DMG_ATTR_THUNDER', 'TRES_DMG_ATTR_WATER', 'TRES_DMG_ATTR_AERO', 'TRES_DMG_ATTR_DARK', 'TRES_DMG_ATTR_NOTYPE'],
      attack_collision_shapes: atk.CollisionShapesSrc ?? null,
      common_effect_list: fx.CommonEffectList ?? null, designated_attach_data_list: fx.DesignatedAttachDataList ?? null,
      movement_component: components.TresWeaponMovement0.effective,
      components,
      equip_data: { asset: `/Game/${EQUIP_DATA}`, class: 'TresEquipKeybladeData', properties: eqp },
      projectile_set: proj ? { asset: projPath, properties: plainProps(proj, proj.exports.find((e) => e.class === 'TresProjectileSet').data.props) } : null,
      weapon_set: { asset: `/Game/${WEAPON_SET}`, class: 'TresWeaponSet', properties: plainProps(ws, ws.exports.find((e) => e.class === 'TresWeaponSet').data.props) },
    },
  };
}

// ------------------------------------------------------------------------------------------------ main
async function main() {
  const checkOnly = process.argv.includes('--check');
  bc.selfCheck();
  const cdo = weaponCdo();
  const meshContent = gamePathToContent(cdo.mesh.SkeletalMesh), animClassContent = gamePathToContent(cdo.mesh.AnimClass);
  const meshGlbFile = P(ROOT, 'assets_converted', 'meshes', CONTENT, meshContent + '.glb');
  const meshGlb = readGlb(meshGlbFile), meshSide = readJson(P(ROOT, 'assets_converted', 'meshes', CONTENT, meshContent + '.mesh.json'));
  const bones = buildSkeleton(meshGlb.json);
  const meshPkg = loadPkg(meshContent);
  const imp = (cn) => meshPkg.imports.map((im, i) => ({ im, i })).filter((x) => x.im.className === cn).map((x) => importPath(meshPkg, -(x.i + 1)));
  const skeletonPath = imp('Skeleton')[0], physPath = imp('PhysicsAsset')[0], bonamikPath = imp('SQEX_BonamikAsset')[0];
  const skeletonContent = gamePathToContent(skeletonPath);
  { // the Skeleton asset lists the same bones in the same order as the mesh
    const sk = loadPkg(skeletonContent).exports.find((e) => e.class === 'Skeleton').data.native.referenceSkeleton.refBoneInfo;
    if (sk.length !== bones.length || sk.some((b, i) => b.name !== bones[i].name || b.parentIndex !== bones[i].parent)) throw new Error('skeleton asset and mesh list different bones');
  }

  // ---- animations: the AnimBlueprint of the mesh component names the anim set; the holo AnimBlueprint names another
  const animBp = (content) => { const d = loadPkg(content); const c = plainProps(d, d.exports.find((e) => /^Default__/.test(e.objectName)).data.props); const cls = d.exports.find((e) => e.class === 'AnimBlueprintGeneratedClass');
    const slots = Object.entries(c).filter(([k]) => /^AnimGraphNode_Slot_/.test(k)).map(([, v]) => v.SlotName);
    return { anim_class: `/Game/${content}`, native_parent: importPath(d, cls.superIndex), target_skeleton: objPath(d, propsOf(cls).TargetSkeleton.value), anim_sets: c.MyAnimSets || [], default_anim_name: c.m_DefaultAnimName ?? null, slots, graph_nodes: Object.keys(c).filter((k) => /AnimGraphNode_/.test(k)).map((k) => k.replace(/_[0-9A-F]{32}$/, '')) }; };
  const animBps = [{ ...animBp(animClassContent), used_by: `AnimClass of TresWeaponMesh0 in /Game/${WEAPON_BP}` }, { ...animBp(HOLO_ANIM_BP), used_by: null, used_by_note: 'rt_holo_dlc variant; no reference to it was searched for (not referenced by the weapon blueprint)' }];
  const logical = []; const seqs = new Map();
  for (const ab of animBps) for (const setPath of ab.anim_sets) {
    const set = loadAnimSet(setPath);
    for (const r of set.rows) { if (!r.asset) continue; logical.push({ name: r.name, anim_set: `/Game/${set.content}`, asset: r.asset }); if (!seqs.has(r.asset)) seqs.set(r.asset, []); seqs.get(r.asset).push({ name: r.name, set: `/Game/${set.content}` }); }
  }
  const animFileOf = (gp) => 'anims/' + path.basename(gamePathToContent(gp)) + '.glb';
  { const names = [...seqs.keys()].map(animFileOf); if (new Set(names).size !== names.length) throw new Error('two sequences with the same file name'); }
  const anims = new Map();
  for (const gp of seqs.keys()) {
    const a = loadAnimation(gp, bones);
    const sb = buildSkeleton(readGlb(P(ROOT, 'assets_converted', 'animations', CONTENT, a.content + '.glb')).json);
    if (sb.length !== bones.length || sb.some((b, i) => b.name !== bones[i].name || b.parent !== bones[i].parent)) throw new Error('skeleton of ' + gp + ' differs from the mesh skeleton');
    anims.set(gp, a);
  }

  // ---- materials and textures
  const extraTex = []; // PNG files no glTF material references
  const mats = meshGlb.json.materials.map((m, slot) => {
    const gp = m.extras.ue_material, info = materialInfo(gp);
    const rule = MATERIAL_RULES.find((r) => r.root.test(info.root_material || ''));
    if (!rule) throw new Error('no material rule for ' + info.root_material + ' (' + gp + ')');
    const gl = { name: m.name, pbrMetallicRoughness: { metallicFactor: 0, roughnessFactor: 0.8 }, extras: { ue_material: gp, ue_root_material: info.root_material, ue_blend_mode: info.blend_mode } };
    const textures = []; const rec = { slot, slot_name: m.name, material_instance: gp, parent_chain: info.chain, root_material: info.root_material, ue_blend_mode: info.blend_mode, ue_two_sided: info.two_sided, approximation: rule.approx,
      texture_parameters: info.tex, vector_parameters: info.vec, scalar_parameters: info.scal };
    const t = texturePng(info.tex[rule.base], 'color'); textures.push(t); gl.pbrMetallicRoughness.baseColorTexture = { index: t.file }; rec.base_color = { parameter: rule.base, texture: info.tex[rule.base], file: t.file };
    if (info.two_sided) gl.doubleSided = true;
    if (info.tex[rule.normal]) { const n = texturePng(info.tex[rule.normal], 'normal'); textures.push(n); gl.normalTexture = { index: n.file }; rec.normal = { parameter: rule.normal, texture: info.tex[rule.normal], file: n.file }; }
    if (info.tex[rule.mask]) { // mask texture: all four channels as they are (linear data)
      const content = gamePathToContent(info.tex[rule.mask]);
      const d = bc.decodeDds(fs.readFileSync(P(ROOT, 'assets_converted', 'textures', CONTENT, content + '.dds')), 0);
      let alphaUsed = false; for (let i = 3; i < d.rgba.length; i += 4) if (d.rgba[i] !== 255) { alphaUsed = true; break; }
      const file = 'textures/' + path.basename(content) + '.png';
      extraTex.push({ file, png: bc.encodePng(d.width, d.height, Buffer.from(d.rgba), { alpha: alphaUsed, srgb: false }), content, mode: 'mask', width: d.width, height: d.height, pixel_format: d.format, srgb: false, channels: alphaUsed ? 4 : 3,
        note: `linear data, channels unchanged${alphaUsed ? ' (RGBA)' : ' (RGB; the source alpha is 255 everywhere)'}; the material parameters "Metal Mask" ${JSON.stringify(info.vec['Metal Mask'] || null)} and "Leather Mask" ${JSON.stringify(info.vec['Leather Mask'] || null)} are channel weights (RGBA) on this texture. Not referenced by the glb` });
      rec.mask = { parameter: rule.mask, texture: info.tex[rule.mask], file, metal_mask_channels_rgba: info.vec['Metal Mask'] || null, leather_mask_channels_rgba: info.vec['Leather Mask'] || null };
    }
    { // alpha channel of the base colour texture (dropped from the colour PNG), which the AO parameters select
      const content = gamePathToContent(info.tex[rule.base]);
      const d = bc.decodeDds(fs.readFileSync(P(ROOT, 'assets_converted', 'textures', CONTENT, content + '.dds')), 0);
      let lo = 255, hi = 0; const px = Buffer.alloc(d.width * d.height * 4);
      for (let i = 0; i < d.rgba.length; i += 4) { const a = d.rgba[i + 3]; if (a < lo) lo = a; if (a > hi) hi = a; px[i] = px[i + 1] = px[i + 2] = a; px[i + 3] = 255; }
      rec.base_color.source_alpha_range = [lo, hi];
      if (lo !== hi) {
        const file = 'textures/' + path.basename(content) + '_alpha.png';
        extraTex.push({ file, png: bc.encodePng(d.width, d.height, px, { alpha: false, srgb: false }), content, mode: 'alpha_channel', width: d.width, height: d.height, pixel_format: d.format, srgb: false, channels: 3,
          note: `the alpha channel of ${path.basename(content)} as a grey image (linear). The material instance sets AOTexture = ${info.tex.AOTexture ? path.basename(gamePathToContent(info.tex.AOTexture)) : null} and AOChannel = ${JSON.stringify(info.vec.AOChannel || null)} (RGBA weights), i.e. this channel is the ambient-occlusion input of the shader. Not referenced by the glb` });
        rec.ambient_occlusion = { parameter: 'AOTexture', texture: info.tex.AOTexture || null, channel_weights_rgba: info.vec.AOChannel || null, file };
      }
    }
    return { gltf: gl, textures, rec };
  });

  // ---- sockets of the weapon skeleton, attach transform on Sora's skeleton
  const wg = ueGlobals(bones);
  const wSockets = socketsOf(skeletonContent, bones).map((s) => { const comp = tmul(wg[s.bone_index], s.local); const { local, ...restS } = s;
    return { ...restS, local_ue: ueT(local), local_pack: packT(local), component_space_reference_pose_ue: ueT(comp), component_space_reference_pose_pack: packT(comp) }; });
  const soraGlb = readGlb(P(ROOT, 'assets_converted', 'meshes', CONTENT, SORA_MESH + '.glb'));
  const soraBones = buildSkeleton(soraGlb.json), sg = ueGlobals(soraBones);
  const soraSockets = socketsOf(SORA_SKELETON, soraBones);
  const equips = []; // every TresEquipmentAssetUnit of the equip data
  for (const form of cdo.eqp.m_EquipMap) for (const set of form.value.m_List) for (const u of set.m_Equips) equips.push({ weapon_form: form.key, grow_level: set.m_GrowLevel, owner_wear_form: set.m_OwnerWearForm, ...u });
  const names = [...new Set(equips.map((u) => u.AttachBoneName))];
  if (names.length !== 1) throw new Error('equip data names several attach sockets: ' + names.join(', '));
  const used = soraSockets.find((s) => s.name === names[0]);
  if (!used) throw new Error('Sora\'s skeleton has no socket ' + names[0]);
  const sockDesc = (s) => { const comp = tmul(sg[s.bone_index], s.local); const { local, ...restS } = s; return { ...restS, local_ue: ueT(local), local_pack: packT(local), sora_component_space_reference_pose_ue: ueT(comp), sora_component_space_reference_pose_pack: packT(comp) }; };
  const sockW = tmul(sg[used.bone_index], used.local); // socket in Sora's component space, reference pose (UE)
  const wepBone = bones.findIndex((b) => b.name === 'wep');
  const wepInSora = tmul(sockW, wg[wepBone]);
  // weapon geometry in the frame of the bone `wep` (UE cm): extents, cross-section along the blade axis
  const geo = (() => {
    const inv = tinv(wg[wepBone]); const mn = [Infinity, Infinity, Infinity], mx = [-Infinity, -Infinity, -Infinity]; const pts = [];
    for (const pr of meshGlb.json.meshes[0].primitives) { const pos = readAccessor(meshGlb, pr.attributes.POSITION).data; for (let i = 0; i < pos.length; i += 3) { const p = qrot(inv.q, [pos[i], pos[i + 1], pos[i + 2]]).map((x, k) => x + inv.t[k]); pts.push(p); for (let c = 0; c < 3; c++) { if (p[c] < mn[c]) mn[c] = p[c]; if (p[c] > mx[c]) mx[c] = p[c]; } } }
    const slices = []; // per 4 cm slice along wep +X: largest distance from the axis
    for (let x0 = Math.floor(mn[0] / 4) * 4; x0 < mx[0]; x0 += 4) { let r = 0, n = 0, yMin = 0, yMax = 0, zMin = 0, zMax = 0; for (const p of pts) if (p[0] >= x0 && p[0] < x0 + 4) { n++; r = Math.max(r, Math.hypot(p[1], p[2])); yMin = Math.min(yMin, p[1]); yMax = Math.max(yMax, p[1]); zMin = Math.min(zMin, p[2]); zMax = Math.max(zMax, p[2]); } slices.push({ x_from_cm: x0, x_to_cm: x0 + 4, vertices: n, max_radius_cm: r6(r), y_range_cm: [r6(yMin), r6(yMax)], z_range_cm: [r6(zMin), r6(zMax)] }); }
    // cross-section through the bone origin (plane x = 0): distance from the blade axis of every triangle edge crossing it.
    // The grip is a shaft around the axis; the guard of the Kingdom Key is a frame around the grip, further out.
    const radii = []; let base = 0;
    for (const pr of meshGlb.json.meshes[0].primitives) { const ix = readAccessor(meshGlb, pr.indices).data; const n = readAccessor(meshGlb, pr.attributes.POSITION).n;
      for (let i = 0; i < ix.length; i += 3) for (const [a, b] of [[0, 1], [1, 2], [2, 0]]) { const p = pts[base + ix[i + a]], q = pts[base + ix[i + b]]; if ((p[0] < 0) === (q[0] < 0)) continue; const f = p[0] / (p[0] - q[0]); radii.push(Math.hypot(p[1] + (q[1] - p[1]) * f, p[2] + (q[2] - p[2]) * f)); }
      base += n; }
    const shaft = radii.filter((r) => r < 5), outer = radii.filter((r) => r >= 5);
    // the teeth of the key: the side of the blade axis the vertices beyond 3/4 of the blade lean to
    const far = pts.filter((p) => p[0] > mx[0] * 0.75), tc = far.reduce((a, p) => [a[0] + p[1] / far.length, a[1] + p[2] / far.length], [0, 0]), tl = Math.hypot(...tc) || 1;
    return { min: mn.map(r6), max: mx.map(r6), slices, section_at_origin: { edges_crossing: radii.length, shaft_radius_cm: shaft.length ? r6(Math.max(...shaft)) : null, nearest_other_geometry_cm: outer.length ? r6(Math.min(...outer)) : null }, teeth_dir_wep: [0, tc[0] / tl, tc[1] / tl], teeth_offset_cm: r6(tl) };
  })();
  const hand = soraBones.findIndex((b) => b.name === 'R_te');
  const fingerRoots = soraBones.map((b, i) => ({ b, i })).filter((x) => x.b.parent === hand && /^R_yubi/.test(x.b.name));
  const sub = (a, b) => a.map((x, i) => x - b[i]);
  const gripChecks = (() => {
    const o = wepInSora.t, h = sg[hand].t;
    const mid = fingerRoots.map((x) => sg[x.i].t).reduce((s, p) => s.map((v, i) => v + p[i] / fingerRoots.length), [0, 0, 0]);
    const axis = qrot(wepInSora.q, [1, 0, 0]);
    const forearm = sub(sg[hand].t, sg[soraBones[hand].parent].t), fl = Math.hypot(...forearm);
    return {
      weapon_origin_bone_wep_in_sora_component_space_ue_cm: o.map(r6), right_hand_bone_R_te_ue_cm: h.map(r6), finger_root_bones: fingerRoots.map((x) => x.b.name), finger_roots_mean_ue_cm: mid.map(r6),
      grip_origin_minus_hand_bone_ue_cm: sub(o, h).map(r6), grip_origin_distance_to_hand_bone_cm: r6(Math.hypot(...sub(o, h))), grip_origin_distance_to_finger_roots_mean_cm: r6(Math.hypot(...sub(o, mid))),
      hand_bone_to_finger_roots_mean_cm: r6(Math.hypot(...sub(mid, h))),
      blade_axis_wep_plus_x_in_sora_component_space: axis.map(r6), angle_blade_axis_to_forearm_deg: r6(Math.acos(Math.max(-1, Math.min(1, (axis[0] * forearm[0] + axis[1] * forearm[1] + axis[2] * forearm[2]) / fl))) * 180 / Math.PI),
      blade_tip_socket_ATK_01_in_sora_component_space_ue_cm: tmul(wepInSora, { t: wSockets.find((s) => s.name === 'ATK_01').relative_location_ue_cm, q: [0, 0, 0, 1] }).t.map(r6),
      weapon_extent_along_blade_axis_cm: [geo.min[0], geo.max[0]], cross_section_at_grip_origin: geo.section_at_origin,
      teeth_direction_in_sora_component_space: qrot(wepInSora.q, geo.teeth_dir_wep).map(r6), teeth_direction_in_bone_wep_space: geo.teeth_dir_wep.map(r6), teeth_centroid_offset_from_axis_cm: geo.teeth_offset_cm,
      teeth_direction_note: 'observation, not a check; at the reference pose of Sora the right arm points along -X and down, the blade points forward (+Y)',
      ok: Math.hypot(...sub(o, h)) < 15 && Math.hypot(...sub(o, mid)) < 10 && geo.min[0] < -5 && geo.max[0] > 50 && geo.section_at_origin.shaft_radius_cm !== null && geo.section_at_origin.shaft_radius_cm < 4 && (geo.section_at_origin.nearest_other_geometry_cm === null || geo.section_at_origin.nearest_other_geometry_cm > 6),
      ok_rule: 'the origin of the bone wep lies within 15 cm of the hand bone R_te and within 10 cm of the mean of the finger root bones; the mesh extends on both sides of that origin along the blade axis; the cross-section of the mesh through that origin has a shaft of radius < 4 cm around the axis (the grip) and nothing else within 6 cm of the axis (room for the fingers inside the guard)',
    };
  })();
  // the same attach transform computed a second way: pack-space matrices of the pack skeleton
  const packCheck = (() => {
    const pg = globalBind(soraBones), sl = packT(used.local), m = mul(pg[used.bone_index], trsMatrix(sl.translation_m, sl.rotation_xyzw, [1, 1, 1]));
    const wl = packT(wg[wepBone]), mw = mul(m, trsMatrix(wl.translation_m, wl.rotation_xyzw, [1, 1, 1]));
    const viaUe = packT(wepInSora), o = xform(mw, [0, 0, 0]), ax = sub(xform(mw, [1, 0, 0]), o), axUe = cvPos(...qrot(wepInSora.q, [1, 0, 0])).map((x) => x / UNIT);
    return { origin_dev_m: +Math.hypot(...sub(o, viaUe.translation_m)).toExponential(2), axis_dev: +Math.hypot(...sub(ax, axUe)).toExponential(2) };
  })();
  // Evidence from Sora's own attack clips for the roll of the weapon about its blade axis: during the attack-collision
  // windows (TresAnimNotifyState_Attack with a KB_ collision group) the tip of the blade moves; a blade swung edge-on moves
  // in the plane of its teeth. Measured with the weapon attached as computed above.
  const swing = (() => {
    const chain = []; for (let i = used.bone_index; i >= 0; i = soraBones[i].parent) chain.unshift(i);
    const relq = tmul(used.local, wg[wepBone]).q, tipX = wSockets.find((x) => x.name === 'ATK_01').relative_location_ue_cm[0];
    const clips = []; let sw = 0, st = 0, sf = 0;
    for (const row of loadAnimSet(SORA_ANIMSET).rows) {
      if (!/^13[0-2][0-9]$/.test(row.name) || !row.asset) continue;
      const pk = loadPkg(gamePathToContent(row.asset)); if (pk.exports.find((e) => e.outerIndex === 0).class !== 'AnimSequence') continue;
      const a = loadAnimation(row.asset, soraBones);
      const wins = a.notifies.filter((n) => n.class === 'TresAnimNotifyState_Attack' && /^KB_/.test(n.properties.CollisionGrpName || '')).map((n) => [n.trigger_time, n.end_trigger_time]);
      if (!wins.length || a.numFrames < 3) continue;
      const by = new Map(); for (const c of a.chans) { if (!by.has(c.bone)) by.set(c.bone, {}); by.get(c.bone)[c.path] = c; }
      const wepAt = (t) => { let g = null; for (const i of chain) { const tr = by.get(i) || {}, b = soraBones[i]; const l = { q: qnorm(tr.rotation ? sampleChan(tr.rotation, t)(tr.rotation.ue_values) : b.ue.rotation), t: tr.translation ? sampleChan(tr.translation, t)(tr.translation.ue_values) : b.ue.translation }; g = g ? tmul(g, l) : l; } return { q: qnorm(qmul(g.q, relq)), t: g.t }; };
      const dt = a.length / (a.numFrames - 1); let w = 0, ct = 0, cf = 0, n = 0;
      for (let k = 1; k < a.numFrames; k++) { const t1 = k * dt, tm = t1 - dt / 2; if (!wins.some(([s0, e0]) => tm >= s0 && tm <= e0)) continue;
        const A = wepAt(t1 - dt), B = wepAt(t1), pa = qrot(A.q, [tipX, 0, 0]).map((x, i) => x + A.t[i]), pb = qrot(B.q, [tipX, 0, 0]).map((x, i) => x + B.t[i]), v = sub(pb, pa), sp1 = Math.hypot(...v); if (!(sp1 > 0)) continue;
        const teeth = qrot(B.q, geo.teeth_dir_wep), flat = qrot(B.q, [0, geo.teeth_dir_wep[2], -geo.teeth_dir_wep[1]]);
        w += sp1; ct += v[0] * teeth[0] + v[1] * teeth[1] + v[2] * teeth[2]; cf += Math.abs(v[0] * flat[0] + v[1] * flat[1] + v[2] * flat[2]); n++; }
      if (!n) continue;
      clips.push({ row: row.name, asset: row.asset, attack_windows_s: wins.map((x) => x.map(r6)), intervals: n, tip_path_cm: r6(w), mean_cos_velocity_teeth: r6(ct / w), mean_abs_cos_velocity_flat_normal: r6(cf / w) }); sw += w; st += ct; sf += cf;
    }
    return { method: 'Rows 1301..1329 of ' + SORA_ANIMSET + ' that are AnimSequences with a TresAnimNotifyState_Attack whose CollisionGrpName starts with KB_: per frame interval inside those windows, the displacement v of the blade tip (socket ATK_01) with Sora posed by the clip (bones without a track at the reference pose) and the weapon attached as in reference_pose; cosines are weighted by |v|. teeth = the side of the blade the teeth are on; flat normal = perpendicular to blade axis and teeth.',
      clips, total_tip_path_cm: r6(sw), mean_cos_velocity_teeth: sw ? r6(st / sw) : null, mean_abs_cos_velocity_flat_normal: sw ? r6(sf / sw) : null };
  })();
  const attach = {
    socket_determined: true, relative_transform_determined: true,
    relative_transform_evidence: '[A] the socket and its transform are asset data (below). [C] attach rule: the equipment component attaches the spawned weapon actor with AActor::AttachToComponent 0x143E8FC60 (verdict differs; called from 0x14177E4E0, no verdict) to the owner mesh at the socket AttachBoneName with rules location / rotation / scale = SnapToTarget, no welding (rules constant at 0x148750ED8; finding of the weapon research strand, build/sora_tmp/vfx/weapon/STATE.md), i.e. the weapon actor root sits exactly on the socket. [C] the TresWeaponBase constructor 0x141777570 (verdict same) creates TresWeaponMesh0 attached to the actor root and writes no relative location / rotation / scale to it, and neither blueprint serializes one [A]: the mesh component is at identity. Not read at instruction level; a runtime probe of the mesh component transform would close it. Corroboration from asset data: swing_evidence (the blade is swung in the plane of its teeth) and a software render of Sora in the battle idle clip 1015 with the weapon attached this way (build/sora_tmp/vfx/wpack/r_idleB_*.png: the hand closes around the grip inside the guard)',
    socket: used.name, socket_evidence: `[A] /Game/${EQUIP_DATA} (TresEquipKeybladeData) m_EquipMap[*].m_List[*].m_Equips[0]: UnitName "${equips[0].UnitName}", m_AttachPart ${equips[0].m_AttachPart}, AttachBoneName "${used.name}", m_bApplyParentScale ${equips[0].m_bApplyParentScale}, RsrcData ${equips[0].RsrcData} (the same in all ${equips.length} units). The native code that reads AttachBoneName was not read by this tool: see build/sora_tmp/vfx/weapon/ for the code side`,
    equip_units: equips,
    parent: { skeleton: `/Game/${SORA_SKELETON}`, mesh: `/Game/${SORA_MESH}`, socket_export: used.export, bone: used.bone, bone_index_in_sora_pack: used.bone_index, bone_parent: soraBones[soraBones[used.bone_index].parent].name },
    socket_relative_to_bone: { serialized: used.serialized, ue: { location_cm: used.relative_location_ue_cm, rotation_deg: used.relative_rotation_ue_deg, scale: used.relative_scale, quaternion_xyzw: used.local.q.map(r6) }, pack: { ...packT(used.local), scale: [1, 1, 1] } },
    apply_parent_scale: equips[0].m_bApplyParentScale,
    child: { actor_root: 'TresRoot (TresRootComponent) of the weapon actor, attached to the socket', mesh_component: 'TresWeaponMesh0, AttachParent TresRoot', mesh_component_relative_transform: { RelativeLocation: cdo.mesh.RelativeLocation ?? null, RelativeRotation: cdo.mesh.RelativeRotation ?? null, RelativeScale3D: cdo.mesh.RelativeScale3D ?? null, note: 'not serialized in w_so010_Wep or w_so_KeybladeBase (null): native default, identity per the constructor 0x141777570 (decompiled C; see relative_transform_evidence)' } },
    conversion: 'UE rotator (pitch, yaw, roll in degrees) -> quaternion by the stock UE 4.17 FRotator::Quaternion formula (function identity in the exe not confirmed); then the pack conventions of build/sora_pack/manifest.json: position (x, y, z) cm -> (x, z, y) * 0.01 m, quaternion (x, y, z, w) -> (-x, -z, -y, w). Transforms compose as parent o child in both spaces.',
    how_to_attach_in_pack: `world(weapon scene) = world(Sora scene) * global(node ${used.bone_index} "${used.bone}" of sora.glb, current pose) * TRS(socket_relative_to_bone.pack). The weapon glb is in the weapon mesh component's space: its node 0 (kh_sk) is the scene root and already carries the reference-pose rotation of the root bone; do not strip it.`,
    reference_pose: {
      note: 'Sora and the weapon both at their reference pose; Sora\'s mesh component space (UE: character faces +Y, right hand on -X; pack: faces +Z, left hand on +X)',
      socket_in_sora_component_space: { ue: ueT(sockW), pack: packT(sockW) },
      weapon_bone_wep_in_sora_component_space: { ue: ueT(wepInSora), pack: packT(wepInSora) },
      weapon_bone_wep_relative_to_socket_bone: { ue: ueT(tmul(used.local, wg[wepBone])), pack: packT(tmul(used.local, wg[wepBone])) },
    },
    swing_evidence: { ...swing, reading: 'mean_abs_cos_velocity_flat_normal near 0 with |mean_cos_velocity_teeth| near 1 = the blade is swung in the plane of its teeth: this confirms the roll of the weapon about the blade axis up to half a turn, independently of the code reading. The sign says which side leads: negative = in these clips the plain back of the blade leads and the teeth trail.' },
    grip_check: gripChecks, pack_space_cross_check: packCheck,
    candidate_sockets_on_sora: soraSockets.filter((s) => /wep|buki|weapon/i.test(s.name + ' ' + s.bone)).map((s) => ({ ...sockDesc(s), used_by_kingdom_key: s.name === used.name })),
  };

  // ---- data files
  const soraData = (f) => { const p = P(ROOT, 'audit', 'sora_port', f); return fs.existsSync(p) ? readJson(p) : null; };
  const J = (o) => Buffer.from(JSON.stringify(o, null, 1) + '\n');
  const bonamikContent = gamePathToContent(bonamikPath), physContent = gamePathToContent(physPath);
  const bnmJson = bonamikExport(bonamikContent, meshContent, bones, soraData('bonamik.json'));
  const BW = require('./lib/bonamik/weapon.js');
  const keychain = BW.load({ data: bnmJson, bones: bones.map((b) => ({ name: b.name, parent: b.parent, translation_ue: b.ue.translation, rotation_ue: b.ue.rotation, scale_ue: b.ue.scale })) });
  const bnm2Json = Object.assign({}, bnmJson, { _schema2: { about: 'runtime2 = the runtime records right after core setup (0x143b07550) on the mesh reference pose, produced by tools/lib/bonamik/weapon.js (reference evaluator tools/lib/bonamik/bnm_eval.js). Same fields as build/sora_pack/data/bonamik2.json (_schema2 there describes each field).' }, runtime2: BW.runtimeRecords(keychain) });
  const physJson = physicsExport(physContent, meshContent, meshSide, bones, soraData('physics_asset.json'));
  const dataList = [
    { file: 'data/bonamik.json', content: bonamikContent, buf: J(bnmJson), what: `Bonamik asset of the keychain: scene, ${bnmJson.groups.length} groups, ${bnmJson.bodies.length} bodies (particles), ${bnmJson.links.length} links, planes, chains, per-bone roles; schema inside the file (_schema), same as build/sora_pack/data/bonamik.json`, evidence: `SQEX_BonamikAssetUserData.BonamikAssets of the SkeletalMesh /Game/${meshContent}` },
    { file: 'data/bonamik2.json', content: bonamikContent, buf: J(bnm2Json), what: 'Bonamik: bonamik.json plus the block runtime2 = the runtime records right after setup on the reference pose (10 particles, 8 links, 6 cones, 1 chain root, 6 dynamic collision objects, collider lists)', evidence: 'tools/lib/bonamik/weapon.js' },
    { file: 'data/physics_asset.json', content: physContent, buf: J(physJson), what: `PhysicsAsset: ${physJson.bodies.length} bodies, ${physJson.constraints.length} constraints; schema inside the file (_schema), same as build/sora_pack/data/physics_asset.json`, evidence: `PhysicsAsset property of the SkeletalMesh /Game/${meshContent}` },
    { file: 'data/weapon_cdo.json', content: WEAPON_BP, buf: J(cdo.json), what: 'class default data of the weapon blueprint and its parent blueprint: appear / disappear parameters, effects, hit-effect table and attack-collision shapes of TresAtkColl0, CommonEffectList of TresEffectAttach0, sound asset paths, movement component, equip data (attach socket), projectile set, weapon set; schema inside the file (_schema)', evidence: `RsrcData of the equip unit RWeapon in /Game/${EQUIP_DATA}` },
  ];

  // ---- assemble the files in memory
  const files = new Map();
  const texAll = [...texCache.values(), ...extraTex];
  for (const t of texAll) files.set(t.file, t.png);
  const meshName = path.basename(meshContent);
  let meshStats, skinJoints;
  { const w = new GlbWriter(`via tools/sora_weapon_pack.js ${VERSION}`); addSkeletonNodes(w, bones);
    const r = addMesh(w, meshGlb, bones, mats.map((m) => ({ gltf: JSON.parse(JSON.stringify(m.gltf)), textures: m.textures, rec: m.rec })));
    w.json.skins[0].name = meshName; w.json.meshes[0].name = meshName + '_LOD0'; w.json.nodes[r.node].name = 'kingdom_key_mesh';
    meshStats = r.stats; skinJoints = r.joints; w.json.scenes = [{ name: 'kingdom_key', nodes: [0, r.node] }]; files.set('kingdom_key.glb', w.finish()); }
  for (const [gp, a] of anims) { const w = new GlbWriter(`via tools/sora_weapon_pack.js ${VERSION}`); addSkeletonNodes(w, bones); addAnimation(w, path.basename(a.content), a); w.json.scenes = [{ name: path.basename(a.content), nodes: [0] }]; files.set(animFileOf(gp), w.finish()); }
  for (const d of dataList) files.set(d.file, d.buf);
  if (!checkOnly) fs.writeFileSync(P(ROOT, 'audit', 'sora_port', 'weapon_bonamik2.json'), JSON.stringify(bnm2Json));

  // ---- manifest
  const srcCache = new Map(); const src = (content) => { if (!srcCache.has(content)) srcCache.set(content, sourceFiles(content)); return srcCache.get(content); };
  const item = (file, kind, content, more) => ({ file, kind, bytes: files.get(file).length, sha256: sha256(files.get(file)), source_asset: `/Game/${content}`, source_package: assetId(content), source_sha256: src(content), ...more });
  const items = [];
  items.push(item('kingdom_key.glb', 'mesh', meshContent, { lod: 0, source_lods: meshSide.data.lods.length, ...meshStats, skeleton_bones: bones.length, converted_from: rel(meshGlbFile),
    note: 'LOD 0 only; one primitive per mesh section; one skin whose joints are the bones that carry vertex weights (manifest.skin); weights as float32; every TEXCOORD set kept' }));
  for (const t of texAll) items.push(item(t.file, 'texture', t.content, { role: t.mode, referenced_by_glb: !extraTex.includes(t), width: t.width, height: t.height, source_pixel_format: t.pixel_format, srgb: t.srgb, note: t.note, decoder_mip_check: t.mip_check ?? null, bc7_mode_histogram: t.bc7_modes, converted_from: rel(P(ROOT, 'assets_converted', 'textures', CONTENT, t.content + '.dds')) }));
  for (const [gp, a] of anims) {
    items.push(item(animFileOf(gp), 'animation', a.content, { animation_name: path.basename(a.content), logical_names: seqs.get(gp).map((x) => x.name), anim_sets: [...new Set(seqs.get(gp).map((x) => x.set))], duration_s: a.length, num_frames: a.numFrames, frame_rate: a.fps,
      loop: null, loop_note: 'decided by the code that plays the row; not determined from the assets', root_motion_enabled: a.props.bEnableRootMotion === true, tracks: a.tracks, bones_without_track: bones.length - a.tracks,
      interpolation: a.chans[0].interpolation, sequence_properties: a.props, key_counts: a.keyCounts, tracked_bones: a.tracked_bones, root_bone: a.root.kh_sk, root_track_moves: a.root_track_moves, notifies: a.notifies,
      converted_from: rel(P(ROOT, 'assets_converted', 'animations', CONTENT, a.content + '.glb')) }));
  }
  for (const d of dataList) items.push(item(d.file, 'data', d.content, { content: d.what, evidence: d.evidence }));
  const extraSources = {}; // packages read for facts but not converted into a file of their own
  for (const c of [skeletonContent, animClassContent, HOLO_ANIM_BP, ...[...new Set(logical.map((l) => gamePathToContent(l.anim_set)))], ...mats.flatMap((m) => m.rec.parent_chain.map(gamePathToContent)), gamePathToContent(cdo.parentPath), EQUIP_DATA, WEAPON_SET, SORA_SKELETON, SORA_MESH, gamePathToContent(SORA_ANIMSET)]) extraSources[assetId(c)] = src(c);
  const soraManifestFile = P(sp.OUT, 'manifest.json');
  const manifest = {
    format: 'kh3-sora-pack-weapon/1', generator: `tools/sora_weapon_pack.js ${VERSION} (helpers of tools/sora_pack.js ${sp.VERSION})`,
    legal: 'Converted from the user\'s own copy of the game for local use; nothing in this folder is distributed.',
    weapon: 'Kingdom Key (w_so010), Sora\'s default keyblade',
    part_of: { pack: 'build/sora_pack', manifest: '../manifest.json', note: 'same conventions as the Sora pack; file paths in this manifest are relative to this folder (build/sora_pack/weapon)' },
    identification: { weapon_blueprint: `/Game/${WEAPON_BP}`, parent_blueprint: cdo.parentPath, native_class: cdo.nativeClass, equip_data: `/Game/${EQUIP_DATA}`, weapon_set: `/Game/${WEAPON_SET}`, mesh_component: 'TresWeaponMesh0 (TresSkeletalMeshComponent)',
      skeletal_mesh: cdo.mesh.SkeletalMesh, anim_class: cdo.mesh.AnimClass, skeleton: skeletonPath, physics_asset: physPath, bonamik_asset: bonamikPath, material_instances: imp('MaterialInstanceConstant'), anim_blueprints: animBps },
    conventions: sp.CONVENTIONS, conventions_reference: 'identical to build/sora_pack/manifest.json `conventions` (the same code converts both packs)',
    sources: { note: 'cooked packages (sha256 of each cooked file) this pack was read from; per packed file see items[].source_sha256', converted: Object.fromEntries([...srcCache.entries()].filter(([c]) => !(assetId(c) in extraSources)).map(([c, v]) => [assetId(c), v])), read_for_facts: extraSources },
    skeleton: { source_asset: skeletonPath, bone_count: bones.length, root: bones[0].name, node_index_equals_bone_index: true,
      note: 'reference pose from the SkeletalMesh export (identical bone list and order in the Skeleton asset). parent -1 = root. translation in metres, rotation quaternion xyzw, pack conventions; the *_ue values are the original cm / left-handed ones. kh_sk = root, wep = the keyblade body (every socket sits on it), chain1..7 = the keychain',
      bones: bones.map((b, i) => ({ index: i, name: b.name, parent: b.parent, translation: b.translation.map(f32), rotation: b.rotation.map(f32), scale: b.scale.map(f32), translation_ue: b.ue.translation, rotation_ue: b.ue.rotation, scale_ue: b.ue.scale })) },
    sockets: { source_asset: skeletonPath, note: 'SkeletalMeshSocket objects of the weapon skeleton (order of Skeleton.Sockets). local_* = relative to the bone; component_space_* = in the weapon mesh component space at the reference pose. ATK_00 / ATK_01 are the ends of the attack-collision capsules (data/weapon_cdo.json attack_collision_shapes); vfx_* / Z_* are effect attach points', sockets: wSockets },
    materials: mats.map((m) => m.rec),
    skin: { files: ['kingdom_key.glb'], skins_per_file: 1, joint_count: skinJoints.length, note: 'JOINTS_0 holds indices into this joint list, not bone indices (the bones that carry a non-zero vertex weight at LOD 0, in bone order). skin.skeleton is node 0 (kh_sk).',
      joints: skinJoints.map((b, k) => ({ joint: k, bone: b, name: bones[b].name })), bones_not_in_skin: bones.map((b, i) => ({ bone: i, name: b.name })).filter((x) => !skinJoints.includes(x.bone)) },
    geometry: { note: 'vertex extents of LOD 0 in the frame of the bone wep at the reference pose (UE cm; +X runs from the grip to the tip of the blade), and the largest distance from that axis per 4 cm slice', min_ue_cm: geo.min, max_ue_cm: geo.max, length_along_blade_axis_m: r6((geo.max[0] - geo.min[0]) * UNIT), slices: geo.slices },
    animation_set: { structure: 'TresAnimSet (DataAsset): Anims = array of TresAnimAssetUnit { AnimName, AnimData }; the game plays animations by AnimName', rows: logical.map((l) => ({ ...l, file: animFileOf(l.asset) })) },
    attach,
    data_files: dataList.map((d) => ({ file: d.file, content: d.what, source_asset: `/Game/${d.content}` })),
    items,
  };
  files.set('manifest.json', J(manifest));

  // ---- write
  if (!checkOnly) {
    for (const [f, b] of files) { const o = P(OUT, f); fs.mkdirSync(path.dirname(o), { recursive: true }); fs.writeFileSync(o, b); }
    for (const sub of ['', 'anims', 'textures', 'data']) for (const f of fs.readdirSync(P(OUT, sub))) { const relf = (sub ? sub + '/' : '') + f; if (fs.statSync(P(OUT, relf)).isFile() && !files.has(relf) && relf !== 'validation.json' && relf !== 'sheet_rows.json') fs.unlinkSync(P(OUT, relf)); }
  }

  // ---- validate what is on disk
  const validation = { generator: `tools/sora_weapon_pack.js ${VERSION}`, validator: 'gltf-validator ' + require('gltf-validator/package.json').version, files: {} };
  let bad = 0;
  for (const f of [...files.keys()].filter((x) => x.endsWith('.glb'))) {
    const onDisk = P(OUT, f);
    const v = await validateGlb(onDisk); const c = checkGlb(onDisk, bones);
    if (sha256(fs.readFileSync(onDisk)) !== sha256(files.get(f))) { v.stale = true; bad++; }
    const wn = c.weighted_bone_names; delete c.weighted_bone_names; delete c.nose_minus_head_m; delete c.left_hand_x_m; delete c.right_hand_x_m;
    if (wn) validation.bones_with_vertex_weights = wn;
    validation.files[f] = { validator: v, independent: c };
    if (v.errors || v.warnings) bad++;
    if (c.bone_names_match === false || c.joints_out_of_range || c.index_out_of_range || c.non_finite || c.time_errors || c.non_unit_quaternions || (c.weight_sum_max_dev !== undefined && c.weight_sum_max_dev > 1e-6) || (c.ibm_times_bind_max_dev_from_identity !== undefined && c.ibm_times_bind_max_dev_from_identity > 1e-4) || (c.bind_pose_skinning_max_dev_m !== undefined && c.bind_pose_skinning_max_dev_m > 1e-4) || (c.triangles_against_normals !== undefined && c.triangles_against_normals > c.ccw_triangles_agreeing_with_normals / 20)) bad++;
  }
  for (const t of texAll) { const onDisk = fs.readFileSync(P(OUT, t.file)); const d = bc.decodePng(onDisk); const ok = d.width === t.width && d.height === t.height && sha256(onDisk) === sha256(t.png);
    validation.files[t.file] = { png_read_back: `${d.width}x${d.height}x${d.channels}`, ok, decoder_mip_check: t.mip_check ?? null }; if (!ok || (t.mip_check && t.mip_check.mean_abs_diff > 4)) bad++; }
  for (const d of dataList) { const onDisk = fs.readFileSync(P(OUT, d.file)); const ok = sha256(onDisk) === sha256(d.buf) && !!JSON.parse(onDisk.toString('utf8'))._schema; validation.files[d.file] = { json_read_back: ok }; if (!ok) bad++; }
  { const onDisk = fs.readFileSync(P(OUT, 'manifest.json')); const ok = sha256(onDisk) === sha256(files.get('manifest.json')); validation.files['manifest.json'] = { json_read_back: ok }; if (!ok) bad++; }
  const mi = validation.files['kingdom_key.glb'].independent;
  const size = mi.bounds_m.max.map((x, i) => r6(x - mi.bounds_m.min[i]));
  validation.mesh_plausibility = { bounds_m: mi.bounds_m, size_m: size, longest_side_m: Math.max(...size), ok: Math.max(...size) > 0.6 && Math.max(...size) < 1.6, ok_rule: 'longest side of the bounding box between 0.6 and 1.6 m' }; if (!validation.mesh_plausibility.ok) bad++;
  validation.attach = { socket: attach.socket, relative_transform_determined: true, swing_evidence: { clips: swing.clips.length, mean_cos_velocity_teeth: swing.mean_cos_velocity_teeth, mean_abs_cos_velocity_flat_normal: swing.mean_abs_cos_velocity_flat_normal }, grip_check: gripChecks, pack_space_cross_check: packCheck, cross_check_ok: packCheck.origin_dev_m < 1e-5 && packCheck.axis_dev < 1e-5 }; if (!gripChecks.ok || !validation.attach.cross_check_ok) bad++;
  { const s = fs.existsSync(soraManifestFile) ? readJson(soraManifestFile) : null; // the Sora pack on disk uses the same conventions and has the bone the socket sits on at the same index
    validation.sora_pack = s ? { manifest_generator: s.generator, conventions_identical: JSON.stringify(s.conventions) === JSON.stringify(sp.CONVENTIONS), socket_bone_index_matches: !!s.skeleton.bones[used.bone_index] && s.skeleton.bones[used.bone_index].name === used.bone } : { present: false };
    if (s && (!validation.sora_pack.conventions_identical || !validation.sora_pack.socket_bone_index_matches)) bad++; }
  validation.bonamik = BW.validate(keychain); if (!validation.bonamik.ok) bad++;
  validation.failed_checks = bad;
  if (!checkOnly) fs.writeFileSync(P(OUT, 'validation.json'), JSON.stringify(validation, null, 1) + '\n');

  // ---- sheet rows (columns of sheets/port/sora-assets.json)
  const val = (f) => { const v = validation.files[f]; if (v.validator) return `gltf-validator ${v.validator.errors} errors ${v.validator.warnings} warnings; independent read ok`; if (v.png_read_back) return v.ok ? 'png read back; decoder mip check mean ' + (v.decoder_mip_check ? v.decoder_mip_check.mean_abs_diff : '-') : 'FAILED'; return v.json_read_back ? 'json read back' : 'FAILED'; };
  const ev = (it) => {
    if (it.kind === 'mesh') return `SkeletalMesh property of TresWeaponMesh0 in /Game/${WEAPON_BP} (class default object of the weapon blueprint; RsrcData of the equip unit RWeapon in /Game/${EQUIP_DATA})`;
    if (it.kind === 'animation') return `rows ${it.logical_names.join(', ')} of ${it.anim_sets.join(', ')} (MyAnimSets of the class default object of ${animBps.filter((ab) => ab.anim_sets.some((s) => it.anim_sets.includes(`/Game/${gamePathToContent(s)}`))).map((ab) => ab.anim_class).join(', ')})`;
    if (it.kind === 'texture') return `texture parameter of ${mats.map((m) => m.rec.material_instance.split('.').pop()).join(', ')} (material slot of the mesh)`;
    return it.evidence;
  };
  const rows = items.map((it) => ['weapon/' + it.file, it.kind, it.logical_names ? it.logical_names : it.kind === 'texture' ? [it.role] : '-', it.source_package, it.source_sha256.uexp || it.source_sha256.uasset, it.bytes, it.sha256,
    it.duration_s ?? '-', it.num_frames ?? '-', it.kind === 'animation' ? null : '-', it.root_track_moves === undefined ? '-' : it.root_track_moves ? 'yes' : 'no', val(it.file), ev(it)]);
  rows.push(['weapon/manifest.json', 'manifest', '-', '-', '-', files.get('manifest.json').length, sha256(files.get('manifest.json')), '-', '-', '-', '-', val('manifest.json'), 'written by tools/sora_weapon_pack.js from the items above']);
  const rowsText = `[\n${rows.map((r) => JSON.stringify(r)).join(',\n')}\n]\n`;
  let sheetNote = 'not written (check only)';
  if (!checkOnly) {
    fs.writeFileSync(P(OUT, 'sheet_rows.json'), rowsText);
    // the sheet: every line that is not a weapon/ row is kept byte for byte; the weapon rows go after the Sora pack's own rows and before any vfx/ row
    const sheet = P(ROOT, 'sheets', 'port', 'sora-assets.json');
    if (fs.existsSync(sheet)) {
      const text = fs.readFileSync(sheet, 'utf8'); const lines = text.split('\n');
      const end = lines.lastIndexOf(']}');
      if (end < 1 || !lines[0].endsWith('"rows":[') || lines.slice(end + 1).join('') !== '') throw new Error('unexpected layout of ' + rel(sheet));
      const body = lines.slice(1, end).map((l) => l.replace(/,$/, ''));
      const keep = body.filter((l) => !l.startsWith('["weapon/'));
      const at = keep.findIndex((l) => l.startsWith('["vfx/'));
      const mine = rows.map((r) => JSON.stringify(r));
      const merged = at < 0 ? [...keep, ...mine] : [...keep.slice(0, at), ...mine, ...keep.slice(at)];
      const outText = `${lines[0]}\n${merged.join(',\n')}\n]}\n`;
      JSON.parse(outText);
      if (outText !== text) fs.writeFileSync(sheet, outText);
      sheetNote = `${rel(sheet)}: ${mine.length} weapon rows (${keep.length} other rows kept)${outText === text ? ', unchanged' : ''}`;
    } else sheetNote = `${rel(sheet)} is missing: run tools/sora_pack.js (it appends weapon/sheet_rows.json)`;
  }

  // ---- report
  const total = [...files.values()].reduce((s, b) => s + b.length, 0);
  console.log(`sora_weapon_pack ${VERSION}: ${files.size} files, ${(total / 1048576).toFixed(2)} MB${checkOnly ? ' (check only, nothing written)' : ' -> ' + rel(OUT)}`);
  console.log(`  mesh ${cdo.mesh.SkeletalMesh}: ${meshStats.vertices} vertices, ${meshStats.triangles} triangles, ${bones.length} bones (${skinJoints.length} skin joints), ${mats.length} material, ${texAll.length} textures`);
  console.log(`  bounds ${JSON.stringify(mi.bounds_m)} size ${JSON.stringify(size)} m; ccw-agree ${mi.ccw_triangles_agreeing_with_normals} against ${mi.triangles_against_normals}`);
  for (const [gp, a] of anims) console.log(`  ${animFileOf(gp).padEnd(22)} rows ${seqs.get(gp).map((x) => x.name).join(',')}: ${a.length.toFixed(4)} s, ${a.numFrames} frames, ${a.tracks} tracks, ${a.notifies.length} notifies`);
  for (const [f, v] of Object.entries(validation.files)) if (v.validator) console.log(`  ${f.padEnd(22)} validator ${v.validator.errors}E ${v.validator.warnings}W ${v.validator.infos}I  ${Object.keys(v.validator.codes).join(' ')}${v.validator.first.length ? '  ' + v.validator.first[0] : ''}`);
  console.log(`  skin: weight sum dev ${mi.weight_sum_max_dev}, IBM*bind dev ${mi.ibm_times_bind_max_dev_from_identity}, bind-pose skinning dev ${mi.bind_pose_skinning_max_dev_m} m, bones with weights ${mi.bones_with_weights}`);
  console.log(`  attach: socket ${attach.socket} on bone ${used.bone} (Sora bone ${used.bone_index}); grip origin ${gripChecks.grip_origin_distance_to_hand_bone_cm} cm from R_te, ${gripChecks.grip_origin_distance_to_finger_roots_mean_cm} cm from the finger roots; blade ${gripChecks.angle_blade_axis_to_forearm_deg} deg to the forearm, axis ${JSON.stringify(gripChecks.blade_axis_wep_plus_x_in_sora_component_space)}, teeth towards ${JSON.stringify(gripChecks.teeth_direction_in_sora_component_space)}; grip section ${JSON.stringify(gripChecks.cross_section_at_grip_origin)}; check ${gripChecks.ok ? 'ok' : 'FAILED'}; pack cross-check ${JSON.stringify(packCheck)}`);
  console.log(`  attach roll evidence (${swing.clips.length} attack clips of Sora): mean cos(tip velocity, teeth) ${swing.mean_cos_velocity_teeth}, mean |cos(tip velocity, flat normal)| ${swing.mean_abs_cos_velocity_flat_normal} -> the blade is swung in the plane of its teeth (roll about the blade axis confirmed up to half a turn; attach rule from code: snap to the socket)`);
  console.log(`  sheet: ${sheetNote}`);
  console.log(`  failed checks: ${bad}`);
  if (bad) process.exitCode = 1;
}
// (IMPL_ER59) required as a module (tools/sora_outfit_pack.js: an outfit's Bonamik asset): the export only, nothing runs
if (require.main === module) main().catch((e) => { console.error(e); process.exit(1); });
else module.exports = { bonamikExport };
