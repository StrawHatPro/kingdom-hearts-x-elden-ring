// Builds build/sora_tmp/sora_bonamik.json from the decoded p_ex001_Bonamik package + skeleton manifest.
// usage: node build/sora_tmp/bnm_extract.js
const fs = require('fs'), path = require('path'); const R = path.resolve(__dirname, '../..');
const { conv, load } = require('./bnm_plain.js');
const F = path.join(R, 'assets_converted/packages/KINGDOM HEARTS III/Content/Character/pc/p_ex001/mdl/p_ex001_Bonamik.uasset.json');
const fix = x => { // {f32bits} -> number
  if (x && typeof x === 'object') {
    if ('f32bits' in x) { const b = Buffer.alloc(4); b.writeUInt32LE(x.f32bits >>> 0); return b.readFloatLE(0); }
    if (Array.isArray(x)) return x.map(fix); const o = {}; for (const k in x) o[k] = fix(x[k]); return o;
  } return x;
};
const A = fix(conv(load(F).exports[0].data.props));
const man = JSON.parse(fs.readFileSync(path.join(R, 'build/sora_pack/manifest.json'), 'utf8')); const bones = man.skeleton.bones;
const bi = Object.fromEntries(bones.map(b => [b.name, b.index]));
const strip = (s, p) => typeof s === 'string' ? s.replace(p, '') : s;
const v3 = v => [v.x, v.y, v.z], q4 = q => [q.x, q.y, q.z, q.w];
const groups = A.m_Solvers.map(s => ({ id: s.m_GroupId, name: s.m_GroupName, enable: s.m_IsEnable, useConeSEQ: s.m_IsUseConeSEQ, colIter: s.m_ColIter, consIter: s.m_ConsIter, coneIter: s.m_ConeIter, linkIter: s.m_LinkIter, solverIter: s.m_SolverIter, tagId: s.m_TagId, collisionTagId: s.m_CollisionTagId, enableGround: s.m_EnableGround, lodDistance: s.m_LODdistance, refUpVec: s.m_RefUpVec, fpsCorrection: s.m_FpsCorrection || [] }));
const bodies = A.m_Bodies.map((b, i) => ({
  index: b.m_Index, joint: b.m_JointName, bone_index: bi[b.m_JointName] ?? -1, parentBody: b.m_ParentId, childBody: b.m_ChildId, parentBodyName: b.m_ParentBodyName, childBodyName: b.m_ChildBodyName, group: b.m_GroupId, groupName: groups[b.m_GroupId].name,
  kinematic: b.m_IsKinematic, collision: b.m_IsCollision, multipleCollisionObject: b.m_IsMultipleCollisionObject, colId: b.m_ColId,
  mass: b.m_Mass, inertia: b.m_Inertia, radius: b.m_Radius, damping: b.m_Damping, bounciness: b.m_Bounciness, friction: b.m_Friction, windDrag: b.m_WindDrag, windLift: b.m_WindLift,
  localTransCoeff: b.m_LocalTransCoeff, maxLocalTrans: b.m_MaxLocalTrans, colShape: strip(b.m_ColShape, 'ESQEX_Bonamik_CollisionShape_'), addiConstType: strip(b.m_AddiConstType, 'ESQEX_Bonamik_ConstraintType_'), addiConstStr: b.m_AddiConstStr, addiConstRad: b.m_AddiConstRad,
  localTranslation: v3(b.m_LocalTransform), localRotation: q4(b.m_LocalRotation), localScale: v3(b.m_LocalScale), collisionBatchNo: b.m_CollisionBatchNo, concaveRatio: b.m_ConcaveRatio,
  lfCoeff: b.m_LFcoeff, parkRate: b.m_ParkRate, maxLF: b.m_MaxLF, lfDrag: b.m_LFdrag, lfLift: b.m_LFlift
}));
if (bodies.some((b, i) => b.index !== i)) throw new Error('body index mismatch');
const links = A.m_Links.map((l, i) => ({
  index: i, parent: l.m_ParentId, child: l.m_ChildId, parentName: l.m_ParentBodyName, childName: l.m_ChildBodyName, type: strip(l.m_LinkType, 'ESQEX_Bonamik_LinkType_'), selfCollision: l.m_IsSelfCollision, jointTwist: l.m_EnableJointTwist, cone: l.m_EnableCone, coneAnimation: l.m_EnableConeAnimation,
  linkStr: l.m_LinkStr, linkParentStr: l.m_LinkParentStr, coneInnerStr: l.m_ConeInnerStr, coneOuterStr: l.m_ConeOuterStr, coneParentStr: l.m_ConeParentStr, limitYdeg: l.m_LimitY, limitZdeg: l.m_LimitZ, coneOffsetYdeg: l.m_ConeOffsetY, coneOffsetZdeg: l.m_ConeOffsetZ, limitOffsetYdeg: l.m_LimitOffsetY, limitOffsetZdeg: l.m_LimitOffsetZ,
  collisionReceiver: l.m_IsCollisionReceiver, collisionRepulsor: l.m_IsCollisionRepulsor, lra: l.m_IsLRA, lraStr: l.m_LRAstr, lraDamping: l.m_LRAdamping, linkBatchNo: l.m_LinkBatchNo, coneBatchNo: l.m_ConeBatchNo, collisionBatchNo: l.m_CollisionBatchNo, pinRadius: l.m_PinRadius, pinStr: l.m_PinStr, kdiTrans: l.m_KdiTrans, kdiRot: l.m_KdiRot, concaveRatio: l.m_ConcaveRatio
}));
const planes = (A.m_Planes || []).map(p => ({ index: p.m_Index, group: p.m_GroupId, particleA: p.m_ParticleA, particleB: p.m_ParticleB, particleC: p.m_ParticleC, names: [p.m_ParticleAName, p.m_ParticleBName, p.m_ParticleCName], thickness: p.m_Thickness, offset: p.m_Offset, collision: p.m_IsCollision, collisionBatchNo: p.m_CollisionBatchNo, colId: p.m_ColId }));
// ---- runtime topology as built by 0x143b07550 (see native_G_bonamik.md)
const P = bodies.map(b => ({ b, kin: b.kinematic, col: b.collision, parent: b.parentBody, child: b.childBody, pc: [], bc: false, addi: !(b.addiConstType === 'None' || b.addiConstType === 'GroundAttach') }));
for (const l of links) {
  if (l.selfCollision || l.lra) continue; if (['ReactionTransfer', 'PinConstraint', 'KdiConstraint'].includes(l.type)) continue;
  if (l.type === 'ParentChild') { const p = P[l.parent]; if (p.pc.length < 4) p.pc.push(l.index); p.child = l.child; P[l.child].bc = true; }
}
const roots = [];
P.forEach((p, i) => {
  if (p.col) return;
  if (p.parent < 0) { for (const li of p.pc) { if (!P[links[li].child].kin) { if (p.kin || p.addi) roots.push(i); break; } } }
  else {
    const par = P[p.parent]; const ch = p.child >= 0 ? P[p.child] : null;
    if ((!p.bc || (p.kin && par.kin)) && ((ch && !ch.kin && p.kin) || (!p.kin && p.addi))) { p.bc = false; roots.push(i); }
  }
});
const chainOf = i => { const p = P[i]; return { body: i, joint: bodies[i].joint, kinematic: p.kin, children: p.pc.map(li => ({ link: li, cone: links[li].cone, ...chainOf(links[li].child) })) }; };
// driven bones (0x143b11700): node bones + kinematic parents whose PC child is dynamic; followers = skeleton descendants
const role = new Array(bones.length).fill(null), body = new Array(bones.length).fill(-1);
P.forEach((p, i) => {
  if (p.col || !groups[p.b.group].enable || p.b.multipleCollisionObject) return; const bidx = p.b.bone_index; if (bidx < 0) return;
  if (!p.kin) { role[bidx] = 'simulated_node'; body[bidx] = i; }
  else if (p.child >= 0 && !P[p.child].kin && P[p.child].bc) { if (role[bidx] !== 'simulated_node') { role[bidx] = 'kinematic_parent_rotation'; body[bidx] = i; } }
});
const drivenRoot = role.map(r => !!r);
bones.forEach(b => { if (role[b.index]) return; let p = b.parent; while (p >= 0) { if (drivenRoot[p]) { role[b.index] = 'follower'; break; } p = bones[p].parent; } });
const boneTable = bones.map(b => ({ index: b.index, name: b.name, parent: b.parent, bonamik: role[b.index] || 'none', body: body[b.index], group: body[b.index] >= 0 ? bodies[body[b.index]].groupName : null, bodies_on_bone: bodies.filter(x => x.bone_index === b.index).map(x => x.index) }));
const count = (a, f) => { const c = {}; a.forEach(x => { const k = f(x); c[k] = (c[k] || 0) + 1; }); return c; };
let notif = null; try { notif = JSON.parse(fs.readFileSync(path.join(__dirname, 'bnmsub_notifies.json'), 'utf8')); } catch (e) { }
const gnames = new Set(groups.map(g => g.name));
// annotate which group names used by notifies exist in this asset
const usedNames = new Set(); const walk = x => { if (typeof x === 'string') usedNames.add(x); else if (Array.isArray(x)) x.forEach(walk); else if (x && typeof x === 'object') for (const k in x) { if (/group/i.test(k)) walk(x[k]); else if (typeof x[k] === 'object') walk2(x[k]); } };
const walk2 = x => { if (Array.isArray(x)) x.forEach(walk2); else if (x && typeof x === 'object') for (const k in x) { if (/group/i.test(k)) walk(x[k]); else if (typeof x[k] === 'object') walk2(x[k]); } };
if (notif) walk2(notif);
const S = A.m_SceneManager;
const out = {
  _schema: {
    about: 'Sora (p_ex001) Bonamik data. Source: Content/Character/pc/p_ex001/mdl/p_ex001_Bonamik (class SQEX_BonamikAsset, m_VersionNo 2401) + mesh user data. Units: cm, degrees where the key ends in deg; runtime positions are in mesh component space. Quaternions [x,y,z,w]. scene.gravity is in the authoring convention (Y-up, m/s^2).',
    global_config: 'SQEX_BonamikGlobalConfig constructor defaults (0x143AE8A30); DefaultEngine.ini only sets bMultiThreadUpdate=true and OutOfCamera=Override.',
    scene: 'SQEX_BonamikSceneManagerDesc as serialized; *Batches are the authoring-time batch layout.',
    runtime: 'values the runtime derives at setup (0x143b07550 / 0x143b61cf0) for this asset.',
    groups: 'm_Solvers[31]; array index = group id used by bodies/planes. enable=false: group starts in state 0 with animation weight 1 (not simulated).',
    bodies: 'm_Bodies[252] = particles. joint = skeleton bone the particle sits on (bone_index into build/sora_pack/manifest.json skeleton.bones). Rest/kinematic position = bone.T + bone.R*(bone.S*localTranslation); orientation = bone.R*localRotation. kinematic=true: re-set from the animated bone every evaluation. collision=true: collider only (colShape, radius; Capsule = segment to childBody; Plane = oriented plane). colId[8]: group ids of the collider groups a particle collides with (-1 unused). addiConstType Sphere/Spring: anchor constraint to the particle own animated bone position (radius addiConstRad, strength addiConstStr). localTransCoeff/maxLocalTrans are not read by the 2401 runtime (lfCoeff/maxLF/parkRate/lfDrag/lfLift are).',
    links: 'm_Links[189]. Lateral/Bending/ParentChild-without-cone = distance springs solved Jacobi style (linkStr stiffness, linkParentStr = share applied to the child side). ParentChild = chain segment walked root to tip; cone=true adds the cone constraint (limitY/limitZ half angles, cone/limit offsets, coneInnerStr/coneOuterStr/coneParentStr); coneAnimation=true: cone axis re-derived from the animated pose every evaluation. selfCollision=true rows are self-collision pairs, not springs. KdiConstraint: kinematic driven interpolation (1 row).',
    planes: 'm_Planes: triangles of three particles used as limit planes.',
    chains: 'chain roots and parent-child trees as the runtime builds them (rules read from 0x143b07550, with the two pointer tests particle+0x238/+0x240 taken as null). This yields 37 roots; the asset field totals.chainRootParticles is 49 (used by the runtime only as array capacity) - the difference of 12 is not explained, see native_G_bonamik.md.',
    bones: 'one row per skeleton bone (274). bonamik: simulated_node = a dynamic particle sits on this bone, component-space translation AND rotation are written (0x143b0c600/0x143b13b70); kinematic_parent_rotation = animated bone whose rotation is re-aimed at its simulated child, translation stays animated; follower = no particle, skeleton descendant of a driven bone, carried rigidly (0x143afc920); none = untouched.',
    notify_states: 'per stage-1 clip: Bonamik notify states with decoded properties (from build/sora_tmp/bnmsub_notifies.json). notify_group_names_present lists, for every group name the notifies use, whether this asset has such a group (a name that matches no group is ignored by the runtime).'
  },
  source: { package: '/Game/Character/pc/p_ex001/mdl/p_ex001_Bonamik', class: 'SQEX_BonamikAsset', versionNo: A.m_VersionNo },
  user_data: { class: 'SQEX_BonamikAssetUserData', mesh: '/Game/Character/pc/p_ex001/mdl/p_ex001_0', BonamikAssets: ['/Game/Character/pc/p_ex001/mdl/p_ex001_Bonamik'], TeleportDistance: 800, note: 'all other properties at class default (not serialized)' },
  global_config: { bEnableTickComponent: true, bEnableUpdate: true, bMultiThreadUpdate: true, bEnableSimulation: true, bEnableUpdateOverlaps: true, TimeStep: 1 / 30, ExpectedFrameRate: 30, MayaAnimationFrameRate: 30, MayaSimulationFrameRate: 20, Gravity: [0, 0, -981], WindSourceSpeedFactor: 100, bTickWindWhenPaused: true, OutOfCamera: 'Override', bKBDEnable: true, bKBDEnableInitializaiton: true, bKBDEnableCreationOfBonamikComponent: true, SkipUpdateForActorType: 255, SkipWaitForActorType: 255 },
  scene: {
    isEnable: S.m_IsEnable, subStep: S.m_SubStep, totalGroup: S.m_TotalGroup, lssCoeffTran: S.m_LSScoeff_tran, lssCoeffRot: S.m_LSScoeff_rot, rootName: S.m_RootName, gravity: v3(S.m_Gravity), fastMotionHandlingMode: strip(S.m_FastMotionHandlingMode, 'ESQEX_Bonamik_FMH_'), jointLinkSimulationMode: S.m_JointLinkSimulationMode ? strip(S.m_JointLinkSimulationMode, 'ESQEX_Bonamik_JOINTLINK_MODE_') : 'SEQ (not serialized = default 0)', enableGroundCollision: S.m_EnableGroundCollision, refUpVec: S.m_RefUpVec,
    linkBatches: S.m_TotalLinksInBatch, coneBatches: S.m_TotalConesInBatch, collisionBatches: S.m_TotalCollisionsInBatch, selfCollisionBatches: S.m_TotalSelfCollisionsInBatch, fpsCorrection: S.m_FpsCorrection,
    totals: { kinematics: A.m_TotalKinematics, constraints: A.m_TotalConstraints, cones: A.m_TotalCones, links: A.m_TotalLinks, collisions: A.m_TotalCollisions, selfCollisions: A.m_TotalSelfCollisions, chainRootParticles: A.m_TotalChainRootParticles, lateralLinkCones: A.m_TotalLateralLinkCones }
  },
  runtime: {
    substeps: S.m_SubStep, outerIterations: Math.max(...groups.map(g => g.solverIter)), linkIterations: Math.max(...groups.map(g => g.linkIter)), collisionIterations: 1, fastMotionMode: 1, teleportDistance: 800, teleportDistanceSq: 640000, gravityMagnitude: 981, gravityDirAsset: [0, -1, 0],
    evaluation_at_30Hz: { stepSeconds: 1 / 30, dtSim: 0.05, frameRatio_r: 1.0, h_over_r: 0.05 },
    counts: { bodies: bodies.length, dynamic: bodies.filter(b => !b.kinematic).length, kinematic: bodies.filter(b => b.kinematic).length, colliders: bodies.filter(b => b.collision).length, byShape: count(bodies, b => b.colShape + (b.collision ? '/collider' : b.kinematic ? '/kinematic' : '/dynamic')), linksByType: count(links, l => l.selfCollision ? 'SelfCollisionPair' : l.type + (l.cone ? '+cone' : '')), addiConstraints: count(bodies, b => b.addiConstType), chainRoots: roots.length, dynamicByGroup: count(bodies.filter(b => !b.kinematic), b => b.groupName) }
  },
  groups, bodies, links, planes,
  chains: roots.map(chainOf),
  bones: boneTable,
  notify_states: notif ? notif.clips : null,
  notify_group_names_present: Object.fromEntries([...usedNames].sort().map(n => [n, gnames.has(n)]))
};
fs.writeFileSync(path.join(__dirname, 'sora_bonamik.json'), JSON.stringify(out, null, 1));
console.log(JSON.stringify(out.runtime.counts)); console.log('roots', roots.length, 'expected', A.m_TotalChainRootParticles);
console.log('bone roles', JSON.stringify(count(boneTable, b => b.bonamik)));
for (const r of ['simulated_node', 'kinematic_parent_rotation', 'follower']) console.log(r + ':', boneTable.filter(b => b.bonamik === r).map(b => b.name).join(' '));
console.log('notify groups', JSON.stringify(out.notify_group_names_present));
