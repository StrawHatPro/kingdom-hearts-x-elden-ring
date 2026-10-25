// AVM2 (ActionScript 3) ABC reader, disassembler and a light decompiler for the DoABC blocks of the
// game's Scaleform movies. The decompiler rebuilds expressions from the operand stack and keeps
// control flow as `if (c) goto L<n>` with labels: enough to read class/timeline logic by hand.
// Used by tools/sora_ui_pack.js (via tools/lib/gfx.js); evidence tooling, not a general ABC library.
'use strict';

class R {
  constructor(b, p) { this.b = b; this.p = p || 0; }
  u8() { return this.b[this.p++]; }
  u16() { const v = this.b.readUInt16LE(this.p); this.p += 2; return v; }
  s24() { const v = this.b.readIntLE(this.p, 3); this.p += 3; return v; }
  u30() { let v = 0, sh = 0; for (let i = 0; i < 5; i++) { const x = this.b[this.p++]; v += (x & 127) * Math.pow(2, sh); sh += 7; if (!(x & 128)) break; } return v >>> 0; }
  s32() { return this.u30() | 0; }
  d64() { const v = this.b.readDoubleLE(this.p); this.p += 8; return v; }
  str() { const n = this.u30(); const s = this.b.toString('utf8', this.p, this.p + n); this.p += n; return s; }
}

const NSK = { 0x08: '', 0x16: '', 0x17: 'internal', 0x18: 'protected', 0x19: 'explicit', 0x1a: 'static-protected', 0x05: 'private' };

