// The release zip (release-tools/make_release.js): a plain zip writer, deflate through Node's zlib, so the entry names use
// "/" as the zip format requires (APPNOTE 4.4.17). PowerShell 5.1's Compress-Archive wrote "\", which Linux unzip tools
// (and other non-Windows tools) take as part of a file name: everything landed in one folder as files named
// "kingdom-hearts-x-elden-ring\er\mods\...". Every folder gets its own entry; the files' bytes are stored as they are (the
// .bat files keep their CRLF); one fixed time stamp and a sorted order make the same zip from the same folder.
//
//   const { zipFolder, readZip } = require('./zip');
//   zipFolder(dir, outFile, { top: 'kingdom-hearts-x-elden-ring', time: <Date> })   -> { entries, bytes }
//   readZip(buf)                                                                    -> [{ name, dir, size, data }] (checked)
'use strict';
const fs = require('fs');
const path = require('path');
const zlib = require('zlib');

const crc32 = (b) => zlib.crc32(b) >>> 0;
// the MS-DOS date and time of a Date (local fields of the UTC time: the same on every machine)
function dosTime(d) {
  const y = Math.max(1980, d.getUTCFullYear());
  return { time: (d.getUTCHours() << 11) | (d.getUTCMinutes() << 5) | (d.getUTCSeconds() >> 1), date: ((y - 1980) << 9) | ((d.getUTCMonth() + 1) << 5) | d.getUTCDate() };
}

function zipFolder(dir, outFile, opts = {}) {
  const top = opts.top ? opts.top.replace(/\/+$/, '') : null;
  const { time, date } = dosTime(opts.time || new Date(Date.UTC(1980, 0, 1)));
  const list = [];
  (function walk(d, rel) {
    const items = fs.readdirSync(d, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0));
    for (const e of items) {
      const r = rel ? `${rel}/${e.name}` : e.name;
      if (e.isDirectory()) { list.push({ name: r + '/', dir: true }); walk(path.join(d, e.name), r); }
      else if (e.isFile()) list.push({ name: r, file: path.join(d, e.name) });
    }
  })(dir, top || '');
  if (top) list.unshift({ name: top + '/', dir: true });
  const fd = fs.openSync(outFile, 'w');
  let off = 0;
  const central = [];
  const write = (b) => { fs.writeSync(fd, b); off += b.length; };
  for (const e of list) {
    if (e.name.includes('\\')) throw new Error(`zip: ${e.name}: a backslash in a name`);
    const name = Buffer.from(e.name, 'utf8');
    const data = e.dir ? Buffer.alloc(0) : fs.readFileSync(e.file);
    const crc = e.dir ? 0 : crc32(data);
    let method = 0, body = data;
    if (!e.dir && data.length) { const z = zlib.deflateRawSync(data, { level: 9 }); if (z.length < data.length) { method = 8; body = z; } }
    const utf8 = /[^\x00-\x7f]/.test(e.name) ? 0x0800 : 0;
    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0); local.writeUInt16LE(20, 4); local.writeUInt16LE(utf8, 6); local.writeUInt16LE(method, 8);
    local.writeUInt16LE(time, 10); local.writeUInt16LE(date, 12); local.writeUInt32LE(crc, 14); local.writeUInt32LE(body.length, 18);
    local.writeUInt32LE(data.length, 22); local.writeUInt16LE(name.length, 26); local.writeUInt16LE(0, 28);
    const at = off;
    if (at > 0xffffffff || body.length > 0xffffffff) throw new Error('zip: over 4 GB (zip64 is not written)');
    write(local); write(name); write(body);
    const c = Buffer.alloc(46);
    c.writeUInt32LE(0x02014b50, 0); c.writeUInt16LE((3 << 8) | 20, 4); // (made by Unix: the modes below are read by unzip)
    c.writeUInt16LE(20, 6); c.writeUInt16LE(utf8, 8); c.writeUInt16LE(method, 10); c.writeUInt16LE(time, 12); c.writeUInt16LE(date, 14);
    c.writeUInt32LE(crc, 16); c.writeUInt32LE(body.length, 20); c.writeUInt32LE(data.length, 24); c.writeUInt16LE(name.length, 28);
    c.writeUInt16LE(0, 30); c.writeUInt16LE(0, 32); c.writeUInt16LE(0, 34); c.writeUInt16LE(0, 36);
    // external attributes: the Unix mode in the high half (folders 755, files 644), the MS-DOS folder flag in the low byte
    c.writeUInt32LE((((e.dir ? 0o40755 : 0o100644) << 16) | (e.dir ? 0x10 : 0)) >>> 0, 38);
    c.writeUInt32LE(at, 42);
    central.push(Buffer.concat([c, name]));
  }
  const cdAt = off;
  for (const c of central) write(c);
  const end = Buffer.alloc(22);
  if (list.length > 0xffff) throw new Error('zip: over 65535 entries (zip64 is not written)');
  end.writeUInt32LE(0x06054b50, 0); end.writeUInt16LE(list.length, 8); end.writeUInt16LE(list.length, 10);
  end.writeUInt32LE(off - cdAt, 12); end.writeUInt32LE(cdAt, 16);
  write(end);
  fs.closeSync(fd);
  return { entries: list.length, bytes: off };
}

// A zip's entries through its central directory, each checked against its local header and its CRC-32
function readZip(buf) {
  let e = buf.length - 22;
  while (e >= 0 && buf.readUInt32LE(e) !== 0x06054b50) e--;
  if (e < 0) throw new Error('zip: no end of central directory');
  const n = buf.readUInt16LE(e + 10);
  let p = buf.readUInt32LE(e + 16);
  const out = [];
  for (let k = 0; k < n; k++) {
    if (buf.readUInt32LE(p) !== 0x02014b50) throw new Error('zip: a broken central directory');
    const method = buf.readUInt16LE(p + 10), crc = buf.readUInt32LE(p + 16), csize = buf.readUInt32LE(p + 20), size = buf.readUInt32LE(p + 24);
    const nl = buf.readUInt16LE(p + 28), xl = buf.readUInt16LE(p + 30), cl = buf.readUInt16LE(p + 32), at = buf.readUInt32LE(p + 42);
    const name = buf.toString('utf8', p + 46, p + 46 + nl);
    p += 46 + nl + xl + cl;
    if (buf.readUInt32LE(at) !== 0x04034b50) throw new Error(`zip: ${name}: no local header`);
    const lnl = buf.readUInt16LE(at + 26), lxl = buf.readUInt16LE(at + 28);
    const body = buf.subarray(at + 30 + lnl + lxl, at + 30 + lnl + lxl + csize);
    const data = method === 8 ? zlib.inflateRawSync(body) : method === 0 ? body : null;
    if (!data) throw new Error(`zip: ${name}: method ${method}`);
    if (data.length !== size || crc32(data) !== crc) throw new Error(`zip: ${name}: size or CRC-32 wrong`);
    out.push({ name, dir: name.endsWith('/'), size, data });
  }
  return out;
}

module.exports = { zipFolder, readZip };
