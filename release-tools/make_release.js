#!/usr/bin/env node
// Builds the release download: the prebuilt mod (kh_er.dll, kh_er_loader.dll, the shader compiler kh-shaderc.exe), the me3
// profile, the setup and its tools, the docs, as one zip. Nothing is uploaded: the zip is left in dist/ for the owner.
//
//   node release-tools/make_release.js [--version V] [--skip-build] [--out DIR]
//
// Needs a checkout where the setup has built the pack (build/sora_pack) and the generated tables (build/gen/rust): the
// DLL is compiled against the RUN-TIME version of the tables made from KH3 data (tools/packgen_genruntime.js -> build/gen/
// rust_runtime), so it carries no KH3 table; the player's setup makes those tables' data files (build/sora_pack/gen) from
// their own install, and the DLL reads them when it starts (crates/kh-gendata). Steps:
//   1. tools/packgen_genruntime.js; cargo build --release -p kh-er -p kh-er-loader -p kh-shaderc (KH_GEN_RUST_DIR)
//   2. the release folder dist/kingdom-hearts-x-elden-ring-<V>/kingdom-hearts-x-elden-ring/ (the zip's top folder has no
//      version, so a new version unzips over the old one): the repository's player-facing files (no Rust sources,
//      no SDK) + er/mods/{kh_er.dll, kh_er_loader.dll, kh_er.schemas.json} + tools/bin/kh-shaderc.exe
//      + setup.exe (the Node.js release-tools/node_pin.json pins and hashes, as a single executable application of
//      release-tools/sea_main.js, by release-tools/sea_build.mjs; its hash in release-tools/setup_exe.json) + the npm
//      packages (node_modules: npm ci of package-lock.json, production only) + THIRD-PARTY-NOTICES.txt, so a player
//      installs neither Node nor npm packages (double-click setup.exe; setup\setup.bat runs it)
//   3. the checks: release-tools/scan.js on the folder (game files, keys, paths), and the binaries scanned for KH3 content:
//      no Japanese text, none of the game's text lines (the pack's own UI strings, read from the player's KH3 install, are
//      the reference), none of the KH3 tables moved to run time (samples of their strings), no developer paths; the loader
//      is the 2.7.1.0 / 2.7.2.0 one; the DLL exports its offline self-check
//   4. the zip (PowerShell Compress-Archive)
//   5. the linux-x64 download (Steam's Proton on Linux, Steam Deck): the same folder with setup-linux (the pinned Node.js
//      linux-x64 as a single executable application of the same sea_main.js; release-tools/setup_linux.json) and the
//      static Linux builds tools/bin/kh-shaderc, kh-oodle, kh-er-selfcheck (cargo --target x86_64-unknown-linux-musl,
//      linked with rust-lld) in place of setup.exe and kh-shaderc.exe, the same DLLs and npm packages, the notices of
//      Node.js for Linux and musl; the same checks; a deterministic <name>-linux-x64.tar.gz (setup/tar.js).
//      --no-linux skips it.
'use strict';
const fs = require('fs');
const path = require('path');
const cp = require('child_process');

const REPO = path.resolve(__dirname, '..');
const args = process.argv.slice(2);
const opt = (k) => { const i = args.indexOf(k); return i >= 0 ? args[i + 1] : null; };
const VERSION = opt('--version') || JSON.parse(fs.readFileSync(path.join(REPO, 'package.json'), 'utf8')).version;
const DIST = path.resolve(opt('--out') || path.join(REPO, 'dist'));
const NAME = `kingdom-hearts-x-elden-ring-${VERSION}`;
// the folder inside the zip has no version in its name, so a new version unzips over the old one (keeping er/saves,
// the settings and the built pack)
const OUT = path.join(DIST, NAME, 'kingdom-hearts-x-elden-ring');
const GEN = path.join(REPO, 'build', 'gen', 'rust');
const GENRT = path.join(REPO, 'build', 'gen', 'rust_runtime');
const PACK = path.join(REPO, 'build', 'sora_pack');
const fail = (m) => { console.error('make_release: ' + m); process.exit(1); };
const run = (cmd, a, o = {}) => { console.log(`> ${cmd} ${a.join(' ')}`); const r = cp.spawnSync(cmd, a, { stdio: 'inherit', cwd: REPO, ...o }); if (r.status !== 0) fail(`${cmd} failed`); };

