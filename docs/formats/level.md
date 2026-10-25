# Maps: World, Level, Model, ModelComponent

Code: `tools/lib/pkg_classes/level.js`. Evidence: the classes below in
`audit/package_manifest.json`.

## World

package index PersistentLevel; int32 count + package index (ExtraReferencedObjects);
int32 count + package index (StreamingLevels).

## Level

| Field | Type |
|---|---|
| Actors | int32 count + package index (0 entries occur) |
| URL | FString Protocol, Host, Map, Portal; int32 count + FString Op; int32 Port; int32 Valid |
| Model | package index |
| ModelComponents | int32 count + package index |
| LevelScriptActor | package index |
| NavListStart, NavListEnd | package index |
| PrecomputedVisibilityHandler | see below |
| PrecomputedVolumeDistanceField | see below |

FPrecomputedVisibilityHandler: FVector2D bucket origin XY, f32 cell size XY, f32 cell size Z,
int32 bucket size XY, int32 bucket count, int32 count + bucket. Bucket: int32 CellDataSize,
int32 count + { FVector Min, uint16 ChunkIndex, uint16 DataOffset }, int32 count +
{ uint32 bCompressed, int32 UncompressedSize, int32 count + bytes }.

FPrecomputedVolumeDistanceField: f32 VolumeMaxDistance, FBox, int32 SizeX, SizeY, SizeZ,
int32 count + FColor (kept packed).

A level without precomputed data ends in 73 zero bytes (28 + 45), e.g.
`Levels/ts/ts_02/umap/ts_02_SpecialSpawn.umap`.

## Model (BSP)

| Field | Type |
|---|---|
| strip | 2 × u8 (FStripDataFlags) |
| Bounds | FBoxSphereBounds: FVector origin, FVector extent, f32 radius |
| Vectors, Points | bulk array (element size 12) of FVector |
| Nodes | bulk array (element size 64) of FBspNode |
| Surfs | int32 count + FBspSurf |
| Verts | bulk array (element size 24) of FVert |
| NumSharedSides | int32 |
| RootOutside, Linked | uint32 bool each |
| NumUniqueVertices | int32 |
| VertexBuffer | bulk array (element size 36) of FModelVertex; present unless strip.global bit 1 and strip.class bit 1 are both set |
| LightingGuid | 16 bytes |
| LightmassSettings | int32 count + FLightmassPrimitiveSettings |

A bulk array is int32 element size, int32 count, elements as in memory. Stock 4.17 also lists
leaf hulls and leaves between NumSharedSides and RootOutside; they are not in these files (an
empty model is exactly 146 bytes, which leaves no room for two more array headers).

| Struct | Layout |
|---|---|
| FBspNode (64) | FPlane (4 × f32), int32 iVertPool, iSurf, iVertexIndex, uint16 ComponentIndex, ComponentNodeIndex, int32 ComponentElementIndex, iBack, iFront, iPlane, iCollisionBound, u8 iZone[2], u8 NumVertices, u8 NodeFlags, int32 iLeaf[2] |
| FBspSurf (56) | package index Material, uint32 PolyFlags, int32 pBase, vNormal, vTextureU, vTextureV, iBrushPoly, package index Actor, FPlane, f32 LightMapScale, int32 iLightmassIndex |
| FVert (24) | int32 pVertex, iSide, FVector2D ShadowTexCoord, BackfaceShadowTexCoord |
| FModelVertex (36) | FVector Position, uint32 TangentX, uint32 TangentZ (packed normals), FVector2D TexCoord, ShadowTexCoord |
| FLightmassPrimitiveSettings (36) | uint32 bUseTwoSidedLighting, bShadowIndirectOnly, f32 FullyOccludedSamplesFraction, uint32 bUseEmissiveForStaticLighting, bUseVertexNormalForHemisphereGather, f32 EmissiveLightFalloffExponent, EmissiveLightExplicitInfluenceRadius, EmissiveBoost, DiffuseBoost |

Member names inside the fixed-size structs are those of the stock engine structs of the same
size; the sizes are confirmed by the bulk-array element sizes in the files.

## ModelComponent

package index Model; int32 count + FModelElement { guid MapBuildDataId, package index
Component, package index Material, int32 count + uint16 Nodes }; int32 ComponentIndex;
int32 count + uint16 Nodes. There is one export of this class
(`Maps/ca/umap/ca_01/ca_01_Area_Fortres_Set.umap`).
