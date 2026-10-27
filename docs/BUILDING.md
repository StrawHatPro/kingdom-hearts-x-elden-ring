# Building and advanced setup

This page is for people who want to build the mod from source, or who need the setup's extra options. Players don't
need any of it: see the [README](../README.md).

## What's in the repository

| Folder | What |
|---|---|
| `crates/` | The mod's Rust source. `kh-er` is `kh_er.dll`, `kh-er-loader` is `kh_er_loader.dll` (what me3 loads), `kh-shaderc` is the shader compiler. |
| `vendor/fromsoftware-rs` | The Elden Ring SDK (MIT or Apache-2.0), with one added address table for 2.7.2.0. |
| `setup/` | The setup (`setup.js`), its batch shortcut (`setup.sh` on Linux), the settings template and `PACK_PIPELINE.md`. |
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
   [setup/PACK_PIPELINE.md](../setup/PACK_PIPELINE.md). A finished pack is kept. After an update the setup compares two
   fingerprints of the pack's inputs (`build/packgen/pack_inputs_fp.json`): when only the shader compiler or the shader
   recipe changed it makes the shaders again (`tools/packgen.js shaders`, a few minutes); when any other pack tool
   changed, or `PACK_EPOCH` in `setup/setup.js`, it builds the pack again. A pack from before the fingerprints gets its
   shaders made again once. `--rebuild-pack` always builds it all.
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

## Linux

The Linux setup does the same eight steps as the Windows one and makes the same pack, tables and settings from the same
game files. The mod stays the same Windows DLLs: they run inside Proton, with Elden Ring. What is different:

| | Windows | Linux |
|---|---|---|
| The setup program | `setup.exe` (Node.js win-x64 as a single executable) | `setup-linux` (the same Node.js version, linux-x64, the same `release-tools/sea_main.js`; its hash in `release-tools/setup_linux.json`). It can't be called `setup`: that's the setup's folder. |
| Without the release download | `setup\setup.bat`, `setup\get_node.ps1` | `setup/setup.sh`, `setup/get_node.sh` (the pinned linux-x64 Node of `release-tools/node_pin.json`, checked by SHA-256) |
| Finding the games | Steam (registry), Epic's manifests | Steam's folders (`~/.steam/steam`, `~/.local/share/Steam`, the Flatpak and Snap ones) and every library in `libraryfolders.vdf`; Epic games installed by Heroic (or legendary), and those of an Epic launcher in a Wine prefix (Lutris). Folders are found whatever their case. |
| Folder picker | Windows' folder dialog | zenity or kdialog when there is a desktop, else a typed path |
| Elden Ring's version | PowerShell | read from `eldenring.exe`'s version resource |
| DirectX shader compiler | the Windows SDK's, or Microsoft's v1.8.2502 zip | Microsoft's v1.8.2502 Linux build (`bin/dxc`, `lib/libdxcompiler.so`, `lib/libdxil.so`), downloaded and checked by SHA-256 |
| Shader compiler | `tools/bin/kh-shaderc.exe` | `tools/bin/kh-shaderc` (the same crate, built for `x86_64-unknown-linux-musl`) |
| KH3's compiled shaders read as text | Windows' `d3dcompiler_47.dll` (`tools/lib/dxbc_disasm.ps1`) | `tools/lib/dxbc_disasm.js`, which writes the same text byte for byte. Wine's d3dcompiler writes a different text, so it isn't used. `release-tools/dxbc_disasm_check.js` proves it against a whole build's shaders and against generated test shaders. |
| Elden Ring's icon archives (Oodle) | the game's `oo2core_6_win64.dll` through PowerShell | the same DLL, loaded by `tools/bin/kh-oodle` (crate `kh-oodle`, a small PE loader); there is no Oodle for Linux |
| Folder listing order | NTFS's | `tools/lib/ntfs_order.js` (loaded into every Node process of the run) gives the NTFS order, so tools that follow the order they find files in write the same files |
| The DLL's self-check | `release-tools/selfcheck.ps1` loads the DLL | the DLL only runs inside Proton: `tools/bin/kh-er-selfcheck` (crate `kh-selfcheck`) runs the same check code, built for Linux against the same table list, plus the loader's version text check |
| me3 | `me3.exe`; `er\play.bat` | me3's Linux build (0.13.0 or newer, from [its installer](https://github.com/garyttierney/me3/blob/main/docs/user-guide/installation.md): `~/.local/bin/me3`); `er/play.sh` |
| Admin checks | `setup\launch_check.ps1` | not needed: Linux has no "always run as administrator" flag, and Steam starts the game. `play.sh` refuses to run as root, because Steam does. |