// ---- 1. build
if (!fs.existsSync(path.join(GEN, 'sora_anim.rs'))) fail('build/gen/rust is missing: run the setup (it builds the pack and the generated tables) first');
run(process.execPath, ['tools/packgen_genruntime.js']);
// (the binaries name no folder of the machine they were built on: the sources' and the crates' paths remapped, the debug
// file named without its folder)
const os = require('os');
const CARGO_HOME = process.env.CARGO_HOME || path.join(os.homedir(), '.cargo');
const RUSTUP_HOME = process.env.RUSTUP_HOME || path.join(os.homedir(), '.rustup');
const RUSTFLAGS = [`--remap-path-prefix=${REPO}=kh-x-er`, `--remap-path-prefix=${CARGO_HOME}=cargo`, `--remap-path-prefix=${RUSTUP_HOME}=rustup`, `--remap-path-prefix=${os.homedir()}=home`, '-Clink-arg=/PDBALTPATH:%_PDB%'].join(String.fromCharCode(31)); // (CARGO_ENCODED_RUSTFLAGS: the paths have spaces)
if (!args.includes('--skip-build')) run('cargo', ['build', '--release', '-p', 'kh-er', '-p', 'kh-er-loader', '-p', 'kh-shaderc'], { env: { ...process.env, KH_GEN_RUST_DIR: GENRT, CARGO_ENCODED_RUSTFLAGS: RUSTFLAGS } });
const T = path.join(REPO, 'target', 'release');
for (const f of ['kh_er.dll', 'kh_er_loader.dll', 'kh-shaderc.exe']) if (!fs.existsSync(path.join(T, f))) fail(`target/release/${f} is missing`);
// (linux-x64) the Linux setup's three tools, static Linux builds (x86_64-unknown-linux-musl, linked with the Rust
// toolchain's own rust-lld: they run on any Linux, Steam Deck included): kh-shaderc, kh-oodle, kh-er-selfcheck (the
// self-check against the same table list as the DLL, KH_GEN_RUST_DIR)
const LINUX_TARGET = 'x86_64-unknown-linux-musl';
const TL = path.join(REPO, 'target', LINUX_TARGET, 'release');
if (!args.includes('--skip-build') && !args.includes('--no-linux')) run('cargo', ['build', '--release', '--target', LINUX_TARGET, '-p', 'kh-shaderc', '-p', 'kh-oodle', '-p', 'kh-selfcheck'], { env: { ...process.env, KH_GEN_RUST_DIR: GENRT, CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER: 'rust-lld', CARGO_ENCODED_RUSTFLAGS: RUSTFLAGS.split(String.fromCharCode(31)).filter((f) => !f.startsWith('-Clink-arg=/PDBALTPATH')).join(String.fromCharCode(31)) } });