function parse(buf, pos, len) {
  const r = new R(buf, pos); const abc = { minor: r.u16(), major: r.u16() };
  let n = r.u30(); abc.ints = [0]; for (let i = 1; i < n; i++) abc.ints.push(r.s32());
  n = r.u30(); abc.uints = [0]; for (let i = 1; i < n; i++) abc.uints.push(r.u30());
  n = r.u30(); abc.doubles = [NaN]; for (let i = 1; i < n; i++) abc.doubles.push(r.d64());
  n = r.u30(); abc.strings = ['']; for (let i = 1; i < n; i++) abc.strings.push(r.str());
  n = r.u30(); abc.ns = [{ kind: 0, name: '*' }]; for (let i = 1; i < n; i++) { const kind = r.u8(); abc.ns.push({ kind, name: abc.strings[r.u30()] }); }
  n = r.u30(); abc.nssets = [[]]; for (let i = 1; i < n; i++) { const c = r.u30(); const a = []; for (let k = 0; k < c; k++) a.push(r.u30()); abc.nssets.push(a); }
  n = r.u30(); abc.mn = [{ kind: 0 }];
  for (let i = 1; i < n; i++) {
    const kind = r.u8(); const m = { kind };
    if (kind === 0x07 || kind === 0x0d) { m.ns = r.u30(); m.name = r.u30(); }
    else if (kind === 0x0f || kind === 0x10) { m.name = r.u30(); m.rtns = true; }
    else if (kind === 0x11 || kind === 0x12) { m.rtns = true; m.rtname = true; }
    else if (kind === 0x09 || kind === 0x0e) { m.name = r.u30(); m.nsset = r.u30(); }
    else if (kind === 0x1b || kind === 0x1c) { m.nsset = r.u30(); m.rtname = true; }
    else if (kind === 0x1d) { m.base = r.u30(); const c = r.u30(); m.params = []; for (let k = 0; k < c; k++) m.params.push(r.u30()); }
    else throw new Error('multiname kind ' + kind);
    abc.mn.push(m);
  }
  abc.mname = function (i) {
    if (i === 0) return '*'; const m = abc.mn[i]; if (!m) return 'mn#' + i;
    if (m.kind === 0x1d) return abc.mname(m.base) + '.<' + m.params.map(abc.mname).join(',') + '>';
    if (m.rtname) return '[rt]';
    const nm = abc.strings[m.name] || '*';
    return (m.kind === 0x0d || m.kind === 0x10 || m.kind === 0x0e ? '@' : '') + nm;
  };
  abc.qname = function (i) {
    const m = abc.mn[i]; if (!m || i === 0) return '*';
    if (m.kind === 0x07 || m.kind === 0x0d) { const ns = abc.ns[m.ns]; const nm = abc.strings[m.name]; return ns && ns.name && ns.kind !== 5 ? ns.name + '::' + nm : nm; }
    return abc.mname(i);
  };
  n = r.u30(); abc.methods = [];
  for (let i = 0; i < n; i++) {
    const pc = r.u30(); const m = { ret: r.u30(), params: [] };
    for (let k = 0; k < pc; k++) m.params.push({ type: r.u30() });
    m.name = abc.strings[r.u30()]; m.flags = r.u8();
    if (m.flags & 0x08) { const oc = r.u30(); m.optional = []; for (let k = 0; k < oc; k++) { const val = r.u30(), kind = r.u8(); m.optional.push({ val, kind }); } }
    if (m.flags & 0x80) for (let k = 0; k < pc; k++) m.params[k].name = abc.strings[r.u30()];
    abc.methods.push(m);
  }
  n = r.u30(); abc.metadata = [];
  for (let i = 0; i < n; i++) { const md = { name: abc.strings[r.u30()], items: [] }; const c = r.u30(); const keys = []; for (let k = 0; k < c; k++) keys.push(r.u30()); for (let k = 0; k < c; k++) md.items.push([abc.strings[keys[k]], abc.strings[r.u30()]]); abc.metadata.push(md); }
  const traits = () => {
    const c = r.u30(); const out = [];
    for (let i = 0; i < c; i++) {
      const t = { name: r.u30() }; const k = r.u8(); t.kind = k & 15; t.attr = k >> 4;
      if (t.kind === 0 || t.kind === 6) { t.slot = r.u30(); t.type = r.u30(); t.vindex = r.u30(); if (t.vindex) t.vkind = r.u8(); }
      else if (t.kind === 4) { t.slot = r.u30(); t.classi = r.u30(); }
      else if (t.kind === 5) { t.slot = r.u30(); t.method = r.u30(); }
      else if (t.kind === 1 || t.kind === 2 || t.kind === 3) { t.disp = r.u30(); t.method = r.u30(); }
      else throw new Error('trait kind ' + t.kind);
      if (t.attr & 4) { const mc = r.u30(); t.metadata = []; for (let q = 0; q < mc; q++) t.metadata.push(r.u30()); }
      out.push(t);
    }
    return out;
  };
  n = r.u30(); abc.instances = [];
  for (let i = 0; i < n; i++) {
    const inst = { name: r.u30(), super: r.u30(), flags: r.u8() };
    if (inst.flags & 8) inst.protectedNs = r.u30();
    const ic = r.u30(); inst.interfaces = []; for (let k = 0; k < ic; k++) inst.interfaces.push(r.u30());
    inst.iinit = r.u30(); inst.traits = traits(); abc.instances.push(inst);
  }
  abc.classes = []; for (let i = 0; i < n; i++) abc.classes.push({ cinit: r.u30(), traits: traits() });
  n = r.u30(); abc.scripts = []; for (let i = 0; i < n; i++) abc.scripts.push({ init: r.u30(), traits: traits() });
  n = r.u30(); abc.bodies = {};
  for (let i = 0; i < n; i++) {
    const b = { method: r.u30(), maxStack: r.u30(), localCount: r.u30(), initScope: r.u30(), maxScope: r.u30() };
    const cl = r.u30(); b.codePos = r.p; b.codeLen = cl; r.p += cl;
    const ec = r.u30(); b.exceptions = []; for (let k = 0; k < ec; k++) b.exceptions.push({ from: r.u30(), to: r.u30(), target: r.u30(), type: r.u30(), name: r.u30() });
    b.traits = traits(); abc.bodies[b.method] = b;
  }
  abc.buf = buf; abc.end = r.p;
  return abc;
}

