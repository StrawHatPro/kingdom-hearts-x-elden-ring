// Phase 1 triage of a PE image: headers, sections + entropy, Rich header (toolchain
// fingerprint), debug directory, imports, exception table, TLS, overlay.
//   node tools/pe.js [path relative to gameRoot] [out json]
const fs = require('fs');
const path = require('path');
const { ROOT, CONFIG, sha256File, logRun, writeJson, hex } = require('./lib/log');

const VERSION = '1.0.0';
const relExe = process.argv[2] || CONFIG.mainExe;
const exePath = path.join(CONFIG.gameRoot, relExe);
const OUT = process.argv[3]
  ? path.resolve(process.argv[3])
  : path.join(ROOT, 'audit', 'triage', 'pe_' + path.basename(relExe).replace(/[^A-Za-z0-9]+/g, '_') + '.json');

const b = fs.readFileSync(exePath);
const u16 = (o) => b.readUInt16LE(o);
const u32 = (o) => b.readUInt32LE(o);
const u64 = (o) => b.readBigUInt64LE(o);

function entropy(buf) {
  if (!buf.length) return 0;
  const c = new Float64Array(256);
  for (let i = 0; i < buf.length; i++) c[buf[i]]++;
  let e = 0;
  for (let i = 0; i < 256; i++) if (c[i]) e -= (c[i] / buf.length) * Math.log2(c[i] / buf.length);
  return Math.round(e * 1000) / 1000;
}

function cstr(o, max = 4096) {
  let e = o;
  while (e < b.length && e - o < max && b[e]) e++;
  return b.toString('latin1', o, e);
}

if (u16(0) !== 0x5a4d) throw new Error('not MZ');
const pe = u32(0x3c);
if (u32(pe) !== 0x4550) throw new Error('not PE');
const coff = pe + 4;
const nSections = u16(coff + 2);
const optSize = u16(coff + 16);
const opt = coff + 20;
const magic = u16(opt);
if (magic !== 0x20b) throw new Error('expected PE32+, got magic ' + hex(magic));
const imageBase = u64(opt + 24);
const nDirs = u32(opt + 108);
const DIR_NAMES = ['export', 'import', 'resource', 'exception', 'security', 'basereloc', 'debug', 'architecture',
  'globalptr', 'tls', 'load_config', 'bound_import', 'iat', 'delay_import', 'com_descriptor', 'reserved'];
const dirs = {};
for (let i = 0; i < nDirs; i++) dirs[DIR_NAMES[i] || 'dir' + i] = { rva: u32(opt + 112 + i * 8), size: u32(opt + 116 + i * 8) };

const secTable = opt + optSize;
const sections = [];
for (let i = 0; i < nSections; i++) {
  const o = secTable + i * 40;
  const s = {
    name: b.toString('latin1', o, o + 8).replace(/\0+$/, ''),
    vsize: u32(o + 8), rva: u32(o + 12), rawSize: u32(o + 16), rawPtr: u32(o + 20), flags: u32(o + 36),
  };
  sections.push(s);
}
function rvaToOff(rva) {
  for (const s of sections) if (rva >= s.rva && rva < s.rva + Math.max(s.vsize, s.rawSize)) {
    const d = rva - s.rva;
    return d < s.rawSize ? s.rawPtr + d : -1;
  }
  return -1;
}
const flagStr = (f) => [[0x20, 'CODE'], [0x40, 'IDATA'], [0x80, 'UDATA'], [0x2000000, 'DISCARD'],
  [0x20000000, 'X'], [0x40000000, 'R'], [0x80000000, 'W']].filter(([m]) => (f & m) >>> 0).map(([, n]) => n).join('|');

// Rich header: XOR-masked list of (product id, build, object count) the linker saw.
function richHeader() {
  const end = b.indexOf('Rich', 0x80, 'latin1');
  if (end < 0 || end > pe) return null;
  const key = u32(end + 4);
  let start = -1;
  for (let o = end - 4; o >= 0x40; o -= 4) if ((u32(o) ^ key) >>> 0 === 0x536e6144) { start = o; break; }
  if (start < 0) return null;
  const entries = [];
  for (let o = start + 16; o < end; o += 8) {
    const id = (u32(o) ^ key) >>> 0;
    entries.push({ prodId: id >>> 16, build: id & 0xffff, count: (u32(o + 4) ^ key) >>> 0 });
  }
  return { key: hex(key, 8), entries };
}

function debugDir() {
  const d = dirs.debug;
  const out = [];
  if (!d || !d.size) return out;
  const base = rvaToOff(d.rva);
  for (let o = base; o < base + d.size; o += 28) {
    const e = { type: u32(o + 12), timestamp: u32(o + 4), size: u32(o + 16), rva: u32(o + 20), rawPtr: u32(o + 24) };
    if (e.type === 2 && e.rawPtr && b.toString('latin1', e.rawPtr, e.rawPtr + 4) === 'RSDS') {
      e.codeview = {
        guid: b.subarray(e.rawPtr + 4, e.rawPtr + 20).toString('hex'),
        age: u32(e.rawPtr + 20),
        pdb: cstr(e.rawPtr + 24),
      };
    }
    out.push(e);
  }
  return out;
}

