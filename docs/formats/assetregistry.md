# Asset registry state (`DevelopmentAssetRegistry.bin`)

Code: `tools/lib/assetregistry.js`, driver `tools/loose_files.js`. Evidence:
`audit/loose_files.json` - both copies (live and `_superseded/pakchunk0-WindowsNoEditor`)
decode to `assets_converted/loose/<path>.json` and rebuild byte for byte.

| Field | Type | Notes |
|---|---|---|
| version GUID | 16 bytes | `E7 9E 7F 71 3A 49 B0 E9 32 91 B3 88 07 81 38 1B` |
| version | int32 | 4 |
| name table offset | int64 | file offset of the name table; equals the end of the package data |
| assets | int32 count + FAssetData | |
| dependency nodes | int32 count + node | |
| package data | int32 count + { name, int64 DiskSize, 16-byte PackageGuid } | |
| name table | int32 count + { FString, uint16 hash, uint16 hash } | ends the file |

A name is int32 index into the file's name table + int32 number.

FAssetData: names ObjectPath, PackagePath, AssetClass, PackageName, AssetName; int32 count +
{ name key, FString value } (tags; none in these files); int32 count + int32 (chunk ids);
uint32 PackageFlags.

Dependency node: FAssetIdentifier - u8 bit field saying which names follow (1 PackageName,
2 PrimaryAssetType, 4 ObjectName, 8 ValueName) and those names - then five int32 counts, then
that many int32 node indexes in total, list after list. In the live file 180,488 / 12,369 /
300 / 0 / 201,026 of the 227,598 nodes have a non-empty first ... fifth list.

What the data shows about the lists (checked over all 996,875 entries of the live file):
the fifth list is exactly the inverse of the first four - whenever node A names B in lists
1-4, B names A in list 5, and the reverse - so it is `referencers`; every target of the third
list is an identifier with field bits 13 (package + object + value name), i.e. a searchable
name, so it is `searchableNameDependencies`; all targets of the first two lists are plain
package identifiers. The first, second and fourth lists are `dependencies0`, `dependencies1`,
`dependencies3` in the JSON: the stock engine's order would make them hard, soft and manage
dependencies, which the data cannot confirm.
