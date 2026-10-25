# Cooked package format (`.uasset` / `.umap` + `.uexp` + `.ubulk`)

Reader/writer: `tools/lib/uasset.js` (header), `tools/lib/uprops.js` (tagged properties),
`tools/lib/datatable.js` (UDataTable), `tools/lib/sheet.js` (plain tagged-property objects). Evidence: `audit/uasset_roundtrip.json` — the header of
all 223,956 packages parses and serializes back byte-identically — and
`audit/sheets_roundtrip.json` for tagged properties, DataTables and plain objects.

Packages are cooked for UE 4.17 and **unversioned**: `FileVersionUE4`, the licensee version and
the saved-by engine version are all 0, so the reader assumes the 4.17 layout throughout.
Each package is split: `.uasset` holds the header tables, `.uexp` the export data, `.ubulk`
bulk data (textures, audio).

## `.uasset` header (FPackageFileSummary), little-endian

| Field | Type | Notes |
|---|---|---|
| Tag | u32 | `0x9E2A83C1` |
| LegacyFileVersion | i32 | -7 |
| LegacyUE3Version, FileVersionUE4, FileVersionLicenseeUE4 | i32 ×3 | 0 |
| CustomVersions | i32 count + {GUID, i32} | count 0 |
| TotalHeaderSize | i32 | size of the `.uasset`; export offsets are relative to a file where `.uexp` follows it |
| FolderName | FString | `None` |
| PackageFlags | u32 | |
| NameCount, NameOffset | i32 ×2 | |
| GatherableTextDataCount, Offset | i32 ×2 | |
| ExportCount, ExportOffset, ImportCount, ImportOffset | i32 ×4 | |
| DependsOffset | i32 | |
| StringAssetReferencesCount, Offset | i32 ×2 | |
| SearchableNamesOffset, ThumbnailTableOffset | i32 ×2 | |
| Guid | 16 bytes | |
| Generations | i32 count + {ExportCount, NameCount} | |
| SavedByEngineVersion, CompatibleWithEngineVersion | {u16, u16, u16, u32, FString} ×2 | |
| CompressionFlags | u32 | |
| CompressedChunks | i32 count | 0 |
| PackageSource | u32 | |
| AdditionalPackagesToCook | i32 count + FString | |
| AssetRegistryDataOffset | i32 | |
| BulkDataStartOffset | i64 | |
| WorldTileInfoDataOffset | i32 | |
| ChunkIDs | i32 count + i32 | |
| PreloadDependencyCount, Offset | i32 ×2 | |

**Name table:** `FString` + u16 non-case-preserving hash + u16 case-preserving hash per entry.
**FName reference:** i32 name index + i32 number (0 = none; n means suffix `_<n-1>`).
**Import (28 bytes):** ClassPackage FName, ClassName FName, OuterIndex i32, ObjectName FName.
**Export (104 bytes):** ClassIndex, SuperIndex, TemplateIndex, OuterIndex (i32 each), ObjectName
FName, ObjectFlags u32, SerialSize i64, SerialOffset i64, bForcedExport, bNotForClient,
bNotForServer (i32 each), PackageGuid (16), PackageFlags u32, bNotAlwaysLoadedForEditorGame,
bIsAsset (i32 each), then five i32 preload-dependency fields.
**Package index:** positive = export (1-based), negative = import, 0 = none.

The depends map, asset registry data and preload dependency list after the export table are
carried verbatim (`tail`) by `uasset.js`; `tools/lib/package.js` decodes them (see
`package_json.md`, which also covers every property type and the native data of each class).

## Export data

An export's bytes are in the `.uexp` at `SerialOffset - TotalHeaderSize`, length `SerialSize`.
The `.uexp` ends with the 4-byte package tag.

### Tagged properties (FPropertyTag)

```
FName  name            "None" ends the list
FName  type            IntProperty, StructProperty, ...
i32    size            of the value that follows the tag
i32    array index
type-specific:  StructProperty: FName struct name + 16-byte GUID
                BoolProperty:   u8 value (size is 0)
                ByteProperty / EnumProperty: FName enum name
                ArrayProperty / SetProperty: FName inner type
                MapProperty:    FName key type, FName value type
u8     has property GUID (+16 bytes if 1)
value  `size` bytes
```

Values: integers and floats as their binary form; `NameProperty`/`EnumProperty` an FName;
`StrProperty`/`AssetObjectProperty` an FString; `ObjectProperty` a package index;
`ByteProperty` one byte, or an FName when it holds an enum value. `StructProperty` is either a
natively serialized struct (`Vector`, `Vector2D`, `Rotator`, `Quat`, `LinearColor`, `Color`,
`IntPoint`, `Guid`, `RichCurveKey`, `StringAssetReference`) or a nested tagged property list.
`ArrayProperty` is an i32 count followed by the elements; an array of structs has one inner
property tag before the elements. A value the reader does not decode, or whose decoded length
differs from the tag's size, is kept as raw bytes (the size makes that exact); see
`uninterpreted_values` in `audit/sheets_roundtrip.json` for how many the sheets contain.

Native struct layouts added for the sheets, each confirmed by the tag sizes and the byte-exact
round trip of every sheet that contains one:

| Struct | Layout |
|---|---|
| `RichCurveKey` (27 bytes) | u8 InterpMode, u8 TangentMode, u8 TangentWeightMode, f32 Time, f32 Value, f32 ArriveTangent, f32 ArriveTangentWeight, f32 LeaveTangent, f32 LeaveTangentWeight |
| `StringAssetReference` | FString asset path |

`MapProperty`:

```
i32  number of keys to remove, then those keys
i32  entry count
entries: key, value
```

Keys and values are serialized like array elements, without tags: an `EnumProperty` or
`NameProperty` key is an FName, a `StrProperty` an FString, an `IntProperty` 4 bytes, an
`Int16Property` 2 bytes, an `ObjectProperty` a package index. A `StructProperty` value is a
tagged property list ending in `None` (the map's tag does not name the struct). Key/value type
pairs present in the sheets: Name/Str, Str/Str, Enum/Int, Enum/Int16, Enum/Object, Name/Object,
Enum/Struct. Maps with other key or value types (`ByteProperty`, `BoolProperty`, struct keys)
are not decoded and would stay raw.

Not decoded, because no sheet contains them: `TextProperty`, `SetProperty`, and native structs
other than those listed (for example `Box` in `CameraAnim` and `ExpressionInput` in material
functions, seen while surveying classes that are not turned into sheets).

### UDataTable

```
tagged properties of the table object (RowStruct, ...), None
i32  0
i32  row count
rows: FName row name + tagged properties of the row struct, None
```

1,589 DataTables, 146 row structs, 19,353 rows (`audit/sheets.json`).

### Plain tagged-property objects

Many data-asset classes add nothing to `UObject` serialization. Such an export is:

```
tagged properties, None
i32  0
```

In a package made only of such exports, the exports tile the `.uexp` in `SerialOffset` order with
no gaps and the 4-byte package tag follows the last one. `tools/sheets_build.js` turns these
packages into object sheets when the main asset's class is defined in `/Script/TresGame` or is
`CurveFloat` / `CurveVector` (`TresTexturePump` is left out for size): 5,855 packages, 8,001 objects, 109 object classes
(`audit/sheets.json`), all compiling back to the original bytes (`audit/sheets_roundtrip.json`,
`by_kind.objects`). The paks contain no export of class `CurveTable` or `StringTable` (`export_classes` in `audit/uasset_roundtrip.json`).
