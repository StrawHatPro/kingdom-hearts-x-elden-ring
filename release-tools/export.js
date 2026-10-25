#!/usr/bin/env node
// Refreshes this repository's copy of the mod's source from the developer's working tree.
//
//   node release-tools/export.js --dev "<path of the dev tree>" [--patches DIR] [--dry-run]
//
// What it does:
//   - deletes and re-copies only the folders and files it manages (MANAGED below). The hand-written parts of the repo
//     (README.md, LEGAL.md, setup/, er/, release-tools/, package.json, .gitignore) are never touched;
//   - copies only the Elden Ring mod: the kh-er DLL, the loader, their dependency crates, the shader compiler, the
//     vendored Elden Ring SDK (with its licence files), the pack / sheet tools, the port sheets and the docs;
//   - leaves out everything in EXCLUDE (game files, decompiler tooling, build output, test-only crates);
//   - applies the TRANSFORMS (game text removed, machine paths made relative);
//   - then runs release-tools/scan.js, which refuses game files, keys and oversized files.
//
// It reads the dev tree and writes only inside this repository. It never runs git: committing is left to you.
'use strict';
const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const REPO = path.resolve(__dirname, '..');
const args = process.argv.slice(2);
const arg = (k) => { const i = args.indexOf(k); return i >= 0 ? args[i + 1] : undefined; };
const DEV = arg('--dev');
const DRY = args.includes('--dry-run');
// The release patches (code changes of the released mod) are kept OUTSIDE the repository, because they quote the code
// they remove: default <repo>/../<repo folder name>.patches.
const PATCHES = path.resolve(arg('--patches') || path.join(REPO, '..', path.basename(REPO) + '.patches'));
if (!DEV || !fs.existsSync(path.join(DEV, 'crates', 'kh-er', 'Cargo.toml'))) {
  console.error('usage: node release-tools/export.js --dev "<dev tree with crates/kh-er>" [--dry-run]');
  process.exit(2);
}
// the development tree's own path, as its files write it (never written into this repository's tools)
const DEV_SLASH = path.resolve(DEV).split(path.sep).join('/');
if (path.resolve(DEV) === REPO) { console.error('the dev tree and this repository are the same folder'); process.exit(2); }

// Paths (relative, forward slashes) this script owns in the repo. Everything else in the repo is hand-written.
const MANAGED = ['crates', 'vendor', 'tools', 'sheets/port', 'docs', 'documentation', 'Cargo.toml', 'Cargo.lock'];

// What is copied from the dev tree.
const COPY = [
  'Cargo.toml', 'Cargo.lock',
  'crates', 'vendor/fromsoftware-rs', 'tools', 'sheets/port', 'documentation',
  'docs/formats', /^docs\/(sora|party)-[^/]+\.md$/,
];

