#!/usr/bin/env node
// Proves tools/lib/dxbc_disasm.js (the Linux setup's shader disassembler) writes exactly what Microsoft's D3DDisassemble
// writes: compares its text with the .asm files the system DLL wrote (tools/lib/dxbc_disasm.ps1) for every .bin under the
// folders given (e.g. a whole pack build's build/cache), byte for byte. Reads only; prints the first differences.
//
//   node release-tools/dxbc_disasm_check.js <folder>... [--show N] [--out report.json]
//
// The number formats and the instructions a pack build may not contain are proven with generated test shaders (Windows):
//   node release-tools/dxbc_disasm_check.js --make-tests DIR --template <a .bin of a pack build> [--seed N]
//   powershell -NoProfile -ExecutionPolicy Bypass -File tools\lib\dxbc_disasm.ps1 -Dir DIR    (the system DLL)
//   node release-tools/dxbc_disasm_check.js DIR
'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { disassemble } = require(path.join(__dirname, '..', 'tools', 'lib', 'dxbc_disasm.js'));

const args = process.argv.slice(2);
const val = (k) => { const i = args.indexOf(k); return i >= 0 ? args.splice(i, 2)[1] : null; };
const show = Number(val('--show') || 5), outFile = val('--out');
const mk = val('--make-tests'), tplArg = val('--template'), seedArg = val('--seed');
if (mk) { fs.mkdirSync(mk, { recursive: true }); makeTests(tplArg, mk, seedArg); process.exit(0); }
if (!args.length) { console.error('usage: dxbc_disasm_check.js <folder>... [--show N] [--out FILE]'); process.exit(2); }
const pairs = [];
const walk = (d) => { for (const e of fs.readdirSync(d, { withFileTypes: true })) { const p = path.join(d, e.name); if (e.isDirectory()) walk(p); else if (e.name.endsWith('.bin') && fs.existsSync(p.replace(/\.bin$/, '.asm'))) pairs.push(p); } };
for (const a of args) walk(path.resolve(a));
let same = 0; const diff = [], threw = [];
const all = crypto.createHash('sha256');
for (const bin of pairs.sort()) {
  const want = fs.readFileSync(bin.replace(/\.bin$/, '.asm'), 'utf8');
  let got;
  try { got = disassemble(fs.readFileSync(bin)); } catch (e) { threw.push({ bin, error: e.message }); continue; }
  all.update(got);
  if (got === want) { same++; continue; }
  const a = want.split('\n'), b = got.split('\n');
  let k = 0; while (k < a.length && a[k] === b[k]) k++;
  diff.push({ bin, line: k + 1, want: a[k], got: b[k] });
}
console.log(`dxbc_disasm_check: ${pairs.length} shaders: ${same} identical, ${diff.length} different, ${threw.length} not read`);
for (const d of diff.slice(0, show)) console.log(`  ${d.bin}:${d.line}\n    D3DDisassemble: ${JSON.stringify(d.want)}\n    dxbc_disasm.js: ${JSON.stringify(d.got)}`);
for (const d of threw.slice(0, show)) console.log(`  ${d.bin}: ${d.error}`);
if (outFile) fs.writeFileSync(outFile, JSON.stringify({ shaders: pairs.length, identical: same, different: diff.length, not_read: threw.length, diff: diff.slice(0, 200), threw: threw.slice(0, 200) }, null, 1) + '\n');
process.exit(diff.length || threw.length ? 1 : 0);

