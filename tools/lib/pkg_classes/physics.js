// Physics: UBodySetup / USkeletalBodySetup (cooked PhysX collision), UPhysicsAsset,
// UPhysicsSerializer, UNavCollision. Docs: docs/formats/physics.md.
const { T, register } = require('../package');

// FPhysXFormatDataReader stream: endian flag and mesh counts, then per mesh a "cooked" byte and a
// PhysX cooked stream. The PhysX streams carry no length, so they are kept together as one
// third-party payload (magic "NXS\1" + "CVXM" convex mesh / "MESH" triangle mesh).
const physx = T.struct([
  ['littleEndian', T.b8], ['numConvexElementsCooked', T.i32], ['numMirroredElementsCooked', T.i32], ['numTriMeshesCooked', T.i32],
  ['meshStreams', T.foreign('PhysX 3.4 cooked collision streams (NXS CVXM / NXS MESH)')],
]);
const bodySetup = T.struct([
  ['bodySetupGuid', T.guid], ['cooked', T.b32],
  ['hasCookedCollisionData', T.when((o) => o.cooked, T.b32)],
  ['cookedFormatData', T.when((o) => o.cooked, T.formatContainer({ PhysXPC: physx }))],
]);
register(['BodySetup', 'SkeletalBodySetup'], bodySetup, 'physics');

// UPhysicsAsset: TMap<FRigidBodyIndexPair, bool> CollisionDisableTable
register('PhysicsAsset', T.struct([['collisionDisableTable', T.arr(T.struct([['bodyIndex0', T.i32], ['bodyIndex1', T.i32], ['value', T.b32]]), 12)]]), 'physics');

// UPhysicsSerializer: bCooked + FFormatContainer of serialized PhysX collections
register('PhysicsSerializer', T.struct([['cooked', T.b32], ['binaryFormatData', T.when((o) => o.cooked, T.formatContainer({ PhysXPC: T.foreign('PhysX 3.4 binary collection') }))]]), 'physics');

// UNavCollision cooked payload (FNavCollisionDataReader): endian flag, triangle mesh and convex
// vertex/index buffers, convex shape start indexes. FVector = 12 bytes, index = int32.
const navPayload = T.struct([
  ['littleEndian', T.b8],
  ['triMeshVertices', T.packed(12)], ['triMeshIndices', T.packed(4)],
  ['convexVertices', T.packed(12)], ['convexIndices', T.packed(4)],
  ['convexShapeIndices', T.arr(T.i32, 4)],
]);
// The game's UNavCollision::Serialize (func_143EABEB0) reads the container only when
// bGatherConvexGeometry is set or both CylinderCollision and BoxCollision are empty, then reads
// AreaClass when the version is above 1. bGatherConvexGeometry is absent from every export's
// properties; the one export with non-empty collision arrays has no container, so its default is false.
const propValue = (c, name) => { const p = (c.props || []).find((x) => x.name === name); return p ? p.value : undefined; };
const hasNavData = (o, c) => {
  const count = (v) => (v === undefined ? 0 : v.items.length);
  return o.cooked && (propValue(c, 'bGatherConvexGeometry') === true || (count(propValue(c, 'CylinderCollision')) === 0 && count(propValue(c, 'BoxCollision')) === 0));
};
register('NavCollision', T.struct([
  ['magic', T.constant(T.u32, 0xa237f237)], ['version', T.i32], ['bodySetupGuid', T.guid], ['cooked', T.b32],
  ['cookedFormatData', T.when(hasNavData, T.formatContainer({ NavCollision_X: navPayload }))],
  ['areaClass', T.when((o) => o.version >= 2, T.obj)],
]), 'physics');