function imports(dirName, entrySize, nameField, thunkField, rvaBased = true) {
  const d = dirs[dirName];
  const out = [];
  if (!d || !d.size) return out;
  let o = rvaToOff(d.rva);
  if (o < 0) return [{ error: 'directory not backed by file data' }];
  for (; ; o += entrySize) {
    const nameRva = u32(o + nameField);
    const thunkRva = u32(o + thunkField) || u32(o + 16);
    if (!nameRva) break;
    const no = rvaToOff(nameRva);
    if (no < 0) { out.push({ error: 'bad name rva ' + hex(nameRva) }); break; }
    const names = [];
    let t = rvaToOff(thunkRva);
    for (; t >= 0 && t + 8 <= b.length; t += 8) {
      const v = u64(t);
      if (!v) break;
      if (v >> 63n) names.push('#' + (v & 0xffffn));
      else {
        const ho = rvaToOff(Number(v & 0x7fffffffn));
        names.push(ho >= 0 ? cstr(ho + 2, 512) : '?' + hex(Number(v & 0x7fffffffn)));
      }
    }
    out.push({ dll: cstr(no, 260), count: names.length, names });
  }
  return out;
}

function exceptionTable() {
  const d = dirs.exception;
  if (!d || !d.size) return null;
  const base = rvaToOff(d.rva);
  const n = d.size / 12;
  let covered = 0, minRva = 0xffffffff, maxRva = 0, bad = 0, prevEnd = 0, unsorted = 0;
  for (let i = 0; i < n; i++) {
    const s = u32(base + i * 12), e = u32(base + i * 12 + 4);
    if (!s || e <= s) { bad++; continue; }
    if (s < prevEnd) unsorted++;
    prevEnd = e;
    covered += e - s;
    if (s < minRva) minRva = s;
    if (e > maxRva) maxRva = e;
  }
  return { entries: n, bad, overlapping_or_unsorted: unsorted, covered_bytes: covered, min_rva: hex(minRva), max_rva: hex(maxRva) };
}

function tls() {
  const d = dirs.tls;
  if (!d || !d.size) return null;
  const o = rvaToOff(d.rva);
  if (o < 0) return { error: 'not backed' };
  const cbVa = u64(o + 24);
  const callbacks = [];
  let co = cbVa ? rvaToOff(Number(cbVa - imageBase)) : -1;
  for (; co >= 0; co += 8) {
    const v = u64(co);
    if (!v) break;
    callbacks.push(hex(Number(v - imageBase)));
  }
  return { callbacks_va: hex(Number(cbVa)), callback_rvas: callbacks };
}

const ep = u32(opt + 16);
const lastRawEnd = Math.max(...sections.map((s) => s.rawPtr + s.rawSize));
const result = {
  file: relExe,
  size: b.length,
  sha256: sha256File(exePath),
  coff: {
    machine: hex(u16(coff), 4),
    sections: nSections,
    timestamp: u32(coff + 4),
    timestamp_iso: new Date(u32(coff + 4) * 1000).toISOString(),
    characteristics: hex(u16(coff + 18), 4),
  },
  optional: {
    magic: hex(magic, 4),
    linker_version: `${b[opt + 2]}.${b[opt + 3]}`,
    size_of_code: u32(opt + 4),
    entry_point_rva: hex(ep),
    entry_point_section: (sections.find((s) => ep >= s.rva && ep < s.rva + s.vsize) || {}).name || null,
    image_base: hex(Number(imageBase)),
    section_alignment: u32(opt + 32),
    file_alignment: u32(opt + 36),
    os_version: `${u16(opt + 40)}.${u16(opt + 42)}`,
    subsystem_version: `${u16(opt + 48)}.${u16(opt + 50)}`,
    size_of_image: u32(opt + 56),
    size_of_headers: u32(opt + 60),
    checksum: hex(u32(opt + 64), 8),
    subsystem: u16(opt + 68),
    dll_characteristics: hex(u16(opt + 70), 4),
  },
  data_directories: Object.fromEntries(Object.entries(dirs).map(([k, v]) => [k, { rva: hex(v.rva), size: v.size }])),
  sections: sections.map((s) => ({
    name: s.name, rva: hex(s.rva), vsize: s.vsize, raw_ptr: hex(s.rawPtr), raw_size: s.rawSize,
    flags: hex(s.flags, 8), flags_decoded: flagStr(s.flags),
    entropy: entropy(b.subarray(s.rawPtr, s.rawPtr + s.rawSize)),
  })),
  overlay: { offset: hex(lastRawEnd), size: b.length - lastRawEnd },
  rich_header: richHeader(),
  debug_directory: debugDir(),
  exception_table: exceptionTable(),
  tls: tls(),
  imports: imports('import', 20, 12, 0),
  delay_imports: imports('delay_import', 32, 4, 16),
};
writeJson(OUT, result);
logRun({ tool: 'pe', version: VERSION, outputs: [OUT] });

const r = result;
console.log(`${r.file}\n  size ${r.size}  sha256 ${r.sha256}`);
console.log(`  machine ${r.coff.machine}  linked ${r.coff.timestamp_iso}  linker ${r.optional.linker_version}  EP ${r.optional.entry_point_rva} in ${r.optional.entry_point_section}`);
for (const s of r.sections) console.log(`  ${s.name.padEnd(9)} rva ${s.rva.padEnd(11)} vsize ${String(s.vsize).padStart(10)} raw ${String(s.raw_size).padStart(10)} H=${s.entropy} ${s.flags_decoded}`);
console.log(`  overlay ${r.overlay.size} bytes at ${r.overlay.offset}`);
console.log('  rich:', r.rich_header ? r.rich_header.entries.map((e) => `${e.prodId}/${e.build}x${e.count}`).join(' ') : 'none');
console.log('  debug:', JSON.stringify(r.debug_directory));
console.log('  pdata:', JSON.stringify(r.exception_table));
console.log('  tls:', JSON.stringify(r.tls));
console.log('  imports:', r.imports.map((i) => `${i.dll || i.error}(${i.count ?? ''})`).join(' '));
console.log('  delay:', r.delay_imports.map((i) => `${i.dll || i.error}(${i.count ?? ''})`).join(' '));
