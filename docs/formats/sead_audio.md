# SEAD audio assets (`SQEXSEADSound`, `SQEXSEADMusic`, `SQEXSEADSoundBank`)

Reader/writer of the cooked exports: `tools/lib/sead.js`; converter and proof: `tools/sead_convert.js`.
Bank contents: `tools/lib/sead_bank.js` (sounds, sequences, tracks), `tools/lib/sead_material.js`
and `tools/sead_decode.js` (audio streams to WAV), `tools/lib/sead_config.js` (the configuration).
How the game plays these sounds for the player character: `docs/sora-audio-spec.md`.
Evidence: `audit/sead_manifest.json` (one row per export, in-memory round trip),
`audit/sead_summary.json` (the counts quoted here), `audit/sead_export.json` and
`audit/sead_export_summary.json` (files written to `assets_converted/audio/`, round trip repeated
from disk). Integers are little-endian unless stated.

The game does not use Unreal's `USoundWave` for its audio (2 exports of that class exist). Sound
is played by Square Enix's SEAD middleware, and the audio lives in SEAD bank files embedded in
the export data of these three classes. None of these packages has a `.ubulk`.

Confidence: **confirmed** = read on every export in scope and rebuilt byte-identically;
**inferred** = bytes round-trip but the meaning is a reading of the pattern (or of publicly
known SEAD/HCA layouts), not something established from the game's code.

## Scope

| Class | Exports | With an embedded bank | Round trip |
|---|---|---|---|
| `SQEXSEADSound` | 39,875 | 28,371 | 39,875 |
| `SQEXSEADMusic` | 369 | 369 | 369 |
| `SQEXSEADSoundBank` | 804 | 804 | 804 |
| total | 41,048 | 29,544 | 41,048 |

Packages that contain these exports contain nothing else, so the whole `.uexp` (export bytes +
the 4-byte package tag) is reproduced for every one of them.

The 11,504 `SQEXSEADSound` exports without a bank carry a `ReferenceBank` object property
(11,504) and a `SoundIndex` (11,435): they point at a sound inside a `SQEXSEADSoundBank` asset.

## Export layout

```
tagged properties ... None
i32   object GUID flag                 0 on all 41,048
--- SQEXSEADSound and SQEXSEADMusic only ---
i32   (1 on all 40,244)                inferred: bCooked
i32   (0 on all 40,244)                unknown
u8[16] GUID
i32   bank present                     0 or 1
--- bank record: always for SQEXSEADSoundBank, else when "bank present" is 1 ---
i32   record version                   3, 2 or 0 (name is this project's; see below)
i32   bank size in bytes
u8[N] zero                             N = 20 for versions 2 and 3, 16 for version 0
u32   BulkDataFlags                    0 on all 29,544
i32   ElementCount                     = bank size
i32   SizeOnDisk                       = bank size
i64   OffsetInFile                     offset of the bank in the virtual file <.uasset><.uexp>
u8[]  the bank file
```

Nothing follows the bank (0 trailing bytes on all exports).

| Field | Confidence | Notes |
|---|---|---|
| tagged properties | confirmed | `SQEXSEADSound`: `Duration`, `SoundClassObject`, `Volume`, `AttenuationSettings`, `ReferenceBank`, `SoundIndex`, ... (counts in the summary under `property:<class>`). `SQEXSEADMusic`: `Duration`, `Volume`. `SQEXSEADSoundBank`: `AssetImportData` (null). |
| preamble (two i32 + GUID) | confirmed values, meaning inferred/unknown | Same shape as the cooked flag + GUID of an Unreal sound asset. |
| record version | confirmed values, meaning unknown | 3: 28,909 banks (all `sabf`). 2: 632 banks (383 `mabf`, 249 `sabf`). 0: 3 banks (`sabf`, under `Sound/SE/world/ra_e3/`). It is *not* the bank type. The only structural effect seen is the length of the zero block. |
| zero block | confirmed | All zero in every record. Purpose unknown. |
| bulk header | confirmed | Last four fields have the shape of an Unreal `FByteBulkData` header stored inline; the offset rule holds on all 29,544. |

