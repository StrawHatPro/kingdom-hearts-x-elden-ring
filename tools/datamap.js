// Typed map of the non-code sections of base/KH3.exe, derived only from the binary.
//   node tools/datamap.js [--ref-cache=<file>] [--no-bss-split] [--no-write]
// --ref-cache keeps the result of the .text scan in a file (development aid); --no-write runs
// the analysis and the self-check without writing audit files or a tool-log line.
// Writes audit/data_map.json: ranges {start,end,section,kind,symbol,type,confidence,how} tiling
// .rdata, .data (+ .bss), .pdata, _RDATA, .rsrc and .reloc with no gaps and no overlaps.
// The splitter may consume it later; this tool does not touch asm/ or symbol_coverage.json.
//
// Evidence sources, strongest first (the `how` field of every row names the one used):
//   dir     a PE data directory or a structure reached from one by following its own fields
//   pogo    the linker's section-contribution list in the IMAGE_DEBUG_TYPE_POGO record
//   reloc   the base relocation table: every absolute pointer in the image has a DIR64 entry,
//           so "this qword is a pointer" is a fact, not a guess
//   xref    a RIP-relative operand of an instruction in .text (decoded with iced-x86), or the
//           target of a relocated pointer
//   content byte patterns only (zero fill, printable text) - always combined with one of the above
// Documented in docs/formats/exe_data.md.
const fs = require('fs');
const path = require('path');
const { Decoder, DecoderOptions, Instruction, FlowControl, Code, Mnemonic, MemorySize } = require('iced-x86');
const { ROOT, logRun, hex } = require('./lib/log');
const pe = require('./lib/pe');

const VERSION = '1.0.0';
const OUT = path.join(ROOT, 'audit', 'data_map.json');
const argv = process.argv.slice(2);
const REF_CACHE = (argv.find((a) => a.startsWith('--ref-cache=')) || '').slice(12);
const BSS_SPLIT = !argv.includes('--no-bss-split');
const WRITE = !argv.includes('--no-write');
const T0 = Date.now();
const tick = (m) => console.error(`[${((Date.now() - T0) / 1000).toFixed(1)}s] ${m}`);

const img = pe.load();
const { b, dirs, sections } = img;
const BASE = Number(img.imageBase);
const text = sections.find((s) => s.name === '.text');
const TARGETS = ['.rdata', '.data', '.pdata', '_RDATA', '.rsrc', '.reloc'];
const SECTION_KIND = { '.rdata': 'rodata', '.data': 'data', '.pdata': 'data', _RDATA: 'rodata', '.rsrc': 'data', '.reloc': 'data' };
const secs = TARGETS.map((n) => sections.find((s) => s.name === n)).filter(Boolean);
for (const s of secs) { s.ext = Math.max(s.vsize, s.rawSize); s.end = s.rva + s.ext; }
const LO = Math.min(...secs.map((s) => s.rva)), HI = Math.max(...secs.map((s) => s.end));
const SPAN = HI - LO;
const textLo = text.rva, textHi = text.rva + text.vsize;
const inText = (rva) => rva >= textLo && rva < textHi;
const inSpan = (rva) => rva >= LO && rva < HI;
const secOf = (rva) => { for (const s of secs) if (rva >= s.rva && rva < s.end) return s; return null; };
const vaHex = (rva) => (BASE + rva).toString(16).toUpperCase();

// Virtual image of the mapped data sections (bytes past the raw data read as zero, as loaded).
const M = Buffer.alloc(SPAN);
for (const s of secs) b.copy(M, s.rva - LO, s.rawPtr, s.rawPtr + Math.min(s.rawSize, s.ext));
const u8 = (rva) => M[rva - LO];
const u16 = (rva) => M.readUInt16LE(rva - LO);
const u32 = (rva) => M.readUInt32LE(rva - LO);
const i32 = (rva) => M.readInt32LE(rva - LO);
const u64 = (rva) => M.readUInt32LE(rva - LO) + M.readUInt32LE(rva - LO + 4) * 4294967296;
const ptrRva = (rva) => u64(rva) - BASE; // target RVA of the qword at rva (meaningful when relocated)
const ok = (rva, n = 1) => rva >= LO && rva + n <= HI && secOf(rva) !== null;

// Per-byte evidence flags over [LO, HI).
const F_RELOC = 1;   // a DIR64 relocation applies to the qword starting here
const F_PTR_TGT = 2; // some relocated pointer in the image points here
const F_LEA = 4;     // lea reg,[rip+here] in .text
const F_MEM = 8;     // RIP-relative memory operand (read/write/call/jmp) in .text
const F_REF = F_PTR_TGT | F_LEA | F_MEM;
const F = new Uint8Array(SPAN);
// Access description for F_MEM targets: low 3 bits size class (1,2,4,8,16,32 bytes -> 1..6,
// 7 other), 0x20 a non-float access seen, 0x40 conflicting sizes seen, 0x80 float access seen.
const ACC = new Uint8Array(SPAN);
const OWN = new Uint8Array(SPAN); // 1 = claimed by an item

const items = [];
const conflicts = {}; // claims refused because part of the range already belongs to another item
const duplicates = {}; // claims of exactly a range already claimed (shared records, e.g. folded EH tables)
const claimedEnd = new Map();
const stats = {};
function claim(start, end, type, prefix, confidence, how, extra) {
  if (!(end > start)) { conflicts[type + ':empty'] = (conflicts[type + ':empty'] || 0) + 1; return false; }
  const s = secOf(start);
  if (!s || end > s.end) { conflicts[type + ':outside'] = (conflicts[type + ':outside'] || 0) + 1; return false; }
  if (claimedEnd.get(start) === end) { duplicates[type] = (duplicates[type] || 0) + 1; return false; }
  for (let i = start - LO, e = end - LO; i < e; i++) if (OWN[i]) { conflicts[type + ':overlap'] = (conflicts[type + ':overlap'] || 0) + 1; return false; }
  OWN.fill(1, start - LO, end - LO);
  claimedEnd.set(start, end);
  const it = { s: start, e: end, type, prefix, confidence, how };
  if (extra) Object.assign(it, extra);
  items.push(it);
  return true;
}
const free = (start, end) => { for (let i = start - LO, e = end - LO; i < e; i++) if (OWN[i]) return false; return true; };

// ---- POGO section contributions (debug directory type 13) -----------------------------------
const pogo = []; // { rva, size, name }
const debugEntries = [];
if (dirs[6] && dirs[6].size) {
  for (let i = 0; i + 28 <= dirs[6].size; i += 28) {
    const r = dirs[6].rva + i;
    debugEntries.push({ at: r, type: u32(r + 12), size: u32(r + 16), rva: u32(r + 20), raw: u32(r + 24) });
  }
  const pg = debugEntries.find((d) => d.type === 13);
  if (pg && ok(pg.rva, pg.size)) {
    for (let p = pg.rva + 4, end = pg.rva + pg.size; p + 8 < end;) {
      const rva = u32(p), size = u32(p + 4);
      let e = p + 8; while (e < end && u8(e)) e++;
      const name = M.toString('latin1', p + 8 - LO, e - LO);
      if (!name) break;
      pogo.push({ rva, size, name });
      p = (e + 4) & ~3;
    }
  }
}
const pogoOf = (name) => pogo.find((p) => p.name === name);

// ---- A. base relocations ---------------------------------------------------------------------
const relocs = []; // RVAs (inside [LO,HI)) of relocated qwords, ascending
{
  const st = stats.relocations = { blocks: 0, dir64: 0, absolute_pad_entries: 0, other_types: 0, in_text: 0, in_data_sections: 0, elsewhere: 0 };
  const d = dirs[5];
  let p = d.rva; const end = d.rva + d.size;
  while (p + 8 <= end) {
    const page = u32(p), size = u32(p + 4);
    if (size < 8) break;
    st.blocks++;
    for (let i = 8; i < size; i += 2) {
      const e = u16(p + i), t = e >> 12, r = page + (e & 0xfff);
      if (t === 0) { st.absolute_pad_entries++; continue; }
      if (t !== 10) { st.other_types++; continue; }
      st.dir64++;
      let tgt;
      if (inSpan(r)) { st.in_data_sections++; F[r - LO] |= F_RELOC; relocs.push(r); tgt = ptrRva(r); }
      else {
        const o = img.rvaToOff(r);
        if (inText(r)) st.in_text++; else st.elsewhere++;
        tgt = o >= 0 ? b.readUInt32LE(o) + b.readUInt32LE(o + 4) * 4294967296 - BASE : -1;
      }
      if (inSpan(tgt)) F[tgt - LO] |= F_PTR_TGT;
    }
    claim(p, p + size, 'base_reloc_block', 'reloc', 'confirmed', `dir: base relocation block for page ${hex(BASE + page)} (${(size - 8) / 2} entries)`);
    p += size;
  }
  st.directory_fully_parsed = p === end;
  relocs.sort((a, c) => a - c);
}
const isReloc = (rva) => inSpan(rva) && (F[rva - LO] & F_RELOC) !== 0;
tick(`relocations: ${stats.relocations.dir64}`);

