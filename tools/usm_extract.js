// Extract the elementary streams of chosen .usm movies to assets/ (on demand; never all 31 GB).
//   node tools/usm_extract.js <file or prefix> [--verify] [--raw]
// The selector is a path relative to the game root (or to its Content/CriMovie folder) and
// matches as a prefix, e.g.  "main/livefeed/"  or  "KINGDOM HEARTS III/Content/CriMovie/main/en/mv_sf231.usm".
//
// Output, per movie, in assets/<path>.usm.d/ :
//   video_<chno>.<m1v|m2v|h264>   @SFV stream, unmasked: the plain codec bitstream
//   alpha_<chno>.<m1v|...>        @ALP stream (alpha plane as a second video), unmasked
//   audio_<chno>.<hca|adx>        @SFA stream exactly as stored (HCA header + blocks)
//   <sig>_<chno>.bin              any other stream signature, exactly as stored
//   <sig>_<chno>.json             readable copy of that stream's header/metadata tables
//   container.json                everything else needed to rebuild the .usm byte for byte:
//                                 CRID table and one descriptor per chunk, in file order
// (No file in this game carries @SBT subtitle or @CUE chunks, so there is no text output.)
//
// --verify  rebuilds the container from the files just written and compares its SHA-256 with
//           the original (and with audit/usm_manifest.json when the file is listed there).
// --raw     keeps video/alpha payloads masked, exactly as stored.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { ROOT, CONFIG, logRun, writeJson } = require('./lib/log');
const usm = require('./lib/usm');

const VERSION = '1.0.0';
const MOVIE_DIR = 'KINGDOM HEARTS III/Content/CriMovie';
const args = process.argv.slice(2);
const sel = args.find((a) => !a.startsWith('--'));
const VERIFY = args.includes('--verify'), RAW = args.includes('--raw');
if (!sel) { console.error('usage: usm_extract.js <file or prefix> [--verify] [--raw]'); process.exit(2); }

const all = fs.readdirSync(path.join(CONFIG.gameRoot, MOVIE_DIR), { recursive: true })
  .filter((f) => /\.usm$/i.test(f)).map((f) => (MOVIE_DIR + '/' + f).replace(/\\/g, '/')).sort();
const want = sel.replace(/\\/g, '/');
const chosen = all.filter((p) => p.startsWith(want) || p.slice(MOVIE_DIR.length + 1).startsWith(want));
if (!chosen.length) { console.error('no .usm matches ' + sel); process.exit(1); }

let mask = null;
if (!RAW) {
  const m = usm.findVideoMask(all.map((f) => path.join(CONFIG.gameRoot, f)));
  if (m.ok) mask = m.m2;
  else console.error('video mask could not be derived; video is written as stored (masked)');
}

let manifest = null;
try { manifest = new Map(JSON.parse(fs.readFileSync(path.join(ROOT, 'audit', 'usm_manifest.json'), 'utf8')).files.map((r) => [r.path, r])); } catch { /* not built yet */ }