// ---- 2. the release folder
fs.rmSync(path.dirname(OUT), { recursive: true, force: true });
fs.mkdirSync(OUT, { recursive: true });
// the repository's files git would ship, without the Rust sources (a release builds nothing from source; the full
// repository stays the place to build from source)
// (not a git checkout, e.g. an unpacked copy: every file but the generated, downloaded and personal folders .gitignore lists)
const GENERATED = /^(\.git|node_modules|target|build|assets|assets_converted|audit|user|dist|tools\/bin|er\/(mods|saves|logs|live))(\/|$)|^tools\/config\.json$|^tools\/third_party\/ogg-vorbis-decoder\/ogg-vorbis-decoder\.min\.js$|^sheets\/(?!port(\/|$))|\.(exe|dll|pdb|log)$/i;
const walkAll = (d, base = d, out = []) => { for (const e of fs.readdirSync(d, { withFileTypes: true })) { const p = path.join(d, e.name); const r = path.relative(base, p).split(path.sep).join('/'); if (GENERATED.test(r)) continue; if (e.isDirectory()) walkAll(p, base, out); else out.push(r); } return out; };
let tracked;
try { tracked = cp.execFileSync('git', ['ls-files', '-co', '--exclude-standard'], { cwd: REPO, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).split('\n').filter(Boolean); } catch { tracked = walkAll(REPO); }
const LEAVE_OUT = /^(crates|vendor|target|dist)\/|^Cargo\.(toml|lock)$|^release-tools\/(export\.js|overlay\/|make_release\.js|sea_build\.mjs|sea\/|zip\.js|dxbc_disasm_check\.js|linux_sim\.js|linux_check\.sh)|^docs\/images\/logo\.png$/; // (the README's 640 px logo is shipped, not the 2048 px original)
let n = 0;
for (const f of tracked) {
  if (LEAVE_OUT.test(f) || !fs.existsSync(path.join(REPO, f))) continue;
  const d = path.join(OUT, f); fs.mkdirSync(path.dirname(d), { recursive: true }); fs.copyFileSync(path.join(REPO, f), d); n++;
}
const mods = path.join(OUT, 'er', 'mods'); fs.mkdirSync(mods, { recursive: true });
for (const f of ['kh_er.dll', 'kh_er_loader.dll']) fs.copyFileSync(path.join(T, f), path.join(mods, f));
fs.mkdirSync(path.join(OUT, 'tools', 'bin'), { recursive: true });
fs.copyFileSync(path.join(T, 'kh-shaderc.exe'), path.join(OUT, 'tools', 'bin', 'kh-shaderc.exe'));
// the tables the DLL expects (file -> schema): the setup checks the player's pack against them
const schemas = [...fs.readFileSync(path.join(GENRT, 'khg_schemas.rs'), 'utf8').matchAll(/\("([^"]+)", "([0-9a-f]+)"\)/g)].map((m) => [m[1], m[2]]);
if (!schemas.length) fail('build/gen/rust_runtime/khg_schemas.rs lists no table');
fs.writeFileSync(path.join(mods, 'kh_er.schemas.json'), JSON.stringify({ format: 'kh-gendata-schemas/1', release: VERSION, files: Object.fromEntries(schemas) }, null, 1) + '\n');
// ---- 2b. setup.exe and the npm packages: a release download needs no Node and runs no npm install
// Node: the pinned official win-x64 zip (release-tools/node_pin.json, checked against its SHASUMS256.txt hash), cached in
// dist/.cache; its node.exe becomes setup.exe (below; no separate node.exe is shipped). The npm packages: `npm ci --omit=dev
// --ignore-scripts` with that Node's own npm, from package.json + package-lock.json (the locked versions), into node_modules.
const crypto = require('crypto');
const PIN = JSON.parse(fs.readFileSync(path.join(REPO, 'release-tools', 'node_pin.json'), 'utf8'));
const sha256f = (p) => crypto.createHash('sha256').update(fs.readFileSync(p)).digest('hex');
const qps = (s) => s.replace(/'/g, "''");
const CACHE = path.join(DIST, '.cache'); fs.mkdirSync(CACHE, { recursive: true });
const nodeZip = path.join(CACHE, path.basename(PIN.zip_url));
if (!fs.existsSync(nodeZip) || sha256f(nodeZip) !== PIN.zip_sha256) {
  console.log(`> download ${PIN.zip_url}`);
  const d = cp.spawnSync('powershell', ['-NoProfile', '-Command', `$ProgressPreference='SilentlyContinue'; Invoke-WebRequest -UseBasicParsing -Uri '${PIN.zip_url}' -OutFile '${qps(nodeZip)}'`], { stdio: 'inherit' });
  if (d.status !== 0 || !fs.existsSync(nodeZip)) fail('the Node.js download failed');
}
if (sha256f(nodeZip) !== PIN.zip_sha256) fail(`${nodeZip} is not the pinned Node.js zip (SHA-256 of ${PIN.shasums_url})`);
const nodeDir = path.join(CACHE, PIN.zip_folder);
const nodeExe = path.join(nodeDir, 'node.exe');
if (!fs.existsSync(nodeExe) || sha256f(nodeExe) !== PIN.node_exe_sha256) {
  fs.rmSync(nodeDir, { recursive: true, force: true });
  const x = cp.spawnSync('powershell', ['-NoProfile', '-Command', `Expand-Archive -LiteralPath '${qps(nodeZip)}' -DestinationPath '${qps(CACHE)}' -Force`], { stdio: 'inherit' });
  if (x.status !== 0) fail('could not unpack the Node.js zip');
}
if (sha256f(nodeExe) !== PIN.node_exe_sha256) fail('node.exe is not the pinned one (release-tools/node_pin.json)');
// setup.exe: the pinned node.exe as a single executable application of release-tools/sea_main.js (the dispatcher that
// runs setup/setup.js and the tool scripts), built by release-tools/sea_build.mjs with the pinned postject and resedit
// (release-tools/sea/package-lock.json, installed in dist/.cache/sea_tools). No separate node.exe is shipped.
const SEA_TOOLS = path.join(CACHE, 'sea_tools'); fs.rmSync(SEA_TOOLS, { recursive: true, force: true }); fs.mkdirSync(SEA_TOOLS, { recursive: true });
for (const f of ['package.json', 'package-lock.json']) fs.copyFileSync(path.join(REPO, 'release-tools', 'sea', f), path.join(SEA_TOOLS, f));
run(nodeExe, [path.join(nodeDir, 'node_modules', 'npm', 'bin', 'npm-cli.js'), 'ci', '--ignore-scripts', '--no-fund', '--no-audit', '--no-update-notifier'], { cwd: SEA_TOOLS });
const SETUP_EXE = path.join(OUT, 'setup.exe');
run(nodeExe, [path.join(REPO, 'release-tools', 'sea_build.mjs'), '--node', nodeExe, '--tools', SEA_TOOLS, '--work', path.join(CACHE, 'sea_work'), '--out', SETUP_EXE, '--version', VERSION]);
const setupSha = sha256f(SETUP_EXE);
const setupRecord = JSON.stringify({ about: 'setup.exe of the release (release-tools/make_release.js): Node.js ' + PIN.version + ' (node.exe ' + PIN.node_exe_sha256 + ') as a single executable application of release-tools/sea_main.js; release-tools/scan.js allows setup.exe by this hash. The build is deterministic.', version: VERSION, node: PIN.version, sea_main_sha256: sha256f(path.join(REPO, 'release-tools', 'sea_main.js')), setup_exe_sha256: setupSha, bytes: fs.statSync(SETUP_EXE).size }, null, 1) + '\n';
fs.writeFileSync(path.join(OUT, 'release-tools', 'setup_exe.json'), setupRecord);
fs.writeFileSync(path.join(REPO, 'release-tools', 'setup_exe.json'), setupRecord); // (recorded in the repository too)
const W = path.join(CACHE, 'npm_ci'); fs.rmSync(W, { recursive: true, force: true }); fs.mkdirSync(W, { recursive: true });
for (const f of ['package.json', 'package-lock.json']) fs.copyFileSync(path.join(REPO, f), path.join(W, f));
run(nodeExe, [path.join(nodeDir, 'node_modules', 'npm', 'bin', 'npm-cli.js'), 'ci', '--omit=dev', '--ignore-scripts', '--no-fund', '--no-audit', '--no-update-notifier'], { cwd: W });
const lock = JSON.parse(fs.readFileSync(path.join(REPO, 'package-lock.json'), 'utf8'));
const pkgs = Object.entries(lock.packages).filter(([p, v]) => p && !v.dev).map(([p, v]) => ({ dir: p, name: p.replace(/^.*node_modules\//, ''), version: v.version, license: v.license }));
for (const p of pkgs) { const j = JSON.parse(fs.readFileSync(path.join(W, p.dir, 'package.json'), 'utf8')); if (j.version !== p.version) fail(`${p.name}: ${j.version} installed, ${p.version} locked`); }
fs.cpSync(path.join(W, 'node_modules'), path.join(OUT, 'node_modules'), { recursive: true });
// the licences: Node's own file, and each package's licence file (a package without one: its licence's standard text with
// the author from its package.json)
const MIT = (who) => `MIT License\n\nCopyright (c) ${who}\n\nPermission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:\n\nThe above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.\n\nTHE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.\n`;
let notices = `THIRD-PARTY SOFTWARE IN THIS DOWNLOAD (Kingdom Hearts x Elden Ring ${VERSION})\n\nThe mod's own files carry no licence yet (see LEGAL.md). Bundled with them, unmodified:\n\n`;
notices += `==== Node.js ${PIN.version} (built into setup.exe: a Node.js single executable application) - ${PIN.zip_url}\n(the MIT licence of Node.js, then the licences of the components built into node.exe)\n\n${fs.readFileSync(path.join(nodeDir, 'LICENSE'), 'utf8')}\n\n`;
for (const p of pkgs) {
  const dir = path.join(W, p.dir); const lf = fs.readdirSync(dir).find((f) => /^(licen[sc]e|copying)(\.(md|txt))?$/i.test(f));
  const j = JSON.parse(fs.readFileSync(path.join(dir, 'package.json'), 'utf8'));
  const who = typeof j.author === 'string' ? j.author : j.author && j.author.name;
  let text = lf ? fs.readFileSync(path.join(dir, lf), 'utf8') : p.license === 'MIT' && who ? MIT(who) : null;
  if (!text) fail(`${p.name}: no licence file and no standard text for ${p.license}`);
  if (!lf) fs.writeFileSync(path.join(OUT, p.dir, 'LICENSE'), text); // (written next to the package too)
  notices += `==== ${p.name} ${p.version} (${p.license}) - node_modules/${p.name}\n\n${text.trim()}\n\n`;
}
fs.writeFileSync(path.join(OUT, 'THIRD-PARTY-NOTICES.txt'), notices);
console.log(`bundled: setup.exe ${setupSha} (Node.js ${PIN.version}, node.exe ${PIN.node_exe_sha256}), ${pkgs.length} npm packages (${pkgs.map((p) => p.name + '@' + p.version).join(', ')})`);
console.log(`release folder: ${OUT} (${n} files + 3 binaries + setup.exe + node_modules)`);

// ---- 3. checks
const problems = [];
// 3a. the repository scan, with the three release binaries allowed
// (the folder this checkout sits in is a path of the build machine: no file of the release may name it)
const sc = cp.spawnSync(process.execPath, [path.join(OUT, 'release-tools', 'scan.js'), '--release-binaries'], { cwd: OUT, encoding: 'utf8', env: { ...process.env, KH_SCAN_PATHS: [process.env.KH_SCAN_PATHS, path.dirname(REPO)].filter(Boolean).join(';') } });
process.stdout.write(sc.stdout + sc.stderr);
if (sc.status !== 0) problems.push('release-tools/scan.js found problems');
// 3b. the binaries' content
const JP = /[぀-ヿ㐀-鿿ｦ-ﾟ]{2,}/;
// the game's own text lines: the UI pack's strings (made by the setup from the player's KH3 install)
const gameText = new Set();
const sdir = path.join(PACK, 'ui', 'strings', 'en');
// (sentences: a line that ends as one or has six words or more; names and short labels, like the item and ability names
// of the sheets, are not counted)
if (fs.existsSync(sdir)) for (const f of fs.readdirSync(sdir)) {
  const j = JSON.parse(fs.readFileSync(path.join(sdir, f), 'utf8'));
  for (const ns of Object.values(j.namespaces || {})) for (const [k, v] of Object.entries(ns)) {
    if (typeof v !== 'string' || /(^|_)name(_|$)/.test(k)) continue;
    const t = v.replace(/<br>/g, ' ').replace(/<[^>]*>/g, '').replace(/\s+/g, ' ').trim();
    if (t.length >= 24 && (/[.!?]$/.test(t) || t.split(' ').length >= 6)) gameText.add(t);
  }
}
// samples of the tables moved to run time (they must not be in the DLL any more): every string of the generated data
const runtimeSamples = [];
for (const f of fs.readdirSync(path.join(PACK, 'gen')).filter((x) => x.endsWith('.khg'))) {
  const b = fs.readFileSync(path.join(PACK, 'gen', f)).toString('latin1');
  let k = 0; for (const m of b.matchAll(/\/Game\/[A-Za-z0-9_\/.-]{12,}/g)) { runtimeSamples.push(m[0]); if (++k >= 40) break; }
}
const report = { version: VERSION, binaries: {} };
for (const rel of ['er/mods/kh_er.dll', 'er/mods/kh_er_loader.dll', 'tools/bin/kh-shaderc.exe']) {
  const b = fs.readFileSync(path.join(OUT, rel));
  const latin = b.toString('latin1'), utf8 = b.toString('utf8');
  const r = { bytes: b.length, japanese: 0, game_text_lines: [], runtime_table_strings: 0, developer_paths: 0, game_asset_paths: 0 };
  r.japanese = (utf8.match(new RegExp(JP.source, 'g')) || []).length;
  for (const t of gameText) { const probe = t.slice(0, 40); if (probe.length >= 24 && latin.includes(probe)) r.game_text_lines.push(probe); }
  r.runtime_table_strings = runtimeSamples.filter((s) => latin.includes(s)).length;
  r.developer_paths = (latin.match(/[A-Za-z]:[\\/]+Users[\\/]+/g) || []).length + [path.dirname(REPO), path.dirname(REPO).split(path.sep).join('/')].reduce((n, p) => n + latin.split(p).length - 1, 0);
  r.game_asset_paths = (latin.match(/\/Game\/[A-Za-z0-9_\/.-]+/g) || []).length;
  report.binaries[rel] = r;
  console.log(`${rel}: ${(b.length / 1048576).toFixed(1)} MB; Japanese runs ${r.japanese}; game text lines ${r.game_text_lines.length}; run-time table strings ${r.runtime_table_strings}/${runtimeSamples.length}; developer paths ${r.developer_paths}; /Game/ paths ${r.game_asset_paths} (asset identifiers of our own design sheets)`);
  if (r.japanese || r.game_text_lines.length || r.runtime_table_strings || r.developer_paths) problems.push(`${rel}: KH3 content or a developer path in the binary`);
}
const loader = fs.readFileSync(path.join(mods, 'kh_er_loader.dll')).toString('latin1');
if (!loader.includes('2.7.1.0 or 2.7.2.0') || loader.includes('2.7.1.0 (app 1.17.1) only')) problems.push('the loader is not the 2.7.1.0 / 2.7.2.0 one');
if (!fs.readFileSync(path.join(mods, 'kh_er.dll')).toString('latin1').includes('kh_er_selfcheck')) problems.push('kh_er.dll does not export kh_er_selfcheck');
fs.writeFileSync(path.join(DIST, NAME + '.check.json'), JSON.stringify({ ...report, game_text_reference_lines: gameText.size, runtime_samples: runtimeSamples.length, problems }, null, 1) + '\n');
if (problems.length) { for (const p of problems) console.error('PROBLEM: ' + p); fail('the release folder is not clean: no zip made'); }

// ---- 4. the zip
// (release-tools/zip.js: "/" in every entry name and an entry per folder, as the zip format requires; PowerShell 5.1's
// Compress-Archive wrote "\", which unzip tools outside Windows keep as part of one flat file name. Sorted, one fixed time
// stamp: the same folder gives the same zip. The files' bytes are stored as they are: the .bat files keep their CRLF.)
const zip = path.join(DIST, NAME + '-win64.zip');
fs.rmSync(zip, { force: true });
const zipLib = require(path.join(__dirname, 'zip.js'));
let zipTime = new Date(Date.UTC(2026, 0, 1));
try { const ct = Number(cp.execFileSync('git', ['log', '-1', '--format=%ct'], { cwd: REPO, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim()); if (ct) zipTime = new Date(ct * 1000); } catch { /* not a git checkout */ }
const zr = zipLib.zipFolder(OUT, zip, { top: 'kingdom-hearts-x-elden-ring', time: zipTime });
// the zip read back: every name with "/", none with "\", every file as in the folder
const back = zipLib.readZip(fs.readFileSync(zip));
const badNames = back.filter((e) => e.name.includes('\\') || !e.name.startsWith('kingdom-hearts-x-elden-ring/'));
if (badNames.length) fail(`the zip has ${badNames.length} entry name(s) with "\\" or outside its top folder`);
for (const e of back.filter((x) => !x.dir)) if (!fs.readFileSync(path.join(OUT, ...e.name.split('/').slice(1))).equals(e.data)) fail(`the zip's ${e.name} is not the release folder's file`);
console.log(`zip: ${zr.entries} entries (${back.filter((e) => e.dir).length} folders), every name with "/"; read back and checked`);
console.log(`make_release: ${zip} (${(fs.statSync(zip).size / 1048576).toFixed(1)} MB); checks: ${path.join(DIST, NAME + '.check.json')}`);

// ---- 5. the linux-x64 download (Steam's Proton on Linux, Steam Deck): the same folder with the Linux programs
// The same player files, the same prebuilt Windows DLLs (they run inside Proton) and the same node_modules; instead of
// setup.exe and kh-shaderc.exe: setup-linux (the pinned Node.js linux-x64 as a single executable application of the same
// release-tools/sea_main.js; its hash in release-tools/setup_linux.json; deterministic), tools/bin/kh-shaderc,
// tools/bin/kh-oodle, tools/bin/kh-er-selfcheck (static Linux builds); er/play.sh and setup/setup.sh are in the
// repository's files. The same checks, then a deterministic .tar.gz (setup/tar.js: fixed owners, times and order, the
// executables' modes 755).
if (args.includes('--no-linux')) process.exit(0);
const tar = require(path.join(REPO, 'setup', 'tar.js'));
const LX = PIN.linux_x64;
if (!LX) fail('release-tools/node_pin.json has no linux_x64 pin');
const OUTL = path.join(DIST, NAME + '-linux', 'kingdom-hearts-x-elden-ring');
for (const f of ['kh-shaderc', 'kh-oodle', 'kh-er-selfcheck']) {
  const p = path.join(TL, f);
  if (!fs.existsSync(p)) fail(`target/${LINUX_TARGET}/release/${f} is missing`);
  const b = fs.readFileSync(p);
  if (b.toString('latin1', 0, 4) !== '\x7fELF') fail(`${f}: not an ELF file`);
  // static: no program interpreter (PT_INTERP) and no dynamic libraries
  const phoff = Number(b.readBigUInt64LE(32)), phn = b.readUInt16LE(56), phs = b.readUInt16LE(54);
  for (let k = 0; k < phn; k++) if (b.readUInt32LE(phoff + k * phs) === 3) fail(`${f}: not a static build (it names a program interpreter)`);
}
fs.rmSync(path.dirname(OUTL), { recursive: true, force: true });
fs.mkdirSync(OUTL, { recursive: true });
fs.cpSync(OUT, OUTL, { recursive: true, filter: (src) => { const r = path.relative(OUT, src).split(path.sep).join('/'); return r !== 'setup.exe' && r !== 'tools/bin/kh-shaderc.exe' && r !== 'release-tools/setup_exe.json'; } });
for (const f of ['kh-shaderc', 'kh-oodle', 'kh-er-selfcheck']) fs.copyFileSync(path.join(TL, f), path.join(OUTL, 'tools', 'bin', f));
// Node.js linux-x64: the pinned tarball (checked against its SHASUMS256.txt hash), cached; its bin/node and LICENSE
const nodeTgz = path.join(CACHE, path.basename(LX.tgz_url));
if (!fs.existsSync(nodeTgz) || sha256f(nodeTgz) !== LX.tgz_sha256) {
  console.log(`> download ${LX.tgz_url}`);
  const d = cp.spawnSync('powershell', ['-NoProfile', '-Command', `$ProgressPreference='SilentlyContinue'; Invoke-WebRequest -UseBasicParsing -Uri '${LX.tgz_url}' -OutFile '${qps(nodeTgz)}'`], { stdio: 'inherit' });
  if (d.status !== 0 || !fs.existsSync(nodeTgz)) fail('the Node.js linux-x64 download failed');
}
if (sha256f(nodeTgz) !== LX.tgz_sha256) fail(`${nodeTgz} is not the pinned Node.js linux-x64 tarball (SHA-256 of ${PIN.shasums_url})`);
const nodeEntries = tar.list(fs.readFileSync(nodeTgz));
const linuxNode = nodeEntries.find((e) => e.name === `${LX.tgz_folder}/bin/node`);
const linuxLicense = nodeEntries.find((e) => e.name === `${LX.tgz_folder}/LICENSE`);
if (!linuxNode || require('crypto').createHash('sha256').update(linuxNode.data).digest('hex') !== LX.node_sha256) fail('bin/node of the linux-x64 tarball is not the pinned one (release-tools/node_pin.json linux_x64)');
const linuxNodeFile = path.join(CACHE, `${LX.tgz_folder}-node`);
fs.writeFileSync(linuxNodeFile, linuxNode.data);
const SETUP_LINUX = path.join(OUTL, 'setup-linux');
run(nodeExe, [path.join(REPO, 'release-tools', 'sea_build.mjs'), '--target', 'linux-x64', '--node', linuxNodeFile, '--blob-node', nodeExe, '--tools', SEA_TOOLS, '--work', path.join(CACHE, 'sea_work_linux'), '--out', SETUP_LINUX, '--version', VERSION]);
const linuxSha = sha256f(SETUP_LINUX);
const linuxRecord = JSON.stringify({ about: 'setup-linux of the linux-x64 release (release-tools/make_release.js): Node.js ' + PIN.version + ' linux-x64 (bin/node ' + LX.node_sha256 + ') as a single executable application of release-tools/sea_main.js (the same dispatcher as setup.exe); release-tools/scan.js allows setup-linux by this hash. The build is deterministic.', version: VERSION, node: PIN.version, sea_main_sha256: sha256f(path.join(REPO, 'release-tools', 'sea_main.js')), setup_linux_sha256: linuxSha, bytes: fs.statSync(SETUP_LINUX).size }, null, 1) + '\n';
fs.writeFileSync(path.join(OUTL, 'release-tools', 'setup_linux.json'), linuxRecord);
fs.writeFileSync(path.join(REPO, 'release-tools', 'setup_linux.json'), linuxRecord);
// the notices: Node.js for Linux (its own LICENSE) in place of node.exe's, musl (the C library in the static Linux tools)
const MUSL = MIT('2005-2020 Rich Felker, et al.') + '\n(musl\'s full COPYRIGHT file, with the list of its authors: https://git.musl-libc.org/cgit/musl/tree/COPYRIGHT)\n';
let noticesL = fs.readFileSync(path.join(OUT, 'THIRD-PARTY-NOTICES.txt'), 'utf8');
const nodeHead = `==== Node.js ${PIN.version} (built into setup.exe: a Node.js single executable application) - ${PIN.zip_url}`;
const nodeAt = noticesL.indexOf(nodeHead), nextAt = noticesL.indexOf('\n==== ', nodeAt + 1);
if (nodeAt < 0 || nextAt < 0 || !linuxLicense) fail('THIRD-PARTY-NOTICES.txt: the Node.js section was not found');
noticesL = noticesL.slice(0, nodeAt) + `==== Node.js ${PIN.version} for Linux (built into setup-linux: a Node.js single executable application) - ${LX.tgz_url}\n(the MIT licence of Node.js, then the licences of the components built into bin/node)\n\n${linuxLicense.data.toString('utf8')}\n\n` +
  `==== musl libc (statically linked into tools/bin/kh-shaderc, kh-oodle and kh-er-selfcheck, the Linux builds of our tools) - https://musl.libc.org\n\n${MUSL}\n` + noticesL.slice(nextAt + 1);
fs.writeFileSync(path.join(OUTL, 'THIRD-PARTY-NOTICES.txt'), noticesL);
// the checks: the scan (with the Linux binaries) and the binaries' content, as for the win64 folder
const problemsL = [];
const scL = cp.spawnSync(process.execPath, [path.join(OUTL, 'release-tools', 'scan.js'), '--release-binaries'], { cwd: OUTL, encoding: 'utf8', env: { ...process.env, KH_SCAN_PATHS: [process.env.KH_SCAN_PATHS, path.dirname(REPO)].filter(Boolean).join(';') } });
process.stdout.write(scL.stdout + scL.stderr);
if (scL.status !== 0) problemsL.push('release-tools/scan.js found problems');
const reportL = { version: VERSION, platform: 'linux-x64', binaries: {} };
for (const rel of ['er/mods/kh_er.dll', 'er/mods/kh_er_loader.dll', 'tools/bin/kh-shaderc', 'tools/bin/kh-oodle', 'tools/bin/kh-er-selfcheck']) {
  const b = fs.readFileSync(path.join(OUTL, rel));
  const latin = b.toString('latin1'), utf8 = b.toString('utf8');
  const r = { bytes: b.length, japanese: (utf8.match(new RegExp(JP.source, 'g')) || []).length, game_text_lines: [], runtime_table_strings: 0, developer_paths: 0 };
  for (const t of gameText) { const probe = t.slice(0, 40); if (probe.length >= 24 && latin.includes(probe)) r.game_text_lines.push(probe); }
  r.runtime_table_strings = runtimeSamples.filter((s) => latin.includes(s)).length;
  r.developer_paths = (latin.match(/[A-Za-z]:[\\/]+Users[\\/]+/g) || []).length + (latin.match(/\/home\/[A-Za-z0-9_.-]+\//g) || []).length + [path.dirname(REPO), path.dirname(REPO).split(path.sep).join('/')].reduce((n, p) => n + latin.split(p).length - 1, 0);
  reportL.binaries[rel] = r;
  console.log(`${rel}: ${(b.length / 1048576).toFixed(1)} MB; Japanese runs ${r.japanese}; game text lines ${r.game_text_lines.length}; run-time table strings ${r.runtime_table_strings}/${runtimeSamples.length}; developer paths ${r.developer_paths}`);
  if (r.japanese || r.game_text_lines.length || r.runtime_table_strings || r.developer_paths) problemsL.push(`${rel}: KH3 content or a developer path in the binary`);
}
for (const f of ['setup-linux', 'er/play.sh', 'setup/setup.sh', 'setup/get_node.sh']) if (!fs.existsSync(path.join(OUTL, f))) problemsL.push(`${f} is missing`);
fs.writeFileSync(path.join(DIST, NAME + '-linux.check.json'), JSON.stringify({ ...reportL, problems: problemsL }, null, 1) + '\n');
if (problemsL.length) { for (const p of problemsL) console.error('PROBLEM: ' + p); fail('the linux-x64 release folder is not clean: no tarball made'); }
// the tarball: every file under kingdom-hearts-x-elden-ring/, sorted; programs and scripts 755, the rest 644
const EXEC = /^(setup-linux|tools\/bin\/kh-(shaderc|oodle|er-selfcheck)|er\/play\.sh|setup\/(setup|get_node)\.sh)$/;
const entries = [];
(function walk(d, rel) {
  for (const e of fs.readdirSync(d, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0))) {
    const r = rel ? rel + '/' + e.name : e.name;
    if (e.isDirectory()) { entries.push({ name: 'kingdom-hearts-x-elden-ring/' + r, dir: true }); walk(path.join(d, e.name), r); }
    else entries.push({ name: 'kingdom-hearts-x-elden-ring/' + r, mode: EXEC.test(r) ? 0o755 : 0o644, data: fs.readFileSync(path.join(d, e.name)) });
  }
})(OUTL, '');
let mtime = 0;
try { mtime = Number(cp.execFileSync('git', ['log', '-1', '--format=%ct'], { cwd: REPO, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim()) || 0; } catch { /* not a git checkout */ }
const tgz = path.join(DIST, NAME + '-linux-x64.tar.gz');
fs.writeFileSync(tgz, tar.create([{ name: 'kingdom-hearts-x-elden-ring', dir: true }, ...entries], { mtime }));
console.log(`make_release: ${tgz} (${(fs.statSync(tgz).size / 1048576).toFixed(1)} MB, ${entries.length} entries, sha256 ${sha256f(tgz)}); setup-linux ${linuxSha}; checks: ${path.join(DIST, NAME + '-linux.check.json')}`);