// ---- B. references from .text ------------------------------------------------------------------
const fns = pe.runtimeFunctions(img);
const fnBegin = new Set(fns.map((f) => f.begin));
// MemorySize enum -> [bytes, isFloat]
const MS = new Array(256).fill(null);
for (const [name, v] of Object.entries(MemorySize)) {
  if (typeof v !== 'number') continue;
  let m, size = 0, fl = false;
  if ((m = /^Packed(\d+)_(.*)$/.exec(name))) { size = +m[1] / 8; fl = /^Float(32|64)$/.test(m[2]); }
  else if ((m = /^Broadcast/.exec(name))) size = 0;
  else if ((m = /^(?:U?Int|Float|Bcd)(\d+)$/.exec(name))) { size = +m[1] / 8; fl = name.startsWith('Float'); }
  else if ((m = /^(?:Byte|Word|Dword|Qword)Offset$/.exec(name))) size = { Byte: 1, Word: 2, Dword: 4, Qword: 8 }[name.replace('Offset', '')];
  MS[v] = [size, fl];
}
const SIZE_CLASS = { 1: 1, 2: 2, 4: 3, 8: 4, 16: 5, 32: 6 };
const CLASS_SIZE = [0, 1, 2, 4, 8, 16, 32, 0];
function scanText() {
  const st = stats.text_scan = { pdata_ranges: fns.length, instructions: 0, undecodable_pdata_ranges: 0, gap_blocks_accepted: 0, gap_blocks_rejected: 0, rip_relative_operands: 0, into_data_sections: 0 };
  const raw = b.subarray(text.rawPtr, text.rawPtr + Math.min(text.rawSize, text.vsize));
  const dec = new Decoder(64, raw, DecoderOptions.None);
  const instr = new Instruction();
  let pend = []; // buffered refs of an unconfirmed gap block: [idx, flag, acc]
  const note = (buffer) => {
    st.rip_relative_operands++;
    const t = Number(instr.ipRelMemoryAddress) - BASE;
    if (!inSpan(t)) return;
    let flag, acc = 0;
    if (instr.mnemonic === Mnemonic.Lea) flag = F_LEA;
    else {
      flag = F_MEM;
      const ms = MS[instr.memorySize];
      acc = (ms && SIZE_CLASS[ms[0]]) || 7;
      acc |= ms && ms[1] ? 0x80 : 0x20;
    }
    if (buffer) pend.push(t - LO, flag, acc); else commit(t - LO, flag, acc);
  };
  const commit = (i, flag, acc) => {
    st.into_data_sections++;
    F[i] |= flag;
    if (!acc) return;
    const old = ACC[i];
    if (!old) { ACC[i] = acc; return; }
    let cls = old & 7;
    const ncls = acc & 7;
    let mixed = old & 0x40;
    if (cls !== ncls) { mixed = 0x40; if (ncls < cls) cls = ncls; }
    ACC[i] = cls | mixed | ((old | acc) & 0xa0);
  };
  const decodeFn = (rva, end) => {
    dec.position = rva - text.rva; dec.ip = BigInt(BASE + rva);
    let p = rva;
    while (p < end) {
      dec.decodeOut(instr);
      if (instr.code === Code.INVALID) { st.undecodable_pdata_ranges++; return; }
      st.instructions++;
      p += instr.length;
      if (instr.isIpRelMemoryOperand) note(false);
    }
  };
  // A region no .pdata entry covers: skip fill bytes, accept only blocks that decode cleanly up
  // to a terminator with no pending forward branch (same rule as tools/split.js sweepGap).
  const sweepGap = (s, e) => {
    let p = s;
    while (p < e) {
      const o = p - text.rva, fill = raw[o];
      if (fill === 0xcc || fill === 0x90 || fill === 0x00) { p++; continue; }
      dec.position = o; dec.ip = BigInt(BASE + p);
      let pos = 0, far = 0, good = true, closed = false, n = 0;
      pend = [];
      while (p + pos < e) {
        dec.decodeOut(instr);
        if (instr.code === Code.INVALID) { good = false; break; }
        pos += instr.length; n++;
        if (p + pos > e) { good = false; break; }
        if (instr.isIpRelMemoryOperand) note(true);
        const fc = instr.flowControl;
        if (fc === FlowControl.ConditionalBranch || fc === FlowControl.UnconditionalBranch) {
          const t = Number(instr.nearBranchTarget) - BASE - p;
          if (t > far && t < e - p) far = t;
        }
        if ((fc === FlowControl.Return || fc === FlowControl.UnconditionalBranch || fc === FlowControl.IndirectBranch || fc === FlowControl.Interrupt) && pos > far) { closed = true; break; }
      }
      if (good && closed) {
        st.gap_blocks_accepted++; st.instructions += n;
        for (let i = 0; i < pend.length; i += 3) commit(pend[i], pend[i + 1], pend[i + 2]);
        p += pos;
      } else {
        st.gap_blocks_rejected++;
        let q = Math.max(pos, 1);
        while (p + q < e && !(raw[o + q] === 0xcc && raw[o + q - 1] === 0xcc)) q++;
        p += q;
      }
    }
  };
  let cursor = text.rva;
  const lim = text.rva + raw.length;
  for (const f of fns) {
    if (f.begin < cursor || f.end > lim) { if (f.end > cursor && f.end <= lim) cursor = f.end; continue; }
    if (f.begin > cursor) sweepGap(cursor, f.begin);
    decodeFn(f.begin, f.end);
    cursor = f.end;
  }
  if (cursor < lim) sweepGap(cursor, lim);
  dec.free();
}
if (REF_CACHE && fs.existsSync(REF_CACHE)) {
  const c = fs.readFileSync(REF_CACHE);
  const hl = c.readUInt32LE(0);
  stats.text_scan = JSON.parse(c.toString('utf8', 4, 4 + hl));
  const fl = c.subarray(4 + hl, 4 + hl + SPAN), ac = c.subarray(4 + hl + SPAN, 4 + hl + 2 * SPAN);
  for (let i = 0; i < SPAN; i++) { F[i] |= fl[i]; ACC[i] = ac[i]; }
} else {
  scanText();
  if (REF_CACHE) {
    const fl = Buffer.alloc(SPAN);
    for (let i = 0; i < SPAN; i++) fl[i] = F[i] & (F_LEA | F_MEM);
    const h = Buffer.from(JSON.stringify(stats.text_scan));
    const hl = Buffer.alloc(4); hl.writeUInt32LE(h.length);
    fs.writeFileSync(REF_CACHE, Buffer.concat([hl, h, fl, Buffer.from(ACC.buffer)]));
  }
}
tick(`.text scanned: ${stats.text_scan.instructions} instructions`);

// ---- helpers for strings ---------------------------------------------------------------------
const printable = (c) => (c >= 0x20 && c < 0x7f) || c === 9 || c === 10 || c === 13;
const wprintable = (c) => printable(c) || (c >= 0xa0 && c < 0x2800) || (c >= 0x3000 && c < 0xa000) || (c >= 0xac00 && c < 0xd7a4) || (c >= 0xf900 && c < 0xfb00) || (c >= 0xff00 && c < 0xfff0);
// Length in bytes including the terminator, or 0. `limit` is the exclusive RVA bound.
function asciiLen(rva, limit, min = 4) {
  let p = rva;
  while (p < limit && printable(M[p - LO])) p++;
  if (p >= limit || M[p - LO] !== 0 || p - rva < min) return 0;
  return p - rva + 1;
}
function wideLen(rva, limit, min = 4) {
  let p = rva;
  while (p + 1 < limit) {
    const c = M.readUInt16LE(p - LO);
    if (c === 0) break;
    if (!wprintable(c)) return 0;
    p += 2;
  }
  if (p + 1 >= limit || (p - rva) / 2 < min) return 0;
  return p - rva + 2;
}
function cstr(rva, max = 512) {
  if (!ok(rva)) return null;
  let p = rva;
  while (p < HI && u8(p) && p - rva < max) p++;
  return M.toString('latin1', rva - LO, p - LO);
}
const DIR = 'dir: ';

