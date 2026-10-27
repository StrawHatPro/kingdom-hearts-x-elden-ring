// A Linux run's platform differences, simulated on Windows, to prove the pack tools make the same files on Linux without
// a Linux machine (used for the Linux setup's proof; docs/BUILDING.md, "Linux"). Load it into every Node process of a run:
//
//   set NODE_OPTIONS=--require "<repo>/release-tools/linux_sim.js"
//   set KH_DXBC_DISASM=js & set KH_OODLE=<a kh-oodle build>     (the Linux replacements of the two Windows programs)
//
// in a clean room whose folder was made case-sensitive before any file was written into it
// (fsutil file setCaseSensitiveInfo <folder> enable: every folder made under it is case-sensitive, as on Linux).
// What it changes, in every process:
//   - folder listings (fs.readdirSync, fs.readdir, fs.promises.readdir) come in Linux's order: Node's libuv sorts a
//     folder's names by their bytes (scandir with strcmp) on Linux; NTFS gives its own, case-insensitive order;
//   - String.prototype.localeCompare without a locale compares UTF-16 units, as under LANG=C (ICU's POSIX locale);
//     with a locale it is left as it is.
// It does not load tools/lib/ntfs_order.js (Windows' order on Linux): a run with this file shows what a tool does when
// it is started on Linux without the setup.
'use strict';
const fs = require('fs');

const bytes = (s) => Buffer.from(s, 'utf8');
const nameOf = (e) => (typeof e === 'string' ? e : e && typeof e.name === 'string' ? e.name : null);
function linuxOrder(list, opts) {
  if (!Array.isArray(list) || (opts && typeof opts === 'object' && (opts.recursive || opts.encoding === 'buffer')) || opts === 'buffer') return list;
  if (!list.every((e) => nameOf(e) !== null)) return list;
  return list.sort((a, b) => Buffer.compare(bytes(nameOf(a)), bytes(nameOf(b))));
}
const sync = fs.readdirSync;
fs.readdirSync = function readdirSync(p, opts) { return linuxOrder(sync.call(fs, p, opts), opts); };
const cb = fs.readdir;
fs.readdir = function readdir(p, opts, done) {
  if (typeof opts === 'function') { done = opts; opts = undefined; }
  return cb.call(fs, p, opts, (err, list) => done(err, err ? list : linuxOrder(list, opts)));
};
const pr = fs.promises.readdir;
fs.promises.readdir = async function readdir(p, opts) { return linuxOrder(await pr.call(fs.promises, p, opts), opts); };

const lc = String.prototype.localeCompare;
// eslint-disable-next-line no-extend-native
String.prototype.localeCompare = function localeCompare(that, locales, options) {
  if (locales === undefined && options === undefined) { const a = String(this), b = String(that); return a < b ? -1 : a > b ? 1 : 0; }
  return lc.call(this, that, locales, options);
};
