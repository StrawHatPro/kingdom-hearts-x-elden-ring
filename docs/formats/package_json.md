# Generic package decoder (`.uasset`/`.umap` + `.uexp` ⇄ JSON)

Code: `tools/lib/package.js` (framework, tagged properties), `tools/lib/pkg_classes/*.js`
(native data per class), `tools/lib/kismet.js` (Blueprint bytecode).
Tools: `tools/asset_census.js`, `tools/package_convert.js`, `tools/package_check.js`.
Evidence: `audit/asset_census.json`, `audit/package_convert.json`, `audit/package_manifest.json`.
The header layout itself is in `uasset.md`; this file covers what the generic decoder adds.

## What is proven and how

`decode(uasset, uexp)` produces one JSON document; `encode(doc)` rebuilds both files.
`tools/package_convert.js` writes `assets_converted/packages/<path>.json` for every package
under `assets/` and, before writing, parses the JSON text back and re-encodes it.
`tools/package_check.js` repeats that from the files on disk and writes
`audit/package_manifest.json`. A package counts as round-trip identical only when both the
`.uasset` and the `.uexp` are reproduced byte for byte.

Every byte of an export is in exactly one of four states, counted per class in the manifest:

| State | Meaning | JSON form |
|---|---|---|
| decoded | read by a codec that also writes it back | named fields |
| third-party | payload in a named external format, kept verbatim | hex string in a field whose codec names the format (`third_party` in the manifest) |
| delegated | export of a class another converter of this project decodes (textures, meshes, SEAD audio, AnimSequence; also the skeletal-mesh part of a DestructibleMesh) | `{ "delegated": tool, "size", "sha256" }`; the bytes are not embedded |
| uninterpreted | no layout known | `undecoded` (native data), `raw` (a property value), `parametersRaw` / `extraRaw` (shader bindings of types the executable lacks) |

A codec that does not consume an export exactly is discarded for that export and the native
bytes fall back to `undecoded` with the error text, so a wrong layout cannot pass as decoded.

## Document

```
format            "kh3-package-json/1"
summary           FPackageFileSummary fields (uasset.md)
names             [text, hashNoCase, hashCase] per name-table entry
imports           { classPackage, className, outerIndex, objectName }
headerTables      depends (per export: package indexes), assetRegistryCount (0),
                  preloadDependencies (int32 list)
uexpTrailer       "tag" = the 4-byte package tag C1 83 2A 9E ends the .uexp
exports           one per line: export-table row + class + data
```

The three tables after the export table are decoded here (they were kept as `tail` by
`uasset.js`): the depends map is `ExportCount` × (int32 count + int32 indexes) at
`DependsOffset`; an int32 0 at `AssetRegistryDataOffset`; `PreloadDependencyCount` int32 at
`PreloadDependencyOffset`. `audit/package_manifest.json` `totals.header_uninterpreted_bytes`
is the number of header bytes outside these tables (0).

Names are written as text. `Base_N` means name `Base` with number N+1; where the text would
be ambiguous (a name-table entry whose own text ends in `_N`, or duplicate entries) the pair
`{ index, number }` is written instead, so the mapping back is exact.

### Export data

```
tagged properties ... None
int32 hasGuid (+ 16-byte guid)      absent for class default objects (ObjectFlags & 0x10)
native data of the class            docs per family below
```

In every package of the current tree the exports tile the `.uexp` in
`SerialOffset` order with no gap and the package tag follows (`totals.uexp_gap_bytes`).

