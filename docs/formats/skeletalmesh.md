# `USkeletalMesh` exports

Status: **converted and reversible, all 1,607 exports.** `tools/mesh_convert.js` turns every
export into a glTF 2.0 binary (`.glb`, with skeleton and skin) plus a sidecar (`.mesh.json`),
rebuilds the export from those two files and the package name table, and compares the result
with the original bytes.

| | Exports | Export bytes |
|---|---|---|
| round trip identical (SHA-256) | 1,607 | 4,350,956,856 |
| failed | 0 | 0 |

Evidence: `audit/mesh_manifest.json` and `mesh_summary.json` (`classes.SkeletalMesh.stats`
holds every statistic quoted here), `audit/mesh_export.json` (the proof made from the files on
disk), `audit/mesh_validation_all.json` (independent validator), `audit/mesh_mutation.json`
(what the rebuild depends on). The confidence terms are those of `staticmesh.md`.

The meshes hold 3,187 LODs, 14,474 sections, 129,758 bones, 47,795,106 vertices and
166,935,321 indices. All data is in the `.uexp`. Output:
`assets_converted/meshes/<package path>.glb` and `.mesh.json`.

The six exports the earlier survey could not walk (`Character/npc/n_ca201`, `n_ca202`,
`n_ca206`, `n_ca209`, `n_ex016`, `Character/obj/o_ra215`) are the ones that use the two
game-specific arrays described under "Unidentified data"; they convert and round-trip like
the rest. The earlier guess that cloth data was the cause was wrong: no section in the game
has cloth mapping data.

## Layout

"bulk array" = i32 element size, i32 count, data. "array" = i32 count, elements. bool = i32
that is 0 or 1.

```
tagged properties ... None        Skeleton, LODInfo, PhysicsAsset, bHasVertexColors, ...
i32    object GUID flag           0
u8[2]  strip flags                01 00
f32[7] ImportedBounds             origin, box extent, sphere radius
array  Materials                  36 bytes each (same entry as a static material)
-- reference skeleton: three arrays of the same length
array  bone info                  12 bytes: FName name, i32 parent index
array  bone pose                  40 bytes: quaternion (x, y, z, w), translation (3 f32), scale (3 f32)
array  name -> index map          12 bytes: FName, i32
i32    LOD count                  1..5
LOD x count
i32    0                          on all 1,607; not identified (unk_afterLods)
[i32 count, count x array of u16] 7 exports; not identified (unk_trailingArrays)
```

LOD counts: 1 (1,039 meshes), 2 (55), 3 (16), 4 (495), 5 (2).

### LOD

```
u8[2]  strip flags                01 00
array  sections
u8     index size (2 or 4)
bulk   indices                    element size = index size
array  u16 active bone indices
u32    0                          ("Size" in the UE source; 0 on all 3,187 LODs)
u32    vertex count
array  u16 required bones
array  i32 mesh-to-import vertex map    one entry per vertex on every LOD; opaque
i32    max import vertex
u32    NumTexCoords
-- vertex buffer
u8[2]  strip flags
u32    NumTexCoords (same), bool bUseFullPrecisionUVs
f32[3] MeshExtension, f32[3] MeshOrigin      (1, 1, 1) and (0, 0, 0) on all 3,187 LODs
bulk   vertices                   one per vertex
-- skin weights
u8[2]  strip flags
bool   bExtraBoneInfluences, u32 vertex count
bulk   weights                    8 bytes per vertex (4 bone indices, 4 weights), or 16 (8 + 8)
-- color buffer: only when the tagged property bHasVertexColors is true
u8[2]  strip flags, i32 stride, i32 count, bulk colors (4 bytes each)
-- adjacency
u8     index size
bulk   adjacency indices          4 x the index count on every LOD; opaque
-- only when a section of this LOD has a non-empty unk_tailArray (6 LODs):
u8[2]  strip flags, bulk          24 bytes per element, one element per LOD vertex; not identified
```

Vertex element: TangentX, TangentZ (4 x u8 each), position (3 x f32), then `NumTexCoords` UV
pairs (half floats, or f32 with full-precision UVs).

| Item | Values |
|---|---|
| index size | 16-bit on 3,085 LODs, 32-bit on 102 |
| vertex format | 2 UV sets (28 bytes): 1,151 LODs · 4 sets (36): 784 · 3 sets (32): 644 · 1 set (24): 583 · 3 sets, full-precision UVs (44): 15 · 2 sets, full-precision UVs (36): 10 |
| bone influences per vertex | 4 on 2,864 LODs, 8 on 323 |
| color buffer | present on 513 LODs (one colour per vertex), absent on 2,674 |
| sections per LOD | 1 to 57 |

### Section

