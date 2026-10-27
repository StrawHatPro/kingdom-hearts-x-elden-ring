#!/usr/bin/env node
// Kingdom Hearts x Elden Ring: the first-run setup. Run from the repository folder (a release download or a checkout):
//
//   double-click setup.exe                 (a release download: Node.js built in; `setup.exe [options]` from a prompt)
//   double-click setup\setup.bat           (runs setup.exe; in a source checkout: a Node 20+ on PATH, or it gets the
//                                           pinned portable Node into tools\bin\node; options are passed on)
//   node setup/setup.js [options]          (advanced: with a Node 20+ of your own)
//
// On Linux (Elden Ring and KH3 through Steam's Proton, Steam Deck too; docs/BUILDING.md, "Linux"):
//   ./setup [options]                      (the linux-x64 release download: Node.js built in; from a terminal)
//   setup/setup.sh [options]               (runs ./setup; in a source checkout: a Node 20+ on PATH, or it gets the pinned
//                                           portable Node into tools/bin/node)
// The steps and what they make are the same; the Linux names of the tools (tools/bin/kh-shaderc, kh-oodle,
// kh-er-selfcheck, the Linux dxc) and Linux's own places (Steam's folders, Heroic / Lutris for Epic) are used.
//
// Steps (each one says what it found; nothing is ever written into either game's folder):
//   1. prerequisites: Node, npm packages, the DirectX shader compiler (the Windows SDK's, or Microsoft's release downloaded
//      once), me3; Rust only when the DLLs are built from source
//   2. finds KINGDOM HEARTS III (Steam, Epic, or asks) and ELDEN RING (Steam, or asks) and checks both folders
//   3. finds the KH3 pak key in YOUR OWN KH3 executable (tools/pak_key.js; only where it is, never the key, is written)
//   4. the asset pack, made on this PC from your own KH3 install (tools/packgen.js: extract, convert, research, pack,
//      replay, shaders; setup/PACK_PIPELINE.md), with Elden Ring's item icons and title from your own ER install
//   5. the generated tables (tools/packgen.js gen): the Rust tables, and the tables made from KH3 data as the pack's data
//      files (build/sora_pack/gen, tools/packgen_genruntime.js), which the released DLL reads when it starts
//   6. the DLLs: a release download has them prebuilt in er/mods (checked against the pack's data); a source checkout
//      (or --from-source) builds them with cargo
//   7. writes er/mods/kh-er.ini, the mod's settings (first run only; an older setup's er/mods/kh-er.ctl is converted into
//      it once, tools/ctl_to_ini.js), checks the me3 profile er/profiles/kh-sora.me3, and runs the DLL's
//      offline self-check (release-tools/selfcheck.ps1: the DLL finds and reads the pack's generated data)
//   8. prints how to launch
//
// Options:
//   --kh3 DIR        the KH3 install folder (the one that contains "KINGDOM HEARTS III\Binaries\Win64")
//   --er DIR         the Elden Ring install folder (the one that contains "Game\eldenring.exe")
//   --me3 FILE       me3.exe, when it is not installed where the setup looks
//   --pack DIR       an asset pack that is already built (written into er/mods/kh-er.cfg)
//   --gen DIR        generated Rust tables that are already built (used as KH_GEN_RUST_DIR for a build from source)
//   --rebuild-pack   step 4: build the pack again even when one is there
//   --from-source    step 6: build the DLLs with cargo even when prebuilt ones are there
//   --skip-build     do not run cargo (step 6)
//   --dxc FILE       the dxc.exe to compile the pack's shaders with
//   --no-download    never download anything: the DirectX shader compiler (step 1), the npm packages (a source checkout),
//                    Node.js (setup\setup.bat)
//   --workers N      parallel workers of the converters and the shader compiler (default: the CPU count - 1)
//   --browse         ask for missing folders with a folder dialog instead of a typed path
//   --yes            never ask: a missing answer stops the setup
//
// Exit codes: 0 ready to play, 1 a step failed, 2 bad arguments.
'use strict';
const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const crypto = require('crypto');
const readline = require('readline');

const REPO = path.resolve(__dirname, '..');
const ER_DIR = path.join(REPO, 'er');
const MODS = path.join(ER_DIR, 'mods');
const PROFILE = path.join(ER_DIR, 'profiles', 'kh-sora.me3');
const TESTED_ME3 = '0.13.0';
const ER_VERSIONS = ['2.7.1.0', '2.7.2.0'];
const ER_APPID = '1245620';
// The DirectX shader compiler the pack's shaders are compiled with when the Windows SDK's is not installed: Microsoft's own
// release DirectXShaderCompiler v1.8.2502 (the version of the Windows SDK 10.0.26100 the mod is built with; the LLVM and
// MIT licences and dxil.dll's MS licence come in the zip), downloaded once into tools/bin/dxc, checked by its SHA-256.
const DXC_URL = 'https://github.com/microsoft/DirectXShaderCompiler/releases/download/v1.8.2502/dxc_2025_02_20.zip';
const DXC_SHA256 = '70b1913a1bfce4a3e1a5311d16246f4ecdf3a3e613abec8aa529e57668426f85';
const DXC_DIR = path.join(REPO, 'tools', 'bin', 'dxc');
// (Linux) the same release's Linux build (bin/dxc, lib/libdxcompiler.so, lib/libdxil.so, the same licences), into the same
// folder, checked by its SHA-256
const DXC_LINUX_URL = 'https://github.com/microsoft/DirectXShaderCompiler/releases/download/v1.8.2502/linux_dxc_2025_02_20.x86_64.tar.gz';
const DXC_LINUX_SHA256 = 'e0580d90dbf6053a783ddd8d5153285f0606e5deaad17a7a6452f03acdf88c71';

// ---------------------------------------------------------------- arguments
const argv = process.argv.slice(2);
const FLAGS = new Set(['--rebuild-pack', '--from-source', '--skip-build', '--no-download', '--browse', '--yes', '--help']);
const VALUES = new Set(['--kh3', '--er', '--me3', '--pack', '--gen', '--dxc', '--workers']);
const opt = {};
for (let i = 0; i < argv.length; i++) {
  const a = argv[i];
  if (FLAGS.has(a)) opt[a.slice(2)] = true;
  else if (VALUES.has(a) && i + 1 < argv.length) opt[a.slice(2)] = argv[++i];
  else { console.error(`unknown option ${a} (setup.exe --help, or node setup/setup.js --help from source)`); process.exit(2); }
}
if (opt.help) {
  console.log(fs.readFileSync(__filename, 'utf8').split('\n').filter((l) => l.startsWith('//')).map((l) => l.slice(3)).join('\n'));
  process.exit(0);
}

// ---------------------------------------------------------------- helpers
const isWin = process.platform === 'win32';
const say = (s = '') => console.log(s);
const step = (n, s) => say(`\n== ${n}. ${s}`);
const ok = (s) => say(`   ok    ${s}`);
const warn = (s) => say(`   WARN  ${s}`);
const bad = (s) => say(`   FAIL  ${s}`);
function stop(code, lines) {
  say('');
  for (const l of [].concat(lines)) say(l);
  process.exit(code);
}
function capture(cmd, args, opts = {}) {
  const r = cp.spawnSync(cmd, args, { encoding: 'utf8', windowsHide: true, ...opts });
  return { status: r.status, out: `${r.stdout || ''}${r.stderr || ''}`.trim(), error: r.error };
}
function run(cmd, args, opts = {}) {
  say(`   > ${cmd} ${args.map((a) => (/\s/.test(a) ? `"${a}"` : a)).join(' ')}`);
  const r = cp.spawnSync(cmd, args, { stdio: 'inherit', cwd: REPO, windowsHide: true, ...opts });
  return r.status === null ? 1 : r.status;
}
const exists = (p) => { try { fs.accessSync(p); return true; } catch { return false; } };
const isDir = (p) => { try { return fs.statSync(p).isDirectory(); } catch { return false; } };

