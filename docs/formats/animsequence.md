# `UAnimSequence` exports

Status: **decoded and converted.** `tools/lib/anim.js` reads every byte of every export into a
named field and writes the same bytes back; `tools/anim_convert.js` converts every sequence to a
glTF animation and proves the round trip per file; `tools/anim_validate.js` checks the output.

| | |
|---|---|
| Exports | 51,487 `AnimSequence` (3,002,489,648 bytes), all in `.uasset`/`.uexp` packages |
| Byte-exact re-serialisation (parse → write) | 51,487 / 51,487 |
| Byte-exact rebuild from the `.glb` + sidecar on disk | 51,487 / 51,487 |
| Failures | 0 |
| glTF-Validator 2.0.0-dev.3.10, every `.glb` | 51,487 files, 0 errors, 0 warnings |
| Evidence files | `audit/anim_manifest.json` (summary, failures), `audit/anim_export.json` (one row per export), `audit/anim_validation.json` |

Two kinds of evidence are used below and named on each item:

- **exe** — the game's own code in `base/KH3.exe` (image base 0x140000000), read from the
  decompiled C in `build/decomp/` and, where the decompiler drops detail, from the disassembly.
  Addresses are virtual addresses.
- **data** — a property that the converter checked on all 51,487 exports; the count is in
  `audit/anim_manifest.json` (`sequence_stats`).

The stream layout is the stock UE 4.17 one. What an earlier survey took for a non-stock layout is
the difference between file order and memory order described under "Compressed byte stream".
The Square Enix differences found are in the decoders, not in the layout: see "Decoding".

## Export layout

"array" = i32 count, then the elements. bool = i32 holding 0 or 1.