## The embedded bank

Two kinds, by magic: `sabf` (sound bank, written as `.sab`): 29,161 files, 4,412,920,992 bytes;
`mabf` (music bank, `.mab`): 383 files, 2,351,931,472 bytes. All 369 `SQEXSEADMusic` assets hold
a `mabf`; so do 14 `SQEXSEADSound` assets.

The converter treats the bank as opaque; nothing below is needed for the round trip. Counts in
the header table are over the 29,544 embedded banks of the manifest; the sections after it were
read from the 29,542 bank files on disk under `assets_converted/audio/` (29,159 `.sab`, 383 `.mab`).

### Header and section table (confirmed on all 29,544)

| Offset | Size | Field | Observed |
|---|---|---|---|
| 0 | 4 | magic | `sabf` / `mabf` |
| 4 | 1 | version | 2 |
| 5 | 1 | ? | 1 on 28,740 banks, 0 on 804 (the number of `SQEXSEADSoundBank` assets; that it is exactly those was not checked) |
| 6 | 2 | header size | 16 |
| 8 | 1 | section count | 4 (`sabf`), 3 (`mabf`) |
| 9 | 1 | name length | 2 to 26 |
| 10 | 2 | ? | varies (not interpreted) |
| 12 | 4 | file size | equals the embedded length on all |
| 16 | n+1 | name, NUL-terminated, padded so the table starts on a 16-byte boundary | |

Section table, 16 bytes per entry: 4-char tag, u32 `0x00100000` (constant), u32 offset from the
start of the bank, u32 0. The first section starts where the table ends, in every bank.

| Bank | Sections, in order |
|---|---|
| `sabf` | `snd `, `seq `, `trk `, `mtrl` |
| `mabf` | `musc`, `inst`, `mtrl` |

`musc` and `inst` (music and instrument definitions of the music banks, going by the tags) were
not parsed.

### `snd `, `seq `, `trk `: sounds, sequences, tracks

Parser: `tools/lib/sead_bank.js` (`parseBank`, `resolveSound`). Run over all 29,159 `.sab` on
disk: 0 parse failures, no non-zero byte left unparsed inside an entry; 46,931 sounds, 55,021
sequences, 55,248 tracks, 118,776 sequence commands, 305,194 track commands. The layouts were
read from the driver (`SQEX::Sd`, about `0x145940000`–`0x1459C0000`, RTTI names present).
Marks: **[I]** instruction level, **[C]** decompiled C only, **[A]** statistics over all banks.
Field-by-field tables with addresses: `audit/sora_port/audio/bank_findings.json`.

Every section starts with u8 version (0), u8 0, u16 header size (16), u16 count, and holds
`count` u32 offsets (relative to the section) at `+header size`. File header byte 5 bit 0 marks
a single-sound bank (28,357); u16 at +0x0A is the bank number.

**Sound** (0x60 bytes; then the name, a switch block on Switch sounds, and one 16-byte sequence
info per sequence: u8 1, u8 0, u16 16, u16 sequence index, u16 sequence number, 8 zero) [C]:

| Offset | Size | Field |
|---|---|---|
| 0x00 | 1 | version (17: 46,650 · 16: 267 · 15: 11 · 9: 3) |
| 0x01 | 1 | runtime cursor of Order/Random sounds, written into the loaded bank (0xFF in the file) |
| 0x02 | 2 | header size = name offset |
| 0x04 | 1 | type: 0 empty slot 6,696 · 1 Sound 36,746 · 2 Random 1,231 · 3 Order 1,935 · 4 Cycle 294 · 5 Switch 29 |
| 0x05 | 1 | sequence count |
| 0x06 | 1 | category number (configuration `ctgr`) |
| 0x07 | 1 | priority, higher wins (128 on 39,897) |
| 0x08 | 2 | sound number |
| 0x0A / 0x0B | 1 + 1 | start macro / stop macro (configuration `mcr `) |
| 0x0C | 4 | f32 volume |
| 0x10 / 0x14 / 0x18 | 4 + 4 + 1 | Cycle: interval min / max in ms, max active groups |
| 0x19 | 1 | flags: 0x01 no 3D pan, 0x02 no 3D volume, 0x08 centre channel only (2D), 0x20 Random may repeat, 0x80 stop breaks loops; 0x04 has no reader found |
| 0x1A | 2 | sequence-info offset |
| 0x1C | 4 | f32, 30.0 on every sound [A] (read by the UE plugin as the bank's own max distance, used only with `bUseKohrogiAttenuation`, which no asset sets) |
| 0x20 | 1 | main bus (0 Master 26,662 · 1 3DSE 10,873 · 5 Voice 7,276 · 3 HeliumVoice 2,119) |
| 0x21 | 1 | distance curve number handed to the 3D layer by the UE plugin |
| 0x23 | 1 | name length |
| 0x24 | 4 | f32 duration in seconds [A] |
| 0x28 | 4 | f32 interior pan factor (non-zero on 46 sounds) |
| 0x30 / 0x34 / 0x44 | 4×1 + 4×4 + 1 | aux send bus and f32 volume per slot, slot count (slot 0 → bus 2 "Revibe" on 36,524) |
| 0x4F | 1 | macro for event 3 (107 on 127 voice sounds) |
| 0x50 / 0x54 | 1 + 4 | pan type, f32 pan parameter |
| 0x51 | 1 | flags2: bit 0 = use the sound's own release time |
| 0x58 | 4 | f32 release seconds |

**Sequence** (0x20 header, zero-one offset table, zero-one curves, commands) [C]: 0x00 version
(4), 0x02 u16 header size, 0x04 u16 number, 0x06 u16 command offset, 0x08 u8 zero-one count,
0x10 u8 weight (Random/Cycle; 0 = always plays) or fixed flag (Order), 0x11 u8 group; on Switch
sounds f32 min at 0x10 and f32 max at 0x14.

**Zero-one curve** [C]: 0x00 version (5), 0x01 name length, 0x02 u16 header size, 0x04 u16 point
count, 0x06 target (1 volume factor, 2 pitch factor, 3 track parameter, 5 low-pass factor,
6 aux send in dB), 0x07 target index, 0x08 / 0x0C f32 input min / max, 0x10 u16 parameter id,
0x14 flags; then the name and the points {f32 x, f32 y, u8 curve, 7 zero}. The input is chosen
by name: `zeroone_slot0/1`, `zeroone_distance`, `zeroone_distance_rate`, otherwise an external
parameter of the configuration. Curves (also used by every fade): 0 `t`, 1 `1-(1-t²)³`,
2 `1-(1-t)²`, 3 `t²`, 4 `2^t-1`, 5 `2-2^(1-t)` (`0x14596A9C0`).

**Track** (0x10 header + commands) [C]: 0x00 version, 0x01 type (1 material track 55,127 ·
2 child-sound track 121), 0x04 u16 material index (or index of a child sound of the same bank),
0x06 u16 bank number, 0x0E u16 command offset.

**Commands** (both streams): u8 version, u8 header size (4), u8 opcode, u8 body size.

| Sequence opcode (`0x145982A40`) | Operands, effect | Count |
|---|---|---|
| 1 end | | 55,021 |
| 2 keyOn | u32 track, u8 loop flag, f32 duration s; releases the previous track | 55,248 |
| 3 wait | u32 ms | 6,002 |
| 4 keyOff | | 261 |
| 5 waitRandom | u32 max ms, u32 min ms: `min + rand() % (max-min+1)` [I] | 2,222 |
| 6 loopStart / 7 loopEnd | u16 max, u16 min repeat count; 0/0 endless | 11 / 11 |
| 8 waitZeroOne | not used by any bank | 0 |

| Track opcode (`0x1459B0450`) | Operands, effect |
|---|---|
| 2 wait | u32 ms |
| 3 volume, 4 pitch, 5 pan, 6 panDepth | f32 target, u32 ms, u8 curve; pitch is a frequency ratio, pan an angle in units of π |
| 7–10 random volume / pitch / pan / panDepth | f32 max, f32 min, u8 mode (1 add, 2 multiply): `min + (rand() % 1000) × 0.001 × (max − min)` |
| 11–14 LFO volume / pitch / pan / panDepth | f32 period s, f32 depth, u8 wave (1 sine, 2 square, 3 triangle, 4 saw, 5 noise, 6–9 inverted) |
| 15 lfeSend, 16 centerMix | f32 target, u32 ms, u8 curve |
| 17 panPath | 4 points, u32 ms, u8 mode |

What the driver does with these when a sound is started (selection, volume and pitch
composition, pools, macros, ending) is specified in `docs/sora-audio-spec.md`, section 2.

### The SEAD configuration (`scnf`)

Categories, category layers, buses, macros, events, distance curves and external parameters are
not in any bank or asset: they are the value of `ConfigString` in the section `[SQEXSEAD]` of
`Config/Windows/WindowsEngine.ini`, three decimal digits per byte, 44,768 bytes, magic `scnf`
(read by the plugin at `0x145814BC0`, handed to the driver's init `0x14594BA20`). Sections:
`ctgr` (106 category slots, 28 defined), `ctgl` (7 layers), `mcr ` (112 macros), `bus ` (6),
`expr` (11 external parameters), `curv`, `zeon`, `evt ` (24 events), `eps `, `epsp`, `hcam`,
`spka`, `pext`. Parser: `tools/lib/sead_config.js` (categories, layers, macros, buses, external
parameters; the other sections are kept raw); `node tools/lib/sead_config.js` writes
`build/gen/audio/sead_config.json`.

### `mtrl`: the audio streams

At the section offset: u32, **u32 entry count**, 8 bytes, then one u32 offset per entry
(relative to the section start). 56,684 entries in the 29,542 banks on disk. Each entry:

| Offset | Size | Field | Observed / use |
|---|---|---|---|
| 0 | 1 | version | 1 on all but the 9 MS ADPCM entries (0) |
| 2 | 2 | entry header size | 32 |
| 4 | 1 | channels | 1: 43,445 · 2: 5,675 · 4: 583 · 6: 2,777 |
| 5 | 1 | codec | 7 HCA: 52,161 · 0 empty slot: 4,204 · 3 Ogg Vorbis: 310 · 2 MS ADPCM: 9 |
| 6 | 2 | number | index of the material in the bank |
| 8 | 4 | sample rate | 48000: 52,458 · 44100: 22 |
| 12 | 4 | loop start (samples) | the driver treats the stream as looped when start < end (0x145986E00) |
| 16 | 4 | loop end (samples, the last looped sample) | non-zero on 1,793 |
| 20 | 4 | extra data size | codec header, see below |
| 24 | 4 | stream size | |
| 28 | 1 | key seed | start index into the XOR table for obfuscated streams (see HCA) |
| 32 | | extra data, then the stream | |

The size fields are confirmed structurally (entries are back to back in every bank). The codec
ids are confirmed twice: by the data (each codec's own magic and checksums, below) and by the
driver's stream reader `0x145986E00`, which switches on this byte (1 = raw PCM, which no bank of
this game uses; 2, 3, 7 as here). Decoders: `tools/lib/sead_material.js` (dispatch),
`tools/sead_decode.js` (CLI). Every number in this section is from `audit/sead_decode.json`
(one row per material) and `audit/sead_decode_summary.json`, written by
`node tools/sead_decode.js verify`.

#### Codec 7: CRI HCA 2.0 (52,161 materials, 177,855 s of audio)

Extra data = a 16-byte SEAD block followed by a complete HCA header:

| Offset | Size | Field | Observed |
|---|---|---|---|
| 0 | 1 | version | 1 |
| 1 | 1 | size of this block | 16 |
| 2 | 2 | HCA header size | = `extra − 16`, = the header's own size field |
| 4 | 2 | HCA frame size | = `comp` frame size |
| 6 | 2 | loop start frame | = HCA `loop` chunk |
| 8 | 2 | loop end frame | = HCA `loop` chunk |
| 10 | 2 | inserted samples | = HCA encoder delay |
| 12 | 1 | ? | 0 |
| 13 | 1 | obfuscated | 0: 40,593 · 1: 11,568 |
| 14 | 2 | ? | 0 |

The HCA header (big-endian, chunks `fmt`, `comp`, optional `loop`, `ciph`, `pad`; header CRC-16
valid on all) is plain in every bank: version 0x0200, `ciph` type 0, no `ath`/`vbr`/`rva`
chunk, minimum resolution 1, maximum 15, 128 total bands. Layouts in use:

| Channels | Frame bytes | Base bands | Stereo bands | HFR group | Materials |
|---|---|---|---|---|---|
| 1 | 256 | 96 | 0 | 4 | 41,394 |
| 1 | 341 | 128 | 0 | – | 1,332 |
| 1 | 512 | 128 | 0 | – | 596 |
| 2 (two tracks) | 512 | 96 | 0 | 4 | 2,707 |
| 2 (two tracks) | 682 | 128 | 0 | – | 1,476 |
| 2 (two tracks) | 1024 | 128 | 0 | – | 1,160 |
| 2 (intensity stereo) | 512 | 64 | 64 | – | 161 |
| 4 | 1024 / 1365 / 2048 | 96 / 128 / 128 | 0 | 4 / – / – | 263 / 253 / 42 |
| 6 | 1536 / 3072 / 2048 | 96 / 128 / 128 | 0 | 4 / – / – | 2,429 / 342 / 6 |

Stream = `frame count × frame size` bytes (true on all 52,161).

**Obfuscation (Square Enix, not CRI).** When byte 13 of the SEAD block is 1 the frame data is
XORed with a fixed 256-byte table: `plain[i] = table[(seed + i) & 255] ^ stored[i]`, `i` = byte
position in the stream, `seed` = byte 28 of the material header. Read in the decompiled C of
the driver's stream reader `0x145986E00` (HCA branch; the table is at `0x1487BA470`), and
confirmed on the data: after the XOR all frames of all 11,568 obfuscated materials start with
the sync word and pass their CRC. The HCA header itself is never obfuscated. Obfuscated banks
are most of `SE/common`, `SE/weapon`, `SE/enemy`, `SE/gimmick`, `SE/cutscene`, `SE/world`,
`SE/NPC`, `SE/gumi`, `SE/link`, the DLC SE and `LSI/Games/sound`; voices and music are plain.

**Decoder** (`tools/lib/hca.js`). The game links CRI's decoder ("HCA Mixer Ver.2.00.02"). The
decoder here takes every constant table from the exe (addresses in `EXE_TABLES`; cached in
`build/gen/audio/sead_codec_tables.json`; `node tools/sead_decode.js tables` re-reads and
compares) and follows the exe's functions in single precision:

| Step | Exe function | Table(s) |
|---|---|---|
| frame CRC-16 (poly 0x8005) | `0x1400457D0` | `0x147CA3C70` |
| resolution from scale factor and noise level | `0x140042B00` | `0x147CA18C8` (57 entries; index < 0 → 15, ≥ 57 → 1) |
| spectrum dequantisation (prefix codes for resolution < 8, sign-magnitude above) | `0x140042C50` | `0x147CA1BB0`, `0x147CA1BC0`, `0x147CA1DC0`, scale `0x147CA4070`, step `0x147CA4170` |
| high-frequency reconstruction | `0x1400441B0` | `0x147CA1950` (1.0 at index 63) |
| intensity stereo (right = left × (2 − ratio)) | `0x140043BA0` | `0x147CA1910` |
| DCT-IV: 7 butterfly stages, 7 rotation stages | `0x1400462C0`, `0x140045FC0` | sin `0x147CA73C0`…, cos `0x147CA6CC0`… (7 × 64 each) |
| window and overlap-add | `0x140045890` | `0x147CA3E70` (second half negative) |

These functions were read as decompiled C (their verdict in `sheets/impl/progress` is
`unconfirmed`), not instruction by instruction. The frame header and scale-factor unpacking
(sync 0xFFFF, 9-bit noise level, 7-bit evaluation boundary; per channel a 3-bit delta width,
6-bit scale factors or deltas with escape; 6-bit high-frequency scales, or 8 × 4-bit intensity
indices on a stereo-secondary channel) follow the publicly documented HCA bitstream and were not
re-read from the exe. Both are confirmed by the data instead (next paragraph). Output: 1024
samples per frame and channel, full scale ±1.0; the first `encoder delay` samples and the last
`encoder padding` samples are dropped (fmt chunk).

**Verification** (all 52,161 materials, 8,371,878 frames): every frame passes its CRC; no frame
fails to parse; in 51,089 materials at least one frame's coefficients end on the exact bit where
the frame's CRC begins and in 1,067 all frames end before it (a decoder that mis-parses drifts
and overruns at once). Five frames in the whole game spend more bits than they have (four by 2
bits, one by 17: an encoder quirk on full-band noise; the decoder reads the CRC bytes as data
like the game's reader and zero beyond; their rows carry a note). Decoded peaks cluster at full
scale (13,512 materials peak between 1.0 and 1.1, 6,627 between 0.9 and 1.0, none above 2),
which fixes the overall gain. There is no independent reference decoder in this project, so
equality with the game's output sample for sample rests on construction (same tables, same
operation order, single precision), not on a comparison with a recording: see "Not covered".

**Loop.** 1,774 materials carry an HCA `loop` chunk. The driver loops on the chunk, not on the
material header (`0x145986E00`: on reaching the end it re-reads the chunk through
`0x140042590`, seeks to `loop start frame × frame size`, and discards `start delay` samples).
In samples after delay removal: `start = startFrame × 1024 + startDelay − encoderDelay`,
`end (exclusive) = (endFrame + 1) × 1024 − endPadding − encoderDelay`. The material header
agrees: its loop start equals that start on 1,773; its loop end is the last looped sample
(`end − 1`) on 1,569 and `end` itself on 204; one DLC music material has loop start 0 in the
header against 300,195 in the chunk. On 1,766 the loop end is the end of the stream.

#### Codec 3: Ogg Vorbis (310 materials, 1,305 s)

Extra data: u16 version (2), u16 header size (32), u32 loop start byte offset, u32 loop end
byte offset (0/0 when not looped), u32 sample count of the source, u32 size of the three Vorbis
header packets' pages, u32 seek entry count, 8 zero bytes, then the seek table (one u32 stream
offset per 4,800 samples). The stream is a complete, unencrypted Ogg Vorbis file (one logical
stream, block sizes 256/2048): `SE/cutscene/dw` (208), `SE/gimmick/dw` (35), four footstep sets
(`10400_mickey`, `10500_woody`, `11000_npc_men`, `11100_npc_women`: 66) and one English voice
line. None of Sora's sounds use it.