// ---------------------------------------------------------------- (Linux) helpers
// Linux runs the setup for the Windows games installed through Steam's Proton (or Heroic / Lutris): its file system is
// case-sensitive, so every folder the setup looks for in a game install is found whatever its case ("steamapps" or
// "SteamApps"); on Windows nothing changes. The tool names are the Linux ones (no .exe).
const HOME = process.env.HOME || require('os').homedir();
// `base` + the segments, each segment matched without regard to case when the exact name is not there (Linux)
function ciJoin(base, ...segs) {
  if (isWin) return path.join(base, ...segs);
  let p = base;
  for (const s of segs) {
    const exact = path.join(p, s);
    if (exists(exact)) { p = exact; continue; }
    let hit = null;
    try { hit = fs.readdirSync(p).find((f) => f.toLowerCase() === s.toLowerCase()); } catch { /* not a folder */ }
    p = path.join(p, hit || s);
  }
  return p;
}
// a program on PATH (Linux: what `command -v` finds)
function onPath(name) {
  for (const d of (process.env.PATH || '').split(path.delimiter).filter(Boolean)) { const f = path.join(d, name); try { fs.accessSync(f, fs.constants.X_OK); if (fs.statSync(f).isFile()) return f; } catch { /* not here */ } }
  return null;
}
// (Linux) a PE file's version (VS_FIXEDFILEINFO of its RT_VERSION resource): what PowerShell's VersionInfo gives on Windows
function peVersion(file) {
  try {
    const b = fs.readFileSync(file);
    const pe = b.readUInt32LE(0x3c), opt = pe + 24, nsec = b.readUInt16LE(pe + 6), sec0 = opt + b.readUInt16LE(pe + 20);
    const rsrc = b.readUInt32LE(opt + (b.readUInt16LE(opt) === 0x20b ? 112 : 96) + 2 * 8);
    let base = -1, rva0 = 0;
    for (let k = 0; k < nsec; k++) { const s = sec0 + k * 40, va = b.readUInt32LE(s + 12), sz = Math.max(b.readUInt32LE(s + 8), b.readUInt32LE(s + 16)); if (rsrc >= va && rsrc < va + sz) { base = b.readUInt32LE(s + 20) + (rsrc - va); rva0 = va - b.readUInt32LE(s + 20); } }
    if (base < 0) return null;
    // the resource tree: type 16 (RT_VERSION) -> its first name -> its first language -> the data
    const entry = (dir, id) => { const n = b.readUInt16LE(dir + 12) + b.readUInt16LE(dir + 14); for (let i = 0; i < n; i++) { const e = dir + 16 + i * 8; if (id === null || b.readUInt32LE(e) === id) return b.readUInt32LE(e + 4); } return null; };
    let off = entry(base, 16); if (off === null) return null;
    off = entry(base + (off & 0x7fffffff), null); if (off === null) return null;
    off = entry(base + (off & 0x7fffffff), null); if (off === null) return null;
    const data = b.readUInt32LE(base + off) - rva0;
    const sig = b.indexOf(Buffer.from([0xbd, 0x04, 0xef, 0xfe]), data);
    if (sig < 0) return null;
    const ms = b.readUInt32LE(sig + 8), ls = b.readUInt32LE(sig + 12);
    return `${ms >>> 16}.${ms & 0xffff}.${ls >>> 16}.${ls & 0xffff}`;
  } catch { return null; }
}
// (Linux) the folder picker: zenity (GNOME, Steam Deck desktop mode) or kdialog (KDE), when there is a desktop
function linuxPicker() {
  if (isWin || !(process.env.DISPLAY || process.env.WAYLAND_DISPLAY)) return null;
  if (onPath('zenity')) return 'zenity';
  if (onPath('kdialog')) return 'kdialog';
  return null;
}
// (Windows setup under Proton / Wine) Wine runs it: Wine's own environment variables, else its registry key. On Windows
// neither is there and nothing changes.
const underWine = isWin && (!!(process.env.WINEPREFIX || process.env.WINELOADER || process.env.WINEDLLPATH || process.env.WINEDLLOVERRIDES) || (() => { try { return capture('reg', ['query', 'HKCU\\Software\\Wine']).status === 0; } catch { return false; } })());
// (under Wine) the Linux Steam folders and their libraries through Wine's Z: drive (Z: is the Linux root): the Unix HOME
// that Wine passes on, the native, Flatpak and Snap Steam folders, and every library of their libraryfolders.vdf
// (Linux paths there, e.g. a Steam Deck's SD card under /run/media)
function wineSteamLibraries() {
  const home = process.env.HOME && process.env.HOME.startsWith('/') ? process.env.HOME : (process.env.USER ? `/home/${process.env.USER}` : null);
  if (!home) return [];
  const z = (p) => 'Z:' + p.split('/').join('\\');
  const libs = new Set();
  for (const r of ['.steam/steam', '.steam/root', '.local/share/Steam', '.var/app/com.valvesoftware.Steam/.local/share/Steam', '.var/app/com.valvesoftware.Steam/data/Steam', 'snap/steam/common/.local/share/Steam']) {
    const root = z(`${home}/${r}`);
    if (!isDir(path.join(root, 'steamapps'))) continue;
    libs.add(root);
    const vdf = path.join(root, 'steamapps', 'libraryfolders.vdf');
    if (exists(vdf)) for (const m of fs.readFileSync(vdf, 'utf8').matchAll(/"path"\s+"([^"]+)"/g)) { const p = m[1].replace(/\\\\/g, '\\'); libs.add(p.startsWith('/') ? z(p) : p); }
  }
  // (one library seen through two Steam folders: once)
  const seen = new Set();
  return [...libs].filter(isDir).filter((l) => { let k; try { k = fs.realpathSync(l).toLowerCase(); } catch { k = l.toLowerCase(); } if (seen.has(k)) return false; seen.add(k); return true; });
}
const WINE_HINT = 'Running under Proton/Wine? Use the Linux version of the setup instead (the linux-x64 download, ./setup-linux: README, "Linux / Steam Deck"). The Windows setup needs Windows\' own PowerShell and DirectX shader tools to build the mod\'s files, which Proton and Wine do not have.';
// (Linux) the setup's own tools: tools/bin in a release download, the cargo build in a source checkout
const linuxTool = (name) => [path.join(REPO, 'tools', 'bin', name), path.join(REPO, 'target', 'release', name)].find(exists) || null;

let rl = null;
function ask(question) {
  if (opt.yes) return Promise.resolve('');
  rl = rl || readline.createInterface({ input: process.stdin, output: process.stdout });
  return new Promise((res) => rl.question(`   ? ${question} `, (a) => res(a.trim().replace(/^"|"$/g, ''))));
}
function browse(title) {
  if (!isWin) {
    const picker = linuxPicker();
    const r = picker === 'zenity' ? capture('zenity', ['--file-selection', '--directory', `--title=${title}`]) : picker === 'kdialog' ? capture('kdialog', ['--getexistingdirectory', HOME, '--title', title]) : null;
    return r && r.status === 0 ? r.out.split(/\r?\n/).pop().trim() : '';
  }
  const ps = `Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.FolderBrowserDialog; $d.Description = '${title.replace(/'/g, "''")}'; if ($d.ShowDialog() -eq 'OK') { $d.SelectedPath }`;
  return capture('powershell', ['-NoProfile', '-STA', '-Command', ps]).out.split(/\r?\n/).pop() || '';
}
// A game that was not found: the folder picker (with --browse, or when started from setup\setup.bat, which sets
// KH_SETUP_PICKER=1), else a typed path. A cancelled picker falls back to the typed path.
async function askFolder(what, check) {
  const picker = (isWin || linuxPicker()) && !opt.yes && (opt.browse || process.env.KH_SETUP_PICKER === '1');
  const flag = /KINGDOM/.test(what) ? '--kh3' : '--er';
  for (let tries = 0; tries < 3; tries++) {
    let dir = '';
    if (picker) { say(`   a folder picker is open: select the ${what} install folder`); dir = browse(`Select the ${what} install folder`); }
    if (!dir) dir = await ask(`Where is ${what} installed? Paste the folder (empty = stop; or run again with ${flag} "<folder>"):`);
    if (!dir) return null;
    const found = check(dir);
    if (found) return found;
    bad(`${dir}: not a ${what} install (see README, "Finding the games")`);
  }
  return null;
}