// Never copied (tested against the relative path; a folder rule excludes everything under it).
const EXCLUDE = [
  // build output, dependencies, VCS, scratch
  /(^|\/)(target|node_modules|\.git)(\/|$)/, /\.(orig|rej|log|bak)$/i, /(^|\/)\{$/,
  // the KH3 test level (Bevy) and its capture replayer: not part of the Elden Ring mod
  /^crates\/kh-(app|replay)(\/|$)/,
  // screenshots of the games in the SDK's README
  /^vendor\/fromsoftware-rs\/examples\/img(\/|$)/,
  // decompiler / reassembly / differential-test tooling of the KH3 decompilation (not needed by the mod)
  /^tools\/ghidra(\/|$)/, /^tools\/ghidra[^/]*\.ps1$/, /^tools\/native(\/|$)/,
  /^tools\/third_party\/(zlib-1\.2\.8|hde64)(\/|$)/,
  /^tools\/(build|check|check_asserts|split|lower|promote|verify_l2|gen|gen_prelude|gen_structs|link|native_sheets|steamstub|docs_gen|formats_readme|mesh_mutation|probe_fields|probe_test|harness_smoke|port_smoke)\.js$/,
  // the mod's development aids (test-only module; its call sites are removed by the release patch)
  /^crates\/kh-er\/src\/testaid\.rs$/,
  // briefs written for development agents
  /^docs\/sora-agent-brief\.md$/,
  /^tools\/l2_[^/]*\.js$/,/^tools\/probe_fields\.spec\.json$/,
  // the bundled Ogg Vorbis decoder is fetched from npm by the setup tool (licence texts stay with the package)
  /^tools\/third_party\/ogg-vorbis-decoder\/(ogg-vorbis-decoder\.min\.js|LICENSE\.codec-parser)$/,
  // machine-specific config (the setup tool writes it)
  /^tools\/config\.json$/,
  // (PACKGEN) the KH1 outfit's own voice (the developer's KH1 clips): not part of the release; the KH1 outfit speaks with
  // KH3 Sora's battle voice (crates/kh-er/src/boc.rs, packgen.patch)
  /^tools\/sora_outfit_voice\.js$/, /^sheets\/port\/sora-outfit-voice\.json$/,
  // (PACKGEN) the development capture of tools/packgen_notes.js, and the tools' downloaded / built binaries
  /^tools\/packgen\/notes\/\.capture\.json$/, /^tools\/bin(\/|$)/,
  // (recipes v2) the file recipes are the release's own: compiled with this repository's tools/packgen_recipe.js (v2:
  // every part the same as a file made from the player's KH3 install is a reference) against a clean-room build:
  //   node tools/packgen_recipe.js compile --sandbox <clean-room> --dev <dev tree>   (and again with --overlays)
  // The development tree's own recipes (v1) are not copied; the repository's are kept (RELEASE_OWNED).
  /^tools\/packgen\/replay\/(files|overlays)_[^/]*\.json$/, /^tools\/packgen\/replay\/report(_overlays)?\.json$/,
];
// Paths under MANAGED that this repository owns (kept as they are when the managed paths are copied again)
// (also the release's own documents in docs/: the build guide and the mod's logo)
const RELEASE_OWNED = /^tools\/packgen\/replay\/((files|overlays)_[^/]*|report(_overlays)?)\.json$|^docs\/(BUILDING\.md|images\/[^/]+)$/;

// Extensions that must never be copied, whatever folder they are in (game binaries, archives, assets, saves).
const BANNED_EXT = /\.(exe|dll|pdb|pak|sig|uasset|uexp|ubulk|umap|bnk|wem|awb|acb|usm|bk2|glb|gltf|dds|tga|png|jpg|jpeg|wav|ogg|mp3|mp4|bhd|bdt|dcx|bnd|sl2|co2|khsave|bin|fmg|emevd|flver|tpf|hks)$/i;
// Our own compiled HUD shader (built from crates/kh-er-render/shaders/hud/hud.hlsl) is the one allowed binary.
const ALLOWED_BINARY = /^crates\/kh-er-render\/shaders\/hud\/hud\.(ps|vs)\.dxil$/;

const rel = (p) => p.split(path.sep).join('/');
const excluded = (r) => EXCLUDE.some((re) => re.test(r)) || (BANNED_EXT.test(r) && !ALLOWED_BINARY.test(r));

function walk(dir, base, out) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const abs = path.join(dir, e.name);
    const r = rel(path.relative(base, abs));
    if (excluded(r)) continue;
    if (e.isDirectory()) walk(abs, base, out);
    else if (e.isFile()) out.push(r);
  }
}

