// Landscape: ULandscapeComponent (grass data + cook flags), ULandscapeHeightfieldCollisionComponent
// and ULandscapeMeshCollisionComponent (cooked PhysX collision). Docs: docs/formats/landscape.md.
const { T, register } = require('../package');

// FLandscapeComponentGrassData: HeightData (uint16, BulkSerialize), WeightData TMap<ULandscapeGrassType*, TArray<uint8>>
register('LandscapeComponent', T.struct([
  ['grassHeightData', T.packedBulk(2)],
  ['grassWeightData', T.map(T.obj, T.bytes, 8)],
  ['cooked', T.b32],
  ['cookedMobileData', T.when((o) => o.cooked, T.constant(T.u32, 0))],
]), 'landscape');

// bCooked, then TArray<uint8>::BulkSerialize of the PhysX cooked data
const collision = (format) => T.struct([
  ['cooked', T.b32],
  ['cookedElementSize', T.when((o) => o.cooked, T.constant(T.i32, 1))],
  ['cookedCollisionData', T.when((o) => o.cooked, T.foreign(format, T.i32))],
]);
register('LandscapeHeightfieldCollisionComponent', collision('PhysX 3.4 cooked heightfield (NXS HFHF)'), 'landscape');
// ULandscapeMeshCollisionComponent adds its own bCooked after the base class data
const meshBase = collision('PhysX 3.4 cooked triangle mesh (NXS MESH)');
register('LandscapeMeshCollisionComponent', {
  r: (r, c) => { const v = meshBase.r(r, c); v.meshCooked = T.b32.r(r, c); return v; },
  w: (w, v, c) => { meshBase.w(w, v, c); T.b32.w(w, v.meshCooked, c); },
}, 'landscape');