```
u8[2]  strip flags                01 00
u16    MaterialIndex
u32    BaseIndex
u32    NumTriangles
u8     TriangleSorting            0 on all
bool   bDisabled                  0 on all
i16    CorrespondClothSectionIndex   -1 on all
u8     bEnableClothLOD            205 (0xCD) on 13,482 sections, 221 on 582, 43 on 405, other values on 5:
                                  looks like an uninitialised byte written by the cooker
bool   bRecomputeTangent          0 on all
bool   bCastShadow                1 on 14,187 sections, 0 on 287
u32    BaseVertexIndex
array  u16 bone map
i32    NumVertices
i32    MaxBoneInfluences
array  cloth mapping data         empty on all 14,474 sections (element size 64 by the UE source: unverified)
array  physical mesh vertices     empty on all
array  physical mesh normals      empty on all
i16    CorrespondClothAssetIndex  -12851 (0xCDCD) on 13,828 sections, -1 on 550, -8739 (0xDDDD) on 96
u8[16] clothing asset GUID        zero on all
i32    clothing asset LOD index   -1 on all
i32    unk_tailFlag               0 on 14,464 sections, 1 on 10
array  unk_tailArray              24 bytes per element; empty on 14,467 sections, one element per
                                  section vertex on 7 (all 7 have the flag set)
```

## Field status

| Field | Status | Evidence |
|---|---|---|
| bone info: name, parent | confirmed (decode) | on all 1,607 meshes bone 0 is the only root and every parent index is smaller than the bone's own index |
| name → index map | confirmed (decode) | on all 1,607 meshes every entry names the bone found at its index |
| bone pose | confirmed (decode) for the rotation | the first four floats have squared norm within 1e-6 of 1 on every one of the 129,758 bones (a unit quaternion). Translation and scale: inferred. That the pose is parent-relative: inferred (it is exported that way) |
| positions | confirmed (decode) | the bounding box of the LOD 0 positions lies inside `ImportedBounds` origin ± extent on all 1,607 meshes (it is never equal to it, so the bounds are padded or come from elsewhere) |
| packed tangent basis | confirmed (decode) | decoded as `byte / 127.5 - 1`, all TangentZ vectors of a LOD have length 0.99–1.02 (to two decimals) on 3,124 of 3,187 LODs; the other 63 contain at least one much shorter vector (about 0.01). None is shorter than 0.0001. TangentZ.w is 0 (13,380,752 vertices) or 255 (34,414,354), nothing else |
| TangentX is the tangent | inferred | perpendicular to TangentZ (|cos| < 0.05 on every vertex) on 1,572 LODs, not on 1,544 (|cos| ≥ 0.5 on at least one vertex) |
| indices | confirmed (decode) | below the vertex count on every mesh (LOD 0 checked); every section's indices stay inside the section's own vertex range (14,474 sections) |
| triangle winding | confirmed (decode) | on LOD 0, `(p1-p0) x (p2-p0)` points against the normal on 33,604,484 triangles and along it on 153,030 (286 degenerate); against on the majority of triangles in all 1,607 meshes |
| section: BaseIndex, NumTriangles | confirmed (decode) | sections tile the index buffer on all 3,187 LODs; every BaseIndex is a multiple of 3 |
| section: BaseVertexIndex, NumVertices | confirmed (decode) | sections tile the vertex buffer on all 3,187 LODs |
| section: bone map | confirmed (decode) | every entry is a bone of the reference skeleton and no bone appears twice, on all 14,474 sections |
| skin weights: 4 or 8 indices then 4 or 8 weights | confirmed (decode) | the weights of every one of the 47,795,106 vertices sum to 255; every index with a non-zero weight is inside its section's bone map; every slot with weight 0 has index 0 |
| section: MaxBoneInfluences | confirmed (decode) | equals the largest number of non-zero weights on a vertex of the section, on all 14,474 sections |
| section: MaterialIndex | confirmed (decode) | a valid slot on all 14,474 sections |
| UVs, vertex colours | confirmed (round trip); meaning inferred | as for static meshes |
| active bone indices, required bones | inferred | carried in the sidecar as number arrays |
| ImportedBounds sphere radius, material entries, bCastShadow, cloth fields | inferred | carried in the sidecar |
| mesh-to-import vertex map | opaque | 191,180,424 bytes, raw bufferViews |
| adjacency index buffer | opaque | 1,705,598,352 bytes, raw bufferViews; always 4 x the index count |

## Unidentified data

All of it is carried, none of it is interpreted:

- **`unk_tailFlag` / `unk_tailArray` at the end of every section** (8 bytes when empty: the
  "8 unidentified bytes" of the earlier survey). The array holds 24 bytes per element, which
  read as six floats; where it is non-empty it has exactly one element per section vertex.
  7 sections in 6 LODs have it; 3 more sections have the flag set with an empty array.
  1,333,728 bytes in total.
- **`unk_buffer24` at the end of a LOD**: strip flags and a bulk array of 24-byte elements, one
  per LOD vertex, present on exactly the 6 LODs that have a section with a non-empty
  `unk_tailArray`. 7,268,712 bytes in total.