`tools/lib/ogg.js` checks the container (page CRCs, sequence, BOS/EOS, the three header
packets): ok on all 310. The last granule equals the header's sample count on the 301 one-shot
streams; the 9 looped streams end at the loop end instead (`last granule = loop end + 1`).
PCM comes from libvorbis compiled to WebAssembly (`tools/third_party/ogg-vorbis-decoder`, a
vendored npm package, see its README; wrapper `tools/lib/vorbis.js`): all 310 decode without an
error and return exactly `last granule` samples. `sead_decode.js decode` writes both the `.ogg`
as stored and the `.wav`.

#### Codec 2: Microsoft ADPCM (9 materials, 389 s)

Extra data: u16 0, u16 header size (16), u16 block size, u16 0, u32 loop start, u32 loop end
(both rounded to whole blocks; the driver's ADPCM branch reads these). Standard MS ADPCM blocks
with the 7 standard coefficient pairs: block 22 bytes mono (8 materials), 44 bytes stereo (1),
32 samples per block. Only in `SE/world/ra_e3` (three ambience banks). Decoder:
`tools/lib/msadpcm.js`. Verification by self-consistency: the stream is a whole number of
blocks, every predictor index is ≤ 6, `blocks × 32` ends exactly at the loop end, and the
signal is continuous across block boundaries, where the decoder state is reloaded from the
block header (mean sample step at boundaries / inside blocks = 1.06 to 1.17 on the 9).

## Decoding to WAV

```
node tools/sead_decode.js info   <bank>                      materials of a bank
node tools/sead_decode.js decode <bank> [--material N] [--out DIR] [--float]
node tools/sead_decode.js verify [--prefix P] [--workers N]  decode everything, write the audit
node tools/sead_decode.js tables                             compare the table cache with the exe
```

`decode` writes one WAV per material: PCM s16 (`round(x × 32768)`, clamped; the count of clamped
samples is printed) or IEEE float32 with `--float` (the decoder's output unchanged), channels
and sample rate as stored, the loop as a `smpl` chunk (start, end inclusive). A bank path may be
given relative to `assets_converted/audio/`. The whole game decodes in about 3 minutes on 8
workers (`verify`: 29,542 banks, 56,684 materials, 0 failures).

## Container conversion

`node tools/sead_convert.js export` writes, per export:

- `assets_converted/audio/<package path>.sab` or `.mab`: the embedded bank, unmodified. Not
  written for assets that only reference a bank.
- `assets_converted/audio/<package path>.sead.json`: the sidecar: tagged properties, preamble,
  and the bank record's fields (41,048 files).

`rebuild(pkg, sidecar, bankFile)` returns the export's bytes from the sidecar, the bank file
and the package name table. `roundtrip_matches` is true when they equal the original export.

| Run | Exports | Matches | Fails |
|---|---|---|---|
| `verify` (`audit/sead_manifest.json`) | 41,048 | 41,048 | 0 |
| `export`, rebuilt from the files on disk (`audit/sead_export.json`) | 41,048 | 41,048 | 0 |

## Not covered

- A comparison of decoded samples with the running game or with another HCA decoder: there is
  none on this machine. The HCA decoder is tied to the game by its tables and code path and to
  the data by checksums and bit budgets, as described above; nobody has listened to the output
  in this project yet.
- The `musc` / `inst` sections of the music banks (segments, transitions, the music's own loop
  points). Their streams decode like any other material.
- Re-encoding: WAV back to HCA is not built (the container round trip does not need it).
- The configuration sections `curv`, `zeon`, `evt `, `eps `, `epsp`, `hcam`, `spka`, `pext` are
  kept as raw bytes by `tools/lib/sead_config.js`; events and distance curves were decoded by a
  research pass only (`audit/sora_port/audio/plugin_findings.json`).
- Other SEAD classes that carry no audio: `SQEXSEADLayoutSound` (657), `SQEXSEADAudioVolume`
  (762), `SQEXSEADSoundReferenceEnumSet` (195), the Matinee track classes, and the 2
  `SoundWave` / 3 `SoundCue` exports. The auto-SE assets and anim notifies are covered for the
  player character in `docs/sora-audio-spec.md`.