// ---- C. PE directory structures ------------------------------------------------------------------
const iatSlotName = new Map(); // IAT slot RVA -> "dll!name"
function hintName(rva, how) {
  const n = cstr(rva + 2);
  if (n === null) return null;
  let len = 2 + n.length + 1;
  if (len & 1 && ok(rva + len) && u8(rva + len) === 0 && free(rva + len, rva + len + 1)) len++; // entries are word aligned
  claim(rva, rva + len, 'import_hint_name', 'impname', 'confirmed', how);
  return n;
}
// imports
if (dirs[1] && dirs[1].size) {
  const st = stats.imports = { dlls: 0, functions: 0, by_ordinal: 0 };
  let p = dirs[1].rva;
  const first = p;
  for (; ok(p, 20); p += 20) {
    const ilt = u32(p), nameRva = u32(p + 12), iat = u32(p + 16);
    if (!ilt && !nameRva && !iat) break;
    st.dlls++;
    const dll = cstr(nameRva);
    claim(nameRva, nameRva + dll.length + 1, 'import_dll_name', 'str', 'confirmed', DIR + 'IMAGE_IMPORT_DESCRIPTOR.Name');
    let n = 0;
    const look = ilt || iat;
    for (; ok(look + n * 8, 8); n++) {
      const lo = u32(look + n * 8), hi = u32(look + n * 8 + 4);
      if (!lo && !hi) break;
      st.functions++;
      let nm;
      if (hi & 0x80000000) { st.by_ordinal++; nm = '#' + (lo & 0xffff); }
      else nm = hintName(lo, DIR + `IMAGE_IMPORT_BY_NAME of ${dll}`);
      iatSlotName.set(iat + n * 8, dll + '!' + nm);
    }
    if (ilt) claim(ilt, ilt + (n + 1) * 8, 'import_lookup_table', 'ilt', 'confirmed', DIR + `OriginalFirstThunk array of ${dll} (${n} + null)`);
    claim(iat, iat + (n + 1) * 8, 'import_address_table', 'iat', 'confirmed', DIR + `FirstThunk array (IAT) of ${dll} (${n} + null)`);
  }
  claim(first, p + 20, 'import_descriptor_table', 'impdesc', 'confirmed', DIR + `import directory, ${st.dlls} IMAGE_IMPORT_DESCRIPTOR + null`);
  // is the IAT directory exactly the union of the per-DLL arrays?
  if (dirs[12] && dirs[12].size) st.iat_directory_fully_claimed = !OWN.subarray(dirs[12].rva - LO, dirs[12].rva + dirs[12].size - LO).includes(0);
}
// delay imports
if (dirs[13] && dirs[13].size) {
  const st = stats.delay_imports = { dlls: 0, functions: 0, by_ordinal: 0 };
  const bounds = [];
  let p = dirs[13].rva;
  const first = p;
  for (; ok(p, 32); p += 32) {
    const attrs = u32(p), nameRva = u32(p + 4), hmod = u32(p + 8), iat = u32(p + 12), int = u32(p + 16), bound = u32(p + 20), unload = u32(p + 24);
    if (!nameRva && !iat && !int) break;
    if (!(attrs & 1)) { st.va_based_descriptor_skipped = (st.va_based_descriptor_skipped || 0) + 1; continue; }
    st.dlls++;
    const dll = cstr(nameRva);
    claim(nameRva, nameRva + dll.length + 1, 'delay_import_dll_name', 'str', 'confirmed', DIR + 'ImgDelayDescr.rvaDLLName');
    if (hmod) claim(hmod, hmod + 8, 'delay_import_module_handle', 'D', 'confirmed', DIR + `ImgDelayDescr.rvaHmod of ${dll}`);
    let n = 0;
    for (; ok(int + n * 8, 8); n++) {
      const lo = u32(int + n * 8), hi = u32(int + n * 8 + 4);
      if (!lo && !hi) break;
      st.functions++;
      let nm;
      if (hi & 0x80000000) { st.by_ordinal++; nm = '#' + (lo & 0xffff); }
      else nm = hintName(lo, DIR + `IMAGE_IMPORT_BY_NAME (delay) of ${dll}`);
      iatSlotName.set(iat + n * 8, dll + '!' + nm);
    }
    claim(int, int + (n + 1) * 8, 'delay_import_name_table', 'dint', 'confirmed', DIR + `ImgDelayDescr.rvaINT of ${dll} (${n} + null)`);
    claim(iat, iat + (n + 1) * 8, 'delay_import_address_table', 'diat', 'confirmed', DIR + `ImgDelayDescr.rvaIAT of ${dll} (${n} + null); slots are relocated pointers to load thunks`);
    if (bound) bounds.push(bound);
    if (unload) claim(unload, unload + (n + 1) * 8, 'delay_import_unload_table', 'duiat', 'confirmed', DIR + `ImgDelayDescr.rvaUnloadIAT of ${dll}`);
  }
  claim(first, p + 32, 'delay_import_descriptor_table', 'dimpdesc', 'confirmed', DIR + `delay import directory, ${st.dlls} ImgDelayDescr + null`);
  // Bound IAT: the descriptors' rvaBoundIAT values are closer together than their slot counts
  // (they overlap if read as n+1 slots each), so the area is taken as the linker contribution
  // .didat$7 that contains all of them, provided it is entirely zero (nothing is bound).
  const c = pogoOf('.didat$7');
  if (bounds.length && c && bounds.every((x) => x >= c.rva && x < c.rva + c.size) && !M.subarray(c.rva - LO, c.rva + c.size - LO).some((x) => x)) {
    st.bound_iat = `one zero-filled contribution .didat$7 of ${c.size} bytes holding all ${bounds.length} rvaBoundIAT targets`;
    claim(c.rva, c.rva + c.size, 'delay_import_bound_table', 'dbiat', 'confirmed', `dir+pogo: contribution .didat$7; all ${bounds.length} ImgDelayDescr.rvaBoundIAT point into it; all zero (unbound)`);
  } else if (bounds.length) st.bound_iat = 'not claimed: no single zero-filled contribution holds the rvaBoundIAT targets';
}
// exports
if (dirs[0] && dirs[0].size) {
  const d = dirs[0].rva, dEnd = d + dirs[0].size;
  const nameRva = u32(d + 12), nFn = u32(d + 20), nNm = u32(d + 24), eat = u32(d + 28), npt = u32(d + 32), ot = u32(d + 36);
  const st = stats.exports = { functions: nFn, names: nNm, forwarders: 0 };
  claim(d, d + 40, 'export_directory', 'expdir', 'confirmed', DIR + 'IMAGE_EXPORT_DIRECTORY');
  const nm = cstr(nameRva);
  claim(nameRva, nameRva + nm.length + 1, 'export_dll_name', 'str', 'confirmed', DIR + 'IMAGE_EXPORT_DIRECTORY.Name');
  claim(eat, eat + nFn * 4, 'export_address_table', 'expfuncs', 'confirmed', DIR + `AddressOfFunctions (${nFn} RVAs)`);
  if (nNm) {
    claim(npt, npt + nNm * 4, 'export_name_pointer_table', 'expnames', 'confirmed', DIR + `AddressOfNames (${nNm} RVAs)`);
    claim(ot, ot + nNm * 2, 'export_ordinal_table', 'expords', 'confirmed', DIR + `AddressOfNameOrdinals (${nNm} words)`);
  }
  for (let i = 0; i < nNm; i++) {
    const r = u32(npt + i * 4), s = cstr(r);
    claim(r, r + s.length + 1, 'export_name', 'str', 'confirmed', DIR + 'export name string');
  }
  for (let i = 0; i < nFn; i++) {
    const r = u32(eat + i * 4);
    if (r >= d && r < dEnd) { st.forwarders++; const s = cstr(r); claim(r, r + s.length + 1, 'export_forwarder', 'str', 'confirmed', DIR + 'export forwarder string'); }
  }
}
// debug directory
if (debugEntries.length) {
  const d = dirs[6];
  claim(d.rva, d.rva + debugEntries.length * 28, 'debug_directory', 'dbgdir', 'confirmed', DIR + `${debugEntries.length} IMAGE_DEBUG_DIRECTORY entries`);
  const NAMES = { 2: ['debug_codeview', 'CodeView record'], 12: ['debug_vc_feature', 'VC_FEATURE counters'], 13: ['debug_pogo', 'POGO section-contribution list'] };
  stats.debug = { entries: debugEntries.map((e) => ({ type: e.type, size: e.size })) };
  for (const e of debugEntries) {
    if (!e.size || !ok(e.rva, e.size)) continue;
    const [type, what] = NAMES[e.type] || ['debug_data', 'debug data'];
    let more = '';
    if (e.type === 2) { const sig = M.toString('latin1', e.rva - LO, e.rva - LO + 4); more = ` (signature ${sig}, ${cstr(e.rva + 24)})`; stats.debug.codeview_signature = sig; stats.debug.pdb = cstr(e.rva + 24); }
    claim(e.rva, e.rva + e.size, type, 'dbg', 'confirmed', DIR + `debug type ${e.type}: ${what}${more}`);
  }
}
// TLS
if (dirs[9] && dirs[9].size) {
  const d = dirs[9].rva;
  const rs = u64(d) - BASE, re = u64(d + 8) - BASE, idx = u64(d + 16) - BASE, cb = u64(d + 24) - BASE;
  claim(d, d + 40, 'tls_directory', 'tlsdir', 'confirmed', DIR + 'IMAGE_TLS_DIRECTORY64');
  if (re > rs) claim(rs, re, 'tls_template', 'tlsdata', 'confirmed', DIR + 'TLS StartAddressOfRawData..EndAddressOfRawData');
  claim(idx, idx + 4, 'tls_index', 'D', 'confirmed', DIR + 'TLS AddressOfIndex (_tls_index, 4 bytes)');
  let n = 0;
  while (ok(cb + n * 8, 8) && u64(cb + n * 8)) n++;
  claim(cb, cb + (n + 1) * 8, 'tls_callback_table', 'tlscb', 'confirmed', DIR + `TLS AddressOfCallBacks (${n} + null)`);
  stats.tls = { template_bytes: re - rs, callbacks: n };
}
// load config
if (dirs[10] && dirs[10].size) {
  const d = dirs[10].rva, size = u32(d);
  claim(d, d + size, 'load_config_directory', 'loadcfg', 'confirmed', DIR + `IMAGE_LOAD_CONFIG_DIRECTORY64 (Size field ${size})`);
  const st = stats.load_config = { size, guard_flags: hex(size > 144 ? u32(d + 144) : 0) };
  const slot = (o, type, what) => {
    if (size < o + 8) return;
    const v = u64(d + o);
    if (!v) return;
    const r = v - BASE;
    st[what] = hex(v);
    claim(r, r + 8, type, 'D', 'confirmed', DIR + `load config ${what}`);
  };
  slot(88, 'security_cookie', 'SecurityCookie');
  slot(112, 'guard_cf_check_pointer', 'GuardCFCheckFunctionPointer');
  slot(120, 'guard_cf_dispatch_pointer', 'GuardCFDispatchFunctionPointer');
  const table = (o, type, what) => {
    if (size < o + 16) return;
    const v = u64(d + o), n = u64(d + o + 8);
    st[what + '_count'] = n;
    if (!v || !n) return;
    const stride = 4 + ((u32(d + 144) >>> 28) & 0xf);
    claim(v - BASE, v - BASE + n * stride, type, 'gfids', 'confirmed', DIR + `load config ${what} (${n} entries of ${stride} bytes)`);
  };
  table(128, 'guard_cf_function_table', 'GuardCFFunctionTable');
  table(160, 'guard_iat_entry_table', 'GuardAddressTakenIatEntryTable');
  table(176, 'guard_longjmp_table', 'GuardLongJumpTargetTable');
}
// resources
{
  const rs = secs.find((s) => s.name === '.rsrc');
  if (rs && dirs[2] && dirs[2].size) {
    const root = dirs[2].rva;
    const st = stats.resources = { directories: 0, data_entries: 0, names: 0, data_bytes: 0 };
    const seen = new Set();
    const walk = (off, depth) => {
      if (seen.has(off) || depth > 8) return;
      seen.add(off);
      const d = root + off;
      const n = u16(d + 12) + u16(d + 14);
      st.directories++;
      claim(d, d + 16 + n * 8, 'resource_directory', 'rsrcdir', 'confirmed', DIR + `IMAGE_RESOURCE_DIRECTORY level ${depth} (${n} entries)`);
      for (let i = 0; i < n; i++) {
        const id = u32(d + 16 + i * 8), o = u32(d + 20 + i * 8);
        if (id & 0x80000000) {
          const s = root + (id & 0x7fffffff), len = u16(s);
          if (!seen.has('n' + s)) { seen.add('n' + s); st.names++; claim(s, s + 2 + len * 2, 'resource_name', 'rsrcname', 'confirmed', DIR + 'IMAGE_RESOURCE_DIR_STRING_U'); }
        }
        if (o & 0x80000000) walk(o & 0x7fffffff, depth + 1);
        else {
          const e = root + o;
          if (seen.has('e' + e)) continue;
          seen.add('e' + e);
          st.data_entries++;
          claim(e, e + 16, 'resource_data_entry', 'rsrcent', 'confirmed', DIR + 'IMAGE_RESOURCE_DATA_ENTRY');
          const dr = u32(e), ds = u32(e + 4);
          st.data_bytes += ds;
          if (ds && !seen.has('d' + dr)) { seen.add('d' + dr); claim(dr, dr + ds, 'resource_data', 'rsrc', 'confirmed', DIR + `resource data (${ds} bytes) named by its data entry`); }
        }
      }
    };
    walk(0, 0);
  }
}
// exception directory
if (dirs[3] && dirs[3].size) {
  claim(dirs[3].rva, dirs[3].rva + dirs[3].size, 'runtime_function_table', 'pdata', 'confirmed', DIR + `exception directory, ${dirs[3].size / 12} RUNTIME_FUNCTION entries of 12 bytes`);
}
tick('PE directories');

