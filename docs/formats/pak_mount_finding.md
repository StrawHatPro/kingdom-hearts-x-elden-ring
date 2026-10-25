# Finding (fixed): the two voice paks are overlays, not part of the mount order

Scope: `tools/lib/pak.js` `mountAll()` / `tools/pak_extract_all.js`, the layout of `assets/`.
Status: **fixed**. The extraction used to mount both voice paks as ordinary patch paks; it now
leaves them out and records them as a path mapping (`audit/pak_overlays.json`), and the tree
under `assets/` was corrected in place. Evidence: the pak indexes
(`tools/lib/pak.js readIndex`), `build/pak_winners.json` and the executable (decompiled
functions are cited by address; `build/decomp/`, `re/decomp/<prefix>/<ADDR>.c`).

## What the paks contain

142 paks: 140 in `Content/Paks/` and two more, both named `voice_7_P.pak`, in
`Content/_DLC_/VOICE/en/Paks/` and `Content/_DLC_/VOICE/ja/Paks/`. Both mount at
`../../../KINGDOM HEARTS III/Content/` and hold 35,894 entries each.

- Every entry of `VOICE/en/.../voice_7_P.pak` is at a `/ja/` path (for example
  `Sound/VOICE/Localization/ja/...`, `Cutscene/Localization/ja/...`) and its SHA-1 and
  uncompressed size in the pak index equal those of the file at the corresponding `/en/` path
  that wins in the regular paks: 35,894 of 35,894.
- Every entry of `VOICE/ja/.../voice_7_P.pak` is at an `/en/` path and equals the regular
  `/ja/` file: 35,894 of 35,894.

(0 different, 0 without a regular file; re-checked by every run of `tools/pak_extract_all.js`,
which exits 1 otherwise.) So each voice pak is an overlay that puts one language's voiced
assets at the other language's paths. Example (pak index SHA-1):

| Path | pakchunk0_s6 (base) | VOICE/en pak | VOICE/ja pak |
|---|---|---|---|
| `Cutscene/Localization/en/ss/cutscene_ss50_memory1.umap` | 2c34147b... | - | cb844c6a... |
| `Cutscene/Localization/en/ss/cutscene_ss50_memory1.uexp` (44,395 / 44,716 bytes) | f55af312... | - | be0d9cb9... |
| `Cutscene/Localization/ja/ss/cutscene_ss50_memory1.umap` | cb844c6a... | 2c34147b... | - |
| `Cutscene/Localization/ja/ss/cutscene_ss50_memory1.uexp` | be0d9cb9... | f55af312... | - |

## What the game does (from the executable)

**Which directory.** `func_141555B10` is the only code that references the string `_DLC_`
(0x146A46928). It obtains a language string from the game's settings object (`f_140ECA1E0`),
compares it with `"ja"`, builds a path ending in `_DLC_` + `VOICE` and appends one more
component: `"en"` when the language string equals `"ja"`, otherwise `"ja"`. It registers
that single directory under the name `VOICE` (`f_142CF7420`: name -> directory, map at
0x14871C910 under the critical section 0x149B2FAD0) after removing a previous registration
(`f_142D06D20`). It is called from the state machine `func_14156F0C0`, case 0xb, when
`param_1[0xbd] != 0`. `func_142CF49E0` translates a path whose components start with
`DLC/<registered name>` to the registered directory (`func_142CF6630` = is the name
registered).

**How it is mounted.** `func_144EF8980(FPakPlatformFile, name, outList)` enumerates the paks
in `"/DLC/%s/Paks"` (string 0x1479E79A0), i.e. through the translation above in the registered
directory. `func_144EF8E10(FPakPlatformFile, name)` mounts each of them that is not mounted yet
with `f_144EF4D30(this, pakPath, 5, nullptr)`; `func_144EF9050` unmounts them. The regular
start-up mount (`func_144EF1AB0`) does not look into `_DLC_`.

**Priority - established.** In the mount function `func_144EF4D30` the order argument becomes
`order + 100 * EndsWith("_P.pak")` (string 0x1479E7830). `func_144EF1AB0` gives the regular
paks the orders 0 to 4 by location as stock UE4 does; 4 only for names matching
`"%sPaks/%s-"` with the project name `TresGame`, which no shipped pak has, so the
`Content/Paks` paks get 3 and their `_P` patch paks 103. The overlay is mounted with order 5
and is named `voice_7_P.pak`: **105**. An overlay therefore outranks every base pak (3) and
every patch pak (103). This agrees with the contents: for
`cutscene_ss50_memory{1,3,4,5}` the overlay holds both `.umap` and `.uexp` while a patch
pak holds only the `.uexp` of the regular language - only an overlay above the patch paks
gives a consistent pair.

**When it is mounted - established as far as the code reads.** `func_141562020` reads one
language string from the internationalization object (`f_142D644D0`) and determines a second,
requested one: with a settings object present it takes that object's string
(`f_140ECC3E0`), otherwise `"en"` or `"ja"` chosen from a flag (`f_140ECC4D0`, inverted
under a further condition). It then unmounts the `VOICE` paks (`f_144EF9050`), stores the
first string as the object's voice language (setter `func_142D64B10`, member +0xF8; the valid
values are a two-entry list at 0x14871D888), and **only if the two strings differ** stores the
requested string and mounts the `VOICE` paks (`f_144EF8E10`); when that mounts nothing, the
member is reset to the empty string. `func_14066EAC0` / `func_14152DD10` answer "is the
`VOICE` directory registered and does it hold paks"; `func_14170C0E0` is a second caller of
the mount. So: no overlay when the requested voice language equals the language the object
already has; exactly one overlay otherwise - at most one of the two paks is ever mounted.