| Field | Type | Evidence |
|---|---|---|
| tagged properties … `None` | property list | `UObject::Serialize`, called first by 0x143F77080. Occurring names: `NumFrames`, `SequenceLength`, `Skeleton`, `TrackToSkeletonMapTable`, `Notifies`, `m_InterpolationTime`, `bEnableRootMotion`, `m_DisableInterpolation`, `RefPoseType`, `AdditiveAnimType`, `RefPoseSeq`, `RateScale`, `RefFrameIndex`, `bForceRootLock`, `Interpolation`, `AssetUserData`, `RootMotionRootLock`. No value in any sequence is left as raw bytes by `tools/lib/uprops.js` (data: 0 raw values) |
| object GUID flag | i32, 0 | data: 0 on all (tail of `UObject::Serialize`, see `uasset.md`) |
| `SkeletonGuid` | 16 bytes | exe 0x143F77080: read into object +0x50 when the package version is above 0x151. data: equals the 16 bytes that follow an empty retarget-source map in the referenced `Skeleton` export on 51,486 sequences; the remaining one (`Maps/bt/bt_gimmick/g_bt_FlagC/mdl/g_bt_FlagC_Anim`) references a skeleton whose map is not empty, so the position of its GUID was not located and nothing was compared |
| strip flags | 2 × u8 | exe 0x143F4DB20 (call to 0x1440D42A0). data: `01 00` on all. Bit 0 of the first byte = editor data stripped; when clear the code reads raw animation data into +0xE8 — never the case, and `anim.js` refuses such an export |
| `bSerializeCompressedData` | bool | exe 0x143F4DB20. data: 1 on all |
| `KeyEncodingFormat` | u8 | exe 0x143F4DF10, object +0x138 |
| `TranslationCompressionFormat` | u8 | exe 0x143F4DF10, +0xF8 |
| `RotationCompressionFormat` | u8 | +0xF9 |
| `ScaleCompressionFormat` | u8 | +0xFA |
| `CompressedTrackOffsets` | array of i32 | +0x100 |
| `CompressedScaleOffsets.OffsetData` | array of i32 | +0x110 |
| `CompressedScaleOffsets.StripSize` | i32 | +0x120 |
| `CompressedTrackToSkeletonMapTable` | array of i32 | +0xD8. One bone index per track (index into the skeleton's reference skeleton). data: every value is a bone of the referenced skeleton, no bone is named twice |
| `CompressedCurveData` | property list … `None` | exe 0x143F4DF10 (call to 0x143F0DE90 on +0x158). data: always exactly one property, `FloatCurves` (array of `FloatCurve` structs); non-empty on 1,057 sequences, 1,904 curves. Each curve: `FloatCurve` (`RichCurve`: `PreInfinityExtrap`, `PostInfinityExtrap`, `DefaultValue`, `Keys` = array of 27-byte `RichCurveKey`), `Name` (`SmartName`, 8 bytes = one FName: every one of the 1,904 is a valid name-table reference), `CurveTypeFlags` |
| `CompressedRawDataSize` | i32 | +0x168. Not used by the converter; sum over all sequences 7,153,929,512 |
| byte count | i32 | exe 0x143F4DF10: read, then that many bytes into a temporary array |
| compressed byte stream | bytes | see below |
| `bUseRawDataOnly` | bool | exe 0x143F4DB20, +0x1B0. data: 0 on all |

0x143F4DB20 (`UAnimSequence::Serialize`) has two more branches that no file takes: an older
framework version (custom version below 5: a bare byte array), and one extra bool (+0x1B1) read
when archive flag bits 0x401000 are set. The export ends exactly after `bUseRawDataOnly` on all
51,487 exports, so neither is present.

### Enumerations

`KeyEncodingFormat`: 0 `AKF_ConstantKeyLerp`, 1 `AKF_VariableKeyLerp`, 2 `AKF_PerTrackCompression`.
Compression formats: 0 `ACF_None`, 1 `ACF_Float96NoW`, 2 `ACF_Fixed48NoW`,
3 `ACF_IntervalFixed32NoW`, 4 `ACF_Fixed32NoW`, 5 `ACF_Float32NoW`, 6 `ACF_Identity`
(names: the engine's reflected enums; numbers: the decoders' comparisons in the exe).

| Key encoding | Sequences | T / R / S format bytes | Offsets per track | Scale table |
|---|---|---|---|---|
| `AKF_PerTrackCompression` | 48,645 | 6 / 6 / 6 on all (not used by this encoding: each block names its own format) | 2 | present on 40,016 (StripSize 1), absent on 8,629 (StripSize field 2) |
| `AKF_ConstantKeyLerp` | 2,833 | 0 / 1 / 0 on all | 4 | present on 2,078, absent on 755 (StripSize 2) |
| `AKF_VariableKeyLerp` | 9 | 0 / 1 / 0 on all | 4 | present on 6, absent on 3 (StripSize 2) |

## Offset tables

- Per-track encoding: `CompressedTrackOffsets[2t]` = translation, `[2t+1]` = rotation;
  `CompressedScaleOffsets.OffsetData[t * StripSize]` = scale. −1 = the component has no block.
  (exe 0x143F0EC00, 0x143F120E0.)
- Key-lerp encodings: `CompressedTrackOffsets[4t .. 4t+3]` = translation offset, translation key
  count, rotation offset, rotation key count; scale `OffsetData[2t]`, `[2t+1]` = offset, key
  count. (exe 0x143F0ED10.) data: no offset is −1 in these encodings.

**The offsets are memory positions, not file positions.** The loader allocates a buffer of
"byte count" bytes and then reads the blocks from the file *sequentially*, in the order track 0
translation, rotation, scale, track 1 …, storing each block at its offset in that buffer (exe:
per-track `ByteSwapIn` 0x143F0EC00 → 0x143F0BE50 per block; key-lerp `ByteSwapIn` 0x143F0ED10).
So the file order is always T, R, S per track, whatever the offsets say.

data: laid out by their offsets, the blocks never overlap and fill the buffer exactly, on all
sequences. The memory order equals the file order on every sequence without a scale table and on
every key-lerp sequence. On 39,836 of the 40,016 per-track sequences with a scale table it
differs: in memory each track is scale, translation, rotation. On the other 180 the two orders
coincide; 178 of those have no scale block at all (every scale offset is −1).

## Compressed byte stream

A concatenation of blocks. Every block is a multiple of 4 bytes; padding bytes are 0x55 (exe
0x143F0C190 writes 0x55 when saving; data: every pad byte in every file is 0x55). There are no
other bytes: 10,778,631 blocks account for all 2,665,143,816 stream bytes. 41 sequences have an
empty stream (one track, no blocks; e.g. `Character/enemy/e_bx059/anm/rt/body/750`).

Tables in the exe used for sizes:

| Address | Content |
|---|---|
| 0x1475E7918 | u8[8][8], per-track components per key by [format][flags & 7]: format 0: 4…; 1 and 2: 3,1,1,2,1,2,2,3; 3: 6,2,2,4,2,4,4,6; 4 and 5: 1…; 6 and 7: 0… |
| 0x1475E7A38 | i32[7] bytes per component (also the rotation stride of the key-lerp codecs): 4,4,2,4,4,4,0 |
| 0x1475E79F8 / 0x1475E7A18 | key-lerp translation: bytes per component 4,4,4,4,4,4,0 / components 3,3,3,1,3,3,0 |
| 0x1475E7A58 | key-lerp rotation components 4,3,3,1,1,1,0 |
| 0x1475E7A78 / 0x1475E7A98 | key-lerp scale: 4,4,4,4,4,4,0 / 3,3,3,1,3,3,0 |

### Per-track block (`AKF_PerTrackCompression`)

exe: 0x143F0BE50 (load), 0x143F0C190 (same walk through an archive), and the three decoders.

```
u32   header      bits 28-31 format, bits 24-27 flags, bits 0-23 key count
f32[] bounds      only format 3: (min, range) for each component whose flag bit is set
key[] keys        key count x bytes per key
      pad to 4
      -- only when flag bit 3 is set:
u8[] or u16[] frame table   one entry per key; u16 when NumFrames > 255
      pad to 4
```

Flags: bit 0 = X stored, bit 1 = Y, bit 2 = Z, bit 3 = a frame table follows.
Bytes per key = components × bytes per component, except format 3: 4.
For formats 1 and 2 with flags & 7 = 0 the table gives 3 components (not seen in the data).

| Format | Key | Occurs for |
|---|---|---|
| 1 `ACF_Float96NoW` | one f32 per stored component (rotation: the decoder always reads three; data: rotation blocks have flags & 7 = 7) | translation, rotation, scale |
| 2 `ACF_Fixed48NoW` | one u16 per stored component | translation, rotation, scale |
| 3 `ACF_IntervalFixed32NoW` | one u32 | translation, scale |
| 0, 4, 5, 6 | (sizes from the tables; the decoders handle 4, 5, 6) | never in the data |

Frame table (data, 1,827,588 tables): first entry 0, last entry NumFrames − 1, strictly
increasing — all of them.

### Key-lerp block (`AKF_ConstantKeyLerp`, `AKF_VariableKeyLerp`)

exe: 0x143F10150 (translation), 0x143F0F520 (rotation), 0x143F0FB30 (scale); the variable
variants 0x143F10460 and 0x143F0F830 call these and then read the frame table. (The scale
variant of the latter, 0x143F0FE40, is identified by its vtable slot only.)

```
f32[6] bounds     only when the effective format is 3
key[]  keys       key count x (bytes per component x components)   [tables above]
       pad to 4
       -- only AKF_VariableKeyLerp and key count > 1:
u8[] or u16[] frame table   u16 when NumFrames > 255
       pad to 4
```

Effective format: the sequence's format byte for that component; with exactly one key it is 0
(translation, scale) or 1 (rotation) instead. data: the format bytes are 0 / 1 / 0 on all, so
every key is three f32 (12 bytes).

### Blocks and keys by format (all sequences)

| Encoding | Component | Format | Blocks | Keys | Flag values seen |
|---|---|---|---|---|---|
| per-track | translation | Float96NoW | 2,593,353 | 26,603,983 | 1–7, 9–15 |
| per-track | translation | IntervalFixed32NoW | 857,662 | 79,503,041 | 3, 5, 6, 7, 11, 13, 14, 15 |
| per-track | translation | Fixed48NoW | 3,277 | 109,033 | 1–7, 9–15 |
| per-track | rotation | Fixed48NoW | 2,966,341 | 169,611,756 | 1–7, 9–15 |
| per-track | rotation | Float96NoW | 117,648 | 5,820,588 | 7, 15 |
| per-track | scale | Fixed48NoW | 2,610,562 | 2,611,060 | 7 |
| per-track | scale | Float96NoW | 568,910 | 24,098,141 | 1–7, 15 |
| per-track | scale | IntervalFixed32NoW | 263,028 | 31,988,583 | 3, 5, 7, 15 |
| constant | translation | None | 285,234 | 17,854,013 | — |
| constant | rotation | Float96NoW | 285,234 | 22,308,871 | — |
| constant | scale | None | 226,436 | 4,241,853 | — |
| variable | translation | None | 420 | 38,390 | — |
| variable | rotation | Float96NoW | 420 | 30,232 | — |
| variable | scale | None | 106 | 874 | — |

Total 384,820,418 keys in 4,209,076 tracks. 1,322,192 per-track components have no block.

## Decoding

exe: per-track `GetBoneAtom` 0x143F120E0 calls translation 0x143F1BF10, rotation 0x143F15D00
and — only when the scale table is not empty — scale 0x143F19580. Key-lerp decoders: constant
translation 0x143F19F60, rotation (format 1) 0x143F12340, scale 0x143F16890; variable
translation 0x143F1AB60, rotation 0x143F13B70, scale 0x143F17B20. Vtables at 0x1475EC990 ff.

### Key values

| Component, format | Value |
|---|---|
| rotation, Float96NoW | X, Y, Z = the three f32; W = sqrt(1 − X² − Y² − Z²), or 0 when that is not positive |
| rotation, Fixed48NoW | component = (u16 − 32767) × 3.051851e-05; a component that is not stored is 0; W = sqrt(1 − Σ (u16 − 32767)² × 9.313794e-10), or 0 when negative |
| rotation, IntervalFixed32NoW | X = ((v >> 21) − 1023) × 0.0009775171 × range + min, Y = (((v >> 10) & 0x7FF) − 1023) × 0.0009775171 × range + min, Z = ((v & 0x3FF) − 511) × 0.0019569471 × range + min; W as for Float96NoW (in the exe; not in the data) |
| translation / scale, Float96NoW | the stored f32 values (flags & 7 = 0: three) |
| translation / scale, Fixed48NoW | u16 − 255, **with no scale factor**: the value is an integer (exe: no multiplication in 0x143F1BF10 / 0x143F19580; data: such keys reproduce reference-pose values like (0, −90, 7) and scale (1, 1, 1)) |
| translation / scale, IntervalFixed32NoW | X = ((v & 0x3FF) − 511) × 0.0019569471 × range + min, Y = (((v >> 10) & 0x7FF) − 1023) × 0.0009775171 × range + min, Z = ((v >> 21) − 1023) × 0.0009775171 × range + min — the bit order is the reverse of the rotation's |
| key-lerp translation / scale | three f32 |
| key-lerp rotation | as rotation Float96NoW |

Components that are not stored, and missing blocks:

| Case | Value | Evidence |
|---|---|---|
| translation component not stored (flag clear) | 0 | exe 0x143F1BF10 |
| **scale component not stored**, and scale block of format 6 | **0.0001** (0x38D1B717) | exe 0x143F19580. Square Enix change; the same code path for translation gives 0 |
| translation offset −1 | (0, 0, 0) | exe: the vector at 0x149B5C798, which lies beyond the file-backed part of `.data` and has no writer in the decompiled code other than self-copies |
| rotation offset −1 | (0, 0, 0, 1) | exe: constant at 0x14871E410 |
| scale offset −1, scale table present | (0, 0, 0) | exe: the same vector 0x149B5C798 |
| no scale table | (1, 1, 1) | exe 0x143F120E0 initialises the transform to identity and does not call the scale decoder |

Decoded rotations are unit quaternions: over all 197,771,447 rotation keys the largest
| |q|² − 1 | is 2.3e-7. 117,785 keys have W = 0 with X² + Y² + Z² marginally above 1.

### Key times and interpolation

With `p` = time / `SequenceLength` (0x143F120E0; `SequenceLength` at +0xA0, `NumFrames` at +0xC0):

- block without frame table: the keys are spread evenly, key position = p × (key count − 1);
- block with frame table: frame position = p × (NumFrames − 1), bracketed by the table entries.

Between two keys: translation linear; rotation a normalised linear blend along the shorter arc;
**scale geometric per component** — a × powf(b / a, t) when a × b > 0, linear otherwise (exe
0x143F19580, 0x143F16890, 0x143F17B20; the callee 0x14587199A is the import thunk of `powf`).
The geometric scale blend is a Square Enix change.

The blend factor is forced to 0 (hold the earlier key) when the byte at +0x188 is 1
(`Interpolation` = `EAnimInterpolationType::Step`; offset from the class registration) or the
byte at +0x18D is non-zero. +0x18D is `m_DisableInterpolation`: the registration function
0x144661760 determines that property's offset with the setter 0x14465C870, which writes +0x18D.
560 sequences have one of the two set.

`NumFrames` and `SequenceLength`: (NumFrames − 1) / SequenceLength = 30 on 51,035 sequences;
29 sequences have one frame; the largest `NumFrames` is 5,269 (3,851 sequences use u16 frame
tables).

## Conversion

`node tools/anim_convert.js index`, then `node tools/anim_convert.js export` (8 workers,
resumable; a full run takes about five minutes). Output per sequence, under
`assets_converted/animations/<package path>`:

- `<name>.glb` — node 0 is the same UE-to-glTF root matrix as in the mesh files; nodes 1… are
  the bones of the referenced `Skeleton` asset with its reference pose, in bone order, in UE
  units. One animation with three channels (translation, rotation, scale) for every track,
  targeting the track's bone. A component without a block gets a one-key channel holding the
  value from the table above, so the animation alone gives what the game computes for a tracked
  bone. Key times are seconds as defined above. Rotations are normalised. Sampler interpolation
  is `STEP` for the 560 sequences above, `LINEAR` otherwise.
- `<name>.anim.json` — the export's fields other than the stream (properties, tables, curve
  data), the skeleton path, and the index of the `.glb` buffer view `ue:compressedByteStream`,
  which holds the stream's bytes. The rebuild takes the stream from that view, runs it through
  the block parser and writer, and serialises the export; the float channels are derived data
  and are not used for the rebuild.