me3 on Linux starts Elden Ring through Steam with the Proton that Steam uses for it (the game's own setting, else Steam's
default, else the one Steam Deck verifies), in the game's own prefix (`steamapps/compatdata/1245620`). The profile
`er/profiles/kh-sora.me3` is the same file: me3 reads its paths relative to the profile's folder, on Linux too.

The same pack on both: the pack tools don't depend on the machine. Names are compared with a fixed collation
(`localeCompare(x, 'en')`, which is what Windows' default gives; on Linux the default follows `LANG`, and `LANG=C` sorts
differently), the folder walks whose order shows in the pack sort explicitly in Windows' order, and a pack build that
starts from the beginning starts from an empty `build/sora_pack` (a pack tool records what its folder holds). It is proven
on Windows with `release-tools/linux_sim.js`: Linux's folder order, a C locale, case-sensitive folders and the two Linux
replacements, in a clean room, compared file by file with a Windows build (`release-tools/pack_compare.js`).

Notes:
- **Flatpak Steam:** me3 and the mod's folder must be reachable from Steam's sandbox. If the game doesn't start, keep the
  mod's folder in your home folder and give Steam access to it, e.g.
  `flatpak override --user --filesystem=home com.valvesoftware.Steam`.
- The setup's tools need glibc 2.28 or newer for `setup-linux` (every current distribution and SteamOS 3); the three
  tools in `tools/bin` are static and run anywhere.

**Building from source on Linux:** Rust (<https://rustup.rs>), and for the Windows DLLs
[cargo-xwin](https://github.com/rust-cross/cargo-xwin) (`cargo install cargo-xwin`,
`rustup target add x86_64-pc-windows-msvc`). Then `setup/setup.sh`: it builds `kh-shaderc` and `kh-oodle` for Linux
(before the pack), the DLLs with `cargo xwin build --release --target x86_64-pc-windows-msvc -p kh-er -p kh-er-loader`
and `kh-selfcheck` (after the tables). The DLLs built this way have not been tried in the game yet: the release download's
DLLs are the tested ones.

**The release download:** `node release-tools/make_release.js` (on Windows) builds the linux-x64 tarball after the win64
zip: the same folder with `setup-linux` and the three Linux tools (built with
`cargo build --release --target x86_64-unknown-linux-musl`, linked with the Rust toolchain's own `rust-lld`; add the
target once with `rustup target add x86_64-unknown-linux-musl`), the same checks, and a deterministic `.tar.gz` that
keeps the programs and scripts executable. `--no-linux` skips it.

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
  `THIRD-PARTY-NOTICES.txt`, then the checks; then the linux-x64 tarball (see "Linux"). It uploads nothing. The zip is
  written by `release-tools/zip.js`, with `/` in every name, so it unpacks into its folders with every unzip tool.
- `node release-tools/dxbc_disasm_check.js <build/cache>` proves the Linux shader disassembler against Windows' own.
- `node release-tools/export.js --dev <dev tree>` refreshes this repository from the developer's working tree.
- Before a release: if it changes a design sheet in `sheets/port` that the pack is made from (not one the DLL reads at
  run time), bump `PACK_EPOCH` in `setup/setup.js`, so that players' packs are built again on their update. Changes to
  the pack tools and to the shader compiler are noticed by themselves.
