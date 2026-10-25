# Cooked textures (`UTexture2D` and relatives) and their DDS conversion

Reader/writer: `tools/lib/texture.js`. Converter and proof: `tools/texture_convert.js`.
Evidence: `audit/texture_manifest.json` (one row per texture, in-memory round trip),
`audit/texture_summary.json` (the counts quoted here), `audit/texture_export.json` and
`audit/texture_export_summary.json` (the files written to `assets_converted/textures/` and the
round trip repeated from those files). All integers little-endian. Package header, FName and
tagged-property encodings are in `uasset.md`.

Confidence used below: **confirmed** = read on every export listed in the scope and the bytes
were rebuilt identically from the parsed fields; **inferred** = the bytes round-trip, but the
*meaning* given to the field comes from the public UE 4.17 source layout or from a pattern, not
from the game.

## Scope

76,216 exports in 38,215 packages (`.uasset` and `.umap`), all of them parsed and rebuilt:

| Class | Exports | Round trip | Pixel formats (count) |
|---|---|---|---|
| `Texture2D` | 48,062 | 48,062 | PF_DXT1 15,950 · PF_B8G8R8A8 13,711 · PF_BC7 8,768 · PF_BC5 7,418 · PF_BC4 1,137 · PF_DXT5 818 · PF_G8 147 · PF_BC6H 64 · PF_FloatRGBA 49 |
| `LightMapTexture2D` | 18,330 | 18,330 | PF_DXT5 18,296 · PF_DXT1 34 |
| `ShadowMapTexture2D` | 9,637 | 9,637 | PF_G8 9,353 · PF_B8G8R8A8 284 |
| `TextureCube` | 184 | 184 | PF_BC6H 157 · PF_FloatRGBA 18 · PF_DXT1 7 · PF_BC7 2 |
| `TextureLightProfile` | 3 | 3 | PF_FloatRGBA 3 |

Pixel data: 42,644,308,862 bytes in total, 37,379,695,360 of them in `.ubulk` files.
Not covered: `TextureRenderTarget2D` (42 exports) and `TresTexturePump` (287) were not examined.

## Export layout

```
tagged properties ... None                      (uasset.md)
i32   object GUID flag                          0 on all 76,216
u8[2] FStripDataFlags of UTexture               01 00 on all
u8[2] FStripDataFlags of UTexture2D             01 00 on all
i32   bCooked                                   1 on all
repeat:
  FName pixel format name                       "None" ends the list
  i32   SkipOffset
  FTexturePlatformData
subclass data                                   see "Subclasses"
```

Exactly one platform entry precedes the `None` terminator in every export.

| Field | Confidence | Notes |
|---|---|---|
| tagged properties | confirmed | Property names seen: `ImportedSize`, `SRGB`, `LightingGuid`, `LODGroup`, `CompressionSettings`, `AddressX/Y`, `NeverStream`, `Filter`, `LODBias`, `ShadowmapFlags`, and the game's own `SqexTextureQualityLODBias`, `bSqexOverwriteMaxAllowedMip`, `SqexMinLODSize`, `m_ResidentLevel`; full counts in the summary under `property`. `SRGB` appears only as `false` (60,518 exports); absent means the class default. |
| object GUID flag | confirmed value, inferred meaning | |
| strip flags, `bCooked` | confirmed value, inferred meaning | |
| pixel format FName | confirmed | Equals the `PixelFormat` string of the platform data on all 76,216. |
| `SkipOffset` | confirmed | Equals the offset of the byte after the platform data, measured in the virtual file `<.uasset><.uexp>` (i.e. `.uexp` position + `TotalHeaderSize`); holds on all 76,216. |

### FTexturePlatformData

```
i32     SizeX
i32     SizeY
i32     NumSlices          1, or 6 for TextureCube
FString PixelFormat        "PF_DXT1", ... (ANSI on all)
i32     FirstMip
i32     mip count
mips
```

| Field | Confidence | Notes |
|---|---|---|
| `SizeX`, `SizeY` | confirmed value, inferred meaning | Equal to the first stored mip on 76,124 exports. On 87 they are the first stored mip shifted left by `FirstMip`; the remaining 5 are textures whose first stored mip is nominally narrower than 4 px and is recorded as 4 (see below). So the header keeps the size *before* cooked-out mips are dropped. |
| `NumSlices` | confirmed | 1 on 76,032; 6 on the 184 cubes. |
| `FirstMip` | confirmed value, inferred meaning | 0 on 76,124; 1 (69), 2 (19), 6 (1), 11 (3). |
| mip count | confirmed | 1 to 13. |