// ---- D. unwind information and C++/SEH handler data ---------------------------------------------
{
  const st = stats.unwind = { runtime_functions: fns.length, distinct_unwind_info: 0, versions: {}, chained: 0, with_handler: 0, handlers: {}, funcinfo: 0, funcinfo_magic: {}, funcinfo_bad_magic: 0, unwind_map_entries: 0, try_blocks: 0, catch_handlers: 0, ip2state_entries: 0, scope_records: 0, es_type_lists: 0 };
  // collect distinct unwind RVAs, following chains to records that only a chain reaches
  const uw = new Set();
  const queue = [];
  for (const f of fns) if (!uw.has(f.unwind)) { uw.add(f.unwind); queue.push(f.unwind); }
  const recs = []; // { rva, flags, base (end of codes), handler }
  for (let qi = 0; qi < queue.length; qi++) {
    const r = queue[qi];
    if (!ok(r, 4)) continue;
    const ver = u8(r) & 7, flags = u8(r) >> 3, n = u8(r + 2);
    st.versions[ver] = (st.versions[ver] || 0) + 1;
    const base = r + 4 + ((n + 1) & ~1) * 2;
    const rec = { rva: r, flags, base, handler: 0 };
    if (flags & 4) { const cu = u32(base + 8); if (!uw.has(cu)) { uw.add(cu); queue.push(cu); st.reached_only_by_chain = (st.reached_only_by_chain || 0) + 1; } }
    else if (flags & 3) rec.handler = u32(base);
    recs.push(rec);
  }
  recs.sort((a, c) => a.rva - c.rva);
  st.distinct_unwind_info = recs.length;

  // classify the language handlers by what their code does
  const handlerUse = new Map();
  for (const r of recs) if (r.handler) handlerUse.set(r.handler, (handlerUse.get(r.handler) || 0) + 1);
  const fnAt = new Map(fns.map((f) => [f.begin, f]));
  const thunkImport = (rva) => { // jmp qword ptr [rip+disp] -> import name
    const o = img.rvaToOff(rva);
    if (o < 0 || b[o] !== 0xff || b[o + 1] !== 0x25) return null;
    return iatSlotName.get(rva + 6 + b.readInt32LE(o + 2)) || null;
  };
  const branchTargets = (rva) => {
    const f = fnAt.get(rva);
    const out = new Set();
    if (!f) return out;
    const dec = new Decoder(64, b.subarray(img.rvaToOff(f.begin), img.rvaToOff(f.begin) + (f.end - f.begin)), DecoderOptions.None);
    dec.ip = BigInt(BASE + f.begin);
    const ins = new Instruction();
    while (dec.canDecode) {
      dec.decodeOut(ins);
      if (ins.code === Code.INVALID) break;
      const fc = ins.flowControl;
      if (fc === FlowControl.Call || fc === FlowControl.UnconditionalBranch) { const t = Number(ins.nearBranchTarget) - BASE; if (inText(t)) out.add(t); }
    }
    dec.free();
    return out;
  };
  const hclass = new Map(); // handler RVA -> { cls, why }
  const pending = [];
  for (const h of handlerUse.keys()) {
    const imp = thunkImport(h);
    if (imp && /!__CxxFrameHandler/.test(imp)) hclass.set(h, { cls: 'cxx', why: `handler is a thunk to import ${imp}` });
    else if (imp && /!__C_specific_handler/.test(imp)) hclass.set(h, { cls: 'seh', why: `handler is a thunk to import ${imp}` });
    else pending.push(h);
  }
  const ofCls = (c) => [...hclass].filter((x) => x[1].cls === c).map((x) => x[0]);
  const bt = new Map(pending.map((h) => [h, branchTargets(h)]));
  for (const h of pending) {
    const t = bt.get(h);
    const cxx = ofCls('cxx').find((x) => t.has(x)), seh = ofCls('seh').find((x) => t.has(x));
    if (cxx) hclass.set(h, { cls: 'gs_cxx', why: `handler calls another routine, then the __CxxFrameHandler thunk at ${hex(BASE + cxx)} (the shape of __GSHandlerCheck_EH)`, t });
    else if (seh) hclass.set(h, { cls: 'gs_seh', why: `handler calls another routine, then the __C_specific_handler thunk at ${hex(BASE + seh)} (the shape of __GSHandlerCheck_SEH)`, t });
  }
  // the cookie-check routine is the call target the gs_cxx and gs_seh handlers share
  const common = new Set();
  for (const [, v] of hclass) if (v.t) for (const x of v.t) if (!hclass.has(x)) common.add(x);
  for (const h of pending) {
    if (hclass.has(h)) continue;
    const t = bt.get(h);
    const c = [...t].find((x) => common.has(x));
    if (c !== undefined && t.size === 1) hclass.set(h, { cls: 'gs', why: `handler only calls ${hex(BASE + c)}, the cookie check shared with the GS EH handlers (the shape of __GSHandlerCheck)` });
    else hclass.set(h, { cls: 'custom', why: 'handler is ordinary code in .text that calls none of the known frame handlers (hand-written assembly handler)' });
  }
  for (const [h, n] of handlerUse) { const c = hclass.get(h); const k = c.cls; (st.handlers[k] = st.handlers[k] || { handlers: 0, unwind_infos: 0 }); st.handlers[k].handlers++; st.handlers[k].unwind_infos += n; }
  st.handler_evidence = [...hclass].filter((x) => x[1].cls !== 'custom').map(([h, c]) => ({ handler: hex(BASE + h), class: c.cls, unwind_infos: handlerUse.get(h), why: c.why }));

  // C++ EH tables reached from a FuncInfo
  const typeDescFromCatch = stats._catchTypes = new Set();
  const doneFI = new Set();
  const X = 'dir>unwind: ';
  const funcInfo = (fi) => {
    if (doneFI.has(fi)) return;
    doneFI.add(fi);
    if (!ok(fi, 40)) { st.funcinfo_bad_magic++; return; }
    const magic = u32(fi);
    if (magic < 0x19930520 || magic > 0x19930522) { st.funcinfo_bad_magic++; return; }
    st.funcinfo++;
    st.funcinfo_magic[hex(magic)] = (st.funcinfo_magic[hex(magic)] || 0) + 1;
    const size = magic === 0x19930520 ? 32 : magic === 0x19930521 ? 36 : 40;
    claim(fi, fi + size, 'eh_funcinfo', 'ehfuncinfo', 'confirmed', X + `FuncInfo (magic ${hex(magic)}) named by the handler data of a C++ frame handler`);
    const maxState = u32(fi + 4), um = u32(fi + 8), nTry = u32(fi + 12), tm = u32(fi + 16), nIp = u32(fi + 20), ip = u32(fi + 24);
    if (um && maxState && ok(um, maxState * 8)) { st.unwind_map_entries += maxState; claim(um, um + maxState * 8, 'eh_unwind_map', 'ehunwind', 'confirmed', X + `FuncInfo.pUnwindMap (${maxState} UnwindMapEntry of 8 bytes)`); }
    if (tm && nTry && ok(tm, nTry * 20)) {
      st.try_blocks += nTry;
      claim(tm, tm + nTry * 20, 'eh_tryblock_map', 'ehtry', 'confirmed', X + `FuncInfo.pTryBlockMap (${nTry} TryBlockMapEntry of 20 bytes)`);
      for (let i = 0; i < nTry; i++) {
        const nC = u32(tm + i * 20 + 12), ha = u32(tm + i * 20 + 16);
        if (!nC || !ha || !ok(ha, nC * 20) || doneFI.has('h' + ha)) continue;
        doneFI.add('h' + ha);
        st.catch_handlers += nC;
        claim(ha, ha + nC * 20, 'eh_handler_array', 'ehcatch', 'confirmed', X + `TryBlockMapEntry.pHandlerArray (${nC} HandlerType of 20 bytes)`);
        for (let k = 0; k < nC; k++) { const td = u32(ha + k * 20 + 4); if (td) typeDescFromCatch.add(td); }
      }
    }
    if (ip && nIp && ok(ip, nIp * 8)) { st.ip2state_entries += nIp; claim(ip, ip + nIp * 8, 'eh_ip2state_map', 'ehip2state', 'confirmed', X + `FuncInfo.pIPToStateMap (${nIp} entries of 8 bytes)`); }
    if (size >= 36) {
      const es = u32(fi + 32);
      if (es && ok(es, 8)) { st.es_type_lists++; const n = u32(es); claim(es, es + 8, 'eh_estypelist', 'ehestypes', 'confirmed', X + `FuncInfo.pESTypeList header (${n} types)`); }
    }
  };
  const gsLen = (r) => (u32(r) & 4 ? 12 : 4); // GS_HANDLER_DATA: cookie offset + optional alignment pair
  const TYPE = { cxx: 'unwind_info_cxx_eh', gs_cxx: 'unwind_info_cxx_eh', seh: 'unwind_info_seh', gs_seh: 'unwind_info_seh', gs: 'unwind_info_gs', custom: 'unwind_info_custom_handler' };
  const HOW = {
    plain: 'dir: UNWIND_INFO named by a RUNTIME_FUNCTION',
    chain: 'dir: UNWIND_INFO with UNW_FLAG_CHAININFO (ends in the parent RUNTIME_FUNCTION)',
    cxx: 'dir: UNWIND_INFO + handler RVA (__CxxFrameHandler3 thunk) + FuncInfo RVA',
    gs_cxx: 'dir: UNWIND_INFO + handler RVA (GS + C++ frame handler) + FuncInfo RVA + GS_HANDLER_DATA',
    seh: 'dir: UNWIND_INFO + handler RVA (__C_specific_handler thunk) + SCOPE_TABLE',
    gs_seh: 'dir: UNWIND_INFO + handler RVA (GS + SEH handler) + SCOPE_TABLE + GS_HANDLER_DATA',
    gs: 'dir: UNWIND_INFO + handler RVA (GS cookie check) + GS_HANDLER_DATA',
    custom: 'dir: UNWIND_INFO + handler RVA (assembly handler); handler data extent bounded by the next unwind record',
  };
  for (let i = 0; i < recs.length; i++) {
    const r = recs[i];
    let end, type, how;
    if (r.flags & 4) { st.chained++; end = r.base + 12; type = 'unwind_info_chained'; how = HOW.chain; }
    else if (!r.handler) { end = r.base; type = 'unwind_info'; how = HOW.plain; }
    else {
      st.with_handler++;
      const c = hclass.get(r.handler).cls;
      type = TYPE[c]; how = HOW[c];
      const d = r.base + 4;
      if (c === 'cxx') { end = d + 4; funcInfo(u32(d)); }
      else if (c === 'gs_cxx') { end = d + 4 + gsLen(d + 4); funcInfo(u32(d)); }
      else if (c === 'gs') end = d + gsLen(d);
      else if (c === 'seh' || c === 'gs_seh') { const n = u32(d); st.scope_records += n; end = d + 4 + n * 16; if (c === 'gs_seh') end += gsLen(end); }
      else {
        // language-specific data of unknown layout: runs to the next unwind record, 4-byte entries
        const next = i + 1 < recs.length ? recs[i + 1].rva : d;
        end = next > d && next - d <= 64 ? next : d;
      }
    }
    claim(r.rva, end, type, 'unwind', hclass.get(r.handler) && hclass.get(r.handler).cls === 'custom' ? 'inferred' : 'confirmed', how);
  }
}
tick('unwind info');

