#!/usr/bin/env node
// The release check: refuses anything in this repository that must not be distributed.
//
//   node release-tools/scan.js [--key-from "<KINGDOM HEARTS III.exe>" --key-offset 0x...]  [--release-binaries]
//
// Checks every file git would see (build output, node_modules, the player's generated folders are skipped):
//   - no game binaries / archives / assets by extension, and none by content (file magic: DCX, BND4, BHF4, DDS, RIFF,
//     OggS, glTF, UE package tag, PE executables);
//   - no key material: PEM blocks, and with --key-from, the exact 32 bytes of the player's KH3 pak key (read from their
//     own exe, never printed);
//   - no file over 50 MB, total size reported;
//   - no absolute paths of the developer's machine in code / config files (docs are only counted);
//   - no Japanese text (the game's text) in the pack pipeline's data (tools/packgen: recipes, notes, lists).
// --release-binaries: the folder is a release (release-tools/make_release.js): its three prebuilt binaries are allowed
// (er/mods/kh_er.dll, er/mods/kh_er_loader.dll, tools/bin/kh-shaderc.exe; make_release.js checks what is in them), and the
// setup.exe (Node.js as a single executable application of the setup) only when it is the one make_release.js built:
// the SHA-256 in release-tools/setup_exe.json (it is over 50 MB and never in git). The bundled npm packages
// (node_modules) are scanned like the rest.
// Exit code 0 = clean, 1 = something was found.
'use strict';
const fs = require('fs');
const path = require('path');

const REPO = path.resolve(__dirname, '..');
const args = process.argv.slice(2);
const arg = (k) => { const i = args.indexOf(k); return i >= 0 ? args[i + 1] : undefined; };

// Not part of the repository (git-ignored): generated, downloaded or the player's own.
const RELEASE = args.includes('--release-binaries');
const RELEASE_BINARY = /^(er\/mods\/kh_er\.dll|er\/mods\/kh_er_loader\.dll|tools\/bin\/kh-shaderc\.exe)$/;
const SKIP = RELEASE ? /^(\.git|target|build|assets|assets_converted|audit|user|er\/(saves|logs|live))(\/|$)|^er\/mods\/kh-er\.(cfg|ini|ctl|ctl\.bak\d*|selfcheck\.txt)$|^er\/mods\/kh-er-selfcheck\.txt$|^tools\/config\.json$|^tools\/third_party\/ogg-vorbis-decoder\/ogg-vorbis-decoder\.min\.js$/ : /^(\.git|node_modules|target|build|assets|assets_converted|audit|user|tools\/bin|er\/(mods|saves|logs|live))(\/|$)|^tools\/config\.json$|^tools\/third_party\/ogg-vorbis-decoder\/ogg-vorbis-decoder\.min\.js$|^sheets\/(reflect|entities|systems|exe|native|data|assets|impl|design)(\/|$)/;
const BANNED_EXT = /\.(exe|dll|pdb|pak|sig|uasset|uexp|ubulk|umap|bnk|wem|awb|acb|usm|bk2|glb|gltf|dds|tga|png|jpg|jpeg|wav|ogg|mp3|mp4|bhd|bdt|dcx|bnd|sl2|co2|khsave|bin|fmg|emevd|flver|tpf|hks)$/i;
const HUD_DXIL = /^crates\/kh-er-render\/shaders\/hud\/hud\.(ps|vs)\.dxil$/;
// the bundled Node.js: allowed by its hash only
const NODE_EXE = 'setup.exe';
const NODE_PIN = (() => { try { return JSON.parse(fs.readFileSync(path.join(__dirname, 'setup_exe.json'), 'utf8')); } catch { return null; } })();
const nodeIsPinned = (abs) => !!NODE_PIN && require('crypto').createHash('sha256').update(fs.readFileSync(abs)).digest('hex') === NODE_PIN.setup_exe_sha256;
let nodeOk = false;
// the mod's own logo (the owner's fan art, docs/images): the only images allowed, each by its SHA-256
const OWN_IMAGES = {
  'docs/images/logo.png': '4c83e7789dc09a59b941c3d0359ba73957e24c7c40eb5da778283d5a90d41c2f',
  'docs/images/logo-640.png': '85233767cf9772a2bde8275876c10679eb2893cc57e6bf10977635de9adf475c',
};
let imageOk = false;
const ALLOWED_BINARY = { test: (r) => HUD_DXIL.test(r) || (RELEASE && RELEASE_BINARY.test(r)) || (RELEASE && r === NODE_EXE && nodeOk) || (imageOk && r in OWN_IMAGES) };
// the pack pipeline's data (recipes, notes, lists) holds no text of the game's: no Japanese at all
const JP = /[぀-ヿ㐀-鿿ｦ-ﾟ]/;
const JP_FREE = /^tools\/packgen\//;
const MAGIC = [['DCX\0', 'DCX archive'], ['BND4', 'BND4 archive'], ['BHF4', 'BHF4 index'], ['BDF4', 'BDF4 data'], ['DDS ', 'DDS texture'], ['RIFF', 'RIFF audio'], ['OggS', 'Ogg audio'], ['glTF', 'glTF model'], ['TPF\0', 'TPF textures'], ['FLVE', 'FLVER model']];
const UE_TAG = Buffer.from([0xc1, 0x83, 0x2a, 0x9e]);
const CODE = /\.(rs|js|toml|json|ps1|bat|cmd|sh|me3|ini|ctl|cfg|wgsl|hlsl)$/i;
// absolute paths of the machine the scan runs on: its user profile folder (os.homedir()) and any folder named in
// KH_SCAN_PATHS (";"-separated: the developer's working folders; make_release.js passes the one it builds in)
const esc = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&').split(/\\\\|\//).join('[\\\\/]{1,2}');
const ABS = new RegExp([require('os').homedir(), ...(process.env.KH_SCAN_PATHS || '').split(';')].map((s) => s.trim()).filter((s) => s.length > 3).map(esc).join('|'), 'i');
const LIMIT = 50 * 1024 * 1024;