// opcode table: name, operand kinds (u = u30, m = multiname, s = string, i = int, U = uint, d = double,
// b = s8, B = u8, j = s24 branch, a = argc, S = lookupswitch, n = namespace, c = class, f = method)
const OP = {
  0x01: ['bkpt'], 0x02: ['nop'], 0x03: ['throw'], 0x04: ['getsuper', 'm'], 0x05: ['setsuper', 'm'], 0x06: ['dxns', 's'], 0x07: ['dxnslate'], 0x08: ['kill', 'u'], 0x09: ['label'],
  0x0c: ['ifnlt', 'j'], 0x0d: ['ifnle', 'j'], 0x0e: ['ifngt', 'j'], 0x0f: ['ifnge', 'j'], 0x10: ['jump', 'j'], 0x11: ['iftrue', 'j'], 0x12: ['iffalse', 'j'], 0x13: ['ifeq', 'j'], 0x14: ['ifne', 'j'], 0x15: ['iflt', 'j'], 0x16: ['ifle', 'j'], 0x17: ['ifgt', 'j'], 0x18: ['ifge', 'j'], 0x19: ['ifstricteq', 'j'], 0x1a: ['ifstrictne', 'j'],
  0x1b: ['lookupswitch', 'S'], 0x1c: ['pushwith'], 0x1d: ['popscope'], 0x1e: ['nextname'], 0x1f: ['hasnext'], 0x20: ['pushnull'], 0x21: ['pushundefined'], 0x23: ['nextvalue'], 0x24: ['pushbyte', 'b'], 0x25: ['pushshort', 'u'], 0x26: ['pushtrue'], 0x27: ['pushfalse'], 0x28: ['pushnan'], 0x29: ['pop'], 0x2a: ['dup'], 0x2b: ['swap'], 0x2c: ['pushstring', 's'], 0x2d: ['pushint', 'i'], 0x2e: ['pushuint', 'U'], 0x2f: ['pushdouble', 'd'], 0x30: ['pushscope'], 0x31: ['pushnamespace', 'n'], 0x32: ['hasnext2', 'u', 'u'],
  0x40: ['newfunction', 'f'], 0x41: ['call', 'a'], 0x42: ['construct', 'a'], 0x43: ['callmethod', 'u', 'a'], 0x44: ['callstatic', 'f', 'a'], 0x45: ['callsuper', 'm', 'a'], 0x46: ['callproperty', 'm', 'a'], 0x47: ['returnvoid'], 0x48: ['returnvalue'], 0x49: ['constructsuper', 'a'], 0x4a: ['constructprop', 'm', 'a'], 0x4c: ['callproplex', 'm', 'a'], 0x4e: ['callsupervoid', 'm', 'a'], 0x4f: ['callpropvoid', 'm', 'a'],
  0x53: ['applytype', 'a'], 0x55: ['newobject', 'a'], 0x56: ['newarray', 'a'], 0x57: ['newactivation'], 0x58: ['newclass', 'c'], 0x59: ['getdescendants', 'm'], 0x5a: ['newcatch', 'u'], 0x5d: ['findpropstrict', 'm'], 0x5e: ['findproperty', 'm'], 0x60: ['getlex', 'm'], 0x61: ['setproperty', 'm'], 0x62: ['getlocal', 'u'], 0x63: ['setlocal', 'u'], 0x64: ['getglobalscope'], 0x65: ['getscopeobject', 'B'], 0x66: ['getproperty', 'm'], 0x68: ['initproperty', 'm'], 0x6a: ['deleteproperty', 'm'], 0x6c: ['getslot', 'u'], 0x6d: ['setslot', 'u'], 0x6e: ['getglobalslot', 'u'], 0x6f: ['setglobalslot', 'u'],
  0x70: ['convert_s'], 0x71: ['esc_xelem'], 0x72: ['esc_xattr'], 0x73: ['convert_i'], 0x74: ['convert_u'], 0x75: ['convert_d'], 0x76: ['convert_b'], 0x77: ['convert_o'], 0x78: ['checkfilter'], 0x80: ['coerce', 'm'], 0x82: ['coerce_a'], 0x85: ['coerce_s'], 0x86: ['astype', 'm'], 0x87: ['astypelate'],
  0x90: ['negate'], 0x91: ['increment'], 0x92: ['inclocal', 'u'], 0x93: ['decrement'], 0x94: ['declocal', 'u'], 0x95: ['typeof'], 0x96: ['not'], 0x97: ['bitnot'],
  0xa0: ['add'], 0xa1: ['subtract'], 0xa2: ['multiply'], 0xa3: ['divide'], 0xa4: ['modulo'], 0xa5: ['lshift'], 0xa6: ['rshift'], 0xa7: ['urshift'], 0xa8: ['bitand'], 0xa9: ['bitor'], 0xaa: ['bitxor'], 0xab: ['equals'], 0xac: ['strictequals'], 0xad: ['lessthan'], 0xae: ['lessequals'], 0xaf: ['greaterthan'], 0xb0: ['greaterequals'], 0xb1: ['instanceof'], 0xb2: ['istype', 'm'], 0xb3: ['istypelate'], 0xb4: ['in'],
  0xc0: ['increment_i'], 0xc1: ['decrement_i'], 0xc2: ['inclocal_i', 'u'], 0xc3: ['declocal_i', 'u'], 0xc4: ['negate_i'], 0xc5: ['add_i'], 0xc6: ['subtract_i'], 0xc7: ['multiply_i'],
  0xd0: ['getlocal_0'], 0xd1: ['getlocal_1'], 0xd2: ['getlocal_2'], 0xd3: ['getlocal_3'], 0xd4: ['setlocal_0'], 0xd5: ['setlocal_1'], 0xd6: ['setlocal_2'], 0xd7: ['setlocal_3'],
  0xef: ['debug', 'D'], 0xf0: ['debugline', 'u'], 0xf1: ['debugfile', 's'],
};