// ---- E. MSVC RTTI ----------------------------------------------------------------------------------
const colSet = new Set();
{
  const st = stats.rtti = { locators: 0, type_descriptors: 0, type_descriptors_from_catch_only: 0, class_hierarchy_descriptors: 0, base_class_arrays: 0, base_class_descriptors: 0, vtables: 0, vtable_slots: 0, locators_without_vtable: 0 };
  const R = 'content+self-RVA: ';
  const tdDone = new Set(), chdDone = new Set(), bcdDone = new Set();
  const typeDesc = (rva, how) => {
    if (tdDone.has(rva) || !ok(rva, 17)) return tdDone.has(rva);
    const name = cstr(rva + 16, 4096);
    if (!name || name[0] !== '.') return false;
    tdDone.add(rva);
    if (claim(rva, rva + 16 + name.length + 1, 'rtti_type_descriptor', 'rtti_td', 'confirmed', how)) st.type_descriptors++;
    return true;
  };
  const bcd = (rva) => {
    if (bcdDone.has(rva) || !ok(rva, 28)) return;
    bcdDone.add(rva);
    if (claim(rva, rva + 28, 'rtti_base_class_descriptor', 'rtti_bcd', 'confirmed', 'RTTI: BaseClassDescriptor listed in a base class array')) st.base_class_descriptors++;
    typeDesc(u32(rva), 'RTTI: TypeDescriptor named by a BaseClassDescriptor (name starts with ".")');
    chd(u32(rva + 24));
  };
  function chd(rva) {
    if (chdDone.has(rva) || !ok(rva, 16)) return;
    chdDone.add(rva);
    if (claim(rva, rva + 16, 'rtti_class_hierarchy_descriptor', 'rtti_chd', 'confirmed', 'RTTI: ClassHierarchyDescriptor named by a locator or base class descriptor')) st.class_hierarchy_descriptors++;
    const n = u32(rva + 8), arr = u32(rva + 12);
    if (!n || n > 4096 || !ok(arr, n * 4)) return;
    // MSVC terminates the array with a zero entry
    const len = n * 4 + (ok(arr + n * 4, 4) && u32(arr + n * 4) === 0 && free(arr + n * 4, arr + n * 4 + 4) ? 4 : 0);
    if (claim(arr, arr + len, 'rtti_base_class_array', 'rtti_bca', 'confirmed', `RTTI: ClassHierarchyDescriptor.pBaseClassArray (${n} RVAs${len > n * 4 ? ' + null' : ''})`)) st.base_class_arrays++;
    for (let i = 0; i < n; i++) bcd(u32(arr + i * 4));
  }
  // complete object locators: signature 1 and a self RVA that equals the record's own address
  for (const s of secs) {
    if (s.name !== '.rdata' && s.name !== '.data') continue;
    const end = s.rva + Math.min(s.rawSize, s.vsize) - 24;
    for (let r = s.rva; r <= end; r += 4) {
      if (u32(r) !== 1 || u32(r + 20) !== r) continue;
      const td = u32(r + 12);
      const name = ok(td, 20) ? cstr(td + 16, 4096) : null;
      if (!name || !name.startsWith('.?A')) continue;
      if (!claim(r, r + 24, 'rtti_complete_object_locator', 'rtti_col', 'confirmed', R + 'RTTICompleteObjectLocator (signature 1, pSelf equals its own RVA, type name ".?A...")')) continue;
      st.locators++;
      colSet.add(r);
      typeDesc(td, 'RTTI: TypeDescriptor named by a complete object locator (name starts with ".?A")');
      chd(u32(r + 16));
    }
  }
  // type descriptors reachable only from catch clauses
  for (const td of stats._catchTypes) if (!tdDone.has(td) && typeDesc(td, 'dir>unwind: TypeDescriptor named by an EH HandlerType.pType')) st.type_descriptors_from_catch_only++;
  delete stats._catchTypes;
  // any other TypeDescriptor: relocated vftable pointer, null spare, decorated name
  const tiVft = new Map();
  for (const td of tdDone) { const v = u64(td); tiVft.set(v, (tiVft.get(v) || 0) + 1); }
  st.type_info_vftable = [...tiVft].map(([v, n]) => `${hex(v)} x${n}`).join(', ');
  st.type_descriptors_unreferenced = 0;
  for (const r of relocs) {
    if (OWN[r - LO] || !tiVft.has(u64(r)) || !ok(r, 24) || u64(r + 8) !== 0 || u8(r + 16) !== 0x2e) continue;
    if (typeDesc(r, 'reloc+content: TypeDescriptor (relocated type_info vftable pointer, null spare, name starts with ".")')) st.type_descriptors_unreferenced++;
  }
  // vtables: a relocated pointer to a locator, followed by relocated pointers into .text
  const colVt = new Set();
  for (const r of relocs) {
    const t = ptrRva(r);
    if (!colSet.has(t) || OWN[r - LO]) continue;
    let n = 0;
    for (let p = r + 8; isReloc(p) && inText(ptrRva(p)) && !OWN[p - LO]; p += 8, n++) if (n > 0 && (F[p - LO] & (F_LEA | F_PTR_TGT))) break;
    claim(r, r + 8, 'rtti_vtable_locator_pointer', 'vtbl_meta', 'confirmed', 'reloc: vtable[-1], a relocated pointer to an RTTICompleteObjectLocator');
    if (!n) continue;
    colVt.add(t);
    st.vtables++; st.vtable_slots += n;
    claim(r + 8, r + 8 + n * 8, 'vtable_rtti', 'vtbl', 'confirmed', `reloc+RTTI: ${n} relocated pointers into .text after a locator pointer`);
  }
  st.locators_without_vtable = colSet.size - colVt.size;
}
tick('RTTI');

