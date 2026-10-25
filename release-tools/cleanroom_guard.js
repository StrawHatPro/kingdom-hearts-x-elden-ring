// The clean-room guard: proves a setup run reads nothing but this repository, the games and the system.
//
//   set NODE_OPTIONS=--require "<repo>/release-tools/cleanroom_guard.js"
//   set KH_GUARD_DENY=<folder>[;<folder>...]   folders no file may be read from or written to (e.g. a development tree)
//   set KH_GUARD_LOG=<file>                    optional: every path a Node process of the run opened (deduplicated)
//
// Every Node process of the run (the setup and every tool it starts: NODE_OPTIONS is inherited) has its fs calls checked:
// a path under a denied folder makes the call throw EACCES and is logged as DENIED. Paths are compared case-insensitively
// after resolving them (Windows). It changes nothing else. Used by release-tools/cleanroom.js.
'use strict';
const fs = require('fs');
const path = require('path');
const deny = (process.env.KH_GUARD_DENY || '').split(';').filter(Boolean).map((d) => path.resolve(d).toLowerCase() + path.sep);
const LOG = process.env.KH_GUARD_LOG || null;
const orig = { appendFileSync: fs.appendFileSync };
const seen = new Set();
let denied = 0;
function note(p, what) {
  if (!LOG) return;
  const k = what + '\t' + p;
  if (seen.has(k)) return;
  seen.add(k);
  if (seen.size % 2000 === 0) flush();
}
function flush() { if (!LOG || !seen.size) return; try { orig.appendFileSync.call(fs, LOG, [...seen].map((s) => `${process.pid}\t${s}`).join('\n') + '\n'); } catch { /* the log is best effort */ } seen.clear(); }
process.on('exit', flush);
function check(p, what) {
  let s = p;
  if (s instanceof URL) s = require('url').fileURLToPath(s);
  if (Buffer.isBuffer(s)) s = s.toString();
  if (typeof s !== 'string') return;
  const abs = path.resolve(s);
  const l = abs.toLowerCase() + path.sep;
  if (deny.some((d) => l.startsWith(d))) {
    denied++;
    note(abs, 'DENIED');
    flush();
    const e = new Error(`cleanroom_guard: ${what} of ${abs} refused (KH_GUARD_DENY)`);
    e.code = 'EACCES';
    throw e;
  }
  note(abs, what);
}
const READ = ['readFileSync', 'openSync', 'statSync', 'lstatSync', 'existsSync', 'readdirSync', 'accessSync', 'createReadStream', 'realpathSync', 'opendirSync', 'readFile', 'stat', 'lstat', 'readdir', 'access', 'open', 'readlinkSync'];
const WRITE = ['writeFileSync', 'appendFileSync', 'mkdirSync', 'createWriteStream', 'rmSync', 'unlinkSync', 'writeFile', 'rmdirSync', 'truncateSync'];
for (const k of READ) { const o = fs[k]; if (typeof o !== 'function') continue; fs[k] = function (p, ...a) { if (k === 'existsSync') { try { check(p, 'read'); } catch { return false; } } else check(p, 'read'); return o.call(fs, p, ...a); }; if (o.native) fs[k].native = o.native; }
for (const k of WRITE) { const o = fs[k]; if (typeof o !== 'function') continue; fs[k] = function (p, ...a) { check(p, 'write'); return o.call(fs, p, ...a); }; }
for (const k of ['copyFileSync', 'renameSync', 'cpSync', 'linkSync', 'symlinkSync']) { const o = fs[k]; if (typeof o !== 'function') continue; fs[k] = function (a, b, ...r) { check(a, 'read'); check(b, 'write'); return o.call(fs, a, b, ...r); }; }
const fsp = fs.promises;
for (const k of ['readFile', 'stat', 'lstat', 'readdir', 'access', 'open']) { const o = fsp[k]; fsp[k] = function (p, ...a) { check(p, 'read'); return o.call(fsp, p, ...a); }; }
for (const k of ['writeFile', 'mkdir', 'rm', 'unlink', 'appendFile']) { const o = fsp[k]; fsp[k] = function (p, ...a) { check(p, 'write'); return o.call(fsp, p, ...a); }; }
// child processes: a command line naming a denied folder is refused too (cargo, dxc, PowerShell are not Node)
const cp = require('child_process');
for (const k of ['spawn', 'spawnSync', 'execFile', 'execFileSync', 'exec', 'execSync']) {
  const o = cp[k];
  cp[k] = function (cmd, argv, ...rest) {
    const all = [cmd].concat(Array.isArray(argv) ? argv : []).filter((x) => typeof x === 'string');
    for (const x of all) for (const d of deny) if ((x.toLowerCase() + path.sep).includes(d.slice(0, -1).toLowerCase())) { denied++; note(x, 'DENIED-SPAWN'); flush(); const e = new Error(`cleanroom_guard: a command line naming ${d} refused`); e.code = 'EACCES'; throw e; }
    note(all.join(' ').slice(0, 300), 'spawn');
    return o.call(cp, cmd, argv, ...rest);
  };
}
module.exports = { denied: () => denied };