A blueprint-class instance (class exported by a package) uses the native serializer of the
class's native ancestor. Inside one package the super chain is followed directly; across
packages `audit/blueprint_class_bases.json` (written by `tools/asset_census.js` from every
class export's `SuperIndex`) supplies the base. The resolved name is stored as `nativeClass`.

### Tagged properties

Tag layout as in `uasset.md`. A property is `{ name, type, value }` plus `arrayIndex`,
`structName`, `structGuid`, `enumName`, `innerType`, `valueType`, `guid` when present/non-zero.
The tag's size is not stored: it is recomputed from the encoded value. A value that does not
decode to exactly the tagged size is kept as `raw`.

| Type | Value |
|---|---|
| Int8/16/32/64, UInt16/32/64, Float, Double | number (64-bit beyond 2^53 as string; a float that JSON cannot hold - NaN, ±Inf, -0 - as `{ f32bits }`) |
| Bool | in the tag |
| Byte | u8, or a name when the size is 8 |
| Name, Enum | name |
| Object, Class, WeakObject, Interface | package index |
| Str, AssetObject, AssetClass | FString (`{ s, enc }` when empty or UTF-16) |
| Text | FText: uint32 flags, int8 history type; type -1: nothing more; type 0: FString namespace, key, source |
| LazyObject | 16-byte guid |
| Delegate | package index + function name; MulticastDelegate: int32 count of those |
| Struct | native layout by struct name (table below), else a nested tagged list |
| Array | int32 count + elements; struct arrays carry one inner tag (`structName`, its size is recomputed); byte arrays are hex |
| Set | int32 removed count + elements, int32 count + elements |
| Map | int32 removed count + keys, int32 count + (key, value) |

Map and set tags do not name struct types and do not say whether a byte key is a name. The
reader tries the candidate layouts (tagged list, then `Guid`, `Vector`, `IntPoint`, `Vector2D`,
`LinearColor`, `StringAssetReference`, `Rotator`, `Color`, `IntVector`; u8 then name) and
keeps the first that consumes the value exactly; the choice is recorded as `keyLayout` /
`valueLayout`.

Native structs (each confirmed by tag sizes and the round trip of every package using it):

| Struct | Layout |
|---|---|
| Vector, Vector2D, Vector4, Plane, Quat | 3 / 2 / 4 / 4 / 4 × f32 |
| Rotator | f32 pitch, yaw, roll |
| LinearColor / Color | 4 × f32 r,g,b,a / 4 × u8 b,g,r,a |
| IntPoint, IntVector | 2 / 3 × int32 |
| Guid | 16 bytes |
| Box / Box2D | min, max (Vector / Vector2D), u8 isValid |
| TwoVectors | 2 × Vector |
| DateTime, Timespan | int64 ticks |
| StringAssetReference, StringClassReference | FString |
| RichCurveKey | 3 × u8 modes, 6 × f32 (uasset.md) |
| ExpressionInput | int32 OutputIndex, FString InputName, int32 Mask, MaskR, MaskG, MaskB, MaskA, FName ExpressionName |
| Color/Scalar/Vector/Vector2MaterialInput | ExpressionInput + uint32 UseConstant + constant (Color / f32 / Vector / Vector2D) |
| MaterialAttributesInput | ExpressionInput |
| NavAgentSelector | uint32 packed bits |
| SmartName | FName DisplayName |
| FontCharacter | int32 StartU, StartV, USize, VSize, u8 TextureIndex, int32 VerticalOffset |
| FontData | uint32 1 (cooked), package index FontFaceAsset; when that is 0: FString file name, u8 hinting, u8 loading policy |

### Bulk data and format containers

`FByteBulkData`: uint32 flags, int32 element count, int32 size on disk, int64 offset, then
the payload when it is inline (flag 0x100 "separate file" and 0x1 "at end of file" both
clear). For an inline payload the offset equals `TotalHeaderSize` + position of the payload
in the `.uexp`; a header that broke this would be marked `offsetRule: false` (none is).
`FFormatContainer`: int32 count + { FName format, FByteBulkData }.

## A header that does not describe its .uexp

No package of the current tree is in this state (`decoded_with_another_header` and
`export_table_mismatch` in `audit/package_manifest.json` are both empty). Until the pak mount
order was corrected, eight packages -
`Cutscene/Localization/{en,ja}/ss/cutscene_ss50_memory{1,3,4,5}.umap` - had a header from a
voice overlay pak beside a `.uexp` from a patch pak (`pak_mount_finding.md`); the mechanism
that handled them stays in the decoder:

When the exports do not tile the `.uexp` or a tagged-property list fails to parse, and the
caller supplies the other headers found at the same path (`opts.altHeaders`; the tools pass
the live and `_superseded` copies), the first header with which the `.uexp` tiles and every
property list parses is used. The document then has this header's tables, `uexpMismatch`
(why), and `uexpDecodedWith = { headerFrom, package }` where `package` is a complete document
made from the other header and the `.uexp`. Both files still rebuild byte for byte.
`audit/package_manifest.json` lists such packages under `decoded_with_another_header`, and
`export_table_mismatch` lists any package for which no such header exists (its `.uexp` would
be carried whole as `uexpRaw` and counted in `totals.uexp_gap_bytes`).

In 3,663 packages (the eight above among them) the header and the `.uexp` come from different paks - a patch pak
(`*_0_P.pak`) that replaces the `.uexp` alone - and decode normally; the replaced `.uexp` of
each is the header-less file under `_superseded/` that the converter pairs with the same header
(`loose_files.md`).

## Family documents

| Family | Classes | Doc |
|---|---|---|
| blueprint | property classes, Function, BlueprintGeneratedClass (+Widget/Anim), ScriptStruct, UserDefinedStruct, Enum, UserDefinedEnum | `blueprint.md`, `kismet.md` |
| components | StaticMeshComponent family, InstancedStaticMeshComponent, HierarchicalInstancedStaticMeshComponent, FoliageInstancedStaticMeshComponent, InstancedFoliageActor | `components.md` |
| physics | BodySetup, SkeletalBodySetup, PhysicsAsset, PhysicsSerializer, NavCollision | `physics.md` |
| landscape | LandscapeComponent, LandscapeHeightfieldCollisionComponent, LandscapeMeshCollisionComponent | `landscape.md` |
| level | World, Level, Model, ModelComponent | `level.md` |
| lighting | MapBuildDataRegistry | `mapbuilddata.md` |
| material | Material, MaterialInstanceConstant, LandscapeMaterialInstanceConstant, global shader cache | `material.md`, `shader_parameters.md`, `shader_parameters_table.md` |
| navigation | RecastNavMesh, RecastNavMeshDataChunk | `navmesh.md` |
| mercuna | MercunaNavOctree | `mercuna.md` |
| destructible | DestructibleMesh | `misc_classes.md` |
| others | Skeleton, Rig, animation assets, MorphTarget, ParticleModuleRequired, VectorFieldStatic, Font, FontFace, SoundWave, SoundCue, SoundNodeWavePlayer, TextureRenderTarget2D, HoudiniAsset, DataTable | `misc_classes.md` |
