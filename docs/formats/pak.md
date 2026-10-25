# `.pak` archive format (as used by KINGDOM HEARTS III, Steam)

Reader: `tools/lib/pak.js`. Evidence: `audit/triage/pak_footers.json`, `pak_key.json`,
`pak_index_summary.json`, `pak_probe.json`, and the per-entry rows in `audit/asset_manifest/`.
All integers are little-endian. Confidence is `confirmed` (parsed and cross-checked on all 142
archives, 555,921 entries) unless noted.

## File layout

```
[ entry 0 ][ entry 1 ] ... [ entry N-1 ][ index (encrypted) ][ footer, 45 bytes ]
```

## Footer (last 45 bytes)

| Offset | Size | Field | Value here |
|---|---|---|---|
| 0 | 1 | `bEncryptedIndex` | 1 |
| 1 | 4 | magic | `0x5A6F12E1` |
| 5 | 4 | version | 4 |
| 9 | 8 | index offset | |
| 17 | 8 | index size (multiple of 16) | |
| 25 | 20 | SHA-1 of the **decrypted** index | verified on all 142 |

## Index

AES-256-ECB, no IV, whole index encrypted in 16-byte blocks. The key is 32 printable bytes in the
executable's `.rdata` (file offset 0x7A17100); it is read from the user's exe at run time and
never stored here.

Decrypted:

```
FString  mount point            int32 length incl. NUL, then bytes (negative length = UTF-16)
int32    entry count
repeat:
  FString  file name (relative to the mount point)
  FPakEntry
(zero padding to a multiple of 16)
```

Mount points seen: `../../../`, `../../../KINGDOM HEARTS III/Content/...`, and `/` (five paks).

## FPakEntry (version 4)

| Size | Field | Notes |
|---|---|---|
| 8 | offset | file offset of this entry's own header; 0 in the copy stored before the payload |
| 8 | size | stored (compressed) size |
| 8 | uncompressed size | |
| 4 | compression method | 0 = none, 1 = zlib |
| 20 | SHA-1 | of the first `size` bytes of the stored payload (so before any encryption padding) |
| 4 + 16n | compression blocks | only if method != 0: count, then `{int64 start, int64 end}`; **absolute** file offsets |
| 1 | `bEncrypted` | |
| 4 | compression block size | uncompressed bytes per block, at most 65536 |

The same struct, with offset 0, sits immediately before each payload in the data area; it agrees
with the index copy for every entry (`header_mismatch: 0`).

## Payload

- **Not compressed:** `size` bytes. If encrypted, stored length is `size` rounded up to 16; the
  plaintext padding bytes are arbitrary (not zero) and are recorded per entry as
  `encryption_padding_hex` so the entry can be repacked exactly.
- **zlib:** blocks are contiguous, each an independent zlib stream (header `78 9C`). Every block
  in the game re-compresses byte-identically with **zlib 1.2.8, level 6, windowBits 15,
  memLevel 8** (507,965 entries). Other zlib builds do not reproduce the bytes (Node's bundled
  zlib 1.3.x fails), so `tools/native/zrt.exe` is built from the 1.2.8 source.
- **zlib + encrypted** (config `.ini` files): each block is padded to 16 bytes and encrypted
  separately; the block's `end - start` is the unpadded compressed length.

## Round-trip (`tools/pak_verify.js`)

For every entry: stored payload hashed (`sha256_raw`); decrypted and inflated (`sha256_extracted`);
then re-deflated with zlib 1.2.8 and/or re-encrypted and compared byte-for-byte with the stored
payload. `roundtrip_matches` is true only when every block compared equal.

## Mount order and the two voice overlay paks

142 paks ship: 140 in `Content/Paks/` and two more, both named `voice_7_P.pak`, in
`Content/_DLC_/VOICE/en/Paks/` and `Content/_DLC_/VOICE/ja/Paks/`.

`tools/lib/pak.js mountAll()` mounts the 140 regular paks only: base paks first, `_P` patch paks
after them, so that for every path the last pak wins - the view the game has with no overlay
mounted. The two voice paks are **overlays** and are left out (`isOverlayPak`,
`listOverlayPaks`, `overlayLanguages`). The game does not mount them with the regular paks: it
registers at most one of the two directories under the name `VOICE` (`func_141555B10`) and
mounts its paks through a separate DLC mount function at a priority above every regular and
patch pak. The code evidence is in `pak_mount_finding.md`.

Each overlay holds 35,894 entries. Every entry sits at a path of the *other* language (the `en`
pak holds `/ja/` paths, the `ja` pak `/en/` paths) and is identical - index SHA-1 and
uncompressed size - to the regular file at the same path with the language component swapped:
35,894 of 35,894 for both paks, 0 different, 0 without a regular file. They are therefore not
extracted. `tools/pak_extract_all.js` writes `audit/pak_overlays.json` instead:

```
overlays[]: { pak, voice_language, entries_use_paths_of_language, mount_point, entries, bytes,
              identical_to_regular_file, different, no_regular_file, mismatches[],
              manifest_sha256,
              files: [ [ mounted path in the overlay, SHA-1, size,
                         regular path it duplicates (under assets/), regular pak ] ... ] }
```

and the overlay paks' rows in `audit/asset_manifest/<pak>.json` carry `extracted_path: null`,
`on_disk: false`, `overlay: true` and `duplicates: "assets/<regular path>"`. Mounting an overlay
is thus a pure path mapping: path -> the listed regular file.

## Extraction (`tools/pak_extract_all.js`)

- Winner of a path (`build/pak_winners.json`) -> `assets/<mounted path>`; every other copy ->
  `assets/_superseded/<pak name>/<mounted path>`; a path that differs from another only in case
  -> `assets/_case_collision/<pak name>/...` (none with the corrected mount order).
- A file that is already there with the right size is re-hashed (SHA-256 against
  `sha256_extracted` of the manifest row, which `tools/pak_verify.js` recorded together with the
  check of the pak index SHA-1); a missing, wrong-sized or wrong-hash file is written from the
  pak and hashed again. The run reports `skipped_existing`, `written_missing_or_wrong_size`,
  `rewritten_wrong_hash` and `hash_mismatch`, so a run over a finished tree is a full
  verification.
- Afterwards files under `_superseded/` and `_case_collision/` that no manifest row names are
  removed (`stale_files` in `audit/asset_manifest.json`; `*.usm.d` directories are left alone;
  live files not named by a row are counted, never deleted).

Totals with the corrected mount order: 484,133 files, 78,198,780,656 bytes on disk (479,822
live paths, 4,311 superseded copies), 0 hash mismatches, 0 case collisions.
