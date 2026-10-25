// audit/binary_manifest.json: SHA-256, size and path of every original file (spec 3.1).
// Resumable: a file whose size and mtime are unchanged keeps its recorded hash.
const fs = require('fs');
const path = require('path');
const { ROOT, CONFIG, sha256File, logRun, writeJson } = require('./lib/log');

const OUT = path.join(ROOT, 'audit', 'binary_manifest.json');
const VERSION = '1.0.0';

function walk(dir, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walk(p, out);
    else out.push(p);
  }
  return out;
}

const prev = new Map();
if (fs.existsSync(OUT)) for (const f of JSON.parse(fs.readFileSync(OUT, 'utf8')).files) prev.set(f.path, f);

const files = [];
const all = walk(CONFIG.gameRoot).sort();
let hashed = 0;
for (const [i, p] of all.entries()) {
  const st = fs.statSync(p);
  const relPath = path.relative(CONFIG.gameRoot, p).replace(/\\/g, '/');
  const mtime = st.mtime.toISOString();
  const old = prev.get(relPath);
  let sha256 = old && old.size === st.size && old.mtime === mtime ? old.sha256 : null;
  if (!sha256) {
    sha256 = sha256File(p);
    hashed++;
  }
  files.push({ path: relPath, size: st.size, mtime, sha256 });
  // Checkpoint so a tool timeout loses at most a few files.
  if (hashed && i % 8 === 0) writeJson(OUT, { root: CONFIG.gameRoot, complete: false, files });
  process.stdout.write(`\r${i + 1}/${all.length}`);
}
writeJson(OUT, {
  root: CONFIG.gameRoot,
  complete: true,
  file_count: files.length,
  total_bytes: files.reduce((a, f) => a + f.size, 0),
  files,
});
logRun({ tool: 'manifest', version: VERSION, outputs: [OUT], note: `${hashed} hashed, ${files.length - hashed} reused` });
console.log(`\n${files.length} files, ${hashed} hashed this run`);
