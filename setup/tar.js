// A small tar (ustar / GNU long names) reader and writer, gzip through Node's zlib: the Linux setup unpacks the DirectX
// shader compiler's Linux release with it (setup/setup.js), release-tools/make_release.js reads the pinned Node.js
// linux-x64 tarball and writes the linux-x64 release tarball with it. No system tar is needed (and none is trusted to
// read a path with a colon as a local file). Only built-in modules.
//
//   const tar = require('./tar');
//   tar.extract(gzBuffer, destDir, { strip: 0 })   regular files, folders and symbolic links; file modes kept
//   tar.list(gzBuffer)                              [{ name, type, mode, size, data, link }]
//   tar.create(entries, { mtime })                  entries [{ name, mode, data | dir: true | link }] -> the .tar.gz bytes
//                                                   (deterministic: fixed owner, time and order as given, gzip level 9)
'use strict';
const fs = require('fs');
const path = require('path');
const zlib = require('zlib');

const str = (b, o, n) => { const e = b.indexOf(0, o); return b.toString('utf8', o, e < 0 || e > o + n ? o + n : e); };
const oct = (b, o, n) => { const s = str(b, o, n).trim(); return s ? parseInt(s, 8) : 0; };

function list(gz) {
  const b = gz[0] === 0x1f && gz[1] === 0x8b ? zlib.gunzipSync(gz) : gz;
  const out = [];
  let longName = null, longLink = null, pax = {};
  for (let o = 0; o + 512 <= b.length;) {
    if (b.subarray(o, o + 512).every((x) => x === 0)) break;
    let name = str(b, o, 100);
    const mode = oct(b, o + 100, 8), size = oct(b, o + 124, 12), type = String.fromCharCode(b[o + 156] || 48);
    let link = str(b, o + 157, 100);
    const prefix = b.toString('latin1', o + 257, o + 263).startsWith('ustar') ? str(b, o + 345, 155) : '';
    if (prefix) name = prefix + '/' + name;
    const data = b.subarray(o + 512, o + 512 + size);
    o += 512 + Math.ceil(size / 512) * 512;
    if (type === 'L') { longName = str(data, 0, data.length); continue; }
    if (type === 'K') { longLink = str(data, 0, data.length); continue; }
    if (type === 'x') { for (const m of data.toString('utf8').matchAll(/\d+ ([^=]+)=([^\n]*)\n/g)) pax[m[1]] = m[2]; continue; }
    if (type === 'g') continue;
    if (longName) { name = longName; longName = null; }
    if (longLink) { link = longLink; longLink = null; }
    if (pax.path) name = pax.path;
    if (pax.linkpath) link = pax.linkpath;
    pax = {};
    out.push({ name, type: type === '0' || type === '\0' || type === '7' ? 'file' : type === '5' ? 'dir' : type === '2' ? 'symlink' : type === '1' ? 'hardlink' : 'other', mode, size, data, link });
  }
  return out;
}

function extract(gz, dest, opts = {}) {
  const strip = opts.strip || 0;
  const root = path.resolve(dest);
  const done = [];
  for (const e of list(gz)) {
    const parts = e.name.split('/').filter((p) => p && p !== '.');
    if (parts.length <= strip) continue;
    const rel = parts.slice(strip);
    if (rel.includes('..')) throw new Error(`tar: ${e.name}: a path outside the folder`);
    const p = path.join(root, ...rel);
    if (e.type === 'dir') { fs.mkdirSync(p, { recursive: true }); continue; }
    fs.mkdirSync(path.dirname(p), { recursive: true });
    if (e.type === 'file') { fs.writeFileSync(p, e.data); try { fs.chmodSync(p, e.mode & 0o777); } catch { /* (Windows) */ } done.push(p); }
    else if (e.type === 'symlink') { if (path.isAbsolute(e.link) || path.normalize(path.join(path.dirname(rel.join('/')), e.link)).startsWith('..')) throw new Error(`tar: ${e.name}: a link outside the folder`); fs.rmSync(p, { force: true }); fs.symlinkSync(e.link, p); done.push(p); }
    else if (e.type === 'hardlink') { const t = path.join(root, ...e.link.split('/').filter(Boolean).slice(strip)); fs.copyFileSync(t, p); done.push(p); }
  }
  return done;
}

function header(name, mode, size, type, mtime, link = '') {
  const h = Buffer.alloc(512);
  const w = (s, o, n) => Buffer.from(s, 'utf8').copy(h, o, 0, n);
  const num = (v, o, n) => w(v.toString(8).padStart(n - 1, '0') + '\0', o, n);
  let prefix = '';
  if (Buffer.byteLength(name) > 100) { const k = name.lastIndexOf('/', name.length - 1 - 0); const cut = name.slice(0, k); if (k > 0 && Buffer.byteLength(cut) <= 155 && Buffer.byteLength(name.slice(k + 1)) <= 100) { prefix = cut; name = name.slice(k + 1); } else return null; }
  w(name, 0, 100); num(mode, 100, 8); num(0, 108, 8); num(0, 116, 8); num(size, 124, 12); num(mtime, 136, 12);
  h.fill(0x20, 148, 156); h[156] = type.charCodeAt(0); w(link, 157, 100);
  w('ustar\0', 257, 6); w('00', 263, 2); w('root', 265, 32); w('root', 297, 32); w(prefix, 345, 155);
  let sum = 0; for (const x of h) sum += x;
  w(sum.toString(8).padStart(6, '0') + '\0 ', 148, 8);
  return h;
}
function create(entries, opts = {}) {
  const mtime = opts.mtime || 0;
  const parts = [];
  const pad = (n) => Buffer.alloc((512 - (n % 512)) % 512);
  for (const e of entries) {
    const name = e.dir && !e.name.endsWith('/') ? e.name + '/' : e.name;
    const data = e.dir || e.link ? Buffer.alloc(0) : Buffer.from(e.data);
    const type = e.dir ? '5' : e.link ? '2' : '0';
    let h = header(name, e.mode || (e.dir ? 0o755 : 0o644), data.length, type, mtime, e.link || '');
    if (!h) { // (a GNU long name first)
      const nb = Buffer.from(name + '\0', 'utf8');
      parts.push(header('././@LongLink', 0o644, nb.length, 'L', mtime), nb, pad(nb.length));
      h = header(name.slice(0, 100), e.mode || (e.dir ? 0o755 : 0o644), data.length, type, mtime, e.link || '');
    }
    parts.push(h, data, pad(data.length));
  }
  parts.push(Buffer.alloc(1024));
  const gz = zlib.gzipSync(Buffer.concat(parts), { level: 9 });
  gz[9] = 3; // (the gzip header's OS byte: Unix, whatever machine wrote it)
  return gz;
}

module.exports = { list, extract, create };
