# Files outside cooked packages

Tool: `tools/loose_files.js` → `audit/loose_files.json` (one row per file: format, how it was
identified, and for decoded formats the JSON path and whether it rebuilds identically).
The totals and the per-format table are reproduced in `README.md`.

Every check below is run on every file of the kind; a file that fails its check is reported
as `unidentified` (there are none).

## Decoded with a byte-exact round trip

| Kind | Reader/writer | Doc |
|---|---|---|
| `.locres`, `.locmeta` | `tools/lib/locres.js` | `locres.md` |
| `Engine/GlobalShaderCache-PCD3D_SM5.bin` | `tools/lib/pkg_classes/material.js` | `material.md` (its shader parameter bindings stay uninterpreted) |
| `DevelopmentAssetRegistry.bin` | `tools/lib/assetregistry.js` | `assetregistry.md` |

There is no shader code library (`.ushaderbytecode`) and no pipeline cache file in the paks;
the only shader container outside packages is the global shader cache.

## Text

`.ini` (Unreal config), `.txt`, `.json`, `.uplugin`, `.uproject`, `.tps` (XML), `.vdf` (Valve
KeyValues). Identified by decoding the whole file as UTF-8 (or UTF-16LE when it starts with
the BOM `FF FE`) without loss and without NUL characters; JSON kinds must also parse. Two
plugin descriptors (`DirectInput.uplugin`, `SQEXSEAD.uplugin`) have a comma before a closing
bracket and parse only once those are dropped; they are labelled accordingly. Text files are
their own readable form and are not converted.

487 of the `.json` files are not pak content: they are the sidecars `tools/usm_extract.js`
wrote next to the demultiplexed movie streams (below).

## Standard third-party formats (identified, not converted)

| Format | Files | Identified by |
|---|---|---|
| ICU data (`.res`, `.brk`, `.dict`, `.icu`, `.nrm`, `.cfu`, `.cnv` under `Engine/Content/Internationalization/icudt53l/`) | ICU 53 common data files | bytes `DA 27` at offset 2 and the `UDataInfo` block; the 4-character data format (`ResB`, `Brk `, `Dict`, `CvAl`, `UCol`, `Nrm2`, `Cfu `, `cnvt`, `unam`) is recorded per file |
| TrueType (`.ttf`, `.ufont`) | fonts | sfnt version `00 01 00 00` and a table directory whose tables lie inside the file |
| PNG (`.png`) | Slate UI images | signature, every chunk's CRC-32 verified, ends with `IEND` |
| H.264 Annex B (`.h264`), MPEG-1 video (`.m1v`), CRI HCA (`.hca`) | movie streams | start code `00 00 00 01` / sequence header `00 00 01 B3` / signature `HCA\0` |

The movie streams live in `<name>.usm.d/` directories. They were produced by
`tools/usm_extract.js` from the CRI USM containers (format and round-trip proof in `usm.md`,
`audit/usm_manifest.json`); the census counts them because they are under `assets/`.

## Package data without a header

`.uexp` / `.ubulk` files with no `.uasset`/`.umap` of the same name beside them
(`orphan_package_data_files` in the audit). `pak_mount_finding.md` gives the pak-index
evidence for where they come from.

`tools/package_convert.js` pairs such a `.uexp` with a header and converts it like any package
(JSON at `assets_converted/packages/<path>.uexp.json`, `headerFrom` naming the header used;
`totals.orphan_uexp_paired_with_a_header` in `audit/package_manifest.json`), accepting a header
only when its export table tiles the file exactly (the exports end 4 bytes before the end,
where the package tag is). It tries, in order:

1. a header at the same path in the live tree or another superseded pak - the case of the
   `.uexp` files that a patch pak replaced without replacing the header;
2. a header of the same file name in a directory one level below (`<dir>/<lang>/<name>`) - the
   case of the `*_loc.uexp` textures that the paks ship without any header at their own path.

`tools/loose_files.js` rebuilds every paired file once more from its JSON (3,829 files).

For the files no header tiles, it makes two further checks with `tools/lib/texture.js`:

- ten `.uexp` files (e.g. `UI/Scaleform/01_hud/act_logo/ActLogoF21Txt_loc.uexp`) are read
  whole as one `Texture2D` export followed by the package tag, using the name table of a
  per-language header of the same name; that header's own export has another size (another
  texture resolution), so no header for them exists in the paks. They are reported as
  "cooked Texture2D export data (.uexp) with no header in any pak" with format and size;
- the four superseded `.ubulk` files are covered exactly by the mip table of the live copy of
  their package (a patch pak replaced only the `.ubulk`).

Two files remain uninterpreted: the live and the superseded copy of
`UI/Scaleform/01_hud/hud_left/HudLeft_loc.uexp` (8,555 bytes each). No pak has a header at that
path, and with the name tables of the per-language `HudLeft_loc.uasset` files its tagged
properties do not parse.
