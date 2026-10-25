# MercunaNavOctree (Mercuna 3D navigation)

Code: `tools/lib/pkg_classes/mercuna.js`. Evidence: `MercunaNavOctree` in
`audit/package_manifest.json` (every export consumed to its last byte and rebuilt identically)
and the value sets in `audit/package_facts.json`. The layout comes from the game's loader;
addresses are virtual addresses in `base/KH3.exe`, decompiled C in `build/decomp/`.

## Native data of the actor

| Field | Type | Evidence |
|---|---|---|
| unk_i32 | int32 | first four bytes; 1 in every export (the engine-side value written before the Mercuna stream) |
| header | see below | `func_145759AF0` |
| stream size | uint64 | `func_145751C90` reads 8 bytes after the header; when the header version is not 7 it skips that many bytes instead of parsing. In the data it always equals the number of bytes left in the export |
| node pool | see below | `func_145751C90` → `func_14576A1E0` / `func_14576A4C0` |
| five lists | see below | `func_145768860` |

### Header (`func_145759AF0`)

The function fills a settings block with defaults and then reads, in this order: int32 version
(stored at +0x3C), 4 bytes to +0x00, int32 to +0x04, +0x08, +0x0C; one discarded byte when the
version is 3 or 4; one byte to +0x10 when the version is 4 to 7; int32 to +0x14, +0x18, +0x1C,
+0x20. The fields are named by those offsets (`unk_0x04` ...); their meaning is not established.
In `Levels/ts/ts_02/umap/ts_02_mercuna_02.umap`: version 7, +0x00 = 50.0 as a float, then 14, 1,
8, byte 0, 6, 0x4000, 0x2000, 0xC000 (the last four equal the defaults the function sets).

### Node pool (signature `enkiOT16`)

8 bytes `enkiOT16` (the loader compares them as the 64-bit constant 0x3631544F696B6E65), int32
format (1 or 2; the loader calls `func_14576A1E0` for 1 and `func_14576A4C0` for 2), then
uint32 block count (→ +0x08), uint32 (→ +0x10), uint32 nodes per block (→ +0x14), then per block:

| Field | Type | Notes |
|---|---|---|
| used | uint32 | nodes in use (block +0x10) |
| count2 | uint32 | block +0x18; in format 1 the length of the uint16 list at the end; 0 in the data |
| unk_0x1c_u16 | uint16 | format 2 only (block +0x1C) |
| nodes | used × 32 bytes | read as raw memory into the block's node array |
| nodeWords | used × uint16 | read as raw memory into the block's second array |
| list | count2 × uint16 | format 1 only; the loader rebuilds a chain through the nodes from it |

All exports use format 2, 4,096 nodes per block, count2 = 0 and the value 14 at +0x10 (`audit/package_facts.json`). The loader does not interpret a node; from the bytes a node is eight
uint32 values and its word holds eight 2-bit values (e.g. node `02000000 610F0800 ... FB07D501`
with word 0x5555, node `00.. 03000000` with word 0x4000), which fits eight child slots of an
octree node with a 2-bit state each. That reading is an inference from the data, not from code;
nodes and words are kept packed (hex) in the JSON.

### Lists (`func_145768860`)

Five times: uint64 count, then per entry 16 bytes, uint32, uint64 n, n × 32 bytes. The loader
inserts each entry into one of five containers of the octree object (at +8, +0x48, ... step
0x40) keyed by the 16 bytes. Keys look like four uint32 (e.g. `001E0000 001F0000 40200000
02000000`); the 32-byte records are not interpreted further and are kept packed. The fifth
list is non-empty in 10 of the 174 converted exports (superseded copies included).

## What is and is not established

Established by the loader and confirmed by exact consumption of all exports: the framing above
(every count, size and record width). Not established: the meaning of the header fields, of a
node's eight values and of the 32-byte list records - they are named `unk_*` or kept as packed
fixed-width records.
