# `.usm` movie container (CRI Sofdec2, as used by KINGDOM HEARTS III, Steam)

Reader/writer: `tools/lib/usm.js`. Proof: `node tools/usm_verify.js` -> `audit/usm_manifest.json`
(every number below is taken from its `totals` block or from the per-file rows).
Extraction: `node tools/usm_extract.js <file or prefix> [--verify]`.

Scope: the 180 files under `KINGDOM HEARTS III/Content/CriMovie/` (33,753,351,552 bytes,
1,748,361 chunks). All integers are **big-endian**.

Confidence wording used here:

- **confirmed** - parsed on all 180 files and reproduced by the byte-exact rebuild
  (180/180 files `roundtrip_matches`, 0 chunks needed a raw-bytes fallback), or checked against
  the data by a named cross-check in `totals.checks` with 0 failures.
- **inferred** - the bytes round-trip, but the *meaning* given here rests on the field name, on
  a single constant value, or on general knowledge of the format, not on a check in this project.

## 1. Round-trip result

| | |
|---|---|
| Files | 180 |
| Rebuilt container SHA-256 equals original | 180 |
| Mismatches / parse errors | 0 / 0 |
| Bytes outside any chunk (`trailing_bytes`) | 0 in every file |
| Files described purely structurally | 180 (`representation_exceptions` all 0) |

"Rebuilt" means: chunk headers re-serialised from fields (`chunkSize` is derived, not copied),
`@UTF` tables re-serialised from their rows, marker strings re-emitted from text, padding
regenerated as zeros, video payloads re-masked from the unmasked elementary stream. The original
hash is computed from the raw file reads, independently of the parser.
`tools/usm_extract.js --verify` additionally rebuilds from the files written to disk.

## 2. File layout

A file is a sequence of chunks and nothing else. Every chunk starts at a multiple of 0x20 and
has a total size that is a multiple of 0x20 (confirmed: `all_chunks_0x20_aligned` 180/180).

Order (confirmed, `leading_chunk_orders`; S = each stream signature present, in the order
`@SFV`, then `@SFA` or `@ALP`):

```
CRID header
S header ...            one per stream
S "#HEADER END" ...     one per stream
S metadata ...          one per stream
S "#METADATA END" ...   one per stream
stream chunks of all streams, interleaved by time
S "#CONTENTS END" ...   one per stream
```

No header or metadata chunk follows the first stream chunk (confirmed, 180/180). The first stream
chunk is at offset 2880 to 19520 depending on the size of the seek table.

Stream combinations seen:

| Streams | Files |
|---|---|
| `@SFV` H.264 + `@SFA` HCA | 110 |
| `@SFV` H.264 | 36 |
| `@SFV` MPEG-1 | 19 |
| `@SFV` MPEG-1 + `@ALP` MPEG-1 | 15 |

Chunk counts: `CRID` 180 header; `@SFV` 180 header, 180 metadata, 540 section end, 695,490 stream;
`@SFA` 110 / 110 / 330 / 1,043,636; `@ALP` 15 / 15 / 45 / 7,530.
**Not present in any file: `@SBT` (subtitles), `@CUE`, `@AHX`, or any other signature**, and no
`chno` other than 0. Their layout is therefore not documented here.

## 3. Chunk header (0x20 bytes)

| Off | Size | Field | Values in these files | Confidence |
|---|---|---|---|---|
| 0x00 | 4 | signature | `CRID`, `@SFV`, `@SFA`, `@ALP` | confirmed |
| 0x04 | 4 | `chunkSize`: bytes after this field | = `dataOffset` + payload + `paddingSize` | confirmed |
| 0x08 | 1 | reserved | 0 | confirmed 0; meaning unknown |
| 0x09 | 1 | `dataOffset`: payload starts at 8 + this | 0x18 in all 1,748,361 chunks | confirmed value; that it is an offset (and one byte wide rather than a 16-bit field with 0x08) is inferred |
| 0x0A | 2 | `paddingSize`: bytes after the payload | 0 to 1768 | confirmed |
| 0x0C | 1 | `chno` channel number | 0 | confirmed value; meaning inferred |
| 0x0D | 2 | reserved | 0 | confirmed 0 |
| 0x0F | 1 | `dataType` | 0 stream, 1 header, 2 section end, 3 metadata | confirmed |
| 0x10 | 4 | `frameTime` | see 3.1 | confirmed |
| 0x14 | 4 | `frameRate` (time base) | 30 on all 1,705 non-stream chunks; 3000 on `@SFV`/`@ALP` stream chunks; 2997 on `@SFA` stream chunks | confirmed values; meaning inferred |
| 0x18 | 8 | reserved | 0 | confirmed 0 |