- **`unk_afterLods`**: an i32 that is 0 on all meshes (the stock layout has an array count in
  this neighbourhood; not checked further).
- **`unk_trailingArrays`**: on 7 exports (for example
  `Character/enemy/e_ex711/mdl/e_ex711_0_atr02`), one array of u16 values each, 24,630 values
  in total.

None of these has a counterpart in the stock 4.17 layout as this project understands it.

## The converted files

`<name>.glb`:

- node 0 `UE_to_glTF` carries the matrix (x, y, z) → (x, z, y) / 100. Bones are nodes 1..N in
  bone order, parented as in the bone info array, with the root bone under node 0. Each bone
  node has the reference pose as `rotation`, `translation`, `scale`, unchanged.
- one skin with every bone as a joint and inverse bind matrices computed from the reference
  pose (derived data; no bone has a singular matrix).
- one mesh per LOD, one primitive per section, as for static meshes. The LOD 0 mesh node is in
  the scene at root level with the skin; other LODs are nodes outside the scene.
- attributes: `POSITION`, `NORMAL`, `TANGENT`, `TEXCOORD_0..n`, `COLOR_0`, `JOINTS_0`,
  `WEIGHTS_0` (and `JOINTS_1`, `WEIGHTS_1` with 8 influences), `_UE_TANGENT_X`, `_UE_TANGENT_Z`.
- `JOINTS_n` holds skeleton bone indices (unsigned short): the export's per-section local
  index looked up in the section's bone map. A slot with weight 0 is written as joint 0.
  `WEIGHTS_n` holds the original weight bytes, normalised.
- `NORMAL`, `TANGENT` and the inverse bind matrices are derived for viewers and are not used by
  the reverse direction; the packed tangent basis is rebuilt from the two `_UE_TANGENT_*`
  accessors (the original integers).
- index order: every triangle is stored as (a, c, b), as for static meshes (all 3,187 LODs).
- the mesh-to-import maps, adjacency buffers and the unidentified 24-byte arrays are raw
  bufferViews named `ue:lod<N>....`.

`<name>.mesh.json`: `data` holds every other field (properties, bounds, materials, bone names
and parents, the name map, sections with their bone maps, active and required bone lists, the
unidentified values); `gltf.blobs` maps each byte range of the export to its source in the
`.glb`.

How the reverse direction recovers the parts that are not stored verbatim:

- **vertex buffer**: interleaved again from `_UE_TANGENT_X`, `_UE_TANGENT_Z`, `POSITION` and
  the `TEXCOORD_n` accessors (float32 narrowed back to half, exactly).
- **skin weights**: `JOINTS_n` mapped back through the inverse of each section's bone map
  (possible because no bone map repeats a bone), local index 0 where the weight is 0. This
  reproduced the original weight buffer on all 3,187 LODs; the raw-bufferView fallback that the
  converter has for a non-invertible case was never needed.
- **reference pose**: from the bone nodes' JSON numbers, which reproduce the float32 bits,
  except for bones that contain a negative zero, which JSON written by `JSON.stringify` cannot
  express: those 36,115 bones (of 129,758) have their 40 original bytes in the sidecar
  (`gltf.blobs.refPose.overrides`). No bone needed an override for any other reason (no
  non-finite value, no non-unit quaternion).

What the rebuild reads, shown by changing one value at a time in 25 exported meshes
(`audit/mesh_mutation.json`): a change in `POSITION`, `_UE_TANGENT_X`, `_UE_TANGENT_Z`,
`TEXCOORD_n`, `COLOR_0`, `JOINTS_n`, `WEIGHTS_n`, the index bufferView, a bone node's
translation, or a raw bufferView changes the rebuilt export (or the rebuild refuses the value)
in every case tried; a change in `NORMAL` or `TANGENT` leaves it unchanged in all 25.

Independent reader: the Khronos glTF-Validator (`gltf-validator` 2.0.0-dev.3.10) was run on
all 1,607 files: 0 errors and 0 warnings on every file; only the informational `UNUSED_OBJECT`
and `UNUSED_MESH_TANGENT` messages. Its checks include index ranges, unit normals and
tangents, joint indices within the skin, weights summing to 1 and the form of the inverse bind
matrices.

Size: 6,412,428,328 bytes of `.glb` and 52,199,723 bytes of sidecars for the 1,607 meshes.

## Opaque / not done

- The unidentified data listed above, the mesh-to-import map and the adjacency buffers.
- Cloth: no section has cloth mapping data, so that part of the layout (and the cloth vertex
  buffer that would follow in the LOD) is written from the UE source and has never been
  exercised.
- Related exports not converted: `Skeleton` (the mesh carries its own reference skeleton, which
  is what is exported), `PhysicsAsset`, morph targets (`MorphTargets` is referenced by some
  meshes), and the game's `SQEX_*AssetUserData` exports in the same packages.
- Materials are exported as named slots only.
- The rebuild needs the package's name table (the `.uasset`).
