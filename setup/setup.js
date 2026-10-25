#!/usr/bin/env node
// Kingdom Hearts x Elden Ring: the first-run setup. Run from the repository folder (a release download or a checkout):
//
//   double-click setup.exe                 (a release download: Node.js built in; `setup.exe [options]` from a prompt)
//   double-click setup\setup.bat           (runs setup.exe; in a source checkout: a Node 20+ on PATH, or it gets the
//                                           pinned portable Node into tools\bin\node; options are passed on)
//   node setup/setup.js [options]          (advanced: with a Node 20+ of your own)
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

let rl = null;
function ask(question) {
  if (opt.yes) return Promise.resolve('');
  rl = rl || readline.createInterface({ input: process.stdin, output: process.stdout });
  return new Promise((res) => rl.question(`   ? ${question} `, (a) => res(a.trim().replace(/^"|"$/g, ''))));
}
function browse(title) {
  if (!isWin) return '';
  const ps = `Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.FolderBrowserDialog; $d.Description = '${title.replace(/'/g, "''")}'; if ($d.ShowDialog() -eq 'OK') { $d.SelectedPath }`;
  return capture('powershell', ['-NoProfile', '-STA', '-Command', ps]).out.split(/\r?\n/).pop() || '';
}
// A game that was not found: the folder picker (with --browse, or when started from setup\setup.bat, which sets
// KH_SETUP_PICKER=1), else a typed path. A cancelled picker falls back to the typed path.
async function askFolder(what, check) {
  const picker = isWin && !opt.yes && (opt.browse || process.env.KH_SETUP_PICKER === '1');
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
function steamRoot() {
  if (!isWin) return null;
  for (const [key, value] of [['HKCU\\Software\\Valve\\Steam', 'SteamPath'], ['HKLM\\SOFTWARE\\WOW6432Node\\Valve\\Steam', 'InstallPath'], ['HKLM\\SOFTWARE\\Valve\\Steam', 'InstallPath']]) {
    const r = capture('reg', ['query', key, '/v', value]);
    const m = r.status === 0 && r.out.match(new RegExp(`${value}\\s+REG_\\w+\\s+(.+)`));
    if (m && isDir(m[1].trim())) return path.normalize(m[1].trim());
  }
  return null;
}
function steamLibraries() {
  const root = steamRoot();
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
    const sa = path.join(lib, 'steamapps');
    let names = [];
    try { names = fs.readdirSync(sa).filter((f) => /^appmanifest_\d+\.acf$/.test(f)); } catch { continue; }
    for (const f of names) {
      const t = fs.readFileSync(path.join(sa, f), 'utf8');
      const get = (k) => (t.match(new RegExp(`"${k}"\\s+"([^"]*)"`)) || [])[1];
      const dir = get('installdir') && path.join(sa, 'common', get('installdir'));
      if (dir && isDir(dir)) apps.push({ appid: get('appid'), name: get('name') || '', dir });
    }
  }
  return apps;
}
// Every installed Epic game: { name, dir }
function epicApps() {
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
    const inner = path.join(root, 'KINGDOM HEARTS III');
    const bin = path.join(inner, 'Binaries', 'Win64');
    const paks = path.join(inner, 'Content', 'Paks');
    if (!isDir(bin) || !isDir(paks)) continue;
    const exe = fs.readdirSync(bin).find((f) => /^KINGDOM HEARTS III.*\.exe$/i.test(f) && !/shipping-cmd/i.test(f));
    if (!exe || !fs.readdirSync(paks).some((f) => f.toLowerCase().endsWith('.pak'))) continue;
    return { gameRoot: root.split(path.sep).join('/'), mainExe: `KINGDOM HEARTS III/Binaries/Win64/${exe}`, paksDir: 'KINGDOM HEARTS III/Content/Paks' };
  }
  return null;
}
// An Elden Ring install: returns the path of eldenring.exe or null (the install folder, its Game folder or the exe itself).
function erAt(dir) {
  if (!dir) return null;
  dir = path.resolve(dir);
  for (const exe of [path.join(dir, 'Game', 'eldenring.exe'), path.join(dir, 'eldenring.exe'), dir]) if (/eldenring\.exe$/i.test(exe) && exists(exe)) return exe;
  return null;
}
function fileVersion(exe) {
  if (!isWin) return null;
  const r = capture('powershell', ['-NoProfile', '-Command', `(Get-Item -LiteralPath '${exe.replace(/'/g, "''")}').VersionInfo.FileVersion`]);
  return r.status === 0 ? r.out.split(/\r?\n/).pop().trim() : null;
}