Padding is always zero bytes (confirmed: 0 `raw_padding` exceptions).

- Stream chunks carry the minimum padding that brings the chunk to a multiple of 0x20
  (confirmed, 180/180 files).
- Other chunks are padded to fixed or table-dependent sizes: `CRID` header chunk 2048 bytes
  total (180/180); `@SFV` and `@ALP` header chunks 512; `@SFA` header chunk 256; section-end
  chunks 64; `@SFA` metadata chunk 256; `@SFV` metadata chunk 192 to 16,192 and `@ALP` 320 to
  1,152, growing with the seek table. The rule that produces these sizes is not determined; the
  tools store `paddingSize` per chunk.

### 3.1 Time stamps

`frameTime` is 0 on every header, metadata and section-end chunk.

- Video and alpha: chunk *i* (0-based) has `frameTime = floor(i * 2997 * framerate_d / framerate_n)`
  (confirmed on every chunk of all 195 video/alpha streams). Only one frame rate occurs
  (`framerate_n` 30000, `framerate_d` 1000), so the general form of the formula is inferred.
- Audio: chunk 0 (the HCA header) has time 0; chunk *j* >= 1 holds HCA block *j*-1 and has
  `frameTime = round((j-1) * 1024 * 2997 / sampling_rate)` (confirmed on all 110 streams; only
  48000 Hz occurs, and the 1024 samples per block is inferred from this fit).

## 4. Payload by `dataType`

| Type | Payload | Confidence |
|---|---|---|
| 1 header | one `@UTF` table (section 5) | confirmed |
| 3 metadata | one `@UTF` table | confirmed |
| 2 section end | 32 bytes: ASCII text + NUL. Exactly three per stream, in this order: `#HEADER END     ===============`, `#METADATA END   ===============`, `#CONTENTS END   ===============` | confirmed (305/305 streams) |
| 0 stream | one unit of the elementary stream (section 7) | confirmed |

## 5. `@UTF` table

Used for all header and metadata payloads. Every table in these files re-serialises
byte-identically from its parsed rows (0 `raw_payload` exceptions).

| Off | Size | Field | Confidence |
|---|---|---|---|
| 0 | 4 | `@UTF` | confirmed |
| 4 | 4 | `tableSize`: bytes after this field | confirmed |
| 8 | 2 | reserved, 0 | confirmed 0 |
| 10 | 2 | `rowsOffset` | confirmed |
| 12 | 4 | `stringsOffset` | confirmed |
| 16 | 4 | `dataOffset` (= `tableSize` when the table has no data pool) | confirmed |
| 20 | 4 | `nameOffset` of the table name in the string pool | confirmed |
| 24 | 2 | column count | confirmed |
| 26 | 2 | row length in bytes | confirmed |
| 28 | 4 | row count | confirmed |
| 32 | | column schema, rows, string pool, data pool, zero padding | confirmed |

The four offsets are relative to byte 8.

Column schema entry: `u8 flags`, `u32 nameOffset`, then a value if the storage is "constant".
`flags >> 4` is the storage, `flags & 15` the type.

| Storage | Meaning | Seen |
|---|---|---|
| 1 | no value stored (reads as zero / empty) | not checked; handled by the parser, meaning inferred |
| 3 | one constant value stored in the schema | yes |
| 5 | value stored in every row | yes |