const KIND = { '@SFV': 'video', '@ALP': 'alpha', '@SFA': 'audio' };
const listing = [];
let failures = 0;
for (const rel of chosen) {
  const src = path.join(CONFIG.gameRoot, rel);
  const outDir = path.join(ROOT, 'assets', rel + '.d');
  fs.mkdirSync(outDir, { recursive: true });
  const fds = new Map(); // key -> { fd, tmp }
  const descriptors = [];
  const r = usm.scan(src, {
    onStream(key, payload) {
      let f = fds.get(key);
      if (!f) { const tmp = path.join(outDir, key.replace(/[^A-Za-z0-9]/g, '_') + '.tmp'); f = { tmp, fd: fs.openSync(tmp, 'w') }; fds.set(key, f); }
      fs.writeSync(f.fd, payload);
    },
    onDescriptor(d) { descriptors.push(d); },
  }, { videoMask: mask });

  const streams = [];
  for (const [key, s] of r.streams) {
    if (s.sig === 'CRID') continue;
    const f = fds.get(key);
    const probe = s.probe ? s.probe.result() : null;
    const base = `${KIND[s.sig] || s.sig.replace(/[^A-Za-z0-9]/g, '')}_${s.chno}`;
    const entry = { id: key, signature: s.sig, chno: s.chno, codec: probe ? probe.codec : null, unmasked: s.unmasked, file: null, size: s.bytes, sha256: s.hash.digest('hex') };
    if (f) {
      fs.closeSync(f.fd);
      entry.file = base + (usm.STREAM_EXT[probe ? probe.codec : 'unknown'] || '.bin');
      fs.rmSync(path.join(outDir, entry.file), { force: true });
      fs.renameSync(f.tmp, path.join(outDir, entry.file));
    }
    writeJson(path.join(outDir, base + '.json'), {
      stream: key, codec: entry.codec, bitstream: probe,
      header: s.header ? { table: s.header.name, rows: usm.utfRows(s.header) } : null,
      metadata: s.metadata.map((m) => ({ table: m.name, rows: usm.utfRows(m) })),
    });
    streams.push(entry);
  }
  // container.json: one descriptor per line so that large movies stay diffable and streamable.
  const head = {
    format: 'usm-container', tool: 'tools/usm_extract.js', version: VERSION, lib_version: usm.VERSION,
    source: rel, size: r.size, sha256: r.sha256_original, video_masked_in_container: !!mask, streams,
  };
  const cf = path.join(outDir, 'container.json');
  const cfd = fs.openSync(cf, 'w');
  fs.writeSync(cfd, '{\n"info": ' + JSON.stringify(head) + ',\n"chunks": [\n');
  descriptors.forEach((d, i) => fs.writeSync(cfd, JSON.stringify(d) + (i + 1 < descriptors.length ? ',\n' : '\n')));
  fs.writeSync(cfd, ']\n}\n');
  fs.closeSync(cfd);

  const row = { path: 'assets/' + rel + '.d', source: rel, size: r.size, sha256: r.sha256_original, streams };
  if (manifest && manifest.has(rel)) {
    const m = manifest.get(rel);
    const same = (x) => { const ms = m.streams.find((y) => y.id === x.id); return !!ms && (ms.unmasked_by_tool !== x.unmasked || ms.sha256_elementary === x.sha256); };
    row.matches_manifest = m.sha256 === r.sha256_original && streams.every(same);
  }
  if (VERIFY) {
    // Rebuild from what is on disk now, not from anything still in memory.
    const c = JSON.parse(fs.readFileSync(cf, 'utf8'));
    const open = new Map();
    const h = crypto.createHash('sha256');
    let n = 0;
    usm.remux(c.chunks, (key) => {
      if (!open.has(key)) open.set(key, fs.openSync(path.join(outDir, c.info.streams.find((s) => s.id === key).file), 'r'));
      return open.get(key);
    }, (buf) => { h.update(buf); n += buf.length; }, c.info.video_masked_in_container ? mask : null);
    for (const fd of open.values()) fs.closeSync(fd);
    row.remux_sha256 = h.digest('hex');
    row.remux_size = n;
    row.remux_matches = row.remux_sha256 === r.sha256_original && n === r.size;
    if (!row.remux_matches) failures++;
  }
  listing.push(row);
  console.log(`${rel}: ${streams.map((s) => `${s.file} ${s.size}`).join(', ')}${VERIFY ? (row.remux_matches ? '  remux OK' : '  REMUX MISMATCH') : ''}`);
}
for (const row of listing) console.log(JSON.stringify(row));
logRun({ tool: 'usm_extract', version: VERSION, outputs: listing.map((x) => path.join(ROOT, x.path, 'container.json')), note: `${listing.length} movies, selector ${sel}${RAW ? ', raw' : ''}${VERIFY ? `, remux verified: ${listing.length - failures}/${listing.length}` : ''}` });
console.log(`${listing.length} movies -> assets/` + (VERIFY ? `, remux ${listing.length - failures}/${listing.length} identical` : ''));
if (failures) process.exitCode = 1;
