# Localization resources: `.locres`, `.locmeta`

Code: `tools/lib/locres.js`, driver `tools/loose_files.js`. Evidence:
`audit/loose_files.json` - 627 `.locres` and 13 `.locmeta` files, all decoded to
`assets_converted/loose/<path>.json` and rebuilt byte for byte.

Strings are FStrings: int32 length including the terminator, positive = 8-bit characters,
negative = UTF-16 code units; the JSON keeps which form each string used.

## `.locres`, version 1 (624 files)

| Field | Type | Notes |
|---|---|---|
| magic | 16 bytes | `0E 14 74 75 67 4A 03 FC 4A 15 90 9D C3 37 7F 1B` |
| version | u8 | 1 |
| string array offset | int64 | file offset of the string array; equals the end of the key table in every file |
| namespaces | int32 count | |
| per namespace | FString name, int32 key count | |
| per key | FString key, uint32 source string hash, int32 index into the string array | |
| string array | int32 count + FString | at the offset above; ends the file |

## `.locres`, legacy (3 files)

No magic: `Engine/Content/Localization/Engine/{de,fr,zh}/Engine.locres`.

int32 namespace count; per namespace FString name, int32 key count; per key FString key,
uint32 source string hash, FString text.

## `.locmeta`

16-byte magic `4F EE 4C A1 68 48 55 83 6C 4C 46 BD 70 DA 50 7C`, u8 version (0), FString
native culture, FString path of the native `.locres`.