| Type | Size | Seen in these files |
|---|---|---|
| 0 u8, 1 s8 | 1 | u8 |
| 2 u16, 3 s16 | 2 | both |
| 4 u32, 5 s32 | 4 | both |
| 6 u64, 7 s64 | 8 | u64 |
| 8 float, 9 double | 4, 8 | no (handled, unexercised: inferred) |
| 0xA string | 4: offset into the string pool | yes |
| 0xB data | 8: offset into the data pool, length | yes (`AUDIO_HEADER.hca_header` only) |

Which type code each column uses is in the manifest (`metadata_table_columns`) and in the
`container.json` written by the extractor.

Serialisation rules that reproduce every table (confirmed by the rebuild):

- String pool: `<NULL>` at offset 0, then the table name, then the column names in schema order
  (a constant string value directly after its column name), then row strings in row order.
- A string already in the pool is not stored twice; the earlier offset is reused. (Seen where
  the `@SFV` and `@ALP` rows of `CRID` name the same source file.)
- Data pool: starts at the next multiple of 8 after the string pool. Only one table kind has a
  data pool, so this alignment rule is inferred from that one case.
- The table is zero-padded to a multiple of 8 bytes.

## 6. Tables

Semantics below are from the column names unless a check is cited.

### 6.1 `CRIUSF_DIR_STREAM` (payload of the `CRID` chunk)

Row 0 describes the file, each further row one stream. 2 rows in 55 files, 3 rows in 125.

| Column | Type | Observed | Confidence |
|---|---|---|---|
| `fmtver` | u32 | 0 | value confirmed |
| `filename` | string | row 0: `*.usm`. Streams: source file name given to the encoder (`*.264_med` 146, `*.avi`, `*.tga`, `*.wav` 110) | inferred |
| `filesize` | u32 | row 0: size of the `.usm` file (180/180). Stream rows: byte length of that elementary stream (305/305) | confirmed |
| `datasize` | u32 | 0 | value confirmed |
| `stmid` | u32 | row 0: 0. Streams: the chunk signature as a number (`@SFV` = 1079199318, `@SFA` = 1079199297, `@ALP` = 1078021200) | confirmed (every stream has its row) |
| `chno` | u16 | row 0: 65535. Streams: 0 | confirmed |
| `minchk` | u16 | row 0: 1; `@SFV`, `@ALP`: 3; `@SFA`: 1 | value confirmed, meaning unknown |
| `minbuf` | u32 | varies; `@SFA`: 27860 in all 110 | meaning inferred (minimum buffer size) |
| `avbps` | u32 | varies | meaning inferred (average bit rate) |

### 6.2 `VIDEO_HDRINFO` (header of `@SFV` and `@ALP`)

One row. All columns are present in all 195 tables.

| Column | Observed | Confidence |
|---|---|---|
| `width`, `height` | coded size: 1920x1088 (135), 1920x1080 (15 `@SFV` + 15 `@ALP`), 512x256 (14), 1280x768 (9), 768x512 (5), 1280x720 (1), 800x208 (1) | equals the MPEG sequence header size in all 49 MPEG streams (confirmed); for H.264 inferred |
| `mat_width`, `mat_height` | equal to `width`, `height` in every file | meaning unknown |
| `disp_width`, `disp_height` | display size; differs from the coded size only as 1088 -> 1080 and 208 -> 200 | inferred |
| `scrn_width` | 0 | unknown |
| `mpeg_dcprec` | 11 | unknown |
| `mpeg_codec` | 1 on all 49 streams whose bitstream is MPEG-1 video, 5 on all 146 whose bitstream is H.264 | confirmed against the bitstreams |
| `alpha_type` | 0 on all `@SFV`, 1 on all `@ALP` | confirmed correlation |
| `total_frames` | number of stream chunks of that stream (53 to 40,050) | confirmed (195/195) |
| `framerate_n`, `framerate_d` | 30000, 1000 | inferred (see 3.1) |
| `metadata_count` | 1 = number of metadata chunks of the stream | confirmed (305/305 incl. audio) |
| `metadata_size` | total size in bytes of that metadata chunk including its header | confirmed (305/305 incl. audio) |
| `ixsize` | 50,944 to 532,320 for `@SFV`. In all 195 video/alpha streams it exceeds the largest payload of the stream by less than 4096 (computed from the manifest rows `header.ixsize` and `payload_size_max`) | meaning unknown |
| `pre_padding`, `max_picture_size`, `color_space`, `picture_type` | 0 | unknown |