// ---- the list of files to copy
const files = [];
for (const c of COPY) {
  if (c instanceof RegExp) {
    const top = c.source.match(/^\^([a-z]+)\\\//)[1];
    for (const e of fs.readdirSync(path.join(DEV, top))) { const r = `${top}/${e}`; if (c.test(r) && !excluded(r)) files.push(r); }
    continue;
  }
  const abs = path.join(DEV, c);
  if (!fs.existsSync(abs)) { console.error(`missing in the dev tree: ${c}`); process.exit(1); }
  if (fs.statSync(abs).isDirectory()) walk(abs, DEV, files); else if (!excluded(c)) files.push(c);
}

// ---- transforms (applied to the copy, never to the dev tree)
// Files the mod's crates share as source from a crate that is not copied (the loadout reader lived in the test level's
// crate): copied to a place inside the copied crates, and the #[path] attributes pointed at it (TRANSFORMS below).
const RELOCATE = { 'crates/kh-app/src/loadout.rs': 'crates/kh-host/src/loadout_shared.rs' };
const LOADOUT_PATH = '#[path = "../../kh-app/src/loadout.rs"]';

const TRANSFORMS = {
  'crates/kh-host/src/rig.rs': (text) => replaceAll(text, LOADOUT_PATH, '#[path = "loadout_shared.rs"]'),
  'crates/kh-er/src/sora.rs': (text) => replaceAll(text, LOADOUT_PATH, '#[path = "../../kh-host/src/loadout_shared.rs"]'),
  'crates/kh-er-render/examples/hud.rs': (text) => replaceAll(text, LOADOUT_PATH, '#[path = "../../kh-host/src/loadout_shared.rs"]'),
  // The help lines KH3 shows for its abilities are the game's text: only the lines we wrote ourselves (our own
  // abilities, port = "ours") are kept; the menu takes the game's lines from the UI pack the setup builds.
  'sheets/port/sora-abilities.json': (text) => {
    const lines = text.split('\n');
    const head = JSON.parse(lines[0].replace(/"rows":\[\s*$/, '"rows":[]}'));
    const ci = head.columns.indexOf('help_text'), pi = head.columns.indexOf('port');
    if (ci < 0 || pi < 0) throw new Error('sora-abilities: help_text / port column not found');
    let scrubbed = 0;
    const out = lines.map((line, n) => {
      if (n === 0 || !line.startsWith('[')) return line;
      const comma = line.endsWith(',');
      const row = JSON.parse(comma ? line.slice(0, -1) : line);
      if (row[pi] !== 'ours' && row[ci] !== '-') { row[ci] = '-'; scrubbed++; }
      return JSON.stringify(row) + (comma ? ',' : '');
    });
    console.log(`  sora-abilities.json: ${scrubbed} game help lines removed`);
    return out.join('\n');
  },
  // Test-only fallback path to the developer's tree: made relative to the workspace.
  'crates/kh-er/src/gamever.rs': (text) => replaceAll(text, `std::path::Path::new("${DEV_SLASH}")`, 'std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))'),
  // The workspace: only the Elden Ring mod's crates (crates/* = what was copied); comments about the test level dropped.
  'Cargo.toml': (text) => {
    text = replaceAll(text, '# The Rust side of the Sora port (docs/sora-port-plan.md). ONE shared target directory: target/ at\n# this root. Do not build with --target-dir or CARGO_TARGET_DIR pointing elsewhere.\n',
      '# Kingdom Hearts x Elden Ring: the Elden Ring mod (kh-er = kh_er.dll, kh-er-loader = kh_er_loader.dll), its crates and\n# the shader compiler (kh-shaderc). One target directory: target/ at this root.\n');
    text = replaceAll(text, '# Release is what build.ps1 builds and what play.bat runs. Tuned for build time, not for the last\n# few percent: no LTO, parallel codegen, incremental, no debug info.\n',
      '# Release is what setup/setup.js builds. Tuned for build time, not for the last few percent: no LTO, parallel\n# codegen, incremental, no debug info.\n');
    text = replaceAll(text, '# (PORT) Elden Ring 2.7.2.0 (2026-10-21 patch): the pinned SDK (Dasaav-dsv/fromsoftware-rs rev 79266c9, unchanged otherwise)\n# is vendored with one addition, the 2.7.2.0 RVA table its own binary-mapper produced (vendor/fromsoftware-rs/crates/\n# eldenring/src/rva/rva_ww2720.rs; evidence build/sora_tmp/er/port272/INVENTORY.md). Both 2.7.1.0 and 2.7.2.0 are served.\n',
      '# Elden Ring 2.7.2.0 (2026-10-21 patch): the pinned SDK (Dasaav-dsv/fromsoftware-rs rev 79266c9, MIT OR Apache-2.0, unchanged\n# otherwise) is vendored with one addition, the 2.7.2.0 RVA table its own binary-mapper produced (vendor/fromsoftware-rs/\n# crates/eldenring/src/rva/rva_ww2720.rs). Both 2.7.1.0 and 2.7.2.0 are served.\n', true);
    return text;
  },
};
// Japanese text quoted from the game's data (asset descriptions, mission and region labels, ability help lines in the
// evidence columns) is removed from every port sheet: each run of Japanese characters becomes "…". The `name` column
// (short item / weapon names the code looks rows up by) is kept.
const JP = /[　-ヿ㐀-鿿豈-﫿＀-￯]+/g;
function scrubJapanese(r, text) {
  if (!JP.test(text)) return text;
  JP.lastIndex = 0;
  const lines = text.split('\n');
  const sheet = JSON.parse(text);
  const nameCol = (sheet.columns || []).indexOf('name');
  const hasJp = (v) => { const t = JSON.stringify(v); JP.lastIndex = 0; const y = JP.test(t); JP.lastIndex = 0; return y; };
  // a sheet whose only Japanese text is in the kept name column is copied as it is (whatever its layout)
  if (nameCol >= 0 && (sheet.rows || []).every((row) => row.every((v, i) => i === nameCol || !hasJp(v)))) return text;
  const deep = (v) => typeof v === 'string' ? v.replace(JP, '…') : Array.isArray(v) ? v.map(deep) : v && typeof v === 'object' ? Object.fromEntries(Object.entries(v).map(([k, x]) => [k, deep(x)])) : v;
  let n = 0;
  const out = lines.map((line) => {
    if (!line.startsWith('[') || !JP.test(line)) { JP.lastIndex = 0; return line; }
    JP.lastIndex = 0;
    const comma = line.endsWith(',');
    const row = JSON.parse(comma ? line.slice(0, -1) : line);
    const next = row.map((v, i) => (i === nameCol ? v : deep(v)));
    const s = JSON.stringify(next);
    if (s !== JSON.stringify(row)) n++;
    return s + (comma ? ',' : '');
  });
  const result = out.join('\n');
  // what is left must be only the kept name column, else the sheet's layout was not one row per line
  const left = result.split('\n').filter((l) => JP.test(l) && (JP.lastIndex = 0, true));
  JP.lastIndex = 0;
  if (left.length && nameCol < 0) throw new Error(`${r}: Japanese text left on ${left.length} line(s) (layout not one row per line?)`);
  if (n) console.log(`  ${r}: Japanese game text removed from ${n} row(s)`);
  return result;
}
function replaceAll(text, from, to, optional) {
  if (!text.includes(from)) { if (optional) return text; throw new Error(`transform: text not found: ${from.slice(0, 80)}`); }
  return text.split(from).join(to);
}

console.log(`export: ${files.length} files from ${DEV}`);
if (DRY) { for (const f of files) console.log('  ' + f); process.exit(0); }

// ---- write: remove the managed paths, copy, transform
// (the release-owned files under the managed paths are kept: moved aside, then put back)
const KEEP = path.join(REPO, '..', path.basename(REPO) + '.export_keep');
fs.rmSync(KEEP, { recursive: true, force: true });
const kept = [];
(function keepWalk(dir) { if (!fs.existsSync(dir)) return; for (const e of fs.readdirSync(dir, { withFileTypes: true })) { const p = path.join(dir, e.name); const r = rel(path.relative(REPO, p)); if (e.isDirectory()) keepWalk(p); else if (RELEASE_OWNED.test(r)) { fs.mkdirSync(path.dirname(path.join(KEEP, r)), { recursive: true }); fs.renameSync(p, path.join(KEEP, r)); kept.push(r); } } })(path.join(REPO, 'tools'));
for (const d of ['docs']) (function keepWalk2(dir) { if (!fs.existsSync(dir)) return; for (const e of fs.readdirSync(dir, { withFileTypes: true })) { const p = path.join(dir, e.name); const r = rel(path.relative(REPO, p)); if (e.isDirectory()) keepWalk2(p); else if (RELEASE_OWNED.test(r)) { fs.mkdirSync(path.dirname(path.join(KEEP, r)), { recursive: true }); fs.renameSync(p, path.join(KEEP, r)); kept.push(r); } } })(path.join(REPO, d));
for (const m of MANAGED) fs.rmSync(path.join(REPO, m), { recursive: true, force: true });
for (const r of kept) { fs.mkdirSync(path.dirname(path.join(REPO, r)), { recursive: true }); fs.renameSync(path.join(KEEP, r), path.join(REPO, r)); }
fs.rmSync(KEEP, { recursive: true, force: true });
if (kept.length) console.log(`  ${kept.length} release-owned file(s) kept (the v2 recipes, docs/BUILDING.md, docs/images)`);
for (const r of files) {
  const dst = path.join(REPO, r);
  fs.mkdirSync(path.dirname(dst), { recursive: true });
  if (TRANSFORMS[r] || /^sheets\/port\/[^/]+\.json$/.test(r)) {
    let text = fs.readFileSync(path.join(DEV, r), 'utf8');
    if (TRANSFORMS[r]) text = TRANSFORMS[r](text);
    if (r.startsWith('sheets/port/')) text = scrubJapanese(r, text);
    fs.writeFileSync(dst, text);
  } else fs.copyFileSync(path.join(DEV, r), dst);
}
for (const [from, to] of Object.entries(RELOCATE)) {
  fs.mkdirSync(path.dirname(path.join(REPO, to)), { recursive: true });
  fs.copyFileSync(path.join(DEV, from), path.join(REPO, to));
  console.log(`  ${from} -> ${to}`);
}
for (const t of Object.keys(TRANSFORMS)) if (!files.includes(t)) { console.error(`transform target not copied: ${t}`); process.exit(1); }

// (PACKGEN) The research scripts the pack build runs on the player's PC (they live in the development tree's build/sora_tmp;
// the list is tools/packgen.js --research-files): copied into tools/research, with the development tree's absolute paths
// made relative to the repository (tools/packgen.js sets KH_PACKGEN_ROOT).
{
  const list = JSON.parse(execFileSync(process.execPath, [path.join(DEV, 'tools', 'packgen.js'), '--research-files'], { encoding: 'utf8' }));
  const Q = "'";
  for (const e of list) {
    const [f, from] = e.includes('<=') ? e.split('<=') : [e, e]; // ("A<=B": staged as A, the development tree's B)
    const s = path.join(DEV, 'build', 'sora_tmp', from), d = path.join(REPO, 'tools', 'research', f);
    fs.mkdirSync(path.dirname(d), { recursive: true });
    let b = fs.readFileSync(s);
    if (/\.js$/.test(f)) {
      let t = b.toString('utf8');
      t = t.split(Q + DEV_SLASH + Q).join('process.env.KH_PACKGEN_ROOT.slice(0, -1)');
      t = t.split(Q + DEV_SLASH + '/').join('process.env.KH_PACKGEN_ROOT + ' + Q);
      if (t.includes(DEV_SLASH) || t.includes(DEV_SLASH.split('/').join('\\')) || /[A-Za-z]:[\\/]+Users[\\/]/.test(t)) { console.error(`research file ${f}: an absolute development path is left`); process.exit(1); }
      b = Buffer.from(t, 'utf8');
    }
    fs.writeFileSync(d, b);
  }
  console.log(`  ${list.length} research files -> tools/research`);
}
// (PACKGEN) Files only the release has (release-tools/overlay: the run-time table crate kh-gendata), copied into place.
(function overlay(dir, base) {
  if (!fs.existsSync(dir)) return;
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) { overlay(p, base); continue; }
    const d = path.join(REPO, path.relative(base, p)); fs.mkdirSync(path.dirname(d), { recursive: true }); fs.copyFileSync(p, d);
  }
})(path.join(__dirname, 'overlay'), path.join(__dirname, 'overlay'));