// ---- F. linker-ordered tables named by POGO ------------------------------------------------------
{
  const st = stats.pogo = { contributions: pogo.length, crt_tables: 0, crt_table_pointers: 0 };
  for (const p of pogo) {
    if (!/^\.(CRT|rtc)\$/.test(p.name) || !ok(p.rva, p.size)) continue;
    // keep to whole unclaimed qwords (the TLS callback table lives in .CRT$XL*)
    let s = p.rva;
    const end = p.rva + p.size;
    while (s < end) {
      while (s < end && OWN[s - LO]) s++;
      let e = s;
      while (e < end && !OWN[e - LO]) e++;
      if (e > s) {
        let n = 0;
        for (let q = s; q + 8 <= e; q += 8) if (isReloc(q)) n++;
        st.crt_tables++; st.crt_table_pointers += n;
        claim(s, e, 'crt_init_table', 'crt', 'confirmed', `pogo: contribution ${p.name}; ${n} of ${Math.floor((e - s) / 8)} slots are relocated pointers`);
      }
      s = e;
    }
  }
}

// ---- G. UE4 native-function tables and pointer runs --------------------------------------------------
function identAt(rva) { // C identifier, NUL terminated, 3..95 chars (same test as tools/names.js)
  if (!ok(rva)) return 0;
  let p = rva;
  while (p < HI && p - rva < 96) {
    const c = u8(p);
    if (c === 0) break;
    if (!((c >= 0x30 && c <= 0x39) || (c >= 0x41 && c <= 0x5a) || (c >= 0x61 && c <= 0x7a) || c === 0x5f)) return 0;
    p++;
  }
  if (u8(p) !== 0 || p - rva < 3 || (u8(rva) >= 0x30 && u8(rva) <= 0x39)) return 0;
  return p - rva;
}
{
  const st = stats.native_tables = { tables: 0, entries: 0, single_pairs_ignored: 0 };
  const rdata = secs.find((s) => s.name === '.rdata');
  const pairAt = (r) => {
    if (!isReloc(r) || !isReloc(r + 8) || OWN[r - LO] || OWN[r + 8 - LO]) return false;
    const ps = ptrRva(r), pf = ptrRva(r + 8);
    return ps >= rdata.rva && ps < rdata.end && inText(pf) && identAt(ps) > 0;
  };
  for (let i = 0; i < relocs.length; i++) {
    const r = relocs[i];
    if (r < rdata.rva || r >= rdata.end || !pairAt(r)) continue;
    let n = 1;
    while (pairAt(r + n * 16)) n++;
    if (n < 2) { st.single_pairs_ignored++; continue; }
    st.tables++; st.entries += n;
    claim(r, r + n * 16, 'ue4_native_function_table', 'natives', 'inferred', `reloc+content: ${n} pairs {relocated pointer to a C identifier string, relocated pointer into .text}`);
    while (i + 1 < relocs.length && relocs[i + 1] < r + n * 16) i++;
  }
}
{
  const st = stats.pointer_runs = { runs: 0, pointers: 0, code_pointers: 0, code_pointers_at_pdata_begin: 0, code_pointers_inside_pdata_range: 0, code_pointers_outside_pdata: 0, code_pointers_outside_pdata_16_aligned: 0, data_pointers: 0, other_pointers: 0 };
  // sorted .pdata ranges: is an address strictly inside a function body?
  const fb = fns.map((f) => f.begin), fe = fns.map((f) => f.end);
  const insideBody = (t) => { let lo = 0, hi = fb.length - 1; while (lo < hi) { const m = (lo + hi + 1) >> 1; if (fb[m] <= t) lo = m; else hi = m - 1; } return fb[lo] < t && t < fe[lo]; };
  const P = 'reloc: ';
  for (let i = 0; i < relocs.length;) {
    const r = relocs[i];
    if (OWN[r - LO]) { i++; continue; }
    let n = 1;
    while (i + n < relocs.length && relocs[i + n] === r + n * 8 && !OWN[r + n * 8 - LO] && !(F[r + n * 8 - LO] & F_REF)) n++;
    let nText = 0, nData = 0, nStart = 0, nInside = 0;
    for (let k = 0; k < n; k++) {
      const t = ptrRva(r + k * 8);
      if (inText(t)) { nText++; if (fnBegin.has(t)) nStart++; else if (insideBody(t)) nInside++; else if ((t & 15) === 0) st.code_pointers_outside_pdata_16_aligned++; } else if (inSpan(t)) nData++;
    }
    st.code_pointers_inside_pdata_range += nInside; st.code_pointers_outside_pdata += nText - nStart - nInside;
    st.runs++; st.pointers += n; st.code_pointers += nText; st.code_pointers_at_pdata_begin += nStart; st.data_pointers += nData; st.other_pointers += n - nText - nData;
    const refd = F[r - LO];
    const by = (refd & F_LEA ? 'lea' : '') + (refd & F_MEM ? (refd & F_LEA ? '+' : '') + 'mem' : '') + (refd & F_PTR_TGT ? (refd & (F_LEA | F_MEM) ? '+' : '') + 'ptr' : '');
    const ref = by ? `; start referenced (${by})` : '; start unreferenced';
    if (nText === n) {
      if (n === 1) claim(r, r + 8, 'code_pointer', 'fptr', 'confirmed', P + `relocated pointer into .text${nStart ? ' (.pdata function start)' : ''}` + ref);
      else if ((refd & F_LEA) && nInside === 0) { st.lea_referenced_code_tables = (st.lea_referenced_code_tables || 0) + 1; claim(r, r + n * 8, 'code_pointer_table', 'vtbl', 'inferred', P + `${n} consecutive relocated pointers into .text, none inside a .pdata function body (${nStart} at .pdata starts); start taken by lea in .text as a constructor does; no RTTI`); }
      else claim(r, r + n * 8, 'code_pointer_table', 'fptrs', 'confirmed', P + `${n} consecutive relocated pointers into .text (${nStart} at .pdata function starts)` + ref);
    } else if (nText === 0) {
      if (n === 1) claim(r, r + 8, 'data_pointer', 'ptr', 'confirmed', P + 'relocated pointer into a data section' + ref);
      else claim(r, r + n * 8, 'data_pointer_table', 'ptrs', 'confirmed', P + `${n} consecutive relocated pointers into data sections` + ref);
    } else claim(r, r + n * 8, 'mixed_pointer_table', 'ptrs', 'confirmed', P + `${n} consecutive relocated pointers (${nText} into .text, ${n - nText} into data)` + ref);
    i += n;
  }
}
tick('pointer tables');