### 6.3 `VIDEO_SEEKINFO` (metadata of `@SFV` and `@ALP`)

One row per seek point.

| Column | Type | Meaning | Confidence |
|---|---|---|---|
| `ofs_byte` | u64, per row | absolute file offset of a stream chunk of this stream | confirmed (195/195) |
| `ofs_frmid` | s32, per row | index of that chunk within the stream | confirmed (same check) |
| `num_skip` | s16, constant | 0 | unknown |
| `resv` | s16, constant | 0 | unknown |

The row count equals the number of SPS NAL units (146/146 H.264 streams) or of MPEG sequence
headers (49/49 MPEG streams): one row per random-access point.

### 6.4 `AUDIO_HDRINFO` (header of `@SFA`)

| Column | Observed | Confidence |
|---|---|---|
| `audio_codec` | 4 on all 110 streams, all of which are HCA | confirmed correlation |
| `sampling_rate` | 48000; equals the HCA header | confirmed (110/110) |
| `num_channels` | 6 (88 files), 2 (22 files); equals the HCA header | confirmed (110/110) |
| `total_samples` | 1,116,800 to 64,080,000 | inferred |
| `metadata_count`, `metadata_size` | 1, 256 | confirmed as in 6.2 |
| `ixsize` | 27860 (108 files), 42624, 52192 | unknown |
| `ambisonics` | 0 | unknown |

### 6.5 `AUDIO_HEADER` (metadata of `@SFA`)

One row, one column `hca_header` of type data. It has the length of the stream's HCA header
(96 bytes; confirmed 110/110). That it is a copy of that header is inferred: the contents were
compared by eye on one file, not by the tool.

## 7. Stream payloads and masking

### 7.1 Video (`@SFV`) and alpha (`@ALP`): masked

**The stored video and alpha payloads are not plain codec bitstreams.** They are obfuscated with
the CRI video mask, and are plain after it is removed. Evidence, all from `totals.mask_evidence`:

| Test | Stored bytes | After unmasking |
|---|---|---|
| H.264 (146 streams, 29,869,163,219 bytes): byte sequences forbidden inside Annex B NAL units | 6,427 violations (7,093.6 expected for random data) | 0 |
| H.264: chunks containing no slice NAL | 22,623 | 0; every chunk of every stream has exactly 4 slice NALs, and only NAL types 1, 5, 6, 7, 8, 9 occur |
| MPEG-1 (49 streams, 25,910 chunks): pictures carrying a slice start code for every macroblock row | 0 of 23,782 pictures found | 25,910 of 25,910, with 0 slice order errors |

Verdict per stream in the manifest: `masked: true` for all 180 `@SFV` and all 15 `@ALP`.

The first 0x40 bytes of a payload are stored plain, which is why the stored streams still begin
with recognisable start codes.

Algorithm (confirmed by the table above and by the byte-exact re-masking in the round trip).
`m2` is a 32-byte table, `m1[i] = m2[i] ^ 0xFF`, `p` plain, `c` stored, offsets relative to the
start of the chunk payload:

```
if payload length < 0x240: stored as is
bytes 0x000..0x03F: plain
bytes 0x140..end:   k = copy of m2
                    for i = 0x140 .. end-1:  c[i] = p[i] ^ k[i & 31];  k[i & 31] = p[i] ^ m2[i & 31]
bytes 0x040..0x13F: k = copy of m1
                    for i = 0 .. 0xFF:       k[i & 31] ^= p[0x140 + i];  c[0x40 + i] = p[0x40 + i] ^ k[i & 31]
```

- Threshold: 3,483 video chunks are below 0x240 bytes and are stored plain. The largest chunk
  left as stored is 575 bytes and the smallest unmasked one is 578 bytes, so the data pins the
  threshold to 576..578; the exact value 0x240 (576) is inferred.
