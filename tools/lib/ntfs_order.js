// The Linux setup's folder listings in the order Windows gives them. On Windows, fs.readdir lists a folder in NTFS's own
// order: the names compared after upper-casing (A-Z and the rest of NTFS's upcase table); on Linux, Node (libuv) lists
// them in byte order, where "Hair" comes before "body" and "a_b" before "ab". A tool whose output follows the order it
// finds files in would then write another file on Linux. setup/setup.js loads this file into every Node process of a
// Linux run (NODE_OPTIONS=--require), so the pack's tools see the same order as on Windows. On Windows it does nothing.
//
// fs.readdirSync, fs.readdir and fs.promises.readdir (names or Dirents; a recursive listing is left as it is).
'use strict';
const fs = require('fs');

function upper(c) {
  if (c >= 0x61 && c <= 0x7a) return c - 32;
  if (c < 0x80) return c;
  const u = String.fromCharCode(c).toUpperCase();
  return u.length === 1 ? u.charCodeAt(0) : c;
}
// NTFS collation: the upper-cased UTF-16 units, then (names that differ only in case, which only a case-sensitive
// folder holds) the units themselves
function ntfsCompare(a, b) {
  const n = Math.min(a.length, b.length);
  for (let i = 0; i < n; i++) { const x = upper(a.charCodeAt(i)), y = upper(b.charCodeAt(i)); if (x !== y) return x - y; }
  if (a.length !== b.length) return a.length - b.length;
  return a < b ? -1 : a > b ? 1 : 0;
}
const nameOf = (e) => (typeof e === 'string' ? e : e && typeof e.name === 'string' ? e.name : null);
function order(list, opts) {
  if (!Array.isArray(list) || (opts && typeof opts === 'object' && (opts.recursive || opts.encoding === 'buffer')) || opts === 'buffer') return list;
  if (!list.every((e) => nameOf(e) !== null)) return list;
  return list.sort((a, b) => ntfsCompare(nameOf(a), nameOf(b)));
}

if (process.platform !== 'win32' && !fs.readdirSync.__ntfsOrder) {
  const sync = fs.readdirSync;
  fs.readdirSync = function readdirSync(p, opts) { return order(sync.call(fs, p, opts), opts); };
  fs.readdirSync.__ntfsOrder = true;
  const cb = fs.readdir;
  fs.readdir = function readdir(p, opts, done) {
    if (typeof opts === 'function') { done = opts; opts = undefined; }
    return cb.call(fs, p, opts, (err, list) => done(err, err ? list : order(list, opts)));
  };
  const pr = fs.promises.readdir;
  fs.promises.readdir = async function readdir(p, opts) { return order(await pr.call(fs.promises, p, opts), opts); };
}

module.exports = { ntfsCompare };
