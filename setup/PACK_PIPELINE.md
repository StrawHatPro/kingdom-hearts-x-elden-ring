# The asset pack: how the setup builds it

The mod draws, animates and voices Sora, Donald and Goofy from an **asset pack** (`build/sora_pack`, about 19,000
files): models, skeletons, animations, textures, materials and their Elden Ring shaders, effects, sounds, the HUD and
menu movies, and the data tables the mod reads when it starts (`gen/`). Every file in it is made **on your PC from your
own KINGDOM HEARTS III install** (and a few from your own ELDEN RING install). The pack is never committed or
distributed (see `LEGAL.md`).

The setup (`setup.exe`, or `setup/setup.js` in a source checkout) builds it with one command, `tools/packgen.js`, in
seven phases. Each phase can be run on its own (`setup.exe --run tools/packgen.js <phase>` in a release download,
`node tools/packgen.js <phase>` from source), resumed (`--from STEP`) or listed (`--list`). The log is
`build/packgen/logs/packgen.log`.

| Phase | What it makes | Reads |
|---|---|---|
| `extract` | the pak key (found in your `KINGDOM HEARTS III.exe`, never written), a check that the executable is the build the tools know (`tools/packgen_exe_check.js`), and the KH3 folders the build reads, extracted from your paks (`setup/pack_inputs.json`: 4,274 folders) | your KH3 install |
| `convert` | the cooked files decoded and converted: packages to JSON, textures to DDS / PNG, meshes and animations to glTF, sounds to WAV (`assets_converted/`) | `assets/` |
| `research` | the research data the pack tools read (see below), made by the research scripts | `assets_converted/`, your executable, `tools/research`, `tools/packgen/notes` |
| `pack` | the pack tools, in the order the pack was built in development (Sora, weapon, materials, outfits, UI, effects, sounds, Donald and Goofy, the XS abilities, the form weapons, face) | everything above, `sheets/port`, your ELDEN RING install (item icons) |
| `replay` | the pack files the development streams made with their own tools, made again from **recipes** (see below); the effect tables and their overlays; Elden Ring's title picture from your ELDEN RING install | the pack, `tools/packgen/replay` |
| `shaders` | every Elden Ring shader of the pack (DXIL), by `kh-shaderc` and the DirectX shader compiler | the pack's translated materials, `tools/packgen/recipes/shaders.json` |
| `gen` | the generated Rust tables (`build/gen/rust`) and, for a prebuilt DLL, the tables made from KH3 data as the pack's data files (`build/sora_pack/gen/*.khg`, `tools/packgen_genruntime.js`) | the pack, `sheets/port`, the research data |

The whole build took about 30 minutes on the developer's PC (longer on a slower one) and needs about 80 GB of free disk space (extracted and converted files
included). It writes only inside the repository folder, in git-ignored folders.

## Where every input comes from

Everything the pipeline reads beyond your KH3 install falls into one of three kinds.

### (a) Made from your KH3 install (the `research` phase)

These were research data in development. They are KH3 data, so they are never in the repository: the research scripts
make them again on your PC, and the clean-room build (below) checked that they come out byte-identical to the
development tree's.