- One table serves all 195 streams.
- **The table is not stored in this repository.** `findVideoMask` in `tools/lib/usm.js` recovers
  it from the movies at run time: for payload offsets >= 0x160 the stored byte is
  `p[i] ^ p[i-32] ^ m2[i & 31]`, and in MPEG video `p[i] ^ p[i-32]` is most often 0, so the most
  frequent stored byte at each offset mod 32 is `m2`. This run used the 4 smallest files
  (4,382,854 payload bytes; weakest peak 1.117 times the runner-up). 24 of the 32 bytes are
  functions of the other 8 (`maskRelationsHold`), which the recovered table satisfies. Only its
  SHA-256 is recorded: `0b84873af71d0b15139210e6a76fa1513837aca9c14eff2221ac88cd65d4f0f2`.
- Not determined: the 64-bit key the game supplies, and how the table derives from it. The
  relations above constrain 24 bytes; the remaining 8 are simply the recovered values.

Because masking and unmasking are exact inverses for any table, the round-trip proof does not
depend on the table being correct; the bitstream tests above are what show that it is.

Each stream chunk holds exactly one picture / access unit:

- MPEG-1 video (no MPEG-2 extension start codes): pictures = chunks in all 49 streams; frame
  rate code 5, aspect code 1; sizes 1920x1080 (30 streams), 512x256 (14), 768x512 (5).
- H.264 Annex B: every chunk starts with a start code; High profile (`profile_idc` 100), level
  4.1 (135 streams), 3.2 (9), 3.1 (1), 2.1 (1).
- `@ALP` is a second MPEG-1 video stream with the same dimensions and chunk structure as `@SFV`.
  That it carries the alpha plane is inferred from the signature and `alpha_type` = 1.

### 7.2 Audio (`@SFA`): not masked

All 110 audio streams are HCA, stored plain:

- Chunk 0 is the 96-byte HCA header; each later chunk is exactly one HCA block (3072 bytes for
  the 88 six-channel streams, 1024 bytes for the 22 stereo streams) (confirmed 110/110).
- Header: version 0x200, sub-chunks `fmt`, `comp`, `ciph`, `pad`; tags are not high-bit
  masked (0 of 110); header CRC-16 valid in 110/110.
- `ciph` type is **0 (no cipher)** in all 110.
- All 1,043,526 blocks start with `FF FF` and have a valid CRC-16 (polynomial 0x8005, CRC of the
  whole block is 0). A container-level mask over the payload would break these.

No ADX audio occurs. The HCA audio was not decoded here, so "decodes correctly" is not claimed;
the evidence is the cipher field and the block checksums.

## 8. Extracted form (`tools/usm_extract.js`)

`assets/<path>.usm.d/`:

| File | Content |
|---|---|
| `video_0.m1v` / `video_0.h264` | `@SFV` elementary stream, unmasked |
| `alpha_0.m1v` | `@ALP` elementary stream, unmasked |
| `audio_0.hca` | `@SFA` elementary stream as stored (a complete `.hca` file) |
| `<kind>_0.json` | that stream's header and metadata tables, readable |
| `container.json` | `info` (source hash, stream files and their hashes) and `chunks`: one descriptor per chunk in file order (signature, chno, type, paddingSize, frameTime, frameRate, and either the `@UTF` table, the marker text, or `payloadSize`) |

`--verify` rebuilds the `.usm` from these files and compares SHA-256 with the original.
`--raw` writes video as stored (masked). Nothing is extracted by default: the elementary streams
total 33,685,662,225 bytes.

## 9. Not determined

- The mask key and the key-to-table derivation (7.1).
- Meaning of `mat_*`, `scrn_width`, `mpeg_dcprec`, `ixsize`, `pre_padding`, `max_picture_size`,
  `color_space`, `picture_type`, `num_skip`, `resv`, `minchk`, `ambisonics`, `fmtver`, `datasize`.
- The rule for the padding of header and metadata chunks.
- Anything about `@SBT`, `@CUE`, ADX audio, VP9 video, multiple channels (`chno` > 0) or a
  `dataOffset` other than 0x18: none occur in this game's files.
- Whether the streams decode: no video or audio decoder was run. The checks are structural.
