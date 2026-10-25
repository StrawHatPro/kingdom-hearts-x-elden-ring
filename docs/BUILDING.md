# Building and advanced setup

This page is for people who want to build the mod from source, or who need the setup's extra options. Players don't
need any of it: see the [README](../README.md).

## What's in the repository

| Folder | What |
|---|---|
| `crates/` | The mod's Rust source. `kh-er` is `kh_er.dll`, `kh-er-loader` is `kh_er_loader.dll` (what me3 loads), `kh-shaderc` is the shader compiler. |
| `vendor/fromsoftware-rs` | The Elden Ring SDK (MIT or Apache-2.0), with one added address table for 2.7.2.0. |
| `setup/` | The setup (`setup.js`), its batch shortcut, the settings template and `PACK_PIPELINE.md`. |
| `tools/` | The asset tools the setup runs: extraction, conversion, research, the pack tools, the recipes. |
| `sheets/port/` | The mod's design and research tables. |
| `release-tools/` | The release checks and builders: `scan.js`, `make_release.js`, the `setup.exe` builder. |
| `documentation/`, `docs/` | Sora, Donald and Goofy as they are now, and the design specs. |

No game files are in the repository. Everything taken from the games is made on your PC from your own installs. See
[LEGAL.md](../LEGAL.md).

## Building from source

You need:
- **Rust** (stable, `x86_64-pc-windows-msvc`) with the Visual Studio C++ Build Tools: <https://rustup.rs>;
- **Git**;
- the same games and me3 as a player.

Node.js is optional. `setup\setup.bat` uses a Node 20+ on your PATH if you have one. If not, it downloads the pinned
portable Node (`release-tools/node_pin.json`, from nodejs.org, checked by its SHA-256) into `tools\bin\node`, and runs
`npm ci` once for the locked npm packages.

```
git clone https://github.com/StrawHatPro/kingdom-hearts-x-elden-ring.git
cd kingdom-hearts-x-elden-ring
setup\setup.bat
```

A source checkout has no `setup.exe` and no prebuilt DLLs. The setup builds the asset pack and then the DLLs with
`cargo build --release -p kh-er -p kh-er-loader`, into `er\mods`.

If you already have Node 20+, `node setup/setup.js [options]` runs the same setup.

## Setup options

Options go after `setup.exe` (or `setup\setup.bat`), from a command prompt in the mod's folder. `setup.exe --help` lists
them all.

| Option | What it does |
|---|---|
| `--kh3 DIR` | The KINGDOM HEARTS III install folder (the one that contains `KINGDOM HEARTS III\Binaries\Win64`). |
| `--er DIR` | The ELDEN RING install folder (the one that contains `Game\eldenring.exe`). |
| `--browse` | Ask for every missing folder with a folder picker. (Double-clicking `setup.exe` does this anyway.) |
| `--me3 FILE` | Where `me3.exe` is, if the setup can't find it. |
| `--rebuild-pack` | Build the asset pack again even if one is there. |
| `--pack DIR` | Use an asset pack that is already built (written to `er\mods\kh-er.cfg`). |
| `--gen DIR` | Use generated tables that are already built. |
| `--from-source` | Build the DLLs with cargo even when prebuilt ones are there. |
| `--skip-build` | Don't run cargo. |
| `--dxc FILE` | The DirectX shader compiler to use. |
| `--no-download` | Never download anything (the shader compiler, Node, npm packages). |
| `--workers N` | How many worker processes the converters and the shader compiler use. |
| `--yes` | Never ask questions: stop instead. |

## What the setup does

1. Checks Node, the npm packages, the DirectX shader compiler (it downloads Microsoft's release if the Windows SDK isn't
   installed), me3, and Rust when it builds from source.
2. Finds KINGDOM HEARTS III (Steam libraries, then Epic manifests) and ELDEN RING (Steam), and checks Elden Ring's
   version.
3. Finds the KH3 pak key in your own KH3 executable. Only its location and a hash are kept.
4. Builds the asset pack (`build\sora_pack`) from your KH3 install with `tools/packgen.js`. See
   [setup/PACK_PIPELINE.md](../setup/PACK_PIPELINE.md). A finished pack is kept.
5. Makes the generated tables, including the data files the DLL reads when it starts.
6. A release download: checks that the prebuilt DLLs match your pack. From source: builds them.
7. Writes `er\mods\kh-er.ini` (the mod's settings, first run only), checks the me3 profile, and runs the DLL's offline
   self-check.
8. Prints how to launch.

Nothing is written into either game's folder. It needs about 80 GB of free space while it builds the pack.

## setup.exe

`setup.exe` is the pinned Node.js (`release-tools/node_pin.json`) as a Node single executable application. The program
inside it is `release-tools/sea_main.js`:
- with no script argument, it runs `setup/setup.js`;
- `setup.exe --run <script.js> [args]` (or `setup.exe <script.js> [args]`) runs that tool script from disk, the way
  `node <script.js>` would. This is how the tools start each other;
- a Node option before the script (`--max-old-space-size=N`) is passed on through `NODE_OPTIONS`.

`release-tools/make_release.js` builds it (`release-tools/sea_build.mjs`) and records its SHA-256 in
`release-tools/setup_exe.json`. The build is deterministic.

## Settings and save files (advanced)

- The mod's settings are in `er\mods\kh-er.ini`, written by the setup the first time. One `key = value` per line.
  Development and cheat keys are ignored in the released mod.
- A `.me3` profile you write yourself must give the mod its own save file: add `savefile = "KHSora.sl2"` at the top,
  before the first `[[...]]` section. Any name ending in `.sl2` works, except `ER0000.sl2`.
- me3 0.13.0 or newer is required. When the profile's save file doesn't exist yet, me3 creates it in
  `%APPDATA%\EldenRing\<your id>\` as a copy of `ER0000.sl2`.
- The mod finds the save file the game really opens and only ever reads it. If the log says it was not detected, name
  it in `er\mods\kh-er.ini`: `save.file = KHSora.sl2` (the name only).
- The KH progress is in `er\saves` in the mod's folder.

## Release tools

- `node release-tools/scan.js` checks the repository for anything that must not be shared (game files, keys, large
  files, machine paths). `--release-binaries` checks a release folder.
- `node release-tools/make_release.js` builds the release zip into `dist/`: fresh DLLs, `setup.exe`, the npm packages,
  `THIRD-PARTY-NOTICES.txt`, then the checks. It uploads nothing.
- `node release-tools/export.js --dev <dev tree>` refreshes this repository from the developer's working tree.
