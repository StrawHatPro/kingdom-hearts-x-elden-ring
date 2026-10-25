# Navigation meshes: RecastNavMesh, RecastNavMeshDataChunk

Code: `tools/lib/pkg_classes/nav.js`. Evidence: both classes in
`audit/package_manifest.json` (1,160 and 7 exports).

`MercunaNavOctree` (the Mercuna 3D-navigation middleware's octree, signature `enkiOT16`) is
described in `mercuna.md`.

## RecastNavMesh

| Field | Type | Notes |
|---|---|---|
| NavMeshVersion | uint32 | 13 |
| RecastNavMeshSizeBytes | uint32 | bytes from this field to the end of the export |
| NumTiles | int32 | |
| Params | FVector orig, f32 tileWidth, f32 tileHeight, int32 maxTiles, int32 maxPolys | dtNavMeshParams |
| tiles | NumTiles × tile entry | |

Tile entry: uint64 TileRef, int32 TileDataSize (the size of the tile blob in memory, not of
what follows), then the tile and its compressed tile-cache layer.

## Tile

Ten int32 totals: totVertCount, totPolyCount, maxLinkCount, detailMeshCount, detailVertCount,
detailTriCount, bvNodeCount, offMeshConCount, offMeshSegConCount, clusterCount. Then the
Detour tile header, written field by field:

| Field | Type |
|---|---|
| magic | uint32 `DNAV` (bytes `56 41 4E 44`) |
| version | int32 (7) |
| x, y, layer | int32 |
| userId | uint32 |
| polyCount, vertCount, maxLinkCount, detailMeshCount, detailVertCount, detailTriCount, bvNodeCount, offMeshConCount, offMeshBase | int32 |
| walkableHeight, walkableRadius, walkableClimb | f32 |
| bmin, bmax | 3 × f32 each |
| bvQuantFactor | f32 |
| clusterCount, offMeshSegConCount, offMeshSegPolyBase, offMeshSegVertBase | int32 |

Then the arrays, each sized by a total:

| Array | Count | Element |
|---|---|---|
| verts | totVertCount | 3 × f32 |
| polys | totPolyCount | dtPoly, 32 bytes: uint32 firstLink, uint16 verts[6], uint16 neis[6], uint16 flags, u8 vertCount, u8 areaAndType |
| detailMeshes | detailMeshCount | 10 bytes: uint32 vertBase, uint32 triBase, u8 vertCount, u8 triCount |
| detailVerts | detailVertCount | 3 × f32 |
| detailTris | detailTriCount | 4 × u8 |
| bvTree | bvNodeCount | 16 bytes: uint16 bmin[3], uint16 bmax[3], int32 i |
| offMeshCons | offMeshConCount | 36 bytes: f32 pos[6], f32 rad, uint16 poly, u8 flags, u8 side, uint32 userId |
| offMeshConHeights | offMeshConCount | f32 |
| clusterCenters | clusterCount | 3 × f32 |
| polyClusters | header.offMeshBase | uint16 (one per ground polygon) |

Links are not stored (only maxLinkCount). `offMeshSegConCount` is 0 in every tile; a tile
with segment connections would be rejected by the decoder rather than guessed at.
The field names are Detour's; the widths and order are fixed by the bytes (for example the
tile at the start of `Levels/ca/ca_01/ca_01.umap` `RecastNavMesh-Group1`: 15 vertices, 7
polygons, 7 detail meshes, 13 detail triangles, 14 BV nodes, 1 cluster, and the off-mesh
layout from `Levels/bt/bt_01/bt_01.umap`, a tile with 4 connections whose four heights are
100, 100, 51, 51).

## Compressed tile-cache layer

int32 size, then that many bytes (0 = none):

| Field | Type |
|---|---|
| magic | uint32 `DTLR` (bytes `52 4C 54 44`) |
| version | int32 |
| tx, ty, tlayer | int32 |
| bmin, bmax | 3 × f32 each |
| hmin, hmax, width, height, minx, maxx, miny, maxy | uint16 |
| uncompressedSize | int32 |
| data | zlib stream (third-party container, kept verbatim) of the layer's grids |

## RecastNavMeshDataChunk

uint32 NavMeshVersion, int64 size (bytes from that field's position to the end), int32 tile
count, per tile: int32 TileDataSize, the tile and the compressed layer as above (no TileRef).
