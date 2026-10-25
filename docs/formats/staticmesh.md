# `UStaticMesh` exports

Status: **converted and reversible, all 16,632 exports.** `tools/mesh_convert.js` turns every
export into a glTF 2.0 binary (`.glb`) plus a sidecar (`.mesh.json`), rebuilds the export from
those two files and the package name table, and compares the result with the original bytes.

| | Exports | Export bytes |
|---|---|---|
| round trip identical (SHA-256) | 16,632 | 4,162,750,453 |
| failed | 0 | 0 |

Evidence (all written by tools, logged in `audit/tool_log.jsonl`):

| File | Produced by | Holds |
|---|---|---|
| `audit/mesh_manifest.json`, `mesh_summary.json` | `mesh_convert.js verify` | one row per mesh (hashes, LODs, vertices, triangles, formats, `roundtrip_matches`), and every statistic quoted below (`classes.StaticMesh.stats`) |
| `audit/mesh_export.json`, `mesh_export_summary.json` | `mesh_convert.js export` | the same proof made from the files read back from disk |
| `audit/mesh_validation_all.json` | `mesh_validate.js --all` | Khronos glTF-Validator result for every exported file |
| `audit/mesh_mutation.json` | `mesh_mutation.js` | which glTF sources the rebuild depends on |

The meshes hold 27,440 LODs, 92,315,264 vertices and 273,594,921 indices in the main index
buffers. All data is in the `.uexp`; no `.ubulk`. Output: `assets_converted/meshes/<package
path>.glb` and `.mesh.json`.

## Confidence terms

- **confirmed (round trip)**: the field is read and written by the same layout description
  (`tools/lib/mesh.js`, one function used in both directions), and the rebuilt export is
  byte-identical on all 16,632 exports. This proves size, position and order of the field, and
  that nothing is lost; it does not by itself prove what the field *means*.
- **confirmed (decode)**: in addition, a check on the decoded values passed (listed per field).
- **inferred**: the name or meaning is taken from the public UE 4.17 source and no check on the
  values was made.
- **opaque**: carried as bytes, not interpreted.

## Layout

"bulk array" = i32 element size, i32 count, data. "array" = i32 count, elements. bool = i32
that is 0 or 1 (the reader rejects other values).

```
tagged properties ... None        StaticMaterials, ExtendedBounds, LightMapResolution, ...
i32    object GUID flag           0
u8[2]  strip flags                01 00
bool   bCooked                    1
i32    BodySetup                  package index: an export of the same package on all 16,632
i32    NavCollision               package index: an export of the same package on 16,630, null on 2
u8[16] LightingGuid
array  Sockets                    i32 package index each (non-empty on 10 meshes)
i32    LOD count                  1..6
LOD x count
u8[2]  strip flags                01 00
per LOD: bool distance field valid, [distance field]
f32[7] Bounds                     origin, box extent, sphere radius
bool   bLODsShareStaticLighting   1 on 16,412, 0 on 220
f32[8] ScreenSize
bool   bHasSpeedTreeWind          1 on 14
[u8[1460] SpeedTree wind block]   only when the flag is 1; opaque
array  StaticMaterials            36 bytes each
```

LOD counts: 1 (11,934 meshes), 2 (936), 3 (1,684), 4 (1,813), 5 (260), 6 (5).

### LOD

```
u8[2]  strip flags                01 00
array  sections                   28 bytes each
f32    MaxDeviation
-- position buffer
i32    stride (12), i32 vertex count
bulk   positions                  element size 12 (3 x f32)
-- vertex buffer
u8[2]  strip flags
i32    NumTexCoords, i32 stride, i32 vertex count
bool   bUseFullPrecisionUVs, bool bUseHighPrecisionTangentBasis
bulk   vertices                   element size = stride
-- color buffer
u8[2]  strip flags
i32    stride, i32 vertex count
bulk   colors (4 bytes each)      only when the count is > 0
-- five index buffers, each:
bool   b32Bit
bulk   bytes                      element size 1; count is the byte length
       order: index, reversed index, depth-only, reversed depth-only, adjacency
-- area-weighted samplers: one per section, then one for the LOD, each:
array  f32 Prob, array i32 Alias, f32 TotalWeight
```

