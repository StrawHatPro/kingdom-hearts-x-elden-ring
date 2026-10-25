// Kismet (Blueprint VM) bytecode, UE 4.17 opcode set: serialized bytes <-> expression tree.
// Docs: docs/formats/kismet.md. The serialized form differs from the in-memory script only in
// the width of object pointers (int32 package index on disk, 8 bytes in memory) and names
// (8 bytes on disk, 12 in memory); jump targets are in-memory offsets, so each expression
// records its in-memory offset as "at" (derived on read; ignored on write).
const { T } = require('./package');

// operand kinds: p = object pointer (package index), n = FName, e = expression,
// E:<end op> = expressions up to and including the end marker,
// u8/u16/i32/u32/i64/u64/f32, s = ANSI string, S = UTF-16 string
const OPS = {
  0x00: ['LocalVariable', 'property:p'],
  0x01: ['InstanceVariable', 'property:p'],
  0x02: ['DefaultVariable', 'property:p'],
  0x04: ['Return', 'value:e'],
  0x06: ['Jump', 'target:u32'],
  0x07: ['JumpIfNot', 'target:u32', 'condition:e'],
  0x09: ['Assert', 'line:u16', 'debugOnly:u8', 'condition:e'],
  0x0b: ['Nothing'],
  0x0f: ['Let', 'property:p', 'variable:e', 'value:e'],
  0x12: ['ClassContext', 'object:e', 'skip:u32', 'rvalueProperty:p', 'context:e'],
  0x13: ['MetaCast', 'class:p', 'value:e'],
  0x14: ['LetBool', 'variable:e', 'value:e'],
  0x15: ['EndParmValue'],
  0x16: ['EndFunctionParms'],
  0x17: ['Self'],
  0x18: ['Skip', 'skip:u32', 'value:e'],
  0x19: ['Context', 'object:e', 'skip:u32', 'rvalueProperty:p', 'context:e'],
  0x1a: ['Context_FailSilent', 'object:e', 'skip:u32', 'rvalueProperty:p', 'context:e'],
  0x1b: ['VirtualFunction', 'function:n', 'params:E:EndFunctionParms'],
  0x1c: ['FinalFunction', 'function:p', 'params:E:EndFunctionParms'],
  0x1d: ['IntConst', 'value:i32'],
  0x1e: ['FloatConst', 'value:f32'],
  0x1f: ['StringConst', 'value:s'],
  0x20: ['ObjectConst', 'object:p'],
  0x21: ['NameConst', 'value:n'],
  0x22: ['RotationConst', 'pitch:f32', 'yaw:f32', 'roll:f32'],
  0x23: ['VectorConst', 'x:f32', 'y:f32', 'z:f32'],
  0x24: ['ByteConst', 'value:u8'],
  0x25: ['IntZero'],
  0x26: ['IntOne'],
  0x27: ['True'],
  0x28: ['False'],
  0x29: ['TextConst', 'text:text'],
  0x2a: ['NoObject'],
  0x2b: ['TransformConst', 'rx:f32', 'ry:f32', 'rz:f32', 'rw:f32', 'tx:f32', 'ty:f32', 'tz:f32', 'sx:f32', 'sy:f32', 'sz:f32'],
  0x2c: ['IntConstByte', 'value:u8'],
  0x2d: ['NoInterface'],
  0x2e: ['DynamicCast', 'class:p', 'value:e'],
  0x2f: ['StructConst', 'struct:p', 'serializedSize:i32', 'members:E:EndStructConst'],
  0x30: ['EndStructConst'],
  0x31: ['SetArray', 'array:e', 'elements:E:EndArray'],
  0x32: ['EndArray'],
  0x34: ['UnicodeStringConst', 'value:S'],
  0x35: ['Int64Const', 'value:i64'],
  0x36: ['UInt64Const', 'value:u64'],
  0x38: ['PrimitiveCast', 'castType:u8', 'value:e'],
  0x39: ['SetSet', 'set:e', 'count:i32', 'elements:E:EndSet'],
  0x3a: ['EndSet'],
  0x3b: ['SetMap', 'map:e', 'count:i32', 'elements:E:EndMap'],
  0x3c: ['EndMap'],
  0x3d: ['SetConst', 'innerProperty:p', 'count:i32', 'elements:E:EndSetConst'],
  0x3e: ['EndSetConst'],
  0x3f: ['MapConst', 'keyProperty:p', 'valueProperty:p', 'count:i32', 'elements:E:EndMapConst'],
  0x40: ['EndMapConst'],
  0x42: ['StructMemberContext', 'property:p', 'struct:e'],
  0x43: ['LetMulticastDelegate', 'variable:e', 'value:e'],
  0x44: ['LetDelegate', 'variable:e', 'value:e'],
  0x48: ['LocalOutVariable', 'property:p'],
  0x4a: ['DeprecatedOp4A'],
  0x4b: ['InstanceDelegate', 'function:n'],
  0x4c: ['PushExecutionFlow', 'target:u32'],
  0x4d: ['PopExecutionFlow'],
  0x4e: ['ComputedJump', 'target:e'],
  0x4f: ['PopExecutionFlowIfNot', 'condition:e'],
  0x50: ['Breakpoint'],
  0x51: ['InterfaceContext', 'value:e'],
  0x52: ['ObjToInterfaceCast', 'class:p', 'value:e'],
  0x53: ['EndOfScript'],
  0x54: ['CrossInterfaceCast', 'class:p', 'value:e'],
  0x55: ['InterfaceToObjCast', 'class:p', 'value:e'],
  0x5a: ['WireTracepoint'],
  0x5b: ['SkipOffsetConst', 'value:u32'],
  0x5c: ['AddMulticastDelegate', 'delegate:e', 'toAdd:e'],
  0x5d: ['ClearMulticastDelegate', 'delegate:e'],
  0x5e: ['Tracepoint'],
  0x5f: ['LetObj', 'variable:e', 'value:e'],
  0x60: ['LetWeakObjPtr', 'variable:e', 'value:e'],
  0x61: ['BindDelegate', 'function:n', 'delegate:e', 'object:e'],
  0x62: ['RemoveMulticastDelegate', 'delegate:e', 'toRemove:e'],
  0x63: ['CallMulticastDelegate', 'signature:p', 'params:E:EndFunctionParms'],
  0x64: ['LetValueOnPersistentFrame', 'property:p', 'value:e'],
  0x65: ['ArrayConst', 'innerProperty:p', 'count:i32', 'elements:E:EndArrayConst'],
  0x66: ['EndArrayConst'],
  0x67: ['AssetConst', 'value:e'],
  0x68: ['CallMath', 'function:p', 'params:E:EndFunctionParms'],
  0x69: ['SwitchValue', 'switch:switch'],
  0x6a: ['InstrumentationEvent', 'event:instr'],
  0x6b: ['ArrayGetByRef', 'array:e', 'index:e'],
  // Not a stock UE 4.17 opcode. GNatives[0xFE] (table at 149C6E260, stored by the initializer at
  // 1402D1C0B) is func_142F60EB0: it reads a name from the script, turns it into a string, finds
  // the UFunction object of that path with no outer (func_142F797B0 with UFunction::StaticClass,
  // func_142E54480) and calls it through func_142F4AEB0, the routine the VirtualFunction handler
  // (GNatives[0x1B] = func_142F62AB0) also ends in. The name given here describes that behaviour.
  0xfe: ['CallFunctionByPath_0xFE', 'function:n', 'params:E:EndFunctionParms'],
};
const BY_NAME = {};
for (const [code, [name, ...ops]] of Object.entries(OPS)) BY_NAME[name] = { code: +code, ops: ops.map((o) => { const [k, t, end] = o.split(':'); return { k, t, end }; }) };
for (const code of Object.keys(OPS)) OPS[code] = BY_NAME[OPS[code][0]];
for (const [name, v] of Object.entries(BY_NAME)) v.name = name;