let key = null;
if (arg('--key-from')) {
  const off = parseInt(arg('--key-offset'), 16);
  const fd = fs.openSync(arg('--key-from'), 'r');
  key = Buffer.alloc(32); fs.readSync(fd, key, 0, 32, off); fs.closeSync(fd);
}

const problems = [], notes = [];
let total = 0, count = 0, docPaths = 0;
function walk(dir) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const abs = path.join(dir, e.name);
    const r = path.relative(REPO, abs).split(path.sep).join('/');
    if (SKIP.test(r)) continue;
    if (e.isDirectory()) { walk(abs); continue; }
    if (!e.isFile()) continue;
    const size = fs.statSync(abs).size;
    total += size; count++;
    nodeOk = RELEASE && r === NODE_EXE && nodeIsPinned(abs);
    imageOk = r in OWN_IMAGES && require('crypto').createHash('sha256').update(fs.readFileSync(abs)).digest('hex') === OWN_IMAGES[r];
    if (r in OWN_IMAGES && !imageOk) problems.push(`${r}: not the mod's logo as allowed (release-tools/scan.js OWN_IMAGES)`);
    if (RELEASE && r === NODE_EXE && !nodeOk) problems.push(`${r}: not the setup.exe make_release.js built (release-tools/setup_exe.json)`);
    if (size > LIMIT && !nodeOk) problems.push(`${r}: ${(size / 1048576).toFixed(1)} MB (over 50 MB)`);
    if (BANNED_EXT.test(r) && !ALLOWED_BINARY.test(r)) problems.push(`${r}: banned file type`);
    const buf = fs.readFileSync(abs);
    const head = buf.subarray(0, 4).toString('latin1');
    for (const [m, what] of MAGIC) if (head === m) problems.push(`${r}: looks like a game file (${what})`);
    if (buf.subarray(0, 4).equals(UE_TAG)) problems.push(`${r}: looks like an Unreal package`);
    if (head.startsWith('MZ') && !ALLOWED_BINARY.test(r)) problems.push(`${r}: a Windows executable`);
    const text = buf.toString('latin1');
    if (/-----BEGIN [A-Z ]*(PUBLIC|PRIVATE) KEY-----\s*\n?[A-Za-z0-9+/=\r\n]{40,}/.test(text)) problems.push(`${r}: contains a PEM key block`);
    if (key && buf.indexOf(key) >= 0) problems.push(`${r}: contains the KH3 pak key`);
    if (key && buf.indexOf(Buffer.from(key.toString('hex'))) >= 0) problems.push(`${r}: contains the KH3 pak key as hex`);
    if (JP_FREE.test(r) && JP.test(buf.toString('utf8'))) problems.push(`${r}: Japanese text (the game's text) in the pack pipeline's data`);
    if (ABS.test(text) && !r.startsWith('release-tools/') && !(RELEASE && RELEASE_BINARY.test(r))) { if (CODE.test(r)) problems.push(`${r}: absolute path of the developer's machine`); else docPaths++; }
  }
}
walk(REPO);
if (docPaths) notes.push(`${docPaths} documentation file(s) mention paths of the developer's machine (notes only, not used to build or run)`);
console.log(`scan: ${count} files, ${(total / 1048576).toFixed(1)} MB${key ? ', KH3 pak key checked' : ''}`);
for (const n of notes) console.log('  note: ' + n);
for (const p of problems) console.log('  PROBLEM: ' + p);
console.log(problems.length ? `scan: ${problems.length} problem(s)` : 'scan: clean');
process.exit(problems.length ? 1 : 0);