13,717,938,276 bytes of `.glb` and 1,017,510,706 bytes of sidecars.

Differences between the glTF animation and the game, by construction:

- glTF `LINEAR` scale is linear and glTF `LINEAR` rotation is a spherical blend; the game uses
  the blends described above. Values at the keys are the game's.
- Additive sequences (`AdditiveAnimType` set: 126 `AAT_LocalSpaceBase`, 134
  `AAT_RotationOffsetMeshSpace`) are written as their stored (delta) keys; applying them to a
  base pose is not done.
- Bones without a track keep the reference pose of the node. What the game does with such bones
  (and any retargeting between skeleton and animation translations) was not examined.
- Float curves and `Notifies` are in the sidecar only.
- 260 sequences have a `TrackToSkeletonMapTable` property whose length differs from the
  compressed table; the converter uses the compressed table, which is the one the offsets index.

## Validation

`node tools/anim_validate.js --sample 0` (every file; default is a seeded sample of 2,000):

- glTF-Validator: 51,487 files, 0 errors, 0 warnings. Infos only: `NODE_EMPTY` (bones carry no
  mesh) and `UNUSED_OBJECT` (the raw stream view, 51,446 files — the 41 empty streams have none).
- Independent read of each `.glb`: 12,627,228 channels, 386,669,015 keys; every rotation key
  within 5.3e-8 of unit length; no non-finite value; every sampler's times start at 0, increase
  strictly and end at or before `SequenceLength`; no sampler has more keys than `NumFrames`.