Not established:

- which of the object's languages `f_142D644D0` returns (the code only shows that it is the
  value the voice language defaults to), and which user-visible option sets the flag read by
  `f_140ECC4D0` - both functions were read only as far as their use here;
- the string `"/DLC/VOICE/%s"` (0x146533258) has no code or data reference that was found;
- nothing was run: the statements above are from the code, not from observing the game.

## What the extraction did, and the fix

`mountAll()` sorted all 142 paks with `_P` paks last and ties broken by the absolute path.
`Content/_DLC_/...` sorts before `Content/Paks/...`, so both voice paks were mounted as
ordinary patch paks below the `Content/Paks/*_P.pak` files - a state the game never has.
Consequences in the old tree:

1. 71,778 live files (35,885 `.uasset`, 35,885 `.uexp`, 8 `.umap`) were the voice paks'
   copies: every localized voice, cutscene and facial-animation asset under an `/en/` path was
   the Japanese-voiced version and the reverse. The regular versions sat under
   `assets/_superseded/<regular pak>/`. No bytes were missing; the labels were swapped.
2. Eight packages - `Cutscene/Localization/{en,ja}/ss/cutscene_ss50_memory{1,3,4,5}.umap` -
   had the `.umap` of a voice pak beside the `.uexp` of a `pakchunk0_s*_0_P` patch pak:
   header and export data of different languages.
3. Four paths counted as case collisions (`assets/_case_collision/`).

Fix:

- `tools/lib/pak.js`: `mountAll()` leaves out `Content/_DLC_/VOICE/<lang>/Paks/*.pak`
  (`isOverlayPak`); `listOverlayPaks()` and `overlayLanguages()` expose them.
- `tools/pak_extract_all.js`: overlay paks are not extracted. Each entry is checked against
  the winning regular file at the swapped-language path and recorded in
  `audit/pak_overlays.json` (overlay pak, entry path, SHA-1, size, the regular path it
  duplicates, that file's pak); their manifest rows carry `overlay: true` and `duplicates`.
  Files under `_superseded/` and `_case_collision/` that no manifest row names are removed.
- The tree was corrected by that tool, without a full re-extract: the 71,778 live paths were
  rewritten with the regular version from the paks (the tool rewrites any file whose SHA-256
  differs from its manifest row), and 71,788 stale files (1,707,033,841 bytes, 5,026
  directories) were removed: the 71,778 superseded regular copies that are now the live files
  and the 10 remaining voice-pak copies. `assets/_superseded/voice_7_P/` is gone and
  `_case_collision/` is empty (0 case collisions - the four were an artifact of the overlays).
- Result, re-verified by a complete hash pass: 484,133 files, 78,198,780,656 bytes, 479,822
  live paths, 4,311 superseded copies, 0 hash mismatches.
- The eight `cutscene_ss50_memory{1,3,4,5}` packages now have the `.umap` of
  `pakchunk0_s6-WindowsNoEditor.pak` and the `.uexp` of the `pakchunk0_s5` / `s6` / `s7`
  `_0_P` patch pak - the pair the game has without an overlay - and decode like any package
  (`decoded_with_another_header` in `audit/package_manifest.json` is empty).

To see the tree as the game has it with an overlay mounted, apply the mapping of
`audit/pak_overlays.json`: each listed path resolves to the listed regular file.

## Header-less `.uexp` files

3,841 `.uexp` files have no header beside them.

- 3,663 are under `_superseded/`: a `Content/Paks/*_0_P.pak` patch replaces the `.uexp` of a
  package but not its header (in `pak_winners.json` exactly 3,663 packages have header and
  `.uexp` from different paks, the eight above among them). The replaced `.uexp` is decoded
  with the live header (same path), accepted because its export table tiles the file.
- 166 are files such as `UI/Scaleform/01_hud/act_logo/ActLogoA01Txt_loc.uexp` (83 in the live
  tree from patch paks, 83 superseded base copies): the paks contain `<dir>/<name>.uexp` but a
  header only in the per-language directories `<dir>/{de,en,es,fr,it,ja}/<name>.uasset` - no pak
  has `<dir>/<name>.uasset` under any spelling. These are genuinely orphaned in the shipped
  data. Each tiles with the per-language header of the same name (one `Texture2D` export) and
  `tools/lib/texture.js` parses all 166; the file offsets stored inside are 3 less than that
  header implies in every one of them - the header they were cooked with was 3 bytes shorter,
  which is the length of the `/de` path component in the package name. They are converted
  with the first such header (`headerFrom` in the JSON) and their bytes are not the same as any
  language's `.uexp`.
- 12 `.uexp` and 4 `.ubulk` have no header whose export table tiles them. Ten of the `.uexp`
  are single `Texture2D` exports of another resolution than their per-language namesakes
  (read whole by `tools/lib/texture.js`), the four `.ubulk` are superseded mip data covered
  exactly by the live header's mip table, and two (`HudLeft_loc.uexp`, live and superseded
  copy) stay uninterpreted. See `loose_files.md` and `audit/loose_files.json`.