// ---------------------------------------------------------------- me3
function findMe3() {
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
const packMissing = (dir) => PACK_REQUIRED.filter((f) => !exists(path.join(dir, f)));
function findDxc() {
  if (opt.dxc) return exists(opt.dxc) ? path.resolve(opt.dxc) : null;
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

// ================================================================ the steps
(async function main() {
  say('Kingdom Hearts x Elden Ring: setup');
  say(`repository: ${REPO}`);

  // ---- 1. prerequisites
  step(1, 'prerequisites');
  const nodeMajor = Number(process.versions.node.split('.')[0]);
  const bundledNode = path.join(REPO, 'tools', 'bin', 'node', 'node.exe');
  const inSea = (() => { try { return require('node:sea').isSea(); } catch { return false; } })();
  const where = inSea ? ' (built into setup.exe)' : path.resolve(process.execPath).toLowerCase() === bundledNode.toLowerCase() ? ' (the portable Node in tools/bin/node)' : ` (${process.execPath})`;
  if (nodeMajor >= 20) ok(`Node ${process.versions.node}${where}`);
  else stop(1, `Node 20 or newer is needed (found ${process.versions.node}): start the setup with setup.exe or setup\\setup.bat, which bring their own Node`);
  // the npm packages: bundled in a release download (node_modules, the versions of package-lock.json); a source checkout
  // installs the locked versions once (npm ci; setup\setup.bat does it before this)
  if (!['ajv', 'iced-x86', '@wasm-audio-decoders/ogg-vorbis', 'gltf-validator'].every((m) => exists(path.join(REPO, 'node_modules', ...m.split('/'), 'package.json')))) {
    if (opt['no-download']) stop(1, 'the npm packages (node_modules) are missing and --no-download was given: use a release download, or run without --no-download');
    const ownNpm = path.join(REPO, 'tools', 'bin', 'node', 'npm.cmd');
    const npm = isWin ? (exists(ownNpm) ? ownNpm : 'npm.cmd') : 'npm';
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
  const prebuilt = ['kh_er.dll', 'kh_er_loader.dll', 'kh_er.schemas.json'].every((d) => exists(path.join(MODS, d))) && exists(path.join(REPO, 'tools', 'bin', 'kh-shaderc.exe'));
  const fromSource = !!opt['from-source'] || !prebuilt;
  const cargo = capture('cargo', ['--version']);
  const haveCargo = cargo.status === 0;
  if (!fromSource) ok('prebuilt DLLs and shader compiler (a release download): Rust is not needed');
  else if (haveCargo) ok(cargo.out);
  else if (!opt['skip-build']) stop(1, ['Rust is needed to build the mod from source: https://rustup.rs (stable, x86_64-pc-windows-msvc, with the "Desktop development with C++" Visual Studio Build Tools).', 'Or use a release download (the DLLs come prebuilt).']);
  else warn('cargo not found (--skip-build)');
  let dxc = findDxc();
  if (!dxc && !opt['no-download']) dxc = downloadDxc();
  if (dxc) ok(`dxc ${dxc} (the pack's shaders)`); else warn('no DirectX shader compiler: the pack cannot be built. Install the Windows 10/11 SDK, allow the download (no --no-download) or give --dxc FILE');
  const me3 = findMe3();
  if (me3) {
    const v = capture(me3, ['--version']).out;
    const ver = (v.match(/\d+\.\d+\.\d+/) || [])[0];
    ok(`me3 ${ver || '(version unknown)'} ${me3}`);
    if (ver && ver !== TESTED_ME3) warn(`the mod was tested with me3 ${TESTED_ME3}; ${ver} may work`);
  } else warn(`me3 not found (Mod Engine 3, tested ${TESTED_ME3}: https://github.com/garyttierney/me3/releases). Install it before launching, or give --me3 FILE.`);

  // ---- 2. the games
  step(2, 'finding the games');
  const steam = steamApps(), epic = epicApps();
  say(`   Steam libraries: ${steamLibraries().length}, Steam apps: ${steam.length}, Epic games: ${epic.length}`);
  let kh3 = kh3At(opt.kh3);
  if (opt.kh3 && !kh3) stop(2, `--kh3 ${opt.kh3}: not a KINGDOM HEARTS III install (it must contain "KINGDOM HEARTS III\\Binaries\\Win64" and "KINGDOM HEARTS III\\Content\\Paks")`);
  if (!kh3) for (const a of steam.filter((a) => /KINGDOM HEARTS\s*(III|Ⅲ|3)/i.test(a.name) || /KINGDOM HEARTS III/i.test(a.dir))) if ((kh3 = kh3At(a.dir))) { ok(`KH3 (Steam, app ${a.appid}): ${a.dir}`); break; }
  if (!kh3) for (const a of epic.filter((a) => /KINGDOM HEARTS\s*(III|Ⅲ|3)/i.test(a.name))) if ((kh3 = kh3At(a.dir))) { ok(`KH3 (Epic): ${a.dir}`); break; }
  if (!kh3) { warn('KINGDOM HEARTS III was not found in Steam or Epic'); kh3 = await askFolder('KINGDOM HEARTS III', kh3At); }
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
  let missing = packMissing(packDir);
  if (!opt.pack && !exists(PACK_DONE)) missing.push('build/packgen/pack_done.json (the last pack build did not finish)');
  if (!missing.length && !opt['rebuild-pack']) ok(`pack: ${packDir}`);
  else if (opt.pack) stop(1, `--pack ${packDir} is not complete (missing: ${missing.join(', ')})`);
  else {
    if (!dxc) stop(1, 'the pack needs the DirectX shader compiler (step 1)');
    if (fromSource && !opt['skip-build']) {
      say('   the shader compiler kh-shaderc (cargo build --release -p kh-shaderc)');
      if (run('cargo', ['build', '--release', '-p', 'kh-shaderc'])) stop(1, 'cargo build -p kh-shaderc failed (see the errors above)');
      packEnv.KH_SHADERC = path.join(REPO, 'target', 'release', 'kh-shaderc.exe');
    }
    say('   building the pack from your KH3 install: extract, convert, research, pack, replay, shaders (half an hour or more, about 80 GB of disk)');
    // a build that stopped goes on where it stopped (tools/packgen.js --resume: build/packgen/done_steps.json); --rebuild-pack starts again
    const doneSteps = path.join(REPO, 'build', 'packgen', 'done_steps.json');
    if (opt['rebuild-pack']) fs.rmSync(doneSteps, { force: true });
    const resume = exists(doneSteps) ? ['--resume'] : [];
    if (resume.length) say('   going on with the last pack build (the steps it finished are kept; --rebuild-pack starts again)');
    if (run(process.execPath, ['tools/packgen.js', 'extract', 'convert', 'research', 'pack', 'replay', 'shaders', '--er', erGame, ...workers, ...resume], { env: packEnv })) stop(1, ['the pack build failed: the step that failed and its log are above (build/packgen/logs).', 'Run the setup again after fixing it: the build goes on from the step that failed.']);
    fs.mkdirSync(path.dirname(PACK_DONE), { recursive: true }); fs.writeFileSync(PACK_DONE, JSON.stringify({ done: new Date().toISOString(), kh3: exeNow }, null, 1) + '\n');
    ok(`pack: ${packDir}`);
  }

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
    if (run('cargo', ['build', '--release', '-p', 'kh-er', '-p', 'kh-er-loader'], { env: { ...process.env, KH_GEN_RUST_DIR: genDir } })) stop(1, 'cargo build failed (see the errors above)');
    for (const dll of ['kh_er.dll', 'kh_er_loader.dll']) {
      const src = path.join(REPO, 'target', 'release', dll), dst = path.join(MODS, dll);
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
    if (sc.status !== 0) stop(1, 'the DLL\'s self-check failed (above): the pack\'s generated data is missing or does not fit this DLL');
    ok('the DLL reads the pack\'s generated data');
  }

  // ---- 8. launching
  step(8, 'launching');
  say('   Ready. OFFLINE ONLY: me3 starts Elden Ring without Easy Anti-Cheat; never play online with the mod.');
  say('   Steam must be running. Start the mod any of these ways:');
  say('     - er\\play.bat (runs me3 with the mod\'s profile), or');
  say(`     - ${me3 ? `"${me3}"` : 'me3'} launch -g eldenring -p "${PROFILE}"`);
  say('     - with me3\'s Windows installer, double-clicking the profile also works; with Me3 Manager, see README "Using Me3 Manager (GUI)".');
  say('   The mod uses its own save file (KHSora.sl2); your normal saves are not used. To play the normal game, start it from Steam as usual.');
  if (rl) rl.close();
})().catch((e) => { console.error(e); process.exit(1); });
