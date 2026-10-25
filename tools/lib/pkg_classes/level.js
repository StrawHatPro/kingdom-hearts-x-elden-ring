// Maps: UWorld, ULevel, UModel (BSP), UModelComponent. Docs: docs/formats/level.md.
const { T, register } = require('../package');

register('World', T.struct([['persistentLevel', T.obj], ['extraReferencedObjects', T.arr(T.obj, 4)], ['streamingLevels', T.arr(T.obj, 4)]]), 'level');

const furl = T.struct([['protocol', T.str], ['host', T.str], ['map', T.str], ['portal', T.str], ['op', T.arr(T.str, 4)], ['port', T.i32], ['valid', T.i32]]);
// FPrecomputedVisibilityHandler
const visCell = T.struct([['min', T.vec3], ['chunkIndex', T.u16], ['dataOffset', T.u16]]);
const visChunk = T.struct([['compressed', T.b32], ['uncompressedSize', T.i32], ['data', T.bytes]]);
const visBucket = T.struct([['cellDataSize', T.i32], ['cells', T.arr(visCell, 16)], ['cellDataChunks', T.arr(visChunk, 12)]]);
const visibility = T.struct([
  ['cellBucketOriginXY', T.vec2], ['cellSizeXY', T.f32], ['cellSizeZ', T.f32], ['cellBucketSizeXY', T.i32], ['numCellBuckets', T.i32],
  ['cellBuckets', T.arr(visBucket, 12)],
]);
// FPrecomputedVolumeDistanceField: Data is TArray<FColor>, kept packed (4 bytes B,G,R,A)
const volumeDistanceField = T.struct([['volumeMaxDistance', T.f32], ['volumeBox', T.box], ['sizeX', T.i32], ['sizeY', T.i32], ['sizeZ', T.i32], ['data', T.packed(4)]]);
register('Level', T.struct([
  ['actors', T.arr(T.obj, 4)], ['url', furl], ['model', T.obj], ['modelComponents', T.arr(T.obj, 4)],
  ['levelScriptActor', T.obj], ['navListStart', T.obj], ['navListEnd', T.obj],
  ['precomputedVisibilityHandler', visibility], ['precomputedVolumeDistanceField', volumeDistanceField],
]), 'level');

// UModelComponent: Model, Elements (FModelElement), ComponentIndex, Nodes. One export in the corpus.
const modelElement = T.struct([['mapBuildDataId', T.guid], ['component', T.obj], ['material', T.obj], ['nodes', T.arr(T.u16, 2)]]);
register('ModelComponent', T.struct([['model', T.obj], ['elements', T.arr(modelElement, 28)], ['componentIndex', T.i32], ['nodes', T.arr(T.u16, 2)]]), 'level');

// UModel. BulkSerialize arrays are raw in-memory structs.
const bspNode = T.struct([ // FBspNode, 64 bytes
  ['plane', T.vec4], ['iVertPool', T.i32], ['iSurf', T.i32], ['iVertexIndex', T.i32], ['componentIndex', T.u16], ['componentNodeIndex', T.u16],
  ['componentElementIndex', T.i32], ['iBack', T.i32], ['iFront', T.i32], ['iPlane', T.i32], ['iCollisionBound', T.i32],
  ['iZone', T.fixed(T.u8, 2)], ['numVertices', T.u8], ['nodeFlags', T.u8], ['iLeaf', T.fixed(T.i32, 2)],
]);
const bspSurf = T.struct([
  ['material', T.obj], ['polyFlags', T.u32], ['pBase', T.i32], ['vNormal', T.i32], ['vTextureU', T.i32], ['vTextureV', T.i32],
  ['iBrushPoly', T.i32], ['actor', T.obj], ['plane', T.vec4], ['lightMapScale', T.f32], ['iLightmassIndex', T.i32],
]);
const vert = T.struct([['pVertex', T.i32], ['iSide', T.i32], ['shadowTexCoord', T.vec2], ['backfaceShadowTexCoord', T.vec2]]); // FVert, 24 bytes
// FModelVertex, 36 bytes: Position, TangentX, TangentZ (FPackedNormal), TexCoord, ShadowTexCoord
const modelVertex = T.struct([['position', T.vec3], ['tangentX', T.u32], ['tangentZ', T.u32], ['texCoord', T.vec2], ['shadowTexCoord', T.vec2]]);
const lightmassSettings = T.struct([
  ['useTwoSidedLighting', T.b32], ['shadowIndirectOnly', T.b32], ['fullyOccludedSamplesFraction', T.f32], ['useEmissiveForStaticLighting', T.b32],
  ['useVertexNormalForHemisphereGather', T.b32], ['emissiveLightFalloffExponent', T.f32], ['emissiveLightExplicitInfluenceRadius', T.f32],
  ['emissiveBoost', T.f32], ['diffuseBoost', T.f32],
]);
register('Model', T.struct([
  ['strip', T.strip], ['bounds', T.boxSphereBounds],
  ['vectors', T.bulkArr(T.vec3, 12)], ['points', T.bulkArr(T.vec3, 12)], ['nodes', T.bulkArr(bspNode, 64)],
  ['surfs', T.arr(bspSurf, 56)], ['verts', T.bulkArr(vert, 24)],
  ['numSharedSides', T.i32], ['rootOutside', T.b32], ['linked', T.b32], ['numUniqueVertices', T.i32],
  ['vertexBuffer', T.when((o) => !(o.strip.global & 1) || !(o.strip.cls & 1), T.bulkArr(modelVertex, 36))],
  ['lightingGuid', T.guid], ['lightmassSettings', T.arr(lightmassSettings, 36)],
]), 'level');