Vertex element: TangentX, TangentZ (4 x u8 each, or 4 x u16 each with high-precision tangents),
then `NumTexCoords` UV pairs (half floats, or f32 with full-precision UVs).

| Vertex format | LODs |
|---|---|
| 2 UV sets, 16 bytes | 19,474 |
| 1 set, 12 | 5,091 |
| 3 sets, 20 | 1,851 |
| 4 sets, 24 | 857 |
| 8 sets, 40 | 58 |
| 5 sets, 28 | 56 |
| 7 sets, 36 | 22 |
| 2 sets, high-precision tangents, 24 | 17 |
| 2 sets, full-precision UVs, 24 | 10 |
| 6 sets, 32 | 2 |
| 3 sets, full-precision UVs, 32 | 1 |
| 1 set, full-precision UVs, 16 | 1 |

Index buffer: 16-bit on 27,305 LODs, 32-bit on 135. Colour buffer: present on 12,294 LODs (one
colour per vertex on each), absent on 15,146.

## Field status

| Field | Status | Evidence |
|---|---|---|
| positions (3 x f32) | confirmed (decode) | bounding box of the LOD 0 positions equals `Bounds` origin ± extent on all 16,632 meshes |
| `Bounds` (origin, extent) | confirmed (decode) | same check. The sphere radius is not checked: inferred |
| packed tangent basis (4 x u8, or 4 x u16 on 17 LODs) | confirmed (decode) | decoded as `byte / 127.5 - 1` (`u16 / 32767.5 - 1` for the wide form), the TangentZ vectors of a LOD all have length 0.99–1.01 (to two decimals) on 27,348 of 27,440 LODs; the other 92 LODs contain at least one much shorter vector (shortest about 0.01). None is shorter than 0.0001. The statistic is not split by format. TangentZ is the normal: see winding |
| TangentZ.w | confirmed (decode) as a two-valued sign | over all 92,315,264 vertices only the values 0 (9,663,593), 255 (82,520,742) and, in the wide format, 65,535 (130,929) occur. That it is the bitangent sign is inferred |
| TangentX is the tangent | inferred | TangentX is perpendicular to TangentZ (|cos| < 0.05 on every vertex) on 18,252 LODs, but not on 7,605 LODs (|cos| ≥ 0.5 on at least one vertex) |
| indices | confirmed (decode) | every index is below the vertex count on all 27,440 LODs |
| triangle winding | confirmed (decode) | on LOD 0, `(p1-p0) x (p2-p0)` points against the normal on 68,703,249 triangles and along it on 344,631 (587 degenerate); by majority per mesh: 16,466 against, 140 along, 26 tie. Front faces are therefore clockwise seen from the normal side, in the export's left-handed space |
| section: FirstIndex, NumTriangles | confirmed (decode) | the sections tile the index buffer on 27,439 of 27,440 LODs; every FirstIndex is a multiple of 3 (52,032 sections) |
| section: MinVertexIndex, MaxVertexIndex | confirmed (decode) | equal the smallest and largest index used by the section on 52,031 of 52,032 sections (the other one is empty) |
| section: MaterialIndex | confirmed (decode) | a valid slot of the material array on 51,991 sections; out of range on 40 |
| section: bEnableCollision, bCastShadow | inferred | only that they are 0 or 1 |
| UVs (half or f32 pairs) | confirmed (round trip); meaning inferred | widening to float32 and narrowing back is exact on every value. Which set is the lightmap set, and the V direction, are not checked |
| vertex colours | confirmed (round trip); channel order inferred | stored B, G, R, A according to the UE source; nothing in the data tells the channels apart |
| depth-only index buffer | opaque | as many indices as the main buffer and the same index size on all 27,440 LODs (655,823,904 bytes); the values were not compared with the main buffer |
| reversed, reversed depth-only, adjacency buffers | confirmed empty | on all 27,440 LODs |
| area-weighted samplers | confirmed empty | on all 27,440 LODs |
| static material entry (36 bytes) | confirmed (round trip); names inferred | MaterialInterface (package index), MaterialSlotName (FName), bool bInitialized, bool bOverrideDensities, f32[4] LocalUVDensities |
| BodySetup, NavCollision | confirmed (round trip) as package indices | the referenced exports themselves are not examined |
| LightingGuid, MaxDeviation, ScreenSize, bLODsShareStaticLighting, sockets | inferred | carried in the sidecar |
| distance field | inferred | valid flag set on 6 LODs; 2 of them hold a volume (2,048 bytes in total), 4 are empty. After the volume: size (3 x i32), box (6 x f32 + u8), 2 x f32 taken to be `DistanceMinMax`, three bools |
| SpeedTree wind block | opaque | 1,460 bytes on each of the 14 meshes that have it. Its end is located from the end of the export (the material array is last, and its length is also in the `StaticMaterials` property) |