// ---------------------------------------------------------------- generated test shaders
// (a template's signatures + our own program: immediates of every number class in float, integer and typeless instructions,
// control flow; the container checksum so D3DDisassemble accepts it)
// DXBC container checksum (MD5 with the container's own final-block layout), over the bytes after the checksum field
function md5blocks(state, blk) {
  const K = new Uint32Array(64), S = [7,12,17,22,7,12,17,22,7,12,17,22,7,12,17,22,5,9,14,20,5,9,14,20,5,9,14,20,5,9,14,20,4,11,16,23,4,11,16,23,4,11,16,23,4,11,16,23,6,10,15,21,6,10,15,21,6,10,15,21,6,10,15,21];
  for (let i = 0; i < 64; i++) K[i] = Math.floor(Math.abs(Math.sin(i + 1)) * 2 ** 32) >>> 0;
  for (let o = 0; o < blk.length; o += 64) {
    const M = []; for (let i = 0; i < 16; i++) M.push(blk.readUInt32LE(o + i * 4));
    let [a, b, c, d] = state;
    for (let i = 0; i < 64; i++) {
      let f, g;
      if (i < 16) { f = (b & c) | (~b & d); g = i; } else if (i < 32) { f = (d & b) | (~d & c); g = (5 * i + 1) % 16; } else if (i < 48) { f = b ^ c ^ d; g = (3 * i + 5) % 16; } else { f = c ^ (b | ~d); g = (7 * i) % 16; }
      const t = d; d = c; c = b; const x = (a + f + K[i] + M[g]) >>> 0; b = (b + ((x << S[i]) | (x >>> (32 - S[i])))) >>> 0; a = t;
    }
    state[0] = (state[0] + a) >>> 0; state[1] = (state[1] + b) >>> 0; state[2] = (state[2] + c) >>> 0; state[3] = (state[3] + d) >>> 0;
  }
}
function dxbcChecksum(buf) {
  const data = buf.subarray(20); const n = data.length, bits = (n * 8) >>> 0;
  const st = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476];
  const full = n - (n % 64); md5blocks(st, Buffer.from(data.subarray(0, full)));
  const r = data.subarray(full);
  let last;
  if (r.length < 56) { last = Buffer.alloc(64); last.writeUInt32LE(bits, 0); r.copy(last, 4); last[4 + r.length] = 0x80; last.writeUInt32LE(((bits >>> 2) | 1) >>> 0, 60); }
  else { const b1 = Buffer.alloc(64); r.copy(b1, 0); b1[r.length] = 0x80; last = Buffer.alloc(128); b1.copy(last, 0); last.writeUInt32LE(bits, 64); last.writeUInt32LE(((bits >>> 2) | 1) >>> 0, 124); }
  md5blocks(st, last);
  const out = Buffer.alloc(16); st.forEach((v, i) => out.writeUInt32LE(v, i * 4)); return out;
}
function makeTests(tpl, out, seedArg) {
const T = fs.readFileSync(tpl); const ch = {};
for (let k = 0, n = T.readUInt32LE(28); k < n; k++) { const o = T.readUInt32LE(32 + k * 4); ch[T.toString('latin1', o, o + 4)] = T.slice(o, o + 8 + T.readUInt32LE(o + 4)); }
function container(shex) {
  const parts = [ch.ISGN, ch.OSGN, shex]; const hdr = 32 + parts.length * 4; let off = hdr; const offs = [];
  for (const p of parts) { offs.push(off); off += p.length; }
  const b = Buffer.alloc(off); b.write('DXBC', 0, 'latin1'); b.writeUInt32LE(1, 20); b.writeUInt32LE(off, 24); b.writeUInt32LE(parts.length, 28);
  parts.forEach((p, i) => { b.writeUInt32LE(offs[i], 32 + i * 4); p.copy(b, offs[i]); }); dxbcChecksum(b).copy(b, 4); return b;
}
function shex(words) { const prog = [0x50, 0, ...words]; prog[1] = prog.length; const b = Buffer.alloc(8 + prog.length * 4); b.write('SHEX', 0, 'latin1'); b.writeUInt32LE(prog.length * 4, 4); prog.forEach((w, i) => b.writeUInt32LE(w >>> 0, 8 + i * 4)); return b; }
const DST = [2 | (0xF << 4) | (1 << 20), 0], SRC = [2 | (1 << 2) | (0xE4 << 4) | (1 << 20), 0];
const DSTX = [2 | (0x1 << 4) | (1 << 20), 0];
const IMM = (v) => [2 | (4 << 12), ...v];
const IMM1 = (v) => [1 | (4 << 12), v];
const ins = (code, ops, flags = 0) => { const w = [].concat(...ops); return [code | flags | ((w.length + 1) << 24), ...w]; };
let rnd = Number(seedArg || 12345) >>> 0; const R = () => { rnd = (Math.imul(rnd, 1103515245) + 12345) >>> 0; return rnd; };
const vals = [0, 1, 2, 5, 9999, 10000, 10001, -9999, -10000, -10001, 4097, 8191, 8192, 16383, 16384, 32767, 65535, -1, -2, 0x7fffffff, 0x80000000, 0x80000001, 0x3f800000, 0x3C000000, 0xBC000000, 0x3CC00000, 0x3D200000, 0x7fc00000, 0xffc00000, 0x7f800001, 0xff800001, 0xffffffff, 0x7f800000, 0xff800000, 0x00000001, 0x807fffff, 0x00800000, 0x00400000, 0x01000000, 0x34800000, 0x35000000, 0x35800000, 0x35A00000, 0x7f7fffff, 0xff7fffff, 0x3e99999a, 0x3eaaaaab, 0x00ffffff, 0x007fffff, 0x7fbfffff, 0xffbfffff, 0x7fc00001, 0xffc00001];
const groups = []; for (let i = 0; i < vals.length; i += 4) groups.push(vals.slice(i, i + 4).concat([0, 0, 0]).slice(0, 4));
for (let k = 0; k < 600; k++) { const g = [R(), R(), R(), R()].map((x) => (k % 4 === 0 ? x : k % 4 === 1 ? (x & 0x80ffffff) : k % 4 === 2 ? (x % 30000) - 15000 : (0x3a000000 + (x & 0x07ffffff)) | (x & 0x80000000))); groups.push(g); }
let n = 0;
for (const g of groups) {
  const words = [104 | (2 << 24), 1,
    ...ins(54, [DST, IMM(g)]), ...ins(54, [DSTX, IMM1(g[0])]), ...ins(54, [DSTX, IMM1(g[1])]),
    ...ins(30, [DST, SRC, IMM(g)]), ...ins(0, [DST, SRC, IMM(g)]), ...ins(55, [DST, SRC, IMM(g), SRC]), ...ins(55, [DST, IMM(g), SRC, SRC]),
    ...ins(27, [DST, IMM(g)]), ...ins(43, [DST, IMM(g)]), ...ins(1, [DST, SRC, IMM(g)]), ...ins(142, [DST, DST, SRC, IMM(g), SRC]),
    ...ins(130, [DST, IMM(g)]), ...ins(131, [DST, IMM(g)]), ...ins(86, [DST, IMM(g)]), ...ins(28, [DST, IMM(g)]), ...ins(77, [DST, DST, IMM(g)]),
    ...ins(56, [DST, SRC, IMM(g)], 1 << 13), ...ins(31, [IMM1(g[2])], 1 << 18), ...ins(21, []), ...ins(76, [IMM1(g[3])]), ...ins(6, [IMM1(g[0])]), ...ins(2, []), ...ins(10, []), ...ins(2, []), ...ins(23, []),
    62 | (1 << 24)];
  fs.writeFileSync(path.join(out, `t${String(n++).padStart(4, '0')}.bin`), container(shex(words)));
}
console.log(`dxbc_disasm_check: ${n} test shaders written to ${out}`);
}