// ---------------------------------------------------------------- finding the games
// (Linux) Steam's folders: the native client (~/.steam/steam, ~/.local/share/Steam), the Flatpak and the Snap; each once
function linuxSteamRoots() {
  const out = new Map();
  for (const c of [path.join(HOME, '.steam', 'steam'), path.join(HOME, '.steam', 'root'), path.join(HOME, '.local', 'share', 'Steam'), path.join(HOME, '.var', 'app', 'com.valvesoftware.Steam', '.local', 'share', 'Steam'), path.join(HOME, '.var', 'app', 'com.valvesoftware.Steam', 'data', 'Steam'), path.join(HOME, 'snap', 'steam', 'common', '.local', 'share', 'Steam')]) {
    let real; try { real = fs.realpathSync(c); } catch { continue; }
    if (isDir(ciJoin(real, 'steamapps')) && !out.has(real)) out.set(real, real);
  }
  return [...out.keys()];
}
function steamRoot() {
  if (!isWin) return linuxSteamRoots()[0] || null;
  for (const [key, value] of [['HKCU\\Software\\Valve\\Steam', 'SteamPath'], ['HKLM\\SOFTWARE\\WOW6432Node\\Valve\\Steam', 'InstallPath'], ['HKLM\\SOFTWARE\\Valve\\Steam', 'InstallPath']]) {
    const r = capture('reg', ['query', key, '/v', value]);
    const m = r.status === 0 && r.out.match(new RegExp(`${value}\\s+REG_\\w+\\s+(.+)`));
    if (m && isDir(m[1].trim())) return path.normalize(m[1].trim());
  }
  return null;
}
function steamLibraries() {
  if (!isWin) {
    // (Linux) every Steam folder's libraries (libraryfolders.vdf: the paths are the Linux ones), each once
    const libs = new Set();
    for (const root of linuxSteamRoots()) {
      libs.add(root);
      const vdf = ciJoin(root, 'steamapps', 'libraryfolders.vdf');
      if (exists(vdf)) for (const m of fs.readFileSync(vdf, 'utf8').matchAll(/"path"\s+"([^"]+)"/g)) { try { libs.add(fs.realpathSync(m[1].replace(/\\\\/g, '\\'))); } catch { /* a library that is not mounted */ } }
    }
    return [...libs].filter(isDir);
  }
  const root = steamRoot();
  if (underWine) { const w = wineSteamLibraries(); if (w.length) return w; }
  if (!root) return [];
  const libs = new Set([root]);
  const vdf = path.join(root, 'steamapps', 'libraryfolders.vdf');
  if (exists(vdf)) for (const m of fs.readFileSync(vdf, 'utf8').matchAll(/"path"\s+"([^"]+)"/g)) libs.add(path.normalize(m[1].replace(/\\\\/g, '\\')));
  return [...libs].filter(isDir);
}
// Every installed Steam app: { appid, name, dir }
function steamApps() {
  const apps = [];
  for (const lib of steamLibraries()) {
    const sa = ciJoin(lib, 'steamapps');
    let names = [];
    try { names = fs.readdirSync(sa).filter((f) => /^appmanifest_\d+\.acf$/.test(f)); } catch { continue; }
    for (const f of names) {
      const t = fs.readFileSync(path.join(sa, f), 'utf8');
      const get = (k) => (t.match(new RegExp(`"${k}"\\s+"([^"]*)"`)) || [])[1];
      const dir = get('installdir') && ciJoin(sa, 'common', get('installdir'));
      if (dir && isDir(dir)) apps.push({ appid: get('appid'), name: get('name') || '', dir });
    }
  }
  return apps;
}
// (Linux) the Epic games Heroic (or legendary) installed, and those of an Epic launcher in a Wine prefix (Lutris'
// "epic-games-store", Heroic's own prefixes): { name, dir }
function linuxEpicApps() {
  const out = [];
  const XDG = process.env.XDG_CONFIG_HOME || path.join(HOME, '.config');
  for (const f of [path.join(XDG, 'heroic', 'legendaryConfig', 'legendary', 'installed.json'), path.join(HOME, '.var', 'app', 'com.heroicgameslauncher.hgl', 'config', 'heroic', 'legendaryConfig', 'legendary', 'installed.json'), path.join(XDG, 'legendary', 'installed.json')]) {
    try { for (const g of Object.values(JSON.parse(fs.readFileSync(f, 'utf8')))) if (g && g.install_path && isDir(g.install_path)) out.push({ name: g.title || g.app_name || '', dir: g.install_path }); } catch { /* not installed */ }
  }
  // Wine prefixes: Lutris' game files name theirs (prefix: ...), plus the usual folders
  const prefixes = new Set([path.join(HOME, 'Games', 'epic-games-store'), path.join(HOME, 'Games', 'Heroic', 'Prefixes', 'default'), path.join(HOME, '.wine')]);
  for (const d of [path.join(XDG, 'lutris', 'games'), path.join(HOME, '.local', 'share', 'lutris', 'games')]) {
    let names = []; try { names = fs.readdirSync(d).filter((f) => f.endsWith('.yml')); } catch { continue; }
    for (const f of names) { const m = fs.readFileSync(path.join(d, f), 'utf8').match(/^\s*prefix:\s*['"]?([^'"\r\n]+)/m); if (m) prefixes.add(m[1].trim().replace(/^~(?=\/)/, HOME)); }
  }
  const seen = new Set();
  for (const pfx of prefixes) {
    let real; try { real = fs.realpathSync(pfx); } catch { continue; }
    if (seen.has(real)) continue;
    seen.add(real);
    const man = ciJoin(real, 'drive_c', 'ProgramData', 'Epic', 'EpicGamesLauncher', 'Data', 'Manifests');
    let items = []; try { items = fs.readdirSync(man).filter((f) => f.endsWith('.item')); } catch { continue; }
    for (const f of items) {
      try {
        const j = JSON.parse(fs.readFileSync(path.join(man, f), 'utf8'));
        const m = j.InstallLocation && /^([a-zA-Z]):[\\/](.*)$/.exec(j.InstallLocation);
        const dir = m && (m[1].toLowerCase() === 'c' ? ciJoin(real, 'drive_c', ...m[2].split(/[\\/]+/).filter(Boolean)) : null);
        if (dir && isDir(dir)) out.push({ name: j.DisplayName || '', dir });
      } catch { /* a broken manifest */ }
    }
  }
  return out;
}
// Every installed Epic game: { name, dir }
function epicApps() {
  if (!isWin) return linuxEpicApps();
  const dir = 'C:\\ProgramData\\Epic\\EpicGamesLauncher\\Data\\Manifests';
  const out = [];
  try {
    for (const f of fs.readdirSync(dir).filter((f) => f.endsWith('.item'))) {
      try { const j = JSON.parse(fs.readFileSync(path.join(dir, f), 'utf8')); if (j.InstallLocation && isDir(j.InstallLocation)) out.push({ name: j.DisplayName || '', dir: j.InstallLocation }); } catch { /* a broken manifest */ }
    }
  } catch { /* no Epic launcher */ }
  return out;
}

// A KH3 install: returns { gameRoot, mainExe, paksDir } or null. Accepts the install folder or its inner "KINGDOM HEARTS III".
function kh3At(dir) {
  if (!dir) return null;
  dir = path.resolve(dir);
  for (const root of [dir, path.dirname(dir)]) {
    const inner = ciJoin(root, 'KINGDOM HEARTS III');
    const bin = ciJoin(inner, 'Binaries', 'Win64');
    const paks = ciJoin(inner, 'Content', 'Paks');
    if (!isDir(bin) || !isDir(paks)) continue;
    const exe = fs.readdirSync(bin).find((f) => /^KINGDOM HEARTS III.*\.exe$/i.test(f) && !/shipping-cmd/i.test(f));
    if (!exe || !fs.readdirSync(paks).some((f) => f.toLowerCase().endsWith('.pak'))) continue;
    // (Linux: the folders as they are spelled on disk)
    const rel = (p) => path.relative(root, p).split(path.sep).join('/');
    if (!isWin) return { gameRoot: root.split(path.sep).join('/'), mainExe: `${rel(bin)}/${exe}`, paksDir: rel(paks) };
    return { gameRoot: root.split(path.sep).join('/'), mainExe: `KINGDOM HEARTS III/Binaries/Win64/${exe}`, paksDir: 'KINGDOM HEARTS III/Content/Paks' };
  }
  return null;
}
// An Elden Ring install: returns the path of eldenring.exe or null (the install folder, its Game folder or the exe itself).
function erAt(dir) {
  if (!dir) return null;
  dir = path.resolve(dir);
  for (const exe of [ciJoin(dir, 'Game', 'eldenring.exe'), ciJoin(dir, 'eldenring.exe'), dir]) if (/eldenring\.exe$/i.test(exe) && exists(exe) && (isWin || !isDir(exe))) return exe;
  return null;
}
function fileVersion(exe) {
  if (!isWin || underWine) return peVersion(exe);
  const r = capture('powershell', ['-NoProfile', '-Command', `(Get-Item -LiteralPath '${exe.replace(/'/g, "''")}').VersionInfo.FileVersion`]);
  return r.status === 0 ? r.out.split(/\r?\n/).pop().trim() : null;
}

// ---------------------------------------------------------------- me3
function findMe3() {
  if (!isWin) {
    // (Linux) me3's installer puts it in ~/.local/bin; its portable download unpacks to <folder>/bin/me3
    const xdgData = process.env.XDG_DATA_HOME || path.join(HOME, '.local', 'share');
    for (const c of [opt.me3, path.join(HOME, '.local', 'bin', 'me3'), path.join(xdgData, 'me3', 'bin', 'me3'), path.join(HOME, 'me3', 'bin', 'me3')].filter(Boolean)) if (exists(c) && !isDir(c)) return path.resolve(c);
    return onPath('me3');
  }
  const local = process.env.LOCALAPPDATA || '', pf = process.env.ProgramFiles || 'C:\\Program Files';
  const candidates = [opt.me3, path.join(local, 'Programs', 'me3', 'bin', 'me3.exe'), path.join(local, 'me3', 'bin', 'me3.exe'), path.join(pf, 'me3', 'bin', 'me3.exe')].filter(Boolean);
  for (const c of candidates) if (exists(c)) return c;
  const w = capture('where', ['me3']);
  if (w.status === 0) return w.out.split(/\r?\n/)[0].trim();
  return null;
}

// ---------------------------------------------------------------- the pack
// A pack is complete when the pack tools, the replay, the shaders and the generated data have all run (packgen writes
// build/packgen/done.json at the end of a whole build).
const PACK_REQUIRED = ['manifest.json', 'sora.glb', 'materials/er/skin.vs.dxil', 'vfx/er/shaders.json', 'ui/manifest.json', 'audio/manifest.json', 'party/n_ex001/manifest.json', 'party/n_ex002/manifest.json'];
// (written when tools/packgen.js has made the whole pack: a pack build that stopped half way is built again)
const PACK_DONE = path.join(REPO, 'build', 'packgen', 'pack_done.json');

// (PACKUPDATE, the user 2026-10-27: "does the setup tool have to regenerate things that have already been generated ...
// can it only regenerate things that have changed") THE PACK'S INPUT FINGERPRINTS, written after every pack build: the
// shaders' (kh-shaderc, which carries our shader code, and the shader recipe and its scripts) and the rest's (every other
// shipped pack tool, the list of KH3 files and PACK_EPOCH). The design sheets are left out: most of sheets/port is read by
// the DLL at run time, and a gameplay number must not rebuild the pack. A RELEASE THAT CHANGES A SHEET THE PACK IS MADE
// FROM BUMPS PACK_EPOCH (docs/BUILDING.md, the release checklist). An update whose shaders' fingerprint differs re-runs only
// packgen's `shaders` phase (minutes, from the files the first build made); any other difference builds the pack again.
// A pack built before the fingerprints existed: its shaders are made again (the update that brought them changed only
// those), then the fingerprints are written.
const PACK_PRINTS = path.join(REPO, 'build', 'packgen', 'pack_inputs_fp.json');
const PACK_EPOCH = 1;
const SHADER_INPUTS = ['tools/packgen/recipes/shaders.json', 'tools/packgen_shaders.js', 'tools/packgen_overlays.js'];
function listFiles(rel) {
  const out = [];
  const walk = (r) => {
    let items = [];
    try { items = fs.readdirSync(path.join(REPO, r), { withFileTypes: true }); } catch { return; }
    for (const e of items) {
      const p = `${r}/${e.name}`;
      if (e.isDirectory()) walk(p);
      else if (e.isFile()) out.push(p);
    }
  };
  walk(rel);
  return out.sort();
}
function hashFiles(files, extra = '') {
  const h = crypto.createHash('sha256').update(extra);
  for (const f of [...files].sort()) {
    h.update(`${f}\0`);
    try { h.update(fs.readFileSync(path.join(REPO, f))); } catch { h.update('(missing)'); }
    h.update('\0');
  }
  return h.digest('hex');
}
function packPrints(shaderc) {
  const exe = shaderc && exists(shaderc) ? crypto.createHash('sha256').update(fs.readFileSync(shaderc)).digest('hex') : 'none';
  const rest = [...listFiles('tools').filter((f) => !SHADER_INPUTS.includes(f) && !f.startsWith('tools/bin/') && !f.startsWith('tools/third_party/') && f !== 'tools/config.json' /* (the setup's own: written by it, copied from npm) */), 'setup/pack_inputs.json'];
  return { format: 1, shaders: hashFiles(SHADER_INPUTS, `kh-shaderc ${exe}\0`), rest: hashFiles(rest, `epoch ${PACK_EPOCH}\0`) };
}
function readPrints() {
  try { return JSON.parse(fs.readFileSync(PACK_PRINTS, 'utf8')); } catch { return null; }
}
const packMissing = (dir) => PACK_REQUIRED.filter((f) => !exists(path.join(dir, f)));
function findDxc() {
  if (opt.dxc) return exists(opt.dxc) ? path.resolve(opt.dxc) : null;
  if (!isWin) { const own = path.join(DXC_DIR, 'bin', 'dxc'); return exists(own) && exists(path.join(DXC_DIR, 'lib', 'libdxcompiler.so')) ? own : null; }
  const kits = 'C:\\Program Files (x86)\\Windows Kits\\10\\bin';
  const sdk = isDir(kits) && fs.readdirSync(kits).map((v) => path.join(kits, v, 'x64', 'dxc.exe')).filter(exists).sort().pop();
  if (sdk) return sdk;
  const own = path.join(DXC_DIR, 'bin', 'x64', 'dxc.exe');
  return exists(own) ? own : null;
}
function downloadDxc() {
  const zip = path.join(REPO, 'tools', 'bin', 'dxc.zip');
  fs.mkdirSync(path.dirname(zip), { recursive: true });
  say(`   downloading Microsoft's DirectX shader compiler: ${DXC_URL}`);
  const q = (s) => s.replace(/'/g, "''");
  const dl = isWin ? capture('powershell', ['-NoProfile', '-Command', `$ProgressPreference='SilentlyContinue'; Invoke-WebRequest -UseBasicParsing -Uri '${DXC_URL}' -OutFile '${q(zip)}'`]) : capture('curl', ['-sSL', '-o', zip, DXC_URL]);
  const got = exists(zip) ? crypto.createHash('sha256').update(fs.readFileSync(zip)).digest('hex') : null;
  if (dl.status !== 0 || got !== DXC_SHA256) { fs.rmSync(zip, { force: true }); warn(`the download failed or is not the expected file (${got ? 'SHA-256 ' + got : dl.out.slice(0, 200)})`); return null; }
  fs.rmSync(DXC_DIR, { recursive: true, force: true }); fs.mkdirSync(DXC_DIR, { recursive: true });
  const ex = isWin ? capture('powershell', ['-NoProfile', '-Command', `Expand-Archive -LiteralPath '${q(zip)}' -DestinationPath '${q(DXC_DIR)}' -Force`]) : capture('unzip', ['-q', '-o', zip, '-d', DXC_DIR]);
  fs.rmSync(zip, { force: true });
  const exe = path.join(DXC_DIR, 'bin', 'x64', 'dxc.exe');
  return ex.status === 0 && exists(exe) ? exe : null;
}
// (Linux) the same release's Linux build: downloaded with Node's own fetch, checked, unpacked (setup/tar.js) into tools/bin/dxc
async function downloadDxcLinux() {
  const tgz = path.join(REPO, 'tools', 'bin', 'dxc.tar.gz');
  fs.mkdirSync(path.dirname(tgz), { recursive: true });
  say(`   downloading Microsoft's DirectX shader compiler (Linux build): ${DXC_LINUX_URL}`);
  let got = null, why = '';
  try {
    const r = await fetch(DXC_LINUX_URL, { redirect: 'follow' });
    if (!r.ok) throw new Error(`HTTP ${r.status}`);
    fs.writeFileSync(tgz, Buffer.from(await r.arrayBuffer()));
    got = crypto.createHash('sha256').update(fs.readFileSync(tgz)).digest('hex');
  } catch (e) { why = e.message; }
  if (got !== DXC_LINUX_SHA256) { fs.rmSync(tgz, { force: true }); warn(`the download failed or is not the expected file (${got ? 'SHA-256 ' + got : why})`); return null; }
  fs.rmSync(DXC_DIR, { recursive: true, force: true }); fs.mkdirSync(DXC_DIR, { recursive: true });
  let unpacked = false;
  try { require(path.join(__dirname, 'tar.js')).extract(fs.readFileSync(tgz), DXC_DIR); unpacked = true; } catch (e) { warn(`the download could not be unpacked: ${e.message}`); }
  fs.rmSync(tgz, { force: true });
  const exe = path.join(DXC_DIR, 'bin', 'dxc');
  return unpacked && exists(exe) && exists(path.join(DXC_DIR, 'lib', 'libdxcompiler.so')) ? exe : null;
}

// ================================================================ the steps
(async function main() {
  say('Kingdom Hearts x Elden Ring: setup');
  say(`repository: ${REPO}`);
  // (Linux) every Node process of the run lists folders in the order Windows does (tools/lib/ntfs_order.js), so the pack's
  // tools make the same files as on Windows
  if (!isWin) {
    const order = path.join(REPO, 'tools', 'lib', 'ntfs_order.js');
    if (exists(order)) { require(order); if (!(process.env.NODE_OPTIONS || '').includes('ntfs_order.js')) process.env.NODE_OPTIONS = [process.env.NODE_OPTIONS, `--require "${order.replace(/[\\"]/g, (c) => '\\' + c)}"`].filter(Boolean).join(' '); }
  }

  // ---- 1. prerequisites
  step(1, 'prerequisites');
  if (underWine) warn(WINE_HINT);
  const nodeMajor = Number(process.versions.node.split('.')[0]);
  const bundledNode = isWin ? path.join(REPO, 'tools', 'bin', 'node', 'node.exe') : path.join(REPO, 'tools', 'bin', 'node', 'bin', 'node');
  const inSea = (() => { try { return require('node:sea').isSea(); } catch { return false; } })();
  const where = inSea ? (isWin ? ' (built into setup.exe)' : ' (built into setup)') : path.resolve(process.execPath).toLowerCase() === bundledNode.toLowerCase() ? ' (the portable Node in tools/bin/node)' : ` (${process.execPath})`;
  if (nodeMajor >= 20) ok(`Node ${process.versions.node}${where}`);
  else stop(1, `Node 20 or newer is needed (found ${process.versions.node}): start the setup with ${isWin ? 'setup.exe or setup\\setup.bat' : './setup or setup/setup.sh'}, which bring their own Node`);
  // the npm packages: bundled in a release download (node_modules, the versions of package-lock.json); a source checkout
  // installs the locked versions once (npm ci; setup\setup.bat does it before this)
  if (!['ajv', 'iced-x86', '@wasm-audio-decoders/ogg-vorbis', 'gltf-validator'].every((m) => exists(path.join(REPO, 'node_modules', ...m.split('/'), 'package.json')))) {
    if (opt['no-download']) stop(1, 'the npm packages (node_modules) are missing and --no-download was given: use a release download, or run without --no-download');
    const ownNpm = path.join(REPO, 'tools', 'bin', 'node', 'npm.cmd');
    const linuxNpm = path.join(REPO, 'tools', 'bin', 'node', 'bin', 'npm');
    const npm = isWin ? (exists(ownNpm) ? ownNpm : 'npm.cmd') : exists(linuxNpm) ? linuxNpm : 'npm';
    say('   npm packages missing: installing the locked versions (npm ci)');
    if (run(isWin ? `"${npm}"` : npm, ['ci', '--omit=dev', '--no-fund', '--no-audit'], { shell: isWin })) stop(1, 'npm ci failed');
  }
  ok('npm packages (node_modules)');
  // the Ogg Vorbis decoder the audio tools load from tools/third_party (from npm: it is not stored in the repository)
  const oggSrc = path.join(REPO, 'node_modules', '@wasm-audio-decoders', 'ogg-vorbis', 'dist', 'ogg-vorbis-decoder.min.js');
  const oggDst = path.join(REPO, 'tools', 'third_party', 'ogg-vorbis-decoder', 'ogg-vorbis-decoder.min.js');
  if (exists(oggSrc) && !exists(oggDst)) fs.copyFileSync(oggSrc, oggDst);
  if (exists(oggDst)) ok('Ogg Vorbis decoder (npm @wasm-audio-decoders/ogg-vorbis)'); else warn('the Ogg Vorbis decoder was not found in node_modules: the audio pack tools will not run');
  // a release download carries the DLLs, their table list and the shader compiler kh-shaderc prebuilt
  // (Linux: the Linux builds of kh-shaderc and of the setup's two helpers, kh-oodle and kh-er-selfcheck; the DLLs are the
  // same Windows DLLs, which run inside Proton)
  const prebuilt = ['kh_er.dll', 'kh_er_loader.dll', 'kh_er.schemas.json'].every((d) => exists(path.join(MODS, d))) && (isWin ? exists(path.join(REPO, 'tools', 'bin', 'kh-shaderc.exe')) : ['kh-shaderc', 'kh-oodle', 'kh-er-selfcheck'].every((t) => exists(path.join(REPO, 'tools', 'bin', t))));
  const fromSource = !!opt['from-source'] || !prebuilt;
  const cargo = capture('cargo', ['--version']);
  const haveCargo = cargo.status === 0;
  if (!fromSource) ok('prebuilt DLLs and shader compiler (a release download): Rust is not needed');
  else if (haveCargo) ok(cargo.out);
  else if (!opt['skip-build']) stop(1, isWin ? ['Rust is needed to build the mod from source: https://rustup.rs (stable, x86_64-pc-windows-msvc, with the "Desktop development with C++" Visual Studio Build Tools).', 'Or use a release download (the DLLs come prebuilt).'] : ['Rust is needed to build the mod from source: https://rustup.rs (stable), with cargo-xwin for the Windows DLLs (docs/BUILDING.md, "Linux").', 'Or use the linux-x64 release download (the DLLs and the tools come prebuilt).']);
  else warn('cargo not found (--skip-build)');
  // (Linux, from source) the DLLs are Windows DLLs: cargo-xwin cross-compiles them (x86_64-pc-windows-msvc)
  if (!isWin && fromSource && haveCargo && !opt['skip-build']) {
    if (capture('cargo', ['xwin', '--version']).status !== 0) stop(1, ['On Linux the Windows DLLs are built with cargo-xwin: cargo install cargo-xwin, then rustup target add x86_64-pc-windows-msvc (docs/BUILDING.md, "Linux").', 'Or use the linux-x64 release download (the DLLs come prebuilt).']);
    ok('cargo-xwin (the Windows DLLs, x86_64-pc-windows-msvc)');
  }
  let dxc = findDxc();
  if (!dxc && !opt['no-download']) dxc = isWin ? downloadDxc() : await downloadDxcLinux();
  if (dxc) ok(`dxc ${dxc} (the pack's shaders)`); else warn(isWin ? 'no DirectX shader compiler: the pack cannot be built. Install the Windows 10/11 SDK, allow the download (no --no-download) or give --dxc FILE' : 'no DirectX shader compiler: the pack cannot be built. Allow the download (no --no-download) or give --dxc FILE (the Linux dxc of DirectXShaderCompiler v1.8.2502, next to its lib/libdxcompiler.so)');
  const me3 = findMe3();
  if (me3) {
    const v = capture(me3, ['--version']).out;
    const ver = (v.match(/\d+\.\d+\.\d+/) || [])[0];
    ok(`me3 ${ver || '(version unknown)'} ${me3}`);
    try { fs.mkdirSync(ER_DIR, { recursive: true }); fs.writeFileSync(path.join(ER_DIR, 'me3_path.txt'), me3 + (isWin ? '\r\n' : '\n')); } catch {} // er\play.bat (Linux: er/play.sh) starts this me3
    const older = ver && ver.split('.').map(Number).reduce((d, n, i) => d || n - TESTED_ME3.split('.').map(Number)[i], 0) < 0;
    if (!ver || older) warn(`me3 ${ver || '(version unknown)'} is older than ${TESTED_ME3}: an older me3 can ignore the mod's own save file and start Elden Ring on your NORMAL saves (ER0000.sl2). Update me3 (https://github.com/garyttierney/me3/releases) before playing: ${isWin ? 'er\\play.bat' : 'er/play.sh'} refuses an older me3.`);
    else if (ver !== TESTED_ME3) warn(`the mod was tested with me3 ${TESTED_ME3}; ${ver} may work`);
  } else warn(`me3 not found (Mod Engine 3, tested ${TESTED_ME3}: https://github.com/garyttierney/me3/releases). Install it before launching, or give --me3 FILE.`);

  // ---- 2. the games
  step(2, 'finding the games');
  const steam = steamApps(), epic = epicApps();
  say(`   Steam libraries: ${steamLibraries().length}, Steam apps: ${steam.length}, Epic games: ${epic.length}`);
  let kh3 = kh3At(opt.kh3);
  if (opt.kh3 && !kh3) stop(2, `--kh3 ${opt.kh3}: not a KINGDOM HEARTS III install (it must contain "KINGDOM HEARTS III\\Binaries\\Win64" and "KINGDOM HEARTS III\\Content\\Paks")`);
  if (!kh3) for (const a of steam.filter((a) => /KINGDOM HEARTS\s*(III|Ⅲ|3)/i.test(a.name) || /KINGDOM HEARTS III/i.test(a.dir))) if ((kh3 = kh3At(a.dir))) { ok(`KH3 (Steam, app ${a.appid}): ${a.dir}`); break; }
  if (!kh3) for (const a of epic.filter((a) => /KINGDOM HEARTS\s*(III|Ⅲ|3)/i.test(a.name))) if ((kh3 = kh3At(a.dir))) { ok(`KH3 (Epic): ${a.dir}`); break; }
  if (!kh3) { warn('KINGDOM HEARTS III was not found in Steam or Epic'); if (underWine) warn(WINE_HINT); kh3 = await askFolder('KINGDOM HEARTS III', kh3At); }
  if (!kh3) stop(1, 'KINGDOM HEARTS III is needed (your own PC copy). Run again with --kh3 "<install folder>".');
  ok(`KH3 executable: ${kh3.gameRoot}/${kh3.mainExe}`);

  let erExe = erAt(opt.er);
  if (opt.er && !erExe) stop(2, `--er ${opt.er}: no Game\\eldenring.exe there`);
  if (!erExe) for (const a of steam.filter((a) => a.appid === ER_APPID || /ELDEN RING/i.test(a.name))) if ((erExe = erAt(a.dir))) break;
  if (!erExe) { warn('ELDEN RING was not found in Steam'); erExe = await askFolder('ELDEN RING', erAt); }
  if (!erExe) stop(1, 'ELDEN RING is needed (your own Steam copy). Run again with --er "<install folder>".');
  const erVer = fileVersion(erExe);
  if (ER_VERSIONS.includes(erVer)) ok(`Elden Ring ${erVer}: ${erExe}`);
  else warn(`Elden Ring ${erVer || '(version unknown)'} at ${erExe}: the mod supports ${ER_VERSIONS.join(' and ')} only; on another version the loader does nothing`);

  // tools/config.json: where the KH3 tools read the game (machine-specific, git-ignored)
  fs.writeFileSync(path.join(REPO, 'tools', 'config.json'), JSON.stringify(kh3, null, 2) + '\n');
  ok('tools/config.json written');

  // ---- 3. the KH3 pak key
  step(3, 'the KH3 pak key (read from your own executable)');
  fs.mkdirSync(path.join(REPO, 'audit', 'triage'), { recursive: true });
  const keyFile = path.join(REPO, 'audit', 'triage', 'pak_key.json');
  const keyInfo = () => { try { return JSON.parse(fs.readFileSync(keyFile, 'utf8')); } catch { return null; } };
  let key = keyInfo();
  const exeNow = path.join(kh3.gameRoot, kh3.mainExe);
  const stampFile = path.join(REPO, 'audit', 'triage', 'setup_exe.json');
  const stamp = JSON.stringify({ exe: exeNow, size: fs.statSync(exeNow).size, mtime: fs.statSync(exeNow).mtimeMs });
  if (!key || !key.key_file_offset || !exists(stampFile) || fs.readFileSync(stampFile, 'utf8') !== stamp) {
    if (run(process.execPath, ['tools/pak_footers.js'], { stdio: ['ignore', 'ignore', 'inherit'] })) stop(1, 'tools/pak_footers.js failed: are the .pak files readable?');
    say('   scanning the executable for the key (a minute or two)');
    if (run(process.execPath, ['tools/pak_key.js'], { stdio: ['ignore', 'ignore', 'inherit'] })) stop(1, 'tools/pak_key.js failed');
    key = keyInfo();
    fs.writeFileSync(stampFile, stamp);
  }
  if (!key || !key.key_file_offset) stop(1, 'no pak key was found in the executable: is this the PC release of KINGDOM HEARTS III + Re Mind?');
  ok(`key found at file offset ${key.key_file_offset}; ${key.verified_paks} of ${key.total_paks} pak indexes open with it (only the offset and a hash are kept)`);
  if (key.verified_paks < key.total_paks) warn(`${key.total_paks - key.verified_paks} pak(s) did not open: ${(key.failures || []).join(', ')}`);

  // ---- 4. the asset pack
  step(4, 'the asset pack (made from your KH3 files on this PC; never distributed)');
  const packDir = opt.pack ? path.resolve(opt.pack) : path.join(REPO, 'build', 'sora_pack');
  const erGame = path.dirname(erExe);
  const workers = opt.workers ? ['--workers', String(opt.workers)] : [];
  const packEnv = { ...process.env, KH_ER_GAME: erGame, ...(dxc ? { KH_DXC: dxc } : {}), ...(!fromSource ? { KH_SHADERC: path.join(REPO, 'tools', 'bin', 'kh-shaderc.exe') } : {}) };
  // (Linux) the Linux kh-shaderc, and kh-oodle for Elden Ring's icon archives (tools/er_icons.js)
  if (!isWin && !fromSource) Object.assign(packEnv, { KH_SHADERC: path.join(REPO, 'tools', 'bin', 'kh-shaderc'), KH_OODLE: path.join(REPO, 'tools', 'bin', 'kh-oodle') });
  let missing = packMissing(packDir);
  if (!opt.pack && !exists(PACK_DONE)) missing.push('build/packgen/pack_done.json (the last pack build did not finish)');
  // the shader compiler (and on Linux the Oodle helper): the shipped ones, or built from source when the pack is made
  const prep = () => {
    if (underWine) stop(1, ['The asset pack cannot be built by the Windows setup under Proton/Wine.', WINE_HINT]);
    if (!dxc) stop(1, 'the pack needs the DirectX shader compiler (step 1)');
    if (fromSource && !opt['skip-build'] && !isWin) {
      // (Linux) the shader compiler and the Oodle helper, built for Linux
      say('   the shader compiler kh-shaderc and the Oodle helper kh-oodle (cargo build --release -p kh-shaderc -p kh-oodle)');
      if (run('cargo', ['build', '--release', '-p', 'kh-shaderc', '-p', 'kh-oodle'])) stop(1, 'cargo build -p kh-shaderc -p kh-oodle failed (see the errors above)');
      Object.assign(packEnv, { KH_SHADERC: path.join(REPO, 'target', 'release', 'kh-shaderc'), KH_OODLE: path.join(REPO, 'target', 'release', 'kh-oodle') });
    } else if (fromSource && !opt['skip-build']) {
      say('   the shader compiler kh-shaderc (cargo build --release -p kh-shaderc)');
      if (run('cargo', ['build', '--release', '-p', 'kh-shaderc'])) stop(1, 'cargo build -p kh-shaderc failed (see the errors above)');
      packEnv.KH_SHADERC = path.join(REPO, 'target', 'release', 'kh-shaderc.exe');
    }
  };
  const writePrints = () => { fs.mkdirSync(path.dirname(PACK_PRINTS), { recursive: true }); fs.writeFileSync(PACK_PRINTS, JSON.stringify({ ...packPrints(packEnv.KH_SHADERC), written: new Date().toISOString() }, null, 1) + '\n'); };
  // (PACKUPDATE) a complete pack of this repository: what this version changed in the pack's inputs decides what is made again
  let full = !!missing.length || !!opt['rebuild-pack'];
  let shadersOnly = false;
  if (opt.pack) {
    if (missing.length) stop(1, `--pack ${packDir} is not complete (missing: ${missing.join(', ')})`);
  } else if (!full) {
    const before = readPrints();
    if (fromSource) prep(); // (the compiler built from this checkout: its own print)
    const now = packPrints(packEnv.KH_SHADERC);
    if (before && before.rest !== now.rest) {
      full = true;
      say('   this version changed the tools the pack is made with: the pack is made again (half an hour or more)');
    } else if (!before || before.shaders !== now.shaders) {
      shadersOnly = true;
      say(before ? '   this version changed the shaders: only they are made again (a few minutes; nothing else of the pack)' : '   a pack of an earlier version: its shaders are made again for this one (a few minutes; nothing else of the pack)');
    }
  }
  if (shadersOnly) {
    if (!fromSource) prep();
    if (run(process.execPath, ['tools/packgen.js', 'shaders', '--er', erGame, ...workers], { env: packEnv })) stop(1, ['the shaders could not be made (the step that failed and its log are above: build/packgen/logs).', 'Run the setup again; with --rebuild-pack the whole pack is made again.']);
    writePrints();
    ok(`pack: ${packDir} (its shaders made again)`);
  } else if (full) {
    if (!fromSource || missing.length || opt['rebuild-pack']) prep();
    say('   building the pack from your KH3 install: extract, convert, research, pack, replay, shaders (half an hour or more, about 80 GB of disk)');
    // a build that stopped goes on where it stopped (tools/packgen.js --resume: build/packgen/done_steps.json); --rebuild-pack starts again
    const doneSteps = path.join(REPO, 'build', 'packgen', 'done_steps.json');
    if (opt['rebuild-pack'] || !missing.length) fs.rmSync(doneSteps, { force: true }); // (PACKUPDATE: changed tools start again too)
    const resume = exists(doneSteps) ? ['--resume'] : [];
    if (resume.length) say('   going on with the last pack build (the steps it finished are kept; --rebuild-pack starts again)');
    // a build from the start starts from an empty pack folder: a pack tool that records what its folder holds (the
    // outfits' file counts and sizes in outfits/outfits.json) would else count the files a previous build's later steps
    // left there, and the replay's check of the recorded files fails (the same on Windows and Linux)
    else fs.rmSync(path.join(REPO, 'build', 'sora_pack'), { recursive: true, force: true, maxRetries: 3 });
    if (run(process.execPath, ['tools/packgen.js', 'extract', 'convert', 'research', 'pack', 'replay', 'shaders', '--er', erGame, ...workers, ...resume], { env: packEnv })) stop(1, ['the pack build failed: the step that failed and its log are above (build/packgen/logs).', 'Run the setup again after fixing it: the build goes on from the step that failed.']);
    fs.mkdirSync(path.dirname(PACK_DONE), { recursive: true }); fs.writeFileSync(PACK_DONE, JSON.stringify({ done: new Date().toISOString(), kh3: exeNow }, null, 1) + '\n');
    writePrints();
    ok(`pack: ${packDir}`);
  } else ok(`pack: ${packDir} (up to date)`);

  // ---- 5. generated tables
  step(5, 'generated tables (tools/packgen.js gen)');
  let genDir = opt.gen ? path.resolve(opt.gen) : path.join(REPO, 'build', 'gen', 'rust_runtime');
  if (!opt.gen && (opt['rebuild-pack'] || !exists(path.join(genDir, 'khg_schemas.rs')) || !exists(path.join(packDir, 'gen', 'schemas.json')))) {
    if (packDir !== path.join(REPO, 'build', 'sora_pack')) stop(1, 'the generated tables are made from build/sora_pack: give --gen DIR with --pack, or let the setup build the pack');
    if (run(process.execPath, ['tools/packgen.js', 'gen'], { env: packEnv })) stop(1, 'the generated tables failed (see above)');
  }
  const schemasFile = path.join(packDir, 'gen', 'schemas.json');
  const packSchemas = exists(schemasFile) ? JSON.parse(fs.readFileSync(schemasFile, 'utf8')).files : null;
  if (packSchemas) ok(`tables: ${genDir}; the pack's data files: ${Object.keys(packSchemas).length} (${path.join(packDir, 'gen')})`);
  else stop(1, `the pack has no generated data (${schemasFile}): run the setup again with --rebuild-pack`);

  // ---- 6. the DLLs
  step(6, fromSource ? 'building the mod (cargo build --release -p kh-er -p kh-er-loader)' : 'the prebuilt DLLs');
  fs.mkdirSync(MODS, { recursive: true });
  if (!fromSource) {
    // the prebuilt DLL expects the tables of the pack it was released with: same files, same schemas
    const want = JSON.parse(fs.readFileSync(path.join(MODS, 'kh_er.schemas.json'), 'utf8')).files;
    const bad = Object.entries(want).filter(([f, s]) => packSchemas[f] !== s).map(([f]) => f);
    if (bad.length) stop(1, [`your pack's generated tables do not match the prebuilt DLL's (${bad.join(', ')}): your KH3 data gives other tables than the release's.`, 'Build the mod from source instead: install Rust (https://rustup.rs) and run node setup/setup.js --from-source']);
    ok('er/mods/kh_er.dll, er/mods/kh_er_loader.dll (prebuilt; the pack\'s tables match them)');
  } else if (opt['skip-build']) say('   skipped (--skip-build)');
  else {
    // (Linux) the same DLLs, cross-compiled with cargo-xwin, and the self-check built for Linux (the same table list)
    const target = isWin ? ['build', '--release', '-p', 'kh-er', '-p', 'kh-er-loader'] : ['xwin', 'build', '--release', '--target', 'x86_64-pc-windows-msvc', '-p', 'kh-er', '-p', 'kh-er-loader'];
    if (run('cargo', target, { env: { ...process.env, KH_GEN_RUST_DIR: genDir } })) stop(1, 'cargo build failed (see the errors above)');
    if (!isWin && run('cargo', ['build', '--release', '-p', 'kh-selfcheck'], { env: { ...process.env, KH_GEN_RUST_DIR: genDir } })) stop(1, 'cargo build -p kh-selfcheck failed (see the errors above)');
    for (const dll of ['kh_er.dll', 'kh_er_loader.dll']) {
      const src = isWin ? path.join(REPO, 'target', 'release', dll) : path.join(REPO, 'target', 'x86_64-pc-windows-msvc', 'release', dll), dst = path.join(MODS, dll);
      try { fs.rmSync(dst, { force: true }); fs.copyFileSync(src, dst); } catch (e) { stop(1, `could not write ${dst} (${e.code}): close Elden Ring and run the setup again`); }
    }
    fs.writeFileSync(path.join(MODS, 'kh_er.schemas.json'), JSON.stringify({ format: 'kh-gendata-schemas/1', built_from: 'source', files: packSchemas }, null, 1) + '\n');
    ok('er/mods/kh_er.dll, er/mods/kh_er_loader.dll');
  }
  if (!['kh_er.dll', 'kh_er_loader.dll'].every((d) => exists(path.join(MODS, d)))) stop(1, 'er/mods/kh_er.dll and kh_er_loader.dll are not there');

  // ---- 7. config files, the me3 profile, the DLL's self-check
  step(7, 'settings file, me3 profile, self-check');
  for (const d of ['saves', 'logs']) fs.mkdirSync(path.join(ER_DIR, d), { recursive: true });
  // the mod's settings file is er/mods/kh-er.ini; an older setup wrote er/mods/kh-er.ctl, which is converted once (the same
  // keys and values, in sections; the old file is kept as kh-er.ctl.bak: tools/ctl_to_ini.js)
  const ini = path.join(MODS, 'kh-er.ini'), ctl = path.join(MODS, 'kh-er.ctl');
  if (exists(ini)) ok('er/mods/kh-er.ini kept (your settings)');
  else if (exists(ctl)) {
    if (require(path.join(REPO, 'tools', 'ctl_to_ini.js')).run(ctl, false) !== 0) stop(1, 'er/mods/kh-er.ctl could not be converted to er/mods/kh-er.ini (above): close Elden Ring and run the setup again');
    ok('er/mods/kh-er.ini written from your er/mods/kh-er.ctl (the same settings; the old file is kept as kh-er.ctl.bak)');
  } else { fs.copyFileSync(path.join(__dirname, 'templates', 'kh-er.ini'), ini); ok('er/mods/kh-er.ini written (the mod\'s settings; yours from now on)'); }
  const cfg = path.join(MODS, 'kh-er.cfg');
  if (opt.pack) { fs.writeFileSync(cfg, `# written by setup/setup.js: where the asset pack is\npack=${packDir}\n`); ok(`er/mods/kh-er.cfg: pack=${packDir}`); }
  else if (exists(cfg)) ok('er/mods/kh-er.cfg kept');
  const prof = exists(PROFILE) ? fs.readFileSync(PROFILE, 'utf8') : '';
  // the profile must give the mod its OWN save file: without a savefile line (or with ER0000.sl2, the normal game's save)
  // the game plays the mod on your normal Elden Ring characters, and the mod's saving rules then apply to them
  const save = (prof.match(/^[ \t]*savefile[ \t]*=[ \t]*["']([^"'\r\n]*)["']/m) || [])[1];
  const saveFix = `Fix: put the line  savefile = "KHSora.sl2"  at the top of ${path.relative(REPO, PROFILE)} (before the first [[...]] section), or restore the file from the repository. Any name ending in .sl2 works, except ER0000.sl2.`;
  if (prof && save === undefined) stop(1, [`${PROFILE} has no savefile line: me3 would start Elden Ring on its normal save ER0000.sl2, and the mod would play (and save) on your normal characters.`, saveFix]);
  if (prof && /^er0000\.sl2$/i.test(path.basename(save.trim()))) stop(1, [`${PROFILE} sets savefile = "${save}": that is the normal Elden Ring save. The mod must never play on it: its saving rules would apply to your normal characters.`, saveFix]);
  if (prof && !/\.sl2$/i.test(save.trim())) stop(1, [`${PROFILE} sets savefile = "${save}", which is not an .sl2 save file name.`, saveFix]);
  if (/path\s*=\s*'\.\.\/mods\/kh_er_loader\.dll'/.test(prof) && !/start_online\s*=\s*true/.test(prof)) ok(`me3 profile: ${path.relative(REPO, PROFILE)} (offline, own save file ${save}, loads er/mods/kh_er_loader.dll)`);
  else stop(1, `${PROFILE} is missing or changed: restore it from the repository`);
  if (isWin) {
    const sc = capture('powershell', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', path.join(REPO, 'release-tools', 'selfcheck.ps1'), '-Mods', MODS]);
    for (const l of sc.out.split(/\r?\n/).filter(Boolean)) say('   ' + l.trim());
    if (sc.status !== 0) stop(1, underWine ? ['the DLL\'s self-check could not run under Proton/Wine (it needs Windows PowerShell).', WINE_HINT] : 'the DLL\'s self-check failed (above): the pack\'s generated data is missing or does not fit this DLL');
    ok('the DLL reads the pack\'s generated data');
  } else {
    // (Linux) the DLL runs only inside Proton: the checks of release-tools/selfcheck.ps1 made here, the table check with the
    // DLL's own self-check code built for Linux (tools/bin/kh-er-selfcheck, crates/kh-selfcheck)
    const loader = fs.readFileSync(path.join(MODS, 'kh_er_loader.dll')).toString('latin1');
    const loaderOk = loader.includes('2.7.1.0 or 2.7.2.0') && !loader.includes('2.7.1.0 (app 1.17.1) only');
    say(loaderOk ? '   loader: built for Elden Ring 2.7.1.0 and 2.7.2.0' : '   loader: NOT the 2.7.1.0 / 2.7.2.0 loader');
    if (!fs.readFileSync(path.join(MODS, 'kh_er.dll')).toString('latin1').includes('kh_er_selfcheck')) stop(1, 'er/mods/kh_er.dll has no offline self-check: not a release build of kh_er.dll');
    const tool = linuxTool('kh-er-selfcheck');
    if (!tool) stop(1, 'tools/bin/kh-er-selfcheck is missing (a source checkout: cargo build --release -p kh-selfcheck)');
    const sc = capture(tool, [MODS]);
    for (const l of sc.out.split(/\r?\n/).filter(Boolean)) say('   ' + l.trim());
    if (sc.status !== 0 || !loaderOk) stop(1, 'the DLL\'s self-check failed (above): the pack\'s generated data is missing or does not fit this DLL');
    ok('the DLL\'s tables read the pack\'s generated data');
  }

  // ---- 8. launching
  step(8, 'launching');
  if (!isWin) {
    // (Linux) me3's Linux build starts the game through Steam's Proton (the game's own Proton version and prefix)
    say('   Ready. OFFLINE ONLY: me3 starts Elden Ring without Easy Anti-Cheat; never play online with the mod.');
    say('   Reminder: the files the setup made contain game content. Keep them on your PC and don\'t share them.');
    say('   Steam must be running. Start the mod any of these ways:');
    say('     - er/play.sh (runs me3 with the mod\'s profile), or');
    say(`     - ${me3 ? `"${me3}"` : 'me3'} launch -g eldenring -p "${PROFILE}"`);
    say('   The mod uses its own save file (KHSora.sl2, in Elden Ring\'s Proton prefix: steamapps/compatdata/1245620/pfx/drive_c/users/steamuser/AppData/Roaming/EldenRing/<your id>/); your KH progress is kept in er/saves. Your normal saves are not used. To play the normal game, start it from Steam as usual.');
    if (rl) rl.close();
    return;
  }
  say('   Ready. OFFLINE ONLY: me3 starts Elden Ring without Easy Anti-Cheat; never play online with the mod.');
  say('   Reminder: the files the setup made contain game content. Keep them on your PC and don\'t share them.');
  say('   Steam must be running. Start the mod any of these ways:');
  say('     - er\\play.bat (runs me3 with the mod\'s profile), or');
  say(`     - ${me3 ? `"${me3}"` : 'me3'} launch -g eldenring -p "${PROFILE}"`);
  say('     - with me3\'s Windows installer, double-clicking the profile also works; with Me3 Manager, see the README, "Play".');
  say('   The mod uses its own save file (KHSora.sl2; your KH progress is kept in er\\saves); your normal saves are not used. To play the normal game, start it from Steam as usual.');
  if (rl) rl.close();
})().catch((e) => { console.error(e); process.exit(1); });