// st: { mem } running in-memory offset
function readExpr(r, c, st) {
  const at = st.mem;
  const code = r.u8();
  const def = OPS[code];
  if (!def) throw new Error(`unknown Kismet opcode 0x${code.toString(16)} at disk offset ${r.p - 1}`);
  st.mem += 1;
  const x = { op: def.name, at };
  for (const o of def.ops) x[o.k] = readOperand(r, c, st, o);
  return x;
}
function readOperand(r, c, st, o) {
  switch (o.t) {
    case 'p': st.mem += 8; return T.obj.r(r, c);
    case 'n': st.mem += 12; return T.name.r(r, c);
    case 'e': return readExpr(r, c, st);
    case 'E': { const a = []; for (;;) { const e = readExpr(r, c, st); a.push(e); if (e.op === o.end) return a; } }
    case 'u8': st.mem += 1; return r.u8();
    case 'u16': st.mem += 2; return r.u16();
    case 'i32': st.mem += 4; return r.i32();
    case 'u32': st.mem += 4; return r.u32();
    case 'i64': st.mem += 8; return T.i64.r(r, c);
    case 'u64': st.mem += 8; return T.u64.r(r, c);
    case 'f32': st.mem += 4; return T.f32.r(r, c);
    case 's': { let e = r.p; while (r.b[e] !== 0) { if (++e >= r.b.length) throw new Error('unterminated string'); } const s = r.b.toString('latin1', r.p, e); st.mem += e - r.p + 1; r.p = e + 1; return s; }
    case 'S': {
      let e = r.p; for (;;) { if (e + 2 > r.b.length) throw new Error('unterminated string'); if (r.b.readUInt16LE(e) === 0) break; e += 2; }
      const raw = r.b.subarray(r.p, e); st.mem += e - r.p + 2; r.p = e + 2;
      const s = raw.toString('utf16le');
      return Buffer.from(s, 'utf16le').equals(raw) ? s : { hex16: raw.toString('hex') };
    }
    case 'text': {
      const t = { literalType: r.u8() }; st.mem += 1;
      if (t.literalType === 0) return t; // Empty
      if (t.literalType === 1) { t.source = readExpr(r, c, st); t.key = readExpr(r, c, st); t.namespace = readExpr(r, c, st); return t; } // LocalizedText
      if (t.literalType === 2 || t.literalType === 3) { t.source = readExpr(r, c, st); return t; } // InvariantText, LiteralString
      if (t.literalType === 4) { st.mem += 8; t.tableAsset = T.obj.r(r, c); t.tableId = readExpr(r, c, st); t.key = readExpr(r, c, st); return t; } // StringTableEntry
      throw new Error('text literal type ' + t.literalType);
    }
    case 'switch': {
      const n = r.u16(); st.mem += 2;
      const s = { endOffset: r.u32() }; st.mem += 4;
      s.index = readExpr(r, c, st);
      s.cases = [];
      for (let i = 0; i < n; i++) { const k = { value: readExpr(r, c, st) }; k.nextOffset = r.u32(); st.mem += 4; k.result = readExpr(r, c, st); s.cases.push(k); }
      s.default = readExpr(r, c, st);
      return s;
    }
    case 'instr': {
      const ev = { type: r.u8() }; st.mem += 1;
      if (ev.type === 4) { st.mem += 12; ev.name = T.name.r(r, c); } // EScriptInstrumentation::InlineEvent
      return ev;
    }
    default: throw new Error('operand kind ' + o.t);
  }
}
function writeExpr(w, c, x) {
  const def = BY_NAME[x.op];
  if (!def) throw new Error('unknown Kismet op ' + x.op);
  w.u8(def.code);
  for (const o of def.ops) writeOperand(w, c, o, x[o.k]);
}
function writeOperand(w, c, o, v) {
  switch (o.t) {
    case 'p': T.obj.w(w, v, c); return;
    case 'n': T.name.w(w, v, c); return;
    case 'e': writeExpr(w, c, v); return;
    case 'E': for (const e of v) writeExpr(w, c, e); return;
    case 'u8': w.u8(v); return;
    case 'u16': w.u16(v); return;
    case 'i32': w.i32(v); return;
    case 'u32': w.u32(v); return;
    case 'i64': T.i64.w(w, v, c); return;
    case 'u64': T.u64.w(w, v, c); return;
    case 'f32': T.f32.w(w, v, c); return;
    case 's': w.push(Buffer.from(v + '\0', 'latin1')); return;
    case 'S': if (typeof v === 'string') w.push(Buffer.from(v + '\0', 'utf16le')); else { w.hex(v.hex16); w.u16(0); } return;
    case 'text':
      w.u8(v.literalType);
      if (v.literalType === 1) { writeExpr(w, c, v.source); writeExpr(w, c, v.key); writeExpr(w, c, v.namespace); }
      else if (v.literalType === 2 || v.literalType === 3) writeExpr(w, c, v.source);
      else if (v.literalType === 4) { T.obj.w(w, v.tableAsset, c); writeExpr(w, c, v.tableId); writeExpr(w, c, v.key); }
      return;
    case 'switch':
      w.u16(v.cases.length); w.u32(v.endOffset); writeExpr(w, c, v.index);
      for (const k of v.cases) { writeExpr(w, c, k.value); w.u32(k.nextOffset); writeExpr(w, c, k.result); }
      writeExpr(w, c, v.default);
      return;
    case 'instr': w.u8(v.type); if (v.type === 4) T.name.w(w, v.name, c); return;
    default: throw new Error('operand kind ' + o.t);
  }
}

// UStruct script: int32 in-memory size, int32 serialized size, serialized bytes.
const script = {
  r: (r, c) => {
    const memSize = r.i32(), diskSize = r.i32();
    if (diskSize < 0 || memSize < 0) throw new Error('script sizes');
    const sub = new r.constructor(r.bytes(diskSize));
    const st = { mem: 0 };
    const code = [];
    while (sub.p < sub.b.length) code.push(readExpr(sub, c, st));
    if (st.mem !== memSize) throw new Error(`script in-memory size ${st.mem} != recorded ${memSize}`);
    if (c.stats) { c.stats.scriptBytes = (c.stats.scriptBytes || 0) + diskSize; }
    return code;
  },
  w: (w, code, c) => {
    const sub = new w.constructor(256);
    for (const x of code) writeExpr(sub, c, x);
    // in-memory size: disk size + 4 per object pointer + 4 per name
    const st = { mem: 0 };
    const rr = new (require('./package').R)(sub.b.subarray(0, sub.len));
    while (rr.p < rr.b.length) readExpr(rr, c, st);
    w.i32(st.mem); w.i32(sub.len); w.push(sub.b.subarray(0, sub.len));
  },
};

module.exports = { OPS, BY_NAME, script, readExpr, writeExpr };