// Code changes of the released mod (PATCHES/*.patch, applied in name order): no-dev-aids.patch removes the
// development aids (the test-aid module, the extra control file, the instant-Zettaflare aid) and makes kh-er ignore every
// development / cheat key unless it is built with the cargo feature `dev-aids` (off by default, never used by the setup).
if (!fs.existsSync(path.join(PATCHES, 'no-dev-aids.patch'))) { console.error(`the release patches are missing: ${PATCHES} (give --patches DIR)`); process.exit(1); }
for (const p of fs.readdirSync(PATCHES).filter((f) => f.endsWith('.patch')).sort()) {
  execFileSync('git', ['apply', '--whitespace=nowarn', path.join(PATCHES, p)], { cwd: REPO, stdio: 'inherit' });
  console.log(`  patch applied: ${p}`);
}

// Cargo.lock came from the dev workspace (with crates not copied here): cargo resolves the workspace once, which drops
// their entries and changes no version.
try {
  execFileSync('cargo', ['metadata', '--format-version', '1', '--offline'], { cwd: REPO, stdio: ['ignore', 'ignore', 'inherit'] });
  console.log('  Cargo.lock: pruned to this workspace');
} catch { console.log('  Cargo.lock: NOT pruned (cargo metadata failed): run any cargo command once'); }

// The decoder's README: say where the bundle comes from now.
const oggReadme = path.join(REPO, 'tools/third_party/ogg-vorbis-decoder/README.txt');
if (fs.existsSync(oggReadme)) fs.appendFileSync(oggReadme, '\nIn this repository the bundle is NOT included: setup/setup.js copies it from node_modules after `npm install`\n(package @wasm-audio-decoders/ogg-vorbis 0.1.20, MIT; its licence texts come with the npm package).\n');

console.log('export: done; scanning');
execFileSync(process.execPath, [path.join(__dirname, 'scan.js')], { stdio: 'inherit' });