## The converted files

`<name>.glb` (glTF 2.0 binary, one buffer):

- node 0 `UE_to_glTF` carries the matrix (x, y, z) → (x, z, y) / 100: Unreal's left-handed,
  Z-up, centimetre space to glTF's right-handed, Y-up, metre space. Vertex data is **not**
  transformed; positions are the original float32 values.
- one mesh per LOD (`LOD0`, `LOD1`, …), one primitive per section, all primitives of a LOD
  sharing the LOD's vertex accessors. Only LOD 0 is in the scene; the other LODs are nodes
  outside the scene under their own copy of the root (`scenes[0].extras.ue_lod_nodes` lists them).
- attributes: `POSITION`, `NORMAL`, `TANGENT`, `TEXCOORD_0..n`, `COLOR_0` (when present),
  and `_UE_TANGENT_X` / `_UE_TANGENT_Z` (unsigned byte or unsigned short VEC4): the packed
  tangent basis exactly as stored.
- `NORMAL` and `TANGENT` are float32, decoded from the packed values and normalised, with
  `TANGENT.w` = +1 or −1 from TangentZ.w. **They are derived data for viewers and are not used
  by the reverse direction**: normalising does not re-encode bit-exactly, so the packed integers
  are kept in the two extra accessors and the rebuild reads those.
- indices: one bufferView for the LOD's whole index buffer; each section's accessor is a window
  into it. Because the root matrix is a reflection, every triangle is stored as (a, c, b); the
  rebuild swaps it back (`windingSwapped` in the sidecar; true on all 27,440 LODs).
- materials: one per slot, named after the slot, with the material's object path in `extras`.
  No textures or shading parameters are exported.
- the depth-only index buffers and distance-field volumes are raw bufferViews named
  `ue:lod<N>.depthOnlyIndices` etc.

`<name>.mesh.json` (sidecar, one line of JSON): `data` holds every other field of the export
under the names used above (tagged properties in the lossless form of `tools/lib/uprops.js`,
strip flags, references, bounds, sections, materials, the wind block as hex);
`gltf.blobs` says, for each byte range of the export, which accessor or bufferView of the
`.glb` it is rebuilt from and how.

What the rebuild reads, shown by changing one value at a time in 25 exported meshes
(`audit/mesh_mutation.json`): a change in `POSITION`, `_UE_TANGENT_X`, `_UE_TANGENT_Z`,
`TEXCOORD_n`, `COLOR_0`, the index bufferView or the depth-only bufferView changes the rebuilt
export (or the rebuild refuses the value) in every case tried; a change in `NORMAL` or
`TANGENT` leaves it unchanged in all 25.

Independent reader: the Khronos glTF-Validator (`gltf-validator` 2.0.0-dev.3.10, npm) was run
on all 16,632 files: 0 errors and 0 warnings on every file. It reports only informational
messages (`UNUSED_OBJECT` for the nodes and bufferViews outside the scene, `UNUSED_MESH_TANGENT`
because the materials have no normal texture). The validator checks, among other things, that
indices are in range, that normals and tangents are unit length and that the declared
position bounds match the data.

Size: 7,569,358,496 bytes of `.glb` and 120,924,489 bytes of sidecars for the 16,632 meshes.

## Opaque / not done

- SpeedTree wind block (14 meshes, 1,460 bytes each) and the depth-only index buffers are
  carried, not interpreted.
- The `BodySetup` (collision) and `NavCollision` exports that accompany each mesh are separate
  exports and are not converted; the mesh keeps only its references to them.
- Materials are exported as named slots only.
- Meaning of UV sets, colour channels, tangent handedness, and the fields marked *inferred*
  above.
- The rebuild needs the package's name table (the `.uasset`) to turn names back into indices,
  as the texture converter does.