### Mip (FTexture2DMipMap)

```
i32  bCooked               1 on all 635,483 mips
u32  BulkDataFlags
i32  ElementCount          payload size in bytes
i32  SizeOnDisk            equal to ElementCount on all
i64  OffsetInFile
[payload]                  only when stored inline
i32  SizeX
i32  SizeY
```

Two flag values exist, and they decide where the payload is:

| `BulkDataFlags` | Mips | Payload |
|---|---|---|
| `0x0048` | 478,621 | **Inline**, directly after `OffsetInFile`. `OffsetInFile` equals the payload's offset in the virtual file `<.uasset><.uexp>` (holds on all 478,621). |
| `0x0501` | 156,862 | **In the `.ubulk`**, at `OffsetInFile + BulkDataStartOffset` (the summary field of the `.uasset`; `OffsetInFile` is negative or small because the stored value is relative to that field). |

Flag bit names from the UE 4.17 source (inferred): `0x0001` PayloadAtEndOfFile, `0x0008`
SingleUse, `0x0040` ForceInlinePayload, `0x0100` PayloadInSeperateFile, `0x0400`
Force_NOT_InlinePayload. The location rule itself is confirmed by the round trip.

Facts about placement (confirmed on all rows of the manifest):

- 18,010 textures are entirely inline; 58,206 have 1 to 6 leading mips in the `.ubulk`.
- A `.ubulk` mip never follows an inline mip, and a texture's `.ubulk` mips are contiguous in
  mip order (largest first).
- In 58,200 of the 58,206 streamed textures the first inline mip has a longest side of 64 px
  (128 px in 4, 256 px in 2).
- Every one of the 28,136 `.ubulk` files is tiled exactly, without gaps or overlap, by the mips
  of the textures in its package: rebuilding all of them reproduces the whole file.

Payload size is what the pixel format implies for the mip's nominal size
(`max(1, top >> i)`), times `NumSlices`; true for all 635,483 mips. For block formats that is
`ceil(w/4) * ceil(h/4) * block bytes`, so 2×2 and 1×1 mips still hold one whole block.

`SizeX`/`SizeY` of a mip are the nominal size on 540,848 mips; on 94,635 mips of
block-compressed textures the recorded size is clamped to 4 (a 2×2 or 1×1 mip is recorded as
4×4). The payload size is the same either way.

### Pixel formats

`B`: bytes per 4×4 block; `P`: bytes per
pixel. The DDS column is how `tools/lib/texture.js` labels the same bytes.

| `PixelFormat` | Size | DDS header | Exports | Bytes |
|---|---|---|---|---|
| PF_DXT1 | B 8 | FourCC `DXT1` | 15,991 | 9,216,011,816 |
| PF_DXT5 | B 16 | FourCC `DXT5` | 19,114 | 8,916,162,192 |
| PF_BC4 | B 8 | FourCC `ATI1` | 1,137 | 208,202,688 |
| PF_BC5 | B 16 | FourCC `ATI2` | 7,418 | 11,586,382,880 |
| PF_BC6H | B 16 | DX10 header, DXGI 95 (BC6H_UF16) | 221 | 817,204,000 |
| PF_BC7 | B 16 | DX10 header, DXGI 98 (BC7_UNORM) | 8,770 | 6,283,190,512 |
| PF_B8G8R8A8 | P 4 | 32-bit masks R `00FF0000` G `0000FF00` B `000000FF` A `FF000000` | 13,995 | 3,481,150,528 |
| PF_G8 | P 1 | 8-bit luminance | 9,500 | 2,074,029,398 |
| PF_FloatRGBA | P 8 | FourCC 113 (A16B16G16R16F) | 70 | 61,974,848 |

The size column is confirmed (it predicts every payload length). The channel interpretation is
inferred from the format names: no pixels were decoded here. In particular the choice of
BC6H **unsigned** and of non-sRGB BC7 in the DDS header is a labelling decision; the sRGB flag
of a texture is the `SRGB` property, kept in the sidecar. The library also has table entries
for formats that do not occur in the game (PF_DXT3, PF_R8G8B8A8, PF_A8, ...); those are untested.

### Subclasses

| Class | Bytes after the `None` terminator | Confidence |
|---|---|---|
| `Texture2D`, `ShadowMapTexture2D`, `TextureLightProfile`, `TextureCube` | none (0 on all) | confirmed |
| `LightMapTexture2D` | 4 bytes on all 18,330 | confirmed length; meaning inferred (`ELightMapFlags` in the UE source). Kept verbatim. |

