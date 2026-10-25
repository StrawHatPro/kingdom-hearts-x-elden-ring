// Scene components with native data: UStaticMeshComponent (per-LOD override vertex colours),
// UInstancedStaticMeshComponent (instances), UHierarchicalInstancedStaticMeshComponent (cluster
// tree), AInstancedFoliageActor. Docs: docs/formats/components.md.
const { T, register } = require('../package');

// FColorVertexBuffer::Serialize
const colorVertexBuffer = T.struct([
  ['strip', T.strip], ['stride', T.i32], ['numVertices', T.i32],
  // FColor per vertex (B, G, R, A), kept packed
  ['colors', T.when((o) => !(o.strip.global & 2) && o.numVertices > 0, T.packedBulk(4))],
]);
// FStaticMeshComponentLODInfo::Serialize. Class strip flag 1 = override colours stripped.
const lodInfo = T.struct([
  ['strip', T.strip],
  ['mapBuildDataId', T.when((o) => !(o.strip.global & 2), T.guid)],
  ['hasOverrideVertexColors', T.when((o) => !(o.strip.cls & 1), T.b8)],
  ['overrideVertexColors', T.when((o) => o.hasOverrideVertexColors === true, colorVertexBuffer)],
  ['_editorDataStripped', (o) => { if (!(o.strip.global & 1)) throw new Error('LOD info with editor data'); return null; }],
]);
const smc = [['lodData', T.arr(lodInfo, 2)]];
register(['StaticMeshComponent', 'TresStaticMeshComponent', 'SplineMeshComponent', 'TresOceanGridMeshComponent'], T.struct(smc), 'components');

// FInstancedStaticMeshInstanceData (80 bytes): FMatrix Transform, FVector2D LightmapUVBias, FVector2D ShadowmapUVBias
const instance = T.struct([['transform', T.fixed(T.f32, 16)], ['lightmapUVBias', T.vec2], ['shadowmapUVBias', T.vec2]]);
const ismc = [...smc, ['perInstanceSMData', T.bulkArr(instance, 80)]];
register(['InstancedStaticMeshComponent', 'TresInstancedStaticMeshComponent'], T.struct(ismc), 'components');

// FClusterNode (40 bytes)
const clusterNode = T.struct([['boundMin', T.vec3], ['firstChild', T.i32], ['boundMax', T.vec3], ['lastChild', T.i32], ['firstInstance', T.i32], ['lastInstance', T.i32]]);
register(['HierarchicalInstancedStaticMeshComponent', 'FoliageInstancedStaticMeshComponent'], T.struct([...ismc, ['clusterTree', T.bulkArr(clusterNode, 40)]]), 'components');

// AInstancedFoliageActor: TMap<UFoliageType*, FFoliageMeshInfo>; cooked FFoliageMeshInfo is its component reference
register('InstancedFoliageActor', T.struct([['foliageMeshes', T.map(T.obj, T.obj, 8)]]), 'components');