// ---- H. referenced strings and scalar constants ---------------------------------------------------
const bssPogo = pogoOf('.bss');
const dataSec = secs.find((s) => s.name === '.data');
let bssStart = dataSec ? dataSec.rva + Math.min(dataSec.rawSize, dataSec.vsize) : 0;
let bssHow = 'section table: beyond SizeOfRawData, no file bytes';
if (dataSec && bssPogo && bssPogo.rva > dataSec.rva && bssPogo.rva < bssStart) {
  let z = true;
  for (let r = bssPogo.rva; r < bssStart; r++) if (u8(r)) { z = false; break; }
  if (z) { stats.bss_file_backed_zero_bytes = bssStart - bssPogo.rva; bssStart = bssPogo.rva; }
}
stats.bss_start = dataSec ? hex(BASE + bssStart) : null;
const isBss = (rva) => dataSec && rva >= bssStart && rva < dataSec.end;
{
  const st = stats.referenced = { targets: 0, ascii: 0, utf16: 0, short_strings: 0, utf16_with_non_ascii_units: 0, scalars: 0, float_scalars: 0, already_claimed: 0, unresolved: 0 };
  const refHow = (f) => 'xref(' + [f & F_LEA ? 'lea' : '', f & F_MEM ? 'mem' : '', f & F_PTR_TGT ? 'ptr' : ''].filter(Boolean).join('+') + ')';
  const scalar = (r, s) => {
    const a = ACC[r - LO], cls = a & 7, size = CLASS_SIZE[cls];
    if (!size || (a & 0x40)) return false;
    if (r + size > s.end || !free(r, r + size)) return false;
    // do not run into another referenced object
    for (let k = 1; k < size; k++) if (F[r + k - LO] & F_REF) return false;
    const fl = (a & 0x80) && !(a & 0x20);
    let type;
    if (fl) type = size === 4 ? 'float32' : size === 8 ? 'float64' : `simd${size * 8}_const`;
    else type = (a & 0x80 ? `simd${size * 8}_const` : `scalar${size * 8}`);
    if ((a & 0x80) && size < 16 && !fl) type = `scalar${size * 8}`;
    if (!claim(r, r + size, type, 'D', 'inferred', `xref: every RIP-relative operand in .text that names this address is ${size} bytes wide${a & 0x80 ? (fl ? ', all SSE/x87 float' : ', some SSE float') : ''}`)) return false;
    st.scalars++; if (a & 0x80) st.float_scalars++;
    return true;
  };
  // Pass 1: text. Strings go first for every referenced address, because the compiler copies
  // short literals inline (movups/movsd/mov loads from the middle of a string), which would
  // otherwise look like scalar constants.
  // Pass 2: scalars at the referenced addresses that are still free.
  for (let pass = 1; pass <= 2; pass++) for (const s of secs) {
    if (s.name === '.reloc' || s.name === '.rsrc' || s.name === '.pdata') continue;
    const lim = s.name === '.data' ? bssStart : s.rva + Math.min(s.rawSize, s.vsize);
    for (let r = s.rva; r < lim; r++) {
      const f = F[r - LO] & F_REF;
      if (!f) continue;
      if (pass === 2) { if (!OWN[r - LO] && !((f & F_MEM) && scalar(r, s))) st.unresolved++; continue; }
      st.targets++;
      if (OWN[r - LO]) { st.already_claimed++; continue; }
      let limit = r;
      while (limit < lim && !OWN[limit - LO] && limit - r < 65536) limit++;
      // A constant only ever loaded as a 4- or 8-byte float is a float even if its bytes happen to
      // be printable (0.7f is "333?"): text there must be clearly longer than the load.
      const acc = ACC[r - LO], fsize = f === F_MEM && (acc & 0x80) && !(acc & 0x60) ? CLASS_SIZE[acc & 7] : 0;
      let n = asciiLen(r, limit, fsize === 4 || fsize === 8 ? 2 * fsize + 1 : 4);
      if (n) { claim(r, r + n, 'string_ascii', 'str', 'inferred', `${refHow(f)}+content: printable ASCII + NUL`); st.ascii++; continue; }
      n = (r & 1) === 0 ? wideLen(r, limit) : 0;
      if (n) { claim(r, r + n, 'string_utf16', 'wstr', 'inferred', `${refHow(f)}+content: printable UTF-16LE + NUL`); st.utf16++; for (let k = r; k < r + n; k += 2) if (u16(k) >= 0x7f) { st.utf16_with_non_ascii_units++; break; } continue; }
      {
        // Shorter than 4 characters: accepted only when nothing but zero fill follows up to the
        // next address that is taken (or the next item). If the address itself is only read
        // (an inline copy of the literal), one character is not enough: small integers look
        // the same, so at least 2 UTF-16 units or 3 bytes are required.
        const taken = (f & (F_LEA | F_PTR_TGT)) !== 0, stop = taken ? F_REF : F_LEA | F_PTR_TGT;
        let pe = r + 1;
        while (pe < limit && pe - r < 64 && !(F[pe - LO] & stop)) pe++;
        const zeroTo = (from) => { for (let k = from; k < pe; k++) if (M[k - LO]) return false; return true; };
        const a = asciiLen(r, pe, taken ? 1 : 3), w = (r & 1) === 0 ? wideLen(r, pe, taken ? 1 : 2) : 0;
        const aOk = a && zeroTo(r + a), wOk = w && zeroTo(r + w);
        if (wOk) { claim(r, r + w, 'string_utf16', 'wstr', 'guess', `${refHow(f)}+content: short, ${w / 2 - 1} UTF-16LE unit(s) + NUL, only zero fill after it${aOk ? ' (also reads as 1 ASCII character)' : ''}`); st.utf16++; st.short_strings++; continue; }
        if (aOk) { claim(r, r + a, 'string_ascii', 'str', 'guess', `${refHow(f)}+content: short, ${a - 1} printable byte(s) + NUL, only zero fill after it`); st.ascii++; st.short_strings++; continue; }
      }
    }
  }
}
// Unreferenced strings: text that nothing in .text or the relocation table points at (kept by
// the linker with the rest of its object file's .rdata). Content evidence only.
{
  const st = stats.unreferenced_strings = { ascii: 0, utf16: 0 };
  const strEnd = new Set();
  for (const it of items) if (it.type.startsWith('string_')) strEnd.add(it.e);
  const aprint = (c) => c >= 0x20 && c < 0x7f;
  const wideAscii = (rva, limit, min) => {
    let p = rva;
    while (p + 1 < limit) { const c = M.readUInt16LE(p - LO); if (c === 0) break; if (!aprint(c)) return 0; p += 2; }
    if (p + 1 >= limit || (p - rva) / 2 < min) return 0;
    return p - rva + 2;
  };
  for (const s of secs) {
    if (s.name !== '.rdata' && s.name !== '.data' && s.name !== '_RDATA') continue;
    const lim = s.name === '.data' ? bssStart : s.rva + Math.min(s.rawSize, s.vsize);
    let p = s.rva;
    while (p < lim) {
      if (OWN[p - LO]) { p++; continue; }
      let e = p;
      while (e < lim && !OWN[e - LO]) e++;
      // [p, e) is an unclaimed run; text right after a string needs 4 characters, elsewhere 8 / 6
      let near = strEnd.has(p), q = p, pe = p;
      while (q < e) {
        if (M[q - LO] === 0) { q++; continue; }
        if (pe <= q) { pe = q + 1; while (pe < e && !(F[pe - LO] & F_REF)) pe++; }
        let n = asciiLen(q, pe, near ? 4 : 8);
        if (n) { claim(q, q + n, 'string_ascii_unreferenced', 'str', 'guess', `content: printable ASCII + NUL, no reference found${near ? ', follows another string' : ''}`); st.ascii++; q += n; near = true; continue; }
        n = (q & 1) === 0 ? wideAscii(q, pe, near ? 4 : 6) : 0;
        if (n) { claim(q, q + n, 'string_utf16_unreferenced', 'wstr', 'guess', `content: ASCII-range UTF-16LE + NUL, no reference found${near ? ', follows another string' : ''}`); st.utf16++; q += n; near = true; continue; }
        // not text here: step over this byte; binary data ends the "near" state
        if (!aprint(M[q - LO])) near = false;
        q++;
      }
      p = e;
    }
  }
}
tick('strings and constants');

