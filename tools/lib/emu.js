// A small x64 emulator for straight-line start-up code (tools/reflection.js runs the game's
// reflection registration functions through it). It executes one function from its entry to its
// return on concrete values, reading the executable's own data, and does NOT enter callees: every
// call is recorded with its argument values and, unless a hook supplies a result, returns a fresh
// non-null token ("fake pointer") that later instructions carry around like any pointer. So for a
// call such as  UObjectProperty::ctor(mem, init, 0, 0x4730, flags, Z_Construct_UClass_X())  the
// record holds the literal offset and flags and a token that names the call which produced the
// last argument. Nothing is guessed: a value that is not known is `null`, and a conditional jump
// whose flags are not known falls through (counted in `unknown_branches`).
//   const emu = require('./lib/emu').create();
//   const r = emu.run(va, { hooks: Map<targetVa, (call, st) => BigInt|undefined> });
//   r.calls[i] = { site, target (thunks resolved; null when indirect), disp, a: [rcx, rdx, r8, r9],
//                  sq/sd: stack arguments 5.. read as 8/4 bytes, dq/dd: what a[i] points at, ret }
// Values are BigInt (unsigned 64-bit) or null.
const iced = require('iced-x86');
const pe = require('./pe');

const M64 = 0xFFFFFFFFFFFFFFFFn;
const MASK = { 1: 0xFFn, 2: 0xFFFFn, 4: 0xFFFFFFFFn, 8: M64 };
const SIGN = { 1: 0x80n, 2: 0x8000n, 4: 0x80000000n, 8: 0x8000000000000000n };
// Tokens live in an address range no constant of the program falls into, and always end in 0x0101 (non-zero as a
// byte, a word and a dword), so a flag constant such as 0x100000000000 is never mistaken for one.
const HEAP = 0x13F700000000n, HEAP_END = HEAP + (1n << 42n);
const STACK_TOP = 0x7F0000000000;
const ENTER = Symbol('enter'); // a hook returns this to have the callee executed instead of stubbed