function disasm(abc, body) {
  const r = new R(abc.buf, body.codePos); const end = body.codePos + body.codeLen; const out = [];
  while (r.p < end) {
    const at = r.p - body.codePos; const op = r.u8(); const d = OP[op];
    if (!d) { out.push({ at, op: 'op' + op.toString(16) }); continue; }
    const ins = { at, op: d[0] };
    for (let k = 1; k < d.length; k++) {
      const t = d[k];
      if (t === 'u') (ins.u = ins.u || []).push(r.u30());
      else if (t === 'm') ins.m = r.u30();
      else if (t === 's') ins.s = abc.strings[r.u30()];
      else if (t === 'i') ins.v = abc.ints[r.u30()];
      else if (t === 'U') ins.v = abc.uints[r.u30()];
      else if (t === 'd') ins.v = abc.doubles[r.u30()];
      else if (t === 'b') ins.v = (r.u8() << 24) >> 24;
      else if (t === 'B') ins.v = r.u8();
      else if (t === 'a') ins.argc = r.u30();
      else if (t === 'n') ins.ns = r.u30();
      else if (t === 'c') ins.classi = r.u30();
      else if (t === 'f') ins.method = r.u30();
      else if (t === 'j') { const o = r.s24(); ins.to = r.p - body.codePos + o; }
      else if (t === 'S') { const base = at; const def = r.s24(); const c = r.u30(); ins.def = base + def; ins.cases = []; for (let q = 0; q <= c; q++) ins.cases.push(base + r.s24()); }
      else if (t === 'D') { ins.dtype = r.u8(); ins.s = abc.strings[r.u30()]; ins.reg = r.u8(); r.u30(); }
    }
    if (ins.op === 'pushshort') { ins.v = (ins.u[0] << 16) >> 16; delete ins.u; }
    out.push(ins);
  }
  return out;
}

