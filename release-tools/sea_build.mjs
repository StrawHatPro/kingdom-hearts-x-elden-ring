// Builds setup.exe: Node.js's single executable application (SEA) of release-tools/sea_main.js, on the pinned node.exe
// (release-tools/node_pin.json). Run by release-tools/make_release.js:
//
//   node release-tools/sea_build.mjs --node <pinned node.exe> --tools <folder with node_modules/{postject,resedit}>
//                                    --work <scratch folder> --out <setup.exe> --version <release version>
//
// 1. the SEA blob: <node.exe> --experimental-sea-config (main = sea_main.js; no code cache, no snapshot: the same bytes
//    on every build)
// 2. node.exe copied, its Authenticode signature dropped (an injected exe would carry a broken one) and its version
//    information set to the mod's setup (resedit)
// 3. the blob injected as the NODE_SEA_BLOB resource with Node's SEA fuse (postject)
// Deterministic: the same inputs give the same setup.exe.
//
// --target linux-x64 (with --node the pinned linux-x64 bin/node, release-tools/node_pin.json linux_x64): setup-linux, the
// same dispatcher on the Linux Node: the blob (made by the build machine's Node: no code cache and no snapshot, so it
// is the same on any machine), the ELF copied unchanged (there is no signature or version resource to change) and the
// blob injected with the same fuse (postject writes it into the ELF as a note). Also deterministic.
import fs from 'node:fs';
import path from 'node:path';
import cp from 'node:child_process';
import crypto from 'node:crypto';
import { pathToFileURL, fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const args = process.argv.slice(2);
const opt = (k) => { const i = args.indexOf(k); if (i < 0 || i + 1 >= args.length) throw new Error(`sea_build: ${k} is needed`); return path.resolve(args[i + 1]); };
const NODE = opt('--node'), TOOLS = opt('--tools'), WORK = opt('--work'), OUT = opt('--out');
const VERSION = args[args.indexOf('--version') + 1];
const HERE = path.dirname(fileURLToPath(import.meta.url));
const FUSE = 'NODE_SEA_FUSE_fce680ab2cc467b6e072b8b5df1996b2';

fs.rmSync(WORK, { recursive: true, force: true }); fs.mkdirSync(WORK, { recursive: true });
// ---- 1. the blob
const main = path.join(WORK, 'sea_main.js'); fs.copyFileSync(path.join(HERE, 'sea_main.js'), main);
const blob = path.join(WORK, 'sea-prep.blob');
fs.writeFileSync(path.join(WORK, 'sea-config.json'), JSON.stringify({ main: 'sea_main.js', output: 'sea-prep.blob', disableExperimentalSEAWarning: true, useCodeCache: false, useSnapshot: false }, null, 1));
const TARGET = args.includes('--target') ? args[args.indexOf('--target') + 1] : 'win-x64';
if (TARGET !== 'win-x64' && TARGET !== 'linux-x64') throw new Error(`sea_build: --target ${TARGET}: win-x64 or linux-x64`);
// (linux-x64: the Linux node cannot run here; the pinned Node of the build machine makes the blob, --blob-node: the same
// Node version, and without code cache or snapshot the blob does not depend on the machine)
const BLOB_NODE = TARGET === 'linux-x64' ? (args.includes('--blob-node') ? opt('--blob-node') : process.execPath) : NODE;
const r = cp.spawnSync(BLOB_NODE, ['--experimental-sea-config', path.join(WORK, 'sea-config.json')], { cwd: WORK, stdio: 'inherit' });
if (r.status !== 0 || !fs.existsSync(blob)) throw new Error('sea_build: the SEA blob was not made');
if (TARGET === 'linux-x64') {
  const elf = fs.readFileSync(NODE);
  if (elf.toString('latin1', 0, 4) !== '\x7fELF' || elf[4] !== 2 || elf.readUInt16LE(18) !== 0x3e) throw new Error(`sea_build: ${NODE} is not an x86-64 ELF`);
  fs.mkdirSync(path.dirname(OUT), { recursive: true });
  fs.writeFileSync(OUT, elf);
  const { inject } = createRequire(import.meta.url)(path.join(TOOLS, 'node_modules', 'postject', 'dist', 'api.js'));
  await inject(OUT, 'NODE_SEA_BLOB', fs.readFileSync(blob), { sentinelFuse: FUSE });
  try { fs.chmodSync(OUT, 0o755); } catch { /* (Windows: the release tarball records the mode) */ }
  const sha = crypto.createHash('sha256').update(fs.readFileSync(OUT)).digest('hex');
  console.log(`sea_build: ${OUT} (linux-x64, ${(fs.statSync(OUT).size / 1048576).toFixed(1)} MB) sha256 ${sha}`);
  process.exit(0);
}

// ---- 2. node.exe without its signature, with the setup's version information
const ResEdit = await import(pathToFileURL(path.join(TOOLS, 'node_modules', 'resedit', 'dist', 'index.js')).href);
const exe = ResEdit.NtExecutable.from(fs.readFileSync(NODE), { ignoreCert: true });
const res = ResEdit.NtExecutableResource.from(exe);
const vi = ResEdit.Resource.VersionInfo.fromEntries(res.entries)[0];
const nums = (VERSION.match(/\d+/g) || []).map(Number).concat([0, 0, 0, 0]).slice(0, 4);
const ver = { major: nums[0], minor: nums[1], micro: nums[2], revision: nums[3] };
vi.setFileVersion(ver.major, ver.minor, ver.micro, ver.revision, 1033);
vi.setProductVersion(ver.major, ver.minor, ver.micro, ver.revision, 1033);
vi.setStringValues({ lang: 1033, codepage: 1200 }, {
  FileDescription: 'Kingdom Hearts x Elden Ring setup',
  ProductName: 'Kingdom Hearts x Elden Ring',
  FileVersion: `${ver.major}.${ver.minor}.${ver.micro}`, // (numeric; the release name, e.g. 0.1-preview, is the product version)
  ProductVersion: VERSION,
  OriginalFilename: 'setup.exe',
  InternalName: 'setup',
  CompanyName: 'Kingdom Hearts x Elden Ring (unofficial fan project)',
  LegalCopyright: 'Built on Node.js (MIT, (c) OpenJS Foundation and Node.js contributors): THIRD-PARTY-NOTICES.txt',
  Comments: 'Node.js single executable application of the mod\'s setup; runs the setup and its tool scripts',
});
vi.outputToResourceEntries(res.entries);
res.outputResource(exe);
fs.mkdirSync(path.dirname(OUT), { recursive: true });
fs.writeFileSync(OUT, Buffer.from(exe.generate()));

// ---- 3. the blob, injected
const { inject } = createRequire(import.meta.url)(path.join(TOOLS, 'node_modules', 'postject', 'dist', 'api.js'));
await inject(OUT, 'NODE_SEA_BLOB', fs.readFileSync(blob), { sentinelFuse: FUSE });
const sha = crypto.createHash('sha256').update(fs.readFileSync(OUT)).digest('hex');
console.log(`sea_build: ${OUT} (${(fs.statSync(OUT).size / 1048576).toFixed(1)} MB) sha256 ${sha}`);