| Data | Made by |
|---|---|
| `audit/reflection.json` (the executable's reflected classes, structs, enums) | `tools/reflection.js` (reads the executable) |
| `sheets/reflect/enum-values.json` | `tools/packgen_enums.js` |
| `sheets/entities`, `sheets/systems` (KH3's own tables) | `tools/sheets_build.js` |
| `audit/blueprint_class_bases.json` (every blueprint's native base) | `tools/packgen_bases.js` (all the paks' headers) |
| `audit/sora_port/vfx_class_defaults.json` | `tools/sora_vfx_pack.js --defaults` |
| `audit/sora_port/{animgraph, legik, physics_asset, bonamik, bonamik2, kinedriver}.json`, the Bonamik notifies | the research scripts `tools/research/{graphdump, legik, bnm, kd}` |
| the sound trigger tables (notifies, footsteps, voices, weapon / cloth sets, blueprint and code sounds, the SEAD configuration and its events) | `tools/research/audio/...` |
| the effect inventory (Sora's clip notifies, anim sets, the effect list) | `tools/research/vfx/fxinv` |
| Donald and Goofy's anim sets and weapons | `tools/research/party`, `tools/packgen_party_inventory.js` |
| a skeleton manifest the scripts read before the pack exists | `tools/packgen_bootstrap.js` |

Two development inputs are not needed any more: the decompiler's function table (`sheets/native/code.json`, replaced by
the sheet's own address column) and the copy of the executable (`base/KH3.exe`, replaced by your installed executable).

### (b) Our own decisions and mappings (in the repository, as source)

| Data | Where |
|---|---|
| The port's design and research sheets | `sheets/port/` |
| The research scripts | `tools/research/<path>` (staged to `build/sora_tmp/<path>` when they run) |
| Notes: the parts of the research that came from reading KH3's code in development (which code sites play which effect or sound, the UI's ActionScript call map, the SQEX effect modules), reduced to what the tools read: names, addresses and our conclusions | `tools/packgen/notes` |
| The shader build plan: which `kh-shaderc` runs make the pack's shaders, with their defines (names only) | `tools/packgen/recipes/shaders.json`, `own_sets.json` |
| The effect tables' owner ids, the effect overlay translator runs | `tools/packgen/recipes/vfx_owners_*.json`, `vfx_stages.json` |
| The party's extra sounds list | `tools/packgen/recipes/party_extra_sounds.json` |
| The SHA-256 of the executable sections the tools read at fixed addresses (hashes only) | `tools/packgen/recipes/exe_sections.json` |
| The replay recipes (below) and the effect lists of their scratch packs | `tools/packgen/replay/` |
| The KH3 folders the build extracts | `setup/pack_inputs.json` |

`release-tools/scan.js` refuses any Japanese text (the game's text) in `tools/packgen`, and the recipe writer refuses
it too: a key of the game's (a material parameter name) is written as its hash and found again in your files.

### (c) Made from your ELDEN RING install

| Data | Made by |
|---|---|
| The Elden Ring item icons of the menus | `tools/er_icons.js` (the `pack` phase; keys read from your own `eldenring.exe`) |
| The Elden Ring title picture of the load screens | `tools/research/party/SP2/tools/logo_cut.js` (the `replay` phase), from the menu textures `er_icons.js` read |

## The replay recipes

During development several streams made pack files with their own scripts: Donald's composed spells, the party's and
the XS abilities' effect packs, the merged stream packs, the effect tables' overlays. Those scripts read development
folders, so the pipeline does not run them. Instead, `tools/packgen/replay/` holds a **recipe** for every such file:

- `copy`: the file is byte for byte a file the build made before (a scratch pack of KH3 effects, `build/packgen/scratch`,
  packed from your files by `tools/sora_vfx_pack.js` from the lists `scratch_*.json`);
- `json` (recipes v2, `tools/packgen_diff.js`): the file as its **differences from files the build made before** on
  your PC (the pack, the scratch packs, and the files this replay made before it). Every part that is the same as one
  of those files is a reference to it (a file + a JSON pointer), down to small integers, short strings and whole
  emitters and modules. A number that is such a value times one of our gains is the reference and our factor. What is
  left as a literal is ours: the curves, timings and colours our scripts authored for the composed effects, our
  changes to KH3's effects, and our build records;
- `jsontext`: a file in a layout of its own, its text with every value of the game's replaced by a reference;
- `text`: a WGSL overlay, each line a line of the translator run it came from (only comments and preprocessor lines are
  our own);
- `raw`: a file of our own design (a slot list, our light values);
- `bake`: an effect whose spawn points are baked from your textures afterwards (`vfxr_bake.js`).

Each recipe carries the file's SHA-256: `tools/packgen_replay.js` checks every file it writes, and the `replay-verify`
step checks the pack against all of them. A recipe never holds game data: what it cannot reference stays in
`tools/packgen/replay/report.json` as unresolved, and the file is made by the pack tools instead.

The streams' tools that only need the converted assets (the XS form weapons, Chandelier's pawn, the form weapons' maps,
the shatter fragments, Zettaflare's sounds and clips) run as they are, from `tools/research`.

## Checked: the clean-room build

The release was built in an empty folder from a copy of the release files, against a read-only KH3 install and ELDEN
RING install, with every read of the development tree refused (`release-tools/cleanroom_guard.js`), and compared file by
file with the development pack (`release-tools/pack_compare.js`). Every Node process of the run was guarded; none was
refused. Two pack builds in a row gave the same files (the build is deterministic).

Result (2026-10-24, Steam copies of both games, the development pack: 19,381 files):

| | Files | Why |
|---|---|---|
| identical | 17,987 | |
| KH1 outfit voice | 36 missing | not part of the release (the KH1 outfit speaks with KH3's battle voice) |
| shader variants the DLL no longer reads (`*_hy`, `*_gb`, `*_hy2`, `*_gb2` and their hair files, the folders' kh-shaderc index files) | 55 differing, 502 missing, 172 extra | not loaded by the current DLL |
| the XS effect shaders, the 2026-10-16 overlay shaders (and two shader index files) | 653 differing, 120 extra | the development pack still has a build of 2026-10-16 (an older compiler harness, before ZA's decal pass). Built from the development tree's own table with today's compiler they come out as the release's, 1,117 of 1,118 |
| the main and party effect tables | 2 differing | the translator now always evaluates TrigMath (RSF round 20); the records the development pack takes from its overlays (shading_vfb.json) are the same, so the tables the hosts use are identical |
| character shaders built again | 143 differing, 16 extra | the development pack's ER91 hair halves (`*_h91m`, `*_h91n`) are a build of 2026-10-17: today's compiler on the same inputs makes the release's bytes; the debug probes come from the harness with its probe fix |
| `vfx/triggers.json` | 1 differing | a description field from the release sheets, where Japanese quoted from the game is `…` (not read by the mod) |
| `ui/sheet_rows.json` | 1 differing | the UI tool's record of its files (not read by the mod) |
| `gen/*.khg`, `gen/schemas.json` | 9 extra | the tables a prebuilt DLL reads at start |
| a test harness dump (`dump.ps.dxil`) | 1 missing | a development aid |

The generated Rust tables: 56 of 63 identical; 3 differ because the development tree's copies are older than its pack
(made again from it today they are the release's), 4 because the release sheets leave out text quoted from the game.

The release download made from that folder (`release-tools/make_release.js`) was then unpacked into another empty folder:
its setup used the prebuilt DLLs (no Rust), checked them against the pack's tables, and the DLL's self-check read the
tables and the game's text from the pack. The prebuilt `kh-shaderc.exe` rebuilt all 5,554 shader and texture files of
the pack byte for byte.

## Steam and Epic

The pak key is found by searching your executable and checking it against the paks' own index hashes, so it works for
any build. The research tools read the executable at fixed addresses: `tools/packgen_exe_check.js` checks that your
executable's code and data sections are the ones the tools were made with (the Steam build; the Steam wrapper section
is not compared, so a store copy with the same code passes). An Epic copy has not been tested. If the check fails, the
setup stops with the sections that differ, before it makes any research data.