// ---- I. fill: padding, bss, unknown ---------------------------------------------------------------
items.sort((a, c) => a.s - c.s);
const rows = [];
{
  const st = stats.fill = { padding: 0, unknown_referenced: 0, unknown: 0, bss: 0 };
  const emit = (s, e, kind, type, prefix, confidence, how) => rows.push({ s, e, kind, type, prefix, confidence, how });
  const gap = (sec, a, e) => {
    const vEnd = sec.rva + sec.vsize;
    if (a >= vEnd) { emit(a, e, 'padding', 'section_tail_padding', 'pad', 'confirmed', 'section table: raw bytes past VirtualSize (file alignment)'); st.padding++; return; }
    if (e > vEnd) { gap(sec, a, vEnd); gap(sec, vEnd, e); return; }
    if (isBss(a) || (isBss(e - 1) && a < bssStart)) {
      if (a < bssStart) { gap(sec, a, bssStart); a = bssStart; }
      let p = a;
      while (p < e) {
        let q = p + 1;
        if (BSS_SPLIT) while (q < e && !(F[q - LO] & F_REF)) q++; else q = e;
        const f = F[p - LO] & F_REF;
        emit(p, q, 'bss', f ? 'bss_referenced' : 'bss', f ? 'B' : 'bss', f ? 'inferred' : 'confirmed',
          f ? 'pogo .bss + xref: starts at a referenced address, runs to the next one' : 'pogo .bss: no reference to its start');
        st.bss++;
        p = q;
      }
      return;
    }
    // split at referenced addresses; a zero/0xCC piece whose start nothing references is padding
    let p = a;
    while (p < e) {
      let q = p + 1;
      while (q < e && !(F[q - LO] & F_REF)) q++;
      const f = F[p - LO] & F_REF;
      const fill = M[p - LO];
      let uniform = fill === 0 || fill === 0xcc;
      if (uniform) for (let i = p - LO + 1, n = q - LO; i < n; i++) if (M[i] !== fill) { uniform = false; break; }
      if (uniform && !f) { emit(p, q, 'padding', fill ? 'int3_padding' : 'zero_padding', 'pad', q - p < 16 ? 'inferred' : 'guess', fill ? 'content: unreferenced 0xCC fill between items' : 'content: unreferenced zero fill between items'); st.padding++; }
      else if (f) { emit(p, q, 'unknown', uniform ? 'unknown_referenced_zero' : 'unknown_referenced', 'D', 'guess', 'xref: start referenced, layout not identified; runs to the next referenced address or item'); st.unknown_referenced++; }
      else { emit(p, q, 'unknown', 'unknown', 'unk', 'guess', 'none: no reference to its start, not uniform fill'); st.unknown++; }
      p = q;
    }
  };
  let ii = 0;
  for (const sec of secs) {
    let cur = sec.rva;
    while (ii < items.length && items[ii].s < sec.end) {
      const it = items[ii++];
      if (it.s > cur) gap(sec, cur, it.s);
      const kind = it.type.endsWith('padding') ? 'padding' : isBss(it.s) ? 'bss' : SECTION_KIND[sec.name];
      rows.push({ s: it.s, e: it.e, kind, type: it.type, prefix: it.prefix, confidence: it.confidence, how: it.how });
      cur = it.e;
    }
    if (cur < sec.end) gap(sec, cur, sec.end);
  }
}
tick(`rows: ${rows.length}`);

// ---- self-check: exact tiling of each section's max(vsize, rawSize) extent ---------------------------
const failures = [];
const perSection = {};
const perType = {};
{
  let ri = 0;
  for (const sec of secs) {
    let cur = sec.rva, n = 0, unknown = 0;
    const byKind = {};
    while (ri < rows.length && rows[ri].s < sec.end) {
      const r = rows[ri++];
      if (r.s > cur) failures.push(`${sec.name}: gap ${hex(BASE + cur)}-${hex(BASE + r.s)}`);
      if (r.s < cur) failures.push(`${sec.name}: overlap at ${hex(BASE + r.s)}`);
      if (r.e <= r.s) failures.push(`${sec.name}: empty row at ${hex(BASE + r.s)}`);
      if (r.e > sec.end) failures.push(`${sec.name}: row ${hex(BASE + r.s)} crosses the section end`);
      if (!['data', 'rodata', 'bss', 'padding', 'unknown'].includes(r.kind)) failures.push(`bad kind ${r.kind}`);
      r.section = sec.name;
      cur = Math.max(cur, r.e); n++;
      const bytes = r.e - r.s;
      if (r.kind === 'unknown') unknown += bytes;
      (byKind[r.kind] = byKind[r.kind] || { count: 0, bytes: 0 }).count++; byKind[r.kind].bytes += bytes;
      (perType[r.type] = perType[r.type] || { count: 0, bytes: 0 }).count++; perType[r.type].bytes += bytes;
    }
    if (cur !== sec.end) failures.push(`${sec.name}: coverage ends at ${hex(BASE + cur)}, section ends at ${hex(BASE + sec.end)}`);
    perSection[sec.name] = { start: hex(BASE + sec.rva), end: hex(BASE + sec.end), bytes: sec.ext, rows: n, by_kind: byKind, unknown_bytes: unknown, unknown_percent: +(100 * unknown / sec.ext).toFixed(3) };
  }
  if (rows.some((r) => !r.section)) failures.push('rows outside every target section');
}
// Where did the relocated qwords end up? (Every one of them is a pointer; a relocation that
// starts in one row and ends in the next would mean a row boundary cuts a pointer.)
{
  const by = stats.relocated_qwords_by_type = {};
  let ri = 0, cut = 0;
  for (const r of relocs) {
    while (ri < rows.length && rows[ri].e <= r) ri++;
    const row = rows[ri];
    by[row.type] = (by[row.type] || 0) + 1;
    if (r + 8 > row.e) cut++;
  }
  stats.relocated_qwords_cut_by_a_row_boundary = cut;
  if (cut) failures.push(`${cut} relocated qwords straddle a row boundary`);
}
if (failures.length) {
  console.error('TILING FAILED:\n' + failures.slice(0, 20).join('\n'));
  process.exit(1);
}

// ---- write -----------------------------------------------------------------------------------------
if (WRITE) {
  const fd = fs.openSync(OUT, 'w');
  let buf = ['[\n'];
  let size = 0;
  for (let i = 0; i < rows.length; i++) {
    const r = rows[i];
    const line = JSON.stringify({ start: '0x' + vaHex(r.s), end: '0x' + vaHex(r.e), section: r.section, kind: r.kind, symbol: `${r.prefix}_${vaHex(r.s)}`, type: r.type, confidence: r.confidence, how: r.how }) + (i + 1 < rows.length ? ',\n' : '\n');
    buf.push(line); size += line.length;
    if (size > 1 << 24) { fs.writeSync(fd, buf.join('')); buf = []; size = 0; }
  }
  buf.push(']\n');
  fs.writeSync(fd, buf.join(''));
  fs.closeSync(fd);
}

// ---- report ----------------------------------------------------------------------------------------
const totalBytes = secs.reduce((a, s) => a + s.ext, 0);
const typeTable = Object.entries(perType).sort((a, c) => c[1].bytes - a[1].bytes);
console.log('tiling: OK (' + secs.map((s) => `${s.name} ${perSection[s.name].rows} rows`).join(', ') + ')');
console.log(`rows ${rows.length}, bytes ${totalBytes}`);
console.log('\ntype                              count        bytes');
for (const [t, v] of typeTable) console.log(t.padEnd(30) + String(v.count).padStart(9) + String(v.bytes).padStart(13));
console.log('\nsection   bytes        rows     unknown bytes   unknown %');
for (const s of secs) { const p = perSection[s.name]; console.log(s.name.padEnd(8) + String(p.bytes).padStart(10) + String(p.rows).padStart(10) + String(p.unknown_bytes).padStart(15) + String(p.unknown_percent.toFixed(3)).padStart(11)); }
console.log('\nstats ' + JSON.stringify(stats, null, 1));
console.log('claims rejected ' + JSON.stringify(conflicts));
console.log('duplicate claims (same range reached twice) ' + JSON.stringify(duplicates));
const summary = { rows: rows.length, bytes: totalBytes, per_section: perSection, per_type: Object.fromEntries(typeTable), stats, claims_rejected: conflicts, duplicate_claims: duplicates };
const SUMMARY = path.join(ROOT, 'audit', 'data_map_summary.json');
if (WRITE) fs.writeFileSync(SUMMARY, JSON.stringify(summary, null, 1) + '\n');
if (WRITE) logRun({ tool: 'datamap', version: VERSION, outputs: [OUT, SUMMARY], note: JSON.stringify({ rows: rows.length, unknown_percent: Object.fromEntries(secs.map((s) => [s.name, perSection[s.name].unknown_percent])) }) });
tick('done');
