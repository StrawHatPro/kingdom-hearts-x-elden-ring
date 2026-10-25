# Physics and navigation collision

Code: `tools/lib/pkg_classes/physics.js`. Evidence: the classes below in
`audit/package_manifest.json`. Common export framing and FFormatContainer / FByteBulkData are
in `package_json.md`.

## BodySetup, SkeletalBodySetup

| Field | Type | Notes |
|---|---|---|
| BodySetupGuid | 16 bytes | |
| bCooked | uint32 bool | 1 |
| bHasCookedCollisionData | uint32 bool | |
| CookedFormatData | FFormatContainer | present whenever bCooked, also with bHasCookedCollisionData = 0 (count 0) |

The only format name is `PhysXPC`. Its inline payload (the stream `FPhysXFormatDataReader` reads):

| Field | Type |
|---|---|
| bLittleEndian | u8 (1) |
| NumConvexElementsCooked | int32 |
| NumMirroredElementsCooked | int32 |
| NumTriMeshesCooked | int32 |
| mesh streams | per convex / mirrored convex: u8 "is cooked" + a PhysX convex mesh stream; per triangle mesh: a PhysX triangle mesh stream |

The PhysX streams are **third-party data** (PhysX 3.4 cooked meshes), identified by their
signatures `NXS\x01` + `CVXM` (convex; contains `ICE\x01CLHL`, `ICE\x01CVHL`) and `NXS\x01` +
`MESH` (triangle mesh). They carry no length field, so the per-mesh "is cooked" bytes between
them cannot be separated without parsing PhysX's own format; everything after the three
counts is kept as one payload (`meshStreams`) and counted under third-party bytes.

## PhysicsAsset

`CollisionDisableTable`: int32 count + { int32 body index, int32 body index, uint32 bool }.

## PhysicsSerializer

uint32 bCooked, FFormatContainer. Every export is `01 00 00 00 00 00 00 00`: cooked, no
formats (`PhysicsSerializer formats` in `audit/package_facts.json`).

## NavCollision

| Field | Type | Notes |
|---|---|---|
| magic | uint32 | 0xA237F237 |
| Version | int32 | 3 |
| BodySetupGuid | 16 bytes | |
| bCooked | uint32 bool | |
| CookedFormatData | FFormatContainer | see condition below |
| AreaClass | package index | when Version > 1 |

Evidence for the conditions is the game's own serializer, `func_143EABEB0`
(`build/decomp/143EA0000.jsonl`): it reads the magic and version, four uint32 of guid, bCooked,
then calls the format-container serializer only if `(byte at +0xB8 & 2) != 0` or the counts at
+0x98 and +0xA8 are both 0, and finally serializes the object at +0xB0 when the version is
above 1. `src/include/generated/reflection/NavCollision.h` places `CylinderCollision` at 0x90,
`BoxCollision` at 0xA0 (so +0x98/+0xA8 are their counts) and `AreaClass` at 0xB0; the bit is
one of the two reflected booleans (`bGatherConvexGeometry`, `bIsDynamicObstacle`), by the stock
source the former. No export serializes that boolean as a property; the one export with
non-empty collision arrays (`Maps/bt/uasset/Meshes/SM_bt_Arc_SideWallBc.uasset`) has no
container, so the decoder takes the flag as false unless a property sets it.

Format `NavCollision_X` payload (engine data, fully decoded):

| Field | Type |
|---|---|
| bLittleEndian | u8 (1) |
| TriMeshCollision.VertexBuffer | int32 count + FVector (12 bytes) |
| TriMeshCollision.IndexBuffer | int32 count + int32 |
| ConvexCollision.VertexBuffer | int32 count + FVector |
| ConvexCollision.IndexBuffer | int32 count + int32 |
| ConvexShapeIndices | int32 count + int32 |

The four buffers are kept packed (hex) in the JSON.
