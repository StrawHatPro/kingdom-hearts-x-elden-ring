# Scene components with native data

Code: `tools/lib/pkg_classes/components.js`. Evidence: the classes below in
`audit/package_manifest.json`. All follow the common export framing (`package_json.md`).

## StaticMeshComponent (also TresStaticMeshComponent, SplineMeshComponent, TresOceanGridMeshComponent)

```
int32 count + FStaticMeshComponentLODInfo
```

| Field | Type | Present when |
|---|---|---|
| strip | 2 × u8 (FStripDataFlags: global, class) | always; global is 1 (editor data stripped) in every export |
| MapBuildDataId | 16-byte guid | global bit 2 (server strip) clear |
| hasOverrideVertexColors | u8 bool | class bit 1 clear (bit 1 = override colours stripped) |
| OverrideVertexColors | FColorVertexBuffer | the bool is 1 |

FColorVertexBuffer: strip (2 × u8), int32 Stride (4), int32 NumVertices, then - when
NumVertices > 0 - a bulk array: int32 element size (4), int32 count, count × FColor (B, G, R, A).
The colours are kept packed (one hex string) in the JSON.

Most components have a count of 0. Example with painted LODs:
`Levels/mi/mi_03/umap/mi_03_gimmick_FireMission_01.umap`, `TresGimmickMesh0` (2 LODs of 12,284 colours).

## InstancedStaticMeshComponent (also TresInstancedStaticMeshComponent)

StaticMeshComponent data, then `PerInstanceSMData` as a bulk array: int32 element size (80),
int32 count, per instance: FMatrix Transform (16 × f32), FVector2D LightmapUVBias, FVector2D
ShadowmapUVBias.

## HierarchicalInstancedStaticMeshComponent, FoliageInstancedStaticMeshComponent

InstancedStaticMeshComponent data, then the cluster tree as a bulk array: int32 element size
(40), int32 count, per FClusterNode: FVector BoundMin, int32 FirstChild, FVector BoundMax,
int32 LastChild, int32 FirstInstance, int32 LastInstance.

## InstancedFoliageActor

int32 count + { package index FoliageType, package index Component } - the cooked form of
`TMap<UFoliageType*, FFoliageMeshInfo>` (the mesh info keeps only its component).
