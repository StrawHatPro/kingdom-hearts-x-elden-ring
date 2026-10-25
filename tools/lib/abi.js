// What the original machine code says about the registers that carry values across a call.
// The decompiler's C names a parameter and its type but not its register, and it types a value
// it knows nothing about as an integer (undefined4) even when the code keeps it in XMM1. In C
// that distinction decides the register, so it is read back from the instructions:
//   entryClasses(va)  for each of the four argument positions, whether the code entered at `va`
//                     reads the integer register (RCX, RDX, R8, R9: 'i'), the vector register
//                     (XMM0..XMM3: 'f', or 'd' when first read as a double), both ('b') or
//                     neither (null) before writing it.
//   returnClass(va)   for the call instruction at `va`, whether the code that follows reads RAX
//                     or XMM0 (the value the callee returned) before writing it.
// Both follow direct jumps and both sides of conditional jumps over a bounded number of
// instructions; a direct call is followed into the callee for the positions still untouched
// (a forwarded argument). Indirect transfers end a path.
const IMG_BASE = 0x140000000;
let image = null, iced = null, factory = null;
function init() {
  if (image) return;
  image = require('./pe').load();
  iced = require('iced-x86');
  factory = new iced.InstructionInfoFactory();
}
// register -> bit: 0..3 integer argument registers, 4..7 XMM0..3, 8 RAX
let regBit = null;
function bitOf(reg) {
  if (!regBit) {
    regBit = new Map();
    const R = iced.Register;
    [['RCX', 'ECX', 'CX', 'CL', 'CH'], ['RDX', 'EDX', 'DX', 'DL', 'DH'], ['R8', 'R8D', 'R8W', 'R8L'], ['R9', 'R9D', 'R9W', 'R9L']].forEach((names, i) => { for (const n of names) if (R[n] !== undefined) regBit.set(R[n], i); });
    for (let i = 0; i < 4; i++) for (const p of ['XMM', 'YMM', 'ZMM']) if (R[p + i] !== undefined) regBit.set(R[p + i], 4 + i);
    for (const n of ['RAX', 'EAX', 'AX', 'AL', 'AH']) regBit.set(R[n], 8);
  }
  const b = regBit.get(reg);
  return b === undefined ? -1 : b;
}
// scalar instructions that write the low part of their destination and keep the rest: not a read of it
let MERGE = null, DOUBLE = null;
function tables() {
  if (MERGE) return;
  const M = iced.Mnemonic;
  MERGE = new Set(['Cvtsi2ss', 'Cvtsi2sd', 'Cvtss2sd', 'Cvtsd2ss', 'Movss', 'Movsd', 'Sqrtss', 'Sqrtsd', 'Roundss', 'Roundsd', 'Rcpss', 'Rsqrtss'].map((n) => M[n]));
  DOUBLE = new Set(Object.keys(M).filter((n) => isNaN(n) && /^(?:V?(?:Mov|Add|Sub|Mul|Div|Min|Max|Sqrt|Comi|Ucomi|Cmp|Round|And|Andn|Or|Xor|Unpckl|Unpckh|Shuf)[sp]d|V?Cvtt?[sp]d2\w+|V?Movq|V?Mov[lh]pd)$/.test(n)).map((n) => M[n]));
}
// Walks from `start` with the registers in `killed` already written. Returns { live, dbl }:
// bit masks of the registers read before being written, and of those read as a double.
function walk(start, killed0, budget) {
  init(); tables();
  const { FlowControl, OpAccess, OpKind } = iced;
  let live = 0, dbl = 0;
  const seen = new Map(); // ip -> killed mask it was visited with
  const work = [[start, killed0]];
  const ALL = 0x1FF;
  while (work.length && budget > 0) {
    let [ip, killed] = work.pop();
    for (;;) {
      if ((killed | live) === ALL || budget-- <= 0) break;
      const before = seen.get(ip);
      if (before !== undefined && (before & ~killed) === 0) break; // seen with no more registers killed than now
      seen.set(ip, before === undefined ? killed : before & killed);
      const off = image.rvaToOff(ip - IMG_BASE);
      if (off < 0) break;
      const dec = new iced.Decoder(64, image.b.subarray(off, off + 16), iced.DecoderOptions.None);
      dec.ip = BigInt(ip);
      const ins = dec.decode();
      const flow = ins.flowControl, len = ins.length, mn = ins.mnemonic;
      if (ins.isInvalid) { ins.free(); dec.free(); break; }
      const direct = ins.op0Kind === OpKind.NearBranch64 ? Number(ins.nearBranchTarget) : null;
      const info = factory.info(ins);
      const used = info.usedRegisters();
      let wr = 0;
      const op0reg = ins.op0Kind === OpKind.Register ? ins.op0Register : -1;
      for (const u of used) {
        const b = bitOf(u.register), acc = u.access;
        u.free();
        if (b < 0) continue;
        const merge = MERGE.has(mn) && b >= 4 && bitOf(op0reg) === b;
        const reads = acc === OpAccess.Read || acc === OpAccess.CondRead || acc === OpAccess.ReadWrite || acc === OpAccess.ReadCondWrite;
        if (reads && !merge && !(killed & (1 << b)) && flow !== FlowControl.Call && flow !== FlowControl.IndirectCall && flow !== FlowControl.Return) { if (!(live & (1 << b)) && DOUBLE.has(mn)) dbl |= 1 << b; live |= 1 << b; }
        if (acc === OpAccess.Write || acc === OpAccess.ReadWrite) wr |= 1 << b;
      }
      info.free(); ins.free(); dec.free();
      killed |= wr;
      if (flow === FlowControl.Next) { ip += len; continue; }
      if (flow === FlowControl.ConditionalBranch && direct !== null) { work.push([direct, killed]); ip += len; continue; }
      if ((flow === FlowControl.UnconditionalBranch || flow === FlowControl.Call) && direct !== null) { killed |= 0x100; ip = direct; continue; } // a callee is entered with RAX undefined
      break;
    }
  }
  return { live, dbl };
}
const entryCache = new Map();
function entryClasses(va) {
  let r = entryCache.get(va);
  if (r) return r;
  const { live, dbl } = walk(va, 0x100, 600);
  r = [0, 1, 2, 3].map((i) => { const g = live & (1 << i), x = live & (1 << (4 + i)); return g && x ? 'b' : g ? 'i' : x ? (dbl & (1 << (4 + i)) ? 'd' : 'f') : null; });
  entryCache.set(va, r);
  return r;
}
// 'i', 'f' (float), 'd' (double), 'b' (both) or null for the value used after the call at `site`
function returnClass(site) {
  init();
  const off = image.rvaToOff(site - IMG_BASE);
  if (off < 0) return null;
  const dec = new iced.Decoder(64, image.b.subarray(off, off + 16), iced.DecoderOptions.None);
  dec.ip = BigInt(site);
  const ins = dec.decode();
  const len = ins.length, flow = ins.flowControl;
  ins.free(); dec.free();
  if (flow !== iced.FlowControl.Call && flow !== iced.FlowControl.IndirectCall) return null;
  const { live, dbl } = walk(site + len, 0xEF, 300); // only RAX and XMM0 are of interest
  const g = live & 0x100, x = live & 0x10;
  return g && x ? 'b' : g ? 'i' : x ? (dbl & 0x10 ? 'd' : 'f') : null;
}
// Addresses of the call instructions in [lo, hi) with their direct target (or null when indirect).
function callSites(ranges) {
  init();
  const out = [];
  for (const [s, e] of ranges) {
    const lo = parseInt(s, 16), hi = parseInt(e, 16);
    const off = image.rvaToOff(lo - IMG_BASE);
    if (off < 0) continue;
    const dec = new iced.Decoder(64, image.b.subarray(off, off + (hi - lo)), iced.DecoderOptions.None);
    dec.ip = BigInt(lo);
    const ins = new iced.Instruction();
    while (dec.canDecode) {
      dec.decodeOut(ins);
      const f = ins.flowControl;
      if (f === iced.FlowControl.Call || f === iced.FlowControl.IndirectCall) out.push({ site: Number(ins.ip), target: ins.op0Kind === iced.OpKind.NearBranch64 ? Number(ins.nearBranchTarget) : null });
      else if (f === iced.FlowControl.UnconditionalBranch && ins.op0Kind === iced.OpKind.NearBranch64) { const t = Number(ins.nearBranchTarget); if (!ranges.some(([a, b]) => t >= parseInt(a, 16) && t < parseInt(b, 16))) out.push({ site: Number(ins.ip), target: t, tail: true }); }
    }
    ins.free(); dec.free();
  }
  return out;
}
// For the call at `site` inside `ranges`: for each of the four argument positions, 'f', 'd' or
// 'x' (vector register, width unknown) when the straight-line code in front of the call writes
// XMMn and does not write the integer register of that position (other than to hold the address
// the call goes through); otherwise null. The scan stops
// at the previous call, return or unconditional jump, or after 60 instructions; the result's
// `stop` says where: 'call' (the integer registers were dead at that point), 'entry' (they may
// still hold the function's own arguments) or 'other' (unknown predecessors).
function argClasses(ranges, site, entry) {
  init(); tables();
  const { FlowControl, OpAccess } = iced;
  const r = ranges.find(([s, e]) => site >= parseInt(s, 16) && site < parseInt(e, 16));
  if (!r) return [null, null, null, null];
  const lo = parseInt(r[0], 16), hi = parseInt(r[1], 16);
  const off = image.rvaToOff(lo - IMG_BASE);
  if (off < 0) return [null, null, null, null];
  const dec = new iced.Decoder(64, image.b.subarray(off, off + (hi - lo)), iced.DecoderOptions.None);
  dec.ip = BigInt(lo);
  const list = []; // [flow, written mask, double mask, scalar mask]
  let found = false, scratch = 0; // integer registers the call itself goes through: call [r9+10h]
  while (dec.canDecode) {
    const ins = dec.decode();
    if (Number(ins.ip) === site) {
      for (const reg of [ins.memoryBase, ins.memoryIndex, ins.op0Kind === iced.OpKind.Register ? ins.op0Register : 0]) { const b = bitOf(reg); if (b >= 0 && b < 4) scratch |= 1 << b; }
      ins.free(); found = true; break;
    }
    const info = factory.info(ins);
    let wr = 0;
    for (const u of info.usedRegisters()) { const b = bitOf(u.register), acc = u.access; u.free(); if (b >= 0 && b < 8 && (acc === OpAccess.Write || acc === OpAccess.ReadWrite)) wr |= 1 << b; }
    const name = iced.Mnemonic[ins.mnemonic] || '';
    list.push([ins.flowControl, wr, DOUBLE.has(ins.mnemonic), /s[sd]$/i.test(name) || /^V?Cvt/i.test(name)]);
    info.free(); ins.free();
  }
  dec.free();
  const out = [null, null, null, null];
  if (!found) return out;
  let seen = 0, stop = lo === (entry === undefined ? parseInt(ranges[0][0], 16) : entry) ? 'entry' : 'other';
  for (let i = list.length - 1, n = 0; i >= 0; i--, n++) {
    const [flow, wr, dbl, scalar] = list[i];
    if (n >= 60) { stop = 'other'; break; }
    if (flow !== FlowControl.Next && flow !== FlowControl.ConditionalBranch) { stop = flow === FlowControl.Call || flow === FlowControl.IndirectCall ? 'call' : 'other'; break; }
    for (let p = 0; p < 4; p++) if ((wr & (0x10 << p)) && !(seen & (0x10 << p))) out[p] = dbl ? 'd' : scalar ? 'f' : 'x';
    seen |= wr;
  }
  for (let p = 0; p < 4; p++) if (seen & ~scratch & (1 << p)) out[p] = null;
  out.stop = stop;
  return out;
}
// Whether the function at `entry` (code in `ranges`) returns its value in XMM0: 'f', 'd' or 'x'
// (width unknown), else null. Optimised code has no dead stores, so a return reached with XMM0
// written and not read since (and not since the last call) returns that value; likewise RAX.
// The answer is a vector class when at least one return is of the first kind and none of the
// second. Paths through indirect jumps are not followed.
function returnsVector(entry, ranges) {
  init(); tables();
  const { FlowControl, OpAccess, OpKind } = iced;
  const bounds = ranges.map(([s, e]) => [parseInt(s, 16), parseInt(e, 16)]);
  const inside = (a) => bounds.some(([lo, hi]) => a >= lo && a < hi);
  const seen = new Set();
  const work = [[entry, 0, 0, '']]; // ip, XMM0 state, RAX state (0 untouched since entry/call, 1 written, 2 written then read), width
  let vec = 0, int = 0, width = '', budget = 6000;
  while (work.length && budget > 0) {
    let [ip, xs, as, w] = work.pop();
    for (;;) {
      const key = ip * 16 + xs * 4 + as;
      if (seen.has(key) || budget-- <= 0 || !inside(ip)) break;
      seen.add(key);
      const off = image.rvaToOff(ip - IMG_BASE);
      if (off < 0) break;
      const dec = new iced.Decoder(64, image.b.subarray(off, off + 16), iced.DecoderOptions.None);
      dec.ip = BigInt(ip);
      const ins = dec.decode();
      if (ins.isInvalid) { ins.free(); dec.free(); break; }
      const flow = ins.flowControl, len = ins.length, mn = ins.mnemonic;
      const direct = ins.op0Kind === OpKind.NearBranch64 ? Number(ins.nearBranchTarget) : null;
      if (flow === FlowControl.Next) {
        const info = factory.info(ins);
        const op0reg = ins.op0Kind === OpKind.Register ? ins.op0Register : -1;
        for (const u of info.usedRegisters()) {
          const b = bitOf(u.register), acc = u.access;
          u.free();
          if (b !== 4 && b !== 8) continue;
          const writes = acc === OpAccess.Write || acc === OpAccess.ReadWrite || (MERGE.has(mn) && b === 4 && bitOf(op0reg) === 4);
          const reads = acc === OpAccess.Read || acc === OpAccess.CondRead;
          if (b === 4) { if (writes) { xs = 1; const name = iced.Mnemonic[mn] || ''; w = DOUBLE.has(mn) ? 'd' : /s[sd]$/i.test(name) || /^V?Cvt/i.test(name) ? 'f' : acc === OpAccess.ReadWrite && w ? w : 'x'; } else if (reads && xs === 1) xs = 2; }
          else if (writes) as = 1; else if (reads && as === 1) as = 2;
        }
        info.free();
      }
      ins.free(); dec.free();
      if (flow === FlowControl.Next) { ip += len; continue; }
      if (flow === FlowControl.Return) { if (xs === 1) { vec++; if (!width || width === 'x') width = w; else if (w !== 'x' && w !== width) width = 'x'; } if (as === 1) int++; break; }
      if (flow === FlowControl.Call || flow === FlowControl.IndirectCall) { xs = 0; as = 0; w = ''; ip += len; continue; }
      if (flow === FlowControl.ConditionalBranch && direct !== null) { work.push([direct, xs, as, w]); ip += len; continue; }
      if (flow === FlowControl.UnconditionalBranch && direct !== null) { ip = direct; continue; }
      break;
    }
  }
  return vec && !int && budget > 0 ? width || 'x' : null;
}
// Whether the code in `ranges` contains an instruction that converts an integer to floating
// point (toFloat) or floating point to an integer (toInt). The decompiler prints a conversion and
// a reinterpretation of the same bits with the same cast; a function without the instruction
// cannot contain the conversion.
function conversions(ranges) {
  init();
  const out = { toFloat: false, toInt: false };
  for (const [s, e] of ranges) {
    const lo = parseInt(s, 16), hi = parseInt(e, 16);
    const off = image.rvaToOff(lo - IMG_BASE);
    if (off < 0) return { toFloat: true, toInt: true };
    const dec = new iced.Decoder(64, image.b.subarray(off, off + (hi - lo)), iced.DecoderOptions.None);
    dec.ip = BigInt(lo);
    const ins = new iced.Instruction();
    while (dec.canDecode) {
      dec.decodeOut(ins);
      const name = iced.Mnemonic[ins.mnemonic] || '';
      if (/^V?Cvt(?:u?si|u?dq|pi|u?qq)2[sp][sdh]$|^Fi(?:ld|add|sub|subr|mul|div|divr|com|comp)$/i.test(name)) out.toFloat = true;
      else if (/^V?Cvtt?[sp][sdh]2(?:u?si|u?dq|pi|u?qq)$|^Fistt?p?$/i.test(name)) out.toInt = true;
    }
    ins.free(); dec.free();
  }
  return out;
}
// Function entries known without the decompiler: Ghidra's function list and the roots of the
// exception table (the same sources tools/split.js reads), and the functions that do not return.
let entries = null, noReturn = null;
function knownEntries() {
  if (entries) return entries;
  init();
  const fs = require('fs'), path = require('path');
  const { ROOT } = require('./log');
  entries = new Set(); noReturn = new Set();
  const tsv = path.join(ROOT, 'build', 'ghidra', 'functions.tsv');
  if (fs.existsSync(tsv)) for (const l of fs.readFileSync(tsv, 'utf8').split('\n').slice(1)) { const c = l.split('\t'); if (!c[0]) continue; const a = parseInt(c[0], 16); entries.add(a); if (c[4] === '1') noReturn.add(a); }
  for (const e of require('./pe').runtimeFunctions(image)) if (e.root === e.begin) entries.add(IMG_BASE + e.begin);
  const rt = path.join(ROOT, 'build', 'ghidra', 'runtime_symbols.json');
  if (fs.existsSync(rt)) for (const x of JSON.parse(fs.readFileSync(rt, 'utf8')).noreturn || []) noReturn.add(Number(x));
  return entries;
}
// The ranges of a record extended by the code the decompiler followed out of them. The exported
// ranges are Ghidra's function body; the decompiler also follows a direct jump (jcc or jmp) that
// leaves the body when it does not regard it as a call: a jump to a fragment that is no function,
// or a conditional jump to another function, whose body it then prints inline. Such a target is
// recognised by not being among the record's callees (a jump the decompiler took for a tail call
// is listed there); a target that is a known function entry additionally needs a recorded call
// site inside the walked code, or no call in it at all. The code is walked by recursive descent
// until ret, int3, an indirect jump, a jump to a callee, or a call to a function that does not
// return; a jump table is not followed. Returns the merged, sorted ranges in the record's format,
// or null when nothing was added (or the walk left the image or grew past `limit` bytes).
function extendRanges(rec, limit = 0x4000) {
  init();
  const { FlowControl, OpKind } = iced;
  knownEntries();
  const bounds = rec.ranges.map(([s, e]) => [parseInt(s, 16), parseInt(e, 16)]);
  const inside = (a) => bounds.some(([lo, hi]) => a >= lo && a < hi);
  const callee = (a) => { const h = a.toString(16).toUpperCase(); return h in rec.callees || ('!' + h) in rec.callees; };
  const siteSet = new Set((rec.sites || []).map((s) => parseInt(s[0], 16)));
  // direct jumps that leave the exported ranges
  const starts = [];
  for (const [lo, hi] of bounds) {
    const off = image.rvaToOff(lo - IMG_BASE);
    if (off < 0) return null;
    const dec = new iced.Decoder(64, image.b.subarray(off, off + (hi - lo)), iced.DecoderOptions.None);
    dec.ip = BigInt(lo);
    const ins = new iced.Instruction();
    let last = -1;
    while (dec.canDecode) {
      dec.decodeOut(ins);
      const f = last = ins.flowControl;
      if ((f === FlowControl.UnconditionalBranch || f === FlowControl.ConditionalBranch) && ins.op0Kind === OpKind.NearBranch64) { const t = Number(ins.nearBranchTarget); if (!inside(t) && !callee(t)) starts.push(t); }
    }
    ins.free(); dec.free();
    // a range that ends in the middle of straight-line code: the body continues behind it
    if ((last === FlowControl.Next || last === FlowControl.ConditionalBranch) && !inside(hi)) starts.push(hi);
  }
  if (!starts.length) return null;
  const added = new Map(); // start of an instruction -> its length
  let bytes = 0;
  // walks one out-of-range target; returns false when the code cannot be taken (and adds nothing)
  const walkFrom = (start) => {
    const mine = new Map(), work = [start];
    let calls = 0, known = 0;
    while (work.length) {
      let ip = work.pop();
      for (;;) {
        if (inside(ip) || added.has(ip) || mine.has(ip)) break;
        const off = image.rvaToOff(ip - IMG_BASE);
        if (off < 0) return false;
        const dec = new iced.Decoder(64, image.b.subarray(off, off + 16), iced.DecoderOptions.None);
        dec.ip = BigInt(ip);
        const ins = dec.decode();
        const bad = ins.isInvalid, len = ins.length, f = ins.flowControl, int3 = ins.mnemonic === iced.Mnemonic.Int3;
        const direct = ins.op0Kind === OpKind.NearBranch64 ? Number(ins.nearBranchTarget) : null;
        ins.free(); dec.free();
        if (bad) return false;
        if (int3) break; // padding: the path ended before it
        mine.set(ip, len);
        if ((bytes += len) > limit) return false;
        if (f === FlowControl.Next) { ip += len; continue; }
        if (f === FlowControl.Call || f === FlowControl.IndirectCall) { calls++; if (siteSet.has(ip)) known++; if (direct !== null && noReturn.has(direct)) break; ip += len; continue; }
        if (f === FlowControl.ConditionalBranch && direct !== null) { if (!inside(direct) && !callee(direct)) work.push(direct); ip += len; continue; }
        if (f === FlowControl.UnconditionalBranch && direct !== null) { if (inside(direct) || callee(direct)) break; ip = direct; continue; }
        break; // ret, indirect jump, interrupt
      }
    }
    // a known function is taken only on evidence that its body was printed inline
    if (entries.has(start) && calls && !known) return false;
    for (const [a, l] of mine) added.set(a, l);
    return true;
  };
  let any = false;
  for (const t of starts) if (!added.has(t) && walkFrom(t)) any = true;
  if (!any || !added.size) return null;
  const all = bounds.map(([lo, hi]) => [lo, hi]).concat([...added].map(([a, l]) => [a, a + l])).sort((x, y) => x[0] - y[0]);
  const merged = [];
  for (const [lo, hi] of all) { const last = merged[merged.length - 1]; if (last && lo <= last[1]) { if (hi > last[1]) last[1] = hi; } else merged.push([lo, hi]); }
  return merged.map(([lo, hi]) => [lo.toString(16).toUpperCase(), hi.toString(16).toUpperCase()]);
}
module.exports = { entryClasses, returnClass, callSites, argClasses, returnsVector, conversions, extendRanges, knownEntries };