`TextureCube` uses the same layout with `NumSlices` 6: each mip payload is six face images back
to back. That the six parts are faces in +X, −X, +Y, −Y, +Z, −Z order is inferred, not checked.

## Conversion to DDS

`node tools/texture_convert.js export` writes, per texture:

- `assets_converted/textures/<package path>[.<export name>].dds` — a standard DDS holding every
  stored mip, **the original bytes unmodified** (no decode, no re-encode). The export name is
  appended when it differs from the package's base name (textures inside maps and other
  multi-export packages).
- `...texture.json` — the sidecar: every field of the export that is not pixel data (tagged
  properties, flags, the platform header, each mip's bulk header and recorded size, subclass
  bytes).

DDS details: the header describes the first *stored* mip, and the mip count is the stored count.
Legacy headers are used where a FourCC or bit masks exist, the DX10 extension only for BC6H and
BC7. Cubes set `DDSCAPS2_CUBEMAP` with all six faces. A DDS is face-major (each face followed by
its mip chain) while the export is mip-major, so for cubes the same bytes are reordered; that is
the only case where the DDS pixel area is not a plain concatenation of the payloads.

Where the export records a clamped mip size (4×4 for a nominal 2×2), the DDS follows the nominal
chain, which is what a DDS reader computes; the recorded values are in the sidecar. Five
textures have a first stored mip that is nominally narrower than 4 px (for example a 2048² texture
with `FirstMip` 11, whose only stored mip is nominally 1×1): their DDS header carries the recorded,
clamped size (4×4 in that example).

Independent reader check (manual spot check, not part of the audit files): the Windows Imaging
Component DDS decoder opened the four DXT1/DXT5 2D samples tried and reported the expected
size and one frame per mip. That decoder does not support the other formats, and it rejected the
cube sample in every header variant tried, so the BC4/BC5/BC6H/BC7/uncompressed/float and cube
headers have **not** been validated by a third-party reader.

## Round trip

`rebuild(pkg, sidecar, dds)` in `tools/lib/texture.js` takes the sidecar, the `.dds` bytes and
the package's name table (FNames are stored as strings; `BulkDataStartOffset` also comes from
the `.uasset`). It returns the export's bytes and its `.ubulk` pieces with their offsets.

What `roundtrip_matches: true` means for a row:

1. the rebuilt export equals the export's slice of the `.uexp` byte for byte
   (`sha256_source_export` = `sha256_rebuilt_export`), and
2. every rebuilt `.ubulk` piece equals the original bytes at the same offset
   (`sha256_source_bulk` = `sha256_rebuilt_bulk`, hashes over the texture's pieces in mip order).

In `verify` mode the DDS and the sidecar text exist only in memory; in `export` mode both are
written, read back from disk, and the rebuild uses what was read.

| Run | Textures | Matches | Fails |
|---|---|---|---|
| `verify` (`audit/texture_manifest.json`) | 76,216 | 76,216 | 0 |
| `export` (`audit/texture_export.json`) | 76,216 | 76,216 | 0 |

Whole-file proof (`package_files` in each row). A whole `.uexp` can only be rebuilt when every
export in the package is a texture; a `.ubulk` whenever the package's textures tile it.

| Packages | `.uexp` | `.ubulk` |
|---|---|---|
| 27,434 | identical | identical |
| 10,016 | identical | none in the package |
| 702 | other exports present: only the texture exports are proven | identical |
| 63 | other exports present: only the texture exports are proven | none in the package |

By class: all 184 `TextureCube`, all 3 `TextureLightProfile` and 37,263 `Texture2D` exports are in
packages whose whole `.uexp` is reproduced. The other 10,799 `Texture2D` (10,207 of them inside
`.umap` packages), and every `LightMapTexture2D` and `ShadowMapTexture2D`, share their `.uexp`
with non-texture exports; for those the claim is limited to the export's own byte range plus the
whole `.ubulk`.

Output size: 42,654,244,330 bytes of `.dds` in 76,216 files, plus one sidecar each
(322,170,528 bytes in total).

## Not covered

- No pixel decoding: nothing here converts to PNG/EXR, and channel meaning is taken from format
  names.
- `TextureRenderTarget2D` and `TresTexturePump` exports.
- The reverse tool rebuilds exports from *unmodified* DDS data. Writing a changed texture back
  would also need `SkipOffset`, inline offsets and the export table's sizes recomputed; the
  rules for the first two are documented above, but no such repacker exists yet.