const q = s => JSON.stringify(s);
const isId = s => /^[A-Za-z_$][\w$]*$/.test(s);
const BIN = { add: '+', subtract: '-', multiply: '*', divide: '/', modulo: '%', lshift: '<<', rshift: '>>', urshift: '>>>', bitand: '&', bitor: '|', bitxor: '^', equals: '==', strictequals: '===', lessthan: '<', lessequals: '<=', greaterthan: '>', greaterequals: '>=', instanceof: 'instanceof', in: 'in', add_i: '+', subtract_i: '-', multiply_i: '*', istypelate: 'is', astypelate: 'as' };
const CMP = { ifnlt: (a, b) => '!(' + a + ' < ' + b + ')', ifnle: (a, b) => '!(' + a + ' <= ' + b + ')', ifngt: (a, b) => '!(' + a + ' > ' + b + ')', ifnge: (a, b) => '!(' + a + ' >= ' + b + ')', ifeq: (a, b) => a + ' == ' + b, ifne: (a, b) => a + ' != ' + b, iflt: (a, b) => a + ' < ' + b, ifle: (a, b) => a + ' <= ' + b, ifgt: (a, b) => a + ' > ' + b, ifge: (a, b) => a + ' >= ' + b, ifstricteq: (a, b) => a + ' === ' + b, ifstrictne: (a, b) => a + ' !== ' + b };

