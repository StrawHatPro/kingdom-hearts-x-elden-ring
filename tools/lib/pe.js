// Minimal PE32+ reader shared by the split/build/check tools.
const fs = require('fs');
const path = require('path');
const { ROOT, CONFIG } = require('./log');

// The executable: the copy in base/ when there is one (the decompilation's), else the player's own install (tools/config.json).
const BASE_EXE = fs.existsSync(path.join(ROOT, 'base', 'KH3.exe')) ? path.join(ROOT, 'base', 'KH3.exe') : path.join(CONFIG.gameRoot, CONFIG.mainExe);

function load(file = BASE_EXE) {
  const b = fs.readFileSync(file);
  const pe = b.readUInt32LE(0x3c);
  const coff = pe + 4;
  const nSections = b.readUInt16LE(coff + 2);
  const opt = coff + 20;
  const imageBase = b.readBigUInt64LE(opt + 24);
  const sizeOfHeaders = b.readUInt32LE(opt + 60);
  const nDirs = b.readUInt32LE(opt + 108);
  const dirs = [];
  for (let i = 0; i < nDirs; i++) dirs.push({ rva: b.readUInt32LE(opt + 112 + i * 8), size: b.readUInt32LE(opt + 116 + i * 8) });
  const secTable = opt + b.readUInt16LE(coff + 16);
  const sections = [];
  for (let i = 0; i < nSections; i++) {
    const o = secTable + i * 40;
    sections.push({
      name: b.toString('latin1', o, o + 8).replace(/\0+$/, ''),
      vsize: b.readUInt32LE(o + 8), rva: b.readUInt32LE(o + 12),
      rawSize: b.readUInt32LE(o + 16), rawPtr: b.readUInt32LE(o + 20), flags: b.readUInt32LE(o + 36),
    });
  }
  const rvaToOff = (rva) => {
    for (const s of sections) if (rva >= s.rva && rva < s.rva + s.rawSize) return s.rawPtr + (rva - s.rva);
    return -1;
  };
  return { b, imageBase, sizeOfHeaders, dirs, sections, rvaToOff, entryPoint: b.readUInt32LE(opt + 16) };
}

// RUNTIME_FUNCTION table. `root` is the begin RVA of the function a chained fragment belongs to.
function runtimeFunctions(img) {
  const { b, dirs, rvaToOff } = img;
  const base = rvaToOff(dirs[3].rva);
  const n = dirs[3].size / 12;
  const out = new Array(n);
  const byBegin = new Map();
  for (let i = 0; i < n; i++) {
    const e = { begin: b.readUInt32LE(base + i * 12), end: b.readUInt32LE(base + i * 12 + 4), unwind: b.readUInt32LE(base + i * 12 + 8) };
    out[i] = e;
    byBegin.set(e.begin, e);
  }
  const parentOf = (e) => {
    const u = rvaToOff(e.unwind);
    if (u < 0) return null;
    if (!((b[u] >> 3) & 0x4)) return null; // UNW_FLAG_CHAININFO
    const codes = b[u + 2];
    return b.readUInt32LE(u + 4 + ((codes + 1) & ~1) * 2); // begin RVA of the chained-to entry
  };
  for (const e of out) {
    let cur = e, hops = 0;
    for (let p; (p = parentOf(cur)) !== null && byBegin.has(p) && hops < 32; hops++) cur = byBegin.get(p);
    e.root = cur.begin;
  }
  return out;
}

module.exports = { BASE_EXE, load, runtimeFunctions };