function create(file) {
  const img = pe.load(file);
  const BASE = Number(img.imageBase);
  const b = img.b;
  const secs = img.sections.map((s) => ({ lo: BASE + s.rva, hi: BASE + s.rva + Math.max(s.vsize, s.rawSize), rawHi: BASE + s.rva + s.rawSize, raw: s.rawPtr, name: s.name, writable: !!(s.flags & 0x80000000) }));
  const IMG_LO = BASE, IMG_HI = Math.max(...secs.map((s) => s.hi));
  let lastSec = secs[0];
  const secOf = (va) => { if (va >= lastSec.lo && va < lastSec.hi) return lastSec; for (const s of secs) if (va >= s.lo && va < s.hi) return (lastSec = s); return null; };
  const imgByte = (va) => { const s = secOf(va); if (!s) return -1; return va < s.rawHi ? b[s.raw + (va - s.lo)] : 0; };
  const inImage = (va) => va >= IMG_LO && va < IMG_HI;
  const text = secs[0];
  const inText = (va) => va >= text.lo && va < text.rawHi;
  const u32 = (va) => { const s = secOf(va); return s && va + 4 <= s.rawHi ? b.readUInt32LE(s.raw + (va - s.lo)) : 0; };
  const u64 = (va) => { const s = secOf(va); return s && va + 8 <= s.rawHi ? b.readBigUInt64LE(s.raw + (va - s.lo)) : 0n; };
  // strings in the image
  function wstr(va, max = 1024) {
    const s = secOf(va); if (!s || va >= s.rawHi) return null;
    let o = s.raw + (va - s.lo); const st = o, end = s.raw + (s.rawHi - s.lo);
    while (o + 1 < end && o - st < max * 2 && (b[o] || b[o + 1])) o += 2;
    return b.toString('utf16le', st, o);
  }
  function astr(va, max = 1024) {
    const s = secOf(va); if (!s || va >= s.rawHi) return null;
    let o = s.raw + (va - s.lo); const st = o, end = s.raw + (s.rawHi - s.lo);
    while (o < end && o - st < max && b[o]) o++;
    return b.toString('latin1', st, o);
  }
  // a wide or narrow printable string, whichever the bytes are
  function str(va) {
    if (!inImage(va)) return null;
    const b0 = imgByte(va), b1 = imgByte(va + 1);
    if (b0 <= 0) return b0 === 0 && b1 === 0 ? '' : null;
    const s = b1 === 0 ? wstr(va) : astr(va);
    return s !== null && /^[\x20-\x7e]*$/.test(s) ? s : s;
  }
  // function extents from .pdata
  const rf = pe.runtimeFunctions(img).slice().sort((x, y) => x.begin - y.begin);
  const funcOf = (va) => {
    const rva = va - BASE; let lo = 0, hi = rf.length - 1;
    while (lo <= hi) { const m = (lo + hi) >> 1; if (rf[m].begin > rva) hi = m - 1; else if (rf[m].end <= rva) lo = m + 1; else return rf[m]; }
    return null;
  };
  const funcStart = (va) => { const e = funcOf(va); return e ? BASE + e.root : null; };
  // incremental-link thunks: jmp rel32
  const resolveThunk = (va) => {
    for (let i = 0; i < 8 && inText(va) && imgByte(va) === 0xE9; i++) va = va + 5 + (u32(va + 1) | 0);
    return va;
  };

  // ---- registers
  const R = iced.Register, REG = new Map();
  const n64 = ['RAX', 'RCX', 'RDX', 'RBX', 'RSP', 'RBP', 'RSI', 'RDI', 'R8', 'R9', 'R10', 'R11', 'R12', 'R13', 'R14', 'R15'];
  const n32 = ['EAX', 'ECX', 'EDX', 'EBX', 'ESP', 'EBP', 'ESI', 'EDI', 'R8D', 'R9D', 'R10D', 'R11D', 'R12D', 'R13D', 'R14D', 'R15D'];
  const n16 = ['AX', 'CX', 'DX', 'BX', 'SP', 'BP', 'SI', 'DI', 'R8W', 'R9W', 'R10W', 'R11W', 'R12W', 'R13W', 'R14W', 'R15W'];
  const n8 = ['AL', 'CL', 'DL', 'BL', 'SPL', 'BPL', 'SIL', 'DIL', 'R8L', 'R9L', 'R10L', 'R11L', 'R12L', 'R13L', 'R14L', 'R15L'];
  n64.forEach((n, i) => REG.set(R[n], { i, size: 8 })); n32.forEach((n, i) => REG.set(R[n], { i, size: 4 }));
  n16.forEach((n, i) => REG.set(R[n], { i, size: 2 })); n8.forEach((n, i) => REG.set(R[n], { i, size: 1 }));
  ['AH', 'CH', 'DH', 'BH'].forEach((n, i) => REG.set(R[n], { i, size: 1, hi: true }));
  const XMM = new Map();
  for (let i = 0; i < 16; i++) for (const p of ['XMM', 'YMM', 'ZMM']) if (R[p + i] !== undefined) XMM.set(R[p + i], i);
  const RAX = 0, RCX = 1, RDX = 2, RSP = 4, RSI = 6, RDI = 7;
  const Mn = iced.Mnemonic, OK = iced.OpKind, FC = iced.FlowControl, CC = iced.ConditionCode;
  const mnName = new Map(Object.keys(Mn).filter((k) => isNaN(k)).map((k) => [Mn[k], k]));
  const factory = new iced.InstructionInfoFactory();
  const dec = new iced.Decoder(64, b.subarray(text.raw, text.raw + (text.rawHi - text.lo)), iced.DecoderOptions.None);
  const ins = new iced.Instruction();

  // ---- tokens: results of calls. Records live for the life of the emulator.
  const recs = [];
  const fresh = (rec) => { recs.push(rec); return HEAP + BigInt(recs.length - 1) * 0x10000n + 0x101n; };
  const recOf = (v) => (typeof v === 'bigint' && v >= HEAP && v < HEAP_END && (v & 0xFFFFn) === 0x101n ? recs[Number((v - HEAP) >> 16n)] || null : null);
  const recAt = (v) => (typeof v === 'bigint' && v >= HEAP && v < HEAP_END ? recs[Number((v - HEAP) >> 16n)] || null : null); // the token an address lies in
  const isStack = (v) => typeof v === 'bigint' && v > BigInt(STACK_TOP - 0x4000000) && v < BigInt(STACK_TOP + 0x10000);

  function run(start, opts = {}) {
    const hooks = opts.hooks || new Map();
    const budget = opts.budget || 4000000;
    const regs = new Array(16).fill(null);
    const xmm = Array.from({ length: 16 }, () => [null, null]);
    const mem = new Map();
    regs[RSP] = BigInt(STACK_TOP - 0x1000);
    if (opts.regs) for (const [k, v] of Object.entries(opts.regs)) regs[Number(k)] = v;
    const dataUnknown = !!opts.dataUnknown; // writable sections hold run-time state: their file contents are not the values a running program sees
    const zlo = opts.zero ? Number(opts.zero[0]) : null, zhi = opts.zero ? Number(opts.zero[1]) : null, onWrite = opts.onWrite || null;
    let myFn = funcOf(start), myRoot = myFn ? myFn.root : -1, myStart = start, curIp = start;
    const frames = []; // callers of the function being executed (entered calls)
    const res = { start, calls: [], ret: null, steps: 0, unknown_branches: 0, unknown_branch_sites: [], entered: 0, max_depth: 0, end: null };

    const rd = (addr, size) => {
      if (addr === null || addr > 0x1FFFFFFFFFFFFFn) return null;
      const a = Number(addr);
      let lo = 0, hi = 0;
      for (let k = 0; k < size; k++) {
        let v = mem.get(a + k);
        if (v === undefined) v = zlo !== null && a + k >= zlo && a + k < zhi ? 0 : dataUnknown && inImage(a + k) && secOf(a + k) && secOf(a + k).writable ? -1 : imgByte(a + k);
        if (v < 0) return null;
        if (k < 4) lo += v * 2 ** (8 * k); else hi += v * 2 ** (8 * (k - 4));
      }
      return size > 4 ? (BigInt(hi) << 32n) | BigInt(lo) : BigInt(lo);
    };
    const wr = (addr, size, val) => {
      if (addr === null || addr > 0x1FFFFFFFFFFFFFn) return;
      const a = Number(addr);
      if (val === null) { for (let k = 0; k < size; k++) { mem.set(a + k, -1); if (onWrite) onWrite(a + k, -1, curIp); } return; }
      for (let k = 0; k < size; k++) { const v = Number((val >> BigInt(8 * k)) & 0xFFn); mem.set(a + k, v); if (onWrite) onWrite(a + k, v, curIp); }
    };
    const rreg = (r) => {
      const d = REG.get(r); if (!d) return null;
      const v = regs[d.i]; if (v === null) return null;
      return d.hi ? (v >> 8n) & 0xFFn : v & MASK[d.size];
    };
    const wreg = (r, val) => {
      const d = REG.get(r); if (!d) { const x = XMM.get(r); if (x !== undefined) xmm[x] = [null, null]; return; }
      if (d.size >= 4) { regs[d.i] = val === null ? null : val & MASK[d.size]; return; }
      const old = regs[d.i];
      if (old === null || val === null) { regs[d.i] = null; return; }
      if (d.hi) regs[d.i] = (old & ~0xFF00n) | ((val & 0xFFn) << 8n);
      else regs[d.i] = (old & ~MASK[d.size]) | (val & MASK[d.size]);
    };
    const ea = () => {
      if (ins.isIpRelMemoryOperand) return ins.ipRelMemoryAddress;
      const seg = ins.segmentPrefix;
      if (seg === R.GS || seg === R.FS) return null;
      let a = BigInt.asUintN(64, ins.memoryDisplacement);
      const mb = ins.memoryBase, mi = ins.memoryIndex;
      if (mb !== R.None) { const v = rreg(mb); if (v === null) return null; a += v; }
      if (mi !== R.None) { const v = rreg(mi); if (v === null) return null; a += v * BigInt(ins.memoryIndexScale); }
      return a & M64;
    };
    const memSize = () => iced.MemorySizeExt.size(ins.memorySize);
    const opSize = (n) => { const k = ins.opKind(n); if (k === OK.Register) { const d = REG.get(ins.opRegister(n)); return d ? d.size : 0; } if (k === OK.Memory) return memSize(); return 0; };
    const rop = (n, size) => {
      const k = ins.opKind(n);
      if (k === OK.Register) return rreg(ins.opRegister(n));
      if (k === OK.Memory) return rd(ea(), memSize());
      if (k >= OK.Immediate8 && k <= OK.Immediate32to64) return BigInt.asUintN(64, ins.immediate(n)) & MASK[size || 8];
      return null;
    };
    const wop = (n, val, size) => {
      const k = ins.opKind(n);
      if (k === OK.Register) wreg(ins.opRegister(n), val);
      else if (k === OK.Memory) wr(ea(), size || memSize(), val);
    };
    const sext = (v, from) => (v === null ? null : (v & SIGN[from]) ? (v | (M64 ^ MASK[from])) & M64 : v);
    let fl = null; // { t: 'sub'|'add'|'logic'|'res', a, b, r, size }
    const cond = (cc) => {
      if (!fl) return null;
      const s = fl.size, m = MASK[s], sg = SIGN[s];
      let zf = null, cf = null, sf = null, of = null;
      if (fl.t === 'sub') { if (fl.a === null || fl.b === null) return null; const r = (fl.a - fl.b) & m; zf = r === 0n; cf = fl.a < fl.b; sf = !!(r & sg); of = !!((fl.a ^ fl.b) & (fl.a ^ r) & sg); }
      else if (fl.t === 'add') { if (fl.a === null || fl.b === null) return null; const r = (fl.a + fl.b) & m; zf = r === 0n; cf = r < fl.a; sf = !!(r & sg); of = !!(~(fl.a ^ fl.b) & (fl.a ^ r) & sg); }
      else { if (fl.r === null) return null; zf = fl.r === 0n; sf = !!(fl.r & sg); if (fl.t === 'logic') { cf = false; of = false; } }
      switch (cc) {
        case CC.o: return of; case CC.no: return of === null ? null : !of;
        case CC.b: return cf; case CC.ae: return cf === null ? null : !cf;
        case CC.e: return zf; case CC.ne: return !zf;
        case CC.be: return cf === null ? null : cf || zf; case CC.a: return cf === null ? null : !cf && !zf;
        case CC.s: return sf; case CC.ns: return !sf;
        case CC.l: return of === null ? null : sf !== of; case CC.ge: return of === null ? null : sf === of;
        case CC.le: return of === null ? null : zf || sf !== of; case CC.g: return of === null ? null : !zf && sf === of;
        default: return null;
      }
    };
    const generic = () => { // an instruction that is not modelled: whatever it writes becomes unknown
      const info = factory.info(ins);
      for (const u of info.usedRegisters()) { const acc = u.access, r = u.register; u.free(); if (acc !== iced.OpAccess.Read && acc !== iced.OpAccess.CondRead && acc !== iced.OpAccess.NoMemAccess && acc !== iced.OpAccess.None) { const d = REG.get(r); if (d) { if (d.i !== RSP) regs[d.i] = null; } else wreg(r, null); } }
      for (const u of info.usedMemory()) { const acc = u.access; u.free(); if (acc === iced.OpAccess.Write || acc === iced.OpAccess.ReadWrite || acc === iced.OpAccess.CondWrite || acc === iced.OpAccess.ReadCondWrite) { if (ins.op0Kind === OK.Memory) wr(ea(), memSize() || 8, null); } }
      info.free();
      if (ins.rflagsModified) fl = null;
    };
    const doCall = (target, site, extra) => {
      const rsp = regs[RSP];
      const call = { site, target, a: [regs[RCX], regs[RDX], regs[8], regs[9]], sq: [], sd: [], sb: [], dq: [], dd: [], ret: null, ...extra };
      for (let k = 0; k < 10; k++) { const a = rsp + BigInt(0x20 + 8 * k); call.sq.push(rd(a, 8)); call.sd.push(rd(a, 4)); call.sb.push(rd(a, 1)); }
      for (let k = 0; k < 4; k++) { const v = call.a[k]; if (isStack(v) || recOf(v)) { call.dq.push(rd(v, 8)); call.dd.push(rd(v, 4)); } else { call.dq.push(null); call.dd.push(null); } }
      const h = target !== null ? hooks.get(target) : hooks.get('indirect');
      call.depth = frames.length;
      let r = h ? h(call, st) : undefined;
      if (r === undefined && hooks.has('*')) r = hooks.get('*')(call, st);
      if (r === ENTER && target !== null) { call.entered = true; res.calls.push(call); res.entered++; return ENTER; }
      if (r === undefined || r === ENTER) r = fresh({ call: target, site });
      call.ret = r;
      res.calls.push(call);
      jumped = true; // a hook may have run another emulation on the shared decoder
      regs[RAX] = r; regs[RCX] = regs[RDX] = regs[8] = regs[9] = regs[10] = regs[11] = null;
      for (let i = 0; i < 6; i++) xmm[i] = [null, null];
      fl = null;
    };
    const st = { rd, wr, regs, fresh, recOf, isStack, calls: res.calls, depth: () => frames.length };
    const enter = (target, retTo) => { // push the return address and continue in the callee; retTo === null: a tail call, the frame is replaced
      if (retTo !== null) { regs[RSP] -= 8n; wr(regs[RSP], 8, BigInt(retTo)); frames.push({ myFn, myRoot, myStart }); if (frames.length > res.max_depth) res.max_depth = frames.length; }
      myFn = funcOf(target); myRoot = myFn ? myFn.root : -1; myStart = target;
    };
    const leave = () => { // return to the caller of an entered function; false when the outermost function returns
      if (!frames.length) return false;
      const ra = rd(regs[RSP], 8); regs[RSP] += 8n;
      const f = frames.pop(); myFn = f.myFn; myRoot = f.myRoot; myStart = f.myStart;
      if (ra === null || !inText(Number(ra))) { res.end = 'bad-return'; return null; }
      return Number(ra);
    };
    let ip = start, jumped = true;
    if (opts.init) opts.init(st);
    for (;;) {
      if (res.steps++ >= budget) { res.end = 'budget'; break; }
      curIp = ip;
      if (!inText(ip)) { res.end = 'bad'; break; }
      if (jumped) { dec.position = ip - text.lo; dec.ip = BigInt(ip); jumped = false; }
      dec.decodeOut(ins);
      if (ins.isInvalid) { res.end = 'bad'; break; }
      const next = ip + ins.length;
      const flow = ins.flowControl;
      if (flow !== FC.Next) {
        if (flow === FC.Return) {
          const back = leave();
          if (back === false) { res.end = 'ret'; res.ret = regs[RAX]; break; }
          if (back === null) break;
          ip = back; jumped = true; continue;
        }
        if (flow === FC.Call) {
          const t = resolveThunk(Number(ins.nearBranchTarget));
          if (doCall(t, ip) === ENTER) { enter(t, next); ip = t; jumped = true; } else ip = next;
          continue;
        }
        if (flow === FC.IndirectCall) {
          let t = null, disp = null;
          if (ins.op0Kind === OK.Memory) { const v = rd(ea(), 8); if (v !== null && inText(Number(v))) t = resolveThunk(Number(v)); disp = ins.isIpRelMemoryOperand ? null : Number(BigInt.asIntN(64, ins.memoryDisplacement)); }
          else if (ins.op0Kind === OK.Register) { const v = rreg(ins.op0Register); if (v !== null && v < 0x200000000n && inText(Number(v))) t = resolveThunk(Number(v)); }
          if (doCall(t, ip, { indirect: true, disp, mem: ins.op0Kind === OK.Memory && ins.isIpRelMemoryOperand ? Number(ins.ipRelMemoryAddress) : null }) === ENTER) { enter(t, next); ip = t; jumped = true; } else ip = next;
          continue;
        }
        if (flow === FC.UnconditionalBranch || flow === FC.IndirectBranch) {
          if (ins.op0Kind !== OK.NearBranch64) { // jmp [mem] / jmp reg: a tail call or a jump table
            let t = null;
            if (ins.op0Kind === OK.Memory) { const v = rd(ea(), 8); if (v !== null && inText(Number(v))) t = resolveThunk(Number(v)); }
            else if (ins.op0Kind === OK.Register) { const v = rreg(ins.op0Register); if (v !== null && v < 0x200000000n && inText(Number(v))) t = resolveThunk(Number(v)); }
            if (doCall(t, ip, { indirect: true, tail: true }) === ENTER) { enter(t, null); ip = t; jumped = true; continue; }
            const back = leave();
            if (back === false) { res.end = 'tail'; res.ret = regs[RAX]; break; }
            if (back === null) break;
            ip = back; jumped = true; continue;
          }
          const t = Number(ins.nearBranchTarget);
          const tf = funcOf(t);
          const outside = myFn ? (!tf || tf.root !== myRoot || t === BASE + tf.root) : (tf !== null || Math.abs(t - myStart) > 0x400);
          if (outside && t !== myStart) {
            const rt = resolveThunk(t);
            if (doCall(rt, ip, { tail: true }) === ENTER) { enter(rt, null); ip = rt; jumped = true; continue; }
            const back = leave();
            if (back === false) { res.end = 'tail'; res.ret = regs[RAX]; break; }
            if (back === null) break;
            ip = back; jumped = true; continue;
          }
          ip = t; jumped = true; continue;
        }
        if (flow === FC.ConditionalBranch) {
          let c = null;
          if (ins.isJccShortOrNear) c = cond(ins.conditionCode);
          if (c === null) { // not decidable here: fall through, unless the caller explores both ways (opts.decide)
            if (opts.decide) c = !!opts.decide(ip, res.unknown_branches);
            res.unknown_branches++; if (res.unknown_branch_sites.length < 32) res.unknown_branch_sites.push(ip);
          }
          if (c === true) { ip = Number(ins.nearBranchTarget); jumped = true; } else ip = next;
          continue;
        }
        res.end = flow === FC.Interrupt || flow === FC.Exception ? 'trap' : 'bad'; res.end_ip = ip; break; // int3 / ud2 (the code after a call that does not return), far transfers
      }
      const name = mnName.get(ins.mnemonic);
      switch (name) {
        case 'Mov': { const s = opSize(0) || opSize(1); wop(0, rop(1, s), s); break; }
        case 'Movzx': wop(0, rop(1)); break;
        case 'Movsx': case 'Movsxd': { const from = opSize(1); wop(0, sext(rop(1), from)); break; }
        case 'Lea': wop(0, ea()); break;
        case 'Add': case 'Sub': case 'And': case 'Or': case 'Xor': case 'Cmp': case 'Test': {
          const s = opSize(0) || opSize(1), m = MASK[s];
          let a = rop(0, s), c = rop(1, s), r = null;
          const same = ins.op0Kind === OK.Register && ins.op1Kind === OK.Register && ins.op0Register === ins.op1Register;
          if (same && (name === 'Xor' || name === 'Sub')) { a = c = 0n; }
          if (a !== null && c !== null) r = (name === 'Add' ? a + c : name === 'Sub' || name === 'Cmp' ? a - c : name === 'Or' ? a | c : name === 'Xor' ? a ^ c : a & c) & m;
          else if (name === 'And' && (a === 0n || c === 0n)) r = 0n;
          fl = name === 'Add' ? { t: 'add', a, b: c, size: s } : name === 'Sub' || name === 'Cmp' ? { t: 'sub', a, b: c, size: s } : { t: 'logic', r, size: s };
          if (name !== 'Cmp' && name !== 'Test') wop(0, r, s);
          break;
        }
        case 'Inc': case 'Dec': case 'Neg': case 'Not': {
          const s = opSize(0), a = rop(0, s);
          const r = a === null ? null : (name === 'Inc' ? a + 1n : name === 'Dec' ? a - 1n : name === 'Neg' ? -a : ~a) & MASK[s];
          wop(0, r, s); if (name !== 'Not') fl = { t: 'res', r, size: s };
          break;
        }
        case 'Shl': case 'Sal': case 'Shr': case 'Sar': {
          const s = opSize(0), a = rop(0, s), c = rop(1, 1);
          let r = null;
          if (a !== null && c !== null) { const n = c & (s === 8 ? 63n : 31n); r = name === 'Shr' ? a >> n : name === 'Sar' ? (BigInt.asIntN(s * 8, a) >> n) & MASK[s] : (a << n) & MASK[s]; }
          wop(0, r, s); fl = { t: 'res', r, size: s };
          break;
        }
        case 'Bts': case 'Btr': case 'Btc': {
          if (ins.op0Kind !== OK.Register) { generic(); break; }
          const s = opSize(0), a = rop(0, s), c = rop(1, 1);
          let r = null;
          if (a !== null && c !== null) { const bit = 1n << (c & BigInt(s * 8 - 1)); r = name === 'Bts' ? a | bit : name === 'Btr' ? a & ~bit & MASK[s] : a ^ bit; }
          wop(0, r, s); fl = null;
          break;
        }
        case 'Imul': {
          if (ins.opCount === 1) { generic(); break; }
          const s = opSize(0), a = ins.opCount === 3 ? rop(1, s) : rop(0, s), c = ins.opCount === 3 ? rop(2, s) : rop(1, s);
          wop(0, a === null || c === null ? null : (BigInt.asIntN(s * 8, a) * BigInt.asIntN(s * 8, c)) & MASK[s], s); fl = null;
          break;
        }
        case 'Push': { const v = rop(0, 8); regs[RSP] -= 8n; wr(regs[RSP], 8, v); break; }
        case 'Pop': { const v = rd(regs[RSP], 8); regs[RSP] += 8n; wop(0, v, 8); break; }
        case 'Cdqe': regs[RAX] = regs[RAX] === null ? null : sext(regs[RAX] & MASK[4], 4); break;
        case 'Cwde': regs[RAX] = regs[RAX] === null ? null : sext(regs[RAX] & MASK[2], 2) & MASK[4]; break;
        case 'Cdq': regs[RDX] = regs[RAX] === null ? null : (regs[RAX] & SIGN[4]) ? MASK[4] : 0n; break;
        case 'Cqo': regs[RDX] = regs[RAX] === null ? null : (regs[RAX] & SIGN[8]) ? M64 : 0n; break;
        case 'Xchg': { const s = opSize(0) || opSize(1); const a = rop(0, s), c = rop(1, s); wop(0, c, s); wop(1, a, s); break; }
        case 'Nop': case 'Pause': break;
        case 'Movups': case 'Movaps': case 'Movdqa': case 'Movdqu': case 'Movupd': case 'Movapd': {
          const k0 = ins.op0Kind, k1 = ins.op1Kind;
          let v = [null, null];
          if (k1 === OK.Register) { const x = XMM.get(ins.op1Register); if (x !== undefined) v = xmm[x]; } else if (k1 === OK.Memory) { const a = ea(); v = a === null ? v : [rd(a, 8), rd(a + 8n, 8)]; }
          if (k0 === OK.Register) { const x = XMM.get(ins.op0Register); if (x !== undefined) xmm[x] = [v[0], v[1]]; } else if (k0 === OK.Memory) { const a = ea(); if (a !== null) { wr(a, 8, v[0]); wr(a + 8n, 8, v[1]); } }
          break;
        }
        case 'Movq': case 'Movd': case 'Movss': case 'Movsd': {
          if (ins.op0Kind !== OK.Register && ins.op0Kind !== OK.Memory) { // string movsd
            strOp('Movs', 4); break;
          }
          const s = name === 'Movq' || name === 'Movsd' ? 8 : 4;
          const k0 = ins.op0Kind, k1 = ins.op1Kind;
          let v = null;
          if (k1 === OK.Register) { const x = XMM.get(ins.op1Register); v = x !== undefined ? (xmm[x][0] === null ? null : xmm[x][0] & MASK[s]) : rreg(ins.op1Register); } else if (k1 === OK.Memory) v = rd(ea(), s);
          if (k0 === OK.Register) { const x = XMM.get(ins.op0Register); if (x !== undefined) xmm[x] = [v, k1 === OK.Register && XMM.has(ins.op1Register) && (name === 'Movss' || name === 'Movsd') ? xmm[x][1] : 0n]; else wreg(ins.op0Register, v); } else wr(ea(), s, v);
          break;
        }
        case 'Xorps': case 'Pxor': case 'Xorpd': {
          if (ins.op0Kind === OK.Register && ins.op1Kind === OK.Register && ins.op0Register === ins.op1Register) { const x = XMM.get(ins.op0Register); if (x !== undefined) xmm[x] = [0n, 0n]; } else generic();
          break;
        }
        case 'Movsb': strOp('Movs', 1); break; case 'Movsw': strOp('Movs', 2); break; case 'Movsq': strOp('Movs', 8); break;
        case 'Stosb': strOp('Stos', 1); break; case 'Stosw': strOp('Stos', 2); break; case 'Stosd': strOp('Stos', 4); break; case 'Stosq': strOp('Stos', 8); break;
        default:
          if (/^Set/.test(name) && ins.opCount === 1 && ins.conditionCode !== CC.None) { const c = cond(ins.conditionCode); wop(0, c === null ? null : c ? 1n : 0n, 1); }
          else if (/^Cmov/.test(name) && ins.conditionCode !== CC.None) { const s = opSize(0), c = cond(ins.conditionCode); if (c === null) wop(0, null, s); else wop(0, c ? rop(1, s) : rop(0, s), s); }
          else generic();
      }
      ip = next;
    }
    function strOp(kind, size) {
      const rep = ins.hasRepPrefix || ins.hasRepePrefix;
      const n = rep ? regs[RCX] : 1n;
      const di = regs[RDI], si = regs[RSI];
      if (n === null || n > 0x10000n || di === null || (kind === 'Movs' && si === null)) { if (rep) regs[RCX] = 0n; regs[RDI] = null; if (kind === 'Movs') regs[RSI] = null; return; }
      const sz = BigInt(size);
      for (let k = 0n; k < n; k++) wr(di + k * sz, size, kind === 'Movs' ? rd(si + k * sz, size) : regs[RAX] === null ? null : regs[RAX] & MASK[size]);
      regs[RDI] = di + n * sz; if (kind === 'Movs') regs[RSI] = si + n * sz; if (rep) regs[RCX] = 0n;
    }
    res.mem = { rd };
    return res;
  }
  return { img, BASE, run, fresh, recOf, recAt, isStack, resolveThunk, funcOf, funcStart, inImage, inText, imgByte, u32, u64, wstr, astr, str, secs, runtimeFunctions: rf };
}

module.exports = { create, HEAP, ENTER };