// returns lines of pseudo-AS3 for one method body
function decompile(abc, mi, ind, isStatic) {
  const body = abc.bodies[mi]; if (!body) return [ind + '/* native / no body */'];
  const code = disasm(abc, body); const m = abc.methods[mi];
  const targets = new Set(); for (const i of code) { if (i.to !== undefined) targets.add(i.to); if (i.cases) { targets.add(i.def); i.cases.forEach(c => targets.add(c)); } }
  for (const e of body.exceptions) targets.add(e.target);
  const names = { 0: 'this' }; m.params.forEach((p, k) => { names[k + 1] = 'a' + (k + 1); }); // param-name strings in these files are unreliable
  
  const L = r => names[r] !== undefined ? names[r] : 'l' + r;
  const out = []; let st = [];
  const pop = () => st.length ? st.pop() : '/*empty*/';
  const args = n => { const a = []; for (let k = 0; k < n; k++) a.unshift(pop()); return a.join(', '); };
  // property reference: pops runtime name / namespace then the object
  const prop = (mnIdx, noObj) => {
    const mm = abc.mn[mnIdx]; let nm;
    if (mm && mm.rtname) nm = '[' + pop() + ']'; else nm = abc.mname(mnIdx);
    if (mm && mm.rtns) pop();
    if (noObj) return nm.startsWith('[') ? nm : nm;
    const o = pop();
    if (nm.startsWith('[')) return o + nm;
    if (!isId(nm)) return o + '[' + q(nm) + ']';
    return o === '<scope>' ? nm : o + '.' + nm;
  };
  const emit = s => out.push(ind + s);
  // ternaries: `if (!c) goto A; <x>; jump B; A: <y>; B:` carries x over the jump; short-circuit: `dup; iffalse L; pop; <b>; L:`
  const carried = {}, andor = {}; let dead = false;
  for (let k = 0; k < code.length; k++) {
    const i = code[k]; const prev = code[k - 1], next = code[k + 1];
    if (targets.has(i.at)) {
      const cl = carried[i.at]; const c = cl && cl[cl.length - 1];
      if (c && dead) { st = c.st.slice(); }
      else if (c && st.length === c.st.length && st.length) { const y = st.pop(); const top = e => e.st[e.st.length - 1]; st = c.st.slice(0, -1); st.push(cl.length > 1 ? 'select(' + cl.map(e => (e.cond || '?') + ' -> ' + top(e)).join(', ') + ', else -> ' + y + ')' : c.cond ? '(' + c.cond + ' ? ' + top(c) + ' : ' + y + ')' : 'phi(' + top(c) + ', ' + y + ')'); }
      const ao = andor[i.at]; if (ao && st.length) { const b = st.pop(); st.push('(' + ao.lhs + ' ' + ao.op + ' ' + b + ')'); }
      if (st.length && !c && !ao) emit('/*stack: ' + st.join(' | ') + '*/');
      out.push(ind.slice(2) + 'L' + i.at + ':'); dead = false;
    }
    if ((i.op === 'iffalse' || i.op === 'iftrue') && prev && prev.op === 'dup' && next && next.op === 'pop' && i.to > i.at) { pop(); const lhs = pop(); andor[i.to] = { lhs, op: i.op === 'iffalse' ? '&&' : '||' }; k++; continue; }
    if (i.op === 'jump' && st.length && i.to > i.at) {
      let cond = null; const last = out[out.length - 1]; const mm = last && /^\s*if \((.*)\) goto L(\d+);$/.exec(last);
      if (mm && next && +mm[2] === next.at) { out.pop(); cond = '!(' + mm[1] + ')'; if (/^!\(!/.test(cond)) cond = cond.slice(3, -1); }
      (carried[i.to] = carried[i.to] || []).push({ st: st.slice(), cond }); st = st.slice(0, -1); dead = true; continue;
    }
    if (i.op === 'jump' || i.op === 'returnvoid' || i.op === 'returnvalue' || i.op === 'throw' || i.op === 'lookupswitch') dead = true;
    const op = i.op;
    if (BIN[op]) { const b = pop(), a = pop(); st.push('(' + a + ' ' + BIN[op] + ' ' + b + ')'); continue; }
    if (CMP[op]) { const b = pop(), a = pop(); emit('if (' + CMP[op](a, b) + ') goto L' + i.to + ';'); continue; }
    switch (op) {
      case 'nop': case 'label': case 'debugline': case 'debugfile': case 'debug': case 'bkpt': break;
      case 'getlocal_0': case 'getlocal_1': case 'getlocal_2': case 'getlocal_3': st.push(L(+op.slice(-1))); break;
      case 'getlocal': st.push(L(i.u[0])); break;
      case 'setlocal_0': case 'setlocal_1': case 'setlocal_2': case 'setlocal_3': emit(L(+op.slice(-1)) + ' = ' + pop() + ';'); break;
      case 'setlocal': emit(L(i.u[0]) + ' = ' + pop() + ';'); break;
      case 'kill': break;
      case 'pushscope': case 'pushwith': pop(); break;
      case 'popscope': break;
      case 'pushnull': st.push('null'); break;
      case 'pushundefined': st.push('undefined'); break;
      case 'pushtrue': st.push('true'); break;
      case 'pushfalse': st.push('false'); break;
      case 'pushnan': st.push('NaN'); break;
      case 'pushbyte': case 'pushshort': case 'pushint': case 'pushuint': case 'pushdouble': st.push(String(i.v)); break;
      case 'pushstring': st.push(q(i.s)); break;
      case 'pushnamespace': st.push('ns(' + q(abc.ns[i.ns].name) + ')'); break;
      case 'pop': { const e = pop(); if (/\(|=/.test(e)) emit(e + ';'); break; }
      case 'dup': { const e = pop(); st.push(e, e); break; }
      case 'swap': { const b = pop(), a = pop(); st.push(b, a); break; }
      case 'getglobalscope': st.push('<scope>'); break;
      case 'getscopeobject': st.push(i.v === 0 ? 'this' : '<scope' + i.v + '>'); break;
      case 'newactivation': st.push('<activation>'); break;
      case 'newcatch': st.push('<catch>'); break;
      case 'findpropstrict': case 'findproperty': { const mm = abc.mn[i.m]; if (mm && mm.rtname) pop(); if (mm && mm.rtns) pop(); st.push('<scope>'); break; }
      case 'getlex': st.push(abc.mname(i.m)); break;
      case 'getproperty': st.push(prop(i.m)); break;
      case 'getsuper': { const p = prop(i.m); st.push('super:' + p); break; }
      case 'getdescendants': st.push(prop(i.m) + '/*descendants*/'); break;
      case 'setproperty': case 'initproperty': { const v = pop(); emit(prop(i.m) + ' = ' + v + ';'); break; }
      case 'setsuper': { const v = pop(); emit('super:' + prop(i.m) + ' = ' + v + ';'); break; }
      case 'deleteproperty': st.push('delete ' + prop(i.m)); break;
      case 'getslot': st.push(pop() + '.slot' + i.u[0]); break;
      case 'setslot': { const v = pop(); emit(pop() + '.slot' + i.u[0] + ' = ' + v + ';'); break; }
      case 'getglobalslot': st.push('global.slot' + i.u[0]); break;
      case 'setglobalslot': emit('global.slot' + i.u[0] + ' = ' + pop() + ';'); break;
      case 'callproperty': case 'callproplex': { const a = args(i.argc); st.push(prop(i.m) + '(' + a + ')'); break; }
      case 'callpropvoid': { const a = args(i.argc); emit(prop(i.m) + '(' + a + ');'); break; }
      case 'callsuper': { const a = args(i.argc); const p = prop(i.m, true); pop(); st.push('super.' + p + '(' + a + ')'); break; }
      case 'callsupervoid': { const a = args(i.argc); const p = prop(i.m, true); pop(); emit('super.' + p + '(' + a + ');'); break; }
      case 'constructsuper': { const a = args(i.argc); pop(); emit('super(' + a + ');'); break; }
      case 'constructprop': { const a = args(i.argc); const p = prop(i.m); st.push('new ' + p + '(' + a + ')'); break; }
      case 'construct': { const a = args(i.argc); st.push('new ' + pop() + '(' + a + ')'); break; }
      case 'call': { const a = args(i.argc); pop(); st.push(pop() + '(' + a + ')'); break; }
      case 'callmethod': { const a = args(i.argc); st.push(pop() + '.method#' + i.u[0] + '(' + a + ')'); break; }
      case 'callstatic': { const a = args(i.argc); pop(); st.push('static#' + i.method + '(' + a + ')'); break; }
      case 'applytype': { const a = args(i.argc); st.push(pop() + '.<' + a + '>'); break; }
      case 'newobject': { const kv = []; for (let k = 0; k < i.argc; k++) { const v = pop(), key = pop(); kv.unshift(key + ': ' + v); } st.push('{' + kv.join(', ') + '}'); break; }
      case 'newarray': st.push('[' + args(i.argc) + ']'); break;
      case 'newclass': pop(); st.push('<class ' + abc.mname(abc.instances[i.classi].name) + '>'); break;
      case 'newfunction': st.push('function#' + i.method); break;
      case 'returnvoid': emit('return;'); break;
      case 'returnvalue': emit('return ' + pop() + ';'); break;
      case 'throw': emit('throw ' + pop() + ';'); break;
      case 'jump': emit('goto L' + i.to + ';'); break;
      case 'iftrue': emit('if (' + pop() + ') goto L' + i.to + ';'); break;
      case 'iffalse': emit('if (!' + pop() + ') goto L' + i.to + ';'); break;
      case 'lookupswitch': emit('switch (' + pop() + ') { ' + i.cases.map((c, k) => k + ': L' + c).join(', ') + ', default: L' + i.def + ' }'); break;
      case 'convert_s': case 'coerce_s': case 'convert_d': case 'convert_b': case 'convert_o': case 'coerce_a': case 'coerce': case 'checkfilter': case 'esc_xelem': case 'esc_xattr': break;
      case 'convert_i': st.push('int(' + pop() + ')'); break;
      case 'convert_u': st.push('uint(' + pop() + ')'); break;
      case 'astype': st.push('(' + pop() + ' as ' + abc.mname(i.m) + ')'); break;
      case 'istype': st.push('(' + pop() + ' is ' + abc.mname(i.m) + ')'); break;
      case 'negate': case 'negate_i': st.push('-' + pop()); break;
      case 'increment': case 'increment_i': st.push('(' + pop() + ' + 1)'); break;
      case 'decrement': case 'decrement_i': st.push('(' + pop() + ' - 1)'); break;
      case 'inclocal': case 'inclocal_i': emit(L(i.u[0]) + '++;'); break;
      case 'declocal': case 'declocal_i': emit(L(i.u[0]) + '--;'); break;
      case 'typeof': st.push('typeof ' + pop()); break;
      case 'not': st.push('!' + pop()); break;
      case 'bitnot': st.push('~' + pop()); break;
      case 'hasnext2': st.push('hasnext2(' + L(i.u[0]) + ', ' + L(i.u[1]) + ')'); break;
      case 'hasnext': { const b = pop(), a = pop(); st.push('hasnext(' + a + ', ' + b + ')'); break; }
      case 'nextname': { const b = pop(), a = pop(); st.push('nextname(' + a + ', ' + b + ')'); break; }
      case 'nextvalue': { const b = pop(), a = pop(); st.push('nextvalue(' + a + ', ' + b + ')'); break; }
      case 'dxns': case 'dxnslate': break;
      default: emit('/* ' + op + ' */');
    }
  }
  if (st.length) emit('/*stack left: ' + st.join(' | ') + '*/');
  return out;
}

const constVal = (abc, t) => {
  if (!t.vindex) return undefined;
  switch (t.vkind) { case 0x03: return abc.ints[t.vindex]; case 0x04: return abc.uints[t.vindex]; case 0x06: return abc.doubles[t.vindex]; case 0x01: return q(abc.strings[t.vindex]); case 0x0b: return true; case 0x0a: return false; case 0x0c: return null; default: return 'ns'; }
};

function sig(abc, mi) { const m = abc.methods[mi]; return '(' + m.params.map((p, k) => 'a' + (k + 1) + ':' + abc.mname(p.type)).join(', ') + '):' + abc.mname(m.ret); }

// full text listing of an ABC block
function listing(abc) {
  const L = [];
  const dumpTraits = (traits, pre, isStatic) => {
    for (const t of traits) {
      const nm = abc.mname(t.name);
      if (t.kind === 0 || t.kind === 6) { const v = constVal(abc, t); L.push(`  ${pre}${t.kind === 6 ? 'const' : 'var'} ${nm}:${abc.mname(t.type)}${v !== undefined ? ' = ' + v : ''};`); }
    }
    for (const t of traits) {
      const nm = abc.mname(t.name);
      if (t.kind === 1 || t.kind === 2 || t.kind === 3 || t.kind === 5) {
        const k = t.kind === 2 ? 'get ' : t.kind === 3 ? 'set ' : '';
        L.push(''); L.push(`  ${pre}function ${k}${nm}${sig(abc, t.method)} {  // method ${t.method}`);
        for (const l of decompile(abc, t.method, '    ', isStatic)) L.push(l);
        L.push('  }');
      }
    }
  };
  abc.instances.forEach((inst, ci) => {
    L.push(`class ${abc.qname(inst.name)} extends ${abc.qname(inst.super)}${inst.interfaces.length ? ' implements ' + inst.interfaces.map(abc.mname).join(', ') : ''} {`);
    dumpTraits(abc.classes[ci].traits, 'static ', true);
    L.push(''); L.push(`  static function <cinit>() {  // method ${abc.classes[ci].cinit}`);
    for (const l of decompile(abc, abc.classes[ci].cinit, '    ', true)) L.push(l);
    L.push('  }');
    dumpTraits(inst.traits, '', false);
    L.push(''); L.push(`  function <init>${sig(abc, inst.iinit)} {  // method ${inst.iinit}`);
    for (const l of decompile(abc, inst.iinit, '    ')) L.push(l);
    L.push('  }'); L.push('}'); L.push('');
  });
  // anonymous functions (newfunction targets) and script-level functions
  const used = new Set(); abc.instances.forEach((inst, ci) => { used.add(inst.iinit); used.add(abc.classes[ci].cinit); for (const t of inst.traits.concat(abc.classes[ci].traits)) if (t.method !== undefined) used.add(t.method); });
  abc.scripts.forEach((s, si) => { used.add(s.init); for (const t of s.traits) if (t.method !== undefined && t.kind !== 4) { L.push(`script function ${abc.mname(t.name)}${sig(abc, t.method)} {  // method ${t.method}`); for (const l of decompile(abc, t.method, '  ')) L.push(l); L.push('}'); L.push(''); used.add(t.method); } });
  for (const mi of Object.keys(abc.bodies).map(Number)) if (!used.has(mi)) { L.push(`function#${mi}${sig(abc, mi)} {`); for (const l of decompile(abc, mi, '  ')) L.push(l); L.push('}'); L.push(''); }
  return L.join('\n');
}

module.exports = { parse, disasm, decompile, listing };