## Related classes

Surveyed by `tools/anim_convert.js index` (223,954 packages under `assets/`):

| Class | Exports | Bytes | Native data after the properties |
|---|---|---|---|
| `AnimMontage` | 8 | 33,675 | object GUID flag + `SkeletonGuid` (20 bytes) |
| `BlendSpace` | 150 | 1,111,657 | same |
| `BlendSpace1D` | 889 | 1,710,122 | same |
| `AimOffsetBlendSpace` | 19 | 119,762 | same |
| `AimOffsetBlendSpace1D` | 1 | 1,664 | same |
| `AnimComposite`, `PoseAsset` | 0 | — | — |

All 1,067 are parsed to the last byte and re-serialised byte-exactly (rows in
`audit/anim_export.json`). They hold no key data, so nothing is converted to glTF.

## Not done

- `Skeleton` exports (1,303): only the reference skeleton at the start of the native data is
  read (bone names, parents, reference pose, name map — the same three arrays as in
  `skeletalmesh.md`). The bytes after it (retarget sources, GUID, smart names) are not parsed.
- Decoding of formats 0, 4, 5, 6 inside per-track blocks and of rotation format 3 is not
  implemented or only transcribed from the exe: no file contains them, so nothing could be
  checked. `anim.js` stops with an error if it meets a format it cannot decode.
- `Notifies` reference notify objects stored as separate exports (`TresAnimNotify_*` and
  others); those exports are outside this document.
